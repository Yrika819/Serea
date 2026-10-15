//! Offline model dispatch reachable only through explicit synthetic commands.

use std::fs::OpenOptions;
use std::future::Future;
use std::path::Path;
use std::pin::pin;
use std::sync::Arc;
use std::task::{Context, Poll, Wake, Waker};

use serea_event_bus::EventBus;
#[cfg(feature = "synthetic-test-hooks")]
use serea_model_router::RouterError;
use serea_model_router::{
    ModelDeploymentClass, ModelDispatchGateSnapshotV1, ModelDispatchGateSource,
    ModelDispatchGateSourceError, ModelEgressPolicySnapshotV1, ModelRosterEntryV1, ModelRosterV1,
    ModelRouterCallFailureV1, ModelRouterHostContextV1, ModelRouterProcessV1,
    ModelRoutingRequirementsV1, PreparedModelCallDraftV1, PreparedModelCallV1,
    StructuredRequirementV1,
};
use serea_protocol::provider::ModelProvider;
use serea_protocol::{
    Clock, CostClass, DataClass, ModelMessage, ModelPurpose, ResponseFormat, TimestampMs,
};
#[cfg(feature = "synthetic-test-hooks")]
use serea_protocol::{ModelRequest, ModelResponse, ProviderHealth, ProviderId};
use serea_storage::{ModelPriceSnapshot, Store, UsdMicros};
use serea_testkit::{MockModelProvider, ModelScript, synthetic_model_roster};

struct OfflineGate {
    cancelled: bool,
    cancel_after_initial: bool,
    deadline_ms: u32,
    snapshots: std::sync::atomic::AtomicUsize,
}

impl ModelDispatchGateSource for OfflineGate {
    fn snapshot(
        &self,
        _task_id: Option<&serea_protocol::TaskId>,
        _data_class: DataClass,
    ) -> Result<ModelDispatchGateSnapshotV1, ModelDispatchGateSourceError> {
        let snapshot = self
            .snapshots
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Ok(ModelDispatchGateSnapshotV1::from_host(
            self.cancelled || (self.cancel_after_initial && snapshot > 0),
            ModelEgressPolicySnapshotV1::from_host(false),
            self.deadline_ms,
        ))
    }
}

fn configured_router(
    failure: bool,
) -> Result<(ModelRouterProcessV1, Arc<MockModelProvider>), String> {
    configured_router_with_barrier(failure, None, None)
}

#[cfg(feature = "synthetic-test-hooks")]
#[derive(Clone)]
struct TestBarrier {
    directory: std::path::PathBuf,
    phase: &'static str,
}

#[cfg(feature = "synthetic-test-hooks")]
impl TestBarrier {
    fn wait(&self) -> Result<(), String> {
        std::fs::create_dir_all(&self.directory).map_err(|_| "barrier directory unavailable")?;
        std::fs::write(
            self.directory.join(format!("{}.reached", self.phase)),
            b"ready",
        )
        .map_err(|_| "barrier marker unavailable")?;
        let release = self.directory.join(format!("{}.release", self.phase));
        for _ in 0..24_000 {
            if release.exists() {
                return Ok(());
            }
            std::thread::park_timeout(std::time::Duration::from_millis(5));
        }
        Err("test barrier deadline expired".to_owned())
    }

    async fn wait_async(&self) -> Result<(), String> {
        use std::sync::{
            Arc,
            atomic::{AtomicBool, Ordering},
        };
        let announced = Arc::new(AtomicBool::new(false));
        let barrier = self.clone();
        std::future::poll_fn(move |_cx| {
            if !announced.swap(true, Ordering::AcqRel)
                && (std::fs::create_dir_all(&barrier.directory).is_err()
                    || std::fs::write(
                        barrier.directory.join(format!("{}.reached", barrier.phase)),
                        b"ready",
                    )
                    .is_err())
            {
                return Poll::Ready(Err("test barrier marker unavailable".to_owned()));
            }
            if barrier
                .directory
                .join(format!("{}.release", barrier.phase))
                .exists()
            {
                Poll::Ready(Ok(()))
            } else {
                Poll::Pending
            }
        })
        .await
    }
}

