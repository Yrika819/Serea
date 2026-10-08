//! P2D constructor tests; raw SQLite access is crate-private.
//! The local refusal double is NOT ENCRYPTION and must never be called by open.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};

use rusqlite::{Connection, params};
use serea_protocol::{Clock, EpochMillis, ProtocolError};

use crate::{
    AtRestProtection, AtRestProtectionError, CheckpointOutcome, Migrations, Store, StoreError,
};

const INITIAL_SQL: &str = include_str!("../migrations/0001_initial.sql");
const STAMP: i64 = -1;
static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

struct FixedClock(i64);
impl Clock for FixedClock {
    fn now_ms(&self) -> Result<EpochMillis, ProtocolError> {
        EpochMillis::new(self.0)
    }
}

struct CountingClock(AtomicUsize);
impl Clock for CountingClock {
    fn now_ms(&self) -> Result<EpochMillis, ProtocolError> {
        self.0.fetch_add(1, Ordering::Relaxed);
        EpochMillis::new(STAMP)
    }
}

struct FailingClock;
impl Clock for FailingClock {
    fn now_ms(&self) -> Result<EpochMillis, ProtocolError> {
        EpochMillis::new(EpochMillis::MIN - 1)
    }
}

// Deliberately no Debug impl: the public seam must not require one.
#[derive(Default)]
struct UnusedProtection {
    protect_calls: AtomicUsize,
    unprotect_calls: AtomicUsize,
}
impl AtRestProtection for UnusedProtection {
    fn protect(&self, _plaintext: &[u8]) -> Result<Vec<u8>, AtRestProtectionError> {
        self.protect_calls.fetch_add(1, Ordering::Relaxed);
        Err(AtRestProtectionError)
    }

    fn unprotect(&self, _protected: &[u8]) -> Result<Vec<u8>, AtRestProtectionError> {
        self.unprotect_calls.fetch_add(1, Ordering::Relaxed);
        Err(AtRestProtectionError)
    }
}
impl UnusedProtection {
    fn assert_unused(&self) {
        assert_eq!(self.protect_calls.load(Ordering::Relaxed), 0);
        assert_eq!(self.unprotect_calls.load(Ordering::Relaxed), 0);
    }
}

struct TempStore {
    dir: PathBuf,
    path: PathBuf,
}
impl TempStore {
    fn new() -> Self {
        loop {
            let dir = std::env::temp_dir().join(format!(
                "serea-protection-{}-{}",
                std::process::id(),
                NEXT_TEMP.fetch_add(1, Ordering::Relaxed)
            ));
            match fs::create_dir(&dir) {
                Ok(()) => {
                    return Self {
                        path: dir.join("store.sqlite"),
                        dir,
                    };
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => panic!("cannot create test directory: {error}"),
            }
        }
    }
}
impl Drop for TempStore {
    fn drop(&mut self) {
        let result = fs::remove_dir_all(&self.dir);
        if !std::thread::panicking() {
            result.unwrap();
        }
    }
}

fn error<T>(result: Result<T, StoreError>) -> StoreError {
    match result {
        Ok(_) => panic!("expected a typed refusal"),
        Err(error) => error,
    }
}

fn catalog(conn: &Connection) -> Vec<(i64, String, String, i64)> {
    conn.prepare(
        "SELECT version,name,checksum,applied_at_ms FROM schema_migrations ORDER BY version",
    )
    .unwrap()
    .query_map([], |row| {
        Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
    })
    .unwrap()
    .collect::<Result<_, _>>()
    .unwrap()
}

fn schema(conn: &Connection) -> Vec<(String, String, String, Option<String>)> {
    conn.prepare("SELECT type,name,tbl_name,sql FROM sqlite_schema ORDER BY type,name")
        .unwrap()
        .query_map([], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
        })
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap()
}

fn scalar(conn: &Connection, sql: &str) -> i64 {
    conn.query_row(sql, [], |row| row.get(0)).unwrap()
}

