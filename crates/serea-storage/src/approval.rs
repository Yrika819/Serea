//! Typed durable approval request persistence over the P6B authority schema.

use std::collections::BTreeSet;
use std::fmt;

use rusqlite::{OptionalExtension, params};
use serde_json::{Value, json};
use serea_protocol::{
    ActorId, ActorKind, ApprovalId, Authorization, CapabilityId, DataClass, Digest, EpochMillis,
    GrantId, LeaseOwner, RequestedBy, RiskClass, SemVer, SideEffectClass, StepId, StepStatus,
    TaskId, TaskKind, TaskState, canonicalize, digest_of,
};

use crate::audit::{AuditOperation, DurableTransition};
use crate::{EventDraft, Store, StoreError, TransitionContext, Tx, task::task_class};

type ApprovalResponseRow = (
    String,
    String,
    i64,
    i64,
    String,
    i64,
    String,
    String,
    i64,
    String,
    String,
    String,
    i64,
    i64,
    Option<String>,
    Option<String>,
);
type ExistingGrantResponseRow = (
    String,
    String,
    Option<String>,
    Option<String>,
    String,
    Option<String>,
    i64,
    i64,
    String,
);

fn read_gate_action_members(
    tx: &mut Tx<'_>,
    task_id: &TaskId,
    gate_id: &StepId,
    class: DataClass,
) -> Result<std::collections::BTreeMap<String, String>, StoreError> {
    let digest: String = tx
        .inner
        .query_row(
            "SELECT input_digest FROM task_steps WHERE task_id=?1 AND step_id=?2
         AND kind='WAIT_APPROVAL'",
            rusqlite::params![task_id.as_str(), gate_id.as_str()],
            |row| row.get(0),
        )
        .optional()?
        .ok_or(StoreError::InvalidApprovalRequest)?;
    let blob = crate::BlobRef::new(
        Digest::new(digest).map_err(|_| StoreError::CorruptRow)?,
        class,
    );
    let raw = tx.get_blob(&blob)?;
    let value: Value = serde_json::from_slice(&raw).map_err(|_| StoreError::CorruptRow)?;
    if value.get("gate").and_then(Value::as_str) != Some("R2_ACTION_GROUP") {
        return Err(StoreError::InvalidApprovalRequest);
    }
    let actions = value
        .get("actions")
        .and_then(Value::as_array)
        .ok_or(StoreError::InvalidApprovalRequest)?;
    let mut result = std::collections::BTreeMap::new();
    for action in actions {
        let step_id = action
            .get("step_id")
            .and_then(Value::as_str)
            .ok_or(StoreError::InvalidApprovalRequest)?;
        let digest = action
            .get("arguments_digest")
            .and_then(Value::as_str)
            .ok_or(StoreError::InvalidApprovalRequest)?;
        if result
            .insert(step_id.to_owned(), digest.to_owned())
            .is_some()
        {
            return Err(StoreError::InvalidApprovalRequest);
        }
    }
    Ok(result)
}

/// One complete action member for the request side of the R2 authority set.
#[derive(Clone, PartialEq, Eq)]
pub struct ApprovalRequestActionDraft {
    /// Exact Step identity.
    pub step_id: StepId,
    /// Exact canonical arguments digest persisted on the durable Step.
    pub arguments_digest: Digest,
    /// Canonical structural scope displayed to the human.
    pub scope_json: String,
    /// Digest of scope JSON under SCJ-1.
    pub scope_digest: Digest,
}

impl fmt::Debug for ApprovalRequestActionDraft {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ApprovalRequestActionDraft")
            .field("step_id", &self.step_id)
            .field("arguments_digest", &self.arguments_digest)
            .field("scope_digest", &self.scope_digest)
            .field("scope_json_bytes", &self.scope_json.len())
            .finish()
    }
}

/// Complete host-authored approval request facts.
#[derive(Clone, PartialEq, Eq)]
pub struct ApprovalRequestStorageDraft {
    /// Host-minted immutable request identity.
    pub approval_id: ApprovalId,
    /// Owning Task.
    pub task_id: TaskId,
    /// Host-validated Task kind.
    pub task_kind: TaskKind,
    /// Exact durable plan revision.
    pub plan_revision: i64,
    /// Capability shared by all enumerated actions.
    pub capability_id: CapabilityId,
    /// Pinned capability version shared by all actions.
    pub capability_version: SemVer,
    /// Pinned registry generation shared by all actions.
    pub generation_id: i64,
    /// Pinned descriptor digest shared by all actions.
    pub descriptor_digest: Digest,
    /// Host-reviewed risk class.
    pub risk_class: RiskClass,
    /// Host-reviewed side-effect class.
    pub side_effect_class: SideEffectClass,
    /// Host-resolved authorization requirement.
    pub authorization: Authorization,
    /// Persisted request class, capped at PERSONAL.
    pub data_class: DataClass,
    /// Host-owned action provenance.
    pub requested_by: RequestedBy,
    /// Host-derived automation context.
    pub automation_context: ApprovalAutomationContext,
    /// SHA-256 of the complete normalized action set.
    pub action_set_digest: Digest,
    /// Complete normalized one-to-eight requested actions.
    pub actions: Vec<ApprovalRequestActionDraft>,
    /// Stable host summary builder category.
    pub summary_kind: String,
    /// Host-authored safe preview. No unratified global byte bound applies.
    pub summary: String,
    /// Request creation instant.
    pub raised_at: EpochMillis,
    /// Expiry no more than 30 minutes after creation.
    pub expires_at: EpochMillis,
}

impl fmt::Debug for ApprovalRequestStorageDraft {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ApprovalRequestStorageDraft")
            .field("approval_id", &self.approval_id)
            .field("task_id", &self.task_id)
            .field("plan_revision", &self.plan_revision)
            .field("capability_id", &self.capability_id)
            .field("capability_version", &self.capability_version)
            .field("generation_id", &self.generation_id)
            .field("data_class", &self.data_class)
            .field("action_set_digest", &self.action_set_digest)
            .field("action_count", &self.actions.len())
            .field("summary_kind", &self.summary_kind)
            .field("summary_bytes", &self.summary.len())
            .field("raised_at", &self.raised_at)
            .field("expires_at", &self.expires_at)
            .finish()
    }
}

/// Authenticated response facts after the Policy authentication port has
/// verified the caller, session freshness and any elevated confirmation.
/// Storage still validates every request/action binding and lifecycle edge.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApprovalResponseStorageDraft {
    pub approval_id: ApprovalId,
    pub grant_id: Option<GrantId>,
    pub actor_id: ActorId,
    pub request_action_set_digest: Digest,
    /// Trusted host instant when the response was revalidated against current state.
    pub checked_at: EpochMillis,
    pub decision: ApprovalResponseDecisionStorage,
    pub approved_step_ids: Vec<StepId>,
    pub decided_at: EpochMillis,
    pub expires_at: Option<EpochMillis>,
    pub granted_by: &'static str,
    pub actor_device_id: Option<String>,
    pub actor_session_id: Option<String>,
    pub auth_strength: &'static str,
    pub elevated_ref: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApprovalResponseDecisionStorage {
    Grant,
    Deny,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApprovalResponseStorageOutcome {
    Granted(GrantId),
    Denied,
    AlreadyGranted(GrantId),
    AlreadyDenied,
    Expired,
}

/// Result of one durable per-Step grant-use attempt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GrantConsumptionOutcome {
    Consumed { uses_remaining: u32 },
    AlreadyConsumed,
    Expired,
    Revoked,
    Exhausted,
}

/// Result of the separate authenticated grant-revocation operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApprovalRevocationOutcome {
    Revoked,
    AlreadyRevoked,
}

