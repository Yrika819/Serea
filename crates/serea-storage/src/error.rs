use std::fmt;

use serea_protocol::ProtocolError;

/// Payload-free storage failure categories. Neither formatter nor the error
/// source chain exposes SQL, paths, rejected values, or SQLite diagnostics.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum StoreError {
    /// File or connection could not be opened.
    Open,
    /// A SQLite operation failed.
    Sqlite,
    /// A nonempty file lacks Serea migration authority or is not SQLite.
    NotSereaStore,
    /// The file contains a migration newer than this binary understands.
    SchemaTooNew,
    /// Applied migration bytes differ from this binary's embedded source.
    MigrationChecksumMismatch,
    /// Embedded or applied migration identities do not form an ordered prefix.
    MigrationCatalogInvalid,
    /// Page or foreign-key verification failed.
    IntegrityCheckFailed,
    /// SQLite is busy or a TRUNCATE checkpoint did not complete. Retry is allowed.
    Busy,
    /// A SQL constraint refused a write.
    ConstraintViolation,
    /// A connection setting did not read back as required.
    ConnectionPolicy,
    /// Required SQLite version or JSON functions are unavailable.
    UnsupportedSqlite,
    /// A panicking transaction poisoned the connection mutex.
    LockPoisoned,
    /// Original bytes are not a UTF-8 SCJ-1 JSON document.
    CanonicalJson,
    /// No row exists at the exact blob digest and class.
    BlobMissing,
    /// Stored metadata or canonical plaintext integrity is invalid.
    BlobCorrupt,
    /// SECRET/CREDENTIAL cannot use ordinary blob storage.
    ClassRefused,
    /// PRIVATE requires an injected at-rest backend.
    AtRestProtectionUnavailable,
    /// The injected backend refused protection or unprotection.
    AtRestProtectionFailed,
    /// Acquisition encountered current unreleased, unexpired authority.
    LeaseHeld,
    /// The requested generation, binding or guard is not authoritative.
    LeaseFenced,
    /// A matching current guard has expired and cannot renew.
    LeaseExpired,
    /// The durable acquisition budget is exhausted.
    AttemptCeilingReached,
    /// An otherwise eligible acquisition cannot advance positive-u32 generation.
    LeaseGenerationOverflow,
    /// Requested expiry is not later, or release predates acquisition.
    InvalidLeaseInterval,
    /// An audited operation requires the single audit participant.
    AuditRequired,
    /// The audit participant returned an invalid or empty record batch.
    AuditRejected,
    /// A production task transition requires the fixed Event Bus participant.
    EventParticipantRequired,
    /// A requested task is absent.
    TaskNotFound,
    /// A supplied task identity already exists.
    TaskExists,
    /// Durable task/step/provenance data cannot be decoded safely.
    CorruptRow,
    /// A whole operation's expected lifecycle predicate did not match.
    IllegalTaskTransition,
    /// A plan revision is stale or not the contiguous next revision.
    PlanRevisionConflict,
    /// The next revision cannot be represented.
    PlanRevisionOverflow,
    /// A plan would omit an ever-leased step.
    PlanRevisionWouldDropExecutedStep,
    /// A supplied plan is malformed or changes retained step facts.
    InvalidPlan,
    /// A plan does not have an ordinary prefix and VERIFY suffix.
    InvalidPlanLayout,
    /// A plan repeats a step identity.
    DuplicateStepId,
    /// A plan repeats a sequence.
    DuplicateSequence,
    /// A plan repeats a capability key.
    DuplicateIdempotencyKey,
    /// A task mutation supplies a backward or inconsistent timestamp.
    InvalidTimestamp,
    /// Event payload exceeds the frozen 32 KiB canonical byte bound.
    EventPayloadTooLarge,
    /// One transaction attempted to append more than sixteen events.
    EventTransactionLimit,
    InvalidEventReplayPage,
    InvalidRetentionDeleteBatch,
    EventHistoryCorrupt,
    /// Event history has no capacity after eligible retention pruning.
    EventStoreCapacity,
    /// Global event sequence cannot advance without overflowing SQLite's integer domain.
    EventSequenceOverflow,
    /// Event content declares a class that this Store cannot safely persist.
    EventClassRefused,
    ScheduleNotActive,
    ScheduleRevisionConflict,
    SchedulerLeaseFenced,
    StaleSchedulerLease,
    ScheduleOccurrenceNotClaimable,
    ScheduleOccurrenceAlreadyMapped,
    SchedulerLeaseGenerationOverflow,
    ScheduleActiveLimit,
    SchedulePendingOccurrenceLimit,
    ScheduleCommandIdentityConflict,
    InvalidSchedule,
    /// Recovery observation no longer matches durable facts, even within one Tx.
    RecoverySnapshotStale,
    /// Requested recovery action lacks validated durable evidence.
    InvalidRecoveryAction,
    /// A model-call RequestId already identifies a committed provider attempt.
    DuplicateModelRequestId,
    /// A Task already has one in-flight DISPATCH_INTENT model attempt.
    ModelCallInFlight,
    /// The daily spend reservation would exceed the configured cap.
    DailySpendExceeded,
    /// The durable model-call count has reached its configured maximum.
    ModelCallBudgetExceeded,
    /// A model attempt is missing or cannot make the requested terminal transition.
    InvalidModelCallTransition,
    /// A model attempt was not found by RequestId.
    ModelCallNotFound,
    /// Model accounting arithmetic exceeded SQLite's signed INTEGER range.
    ModelAccountingOverflow,
    /// Model attempt or accepted response data class is refused by P4B storage.
    ModelDataClassRefused,
    /// Trustworthy usage exceeds a snapshotted token bound or reservation.
    ModelUsageExceedsBound,
    /// Model attempt facts violate the P4B frozen storage contract.
    InvalidModelCall,
    /// Model usage retention arguments or deletion limits are invalid.
    InvalidModelRetention,
    /// The injected clock refused its reading.
    Clock(ProtocolError),
}