#[cfg(feature = "synthetic-test-hooks")]
struct TestBarrierProvider {
    inner: Arc<MockModelProvider>,
    barrier: Option<TestBarrier>,
    behavior: Option<TestProviderBehavior>,
}

#[cfg(feature = "synthetic-test-hooks")]
#[derive(Clone, Copy, PartialEq, Eq)]
enum TestProviderBehavior {
    IdentityMismatch,
    DelayedResponse,
    InvalidUsage,
    PaidUsage,
    DailySpendLimit,
    HighTokenUsage,
}

#[cfg(feature = "synthetic-test-hooks")]
#[async_trait::async_trait]
impl ModelProvider for TestBarrierProvider {
    fn provider_id(&self) -> ProviderId {
        self.inner.provider_id()
    }

    fn models(&self) -> Vec<serea_protocol::ModelDescriptor> {
        self.inner.models()
    }

    async fn generate(
        &self,
        request: &ModelRequest,
        context: &serea_protocol::provider::ModelCallContext,
    ) -> Result<ModelResponse, serea_protocol::ModelError> {
        if self.barrier.as_ref().is_some_and(|b| b.phase == "provider") {
            self.barrier
                .as_ref()
                .expect("checked barrier")
                .wait_async()
                .await
                .map_err(|_| test_barrier_error())?;
        }
        let mut response = self.inner.generate(request, context).await?;
        if self
            .barrier
            .as_ref()
            .is_some_and(|b| b.phase == "after-result")
        {
            self.barrier
                .as_ref()
                .expect("checked barrier")
                .wait_async()
                .await
                .map_err(|_| test_barrier_error())?;
        }
        match self.behavior {
            Some(TestProviderBehavior::IdentityMismatch) => {
                response.provider_id = ProviderId::new("synthetic_unexpected_provider")
                    .map_err(|_| test_barrier_error())?;
            }
            Some(TestProviderBehavior::InvalidUsage) => {
                response.usage.output_tokens = serea_protocol::TokenCount::new(9);
            }
            Some(TestProviderBehavior::HighTokenUsage) => {
                // Keep input plus output at the fake descriptor's 131,072
                // context-window ceiling while crossing Router's 128,000
                // per-Task usage bound.
                response.usage.input_tokens = serea_protocol::TokenCount::new(131_071);
            }
            Some(TestProviderBehavior::PaidUsage) => {
                response.usage.cost_class = CostClass::Paid;
            }
            Some(TestProviderBehavior::DailySpendLimit) => {}
            Some(TestProviderBehavior::DelayedResponse) => {
                delayed_fake_response().await;
            }
            None => {}
        }
        Ok(response)
    }

    async fn health(&self) -> ProviderHealth {
        self.inner.health().await
    }
}

#[cfg(feature = "synthetic-test-hooks")]
async fn delayed_fake_response() {
    use std::sync::atomic::{AtomicBool, Ordering};

    let ready = Arc::new(AtomicBool::new(false));
    let wait_ready = Arc::clone(&ready);
    let mut started = false;
    std::future::poll_fn(move |cx| {
        if wait_ready.load(Ordering::Acquire) {
            return Poll::Ready(());
        }
        if !started {
            started = true;
            let wake = cx.waker().clone();
            let ready = Arc::clone(&wait_ready);
            std::thread::spawn(move || {
                std::thread::park_timeout(std::time::Duration::from_millis(1_000));
                ready.store(true, Ordering::Release);
                wake.wake();
            });
        }
        Poll::Pending
    })
    .await
}

#[cfg(feature = "synthetic-test-hooks")]
fn test_barrier_error() -> serea_protocol::ModelError {
    serea_protocol::ModelError {
        kind: serea_protocol::ModelErrorCode::new("TEST_BARRIER_FAILED")
            .unwrap_or_else(|_| unreachable!()),
        message: serea_protocol::ErrorMessage::new("synthetic test barrier failed")
            .unwrap_or_else(|_| unreachable!()),
        retryable: false,
    }
}

