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
//! inspection and fenced repairs remain storage-owned; no event runtime is implemented.

mod audit;
mod blob;
mod classify;
mod error;
mod lease;
mod lifecycle;
mod migrate;
mod recovery;
mod store;
mod task;
mod tx;

pub use audit::{
    AuditOperation, DurableTransition, JournalKind, JournalRecord, JournalRecords,
    TaskAuditParticipant,
};
pub use blob::BlobRef;
pub use classify::{AtRestProtection, AtRestProtectionError};
pub use error::StoreError;
pub use lease::{LeaseGuard, StepCommit, StepFailure, StepOutcome, TransitionContext};
pub use lifecycle::{CancellationOutcome, DeletionOutcome};
pub use migrate::{Migration, Migrations};
pub use recovery::{
    RecoveryAction, RecoveryApplied, RecoveryAuthority, RecoveryPass, RecoveryReceiptRepair,
    RecoverySnapshot, RecoveryStep,
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
mod schema_tests;
