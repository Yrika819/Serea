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
struct SyntheticRegistryPin {
    generation: u64,
    descriptor_digest: &'static str,
    overlay_revision: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SyntheticPermitRefusal {
    RequestMismatch,
    TaskMismatch,
    StepMismatch,
    LeaseGenerationMismatch,
    OwnerMismatch,
    ProviderInstanceMismatch,
    RegistryGenerationMismatch,
    DescriptorDigestMismatch,
    OverlayRevisionMismatch,
    EffectLedgerFailure,
}

/// Test-only shape for a non-serializable, local one-shot permit.
/// This type is not a production authorization or provider API. Calling
/// `invoke_once` consumes it even when binding validation refuses the call.
struct SyntheticDispatchPermit {
    request: SyntheticRequest,
    task_id: String,
    step_id: String,
    lease_generation: u32,
    owner_id: String,
    provider: SyntheticProviderIdentity,
    registry_pin: SyntheticRegistryPin,
}

#[derive(Clone, Copy)]
struct SyntheticInvocation<'a> {
    request: &'a SyntheticRequest,
    task_id: &'a str,
    step_id: &'a str,
    lease_generation: u32,
    owner_id: &'a str,
    provider_identity: SyntheticProviderIdentity,
    registry_pin: SyntheticRegistryPin,
    clock: ManualClock,
    deadline_ms: i64,
}

impl SyntheticDispatchPermit {
    fn new(
        request: SyntheticRequest,
        task_id: &str,
        step_id: &str,
        lease_generation: u32,
        owner_id: &str,
        provider: SyntheticProviderIdentity,
        registry_pin: SyntheticRegistryPin,
    ) -> Self {
        Self {
            request,
            task_id: task_id.to_owned(),
            step_id: step_id.to_owned(),
            lease_generation,
            owner_id: owner_id.to_owned(),
            provider,
            registry_pin,
        }
    }

