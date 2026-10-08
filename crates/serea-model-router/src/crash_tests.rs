//! Process-death proof for durable model dispatch and completion boundaries.
//!
//! The parent kills a re-invoked unit-test process only after a file
//! acknowledgement from the named boundary. A fresh Store then verifies the
//! durable state; no in-process state can satisfy these assertions.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::task::{Context, Poll, Waker};
use std::time::Duration;

use async_trait::async_trait;
use serea_event_bus::{EventBus, ReplayItem};
use serea_protocol::provider::{ModelCallContext, ModelProvider};
use serea_protocol::{
    Clock, CostClass, DataClass, EpochMillis, EventKind, FinishReason, JsonSchemaMode,
    ModelCapabilities, ModelDescriptor, ModelError, ModelId, ModelMessage, ModelPurpose,
    ModelRequest, ModelResponse, ModelUsage, ProtocolError, ProviderHealth, ProviderId,
    ResponseFormat, TimestampMs, TokenCount, UlidSource, UlidValue,
};
use serea_storage::fault::{Action, Window};
use serea_storage::{
    ModelAttemptRelationKind, ModelAttemptState, ModelPriceSnapshot, Store, StoreError, UsdMicros,
};

use crate::{
    ModelDispatchContext, ModelDispatchFailure, ModelDispatchGateSnapshotV1,
    ModelDispatchGateSource, ModelDispatchGateSourceError, ModelEgressPolicySnapshotV1,
    ModelRosterEntryV1, ModelRosterV1, ModelRouterV1, ModelRoutingRequirementsV1,
    PreparedModelCallDraftV1, PreparedModelCallV1, StructuredRequirementV1,
    recover_completed_chat_text_response, recover_unresolved_model_calls,
};

const ROLE: &str = "SEREA_P4D_CRASH_ROLE";
const MODE: &str = "SEREA_P4D_CRASH_MODE";
const DATABASE: &str = "SEREA_P4D_CRASH_DATABASE";
const ACK: &str = "SEREA_P4D_CRASH_ACK";
const CALLED: &str = "SEREA_P4D_CRASH_CALLED";
const CALL_COUNT: &str = "SEREA_P4D_CRASH_CALL_COUNT";
const CHILD_TEST: &str = "crash_tests::model_dispatch_crash_child";
const POLLS: usize = 4_000;

struct OpenGate;

impl ModelDispatchGateSource for OpenGate {
    fn snapshot(
        &self,
        _task_id: Option<&serea_protocol::TaskId>,
        _data_class: DataClass,
    ) -> Result<ModelDispatchGateSnapshotV1, ModelDispatchGateSourceError> {
        Ok(ModelDispatchGateSnapshotV1::from_host(
            false,
            ModelEgressPolicySnapshotV1::from_host(true),
            1000,
        ))
    }
}

static OPEN_GATE: OpenGate = OpenGate;
const TICK: Duration = Duration::from_millis(5);

static NEXT: AtomicU64 = AtomicU64::new(0);

struct FixedClock;

impl Clock for FixedClock {
    fn now_ms(&self) -> Result<EpochMillis, ProtocolError> {
        EpochMillis::new(1_767_225_600_000)
    }
}

struct IncrementingIds(u64);

impl UlidSource for IncrementingIds {
    fn next_ulid(&mut self) -> UlidValue {
        self.0 += 1;
        let timestamp = TimestampMs::new(self.0).unwrap_or_else(|_| unreachable!());
        UlidValue::new(timestamp, [self.0 as u8; 10])
    }
}

struct CrashProvider {
    ack: PathBuf,
    called: PathBuf,
    calls_file: PathBuf,
    calls: AtomicUsize,
    mode: String,
    block_in_generate: bool,
}

#[async_trait]
impl ModelProvider for CrashProvider {
    fn provider_id(&self) -> ProviderId {
        ProviderId::new("provider").unwrap_or_else(|_| unreachable!())
    }

    fn models(&self) -> Vec<ModelDescriptor> {
        ["nemotron-3-nano-30b", "gpt-oss-20b"]
            .into_iter()
            .map(|model_id| ModelDescriptor {
                model_id: ModelId::new(model_id).unwrap_or_else(|_| unreachable!()),
                provider_id: self.provider_id(),
                capabilities: ModelCapabilities {
                    vision: false,
                    tools: false,
                    structured_output: true,
                    max_context_tokens: 1_000,
                    max_output_tokens: 1_000,
                    json_schema_mode: JsonSchemaMode::Strict,
                    thinking: false,
                    long_context: false,
                    fast: false,
                    code_specialist: false,
                    supports_streaming: false,
                    supports_seeds: false,
                },
            })
            .collect()
    }

