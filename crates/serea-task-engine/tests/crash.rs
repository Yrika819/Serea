//! P2H child-process crash harness, fresh-process verification, and the
//! N1-N8 / F25 / F26 evidence.
//!
//! # Process architecture
//!
//! Three distinct OS processes, never two roles in one process:
//!
//! ```text
//! parent coordinator (a #[test] in this binary)
//!   -> writer/crash child  (current_exe() re-invoked, SEREA_P2H_ROLE=child)
//!   -> fresh verifier child (current_exe() re-invoked, SEREA_P2H_ROLE=verify)
//! ```
//!
//! The crashing child and the verifying child are both different processes from
//! the parent, and from each other. The parent never opens the fixture while a
//! crash child holds it, so no assertion can be satisfied by in-process state.
//! A crash child that returns is treated as a test failure: `Err` before commit
//! is not a crash (test matrix §3).
//!
//! # Determinism
//!
//! A child reaches a named private transaction stage and only then creates an
//! acknowledgement file. The parent polls for that file with a bounded budget
//! and then sends SIGKILL. The file is the authority, never a sleep. Physical
//! SQLite/WAL bytes are never compared; only durable logical row sets are.
//!
//! # What is NOT a crash test
//!
//! N8 and the audit/savepoint seams inject a deterministic typed error and
//! assert rollback. They run in this process and say so in their names.
//!
//! No network, shell, external service, credential, wall clock or randomness
//! is used here or in the storage fault seam.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;
mod support;
use support::event_bus;

use rusqlite::{Connection, types::ValueRef};
use serea_protocol::*;
use serea_storage::fault::{Action, Window};
use serea_storage::{Store, StoreError};
use serea_task_engine::*;

const ROLE: &str = "SEREA_P2H_ROLE";
const MODE: &str = "SEREA_P2H_MODE";
const DIR: &str = "SEREA_P2H_DIR";
const STAGE: &str = "SEREA_P2H_STAGE";
const TARGET: &str = "SEREA_P2H_TARGET";

const CHILD: &str = "p2h_child_entry";
const VERIFY: &str = "p2h_fresh_verifier_entry";
const WRITER: &str = "p2h_stress_writer_entry";
const PROBE: &str = "p2h_tempstore_identity_probe";

/// Bounded poll budget. The acknowledgement file is the authority; this only
/// bounds how long a broken harness waits before failing.
const POLLS: usize = 4_000;
const TICK: Duration = Duration::from_millis(5);

// ---------------------------------------------------------------------------
// Fixture identity: F25 and F27
// ---------------------------------------------------------------------------

static NEXT: AtomicU64 = AtomicU64::new(0);

/// The binary identity component: the sanitised current executable file name.
fn binary_identity() -> String {
    std::env::current_exe()
        .unwrap()
        .file_name()
        .unwrap()
        .to_string_lossy()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect()
}

/// The one directory derivation used by every P2H fixture.
///
/// Identity is `(label, binary identity, pid, atomic counter)`. The binary
/// component is exactly what F25 requires: a counter alone collides across
/// integration-test binaries, and a pid alone is insufficient because
/// `cargo test` runs tests as threads of one process while this harness also
/// spawns children. There is no wall-clock and no randomness input, which is
/// what F27 requires.
fn derive_dir(label: &str, identity: &str, pid: u32, counter: u64) -> PathBuf {
    std::env::temp_dir().join(format!("serea-p2h-{label}-{identity}-{pid}-{counter}"))
}

struct Fixture {
    dir: PathBuf,
    path: PathBuf,
    /// Whether this handle owns the directory and may delete it on drop. Only
    /// the creating process may. A child that inherits a location must never
    /// delete it, or its normal exit would destroy the parent's evidence.
    owns_dir: bool,
}

impl Fixture {
    /// Mints a fresh fixture directory with a full identity.
    fn create(label: &str) -> Self {
        let identity = binary_identity();
        let pid = std::process::id();
        loop {
            let counter = NEXT.fetch_add(1, Ordering::Relaxed);
            let dir = std::env::temp_dir()
                .canonicalize()
                .expect("temporary directory must be resolvable")
                .join(format!("serea-p2h-{label}-{identity}-{pid}-{counter}"));
            match fs::create_dir(&dir) {
                Ok(()) => {
                    let fixture = Self {
                        path: dir.join("store.sqlite"),
                        dir,
                        owns_dir: true,
                    };
                    support::seed_active_registry_generation(
                        &Store::open(&fixture.path, &Fixed).unwrap(),
                    );
                    return fixture;
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => panic!("cannot create fixture directory: {error}"),
            }
        }
    }

    /// F26: adopts the parent's exact directory. Never mints a new identity,
    /// never creates a second database and never deletes the location, because
    /// this process does not own it.
    fn child_inherited(dir: &Path) -> Self {
        let path = dir.join("store.sqlite");
        assert!(path.is_file(), "inherited fixture has no database file");
        Self {
            dir: dir.to_path_buf(),
            path,
            owns_dir: false,
        }
    }

    fn open(&self) -> TaskEngine {
        support::engine(Store::open(&self.path, &Fixed).unwrap(), event_bus())
    }

    fn sql(&self) -> Connection {
        let conn = Connection::open(&self.path).unwrap();
        conn.busy_timeout(Duration::from_millis(5000)).unwrap();
        conn.pragma_update(None, "foreign_keys", "ON").unwrap();
        conn
    }

    fn count(&self, sql: &str) -> i64 {
        self.sql().query_row(sql, [], |row| row.get(0)).unwrap()
    }

    fn dump(&self) -> Dump {
        logical_dump(&self.path)
    }

    fn ack(&self, window: Window) -> PathBuf {
        self.dir.join(format!("ack-{window:?}"))
    }

    fn dir_var(&self) -> (&str, &str) {
        (DIR, self.dir.to_str().unwrap())
    }

    /// Asserts no table other than `allowed` gained or lost a durable row.
    fn only_changed(&self, before: &Dump, allowed: &[&str]) {
        let after = self.dump();
        assert_eq!(
            before.keys().collect::<Vec<_>>(),
            after.keys().collect::<Vec<_>>(),
            "no new or removed durable table"
        );
        for (table, rows) in before {
            if !allowed.contains(&table.as_str()) {
                assert_eq!(rows, &after[table], "unexpected durable change to {table}");
            }
        }
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        if self.owns_dir {
            let _ = fs::remove_dir_all(&self.dir);
        }
    }
}

/// Durable logical state: every table, type-tagged and length-framed, from one
/// read snapshot, rows sorted. Physical SQLite/WAL bytes are never asserted.
type Dump = BTreeMap<String, Vec<Vec<u8>>>;

fn logical_dump(path: &Path) -> Dump {
    let mut conn = Connection::open(path).unwrap();
    let tx = conn.transaction().unwrap();
    let names: Vec<String> = tx
        .prepare("SELECT name FROM sqlite_schema WHERE type='table' ORDER BY name")
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    let mut dump = Dump::new();
    for name in names {
        let mut stmt = tx
            .prepare(&format!("SELECT * FROM \"{}\"", name.replace('"', "\"\"")))
            .unwrap();
        let columns = stmt.column_count();
        let mut rows = stmt.query([]).unwrap();
        let mut encoded = Vec::new();
        while let Some(row) = rows.next().unwrap() {
            let mut bytes = Vec::new();
            for column in 0..columns {
                let (tag, value): (u8, Vec<u8>) = match row.get_ref(column).unwrap() {
                    ValueRef::Null => (0, vec![]),
                    ValueRef::Integer(n) => (1, n.to_be_bytes().to_vec()),
                    ValueRef::Real(n) => (2, n.to_bits().to_be_bytes().to_vec()),
                    ValueRef::Text(s) => (3, s.to_vec()),
                    ValueRef::Blob(b) => (4, b.to_vec()),
                };
                bytes.push(tag);
                bytes.extend_from_slice(&(value.len() as u64).to_be_bytes());
                bytes.extend_from_slice(&value);
            }
            encoded.push(bytes);
        }
        encoded.sort();
        dump.insert(name, encoded);
    }
    tx.commit().unwrap();
    dump
}

// ---------------------------------------------------------------------------
// Deterministic protocol fixtures
// ---------------------------------------------------------------------------

struct Fixed;
impl Clock for Fixed {
    fn now_ms(&self) -> Result<EpochMillis, ProtocolError> {
        EpochMillis::new(0)
    }
}

fn at(n: i64) -> EpochMillis {
    EpochMillis::new(n).unwrap()
}
fn tid(n: u32) -> TaskId {
    TaskId::new(format!("tsk_{n:026}")).unwrap()
}
fn sid(n: u32) -> StepId {
    StepId::new(format!("stp_{n:026}")).unwrap()
}
fn rid(n: u32) -> ReceiptId {
    ReceiptId::new(format!("rcp_{n:026}")).unwrap()
}
fn owner() -> LeaseOwner {
    LeaseOwner::new("crash-host").unwrap()
}

struct Context {
    actor: ActorId,
    version: SemVer,
    cause: EventId,
}
impl Context {
    fn new() -> Self {
        Self {
            actor: ActorId::new("crash-host").unwrap(),
            version: SemVer::new("0.2.0").unwrap(),
            cause: EventId::new("evt_00000000000000000000000001").unwrap(),
        }
    }
    fn view(&self) -> TransitionContext<'_> {
        TransitionContext {
            actor_kind: ActorKind::Host,
            actor_id: &self.actor,
            actor_version: &self.version,
            causation_id: Some(&self.cause),
        }
    }
}

