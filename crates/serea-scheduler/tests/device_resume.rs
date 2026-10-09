use serea_event_bus::EventBus;
use serea_protocol::*;
use serea_scheduler::Scheduler;
use serea_storage::Store;
use serea_task_engine::{NewTask, TaskEngine, TransitionContext};
use serea_testkit::DeterministicUlidSource;
mod support;
use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Barrier};
use support::seed_active_registry;

static NEXT_DB: AtomicUsize = AtomicUsize::new(0);

struct Fixed;
impl Clock for Fixed {
    fn now_ms(&self) -> Result<EpochMillis, ProtocolError> {
        EpochMillis::new(1_700_000_000_000)
    }
}
fn at(n: i64) -> EpochMillis {
    EpochMillis::new(n).unwrap()
}
fn path() -> PathBuf {
    std::env::temp_dir().join(format!(
        "serea-device-resume-{}-{}.sqlite",
        std::process::id(),
        NEXT_DB.fetch_add(1, Ordering::Relaxed)
    ))
}
fn task_id(n: u32) -> TaskId {
    TaskId::new(format!("tsk_{n:026}")).unwrap()
}
fn event_id(n: u32) -> EventId {
    EventId::new(format!("evt_{n:026}")).unwrap()
}
fn device_id() -> DeviceId {
    DeviceId::new("dev_00000000000000000000000001").unwrap()
}
fn task_spec(id: TaskId, origin_device: Option<DeviceId>) -> NewTask {
    NewTask {
        task_id: id,
        kind: TaskKind::UserRequest,
        title: TaskTitle::new("Wait for device").unwrap(),
        origin: TaskOrigin {
            kind: TaskOriginKind::new("USER_MESSAGE").unwrap(),
            device_id: origin_device,
            message_id: None,
            extensions: Default::default(),
        },
        data_class: DataClass::Public,
        policy_class: RiskClass::Observe,
        attempt_budget: AttemptBudget {
            max_model_calls: 5,
            max_tool_calls: 5,
            max_attempts_per_step: 2,
            extensions: Default::default(),
        },
        created_at: at(0),
        deadline_at: None,
        extensions: Default::default(),
    }
}
fn identities() -> (ActorId, SemVer) {
    let actor = ActorId::new("host-test").unwrap();
    let version = SemVer::new("1.0.0").unwrap();
    (actor, version)
}
fn connected_event(id: EventId, device: &DeviceId) -> SereaEvent {
    SereaEvent {
        envelope_version: EnvelopeVersion::new("1").unwrap(),
        surface: WireSurface::new(WireSurface::EVENT).unwrap(),
        message_id: id,
        seq: Seq::new(0),
        kind: EventKind::DeviceConnected,
        occurred_at: Timestamp::from_epoch_millis(at(500)),
        correlation_id: None,
        causation_id: None,
        actor: Actor {
            kind: ActorKind::Host,
            id: ActorId::new("host-test").unwrap(),
            version: SemVer::new("1.0.0").unwrap(),
            extensions: Default::default(),
        },
        data_class: DataClass::Public,
        trace: None,
        payload: serde_json::json!({"device_id": device.as_str()})
            .as_object()
            .unwrap()
            .clone(),
        extensions: Default::default(),
    }
}

struct RepeatingFinalUlids {
    values: VecDeque<UlidValue>,
    last: UlidValue,
}
impl UlidSource for RepeatingFinalUlids {
    fn next_ulid(&mut self) -> UlidValue {
        if let Some(next) = self.values.pop_front() {
            self.last = next.clone();
            next
        } else {
            self.last.clone()
        }
    }
}

