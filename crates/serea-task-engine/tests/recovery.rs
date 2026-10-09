//! Test-first P2G contracts, reconciled against docs/plans/P2G-review-and-closure.md.
//! Only fixtures use SQL. Production recovery must remain SQL-free and effect-free.
//! Fixtures reuse the workspace's existing SQLite dependency; it is dev-only here.
//! RecoveryReport/RecoveryDecision pin the selected public classification shapes.
//! The frozen report calls the eligibility counter `tasks_resumed`, not `resumed`.
use rusqlite::{Connection, types::ValueRef};
use serea_protocol::*;
use serea_storage::{Store, StoreError};
use serea_task_engine::*;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Barrier;
use std::sync::atomic::{AtomicU64, Ordering};
mod support;
use support::event_bus;

fn at(n: i64) -> EpochMillis {
    EpochMillis::new(n).unwrap()
}
struct Fixed;
impl Clock for Fixed {
    fn now_ms(&self) -> Result<EpochMillis, ProtocolError> {
        Ok(at(0))
    }
}
fn tid(n: u32) -> TaskId {
    TaskId::new(format!("tsk_{n:026}")).unwrap()
}
fn sid(n: u32) -> StepId {
    StepId::new(format!("stp_{n:026}")).unwrap()
}
fn rid() -> ReceiptId {
    ReceiptId::new("rcp_00000000000000000000000001").unwrap()
}
struct Context {
    actor: ActorId,
    version: SemVer,
    cause: EventId,
}
impl Context {
    fn new() -> Self {
        Self {
            actor: ActorId::new("recovery-host").unwrap(),
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
        title: TaskTitle::new("recovery sensitive title sentinel").unwrap(),
        origin: TaskOrigin {
            kind: TaskOriginKind::new("USER_MESSAGE").unwrap(),
            device_id: None,
            message_id: None,
            extensions: [(
                "future_origin".into(),
                serde_json::json!({"nested":[1,null]}),
            )]
            .into(),
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
        extensions: [("future_task".into(), serde_json::json!({"x":[true]}))].into(),
    }
}
fn input(task: u32, step: u32, sequence: u32, kind: StepKind) -> PlanStep {
    let raw =
        format!("{{\"value\":{step},\"instruction\":\"recovery input sentinel\"}}").into_bytes();
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
        extensions: [(
            "future_step".into(),
            serde_json::json!({"original":[7,true,null]}),
        )]
        .into(),
    })
    .unwrap();
    PlanStep {
        step,
        input_json: raw,
    }
}
fn planning(e: &mut TaskEngine, c: &Context, task: u32, ceiling: u32) {
    e.create_task(spec(task, ceiling), &c.view()).unwrap();
    e.start_planning(tid(task), TaskState::Received, 0, at(20), &c.view())
        .unwrap();
}
fn ready(e: &mut TaskEngine, c: &Context, task: u32, ceiling: u32, steps: Vec<PlanStep>) {
    planning(e, c, task, ceiling);
    e.persist_plan(tid(task), Plan { revision: 1, steps }, at(30), &c.view())
        .unwrap();
}
fn single(e: &mut TaskEngine, c: &Context, kind: StepKind, ceiling: u32) {
    ready(e, c, 1, ceiling, vec![input(1, 1, 10, kind)]);
}
fn acquire(
    e: &mut TaskEngine,
    c: &Context,
    step: u32,
    generation: Option<u32>,
    now: i64,
    expiry: i64,
) -> LeaseGuard {
    e.acquire(
        tid(1),
        sid(step),
        LeaseOwner::new("recovery-worker").unwrap(),
        generation,
        at(now),
        at(expiry),
        &c.view(),
    )
    .unwrap()
}
fn inflight(e: &mut TaskEngine, c: &Context, ceiling: u32) -> LeaseGuard {
    single(e, c, StepKind::Capability, ceiling);
    let guard = acquire(e, c, 1, None, 40, 50);
    e.begin_attempt(&guard, at(41), &c.view()).unwrap();
    guard
}
fn receipt(step: &TaskStep) -> SideEffectReceipt {
    SideEffectReceipt {
        receipt_id: rid(),
        capability_id: step.capability_id.clone().unwrap(),
        idempotency_key: step.idempotency_key.clone().unwrap(),
        provider_reference: Some(ProviderReference::new("calendar-event-42").unwrap()),
        effect_summary: EffectSummary::new("recovery effect sentinel").unwrap(),
        observed_at: Timestamp::from_epoch_millis(at(42)),
        replay_safe: false,
    }
}
fn success() -> StepOutcome<'static> {
    StepOutcome::Succeeded {
        result_json: b"{\"result\":true}",
        receipt: None,
    }
}
fn committed_receipt(e: &mut TaskEngine, c: &Context) {
    let guard = inflight(e, c, 3);
    let receipt = receipt(&e.load(tid(1)).unwrap().steps[0].step);
    let committed = e
        .commit_step(
            guard,
            StepOutcome::Succeeded {
                result_json: b"{\"event_reference\":\"calendar-event-42\"}",
                receipt: Some(&receipt),
            },
            at(42),
            &c.view(),
        )
        .unwrap();
    assert_eq!(committed.task_state, TaskState::Verifying);
}

static NEXT: AtomicU64 = AtomicU64::new(0);
struct FileFixture {
    path: PathBuf,
}
impl FileFixture {
    fn new(name: &str) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "serea-p2g-{}-{name}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&dir).unwrap();
        Self {
            path: dir.join("task.sqlite"),
        }
    }
    fn open(&self) -> TaskEngine {
        TaskEngine::new(Store::open(&self.path, &Fixed).unwrap(), event_bus())
    }
    fn sql(&self) -> Connection {
        let conn = Connection::open(&self.path).unwrap();
        conn.busy_timeout(std::time::Duration::from_millis(5000))
            .unwrap();
        conn.pragma_update(None, "foreign_keys", "ON").unwrap();
        conn
    }
    fn execute(&self, sql: &str) {
        self.sql().execute_batch(sql).unwrap();
    }
    fn count(&self, sql: &str) -> u64 {
        let count: i64 = self.sql().query_row(sql, [], |row| row.get(0)).unwrap();
        u64::try_from(count).expect("SQL count must be nonnegative")
    }
    fn dump(&self) -> Dump {
        logical_dump(&self.path)
    }
}
impl Drop for FileFixture {
    fn drop(&mut self) {
        std::fs::remove_dir_all(self.path.parent().unwrap()).unwrap();
    }
}

// All durable tables are discovered, including migration authority, generated
// columns and any future durable tables. Type tags + length framing preserve raw
// TEXT/BLOB bytes, NULL, integer values and floating-point bits without ambiguity.
// Sort complete rows, not rowids, and hold one read snapshot for the entire dump.
// WAL/checkpoint metadata and physical SQLite file bytes are deliberately excluded.
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
fn same_except(before: &Dump, after: &Dump, allowed: &[&str]) {
    assert_eq!(
        before.keys().collect::<Vec<_>>(),
        after.keys().collect::<Vec<_>>(),
        "no new marker table"
    );
    for (table, rows) in before {
        // A changed task aggregate in P3 recovery is paired with exactly the
        // event rows/sequence metadata written by the same successful task
        // transition. The dedicated P3 event assertions check its kind and
        // count; unrelated tables remain byte-for-byte frozen here.
        let event_participates = allowed.contains(&"tasks")
            && matches!(
                table.as_str(),
                "event_content" | "event_sequence_ledger" | "event_store_state"
            );
        if !allowed.contains(&table.as_str()) && !event_participates {
            assert_eq!(rows, &after[table], "unexpected writes to {table}");
        }
    }
}
fn journal_count(f: &FileFixture) -> u64 {
    f.count("SELECT count(*) FROM task_journal")
}
fn recovery_count(f: &FileFixture) -> u64 {
    f.count("SELECT count(*) FROM task_journal WHERE journal_kind='RECOVERY_DECISION'")
}
fn assert_pending(f: &FileFixture, r: &RecoveryReport) {
    assert_eq!(r.pending_event_transitions, journal_count(f));
    let conn = f.sql();
    let mut stmt = conn.prepare("SELECT journal_seq,payload_json,payload_digest,actor_kind,actor_id,actor_version,causation_id,data_class_rank FROM task_journal WHERE journal_kind='RECOVERY_DECISION' ORDER BY task_id,journal_seq").unwrap();
    let mut rows = stmt.query([]).unwrap();
    while let Some(row) = rows.next().unwrap() {
        let payload: String = row.get(1).unwrap();
        let digest: String = row.get(2).unwrap();
        assert_eq!(digest_of(&payload).unwrap().as_str(), digest);
        assert_eq!(canonicalize(&payload).unwrap(), payload.as_bytes());
        assert_eq!(row.get::<_, String>(3).unwrap(), "HOST");
        assert_eq!(row.get::<_, String>(4).unwrap(), "recovery-host");
        assert_eq!(row.get::<_, String>(5).unwrap(), "0.2.0");
        assert_eq!(
            row.get::<_, String>(6).unwrap(),
            "evt_00000000000000000000000001"
        );
        assert_eq!(row.get::<_, u32>(7).unwrap(), 1);
        for sentinel in [
            "recovery sensitive title sentinel",
            "recovery input sentinel",
            "recovery effect sentinel",
        ] {
            assert!(
                !payload.contains(sentinel),
                "prose is not decision authority"
            );
        }
    }
    assert_eq!(f.count("SELECT count(*) FROM (SELECT task_id,count(*) AS n,max(journal_seq) AS m,min(journal_seq) AS first FROM task_journal GROUP BY task_id) WHERE n<>m OR first<>1"), 0);
}
fn assert_corruption_evidence(f: &FileFixture, observed: &Digest, raw: &str) {
    let conn = f.sql();
    let mut stmt = conn.prepare("SELECT payload_json,payload_ref_digest FROM task_journal WHERE journal_kind='RECOVERY_DECISION'").unwrap();
    let mut rows = stmt.query([]).unwrap();
    let mut found = false;
    while let Some(row) = rows.next().unwrap() {
        let payload: String = row.get(0).unwrap();
        let value: serde_json::Value = serde_json::from_str(&payload).unwrap();
        assert!(value.get("raw_state").is_none());
        assert!(value.get("raw_status").is_none());
        if value["decision"] == "CorruptOrInvariantViolation" {
            assert_eq!(
                value["observed_fingerprint"].as_str(),
                Some(observed.as_str())
            );
            let identity = Digest::new(value["fingerprint"].as_str().unwrap()).unwrap();
            assert_eq!(row.get::<_, String>(1).unwrap(), identity.as_str());
            found = true;
        }
    }
    assert!(found, "missing digest-bound corruption evidence");
    // Check all journal TEXT fields, including envelopes, not just JSON keys.
    let mut stmt = conn.prepare("SELECT * FROM task_journal").unwrap();
    let columns = stmt.column_count();
    let mut rows = stmt.query([]).unwrap();
    while let Some(row) = rows.next().unwrap() {
        for column in 0..columns {
            if let ValueRef::Text(text) = row.get_ref(column).unwrap() {
                assert!(
                    !text.windows(raw.len()).any(|part| part == raw.as_bytes()),
                    "raw corrupt value leaked into journal column {column}"
                );
            }
        }
    }
}
fn recover(e: &mut TaskEngine, c: &Context, now: i64) -> RecoveryReport {
    let result: Result<RecoveryReport, EngineError> = e.recover(at(now), &c.view());
    result.unwrap_or_else(|error| match error {
        EngineError::Store(cause) => panic!("recovery refused with storage category {cause:?}"),
        other => panic!("recovery refused with engine category {other:?}"),
    })
}
fn stable(
    f: &FileFixture,
    e: &mut TaskEngine,
    c: &Context,
    now: i64,
    first: &RecoveryReport,
) -> RecoveryReport {
    assert_pending(f, first);
    let after_first = f.dump();
    let second = recover(e, c, now);
    assert_eq!(
        f.dump(),
        after_first,
        "second identical pass changed durable logical bytes"
    );
    assert_eq!(second.repairs_committed, 0);
    assert_eq!(second.tasks_examined, first.tasks_examined);
    assert_eq!(
        second.pending_event_transitions,
        first.pending_event_transitions
    );
    assert_pending(f, &second);
    second
}
fn has(r: &RecoveryReport, expected: RecoveryDecision) {
    assert!(
        r.decisions.contains(&expected),
        "missing {expected:?} in {:?}",
        r.decisions
    );
}
fn resume(task: u32, step: Option<u32>) -> RecoveryDecision {
    RecoveryDecision::ResumeNormally {
        task_id: tid(task),
        next_step_id: step.map(sid),
    }
}
fn uncertain(r: &RecoveryReport) {
    has(
        r,
        RecoveryDecision::NeedsReconciliation {
            task_id: tid(1),
            step_id: sid(1),
        },
    );
    assert_eq!(r.tasks_resumed, 0);
}
fn invariant(r: &RecoveryReport, task: u32) {
    assert!(r.decisions.iter().any(|d| matches!(d, RecoveryDecision::CorruptOrInvariantViolation { task_id, reason } if *task_id == tid(task) && !reason.as_str().is_empty())), "task {} lacks required invariant classification: {r:?}", tid(task).as_str());
    assert_eq!(r.invariant_violations, 1);
}
fn no_outcomes(f: &FileFixture, since: u64) {
    assert_eq!(f.count(&format!("SELECT count(*) FROM task_journal WHERE journal_seq>{since} AND journal_kind IN ('STEP_ATTEMPT_STARTED','STEP_COMMITTED','STEP_FAILED','RECEIPT_RECORDED','STEP_RECONCILED_ABSENT')")), 0);
    assert_eq!(f.count("SELECT count(*) FROM side_effect_receipts"), 0);
    assert_eq!(
        f.count("SELECT count(*) FROM step_blob_refs WHERE role='RESULT'"),
        0
    );
}

