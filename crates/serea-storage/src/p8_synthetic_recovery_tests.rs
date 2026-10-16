//! Test-only P8C recovery simulator.
//!
//! This deliberately does not implement `CapabilityProvider`, is not wired to
//! TaskEngine, and is unreachable from Host/runtime. It tests the semantics of
//! a scripted effect ledger across independent SQLite reopen; it cannot prove
//! the P8B authority transaction or production provider behavior.

use rusqlite::{Connection, params};
use std::collections::VecDeque;
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_LEDGER: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, Copy)]
struct ManualClock(i64);

impl ManualClock {
    fn now(self) -> i64 {
        self.0
    }

    fn advance(&mut self, by_ms: i64) -> Result<(), &'static str> {
        self.0 = self.0.checked_add(by_ms).ok_or("clock overflow")?;
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SyntheticRequest {
    request_id: String,
    idempotency_key: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SyntheticResponse {
    request_id: String,
    idempotency_key: String,
    response_digest: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SyntheticProviderIdentity {
    provider_id: &'static str,
    instance_id: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SyntheticPermitRefusal {
    RequestMismatch,
    StepMismatch,
    LeaseGenerationMismatch,
    OwnerMismatch,
    ProviderInstanceMismatch,
    AlreadyConsumed,
    EffectLedgerFailure,
}

/// Proposed test-only shape for a non-serializable, local one-shot permit.
/// This type is not a production authorization or provider API.
struct SyntheticDispatchPermit {
    request: SyntheticRequest,
    step_id: String,
    lease_generation: u32,
    owner_id: String,
    provider: SyntheticProviderIdentity,
    consumed: bool,
}

struct SyntheticInvocation<'a> {
    request: &'a SyntheticRequest,
    step_id: &'a str,
    lease_generation: u32,
    owner_id: &'a str,
    provider_identity: SyntheticProviderIdentity,
    clock: ManualClock,
    deadline_ms: i64,
}

impl SyntheticDispatchPermit {
    fn new(
        request: SyntheticRequest,
        step_id: &str,
        lease_generation: u32,
        owner_id: &str,
        provider: SyntheticProviderIdentity,
    ) -> Self {
        Self {
            request,
            step_id: step_id.to_owned(),
            lease_generation,
            owner_id: owner_id.to_owned(),
            provider,
            consumed: false,
        }
    }

    fn invoke_once(
        &mut self,
        invocation: SyntheticInvocation<'_>,
        provider: &mut ScriptedFakeProvider,
    ) -> Result<SimulatedOutcome, SyntheticPermitRefusal> {
        if self.consumed {
            return Err(SyntheticPermitRefusal::AlreadyConsumed);
        }
        if invocation.request != &self.request {
            return Err(SyntheticPermitRefusal::RequestMismatch);
        }
        if invocation.step_id != self.step_id {
            return Err(SyntheticPermitRefusal::StepMismatch);
        }
        if invocation.lease_generation != self.lease_generation {
            return Err(SyntheticPermitRefusal::LeaseGenerationMismatch);
        }
        if invocation.owner_id != self.owner_id {
            return Err(SyntheticPermitRefusal::OwnerMismatch);
        }
        if invocation.provider_identity != self.provider {
            return Err(SyntheticPermitRefusal::ProviderInstanceMismatch);
        }

        // Consume before crossing the fake provider boundary. A failure or
        // ambiguous result cannot reuse this in-memory permit.
        self.consumed = true;
        provider
            .invoke(invocation.request, invocation.clock, invocation.deadline_ms)
            .map_err(|_| SyntheticPermitRefusal::EffectLedgerFailure)
    }
}

#[derive(Debug, Clone, Copy)]
enum Script {
    ApplyAndAcknowledge,
    ApplyThenLoseAcknowledgement,
    RejectBeforeEffect,
}

#[derive(Debug, PartialEq, Eq)]
enum SimulatedOutcome {
    Acknowledged(SyntheticResponse),
    Ambiguous,
    RejectedBeforeEffect,
    TimedOutBeforeEffect,
}

struct EffectLedger {
    path: PathBuf,
}

impl EffectLedger {
    fn open(path: PathBuf) -> rusqlite::Result<Self> {
        let conn = Connection::open(&path)?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS effects(
                 sequence INTEGER PRIMARY KEY AUTOINCREMENT,
                 request_id TEXT NOT NULL,
                 idempotency_key TEXT NOT NULL,
                 effect_digest TEXT NOT NULL
             ) STRICT;",
        )?;
        Ok(Self { path })
    }

    fn apply(&self, request: &SyntheticRequest) -> rusqlite::Result<()> {
        let conn = Connection::open(&self.path)?;
        conn.execute(
            "INSERT INTO effects(request_id,idempotency_key,effect_digest) VALUES(?1,?2,?3)",
            params![
                request.request_id,
                request.idempotency_key,
                "sha256:synthetic-effect"
            ],
        )?;
        Ok(())
    }

    fn count(&self, request_id: &str) -> rusqlite::Result<i64> {
        let conn = Connection::open(&self.path)?;
        conn.query_row(
            "SELECT count(*) FROM effects WHERE request_id=?1",
            [request_id],
            |row| row.get(0),
        )
    }

    fn count_pair(&self, request_id: &str, idempotency_key: &str) -> rusqlite::Result<i64> {
        let conn = Connection::open(&self.path)?;
        conn.query_row(
            "SELECT count(*) FROM effects WHERE request_id=?1 AND idempotency_key=?2",
            params![request_id, idempotency_key],
            |row| row.get(0),
        )
    }
}

/// Minimal independent SQLite ledger for committed synthetic Intent identity.
/// It is deliberately not the production Storage schema or dispatch API.
struct IntentLedger {
    path: PathBuf,
}

impl IntentLedger {
    fn open(path: PathBuf) -> rusqlite::Result<Self> {
        let conn = Connection::open(&path)?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS intents(
                 request_id TEXT PRIMARY KEY,
                 idempotency_key TEXT NOT NULL UNIQUE
             ) STRICT;",
        )?;
        Ok(Self { path })
    }

    fn commit(&self, request: &SyntheticRequest) -> rusqlite::Result<()> {
        let conn = Connection::open(&self.path)?;
        conn.execute(
            "INSERT INTO intents(request_id,idempotency_key) VALUES(?1,?2)",
            params![request.request_id, request.idempotency_key],
        )?;
        Ok(())
    }

    fn matches(&self, request: &SyntheticRequest) -> rusqlite::Result<bool> {
        let conn = Connection::open(&self.path)?;
        conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM intents WHERE request_id=?1 AND idempotency_key=?2)",
            params![request.request_id, request.idempotency_key],
            |row| row.get(0),
        )
    }
}

