use rusqlite::Connection;
use serea_protocol::EpochMillis;

use crate::{Migrations, migrate};

const P7_V9_CANDIDATE: &str =
    include_str!("../migrations/candidates/0009_p7_memory_schema_candidate.sql");
const P8_V10_CANDIDATE: &str =
    include_str!("../migrations/candidates/0010_p8_dispatch_candidate.sql");

#[test]
fn p8_v10_candidate_isolated_shape_is_frozen_and_not_catalogued() {
    let mut catalog = Migrations::embedded()[..8]
        .iter()
        .map(|migration| crate::Migration {
            version: migration.version,
            name: migration.name,
            sql: migration.sql,
        })
        .collect::<Vec<_>>();
    catalog.push(crate::Migration {
        version: 9,
        name: "0009_p7_memory_schema_candidate_not_accepted",
        sql: P7_V9_CANDIDATE,
    });
    catalog.push(crate::Migration {
        version: 10,
        name: "0010_p8_dispatch_candidate_not_accepted",
        sql: P8_V10_CANDIDATE,
    });

    let mut conn = Connection::open_in_memory().unwrap();
    conn.pragma_update(None, "foreign_keys", "ON").unwrap();
    migrate::apply(&mut conn, &catalog, EpochMillis::new(0).unwrap(), true).unwrap();

    let applied: (String, String) = conn
        .query_row(
            "SELECT name,checksum FROM schema_migrations WHERE version=10",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert_eq!(applied.0, "0010_p8_dispatch_candidate_not_accepted");
    assert_eq!(applied.1, Migrations::checksum(P8_V10_CANDIDATE).as_str());
    let intent_columns = conn
        .prepare("PRAGMA table_info(dispatch_intents)")
        .unwrap()
        .query_map([], |row| row.get::<_, String>(1))
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    for required in [
        "request_id",
        "idempotency_key",
        "task_id",
        "step_id",
        "owner_id",
        "grant_id",
        "plan_revision",
        "task_state_revision",
        "lease_generation",
        "policy_revision_id",
        "generation_id",
        "descriptor_digest",
        "overlay_revision",
        "event_id",
        "committed_at_ms",
    ] {
        assert!(intent_columns.iter().any(|column| column == required));
    }
    assert!(!intent_columns.iter().any(|column| column == "state"));
    assert!(!intent_columns.iter().any(|column| column == "receipt_id"));

    let integrity: String = conn
        .query_row("PRAGMA integrity_check", [], |row| row.get(0))
        .unwrap();
    let foreign_key_errors: i64 = conn
        .query_row("SELECT count(*) FROM pragma_foreign_key_check", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(integrity, "ok");
    assert_eq!(foreign_key_errors, 0);
    assert!(
        !Migrations::embedded()
            .iter()
            .any(|migration| migration.sql == P8_V10_CANDIDATE)
    );
}
