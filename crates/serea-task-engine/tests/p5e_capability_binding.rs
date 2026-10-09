//! P5E: Task generation pinning and capability Step binding.
//!
//! Post-P5 Tasks pin the active registry generation at creation. A pinned Task
//! resolves new capability Steps against that generation forever, while a live
//! overlay still gates every NEW binding. Already-bound Steps keep their exact
//! revision through every later change. Nothing here executes anything.
mod support;

use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

use serea_capability::{
    CapabilityAvailabilitySnapshotV1, CapabilityManifestV1, CapabilitySchemaCatalogV1,
    HostEligibility, ManifestEntryV1, ProviderRegistry, descriptor_semantic_digest, install,
};
use serea_event_bus::EventBus;
use serea_protocol::provider::CapabilityProvider;
use serea_protocol::*;
use serea_storage::Store;
use serea_task_engine::*;
use support::event_bus;

const PREFIX: &str = "https://serea.local/schemas/";

struct Fixed;
impl Clock for Fixed {
    fn now_ms(&self) -> Result<EpochMillis, ProtocolError> {
        Ok(EpochMillis::new(1_796_000_000_000).unwrap())
    }
}

fn at(n: i64) -> EpochMillis {
    EpochMillis::new(n).unwrap()
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

fn spec(n: u32) -> NewTask {
    spec_with_class(n, DataClass::Personal)
}

fn spec_with_class(n: u32, data_class: DataClass) -> NewTask {
    NewTask {
        task_id: tid(n),
        kind: TaskKind::UserRequest,
        title: TaskTitle::new("read my calendar").unwrap(),
        origin: TaskOrigin {
            kind: TaskOriginKind::new("USER_MESSAGE").unwrap(),
            device_id: None,
            message_id: None,
            extensions: [].into(),
        },
        data_class,
        policy_class: RiskClass::Observe,
        attempt_budget: AttemptBudget {
            max_model_calls: 12,
            max_tool_calls: 24,
            max_attempts_per_step: 3,
            extensions: [].into(),
        },
        created_at: at(10),
        deadline_at: Some(at(1_000_000)),
        extensions: [].into(),
    }
}

fn historical_task(n: u32) -> AssistantTask {
    let spec = spec(n);
    AssistantTask {
        task_id: spec.task_id,
        kind: spec.kind,
        title: spec.title,
        state: TaskState::Received,
        origin: spec.origin,
        data_class: spec.data_class,
        policy_class: spec.policy_class,
        created_at: Timestamp::from_epoch_millis(spec.created_at),
        updated_at: Timestamp::from_epoch_millis(spec.created_at),
        deadline_at: spec.deadline_at.map(Timestamp::from_epoch_millis),
        attempt_budget: spec.attempt_budget,
        steps: Vec::new(),
        blocked_reason: None,
        result_summary: None,
        cancelled_at: None,
        cancelled_by: None,
        failure_reason: None,
        extensions: spec.extensions,
    }
}

fn classified_args() -> serea_capability::ClassifiedArgumentsV1 {
    classified_args_with_class(DataClass::Personal)
}

fn classified_args_with_class(class: DataClass) -> serea_capability::ClassifiedArgumentsV1 {
    let mut args = serde_json::Map::new();
    args.insert("calendar".into(), serde_json::Value::String("work".into()));
    serea_capability::ClassifiedArgumentsV1::new_trusted(args, class)
}

fn catalog() -> CapabilitySchemaCatalogV1 {
    let mut docs = BTreeMap::new();
    docs.insert(
        format!("{PREFIX}cal-input.json"),
        r#"{"$schema":"https://json-schema.org/draft/2020-12/schema","type":"object","additionalProperties":false,"properties":{"calendar":{"type":"string","maxLength":64}}}"#
            .to_string(),
    );
    docs.insert(
        format!("{PREFIX}cal-output.json"),
        r#"{"$schema":"https://json-schema.org/draft/2020-12/schema","type":"object","additionalProperties":false,"properties":{"ok":{"type":"boolean"}}}"#
            .to_string(),
    );
    CapabilitySchemaCatalogV1::build(docs).unwrap()
}

fn descriptor() -> CapabilityDescriptor {
    descriptor_with_class(DataClass::Personal)
}

fn descriptor_with_class(data_class: DataClass) -> CapabilityDescriptor {
    CapabilityDescriptor::new(CapabilityDescriptorDraft {
        id: CapabilityId::new("calendar.events.read").unwrap(),
        version: SemVer::new("1.0.0").unwrap(),
        title: DescriptorTitle::new("Read events").unwrap(),
        description: DescriptorDescription::new("Read private calendar events").unwrap(),
        provider_id: ProviderId::new("calendar").unwrap(),
        implementation_id: None,
        input_schema: JsonSchemaRef::new(format!("{PREFIX}cal-input.json")).unwrap(),
        output_schema: JsonSchemaRef::new(format!("{PREFIX}cal-output.json")).unwrap(),
        side_effect_class: SideEffectClass::None,
        risk_class: RiskClass::Observe,
        required_authorization: Authorization::None,
        replay_safety: ReplaySafety::Idempotent,
        data_class,
        root_requirement: RootRequirement::NotRequired,
        idempotency_support: IdempotencySupport::None,
        max_duration_ms: 5_000,
        cost_class: CostClass::Free,
        experimental: false,
    })
    .unwrap()
}

/// Builds a manifest from `(descriptor, candidate_priority)` pairs and an
/// explicit default-version selection, so a test can differ from another
/// generation only in default version or candidate priority.
fn manifest_built(
    described: Vec<(CapabilityDescriptor, u32)>,
    defaults: Vec<(CapabilityId, SemVer)>,
) -> CapabilityManifestV1 {
    let catalog = catalog();
    let entries: Vec<ManifestEntryV1> = described
        .iter()
        .map(|(d, priority)| ManifestEntryV1 {
            descriptor: d.clone(),
            candidate_priority: *priority,
            descriptor_digest: descriptor_semantic_digest(d, &catalog).unwrap(),
            input_schema_digest: catalog
                .document_digest(d.input_schema().as_str())
                .unwrap()
                .clone(),
            output_schema_digest: catalog
                .document_digest(d.output_schema().as_str())
                .unwrap()
                .clone(),
        })
        .collect();
    CapabilityManifestV1::build(entries, defaults, catalog).unwrap()
}

fn manifest_for(descriptors: Vec<CapabilityDescriptor>) -> CapabilityManifestV1 {
    let defaults = descriptors
        .iter()
        .map(|d| (d.id().clone(), d.version().clone()))
        .collect();
    manifest_built(descriptors.into_iter().map(|d| (d, 0)).collect(), defaults)
}

struct TestProvider {
    id: ProviderId,
    descriptors: Vec<CapabilityDescriptor>,
    health: ProviderHealth,
}

#[async_trait::async_trait]
impl CapabilityProvider for TestProvider {
    fn provider_id(&self) -> ProviderId {
        self.id.clone()
    }
    fn capabilities(&self) -> Vec<CapabilityDescriptor> {
        self.descriptors.clone()
    }
    async fn invoke(
        &self,
        _request: &ActionRequest,
        _ctx: &serea_protocol::provider::ProviderContext,
    ) -> Result<ActionResult, ActionError> {
        panic!("P5E must never invoke a provider")
    }
    async fn health(&self) -> ProviderHealth {
        self.health
    }
}

fn provider(
    descriptors: Vec<CapabilityDescriptor>,
    health: ProviderHealth,
) -> Arc<dyn CapabilityProvider> {
    provider_with("calendar", descriptors, health)
}

fn provider_with(
    provider_id: &str,
    descriptors: Vec<CapabilityDescriptor>,
    health: ProviderHealth,
) -> Arc<dyn CapabilityProvider> {
    Arc::new(TestProvider {
        id: ProviderId::new(provider_id).unwrap(),
        descriptors,
        health,
    })
}

/// Installs a manifest and returns the active generation id.
fn install_manifest(
    store: &Store,
    events: &EventBus,
    manifest: &CapabilityManifestV1,
    providers: Vec<Arc<dyn CapabilityProvider>>,
) -> i64 {
    let registry = ProviderRegistry::build(providers).unwrap();
    registry.validate_advertisements(manifest).unwrap();
    let outcome = install(store, events, manifest, &registry, at(50)).unwrap();
    assert!(outcome.activated());
    outcome.generation_id()
}

fn snapshot(
    store: &Store,
    manifest: CapabilityManifestV1,
    providers: Vec<Arc<dyn CapabilityProvider>>,
    eligible: bool,
) -> CapabilityAvailabilitySnapshotV1 {
    let generation_id = store
        .current_registry_generation()
        .unwrap()
        .expect("test manifest installed")
        .generation_id();
    snapshot_for_generation(store, manifest, providers, eligible, generation_id)
}

fn snapshot_for_generation(
    store: &Store,
    manifest: CapabilityManifestV1,
    providers: Vec<Arc<dyn CapabilityProvider>>,
    eligible: bool,
    generation_id: i64,
) -> CapabilityAvailabilitySnapshotV1 {
    let registry = ProviderRegistry::build(providers).unwrap();
    let mut eligibility = HashMap::new();
    for entry in manifest.entries() {
        eligibility.insert(entry.descriptor_digest.clone(), eligible);
    }
    block_on(CapabilityAvailabilitySnapshotV1::build_for_generation(
        manifest,
        generation_id,
        &registry,
        store,
        HostEligibility::new(eligibility),
    ))
    .unwrap()
}

#[test]
fn historical_snapshot_uses_its_durable_default_and_priority() {
    let temp = TempDb::new();
    let store = temp.open();
    let events = event_bus();
    let mut d1_draft = CapabilityDescriptorDraft::from(descriptor());
    d1_draft.implementation_id = Some(ImplementationId::new("calendar-base").unwrap());
    let d1 = CapabilityDescriptor::new(d1_draft).unwrap();
    let mut draft = CapabilityDescriptorDraft::from(d1.clone());
    draft.version = SemVer::new("2.0.0").unwrap();
    let d2 = CapabilityDescriptor::new(draft).unwrap();
    let providers = vec![provider(
        vec![d1.clone(), d2.clone()],
        ProviderHealth::Ready,
    )];
    let manifest_a = manifest_built(
        vec![(d1.clone(), 1), (d2.clone(), 2)],
        vec![(d1.id().clone(), d1.version().clone())],
    );
    let generation_a = install_manifest(&store, &events, &manifest_a, providers.clone());
    let mut engine = engine(temp.open(), events.clone());
    engine.create_task(spec(1), &Context::new().view()).unwrap();
    assert_eq!(pinned_generation(&store, &tid(1)), Some(generation_a));
    engine
        .start_planning(
            tid(1),
            TaskState::Received,
            0,
            at(20),
            &Context::new().view(),
        )
        .unwrap();

    let manifest_b = manifest_built(
        vec![(d1.clone(), 9), (d2.clone(), 0)],
        vec![(d1.id().clone(), d2.version().clone())],
    );
    let generation_b = install_manifest(&store, &events, &manifest_b, providers.clone());
    assert_ne!(generation_a, generation_b);
    let generation_a_again = install_manifest(&store, &events, &manifest_a, providers.clone());
    assert_ne!(generation_a, generation_a_again);
    let durable_a = store
        .get_registry_generation(generation_a)
        .unwrap()
        .unwrap();
    let durable_a_again = store
        .get_registry_generation(generation_a_again)
        .unwrap()
        .unwrap();
    assert_eq!(
        durable_a.manifest_digest(),
        durable_a_again.manifest_digest()
    );

    let old = snapshot_for_generation(&store, manifest_a, providers, true, generation_a);
    assert_eq!(old.generation_id(), generation_a);
    let resolution = old.resolve(d1.id()).unwrap();
    assert_eq!(resolution.version, d1.version().clone());
    assert_eq!(resolution.provider_id, d1.provider_id().clone());
    let binding = engine
        .create_capability_step(
            &old,
            tid(1),
            sid(1),
            1,
            d1.id(),
            &classified_args(),
            RequestedBy::Scheduler,
            at(30),
            &Context::new().view(),
        )
        .expect("old Task resolves through generation A");
    assert_eq!(binding.generation_id(), generation_a);
    assert_eq!(binding.capability_version(), d1.version());
}

#[test]
fn missing_historical_generation_material_fails_closed() {
    let temp = TempDb::new();
    let store = temp.open();
    let d = descriptor();
    let manifest = manifest_for(vec![d.clone()]);
    let registry = ProviderRegistry::build(vec![provider(vec![d], ProviderHealth::Ready)]).unwrap();
    let result = block_on(CapabilityAvailabilitySnapshotV1::build_for_generation(
        manifest,
        9_999,
        &registry,
        &store,
        HostEligibility::new(HashMap::new()),
    ));
    assert!(matches!(
        result,
        Err(serea_capability::AvailabilityError::RegistryGenerationMismatch)
    ));
}

#[test]
fn historical_snapshot_uses_its_candidate_priority() {
    let temp = TempDb::new();
    let store = temp.open();
    let events = event_bus();
    let mut d1_draft = CapabilityDescriptorDraft::from(descriptor());
    d1_draft.implementation_id = Some(ImplementationId::new("calendar-base").unwrap());
    let d1 = CapabilityDescriptor::new(d1_draft).unwrap();
    let mut draft = CapabilityDescriptorDraft::from(d1.clone());
    draft.implementation_id = Some(ImplementationId::new("calendar-alt").unwrap());
    let d2 = CapabilityDescriptor::new(draft).unwrap();
    let providers = vec![provider(
        vec![d1.clone(), d2.clone()],
        ProviderHealth::Ready,
    )];
    let defaults = vec![(d1.id().clone(), d1.version().clone())];
    let manifest_a = manifest_built(vec![(d1.clone(), 0), (d2.clone(), 1)], defaults.clone());
    let generation_a = install_manifest(&store, &events, &manifest_a, providers.clone());
    let manifest_b = manifest_built(vec![(d1.clone(), 1), (d2.clone(), 0)], defaults);
    let generation_b = install_manifest(&store, &events, &manifest_b, providers.clone());
    assert_ne!(generation_a, generation_b);

    let old = snapshot_for_generation(&store, manifest_a, providers, true, generation_a);
    assert_eq!(
        old.resolve(d1.id()).unwrap().implementation_id,
        d1.implementation_id().cloned()
    );
}

fn block_on<F: std::future::Future>(future: F) -> F::Output {
    use std::task::{Context, Poll, Wake, Waker};
    struct ThreadWake(std::thread::Thread);
    impl Wake for ThreadWake {
        fn wake(self: Arc<Self>) {
            self.0.unpark();
        }
        fn wake_by_ref(self: &Arc<Self>) {
            self.0.unpark();
        }
    }
    let waker = Waker::from(Arc::new(ThreadWake(std::thread::current())));
    let mut context = Context::from_waker(&waker);
    let mut future = Box::pin(future);
    loop {
        match future.as_mut().poll(&mut context) {
            Poll::Ready(output) => return output,
            Poll::Pending => std::thread::park(),
        }
    }
}

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);