    async fn generate(
        &self,
        request: &ModelRequest,
        _ctx: &ModelCallContext,
    ) -> Result<ModelResponse, ModelError> {
        let call_number = self.calls.fetch_add(1, Ordering::SeqCst) + 1;
        fs::write(&self.called, request.request_id.as_str()).unwrap_or_else(|_| unreachable!());
        fs::write(&self.calls_file, call_number.to_string()).unwrap_or_else(|_| unreachable!());
        if self.block_in_generate {
            announce_and_park(&self.ack, "provider-generate");
        }
        if matches!(self.mode.as_str(), "fallback" | "fallback-before-decision") && call_number == 1
        {
            return Err(ModelError {
                kind: serea_protocol::ModelErrorCode::new("UPSTREAM_UNAVAILABLE")
                    .unwrap_or_else(|_| unreachable!()),
                message: serea_protocol::ErrorMessage::new("retryable crash-test failure")
                    .unwrap_or_else(|_| unreachable!()),
                retryable: true,
            });
        }
        let content = if matches!(self.mode.as_str(), "repair" | "repair2") {
            "{\"count\":\"invalid\"}".to_owned()
        } else {
            "accepted text".to_owned()
        };
        Ok(ModelResponse {
            request_id: request.request_id.clone(),
            model_id: request.model_id.clone(),
            provider_id: self.provider_id(),
            content,
            structured: None,
            finish_reason: FinishReason::Stop,
            usage: ModelUsage {
                input_tokens: TokenCount::new(3),
                output_tokens: TokenCount::new(2),
                cost_class: CostClass::Paid,
            },
            latency_ms: 7,
            repair_attempts: 0,
        })
    }

    async fn health(&self) -> ProviderHealth {
        ProviderHealth::Ready
    }
}

fn call() -> PreparedModelCallV1 {
    PreparedModelCallV1::from_host(PreparedModelCallDraftV1 {
        task_id: None,
        purpose: ModelPurpose::Chat,
        messages: vec![ModelMessage {
            role: serea_protocol::MessageRole::new("user").unwrap_or_else(|_| unreachable!()),
            content: "crash-test prompt".into(),
        }],
        system: None,
        response_format: ResponseFormat::Text,
        tools: Vec::new(),
        max_output_tokens: 128,
        temperature: 0.0,
        deadline_ms: 1_000,
        data_class: DataClass::Public,
        requirements: ModelRoutingRequirementsV1 {
            vision_required: false,
            tools_required: false,
            min_context_tokens: 1,
            min_output_tokens: 1,
            structured_requirement: StructuredRequirementV1::Any,
        },
        egress: ModelEgressPolicySnapshotV1::from_host(false),
        host_max_output_tokens: 2_048,
    })
    .unwrap_or_else(|_| unreachable!())
}

fn structured_call() -> PreparedModelCallV1 {
    PreparedModelCallV1::from_host(PreparedModelCallDraftV1 {
        task_id: None,
        purpose: ModelPurpose::Planning,
        messages: vec![ModelMessage {
            role: serea_protocol::MessageRole::new("user").unwrap_or_else(|_| unreachable!()),
            content: "structured crash-test prompt".into(),
        }],
        system: None,
        response_format: ResponseFormat::JsonSchema {
            schema: serde_json::json!({
                "type": "object",
                "properties": {"count": {"type": "integer"}},
                "required": ["count"],
                "additionalProperties": false
            }),
        },
        tools: Vec::new(),
        max_output_tokens: 128,
        temperature: 0.0,
        deadline_ms: 1_000,
        data_class: DataClass::Public,
        requirements: ModelRoutingRequirementsV1 {
            vision_required: false,
            tools_required: false,
            min_context_tokens: 1,
            min_output_tokens: 1,
            structured_requirement: StructuredRequirementV1::Strict,
        },
        egress: ModelEgressPolicySnapshotV1::from_host(false),
        host_max_output_tokens: 2_048,
    })
    .unwrap_or_else(|_| unreachable!())
}

