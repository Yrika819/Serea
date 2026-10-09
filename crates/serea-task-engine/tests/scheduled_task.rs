use serea_event_bus::EventBus;
use serea_protocol::*;
use serea_storage::{
    EventDraft, MissedOccurrencePolicy, ScheduleDraft, ScheduleOccurrenceDraft, ScheduleOwnerKind,
    ScheduleTriggerKind, Store,
};
use serea_task_engine::{NewTask, TransitionContext};
use serea_testkit::DeterministicUlidSource;
use std::path::PathBuf;
use std::sync::{Arc, Barrier};

struct Fixed;
impl Clock for Fixed {
    fn now_ms(&self) -> Result<EpochMillis, ProtocolError> {
        EpochMillis::new(1_700_000_000_000)
    }
}

fn at(value: i64) -> EpochMillis {
    EpochMillis::new(value).unwrap()
}
fn schedule_id() -> ScheduleId {
    ScheduleId::new("sch_00000000000000000000000001").unwrap()
}
fn event_id(n: u32) -> EventId {
    EventId::new(format!("evt_{n:026}")).unwrap()
}
fn path() -> PathBuf {
    std::env::temp_dir().join(format!(
        "serea-scheduled-task-{}.sqlite",
        std::process::id()
    ))
}
fn event(
    kind: EventKind,
    n: u32,
    cause: Option<EventId>,
    payload: serde_json::Value,
) -> EventDraft {
    EventDraft {
        event: SereaEvent {
            envelope_version: EnvelopeVersion::new("1").unwrap(),
            surface: WireSurface::new(WireSurface::EVENT).unwrap(),
            message_id: event_id(n),
            seq: Seq::new(0),
            kind,
            occurred_at: Timestamp::from_epoch_millis(at(1_700_000_000_000)),
            correlation_id: None,
            causation_id: cause,
            actor: Actor {
                kind: ActorKind::Host,
                id: ActorId::new("host-test").unwrap(),
                version: SemVer::new("1.0.0").unwrap(),
                extensions: Default::default(),
            },
            data_class: DataClass::Public,
            trace: None,
            payload: payload.as_object().unwrap().clone(),
            extensions: Default::default(),
        },
        retention_at: None,
    }
}

