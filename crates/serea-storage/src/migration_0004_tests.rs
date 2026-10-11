//! P5B RED-first migration contract tests.

use crate::{Migrations, Store};
use rusqlite::{Connection, params};
use serea_protocol::{Clock, EpochMillis, ProtocolError};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);

struct FixedClock;

impl Clock for FixedClock {
    fn now_ms(&self) -> Result<EpochMillis, ProtocolError> {
        EpochMillis::new(1_796_000_000_000)
    }
}

struct TempDb(PathBuf);

impl TempDb {
    fn new() -> Self {
        let dir = std::env::temp_dir().join(format!(
            "serea-p5b-migration-{}-{}",
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

fn seed_v3(path: &std::path::Path) {
    let conn = Connection::open(path).unwrap();
    conn.pragma_update(None, "foreign_keys", "ON").unwrap();
    for migration in Migrations::embedded().iter().take(3) {
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
         VALUES (?1,'MAINTENANCE','legacy','EXECUTING','SYSTEM',0,0,0,0,12,0,0)",
        [task_id],
    )
    .unwrap();
}

#[test]
fn migration_catalog_preserves_v1_to_v3_and_records_amended_v4() {
    let catalog = Migrations::embedded();
    assert_eq!(Migrations::LATEST, 6);
    assert_eq!(catalog.len(), 6);
    assert_eq!(
        (catalog[3].version, catalog[3].name),
        (4, "0004_capability_registry")
    );
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
    assert_eq!(
        Migrations::checksum(catalog[3].sql).as_str(),
        "sha256:06b22fa682564290b71a26825a777f7a295220bd02f4c6c614d234b73001685f"
    );
}

#[test]
fn fresh_store_applies_schema_v4_and_keeps_new_task_generation_nullable() {
    let store = Store::open_in_memory(&FixedClock).unwrap();
    assert_eq!(store.schema_version().unwrap(), 6);
    let conn = store.conn.lock().unwrap();
    let columns: Vec<(String, i64)> = conn
        .prepare("SELECT name, \"notnull\" FROM pragma_table_info('tasks')")
        .unwrap()
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    let generation = columns
        .iter()
        .find(|(name, _)| name == "capability_registry_generation")
        .expect("0004 adds the nullable Task generation pin");
    assert_eq!(generation.1, 0);
    let task_pin_delete_action: String = conn
        .query_row(
            "SELECT on_delete FROM pragma_foreign_key_list('tasks') WHERE \"from\"='capability_registry_generation'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(task_pin_delete_action, "RESTRICT");
}

#[test]
fn fresh_store_closes_and_reopens_at_schema_v4() {
    let temp = TempDb::new();
    {
        let store = Store::open(&temp.0, &FixedClock).unwrap();
        assert_eq!(store.schema_version().unwrap(), 6);
        store.verify_integrity().unwrap();
    }
    let reopened = Store::open(&temp.0, &FixedClock).unwrap();
    assert_eq!(reopened.schema_version().unwrap(), 6);
    reopened.verify_integrity().unwrap();
}

#[test]
fn existing_schema_v3_upgrades_to_v4_without_pinning_existing_tasks() {
    let temp = TempDb::new();
    seed_v3(&temp.0);
    let legacy = Connection::open(&temp.0).unwrap();
    seed_task(&legacy, "tsk_00000000000000000000000001");
    drop(legacy);

    let store = Store::open(&temp.0, &FixedClock).unwrap();
    assert_eq!(store.schema_version().unwrap(), 6);
    let conn = store.conn.lock().unwrap();
    let pin: Option<i64> = conn
        .query_row(
            "SELECT capability_registry_generation FROM tasks WHERE task_id=?1",
            ["tsk_00000000000000000000000001"],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(pin, None);
}

#[test]
fn interrupted_0004_transaction_rolls_back_all_schema_and_catalog_changes() {
    let temp = TempDb::new();
    seed_v3(&temp.0);
    let conn = Connection::open(&temp.0).unwrap();
    conn.pragma_update(None, "foreign_keys", "ON").unwrap();
    conn.execute_batch("BEGIN IMMEDIATE").unwrap();
    conn.execute_batch(Migrations::embedded()[3].sql).unwrap();
    assert!(
        conn.execute_batch("SELECT * FROM missing_p5b_table")
            .is_err()
    );
    conn.execute_batch("ROLLBACK").unwrap();

    let versions: Vec<i64> = conn
        .prepare("SELECT version FROM schema_migrations ORDER BY version")
        .unwrap()
        .query_map([], |row| row.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(versions, [1, 2, 3]);
    for table in [
        "capability_registry_generations",
        "capability_descriptor_revisions",
        "capability_generation_members",
        "capability_generation_defaults",
        "capability_registry_state",
        "capability_overlays",
        "step_capability_bindings",
    ] {
        let exists: i64 = conn
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM sqlite_schema WHERE type='table' AND name=?1)",
                [table],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(exists, 0, "interrupted migration left {table}");
    }
}

#[test]
fn fresh_store_has_strict_registry_tables_indexes_and_clean_integrity() {
    for store in [Store::open_in_memory(&FixedClock).unwrap()] {
        let conn = store.conn.lock().unwrap();
        for table in [
            "capability_registry_generations",
            "capability_descriptor_revisions",
            "capability_generation_members",
            "capability_generation_defaults",
            "capability_registry_state",
            "capability_overlays",
            "step_capability_bindings",
        ] {
            let strict: i64 = conn
                .query_row(
                    "SELECT strict FROM pragma_table_list WHERE name=?1",
                    [table],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(strict, 1, "{table} must be STRICT");
        }
        let priority_index: i64 = conn
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM sqlite_schema WHERE type='index' AND name=?1)",
                ["capability_generation_members_priority"],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(priority_index, 1, "missing query-driven membership index");
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
}

#[test]
fn registry_query_plans_use_primary_keys_and_the_membership_priority_index() {
    let store = Store::open_in_memory(&FixedClock).unwrap();
    let conn = store.conn.lock().unwrap();
    let queries = [
        (
            "active generation",
            "SELECT g.generation_id FROM capability_registry_state AS h JOIN capability_registry_generations AS g ON g.generation_id=h.active_generation_id WHERE h.singleton=1",
            "PRIMARY KEY",
        ),
        (
            "descriptor revision",
            "SELECT descriptor_digest FROM capability_descriptor_revisions WHERE descriptor_digest='sha256:0000000000000000000000000000000000000000000000000000000000000000'",
            "INDEX sqlite_autoindex_capability_descriptor_revisions_1",
        ),
        (
            "generation membership",
            "SELECT descriptor_digest FROM capability_generation_members WHERE generation_id=1 ORDER BY capability_id,capability_version,candidate_priority,descriptor_digest",
            "capability_generation_members_priority",
        ),
        (
            "default version",
            "SELECT capability_version FROM capability_generation_defaults WHERE generation_id=1 AND capability_id='calendar.events.read'",
            "INDEX sqlite_autoindex_capability_generation_defaults_1",
        ),
        (
            "implementation candidates",
            "SELECT descriptor_digest FROM capability_generation_members WHERE generation_id=1 AND capability_id='calendar.events.read' AND capability_version='1.0.0' ORDER BY candidate_priority,descriptor_digest",
            "capability_generation_members_priority",
        ),
        (
            "overlay",
            "SELECT availability_state FROM capability_overlays WHERE capability_id='calendar.events.read'",
            "INDEX sqlite_autoindex_capability_overlays_1",
        ),
        (
            "task generation",
            "SELECT capability_registry_generation FROM tasks WHERE task_id='tsk_00000000000000000000000001'",
            "INDEX sqlite_autoindex_tasks_1",
        ),
        (
            "step binding",
            "SELECT descriptor_digest FROM step_capability_bindings WHERE task_id='tsk_00000000000000000000000001' AND step_id='stp_00000000000000000000000001'",
            "INDEX sqlite_autoindex_step_capability_bindings_1",
        ),
    ];
    for (name, sql, required) in queries {
        let details: Vec<String> = conn
            .prepare(&format!("EXPLAIN QUERY PLAN {sql}"))
            .unwrap()
            .query_map([], |row| row.get(3))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        let joined = details.join(" | ");
        assert!(
            joined.contains(required),
            "{name} query did not use expected access path: {joined}"
        );
    }
}
