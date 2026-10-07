use serea_event_bus::{EventBus, ReplayItem};
use serea_protocol::*;
use serea_scheduler::{ScheduleDefinition, Scheduler, occurrence_identity_key};
use serea_storage::{
    EventDraft, MissedOccurrencePolicy, ModelAttemptState, ModelCallAttemptDraft,
    ModelCallCompletion, ModelDeploymentClass, ModelPriceSnapshot, ModelResponseStorage,
    ScheduleOwnerKind, ScheduleTriggerKind, Store, UsdMicros,
};
use serea_task_engine::{NewTask, TaskEngine, TransitionContext};
use serea_testkit::DeterministicUlidSource;
use std::path::PathBuf;

struct Fixed;
impl Clock for Fixed {
    fn now_ms(&self) -> Result<EpochMillis, ProtocolError> {
        EpochMillis::new(1_700_000_000_000)
    }
}

fn at(value: i64) -> EpochMillis {
    EpochMillis::new(value).unwrap()
}

fn event_bus() -> EventBus {
    EventBus::new(DeterministicUlidSource::starting_at(1_700_800_000_000).unwrap())
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
        created_at: at(1_700_000_000_000),
        deadline_at: None,
        extensions: Default::default(),
    }
}

fn custom_event(
    id: u32,
    kind: EventKind,
    correlation_id: Option<TaskId>,
    trace: Option<Trace>,
    payload: serde_json::Value,
    retention_at: Option<EpochMillis>,
) -> EventDraft {
    EventDraft {
        event: SereaEvent {
            envelope_version: EnvelopeVersion::new("1").unwrap(),
            surface: WireSurface::new(WireSurface::EVENT).unwrap(),
            message_id: EventId::new(format!("evt_{id:026}")).unwrap(),
            seq: Seq::new(0),
            kind,
            occurred_at: Timestamp::from_epoch_millis(at(1_700_000_000_000)),
            correlation_id,
            causation_id: None,
            actor: Actor {
                kind: ActorKind::Host,
                id: ActorId::new("host-test").unwrap(),
                version: SemVer::new("1.0.0").unwrap(),
                extensions: Default::default(),
            },
            data_class: DataClass::Public,
            trace,
            payload: payload.as_object().unwrap().clone(),
            extensions: Default::default(),
        },
        retention_at,
    }
}

fn add_model_accounting_fixture(store: &Store) {
    let price = ModelPriceSnapshot::new(CostClass::Free, "portable-free-1", 0, 0);
    let make_attempt = |request_id: &str| ModelCallAttemptDraft {
        request_id: RequestId::new(request_id).unwrap(),
        task_id: None,
        purpose: ModelPurpose::Chat,
        model_id: ModelId::new("nemotron-3-nano-30b").unwrap(),
        provider_id: ProviderId::new("ollama").unwrap(),
        deployment_class: ModelDeploymentClass::Local,
        data_class: DataClass::Public,
        relation_kind: serea_storage::ModelAttemptRelationKind::None,
        parent_request_id: None,
        fallback_from_model_id: None,
        price: price.clone(),
        max_context_tokens: 4096,
        effective_max_output_tokens: 1024,
        dispatch_intent_at: at(1_700_000_000_030),
    };
    let cap = UsdMicros::new(0).unwrap();
    let ambiguous_id = RequestId::new("req_00000000000000000000000071").unwrap();
    store
        .reserve_model_call(make_attempt(ambiguous_id.as_str()), cap)
        .unwrap();
    store
        .mark_model_call_ambiguous(&ambiguous_id, "AMBIGUOUS_DISPATCH", at(1_700_000_000_031))
        .unwrap();

    let completed_id = RequestId::new("req_00000000000000000000000072").unwrap();
    store
        .reserve_model_call(make_attempt(completed_id.as_str()), cap)
        .unwrap();
    store
        .complete_model_call(
            &completed_id,
            ModelCallCompletion {
                input_tokens: TokenCount::new(10),
                output_tokens: TokenCount::new(5),
                latency_ms: 7,
                repair_attempts: 0,
                finish_reason: FinishReason::Stop,
                recorded_at: at(1_700_000_000_032),
                accepted_response: ModelResponseStorage {
                    canonical_json: br#"{"result":"portable"}"#.to_vec(),
                    data_class: DataClass::Public,
                },
            },
        )
        .unwrap();
}

fn portable_path() -> Option<PathBuf> {
    std::env::var_os("SEREA_PORTABLE_DB_PATH").map(PathBuf::from)
}