fn empty_execution_aggregate_is_invariant(state: TaskState) {
    let f = FileFixture::new("remediation-empty-aggregate");
    let c = Context::new();
    let mut e = f.open();
    e.create_task(spec(1, 3), &c.view()).unwrap();
    e.create_task(spec(2, 3), &c.view()).unwrap();
    // A schema-valid aggregate UPDATE is not evidence of planning or execution.
    f.execute(&format!(
        "UPDATE tasks SET state='{}' WHERE task_id='{}'",
        state.wire_name(),
        tid(1).as_str()
    ));
    assert_eq!(e.load(tid(1)).unwrap().plan_revision, 0);
    assert!(e.load(tid(1)).unwrap().steps.is_empty());
    let before = f.dump();
    let seq = journal_count(&f);
    let r = recover(&mut e, &c, 50);
    invariant(&r, 1);
    assert_eq!(r.tasks_examined, 2);
    assert_eq!(r.tasks_resumed, 1);
    assert!(
        !r.decisions.contains(&resume(1, None)),
        "empty execution aggregate cannot be resumed"
    );
    has(&r, resume(2, None));
    same_except(&before, &f.dump(), &["tasks", "task_journal"]);
    no_outcomes(&f, seq);
    stable(&f, &mut e, &c, 50, &r);
}
#[test]
fn remediation_f4_unplanned_empty_executing_is_invariant_not_resumable() {
    empty_execution_aggregate_is_invariant(TaskState::Executing);
}
#[test]
fn remediation_f4_unplanned_empty_verifying_is_invariant_not_resumable() {
    empty_execution_aggregate_is_invariant(TaskState::Verifying);
}

fn residual_blocked_reason_is_journal_only(state: TaskState) {
    let f = FileFixture::new("remediation-residual-reason");
    let c = Context::new();
    let mut e = f.open();
    if state == TaskState::Ready {
        single(&mut e, &c, StepKind::Capability, 3);
    } else {
        e.create_task(spec(1, 3), &c.view()).unwrap();
    }
    e.create_task(spec(2, 3), &c.view()).unwrap();
    // Independent task CHECK damage must remain byte-identical; it is not
    // permission to erase the supplied reason or manufacture a reassessment.
    f.execute(&format!("PRAGMA ignore_check_constraints=ON; UPDATE tasks SET blocked_reason='POLICY_DENIED' WHERE task_id='{}'; PRAGMA ignore_check_constraints=OFF", tid(1).as_str()));
    let before = f.dump();
    let seq = journal_count(&f);
    let r = recover(&mut e, &c, 50);
    invariant(&r, 1);
    assert_eq!(r.tasks_examined, 2);
    assert_eq!(r.tasks_resumed, 1);
    has(&r, resume(2, None));
    assert!(!r.decisions.iter().any(
        |d| matches!(d, RecoveryDecision::ResumeNormally { task_id, .. } if *task_id == tid(1))
    ));
    assert_eq!(f.count(&format!("SELECT count(*) FROM tasks WHERE task_id='{}' AND state='{}' AND blocked_reason='POLICY_DENIED'", tid(1).as_str(), state.wire_name())), 1);
    same_except(&before, &f.dump(), &["task_journal"]);
    no_outcomes(&f, seq);
    stable(&f, &mut e, &c, 50, &r);
}
#[test]
fn remediation_f5_ready_residual_blocked_reason_preserves_raw_task_and_continues() {
    residual_blocked_reason_is_journal_only(TaskState::Ready);
}
#[test]
fn remediation_f5_received_residual_blocked_reason_preserves_raw_task_and_continues() {
    residual_blocked_reason_is_journal_only(TaskState::Received);
}

#[test]
fn zero_attempt_planned_work_conservatively_refuses_the_atomic_pass() {
    let f = FileFixture::new("zero-attempt-planned");
    let c = Context::new();
    let mut e = f.open();
    single(&mut e, &c, StepKind::Notify, 0);
    let before = f.dump();
    assert_eq!(
        e.recover(at(50), &c.view()).err(),
        Some(EngineError::Store(StoreError::InvalidRecoveryAction))
    );
    assert_eq!(
        before,
        f.dump(),
        "unsupported disabled work must not be guessed or partially repaired"
    );
}

#[test]
fn zero_attempt_recovery_refusal_rolls_back_earlier_task_repairs_in_the_pass() {
    let f = FileFixture::new("zero-attempt-mixed-atomic");
    let c = Context::new();
    let mut e = f.open();
    committed_receipt(&mut e, &c);
    ready(&mut e, &c, 2, 0, vec![input(2, 2, 10, StepKind::Notify)]);
    f.execute("UPDATE tasks SET state='EXECUTING' WHERE task_id='tsk_00000000000000000000000001'");
    let before = f.dump();
    assert_eq!(
        e.recover(at(50), &c.view()).err(),
        Some(EngineError::Store(StoreError::InvalidRecoveryAction))
    );
    assert_eq!(
        before,
        f.dump(),
        "the unsupported task must roll back earlier candidate repairs"
    );
}

#[test]
fn m0_empty_recovery_has_public_typed_zero_report() {
    let f = FileFixture::new("empty");
    let c = Context::new();
    let mut e = f.open();
    let before = f.dump();
    let r = recover(&mut e, &c, 50);
    let _: &Vec<RecoveryDecision> = &r.decisions;
    let counters: [u64; 5] = [
        r.tasks_examined,
        r.tasks_resumed,
        r.repairs_committed,
        r.invariant_violations,
        r.pending_event_transitions,
    ];
    assert_eq!(counters, [0; 5]);
    assert!(r.decisions.is_empty());
    assert_eq!(before, f.dump());
    stable(&f, &mut e, &c, 50, &r);
}

#[test]
fn m1_terminal_completed_failed_and_cancelled_inflight_are_strict_noops() {
    for state in [
        TaskState::Completed,
        TaskState::Failed,
        TaskState::Cancelled,
    ] {
        let f = FileFixture::new(state.wire_name());
        let c = Context::new();
        let mut e = f.open();
        match state {
            TaskState::Completed => {
                ready(
                    &mut e,
                    &c,
                    1,
                    3,
                    vec![
                        input(1, 1, 10, StepKind::Notify),
                        input(1, 2, 20, StepKind::Verify),
                    ],
                );
                for (step, now) in [(1, 40), (2, 43)] {
                    let g = acquire(&mut e, &c, step, None, now, 60);
                    e.begin_attempt(&g, at(now + 1), &c.view()).unwrap();
                    e.commit_step(g, success(), at(now + 2), &c.view()).unwrap();
                }
            }
            TaskState::Failed => {
                planning(&mut e, &c, 1, 3);
                e.fail_invariant(tid(1), TaskState::Planning, at(40), &c.view())
                    .unwrap();
            }
            TaskState::Cancelled => {
                let g = inflight(&mut e, &c, 3);
                e.cancel(
                    tid(1),
                    TaskOriginKind::new("USER_MESSAGE").unwrap(),
                    at(42),
                    &c.view(),
                )
                .unwrap();
                drop(g); // Terminal precedence preserves even an expired in-flight lease.
            }
            _ => unreachable!(),
        }
        drop(e);
        let mut e = f.open();
        let before = f.dump();
        let r = recover(&mut e, &c, 100);
        assert_eq!(
            r.decisions,
            vec![RecoveryDecision::TerminalNoop {
                task_id: tid(1),
                state
            }]
        );
        assert_eq!(
            [
                r.tasks_examined,
                r.tasks_resumed,
                r.repairs_committed,
                r.invariant_violations
            ],
            [1, 0, 0, 0]
        );
        assert_eq!(before, f.dump());
        stable(&f, &mut e, &c, 100, &r);
    }
}

#[test]
fn m2_attributable_missing_input_provenance_is_not_silently_resumed() {
    let f = FileFixture::new("missing-input-ref");
    let c = Context::new();
    let mut e = f.open();
    single(&mut e, &c, StepKind::Capability, 3);
    f.execute("DELETE FROM step_blob_refs WHERE role='ARGUMENTS'");
    let before = f.dump();
    let r = recover(&mut e, &c, 50);
    invariant(&r, 1);
    assert_eq!(r.tasks_resumed, 0);
    same_except(&before, &f.dump(), &["tasks", "task_journal"]);
    no_outcomes(&f, journal_count(&f));
    stable(&f, &mut e, &c, 50, &r);
}

