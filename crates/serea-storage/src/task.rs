//! Whole task creation/planning operations and consistent, checked read models.

use std::collections::{BTreeMap, BTreeSet};

use rusqlite::{Connection, OptionalExtension, Row, named_params, params};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::{Map, Value};
use serea_protocol::{
    AssistantTask, DataClass, Digest, EpochMillis, Extensions, StepId, StepKind, TaskId, TaskState,
    TaskStep, TaskStepDraft, Timestamp, canonicalize, derive_idempotency_key, digest_of,
};

use crate::audit::{AuditOperation, DurableTransition};
use crate::{BlobRef, Store, StoreError, TransitionContext, Tx};

const MAX_TASK_SCHEMA_STEPS: usize = 1024;

#[derive(Debug, Clone, PartialEq)]
pub struct TaskSnapshot {
    pub task: AssistantTask,
    pub plan_revision: u32,
    /// Monotonic storage-owned revision for task state transitions. Not a wire field.
    pub state_revision: u64,
    pub steps: Vec<StepSnapshot>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct StepSnapshot {
    pub step: TaskStep,
    pub plan_revision: u32,
}

#[derive(Debug, Clone)]
pub struct StepInput {
    pub step: TaskStep,
    pub input_json: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct PlanWrite {
    pub revision: u32,
    pub steps: Vec<StepInput>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanRevisionSnapshot {
    pub revision: u32,
    pub blob: BlobRef,
    pub created_at: EpochMillis,
    pub step_count: u32,
}

// Canonical input text is retained inside the full revision, independently of
// current step refs. Removing a step must not erase its specification/history.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct PlanDocument {
    pub(crate) task_id: TaskId,
    pub(crate) revision: u32,
    pub(crate) steps: Vec<StoredInput>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct StoredInput {
    pub(crate) step: TaskStep,
    pub(crate) input_json: String,
}

const TASK_MEMBERS: &[&str] = &[
    "task_id",
    "kind",
    "title",
    "state",
    "origin",
    "data_class",
    "policy_class",
    "created_at",
    "updated_at",
    "deadline_at",
    "attempt_budget",
    "steps",
    "blocked_reason",
    "result_summary",
    "cancelled_at",
    "cancelled_by",
    "failure_reason",
];
const ORIGIN_MEMBERS: &[&str] = &["kind", "device_id", "message_id"];
const BUDGET_MEMBERS: &[&str] = &["max_model_calls", "max_tool_calls", "max_attempts_per_step"];

fn extensions_valid(ext: &Extensions, reserved: &[&str]) -> bool {
    !ext.keys().any(|key| reserved.contains(&key.as_str()))
}
fn ordinary_class(class: DataClass) -> Result<(), StoreError> {
    match class {
        DataClass::Public | DataClass::Personal => Ok(()),
        DataClass::Private => Err(StoreError::AtRestProtectionUnavailable),
        DataClass::Secret | DataClass::Credential => Err(StoreError::ClassRefused),
    }
}
pub(crate) fn task_class(conn: &Connection, task_id: &TaskId) -> Result<DataClass, StoreError> {
    let rank: i64 = conn
        .query_row(
            "SELECT data_class_rank FROM tasks WHERE task_id=?1",
            [task_id.as_str()],
            |r| r.get(0),
        )
        .optional()?
        .ok_or(StoreError::TaskNotFound)?;
    match rank {
        0 => Ok(DataClass::Public),
        1 => Ok(DataClass::Personal),
        2 => Err(StoreError::AtRestProtectionUnavailable),
        3 | 4 => Err(StoreError::ClassRefused),
        _ => Err(StoreError::CorruptRow),
    }
}
pub(crate) fn json_bytes<T: Serialize>(value: &T) -> Result<Vec<u8>, StoreError> {
    let bytes = serde_json::to_vec(value).map_err(|_| StoreError::CanonicalJson)?;
    canonicalize(std::str::from_utf8(&bytes).map_err(|_| StoreError::CanonicalJson)?)
        .map_err(|_| StoreError::CanonicalJson)
}
fn json_text<T: Serialize>(value: &T) -> Result<String, StoreError> {
    String::from_utf8(json_bytes(value)?).map_err(|_| StoreError::CanonicalJson)
}
fn decode<T: DeserializeOwned>(value: &Value) -> Result<T, StoreError> {
    serde_json::from_slice(&serde_json::to_vec(value).map_err(|_| StoreError::CorruptRow)?)
        .map_err(|_| StoreError::CorruptRow)
}
fn parse_json(text: &str) -> Result<Value, StoreError> {
    canonicalize(text).map_err(|_| StoreError::CorruptRow)?;
    serde_json::from_str(text).map_err(|_| StoreError::CorruptRow)
}
fn g<T: rusqlite::types::FromSql>(row: &Row<'_>, index: usize) -> Result<T, StoreError> {
    row.get(index).map_err(|_| StoreError::CorruptRow)
}
fn timestamp(value: i64) -> Result<Value, StoreError> {
    let epoch = EpochMillis::new(value).map_err(|_| StoreError::CorruptRow)?;
    serde_json::to_value(Timestamp::from_epoch_millis(epoch)).map_err(|_| StoreError::CorruptRow)
}
fn optional_timestamp(value: Option<i64>) -> Result<Value, StoreError> {
    value
        .map(timestamp)
        .transpose()
        .map(|v| v.unwrap_or(Value::Null))
}
pub(crate) fn one(changed: usize, error: StoreError) -> Result<(), StoreError> {
    if changed == 1 { Ok(()) } else { Err(error) }
}
fn valid_task(task: &AssistantTask) -> Result<(), StoreError> {
    if !extensions_valid(&task.extensions, TASK_MEMBERS)
        || !extensions_valid(&task.origin.extensions, ORIGIN_MEMBERS)
        || !extensions_valid(&task.attempt_budget.extensions, BUDGET_MEMBERS)
        || task.updated_at.to_epoch_millis() < task.created_at.to_epoch_millis()
        || task
            .deadline_at
            .as_ref()
            .is_some_and(|t| t.to_epoch_millis() < task.created_at.to_epoch_millis())
        || (task.state != TaskState::Blocked && task.blocked_reason.is_some())
        || (task.state == TaskState::Failed) != task.failure_reason.is_some()
        || (task.state == TaskState::Cancelled)
            != (task.cancelled_at.is_some() && task.cancelled_by.is_some())
        || (task.state != TaskState::Cancelled
            && (task.cancelled_at.is_some() || task.cancelled_by.is_some()))
        || task.cancelled_at.as_ref().is_some_and(|t| {
            t.to_epoch_millis() > task.updated_at.to_epoch_millis()
                || t.to_epoch_millis() < task.created_at.to_epoch_millis()
        })
    {
        return Err(StoreError::CorruptRow);
    }
    // Each stored JSON document is validated at its own boundary. Reapplying
    // SCJ-1 to the assembled read projection adds nesting and would reject
    // otherwise valid, independently admitted outcome details.
    Ok(())
}
fn known_status(step: &TaskStep) -> bool {
    matches!(
        step.status.as_str(),
        "PLANNED"
            | "LEASED"
            | "EXECUTING"
            | "WAITING"
            | "SUCCEEDED"
            | "FAILED"
            | "RECONCILED_ABSENT"
    )
}
fn capability_shaped(kind: StepKind) -> bool {
    matches!(
        kind,
        StepKind::Capability | StepKind::Delegate | StepKind::Verify
    )
}
pub(crate) fn input_role(kind: StepKind) -> &'static str {
    if capability_shaped(kind) {
        "ARGUMENTS"
    } else {
        "INSTRUCTION"
    }
}
pub(crate) fn validate_input(
    task: &TaskId,
    step: &TaskStep,
    bytes: &[u8],
) -> Result<String, StoreError> {
    TaskStep::new(TaskStepDraft::from(step.clone())).map_err(|_| StoreError::InvalidPlan)?;
    if &step.task_id != task || !known_status(step) {
        return Err(StoreError::InvalidPlan);
    }
    let original = std::str::from_utf8(bytes).map_err(|_| StoreError::CanonicalJson)?;
    let canonical = canonicalize(original).map_err(|_| StoreError::CanonicalJson)?;
    if digest_of(original).map_err(|_| StoreError::CanonicalJson)? != step.input_digest {
        return Err(StoreError::InvalidPlan);
    }
    if capability_shaped(step.kind) {
        if canonical.first() != Some(&b'{') {
            return Err(StoreError::InvalidPlan);
        }
        let key = derive_idempotency_key(
            task,
            &step.step_id,
            step.capability_id.as_ref().ok_or(StoreError::InvalidPlan)?,
            step.capability_version
                .as_ref()
                .ok_or(StoreError::InvalidPlan)?,
            original,
        )
        .map_err(|_| StoreError::CanonicalJson)?;
        if step.idempotency_key.as_ref() != Some(&key) {
            return Err(StoreError::InvalidPlan);
        }
    }
    String::from_utf8(canonical).map_err(|_| StoreError::CanonicalJson)
}
fn validate_membership(steps: &[StoredInput]) -> Result<(), StoreError> {
    let mut ids = BTreeSet::new();
    let mut sequences = BTreeSet::new();
    let mut keys = BTreeSet::new();
    let mut verify = false;
    let mut ordinary = false;
    for input in steps {
        let step = &input.step;
        if !ids.insert(step.step_id.as_str()) {
            return Err(StoreError::DuplicateStepId);
        }
        if !sequences.insert(step.sequence) {
            return Err(StoreError::DuplicateSequence);
        }
        if let Some(key) = &step.idempotency_key {
            if !keys.insert(key.as_str()) {
                return Err(StoreError::DuplicateIdempotencyKey);
            }
        }
        if step.kind == StepKind::Verify {
            verify = true;
        } else {
            if verify {
                return Err(StoreError::InvalidPlanLayout);
            }
            ordinary = true;
        }
    }
    if !ordinary {
        return Err(StoreError::InvalidPlanLayout);
    }
    Ok(())
}
fn immutable_equal(a: &TaskStep, b: &TaskStep) -> bool {
    a.step_id == b.step_id
        && a.task_id == b.task_id
        && a.sequence == b.sequence
        && a.kind == b.kind
        && a.provider_id == b.provider_id
        && a.capability_id == b.capability_id
        && a.capability_version == b.capability_version
        && a.idempotency_key == b.idempotency_key
        && a.input_digest == b.input_digest
        && a.extensions == b.extensions
}

impl Store {
    /// Loads a checked projection in one deferred read snapshot. No connection
    /// policy/PRAGMA is changed, and no write transaction is acquired.
    pub fn load_task(&self, task_id: &TaskId) -> Result<TaskSnapshot, StoreError> {
        let mut conn = self.conn.lock().map_err(|_| StoreError::LockPoisoned)?;
        let read = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Deferred)?;
        let result = load(&read, task_id);
        read.rollback()?;
        result
    }
}
impl Tx<'_> {
    pub fn load_task(&self, task_id: &TaskId) -> Result<TaskSnapshot, StoreError> {
        self.ensure_active()?;
        load(&self.inner, task_id)
    }

