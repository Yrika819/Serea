//! Offline model dispatch reachable only through explicit synthetic commands.

use std::fs::OpenOptions;
use std::future::Future;
use std::path::Path;
use std::pin::pin;
use std::sync::Arc;
use std::task::{Context, Poll, Wake, Waker};

use serea_event_bus::EventBus;
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

struct OfflineGate;

impl ModelDispatchGateSource for OfflineGate {
    fn snapshot(
        &self,
        _task_id: Option<&serea_protocol::TaskId>,
        _data_class: DataClass,
    ) -> Result<ModelDispatchGateSnapshotV1, ModelDispatchGateSourceError> {
        Ok(ModelDispatchGateSnapshotV1::from_host(
            false,
            ModelEgressPolicySnapshotV1::from_host(false),
            10_000,
        ))
    }
}

fn configured_router(
    failure: bool,
) -> Result<(ModelRouterProcessV1, Arc<MockModelProvider>), String> {
    configured_router_with_barrier(failure, None)
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
}

#[cfg(feature = "synthetic-test-hooks")]
struct TestBarrierProvider {
    inner: Arc<MockModelProvider>,
    barrier: TestBarrier,
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
        if self.barrier.phase == "provider" {
            self.barrier.wait().map_err(|_| test_barrier_error())?;
        }
        let response = self.inner.generate(request, context).await?;
        if self.barrier.phase == "after-result" {
            self.barrier.wait().map_err(|_| test_barrier_error())?;
        }
        Ok(response)
    }

    async fn health(&self) -> ProviderHealth {
        self.inner.health().await
    }
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
    let providers: Vec<Arc<dyn ModelProvider>> = match barrier {
        Some(barrier) => vec![Arc::new(TestBarrierProvider {
            inner: provider.clone(),
            barrier,
        })],
        None => vec![provider.clone()],
    };
    #[cfg(not(feature = "synthetic-test-hooks"))]
    let providers: Vec<Arc<dyn ModelProvider>> = vec![provider.clone()];
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
                CostClass::Free,
            )
            .map_err(|_| "synthetic model roster rejected")?,
        );
        prices.push((
            id,
            ModelPriceSnapshot::new(CostClass::Free, "synthetic-free-v1", 0, 0),
        ));
    }
    let roster = ModelRosterV1::new(entries).map_err(|_| "synthetic roster invalid")?;
    let process = ModelRouterProcessV1::new(
        roster,
        providers,
        prices,
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

fn prepared_call() -> Result<PreparedModelCallV1, String> {
    let draft = PreparedModelCallDraftV1 {
        task_id: None,
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
pub(crate) fn execute_once(root: &Path, failure: bool) -> Result<(), String> {
    execute_once_inner(root, failure, None)
}

#[cfg(feature = "synthetic-test-hooks")]
pub(crate) fn execute_once_with_test_barrier(
    root: &Path,
    failure: bool,
    phase: Option<&str>,
    barrier_directory: Option<&Path>,
) -> Result<(), String> {
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
    execute_once_inner(root, failure, barrier)
}

fn execute_once_inner(
    root: &Path,
    failure: bool,
    #[cfg(feature = "synthetic-test-hooks")] barrier: Option<TestBarrier>,
    #[cfg(not(feature = "synthetic-test-hooks"))] _barrier: Option<()>,
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
    let (router, provider) = configured_router_with_barrier(failure, provider_barrier)?;
    let call = prepared_call()?;
    let host = ModelRouterHostContextV1::new(&store, &events, &clock, &OfflineGate);
    #[cfg(feature = "synthetic-test-hooks")]
    if let Some(barrier) = barrier.clone().filter(|b| b.phase == "pre-dispatch") {
        barrier.wait()?;
    }
    let outcome = block_on(router.execute(&call, &host));
    #[cfg(feature = "synthetic-test-hooks")]
    if let Some(barrier) = barrier.filter(|b| b.phase == "terminal") {
        barrier.wait()?;
    }
    match (failure, outcome) {
        (false, Ok(response)) if response.content == "synthetic response" => {}
        (
            true,
            Err(ModelRouterCallFailureV1::DefiniteProvider {
                retryable: false, ..
            }),
        ) => {}
        _ => return Err("synthetic Router result did not match scripted outcome".to_owned()),
    }
    if provider.calls() != 1 {
        return Err("synthetic Router attempted an unintended retry".to_owned());
    }
    if !store
        .list_unfinished_model_call_attempts()
        .map_err(|_| "synthetic accounting inspection failed")?
        .is_empty()
    {
        return Err("synthetic dispatch left an unresolved attempt".to_owned());
    }
    if failure {
        println!(
            "{{\"event\":\"synthetic_model_failed\",\"provider\":\"in_process_fake\",\"result\":\"definite_failure\"}}"
        );
    } else {
        println!(
            "{{\"event\":\"synthetic_model_completed\",\"provider\":\"in_process_fake\",\"result\":\"success\"}}"
        );
    }
    store
        .checkpoint_for_close()
        .map_err(|_| "synthetic database checkpoint failed")?;
    Ok(())
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

fn block_on<F: Future>(future: F) -> F::Output {
    let waker = Waker::from(Arc::new(ThreadWake(std::thread::current())));
    let mut context = Context::from_waker(&waker);
    let mut future = pin!(future);
    loop {
        match future.as_mut().poll(&mut context) {
            Poll::Ready(value) => return value,
            Poll::Pending => std::thread::park(),
        }
    }
}