struct ScriptedFakeProvider {
    ledger: EffectLedger,
    script: VecDeque<Script>,
}

impl ScriptedFakeProvider {
    fn new(ledger: EffectLedger, script: impl IntoIterator<Item = Script>) -> Self {
        Self {
            ledger,
            script: script.into_iter().collect(),
        }
    }

    fn invoke(
        &mut self,
        request: &SyntheticRequest,
        clock: ManualClock,
        deadline_ms: i64,
    ) -> rusqlite::Result<SimulatedOutcome> {
        if clock.now() >= deadline_ms {
            return Ok(SimulatedOutcome::TimedOutBeforeEffect);
        }
        match self.script.pop_front() {
            Some(Script::ApplyAndAcknowledge) => {
                self.ledger.apply(request)?;
                Ok(SimulatedOutcome::Acknowledged(SyntheticResponse {
                    request_id: request.request_id.clone(),
                    idempotency_key: request.idempotency_key.clone(),
                    response_digest: "sha256:synthetic-response".into(),
                }))
            }
            Some(Script::ApplyThenLoseAcknowledgement) => {
                self.ledger.apply(request)?;
                Ok(SimulatedOutcome::Ambiguous)
            }
            Some(Script::RejectBeforeEffect) | None => Ok(SimulatedOutcome::RejectedBeforeEffect),
        }
    }
}

struct TempLedger(PathBuf);

