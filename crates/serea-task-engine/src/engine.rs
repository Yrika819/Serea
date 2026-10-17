use crate::{
    ApprovalWaitOutcome, CancellationOutcome, DeletionOutcome, EngineError, LeaseGuard, NewTask,
    Plan, PlanRevision, StepCommit, StepOutcome, TaskJournal, TaskRecord, TransitionContext,
};
use serea_event_bus::{ApprovalLifecycleKindV1, EventBus};
use serea_protocol::{
    ActorId, ActorKind, AssistantTask, BlockedReason, DeviceId, EpochMillis, LeaseOwner, StepId,
    TaskId, TaskKind, TaskOriginKind, TaskState, Timestamp,
};
use serea_storage::{
    ApprovalLifecycleWake, ApprovalWakeApplyOutcome, CapabilityOverlay, DescriptorRevision,
    DeviceSessionResumeWake, EventDraft, GrantFactSnapshot, LeaseFactSnapshot, PlanWrite,
    PolicyRevisionSnapshot, RegistryGeneration, ScheduleOccurrenceLease, StepCapabilityBinding,
    StepInput, Store, StoreError, TaskSnapshot,
};

/// A mismatch between caller selectors and facts reloaded from SQLite.
/// These comparisons report stale or inconsistent pins; they are not a policy
/// decision and never imply permission to execute.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DurableFactMismatch {
    TaskId,
    StepId,
    StepPlanRevision,
    StepKind,
    StepInputDigest,
    RegistryGeneration,
    RegistryManifestDigest,
    DescriptorDigest,
    CapabilityId,
    CapabilityVersion,
    ProviderId,
    ImplementationId,
    DescriptorProperties,
    LeaseTaskOrStep,
    GrantPin,
    GrantMemberSet,
    GrantTaskOrMemberDigest,
}

/// Read-only facts and exact pin comparison results rebuilt in one SQLite
/// `BEGIN IMMEDIATE` transaction. This is intentionally not an authorization
/// result, PolicyEvaluation, commit proof, or Permit. Host-authenticated
/// provenance and a trusted clock are not present in this snapshot.
pub struct L1FactObservation {
    task: TaskSnapshot,
    generation: Option<RegistryGeneration>,
    binding: Option<StepCapabilityBinding>,
    descriptor: Option<DescriptorRevision>,
    overlay: Option<CapabilityOverlay>,
    policy: Option<PolicyRevisionSnapshot>,
    lease: LeaseFactSnapshot,
    grant: Option<GrantFactSnapshot>,
    mismatches: Vec<DurableFactMismatch>,
}

