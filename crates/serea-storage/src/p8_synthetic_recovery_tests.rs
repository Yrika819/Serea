//! Test-only P8C recovery simulator.
//!
//! This deliberately does not implement `CapabilityProvider`, is not wired to
//! TaskEngine, and is unreachable from Host/runtime. It tests the semantics of
//! a scripted effect ledger across independent SQLite reopen; it cannot prove
//! the P8B authority transaction or production provider behavior.

use rusqlite::{Connection, params};
use std::collections::VecDeque;
use std::path::PathBuf;
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