    /// Inserts an observable RECEIVED task, never an implicit initial plan.
    pub fn insert_task(
        &mut self,
        task: &AssistantTask,
        context: &TransitionContext<'_>,
    ) -> Result<TaskSnapshot, StoreError> {
        self.operation_savepoint(|tx| {
            tx.require_audit()?; ordinary_class(task.data_class)?; valid_task(task)?;
            if task.state != TaskState::Received || !task.steps.is_empty()
                || task.created_at.to_epoch_millis() != task.updated_at.to_epoch_millis()
                || task.blocked_reason.is_some() || task.result_summary.is_some()
                || task.failure_reason.is_some() || task.cancelled_at.is_some() || task.cancelled_by.is_some()
            { return Err(StoreError::InvalidPlan); }
            let origin = json_text(&task.origin.extensions)?;
            let budget = json_text(&task.attempt_budget.extensions)?;
            let extensions = json_text(&task.extensions)?;
            let exists: bool = tx.inner.query_row("SELECT EXISTS(SELECT 1 FROM tasks WHERE task_id=?1)", [task.task_id.as_str()], |r| r.get(0))?;
            if exists { return Err(StoreError::TaskExists); }
            one(tx.inner.execute(
                "INSERT INTO tasks(task_id,kind,title,state,origin_kind,origin_device_id,origin_message_id,origin_extensions,
                  data_class_rank,policy_class_rank,created_at_ms,updated_at_ms,deadline_at_ms,max_model_calls,max_tool_calls,
                  max_attempts_per_step,budget_extensions,extensions) VALUES
                  (:id,:kind,:title,'RECEIVED',:origin,:device,:message,:origin_ext,:class,:risk,:now,:now,:deadline,:models,:tools,:attempts,:budget,:ext)",
                named_params! { ":id":task.task_id.as_str(), ":kind":task.kind.wire_name(), ":title":task.title.as_str(),
                    ":origin":task.origin.kind.as_str(), ":device":task.origin.device_id.as_ref().map(|v|v.as_str()),
                    ":message":task.origin.message_id.as_ref().map(|v|v.as_str()), ":origin_ext":origin,
                    ":class":task.data_class.rank(), ":risk":task.policy_class.rank(), ":now":task.created_at.to_epoch_millis().get(),
                    ":deadline":task.deadline_at.as_ref().map(|v|v.to_epoch_millis().get()), ":models":task.attempt_budget.max_model_calls,
                    ":tools":task.attempt_budget.max_tool_calls, ":attempts":task.attempt_budget.max_attempts_per_step,
                    ":budget":budget, ":ext":extensions },
            )?, StoreError::ConstraintViolation)?;
            // P2H N3: the task row exists but its audit/journal row does not.
            // Dying here proves task and journal share one transaction.
            #[cfg(feature = "p2h-fault-injection")]
            crate::fault::reach(crate::fault::Window::AfterTaskInsert)?;
            let snapshot = tx.load_task(&task.task_id)?;
            tx.record_transition(&DurableTransition::task(AuditOperation::TaskInserted, &task.task_id, None,
                TaskState::Received, task.data_class, task.created_at.to_epoch_millis(), context))?;
            Ok(snapshot)
        })
    }

    pub fn start_planning(
        &mut self,
        task_id: &TaskId,
        expected_state: TaskState,
        expected_revision: u32,
        now: EpochMillis,
        context: &TransitionContext<'_>,
    ) -> Result<TaskSnapshot, StoreError> {
        self.operation_savepoint(|tx| {
            tx.require_audit()?;
            if !matches!(
                expected_state,
                TaskState::Received
                    | TaskState::Ready
                    | TaskState::WaitingUser
                    | TaskState::Blocked
            ) {
                return Err(StoreError::IllegalTaskTransition);
            }
            let before = tx.load_task(task_id)?;
            ordinary_class(before.task.data_class)?;
            if before.task.state != expected_state || before.plan_revision != expected_revision {
                return Err(StoreError::IllegalTaskTransition);
            }
            if now < before.task.updated_at.to_epoch_millis() {
                return Err(StoreError::InvalidTimestamp);
            }
            one(
                tx.inner.execute(
                    "UPDATE tasks SET state='PLANNING',updated_at_ms=?1,blocked_reason=NULL
                WHERE task_id=?2 AND state=?3 AND plan_revision=?4 AND updated_at_ms<=?1",
                    params![
                        now.get(),
                        task_id.as_str(),
                        expected_state.wire_name(),
                        expected_revision
                    ],
                )?,
                StoreError::IllegalTaskTransition,
            )?;
            let mut facts = DurableTransition::task(
                AuditOperation::PlanningStarted,
                task_id,
                Some(expected_state),
                TaskState::Planning,
                before.task.data_class,
                now,
                context,
            );
            facts.revision = Some(expected_revision);
            let snapshot = tx.load_task(task_id)?;
            tx.record_transition(&facts)?;
            Ok(snapshot)
        })
    }

    pub fn put_plan_revision(
        &mut self,
        task_id: &TaskId,
        plan: PlanWrite,
        now: EpochMillis,
        context: &TransitionContext<'_>,
    ) -> Result<PlanRevisionSnapshot, StoreError> {
        self.operation_savepoint(|tx| {
            tx.require_audit()?;
            let class = task_class(&tx.inner, task_id)?;
            let (state, revision): (String, u32) = tx.inner.query_row(
                "SELECT state,plan_revision FROM tasks WHERE task_id=?1", [task_id.as_str()],
                |r| Ok((r.get(0)?, r.get(1)?)))?;
            if state != TaskState::Planning.wire_name() { return Err(StoreError::IllegalTaskTransition); }
            // Arithmetic refusal precedes provenance reconstruction: an unrepresentable
            // next revision must not require allocating/reading billions of revisions.
            revision.checked_add(1).ok_or(StoreError::PlanRevisionOverflow)?;
            if plan.steps.len() > MAX_TASK_SCHEMA_STEPS {
                return Err(StoreError::InvalidPlan);
            }
            let before = tx.load_task(task_id)?;
            ordinary_class(class)?;
            let next = before.plan_revision.checked_add(1).ok_or(StoreError::PlanRevisionOverflow)?;
            if plan.revision != next { return Err(StoreError::PlanRevisionConflict); }
            if now < before.task.updated_at.to_epoch_millis() { return Err(StoreError::InvalidTimestamp); }
            let mut inputs = plan.steps.into_iter().map(|input| {
                let text = validate_input(task_id, &input.step, &input.input_json)?;
                Ok(StoredInput { step: input.step, input_json: text })
            }).collect::<Result<Vec<_>,StoreError>>()?;
            inputs.sort_by_key(|s|s.step.sequence); validate_membership(&inputs)?;
            // PLANNING can publish only READY. Preserve retained runtime, but do
            // not strand an in-flight attempt or a verify-only remaining suffix
            // in a task state from which the frozen outcome paths cannot advance.
            if inputs.iter().any(|s| !matches!(s.step.status.as_str(), "PLANNED" | "LEASED" | "SUCCEEDED"))
                || !inputs.iter().any(|s| s.step.kind != StepKind::Verify && s.step.status.as_str() != "SUCCEEDED")
            { return Err(StoreError::InvalidPlan); }
            let step_count = u32::try_from(inputs.len()).map_err(|_|StoreError::InvalidPlan)?;
            let history = load_history(&tx.inner, task_id, before.plan_revision, class)?;
            let historical_ids: BTreeSet<&str> = history.iter().flat_map(|p|p.steps.iter().map(|s|s.step.step_id.as_str())).collect();
            let high_water = history.iter().flat_map(|p|p.steps.iter().map(|s|s.step.sequence)).max();
            let current: BTreeMap<&str,&StepSnapshot> = before.steps.iter().map(|s|(s.step.step_id.as_str(),s)).collect();
            let wanted: BTreeSet<&str> = inputs.iter().map(|s|s.step.step_id.as_str()).collect();
            let mut removed = Vec::new();
            for old in &before.steps {
                if !wanted.contains(old.step.step_id.as_str()) {
                    let leased: bool = tx.inner.query_row("SELECT EXISTS(SELECT 1 FROM leases WHERE step_id=?1)", [old.step.step_id.as_str()], |r|r.get(0))?;
                    if old.step.status.as_str() != "PLANNED" || old.step.attempt != 0 || old.step.lease_generation.is_some() || leased {
                        return Err(StoreError::PlanRevisionWouldDropExecutedStep);
                    }
                    removed.push(old.step.step_id.clone());
                }
            }
            for input in &inputs {
                if let Some(old) = current.get(input.step.step_id.as_str()) {
                    if old.step != input.step { return Err(StoreError::InvalidPlan); }
                } else {
                    let occupied: bool = tx.inner.query_row("SELECT EXISTS(SELECT 1 FROM task_steps WHERE step_id=?1)", [input.step.step_id.as_str()], |r|r.get(0))?;
                    if occupied || historical_ids.contains(input.step.step_id.as_str())
                        || high_water.is_some_and(|max|input.step.sequence<=max)
                        || input.step.status.as_str() != "PLANNED" || input.step.attempt != 0 || input.step.lease_generation.is_some()
                    { return Err(StoreError::InvalidPlan); }
                }
            }
            let candidates = removed.iter().map(|id| step_blob_candidates(&tx.inner,id)).collect::<Result<Vec<_>,_>>()?.into_iter().flatten().collect::<Vec<_>>();
            let document = PlanDocument { task_id: task_id.clone(), revision: next, steps: inputs };
            let plan_json = json_bytes(&document)?;
            // No data writes precede the complete validation above.
            let blob = tx.put_blob(&plan_json,class)?;
            one(tx.inner.execute("INSERT INTO plan_revisions(task_id,plan_revision,created_at_ms,plan_digest,data_class_rank,step_count)
                VALUES(?1,?2,?3,?4,?5,?6)",params![task_id.as_str(),next,now.get(),blob.digest().as_str(),class.rank(),step_count])?,StoreError::ConstraintViolation)?;
            let deleted = tx.inner.execute("DELETE FROM task_blob_refs WHERE task_id=?1 AND role='PLAN'",[task_id.as_str()])?;
            if deleted != usize::from(before.plan_revision != 0) { return Err(StoreError::ConstraintViolation); }
            for role in ["PLAN","PLAN_REVISION"] {
                one(tx.inner.execute("INSERT INTO task_blob_refs(task_id,role,digest,data_class_rank) VALUES(?1,?2,?3,?4)",
                    params![task_id.as_str(),role,blob.digest().as_str(),class.rank()])?,StoreError::ConstraintViolation)?;
            }
            for id in &removed {
                one(tx.inner.execute("DELETE FROM task_steps WHERE step_id=?1 AND task_id=?2 AND status='PLANNED' AND attempt=0 AND lease_generation=0
                    AND NOT EXISTS(SELECT 1 FROM leases WHERE leases.step_id=task_steps.step_id)",params![id.as_str(),task_id.as_str()])?,StoreError::PlanRevisionWouldDropExecutedStep)?;
            }
            for input in &document.steps {
                if current.contains_key(input.step.step_id.as_str()) { continue; }
                let s=&input.step;
                let input_blob=tx.put_blob(input.input_json.as_bytes(),class)?;
                one(tx.inner.execute("INSERT INTO task_steps(step_id,task_id,sequence,kind,status,attempt,plan_revision,
                    provider_id,capability_id,capability_version,idempotency_key,input_digest)
                    VALUES(?1,?2,?3,?4,'PLANNED',0,?5,?6,?7,?8,?9,?10)",
                    params![s.step_id.as_str(),task_id.as_str(),s.sequence,s.kind.wire_name(),next,
                        s.provider_id.as_ref().map(|v|v.as_str()),s.capability_id.as_ref().map(|v|v.as_str()),
                        s.capability_version.as_ref().map(|v|v.as_str()),s.idempotency_key.as_ref().map(|v|v.as_str()),s.input_digest.as_str()])?,StoreError::ConstraintViolation)?;
                one(tx.inner.execute("INSERT INTO step_blob_refs(step_id,role,digest,data_class_rank) VALUES(?1,?2,?3,?4)",
                    params![s.step_id.as_str(),input_role(s.kind),input_blob.digest().as_str(),class.rank()])?,StoreError::ConstraintViolation)?;
            }
            sweep_blob_candidates(&tx.inner,&candidates)?;
            one(tx.inner.execute("UPDATE tasks SET plan_revision=?1,state='READY',updated_at_ms=?2,blocked_reason=NULL
                WHERE task_id=?3 AND state='PLANNING' AND plan_revision=?4 AND updated_at_ms<=?2",
                params![next,now.get(),task_id.as_str(),before.plan_revision])?,StoreError::PlanRevisionConflict)?;
            let mut facts = DurableTransition::task(AuditOperation::PlanPersisted,task_id,Some(TaskState::Planning),TaskState::Ready,class,now,context);
            facts.revision=Some(next); tx.record_transition(&facts)?;
            Ok(PlanRevisionSnapshot {revision:next,blob,created_at:now,step_count})
        })
    }
}

fn load(conn: &Connection, task_id: &TaskId) -> Result<TaskSnapshot, StoreError> {
    task_class(conn, task_id)?;
    let (mut task,revision,state_revision) = conn.query_row("SELECT task_id,kind,title,state,origin_kind,origin_device_id,origin_message_id,origin_extensions,
        data_class_rank,policy_class_rank,created_at_ms,updated_at_ms,deadline_at_ms,blocked_reason,result_summary,cancelled_at_ms,
        cancelled_by,failure_reason,max_model_calls,max_tool_calls,max_attempts_per_step,budget_extensions,extensions,plan_revision,state_revision
        FROM tasks WHERE task_id=?1",[task_id.as_str()],|r|Ok(task_row(r))).optional()?.ok_or(StoreError::TaskNotFound)??;
    let history = load_history(conn, task_id, revision, task.data_class)?;
    let mut statement=conn.prepare("SELECT step_id,task_id,sequence,kind,status,attempt,plan_revision,provider_id,capability_id,capability_version,
        idempotency_key,input_digest,result_digest,started_at_ms,completed_at_ms,lease_owner,lease_expires_at_ms,lease_generation,
        error_kind,error_code,error_message,error_retryable,error_host_action,error_details FROM task_steps WHERE task_id=?1 ORDER BY sequence")?;
    let rows = statement
        .query_map([task_id.as_str()], |r| Ok(step_row(r)))?
        .collect::<Result<Vec<_>, _>>()?;
    let mut steps = Vec::new();
    for row in rows {
        let (mut value, original_revision) = row?;
        let id = value
            .get("step_id")
            .and_then(Value::as_str)
            .ok_or(StoreError::CorruptRow)?;
        let origin = history
            .iter()
            .find_map(|p| {
                p.steps
                    .iter()
                    .find(|s| s.step.step_id.as_str() == id)
                    .map(|s| (p.revision, s))
            })
            .ok_or(StoreError::CorruptRow)?;
        if origin.0 != original_revision {
            return Err(StoreError::CorruptRow);
        }
        let object = value.as_object_mut().ok_or(StoreError::CorruptRow)?;
        for (key, v) in &origin.1.step.extensions {
            object.insert(key.clone(), v.clone());
        }
        let receipt = load_receipt(conn, id_from_value(object)?, task_id, task.data_class)?;
        object.insert("side_effect_receipt".into(), receipt);
        let step: TaskStep = decode(&value)?;
        if step.side_effect_receipt.as_ref().is_some_and(|receipt| {
            step.capability_id.as_ref() != Some(&receipt.capability_id)
                || step.idempotency_key.as_ref() != Some(&receipt.idempotency_key)
        }) {
            return Err(StoreError::CorruptRow);
        }
        if !known_status(&step)
            || !immutable_equal(&step, &origin.1.step)
            || step
                .completed_at
                .as_ref()
                .zip(step.started_at.as_ref())
                .is_some_and(|(end, start)| end.to_epoch_millis() < start.to_epoch_millis())
        {
            return Err(StoreError::CorruptRow);
        }
        let refs:Vec<(String,String,u8)>=conn.prepare("SELECT role,digest,data_class_rank FROM step_blob_refs WHERE step_id=?1 AND role IN ('ARGUMENTS','INSTRUCTION')")?
            .query_map([step.step_id.as_str()],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?)))?
            .collect::<Result<_,_>>().map_err(|_|StoreError::CorruptRow)?;
        if refs
            != vec![(
                input_role(step.kind).into(),
                step.input_digest.as_str().into(),
                task.data_class.rank(),
            )]
        {
            return Err(StoreError::CorruptRow);
        }
        let bytes = read_plan_blob(conn, &step.input_digest, task.data_class)?;
        if bytes != origin.1.input_json.as_bytes() {
            return Err(StoreError::CorruptRow);
        }
        if let Some(digest) = &step.result_digest {
            let count:i64=conn.query_row("SELECT count(*) FROM step_blob_refs WHERE step_id=?1 AND role='RESULT' AND digest=?2 AND data_class_rank=?3",
                params![step.step_id.as_str(),digest.as_str(),task.data_class.rank()],|r|r.get(0))?;
            if count != 1 {
                return Err(StoreError::CorruptRow);
            }
            read_plan_blob(conn, digest, task.data_class)?;
        }
        steps.push(StepSnapshot {
            step,
            plan_revision: original_revision,
        });
    }
    let actual: BTreeSet<&str> = steps.iter().map(|s| s.step.step_id.as_str()).collect();
    let desired: BTreeSet<&str> = history
        .last()
        .map(|p| p.steps.iter().map(|s| s.step.step_id.as_str()).collect())
        .unwrap_or_default();
    if actual != desired {
        return Err(StoreError::CorruptRow);
    }
    task.steps = steps.iter().map(|s| s.step.clone()).collect();
    valid_task(&task)?;
    Ok(TaskSnapshot {
        task,
        plan_revision: revision,
        state_revision,
        steps,
    })
}
fn id_from_value(object: &Map<String, Value>) -> Result<&str, StoreError> {
    object
        .get("step_id")
        .and_then(Value::as_str)
        .ok_or(StoreError::CorruptRow)
}
fn task_row(r: &Row<'_>) -> Result<(AssistantTask, u32, u64), StoreError> {
    let mut m = Map::new();
    for (i, key) in [(0, "task_id"), (1, "kind"), (2, "title"), (3, "state")] {
        m.insert(key.into(), Value::String(g(r, i)?));
    }
    let mut origin = Map::new();
    origin.insert("kind".into(), Value::String(g(r, 4)?));
    for (i, key) in [(5, "device_id"), (6, "message_id")] {
        origin.insert(key.into(), serde_json::json!(g::<Option<String>>(r, i)?));
    }
    let origin_ext: Extensions = decode(&parse_json(&g::<String>(r, 7)?)?)?;
    if !extensions_valid(&origin_ext, ORIGIN_MEMBERS) {
        return Err(StoreError::CorruptRow);
    }
    origin.extend(origin_ext);
    m.insert("origin".into(), Value::Object(origin));
    let class = match g::<u8>(r, 8)? {
        0 => "PUBLIC",
        1 => "PERSONAL",
        2 => "PRIVATE",
        _ => return Err(StoreError::CorruptRow),
    };
    let risk = match g::<u8>(r, 9)? {
        0 => "OBSERVE",
        1 => "LOCAL_STATE",
        2 => "REVERSIBLE_WRITE",
        3 => "EXTERNAL_WRITE",
        4 => "COMMUNICATION",
        5 => "ELEVATED_DEVICE",
        6 => "DESTRUCTIVE",
        7 => "CREDENTIAL",
        _ => return Err(StoreError::CorruptRow),
    };
    m.insert("data_class".into(), Value::String(class.into()));
    m.insert("policy_class".into(), Value::String(risk.into()));
    for (i, key) in [(10, "created_at"), (11, "updated_at")] {
        m.insert(key.into(), timestamp(g(r, i)?)?);
    }
    for (i, key) in [(12, "deadline_at"), (15, "cancelled_at")] {
        m.insert(key.into(), optional_timestamp(g(r, i)?)?);
    }
    for (i, key) in [
        (13, "blocked_reason"),
        (14, "result_summary"),
        (16, "cancelled_by"),
        (17, "failure_reason"),
    ] {
        m.insert(key.into(), serde_json::json!(g::<Option<String>>(r, i)?));
    }
    let mut budget = Map::new();
    for (i, key) in [
        (18, "max_model_calls"),
        (19, "max_tool_calls"),
        (20, "max_attempts_per_step"),
    ] {
        budget.insert(key.into(), serde_json::json!(g::<u32>(r, i)?));
    }
    let budget_ext: Extensions = decode(&parse_json(&g::<String>(r, 21)?)?)?;
    if !extensions_valid(&budget_ext, BUDGET_MEMBERS) {
        return Err(StoreError::CorruptRow);
    }
    budget.extend(budget_ext);
    m.insert("attempt_budget".into(), Value::Object(budget));
    let ext: Extensions = decode(&parse_json(&g::<String>(r, 22)?)?)?;
    if !extensions_valid(&ext, TASK_MEMBERS) {
        return Err(StoreError::CorruptRow);
    }
    m.extend(ext);
    m.insert("steps".into(), Value::Array(vec![]));
    let task = decode(&Value::Object(m))?;
    valid_task(&task)?;
    let state_revision = u64::try_from(g::<i64>(r, 24)?).map_err(|_| StoreError::CorruptRow)?;
    if state_revision == 0 {
        return Err(StoreError::CorruptRow);
    }
    Ok((task, g(r, 23)?, state_revision))
}
fn step_row(r: &Row<'_>) -> Result<(Value, u32), StoreError> {
    let mut m = Map::new();
    for (i, key) in [
        (0, "step_id"),
        (1, "task_id"),
        (3, "kind"),
        (4, "status"),
        (11, "input_digest"),
    ] {
        m.insert(key.into(), Value::String(g(r, i)?));
    }
    for (i, key) in [(2, "sequence"), (5, "attempt")] {
        m.insert(key.into(), serde_json::json!(g::<u32>(r, i)?));
    }
    for (i, key) in [
        (7, "provider_id"),
        (8, "capability_id"),
        (9, "capability_version"),
        (10, "idempotency_key"),
        (12, "result_digest"),
        (15, "lease_owner"),
    ] {
        m.insert(key.into(), serde_json::json!(g::<Option<String>>(r, i)?));
    }
    for (i, key) in [
        (13, "started_at"),
        (14, "completed_at"),
        (16, "lease_expires_at"),
    ] {
        m.insert(key.into(), optional_timestamp(g(r, i)?)?);
    }
    let generation: u32 = g(r, 17)?;
    m.insert(
        "lease_generation".into(),
        if generation == 0 {
            Value::Null
        } else {
            serde_json::json!(generation)
        },
    );
    let mut error = Map::new();
    let mut any = false;
    for (i, key) in [
        (18, "kind"),
        (19, "code"),
        (20, "message"),
        (22, "host_action"),
    ] {
        let v: Option<String> = g(r, i)?;
        any |= v.is_some();
        error.insert(key.into(), serde_json::json!(v));
    }
    let retry: Option<i64> = g(r, 21)?;
    any |= retry.is_some();
    error.insert(
        "retryable".into(),
        match retry {
            None => Value::Null,
            Some(0) => Value::Bool(false),
            Some(1) => Value::Bool(true),
            _ => return Err(StoreError::CorruptRow),
        },
    );
    let details: Option<String> = g(r, 23)?;
    any |= details.is_some();
    let details = details.map(|s| parse_json(&s)).transpose()?;
    if details.as_ref().is_some_and(|v| !v.is_object()) {
        return Err(StoreError::CorruptRow);
    }
    if let Some(details) = details {
        error.insert("details".into(), details);
    }
    m.insert(
        "error".into(),
        if any {
            Value::Object(error)
        } else {
            Value::Null
        },
    );
    Ok((Value::Object(m), g(r, 6)?))
}
fn load_receipt(
    conn: &Connection,
    step_id: &str,
    task_id: &TaskId,
    class: DataClass,
) -> Result<Value, StoreError> {
    conn.query_row("SELECT receipt_id,capability_id,idempotency_key,provider_reference,effect_summary,observed_at_ms,replay_safe,task_id,data_class_rank
        FROM side_effect_receipts WHERE step_id=?1",[step_id],|r|Ok((|| {
            if g::<String>(r,7)?!=task_id.as_str() || g::<u8>(r,8)?!=class.rank(){return Err(StoreError::CorruptRow);}
            let mut m=Map::new();
            for (i,key) in [(0,"receipt_id"),(1,"capability_id"),(2,"idempotency_key"),(4,"effect_summary")] {m.insert(key.into(),Value::String(g(r,i)?));}
            m.insert("provider_reference".into(),serde_json::json!(g::<Option<String>>(r,3)?));m.insert("observed_at".into(),timestamp(g(r,5)?)?);
            m.insert("replay_safe".into(),match g::<i64>(r,6)?{0=>Value::Bool(false),1=>Value::Bool(true),_=>return Err(StoreError::CorruptRow)});
            Ok(Value::Object(m))
        })())).optional()?.transpose().map(|v|v.unwrap_or(Value::Null))
}
fn read_plan_blob(
    conn: &Connection,
    digest: &Digest,
    class: DataClass,
) -> Result<Vec<u8>, StoreError> {
    // Ordinary task provenance has no protected representation. Never reinterpret
    // protected content as plaintext even if a blob backend is installed.
    if !matches!(class, DataClass::Public | DataClass::Personal) {
        return Err(StoreError::CorruptRow);
    }
    let (content,marker,size)=conn.query_row("SELECT content,protection,size_bytes FROM blobs WHERE digest=?1 AND data_class_rank=?2",
        params![digest.as_str(),class.rank()],|r|Ok((||Ok::<_,StoreError>((g::<Vec<u8>>(r,0)?,g::<String>(r,1)?,g::<i64>(r,2)?)))()))
        .optional()?.ok_or(StoreError::CorruptRow)??;
    if marker != "NONE" || i64::try_from(content.len()).ok() != Some(size) {
        return Err(StoreError::CorruptRow);
    }
    let text = std::str::from_utf8(&content).map_err(|_| StoreError::CorruptRow)?;
    let canonical = canonicalize(text).map_err(|_| StoreError::CorruptRow)?;
    if canonical != content || digest_of(text).map_err(|_| StoreError::CorruptRow)? != *digest {
        return Err(StoreError::CorruptRow);
    }
    Ok(content)
}
fn load_history(
    conn: &Connection,
    task_id: &TaskId,
    current: u32,
    class: DataClass,
) -> Result<Vec<PlanDocument>, StoreError> {
    let mut stmt=conn.prepare("SELECT plan_revision,plan_digest,data_class_rank,step_count,created_at_ms FROM plan_revisions WHERE task_id=?1 ORDER BY plan_revision")?;
    let rows = stmt
        .query_map([task_id.as_str()], |r| {
            Ok((|| {
                Ok::<_, StoreError>((
                    g::<u32>(r, 0)?,
                    g::<String>(r, 1)?,
                    g::<u8>(r, 2)?,
                    g::<u32>(r, 3)?,
                    g::<i64>(r, 4)?,
                ))
            })())
        })?
        .collect::<Result<Vec<_>, _>>()?;
    if u32::try_from(rows.len()).ok() != Some(current) {
        return Err(StoreError::CorruptRow);
    }
    let mut history: Vec<PlanDocument> = Vec::new();
    let mut seen: BTreeMap<String, (u32, TaskStep)> = BTreeMap::new();
    let mut previous = BTreeSet::new();
    let mut high_water = None;
    let mut previous_time = None;
    for row in rows {
        let (revision, digest, rank, count, created) = row?;
        if revision
            != u32::try_from(history.len())
                .map_err(|_| StoreError::CorruptRow)?
                .checked_add(1)
                .ok_or(StoreError::CorruptRow)?
            || rank != class.rank()
            || previous_time.is_some_and(|t| created < t)
        {
            return Err(StoreError::CorruptRow);
        }
        EpochMillis::new(created).map_err(|_| StoreError::CorruptRow)?;
        previous_time = Some(created);
        let digest = Digest::new(digest).map_err(|_| StoreError::CorruptRow)?;
        let bytes = read_plan_blob(conn, &digest, class)?;
        let doc: PlanDocument =
            serde_json::from_slice(&bytes).map_err(|_| StoreError::CorruptRow)?;
        if doc.task_id != *task_id
            || doc.revision != revision
            || u32::try_from(doc.steps.len()).ok() != Some(count)
            || doc
                .steps
                .windows(2)
                .any(|p| p[0].step.sequence >= p[1].step.sequence)
        {
            return Err(StoreError::CorruptRow);
        }
        validate_membership(&doc.steps).map_err(|_| StoreError::CorruptRow)?;
        let mut membership = BTreeSet::new();
        let prior_max = high_water;
        for input in &doc.steps {
            let text = validate_input(task_id, &input.step, input.input_json.as_bytes())
                .map_err(|_| StoreError::CorruptRow)?;
            if text != input.input_json {
                return Err(StoreError::CorruptRow);
            }
            let id = input.step.step_id.as_str();
            membership.insert(id.to_owned());
            if let Some((_, original)) = seen.get(id) {
                if !previous.contains(id) || !immutable_equal(original, &input.step) {
                    return Err(StoreError::CorruptRow);
                }
            } else {
                if input.step.status.as_str() != "PLANNED"
                    || prior_max.is_some_and(|max| input.step.sequence <= max)
                {
                    return Err(StoreError::CorruptRow);
                }
                seen.insert(id.to_owned(), (revision, input.step.clone()));
            }
            high_water = Some(
                high_water.map_or(input.step.sequence, |max: u32| max.max(input.step.sequence)),
            );
        }
        previous = membership;
        let refs:i64=conn.query_row("SELECT count(*) FROM task_blob_refs WHERE task_id=?1 AND role='PLAN_REVISION' AND digest=?2 AND data_class_rank=?3",
            params![task_id.as_str(),digest.as_str(),class.rank()],|r|r.get(0))?;
        if refs != 1 {
            return Err(StoreError::CorruptRow);
        }
        if revision == current {
            let refs:Vec<(String,u8)>=conn.prepare("SELECT digest,data_class_rank FROM task_blob_refs WHERE task_id=?1 AND role='PLAN'")?
                .query_map([task_id.as_str()],|r|Ok((r.get(0)?,r.get(1)?)))?.collect::<Result<_,_>>().map_err(|_|StoreError::CorruptRow)?;
            if refs != vec![(digest.as_str().into(), class.rank())] {
                return Err(StoreError::CorruptRow);
            }
        }
        history.push(doc);
    }
    if current == 0 {
        let refs: i64 = conn.query_row(
            "SELECT count(*) FROM task_blob_refs WHERE task_id=?1",
            [task_id.as_str()],
            |r| r.get(0),
        )?;
        if refs != 0 {
            return Err(StoreError::CorruptRow);
        }
    }
    Ok(history)
}