impl TempLedger {
    fn new() -> Self {
        let sequence = NEXT_LEDGER.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!(
            "serea-p8-effect-ledger-{}-{sequence}",
            std::process::id()
        ));
        std::fs::create_dir(&dir).unwrap();
        Self(dir.join("effects.sqlite"))
    }

    fn intent_path(&self) -> PathBuf {
        self.0.parent().unwrap().join("intents.sqlite")
    }
}

impl Drop for TempLedger {
    fn drop(&mut self) {
        std::fs::remove_dir_all(self.0.parent().unwrap()).unwrap();
    }
}

#[test]
fn fake_provider_ack_loss_survives_independent_ledger_reopen_without_retry() {
    let db = TempLedger::new();
    let request = SyntheticRequest {
        request_id: "req_synthetic_1".into(),
        idempotency_key: "idk_synthetic_1".into(),
    };
    let clock = ManualClock(100);
    let mut provider = ScriptedFakeProvider::new(
        EffectLedger::open(db.0.clone()).unwrap(),
        [Script::ApplyThenLoseAcknowledgement],
    );
    assert_eq!(
        provider.invoke(&request, clock, 200).unwrap(),
        SimulatedOutcome::Ambiguous
    );
    drop(provider);

    // Reopening the ledger is independent from the provider object and proves
    // the simulated effect remains after the acknowledgement was lost.
    let reopened = EffectLedger::open(db.0.clone()).unwrap();
    assert_eq!(reopened.count(&request.request_id).unwrap(), 1);
    // V1 has no automatic same-Step effect retry. Recovery observes AMBIGUOUS
    // and leaves the effect count unchanged instead of invoking a second time.
    assert_eq!(reopened.count(&request.request_id).unwrap(), 1);
}

#[test]
fn committed_intent_and_independent_effect_ledger_correlate_after_ack_loss() {
    let db = TempLedger::new();
    let request = SyntheticRequest {
        request_id: "req_synthetic_correlated".into(),
        idempotency_key: "idk_synthetic_correlated".into(),
    };
    let intent = IntentLedger::open(db.intent_path()).unwrap();
    intent.commit(&request).unwrap();
    let mut provider = ScriptedFakeProvider::new(
        EffectLedger::open(db.0.clone()).unwrap(),
        [Script::ApplyThenLoseAcknowledgement],
    );
    assert_eq!(
        provider.invoke(&request, ManualClock(10), 20).unwrap(),
        SimulatedOutcome::Ambiguous
    );
    drop(provider);
    drop(intent);

    // These two files are reopened independently. Recovery can correlate the
    // exact committed RequestId/IDK but it does not call the provider again.
    let reopened_intent = IntentLedger::open(db.intent_path()).unwrap();
    let reopened_effects = EffectLedger::open(db.0.clone()).unwrap();
    assert!(reopened_intent.matches(&request).unwrap());
    assert_eq!(reopened_effects.count(&request.request_id).unwrap(), 1);
    assert_eq!(
        reopened_effects
            .count_pair(&request.request_id, &request.idempotency_key)
            .unwrap(),
        1
    );
    assert_eq!(reopened_effects.count(&request.request_id).unwrap(), 1);
}

#[test]
fn committed_intent_reopen_without_invoke_never_starts_effect_during_recovery() {
    let db = TempLedger::new();
    let request = SyntheticRequest {
        request_id: "req_synthetic_preinvoke_crash".into(),
        idempotency_key: "idk_synthetic_preinvoke_crash".into(),
    };
    IntentLedger::open(db.intent_path())
        .unwrap()
        .commit(&request)
        .unwrap();
    // Simulate loss of the worker after commit and before invoking the fake.
    // This is an independent-ledger reopen test, not an OS process-kill test.

    let reopened_intent = IntentLedger::open(db.intent_path()).unwrap();
    let reopened_effects = EffectLedger::open(db.0.clone()).unwrap();
    assert!(reopened_intent.matches(&request).unwrap());
    assert_eq!(reopened_effects.count(&request.request_id).unwrap(), 0);
    // Recovery intentionally has no code path that invokes a committed Intent.
    assert_eq!(reopened_effects.count(&request.request_id).unwrap(), 0);
}

