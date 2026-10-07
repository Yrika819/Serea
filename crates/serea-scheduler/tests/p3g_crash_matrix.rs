//! Integrated P3G restart/fault coverage across Event Bus, TaskEngine, and
//! Scheduler durable state. The injected stale-fence failure models a lost
//! worker between handler work and cursor commit; dropping/reopening stores
//! then verifies only committed state is recovered.

use rusqlite::Connection;
use serea_event_bus::EventBus;
use serea_protocol::*;
use serea_scheduler::{ScheduleDefinition, Scheduler, occurrence_identity_key};
use serea_storage::{MissedOccurrencePolicy, ScheduleOwnerKind, ScheduleTriggerKind, Store};
use serea_task_engine::{NewTask, TaskEngine, TransitionContext};
use serea_testkit::DeterministicUlidSource;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

static NEXT_DB: AtomicUsize = AtomicUsize::new(0);
const BASE: i64 = 1_700_000_000_000;

struct Fixed;
impl Clock for Fixed {
    fn now_ms(&self) -> Result<EpochMillis, ProtocolError> {
        EpochMillis::new(BASE)
    }
}

fn at(offset: i64) -> EpochMillis {
    EpochMillis::new(BASE + offset).unwrap()
}

fn path() -> PathBuf {
    std::env::temp_dir().join(format!(
        "serea-p3g-integrated-{}-{}.sqlite",
        std::process::id(),
        NEXT_DB.fetch_add(1, Ordering::Relaxed)
    ))
}

fn bus(seed: i64) -> EventBus {
    EventBus::new(DeterministicUlidSource::starting_at(seed.try_into().unwrap()).unwrap())
}

