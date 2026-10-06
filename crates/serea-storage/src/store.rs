use std::path::Path;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use rusqlite::{Connection, OpenFlags, TransactionBehavior};
use serea_protocol::Clock;

use crate::{AtRestProtection, Migrations, StoreError, Tx, migrate};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Profile {
    File,
    Memory,
}

/// Explicit close-preparation outcome. A memory store has no WAL to checkpoint.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckpointOutcome {
    /// TRUNCATE completed, with no busy or uncheckpointed frames.
    Complete,
    /// In-memory profile: no durability or checkpoint guarantee applies.
    NotApplicable,
}

/// Single-connection SQLite foundation, serialized by a connection mutex.
/// No clock borrow is retained. Blobs and atomic P2F begin/outcome transitions
/// require an opaque Tx. Optional PRIVATE blob protection is
/// owned, not borrowed, and is not complete PRIVATE task/row protection.
///
/// Do not call Store methods again from a transact closure: the connection lock
/// is already held. Panic rolls back the SQL transaction and poisons the mutex.
///
/// External callers cannot acquire the raw SQLite connection:
/// ```compile_fail
/// use serea_storage::Store;
/// fn bypass(store: &Store) { let _ = &store.conn; }
/// ```
/// Nor can they execute arbitrary SQL through Store:
/// ```compile_fail
/// use serea_storage::Store;
/// fn bypass(store: &Store) { store.execute_batch("DELETE FROM tasks").unwrap(); }
/// ```
pub struct Store {
    pub(crate) conn: Mutex<Connection>,
    profile: Profile,
    protection: Option<Arc<dyn AtRestProtection>>,
}

impl Store {
    /// Opens a file-backed WAL/FULL store. Only absent or zero-length files may
    /// initialize. Nonempty files undergo read-only identity/catalog/integrity
    /// preflight before any persistent connection setting or migration write.
    /// SQLite may create transient WAL sidecars even during read-only inspection.
    pub fn open(path: &Path, clock: &dyn Clock) -> Result<Self, StoreError> {
        let catalog = Migrations::embedded();
        migrate::validate_catalog(catalog)?;
        let now = clock.now_ms().map_err(StoreError::Clock)?;
        let fresh = match std::fs::metadata(path) {
            Ok(metadata) => {
                if !metadata.is_file() {
                    return Err(StoreError::Open);
                }
                metadata.len() == 0
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => true,
            Err(_) => return Err(StoreError::Open),
        };
        if !fresh {
            let conn = Connection::open_with_flags(
                path,
                OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
            )
            .map_err(|_| StoreError::Open)?;
            conn.busy_timeout(Duration::from_millis(5000))?;
            migrate::inspect(&conn, catalog, false)?;
            migrate::page_check(&conn, "PRAGMA quick_check")?;
        }
        let mut conn = Connection::open_with_flags(
            path,
            OpenFlags::SQLITE_OPEN_READ_WRITE
                | OpenFlags::SQLITE_OPEN_CREATE
                | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )
        .map_err(|_| StoreError::Open)?;
        conn.busy_timeout(Duration::from_millis(5000))?;
        // Revalidate on the actual connection before changing journal mode.
        migrate::inspect(&conn, catalog, fresh)?;
        migrate::page_check(&conn, "PRAGMA quick_check")?;
        if fresh {
            // Do not publish a nonempty WAL header with no committed migration
            // authority. Bootstrap under SQLite's atomic rollback journal first;
            // concurrent readers see either the zero-length file, a busy lock,
            // or the committed catalog. FULL/FK/timeout apply during bootstrap.
            configure_common(&conn, Profile::File)?;
            migrate::apply(&mut conn, catalog, now, true)?;
            configure_file(&conn)?;
        } else {
            configure_file(&conn)?;
            migrate::apply(&mut conn, catalog, now, false)?;
        }
        migrate::page_check(&conn, "PRAGMA quick_check")?;
        Ok(Self {
            conn: Mutex::new(conn),
            profile: Profile::File,
            protection: None,
        })
    }

    /// Pure test profile: memory journal, explicit FK/timeout settings. Not
    /// ADR-0005 durable: no WAL, restart/reopen, crash, or fsync guarantee.
    pub fn open_in_memory(clock: &dyn Clock) -> Result<Self, StoreError> {
        let catalog = Migrations::embedded();
        migrate::validate_catalog(catalog)?;
        let now = clock.now_ms().map_err(StoreError::Clock)?;
        let mut conn = Connection::open_in_memory().map_err(|_| StoreError::Open)?;
        configure(&conn, Profile::Memory)?;
        migrate::apply(&mut conn, catalog, now, true)?;
        Ok(Self {
            conn: Mutex::new(conn),
            profile: Profile::Memory,
            protection: None,
        })
    }

    /// Opens through the identical P2C preflight/policy path, then retains an
    /// owned PRIVATE blob backend. No backend is invoked while opening.
    /// Backend injection alone is not a claim of cryptographic quality or
    /// protected ordinary task rows. P2D ships no production backend.
    pub fn open_with_protection(
        path: &Path,
        clock: &dyn Clock,
        protection: Arc<dyn AtRestProtection>,
    ) -> Result<Self, StoreError> {
        let mut store = Self::open(path, clock)?;
        store.protection = Some(protection);
        Ok(store)
    }

    /// Injects owned PRIVATE blob protection into the non-durable memory test
    /// profile. Normal callers can keep using open_in_memory with no backend.
    pub fn open_in_memory_with_protection(
        clock: &dyn Clock,
        protection: Arc<dyn AtRestProtection>,
    ) -> Result<Self, StoreError> {
        let mut store = Self::open_in_memory(clock)?;
        store.protection = Some(protection);
        Ok(store)
    }

    fn connection(&self) -> Result<MutexGuard<'_, Connection>, StoreError> {
        self.conn.lock().map_err(|_| StoreError::LockPoisoned)
    }

