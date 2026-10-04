//! Test-only schema-valid fixtures through the real Store migration.
//! No production task/step insertion or raw-SQL API is introduced.

use crate::{LeaseGuard, Store, StoreError, Tx};
use rusqlite::{Connection, params, types::Value};
use serea_protocol::{Clock, DataClass, EpochMillis, LeaseOwner, ProtocolError, StepId, TaskId};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

const TASK: &str = "tsk_01JQ8Z9K3M7QWXR4V2T6YH0BNA";
const OTHER_TASK: &str = "tsk_01JQ8Z9K3M7QWXR4V2T6YH0BNB";
const STEP: &str = "stp_01JQ8Z9M3R2CVN8H5FWK7PQDSF";
const OWNER: &str = "worker-A-private-sentinel";
static NEXT: AtomicU64 = AtomicU64::new(0);

struct Fixed;
impl Clock for Fixed {
    fn now_ms(&self) -> Result<EpochMillis, ProtocolError> {
        EpochMillis::new(0)
    }
}
fn time(n: i64) -> EpochMillis {
    EpochMillis::new(n).unwrap()
}
fn task() -> TaskId {
    TaskId::new(TASK).unwrap()
}
fn step() -> StepId {
    StepId::new(STEP).unwrap()
}
fn owner(s: &str) -> LeaseOwner {
    LeaseOwner::new(s).unwrap()
}
fn fixture(store: &Store, ceiling: u32) {
    let conn = store.conn.lock().unwrap();
    conn.execute("INSERT INTO tasks(task_id,kind,title,state,origin_kind,data_class_rank,policy_class_rank,created_at_ms,updated_at_ms,max_model_calls,max_tool_calls,max_attempts_per_step) VALUES (?1,'USER_REQUEST','lease fixture','READY','USER_MESSAGE',0,0,0,0,0,0,?2)", params![TASK, ceiling]).unwrap();
    conn.execute("INSERT INTO task_steps(step_id,task_id,sequence,kind,status,input_digest) VALUES (?1,?2,0,'NOTIFY','PLANNED',?3)", params![STEP,TASK,format!("sha256:{}", "a".repeat(64))]).unwrap();
}
fn memory(ceiling: u32) -> Store {
    let store = Store::open_in_memory(&Fixed).unwrap();
    fixture(&store, ceiling);
    store
}
fn acquire(
    tx: &mut Tx<'_>,
    who: &str,
    expected: Option<u32>,
    now: i64,
    expiry: i64,
) -> Result<LeaseGuard, StoreError> {
    tx.acquire_lease(
        task(),
        step(),
        owner(who),
        expected,
        time(now),
        time(expiry),
    )
}
fn get(store: &Store, who: &str, expected: Option<u32>, now: i64, expiry: i64) -> LeaseGuard {
    store
        .transact(|tx| acquire(tx, who, expected, now, expiry))
        .unwrap()
}
fn rows(conn: &Connection, sql: &str) -> Vec<Vec<Value>> {
    let mut stmt = conn.prepare(sql).unwrap();
    let columns = stmt.column_count();
    stmt.query_map([], |row| (0..columns).map(|i| row.get(i)).collect())
        .unwrap()
        .map(Result::unwrap)
        .collect()
}
fn snapshot(store: &Store) -> Vec<Vec<Vec<Value>>> {
    let conn = store.conn.lock().unwrap();
    [
        "schema_migrations",
        "tasks",
        "task_steps",
        "leases",
        "blobs",
        "side_effect_receipts",
        "plan_revisions",
        "task_blob_refs",
        "step_blob_refs",
        "task_journal",
    ]
    .iter()
    .map(|table| rows(&conn, &format!("SELECT * FROM {table} ORDER BY 1")))
    .collect()
}
fn step_row(store: &Store) -> Vec<Value> {
    rows(&store.conn.lock().unwrap(), "SELECT status,attempt,lease_generation,lease_owner,lease_expires_at_ms,started_at_ms,result_digest,completed_at_ms FROM task_steps").remove(0)
}
fn lease_row(store: &Store) -> Vec<Value> {
    rows(
        &store.conn.lock().unwrap(),
        "SELECT owner,generation,acquired_at_ms,expires_at_ms,released_at_ms FROM leases",
    )
    .remove(0)
}
fn caught(
    store: &Store,
    who: &str,
    expected: Option<u32>,
    now: i64,
    expiry: i64,
    error: StoreError,
) {
    let before = snapshot(store);
    store
        .transact(|tx| {
            assert_eq!(acquire(tx, who, expected, now, expiry).err(), Some(error));
            Ok(())
        })
        .unwrap();
    assert_eq!(
        snapshot(store),
        before,
        "caught acquisition error must leave all durable rows identical"
    );
}

// Both guards are produced through acquire, never reconstructed or cloned.
// The first escapes an outer rollback; it conveys no authority on its own.
fn rollback_twins(store: &Store) -> (LeaseGuard, LeaseGuard) {
    let mut escaped = None;
    assert_eq!(
        store.transact(|tx| {
            escaped = Some(acquire(tx, OWNER, None, 10, 20)?);
            Err::<(), _>(StoreError::Sqlite)
        }),
        Err(StoreError::Sqlite)
    );
    (escaped.unwrap(), get(store, OWNER, None, 10, 20))
}

