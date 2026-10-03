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