#[test]
fn m3_unknown_task_state_is_compatibility_quarantined_without_enum_coercion() {
    let f = FileFixture::new("unknown-task-state");
    let c = Context::new();
    let mut e = f.open();
    planning(&mut e, &c, 1, 3);
    let inspection = Store::open(&f.path, &Fixed).unwrap();
    // Keep the engine open: normal quick_check/typed loading is intentionally fail closed.
    f.execute("PRAGMA ignore_check_constraints=ON; UPDATE tasks SET state='FUTURE_STATE'; PRAGMA ignore_check_constraints=OFF");
    assert!(e.load(tid(1)).is_err());
    let observed = inspection
        .transact(|tx| {
            let snapshot = tx.inspect_recovery_task(&tid(1))?;
            assert_eq!(snapshot.raw_state(), "FUTURE_STATE");
            assert_eq!(
                snapshot.state(),
                None,
                "unknown state must not become a guessed enum"
            );
            Ok(snapshot.fingerprint().clone())
        })
        .unwrap();
    let before = f.dump();
    let r = recover(&mut e, &c, 50);
    has(
        &r,
        RecoveryDecision::CorruptOrInvariantViolation {
            task_id: tid(1),
            reason: ReasonCode::new("UNRECOGNISED_STATE").unwrap(),
        },
    );
    has(&r, RecoveryDecision::BlockedTask { task_id: tid(1) });
    invariant(&r, 1);
    assert_eq!(f.count("SELECT count(*) FROM tasks WHERE state='BLOCKED' AND blocked_reason='UNRECOGNISED_STATE'"), 1);
    assert_corruption_evidence(&f, &observed, "FUTURE_STATE");
    assert_eq!(f.count("SELECT count(*) FROM task_journal WHERE journal_kind='RECOVERY_DECISION' AND state_from='FUTURE_STATE'"), 0);
    same_except(&before, &f.dump(), &["tasks", "task_journal"]);
    stable(&f, &mut e, &c, 50, &r);
}

#[test]
fn m3b_any_foreign_key_failure_refuses_the_entire_pass_before_audit() {
    let f = FileFixture::new("fk-gate");
    let c = Context::new();
    let mut e = f.open();
    e.create_task(spec(1, 3), &c.view()).unwrap();
    planning(&mut e, &c, 2, 3);
    let conn = f.sql();
    conn.pragma_update(None, "foreign_keys", "OFF").unwrap();
    conn.execute(
        "INSERT INTO task_blob_refs(task_id,role,digest,data_class_rank) VALUES (?1,'PLAN',?2,1)",
        [tid(2).as_str(), digest_of("{}").unwrap().as_str()],
    )
    .unwrap();
    conn.pragma_update(None, "foreign_keys", "ON").unwrap();
    let before = f.dump();
    for _ in 0..2 {
        assert_eq!(
            e.recover(at(50), &c.view()).err(),
            Some(EngineError::Store(StoreError::IntegrityCheckFailed))
        );
        assert_eq!(before, f.dump());
    }
    assert_eq!(recovery_count(&f), 0);
}

#[test]
fn m3b_catalog_checksum_is_rechecked_on_an_already_open_engine() {
    let f = FileFixture::new("catalog-gate");
    let c = Context::new();
    let mut e = f.open();
    e.create_task(spec(1, 3), &c.view()).unwrap();
    f.execute("UPDATE schema_migrations SET checksum='sha256:0000000000000000000000000000000000000000000000000000000000000000'");
    let before = f.dump();
    for _ in 0..2 {
        assert_eq!(
            e.recover(at(50), &c.view()).err(),
            Some(EngineError::Store(StoreError::MigrationChecksumMismatch))
        );
        assert_eq!(before, f.dump());
    }
}

#[test]
fn m4_unexpired_leased_and_executing_authority_is_held_not_executed() {
    for begun in [false, true] {
        let f = FileFixture::new("held");
        let c = Context::new();
        let mut e = f.open();
        single(&mut e, &c, StepKind::Capability, 1);
        let g = acquire(&mut e, &c, 1, None, 40, 100);
        if begun {
            e.begin_attempt(&g, at(41), &c.view()).unwrap();
        }
        let before = f.dump();
        let seq = journal_count(&f);
        let r = recover(&mut e, &c, 99);
        has(
            &r,
            RecoveryDecision::HeldLease {
                task_id: tid(1),
                step_id: sid(1),
            },
        );
        assert_eq!(r.tasks_resumed, 0);
        same_except(&before, &f.dump(), &["task_journal"]);
        no_outcomes(&f, seq);
        stable(&f, &mut e, &c, 99, &r);
        // A held classification did not revoke the known worker's authority.
        if !begun {
            e.begin_attempt(&g, at(99), &c.view()).unwrap();
        }
        e.commit_step(g, success(), at(100), &c.view()).unwrap();
    }
}

#[test]
fn m4_exact_expiry_revokes_leased_authority_then_resumes_without_begin() {
    let f = FileFixture::new("expiry-equality");
    let c = Context::new();
    let mut e = f.open();
    single(&mut e, &c, StepKind::Capability, 3);
    let g = acquire(&mut e, &c, 1, None, 40, 50);
    let expected = e.load(tid(1)).unwrap();
    let before = f.dump();
    let seq = journal_count(&f);
    let r = recover(&mut e, &c, 50);
    has(
        &r,
        RecoveryDecision::ExpiredLease {
            task_id: tid(1),
            step_id: sid(1),
        },
    );
    has(&r, resume(1, Some(1)));
    assert_eq!(
        [
            r.tasks_examined,
            r.tasks_resumed,
            r.repairs_committed,
            r.invariant_violations
        ],
        [1, 1, 1, 0]
    );
    assert_eq!(
        f.count("SELECT count(*) FROM leases WHERE generation=1 AND released_at_ms=50"),
        1
    );
    assert_eq!(e.load(tid(1)).unwrap().task, expected.task);
    same_except(&before, &f.dump(), &["leases", "task_journal"]);
    no_outcomes(&f, seq);
    let second = stable(&f, &mut e, &c, 50, &r);
    has(&second, resume(1, Some(1)));
    assert!(
        !second
            .decisions
            .iter()
            .any(|d| matches!(d, RecoveryDecision::ExpiredLease { .. }))
    );
    assert_eq!(
        e.begin_attempt(&g, at(50), &c.view()).err(),
        Some(EngineError::Store(StoreError::LeaseFenced))
    );
}

#[test]
fn m5_m15_m16_complete_receipt_audit_repairs_only_proven_stale_aggregate() {
    let f = FileFixture::new("stale-receipt-aggregate");
    let c = Context::new();
    let mut e = f.open();
    committed_receipt(&mut e, &c);
    assert_eq!(f.count("SELECT count(*) FROM task_journal WHERE journal_kind IN ('STEP_COMMITTED','RECEIPT_RECORDED','STEP_LEASE_RELEASED')"), 2);
    assert_eq!(f.count("SELECT count(*) FROM task_journal WHERE journal_kind='TASK_STATE_CHANGED' AND state_from='EXECUTING' AND state_to='VERIFYING'"), 1);
    assert_eq!(
        f.count("SELECT count(*) FROM leases WHERE released_at_ms=42"),
        1
    );
    let expected = e.load(tid(1)).unwrap();
    // P2F cannot partially commit. This is controlled aggregate corruption AFTER
    // its complete real outcome batch, not the impossible EXECUTING+receipt window.
    f.execute("UPDATE tasks SET state='EXECUTING' WHERE state='VERIFYING'");
    let before = f.dump();
    let seq = journal_count(&f);
    let r = recover(&mut e, &c, 50);
    has(
        &r,
        RecoveryDecision::ReceiptAlreadyCommitted {
            task_id: tid(1),
            step_id: sid(1),
            receipt_id: rid(),
        },
    );
    assert_eq!(r.repairs_committed, 1);
    assert_eq!(r.invariant_violations, 0);
    let actual = e.load(tid(1)).unwrap();
    assert_eq!(actual.task.state, TaskState::Verifying);
    assert!(
        actual.steps == expected.steps,
        "step runtime/provenance changed"
    );
    assert_eq!(actual.plan_revision, expected.plan_revision);
    same_except(&before, &f.dump(), &["tasks", "task_journal"]);
    assert_eq!(f.count("SELECT count(*) FROM side_effect_receipts"), 1);
    assert_eq!(f.count(&format!("SELECT count(*) FROM task_journal WHERE journal_seq>{seq} AND journal_kind IN ('STEP_COMMITTED','RECEIPT_RECORDED','STEP_ATTEMPT_STARTED','STEP_FAILED')")), 0);
    stable(&f, &mut e, &c, 50, &r);
}

#[test]
fn m5_missing_outcome_audit_is_quarantined_not_reconstructed() {
    let f = FileFixture::new("receipt-missing-audit");
    let c = Context::new();
    let mut e = f.open();
    committed_receipt(&mut e, &c);
    // Preserve a contiguous journal but remove the corroborating receipt kind.
    f.execute("UPDATE task_journal SET journal_kind='RECOVERY_DECISION' WHERE journal_kind='RECEIPT_RECORDED'; UPDATE tasks SET state='EXECUTING' WHERE state='VERIFYING'");
    let before = f.dump();
    let r = recover(&mut e, &c, 50);
    invariant(&r, 1);
    assert_eq!(r.tasks_resumed, 0);
    assert_eq!(
        f.count("SELECT count(*) FROM tasks WHERE state='VERIFYING'"),
        0
    );
    same_except(&before, &f.dump(), &["tasks", "task_journal"]);
    assert_eq!(
        f.count("SELECT count(*) FROM task_journal WHERE journal_kind='RECEIPT_RECORDED'"),
        0
    );
    // The intentionally relabelled old row is not a recovery envelope; do not
    // demand recovery attribution from it, but do demand full second-pass identity.
    let after = f.dump();
    let second = recover(&mut e, &c, 50);
    assert_eq!(second.repairs_committed, 0);
    assert_eq!(after, f.dump());
}

#[test]
fn m5_receipt_insert_for_nonsucceeded_step_is_refused_with_all_triggers_enabled() {
    let f = FileFixture::new("receipt-trigger");
    let c = Context::new();
    let mut e = f.open();
    let g = inflight(&mut e, &c, 3);
    let step = e.load(tid(1)).unwrap().steps.remove(0).step;
    let before = f.dump();
    let conn = f.sql();
    let error = conn.execute("INSERT INTO side_effect_receipts(receipt_id,task_id,step_id,capability_id,idempotency_key,effect_summary,observed_at_ms,replay_safe,data_class_rank) VALUES (?1,?2,?3,?4,?5,'known',42,0,1)", rusqlite::params![rid().as_str(), tid(1).as_str(), sid(1).as_str(), step.capability_id.as_ref().unwrap().as_str(), step.idempotency_key.as_ref().unwrap().as_str()]).unwrap_err();
    assert_eq!(
        error.sqlite_error_code(),
        Some(rusqlite::ErrorCode::ConstraintViolation)
    );
    assert_eq!(before, f.dump());
    let r = recover(&mut e, &c, 50);
    uncertain(&r);
    no_outcomes(&f, journal_count(&f));
    stable(&f, &mut e, &c, 50, &r);
    drop(g);
}

#[test]
fn m5_receipt_with_closed_absence_status_is_corruption_not_success() {
    let f = FileFixture::new("receipt-incompatible-status");
    let c = Context::new();
    let mut e = f.open();
    committed_receipt(&mut e, &c);
    // INSERT triggers stay enabled. They do not cover a later status UPDATE.
    // This schema-representable corruption is NOT an ordinary recovery window.
    f.execute("UPDATE task_steps SET status='RECONCILED_ABSENT'");
    let before = f.dump();
    let r = recover(&mut e, &c, 50);
    invariant(&r, 1);
    assert_eq!(r.tasks_resumed, 0);
    same_except(&before, &f.dump(), &["tasks", "task_journal"]);
    assert_eq!(
        f.count("SELECT count(*) FROM task_steps WHERE status='RECONCILED_ABSENT'"),
        1
    );
    stable(&f, &mut e, &c, 50, &r);
}

