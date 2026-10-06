//! Foundation contract tests; SQL access here is crate-private, not a public API.

use crate::migrate::{self, Migration, Migrations};
use crate::{CheckpointOutcome, Store, StoreError};
use rusqlite::{Connection, params};
use serea_protocol::{Clock, EpochMillis, ProtocolError, SerializationRejection};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Barrier;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

const INITIAL_SQL: &str = include_str!("../migrations/0001_initial.sql");
const STAMP: i64 = -1;
const SENTINEL: &str = "PRIVATE-sentinel-path-sql-bound-value-do-not-log";
static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

struct FixedClock(i64);
impl Clock for FixedClock {
    fn now_ms(&self) -> Result<EpochMillis, ProtocolError> {
        EpochMillis::new(self.0)
    }
}

struct TempStore {
    dir: PathBuf,
    path: PathBuf,
}
impl TempStore {
    fn new() -> Self {
        let executable = std::env::current_exe().unwrap();
        let identity: String = executable
            .file_name()
            .unwrap()
            .to_string_lossy()
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
            .collect();
        loop {
            let dir = std::env::temp_dir().join(format!(
                "serea-foundation-{identity}-{}-{}",
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

    fn open(&self) -> Store {
        Store::open(&self.path, &FixedClock(STAMP)).unwrap()
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

fn assert_error<T>(result: Result<T, StoreError>, expected: StoreError) {
    let actual = error(result);
    assert_eq!(
        std::mem::discriminant(&actual),
        std::mem::discriminant(&expected),
        "expected {expected:?}, got {actual:?}"
    );
}

fn instant() -> EpochMillis {
    EpochMillis::new(STAMP).unwrap()
}

fn rows(conn: &Connection) -> Vec<(i64, String, String, i64)> {
    conn.prepare(
        "SELECT version, name, checksum, applied_at_ms FROM schema_migrations ORDER BY version",
    )
    .unwrap()
    .query_map([], |row| {
        Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
    })
    .unwrap()
    .collect::<Result<_, _>>()
    .unwrap()
}

fn assert_initial(conn: &Connection, stamp: i64) {
    let embedded = Migrations::embedded();
    assert_eq!(embedded[0].version, 1);
    assert_eq!(embedded[0].name, "0001_initial");
    assert_eq!(embedded[0].sql, INITIAL_SQL);
    assert_eq!(
        rows(conn).first(),
        Some(&(
            1,
            "0001_initial".into(),
            Migrations::checksum(INITIAL_SQL).as_str().into(),
            stamp
        ))
    );
}

fn assert_latest(conn: &Connection, stamp: i64) {
    assert_eq!(Migrations::LATEST, 2);
    assert_eq!(Migrations::embedded().len(), 2);
    assert_initial(conn, stamp);
    assert_eq!(rows(conn).len(), 2);
    assert_eq!(
        scalar(conn, "SELECT max(version) FROM schema_migrations"),
        2
    );
}

fn seed(path: &Path) -> Connection {
    let conn = Connection::open(path).unwrap();
    conn.execute_batch(INITIAL_SQL).unwrap();
    conn.execute(
        "INSERT INTO schema_migrations VALUES (1, '0001_initial', ?1, ?2)",
        params![Migrations::checksum(INITIAL_SQL).as_str(), STAMP],
    )
    .unwrap();
    conn
}

fn refuse_unchanged(path: &Path, expected: StoreError) {
    let before = fs::read(path).unwrap();
    assert!(!before.is_empty());
    assert_error(Store::open(path, &FixedClock(STAMP)), expected);
    assert_eq!(
        fs::read(path).unwrap(),
        before,
        "refusal mutated the main database file"
    );
}

fn exists(conn: &Connection, name: &str) -> bool {
    conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_schema WHERE name = ?1)",
        [name],
        |row| row.get(0),
    )
    .unwrap()
}

fn scalar(conn: &Connection, sql: &str) -> i64 {
    conn.query_row(sql, [], |row| row.get(0)).unwrap()
}

fn strings(conn: &Connection, sql: &str) -> Vec<String> {
    conn.prepare(sql)
        .unwrap()
        .query_map([], |row| row.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap()
}

fn assert_file_policy(conn: &Connection) {
    assert_eq!(strings(conn, "PRAGMA journal_mode"), ["wal"]);
    assert_eq!(scalar(conn, "PRAGMA foreign_keys"), 1);
    assert_eq!(scalar(conn, "PRAGMA synchronous"), 2);
    assert_eq!(scalar(conn, "PRAGMA busy_timeout"), 5000);
}

fn configure_file_connection(conn: &Connection) {
    assert!(
        conn.is_autocommit(),
        "file policy must be configured outside a transaction"
    );
    crate::store::configure_file(conn).unwrap();
    assert_file_policy(conn);
}

#[test]
fn absent_and_zero_length_files_migrate_exactly_once_and_reopen_keeps_original_stamp() {
    for zero_length in [false, true] {
        let temp = TempStore::new();
        if zero_length {
            fs::File::create_new(&temp.path).unwrap();
        } else {
            assert!(!temp.path.exists());
        }
        {
            let store = temp.open();
            assert_eq!(store.schema_version().unwrap(), 2);
            assert_latest(&store.conn.lock().unwrap(), STAMP);
            store.verify_integrity().unwrap();
            assert_eq!(
                store.checkpoint_for_close().unwrap(),
                CheckpointOutcome::Complete
            );
        }
        for stamp in [0, EpochMillis::MAX] {
            let store = Store::open(&temp.path, &FixedClock(stamp)).unwrap();
            assert_latest(&store.conn.lock().unwrap(), STAMP);
            assert_eq!(store.schema_version().unwrap(), 2);
            store.verify_integrity().unwrap();
        }
    }
}

#[test]
fn file_connection_policy_is_explicit_wal_fk_full_and_five_second_timeout() {
    let temp = TempStore::new();
    let store = temp.open();
    let conn = store.conn.lock().unwrap();
    assert_file_policy(&conn);
    assert_eq!(scalar(&conn, "PRAGMA application_id"), 0);
    assert_eq!(scalar(&conn, "PRAGMA user_version"), 0);
}

#[test]
fn memory_profile_reports_memory_and_not_applicable_without_claiming_durability() {
    let store = Store::open_in_memory(&FixedClock(STAMP)).unwrap();
    {
        let conn = store.conn.lock().unwrap();
        assert_eq!(strings(&conn, "PRAGMA journal_mode"), ["memory"]);
        assert_eq!(scalar(&conn, "PRAGMA foreign_keys"), 1);
        assert_eq!(scalar(&conn, "PRAGMA busy_timeout"), 5000);
        assert_initial(&conn, STAMP);
        conn.execute_batch("CREATE TABLE memory_only (value INTEGER)")
            .unwrap();
    }
    assert_eq!(
        store.checkpoint_for_close().unwrap(),
        CheckpointOutcome::NotApplicable
    );
    store.verify_integrity().unwrap();
    drop(store);
    let independent = Store::open_in_memory(&FixedClock(0)).unwrap();
    assert!(!exists(&independent.conn.lock().unwrap(), "memory_only"));
    assert_initial(&independent.conn.lock().unwrap(), 0);
}

#[test]
fn exact_raw_sql_hash_and_bundled_sqlite_build_are_pinned() {
    assert_eq!(
        Migrations::checksum(INITIAL_SQL).as_str(),
        "sha256:d9068dccbc26ececb71be79c475080633166ba0163c62b2d98b9733512baefea"
    );
    assert_eq!(
        Migrations::checksum("").as_str(),
        "sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    );
    assert_eq!(
        Migrations::checksum("abc").as_str(),
        "sha256:ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
    for different in [
        format!("{INITIAL_SQL}\n"),
        format!("-- comment\n{INITIAL_SQL}"),
        INITIAL_SQL.trim_end().to_owned(),
        format!(" {INITIAL_SQL}"),
    ] {
        assert_ne!(
            Migrations::checksum(&different),
            Migrations::checksum(INITIAL_SQL)
        );
    }
    let temp = TempStore::new();
    let store = temp.open();
    let conn = store.conn.lock().unwrap();
    assert_eq!(strings(&conn, "SELECT sqlite_version()"), ["3.53.2"]);
    assert_eq!(
        strings(&conn, "SELECT sqlite_source_id()"),
        ["2026-06-03 19:12:13 d6e03d8c777cfa2d35e3b60d8ec3e0187f3e9f99d8e2ee9cac695fd6fcdf1a24"]
    );
    assert_eq!(scalar(&conn, "SELECT json_valid('{}')"), 1);
}

#[test]
fn foreign_binary_unrelated_sqlite_and_nonempty_zero_table_sqlite_are_never_adopted() {
    let binary = TempStore::new();
    fs::write(&binary.path, b"not SQLite\0PRIVATE binary sentinel\xff").unwrap();
    refuse_unchanged(&binary.path, StoreError::NotSereaStore);
    for drop_table in [false, true] {
        let temp = TempStore::new();
        {
            let conn = Connection::open(&temp.path).unwrap();
            conn.execute_batch(
                "CREATE TABLE unrelated (value TEXT); INSERT INTO unrelated VALUES ('keep')",
            )
            .unwrap();
            if drop_table {
                conn.execute_batch("DROP TABLE unrelated").unwrap();
                assert_eq!(
                    scalar(
                        &conn,
                        "SELECT count(*) FROM sqlite_schema WHERE type='table' AND name NOT LIKE 'sqlite_%'"
                    ),
                    0
                );
            }
        }
        refuse_unchanged(&temp.path, StoreError::NotSereaStore);
    }
}

#[test]
fn catalog_prefix_refusals_are_typed_and_leave_main_bytes_unchanged() {
    let cases = [
        (
            "DELETE FROM schema_migrations",
            StoreError::MigrationCatalogInvalid,
        ),
        (
            "UPDATE schema_migrations SET name='0001_wrong'",
            StoreError::MigrationCatalogInvalid,
        ),
        (
            "UPDATE schema_migrations SET checksum='sha256:0000000000000000000000000000000000000000000000000000000000000000'",
            StoreError::MigrationChecksumMismatch,
        ),
        (
            "PRAGMA ignore_check_constraints=ON; UPDATE schema_migrations SET version=0",
            StoreError::MigrationCatalogInvalid,
        ),
        (
            "PRAGMA ignore_check_constraints=ON; UPDATE schema_migrations SET checksum='malformed'",
            StoreError::MigrationCatalogInvalid,
        ),
        (
            "DROP TABLE schema_migrations; CREATE TABLE schema_migrations (version INTEGER)",
            StoreError::MigrationCatalogInvalid,
        ),
        (
            "ALTER TABLE schema_migrations RENAME TO old_catalog; CREATE TABLE schema_migrations (version, name, checksum, applied_at_ms); INSERT INTO schema_migrations SELECT 'not-an-integer', name, checksum, applied_at_ms FROM old_catalog",
            StoreError::MigrationCatalogInvalid,
        ),
        (
            "ALTER TABLE schema_migrations RENAME TO old_catalog; CREATE TABLE schema_migrations (version, name, checksum, applied_at_ms); INSERT INTO schema_migrations SELECT * FROM old_catalog; INSERT INTO schema_migrations SELECT * FROM old_catalog",
            StoreError::MigrationCatalogInvalid,
        ),
    ];
    for (sql, expected) in cases {
        let temp = TempStore::new();
        {
            let conn = seed(&temp.path);
            conn.execute_batch(sql).unwrap();
        }
        refuse_unchanged(&temp.path, expected);
    }
}

#[test]
fn newer_version_takes_priority_over_wrong_names_checksums_and_missing_prefix() {
    for sql in [
        "UPDATE schema_migrations SET version=3, name='0003_future'",
        "UPDATE schema_migrations SET version=3, name='wrong', checksum='sha256:0000000000000000000000000000000000000000000000000000000000000000'",
        "UPDATE schema_migrations SET name='wrong', checksum='sha256:0000000000000000000000000000000000000000000000000000000000000000'; INSERT INTO schema_migrations VALUES (3, '0003_future', 'sha256:0000000000000000000000000000000000000000000000000000000000000000', 0)",
    ] {
        let temp = TempStore::new();
        {
            let conn = seed(&temp.path);
            conn.execute_batch(sql).unwrap();
        }
        refuse_unchanged(&temp.path, StoreError::SchemaTooNew);
    }
}

#[test]
fn live_uncheckpointed_wal_refusals_preserve_main_wal_and_committed_state() {
    let cases = [
        (
            false,
            "UPDATE unrelated SET value='committed WAL value'",
            StoreError::NotSereaStore,
        ),
        (false, "DROP TABLE unrelated", StoreError::NotSereaStore),
        (
            true,
            "UPDATE schema_migrations SET version=3, name='0003_future'",
            StoreError::SchemaTooNew,
        ),
        (
            true,
            "UPDATE schema_migrations SET checksum='sha256:0000000000000000000000000000000000000000000000000000000000000000'",
            StoreError::MigrationChecksumMismatch,
        ),
    ];
    for (serea, sql, expected) in cases {
        let temp = TempStore::new();
        let writer = if serea {
            seed(&temp.path)
        } else {
            let conn = Connection::open(&temp.path).unwrap();
            conn.execute_batch(
                "CREATE TABLE unrelated (value TEXT); INSERT INTO unrelated VALUES ('baseline')",
            )
            .unwrap();
            conn
        };
        configure_file_connection(&writer);
        writer.pragma_update(None, "wal_autocheckpoint", 0).unwrap();
        let checkpoint: (i64, i64, i64) = writer
            .query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?))
            })
            .unwrap();
        assert_eq!(checkpoint, (0, 0, 0));
        let reader =
            Connection::open_with_flags(&temp.path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
                .unwrap();
        reader.execute_batch("BEGIN").unwrap();
        let reader_schema = strings(&reader, "SELECT name FROM sqlite_schema ORDER BY name");
        if serea {
            assert_initial(&reader, STAMP);
        } else {
            assert_eq!(
                strings(&reader, "SELECT value FROM unrelated"),
                ["baseline"]
            );
        }
        let checkpointed_main = fs::read(&temp.path).unwrap();
        writer.execute_batch(sql).unwrap();
        assert!(
            writer.is_autocommit(),
            "fixture modification must be committed"
        );
        let main_before = fs::read(&temp.path).unwrap();
        assert_eq!(
            main_before, checkpointed_main,
            "fixture changes should still live only in WAL"
        );
        let wal_path = temp.path.with_file_name("store.sqlite-wal");
        let wal_before = fs::read(&wal_path).unwrap();
        assert!(
            wal_before.len() > 32,
            "fixture must contain WAL frames, not just a header"
        );
        let schema_before = strings(
            &writer,
            "SELECT type || ':' || name || ':' || coalesce(sql, '') FROM sqlite_schema ORDER BY type, name",
        );
        let catalog_before = serea.then(|| rows(&writer));
        let unrelated_before =
            exists(&writer, "unrelated").then(|| strings(&writer, "SELECT value FROM unrelated"));

        assert_error(Store::open(&temp.path, &FixedClock(STAMP)), expected);

        assert_eq!(fs::read(&temp.path).unwrap(), main_before);
        assert_eq!(fs::read(&wal_path).unwrap(), wal_before);
        assert_eq!(strings(&writer, "PRAGMA journal_mode"), ["wal"]);
        assert_eq!(strings(&reader, "PRAGMA journal_mode"), ["wal"]);
        assert_eq!(
            strings(
                &writer,
                "SELECT type || ':' || name || ':' || coalesce(sql, '') FROM sqlite_schema ORDER BY type, name"
            ),
            schema_before
        );
        assert_eq!(serea.then(|| rows(&writer)), catalog_before);
        assert_eq!(
            exists(&writer, "unrelated").then(|| strings(&writer, "SELECT value FROM unrelated")),
            unrelated_before
        );
        assert_eq!(
            strings(&reader, "SELECT name FROM sqlite_schema ORDER BY name"),
            reader_schema
        );
        if serea {
            assert_initial(&reader, STAMP);
        } else {
            assert_eq!(
                strings(&reader, "SELECT value FROM unrelated"),
                ["baseline"]
            );
        }
        reader.execute_batch("ROLLBACK").unwrap();
    }
}

#[test]
fn malformed_earlier_catalog_rows_do_not_hide_a_later_future_version() {
    for malformed in [
        rusqlite::types::Value::Null,
        rusqlite::types::Value::Integer(0),
        rusqlite::types::Value::Integer(-1),
        rusqlite::types::Value::Text("-1".into()),
    ] {
        let temp = TempStore::new();
        {
            let conn = seed(&temp.path);
            configure_file_connection(&conn);
            conn.execute_batch(
                "ALTER TABLE schema_migrations RENAME TO old_catalog;
                CREATE TABLE schema_migrations (version, name, checksum, applied_at_ms);
                DROP TABLE old_catalog;",
            )
            .unwrap();
            conn.execute(
                "INSERT INTO schema_migrations VALUES (?1, 'malformed', 'bad checksum', NULL)",
                [malformed],
            )
            .unwrap();
            conn.execute(
                "INSERT INTO schema_migrations VALUES (3, '0003_future', ?1, 0)",
                [Migrations::checksum(INITIAL_SQL).as_str()],
            )
            .unwrap();
        }
        let before = fs::read(&temp.path).unwrap();
        {
            let conn = Connection::open(&temp.path).unwrap();
            assert_error(
                migrate::inspect(&conn, Migrations::embedded(), false),
                StoreError::SchemaTooNew,
            );
        }
        assert_eq!(fs::read(&temp.path).unwrap(), before);
        refuse_unchanged(&temp.path, StoreError::SchemaTooNew);
    }
}

#[test]
fn a_checkpointed_valid_store_with_corrupted_header_magic_is_refused_unchanged() {
    let temp = TempStore::new();
    {
        let store = temp.open();
        store.verify_integrity().unwrap();
        assert_eq!(
            store.checkpoint_for_close().unwrap(),
            CheckpointOutcome::Complete
        );
    }
    let original = fs::read(&temp.path).unwrap();
    assert_eq!(&original[..16], b"SQLite format 3\0");
    let mut magic = *b"SQLite format 3\0";
    magic[0] = b'X';
    {
        let mut file = fs::OpenOptions::new().write(true).open(&temp.path).unwrap();
        file.write_all(&magic).unwrap();
        file.sync_all().unwrap();
    }
    let corrupted = fs::read(&temp.path).unwrap();
    assert_eq!(&corrupted[..16], &magic);
    assert_eq!(&corrupted[16..], &original[16..]);
    refuse_unchanged(&temp.path, StoreError::NotSereaStore);
}

fn migration(version: u32, name: &'static str, sql: &'static str) -> Migration {
    Migration { version, name, sql }
}

fn initial() -> Migration {
    migration(1, "0001_initial", INITIAL_SQL)
}

#[test]
fn embedded_catalog_validation_refuses_zero_gaps_duplicates_and_reordering() {
    migrate::validate_catalog(Migrations::embedded()).unwrap();
    let cases = vec![
        vec![],
        vec![migration(0, "0000_zero", "SELECT 1;")],
        vec![migration(2, "0002_gap", "SELECT 1;")],
        vec![initial(), migration(1, "0001_duplicate", "SELECT 1;")],
        vec![initial(), migration(3, "0003_gap", "SELECT 1;")],
        vec![migration(2, "0002_first", "SELECT 1;"), initial()],
        vec![initial(), migration(2, "0001_initial", "SELECT 1;")],
        vec![migration(1, "", "SELECT 1;")],
    ];
    for catalog in cases {
        assert_error(
            migrate::validate_catalog(&catalog),
            StoreError::MigrationCatalogInvalid,
        );
        let temp = TempStore::new();
        let mut conn = seed(&temp.path);
        configure_file_connection(&conn);
        let before = fs::read(&temp.path).unwrap();
        assert_error(
            migrate::apply(&mut conn, &catalog, instant(), false),
            StoreError::MigrationCatalogInvalid,
        );
        assert!(conn.is_autocommit());
        assert_initial(&conn, STAMP);
        assert_file_policy(&conn);
        assert_eq!(fs::read(&temp.path).unwrap(), before);
        drop(conn);
        let reopened = temp.open();
        assert_file_policy(&reopened.conn.lock().unwrap());
        assert_initial(&reopened.conn.lock().unwrap(), STAMP);
    }
}

#[test]
fn migrations_apply_in_order_and_reapplication_preserves_every_catalog_row() {
    let temp = TempStore::new();
    let mut conn = Connection::open(&temp.path).unwrap();
    configure_file_connection(&conn);
    let catalog = [
        initial(),
        migration(
            2,
            "0002_parent",
            "CREATE TABLE migration_parent (value INTEGER PRIMARY KEY); INSERT INTO migration_parent VALUES (7);",
        ),
        migration(
            3,
            "0003_child",
            "CREATE TABLE migration_child (value INTEGER REFERENCES migration_parent(value)); INSERT INTO migration_child VALUES (7);",
        ),
    ];
    migrate::apply(&mut conn, &catalog, instant(), true).unwrap();
    let before = rows(&conn);
    assert_eq!(before.len(), 3);
    for (row, item) in before.iter().zip(&catalog) {
        assert_eq!(
            row,
            &(
                i64::from(item.version),
                item.name.into(),
                Migrations::checksum(item.sql).as_str().into(),
                STAMP
            )
        );
    }
    migrate::apply(&mut conn, &catalog, EpochMillis::new(0).unwrap(), false).unwrap();
    assert_eq!(rows(&conn), before);
    assert_eq!(scalar(&conn, "SELECT count(*) FROM migration_child"), 1);
    assert!(conn.is_autocommit());
    drop(conn);
    let mut reopened = Connection::open(&temp.path).unwrap();
    configure_file_connection(&reopened);
    assert_eq!(rows(&reopened), before);
    assert_eq!(scalar(&reopened, "SELECT count(*) FROM migration_child"), 1);
    migrate::apply(&mut reopened, &catalog, EpochMillis::new(1).unwrap(), false).unwrap();
    assert_eq!(rows(&reopened), before);
}

#[test]
fn modified_embedded_sql_checksum_refuses_before_pending_migration_and_preserves_file() {
    const MODIFIED_SQL: &str = concat!(
        include_str!("../migrations/0001_initial.sql"),
        "\n-- changed embedded source bytes\n"
    );
    let temp = TempStore::new();
    let catalog = [
        migration(1, "0001_initial", MODIFIED_SQL),
        migration(
            2,
            "0002_pending",
            "CREATE TABLE pending_migration (value INTEGER); INSERT INTO pending_migration VALUES (42);",
        ),
    ];
    assert_ne!(
        Migrations::checksum(MODIFIED_SQL),
        Migrations::checksum(INITIAL_SQL)
    );
    let original_rows;
    let original_schema;
    {
        let conn = seed(&temp.path);
        configure_file_connection(&conn);
        original_rows = rows(&conn);
        original_schema = strings(
            &conn,
            "SELECT type || ':' || name || ':' || coalesce(sql, '') FROM sqlite_schema ORDER BY type, name",
        );
    }
    let before = fs::read(&temp.path).unwrap();
    {
        let mut conn = Connection::open(&temp.path).unwrap();
        configure_file_connection(&conn);
        assert_error(
            migrate::apply(&mut conn, &catalog, EpochMillis::new(0).unwrap(), false),
            StoreError::MigrationChecksumMismatch,
        );
        assert!(conn.is_autocommit());
        assert!(!exists(&conn, "pending_migration"));
        assert_eq!(rows(&conn), original_rows);
        assert_eq!(
            strings(
                &conn,
                "SELECT type || ':' || name || ':' || coalesce(sql, '') FROM sqlite_schema ORDER BY type, name"
            ),
            original_schema
        );
    }
    assert_eq!(fs::read(&temp.path).unwrap(), before);
    let reopened = temp.open();
    assert_file_policy(&reopened.conn.lock().unwrap());
    assert_initial(&reopened.conn.lock().unwrap(), STAMP);
    assert!(!exists(&reopened.conn.lock().unwrap(), "pending_migration"));
    reopened.verify_integrity().unwrap();
}

#[test]
fn gap_in_existing_catalog_is_not_accepted_as_max_version() {
    let temp = TempStore::new();
    let catalog = [
        initial(),
        migration(2, "0002_middle", "SELECT 1;"),
        migration(3, "0003_last", "SELECT 1;"),
    ];
    {
        let mut conn = Connection::open(&temp.path).unwrap();
        configure_file_connection(&conn);
        migrate::apply(&mut conn, &catalog, instant(), true).unwrap();
        conn.execute("DELETE FROM schema_migrations WHERE version=2", [])
            .unwrap();
    }
    let before = fs::read(&temp.path).unwrap();
    {
        let mut conn = Connection::open(&temp.path).unwrap();
        configure_file_connection(&conn);
        assert_error(
            migrate::apply(&mut conn, &catalog, instant(), false),
            StoreError::MigrationCatalogInvalid,
        );
        assert!(conn.is_autocommit());
    }
    assert_eq!(fs::read(&temp.path).unwrap(), before);
}

#[test]
fn internal_apply_does_not_adopt_nonfresh_empty_or_unrelated_databases() {
    for unrelated in [false, true] {
        let temp = TempStore::new();
        {
            let conn = Connection::open(&temp.path).unwrap();
            configure_file_connection(&conn);
            conn.execute_batch("CREATE TABLE unrelated (value INTEGER)")
                .unwrap();
            if !unrelated {
                conn.execute_batch("DROP TABLE unrelated").unwrap();
            }
        }
        let before = fs::read(&temp.path).unwrap();
        {
            let mut conn = Connection::open(&temp.path).unwrap();
            configure_file_connection(&conn);
            assert_error(
                migrate::apply(&mut conn, Migrations::embedded(), instant(), false),
                StoreError::NotSereaStore,
            );
            assert!(!exists(&conn, "schema_migrations"));
        }
        assert_eq!(fs::read(&temp.path).unwrap(), before);
    }
}

#[test]
fn failed_second_migration_rolls_back_ddl_and_inserted_row_but_keeps_first_commit() {
    let temp = TempStore::new();
    {
        let mut conn = Connection::open(&temp.path).unwrap();
        configure_file_connection(&conn);
        let catalog = [
            initial(),
            migration(
                2,
                "0002_failure",
                "
            CREATE TABLE failed_ddl (value INTEGER);
            INSERT INTO failed_ddl VALUES (42);
            INSERT INTO schema_migrations VALUES (2, '0002_failure',
              'sha256:0000000000000000000000000000000000000000000000000000000000000000', -1);
            INSERT INTO nonexistent_table VALUES (1);
        ",
            ),
        ];
        assert_error(
            migrate::apply(&mut conn, &catalog, instant(), true),
            StoreError::Sqlite,
        );
        assert!(conn.is_autocommit());
        assert!(!exists(&conn, "failed_ddl"));
        assert_initial(&conn, STAMP);
    }
    let reopened = temp.open();
    assert_file_policy(&reopened.conn.lock().unwrap());
    assert_initial(&reopened.conn.lock().unwrap(), STAMP);
    assert!(!exists(&reopened.conn.lock().unwrap(), "failed_ddl"));
}

#[test]
fn postmigration_foreign_key_gate_rolls_back_ddl_orphan_and_version_row() {
    let temp = TempStore::new();
    let mut conn = Connection::open(&temp.path).unwrap();
    configure_file_connection(&conn);
    let catalog = [initial(), migration(2, "0002_orphan", "
        CREATE TABLE migration_fk_parent (id INTEGER PRIMARY KEY);
        CREATE TABLE migration_fk_child (id INTEGER REFERENCES migration_fk_parent(id) DEFERRABLE INITIALLY DEFERRED);
        INSERT INTO migration_fk_child VALUES (99);
    ")];
    assert_error(
        migrate::apply(&mut conn, &catalog, instant(), true),
        StoreError::IntegrityCheckFailed,
    );
    assert!(conn.is_autocommit());
    assert!(!exists(&conn, "migration_fk_parent"));
    assert!(!exists(&conn, "migration_fk_child"));
    assert_initial(&conn, STAMP);
    drop(conn);
    let reopened = temp.open();
    let conn = reopened.conn.lock().unwrap();
    assert_file_policy(&conn);
    assert_initial(&conn, STAMP);
    assert!(!exists(&conn, "migration_fk_parent"));
    assert!(!exists(&conn, "migration_fk_child"));
}

#[test]
fn postmigration_quick_check_gate_rolls_back_check_violation_and_version_row() {
    let temp = TempStore::new();
    let mut conn = Connection::open(&temp.path).unwrap();
    configure_file_connection(&conn);
    let catalog = [
        initial(),
        migration(
            2,
            "0002_bad_check",
            "
        CREATE TABLE migration_bad_check (value INTEGER CHECK(value > 0));
        PRAGMA ignore_check_constraints=ON;
        INSERT INTO migration_bad_check VALUES (-1);
        PRAGMA ignore_check_constraints=OFF;
    ",
        ),
    ];
    assert_error(
        migrate::apply(&mut conn, &catalog, instant(), true),
        StoreError::IntegrityCheckFailed,
    );
    assert!(conn.is_autocommit());
    assert!(!exists(&conn, "migration_bad_check"));
    assert_initial(&conn, STAMP);
    drop(conn);
    let reopened = temp.open();
    let conn = reopened.conn.lock().unwrap();
    assert_file_policy(&conn);
    assert_initial(&conn, STAMP);
    assert!(!exists(&conn, "migration_bad_check"));
}

#[test]
fn transaction_success_and_typed_error_are_atomic_in_file_and_after_reopen() {
    let temp = TempStore::new();
    {
        let store = temp.open();
        let returned = store
            .transact(|tx| {
                tx.inner.execute_batch(
                    "CREATE TABLE tx_values (value INTEGER); INSERT INTO tx_values VALUES (7);",
                )?;
                Ok(123_u32)
            })
            .unwrap();
        assert_eq!(returned, 123);
        let failed: Result<(), StoreError> = store.transact(|tx| {
            tx.inner.execute("INSERT INTO tx_values VALUES (8)", [])?;
            tx.inner
                .execute_batch("CREATE TABLE rolled_back_ddl (value INTEGER)")?;
            Err(StoreError::ConnectionPolicy)
        });
        assert_error(failed, StoreError::ConnectionPolicy);
        let conn = store.conn.lock().unwrap();
        assert_eq!(scalar(&conn, "SELECT sum(value) FROM tx_values"), 7);
        assert!(!exists(&conn, "rolled_back_ddl"));
        assert!(conn.is_autocommit());
    }
    let store = temp.open();
    assert_eq!(
        scalar(
            &store.conn.lock().unwrap(),
            "SELECT sum(value) FROM tx_values"
        ),
        7
    );
    assert!(!exists(&store.conn.lock().unwrap(), "rolled_back_ddl"));
    store.verify_integrity().unwrap();
}

#[test]
fn production_foreign_key_restrict_and_cascade_transactions_survive_rollback_and_reopen() {
    const TASK: &str = "tsk_00000000000000000000000000";
    const STEP: &str = "stp_00000000000000000000000000";
    const RECEIPT: &str = "rcp_00000000000000000000000000";
    fn assert_owned_rows(conn: &Connection, expected: i64) {
        for table in [
            "tasks",
            "task_steps",
            "plan_revisions",
            "task_blob_refs",
            "step_blob_refs",
            "leases",
            "side_effect_receipts",
            "task_journal",
        ] {
            assert_eq!(
                scalar(conn, &format!("SELECT count(*) FROM {table}")),
                expected,
                "{table}"
            );
        }
    }
    let temp = TempStore::new();
    let digest = Migrations::checksum("durable cascade blob");
    let key = format!("idk_{}", "0".repeat(64));
    {
        let store = temp.open();
        store.transact(|tx| {
            tx.inner.execute("INSERT INTO blobs (digest, data_class_rank, protection, size_bytes, content)
                VALUES (?1, 0, 'NONE', 1, X'00')", [digest.as_str()])?;
            tx.inner.execute("INSERT INTO tasks (task_id, kind, title, state, origin_kind,
                data_class_rank, policy_class_rank, created_at_ms, updated_at_ms,
                max_model_calls, max_tool_calls, max_attempts_per_step)
                VALUES (?1, 'USER_REQUEST', 'durable cascade', 'READY', 'USER', 0, 0, 0, 0, 3, 3, 3)", [TASK])?;
            tx.inner.execute("INSERT INTO task_steps (step_id, task_id, sequence, kind, status,
                attempt, provider_id, capability_id, capability_version, idempotency_key,
                input_digest, result_digest, started_at_ms, completed_at_ms, lease_generation)
                VALUES (?1, ?2, 0, 'CAPABILITY', 'SUCCEEDED', 1, 'device', 'device.fs.read',
                '1.0.0', ?3, ?4, ?4, 0, 1, 1)", params![STEP, TASK, &key, digest.as_str()])?;
            tx.inner.execute("INSERT INTO plan_revisions VALUES (?1, 0, 0, ?2, 0, 1)", params![TASK, digest.as_str()])?;
            tx.inner.execute("INSERT INTO task_blob_refs VALUES (?1, 'PLAN', ?2, 0)", params![TASK, digest.as_str()])?;
            tx.inner.execute("INSERT INTO step_blob_refs VALUES (?1, 'RESULT', ?2, 0)", params![STEP, digest.as_str()])?;
            tx.inner.execute("INSERT INTO leases VALUES (?1, 'host', 1, 0, 2, 1)", [STEP])?;
            tx.inner.execute("INSERT INTO side_effect_receipts (receipt_id, task_id, step_id,
                capability_id, idempotency_key, effect_summary, observed_at_ms, replay_safe, data_class_rank)
                VALUES (?1, ?2, ?3, 'device.fs.read', ?4, 'durable receipt', 1, 1, 0)", params![RECEIPT, TASK, STEP, &key])?;
            tx.inner.execute("INSERT INTO task_journal (journal_id, task_id, step_id, journal_seq,
                journal_kind, actor_kind, actor_id, actor_version, data_class_rank, occurred_at_ms)
                VALUES ('durable-journal', ?1, ?2, 1, 'STEP_COMMITTED', 'HOST', 'host', '1.0.0', 0, 1)", params![TASK, STEP])?;
            Ok(())
        }).unwrap();
        assert_owned_rows(&store.conn.lock().unwrap(), 1);
        store.verify_integrity().unwrap();
    }
    {
        let store = temp.open();
        assert_file_policy(&store.conn.lock().unwrap());
        assert_owned_rows(&store.conn.lock().unwrap(), 1);
        assert_error(
            store.transact(|tx| {
                tx.inner.execute(
                    "UPDATE tasks SET title='must roll back' WHERE task_id=?1",
                    [TASK],
                )?;
                tx.inner.execute(
                    "DELETE FROM blobs WHERE digest=?1 AND data_class_rank=0",
                    [digest.as_str()],
                )?;
                Ok(())
            }),
            StoreError::ConstraintViolation,
        );
        assert_eq!(
            strings(&store.conn.lock().unwrap(), "SELECT title FROM tasks"),
            ["durable cascade"]
        );
        assert_owned_rows(&store.conn.lock().unwrap(), 1);
        let rollback: Result<(), StoreError> = store.transact(|tx| {
            assert_eq!(
                tx.inner
                    .execute("DELETE FROM tasks WHERE task_id=?1", [TASK])?,
                1
            );
            assert_owned_rows(&tx.inner, 0);
            Err(StoreError::ConnectionPolicy)
        });
        assert_error(rollback, StoreError::ConnectionPolicy);
        assert_owned_rows(&store.conn.lock().unwrap(), 1);
        assert_eq!(
            scalar(&store.conn.lock().unwrap(), "SELECT count(*) FROM blobs"),
            1
        );
    }
    {
        let store = temp.open();
        assert_owned_rows(&store.conn.lock().unwrap(), 1);
        assert_eq!(
            strings(&store.conn.lock().unwrap(), "SELECT title FROM tasks"),
            ["durable cascade"]
        );
        store
            .transact(|tx| {
                assert_eq!(
                    tx.inner
                        .execute("DELETE FROM tasks WHERE task_id=?1", [TASK])?,
                    1
                );
                assert_owned_rows(&tx.inner, 0);
                Ok(())
            })
            .unwrap();
        store.verify_integrity().unwrap();
    }
    {
        let store = temp.open();
        assert_owned_rows(&store.conn.lock().unwrap(), 0);
        assert_eq!(
            strings(
                &store.conn.lock().unwrap(),
                "SELECT hex(content) FROM blobs"
            ),
            ["00"]
        );
        store
            .transact(|tx| {
                assert_eq!(
                    tx.inner.execute(
                        "DELETE FROM blobs WHERE digest=?1 AND data_class_rank=0",
                        [digest.as_str()]
                    )?,
                    1
                );
                Ok(())
            })
            .unwrap();
    }
    let reopened = temp.open();
    assert_owned_rows(&reopened.conn.lock().unwrap(), 0);
    assert_eq!(
        scalar(&reopened.conn.lock().unwrap(), "SELECT count(*) FROM blobs"),
        0
    );
    reopened.verify_integrity().unwrap();
}

#[test]
fn transact_reserves_writer_immediately_and_maps_contention_to_busy() {
    let temp = TempStore::new();
    let store = temp.open();
    let other = Connection::open(&temp.path).unwrap();
    other.busy_timeout(Duration::from_millis(10)).unwrap();
    store
        .conn
        .lock()
        .unwrap()
        .busy_timeout(Duration::from_millis(10))
        .unwrap();
    store
        .transact(|_tx| {
            // No write precedes the competing BEGIN: this distinguishes IMMEDIATE from DEFERRED.
            assert_error(
                other
                    .execute_batch("BEGIN IMMEDIATE")
                    .map_err(StoreError::from),
                StoreError::Busy,
            );
            assert!(other.is_autocommit());
            Ok(())
        })
        .unwrap();
    other.execute_batch("BEGIN IMMEDIATE").unwrap();
    let mut called = false;
    assert_error(
        store.transact(|_tx| {
            called = true;
            Ok(())
        }),
        StoreError::Busy,
    );
    assert!(!called);
    other.execute_batch("ROLLBACK").unwrap();
    store.transact(|_tx| Ok(())).unwrap();
}

#[test]
fn deferred_foreign_key_failure_at_real_commit_is_typed_and_rolls_back() {
    let temp = TempStore::new();
    {
        let store = temp.open();
        store.conn.lock().unwrap().execute_batch("
            CREATE TABLE tx_parent (id INTEGER PRIMARY KEY);
            CREATE TABLE tx_child (id INTEGER REFERENCES tx_parent(id) DEFERRABLE INITIALLY DEFERRED);
        ").unwrap();
        let mut closure_completed = false;
        assert_error(
            store.transact(|tx| {
                tx.inner.execute("INSERT INTO tx_child VALUES (42)", [])?;
                closure_completed = true;
                Ok(())
            }),
            StoreError::ConstraintViolation,
        );
        assert!(closure_completed, "must fail at COMMIT, not at INSERT");
        assert_eq!(
            scalar(&store.conn.lock().unwrap(), "SELECT count(*) FROM tx_child"),
            0
        );
        assert!(store.conn.lock().unwrap().is_autocommit());
        store
            .transact(|tx| {
                tx.inner.execute("INSERT INTO tx_parent VALUES (42)", [])?;
                tx.inner.execute("INSERT INTO tx_child VALUES (42)", [])?;
                Ok(())
            })
            .unwrap();
    }
    let store = temp.open();
    assert_eq!(
        scalar(&store.conn.lock().unwrap(), "SELECT count(*) FROM tx_child"),
        1
    );
    store.verify_integrity().unwrap();
}

#[test]
fn independent_stores_share_commits_but_have_separate_connections_and_writer_reservations() {
    let temp = TempStore::new();
    let first = temp.open();
    let second = temp.open();
    second
        .conn
        .lock()
        .unwrap()
        .busy_timeout(Duration::from_millis(10))
        .unwrap();
    assert_eq!(
        scalar(&first.conn.lock().unwrap(), "PRAGMA busy_timeout"),
        5000
    );
    assert_eq!(
        scalar(&second.conn.lock().unwrap(), "PRAGMA busy_timeout"),
        10
    );
    first
        .transact(|tx| {
            tx.inner.execute_batch(
                "CREATE TABLE shared_values (value INTEGER); INSERT INTO shared_values VALUES (1)",
            )?;
            Ok(())
        })
        .unwrap();
    assert_eq!(
        scalar(
            &second.conn.lock().unwrap(),
            "SELECT sum(value) FROM shared_values"
        ),
        1
    );
    second
        .transact(|tx| {
            tx.inner
                .execute("INSERT INTO shared_values VALUES (2)", [])?;
            Ok(())
        })
        .unwrap();
    assert_eq!(
        scalar(
            &first.conn.lock().unwrap(),
            "SELECT sum(value) FROM shared_values"
        ),
        3
    );
    first
        .transact(|tx| {
            tx.inner
                .execute("INSERT INTO shared_values VALUES (3)", [])?;
            assert_eq!(
                scalar(
                    &second.conn.lock().unwrap(),
                    "SELECT sum(value) FROM shared_values"
                ),
                3
            );
            let mut called = false;
            assert_error(
                second.transact(|_tx| {
                    called = true;
                    Ok(())
                }),
                StoreError::Busy,
            );
            assert!(!called);
            Ok(())
        })
        .unwrap();
    assert_eq!(
        scalar(
            &second.conn.lock().unwrap(),
            "SELECT sum(value) FROM shared_values"
        ),
        6
    );
    assert_initial(&first.conn.lock().unwrap(), STAMP);
    assert_initial(&second.conn.lock().unwrap(), STAMP);
    first.verify_integrity().unwrap();
    second.verify_integrity().unwrap();
    drop(second);
    assert_eq!(
        first.checkpoint_for_close().unwrap(),
        CheckpointOutcome::Complete
    );
    drop(first);
    let reopened = temp.open();
    assert_eq!(
        scalar(
            &reopened.conn.lock().unwrap(),
            "SELECT sum(value) FROM shared_values"
        ),
        6
    );
}

#[test]
fn concurrent_fresh_open_commits_one_catalog_and_busy_opener_can_retry() {
    let temp = TempStore::new();
    assert!(!temp.path.exists());
    let barrier = Barrier::new(3);
    let results = std::thread::scope(|scope| {
        let first = scope.spawn(|| {
            barrier.wait();
            Store::open(&temp.path, &FixedClock(STAMP))
        });
        let second = scope.spawn(|| {
            barrier.wait();
            Store::open(&temp.path, &FixedClock(STAMP))
        });
        barrier.wait();
        [first.join().unwrap(), second.join().unwrap()]
    });
    assert!(
        results.iter().any(Result::is_ok),
        "at least one fresh opener must initialize"
    );
    let stores: Vec<_> = results
        .into_iter()
        .map(|result| match result {
            Ok(store) => store,
            // A bounded writer/journal contention refusal is retryable once both opens finish.
            Err(StoreError::Busy) => temp.open(),
            Err(other) => panic!("concurrent fresh opener unexpectedly refused: {other:?}"),
        })
        .collect();
    for store in &stores {
        assert_eq!(store.schema_version().unwrap(), 2);
        assert_latest(&store.conn.lock().unwrap(), STAMP);
        store.verify_integrity().unwrap();
    }
    drop(stores);
    let reopened = temp.open();
    assert_latest(&reopened.conn.lock().unwrap(), STAMP);
    reopened.verify_integrity().unwrap();
}

#[test]
fn truncate_checkpoint_reports_busy_with_reader_snapshot_then_retry_completes() {
    let temp = TempStore::new();
    let store = temp.open();
    store
        .conn
        .lock()
        .unwrap()
        .busy_timeout(Duration::from_millis(10))
        .unwrap();
    store.transact(|tx| {
        tx.inner.execute_batch("CREATE TABLE checkpoint_values (value INTEGER); INSERT INTO checkpoint_values VALUES (1)")?;
        Ok(())
    }).unwrap();
    assert_eq!(
        store.checkpoint_for_close().unwrap(),
        CheckpointOutcome::Complete
    );
    let reader = Connection::open(&temp.path).unwrap();
    reader.execute_batch("BEGIN").unwrap();
    assert_eq!(
        scalar(&reader, "SELECT sum(value) FROM checkpoint_values"),
        1
    );
    store
        .transact(|tx| {
            tx.inner
                .execute("INSERT INTO checkpoint_values VALUES (2)", [])?;
            Ok(())
        })
        .unwrap();
    assert_eq!(
        scalar(&reader, "SELECT sum(value) FROM checkpoint_values"),
        1
    );
    assert_error(store.checkpoint_for_close(), StoreError::Busy);
    reader.execute_batch("ROLLBACK").unwrap();
    assert_eq!(
        store.checkpoint_for_close().unwrap(),
        CheckpointOutcome::Complete
    );
    assert_eq!(
        store.checkpoint_for_close().unwrap(),
        CheckpointOutcome::Complete
    );
    let wal = temp.path.with_file_name("store.sqlite-wal");
    if wal.exists() {
        assert_eq!(fs::metadata(wal).unwrap().len(), 0);
    }
    drop(reader);
    drop(store);
    let reopened = temp.open();
    assert_eq!(
        scalar(
            &reopened.conn.lock().unwrap(),
            "SELECT sum(value) FROM checkpoint_values"
        ),
        3
    );
}

#[test]
fn normal_open_quick_check_accepts_fk_orphan_but_explicit_integrity_rejects_it() {
    let temp = TempStore::new();
    {
        let store = temp.open();
        store
            .conn
            .lock()
            .unwrap()
            .execute_batch(
                "
            CREATE TABLE integrity_parent (id INTEGER PRIMARY KEY);
            CREATE TABLE integrity_child (id INTEGER REFERENCES integrity_parent(id));
        ",
            )
            .unwrap();
        store.checkpoint_for_close().unwrap();
    }
    {
        let conn = Connection::open(&temp.path).unwrap();
        conn.pragma_update(None, "foreign_keys", false).unwrap();
        conn.execute("INSERT INTO integrity_child VALUES (99)", [])
            .unwrap();
        assert_eq!(strings(&conn, "PRAGMA quick_check"), ["ok"]);
        assert_eq!(strings(&conn, "PRAGMA integrity_check"), ["ok"]);
        let mut statement = conn.prepare("PRAGMA foreign_key_check").unwrap();
        assert!(statement.query([]).unwrap().next().unwrap().is_some());
    }
    let store = temp.open();
    assert_error(store.verify_integrity(), StoreError::IntegrityCheckFailed);
    store
        .transact(|tx| {
            tx.inner.execute("DELETE FROM integrity_child", [])?;
            Ok(())
        })
        .unwrap();
    store.verify_integrity().unwrap();
}

#[test]
fn quick_check_damage_is_refused_before_wal_and_preserves_main_bytes() {
    let temp = TempStore::new();
    {
        let conn = seed(&temp.path);
        conn.execute_batch(
            "
            CREATE TABLE damaged_checks (value INTEGER CHECK(value > 0));
            PRAGMA ignore_check_constraints=ON;
            INSERT INTO damaged_checks VALUES (-1), (-2);
            PRAGMA ignore_check_constraints=OFF;
        ",
        )
        .unwrap();
        let messages = strings(&conn, "PRAGMA quick_check");
        assert!(messages.iter().any(|message| message != "ok"));
        assert_eq!(strings(&conn, "PRAGMA journal_mode"), ["delete"]);
    }
    refuse_unchanged(&temp.path, StoreError::IntegrityCheckFailed);
    let conn = Connection::open(&temp.path).unwrap();
    assert_eq!(strings(&conn, "PRAGMA journal_mode"), ["delete"]);
}

#[test]
fn page_check_requires_every_returned_row_to_be_ok_and_at_least_one_row() {
    let temp = TempStore::new();
    let store = temp.open();
    let conn = store.conn.lock().unwrap();
    assert_file_policy(&conn);
    let sql = format!("SELECT 'ok' UNION ALL SELECT '{SENTINEL}' UNION ALL SELECT 'ok'");
    assert_eq!(strings(&conn, &sql), ["ok", SENTINEL, "ok"]);
    let failure = error(migrate::page_check(&conn, &sql));
    assert_redacted(&failure);
    assert_error::<()>(Err(failure), StoreError::IntegrityCheckFailed);
    migrate::page_check(&conn, "SELECT 'ok' UNION ALL SELECT 'ok'").unwrap();
    migrate::page_check(&conn, "SELECT 'ok'").unwrap();
    for sql in [
        "SELECT 'ok' WHERE 0",
        "SELECT 'ok' UNION ALL SELECT NULL",
        "SELECT 'ok' UNION ALL SELECT 42",
        "SELECT * FROM nonexistent_page_check_table",
    ] {
        assert_error(
            migrate::page_check(&conn, sql),
            StoreError::IntegrityCheckFailed,
        );
    }
}

fn assert_redacted(error: &StoreError) {
    assert!(
        std::error::Error::source(error).is_none(),
        "storage error exposed a source chain"
    );
    for rendered in [format!("{error}"), format!("{error:?}")] {
        assert!(
            !rendered.contains(SENTINEL),
            "untrusted detail leaked: {rendered}"
        );
        assert!(!rendered.is_empty());
    }
}

#[test]
fn raw_sqlite_errors_are_typed_and_display_debug_redact_untrusted_details() {
    for (code, expected) in [
        (rusqlite::ffi::SQLITE_BUSY, StoreError::Busy),
        (rusqlite::ffi::SQLITE_LOCKED, StoreError::Busy),
        (
            rusqlite::ffi::SQLITE_CONSTRAINT_UNIQUE,
            StoreError::ConstraintViolation,
        ),
        (rusqlite::ffi::SQLITE_ERROR, StoreError::Sqlite),
    ] {
        let mapped = StoreError::from(rusqlite::Error::SqliteFailure(
            rusqlite::ffi::Error::new(code),
            Some(SENTINEL.into()),
        ));
        assert_redacted(&mapped);
        assert_error::<()>(Err(mapped), expected);
    }
    for raw in [
        rusqlite::Error::InvalidParameterName(SENTINEL.into()),
        rusqlite::Error::InvalidColumnName(SENTINEL.into()),
        rusqlite::Error::InvalidPath(PathBuf::from(SENTINEL)),
    ] {
        let mapped = StoreError::from(raw);
        assert_redacted(&mapped);
        assert_error::<()>(Err(mapped), StoreError::Sqlite);
    }
    let store = Store::open_in_memory(&FixedClock(STAMP)).unwrap();
    store
        .conn
        .lock()
        .unwrap()
        .execute_batch(&format!(
            "
        CREATE TABLE error_values (value TEXT UNIQUE);
        CREATE TRIGGER sentinel_trigger BEFORE INSERT ON error_values
          WHEN NEW.value='trigger' BEGIN SELECT RAISE(ABORT, '{SENTINEL}'); END;
    "
        ))
        .unwrap();
    for value in ["trigger", SENTINEL] {
        if value == SENTINEL {
            store
                .transact(|tx| {
                    tx.inner
                        .execute("INSERT INTO error_values VALUES (?1)", [value])?;
                    Ok(())
                })
                .unwrap();
        }
        let mapped = error(store.transact(|tx| {
            let raw = tx
                .inner
                .execute("INSERT INTO error_values VALUES (?1)", [value])
                .unwrap_err();
            if value == "trigger" {
                assert!(
                    raw.to_string().contains(SENTINEL),
                    "trigger must exercise a real untrusted SQLite message"
                );
            }
            Err::<(), _>(StoreError::from(raw))
        }));
        assert_redacted(&mapped);
        assert_error::<()>(Err(mapped), StoreError::ConstraintViolation);
    }
    let mapped = error(store.transact(|tx| {
        let raw = tx
            .inner
            .execute(&format!("INSERT INTO \"{SENTINEL}\" VALUES (1)"), [])
            .unwrap_err();
        assert!(format!("{raw:?}").contains(SENTINEL));
        Err::<(), _>(StoreError::from(raw))
    }));
    assert_redacted(&mapped);
    assert_error::<()>(Err(mapped), StoreError::Sqlite);
}

#[test]
fn every_payload_free_error_has_redacted_display_and_debug() {
    let protocol = ProtocolError::Serialization {
        surface: SENTINEL,
        reason: SerializationRejection::MalformedJson,
    };
    assert!(format!("{protocol}").contains(SENTINEL));
    assert!(format!("{protocol:?}").contains(SENTINEL));
    for error in [
        StoreError::Open,
        StoreError::Sqlite,
        StoreError::NotSereaStore,
        StoreError::SchemaTooNew,
        StoreError::MigrationChecksumMismatch,
        StoreError::MigrationCatalogInvalid,
        StoreError::IntegrityCheckFailed,
        StoreError::Busy,
        StoreError::ConstraintViolation,
        StoreError::ConnectionPolicy,
        StoreError::UnsupportedSqlite,
        StoreError::LockPoisoned,
        StoreError::Clock(protocol),
    ] {
        assert_redacted(&error);
    }
}

#[test]
fn clock_failure_is_typed_before_file_creation_and_does_not_touch_existing_bytes() {
    struct FailingClock;
    impl Clock for FailingClock {
        fn now_ms(&self) -> Result<EpochMillis, ProtocolError> {
            EpochMillis::new(EpochMillis::MIN - 1)
        }
    }
    let expected = EpochMillis::new(EpochMillis::MIN - 1).unwrap_err();
    let temp = TempStore::new();
    for existing in [false, true] {
        if existing {
            fs::write(&temp.path, SENTINEL).unwrap();
        }
        match error(Store::open(&temp.path, &FailingClock)) {
            StoreError::Clock(actual) => assert_eq!(actual, expected),
            other => panic!("expected Clock, got {other:?}"),
        }
        if existing {
            assert_eq!(fs::read(&temp.path).unwrap(), SENTINEL.as_bytes());
        } else {
            assert!(!temp.path.exists());
            assert_eq!(fs::read_dir(&temp.dir).unwrap().count(), 0);
        }
    }
    match error(Store::open_in_memory(&FailingClock)) {
        StoreError::Clock(actual) => assert_eq!(actual, expected),
        other => panic!("expected Clock, got {other:?}"),
    }
}

#[test]
fn store_owns_no_clock_borrow_and_reads_injected_clock_once_per_open() {
    struct CountingClock(AtomicU64);
    impl Clock for CountingClock {
        fn now_ms(&self) -> Result<EpochMillis, ProtocolError> {
            assert_eq!(self.0.fetch_add(1, Ordering::Relaxed), 0);
            EpochMillis::new(STAMP)
        }
    }
    let temp = TempStore::new();
    let store = {
        let clock = CountingClock(AtomicU64::new(0));
        let store = Store::open(&temp.path, &clock).unwrap();
        assert_eq!(clock.0.load(Ordering::Relaxed), 1);
        store
    };
    store.schema_version().unwrap();
    store.verify_integrity().unwrap();
    store.transact(|_tx| Ok(())).unwrap();
    store.checkpoint_for_close().unwrap();
    let memory = {
        let clock = CountingClock(AtomicU64::new(0));
        Store::open_in_memory(&clock).unwrap()
    };
    memory.verify_integrity().unwrap();
}

#[test]
fn panic_unwinds_transaction_rolls_back_and_poison_is_typed_not_another_panic() {
    let temp = TempStore::new();
    let store = temp.open();
    store
        .conn
        .lock()
        .unwrap()
        .execute_batch("CREATE TABLE panic_values (value INTEGER)")
        .unwrap();
    let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _: Result<(), StoreError> = store.transact(|tx| {
            tx.inner
                .execute("INSERT INTO panic_values VALUES (42)", [])?;
            tx.inner
                .execute_batch("CREATE TABLE panic_ddl (value INTEGER)")?;
            panic!("test-only transaction unwind");
        });
    }));
    assert!(panic.is_err());
    {
        let conn = match store.conn.lock() {
            Ok(_) => panic!("unwind while holding connection should poison mutex"),
            Err(poisoned) => poisoned.into_inner(),
        };
        assert!(conn.is_autocommit());
        assert_eq!(scalar(&conn, "SELECT count(*) FROM panic_values"), 0);
        assert!(!exists(&conn, "panic_ddl"));
    }
    assert_error(store.schema_version(), StoreError::LockPoisoned);
    assert_error(store.verify_integrity(), StoreError::LockPoisoned);
    assert_error(store.transact(|_tx| Ok(())), StoreError::LockPoisoned);
    assert_error(store.checkpoint_for_close(), StoreError::LockPoisoned);
    drop(store);
    let reopened = temp.open();
    assert_eq!(
        scalar(
            &reopened.conn.lock().unwrap(),
            "SELECT count(*) FROM panic_values"
        ),
        0
    );
    assert!(!exists(&reopened.conn.lock().unwrap(), "panic_ddl"));
    reopened.verify_integrity().unwrap();
}
