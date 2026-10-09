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
        data_class: DataClass::Personal,
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
        data_class: DataClass::Personal,
        root_requirement: RootRequirement::NotRequired,
        idempotency_support: IdempotencySupport::None,
        max_duration_ms: 5_000,
        cost_class: CostClass::Free,
        experimental: false,
    })
    .unwrap()
}

fn manifest_for(descriptors: Vec<CapabilityDescriptor>) -> CapabilityManifestV1 {
    let catalog = catalog();
    let entries: Vec<ManifestEntryV1> = descriptors
        .iter()
        .map(|d| ManifestEntryV1 {
            descriptor: d.clone(),
            candidate_priority: 0,
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
    let defaults = descriptors
        .iter()
        .map(|d| (d.id().clone(), d.version().clone()))
        .collect();
    CapabilityManifestV1::build(entries, defaults, catalog).unwrap()
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
    Arc::new(TestProvider {
        id: ProviderId::new("calendar").unwrap(),
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
    let registry = ProviderRegistry::build(providers).unwrap();
    let mut eligibility = HashMap::new();
    for entry in manifest.entries() {
        eligibility.insert(entry.descriptor_digest.clone(), eligible);
    }
    CapabilityAvailabilitySnapshotV1::build(
        manifest,
        &registry,
        store,
        HostEligibility::new(eligibility),
    )
    .unwrap()
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
fn task_creation_without_active_generation_fails_closed() {
    let temp = TempDb::new();
    let events = event_bus();
    let mut engine = engine(temp.open(), events);
    // No registry generation exists yet: a post-P5 task cannot be created
    // unpinned, and it is never silently attached to "current" later.
    let err =
        match engine.create_task_with_capability_pinning(spec(1), None, &Context::new().view()) {
            Ok(_) => panic!("must fail closed without an active generation"),
            Err(error) => error,
        };
    assert!(matches!(
        err,
        EngineError::NoActiveCapabilityGeneration | EngineError::Store(_)
    ));
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
        r#"{"calendar":"work"}"#.as_bytes(),
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
    let mut legacy_engine = TaskEngine::new(legacy.open(), events.clone());
    legacy_engine
        .create_task(spec(1), &Context::new().view())
        .unwrap();
    assert_eq!(pinned_generation(&legacy_store, &tid(1)), None);
    legacy_engine
        .start_planning(
            tid(1),
            TaskState::Received,
            0,
            at(20),
            &Context::new().view(),
        )
        .unwrap();
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
        r#"{"calendar":"work"}"#.as_bytes(),
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
            r#"{"calendar":"work"}"#.as_bytes(),
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
            r#"{"calendar":"work"}"#.as_bytes(),
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
            r#"{"calendar":"work"}"#.as_bytes(),
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
            r#"{"calendar":"work"}"#.as_bytes(),
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
        r#"{"calendar":"other"}"#.as_bytes(),
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
            r#"{"calendar":"work"}"#.as_bytes(),
            at(30),
            &Context::new().view(),
        )
        .unwrap();
    let _ = engine.load(tid(1)).unwrap();
    // Reaching here without a panic is the zero-invoke proof.
}
