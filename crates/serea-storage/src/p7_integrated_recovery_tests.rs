//! File-backed end-to-end synthetic Gate C coordinator tests. The persistent
//! checkpoint is stored in a separate temporary fixture directory, with a
//! synthetic key. It is neither production custody nor rollback-independent.
use crate::experimental_memory::{
    MemoryClass, MemoryDraft, MemoryKind, MemoryProvenance, P7ExperimentalMemory,
};
use crate::p7_gate_c::{
    AuthEvidence, AuthPurpose, EvidenceReplayGuard, FrameKind, LedgerError, LedgerFileGuard,
    LedgerFrame, OwnerId, PersistentCheckpointStore, SyntheticKeyring, TrustedLedgerCheckpoint,
    verify_stream_at_checkpoint,
};
use crate::{Store, StoreError};
use rusqlite::{OptionalExtension, params};
use serea_protocol::{Clock, EpochMillis, ProtocolError};
use sha2::{Digest, Sha256};
use std::cell::Cell;
use std::fs;
use std::marker::PhantomData;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Barrier};

const OWNER: &str = "own_00000000000000000000000000000071";
const OWNER_EPOCH: u64 = 71;
static NEXT: AtomicU64 = AtomicU64::new(1);

fn crash_at(point: &str) {
    if std::env::var("SEREA_P7_CRASH_AT").is_ok_and(|requested| requested == point) {
        std::process::exit(73);
    }
}

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
    fn checkpoint(&self) -> PathBuf {
        self.0.join("trusted-checkpoint")
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ForgetStatus {
    Pending,
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
    checkpoint: Mutex<PersistentCheckpointStore>,
    replay: Mutex<EvidenceReplayGuard>,
}

/// Non-forgeable, non-clonable permit. It owns the cross-process lock and is
/// tied to one coordinator borrow. Dropping it releases the permit; each
/// operation also rechecks the primary generation/read barrier.
struct VerifiedMemory<'a> {
    coordinator: &'a SyntheticCoordinator,
    _lock: LedgerFileGuard,
    owner: OwnerId,
    generation: u64,
    used: Cell<bool>,
    _unique: PhantomData<&'a mut ()>,
}

impl VerifiedMemory<'_> {
    fn ensure_live(&self) -> Result<(), LedgerError> {
        if self.used.replace(true) {
            return Err(LedgerError::Stale);
        }
        let state: Option<(i64, String)> = self
            .coordinator
            .store
            .transact(|tx| {
                tx.inner
                    .query_row(
                        "SELECT generation,read_state FROM p7_ledger_state WHERE owner_id=?1",
                        [self.owner.as_str()],
                        |r| Ok((r.get(0)?, r.get(1)?)),
                    )
                    .optional()
                    .map_err(StoreError::from)
            })
            .map_err(|_| LedgerError::Stale)?;
        match state {
            Some((generation, state))
                if generation >= 0 && generation as u64 == self.generation && state == "READY" =>
            {
                Ok(())
            }
            _ => Err(LedgerError::Stale),
        }
    }

    fn get(&self, id: &str) -> Result<bool, LedgerError> {
        self.ensure_live()?;
        self.coordinator
            .memory
            .get(
                &self.coordinator.store,
                id,
                EpochMillis::new(10_000).unwrap(),
            )
            .map(|row| row.is_some())
            .map_err(|_| LedgerError::Invalid)
    }

    fn insert(&self, draft: &MemoryDraft) -> Result<(), LedgerError> {
        self.ensure_live()?;
        self.coordinator
            .memory
            .insert(
                &self.coordinator.store,
                draft,
                EpochMillis::new(10_000).unwrap(),
            )
            .map_err(|_| LedgerError::Invalid)
    }

    fn search(&self, literal: &str) -> Result<Vec<String>, LedgerError> {
        self.ensure_live()?;
        self.coordinator
            .memory
            .search(
                &self.coordinator.store,
                MemoryClass::Public,
                literal,
                EpochMillis::new(10_000).unwrap(),
                None,
                20,
            )
            .map(|page| page.records.into_iter().map(|r| r.memory_id).collect())
            .map_err(|_| LedgerError::Invalid)
    }

    fn list(&self) -> Result<Vec<String>, LedgerError> {
        self.ensure_live()?;
        self.coordinator
            .memory
            .list(
                &self.coordinator.store,
                MemoryClass::Public,
                EpochMillis::new(10_000).unwrap(),
                None,
                20,
            )
            .map(|page| page.records.into_iter().map(|r| r.memory_id).collect())
            .map_err(|_| LedgerError::Invalid)
    }

    fn supersede(&self, prior: &str, successor: &str) -> Result<(), LedgerError> {
        self.ensure_live()?;
        self.coordinator
            .memory
            .supersede(
                &self.coordinator.store,
                prior,
                successor,
                EpochMillis::new(10_000).unwrap(),
            )
            .map_err(|_| LedgerError::Invalid)
    }

    fn verify_fts_integrity(&self) -> Result<(), LedgerError> {
        self.ensure_live()?;
        self.coordinator
            .memory
            .verify_fts_integrity(&self.coordinator.store)
            .map_err(|_| LedgerError::Stale)
    }
}

impl SyntheticCoordinator {
    fn open(db: &Path, ledger: &Path, checkpoint_path: &Path) -> Result<Self, StoreError> {
        if db.exists() || ledger.exists() || checkpoint_path.exists() {
            return Err(StoreError::Open);
        }
        let store = Store::open(db, &TestClock)?;
        let owner = OwnerId::parse(OWNER).unwrap();
        let keys = SyntheticKeyring::synthetic(1, b"synthetic-integrated-owner-71").unwrap();
        let mut replay = EvidenceReplayGuard::default();
        let bootstrap = AuthEvidence::from_verified_adapter(
            AuthPurpose::Bootstrap,
            10_000,
            11_000,
            [254; 32],
            "synthetic-bootstrap-host-v1".into(),
        )
        .map_err(|_| StoreError::Open)?;
        replay
            .consume(&bootstrap, AuthPurpose::Bootstrap, 10_100)
            .map_err(|_| StoreError::Open)?;
        fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(ledger)
            .map_err(|_| StoreError::Open)?;
        let checkpoint = TrustedLedgerCheckpoint {
            revision: 1,
            owner: owner.clone(),
            owner_epoch: OWNER_EPOCH,
            key_version: 1,
            final_sequence: 0,
            generation: 0,
            stream_digest: Sha256::digest([]).into(),
        };
        let checkpoint = PersistentCheckpointStore::bootstrap(checkpoint_path, checkpoint, &keys)
            .map_err(|_| StoreError::Open)?;
        store.transact(|tx| {
            tx.inner.execute("INSERT INTO p7_owners VALUES(?1,?2,'ACTIVE',0)", params![OWNER, OWNER_EPOCH as i64])?;
            tx.inner.execute("INSERT INTO p7_resource_accounting(owner_id) VALUES(?1)", [OWNER])?;
            tx.inner.execute("INSERT INTO p7_ledger_state(owner_id,generation,checkpoint_digest,read_state,pending_operation_id,pending_generation) VALUES(?1,0,NULL,'READY',NULL,NULL)", [OWNER])?;
            Ok(())
        })?;
        Ok(Self {
            store,
            memory: P7ExperimentalMemory::fake(OWNER.into(), OWNER_EPOCH),
            ledger_path: ledger.to_path_buf(),
            owner,
            keys,
            checkpoint: Mutex::new(checkpoint),
            replay: Mutex::new(replay),
        })
    }

    fn reopen(db: &Path, ledger: &Path, checkpoint_path: &Path) -> Result<Self, StoreError> {
        let keys = SyntheticKeyring::synthetic(1, b"synthetic-integrated-owner-71").unwrap();
        let checkpoint = PersistentCheckpointStore::open_existing(checkpoint_path, &keys)
            .map_err(|_| StoreError::Open)?;
        let coordinator = Self {
            store: Store::open(db, &TestClock)?,
            memory: P7ExperimentalMemory::fake(OWNER.into(), OWNER_EPOCH),
            ledger_path: ledger.to_path_buf(),
            owner: OwnerId::parse(OWNER).unwrap(),
            keys,
            checkpoint: Mutex::new(checkpoint),
            replay: Mutex::new(EvidenceReplayGuard::default()),
        };
        coordinator
            .recover_startup()
            .map_err(|_| StoreError::Open)?;
        Ok(coordinator)
    }

    fn verify_locked(&self) -> Result<(LedgerFileGuard, Vec<LedgerFrame>, u64), LedgerError> {
        let guard = LedgerFileGuard::acquire_existing(&self.ledger_path)?;
        let frames = guard.read_verified(&self.keys)?;
        let bytes = guard.read_bytes()?;
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
        let mut checkpoint_store = self.checkpoint.lock().map_err(|_| LedgerError::Stale)?;
        checkpoint_store.refresh(&self.keys)?;
        let checkpoint = checkpoint_store.current();
        verify_stream_at_checkpoint(
            &bytes,
            &self.keys,
            Some(checkpoint),
            &self.owner,
            OWNER_EPOCH,
            primary_generation,
        )?;
        Ok((guard, frames, primary_generation))
    }