/// Facts emitted only by the distinct trusted revocation authentication port.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApprovalRevocationStorageDraft {
    pub approval_id: ApprovalId,
    pub grant_id: GrantId,
    pub actor_id: ActorId,
    pub actor_class: &'static str,
    pub device_id: Option<String>,
    pub session_id: Option<String>,
    pub revoked_at: EpochMillis,
    pub checked_at: EpochMillis,
}

/// Closed host-derived context used in durable approval provenance.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApprovalAutomationContext {
    /// Direct user request or model-proposed work.
    Interactive,
    /// Durable schedule-triggered work.
    Scheduled,
    /// Read-only proactive work.
    Proactive,
    /// Host maintenance.
    System,
}

impl ApprovalAutomationContext {
    const fn wire_name(self) -> &'static str {
        match self {
            Self::Interactive => "INTERACTIVE",
            Self::Scheduled => "SCHEDULED",
            Self::Proactive => "PROACTIVE",
            Self::System => "SYSTEM",
        }
    }

    fn derived(kind: TaskKind, requested_by: RequestedBy) -> Option<Self> {
        use RequestedBy as R;
        use TaskKind as T;
        match (kind, requested_by) {
            (T::Proactive, _) | (_, R::ProactiveWatcher) => Some(Self::Proactive),
            (T::Maintenance, _) => Some(Self::System),
            (T::Scheduled, _) => Some(Self::Scheduled),
            (T::UserRequest | T::DelegatedHostGoal, R::User | R::Model | R::Scheduler) => {
                Some(Self::Interactive)
            }
            (T::UserRequest | T::DelegatedHostGoal, R::System) => None,
        }
    }
}

impl Store {
    /// Persists a complete PENDING request, normalized actions and routing event atomically.
    pub fn create_approval_request(
        &self,
        draft: ApprovalRequestStorageDraft,
        event: EventDraft,
    ) -> Result<(), StoreError> {
        self.transact(|tx| tx.create_approval_request(draft, event))
    }

    /// Applies one previously authenticated response to durable request state.
    pub fn respond_to_approval(
        &self,
        draft: ApprovalResponseStorageDraft,
        event: EventDraft,
    ) -> Result<ApprovalResponseStorageOutcome, StoreError> {
        self.transact(|tx| tx.respond_to_approval(draft, event))
    }

    /// Consumes one sealed grant member without opening an invocation path.
    pub fn consume_grant_use(
        &self,
        grant_id: &GrantId,
        task_id: &TaskId,
        step_id: &StepId,
        now: EpochMillis,
    ) -> Result<GrantConsumptionOutcome, StoreError> {
        self.transact(|tx| tx.consume_grant_use(grant_id, task_id, step_id, now))
    }

    /// Expires a grant with remaining uses and appends its grant-keyed audit event atomically.
    pub fn expire_approval_grant(
        &self,
        grant_id: &GrantId,
        now: EpochMillis,
        event: EventDraft,
    ) -> Result<bool, StoreError> {
        self.transact(|tx| tx.expire_approval_grant(grant_id, now, event))
    }

    /// Expires one pending request at the exact inclusive expiry boundary.
    pub fn expire_approval_request(
        &self,
        approval_id: &ApprovalId,
        now: EpochMillis,
        event: EventDraft,
    ) -> Result<bool, StoreError> {
        self.transact(|tx| tx.expire_approval_request(approval_id, now, event))
    }

    /// Revokes one active grant through the separate authenticated port.
    pub fn revoke_approval_grant(
        &self,
        draft: ApprovalRevocationStorageDraft,
        event: EventDraft,
    ) -> Result<ApprovalRevocationOutcome, StoreError> {
        self.transact(|tx| tx.revoke_approval_grant(draft, event))
    }
}

