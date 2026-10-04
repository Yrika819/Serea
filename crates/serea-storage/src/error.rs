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
