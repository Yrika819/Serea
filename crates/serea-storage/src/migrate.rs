use std::collections::BTreeSet;

use rusqlite::{Connection, TransactionBehavior, params};
use serea_protocol::{Digest, EpochMillis};
use sha2::{Digest as _, Sha256};

use crate::StoreError;

/// An immutable embedded migration. Applied versions are never reused or edited.
pub struct Migration {
    /// Contiguous schema version, starting at one.
    pub version: u32,
    /// Unique migration identity.
    pub name: &'static str,
    /// Exact UTF-8 source, including comments, whitespace and trailing newline.
    pub sql: &'static str,
}

/// Ordered migration source catalog. schema_migrations is the only version authority.
pub struct Migrations;
static EMBEDDED: [Migration; 2] = [
    Migration {
        version: 1,
        name: "0001_initial",
        sql: include_str!("../migrations/0001_initial.sql"),
    },
    Migration {
        version: 2,
        name: "0002_event_scheduler",
        sql: include_str!("../migrations/0002_event_scheduler.sql"),
    },
];

impl Migrations {
    /// Latest version understood by this binary.
    pub const LATEST: u32 = 2;

    /// The production migration sources in deterministic application order.
    pub fn embedded() -> &'static [Migration] {
        &EMBEDDED
    }

    /// SHA-256 of the raw migration bytes, NOT SCJ-1 canonical JSON. No trimming,
    /// newline conversion, or other normalization is performed.
    pub fn checksum(sql: &str) -> Digest {
        let hash = Sha256::digest(sql.as_bytes());
        let mut value = String::from("sha256:");
        for byte in hash {
            use std::fmt::Write;
            write!(value, "{byte:02x}").expect("writing to String cannot fail");
        }
        Digest::new(value).expect("SHA-256 encoder produces a protocol Digest")
    }
}

pub(crate) fn validate_catalog(catalog: &[Migration]) -> Result<(), StoreError> {
    if catalog.is_empty() {
        return Err(StoreError::MigrationCatalogInvalid);
    }
    let mut names = BTreeSet::new();
    for (index, migration) in catalog.iter().enumerate() {
        if usize::try_from(migration.version).ok() != index.checked_add(1)
            || migration.name.is_empty()
            || migration.name.as_bytes().contains(&0)
            || migration.sql.trim().is_empty()
            || !names.insert(migration.name)
        {
            return Err(StoreError::MigrationCatalogInvalid);
        }
    }
    Ok(())
}

fn has_authority(conn: &Connection) -> Result<bool, StoreError> {
    conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_schema WHERE type='table' AND name='schema_migrations')",
        [],
        |row| row.get(0),
    )
    .map_err(|error| match error.sqlite_error_code() {
        Some(rusqlite::ErrorCode::NotADatabase) => StoreError::NotSereaStore,
        _ => StoreError::from(error),
    })
}