    /// Validates the applied migration prefix and returns its latest version.
    pub fn schema_version(&self) -> Result<u32, StoreError> {
        let conn = self.connection()?;
        let count = migrate::inspect(&conn, Migrations::embedded(), false)?;
        u32::try_from(count).map_err(|_| StoreError::MigrationCatalogInvalid)
    }

    /// Administrative page/index verification (at most 100 diagnostic rows),
    /// plus separate referential verification. Diagnostic contents are discarded.
    /// Neither check proves application invariants or local-file tamper evidence.
    pub fn verify_integrity(&self) -> Result<(), StoreError> {
        let conn = self.connection()?;
        migrate::page_check(&conn, "PRAGMA integrity_check(100)")?;
        migrate::foreign_key_check(&conn)
    }

    /// Runs a synchronous body in BEGIN IMMEDIATE. Ok commits, Err explicitly
    /// rolls back. Commit failures are typed errors; no success is manufactured.
    /// Tx exposes blobs and low-level lease authority, not SQL. Audited lifecycle
    /// methods refuse with AuditRequired here; use transact_with_participants.
    /// Propagate errors to roll back surrounding operations; failed method
    /// savepoint cleanup makes the
    /// transaction rollback-only even if the closure catches the operation error.
    pub fn transact<T>(
        &self,
        body: impl FnOnce(&mut Tx<'_>) -> Result<T, StoreError>,
    ) -> Result<T, StoreError> {
        self.transact_in(None, None, body)
    }

    /// Runs the fixed Task Audit + Event participants in one transaction.
    /// Neither participant receives SQL or transaction re-entry capability.
    pub fn transact_with_participants<T>(
        &self,
        audit: &dyn crate::audit::TaskAuditParticipant,
        events: &dyn crate::audit::EventParticipant,
        body: impl FnOnce(&mut Tx<'_>) -> Result<T, StoreError>,
    ) -> Result<T, StoreError> {
        self.transact_in(Some(audit), Some(events), body)
    }

    /// Preserves the P2 journal-only test harness. It is absent from every
    /// production build; P3 task transitions require transact_with_participants.
    #[cfg(test)]
    pub fn transact_with_audit<T>(
        &self,
        participant: &dyn crate::audit::TaskAuditParticipant,
        body: impl FnOnce(&mut Tx<'_>) -> Result<T, StoreError>,
    ) -> Result<T, StoreError> {
        self.transact_in(Some(participant), None, body)
    }

    fn transact_in<T>(
        &self,
        audit: Option<&dyn crate::audit::TaskAuditParticipant>,
        events: Option<&dyn crate::audit::EventParticipant>,
        body: impl FnOnce(&mut Tx<'_>) -> Result<T, StoreError>,
    ) -> Result<T, StoreError> {
        let mut conn = self.connection()?;
        // P2H N1: real process death before BEGIN IMMEDIATE is issued. Entirely
        // absent from a build without the test-only fault feature.
        #[cfg(feature = "p2h-fault-injection")]
        crate::fault::reach(crate::fault::Window::BeforeBegin)?;
        let mut tx = Tx {
            inner: conn.transaction_with_behavior(TransactionBehavior::Immediate)?,
            audit,
            events,
            protection: self.protection.clone(),
            rollback_only: false,
            event_count: 0,
            origin: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        };
        // P2H N2: real process death after BEGIN IMMEDIATE, before any write.
        #[cfg(feature = "p2h-fault-injection")]
        crate::fault::reach(crate::fault::Window::AfterBegin)?;
        let result = body(&mut tx);
        if tx.rollback_only {
            if !tx.inner.is_autocommit() {
                tx.inner.rollback()?;
            }
            return Err(StoreError::Sqlite);
        }
        match result {
            Ok(value) => {
                // P2H N5: every write is done; die before COMMIT.
                #[cfg(feature = "p2h-fault-injection")]
                crate::fault::reach(crate::fault::Window::BeforeCommit)?;
                tx.inner.commit()?;
                // P2H N6: COMMIT returned Ok and the caller never learns that.
                #[cfg(feature = "p2h-fault-injection")]
                crate::fault::reach(crate::fault::Window::AfterCommit)?;
                tx.origin.store(true, std::sync::atomic::Ordering::Release);
                Ok(value)
            }
            Err(error) => {
                tx.inner.rollback()?;
                Err(error)
            }
        }
    }

    /// Retryable clean-close preparation. Busy retains this Store handle.
    /// Quiesce callers, release other readers/writers, retry until Complete,
    /// then drop all connections before copying the main database file alone.
    /// Drop is only connection destruction, not proof of a successful checkpoint.
    pub fn checkpoint_for_close(&self) -> Result<CheckpointOutcome, StoreError> {
        if self.profile == Profile::Memory {
            return Ok(CheckpointOutcome::NotApplicable);
        }
        let conn = self.connection()?;
        let (busy, log, checkpointed): (i64, i64, i64) =
            conn.query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?))
            })?;
        if busy != 0 || log != checkpointed {
            return Err(StoreError::Busy);
        }
        if log != 0 {
            return Err(StoreError::ConnectionPolicy);
        }
        Ok(CheckpointOutcome::Complete)
    }
}