fn router(provider: Arc<CrashProvider>) -> ModelRouterV1 {
    let descriptors = provider.models();
    let roster = ModelRosterV1::new(
        descriptors
            .into_iter()
            .map(|descriptor| {
                ModelRosterEntryV1::new(
                    descriptor.model_id,
                    provider.provider_id(),
                    crate::ModelDeploymentClass::Local,
                    true,
                    descriptor.capabilities,
                    CostClass::Paid,
                )
                .unwrap_or_else(|_| unreachable!())
            })
            .collect(),
    )
    .unwrap_or_else(|_| unreachable!());
    ModelRouterV1::new(roster, vec![provider]).unwrap_or_else(|_| unreachable!())
}

fn announce_and_park(path: &Path, stage: &str) -> ! {
    fs::write(path, stage).unwrap_or_else(|_| std::process::abort());
    loop {
        std::thread::park();
    }
}

fn block_on<F: std::future::Future>(future: F) -> F::Output {
    let mut context = Context::from_waker(Waker::noop());
    let mut future = std::pin::pin!(future);
    match std::future::Future::poll(future.as_mut(), &mut context) {
        Poll::Ready(value) => value,
        Poll::Pending => unreachable!("crash-test provider futures are immediately ready"),
    }
}

/// Child entry point re-invoked by the parent coordinator below.
#[test]
fn model_dispatch_crash_child() {
    if std::env::var(ROLE).as_deref() != Ok("child") {
        return;
    }
    let mode = std::env::var(MODE).unwrap_or_else(|_| unreachable!());
    let db_path = PathBuf::from(std::env::var(DATABASE).unwrap_or_else(|_| unreachable!()));
    let ack = PathBuf::from(std::env::var(ACK).unwrap_or_else(|_| unreachable!()));
    let called = PathBuf::from(std::env::var(CALLED).unwrap_or_else(|_| unreachable!()));
    let calls_file = PathBuf::from(std::env::var(CALL_COUNT).unwrap_or_else(|_| unreachable!()));
    let store = Store::open(&db_path, &FixedClock).unwrap_or_else(|_| unreachable!());
    let bus = EventBus::new(IncrementingIds(10));
    let provider = Arc::new(CrashProvider {
        ack: ack.clone(),
        called,
        calls_file,
        calls: AtomicUsize::new(0),
        mode: mode.clone(),
        block_in_generate: mode == "provider",
    });
    let router = router(provider);
    let prepared = if matches!(mode.as_str(), "repair" | "repair2") {
        structured_call()
    } else {
        call()
    };
    let session = block_on(router.route(&prepared)).unwrap_or_else(|_| unreachable!());
    let context = ModelDispatchContext {
        store: &store,
        events: &bus,
        price: ModelPriceSnapshot::new(CostClass::Paid, "price-1", 1_000_000, 1_000_000),
        max_daily_spend_usd_micros: UsdMicros::new(10_000_000).unwrap_or_else(|_| unreachable!()),
        clock: &FixedClock,
        gate: &OPEN_GATE,
    };
    match mode.as_str() {
        "intent" => Window::AfterCommit
            .arm(Action::Crash { ack })
            .unwrap_or_else(|_| unreachable!()),
        "response" => Window::AfterModelResponseBlob
            .arm(Action::Crash { ack })
            .unwrap_or_else(|_| unreachable!()),
        "complete" => Window::AfterCommit
            .arm_after(1, Action::Crash { ack })
            .unwrap_or_else(|_| unreachable!()),
        "provider" => {}
        "fallback-before-decision" => Window::BeforeCommit
            .arm_after(1, Action::Crash { ack })
            .unwrap_or_else(|_| unreachable!()),
        "fallback" => Window::AfterCommit
            .arm_after(1, Action::Crash { ack })
            .unwrap_or_else(|_| unreachable!()),
        "repair" => Window::AfterCommit
            .arm_after(2, Action::Crash { ack })
            .unwrap_or_else(|_| unreachable!()),
        "repair2" => Window::AfterCommit
            .arm_after(4, Action::Crash { ack })
            .unwrap_or_else(|_| unreachable!()),
        _ => unreachable!(),
    }
    let result = match mode.as_str() {
        "fallback" | "fallback-before-decision" => {
            block_on(router.dispatch_chat_text_with_fallback(&prepared, &session, &context))
        }
        "repair" | "repair2" => {
            block_on(router.dispatch_structured_with_repair(&prepared, &session, &context))
        }
        _ => block_on(router.dispatch_chat_text(&prepared, &session, &context)),
    };
    assert!(matches!(result, Err(ModelDispatchFailure::Storage(_))));
    unreachable!("dispatch returned after a process-crash window")
}