impl Tx<'_> {
    /// Completes a planned approval gate as NOT_REQUIRED only while its exact
    /// enumerated action group, active Policy revision, Task and gate remain
    /// current. The normal lease/attempt/result lifecycle supplies the required
    /// SUCCEEDED fields and audit records; this creates no approval or Grant.
    #[allow(clippy::too_many_arguments)]
    pub fn complete_approval_gate_not_required(
        &mut self,
        task_id: &TaskId,
        wait_step_id: &StepId,
        plan_revision: u32,
        actions: &[(StepId, Digest)],
        policy_revision_id: i64,
        policy_rules_digest: &Digest,
        owner: LeaseOwner,
        now: EpochMillis,
        expires_at: EpochMillis,
        context: &TransitionContext<'_>,
    ) -> Result<crate::StepCommit, StoreError> {
        use std::collections::{BTreeMap, BTreeSet};

        self.require_audit()?;
        self.operation_savepoint(|tx| {
            if actions.is_empty() || actions.len() > 8 || policy_revision_id <= 0 {
                return Err(StoreError::InvalidApprovalRequest);
            }
            let mut expected = BTreeMap::new();
            for (step_id, digest) in actions {
                if expected.insert(step_id.as_str(), digest.as_str()).is_some() {
                    return Err(StoreError::InvalidApprovalRequest);
                }
            }
            let task_facts: (String, i64, Option<i64>, Option<i64>, i64) = tx.inner.query_row(
                "SELECT state,plan_revision,cancelled_at_ms,deadline_at_ms,data_class_rank
                 FROM tasks WHERE task_id=?1",
                [task_id.as_str()],
                |row| Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?,row.get(4)?)),
            ).optional()?.ok_or(StoreError::TaskNotFound)?;
            if !matches!(task_facts.0.as_str(), "READY" | "EXECUTING")
                || task_facts.1 != i64::from(plan_revision)
                || task_facts.2.is_some()
                || task_facts.3.is_some_and(|deadline| now.get() >= deadline)
            {
                return Err(StoreError::InvalidApprovalRequest);
            }
            let policy: Option<(i64, String)> = tx.inner.query_row(
                "SELECT r.revision_id,r.rules_digest FROM policy_state p JOIN policy_revisions r
                   ON r.revision_id=p.active_revision_id WHERE p.singleton=1",
                [],
                |row| Ok((row.get(0)?,row.get(1)?)),
            ).optional()?;
            if policy.as_ref() != Some(&(policy_revision_id, policy_rules_digest.as_str().to_owned())) {
                return Err(StoreError::InvalidApprovalRequest);
            }
            let class = task_class(&tx.inner, task_id)?;
            let gate: Option<(i64, String)> = tx.inner.query_row(
                "SELECT sequence,input_digest FROM task_steps
                 WHERE task_id=?1 AND step_id=?2 AND plan_revision=?3
                   AND kind='WAIT_APPROVAL' AND status='PLANNED'",
                rusqlite::params![task_id.as_str(),wait_step_id.as_str(),plan_revision],
                |row| Ok((row.get(0)?,row.get(1)?)),
            ).optional()?;
            let (gate_sequence, input_digest) = gate.ok_or(StoreError::InvalidApprovalRequest)?;
            let input_ref = crate::BlobRef::new(
                Digest::new(input_digest).map_err(|_| StoreError::CorruptRow)?, class,
            );
            let input = tx.get_blob(&input_ref)?;
            let gate_json: Value = serde_json::from_slice(&input).map_err(|_| StoreError::CorruptRow)?;
            if gate_json.get("gate").and_then(Value::as_str) != Some("R2_ACTION_GROUP") {
                return Err(StoreError::InvalidApprovalRequest);
            }
            let listed = gate_json.get("actions").and_then(Value::as_array)
                .ok_or(StoreError::InvalidApprovalRequest)?;
            let mut listed_actions = BTreeMap::new();
            for member in listed {
                let step = member.get("step_id").and_then(Value::as_str)
                    .ok_or(StoreError::InvalidApprovalRequest)?;
                let digest = member.get("arguments_digest").and_then(Value::as_str)
                    .ok_or(StoreError::InvalidApprovalRequest)?;
                if listed_actions.insert(step, digest).is_some() {
                    return Err(StoreError::InvalidApprovalRequest);
                }
            }
            if listed_actions != expected {
                return Err(StoreError::InvalidApprovalRequest);
            }
            let mut seen_sequences = BTreeSet::new();
            for (step_id, digest) in actions {
                let member: Option<(i64,String,String,String)> = tx.inner.query_row(
                    "SELECT sequence,status,kind,input_digest FROM task_steps
                     WHERE task_id=?1 AND step_id=?2 AND plan_revision=?3",
                    rusqlite::params![task_id.as_str(),step_id.as_str(),plan_revision],
                    |row| Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?)),
                ).optional()?;
                let (sequence,status,kind,actual_digest) = member.ok_or(StoreError::InvalidApprovalRequest)?;
                if sequence <= gate_sequence || status != "PLANNED" || kind != "CAPABILITY"
                    || actual_digest != digest.as_str() || !seen_sequences.insert(sequence)
                {
                    return Err(StoreError::InvalidApprovalRequest);
                }
                let overlay: Option<(String,i64,i64)> = tx.inner.query_row(
                    "SELECT coalesce(o.availability_state,'ENABLED'),
                            coalesce(o.experimental_opt_in,0),d.experimental
                     FROM task_steps s JOIN step_capability_bindings b
                       ON b.task_id=s.task_id AND b.step_id=s.step_id
                     JOIN capability_descriptor_revisions d ON d.descriptor_digest=b.descriptor_digest
                     LEFT JOIN capability_overlays o ON o.capability_id=b.capability_id
                     WHERE s.task_id=?1 AND s.step_id=?2",
                    rusqlite::params![task_id.as_str(),step_id.as_str()],
                    |row| Ok((row.get(0)?,row.get(1)?,row.get(2)?)),
                ).optional()?;
                if overlay.is_none_or(|(state,opt_in,experimental)|
                    state != "ENABLED" || (experimental == 1 && opt_in != 1))
                {
                    return Err(StoreError::InvalidApprovalRequest);
                }
            }
            let first_sequence = seen_sequences.first().copied().ok_or(StoreError::InvalidApprovalRequest)?;
            let available_gates: Vec<String> = {
                let mut statement = tx.inner.prepare(
                    "SELECT step_id FROM task_steps WHERE task_id=?1 AND plan_revision=?2
                      AND kind='WAIT_APPROVAL' AND status='PLANNED' AND sequence<?3 ORDER BY sequence",
                )?;
                statement.query_map(rusqlite::params![task_id.as_str(),plan_revision,first_sequence],
                    |row| row.get(0))?.collect::<Result<_,_>>()?
            };
            if available_gates.as_slice() != [wait_step_id.as_str()] {
                return Err(StoreError::InvalidApprovalRequest);
            }
            let pending: i64 = tx.inner.query_row(
                "SELECT count(*) FROM approval_requests WHERE task_id=?1 AND status='PENDING'",
                [task_id.as_str()], |row| row.get(0),
            )?;
            if pending != 0 {
                return Err(StoreError::InvalidApprovalRequest);
            }
            let guard = tx.acquire_audited(
                task_id.clone(), wait_step_id.clone(), owner, None, now, expires_at, context,
            )?;
            tx.begin_attempt(&guard, now, context)?;
            tx.commit_step_outcome(
                guard,
                crate::StepOutcome::Succeeded { result_json: br#"{"outcome":"NOT_REQUIRED"}"#, receipt: None },
                now,
                context,
            )
        })
    }

    /// Moves a live Task into `WAITING_APPROVAL` and creates its complete durable
    /// request plus `APPROVAL_REQUIRED` event in the same outer transaction.
    /// A failed request, event, audit participant, or commit therefore leaves
    /// neither half of the wait transition visible.
    pub fn raise_approval_request_and_wait(
        &mut self,
        draft: ApprovalRequestStorageDraft,
        event: EventDraft,
        policy_revision_id: i64,
        policy_rules_digest: &serea_protocol::Digest,
        now: EpochMillis,
        context: &TransitionContext<'_>,
    ) -> Result<(), StoreError> {
        self.ensure_active()?;
        self.operation_savepoint(|tx| {
            if draft.raised_at != now || policy_revision_id <= 0 {
                return Err(StoreError::InvalidApprovalRequest);
            }
            let active: Option<(i64, String)> = tx
                .inner
                .query_row(
                    "SELECT p.active_revision_id,r.rules_digest
                   FROM policy_state p JOIN policy_revisions r
                     ON r.revision_id=p.active_revision_id
                  WHERE p.singleton=1",
                    [],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .optional()?;
            if active.as_ref().map(|(id, digest)| (*id, digest.as_str()))
                != Some((policy_revision_id, policy_rules_digest.as_str()))
            {
                return Err(StoreError::InvalidPolicyRevision);
            }
            let (state, plan_revision, updated_at): (String, i64, i64) = tx
                .inner
                .query_row(
                    "SELECT state,plan_revision,updated_at_ms FROM tasks WHERE task_id=?1",
                    [draft.task_id.as_str()],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
                )
                .map_err(|error| match error {
                    rusqlite::Error::QueryReturnedNoRows => StoreError::TaskNotFound,
                    other => other.into(),
                })?;
            let from = match state.as_str() {
                "READY" => TaskState::Ready,
                "EXECUTING" => TaskState::Executing,
                _ => return Err(StoreError::IllegalTaskTransition),
            };
            if plan_revision != draft.plan_revision || now.get() < updated_at {
                return Err(StoreError::IllegalTaskTransition);
            }
            let pending: i64 = tx.inner.query_row(
                "SELECT count(*) FROM approval_requests WHERE task_id=?1 AND status='PENDING'",
                [draft.task_id.as_str()],
                |row| row.get(0),
            )?;
            if pending != 0 {
                return Err(StoreError::InvalidApprovalRequest);
            }
            let mut first_action_sequence = None;
            for action in &draft.actions {
                let sequence: Option<i64> = tx.inner.query_row(
                    "SELECT sequence FROM task_steps
                      WHERE task_id=?1 AND step_id=?2 AND plan_revision=?3 AND kind='CAPABILITY'",
                    rusqlite::params![
                        draft.task_id.as_str(),
                        action.step_id.as_str(),
                        draft.plan_revision
                    ],
                    |row| row.get(0),
                ).optional()?;
                let sequence = sequence.ok_or(StoreError::InvalidApprovalRequest)?;
                first_action_sequence = Some(first_action_sequence.map_or(sequence, |current: i64| current.min(sequence)));
            }
            let first_action_sequence = first_action_sequence.ok_or(StoreError::InvalidApprovalRequest)?;
            let wait_steps: Vec<String> = {
                let mut statement = tx.inner.prepare(
                    "SELECT step_id FROM task_steps
                      WHERE task_id=?1 AND kind='WAIT_APPROVAL' AND status='PLANNED'
                        AND plan_revision=?2 AND sequence<?3 ORDER BY sequence",
                )?;
                statement.query_map(
                    rusqlite::params![draft.task_id.as_str(), draft.plan_revision, first_action_sequence],
                    |row| row.get(0),
                )?.collect::<Result<_, _>>()?
            };
            if wait_steps.len() != 1 {
                return Err(StoreError::InvalidApprovalRequest);
            }
            let wait_step_id = StepId::new(wait_steps[0].clone())
                .map_err(|_| StoreError::InvalidApprovalRequest)?;
            let class = task_class(&tx.inner, &draft.task_id)?;
            let expected_gate_actions: Vec<(StepId, Digest)> = draft.actions.iter()
                .map(|action| (action.step_id.clone(), action.arguments_digest.clone()))
                .collect();
            if read_gate_action_members(tx, &draft.task_id, &wait_step_id, class)?
                != expected_gate_actions.into_iter().map(|(step,digest)|
                    (step.to_string(), digest.to_string())).collect()
            {
                return Err(StoreError::InvalidApprovalRequest);
            }
            let changed = tx.inner.execute(
                "UPDATE tasks SET state='WAITING_APPROVAL',blocked_reason=NULL,updated_at_ms=?1
                 WHERE task_id=?2 AND state=?3 AND plan_revision=?4 AND updated_at_ms<=?1
                   AND cancelled_at_ms IS NULL",
                rusqlite::params![
                    now.get(),
                    draft.task_id.as_str(),
                    from.wire_name(),
                    draft.plan_revision
                ],
            )?;
            if changed != 1 {
                return Err(StoreError::IllegalTaskTransition);
            }
            let changed = tx.inner.execute(
                "UPDATE task_steps SET status='WAITING',attempt=1,started_at_ms=?1,lease_generation=1
                  WHERE task_id=?2 AND step_id=?3 AND kind='WAIT_APPROVAL' AND status='PLANNED'
                    AND plan_revision=?4",
                rusqlite::params![now.get(), draft.task_id.as_str(), wait_step_id.as_str(), draft.plan_revision],
            )?;
            if changed != 1 {
                return Err(StoreError::InvalidApprovalRequest);
            }
            tx.create_approval_request(draft.clone(), event)?;
            let mut facts = DurableTransition::task(
                AuditOperation::ApprovalWaitStarted,
                &draft.task_id,
                Some(from),
                TaskState::WaitingApproval,
                class,
                now,
                context,
            );
            facts.step_id = Some(wait_step_id);
            facts.step_from = Some(StepStatus::new("PLANNED").map_err(|_| StoreError::CorruptRow)?);
            facts.step_to = Some(StepStatus::new("WAITING").map_err(|_| StoreError::CorruptRow)?);
            facts.attempt = Some(1);
            facts.generation = Some(1);
            tx.record_transition(&facts)?;
            Ok(())
        })
    }

    /// Persists a complete PENDING request, normalized actions and routing event in one
    /// failure-atomic savepoint, joinable to the Task Engine's larger transaction.
    pub fn create_approval_request(
        &mut self,
        draft: ApprovalRequestStorageDraft,
        event: EventDraft,
    ) -> Result<(), StoreError> {
        self.ensure_active()?;
        self.operation_savepoint(|tx| {
            validate_draft(tx, &draft, &event)?;
            let count = i64::try_from(draft.actions.len())
                .map_err(|_| StoreError::InvalidApprovalRequest)?;
            tx.inner
                .execute(
                    "INSERT INTO approval_requests(
                    approval_id,task_id,plan_revision,capability_id,capability_version,
                    generation_id,descriptor_digest,risk_class,side_effect_class,authorization,
                    data_class,requested_by,automation_context,action_set_digest,action_count,
                    max_uses,summary_kind,summary,raised_at_ms,expires_at_ms,status)
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?15,
                         ?16,?17,?18,?19,'PENDING')",
                    params![
                        draft.approval_id.as_str(),
                        draft.task_id.as_str(),
                        draft.plan_revision,
                        draft.capability_id.to_string(),
                        draft.capability_version.to_string(),
                        draft.generation_id,
                        draft.descriptor_digest.as_str(),
                        draft.risk_class.wire_name(),
                        draft.side_effect_class.wire_name(),
                        draft.authorization.wire_name(),
                        draft.data_class.wire_name(),
                        draft.requested_by.wire_name(),
                        draft.automation_context.wire_name(),
                        draft.action_set_digest.as_str(),
                        count,
                        draft.summary_kind,
                        draft.summary,
                        draft.raised_at.get(),
                        draft.expires_at.get(),
                    ],
                )
                .map_err(approval_sql_error)?;
            for (position, action) in draft.actions.iter().enumerate() {
                let position =
                    i64::try_from(position).map_err(|_| StoreError::InvalidApprovalRequest)?;
                tx.inner
                    .execute(
                        "INSERT INTO approval_request_actions(
                        approval_id,position,step_id,arguments_digest,scope,scope_digest)
                     VALUES (?1,?2,?3,?4,?5,?6)",
                        params![
                            draft.approval_id.as_str(),
                            position,
                            action.step_id.as_str(),
                            action.arguments_digest.as_str(),
                            action.scope_json,
                            action.scope_digest.as_str(),
                        ],
                    )
                    .map_err(approval_sql_error)?;
            }
            tx.append_event(event.event, event.retention_at)?;
            Ok(())
        })
    }

    /// Response transaction is joinable so Task Engine state, the request,
    /// grant seal/members and the lifecycle event can commit as one unit.
    pub fn respond_to_approval(
        &mut self,
        draft: ApprovalResponseStorageDraft,
        event: EventDraft,
    ) -> Result<ApprovalResponseStorageOutcome, StoreError> {
        self.ensure_active()?;
        self.operation_savepoint(|tx| apply_response(tx, &draft, &event))
    }

    /// Transaction-joinable, exact-member authority consumption interface.
    pub fn consume_grant_use(
        &mut self,
        grant_id: &GrantId,
        task_id: &TaskId,
        step_id: &StepId,
        now: EpochMillis,
    ) -> Result<GrantConsumptionOutcome, StoreError> {
        self.ensure_active()?;
        self.operation_savepoint(|tx| {
            let row: Option<(String, i64, String)> = tx.inner.query_row(
                "SELECT g.status,g.expires_at_ms,t.state FROM approval_grants g JOIN tasks t ON t.task_id=g.task_id WHERE g.grant_id=?1 AND g.task_id=?2",
                params![grant_id.as_str(),task_id.as_str()],
                |row| Ok((row.get(0)?,row.get(1)?,row.get(2)?)),
            ).optional()?;
            let Some((status,expires_at,task_state))=row else { return Err(StoreError::ApprovalGrantNotFound); };
            if !matches!(task_state.as_str(),"READY"|"EXECUTING"|"WAITING_APPROVAL") {
                return Err(StoreError::InvalidApprovalResponse);
            }
            if tx.inner.query_row("SELECT EXISTS(SELECT 1 FROM approval_grant_uses WHERE step_id=?1)",[step_id.as_str()],|row|row.get::<_,bool>(0))? {
                return Ok(GrantConsumptionOutcome::AlreadyConsumed);
            }
            match status.as_str() {
                "REVOKED" => return Ok(GrantConsumptionOutcome::Revoked),
                "EXPIRED" => return Ok(GrantConsumptionOutcome::Expired),
                "EXHAUSTED" => return Ok(GrantConsumptionOutcome::Exhausted),
                "ACTIVE" => {}
                _ => return Err(StoreError::InvalidApprovalResponse),
            }
            if now.get() >= expires_at {
                return Ok(GrantConsumptionOutcome::Expired);
            }
            let member: bool = tx.inner.query_row(
                "SELECT EXISTS(SELECT 1 FROM approval_grant_members WHERE grant_id=?1 AND step_id=?2)",
                params![grant_id.as_str(),step_id.as_str()],|row|row.get(0))?;
            if !member { return Err(StoreError::InvalidApprovalResponse); }
            tx.inner.execute(
                "INSERT INTO approval_grant_uses(grant_id,step_id,task_id,consumed_at_ms) VALUES (?1,?2,?3,?4)",
                params![grant_id.as_str(),step_id.as_str(),task_id.as_str(),now.get()],
            ).map_err(approval_sql_error)?;
            let remaining: i64=tx.inner.query_row("SELECT uses_remaining FROM approval_grants WHERE grant_id=?1",[grant_id.as_str()],|row|row.get(0))?;
            Ok(GrantConsumptionOutcome::Consumed { uses_remaining: u32::try_from(remaining).map_err(|_|StoreError::InvalidApprovalResponse)? })
        })
    }

    /// Transaction-joinable grant expiry with remaining authority and its strict audit event.
    pub fn expire_approval_grant(
        &mut self,
        grant_id: &GrantId,
        now: EpochMillis,
        event: EventDraft,
    ) -> Result<bool, StoreError> {
        self.ensure_active()?;
        self.operation_savepoint(|tx| {
            let row: Option<(String, i64, i64, String, String)> = tx.inner.query_row(
                "SELECT g.status,g.expires_at_ms,g.uses_remaining,g.task_id,g.approval_id
                   FROM approval_grants g WHERE g.grant_id=?1",
                [grant_id.as_str()],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?)),
            ).optional()?;
            let Some((status, expires_at, uses_remaining, task_id, approval_id)) = row else {
                return Err(StoreError::ApprovalGrantNotFound);
            };
            if status != "ACTIVE" || uses_remaining <= 0 || now.get() < expires_at {
                return Ok(false);
            }
            let step_id: String = tx.inner.query_row(
                "SELECT step_id FROM approval_grant_members WHERE grant_id=?1 ORDER BY position LIMIT 1",
                [grant_id.as_str()],
                |row| row.get(0),
            ).map_err(|_| StoreError::InvalidApprovalResponse)?;
            validate_expired_unused_event(&event, grant_id, &approval_id, &task_id, &step_id, now)?;
            let changed = tx.inner.execute(
                "UPDATE approval_grants SET status='EXPIRED' WHERE grant_id=?1 AND status='ACTIVE' AND uses_remaining>0",
                [grant_id.as_str()],
            )?;
            if changed != 1 {
                return Err(StoreError::InvalidApprovalResponse);
            }
            tx.append_event(event.event.clone(), event.retention_at)?;
            Ok(true)
        })
    }

    /// Transaction-joinable expiry transition with its durable routing event.
    pub fn expire_approval_request(
        &mut self,
        approval_id: &ApprovalId,
        now: EpochMillis,
        event: EventDraft,
    ) -> Result<bool, StoreError> {
        self.ensure_active()?;
        self.operation_savepoint(|tx| {
            let state: Option<(String,i64,String)> = tx.inner.query_row(
                "SELECT r.status,r.expires_at_ms,r.task_id FROM approval_requests r WHERE r.approval_id=?1",
                [approval_id.as_str()],|row|Ok((row.get(0)?,row.get(1)?,row.get(2)?)),
            ).optional()?;
            let Some((status,expires_at,task_id))=state else {return Err(StoreError::ApprovalRequestNotFound);};
            if status!="PENDING" || now.get()<expires_at { return Ok(false); }
            let leading=request_leading_step(tx,approval_id)?;
            validate_expiration_event(&event,approval_id,&task_id,&leading,now)?;
            let changed=tx.inner.execute("UPDATE approval_requests SET status='EXPIRED',terminal_reason='REQUEST_EXPIRED' WHERE approval_id=?1 AND status='PENDING'",[approval_id.as_str()])?;
            if changed!=1 {return Err(StoreError::InvalidApprovalResponse);}
            tx.append_event(event.event.clone(),event.retention_at)?;
            Ok(true)
        })
    }

    /// Transaction-joinable grant-only revocation; consumed uses remain historical.
    pub fn revoke_approval_grant(
        &mut self,
        draft: ApprovalRevocationStorageDraft,
        event: EventDraft,
    ) -> Result<ApprovalRevocationOutcome, StoreError> {
        self.ensure_active()?;
        self.operation_savepoint(|tx|{
            if !matches!(draft.actor_class,"USER"|"ADMIN")
                || draft.revoked_at>draft.checked_at
                || (draft.actor_class=="USER"&&(draft.device_id.is_none()||draft.session_id.is_none()))
                || draft.device_id.is_some()!=draft.session_id.is_some() { return Err(StoreError::InvalidApprovalResponse); }
            let row:Option<(String,i64,i64,String,String,String)>=tx.inner.query_row(
                "SELECT g.status,g.expires_at_ms,g.granted_at_ms,g.task_id,r.approval_id,r.status FROM approval_grants g JOIN approval_requests r ON r.approval_id=g.approval_id WHERE g.grant_id=?1 AND g.approval_id=?2",
                params![draft.grant_id.as_str(),draft.approval_id.as_str()],|row|Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?,row.get(4)?,row.get(5)?)),
            ).optional()?;
            let Some((status,expires_at,granted_at,task_id,approval_id,request_status))=row else{return Err(StoreError::ApprovalGrantNotFound);};
            if request_status!="APPROVED" {return Err(StoreError::InvalidApprovalResponse);}
            if status=="REVOKED" {return Ok(ApprovalRevocationOutcome::AlreadyRevoked);}
            if status=="EXPIRED"||draft.checked_at.get()>=expires_at{return Err(StoreError::ApprovalGrantExpired);}
            if status!="ACTIVE" {return Err(StoreError::InvalidApprovalResponse);}
            if draft.revoked_at.get()<granted_at{return Err(StoreError::InvalidApprovalResponse);}
            let step:String=tx.inner.query_row("SELECT step_id FROM approval_grant_members WHERE grant_id=?1 ORDER BY position LIMIT 1",[draft.grant_id.as_str()],|row|row.get(0)).map_err(|_|StoreError::InvalidApprovalResponse)?;
            validate_revocation_event(&event,&draft,&task_id,&step)?;
            let changed=tx.inner.execute("UPDATE approval_grants SET status='REVOKED' WHERE grant_id=?1 AND status='ACTIVE'",[draft.grant_id.as_str()])?;
            if changed!=1{return Err(StoreError::InvalidApprovalResponse);}
            tx.append_event(event.event.clone(),event.retention_at)?;
            let _=approval_id;
            Ok(ApprovalRevocationOutcome::Revoked)
        })
    }
}