#[test]
fn m6_released_executing_without_outcome_needs_reconciliation_journal_only() {
    let f = FileFixture::new("released-inflight");
    let c = Context::new();
    let mut e = f.open();
    let g = inflight(&mut e, &c, 3);
    e.release(g, at(42), &c.view()).unwrap();
    drop(e);
    let mut e = f.open();
    let before = f.dump();
    let seq = journal_count(&f);
    let r = recover(&mut e, &c, 50);
    uncertain(&r);
    assert_eq!(r.repairs_committed, 1);
    same_except(&before, &f.dump(), &["task_journal"]);
    no_outcomes(&f, seq);
    let second = stable(&f, &mut e, &c, 50, &r);
    assert_eq!(r.decisions, second.decisions);
}

// No normal P2F WAITING writer exists. These are schema-representable imported
// snapshots: real plan/acquire/begin/release establish input and prior authority,
// then the snapshot supplies WAITING and its legal aggregate, without pretending
// this SQL is a production writer or inventing an approval/input delivery event.
fn waiting(f: &FileFixture, e: &mut TaskEngine, c: &Context, kind: StepKind) {
    single(e, c, kind, 3);
    let g = acquire(e, c, 1, None, 40, 60);
    e.begin_attempt(&g, at(41), &c.view()).unwrap();
    e.release(g, at(42), &c.view()).unwrap();
    let state = if kind == StepKind::WaitApproval {
        "WAITING_APPROVAL"
    } else {
        "WAITING_USER"
    };
    f.execute(&format!("BEGIN IMMEDIATE; UPDATE task_steps SET status='WAITING',lease_owner=NULL,lease_expires_at_ms=NULL; UPDATE tasks SET state='{state}',updated_at_ms=42; COMMIT"));
    assert!(e.load(tid(1)).is_ok());
}

#[test]
fn m7_waiting_approval_step_is_deferred_without_render_or_mutation() {
    let f = FileFixture::new("waiting-approval");
    let c = Context::new();
    let mut e = f.open();
    waiting(&f, &mut e, &c, StepKind::WaitApproval);
    let before = f.dump();
    let r = recover(&mut e, &c, 50);
    has(
        &r,
        RecoveryDecision::AwaitApproval {
            task_id: tid(1),
            step_id: Some(sid(1)),
        },
    );
    assert_eq!(r.tasks_resumed, 0);
    same_except(&before, &f.dump(), &["task_journal"]);
    assert_eq!(r.decisions, stable(&f, &mut e, &c, 50, &r).decisions);
}

#[test]
fn m7_aggregate_approval_without_step_is_deferred_with_none() {
    let f = FileFixture::new("aggregate-approval");
    let c = Context::new();
    let mut e = f.open();
    planning(&mut e, &c, 1, 3);
    // Schema-representable aggregate waiting before a plan exists; no writer yet.
    f.execute("UPDATE tasks SET state='WAITING_APPROVAL'");
    let before = f.dump();
    let r = recover(&mut e, &c, 50);
    has(
        &r,
        RecoveryDecision::AwaitApproval {
            task_id: tid(1),
            step_id: None,
        },
    );
    same_except(&before, &f.dump(), &["task_journal"]);
    stable(&f, &mut e, &c, 50, &r);
}

#[test]
fn m8_wait_user_and_wait_schedule_are_deferred_without_input_or_scheduler() {
    for kind in [StepKind::WaitUser, StepKind::WaitSchedule] {
        let f = FileFixture::new("waiting-user-schedule");
        let c = Context::new();
        let mut e = f.open();
        waiting(&f, &mut e, &c, kind);
        let before = f.dump();
        let r = recover(&mut e, &c, 50);
        has(&r, RecoveryDecision::AwaitUser { task_id: tid(1) });
        assert_eq!(r.tasks_resumed, 0);
        same_except(&before, &f.dump(), &["task_journal"]);
        assert_eq!(r.decisions, stable(&f, &mut e, &c, 50, &r).decisions);
    }
}

#[test]
fn m9_released_leased_step_resumes_ready_or_executing_without_resetting_runtime() {
    for executing in [false, true] {
        let f = FileFixture::new("released-leased");
        let c = Context::new();
        let mut e = f.open();
        single(&mut e, &c, StepKind::Capability, 3);
        let mut g = acquire(&mut e, &c, 1, None, 40, 50);
        if executing {
            e.begin_attempt(&g, at(41), &c.view()).unwrap();
            drop(g);
            g = acquire(&mut e, &c, 1, Some(1), 50, 70);
        }
        e.release(g, at(51), &c.view()).unwrap();
        let expected = e.load(tid(1)).unwrap();
        let before = f.dump();
        let seq = journal_count(&f);
        let r = recover(&mut e, &c, 60);
        has(&r, resume(1, Some(1)));
        assert_eq!(r.tasks_resumed, 1);
        assert_eq!(e.load(tid(1)).unwrap().task, expected.task);
        same_except(&before, &f.dump(), &["task_journal"]);
        no_outcomes(&f, seq);
        stable(&f, &mut e, &c, 60, &r);
    }
}

#[test]
fn m10_valid_committed_receipt_is_observed_without_repeat_outcome() {
    let f = FileFixture::new("committed-receipt");
    let c = Context::new();
    let mut e = f.open();
    committed_receipt(&mut e, &c);
    let before = f.dump();
    let seq = journal_count(&f);
    let r = recover(&mut e, &c, 50);
    has(
        &r,
        RecoveryDecision::ReceiptAlreadyCommitted {
            task_id: tid(1),
            step_id: sid(1),
            receipt_id: rid(),
        },
    );
    has(&r, resume(1, None));
    same_except(&before, &f.dump(), &["task_journal"]);
    assert_eq!(f.count(&format!("SELECT count(*) FROM task_journal WHERE journal_seq>{seq} AND journal_kind IN ('STEP_COMMITTED','RECEIPT_RECORDED')")), 0);
    assert_eq!(f.count("SELECT count(*) FROM side_effect_receipts"), 1);
    stable(&f, &mut e, &c, 50, &r);
}

#[test]
fn m10_verifying_without_verifier_never_invents_completed() {
    let f = FileFixture::new("no-verifier");
    let c = Context::new();
    let mut e = f.open();
    single(&mut e, &c, StepKind::Notify, 3);
    let g = acquire(&mut e, &c, 1, None, 40, 50);
    e.begin_attempt(&g, at(41), &c.view()).unwrap();
    e.commit_step(g, success(), at(42), &c.view()).unwrap();
    let before = f.dump();
    let r = recover(&mut e, &c, 50);
    has(&r, resume(1, None));
    assert_eq!(e.load(tid(1)).unwrap().task.state, TaskState::Verifying);
    assert_eq!(
        f.count("SELECT count(*) FROM task_journal WHERE journal_kind='TASK_TERMINAL'"),
        0
    );
    same_except(&before, &f.dump(), &["task_journal"]);
    stable(&f, &mut e, &c, 50, &r);
}

#[test]
fn m10_verifier_suffix_is_eligible_only_after_real_ordinary_success() {
    let f = FileFixture::new("verifier-eligible");
    let c = Context::new();
    let mut e = f.open();
    ready(
        &mut e,
        &c,
        1,
        3,
        vec![
            input(1, 1, 10, StepKind::Notify),
            input(1, 2, 20, StepKind::Verify),
        ],
    );
    let g = acquire(&mut e, &c, 1, None, 40, 50);
    e.begin_attempt(&g, at(41), &c.view()).unwrap();
    e.commit_step(g, success(), at(42), &c.view()).unwrap();
    let before = f.dump();
    let r = recover(&mut e, &c, 50);
    has(&r, resume(1, Some(2)));
    assert_eq!(r.tasks_resumed, 1);
    same_except(&before, &f.dump(), &["task_journal"]);
    stable(&f, &mut e, &c, 50, &r);
}

fn verifier_ready(e: &mut TaskEngine, c: &Context, ceiling: u32) {
    ready(
        e,
        c,
        1,
        ceiling,
        vec![
            input(1, 1, 10, StepKind::Notify),
            input(1, 2, 20, StepKind::Verify),
        ],
    );
    let g = acquire(e, c, 1, None, 40, 50);
    e.begin_attempt(&g, at(41), &c.view()).unwrap();
    assert_eq!(
        e.commit_step(g, success(), at(42), &c.view())
            .unwrap()
            .task_state,
        TaskState::Verifying
    );
}
fn no_new_verifier_execution(f: &FileFixture, since: u64) {
    assert_eq!(f.count(&format!("SELECT count(*) FROM task_journal WHERE journal_seq>{since} AND journal_kind IN ('STEP_ATTEMPT_STARTED','STEP_COMMITTED','STEP_FAILED','RECEIPT_RECORDED','STEP_RECONCILED_ABSENT')")), 0);
    assert_eq!(f.count("SELECT count(*) FROM task_steps WHERE status='FAILED' OR error_kind IS NOT NULL OR error_code IS NOT NULL"), 0);
    assert_eq!(
        f.count("SELECT count(*) FROM tasks WHERE failure_reason IS NOT NULL"),
        0
    );
    assert_eq!(f.count("SELECT count(*) FROM side_effect_receipts"), 0);
    // The only result is the real ordinary predecessor, not invented verifier work.
    assert_eq!(
        f.count("SELECT count(*) FROM step_blob_refs WHERE role='RESULT'"),
        1
    );
}

#[test]
fn remediation_t3_verifier_receipt_repairs_stale_verifying_to_completed_then_terminal_noop() {
    let f = FileFixture::new("verifier-receipt-terminal-repair");
    let c = Context::new();
    let mut e = f.open();
    verifier_ready(&mut e, &c, 3);
    let g = acquire(&mut e, &c, 2, None, 43, 60);
    e.begin_attempt(&g, at(44), &c.view()).unwrap();
    let seq_before_commit = journal_count(&f);
    let mut receipt = receipt(&e.load(tid(1)).unwrap().steps[1].step);
    receipt.observed_at = Timestamp::from_epoch_millis(at(45));
    assert_eq!(
        e.commit_step(
            g,
            StepOutcome::Succeeded {
                result_json: b"{\"verified\":true}",
                receipt: Some(&receipt)
            },
            at(46),
            &c.view()
        )
        .unwrap()
        .task_state,
        TaskState::Completed
    );
    let kinds: Vec<String> = f.sql().prepare(&format!("SELECT journal_kind FROM task_journal WHERE journal_seq>{seq_before_commit} ORDER BY journal_seq")).unwrap().query_map([], |r| r.get(0)).unwrap().collect::<Result<_, _>>().unwrap();
    assert_eq!(
        kinds,
        [
            "STEP_COMMITTED",
            "RECEIPT_RECORDED",
            "TASK_STATE_CHANGED",
            "TASK_TERMINAL"
        ]
    );
    assert_eq!(
        f.count("SELECT count(*) FROM leases WHERE released_at_ms=46"),
        1
    );
    let expected = e.load(tid(1)).unwrap();
    // Controlled corruption only AFTER the complete real verifier outcome audit.
    f.execute("UPDATE tasks SET state='VERIFYING' WHERE state='COMPLETED'");
    let before = f.dump();
    let seq = journal_count(&f);
    let r = recover(&mut e, &c, 50);
    assert_eq!(
        [
            r.tasks_examined,
            r.repairs_committed,
            r.tasks_resumed,
            r.invariant_violations
        ],
        [1, 1, 0, 0]
    );
    assert_eq!(
        r.decisions,
        vec![RecoveryDecision::ReceiptAlreadyCommitted {
            task_id: tid(1),
            step_id: sid(2),
            receipt_id: rid()
        }]
    );
    let actual = e.load(tid(1)).unwrap();
    assert_eq!(actual.task.state, TaskState::Completed);
    assert!(
        actual.steps == expected.steps,
        "verifier runtime/provenance changed"
    );
    assert_eq!(actual.plan_revision, expected.plan_revision);
    same_except(&before, &f.dump(), &["tasks", "task_journal"]);
    assert_eq!(f.count("SELECT count(*) FROM side_effect_receipts"), 1);
    assert_eq!(f.count(&format!("SELECT count(*) FROM task_journal WHERE journal_seq>{seq} AND journal_kind IN ('STEP_ATTEMPT_STARTED','STEP_COMMITTED','STEP_FAILED','RECEIPT_RECORDED')")), 0);
    let second = stable(&f, &mut e, &c, 50, &r);
    assert_eq!(
        second.decisions,
        vec![RecoveryDecision::TerminalNoop {
            task_id: tid(1),
            state: TaskState::Completed
        }]
    );
    assert_eq!(
        [
            second.tasks_examined,
            second.tasks_resumed,
            second.repairs_committed,
            second.invariant_violations
        ],
        [1, 0, 0, 0]
    );
}

