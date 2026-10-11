use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use serea_capability::{
    CapabilityAvailabilitySnapshotV1, CapabilityManifestV1, CapabilitySchemaCatalogV1,
    HostEligibility, ManifestEntryV1, ProviderRegistry, install,
};
use serea_event_bus::EventBus;
use serea_policy::{
    ApprovalActionSetV1, ApprovalActionV1, ApprovalAuthenticationError,
    ApprovalAuthenticationStrengthV1, ApprovalAuthenticationVerifier, ApprovalPrincipalV1,
    ApprovalRequestDraftV1, ApprovalRequestV1, ApprovalResponseCandidateV1,
    ApprovalResponseDecisionV1, AuthenticatedApprovalResponseV1, PolicyDecision, PolicyRule,
    RuleDecision, RuleMatch, SafeApprovalSummaryV1,
};
use serea_protocol::provider::CapabilityProvider;
use serea_protocol::*;
use serea_scheduler::Scheduler;
use serea_storage::{PolicyActorClass, Store};
use serea_task_engine::{
    CapabilityPlanStep, MixedCapabilityPlanStep, NewTask, TaskEngine, TransitionContext,
};
use serea_testkit::DeterministicUlidSource;

static INVOCATIONS: AtomicUsize = AtomicUsize::new(0);

struct Fixed;
impl Clock for Fixed {
    fn now_ms(&self) -> Result<EpochMillis, ProtocolError> {
        EpochMillis::new(1_700_000_000_000)
    }
}

fn at(value: i64) -> EpochMillis {
    EpochMillis::new(value).unwrap()
}

fn database_path() -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "serea-scheduler-task-approval-{}-{}.sqlite",
        std::process::id(),
        at(1).get()
    ))
}

fn task_id() -> TaskId {
    TaskId::new("tsk_00000000000000000000000031").unwrap()
}

fn gate_id() -> StepId {
    StepId::new("stp_00000000000000000000000031").unwrap()
}

fn action_step_id() -> StepId {
    StepId::new("stp_00000000000000000000000032").unwrap()
}

fn task_spec() -> NewTask {
    NewTask {
        task_id: task_id(),
        kind: TaskKind::UserRequest,
        title: TaskTitle::new("Read my calendar").unwrap(),
        origin: TaskOrigin {
            kind: TaskOriginKind::new("USER_MESSAGE").unwrap(),
            device_id: None,
            message_id: None,
            extensions: Default::default(),
        },
        data_class: DataClass::Personal,
        policy_class: RiskClass::Observe,
        attempt_budget: AttemptBudget {
            max_model_calls: 1,
            max_tool_calls: 1,
            max_attempts_per_step: 1,
            extensions: Default::default(),
        },
        created_at: at(10),
        deadline_at: Some(at(1_000)),
        extensions: Default::default(),
    }
}

fn descriptor() -> CapabilityDescriptor {
    CapabilityDescriptor::new(CapabilityDescriptorDraft {
        id: CapabilityId::new("calendar.events.read").unwrap(),
        version: SemVer::new("1.0.0").unwrap(),
        title: DescriptorTitle::new("Read calendar events").unwrap(),
        description: DescriptorDescription::new("Read private calendar events").unwrap(),
        provider_id: ProviderId::new("calendar").unwrap(),
        implementation_id: None,
        input_schema: JsonSchemaRef::new("https://serea.local/schemas/cal-input.json").unwrap(),
        output_schema: JsonSchemaRef::new("https://serea.local/schemas/cal-output.json").unwrap(),
        side_effect_class: SideEffectClass::None,
        risk_class: RiskClass::Observe,
        required_authorization: Authorization::None,
        replay_safety: ReplaySafety::Idempotent,
        data_class: DataClass::Personal,
        root_requirement: RootRequirement::NotRequired,
        idempotency_support: IdempotencySupport::None,
        max_duration_ms: 5_000,
        cost_class: CostClass::Free,
        experimental: false,
    })
    .unwrap()
}

struct ReadProvider(CapabilityDescriptor);

#[async_trait::async_trait]
impl CapabilityProvider for ReadProvider {
    fn provider_id(&self) -> ProviderId {
        self.0.provider_id().clone()
    }

    fn capabilities(&self) -> Vec<CapabilityDescriptor> {
        vec![self.0.clone()]
    }

    async fn invoke(
        &self,
        _: &ActionRequest,
        _: &ProviderContext,
    ) -> Result<ActionResult, ActionError> {
        INVOCATIONS.fetch_add(1, Ordering::SeqCst);
        panic!("P6 approval path must never invoke a provider")
    }

    async fn health(&self) -> ProviderHealth {
        ProviderHealth::Ready
    }
}