    /// Startup recovery accepts only a cryptographically verified extension of
    /// the exact persistent checkpoint prefix. It never grants reads until the
    /// primary DB, FTS, terminal ledger and a newly persisted checkpoint agree.
    fn recover_startup(&self) -> Result<(), LedgerError> {
        let guard = LedgerFileGuard::acquire_existing(&self.ledger_path)?;
        let bytes = guard.read_bytes()?;
        let mut checkpoint_guard = self.checkpoint.lock().map_err(|_| LedgerError::Stale)?;
        checkpoint_guard.refresh(&self.keys)?;
        let trusted = checkpoint_guard.current().clone();
        let (_prefix, extension) = crate::p7_gate_c::verify_stream_recovery_extension(
            &bytes,
            &self.keys,
            &trusted,
            &self.owner,
            OWNER_EPOCH,
        )?;
        let primary: Option<(i64, String, Option<String>, Option<i64>)> = self.store.transact(|tx| {
            tx.inner.query_row(
                "SELECT generation,read_state,pending_operation_id,pending_generation FROM p7_ledger_state WHERE owner_id=?1",
                [self.owner.as_str()], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            ).optional().map_err(StoreError::from)
        }).map_err(|_| LedgerError::Stale)?;
        let (db_generation, read_state, pending_operation, pending_generation) =
            primary.ok_or(LedgerError::Stale)?;
        if db_generation < 0 {
            return Err(LedgerError::Stale);
        }
        let frames = guard.read_verified(&self.keys)?;
        if extension.is_empty() {
            if db_generation as u64 != trusted.generation {
                return Err(LedgerError::Generation);
            }
            crate::p7_gate_c::verify_stream_at_checkpoint(
                &bytes,
                &self.keys,
                Some(&trusted),
                &self.owner,
                OWNER_EPOCH,
                db_generation as u64,
            )?;
            self.check_fts_integrity()?;
            self.check_owner_accounting()?;
            match (pending_operation, pending_generation) {
                (Some(op), Some(generation))
                    if generation >= 0
                        && generation as u64 == trusted.generation
                        && frames.last().is_some_and(|f| {
                            f.kind == FrameKind::Reconciled
                                && f.operation_id == op
                                && f.generation == trusted.generation
                        }) =>
                {
                    self.release_ready(trusted.generation)?;
                }
                (None, None) if read_state == "READY" => {}
                _ => return Err(LedgerError::Stale),
            }
            return Ok(());
        }

        let op = extension.first().ok_or(LedgerError::Invalid)?;
        let reconciled_generation = op
            .generation
            .checked_add(1)
            .ok_or(LedgerError::Generation)?;
        if op.kind != FrameKind::Intent
            || op.operation_id.is_empty()
            || op.operation_id.len() > 96
            || op.payload.len() != 32
            || extension.iter().any(|f| {
                f.operation_id != op.operation_id
                    || f.owner != self.owner
                    || f.key_version != op.key_version
                    || f.target_count != op.target_count
                    || f.chunk_count != op.chunk_count
                    || f.closure_digest != op.closure_digest
            })
            || frames
                .iter()
                .any(|f| f.operation_id == op.operation_id && f.sequence <= trusted.final_sequence)
            || extension.iter().any(|f| {
                f.generation != op.generation
                    && !(f.kind == FrameKind::Reconciled && f.generation == reconciled_generation)
            })
            || op.generation
                != trusted
                    .generation
                    .checked_add(1)
                    .ok_or(LedgerError::Generation)?
        {
            return Err(LedgerError::Stale);
        }
        let chunks = extension
            .iter()
            .filter(|f| f.kind == FrameKind::Chunk)
            .collect::<Vec<_>>();
        if chunks.len() != op.chunk_count as usize
            || chunks
                .iter()
                .enumerate()
                .any(|(i, f)| f.chunk_index as usize != i)
        {
            return Err(LedgerError::Invalid);
        }
        let terminal_kinds = extension
            .iter()
            .skip(1 + chunks.len())
            .map(|f| f.kind)
            .collect::<Vec<_>>();
        let expected_terminal = [
            FrameKind::PrimaryApplied,
            FrameKind::Finalized,
            FrameKind::Reconciled,
        ];
        if terminal_kinds.len() > expected_terminal.len()
            || terminal_kinds != expected_terminal[..terminal_kinds.len()]
        {
            return Err(LedgerError::Invalid);
        }
        let adopting_complete_intent = pending_operation.is_none()
            && pending_generation.is_none()
            && read_state == "READY"
            && db_generation as u64 == trusted.generation
            && extension.len() == 1 + chunks.len();
        let matching_pending = pending_operation.as_deref() == Some(op.operation_id.as_str())
            && pending_generation == Some(reconciled_generation as i64)
            && read_state == "QUARANTINED";
        if !adopting_complete_intent && !matching_pending {
            return Err(LedgerError::Stale);
        }
        let encoded = chunks
            .iter()
            .flat_map(|f| f.payload.iter().copied())
            .collect::<Vec<_>>();
        if encoded.len() > crate::p7_gate_c::PROVISIONAL_MAX_CLOSURE_BYTES
            || <[u8; 32]>::from(Sha256::digest(&encoded)) != op.closure_digest
        {
            return Err(LedgerError::Authentication);
        }
        let closure = decode_ids(&encoded)?;
        if closure.len() != op.target_count as usize
            || closure.is_empty()
            || closure.len() > 1000
            || closure.windows(2).any(|pair| pair[0] >= pair[1])
        {
            return Err(LedgerError::Invalid);
        }
        let _root_digest: [u8; 32] = op
            .payload
            .as_slice()
            .try_into()
            .map_err(|_| LedgerError::Invalid)?;
        if adopting_complete_intent {
            // This fixture treats a MAC-valid intent written by the synthetic
            // coordinator as an already-admitted Forget. The test key is not
            // an Owner authenticator; production recovery must independently
            // prove admission and authorization at its real authority boundary.
            self.validate_unapplied_recovered_closure(&op.operation_id, &closure)?;
            let adopted = self.store.transact(|tx| {
                let updated = tx.inner.execute(
                    "UPDATE p7_ledger_state SET read_state='QUARANTINED',pending_operation_id=?2,pending_generation=?3 WHERE owner_id=?1 AND generation=?4 AND read_state='READY' AND pending_operation_id IS NULL AND pending_generation IS NULL",
                    params![self.owner.as_str(), op.operation_id, reconciled_generation as i64, trusted.generation as i64],
                )?;
                if updated != 1 { return Err(StoreError::Sqlite); }
                crash_at("during-pending-transaction");
                Ok(())
            });
            if adopted.is_err() {
                return Err(LedgerError::Stale);
            }
            crash_at("after-pending-commit");
        }
        self.validate_or_apply_recovered_closure(
            &op.operation_id,
            op.generation,
            &closure,
            db_generation as u64,
            trusted.generation,
        )?;
        crash_at("replay-after-primary-commit");

        let has_primary = extension
            .iter()
            .any(|f| f.kind == FrameKind::PrimaryApplied);
        let has_finalized = extension.iter().any(|f| f.kind == FrameKind::Finalized);
        let has_reconciled = extension.iter().any(|f| f.kind == FrameKind::Reconciled);
        let mut sequence = frames.last().map_or(0, |f| f.sequence);
        if !has_primary {
            let record = frame(
                sequence + 1,
                op.generation,
                op.target_count,
                op.chunk_count,
                op.closure_digest,
                self.owner.clone(),
                &op.operation_id,
                FrameKind::PrimaryApplied,
                Vec::new(),
            );
            guard.append_sync(&record, &self.keys)?;
            sequence += 1;
            crash_at("replay-after-primary-record");
        }
        if !has_finalized {
            let record = frame(
                sequence + 1,
                op.generation,
                op.target_count,
                op.chunk_count,
                op.closure_digest,
                self.owner.clone(),
                &op.operation_id,
                FrameKind::Finalized,
                Vec::new(),
            );
            guard.append_sync(&record, &self.keys)?;
            sequence += 1;
            crash_at("replay-after-finalized");
        }
        let final_generation = reconciled_generation;
        if !has_reconciled {
            let record = frame(
                sequence + 1,
                final_generation,
                op.target_count,
                op.chunk_count,
                op.closure_digest,
                self.owner.clone(),
                &op.operation_id,
                FrameKind::Reconciled,
                Vec::new(),
            );
            guard.append_sync(&record, &self.keys)?;
            crash_at("replay-after-reconciled");
        }
        self.set_primary_generation(final_generation)?;
        crash_at("replay-after-primary-generation");
        let new_bytes = guard.read_bytes()?;
        let final_frames = guard.read_verified(&self.keys)?;
        let terminal = final_frames.last().ok_or(LedgerError::Invalid)?;
        let next = TrustedLedgerCheckpoint {
            revision: 0,
            owner: self.owner.clone(),
            owner_epoch: OWNER_EPOCH,
            key_version: terminal.key_version,
            final_sequence: terminal.sequence,
            generation: final_generation,
            stream_digest: Sha256::digest(&new_bytes).into(),
        };
        crate::p7_gate_c::verify_stream_at_checkpoint(
            &new_bytes,
            &self.keys,
            Some(&next),
            &self.owner,
            OWNER_EPOCH,
            final_generation,
        )?;
        self.check_fts_integrity()?;
        self.check_owner_accounting()?;
        drop(checkpoint_guard);
        self.checkpoint
            .lock()
            .map_err(|_| LedgerError::Io)?
            .advance(next, &self.keys)?;
        crash_at("replay-after-checkpoint");
        self.release_ready(final_generation)?;
        Ok(())
    }

    fn validate_unapplied_recovered_closure(
        &self,
        op: &str,
        ids: &[String],
    ) -> Result<(), LedgerError> {
        let valid = self.store.transact(|tx| {
            let mut rows = 0_usize;
            for id in ids {
                let row: bool = tx.inner.query_row(
                    "SELECT EXISTS(SELECT 1 FROM p7_memory_items WHERE owner_id=?1 AND memory_id=?2)",
                    params![self.owner.as_str(), id], |r| r.get(0),
                )?;
                let tombstone: bool = tx.inner.query_row(
                    "SELECT EXISTS(SELECT 1 FROM p7_memory_tombstones WHERE owner_id=?1 AND memory_id=?2)",
                    params![self.owner.as_str(), id], |r| r.get(0),
                )?;
                if !row || tombstone { return Ok(false); }
                rows += 1;
            }
            let prior_operation: bool = tx.inner.query_row(
                "SELECT EXISTS(SELECT 1 FROM p7_memory_tombstones WHERE owner_id=?1 AND forget_op_id=?2)",
                params![self.owner.as_str(), op], |r| r.get(0),
            )?;
            Ok(rows == ids.len() && !prior_operation)
        }).map_err(|_| LedgerError::Stale)?;
        if !valid {
            return Err(LedgerError::Stale);
        }
        Ok(())
    }

    fn validate_or_apply_recovered_closure(
        &self,
        op: &str,
        intent_generation: u64,
        ids: &[String],
        db_generation: u64,
        checkpoint_generation: u64,
    ) -> Result<(), LedgerError> {
        let state = self.store.transact(|tx| {
            let mut tombstones = 0_usize;
            let mut rows = 0_usize;
            for id in ids {
                let tombstone: Option<(String, i64)> = tx.inner.query_row(
                    "SELECT forget_op_id,ledger_generation FROM p7_memory_tombstones WHERE owner_id=?1 AND memory_id=?2",
                    params![self.owner.as_str(), id], |r| Ok((r.get(0)?,r.get(1)?)),
                ).optional()?;
                let row: bool = tx.inner.query_row("SELECT EXISTS(SELECT 1 FROM p7_memory_items WHERE owner_id=?1 AND memory_id=?2)", params![self.owner.as_str(), id], |r| r.get(0))?;
                match tombstone {
                    Some((known_op, generation)) if known_op == op && generation == intent_generation as i64 && !row => tombstones += 1,
                    None if row => rows += 1,
                    _ => return Err(StoreError::Sqlite),
                }
            }
            Ok((tombstones, rows))
        }).map_err(|_| LedgerError::Stale)?;
        if state.0 == ids.len() {
            if db_generation != intent_generation && db_generation != intent_generation + 1 {
                return Err(LedgerError::Generation);
            }
            let leftovers = self.store.transact(|tx| {
                tx.inner.query_row(
                    "SELECT (SELECT count(*) FROM p7_memory_blob_refs WHERE owner_id=?1 AND memory_id IN (SELECT value FROM json_each(?2))) + (SELECT count(*) FROM p7_memory_lineage WHERE owner_id=?1 AND (parent_memory_id IN (SELECT value FROM json_each(?2)) OR derived_memory_id IN (SELECT value FROM json_each(?2))))",
                    params![self.owner.as_str(), serde_json::to_string(ids).map_err(|_| StoreError::Sqlite)?], |r| r.get::<_,i64>(0),
                ).map_err(StoreError::from)
            }).map_err(|_| LedgerError::Stale)?;
            if leftovers != 0 {
                return Err(LedgerError::Stale);
            }
            self.check_owner_accounting()?;
            return Ok(());
        }
        if state.1 != ids.len() || state.0 != 0 || db_generation != checkpoint_generation {
            return Err(LedgerError::Stale);
        }
        let blob_candidates = self.blob_candidates(ids)?;
        self.apply_closure(op, intent_generation, ids, &blob_candidates)?;
        Ok(())
    }

    fn set_primary_generation(&self, generation: u64) -> Result<(), LedgerError> {
        self.store.transact(|tx| {
            tx.inner.execute("UPDATE p7_ledger_state SET generation=?2,read_state='QUARANTINED' WHERE owner_id=?1", params![self.owner.as_str(), generation as i64])?;
            Ok(())
        }).map_err(|_| LedgerError::Io)
    }

