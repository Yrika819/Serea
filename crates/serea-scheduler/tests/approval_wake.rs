use serea_event_bus::EventBus;
use serea_protocol::*;
use serea_scheduler::Scheduler;
use serea_storage::{ApprovalLifecycleOutcome, Store};
use serea_task_engine::{NewTask, TaskEngine, TransitionContext};
use serea_testkit::DeterministicUlidSource;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

static NEXT_DB: AtomicUsize = AtomicUsize::new(0);

struct Fixed;
impl Clock for Fixed {
    fn now_ms(&self) -> Result<EpochMillis, ProtocolError> {
        EpochMillis::new(1_700_000_000_000)
    }
}

fn at(ms: i64) -> EpochMillis {
    EpochMillis::new(ms).unwrap()
}

fn db_path() -> PathBuf {
    std::env::temp_dir().join(format!(
        "serea-approval-wake-{}-{}.sqlite",
        std::process::id(),
        NEXT_DB.fetch_add(1, Ordering::Relaxed)
    ))
}

fn task_spec(task_id: TaskId) -> NewTask {
    NewTask {
        task_id,
        kind: TaskKind::UserRequest,
        title: TaskTitle::new("Approval wake test").unwrap(),
        origin: TaskOrigin {
            kind: TaskOriginKind::new("USER_MESSAGE").unwrap(),
            device_id: None,
            message_id: None,
            extensions: Default::default(),
        },
        data_class: DataClass::Public,
        policy_class: RiskClass::Observe,
        attempt_budget: AttemptBudget {
            max_model_calls: 1,
            max_tool_calls: 1,
            max_attempts_per_step: 1,
            extensions: Default::default(),
        },
        created_at: at(0),
        deadline_at: None,
        extensions: Default::default(),
    }
}

fn approval_event(
    kind: EventKind,
    event_id: EventId,
    task_id: &TaskId,
    step_id: &StepId,
    payload_task_id: &str,
) -> SereaEvent {
    SereaEvent {
        envelope_version: EnvelopeVersion::new("1").unwrap(),
        surface: WireSurface::new(WireSurface::EVENT).unwrap(),
        message_id: event_id,
        seq: Seq::new(0),
        kind,
        occurred_at: Timestamp::from_epoch_millis(at(1)),
        correlation_id: Some(task_id.clone()),
        causation_id: None,
        actor: Actor {
            kind: ActorKind::Host,
            id: ActorId::new("host-test").unwrap(),
            version: SemVer::new("1.0.0").unwrap(),
            extensions: Default::default(),
        },
        data_class: DataClass::Public,
        trace: Some(Trace {
            task_id: Some(task_id.clone()),
            step_id: Some(step_id.clone()),
            ..Trace::default()
        }),
        payload: serde_json::json!({
            "approval_id": "apr_00000000000000000000000001",
            "task_id": payload_task_id,
            "step_id": step_id.as_str(),
        })
        .as_object()
        .unwrap()
        .clone(),
        extensions: Default::default(),
    }
}

