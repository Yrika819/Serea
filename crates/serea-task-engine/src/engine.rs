use crate::{
    CancellationOutcome, DeletionOutcome, EngineError, LeaseGuard, NewTask, Plan, PlanRevision,
    StepCommit, StepOutcome, TaskJournal, TaskRecord, TransitionContext,
};
use serea_event_bus::EventBus;
use serea_protocol::{
    AssistantTask, BlockedReason, DeviceId, EpochMillis, LeaseOwner, StepId, TaskId, TaskKind,
    TaskOriginKind, TaskState, Timestamp,
};
use serea_storage::{
    DeviceSessionResumeWake, EventDraft, PlanWrite, ScheduleOccurrenceLease, StepInput, Store,
    StoreError,
};

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

    /// Creates a Task, pinning the active capability registry generation in the
    /// same transaction as the Task row.
    ///
    /// Every runtime-created Task requires and pins the active registry
    /// generation. Historical pre-P5 Tasks retain their migrated NULL value.
    pub fn create_task(
        &mut self,
        spec: NewTask,
        context: &TransitionContext<'_>,
    ) -> Result<TaskRecord, EngineError> {
        let task = task_from_spec(spec);
        self.store
            .transact_with_participants(&TaskJournal, &self.event_bus, |tx| {
                let active = tx
                    .current_registry_generation()?
                    .ok_or(StoreError::RegistryGenerationNotFound)?;
                let snapshot = tx.insert_task(&task, context)?;
                tx.pin_task_registry_generation(&task.task_id, active.generation_id())?;
                Ok(snapshot)
            })
            .map(TaskRecord::from)
            .map_err(|error| match error {
                StoreError::RegistryGenerationNotFound => EngineError::NoActiveCapabilityGeneration,
                other => EngineError::from(other),
            })
    }

    /// Creates a post-P5 Task and pins the active capability registry
    /// generation in the same transaction as the Task row.
    ///
    /// The pin and the Task become durable together or not at all, so a Task is
    /// never transiently usable for capability planning without a generation.
    /// When no generation is active this fails closed: the Task is never created
    /// unpinned and is never silently attached to a later "current" generation.
    pub fn create_task_with_capability_pinning(
        &mut self,
        spec: NewTask,
        expected_active_generation: Option<i64>,
        context: &TransitionContext<'_>,
    ) -> Result<TaskRecord, EngineError> {
        let task = task_from_spec(spec);
        self.store
            .transact_with_participants(&TaskJournal, &self.event_bus, |tx| {
                // Storage has no dedicated "no active generation" code, so the
                // condition travels as its nearest typed refusal and is mapped
                // back to the specific engine outcome below.
                let active = tx
                    .current_registry_generation()?
                    .ok_or(StoreError::RegistryGenerationNotFound)?;
                if let Some(expected) = expected_active_generation {
                    if expected != active.generation_id() {
                        return Err(StoreError::RegistryGenerationNotFound);
                    }
                }
                let snapshot = tx.insert_task(&task, context)?;
                tx.pin_task_registry_generation(&task.task_id, active.generation_id())?;
                Ok(snapshot)
            })
            .map(TaskRecord::from)
            .map_err(|error| match error {
                StoreError::RegistryGenerationNotFound => EngineError::NoActiveCapabilityGeneration,
                other => EngineError::from(other),
            })
    }

    /// Creates one capability Step for an existing Task and binds it, in one
    /// durable operation.
    ///
    /// The Task's pinned generation selects the descriptor: an unpinned legacy
    /// Task fails closed rather than borrowing the current generation. The
    /// candidate comes from the frozen availability snapshot, so overlay,
    /// experimental opt-in, provider health, exact advertisement and host
    /// eligibility all gate the NEW binding, while an already-bound Step keeps
    /// its exact revision regardless of later changes.
    #[allow(clippy::too_many_arguments)]
    pub fn create_capability_step(
        &mut self,
        snapshot: &serea_capability::CapabilityAvailabilitySnapshotV1,
        task_id: TaskId,
        step_id: StepId,
        sequence: u32,
        capability_id: &serea_protocol::CapabilityId,
        classified: &serea_capability::ClassifiedArgumentsV1,
        requested_by: serea_protocol::RequestedBy,
        now: EpochMillis,
        context: &TransitionContext<'_>,
    ) -> Result<serea_storage::StepCapabilityBinding, EngineError> {
        self.create_capability_plan(
            snapshot,
            task_id,
            vec![crate::CapabilityPlanStep {
                step_id,
                sequence,
                capability_id: capability_id.clone(),
                arguments: classified.clone(),
                requested_by,
            }],
            now,
            context,
        )?
        .into_iter()
        .next()
        .ok_or(EngineError::InvalidPlan)
    }

    /// Persists one complete newly-created capability plan with every immutable
    /// binding in the same transaction. Capability Steps are never published
    /// through a partial append path.
    pub fn create_capability_plan(
        &mut self,
        snapshot: &serea_capability::CapabilityAvailabilitySnapshotV1,
        task_id: TaskId,
        proposals: Vec<crate::CapabilityPlanStep>,
        now: EpochMillis,
        context: &TransitionContext<'_>,
    ) -> Result<Vec<serea_storage::StepCapabilityBinding>, EngineError> {
        use serde_json::Value;
        if proposals.is_empty() {
            return Err(EngineError::InvalidPlan);
        }
        let fallback_capability_id = proposals[0].capability_id.clone();
        let before = self.store.load_task(&task_id).map_err(EngineError::from)?;
        let pinned = self
            .store
            .get_task_registry_generation(&task_id)
            .map_err(EngineError::from)?
            .ok_or(EngineError::UnpinnedTaskCannotBindCapability)?;
        if pinned != snapshot.generation_id() {
            return Err(EngineError::CapabilityUnavailable {
                capability_id: fallback_capability_id,
            });
        }
        if !before.steps.is_empty() {
            return Err(EngineError::InvalidPlan);
        }
        let next_revision = before
            .plan_revision
            .checked_add(1)
            .ok_or(EngineError::InvalidPlan)?;
        let mut steps = Vec::with_capacity(proposals.len());
        let mut bindings = Vec::with_capacity(proposals.len());
        for proposal in proposals {
            let resolution = snapshot.resolve(&proposal.capability_id).map_err(|_| {
                EngineError::CapabilityUnavailable {
                    capability_id: proposal.capability_id.clone(),
                }
            })?;
            if proposal.arguments.data_class().rank() > resolution.descriptor.data_class().rank()
                || proposal.arguments.data_class().rank() > before.task.data_class.rank()
            {
                return Err(EngineError::CapabilityUnavailable {
                    capability_id: proposal.capability_id.clone(),
                });
            }
            let prepared = serea_capability::prepare_action(
                snapshot,
                &proposal.capability_id,
                task_id.clone(),
                proposal.step_id.clone(),
                &proposal.arguments,
                proposal.requested_by,
                None,
                None,
            )
            .map_err(|_| EngineError::CapabilityUnavailable {
                capability_id: proposal.capability_id.clone(),
            })?;
            let input_json = serde_json::to_vec(&Value::Object(prepared.arguments().clone()))
                .map_err(|_| EngineError::InvalidPlan)?;
            let canonical_input = serea_protocol::canonicalize(
                std::str::from_utf8(&input_json).map_err(|_| EngineError::InvalidPlan)?,
            )
            .map_err(|_| EngineError::InvalidPlan)?;
            let step = serea_protocol::TaskStep::new(serea_protocol::TaskStepDraft {
                task_id: task_id.clone(),
                step_id: proposal.step_id.clone(),
                sequence: proposal.sequence,
                kind: serea_protocol::StepKind::Capability,
                status: serea_protocol::StepStatus::new("PLANNED")
                    .map_err(|_| EngineError::InvalidPlan)?,
                attempt: 0,
                idempotency_key: Some(prepared.idempotency_key().clone()),
                provider_id: Some(prepared.provider_id().clone()),
                capability_id: Some(proposal.capability_id),
                capability_version: Some(prepared.capability_version().clone()),
                input_digest: prepared.arguments_digest().clone(),
                result_digest: None,
                side_effect_receipt: None,
                started_at: None,
                completed_at: None,
                lease_owner: None,
                lease_expires_at: None,
                lease_generation: None,
                error: None,
                extensions: [].into(),
            })
            .map_err(|_| EngineError::InvalidPlan)?;
            steps.push(StepInput {
                step,
                input_json: canonical_input,
            });
            bindings.push(serea_storage::CapabilityPlanBindingDraft {
                step_id: proposal.step_id,
                descriptor_digest: resolution.descriptor_digest,
            });
        }
        self.store
            .transact_with_participants(&TaskJournal, &self.event_bus, |tx| {
                tx.put_capability_plan_revision(
                    &task_id,
                    PlanWrite {
                        revision: next_revision,
                        steps,
                    },
                    &bindings,
                    now,
                    context,
                )
                .map(|(_, bindings)| bindings)
                .map_err(|error| match error {
                    serea_storage::StoreError::RegistryCapabilityUnavailable => {
                        StoreError::RegistryBindingRefused
                    }
                    serea_storage::StoreError::RegistryTaskUnpinned => {
                        StoreError::RegistryTaskUnpinned
                    }
                    other => other,
                })
            })
            .map_err(|error| match error {
                StoreError::RegistryBindingRefused => EngineError::CapabilityUnavailable {
                    capability_id: fallback_capability_id,
                },
                StoreError::RegistryTaskUnpinned => EngineError::UnpinnedTaskCannotBindCapability,
                other => EngineError::from(other),
            })
    }

    /// Creates a scheduled task and commits its occurrence mapping with the
    pub fn create_scheduled_task(
        &mut self,
        spec: NewTask,
        occurrence: &ScheduleOccurrenceLease,
        now: EpochMillis,
        schedule_event: EventDraft,
        context: &TransitionContext<'_>,
    ) -> Result<TaskRecord, EngineError> {
        let task = task_from_spec(spec);
        self.store
            .transact_with_participants(&TaskJournal, &self.event_bus, |tx| {
                if let Some(existing_id) = tx
                    .schedule_occurrence_task(&occurrence.schedule_id, &occurrence.occurrence_key)?
                {
                    let existing = tx.load_task(&existing_id)?;
                    if existing.task.kind != TaskKind::Scheduled {
                        return Err(serea_storage::StoreError::CorruptRow);
                    }
                    return Ok(existing);
                }
                let active = tx
                    .current_registry_generation()?
                    .ok_or(StoreError::RegistryGenerationNotFound)?;
                let snapshot = tx.insert_task(&task, context)?;
                tx.pin_task_registry_generation(&task.task_id, active.generation_id())?;
                tx.map_schedule_occurrence_with_event(
                    occurrence,
                    &task.task_id,
                    now,
                    schedule_event,
                )?;
                Ok(snapshot)
            })
            .map(TaskRecord::from)
            .map_err(|error| match error {
                StoreError::RegistryGenerationNotFound => EngineError::NoActiveCapabilityGeneration,
                other => EngineError::from(other),
            })
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
        if reason.as_str() == "DEVICE_OFFLINE" {
            return Err(EngineError::IllegalTaskTransition);
        }
        self.store
            .transact_with_participants(&TaskJournal, &self.event_bus, |tx| {
                tx.block_task(&task_id, expected_state, reason, now, context)
            })
            .map(TaskRecord::from)
            .map_err(EngineError::from)
    }

    /// Blocks a task specifically for one device and atomically records the
    /// sequence-fenced wait needed for a later DEVICE_CONNECTED wake.
    pub fn block_for_device(
        &mut self,
        task_id: TaskId,
        device_id: DeviceId,
        expected_state: TaskState,
        expected_state_revision: u64,
        now: EpochMillis,
        context: &TransitionContext<'_>,
    ) -> Result<TaskRecord, EngineError> {
        self.store
            .transact_with_participants(&TaskJournal, &self.event_bus, |tx| {
                tx.block_task_for_device(
                    &task_id,
                    &device_id,
                    expected_state,
                    expected_state_revision,
                    now,
                    context,
                )
            })
            .map(TaskRecord::from)
            .map_err(EngineError::from)
    }

    /// Processes one already-materialized device resume wake. A stale wake is
    /// durably consumed without changing the task or emitting TASK_RESUMED.
    pub fn resume_device_session_wake(
        &mut self,
        wake: &DeviceSessionResumeWake,
        now: EpochMillis,
        context: &TransitionContext<'_>,
    ) -> Result<Option<TaskRecord>, EngineError> {
        self.store
            .transact_with_participants(&TaskJournal, &self.event_bus, |tx| {
                tx.resume_device_session_wake(wake, now, context)
            })
            .map(|snapshot| snapshot.map(TaskRecord::from))
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

fn task_from_spec(spec: crate::NewTask) -> AssistantTask {
    AssistantTask {
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
    }
}