#[test]
fn explicit_wait_resumes_once_and_origin_alone_does_not() {
    let db = path();
    let device = device_id();
    let bus = EventBus::new(DeterministicUlidSource::starting_at(1_700_100_000_000).unwrap());
    let store = Store::open(&db, &Fixed).unwrap();
    seed_active_registry(&store);
    let mut engine = TaskEngine::new(Store::open(&db, &Fixed).unwrap(), bus.clone());
    let (actor, version) = identities();
    let ctx = TransitionContext {
        actor_kind: ActorKind::Host,
        actor_id: &actor,
        actor_version: &version,
        causation_id: None,
    };
    let id = task_id(1);
    engine
        .create_task(task_spec(id.clone(), Some(device.clone())), &ctx)
        .unwrap();
    let origin_only_id = task_id(2);
    engine
        .create_task(
            task_spec(origin_only_id.clone(), Some(device.clone())),
            &ctx,
        )
        .unwrap();
    let planning = engine
        .start_planning(id.clone(), TaskState::Received, 0, at(1), &ctx)
        .unwrap();
    let state_revision = store.load_task(&id).unwrap().state_revision;
    assert_eq!(planning.task.state, TaskState::Planning);
    let blocked = engine
        .block_for_device(
            id.clone(),
            device.clone(),
            TaskState::Planning,
            state_revision,
            at(2),
            &ctx,
        )
        .unwrap();
    assert_eq!(blocked.task.state, TaskState::Blocked);
    let wait = store
        .transact(|tx| tx.device_resume_wait(&id))
        .unwrap()
        .unwrap();
    assert_eq!(wait.device_id, device);
    assert_eq!(
        wait.blocked_task_revision,
        store.load_task(&id).unwrap().state_revision
    );
    let raw = rusqlite::Connection::open(&db).unwrap();
    raw.execute(
        "UPDATE device_resume_waits SET blocked_task_revision=blocked_task_revision+1 WHERE task_id=?1",
        [id.as_str()],
    )
    .unwrap();
    assert!(store.verify_integrity().is_err());
    raw.execute(
        "UPDATE device_resume_waits SET blocked_task_revision=?2 WHERE task_id=?1",
        rusqlite::params![
            id.as_str(),
            i64::try_from(wait.blocked_task_revision).unwrap()
        ],
    )
    .unwrap();
    raw.execute(
        "DELETE FROM device_resume_waits WHERE task_id=?1",
        [id.as_str()],
    )
    .unwrap();
    assert!(store.verify_integrity().is_err());
    raw.execute(
        "INSERT INTO device_resume_waits(task_id,device_id,blocked_task_revision,
           registration_event_high_water_seq,created_at_ms) VALUES (?1,?2,?3,?4,?5)",
        rusqlite::params![
            id.as_str(),
            wait.device_id.as_str(),
            i64::try_from(wait.blocked_task_revision).unwrap(),
            i64::try_from(wait.registration_event_high_water_seq).unwrap(),
            wait.created_at.get()
        ],
    )
    .unwrap();
    store.verify_integrity().unwrap();
    drop(raw);

    let other_device = DeviceId::new("dev_00000000000000000000000002").unwrap();
    store
        .transact(|tx| tx.append_event(connected_event(event_id(9), &other_device), None))
        .unwrap();
    let mut scheduler = Scheduler::new(Store::open(&db, &Fixed).unwrap(), engine, bus.clone());
    let lease = scheduler
        .claim_event_consumer("scheduler-a", at(3), at(100))
        .unwrap();
    scheduler.replay_host_events(&lease, at(4), 32).unwrap();
    assert_eq!(
        scheduler
            .process_device_session_resume_wakes(at(4), 32)
            .unwrap(),
        0,
        "a different connected device cannot satisfy the wait"
    );
    store.verify_integrity().unwrap();
    assert_eq!(store.load_task(&id).unwrap().task.state, TaskState::Blocked);
    store
        .transact(|tx| tx.append_event(connected_event(event_id(10), &device), None))
        .unwrap();
    scheduler.replay_host_events(&lease, at(4), 32).unwrap();
    store
        .transact(|tx| tx.append_event(connected_event(event_id(11), &device), None))
        .unwrap();
    scheduler.replay_host_events(&lease, at(4), 32).unwrap();
    assert_eq!(
        store
            .transact(|tx| tx.pending_device_session_resume_wakes(32))
            .unwrap()
            .len(),
        1,
        "a later connection cannot rematerialize a wait already consumed by an earlier event"
    );
    drop(scheduler);
    let mut scheduler = Scheduler::new(
        Store::open(&db, &Fixed).unwrap(),
        TaskEngine::new(Store::open(&db, &Fixed).unwrap(), bus.clone()),
        bus.clone(),
    );
    assert_eq!(
        scheduler
            .process_device_session_resume_wakes(at(5), 32)
            .unwrap(),
        1
    );
    assert_eq!(
        scheduler
            .process_device_session_resume_wakes(at(6), 32)
            .unwrap(),
        0
    );
    assert_eq!(store.load_task(&id).unwrap().task.state, TaskState::Ready);
    assert_eq!(
        store.load_task(&origin_only_id).unwrap().task.state,
        TaskState::Received,
        "TaskOrigin.device_id alone must never create resume eligibility"
    );
    assert!(
        store
            .transact(|tx| tx.device_resume_wait(&id))
            .unwrap()
            .is_none()
    );
    assert!(
        store
            .transact(|tx| tx.pending_device_session_resume_wakes(32))
            .unwrap()
            .is_empty()
    );
    let events = store.replay_events(None, None, 32).unwrap();
    assert_eq!(
        events
            .items
            .iter()
            .filter(|item| matches!(item, serea_storage::ReplayItem::Event { event } if event.kind == EventKind::TaskResumed))
            .count(),
        1
    );
    drop((actor, version));
    drop(scheduler);
    drop(store);
    std::fs::remove_file(db).unwrap();
}