#[test]
fn closed_serea_database_can_be_produced_or_consumed_cross_architecture() {
    let Some(path) = portable_path() else {
        return;
    };
    match std::env::var("SEREA_PORTABLE_MODE").as_deref() {
        Ok("produce") => produce(&path),
        Ok("consume") => consume(&path),
        _ => panic!("SEREA_PORTABLE_MODE must be produce or consume"),
    }
}

fn produce(path: &std::path::Path) {
    let bus = event_bus();
    let store = Store::open(path, &Fixed).unwrap();
    let mut tasks = TaskEngine::new(Store::open(path, &Fixed).unwrap(), bus.clone());
    let actor = ActorId::new("host-test").unwrap();
    let version = SemVer::new("1.0.0").unwrap();
    let context = TransitionContext {
        actor_kind: ActorKind::Host,
        actor_id: &actor,
        actor_version: &version,
        causation_id: None,
    };
    let device_task = TaskId::new("tsk_00000000000000000000000071").unwrap();
    tasks
        .create_task(
            task_spec(device_task.clone(), "Waiting device task"),
            &context,
        )
        .unwrap();
    tasks
        .start_planning(
            device_task.clone(),
            TaskState::Received,
            0,
            at(1_700_000_000_000),
            &context,
        )
        .unwrap();
    let state_revision = store.load_task(&device_task).unwrap().state_revision;
    tasks
        .block_for_device(
            device_task.clone(),
            DeviceId::new("dev_00000000000000000000000071").unwrap(),
            TaskState::Planning,
            state_revision,
            at(1_700_000_000_001),
            &context,
        )
        .unwrap();

    let mut scheduler = Scheduler::new(
        Store::open(path, &Fixed).unwrap(),
        TaskEngine::new(Store::open(path, &Fixed).unwrap(), bus.clone()),
        bus.clone(),
    );
    let schedule_id = ScheduleId::new("sch_00000000000000000000000071").unwrap();
    scheduler
        .create_schedule(
            ScheduleDefinition {
                schedule_id: schedule_id.clone(),
                owner_kind: ScheduleOwnerKind::Host,
                owner_id: "portable-test-host".into(),
                trigger_kind: ScheduleTriggerKind::Calendar,
                recurrence_json: Some(
                    r#"{"version":"1","kind":"ONCE","anchor_local":"2020-01-01T00:00"}"#.into(),
                ),
                event_predicate_json: None,
                template_json:
                    br#"{"version":"1","title":"Portable task","intent":"Check portable state."}"#
                        .to_vec(),
                title_data_class: DataClass::Public,
                intent_data_class: DataClass::Public,
                policy_class_rank: 0,
                approval_policy: serde_json::json!({}),
                timezone: Some("UTC".into()),
                missed_policy: MissedOccurrencePolicy::RunEach,
            },
            &EventId::new("evt_00000000000000000000000071").unwrap(),
            &Digest::new(format!("sha256:{}", "d".repeat(64))).unwrap(),
            at(1_700_000_000_000),
        )
        .unwrap();
    assert_eq!(
        scheduler
            .process_calendar_due(at(1_700_000_000_000), "portable-worker")
            .unwrap(),
        1
    );
    let occurrence = occurrence_identity_key("2020-01-01T00:00", "UTC").unwrap();
    let scheduled_task = store
        .transact(|tx| tx.schedule_occurrence_task(&schedule_id, &occurrence))
        .unwrap()
        .unwrap();

    let approval_task = TaskId::new("tsk_00000000000000000000000072").unwrap();
    tasks
        .create_task(
            task_spec(approval_task.clone(), "Approval routing task"),
            &context,
        )
        .unwrap();
    let step_id = StepId::new("stp_00000000000000000000000071").unwrap();
    store
        .transact(|tx| {
            let draft = custom_event(
                72,
                EventKind::ApprovalGranted,
                Some(approval_task.clone()),
                Some(Trace {
                    task_id: Some(approval_task.clone()),
                    step_id: Some(step_id.clone()),
                    ..Trace::default()
                }),
                serde_json::json!({
                    "approval_id":"apr_00000000000000000000000071",
                    "task_id":approval_task.as_str(),
                    "step_id":step_id.as_str()
                }),
                None,
            );
            tx.append_event(draft.event, draft.retention_at)
        })
        .unwrap();
    // A separate retained event is expired to persist an intentional range.
    store
        .transact(|tx| {
            let draft = custom_event(
                73,
                EventKind::TaskCreated,
                None,
                None,
                serde_json::json!({"fixture":"expired"}),
                Some(at(1_700_000_000_010)),
            );
            tx.append_event(draft.event, draft.retention_at)
        })
        .unwrap();
    assert_eq!(
        store
            .expire_eligible_events(at(1_700_000_000_010), 16)
            .unwrap()
            .content_records_deleted,
        1
    );

    let connected = custom_event(
        74,
        EventKind::DeviceConnected,
        None,
        None,
        serde_json::json!({"device_id":"dev_00000000000000000000000071"}),
        None,
    );
    store
        .transact(|tx| tx.append_event(connected.event, None))
        .unwrap();
    let consumer = scheduler
        .claim_event_consumer(
            "portable-consumer",
            at(1_700_000_000_020),
            at(1_700_000_120_020),
        )
        .unwrap();
    while scheduler
        .replay_host_events(&consumer, at(1_700_000_000_021), 256)
        .unwrap()
        > 0
    {}
    assert_eq!(
        scheduler
            .process_device_session_resume_wakes(at(1_700_000_000_022), 256)
            .unwrap(),
        1
    );

    assert_eq!(
        store.load_task(&device_task).unwrap().task.state,
        TaskState::Ready
    );
    assert_eq!(
        store.load_task(&scheduled_task).unwrap().task.kind,
        TaskKind::Scheduled
    );
    add_model_accounting_fixture(&store);
    assert_eq!(store.schema_version().unwrap(), 3);
    store.verify_integrity().unwrap();
    drop(scheduler);
    drop(tasks);
    drop(store);

    let connection = rusqlite::Connection::open(path).unwrap();
    connection
        .execute_batch("PRAGMA wal_checkpoint(TRUNCATE); PRAGMA journal_mode=DELETE;")
        .unwrap();
    let integrity: String = connection
        .query_row("PRAGMA integrity_check", [], |row| row.get(0))
        .unwrap();
    assert_eq!(integrity, "ok");
    drop(connection);
    assert!(!PathBuf::from(format!("{}-shm", path.display())).exists());
}