fn assert_policy(store: &Store, journal: &str) {
    let conn = store.conn.lock().unwrap();
    assert_eq!(
        conn.query_row("PRAGMA journal_mode", [], |row| row.get::<_, String>(0))
            .unwrap(),
        journal
    );
    assert_eq!(scalar(&conn, "PRAGMA foreign_keys"), 1);
    assert_eq!(scalar(&conn, "PRAGMA busy_timeout"), 5000);
    if journal == "wal" {
        assert_eq!(scalar(&conn, "PRAGMA synchronous"), 2);
    }
}

fn seed(path: &Path) -> Connection {
    let conn = Connection::open(path).unwrap();
    conn.execute_batch(INITIAL_SQL).unwrap();
    conn.execute(
        "INSERT INTO schema_migrations VALUES (1,'0001_initial',?1,?2)",
        params![Migrations::checksum(INITIAL_SQL).as_str(), STAMP],
    )
    .unwrap();
    conn
}

fn assert_injected_refusal_unchanged(path: &Path, expected: StoreError) {
    let before = fs::read(path).unwrap();
    assert!(!before.is_empty());
    let backend = Arc::new(UnusedProtection::default());
    let weak = Arc::downgrade(&backend);
    assert_eq!(
        error(Store::open_with_protection(
            path,
            &FixedClock(STAMP),
            backend.clone(),
        )),
        expected
    );
    assert_eq!(
        fs::read(path).unwrap(),
        before,
        "injected constructor refusal mutated the main database file"
    );
    backend.assert_unused();
    drop(backend);
    assert!(weak.upgrade().is_none(), "failed open retained the backend");
}

fn assert_retained_until_store_drop(
    open: impl FnOnce(&dyn Clock, Arc<dyn AtRestProtection>) -> Result<Store, StoreError>,
) {
    let backend = Arc::new(UnusedProtection::default());
    let weak = Arc::downgrade(&backend);
    let store = {
        let clock = CountingClock(AtomicUsize::new(0));
        let erased: Arc<dyn AtRestProtection> = backend.clone();
        let store = open(&clock, erased).unwrap();
        assert_eq!(clock.0.load(Ordering::Relaxed), 1);
        backend.assert_unused();
        store
    };
    drop(backend);
    {
        let retained = weak
            .upgrade()
            .expect("Store must own the backend after the caller drops its Arc");
        retained.assert_unused();
    }
    assert_eq!(store.schema_version().unwrap(), Migrations::LATEST);
    store.verify_integrity().unwrap();
    store.transact(|_| Ok(())).unwrap();
    store.checkpoint_for_close().unwrap();
    weak.upgrade().unwrap().assert_unused();
    drop(store);
    assert!(weak.upgrade().is_none(), "Store drop leaked the backend");
}

#[test]
fn protection_trait_object_and_store_are_send_sync_without_a_debug_bound() {
    fn assert_send_sync_static<T: ?Sized + Send + Sync + 'static>() {}
    assert_send_sync_static::<dyn AtRestProtection>();
    assert_send_sync_static::<Arc<dyn AtRestProtection>>();
    assert_send_sync_static::<Store>();
    let backend: Arc<dyn AtRestProtection> = Arc::new(UnusedProtection::default());
    let _: &dyn AtRestProtection = backend.as_ref();
}

#[test]
fn protection_error_is_a_payload_free_unit_struct() {
    let AtRestProtectionError = AtRestProtectionError;
    assert_eq!(std::mem::size_of::<AtRestProtectionError>(), 0);
}

#[test]
fn injected_file_store_owns_backend_and_outlives_the_clock_and_original_arc() {
    let temp = TempStore::new();
    assert_retained_until_store_drop(|clock, backend| {
        Store::open_with_protection(&temp.path, clock, backend)
    });
}

#[test]
fn injected_memory_store_owns_backend_and_outlives_the_clock_and_original_arc() {
    assert_retained_until_store_drop(Store::open_in_memory_with_protection);
}