#[test]
fn independent_scheduler_workers_race_replay_and_resume_without_duplicate_task_events() {
    let db = path();
    let device = device_id();
    let bus = EventBus::new(DeterministicUlidSource::starting_at(1_700_150_000_000).unwrap());
    let store = Store::open(&db, &Fixed).unwrap();
    seed_active_registry(&store);
    let mut tasks = TaskEngine::new(Store::open(&db, &Fixed).unwrap(), bus.clone());
    let (actor, version) = identities();
    let context = TransitionContext {
        actor_kind: ActorKind::Host,
        actor_id: &actor,
        actor_version: &version,
        causation_id: None,
    };
    let id = task_id(15);
    tasks
        .create_task(task_spec(id.clone(), None), &context)
        .unwrap();
    let planning = tasks
        .start_planning(id.clone(), TaskState::Received, 0, at(1), &context)
        .unwrap();
    let revision = store.load_task(&id).unwrap().state_revision;
    tasks
        .block_for_device(
            id.clone(),
            device.clone(),
            planning.task.state,
            revision,
            at(2),
            &context,
        )
        .unwrap();
    store
        .transact(|tx| tx.append_event(connected_event(event_id(15), &device), None))
        .unwrap();

    let coordinator = Scheduler::new(
        Store::open(&db, &Fixed).unwrap(),
        TaskEngine::new(Store::open(&db, &Fixed).unwrap(), bus.clone()),
        bus.clone(),
    );
    let lease = coordinator
        .claim_event_consumer("concurrent-replay", at(3), at(100))
        .unwrap();
    drop(coordinator);

    let barrier = Arc::new(Barrier::new(3));
    let mut workers = Vec::new();
    for worker_index in 0..2 {
        let barrier = Arc::clone(&barrier);
        let db = db.clone();
        let bus = bus.clone();
        let lease = lease.clone();
        workers.push(std::thread::spawn(move || {
            let scheduler = Scheduler::new(
                Store::open(&db, &Fixed).unwrap(),
                TaskEngine::new(Store::open(&db, &Fixed).unwrap(), bus.clone()),
                bus,
            );
            barrier.wait();
            scheduler.replay_host_events(&lease, at(4), 32).unwrap();
            worker_index
        }));
    }
    barrier.wait();
    for worker in workers {
        worker.join().unwrap();
    }
    assert_eq!(
        store
            .transact(|tx| tx.pending_device_session_resume_wakes(32))
            .unwrap()
            .len(),
        1
    );

    let barrier = Arc::new(Barrier::new(3));
    let mut workers = Vec::new();
    for worker_index in 0..2 {
        let barrier = Arc::clone(&barrier);
        let db = db.clone();
        let bus = bus.clone();
        workers.push(std::thread::spawn(move || {
            let mut scheduler = Scheduler::new(
                Store::open(&db, &Fixed).unwrap(),
                TaskEngine::new(Store::open(&db, &Fixed).unwrap(), bus.clone()),
                bus,
            );
            barrier.wait();
            scheduler
                .process_device_session_resume_wakes(at(5), 32)
                .unwrap();
            worker_index
        }));
    }
    barrier.wait();
    for worker in workers {
        worker.join().unwrap();
    }
    assert_eq!(store.load_task(&id).unwrap().task.state, TaskState::Ready);
    assert_eq!(
        store
            .replay_events(None, None, 128)
            .unwrap()
            .items
            .iter()
            .filter(|item| matches!(item, serea_storage::ReplayItem::Event { event } if event.kind == EventKind::TaskResumed))
            .count(),
        1
    );
    drop((actor, version, tasks, store));
    std::fs::remove_file(db).unwrap();
}