fn spec(n: u32, ceiling: u32) -> NewTask {
    NewTask {
        task_id: tid(n),
        kind: TaskKind::UserRequest,
        title: TaskTitle::new("crash window title sentinel").unwrap(),
        origin: TaskOrigin {
            kind: TaskOriginKind::new("USER_MESSAGE").unwrap(),
            device_id: None,
            message_id: None,
            extensions: [("future_origin".into(), serde_json::json!({"n":[1,null]}))].into(),
        },
        data_class: DataClass::Personal,
        policy_class: RiskClass::Communication,
        attempt_budget: AttemptBudget {
            max_model_calls: 12,
            max_tool_calls: 24,
            max_attempts_per_step: ceiling,
            extensions: [("future_budget".into(), serde_json::json!(true))].into(),
        },
        created_at: at(10),
        deadline_at: Some(at(1000)),
        extensions: [("future_task".into(), serde_json::json!({"x":true}))].into(),
    }
}

fn input(task: u32, step: u32, sequence: u32, kind: StepKind) -> PlanStep {
    let raw = format!("{{\"value\":{step}}}").into_bytes();
    let shaped = matches!(
        kind,
        StepKind::Capability | StepKind::Delegate | StepKind::Verify
    );
    let capability = CapabilityId::new("calendar.events.create").unwrap();
    let version = SemVer::new("1.0.0").unwrap();
    let step = TaskStep::new(TaskStepDraft {
        task_id: tid(task),
        step_id: sid(step),
        sequence,
        kind,
        status: StepStatus::new("PLANNED").unwrap(),
        attempt: 0,
        idempotency_key: shaped.then(|| {
            derive_idempotency_key(
                &tid(task),
                &sid(step),
                &capability,
                &version,
                std::str::from_utf8(&raw).unwrap(),
            )
            .unwrap()
        }),
        provider_id: shaped.then(|| ProviderId::new("calendar").unwrap()),
        capability_id: shaped.then_some(capability),
        capability_version: shaped.then_some(version),
        input_digest: digest_of(std::str::from_utf8(&raw).unwrap()).unwrap(),
        result_digest: None,
        side_effect_receipt: None,
        started_at: None,
        completed_at: None,
        lease_owner: None,
        lease_expires_at: None,
        lease_generation: None,
        error: None,
        extensions: [("future_step".into(), serde_json::json!({"n":7}))].into(),
    })
    .unwrap();
    PlanStep {
        step,
        input_json: raw,
    }
}

const RESULT: &[u8] = b"{\"event_reference\":\"calendar-event-42\"}";

/// SCJ-1 canonical SHA-256 plaintext digest of [`RESULT`], the blob the outcome
/// transaction writes. Pinned so a test can prove that blob row's absence or
/// presence directly rather than inferring it from a reference row.
const RESULT_DIGEST: &str =
    "sha256:6687c172e3c72bc8b4b6e37ba75ac45477193414e8e5c1b9cd63d80a9243939c";

fn receipt(step: &TaskStep) -> SideEffectReceipt {
    let n = step.sequence;
    SideEffectReceipt {
        receipt_id: rid(n),
        capability_id: step.capability_id.clone().unwrap(),
        idempotency_key: step.idempotency_key.clone().unwrap(),
        provider_reference: Some(ProviderReference::new("calendar-event-42").unwrap()),
        effect_summary: EffectSummary::new("crash window effect sentinel").unwrap(),
        observed_at: Timestamp::from_epoch_millis(at(42)),
        replay_safe: false,
    }
}

/// Stages the fixture durably in the parent, then releases the connection so a
/// child can open the very same durable file.
///
/// * `insert` - migrated schema only; the child's `create_task` is the first
///   durable write in the whole database.
/// * `n6` - RECEIVED -> PLANNING -> READY. The child acquires, begins and
///   commits the outcome, so the crash window sits in the commit.
/// * `terminal` - as `n6`, plus a fully committed ordinary step, leaving the
///   task VERIFYING so the child's verify-step commit completes it.
fn setup(fixture: &Fixture, stage: &str) {
    let c = Context::new();
    let mut e = fixture.open();
    if stage == "insert" {
        drop(e);
        return;
    }
    e.create_task(spec(1, 3), &c.view()).unwrap();
    e.start_planning(tid(1), TaskState::Received, 0, at(20), &c.view())
        .unwrap();
    let steps = if stage == "terminal" {
        vec![
            input(1, 1, 10, StepKind::Capability),
            input(1, 2, 20, StepKind::Verify),
        ]
    } else {
        vec![input(1, 1, 10, StepKind::Capability)]
    };
    let store = Store::open(&fixture.path, &Fixed).unwrap();
    let plan = Plan { revision: 1, steps };
    let bindings = [serea_storage::CapabilityPlanBindingDraft {
        step_id: sid(1),
        descriptor_digest: support::fixture_descriptor_digest(),
    }];
    store
        .transact_with_participants(&TaskJournal, &event_bus(), |tx| {
            tx.put_capability_plan_revision(
                &tid(1),
                serea_storage::PlanWrite {
                    revision: plan.revision,
                    steps: plan
                        .steps
                        .into_iter()
                        .map(|step| serea_storage::StepInput {
                            step: step.step,
                            input_json: step.input_json,
                        })
                        .collect(),
                },
                &bindings,
                at(30),
                &c.view(),
            )
            .map(|_| ())
        })
        .unwrap();
    if stage == "terminal" {
        let first = e
            .acquire(tid(1), sid(1), owner(), None, at(40), at(500), &c.view())
            .unwrap();
        e.begin_attempt(&first, at(41), &c.view()).unwrap();
        let step = e.load(tid(1)).unwrap().steps[0].step.clone();
        e.commit_step(
            first,
            StepOutcome::Succeeded {
                result_json: RESULT,
                receipt: Some(&receipt(&step)),
            },
            at(42),
            &c.view(),
        )
        .unwrap();
    }
    drop(e);
}