#[test]
fn remediation_t4_held_verifier_leased_and_executing_preserve_even_exhausted_authority() {
    for begun in [false, true] {
        let f = FileFixture::new("held-verifier");
        let c = Context::new();
        let mut e = f.open();
        verifier_ready(&mut e, &c, 1);
        let g = acquire(&mut e, &c, 2, None, 43, 50);
        if begun {
            e.begin_attempt(&g, at(44), &c.view()).unwrap();
        }
        let before = f.dump();
        let seq = journal_count(&f);
        let r = recover(&mut e, &c, 49);
        assert_eq!(
            r.decisions,
            vec![RecoveryDecision::HeldLease {
                task_id: tid(1),
                step_id: sid(2)
            }]
        );
        assert_eq!(
            [r.tasks_examined, r.tasks_resumed, r.invariant_violations],
            [1, 0, 0]
        );
        same_except(&before, &f.dump(), &["task_journal"]);
        no_new_verifier_execution(&f, seq);
        assert_eq!(stable(&f, &mut e, &c, 49, &r).decisions, r.decisions);
        if !begun {
            e.begin_attempt(&g, at(49), &c.view()).unwrap();
        }
        assert_eq!(
            e.commit_step(g, success(), at(50), &c.view())
                .unwrap()
                .task_state,
            TaskState::Completed
        );
    }
}

#[test]
fn remediation_t4_released_verifier_leased_resumes_but_executing_needs_reconciliation() {
    for begun in [false, true] {
        let f = FileFixture::new("released-verifier");
        let c = Context::new();
        let mut e = f.open();
        verifier_ready(&mut e, &c, 3);
        let g = acquire(&mut e, &c, 2, None, 43, 50);
        if begun {
            e.begin_attempt(&g, at(44), &c.view()).unwrap();
        }
        e.release(g, at(45), &c.view()).unwrap();
        let before = f.dump();
        let seq = journal_count(&f);
        let r = recover(&mut e, &c, 50);
        let expected = if begun {
            RecoveryDecision::NeedsReconciliation {
                task_id: tid(1),
                step_id: sid(2),
            }
        } else {
            resume(1, Some(2))
        };
        assert_eq!(r.decisions, vec![expected]);
        assert_eq!(r.tasks_resumed, u64::from(!begun));
        assert_eq!(r.invariant_violations, 0);
        assert_eq!(e.load(tid(1)).unwrap().task.state, TaskState::Verifying);
        same_except(&before, &f.dump(), &["task_journal"]);
        no_new_verifier_execution(&f, seq);
        assert_eq!(stable(&f, &mut e, &c, 50, &r).decisions, r.decisions);
    }
}

#[test]
fn remediation_t4_released_exhausted_verifier_blocks_without_rewriting_runtime() {
    for begun in [false, true] {
        let f = FileFixture::new("released-exhausted-verifier");
        let c = Context::new();
        let mut e = f.open();
        verifier_ready(&mut e, &c, 1);
        let g = acquire(&mut e, &c, 2, None, 43, 50);
        if begun {
            e.begin_attempt(&g, at(44), &c.view()).unwrap();
        }
        e.release(g, at(45), &c.view()).unwrap();
        let expected = e.load(tid(1)).unwrap();
        let before = f.dump();
        let seq = journal_count(&f);
        let r = recover(&mut e, &c, 50);
        has(
            &r,
            RecoveryDecision::NeedsReconciliation {
                task_id: tid(1),
                step_id: sid(2),
            },
        );
        has(&r, RecoveryDecision::BlockedTask { task_id: tid(1) });
        assert!(
            !r.decisions
                .iter()
                .any(|d| matches!(d, RecoveryDecision::ExpiredLease { .. })),
            "already released authority cannot be revoked again"
        );
        assert_eq!(
            [
                r.tasks_examined,
                r.tasks_resumed,
                r.repairs_committed,
                r.invariant_violations
            ],
            [1, 0, 1, 0]
        );
        let actual = e.load(tid(1)).unwrap();
        assert_eq!(actual.task.state, TaskState::Blocked);
        assert!(matches!(
            actual.task.blocked_reason.as_ref().unwrap().as_str(),
            "NEEDS_RECONCILIATION" | "INVARIANT_VIOLATION"
        ));
        assert!(
            actual.steps == expected.steps,
            "released verifier facts must remain intact"
        );
        assert_eq!(actual.plan_revision, 1);
        same_except(&before, &f.dump(), &["tasks", "task_journal"]);
        no_new_verifier_execution(&f, seq);
        assert_eq!(stable(&f, &mut e, &c, 50, &r).decisions, r.decisions);
    }
}

#[test]
fn remediation_t4_exact_expired_exhausted_verifier_blocks_without_provider_failure() {
    for begun in [false, true] {
        let f = FileFixture::new("expired-exhausted-verifier");
        let c = Context::new();
        let mut e = f.open();
        verifier_ready(&mut e, &c, 1);
        let g = acquire(&mut e, &c, 2, None, 43, 50);
        if begun {
            e.begin_attempt(&g, at(44), &c.view()).unwrap();
        }
        let expected = e.load(tid(1)).unwrap();
        let before = f.dump();
        let seq = journal_count(&f);
        let r = recover(&mut e, &c, 50);
        has(
            &r,
            RecoveryDecision::ExpiredLease {
                task_id: tid(1),
                step_id: sid(2),
            },
        );
        has(
            &r,
            RecoveryDecision::NeedsReconciliation {
                task_id: tid(1),
                step_id: sid(2),
            },
        );
        has(&r, RecoveryDecision::BlockedTask { task_id: tid(1) });
        assert_eq!(
            [
                r.tasks_examined,
                r.tasks_resumed,
                r.repairs_committed,
                r.invariant_violations
            ],
            [1, 0, 1, 0]
        );
        let actual = e.load(tid(1)).unwrap();
        assert_eq!(actual.task.state, TaskState::Blocked);
        assert!(matches!(
            actual.task.blocked_reason.as_ref().unwrap().as_str(),
            "NEEDS_RECONCILIATION" | "INVARIANT_VIOLATION"
        ));
        assert!(
            actual.steps == expected.steps,
            "expiry must not begin/reset/finish verifier work"
        );
        assert_eq!(actual.plan_revision, 1);
        assert_eq!(
            f.count("SELECT count(*) FROM leases WHERE released_at_ms=50"),
            1
        );
        same_except(&before, &f.dump(), &["tasks", "leases", "task_journal"]);
        no_new_verifier_execution(&f, seq);
        let second = stable(&f, &mut e, &c, 50, &r);
        has(
            &second,
            RecoveryDecision::NeedsReconciliation {
                task_id: tid(1),
                step_id: sid(2),
            },
        );
        assert!(
            !second
                .decisions
                .iter()
                .any(|d| matches!(d, RecoveryDecision::ExpiredLease { .. }))
        );
        let after = f.dump();
        assert_eq!(
            e.commit_step(g, success(), at(51), &c.view()).err(),
            Some(EngineError::Store(StoreError::LeaseFenced))
        );
        assert_eq!(after, f.dump());
    }
}

#[test]
fn m11_received_planning_and_ready_are_eligible_not_planned_or_executed() {
    let f = FileFixture::new("ordinary-eligibility");
    let c = Context::new();
    let mut e = f.open();
    e.create_task(spec(1, 3), &c.view()).unwrap();
    planning(&mut e, &c, 2, 3);
    ready(
        &mut e,
        &c,
        3,
        3,
        vec![input(3, 3, 10, StepKind::Capability)],
    );
    let before = f.dump();
    let r = recover(&mut e, &c, 50);
    assert_eq!(
        r.decisions,
        vec![resume(1, None), resume(2, None), resume(3, Some(3))]
    );
    assert_eq!(
        [
            r.tasks_examined,
            r.tasks_resumed,
            r.repairs_committed,
            r.invariant_violations
        ],
        [3, 3, 3, 0]
    );
    same_except(&before, &f.dump(), &["task_journal"]);
    assert_eq!(r.decisions, stable(&f, &mut e, &c, 50, &r).decisions);
}

#[test]
fn m11_blocked_task_does_not_infer_external_block_cleared() {
    let f = FileFixture::new("blocked");
    let c = Context::new();
    let mut e = f.open();
    planning(&mut e, &c, 1, 3);
    e.block(
        tid(1),
        TaskState::Planning,
        BlockedReason::new("POLICY_DENIED").unwrap(),
        at(40),
        &c.view(),
    )
    .unwrap();
    let before = f.dump();
    let r = recover(&mut e, &c, 50);
    has(&r, RecoveryDecision::BlockedTask { task_id: tid(1) });
    assert_eq!(r.tasks_resumed, 0);
    same_except(&before, &f.dump(), &["task_journal"]);
    assert_eq!(
        e.load(tid(1))
            .unwrap()
            .task
            .blocked_reason
            .unwrap()
            .as_str(),
        "POLICY_DENIED"
    );
    stable(&f, &mut e, &c, 50, &r);
}

#[test]
fn m11_existing_closed_absence_is_not_a_successful_predecessor() {
    let f = FileFixture::new("closed-absence");
    let c = Context::new();
    let mut e = f.open();
    ready(
        &mut e,
        &c,
        1,
        3,
        vec![
            input(1, 1, 10, StepKind::Capability),
            input(1, 2, 20, StepKind::Notify),
        ],
    );
    let g = acquire(&mut e, &c, 1, None, 40, 50);
    e.begin_attempt(&g, at(41), &c.view()).unwrap();
    e.release(g, at(42), &c.view()).unwrap();
    e.block(
        tid(1),
        TaskState::Executing,
        BlockedReason::new("NEEDS_RECONCILIATION").unwrap(),
        at(43),
        &c.view(),
    )
    .unwrap();
    // Imported schema-representable closure, not a P2G inference of absence.
    // P2 has no confirmed-absence writer; recovery must preserve, not synthesise it.
    f.execute("UPDATE task_steps SET status='RECONCILED_ABSENT',completed_at_ms=43,lease_owner=NULL,lease_expires_at_ms=NULL WHERE sequence=10");
    let before = f.dump();
    let seq = journal_count(&f);
    let r = recover(&mut e, &c, 50);
    assert_eq!(r.tasks_resumed, 0);
    assert!(!r.decisions.contains(&resume(1, Some(2))));
    same_except(&before, &f.dump(), &["task_journal"]);
    no_outcomes(&f, seq);
    stable(&f, &mut e, &c, 50, &r);
}

