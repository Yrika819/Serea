//! Storage-owned actual-write facts and a synchronous, SQL-free audit port.
//! Journal semantics belong to the participant; storage binds and persists drafts.

use rusqlite::named_params;
use serea_protocol::{
    ActorId, ActorKind, DataClass, Digest, EpochMillis, EventId, ReasonCode, ReceiptId, SemVer,
    SereaEvent, StepId, StepStatus, TaskId, TaskOriginKind, TaskState, canonicalize, digest_of,
};

use crate::{StoreError, TransitionContext, Tx};

/// The whole operation that produced these actual-write facts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuditOperation {
    TaskInserted,
    PlanningStarted,
    PlanPersisted,
    LeaseAcquired,
    LeaseReleased,
    AttemptStarted,
    StepSucceeded,
    StepFailed,
    Blocked,
    InvariantFailed,
    Cancelled,
    RecoveryDecision,
    RecoveryStateChanged,
}

/// Immutable facts from successful writes in an open operation savepoint.
/// These are not a promise that the enclosing transaction will commit.
/// Only storage may construct them; no owner/guard or provider prose is exposed.
///
/// A participant cannot rewrite actual state:
/// ```compile_fail
/// use serea_storage::DurableTransition;
/// use serea_protocol::TaskState;
/// fn forge(facts: &mut DurableTransition) { facts.task_to = TaskState::Completed; }
/// ```
/// Nor can a consumer submit detached facts to the private sink:
/// ```compile_fail
/// use serea_storage::{DurableTransition, Tx};
/// fn replay(tx: &mut Tx<'_>, facts: &DurableTransition) {
///     tx.record_transition(facts).unwrap();
/// }
/// ```
pub struct DurableTransition {
    pub(crate) task_id: TaskId,
    pub(crate) step_id: Option<StepId>,
    pub(crate) operation: AuditOperation,
    pub(crate) task_from: Option<TaskState>,
    pub(crate) task_to: TaskState,
    pub(crate) step_from: Option<StepStatus>,
    pub(crate) step_to: Option<StepStatus>,
    pub(crate) attempt: Option<u32>,
    pub(crate) generation: Option<u32>,
    pub(crate) result: Option<Digest>,
    pub(crate) revision: Option<u32>,
    pub(crate) receipt_id: Option<ReceiptId>,
    pub(crate) reason: Option<ReasonCode>,
    pub(crate) data_class: DataClass,
    pub(crate) now: EpochMillis,
    pub(crate) actor_kind: ActorKind,
    pub(crate) actor_id: ActorId,
    pub(crate) actor_version: SemVer,
    pub(crate) causation_id: Option<EventId>,
    pub(crate) recovery_identity: Option<Digest>,
    pub(crate) recovery_observed_fingerprint: Option<Digest>,
    pub(crate) recovery_decision: Option<String>,
    pub(crate) cancelled_by: Option<TaskOriginKind>,
}

impl DurableTransition {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn task(
        operation: AuditOperation,
        task_id: &TaskId,
        from: Option<TaskState>,
        to: TaskState,
        class: DataClass,
        now: EpochMillis,
        context: &TransitionContext<'_>,
    ) -> Self {
        Self {
            task_id: task_id.clone(),
            step_id: None,
            operation,
            task_from: from,
            task_to: to,
            step_from: None,
            step_to: None,
            attempt: None,
            generation: None,
            result: None,
            revision: None,
            receipt_id: None,
            reason: None,
            data_class: class,
            now,
            actor_kind: context.actor_kind,
            actor_id: context.actor_id.clone(),
            actor_version: context.actor_version.clone(),
            causation_id: context.causation_id.cloned(),
            recovery_identity: None,
            recovery_observed_fingerprint: None,
            recovery_decision: None,
            cancelled_by: None,
        }
    }

    /// Original raw observation, distinct from the stable post-repair identity.
    pub fn recovery_observed_fingerprint(&self) -> Option<&Digest> {
        self.recovery_observed_fingerprint.as_ref()
    }
    pub fn recovery_identity(&self) -> Option<&Digest> {
        self.recovery_identity.as_ref()
    }
    pub fn recovery_decision(&self) -> Option<&str> {
        self.recovery_decision.as_deref()
    }