// Deliberate test-only duplicate for repeated-use/revocation simulations.
// This module is a private child of lease; no caller constructor/Clone exists.
fn twins(store: &Store) -> (LeaseGuard, LeaseGuard) {
    let current = get(store, OWNER, None, 10, 20);
    let duplicate = LeaseGuard {
        task_id: current.task_id.clone(),
        step_id: current.step_id.clone(),
        owner: current.owner.clone(),
        generation: current.generation,
        origin: current.origin.clone(),
    };
    (duplicate, current)
}

struct FileFixture(PathBuf);
impl FileFixture {
    fn new() -> Self {
        loop {
            let dir = std::env::temp_dir().join(format!(
                "serea-lease-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            match std::fs::create_dir(&dir) {
                Ok(()) => return Self(dir),
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(e) => panic!("test directory: {e}"),
            }
        }
    }
    fn open(&self) -> Store {
        Store::open(&self.0.join("store.sqlite"), &Fixed).unwrap()
    }
}
impl Drop for FileFixture {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).unwrap();
    }
}

#[test]
fn acquire_lease_succeeds_from_planned() {
    let store = memory(2);
    assert_eq!(step_row(&store)[2], Value::Integer(0)); // SQL0 <-> wire None
    let guard = get(&store, OWNER, None, 10, 20);
    assert_eq!(guard.generation(), 1);
    assert_eq!(
        step_row(&store),
        vec![
            Value::Text("LEASED".into()),
            Value::Integer(1),
            Value::Integer(1),
            Value::Text(OWNER.into()),
            Value::Integer(20),
            Value::Null,
            Value::Null,
            Value::Null
        ]
    );
    assert_eq!(
        lease_row(&store),
        vec![
            Value::Text(OWNER.into()),
            Value::Integer(1),
            Value::Integer(10),
            Value::Integer(20),
            Value::Null
        ]
    );
}
#[test]
fn a_competing_acquisition_is_refused() {
    let store = memory(5);
    let _guard = get(&store, OWNER, None, 10, 20);
    caught(&store, "worker-B", Some(1), 15, 30, StoreError::LeaseHeld);
}
#[test]
fn caught_expected_generation_error_cannot_commit_partial_acquisition() {
    let store = memory(5);
    let _guard = get(&store, OWNER, None, 10, 20);
    caught(&store, "worker-B", None, 20, 30, StoreError::LeaseFenced);
}

