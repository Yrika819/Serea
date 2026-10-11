use serea_protocol::{
    ApprovalId, AssistantTask, AttemptBudget, CapabilityId, DataClass, EpochMillis, Extensions,
    RequestedBy, RiskClass, StepId, TaskId, TaskKind, TaskOrigin, TaskState, TaskStep, TaskTitle,
};
use serea_storage::{PlanRevisionSnapshot, TaskSnapshot};

/// Host-supplied creation data. There is no initial state or runtime authority.
#[derive(Clone)]
pub struct NewTask {
    pub task_id: TaskId,
    pub kind: TaskKind,
    pub title: TaskTitle,
    pub origin: TaskOrigin,
    pub data_class: DataClass,
    pub policy_class: RiskClass,
    pub attempt_budget: AttemptBudget,
    pub created_at: EpochMillis,
    pub deadline_at: Option<EpochMillis>,
    pub extensions: Extensions,
}

/// Owned read projection, never a mutable reference into persistence.
#[derive(Clone, PartialEq)]
pub struct TaskRecord {
    pub task: AssistantTask,
    pub plan_revision: u32,
    pub steps: Vec<StepRecord>,
}

#[derive(Clone, PartialEq)]
pub struct StepRecord {
    pub step: TaskStep,
    pub plan_revision: u32,
}

/// Full desired membership, not an incremental patch. Raw input bytes are required
/// so protocol canonicalization can reject lossy or non-SCJ-1 input before writes.
pub struct Plan {
    pub revision: u32,
    pub steps: Vec<PlanStep>,
}

pub struct PlanStep {
    pub step: TaskStep,
    pub input_json: Vec<u8>,
}

/// One trusted capability proposal included in a complete capability plan.
pub struct CapabilityPlanStep {
    pub step_id: StepId,
    pub sequence: u32,
    pub capability_id: CapabilityId,
    pub arguments: serea_capability::ClassifiedArgumentsV1,
    pub requested_by: RequestedBy,
}

/// One member of an initial mixed plan. Approval gates are host-authored and
/// enumerate their exact following R2-compatible Capability action group.
pub enum MixedCapabilityPlanStep {
    ApprovalGate {
        step_id: StepId,
        sequence: u32,
        actions: Vec<StepId>,
    },
    Capability(CapabilityPlanStep),
}

pub type PlanRevision = PlanRevisionSnapshot;

/// Committed result of atomically raising an approval request and entering the
/// durable wait state. This is routing/status data, not authorization evidence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApprovalWaitOutcome {
    /// The durable request identity supplied by the host.
    pub approval_id: ApprovalId,
    /// The Task whose state changed.
    pub task_id: TaskId,
    /// The state committed by this operation.
    pub task_state: TaskState,
}

impl From<TaskSnapshot> for TaskRecord {
    fn from(snapshot: TaskSnapshot) -> Self {
        Self {
            task: snapshot.task,
            plan_revision: snapshot.plan_revision,
            steps: snapshot
                .steps
                .into_iter()
                .map(|s| StepRecord {
                    step: s.step,
                    plan_revision: s.plan_revision,
                })
                .collect(),
        }
    }
}
