//! Rust/bundled-SQLite proof for the feature-gated P7 additive migration.
use crate::{Migrations, migrate};
use rusqlite::Connection;
use serea_protocol::EpochMillis;

#[test]
fn p7_migration_has_exact_parent_keys_and_clean_foreign_key_state() {
    let store = crate::Store::open_in_memory(&TestClock).unwrap();
    assert_eq!(store.schema_version().unwrap(), 9);
    let conn = store.conn.lock().unwrap();
    assert_eq!(
        conn.query_row("SELECT count(*) FROM pragma_foreign_key_check", [], |r| r
            .get::<_, i64>(
            0
        ))
        .unwrap(),
        0
    );
    let fk_enabled: i64 = conn
        .query_row("PRAGMA foreign_keys", [], |r| r.get(0))
        .unwrap();
    assert_eq!(fk_enabled, 1);
    let sqlite_version: String = conn
        .query_row("SELECT sqlite_version()", [], |r| r.get(0))
        .unwrap();
    assert_eq!(sqlite_version, "3.53.2");
    let mut indexes = conn
        .prepare("SELECT name FROM pragma_index_list('p7_memory_items') WHERE \"unique\"=1")
        .unwrap();
    let names = indexes
        .query_map([], |r| r.get::<_, String>(0))
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    let has_exact_parent = names.iter().any(|index| {
        let mut columns = conn
            .prepare("SELECT name FROM pragma_index_info(?1) ORDER BY seqno")
            .unwrap();
        columns
            .query_map([index], |r| r.get::<_, String>(0))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap()
            == ["memory_id", "owner_id"]
    });
    assert!(
        has_exact_parent,
        "the composite FK parent must have UNIQUE(memory_id,owner_id)"
    );
    let blob_key: i64 = conn
        .query_row(
            "SELECT count(*) FROM pragma_index_list('blobs') WHERE origin='pk'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(
        blob_key, 1,
        "Blob identity remains the composite primary key"
    );
    assert!(
        Migrations::embedded()
            .iter()
            .any(|m| m.version == 9 && m.name == "0009_p7_synthetic_memory")
    );
}

#[test]
fn failed_0009_migration_rolls_back_ddl_and_catalog_row_atomically() {
    let mut conn = Connection::open_in_memory().unwrap();
    let mut catalog = Migrations::embedded()[..8]
        .iter()
        .map(|m| crate::Migration {
            version: m.version,
            name: m.name,
            sql: m.sql,
        })
        .collect::<Vec<_>>();
    migrate::apply(&mut conn, &catalog, EpochMillis::new(0).unwrap(), true).unwrap();
    catalog.push(crate::Migration {
        version: 9,
        name: "0009_p7_synthetic_memory",
        sql: "CREATE TABLE p7_rollback_probe(value INTEGER); THIS IS INVALID SQL;",
    });
    assert!(migrate::apply(&mut conn, &catalog, EpochMillis::new(1).unwrap(), false).is_err());
    let exists: i64 = conn
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_schema WHERE name='p7_rollback_probe')",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(exists, 0);
    let latest: i64 = conn
        .query_row("SELECT max(version) FROM schema_migrations", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(latest, 8);
}

#[test]
fn composite_class_bound_blob_reference_rejects_wrong_class_and_cross_owner_provenance() {
    let store = crate::Store::open_in_memory(&TestClock).unwrap();
    store.transact(|tx|{
        tx.inner.execute("INSERT INTO p7_owners VALUES('own_00000000000000000000000000000021',21,'ACTIVE',0)",[])?;
        tx.inner.execute("INSERT INTO p7_resource_accounting(owner_id) VALUES('own_00000000000000000000000000000021')",[])?;
        tx.inner.execute("INSERT INTO p7_ledger_state(owner_id,read_state) VALUES('own_00000000000000000000000000000021','READY')",[])?;
        tx.inner.execute("INSERT INTO p7_memory_items(memory_id,owner_id,class_rank,kind,body,created_at_ms,expires_at_ms) VALUES('mem','own_00000000000000000000000000000021',0,'WORKING','synthetic',1,10)",[])?;
        tx.inner.execute("INSERT INTO blobs(digest,data_class_rank,protection,size_bytes,content) VALUES(?1,0,'NONE',1,x'78')",["sha256:0000000000000000000000000000000000000000000000000000000000000000"])?;
        tx.inner.execute("INSERT INTO p7_memory_blob_refs VALUES('mem','own_00000000000000000000000000000021',?1,0)",["sha256:0000000000000000000000000000000000000000000000000000000000000000"])?;
        Ok(())
    }).unwrap();
    let wrong=store.transact(|tx|{tx.inner.execute("INSERT INTO p7_memory_blob_refs VALUES('mem','own_00000000000000000000000000000021',?1,1)",["sha256:0000000000000000000000000000000000000000000000000000000000000000"])?;Ok(())});
    assert!(wrong.is_err());
    let cross=store.transact(|tx|{tx.inner.execute("INSERT INTO p7_memory_provenance(memory_id,owner_id,source_id,source_class_rank,metadata_bytes,recorded_at_ms) VALUES('mem','own_00000000000000000000000000000022','s',0,0,1)",[])?;Ok(())});
    assert!(cross.is_err());
}