/// Acquires, begins and commits the outcome the crash window targets.
fn run_outcome(e: &mut TaskEngine, c: &Context, step_number: u32, now: i64) {
    let guard = e
        .acquire(
            tid(1),
            sid(step_number),
            owner(),
            None,
            at(40),
            at(500),
            &c.view(),
        )
        .unwrap();
    e.begin_attempt(&guard, at(now), &c.view()).unwrap();
    let index = usize::try_from(step_number - 1).unwrap();
    let step = e.load(tid(1)).unwrap().steps[index].step.clone();
    e.commit_step(
        guard,
        StepOutcome::Succeeded {
            result_json: RESULT,
            receipt: Some(&receipt(&step)),
        },
        at(now + 1),
        &c.view(),
    )
    .unwrap();
}

fn step_of(e: &TaskEngine, index: usize) -> TaskStep {
    e.load(tid(1)).unwrap().steps[index].step.clone()
}

// ---------------------------------------------------------------------------
// Bounded, non-sleep child coordination
// ---------------------------------------------------------------------------

fn wait_for(path: &Path) -> bool {
    for _ in 0..POLLS {
        if path.exists() {
            return true;
        }
        std::thread::sleep(TICK);
    }
    path.exists()
}

fn spawn(test: &str, vars: &[(&str, &str)], cwd: &Path) -> Child {
    let mut command = Command::new(std::env::current_exe().unwrap());
    command
        .args(["--exact", test, "--nocapture", "--test-threads=1"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .current_dir(cwd);
    for (key, value) in vars {
        command.env(key, value);
    }
    command.spawn().unwrap()
}

fn wait_with_output_bounded(mut child: Child, label: &str) -> std::process::Output {
    for _ in 0..POLLS {
        match child.try_wait() {
            Ok(Some(_)) => return child.wait_with_output().unwrap(),
            Ok(None) => std::thread::sleep(TICK),
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                panic!("{label}: wait failed: {error}");
            }
        }
    }
    let _ = child.kill();
    let output = child.wait_with_output().unwrap();
    panic!(
        "{label}: child exceeded bounded wait\n{}",
        describe(&output)
    );
}

fn describe(output: &std::process::Output) -> String {
    format!(
        "status={:?}\n--- stdout ---\n{}\n--- stderr ---\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

/// Kills the child and proves it died by signal. A child that returned `Ok` or
/// `Err` is not a crash and is a test failure.
fn kill_and_prove_death(mut child: Child, label: &str) {
    child.kill().unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(
        !output.status.success(),
        "{label}: crash child reported success\n{}",
        describe(&output)
    );
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        assert_eq!(
            output.status.signal(),
            Some(9),
            "{label}: crash child was not SIGKILLed; returning Err is not a crash"
        );
    }
}

/// Runs a *fresh, separate* verifier process over the same durable store.
fn run_verifier(fixture: &Fixture, mode: &str, cwd: &Path) {
    let vars = [(ROLE, "verify"), (MODE, mode), fixture.dir_var()];
    let output = wait_with_output_bounded(spawn(VERIFY, &vars, cwd), "fresh verifier");
    assert!(
        output.status.success(),
        "fresh verifier rejected {mode}\n{}",
        describe(&output)
    );
}

// ---------------------------------------------------------------------------
// N1 / N2 / N3
// ---------------------------------------------------------------------------

/// Asserts the durable expectation for `mode`.
///
/// This runs **only inside the fresh verifier process**, never in the parent
/// that supervised the crash. The parent proves the acknowledgement arrived and
/// that the child died by signal; the fresh process proves the durable truth.
fn verify_durable(mode: &str, f: &Fixture) {
    let count = |sql: &str| f.count(sql);
    let outcome_rolls_back = || {
        // Nothing the outcome transaction wrote may be durable.
        assert_eq!(
            count("SELECT count(*) FROM task_steps WHERE status<>'EXECUTING'"),
            0,
            "{mode}"
        );
        assert_eq!(
            count("SELECT count(*) FROM task_steps WHERE result_digest IS NOT NULL"),
            0,
            "{mode}"
        );
        assert_eq!(
            count("SELECT count(*) FROM side_effect_receipts"),
            0,
            "{mode}"
        );
        // A result blob is the referenced side of the blob FK: the absence of
        // a RESULT reference would not by itself prove the blob is absent.
        assert_eq!(
            count(&format!(
                "SELECT count(*) FROM blobs WHERE digest='{RESULT_DIGEST}'"
            )),
            0,
            "{mode}"
        );
        assert_eq!(
            count("SELECT count(*) FROM step_blob_refs WHERE role='RESULT'"),
            0,
            "{mode}"
        );
        assert_eq!(
            count("SELECT count(*) FROM leases WHERE released_at_ms IS NOT NULL"),
            0,
            "{mode}"
        );
        assert_eq!(
            count(
                "SELECT count(*) FROM task_journal WHERE journal_kind IN ('STEP_COMMITTED','RECEIPT_RECORDED')"
            ),
            0,
            "{mode}"
        );
    };

    match mode {
        "n1" | "n2" | "n3" => {
            // Before the first durable write: nothing exists and the schema is intact.
            assert_eq!(count("SELECT count(*) FROM tasks"), 0, "{mode}");
            assert_eq!(count("SELECT count(*) FROM task_journal"), 0, "{mode}");
            assert_eq!(f.dump()["schema_migrations"].len(), 4, "{mode}");
        }
        "n4" => {
            // T4 holds in the conservative direction: nothing advanced.
            outcome_rolls_back();
            assert_eq!(
                count("SELECT count(*) FROM tasks WHERE state='EXECUTING'"),
                1,
                "{mode}"
            );
        }
        "n5" => {
            outcome_rolls_back();
            assert_eq!(
                count("SELECT count(*) FROM tasks WHERE state<>'EXECUTING'"),
                0,
                "{mode}"
            );
            // Rollback to the last commit, not data loss: the fixture is intact.
            assert_eq!(count("SELECT count(*) FROM plan_revisions"), 1, "{mode}");
            assert_eq!(
                count("SELECT count(*) FROM task_steps WHERE status='EXECUTING'"),
                1,
                "{mode}"
            );
            assert_eq!(
                count("SELECT count(*) FROM task_journal WHERE journal_kind='TASK_INSERTED'"),
                1,
                "{mode}"
            );
            assert_eq!(count("SELECT count(*) FROM leases"), 1, "{mode}");
        }
        "n6" | "n6-nonterminal" => {
            // Complete committed P2F state: step, result ref, receipt, aggregate
            // task state, journal batch and lease release, all atomically.
            assert_eq!(
                count("SELECT count(*) FROM task_steps WHERE status='SUCCEEDED'"),
                1,
                "{mode}"
            );
            assert_eq!(
                count("SELECT count(*) FROM tasks WHERE state='VERIFYING'"),
                1,
                "{mode}: committed ordinary outcome must advance aggregate to VERIFYING"
            );
            assert_eq!(
                count(&format!(
                    "SELECT count(*) FROM task_steps s JOIN step_blob_refs r ON r.step_id=s.step_id AND r.role='RESULT' AND r.digest=s.result_digest JOIN blobs b ON b.digest=r.digest AND b.data_class_rank=r.data_class_rank WHERE s.status='SUCCEEDED' AND s.result_digest='{RESULT_DIGEST}'"
                )),
                1,
                "{mode}: result digest, ref and durable blob must agree"
            );
            assert_eq!(
                count("SELECT count(*) FROM step_blob_refs WHERE role='RESULT'"),
                1,
                "{mode}"
            );
            assert_eq!(
                count(&format!(
                    "SELECT count(*) FROM blobs WHERE digest='{RESULT_DIGEST}'"
                )),
                1,
                "{mode}"
            );
            assert_eq!(
                count("SELECT count(*) FROM side_effect_receipts"),
                1,
                "{mode}"
            );
            assert_eq!(
                count("SELECT count(*) FROM task_journal WHERE journal_kind='STEP_COMMITTED'"),
                1,
                "{mode}"
            );
            assert_eq!(
                count("SELECT count(*) FROM task_journal WHERE journal_kind='RECEIPT_RECORDED'"),
                1,
                "{mode}"
            );
            assert_eq!(
                count("SELECT count(*) FROM leases WHERE released_at_ms IS NOT NULL"),
                1,
                "{mode}"
            );
            // N6a: durable truth is complete although the caller never learned it.
            assert!(
                !f.dir.join("success").exists(),
                "{mode}: child reported success"
            );
        }
        "n6-terminal" => {
            // The terminal fixture committed the ordinary step in the parent and
            // the verify step in the crashed child, so both outcomes are
            // complete and the task is terminal.
            assert_eq!(
                count("SELECT count(*) FROM tasks WHERE state='COMPLETED'"),
                1,
                "{mode}"
            );
            assert_eq!(
                count("SELECT count(*) FROM task_steps"),
                2,
                "{mode}: terminal fixture contains exactly ordinary and verify steps"
            );
            assert_eq!(
                count("SELECT count(*) FROM task_steps WHERE status<>'SUCCEEDED'"),
                0,
                "{mode}"
            );
            assert_eq!(
                count(&format!(
                    "SELECT count(*) FROM task_steps s JOIN step_blob_refs r ON r.step_id=s.step_id AND r.role='RESULT' AND r.digest=s.result_digest JOIN blobs b ON b.digest=r.digest AND b.data_class_rank=r.data_class_rank WHERE s.status='SUCCEEDED' AND s.result_digest='{RESULT_DIGEST}'"
                )),
                2,
                "{mode}: both terminal result digests, refs and durable blobs must agree"
            );
            assert_eq!(
                count("SELECT count(*) FROM side_effect_receipts"),
                2,
                "{mode}"
            );
            assert_eq!(
                count("SELECT count(*) FROM step_blob_refs WHERE role='RESULT'"),
                2,
                "{mode}"
            );
            assert_eq!(
                count("SELECT count(*) FROM task_journal WHERE journal_kind='STEP_COMMITTED'"),
                2,
                "{mode}"
            );
            assert_eq!(
                count("SELECT count(*) FROM task_journal WHERE journal_kind='RECEIPT_RECORDED'"),
                2,
                "{mode}"
            );
            assert_eq!(
                count("SELECT count(*) FROM leases WHERE released_at_ms IS NULL"),
                0,
                "{mode}"
            );
        }
        "f26" => {
            // The parent's committed content survived and the child added its own.
            assert_eq!(count("SELECT count(*) FROM tasks"), 2, "{mode}");
        }
        "n7" => {}
        other => panic!("unknown verifier mode {other}"),
    }
}

fn run_child_window(mode: &str, window: Window, stage: &str) -> Fixture {
    let fixture = Fixture::create(mode);
    setup(&fixture, stage);
    let ack = fixture.ack(window);
    let cwd = std::env::temp_dir();
    let vars = [
        (ROLE, "child"),
        (MODE, mode),
        fixture.dir_var(),
        (STAGE, stage),
    ];
    let child = spawn(CHILD, &vars, &cwd);
    if !wait_for(&ack) {
        // The child died before the window or hung before acknowledging. The
        // bounded collector kills/reaps a still-running child before failing.
        let output = wait_with_output_bounded(
            child,
            &format!("{mode}: missing {window:?} acknowledgement"),
        );
        panic!(
            "{mode}: child never acknowledged {window:?}\n{}",
            describe(&output)
        );
    }
    kill_and_prove_death(child, mode);
    assert!(
        !fixture.dir.join("success").exists(),
        "{mode}: child published application success"
    );
    // The durable expectation is asserted by the fresh verifier process.
    run_verifier(&fixture, mode, &cwd);
    fixture
}

#[test]
fn n1_child_death_before_begin_leaves_correct_schema_and_no_task() {
    run_child_window("n1", Window::BeforeBegin, "insert");
}

#[test]
fn n2_child_death_after_begin_before_any_write_leaves_no_row() {
    run_child_window("n2", Window::AfterBegin, "insert");
}

#[test]
fn n3_child_death_between_task_insert_and_journal_rolls_both_back() {
    // The task row and its audit/journal row share one transaction.
    run_child_window("n3", Window::AfterTaskInsert, "insert");
}

// ---------------------------------------------------------------------------
// N4 -- fenced step UPDATE succeeded, outcome not completed
// ---------------------------------------------------------------------------

#[test]
fn n4_child_death_after_fenced_update_before_receipt_is_conservative() {
    // The fresh verifier proves the conservative pre-outcome state: step not
    // SUCCEEDED, no receipt, no result blob or reference, no terminal outcome
    // journal, task not advanced, authority not released.
    run_child_window("n4", Window::AfterFencedStepWrite, "n6");
}

// ---------------------------------------------------------------------------
// N5 -- every write done, before COMMIT
// ---------------------------------------------------------------------------

#[test]
fn n5_child_death_before_commit_makes_no_transaction_write_durable() {
    // The fresh verifier proves every row the target transaction wrote is
    // absent, while the previously committed fixture is fully intact.
    run_child_window("n5", Window::BeforeCommit, "n6");
}

// ---------------------------------------------------------------------------
// N6 -- COMMIT returned, caller never observed Ok
// ---------------------------------------------------------------------------

#[test]
fn n6_child_death_after_commit_leaves_complete_committed_p2f_state() {
    let f = run_child_window("n6", Window::AfterCommit, "n6");
    // The fresh verifier proved the complete committed P2F state. N6a adds the
    // caller-side half: the child never published application success, because
    // it was killed inside the window before `commit_step` could return.
    assert!(
        !f.dir.join("success").exists(),
        "N6a: child reported success"
    );
}

#[test]
fn n6b_recovery_after_a_terminal_post_commit_crash_is_a_strict_no_op() {
    let f = run_child_window("n6-terminal", Window::AfterCommit, "terminal");
    // The fresh verifier already proved the terminal fixture committed
    // completely: both steps SUCCEEDED, the task COMPLETED, both receipts and
    // both lease releases durable.

    // Recovery over a consistent terminal task is a strict logical no-op: it
    // reconstructs nothing, duplicates nothing and re-effects nothing. The
    // comparison is a whole-table logical dump, not a row count.
    let c = Context::new();
    let mut e = f.open();
    let before = f.dump();
    let report = e.recover(at(60), &c.view()).unwrap();
    assert_eq!(report.tasks_examined, 1);
    assert_eq!(
        report.repairs_committed, 0,
        "terminal recovery repaired something"
    );
    assert_eq!(report.invariant_violations, 0);
    assert_eq!(report.tasks_resumed, 0);
    assert_eq!(report.decisions.len(), 1);
    assert!(
        matches!(report.decisions[0], RecoveryDecision::TerminalNoop { .. }),
        "a COMPLETED task was not a terminal no-op"
    );
    assert_eq!(f.dump(), before, "N6b: terminal recovery was not a no-op");
    // A second pass at the same explicit time is equally inert.
    let again = e.recover(at(60), &c.view()).unwrap();
    assert_eq!(again.repairs_committed, 0);
    assert_eq!(f.dump(), before);
}

#[test]
fn n6b_recovery_after_a_nonterminal_post_commit_crash_re_effects_nothing() {
    let f = run_child_window("n6-nonterminal", Window::AfterCommit, "n6");
    let c = Context::new();
    let mut e = f.open();
    let before = f.dump();
    let outcome_journal_before =
        f.count("SELECT count(*) FROM task_journal WHERE journal_kind<>'RECOVERY_DECISION'");
    let report = e.recover(at(60), &c.view()).unwrap();

    // A complete nonterminal outcome may receive first classification audit.
    // It must never be reported or repaired as an outcome reconstruction.
    assert_eq!(report.invariant_violations, 0);
    assert!(
        !report
            .decisions
            .iter()
            .any(|d| matches!(d, RecoveryDecision::CorruptOrInvariantViolation { .. })),
        "a consistent committed outcome was reported as corruption"
    );
    // No duplicated receipt and no repeated P2F outcome journal.
    assert_eq!(f.count("SELECT count(*) FROM side_effect_receipts"), 1);
    assert_eq!(
        f.count("SELECT count(*) FROM task_journal WHERE journal_kind='STEP_COMMITTED'"),
        1
    );
    assert_eq!(
        f.count("SELECT count(*) FROM task_journal WHERE journal_kind='RECEIPT_RECORDED'"),
        1
    );
    // Only decision audit may have been added, and every added row must be a
    // classification audit row: no outcome journal may be repeated.
    f.only_changed(&before, &["task_journal"]);
    assert_eq!(
        f.count("SELECT count(*) FROM task_journal WHERE journal_kind<>'RECOVERY_DECISION'"),
        outcome_journal_before,
        "recovery repeated a non-audit journal row"
    );
    // `repairs_committed` counts distinct tasks changed, not journal rows, and
    // the contract only says a first classification audit *may* be written. No
    // exact value is pinned; what must hold is that nothing was re-effected and
    // nothing was reported as an outcome reconstruction or corruption.
    assert!(
        !report.decisions.iter().any(|d| matches!(
            d,
            RecoveryDecision::CorruptOrInvariantViolation { .. }
                | RecoveryDecision::BlockedTask { .. }
                | RecoveryDecision::NeedsReconciliation { .. }
        )),
        "a consistent committed outcome was misclassified: {:?}",
        report.decisions
    );
    assert_eq!(
        report.pending_event_transitions,
        f.count("SELECT count(*) FROM task_journal") as u64
    );
}

// ---------------------------------------------------------------------------
// N7 -- COMMIT in flight. Stress only; weak assertions by contract.
//
// A deterministic mid-`COMMIT` injection is NOT available through `rusqlite`
// (test matrix §3.1). The writer below commits repeatedly and a sibling
// controller SIGKILLs it at an arbitrary point, so whether any individual
// COMMIT is actually in flight at the moment of death is timing-dependent and
// is deliberately NOT asserted. The only load-bearing properties are the
// weak ones the contract allows: the database still opens, `quick_check` is
// `ok`, `foreign_key_check` is empty, and every durable task kept exactly one
// audit row.
// ---------------------------------------------------------------------------

const N7_SAMPLES: usize = 10;
const N7_COMMITS: usize = 6;

#[test]
fn n7_commit_in_flight_stress_never_corrupts_the_database() {
    let cwd = std::env::temp_dir();
    let mut observed = Vec::new();
    for sample in 0..N7_SAMPLES {
        let fixture = Fixture::create(&format!("n7-{sample}"));
        // The parent creates the location so the child inherits a real store.
        {
            let _ = fixture.open();
        }
        let gate = fixture.dir.join("go");
        let vars = [
            (ROLE, "writer"),
            (MODE, "n7"),
            fixture.dir_var(),
            (TARGET, gate.to_str().unwrap()),
        ];
        let mut child = spawn(WRITER, &vars, &cwd);
        // The writer commits only after the gate exists, so the release is an
        // explicit file acknowledgement rather than a sleep.
        fs::write(&gate, "1").unwrap();
        let progress = fixture.dir.join("progress");
        if !wait_for_lines(&progress, N7_COMMITS) {
            let _ = child.kill();
            let _ = child.wait();
            panic!("n7 sample {sample}: writer never completed {N7_COMMITS} commits");
        }
        child.kill().unwrap();
        let status = child.wait().unwrap();

        // The sample counts only if the writer really died by signal rather
        // than exiting cleanly; otherwise it proves nothing about a crash.
        #[cfg(unix)]
        {
            use std::os::unix::process::ExitStatusExt;
            assert_eq!(
                status.signal(),
                Some(9),
                "n7 sample {sample}: writer was not SIGKILLed"
            );
        }
        assert!(
            !status.success(),
            "n7 sample {sample}: writer exited successfully"
        );

        // Allowed durable outcome per transaction: the complete transaction, or
        // none of it. The sample-level task count is recorded, never pinned.
        let committed = fixture.count("SELECT count(*) FROM tasks");
        observed.push(committed);
        // Every durable task has exactly one insertion audit row; compare per
        // task so a missing+duplicate pair cannot cancel out in a total count.
        assert_eq!(
            fixture.count(
                "SELECT count(*) FROM (SELECT tasks.task_id FROM tasks LEFT JOIN task_journal ON task_journal.task_id=tasks.task_id AND task_journal.journal_kind='TASK_INSERTED' GROUP BY tasks.task_id HAVING count(task_journal.task_id)<>1)"
            ),
            0,
            "n7 sample {sample}: a durable task has missing or duplicate insertion audit"
        );
        // Fresh process: openable, quick_check ok, foreign_key_check empty.
        run_verifier(&fixture, "n7", &cwd);
    }
    println!(
        "N7 stress: {N7_SAMPLES} samples; observed durable task counts {observed:?}. \
         No exact count is pinned for any run and neither outcome category is \
         required: a deterministic mid-COMMIT injection is unavailable through \
         rusqlite, so only the weak integrity assertions above are load-bearing."
    );
}

fn wait_for_lines(path: &Path, lines: usize) -> bool {
    for _ in 0..POLLS {
        if fs::read_to_string(path)
            .map(|text| text.lines().count() >= lines)
            .unwrap_or(false)
        {
            return true;
        }
        std::thread::sleep(TICK);
    }
    false
}

// ---------------------------------------------------------------------------
// N8 -- write succeeded, inspection fails. Fault-injection rollback, NOT crash.
// ---------------------------------------------------------------------------

#[test]
fn n8_write_succeeded_then_inspection_failure_rolls_the_transaction_back() {
    let f = Fixture::create("n8");
    setup(&f, "n6");
    let c = Context::new();
    let mut e = f.open();

    // Acquire and begin commit first, so the baseline already contains a live
    // EXECUTING step. Only the fenced outcome transaction is faulted.
    let guard = run_outcome_prefix(&mut e, &c);
    let step = step_of(&e, 0);
    let before = f.dump();

    // A deterministic test-only error between the fenced write and its
    // rows_affected inspection. Labelled: this is a rollback test, not a crash.
    Window::BeforeFenceInspection
        .arm(Action::Fail(StoreError::Sqlite))
        .unwrap();
    assert!(Window::BeforeFenceInspection.is_armed());
    let outcome = e.commit_step(
        guard,
        StepOutcome::Succeeded {
            result_json: RESULT,
            receipt: Some(&receipt(&step)),
        },
        at(42),
        &c.view(),
    );
    assert!(matches!(
        outcome,
        Err(EngineError::Store(StoreError::Sqlite))
    ));
    assert!(
        !Window::BeforeFenceInspection.is_armed(),
        "the window must disarm itself when it fires"
    );

    // Tx rolled back: no receipt and no partial state anywhere.
    assert_eq!(f.count("SELECT count(*) FROM side_effect_receipts"), 0);
    assert_eq!(
        f.count("SELECT count(*) FROM step_blob_refs WHERE role='RESULT'"),
        0
    );
    assert_eq!(
        f.count("SELECT count(*) FROM task_steps WHERE status<>'EXECUTING'"),
        0
    );
    assert_eq!(
        f.count("SELECT count(*) FROM tasks WHERE state<>'EXECUTING'"),
        0
    );
    assert_eq!(
        f.count("SELECT count(*) FROM leases WHERE released_at_ms IS NOT NULL"),
        0
    );
    assert_eq!(
        f.count("SELECT count(*) FROM task_journal WHERE journal_kind IN ('STEP_COMMITTED','RECEIPT_RECORDED')"),
        0
    );
    assert_eq!(f.dump(), before, "N8 left partial durable state");
    // The connection survived; that is exactly why this is not a crash.
    assert!(e.load(tid(1)).is_ok());
}

/// Acquires and begins an attempt, leaving the guard for the caller's commit.
fn run_outcome_prefix(e: &mut TaskEngine, c: &Context) -> LeaseGuard {
    let guard = e
        .acquire(tid(1), sid(1), owner(), None, at(40), at(500), &c.view())
        .unwrap();
    e.begin_attempt(&guard, at(41), &c.view()).unwrap();
    guard
}

// ---------------------------------------------------------------------------
// Failure seams around audit persistence and savepoint release/cleanup
// ---------------------------------------------------------------------------

#[test]
fn an_audit_persistence_fault_rolls_back_the_whole_operation() {
    let f = Fixture::create("audit-seam");
    let c = Context::new();
    let mut e = f.open();
    let before = f.dump();
    Window::BeforeJournalInsert
        .arm(Action::Fail(StoreError::AuditRejected))
        .unwrap();
    let result = e.create_task(spec(1, 3), &c.view());
    assert!(matches!(
        result,
        Err(EngineError::Store(StoreError::AuditRejected))
    ));
    assert_eq!(f.count("SELECT count(*) FROM tasks"), 0);
    assert_eq!(f.count("SELECT count(*) FROM task_journal"), 0);
    f.only_changed(&before, &[]);
}

#[test]
fn a_savepoint_release_fault_makes_the_outer_transaction_rollback_only() {
    let f = Fixture::create("savepoint-seam");
    let c = Context::new();
    let store = Store::open(&f.path, &Fixed).unwrap();
    let before = f.dump();
    Window::BeforeSavepointRelease
        .arm(Action::Fail(StoreError::Sqlite))
        .unwrap();
    // The caller *catches* the operation error and asks to commit anyway.
    let task = assistant_task(1);
    let caught: Result<(), EngineError> = store
        .transact_with_participants(&TaskJournal, &event_bus(), |tx| {
            let _ = tx.insert_task(&task, &c.view());
            Ok(())
        })
        .map_err(EngineError::from);
    assert!(matches!(
        caught,
        Err(EngineError::Store(StoreError::Sqlite))
    ));
    assert_eq!(f.count("SELECT count(*) FROM tasks"), 0);
    assert_eq!(f.count("SELECT count(*) FROM task_journal"), 0);
    f.only_changed(&before, &[]);
}

fn assistant_task(n: u32) -> AssistantTask {
    let spec = spec(n, 3);
    AssistantTask {
        task_id: spec.task_id,
        kind: spec.kind,
        title: spec.title,
        state: TaskState::Received,
        origin: spec.origin,
        data_class: spec.data_class,
        policy_class: spec.policy_class,
        created_at: Timestamp::from_epoch_millis(spec.created_at),
        updated_at: Timestamp::from_epoch_millis(spec.created_at),
        deadline_at: spec.deadline_at.map(Timestamp::from_epoch_millis),
        attempt_budget: spec.attempt_budget,
        steps: Vec::new(),
        blocked_reason: None,
        result_summary: None,
        cancelled_at: None,
        cancelled_by: None,
        failure_reason: None,
        extensions: spec.extensions,
    }
}

// ---------------------------------------------------------------------------
// F25 -- two integration-test binaries get distinct temp paths
// ---------------------------------------------------------------------------

#[test]
fn f25_two_integration_test_binaries_get_distinct_temp_paths() {
    // The exact historical collision: identical pid and counter, two binaries.
    // A counter-only (or pid-only) derivation returns the same name for both.
    let a = derive_dir("f25", "serea_task_engine_crash", 4242, 0);
    let b = derive_dir("f25", "serea_storage_foundation", 4242, 0);
    assert_ne!(a, b, "binary identity must keep two binaries apart");
    assert_ne!(
        derive_dir("f25", "x", 4242, 0),
        derive_dir("f25", "x", 4242, 1)
    );
    assert_ne!(
        derive_dir("f25", "x", 4242, 0),
        derive_dir("f25", "x", 4243, 0)
    );

    // This process really is the binary whose identity it uses.
    let own = Fixture::create("f25-own");
    let name = own.dir.file_name().unwrap().to_string_lossy().into_owned();
    assert!(
        name.contains(&binary_identity()),
        "fixture path {name} does not embed binary identity {}",
        binary_identity()
    );

    // Two genuinely distinct executables in two distinct processes. The second
    // is a byte-for-byte copy of this test binary under another file name, so
    // its `current_exe()` really differs.
    let copy = own.dir.join("serea_p2h_second_integration_binary");
    fs::copy(std::env::current_exe().unwrap(), &copy).unwrap();
    let report = own.dir.join("probe.txt");
    let first = spawn(
        PROBE,
        &[(ROLE, "probe"), (TARGET, report.to_str().unwrap())],
        &std::env::temp_dir(),
    );
    let first = wait_with_output_bounded(first, "F25 first binary identity probe");
    assert!(first.status.success(), "{}", describe(&first));
    let second = Command::new(&copy)
        .args(["--exact", PROBE, "--nocapture", "--test-threads=1"])
        .env(ROLE, "probe")
        .env(TARGET, report.to_str().unwrap())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let second = wait_with_output_bounded(second, "F25 second binary identity probe");
    assert!(second.status.success(), "{}", describe(&second));

    let text = fs::read_to_string(&report).unwrap();
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(lines.len(), 2, "probe report: {text}");
    let (exe_a, dir_a) = lines[0].split_once('|').unwrap();
    let (exe_b, dir_b) = lines[1].split_once('|').unwrap();
    assert_ne!(exe_a, exe_b, "the two executables must really differ");
    // The binary-identity COMPONENT is proven by the pure-function assertions
    // above, which hold the pid and counter equal. The live probe proves only
    // that two real, distinct executables really run here and each derive its
    // own live directory; their pids also differ, so this alone cannot isolate
    // the identity component. The historical collision the matrix describes is
    // pid reuse across sequential runs, which two concurrent processes cannot
    // reproduce by construction.
    assert_ne!(
        dir_a, dir_b,
        "two binaries must not derive the same temp path"
    );
    assert!(Path::new(dir_a).is_dir() && Path::new(dir_b).is_dir());
    // Clean up the two probe fixtures the child processes deliberately left.
    for dir in [dir_a, dir_b] {
        let _ = fs::remove_dir_all(Path::new(dir));
    }
}

#[test]
fn f27_temp_paths_contain_no_wall_clock_and_no_rng() {
    // Deterministic: identical inputs always yield the identical name.
    for _ in 0..3 {
        assert_eq!(
            derive_dir("f27", "ident", 7, 3),
            std::env::temp_dir().join("serea-p2h-f27-ident-7-3")
        );
    }
    assert_eq!(
        derive_dir("f27", "ident", 7, 3)
            .file_name()
            .unwrap()
            .to_string_lossy(),
        "serea-p2h-f27-ident-7-3"
    );
}

// ---------------------------------------------------------------------------
// F26 -- crash child reopens the parent's database by inherited directory
// ---------------------------------------------------------------------------

#[test]
fn f26_a_child_reopens_the_parent_database_by_the_exact_inherited_directory() {
    let c = Context::new();
    // The same identity components every other fixture uses, plus spaces and
    // parentheses, so this location is as collision-proof as the rest and still
    // exercises an ordinary awkward path.
    let identity = binary_identity();
    let pid = std::process::id();
    let temp_root = std::env::temp_dir()
        .canonicalize()
        .expect("temporary directory must be resolvable");
    let dir = loop {
        let candidate = derive_dir(
            "f26 inherited (fixture)",
            &identity,
            pid,
            NEXT.fetch_add(1, Ordering::Relaxed),
        );
        let candidate = temp_root.join(candidate.file_name().unwrap());
        match fs::create_dir(&candidate) {
            Ok(()) => break candidate,
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => panic!("cannot create inherited fixture directory: {error}"),
        }
    };
    assert!(
        dir.to_string_lossy().contains(' '),
        "F26 must exercise a spaced path"
    );

    // The parent creates the fixture location and commits durable content.
    let fixture = Fixture::parent_owned(&dir);
    {
        let mut e = fixture.open();
        e.create_task(spec(1, 3), &c.view()).unwrap();
    }
    assert_eq!(fixture.count("SELECT count(*) FROM tasks"), 1);

    // Use a unique existing directory beneath the fixture rather than the
    // workspace root: the parent test runner may itself start there.
    let other_cwd = fixture.dir.join("child working directory");
    fs::create_dir(&other_cwd).unwrap();
    assert_ne!(other_cwd, std::env::current_dir().unwrap());
    let vars = [
        (ROLE, "child"),
        (MODE, "f26"),
        fixture.dir_var(),
        (STAGE, "f26"),
    ];
    let output = wait_with_output_bounded(spawn(CHILD, &vars, &other_cwd), "F26 child reopen");
    assert!(
        output.status.success(),
        "F26 child failed\n{}",
        describe(&output)
    );

    // The child wrote the exact path it opened: no new temp store was minted.
    let opened = fs::read_to_string(dir.join("opened.txt")).unwrap();
    assert_eq!(
        Path::new(opened.trim()),
        fixture.path,
        "child opened another database"
    );
    // The durable expectation is proved by the fresh verifier process.
    run_verifier(&fixture, "f26", &other_cwd);
    let sqlite_files = fs::read_dir(&dir)
        .unwrap()
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.file_name().to_string_lossy().ends_with(".sqlite"))
        .count();
    assert_eq!(sqlite_files, 1, "an extra temporary database was created");
    let _ = fs::remove_dir_all(&dir);
}

impl Fixture {
    /// Like [`Fixture::child_inherited`] but for a location whose database the
    /// parent has not created yet. The parent still owns the directory.
    fn parent_owned(dir: &Path) -> Self {
        let fixture = Self {
            dir: dir.to_path_buf(),
            path: dir.join("store.sqlite"),
            owns_dir: true,
        };
        support::seed_active_registry_generation(&Store::open(&fixture.path, &Fixed).unwrap());
        fixture
    }
}

// ---------------------------------------------------------------------------
// Crash-harness safety and production isolation guards
// ---------------------------------------------------------------------------

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf()
}

