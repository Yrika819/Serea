//! Accepted frozen-generation-0 regressions only. Independent literal oracles
//! exercise the public engine and the real journal through storage-owned facts.
use serea_protocol::*;
use serea_storage::{Store, StoreError};
use serea_task_engine::*;
use std::sync::Mutex;

fn at(n: i64) -> EpochMillis {
    EpochMillis::new(n).unwrap()
}
struct Fixed;
impl Clock for Fixed {
    fn now_ms(&self) -> Result<EpochMillis, ProtocolError> {
        Ok(at(0))
    }
}
fn tid() -> TaskId {
    TaskId::new("tsk_00000000000000000000000001").unwrap()
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
            actor: ActorId::new("review-host").unwrap(),
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
fn spec() -> NewTask {
    NewTask {
        task_id: tid(),
        kind: TaskKind::UserRequest,
        title: TaskTitle::new("review task").unwrap(),
        origin: TaskOrigin {
            kind: TaskOriginKind::new("USER_MESSAGE").unwrap(),
            device_id: None,
            message_id: None,
            extensions: Default::default(),
        },
        data_class: DataClass::Personal,
        policy_class: RiskClass::Communication,
        attempt_budget: AttemptBudget {
            max_model_calls: 12,
            max_tool_calls: 24,
            max_attempts_per_step: 3,
            extensions: Default::default(),
        },
        created_at: at(10),
        deadline_at: Some(at(1000)),
        extensions: Default::default(),
    }
}
fn input(n: u32, kind: StepKind) -> PlanStep {
    let raw = format!("{{\"value\":{n}}}").into_bytes();
    let shaped = matches!(
        kind,
        StepKind::Capability | StepKind::Delegate | StepKind::Verify
    );
    let capability = CapabilityId::new("calendar.events.create").unwrap();
    let version = SemVer::new("1.0.0").unwrap();
    let step = TaskStep::new(TaskStepDraft {
        task_id: tid(),
        step_id: sid(n),
        sequence: n * 10,
        kind,
        status: StepStatus::new("PLANNED").unwrap(),
        attempt: 0,
        idempotency_key: shaped.then(|| {
            derive_idempotency_key(
                &tid(),
                &sid(n),
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
    TaskEngine::new(Store::open_in_memory(&Fixed).unwrap())
}
fn prepare(e: &mut TaskEngine, c: &Context, steps: Vec<PlanStep>) {
    e.create_task(spec(), &c.view()).unwrap();
    e.start_planning(tid(), TaskState::Received, 0, at(20), &c.view())
        .unwrap();
    e.persist_plan(tid(), Plan { revision: 1, steps }, at(30), &c.view())
        .unwrap();
}
fn acquire(e: &mut TaskEngine, c: &Context, n: u32, now: i64) -> LeaseGuard {
    e.acquire(
        tid(),
        sid(n),
        LeaseOwner::new("review-worker").unwrap(),
        None,
        at(now),
        at(900),
        &c.view(),
    )
    .unwrap()
}
fn result<'a>() -> StepOutcome<'a> {
    StepOutcome::Succeeded {
        result_json: b"{}",
        receipt: None,
    }
}
fn receipt(step: &TaskStep) -> SideEffectReceipt {
    SideEffectReceipt {
        receipt_id: ReceiptId::new("rcp_00000000000000000000000001").unwrap(),
        capability_id: step.capability_id.clone().unwrap(),
        idempotency_key: step.idempotency_key.clone().unwrap(),
        provider_reference: None,
        effect_summary: EffectSummary::new("recorded effect").unwrap(),
        observed_at: Timestamp::from_epoch_millis(at(42)),
        replay_safe: true,
    }
}
fn exact_membership(e: &TaskEngine) -> Vec<PlanStep> {
    e.load(tid())
        .unwrap()
        .steps
        .into_iter()
        .map(|s| {
            let n = s.step.sequence / 10;
            PlanStep {
                input_json: canonicalize(
                    std::str::from_utf8(&input(n, s.step.kind).input_json).unwrap(),
                )
                .unwrap(),
                step: s.step,
            }
        })
        .collect()
}
fn assert_same(actual: &TaskRecord, expected: &TaskRecord) {
    assert_eq!(actual.task, expected.task);
    assert_eq!(actual.plan_revision, expected.plan_revision);
    assert!(
        actual.steps == expected.steps,
        "step runtime/provenance changed"
    );
}
struct FileFixture {
    path: std::path::PathBuf,
}
impl FileFixture {
    fn new(name: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("serea-review-{}-{name}", std::process::id()));
        std::fs::create_dir(&dir).unwrap();
        Self {
            path: dir.join("task.sqlite"),
        }
    }
    fn open(&self) -> TaskEngine {
        TaskEngine::new(Store::open(&self.path, &Fixed).unwrap())
    }
}
impl Drop for FileFixture {
    fn drop(&mut self) {
        std::fs::remove_dir_all(self.path.parent().unwrap()).unwrap();
    }
}

#[test]
fn f1_blocked_verifying_with_leased_verifier_cannot_publish_ready() {
    let c = Context::new();
    let mut e = engine();
    prepare(
        &mut e,
        &c,
        vec![input(1, StepKind::Notify), input(2, StepKind::Verify)],
    );
    let g = acquire(&mut e, &c, 1, 40);
    e.begin_attempt(&g, at(41), &c.view()).unwrap();
    assert_eq!(
        e.commit_step(g, result(), at(42), &c.view())
            .unwrap()
            .task_state,
        TaskState::Verifying
    );
    let verifier = acquire(&mut e, &c, 2, 43);
    e.block(
        tid(),
        TaskState::Verifying,
        BlockedReason::new("POLICY_BOUND").unwrap(),
        at(44),
        &c.view(),
    )
    .unwrap();
    e.start_planning(tid(), TaskState::Blocked, 1, at(45), &c.view())
        .unwrap();
    let before = e.load(tid()).unwrap();
    assert_eq!(before.steps[1].step.status.as_str(), "LEASED");
    let error = e
        .persist_plan(
            tid(),
            Plan {
                revision: 2,
                steps: exact_membership(&e),
            },
            at(46),
            &c.view(),
        )
        .err();
    assert_eq!(error, Some(EngineError::InvalidPlan));
    assert_same(&e.load(tid()).unwrap(), &before);
    // The refusal must not consume or rewrite the retained verifier authority.
    e.release(verifier, at(47), &c.view()).unwrap();
    let counts = e.delete_task(tid()).unwrap();
    assert_eq!(counts.revisions, 1);
    assert_eq!(counts.journal_rows, 13);
}

#[test]
fn f1_blocked_executing_ordinary_replan_refuses_without_resetting_runtime() {
    let c = Context::new();
    let mut e = engine();
    prepare(
        &mut e,
        &c,
        vec![input(1, StepKind::Notify), input(2, StepKind::Notify)],
    );
    let g = acquire(&mut e, &c, 1, 40);
    e.begin_attempt(&g, at(41), &c.view()).unwrap();
    e.block(
        tid(),
        TaskState::Executing,
        BlockedReason::new("POLICY_BOUND").unwrap(),
        at(42),
        &c.view(),
    )
    .unwrap();
    e.start_planning(tid(), TaskState::Blocked, 1, at(43), &c.view())
        .unwrap();
    let before = e.load(tid()).unwrap();
    assert_eq!(before.steps[0].step.status.as_str(), "EXECUTING");
    let error = e
        .persist_plan(
            tid(),
            Plan {
                revision: 2,
                steps: exact_membership(&e),
            },
            at(44),
            &c.view(),
        )
        .err();
    assert_eq!(error, Some(EngineError::InvalidPlan));
    assert_same(&e.load(tid()).unwrap(), &before);
    e.release(g, at(45), &c.view()).unwrap();
    let counts = e.delete_task(tid()).unwrap();
    assert_eq!(counts.revisions, 1);
    assert_eq!(counts.journal_rows, 10);
}

#[test]
fn f1_all_ordinary_succeeded_membership_needs_fresh_ordinary_work_to_publish_ready() {
    let c = Context::new();
    let mut e = engine();
    prepare(&mut e, &c, vec![input(1, StepKind::Notify)]);
    let g = acquire(&mut e, &c, 1, 40);
    e.begin_attempt(&g, at(41), &c.view()).unwrap();
    assert_eq!(
        e.commit_step(g, result(), at(42), &c.view())
            .unwrap()
            .task_state,
        TaskState::Verifying
    );
    e.block(
        tid(),
        TaskState::Verifying,
        BlockedReason::new("POLICY_BOUND").unwrap(),
        at(43),
        &c.view(),
    )
    .unwrap();
    e.start_planning(tid(), TaskState::Blocked, 1, at(44), &c.view())
        .unwrap();
    let before = e.load(tid()).unwrap();
    assert_eq!(before.steps[0].step.status.as_str(), "SUCCEEDED");
    assert_eq!(
        e.persist_plan(
            tid(),
            Plan {
                revision: 2,
                steps: exact_membership(&e)
            },
            at(45),
            &c.view()
        )
        .err(),
        Some(EngineError::InvalidPlan)
    );
    assert_same(&e.load(tid()).unwrap(), &before);
    let mut steps = exact_membership(&e);
    steps.push(input(2, StepKind::Notify));
    e.persist_plan(tid(), Plan { revision: 2, steps }, at(46), &c.view())
        .unwrap();
    let after = e.load(tid()).unwrap();
    assert_eq!(after.task.state, TaskState::Ready);
    assert_eq!(after.plan_revision, 2);
    assert!(after.steps[0] == before.steps[0]);
    assert_eq!(after.steps[1].step.status.as_str(), "PLANNED");
    assert_eq!(after.steps[1].plan_revision, 2);
    let g = acquire(&mut e, &c, 2, 47);
    e.begin_attempt(&g, at(48), &c.view()).unwrap();
    assert_eq!(
        e.commit_step(g, result(), at(49), &c.view())
            .unwrap()
            .task_state,
        TaskState::Verifying
    );
    assert!(e.load(tid()).unwrap().steps[0] == before.steps[0]);
    let counts = e.delete_task(tid()).unwrap();
    assert_eq!(
        (counts.steps, counts.revisions, counts.journal_rows),
        (2, 2, 18)
    );
}

#[test]
fn f3_depth64_failure_details_commit_load_and_reopen_without_projection_depth_rejection() {
    let f = FileFixture::new("deep-failure");
    let c = Context::new();
    let mut e = f.open();
    prepare(&mut e, &c, vec![input(1, StepKind::Notify)]);
    let g = acquire(&mut e, &c, 1, 40);
    e.begin_attempt(&g, at(41), &c.view()).unwrap();
    // SCJ-1 counts the root object and scalar leaf: 62 arrays reaches depth 64.
    let details = format!("{{\"nested\":{}0{}}}", "[".repeat(62), "]".repeat(62));
    assert_eq!(canonicalize(&details).unwrap(), details.as_bytes());
    let over_limit = format!("{{\"nested\":{}0{}}}", "[".repeat(63), "]".repeat(63));
    assert!(canonicalize(&over_limit).is_err());
    let code = ErrorCode::new("KNOWN_FAILURE").unwrap();
    let message = ErrorMessage::new("complete diagnostic").unwrap();
    let action = HostAction::new("STOP").unwrap();
    let reason = FailureReason::new("KNOWN_FAILURE").unwrap();
    let committed = e
        .commit_step(
            g,
            StepOutcome::Failed(StepFailure {
                kind: ActionErrorKind::ProviderError,
                code: &code,
                message: &message,
                retryable: false,
                host_action: &action,
                details_json: Some(details.as_bytes()),
                failure_reason: &reason,
            }),
            at(42),
            &c.view(),
        )
        .unwrap();
    assert_eq!(committed.task_state, TaskState::Failed);
    let expected = e.load(tid()).unwrap();
    assert_eq!(expected.task.failure_reason, Some(reason));
    let step = &expected.steps[0].step;
    assert_eq!(step.status.as_str(), "FAILED");
    assert_eq!(step.attempt, 1);
    assert_eq!(step.lease_generation, Some(1));
    assert_eq!(step.started_at, Some(Timestamp::from_epoch_millis(at(41))));
    assert_eq!(
        step.completed_at,
        Some(Timestamp::from_epoch_millis(at(42)))
    );
    assert!(step.result_digest.is_none());
    assert!(committed.result.is_none());
    let error = step.error.as_ref().unwrap();
    assert_eq!(error.kind, ActionErrorKind::ProviderError);
    assert_eq!(error.code, code);
    assert_eq!(error.message, message);
    assert!(!error.retryable);
    assert_eq!(error.host_action, action);
    assert_eq!(
        serde_json::to_value(&error.details).unwrap(),
        serde_json::from_str::<serde_json::Value>(&details).unwrap()
    );
    drop(e);
    let mut e = f.open();
    assert_same(&e.load(tid()).unwrap(), &expected);
    let counts = e.delete_task(tid()).unwrap();
    assert_eq!((counts.revisions, counts.journal_rows), (1, 10));
}

// The outer match intentionally has no wildcard: a new protocol state requires
// an explicit independent rule/code decision here, not just in production.
fn frozen_rules(from: TaskState) -> &'static [(TaskState, TaskTransitionReason, &'static str)] {
    use TaskState::*;
    use TaskTransitionReason as R;
    match from {
        Received => &[
            (Planning, R::StartPlanning, "START_PLANNING"),
            (Failed, R::Fail, "FAIL"),
            (Cancelled, R::Cancel, "CANCEL"),
        ],
        Planning => &[
            (Ready, R::PlanPersisted, "PLAN_PERSISTED"),
            (WaitingApproval, R::AwaitApproval, "AWAIT_APPROVAL"),
            (WaitingUser, R::AwaitUser, "AWAIT_USER"),
            (Blocked, R::Block, "BLOCK"),
            (Failed, R::Fail, "FAIL"),
            (Cancelled, R::Cancel, "CANCEL"),
        ],
        Ready => &[
            (Planning, R::Replan, "REPLAN"),
            (Executing, R::StartExecution, "START_EXECUTION"),
            (WaitingApproval, R::AwaitApproval, "AWAIT_APPROVAL"),
            (Failed, R::Fail, "FAIL"),
            (Cancelled, R::Cancel, "CANCEL"),
        ],
        Executing => &[
            (Ready, R::ContinueExecution, "CONTINUE_EXECUTION"),
            (WaitingApproval, R::AwaitApproval, "AWAIT_APPROVAL"),
            (WaitingUser, R::AwaitUser, "AWAIT_USER"),
            (Verifying, R::EnterVerification, "ENTER_VERIFICATION"),
            (Blocked, R::Block, "BLOCK"),
            (Failed, R::Fail, "FAIL"),
            (Cancelled, R::Cancel, "CANCEL"),
        ],
        WaitingApproval => &[
            (Ready, R::ResumeReady, "RESUME_READY"),
            (Failed, R::Fail, "FAIL"),
            (Cancelled, R::Cancel, "CANCEL"),
        ],
        WaitingUser => &[
            (Planning, R::Replan, "REPLAN"),
            (Ready, R::ResumeReady, "RESUME_READY"),
            (Failed, R::Fail, "FAIL"),
            (Cancelled, R::Cancel, "CANCEL"),
        ],
        Verifying => &[
            (
                Executing,
                R::VerificationRequiresExecution,
                "VERIFICATION_REQUIRES_EXECUTION",
            ),
            (Blocked, R::Block, "BLOCK"),
            (Completed, R::VerificationComplete, "VERIFICATION_COMPLETE"),
            (Failed, R::Fail, "FAIL"),
            (Cancelled, R::Cancel, "CANCEL"),
        ],
        Blocked => &[
            (Planning, R::Replan, "REPLAN"),
            (Ready, R::ResumeReady, "RESUME_READY"),
            (Failed, R::Fail, "FAIL"),
            (Cancelled, R::Cancel, "CANCEL"),
        ],
        Completed => &[],
        Failed => &[],
        Cancelled => &[],
    }
}
#[test]
fn t2_every_pair_has_the_exact_frozen_typed_rule_and_literal_code() {
    let states = [
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
    let mut names = states.map(TaskState::wire_name);
    names.sort_unstable();
    let mut wire_names = TaskState::WIRE_NAMES.to_vec();
    wire_names.sort_unstable();
    assert_eq!(names.as_slice(), wire_names);
    let mut legal = 0;
    for from in states {
        for to in states {
            let expected = frozen_rules(from).iter().find(|entry| entry.0 == to);
            let actual = task_transition_reason(from, to);
            assert_eq!(actual, expected.map(|entry| entry.1), "{from} -> {to}");
            assert_eq!(
                actual.map(TaskTransitionReason::wire_name),
                expected.map(|entry| entry.2),
                "{from} -> {to} reason code"
            );
            assert_eq!(
                legal_task_transition(from, to),
                expected.is_some(),
                "{from} -> {to} legality"
            );
            legal += usize::from(expected.is_some());
        }
    }
    assert_eq!(legal, 37);
}

#[test]
fn t3_succeeded_capability_prefix_keeps_runtime_and_original_revision_across_appends_and_reopen() {
    let f = FileFixture::new("succeeded-revisions");
    let c = Context::new();
    let mut e = f.open();
    prepare(
        &mut e,
        &c,
        vec![input(1, StepKind::Capability), input(2, StepKind::Notify)],
    );
    let g = acquire(&mut e, &c, 1, 40);
    e.begin_attempt(&g, at(41), &c.view()).unwrap();
    let r = receipt(&e.load(tid()).unwrap().steps[0].step);
    assert_eq!(
        e.commit_step(
            g,
            StepOutcome::Succeeded {
                result_json: b"{}",
                receipt: Some(&r)
            },
            at(42),
            &c.view()
        )
        .unwrap()
        .task_state,
        TaskState::Ready
    );
    let original = e.load(tid()).unwrap().steps[0].clone();
    assert_eq!(original.plan_revision, 1);
    assert_eq!(original.step.status.as_str(), "SUCCEEDED");
    assert_eq!(original.step.side_effect_receipt, Some(r));
    assert_eq!(original.step.result_digest, Some(digest_of("{}").unwrap()));
    assert_eq!(original.step.attempt, 1);
    assert_eq!(original.step.lease_generation, Some(1));
    assert_eq!(
        original.step.started_at,
        Some(Timestamp::from_epoch_millis(at(41)))
    );
    assert_eq!(
        original.step.completed_at,
        Some(Timestamp::from_epoch_millis(at(42)))
    );
    assert!(original.step.lease_owner.is_none());
    assert!(original.step.lease_expires_at.is_none());
    assert_eq!(
        original.step.extensions,
        input(1, StepKind::Capability).step.extensions
    );
    for revision in 2..=3 {
        e.start_planning(
            tid(),
            TaskState::Ready,
            revision - 1,
            at(30 + 10 * i64::from(revision)),
            &c.view(),
        )
        .unwrap();
        let mut steps = exact_membership(&e);
        assert_eq!(steps[0].step, original.step);
        assert_eq!(steps[0].input_json, canonicalize("{\"value\":1}").unwrap());
        steps.push(input(revision + 1, StepKind::Notify));
        e.persist_plan(
            tid(),
            Plan { revision, steps },
            at(35 + 10 * i64::from(revision)),
            &c.view(),
        )
        .unwrap();
        let expected = e.load(tid()).unwrap();
        assert_eq!(expected.plan_revision, revision);
        assert_eq!(expected.task.state, TaskState::Ready);
        assert!(
            expected.steps[0] == original,
            "succeeded provenance changed at revision {revision}"
        );
        assert_eq!(expected.steps[1].step.status.as_str(), "PLANNED");
        assert_eq!(expected.steps[1].plan_revision, 1);
        assert_eq!(expected.steps.last().unwrap().plan_revision, revision);
        drop(e);
        e = f.open();
        assert_same(&e.load(tid()).unwrap(), &expected);
        assert!(e.load(tid()).unwrap().steps[0] == original);
    }
    // All-ordinary succeeded prefix + pending/appended work is actually runnable.
    let g = acquire(&mut e, &c, 2, 70);
    e.begin_attempt(&g, at(71), &c.view()).unwrap();
    assert_eq!(
        e.commit_step(g, result(), at(72), &c.view())
            .unwrap()
            .task_state,
        TaskState::Ready
    );
    assert!(e.load(tid()).unwrap().steps[0] == original);
    let counts = e.delete_task(tid()).unwrap();
    assert_eq!(
        (
            counts.steps,
            counts.receipts,
            counts.revisions,
            counts.journal_rows
        ),
        (4, 1, 3, 21)
    );
}

fn refuse_fresh_plan(candidate: PlanStep) {
    let c = Context::new();
    let mut e = engine();
    e.create_task(spec(), &c.view()).unwrap();
    e.start_planning(tid(), TaskState::Received, 0, at(20), &c.view())
        .unwrap();
    let before = e.load(tid()).unwrap();
    assert_eq!(
        e.persist_plan(
            tid(),
            Plan {
                revision: 1,
                steps: vec![candidate]
            },
            at(30),
            &c.view()
        )
        .err(),
        Some(EngineError::InvalidPlan)
    );
    assert_same(&e.load(tid()).unwrap(), &before);
    let counts = e.delete_task(tid()).unwrap();
    assert_eq!(
        (
            counts.task_rows,
            counts.steps,
            counts.receipts,
            counts.leases,
            counts.revisions,
            counts.task_refs,
            counts.step_refs,
            counts.journal_rows,
            counts.blobs
        ),
        (1, 0, 0, 0, 0, 0, 0, 2, 0)
    );
}
#[test]
fn t5_fresh_protocol_checked_leased_runtime_is_not_planner_authority() {
    let mut p = input(1, StepKind::Notify);
    let mut draft = TaskStepDraft::from(p.step);
    draft.status = StepStatus::new("LEASED").unwrap();
    draft.attempt = 1;
    draft.lease_owner = Some(LeaseOwner::new("forged-worker").unwrap());
    draft.lease_expires_at = Some(Timestamp::from_epoch_millis(at(900)));
    draft.lease_generation = Some(1);
    p.step = TaskStep::new(draft).unwrap();
    refuse_fresh_plan(p);
}
#[test]
fn t5_unknown_protocol_status_is_refused_without_any_plan_writes() {
    let mut p = input(1, StepKind::Notify);
    let mut draft = TaskStepDraft::from(p.step);
    draft.status = StepStatus::new("FUTURE_STATUS").unwrap();
    p.step = TaskStep::new(draft).unwrap();
    refuse_fresh_plan(p);
}
#[test]
fn t5_scalar_capability_input_with_matching_digest_and_idk_is_refused() {
    let mut p = input(1, StepKind::Capability);
    p.input_json = b"7".to_vec();
    let mut draft = TaskStepDraft::from(p.step);
    draft.input_digest = digest_of("7").unwrap();
    draft.idempotency_key = Some(
        derive_idempotency_key(
            &tid(),
            &sid(1),
            draft.capability_id.as_ref().unwrap(),
            draft.capability_version.as_ref().unwrap(),
            "7",
        )
        .unwrap(),
    );
    p.step = TaskStep::new(draft).unwrap();
    refuse_fresh_plan(p);
}
#[test]
fn t5_well_formed_but_wrong_capability_idk_is_refused() {
    let mut p = input(1, StepKind::Capability);
    let mut draft = TaskStepDraft::from(p.step);
    let wrong = derive_idempotency_key(
        &tid(),
        &sid(2),
        draft.capability_id.as_ref().unwrap(),
        draft.capability_version.as_ref().unwrap(),
        std::str::from_utf8(&p.input_json).unwrap(),
    )
    .unwrap();
    assert_ne!(draft.idempotency_key.as_ref(), Some(&wrong));
    draft.idempotency_key = Some(wrong);
    p.step = TaskStep::new(draft).unwrap();
    refuse_fresh_plan(p);
}

// Independent SCJ-1 '{}' test vector and P2F-a payload bytes, not mapper output.
const EMPTY_DIGEST: &str =
    "sha256:44136fa355b3678a1146ad16f7e8649e94fb4fc21fe77e8310c060f61caaff8a";
const NO_EVIDENCE: &str = "{\"attempt\":null,\"generation\":null,\"result_digest\":null}";
const ATTEMPT: &str = "{\"attempt\":1,\"generation\":1,\"result_digest\":null}";
const SUCCESS: &str = "{\"attempt\":1,\"generation\":1,\"result_digest\":\"sha256:44136fa355b3678a1146ad16f7e8649e94fb4fc21fe77e8310c060f61caaff8a\"}";

#[derive(Debug, PartialEq, Eq)]
struct Recorded {
    kind: serea_storage::JournalKind,
    state_from: Option<String>,
    state_to: Option<String>,
    reason: Option<ReasonCode>,
    payload_json: Vec<u8>,
}
impl Recorded {
    fn new(
        kind: serea_storage::JournalKind,
        from: Option<&str>,
        to: &str,
        reason: Option<&str>,
        payload: &str,
    ) -> Self {
        Self {
            kind,
            state_from: from.map(str::to_owned),
            state_to: Some(to.to_owned()),
            reason: reason.map(|s| ReasonCode::new(s).unwrap()),
            payload_json: payload.as_bytes().to_vec(),
        }
    }
    fn capture(row: &serea_storage::JournalRecord) -> Self {
        Self {
            kind: row.kind,
            state_from: row.state_from.clone(),
            state_to: row.state_to.clone(),
            reason: row.reason.clone(),
            payload_json: row.payload_json.clone(),
        }
    }
}
#[derive(Debug, PartialEq, Eq)]
struct Batch {
    operation: serea_storage::AuditOperation,
    task_from: Option<TaskState>,
    task_to: TaskState,
    step_id: Option<StepId>,
    attempt: Option<u32>,
    generation: Option<u32>,
    result: Option<Digest>,
    revision: Option<u32>,
    receipt_id: Option<ReceiptId>,
    now: EpochMillis,
    records: Vec<Recorded>,
}
struct RealAuditProbe {
    batches: Mutex<Vec<Batch>>,
}
impl RealAuditProbe {
    fn new() -> Self {
        Self {
            batches: Mutex::new(Vec::new()),
        }
    }
}
impl serea_storage::TaskAuditParticipant for RealAuditProbe {
    fn records(
        &self,
        f: &serea_storage::DurableTransition,
    ) -> Result<serea_storage::JournalRecords, StoreError> {
        assert_eq!(f.task_id(), &tid());
        assert_eq!(f.actor_kind(), ActorKind::Host);
        assert_eq!(f.actor_id().as_str(), "review-host");
        assert_eq!(f.actor_version().as_str(), "0.2.0");
        assert_eq!(
            f.causation_id().unwrap().as_str(),
            "evt_00000000000000000000000001"
        );
        assert_eq!(f.data_class(), DataClass::Personal);
        let records = serea_storage::TaskAuditParticipant::records(&TaskJournal, f)?;
        self.batches.lock().unwrap().push(Batch {
            operation: f.operation(),
            task_from: f.task_from(),
            task_to: f.task_to(),
            step_id: f.step_id().cloned(),
            attempt: f.attempt(),
            generation: f.generation(),
            result: f.result().cloned(),
            revision: f.revision(),
            receipt_id: f.receipt_id().cloned(),
            now: f.now(),
            records: records.iter().map(Recorded::capture).collect(),
        });
        Ok(records)
    }
}
fn task_batch(
    operation: serea_storage::AuditOperation,
    from: Option<TaskState>,
    to: TaskState,
    now: i64,
    revision: Option<u32>,
    records: Vec<Recorded>,
) -> Batch {
    Batch {
        operation,
        task_from: from,
        task_to: to,
        step_id: None,
        attempt: None,
        generation: None,
        result: None,
        revision,
        receipt_id: None,
        now: at(now),
        records,
    }
}
fn step_batch(
    operation: serea_storage::AuditOperation,
    states: (TaskState, TaskState),
    n: u32,
    now: i64,
    records: Vec<Recorded>,
) -> Batch {
    Batch {
        operation,
        task_from: Some(states.0),
        task_to: states.1,
        step_id: Some(sid(n)),
        attempt: Some(1),
        generation: Some(1),
        result: None,
        revision: None,
        receipt_id: None,
        now: at(now),
        records,
    }
}
fn bootstrap_batches() -> Vec<Batch> {
    use TaskState::*;
    use serea_storage::{AuditOperation as O, JournalKind as K};
    vec![
        task_batch(
            O::TaskInserted,
            None,
            Received,
            10,
            None,
            vec![Recorded::new(
                K::TaskInserted,
                None,
                "RECEIVED",
                None,
                NO_EVIDENCE,
            )],
        ),
        task_batch(
            O::PlanningStarted,
            Some(Received),
            Planning,
            20,
            Some(0),
            vec![Recorded::new(
                K::TaskStateChanged,
                Some("RECEIVED"),
                "PLANNING",
                Some("START_PLANNING"),
                NO_EVIDENCE,
            )],
        ),
        task_batch(
            O::PlanPersisted,
            Some(Planning),
            Ready,
            30,
            Some(1),
            vec![
                Recorded::new(
                    K::PlanPersisted,
                    Some("PLANNING"),
                    "READY",
                    Some("PLAN_PERSISTED"),
                    "{\"revision\":1}",
                ),
                Recorded::new(
                    K::TaskStateChanged,
                    Some("PLANNING"),
                    "READY",
                    Some("PLAN_PERSISTED"),
                    "{\"revision\":1}",
                ),
            ],
        ),
        step_batch(
            O::LeaseAcquired,
            (Ready, Ready),
            1,
            40,
            vec![Recorded::new(
                K::StepLeaseAcquired,
                Some("PLANNED"),
                "LEASED",
                None,
                ATTEMPT,
            )],
        ),
        step_batch(
            O::AttemptStarted,
            (Ready, Executing),
            1,
            41,
            vec![
                Recorded::new(
                    K::StepAttemptStarted,
                    Some("LEASED"),
                    "EXECUTING",
                    None,
                    ATTEMPT,
                ),
                Recorded::new(
                    K::TaskStateChanged,
                    Some("READY"),
                    "EXECUTING",
                    None,
                    ATTEMPT,
                ),
            ],
        ),
    ]
}
fn audited_ready(
    tx: &mut serea_storage::Tx<'_>,
    created: &AssistantTask,
    c: &Context,
    steps: Vec<PlanStep>,
) -> Result<LeaseGuard, StoreError> {
    tx.insert_task(created, &c.view())?;
    tx.start_planning(&tid(), TaskState::Received, 0, at(20), &c.view())?;
    tx.put_plan_revision(
        &tid(),
        serea_storage::PlanWrite {
            revision: 1,
            steps: steps
                .into_iter()
                .map(|p| serea_storage::StepInput {
                    step: p.step,
                    input_json: p.input_json,
                })
                .collect(),
        },
        at(30),
        &c.view(),
    )?;
    let g = tx.acquire_audited(
        tid(),
        sid(1),
        LeaseOwner::new("review-worker").unwrap(),
        None,
        at(40),
        at(900),
        &c.view(),
    )?;
    tx.begin_attempt(&g, at(41), &c.view())?;
    Ok(g)
}
fn created(c: &Context) -> AssistantTask {
    engine().create_task(spec(), &c.view()).unwrap().task
}
fn assert_batches_and_durable_count(
    store: &Store,
    probe: &RealAuditProbe,
    expected: Vec<Batch>,
    journal_count: u64,
) {
    let batches = probe.batches.lock().unwrap();
    assert_eq!(
        batches.len(),
        expected.len(),
        "unexpected mapper invocation count"
    );
    for (index, (actual, expected)) in batches.iter().zip(&expected).enumerate() {
        assert_eq!(actual, expected, "journal batch {index}");
    }
    assert_eq!(
        store
            .transact(|tx| tx.delete_task(&tid()))
            .unwrap()
            .journal_rows,
        journal_count
    );
}

#[test]
fn t1_real_journal_receipt_success_has_complete_literal_batches_and_bound_facts() {
    use TaskState::*;
    use serea_storage::{AuditOperation as O, JournalKind as K};
    let c = Context::new();
    let store = Store::open_in_memory(&Fixed).unwrap();
    let probe = RealAuditProbe::new();
    let p = input(1, StepKind::Capability);
    let r = receipt(&p.step);
    let created = created(&c);
    store
        .transact_with_audit(&probe, |tx| {
            let g = audited_ready(tx, &created, &c, vec![p])?;
            tx.commit_step_outcome(
                g,
                StepOutcome::Succeeded {
                    result_json: b"{}",
                    receipt: Some(&r),
                },
                at(42),
                &c.view(),
            )?;
            Ok(())
        })
        .unwrap();
    let mut expected = bootstrap_batches();
    let mut outcome = step_batch(
        O::StepSucceeded,
        (Executing, Verifying),
        1,
        42,
        vec![
            Recorded::new(
                K::StepCommitted,
                Some("EXECUTING"),
                "SUCCEEDED",
                None,
                SUCCESS,
            ),
            Recorded::new(
                K::ReceiptRecorded,
                Some("EXECUTING"),
                "SUCCEEDED",
                None,
                SUCCESS,
            ),
            Recorded::new(
                K::TaskStateChanged,
                Some("EXECUTING"),
                "VERIFYING",
                None,
                SUCCESS,
            ),
        ],
    );
    outcome.result = Some(Digest::new(EMPTY_DIGEST).unwrap());
    outcome.receipt_id = Some(ReceiptId::new("rcp_00000000000000000000000001").unwrap());
    expected.push(outcome);
    let loaded = store.load_task(&tid()).unwrap();
    assert_eq!(loaded.task.state, Verifying);
    assert_eq!(loaded.steps[0].step.side_effect_receipt, Some(r));
    assert_batches_and_durable_count(&store, &probe, expected, 10);
}

#[test]
fn t1_real_journal_known_final_failure_preserves_specific_cause_and_terminal_batch() {
    use TaskState::*;
    use serea_storage::{AuditOperation as O, JournalKind as K};
    let c = Context::new();
    let store = Store::open_in_memory(&Fixed).unwrap();
    let probe = RealAuditProbe::new();
    let created = created(&c);
    let code = ErrorCode::new("KNOWN_FAILURE").unwrap();
    let message = ErrorMessage::new("not journal payload").unwrap();
    let action = HostAction::new("STOP").unwrap();
    let reason = FailureReason::new("KNOWN_FAILURE").unwrap();
    store
        .transact_with_audit(&probe, |tx| {
            let g = audited_ready(tx, &created, &c, vec![input(1, StepKind::Notify)])?;
            tx.commit_step_outcome(
                g,
                StepOutcome::Failed(StepFailure {
                    kind: ActionErrorKind::ProviderError,
                    code: &code,
                    message: &message,
                    retryable: false,
                    host_action: &action,
                    details_json: Some(b"{\"diagnostic\":true}"),
                    failure_reason: &reason,
                }),
                at(42),
                &c.view(),
            )?;
            Ok(())
        })
        .unwrap();
    let mut expected = bootstrap_batches();
    expected.push(step_batch(
        O::StepFailed,
        (Executing, Failed),
        1,
        42,
        vec![
            Recorded::new(
                K::StepFailed,
                Some("EXECUTING"),
                "FAILED",
                Some("KNOWN_FAILURE"),
                ATTEMPT,
            ),
            Recorded::new(
                K::TaskStateChanged,
                Some("EXECUTING"),
                "FAILED",
                Some("KNOWN_FAILURE"),
                ATTEMPT,
            ),
            Recorded::new(
                K::TaskTerminal,
                Some("EXECUTING"),
                "FAILED",
                Some("KNOWN_FAILURE"),
                ATTEMPT,
            ),
        ],
    ));
    assert_eq!(
        store.load_task(&tid()).unwrap().task.failure_reason,
        Some(reason)
    );
    assert_batches_and_durable_count(&store, &probe, expected, 10);
}

#[test]
fn t1_real_journal_verification_terminal_has_no_receipt_or_self_transition_duplicates() {
    use TaskState::*;
    use serea_storage::{AuditOperation as O, JournalKind as K};
    let c = Context::new();
    let store = Store::open_in_memory(&Fixed).unwrap();
    let probe = RealAuditProbe::new();
    let created = created(&c);
    store
        .transact_with_audit(&probe, |tx| {
            let g = audited_ready(
                tx,
                &created,
                &c,
                vec![input(1, StepKind::Notify), input(2, StepKind::Verify)],
            )?;
            tx.commit_step_outcome(g, result(), at(42), &c.view())?;
            let v = tx.acquire_audited(
                tid(),
                sid(2),
                LeaseOwner::new("review-verifier").unwrap(),
                None,
                at(43),
                at(900),
                &c.view(),
            )?;
            tx.begin_attempt(&v, at(44), &c.view())?;
            tx.commit_step_outcome(v, result(), at(45), &c.view())?;
            assert!(
                !tx.cancel_task(
                    &tid(),
                    TaskOriginKind::new("HOST").unwrap(),
                    at(46),
                    &c.view()
                )?
                .changed
            );
            Ok(())
        })
        .unwrap();
    let mut expected = bootstrap_batches();
    let mut ordinary = step_batch(
        O::StepSucceeded,
        (Executing, Verifying),
        1,
        42,
        vec![
            Recorded::new(
                K::StepCommitted,
                Some("EXECUTING"),
                "SUCCEEDED",
                None,
                SUCCESS,
            ),
            Recorded::new(
                K::TaskStateChanged,
                Some("EXECUTING"),
                "VERIFYING",
                None,
                SUCCESS,
            ),
        ],
    );
    ordinary.result = Some(Digest::new(EMPTY_DIGEST).unwrap());
    expected.push(ordinary);
    expected.push(step_batch(
        O::LeaseAcquired,
        (Verifying, Verifying),
        2,
        43,
        vec![Recorded::new(
            K::StepLeaseAcquired,
            Some("PLANNED"),
            "LEASED",
            None,
            ATTEMPT,
        )],
    ));
    expected.push(step_batch(
        O::AttemptStarted,
        (Verifying, Verifying),
        2,
        44,
        vec![Recorded::new(
            K::StepAttemptStarted,
            Some("LEASED"),
            "EXECUTING",
            None,
            ATTEMPT,
        )],
    ));
    let mut terminal = step_batch(
        O::StepSucceeded,
        (Verifying, Completed),
        2,
        45,
        vec![
            Recorded::new(
                K::StepCommitted,
                Some("EXECUTING"),
                "SUCCEEDED",
                None,
                SUCCESS,
            ),
            Recorded::new(
                K::TaskStateChanged,
                Some("VERIFYING"),
                "COMPLETED",
                None,
                SUCCESS,
            ),
            Recorded::new(
                K::TaskTerminal,
                Some("VERIFYING"),
                "COMPLETED",
                None,
                SUCCESS,
            ),
        ],
    );
    terminal.result = Some(Digest::new(EMPTY_DIGEST).unwrap());
    expected.push(terminal);
    assert_eq!(store.load_task(&tid()).unwrap().task.state, Completed);
    assert_batches_and_durable_count(&store, &probe, expected, 14);
}
