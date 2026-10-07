//! P3B RED-first migration and event-foundation contract tests.

use crate::{Migrations, Store, StoreError};
use rusqlite::Connection;
use serea_protocol::{Clock, EpochMillis, ProtocolError};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

const INITIAL_SQL: &str = include_str!("../migrations/0001_initial.sql");
const EVENT_SCHEDULER_SQL: &str = include_str!("../migrations/0002_event_scheduler.sql");
static NEXT: AtomicU64 = AtomicU64::new(0);

struct FixedClock;
impl Clock for FixedClock {
    fn now_ms(&self) -> Result<EpochMillis, ProtocolError> {
        EpochMillis::new(-1)
    }
}

struct TempDb(PathBuf);
impl TempDb {
    fn new() -> Self {
        let dir = std::env::temp_dir().join(format!(
            "serea-p3b-migration-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&dir).unwrap();
        Self(dir.join("store.sqlite"))
    }
}
impl Drop for TempDb {
    fn drop(&mut self) {
        std::fs::remove_dir_all(self.0.parent().unwrap()).unwrap();
    }
}

fn table_exists(conn: &Connection, name: &str) -> bool {
    conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_schema WHERE type='table' AND name=?1)",
        [name],
        |row| row.get(0),
    )
    .unwrap()
}

#[test]
fn migration_0001_is_immutable_and_catalog_adds_only_0002() {
    assert_eq!(
        Migrations::checksum(INITIAL_SQL).as_str(),
        "sha256:d9068dccbc26ececb71be79c475080633166ba0163c62b2d98b9733512baefea"
    );
    let catalog = Migrations::embedded();
    assert_eq!(catalog.len(), 2);
    assert_eq!((catalog[0].version, catalog[0].name), (1, "0001_initial"));
    assert_eq!(catalog[0].sql, INITIAL_SQL);
    assert_eq!(
        Migrations::checksum(EVENT_SCHEDULER_SQL).as_str(),
        "sha256:4924e69150bbff9c39e2e6b7e2bdd61045202e504900fe0f510d513fbf815e67"
    );
    assert_eq!(
        (catalog[1].version, catalog[1].name),
        (2, "0002_event_scheduler")
    );
    assert_eq!(Migrations::LATEST, 2);
}

#[test]
fn fresh_database_applies_0001_then_0002_and_reopens_with_integrity() {
    let temp = TempDb::new();
    {
        let store = Store::open(&temp.0, &FixedClock).unwrap();
        assert_eq!(store.schema_version().unwrap(), 2);
        let conn = store.conn.lock().unwrap();
        let rows = conn
            .prepare("SELECT version,name,checksum FROM schema_migrations ORDER BY version")
            .unwrap()
            .query_map([], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            })
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!((rows[0].0, rows[0].1.as_str()), (1, "0001_initial"));
        assert_eq!((rows[1].0, rows[1].1.as_str()), (2, "0002_event_scheduler"));
        for name in [
            "event_store_state",
            "event_sequence_ledger",
            "event_content",
            "event_expired_ranges",
            "schedules",
            "schedule_occurrences",
            "device_resume_waits",
            "device_session_resume_wakes",
            "approval_lifecycle_wakes",
            "scheduler_consumer_state",
        ] {
            assert!(table_exists(&conn, name), "missing migration object {name}");
        }
    }
    let reopened = Store::open(&temp.0, &FixedClock).unwrap();
    assert_eq!(reopened.schema_version().unwrap(), 2);
    reopened.verify_integrity().unwrap();
}