/// Every `.rs` file under `crates/`, excluding integration-test directories and
/// `*_tests.rs` unit-test modules.
fn runtime_sources() -> Vec<(String, String)> {
    let root = workspace_root();
    let mut files = Vec::new();
    let mut stack = vec![root.join("crates")];
    while let Some(dir) = stack.pop() {
        for entry in fs::read_dir(&dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().map(|e| e == "rs").unwrap_or(false) {
                let name = path
                    .strip_prefix(&root)
                    .unwrap()
                    .to_string_lossy()
                    .into_owned();
                if name.contains("/tests/") || name.ends_with("_tests.rs") {
                    continue;
                }
                files.push((name, fs::read_to_string(&path).unwrap()));
            }
        }
    }
    files.sort();
    files
}

/// Returns the source of a runtime file with any `#[cfg(test)]` module removed.
///
/// Production reachability is what matters: a `#[cfg(test)]` block cannot exist
/// in a release build, so a temporary directory used only by an inline unit-test
/// module is not a production filesystem path.
fn strip_cfg_test(text: &str) -> String {
    let mut out = String::new();
    let mut rest = text;
    while let Some(start) = rest.find("#[cfg(test)]") {
        out.push_str(&rest[..start]);
        rest = &rest[start + "#[cfg(test)]".len()..];
        // Consume the following `mod ... { ... }` block by brace counting.
        let mut depth = 0usize;
        let mut end = rest.len();
        for (index, byte) in rest.as_bytes().iter().enumerate() {
            match byte {
                b'{' => depth += 1,
                b'}' => {
                    depth -= 1;
                    if depth == 0 {
                        end = index + 1;
                        break;
                    }
                }
                _ => {}
            }
        }
        rest = &rest[end..];
    }
    out.push_str(rest);
    out
}

