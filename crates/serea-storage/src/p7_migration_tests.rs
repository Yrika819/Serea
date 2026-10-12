//! Rust/bundled-SQLite proof for the feature-gated P7 additive migration.
use crate::{Migrations, migrate};
use rusqlite::Connection;
use serea_protocol::EpochMillis;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Barrier};

static NEXT_TEMP_DB: AtomicU64 = AtomicU64::new(1);

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
fn composite_class_bound_blob_reference_allows_safe_raise_and_rejects_downgrade_and_cross_owner() {
    let store = crate::Store::open_in_memory(&TestClock).unwrap();
    store.transact(|tx|{
        tx.inner.execute("INSERT INTO p7_owners VALUES('own_00000000000000000000000000000021',21,'ACTIVE',0)",[])?;
        tx.inner.execute("INSERT INTO p7_resource_accounting(owner_id) VALUES('own_00000000000000000000000000000021')",[])?;
        tx.inner.execute("INSERT INTO p7_ledger_state(owner_id,read_state) VALUES('own_00000000000000000000000000000021','READY')",[])?;
        tx.inner.execute("INSERT INTO p7_memory_items(memory_id,owner_id,class_rank,kind,body,created_at_ms,expires_at_ms) VALUES('mem','own_00000000000000000000000000000021',0,'WORKING','synthetic',1,10)",[])?;
        tx.inner.execute("INSERT INTO p7_memory_items(memory_id,owner_id,class_rank,kind,body,created_at_ms,expires_at_ms) VALUES('personal-mem','own_00000000000000000000000000000021',1,'WORKING','personal',1,10)",[])?;
        tx.inner.execute("INSERT INTO blobs(digest,data_class_rank,protection,size_bytes,content) VALUES(?1,0,'NONE',1,x'78')",["sha256:0000000000000000000000000000000000000000000000000000000000000000"])?;
        tx.inner.execute("INSERT INTO blobs(digest,data_class_rank,protection,size_bytes,content) VALUES(?1,1,'NONE',1,x'78')",["sha256:0000000000000000000000000000000000000000000000000000000000000000"])?;
        tx.inner.execute("INSERT INTO p7_memory_blob_refs VALUES('mem','own_00000000000000000000000000000021',?1,0)",["sha256:0000000000000000000000000000000000000000000000000000000000000000"])?;
        Ok(())
    }).unwrap();
    let wrong=store.transact(|tx|{tx.inner.execute("INSERT INTO p7_memory_blob_refs VALUES('mem','own_00000000000000000000000000000021',?1,1)",["sha256:0000000000000000000000000000000000000000000000000000000000000000"])?;Ok(())});
    assert!(wrong.is_err());
    store.transact(|tx|{tx.inner.execute("INSERT INTO p7_memory_blob_refs VALUES('personal-mem','own_00000000000000000000000000000021',?1,0)",["sha256:0000000000000000000000000000000000000000000000000000000000000000"])?;Ok(())}).unwrap();
    let cross=store.transact(|tx|{tx.inner.execute("INSERT INTO p7_memory_provenance(memory_id,owner_id,source_id,source_class_rank,metadata_bytes,recorded_at_ms) VALUES('mem','own_00000000000000000000000000000022','s',0,0,1)",[])?;Ok(())});
    assert!(cross.is_err());
}