    pub fn task_id(&self) -> &TaskId {
        &self.task_id
    }
    pub fn step_id(&self) -> Option<&StepId> {
        self.step_id.as_ref()
    }
    pub fn operation(&self) -> AuditOperation {
        self.operation
    }
    pub fn task_from(&self) -> Option<TaskState> {
        self.task_from
    }
    pub fn task_to(&self) -> TaskState {
        self.task_to
    }
    pub fn step_from(&self) -> Option<&StepStatus> {
        self.step_from.as_ref()
    }
    pub fn step_to(&self) -> Option<&StepStatus> {
        self.step_to.as_ref()
    }
    pub fn attempt(&self) -> Option<u32> {
        self.attempt
    }
    pub fn generation(&self) -> Option<u32> {
        self.generation
    }
    pub fn result(&self) -> Option<&Digest> {
        self.result.as_ref()
    }
    pub fn revision(&self) -> Option<u32> {
        self.revision
    }
    pub fn receipt_id(&self) -> Option<&ReceiptId> {
        self.receipt_id.as_ref()
    }
    pub fn reason(&self) -> Option<&ReasonCode> {
        self.reason.as_ref()
    }
    pub fn data_class(&self) -> DataClass {
        self.data_class
    }
    pub fn now(&self) -> EpochMillis {
        self.now
    }
    pub fn actor_kind(&self) -> ActorKind {
        self.actor_kind
    }
    pub fn actor_id(&self) -> &ActorId {
        &self.actor_id
    }
    pub fn actor_version(&self) -> &SemVer {
        &self.actor_version
    }
    pub fn causation_id(&self) -> Option<&EventId> {
        self.causation_id.as_ref()
    }
    pub fn cancelled_by(&self) -> Option<&TaskOriginKind> {
        self.cancelled_by.as_ref()
    }
}

/// Journal vocabulary supported by the single audit port. No absent writer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JournalKind {
    TaskInserted,
    PlanPersisted,
    TaskStateChanged,
    StepLeaseAcquired,
    StepLeaseReleased,
    StepAttemptStarted,
    StepCommitted,
    StepFailed,
    ReceiptRecorded,
    TaskCancelRequested,
    TaskTerminal,
    RecoveryDecision,
}

impl JournalKind {
    pub fn wire_name(self) -> &'static str {
        match self {
            Self::TaskInserted => "TASK_INSERTED",
            Self::PlanPersisted => "PLAN_PERSISTED",
            Self::TaskStateChanged => "TASK_STATE_CHANGED",
            Self::StepLeaseAcquired => "STEP_LEASE_ACQUIRED",
            Self::StepLeaseReleased => "STEP_LEASE_RELEASED",
            Self::StepAttemptStarted => "STEP_ATTEMPT_STARTED",
            Self::StepCommitted => "STEP_COMMITTED",
            Self::StepFailed => "STEP_FAILED",
            Self::ReceiptRecorded => "RECEIPT_RECORDED",
            Self::TaskCancelRequested => "TASK_CANCEL_REQUESTED",
            Self::TaskTerminal => "TASK_TERMINAL",
            Self::RecoveryDecision => "RECOVERY_DECISION",
        }
    }
}

/// A semantic draft, not an envelope or independent append capability.
pub struct JournalRecord {
    pub kind: JournalKind,
    pub state_from: Option<String>,
    pub state_to: Option<String>,
    pub reason: Option<ReasonCode>,
    /// Original object-root SCJ-1 JSON; storage canonicalizes before persistence.
    pub payload_json: Vec<u8>,
}

pub type JournalRecords = Vec<JournalRecord>;

/// Exactly one synchronous participant per audited transaction. It receives no
/// SQL, Tx or connection capability and must return an ordered nonempty batch.
pub trait TaskAuditParticipant: Send + Sync {
    fn records(&self, facts: &DurableTransition) -> Result<JournalRecords, StoreError>;
}

/// The second and only other semantic participant in P3's fixed composition.
/// It receives the same immutable successful-write facts as the journal mapper.
pub trait EventParticipant: Send + Sync {
    fn events(&self, facts: &DurableTransition) -> Result<Vec<SereaEvent>, StoreError>;
}

