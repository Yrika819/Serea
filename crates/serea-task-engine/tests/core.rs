//! Test-first P2F-b contracts. The first RED is the absent engine library/API,
//! not a deliberately incorrect production implementation.

use serea_protocol::{
    ActorId, ActorKind, AttemptBudget, Clock, DataClass, EpochMillis, RiskClass, SemVer, StepId,
    StepKind, StepStatus, TaskId, TaskKind, TaskOrigin, TaskOriginKind, TaskState, TaskStep,
    TaskStepDraft, TaskTitle, digest_of,
};
use serea_storage::{Store, TransitionContext};
use serea_task_engine::{
    EngineError, NewTask, Plan, PlanStep, TaskEngine, legal_task_transition, task_transition_reason,
};

const TASK: &str = "tsk_01JQ8Z9K3M7QWXR4V2T6YH0BNA";
const STEP: &str = "stp_01JQ8Z9M3R2CVN8H5FWK7PQDSF";

fn at(value: i64) -> EpochMillis {
    EpochMillis::new(value).unwrap()
}

struct Fixed;
impl Clock for Fixed {
    fn now_ms(&self) -> Result<EpochMillis, serea_protocol::ProtocolError> {
        Ok(at(0))
    }
}

fn engine() -> TaskEngine {
    TaskEngine::new(Store::open_in_memory(&Fixed).unwrap())
}

fn context<'a>(actor: &'a ActorId, version: &'a SemVer) -> TransitionContext<'a> {
    TransitionContext {
        actor_kind: ActorKind::User,
        actor_id: actor,
        actor_version: version,
        causation_id: None,
    }
}

fn spec(class: DataClass) -> NewTask {
    NewTask {
        task_id: TaskId::new(TASK).unwrap(),
        kind: TaskKind::UserRequest,
        title: TaskTitle::new("Test-first task").unwrap(),
        origin: TaskOrigin {
            kind: TaskOriginKind::new("USER_MESSAGE").unwrap(),
            device_id: None,
            message_id: None,
            extensions: [(
                "future_origin".into(),
                serde_json::json!({"retained": true}),
            )]
            .into_iter()
            .collect(),
        },
        data_class: class,
        policy_class: RiskClass::Observe,
        attempt_budget: AttemptBudget {
            max_model_calls: 12,
            max_tool_calls: 24,
            max_attempts_per_step: 3,
            extensions: [("future_budget".into(), serde_json::json!([1, 2, 3]))]
                .into_iter()
                .collect(),
        },
        created_at: at(10),
        deadline_at: Some(at(1000)),
        extensions: [(
            "future_task".into(),
            serde_json::json!({"nested": [true, null]}),
        )]
        .into_iter()
        .collect(),
    }
}

fn planned(kind: StepKind) -> PlanStep {
    let input_json = b"{\"instruction\":\"record supplied result\"}".to_vec();
    let step = TaskStep::new(TaskStepDraft {
        step_id: StepId::new(STEP).unwrap(),
        task_id: TaskId::new(TASK).unwrap(),
        sequence: 10,
        kind,
        status: StepStatus::new("PLANNED").unwrap(),
        attempt: 0,
        idempotency_key: None,
        provider_id: None,
        capability_id: None,
        capability_version: None,
        input_digest: digest_of(std::str::from_utf8(&input_json).unwrap()).unwrap(),
        result_digest: None,
        side_effect_receipt: None,
        started_at: None,
        completed_at: None,
        lease_owner: None,
        lease_expires_at: None,
        lease_generation: None,
        error: None,
        extensions: [("future_step".into(), serde_json::json!({"unchanged": 7}))]
            .into_iter()
            .collect(),
    })
    .unwrap();
    PlanStep { step, input_json }
}