fn validate_revocation_event(
    event: &EventDraft,
    draft: &ApprovalRevocationStorageDraft,
    task_id: &str,
    step_id: &str,
) -> Result<(), StoreError> {
    let p = &event.event.payload;
    let keys: Vec<&str> = p.keys().map(String::as_str).collect();
    let expected_kind = if draft.actor_class == "USER" {
        ActorKind::User
    } else {
        ActorKind::Host
    };
    if event.event.kind.wire_name() != "APPROVAL_REVOKED"
        || event.event.data_class != DataClass::Personal
        || event.event.actor.kind != expected_kind
        || event.event.actor.id != draft.actor_id
        || event.event.occurred_at.to_epoch_millis() != draft.revoked_at
        || event
            .event
            .correlation_id
            .as_ref()
            .is_none_or(|id| id.as_str() != task_id)
        || event
            .event
            .trace
            .as_ref()
            .and_then(|t| t.task_id.as_ref())
            .is_none_or(|id| id.as_str() != task_id)
        || event
            .event
            .trace
            .as_ref()
            .and_then(|t| t.step_id.as_ref())
            .is_none_or(|id| id.as_str() != step_id)
        || keys
            != [
                "actor_class",
                "approval_id",
                "device_id",
                "grant_id",
                "revoked_at",
                "session_id",
                "step_id",
                "task_id",
            ]
        || p.get("approval_id").and_then(Value::as_str) != Some(draft.approval_id.as_str())
        || p.get("grant_id").and_then(Value::as_str) != Some(draft.grant_id.as_str())
        || p.get("task_id").and_then(Value::as_str) != Some(task_id)
        || p.get("step_id").and_then(Value::as_str) != Some(step_id)
        || p.get("actor_class").and_then(Value::as_str) != Some(draft.actor_class)
        || p.get("revoked_at").and_then(Value::as_i64) != Some(draft.revoked_at.get())
        || p.get("device_id").and_then(Value::as_str) != draft.device_id.as_deref()
        || p.get("session_id").and_then(Value::as_str) != draft.session_id.as_deref()
    {
        return Err(StoreError::InvalidApprovalEvent);
    }
    Ok(())
}