#[test]
fn classification_composition_rejects_downgrades_for_provenance_and_blob_contents() {
    let store = crate::Store::open_in_memory(&TestClock).unwrap();
    let owner = "own_00000000000000000000000000000029";
    store
        .transact(|tx| {
            tx.inner.execute(
                "INSERT INTO p7_owners VALUES(?1,29,'ACTIVE',0)",
                [owner],
            )?;
            tx.inner.execute(
                "INSERT INTO p7_resource_accounting(owner_id) VALUES(?1)",
                [owner],
            )?;
            tx.inner.execute(
                "INSERT INTO p7_ledger_state(owner_id,read_state) VALUES(?1,'READY')",
                [owner],
            )?;
            for (id, class) in [("public-memory", 0_i64), ("personal-memory", 1_i64)] {
                tx.inner.execute(
                    "INSERT INTO p7_memory_items(memory_id,owner_id,class_rank,kind,body,created_at_ms,expires_at_ms) VALUES(?1,?2,?3,'WORKING','synthetic',1,10)",
                    rusqlite::params![id, owner, class],
                )?;
            }
            for class in [0_i64, 1_i64] {
                let digest = format!("sha256:{}", if class == 0 { "0".repeat(64) } else { "1".repeat(64) });
                tx.inner.execute(
                    "INSERT INTO blobs(digest,data_class_rank,protection,size_bytes,content) VALUES(?1,?2,'NONE',1,x'78')",
                    rusqlite::params![digest, class],
                )?;
            }
            tx.inner.execute(
                "INSERT INTO blobs(digest,data_class_rank,protection,size_bytes,content) VALUES(?1,1,'NONE',1,x'78')",
                [format!("sha256:{}", "0".repeat(64))],
            )?;
            Ok(())
        })
        .unwrap();

    // UPDATE is a separate direct-SQL route and must enforce the same
    // containing-class rule as INSERT.
    // A derived Memory class must be >= every provenance source class.
    let downgraded_provenance = store.transact(|tx| {
        tx.inner.execute(
            "INSERT INTO p7_memory_provenance(memory_id,owner_id,source_id,source_class_rank,metadata_bytes,recorded_at_ms) VALUES('public-memory',?1,'source',1,0,1)",
            [owner],
        )?;
        Ok(())
    });
    assert!(downgraded_provenance.is_err());
    store
        .transact(|tx| {
            tx.inner.execute(
                "INSERT INTO p7_memory_provenance(memory_id,owner_id,source_id,source_class_rank,metadata_bytes,recorded_at_ms) VALUES('personal-memory',?1,'source',0,0,1)",
                [owner],
            )?;
            Ok(())
        })
        .unwrap();

    assert!(store.transact(|tx| {
        tx.inner.execute(
            "INSERT INTO p7_memory_blob_refs(memory_id,owner_id,digest,data_class_rank) VALUES('public-memory',?1,?2,0)",
            rusqlite::params![owner, format!("sha256:{}", "0".repeat(64))],
        )?;
        tx.inner.execute(
            "UPDATE p7_memory_blob_refs SET data_class_rank=1 WHERE memory_id='public-memory' AND owner_id=?1",
            [owner],
        )?;
        Ok(())
    }).is_err());
    store.transact(|tx| {
        tx.inner.execute(
            "INSERT INTO p7_memory_blob_refs(memory_id,owner_id,digest,data_class_rank) VALUES('personal-memory',?1,?2,0)",
            rusqlite::params![owner, format!("sha256:{}", "0".repeat(64))],
        )?;
        tx.inner.execute(
            "UPDATE p7_memory_blob_refs SET data_class_rank=1 WHERE memory_id='personal-memory' AND owner_id=?1 AND digest=?2",
            rusqlite::params![owner, format!("sha256:{}", "0".repeat(64))],
        )?;
        Ok(())
    }).unwrap();

    // A containing Memory class must be >= the Blob content class.
    let personal_blob_in_public = store.transact(|tx| {
        tx.inner.execute(
            "INSERT INTO p7_memory_blob_refs(memory_id,owner_id,digest,data_class_rank) VALUES('public-memory',?1,?2,1)",
            rusqlite::params![owner, format!("sha256:{}", "1".repeat(64))],
        )?;
        Ok(())
    });
    assert!(personal_blob_in_public.is_err());

    store
        .transact(|tx| {
            tx.inner.execute(
                "INSERT INTO p7_memory_blob_refs(memory_id,owner_id,digest,data_class_rank) VALUES('personal-memory',?1,?2,0)",
                rusqlite::params![owner, format!("sha256:{}", "0".repeat(64))],
            )?;
            Ok(())
        })
        .unwrap();

    for rank in [-1_i64, 2, 3, 4, 5] {
        assert!(store
            .transact(|tx| {
                tx.inner.execute(
                    "INSERT INTO p7_memory_provenance(memory_id,owner_id,source_id,source_class_rank,metadata_bytes,recorded_at_ms) VALUES('personal-memory',?1,?2,?3,0,1)",
                    rusqlite::params![owner, format!("source-{rank}"), rank],
                )?;
                Ok(())
            })
            .is_err());
        assert!(store.transact(|tx| {
            tx.inner.execute(
                "INSERT INTO p7_memory_blob_refs(memory_id,owner_id,digest,data_class_rank) VALUES('personal-memory',?1,?2,?3)",
                rusqlite::params![owner, format!("sha256:{}", "2".repeat(64)), rank],
            )?;
            Ok(())
        }).is_err());
    }
    assert_eq!(store.schema_version().unwrap(), 9);
}