#[test]
fn injected_file_open_preserves_policy_catalog_schema_checkpoint_and_reopen() {
    let original = Store::open_in_memory(&FixedClock(STAMP)).unwrap();
    let expected_catalog = catalog(&original.conn.lock().unwrap());
    let expected_schema = schema(&original.conn.lock().unwrap());
    for zero_length in [false, true] {
        let temp = TempStore::new();
        if zero_length {
            fs::File::create_new(&temp.path).unwrap();
        }
        let backend = Arc::new(UnusedProtection::default());
        let store =
            Store::open_with_protection(&temp.path, &FixedClock(STAMP), backend.clone()).unwrap();
        backend.assert_unused();
        assert_policy(&store, "wal");
        assert_eq!(store.schema_version().unwrap(), Migrations::LATEST);
        assert_eq!(catalog(&store.conn.lock().unwrap()), expected_catalog);
        assert_eq!(schema(&store.conn.lock().unwrap()), expected_schema);
        store.verify_integrity().unwrap();
        store
            .transact(|tx| {
                tx.inner.execute_batch(
                    "CREATE TABLE constructor_checkpoint_probe (value INTEGER); \
                     INSERT INTO constructor_checkpoint_probe VALUES (7)",
                )?;
                Ok(())
            })
            .unwrap();
        let wal = temp.path.with_file_name("store.sqlite-wal");
        assert!(fs::metadata(&wal).unwrap().len() > 0);
        let committed_schema = schema(&store.conn.lock().unwrap());
        assert_eq!(
            store.checkpoint_for_close().unwrap(),
            CheckpointOutcome::Complete
        );
        if wal.exists() {
            assert_eq!(fs::metadata(&wal).unwrap().len(), 0);
        }
        backend.assert_unused();
        drop(store);

        let reopened =
            Store::open_with_protection(&temp.path, &FixedClock(1234), backend.clone()).unwrap();
        backend.assert_unused();
        assert_policy(&reopened, "wal");
        assert_eq!(reopened.schema_version().unwrap(), Migrations::LATEST);
        assert_eq!(catalog(&reopened.conn.lock().unwrap()), expected_catalog);
        assert_eq!(schema(&reopened.conn.lock().unwrap()), committed_schema);
        assert_eq!(
            scalar(
                &reopened.conn.lock().unwrap(),
                "SELECT value FROM constructor_checkpoint_probe",
            ),
            7
        );
        reopened.verify_integrity().unwrap();
        assert_eq!(
            reopened.checkpoint_for_close().unwrap(),
            CheckpointOutcome::Complete
        );
        backend.assert_unused();
    }
}

#[test]
fn injected_memory_open_preserves_memory_profile_catalog_schema_and_checkpoint() {
    let original = Store::open_in_memory(&FixedClock(STAMP)).unwrap();
    let backend = Arc::new(UnusedProtection::default());
    let store = Store::open_in_memory_with_protection(&FixedClock(STAMP), backend.clone()).unwrap();
    backend.assert_unused();
    assert_policy(&store, "memory");
    assert_eq!(store.schema_version().unwrap(), Migrations::LATEST);
    assert_eq!(
        catalog(&store.conn.lock().unwrap()),
        catalog(&original.conn.lock().unwrap())
    );
    assert_eq!(
        schema(&store.conn.lock().unwrap()),
        schema(&original.conn.lock().unwrap())
    );
    store.verify_integrity().unwrap();
    assert_eq!(
        store.checkpoint_for_close().unwrap(),
        CheckpointOutcome::NotApplicable
    );
    backend.assert_unused();
}