#[test]
fn approval_events_materialize_durable_routing_only_wakes_and_require_explicit_ack() {
    let path = db_path();
    let bus = EventBus::new(DeterministicUlidSource::starting_at(1_700_300_000_000).unwrap());
    let store = Store::open(&path, &Fixed).unwrap();
    let task_id = TaskId::new("tsk_00000000000000000000000001").unwrap();
    let step_id = StepId::new("stp_00000000000000000000000001").unwrap();
    let mut tasks = TaskEngine::new(Store::open(&path, &Fixed).unwrap(), bus.clone());
    let actor = ActorId::new("host-test").unwrap();
    let version = SemVer::new("1.0.0").unwrap();
    let context = TransitionContext {
        actor_kind: ActorKind::Host,
        actor_id: &actor,
        actor_version: &version,
        causation_id: None,
    };
    tasks
        .create_task(task_spec(task_id.clone()), &context)
        .unwrap();

    let source = Store::open(&path, &Fixed).unwrap();
    let event_facts = [
        (EventKind::ApprovalGranted, 2),
        (EventKind::ApprovalDenied, 3),
        (EventKind::ApprovalExpired, 4),
    ];
    for (kind, id) in event_facts {
        source
            .transact(|tx| {
                tx.append_event(
                    approval_event(
                        kind,
                        EventId::new(format!("evt_{id:026}")).unwrap(),
                        &task_id,
                        &step_id,
                        task_id.as_str(),
                    ),
                    Some(at(3)),
                )
            })
            .unwrap();
    }

    let scheduler = Scheduler::new(
        store,
        TaskEngine::new(Store::open(&path, &Fixed).unwrap(), bus.clone()),
        bus,
    );
    let lease = scheduler
        .claim_event_consumer("approval-wake-test", at(0), at(100))
        .unwrap();
    scheduler.replay_host_events(&lease, at(2), 32).unwrap();
    assert_eq!(
        source.load_task(&task_id).unwrap().task.state,
        TaskState::Received
    );
    let wakes = scheduler.pending_approval_lifecycle_wakes(16).unwrap();
    assert_eq!(wakes.len(), 3);
    assert_eq!(wakes[0].outcome, ApprovalLifecycleOutcome::Granted);
    assert_eq!(wakes[1].outcome, ApprovalLifecycleOutcome::Denied);
    assert_eq!(wakes[2].outcome, ApprovalLifecycleOutcome::Expired);
    assert_eq!(
        scheduler
            .approval_lifecycle_wake(&wakes[0].source_event_id)
            .unwrap(),
        Some(wakes[0].clone())
    );
    assert_eq!(
        source
            .expire_eligible_events(at(4), 16)
            .unwrap()
            .content_records_deleted,
        3
    );
    assert_eq!(
        scheduler
            .pending_approval_lifecycle_wakes(16)
            .unwrap()
            .len(),
        3
    );
    source.verify_integrity().unwrap();

    // A read or restart does not consume the handoff. Only explicit ACK removes it.
    drop(scheduler);
    let restarted = Scheduler::new(
        Store::open(&path, &Fixed).unwrap(),
        TaskEngine::new(
            Store::open(&path, &Fixed).unwrap(),
            EventBus::new(DeterministicUlidSource::starting_at(1_700_400_000_000).unwrap()),
        ),
        EventBus::new(DeterministicUlidSource::starting_at(1_700_500_000_000).unwrap()),
    );
    assert_eq!(
        restarted
            .pending_approval_lifecycle_wakes(16)
            .unwrap()
            .len(),
        3
    );
    assert!(
        restarted
            .acknowledge_approval_lifecycle_wake(&wakes[0].source_event_id)
            .unwrap()
    );
    assert!(
        !restarted
            .acknowledge_approval_lifecycle_wake(&wakes[0].source_event_id)
            .unwrap()
    );
    assert_eq!(
        restarted
            .pending_approval_lifecycle_wakes(16)
            .unwrap()
            .len(),
        2
    );
    tasks.delete_task(task_id.clone()).unwrap();
    assert!(
        restarted
            .pending_approval_lifecycle_wakes(16)
            .unwrap()
            .is_empty()
    );

    drop(restarted);
    drop(source);
    std::fs::remove_file(path).unwrap();
}