/// Validate every applied row, not merely MAX(version). A missing authority is
/// allowed only while initializing a fresh connection; an empty catalog is corrupt.
pub(crate) fn inspect(
    conn: &Connection,
    catalog: &[Migration],
    allow_fresh: bool,
) -> Result<usize, StoreError> {
    if !has_authority(conn)? {
        let objects: i64 =
            conn.query_row("SELECT count(*) FROM sqlite_schema", [], |r| r.get(0))?;
        return if allow_fresh && objects == 0 {
            Ok(0)
        } else {
            Err(StoreError::NotSereaStore)
        };
    }
    let invalid = |_| StoreError::MigrationCatalogInvalid;
    let latest = i64::from(
        catalog
            .last()
            .ok_or(StoreError::MigrationCatalogInvalid)?
            .version,
    );
    // One SELECT holds one SQLite read snapshot for both newer-version priority
    // and prefix validation. Delay catalog errors until all versions are seen.
    let mut newer = false;
    let mut failure = None;
    let mut statement = conn
        .prepare(
            "SELECT version,name,checksum,applied_at_ms FROM schema_migrations ORDER BY version",
        )
        .map_err(invalid)?;
    let mut rows = statement.query([]).map_err(invalid)?;
    let mut count = 0;
    while let Some(row) = rows.next().map_err(invalid)? {
        let version = row.get::<_, i64>(0);
        if let Ok(version) = version.as_ref() {
            newer |= *version > latest;
        }
        let validation = (|| {
            let version = version.map_err(invalid)?;
            let name: String = row.get(1).map_err(invalid)?;
            let checksum: String = row.get(2).map_err(invalid)?;
            let time: i64 = row.get(3).map_err(invalid)?;
            let expected = catalog
                .get(count)
                .ok_or(StoreError::MigrationCatalogInvalid)?;
            if version != i64::from(expected.version)
                || name != expected.name
                || EpochMillis::new(time).is_err()
                || Digest::new(&checksum).is_err()
            {
                return Err(StoreError::MigrationCatalogInvalid);
            }
            if checksum != Migrations::checksum(expected.sql).as_str() {
                return Err(StoreError::MigrationChecksumMismatch);
            }
            Ok(())
        })();
        if failure.is_none() {
            failure = validation.err();
        }
        count += 1;
    }
    if newer {
        return Err(StoreError::SchemaTooNew);
    }
    if let Some(error) = failure {
        return Err(error);
    }
    if count == 0 {
        // A created-but-uncommitted first migration is never visible to another
        // connection. A durable empty catalog cannot be a valid applied prefix.
        return Err(StoreError::MigrationCatalogInvalid);
    }
    Ok(count)
}

pub(crate) fn page_check(conn: &Connection, sql: &str) -> Result<(), StoreError> {
    let mut statement = conn
        .prepare(sql)
        .map_err(|_| StoreError::IntegrityCheckFailed)?;
    let mut rows = statement
        .query([])
        .map_err(|_| StoreError::IntegrityCheckFailed)?;
    let mut seen = false;
    let mut healthy = true;
    while let Some(row) = rows.next().map_err(|_| StoreError::IntegrityCheckFailed)? {
        seen = true;
        let value: String = row.get(0).map_err(|_| StoreError::IntegrityCheckFailed)?;
        healthy &= value == "ok";
    }
    if seen && healthy {
        Ok(())
    } else {
        Err(StoreError::IntegrityCheckFailed)
    }
}

pub(crate) fn foreign_key_check(conn: &Connection) -> Result<(), StoreError> {
    let mut statement = conn
        .prepare("PRAGMA foreign_key_check")
        .map_err(|_| StoreError::IntegrityCheckFailed)?;
    if statement
        .query([])
        .map_err(|_| StoreError::IntegrityCheckFailed)?
        .next()
        .map_err(|_| StoreError::IntegrityCheckFailed)?
        .is_some()
    {
        return Err(StoreError::IntegrityCheckFailed);
    }
    Ok(())
}

pub(crate) fn apply(
    conn: &mut Connection,
    catalog: &[Migration],
    applied_at: EpochMillis,
    allow_fresh: bool,
) -> Result<(), StoreError> {
    validate_catalog(catalog)?;
    // Re-read the applied prefix under the writer reservation. Two accepted
    // concurrent openers cannot apply a migration from a stale read snapshot.
    loop {
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let count = inspect(&tx, catalog, allow_fresh)?;
        let Some(migration) = catalog.get(count) else {
            tx.rollback()?;
            return Ok(());
        };
        tx.execute_batch(migration.sql)?;
        tx.execute(
            "INSERT INTO schema_migrations(version,name,checksum,applied_at_ms) VALUES (?1,?2,?3,?4)",
            params![migration.version, migration.name, Migrations::checksum(migration.sql).as_str(), applied_at.get()],
        )?;
        page_check(&tx, "PRAGMA quick_check")?;
        foreign_key_check(&tx)?;
        tx.commit()?;
    }
}
