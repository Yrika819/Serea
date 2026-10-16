//! Deterministic in-memory Forget state-machine test double. This is not durable.
use std::collections::BTreeSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LedgerState {
    Empty,
    Partial,
    Intent,
    Applied,
    Finalized,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Phase {
    Ready,
    Authorized,
    Quarantined,
    Complete,
}
#[derive(Debug)]
struct Model {
    visible: BTreeSet<(&'static str, &'static str)>,
    ledger: LedgerState,
    phase: Phase,
    generation: u64,
    response_sent: bool,
}
impl Model {
    fn initial() -> Self {
        Self {
            visible: [("owner-a", "m1"), ("owner-a", "m2"), ("owner-b", "other")].into(),
            ledger: LedgerState::Empty,
            phase: Phase::Ready,
            generation: 0,
            response_sent: false,
        }
    }
    fn crash_at(&mut self, point: CrashPoint) {
        match point {
            CrashPoint::BeforeAuthorization => {}
            CrashPoint::AfterAuthorization | CrashPoint::BeforeIntent => {
                self.phase = Phase::Authorized
            }
            CrashPoint::DuringChunkAppend | CrashPoint::AfterAuthorizationBeforeIntent => {
                self.phase = Phase::Quarantined;
                self.ledger = LedgerState::Partial;
            }
            CrashPoint::AfterCompleteIntent
            | CrashPoint::DuringPrimaryTransaction
            | CrashPoint::DuringFtsUpdate => {
                self.phase = Phase::Quarantined;
                self.ledger = LedgerState::Intent;
            }
            CrashPoint::AfterPrimaryCommit
            | CrashPoint::BeforeLedgerFinalization
            | CrashPoint::DuringBlobGc => {
                self.visible.retain(|(owner, _)| *owner != "owner-a");
                self.phase = Phase::Quarantined;
                self.ledger = LedgerState::Applied;
                self.generation = 1;
            }
            CrashPoint::AfterFinalizationBeforeResponse => {
                self.visible.retain(|(owner, _)| *owner != "owner-a");
                self.phase = Phase::Complete;
                self.ledger = LedgerState::Finalized;
                self.generation = 1;
            }
            CrashPoint::DuringBackupExport | CrashPoint::DuringOldSnapshotRestore => {
                self.phase = Phase::Quarantined;
                self.ledger = LedgerState::Finalized;
                self.generation = 0;
            }
            CrashPoint::DuringConcurrentExtraction
            | CrashPoint::DuringRecoveryReplay
            | CrashPoint::StorageExhaustion => {
                self.phase = Phase::Quarantined;
                self.ledger = LedgerState::Partial;
            }
        }
    }
    fn recover(&mut self) {
        match self.ledger {
            LedgerState::Empty => {
                if self.phase == Phase::Authorized {
                    self.phase = Phase::Ready;
                }
            }
            LedgerState::Partial => self.phase = Phase::Quarantined,
            LedgerState::Intent => {
                self.visible.retain(|(owner, _)| *owner != "owner-a");
                self.generation = 1;
                self.ledger = LedgerState::Applied;
                self.phase = Phase::Quarantined;
            }
            LedgerState::Applied => {
                self.ledger = LedgerState::Finalized;
                self.phase = Phase::Complete;
            }
            LedgerState::Finalized if self.generation == 0 => self.phase = Phase::Quarantined,
            LedgerState::Finalized => self.phase = Phase::Complete,
        }
    }
    fn can_read(&self) -> bool {
        self.phase == Phase::Ready || self.phase == Phase::Complete
    }
    fn can_extract(&self) -> bool {
        self.phase == Phase::Ready
    }
    fn can_commit_extraction(&self, captured_generation: u64) -> bool {
        self.phase == Phase::Ready && self.generation == captured_generation
    }
    fn can_ack(&self) -> bool {
        self.ledger == LedgerState::Finalized && self.phase == Phase::Complete
    }
}
#[derive(Debug, Clone, Copy)]
enum CrashPoint {
    BeforeAuthorization,
    AfterAuthorization,
    BeforeIntent,
    AfterAuthorizationBeforeIntent,
    DuringChunkAppend,
    AfterCompleteIntent,
    DuringPrimaryTransaction,
    AfterPrimaryCommit,
    BeforeLedgerFinalization,
    AfterFinalizationBeforeResponse,
    DuringFtsUpdate,
    DuringBlobGc,
    DuringBackupExport,
    DuringOldSnapshotRestore,
    DuringConcurrentExtraction,
    DuringRecoveryReplay,
    StorageExhaustion,
}
#[test]
fn crash_matrix_never_acknowledges_partial_intent_and_replay_is_idempotent() {
    use CrashPoint::*;
    let cases = [
        BeforeAuthorization,
        AfterAuthorization,
        BeforeIntent,
        AfterAuthorizationBeforeIntent,
        DuringChunkAppend,
        AfterCompleteIntent,
        DuringPrimaryTransaction,
        AfterPrimaryCommit,
        BeforeLedgerFinalization,
        AfterFinalizationBeforeResponse,
        DuringFtsUpdate,
        DuringBlobGc,
        DuringBackupExport,
        DuringOldSnapshotRestore,
        DuringConcurrentExtraction,
        DuringRecoveryReplay,
        StorageExhaustion,
    ];
    for point in cases {
        let mut m = Model::initial();
        m.crash_at(point);
        let expected = match point {
            BeforeAuthorization => (LedgerState::Empty, Phase::Ready, 0, true, true, false),
            AfterAuthorization | BeforeIntent => {
                (LedgerState::Empty, Phase::Authorized, 0, true, false, false)
            }
            AfterAuthorizationBeforeIntent
            | DuringChunkAppend
            | DuringConcurrentExtraction
            | DuringRecoveryReplay
            | StorageExhaustion => (
                LedgerState::Partial,
                Phase::Quarantined,
                0,
                true,
                false,
                false,
            ),
            AfterCompleteIntent | DuringPrimaryTransaction | DuringFtsUpdate => (
                LedgerState::Intent,
                Phase::Quarantined,
                0,
                true,
                false,
                false,
            ),
            AfterPrimaryCommit | BeforeLedgerFinalization | DuringBlobGc => (
                LedgerState::Applied,
                Phase::Quarantined,
                1,
                false,
                false,
                false,
            ),
            AfterFinalizationBeforeResponse => (
                LedgerState::Finalized,
                Phase::Complete,
                1,
                false,
                true,
                true,
            ),
            DuringBackupExport | DuringOldSnapshotRestore => (
                LedgerState::Finalized,
                Phase::Quarantined,
                0,
                true,
                false,
                false,
            ),
        };
        assert_eq!(m.ledger, expected.0, "ledger at {point:?}");
        assert_eq!(m.phase, expected.1, "phase at {point:?}");
        assert_eq!(m.generation, expected.2, "generation at {point:?}");
        assert_eq!(
            m.visible.contains(&("owner-a", "m1")),
            expected.3,
            "Owner A visibility at {point:?}"
        );
        assert_eq!(m.can_read(), expected.4, "read barrier at {point:?}");
        assert_eq!(m.can_extract(), matches!(point, BeforeAuthorization));
        assert_eq!(m.can_ack(), expected.5, "acknowledgment at {point:?}");
        assert!(
            m.visible.contains(&("owner-b", "other")),
            "another Owner remains isolated at {point:?}"
        );
        assert!(
            !m.response_sent,
            "the deterministic harness never claims a network response"
        );
        m.recover();
        let after = m.visible.clone();
        m.recover();
        let recovered = (
            m.ledger,
            m.phase,
            m.generation,
            m.can_read(),
            m.can_extract(),
            m.can_ack(),
        );
        m.recover();
        assert_eq!(m.visible, after, "replay changed state at {point:?}");
        assert_eq!(
            (
                m.ledger,
                m.phase,
                m.generation,
                m.can_read(),
                m.can_extract(),
                m.can_ack()
            ),
            recovered,
            "replay state must be idempotent at {point:?}"
        );
        assert!(m.visible.contains(&("owner-b", "other")));
        if matches!(
            point,
            AfterCompleteIntent
                | DuringPrimaryTransaction
                | DuringFtsUpdate
                | AfterPrimaryCommit
                | BeforeLedgerFinalization
                | DuringBlobGc
                | AfterFinalizationBeforeResponse
        ) {
            assert!(!m.visible.iter().any(|(owner, _)| *owner == "owner-a"));
            assert!(
                m.can_ack(),
                "recovered complete intent should become reportable at {point:?}"
            );
            assert!(
                !m.can_commit_extraction(0),
                "stale extraction must not resurrect forgotten content at {point:?}"
            );
        } else {
            assert!(
                !m.can_ack(),
                "incomplete or unverifiable Forget must not be reportable at {point:?}"
            );
        }
    }
}
#[test]
fn old_primary_snapshot_with_new_ledger_stays_quarantined_and_full_rollback_is_not_detected() {
    let mut restored = Model::initial();
    restored.crash_at(CrashPoint::DuringOldSnapshotRestore);
    restored.recover();
    assert!(!restored.can_read());
    assert!(!restored.can_extract());
    assert!(!restored.can_ack());
    assert!(restored.visible.contains(&("owner-b", "other")));
    // The fake cannot detect a rollback of both independent-looking in-memory values.
    let fully_rolled_back = Model::initial();
    assert!(fully_rolled_back.can_read());
}