#[test]
fn m12_m13_mixed_pass_changes_once_then_all_durable_tables_are_byte_identical() {
    let f = FileFixture::new("mixed-identity");
    let c = Context::new();
    let mut e = f.open();
    let g = inflight(&mut e, &c, 3);
    e.create_task(spec(2, 3), &c.view()).unwrap();
    planning(&mut e, &c, 3, 3);
    e.cancel(
        tid(3),
        TaskOriginKind::new("HOST").unwrap(),
        at(30),
        &c.view(),
    )
    .unwrap();
    ready(&mut e, &c, 4, 3, vec![input(4, 4, 10, StepKind::Notify)]);
    drop(g);
    drop(e);
    let mut e = f.open();
    let before = f.dump();
    assert_eq!(
        before.keys().map(String::as_str).collect::<Vec<_>>(),
        vec![
            "approval_lifecycle_wakes",
            "blobs",
            "capability_descriptor_revisions",
            "capability_generation_defaults",
            "capability_generation_members",
            "capability_overlays",
            "capability_registry_generations",
            "capability_registry_state",
            "device_resume_waits",
            "device_session_resume_wakes",
            "event_content",
            "event_expired_ranges",
            "event_sequence_ledger",
            "event_store_state",
            "leases",
            "model_call_attempts",
            "model_usage",
            "plan_revisions",
            "schedule_command_receipts",
            "schedule_occurrences",
            "scheduler_consumer_state",
            "schedules",
            "schema_migrations",
            "side_effect_receipts",
            "sqlite_sequence",
            "step_blob_refs",
            "step_capability_bindings",
            "task_blob_refs",
            "task_journal",
            "task_steps",
            "tasks"
        ]
    );
    let r = recover(&mut e, &c, 50);
    assert_ne!(before, f.dump());
    assert_eq!(
        [
            r.tasks_examined,
            r.tasks_resumed,
            r.repairs_committed,
            r.invariant_violations
        ],
        [4, 2, 3, 0]
    );
    has(
        &r,
        RecoveryDecision::ExpiredLease {
            task_id: tid(1),
            step_id: sid(1),
        },
    );
    has(
        &r,
        RecoveryDecision::NeedsReconciliation {
            task_id: tid(1),
            step_id: sid(1),
        },
    );
    has(&r, resume(2, None));
    has(
        &r,
        RecoveryDecision::TerminalNoop {
            task_id: tid(3),
            state: TaskState::Cancelled,
        },
    );
    has(&r, resume(4, Some(4)));
    same_except(&before, &f.dump(), &["leases", "task_journal"]);
    let second = stable(&f, &mut e, &c, 50, &r);
    assert_eq!(second.tasks_resumed, 2);
    drop(e);
    let mut e = f.open();
    let before_reopen_pass = f.dump();
    let third = recover(&mut e, &c, 50);
    assert_eq!(third.repairs_committed, 0);
    assert_eq!(third.decisions, second.decisions);
    assert_eq!(before_reopen_pass, f.dump());
}

#[test]
fn m14_expired_inflight_revokes_and_needs_reconciliation_without_execution() {
    let f = FileFixture::new("expired-inflight");
    let c = Context::new();
    let mut e = f.open();
    let g = inflight(&mut e, &c, 3);
    let expected = e.load(tid(1)).unwrap();
    let before = f.dump();
    let seq = journal_count(&f);
    let r = recover(&mut e, &c, 50);
    uncertain(&r);
    has(
        &r,
        RecoveryDecision::ExpiredLease {
            task_id: tid(1),
            step_id: sid(1),
        },
    );
    assert_eq!(r.repairs_committed, 1);
    assert_eq!(e.load(tid(1)).unwrap().task, expected.task);
    same_except(&before, &f.dump(), &["leases", "task_journal"]);
    no_outcomes(&f, seq);
    stable(&f, &mut e, &c, 50, &r);
    let after = f.dump();
    assert_eq!(
        e.commit_step(g, success(), at(51), &c.view()).err(),
        Some(EngineError::Store(StoreError::LeaseFenced))
    );
    assert_eq!(after, f.dump());
}

#[test]
fn m14_crash_exhausted_ready_uses_real_reassessment_then_block_never_begin_or_failure() {
    let f = FileFixture::new("ready-crash-ceiling");
    let c = Context::new();
    let mut e = f.open();
    single(&mut e, &c, StepKind::Capability, 1);
    let g = acquire(&mut e, &c, 1, None, 40, 50);
    drop(g); // Acquisition spent the sole attempt, but no attempt ever started.
    let expected = e.load(tid(1)).unwrap();
    let before = f.dump();
    let seq = journal_count(&f);
    let r = recover(&mut e, &c, 50);
    uncertain(&r);
    has(&r, RecoveryDecision::BlockedTask { task_id: tid(1) });
    assert_eq!(r.repairs_committed, 1);
    assert_eq!(r.invariant_violations, 0); // Crash budget exhaustion is not provider failure/corruption.
    let actual = e.load(tid(1)).unwrap();
    assert_eq!(actual.task.state, TaskState::Blocked);
    assert!(matches!(
        actual.task.blocked_reason.as_ref().unwrap().as_str(),
        "NEEDS_RECONCILIATION" | "INVARIANT_VIOLATION"
    ));
    assert!(actual.task.failure_reason.is_none());
    assert!(
        actual.steps == expected.steps,
        "step runtime/provenance changed"
    );
    assert_eq!(actual.plan_revision, 1);
    let conn = f.sql();
    let edges: Vec<(String, String, String)> = conn.prepare(&format!("SELECT state_from,state_to,reason_code FROM task_journal WHERE journal_seq>{seq} AND journal_kind='TASK_STATE_CHANGED' ORDER BY journal_seq")).unwrap()
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?))).unwrap().collect::<Result<_, _>>().unwrap();
    assert_eq!(edges.len(), 2);
    assert_eq!(
        edges[0],
        ("READY".into(), "PLANNING".into(), "REPLAN".into())
    );
    assert_eq!((&edges[1].0[..], &edges[1].1[..]), ("PLANNING", "BLOCKED"));
    assert!(!legal_task_transition(TaskState::Ready, TaskState::Blocked));
    same_except(&before, &f.dump(), &["tasks", "leases", "task_journal"]);
    no_outcomes(&f, seq);
    stable(&f, &mut e, &c, 50, &r);
}

#[test]
fn m14_crash_exhausted_executing_blocks_without_provider_failure() {
    let f = FileFixture::new("executing-crash-ceiling");
    let c = Context::new();
    let mut e = f.open();
    let g = inflight(&mut e, &c, 1);
    drop(g);
    let before = f.dump();
    let seq = journal_count(&f);
    let r = recover(&mut e, &c, 50);
    uncertain(&r);
    has(&r, RecoveryDecision::BlockedTask { task_id: tid(1) });
    assert_eq!(e.load(tid(1)).unwrap().task.state, TaskState::Blocked);
    assert_eq!(
        f.count("SELECT count(*) FROM tasks WHERE failure_reason IS NOT NULL"),
        0
    );
    assert_eq!(
        f.count("SELECT count(*) FROM task_steps WHERE status='FAILED' OR error_kind IS NOT NULL"),
        0
    );
    same_except(&before, &f.dump(), &["tasks", "leases", "task_journal"]);
    no_outcomes(&f, seq);
    stable(&f, &mut e, &c, 50, &r);
}

#[test]
fn m17_missing_or_mismatched_prior_authority_is_not_guessed_from_step_copy() {
    for corruption in [
        "DELETE FROM leases",
        "UPDATE leases SET owner='other-worker'",
        "UPDATE leases SET generation=2",
    ] {
        let f = FileFixture::new("authority-corruption");
        let c = Context::new();
        let mut e = f.open();
        let g = inflight(&mut e, &c, 3);
        drop(g);
        f.execute(corruption);
        let before = f.dump();
        let r = recover(&mut e, &c, 50);
        invariant(&r, 1);
        assert_eq!(r.tasks_resumed, 0);
        assert!(
            !r.decisions
                .iter()
                .any(|d| matches!(d, RecoveryDecision::ExpiredLease { .. }))
        );
        same_except(&before, &f.dump(), &["tasks", "task_journal"]);
        stable(&f, &mut e, &c, 50, &r);
    }
}

#[test]
fn m18_unknown_status_quarantines_with_exact_unrecognised_state_reason() {
    let f = FileFixture::new("unknown-status");
    let c = Context::new();
    let mut e = f.open();
    let g = inflight(&mut e, &c, 3);
    e.release(g, at(42), &c.view()).unwrap();
    let inspection = Store::open(&f.path, &Fixed).unwrap();
    f.execute("PRAGMA ignore_check_constraints=ON; UPDATE task_steps SET status='FUTURE_STATUS'; PRAGMA ignore_check_constraints=OFF");
    assert!(e.load(tid(1)).is_err());
    let observed = inspection
        .transact(|tx| {
            let snapshot = tx.inspect_recovery_task(&tid(1))?;
            assert_eq!(snapshot.raw_state(), "EXECUTING");
            assert_eq!(snapshot.steps()[0].raw_status(), "FUTURE_STATUS");
            Ok(snapshot.fingerprint().clone())
        })
        .unwrap();
    let before = f.dump();
    let r = recover(&mut e, &c, 50);
    invariant(&r, 1);
    has(
        &r,
        RecoveryDecision::CorruptOrInvariantViolation {
            task_id: tid(1),
            reason: ReasonCode::new("UNRECOGNISED_STATE").unwrap(),
        },
    );
    has(&r, RecoveryDecision::BlockedTask { task_id: tid(1) });
    assert_eq!(f.count("SELECT count(*) FROM tasks WHERE state='BLOCKED' AND blocked_reason='UNRECOGNISED_STATE'"), 1);
    assert_eq!(
        f.count("SELECT count(*) FROM task_steps WHERE status='FUTURE_STATUS'"),
        1
    );
    assert_corruption_evidence(&f, &observed, "FUTURE_STATUS");
    same_except(&before, &f.dump(), &["tasks", "task_journal"]);
    stable(&f, &mut e, &c, 50, &r);
}

#[test]
fn m18_pending_waiting_state_is_not_falsely_resolved_to_reach_blocked() {
    let f = FileFixture::new("waiting-corruption");
    let c = Context::new();
    let mut e = f.open();
    waiting(&f, &mut e, &c, StepKind::WaitApproval);
    f.execute("PRAGMA ignore_check_constraints=ON; UPDATE task_steps SET status='FUTURE_STATUS'; PRAGMA ignore_check_constraints=OFF");
    let before = f.dump();
    let r = recover(&mut e, &c, 50);
    invariant(&r, 1);
    assert_eq!(r.tasks_resumed, 0);
    assert_eq!(
        f.count("SELECT count(*) FROM tasks WHERE state='WAITING_APPROVAL'"),
        1
    );
    same_except(&before, &f.dump(), &["task_journal"]);
    stable(&f, &mut e, &c, 50, &r);
}

#[test]
fn m19_pending_event_transitions_counts_all_committed_history_including_recovery() {
    let f = FileFixture::new("pending-debt");
    let c = Context::new();
    let mut e = f.open();
    e.create_task(spec(1, 3), &c.view()).unwrap();
    assert_eq!(journal_count(&f), 1);
    let r = recover(&mut e, &c, 50);
    assert!(r.pending_event_transitions > 1);
    assert_eq!(r.pending_event_transitions, journal_count(&f));
    assert!(recovery_count(&f) > 0);
    stable(&f, &mut e, &c, 50, &r);
}

