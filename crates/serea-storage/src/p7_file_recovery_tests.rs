//! Disposable file-backed proof of the experimental P7 recovery ordering.
//! The ledger and SQLite files are separate paths on one host filesystem; this
//! deliberately does not claim independent rollback domains or power-loss proof.
use crate::{Store, StoreError};
use rusqlite::params;
use serea_protocol::{Clock, EpochMillis, ProtocolError};
use sha2::{Digest, Sha256};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(1);
const OWNER: &str = "own_00000000000000000000000000000070";
const MEMORY: &str = "synthetic-forget-target";
struct ClockAt;
impl Clock for ClockAt {
    fn now_ms(&self) -> Result<EpochMillis, ProtocolError> {
        EpochMillis::new(1000)
    }
}
struct Scratch(PathBuf);
impl Scratch {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "serea-p7-recovery-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(path.join("primary")).unwrap();
        fs::create_dir_all(path.join("ledger")).unwrap();
        Self(path)
    }
    fn db(&self) -> PathBuf {
        self.0.join("primary/synthetic.sqlite")
    }
    fn ledger(&self) -> PathBuf {
        self.0.join("ledger/forget.frames")
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Frame {
    kind: String,
    owner: String,
    generation: u64,
    memory: String,
}
struct Ledger {
    path: PathBuf,
}
impl Ledger {
    fn new(path: PathBuf) -> Self {
        Self { path }
    }
    fn append(&self, frame: &Frame, fail_before_write: bool) -> Result<(), StoreError> {
        if fail_before_write {
            return Err(StoreError::Sqlite);
        }
        self.read()?;
        let previous = self.last_digest()?;
        let payload = format!(
            "{}\t{}\t{}\t{}",
            frame.kind, frame.owner, frame.generation, frame.memory
        );
        let digest = hex(&Sha256::digest(format!("{previous}\t{payload}").as_bytes()));
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .map_err(|_| StoreError::Sqlite)?;
        writeln!(file, "{payload}\t{previous}\t{digest}").map_err(|_| StoreError::Sqlite)?;
        file.sync_all().map_err(|_| StoreError::Sqlite)
    }
    fn last_digest(&self) -> Result<String, StoreError> {
        if !self.path.exists() {
            return Ok("0".into());
        }
        let text = fs::read_to_string(&self.path).map_err(|_| StoreError::Sqlite)?;
        Ok(text
            .lines()
            .last()
            .and_then(|line| line.split('\t').nth(5))
            .unwrap_or("0")
            .to_owned())
    }
    fn append_partial_chunk(&self) {
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .unwrap();
        file.write_all(b"INTENT\towner-a\t9").unwrap();
        file.sync_all().unwrap();
    }
    fn read(&self) -> Result<Vec<Frame>, StoreError> {
        if !self.path.exists() {
            return Ok(Vec::new());
        }
        let mut bytes = Vec::new();
        File::open(&self.path)
            .and_then(|mut f| f.read_to_end(&mut bytes))
            .map_err(|_| StoreError::Sqlite)?;
        if !bytes.is_empty() && !bytes.ends_with(b"\n") {
            return Err(StoreError::Sqlite);
        }
        let mut out = Vec::new();
        let mut previous = "0".to_owned();
        for line in std::str::from_utf8(&bytes)
            .map_err(|_| StoreError::Sqlite)?
            .lines()
        {
            let fields: Vec<_> = line.split('\t').collect();
            if fields.len() != 6 || fields[4] != previous {
                return Err(StoreError::Sqlite);
            }
            let generation = fields[2].parse().map_err(|_| StoreError::Sqlite)?;
            let expected = hex(&Sha256::digest(
                format!("{}\t{}", fields[4], fields[..4].join("\t")).as_bytes(),
            ));
            if expected != fields[5] {
                return Err(StoreError::Sqlite);
            }
            out.push(Frame {
                kind: fields[0].into(),
                owner: fields[1].into(),
                generation,
                memory: fields[3].into(),
            });
            previous = fields[5].into();
        }
        Ok(out)
    }
}
fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn seed(path: &Path) -> Store {
    let store = Store::open(path, &ClockAt).unwrap();
    store.transact(|tx| {
        tx.inner.execute("INSERT INTO p7_owners VALUES(?1,70,'ACTIVE',0)", [OWNER])?;
        tx.inner.execute("INSERT INTO p7_resource_accounting(owner_id) VALUES(?1)", [OWNER])?;
        tx.inner.execute("INSERT INTO p7_ledger_state(owner_id,read_state) VALUES(?1,'READY')", [OWNER])?;
        tx.inner.execute("INSERT INTO p7_memory_items(memory_id,owner_id,class_rank,kind,body,created_at_ms,expires_at_ms) VALUES(?1,?2,0,'WORKING','synthetic file recovery',1,2000000000)", params![MEMORY, OWNER])?;
        tx.inner.execute("INSERT INTO p7_memory_provenance(memory_id,owner_id,source_id,source_class_rank,metadata_bytes,recorded_at_ms) VALUES(?1,?2,'synthetic-source',0,0,1)", params![MEMORY, OWNER])?;
        Ok(())
    }).unwrap();
    store
}
fn open_after_reconciliation(path: &Path, ledger: &Ledger) -> Result<Store, StoreError> {
    let store = Store::open(path, &ClockAt)?;
    reconcile(&store, ledger)?;
    Ok(store)
}
fn visible(store: &Store) -> i64 {
    store
        .transact(|tx| {
            Ok(tx.inner.query_row(
                "SELECT count(*) FROM p7_memory_items WHERE owner_id=?1 AND memory_id=?2",
                params![OWNER, MEMORY],
                |r| r.get(0),
            )?)
        })
        .unwrap()
}
fn apply_intent(store: &Store, frame: &Frame) -> Result<(), StoreError> {
    store.transact(|tx| {
        tx.inner.execute("UPDATE p7_ledger_state SET read_state='RECOVERY_REQUIRED' WHERE owner_id=?1", [frame.owner.as_str()])?;
        tx.inner.execute("INSERT INTO p7_memory_tombstones VALUES(?1,?2,?3,?4,1000) ON CONFLICT(owner_id,memory_id) DO NOTHING", params![frame.owner,frame.memory,format!("forget-{}",frame.generation),frame.generation as i64])?;
        tx.inner.execute("DELETE FROM p7_memory_items WHERE owner_id=?1 AND memory_id=?2", params![frame.owner,frame.memory])?;
        tx.inner.execute("UPDATE p7_ledger_state SET generation=?2 WHERE owner_id=?1", params![frame.owner,frame.generation as i64])?;
        Ok(())
    })
}
fn reconcile(store: &Store, ledger: &Ledger) -> Result<bool, StoreError> {
    reconcile_with_finalize_failure(store, ledger, false)
}
fn reconcile_with_finalize_failure(
    store: &Store,
    ledger: &Ledger,
    fail_finalize: bool,
) -> Result<bool, StoreError> {
    let frames = ledger.read()?;
    let Some(latest) = frames.last() else {
        return Err(StoreError::Sqlite);
    };
    if latest.kind == "FINALIZED" {
        store.transact(|tx| {
            let generation: i64 = tx.inner.query_row(
                "SELECT generation FROM p7_ledger_state WHERE owner_id=?1",
                [latest.owner.as_str()],
                |r| r.get(0),
            )?;
            if generation != latest.generation as i64 {
                return Err(StoreError::Sqlite);
            }
            tx.inner.execute(
                "UPDATE p7_ledger_state SET read_state='READY' WHERE owner_id=?1",
                [latest.owner.as_str()],
            )?;
            Ok(())
        })?;
        return Ok(true);
    }
    if latest.kind != "INTENT" {
        return Err(StoreError::Sqlite);
    }
    apply_intent(store, latest)?;
    let finalized = Frame {
        kind: "FINALIZED".into(),
        ..(*latest).clone()
    };
    ledger.append(&finalized, fail_finalize)?;
    reconcile(store, ledger)
}

#[test]
fn durable_intent_precedes_primary_delete_restart_replays_and_repeated_recovery_is_idempotent() {
    let scratch = Scratch::new();
    let ledger = Ledger::new(scratch.ledger());
    let store = seed(&scratch.db());
    let intent = Frame {
        kind: "INTENT".into(),
        owner: OWNER.into(),
        generation: 1,
        memory: MEMORY.into(),
    };
    ledger.append(&intent, false).unwrap();
    assert_eq!(
        visible(&store),
        1,
        "intent is durable before primary mutation"
    );
    drop(store); // controlled close/restart boundary; not simulated power loss.

    let reopened = open_after_reconciliation(&scratch.db(), &ledger).unwrap();
    assert_eq!(visible(&reopened), 0);
    let tombstone: i64 = reopened
        .transact(|tx| {
            Ok(tx.inner.query_row(
                "SELECT count(*) FROM p7_memory_tombstones WHERE owner_id=?1 AND memory_id=?2",
                params![OWNER, MEMORY],
                |r| r.get(0),
            )?)
        })
        .unwrap();
    assert_eq!(tombstone, 1);
    assert!(reconcile(&reopened, &ledger).unwrap());
    assert_eq!(visible(&reopened), 0);
    let states = ledger.read().unwrap();
    assert_eq!(
        states.iter().map(|f| f.kind.as_str()).collect::<Vec<_>>(),
        ["INTENT", "FINALIZED"]
    );
}

#[test]
fn missing_ledger_blocks_startup_before_a_memory_handle_is_returned() {
    let scratch = Scratch::new();
    let store = seed(&scratch.db());
    drop(store);
    let absent = Ledger::new(scratch.0.join("missing/no-ledger"));
    assert!(open_after_reconciliation(&scratch.db(), &absent).is_err());
}

#[test]
fn primary_failure_ledger_append_exhaustion_partial_or_invalid_ledger_quarantines() {
    let scratch = Scratch::new();
    let ledger = Ledger::new(scratch.ledger());
    let store = seed(&scratch.db());
    let intent = Frame {
        kind: "INTENT".into(),
        owner: OWNER.into(),
        generation: 1,
        memory: MEMORY.into(),
    };
    assert!(
        ledger.append(&intent, true).is_err(),
        "simulated storage exhaustion before any frame write"
    );
    assert!(ledger.read().unwrap().is_empty());
    assert_eq!(visible(&store), 1);
    ledger.append(&intent, false).unwrap();
    let failed: Result<(), StoreError> = store.transact(|tx| {
        tx.inner.execute(
            "INSERT INTO p7_memory_tombstones VALUES(?1,?2,'forget-fail',1,1000)",
            params![OWNER, MEMORY],
        )?;
        tx.inner.execute(
            "DELETE FROM p7_memory_items WHERE owner_id=?1 AND memory_id=?2",
            params![OWNER, MEMORY],
        )?;
        Err(StoreError::Sqlite)
    });
    assert!(failed.is_err());
    assert_eq!(
        visible(&store),
        1,
        "primary transaction failure rolls back completely"
    );
    assert!(reconcile(&store, &ledger).unwrap());
    assert_eq!(visible(&store), 0);

    let partial = Scratch::new();
    let partial_ledger = Ledger::new(partial.ledger());
    partial_ledger.append_partial_chunk();
    assert!(
        partial_ledger.read().is_err(),
        "truncated final chunk fails closed"
    );
    assert!(
        Ledger::new(partial.0.join("missing/ledger"))
            .read()
            .unwrap()
            .is_empty()
    );

    let invalid = Scratch::new();
    let invalid_ledger = Ledger::new(invalid.ledger());
    invalid_ledger
        .append(
            &Frame {
                kind: "INTENT".into(),
                owner: OWNER.into(),
                generation: 1,
                memory: MEMORY.into(),
            },
            false,
        )
        .unwrap();
    let mut bytes = fs::read(invalid.ledger()).unwrap();
    let corruption = bytes.len() - 3;
    bytes[corruption] ^= 1;
    fs::write(invalid.ledger(), bytes).unwrap();
    assert!(
        invalid_ledger.read().is_err(),
        "tampered frame fails the unkeyed chain-digest check"
    );
}

#[test]
fn primary_commit_before_failed_finalization_remains_quarantined_until_restart_replay() {
    let scratch = Scratch::new();
    let ledger = Ledger::new(scratch.ledger());
    let store = seed(&scratch.db());
    let intent = Frame {
        kind: "INTENT".into(),
        owner: OWNER.into(),
        generation: 1,
        memory: MEMORY.into(),
    };
    ledger.append(&intent, false).unwrap();
    assert!(reconcile_with_finalize_failure(&store, &ledger, true).is_err());
    assert_eq!(
        visible(&store),
        0,
        "primary Forget committed before finalization failed"
    );
    let state: String = store
        .transact(|tx| {
            Ok(tx.inner.query_row(
                "SELECT read_state FROM p7_ledger_state WHERE owner_id=?1",
                [OWNER],
                |r| r.get(0),
            )?)
        })
        .unwrap();
    assert_eq!(state, "RECOVERY_REQUIRED");
    drop(store);

    let reopened = Store::open(&scratch.db(), &ClockAt).unwrap();
    assert!(reconcile(&reopened, &ledger).unwrap());
    assert_eq!(visible(&reopened), 0);
    let state: String = reopened
        .transact(|tx| {
            Ok(tx.inner.query_row(
                "SELECT read_state FROM p7_ledger_state WHERE owner_id=?1",
                [OWNER],
                |r| r.get(0),
            )?)
        })
        .unwrap();
    assert_eq!(state, "READY");
}

#[test]
fn current_ledger_rejects_old_primary_generation_and_complete_rollback_is_unobservable() {
    let scratch = Scratch::new();
    let backup = scratch.0.join("primary/old-snapshot.sqlite");
    let ledger = Ledger::new(scratch.ledger());
    let store = seed(&scratch.db());
    store.checkpoint_for_close().unwrap();
    drop(store);
    fs::copy(scratch.db(), &backup).unwrap();
    let current = Store::open(&scratch.db(), &ClockAt).unwrap();
    let intent = Frame {
        kind: "INTENT".into(),
        owner: OWNER.into(),
        generation: 1,
        memory: MEMORY.into(),
    };
    ledger.append(&intent, false).unwrap();
    reconcile(&current, &ledger).unwrap();
    current.checkpoint_for_close().unwrap();
    drop(current);
    fs::copy(&backup, scratch.db()).unwrap(); // old snapshot restored against current ledger
    let restored = Store::open(&scratch.db(), &ClockAt).unwrap();
    assert!(
        reconcile(&restored, &ledger).is_err(),
        "ledger generation 1 cannot authorize old primary generation 0"
    );
    assert_eq!(
        visible(&restored),
        1,
        "callers must observe quarantine before Memory access"
    );

    // If primary and ledger are both restored together, this local model has no
    // surviving external evidence with which to detect the rollback.
    let rollback = Scratch::new();
    let rollback_ledger = Ledger::new(rollback.ledger());
    let rolled_back = seed(&rollback.db());
    drop(rolled_back);
    fs::copy(backup, rollback.db()).unwrap();
    rollback_ledger
        .path
        .parent()
        .map(fs::create_dir_all)
        .unwrap()
        .unwrap();
    assert!(rollback_ledger.read().unwrap().is_empty());
    let both_old = Store::open(&rollback.db(), &ClockAt).unwrap();
    assert_eq!(visible(&both_old), 1);
}

#[test]
fn extraction_generation_fence_refuses_stale_result_after_forget_intent() {
    let scratch = Scratch::new();
    let store = seed(&scratch.db());
    let ledger = Ledger::new(scratch.ledger());
    let intent = Frame {
        kind: "INTENT".into(),
        owner: OWNER.into(),
        generation: 1,
        memory: MEMORY.into(),
    };
    ledger.append(&intent, false).unwrap();
    store
        .transact(|tx| {
            let captured_generation = 0_i64;
            let current_generation: i64 = tx.inner.query_row(
                "SELECT generation FROM p7_ledger_state WHERE owner_id=?1",
                [OWNER],
                |r| r.get(0),
            )?;
            assert_ne!(
                captured_generation,
                current_generation + 1,
                "Forget intent fences stale extraction commit"
            );
            tx.inner.execute(
                "UPDATE p7_ledger_state SET read_state='RECOVERY_REQUIRED' WHERE owner_id=?1",
                [OWNER],
            )?;
            Ok(())
        })
        .unwrap();
    let state: String = store
        .transact(|tx| {
            Ok(tx.inner.query_row(
                "SELECT read_state FROM p7_ledger_state WHERE owner_id=?1",
                [OWNER],
                |r| r.get(0),
            )?)
        })
        .unwrap();
    assert_eq!(state, "RECOVERY_REQUIRED");
}