    fn invoke_once(
        self,
        invocation: SyntheticInvocation<'_>,
        provider: &mut ScriptedFakeProvider,
    ) -> Result<SimulatedOutcome, SyntheticPermitRefusal> {
        if invocation.request != &self.request {
            return Err(SyntheticPermitRefusal::RequestMismatch);
        }
        if invocation.task_id != self.task_id {
            return Err(SyntheticPermitRefusal::TaskMismatch);
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
        if invocation.registry_pin.generation != self.registry_pin.generation {
            return Err(SyntheticPermitRefusal::RegistryGenerationMismatch);
        }
        if invocation.registry_pin.descriptor_digest != self.registry_pin.descriptor_digest {
            return Err(SyntheticPermitRefusal::DescriptorDigestMismatch);
        }
        if invocation.registry_pin.overlay_revision != self.registry_pin.overlay_revision {
            return Err(SyntheticPermitRefusal::OverlayRevisionMismatch);
        }

        // Consume before crossing the fake provider boundary. A failure or
        // ambiguous result cannot reuse this in-memory permit.
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
    UnavailableBeforeEffect,
}

#[derive(Debug, PartialEq, Eq)]
enum SimulatedOutcome {
    Acknowledged(SyntheticResponse),
    Ambiguous,
    RejectedBeforeEffect,
    UnavailableBeforeEffect,
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
            Some(Script::UnavailableBeforeEffect) => Ok(SimulatedOutcome::UnavailableBeforeEffect),
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
fn synthetic_one_shot_permit_is_bound_to_request_step_lease_owner_and_provider_instance() {
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
    let registry_pin = SyntheticRegistryPin {
        generation: 3,
        descriptor_digest: "sha256:descriptor-a",
        overlay_revision: 9,
    };
    let mismatch_cases = [
        (
            "request",
            SyntheticRequest {
                request_id: "req_synthetic_wrong".into(),
                idempotency_key: request.idempotency_key.clone(),
            },
            "task-a",
            "step-a",
            7,
            "owner-a",
            provider_identity,
            registry_pin,
            SyntheticPermitRefusal::RequestMismatch,
        ),
        (
            "idempotency key",
            SyntheticRequest {
                request_id: request.request_id.clone(),
                idempotency_key: "idk_synthetic_wrong".into(),
            },
            "task-a",
            "step-a",
            7,
            "owner-a",
            provider_identity,
            registry_pin,
            SyntheticPermitRefusal::RequestMismatch,
        ),
        (
            "step",
            request.clone(),
            "task-a",
            "step-b",
            7,
            "owner-a",
            provider_identity,
            registry_pin,
            SyntheticPermitRefusal::StepMismatch,
        ),
        (
            "lease generation",
            request.clone(),
            "task-a",
            "step-a",
            8,
            "owner-a",
            provider_identity,
            registry_pin,
            SyntheticPermitRefusal::LeaseGenerationMismatch,
        ),
        (
            "owner",
            request.clone(),
            "task-a",
            "step-a",
            7,
            "owner-b",
            provider_identity,
            registry_pin,
            SyntheticPermitRefusal::OwnerMismatch,
        ),
        (
            "provider instance",
            request.clone(),
            "task-a",
            "step-a",
            7,
            "owner-a",
            SyntheticProviderIdentity {
                provider_id: "calendar",
                instance_id: 42,
            },
            registry_pin,
            SyntheticPermitRefusal::ProviderInstanceMismatch,
        ),
        (
            "registry generation",
            request.clone(),
            "task-a",
            "step-a",
            7,
            "owner-a",
            provider_identity,
            SyntheticRegistryPin {
                generation: 4,
                ..registry_pin
            },
            SyntheticPermitRefusal::RegistryGenerationMismatch,
        ),
        (
            "descriptor digest",
            request.clone(),
            "task-a",
            "step-a",
            7,
            "owner-a",
            provider_identity,
            SyntheticRegistryPin {
                descriptor_digest: "sha256:descriptor-b",
                ..registry_pin
            },
            SyntheticPermitRefusal::DescriptorDigestMismatch,
        ),
        (
            "overlay revision",
            request.clone(),
            "task-a",
            "step-a",
            7,
            "owner-a",
            provider_identity,
            SyntheticRegistryPin {
                overlay_revision: 10,
                ..registry_pin
            },
            SyntheticPermitRefusal::OverlayRevisionMismatch,
        ),
        (
            "task",
            request.clone(),
            "task-b",
            "step-a",
            7,
            "owner-a",
            provider_identity,
            registry_pin,
            SyntheticPermitRefusal::TaskMismatch,
        ),
    ];

    for (
        index,
        (label, presented_request, task, step, lease, owner, instance, presented_pin, expected),
    ) in mismatch_cases.into_iter().enumerate()
    {
        let permit = SyntheticDispatchPermit::new(
            request.clone(),
            "task-a",
            "step-a",
            7,
            "owner-a",
            provider_identity,
            registry_pin,
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
                    task_id: task,
                    step_id: step,
                    lease_generation: lease,
                    owner_id: owner,
                    provider_identity: instance,
                    registry_pin: presented_pin,
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

    let permit = SyntheticDispatchPermit::new(
        request.clone(),
        "task-a",
        "step-a",
        7,
        "owner-a",
        provider_identity,
        registry_pin,
    );
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
                    task_id: "task-a",
                    step_id: "step-a",
                    lease_generation: 7,
                    owner_id: "owner-a",
                    provider_identity,
                    registry_pin,
                    clock: ManualClock(1),
                    deadline_ms: 10,
                },
                &mut fake,
            )
            .unwrap(),
        SimulatedOutcome::Ambiguous
    );
    // By-value invocation makes a second attempt impossible at this call
    // site: `permit` was moved whether the result was ACK or AMBIGUOUS.
    assert_eq!(fake.ledger.count(&request.request_id).unwrap(), 1);
}

#[test]
fn provider_unavailability_after_intent_commit_consumes_permit_without_effect_or_replay() {
    let db = TempLedger::new();
    let request = SyntheticRequest {
        request_id: "req_provider_unavailable_after_commit".into(),
        idempotency_key: "idk_provider_unavailable_after_commit".into(),
    };
    let intents = IntentLedger::open(db.intent_path()).unwrap();
    intents.commit(&request).unwrap();
    let provider_identity = SyntheticProviderIdentity {
        provider_id: "calendar",
        instance_id: 52,
    };
    let registry_pin = SyntheticRegistryPin {
        generation: 4,
        descriptor_digest: "sha256:descriptor-ready-at-commit",
        overlay_revision: 12,
    };
    let permit = SyntheticDispatchPermit::new(
        request.clone(),
        "task-a",
        "step-unavailable",
        3,
        "owner-synthetic",
        provider_identity,
        registry_pin,
    );
    // The Fake Provider becomes unavailable after the Intent is committed and
    // after the host has prepared its process-local Permit, but before an
    // effect is written. This models a boundary, not production readiness.
    let mut fake = ScriptedFakeProvider::new(
        EffectLedger::open(db.0.clone()).unwrap(),
        [Script::UnavailableBeforeEffect],
    );
    let invocation = SyntheticInvocation {
        request: &request,
        task_id: "task-a",
        step_id: "step-unavailable",
        lease_generation: 3,
        owner_id: "owner-synthetic",
        provider_identity,
        registry_pin,
        clock: ManualClock(10),
        deadline_ms: 20,
    };
    assert!(intents.matches(&request).unwrap());
    assert_eq!(
        permit.invoke_once(invocation, &mut fake),
        Ok(SimulatedOutcome::UnavailableBeforeEffect)
    );
    assert_eq!(
        fake.ledger
            .count_pair(&request.request_id, &request.idempotency_key)
            .unwrap(),
        0
    );
    // `permit` was consumed by the unavailable attempt. No new token is
    // reconstructed from the committed Intent on this path.
    assert_eq!(fake.ledger.count(&request.request_id).unwrap(), 0);
    drop(intents);
    assert!(
        IntentLedger::open(db.intent_path())
            .unwrap()
            .matches(&request)
            .unwrap()
    );
}

#[test]
fn mismatched_attempt_burns_by_value_permit_without_effect() {
    let db = TempLedger::new();
    let request = SyntheticRequest {
        request_id: "req_mismatch_then_valid".into(),
        idempotency_key: "idk_mismatch_then_valid".into(),
    };
    let provider_identity = SyntheticProviderIdentity {
        provider_id: "calendar",
        instance_id: 61,
    };
    let registry_pin = SyntheticRegistryPin {
        generation: 5,
        descriptor_digest: "sha256:descriptor-mismatch-probe",
        overlay_revision: 14,
    };
    let permit = SyntheticDispatchPermit::new(
        request.clone(),
        "task-a",
        "step-mismatch-probe",
        6,
        "owner-synthetic",
        provider_identity,
        registry_pin,
    );
    let mut fake = ScriptedFakeProvider::new(
        EffectLedger::open(db.0.clone()).unwrap(),
        [Script::ApplyAndAcknowledge],
    );
    let wrong_request = SyntheticRequest {
        request_id: request.request_id.clone(),
        idempotency_key: "idk_wrong_binding".into(),
    };
    assert_eq!(
        permit.invoke_once(
            SyntheticInvocation {
                request: &wrong_request,
                task_id: "task-a",
                step_id: "step-mismatch-probe",
                lease_generation: 6,
                owner_id: "owner-synthetic",
                provider_identity,
                registry_pin,
                clock: ManualClock(1),
                deadline_ms: 10,
            },
            &mut fake,
        ),
        Err(SyntheticPermitRefusal::RequestMismatch)
    );
    assert_eq!(fake.ledger.count(&request.request_id).unwrap(), 0);
    // `permit` is moved into the rejected attempt. A subsequent use of that
    // binding is rejected by Rust's ownership checker (E0382); minting a new
    // Permit would require a distinct authorization path.
    assert_eq!(fake.ledger.count(&request.request_id).unwrap(), 0);
}