#[test]
fn m12_decision_identity_excludes_caller_time_actor_and_causation() {
    let f = FileFixture::new("identity-context");
    let c = Context::new();
    let mut e = f.open();
    e.create_task(spec(1, 3), &c.view()).unwrap();
    let first = recover(&mut e, &c, 50);
    assert_pending(&f, &first);
    let before = f.dump();
    let other = Context {
        actor: ActorId::new("other-recovery-host").unwrap(),
        version: SemVer::new("9.0.0").unwrap(),
        cause: EventId::new("evt_00000000000000000000000002").unwrap(),
    };
    let second = recover(&mut e, &other, 900);
    assert_eq!(second.repairs_committed, 0);
    assert_eq!(second.decisions, first.decisions);
    assert_eq!(before, f.dump());
}

#[test]
fn m12_materially_new_lease_generation_requires_new_decision_evidence() {
    let f = FileFixture::new("identity-generation");
    let c = Context::new();
    let mut e = f.open();
    single(&mut e, &c, StepKind::Capability, 3);
    let g = acquire(&mut e, &c, 1, None, 40, 50);
    drop(g);
    let first = recover(&mut e, &c, 50);
    stable(&f, &mut e, &c, 50, &first);
    let count = recovery_count(&f);
    let g = acquire(&mut e, &c, 1, Some(1), 51, 60);
    assert_eq!(g.generation(), 2);
    drop(g);
    let second = recover(&mut e, &c, 60);
    assert_eq!(second.repairs_committed, 1);
    assert!(recovery_count(&f) > count);
    assert_eq!(f.count("SELECT count(*) FROM task_steps WHERE attempt=2 AND lease_generation=2 AND started_at_ms IS NULL"), 1);
    stable(&f, &mut e, &c, 60, &second);
}

#[test]
fn m12_free_text_recovery_payload_does_not_override_structural_authority() {
    let f = FileFixture::new("identity-payload");
    let c = Context::new();
    let mut e = f.open();
    e.create_task(spec(1, 3), &c.view()).unwrap();
    let first = recover(&mut e, &c, 50);
    assert_pending(&f, &first);
    let payload = "{\"decision\":\"TerminalNoop\",\"note\":\"untrusted recovery prose\"}";
    f.sql().execute("UPDATE task_journal SET payload_json=?1,payload_digest=?2 WHERE journal_kind='RECOVERY_DECISION'", [payload, digest_of(payload).unwrap().as_str()]).unwrap();
    let r = recover(&mut e, &c, 50);
    has(&r, resume(1, None));
    assert_eq!(r.invariant_violations, 0);
    // A changed decision digest can cause fresh evidence; it cannot control flow.
    let before = f.dump();
    let second = recover(&mut e, &c, 50);
    assert_eq!(second.repairs_committed, 0);
    assert_eq!(before, f.dump());
}

#[test]
fn m17_task_local_semantic_damage_does_not_skip_other_valid_tasks() {
    let f = FileFixture::new("local-continue");
    let c = Context::new();
    let mut e = f.open();
    let g = inflight(&mut e, &c, 3);
    e.release(g, at(42), &c.view()).unwrap();
    e.create_task(spec(2, 3), &c.view()).unwrap();
    f.execute("UPDATE leases SET owner='mismatched-owner'");
    let r = recover(&mut e, &c, 50);
    invariant(&r, 1);
    has(&r, resume(2, None));
    assert_eq!(r.tasks_examined, 2);
    assert_eq!(r.tasks_resumed, 1);
    stable(&f, &mut e, &c, 50, &r);
}

// Races use independent file-backed engines. Serial tests pin BOTH winner
// orders; barrier tests accept either complete SQLite serialization, not timing
// or sleep-based guesses. No callback/effect is invoked inside recover.
fn race_outcome(order: Option<bool>) {
    let f = FileFixture::new("race-outcome");
    let c = Context::new();
    let mut worker = f.open();
    let guard = inflight(&mut worker, &c, 3);
    let receipt = receipt(&worker.load(tid(1)).unwrap().steps[0].step);
    let mut recovery = f.open();
    let outcome = StepOutcome::Succeeded {
        result_json: b"{\"known\":true}",
        receipt: Some(&receipt),
    };
    let (report, committed) = match order {
        Some(true) => {
            let r = recover(&mut recovery, &c, 50);
            (r, worker.commit_step(guard, outcome, at(50), &c.view()))
        }
        Some(false) => {
            let result = worker.commit_step(guard, outcome, at(50), &c.view());
            (recover(&mut recovery, &c, 50), result)
        }
        None => {
            let barrier = Barrier::new(2);
            let shared_barrier = &barrier;
            let context = &c;
            std::thread::scope(|scope| {
                let a = scope.spawn(|| {
                    shared_barrier.wait();
                    recover(&mut recovery, context, 50)
                });
                let b = scope.spawn(move || {
                    shared_barrier.wait();
                    worker.commit_step(guard, outcome, at(50), &context.view())
                });
                (a.join().unwrap(), b.join().unwrap())
            })
        }
    };
    match order {
        Some(false) => {
            assert!(
                committed.is_ok(),
                "forced outcome-first must commit: {:?}",
                committed.as_ref().err()
            );
            assert!(
                !report
                    .decisions
                    .iter()
                    .any(|d| matches!(d, RecoveryDecision::ExpiredLease { .. })),
                "outcome-first cannot revoke already committed authority"
            );
        }
        Some(true) => assert_eq!(
            committed.as_ref().err().cloned(),
            Some(EngineError::Store(StoreError::LeaseFenced)),
            "forced recovery-first must fence the old outcome"
        ),
        None => {} // A real barrier permits either complete SQLite winner order.
    }
    if committed.is_ok() {
        assert_eq!(f.count("SELECT count(*) FROM side_effect_receipts"), 1);
        assert_eq!(
            f.count("SELECT count(*) FROM task_journal WHERE journal_kind='STEP_COMMITTED'"),
            1
        );
        assert_eq!(
            f.count("SELECT count(*) FROM task_journal WHERE journal_kind='RECEIPT_RECORDED'"),
            1
        );
        has(
            &report,
            RecoveryDecision::ReceiptAlreadyCommitted {
                task_id: tid(1),
                step_id: sid(1),
                receipt_id: rid(),
            },
        );
        assert!(
            !report
                .decisions
                .iter()
                .any(|d| matches!(d, RecoveryDecision::ExpiredLease { .. }))
        );
        assert_eq!(
            recovery.load(tid(1)).unwrap().task.state,
            TaskState::Verifying
        );
    } else {
        assert_eq!(
            committed.err(),
            Some(EngineError::Store(StoreError::LeaseFenced))
        );
        uncertain(&report);
        has(
            &report,
            RecoveryDecision::ExpiredLease {
                task_id: tid(1),
                step_id: sid(1),
            },
        );
        assert_eq!(f.count("SELECT count(*) FROM side_effect_receipts"), 0);
        assert_eq!(
            f.count("SELECT count(*) FROM step_blob_refs WHERE role='RESULT'"),
            0
        );
        assert_eq!(
            recovery.load(tid(1)).unwrap().steps[0].step.status.as_str(),
            "EXECUTING"
        );
    }
    stable(&f, &mut recovery, &Context::new(), 50, &report);
}
#[test]
fn race_a_recovery_wins_before_known_expired_outcome() {
    race_outcome(Some(true));
}
#[test]
fn race_a_known_expired_outcome_wins_before_recovery() {
    race_outcome(Some(false));
}
#[test]
fn race_a_barrier_recovery_vs_known_expired_outcome() {
    race_outcome(None);
}

fn race_reclaim(order: Option<bool>) {
    let f = FileFixture::new("race-reclaim");
    let c = Context::new();
    let mut stale = f.open();
    let old = inflight(&mut stale, &c, 3);
    let mut recovery = f.open();
    let mut reclaimer = f.open();
    let (report, current) = match order {
        Some(true) => {
            let r = recover(&mut recovery, &c, 50);
            (r, acquire(&mut reclaimer, &c, 1, Some(1), 50, 100))
        }
        Some(false) => {
            let g = acquire(&mut reclaimer, &c, 1, Some(1), 50, 100);
            (recover(&mut recovery, &c, 50), g)
        }
        None => {
            let barrier = Barrier::new(2);
            std::thread::scope(|scope| {
                let a = scope.spawn(|| {
                    barrier.wait();
                    recover(&mut recovery, &c, 50)
                });
                let b = scope.spawn(|| {
                    barrier.wait();
                    acquire(&mut reclaimer, &c, 1, Some(1), 50, 100)
                });
                (a.join().unwrap(), b.join().unwrap())
            })
        }
    };
    assert_eq!(current.generation(), 2);
    assert_eq!(f.count("SELECT count(*) FROM leases WHERE generation=2 AND expires_at_ms=100 AND released_at_ms IS NULL"), 1);
    let step = recovery.load(tid(1)).unwrap().steps.remove(0).step;
    assert_eq!(step.attempt, 2);
    assert_eq!(step.status.as_str(), "LEASED");
    assert!(step.started_at.is_none());
    if report
        .decisions
        .iter()
        .any(|d| matches!(d, RecoveryDecision::ExpiredLease { .. }))
    {
        uncertain(&report);
    } else {
        has(
            &report,
            RecoveryDecision::HeldLease {
                task_id: tid(1),
                step_id: sid(1),
            },
        );
    }
    let before = f.dump();
    assert_eq!(
        stale.commit_step(old, success(), at(51), &c.view()).err(),
        Some(EngineError::Store(StoreError::LeaseFenced))
    );
    assert_eq!(before, f.dump());
    // Reclaim is a material authority change after recovery-first, so settle one
    // new observation before checking the identical second-pass invariant.
    let settled = recover(&mut recovery, &c, 50);
    has(
        &settled,
        RecoveryDecision::HeldLease {
            task_id: tid(1),
            step_id: sid(1),
        },
    );
    stable(&f, &mut recovery, &c, 50, &settled);
    reclaimer
        .begin_attempt(&current, at(51), &c.view())
        .unwrap();
    reclaimer
        .commit_step(current, success(), at(52), &c.view())
        .unwrap();
}
#[test]
fn race_b_recovery_wins_before_reclaim() {
    race_reclaim(Some(true));
}
#[test]
fn race_b_reclaim_wins_before_recovery() {
    race_reclaim(Some(false));
}
#[test]
fn race_b_barrier_recovery_vs_reclaim() {
    race_reclaim(None);
}