impl Tx<'_> {
    pub(crate) fn require_audit(&self) -> Result<(), StoreError> {
        self.ensure_active()?;
        self.audit.map(|_| ()).ok_or(StoreError::AuditRequired)
    }

    pub(crate) fn record_transition(
        &mut self,
        facts: &DurableTransition,
    ) -> Result<(), StoreError> {
        self.require_audit()?;
        match facts.data_class {
            DataClass::Public | DataClass::Personal => (),
            DataClass::Private => return Err(StoreError::AtRestProtectionUnavailable),
            DataClass::Secret | DataClass::Credential => return Err(StoreError::ClassRefused),
        }
        let drafts = self
            .audit
            .ok_or(StoreError::AuditRequired)?
            .records(facts)?;
        let event_drafts = match self.events {
            Some(participant) => participant.events(facts)?,
            None => {
                #[cfg(test)]
                {
                    Vec::new()
                }
                #[cfg(not(test))]
                {
                    return Err(StoreError::EventParticipantRequired);
                }
            }
        };
        if drafts.is_empty()
            || (facts.operation == AuditOperation::RecoveryDecision
                && (drafts.len() != 1 || drafts[0].kind != JournalKind::RecoveryDecision))
        {
            return Err(StoreError::AuditRejected);
        }
        // Validate the complete batch before its first INSERT. Journal semantics
        // remain participant-owned; these checks bind claimed states/evidence.
        let rows = drafts
            .into_iter()
            .map(|draft| {
                validate_draft(facts, &draft)?;
                let text = std::str::from_utf8(&draft.payload_json)
                    .map_err(|_| StoreError::AuditRejected)?;
                let payload = canonicalize(text).map_err(|_| StoreError::AuditRejected)?;
                if payload.first() != Some(&b'{') {
                    return Err(StoreError::AuditRejected);
                }
                let payload = String::from_utf8(payload).map_err(|_| StoreError::AuditRejected)?;
                if draft.kind == JournalKind::RecoveryDecision {
                    let value: serde_json::Value =
                        serde_json::from_str(&payload).map_err(|_| StoreError::AuditRejected)?;
                    let text = |key| value.get(key).and_then(serde_json::Value::as_str);
                    if text("fingerprint") != facts.recovery_identity().map(Digest::as_str)
                        || text("observed_fingerprint")
                            != facts.recovery_observed_fingerprint().map(Digest::as_str)
                        || text("decision") != facts.recovery_decision()
                        || value
                            .get("observed_fingerprint")
                            .is_some_and(|value| !value.is_null() && !value.is_string())
                        || value.get("raw_state").is_some()
                        || value.get("raw_status").is_some()
                    {
                        return Err(StoreError::AuditRejected);
                    }
                }
                let digest = digest_of(&payload).map_err(|_| StoreError::AuditRejected)?;
                Ok((draft, payload, digest))
            })
            .collect::<Result<Vec<_>, StoreError>>()?;
        // P2H: audit persistence fault. The whole batch is validated and not one
        // journal row has been written; a failure here must roll the whole
        // enclosing operation back. Absent without the test-only feature.
        #[cfg(feature = "p2h-fault-injection")]
        crate::fault::reach(crate::fault::Window::BeforeJournalInsert)?;
        for (draft, payload, digest) in rows {
            let reference = facts.recovery_identity.as_ref().or(facts.result.as_ref());
            if draft.kind == JournalKind::RecoveryDecision {
                let exists: bool = self.inner.query_row(
                    "SELECT EXISTS(SELECT 1 FROM task_journal WHERE task_id=?1
                        AND step_id IS ?2 AND journal_kind='RECOVERY_DECISION'
                        AND state_from IS ?3 AND state_to IS ?4 AND attempt IS ?5
                        AND reason_code IS ?6 AND payload_ref_digest=?7)",
                    rusqlite::params![
                        facts.task_id.as_str(),
                        facts.step_id.as_ref().map(StepId::as_str),
                        draft.state_from,
                        draft.state_to,
                        facts.attempt,
                        draft.reason.as_ref().map(ReasonCode::as_str),
                        reference.map(Digest::as_str)
                    ],
                    |r| r.get(0),
                )?;
                if exists {
                    continue;
                }
            }
            let changed = self.inner.execute(
                "INSERT INTO task_journal(journal_id,task_id,step_id,journal_seq,journal_kind,
                    state_from,state_to,attempt,reason_code,actor_kind,actor_id,actor_version,causation_id,
                    data_class_rank,occurred_at_ms,payload_digest,payload_json,payload_ref_digest)
                 SELECT :task||':'||(coalesce(max(journal_seq),0)+1),:task,:step,coalesce(max(journal_seq),0)+1,
                    :kind,:from,:to,:attempt,:reason,:actor_kind,:actor_id,:version,:cause,:rank,:now,:digest,:payload,:ref
                 FROM task_journal WHERE task_id=:task",
                named_params! {
                    ":task": facts.task_id.as_str(), ":step": facts.step_id.as_ref().map(StepId::as_str),
                    ":kind": draft.kind.wire_name(), ":from": draft.state_from, ":to": draft.state_to,
                    ":attempt": facts.attempt, ":reason": draft.reason.as_ref().map(ReasonCode::as_str),
                    ":actor_kind": facts.actor_kind.wire_name(), ":actor_id": facts.actor_id.as_str(),
                    ":version": facts.actor_version.as_str(), ":cause": facts.causation_id.as_ref().map(EventId::as_str),
                    ":rank": facts.data_class.rank(), ":now": facts.now.get(), ":digest": digest.as_str(),
                    ":payload": payload, ":ref": reference.map(Digest::as_str),
                },
            )?;
            if changed != 1 {
                return Err(StoreError::ConstraintViolation);
            }
        }
        for event in event_drafts {
            self.append_event(event, None)?;
        }
        Ok(())
    }
}

