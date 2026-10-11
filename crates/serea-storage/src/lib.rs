//! SQLite foundation, classified JSON blobs and durable lease authority (P2E).
//!
//! File-backed stores use WAL, FULL synchronization, explicit foreign keys and
//! a bounded busy timeout. The separate memory constructor is test-only in its
//! guarantees, not durable. Blob identity is canonical plaintext SHA-256 plus
//! class. PRIVATE blobs fail closed without injected protection; P2D ships no
//! real backend and claims no complete PRIVATE task/row support. SECRET and
//! CREDENTIAL are refused. Transaction-scoped acquisition/renewal/release use
//! SQLite authority. P2F adds atomic begin/known outcome transitions with result,
//! receipt and journal fencing. The single SQL-free audit port accepts actual-write
//! facts; task-engine owns journal semantics and recovery orchestration. Recovery
//! inspection and fenced repairs remain storage-owned. P3 event append, replay,
//! and retention use typed Store operations over the same transaction authority.

// P2H: the crate has no unsafe code, and the P2H fault seam adds none. Declaring
// it here makes that a compile error rather than a review comment, matching
// protocol, task-engine and testkit.
#![forbid(unsafe_code)]

mod audit;
mod blob;
mod capability_registry;
#[cfg(test)]
mod capability_registry_tests;
mod classify;
mod error;
mod event;
mod lease;
mod lifecycle;
mod migrate;
#[cfg(test)]
mod migration_0002_tests;
#[cfg(test)]
mod migration_0003_tests;
#[cfg(test)]
mod migration_0004_tests;
#[cfg(test)]
mod migration_0005_tests;
mod model_accounting;
#[cfg(test)]
mod model_call_tests;
#[cfg(test)]
mod model_cost_tests;
mod recovery;
mod scheduler;
mod store;
mod task;
mod tx;

// P2H: the crash/fault seam is absent from the normal/default build. The
// workspace requests its explicitly test-oriented feature only through the
// test-target dev-dependency edges; any build that opts into the feature is
// seam-bearing and is outside the default-production exclusion proof.
#[cfg(feature = "p2h-fault-injection")]
pub mod fault;

pub use audit::{
    AuditOperation, DurableTransition, EventDraft, EventParticipant, JournalKind, JournalRecord,
    JournalRecords, TaskAuditParticipant,
};
pub use blob::BlobRef;
pub use capability_registry::{
    CapabilityOverlay, CapabilityOverlayState, CapabilityPlanBindingDraft, DescriptorRevision,
    DescriptorRevisionDraft, GenerationMember, GenerationMemberDraft, RegistryGeneration,
    RegistryGenerationDraft, StepCapabilityBinding,
};
pub use classify::{AtRestProtection, AtRestProtectionError};
pub use error::StoreError;
pub use event::{
    EventReplayPage, EventRetentionReport, MAX_EVENT_REPLAY_PAGE, MAX_RETENTION_DELETE_BATCH,
    ReplayItem,
};
pub use lease::{LeaseGuard, StepCommit, StepFailure, StepOutcome, TransitionContext};
pub use lifecycle::{CancellationOutcome, DeletionOutcome};
pub use migrate::{Migration, Migrations};
pub use model_accounting::{
    MAX_MODEL_CALLS_PER_TASK, MAX_MODEL_RESPONSE_BYTES, MAX_MODEL_RETENTION_BATCH,
    MAX_MODEL_TURNS_PER_TASK, ModelAccountingError, ModelAttemptRelationKind, ModelAttemptState,
    ModelCallAttempt, ModelCallAttemptDraft, ModelCallCompletion, ModelDeploymentClass,
    ModelFailureUsage, ModelPriceSnapshot, ModelResponseStorage, ModelUsageRecord, UsdMicros,
    UtcAccountingDay, calculate_cost_usd_micros, calculate_reservation_usd_micros,
    validate_price_snapshot,
};
pub use recovery::{
    RecoveryAction, RecoveryApplied, RecoveryAuthority, RecoveryPass, RecoveryReceiptRepair,
    RecoverySnapshot, RecoveryStep,
};
pub(crate) use scheduler::validate_scheduler_integrity as validate_p3_storage_integrity;
pub use scheduler::{
    ApprovalLifecycleOutcome, ApprovalLifecycleWake, DeviceResumeWait, DeviceSessionResumeWake,
    MissedOccurrencePolicy, RecoverableScheduleOccurrence, ScheduleCommandOutcome,
    ScheduleCommandState, ScheduleDraft, ScheduleDueOccurrence, ScheduleOccurrenceDraft,
    ScheduleOccurrenceLease, ScheduleOccurrenceWork, ScheduleOwnerKind, ScheduleSnapshot,
    ScheduleStateCommand, ScheduleStateCommandRequest, ScheduleTaskProvenance, ScheduleTriggerKind,
    SchedulerConsumerLease, SchedulerCursorSnapshot,
};
pub use store::{CheckpointOutcome, Store};
pub use task::{PlanRevisionSnapshot, PlanWrite, StepInput, StepSnapshot, TaskSnapshot};
pub use tx::Tx;

#[cfg(test)]
mod blob_tests;
#[cfg(test)]
mod foundation_tests;

#[cfg(test)]
mod protection_tests;
#[cfg(test)]
mod scheduler_tests;
#[cfg(test)]
mod schema_tests;
