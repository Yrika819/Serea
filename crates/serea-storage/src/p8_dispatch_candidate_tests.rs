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

#[test]
fn p8_v10_candidate_requires_grant_and_rejects_unresolved_intent_deletion() {
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

    let grant_is_required: i64 = conn
        .query_row(
            "SELECT \"notnull\" FROM pragma_table_info('dispatch_intents') WHERE name='grant_id'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(
        grant_is_required, 1,
        "OBSERVE cannot create a grant-less intent"
    );
    let grantless = conn.execute(
        "INSERT INTO dispatch_intents(request_id,idempotency_key,task_id,step_id,owner_id,plan_revision,task_state_revision,lease_generation,policy_revision_id,policy_rules_digest,generation_id,descriptor_digest,capability_id,capability_version,provider_id,overlay_revision,arguments_digest,request_digest,event_id,committed_at_ms) VALUES('req_00000000000000000000000052',?1,'tsk_00000000000000000000000052','stp_00000000000000000000000052','own_synthetic',0,1,1,1,?2,1,?3,'calendar.event.create','1.0.0','calendar',1,?4,?5,'evt_00000000000000000000000052',0)",
        rusqlite::params![
            format!("idk_{}", "1".repeat(64)),
            format!("sha256:{}", "2".repeat(64)),
            format!("sha256:{}", "3".repeat(64)),
            format!("sha256:{}", "4".repeat(64)),
            format!("sha256:{}", "5".repeat(64)),
        ],
    );
    assert!(
        grantless.is_err(),
        "a grant-less OBSERVE intent must fail closed"
    );

    // Seed an intent with FK checks disabled so unrelated authority records do
    // not have to be invented for this schema-only deletion probe. Re-enable
    // foreign keys before exercising the real ON DELETE action.
    conn.pragma_update(None, "foreign_keys", "OFF").unwrap();
    let task = "tsk_00000000000000000000000051";
    let step = "stp_00000000000000000000000051";
    conn.execute(
        "INSERT INTO tasks(task_id,kind,title,state,origin_kind,data_class_rank,policy_class_rank,created_at_ms,updated_at_ms,max_model_calls,max_tool_calls,max_attempts_per_step) VALUES(?1,'USER_REQUEST','candidate intent','EXECUTING','USER',0,0,0,0,1,1,1)",
        [task],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO task_steps(step_id,task_id,sequence,kind,status,attempt,plan_revision,provider_id,capability_id,capability_version,input_digest,idempotency_key,started_at_ms,lease_owner,lease_expires_at_ms,lease_generation) VALUES(?1,?2,0,'CAPABILITY','EXECUTING',1,0,'calendar','calendar.event.create','1.0.0',?3,?4,0,'synthetic-worker',1000,1)",
        rusqlite::params![step, task, format!("sha256:{}", "a".repeat(64)), format!("idk_{}", "b".repeat(64))],
    )
    .unwrap();
    // In this schema-only probe, bypass the business triggers while seeding a
    // synthetic historical Grant-use parent. The FK actions themselves remain
    // enabled for the deletion attempts below.
    conn.execute_batch(
        "DROP TRIGGER approval_grant_use_must_be_a_granted_step;
         DROP TRIGGER approval_grant_use_matches_grant;
         DROP TRIGGER approval_grant_use_requires_consumable_grant;
         DROP TRIGGER approval_grant_use_requires_sealed_complete_set;
         DROP TRIGGER approval_grant_use_matches_current_step_authority;",
    )
    .unwrap();
    conn.execute(
        "INSERT INTO approval_grant_uses(grant_id,step_id,task_id,consumed_at_ms) VALUES('grt_synthetic',?1,?2,0)",
        rusqlite::params![step, task],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO dispatch_intents(request_id,idempotency_key,task_id,step_id,owner_id,grant_id,plan_revision,task_state_revision,lease_generation,policy_revision_id,policy_rules_digest,generation_id,descriptor_digest,capability_id,capability_version,provider_id,overlay_revision,arguments_digest,request_digest,event_id,committed_at_ms) VALUES(?1,?2,?3,?4,'own_synthetic','grt_synthetic',0,1,1,1,?5,1,?6,'calendar.event.create','1.0.0','calendar',1,?7,?8,?9,0)",
        rusqlite::params![
            "req_00000000000000000000000051",
            format!("idk_{}", "b".repeat(64)),
            task,
            step,
            format!("sha256:{}", "c".repeat(64)),
            format!("sha256:{}", "d".repeat(64)),
            format!("sha256:{}", "e".repeat(64)),
            format!("sha256:{}", "f".repeat(64)),
            "evt_00000000000000000000000051",
        ],
    )
    .unwrap();
    conn.pragma_update(None, "foreign_keys", "ON").unwrap();

    let before: i64 = conn
        .query_row("SELECT count(*) FROM dispatch_intents", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(before, 1);
    assert!(
        conn.execute("DELETE FROM tasks WHERE task_id=?1", [task])
            .is_err()
    );
    assert!(
        conn.execute(
            "DELETE FROM dispatch_intents WHERE request_id='req_00000000000000000000000051'",
            [],
        )
        .is_err(),
        "direct deletion must not erase unresolved intent or its budget evidence"
    );
    assert!(
        conn.execute(
            "DELETE FROM approval_grant_uses WHERE grant_id='grt_synthetic' AND step_id=?1",
            [step],
        )
        .is_err()
    );
    assert!(
        conn.execute("DELETE FROM task_steps WHERE step_id=?1", [step])
            .is_err()
    );

    let after: i64 = conn
        .query_row("SELECT count(*) FROM dispatch_intents", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(
        after, 1,
        "task deletion must preserve unresolved intent evidence"
    );
}