fn validate_draft(facts: &DurableTransition, draft: &JournalRecord) -> Result<(), StoreError> {
    // Refuse claims about a different whole operation without selecting which
    // records the participant must emit or rendering any semantic payload.
    let operation_matches = match draft.kind {
        JournalKind::TaskInserted => facts.operation == AuditOperation::TaskInserted,
        JournalKind::PlanPersisted => facts.operation == AuditOperation::PlanPersisted,
        JournalKind::StepLeaseAcquired => facts.operation == AuditOperation::LeaseAcquired,
        JournalKind::StepLeaseReleased => facts.operation == AuditOperation::LeaseReleased,
        JournalKind::StepAttemptStarted => facts.operation == AuditOperation::AttemptStarted,
        JournalKind::StepCommitted | JournalKind::ReceiptRecorded => {
            facts.operation == AuditOperation::StepSucceeded
        }
        JournalKind::StepFailed => facts.operation == AuditOperation::StepFailed,
        JournalKind::TaskCancelRequested => facts.operation == AuditOperation::Cancelled,
        JournalKind::TaskStateChanged | JournalKind::TaskTerminal => true,
        JournalKind::RecoveryDecision => facts.operation == AuditOperation::RecoveryDecision,
    };
    if !operation_matches {
        return Err(StoreError::AuditRejected);
    }
    let step_kind = matches!(
        draft.kind,
        JournalKind::StepLeaseAcquired
            | JournalKind::StepLeaseReleased
            | JournalKind::StepAttemptStarted
            | JournalKind::StepCommitted
            | JournalKind::StepFailed
            | JournalKind::ReceiptRecorded
    );
    let (from, to) = if step_kind {
        if facts.step_id.is_none() {
            return Err(StoreError::AuditRejected);
        }
        (
            facts.step_from.as_ref().map(StepStatus::as_str),
            facts.step_to.as_ref().map(StepStatus::as_str),
        )
    } else {
        (
            facts.task_from.map(TaskState::wire_name),
            Some(facts.task_to.wire_name()),
        )
    };
    if draft.state_from.as_deref() != from || draft.state_to.as_deref() != to {
        return Err(StoreError::AuditRejected);
    }
    if facts.reason.is_some() && draft.reason.as_ref() != facts.reason.as_ref() {
        return Err(StoreError::AuditRejected);
    }
    let evidence = match draft.kind {
        JournalKind::TaskInserted => facts.task_from.is_none(),
        JournalKind::PlanPersisted => facts.revision.is_some(),
        JournalKind::TaskStateChanged => facts.task_from.is_some_and(|from| from != facts.task_to),
        JournalKind::StepLeaseAcquired
        | JournalKind::StepLeaseReleased
        | JournalKind::StepAttemptStarted => facts.attempt.is_some() && facts.generation.is_some(),
        JournalKind::StepCommitted => facts.result.is_some() && to == Some("SUCCEEDED"),
        JournalKind::StepFailed => to == Some("FAILED"),
        JournalKind::ReceiptRecorded => facts.receipt_id.is_some() && to == Some("SUCCEEDED"),
        JournalKind::TaskCancelRequested => facts.task_to == TaskState::Cancelled,
        JournalKind::TaskTerminal => facts.task_to.is_terminal(),
        JournalKind::RecoveryDecision => {
            facts.recovery_identity.is_some()
                && facts.recovery_decision.is_some()
                && facts.reason.is_some()
        }
    };
    if evidence {
        Ok(())
    } else {
        Err(StoreError::AuditRejected)
    }
}

