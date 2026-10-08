//! P4B RED-first migration contract tests.

use rusqlite::{Connection, params};
use serea_protocol::{Clock, EpochMillis, ProtocolError};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::{Migrations, Store};

struct FixedClock;
static NEXT: AtomicU64 = AtomicU64::new(0);

impl Clock for FixedClock {
    fn now_ms(&self) -> Result<EpochMillis, ProtocolError> {
        Ok(EpochMillis::new(1_767_225_600_000).unwrap())
    }
}

struct TempDb(PathBuf);

impl TempDb {
    fn new() -> Self {
        let dir = std::env::temp_dir().join(format!(
            "serea-p4b-migration-{}-{}",
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

fn seed_v2(path: &std::path::Path) {
    let conn = Connection::open(path).unwrap();
    conn.pragma_update(None, "foreign_keys", "ON").unwrap();
    for migration in Migrations::embedded().iter().take(2) {
        conn.execute_batch(migration.sql).unwrap();
        conn.execute(
            "INSERT INTO schema_migrations(version,name,checksum,applied_at_ms) VALUES (?1,?2,?3,0)",
            params![migration.version, migration.name, Migrations::checksum(migration.sql).as_str()],
        )
        .unwrap();
    }
}

fn seed_task(conn: &Connection, task_id: &str) {
    conn.execute(
        "INSERT INTO tasks(task_id,kind,title,state,origin_kind,data_class_rank,policy_class_rank,created_at_ms,updated_at_ms,max_model_calls,max_tool_calls,max_attempts_per_step)
         VALUES (?1,'MAINTENANCE','fixture','EXECUTING','SYSTEM',0,0,0,0,12,0,0)",
        [task_id],
    )
    .unwrap();
}

fn insert_intent(
    conn: &Connection,
    request_id: &str,
    task_id: Option<&str>,
) -> rusqlite::Result<usize> {
    conn.execute(
        "INSERT INTO model_call_attempts(
          request_id,task_id,purpose,model_id,provider_id,deployment_class,data_class_rank,state,
          relation_kind,parent_request_id,fallback_from_model_id,accounting_day_utc,cost_class,
          price_revision,input_rate_microusd_per_million,output_rate_microusd_per_million,
          max_context_tokens,effective_max_output_tokens,reserved_cost_usd_micros,
          dispatch_intent_at_ms)
         VALUES (?1,?2,'CHAT','model-a','provider-a','LOCAL',0,'DISPATCH_INTENT',
          'NONE',NULL,NULL,0,'FREE','rev-1',0,0,4096,1024,0,0)",
        params![request_id, task_id],
    )
}

#[test]
fn catalog_adds_0003_without_rewriting_prior_migrations() {
    let catalog = Migrations::embedded();
    assert_eq!(Migrations::LATEST, 3);
    assert_eq!(catalog.len(), 3);
    assert_eq!(catalog[2].version, 3);
    assert_eq!(catalog[2].name, "0003_model_accounting");
    assert_eq!(
        Migrations::checksum(catalog[0].sql).as_str(),
        "sha256:d9068dccbc26ececb71be79c475080633166ba0163c62b2d98b9733512baefea"
    );
    assert_eq!(
        Migrations::checksum(catalog[1].sql).as_str(),
        "sha256:4924e69150bbff9c39e2e6b7e2bdd61045202e504900fe0f510d513fbf815e67"
    );
    assert_eq!(
        Migrations::checksum(catalog[2].sql).as_str(),
        "sha256:530a6d6cb5ec9c757311d48e10a62ef456d9d01c09512f321cffe42fe3307f80"
    );
}

#[test]
fn fresh_store_applies_model_accounting_schema_v3() {
    let store = Store::open_in_memory(&FixedClock).unwrap();
    assert_eq!(store.schema_version().unwrap(), 3);

    let conn = store.conn.lock().unwrap();
    for table in ["model_call_attempts", "model_usage"] {
        let strict: i64 = conn
            .query_row(
                "SELECT strict FROM pragma_table_list WHERE name=?1",
                [table],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(strict, 1, "{table} must be STRICT");
    }
    let integrity: String = conn
        .query_row("PRAGMA integrity_check", [], |row| row.get(0))
        .unwrap();
    assert_eq!(integrity, "ok");
    let fk_violations: i64 = conn
        .query_row("SELECT count(*) FROM pragma_foreign_key_check", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(fk_violations, 0);
}

#[test]
fn existing_v2_store_upgrades_to_v3() {
    let path = TempDb::new();
    seed_v2(&path.0);
    let store = Store::open(&path.0, &FixedClock).unwrap();
    assert_eq!(store.schema_version().unwrap(), 3);
}

#[test]
fn v2_task_turn_counter_defaults_to_zero_and_is_bounded() {
    let path = TempDb::new();
    seed_v2(&path.0);
    let legacy = Connection::open(&path.0).unwrap();
    seed_task(&legacy, "tsk_00000000000000000000000001");
    drop(legacy);

    let store = Store::open(&path.0, &FixedClock).unwrap();
    let task_id = serea_protocol::TaskId::new("tsk_00000000000000000000000001").unwrap();
    assert_eq!(store.task_model_turn_count(&task_id).unwrap(), 0);
    assert_eq!(
        store.task_model_token_usage(&task_id).unwrap(),
        serea_protocol::TokenCount::new(0)
    );
    let conn = store.conn.lock().unwrap();
    assert!(
        conn.execute(
            "UPDATE tasks SET model_turn_count=-1 WHERE task_id=?1",
            [task_id.as_str()]
        )
        .is_err()
    );
    assert!(
        conn.execute(
            "UPDATE tasks SET model_turn_count=13 WHERE task_id=?1",
            [task_id.as_str()]
        )
        .is_err()
    );
    assert!(
        conn.execute(
            "UPDATE tasks SET model_token_count=-1 WHERE task_id=?1",
            [task_id.as_str()]
        )
        .is_err()
    );
}

#[test]
fn interrupted_0003_transaction_rolls_back_schema_and_catalog_row() {
    let path = TempDb::new();
    seed_v2(&path.0);
    let conn = Connection::open(&path.0).unwrap();
    conn.pragma_update(None, "foreign_keys", "ON").unwrap();
    conn.execute_batch("BEGIN IMMEDIATE").unwrap();
    conn.execute_batch(Migrations::embedded()[2].sql).unwrap();
    assert!(
        conn.execute_batch("SELECT * FROM missing_p4b_table")
            .is_err()
    );
    conn.execute_batch("ROLLBACK").unwrap();
    assert_eq!(
        conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_schema WHERE type='table' AND name='model_call_attempts')",
            [],
            |row| row.get::<_, i64>(0),
        )
        .unwrap(),
        0
    );
    let versions: Vec<i64> = conn
        .prepare("SELECT version FROM schema_migrations ORDER BY version")
        .unwrap()
        .query_map([], |row| row.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(versions, [1, 2]);
}

#[test]
fn schema_enforces_request_identity_relations_and_one_active_task_attempt() {
    let store = Store::open_in_memory(&FixedClock).unwrap();
    let conn = store.conn.lock().unwrap();
    let task_id = "tsk_00000000000000000000000000";
    seed_task(&conn, task_id);
    assert_eq!(
        insert_intent(&conn, "req_00000000000000000000000000", Some(task_id)).unwrap(),
        1
    );
    assert!(insert_intent(&conn, "req_00000000000000000000000000", None).is_err());
    assert!(insert_intent(&conn, "req_00000000000000000000000001", Some(task_id)).is_err());
    assert_eq!(
        insert_intent(&conn, "req_00000000000000000000000002", None).unwrap(),
        1
    );
    assert_eq!(
        insert_intent(&conn, "req_00000000000000000000000003", None).unwrap(),
        1
    );

    let bad_relation = conn.execute(
        "INSERT INTO model_call_attempts(request_id,purpose,model_id,provider_id,deployment_class,data_class_rank,state,relation_kind,accounting_day_utc,cost_class,price_revision,input_rate_microusd_per_million,output_rate_microusd_per_million,max_context_tokens,effective_max_output_tokens,reserved_cost_usd_micros,dispatch_intent_at_ms)
         VALUES ('req_00000000000000000000000004','CHAT','m','p','CLOUD',0,'DISPATCH_INTENT','FALLBACK',0,'FREE','rev',0,0,10,5,0,0)",
        [],
    );
    assert!(bad_relation.is_err());
}

#[test]
fn migration_has_query_driven_indexes_for_accounting_and_recovery() {
    let store = Store::open_in_memory(&FixedClock).unwrap();
    let conn = store.conn.lock().unwrap();
    let index_names: Vec<String> = conn
        .prepare("SELECT name FROM sqlite_schema WHERE type='index' AND name LIKE 'model_%' ORDER BY name")
        .unwrap()
        .query_map([], |row| row.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    for expected in [
        "model_call_attempts_recovery",
        "model_call_attempts_response_blob",
        "model_call_attempts_spend_day_state",
        "model_call_attempts_task_state",
        "model_call_attempts_terminal_retention",
        "model_call_attempts_task_state",
        "model_usage_recorded_at",
        "model_usage_task_id",
    ] {
        assert!(
            index_names.iter().any(|name| name == expected),
            "missing {expected}"
        );
    }

    let plans = [
        "EXPLAIN QUERY PLAN SELECT * FROM model_call_attempts WHERE request_id='req_00000000000000000000000000'",
        "EXPLAIN QUERY PLAN SELECT request_id FROM model_call_attempts WHERE task_id='tsk_00000000000000000000000000' AND state='DISPATCH_INTENT'",
        "EXPLAIN QUERY PLAN SELECT count(*) FROM model_call_attempts WHERE task_id='tsk_00000000000000000000000000'",
        "EXPLAIN QUERY PLAN SELECT sum(input_tokens+output_tokens) FROM model_usage WHERE task_id='tsk_00000000000000000000000000'",
        "EXPLAIN QUERY PLAN SELECT sum(CASE WHEN state IN ('DISPATCH_INTENT','AMBIGUOUS') THEN reserved_cost_usd_micros ELSE actual_cost_usd_micros END) FROM model_call_attempts WHERE accounting_day_utc=0",
        "EXPLAIN QUERY PLAN SELECT request_id FROM model_call_attempts WHERE state='DISPATCH_INTENT' ORDER BY dispatch_intent_at_ms,request_id",
        "EXPLAIN QUERY PLAN SELECT request_id FROM model_call_attempts WHERE terminal_at_ms < 0 ORDER BY terminal_at_ms,request_id",
        "EXPLAIN QUERY PLAN SELECT usage_id FROM model_usage WHERE recorded_at_ms < 0 ORDER BY recorded_at_ms,usage_id",
    ];
    let expected_indexes = [
        "sqlite_autoindex_model_call_attempts_1", // RequestId lookup.
        "model_call_attempts_task_state",
        "model_call_attempts_task_state",
        "model_usage_task_id",
        "model_call_attempts_spend_day_state",
        "model_call_attempts_recovery",
        "model_call_attempts_terminal_retention",
        "model_usage_recorded_at",
    ];
    for (sql, expected) in plans.into_iter().zip(expected_indexes) {
        let details: Vec<String> = conn
            .prepare(sql)
            .unwrap()
            .query_map([], |row| row.get(3))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        assert!(!details.is_empty(), "no query plan for {sql}");
        let plan = details.join(" ");
        assert!(
            plan.contains(expected),
            "query did not use {expected}: {plan}"
        );
    }
}