#[test]
fn memory_accounting_matches_authoritative_rows_across_deactivate_delete_and_rollback() {
    let store = crate::Store::open_in_memory(&TestClock).unwrap();
    let owner = "own_00000000000000000000000000000030";
    store.transact(|tx| {
        tx.inner.execute("INSERT INTO p7_owners VALUES(?1,30,'ACTIVE',0)", [owner])?;
        tx.inner.execute("INSERT INTO p7_resource_accounting(owner_id) VALUES(?1)", [owner])?;
        tx.inner.execute("INSERT INTO p7_ledger_state(owner_id,read_state) VALUES(?1,'READY')", [owner])?;
        for (id, body) in [("prior", "prior synthetic"), ("successor", "successor synthetic"), ("active", "active synthetic")] {
            tx.inner.execute("INSERT INTO p7_memory_items(memory_id,owner_id,class_rank,kind,body,created_at_ms,expires_at_ms) VALUES(?1,?2,0,'WORKING',?3,1,10)", rusqlite::params![id, owner, body])?;
        }
        tx.inner.execute("INSERT INTO p7_supersession_edges VALUES(?1,'prior','successor',2)", [owner])?;
        tx.inner.execute("UPDATE p7_memory_items SET active=0 WHERE owner_id=?1 AND memory_id='prior'", [owner])?;
        Ok(())
    }).unwrap();
    assert_memory_accounting(&store, owner);

    // Expiry is a visibility rule; it does not silently free retained capacity.
    assert_memory_accounting(&store, owner);

    // The old trigger subtracted the active counters a second time here and
    // failed a CHECK. Inactive rows must only release retained-row accounting.
    forget_rows(&store, owner, &["prior"], 1);
    assert_memory_accounting(&store, owner);

    // A failed transaction rolls back both tombstone and deletion, so retry is
    // safe and the eventual committed transition counts exactly once.
    let failed: Result<(), crate::StoreError> = store.transact(|tx| {
        tx.inner.execute(
            "INSERT INTO p7_memory_tombstones VALUES(?1,'active','retry',2,3)",
            [owner],
        )?;
        tx.inner.execute(
            "DELETE FROM p7_memory_items WHERE owner_id=?1 AND memory_id='active'",
            [owner],
        )?;
        Err(crate::StoreError::Sqlite)
    });
    assert!(failed.is_err());
    assert_memory_accounting(&store, owner);
    forget_rows(&store, owner, &["active"], 2);
    assert_memory_accounting(&store, owner);

    // A shared Blob remains while referenced by the successor, then remains
    // uncollected after the last reference (physical GC is a separate phase).
    store.transact(|tx| {
        tx.inner.execute("INSERT INTO p7_memory_items(memory_id,owner_id,class_rank,kind,body,created_at_ms,expires_at_ms) VALUES('shared-peer',?1,0,'WORKING','shared synthetic',1,10)", [owner])?;
        tx.inner.execute("INSERT INTO blobs(digest,data_class_rank,protection,size_bytes,content) VALUES(?1,0,'NONE',1,x'78')", ["sha256:2222222222222222222222222222222222222222222222222222222222222222"])?;
        tx.inner.execute("INSERT INTO p7_memory_blob_refs VALUES('successor',?1,?2,0)", rusqlite::params![owner,"sha256:2222222222222222222222222222222222222222222222222222222222222222"])?;
        tx.inner.execute("INSERT INTO p7_memory_blob_refs VALUES('shared-peer',?1,?2,0)", rusqlite::params![owner,"sha256:2222222222222222222222222222222222222222222222222222222222222222"])?;
        Ok(())
    }).unwrap();
    assert_memory_accounting(&store, owner);
    forget_rows(&store, owner, &["successor"], 3);
    assert_memory_accounting(&store, owner);
    let (blob_rows, refs): (i64, i64) = store
        .transact(|tx| {
            Ok((
                tx.inner.query_row(
                    "SELECT count(*) FROM blobs WHERE digest=?1 AND data_class_rank=0",
                    ["sha256:2222222222222222222222222222222222222222222222222222222222222222"],
                    |r| r.get(0),
                )?,
                tx.inner
                    .query_row("SELECT count(*) FROM p7_memory_blob_refs", [], |r| r.get(0))?,
            ))
        })
        .unwrap();
    assert_eq!((blob_rows, refs), (1, 1));
    forget_rows(&store, owner, &["shared-peer"], 4);
    let refs: i64 = store
        .transact(|tx| {
            Ok(tx
                .inner
                .query_row("SELECT count(*) FROM p7_memory_blob_refs", [], |r| r.get(0))?)
        })
        .unwrap();
    assert_eq!(refs, 0);
    assert_memory_accounting(&store, owner);
}