fn apply_response(
    tx: &mut Tx<'_>,
    draft: &ApprovalResponseStorageDraft,
    event: &EventDraft,
) -> Result<ApprovalResponseStorageOutcome, StoreError> {
    let row: Option<ApprovalResponseRow> = tx
        .inner
        .query_row(
            "SELECT r.status,t.state,r.raised_at_ms,r.expires_at_ms,r.task_id,r.plan_revision,
                r.capability_id,r.capability_version,r.generation_id,r.descriptor_digest,
                r.action_set_digest,r.requested_by,r.action_count,r.max_uses,
                r.terminal_reason,g.grant_id
           FROM approval_requests r JOIN tasks t ON t.task_id=r.task_id
           LEFT JOIN approval_grants g ON g.approval_id=r.approval_id
          WHERE r.approval_id=?1",
            [draft.approval_id.as_str()],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                    row.get(6)?,
                    row.get(7)?,
                    row.get(8)?,
                    row.get(9)?,
                    row.get(10)?,
                    row.get(11)?,
                    row.get(12)?,
                    row.get(13)?,
                    row.get(14)?,
                    row.get(15)?,
                ))
            },
        )
        .optional()
        .map_err(StoreError::from)?;
    let Some((
        status,
        task_state,
        _raised,
        request_expires,
        task_id,
        plan_revision,
        capability_id,
        capability_version,
        generation_id,
        descriptor_digest,
        request_digest,
        _requested_by,
        action_count,
        _max_uses,
        _terminal_reason,
        prior_grant,
    )) = row
    else {
        return Err(StoreError::ApprovalRequestNotFound);
    };

    if request_digest != draft.request_action_set_digest.as_str() {
        return Err(StoreError::InvalidApprovalResponse);
    }
    validate_response_shape(draft)?;
    if task_state == "CANCELLED" || task_state == "FAILED" || task_state == "COMPLETED" {
        return Err(StoreError::TaskNotApprovable);
    }

    match status.as_str() {
        "APPROVED" if draft.decision == ApprovalResponseDecisionStorage::Grant => {
            let id = prior_grant.ok_or(StoreError::InvalidApprovalResponse)?;
            let stored: ExistingGrantResponseRow = tx.inner.query_row(
                "SELECT grant_id,status,actor_device_id,actor_session_id,auth_strength,elevated_ref,granted_at_ms,expires_at_ms,granted_by FROM approval_grants WHERE approval_id=?1",
                [draft.approval_id.as_str()],
                |row| Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?,row.get(4)?,row.get(5)?,row.get(6)?,row.get(7)?,row.get(8)?)),
            ).map_err(|_| StoreError::InvalidApprovalResponse)?;
            let mut member_ids = tx.inner.prepare(
                "SELECT step_id FROM approval_grant_members WHERE grant_id=?1 ORDER BY step_id",
            )?;
            let saved_ids: Vec<String> = member_ids
                .query_map([&id], |row| row.get(0))?
                .collect::<Result<_, _>>()?;
            let mut requested_ids: Vec<&str> =
                draft.approved_step_ids.iter().map(StepId::as_str).collect();
            requested_ids.sort_unstable();
            if stored.1 == "REVOKED" {
                return Err(StoreError::ApprovalGrantRevoked);
            }
            if stored.1 == "EXPIRED" || draft.checked_at.get() >= stored.7 {
                return Err(StoreError::ApprovalGrantExpired);
            }
            if stored.0 != id
                || !matches!(stored.1.as_str(), "ACTIVE" | "EXHAUSTED")
                || stored.2 != draft.actor_device_id
                || stored.3 != draft.actor_session_id
                || stored.4 != draft.auth_strength
                || stored.5 != draft.elevated_ref
                || stored.6 != draft.decided_at.get()
                || Some(stored.7) != draft.expires_at.map(EpochMillis::get)
                || stored.8 != draft.granted_by
                || saved_ids.iter().map(String::as_str).collect::<Vec<_>>() != requested_ids
                || draft.grant_id.as_ref().map(GrantId::as_str) != Some(id.as_str())
            {
                return Err(StoreError::InvalidApprovalResponse);
            }
            let id = GrantId::new(id).map_err(|_| StoreError::InvalidApprovalResponse)?;
            return Ok(ApprovalResponseStorageOutcome::AlreadyGranted(id));
        }
        "DENIED" if draft.decision == ApprovalResponseDecisionStorage::Deny => {
            return Ok(ApprovalResponseStorageOutcome::AlreadyDenied);
        }
        "APPROVED" if draft.decision == ApprovalResponseDecisionStorage::Deny => {
            let grant_status: String = tx
                .inner
                .query_row(
                    "SELECT status FROM approval_grants WHERE approval_id=?1",
                    [draft.approval_id.as_str()],
                    |row| row.get(0),
                )
                .map_err(|_| StoreError::InvalidApprovalResponse)?;
            if grant_status == "REVOKED" {
                return Err(StoreError::ApprovalGrantRevoked);
            }
            if grant_status == "EXPIRED" {
                return Err(StoreError::ApprovalGrantExpired);
            }
            return Err(StoreError::ApprovalResponseConflict);
        }
        "DENIED" => return Err(StoreError::ApprovalResponseConflict),
        "EXPIRED" => return Err(StoreError::ApprovalRequestExpired),
        "PENDING" => {}
        _ => return Err(StoreError::InvalidApprovalResponse),
    }
    if draft.checked_at.get() >= request_expires {
        tx.inner.execute(
            "UPDATE approval_requests SET status='EXPIRED',terminal_reason='REQUEST_EXPIRED' WHERE approval_id=?1 AND status='PENDING'",
            [draft.approval_id.as_str()],
        )?;
        let leading = request_leading_step(tx, &draft.approval_id)?;
        validate_response_event(
            event,
            draft,
            &task_id,
            leading.as_str(),
            None,
            "APPROVAL_EXPIRED",
        )?;
        tx.append_event(event.event.clone(), event.retention_at)?;
        return Ok(ApprovalResponseStorageOutcome::Expired);
    }
    if !matches!(
        task_state.as_str(),
        "READY" | "EXECUTING" | "WAITING_APPROVAL"
    ) {
        return Err(StoreError::InvalidApprovalResponse);
    }

    match draft.decision {
        ApprovalResponseDecisionStorage::Deny => {
            if draft.grant_id.is_some()
                || !draft.approved_step_ids.is_empty()
                || draft.expires_at.is_some()
            {
                return Err(StoreError::InvalidApprovalResponse);
            }
            let leading = request_leading_step(tx, &draft.approval_id)?;
            validate_response_event(
                event,
                draft,
                &task_id,
                leading.as_str(),
                None,
                "APPROVAL_DENIED",
            )?;
            let changed = tx.inner.execute(
                "UPDATE approval_requests SET status='DENIED',terminal_reason='USER_DENIED' WHERE approval_id=?1 AND status='PENDING'",
                [draft.approval_id.as_str()],
            )?;
            if changed != 1 {
                return Err(StoreError::InvalidApprovalResponse);
            }
            tx.append_event(event.event.clone(), event.retention_at)?;
            Ok(ApprovalResponseStorageOutcome::Denied)
        }
        ApprovalResponseDecisionStorage::Grant => {
            let grant_id = draft
                .grant_id
                .as_ref()
                .ok_or(StoreError::InvalidApprovalResponse)?;
            let expires_at = draft
                .expires_at
                .ok_or(StoreError::InvalidApprovalResponse)?;
            if draft.approved_step_ids.is_empty()
                || draft.approved_step_ids.len() > 8
                || draft.approved_step_ids.len() as i64 > action_count
                || expires_at.get() <= draft.checked_at.get()
                || expires_at.get() > request_expires
                || expires_at
                    .get()
                    .checked_sub(draft.decided_at.get())
                    .is_none_or(|d| !(1..=1_800_000).contains(&d))
            {
                return Err(StoreError::InvalidApprovalResponse);
            }
            let mut seen = BTreeSet::new();
            if draft
                .approved_step_ids
                .iter()
                .any(|id| !seen.insert(id.as_str()))
            {
                return Err(StoreError::InvalidApprovalResponse);
            }
            let mut members = Vec::with_capacity(draft.approved_step_ids.len());
            for step_id in &draft.approved_step_ids {
                let member: Option<(String,String)> = tx.inner.query_row(
                    "SELECT arguments_digest,scope_digest FROM approval_request_actions WHERE approval_id=?1 AND step_id=?2",
                    params![draft.approval_id.as_str(),step_id.as_str()],
                    |row| Ok((row.get(0)?,row.get(1)?)),
                ).optional()?;
                members.push((
                    step_id.clone(),
                    member.ok_or(StoreError::InvalidApprovalResponse)?,
                ));
            }
            members.sort_by(|a, b| a.0.as_str().as_bytes().cmp(b.0.as_str().as_bytes()));
            let digest_actions: Vec<ApprovalRequestActionDraft> = members.iter().map(|(step,(args,scope))| {
                let raw_scope: String = tx.inner.query_row(
                    "SELECT scope FROM approval_request_actions WHERE approval_id=?1 AND step_id=?2",
                    params![draft.approval_id.as_str(),step.as_str()], |row| row.get(0))?;
                Ok(ApprovalRequestActionDraft {
                    step_id: step.clone(),
                    arguments_digest: Digest::new(args.clone()).map_err(|_| StoreError::InvalidApprovalResponse)?,
                    scope_json: raw_scope,
                    scope_digest: Digest::new(scope.clone()).map_err(|_| StoreError::InvalidApprovalResponse)?,
                })
            }).collect::<Result<_,StoreError>>()?;
            let action_digest = action_set_digest(&digest_actions)?;
            validate_response_event(
                event,
                draft,
                &task_id,
                members[0].0.as_str(),
                Some(grant_id.as_str()),
                "APPROVAL_GRANTED",
            )?;
            let changed = tx.inner.execute("UPDATE approval_requests SET status='APPROVED' WHERE approval_id=?1 AND status='PENDING'", [draft.approval_id.as_str()])?;
            if changed != 1 {
                return Err(StoreError::InvalidApprovalResponse);
            }
            tx.inner.execute(
                "INSERT INTO approval_grants(grant_id,approval_id,task_id,plan_revision,capability_id,capability_version,generation_id,descriptor_digest,action_set_digest,action_count,max_uses,uses_remaining,granted_at_ms,expires_at_ms,granted_by,actor_device_id,actor_session_id,auth_strength,elevated_ref,status) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?10,?10,?11,?12,?13,?14,?15,?16,?17,'ACTIVE')",
                params![grant_id.as_str(),draft.approval_id.as_str(),task_id,plan_revision,capability_id,capability_version,generation_id,descriptor_digest,action_digest.as_str(),members.len() as i64,draft.decided_at.get(),expires_at.get(),draft.granted_by,draft.actor_device_id,draft.actor_session_id,draft.auth_strength,draft.elevated_ref],
            ).map_err(approval_sql_error)?;
            for (position, (step, (args, scope))) in members.iter().enumerate() {
                tx.inner.execute("INSERT INTO approval_grant_members(grant_id,position,approval_id,step_id,arguments_digest,scope_digest) VALUES (?1,?2,?3,?4,?5,?6)",params![grant_id.as_str(),position as i64,draft.approval_id.as_str(),step.as_str(),args,scope]).map_err(approval_sql_error)?;
            }
            tx.inner
                .execute(
                    "INSERT INTO approval_grant_seals(grant_id,sealed_at_ms) VALUES (?1,?2)",
                    params![grant_id.as_str(), draft.checked_at.get()],
                )
                .map_err(approval_sql_error)?;
            tx.append_event(event.event.clone(), event.retention_at)?;
            Ok(ApprovalResponseStorageOutcome::Granted(grant_id.clone()))
        }
    }
}