fn configured_router_with_barrier(
    failure: bool,
    #[cfg(feature = "synthetic-test-hooks")] barrier: Option<TestBarrier>,
    #[cfg(not(feature = "synthetic-test-hooks"))] _barrier: Option<()>,
    #[cfg(feature = "synthetic-test-hooks")] behavior: Option<TestProviderBehavior>,
    #[cfg(not(feature = "synthetic-test-hooks"))] _behavior: Option<()>,
) -> Result<(ModelRouterProcessV1, Arc<MockModelProvider>), String> {
    let mut descriptors = synthetic_model_roster();
    descriptors.truncate(1); // this one-call probe has no fallback candidate
    let provider_id = descriptors
        .first()
        .ok_or_else(|| "synthetic roster is empty".to_owned())?
        .provider_id
        .clone();
    let provider = Arc::new(MockModelProvider::new(
        provider_id.clone(),
        descriptors.clone(),
    ));
    if failure {
        provider.push(ModelScript::Fail(serea_protocol::ModelError {
            kind: serea_protocol::ModelErrorCode::new("SYNTHETIC_FAILURE")
                .map_err(|_| "synthetic error code rejected")?,
            message: serea_protocol::ErrorMessage::new("scripted synthetic failure")
                .map_err(|_| "synthetic error message rejected")?,
            retryable: false,
        }));
    } else {
        provider.push(ModelScript::EchoText("synthetic response".to_owned()));
    }
    #[cfg(feature = "synthetic-test-hooks")]
    let providers: Vec<Arc<dyn ModelProvider>> = if barrier.is_some() || behavior.is_some() {
        vec![Arc::new(TestBarrierProvider {
            inner: provider.clone(),
            barrier,
            behavior,
        })]
    } else {
        vec![provider.clone()]
    };
    #[cfg(not(feature = "synthetic-test-hooks"))]
    let providers: Vec<Arc<dyn ModelProvider>> = vec![provider.clone()];
    #[cfg(feature = "synthetic-test-hooks")]
    let cost_class = if matches!(
        behavior,
        Some(
            TestProviderBehavior::PaidUsage
                | TestProviderBehavior::DailySpendLimit
                | TestProviderBehavior::DelayedResponse
        )
    ) {
        CostClass::Paid
    } else {
        CostClass::Free
    };
    #[cfg(not(feature = "synthetic-test-hooks"))]
    let cost_class = CostClass::Free;
    let mut entries = Vec::with_capacity(descriptors.len());
    let mut prices = Vec::with_capacity(descriptors.len());
    for descriptor in descriptors {
        let id = descriptor.model_id.clone();
        entries.push(
            ModelRosterEntryV1::new(
                id.clone(),
                provider_id.clone(),
                ModelDeploymentClass::Local,
                true,
                descriptor.capabilities,
                cost_class,
            )
            .map_err(|_| "synthetic model roster rejected")?,
        );
        prices.push((
            id,
            if cost_class == CostClass::Paid {
                ModelPriceSnapshot::new(CostClass::Paid, "synthetic-paid-v1", 1_000_000, 1_000_000)
            } else {
                ModelPriceSnapshot::new(CostClass::Free, "synthetic-free-v1", 0, 0)
            },
        ));
    }
    let roster = ModelRosterV1::new(entries).map_err(|_| "synthetic roster invalid")?;
    let process = ModelRouterProcessV1::new(
        roster,
        providers,
        prices,
        #[cfg(feature = "synthetic-test-hooks")]
        if matches!(
            behavior,
            Some(TestProviderBehavior::PaidUsage | TestProviderBehavior::DelayedResponse)
        ) {
            UsdMicros::new(10_000_000).map_err(|_| "synthetic budget invalid")?
        } else if matches!(behavior, Some(TestProviderBehavior::DailySpendLimit)) {
            UsdMicros::new(1).map_err(|_| "synthetic budget invalid")?
        } else {
            UsdMicros::new(0).map_err(|_| "synthetic budget invalid")?
        },
        #[cfg(not(feature = "synthetic-test-hooks"))]
        UsdMicros::new(0).map_err(|_| "synthetic budget invalid")?,
    )
    .map_err(|_| "synthetic Router configuration rejected")?;
    Ok((process, provider))
}

