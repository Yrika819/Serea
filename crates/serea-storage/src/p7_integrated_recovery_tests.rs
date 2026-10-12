//! File-backed end-to-end synthetic Gate C coordinator tests. The checkpoint is
//! deliberately held by the test process as an independent authority fixture;
//! it is not persisted beside the database or ledger and is not production custody.
use crate::experimental_memory::{
    MemoryClass, MemoryDraft, MemoryKind, MemoryProvenance, P7ExperimentalMemory,
};
use crate::p7_gate_c::{
    AuthEvidence, AuthPurpose, EvidenceReplayGuard, FrameKind, LedgerError, LedgerFileGuard,
    LedgerFrame, OwnerId, SyntheticKeyring, TrustedLedgerCheckpoint, verify_stream_at_checkpoint,
};
use crate::{Store, StoreError};
use rusqlite::{OptionalExtension, params};
use serea_protocol::{Clock, EpochMillis, ProtocolError};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Barrier};

const OWNER: &str = "own_00000000000000000000000000000071";
const OWNER_EPOCH: u64 = 71;
static NEXT: AtomicU64 = AtomicU64::new(1);

struct TestClock;
impl Clock for TestClock {
    fn now_ms(&self) -> Result<EpochMillis, ProtocolError> {
        EpochMillis::new(10_000)
    }
}
struct Scratch(PathBuf);
impl Scratch {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "serea-p7-integrated-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }
    fn db(&self) -> PathBuf {
        self.0.join("primary.sqlite")
    }
    fn ledger(&self) -> PathBuf {
        self.0.join("forget.ledger")
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ForgetStatus {
    Completed,
    AlreadyCompleted,
}

/// Test-only gate. It holds the cooperative cross-process lock across evidence
/// verification and each Memory operation. The database's readiness bit alone
/// is never treated as authority.
struct SyntheticCoordinator {
    store: Store,
    memory: P7ExperimentalMemory,
    ledger_path: PathBuf,
    owner: OwnerId,
    keys: SyntheticKeyring,
    checkpoint: Mutex<TrustedLedgerCheckpoint>,
    replay: Mutex<EvidenceReplayGuard>,
}

impl SyntheticCoordinator {
    fn open(db: &Path, ledger: &Path) -> Result<Self, StoreError> {
        let store = Store::open(db, &TestClock)?;
        let owner = OwnerId::parse(OWNER).unwrap();
        let keys = SyntheticKeyring::synthetic(1, b"synthetic-integrated-owner-71").unwrap();
        fs::write(ledger, []).map_err(|_| StoreError::Open)?;
        let checkpoint = TrustedLedgerCheckpoint {
            owner: owner.clone(),
            owner_epoch: OWNER_EPOCH,
            key_version: 1,
            final_sequence: 0,
            generation: 0,
            stream_digest: Sha256::digest([]).into(),
        };
        store.transact(|tx| {
            tx.inner.execute("INSERT INTO p7_owners VALUES(?1,?2,'ACTIVE',0)", params![OWNER, OWNER_EPOCH as i64])?;
            tx.inner.execute("INSERT INTO p7_resource_accounting(owner_id) VALUES(?1)", [OWNER])?;
            tx.inner.execute("INSERT INTO p7_ledger_state(owner_id,generation,checkpoint_digest,read_state) VALUES(?1,0,NULL,'READY')", [OWNER])?;
            Ok(())
        })?;
        Ok(Self {
            store,
            memory: P7ExperimentalMemory::fake(OWNER.into(), OWNER_EPOCH),
            ledger_path: ledger.to_path_buf(),
            owner,
            keys,
            checkpoint: Mutex::new(checkpoint),
            replay: Mutex::new(EvidenceReplayGuard::default()),
        })
    }

    fn reopen(
        db: &Path,
        ledger: &Path,
        checkpoint: TrustedLedgerCheckpoint,
    ) -> Result<Self, StoreError> {
        Ok(Self {
            store: Store::open(db, &TestClock)?,
            memory: P7ExperimentalMemory::fake(OWNER.into(), OWNER_EPOCH),
            ledger_path: ledger.to_path_buf(),
            owner: OwnerId::parse(OWNER).unwrap(),
            keys: SyntheticKeyring::synthetic(1, b"synthetic-integrated-owner-71").unwrap(),
            checkpoint: Mutex::new(checkpoint),
            replay: Mutex::new(EvidenceReplayGuard::default()),
        })
    }

    fn verify_locked(&self) -> Result<(LedgerFileGuard, Vec<LedgerFrame>, u64), LedgerError> {
        let guard = LedgerFileGuard::acquire(&self.ledger_path)?;
        let frames = guard.read_verified(&self.keys)?;
        let bytes = fs::read(&self.ledger_path).map_err(|_| LedgerError::Io)?;
        let primary_generation = self
            .store
            .transact(|tx| {
                let state: Option<(i64, String)> = tx
                    .inner
                    .query_row(
                        "SELECT l.generation,l.read_state FROM p7_ledger_state l WHERE owner_id=?1",
                        [self.owner.as_str()],
                        |r| Ok((r.get(0)?, r.get(1)?)),
                    )
                    .optional()?;
                match state {
                    Some((generation, state)) if state == "READY" && generation >= 0 => {
                        Ok(generation as u64)
                    }
                    _ => Err(StoreError::Sqlite),
                }
            })
            .map_err(|_| LedgerError::Stale)?;
        let checkpoint = self.checkpoint.lock().map_err(|_| LedgerError::Stale)?;
        verify_stream_at_checkpoint(
            &bytes,
            &self.keys,
            Some(&checkpoint),
            &self.owner,
            OWNER_EPOCH,
            primary_generation,
        )?;
        Ok((guard, frames, primary_generation))
    }

    fn insert(&self, draft: &MemoryDraft) -> Result<(), LedgerError> {
        let (_lock, _, _) = self.verify_locked()?;
        self.memory
            .insert(&self.store, draft, EpochMillis::new(10_000).unwrap())
            .map_err(|_| LedgerError::Invalid)
    }
    fn search(&self, literal: &str) -> Result<Vec<String>, LedgerError> {
        let (_lock, _, _) = self.verify_locked()?;
        self.memory
            .search(
                &self.store,
                MemoryClass::Public,
                literal,
                EpochMillis::new(10_000).unwrap(),
                None,
                20,
            )
            .map(|page| page.records.into_iter().map(|r| r.memory_id).collect())
            .map_err(|_| LedgerError::Invalid)
    }
    fn get(&self, id: &str) -> Result<bool, LedgerError> {
        let (_lock, _, _) = self.verify_locked()?;
        self.memory
            .get(&self.store, id, EpochMillis::new(10_000).unwrap())
            .map(|row| row.is_some())
            .map_err(|_| LedgerError::Invalid)
    }

    fn list(&self) -> Result<Vec<String>, LedgerError> {
        let (_lock, _, _) = self.verify_locked()?;
        self.memory
            .list(
                &self.store,
                MemoryClass::Public,
                EpochMillis::new(10_000).unwrap(),
                None,
                20,
            )
            .map(|page| page.records.into_iter().map(|r| r.memory_id).collect())
            .map_err(|_| LedgerError::Invalid)
    }

    fn supersede(&self, prior: &str, successor: &str) -> Result<(), LedgerError> {
        let (_lock, _, _) = self.verify_locked()?;
        self.memory
            .supersede(
                &self.store,
                prior,
                successor,
                EpochMillis::new(10_000).unwrap(),
            )
            .map_err(|_| LedgerError::Invalid)
    }

    fn forget(
        &self,
        evidence: &AuthEvidence,
        op: &str,
        roots: &[&str],
    ) -> Result<ForgetStatus, LedgerError> {
        evidence.validate_for(AuthPurpose::Forget, 10_100)?;
        self.replay
            .lock()
            .map_err(|_| LedgerError::Authentication)?
            .consume(evidence, AuthPurpose::Forget, 10_100)?;
        let (guard, frames, old_generation) = self.verify_locked()?;
        if op.is_empty() || op.len() > 96 || roots.is_empty() || roots.len() > 1000 {
            return Err(LedgerError::Invalid);
        }
        let root_set = canonical_roots(roots)?;
        let requested_roots = encode_ids(&root_set)?;
        let requested_digest: [u8; 32] = Sha256::digest(&requested_roots).into();
        if frames
            .iter()
            .any(|f| f.operation_id == op && f.kind == FrameKind::Reconciled)
        {
            let intent = frames
                .iter()
                .find(|f| f.operation_id == op && f.kind == FrameKind::Intent)
                .ok_or(LedgerError::Invalid)?;
            return if intent.payload.as_slice() == requested_digest {
                Ok(ForgetStatus::AlreadyCompleted)
            } else {
                Err(LedgerError::Invalid)
            };
        }
        let root_refs = root_set.iter().map(String::as_str).collect::<Vec<_>>();
        let closure = self.resolve_closure(&root_refs)?;
        if closure.is_empty() || closure.len() > 1000 {
            return Err(LedgerError::TooLarge);
        }
        let encoded_ids = encode_ids(&closure)?;
        let closure_digest: [u8; 32] = Sha256::digest(&encoded_ids).into();
        let generation = old_generation
            .checked_add(1)
            .ok_or(LedgerError::Generation)?;
        let reconciliation_generation = generation.checked_add(1).ok_or(LedgerError::Generation)?;

        // Close the database read/extraction gate before writing an intent. If a
        // later step fails, the durable DB bit remains denied on reopen.
        self.store
            .transact(|tx| {
                tx.inner.execute(
                    "UPDATE p7_ledger_state SET read_state='QUARANTINED' WHERE owner_id=?1",
                    [self.owner.as_str()],
                )?;
                Ok(())
            })
            .map_err(|_| LedgerError::Io)?;

        let chunk_count = encoded_ids
            .len()
            .div_ceil(crate::p7_gate_c::MAX_PAYLOAD_BYTES);
        if chunk_count == 0 || chunk_count > 500 {
            return Err(LedgerError::TooLarge);
        }
        let target_count = u16::try_from(closure.len()).map_err(|_| LedgerError::TooLarge)?;
        let mut sequence = frames.len() as u64;
        let mut intent = frame(
            sequence + 1,
            generation,
            target_count,
            chunk_count as u16,
            closure_digest,
            self.owner.clone(),
            op,
            FrameKind::Intent,
            requested_digest.to_vec(),
        );
        sequence += 1;
        guard.append_sync(&intent, &self.keys)?;
        for (index, bytes) in encoded_ids
            .chunks(crate::p7_gate_c::MAX_PAYLOAD_BYTES)
            .enumerate()
        {
            let chunk = frame(
                sequence + 1,
                generation,
                target_count,
                chunk_count as u16,
                closure_digest,
                self.owner.clone(),
                op,
                FrameKind::Chunk,
                bytes.to_vec(),
            );
            let mut chunk = chunk;
            chunk.chunk_index = index as u16;
            sequence += 1;
            guard.append_sync(&chunk, &self.keys)?;
        }
        intent.payload.clear();

        self.apply_closure(op, generation, &closure)?;
        for kind in [FrameKind::PrimaryApplied, FrameKind::Finalized] {
            let record = frame(
                sequence + 1,
                generation,
                target_count,
                chunk_count as u16,
                closure_digest,
                self.owner.clone(),
                op,
                kind,
                Vec::new(),
            );
            sequence += 1;
            guard.append_sync(&record, &self.keys)?;
        }
        let reconciled = frame(
            sequence + 1,
            reconciliation_generation,
            target_count,
            chunk_count as u16,
            closure_digest,
            self.owner.clone(),
            op,
            FrameKind::Reconciled,
            Vec::new(),
        );
        guard.append_sync(&reconciled, &self.keys)?;
        let bytes = fs::read(&self.ledger_path).map_err(|_| LedgerError::Io)?;
        let latest = guard.read_verified(&self.keys)?;
        let checkpoint = TrustedLedgerCheckpoint {
            owner: self.owner.clone(),
            owner_epoch: OWNER_EPOCH,
            key_version: 1,
            final_sequence: latest.last().ok_or(LedgerError::Stale)?.sequence,
            generation: reconciliation_generation,
            stream_digest: Sha256::digest(&bytes).into(),
        };
        verify_stream_at_checkpoint(
            &bytes,
            &self.keys,
            Some(&checkpoint),
            &self.owner,
            OWNER_EPOCH,
            reconciliation_generation,
        )?;
        *self.checkpoint.lock().map_err(|_| LedgerError::Io)? = checkpoint;
        self.store
            .transact(|tx| {
                tx.inner.execute(
                    "UPDATE p7_ledger_state SET generation=?2,read_state='READY' WHERE owner_id=?1",
                    params![self.owner.as_str(), reconciliation_generation as i64],
                )?;
                Ok(())
            })
            .map_err(|_| LedgerError::Io)?;
        Ok(ForgetStatus::Completed)
    }

    fn resolve_closure(&self, roots: &[&str]) -> Result<Vec<String>, LedgerError> {
        self.store.transact(|tx| {
            for root in roots {
                let exists: bool = tx.inner.query_row("SELECT EXISTS(SELECT 1 FROM p7_memory_items WHERE owner_id=?1 AND memory_id=?2)", params![self.owner.as_str(), root], |r| r.get(0))?;
                if !exists { return Err(StoreError::Sqlite); }
            }
            let mut statement = tx.inner.prepare(
                "WITH RECURSIVE closure(id) AS (SELECT memory_id FROM p7_memory_items WHERE owner_id=?1 AND memory_id IN (SELECT value FROM json_each(?2)) UNION SELECT e.derived_memory_id FROM p7_memory_lineage e JOIN closure c ON e.parent_memory_id=c.id WHERE e.owner_id=?1) SELECT id FROM closure ORDER BY id COLLATE BINARY LIMIT 1001"
            )?;
            let roots_json = serde_json::to_string(roots).map_err(|_| StoreError::Sqlite)?;
            let result = statement.query_map(params![self.owner.as_str(), roots_json], |r| r.get(0))?
                .collect::<Result<Vec<String>, _>>()?;
            Ok(result)
        }).map_err(|_| LedgerError::Invalid)
    }

    fn apply_closure(&self, op: &str, generation: u64, ids: &[String]) -> Result<(), LedgerError> {
        self.store.transact(|tx| {
            for id in ids {
                tx.inner.execute("INSERT INTO p7_memory_tombstones(owner_id,memory_id,forget_op_id,ledger_generation,forgotten_at_ms) VALUES(?1,?2,?3,?4,10100)", params![self.owner.as_str(), id, op, generation as i64])?;
            }
            tx.inner.execute("DELETE FROM p7_memory_lineage WHERE owner_id=?1 AND (parent_memory_id IN (SELECT memory_id FROM p7_memory_tombstones WHERE owner_id=?1 AND forget_op_id=?2) OR derived_memory_id IN (SELECT memory_id FROM p7_memory_tombstones WHERE owner_id=?1 AND forget_op_id=?2))", params![self.owner.as_str(), op])?;
            tx.inner.execute("DELETE FROM p7_memory_blob_refs WHERE owner_id=?1 AND memory_id IN (SELECT memory_id FROM p7_memory_tombstones WHERE owner_id=?1 AND forget_op_id=?2)", params![self.owner.as_str(), op])?;
            tx.inner.execute("DELETE FROM p7_memory_items WHERE owner_id=?1 AND memory_id IN (SELECT memory_id FROM p7_memory_tombstones WHERE owner_id=?1 AND forget_op_id=?2)", params![self.owner.as_str(), op])?;
            Ok(())
        }).map_err(|_| LedgerError::Io)
    }
}

fn encode_ids(ids: &[String]) -> Result<Vec<u8>, LedgerError> {
    let mut bytes = Vec::new();
    for id in ids {
        let len = u16::try_from(id.len()).map_err(|_| LedgerError::TooLarge)?;
        bytes.extend_from_slice(&len.to_be_bytes());
        bytes.extend_from_slice(id.as_bytes());
    }
    Ok(bytes)
}
fn canonical_roots(roots: &[&str]) -> Result<Vec<String>, LedgerError> {
    let mut values = roots
        .iter()
        .map(|root| (*root).to_owned())
        .collect::<Vec<_>>();
    if values
        .iter()
        .any(|root| root.is_empty() || root.len() > 128)
    {
        return Err(LedgerError::Invalid);
    }
    values.sort_unstable();
    if values.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err(LedgerError::Invalid);
    }
    Ok(values)
}
#[allow(clippy::too_many_arguments)]
fn frame(
    sequence: u64,
    generation: u64,
    target_count: u16,
    chunk_count: u16,
    digest: [u8; 32],
    owner: OwnerId,
    operation: &str,
    kind: FrameKind,
    payload: Vec<u8>,
) -> LedgerFrame {
    LedgerFrame {
        sequence,
        generation,
        key_version: 1,
        chunk_index: 0,
        chunk_count,
        target_count,
        closure_digest: digest,
        owner,
        operation_id: operation.into(),
        kind,
        payload,
    }
}

fn draft(id: &str, body: &str) -> MemoryDraft {
    MemoryDraft {
        memory_id: id.into(),
        class: MemoryClass::Public,
        kind: MemoryKind::Semantic,
        body: body.into(),
        provenance: MemoryProvenance {
            source_id: format!("source-{id}"),
            source_task_id: None,
            source_class: MemoryClass::Public,
            metadata_bytes: 0,
        },
    }
}
fn auth(nonce: u8) -> AuthEvidence {
    AuthEvidence::from_verified_adapter(
        AuthPurpose::Forget,
        10_000,
        11_000,
        [nonce; 32],
        "synthetic-host-v1".into(),
    )
    .unwrap()
}

struct SyntheticTaskActor {
    owner: OwnerId,
    epoch: u64,
    evidence: AuthEvidence,
}
fn synthetic_task_actor(
    owner: &str,
    epoch: u64,
    purpose: AuthPurpose,
    nonce: u8,
) -> SyntheticTaskActor {
    SyntheticTaskActor {
        owner: OwnerId::parse(owner).unwrap(),
        epoch,
        evidence: AuthEvidence::from_verified_adapter(
            purpose,
            10_000,
            11_000,
            [nonce; 32],
            "synthetic-host-auth-v1".into(),
        )
        .unwrap(),
    }
}
fn admit_synthetic_task(
    store: &Store,
    actor: &SyntheticTaskActor,
    id: &str,
    replay: &Mutex<EvidenceReplayGuard>,
    fail_before_binding: bool,
) -> Result<(), LedgerError> {
    actor
        .evidence
        .validate_for(AuthPurpose::TaskAdmission, 10_100)?;
    let owner_matches = store
        .transact(|tx| {
            let current: Option<i64> = tx
                .inner
                .query_row(
                    "SELECT epoch FROM p7_owners WHERE owner_id=?1 AND state='ACTIVE'",
                    [actor.owner.as_str()],
                    |r| r.get(0),
                )
                .optional()?;
            Ok(current == Some(actor.epoch as i64))
        })
        .map_err(|_| LedgerError::Io)?;
    if !owner_matches {
        return Err(LedgerError::Authentication);
    }
    replay
        .lock()
        .map_err(|_| LedgerError::Authentication)?
        .consume(&actor.evidence, AuthPurpose::TaskAdmission, 10_100)?;
    store.transact(|tx| {
        tx.inner.execute("INSERT INTO tasks(task_id,kind,title,state,origin_kind,origin_extensions,data_class_rank,policy_class_rank,created_at_ms,updated_at_ms,max_model_calls,max_tool_calls,max_attempts_per_step,budget_extensions,extensions) VALUES(?1,'USER_REQUEST','synthetic admission','RECEIVED','USER_MESSAGE','{}',1,0,10100,10100,1,0,1,'{}','{}')", [id])?;
        if fail_before_binding { return Err(StoreError::Sqlite); }
        tx.inner.execute("INSERT INTO p7_task_owner_bindings(task_id,owner_id,admitted_at_ms) VALUES(?1,?2,10100)", params![id, actor.owner.as_str()])?;
        Ok(())
    }).map_err(|_| LedgerError::Invalid)
}

#[test]
fn file_backed_memory_forget_reconciles_full_lineage_and_blocks_stale_ledger() {
    let scratch = Scratch::new();
    let gate = SyntheticCoordinator::open(&scratch.db(), &scratch.ledger()).unwrap();
    gate.insert(&draft("root", "東京 synthetic note")).unwrap();
    gate.insert(&draft("derived-a", "derived one")).unwrap();
    gate.insert(&draft("derived-b", "derived two")).unwrap();
    gate.insert(&draft("unrelated", "keep this Tokyo note"))
        .unwrap();
    gate.insert(&draft("superseded-old", "old fact")).unwrap();
    gate.insert(&draft("superseding-new", "new fact")).unwrap();
    gate.supersede("superseded-old", "superseding-new").unwrap();
    assert!(!gate.list().unwrap().contains(&"superseded-old".to_owned()));
    gate.store
        .transact(|tx| {
            tx.inner.execute(
                "INSERT INTO p7_memory_lineage VALUES(?1,'root','derived-a',1)",
                [OWNER],
            )?;
            tx.inner.execute(
                "INSERT INTO p7_memory_lineage VALUES(?1,'derived-a','derived-b',1)",
                [OWNER],
            )?;
            assert!(
                tx.inner
                    .execute(
                        "INSERT INTO p7_memory_lineage VALUES(?1,'derived-b','root',1)",
                        [OWNER]
                    )
                    .is_err(),
                "lineage cycle is rejected"
            );
            Ok(())
        })
        .unwrap();
    assert!(gate.search("東京").unwrap().contains(&"root".to_owned()));
    let valid_old_prefix = fs::read(scratch.ledger()).unwrap();
    let status = gate.forget(&auth(9), "forget-chain-1", &["root"]).unwrap();
    assert_eq!(status, ForgetStatus::Completed);
    assert!(!gate.get("root").unwrap());
    assert!(!gate.get("derived-a").unwrap());
    assert!(!gate.get("derived-b").unwrap());
    assert!(gate.get("unrelated").unwrap());
    let (retained_old, old_active): (i64, i64) = gate.store.transact(|tx| {
        Ok(tx.inner.query_row(
            "SELECT count(*),coalesce(max(active),0) FROM p7_memory_items WHERE owner_id=?1 AND memory_id='superseded-old'",
            [OWNER],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?)
    }).unwrap();
    assert_eq!(
        (retained_old, old_active),
        (1, 0),
        "superseded content is retained but not visible"
    );
    assert!(gate.get("superseding-new").unwrap());
    assert!(
        !gate
            .search("derived")
            .unwrap()
            .iter()
            .any(|id| id.starts_with("derived"))
    );
    gate.memory.verify_fts_integrity(&gate.store).unwrap();
    assert_eq!(
        gate.forget(&auth(10), "forget-chain-1", &["root"]).unwrap(),
        ForgetStatus::AlreadyCompleted
    );
    assert_eq!(
        gate.forget(&auth(12), "forget-chain-1", &["unrelated"]),
        Err(LedgerError::Invalid),
        "an operation ID cannot acknowledge a different requested target"
    );
    assert_eq!(
        gate.forget(&auth(11), "forget-old-only", &["superseded-old"])
            .unwrap(),
        ForgetStatus::Completed
    );
    assert!(!gate.get("superseded-old").unwrap());
    assert!(gate.get("superseding-new").unwrap());

    let checkpoint_after_forget = gate.checkpoint.lock().unwrap().clone();
    drop(gate);
    let gate =
        SyntheticCoordinator::reopen(&scratch.db(), &scratch.ledger(), checkpoint_after_forget)
            .unwrap();
    assert!(
        gate.get("unrelated").unwrap(),
        "reopen accepts exact independently held checkpoint and matching primary generation"
    );

    // Replace with an older, correctly authenticated complete ledger prefix.
    let mut file = fs::OpenOptions::new()
        .write(true)
        .truncate(true)
        .open(scratch.ledger())
        .unwrap();
    use std::io::Write as _;
    file.write_all(&valid_old_prefix).unwrap();
    file.sync_all().unwrap();
    assert!(
        gate.search("Tokyo").is_err(),
        "stale ledger prefix denies file-backed Memory access"
    );
    assert!(
        gate.memory
            .get(&gate.store, "unrelated", EpochMillis::new(10_000).unwrap())
            .unwrap()
            .is_some(),
        "known isolation limitation: the legacy test-only Memory object checks the SQLite ready bit only; only the integrated coordinator verifies the checkpoint"
    );
}

#[test]
fn durable_unreconciled_intent_and_quarantined_primary_deny_after_reopen() {
    let scratch = Scratch::new();
    let gate = SyntheticCoordinator::open(&scratch.db(), &scratch.ledger()).unwrap();
    gate.insert(&draft("pending-root", "must remain quarantined"))
        .unwrap();
    let owner = gate.owner.clone();
    let intent = frame(
        1,
        1,
        1,
        1,
        [4; 32],
        owner,
        "pending-crash",
        FrameKind::Intent,
        Vec::new(),
    );
    let guard = LedgerFileGuard::acquire(scratch.ledger()).unwrap();
    guard.append_sync(&intent, &gate.keys).unwrap();
    gate.store
        .transact(|tx| {
            tx.inner.execute(
                "UPDATE p7_ledger_state SET read_state='QUARANTINED' WHERE owner_id=?1",
                [OWNER],
            )?;
            Ok(())
        })
        .unwrap();
    drop(guard);
    let checkpoint = gate.checkpoint.lock().unwrap().clone();
    drop(gate);
    let reopened =
        SyntheticCoordinator::reopen(&scratch.db(), &scratch.ledger(), checkpoint).unwrap();
    assert!(reopened.get("pending-root").is_err());
    assert!(reopened.search("quarantined").is_err());
    assert_eq!(
        reopened.forget(&auth(41), "retry-pending-crash", &["pending-root"]),
        Err(LedgerError::Stale),
        "without a trusted advancement authority, recovery leaves the pending operation denied"
    );
}

#[test]
fn integrated_forget_crash_child_entry() {
    let (Ok(db), Ok(ledger)) = (
        std::env::var("SEREA_P7_CRASH_DB"),
        std::env::var("SEREA_P7_CRASH_LEDGER"),
    ) else {
        return;
    };
    let store = Store::open(Path::new(&db), &TestClock).unwrap();
    store
        .transact(|tx| {
            tx.inner.execute(
                "UPDATE p7_ledger_state SET read_state='QUARANTINED' WHERE owner_id=?1",
                [OWNER],
            )?;
            Ok(())
        })
        .unwrap();
    let keys = SyntheticKeyring::synthetic(1, b"synthetic-integrated-owner-71").unwrap();
    let guard = LedgerFileGuard::acquire(&ledger).unwrap();
    guard
        .append_sync(
            &frame(
                1,
                1,
                1,
                1,
                [5; 32],
                OwnerId::parse(OWNER).unwrap(),
                "crash-child-op",
                FrameKind::Intent,
                Vec::new(),
            ),
            &keys,
        )
        .unwrap();
    // Bypass Rust destructors to model abrupt process death after durable intent.
    std::process::exit(73);
}

#[test]
fn child_termination_after_durable_intent_keeps_reopened_memory_denied() {
    let scratch = Scratch::new();
    let gate = SyntheticCoordinator::open(&scratch.db(), &scratch.ledger()).unwrap();
    gate.insert(&draft("crash-root", "never reveal after crash"))
        .unwrap();
    let checkpoint = gate.checkpoint.lock().unwrap().clone();
    drop(gate);
    let status = Command::new(std::env::current_exe().unwrap())
        .arg("--exact")
        .arg("p7_integrated_recovery_tests::integrated_forget_crash_child_entry")
        .arg("--nocapture")
        .env("SEREA_P7_CRASH_DB", scratch.db())
        .env("SEREA_P7_CRASH_LEDGER", scratch.ledger())
        .status()
        .unwrap();
    assert_eq!(status.code(), Some(73));
    let reopened =
        SyntheticCoordinator::reopen(&scratch.db(), &scratch.ledger(), checkpoint).unwrap();
    assert!(reopened.get("crash-root").is_err());
    assert!(reopened.search("reveal").is_err());
}

#[test]
fn integrated_barrier_rejects_wrong_purpose_and_stale_primary_generation() {
    let scratch = Scratch::new();
    let gate = SyntheticCoordinator::open(&scratch.db(), &scratch.ledger()).unwrap();
    let evidence = AuthEvidence::from_verified_adapter(
        AuthPurpose::RenewMemory,
        10_000,
        11_000,
        [12; 32],
        "synthetic-host-v1".into(),
    )
    .unwrap();
    assert_eq!(
        gate.forget(&evidence, "wrong-purpose", &["missing"]),
        Err(LedgerError::Authentication)
    );
    gate.store
        .transact(|tx| {
            tx.inner.execute(
                "UPDATE p7_ledger_state SET generation=1 WHERE owner_id=?1",
                [OWNER],
            )?;
            Ok(())
        })
        .unwrap();
    assert!(
        gate.get("anything").is_err(),
        "database generation ahead of independent checkpoint is denied"
    );
}

#[test]
fn synthetic_task_owner_admission_binds_in_one_transaction_and_quarantines_legacy() {
    let scratch = Scratch::new();
    let gate = SyntheticCoordinator::open(&scratch.db(), &scratch.ledger()).unwrap();
    let replay = Mutex::new(EvidenceReplayGuard::default());
    let actor = synthetic_task_actor(OWNER, OWNER_EPOCH, AuthPurpose::TaskAdmission, 31);
    let id = "tsk_01JQ8Z9K3M7QWXR4V2T6YH0BNA";
    assert_eq!(
        admit_synthetic_task(&gate.store, &actor, id, &replay, true),
        Err(LedgerError::Invalid)
    );
    let after_rollback: (i64, i64) = gate
        .store
        .transact(|tx| {
            Ok((
                tx.inner
                    .query_row("SELECT count(*) FROM tasks WHERE task_id=?1", [id], |r| {
                        r.get(0)
                    })?,
                tx.inner.query_row(
                    "SELECT count(*) FROM p7_task_owner_bindings WHERE task_id=?1",
                    [id],
                    |r| r.get(0),
                )?,
            ))
        })
        .unwrap();
    assert_eq!(
        after_rollback,
        (0, 0),
        "task and binding roll back together"
    );

    // A fresh challenge admits the task; reopening sees the immutable binding.
    let admitted = synthetic_task_actor(OWNER, OWNER_EPOCH, AuthPurpose::TaskAdmission, 32);
    admit_synthetic_task(&gate.store, &admitted, id, &replay, false).unwrap();
    let owner: String = gate
        .store
        .transact(|tx| {
            Ok(tx.inner.query_row(
                "SELECT owner_id FROM p7_task_owner_bindings WHERE task_id=?1",
                [id],
                |r| r.get(0),
            )?)
        })
        .unwrap();
    assert_eq!(owner, OWNER);
    assert!(gate.store.transact(|tx| {
        tx.inner.execute("UPDATE p7_task_owner_bindings SET owner_id='own_00000000000000000000000000000072' WHERE task_id=?1", [id])?;
        Ok(())
    }).is_err(), "Owner binding is immutable");
    assert_eq!(
        admit_synthetic_task(
            &gate.store,
            &admitted,
            "tsk_01JQ8Z9K3M7QWXR4V2T6YH0BNB",
            &replay,
            false
        ),
        Err(LedgerError::Authentication),
        "challenge replay is refused"
    );
    let wrong_owner = synthetic_task_actor(
        "own_00000000000000000000000000000072",
        OWNER_EPOCH,
        AuthPurpose::TaskAdmission,
        33,
    );
    assert_eq!(
        admit_synthetic_task(
            &gate.store,
            &wrong_owner,
            "tsk_01JQ8Z9K3M7QWXR4V2T6YH0BNC",
            &replay,
            false
        ),
        Err(LedgerError::Authentication)
    );

    let legacy = "tsk_01JQ8Z9K3M7QWXR4V2T6YH0BND";
    gate.store.transact(|tx| {
        tx.inner.execute("INSERT INTO tasks(task_id,kind,title,state,origin_kind,origin_extensions,data_class_rank,policy_class_rank,created_at_ms,updated_at_ms,max_model_calls,max_tool_calls,max_attempts_per_step,budget_extensions,extensions) VALUES(?1,'USER_REQUEST','legacy synthetic','RECEIVED','USER_MESSAGE','{}',1,0,10100,10100,1,0,1,'{}','{}')", [legacy])?;
        Ok(())
    }).unwrap();
    let reason: String = gate
        .store
        .transact(|tx| {
            Ok(tx.inner.query_row(
                "SELECT reason_code FROM p7_legacy_task_quarantine WHERE task_id=?1",
                [legacy],
                |r| r.get(0),
            )?)
        })
        .unwrap();
    assert_eq!(reason, "OWNER_UNPROVEN");

    let reopened = Store::open(&scratch.db(), &TestClock).unwrap();
    let persisted: String = reopened
        .transact(|tx| {
            Ok(tx.inner.query_row(
                "SELECT owner_id FROM p7_task_owner_bindings WHERE task_id=?1",
                [id],
                |r| r.get(0),
            )?)
        })
        .unwrap();
    assert_eq!(persisted, OWNER);

    let shared_store = Arc::new(reopened);
    let start = Arc::new(Barrier::new(3));
    let left = synthetic_task_actor(OWNER, OWNER_EPOCH, AuthPurpose::TaskAdmission, 51);
    let right = synthetic_task_actor(OWNER, OWNER_EPOCH, AuthPurpose::TaskAdmission, 52);
    let left_store = Arc::clone(&shared_store);
    let left_start = Arc::clone(&start);
    let left_replay = Arc::new(Mutex::new(EvidenceReplayGuard::default()));
    let right_store = Arc::clone(&shared_store);
    let right_start = Arc::clone(&start);
    let right_replay = Arc::clone(&left_replay);
    let left_thread = std::thread::spawn(move || {
        left_start.wait();
        admit_synthetic_task(
            &left_store,
            &left,
            "tsk_01JQ8Z9K3M7QWXR4V2T6YH0BNE",
            &left_replay,
            false,
        )
    });
    let right_thread = std::thread::spawn(move || {
        right_start.wait();
        admit_synthetic_task(
            &right_store,
            &right,
            "tsk_01JQ8Z9K3M7QWXR4V2T6YH0BNE",
            &right_replay,
            false,
        )
    });
    start.wait();
    let results = [left_thread.join().unwrap(), right_thread.join().unwrap()];
    assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
    assert_eq!(results.iter().filter(|result| result.is_err()).count(), 1);
}