#[test]
fn malformed_approval_event_does_not_materialize_or_advance_cursor() {
    let path = db_path();
    let bus = EventBus::new(DeterministicUlidSource::starting_at(1_700_600_000_000).unwrap());
    let store = Store::open(&path, &Fixed).unwrap();
    let task_id = TaskId::new("tsk_00000000000000000000000002").unwrap();
    let step_id = StepId::new("stp_00000000000000000000000002").unwrap();
    let mut tasks = TaskEngine::new(Store::open(&path, &Fixed).unwrap(), bus.clone());
    let actor = ActorId::new("host-test").unwrap();
    let version = SemVer::new("1.0.0").unwrap();
    let context = TransitionContext {
        actor_kind: ActorKind::Host,
        actor_id: &actor,
        actor_version: &version,
        causation_id: None,
    };
    tasks
        .create_task(task_spec(task_id.clone()), &context)
        .unwrap();
    let source = Store::open(&path, &Fixed).unwrap();
    source
        .transact(|tx| {
            tx.append_event(
                approval_event(
                    EventKind::ApprovalGranted,
                    EventId::new("evt_00000000000000000000000003").unwrap(),
                    &task_id,
                    &step_id,
                    "tsk_bad",
                ),
                None,
            )
        })
        .unwrap();
    let scheduler = Scheduler::new(
        store,
        TaskEngine::new(Store::open(&path, &Fixed).unwrap(), bus.clone()),
        bus,
    );
    let lease = scheduler
        .claim_event_consumer("approval-invalid-test", at(0), at(100))
        .unwrap();
    assert!(scheduler.replay_host_events(&lease, at(2), 32).is_err());
    assert!(
        scheduler
            .pending_approval_lifecycle_wakes(16)
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        source
            .transact(|tx| tx.scheduler_cursor())
            .unwrap()
            .last_processed_seq,
        0
    );
    drop(scheduler);
    drop(source);
    std::fs::remove_file(path).unwrap();
}

#[test]
fn failed_cursor_commit_rolls_back_approval_wake_materialization() {
    let path = db_path();
    let bus = EventBus::new(DeterministicUlidSource::starting_at(1_700_700_000_000).unwrap());
    let store = Store::open(&path, &Fixed).unwrap();
    let task_id = TaskId::new("tsk_00000000000000000000000003").unwrap();
    let step_id = StepId::new("stp_00000000000000000000000003").unwrap();
    let mut tasks = TaskEngine::new(Store::open(&path, &Fixed).unwrap(), bus.clone());
    let actor = ActorId::new("host-test").unwrap();
    let version = SemVer::new("1.0.0").unwrap();
    let context = TransitionContext {
        actor_kind: ActorKind::Host,
        actor_id: &actor,
        actor_version: &version,
        causation_id: None,
    };
    tasks
        .create_task(task_spec(task_id.clone()), &context)
        .unwrap();
    let source = Store::open(&path, &Fixed).unwrap();
    source
        .transact(|tx| {
            tx.append_event(
                approval_event(
                    EventKind::ApprovalGranted,
                    EventId::new("evt_00000000000000000000000005").unwrap(),
                    &task_id,
                    &step_id,
                    task_id.as_str(),
                ),
                None,
            )
        })
        .unwrap();
    let scheduler = Scheduler::new(
        store,
        TaskEngine::new(Store::open(&path, &Fixed).unwrap(), bus.clone()),
        bus,
    );
    // Expire the consumer lease after it has been acquired. Wake insertion and
    // cursor advancement share one transaction, so the failed cursor fence
    // must roll back the inserted handoff.
    let lease = scheduler
        .claim_event_consumer("approval-fault", at(0), at(100))
        .unwrap();
    assert!(scheduler.replay_host_events(&lease, at(101), 32).is_err());
    assert!(
        scheduler
            .pending_approval_lifecycle_wakes(16)
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        source
            .transact(|tx| tx.scheduler_cursor())
            .unwrap()
            .last_processed_seq,
        0
    );

    let retry = scheduler
        .claim_event_consumer("approval-retry", at(101), at(201))
        .unwrap();
    scheduler.replay_host_events(&retry, at(102), 32).unwrap();
    let wakes = scheduler.pending_approval_lifecycle_wakes(16).unwrap();
    assert_eq!(wakes.len(), 1);
    assert_eq!(
        wakes[0].source_event_id.as_str(),
        "evt_00000000000000000000000005"
    );

    drop(scheduler);
    drop(source);
    drop(tasks);
    std::fs::remove_file(path).unwrap();
}
