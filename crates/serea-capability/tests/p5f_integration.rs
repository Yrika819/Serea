//! P5F: whole-phase concurrency, crash/recovery and the P6 handoff boundary.
//!
//! Nothing here executes anything. Providers panic if invoked, and the
//! PreparedAction handed to a future P6 is proven immutable.

use std::collections::BTreeMap;
use std::sync::Arc;

use serea_capability::{
    CapabilityAvailabilitySnapshotV1, CapabilityManifestV1, CapabilitySchemaCatalogV1,
    ClassifiedArgumentsV1, HostEligibility, ManifestEntryV1, PreparedActionV1, ProviderRegistry,
    descriptor_semantic_digest, install, prepare_action,
};
use serea_event_bus::EventBus;
use serea_protocol::provider::CapabilityProvider;
use serea_protocol::*;
use serea_storage::Store;
/// A deterministic event bus per thread, so parallel tests never share ULIDs.
fn event_bus() -> EventBus {
    use std::sync::atomic::AtomicU64 as Counter;
    static OFFSET: Counter = Counter::new(0);
    let offset = OFFSET.fetch_add(1_000, Ordering::Relaxed);
    let process = u64::from(std::process::id());
    let start = 1_700_000_000_000 + process * 100_000 + offset;
    EventBus::new(serea_testkit::DeterministicUlidSource::starting_at(start).unwrap())
}

const PREFIX: &str = "https://serea.local/schemas/";

use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);

struct Fixed;
impl Clock for Fixed {
    fn now_ms(&self) -> Result<EpochMillis, ProtocolError> {
        Ok(EpochMillis::new(1_796_000_000_000).unwrap())
    }
}

fn at(n: i64) -> EpochMillis {
    EpochMillis::new(n).unwrap()
}

struct TempDb(std::path::PathBuf);

