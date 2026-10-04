use serea_protocol::{
    AssistantTask, AttemptBudget, DataClass, EpochMillis, Extensions, RiskClass, TaskId, TaskKind,
    TaskOrigin, TaskStep, TaskTitle,
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

pub type PlanRevision = PlanRevisionSnapshot;

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