#[test]
fn scheduled_task_and_occurrence_mapping_commit_once_and_retry_returns_same_task() {
    let path = path();
    let schedule = schedule_id();
    let command = event_id(10);
    let bus = EventBus::new(DeterministicUlidSource::starting_at(1_700_000_100_000).unwrap());
    let store = Store::open(&path, &Fixed).unwrap();
    let request_digest = Digest::new(format!("sha256:{}", "a".repeat(64))).unwrap();
    store.transact(|tx| {
        tx.create_schedule_command(
            &command,
            &request_digest,
            ScheduleDraft {
                schedule_id: schedule.clone(),
                owner_kind: ScheduleOwnerKind::Host,
                owner_id: "host-test".into(),
                trigger_kind: ScheduleTriggerKind::Calendar,
                recurrence: Some(serde_json::json!({
                    "version":"1","kind":"ONCE","anchor_local":"2024-01-01T09:00"
                })),
                event_predicate: None,
                template_json: br#"{"version":"1","title":"Scheduled summary","intent":"Summarize approved updates."}"#.to_vec(),
                template_data_class: DataClass::Public,
                policy_class_rank: RiskClass::Communication.rank(),
                approval_policy: serde_json::json!({}),
                timezone: Some("Etc/UTC".into()),
                recurrence_evaluator: Some("jiff-0.2.38".into()),
                tzdb_version: Some("2026e".into()),
                next_due_at: Some(at(0)),
                next_local_label: Some("2024-01-01T09:00".into()),
                missed_policy: MissedOccurrencePolicy::RunEach,
            },
            at(0),
            event(
                EventKind::ScheduleCreated,
                11,
                Some(command.clone()),
                serde_json::json!({"schedule_id":schedule.as_str()}),
            ),
        )?;
        let occurrence_key = r#"{"intended_local_label":"2024-01-01T09:00","timezone":"Etc/UTC"}"#;
        tx.enqueue_schedule_occurrence(ScheduleOccurrenceDraft {
            schedule_id: schedule.clone(),
            occurrence_key: occurrence_key.into(),
            schedule_revision: 1,
            trigger_kind: ScheduleTriggerKind::Calendar,
            source_event_id: None,
            source_event_data_class: None,
            intended_local_label: Some("2024-01-01T09:00".into()),
            timezone: Some("Etc/UTC".into()),
            recurrence_evaluator: Some("jiff-0.2.38".into()),
            tzdb_version: Some("2026e".into()),
            due_at: Some(at(0)),
            not_before: None,
            created_at: at(0),
        })?;
        Ok(())
    }).unwrap();

    let occurrence_key = r#"{"intended_local_label":"2024-01-01T09:00","timezone":"Etc/UTC"}"#;
    let lease = store
        .transact(|tx| {
            tx.claim_schedule_occurrence(&schedule, occurrence_key, 1, "worker-a", at(0), at(100))
        })
        .unwrap();
    let task_id = TaskId::new("tsk_00000000000000000000000001").unwrap();
    let actor = ActorId::new("scheduler").unwrap();
    let version = SemVer::new("1.0.0").unwrap();
    let context = TransitionContext {
        actor_kind: ActorKind::Scheduler,
        actor_id: &actor,
        actor_version: &version,
        causation_id: None,
    };
    let spec = NewTask {
        task_id: task_id.clone(),
        kind: TaskKind::Scheduled,
        title: TaskTitle::new("Scheduled summary").unwrap(),
        origin: TaskOrigin {
            kind: TaskOriginKind::new("SCHEDULED").unwrap(),
            device_id: None,
            message_id: None,
            extensions: Default::default(),
        },
        data_class: DataClass::Public,
        policy_class: RiskClass::Communication,
        attempt_budget: AttemptBudget {
            max_model_calls: 12,
            max_tool_calls: 24,
            max_attempts_per_step: 3,
            extensions: Default::default(),
        },
        created_at: at(0),
        deadline_at: None,
        extensions: Default::default(),
    };
    let mut engine = support::engine(Store::open(&path, &Fixed).unwrap(), bus.clone());
    let no_generation_event = bus
        .draft_schedule_task_created(
            &schedule,
            occurrence_key,
            &task_id,
            None,
            at(0),
            DataClass::Public,
        )
        .unwrap();
    assert!(matches!(
        engine.create_scheduled_task(spec.clone(), &lease, at(0), no_generation_event, &context,),
        Err(serea_task_engine::EngineError::NoActiveCapabilityGeneration)
    ));
    assert!(store.load_task(&task_id).is_err());
    assert!(
        store
            .transact(|tx| tx.schedule_occurrence_task(&schedule, occurrence_key))
            .unwrap()
            .is_none()
    );
    assert_eq!(store.replay_events(None, None, 10).unwrap().items.len(), 1);
    support::seed_active_registry_generation(&store);
    drop(engine);
    let mut engine = support::engine(Store::open(&path, &Fixed).unwrap(), bus.clone());

    // A pin failure after Task insertion must roll the Task, occurrence
    // mapping, journal, and event back as one transaction.
    let pin_fault = rusqlite::Connection::open(&path).unwrap();
    pin_fault
        .execute_batch(
            "CREATE TRIGGER reject_scheduled_task_pin BEFORE UPDATE OF capability_registry_generation ON tasks
             BEGIN SELECT RAISE(ABORT,'pin fault'); END;",
        )
        .unwrap();
    let pin_failure_event = bus
        .draft_schedule_task_created(
            &schedule,
            occurrence_key,
            &task_id,
            None,
            at(0),
            DataClass::Public,
        )
        .unwrap();
    assert!(
        engine
            .create_scheduled_task(spec.clone(), &lease, at(0), pin_failure_event, &context,)
            .is_err()
    );
    assert!(store.load_task(&task_id).is_err());
    assert!(
        store
            .transact(|tx| tx.schedule_occurrence_task(&schedule, occurrence_key))
            .unwrap()
            .is_none()
    );
    assert_eq!(store.replay_events(None, None, 10).unwrap().items.len(), 1);
    pin_fault
        .execute_batch("DROP TRIGGER reject_scheduled_task_pin")
        .unwrap();
    drop(pin_fault);

    let mut rejected_event = bus
        .draft_schedule_task_created(
            &schedule,
            occurrence_key,
            &task_id,
            None,
            at(0),
            DataClass::Public,
        )
        .unwrap();
    rejected_event.event.kind = EventKind::TaskCreated;
    assert!(
        engine
            .create_scheduled_task(spec.clone(), &lease, at(0), rejected_event, &context,)
            .is_err()
    );
    assert!(
        store
            .transact(|tx| tx.schedule_occurrence_task(&schedule, occurrence_key))
            .unwrap()
            .is_none()
    );
    assert!(store.load_task(&task_id).is_err());
    assert_eq!(store.replay_events(None, None, 10).unwrap().items.len(), 1);

    let raw = rusqlite::Connection::open(&path).unwrap();
    raw.execute_batch(
        "CREATE TRIGGER refuse_schedule_task_created BEFORE INSERT ON event_content
         WHEN NEW.kind='SCHEDULE_TASK_CREATED'
         BEGIN SELECT RAISE(ABORT,'scheduled lifecycle event fault'); END;",
    )
    .unwrap();
    let failed_event = bus
        .draft_schedule_task_created(
            &schedule,
            occurrence_key,
            &task_id,
            None,
            at(0),
            DataClass::Public,
        )
        .unwrap();
    assert!(
        engine
            .create_scheduled_task(spec.clone(), &lease, at(0), failed_event, &context)
            .is_err()
    );
    assert!(store.load_task(&task_id).is_err());
    assert!(
        store
            .transact(|tx| tx.schedule_occurrence_task(&schedule, occurrence_key))
            .unwrap()
            .is_none()
    );
    assert_eq!(store.replay_events(None, None, 10).unwrap().items.len(), 1);
    raw.execute_batch("DROP TRIGGER refuse_schedule_task_created")
        .unwrap();
    drop(raw);

    drop(engine);
    let barrier = Arc::new(Barrier::new(3));
    let mut workers = Vec::new();
    for _ in 0..2 {
        let barrier = Arc::clone(&barrier);
        let path = path.clone();
        let bus = bus.clone();
        let lease = lease.clone();
        let spec = spec.clone();
        let event = bus
            .draft_schedule_task_created(
                &schedule,
                occurrence_key,
                &task_id,
                None,
                at(0),
                DataClass::Public,
            )
            .unwrap();
        workers.push(std::thread::spawn(move || {
            let mut worker = support::post_p5_engine(Store::open(&path, &Fixed).unwrap(), bus);
            let actor = ActorId::new("scheduler").unwrap();
            let version = SemVer::new("1.0.0").unwrap();
            let context = TransitionContext {
                actor_kind: ActorKind::Scheduler,
                actor_id: &actor,
                actor_version: &version,
                causation_id: None,
            };
            barrier.wait();
            worker
                .create_scheduled_task(spec, &lease, at(0), event, &context)
                .unwrap()
        }));
    }
    barrier.wait();
    let first = workers.remove(0).join().unwrap();
    let second = workers.remove(0).join().unwrap();
    assert_eq!(first.task.task_id, task_id);
    assert_eq!(second.task.task_id, first.task.task_id);

    let events = store.replay_events(None, None, 10).unwrap();
    assert_eq!(events.items.len(), 3);
    assert!(
        matches!(&events.items[2], serea_event_bus::ReplayItem::Event { event }
        if event.kind == EventKind::ScheduleTaskCreated)
    );
    let provenance = store.schedule_task_provenance(&task_id).unwrap().unwrap();
    assert_eq!(provenance.schedule_id, schedule);
    assert_eq!(provenance.occurrence_key, occurrence_key);
    assert_eq!(
        store
            .transact(|tx| tx.get_blob(&provenance.template))
            .unwrap(),
        br#"{"intent":"Summarize approved updates.","title":"Scheduled summary","version":"1"}"#
    );

    let retry_event = bus
        .draft_schedule_task_created(
            &schedule,
            occurrence_key,
            &task_id,
            None,
            at(0),
            DataClass::Public,
        )
        .unwrap();
    let mut retry_engine =
        support::post_p5_engine(Store::open(&path, &Fixed).unwrap(), bus.clone());
    let retried = retry_engine
        .create_scheduled_task(spec, &lease, at(0), retry_event, &context)
        .unwrap();
    assert_eq!(retried.task.task_id, task_id);
    let events = store.replay_events(None, None, 10).unwrap();
    assert_eq!(events.items.len(), 3);
    drop(retry_engine);
    drop(store);
    std::fs::remove_file(path).unwrap();
}
mod support;