/// Explicit local mapper for storage tests only. Callers must still opt in with
/// `transact_with_audit(&crate::audit::TestAudit, ...)`; there is no default mapper.
#[cfg(test)]
pub(crate) struct TestAudit;

#[cfg(test)]
impl TaskAuditParticipant for TestAudit {
    fn records(&self, facts: &DurableTransition) -> Result<JournalRecords, StoreError> {
        let payload = if facts.operation == AuditOperation::RecoveryDecision {
            serde_json::to_vec(&serde_json::json!({
                "decision": facts.recovery_decision(),
                "fingerprint": facts.recovery_identity().map(Digest::as_str),
                "observed_fingerprint": facts.recovery_observed_fingerprint().map(Digest::as_str),

                "attempt": facts.attempt(), "generation": facts.generation(),
            }))
            .map_err(|_| StoreError::AuditRejected)?
        } else {
            format!(
                "{{\"attempt\":{},\"generation\":{},\"result_digest\":{}}}",
                facts
                    .attempt
                    .map_or_else(|| "null".into(), |n| n.to_string()),
                facts
                    .generation
                    .map_or_else(|| "null".into(), |n| n.to_string()),
                facts
                    .result
                    .as_ref()
                    .map_or_else(|| "null".into(), |d| format!("\"{}\"", d.as_str()))
            )
            .into_bytes()
        };
        let step = |kind| JournalRecord {
            kind,
            state_from: facts.step_from.as_ref().map(|s| s.as_str().to_owned()),
            state_to: facts.step_to.as_ref().map(|s| s.as_str().to_owned()),
            reason: facts.reason.clone(),
            payload_json: payload.clone(),
        };
        let task = |kind| JournalRecord {
            kind,
            state_from: facts.task_from.map(|s| s.wire_name().to_owned()),
            state_to: Some(facts.task_to.wire_name().to_owned()),
            reason: facts.reason.clone(),
            payload_json: payload.clone(),
        };
        let mut rows = match facts.operation {
            AuditOperation::TaskInserted => vec![task(JournalKind::TaskInserted)],
            AuditOperation::PlanPersisted => vec![task(JournalKind::PlanPersisted)],
            AuditOperation::LeaseAcquired => vec![step(JournalKind::StepLeaseAcquired)],
            AuditOperation::LeaseReleased => vec![step(JournalKind::StepLeaseReleased)],
            AuditOperation::AttemptStarted => vec![step(JournalKind::StepAttemptStarted)],
            AuditOperation::StepSucceeded => {
                let mut rows = vec![step(JournalKind::StepCommitted)];
                if facts.receipt_id.is_some() {
                    rows.push(step(JournalKind::ReceiptRecorded));
                }
                rows
            }
            AuditOperation::StepFailed => vec![step(JournalKind::StepFailed)],
            AuditOperation::Cancelled => vec![task(JournalKind::TaskCancelRequested)],
            AuditOperation::RecoveryDecision => vec![task(JournalKind::RecoveryDecision)],
            AuditOperation::PlanningStarted
            | AuditOperation::Blocked
            | AuditOperation::InvariantFailed
            | AuditOperation::RecoveryStateChanged => vec![],
        };
        if facts.task_from.is_some_and(|from| from != facts.task_to) {
            rows.push(task(JournalKind::TaskStateChanged));
            if facts.task_to.is_terminal() {
                rows.push(task(JournalKind::TaskTerminal));
            }
        }
        Ok(rows)
    }
}