struct TestClock;
impl serea_protocol::Clock for TestClock {
    fn now_ms(&self) -> Result<EpochMillis, serea_protocol::ProtocolError> {
        EpochMillis::new(0)
    }
}

#[test]
fn owner_binding_is_immutable_legacy_task_is_quarantined_and_deletion_keeps_detached_records() {
    let store = crate::Store::open_in_memory(&TestClock).unwrap();
    let task = "tsk_00000000000000000000000022";
    store.transact(|tx|{
        tx.inner.execute("INSERT INTO tasks(task_id,kind,title,state,origin_kind,data_class_rank,policy_class_rank,created_at_ms,updated_at_ms,max_model_calls,max_tool_calls,max_attempts_per_step) VALUES(?1,'USER_REQUEST','synthetic','RECEIVED','USER',0,0,0,0,1,1,1)",[task])?;
        let quarantined:i64=tx.inner.query_row("SELECT count(*) FROM p7_legacy_task_quarantine WHERE task_id=?1",[task],|r|r.get(0))?;assert_eq!(quarantined,1);
        tx.inner.execute("INSERT INTO p7_owners VALUES('own_00000000000000000000000000000022',22,'ACTIVE',0)",[])?;
        tx.inner.execute("INSERT INTO p7_task_owner_bindings VALUES(?1,'own_00000000000000000000000000000022',1)",[task])?;
        tx.inner.execute("INSERT INTO p7_resource_accounting(owner_id) VALUES('own_00000000000000000000000000000022')",[])?;
        tx.inner.execute("INSERT INTO p7_ledger_state(owner_id,read_state) VALUES('own_00000000000000000000000000000022','READY')",[])?;
        tx.inner.execute("INSERT INTO p7_memory_items(memory_id,owner_id,class_rank,kind,body,created_at_ms,expires_at_ms) VALUES('detached','own_00000000000000000000000000000022',0,'WORKING','synthetic',1,10)",[])?;
        tx.inner.execute("INSERT INTO p7_memory_provenance(memory_id,owner_id,source_id,source_task_id,source_class_rank,metadata_bytes,recorded_at_ms) VALUES('detached','own_00000000000000000000000000000022','opaque',?1,0,0,1)",[task])?;
        Ok(())
    }).unwrap();
    assert!(store.transact(|tx|{tx.inner.execute("UPDATE p7_task_owner_bindings SET owner_id='own_00000000000000000000000000000022' WHERE task_id=?1",[task])?;Ok(())}).is_err());
    assert!(
        store
            .transact(|tx| {
                tx.inner.execute(
                    "DELETE FROM p7_task_owner_bindings WHERE task_id=?1",
                    [task],
                )?;
                Ok(())
            })
            .is_err()
    );
    store
        .transact(|tx| {
            tx.inner
                .execute("DELETE FROM tasks WHERE task_id=?1", [task])?;
            Ok(())
        })
        .unwrap();
    let (binding, provenance): (i64, i64) = store
        .transact(|tx| {
            Ok((
                tx.inner.query_row(
                    "SELECT count(*) FROM p7_task_owner_bindings WHERE task_id=?1",
                    [task],
                    |r| r.get(0),
                )?,
                tx.inner.query_row(
                    "SELECT count(*) FROM p7_memory_provenance WHERE source_task_id=?1",
                    [task],
                    |r| r.get(0),
                )?,
            ))
        })
        .unwrap();
    assert_eq!((binding, provenance), (1, 1));
}

#[test]
fn upgrade_from_v8_quarantines_every_legacy_task_without_guessing_owner() {
    let mut conn = Connection::open_in_memory().unwrap();
    migrate::apply(
        &mut conn,
        &Migrations::embedded()[..8],
        EpochMillis::new(0).unwrap(),
        true,
    )
    .unwrap();
    conn.execute("INSERT INTO tasks(task_id,kind,title,state,origin_kind,data_class_rank,policy_class_rank,created_at_ms,updated_at_ms,max_model_calls,max_tool_calls,max_attempts_per_step) VALUES('tsk_00000000000000000000000023','USER_REQUEST','synthetic legacy','RECEIVED','USER',0,0,0,0,1,1,1)",[]).unwrap();
    migrate::apply(
        &mut conn,
        Migrations::embedded(),
        EpochMillis::new(1).unwrap(),
        false,
    )
    .unwrap();
    let (quarantined,bindings):(i64,i64)=conn.query_row("SELECT (SELECT count(*) FROM p7_legacy_task_quarantine WHERE task_id='tsk_00000000000000000000000023'),(SELECT count(*) FROM p7_task_owner_bindings WHERE task_id='tsk_00000000000000000000000023')",[],|r|Ok((r.get(0)?,r.get(1)?))).unwrap();
    assert_eq!((quarantined, bindings), (1, 0));
}