#[test]
fn production_runtime_crates_spawn_no_subprocess_and_read_no_environment() {
    for (name, text) in runtime_sources() {
        // The fault seam is a test-only compiled capability, `#[cfg]`-gated out
        // of production entirely (pinned by the next test). Its file writes
        // name a test-harness acknowledgement path, never a store path.
        let scan = strip_cfg_test(&text);
        for banned in [
            "std::process::Command",
            "std::env::var",
            "std::env::args",
            "std::env::temp_dir",
        ] {
            assert!(
                !scan.contains(banned),
                "{name} contains {banned}; runtime crates have no subprocess or environment path"
            );
        }
    }
}

#[test]
fn the_fault_seam_is_compiled_out_of_every_production_build() {
    let root = workspace_root();
    let lib = fs::read_to_string(root.join("crates/serea-storage/src/lib.rs")).unwrap();
    assert!(
        lib.contains("#[cfg(feature = \"p2h-fault-injection\")]\npub mod fault;"),
        "the fault seam must be declared only under the test-only feature"
    );
    let storage = fs::read_to_string(root.join("crates/serea-storage/Cargo.toml")).unwrap();
    assert!(storage.contains("p2h-fault-injection = []"));
    assert!(
        !storage.contains("default = [\"p2h"),
        "storage must not enable any fault feature by default"
    );
    for crate_name in ["serea-task-engine", "serea-model-router"] {
        let manifest =
            fs::read_to_string(root.join(format!("crates/{crate_name}/Cargo.toml"))).unwrap();
        let dev = manifest.split("[dev-dependencies]").nth(1).unwrap();
        assert!(dev.contains("p2h-fault-injection"));
        let runtime = manifest.split("[dev-dependencies]").next().unwrap();
        assert!(
            !runtime.contains("p2h-fault-injection"),
            "the fault feature must be requested only from a dev-dependency edge"
        );
    }
    // The storage manifest has no path dependency that could smuggle it in.
    assert!(!storage.contains("[dependencies]\nserea-testkit"));
}

