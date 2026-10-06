use crate::{
    CancellationOutcome, DeletionOutcome, EngineError, LeaseGuard, NewTask, Plan, PlanRevision,
    StepCommit, StepOutcome, TaskJournal, TaskRecord, TransitionContext,
};
use serea_event_bus::EventBus;
use serea_protocol::{
    AssistantTask, BlockedReason, EpochMillis, LeaseOwner, StepId, TaskId, TaskOriginKind,
    TaskState, Timestamp,
};
use serea_storage::{PlanWrite, StepInput, Store};

/// Owns lifecycle orchestration, not identifier entropy, clocks or effect execution.
/// A successful mutation result is published only after the outer commit succeeds.
pub struct TaskEngine {
    pub(crate) store: Store,
    pub(crate) event_bus: EventBus,
}

impl TaskEngine {
    pub fn new(store: Store, event_bus: EventBus) -> Self {
        Self { store, event_bus }
    }

    pub fn create_task(
        &mut self,
        spec: NewTask,
        context: &TransitionContext<'_>,
    ) -> Result<TaskRecord, EngineError> {
        let task = AssistantTask {
            task_id: spec.task_id,
            kind: spec.kind,
            title: spec.title,
            state: TaskState::Received,
            origin: spec.origin,
            data_class: spec.data_class,
            policy_class: spec.policy_class,
            created_at: Timestamp::from_epoch_millis(spec.created_at),
            updated_at: Timestamp::from_epoch_millis(spec.created_at),
            deadline_at: spec.deadline_at.map(Timestamp::from_epoch_millis),
            attempt_budget: spec.attempt_budget,
            steps: Vec::new(),
            blocked_reason: None,
            result_summary: None,
            cancelled_at: None,
            cancelled_by: None,
            failure_reason: None,
            extensions: spec.extensions,
        };
        self.store
            .transact_with_participants(&TaskJournal, &self.event_bus, |tx| {
                tx.insert_task(&task, context)
            })
            .map(TaskRecord::from)
            .map_err(EngineError::from)
    }

    pub fn start_planning(
        &mut self,
        task_id: TaskId,
        expected_state: TaskState,
        expected_revision: u32,
        now: EpochMillis,
        context: &TransitionContext<'_>,
    ) -> Result<TaskRecord, EngineError> {
        self.store
            .transact_with_participants(&TaskJournal, &self.event_bus, |tx| {
                tx.start_planning(&task_id, expected_state, expected_revision, now, context)
            })
            .map(TaskRecord::from)
            .map_err(EngineError::from)
    }

    pub fn persist_plan(
        &mut self,
        task_id: TaskId,
        plan: Plan,
        now: EpochMillis,
        context: &TransitionContext<'_>,
    ) -> Result<PlanRevision, EngineError> {
        let write = PlanWrite {
            revision: plan.revision,
            steps: plan
                .steps
                .into_iter()
                .map(|s| StepInput {
                    step: s.step,
                    input_json: s.input_json,
                })
                .collect(),
        };
        self.store
            .transact_with_participants(&TaskJournal, &self.event_bus, |tx| {
                tx.put_plan_revision(&task_id, write, now, context)
            })
            .map_err(EngineError::from)
    }

    /// Forwards caller-observed generation unchanged; storage SQL is authority.
    #[allow(clippy::too_many_arguments)]
    pub fn acquire(
        &mut self,
        task_id: TaskId,
        step_id: StepId,
        owner: LeaseOwner,
        expected_generation: Option<u32>,
        now: EpochMillis,
        expires_at: EpochMillis,
        context: &TransitionContext<'_>,
    ) -> Result<LeaseGuard, EngineError> {
        self.store
            .transact_with_participants(&TaskJournal, &self.event_bus, |tx| {
                tx.acquire_audited(
                    task_id,
                    step_id,
                    owner,
                    expected_generation,
                    now,
                    expires_at,
                    context,
                )
            })
            .map_err(EngineError::from)
    }

    /// Borrows the guard and never charges another attempt.
    pub fn begin_attempt(
        &mut self,
        guard: &LeaseGuard,
        now: EpochMillis,
        context: &TransitionContext<'_>,
    ) -> Result<(), EngineError> {
        self.store
            .transact_with_participants(&TaskJournal, &self.event_bus, |tx| {
                tx.begin_attempt(guard, now, context)
            })
            .map_err(EngineError::from)
    }

    /// Consumes authority on every result. No retry or effect inference is made.
    pub fn commit_step(
        &mut self,
        guard: LeaseGuard,
        outcome: StepOutcome<'_>,
        now: EpochMillis,
        context: &TransitionContext<'_>,
    ) -> Result<StepCommit, EngineError> {
        self.store
            .transact_with_participants(&TaskJournal, &self.event_bus, |tx| {
                tx.commit_step_outcome(guard, outcome, now, context)
            })
            .map_err(EngineError::from)
    }

    pub fn release(
        &mut self,
        guard: LeaseGuard,
        now: EpochMillis,
        context: &TransitionContext<'_>,
    ) -> Result<(), EngineError> {
        self.store
            .transact_with_participants(&TaskJournal, &self.event_bus, |tx| {
                tx.release_audited(guard, now, context)
            })
            .map_err(EngineError::from)
    }

    pub fn block(
        &mut self,
        task_id: TaskId,
        expected_state: TaskState,
        reason: BlockedReason,
        now: EpochMillis,
        context: &TransitionContext<'_>,
    ) -> Result<TaskRecord, EngineError> {
        self.store
            .transact_with_participants(&TaskJournal, &self.event_bus, |tx| {
                tx.block_task(&task_id, expected_state, reason, now, context)
            })
            .map(TaskRecord::from)
            .map_err(EngineError::from)
    }

    pub fn fail_invariant(
        &mut self,
        task_id: TaskId,
        expected_state: TaskState,
        now: EpochMillis,
        context: &TransitionContext<'_>,
    ) -> Result<TaskRecord, EngineError> {
        self.store
            .transact_with_participants(&TaskJournal, &self.event_bus, |tx| {
                tx.fail_task_invariant(&task_id, expected_state, now, context)
            })
            .map(TaskRecord::from)
            .map_err(EngineError::from)
    }

    pub fn cancel(
        &mut self,
        task_id: TaskId,
        by: TaskOriginKind,
        now: EpochMillis,
        context: &TransitionContext<'_>,
    ) -> Result<CancellationOutcome, EngineError> {
        self.store
            .transact_with_participants(&TaskJournal, &self.event_bus, |tx| {
                tx.cancel_task(&task_id, by, now, context)
            })
            .map_err(EngineError::from)
    }

    pub fn delete_task(&mut self, task_id: TaskId) -> Result<DeletionOutcome, EngineError> {
        self.store
            .transact(|tx| tx.delete_task(&task_id))
            .map_err(EngineError::from)
    }

    pub fn load(&self, task_id: TaskId) -> Result<TaskRecord, EngineError> {
        self.store
            .load_task(&task_id)
            .map(TaskRecord::from)
            .map_err(EngineError::from)
    }
}