fn task_spec(task_id: TaskId, title: &str) -> NewTask {
    NewTask {
        task_id,
        kind: TaskKind::UserRequest,
        title: TaskTitle::new(title).unwrap(),
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

fn schedule(
    schedule_id: ScheduleId,
    trigger_kind: ScheduleTriggerKind,
    recurrence_json: Option<&str>,
    event_predicate_json: Option<&str>,
) -> ScheduleDefinition {
    ScheduleDefinition {
        schedule_id,
        owner_kind: ScheduleOwnerKind::Host,
        owner_id: "p3g-host".into(),
        trigger_kind,
        recurrence_json: recurrence_json.map(str::to_owned),
        event_predicate_json: event_predicate_json.map(str::to_owned),
        template_json:
            br#"{"version":"1","title":"P3G task","intent":"Retain this planning context."}"#
                .to_vec(),
        title_data_class: DataClass::Public,
        intent_data_class: DataClass::Public,
        policy_class_rank: 0,
        approval_policy: serde_json::json!({}),
        timezone: (trigger_kind == ScheduleTriggerKind::Calendar).then(|| "UTC".to_owned()),
        missed_policy: MissedOccurrencePolicy::RunEach,
    }
}

fn append_event(
    store: &Store,
    id: u32,
    kind: EventKind,
    payload: serde_json::Value,
    correlation_id: Option<TaskId>,
    trace: Option<Trace>,
    retention_at: Option<EpochMillis>,
) {
    let event = SereaEvent {
        envelope_version: EnvelopeVersion::new("1").unwrap(),
        surface: WireSurface::new(WireSurface::EVENT).unwrap(),
        message_id: EventId::new(format!("evt_{id:026}")).unwrap(),
        seq: Seq::new(0),
        kind,
        occurred_at: Timestamp::from_epoch_millis(at(0)),
        correlation_id,
        causation_id: None,
        actor: Actor {
            kind: ActorKind::Host,
            id: ActorId::new("p3g-test").unwrap(),
            version: SemVer::new("1.0.0").unwrap(),
            extensions: Default::default(),
        },
        data_class: DataClass::Public,
        trace,
        payload: payload.as_object().unwrap().clone(),
        extensions: Default::default(),
    };
    store
        .transact(|tx| tx.append_event(event, retention_at))
        .unwrap();
}

fn count_event(store: &Store, kind: EventKind) -> usize {
    store
        .replay_events(None, None, 256)
        .unwrap()
        .items
        .iter()
        .filter(
            |item| matches!(item, serea_storage::ReplayItem::Event { event } if event.kind == kind),
        )
        .count()
}

#[test]
fn restart_integrates_retention_host_replay_device_wake_approval_handoff_and_recovery_twice() {
    let db = path();
    let event_bus = bus(BASE + 10_000);
    let initial_store = Store::open(&db, &Fixed).unwrap();
    let mut initial_tasks = TaskEngine::new(Store::open(&db, &Fixed).unwrap(), event_bus.clone());
    let mut scheduler = Scheduler::new(
        Store::open(&db, &Fixed).unwrap(),
        TaskEngine::new(Store::open(&db, &Fixed).unwrap(), event_bus.clone()),
        event_bus.clone(),
    );
    let actor = ActorId::new("p3g-host").unwrap();
    let actor_version = SemVer::new("1.0.0").unwrap();
    let context = TransitionContext {
        actor_kind: ActorKind::Host,
        actor_id: &actor,
        actor_version: &actor_version,
        causation_id: None,
    };

    // Calendar work commits a normal scheduled task and occurrence mapping.
    let calendar_id = ScheduleId::new("sch_00000000000000000000000081").unwrap();
    scheduler
        .create_schedule(
            schedule(
                calendar_id.clone(),
                ScheduleTriggerKind::Calendar,
                Some(r#"{"version":"1","kind":"ONCE","anchor_local":"2020-01-01T00:00"}"#),
                None,
            ),
            &EventId::new("evt_00000000000000000000000081").unwrap(),
            &Digest::new(format!("sha256:{}", "8".repeat(64))).unwrap(),
            at(0),
        )
        .unwrap();
    assert_eq!(
        scheduler
            .process_calendar_due(at(0), "calendar-worker")
            .unwrap(),
        1
    );
    let calendar_key = occurrence_identity_key("2020-01-01T00:00", "UTC").unwrap();

    // HOST_EVENT is a separate schedule revision whose high-water excludes all
    // earlier lifecycle/task events.
    let host_schedule_id = ScheduleId::new("sch_00000000000000000000000082").unwrap();
    scheduler
        .create_schedule(
            schedule(
                host_schedule_id.clone(),
                ScheduleTriggerKind::HostEvent,
                None,
                Some(r#"{"version":"1","event_kind":"TASK_FAILED"}"#),
            ),
            &EventId::new("evt_00000000000000000000000082").unwrap(),
            &Digest::new(format!("sha256:{}", "9".repeat(64))).unwrap(),
            at(1),
        )
        .unwrap();

    // One task waits explicitly for a device. Another is only a routing target
    // for an approval wake; neither wake path creates a task.
    let device_task_id = TaskId::new("tsk_00000000000000000000000081").unwrap();
    initial_tasks
        .create_task(task_spec(device_task_id.clone(), "Device wait"), &context)
        .unwrap();
    initial_tasks
        .start_planning(
            device_task_id.clone(),
            TaskState::Received,
            0,
            at(2),
            &context,
        )
        .unwrap();
    let state_revision = initial_store
        .load_task(&device_task_id)
        .unwrap()
        .state_revision;
    let device_id = DeviceId::new("dev_00000000000000000000000081").unwrap();
    initial_tasks
        .block_for_device(
            device_task_id.clone(),
            device_id.clone(),
            TaskState::Planning,
            state_revision,
            at(3),
            &context,
        )
        .unwrap();

    let approval_task_id = TaskId::new("tsk_00000000000000000000000082").unwrap();
    initial_tasks
        .create_task(
            task_spec(approval_task_id.clone(), "Approval routing"),
            &context,
        )
        .unwrap();
    let step_id = StepId::new("stp_00000000000000000000000081").unwrap();

    // An intentionally expired nonmatching event lies before live events. The
    // Scheduler must advance its declared range without fabricating a HOST,
    // device, or approval decision.
    append_event(
        &initial_store,
        83,
        EventKind::TaskFailed,
        serde_json::json!({"fixture":"expired-before-consumption"}),
        None,
        None,
        Some(at(5)),
    );
    assert_eq!(
        initial_store
            .expire_eligible_events(at(6), 16)
            .unwrap()
            .content_records_deleted,
        1
    );
    // This later TaskFailed event is the one HOST_EVENT may match.
    append_event(
        &initial_store,
        84,
        EventKind::TaskFailed,
        serde_json::json!({"fixture":"eligible-host-event"}),
        None,
        None,
        None,
    );
    append_event(
        &initial_store,
        85,
        EventKind::DeviceConnected,
        serde_json::json!({"device_id":device_id.as_str()}),
        None,
        None,
        Some(at(1000)),
    );
    append_event(
        &initial_store,
        86,
        EventKind::ApprovalGranted,
        serde_json::json!({
            "approval_id":"apr_00000000000000000000000081",
            "task_id":approval_task_id.as_str(),
            "step_id":step_id.as_str()
        }),
        Some(approval_task_id.clone()),
        Some(Trace {
            task_id: Some(approval_task_id.clone()),
            step_id: Some(step_id),
            ..Trace::default()
        }),
        Some(at(1000)),
    );

    // A stale consumer lease forces the transaction to roll back after it has
    // tentatively processed expired-range, HOST_EVENT, device, and approval
    // rows. None of those consequences may survive without cursor advancement.
    let stale_lease = scheduler
        .claim_event_consumer("p3g-stale", at(10), at(20))
        .unwrap();
    assert!(
        scheduler
            .replay_host_events(&stale_lease, at(21), 256)
            .is_err()
    );
    assert!(
        scheduler
            .pending_approval_lifecycle_wakes(16)
            .unwrap()
            .is_empty()
    );
    assert!(
        initial_store
            .transact(|tx| tx.pending_device_session_resume_wakes(16))
            .unwrap()
            .is_empty()
    );
    assert!(
        initial_store
            .transact(|tx| tx.due_schedule_occurrences(&host_schedule_id, at(21), 16))
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        initial_store
            .transact(|tx| tx.scheduler_cursor())
            .unwrap()
            .last_processed_seq,
        0
    );

    let consumer = scheduler
        .claim_event_consumer("p3g-replay", at(22), at(120_022))
        .unwrap();
    scheduler
        .replay_host_events(&consumer, at(23), 256)
        .unwrap();
    assert_eq!(
        initial_store
            .transact(|tx| tx.scheduler_cursor())
            .unwrap()
            .last_processed_seq,
        initial_store
            .replay_events(None, None, 256)
            .unwrap()
            .snapshot_high_water_seq
            .get()
    );
    assert_eq!(
        scheduler
            .pending_approval_lifecycle_wakes(16)
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        initial_store
            .transact(|tx| tx.pending_device_session_resume_wakes(16))
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        scheduler
            .process_host_event_occurrences(at(24), "host-worker", 256)
            .unwrap(),
        1
    );
    let host_key = EventId::new("evt_00000000000000000000000084").unwrap();
    assert!(
        initial_store
            .transact(|tx| tx.schedule_occurrence_task(&host_schedule_id, host_key.as_str()))
            .unwrap()
            .is_some()
    );

    // Source content expires after both internal wakes and the host occurrence
    // have become durable. Privacy retention does not wait on Scheduler.
    assert_eq!(
        initial_store
            .expire_eligible_events(at(1001), 16)
            .unwrap()
            .content_records_deleted,
        2
    );
    assert_eq!(
        scheduler
            .pending_approval_lifecycle_wakes(16)
            .unwrap()
            .len(),
        1
    );
    initial_store.verify_integrity().unwrap();

    // Simulate process loss after cursor commit but before device wake work.
    drop((scheduler, initial_tasks, initial_store));

    let reopened_store = Store::open(&db, &Fixed).unwrap();
    let reopened_tasks = TaskEngine::new(Store::open(&db, &Fixed).unwrap(), bus(BASE + 20_000));
    let mut recovered = Scheduler::new(
        Store::open(&db, &Fixed).unwrap(),
        reopened_tasks,
        bus(BASE + 30_000),
    );
    let first = recovered.recover(at(200_000), "p3g-recovery", 256).unwrap();
    assert_eq!(first.device_wakes_processed, 1);
    assert_eq!(
        reopened_store
            .load_task(&device_task_id)
            .unwrap()
            .task
            .state,
        TaskState::Ready
    );
    assert_eq!(
        recovered
            .pending_approval_lifecycle_wakes(16)
            .unwrap()
            .len(),
        1
    );
    assert!(
        reopened_store
            .transact(|tx| tx.schedule_occurrence_task(&calendar_id, &calendar_key))
            .unwrap()
            .is_some()
    );
    assert!(
        reopened_store
            .transact(|tx| tx.schedule_occurrence_task(&host_schedule_id, host_key.as_str()))
            .unwrap()
            .is_some()
    );

    // A second complete startup is a semantic no-op: the same mappings and
    // handoff remain, while the device transition/event is not repeated.
    recovered
        .recover(at(400_000), "p3g-recovery-again", 256)
        .unwrap();
    assert_eq!(
        recovered
            .pending_approval_lifecycle_wakes(16)
            .unwrap()
            .len(),
        1
    );
    assert_eq!(count_event(&reopened_store, EventKind::TaskResumed), 1);
    assert_eq!(
        reopened_store
            .load_task(&approval_task_id)
            .unwrap()
            .task
            .state,
        TaskState::Received
    );
    reopened_store.verify_integrity().unwrap();

    drop((recovered, reopened_store));
    std::fs::remove_file(db).unwrap();
}

#[test]
fn two_workers_replay_and_dispatch_one_host_event_occurrence_once() {
    let db = path();
    let event_bus = bus(BASE + 40_000);
    let mut setup = Scheduler::new(
        Store::open(&db, &Fixed).unwrap(),
        TaskEngine::new(Store::open(&db, &Fixed).unwrap(), event_bus.clone()),
        event_bus.clone(),
    );
    let schedule_id = ScheduleId::new("sch_00000000000000000000000083").unwrap();
    setup
        .create_schedule(
            schedule(
                schedule_id.clone(),
                ScheduleTriggerKind::HostEvent,
                None,
                Some(r#"{"version":"1","event_kind":"TASK_FAILED"}"#),
            ),
            &EventId::new("evt_00000000000000000000000087").unwrap(),
            &Digest::new(format!("sha256:{}", "a".repeat(64))).unwrap(),
            at(0),
        )
        .unwrap();
    let source = EventId::new("evt_00000000000000000000000088").unwrap();
    append_event(
        &Store::open(&db, &Fixed).unwrap(),
        88,
        EventKind::TaskFailed,
        serde_json::json!({"fixture":"one committed source"}),
        None,
        None,
        None,
    );
    let lease = setup
        .claim_event_consumer("p3g-workers", at(1), at(1000))
        .unwrap();
    drop(setup);

    let barrier = std::sync::Arc::new(std::sync::Barrier::new(3));
    let mut workers = Vec::new();
    for worker in 0..2 {
        let db = db.clone();
        let bus = event_bus.clone();
        let lease = lease.clone();
        let barrier = barrier.clone();
        workers.push(std::thread::spawn(move || {
            let mut scheduler = Scheduler::new(
                Store::open(&db, &Fixed).unwrap(),
                TaskEngine::new(Store::open(&db, &Fixed).unwrap(), bus.clone()),
                bus,
            );
            barrier.wait();
            scheduler.replay_host_events(&lease, at(2), 32).unwrap();
            scheduler
                .process_host_event_occurrences(at(3), &format!("worker-{worker}"), 32)
                .unwrap();
        }));
    }
    barrier.wait();
    for worker in workers {
        worker.join().unwrap();
    }

    let store = Store::open(&db, &Fixed).unwrap();
    let mapped = store
        .transact(|tx| tx.schedule_occurrence_task(&schedule_id, source.as_str()))
        .unwrap()
        .expect("the one source occurrence has a durable task mapping");
    assert_eq!(
        store.load_task(&mapped).unwrap().task.kind,
        TaskKind::Scheduled
    );
    assert_eq!(count_event(&store, EventKind::TaskCreated), 1);
    assert_eq!(count_event(&store, EventKind::ScheduleTaskCreated), 1);
    assert_eq!(
        store
            .transact(|tx| tx.due_schedule_occurrences(&schedule_id, at(10), 32))
            .unwrap()
            .len(),
        0
    );
    store.verify_integrity().unwrap();

    drop(store);
    std::fs::remove_file(db).unwrap();
}

#[test]
fn calendar_occurrence_insert_failure_rolls_back_occurrence_and_next_due() {
    let db = path();
    let event_bus = bus(BASE + 50_000);
    let store = Store::open(&db, &Fixed).unwrap();
    let mut scheduler = Scheduler::new(
        Store::open(&db, &Fixed).unwrap(),
        TaskEngine::new(Store::open(&db, &Fixed).unwrap(), event_bus.clone()),
        event_bus,
    );
    let schedule_id = ScheduleId::new("sch_00000000000000000000000084").unwrap();
    scheduler
        .create_schedule(
            schedule(
                schedule_id.clone(),
                ScheduleTriggerKind::Calendar,
                Some(r#"{"version":"1","kind":"DAILY","anchor_local":"2020-01-01T00:00","interval":1}"#),
                None,
            ),
            &EventId::new("evt_00000000000000000000000089").unwrap(),
            &Digest::new(format!("sha256:{}", "b".repeat(64))).unwrap(),
            at(0),
        )
        .unwrap();
    let before = scheduler.schedule(&schedule_id).unwrap();
    assert_eq!(before.next_local_label.as_deref(), Some("2020-01-01T00:00"));

    // Force the occurrence insert to fail after the recurrence was resolved.
    // The occurrence row and schedule cursor must share one transaction.
    let fault = Connection::open(&db).unwrap();
    fault
        .execute_batch(
            "CREATE TRIGGER p3g_fail_calendar_occurrence
             BEFORE INSERT ON schedule_occurrences
             WHEN NEW.trigger_kind = 'CALENDAR'
             BEGIN SELECT RAISE(ABORT, 'injected calendar enqueue fault'); END;",
        )
        .unwrap();
    assert!(
        scheduler
            .enqueue_due_calendar_occurrences(at(1_700_000_000_000))
            .is_err()
    );
    let after_failure = scheduler.schedule(&schedule_id).unwrap();
    assert_eq!(after_failure.next_local_label, before.next_local_label);
    assert_eq!(after_failure.next_due_at, before.next_due_at);
    assert!(
        store
            .transact(|tx| tx.due_schedule_occurrences(&schedule_id, at(1_700_000_000_000), 16))
            .unwrap()
            .is_empty()
    );

    fault
        .execute_batch("DROP TRIGGER p3g_fail_calendar_occurrence")
        .unwrap();
    assert_eq!(
        scheduler
            .enqueue_due_calendar_occurrences(at(1_700_000_000_000))
            .unwrap(),
        1
    );
    let occurrence_key = occurrence_identity_key("2020-01-01T00:00", "UTC").unwrap();
    assert_eq!(
        store
            .transact(|tx| tx.due_schedule_occurrences(&schedule_id, at(1_700_000_000_000), 16))
            .unwrap()
            .iter()
            .filter(|occurrence| occurrence.occurrence_key == occurrence_key)
            .count(),
        1
    );
    assert_ne!(
        scheduler.schedule(&schedule_id).unwrap().next_due_at,
        before.next_due_at
    );
    store.verify_integrity().unwrap();

    drop((fault, scheduler, store));
    std::fs::remove_file(db).unwrap();
}