// ---------------------------------------------------------------------------
// Child / verifier process entry points
// ---------------------------------------------------------------------------

/// Re-invoked test binary. A no-op unless the parent spawned it in child mode,
/// which is what keeps the harness out of normal product execution.
#[test]
fn p2h_child_entry() {
    let Ok(role) = std::env::var(ROLE) else {
        return;
    };
    assert_eq!(role, "child");
    let mode = std::env::var(MODE).unwrap();
    let stage = std::env::var(STAGE).unwrap();
    let fixture = Fixture::child_inherited(Path::new(&std::env::var(DIR).unwrap()));

    if mode == "f26" {
        fs::write(
            fixture.dir.join("opened.txt"),
            fixture.path.to_str().unwrap(),
        )
        .unwrap();
        // Read the parent's durable content back through the production API,
        // proving the child really reopened the parent's database.
        let e = fixture.open();
        assert_eq!(e.load(tid(1)).unwrap().task.task_id, tid(1));
        let c = Context::new();
        let mut e = fixture.open();
        e.create_task(spec(2, 3), &c.view()).unwrap();
        return;
    }

    let window = match mode.as_str() {
        "n1" => Window::BeforeBegin,
        "n2" => Window::AfterBegin,
        "n3" => Window::AfterTaskInsert,
        "n4" => Window::AfterFencedStepWrite,
        "n5" => Window::BeforeCommit,
        "n6" | "n6-terminal" | "n6-nonterminal" => Window::AfterCommit,
        other => panic!("unknown child mode {other}"),
    };
    // The transaction envelope windows (`BeforeBegin`, `AfterBegin`,
    // `BeforeCommit`, `AfterCommit`) are shared by *every* transaction, so the
    // arming must skip the setup transactions to reach the target one. The
    // count is derived from what the child itself does:
    //
    //     insert  : `create_task` is the child's first and only transaction -> 0.
    //     n6      : acquire, begin, commit -> the outcome is the third -> 2.
    //     terminal: the parent already committed the ordinary step, so the
    //               child runs acquire, begin, commit -> also 2.
    let envelope = matches!(
        window,
        Window::BeforeBegin | Window::AfterBegin | Window::BeforeCommit | Window::AfterCommit
    );
    let skip = if envelope && stage != "insert" { 2 } else { 0 };
    let ack = fixture.ack(window);
    window.arm_after(skip, Action::Crash { ack }).unwrap();
    assert!(window.is_armed());

    let c = Context::new();
    let mut e = fixture.open();
    if stage == "insert" {
        e.create_task(spec(1, 3), &c.view()).unwrap();
    } else if stage == "n6" {
        run_outcome(&mut e, &c, 1, 41);
    } else {
        // terminal: the ordinary step is already committed; finish the plan.
        run_outcome(&mut e, &c, 2, 45);
    }
    // Only a call that actually returned could publish this. An armed crash
    // window never reaches it, which is what N6a asserts.
    fs::write(fixture.dir.join("success"), "1").unwrap();
    unreachable!("armed window {window:?} did not stop the child");
}