    fn release_ready(&self, generation: u64) -> Result<(), LedgerError> {
        self.store.transact(|tx| {
            tx.inner.execute("UPDATE p7_ledger_state SET generation=?2,read_state='READY',pending_operation_id=NULL,pending_generation=NULL WHERE owner_id=?1", params![self.owner.as_str(), generation as i64])?;
            Ok(())
        }).map_err(|_| LedgerError::Io)
    }

    fn check_fts_integrity(&self) -> Result<(), LedgerError> {
        self.store
            .transact(|tx| {
                tx.inner.execute(
                    "INSERT INTO p7_memory_fts(p7_memory_fts,rank) VALUES('integrity-check',1)",
                    [],
                )?;
                Ok(())
            })
            .map_err(|_| LedgerError::Stale)
    }

    fn check_owner_accounting(&self) -> Result<(), LedgerError> {
        let counts = self.store.transact(|tx| {
            let rows = tx.inner.query_row(
                "SELECT count(*),coalesce(sum(active),0),coalesce(sum(CASE WHEN active=1 THEN length(CAST(body AS BLOB)) ELSE 0 END),0) FROM p7_memory_items WHERE owner_id=?1",
                [self.owner.as_str()], |r| Ok((r.get::<_,i64>(0)?,r.get::<_,i64>(1)?,r.get::<_,i64>(2)?)),
            )?;
            let stored = tx.inner.query_row(
                "SELECT retained_items,active_items,active_body_bytes,logical_fts_bytes FROM p7_resource_accounting WHERE owner_id=?1",
                [self.owner.as_str()], |r| Ok((r.get::<_,i64>(0)?,r.get::<_,i64>(1)?,r.get::<_,i64>(2)?,r.get::<_,i64>(3)?)),
            )?;
            Ok((rows, stored))
        }).map_err(|_| LedgerError::Stale)?;
        if counts.0 != (counts.1.0, counts.1.1, counts.1.2) || counts.1.2 != counts.1.3 {
            return Err(LedgerError::Stale);
        }
        Ok(())
    }

