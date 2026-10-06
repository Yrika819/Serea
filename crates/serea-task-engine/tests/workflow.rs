//! Expanded P2F-b contracts, written before wrapper/lifecycle integration.
use serea_protocol::*;
use serea_protocol::{UlidSource, UlidValue};
use serea_storage::{AtRestProtection, AtRestProtectionError, Store, StoreError};
use serea_task_engine::*;
use std::sync::Arc;
mod support;
use support::event_bus;

fn at(n: i64) -> EpochMillis {
    EpochMillis::new(n).unwrap()
}
struct Fixed;
impl Clock for Fixed {
    fn now_ms(&self) -> Result<EpochMillis, ProtocolError> {
        Ok(at(0))
    }
}
fn tid(n: u32) -> TaskId {
    TaskId::new(format!("tsk_{n:026}")).unwrap()
}
fn sid(n: u32) -> StepId {
    StepId::new(format!("stp_{n:026}")).unwrap()
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
            cause: EventId::new("evt_00000000000000000000000001").unwrap(),
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
fn spec(n: u32, class: DataClass) -> NewTask {
    NewTask {
        task_id: tid(n),
        kind: TaskKind::UserRequest,
        title: TaskTitle::new("sensitive title sentinel").unwrap(),
        origin: TaskOrigin {
            kind: TaskOriginKind::new("USER_MESSAGE").unwrap(),
            device_id: None,
            message_id: None,
            extensions: [(
                "future_origin".into(),
                serde_json::json!({"nested":[1,null]}),
            )]
            .into(),
        },
        data_class: class,
        policy_class: RiskClass::Communication,
        attempt_budget: AttemptBudget {
            max_model_calls: 12,
            max_tool_calls: 24,
            max_attempts_per_step: 3,
            extensions: [("future_budget".into(), serde_json::json!(true))].into(),
        },
        created_at: at(10),
        deadline_at: Some(at(1000)),
        extensions: [("future_task".into(), serde_json::json!({"x":[true]}))].into(),
    }
}
fn input(task: u32, step: u32, seq: u32, kind: StepKind) -> PlanStep {
    let raw = format!("{{\"value\":{step}}}").into_bytes();
    let shaped = matches!(
        kind,
        StepKind::Capability | StepKind::Delegate | StepKind::Verify
    );
    let capability = CapabilityId::new("calendar.events.create").unwrap();
    let version = SemVer::new("1.0.0").unwrap();
    let step = TaskStep::new(TaskStepDraft {
        task_id: tid(task),
        step_id: sid(step),
        sequence: seq,
        kind,
        status: StepStatus::new("PLANNED").unwrap(),
        attempt: 0,
        idempotency_key: shaped.then(|| {
            derive_idempotency_key(
                &tid(task),
                &sid(step),
                &capability,
                &version,
                std::str::from_utf8(&raw).unwrap(),
            )
            .unwrap()
        }),
        provider_id: shaped.then(|| ProviderId::new("calendar").unwrap()),
        capability_id: shaped.then_some(capability),
        capability_version: shaped.then_some(version),
        input_digest: digest_of(std::str::from_utf8(&raw).unwrap()).unwrap(),
        result_digest: None,
        side_effect_receipt: None,
        started_at: None,
        completed_at: None,
        lease_owner: None,
        lease_expires_at: None,
        lease_generation: None,
        error: None,
        extensions: [(
            "future_step".into(),
            serde_json::json!({"original":[7,true,null]}),
        )]
        .into(),
    })
    .unwrap();
    PlanStep {
        step,
        input_json: raw,
    }
}
fn engine() -> TaskEngine {
    TaskEngine::new(Store::open_in_memory(&Fixed).unwrap(), event_bus())
}

#[derive(Clone)]
struct RepeatedUlid(UlidValue);
impl UlidSource for RepeatedUlid {
    fn next_ulid(&mut self) -> UlidValue {
        self.0.clone()
    }
}

#[test]
fn p3c_task_creation_commits_task_journal_event_and_sequence_together() {
    let path = std::env::temp_dir().join(format!("serea-p3c-{}.sqlite", std::process::id()));
    let context = Context::new();
    {
        let mut engine = TaskEngine::new(Store::open(&path, &Fixed).unwrap(), event_bus());
        engine
            .create_task(spec(91, DataClass::Personal), &context.view())
            .unwrap();
        // Model a caller that loses the successful response after COMMIT: a
        // retry is rejected as the existing task and cannot append again.
        assert_eq!(
            engine
                .create_task(spec(91, DataClass::Personal), &context.view())
                .err()
                .unwrap(),
            EngineError::TaskExists
        );
    }
    let conn = rusqlite::Connection::open(&path).unwrap();
    let facts: (i64, i64, i64, i64) = conn.query_row(
        "SELECT (SELECT count(*) FROM tasks WHERE task_id=?1),
                (SELECT count(*) FROM task_journal WHERE task_id=?1 AND journal_kind='TASK_INSERTED'),
                (SELECT count(*) FROM event_content WHERE kind='TASK_CREATED'),
                (SELECT last_allocated_seq FROM event_store_state WHERE singleton=1)",
        [tid(91).as_str()],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
    ).unwrap();
    assert_eq!(facts, (1, 1, 1, 1));
    let event_json: String = conn
        .query_row(
            "SELECT event_json FROM event_content WHERE seq=1",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert!(!event_json.contains("sensitive title sentinel"));
    let event: SereaEvent = serde_json::from_str(&event_json).unwrap();
    assert_eq!(event.kind, EventKind::TaskCreated);
    assert_eq!(event.data_class, DataClass::Personal);
    assert_eq!(event.correlation_id.as_ref(), Some(&tid(91)));
    assert_eq!(
        event.trace.as_ref().unwrap().task_id.as_ref(),
        Some(&tid(91))
    );
    drop(conn);
    std::fs::remove_file(&path).unwrap();
}

#[test]
fn task_journal_insert_failure_rolls_back_task_event_and_sequence() {
    let path = std::env::temp_dir().join(format!(
        "serea-p3c-journal-failure-{}.sqlite",
        std::process::id()
    ));
    let context = Context::new();
    let store = Store::open(&path, &Fixed).unwrap();
    rusqlite::Connection::open(&path)
        .unwrap()
        .execute_batch(
            "CREATE TRIGGER reject_task_journal BEFORE INSERT ON task_journal
             BEGIN SELECT RAISE(ABORT, 'injected journal failure'); END;",
        )
        .unwrap();
    let mut engine = TaskEngine::new(store, event_bus());
    assert_eq!(
        engine
            .create_task(spec(95, DataClass::Personal), &context.view())
            .err()
            .unwrap(),
        EngineError::Store(StoreError::ConstraintViolation)
    );
    drop(engine);
    let conn = rusqlite::Connection::open(&path).unwrap();
    let rows: (i64, i64, i64, i64) = conn
        .query_row(
            "SELECT (SELECT count(*) FROM tasks),
                    (SELECT count(*) FROM task_journal),
                    (SELECT count(*) FROM event_content),
                    (SELECT last_allocated_seq FROM event_store_state WHERE singleton=1)",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .unwrap();
    assert_eq!(rows, (0, 0, 0, 0));
    drop(conn);
    std::fs::remove_file(path).unwrap();
}

#[test]
fn task_transition_events_are_specific_and_step_lease_only_writes_emit_none() {
    let path = std::env::temp_dir().join(format!("serea-p3c-kinds-{}.sqlite", std::process::id()));
    let context = Context::new();
    {
        let mut engine = TaskEngine::new(Store::open(&path, &Fixed).unwrap(), event_bus());
        engine
            .create_task(spec(93, DataClass::Personal), &context.view())
            .unwrap();
        engine
            .start_planning(tid(93), TaskState::Received, 0, at(20), &context.view())
            .unwrap();
        engine
            .persist_plan(
                tid(93),
                Plan {
                    revision: 1,
                    steps: vec![input(93, 1, 10, StepKind::Notify)],
                },
                at(30),
                &context.view(),
            )
            .unwrap();
        let guard = engine
            .acquire(
                tid(93),
                sid(1),
                LeaseOwner::new("worker-a").unwrap(),
                None,
                at(40),
                at(50),
                &context.view(),
            )
            .unwrap();
        engine.release(guard, at(41), &context.view()).unwrap();
        assert_eq!(
            engine
                .start_planning(tid(93), TaskState::Planning, 1, at(42), &context.view())
                .err()
                .unwrap(),
            EngineError::IllegalTaskTransition
        );
    }
    let conn = rusqlite::Connection::open(&path).unwrap();
    let kinds: Vec<String> = conn
        .prepare("SELECT kind FROM event_content ORDER BY seq")
        .unwrap()
        .query_map([], |row| row.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(
        kinds,
        ["TASK_CREATED", "TASK_STARTED", "TASK_STATE_CHANGED"]
    );
    drop(conn);
    std::fs::remove_file(path).unwrap();
}

#[test]
fn duplicate_event_insert_rolls_back_transition_and_retry_does_not_duplicate() {
    let path = std::env::temp_dir().join(format!(
        "serea-p3c-insert-failure-{}.sqlite",
        std::process::id()
    ));
    let context = Context::new();
    let source = serea_testkit::DeterministicUlidSource::new().next_ulid();
    let mut engine = TaskEngine::new(
        Store::open(&path, &Fixed).unwrap(),
        serea_event_bus::EventBus::new(RepeatedUlid(source)),
    );
    engine
        .create_task(spec(94, DataClass::Personal), &context.view())
        .unwrap();
    assert_eq!(
        engine
            .start_planning(tid(94), TaskState::Received, 0, at(20), &context.view())
            .err()
            .unwrap(),
        EngineError::Store(StoreError::ConstraintViolation)
    );
    assert!(
        engine
            .create_task(spec(94, DataClass::Personal), &context.view())
            .is_err()
    );
    drop(engine);
    let conn = rusqlite::Connection::open(&path).unwrap();
    let state: (String, i64, i64, i64) = conn
        .query_row(
            "SELECT (SELECT state FROM tasks WHERE task_id=?1),
                    (SELECT count(*) FROM task_journal WHERE task_id=?1),
                    (SELECT count(*) FROM event_content WHERE kind='TASK_CREATED'),
                    (SELECT last_allocated_seq FROM event_store_state WHERE singleton=1)",
            [tid(94).as_str()],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .unwrap();
    assert_eq!(state, ("RECEIVED".into(), 1, 1, 1));
    drop(conn);
    std::fs::remove_file(path).unwrap();
}
fn prepare(e: &mut TaskEngine, c: &Context) {
    e.create_task(spec(1, DataClass::Personal), &c.view())
        .unwrap();
    e.start_planning(tid(1), TaskState::Received, 0, at(20), &c.view())
        .unwrap();
}
fn ready(e: &mut TaskEngine, c: &Context, kind: StepKind) {
    prepare(e, c);
    e.persist_plan(
        tid(1),
        Plan {
            revision: 1,
            steps: vec![input(1, 1, 10, kind)],
        },
        at(30),
        &c.view(),
    )
    .unwrap();
}
fn acquire(
    e: &mut TaskEngine,
    c: &Context,
    expected: Option<u32>,
    now: i64,
    expiry: i64,
) -> Result<LeaseGuard, EngineError> {
    e.acquire(
        tid(1),
        sid(1),
        LeaseOwner::new("worker-a").unwrap(),
        expected,
        at(now),
        at(expiry),
        &c.view(),
    )
}
fn result<'a>() -> StepOutcome<'a> {
    StepOutcome::Succeeded {
        result_json: b"{\"result\":true}",
        receipt: None,
    }
}

#[test]
fn task_projection_refuses_more_steps_than_the_task_schema_allows() {
    let c = Context::new();
    let mut e = engine();
    prepare(&mut e, &c);
    let steps = (1..=1025)
        .map(|n| input(1, n, n, StepKind::Notify))
        .collect();
    assert!(matches!(
        e.persist_plan(tid(1), Plan { revision: 1, steps }, at(30), &c.view(),),
        Err(EngineError::InvalidPlan)
    ));
    let unchanged = e.load(tid(1)).unwrap();
    assert_eq!(unchanged.task.state, TaskState::Planning);
    assert_eq!(unchanged.plan_revision, 0);
    let boundary_steps = (1..=1024)
        .map(|n| input(1, n, n, StepKind::Notify))
        .collect();
    e.persist_plan(
        tid(1),
        Plan {
            revision: 1,
            steps: boundary_steps,
        },
        at(30),
        &c.view(),
    )
    .unwrap();
    let boundary_task = e.load(tid(1)).unwrap().task;
    assert_eq!(boundary_task.steps.len(), 1024);
    let wire = serde_json::to_value(&boundary_task).unwrap();
    serea_protocol::schema::validate(serea_protocol::schema::SchemaName::AssistantTask, &wire)
        .unwrap();
}

#[test]
fn duplicate_creation_is_typed_and_journal_has_one_insert() {
    let c = Context::new();
    let mut e = engine();
    e.create_task(spec(1, DataClass::Public), &c.view())
        .unwrap();
    assert_eq!(
        e.create_task(spec(1, DataClass::Public), &c.view()).err(),
        Some(EngineError::TaskExists)
    );
    let out = e.delete_task(tid(1)).unwrap();
    assert_eq!(out.task_rows, 1);
    assert_eq!(out.journal_rows, 1);
}
#[test]
fn acquire_begin_outcome_use_durable_authority_without_second_charge() {
    let c = Context::new();
    let mut e = engine();
    ready(&mut e, &c, StepKind::Notify);
    let g = acquire(&mut e, &c, None, 40, 50).unwrap();
    assert_eq!(e.load(tid(1)).unwrap().steps[0].step.attempt, 1);
    e.begin_attempt(&g, at(41), &c.view()).unwrap();
    let committed = e.commit_step(g, result(), at(60), &c.view()).unwrap();
    assert_eq!(committed.attempt, 1);
    assert_eq!(committed.task_state, TaskState::Verifying);
    let loaded = e.load(tid(1)).unwrap();
    assert_eq!(loaded.steps[0].step.status.as_str(), "SUCCEEDED");
    assert_eq!(loaded.steps[0].step.attempt, 1);
    assert_eq!(loaded.task.state, TaskState::Verifying);
    assert_eq!(e.delete_task(tid(1)).unwrap().journal_rows, 9);
}
#[test]
fn exact_expiry_outcome_succeeds_but_expired_begin_refuses() {
    for begun in [false, true] {
        let c = Context::new();
        let mut e = engine();
        ready(&mut e, &c, StepKind::Notify);
        let g = acquire(&mut e, &c, None, 40, 50).unwrap();
        if begun {
            e.begin_attempt(&g, at(41), &c.view()).unwrap();
            assert!(e.commit_step(g, result(), at(50), &c.view()).is_ok());
        } else {
            assert_eq!(
                e.begin_attempt(&g, at(50), &c.view()),
                Err(EngineError::Store(StoreError::LeaseExpired))
            );
        }
    }
}
#[test]
fn same_owner_new_generation_fences_old_outcome_through_engine() {
    let c = Context::new();
    let mut e = engine();
    ready(&mut e, &c, StepKind::Notify);
    let old = acquire(&mut e, &c, None, 40, 50).unwrap();
    e.begin_attempt(&old, at(41), &c.view()).unwrap();
    let new = acquire(&mut e, &c, Some(1), 50, 70).unwrap();
    e.begin_attempt(&new, at(51), &c.view()).unwrap();
    assert_eq!(
        e.commit_step(old, result(), at(52), &c.view()).err(),
        Some(EngineError::Store(StoreError::LeaseFenced))
    );
    assert_eq!(e.load(tid(1)).unwrap().steps[0].step.attempt, 2);
    e.commit_step(new, result(), at(53), &c.view()).unwrap();
}
#[test]
fn release_preserves_steps_and_engine_distinguishes_ceiling_from_fence() {
    let c = Context::new();
    let mut e = engine();
    ready(&mut e, &c, StepKind::Notify);
    let first = acquire(&mut e, &c, None, 40, 100).unwrap();
    let before = e.load(tid(1)).unwrap().steps[0].step.clone();
    e.release(first, at(41), &c.view()).unwrap();
    assert_eq!(e.load(tid(1)).unwrap().steps[0].step, before);
    let second = acquire(&mut e, &c, Some(1), 42, 100).unwrap();
    e.release(second, at(43), &c.view()).unwrap();
    let third = acquire(&mut e, &c, Some(2), 44, 100).unwrap();
    e.release(third, at(45), &c.view()).unwrap();
    assert_eq!(
        acquire(&mut e, &c, Some(3), 46, 100).err(),
        Some(EngineError::Store(StoreError::AttemptCeilingReached))
    );
    assert_eq!(
        acquire(&mut e, &c, Some(2), 46, 100).err(),
        Some(EngineError::Store(StoreError::LeaseFenced))
    );
}
#[test]
fn wrong_parent_and_stale_expected_generation_do_not_mutate() {
    let c = Context::new();
    let mut e = engine();
    ready(&mut e, &c, StepKind::Notify);
    e.create_task(spec(2, DataClass::Public), &c.view())
        .unwrap();
    let before = e.load(tid(1)).unwrap().task;
    assert_eq!(
        e.acquire(
            tid(2),
            sid(1),
            LeaseOwner::new("other").unwrap(),
            None,
            at(40),
            at(50),
            &c.view()
        )
        .err(),
        Some(EngineError::Store(StoreError::LeaseFenced))
    );
    assert_eq!(
        acquire(&mut e, &c, Some(1), 40, 50).err(),
        Some(EngineError::Store(StoreError::LeaseFenced))
    );
    assert_eq!(e.load(tid(1)).unwrap().task, before);
}
#[test]
fn block_refusal_and_explicit_invariant_remediation_are_distinct() {
    let c = Context::new();
    let mut e = engine();
    e.create_task(spec(1, DataClass::Public), &c.view())
        .unwrap();
    let before = e.load(tid(1)).unwrap().task;
    assert_eq!(
        e.block(
            tid(1),
            TaskState::Received,
            BlockedReason::new("RESOURCE_UNAVAILABLE").unwrap(),
            at(20),
            &c.view()
        )
        .err(),
        Some(EngineError::IllegalTaskTransition)
    );
    assert_eq!(e.load(tid(1)).unwrap().task, before);
    e.fail_invariant(tid(1), TaskState::Received, at(20), &c.view())
        .unwrap();
    assert_eq!(
        e.load(tid(1))
            .unwrap()
            .task
            .failure_reason
            .unwrap()
            .as_str(),
        "INVARIANT_VIOLATION"
    );
    assert_eq!(
        e.fail_invariant(tid(1), TaskState::Failed, at(30), &c.view())
            .err(),
        Some(EngineError::IllegalTaskTransition)
    );
}
#[test]
fn blocking_and_replanning_clear_block_metadata_atomically() {
    let c = Context::new();
    let mut e = engine();
    prepare(&mut e, &c);
    e.block(
        tid(1),
        TaskState::Planning,
        BlockedReason::new("RESOURCE_UNAVAILABLE").unwrap(),
        at(21),
        &c.view(),
    )
    .unwrap();
    let loaded = e
        .start_planning(tid(1), TaskState::Blocked, 0, at(22), &c.view())
        .unwrap();
    assert_eq!(loaded.task.state, TaskState::Planning);
    assert!(loaded.task.blocked_reason.is_none());
}
#[test]
fn cancellation_and_second_noop_do_not_change_steps_or_timestamp() {
    let c = Context::new();
    let mut e = engine();
    ready(&mut e, &c, StepKind::Notify);
    let g = acquire(&mut e, &c, None, 40, 60).unwrap();
    e.begin_attempt(&g, at(41), &c.view()).unwrap();
    let before = e.load(tid(1)).unwrap().steps[0].step.clone();
    let out = e
        .cancel(
            tid(1),
            TaskOriginKind::new("USER_MESSAGE").unwrap(),
            at(42),
            &c.view(),
        )
        .unwrap();
    assert!(out.changed);
    assert!(!out.already_terminal);
    assert_eq!(out.cancelled_at, Some(at(42)));
    let cancelled = e.load(tid(1)).unwrap();
    assert_eq!(cancelled.steps[0].step, before);
    let noop = e
        .cancel(
            tid(1),
            TaskOriginKind::new("HOST").unwrap(),
            at(0),
            &c.view(),
        )
        .unwrap();
    assert!(!noop.changed);
    assert!(noop.already_terminal);
    assert_eq!(noop.cancelled_at, None);
    assert_eq!(e.load(tid(1)).unwrap().task, cancelled.task);
    assert_eq!(
        e.commit_step(g, result(), at(43), &c.view()).err(),
        Some(EngineError::Store(StoreError::LeaseFenced))
    );
}
#[test]
fn receipt_success_cancellation_and_delete_counts_preserve_ordering() {
    let c = Context::new();
    let mut e = engine();
    ready(&mut e, &c, StepKind::Capability);
    let g = acquire(&mut e, &c, None, 40, 50).unwrap();
    e.begin_attempt(&g, at(41), &c.view()).unwrap();
    assert_eq!(e.load(tid(1)).unwrap().task.state, TaskState::Executing);
    let step = e.load(tid(1)).unwrap().steps[0].step.clone();
    let receipt = SideEffectReceipt {
        receipt_id: ReceiptId::new("rcp_00000000000000000000000001").unwrap(),
        capability_id: step.capability_id.clone().unwrap(),
        idempotency_key: step.idempotency_key.clone().unwrap(),
        provider_reference: None,
        effect_summary: EffectSummary::new("recorded effect").unwrap(),
        observed_at: Timestamp::from_epoch_millis(at(42)),
        replay_safe: true,
    };
    e.commit_step(
        g,
        StepOutcome::Succeeded {
            result_json: b"{}",
            receipt: Some(&receipt),
        },
        at(42),
        &c.view(),
    )
    .unwrap();
    let succeeded = e.load(tid(1)).unwrap().steps[0].step.clone();
    assert_eq!(succeeded.side_effect_receipt, Some(receipt));
    e.cancel(
        tid(1),
        TaskOriginKind::new("USER_MESSAGE").unwrap(),
        at(43),
        &c.view(),
    )
    .unwrap();
    assert_eq!(e.load(tid(1)).unwrap().steps[0].step, succeeded);
    let out = e.delete_task(tid(1)).unwrap();
    assert_eq!(
        (
            out.task_rows,
            out.steps,
            out.receipts,
            out.leases,
            out.revisions,
            out.task_refs,
            out.step_refs,
            out.journal_rows,
            out.blobs
        ),
        (1, 1, 1, 1, 1, 2, 2, 13, 3)
    );
    assert_eq!(e.delete_task(tid(1)).unwrap(), DeletionOutcome::default());
}
#[test]
fn known_failure_without_details_roundtrips_checked_error() {
    let c = Context::new();
    let mut e = engine();
    ready(&mut e, &c, StepKind::Notify);
    let g = acquire(&mut e, &c, None, 40, 50).unwrap();
    e.begin_attempt(&g, at(41), &c.view()).unwrap();
    let code = ErrorCode::new("KNOWN_FAILURE").unwrap();
    let message = ErrorMessage::new("private diagnostic sentinel").unwrap();
    let action = HostAction::new("STOP").unwrap();
    let reason = FailureReason::new("KNOWN_FAILURE").unwrap();
    e.commit_step(
        g,
        StepOutcome::Failed(StepFailure {
            kind: ActionErrorKind::ProviderError,
            code: &code,
            message: &message,
            retryable: false,
            host_action: &action,
            details_json: None,
            failure_reason: &reason,
        }),
        at(42),
        &c.view(),
    )
    .unwrap();
    let loaded = e.load(tid(1)).unwrap();
    assert_eq!(loaded.task.state, TaskState::Failed);
    assert!(
        loaded.steps[0]
            .step
            .error
            .as_ref()
            .unwrap()
            .details
            .is_empty()
    );
}
#[test]
fn plan_layout_has_ordinary_prefix_and_verify_suffix_not_interleaving() {
    let c = Context::new();
    for (kinds, accepted) in [
        (vec![StepKind::Notify, StepKind::Notify], true),
        (
            vec![StepKind::Notify, StepKind::Verify, StepKind::Verify],
            true,
        ),
        (vec![StepKind::Verify, StepKind::Notify], false),
        (
            vec![
                StepKind::Notify,
                StepKind::Verify,
                StepKind::Notify,
                StepKind::Verify,
            ],
            false,
        ),
        (vec![StepKind::Verify], false),
    ] {
        let mut e = engine();
        prepare(&mut e, &c);
        let plan = Plan {
            revision: 1,
            steps: kinds
                .iter()
                .enumerate()
                .map(|(i, k)| input(1, i as u32 + 1, 10 * (i as u32 + 1), *k))
                .collect(),
        };
        let got = e.persist_plan(tid(1), plan, at(30), &c.view());
        assert_eq!(got.is_ok(), accepted);
        if !accepted {
            assert_eq!(e.load(tid(1)).unwrap().task.state, TaskState::Planning);
            assert!(e.load(tid(1)).unwrap().steps.is_empty());
        }
    }
}
#[test]
fn plan_parent_sequence_and_original_raw_input_validation_precede_writes() {
    let c = Context::new();
    for mode in 0..5 {
        let mut e = engine();
        prepare(&mut e, &c);
        let mut first = input(1, 1, 10, StepKind::Notify);
        let mut second = input(1, 2, 20, StepKind::Notify);
        match mode {
            0 => second = input(2, 2, 20, StepKind::Notify),
            1 => second = input(1, 2, 10, StepKind::Notify),
            2 => second.input_json = b"{\"duplicate\":1,\"duplicate\":2}".to_vec(),
            3 => second.input_json = b"{\"fraction\":1.5}".to_vec(),
            _ => {
                let mut d = TaskStepDraft::from(first.step);
                d.input_digest = digest_of("{}").unwrap();
                first.step = TaskStep::new(d).unwrap();
            }
        }
        assert!(
            e.persist_plan(
                tid(1),
                Plan {
                    revision: 1,
                    steps: vec![first, second]
                },
                at(30),
                &c.view()
            )
            .is_err()
        );
        assert_eq!(e.load(tid(1)).unwrap().plan_revision, 0);
        assert!(e.load(tid(1)).unwrap().steps.is_empty());
        assert_eq!(e.delete_task(tid(1)).unwrap().journal_rows, 2);
    }
}
#[test]
fn revision_append_planned_removal_and_lifetime_high_water_are_enforced() {
    let c = Context::new();
    let mut e = engine();
    ready(&mut e, &c, StepKind::Notify);
    e.start_planning(tid(1), TaskState::Ready, 1, at(31), &c.view())
        .unwrap();
    e.persist_plan(
        tid(1),
        Plan {
            revision: 2,
            steps: vec![
                input(1, 1, 10, StepKind::Notify),
                input(1, 2, 20, StepKind::Notify),
            ],
        },
        at(32),
        &c.view(),
    )
    .unwrap();
    e.start_planning(tid(1), TaskState::Ready, 2, at(33), &c.view())
        .unwrap();
    e.persist_plan(
        tid(1),
        Plan {
            revision: 3,
            steps: vec![input(1, 1, 10, StepKind::Notify)],
        },
        at(34),
        &c.view(),
    )
    .unwrap();
    e.start_planning(tid(1), TaskState::Ready, 3, at(35), &c.view())
        .unwrap();
    for (step, seq) in [(2, 30), (3, 20), (3, 5)] {
        assert_eq!(
            e.persist_plan(
                tid(1),
                Plan {
                    revision: 4,
                    steps: vec![
                        input(1, 1, 10, StepKind::Notify),
                        input(1, step, seq, StepKind::Notify)
                    ]
                },
                at(36),
                &c.view()
            )
            .err(),
            Some(EngineError::InvalidPlan)
        );
    }
    e.persist_plan(
        tid(1),
        Plan {
            revision: 4,
            steps: vec![
                input(1, 1, 10, StepKind::Notify),
                input(1, 3, 30, StepKind::Notify),
            ],
        },
        at(36),
        &c.view(),
    )
    .unwrap();
    let loaded = e.load(tid(1)).unwrap();
    assert_eq!(loaded.steps[0].plan_revision, 1);
    assert_eq!(loaded.steps[1].plan_revision, 4);
}
#[test]
fn renumber_skipped_revision_and_stale_replan_refuse_without_state_change() {
    let c = Context::new();
    let mut e = engine();
    ready(&mut e, &c, StepKind::Notify);
    assert_eq!(
        e.start_planning(tid(1), TaskState::Ready, 0, at(31), &c.view())
            .err(),
        Some(EngineError::IllegalTaskTransition)
    );
    e.start_planning(tid(1), TaskState::Ready, 1, at(31), &c.view())
        .unwrap();
    assert_eq!(
        e.persist_plan(
            tid(1),
            Plan {
                revision: 3,
                steps: vec![input(1, 1, 10, StepKind::Notify)]
            },
            at(32),
            &c.view()
        )
        .err(),
        Some(EngineError::PlanRevisionConflict)
    );
    assert_eq!(
        e.persist_plan(
            tid(1),
            Plan {
                revision: 2,
                steps: vec![input(1, 1, 11, StepKind::Notify)]
            },
            at(32),
            &c.view()
        )
        .err(),
        Some(EngineError::InvalidPlan)
    );
    assert_eq!(e.load(tid(1)).unwrap().plan_revision, 1);
    assert_eq!(e.load(tid(1)).unwrap().steps[0].step.sequence, 10);
}
#[test]
fn ever_leased_step_cannot_be_dropped_or_reset_to_planned_spec() {
    let c = Context::new();
    let mut e = engine();
    ready(&mut e, &c, StepKind::Notify);
    let g = acquire(&mut e, &c, None, 40, 50).unwrap();
    e.release(g, at(41), &c.view()).unwrap();
    e.start_planning(tid(1), TaskState::Ready, 1, at(42), &c.view())
        .unwrap();
    assert_eq!(
        e.persist_plan(
            tid(1),
            Plan {
                revision: 2,
                steps: vec![input(1, 2, 20, StepKind::Notify)]
            },
            at(43),
            &c.view()
        )
        .err(),
        Some(EngineError::PlanRevisionWouldDropExecutedStep)
    );
    assert_eq!(
        e.persist_plan(
            tid(1),
            Plan {
                revision: 2,
                steps: vec![input(1, 1, 10, StepKind::Notify)]
            },
            at(43),
            &c.view()
        )
        .err(),
        Some(EngineError::InvalidPlan)
    );
    let old = e.load(tid(1)).unwrap().steps[0].step.clone();
    e.persist_plan(
        tid(1),
        Plan {
            revision: 2,
            steps: vec![
                PlanStep {
                    step: old.clone(),
                    input_json: b"{\"value\":1}".to_vec(),
                },
                input(1, 2, 20, StepKind::Notify),
            ],
        },
        at(43),
        &c.view(),
    )
    .unwrap();
    assert_eq!(e.load(tid(1)).unwrap().steps[0].step, old);
}
struct NeverProtection;
impl AtRestProtection for NeverProtection {
    fn protect(&self, _: &[u8]) -> Result<Vec<u8>, AtRestProtectionError> {
        panic!("not an ordinary-row representation")
    }
    fn unprotect(&self, _: &[u8]) -> Result<Vec<u8>, AtRestProtectionError> {
        panic!("not an ordinary-row representation")
    }
}
#[test]
fn private_blob_backend_does_not_authorize_ordinary_task_data() {
    let c = Context::new();
    for class in [DataClass::Private, DataClass::Secret, DataClass::Credential] {
        let mut e = TaskEngine::new(
            Store::open_in_memory_with_protection(&Fixed, Arc::new(NeverProtection)).unwrap(),
            event_bus(),
        );
        assert_eq!(
            e.create_task(spec(1, class), &c.view()).err(),
            Some(EngineError::UnsupportedDataClass)
        );
        assert_eq!(e.load(tid(1)).err(), Some(EngineError::TaskNotFound));
    }
}
#[test]
fn engine_error_formatters_and_source_are_payload_free() {
    for e in [
        EngineError::Store(StoreError::Sqlite),
        EngineError::InvalidPlan,
        EngineError::UnsupportedDataClass,
    ] {
        let display = e.to_string();
        let debug = format!("{e:?}");
        assert_eq!(display, debug);
        assert!(!display.contains("sentinel"));
        assert!(std::error::Error::source(&e).is_none());
    }
}
#[test]
fn two_tasks_share_input_blob_but_delete_never_sweeps_other_task_or_standalone() {
    let c = Context::new();
    let fixture = FileFixture::new("candidate-sweep");
    let store = Store::open(&fixture.path, &Fixed).unwrap();
    let observer = Store::open(&fixture.path, &Fixed).unwrap();
    let standalone = store
        .transact(|tx| tx.put_blob(b"{\"standalone\":true}", DataClass::Public))
        .unwrap();
    let mut e = TaskEngine::new(store, event_bus());
    for n in [1, 2] {
        e.create_task(spec(n, DataClass::Personal), &c.view())
            .unwrap();
        e.start_planning(tid(n), TaskState::Received, 0, at(20), &c.view())
            .unwrap();
        let mut p = input(n, n, 10, StepKind::Notify);
        let raw = b"{\"shared\":true}".to_vec();
        let mut draft = TaskStepDraft::from(p.step);
        draft.input_digest = digest_of(std::str::from_utf8(&raw).unwrap()).unwrap();
        p.step = TaskStep::new(draft).unwrap();
        p.input_json = raw;
        e.persist_plan(
            tid(n),
            Plan {
                revision: 1,
                steps: vec![p],
            },
            at(30),
            &c.view(),
        )
        .unwrap();
    }
    assert_eq!(e.delete_task(tid(1)).unwrap().blobs, 1);
    assert_eq!(e.load(tid(2)).unwrap().steps.len(), 1);
    assert_eq!(e.delete_task(tid(2)).unwrap().blobs, 2);
    // Read the actual surviving blob, rather than merely retaining its identity.
    assert_eq!(
        observer.transact(|tx| tx.get_blob(&standalone)).unwrap(),
        b"{\"standalone\":true}"
    );
}

struct FileFixture {
    path: std::path::PathBuf,
}
impl FileFixture {
    fn new(name: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("serea-p2fb-{}-{name}", std::process::id()));
        std::fs::create_dir(&dir).unwrap();
        Self {
            path: dir.join("task.sqlite"),
        }
    }
    fn open(&self) -> TaskEngine {
        TaskEngine::new(Store::open(&self.path, &Fixed).unwrap(), event_bus())
    }
}
impl Drop for FileFixture {
    fn drop(&mut self) {
        std::fs::remove_dir_all(self.path.parent().unwrap()).unwrap();
    }
}
#[test]
fn task_projection_refuses_error_details_over_the_task_schema_property_cap() {
    let c = Context::new();
    let mut e = engine();
    ready(&mut e, &c, StepKind::Notify);
    let g = acquire(&mut e, &c, None, 40, 50).unwrap();
    e.begin_attempt(&g, at(41), &c.view()).unwrap();
    let details = (0..65)
        .map(|n| (format!("key-{n}"), serde_json::json!(n)))
        .collect::<serde_json::Map<_, _>>();
    let details_json = serde_json::to_vec(&details).unwrap();
    let code = ErrorCode::new("KNOWN_FAILURE").unwrap();
    let message = ErrorMessage::new("failure details cap sentinel").unwrap();
    let action = HostAction::new("STOP").unwrap();
    let reason = FailureReason::new("KNOWN_FAILURE").unwrap();
    assert_eq!(
        e.commit_step(
            g,
            StepOutcome::Failed(StepFailure {
                kind: ActionErrorKind::ProviderError,
                code: &code,
                message: &message,
                retryable: false,
                host_action: &action,
                details_json: Some(&details_json),
                failure_reason: &reason,
            }),
            at(42),
            &c.view(),
        )
        .err(),
        Some(EngineError::Store(StoreError::CanonicalJson))
    );
    let loaded = e.load(tid(1)).unwrap();
    assert_eq!(loaded.steps[0].step.status.as_str(), "EXECUTING");
    assert!(loaded.steps[0].step.error.is_none());

    let mut boundary = engine();
    ready(&mut boundary, &c, StepKind::Notify);
    let guard = acquire(&mut boundary, &c, None, 40, 50).unwrap();
    boundary.begin_attempt(&guard, at(41), &c.view()).unwrap();
    let details = (0..64)
        .map(|n| (format!("key-{n}"), serde_json::json!(n)))
        .collect::<serde_json::Map<_, _>>();
    let details_json = serde_json::to_vec(&details).unwrap();
    let code = ErrorCode::new("KNOWN_FAILURE").unwrap();
    let message = ErrorMessage::new("failure details boundary sentinel").unwrap();
    let action = HostAction::new("STOP").unwrap();
    let reason = FailureReason::new("KNOWN_FAILURE").unwrap();
    boundary
        .commit_step(
            guard,
            StepOutcome::Failed(StepFailure {
                kind: ActionErrorKind::ProviderError,
                code: &code,
                message: &message,
                retryable: false,
                host_action: &action,
                details_json: Some(&details_json),
                failure_reason: &reason,
            }),
            at(42),
            &c.view(),
        )
        .unwrap();
    let boundary_task = boundary.load(tid(1)).unwrap().task;
    assert_eq!(
        boundary_task.steps[0].error.as_ref().unwrap().details.len(),
        64
    );
    let wire = serde_json::to_value(&boundary_task).unwrap();
    serea_protocol::schema::validate(serea_protocol::schema::SchemaName::AssistantTask, &wire)
        .unwrap();
}

#[test]
fn reopened_unstarted_inflight_terminal_and_cancelled_rows_keep_provenance() {
    let f = FileFixture::new("provenance");
    let c = Context::new();
    let mut e = f.open();
    ready(&mut e, &c, StepKind::Capability);
    let expected = e.load(tid(1)).unwrap().task;
    drop(e);
    let mut e = f.open();
    assert_eq!(e.load(tid(1)).unwrap().task, expected);
    let g = acquire(&mut e, &c, None, 40, 100).unwrap();
    e.begin_attempt(&g, at(41), &c.view()).unwrap();
    let expected = e.load(tid(1)).unwrap().task;
    drop(e);
    let mut e = f.open();
    assert_eq!(e.load(tid(1)).unwrap().task, expected);
    let s = &expected.steps[0];
    let receipt = SideEffectReceipt {
        receipt_id: ReceiptId::new("rcp_00000000000000000000000001").unwrap(),
        capability_id: s.capability_id.clone().unwrap(),
        idempotency_key: s.idempotency_key.clone().unwrap(),
        provider_reference: None,
        effect_summary: EffectSummary::new("durable").unwrap(),
        observed_at: Timestamp::from_epoch_millis(at(42)),
        replay_safe: true,
    };
    e.commit_step(
        g,
        StepOutcome::Succeeded {
            result_json: b"{}",
            receipt: Some(&receipt),
        },
        at(42),
        &c.view(),
    )
    .unwrap();
    let expected = e.load(tid(1)).unwrap().task;
    drop(e);
    let mut e = f.open();
    assert_eq!(e.load(tid(1)).unwrap().task, expected);
    e.cancel(
        tid(1),
        TaskOriginKind::new("USER_MESSAGE").unwrap(),
        at(43),
        &c.view(),
    )
    .unwrap();
    let expected = e.load(tid(1)).unwrap().task;
    drop(e);
    let e = f.open();
    assert_eq!(e.load(tid(1)).unwrap().task, expected);
}
#[test]
fn separate_engine_store_reclaim_fences_stale_worker() {
    let f = FileFixture::new("cross-store");
    let c = Context::new();
    let mut a = f.open();
    ready(&mut a, &c, StepKind::Notify);
    let mut b = f.open();
    let old = acquire(&mut a, &c, None, 40, 50).unwrap();
    a.begin_attempt(&old, at(41), &c.view()).unwrap();
    let new = acquire(&mut b, &c, Some(1), 50, 70).unwrap();
    b.begin_attempt(&new, at(51), &c.view()).unwrap();
    assert_eq!(
        a.commit_step(old, result(), at(52), &c.view()).err(),
        Some(EngineError::Store(StoreError::LeaseFenced))
    );
    b.commit_step(new, result(), at(53), &c.view()).unwrap();
    assert_eq!(a.load(tid(1)).unwrap().steps[0].step.attempt, 2);
}
#[test]
fn verify_suffix_completes_only_after_real_verify_outcome() {
    let c = Context::new();
    let mut e = engine();
    prepare(&mut e, &c);
    e.persist_plan(
        tid(1),
        Plan {
            revision: 1,
            steps: vec![
                input(1, 1, 10, StepKind::Notify),
                input(1, 2, 20, StepKind::Verify),
            ],
        },
        at(30),
        &c.view(),
    )
    .unwrap();
    let g = acquire(&mut e, &c, None, 40, 50).unwrap();
    e.begin_attempt(&g, at(41), &c.view()).unwrap();
    assert_eq!(
        e.commit_step(g, result(), at(42), &c.view())
            .unwrap()
            .task_state,
        TaskState::Verifying
    );
    let v = e
        .acquire(
            tid(1),
            sid(2),
            LeaseOwner::new("worker-v").unwrap(),
            None,
            at(43),
            at(60),
            &c.view(),
        )
        .unwrap();
    e.begin_attempt(&v, at(44), &c.view()).unwrap();
    assert_eq!(
        e.commit_step(v, result(), at(45), &c.view())
            .unwrap()
            .task_state,
        TaskState::Completed
    );
    assert!(
        !e.cancel(
            tid(1),
            TaskOriginKind::new("HOST").unwrap(),
            at(0),
            &c.view()
        )
        .unwrap()
        .changed
    );
}

#[test]
fn no_recovery_execution_sql_or_upward_dependency_is_exposed() {
    let source = include_str!("../src/engine.rs");
    for forbidden in [
        "pub fn recover",
        "pub fn execute",
        "pub fn run",
        "pub fn poll",
        "pub fn worker",
        "pub fn provider",
        "pub fn schedule",
        "pub fn set_state",
        "SystemTime",
        "Instant",
        "Clock",
        "rusqlite",
    ] {
        assert!(!source.contains(forbidden), "{forbidden}");
    }
    let manifest = include_str!("../Cargo.toml")
        .split("[dev-dependencies]")
        .next()
        .unwrap();
    assert!(!manifest.contains("rusqlite"));
    assert!(!manifest.contains("serea-testkit"));
    assert!(!include_str!("../../serea-storage/Cargo.toml").contains("serea-task-engine"));
    assert!(!include_str!("../../serea-storage/src/outcome.rs").contains("fn outcome_journal"));
}

#[test]
fn persisted_task_wire_validates_against_checked_in_schema() {
    let c = Context::new();
    let mut e = engine();
    ready(&mut e, &c, StepKind::Capability);
    let task = e.load(tid(1)).unwrap().task;
    let wire = serde_json::to_value(&task).unwrap();
    serea_protocol::schema::validate(serea_protocol::schema::SchemaName::AssistantTask, &wire)
        .unwrap();
    assert_eq!(serde_json::from_value::<AssistantTask>(wire).unwrap(), task);
}

struct RealAuditProbe {
    kinds: std::sync::Mutex<Vec<String>>,
}
impl RealAuditProbe {
    fn new() -> Self {
        Self {
            kinds: std::sync::Mutex::new(Vec::new()),
        }
    }
}
impl serea_storage::TaskAuditParticipant for RealAuditProbe {
    fn records(
        &self,
        f: &serea_storage::DurableTransition,
    ) -> Result<serea_storage::JournalRecords, StoreError> {
        assert_eq!(f.actor_id().as_str(), "workflow-host");
        assert_eq!(f.actor_version().as_str(), "0.2.0");
        assert_eq!(
            f.causation_id().unwrap().as_str(),
            "evt_00000000000000000000000001"
        );
        assert_eq!(f.data_class(), DataClass::Personal);
        let rows = serea_storage::TaskAuditParticipant::records(&TaskJournal, f)?;
        for row in &rows {
            let text = std::str::from_utf8(&row.payload_json).unwrap();
            assert!(!text.contains("sentinel"));
            if f.operation() == serea_storage::AuditOperation::AttemptStarted {
                assert_eq!(
                    text,
                    "{\"attempt\":1,\"generation\":1,\"result_digest\":null}"
                );
            }
            self.kinds
                .lock()
                .unwrap()
                .push(row.kind.wire_name().to_owned());
        }
        Ok(rows)
    }
}
#[test]
fn real_journal_mapper_has_literal_order_no_duplicates_and_terminal_noop() {
    let c = Context::new();
    let created = engine()
        .create_task(spec(1, DataClass::Personal), &c.view())
        .unwrap()
        .task;
    let store = Store::open_in_memory(&Fixed).unwrap();
    let probe = RealAuditProbe::new();
    store
        .transact_with_participants(&probe, &event_bus(), |tx| {
            tx.insert_task(&created, &c.view())?;
            tx.start_planning(&tid(1), TaskState::Received, 0, at(20), &c.view())?;
            let p = input(1, 1, 10, StepKind::Notify);
            tx.put_plan_revision(
                &tid(1),
                serea_storage::PlanWrite {
                    revision: 1,
                    steps: vec![serea_storage::StepInput {
                        step: p.step,
                        input_json: p.input_json,
                    }],
                },
                at(30),
                &c.view(),
            )?;
            let g = tx.acquire_audited(
                tid(1),
                sid(1),
                LeaseOwner::new("worker-a").unwrap(),
                None,
                at(40),
                at(50),
                &c.view(),
            )?;
            tx.begin_attempt(&g, at(41), &c.view())?;
            tx.commit_step_outcome(g, result(), at(42), &c.view())?;
            tx.cancel_task(
                &tid(1),
                TaskOriginKind::new("USER_MESSAGE").unwrap(),
                at(43),
                &c.view(),
            )?;
            assert!(
                !tx.cancel_task(
                    &tid(1),
                    TaskOriginKind::new("HOST").unwrap(),
                    at(0),
                    &c.view()
                )?
                .changed
            );
            Ok(())
        })
        .unwrap();
    assert_eq!(
        *probe.kinds.lock().unwrap(),
        [
            "TASK_INSERTED",
            "TASK_STATE_CHANGED",
            "PLAN_PERSISTED",
            "TASK_STATE_CHANGED",
            "STEP_LEASE_ACQUIRED",
            "STEP_ATTEMPT_STARTED",
            "TASK_STATE_CHANGED",
            "STEP_COMMITTED",
            "TASK_STATE_CHANGED",
            "TASK_CANCEL_REQUESTED",
            "TASK_STATE_CHANGED",
            "TASK_TERMINAL"
        ]
    );
    assert_eq!(
        store
            .transact(|tx| tx.delete_task(&tid(1)))
            .unwrap()
            .journal_rows,
        12
    );
}
#[test]
fn outer_error_rolls_back_real_mapped_planning_and_journal() {
    let c = Context::new();
    let created = engine()
        .create_task(spec(1, DataClass::Personal), &c.view())
        .unwrap()
        .task;
    let store = Store::open_in_memory(&Fixed).unwrap();
    store
        .transact_with_participants(&TaskJournal, &event_bus(), |tx| {
            tx.insert_task(&created, &c.view())
        })
        .unwrap();
    let probe = RealAuditProbe::new();
    assert_eq!(
        store.transact_with_participants(&probe, &event_bus(), |tx| {
            tx.start_planning(&tid(1), TaskState::Received, 0, at(20), &c.view())?;
            Err::<(), _>(StoreError::Sqlite)
        }),
        Err(StoreError::Sqlite)
    );
    assert_eq!(*probe.kinds.lock().unwrap(), ["TASK_STATE_CHANGED"]);
    assert_eq!(store.load_task(&tid(1)).unwrap().task, created);
    assert_eq!(
        store
            .transact(|tx| tx.delete_task(&tid(1)))
            .unwrap()
            .journal_rows,
        1
    );
}