fn install_capability(
    store: &Store,
    events: &EventBus,
    descriptor: CapabilityDescriptor,
) -> CapabilityAvailabilitySnapshotV1 {
    let input_uri = descriptor.input_schema().as_str().to_owned();
    let output_uri = descriptor.output_schema().as_str().to_owned();
    let mut documents = BTreeMap::new();
    documents.insert(
        input_uri.clone(),
        r#"{"$schema":"https://json-schema.org/draft/2020-12/schema","type":"object","additionalProperties":false,"properties":{"calendar":{"type":"string","maxLength":64}}}"#.to_owned(),
    );
    documents.insert(
        output_uri.clone(),
        r#"{"$schema":"https://json-schema.org/draft/2020-12/schema","type":"object","additionalProperties":false,"properties":{"ok":{"type":"boolean"}}}"#.to_owned(),
    );
    let catalog = CapabilitySchemaCatalogV1::build(documents).unwrap();
    let entry = ManifestEntryV1 {
        descriptor: descriptor.clone(),
        candidate_priority: 0,
        descriptor_digest: serea_capability::descriptor_semantic_digest(&descriptor, &catalog)
            .unwrap(),
        input_schema_digest: catalog.document_digest(&input_uri).unwrap().clone(),
        output_schema_digest: catalog.document_digest(&output_uri).unwrap().clone(),
    };
    let manifest = CapabilityManifestV1::build(
        vec![entry.clone()],
        vec![(descriptor.id().clone(), descriptor.version().clone())],
        catalog,
    )
    .unwrap();
    let provider: Arc<dyn CapabilityProvider> = Arc::new(ReadProvider(descriptor.clone()));
    let registry = ProviderRegistry::build(vec![provider.clone()]).unwrap();
    registry.validate_advertisements(&manifest).unwrap();
    let installed = install(store, events, &manifest, &registry, at(15)).unwrap();
    assert!(installed.activated());

    let mut eligibility = HashMap::new();
    eligibility.insert(entry.descriptor_digest, true);
    block_on(CapabilityAvailabilitySnapshotV1::build_for_generation(
        manifest,
        installed.generation_id(),
        &registry,
        store,
        HostEligibility::new(eligibility),
    ))
    .unwrap()
}

fn args() -> serea_capability::ClassifiedArgumentsV1 {
    let mut value = serde_json::Map::new();
    value.insert("calendar".into(), serde_json::Value::String("work".into()));
    serea_capability::ClassifiedArgumentsV1::new_trusted(value, DataClass::Personal)
}

struct Context {
    actor: ActorId,
    version: SemVer,
    cause: EventId,
}

impl Context {
    fn new() -> Self {
        Self {
            actor: ActorId::new("workflow-host").unwrap(),
            version: SemVer::new("0.2.0").unwrap(),
            cause: EventId::new("evt_00000000000000000000000031").unwrap(),
        }
    }

    fn view(&self) -> TransitionContext<'_> {
        TransitionContext {
            actor_kind: ActorKind::Host,
            actor_id: &self.actor,
            actor_version: &self.version,
            causation_id: Some(&self.cause),
        }
    }
}

fn block_on<F: std::future::Future>(future: F) -> F::Output {
    use std::task::{Context as TaskContext, Poll, Wake, Waker};
    struct ThreadWake(std::thread::Thread);
    impl Wake for ThreadWake {
        fn wake(self: Arc<Self>) {
            self.0.unpark();
        }
        fn wake_by_ref(self: &Arc<Self>) {
            self.0.unpark();
        }
    }
    let waker = Waker::from(Arc::new(ThreadWake(std::thread::current())));
    let mut context = TaskContext::from_waker(&waker);
    let mut future = Box::pin(future);
    loop {
        match future.as_mut().poll(&mut context) {
            Poll::Ready(output) => return output,
            Poll::Pending => std::thread::park(),
        }
    }
}

struct AcceptAuth;
impl ApprovalAuthenticationVerifier for AcceptAuth {
    fn verify(&self, _: &ApprovalResponseCandidateV1) -> Result<(), ApprovalAuthenticationError> {
        Ok(())
    }
}