// Literal transcription of Task Protocol §4.2. This does not derive expected
// values from either production legality or production reason mapping.
const STATES: [TaskState; 11] = [
    TaskState::Received,
    TaskState::Planning,
    TaskState::Ready,
    TaskState::Executing,
    TaskState::WaitingApproval,
    TaskState::WaitingUser,
    TaskState::Verifying,
    TaskState::Blocked,
    TaskState::Completed,
    TaskState::Failed,
    TaskState::Cancelled,
];
const EXPECTED: [[u8; 11]; 11] = [
    [0, 1, 0, 0, 0, 0, 0, 0, 0, 1, 1],
    [0, 0, 1, 0, 1, 1, 0, 1, 0, 1, 1],
    [0, 1, 0, 1, 1, 0, 0, 0, 0, 1, 1],
    [0, 0, 1, 0, 1, 1, 1, 1, 0, 1, 1],
    [0, 0, 1, 0, 0, 0, 0, 0, 0, 1, 1],
    [0, 1, 1, 0, 0, 0, 0, 0, 0, 1, 1],
    [0, 0, 0, 1, 0, 0, 0, 1, 1, 1, 1],
    [0, 1, 1, 0, 0, 0, 0, 0, 0, 1, 1],
    [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
];

#[test]
fn every_one_of_the_121_task_state_pairs_matches_the_frozen_table() {
    let mut names = STATES.map(TaskState::wire_name).to_vec();
    names.sort_unstable();
    let mut protocol_names = TaskState::WIRE_NAMES.to_vec();
    protocol_names.sort_unstable();
    assert_eq!(
        names, protocol_names,
        "state oracle must cover protocol enum"
    );
    let mut legal = 0;
    for (row, from) in STATES.into_iter().enumerate() {
        for (column, to) in STATES.into_iter().enumerate() {
            let expected = EXPECTED[row][column] == 1;
            assert_eq!(
                legal_task_transition(from, to),
                expected,
                "{from} -> {to} differs from Task Protocol"
            );
            assert_eq!(task_transition_reason(from, to).is_some(), expected);
            legal += usize::from(expected);
        }
    }
    assert_eq!(legal, 37);
}

#[test]
fn terminal_states_have_no_outgoing_transition() {
    for terminal in [
        TaskState::Completed,
        TaskState::Failed,
        TaskState::Cancelled,
    ] {
        for to in STATES {
            assert!(!legal_task_transition(terminal, to));
            assert!(task_transition_reason(terminal, to).is_none());
        }
    }
}

#[test]
fn creation_preserves_host_identity_and_complete_received_projection() {
    let mut engine = engine();
    let actor = ActorId::new("test-user").unwrap();
    let version = SemVer::new("0.1.0").unwrap();
    let context = context(&actor, &version);
    let spec = spec(DataClass::Personal);
    let expected_id = spec.task_id.clone();
    let expected_origin = spec.origin.clone();
    let expected_budget = spec.attempt_budget.clone();
    let expected_extensions = spec.extensions.clone();
    let created = engine.create_task(spec, &context).unwrap();
    assert_eq!(created.task.task_id, expected_id);
    assert_eq!(created.task.state, TaskState::Received);
    assert_eq!(created.task.policy_class, RiskClass::Observe);
    assert_eq!(created.task.data_class, DataClass::Personal);
    assert_eq!(created.task.origin, expected_origin);
    assert_eq!(created.task.attempt_budget, expected_budget);
    assert_eq!(created.task.extensions, expected_extensions);
    assert_eq!(created.task.created_at.to_epoch_millis(), at(10));
    assert_eq!(created.task.updated_at.to_epoch_millis(), at(10));
    assert_eq!(
        created.task.deadline_at.unwrap().to_epoch_millis(),
        at(1000)
    );
    assert_eq!(created.plan_revision, 0);
    assert!(created.steps.is_empty());
    assert!(created.task.steps.is_empty());
    assert!(created.task.blocked_reason.is_none());
    assert!(created.task.failure_reason.is_none());
    assert!(created.task.cancelled_at.is_none());
    assert!(created.task.cancelled_by.is_none());
}

#[test]
fn load_round_trips_task_and_all_nested_extensions() {
    let mut engine = engine();
    let actor = ActorId::new("test-user").unwrap();
    let version = SemVer::new("0.1.0").unwrap();
    let context = context(&actor, &version);
    let created = engine
        .create_task(spec(DataClass::Personal), &context)
        .unwrap();
    let loaded = engine.load(TaskId::new(TASK).unwrap()).unwrap();
    assert_eq!(loaded.task, created.task);
    assert_eq!(loaded.plan_revision, created.plan_revision);
    assert!(loaded.steps.is_empty());
}

#[test]
fn received_to_planning_is_explicit_and_stale_writer_persists_nothing() {
    let mut engine = engine();
    let actor = ActorId::new("test-user").unwrap();
    let version = SemVer::new("0.1.0").unwrap();
    let context = context(&actor, &version);
    engine
        .create_task(spec(DataClass::Public), &context)
        .unwrap();
    let task_id = TaskId::new(TASK).unwrap();
    assert_eq!(
        engine.load(task_id.clone()).unwrap().task.state,
        TaskState::Received
    );
    let planning = engine
        .start_planning(task_id.clone(), TaskState::Received, 0, at(20), &context)
        .unwrap();
    assert_eq!(planning.task.state, TaskState::Planning);
    let stale = engine.start_planning(task_id.clone(), TaskState::Received, 0, at(30), &context);
    assert!(stale.is_err());
    assert_eq!(engine.load(task_id).unwrap().task, planning.task);
}

#[test]
fn persist_initial_plan_makes_ready_and_preserves_unstarted_step_extensions() {
    let mut engine = engine();
    let actor = ActorId::new("test-user").unwrap();
    let version = SemVer::new("0.1.0").unwrap();
    let context = context(&actor, &version);
    let task_id = TaskId::new(TASK).unwrap();
    engine
        .create_task(spec(DataClass::Personal), &context)
        .unwrap();
    engine
        .start_planning(task_id.clone(), TaskState::Received, 0, at(20), &context)
        .unwrap();
    let step = planned(StepKind::Notify);
    let expected = step.step.clone();
    engine
        .persist_plan(
            task_id.clone(),
            Plan {
                revision: 1,
                steps: vec![step],
            },
            at(30),
            &context,
        )
        .unwrap();
    let loaded = engine.load(task_id).unwrap();
    assert_eq!(loaded.task.state, TaskState::Ready);
    assert_eq!(loaded.plan_revision, 1);
    assert_eq!(loaded.steps.len(), 1);
    assert_eq!(loaded.steps[0].step, expected);
    assert_eq!(loaded.steps[0].plan_revision, 1);
    assert_eq!(loaded.task.steps, vec![expected]);
}

#[test]
fn ordinary_creation_fails_closed_for_private_secret_and_credential() {
    let actor = ActorId::new("test-user").unwrap();
    let version = SemVer::new("0.1.0").unwrap();
    let context = context(&actor, &version);
    for class in [DataClass::Private, DataClass::Secret, DataClass::Credential] {
        let mut engine = engine();
        let error = engine.create_task(spec(class), &context).err().unwrap();
        assert!(matches!(error, EngineError::UnsupportedDataClass));
        assert!(matches!(
            engine.load(TaskId::new(TASK).unwrap()).err().unwrap(),
            EngineError::TaskNotFound
        ));
    }
}

#[test]
fn wait_kinds_are_valid_unstarted_plan_steps_without_fabricated_authority() {
    for kind in [
        StepKind::WaitApproval,
        StepKind::WaitUser,
        StepKind::WaitSchedule,
    ] {
        let mut engine = engine();
        let actor = ActorId::new("test-user").unwrap();
        let version = SemVer::new("0.1.0").unwrap();
        let context = context(&actor, &version);
        let task_id = TaskId::new(TASK).unwrap();
        engine
            .create_task(spec(DataClass::Public), &context)
            .unwrap();
        engine
            .start_planning(task_id.clone(), TaskState::Received, 0, at(20), &context)
            .unwrap();
        engine
            .persist_plan(
                task_id.clone(),
                Plan {
                    revision: 1,
                    steps: vec![planned(kind)],
                },
                at(30),
                &context,
            )
            .unwrap();
        let loaded = engine.load(task_id).unwrap();
        let step = &loaded.steps[0].step;
        assert_eq!(step.kind, kind);
        assert_eq!(step.status.as_str(), "PLANNED");
        assert_eq!(step.attempt, 0);
        assert!(step.lease_generation.is_none());
        assert!(step.started_at.is_none());
        assert!(step.completed_at.is_none());
        assert!(step.result_digest.is_none());
        assert!(step.idempotency_key.is_none());
    }
}