fn validate_response_shape(draft: &ApprovalResponseStorageDraft) -> Result<(), StoreError> {
    if draft.checked_at < draft.decided_at
        || !matches!(draft.granted_by, "USER" | "ADMIN")
        || !matches!(draft.auth_strength, "SESSION" | "ELEVATED_CONFIRMED")
        || (draft.auth_strength == "ELEVATED_CONFIRMED") != draft.elevated_ref.is_some()
        || (draft.granted_by == "USER"
            && (draft.actor_device_id.is_none() || draft.actor_session_id.is_none()))
        || draft.actor_device_id.is_some() != draft.actor_session_id.is_some()
    {
        return Err(StoreError::InvalidApprovalResponse);
    }
    match draft.decision {
        ApprovalResponseDecisionStorage::Deny
            if draft.grant_id.is_none()
                && draft.approved_step_ids.is_empty()
                && draft.expires_at.is_none() =>
        {
            Ok(())
        }
        ApprovalResponseDecisionStorage::Grant
            if draft.grant_id.is_some()
                && !draft.approved_step_ids.is_empty()
                && draft.approved_step_ids.len() <= 8
                && draft.expires_at.is_some() =>
        {
            Ok(())
        }
        _ => Err(StoreError::InvalidApprovalResponse),
    }
}