#[test]
fn same_owner_competing_acquire_is_still_held() {
    let store = memory(5);
    let _guard = get(&store, OWNER, None, 10, 20);
    caught(&store, OWNER, Some(1), 19, 30, StoreError::LeaseHeld);
    caught(&store, OWNER, None, 19, 30, StoreError::LeaseHeld);
}
#[test]
fn invalid_expected_generations_refuse_without_mutation() {
    let store = memory(5);
    caught(&store, OWNER, Some(0), 10, 20, StoreError::LeaseFenced);
    caught(&store, OWNER, Some(1), 10, 20, StoreError::LeaseFenced);
    let _guard = get(&store, OWNER, None, 10, 20);
    for expected in [None, Some(0), Some(2), Some(u32::MAX)] {
        caught(&store, OWNER, expected, 20, 30, StoreError::LeaseFenced);
    }
}
#[test]
fn wrong_task_binding_refuses_and_outer_commit_is_harmless() {
    let store = memory(5);
    let before = snapshot(&store);
    store
        .transact(|tx| {
            assert_eq!(
                tx.acquire_lease(
                    TaskId::new(OTHER_TASK).unwrap(),
                    step(),
                    owner(OWNER),
                    None,
                    time(10),
                    time(20)
                )
                .err(),
                Some(StoreError::LeaseFenced)
            );
            Ok(())
        })
        .unwrap();
    assert_eq!(snapshot(&store), before);
}
#[test]
fn missing_step_refuses_without_inserting_lease() {
    let store = memory(5);
    store
        .conn
        .lock()
        .unwrap()
        .execute("DELETE FROM task_steps", [])
        .unwrap();
    caught(&store, OWNER, None, 10, 20, StoreError::LeaseFenced);
}
#[test]
fn terminal_and_waiting_steps_are_not_eligible() {
    for status in ["SUCCEEDED", "FAILED", "RECONCILED_ABSENT", "WAITING"] {
        let store = memory(5);
        let _guard = get(&store, OWNER, None, 10, 20);
        let conn = store.conn.lock().unwrap();
        conn.execute("UPDATE task_steps SET kind='WAIT_USER',status=?1,lease_owner=NULL,lease_expires_at_ms=NULL,started_at_ms=11,completed_at_ms=CASE WHEN ?1='WAITING' THEN NULL ELSE 12 END,result_digest=CASE WHEN ?1='SUCCEEDED' THEN input_digest ELSE NULL END,error_kind=CASE WHEN ?1='FAILED' THEN 'VALIDATION' END,error_code=CASE WHEN ?1='FAILED' THEN 'INVALID' END,error_message=CASE WHEN ?1='FAILED' THEN 'fixture' END,error_retryable=CASE WHEN ?1='FAILED' THEN 0 END,error_host_action=CASE WHEN ?1='FAILED' THEN 'NONE' END",[status]).unwrap();
        drop(conn);
        caught(&store, OWNER, Some(1), 20, 30, StoreError::LeaseFenced);
    }
}
#[test]
fn invalid_intervals_are_typed_and_failure_atomic() {
    let store = memory(5);
    for expiry in [10, 9, EpochMillis::MIN] {
        caught(
            &store,
            OWNER,
            None,
            10,
            expiry,
            StoreError::InvalidLeaseInterval,
        );
    }
}
#[test]
fn exact_expiry_boundary_allows_reclaim() {
    let store = memory(5);
    let old = get(&store, OWNER, None, 10, 20);
    let current = get(&store, "worker-B", Some(1), 20, 30);
    assert_eq!(current.generation(), 2);
    assert_eq!(lease_row(&store)[1], Value::Integer(2));
    assert_eq!(
        step_row(&store)[1..3],
        [Value::Integer(2), Value::Integer(2)]
    );
    assert_eq!(
        store.transact(|tx| tx.renew_lease(&old, time(20), time(40))),
        Err(StoreError::LeaseFenced)
    );
    assert_eq!(
        store.transact(|tx| tx.release_lease(old, time(20))),
        Err(StoreError::LeaseFenced)
    );
}
#[test]
fn after_expiry_same_owner_reclaim_fences_old_generation() {
    let store = memory(5);
    let old = get(&store, OWNER, None, 10, 20);
    let current = get(&store, OWNER, Some(1), 21, 30);
    assert_eq!(current.generation(), 2);
    let before = snapshot(&store);
    assert_eq!(
        store.transact(|tx| tx.renew_lease(&old, time(21), time(40))),
        Err(StoreError::LeaseFenced)
    );
    assert_eq!(
        store.transact(|tx| tx.release_lease(old, time(21))),
        Err(StoreError::LeaseFenced)
    );
    assert_eq!(snapshot(&store), before);
}
#[test]
fn reclaim_from_executing_clears_started_stamp_without_begin_api() {
    let store = memory(5);
    let _old = get(&store, OWNER, None, 10, 20);
    store
        .conn
        .lock()
        .unwrap()
        .execute(
            "UPDATE task_steps SET status='EXECUTING',started_at_ms=11",
            [],
        )
        .unwrap();
    let _current = get(&store, OWNER, Some(1), 20, 30);
    assert_eq!(step_row(&store)[0], Value::Text("LEASED".into()));
    assert_eq!(
        step_row(&store)[5..],
        [Value::Null, Value::Null, Value::Null]
    );
}
#[test]
fn renew_strictly_extends_authority_not_step_snapshot() {
    let store = memory(5);
    let guard = get(&store, OWNER, None, 10, 20);
    let step_before = step_row(&store);
    store
        .transact(|tx| tx.renew_lease(&guard, time(15), time(30)))
        .unwrap();
    assert_eq!(lease_row(&store)[3], Value::Integer(30));
    assert_eq!(step_row(&store), step_before);
    caught(&store, "worker-B", Some(1), 20, 40, StoreError::LeaseHeld);
}
#[test]
fn renewal_equal_and_shorter_expiry_refuse_without_change() {
    let store = memory(5);
    let guard = get(&store, OWNER, None, 10, 20);
    for expiry in [20, 19, 15, 9] {
        let before = snapshot(&store);
        store
            .transact(|tx| {
                assert_eq!(
                    tx.renew_lease(&guard, time(15), time(expiry)),
                    Err(StoreError::InvalidLeaseInterval)
                );
                Ok(())
            })
            .unwrap();
        assert_eq!(snapshot(&store), before);
    }
}
#[test]
fn renewal_at_and_after_expiry_never_resurrects() {
    let store = memory(5);
    let guard = get(&store, OWNER, None, 10, 20);
    for now in [20, 21] {
        let before = snapshot(&store);
        store
            .transact(|tx| {
                assert_eq!(
                    tx.renew_lease(&guard, time(now), time(30)),
                    Err(StoreError::LeaseExpired)
                );
                Ok(())
            })
            .unwrap();
        assert_eq!(snapshot(&store), before);
    }
}
#[test]
fn release_revokes_without_changing_step_and_repeat_is_fenced() {
    let store = memory(5);
    let (old, guard) = twins(&store);
    let before = step_row(&store);
    store
        .transact(|tx| tx.release_lease(guard, time(15)))
        .unwrap();
    assert_eq!(lease_row(&store)[4], Value::Integer(15));
    assert_eq!(step_row(&store), before);
    assert_eq!(
        store.transact(|tx| tx.renew_lease(&old, time(16), time(30))),
        Err(StoreError::LeaseFenced)
    );
    assert_eq!(
        store.transact(|tx| tx.release_lease(old, time(16))),
        Err(StoreError::LeaseFenced)
    );
    assert_eq!(step_row(&store), before);
}
#[test]
fn released_reacquire_before_ttl_increments_generation_and_resets_revocation() {
    let store = memory(5);
    let (old, guard) = twins(&store);
    store
        .transact(|tx| tx.release_lease(guard, time(15)))
        .unwrap();
    let current = get(&store, OWNER, Some(1), 16, 30);
    assert_eq!(current.generation(), 2);
    assert_eq!(lease_row(&store)[4], Value::Null);
    assert_eq!(
        step_row(&store)[1..3],
        [Value::Integer(2), Value::Integer(2)]
    );
    assert_eq!(
        store.transact(|tx| tx.renew_lease(&old, time(16), time(40))),
        Err(StoreError::LeaseFenced)
    );
    assert_eq!(
        store.transact(|tx| tx.release_lease(old, time(16))),
        Err(StoreError::LeaseFenced)
    );
}
#[test]
fn matching_expired_lease_can_be_explicitly_released() {
    let store = memory(5);
    let guard = get(&store, OWNER, None, 10, 20);
    store
        .transact(|tx| tx.release_lease(guard, time(21)))
        .unwrap();
    assert_eq!(lease_row(&store)[4], Value::Integer(21));
}
#[test]
fn backdated_release_has_typed_refusal_and_no_durable_effect() {
    let store = memory(5);
    let guard = get(&store, OWNER, None, 10, 20);
    let before = snapshot(&store);
    store
        .transact(|tx| {
            assert_eq!(
                tx.release_lease(guard, time(9)),
                Err(StoreError::InvalidLeaseInterval)
            );
            Ok(())
        })
        .unwrap();
    assert_eq!(snapshot(&store), before);
}
#[test]
fn absent_authority_fences_even_with_unchanged_step_copy() {
    let store = memory(5);
    let guard = get(&store, OWNER, None, 10, 20);
    store
        .conn
        .lock()
        .unwrap()
        .execute("DELETE FROM leases", [])
        .unwrap();
    assert_eq!(
        store.transact(|tx| tx.renew_lease(&guard, time(15), time(30))),
        Err(StoreError::LeaseFenced)
    );
    assert_eq!(
        store.transact(|tx| tx.release_lease(guard, time(15))),
        Err(StoreError::LeaseFenced)
    );
}
#[test]
fn dropping_guard_does_not_release_durable_lease() {
    let store = memory(5);
    let guard = get(&store, OWNER, None, 10, 20);
    let before = snapshot(&store);
    drop(guard);
    assert_eq!(snapshot(&store), before);
    caught(&store, OWNER, Some(1), 15, 30, StoreError::LeaseHeld);
}
#[test]
fn guard_escaping_outer_rollback_does_not_authorize_mutation() {
    let store = memory(5);
    let mut guard = None;
    store
        .transact(|tx| {
            guard = Some(acquire(tx, OWNER, None, 10, 20)?);
            Err::<(), _>(StoreError::Sqlite)
        })
        .unwrap_err();
    let guard = guard.unwrap();
    assert_eq!(
        store.transact(|tx| tx.renew_lease(&guard, time(15), time(30))),
        Err(StoreError::LeaseFenced)
    );
    assert_eq!(
        store.transact(|tx| tx.release_lease(guard, time(15))),
        Err(StoreError::LeaseFenced)
    );
}
#[test]
fn ceiling_zero_keeps_planned_and_no_lease_even_if_error_caught() {
    let store = memory(0);
    caught(
        &store,
        OWNER,
        None,
        10,
        20,
        StoreError::AttemptCeilingReached,
    );
    assert_eq!(
        step_row(&store)[0..3],
        [
            Value::Text("PLANNED".into()),
            Value::Integer(0),
            Value::Integer(0)
        ]
    );
    assert!(rows(&store.conn.lock().unwrap(), "SELECT * FROM leases").is_empty());
}
#[test]
fn ceiling_one_refuses_next_eligible_reclaim_without_generation_two() {
    let store = memory(1);
    let _guard = get(&store, OWNER, None, 10, 20);
    caught(
        &store,
        OWNER,
        Some(1),
        20,
        30,
        StoreError::AttemptCeilingReached,
    );
    assert_eq!(lease_row(&store)[1], Value::Integer(1));
}
#[test]
fn ceiling_two_crash_only_loop_spends_acquisitions_not_executions() {
    let store = memory(2);
    drop(get(&store, OWNER, None, 10, 20));
    drop(get(&store, OWNER, Some(1), 20, 30));
    caught(
        &store,
        OWNER,
        Some(2),
        30,
        40,
        StoreError::AttemptCeilingReached,
    );
    assert_eq!(
        step_row(&store)[1..3],
        [Value::Integer(2), Value::Integer(2)]
    );
    assert_eq!(step_row(&store)[5], Value::Null);
    assert_eq!(lease_row(&store)[1], Value::Integer(2));
}
#[test]
fn ceiling_is_read_from_durable_task_not_cached() {
    let store = memory(5);
    let _guard = get(&store, OWNER, None, 10, 20);
    store
        .conn
        .lock()
        .unwrap()
        .execute("UPDATE tasks SET max_attempts_per_step=1", [])
        .unwrap();
    caught(
        &store,
        OWNER,
        Some(1),
        20,
        30,
        StoreError::AttemptCeilingReached,
    );
}
fn set_generation(store: &Store, generation: u32) {
    let conn = store.conn.lock().unwrap();
    conn.execute("UPDATE leases SET generation=?1", [generation])
        .unwrap();
    conn.execute("UPDATE task_steps SET lease_generation=?1", [generation])
        .unwrap();
}
#[test]
fn generation_max_overflow_is_typed_failure_atomic() {
    let store = memory(5);
    let _guard = get(&store, OWNER, None, 10, 20);
    set_generation(&store, u32::MAX);
    caught(
        &store,
        OWNER,
        Some(u32::MAX),
        20,
        30,
        StoreError::LeaseGenerationOverflow,
    );
    caught(&store, OWNER, Some(u32::MAX), 19, 30, StoreError::LeaseHeld);
}
#[test]
fn generation_can_reach_max_and_guard_never_has_zero() {
    let store = memory(5);
    let _guard = get(&store, OWNER, None, 10, 20);
    set_generation(&store, u32::MAX - 1);
    let guard = get(&store, OWNER, Some(u32::MAX - 1), 20, 30);
    assert_eq!(guard.generation(), u32::MAX);
    assert_eq!(lease_row(&store)[1], Value::Integer(i64::from(u32::MAX)));
    assert_eq!(step_row(&store)[2], Value::Integer(i64::from(u32::MAX)));
}
#[test]
fn savepoint_rolls_back_late_sql_failure_even_when_caught() {
    let store = memory(5);
    store.conn.lock().unwrap().execute_batch("CREATE TEMP TRIGGER lease_test_abort BEFORE UPDATE ON task_steps BEGIN SELECT RAISE(ABORT,'private fixture diagnostics'); END;").unwrap();
    caught(&store, OWNER, None, 10, 20, StoreError::ConstraintViolation);
}
#[test]
fn destroyed_savepoint_prevents_outer_success_on_caught_error() {
    let store = memory(5);
    store.conn.lock().unwrap().execute_batch("CREATE TEMP TRIGGER lease_test_rollback BEFORE UPDATE ON task_steps BEGIN SELECT RAISE(ROLLBACK,'private fixture diagnostics'); END;").unwrap();
    let before = snapshot(&store);
    let result = store.transact(|tx| {
        assert!(acquire(tx, OWNER, None, 10, 20).is_err());
        Ok(())
    });
    assert!(
        result.is_err(),
        "lost savepoint/transaction must never report committed Ok"
    );
    assert_eq!(snapshot(&store), before);
}
#[test]
fn acquisition_savepoints_compose_with_release_reacquire_and_blobs() {
    let store = memory(5);
    let (guard, blob) = store
        .transact(|tx| {
            let first = acquire(tx, OWNER, None, 10, 20)?;
            assert_eq!(
                acquire(tx, "worker-B", Some(1), 11, 30).err(),
                Some(StoreError::LeaseHeld)
            );
            tx.release_lease(first, time(12))?;
            let second = acquire(tx, OWNER, Some(1), 13, 30)?;
            let blob = tx.put_blob(b"{}", DataClass::Public)?;
            tx.renew_lease(&second, time(14), time(40))?;
            Ok((second, blob))
        })
        .unwrap();
    assert_eq!(guard.generation(), 2);
    assert_eq!(store.transact(|tx| tx.get_blob(&blob)).unwrap(), b"{}");
}
#[test]
fn lease_error_formatters_and_guard_debug_are_category_only() {
    for (error, name) in [
        (StoreError::LeaseHeld, "LeaseHeld"),
        (StoreError::LeaseFenced, "LeaseFenced"),
        (StoreError::LeaseExpired, "LeaseExpired"),
        (StoreError::AttemptCeilingReached, "AttemptCeilingReached"),
        (
            StoreError::LeaseGenerationOverflow,
            "LeaseGenerationOverflow",
        ),
        (StoreError::InvalidLeaseInterval, "InvalidLeaseInterval"),
    ] {
        assert_eq!(format!("{error}"), name);
        assert_eq!(format!("{error:?}"), name);
        assert!(std::error::Error::source(&error).is_none());
    }
    let store = memory(5);
    let guard = get(&store, OWNER, None, 10, 20);
    assert_eq!(format!("{guard:?}"), "LeaseGuard");
}
#[test]
fn absolute_times_support_complete_signed_epoch_domain() {
    for (now, expiry) in [
        (EpochMillis::MIN, EpochMillis::MIN + 1),
        (-2, -1),
        (EpochMillis::MAX - 1, EpochMillis::MAX),
    ] {
        let store = memory(5);
        let guard = get(&store, OWNER, None, now, expiry);
        store
            .transact(|tx| tx.release_lease(guard, time(now)))
            .unwrap();
    }
}
#[test]
fn two_independent_file_stores_observe_one_authority() {
    let file = FileFixture::new();
    let a = file.open();
    fixture(&a, 5);
    let b = file.open();
    let old = get(&a, OWNER, None, 10, 20);
    caught(&b, "worker-B", Some(1), 15, 30, StoreError::LeaseHeld);
    caught(&b, OWNER, Some(1), 15, 30, StoreError::LeaseHeld);
    let current = get(&b, "worker-B", Some(1), 21, 30);
    assert_eq!(current.generation(), 2);
    assert_eq!(snapshot(&a), snapshot(&b));
    assert_eq!(
        a.transact(|tx| tx.renew_lease(&old, time(21), time(40))),
        Err(StoreError::LeaseFenced)
    );
    assert_eq!(
        a.transact(|tx| tx.release_lease(old, time(21))),
        Err(StoreError::LeaseFenced)
    );
    // Origin publication must not bind a committed guard to one Store handle.
    a.transact(|tx| tx.renew_lease(&current, time(22), time(35)))
        .unwrap();
    b.transact(|tx| tx.renew_lease(&current, time(22), time(40)))
        .unwrap();
    b.transact(|tx| tx.release_lease(current, time(23)))
        .unwrap();
    assert_eq!(lease_row(&a)[4], Value::Integer(23));
    drop(a);
    drop(b);
    let reopened = file.open();
    assert_eq!(lease_row(&reopened)[1], Value::Integer(2));
    let third = get(&reopened, OWNER, Some(2), 24, 50);
    assert_eq!(third.generation(), 3);
}
#[test]
fn rolled_back_initial_guard_cannot_authorize_later_same_owner_acquisition() {
    let store = memory(5);
    let (escaped, current) = rollback_twins(&store);
    assert_eq!(escaped.generation(), current.generation());
    let before = snapshot(&store);
    assert_eq!(
        store.transact(|tx| tx.renew_lease(&escaped, time(15), time(30))),
        Err(StoreError::LeaseFenced)
    );
    assert_eq!(
        store.transact(|tx| tx.release_lease(escaped, time(15))),
        Err(StoreError::LeaseFenced)
    );
    assert_eq!(snapshot(&store), before);
    store
        .transact(|tx| tx.renew_lease(&current, time(15), time(30)))
        .unwrap();
}
#[test]
fn rolled_back_reclaim_guard_cannot_authorize_later_same_owner_reclaim() {
    let store = memory(5);
    drop(get(&store, OWNER, None, 10, 20));
    let mut escaped = None;
    store
        .transact(|tx| {
            escaped = Some(acquire(tx, OWNER, Some(1), 20, 30)?);
            Err::<(), _>(StoreError::Sqlite)
        })
        .unwrap_err();
    let escaped = escaped.unwrap();
    let current = get(&store, OWNER, Some(1), 21, 40);
    assert_eq!(escaped.generation(), current.generation());
    let before = snapshot(&store);
    assert_eq!(
        store.transact(|tx| tx.renew_lease(&escaped, time(22), time(50))),
        Err(StoreError::LeaseFenced)
    );
    assert_eq!(
        store.transact(|tx| tx.release_lease(escaped, time(22))),
        Err(StoreError::LeaseFenced)
    );
    assert_eq!(snapshot(&store), before);
    store
        .transact(|tx| tx.release_lease(current, time(22)))
        .unwrap();
}

