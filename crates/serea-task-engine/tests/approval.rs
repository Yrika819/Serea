use serea_event_bus::EventBus;
use serea_policy::{
    ApprovalActionSetV1, ApprovalActionV1, ApprovalRequestDraftV1, ApprovalRequestV1,
    AutomationContext, PolicyDecision, PolicyInputDraft, PolicyInputV1, PolicyRule, RuleDecision,
    RuleMatch, SafeApprovalSummaryV1, activate_revision, evaluate_with_evidence, prepare_revision,
};
use serea_protocol::*;
use serea_storage::{PolicyActorClass, Store};
use serea_task_engine::{EngineError, TaskEngine, TransitionContext};
use serea_testkit::{DeterministicUlidSource, TestClock};

fn at(value: i64) -> EpochMillis {
    EpochMillis::new(value).unwrap()
}

fn input(revision: &serea_policy::PolicyRevision, task_id: TaskId) -> PolicyInputV1 {
    PolicyInputV1::new(
        PolicyInputDraft {
            task_id,
            step_id: StepId::new("stp_01JQ8ZB8H2XKM9P4QW7NRT5YCE").unwrap(),
            task_kind: TaskKind::UserRequest,
            task_state: TaskState::Executing,
            task_cancelled: false,
            task_deadline_at: Some(at(50_000)),
            task_data_class: DataClass::Personal,
            task_policy_class: RiskClass::ExternalWrite,
            capability_id: CapabilityId::new("calendar.events.create").unwrap(),
            capability_version: SemVer::new("1.0.0").unwrap(),
            provider_id: ProviderId::new("calendar").unwrap(),
            implementation_id: None,
            risk_class: RiskClass::ExternalWrite,
            side_effect_class: SideEffectClass::ExternalWrite,
            authorization: Authorization::None,
            root_requirement: RootRequirement::NotRequired,
            data_class: DataClass::Personal,
            requested_by: RequestedBy::Model,
            device_state: None,
            capability_enabled: true,
            destructive_enabled: false,
            plan_revision: 1,
            registry_generation: 1,
            generation_digest: digest_of("\"registry\"").unwrap(),
            descriptor_digest: digest_of("\"descriptor\"").unwrap(),
            arguments_digest: digest_of("{\"summary\":\"exact action\"}").unwrap(),
        },
        revision,
    )
    .unwrap()
}

#[test]
fn request_approval_rejects_policy_evidence_for_another_task_before_mutation() {
    let store = Store::open_in_memory(&TestClock::default()).unwrap();
    let events = EventBus::new(DeterministicUlidSource::starting_at(1_700_000_000_000).unwrap());
    let rule = PolicyRule::new(
        "require_calendar_approval",
        10,
        true,
        RuleMatch {
            capability_id: Some(CapabilityId::new("calendar.events.create").unwrap()),
            automation_context: Some(AutomationContext::Interactive),
            ..RuleMatch::default()
        },
        RuleDecision::RequireApproval,
    )
    .unwrap();
    let prepared = prepare_revision(
        &store,
        vec![rule],
        at(10),
        PolicyActorClass::Host,
        ActorId::new("serea-core").unwrap(),
        ReasonCode::new("POLICY_IMPORT").unwrap(),
    )
    .unwrap();
    let revision = activate_revision(
        &store,
        &events,
        &prepared,
        at(11),
        SemVer::new("1.0.0").unwrap(),
    )
    .unwrap();

    let evidence_task_id = TaskId::new("tsk_01JQ8ZB8H2XKM9P4QW7NRT5YCD").unwrap();
    let evaluation = evaluate_with_evidence(&input(&revision, evidence_task_id), &revision);
    assert!(matches!(
        evaluation.decision,
        PolicyDecision::RequireApproval(_)
    ));
    let request_task_id = TaskId::new("tsk_01JQ8ZB8H2XKM9P4QW7NRT5YCF").unwrap();
    let action = ApprovalActionV1::new(
        StepId::new("stp_01JQ8ZB8H2XKM9P4QW7NRT5YCE").unwrap(),
        digest_of("{\"summary\":\"exact action\"}").unwrap(),
        serde_json::json!({"calendar_id":"primary"}),
        DataClass::Personal,
    )
    .unwrap();
    let request = ApprovalRequestV1::new(ApprovalRequestDraftV1 {
        approval_id: ApprovalId::new("apr_01JQ8ZB8H2XKM9P4QW7NRT5YCG").unwrap(),
        task_id: request_task_id,
        task_kind: TaskKind::UserRequest,
        plan_revision: 1,
        capability_id: CapabilityId::new("calendar.events.create").unwrap(),
        capability_version: SemVer::new("1.0.0").unwrap(),
        registry_generation: 1,
        descriptor_digest: digest_of("\"descriptor\"").unwrap(),
        risk_class: RiskClass::ExternalWrite,
        side_effect_class: SideEffectClass::ExternalWrite,
        authorization: Authorization::None,
        data_class: DataClass::Personal,
        requested_by: RequestedBy::Model,
        actions: ApprovalActionSetV1::new(vec![action]).unwrap(),
        summary: SafeApprovalSummaryV1::from_host_builder(
            "calendar.event",
            "Create one calendar event",
            DataClass::Personal,
        )
        .unwrap(),
        raised_at: at(20),
        expires_at: at(1_000),
    })
    .unwrap();
    let actor = ActorId::new("host-test").unwrap();
    let version = SemVer::new("1.0.0").unwrap();
    let context = TransitionContext {
        actor_kind: ActorKind::Host,
        actor_id: &actor,
        actor_version: &version,
        causation_id: None,
    };
    let mut engine = TaskEngine::new(store, events);
    assert!(matches!(
        engine.request_approval(request, &[evaluation.evidence], at(20), &context),
        Err(EngineError::InvalidPlan)
    ));
}