fn wait_for(path: &Path) -> bool {
    for _ in 0..POLLS {
        if path.exists() {
            return true;
        }
        std::thread::sleep(TICK);
    }
    path.exists()
}

fn kill_and_prove_death(mut child: Child, mode: &str) {
    child
        .kill()
        .unwrap_or_else(|_| unreachable!("{mode}: child must be killable"));
    let output = child
        .wait_with_output()
        .unwrap_or_else(|_| unreachable!("{mode}: child wait failed"));
    assert!(
        !output.status.success(),
        "{mode}: crash child exited successfully"
    );
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        assert_eq!(
            output.status.signal(),
            Some(9),
            "{mode}: child was not SIGKILLed"
        );
    }
}

fn run_crash_child(db: &Path, mode: &str) -> (PathBuf, PathBuf, PathBuf) {
    let dir = db.parent().unwrap_or_else(|| unreachable!());
    let ack = dir.join(format!("ack-{mode}"));
    let called = dir.join(format!("called-{mode}"));
    let calls_file = dir.join(format!("calls-{mode}"));
    let mut child = Command::new(std::env::current_exe().unwrap_or_else(|_| unreachable!()))
        .args(["--exact", CHILD_TEST, "--nocapture", "--test-threads=1"])
        .env(ROLE, "child")
        .env(MODE, mode)
        .env(DATABASE, db)
        .env(ACK, &ack)
        .env(CALLED, &called)
        .env(CALL_COUNT, &calls_file)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap_or_else(|_| unreachable!("{mode}: child spawn failed"));
    if !wait_for(&ack) {
        let _ = child.kill();
        let _ = child.wait();
        assert!(
            ack.exists(),
            "{mode}: child missed the crash acknowledgement"
        );
    }
    kill_and_prove_death(child, mode);
    (ack, called, calls_file)
}

#[test]
fn intent_transaction_failure_rolls_back_event_and_never_calls_provider() {
    let id = NEXT.fetch_add(1, Ordering::SeqCst);
    let dir = std::env::temp_dir().join(format!("serea-p4d-rollback-{}-{id}", std::process::id()));
    fs::create_dir(&dir).unwrap_or_else(|_| unreachable!());
    let db = dir.join("store.sqlite");
    let store = Store::open(&db, &FixedClock).unwrap_or_else(|_| unreachable!());
    let bus = EventBus::new(IncrementingIds(10));
    let called = dir.join("called");
    let provider = Arc::new(CrashProvider {
        ack: dir.join("unused-ack"),
        called: called.clone(),
        calls_file: dir.join("unused-calls"),
        calls: AtomicUsize::new(0),
        mode: "rollback".into(),
        block_in_generate: false,
    });
    let router = router(provider);
    let prepared = call();
    let session = block_on(router.route(&prepared)).unwrap_or_else(|_| unreachable!());
    let context = ModelDispatchContext {
        store: &store,
        events: &bus,
        price: ModelPriceSnapshot::new(CostClass::Paid, "price-1", 1_000_000, 1_000_000),
        max_daily_spend_usd_micros: UsdMicros::new(10_000_000).unwrap_or_else(|_| unreachable!()),
        clock: &FixedClock,
        gate: &OPEN_GATE,
    };
    Window::AfterModelAttemptInsert
        .arm(Action::Fail(StoreError::Sqlite))
        .unwrap_or_else(|_| unreachable!());
    let result = block_on(router.dispatch_chat_text(&prepared, &session, &context));
    assert!(matches!(
        result,
        Err(ModelDispatchFailure::Storage(StoreError::Sqlite))
    ));
    assert!(!called.exists());
    assert!(
        store
            .list_unfinished_model_call_attempts()
            .unwrap_or_else(|_| unreachable!())
            .is_empty()
    );
    assert!(
        EventBus::replay(&store, None, None, 16)
            .unwrap_or_else(|_| unreachable!())
            .items
            .is_empty()
    );
    drop(store);
    fs::remove_dir_all(dir).unwrap_or_else(|_| unreachable!());
}

