//! P5F crash and fault coverage around the P5 durable surfaces.
//!
//! Deterministic SQLite transaction failures are injected at the storage fault
//! seam. No hardware power-loss claim is made.
#[path = "common/mod.rs"]
mod common;

use std::collections::BTreeMap;
use std::sync::Arc;

use serea_capability::{
    CapabilityManifestV1, CapabilitySchemaCatalogV1, HostEligibility, ManifestEntryV1,
    ProviderRegistry, descriptor_semantic_digest, install,
};
use serea_event_bus::EventBus;
use serea_protocol::provider::CapabilityProvider;
use serea_protocol::*;
use serea_storage::Store;
use serea_storage::fault::{Action, Window};
use serea_testkit::DeterministicUlidSource;

const PREFIX: &str = "https://serea.local/schemas/";

static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

struct Fixed;
impl Clock for Fixed {
    fn now_ms(&self) -> Result<EpochMillis, ProtocolError> {
        Ok(EpochMillis::new(1_796_000_000_000).unwrap())
    }
}

fn at(n: i64) -> EpochMillis {
    EpochMillis::new(n).unwrap()
}

fn event_bus() -> EventBus {
    let offset = NEXT.fetch_add(1_000, std::sync::atomic::Ordering::Relaxed);
    let process = u64::from(std::process::id());
    EventBus::new(
        DeterministicUlidSource::starting_at(1_700_000_000_000 + process * 10_000 + offset)
            .unwrap(),
    )
}

struct TempDb(std::path::PathBuf);

impl TempDb {
    fn new() -> Self {
        let dir = std::env::temp_dir().join(format!(
            "serea-p5f-fault-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
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
    descriptors: Vec<CapabilityDescriptor>,
}

#[async_trait::async_trait]
impl CapabilityProvider for TestProvider {
    fn provider_id(&self) -> ProviderId {
        ProviderId::new("calendar").unwrap()
    }
    fn capabilities(&self) -> Vec<CapabilityDescriptor> {
        self.descriptors.clone()
    }
    async fn invoke(
        &self,
        _request: &ActionRequest,
        _ctx: &serea_protocol::provider::ProviderContext,
    ) -> Result<ActionResult, ActionError> {
        panic!("P5 must never invoke a provider")
    }
    async fn health(&self) -> ProviderHealth {
        ProviderHealth::Ready
    }
}

fn provider(descriptors: Vec<CapabilityDescriptor>) -> Arc<dyn CapabilityProvider> {
    Arc::new(TestProvider { descriptors })
}

fn install_manifest(
    store: &Store,
    events: &EventBus,
    manifest: &CapabilityManifestV1,
    providers: Vec<Arc<dyn CapabilityProvider>>,
) -> Result<i64, String> {
    let registry = ProviderRegistry::build(providers).map_err(|e| format!("{e:?}"))?;
    install(store, events, manifest, &registry, at(50))
        .map(|outcome| outcome.generation_id())
        .map_err(|error| format!("{error:?}"))
}

#[test]
fn activation_commit_failure_leaves_the_previous_generation_authoritative() {
    let temp = TempDb::new();
    let d = descriptor();
    let first = manifest_for(vec![d.clone()]);
    let store = temp.open();
    let events = event_bus();
    let first_id = install_manifest(&store, &events, &first, vec![provider(vec![d.clone()])])
        .expect("first install");

    // Inject a deterministic failure at the commit of the *second* generation.
    let mut next = CapabilityDescriptorDraft::from(d.clone());
    next.title = DescriptorTitle::new("Read events v2").unwrap();
    let changed = CapabilityDescriptor::new(next).unwrap();
    let second = manifest_for(vec![changed.clone()]);
    Window::BeforeCommit
        .arm_after(
            2,
            Action::Fail(serea_storage::StoreError::EventHistoryCorrupt),
        )
        .expect("armed");
    let outcome = install_manifest(&store, &events, &second, vec![provider(vec![changed])]);
    assert!(outcome.is_err(), "an injected commit failure must surface");

    // The previously activated generation is still the authority.
    drop(store);
    let reopened = temp.open();
    let current = serea_capability::CapabilityRegistry::current_generation(&reopened)
        .unwrap()
        .unwrap();
    assert_eq!(current.generation_id(), first_id);
    assert_eq!(current.manifest_digest(), first.digest());
}

#[test]
fn caller_lost_after_activation_reopens_to_the_activated_generation() {
    let temp = TempDb::new();
    let d = descriptor();
    let manifest = manifest_for(vec![d.clone()]);
    let events = event_bus();
    let generation = {
        let store = temp.open();
        install_manifest(&store, &events, &manifest, vec![provider(vec![d.clone()])])
            .expect("install")
    };
    // The caller lost its response; a fresh process reopens the same state.
    let reopened = temp.open();
    let current = serea_capability::CapabilityRegistry::current_generation(&reopened)
        .unwrap()
        .unwrap();
    assert_eq!(current.generation_id(), generation);
    assert_eq!(current.manifest_digest(), manifest.digest());
    assert_eq!(current.schema_catalog_digest(), manifest.catalog_digest());
    let members =
        serea_capability::CapabilityRegistry::generation_members(&reopened, generation).unwrap();
    assert_eq!(members.len(), 1);
}

#[test]
fn injected_failure_before_preparation_writes_nothing() {
    let temp = TempDb::new();
    let d = descriptor();
    let manifest = manifest_for(vec![d.clone()]);
    let store = temp.open();
    let events = event_bus();
    Window::BeforeBegin
        .arm(Action::Fail(serea_storage::StoreError::EventHistoryCorrupt))
        .expect("armed");
    let outcome = install_manifest(&store, &events, &manifest, vec![provider(vec![d])]);
    assert!(outcome.is_err());
    drop(store);
    let reopened = temp.open();
    assert!(
        serea_capability::CapabilityRegistry::current_generation(&reopened)
            .unwrap()
            .is_none(),
        "a failed preparation must leave no generation behind"
    );
}

#[test]
fn availability_snapshot_is_independent_of_provider_health_timing() {
    let temp = TempDb::new();
    let d = descriptor();
    let manifest = manifest_for(vec![d.clone()]);
    let store = temp.open();
    let events = event_bus();
    install_manifest(&store, &events, &manifest, vec![provider(vec![d.clone()])]).unwrap();
    let registry = ProviderRegistry::build(vec![provider(vec![d.clone()])]).unwrap();
    let mut eligibility = std::collections::HashMap::new();
    eligibility.insert(
        descriptor_semantic_digest(&descriptor(), &catalog()).unwrap(),
        true,
    );
    // Two snapshots over identical facts agree, so health sampling timing is not
    // an authority input.
    let a = common::build_snapshot(
        manifest.clone(),
        &registry,
        &store,
        HostEligibility::new(eligibility.clone()),
    )
    .unwrap();
    let b = common::build_snapshot(
        manifest,
        &registry,
        &store,
        HostEligibility::new(eligibility),
    )
    .unwrap();
    assert_eq!(
        a.resolve(d.id()).unwrap().descriptor_digest,
        b.resolve(d.id()).unwrap().descriptor_digest
    );
}