pub(crate) fn config_check() -> Result<(), String> {
    configured_router(false).map(|_| ())
}

/// Reconciles only durable unresolved attempts before the synthetic Host is
/// allowed to report readiness. This path does not construct or call a Router.
pub(crate) fn recover_unresolved(
    root: &Path,
    events: &EventBus,
    clock: &dyn Clock,
) -> Result<usize, String> {
    let store = Store::open(&root.join("core.sqlite"), clock)
        .map_err(|_| "synthetic recovery storage unavailable")?;
    serea_model_router::recover_unresolved_model_calls(&store, events, clock)
        .map_err(|_| "synthetic model recovery failed".to_owned())
}

fn prepared_call(task_id: Option<&str>) -> Result<PreparedModelCallV1, String> {
    let draft = PreparedModelCallDraftV1 {
        task_id: task_id
            .map(|value| serea_protocol::TaskId::new(value.to_owned()))
            .transpose()
            .map_err(|_| "synthetic task identity rejected")?,
        purpose: ModelPurpose::Chat,
        messages: vec![ModelMessage {
            role: serea_protocol::MessageRole::new("user")
                .map_err(|_| "synthetic message role rejected")?,
            content: "PUBLIC synthetic host router probe".to_owned(),
        }],
        system: None,
        response_format: ResponseFormat::Text,
        tools: Vec::new(),
        max_output_tokens: 8,
        temperature: 0.0,
        deadline_ms: 10_000,
        data_class: DataClass::Public,
        requirements: ModelRoutingRequirementsV1 {
            vision_required: false,
            tools_required: false,
            min_context_tokens: 1,
            min_output_tokens: 1,
            structured_requirement: StructuredRequirementV1::Any,
        },
        egress: ModelEgressPolicySnapshotV1::from_host(false),
        host_max_output_tokens: 8,
    };
    PreparedModelCallV1::from_host(draft).map_err(|_| "synthetic host request rejected".to_owned())
}

/// Performs one explicitly requested, durable, in-process fake dispatch.
#[cfg(not(feature = "synthetic-test-hooks"))]
pub(crate) fn execute_once(
    root: &Path,
    failure: bool,
    task_id: Option<&str>,
) -> Result<(), String> {
    execute_once_inner(
        root,
        if failure { "failure" } else { "success" },
        failure,
        None,
        None,
        OfflineGate {
            cancelled: false,
            cancel_after_initial: false,
            deadline_ms: 10_000,
            snapshots: std::sync::atomic::AtomicUsize::new(0),
        },
        std::time::Duration::from_millis(10_000),
        task_id,
        None,
    )
}

#[cfg(feature = "synthetic-test-hooks")]
pub(crate) fn execute_once_with_test_barrier(
    root: &Path,
    scenario: &str,
    phase: Option<&str>,
    barrier_directory: Option<&Path>,
    task_id: Option<&str>,
) -> Result<(), String> {
    let failure = scenario == "failure";
    let barrier = match (phase, barrier_directory) {
        (None, None) => None,
        (Some("pre-dispatch"), Some(directory)) => Some(TestBarrier {
            directory: directory.to_path_buf(),
            phase: "pre-dispatch",
        }),
        (Some("provider"), Some(directory)) => Some(TestBarrier {
            directory: directory.to_path_buf(),
            phase: "provider",
        }),
        (Some("after-result"), Some(directory)) => Some(TestBarrier {
            directory: directory.to_path_buf(),
            phase: "after-result",
        }),
        (Some("terminal"), Some(directory)) => Some(TestBarrier {
            directory: directory.to_path_buf(),
            phase: "terminal",
        }),
        _ => return Err("invalid test barrier configuration".to_owned()),
    };
    let behavior = match scenario {
        "success" | "failure" => None,
        "cancel-during-provider" => Some(TestProviderBehavior::PaidUsage),
        "budget-probe" => None,
        "token-probe" => None,
        "identity-mismatch" => Some(TestProviderBehavior::IdentityMismatch),
        "token-usage" => Some(TestProviderBehavior::HighTokenUsage),
        "timeout" => Some(TestProviderBehavior::DelayedResponse),
        "usage-bound" => Some(TestProviderBehavior::InvalidUsage),
        "paid-success" | "cancelled-after-intent" => Some(TestProviderBehavior::PaidUsage),
        "spend-bound" => Some(TestProviderBehavior::DailySpendLimit),
        "cancelled" | "expired" => None,
        _ => return Err("invalid synthetic test scenario".to_owned()),
    };
    let gate = OfflineGate {
        cancelled: scenario == "cancelled",
        cancel_after_initial: scenario == "cancelled-after-intent",
        deadline_ms: if scenario == "expired" { 0 } else { 10_000 },
        snapshots: std::sync::atomic::AtomicUsize::new(0),
    };
    execute_once_inner(
        root,
        scenario,
        failure,
        barrier,
        behavior,
        gate,
        if scenario == "timeout" {
            std::time::Duration::from_millis(150)
        } else {
            std::time::Duration::from_millis(10_000)
        },
        task_id,
        (scenario == "cancel-during-provider")
            .then(|| barrier_directory.map(|dir| dir.join("cancel.request")))
            .flatten(),
    )
}