#[test]
fn failed_outer_commit_never_publishes_acquisition_capability() {
    let store = memory(5);
    store.conn.lock().unwrap().execute_batch("CREATE TABLE lease_test_parent(id INTEGER PRIMARY KEY); CREATE TABLE lease_test_child(id INTEGER REFERENCES lease_test_parent(id) DEFERRABLE INITIALLY DEFERRED);").unwrap();
    let mut escaped = None;
    assert_eq!(
        store.transact(|tx| {
            escaped = Some(acquire(tx, OWNER, None, 10, 20)?);
            tx.inner
                .execute("INSERT INTO lease_test_child VALUES (99)", [])?;
            Ok(())
        }),
        Err(StoreError::ConstraintViolation)
    );
    let escaped = escaped.unwrap();
    let current = get(&store, OWNER, None, 10, 20);
    assert_eq!(escaped.generation(), current.generation());
    assert_eq!(
        store.transact(|tx| tx.renew_lease(&escaped, time(15), time(30))),
        Err(StoreError::LeaseFenced)
    );
    assert_eq!(
        store.transact(|tx| tx.release_lease(escaped, time(15))),
        Err(StoreError::LeaseFenced)
    );
    store
        .transact(|tx| tx.release_lease(current, time(15)))
        .unwrap();
}
#[test]
fn panicked_origin_never_publishes_guard_after_reopen() {
    let file = FileFixture::new();
    let store = file.open();
    fixture(&store, 5);
    let mut escaped = None;
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = store.transact::<()>(|tx| {
                escaped = Some(acquire(tx, OWNER, None, 10, 20)?);
                panic!("lease test panic");
            });
        }))
        .is_err()
    );
    drop(store);
    let reopened = file.open();
    let escaped = escaped.unwrap();
    let current = get(&reopened, OWNER, None, 10, 20);
    assert_eq!(
        reopened.transact(|tx| tx.renew_lease(&escaped, time(15), time(30))),
        Err(StoreError::LeaseFenced)
    );
    assert_eq!(
        reopened.transact(|tx| tx.release_lease(escaped, time(15))),
        Err(StoreError::LeaseFenced)
    );
    reopened
        .transact(|tx| tx.release_lease(current, time(15)))
        .unwrap();
}