impl StoreError {
    fn category(&self) -> &'static str {
        match self {
            Self::Open => "Open",
            Self::Sqlite => "Sqlite",
            Self::NotSereaStore => "NotSereaStore",
            Self::SchemaTooNew => "SchemaTooNew",
            Self::MigrationChecksumMismatch => "MigrationChecksumMismatch",
            Self::MigrationCatalogInvalid => "MigrationCatalogInvalid",
            Self::IntegrityCheckFailed => "IntegrityCheckFailed",
            Self::Busy => "Busy",
            Self::ConstraintViolation => "ConstraintViolation",
            Self::ConnectionPolicy => "ConnectionPolicy",
            Self::UnsupportedSqlite => "UnsupportedSqlite",
            Self::LockPoisoned => "LockPoisoned",
            Self::CanonicalJson => "CanonicalJson",
            Self::BlobMissing => "BlobMissing",
            Self::BlobCorrupt => "BlobCorrupt",
            Self::ClassRefused => "ClassRefused",
            Self::AtRestProtectionUnavailable => "AtRestProtectionUnavailable",
            Self::AtRestProtectionFailed => "AtRestProtectionFailed",
            Self::LeaseHeld => "LeaseHeld",
            Self::LeaseFenced => "LeaseFenced",
            Self::LeaseExpired => "LeaseExpired",
            Self::AttemptCeilingReached => "AttemptCeilingReached",
            Self::LeaseGenerationOverflow => "LeaseGenerationOverflow",
            Self::InvalidLeaseInterval => "InvalidLeaseInterval",
            Self::AuditRequired => "AuditRequired",
            Self::AuditRejected => "AuditRejected",
            Self::EventParticipantRequired => "EventParticipantRequired",
            Self::TaskNotFound => "TaskNotFound",
            Self::TaskExists => "TaskExists",
            Self::CorruptRow => "CorruptRow",
            Self::IllegalTaskTransition => "IllegalTaskTransition",
            Self::PlanRevisionConflict => "PlanRevisionConflict",
            Self::PlanRevisionOverflow => "PlanRevisionOverflow",
            Self::PlanRevisionWouldDropExecutedStep => "PlanRevisionWouldDropExecutedStep",
            Self::InvalidPlan => "InvalidPlan",
            Self::InvalidPlanLayout => "InvalidPlanLayout",
            Self::DuplicateStepId => "DuplicateStepId",
            Self::DuplicateSequence => "DuplicateSequence",
            Self::DuplicateIdempotencyKey => "DuplicateIdempotencyKey",
            Self::InvalidTimestamp => "InvalidTimestamp",
            Self::EventPayloadTooLarge => "EventPayloadTooLarge",
            Self::EventTransactionLimit => "EventTransactionLimit",
            Self::InvalidEventReplayPage => "InvalidEventReplayPage",
            Self::InvalidRetentionDeleteBatch => "InvalidRetentionDeleteBatch",
            Self::EventHistoryCorrupt => "EventHistoryCorrupt",
            Self::EventStoreCapacity => "EventStoreCapacity",
            Self::EventSequenceOverflow => "EventSequenceOverflow",
            Self::EventClassRefused => "EventClassRefused",
            Self::ScheduleNotActive => "ScheduleNotActive",
            Self::ScheduleRevisionConflict => "ScheduleRevisionConflict",
            Self::SchedulerLeaseFenced => "SchedulerLeaseFenced",
            Self::StaleSchedulerLease => "StaleSchedulerLease",
            Self::ScheduleOccurrenceNotClaimable => "ScheduleOccurrenceNotClaimable",
            Self::ScheduleOccurrenceAlreadyMapped => "ScheduleOccurrenceAlreadyMapped",
            Self::SchedulerLeaseGenerationOverflow => "SchedulerLeaseGenerationOverflow",
            Self::ScheduleActiveLimit => "ScheduleActiveLimit",
            Self::SchedulePendingOccurrenceLimit => "SchedulePendingOccurrenceLimit",
            Self::ScheduleCommandIdentityConflict => "ScheduleCommandIdentityConflict",
            Self::InvalidSchedule => "InvalidSchedule",
            Self::RecoverySnapshotStale => "RecoverySnapshotStale",
            Self::InvalidRecoveryAction => "InvalidRecoveryAction",
            Self::DuplicateModelRequestId => "DuplicateModelRequestId",
            Self::ModelCallInFlight => "ModelCallInFlight",
            Self::DailySpendExceeded => "DailySpendExceeded",
            Self::ModelCallBudgetExceeded => "ModelCallBudgetExceeded",
            Self::InvalidModelCallTransition => "InvalidModelCallTransition",
            Self::ModelCallNotFound => "ModelCallNotFound",
            Self::ModelAccountingOverflow => "ModelAccountingOverflow",
            Self::ModelDataClassRefused => "ModelDataClassRefused",
            Self::ModelUsageExceedsBound => "ModelUsageExceedsBound",
            Self::InvalidModelCall => "InvalidModelCall",
            Self::InvalidModelRetention => "InvalidModelRetention",
            Self::Clock(_) => "Clock",
        }
    }
}

impl fmt::Display for StoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.category())
    }
}
impl fmt::Debug for StoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.category())
    }
}
impl std::error::Error for StoreError {}

impl From<rusqlite::Error> for StoreError {
    fn from(error: rusqlite::Error) -> Self {
        match error.sqlite_error_code() {
            Some(rusqlite::ErrorCode::DatabaseBusy | rusqlite::ErrorCode::DatabaseLocked) => {
                Self::Busy
            }
            Some(rusqlite::ErrorCode::ConstraintViolation) => Self::ConstraintViolation,
            _ => Self::Sqlite,
        }
    }
}