impl L1FactObservation {
    pub fn task(&self) -> &TaskSnapshot {
        &self.task
    }
    pub fn generation(&self) -> Option<&RegistryGeneration> {
        self.generation.as_ref()
    }
    pub fn binding(&self) -> Option<&StepCapabilityBinding> {
        self.binding.as_ref()
    }
    pub fn descriptor(&self) -> Option<&DescriptorRevision> {
        self.descriptor.as_ref()
    }
    pub fn overlay(&self) -> Option<&CapabilityOverlay> {
        self.overlay.as_ref()
    }
    pub fn policy(&self) -> Option<&PolicyRevisionSnapshot> {
        self.policy.as_ref()
    }
    pub fn lease(&self) -> &LeaseFactSnapshot {
        &self.lease
    }
    pub fn grant(&self) -> Option<&GrantFactSnapshot> {
        self.grant.as_ref()
    }
    /// Empty means only that these durable pins matched their selectors. It
    /// says nothing about missing Host trust roots or execution authority.
    pub fn mismatches(&self) -> &[DurableFactMismatch] {
        &self.mismatches
    }
}

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

    /// Rebuilds the durable Task/Step/Plan, Registry, Descriptor, Overlay,
    /// Policy, Lease and optional Grant facts under one SQLite write
    /// transaction. `PreparedActionV1` and IDs are selectors/comparison
    /// targets only. Caller-supplied policy facts are not evaluated here.
    ///
    /// An empty mismatch list means the compared durable pins agree. It does
    /// not authenticate Owner/Worker, establish trusted time or Provider
    /// readiness, decide whether a Grant is currently unexpired, or authorize
    /// an effect. The callback performs reads only; no event, Grant use,
    /// Intent, budget, IDK, RequestId, or Provider call is created.
    pub fn observe_l1_facts(
        &self,
        prepared: &serea_capability::PreparedActionV1,
        lease_guard: &LeaseGuard,
        grant_id: Option<&serea_protocol::GrantId>,
    ) -> Result<L1FactObservation, EngineError> {
        use serea_protocol::StepKind;

        self.store
            .transact(|tx| {
                let task = tx.load_task(prepared.task_id())?;
                let step = task
                    .steps
                    .iter()
                    .find(|item| item.step.step_id == *prepared.step_id())
                    .ok_or(StoreError::TaskNotFound)?;
                let task_generation = tx.get_task_registry_generation(prepared.task_id())?;
                let generation = task_generation
                    .map(|id| tx.get_registry_generation(id))
                    .transpose()?
                    .flatten();
                let binding =
                    tx.get_step_capability_binding(prepared.task_id(), prepared.step_id())?;
                let descriptor = tx.get_descriptor_revision(prepared.descriptor_digest())?;
                let overlay = tx.get_capability_overlay(prepared.capability_id())?;
                let policy = tx.current_policy_revision()?;
                let lease = tx.observe_lease_facts(lease_guard)?;
                let grant = grant_id
                    .map(|id| tx.observe_grant_facts(id, prepared.task_id(), prepared.step_id()))
                    .transpose()?
                    .flatten();

                let mut mismatches = Vec::new();
                if task.task.task_id != *prepared.task_id() {
                    mismatches.push(DurableFactMismatch::TaskId);
                }
                if step.step.step_id != *prepared.step_id() {
                    mismatches.push(DurableFactMismatch::StepId);
                }
                if step.plan_revision != task.plan_revision {
                    mismatches.push(DurableFactMismatch::StepPlanRevision);
                }
                if step.step.kind != StepKind::Capability {
                    mismatches.push(DurableFactMismatch::StepKind);
                }
                if step.step.input_digest != *prepared.arguments_digest() {
                    mismatches.push(DurableFactMismatch::StepInputDigest);
                }
                if task_generation != Some(prepared.generation_id()) {
                    mismatches.push(DurableFactMismatch::RegistryGeneration);
                }
                if lease.task_id() != prepared.task_id() || lease.step_id() != prepared.step_id() {
                    mismatches.push(DurableFactMismatch::LeaseTaskOrStep);
                }
                if let Some(generation) = &generation {
                    if generation.activated_at().is_none() {
                        mismatches.push(DurableFactMismatch::RegistryGeneration);
                    }
                    if generation.manifest_digest() != prepared.generation_digest() {
                        mismatches.push(DurableFactMismatch::RegistryManifestDigest);
                    }
                } else {
                    mismatches.push(DurableFactMismatch::RegistryGeneration);
                }
                if let Some(binding) = &binding {
                    if binding.generation_id() != prepared.generation_id() {
                        mismatches.push(DurableFactMismatch::RegistryGeneration);
                    }
                    if binding.descriptor_digest() != prepared.descriptor_digest() {
                        mismatches.push(DurableFactMismatch::DescriptorDigest);
                    }
                    if binding.capability_id() != prepared.capability_id() {
                        mismatches.push(DurableFactMismatch::CapabilityId);
                    }
                    if binding.capability_version() != prepared.capability_version() {
                        mismatches.push(DurableFactMismatch::CapabilityVersion);
                    }
                    if binding.provider_id() != prepared.provider_id() {
                        mismatches.push(DurableFactMismatch::ProviderId);
                    }
                    if binding.implementation_id() != prepared.implementation_id() {
                        mismatches.push(DurableFactMismatch::ImplementationId);
                    }
                } else {
                    mismatches.push(DurableFactMismatch::DescriptorDigest);
                }
                if let Some(descriptor) = &descriptor {
                    let d = descriptor.descriptor();
                    if descriptor.descriptor_digest() != prepared.descriptor_digest() {
                        mismatches.push(DurableFactMismatch::DescriptorDigest);
                    }
                    if d.id() != prepared.capability_id() {
                        mismatches.push(DurableFactMismatch::CapabilityId);
                    }
                    if d.version() != prepared.capability_version() {
                        mismatches.push(DurableFactMismatch::CapabilityVersion);
                    }
                    if d.provider_id() != prepared.provider_id() {
                        mismatches.push(DurableFactMismatch::ProviderId);
                    }
                    if d.implementation_id() != prepared.implementation_id() {
                        mismatches.push(DurableFactMismatch::ImplementationId);
                    }
                    if d.risk_class() != prepared.risk_class()
                        || d.side_effect_class() != prepared.side_effect_class()
                        || d.required_authorization() != prepared.required_authorization()
                        || d.root_requirement() != prepared.root_requirement()
                        || prepared.data_class().rank() > d.data_class().rank()
                    {
                        mismatches.push(DurableFactMismatch::DescriptorProperties);
                    }
                } else {
                    mismatches.push(DurableFactMismatch::DescriptorDigest);
                }
                if overlay.capability_id() != prepared.capability_id() {
                    mismatches.push(DurableFactMismatch::CapabilityId);
                }
                if let Some(grant) = &grant {
                    if grant.generation_id() != prepared.generation_id()
                        || grant.descriptor_digest() != prepared.descriptor_digest()
                        || grant.capability_id() != prepared.capability_id()
                        || grant.capability_version() != prepared.capability_version()
                    {
                        mismatches.push(DurableFactMismatch::GrantPin);
                    }
                    if !grant.sealed() || grant.member_count() != grant.action_count() {
                        mismatches.push(DurableFactMismatch::GrantMemberSet);
                    }
                    if grant.task_id() != prepared.task_id()
                        || grant.member_arguments_digest() != Some(prepared.arguments_digest())
                    {
                        mismatches.push(DurableFactMismatch::GrantTaskOrMemberDigest);
                    }
                }
                Ok(L1FactObservation {
                    task,
                    generation,
                    binding,
                    descriptor,
                    overlay: Some(overlay),
                    policy,
                    lease,
                    grant,
                    mismatches,
                })
            })
            .map_err(EngineError::from)
    }

    /// Evaluates a prepared capability action against its current durable Task,
    /// Step, registry binding, descriptor, overlay and active Policy snapshot.
    ///
    /// The returned evidence is a point-in-time explanation, not execution
    /// authority. Any later approval request must still pass the atomic
    /// Storage revalidation performed by [`Self::request_approval`], and a
    /// future dispatcher must revalidate again immediately before execution.
    pub fn evaluate_prepared_action(
        &self,
        prepared: &serea_capability::PreparedActionV1,
    ) -> Result<serea_policy::PolicyEvaluation, EngineError> {
        use serea_storage::CapabilityOverlayState;

        let snapshot = self
            .store
            .load_task(prepared.task_id())
            .map_err(EngineError::from)?;
        let step = snapshot
            .steps
            .iter()
            .find(|item| item.step.step_id == *prepared.step_id())
            .ok_or(EngineError::InvalidPlan)?;
        let plan_revision = i64::from(snapshot.plan_revision);
        let generation_id = self
            .store
            .get_task_registry_generation(prepared.task_id())
            .map_err(EngineError::from)?
            .ok_or(EngineError::UnpinnedTaskCannotBindCapability)?;
        let generation = self
            .store
            .get_registry_generation(generation_id)
            .map_err(EngineError::from)?
            .ok_or(EngineError::InvalidPlan)?;
        let binding = self
            .store
            .get_step_capability_binding(prepared.task_id(), prepared.step_id())
            .map_err(EngineError::from)?
            .ok_or(EngineError::InvalidPlan)?;
        let descriptor_revision = self
            .store
            .get_descriptor_revision(prepared.descriptor_digest())
            .map_err(EngineError::from)?
            .ok_or(EngineError::InvalidPlan)?;
        let overlay = self
            .store
            .get_capability_overlay(prepared.capability_id())
            .map_err(EngineError::from)?;

        let task = &snapshot.task;
        let descriptor = descriptor_revision.descriptor();
        if step.plan_revision != snapshot.plan_revision
            || step.step.kind != serea_protocol::StepKind::Capability
            || step.step.input_digest != *prepared.arguments_digest()
            || task.task_id != *prepared.task_id()
            || task.state.is_terminal()
            || task.cancelled_at.is_some()
            || generation.generation_id() != prepared.generation_id()
            || generation.manifest_digest() != prepared.generation_digest()
            || generation.activated_at().is_none()
            || binding.task_id() != prepared.task_id()
            || binding.step_id() != prepared.step_id()
            || binding.generation_id() != prepared.generation_id()
            || binding.descriptor_digest() != prepared.descriptor_digest()
            || binding.capability_id() != prepared.capability_id()
            || binding.capability_version() != prepared.capability_version()
            || binding.provider_id() != prepared.provider_id()
            || binding.implementation_id() != prepared.implementation_id()
            || descriptor_revision.descriptor_digest() != prepared.descriptor_digest()
            || descriptor.id() != prepared.capability_id()
            || descriptor.version() != prepared.capability_version()
            || descriptor.provider_id() != prepared.provider_id()
            || descriptor.implementation_id() != prepared.implementation_id()
            || descriptor.risk_class() != prepared.risk_class()
            || descriptor.side_effect_class() != prepared.side_effect_class()
            || descriptor.required_authorization() != prepared.required_authorization()
            || descriptor.root_requirement() != prepared.root_requirement()
            || prepared.data_class().rank() > descriptor.data_class().rank()
        {
            return Err(EngineError::InvalidPlan);
        }

        let revision = serea_policy::load_active_revision(&self.store)
            .map_err(|_| EngineError::InvalidPlan)?
            .ok_or(EngineError::InvalidPlan)?;
        let capability_enabled = overlay.state() == CapabilityOverlayState::Enabled
            && (!descriptor.experimental() || overlay.experimental_opt_in());
        let facts = serea_policy::PolicyInputDraft {
            task_id: task.task_id.clone(),
            step_id: prepared.step_id().clone(),
            task_kind: task.kind,
            task_state: task.state,
            task_cancelled: task.cancelled_at.is_some(),
            task_deadline_at: task
                .deadline_at
                .as_ref()
                .map(|value| value.to_epoch_millis()),
            task_data_class: task.data_class,
            task_policy_class: task.policy_class,
            capability_id: descriptor.id().clone(),
            capability_version: descriptor.version().clone(),
            provider_id: descriptor.provider_id().clone(),
            implementation_id: descriptor.implementation_id().cloned(),
            risk_class: descriptor.risk_class(),
            side_effect_class: descriptor.side_effect_class(),
            authorization: descriptor.required_authorization(),
            root_requirement: descriptor.root_requirement(),
            data_class: prepared.data_class(),
            requested_by: prepared.requested_by(),
            device_state: None,
            capability_enabled,
            // No durable destructive opt-in source exists in the accepted P6
            // schema. Keep the gate closed rather than infer it from unrelated
            // experimental-capability state.
            destructive_enabled: false,
            plan_revision,
            registry_generation: generation_id,
            generation_digest: generation.manifest_digest().clone(),
            descriptor_digest: descriptor_revision.descriptor_digest().clone(),
            arguments_digest: prepared.arguments_digest().clone(),
        };
        let input = serea_policy::PolicyInputV1::new(facts, &revision)
            .map_err(|_| EngineError::InvalidPlan)?;
        Ok(serea_policy::evaluate_with_evidence(&input, &revision))
    }

    /// Raises a fully validated host-built approval request and moves its live
    /// Task to `WAITING_APPROVAL` atomically. Scheduler wake records are created
    /// later from the committed lifecycle event and are only routing signals.
    pub fn request_approval(
        &mut self,
        request: serea_policy::ApprovalRequestV1,
        evidence: &[serea_policy::AuthorizationEvidenceV1],
        now: EpochMillis,
        context: &TransitionContext<'_>,
    ) -> Result<ApprovalWaitOutcome, EngineError> {
        let request_facts = request.draft();
        let approval_id = request_facts.approval_id.clone();
        let task_id = request_facts.task_id.clone();
        if request_facts.actions.actions().len() != evidence.len() || evidence.is_empty() {
            return Err(EngineError::InvalidPlan);
        }
        let (policy_revision_id, policy_rules_digest) = {
            let first = evidence.first().ok_or(EngineError::InvalidPlan)?;
            if !matches!(
                first.decision(),
                serea_policy::PolicyDecision::RequireApproval(_)
            ) {
                return Err(EngineError::InvalidPlan);
            }
            (
                first.policy_revision_id(),
                first.policy_rules_digest().clone(),
            )
        };
        for action in request_facts.actions.actions() {
            let matched = evidence.iter().any(|item| {
                let facts = item.input_facts();
                matches!(
                    item.decision(),
                    serea_policy::PolicyDecision::RequireApproval(_)
                ) && item.policy_revision_id() == policy_revision_id
                    && item.policy_rules_digest() == &policy_rules_digest
                    && facts.task_id == request_facts.task_id
                    && facts.task_kind == request_facts.task_kind
                    && facts.step_id == *action.step_id()
                    && facts.arguments_digest == *action.arguments_digest()
                    && facts.plan_revision == request_facts.plan_revision
                    && facts.registry_generation == request_facts.registry_generation
                    && facts.descriptor_digest == request_facts.descriptor_digest
                    && facts.capability_id == request_facts.capability_id
                    && facts.capability_version == request_facts.capability_version
                    && facts.risk_class == request_facts.risk_class
                    && facts.side_effect_class == request_facts.side_effect_class
                    && facts.authorization == request_facts.authorization
                    && facts.data_class.rank() <= request_facts.data_class.rank()
                    && facts.requested_by == request_facts.requested_by
            });
            if !matched {
                return Err(EngineError::InvalidPlan);
            }
        }
        let first_action = request_facts
            .actions
            .actions()
            .first()
            .ok_or(EngineError::InvalidPlan)?;
        // This lifecycle event is host-authored; model/user provenance stays in
        // the durable request and task journal, never in the event actor slot.
        let event_actor = ActorId::new("serea-core").map_err(|_| EngineError::InvalidPlan)?;
        let event = self
            .event_bus
            .draft_approval_lifecycle(
                ApprovalLifecycleKindV1::Required,
                &request_facts.approval_id,
                &request_facts.task_id,
                first_action.step_id(),
                ActorKind::Host,
                event_actor,
                context.actor_version.clone(),
                now,
            )
            .map_err(EngineError::from)?;
        let draft = request.storage_draft();
        self.store
            .transact_with_participants(&TaskJournal, &self.event_bus, |tx| {
                tx.raise_approval_request_and_wait(
                    draft,
                    event,
                    policy_revision_id,
                    &policy_rules_digest,
                    now,
                    context,
                )
            })
            .map_err(EngineError::from)?;
        Ok(ApprovalWaitOutcome {
            approval_id,
            task_id,
            task_state: TaskState::WaitingApproval,
        })
    }

    /// Resolves one exact approval gate without creating a human approval when
    /// every enumerated action currently evaluates ALLOW. This records only
    /// `NOT_REQUIRED`; it grants no authority to execute any Capability Step.
    pub fn complete_approval_gate_not_required(
        &mut self,
        wait_step_id: StepId,
        actions: &[serea_capability::PreparedActionV1],
        owner: LeaseOwner,
        now: EpochMillis,
        expires_at: EpochMillis,
        context: &TransitionContext<'_>,
    ) -> Result<StepCommit, EngineError> {
        if actions.is_empty() || actions.len() > 8 {
            return Err(EngineError::InvalidPlan);
        }
        let mut evidence = Vec::with_capacity(actions.len());
        for action in actions {
            let evaluation = self.evaluate_prepared_action(action)?;
            if evaluation.decision != serea_policy::PolicyDecision::Allow {
                return Err(EngineError::InvalidPlan);
            }
            evidence.push(evaluation.evidence);
        }
        let first = evidence.first().ok_or(EngineError::InvalidPlan)?;
        let task_id = first.task_id().clone();
        let plan_revision_raw = first.input_facts().plan_revision;
        let plan_revision =
            u32::try_from(plan_revision_raw).map_err(|_| EngineError::InvalidPlan)?;
        let policy_revision_id = first.policy_revision_id();
        let policy_rules_digest = first.policy_rules_digest().clone();
        if evidence.iter().zip(actions).any(|(item, prepared)| {
            item.task_id() != &task_id
                || item.step_id() != prepared.step_id()
                || item.arguments_digest() != prepared.arguments_digest()
                || item.input_facts().plan_revision != plan_revision_raw
                || item.policy_revision_id() != policy_revision_id
                || item.policy_rules_digest() != &policy_rules_digest
                || item.decision() != serea_policy::PolicyDecision::Allow
        }) {
            return Err(EngineError::InvalidPlan);
        }
        let action_digests: Vec<(StepId, serea_protocol::Digest)> = evidence
            .iter()
            .map(|item| (item.step_id().clone(), item.arguments_digest().clone()))
            .collect();
        self.store
            .transact_with_participants(&TaskJournal, &self.event_bus, |tx| {
                tx.complete_approval_gate_not_required(
                    &task_id,
                    &wait_step_id,
                    plan_revision,
                    &action_digests,
                    policy_revision_id,
                    &policy_rules_digest,
                    owner,
                    now,
                    expires_at,
                    context,
                )
            })
            .map_err(|error| match error {
                StoreError::InvalidApprovalRequest => EngineError::InvalidPlan,
                other => EngineError::from(other),
            })
    }

    /// Applies an already materialized Scheduler wake after re-reading the
    /// authoritative approval, grant, Task and wait Step rows. The Task update,
    /// Task event, audit rows and wake acknowledgement share one transaction.
    pub fn apply_approval_wake(
        &mut self,
        wake: &ApprovalLifecycleWake,
        now: EpochMillis,
        context: &TransitionContext<'_>,
    ) -> Result<ApprovalWakeApplyOutcome, EngineError> {
        self.store
            .transact_with_participants(&TaskJournal, &self.event_bus, |tx| {
                tx.apply_approval_lifecycle_wake(wake, now, context)
            })
            .map_err(EngineError::from)
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
        self.create_mixed_capability_plan(
            snapshot,
            task_id,
            proposals
                .into_iter()
                .map(crate::MixedCapabilityPlanStep::Capability)
                .collect(),
            now,
            context,
        )
    }

    /// Persists an initial complete mixed Plan. Every gate must immediately
    /// precede and enumerate its exact 1–8 action R2 group. Capability Steps
    /// use the same pinned-snapshot preparation and binding path as the
    /// capability-only API; wait Steps remain host-only records.
    pub fn create_mixed_capability_plan(
        &mut self,
        snapshot: &serea_capability::CapabilityAvailabilitySnapshotV1,
        task_id: TaskId,
        plan: Vec<crate::MixedCapabilityPlanStep>,
        now: EpochMillis,
        context: &TransitionContext<'_>,
    ) -> Result<Vec<serea_storage::StepCapabilityBinding>, EngineError> {
        use serde_json::{Value, json};
        use std::collections::{HashMap, HashSet};

        if plan.is_empty() {
            return Err(EngineError::InvalidPlan);
        }
        let before = self.store.load_task(&task_id).map_err(EngineError::from)?;
        let pinned = self
            .store
            .get_task_registry_generation(&task_id)
            .map_err(EngineError::from)?
            .ok_or(EngineError::UnpinnedTaskCannotBindCapability)?;
        if pinned != snapshot.generation_id() {
            let capability_id = plan.iter().find_map(|item| match item {
                crate::MixedCapabilityPlanStep::Capability(step) => {
                    Some(step.capability_id.clone())
                }
                crate::MixedCapabilityPlanStep::ApprovalGate { .. } => None,
            });
            return Err(EngineError::CapabilityUnavailable {
                capability_id: capability_id.ok_or(EngineError::InvalidPlan)?,
            });
        }
        if !before.steps.is_empty() {
            return Err(EngineError::InvalidPlan);
        }
        let next_revision = before
            .plan_revision
            .checked_add(1)
            .ok_or(EngineError::InvalidPlan)?;

        let mut ids = HashSet::new();
        let mut last_sequence = 0;
        let mut prepared = HashMap::new();
        let mut fallback_capability_id = None;
        for item in &plan {
            let (step_id, sequence) = match item {
                crate::MixedCapabilityPlanStep::ApprovalGate {
                    step_id, sequence, ..
                } => (step_id, *sequence),
                crate::MixedCapabilityPlanStep::Capability(step) => {
                    fallback_capability_id.get_or_insert(step.capability_id.clone());
                    (&step.step_id, step.sequence)
                }
            };
            if sequence == 0
                || sequence <= last_sequence
                || !ids.insert(step_id.as_str().to_owned())
            {
                return Err(EngineError::InvalidPlan);
            }
            last_sequence = sequence;
            if let crate::MixedCapabilityPlanStep::Capability(proposal) = item {
                let resolution = snapshot.resolve(&proposal.capability_id).map_err(|_| {
                    EngineError::CapabilityUnavailable {
                        capability_id: proposal.capability_id.clone(),
                    }
                })?;
                if proposal.arguments.data_class().rank()
                    > resolution.descriptor.data_class().rank()
                    || proposal.arguments.data_class().rank() > before.task.data_class.rank()
                {
                    return Err(EngineError::CapabilityUnavailable {
                        capability_id: proposal.capability_id.clone(),
                    });
                }
                let action = serea_capability::prepare_action(
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
                prepared.insert(
                    proposal.step_id.clone(),
                    (action, resolution.descriptor_digest),
                );
            }
        }
        if fallback_capability_id.is_none() {
            return Err(EngineError::InvalidPlan);
        }

        let mut covered = HashSet::new();
        for (index, item) in plan.iter().enumerate() {
            let crate::MixedCapabilityPlanStep::ApprovalGate { actions, .. } = item else {
                continue;
            };
            if actions.is_empty() || actions.len() > 8 {
                return Err(EngineError::InvalidPlan);
            }
            let mut unique_actions = HashSet::new();
            let mut group_identity = None;
            for (offset, action_id) in actions.iter().enumerate() {
                if !unique_actions.insert(action_id.as_str()) || !covered.insert(action_id.clone())
                {
                    return Err(EngineError::InvalidPlan);
                }
                let Some(crate::MixedCapabilityPlanStep::Capability(proposal)) =
                    plan.get(index + 1 + offset)
                else {
                    return Err(EngineError::InvalidPlan);
                };
                if proposal.step_id != *action_id {
                    return Err(EngineError::InvalidPlan);
                }
                let (action, digest) = prepared.get(action_id).ok_or(EngineError::InvalidPlan)?;
                let identity = (
                    proposal.capability_id.clone(),
                    action.capability_version().clone(),
                    digest.clone(),
                    action.risk_class(),
                    action.side_effect_class(),
                    action.required_authorization(),
                    proposal.arguments.data_class(),
                    proposal.requested_by,
                );
                if group_identity
                    .as_ref()
                    .is_some_and(|prior| prior != &identity)
                {
                    return Err(EngineError::InvalidPlan);
                }
                group_identity.get_or_insert(identity);
            }
        }

        let mut steps = Vec::with_capacity(plan.len());
        let mut bindings = Vec::with_capacity(prepared.len());
        for item in plan {
            match item {
                crate::MixedCapabilityPlanStep::ApprovalGate {
                    step_id,
                    sequence,
                    actions,
                } => {
                    let action_facts: Vec<Value> = actions
                        .iter()
                        .map(|action_id| {
                            let (action, _) =
                                prepared.get(action_id).ok_or(EngineError::InvalidPlan)?;
                            Ok(json!({
                                "arguments_digest": action.arguments_digest().as_str(),
                                "step_id": action_id.as_str(),
                            }))
                        })
                        .collect::<Result<_, EngineError>>()?;
                    let input_json = serea_protocol::canonicalize(
                        &serde_json::to_string(
                            &json!({"actions": action_facts, "gate": "R2_ACTION_GROUP"}),
                        )
                        .map_err(|_| EngineError::InvalidPlan)?,
                    )
                    .map_err(|_| EngineError::InvalidPlan)?;
                    let input_digest = serea_protocol::digest_of(
                        std::str::from_utf8(&input_json).map_err(|_| EngineError::InvalidPlan)?,
                    )
                    .map_err(|_| EngineError::InvalidPlan)?;
                    let step = serea_protocol::TaskStep::new(serea_protocol::TaskStepDraft {
                        task_id: task_id.clone(),
                        step_id,
                        sequence,
                        kind: serea_protocol::StepKind::WaitApproval,
                        status: serea_protocol::StepStatus::new("PLANNED")
                            .map_err(|_| EngineError::InvalidPlan)?,
                        attempt: 0,
                        idempotency_key: None,
                        provider_id: None,
                        capability_id: None,
                        capability_version: None,
                        input_digest,
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
                    steps.push(StepInput { step, input_json });
                }
                crate::MixedCapabilityPlanStep::Capability(proposal) => {
                    let (prepared_action, descriptor_digest) = prepared
                        .remove(&proposal.step_id)
                        .ok_or(EngineError::InvalidPlan)?;
                    let input_json =
                        serde_json::to_vec(&Value::Object(prepared_action.arguments().clone()))
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
                        idempotency_key: Some(prepared_action.idempotency_key().clone()),
                        provider_id: Some(prepared_action.provider_id().clone()),
                        capability_id: Some(proposal.capability_id),
                        capability_version: Some(prepared_action.capability_version().clone()),
                        input_digest: prepared_action.arguments_digest().clone(),
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
                        descriptor_digest,
                    });
                }
            }
        }
        let fallback_capability_id = fallback_capability_id.ok_or(EngineError::InvalidPlan)?;

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
                    capability_id: fallback_capability_id.clone(),
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