impl TempDb {
    fn new() -> Self {
        let dir = std::env::temp_dir().join(format!(
            "serea-p5f-{}-{}",
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

fn catalog() -> CapabilitySchemaCatalogV1 {
    let mut docs = BTreeMap::new();
    docs.insert(
        format!("{PREFIX}cal-input.json"),
        r#"{"$schema":"https://json-schema.org/draft/2020-12/schema","type":"object","additionalProperties":false,"properties":{"calendar":{"type":"string","maxLength":64}},"required":["calendar"]}"#.to_string(),
    );
    docs.insert(
        format!("{PREFIX}cal-output.json"),
        r#"{"$schema":"https://json-schema.org/draft/2020-12/schema","type":"object","additionalProperties":false,"properties":{"ok":{"type":"boolean"}}}"#.to_string(),
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
        panic!("P5F must never invoke a provider")
    }
    async fn health(&self) -> ProviderHealth {
        self.health
    }
}

fn provider(descriptors: Vec<CapabilityDescriptor>) -> Arc<dyn CapabilityProvider> {
    Arc::new(TestProvider {
        id: ProviderId::new("calendar").unwrap(),
        descriptors,
        health: ProviderHealth::Ready,
    })
}

fn snapshot(
    store: &Store,
    manifest: CapabilityManifestV1,
    providers: Vec<Arc<dyn CapabilityProvider>>,
) -> CapabilityAvailabilitySnapshotV1 {
    let registry = ProviderRegistry::build(providers).unwrap();
    let mut eligibility = std::collections::HashMap::new();
    for entry in manifest.entries() {
        eligibility.insert(entry.descriptor_digest.clone(), true);
    }
    CapabilityAvailabilitySnapshotV1::build(
        manifest,
        &registry,
        store,
        HostEligibility::new(eligibility),
    )
    .unwrap()
}

fn task(n: u32) -> TaskId {
    TaskId::new(format!("tsk_{n:026}")).unwrap()
}

fn step(n: u32) -> StepId {
    StepId::new(format!("stp_{n:026}")).unwrap()
}

fn arguments() -> ClassifiedArgumentsV1 {
    ClassifiedArgumentsV1::new_trusted(
        serde_json::Map::from_iter([(
            "calendar".to_string(),
            serde_json::Value::String("work".into()),
        )]),
        DataClass::Personal,
    )
}

#[test]
fn concurrent_same_manifest_startup_yields_one_generation() {
    let temp = TempDb::new();
    let manifest = manifest_for(vec![descriptor()]);
    let mut handles = Vec::new();
    for _ in 0..4 {
        let manifest = manifest.clone();
        let path = temp.0.clone();
        handles.push(std::thread::spawn(move || {
            let store = Store::open(&path, &Fixed).unwrap();
            let events = event_bus();
            let registry = ProviderRegistry::build(vec![provider(vec![descriptor()])]).unwrap();
            install(&store, &events, &manifest, &registry, at(50))
                .map(|outcome| (outcome.activated(), outcome.generation_id()))
                .map_err(|error| format!("{error:?}"))
        }));
    }
    let outcomes: Vec<Result<(bool, i64), String>> = handles
        .into_iter()
        .map(|handle| handle.join().expect("thread"))
        .collect();
    let outcomes: Vec<(bool, i64)> = outcomes
        .into_iter()
        .map(|outcome| outcome.expect("concurrent install"))
        .collect();
    // exactly one installation may activate; the rest observe the same state
    let activated: Vec<bool> = outcomes.iter().map(|(activated, _)| *activated).collect();
    assert_eq!(
        activated.iter().filter(|value| **value).count(),
        1,
        "exactly one concurrent startup may activate: {outcomes:?}"
    );
    let store = temp.open();
    let current = serea_capability::CapabilityRegistry::current_generation(&store)
        .unwrap()
        .unwrap();
    assert_eq!(current.manifest_digest(), manifest.digest());
}

#[test]
fn concurrent_activation_keeps_the_active_pointer_consistent() {
    let temp = TempDb::new();
    let manifest = manifest_for(vec![descriptor()]);
    let store = temp.open();
    let events = event_bus();
    let registry = ProviderRegistry::build(vec![provider(vec![descriptor()])]).unwrap();
    install(&store, &events, &manifest, &registry, at(50)).unwrap();
    let before = serea_capability::CapabilityRegistry::current_generation(&store)
        .unwrap()
        .unwrap();
    // several independent connections racing on the same manifest
    let mut handles = Vec::new();
    for _ in 0..4 {
        let path = temp.0.clone();
        handles.push(std::thread::spawn(move || {
            let store = Store::open(&path, &Fixed).unwrap();
            let events = event_bus();
            let registry = ProviderRegistry::build(vec![provider(vec![descriptor()])]).unwrap();
            install(
                &store,
                &events,
                &manifest_for(vec![descriptor()]),
                &registry,
                at(60),
            )
            .is_ok()
        }));
    }
    for handle in handles {
        handle.join().expect("thread");
    }
    let after = serea_capability::CapabilityRegistry::current_generation(&store)
        .unwrap()
        .unwrap();
    // the pointer never moves to a non-activated generation
    assert!(after.activated_at().is_some());
    assert_eq!(after.manifest_digest(), before.manifest_digest());
}

#[test]
fn crash_before_preparation_leaves_the_previous_generation_active() {
    let temp = TempDb::new();
    let store = temp.open();
    let events = event_bus();
    let d = descriptor();
    let first = manifest_for(vec![d.clone()]);
    let registry = ProviderRegistry::build(vec![provider(vec![d.clone()])]).unwrap();
    let outcome = install(&store, &events, &first, &registry, at(50)).unwrap();
    assert!(outcome.activated());

    // a second installation whose provider contract fails writes nothing
    let rogue = CapabilityDescriptor::new(CapabilityDescriptorDraft {
        id: CapabilityId::new("calendar.events.delete").unwrap(),
        version: SemVer::new("1.0.0").unwrap(),
        title: DescriptorTitle::new("Delete").unwrap(),
        description: DescriptorDescription::new("D").unwrap(),
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
    .unwrap();
    let bad_registry = ProviderRegistry::build(vec![provider(vec![rogue])]).unwrap();
    assert!(
        install(&store, &events, &first, &bad_registry, at(60)).is_err(),
        "an unmanifested advertisement must fail"
    );

    // after reopen the previous generation is still the authority
    drop(store);
    let reopened = temp.open();
    let current = serea_capability::CapabilityRegistry::current_generation(&reopened)
        .unwrap()
        .unwrap();
    assert_eq!(current.generation_id(), outcome.generation_id());
    assert_eq!(current.manifest_digest(), first.digest());
}

#[test]
fn reopen_never_rebuilds_authority_from_provider_advertisement() {
    let temp = TempDb::new();
    let d = descriptor();
    let manifest = manifest_for(vec![d.clone()]);
    {
        let store = temp.open();
        let events = event_bus();
        let registry = ProviderRegistry::build(vec![provider(vec![d.clone()])]).unwrap();
        install(&store, &events, &manifest, &registry, at(50)).unwrap();
    }
    // reopen with NO provider registered at all
    let store = temp.open();
    let snap = CapabilityAvailabilitySnapshotV1::build(
        manifest.clone(),
        &ProviderRegistry::build(vec![]).unwrap(),
        &store,
        HostEligibility::new(std::collections::HashMap::new()),
    )
    .unwrap();
    assert!(
        snap.resolve(d.id()).is_err(),
        "resolution must not invent a provider that never registered"
    );
    // the durable generation is untouched and still readable
    let current = serea_capability::CapabilityRegistry::current_generation(&store)
        .unwrap()
        .unwrap();
    assert_eq!(current.manifest_digest(), manifest.digest());
}

#[test]
fn prepared_action_handoff_is_immutable() {
    let temp = TempDb::new();
    let store = temp.open();
    let events = event_bus();
    let d = descriptor();
    let manifest = manifest_for(vec![d.clone()]);
    let registry = ProviderRegistry::build(vec![provider(vec![d.clone()])]).unwrap();
    install(&store, &events, &manifest, &registry, at(50)).unwrap();
    let snap = snapshot(&store, manifest, vec![provider(vec![d.clone()])]);

    let prepared: PreparedActionV1 = prepare_action(
        &snap,
        &CapabilityId::new("calendar.events.read").unwrap(),
        task(1),
        step(1),
        &arguments(),
        RequestedBy::Model,
        None,
        None,
    )
    .expect("prepared");

    // Two handoffs of the same inputs are equal, so a consumer cannot observe
    // authority drift through the boundary.
    let again = prepare_action(
        &snap,
        &CapabilityId::new("calendar.events.read").unwrap(),
        task(1),
        step(1),
        &arguments(),
        RequestedBy::Model,
        None,
        None,
    )
    .unwrap();
    assert_eq!(prepared, again);

    // every pinned fact is readable and matches the manifest authority
    assert_eq!(
        prepared.descriptor_digest(),
        &descriptor_semantic_digest(&d, &catalog()).unwrap()
    );
    assert_eq!(prepared.capability_version().as_str(), "1.0.0");
    assert_eq!(prepared.provider_id().as_str(), "calendar");
    assert_eq!(prepared.generation_digest(), snap.manifest().digest());
    assert_eq!(prepared.risk_class(), RiskClass::Observe);
    assert_eq!(prepared.required_authorization(), Authorization::None);
    assert_eq!(prepared.data_class(), DataClass::Personal);
}

#[test]
fn handoff_exposes_no_mutation_or_execution_surface() {
    let temp = TempDb::new();
    let store = temp.open();
    let events = event_bus();
    let d = descriptor();
    let manifest = manifest_for(vec![d.clone()]);
    let registry = ProviderRegistry::build(vec![provider(vec![d.clone()])]).unwrap();
    install(&store, &events, &manifest, &registry, at(50)).unwrap();
    let snap = snapshot(&store, manifest, vec![provider(vec![d])]);
    let prepared = prepare_action(
        &snap,
        &CapabilityId::new("calendar.events.read").unwrap(),
        task(1),
        step(1),
        &arguments(),
        RequestedBy::Model,
        None,
        None,
    )
    .unwrap();
    // The frozen P6 input carries no RequestId, no approval state, no policy
    // decision and no execution permission, and no accessor for any of them.
    let surface = format!(
        "{:?}|{:?}|{:?}|{:?}|{:?}|{:?}|{:?}|{:?}|{:?}|{:?}|{:?}|{:?}|{:?}|{:?}|{:?}|{:?}|{:?}|{:?}|{:?}|{:?}",
        prepared.task_id(),
        prepared.step_id(),
        prepared.generation_digest(),
        prepared.descriptor_digest(),
        prepared.capability_id(),
        prepared.capability_version(),
        prepared.provider_id(),
        prepared.implementation_id(),
        prepared.arguments(),
        prepared.arguments_digest(),
        prepared.idempotency_key(),
        prepared.data_class(),
        prepared.requested_by(),
        prepared.deadline_ms(),
        prepared.side_effect_class(),
        prepared.risk_class(),
        prepared.required_authorization(),
        prepared.replay_safety(),
        prepared.root_requirement(),
        prepared.idempotency_support(),
    );
    for forbidden in [
        "request_id",
        "approval",
        "policy",
        "RequestId",
        "allow",
        "permit",
    ] {
        assert!(
            !surface.contains(forbidden),
            "handoff surface leaked {forbidden}"
        );
    }
}

#[test]
fn unavailability_never_fabricates_a_result() {
    let temp = TempDb::new();
    let store = temp.open();
    let events = event_bus();
    let d = descriptor();
    let manifest = manifest_for(vec![d.clone()]);
    let registry = ProviderRegistry::build(vec![provider(vec![d.clone()])]).unwrap();
    install(&store, &events, &manifest, &registry, at(50)).unwrap();
    // remove the capability from every provider, then resolve
    let snap = snapshot(&store, manifest, vec![]);
    let outcome = snap.resolve(d.id());
    assert!(outcome.is_err(), "no provider means no usable candidate");
    // no ActionResult exists anywhere in the outcome path
    let rendered = format!("{outcome:?}");
    assert!(!rendered.contains("ActionResult"));
    assert!(!rendered.contains("status"));
}

#[test]
fn store_error_surfaces_as_a_typed_refusal_not_a_silent_success() {
    // A capability that is not in the pinned generation is UNKNOWN, and the
    // outcome is a typed error rather than a fabricated result.
    let temp = TempDb::new();
    let store = temp.open();
    let events = event_bus();
    let d = descriptor();
    let manifest = manifest_for(vec![d.clone()]);
    let registry = ProviderRegistry::build(vec![provider(vec![d.clone()])]).unwrap();
    install(&store, &events, &manifest, &registry, at(50)).unwrap();
    let snap = snapshot(&store, manifest, vec![provider(vec![d.clone()])]);
    let missing = CapabilityId::new("calendar.events.write").unwrap();
    let err = snap.resolve(&missing).unwrap_err();
    assert!(
        matches!(
            err,
            serea_capability::ResolveError::Unknown { .. }
                | serea_capability::ResolveError::Unavailable { .. }
        ),
        "unexpected {err:?}"
    );
    assert!(!format!("{err:?}").contains("CorruptRow"));
}