#[test]
fn derived_forget_closure_is_bounded_owner_scoped_and_deleted_atomically() {
    let store = crate::Store::open_in_memory(&TestClock).unwrap();
    let owner = "own_00000000000000000000000000000032";
    let other = "own_00000000000000000000000000000033";
    store.transact(|tx| {
        for (id, epoch) in [(owner,32_i64),(other,33_i64)] {
            tx.inner.execute("INSERT INTO p7_owners VALUES(?1,?2,'ACTIVE',0)", rusqlite::params![id,epoch])?;
            tx.inner.execute("INSERT INTO p7_resource_accounting(owner_id) VALUES(?1)", [id])?;
            tx.inner.execute("INSERT INTO p7_ledger_state(owner_id,read_state) VALUES(?1,'READY')", [id])?;
        }
        for (id, row_owner) in [("root",owner),("child",owner),("grandchild",owner),("other",other)] {
            tx.inner.execute("INSERT INTO p7_memory_items(memory_id,owner_id,class_rank,kind,body,created_at_ms,expires_at_ms) VALUES(?1,?2,0,'WORKING','closure synthetic',1,10)", rusqlite::params![id,row_owner])?;
        }
        tx.inner.execute("INSERT INTO p7_supersession_edges VALUES(?1,'root','child',2)", [owner])?;
        tx.inner.execute("UPDATE p7_memory_items SET active=0 WHERE owner_id=?1 AND memory_id='root'", [owner])?;
        tx.inner.execute("INSERT INTO p7_supersession_edges VALUES(?1,'child','grandchild',3)", [owner])?;
        tx.inner.execute("UPDATE p7_memory_items SET active=0 WHERE owner_id=?1 AND memory_id='child'", [owner])?;
        Ok(())
    }).unwrap();

    let closure: Vec<String> = store.transact(|tx| {
        let mut statement = tx.inner.prepare("WITH RECURSIVE closure(memory_id,depth) AS (SELECT 'root',0 UNION ALL SELECT e.successor_memory_id,c.depth+1 FROM p7_supersession_edges e JOIN closure c ON e.owner_id=?1 AND e.prior_memory_id=c.memory_id WHERE c.depth<100) SELECT memory_id FROM closure ORDER BY depth")?;
        let rows = statement.query_map([owner], |row| row.get(0))?;
        rows.collect::<Result<Vec<_>,_>>().map_err(crate::StoreError::from)
    }).unwrap();
    assert_eq!(closure, ["root", "child", "grandchild"]);
    forget_rows(&store, owner, &["root", "child", "grandchild"], 9);
    assert_memory_accounting(&store, owner);
    let remaining: i64 = store
        .transact(|tx| {
            Ok(tx.inner.query_row(
                "SELECT count(*) FROM p7_memory_items WHERE owner_id=?1",
                [owner],
                |r| r.get(0),
            )?)
        })
        .unwrap();
    let other_remaining: i64 = store
        .transact(|tx| {
            Ok(tx.inner.query_row(
                "SELECT count(*) FROM p7_memory_items WHERE owner_id=?1",
                [other],
                |r| r.get(0),
            )?)
        })
        .unwrap();
    assert_eq!((remaining, other_remaining), (0, 1));
}