#[test]
fn injected_file_clock_failure_precedes_creation_and_preserves_existing_bytes() {
    let expected = StoreError::Clock(EpochMillis::new(EpochMillis::MIN - 1).unwrap_err());
    for existing in [false, true] {
        let temp = TempStore::new();
        let sentinel = b"synthetic plaintext file must stay untouched";
        if existing {
            fs::write(&temp.path, sentinel).unwrap();
        }
        let backend = Arc::new(UnusedProtection::default());
        let weak = Arc::downgrade(&backend);
        assert_eq!(
            error(Store::open_with_protection(
                &temp.path,
                &FailingClock,
                backend.clone(),
            )),
            expected
        );
        if existing {
            assert_eq!(fs::read(&temp.path).unwrap(), sentinel);
            assert_eq!(fs::read_dir(&temp.dir).unwrap().count(), 1);
        } else {
            assert!(!temp.path.exists());
            assert_eq!(fs::read_dir(&temp.dir).unwrap().count(), 0);
        }
        backend.assert_unused();
        drop(backend);
        assert!(weak.upgrade().is_none());
    }
}

#[test]
fn injected_memory_clock_failure_is_typed_and_does_not_invoke_or_retain_backend() {
    let backend = Arc::new(UnusedProtection::default());
    let weak = Arc::downgrade(&backend);
    assert_eq!(
        error(Store::open_in_memory_with_protection(
            &FailingClock,
            backend.clone(),
        )),
        StoreError::Clock(EpochMillis::new(EpochMillis::MIN - 1).unwrap_err())
    );
    backend.assert_unused();
    drop(backend);
    assert!(weak.upgrade().is_none());
}

#[test]
fn injected_open_refuses_foreign_nonempty_files_without_mutation_or_backend_calls() {
    let binary = TempStore::new();
    fs::write(&binary.path, b"not SQLite\0synthetic binary\xff").unwrap();
    assert_injected_refusal_unchanged(&binary.path, StoreError::NotSereaStore);

    let foreign = TempStore::new();
    {
        let conn = Connection::open(&foreign.path).unwrap();
        conn.execute_batch(
            "CREATE TABLE unrelated (value TEXT); INSERT INTO unrelated VALUES ('keep')",
        )
        .unwrap();
    }
    assert_injected_refusal_unchanged(&foreign.path, StoreError::NotSereaStore);
}

#[test]
fn injected_open_refuses_nonempty_empty_schema_without_mutation_or_backend_calls() {
    let temp = TempStore::new();
    {
        let conn = Connection::open(&temp.path).unwrap();
        conn.execute_batch("CREATE TABLE removed (value INTEGER); DROP TABLE removed")
            .unwrap();
        assert_eq!(scalar(&conn, "SELECT count(*) FROM sqlite_schema"), 0);
    }
    assert_injected_refusal_unchanged(&temp.path, StoreError::NotSereaStore);
}

#[test]
fn injected_open_refuses_plaintext_file_without_mutation_or_backend_calls() {
    let temp = TempStore::new();
    fs::write(&temp.path, b"synthetic plaintext is not a Serea database\n").unwrap();
    assert_injected_refusal_unchanged(&temp.path, StoreError::NotSereaStore);
}

#[test]
fn injected_open_refuses_empty_migration_catalog_without_mutation_or_backend_calls() {
    let temp = TempStore::new();
    {
        let conn = seed(&temp.path);
        conn.execute("DELETE FROM schema_migrations", []).unwrap();
    }
    assert_injected_refusal_unchanged(&temp.path, StoreError::MigrationCatalogInvalid);
}

#[test]
fn injected_open_refuses_newer_schema_without_mutation_or_backend_calls() {
    let temp = TempStore::new();
    {
        let conn = seed(&temp.path);
        conn.execute_batch("UPDATE schema_migrations SET version=5,name='0005_future'")
            .unwrap();
    }
    assert_injected_refusal_unchanged(&temp.path, StoreError::SchemaTooNew);
}

#[test]
fn injected_open_refuses_checksum_mismatch_without_mutation_or_backend_calls() {
    let temp = TempStore::new();
    {
        let conn = seed(&temp.path);
        conn.execute(
            "UPDATE schema_migrations SET checksum=?1",
            [format!("sha256:{}", "0".repeat(64))],
        )
        .unwrap();
    }
    assert_injected_refusal_unchanged(&temp.path, StoreError::MigrationChecksumMismatch);
}