#[test]
fn event_before_wait_is_not_replayed_into_a_late_wait_but_a_later_event_is_eligible() {
    let db = path();
    let device = device_id();
    let bus = EventBus::new(DeterministicUlidSource::starting_at(1_700_200_000_000).unwrap());
    let store = Store::open(&db, &Fixed).unwrap();
    seed_active_registry(&store);
    let mut engine = TaskEngine::new(Store::open(&db, &Fixed).unwrap(), bus.clone());
    let (actor, version) = identities();
    let ctx = TransitionContext {
        actor_kind: ActorKind::Host,
        actor_id: &actor,
        actor_version: &version,
        causation_id: None,
    };
    let id = task_id(2);
    engine
        .create_task(task_spec(id.clone(), None), &ctx)
        .unwrap();
    store
        .transact(|tx| tx.append_event(connected_event(event_id(20), &device), None))
        .unwrap();
    let planning = engine
        .start_planning(id.clone(), TaskState::Received, 0, at(1), &ctx)
        .unwrap();
    let revision = store.load_task(&id).unwrap().state_revision;
    engine
        .block_for_device(
            id.clone(),
            device.clone(),
            planning.task.state,
            revision,
            at(2),
            &ctx,
        )
        .unwrap();
    let wait = store
        .transact(|tx| tx.device_resume_wait(&id))
        .unwrap()
        .unwrap();
    assert!(wait.registration_event_high_water_seq >= 3);

    store
        .transact(|tx| tx.append_event(connected_event(event_id(21), &device), None))
        .unwrap();
    store
        .transact(|tx| tx.append_event(connected_event(event_id(22), &device), None))
        .unwrap();
    let scheduler_engine = TaskEngine::new(Store::open(&db, &Fixed).unwrap(), bus.clone());
    let mut scheduler = Scheduler::new(Store::open(&db, &Fixed).unwrap(), scheduler_engine, bus);
    let lease = scheduler
        .claim_event_consumer("scheduler-a", at(3), at(100))
        .unwrap();
    scheduler.replay_host_events(&lease, at(4), 32).unwrap();
    assert_eq!(
        store
            .transact(|tx| tx.pending_device_session_resume_wakes(32))
            .unwrap()
            .len(),
        1,
        "two connection events cannot enqueue the same blocked task revision twice"
    );
    assert_eq!(
        scheduler
            .process_device_session_resume_wakes(at(5), 32)
            .unwrap(),
        1
    );
    assert_eq!(store.load_task(&id).unwrap().task.state, TaskState::Ready);

    let actor = ActorId::new("host-test").unwrap();
    let version = SemVer::new("1.0.0").unwrap();
    let context = TransitionContext {
        actor_kind: ActorKind::Host,
        actor_id: &actor,
        actor_version: &version,
        causation_id: None,
    };
    scheduler
        .task_engine()
        .start_planning(id.clone(), TaskState::Ready, 0, at(6), &context)
        .unwrap();
    let new_revision = store.load_task(&id).unwrap().state_revision;
    scheduler
        .task_engine()
        .block_for_device(
            id.clone(),
            device.clone(),
            TaskState::Planning,
            new_revision,
            at(7),
            &context,
        )
        .unwrap();
    store
        .transact(|tx| tx.append_event(connected_event(event_id(23), &device), None))
        .unwrap();
    scheduler.replay_host_events(&lease, at(8), 32).unwrap();
    assert_eq!(
        store
            .transact(|tx| tx.pending_device_session_resume_wakes(32))
            .unwrap()
            .len(),
        1,
        "a new task wait revision is eligible for a later connection"
    );
    assert_eq!(
        scheduler
            .process_device_session_resume_wakes(at(9), 32)
            .unwrap(),
        1
    );
    assert_eq!(store.load_task(&id).unwrap().task.state, TaskState::Ready);
    drop(scheduler);
    drop(store);
    std::fs::remove_file(db).unwrap();
}