#[test]
fn concurrent_owner_scoped_delete_commits_once_and_counters_match_after_reopen() {
    let unique = NEXT_TEMP_DB.fetch_add(1, Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!(
        "serea-p7-delete-{}-{unique}.sqlite",
        std::process::id()
    ));
    let first = std::sync::Arc::new(crate::Store::open(&path, &TestClock).unwrap());
    first.transact(|tx| {
        tx.inner.execute("INSERT INTO p7_owners VALUES('own_00000000000000000000000000000034',34,'ACTIVE',0)", [])?;
        tx.inner.execute("INSERT INTO p7_resource_accounting(owner_id) VALUES('own_00000000000000000000000000000034')", [])?;
        tx.inner.execute("INSERT INTO p7_ledger_state(owner_id,read_state) VALUES('own_00000000000000000000000000000034','READY')", [])?;
        tx.inner.execute("INSERT INTO p7_memory_items(memory_id,owner_id,class_rank,kind,body,created_at_ms,expires_at_ms) VALUES('concurrent-delete','own_00000000000000000000000000000034',0,'WORKING','synthetic',1,10)", [])?;
        Ok(())
    }).unwrap();
    let second = std::sync::Arc::new(crate::Store::open(&path, &TestClock).unwrap());
    let barrier = Arc::new(Barrier::new(3));
    let spawn = |store: Arc<crate::Store>, barrier: Arc<Barrier>, suffix: &'static str| {
        std::thread::spawn(move || {
            barrier.wait();
            store.transact(|tx| {
            tx.inner.execute("INSERT INTO p7_memory_tombstones VALUES('own_00000000000000000000000000000034','concurrent-delete',?1,1,2)", [format!("forget-{suffix}")])?;
            tx.inner.execute("DELETE FROM p7_memory_items WHERE owner_id='own_00000000000000000000000000000034' AND memory_id='concurrent-delete'", [])?;
            Ok(())
        })
        })
    };
    let a = spawn(first.clone(), barrier.clone(), "a");
    let b = spawn(second.clone(), barrier.clone(), "b");
    barrier.wait();
    let results = [a.join().unwrap(), b.join().unwrap()];
    assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
    let (active, retained, rows): (i64,i64,i64) = first.transact(|tx| Ok((
        tx.inner.query_row("SELECT active_items FROM p7_resource_accounting WHERE owner_id='own_00000000000000000000000000000034'", [], |r| r.get(0))?,
        tx.inner.query_row("SELECT retained_items FROM p7_resource_accounting WHERE owner_id='own_00000000000000000000000000000034'", [], |r| r.get(0))?,
        tx.inner.query_row("SELECT count(*) FROM p7_memory_items WHERE owner_id='own_00000000000000000000000000000034'", [], |r| r.get(0))?,
    ))).unwrap();
    assert_eq!((active, retained, rows), (0, 0, 0));
    drop(first);
    drop(second);
    let reopened = crate::Store::open(&path, &TestClock).unwrap();
    assert_eq!(reopened.schema_version().unwrap(), 9);
    drop(reopened);
    let _ = std::fs::remove_file(path);
}

fn forget_rows(store: &crate::Store, owner: &str, ids: &[&str], generation: i64) {
    store
        .transact(|tx| {
            for id in ids {
                tx.inner.execute(
                    "INSERT INTO p7_memory_tombstones VALUES(?1,?2,?3,?4,5)",
                    rusqlite::params![owner, id, format!("op-{generation}-{id}"), generation],
                )?;
                tx.inner.execute(
                    "DELETE FROM p7_memory_items WHERE owner_id=?1 AND memory_id=?2",
                    rusqlite::params![owner, id],
                )?;
            }
            Ok(())
        })
        .unwrap();
}

fn assert_memory_accounting(store: &crate::Store, owner: &str) {
    store.transact(|tx| {
        let actual: (i64, i64, i64, i64, i64) = tx.inner.query_row(
            "SELECT count(*) FILTER (WHERE active=1),count(*),coalesce(sum(length(CAST(body AS BLOB))) FILTER (WHERE active=1),0),coalesce(sum(length(CAST(body AS BLOB))) FILTER (WHERE active=1),0),count(*) FROM p7_memory_items WHERE owner_id=?1",
            [owner], |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?)))?;
        let accounted: (i64, i64, i64, i64) = tx.inner.query_row(
            "SELECT active_items,retained_items,active_body_bytes,logical_fts_bytes FROM p7_resource_accounting WHERE owner_id=?1", [owner],
            |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?)) )?;
        assert_eq!(accounted, (actual.0, actual.1, actual.2, actual.3));
        let fts_visible: i64 = tx.inner.query_row(
            "SELECT count(*) FROM p7_memory_fts f JOIN p7_memory_items m ON m.memory_id=f.memory_id WHERE m.owner_id=?1 AND m.active=1 AND p7_memory_fts MATCH 'synthetic'", [owner], |r| r.get(0))?;
        let active_matching: i64 = tx.inner.query_row("SELECT count(*) FROM p7_memory_items WHERE owner_id=?1 AND active=1 AND instr(body,'synthetic')>0", [owner], |r| r.get(0))?;
        assert_eq!(fts_visible, active_matching);
        Ok(())
    }).unwrap();
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

#[test]
fn v8_only_catalog_refuses_an_experimental_v9_database() {
    let store = crate::Store::open_in_memory(&TestClock).unwrap();
    let conn = store.conn.lock().unwrap();
    assert!(migrate::inspect(&conn, &Migrations::embedded()[..8], false).is_err());
}