#[test]
fn failed_production_0002_rolls_back_all_0002_ddl_and_catalog_row() {
    let temp = TempDb::new();
    {
        let conn = Connection::open(&temp.0).unwrap();
        conn.execute_batch(INITIAL_SQL).unwrap();
        conn.execute(
            "INSERT INTO schema_migrations(version,name,checksum,applied_at_ms) VALUES (1,'0001_initial',?1,-1)",
            [Migrations::checksum(INITIAL_SQL).as_str()],
        )
        .unwrap();
        conn.execute_batch("CREATE TABLE event_content (forced_conflict INTEGER)")
            .unwrap();
    }
    let opened = Store::open(&temp.0, &FixedClock);
    assert!(matches!(opened, Err(StoreError::Sqlite)));

    let conn = Connection::open(&temp.0).unwrap();
    let versions: Vec<i64> = conn
        .prepare("SELECT version FROM schema_migrations ORDER BY version")
        .unwrap()
        .query_map([], |row| row.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(versions, [1]);
    assert!(!table_exists(&conn, "event_store_state"));
    assert!(!table_exists(&conn, "event_sequence_ledger"));
    assert!(!table_exists(&conn, "event_expired_ranges"));
    assert!(table_exists(&conn, "event_content"));
}

#[test]
fn initial_allocator_and_compaction_boundary_are_zero_and_privacy_minimal() {
    let store = Store::open_in_memory(&FixedClock).unwrap();
    let conn = store.conn.lock().unwrap();
    let state: (i64, i64) = conn
        .query_row(
            "SELECT last_allocated_seq, expired_prefix_through FROM event_store_state WHERE singleton=1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert_eq!(state, (0, 0));

    let columns: Vec<String> = conn
        .prepare("SELECT name FROM pragma_table_info('event_sequence_ledger') ORDER BY cid")
        .unwrap()
        .query_map([], |row| row.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(columns, ["seq"]);
    for forbidden in [
        "task_id",
        "step_id",
        "device_id",
        "actor_id",
        "payload_digest",
        "fingerprint",
        "schedule_args",
        "payload",
    ] {
        assert!(!columns.iter().any(|column| column.contains(forbidden)));
    }

    let range_columns: Vec<String> = conn
        .prepare("SELECT name FROM pragma_table_info('event_expired_ranges') ORDER BY cid")
        .unwrap()
        .query_map([], |row| row.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(range_columns, ["first_seq", "last_seq"]);
}

#[test]
fn event_storage_has_no_task_step_delete_cascade_and_content_is_append_only() {
    let store = Store::open_in_memory(&FixedClock).unwrap();
    let conn = store.conn.lock().unwrap();
    let foreign_keys: Vec<(String, String, String)> = conn
        .prepare(
            r#"SELECT "table", "from", on_delete FROM pragma_foreign_key_list('event_content')"#,
        )
        .unwrap()
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert!(foreign_keys.iter().all(|(table, column, action)| {
        !matches!(table.as_str(), "tasks" | "task_steps")
            && !matches!(column.as_str(), "task_id" | "step_id")
            && action != "CASCADE"
    }));

    let sql: String = conn
        .query_row(
            "SELECT sql FROM sqlite_schema WHERE type='trigger' AND name='event_content_no_update'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert!(sql.contains("RAISE(ABORT"));
}

fn event() -> serea_protocol::SereaEvent {
    serde_json::from_str(
        r#"{
          "envelope_version":"1",
          "surface":"serea.event/1",
          "message_id":"evt_01JQ8ZB7H2XKM9P4QW7NRT5YCD",
          "seq":"0",
          "kind":"TASK_CREATED",
          "occurred_at":"2026-10-01T09:14:23.902Z",
          "correlation_id":null,
          "causation_id":null,
          "actor":{"kind":"HOST","id":"serea-core","version":"0.1.0"},
          "data_class":"PUBLIC",
          "trace":null,
          "payload":{"task_id":"tsk_01JQ8Z9K3M7QWXR4V2T6YH0BNA"}
        }"#,
    )
    .unwrap()
}

#[test]
fn sequence_allocation_and_complete_event_content_commit_or_rollback_together() {
    let store = Store::open_in_memory(&FixedClock).unwrap();
    let first = store.transact(|tx| tx.append_event(event(), None)).unwrap();
    assert_eq!(first.seq.get(), 1);
    let failed: Result<(), StoreError> = store.transact(|tx| {
        let second = tx.append_event(event_with_id("evt_01JQ8ZB8H2XKM9P4QW7NRT5YCD"), None)?;
        assert_eq!(second.seq.get(), 2);
        Err(StoreError::Sqlite)
    });
    assert!(failed.is_err());
    let after = store
        .transact(|tx| tx.append_event(event_with_id("evt_01JQ8ZB9H2XKM9P4QW7NRT5YCD"), None))
        .unwrap();
    assert_eq!(after.seq.get(), 2);
    let conn = store.conn.lock().unwrap();
    let state: i64 = conn
        .query_row(
            "SELECT last_allocated_seq FROM event_store_state WHERE singleton=1",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(state, 2);
    let rows: i64 = conn
        .query_row("SELECT count(*) FROM event_content", [], |r| r.get(0))
        .unwrap();
    assert_eq!(rows, 2);
}

fn event_with_id(id: &str) -> serea_protocol::SereaEvent {
    let mut event = event();
    event.message_id = id
        .parse()
        .unwrap_or_else(|error| panic!("invalid test id {id}: {error:?}"));
    event
}

#[test]
fn event_payload_byte_and_transaction_count_bounds_refuse_without_partial_rows() {
    let store = Store::open_in_memory(&FixedClock).unwrap();
    let mut too_large = event();
    too_large.payload.insert(
        "x".to_string(),
        serde_json::Value::String("a".repeat(32_768)),
    );
    assert!(matches!(
        store.transact(|tx| tx.append_event(too_large, None)),
        Err(StoreError::EventPayloadTooLarge)
    ));

    let result = store.transact(|tx| {
        for index in 0..16 {
            let id = format!(
                "evt_01JQ8ZB7H2XKM9P4QW7NRT5YC{}",
                "0123456789ABCDEF".as_bytes()[usize::try_from(index).unwrap()] as char
            );
            tx.append_event(event_with_id(&id), None)?;
        }
        let seventeenth = event_with_id("evt_01JQ8ZB7H2XKM9P4QW7NRT5YCZ");
        tx.append_event(seventeenth, None)?;
        Ok(())
    });
    assert!(matches!(result, Err(StoreError::EventTransactionLimit)));
    let conn = store.conn.lock().unwrap();
    let state: (i64, i64, i64) = conn
        .query_row(
            "SELECT last_allocated_seq,retained_count,event_store_bytes FROM event_store_state WHERE singleton=1",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .unwrap();
    assert_eq!(state, (0, 0, 0));
}

#[test]
fn event_store_capacity_is_a_typed_refusal_and_does_not_allocate() {
    let store = Store::open_in_memory(&FixedClock).unwrap();
    store
        .transact(|tx| {
            tx.inner.execute(
                "UPDATE event_store_state SET event_store_bytes=536870912 WHERE singleton=1",
                [],
            )?;
            Ok(())
        })
        .unwrap();
    assert!(matches!(
        store.transact(|tx| tx.append_event(event(), None)),
        Err(StoreError::EventStoreCapacity)
    ));
    let conn = store.conn.lock().unwrap();
    let state: (i64, i64) = conn
        .query_row(
            "SELECT last_allocated_seq,retained_count FROM event_store_state WHERE singleton=1",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(state, (0, 0));
}

#[test]
fn private_secret_and_credential_event_content_fail_closed_before_sequence_allocation() {
    for (class, expected) in [
        (
            serea_protocol::DataClass::Private,
            StoreError::AtRestProtectionUnavailable,
        ),
        (
            serea_protocol::DataClass::Secret,
            StoreError::EventClassRefused,
        ),
        (
            serea_protocol::DataClass::Credential,
            StoreError::EventClassRefused,
        ),
    ] {
        let store = Store::open_in_memory(&FixedClock).unwrap();
        let mut forbidden = event();
        forbidden.data_class = class;
        assert!(matches!(
            store.transact(|tx| tx.append_event(forbidden, None)),
            Err(error) if error == expected
        ));
        let conn = store.conn.lock().unwrap();
        let state: (i64, i64) = conn
            .query_row(
                "SELECT last_allocated_seq,retained_count FROM event_store_state WHERE singleton=1",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(state, (0, 0));
    }
}

#[test]
fn event_transaction_bound_counts_only_rows_that_survive_nested_savepoints() {
    let store = Store::open_in_memory(&FixedClock).unwrap();
    store
        .transact(|tx| {
            for index in 0..15 {
                let id = format!(
                    "evt_01JQ8ZB7H2XKM9P4QW7NRT5YC{}",
                    "0123456789ABCDE".as_bytes()[usize::try_from(index).unwrap()] as char
                );
                tx.append_event(event_with_id(&id), None)?;
            }
            let rolled_back: Result<(), StoreError> = tx.operation_savepoint(|tx| {
                tx.append_event(event_with_id("evt_01JQ8ZB8H2XKM9P4QW7NRT5YCD"), None)?;
                Err(StoreError::Sqlite)
            });
            assert!(matches!(rolled_back, Err(StoreError::Sqlite)));
            tx.append_event(event_with_id("evt_01JQ8ZB9H2XKM9P4QW7NRT5YCD"), None)?;
            Ok(())
        })
        .unwrap();
    let conn = store.conn.lock().unwrap();
    let state: (i64, i64) = conn
        .query_row(
            "SELECT last_allocated_seq,retained_count FROM event_store_state WHERE singleton=1",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(state, (16, 16));
}

#[test]
fn two_store_connections_serialize_global_sequence_allocation() {
    use std::sync::{Arc, Barrier};

    let temp = TempDb::new();
    let barrier = Arc::new(Barrier::new(2));
    let mut workers = Vec::new();
    for id in [
        "evt_01JQ8ZB7H2XKM9P4QW7NRT5YCD",
        "evt_01JQ8ZB8H2XKM9P4QW7NRT5YCD",
    ] {
        let path = temp.0.clone();
        let barrier = barrier.clone();
        workers.push(std::thread::spawn(move || {
            let store = Store::open(&path, &FixedClock).unwrap();
            barrier.wait();
            store
                .transact(|tx| tx.append_event(event_with_id(id), None))
                .unwrap()
                .seq
                .get()
        }));
    }
    let mut sequences: Vec<_> = workers
        .into_iter()
        .map(|worker| worker.join().unwrap())
        .collect();
    sequences.sort_unstable();
    assert_eq!(sequences, [1, 2]);
    let reopened = Store::open(&temp.0, &FixedClock).unwrap();
    reopened.verify_integrity().unwrap();
}