#[test]
fn device_wait_registration_failure_rolls_back_task_state_journal_and_wait_row() {
    let db = path();
    let timestamp = TimestampMs::new(1_700_050_000_000).unwrap();
    let values = (1_u8..=2)
        .map(|entropy| UlidValue::new(timestamp, [entropy; 10]))
        .collect::<Vec<_>>();
    let source = RepeatingFinalUlids {
        values: VecDeque::from(values.clone()),
        last: values[0].clone(),
    };
    let bus = EventBus::new(source);
    let store = Store::open(&db, &Fixed).unwrap();
    seed_active_registry(&store);
    let mut engine = TaskEngine::new(Store::open(&db, &Fixed).unwrap(), bus);
    let (actor, version) = identities();
    let context = TransitionContext {
        actor_kind: ActorKind::Host,
        actor_id: &actor,
        actor_version: &version,
        causation_id: None,
    };
    let id = task_id(59);
    let device = device_id();
    engine
        .create_task(task_spec(id.clone(), None), &context)
        .unwrap();
    engine
        .start_planning(id.clone(), TaskState::Received, 0, at(1), &context)
        .unwrap();
    let before = store.load_task(&id).unwrap();
    let high_water_before = store
        .replay_events(None, None, 32)
        .unwrap()
        .snapshot_high_water_seq
        .get();

    // The attempted BLOCKED event reuses the prior TASK_STARTED identifier.
    // Its Event Bus participant rejects after the tentative state transition.
    assert!(
        engine
            .block_for_device(
                id.clone(),
                device,
                TaskState::Planning,
                before.state_revision,
                at(2),
                &context,
            )
            .is_err()
    );
    let after = store.load_task(&id).unwrap();
    assert_eq!(after.task.state, TaskState::Planning);
    assert_eq!(after.state_revision, before.state_revision);
    assert!(
        store
            .transact(|tx| tx.device_resume_wait(&id))
            .unwrap()
            .is_none()
    );
    assert_eq!(
        store
            .replay_events(None, None, 32)
            .unwrap()
            .snapshot_high_water_seq
            .get(),
        high_water_before
    );
    store.verify_integrity().unwrap();
    drop((engine, store));
    std::fs::remove_file(db).unwrap();
}