fn race_recoveries(order: Option<bool>) {
    let f = FileFixture::new("race-two-recoveries");
    let c = Context::new();
    let mut seed = f.open();
    let g = inflight(&mut seed, &c, 3);
    drop(g);
    drop(seed);
    let mut a = f.open();
    let mut b = f.open();
    let before = f.dump();
    let (ra, rb) = match order {
        Some(true) => (recover(&mut a, &c, 50), recover(&mut b, &c, 50)),
        Some(false) => {
            let rb = recover(&mut b, &c, 50);
            (recover(&mut a, &c, 50), rb)
        }
        None => {
            let barrier = Barrier::new(2);
            std::thread::scope(|scope| {
                let left = scope.spawn(|| {
                    barrier.wait();
                    recover(&mut a, &c, 50)
                });
                let right = scope.spawn(|| {
                    barrier.wait();
                    recover(&mut b, &c, 50)
                });
                (left.join().unwrap(), right.join().unwrap())
            })
        }
    };
    uncertain(&ra);
    uncertain(&rb);
    assert_eq!(ra.repairs_committed + rb.repairs_committed, 1);
    assert_eq!(ra.pending_event_transitions, rb.pending_event_transitions);
    assert_eq!(
        ra.decisions
            .iter()
            .chain(&rb.decisions)
            .filter(|d| matches!(d, RecoveryDecision::ExpiredLease { .. }))
            .count(),
        1
    );
    assert_eq!(
        f.count("SELECT count(*) FROM leases WHERE generation=1 AND released_at_ms=50"),
        1
    );
    same_except(&before, &f.dump(), &["leases", "task_journal"]);
    stable(&f, &mut a, &c, 50, &ra);
    let final_dump = f.dump();
    let third = recover(&mut b, &c, 50);
    assert_eq!(third.repairs_committed, 0);
    assert_eq!(final_dump, f.dump());
}
#[test]
fn race_c_recovery_a_wins_before_b() {
    race_recoveries(Some(true));
}
#[test]
fn race_c_recovery_b_wins_before_a() {
    race_recoveries(Some(false));
}
#[test]
fn race_c_barrier_two_recovery_callers() {
    race_recoveries(None);
}

// A conservative source guard, not a claim of compiler-derived call-graph proof.
// Strip standalone comments/doctests and cfg(test) items so fixture APIs cannot
// masquerade as runtime calls. Scan every remaining storage source file: this
// deliberately over-approximates the entire recovery call closure and its context.
fn runtime_source(source: &str) -> String {
    let mut output = String::new();
    let mut test_item = false;
    let mut opened = false;
    let mut braces = 0isize;
    for line in source.lines() {
        let line = line.trim();
        if line.starts_with("//") {
            continue;
        }
        if line == "#[cfg(test)]" {
            test_item = true;
            opened = false;
            braces = 0;
            continue;
        }
        if test_item {
            if line.starts_with("#[") {
                continue;
            }
            opened |= line.contains('{');
            braces += line.chars().filter(|c| *c == '{').count() as isize;
            braces -= line.chars().filter(|c| *c == '}').count() as isize;
            if (opened && braces == 0) || (!opened && line.ends_with(';')) {
                test_item = false;
            }
            continue;
        }
        output.push_str(line);
        output.push('\n');
    }
    assert!(!test_item, "unterminated cfg(test) item in source guard");
    output
}

#[test]
fn remediation_t1_storage_recovery_runtime_closure_has_no_effects_or_ambient_time() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut sources = Vec::new();
    rust_sources(&root.join("../serea-storage/src"), &mut sources);
    let runtime: Vec<_> = sources
        .into_iter()
        .filter(|(path, _)| {
            !path
                .file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .ends_with("_tests.rs")
                // P3's Scheduler storage operations are deliberately outside
                // the P2 Task recovery runtime closure asserted below.
                && path.file_name().unwrap() != "scheduler.rs"
        })
        .collect();
    for name in [
        "recovery.rs",
        "audit.rs",
        "task.rs",
        "lifecycle.rs",
        "lease.rs",
        "outcome.rs",
        "tx.rs",
        "blob.rs",
        "classify.rs",
        "migrate.rs",
        "lib.rs",
        "store.rs",
    ] {
        assert!(
            runtime
                .iter()
                .any(|(path, _)| path.file_name().unwrap() == name),
            "missing recovery closure coverage for {name}"
        );
    }
    for (path, source) in runtime {
        let source = runtime_source(&source);
        for forbidden in [
            "CapabilityProvider",
            "ModelProvider",
            "HostGoalProvider",
            "serea_capability",
            "serea_model_router",
            "serea_provider",
            "serea_scheduler",
            "scheduler::",
            // P3 error variants name Scheduler lease outcomes in error.rs;
            // the runtime module itself remains excluded from this P2 scan.
            "Scheduler",
            "GoalLatch",
            "goallatch::",
            "std::net",
            "TcpStream",
            "TcpListener",
            "UdpSocket",
            "reqwest",
            "hyper::",
            "ureq::",
            "curl::",
            "tokio::",
            "std::process::Command",
            "process::Command",
            "Command::new",
            "posix_spawn",
            "libc::system",
            "std::os::unix::process",
            "SystemTime",
            "Instant::now",
            "Utc::now",
            "Local::now",
            "CURRENT_TIMESTAMP",
            "CURRENT_DATE",
            "CURRENT_TIME",
            "'now'",
            "unixepoch()",
        ] {
            if (path.file_name().unwrap() == "error.rs" && forbidden == "Scheduler")
                || (path.file_name().unwrap() == "lib.rs"
                    && matches!(forbidden, "Scheduler" | "scheduler::"))
            {
                continue;
            }
            assert!(
                !source.contains(forbidden),
                "forbidden runtime API {forbidden} in {}",
                path.display()
            );
        }
        // File-open/init, SQL and Duration-based lock timeouts are legitimate
        // storage operations. Only the two established constructors read Clock.
        let mut compact: String = source.chars().filter(|c| !c.is_whitespace()).collect();
        if path.file_name().unwrap() == "store.rs" {
            let init_read = "letnow=clock.now_ms().map_err(StoreError::Clock)?;";
            assert_eq!(
                compact.matches(init_read).count(),
                2,
                "open-time clock contract changed"
            );
            compact = compact.replace(init_read, "");
            let fields = source
                .split("pub struct Store {")
                .nth(1)
                .unwrap()
                .split('}')
                .next()
                .unwrap();
            assert!(!fields.contains("Clock"), "Store must not retain a Clock");
        } else {
            assert!(
                !compact.contains("dynClock"),
                "Clock injected into storage runtime closure: {}",
                path.display()
            );
        }
        assert!(
            !compact.contains("now_ms"),
            "clock read outside Store initialization: {}",
            path.display()
        );
    }
    let recovery = runtime_source(&std::fs::read_to_string(root.join("src/recovery.rs")).unwrap());
    for forbidden in ["Store::open", "now_ms", "Clock", "std::fs", "std::process"] {
        assert!(
            !recovery.contains(forbidden),
            "engine recovery must use existing storage Tx, not {forbidden}"
        );
    }
}

fn rust_sources(dir: &Path, output: &mut Vec<(PathBuf, String)>) {
    let mut entries = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect::<Vec<_>>();
    entries.sort();
    for path in entries {
        if path.is_dir() {
            rust_sources(&path, output);
        } else if path.extension().is_some_and(|e| e == "rs") {
            output.push((path.clone(), std::fs::read_to_string(path).unwrap()));
        }
    }
}

#[test]
fn m20_recovery_dependency_and_source_closure_has_no_execution_or_upward_runtime() {
    // Scan the entire engine source closure, not just a conveniently isolated
    // recovery file. Its only production dependencies are protocol and storage;
    // provider port declarations in protocol alone are not runtime reachability.
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut sources = Vec::new();
    rust_sources(&root.join("src"), &mut sources);
    assert!(
        sources
            .iter()
            .any(|(_, source)| source.contains("pub fn recover")),
        "recovery API must be real production code"
    );
    for (path, source) in sources {
        for forbidden in [
            "CapabilityProvider",
            "ModelProvider",
            "HostGoalProvider",
            "Command::new",
            "std::process::Command",
            "reqwest",
            "TcpStream",
            "UdpSocket",
            "tokio::",
            "rusqlite",
            "Connection",
            "execute_batch",
            "INSERT INTO",
            "UPDATE tasks",
            "LeaseGuard::new",
        ] {
            assert!(
                !source.contains(forbidden),
                "{forbidden} reachable in {}",
                path.display()
            );
        }
    }
    for relative in [
        "Cargo.toml",
        "../serea-storage/Cargo.toml",
        "../serea-protocol/Cargo.toml",
    ] {
        let manifest = std::fs::read_to_string(root.join(relative)).unwrap();
        let production = manifest.split("[dev-dependencies]").next().unwrap();
        // P5E freezes the direction `task-engine -> capability` so a Task can
        // pin a generation and bind a Step. Every other upward edge stays
        // forbidden, and no crate may point back at the Task Engine.
        let forbidden: &[&str] = if relative == "Cargo.toml" {
            &[
                "serea-model-router",
                "serea-provider",
                "serea-core",
                "serea-scheduler",
                "serea-device",
                "reqwest",
                "tokio",
                "serea-testkit",
            ]
        } else {
            &[
                "serea-capability",
                "serea-model-router",
                "serea-provider",
                "serea-core",
                "serea-scheduler",
                "serea-device",
                "reqwest",
                "tokio",
                "serea-testkit",
            ]
        };
        for forbidden in forbidden {
            assert!(
                !production.contains(forbidden),
                "upward/runtime dependency {forbidden} in {relative}"
            );
        }
        if relative != "Cargo.toml" {
            assert!(
                !production.contains("serea-event-bus"),
                "Event Bus dependency may only point from Task Engine, found in {relative}"
            );
        }
    }
    // The P5E direction is one-way: the capability crate must never name the
    // Task Engine, so P6/P8 dispatch cannot be reached from registry authority.
    let capability_manifest =
        std::fs::read_to_string(root.join("../serea-capability/Cargo.toml")).unwrap();
    assert!(
        !capability_manifest.contains("serea-task-engine"),
        "capability must never depend on the Task Engine"
    );
    let production = std::fs::read_to_string(root.join("Cargo.toml")).unwrap();
    assert!(
        production
            .split("[dev-dependencies]")
            .next()
            .unwrap()
            .contains("serea-event-bus"),
        "Task Engine must use the P3 Event Bus participant"
    );
    assert!(!include_str!("../../serea-storage/Cargo.toml").contains("serea-task-engine"));
}

#[test]
fn m21_explicit_now_is_the_only_recovery_clock_no_retained_or_ambient_clock() {
    struct OpenOnly {
        reads: AtomicU64,
    }
    impl Clock for OpenOnly {
        fn now_ms(&self) -> Result<EpochMillis, ProtocolError> {
            assert_eq!(
                self.reads.fetch_add(1, Ordering::SeqCst),
                0,
                "clock called after open"
            );
            Ok(at(0))
        }
    }
    let f = FileFixture::new("open-only-clock");
    let c = Context::new();
    let clock = OpenOnly {
        reads: AtomicU64::new(0),
    };
    let mut e = TaskEngine::new(Store::open(&f.path, &clock).unwrap(), event_bus());
    single(&mut e, &c, StepKind::Notify, 3);
    let g = acquire(&mut e, &c, 1, None, 40, 50);
    drop(g);
    let held = recover(&mut e, &c, 49);
    has(
        &held,
        RecoveryDecision::HeldLease {
            task_id: tid(1),
            step_id: sid(1),
        },
    );
    let expired = recover(&mut e, &c, 50);
    has(
        &expired,
        RecoveryDecision::ExpiredLease {
            task_id: tid(1),
            step_id: sid(1),
        },
    );
    assert_eq!(clock.reads.load(Ordering::SeqCst), 1);
    stable(&f, &mut e, &c, 50, &expired);
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut sources = Vec::new();
    rust_sources(&root.join("src"), &mut sources);
    for (path, source) in sources {
        for forbidden in [
            "SystemTime",
            "Instant::now",
            "Utc::now",
            "Local::now",
            "datetime('now'",
            "unixepoch()",
            "CURRENT_TIMESTAMP",
            "now_ms(",
            "dyn Clock",
        ] {
            assert!(
                !source.contains(forbidden),
                "ambient/retained time {forbidden} in {}",
                path.display()
            );
        }
    }
}
