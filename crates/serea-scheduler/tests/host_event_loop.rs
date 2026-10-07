use serea_event_bus::EventBus;
use serea_protocol::*;
use serea_scheduler::{ScheduleDefinition, Scheduler};
use serea_storage::{
    MissedOccurrencePolicy, ScheduleOccurrenceDraft, ScheduleOwnerKind, ScheduleStateCommand,
    ScheduleTriggerKind, Store,
};
use serea_task_engine::{NewTask, TaskEngine, TransitionContext};
use serea_testkit::DeterministicUlidSource;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

static NEXT_DB: AtomicUsize = AtomicUsize::new(0);

struct Fixed;
impl Clock for Fixed {
    fn now_ms(&self) -> Result<EpochMillis, ProtocolError> {
        EpochMillis::new(0)
    }
}
fn at(value: i64) -> EpochMillis {
    EpochMillis::new(value).unwrap()
}
fn temp_path() -> PathBuf {
    std::env::temp_dir().join(format!(
        "serea-host-event-loop-{}-{}.sqlite",
        std::process::id(),
        NEXT_DB.fetch_add(1, Ordering::Relaxed)
    ))
}
fn event(
    kind: EventKind,
    correlation_id: Option<TaskId>,
    causation_id: Option<EventId>,
) -> SereaEvent {
    SereaEvent {
        envelope_version: EnvelopeVersion::new("1").unwrap(),
        surface: WireSurface::new(WireSurface::EVENT).unwrap(),
        message_id: EventId::new("evt_00000000000000000000000055").unwrap(),
        seq: Seq::new(55),
        kind,
        occurred_at: Timestamp::from_epoch_millis(at(0)),
        correlation_id,
        causation_id,
        actor: Actor {
            kind: ActorKind::Scheduler,
            id: ActorId::new("scheduler").unwrap(),
            version: SemVer::new("1.0.0").unwrap(),
            extensions: Default::default(),
        },
        data_class: DataClass::Public,
        trace: None,
        payload: serde_json::Map::new(),
        extensions: Default::default(),
    }
}