#[test]
fn effect_ledger_pair_mismatch_is_not_receipt_evidence() {
    let db = TempLedger::new();
    let committed = SyntheticRequest {
        request_id: "req_synthetic_mismatch".into(),
        idempotency_key: "idk_synthetic_committed".into(),
    };
    let observed = SyntheticRequest {
        request_id: committed.request_id.clone(),
        idempotency_key: "idk_synthetic_wrong".into(),
    };
    let intent = IntentLedger::open(db.intent_path()).unwrap();
    intent.commit(&committed).unwrap();
    let mut provider = ScriptedFakeProvider::new(
        EffectLedger::open(db.0.clone()).unwrap(),
        [Script::ApplyAndAcknowledge],
    );
    let SimulatedOutcome::Acknowledged(response) =
        provider.invoke(&observed, ManualClock(10), 20).unwrap()
    else {
        panic!("scripted response expected")
    };

    assert_eq!(response.request_id, committed.request_id);
    assert!(!intent.matches(&observed).unwrap());
    let reopened = EffectLedger::open(db.0.clone()).unwrap();
    assert_eq!(reopened.count(&committed.request_id).unwrap(), 1);
    assert_eq!(
        reopened
            .count_pair(&committed.request_id, &committed.idempotency_key)
            .unwrap(),
        0,
        "an IDK mismatch cannot be accepted as the committed Intent's outcome"
    );
    assert_eq!(
        reopened
            .count_pair(&observed.request_id, &observed.idempotency_key)
            .unwrap(),
        1
    );
}

#[test]
fn fake_provider_keeps_duplicate_idempotency_key_invocations_visible() {
    let db = TempLedger::new();
    let first = SyntheticRequest {
        request_id: "req_synthetic_2".into(),
        idempotency_key: "idk_synthetic_2".into(),
    };
    let mut provider = ScriptedFakeProvider::new(
        EffectLedger::open(db.0.clone()).unwrap(),
        [Script::ApplyAndAcknowledge, Script::ApplyAndAcknowledge],
    );
    let clock = ManualClock(0);
    let SimulatedOutcome::Acknowledged(response) = provider.invoke(&first, clock, 1).unwrap()
    else {
        panic!("scripted acknowledgement expected")
    };
    assert_eq!(response.request_id, first.request_id);

    let second = SyntheticRequest {
        request_id: "req_synthetic_3".into(),
        idempotency_key: first.idempotency_key.clone(),
    };
    let SimulatedOutcome::Acknowledged(response) = provider.invoke(&second, clock, 1).unwrap()
    else {
        panic!("scripted acknowledgement expected")
    };
    assert_ne!(response.request_id, first.request_id);
    let reopened = EffectLedger::open(db.0.clone()).unwrap();
    assert_eq!(reopened.count(&first.request_id).unwrap(), 1);
    assert_eq!(reopened.count(&second.request_id).unwrap(), 1);
}

#[test]
fn manual_clock_controls_deadline_without_wall_time() {
    let mut clock = ManualClock(10);
    clock.advance(5).unwrap();
    assert_eq!(clock.now(), 15);
    let db = TempLedger::new();
    let mut provider = ScriptedFakeProvider::new(
        EffectLedger::open(db.0.clone()).unwrap(),
        [Script::ApplyAndAcknowledge],
    );
    let request = SyntheticRequest {
        request_id: "req_synthetic_4".into(),
        idempotency_key: "idk_synthetic_4".into(),
    };
    assert_eq!(
        provider.invoke(&request, clock, 15).unwrap(),
        SimulatedOutcome::TimedOutBeforeEffect
    );
    assert_eq!(provider.ledger.count(&request.request_id).unwrap(), 0);
}

#[test]
fn fake_provider_rejects_before_effect_without_ledger_write() {
    let db = TempLedger::new();
    let ledger = EffectLedger::open(db.0.clone()).unwrap();
    let mut provider = ScriptedFakeProvider::new(ledger, [Script::RejectBeforeEffect]);
    let request = SyntheticRequest {
        request_id: "req_synthetic_5".into(),
        idempotency_key: "idk_synthetic_5".into(),
    };
    assert_eq!(
        provider.invoke(&request, ManualClock(0), 1).unwrap(),
        SimulatedOutcome::RejectedBeforeEffect
    );
    assert_eq!(provider.ledger.count(&request.request_id).unwrap(), 0);
}