#[test]
fn authenticated_approval_routes_through_public_scheduler_and_task_engine_rechecks_policy() {
    INVOCATIONS.store(0, Ordering::SeqCst);
    let path = database_path();
    let _ = std::fs::remove_file(&path);
    let events = EventBus::new(DeterministicUlidSource::starting_at(1_700_800_000_000).unwrap());
    let store = Store::open(&path, &Fixed).unwrap();
    let descriptor = descriptor();
    let availability = install_capability(&store, &events, descriptor.clone());

    let require_approval = PolicyRule::new(
        "require_read_approval",
        10,
        true,
        RuleMatch {
            capability_id: Some(descriptor.id().clone()),
            ..Default::default()
        },
        RuleDecision::RequireApproval,
    )
    .unwrap();
    let revision = serea_policy::prepare_revision(
        &store,
        vec![require_approval],
        at(20),
        PolicyActorClass::Host,
        ActorId::new("policy-host").unwrap(),
        ReasonCode::new("POLICY_IMPORT").unwrap(),
    )
    .unwrap();
    serea_policy::activate_revision(
        &store,
        &events,
        &revision,
        at(21),
        SemVer::new("0.2.0").unwrap(),
    )
    .unwrap();

    let context = Context::new();
    let mut engine = TaskEngine::new(Store::open(&path, &Fixed).unwrap(), events.clone());
    engine.create_task(task_spec(), &context.view()).unwrap();
    engine
        .start_planning(task_id(), TaskState::Received, 0, at(25), &context.view())
        .unwrap();
    engine
        .create_mixed_capability_plan(
            &availability,
            task_id(),
            vec![
                MixedCapabilityPlanStep::ApprovalGate {
                    step_id: gate_id(),
                    sequence: 1,
                    actions: vec![action_step_id()],
                },
                MixedCapabilityPlanStep::Capability(CapabilityPlanStep {
                    step_id: action_step_id(),
                    sequence: 2,
                    capability_id: descriptor.id().clone(),
                    arguments: args(),
                    requested_by: RequestedBy::Model,
                }),
            ],
            at(30),
            &context.view(),
        )
        .unwrap();

    let prepared = serea_capability::prepare_action(
        &availability,
        descriptor.id(),
        task_id(),
        action_step_id(),
        &args(),
        RequestedBy::Model,
        None,
        None,
    )
    .unwrap();
    let evaluation = engine.evaluate_prepared_action(&prepared).unwrap();
    assert!(matches!(
        evaluation.decision,
        PolicyDecision::RequireApproval(_)
    ));
    let facts = evaluation.evidence.input_facts();
    let approval_id = ApprovalId::new("apr_01JQ8ZB8H2XKM9P4QW7NRT5YCD").unwrap();
    let request = ApprovalRequestV1::new(ApprovalRequestDraftV1 {
        approval_id: approval_id.clone(),
        task_id: task_id(),
        task_kind: facts.task_kind,
        plan_revision: facts.plan_revision,
        capability_id: facts.capability_id.clone(),
        capability_version: facts.capability_version.clone(),
        registry_generation: facts.registry_generation,
        descriptor_digest: facts.descriptor_digest.clone(),
        risk_class: facts.risk_class,
        side_effect_class: facts.side_effect_class,
        authorization: facts.authorization,
        data_class: facts.data_class,
        requested_by: facts.requested_by,
        actions: ApprovalActionSetV1::new(vec![
            ApprovalActionV1::new(
                action_step_id(),
                prepared.arguments_digest().clone(),
                serde_json::json!({"calendar":"work"}),
                DataClass::Personal,
            )
            .unwrap(),
        ])
        .unwrap(),
        summary: SafeApprovalSummaryV1::from_host_builder(
            "calendar.events.read",
            "Read calendar events",
            DataClass::Personal,
        )
        .unwrap(),
        raised_at: at(40),
        expires_at: at(900),
    })
    .unwrap();
    let result = engine
        .request_approval(
            request.clone(),
            &[evaluation.evidence],
            at(40),
            &context.view(),
        )
        .unwrap();
    assert_eq!(result.task_state, TaskState::WaitingApproval);
    let waiting = store.load_task(&task_id()).unwrap();
    assert_eq!(waiting.task.state, TaskState::WaitingApproval);
    assert_eq!(
        waiting.steps[0].step.status,
        StepStatus::new("WAITING").unwrap()
    );

    let candidate = ApprovalResponseCandidateV1::new(
        approval_id,
        ApprovalResponseDecisionV1::Grant,
        ApprovalPrincipalV1::User,
        Some(DeviceId::new("dev_01JQ8ZB8H2XKM9P4QW7NRT5YCD").unwrap()),
        Some(SessionId::new("ses_01JQ8ZB8H2XKM9P4QW7NRT5YCD").unwrap()),
        ApprovalAuthenticationStrengthV1::Session,
        at(50),
        None,
        vec![action_step_id()],
        Some(1),
        Some(at(800)),
    );
    let authenticated =
        AuthenticatedApprovalResponseV1::authenticate(candidate, &AcceptAuth).unwrap();
    let response = authenticated.validate_for(&request, at(50)).unwrap();
    let response_actor = ActorId::new("user-approval-port").unwrap();
    let response_event = events
        .draft_approval_lifecycle(
            serea_event_bus::ApprovalLifecycleKindV1::Granted,
            &request.draft().approval_id,
            &task_id(),
            &action_step_id(),
            ActorKind::User,
            response_actor.clone(),
            SemVer::new("0.2.0").unwrap(),
            at(50),
        )
        .unwrap();
    let response_outcome = store
        .respond_to_approval(
            response.storage_draft(
                Some(GrantId::new("grt_01JQ8ZB8H2XKM9P4QW7NRT5YCE").unwrap()),
                response_actor,
            ),
            response_event,
        )
        .unwrap();
    assert_eq!(
        response_outcome,
        serea_storage::ApprovalResponseStorageOutcome::Granted(
            GrantId::new("grt_01JQ8ZB8H2XKM9P4QW7NRT5YCE").unwrap()
        )
    );
    let grant_conn = rusqlite::Connection::open(&path).unwrap();
    let grant_member: (String, String) = grant_conn
        .query_row(
            "SELECT step_id, arguments_digest FROM approval_grant_members WHERE grant_id=?1",
            ["grt_01JQ8ZB8H2XKM9P4QW7NRT5YCE"],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert_eq!(grant_member.0, action_step_id().as_str());
    assert_eq!(grant_member.1, prepared.arguments_digest().as_str());
    drop(grant_conn);
    assert_eq!(
        store.load_task(&task_id()).unwrap().task.state,
        TaskState::WaitingApproval,
        "authenticated response persists Grant but only Scheduler wake resumes Task"
    );

    let allow = PolicyRule::new(
        "current_policy_allows_recheck",
        10,
        true,
        RuleMatch {
            capability_id: Some(descriptor.id().clone()),
            ..Default::default()
        },
        RuleDecision::Allow,
    )
    .unwrap();
    let current_revision = serea_policy::prepare_revision(
        &store,
        vec![allow],
        at(55),
        PolicyActorClass::Host,
        ActorId::new("policy-host").unwrap(),
        ReasonCode::new("POLICY_CHANGE").unwrap(),
    )
    .unwrap();
    serea_policy::activate_revision(
        &store,
        &events,
        &current_revision,
        at(56),
        SemVer::new("0.2.0").unwrap(),
    )
    .unwrap();

    let scheduler = Scheduler::new(
        Store::open(&path, &Fixed).unwrap(),
        TaskEngine::new(Store::open(&path, &Fixed).unwrap(), events.clone()),
        events.clone(),
    );
    let lease = scheduler
        .claim_event_consumer("task-engine-approval-e2e", at(60), at(800))
        .unwrap();
    scheduler.replay_host_events(&lease, at(60), 64).unwrap();
    assert_eq!(
        scheduler
            .pending_approval_lifecycle_wakes(16)
            .unwrap()
            .len(),
        1
    );
    drop(scheduler);
    let mut scheduler = Scheduler::new(
        Store::open(&path, &Fixed).unwrap(),
        TaskEngine::new(Store::open(&path, &Fixed).unwrap(), events.clone()),
        events.clone(),
    );
    assert_eq!(
        scheduler
            .pending_approval_lifecycle_wakes(16)
            .unwrap()
            .len(),
        1,
        "restart before wake application preserves the durable routing handoff"
    );
    let applied = scheduler
        .process_approval_lifecycle_wakes(at(61), 16)
        .unwrap();
    assert_eq!(
        applied,
        vec![serea_storage::ApprovalWakeApplyOutcome::Applied {
            task_id: task_id(),
            state: TaskState::Ready,
        }]
    );
    assert!(
        scheduler
            .pending_approval_lifecycle_wakes(16)
            .unwrap()
            .is_empty()
    );
    assert!(
        scheduler
            .process_approval_lifecycle_wakes(at(62), 16)
            .unwrap()
            .is_empty()
    );
    let resumed = Store::open(&path, &Fixed)
        .unwrap()
        .load_task(&task_id())
        .unwrap();
    assert_eq!(resumed.task.state, TaskState::Ready);
    assert_eq!(
        resumed.steps[0].step.status,
        StepStatus::new("SUCCEEDED").unwrap()
    );
    assert_eq!(
        resumed.steps[1].step.status,
        StepStatus::new("PLANNED").unwrap()
    );

    let again = serea_capability::prepare_action(
        &availability,
        descriptor.id(),
        task_id(),
        action_step_id(),
        &args(),
        RequestedBy::Model,
        None,
        None,
    )
    .unwrap();
    let reevaluated = TaskEngine::new(Store::open(&path, &Fixed).unwrap(), events.clone())
        .evaluate_prepared_action(&again)
        .unwrap();
    assert_eq!(reevaluated.decision, PolicyDecision::Allow);
    assert_eq!(
        reevaluated.evidence.policy_revision_id(),
        current_revision.revision_id()
    );
    assert_eq!(INVOCATIONS.load(Ordering::SeqCst), 0);
    drop(scheduler);
    drop(engine);
    drop(store);
    let _ = std::fs::remove_file(path);
}