/// Exact blob identities captured before a parent cascade; no global GC.
pub(crate) type BlobCandidate = (String, u8);
pub(crate) fn step_blob_candidates(
    conn: &Connection,
    step_id: &StepId,
) -> Result<Vec<BlobCandidate>, StoreError> {
    Ok(conn
        .prepare("SELECT digest,data_class_rank FROM step_blob_refs WHERE step_id=?1")?
        .query_map([step_id.as_str()], |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect::<Result<_, _>>()?)
}
pub(crate) fn task_blob_candidates(
    conn: &Connection,
    task_id: &TaskId,
) -> Result<Vec<BlobCandidate>, StoreError> {
    Ok(conn.prepare("SELECT digest,data_class_rank FROM task_blob_refs WHERE task_id=?1
        UNION SELECT r.digest,r.data_class_rank FROM step_blob_refs r JOIN task_steps s ON s.step_id=r.step_id WHERE s.task_id=?1
        UNION SELECT plan_digest,data_class_rank FROM plan_revisions WHERE task_id=?1
        UNION SELECT response_blob_digest,response_data_class_rank FROM model_call_attempts
          WHERE task_id=?1 AND response_blob_digest IS NOT NULL")?
        .query_map([task_id.as_str()],|r|Ok((r.get(0)?,r.get(1)?)))?.collect::<Result<_,_>>()?)
}
pub(crate) fn sweep_blob_candidates(
    conn: &Connection,
    candidates: &[BlobCandidate],
) -> Result<u64, StoreError> {
    let mut removed = 0;
    for (digest, rank) in candidates {
        removed+=u64::try_from(conn.execute("DELETE FROM blobs WHERE digest=?1 AND data_class_rank=?2
            AND NOT EXISTS(SELECT 1 FROM task_blob_refs WHERE digest=?1 AND data_class_rank=?2)
            AND NOT EXISTS(SELECT 1 FROM step_blob_refs WHERE digest=?1 AND data_class_rank=?2)
            AND NOT EXISTS(SELECT 1 FROM plan_revisions WHERE plan_digest=?1 AND data_class_rank=?2)
            AND NOT EXISTS(SELECT 1 FROM schedules WHERE template_digest=?1 AND template_data_class_rank=?2)
            AND NOT EXISTS(SELECT 1 FROM schedule_occurrences WHERE template_digest=?1 AND template_data_class_rank=?2)
            AND NOT EXISTS(SELECT 1 FROM model_call_attempts WHERE response_blob_digest=?1 AND response_data_class_rank=?2)",params![digest,rank])?)
            .map_err(|_|StoreError::Sqlite)?;
    }
    Ok(removed)
}

#[cfg(test)]
#[path = "task_tests.rs"]
mod task_tests;