/// Re-invoked test binary: a genuinely fresh process with a fresh `Store`.
#[test]
fn p2h_fresh_verifier_entry() {
    let Ok(role) = std::env::var(ROLE) else {
        return;
    };
    assert_eq!(role, "verify");
    let mode = std::env::var(MODE).unwrap();
    let fixture = Fixture::child_inherited(Path::new(&std::env::var(DIR).unwrap()));

    let store = Store::open(&fixture.path, &Fixed).unwrap();
    assert_eq!(store.schema_version().unwrap(), 4);
    store.verify_integrity().unwrap();
    let conn = fixture.sql();
    let quick: String = conn
        .query_row("PRAGMA quick_check", [], |r| r.get(0))
        .unwrap();
    assert_eq!(quick, "ok", "{mode}: quick_check is not ok");
    let orphans: i64 = conn
        .query_row("SELECT count(*) FROM pragma_foreign_key_check", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(orphans, 0, "{mode}: foreign_key_check reported rows");

    // The mode-specific durable expectation is asserted HERE, in a process that
    // never touched the transaction, never armed a window and never ran the
    // parent's crash coordination.
    verify_durable(&mode, &fixture);
}

/// N7 stress writer: commits repeatedly until the controller kills it.
#[test]
fn p2h_stress_writer_entry() {
    let Ok(role) = std::env::var(ROLE) else {
        return;
    };
    assert_eq!(role, "writer");
    let fixture = Fixture::child_inherited(Path::new(&std::env::var(DIR).unwrap()));
    let gate = PathBuf::from(std::env::var(TARGET).unwrap());
    assert!(wait_for(&gate), "controller never released the writer");
    let c = Context::new();
    let mut e = fixture.open();
    let progress = fixture.dir.join("progress");
    for n in 1..100_000u32 {
        // Only a committed transaction is reported, so the controller's SIGKILL
        // lands while a later COMMIT may still be in flight.
        if e.create_task(spec(n, 3), &c.view()).is_err() {
            break;
        }
        use std::io::Write;
        writeln!(
            fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&progress)
                .unwrap(),
            "c"
        )
        .unwrap();
    }
}

/// F25 probe: reports this executable's identity and the fixture it created.
#[test]
fn p2h_tempstore_identity_probe() {
    if std::env::var(ROLE).ok().as_deref() != Some("probe") {
        return;
    }
    let fixture = Fixture::create("f25probe");
    let exe = std::env::current_exe().unwrap();
    let report = std::env::var(TARGET).unwrap();
    let mut text = fs::read_to_string(&report).unwrap_or_default();
    text.push_str(&format!(
        "{}|{}\n",
        exe.file_name().unwrap().to_string_lossy(),
        fixture.dir.display()
    ));
    fs::write(&report, text).unwrap();
    // The parent checks that both directories exist, so this process must leave
    // its own fixture in place rather than cleaning up on exit.
    std::mem::forget(fixture);
}
