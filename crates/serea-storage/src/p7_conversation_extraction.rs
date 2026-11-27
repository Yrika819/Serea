//! Synthetic conversation-derived Memory path for P7 Gate C development.
//!
//! This module is nested under `experimental_memory` and is compiled only for
//! tests with both explicit P7 development features. It never calls a model.

use super::{MemoryClass, MemoryDraft, MemoryKind, MemoryProvenance, P7ExperimentalMemory};
use crate::{Store, StoreError};
use serea_protocol::EpochMillis;
use sha2::{Digest as _, Sha256};
use std::collections::{BTreeMap, BTreeSet};

const SYNTHETIC_INPUT_MAX_BYTES: usize = 64 * 1024;
const SYNTHETIC_CANDIDATE_MAX_BYTES: usize = 4096;
const SYNTHETIC_LINEAGE_MAX_BYTES: usize = 1024;

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
pub(super) enum ExtractionError {
    InvalidRequestId,
    ConversationNotComplete,
    MissingConversationId,
    OwnerMismatch,
    NoEligibleTurns,
    InputTooLarge,
    InvalidSourceLineage,
    CandidateRejected,
    ExtractorFailed,
    StorageFailed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct SaveSummary {
    pub inserted: usize,
    pub already_present: usize,
}

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
    now: EpochMillis,
) -> Result<SaveSummary, ExtractionError> {
    let validated = validate_conversation(memory, input)?;
    let _admission = ExtractionAdmission::begin(memory, store, now)?;
    let candidates = extractor.extract(&validated)?;
    if candidates.len() > crate::p7_gate_c::ACCEPTED_V1_BOUNDS.max_active_items {
        return Err(ExtractionError::CandidateRejected);
    }

    let mut summary = SaveSummary {
        inserted: 0,
        already_present: 0,
    };
    let mut candidate_keys = BTreeSet::new();
    for candidate in candidates {
        if !candidate_keys.insert(candidate.candidate_key.clone()) {
            return Err(ExtractionError::CandidateRejected);
        }
        let draft = validate_candidate(&validated, candidate, request_id)?;
        match memory
            .insert_once(store, &draft, now)
            .map_err(map_storage_error)?
        {
            true => summary.inserted += 1,
            false => summary.already_present += 1,
        }
    }
    Ok(summary)
}

/// Reuses P7's existing durable accounting row for the Owner-approved daily
/// start and concurrent extraction ceilings. The bounds are read from ADR-0043
/// and this guard is test-only with its containing module.
struct ExtractionAdmission<'a> {
    owner_id: &'a str,
    store: &'a Store,
}

impl<'a> ExtractionAdmission<'a> {
    fn begin(
        memory: &'a P7ExperimentalMemory,
        store: &'a Store,
        now: EpochMillis,
    ) -> Result<Self, ExtractionError> {
        let bounds = crate::p7_gate_c::ACCEPTED_V1_BOUNDS;
        let day = now.get().div_euclid(86_400_000).to_string();
        store
            .transact(|tx| {
                super::ensure_ready(tx, &memory.owner_id)?;
                let last_day: String = tx.inner.query_row(
                    "SELECT utc_day FROM p7_resource_accounting WHERE owner_id=?1",
                    [&memory.owner_id],
                    |row| row.get(0),
                )?;
                let requested_day = day
                    .parse::<i64>()
                    .map_err(|_| StoreError::ConstraintViolation)?;
                let previous_day = if last_day.is_empty() {
                    None
                } else {
                    Some(
                        last_day
                            .parse::<i64>()
                            .map_err(|_| StoreError::ConstraintViolation)?,
                    )
                };
                if previous_day.is_some_and(|previous| requested_day < previous) {
                    return Err(StoreError::ConstraintViolation);
                }
                let changed = tx.inner.execute(
                    "UPDATE p7_resource_accounting SET extraction_starts_today=CASE WHEN utc_day=?2 THEN extraction_starts_today+1 ELSE 1 END,concurrent_extractions=concurrent_extractions+1,utc_day=?2 WHERE owner_id=?1 AND concurrent_extractions < ?3 AND (utc_day<>?2 OR extraction_starts_today < ?4)",
                    rusqlite::params![memory.owner_id, day, bounds.concurrent_extractions_per_owner as i64, bounds.extraction_starts_per_utc_day as i64],
                )?;
                if changed == 1 {
                    Ok(())
                } else {
                    Err(StoreError::ConstraintViolation)
                }
            })
            .map_err(map_storage_error)?;
        Ok(Self {
            owner_id: &memory.owner_id,
            store,
        })
    }
}

impl Drop for ExtractionAdmission<'_> {
    fn drop(&mut self) {
        let _ = self.store.transact(|tx| {
            super::ensure_ready(tx, self.owner_id)?;
            let changed = tx.inner.execute(
                "UPDATE p7_resource_accounting SET concurrent_extractions=concurrent_extractions-1 WHERE owner_id=?1 AND concurrent_extractions > 0",
                [self.owner_id],
            )?;
            if changed == 1 {
                Ok(())
            } else {
                Err(StoreError::ConstraintViolation)
            }
        });
    }
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

    const OWNER: &str = "own_00000000000000000000000000000071";
    const OTHER_OWNER: &str = "own_00000000000000000000000000000072";
    const NOW_MS: i64 = 1_800_000_000_000;

    struct FixedClock;
    impl Clock for FixedClock {
        fn now_ms(&self) -> Result<EpochMillis, ProtocolError> {
            EpochMillis::new(NOW_MS)
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
            EpochMillis::new(NOW_MS).unwrap(),
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
            EpochMillis::new(NOW_MS).unwrap(),
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
                inserted: 0,
                already_present: 1
            }
        );
        assert_eq!(
            extract(&store, &memory, &input, "new-request", vec![candidate]).unwrap(),
            SaveSummary {
                inserted: 0,
                already_present: 1
            }
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
            EpochMillis::new(NOW_MS).unwrap(),
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
            EpochMillis::new(NOW_MS).unwrap(),
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
        let now = EpochMillis::new(NOW_MS).unwrap();
        let held = ExtractionAdmission::begin(&memory, &store, now).unwrap();
        assert_eq!(
            ExtractionAdmission::begin(&memory, &store, now).err(),
            Some(ExtractionError::StorageFailed)
        );
        drop(held);

        for _ in 1..crate::p7_gate_c::ACCEPTED_V1_BOUNDS.extraction_starts_per_utc_day {
            drop(ExtractionAdmission::begin(&memory, &store, now).unwrap());
        }
        assert_eq!(
            ExtractionAdmission::begin(&memory, &store, now).err(),
            Some(ExtractionError::StorageFailed)
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
        assert_eq!(day, NOW_MS.div_euclid(86_400_000).to_string());

        let next_day = EpochMillis::new(NOW_MS + 86_400_000).unwrap();
        let next_day_reservation = ExtractionAdmission::begin(&memory, &store, next_day).unwrap();
        drop(next_day_reservation);
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
            ExtractionAdmission::begin(&memory, &store, now).err(),
            Some(ExtractionError::StorageFailed)
        );
    }
}