fn request_leading_step(tx: &Tx<'_>, approval_id: &ApprovalId) -> Result<StepId, StoreError> {
    let value: String = tx.inner.query_row("SELECT step_id FROM approval_request_actions WHERE approval_id=?1 ORDER BY position LIMIT 1",[approval_id.as_str()],|row|row.get(0)).map_err(|_|StoreError::InvalidApprovalResponse)?;
    StepId::new(value).map_err(|_| StoreError::InvalidApprovalResponse)
}

fn validate_expired_unused_event(
    event: &EventDraft,
    grant_id: &GrantId,
    approval_id: &str,
    task_id: &str,
    step_id: &str,
    now: EpochMillis,
) -> Result<(), StoreError> {
    let p = &event.event.payload;
    let keys: Vec<&str> = p.keys().map(String::as_str).collect();
    if event.event.kind.wire_name() != "APPROVAL_EXPIRED_UNUSED"
        || event.event.data_class != DataClass::Personal
        || event.event.actor.kind != ActorKind::Host
        || event.event.occurred_at.to_epoch_millis() != now
        || event
            .event
            .correlation_id
            .as_ref()
            .is_none_or(|id| id.as_str() != task_id)
        || event
            .event
            .trace
            .as_ref()
            .and_then(|t| t.task_id.as_ref())
            .is_none_or(|id| id.as_str() != task_id)
        || event
            .event
            .trace
            .as_ref()
            .and_then(|t| t.step_id.as_ref())
            .is_none_or(|id| id.as_str() != step_id)
        || keys != ["approval_id", "grant_id", "step_id", "task_id"]
        || p.get("approval_id").and_then(Value::as_str) != Some(approval_id)
        || p.get("grant_id").and_then(Value::as_str) != Some(grant_id.as_str())
        || p.get("task_id").and_then(Value::as_str) != Some(task_id)
        || p.get("step_id").and_then(Value::as_str) != Some(step_id)
    {
        return Err(StoreError::InvalidApprovalEvent);
    }
    Ok(())
}

fn validate_response_event(
    event: &EventDraft,
    draft: &ApprovalResponseStorageDraft,
    task_id: &str,
    expected_step: &str,
    grant_id: Option<&str>,
    kind: &str,
) -> Result<(), StoreError> {
    let payload = &event.event.payload;
    if event.event.kind.wire_name() != kind
        || event.event.data_class != DataClass::Personal
        || event.event.occurred_at.to_epoch_millis() != draft.checked_at
        || event
            .event
            .correlation_id
            .as_ref()
            .is_none_or(|id| id.as_str() != task_id)
        || payload.get("approval_id").and_then(Value::as_str) != Some(draft.approval_id.as_str())
        || payload.get("task_id").and_then(Value::as_str) != Some(task_id)
        || payload.get("step_id").and_then(Value::as_str) != Some(expected_step)
        || payload.len() != 3
        || (grant_id.is_some() && draft.grant_id.as_ref().map(GrantId::as_str) != grant_id)
        || (draft.granted_by == "USER" && event.event.actor.kind != ActorKind::User)
        || (draft.granted_by == "ADMIN" && event.event.actor.kind != ActorKind::Host)
        || event.event.actor.id.as_str() != draft.actor_id.as_str()
    {
        return Err(StoreError::InvalidApprovalEvent);
    }
    Ok(())
}