/// A file-backed database per test, so a test can hold an engine's Store and
/// still read durable facts through an independent connection.
struct TempDb(PathBuf);

impl TempDb {
    fn new() -> Self {
        let dir = std::env::temp_dir().join(format!(
            "serea-p5e-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&dir).unwrap();
        Self(dir.join("store.sqlite"))
    }
    fn open(&self) -> Store {
        Store::open(&self.0, &Fixed).unwrap()
    }
}

impl Drop for TempDb {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(self.0.parent().unwrap());
    }
}

fn engine(store: Store, events: EventBus) -> TaskEngine {
    TaskEngine::new(store, events)
}

/// The generation a Task is pinned to, read through storage SQL authority.
fn pinned_generation(store: &Store, task: &TaskId) -> Option<i64> {
    store.get_task_registry_generation(task).unwrap()
}

#[test]
fn new_task_pins_the_active_generation() {
    let temp = TempDb::new();
    let store = temp.open();
    let events = event_bus();
    let d = descriptor();
    let generation = install_manifest(
        &store,
        &events,
        &manifest_for(vec![d.clone()]),
        vec![provider(vec![d.clone()], ProviderHealth::Ready)],
    );
    let mut engine = engine(temp.open(), events);
    engine
        .create_task(spec(1), &Context::new().view())
        .expect("task created");
    let pinned = pinned_generation(&store, &tid(1)).expect("pinned at creation");
    assert_eq!(pinned, generation);
}

#[test]
fn task_insert_and_generation_pin_roll_back_together() {
    let temp = TempDb::new();
    let store = temp.open();
    let events = event_bus();
    let d = descriptor();
    install_manifest(
        &store,
        &events,
        &manifest_for(vec![d.clone()]),
        vec![provider(vec![d], ProviderHealth::Ready)],
    );
    let raw = rusqlite::Connection::open(&temp.0).unwrap();
    raw.execute_batch(
        "CREATE TRIGGER reject_task_generation_pin BEFORE UPDATE OF capability_registry_generation ON tasks
         BEGIN SELECT RAISE(ABORT,'pin fault'); END;",
    )
    .unwrap();
    let mut engine = TaskEngine::new(temp.open(), events);
    assert!(engine.create_task(spec(1), &Context::new().view()).is_err());
    let tasks: i64 = raw
        .query_row(
            "SELECT count(*) FROM tasks WHERE task_id=?1",
            [tid(1).as_str()],
            |r| r.get(0),
        )
        .unwrap();
    let journal: i64 = raw
        .query_row(
            "SELECT count(*) FROM task_journal WHERE task_id=?1",
            [tid(1).as_str()],
            |r| r.get(0),
        )
        .unwrap();
    let task_events: i64 = raw
        .query_row(
            "SELECT count(*) FROM event_content WHERE kind='TASK_CREATED'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!((tasks, journal, task_events), (0, 0, 0));
}

#[test]
fn task_creation_without_active_generation_fails_closed() {
    let temp = TempDb::new();
    let store = temp.open();
    let events = event_bus();
    let mut engine = engine(temp.open(), events);
    // No registry generation exists yet: a post-P5 task cannot be created
    // unpinned, and it is never silently attached to "current" later.
    let err = match engine.create_task(spec(1), &Context::new().view()) {
        Ok(_) => panic!("must fail closed without an active generation"),
        Err(error) => error,
    };
    assert!(matches!(
        err,
        EngineError::NoActiveCapabilityGeneration | EngineError::Store(_)
    ));
    assert!(
        store.load_task(&tid(1)).is_err(),
        "refused Task must not persist"
    );
}

#[test]
fn restart_preserves_the_generation_pin() {
    let temp = TempDb::new();
    let generation = {
        let store = temp.open();
        let events = event_bus();
        let d = descriptor();
        install_manifest(
            &store,
            &events,
            &manifest_for(vec![d.clone()]),
            vec![provider(vec![d], ProviderHealth::Ready)],
        );
        let mut engine = engine(temp.open(), events);
        engine.create_task(spec(1), &Context::new().view()).unwrap();
        drop(engine);
        let read_back = temp.open();
        pinned_generation(&read_back, &tid(1)).expect("pinned")
    };
    // reopen through a fresh connection: the pin is durable, not in-memory
    let reopened = temp.open();
    assert_eq!(pinned_generation(&reopened, &tid(1)), Some(generation));
}

#[test]
fn new_capability_step_binds_to_the_pinned_generation_descriptor() {
    let temp = TempDb::new();
    let store = temp.open();
    let events = event_bus();
    let d = descriptor();
    let manifest = manifest_for(vec![d.clone()]);
    let generation = install_manifest(
        &store,
        &events,
        &manifest,
        vec![provider(vec![d.clone()], ProviderHealth::Ready)],
    );
    let mut engine = engine(temp.open(), events.clone());
    engine.create_task(spec(1), &Context::new().view()).unwrap();
    assert_eq!(pinned_generation(&store, &tid(1)), Some(generation));
    engine
        .start_planning(
            tid(1),
            TaskState::Received,
            0,
            at(20),
            &Context::new().view(),
        )
        .unwrap();

    let snap = snapshot(
        &store,
        manifest.clone(),
        vec![provider(vec![d.clone()], ProviderHealth::Ready)],
        true,
    );
    let binding = match engine.create_capability_step(
        &snap,
        tid(1),
        sid(1),
        1,
        &d.id().clone(),
        &classified_args(),
        RequestedBy::Model,
        at(30),
        &Context::new().view(),
    ) {
        Ok(binding) => binding,
        Err(error) => panic!("capability step should bind, got {error:?}"),
    };
    assert_eq!(binding.capability_id(), &d.id().clone());
    assert_eq!(binding.generation_id(), generation);
    assert_eq!(
        binding.descriptor_digest(),
        &descriptor_semantic_digest(&d, &catalog()).unwrap()
    );
}

#[test]
fn task_refuses_snapshot_from_another_generation_even_for_same_descriptor() {
    let temp = TempDb::new();
    let store = temp.open();
    let events = event_bus();
    let d = descriptor();
    let manifest_a = manifest_for(vec![d.clone()]);
    let generation_a = install_manifest(
        &store,
        &events,
        &manifest_a,
        vec![provider(vec![d.clone()], ProviderHealth::Ready)],
    );
    let mut engine = engine(temp.open(), events.clone());
    engine.create_task(spec(1), &Context::new().view()).unwrap();
    assert_eq!(pinned_generation(&store, &tid(1)), Some(generation_a));
    engine
        .start_planning(
            tid(1),
            TaskState::Received,
            0,
            at(20),
            &Context::new().view(),
        )
        .unwrap();

    // A distinct durable generation can have identical manifest semantics.
    let manifest_b = manifest_built(
        vec![(d.clone(), 1)],
        vec![(d.id().clone(), d.version().clone())],
    );
    let generation_b = install_manifest(
        &store,
        &events,
        &manifest_b,
        vec![provider(vec![d.clone()], ProviderHealth::Ready)],
    );
    assert_ne!(generation_a, generation_b);
    let snap_b = snapshot(
        &store,
        manifest_b,
        vec![provider(vec![d.clone()], ProviderHealth::Ready)],
        true,
    );
    let result = engine.create_capability_step(
        &snap_b,
        tid(1),
        sid(1),
        1,
        d.id(),
        &classified_args(),
        RequestedBy::Model,
        at(30),
        &Context::new().view(),
    );
    assert!(matches!(
        result,
        Err(EngineError::CapabilityUnavailable { .. })
    ));
}

#[test]
fn public_arguments_keep_public_class_under_personal_descriptor_and_task() {
    let temp = TempDb::new();
    let store = temp.open();
    let events = event_bus();
    let d = descriptor_with_class(DataClass::Personal);
    let manifest = manifest_for(vec![d.clone()]);
    install_manifest(
        &store,
        &events,
        &manifest,
        vec![provider(vec![d.clone()], ProviderHealth::Ready)],
    );
    let mut engine = engine(temp.open(), events.clone());
    engine.create_task(spec(1), &Context::new().view()).unwrap();
    engine
        .start_planning(
            tid(1),
            TaskState::Received,
            0,
            at(20),
            &Context::new().view(),
        )
        .unwrap();
    let snap = snapshot(
        &store,
        manifest,
        vec![provider(vec![d.clone()], ProviderHealth::Ready)],
        true,
    );
    let binding = engine
        .create_capability_step(
            &snap,
            tid(1),
            sid(1),
            1,
            d.id(),
            &classified_args_with_class(DataClass::Public),
            RequestedBy::Scheduler,
            at(30),
            &Context::new().view(),
        )
        .expect("public arguments fit both containers");
    assert_eq!(binding.generation_id(), snap.generation_id());
    let loaded = engine.load(tid(1)).unwrap();
    assert_eq!(loaded.steps.len(), 1);
    assert_eq!(loaded.steps[0].step.capability_id.as_ref(), Some(d.id()));
}

#[test]
fn private_arguments_are_refused_when_task_container_is_personal() {
    let temp = TempDb::new();
    let store = temp.open();
    let events = event_bus();
    let d = descriptor_with_class(DataClass::Private);
    let manifest = manifest_for(vec![d.clone()]);
    install_manifest(
        &store,
        &events,
        &manifest,
        vec![provider(vec![d.clone()], ProviderHealth::Ready)],
    );
    let mut engine = engine(temp.open(), events.clone());
    engine
        .create_task(
            spec_with_class(1, DataClass::Personal),
            &Context::new().view(),
        )
        .unwrap();
    engine
        .start_planning(
            tid(1),
            TaskState::Received,
            0,
            at(20),
            &Context::new().view(),
        )
        .unwrap();
    let snap = snapshot(
        &store,
        manifest,
        vec![provider(vec![d.clone()], ProviderHealth::Ready)],
        true,
    );
    let result = engine.create_capability_step(
        &snap,
        tid(1),
        sid(1),
        1,
        d.id(),
        &classified_args_with_class(DataClass::Private),
        RequestedBy::Model,
        at(30),
        &Context::new().view(),
    );
    assert!(matches!(
        result,
        Err(EngineError::CapabilityUnavailable { .. })
    ));
    assert!(engine.load(tid(1)).unwrap().steps.is_empty());
}

#[test]
fn two_capability_step_revisions_reopen_as_a_complete_plan() {
    let temp = TempDb::new();
    let store = temp.open();
    let events = event_bus();
    let d = descriptor();
    let manifest = manifest_for(vec![d.clone()]);
    let generation = install_manifest(
        &store,
        &events,
        &manifest,
        vec![provider(vec![d.clone()], ProviderHealth::Ready)],
    );
    let mut engine = engine(temp.open(), events.clone());
    engine.create_task(spec(1), &Context::new().view()).unwrap();
    engine
        .start_planning(
            tid(1),
            TaskState::Received,
            0,
            at(20),
            &Context::new().view(),
        )
        .unwrap();
    let snapshot = snapshot(
        &store,
        manifest,
        vec![provider(vec![d.clone()], ProviderHealth::Ready)],
        true,
    );
    let bindings = engine
        .create_capability_plan(
            &snapshot,
            tid(1),
            vec![
                CapabilityPlanStep {
                    step_id: sid(1),
                    sequence: 1,
                    capability_id: d.id().clone(),
                    arguments: classified_args(),
                    requested_by: RequestedBy::Model,
                },
                CapabilityPlanStep {
                    step_id: sid(2),
                    sequence: 2,
                    capability_id: d.id().clone(),
                    arguments: classified_args(),
                    requested_by: RequestedBy::Scheduler,
                },
            ],
            at(31),
            &Context::new().view(),
        )
        .expect("complete plan and both bindings commit together");
    assert_eq!(bindings.len(), 2);
    assert!(
        bindings
            .iter()
            .all(|binding| binding.generation_id() == generation)
    );
    drop(engine);
    let reopened = temp.open();
    let task = reopened.load_task(&tid(1)).expect("full plan reopens");
    assert_eq!(task.steps.len(), 2);
    assert_eq!(task.steps[0].step.step_id, sid(1));
    assert_eq!(task.steps[1].step.step_id, sid(2));
    assert_eq!(task.plan_revision, 1);
    assert!(
        reopened
            .get_step_capability_binding(&tid(1), &sid(1))
            .unwrap()
            .is_some()
    );
    assert!(
        reopened
            .get_step_capability_binding(&tid(1), &sid(2))
            .unwrap()
            .is_some()
    );
    let db = rusqlite::Connection::open(&temp.0).unwrap();
    let current_plan_refs: i64 = db
        .query_row(
            "SELECT count(*) FROM task_blob_refs WHERE task_id=?1 AND role='PLAN'",
            [tid(1).as_str()],
            |row| row.get(0),
        )
        .unwrap();
    let history_refs: i64 = db
        .query_row(
            "SELECT count(*) FROM task_blob_refs WHERE task_id=?1 AND role='PLAN_REVISION'",
            [tid(1).as_str()],
            |row| row.get(0),
        )
        .unwrap();
    let history_rows: i64 = db
        .query_row(
            "SELECT count(*) FROM plan_revisions WHERE task_id=?1",
            [tid(1).as_str()],
            |row| row.get(0),
        )
        .unwrap();
    let current_plan: Vec<u8> = db
        .query_row(
            "SELECT b.content FROM task_blob_refs r JOIN blobs b ON b.digest=r.digest AND b.data_class_rank=r.data_class_rank WHERE r.task_id=?1 AND r.role='PLAN'",
            [tid(1).as_str()],
            |row| row.get(0),
        )
        .unwrap();
    let plan_json: serde_json::Value = serde_json::from_slice(&current_plan).unwrap();
    assert_eq!(
        plan_json["steps"][0]["input_json"],
        r#"{"calendar":"work"}"#
    );
    assert_eq!(
        plan_json["steps"][1]["input_json"],
        r#"{"calendar":"work"}"#
    );
    assert_eq!(current_plan_refs, 1);
    assert_eq!(history_refs, 1);
    assert_eq!(history_rows, 1);
    drop(db);

    // A normal revision uses the same full-plan writer, retains both bound
    // Steps and adds an ordinary Step. The current PLAN ref is replaced while
    // the first revision remains in history.
    let mut reopened_engine = TaskEngine::new(temp.open(), events.clone());
    reopened_engine
        .start_planning(tid(1), TaskState::Ready, 1, at(40), &Context::new().view())
        .unwrap();
    let before_revision = reopened_engine.load(tid(1)).unwrap();
    let notice_input = br#"{"notice":"retained"}"#.to_vec();
    let notice = TaskStep::new(TaskStepDraft {
        task_id: tid(1),
        step_id: sid(3),
        sequence: 3,
        kind: StepKind::Notify,
        status: StepStatus::new("PLANNED").unwrap(),
        attempt: 0,
        idempotency_key: None,
        provider_id: None,
        capability_id: None,
        capability_version: None,
        input_digest: digest_of(std::str::from_utf8(&notice_input).unwrap()).unwrap(),
        result_digest: None,
        side_effect_receipt: None,
        started_at: None,
        completed_at: None,
        lease_owner: None,
        lease_expires_at: None,
        lease_generation: None,
        error: None,
        extensions: [].into(),
    })
    .unwrap();
    let mut complete_plan: Vec<PlanStep> = before_revision
        .steps
        .iter()
        .map(|record| PlanStep {
            step: record.step.clone(),
            input_json: br#"{"calendar":"work"}"#.to_vec(),
        })
        .collect();
    complete_plan.push(PlanStep {
        step: notice,
        input_json: notice_input.clone(),
    });
    reopened_engine
        .persist_plan(
            tid(1),
            Plan {
                revision: 2,
                steps: complete_plan,
            },
            at(41),
            &Context::new().view(),
        )
        .unwrap();
    let after_revision = reopened_engine.load(tid(1)).unwrap();
    assert_eq!(after_revision.plan_revision, 2);
    assert_eq!(after_revision.steps.len(), 3);
    assert_eq!(after_revision.steps[0].step.step_id, sid(1));
    assert_eq!(after_revision.steps[1].step.step_id, sid(2));
    assert_eq!(after_revision.steps[2].step.step_id, sid(3));
    let db = rusqlite::Connection::open(&temp.0).unwrap();
    let current_plan_refs: i64 = db
        .query_row(
            "SELECT count(*) FROM task_blob_refs WHERE task_id=?1 AND role='PLAN'",
            [tid(1).as_str()],
            |row| row.get(0),
        )
        .unwrap();
    let history_refs: i64 = db
        .query_row(
            "SELECT count(*) FROM task_blob_refs WHERE task_id=?1 AND role='PLAN_REVISION'",
            [tid(1).as_str()],
            |row| row.get(0),
        )
        .unwrap();
    let history_rows: i64 = db
        .query_row(
            "SELECT count(*) FROM plan_revisions WHERE task_id=?1",
            [tid(1).as_str()],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(current_plan_refs, 1);
    assert_eq!(history_refs, 2);
    assert_eq!(history_rows, 2);
    drop(db);

    // Removing a bound Step through the ordinary full-plan lifecycle path is
    // refused while the owning Task exists; the binding trigger guards the
    // Step's FK cascade, and the plan transaction rolls back.
    reopened_engine
        .start_planning(tid(1), TaskState::Ready, 2, at(50), &Context::new().view())
        .unwrap();
    let retained = reopened_engine.load(tid(1)).unwrap();
    let removal = reopened_engine.persist_plan(
        tid(1),
        Plan {
            revision: 3,
            steps: vec![
                PlanStep {
                    step: retained.steps[0].step.clone(),
                    input_json: br#"{"calendar":"work"}"#.to_vec(),
                },
                PlanStep {
                    step: retained.steps[2].step.clone(),
                    input_json: notice_input,
                },
            ],
        },
        at(51),
        &Context::new().view(),
    );
    assert!(matches!(
        removal,
        Err(EngineError::Store(
            serea_storage::StoreError::ConstraintViolation
        ))
    ));
    let unchanged = reopened_engine.load(tid(1)).unwrap();
    assert_eq!(unchanged.plan_revision, 2);
    assert_eq!(unchanged.steps.len(), 3);
    assert!(
        reopened
            .get_step_capability_binding(&tid(1), &sid(2))
            .unwrap()
            .is_some()
    );
}

#[test]
fn journal_failure_rolls_back_capability_plan_and_binding() {
    let temp = TempDb::new();
    let store = temp.open();
    let events = event_bus();
    let d = descriptor();
    let manifest = manifest_for(vec![d.clone()]);
    install_manifest(
        &store,
        &events,
        &manifest,
        vec![provider(vec![d.clone()], ProviderHealth::Ready)],
    );
    let mut engine = engine(temp.open(), events.clone());
    engine.create_task(spec(1), &Context::new().view()).unwrap();
    engine
        .start_planning(
            tid(1),
            TaskState::Received,
            0,
            at(20),
            &Context::new().view(),
        )
        .unwrap();
    let snap = snapshot(
        &store,
        manifest,
        vec![provider(vec![d.clone()], ProviderHealth::Ready)],
        true,
    );
    let db = rusqlite::Connection::open(&temp.0).unwrap();
    db.execute_batch(
        "CREATE TRIGGER refuse_capability_plan_journal BEFORE INSERT ON task_journal
         BEGIN SELECT RAISE(ABORT, 'injected capability plan journal failure'); END;",
    )
    .unwrap();
    let result = engine.create_capability_plan(
        &snap,
        tid(1),
        vec![CapabilityPlanStep {
            step_id: sid(1),
            sequence: 1,
            capability_id: d.id().clone(),
            arguments: classified_args(),
            requested_by: RequestedBy::Model,
        }],
        at(30),
        &Context::new().view(),
    );
    assert!(
        result.is_err(),
        "journal participant fault must abort commit"
    );
    drop(engine);
    let reopened = temp.open();
    let task = reopened.load_task(&tid(1)).unwrap();
    assert_eq!(task.task.state, TaskState::Planning);
    assert!(task.steps.is_empty());
    assert!(
        reopened
            .get_step_capability_binding(&tid(1), &sid(1))
            .unwrap()
            .is_none()
    );
    let connection = rusqlite::Connection::open(&temp.0).unwrap();
    let plan_refs: i64 = connection
        .query_row(
            "SELECT count(*) FROM task_blob_refs WHERE task_id=?1 AND role='PLAN'",
            [tid(1).as_str()],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(plan_refs, 0);
}

#[test]
fn legacy_task_without_pin_cannot_create_a_capability_step() {
    let temp = TempDb::new();
    let store = temp.open();
    let events = event_bus();
    let d = descriptor();
    let manifest = manifest_for(vec![d.clone()]);
    install_manifest(
        &store,
        &events,
        &manifest,
        vec![provider(vec![d.clone()], ProviderHealth::Ready)],
    );
    // A genuinely pre-P5 Task row: created on a database that has never held a
    // registry generation, so it keeps the accepted legacy NULL pin.
    let legacy = TempDb::new();
    let legacy_store = legacy.open();
    let legacy_context = Context::new();
    legacy_store
        .transact_with_participants(&TaskJournal, &events, |tx| {
            tx.insert_task(&historical_task(1), &legacy_context.view())
        })
        .unwrap();
    assert_eq!(pinned_generation(&legacy_store, &tid(1)), None);
    let mut legacy_engine = TaskEngine::new(legacy.open(), events.clone());
    legacy_engine
        .start_planning(
            tid(1),
            TaskState::Received,
            0,
            at(20),
            &Context::new().view(),
        )
        .unwrap();
    let input_json = br#"{"notice":"legacy work remains permitted"}"#.to_vec();
    let ordinary_step = TaskStep::new(TaskStepDraft {
        task_id: tid(1),
        step_id: sid(1),
        sequence: 1,
        kind: StepKind::Notify,
        status: StepStatus::new("PLANNED").unwrap(),
        attempt: 0,
        idempotency_key: None,
        provider_id: None,
        capability_id: None,
        capability_version: None,
        input_digest: digest_of(std::str::from_utf8(&input_json).unwrap()).unwrap(),
        result_digest: None,
        side_effect_receipt: None,
        started_at: None,
        completed_at: None,
        lease_owner: None,
        lease_expires_at: None,
        lease_generation: None,
        error: None,
        extensions: [].into(),
    })
    .unwrap();
    legacy_engine
        .persist_plan(
            tid(1),
            Plan {
                revision: 1,
                steps: vec![PlanStep {
                    step: ordinary_step,
                    input_json,
                }],
            },
            at(25),
            &Context::new().view(),
        )
        .expect("legacy Task keeps permitted non-capability behavior");
    assert_eq!(
        legacy_engine.load(tid(1)).unwrap().task.state,
        TaskState::Ready
    );
    drop(legacy_engine);
    // The manifest exists only in the post-P5 database, so the legacy Task is
    // never pinned to it and the attempt must fail closed.
    let snap = snapshot(
        &store,
        manifest,
        vec![provider(vec![d], ProviderHealth::Ready)],
        true,
    );
    let mut legacy_engine = TaskEngine::new(legacy.open(), events.clone());
    let err = match legacy_engine.create_capability_step(
        &snap,
        tid(1),
        sid(1),
        1,
        &CapabilityId::new("calendar.events.read").unwrap(),
        &classified_args(),
        RequestedBy::Model,
        at(30),
        &Context::new().view(),
    ) {
        Ok(_) => panic!("legacy task must fail closed"),
        Err(error) => error,
    };
    assert!(
        matches!(
            err,
            EngineError::UnpinnedTaskCannotBindCapability
                | EngineError::NoActiveCapabilityGeneration
                | EngineError::CapabilityUnavailable { .. }
        ),
        "unexpected {err:?}"
    );
}

#[test]
fn disabled_before_binding_refuses_the_new_binding() {
    let temp = TempDb::new();
    let store = temp.open();
    let events = event_bus();
    let d = descriptor();
    let manifest = manifest_for(vec![d.clone()]);
    install_manifest(
        &store,
        &events,
        &manifest,
        vec![provider(vec![d.clone()], ProviderHealth::Ready)],
    );
    let id = d.id().clone();
    serea_capability::CapabilityRegistry::set_overlay(
        &store,
        &events,
        id.clone(),
        0,
        serea_capability::CapabilityOverlayState::Disabled,
        false,
        at(40),
    )
    .unwrap();
    let mut engine = engine(temp.open(), events.clone());
    engine.create_task(spec(1), &Context::new().view()).unwrap();
    engine
        .start_planning(
            tid(1),
            TaskState::Received,
            0,
            at(20),
            &Context::new().view(),
        )
        .unwrap();
    let snap = snapshot(
        &store,
        manifest,
        vec![provider(vec![d], ProviderHealth::Ready)],
        true,
    );
    let err = engine
        .create_capability_step(
            &snap,
            tid(1),
            sid(1),
            1,
            &id,
            &classified_args(),
            RequestedBy::Model,
            at(30),
            &Context::new().view(),
        )
        .expect_err("disabled capability must not bind");
    assert!(matches!(err, EngineError::CapabilityUnavailable { .. }));
}

#[test]
fn degraded_provider_before_binding_refuses_the_new_binding() {
    let temp = TempDb::new();
    let store = temp.open();
    let events = event_bus();
    let d = descriptor();
    let manifest = manifest_for(vec![d.clone()]);
    install_manifest(
        &store,
        &events,
        &manifest,
        vec![provider(vec![d.clone()], ProviderHealth::Ready)],
    );
    let mut engine = engine(temp.open(), events.clone());
    engine.create_task(spec(1), &Context::new().view()).unwrap();
    engine
        .start_planning(
            tid(1),
            TaskState::Received,
            0,
            at(20),
            &Context::new().view(),
        )
        .unwrap();
    let snap = snapshot(
        &store,
        manifest,
        vec![provider(vec![d], ProviderHealth::Degraded)],
        true,
    );
    let err = engine
        .create_capability_step(
            &snap,
            tid(1),
            sid(1),
            1,
            &CapabilityId::new("calendar.events.read").unwrap(),
            &classified_args(),
            RequestedBy::Model,
            at(30),
            &Context::new().view(),
        )
        .expect_err("degraded provider must not bind");
    assert!(matches!(err, EngineError::CapabilityUnavailable { .. }));
}

#[test]
fn existing_binding_survives_a_later_disable() {
    let temp = TempDb::new();
    let store = temp.open();
    let events = event_bus();
    let d = descriptor();
    let manifest = manifest_for(vec![d.clone()]);
    install_manifest(
        &store,
        &events,
        &manifest,
        vec![provider(vec![d.clone()], ProviderHealth::Ready)],
    );
    let id = d.id().clone();
    let mut engine = engine(temp.open(), events.clone());
    engine.create_task(spec(1), &Context::new().view()).unwrap();
    engine
        .start_planning(
            tid(1),
            TaskState::Received,
            0,
            at(20),
            &Context::new().view(),
        )
        .unwrap();
    let snap = snapshot(
        &store,
        manifest,
        vec![provider(vec![d.clone()], ProviderHealth::Ready)],
        true,
    );
    let bound = engine
        .create_capability_step(
            &snap,
            tid(1),
            sid(1),
            1,
            &id,
            &classified_args(),
            RequestedBy::Model,
            at(30),
            &Context::new().view(),
        )
        .unwrap();
    // disable and remove after binding: the binding is immutable
    serea_capability::CapabilityRegistry::set_overlay(
        &store,
        &events,
        id.clone(),
        0,
        serea_capability::CapabilityOverlayState::Removed,
        false,
        at(60),
    )
    .unwrap();
    let read = store
        .get_step_capability_binding(&tid(1), &sid(1))
        .unwrap()
        .unwrap();
    assert_eq!(read.descriptor_digest(), bound.descriptor_digest());
    assert_eq!(read.generation_id(), bound.generation_id());
}

#[test]
fn hot_update_keeps_old_tasks_on_their_own_generation() {
    let temp = TempDb::new();
    let store = temp.open();
    let events = event_bus();
    let d = descriptor();
    let first = manifest_for(vec![d.clone()]);
    let first_generation = install_manifest(
        &store,
        &events,
        &first,
        vec![provider(vec![d.clone()], ProviderHealth::Ready)],
    );
    let mut engine = engine(temp.open(), events.clone());
    engine.create_task(spec(1), &Context::new().view()).unwrap();
    engine
        .start_planning(
            tid(1),
            TaskState::Received,
            0,
            at(20),
            &Context::new().view(),
        )
        .unwrap();

    // a genuinely different manifest activates a second generation mid-flight
    let mut next_descriptor = descriptor();
    let second_draft = CapabilityDescriptorDraft::from(next_descriptor.clone());
    let mut second_draft = second_draft;
    second_draft.title = DescriptorTitle::new("Read events v2").unwrap();
    next_descriptor = CapabilityDescriptor::new(second_draft).unwrap();
    let second = manifest_for(vec![next_descriptor.clone()]);
    let second_generation = install_manifest(
        &store,
        &events,
        &second,
        vec![provider(vec![next_descriptor], ProviderHealth::Ready)],
    );
    assert_ne!(first_generation, second_generation);

    // a Task created before the update stays on the old generation
    assert_eq!(pinned_generation(&store, &tid(1)), Some(first_generation));
    // a Task created after the update uses the new one
    engine.create_task(spec(2), &Context::new().view()).unwrap();
    assert_eq!(pinned_generation(&store, &tid(2)), Some(second_generation));
}

#[test]
fn duplicate_step_binding_is_refused_and_never_rebound() {
    let temp = TempDb::new();
    let store = temp.open();
    let events = event_bus();
    let d = descriptor();
    let manifest = manifest_for(vec![d.clone()]);
    install_manifest(
        &store,
        &events,
        &manifest,
        vec![provider(vec![d.clone()], ProviderHealth::Ready)],
    );
    let mut engine = engine(temp.open(), events.clone());
    engine.create_task(spec(1), &Context::new().view()).unwrap();
    engine
        .start_planning(
            tid(1),
            TaskState::Received,
            0,
            at(20),
            &Context::new().view(),
        )
        .unwrap();
    let snap = snapshot(
        &store,
        manifest,
        vec![provider(vec![d.clone()], ProviderHealth::Ready)],
        true,
    );
    let first = engine
        .create_capability_step(
            &snap,
            tid(1),
            sid(1),
            1,
            &d.id().clone(),
            &classified_args(),
            RequestedBy::Model,
            at(30),
            &Context::new().view(),
        )
        .unwrap();
    // a second attempt to create the same step must not silently rebind it
    let err = engine.create_capability_step(
        &snap,
        tid(1),
        sid(1),
        1,
        &d.id().clone(),
        &classified_args(),
        RequestedBy::Model,
        at(31),
        &Context::new().view(),
    );
    assert!(err.is_err(), "a bound step must never be rebound");
    let read = store
        .get_step_capability_binding(&tid(1), &sid(1))
        .unwrap()
        .unwrap();
    assert_eq!(read.descriptor_digest(), first.descriptor_digest());
}

#[test]
fn p5e_never_invokes_a_provider() {
    // Every provider in this file panics if invoked; creating tasks, planning,
    // binding and reading back must all complete with zero invocations.
    let temp = TempDb::new();
    let store = temp.open();
    let events = event_bus();
    let d = descriptor();
    let manifest = manifest_for(vec![d.clone()]);
    install_manifest(
        &store,
        &events,
        &manifest,
        vec![provider(vec![d.clone()], ProviderHealth::Ready)],
    );
    let mut engine = engine(temp.open(), events.clone());
    engine.create_task(spec(1), &Context::new().view()).unwrap();
    engine
        .start_planning(
            tid(1),
            TaskState::Received,
            0,
            at(20),
            &Context::new().view(),
        )
        .unwrap();
    let snap = snapshot(
        &store,
        manifest,
        vec![provider(vec![d], ProviderHealth::Ready)],
        true,
    );
    engine
        .create_capability_step(
            &snap,
            tid(1),
            sid(1),
            1,
            &CapabilityId::new("calendar.events.read").unwrap(),
            &classified_args(),
            RequestedBy::Model,
            at(30),
            &Context::new().view(),
        )
        .unwrap();
    let _ = engine.load(tid(1)).unwrap();
    // Reaching here without a panic is the zero-invoke proof.
}