#[test]
fn latest_authoritative_renewal_expiry_controls_second_extension() {
    let store = memory(5);
    let guard = get(&store, OWNER, None, 10, 20);
    store
        .transact(|tx| tx.renew_lease(&guard, time(15), time(30)))
        .unwrap();
    let before = snapshot(&store);
    for expiry in [25, 30] {
        assert_eq!(
            store.transact(|tx| tx.renew_lease(&guard, time(16), time(expiry))),
            Err(StoreError::InvalidLeaseInterval)
        );
        assert_eq!(snapshot(&store), before);
    }
    store
        .transact(|tx| tx.renew_lease(&guard, time(16), time(40)))
        .unwrap();
    assert_eq!(lease_row(&store)[3], Value::Integer(40));
    assert_eq!(step_row(&store)[4], Value::Integer(20));
}
#[test]
fn dead_authority_precedes_invalid_renew_or_release_interval() {
    for reclaim in [false, true] {
        let store = memory(5);
        let (old, guard) = twins(&store);
        if reclaim {
            drop(get(&store, OWNER, Some(1), 20, 30));
        } else {
            store
                .transact(|tx| tx.release_lease(guard, time(15)))
                .unwrap();
        }
        let before = snapshot(&store);
        assert_eq!(
            store.transact(|tx| tx.renew_lease(&old, time(16), time(9))),
            Err(StoreError::LeaseFenced)
        );
        assert_eq!(
            store.transact(|tx| tx.release_lease(old, time(9))),
            Err(StoreError::LeaseFenced)
        );
        assert_eq!(snapshot(&store), before);
    }
}
#[test]
fn released_max_generation_and_stale_ceiling_precedence_are_atomic() {
    let store = memory(5);
    drop(get(&store, OWNER, None, 10, 20));
    set_generation(&store, u32::MAX);
    store
        .conn
        .lock()
        .unwrap()
        .execute("UPDATE leases SET released_at_ms=15", [])
        .unwrap();
    caught(
        &store,
        OWNER,
        Some(u32::MAX),
        16,
        30,
        StoreError::LeaseGenerationOverflow,
    );
    caught(&store, OWNER, Some(1), 16, 30, StoreError::LeaseFenced);
    store
        .conn
        .lock()
        .unwrap()
        .execute("UPDATE tasks SET max_attempts_per_step=1", [])
        .unwrap();
    caught(&store, OWNER, Some(1), 16, 30, StoreError::LeaseFenced);
    caught(
        &store,
        OWNER,
        Some(u32::MAX),
        16,
        30,
        StoreError::AttemptCeilingReached,
    );
}
#[test]
fn release_inner_success_rolled_back_is_not_durable_revocation() {
    let store = memory(5);
    let (duplicate, guard) = twins(&store);
    let before = snapshot(&store);
    assert_eq!(
        store.transact(|tx| {
            tx.release_lease(guard, time(15))?;
            Err::<(), _>(StoreError::Sqlite)
        }),
        Err(StoreError::Sqlite)
    );
    assert_eq!(snapshot(&store), before);
    store
        .transact(|tx| tx.renew_lease(&duplicate, time(16), time(30)))
        .unwrap();
}
#[test]
fn caught_late_acquisition_failure_preserves_unrelated_outer_blob_work() {
    let store = memory(5);
    store.conn.lock().unwrap().execute_batch("CREATE TEMP TRIGGER lease_test_abort BEFORE UPDATE ON task_steps BEGIN SELECT RAISE(ABORT,'private fixture'); END;").unwrap();
    let before = snapshot(&store);
    let (first, second) = store
        .transact(|tx| {
            let first = tx.put_blob(b"1", DataClass::Public)?;
            assert_eq!(
                acquire(tx, OWNER, None, 10, 20).err(),
                Some(StoreError::ConstraintViolation)
            );
            let second = tx.put_blob(b"2", DataClass::Public)?;
            Ok((first, second))
        })
        .unwrap();
    let after = snapshot(&store);
    for index in [0, 1, 2, 3, 5, 6, 7, 8, 9] {
        assert_eq!(after[index], before[index]);
    }
    assert_eq!(after[4].len(), 2);
    assert_eq!(store.transact(|tx| tx.get_blob(&first)).unwrap(), b"1");
    assert_eq!(store.transact(|tx| tx.get_blob(&second)).unwrap(), b"2");
}
#[test]
fn caught_zero_row_step_update_rolls_back_authority_upsert() {
    let store = memory(5);
    store.conn.lock().unwrap().execute_batch("CREATE TEMP TRIGGER lease_test_ignore BEFORE UPDATE ON task_steps BEGIN SELECT RAISE(IGNORE); END;").unwrap();
    caught(&store, OWNER, None, 10, 20, StoreError::LeaseFenced);
}
#[test]
fn caught_post_write_ceiling_refusal_rolls_back_all_savepoint_effects() {
    let store = memory(5);
    store.conn.lock().unwrap().execute_batch("CREATE TEMP TRIGGER lease_test_ceiling AFTER UPDATE ON task_steps BEGIN UPDATE tasks SET max_attempts_per_step=0; END;").unwrap();
    caught(
        &store,
        OWNER,
        None,
        10,
        20,
        StoreError::AttemptCeilingReached,
    );
}
#[test]
fn poisoned_tx_blocks_every_operation_and_prevents_blob_autocommit() {
    let store = memory(5);
    let (borrowed, consumed) = twins(&store);
    let blob = store
        .transact(|tx| tx.put_blob(b"0", DataClass::Public))
        .unwrap();
    store.conn.lock().unwrap().execute_batch("CREATE TEMP TRIGGER lease_test_rollback BEFORE UPDATE ON task_steps BEGIN SELECT RAISE(ROLLBACK,'private fixture'); END;").unwrap();
    let before = snapshot(&store);
    assert_eq!(
        store.transact(|tx| {
            tx.put_blob(b"1", DataClass::Public)?;
            assert_eq!(
                acquire(tx, OWNER, Some(1), 20, 30).err(),
                Some(StoreError::Sqlite)
            );
            assert_eq!(
                tx.put_blob(b"2", DataClass::Public).err(),
                Some(StoreError::Sqlite)
            );
            assert_eq!(tx.get_blob(&blob), Err(StoreError::Sqlite));
            assert_eq!(
                acquire(tx, OWNER, Some(1), 20, 30).err(),
                Some(StoreError::Sqlite)
            );
            assert_eq!(
                tx.renew_lease(&borrowed, time(15), time(30)),
                Err(StoreError::Sqlite)
            );
            assert_eq!(
                tx.release_lease(consumed, time(15)),
                Err(StoreError::Sqlite)
            );
            Ok(())
        }),
        Err(StoreError::Sqlite)
    );
    assert_eq!(snapshot(&store), before);
}
#[test]
fn active_rollback_only_tx_blocks_operations_and_rolls_back_prior_work() {
    let store = memory(5);
    let (borrowed, consumed) = twins(&store);
    let blob = store
        .transact(|tx| tx.put_blob(b"0", DataClass::Public))
        .unwrap();
    let before = snapshot(&store);
    assert_eq!(
        store.transact(|tx| {
            tx.put_blob(b"1", DataClass::Public)?;
            tx.rollback_only = true; // test-only cleanup-failure state with active SQLite Tx
            assert_eq!(
                tx.put_blob(b"2", DataClass::Public).err(),
                Some(StoreError::Sqlite)
            );
            assert_eq!(tx.get_blob(&blob), Err(StoreError::Sqlite));
            assert_eq!(
                acquire(tx, OWNER, Some(1), 20, 30).err(),
                Some(StoreError::Sqlite)
            );
            assert_eq!(
                tx.renew_lease(&borrowed, time(15), time(30)),
                Err(StoreError::Sqlite)
            );
            assert_eq!(
                tx.release_lease(consumed, time(15)),
                Err(StoreError::Sqlite)
            );
            Ok(())
        }),
        Err(StoreError::Sqlite)
    );
    assert_eq!(snapshot(&store), before);
}
#[test]
fn lost_transaction_without_flag_rejects_all_operations() {
    let store = memory(5);
    let (borrowed, consumed) = twins(&store);
    let blob = store
        .transact(|tx| tx.put_blob(b"0", DataClass::Public))
        .unwrap();
    let before = snapshot(&store);
    assert!(
        store
            .transact(|tx| {
                tx.inner.execute_batch("ROLLBACK")?; // private fixture, not an API
                assert!(!tx.rollback_only);
                assert_eq!(
                    tx.put_blob(b"1", DataClass::Public).err(),
                    Some(StoreError::Sqlite)
                );
                assert_eq!(tx.get_blob(&blob), Err(StoreError::Sqlite));
                assert_eq!(
                    acquire(tx, OWNER, Some(1), 20, 30).err(),
                    Some(StoreError::Sqlite)
                );
                assert_eq!(
                    tx.renew_lease(&borrowed, time(15), time(30)),
                    Err(StoreError::Sqlite)
                );
                assert_eq!(
                    tx.release_lease(consumed, time(15)),
                    Err(StoreError::Sqlite)
                );
                Ok(())
            })
            .is_err()
    );
    assert_eq!(snapshot(&store), before);
}
#[test]
fn guard_has_no_wire_serde_or_drop_surface() {
    let source = include_str!("lease.rs");
    assert!(!source.contains("serde::"));
    assert!(!source.contains("impl Drop for LeaseGuard"));
    assert!(!source.contains("derive("));
    let fields = source
        .split("pub struct LeaseGuard {")
        .nth(1)
        .unwrap()
        .split('}')
        .next()
        .unwrap();
    assert!(!fields.contains("pub "));
}

#[test]
fn simultaneous_two_store_acquisition_has_exactly_one_winner() {
    let file = FileFixture::new();
    let a = file.open();
    fixture(&a, 5);
    let b = file.open();
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
    let one = barrier.clone();
    let first = std::thread::spawn(move || {
        one.wait();
        a.transact(|tx| acquire(tx, OWNER, None, 10, 20))
    });
    let second = std::thread::spawn(move || {
        barrier.wait();
        b.transact(|tx| acquire(tx, "worker-B", None, 10, 20))
    });
    let results = [first.join().unwrap(), second.join().unwrap()];
    assert_eq!(results.iter().filter(|r| r.is_ok()).count(), 1);
    assert_eq!(
        results
            .iter()
            .filter(|r| r.as_ref().err() == Some(&StoreError::LeaseHeld))
            .count(),
        1
    );
    let store = file.open();
    assert_eq!(
        step_row(&store)[1..3],
        [Value::Integer(1), Value::Integer(1)]
    );
    assert_eq!(lease_row(&store)[1], Value::Integer(1));
}