#[test]
fn process_crash_child() {
    let Ok(mode) = std::env::var("SEREA_P8_PROCESS_CRASH_MODE") else {
        return;
    };
    let intent_path = PathBuf::from(std::env::var_os("SEREA_P8_INTENT_PATH").unwrap());
    let effect_path = PathBuf::from(std::env::var_os("SEREA_P8_EFFECT_PATH").unwrap());
    let request = match mode.as_str() {
        "after_intent_before_invoke" => SyntheticRequest {
            request_id: "req_process_preinvoke".into(),
            idempotency_key: "idk_process_preinvoke".into(),
        },
        "after_effect_before_ack" => SyntheticRequest {
            request_id: "req_process_ackloss".into(),
            idempotency_key: "idk_process_ackloss".into(),
        },
        _ => panic!("unknown test child crash mode"),
    };
    IntentLedger::open(intent_path)
        .unwrap()
        .commit(&request)
        .unwrap();

    match mode.as_str() {
        "after_intent_before_invoke" => std::process::exit(73),
        "after_effect_before_ack" => {
            let mut provider = ScriptedFakeProvider::new(
                EffectLedger::open(effect_path).unwrap(),
                [Script::ApplyThenLoseAcknowledgement],
            );
            assert_eq!(
                provider.invoke(&request, ManualClock(10), 20).unwrap(),
                SimulatedOutcome::Ambiguous
            );
            // Abrupt process exit stands in for losing the worker after the
            // fake effect but before the caller persists or receives an ACK.
            std::process::exit(74);
        }
        _ => unreachable!("mode was validated above"),
    }
}

#[test]
fn child_process_termination_preserves_commit_and_effect_ledgers_without_replay() {
    for (mode, expected_exit, request, expected_effects) in [
        (
            "after_intent_before_invoke",
            73,
            SyntheticRequest {
                request_id: "req_process_preinvoke".into(),
                idempotency_key: "idk_process_preinvoke".into(),
            },
            0,
        ),
        (
            "after_effect_before_ack",
            74,
            SyntheticRequest {
                request_id: "req_process_ackloss".into(),
                idempotency_key: "idk_process_ackloss".into(),
            },
            1,
        ),
    ] {
        let db = TempLedger::new();
        drop(IntentLedger::open(db.intent_path()).unwrap());
        drop(EffectLedger::open(db.0.clone()).unwrap());
        let status = Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "p8_synthetic_recovery_tests::process_crash_child",
                "--nocapture",
            ])
            .env("SEREA_P8_PROCESS_CRASH_MODE", mode)
            .env("SEREA_P8_INTENT_PATH", db.intent_path())
            .env("SEREA_P8_EFFECT_PATH", &db.0)
            .status()
            .unwrap();
        assert_eq!(status.code(), Some(expected_exit), "child mode {mode}");

        let reopened_intents = IntentLedger::open(db.intent_path()).unwrap();
        let reopened_effects = EffectLedger::open(db.0.clone()).unwrap();
        assert!(reopened_intents.matches(&request).unwrap());
        assert_eq!(
            reopened_effects.count(&request.request_id).unwrap(),
            expected_effects,
            "child mode {mode}"
        );
        assert_eq!(
            reopened_effects
                .count_pair(&request.request_id, &request.idempotency_key)
                .unwrap(),
            expected_effects,
            "exact RequestId/IDK evidence after child mode {mode}"
        );
        // Recovery is deliberately read-only: no permit or invoke path is
        // reconstructed by opening these ledgers again.
        drop(reopened_intents);
        let recovered_again = EffectLedger::open(db.0.clone()).unwrap();
        assert_eq!(
            recovered_again.count(&request.request_id).unwrap(),
            expected_effects,
            "repeated reopen must not replay child mode {mode}"
        );
    }
}