#[test]
fn generic_device_offline_block_is_refused_and_stale_wake_cannot_resurrect_cancelled_task() {
    let db = path();
    let device = device_id();
    let bus = EventBus::new(DeterministicUlidSource::starting_at(1_700_300_000_000).unwrap());
    let store = Store::open(&db, &Fixed).unwrap();
    seed_active_registry(&store);
    let mut engine = TaskEngine::new(Store::open(&db, &Fixed).unwrap(), bus.clone());
    let (actor, version) = identities();
    let ctx = TransitionContext {
        actor_kind: ActorKind::Host,
        actor_id: &actor,
        actor_version: &version,
        causation_id: None,
    };
    let id = task_id(3);
    engine
        .create_task(task_spec(id.clone(), None), &ctx)
        .unwrap();
    let planning = engine
        .start_planning(id.clone(), TaskState::Received, 0, at(1), &ctx)
        .unwrap();
    let offline = BlockedReason::new("DEVICE_OFFLINE").unwrap();
    assert!(
        engine
            .block(id.clone(), TaskState::Planning, offline, at(2), &ctx)
            .is_err()
    );
    engine
        .block_for_device(
            id.clone(),
            device.clone(),
            TaskState::Planning,
            store.load_task(&id).unwrap().state_revision,
            at(2),
            &ctx,
        )
        .unwrap();
    store
        .transact(|tx| tx.append_event(connected_event(event_id(30), &device), None))
        .unwrap();
    let mut scheduler = Scheduler::new(Store::open(&db, &Fixed).unwrap(), engine, bus.clone());
    let lease = scheduler
        .claim_event_consumer("scheduler-a", at(3), at(100))
        .unwrap();
    scheduler.replay_host_events(&lease, at(4), 32).unwrap();
    let wake = store
        .transact(|tx| tx.pending_device_session_resume_wakes(1))
        .unwrap()
        .pop()
        .unwrap();
    let cancel_context = TransitionContext {
        actor_kind: ActorKind::Host,
        actor_id: &actor,
        actor_version: &version,
        causation_id: None,
    };
    scheduler
        .task_engine()
        .cancel(
            id.clone(),
            TaskOriginKind::new("USER").unwrap(),
            at(5),
            &cancel_context,
        )
        .unwrap();
    assert_eq!(scheduler.reconcile_device_resume_waits(32).unwrap(), 0);
    assert!(
        store
            .transact(|tx| tx.device_resume_wait(&id))
            .unwrap()
            .is_none()
    );
    let result = scheduler
        .task_engine()
        .resume_device_session_wake(&wake, at(6), &cancel_context)
        .unwrap();
    assert!(result.is_none());
    assert_eq!(
        store.load_task(&id).unwrap().task.state,
        TaskState::Cancelled
    );
    assert_eq!(
        store
            .replay_events(None, None, 32)
            .unwrap()
            .items
            .iter()
            .filter(|item| matches!(item, serea_storage::ReplayItem::Event { event } if event.kind == EventKind::TaskResumed))
            .count(),
        0
    );
    drop(planning);
    drop(scheduler);
    drop(store);
    std::fs::remove_file(db).unwrap();
}

#[test]
fn failed_resume_event_participant_rolls_back_task_wait_and_wake_consumption() {
    let db = path();
    let timestamp = TimestampMs::new(1_700_100_000_000).unwrap();
    let values = (1_u8..=3)
        .map(|entropy| UlidValue::new(timestamp, [entropy; 10]))
        .collect::<Vec<_>>();
    let source = RepeatingFinalUlids {
        values: VecDeque::from(values.clone()),
        last: values[0].clone(),
    };
    let bus = EventBus::new(source);
    let store = Store::open(&db, &Fixed).unwrap();
    seed_active_registry(&store);
    let mut engine = TaskEngine::new(Store::open(&db, &Fixed).unwrap(), bus.clone());
    let (actor, version) = identities();
    let context = TransitionContext {
        actor_kind: ActorKind::Host,
        actor_id: &actor,
        actor_version: &version,
        causation_id: None,
    };
    let id = task_id(60);
    let device = device_id();
    engine
        .create_task(task_spec(id.clone(), None), &context)
        .unwrap();
    engine
        .start_planning(id.clone(), TaskState::Received, 0, at(1), &context)
        .unwrap();
    let revision = store.load_task(&id).unwrap().state_revision;
    engine
        .block_for_device(
            id.clone(),
            device.clone(),
            TaskState::Planning,
            revision,
            at(2),
            &context,
        )
        .unwrap();

    // The fourth Task lifecycle event reuses the third event ID. SQLite
    // rejects its Event Bus participant after the tentative READY update.
    // That error must roll back the update and keep the internal wake.
    store
        .transact(|tx| tx.append_event(connected_event(event_id(60), &device), None))
        .unwrap();
    let wake_store = Store::open(&db, &Fixed).unwrap();
    let mut scheduler = Scheduler::new(Store::open(&db, &Fixed).unwrap(), engine, bus);
    let lease = scheduler
        .claim_event_consumer("resume-rollback", at(3), at(100))
        .unwrap();
    scheduler.replay_host_events(&lease, at(4), 32).unwrap();
    assert!(
        scheduler
            .process_device_session_resume_wakes(at(5), 32)
            .is_err()
    );
    assert_eq!(store.load_task(&id).unwrap().task.state, TaskState::Blocked);
    assert!(
        store
            .transact(|tx| tx.device_resume_wait(&id))
            .unwrap()
            .is_some(),
        "a failed resume transaction leaves the explicit wait available for wake revalidation"
    );
    assert_eq!(
        wake_store
            .transact(|tx| tx.pending_device_session_resume_wakes(16))
            .unwrap()
            .len(),
        1
    );

    drop(scheduler);
    drop(wake_store);
    drop(store);
    std::fs::remove_file(db).unwrap();
}