pub(crate) fn configure_file(conn: &Connection) -> Result<(), StoreError> {
    configure(conn, Profile::File)
}

fn configure(conn: &Connection, profile: Profile) -> Result<(), StoreError> {
    configure_common(conn, profile)?;
    let mode: String = match profile {
        Profile::File => conn.query_row("PRAGMA journal_mode=WAL", [], |r| r.get(0))?,
        Profile::Memory => conn.query_row("PRAGMA journal_mode", [], |r| r.get(0))?,
    };
    let expected = if profile == Profile::File {
        "wal"
    } else {
        "memory"
    };
    if mode != expected {
        return Err(StoreError::ConnectionPolicy);
    }
    Ok(())
}

fn configure_common(conn: &Connection, profile: Profile) -> Result<(), StoreError> {
    if rusqlite::version_number() < 3_037_000 {
        return Err(StoreError::UnsupportedSqlite);
    }
    let json: (i64, String, i64) = conn
        .query_row(
            "SELECT json_valid('{}'),json_type('{}'),json_extract('{\"n\":1}','$.n')",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .map_err(|_| StoreError::UnsupportedSqlite)?;
    if json != (1, "object".into(), 1) {
        return Err(StoreError::UnsupportedSqlite);
    }
    conn.busy_timeout(Duration::from_millis(5000))?;
    conn.pragma_update(None, "foreign_keys", "ON")?;
    if profile == Profile::File {
        conn.pragma_update(None, "synchronous", "FULL")?;
        if conn.query_row("PRAGMA synchronous", [], |r| r.get::<_, i64>(0))? != 2 {
            return Err(StoreError::ConnectionPolicy);
        }
    }
    if conn.query_row("PRAGMA foreign_keys", [], |r| r.get::<_, i64>(0))? != 1
        || conn.query_row("PRAGMA busy_timeout", [], |r| r.get::<_, i64>(0))? != 5000
    {
        return Err(StoreError::ConnectionPolicy);
    }
    Ok(())
}

#[cfg(test)]
mod policy_tests {
    use super::*;
    use serea_protocol::EpochMillis;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT: AtomicU64 = AtomicU64::new(0);

    fn file_test(body: impl FnOnce(&Path, &mut Connection)) {
        let dir = loop {
            let dir = std::env::temp_dir().join(format!(
                "serea-storage-policy-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            match std::fs::create_dir(&dir) {
                Ok(()) => break dir,
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => panic!("test directory creation failed: {error}"),
            }
        };
        let path = dir.join("store.sqlite");
        let mut conn = Connection::open(&path).unwrap();
        body(&path, &mut conn);
        drop(conn);
        std::fs::remove_dir_all(dir).unwrap();
    }

    fn assert_foreign_keys_restored(conn: &Connection, profile: Profile) {
        conn.pragma_update(None, "foreign_keys", "OFF").unwrap();
        assert_eq!(
            conn.query_row("PRAGMA foreign_keys", [], |r| r.get::<_, i64>(0))
                .unwrap(),
            0
        );
        configure(conn, profile).unwrap();
        assert_eq!(
            conn.query_row("PRAGMA foreign_keys", [], |r| r.get::<_, i64>(0))
                .unwrap(),
            1
        );
        conn.execute_batch("CREATE TABLE policy_parent(id INTEGER PRIMARY KEY); CREATE TABLE policy_child(id INTEGER REFERENCES policy_parent(id));").unwrap();
        assert_eq!(
            StoreError::from(
                conn.execute("INSERT INTO policy_child VALUES (1)", [])
                    .unwrap_err()
            ),
            StoreError::ConstraintViolation
        );
    }

    #[test]
    fn file_configuration_restores_explicitly_disabled_foreign_keys() {
        file_test(|_, conn| assert_foreign_keys_restored(conn, Profile::File));
    }

    #[test]
    fn memory_configuration_restores_explicitly_disabled_foreign_keys() {
        let conn = Connection::open_in_memory().unwrap();
        assert_foreign_keys_restored(&conn, Profile::Memory);
    }

    #[test]
    fn fresh_bootstrap_does_not_publish_nonempty_unmarked_wal_header() {
        file_test(|path, conn| {
            configure_common(conn, Profile::File).unwrap();
            assert_eq!(std::fs::metadata(path).unwrap().len(), 0);
            assert_eq!(
                conn.query_row("PRAGMA journal_mode", [], |r| r.get::<_, String>(0))
                    .unwrap(),
                "delete"
            );
            migrate::apply(
                conn,
                Migrations::embedded(),
                EpochMillis::new(0).unwrap(),
                true,
            )
            .unwrap();
            assert!(std::fs::metadata(path).unwrap().len() > 0);
            // This is the exact stage that used to publish a nonempty foreign
            // file. A separate read-only opener now sees committed authority.
            let reader =
                Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();
            assert_eq!(
                migrate::inspect(&reader, Migrations::embedded(), false).unwrap(),
                2
            );
            drop(reader);
            configure_file(conn).unwrap();
            assert_eq!(
                conn.query_row("PRAGMA journal_mode", [], |r| r.get::<_, String>(0))
                    .unwrap(),
                "wal"
            );
        });
    }
}