fn validate_expiration_event(
    event: &EventDraft,
    approval_id: &ApprovalId,
    task_id: &str,
    step_id: &StepId,
    now: EpochMillis,
) -> Result<(), StoreError> {
    if event.event.kind.wire_name() != "APPROVAL_EXPIRED"
        || event.event.data_class != DataClass::Personal
        || event.event.actor.kind != ActorKind::Host
        || event.event.occurred_at.to_epoch_millis() != now
        || event
            .event
            .correlation_id
            .as_ref()
            .is_none_or(|id| id.as_str() != task_id)
        || event
            .event
            .trace
            .as_ref()
            .and_then(|trace| trace.task_id.as_ref())
            .is_none_or(|id| id.as_str() != task_id)
        || event
            .event
            .trace
            .as_ref()
            .and_then(|trace| trace.step_id.as_ref())
            != Some(step_id)
        || event.event.payload.len() != 3
        || event
            .event
            .payload
            .get("approval_id")
            .and_then(Value::as_str)
            != Some(approval_id.as_str())
        || event.event.payload.get("task_id").and_then(Value::as_str) != Some(task_id)
        || event.event.payload.get("step_id").and_then(Value::as_str) != Some(step_id.as_str())
    {
        return Err(StoreError::InvalidApprovalEvent);
    }
    Ok(())
}

fn validate_draft(
    tx: &Tx<'_>,
    draft: &ApprovalRequestStorageDraft,
    event: &EventDraft,
) -> Result<(), StoreError> {
    if !(1..=8).contains(&draft.actions.len())
        || draft.plan_revision <= 0
        || draft.generation_id <= 0
        || !matches!(draft.data_class, DataClass::Public | DataClass::Personal)
        || draft.risk_class == RiskClass::Credential
        || draft.authorization == Authorization::CredentialHandoff
        || draft.summary_kind.is_empty()
        || draft.summary_kind.len() > 64
        || !draft
            .summary_kind
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"._".contains(&byte))
        || ApprovalAutomationContext::derived(draft.task_kind, draft.requested_by)
            != Some(draft.automation_context)
        || draft
            .expires_at
            .get()
            .checked_sub(draft.raised_at.get())
            .is_none_or(|duration| !(1..=1_800_000).contains(&duration))
    {
        return Err(StoreError::InvalidApprovalRequest);
    }
    let mut previous: Option<&str> = None;
    let mut seen = BTreeSet::new();
    for action in &draft.actions {
        let step = action.step_id.as_str();
        if !seen.insert(step) || previous.is_some_and(|value| value.as_bytes() >= step.as_bytes()) {
            return Err(StoreError::InvalidApprovalRequest);
        }
        previous = Some(step);
        let canonical_scope =
            canonicalize(&action.scope_json).map_err(|_| StoreError::InvalidApprovalRequest)?;
        if String::from_utf8(canonical_scope).map_err(|_| StoreError::InvalidApprovalRequest)?
            != action.scope_json
            || !action.scope_json.starts_with('{')
            || action.scope_json.contains('*')
            || !(2..=4096).contains(&action.scope_json.len())
            || digest_of(&action.scope_json).map_err(|_| StoreError::InvalidApprovalRequest)?
                != action.scope_digest
        {
            return Err(StoreError::InvalidApprovalRequest);
        }
    }
    if action_set_digest(&draft.actions)? != draft.action_set_digest {
        return Err(StoreError::InvalidApprovalRequest);
    }

    let (task_kind, task_state, plan_revision, data_class, policy_class, generation): (
        String,
        String,
        i64,
        i64,
        i64,
        Option<i64>,
    ) = tx
        .inner
        .query_row(
            "SELECT kind,state,plan_revision,data_class_rank,policy_class_rank,
                    capability_registry_generation FROM tasks WHERE task_id=?1",
            [draft.task_id.as_str()],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                ))
            },
        )
        .map_err(|error| match error {
            rusqlite::Error::QueryReturnedNoRows => StoreError::TaskNotFound,
            other => other.into(),
        })?;
    if task_kind != draft.task_kind.wire_name()
        || !matches!(
            task_state.as_str(),
            "READY" | "EXECUTING" | "WAITING_APPROVAL"
        )
        || plan_revision != draft.plan_revision
        || generation != Some(draft.generation_id)
        || i64::from(draft.data_class.rank()) > data_class
        || i64::from(draft.risk_class.rank()) > policy_class
    {
        return Err(StoreError::InvalidApprovalRequest);
    }
    let pending: i64 = tx.inner.query_row(
        "SELECT count(*) FROM approval_requests WHERE task_id=?1 AND status='PENDING'",
        [draft.task_id.as_str()],
        |row| row.get(0),
    )?;
    if pending >= 5 {
        return Err(StoreError::InvalidApprovalRequest);
    }
    validate_required_event(event, draft)?;
    Ok(())
}

fn validate_required_event(
    event: &EventDraft,
    draft: &ApprovalRequestStorageDraft,
) -> Result<(), StoreError> {
    let payload = &event.event.payload;
    if event.event.kind.wire_name() != "APPROVAL_REQUIRED"
        || event.event.data_class != DataClass::Personal
        || event.event.actor.kind != ActorKind::Host
        || event.event.occurred_at.to_epoch_millis() != draft.raised_at
        || event.event.correlation_id.as_ref() != Some(&draft.task_id)
        || event
            .event
            .trace
            .as_ref()
            .and_then(|trace| trace.task_id.as_ref())
            != Some(&draft.task_id)
        || event
            .event
            .trace
            .as_ref()
            .and_then(|trace| trace.step_id.as_ref())
            != draft.actions.first().map(|action| &action.step_id)
        || payload.len() != 3
        || payload.get("approval_id").and_then(Value::as_str) != Some(draft.approval_id.as_str())
        || payload.get("task_id").and_then(Value::as_str) != Some(draft.task_id.as_str())
        || payload.get("step_id").and_then(Value::as_str)
            != draft.actions.first().map(|action| action.step_id.as_str())
        || event
            .retention_at
            .is_some_and(|retention| retention < draft.raised_at)
    {
        return Err(StoreError::InvalidApprovalEvent);
    }
    Ok(())
}

/// Computes the R2 action commitment over a normalized member slice.
pub fn action_set_digest(actions: &[ApprovalRequestActionDraft]) -> Result<Digest, StoreError> {
    if !(1..=8).contains(&actions.len()) {
        return Err(StoreError::InvalidApprovalRequest);
    }
    let values = actions
        .iter()
        .map(|action| {
            let scope: Value = serde_json::from_str(&action.scope_json)
                .map_err(|_| StoreError::InvalidApprovalRequest)?;
            Ok(json!({
                "step_id": action.step_id.as_str(),
                "arguments_digest": action.arguments_digest.as_str(),
                "scope": scope,
            }))
        })
        .collect::<Result<Vec<_>, StoreError>>()?;
    let source = serde_json::to_string(&Value::Array(values))
        .map_err(|_| StoreError::InvalidApprovalRequest)?;
    let canonical = canonicalize(&source).map_err(|_| StoreError::InvalidApprovalRequest)?;
    let canonical = String::from_utf8(canonical).map_err(|_| StoreError::InvalidApprovalRequest)?;
    digest_of(&canonical).map_err(|_| StoreError::InvalidApprovalRequest)
}

fn approval_sql_error(error: rusqlite::Error) -> StoreError {
    match error.sqlite_error_code() {
        Some(rusqlite::ErrorCode::ConstraintViolation) => StoreError::InvalidApprovalRequest,
        _ => error.into(),
    }
}