    fn memory_capability(&self) -> Result<VerifiedMemory<'_>, LedgerError> {
        let (lock, _, generation) = self.verify_locked()?;
        Ok(VerifiedMemory {
            coordinator: self,
            _lock: lock,
            owner: self.owner.clone(),
            generation,
            used: Cell::new(false),
            _unique: PhantomData,
        })
    }

    fn insert(&self, draft: &MemoryDraft) -> Result<(), LedgerError> {
        self.memory_capability()?.insert(draft)
    }
    fn search(&self, literal: &str) -> Result<Vec<String>, LedgerError> {
        self.memory_capability()?.search(literal)
    }
    fn get(&self, id: &str) -> Result<bool, LedgerError> {
        self.memory_capability()?.get(id)
    }

    fn list(&self) -> Result<Vec<String>, LedgerError> {
        self.memory_capability()?.list()
    }

    fn generation(&self) -> Result<u64, LedgerError> {
        self.store
            .transact(|tx| {
                tx.inner
                    .query_row(
                        "SELECT generation FROM p7_ledger_state WHERE owner_id=?1",
                        [self.owner.as_str()],
                        |r| r.get::<_, i64>(0),
                    )
                    .map_err(StoreError::from)
            })
            .map(|n| n as u64)
            .map_err(|_| LedgerError::Stale)
    }

    fn forget_status(&self, op: &str) -> Result<ForgetStatus, LedgerError> {
        let guard = LedgerFileGuard::acquire_existing(&self.ledger_path)?;
        let pending: Option<(Option<String>, i64, String)> = self.store.transact(|tx| tx.inner.query_row(
            "SELECT pending_operation_id,generation,read_state FROM p7_ledger_state WHERE owner_id=?1",
            [self.owner.as_str()], |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?)),
        ).optional().map_err(StoreError::from)).map_err(|_| LedgerError::Stale)?;
        let (pending_op, generation, state) = pending.ok_or(LedgerError::Stale)?;
        if pending_op.as_deref() == Some(op) || state != "READY" {
            return Ok(ForgetStatus::Pending);
        }
        let mut checkpoint = self.checkpoint.lock().map_err(|_| LedgerError::Stale)?;
        checkpoint.refresh(&self.keys)?;
        let cp = checkpoint.current();
        let bytes = guard.read_bytes()?;
        verify_stream_at_checkpoint(
            &bytes,
            &self.keys,
            Some(cp),
            &self.owner,
            OWNER_EPOCH,
            generation as u64,
        )?;
        let frames = guard.read_verified(&self.keys)?;
        if frames
            .iter()
            .any(|f| f.operation_id == op && f.kind == FrameKind::Reconciled)
        {
            Ok(ForgetStatus::AlreadyCompleted)
        } else {
            Err(LedgerError::Invalid)
        }
    }

    fn supersede(&self, prior: &str, successor: &str) -> Result<(), LedgerError> {
        self.memory_capability()?.supersede(prior, successor)
    }

    fn verify_fts_integrity(&self) -> Result<(), LedgerError> {
        self.memory_capability()?.verify_fts_integrity()
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
        // Resolve and bound GC candidates before writing intent so a reference
        // fanout refusal cannot strand a pending Forget.
        let blob_candidates = self.blob_candidates(&closure)?;
        let encoded_ids = encode_ids(&closure)?;
        let closure_digest: [u8; 32] = Sha256::digest(&encoded_ids).into();
        let generation = old_generation
            .checked_add(1)
            .ok_or(LedgerError::Generation)?;
        let reconciliation_generation = generation.checked_add(1).ok_or(LedgerError::Generation)?;

        let chunk_count = encoded_ids
            .len()
            .div_ceil(crate::p7_gate_c::MAX_PAYLOAD_BYTES);
        if chunk_count == 0 || chunk_count > 500 {
            return Err(LedgerError::TooLarge);
        }
        let target_count = u16::try_from(closure.len()).map_err(|_| LedgerError::TooLarge)?;
        let mut sequence = frames.last().map_or(0, |record| record.sequence);
        let mut records = Vec::with_capacity(4 + chunk_count);
        sequence = sequence.checked_add(1).ok_or(LedgerError::Sequence)?;
        records.push(frame(
            sequence,
            generation,
            target_count,
            chunk_count as u16,
            closure_digest,
            self.owner.clone(),
            op,
            FrameKind::Intent,
            requested_digest.to_vec(),
        ));
        for (index, bytes) in encoded_ids
            .chunks(crate::p7_gate_c::MAX_PAYLOAD_BYTES)
            .enumerate()
        {
            sequence = sequence.checked_add(1).ok_or(LedgerError::Sequence)?;
            let chunk = frame(
                sequence,
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
            records.push(chunk);
        }
        for kind in [FrameKind::PrimaryApplied, FrameKind::Finalized] {
            sequence = sequence.checked_add(1).ok_or(LedgerError::Sequence)?;
            records.push(frame(
                sequence,
                generation,
                target_count,
                chunk_count as u16,
                closure_digest,
                self.owner.clone(),
                op,
                kind,
                Vec::new(),
            ));
        }
        sequence = sequence.checked_add(1).ok_or(LedgerError::Sequence)?;
        records.push(frame(
            sequence,
            reconciliation_generation,
            target_count,
            chunk_count as u16,
            closure_digest,
            self.owner.clone(),
            op,
            FrameKind::Reconciled,
            Vec::new(),
        ));

        // The namespace lock is still held. Authenticate the current stream,
        // compute every real encoded frame size, and reserve the checkpoint
        // revision before any durable Forget transition is started.
        let encoded_records = guard.ensure_append_capacity(&records, &self.keys)?;
        let mut checkpoint_store = self.checkpoint.lock().map_err(|_| LedgerError::Io)?;
        checkpoint_store.refresh(&self.keys)?;
        let checkpoint_revision = checkpoint_store
            .current()
            .revision
            .checked_add(1)
            .ok_or(LedgerError::Generation)?;
        if checkpoint_revision > crate::p7_gate_c::MAX_CHECKPOINT_REVISIONS as u64 {
            return Err(LedgerError::TooLarge);
        }
        let mut projected_stream = guard.read_bytes()?;
        for bytes in &encoded_records {
            projected_stream
                .len()
                .checked_add(bytes.len())
                .filter(|length| *length <= crate::p7_gate_c::MAX_LEDGER_BYTES)
                .ok_or(LedgerError::TooLarge)?;
            projected_stream.extend_from_slice(bytes);
        }
        let terminal = records.last().ok_or(LedgerError::Invalid)?;
        let projected_checkpoint = TrustedLedgerCheckpoint {
            revision: checkpoint_revision,
            owner: self.owner.clone(),
            owner_epoch: OWNER_EPOCH,
            key_version: terminal.key_version,
            final_sequence: terminal.sequence,
            generation: reconciliation_generation,
            stream_digest: Sha256::digest(&projected_stream).into(),
        };
        if crate::p7_gate_c::checkpoint_bytes(&projected_checkpoint, &self.keys)?.len()
            > crate::p7_gate_c::MAX_CHECKPOINT_BYTES as usize
        {
            return Err(LedgerError::TooLarge);
        }
        drop(checkpoint_store);
        crash_at("before-intent");

        guard.append_sync(&records[0], &self.keys)?;
        crash_at("after-intent");
        for (chunk_index, record) in records.iter().enumerate().take(1 + chunk_count).skip(1) {
            guard.append_sync(record, &self.keys)?;
            if chunk_index < chunk_count {
                crash_at("after-chunk");
            }
        }
        crash_at("after-last-chunk-before-pending");
        self.store.transact(|tx| {
            let updated = tx.inner.execute(
                "UPDATE p7_ledger_state SET read_state='QUARANTINED',pending_operation_id=?2,pending_generation=?3 WHERE owner_id=?1 AND generation=?4 AND read_state='READY' AND pending_operation_id IS NULL",
                params![self.owner.as_str(),op,reconciliation_generation as i64,old_generation as i64],
            )?;
            if updated != 1 { return Err(StoreError::Sqlite); }
            Ok(())
        }).map_err(|_| LedgerError::Io)?;
        crash_at("after-complete-intent");

        self.apply_closure(op, generation, &closure, &blob_candidates)?;
        crash_at("after-primary-commit");
        for record in records.iter().skip(1 + chunk_count) {
            guard.append_sync(record, &self.keys)?;
            crash_at(if record.kind == FrameKind::PrimaryApplied {
                "after-primary-record"
            } else if record.kind == FrameKind::Finalized {
                "after-finalized"
            } else {
                "after-reconciled"
            });
        }
        let bytes = guard.read_bytes()?;
        let latest = guard.read_verified(&self.keys)?;
        let checkpoint = TrustedLedgerCheckpoint {
            revision: 0,
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
        self.store.transact(|tx| {
            tx.inner.execute(
                "UPDATE p7_ledger_state SET generation=?2,read_state='QUARANTINED' WHERE owner_id=?1",
                params![self.owner.as_str(), reconciliation_generation as i64],
            )?;
            Ok(())
        }).map_err(|_| LedgerError::Io)?;
        crash_at("after-primary-generation");
        self.check_fts_integrity()?;
        self.check_owner_accounting()?;
        self.checkpoint
            .lock()
            .map_err(|_| LedgerError::Io)?
            .advance(checkpoint, &self.keys)?;
        crash_at("after-checkpoint");
        crash_at("before-ready");
        self.store
            .transact(|tx| {
                tx.inner.execute(
                    "UPDATE p7_ledger_state SET generation=?2,read_state='READY',pending_operation_id=NULL,pending_generation=NULL WHERE owner_id=?1",
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

    fn blob_candidates(
        &self,
        ids: &[String],
    ) -> Result<Vec<crate::task::BlobCandidate>, LedgerError> {
        self.store
            .transact(|tx| {
                let ids_json = serde_json::to_string(ids).map_err(|_| StoreError::Sqlite)?;
                let mut statement = tx.inner.prepare(
                    "SELECT DISTINCT digest,data_class_rank FROM p7_memory_blob_refs WHERE owner_id=?1 AND memory_id IN (SELECT value FROM json_each(?2)) ORDER BY digest,data_class_rank LIMIT 1001",
                )?;
                let candidates = statement
                    .query_map(params![self.owner.as_str(), ids_json], |row| {
                        Ok((row.get::<_, String>(0)?, row.get::<_, u8>(1)?))
                    })?
                    .collect::<Result<Vec<_>, _>>()?;
                if candidates.len() > 1000 {
                    return Err(StoreError::Sqlite);
                }
                Ok(candidates)
            })
            .map_err(|_| LedgerError::TooLarge)
    }

    fn apply_closure(
        &self,
        op: &str,
        generation: u64,
        ids: &[String],
        blob_candidates: &[crate::task::BlobCandidate],
    ) -> Result<(), LedgerError> {
        self.store.transact(|tx| {
            for id in ids {
                tx.inner.execute("INSERT INTO p7_memory_tombstones(owner_id,memory_id,forget_op_id,ledger_generation,forgotten_at_ms) VALUES(?1,?2,?3,?4,10100)", params![self.owner.as_str(), id, op, generation as i64])?;
            }
            crash_at("during-primary-transaction");
            tx.inner.execute("DELETE FROM p7_memory_lineage WHERE owner_id=?1 AND (parent_memory_id IN (SELECT memory_id FROM p7_memory_tombstones WHERE owner_id=?1 AND forget_op_id=?2) OR derived_memory_id IN (SELECT memory_id FROM p7_memory_tombstones WHERE owner_id=?1 AND forget_op_id=?2))", params![self.owner.as_str(), op])?;
            tx.inner.execute("DELETE FROM p7_memory_blob_refs WHERE owner_id=?1 AND memory_id IN (SELECT memory_id FROM p7_memory_tombstones WHERE owner_id=?1 AND forget_op_id=?2)", params![self.owner.as_str(), op])?;
            crash_at("after-memory-blob-ref-removal");
            tx.inner.execute("DELETE FROM p7_memory_items WHERE owner_id=?1 AND memory_id IN (SELECT memory_id FROM p7_memory_tombstones WHERE owner_id=?1 AND forget_op_id=?2)", params![self.owner.as_str(), op])?;
            crate::task::sweep_blob_candidates(&tx.inner, blob_candidates)?;
            crash_at("after-blob-candidates-swept");
            tx.inner.execute("UPDATE p7_ledger_state SET generation=?2,read_state='QUARANTINED' WHERE owner_id=?1", params![self.owner.as_str(), generation as i64])?;
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
fn decode_ids(bytes: &[u8]) -> Result<Vec<String>, LedgerError> {
    let mut cursor = 0_usize;
    let mut ids = Vec::new();
    while cursor < bytes.len() {
        let end = cursor.checked_add(2).ok_or(LedgerError::Invalid)?;
        let raw_len: [u8; 2] = bytes
            .get(cursor..end)
            .ok_or(LedgerError::Invalid)?
            .try_into()
            .map_err(|_| LedgerError::Invalid)?;
        cursor = end;
        let len = u16::from_be_bytes(raw_len) as usize;
        if len == 0 || len > 128 {
            return Err(LedgerError::Invalid);
        }
        let end = cursor.checked_add(len).ok_or(LedgerError::Invalid)?;
        let id = std::str::from_utf8(bytes.get(cursor..end).ok_or(LedgerError::Invalid)?)
            .map_err(|_| LedgerError::Invalid)?;
        ids.push(id.to_owned());
        if ids.len() > 1000 {
            return Err(LedgerError::TooLarge);
        }
        cursor = end;
    }
    if encode_ids(&ids)? != bytes {
        return Err(LedgerError::Invalid);
    }
    Ok(ids)
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
    auth_for(AuthPurpose::Forget, nonce)
}
fn auth_for(purpose: AuthPurpose, nonce: u8) -> AuthEvidence {
    AuthEvidence::from_verified_adapter(
        purpose,
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
    let gate = SyntheticCoordinator::open(&scratch.db(), &scratch.ledger(), &scratch.checkpoint())
        .unwrap();
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
    gate.verify_fts_integrity().unwrap();
    assert_eq!(
        gate.forget(&auth(10), "forget-chain-1", &["root"]).unwrap(),
        ForgetStatus::AlreadyCompleted
    );
    assert_eq!(
        gate.forget_status("forget-chain-1").unwrap(),
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

    drop(gate);
    let gate =
        SyntheticCoordinator::reopen(&scratch.db(), &scratch.ledger(), &scratch.checkpoint())
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
        gate.get("unrelated").is_err(),
        "stale checkpoint denies coordinator access"
    );
}

#[test]
fn p7_ir01_forgotten_memory_id_cannot_be_reinserted() {
    let scratch = Scratch::new();
    let gate = SyntheticCoordinator::open(&scratch.db(), &scratch.ledger(), &scratch.checkpoint())
        .unwrap();
    gate.insert(&draft("retired-id", "synthetic original"))
        .unwrap();
    assert_eq!(
        gate.forget(&auth(91), "retire-id", &["retired-id"])
            .unwrap(),
        ForgetStatus::Completed
    );
    let reinsert = gate
        .insert(&draft("retired-id", "synthetic replacement"))
        .unwrap_err();
    assert_eq!(reinsert, LedgerError::Invalid);
    let direct = gate.store.transact(|tx| {
        tx.inner.execute(
            "INSERT INTO p7_memory_items(memory_id,owner_id,class_rank,kind,body,created_at_ms,expires_at_ms) VALUES('retired-id',?1,0,'WORKING','synthetic direct write',10000,1000000)",
            [OWNER],
        )?;
        Ok(())
    });
    assert!(
        direct.is_err(),
        "fixture trigger must reject writes bypassing the API"
    );
    assert_eq!(
        gate.forget(&auth(94), "retire-id", &["retired-id"])
            .unwrap(),
        ForgetStatus::AlreadyCompleted
    );
    let before: (i64,i64,i64) = gate.store.transact(|tx| Ok(tx.inner.query_row(
        "SELECT active_items,active_body_bytes,logical_fts_bytes FROM p7_resource_accounting WHERE owner_id=?1",
        [OWNER], |row| Ok((row.get(0)?,row.get(1)?,row.get(2)?)))?)).unwrap();
    let denied = gate.forget(&auth(95), "new-op-for-retired-id", &["retired-id"]);
    assert!(
        denied.is_err(),
        "a fresh operation cannot create intent for a retired ID"
    );
    let after: (i64,i64,i64) = gate.store.transact(|tx| Ok(tx.inner.query_row(
        "SELECT active_items,active_body_bytes,logical_fts_bytes FROM p7_resource_accounting WHERE owner_id=?1",
        [OWNER], |row| Ok((row.get(0)?,row.get(1)?,row.get(2)?)))?)).unwrap();
    assert_eq!(after, before);
    gate.verify_fts_integrity().unwrap();
    drop(gate);
    let reopened =
        SyntheticCoordinator::reopen(&scratch.db(), &scratch.ledger(), &scratch.checkpoint())
            .unwrap();
    assert!(
        reopened
            .insert(&draft("retired-id", "synthetic after restart"))
            .is_err()
    );
}

#[test]
fn p7_ir02_capacity_refusal_does_not_quarantine_before_intent() {
    let scratch = Scratch::new();
    let gate = SyntheticCoordinator::open(&scratch.db(), &scratch.ledger(), &scratch.checkpoint())
        .unwrap();
    gate.insert(&draft("capacity-root", "synthetic capacity probe"))
        .unwrap();
    drop(gate);
    // Fill the append-only fixture ledger until its first one-item Forget refusal.
    let mut gate =
        SyntheticCoordinator::reopen(&scratch.db(), &scratch.ledger(), &scratch.checkpoint())
            .unwrap();
    let mut refused = false;
    for index in 0..400 {
        let id = format!("capacity-{index:03}");
        gate.insert(&draft(&id, "synthetic fill")).unwrap();
        let before: (String, Option<String>, i64) = gate.store.transact(|tx| Ok(tx.inner.query_row(
            "SELECT read_state,pending_operation_id,generation FROM p7_ledger_state WHERE owner_id=?1",
            [OWNER], |row| Ok((row.get(0)?,row.get(1)?,row.get(2)?)))?)).unwrap();
        if gate
            .forget(&auth(1), &format!("capacity-op-{index:03}"), &[&id])
            .is_err()
        {
            let after: (String, Option<String>, i64) = gate.store.transact(|tx| Ok(tx.inner.query_row(
                "SELECT read_state,pending_operation_id,generation FROM p7_ledger_state WHERE owner_id=?1",
                [OWNER], |row| Ok((row.get(0)?,row.get(1)?,row.get(2)?)))?)).unwrap();
            assert_eq!(
                after, before,
                "pre-admission refusal must preserve READY state and generation"
            );
            refused = true;
            break;
        }
        gate =
            SyntheticCoordinator::reopen(&scratch.db(), &scratch.ledger(), &scratch.checkpoint())
                .unwrap();
    }
    assert!(refused, "finite ledger must eventually refuse Forget");
}

#[test]
fn p7_ir02_checkpoint_exhaustion_refuses_before_quarantine_or_intent() {
    let scratch = Scratch::new();
    let gate = SyntheticCoordinator::open(&scratch.db(), &scratch.ledger(), &scratch.checkpoint())
        .unwrap();
    gate.insert(&draft(
        "checkpoint-capacity-root",
        "synthetic checkpoint probe",
    ))
    .unwrap();
    let mut checkpoint = gate.checkpoint.lock().unwrap().current().clone();
    for revision in 2..=crate::p7_gate_c::MAX_CHECKPOINT_REVISIONS as u64 {
        checkpoint.revision = revision;
        let bytes = crate::p7_gate_c::checkpoint_bytes(&checkpoint, &gate.keys).unwrap();
        fs::write(
            scratch
                .checkpoint()
                .join(format!("checkpoint-{revision:020}.bin")),
            bytes,
        )
        .unwrap();
    }
    let before = gate.store.transact(|tx| Ok(tx.inner.query_row(
        "SELECT read_state,pending_operation_id,generation FROM p7_ledger_state WHERE owner_id=?1",
        [OWNER], |row| Ok((row.get::<_,String>(0)?,row.get::<_,Option<String>>(1)?,row.get::<_,i64>(2)?)))?)).unwrap();
    assert!(
        gate.forget(
            &auth(98),
            "checkpoint-capacity-refused",
            &["checkpoint-capacity-root"]
        )
        .is_err()
    );
    let after = gate.store.transact(|tx| Ok(tx.inner.query_row(
        "SELECT read_state,pending_operation_id,generation FROM p7_ledger_state WHERE owner_id=?1",
        [OWNER], |row| Ok((row.get::<_,String>(0)?,row.get::<_,Option<String>>(1)?,row.get::<_,i64>(2)?)))?)).unwrap();
    assert_eq!(after, before);
}

#[test]
fn p7_ir03_forget_collects_an_exclusive_memory_blob() {
    let scratch = Scratch::new();
    let gate = SyntheticCoordinator::open(&scratch.db(), &scratch.ledger(), &scratch.checkpoint())
        .unwrap();
    gate.insert(&draft("blob-owner", "synthetic blob memory"))
        .unwrap();
    let blob = gate
        .store
        .transact(|tx| {
            tx.put_blob(
                b"\"synthetic blob bytes\"",
                serea_protocol::DataClass::Public,
            )
        })
        .unwrap();
    gate.store.transact(|tx| {
        tx.inner.execute(
            "INSERT INTO p7_memory_blob_refs(memory_id,owner_id,digest,data_class_rank) VALUES('blob-owner',?1,?2,?3)",
            params![OWNER, blob.digest().as_str(), serea_protocol::DataClass::Public.rank()],
        )?;
        Ok(())
    }).unwrap();
    assert_eq!(
        gate.forget(&auth(92), "forget-blob", &["blob-owner"])
            .unwrap(),
        ForgetStatus::Completed
    );
    let blob_count: i64 = gate
        .store
        .transact(|tx| {
            Ok(tx.inner.query_row(
                "SELECT count(*) FROM blobs WHERE digest=?1 AND data_class_rank=?2",
                params![
                    blob.digest().as_str(),
                    serea_protocol::DataClass::Public.rank()
                ],
                |row| row.get(0),
            )?)
        })
        .unwrap();
    assert_eq!(
        blob_count, 0,
        "exclusive forgotten Memory Blob should be collected"
    );
}

#[test]
fn p7_ir03_same_digest_different_class_ranks_are_gc_candidates_separately() {
    let scratch = Scratch::new();
    let gate = SyntheticCoordinator::open(&scratch.db(), &scratch.ledger(), &scratch.checkpoint())
        .unwrap();
    let mut memory = draft("ranked-blob-memory", "synthetic classified blob");
    memory.class = MemoryClass::Personal;
    memory.provenance.source_class = MemoryClass::Personal;
    gate.insert(&memory).unwrap();
    let (public, personal) = gate.store.transact(|tx| {
        let public = tx.put_blob(b"\"same bytes\"", serea_protocol::DataClass::Public)?;
        let personal = tx.put_blob(b"\"same bytes\"", serea_protocol::DataClass::Personal)?;
        tx.inner.execute("INSERT INTO p7_memory_blob_refs(memory_id,owner_id,digest,data_class_rank) VALUES('ranked-blob-memory',?1,?2,0)",params![OWNER,public.digest().as_str()])?;
        tx.inner.execute("INSERT INTO p7_memory_blob_refs(memory_id,owner_id,digest,data_class_rank) VALUES('ranked-blob-memory',?1,?2,1)",params![OWNER,personal.digest().as_str()])?;
        Ok((public,personal))
    }).unwrap();
    assert_eq!(public.digest(), personal.digest());
    assert_ne!(
        serea_protocol::DataClass::Public.rank(),
        serea_protocol::DataClass::Personal.rank()
    );
    gate.forget(&auth(101), "forget-ranked-blob", &["ranked-blob-memory"])
        .unwrap();
    let remaining: i64 = gate
        .store
        .transact(|tx| {
            Ok(tx.inner.query_row(
                "SELECT count(*) FROM blobs WHERE digest=?1 AND data_class_rank IN (0,1)",
                [public.digest().as_str()],
                |row| row.get(0),
            )?)
        })
        .unwrap();
    assert_eq!(remaining, 0);
}

#[test]
fn p7_ir03_shared_memory_blob_survives_until_last_reference_is_forgotten() {
    let scratch = Scratch::new();
    let gate = SyntheticCoordinator::open(&scratch.db(), &scratch.ledger(), &scratch.checkpoint())
        .unwrap();
    gate.insert(&draft("blob-first", "synthetic first memory"))
        .unwrap();
    gate.insert(&draft("blob-second", "synthetic second memory"))
        .unwrap();
    let blob = gate
        .store
        .transact(|tx| {
            tx.put_blob(
                b"\"shared synthetic bytes\"",
                serea_protocol::DataClass::Public,
            )
        })
        .unwrap();
    gate.store.transact(|tx| {
        for memory_id in ["blob-first", "blob-second"] {
            tx.inner.execute(
                "INSERT INTO p7_memory_blob_refs(memory_id,owner_id,digest,data_class_rank) VALUES(?1,?2,?3,?4)",
                params![memory_id,OWNER,blob.digest().as_str(),serea_protocol::DataClass::Public.rank()],
            )?;
        }
        Ok(())
    }).unwrap();
    gate.forget(&auth(96), "forget-first", &["blob-first"])
        .unwrap();
    let count = |gate: &SyntheticCoordinator| {
        gate.store
            .transact(|tx| {
                Ok(tx.inner.query_row(
                    "SELECT count(*) FROM blobs WHERE digest=?1 AND data_class_rank=?2",
                    params![
                        blob.digest().as_str(),
                        serea_protocol::DataClass::Public.rank()
                    ],
                    |row| row.get::<_, i64>(0),
                )?)
            })
            .unwrap()
    };
    assert_eq!(
        count(&gate),
        1,
        "second Memory reference protects the shared Blob"
    );
    gate.forget(&auth(97), "forget-second", &["blob-second"])
        .unwrap();
    assert_eq!(
        count(&gate),
        0,
        "last reference removal makes the Blob collectible"
    );
}

#[test]
fn p7_ir03_task_reference_protects_blob_until_task_reference_is_removed() {
    let scratch = Scratch::new();
    let gate = SyntheticCoordinator::open(&scratch.db(), &scratch.ledger(), &scratch.checkpoint())
        .unwrap();
    gate.insert(&draft("memory-task-shared", "synthetic task shared blob"))
        .unwrap();
    let task_id = "tsk_00000000000000000000000061";
    let blob = gate.store.transact(|tx| {
        tx.inner.execute(
            "INSERT INTO tasks(task_id,kind,title,state,origin_kind,data_class_rank,policy_class_rank,created_at_ms,updated_at_ms,max_model_calls,max_tool_calls,max_attempts_per_step) VALUES(?1,'MAINTENANCE','synthetic','EXECUTING','SYSTEM',0,0,0,0,1,0,0)",
            [task_id],
        )?;
        tx.inner.execute("INSERT INTO p7_task_owner_bindings(task_id,owner_id,admitted_at_ms) VALUES(?1,?2,0)",params![task_id,OWNER])?;
        let blob = tx.put_blob(b"\"task shared bytes\"", serea_protocol::DataClass::Public)?;
        tx.inner.execute("INSERT INTO task_blob_refs(task_id,role,digest,data_class_rank) VALUES(?1,'PLAN',?2,0)",params![task_id,blob.digest().as_str()])?;
        Ok(blob)
    }).unwrap();
    assert_eq!(
        gate.forget(
            &auth(99),
            "forget-memory-task-shared",
            &["memory-task-shared"]
        )
        .unwrap(),
        ForgetStatus::Completed
    );
    let candidates = gate
        .store
        .transact(|tx| {
            crate::task::task_blob_candidates(
                &tx.inner,
                &serea_protocol::TaskId::new(task_id).unwrap(),
            )
        })
        .unwrap();
    assert_eq!(candidates.len(), 1);
    let count = |gate: &SyntheticCoordinator| {
        gate.store
            .transact(|tx| {
                Ok(tx.inner.query_row(
                    "SELECT count(*) FROM blobs WHERE digest=?1 AND data_class_rank=0",
                    [blob.digest().as_str()],
                    |row| row.get::<_, i64>(0),
                )?)
            })
            .unwrap()
    };
    assert_eq!(count(&gate), 1);
    gate.store
        .transact(|tx| {
            tx.inner
                .execute("DELETE FROM task_blob_refs WHERE task_id=?1", [task_id])?;
            crate::task::sweep_blob_candidates(&tx.inner, &candidates)?;
            Ok(())
        })
        .unwrap();
    assert_eq!(count(&gate), 0);
}

#[test]
fn p7_ir03_schedule_and_occurrence_references_protect_blob() {
    let scratch = Scratch::new();
    let gate = SyntheticCoordinator::open(&scratch.db(), &scratch.ledger(), &scratch.checkpoint())
        .unwrap();
    gate.insert(&draft(
        "memory-schedule-shared",
        "synthetic schedule shared blob",
    ))
    .unwrap();
    let schedule_id = "sch_00000000000000000000000071";
    let event_id = "evt_00000000000000000000000071";
    let blob = gate.store.transact(|tx| {
        let blob = tx.put_blob(b"\"schedule shared bytes\"",serea_protocol::DataClass::Public)?;
        tx.inner.execute(
            "INSERT INTO schedules(schedule_id,owner_kind,owner_id,state,revision,trigger_kind,template_digest,template_data_class_rank,policy_class_rank,approval_policy_json,missed_policy,created_at_ms,updated_at_ms) VALUES(?1,'HOST','synthetic-owner','ACTIVE',1,'DEVICE_SESSION_ESTABLISHED',?2,0,0,'{}','SKIP',0,0)",
            params![schedule_id,blob.digest().as_str()],
        )?;
        tx.inner.execute(
            "INSERT INTO schedule_occurrences(schedule_id,occurrence_key,schedule_revision,trigger_kind,source_event_id,source_event_data_class_rank,state,lease_generation,template_digest,template_data_class_rank,created_at_ms,updated_at_ms) VALUES(?1,'occurrence-1',1,'DEVICE_SESSION_ESTABLISHED',?2,0,'PENDING',0,?3,0,0,0)",
            params![schedule_id,event_id,blob.digest().as_str()],
        )?;
        tx.inner.execute("INSERT INTO p7_memory_blob_refs(memory_id,owner_id,digest,data_class_rank) VALUES('memory-schedule-shared',?1,?2,0)",params![OWNER,blob.digest().as_str()])?;
        Ok(blob)
    }).unwrap();
    assert_eq!(
        gate.forget(
            &auth(102),
            "forget-schedule-shared",
            &["memory-schedule-shared"]
        )
        .unwrap(),
        ForgetStatus::Completed
    );
    let candidate = vec![(
        blob.digest().as_str().to_owned(),
        serea_protocol::DataClass::Public.rank(),
    )];
    let count = |gate: &SyntheticCoordinator| {
        gate.store
            .transact(|tx| {
                Ok(tx.inner.query_row(
                    "SELECT count(*) FROM blobs WHERE digest=?1 AND data_class_rank=0",
                    [blob.digest().as_str()],
                    |row| row.get::<_, i64>(0),
                )?)
            })
            .unwrap()
    };
    assert_eq!(count(&gate), 1);
    gate.store
        .transact(|tx| {
            tx.inner.execute(
                "DELETE FROM schedule_occurrences WHERE schedule_id=?1",
                [schedule_id],
            )?;
            crate::task::sweep_blob_candidates(&tx.inner, &candidate)?;
            Ok(())
        })
        .unwrap();
    assert_eq!(
        count(&gate),
        1,
        "Schedule template alone still references Blob"
    );
    gate.store
        .transact(|tx| {
            tx.inner
                .execute("DELETE FROM schedules WHERE schedule_id=?1", [schedule_id])?;
            crate::task::sweep_blob_candidates(&tx.inner, &candidate)?;
            Ok(())
        })
        .unwrap();
    assert_eq!(count(&gate), 0);
}

#[test]
fn p7_ir03_model_call_response_reference_protects_blob() {
    let scratch = Scratch::new();
    let gate = SyntheticCoordinator::open(&scratch.db(), &scratch.ledger(), &scratch.checkpoint())
        .unwrap();
    gate.insert(&draft("memory-model-shared", "synthetic model shared blob"))
        .unwrap();
    let request = serea_protocol::RequestId::new("req_00000000000000000000000071").unwrap();
    let model_call = crate::ModelCallAttemptDraft {
        request_id: request.clone(),
        task_id: None,
        purpose: serea_protocol::ModelPurpose::Chat,
        model_id: serea_protocol::ModelId::new("synthetic-model").unwrap(),
        provider_id: serea_protocol::ProviderId::new("synthetic_provider").unwrap(),
        deployment_class: crate::ModelDeploymentClass::Local,
        data_class: serea_protocol::DataClass::Public,
        relation_kind: crate::ModelAttemptRelationKind::None,
        parent_request_id: None,
        fallback_from_model_id: None,
        price: crate::ModelPriceSnapshot::new(
            serea_protocol::CostClass::Free,
            "synthetic-v1",
            0,
            0,
        ),
        max_context_tokens: 128,
        effective_max_output_tokens: 32,
        dispatch_intent_at: EpochMillis::new(10_000).unwrap(),
    };
    gate.store
        .reserve_model_call(model_call, crate::UsdMicros::new(0).unwrap())
        .unwrap();
    gate.store
        .complete_model_call(
            &request,
            crate::ModelCallCompletion {
                input_tokens: serea_protocol::TokenCount::new(1),
                output_tokens: serea_protocol::TokenCount::new(1),
                latency_ms: 1,
                repair_attempts: 0,
                finish_reason: serea_protocol::FinishReason::Stop,
                recorded_at: EpochMillis::new(10_001).unwrap(),
                accepted_response: crate::ModelResponseStorage {
                    canonical_json: b"\"synthetic model response\"".to_vec(),
                    data_class: serea_protocol::DataClass::Public,
                },
            },
        )
        .unwrap();
    let (digest,rank):(String,u8) = gate.store.transact(|tx| Ok(tx.inner.query_row(
        "SELECT response_blob_digest,response_data_class_rank FROM model_call_attempts WHERE request_id=?1",
        [request.as_str()],|row|Ok((row.get(0)?,row.get(1)?)))?)).unwrap();
    gate.store.transact(|tx| {
        tx.inner.execute("INSERT INTO p7_memory_blob_refs(memory_id,owner_id,digest,data_class_rank) VALUES('memory-model-shared',?1,?2,?3)",params![OWNER,digest,rank])?;
        Ok(())
    }).unwrap();
    assert_eq!(
        gate.forget(&auth(103), "forget-model-shared", &["memory-model-shared"])
            .unwrap(),
        ForgetStatus::Completed
    );
    let count = |gate: &SyntheticCoordinator| {
        gate.store
            .transact(|tx| {
                Ok(tx.inner.query_row(
                    "SELECT count(*) FROM blobs WHERE digest=?1 AND data_class_rank=?2",
                    params![digest, rank],
                    |row| row.get::<_, i64>(0),
                )?)
            })
            .unwrap()
    };
    assert_eq!(count(&gate), 1);
    gate.store
        .transact(|tx| {
            tx.inner.execute(
                "DELETE FROM model_call_attempts WHERE request_id=?1",
                [request.as_str()],
            )?;
            crate::task::sweep_blob_candidates(&tx.inner, &[(digest.clone(), rank)])?;
            Ok(())
        })
        .unwrap();
    assert_eq!(count(&gate), 0);
}

#[test]
fn p7_ir03_gc_candidate_bound_refuses_before_forget_intent() {
    let scratch = Scratch::new();
    let gate = SyntheticCoordinator::open(&scratch.db(), &scratch.ledger(), &scratch.checkpoint())
        .unwrap();
    gate.insert(&draft("gc-bound-memory", "synthetic bounded gc"))
        .unwrap();
    gate.store.transact(|tx| {
        for index in 0..=1000 {
            let content = format!("\"candidate-{index:04}\"");
            let blob = tx.put_blob(content.as_bytes(), serea_protocol::DataClass::Public)?;
            tx.inner.execute(
                "INSERT INTO p7_memory_blob_refs(memory_id,owner_id,digest,data_class_rank) VALUES('gc-bound-memory',?1,?2,0)",
                params![OWNER,blob.digest().as_str()],
            )?;
        }
        Ok(())
    }).unwrap();
    let state = gate.store.transact(|tx| Ok(tx.inner.query_row(
        "SELECT read_state,pending_operation_id,generation FROM p7_ledger_state WHERE owner_id=?1",
        [OWNER],|row|Ok((row.get::<_,String>(0)?,row.get::<_,Option<String>>(1)?,row.get::<_,i64>(2)?)))?)).unwrap();
    let frames = LedgerFileGuard::acquire_existing(scratch.ledger())
        .unwrap()
        .read_verified(&gate.keys)
        .unwrap()
        .len();
    assert!(
        gate.forget(
            &auth(100),
            "gc-candidate-bound-refused",
            &["gc-bound-memory"]
        )
        .is_err()
    );
    let after = gate.store.transact(|tx| Ok(tx.inner.query_row(
        "SELECT read_state,pending_operation_id,generation FROM p7_ledger_state WHERE owner_id=?1",
        [OWNER],|row|Ok((row.get::<_,String>(0)?,row.get::<_,Option<String>>(1)?,row.get::<_,i64>(2)?)))?)).unwrap();
    let frames_after = LedgerFileGuard::acquire_existing(scratch.ledger())
        .unwrap()
        .read_verified(&gate.keys)
        .unwrap()
        .len();
    assert_eq!(after, state);
    assert_eq!(
        frames_after, frames,
        "bounded cleanup refusal must not create a Forget intent"
    );
}

#[test]
fn p7_ir03_crash_during_memory_blob_gc_replays_cleanup_after_reopen() {
    for point in [
        "after-memory-blob-ref-removal",
        "after-blob-candidates-swept",
    ] {
        let scratch = Scratch::new();
        let gate =
            SyntheticCoordinator::open(&scratch.db(), &scratch.ledger(), &scratch.checkpoint())
                .unwrap();
        gate.insert(&draft("gc-crash-memory", "synthetic gc restart"))
            .unwrap();
        let blob = gate
            .store
            .transact(|tx| {
                tx.put_blob(
                    b"\"synthetic gc crash bytes\"",
                    serea_protocol::DataClass::Public,
                )
            })
            .unwrap();
        gate.store.transact(|tx| {
            tx.inner.execute(
                "INSERT INTO p7_memory_blob_refs(memory_id,owner_id,digest,data_class_rank) VALUES('gc-crash-memory',?1,?2,?3)",
                params![OWNER,blob.digest().as_str(),serea_protocol::DataClass::Public.rank()],
            )?;
            Ok(())
        }).unwrap();
        drop(gate);
        let status = Command::new(std::env::current_exe().unwrap())
            .arg("--exact")
            .arg("experimental_memory::p7_integrated_recovery_tests::integrated_forget_crash_child_entry")
            .arg("--nocapture")
            .env("SEREA_P7_CRASH_DB", scratch.db())
            .env("SEREA_P7_CRASH_LEDGER", scratch.ledger())
            .env("SEREA_P7_CRASH_CHECKPOINT", scratch.checkpoint())
            .env("SEREA_P7_CRASH_ROOT", "gc-crash-memory")
            .env("SEREA_P7_CRASH_OP", "gc-crash-op")
            .env("SEREA_P7_CRASH_AT", point)
            .status().unwrap();
        assert_eq!(status.code(), Some(73), "child did not crash at {point}");
        let reopened =
            SyntheticCoordinator::reopen(&scratch.db(), &scratch.ledger(), &scratch.checkpoint())
                .unwrap();
        assert!(!reopened.get("gc-crash-memory").unwrap());
        reopened.verify_fts_integrity().unwrap();
        let (blobs,refs):(i64,i64) = reopened.store.transact(|tx| Ok((
            tx.inner.query_row("SELECT count(*) FROM blobs WHERE digest=?1 AND data_class_rank=?2",params![blob.digest().as_str(),serea_protocol::DataClass::Public.rank()],|row|row.get(0))?,
            tx.inner.query_row("SELECT count(*) FROM p7_memory_blob_refs WHERE memory_id='gc-crash-memory'",[],|row|row.get(0))?,
        ))) .unwrap();
        assert_eq!(
            (blobs, refs),
            (0, 0),
            "replay must finish atomic reference removal and Blob GC"
        );
    }
}

#[test]
fn incomplete_authenticated_intent_is_not_replayed_or_reopened() {
    let scratch = Scratch::new();
    let gate = SyntheticCoordinator::open(&scratch.db(), &scratch.ledger(), &scratch.checkpoint())
        .unwrap();
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
                "UPDATE p7_ledger_state SET read_state='QUARANTINED',pending_operation_id=?2,pending_generation=2 WHERE owner_id=?1",
                params![OWNER, "pending-crash"],
            )?;
            Ok(())
        })
        .unwrap();
    drop(guard);
    assert_eq!(
        gate.forget_status("pending-crash").unwrap(),
        ForgetStatus::Pending
    );
    drop(gate);
    assert!(
        SyntheticCoordinator::reopen(&scratch.db(), &scratch.ledger(), &scratch.checkpoint())
            .is_err(),
        "intent without complete closure chunks fails startup closed"
    );
}

#[test]
fn integrated_forget_crash_child_entry() {
    let (Ok(db), Ok(ledger), Ok(checkpoint)) = (
        std::env::var("SEREA_P7_CRASH_DB"),
        std::env::var("SEREA_P7_CRASH_LEDGER"),
        std::env::var("SEREA_P7_CRASH_CHECKPOINT"),
    ) else {
        return;
    };
    let coordinator =
        SyntheticCoordinator::reopen(Path::new(&db), Path::new(&ledger), Path::new(&checkpoint))
            .unwrap();
    let roots = std::env::var("SEREA_P7_CRASH_ROOTS")
        .map(|value| value.split(',').map(str::to_owned).collect::<Vec<_>>())
        .unwrap_or_else(|_| {
            vec![std::env::var("SEREA_P7_CRASH_ROOT").unwrap_or_else(|_| "crash-root".into())]
        });
    let root_refs = roots.iter().map(String::as_str).collect::<Vec<_>>();
    let operation = std::env::var("SEREA_P7_CRASH_OP").unwrap_or_else(|_| "crash-child-op".into());
    let _ = coordinator.forget(&auth(70), &operation, &root_refs);
}

#[test]
fn ir04_last_chunk_before_pending_recovers_after_red_reproduction() {
    let scratch = Scratch::new();
    let gate = SyntheticCoordinator::open(&scratch.db(), &scratch.ledger(), &scratch.checkpoint())
        .unwrap();
    gate.insert(&draft("ir04-root", "IR-04 authenticated closure"))
        .unwrap();
    let before = gate.checkpoint.lock().unwrap().current().clone();
    let keys = gate.keys.clone();
    drop(gate);
    let status = Command::new(std::env::current_exe().unwrap())
        .arg("--exact")
        .arg("experimental_memory::p7_integrated_recovery_tests::integrated_forget_crash_child_entry")
        .arg("--nocapture")
        .env("SEREA_P7_CRASH_DB", scratch.db())
        .env("SEREA_P7_CRASH_LEDGER", scratch.ledger())
        .env("SEREA_P7_CRASH_CHECKPOINT", scratch.checkpoint())
        .env("SEREA_P7_CRASH_ROOT", "ir04-root")
        .env("SEREA_P7_CRASH_OP", "ir04-op")
        .env("SEREA_P7_CRASH_AT", "after-last-chunk-before-pending")
        .status().unwrap();
    assert_eq!(
        status.code(),
        Some(73),
        "child did not reach deterministic IR-04 checkpoint"
    );
    let guard = LedgerFileGuard::acquire_existing(scratch.ledger()).unwrap();
    let bytes = guard.read_bytes().unwrap();
    let frames = guard.read_verified(&keys).unwrap();
    drop(guard);
    let store = Store::open(&scratch.db(), &TestClock).unwrap();
    let state: (i64, String, Option<String>, Option<i64>) = store.transact(|tx| Ok(tx.inner.query_row(
        "SELECT generation,read_state,pending_operation_id,pending_generation FROM p7_ledger_state WHERE owner_id=?1",
        [OWNER], |row| Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?)),
    )?)).unwrap();
    assert_eq!(
        frames.len(),
        2,
        "expected authenticated Intent plus complete closure chunk"
    );
    assert_eq!(frames[0].kind, FrameKind::Intent);
    assert_eq!(frames[1].kind, FrameKind::Chunk);
    assert_eq!(
        bytes.len(),
        std::fs::metadata(scratch.ledger()).unwrap().len() as usize
    );
    assert_eq!(before.final_sequence, 0);
    assert_eq!(before.generation, 0);
    assert_eq!(state, (0, "READY".into(), None, None));
    let checkpoint = PersistentCheckpointStore::open_existing(scratch.checkpoint(), &keys).unwrap();
    let coordinator = SyntheticCoordinator {
        store,
        memory: P7ExperimentalMemory::fake(OWNER.into(), OWNER_EPOCH),
        ledger_path: scratch.ledger(),
        owner: OwnerId::parse(OWNER).unwrap(),
        keys,
        checkpoint: Mutex::new(checkpoint),
        replay: Mutex::new(EvidenceReplayGuard::default()),
    };
    let recovery = coordinator.recover_startup();
    assert!(
        recovery.is_ok(),
        "IR-04 must adopt the complete authenticated closure; original exact error: {recovery:?}"
    );
    assert!(!coordinator.get("ir04-root").unwrap());
}

#[test]
fn child_termination_after_durable_intent_keeps_reopened_memory_denied() {
    let cases = [
        ("after-intent", false),
        ("during-ledger-append", false),
        ("after-last-chunk-before-pending", true),
        ("after-complete-intent", true),
        ("during-primary-transaction", true),
        ("after-memory-blob-ref-removal", true),
        ("after-blob-candidates-swept", true),
        ("after-primary-commit", true),
        ("after-primary-record", true),
        ("after-finalized", true),
        ("after-reconciled", true),
        ("after-primary-generation", true),
        ("after-checkpoint", true),
        ("during-checkpoint-create", false),
        ("during-checkpoint-write", false),
        ("after-checkpoint-file-sync", true),
        ("before-ready", true),
    ];
    for (point, should_recover) in cases {
        let scratch = Scratch::new();
        let gate =
            SyntheticCoordinator::open(&scratch.db(), &scratch.ledger(), &scratch.checkpoint())
                .unwrap();
        gate.insert(&draft("crash-root", "never reveal after crash"))
            .unwrap();
        drop(gate);
        let status = Command::new(std::env::current_exe().unwrap())
            .arg("--exact")
            .arg("experimental_memory::p7_integrated_recovery_tests::integrated_forget_crash_child_entry")
            .arg("--nocapture")
            .env("SEREA_P7_CRASH_DB", scratch.db())
            .env("SEREA_P7_CRASH_LEDGER", scratch.ledger())
            .env("SEREA_P7_CRASH_CHECKPOINT", scratch.checkpoint())
            .env("SEREA_P7_CRASH_AT", point)
            .status()
            .unwrap();
        assert_eq!(status.code(), Some(73), "child did not stop at {point}");
        let reopened =
            SyntheticCoordinator::reopen(&scratch.db(), &scratch.ledger(), &scratch.checkpoint());
        if should_recover {
            let reopened =
                reopened.unwrap_or_else(|error| panic!("{point} should recover: {error:?}"));
            assert!(
                !reopened.get("crash-root").unwrap(),
                "{point} left forgotten Memory visible"
            );
            assert!(reopened.search("reveal").unwrap().is_empty());
            assert_eq!(reopened.generation().unwrap(), 2);
            assert_eq!(
                reopened
                    .forget(&auth(72), "crash-child-op", &["crash-root"])
                    .unwrap(),
                ForgetStatus::AlreadyCompleted
            );
        } else {
            assert!(reopened.is_err(), "{point} must retain quarantine");
        }
    }
}

#[test]
fn ir04_pending_adoption_sqlite_crash_has_atomic_outcomes() {
    for recovery_point in ["during-pending-transaction", "after-pending-commit"] {
        let scratch = Scratch::new();
        let gate =
            SyntheticCoordinator::open(&scratch.db(), &scratch.ledger(), &scratch.checkpoint())
                .unwrap();
        gate.insert(&draft("ir04-pending-root", "atomic pending adoption"))
            .unwrap();
        drop(gate);
        let child = || {
            let mut command = Command::new(std::env::current_exe().unwrap());
            command
                .arg("--exact")
                .arg("experimental_memory::p7_integrated_recovery_tests::integrated_forget_crash_child_entry")
                .arg("--nocapture")
                .env("SEREA_P7_CRASH_DB", scratch.db())
                .env("SEREA_P7_CRASH_LEDGER", scratch.ledger())
                .env("SEREA_P7_CRASH_CHECKPOINT", scratch.checkpoint());
            command
        };
        let intent_exit = child()
            .env("SEREA_P7_CRASH_ROOT", "ir04-pending-root")
            .env("SEREA_P7_CRASH_OP", "ir04-pending-op")
            .env("SEREA_P7_CRASH_AT", "after-last-chunk-before-pending")
            .status()
            .unwrap();
        assert_eq!(intent_exit.code(), Some(73));
        let adoption_exit = child()
            .env("SEREA_P7_CRASH_AT", recovery_point)
            .status()
            .unwrap();
        assert_eq!(
            adoption_exit.code(),
            Some(73),
            "recovery child missed {recovery_point}"
        );
        let reopened =
            SyntheticCoordinator::reopen(&scratch.db(), &scratch.ledger(), &scratch.checkpoint())
                .unwrap_or_else(|error| {
                    panic!("{recovery_point} must recover atomically: {error:?}")
                });
        assert!(!reopened.get("ir04-pending-root").unwrap());
        assert_eq!(reopened.generation().unwrap(), 2);
        assert_eq!(
            reopened
                .forget(&auth(75), "ir04-pending-op", &["ir04-pending-root"])
                .unwrap(),
            ForgetStatus::AlreadyCompleted
        );
    }
}

#[test]
fn ir04_multichunk_closure_recovers_after_last_chunk_before_pending() {
    let scratch = Scratch::new();
    let gate = SyntheticCoordinator::open(&scratch.db(), &scratch.ledger(), &scratch.checkpoint())
        .unwrap();
    let ids = (0..60)
        .map(|index| format!("ir04-multichunk-{index:02}"))
        .collect::<Vec<_>>();
    for id in &ids {
        gate.insert(&draft(id, "multi chunk recovery")).unwrap();
    }
    drop(gate);
    let status = Command::new(std::env::current_exe().unwrap())
        .arg("--exact")
        .arg("experimental_memory::p7_integrated_recovery_tests::integrated_forget_crash_child_entry")
        .arg("--nocapture")
        .env("SEREA_P7_CRASH_DB", scratch.db())
        .env("SEREA_P7_CRASH_LEDGER", scratch.ledger())
        .env("SEREA_P7_CRASH_CHECKPOINT", scratch.checkpoint())
        .env("SEREA_P7_CRASH_ROOTS", ids.join(","))
        .env("SEREA_P7_CRASH_OP", "ir04-multichunk-op")
        .env("SEREA_P7_CRASH_AT", "after-last-chunk-before-pending")
        .status().unwrap();
    assert_eq!(status.code(), Some(73));
    let reopened =
        SyntheticCoordinator::reopen(&scratch.db(), &scratch.ledger(), &scratch.checkpoint())
            .unwrap();
    for id in &ids {
        assert!(!reopened.get(id).unwrap(), "{id} remained visible");
    }
    assert_eq!(reopened.generation().unwrap(), 2);
    let frames = LedgerFileGuard::acquire_existing(scratch.ledger())
        .unwrap()
        .read_verified(&reopened.keys)
        .unwrap();
    assert!(
        frames
            .iter()
            .filter(|frame| frame.kind == FrameKind::Chunk)
            .count()
            > 1
    );
    assert_eq!(
        reopened
            .forget(
                &auth(74),
                "ir04-multichunk-op",
                &ids.iter().map(String::as_str).collect::<Vec<_>>()
            )
            .unwrap(),
        ForgetStatus::AlreadyCompleted
    );
}

#[test]
fn crash_before_intent_leaves_ready_generation_and_memory_unchanged() {
    let scratch = Scratch::new();
    let gate = SyntheticCoordinator::open(&scratch.db(), &scratch.ledger(), &scratch.checkpoint())
        .unwrap();
    gate.insert(&draft("pre-intent-root", "synthetic remains available"))
        .unwrap();
    drop(gate);
    let status = Command::new(std::env::current_exe().unwrap())
        .arg("--exact")
        .arg("experimental_memory::p7_integrated_recovery_tests::integrated_forget_crash_child_entry")
        .arg("--nocapture")
        .env("SEREA_P7_CRASH_DB", scratch.db())
        .env("SEREA_P7_CRASH_LEDGER", scratch.ledger())
        .env("SEREA_P7_CRASH_CHECKPOINT", scratch.checkpoint())
        .env("SEREA_P7_CRASH_ROOT", "pre-intent-root")
        .env("SEREA_P7_CRASH_AT", "before-intent")
        .status().unwrap();
    assert_eq!(status.code(), Some(73));
    let reopened =
        SyntheticCoordinator::reopen(&scratch.db(), &scratch.ledger(), &scratch.checkpoint())
            .unwrap();
    assert!(reopened.get("pre-intent-root").unwrap());
    assert_eq!(reopened.generation().unwrap(), 0);
    let frames = LedgerFileGuard::acquire_existing(scratch.ledger())
        .unwrap()
        .read_verified(&reopened.keys)
        .unwrap();
    assert!(frames.is_empty());
}

#[test]
fn recovery_itself_is_idempotent_across_process_termination() {
    let scratch = Scratch::new();
    let gate = SyntheticCoordinator::open(&scratch.db(), &scratch.ledger(), &scratch.checkpoint())
        .unwrap();
    gate.insert(&draft("replay-root", "replay exactly once"))
        .unwrap();
    drop(gate);
    let run_child = |point: &str| {
        let mut command = Command::new(std::env::current_exe().unwrap());
        command.arg("--exact")
            .arg("experimental_memory::p7_integrated_recovery_tests::integrated_forget_crash_child_entry")
            .arg("--nocapture")
            .env("SEREA_P7_CRASH_DB", scratch.db())
            .env("SEREA_P7_CRASH_LEDGER", scratch.ledger())
            .env("SEREA_P7_CRASH_CHECKPOINT", scratch.checkpoint())
            .env("SEREA_P7_CRASH_ROOT", "replay-root")
            .env("SEREA_P7_CRASH_AT", point);
        command.status().unwrap().code()
    };
    assert_eq!(run_child("after-last-chunk-before-pending"), Some(73));
    assert_eq!(run_child("replay-after-primary-commit"), Some(73));
    let reopened =
        SyntheticCoordinator::reopen(&scratch.db(), &scratch.ledger(), &scratch.checkpoint())
            .unwrap();
    assert!(!reopened.get("replay-root").unwrap());
    assert_eq!(reopened.generation().unwrap(), 2);
    assert_eq!(
        reopened
            .forget(&auth(73), "crash-child-op", &["replay-root"])
            .unwrap(),
        ForgetStatus::AlreadyCompleted
    );
}

#[test]
fn ir04_recovered_forget_runs_blob_gc_after_last_chunk_crash() {
    let scratch = Scratch::new();
    let gate = SyntheticCoordinator::open(&scratch.db(), &scratch.ledger(), &scratch.checkpoint())
        .unwrap();
    gate.insert(&draft("ir04-gc-root", "recovered blob cleanup"))
        .unwrap();
    let blob = gate
        .store
        .transact(|tx| {
            tx.put_blob(
                b"\"IR-04 recovered blob\"",
                serea_protocol::DataClass::Public,
            )
        })
        .unwrap();
    gate.store.transact(|tx| {
        tx.inner.execute(
            "INSERT INTO p7_memory_blob_refs(memory_id,owner_id,digest,data_class_rank) VALUES('ir04-gc-root',?1,?2,?3)",
            params![OWNER, blob.digest().as_str(), serea_protocol::DataClass::Public.rank()],
        )?;
        Ok(())
    }).unwrap();
    drop(gate);
    let status = Command::new(std::env::current_exe().unwrap())
        .arg("--exact")
        .arg("experimental_memory::p7_integrated_recovery_tests::integrated_forget_crash_child_entry")
        .arg("--nocapture")
        .env("SEREA_P7_CRASH_DB", scratch.db())
        .env("SEREA_P7_CRASH_LEDGER", scratch.ledger())
        .env("SEREA_P7_CRASH_CHECKPOINT", scratch.checkpoint())
        .env("SEREA_P7_CRASH_ROOT", "ir04-gc-root")
        .env("SEREA_P7_CRASH_OP", "ir04-gc-op")
        .env("SEREA_P7_CRASH_AT", "after-last-chunk-before-pending")
        .status().unwrap();
    assert_eq!(status.code(), Some(73));
    let reopened =
        SyntheticCoordinator::reopen(&scratch.db(), &scratch.ledger(), &scratch.checkpoint())
            .unwrap();
    assert!(!reopened.get("ir04-gc-root").unwrap());
    let counts: (i64, i64) = reopened
        .store
        .transact(|tx| {
            Ok((
                tx.inner.query_row(
                    "SELECT count(*) FROM blobs WHERE digest=?1 AND data_class_rank=?2",
                    params![
                        blob.digest().as_str(),
                        serea_protocol::DataClass::Public.rank()
                    ],
                    |row| row.get(0),
                )?,
                tx.inner.query_row(
                    "SELECT count(*) FROM p7_memory_blob_refs WHERE memory_id='ir04-gc-root'",
                    [],
                    |row| row.get(0),
                )?,
            ))
        })
        .unwrap();
    assert_eq!(
        counts,
        (0, 0),
        "recovery must perform the normal bounded Blob GC"
    );
    reopened.verify_fts_integrity().unwrap();
}

#[test]
fn ir04_near_capacity_last_chunk_crash_uses_reserved_recovery_space() {
    let scratch = Scratch::new();
    let gate = SyntheticCoordinator::open(&scratch.db(), &scratch.ledger(), &scratch.checkpoint())
        .unwrap();
    for index in 1..=203 {
        let id = format!("ir04-capacity-{index:03}");
        gate.insert(&draft(&id, "near capacity synthetic fill"))
            .unwrap();
        assert_eq!(
            gate.forget(
                &auth(index as u8),
                &format!("ir04-capacity-op-{index:03}"),
                &[&id]
            )
            .unwrap(),
            ForgetStatus::Completed
        );
    }
    gate.insert(&draft("ir04-capacity-crash", "reserved crash operation"))
        .unwrap();
    drop(gate);
    let before_frames = LedgerFileGuard::acquire_existing(scratch.ledger())
        .unwrap()
        .read_verified(&SyntheticKeyring::synthetic(1, b"synthetic-integrated-owner-71").unwrap())
        .unwrap()
        .len();
    assert_eq!(before_frames, 1015);
    let status = Command::new(std::env::current_exe().unwrap())
        .arg("--exact")
        .arg("experimental_memory::p7_integrated_recovery_tests::integrated_forget_crash_child_entry")
        .arg("--nocapture")
        .env("SEREA_P7_CRASH_DB", scratch.db())
        .env("SEREA_P7_CRASH_LEDGER", scratch.ledger())
        .env("SEREA_P7_CRASH_CHECKPOINT", scratch.checkpoint())
        .env("SEREA_P7_CRASH_ROOT", "ir04-capacity-crash")
        .env("SEREA_P7_CRASH_OP", "ir04-capacity-crash-op")
        .env("SEREA_P7_CRASH_AT", "after-last-chunk-before-pending")
        .status().unwrap();
    assert_eq!(
        status.code(),
        Some(73),
        "preflight must reserve all replay frames at 1,015/1,024 frames"
    );
    let reopened =
        SyntheticCoordinator::reopen(&scratch.db(), &scratch.ledger(), &scratch.checkpoint())
            .unwrap();
    assert!(!reopened.get("ir04-capacity-crash").unwrap());
    let frames = LedgerFileGuard::acquire_existing(scratch.ledger())
        .unwrap()
        .read_verified(&reopened.keys)
        .unwrap();
    assert_eq!(frames.len(), 1020);
}

#[test]
fn ir04_concurrent_startup_recovery_adopts_once_and_both_handles_are_safe() {
    let scratch = Scratch::new();
    let gate = SyntheticCoordinator::open(&scratch.db(), &scratch.ledger(), &scratch.checkpoint())
        .unwrap();
    gate.insert(&draft(
        "ir04-concurrent-root",
        "serialized startup adoption",
    ))
    .unwrap();
    drop(gate);
    let status = Command::new(std::env::current_exe().unwrap())
        .arg("--exact")
        .arg("experimental_memory::p7_integrated_recovery_tests::integrated_forget_crash_child_entry")
        .arg("--nocapture")
        .env("SEREA_P7_CRASH_DB", scratch.db())
        .env("SEREA_P7_CRASH_LEDGER", scratch.ledger())
        .env("SEREA_P7_CRASH_CHECKPOINT", scratch.checkpoint())
        .env("SEREA_P7_CRASH_ROOT", "ir04-concurrent-root")
        .env("SEREA_P7_CRASH_OP", "ir04-concurrent-op")
        .env("SEREA_P7_CRASH_AT", "after-last-chunk-before-pending")
        .status().unwrap();
    assert_eq!(status.code(), Some(73));
    let paths = (scratch.db(), scratch.ledger(), scratch.checkpoint());
    let start = Arc::new(Barrier::new(3));
    let workers = [0, 1].map(|_| {
        let (db, ledger, checkpoint) = paths.clone();
        let start = Arc::clone(&start);
        std::thread::spawn(move || {
            start.wait();
            SyntheticCoordinator::reopen(&db, &ledger, &checkpoint).unwrap()
        })
    });
    start.wait();
    for worker in workers {
        let coordinator = worker.join().unwrap();
        assert!(!coordinator.get("ir04-concurrent-root").unwrap());
        assert_eq!(coordinator.generation().unwrap(), 2);
    }
}

#[test]
fn concurrent_independent_owner_forgets_serialize_checkpoint_advancement() {
    let scratch = Scratch::new();
    let gate = SyntheticCoordinator::open(&scratch.db(), &scratch.ledger(), &scratch.checkpoint())
        .unwrap();
    gate.insert(&draft("parallel-a", "alpha synthetic content"))
        .unwrap();
    gate.insert(&draft("parallel-b", "beta synthetic content"))
        .unwrap();
    drop(gate);
    let paths = (scratch.db(), scratch.ledger(), scratch.checkpoint());
    let start = Arc::new(Barrier::new(3));
    let workers = [
        ("parallel-forget-a", "parallel-a", 81),
        ("parallel-forget-b", "parallel-b", 82),
    ]
    .map(|(operation, root, nonce)| {
        let (db, ledger, checkpoint) = paths.clone();
        let start = Arc::clone(&start);
        std::thread::spawn(move || {
            let coordinator = SyntheticCoordinator::reopen(&db, &ledger, &checkpoint).unwrap();
            start.wait();
            coordinator
                .forget(&auth(nonce), operation, &[root])
                .unwrap()
        })
    });
    start.wait();
    let statuses = workers.map(|worker| worker.join().unwrap());
    assert!(
        statuses
            .iter()
            .all(|status| *status == ForgetStatus::Completed)
    );
    let reopened = SyntheticCoordinator::reopen(&paths.0, &paths.1, &paths.2).unwrap();
    assert!(!reopened.get("parallel-a").unwrap());
    assert!(!reopened.get("parallel-b").unwrap());
    assert_eq!(reopened.generation().unwrap(), 4);
}

#[test]
fn integrated_barrier_rejects_wrong_purpose_and_stale_primary_generation() {
    let scratch = Scratch::new();
    let gate = SyntheticCoordinator::open(&scratch.db(), &scratch.ledger(), &scratch.checkpoint())
        .unwrap();
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
fn capability_is_single_use_generation_bound_and_lost_on_restart() {
    let scratch = Scratch::new();
    let gate = SyntheticCoordinator::open(&scratch.db(), &scratch.ledger(), &scratch.checkpoint())
        .unwrap();
    gate.insert(&draft("capability-item", "private permit evidence"))
        .unwrap();

    let permit = gate.memory_capability().unwrap();
    assert!(permit.get("capability-item").unwrap());
    assert_eq!(
        permit.get("capability-item"),
        Err(LedgerError::Stale),
        "permit cannot be reused"
    );
    drop(permit);

    let stale = gate.memory_capability().unwrap();
    gate.store
        .transact(|tx| {
            tx.inner.execute(
                "UPDATE p7_ledger_state SET generation=generation+1 WHERE owner_id=?1",
                [OWNER],
            )?;
            Ok(())
        })
        .unwrap();
    assert_eq!(
        stale.get("capability-item"),
        Err(LedgerError::Stale),
        "generation change revokes permit"
    );
    drop(stale);
    assert!(
        gate.get("capability-item").is_err(),
        "checkpoint mismatch denies newly minted permits"
    );
}

#[test]
fn missing_checkpoint_or_ledger_does_not_bootstrap_ready_primary() {
    for remove_checkpoint in [true, false] {
        let scratch = Scratch::new();
        let gate =
            SyntheticCoordinator::open(&scratch.db(), &scratch.ledger(), &scratch.checkpoint())
                .unwrap();
        gate.insert(&draft("authority-item", "deny on authority loss"))
            .unwrap();
        drop(gate);
        let missing = if remove_checkpoint {
            scratch.checkpoint()
        } else {
            scratch.ledger()
        };
        fs::rename(&missing, missing.with_extension("offline")).unwrap();
        assert!(
            SyntheticCoordinator::reopen(&scratch.db(), &scratch.ledger(), &scratch.checkpoint())
                .is_err()
        );
    }
}

#[test]
fn synthetic_task_owner_admission_binds_in_one_transaction_and_quarantines_legacy() {
    let scratch = Scratch::new();
    let gate = SyntheticCoordinator::open(&scratch.db(), &scratch.ledger(), &scratch.checkpoint())
        .unwrap();
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