#[test]
fn durable_dispatch_windows_survive_real_process_death() {
    let id = NEXT.fetch_add(1, Ordering::SeqCst);
    let dir = std::env::temp_dir().join(format!("serea-p4d-crash-{}-{id}", std::process::id()));
    fs::create_dir(&dir).unwrap_or_else(|_| unreachable!());

    for mode in ["intent", "provider", "response"] {
        let db = dir.join(format!("{mode}.sqlite"));
        let _ = Store::open(&db, &FixedClock).unwrap_or_else(|_| unreachable!());
        let (ack, called, _) = run_crash_child(&db, mode);
        assert!(ack.exists());
        assert_eq!(called.exists(), mode != "intent");

        let store = Store::open(&db, &FixedClock).unwrap_or_else(|_| unreachable!());
        let unresolved = store
            .list_unfinished_model_call_attempts()
            .unwrap_or_else(|_| unreachable!());
        assert_eq!(unresolved.len(), 1, "{mode}: durable intent missing");
        let request_id = unresolved[0].request_id.clone();
        let reserved = unresolved[0].reserved_cost_usd_micros;
        if mode == "intent" {
            assert_eq!(unresolved[0].state, ModelAttemptState::DispatchIntent);
        }
        if mode == "response" {
            assert_eq!(unresolved[0].response_blob, None);
        }
        let recovery_bus = EventBus::new(IncrementingIds(50));
        assert_eq!(
            recover_unresolved_model_calls(&store, &recovery_bus, &FixedClock)
                .unwrap_or_else(|_| unreachable!()),
            1
        );
        let recovered = store
            .get_model_call_attempt(&request_id)
            .unwrap_or_else(|_| unreachable!())
            .unwrap_or_else(|| unreachable!());
        assert_eq!(recovered.state, ModelAttemptState::Ambiguous);
        assert_eq!(recovered.reserved_cost_usd_micros, reserved);
        assert_eq!(recovered.actual_cost_usd_micros, None);
        assert!(
            store
                .model_usage_for_request(&request_id)
                .unwrap_or_else(|_| unreachable!())
                .is_none()
        );
        let events = EventBus::replay(&store, None, None, 16).unwrap_or_else(|_| unreachable!());
        assert!(
            matches!(events.items.as_slice(), [ReplayItem::Event { event: called }, ReplayItem::Event { event: failed }] if called.kind == EventKind::ModelCalled && failed.kind == EventKind::ModelFailed)
        );
        assert_eq!(
            recover_unresolved_model_calls(&store, &recovery_bus, &FixedClock)
                .unwrap_or_else(|_| unreachable!()),
            0
        );
        drop(store);
    }

    // A provider response whose terminal transaction commits is recoverable
    // even when the process dies before the caller sees it.
    let db = dir.join("complete.sqlite");
    let _ = Store::open(&db, &FixedClock).unwrap_or_else(|_| unreachable!());
    let (ack, called, _) = run_crash_child(&db, "complete");
    assert!(ack.exists() && called.exists());
    let store = Store::open(&db, &FixedClock).unwrap_or_else(|_| unreachable!());
    let request_id = serea_protocol::RequestId::new(
        fs::read_to_string(called).unwrap_or_else(|_| unreachable!()),
    )
    .unwrap_or_else(|_| unreachable!());
    let attempt = store
        .get_model_call_attempt(&request_id)
        .unwrap_or_else(|_| unreachable!())
        .unwrap_or_else(|| unreachable!());
    assert_eq!(attempt.state, ModelAttemptState::Completed);
    let response = recover_completed_chat_text_response(&store, &request_id)
        .unwrap_or_else(|_| unreachable!())
        .unwrap_or_else(|| unreachable!());
    assert_eq!(response.content, "accepted text");
    assert_eq!(response.request_id, request_id);
    assert!(
        store
            .model_usage_for_request(&response.request_id)
            .unwrap_or_else(|_| unreachable!())
            .is_some()
    );
    let events = EventBus::replay(&store, None, None, 16).unwrap_or_else(|_| unreachable!());
    assert!(
        matches!(events.items.as_slice(), [ReplayItem::Event { event: called }, ReplayItem::Event { event: completed }] if called.kind == EventKind::ModelCalled && completed.kind == EventKind::ModelCompleted)
    );
    drop(store);
    fs::remove_dir_all(dir).unwrap_or_else(|_| unreachable!());
}