#[allow(clippy::too_many_arguments)]
fn execute_once_inner(
    root: &Path,
    #[cfg(feature = "synthetic-test-hooks")] scenario: &str,
    #[cfg(not(feature = "synthetic-test-hooks"))] _scenario: &str,
    failure: bool,
    #[cfg(feature = "synthetic-test-hooks")] barrier: Option<TestBarrier>,
    #[cfg(not(feature = "synthetic-test-hooks"))] _barrier: Option<()>,
    #[cfg(feature = "synthetic-test-hooks")] behavior: Option<TestProviderBehavior>,
    #[cfg(not(feature = "synthetic-test-hooks"))] _behavior: Option<()>,
    gate: OfflineGate,
    timeout: std::time::Duration,
    task_id: Option<&str>,
    #[cfg(feature = "synthetic-test-hooks")] cancel_marker: Option<std::path::PathBuf>,
    #[cfg(not(feature = "synthetic-test-hooks"))] _cancel_marker: Option<()>,
) -> Result<(), String> {
    std::fs::create_dir_all(root).map_err(|_| "synthetic data directory unavailable")?;
    let lock = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(root.join("host.lock"))
        .map_err(|_| "synthetic host lock unavailable")?;
    fs2::FileExt::try_lock_exclusive(&lock).map_err(|_| "synthetic host is already running")?;
    let _lock_guard = ModelLock(lock);
    let db_path = root.join("core.sqlite");
    let clock = super::host_clock::WallClock;
    let now = clock.now_ms().map_err(|_| "synthetic clock unavailable")?;
    let id_ms = u64::try_from(now.get()).map_err(|_| "synthetic clock before ULID epoch")?;
    let timestamp =
        TimestampMs::new(id_ms).map_err(|_| "synthetic identifier timestamp invalid")?;
    let ids = super::synthetic_ids::SyntheticHostUlidSource::new(timestamp)
        .map_err(|_| "synthetic identifier entropy unavailable")?;
    let events = EventBus::new(ids);
    let store = Store::open(&db_path, &clock).map_err(|_| "synthetic storage unavailable")?;
    #[cfg(feature = "synthetic-test-hooks")]
    let provider_barrier = barrier
        .clone()
        .filter(|b| matches!(b.phase, "provider" | "after-result"));
    #[cfg(not(feature = "synthetic-test-hooks"))]
    let provider_barrier = None;
    let (router, provider) = configured_router_with_barrier(
        failure,
        provider_barrier,
        #[cfg(feature = "synthetic-test-hooks")]
        behavior,
        #[cfg(not(feature = "synthetic-test-hooks"))]
        None,
    )?;
    let call = prepared_call(task_id)?;
    let host = ModelRouterHostContextV1::new(&store, &events, &clock, &gate);
    #[cfg(feature = "synthetic-test-hooks")]
    if let Some(barrier) = barrier.clone().filter(|b| b.phase == "pre-dispatch") {
        barrier.wait()?;
    }
    #[cfg(feature = "synthetic-test-hooks")]
    let (outcome, cancelled_during_provider) = match block_on_control(
        router.execute(&call, &host),
        timeout,
        cancel_marker.as_deref(),
    ) {
        ControlOutcome::Completed(outcome) => (Some(outcome), false),
        ControlOutcome::TimedOut | ControlOutcome::Cancelled => {
            // Dropping the in-flight Router future leaves its committed
            // dispatch intent unresolved. Reuse Router recovery to mark
            // it ambiguous; this never invokes or retries a provider.
            serea_model_router::recover_unresolved_model_calls(&store, &events, &clock)
                .map_err(|_| "synthetic timeout recovery failed")?;
            (None, true)
        }
    };
    #[cfg(not(feature = "synthetic-test-hooks"))]
    let (outcome, _cancelled_during_provider) =
        match block_on_timeout(router.execute(&call, &host), timeout) {
            Some(outcome) => (Some(outcome), false),
            None => {
                serea_model_router::recover_unresolved_model_calls(&store, &events, &clock)
                    .map_err(|_| "synthetic timeout recovery failed")?;
                (None, false)
            }
        };
    #[cfg(feature = "synthetic-test-hooks")]
    if let Some(barrier) = barrier.filter(|b| b.phase == "terminal") {
        barrier.wait()?;
    }
    #[cfg(feature = "synthetic-test-hooks")]
    let expected = match scenario {
        "cancelled" => matches!(
            &outcome,
            Some(Err(ModelRouterCallFailureV1::Refused(
                RouterError::TaskCancelled
            )))
        ),
        "cancelled-after-intent" => matches!(
            &outcome,
            Some(Err(ModelRouterCallFailureV1::Refused(
                RouterError::TaskCancelled
            )))
        ),
        "expired" => matches!(
            &outcome,
            Some(Err(ModelRouterCallFailureV1::Refused(
                RouterError::DeadlineExpired
            )))
        ),
        "identity-mismatch" | "usage-bound" => matches!(
            &outcome,
            Some(Err(ModelRouterCallFailureV1::TerminalProvider { .. }))
        ),
        "spend-bound" => matches!(
            &outcome,
            Some(Err(ModelRouterCallFailureV1::BoundExceeded { .. }))
        ),
        "budget-probe" => matches!(
            &outcome,
            Some(Err(
                ModelRouterCallFailureV1::ModelCallBudgetExceeded { .. }
            ))
        ),
        "token-probe" => matches!(
            &outcome,
            Some(Err(ModelRouterCallFailureV1::TaskBudgetExceeded {
                bound: serea_event_bus::ModelBoundKindV1::TaskTotalTokens,
                ..
            } | ModelRouterCallFailureV1::BoundExceeded {
                bound: serea_event_bus::ModelBoundKindV1::TaskTotalTokens,
                ..
            }))
        ),
        "timeout" => outcome.is_none(),
        "cancel-during-provider" => outcome.is_none() && cancelled_during_provider,
        "token-usage" => outcome.is_some(),
        "failure" => matches!(
            &outcome,
            Some(Err(ModelRouterCallFailureV1::DefiniteProvider {
                retryable: false,
                ..
            }))
        ),
        "success" | "paid-success" => {
            matches!(&outcome, Some(Ok(response)) if response.content == "synthetic response")
        }
        _ => false,
    };
    #[cfg(feature = "synthetic-test-hooks")]
    if !expected {
        return Err("synthetic Router result did not match scripted outcome".to_owned());
    }
    #[cfg(not(feature = "synthetic-test-hooks"))]
    if !matches!((&outcome, failure), (Some(Ok(response)), false) if response.content == "synthetic response")
        && !matches!(
            (&outcome, failure),
            (
                Some(Err(ModelRouterCallFailureV1::DefiniteProvider {
                    retryable: false,
                    ..
                })),
                true
            )
        )
    {
        return Err("synthetic Router result did not match scripted outcome".to_owned());
    }
    #[cfg(feature = "synthetic-test-hooks")]
    let expected_calls = if matches!(
        scenario,
        "cancelled"
            | "cancelled-after-intent"
            | "expired"
            | "spend-bound"
            | "budget-probe"
            | "token-probe"
            | "cancel-during-provider"
    ) {
        0
    } else {
        1
    };
    #[cfg(not(feature = "synthetic-test-hooks"))]
    let expected_calls = 1;
    if provider.calls() != expected_calls {
        return Err("synthetic Router provider call count was unexpected".to_owned());
    }
    if !store
        .list_unfinished_model_call_attempts()
        .map_err(|_| "synthetic accounting inspection failed")?
        .is_empty()
    {
        return Err("synthetic dispatch left an unresolved attempt".to_owned());
    }
    match outcome {
        Some(Ok(_)) => println!(
            "{{\"event\":\"synthetic_model_completed\",\"provider\":\"in_process_fake\",\"result\":\"success\"}}"
        ),
        Some(Err(_)) => println!(
            "{{\"event\":\"synthetic_model_failed\",\"provider\":\"in_process_fake\",\"result\":\"terminal_failure\"}}"
        ),
        None => println!(
            "{{\"event\":\"synthetic_model_ambiguous\",\"provider\":\"in_process_fake\",\"result\":\"unresolved_after_dispatch\"}}"
        ),
    }
    store
        .checkpoint_for_close()
        .map_err(|_| "synthetic database checkpoint failed")?;
    Ok(())
}