fn consume(path: &std::path::Path) {
    let store = Store::open(path, &Fixed).unwrap();
    assert_eq!(store.schema_version().unwrap(), 3);
    store.verify_integrity().unwrap();
    let device_wait = TaskId::new("tsk_00000000000000000000000071").unwrap();
    assert_eq!(
        store.load_task(&device_wait).unwrap().task.state,
        TaskState::Ready
    );
    let schedule = ScheduleId::new("sch_00000000000000000000000071").unwrap();
    let occurrence = occurrence_identity_key("2020-01-01T00:00", "UTC").unwrap();
    let scheduled_id = store
        .transact(|tx| tx.schedule_occurrence_task(&schedule, &occurrence))
        .unwrap()
        .unwrap();
    assert_eq!(
        store.load_task(&scheduled_id).unwrap().task.kind,
        TaskKind::Scheduled
    );
    let provenance = store
        .schedule_task_provenance(&scheduled_id)
        .unwrap()
        .unwrap();
    assert_eq!(
        store
            .transact(|tx| tx.get_blob(&provenance.template))
            .unwrap(),
        br#"{"intent":"Check portable state.","title":"Portable task","version":"1"}"#
    );
    assert_eq!(store.pending_approval_lifecycle_wakes(16).unwrap().len(), 1);
    let ambiguous = RequestId::new("req_00000000000000000000000071").unwrap();
    assert_eq!(
        store
            .get_model_call_attempt(&ambiguous)
            .unwrap()
            .unwrap()
            .state,
        ModelAttemptState::Ambiguous
    );
    let completed = RequestId::new("req_00000000000000000000000072").unwrap();
    assert_eq!(
        store
            .get_model_call_attempt(&completed)
            .unwrap()
            .unwrap()
            .state,
        ModelAttemptState::Completed
    );
    assert_eq!(
        store
            .model_usage_for_request(&completed)
            .unwrap()
            .unwrap()
            .output_tokens,
        TokenCount::new(5)
    );
    assert_eq!(
        store.get_model_call_response(&completed).unwrap().unwrap(),
        br#"{"result":"portable"}"#
    );
    let replay = store.replay_events(None, None, 256).unwrap();
    assert!(
        replay
            .items
            .iter()
            .any(|item| matches!(item, ReplayItem::ExpiredRange { .. }))
    );
    drop(store);
    let connection = rusqlite::Connection::open(path).unwrap();
    let integrity: String = connection
        .query_row("PRAGMA integrity_check", [], |row| row.get(0))
        .unwrap();
    assert_eq!(integrity, "ok");
}