#[test]
fn fallback_and_repair_child_intents_recover_ambiguous_without_redispatch() {
    let id = NEXT.fetch_add(1, Ordering::SeqCst);
    let dir = std::env::temp_dir().join(format!("serea-p4e-crash-{}-{id}", std::process::id()));
    fs::create_dir(&dir).unwrap_or_else(|_| unreachable!());

    for mode in ["fallback-before-decision", "fallback", "repair", "repair2"] {
        let db = dir.join(format!("{mode}.sqlite"));
        let _ = Store::open(&db, &FixedClock).unwrap_or_else(|_| unreachable!());
        let (ack, called, calls_file) = run_crash_child(&db, mode);
        assert!(
            ack.exists(),
            "{mode}: expected fault-window acknowledgement"
        );
        assert!(
            called.exists(),
            "{mode}: initial provider call should have occurred"
        );
        let expected_calls = if mode == "repair2" { 2 } else { 1 };
        let actual_calls = fs::read_to_string(&calls_file)
            .unwrap_or_else(|_| unreachable!("{mode}: provider call count missing"));
        assert_eq!(
            actual_calls,
            expected_calls.to_string(),
            "{mode}: no provider call may occur after the committed child intent"
        );

        let store = Store::open(&db, &FixedClock).unwrap_or_else(|_| unreachable!());
        let attempts = store
            .list_unfinished_model_call_attempts()
            .unwrap_or_else(|_| unreachable!());
        let child = if mode == "fallback-before-decision" {
            assert_eq!(
                attempts.len(),
                1,
                "{mode}: rollback must leave no fallback child"
            );
            assert_eq!(attempts[0].relation_kind, ModelAttemptRelationKind::None);
            &attempts[0]
        } else {
            attempts
                .iter()
                .find(|attempt| attempt.relation_kind != ModelAttemptRelationKind::None)
                .unwrap_or_else(|| unreachable!("{mode}: committed retry child missing"))
        };
        assert_eq!(child.state, ModelAttemptState::DispatchIntent, "{mode}");
        let child_id = child.request_id.clone();
        let relation = child.relation_kind;
        let parent_id = child.parent_request_id.clone();
        let expected_relation = match mode {
            "fallback-before-decision" => ModelAttemptRelationKind::None,
            "fallback" => ModelAttemptRelationKind::Fallback,
            _ => ModelAttemptRelationKind::Repair,
        };
        assert_eq!(relation, expected_relation);
        if mode != "fallback-before-decision" {
            assert!(parent_id.is_some(), "{mode}: retry lineage must be durable");
        }

        let before_recovery =
            EventBus::replay(&store, None, None, 32).unwrap_or_else(|_| unreachable!());
        let before_kinds = before_recovery
            .items
            .iter()
            .filter_map(|item| match item {
                ReplayItem::Event { event } => Some(event.kind),
                _ => None,
            })
            .collect::<Vec<_>>();
        let expected_before = match mode {
            "fallback-before-decision" => vec![EventKind::ModelCalled],
            "fallback" => vec![
                EventKind::ModelCalled,
                EventKind::ModelFailed,
                EventKind::ModelFallback,
                EventKind::ModelCalled,
            ],
            "repair" => vec![
                EventKind::ModelCalled,
                EventKind::ModelCompleted,
                EventKind::ModelOutputInvalid,
                EventKind::ModelCalled,
            ],
            _ => vec![
                EventKind::ModelCalled,
                EventKind::ModelCompleted,
                EventKind::ModelOutputInvalid,
                EventKind::ModelCalled,
                EventKind::ModelCompleted,
                EventKind::ModelOutputInvalid,
                EventKind::ModelCalled,
            ],
        };
        assert_eq!(
            before_kinds, expected_before,
            "{mode}: pre-recovery event history"
        );
        let before_json =
            serde_json::to_string(&before_recovery.items).unwrap_or_else(|_| unreachable!());
        assert!(
            !before_json.contains("invalid"),
            "{mode}: raw output leaked into events"
        );
        assert!(
            !before_json.contains("crash-test prompt"),
            "{mode}: prompt leaked into events"
        );

        let bus = EventBus::new(IncrementingIds(50));
        assert_eq!(
            recover_unresolved_model_calls(&store, &bus, &FixedClock)
                .unwrap_or_else(|_| unreachable!()),
            1,
            "{mode}: committed child intent should become ambiguous"
        );
        let recovered = store
            .get_model_call_attempt(&child_id)
            .unwrap_or_else(|_| unreachable!())
            .unwrap_or_else(|| unreachable!());
        assert_eq!(recovered.state, ModelAttemptState::Ambiguous, "{mode}");
        assert_eq!(recovered.parent_request_id, parent_id);
        assert_eq!(
            recover_unresolved_model_calls(&store, &bus, &FixedClock)
                .unwrap_or_else(|_| unreachable!()),
            0,
            "{mode}: second recovery must be a no-op"
        );
        drop(store);
    }

    fs::remove_dir_all(dir).unwrap_or_else(|_| unreachable!());
}