#[cfg(feature = "synthetic-test-hooks")]
enum ControlOutcome<T> {
    Completed(T),
    TimedOut,
    Cancelled,
}

#[cfg(feature = "synthetic-test-hooks")]
#[allow(clippy::disallowed_methods)]
fn block_on_control<F: Future>(
    future: F,
    timeout: std::time::Duration,
    cancel_marker: Option<&Path>,
) -> ControlOutcome<F::Output> {
    let Some(deadline) = std::time::Instant::now().checked_add(timeout) else {
        return ControlOutcome::TimedOut;
    };
    let waker = Waker::from(Arc::new(ThreadWake(std::thread::current())));
    let mut context = Context::from_waker(&waker);
    let mut future = pin!(future);
    loop {
        if cancel_marker.is_some_and(Path::exists) {
            return ControlOutcome::Cancelled;
        }
        match future.as_mut().poll(&mut context) {
            Poll::Ready(value) => return ControlOutcome::Completed(value),
            Poll::Pending if std::time::Instant::now() >= deadline => {
                return ControlOutcome::TimedOut;
            }
            Poll::Pending => std::thread::park_timeout(std::time::Duration::from_millis(1)),
        }
    }
}

struct ModelLock(std::fs::File);

impl Drop for ModelLock {
    fn drop(&mut self) {
        let _ = fs2::FileExt::unlock(&self.0);
    }
}

struct ThreadWake(std::thread::Thread);

impl Wake for ThreadWake {
    fn wake(self: Arc<Self>) {
        self.0.unpark();
    }

    fn wake_by_ref(self: &Arc<Self>) {
        self.0.unpark();
    }
}

// A bounded in-process provider wait needs a monotonic elapsed-time source;
// wall-clock `Clock` values can jump and are not suitable for timeout races.
#[cfg(not(feature = "synthetic-test-hooks"))]
#[allow(clippy::disallowed_methods)]
fn block_on_timeout<F: Future>(future: F, timeout: std::time::Duration) -> Option<F::Output> {
    let deadline = std::time::Instant::now().checked_add(timeout)?;
    let waker = Waker::from(Arc::new(ThreadWake(std::thread::current())));
    let mut context = Context::from_waker(&waker);
    let mut future = pin!(future);
    loop {
        match future.as_mut().poll(&mut context) {
            Poll::Ready(value) => return Some(value),
            Poll::Pending => {
                let now = std::time::Instant::now();
                if now >= deadline {
                    return None;
                }
                std::thread::park_timeout(deadline.saturating_duration_since(now));
            }
        }
    }
}