#[test]
fn proposed_one_shot_permit_is_bound_to_request_step_lease_owner_and_provider_instance() {
    let db = TempLedger::new();
    let request = SyntheticRequest {
        request_id: "req_synthetic_permit".into(),
        idempotency_key: "idk_synthetic_permit".into(),
    };
    let committed_intents = IntentLedger::open(db.intent_path()).unwrap();
    committed_intents.commit(&request).unwrap();
    assert!(committed_intents.matches(&request).unwrap());
    let provider_identity = SyntheticProviderIdentity {
        provider_id: "calendar",
        instance_id: 41,
    };
    let mismatch_cases = [
        (
            "request",
            SyntheticRequest {
                request_id: "req_synthetic_wrong".into(),
                idempotency_key: request.idempotency_key.clone(),
            },
            "step-a",
            7,
            "owner-a",
            provider_identity,
            SyntheticPermitRefusal::RequestMismatch,
        ),
        (
            "idempotency key",
            SyntheticRequest {
                request_id: request.request_id.clone(),
                idempotency_key: "idk_synthetic_wrong".into(),
            },
            "step-a",
            7,
            "owner-a",
            provider_identity,
            SyntheticPermitRefusal::RequestMismatch,
        ),
        (
            "step",
            request.clone(),
            "step-b",
            7,
            "owner-a",
            provider_identity,
            SyntheticPermitRefusal::StepMismatch,
        ),
        (
            "lease generation",
            request.clone(),
            "step-a",
            8,
            "owner-a",
            provider_identity,
            SyntheticPermitRefusal::LeaseGenerationMismatch,
        ),
        (
            "owner",
            request.clone(),
            "step-a",
            7,
            "owner-b",
            provider_identity,
            SyntheticPermitRefusal::OwnerMismatch,
        ),
        (
            "provider instance",
            request.clone(),
            "step-a",
            7,
            "owner-a",
            SyntheticProviderIdentity {
                provider_id: "calendar",
                instance_id: 42,
            },
            SyntheticPermitRefusal::ProviderInstanceMismatch,
        ),
    ];

    for (index, (label, presented_request, step, lease, owner, instance, expected)) in
        mismatch_cases.into_iter().enumerate()
    {
        let mut permit = SyntheticDispatchPermit::new(
            request.clone(),
            "step-a",
            7,
            "owner-a",
            provider_identity,
        );
        let mut fake = ScriptedFakeProvider::new(
            EffectLedger::open(
                db.0.parent()
                    .unwrap()
                    .join(format!("mismatch-{index}.sqlite")),
            )
            .unwrap(),
            [Script::ApplyAndAcknowledge],
        );
        assert_eq!(
            permit.invoke_once(
                SyntheticInvocation {
                    request: &presented_request,
                    step_id: step,
                    lease_generation: lease,
                    owner_id: owner,
                    provider_identity: instance,
                    clock: ManualClock(1),
                    deadline_ms: 10,
                },
                &mut fake,
            ),
            Err(expected),
            "mismatch case {label}"
        );
        assert_eq!(
            fake.ledger.count(&request.request_id).unwrap(),
            0,
            "mismatch case {label} must not create an effect"
        );
    }

    let mut permit =
        SyntheticDispatchPermit::new(request.clone(), "step-a", 7, "owner-a", provider_identity);
    let mut fake = ScriptedFakeProvider::new(
        EffectLedger::open(db.0.parent().unwrap().join("matched.sqlite")).unwrap(),
        [
            Script::ApplyThenLoseAcknowledgement,
            Script::ApplyAndAcknowledge,
        ],
    );
    assert_eq!(
        permit
            .invoke_once(
                SyntheticInvocation {
                    request: &request,
                    step_id: "step-a",
                    lease_generation: 7,
                    owner_id: "owner-a",
                    provider_identity,
                    clock: ManualClock(1),
                    deadline_ms: 10,
                },
                &mut fake,
            )
            .unwrap(),
        SimulatedOutcome::Ambiguous
    );
    assert_eq!(
        permit.invoke_once(
            SyntheticInvocation {
                request: &request,
                step_id: "step-a",
                lease_generation: 7,
                owner_id: "owner-a",
                provider_identity,
                clock: ManualClock(1),
                deadline_ms: 10,
            },
            &mut fake,
        ),
        Err(SyntheticPermitRefusal::AlreadyConsumed)
    );
    assert_eq!(fake.ledger.count(&request.request_id).unwrap(), 1);
}