#[test]
fn host_event_predicates_use_exact_kind_and_reject_scheduler_causal_roots_across_schedules() {
    let path = temp_path();
    let bus = EventBus::new(DeterministicUlidSource::starting_at(1_700_001_000_000).unwrap());
    let primary_store = Store::open(&path, &Fixed).unwrap();
    let task_store = Store::open(&path, &Fixed).unwrap();
    let mut scheduler = Scheduler::new(
        primary_store,
        TaskEngine::new(task_store, bus.clone()),
        bus.clone(),
    );
    let schedule = ScheduleId::new("sch_00000000000000000000000001").unwrap();
    let command = EventId::new("evt_00000000000000000000000001").unwrap();
    let request = Digest::new(format!("sha256:{}", "a".repeat(64))).unwrap();
    scheduler
        .create_schedule(
            ScheduleDefinition {
                schedule_id: schedule.clone(),
                owner_kind: ScheduleOwnerKind::Host,
                owner_id: "host-test".into(),
                trigger_kind: ScheduleTriggerKind::HostEvent,
                recurrence_json: None,
                event_predicate_json: Some(r#"{"version":"1","event_kind":"TASK_CREATED"}"#.into()),
                template_json: br#"{"version":"1","title":"Run","intent":"Perform work."}"#
                    .to_vec(),
                title_data_class: DataClass::Public,
                intent_data_class: DataClass::Public,
                policy_class_rank: 0,
                approval_policy: serde_json::json!({}),
                timezone: None,
                missed_policy: MissedOccurrencePolicy::RunEach,
            },
            &command,
            &request,
            at(0),
        )
        .unwrap();

    let operations = Store::open(&path, &Fixed).unwrap();
    let source = EventId::new("evt_00000000000000000000000002").unwrap();
    let lease = operations
        .transact(|tx| {
            tx.enqueue_schedule_occurrence(ScheduleOccurrenceDraft {
                schedule_id: schedule.clone(),
                occurrence_key: source.to_string(),
                schedule_revision: 1,
                trigger_kind: ScheduleTriggerKind::HostEvent,
                source_event_id: Some(source.clone()),
                source_event_data_class: Some(DataClass::Public),
                intended_local_label: None,
                timezone: None,
                recurrence_evaluator: None,
                tzdb_version: None,
                due_at: None,
                not_before: None,
                created_at: at(0),
            })?;
            tx.claim_schedule_occurrence(&schedule, source.as_str(), 1, "worker-a", at(0), at(100))
        })
        .unwrap();
    let task_id = TaskId::new("tsk_00000000000000000000000001").unwrap();
    let actor_id = ActorId::new("scheduler").unwrap();
    let actor_version = SemVer::new("1.0.0").unwrap();
    let context = TransitionContext {
        actor_kind: ActorKind::Scheduler,
        actor_id: &actor_id,
        actor_version: &actor_version,
        causation_id: None,
    };
    scheduler
        .task_engine()
        .create_task(
            NewTask {
                task_id: task_id.clone(),
                kind: TaskKind::UserRequest,
                title: TaskTitle::new("Scheduler root fixture").unwrap(),
                origin: TaskOrigin {
                    kind: TaskOriginKind::new("HOST").unwrap(),
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
            },
            &context,
        )
        .unwrap();
    operations
        .transact(|tx| tx.map_schedule_occurrence(&lease, &task_id, at(1)))
        .unwrap();

    let predicate =
        EventPredicateV1::parse_json(r#"{"version":"1","event_kind":"TASK_CREATED"}"#).unwrap();
    let rooted = event(EventKind::TaskCreated, Some(task_id.clone()), Some(source));
    assert!(!scheduler.matches_host_event(&predicate, &rooted).unwrap());
    // A and B both see the same durable occurrence mapping; neither may create
    // a feedback wake from its scheduled task event.
    assert!(!scheduler.matches_host_event(&predicate, &rooted).unwrap());
    let mut trace_rooted = event(EventKind::TaskCreated, None, None);
    trace_rooted.trace = Some(Trace {
        task_id: Some(task_id),
        ..Trace::default()
    });
    assert!(
        !scheduler
            .matches_host_event(&predicate, &trace_rooted)
            .unwrap()
    );
    let independent = event(EventKind::TaskCreated, None, None);
    assert!(
        scheduler
            .matches_host_event(&predicate, &independent)
            .unwrap()
    );
    let other_kind = event(EventKind::TaskFailed, None, None);
    assert!(
        !scheduler
            .matches_host_event(&predicate, &other_kind)
            .unwrap()
    );

    drop(operations);
    drop(scheduler);
    std::fs::remove_file(path).unwrap();
}

#[test]
fn dedicated_device_and_approval_wakes_cannot_be_created_as_scheduled_tasks() {
    let path = temp_path();
    let bus = EventBus::new(DeterministicUlidSource::starting_at(1_700_001_000_050).unwrap());
    let mut scheduler = Scheduler::new(
        Store::open(&path, &Fixed).unwrap(),
        TaskEngine::new(Store::open(&path, &Fixed).unwrap(), bus.clone()),
        bus,
    );
    for (index, trigger) in [
        ScheduleTriggerKind::DeviceSessionEstablished,
        ScheduleTriggerKind::ApprovalEvent,
    ]
    .into_iter()
    .enumerate()
    {
        let suffix = index + 20;
        let schedule_id = ScheduleId::new(format!("sch_{suffix:026}")).unwrap();
        let command_id = EventId::new(format!("evt_{suffix:026}")).unwrap();
        assert!(
            scheduler
                .create_schedule(
                    ScheduleDefinition {
                        schedule_id,
                        owner_kind: ScheduleOwnerKind::Host,
                        owner_id: "host-test".into(),
                        trigger_kind: trigger,
                        recurrence_json: None,
                        event_predicate_json: None,
                        template_json: br#"{"version":"1","title":"Run","intent":"Perform work."}"#
                            .to_vec(),
                        title_data_class: DataClass::Public,
                        intent_data_class: DataClass::Public,
                        policy_class_rank: 0,
                        approval_policy: serde_json::json!({}),
                        timezone: None,
                        missed_policy: MissedOccurrencePolicy::RunEach,
                    },
                    &command_id,
                    &Digest::new(format!("sha256:{}", "c".repeat(64))).unwrap(),
                    at(0),
                )
                .is_err()
        );
    }
    drop(scheduler);
    std::fs::remove_file(path).unwrap();
}

#[test]
fn committed_host_event_replay_atomically_creates_one_occurrence_and_advances_cursor() {
    let path = temp_path();
    let bus = EventBus::new(DeterministicUlidSource::starting_at(1_700_001_000_100).unwrap());
    let primary_store = Store::open(&path, &Fixed).unwrap();
    let task_store = Store::open(&path, &Fixed).unwrap();
    let source_store = Store::open(&path, &Fixed).unwrap();
    let mut scheduler = Scheduler::new(
        primary_store,
        TaskEngine::new(task_store, bus.clone()),
        bus.clone(),
    );
    let schedule = ScheduleId::new("sch_00000000000000000000000002").unwrap();
    scheduler
        .create_schedule(
            ScheduleDefinition {
                schedule_id: schedule.clone(),
                owner_kind: ScheduleOwnerKind::Host,
                owner_id: "host-test".into(),
                trigger_kind: ScheduleTriggerKind::HostEvent,
                recurrence_json: None,
                event_predicate_json: Some(r#"{"version":"1","event_kind":"TASK_CREATED"}"#.into()),
                template_json: br#"{"version":"1","title":"Run","intent":"Perform work."}"#
                    .to_vec(),
                title_data_class: DataClass::Public,
                intent_data_class: DataClass::Public,
                policy_class_rank: 0,
                approval_policy: serde_json::json!({}),
                timezone: None,
                missed_policy: MissedOccurrencePolicy::RunEach,
            },
            &EventId::new("evt_00000000000000000000000003").unwrap(),
            &Digest::new(format!("sha256:{}", "b".repeat(64))).unwrap(),
            at(0),
        )
        .unwrap();
    let mut source_event = event(EventKind::TaskCreated, None, None);
    source_event.data_class = DataClass::Personal;
    source_store
        .transact(|tx| tx.append_event(source_event, None))
        .unwrap();

    let expired_lease = scheduler
        .claim_event_consumer("host-scheduler", at(0), at(100))
        .unwrap();
    assert!(
        scheduler
            .replay_host_events(&expired_lease, at(101), 32)
            .is_err()
    );
    assert_eq!(
        source_store
            .transact(|tx| tx.scheduler_cursor())
            .unwrap()
            .last_processed_seq,
        0
    );
    assert!(
        source_store
            .transact(|tx| tx.due_schedule_occurrences(&schedule, at(101), 16))
            .unwrap()
            .is_empty()
    );

    let lease = scheduler
        .claim_event_consumer("host-scheduler-retry", at(101), at(201))
        .unwrap();
    assert_eq!(
        scheduler.replay_host_events(&lease, at(102), 32).unwrap(),
        1
    );
    assert_eq!(
        scheduler.replay_host_events(&lease, at(102), 32).unwrap(),
        0
    );
    scheduler
        .update_schedule(
            ScheduleDefinition {
                schedule_id: schedule.clone(),
                owner_kind: ScheduleOwnerKind::Host,
                owner_id: "host-test".into(),
                trigger_kind: ScheduleTriggerKind::HostEvent,
                recurrence_json: None,
                event_predicate_json: Some(r#"{"version":"1","event_kind":"TASK_CREATED"}"#.into()),
                template_json:
                    br#"{"version":"1","title":"Later","intent":"Use the new future context."}"#
                        .to_vec(),
                title_data_class: DataClass::Public,
                intent_data_class: DataClass::Public,
                policy_class_rank: 0,
                approval_policy: serde_json::json!({}),
                timezone: None,
                missed_policy: MissedOccurrencePolicy::RunEach,
            },
            &EventId::new("evt_00000000000000000000000061").unwrap(),
            &Digest::new(format!("sha256:{}", "1".repeat(64))).unwrap(),
            1,
            at(103),
        )
        .unwrap();
    assert_eq!(
        scheduler
            .process_host_event_occurrences(at(104), "worker-a", 32)
            .unwrap(),
        1
    );
    assert_eq!(
        scheduler
            .process_host_event_occurrences(at(104), "worker-a", 32)
            .unwrap(),
        0
    );
    let created_id = source_store
        .transact(|tx| tx.schedule_occurrence_task(&schedule, "evt_00000000000000000000000055"))
        .unwrap()
        .unwrap();
    let created = scheduler.task_engine().load(created_id.clone()).unwrap();
    assert_eq!(created.task.kind, TaskKind::Scheduled);
    assert_eq!(created.task.data_class, DataClass::Personal);
    assert_eq!(created.task.title.as_str(), "Run");
    assert_eq!(created.task.task_id, created_id);
    let provenance = source_store
        .schedule_task_provenance(&created.task.task_id)
        .unwrap()
        .unwrap();
    assert_eq!(
        source_store
            .transact(|tx| tx.get_blob(&provenance.template))
            .unwrap(),
        br#"{"intent":"Perform work.","title":"Run","version":"1"}"#
    );
    // The task-created lifecycle event is a scheduler-rooted event and is
    // replayed for cursor progress without recursively creating an occurrence.
    assert_eq!(
        scheduler.replay_host_events(&lease, at(106), 32).unwrap(),
        0
    );
    let cursor = source_store.transact(|tx| tx.scheduler_cursor()).unwrap();
    assert_eq!(cursor.last_processed_seq, 5);
    assert_eq!(cursor.replay_high_water_seq, None);

    drop(source_store);
    drop(scheduler);
    std::fs::remove_file(path).unwrap();
}

#[test]
fn host_event_schedule_revisions_do_not_match_events_committed_before_the_revision() {
    let path = temp_path();
    let bus = EventBus::new(DeterministicUlidSource::starting_at(1_700_001_000_200).unwrap());
    let store = Store::open(&path, &Fixed).unwrap();
    let task_store = Store::open(&path, &Fixed).unwrap();
    let source_store = Store::open(&path, &Fixed).unwrap();
    let mut scheduler =
        Scheduler::new(store, TaskEngine::new(task_store, bus.clone()), bus.clone());
    let schedule = ScheduleId::new("sch_00000000000000000000000004").unwrap();
    let first = event(EventKind::TaskCreated, None, None);
    source_store
        .transact(|tx| tx.append_event(first, None))
        .unwrap();

    let definition = || ScheduleDefinition {
        schedule_id: schedule.clone(),
        owner_kind: ScheduleOwnerKind::Host,
        owner_id: "host-test".into(),
        trigger_kind: ScheduleTriggerKind::HostEvent,
        recurrence_json: None,
        event_predicate_json: Some(r#"{"version":"1","event_kind":"TASK_CREATED"}"#.into()),
        template_json: br#"{"version":"1","title":"Run","intent":"Perform work."}"#.to_vec(),
        title_data_class: DataClass::Public,
        intent_data_class: DataClass::Public,
        policy_class_rank: 0,
        approval_policy: serde_json::json!({}),
        timezone: None,
        missed_policy: MissedOccurrencePolicy::RunEach,
    };
    scheduler
        .create_schedule(
            definition(),
            &EventId::new("evt_00000000000000000000000057").unwrap(),
            &Digest::new(format!("sha256:{}", "c".repeat(64))).unwrap(),
            at(1),
        )
        .unwrap();
    assert_eq!(
        scheduler
            .schedule(&schedule)
            .unwrap()
            .event_predicate_after_seq,
        Some(1)
    );

    let mut second = event(EventKind::TaskCreated, None, None);
    second.message_id = EventId::new("evt_00000000000000000000000056").unwrap();
    source_store
        .transact(|tx| tx.append_event(second, None))
        .unwrap();
    scheduler
        .update_schedule(
            definition(),
            &EventId::new("evt_00000000000000000000000058").unwrap(),
            &Digest::new(format!("sha256:{}", "d".repeat(64))).unwrap(),
            1,
            at(2),
        )
        .unwrap();
    assert_eq!(
        scheduler
            .schedule(&schedule)
            .unwrap()
            .event_predicate_after_seq,
        Some(3)
    );

    let lease = scheduler
        .claim_event_consumer("revision-worker", at(3), at(100))
        .unwrap();
    scheduler.replay_host_events(&lease, at(4), 32).unwrap();
    assert!(
        source_store
            .transact(|tx| tx.due_schedule_occurrences(&schedule, at(4), 32))
            .unwrap()
            .is_empty()
    );

    drop(source_store);
    drop(scheduler);
    std::fs::remove_file(path).unwrap();
}

#[test]
fn host_event_resume_excludes_events_committed_while_the_schedule_was_paused() {
    let path = temp_path();
    let bus = EventBus::new(DeterministicUlidSource::starting_at(1_700_001_000_300).unwrap());
    let store = Store::open(&path, &Fixed).unwrap();
    let task_store = Store::open(&path, &Fixed).unwrap();
    let source_store = Store::open(&path, &Fixed).unwrap();
    let mut scheduler =
        Scheduler::new(store, TaskEngine::new(task_store, bus.clone()), bus.clone());
    let schedule = ScheduleId::new("sch_00000000000000000000000005").unwrap();
    let definition = ScheduleDefinition {
        schedule_id: schedule.clone(),
        owner_kind: ScheduleOwnerKind::Host,
        owner_id: "host-test".into(),
        trigger_kind: ScheduleTriggerKind::HostEvent,
        recurrence_json: None,
        event_predicate_json: Some(r#"{"version":"1","event_kind":"TASK_CREATED"}"#.into()),
        template_json: br#"{"version":"1","title":"Run","intent":"Perform work."}"#.to_vec(),
        title_data_class: DataClass::Public,
        intent_data_class: DataClass::Public,
        policy_class_rank: 0,
        approval_policy: serde_json::json!({}),
        timezone: None,
        missed_policy: MissedOccurrencePolicy::RunEach,
    };
    scheduler
        .create_schedule(
            definition,
            &EventId::new("evt_00000000000000000000000057").unwrap(),
            &Digest::new(format!("sha256:{}", "e".repeat(64))).unwrap(),
            at(1),
        )
        .unwrap();
    scheduler
        .change_state(
            &schedule,
            &EventId::new("evt_00000000000000000000000059").unwrap(),
            &Digest::new(format!("sha256:{}", "f".repeat(64))).unwrap(),
            1,
            ScheduleStateCommand::Pause,
            at(2),
        )
        .unwrap();
    source_store
        .transact(|tx| tx.append_event(event(EventKind::TaskCreated, None, None), None))
        .unwrap();
    scheduler
        .change_state(
            &schedule,
            &EventId::new("evt_00000000000000000000000060").unwrap(),
            &Digest::new(format!("sha256:{}", "0".repeat(64))).unwrap(),
            2,
            ScheduleStateCommand::Resume,
            at(4),
        )
        .unwrap();
    assert_eq!(
        scheduler
            .schedule(&schedule)
            .unwrap()
            .event_predicate_after_seq,
        Some(3)
    );
    let lease = scheduler
        .claim_event_consumer("resume-worker", at(4), at(100))
        .unwrap();
    scheduler.replay_host_events(&lease, at(5), 32).unwrap();
    assert!(
        source_store
            .transact(|tx| tx.due_schedule_occurrences(&schedule, at(5), 32))
            .unwrap()
            .is_empty()
    );

    drop(source_store);
    drop(scheduler);
    std::fs::remove_file(path).unwrap();
}
