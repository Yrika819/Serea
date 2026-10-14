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
use serea_protocol::{Clock, CostClass, DataClass, ModelMessage, ModelPurpose, ResponseFormat};
use serea_storage::{ModelPriceSnapshot, Store, UsdMicros};
use serea_testkit::{
    DeterministicUlidSource, MockModelProvider, ModelScript, synthetic_model_roster,
};

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
pub(crate) fn execute_once(root: &Path, failure: bool) -> Result<(), String> {
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
    let events = EventBus::new(
        DeterministicUlidSource::starting_at(id_ms)
            .map_err(|_| "synthetic identifier source unavailable")?,
    );
    let store = Store::open(&db_path, &clock).map_err(|_| "synthetic storage unavailable")?;
    let (router, provider) = configured_router(failure)?;
    let call = prepared_call()?;
    let host = ModelRouterHostContextV1::new(&store, &events, &clock, &OfflineGate);
    match (failure, block_on(router.execute(&call, &host))) {
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
