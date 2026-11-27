//! Synthetic conversation-derived Memory path for P7 Gate C development.
//!
//! This module is nested under `experimental_memory` and is compiled only for
//! tests with both explicit P7 development features. It never calls a model.

use super::{MemoryClass, MemoryDraft, MemoryKind, MemoryProvenance, P7ExperimentalMemory};
use crate::{Store, StoreError};
use rusqlite::OptionalExtension;
use serea_protocol::{Clock, EpochMillis};
use sha2::{Digest as _, Sha256};
use std::collections::{BTreeMap, BTreeSet};

const SYNTHETIC_INPUT_MAX_BYTES: usize = 64 * 1024;
const SYNTHETIC_CANDIDATE_MAX_BYTES: usize = 4096;
const SYNTHETIC_LINEAGE_MAX_BYTES: usize = 1024;
// Provisional test-only expiry; never a production lease guarantee.
const TEST_EXTRACTION_LEASE_TTL_MS: i64 = 60_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ConversationState {
    Complete,
    Failed,
    InProgress,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum TurnState {
    Complete,
    Failed,
    InProgress,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum TurnRole {
    User,
    Assistant,
}

/// Mirrors the frozen classes needed at this extraction boundary. P7 storage
/// currently supports only Public and Personal; every other value is denied.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum DataClass {
    Public,
    Personal,
    Private,
    Secret,
    Credential,
    Unknown,
}

impl DataClass {
    fn memory_class(self) -> Option<MemoryClass> {
        match self {
            Self::Public => Some(MemoryClass::Public),
            Self::Personal => Some(MemoryClass::Personal),
            Self::Private | Self::Secret | Self::Credential | Self::Unknown => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum DataOrigin {
    UserAuthored,
    ModelOutput,
    Provider,
    Other,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct SourceLineage {
    pub source_id: String,
    pub origin: DataOrigin,
    pub classification: DataClass,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ConversationTurn {
    pub turn_id: String,
    pub conversation_id: Option<String>,
    pub role: TurnRole,
    pub state: TurnState,
    pub text: String,
    pub classification: DataClass,
    pub origin: DataOrigin,
    pub source_lineage: Vec<SourceLineage>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct CompletedConversation {
    pub owner_id: String,
    pub conversation_id: Option<String>,
    pub state: ConversationState,
    pub turns: Vec<ConversationTurn>,
}

/// Opaque per-attempt key. Reusing it cannot insert another candidate row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ExtractionRequestId(String);

impl ExtractionRequestId {
    pub fn new(value: impl Into<String>) -> Result<Self, ExtractionError> {
        let value = value.into();
        if value.is_empty()
            || value.len() > 128
            || value.bytes().any(|byte| byte.is_ascii_control())
        {
            return Err(ExtractionError::InvalidRequestId);
        }
        Ok(Self(value))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct MemoryCandidate {
    /// Stable extractor-local key used to derive a tombstone-stable Memory ID.
    pub candidate_key: String,
    pub body: String,
    pub classification: DataClass,
    /// Every referenced source must be present in the sanitized input lineage.
    pub source_ids: Vec<String>,
    pub kind: MemoryKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExtractionError {
    InvalidRequestId,
    ConversationNotComplete,
    MissingConversationId,
    OwnerMismatch,
    NoEligibleTurns,
    InputTooLarge,
    InvalidSourceLineage,
    CandidateRejected,
    ExtractorFailed,
    ConcurrentExtraction,
    DailyLimitReached,
    ClockRollback,
    LeaseExpired,
    StaleLease,
    RequestConflict,
    StaleGeneration,
    StorageFailed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct SaveSummary {
    pub inserted: usize,
    pub already_present: usize,
}

type PriorAdmissionRow = (Vec<u8>, i64, String, i64, Option<Vec<u8>>, i64, i64, i64);

/// The extractor receives only this validated, owner-bound subset.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ValidatedConversation {
    pub owner_id: String,
    pub conversation_id: String,
    pub turns: Vec<ConversationTurn>,
}

pub(super) trait MemoryExtractor {
    fn extract(
        &self,
        conversation: &ValidatedConversation,
    ) -> Result<Vec<MemoryCandidate>, ExtractionError>;
}

/// A deterministic extractor used only by synthetic P7 tests. It produces a
/// provenance-bound candidate from each eligible user turn and never calls a
/// model or reads outside the typed input contract.
pub(super) struct MockMemoryExtractor;

impl MemoryExtractor for MockMemoryExtractor {
    fn extract(
        &self,
        conversation: &ValidatedConversation,
    ) -> Result<Vec<MemoryCandidate>, ExtractionError> {
        let mut candidates = Vec::new();
        for turn in conversation
            .turns
            .iter()
            .filter(|turn| turn.role == TurnRole::User)
        {
            let body = format!("Synthetic note: {}", turn.text.trim());
            if body.trim().is_empty() {
                continue;
            }
            let mut source_ids = vec![turn.turn_id.clone()];
            source_ids.extend(
                turn.source_lineage
                    .iter()
                    .map(|source| source.source_id.clone()),
            );
            candidates.push(MemoryCandidate {
                candidate_key: format!("turn:{}", turn.turn_id),
                body,
                classification: turn.classification,
                source_ids,
                kind: MemoryKind::Episodic,
            });
        }
        Ok(candidates)
    }
}

/// Fixed candidate fixture for boundary and idempotency cases.
pub(super) struct CandidateFixtureExtractor {
    pub candidates: Vec<MemoryCandidate>,
}

impl MemoryExtractor for CandidateFixtureExtractor {
    fn extract(
        &self,
        _conversation: &ValidatedConversation,
    ) -> Result<Vec<MemoryCandidate>, ExtractionError> {
        Ok(self.candidates.clone())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ChatResultWithMemory<T> {
    pub chat_result: T,
    pub memory_result: Result<SaveSummary, ExtractionError>,
}

/// Runs best-effort extraction after a chat result is already available. The
/// original result remains present even when extraction, validation or storage
/// fails, so Memory cannot replace a successful reply with an error.
pub(super) fn preserve_chat_result<T>(
    chat_result: T,
    memory_result: Result<SaveSummary, ExtractionError>,
) -> ChatResultWithMemory<T> {
    ChatResultWithMemory {
        chat_result,
        memory_result,
    }
}

pub(super) fn extract_and_save(
    memory: &P7ExperimentalMemory,
    store: &Store,
    input: &CompletedConversation,
    request_id: &ExtractionRequestId,
    extractor: &impl MemoryExtractor,
    clock: &dyn Clock,
) -> Result<SaveSummary, ExtractionError> {
    let validated = validate_conversation(memory, input)?;
    let admission_time = clock.now_ms().map_err(|_| ExtractionError::StorageFailed)?;
    let admission = ExtractionAdmission::begin(
        memory,
        store,
        request_id,
        conversation_digest(&validated),
        admission_time,
    )?;
    if let Some((summary, _)) = &admission.committed {
        return Ok(*summary);
    }
    let candidates = match extractor.extract(&validated) {
        Ok(candidates) => candidates,
        Err(error) => {
            admission.abort(store)?;
            return Err(error);
        }
    };
    if candidates.len() > crate::p7_gate_c::ACCEPTED_V1_BOUNDS.max_active_items {
        admission.abort(store)?;
        return Err(ExtractionError::CandidateRejected);
    }

    let mut candidate_keys = BTreeSet::new();
    let mut drafts = Vec::with_capacity(candidates.len());
    for candidate in candidates {
        if !candidate_keys.insert(candidate.candidate_key.clone()) {
            admission.abort(store)?;
            return Err(ExtractionError::CandidateRejected);
        }
        match validate_candidate(&validated, candidate, request_id) {
            Ok(draft) => drafts.push(draft),
            Err(error) => {
                admission.abort(store)?;
                return Err(error);
            }
        }
    }
    admission.commit(memory, store, &drafts, clock)
}

/// Test-only persistent lease state. The TTL is deliberately provisional;
/// the accepted 20/day and one concurrent/Owner limits come from Gate C.
struct ExtractionAdmission {
    owner_id: String,
    request_id: String,
    lease_id: i64,
    input_digest: [u8; 32],
    generation: i64,
    committed: Option<(SaveSummary, Vec<u8>)>,
}

enum AdmissionDecision {
    Admitted(ExtractionAdmission),
    Replay(ExtractionAdmission),
    Refused(ExtractionError),
}

impl ExtractionAdmission {
    fn begin(
        memory: &P7ExperimentalMemory,
        store: &Store,
        request_id: &ExtractionRequestId,
        input_digest: [u8; 32],
        now: EpochMillis,
    ) -> Result<Self, ExtractionError> {
        let now_ms = now.get();
        let day = now_ms.div_euclid(86_400_000);
        let decision = store.transact(|tx| {
            ensure_extraction_tables(tx)?;
            super::ensure_ready(tx, &memory.owner_id)?;
            let generation: i64 = tx.inner.query_row(
                "SELECT generation FROM p7_ledger_state WHERE owner_id=?1 AND read_state='READY' AND pending_operation_id IS NULL",
                [&memory.owner_id], |row| row.get(0),
            )?;
            tx.inner.execute(
                "INSERT OR IGNORE INTO p7_extraction_clock(owner_id,last_observed_ms) VALUES(?1,?2)",
                rusqlite::params![memory.owner_id, now_ms],
            )?;
            let last_seen: i64 = tx.inner.query_row(
                "SELECT last_observed_ms FROM p7_extraction_clock WHERE owner_id=?1",
                [&memory.owner_id], |row| row.get(0),
            )?;
            if now_ms < last_seen {
                return Ok(AdmissionDecision::Refused(ExtractionError::ClockRollback));
            }
            tx.inner.execute(
                "UPDATE p7_extraction_clock SET last_observed_ms=?2 WHERE owner_id=?1",
                rusqlite::params![memory.owner_id, now_ms],
            )?;

            let prior: Option<PriorAdmissionRow> = tx.inner.query_row(
                "SELECT input_digest,captured_generation,state,expires_at_ms,candidate_digest,inserted_count,existing_count,lease_id FROM p7_extraction_admissions WHERE owner_id=?1 AND request_id=?2",
                rusqlite::params![memory.owner_id, request_id.0],
                |row| Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?,row.get(4)?,row.get(5)?,row.get(6)?,row.get(7)?)),
            ).optional()?;
            if let Some((prior_digest, prior_generation, state, expires, candidate_digest, inserted, existing, prior_lease_id)) = prior {
                if prior_digest.as_slice() != input_digest {
                    return Ok(AdmissionDecision::Refused(ExtractionError::RequestConflict));
                }
                if prior_generation != generation || state == "FENCED" {
                    tx.inner.execute("UPDATE p7_extraction_admissions SET state='FENCED' WHERE owner_id=?1 AND request_id=?2 AND state<>'COMMITTED'", rusqlite::params![memory.owner_id, request_id.0])?;
                    update_active_count(tx, &memory.owner_id)?;
                    return Ok(AdmissionDecision::Refused(ExtractionError::StaleGeneration));
                }
                if state == "COMMITTED" {
                    let Some(candidate_digest) = candidate_digest.filter(|digest| digest.len() == 32)
                    else {
                        return Ok(AdmissionDecision::Refused(ExtractionError::StorageFailed));
                    };
                    let lease = ExtractionAdmission {
                        owner_id: memory.owner_id.clone(), request_id: request_id.0.clone(), lease_id: prior_lease_id, input_digest,
                        generation, committed: Some((SaveSummary { inserted: inserted as usize, already_present: existing as usize }, candidate_digest)),
                    };
                    return Ok(AdmissionDecision::Replay(lease));
                }
                if state == "ACTIVE" && expires > now_ms {
                    return Ok(AdmissionDecision::Refused(ExtractionError::ConcurrentExtraction));
                }
                if state != "ACTIVE" && state != "EXPIRED" && state != "FAILED" {
                    return Ok(AdmissionDecision::Refused(ExtractionError::StaleGeneration));
                }
                tx.inner.execute("UPDATE p7_extraction_admissions SET state='EXPIRED' WHERE owner_id=?1 AND request_id=?2 AND state='ACTIVE'", rusqlite::params![memory.owner_id, request_id.0])?;
            }

            tx.inner.execute("UPDATE p7_extraction_admissions SET state='FENCED' WHERE owner_id=?1 AND state='ACTIVE' AND captured_generation<>?2", rusqlite::params![memory.owner_id, generation])?;
            tx.inner.execute("UPDATE p7_extraction_admissions SET state='EXPIRED' WHERE owner_id=?1 AND state='ACTIVE' AND expires_at_ms<=?2", rusqlite::params![memory.owner_id, now_ms])?;
            let active: i64 = tx.inner.query_row("SELECT count(*) FROM p7_extraction_admissions WHERE owner_id=?1 AND state='ACTIVE'", [&memory.owner_id], |row| row.get(0))?;
            if active >= crate::p7_gate_c::ACCEPTED_V1_BOUNDS.concurrent_extractions_per_owner as i64 {
                update_active_count(tx, &memory.owner_id)?;
                return Ok(AdmissionDecision::Refused(ExtractionError::ConcurrentExtraction));
            }
            let (stored_day, starts): (String, i64) = tx.inner.query_row("SELECT utc_day,extraction_starts_today FROM p7_resource_accounting WHERE owner_id=?1", [&memory.owner_id], |row| Ok((row.get(0)?,row.get(1)?)))?;
            let previous_day = if stored_day.is_empty() { None } else { Some(stored_day.parse::<i64>().map_err(|_| StoreError::ConstraintViolation)?) };
            if previous_day.is_some_and(|previous| day < previous) {
                return Ok(AdmissionDecision::Refused(ExtractionError::ClockRollback));
            }
            let starts_today = if previous_day == Some(day) { starts } else { 0 };
            if starts_today >= crate::p7_gate_c::ACCEPTED_V1_BOUNDS.extraction_starts_per_utc_day as i64 {
                update_active_count(tx, &memory.owner_id)?;
                return Ok(AdmissionDecision::Refused(ExtractionError::DailyLimitReached));
            }
            let expires = now_ms.checked_add(TEST_EXTRACTION_LEASE_TTL_MS).ok_or(StoreError::Sqlite)?;
            let lease_id: i64 = tx.inner.query_row(
                "SELECT next_lease_id FROM p7_extraction_clock WHERE owner_id=?1",
                [&memory.owner_id], |row| row.get(0),
            )?;
            let next_lease_id = lease_id.checked_add(1).ok_or(StoreError::Sqlite)?;
            tx.inner.execute(
                "UPDATE p7_extraction_clock SET next_lease_id=?2 WHERE owner_id=?1",
                rusqlite::params![memory.owner_id, next_lease_id],
            )?;
            let changed = tx.inner.execute(
                "INSERT INTO p7_extraction_admissions(owner_id,request_id,input_digest,captured_generation,admitted_at_ms,expires_at_ms,utc_day,state,lease_id) VALUES(?1,?2,?3,?4,?5,?6,?7,'ACTIVE',?8) ON CONFLICT(owner_id,request_id) DO UPDATE SET admitted_at_ms=excluded.admitted_at_ms,expires_at_ms=excluded.expires_at_ms,utc_day=excluded.utc_day,state='ACTIVE',candidate_digest=NULL,inserted_count=0,existing_count=0,lease_id=excluded.lease_id WHERE input_digest=excluded.input_digest AND captured_generation=excluded.captured_generation AND state IN ('EXPIRED','FAILED')",
                rusqlite::params![memory.owner_id, request_id.0, input_digest.as_slice(), generation, now_ms, expires, day, lease_id],
            )?;
            if changed != 1 { return Ok(AdmissionDecision::Refused(ExtractionError::RequestConflict)); }
            tx.inner.execute("UPDATE p7_resource_accounting SET extraction_starts_today=?2,utc_day=?3 WHERE owner_id=?1", rusqlite::params![memory.owner_id, starts_today + 1, day.to_string()])?;
            update_active_count(tx, &memory.owner_id)?;
            Ok(AdmissionDecision::Admitted(ExtractionAdmission { owner_id: memory.owner_id.clone(), request_id: request_id.0.clone(), lease_id, input_digest, generation, committed: None }))
        }).map_err(map_storage_error)?;
        match decision {
            AdmissionDecision::Admitted(lease) | AdmissionDecision::Replay(lease) => Ok(lease),
            AdmissionDecision::Refused(error) => Err(error),
        }
    }

    fn abort(&self, store: &Store) -> Result<(), ExtractionError> {
        store.transact(|tx| {
            tx.inner.execute("UPDATE p7_extraction_admissions SET state='FAILED' WHERE owner_id=?1 AND request_id=?2 AND lease_id=?3 AND state='ACTIVE' AND captured_generation=?4", rusqlite::params![self.owner_id, self.request_id, self.lease_id, self.generation])?;
            update_active_count(tx, &self.owner_id)
        }).map_err(map_storage_error)
    }

    fn commit(
        &self,
        memory: &P7ExperimentalMemory,
        store: &Store,
        drafts: &[MemoryDraft],
        clock: &dyn Clock,
    ) -> Result<SaveSummary, ExtractionError> {
        let digest = drafts_digest(drafts);
        if let Some((summary, prior_digest)) = &self.committed {
            return if prior_digest.as_slice() == digest {
                Ok(*summary)
            } else {
                Err(ExtractionError::RequestConflict)
            };
        }
        let result = store.transact(|tx| {
            super::ensure_ready(tx, &self.owner_id)?;
            // Sample after BEGIN IMMEDIATE has acquired the writer transaction,
            // so time spent waiting for another writer cannot stale this check.
            let now = match clock.now_ms() {
                Ok(now) => now,
                Err(_) => return Ok(Err(ExtractionError::StorageFailed)),
            };
            let now_ms = now.get();
            let last_observed: i64 = tx.inner.query_row(
                "SELECT last_observed_ms FROM p7_extraction_clock WHERE owner_id=?1",
                [&self.owner_id], |row| row.get(0),
            )?;
            if now_ms < last_observed {
                return Ok(Err(ExtractionError::ClockRollback));
            }
            tx.inner.execute(
                "UPDATE p7_extraction_clock SET last_observed_ms=?2 WHERE owner_id=?1",
                rusqlite::params![self.owner_id, now_ms],
            )?;
            let generation: i64 = tx.inner.query_row("SELECT generation FROM p7_ledger_state WHERE owner_id=?1 AND read_state='READY' AND pending_operation_id IS NULL", [&self.owner_id], |row| row.get(0))?;
            let operation: Option<(Vec<u8>, i64, i64, String, i64)> = tx.inner.query_row("SELECT input_digest,captured_generation,expires_at_ms,state,lease_id FROM p7_extraction_admissions WHERE owner_id=?1 AND request_id=?2", rusqlite::params![self.owner_id,self.request_id], |row| Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?,row.get(4)?))).optional()?;
            let (input_digest, captured, expires, state, lease_id) = match operation {
                Some(operation) => operation,
                None => return Ok(Err(ExtractionError::StaleLease)),
            };
            if input_digest.as_slice() != self.input_digest || captured != self.generation || generation != self.generation || lease_id != self.lease_id || state != "ACTIVE" { return Ok(Err(ExtractionError::StaleLease)); }
            if now_ms >= expires { return Ok(Err(ExtractionError::LeaseExpired)); }
            let mut inserted = 0;
            let mut existing = 0;
            for (index, draft) in drafts.iter().enumerate() {
                if memory.insert_once_in_tx(tx, draft, now)? { inserted += 1; } else { existing += 1; }
                if index == 0 && std::env::var_os("SEREA_P7_CRASH_DURING_EXTRACTION_COMMIT").is_some() {
                    std::process::exit(74);
                }
            }
            let digest = drafts_digest(drafts);
            let changed = tx.inner.execute("UPDATE p7_extraction_admissions SET state='COMMITTED',candidate_digest=?5,inserted_count=?6,existing_count=?7 WHERE owner_id=?1 AND request_id=?2 AND lease_id=?3 AND state='ACTIVE' AND captured_generation=?4 AND expires_at_ms>?8", rusqlite::params![self.owner_id,self.request_id,self.lease_id,self.generation,digest.as_slice(),inserted as i64,existing as i64,now_ms])?;
            if changed != 1 { return Ok(Err(ExtractionError::StaleLease)); }
            update_active_count(tx, &self.owner_id)?;
            Ok(Ok(SaveSummary { inserted: inserted as usize, already_present: existing as usize }))
        });
        match result {
            Ok(Ok(summary)) => Ok(summary),
            Ok(Err(error)) => {
                let _ = self.abort(store);
                Err(error)
            }
            Err(error) => {
                // The candidate transaction has rolled back. Release the
                // durable admission on ordinary errors; process death skips
                // this path and is recovered by expiry on the next admission.
                let _ = self.abort(store);
                Err(map_storage_error(error))
            }
        }
    }
}

fn ensure_extraction_tables(tx: &mut crate::Tx<'_>) -> Result<(), StoreError> {
    tx.inner.execute_batch("CREATE TABLE IF NOT EXISTS p7_extraction_clock(owner_id TEXT PRIMARY KEY REFERENCES p7_owners(owner_id),last_observed_ms INTEGER NOT NULL,next_lease_id INTEGER NOT NULL DEFAULT 1 CHECK(next_lease_id>0)) STRICT; CREATE TABLE IF NOT EXISTS p7_extraction_admissions(owner_id TEXT NOT NULL REFERENCES p7_owners(owner_id),request_id TEXT NOT NULL,input_digest BLOB NOT NULL CHECK(length(input_digest)=32),captured_generation INTEGER NOT NULL,admitted_at_ms INTEGER NOT NULL,expires_at_ms INTEGER NOT NULL,utc_day INTEGER NOT NULL,state TEXT NOT NULL CHECK(state IN ('ACTIVE','COMMITTED','EXPIRED','FAILED','FENCED')),candidate_digest BLOB,inserted_count INTEGER NOT NULL DEFAULT 0,existing_count INTEGER NOT NULL DEFAULT 0,lease_id INTEGER NOT NULL CHECK(lease_id>0),PRIMARY KEY(owner_id,request_id)) STRICT;")?;
    Ok(())
}

fn update_active_count(tx: &mut crate::Tx<'_>, owner_id: &str) -> Result<(), StoreError> {
    tx.inner.execute("UPDATE p7_resource_accounting SET concurrent_extractions=(SELECT count(*) FROM p7_extraction_admissions WHERE owner_id=?1 AND state='ACTIVE') WHERE owner_id=?1", [owner_id])?;
    Ok(())
}

fn hash_field(hasher: &mut Sha256, value: &[u8]) {
    hasher.update((value.len() as u64).to_be_bytes());
    hasher.update(value);
}

fn conversation_digest(conversation: &ValidatedConversation) -> [u8; 32] {
    let mut digest = Sha256::new();
    hash_field(&mut digest, conversation.owner_id.as_bytes());
    hash_field(&mut digest, conversation.conversation_id.as_bytes());
    for turn in &conversation.turns {
        hash_field(&mut digest, turn.turn_id.as_bytes());
        hash_field(&mut digest, turn.text.as_bytes());
        hash_field(&mut digest, class_code(turn.classification).as_bytes());
        hash_field(&mut digest, origin_code(turn.origin).as_bytes());
        hash_field(
            &mut digest,
            &[match turn.role {
                TurnRole::User => 1,
                TurnRole::Assistant => 2,
            }],
        );
        for source in &turn.source_lineage {
            hash_field(&mut digest, source.source_id.as_bytes());
            hash_field(&mut digest, class_code(source.classification).as_bytes());
            hash_field(&mut digest, origin_code(source.origin).as_bytes());
        }
    }
    digest.finalize().into()
}

fn drafts_digest(drafts: &[MemoryDraft]) -> [u8; 32] {
    let mut ordered: Vec<_> = drafts.iter().collect();
    ordered.sort_by(|left, right| left.memory_id.cmp(&right.memory_id));
    let mut digest = Sha256::new();
    for draft in ordered {
        hash_field(&mut digest, draft.memory_id.as_bytes());
        hash_field(&mut digest, draft.body.as_bytes());
        hash_field(&mut digest, draft.provenance.source_id.as_bytes());
        hash_field(&mut digest, draft.kind.name().as_bytes());
        hash_field(
            &mut digest,
            class_code(match draft.class {
                MemoryClass::Public => DataClass::Public,
                MemoryClass::Personal => DataClass::Personal,
            })
            .as_bytes(),
        );
        hash_field(
            &mut digest,
            class_code(match draft.provenance.source_class {
                MemoryClass::Public => DataClass::Public,
                MemoryClass::Personal => DataClass::Personal,
            })
            .as_bytes(),
        );
    }
    digest.finalize().into()
}

fn validate_conversation(
    memory: &P7ExperimentalMemory,
    input: &CompletedConversation,
) -> Result<ValidatedConversation, ExtractionError> {
    if input.state != ConversationState::Complete {
        return Err(ExtractionError::ConversationNotComplete);
    }
    let conversation_id = input
        .conversation_id
        .as_deref()
        .filter(|id| !id.is_empty() && id.len() <= 128)
        .ok_or(ExtractionError::MissingConversationId)?;
    crate::p7_gate_c::OwnerId::parse(input.owner_id.clone())
        .map_err(|_| ExtractionError::OwnerMismatch)?;
    if memory.owner_id != input.owner_id {
        return Err(ExtractionError::OwnerMismatch);
    }

    let mut total_bytes = 0usize;
    let mut turn_ids = BTreeSet::new();
    let mut accepted = Vec::new();
    for turn in &input.turns {
        if turn.state != TurnState::Complete
            || turn.conversation_id.as_deref() != Some(conversation_id)
        {
            continue;
        }
        if turn.turn_id.is_empty()
            || turn.turn_id.len() > 128
            || !turn_ids.insert(turn.turn_id.clone())
        {
            return Err(ExtractionError::InvalidSourceLineage);
        }
        total_bytes = total_bytes
            .checked_add(turn.text.len())
            .ok_or(ExtractionError::InputTooLarge)?;
        if total_bytes > SYNTHETIC_INPUT_MAX_BYTES {
            return Err(ExtractionError::InputTooLarge);
        }

        // A turn or any of its lineage above PERSONAL is omitted before the
        // extractor sees it. Unclassified/unknown values therefore fail closed.
        let Some(_) = turn.classification.memory_class() else {
            continue;
        };
        if turn.source_lineage.is_empty() {
            continue;
        }
        let mut valid_lineage = Vec::with_capacity(turn.source_lineage.len());
        let mut source_ids = BTreeSet::new();
        let mut all_storable = true;
        let mut inherited_class = turn.classification;
        for source in &turn.source_lineage {
            if source.source_id.is_empty()
                || source.source_id.len() > 256
                || !source_ids.insert(source.source_id.clone())
            {
                return Err(ExtractionError::InvalidSourceLineage);
            }
            if source.classification.memory_class().is_none() {
                all_storable = false;
                break;
            }
            inherited_class = inherited_class.max(source.classification);
            valid_lineage.push(source.clone());
        }
        if all_storable {
            let mut safe_turn = turn.clone();
            safe_turn.classification = inherited_class;
            safe_turn.source_lineage = valid_lineage;
            accepted.push(safe_turn);
        }
    }
    if accepted.is_empty() {
        return Err(ExtractionError::NoEligibleTurns);
    }
    Ok(ValidatedConversation {
        owner_id: input.owner_id.clone(),
        conversation_id: conversation_id.to_owned(),
        turns: accepted,
    })
}

fn validate_candidate(
    conversation: &ValidatedConversation,
    candidate: MemoryCandidate,
    request_id: &ExtractionRequestId,
) -> Result<MemoryDraft, ExtractionError> {
    let class = candidate
        .classification
        .memory_class()
        .ok_or(ExtractionError::CandidateRejected)?;
    let bounds = crate::p7_gate_c::ACCEPTED_V1_BOUNDS;
    if candidate.candidate_key.is_empty()
        || candidate.candidate_key.len() > 128
        || candidate.body.trim().is_empty()
        || candidate.body.len() > bounds.max_item_body_utf8_bytes
        || candidate.body.len() > SYNTHETIC_CANDIDATE_MAX_BYTES
        || candidate.source_ids.is_empty()
    {
        return Err(ExtractionError::CandidateRejected);
    }

    let mut available = BTreeMap::<String, (DataClass, DataOrigin)>::new();
    for turn in &conversation.turns {
        for source in &turn.source_lineage {
            if let Some(prior) = available.get(&source.source_id) {
                if prior.1 != source.origin {
                    return Err(ExtractionError::CandidateRejected);
                }
                available.insert(
                    source.source_id.clone(),
                    (prior.0.max(source.classification), source.origin),
                );
            } else {
                available.insert(
                    source.source_id.clone(),
                    (source.classification, source.origin),
                );
            }
        }
        if available
            .insert(turn.turn_id.clone(), (turn.classification, turn.origin))
            .is_some()
        {
            return Err(ExtractionError::CandidateRejected);
        }
    }

    let mut unique_refs = BTreeSet::new();
    // A generic extractor may synthesize from any eligible input turn, so a
    // candidate inherits the highest class of the full sanitized conversation,
    // even if its citations name a narrower subset.
    let mut source_class = conversation
        .turns
        .iter()
        .map(|turn| turn.classification)
        .max()
        .unwrap_or(DataClass::Public);
    let mut cited = Vec::with_capacity(candidate.source_ids.len());
    for source_id in &candidate.source_ids {
        if !unique_refs.insert(source_id.clone()) {
            return Err(ExtractionError::CandidateRejected);
        }
        let (cited_class, cited_origin) = available
            .get(source_id)
            .copied()
            .ok_or(ExtractionError::CandidateRejected)?;
        source_class = source_class.max(cited_class);
        cited.push((source_id, cited_class, cited_origin));
    }
    if source_class > candidate.classification {
        return Err(ExtractionError::CandidateRejected);
    }
    let source_memory_class = source_class
        .memory_class()
        .ok_or(ExtractionError::CandidateRejected)?;

    // Pack the bounded conversation + turn/source references into the existing
    // opaque provenance source_id field. No experimental schema/migration is
    // added; this retains source lineage alongside the candidate.
    let mut lineage = format!(
        "p7-conversation-v1|{}:{}",
        conversation.conversation_id.len(),
        conversation.conversation_id
    );
    for (source_id, source_class, source_origin) in cited {
        lineage.push_str(&format!(
            "|{}:{}:{}:{}",
            source_id.len(),
            source_id,
            class_code(source_class),
            origin_code(source_origin),
        ));
    }
    if lineage.len() > SYNTHETIC_LINEAGE_MAX_BYTES {
        return Err(ExtractionError::CandidateRejected);
    }

    let memory_id = deterministic_memory_id(
        &conversation.owner_id,
        &conversation.conversation_id,
        &candidate.candidate_key,
    );
    let idempotency_key = format!("{}:{memory_id}", request_id.0);
    Ok(MemoryDraft {
        memory_id,
        class,
        kind: candidate.kind,
        body: candidate.body,
        provenance: MemoryProvenance {
            source_id: lineage.clone(),
            source_task_id: Some(idempotency_key),
            source_class: source_memory_class,
            metadata_bytes: lineage.len(),
        },
    })
}

fn class_code(classification: DataClass) -> &'static str {
    match classification {
        DataClass::Public => "PUBLIC",
        DataClass::Personal => "PERSONAL",
        DataClass::Private => "PRIVATE",
        DataClass::Secret => "SECRET",
        DataClass::Credential => "CREDENTIAL",
        DataClass::Unknown => "UNKNOWN",
    }
}

fn origin_code(origin: DataOrigin) -> &'static str {
    match origin {
        DataOrigin::UserAuthored => "USER_AUTHORED",
        DataOrigin::ModelOutput => "MODEL_OUTPUT",
        DataOrigin::Provider => "PROVIDER",
        DataOrigin::Other => "OTHER",
    }
}

fn deterministic_memory_id(owner_id: &str, conversation_id: &str, candidate_key: &str) -> String {
    let mut hash = Sha256::new();
    hash.update(owner_id.as_bytes());
    hash.update([0]);
    hash.update(conversation_id.as_bytes());
    hash.update([0]);
    hash.update(candidate_key.as_bytes());
    let mut encoded = String::with_capacity(68);
    encoded.push_str("mem-");
    for byte in hash.finalize() {
        use std::fmt::Write;
        write!(encoded, "{byte:02x}").expect("writing to String cannot fail");
    }
    encoded
}

fn map_storage_error(_error: StoreError) -> ExtractionError {
    ExtractionError::StorageFailed
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Store, experimental_memory::P7ExperimentalMemory};
    use rusqlite::params;
    use serea_protocol::{Clock, ProtocolError};
    use std::path::Path;
    use std::process::{Command, Stdio};
    use std::sync::atomic::{AtomicI64, AtomicU64, Ordering};
    use std::sync::{Arc, Barrier};

    const OWNER: &str = "own_00000000000000000000000000000071";
    const OTHER_OWNER: &str = "own_00000000000000000000000000000072";
    const NOW_MS: i64 = 1_800_000_000_000;
    static NEXT_CRASH_DB: AtomicU64 = AtomicU64::new(1);

    struct FixedClock;
    impl Clock for FixedClock {
        fn now_ms(&self) -> Result<EpochMillis, ProtocolError> {
            EpochMillis::new(NOW_MS)
        }
    }

    struct MutableClock(AtomicI64);
    impl MutableClock {
        fn new(now_ms: i64) -> Self {
            Self(AtomicI64::new(now_ms))
        }

        fn set(&self, now_ms: i64) {
            self.0.store(now_ms, Ordering::SeqCst);
        }
    }
    impl Clock for MutableClock {
        fn now_ms(&self) -> Result<EpochMillis, ProtocolError> {
            EpochMillis::new(self.0.load(Ordering::SeqCst))
        }
    }

    fn setup() -> (Store, P7ExperimentalMemory) {
        let store = Store::open_in_memory(&FixedClock).unwrap();
        store
            .transact(|tx| {
                tx.inner.execute(
                    "INSERT INTO p7_owners VALUES(?1,71,'ACTIVE',?2)",
                    params![OWNER, NOW_MS],
                )?;
                tx.inner.execute(
                    "INSERT INTO p7_resource_accounting(owner_id) VALUES(?1)",
                    [OWNER],
                )?;
                tx.inner.execute(
                    "INSERT INTO p7_ledger_state(owner_id,read_state) VALUES(?1,'READY')",
                    [OWNER],
                )?;
                Ok(())
            })
            .unwrap();
        (store, P7ExperimentalMemory::fake(OWNER.to_owned(), 71))
    }

    fn seed_file_store(path: &Path) {
        let store = Store::open(path, &FixedClock).unwrap();
        store
            .transact(|tx| {
                tx.inner.execute(
                    "INSERT INTO p7_owners VALUES(?1,71,'ACTIVE',?2)",
                    params![OWNER, NOW_MS],
                )?;
                tx.inner.execute(
                    "INSERT INTO p7_resource_accounting(owner_id) VALUES(?1)",
                    [OWNER],
                )?;
                tx.inner.execute(
                    "INSERT INTO p7_ledger_state(owner_id,read_state) VALUES(?1,'READY')",
                    [OWNER],
                )?;
                Ok(())
            })
            .unwrap();
    }

    fn crash_input_and_candidates() -> (CompletedConversation, Vec<MemoryCandidate>) {
        let input = conversation(vec![turn(
            "crash-turn",
            Some("conversation-c05"),
            TurnState::Complete,
            DataClass::Public,
            vec![source(
                "crash-source",
                DataClass::Public,
                DataOrigin::UserAuthored,
            )],
        )]);
        let first = MemoryCandidate {
            candidate_key: "crash-one".into(),
            body: "synthetic crash first".into(),
            classification: DataClass::Public,
            source_ids: vec!["crash-turn".into(), "crash-source".into()],
            kind: MemoryKind::Episodic,
        };
        let second = MemoryCandidate {
            candidate_key: "crash-two".into(),
            body: "synthetic crash second".into(),
            classification: DataClass::Public,
            source_ids: vec!["crash-turn".into(), "crash-source".into()],
            kind: MemoryKind::Episodic,
        };
        (input, vec![first, second])
    }

    #[test]
    fn extraction_process_child_entry() {
        let Ok(path) = std::env::var("SEREA_P7_C06_CRASH_DB") else {
            return;
        };
        let store = Store::open(Path::new(&path), &FixedClock).unwrap();
        let memory = P7ExperimentalMemory::fake(OWNER.to_owned(), 71);
        let (input, candidates) = crash_input_and_candidates();
        let _ = extract_and_save(
            &memory,
            &store,
            &input,
            &ExtractionRequestId::new("process-crash-request").unwrap(),
            &CandidateFixtureExtractor { candidates },
            &FixedClock,
        );
        panic!("the crash seam should exit while the final SQLite transaction is open");
    }

    #[test]
    fn process_death_mid_batch_recovers_by_expiry_without_partial_memory_rows() {
        let dir = std::env::temp_dir().join(format!(
            "serea-c06-extraction-{}-{}",
            std::process::id(),
            NEXT_CRASH_DB.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("synthetic.sqlite");
        seed_file_store(&path);
        let status = Command::new(std::env::current_exe().unwrap())
            .arg("--exact")
            .arg("experimental_memory::p7_conversation_extraction::tests::extraction_process_child_entry")
            .arg("--nocapture")
            .env("SEREA_P7_C06_CRASH_DB", &path)
            .env("SEREA_P7_CRASH_DURING_EXTRACTION_COMMIT", "1")
            .stdout(Stdio::null()).stderr(Stdio::null())
            .status().unwrap();
        assert_eq!(status.code(), Some(74));

        let store = Store::open(&path, &FixedClock).unwrap();
        let memory = P7ExperimentalMemory::fake(OWNER.to_owned(), 71);
        let (rows, starts, active, operation_state): (i64, i64, i64, String) = store.transact(|tx| Ok((
            tx.inner.query_row("SELECT count(*) FROM p7_memory_items WHERE owner_id=?1", [OWNER], |row| row.get(0))?,
            tx.inner.query_row("SELECT extraction_starts_today FROM p7_resource_accounting WHERE owner_id=?1", [OWNER], |row| row.get(0))?,
            tx.inner.query_row("SELECT concurrent_extractions FROM p7_resource_accounting WHERE owner_id=?1", [OWNER], |row| row.get(0))?,
            tx.inner.query_row("SELECT state FROM p7_extraction_admissions WHERE owner_id=?1 AND request_id='process-crash-request'", [OWNER], |row| row.get(0))?,
        ))).unwrap();
        assert_eq!(
            (rows, starts, active, operation_state.as_str()),
            (0, 1, 1, "ACTIVE")
        );

        let (input, candidates) = crash_input_and_candidates();
        let recovered = extract_and_save(
            &memory,
            &store,
            &input,
            &ExtractionRequestId::new("process-crash-request").unwrap(),
            &CandidateFixtureExtractor { candidates },
            &MutableClock::new(NOW_MS + TEST_EXTRACTION_LEASE_TTL_MS + 1),
        )
        .unwrap();
        assert_eq!(
            recovered,
            SaveSummary {
                inserted: 2,
                already_present: 0
            }
        );
        let rows: i64 = store
            .transact(|tx| {
                Ok(tx.inner.query_row(
                    "SELECT count(*) FROM p7_memory_items WHERE owner_id=?1",
                    [OWNER],
                    |row| row.get(0),
                )?)
            })
            .unwrap();
        assert_eq!(rows, 2);
        drop(store);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn independent_file_connections_admit_only_one_active_owner_extraction() {
        let dir = std::env::temp_dir().join(format!(
            "serea-c06-admission-race-{}-{}",
            std::process::id(),
            NEXT_CRASH_DB.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("synthetic.sqlite");
        seed_file_store(&path);
        let path = Arc::new(path);
        let barrier = Arc::new(Barrier::new(2));
        let workers = (0..2)
            .map(|index| {
                let path = Arc::clone(&path);
                let barrier = Arc::clone(&barrier);
                std::thread::spawn(move || {
                    let store = Store::open(path.as_path(), &FixedClock).unwrap();
                    let memory = P7ExperimentalMemory::fake(OWNER.to_owned(), 71);
                    let request = ExtractionRequestId::new(format!("parallel-{index}")).unwrap();
                    barrier.wait();
                    ExtractionAdmission::begin(
                        &memory,
                        &store,
                        &request,
                        [3u8; 32],
                        EpochMillis::new(NOW_MS).unwrap(),
                    )
                    .is_ok()
                })
            })
            .collect::<Vec<_>>();
        let admitted = workers
            .into_iter()
            .map(|worker| worker.join().unwrap())
            .filter(|admitted| *admitted)
            .count();
        assert_eq!(admitted, 1);
        let store = Store::open(path.as_path(), &FixedClock).unwrap();
        let counts: (i64, i64) = store.transact(|tx| Ok((
            tx.inner.query_row("SELECT concurrent_extractions FROM p7_resource_accounting WHERE owner_id=?1", [OWNER], |row| row.get(0))?,
            tx.inner.query_row("SELECT extraction_starts_today FROM p7_resource_accounting WHERE owner_id=?1", [OWNER], |row| row.get(0))?,
        ))).unwrap();
        assert_eq!(counts, (1, 1));
        drop(store);
        std::fs::remove_dir_all(dir).unwrap();
    }

    fn source(id: &str, class: DataClass, origin: DataOrigin) -> SourceLineage {
        SourceLineage {
            source_id: id.into(),
            origin,
            classification: class,
        }
    }

    fn turn(
        id: &str,
        conversation_id: Option<&str>,
        state: TurnState,
        class: DataClass,
        source_lineage: Vec<SourceLineage>,
    ) -> ConversationTurn {
        ConversationTurn {
            turn_id: id.into(),
            conversation_id: conversation_id.map(str::to_owned),
            role: TurnRole::User,
            state,
            text: format!("synthetic turn text {id}"),
            classification: class,
            origin: DataOrigin::UserAuthored,
            source_lineage,
        }
    }

    fn conversation(turns: Vec<ConversationTurn>) -> CompletedConversation {
        CompletedConversation {
            owner_id: OWNER.into(),
            conversation_id: Some("conversation-c05".into()),
            state: ConversationState::Complete,
            turns,
        }
    }

    fn candidate(classification: DataClass, refs: &[&str]) -> MemoryCandidate {
        MemoryCandidate {
            candidate_key: "stable-preference".into(),
            body: "Synthetic preference: concise answers".into(),
            classification,
            source_ids: refs.iter().map(|value| (*value).into()).collect(),
            kind: MemoryKind::Preference,
        }
    }

    fn extract(
        store: &Store,
        memory: &P7ExperimentalMemory,
        input: &CompletedConversation,
        request_id: &str,
        candidates: Vec<MemoryCandidate>,
    ) -> Result<SaveSummary, ExtractionError> {
        extract_and_save(
            memory,
            store,
            input,
            &ExtractionRequestId::new(request_id).unwrap(),
            &CandidateFixtureExtractor { candidates },
            &FixedClock,
        )
    }

    #[test]
    fn synthetic_completed_conversation_generates_validated_memory_and_fts_result() {
        let (store, memory) = setup();
        let mut input = conversation(vec![turn(
            "turn-1",
            Some("conversation-c05"),
            TurnState::Complete,
            DataClass::Personal,
            vec![source(
                "profile-source",
                DataClass::Personal,
                DataOrigin::Provider,
            )],
        )]);
        input.turns[0].text = "I prefer concise answers".into();
        input.turns.push(ConversationTurn {
            turn_id: "assistant-turn".into(),
            conversation_id: Some("conversation-c05".into()),
            role: TurnRole::Assistant,
            state: TurnState::Complete,
            text: "assistant text is not independently extracted".into(),
            classification: DataClass::Personal,
            origin: DataOrigin::ModelOutput,
            source_lineage: vec![source(
                "assistant-source",
                DataClass::Personal,
                DataOrigin::ModelOutput,
            )],
        });

        let result = extract_and_save(
            &memory,
            &store,
            &input,
            &ExtractionRequestId::new("extract-request-1").unwrap(),
            &MockMemoryExtractor,
            &FixedClock,
        )
        .unwrap();

        assert_eq!(
            result,
            SaveSummary {
                inserted: 1,
                already_present: 0
            }
        );
        let page = memory
            .search(
                &store,
                MemoryClass::Personal,
                "concise",
                EpochMillis::new(NOW_MS + 1).unwrap(),
                None,
                10,
            )
            .unwrap();
        assert_eq!(page.records.len(), 1);
        assert_eq!(
            page.records[0].body,
            "Synthetic note: I prefer concise answers"
        );
        assert_eq!(page.records[0].expires_at_ms, NOW_MS + 90 * 86_400_000);

        let (source_id, source_class, metadata_bytes): (String, i64, i64) = store
            .transact(|tx| {
                Ok(tx.inner.query_row(
                    "SELECT source_id,source_class_rank,metadata_bytes FROM p7_memory_provenance WHERE source_task_id LIKE 'extract-request-1:%'",
                    [],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
                )?)
            })
            .unwrap();
        assert!(source_id.contains("conversation-c05"));
        assert!(source_id.contains("profile-source"));
        assert_eq!(source_class, 1);
        assert_eq!(metadata_bytes, source_id.len() as i64);
    }

    #[test]
    fn retries_and_new_requests_for_same_candidate_do_not_duplicate_rows() {
        let (store, memory) = setup();
        let input = conversation(vec![turn(
            "turn-1",
            Some("conversation-c05"),
            TurnState::Complete,
            DataClass::Public,
            vec![source(
                "public-source",
                DataClass::Public,
                DataOrigin::Other,
            )],
        )]);
        let candidate = candidate(DataClass::Public, &["turn-1", "public-source"]);

        assert_eq!(
            extract(
                &store,
                &memory,
                &input,
                "same-request",
                vec![candidate.clone()]
            )
            .unwrap(),
            SaveSummary {
                inserted: 1,
                already_present: 0
            }
        );
        assert_eq!(
            extract(
                &store,
                &memory,
                &input,
                "same-request",
                vec![candidate.clone()]
            )
            .unwrap(),
            SaveSummary {
                inserted: 1,
                already_present: 0
            }
        );
        assert_eq!(
            extract(&store, &memory, &input, "new-request", vec![candidate]).unwrap(),
            SaveSummary {
                inserted: 0,
                already_present: 1
            }
        );
        let validated = validate_conversation(&memory, &input).unwrap();
        let replay = ExtractionAdmission::begin(
            &memory,
            &store,
            &ExtractionRequestId::new("same-request").unwrap(),
            conversation_digest(&validated),
            EpochMillis::new(NOW_MS).unwrap(),
        )
        .unwrap();
        let changed_draft = validate_candidate(
            &validated,
            MemoryCandidate {
                candidate_key: "stable-preference".into(),
                body: "different content under a committed request".into(),
                classification: DataClass::Public,
                source_ids: vec!["turn-1".into(), "public-source".into()],
                kind: MemoryKind::Preference,
            },
            &ExtractionRequestId::new("same-request").unwrap(),
        )
        .unwrap();
        assert_eq!(
            replay.commit(&memory, &store, &[changed_draft], &FixedClock),
            Err(ExtractionError::RequestConflict)
        );
        let mut changed_input = input.clone();
        changed_input.turns[0].text.push_str(" modified");
        assert_eq!(
            extract(&store, &memory, &changed_input, "same-request", vec![]),
            Err(ExtractionError::RequestConflict)
        );
        let count: i64 = store
            .transact(|tx| {
                Ok(tx
                    .inner
                    .query_row("SELECT count(*) FROM p7_memory_items", [], |row| row.get(0))?)
            })
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn completed_request_replay_does_not_execute_extractor_again() {
        struct CountingExtractor<'a> {
            calls: &'a AtomicU64,
        }
        impl MemoryExtractor for CountingExtractor<'_> {
            fn extract(
                &self,
                _conversation: &ValidatedConversation,
            ) -> Result<Vec<MemoryCandidate>, ExtractionError> {
                self.calls.fetch_add(1, Ordering::SeqCst);
                Ok(vec![candidate(
                    DataClass::Public,
                    &["turn-1", "public-source"],
                )])
            }
        }

        let (store, memory) = setup();
        let input = conversation(vec![turn(
            "turn-1",
            Some("conversation-c05"),
            TurnState::Complete,
            DataClass::Public,
            vec![source(
                "public-source",
                DataClass::Public,
                DataOrigin::Other,
            )],
        )]);
        let calls = AtomicU64::new(0);
        let extractor = CountingExtractor { calls: &calls };
        for _ in 0..4 {
            assert_eq!(
                extract_and_save(
                    &memory,
                    &store,
                    &input,
                    &ExtractionRequestId::new("completed-replay").unwrap(),
                    &extractor,
                    &FixedClock,
                )
                .unwrap(),
                SaveSummary {
                    inserted: 1,
                    already_present: 0
                }
            );
        }
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        let starts: i64 = store
            .transact(|tx| {
                Ok(tx.inner.query_row(
                    "SELECT extraction_starts_today FROM p7_resource_accounting WHERE owner_id=?1",
                    [OWNER],
                    |row| row.get(0),
                )?)
            })
            .unwrap();
        assert_eq!(starts, 1);
    }

    #[test]
    fn a_storage_failure_after_the_first_candidate_rolls_back_the_entire_batch() {
        let (store, memory) = setup();
        let input = conversation(vec![turn(
            "turn-1",
            Some("conversation-c05"),
            TurnState::Complete,
            DataClass::Public,
            vec![source(
                "public-source",
                DataClass::Public,
                DataOrigin::Other,
            )],
        )]);
        let second_id = deterministic_memory_id(OWNER, "conversation-c05", "second");
        store.transact(|tx| {
            tx.inner.execute_batch(&format!(
                "CREATE TRIGGER reject_second_provenance BEFORE INSERT ON p7_memory_provenance WHEN NEW.memory_id='{second_id}' BEGIN SELECT RAISE(ABORT,'synthetic middle write failure'); END;"
            ))?;
            Ok(())
        }).unwrap();
        let first = candidate(DataClass::Public, &["turn-1", "public-source"]);
        let second = MemoryCandidate {
            candidate_key: "second".into(),
            body: "second synthetic fact".into(),
            classification: DataClass::Public,
            source_ids: vec!["turn-1".into(), "public-source".into()],
            kind: MemoryKind::Episodic,
        };
        assert_eq!(
            extract(&store, &memory, &input, "atomic-batch", vec![first, second]),
            Err(ExtractionError::StorageFailed)
        );
        let (items, provenance, fts, active): (i64, i64, i64, i64) = store.transact(|tx| Ok((
            tx.inner.query_row("SELECT count(*) FROM p7_memory_items", [], |row| row.get(0))?,
            tx.inner.query_row("SELECT count(*) FROM p7_memory_provenance", [], |row| row.get(0))?,
            tx.inner.query_row("SELECT count(*) FROM p7_memory_fts", [], |row| row.get(0))?,
            tx.inner.query_row("SELECT concurrent_extractions FROM p7_resource_accounting WHERE owner_id=?1", [OWNER], |row| row.get(0))?,
        ))).unwrap();
        assert_eq!((items, provenance, fts), (0, 0, 0));
        assert_eq!(
            active, 0,
            "ordinary storage failure releases persistent admission"
        );
    }

    struct ForgetDuringExtraction<'a> {
        store: &'a Store,
    }
    impl MemoryExtractor for ForgetDuringExtraction<'_> {
        fn extract(
            &self,
            _conversation: &ValidatedConversation,
        ) -> Result<Vec<MemoryCandidate>, ExtractionError> {
            // Models a completed authenticated Forget between admission and the final save.
            self.store
                .transact(|tx| {
                    tx.inner.execute(
                        "UPDATE p7_ledger_state SET generation=generation+1 WHERE owner_id=?1",
                        [OWNER],
                    )?;
                    Ok(())
                })
                .unwrap();
            Ok(vec![candidate(
                DataClass::Public,
                &["turn-1", "public-source"],
            )])
        }
    }

    #[test]
    fn completed_forget_generation_fences_an_already_running_extraction() {
        let (store, memory) = setup();
        let input = conversation(vec![turn(
            "turn-1",
            Some("conversation-c05"),
            TurnState::Complete,
            DataClass::Public,
            vec![source(
                "public-source",
                DataClass::Public,
                DataOrigin::Other,
            )],
        )]);
        assert_eq!(
            extract_and_save(
                &memory,
                &store,
                &input,
                &ExtractionRequestId::new("forget-race").unwrap(),
                &ForgetDuringExtraction { store: &store },
                &FixedClock,
            ),
            Err(ExtractionError::StaleLease)
        );
        let count: i64 = store
            .transact(|tx| {
                Ok(tx
                    .inner
                    .query_row("SELECT count(*) FROM p7_memory_items", [], |row| row.get(0))?)
            })
            .unwrap();
        assert_eq!(count, 0);
        let state: (i64, i64, String) = store.transact(|tx| Ok(tx.inner.query_row(
            "SELECT l.generation,r.concurrent_extractions,a.state FROM p7_ledger_state l JOIN p7_resource_accounting r USING(owner_id) JOIN p7_extraction_admissions a USING(owner_id) WHERE l.owner_id=?1",
            [OWNER], |row| Ok((row.get(0)?,row.get(1)?,row.get(2)?)),
        )?)).unwrap();
        assert_eq!(state, (1, 0, "FAILED".into()));
    }

    #[test]
    fn failed_incomplete_unowned_and_other_conversation_turns_never_reach_extractor() {
        let (store, memory) = setup();
        let input = conversation(vec![
            turn(
                "eligible-turn",
                Some("conversation-c05"),
                TurnState::Complete,
                DataClass::Public,
                vec![source(
                    "eligible-source",
                    DataClass::Public,
                    DataOrigin::UserAuthored,
                )],
            ),
            turn(
                "pending-turn",
                Some("conversation-c05"),
                TurnState::InProgress,
                DataClass::Personal,
                vec![source(
                    "pending-source",
                    DataClass::Personal,
                    DataOrigin::UserAuthored,
                )],
            ),
            turn(
                "failed-turn",
                Some("conversation-c05"),
                TurnState::Failed,
                DataClass::Personal,
                vec![source(
                    "failed-source",
                    DataClass::Personal,
                    DataOrigin::UserAuthored,
                )],
            ),
            turn(
                "unowned-turn",
                None,
                TurnState::Complete,
                DataClass::Personal,
                vec![source(
                    "unowned-source",
                    DataClass::Personal,
                    DataOrigin::UserAuthored,
                )],
            ),
            turn(
                "other-turn",
                Some("other-conversation"),
                TurnState::Complete,
                DataClass::Personal,
                vec![source(
                    "other-source",
                    DataClass::Personal,
                    DataOrigin::UserAuthored,
                )],
            ),
        ]);
        struct InspectExtractor;
        impl MemoryExtractor for InspectExtractor {
            fn extract(
                &self,
                input: &ValidatedConversation,
            ) -> Result<Vec<MemoryCandidate>, ExtractionError> {
                assert_eq!(
                    input
                        .turns
                        .iter()
                        .map(|turn| turn.turn_id.as_str())
                        .collect::<Vec<_>>(),
                    ["eligible-turn"]
                );
                Ok(vec![candidate(
                    DataClass::Public,
                    &["eligible-turn", "eligible-source"],
                )])
            }
        }

        let result = extract_and_save(
            &memory,
            &store,
            &input,
            &ExtractionRequestId::new("filtered-request").unwrap(),
            &InspectExtractor,
            &FixedClock,
        )
        .unwrap();
        assert_eq!(result.inserted, 1);

        let mut failed = input.clone();
        failed.state = ConversationState::Failed;
        assert_eq!(
            extract(&store, &memory, &failed, "failed-conversation", vec![]),
            Err(ExtractionError::ConversationNotComplete)
        );
        let mut in_progress = input.clone();
        in_progress.state = ConversationState::InProgress;
        assert_eq!(
            extract(
                &store,
                &memory,
                &in_progress,
                "in-progress-conversation",
                vec![]
            ),
            Err(ExtractionError::ConversationNotComplete)
        );
        let mut unowned = input;
        unowned.conversation_id = None;
        assert_eq!(
            extract(&store, &memory, &unowned, "unowned-conversation", vec![]),
            Err(ExtractionError::MissingConversationId)
        );
    }

    #[test]
    fn restricted_unknown_or_falsely_downgraded_candidate_is_rejected() {
        let (store, memory) = setup();
        let restricted = conversation(vec![turn(
            "secret-turn",
            Some("conversation-c05"),
            TurnState::Complete,
            DataClass::Secret,
            vec![source(
                "secret-source",
                DataClass::Secret,
                DataOrigin::UserAuthored,
            )],
        )]);
        assert_eq!(
            extract(
                &store,
                &memory,
                &restricted,
                "restricted-input",
                vec![candidate(DataClass::Personal, &["secret-source"])]
            ),
            Err(ExtractionError::NoEligibleTurns)
        );

        let inherited_personal = conversation(vec![turn(
            "public-label",
            Some("conversation-c05"),
            TurnState::Complete,
            DataClass::Public,
            vec![source(
                "personal-lineage",
                DataClass::Personal,
                DataOrigin::Provider,
            )],
        )]);
        assert_eq!(
            extract(
                &store,
                &memory,
                &inherited_personal,
                "inherited-class",
                vec![candidate(DataClass::Public, &["public-label"])],
            ),
            Err(ExtractionError::CandidateRejected)
        );

        let mixed_conversation = conversation(vec![
            turn(
                "public-turn",
                Some("conversation-c05"),
                TurnState::Complete,
                DataClass::Public,
                vec![source(
                    "public-turn-source",
                    DataClass::Public,
                    DataOrigin::UserAuthored,
                )],
            ),
            turn(
                "personal-turn",
                Some("conversation-c05"),
                TurnState::Complete,
                DataClass::Personal,
                vec![source(
                    "personal-turn-source",
                    DataClass::Personal,
                    DataOrigin::UserAuthored,
                )],
            ),
        ]);
        assert_eq!(
            extract(
                &store,
                &memory,
                &mixed_conversation,
                "mixed-downgrade",
                vec![candidate(DataClass::Public, &["public-turn"])],
            ),
            Err(ExtractionError::CandidateRejected)
        );

        for (index, class) in [
            DataClass::Private,
            DataClass::Credential,
            DataClass::Unknown,
        ]
        .into_iter()
        .enumerate()
        {
            let restricted = conversation(vec![turn(
                &format!("restricted-{index}"),
                Some("conversation-c05"),
                TurnState::Complete,
                class,
                vec![source(
                    &format!("restricted-source-{index}"),
                    class,
                    DataOrigin::Other,
                )],
            )]);
            assert_eq!(
                extract(
                    &store,
                    &memory,
                    &restricted,
                    &format!("restricted-{index}"),
                    vec![],
                ),
                Err(ExtractionError::NoEligibleTurns)
            );
        }

        let public = conversation(vec![turn(
            "turn-public",
            Some("conversation-c05"),
            TurnState::Complete,
            DataClass::Public,
            vec![source(
                "public-source",
                DataClass::Public,
                DataOrigin::Other,
            )],
        )]);
        assert_eq!(
            extract(
                &store,
                &memory,
                &public,
                "invented-source",
                vec![candidate(DataClass::Public, &["not-in-lineage"])]
            ),
            Err(ExtractionError::CandidateRejected)
        );
        assert_eq!(
            extract(
                &store,
                &memory,
                &public,
                "bad-class",
                vec![candidate(DataClass::Secret, &["public-source"])]
            ),
            Err(ExtractionError::CandidateRejected)
        );
        let secret_lineage = conversation(vec![turn(
            "mixed-turn",
            Some("conversation-c05"),
            TurnState::Complete,
            DataClass::Personal,
            vec![
                source(
                    "personal-source",
                    DataClass::Personal,
                    DataOrigin::UserAuthored,
                ),
                source("unknown-source", DataClass::Unknown, DataOrigin::Other),
            ],
        )]);
        assert_eq!(
            extract(
                &store,
                &memory,
                &secret_lineage,
                "unknown-lineage",
                vec![candidate(DataClass::Personal, &["personal-source"])]
            ),
            Err(ExtractionError::NoEligibleTurns)
        );
    }

    #[test]
    fn owner_mismatch_and_extraction_failure_preserve_original_chat_result() {
        let (store, memory) = setup();
        let mut input = conversation(vec![turn(
            "turn-1",
            Some("conversation-c05"),
            TurnState::Complete,
            DataClass::Public,
            vec![source(
                "public-source",
                DataClass::Public,
                DataOrigin::Other,
            )],
        )]);
        input.owner_id = OTHER_OWNER.into();
        assert_eq!(
            extract(&store, &memory, &input, "other-owner", vec![]),
            Err(ExtractionError::OwnerMismatch)
        );

        struct FailingExtractor;
        impl MemoryExtractor for FailingExtractor {
            fn extract(
                &self,
                _input: &ValidatedConversation,
            ) -> Result<Vec<MemoryCandidate>, ExtractionError> {
                Err(ExtractionError::ExtractorFailed)
            }
        }
        let input = conversation(vec![turn(
            "turn-1",
            Some("conversation-c05"),
            TurnState::Complete,
            DataClass::Public,
            vec![source(
                "public-source",
                DataClass::Public,
                DataOrigin::Other,
            )],
        )]);
        let memory_result = extract_and_save(
            &memory,
            &store,
            &input,
            &ExtractionRequestId::new("extractor-fails").unwrap(),
            &FailingExtractor,
            &FixedClock,
        );
        let combined = preserve_chat_result("synthetic original reply", memory_result);
        assert_eq!(combined.chat_result, "synthetic original reply");
        assert_eq!(
            combined.memory_result,
            Err(ExtractionError::ExtractorFailed)
        );
    }

    #[test]
    fn tombstoned_candidate_cannot_be_reimported_or_found_by_fts() {
        let (store, memory) = setup();
        let input = conversation(vec![turn(
            "turn-1",
            Some("conversation-c05"),
            TurnState::Complete,
            DataClass::Public,
            vec![source(
                "public-source",
                DataClass::Public,
                DataOrigin::Other,
            )],
        )]);
        let draft_candidate = candidate(DataClass::Public, &["turn-1", "public-source"]);
        extract(
            &store,
            &memory,
            &input,
            "forget-me",
            vec![draft_candidate.clone()],
        )
        .unwrap();
        let id = deterministic_memory_id(OWNER, "conversation-c05", "stable-preference");
        store
            .transact(|tx| {
                tx.inner.execute(
                    "INSERT INTO p7_memory_tombstones(owner_id,memory_id,forget_op_id,ledger_generation,forgotten_at_ms) VALUES(?1,?2,'synthetic-forget',1,?3)",
                    params![OWNER, id, NOW_MS],
                )?;
                tx.inner.execute(
                    "DELETE FROM p7_memory_items WHERE owner_id=?1 AND memory_id=?2",
                    params![OWNER, id],
                )?;
                Ok(())
            })
            .unwrap();

        assert_eq!(
            extract(
                &store,
                &memory,
                &input,
                "retry-after-forget",
                vec![draft_candidate]
            ),
            Err(ExtractionError::StorageFailed)
        );
        let page = memory
            .search(
                &store,
                MemoryClass::Public,
                "concise",
                EpochMillis::new(NOW_MS + 1).unwrap(),
                None,
                10,
            )
            .unwrap();
        assert!(page.records.is_empty());
    }

    #[test]
    fn extraction_can_save_multiple_candidates_within_one_request_and_enforces_accepted_body_cap() {
        let (store, memory) = setup();
        let input = conversation(vec![turn(
            "turn-1",
            Some("conversation-c05"),
            TurnState::Complete,
            DataClass::Public,
            vec![source(
                "public-source",
                DataClass::Public,
                DataOrigin::Other,
            )],
        )]);
        let first = candidate(DataClass::Public, &["turn-1", "public-source"]);
        let second = MemoryCandidate {
            candidate_key: "stable-secondary-fact".into(),
            body: "Synthetic public fact for retrieval".into(),
            classification: DataClass::Public,
            source_ids: vec!["public-source".into()],
            kind: MemoryKind::Episodic,
        };
        let result = extract(
            &store,
            &memory,
            &input,
            "two-candidates",
            vec![first, second],
        )
        .unwrap();
        assert_eq!(result.inserted, 2);

        let over_limit = MemoryCandidate {
            candidate_key: "too-long".into(),
            body: "x".repeat(crate::p7_gate_c::ACCEPTED_V1_BOUNDS.max_item_body_utf8_bytes + 1),
            classification: DataClass::Public,
            source_ids: vec!["public-source".into()],
            kind: MemoryKind::Semantic,
        };
        assert_eq!(
            extract(&store, &memory, &input, "over-limit", vec![over_limit]),
            Err(ExtractionError::CandidateRejected)
        );
        assert_eq!(
            P7ExperimentalMemory::config().status,
            super::super::ProvisionalValue::TestOnly
        );
        assert_eq!(
            crate::p7_gate_c::ACCEPTED_V1_BOUNDS.max_item_body_utf8_bytes,
            4096
        );
    }

    #[test]
    fn extraction_admission_reuses_accepted_daily_and_concurrency_limits() {
        let (store, memory) = setup();
        let request_id = |n| ExtractionRequestId::new(format!("admission-{n}")).unwrap();
        let mut now_ms = NOW_MS;
        let first_id = request_id(0);
        let digest = [9u8; 32];
        let _lease = ExtractionAdmission::begin(
            &memory,
            &store,
            &first_id,
            digest,
            EpochMillis::new(now_ms).unwrap(),
        )
        .unwrap();
        assert_eq!(
            ExtractionAdmission::begin(
                &memory,
                &store,
                &request_id(1),
                digest,
                EpochMillis::new(now_ms).unwrap(),
            )
            .err(),
            Some(ExtractionError::ConcurrentExtraction)
        );

        for n in 1..crate::p7_gate_c::ACCEPTED_V1_BOUNDS.extraction_starts_per_utc_day {
            now_ms += TEST_EXTRACTION_LEASE_TTL_MS + 1;
            ExtractionAdmission::begin(
                &memory,
                &store,
                &request_id(n),
                digest,
                EpochMillis::new(now_ms).unwrap(),
            )
            .unwrap();
        }
        now_ms += TEST_EXTRACTION_LEASE_TTL_MS + 1;
        assert_eq!(
            ExtractionAdmission::begin(
                &memory,
                &store,
                &request_id(21),
                digest,
                EpochMillis::new(now_ms).unwrap(),
            )
            .err(),
            Some(ExtractionError::DailyLimitReached)
        );

        let (starts, concurrent, day): (i64, i64, String) = store
            .transact(|tx| {
                Ok(tx.inner.query_row(
                    "SELECT extraction_starts_today,concurrent_extractions,utc_day FROM p7_resource_accounting WHERE owner_id=?1",
                    [OWNER],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
                )?)
            })
            .unwrap();
        assert_eq!(starts, 20);
        assert_eq!(concurrent, 0);
        assert_eq!(day, now_ms.div_euclid(86_400_000).to_string());

        let next_day_ms = NOW_MS + 86_400_000;
        let next_day = EpochMillis::new(next_day_ms).unwrap();
        ExtractionAdmission::begin(&memory, &store, &request_id(22), digest, next_day).unwrap();
        let starts_next_day: i64 = store
            .transact(|tx| {
                Ok(tx.inner.query_row(
                    "SELECT extraction_starts_today FROM p7_resource_accounting WHERE owner_id=?1",
                    [OWNER],
                    |row| row.get(0),
                )?)
            })
            .unwrap();
        assert_eq!(starts_next_day, 1);
        assert_eq!(
            ExtractionAdmission::begin(
                &memory,
                &store,
                &request_id(23),
                digest,
                EpochMillis::new(now_ms).unwrap()
            )
            .err(),
            Some(ExtractionError::ClockRollback)
        );
    }

    #[test]
    fn expired_request_re_admission_fences_old_commit_and_abort_tokens() {
        let (store, memory) = setup();
        let input = conversation(vec![turn(
            "turn-1",
            Some("conversation-c05"),
            TurnState::Complete,
            DataClass::Public,
            vec![source(
                "public-source",
                DataClass::Public,
                DataOrigin::Other,
            )],
        )]);
        let request = ExtractionRequestId::new("same-request-expired").unwrap();
        let validated = validate_conversation(&memory, &input).unwrap();
        let digest = conversation_digest(&validated);
        let old = ExtractionAdmission::begin(
            &memory,
            &store,
            &request,
            digest,
            EpochMillis::new(NOW_MS).unwrap(),
        )
        .unwrap();
        let renewal_time = NOW_MS + TEST_EXTRACTION_LEASE_TTL_MS + 1;
        let current = ExtractionAdmission::begin(
            &memory,
            &store,
            &request,
            digest,
            EpochMillis::new(renewal_time).unwrap(),
        )
        .unwrap();
        assert_ne!(old.lease_id, current.lease_id);

        let draft = validate_candidate(
            &validated,
            candidate(DataClass::Public, &["turn-1", "public-source"]),
            &request,
        )
        .unwrap();
        old.abort(&store).unwrap();
        assert_eq!(
            old.commit(
                &memory,
                &store,
                std::slice::from_ref(&draft),
                &MutableClock::new(renewal_time + 1),
            ),
            Err(ExtractionError::StaleLease)
        );
        let (current_lease, active): (i64, i64) = store
            .transact(|tx| {
                Ok((
                    tx.inner.query_row(
                        "SELECT lease_id FROM p7_extraction_admissions WHERE owner_id=?1 AND request_id=?2 AND state='ACTIVE'",
                        rusqlite::params![OWNER, request.0],
                        |row| row.get(0),
                    )?,
                    tx.inner.query_row(
                        "SELECT concurrent_extractions FROM p7_resource_accounting WHERE owner_id=?1",
                        [OWNER],
                        |row| row.get(0),
                    )?,
                ))
            })
            .unwrap();
        assert_eq!((current_lease, active), (current.lease_id, 1));
        assert_eq!(
            current.commit(
                &memory,
                &store,
                &[draft],
                &MutableClock::new(renewal_time + 1),
            ),
            Ok(SaveSummary {
                inserted: 1,
                already_present: 0
            })
        );
    }

    #[test]
    fn final_commit_uses_fresh_clock_and_rejects_expiry_or_rollback() {
        struct AdvanceClockExtractor<'a> {
            clock: &'a MutableClock,
            final_time: i64,
        }
        impl MemoryExtractor for AdvanceClockExtractor<'_> {
            fn extract(
                &self,
                _conversation: &ValidatedConversation,
            ) -> Result<Vec<MemoryCandidate>, ExtractionError> {
                self.clock.set(self.final_time);
                Ok(vec![candidate(
                    DataClass::Public,
                    &["turn-1", "public-source"],
                )])
            }
        }

        let input = conversation(vec![turn(
            "turn-1",
            Some("conversation-c05"),
            TurnState::Complete,
            DataClass::Public,
            vec![source(
                "public-source",
                DataClass::Public,
                DataOrigin::Other,
            )],
        )]);

        let (store, memory) = setup();
        let expired_clock = MutableClock::new(NOW_MS);
        assert_eq!(
            extract_and_save(
                &memory,
                &store,
                &input,
                &ExtractionRequestId::new("fresh-expiry-check").unwrap(),
                &AdvanceClockExtractor {
                    clock: &expired_clock,
                    final_time: NOW_MS + TEST_EXTRACTION_LEASE_TTL_MS,
                },
                &expired_clock,
            ),
            Err(ExtractionError::LeaseExpired)
        );
        let count: i64 = store
            .transact(|tx| {
                Ok(tx
                    .inner
                    .query_row("SELECT count(*) FROM p7_memory_items", [], |row| row.get(0))?)
            })
            .unwrap();
        assert_eq!(count, 0);

        let (store, memory) = setup();
        let rollback_clock = MutableClock::new(NOW_MS);
        assert_eq!(
            extract_and_save(
                &memory,
                &store,
                &input,
                &ExtractionRequestId::new("fresh-rollback-check").unwrap(),
                &AdvanceClockExtractor {
                    clock: &rollback_clock,
                    final_time: NOW_MS - 1,
                },
                &rollback_clock,
            ),
            Err(ExtractionError::ClockRollback)
        );
        let count: i64 = store
            .transact(|tx| {
                Ok(tx
                    .inner
                    .query_row("SELECT count(*) FROM p7_memory_items", [], |row| row.get(0))?)
            })
            .unwrap();
        assert_eq!(count, 0);
    }
}