#[test]
fn device_connection_fanout_is_bounded_cursor_fenced_and_replay_safe_above_256_waits() {
    let db = path();
    let device = device_id();
    let bus = EventBus::new(DeterministicUlidSource::starting_at(1_700_400_000_000).unwrap());
    let store = Store::open(&db, &Fixed).unwrap();
    seed_active_registry(&store);
    let mut engine = TaskEngine::new(Store::open(&db, &Fixed).unwrap(), bus.clone());
    let (actor, version) = identities();
    let ctx = TransitionContext {
        actor_kind: ActorKind::Host,
        actor_id: &actor,
        actor_version: &version,
        causation_id: None,
    };
    let mut ids = Vec::new();
    for number in 100..357 {
        let id = task_id(number);
        engine
            .create_task(task_spec(id.clone(), None), &ctx)
            .unwrap();
        engine
            .start_planning(id.clone(), TaskState::Received, 0, at(1), &ctx)
            .unwrap();
        let revision = store.load_task(&id).unwrap().state_revision;
        engine
            .block_for_device(
                id.clone(),
                device.clone(),
                TaskState::Planning,
                revision,
                at(2),
                &ctx,
            )
            .unwrap();
        ids.push(id);
    }
    let source_id = event_id(800);
    store
        .transact(|tx| tx.append_event(connected_event(source_id.clone(), &device), None))
        .unwrap();
    let source_seq = store
        .replay_events(None, None, 256)
        .unwrap()
        .snapshot_high_water_seq
        .get();
    let scheduler = Scheduler::new(Store::open(&db, &Fixed).unwrap(), engine, bus.clone());
    let lease = scheduler
        .claim_event_consumer("scheduler-a", at(3), at(100))
        .unwrap();
    let mut rounds = 0;
    while store
        .transact(|tx| tx.scheduler_cursor())
        .unwrap()
        .last_processed_seq
        < source_seq - 1
    {
        scheduler.replay_host_events(&lease, at(4), 256).unwrap();
        rounds += 1;
        assert!(rounds < 10, "event replay did not reach the device event");
    }
    let cursor = store.transact(|tx| tx.scheduler_cursor()).unwrap();
    assert_eq!(cursor.last_processed_seq, source_seq - 1);
    assert_eq!(
        store
            .transact(|tx| tx.pending_device_session_resume_wakes(256))
            .unwrap()
            .len(),
        256
    );
    drop(scheduler);
    let mut scheduler = Scheduler::new(
        Store::open(&db, &Fixed).unwrap(),
        TaskEngine::new(Store::open(&db, &Fixed).unwrap(), bus.clone()),
        bus,
    );
    let lease = scheduler
        .claim_event_consumer("scheduler-b", at(101), at(200))
        .unwrap();
    scheduler.replay_host_events(&lease, at(102), 256).unwrap();
    assert_eq!(
        store
            .transact(|tx| tx.scheduler_cursor())
            .unwrap()
            .last_processed_seq,
        source_seq
    );
    assert_eq!(
        scheduler
            .process_device_session_resume_wakes(at(103), 256)
            .unwrap(),
        256
    );
    assert_eq!(
        scheduler
            .process_device_session_resume_wakes(at(104), 256)
            .unwrap(),
        1
    );
    assert_eq!(
        scheduler
            .process_device_session_resume_wakes(at(105), 256)
            .unwrap(),
        0
    );
    for id in &ids {
        assert_eq!(store.load_task(id).unwrap().task.state, TaskState::Ready);
    }
    drop(scheduler);
    drop(store);
    std::fs::remove_file(db).unwrap();
}
