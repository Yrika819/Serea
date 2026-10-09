use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use serea_capability::{
    CapabilityManifestV1, CapabilitySchemaCatalogV1, ManifestEntryV1, ProviderRegistry,
    descriptor_semantic_digest,
};
use serea_event_bus::{EventBus, ReplayItem};
use serea_protocol::{
    Authorization, CapabilityDescriptor, CapabilityDescriptorDraft, CapabilityId, CostClass,
    DataClass, DescriptorDescription, DescriptorTitle, EpochMillis, EventKind, IdempotencySupport,
    JsonSchemaRef, ProviderId, ReplaySafety, RiskClass, RootRequirement, SemVer, SideEffectClass,
};
use serea_storage::Store;
use serea_testkit::DeterministicUlidSource;

const PREFIX: &str = "https://serea.local/schemas/";

static NEXT: AtomicU64 = AtomicU64::new(0);

struct FixedClock;
impl serea_protocol::Clock for FixedClock {
    fn now_ms(&self) -> Result<serea_protocol::EpochMillis, serea_protocol::ProtocolError> {
        serea_protocol::EpochMillis::new(1_796_000_000_000)
    }
}

fn store() -> Store {
    Store::open_in_memory(&FixedClock).unwrap()
}

fn events() -> EventBus {
    EventBus::new(DeterministicUlidSource::new())
}

fn catalog() -> CapabilitySchemaCatalogV1 {
    let mut docs = BTreeMap::new();
    docs.insert(
        format!("{PREFIX}input.json"),
        r#"{"$schema":"https://json-schema.org/draft/2020-12/schema","type":"object","additionalProperties":false,"properties":{"name":{"type":"string","maxLength":32}},"required":["name"]}"#.to_string(),
    );
    docs.insert(
        format!("{PREFIX}output.json"),
        r#"{"$schema":"https://json-schema.org/draft/2020-12/schema","type":"object","additionalProperties":false,"properties":{"ok":{"type":"boolean"}}}"#.to_string(),
    );
    CapabilitySchemaCatalogV1::build(docs).unwrap()
}

fn descriptor(id: &str, version: &str, provider: &str) -> CapabilityDescriptor {
    CapabilityDescriptor::new(CapabilityDescriptorDraft {
        id: CapabilityId::new(id).unwrap(),
        version: SemVer::new(version).unwrap(),
        title: DescriptorTitle::new("T").unwrap(),
        description: DescriptorDescription::new("D").unwrap(),
        provider_id: ProviderId::new(provider).unwrap(),
        implementation_id: None,
        input_schema: JsonSchemaRef::new(format!("{PREFIX}input.json")).unwrap(),
        output_schema: JsonSchemaRef::new(format!("{PREFIX}output.json")).unwrap(),
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

fn entry(
    d: &CapabilityDescriptor,
    priority: u32,
    catalog: &CapabilitySchemaCatalogV1,
) -> ManifestEntryV1 {
    ManifestEntryV1 {
        descriptor: d.clone(),
        candidate_priority: priority,
        descriptor_digest: descriptor_semantic_digest(d, catalog).unwrap(),
        input_schema_digest: catalog
            .document_digest(d.input_schema().as_str())
            .unwrap()
            .clone(),
        output_schema_digest: catalog
            .document_digest(d.output_schema().as_str())
            .unwrap()
            .clone(),
    }
}

fn manifest() -> (CapabilityManifestV1, CapabilitySchemaCatalogV1) {
    let c = catalog();
    let d = descriptor("calendar.events.read", "1.0.0", "calendar");
    let m = CapabilityManifestV1::build(
        vec![entry(&d, 0, &c)],
        vec![(d.id().clone(), d.version().clone())],
        c.clone(),
    )
    .unwrap();
    (m, c)
}

fn provider_registry_for() -> ProviderRegistry {
    let d = descriptor("calendar.events.read", "1.0.0", "calendar");
    let p = Arc::new(serea_testkit::MockCapabilityProvider::new(
        ProviderId::new("calendar").unwrap(),
        vec![d],
    ));
    ProviderRegistry::build(vec![p]).unwrap()
}

fn registry_change_count(store: &Store, events: &EventBus) -> usize {
    let _ = events;
    let page = EventBus::replay(store, None, None, 256).unwrap();
    page.items
        .iter()
        .filter(|item| {
            matches!(
                item,
                ReplayItem::Event { event } if event.kind == EventKind::CapabilityRegistryChanged
            )
        })
        .count()
}

#[test]
fn reopen_preserves_generation_facts() {
    let (m, _c) = manifest();
    let registry = provider_registry_for();
    let dir = std::env::temp_dir().join(format!(
        "serea-p5c-reopen-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("store.sqlite");
    let epoch = EpochMillis::new(1_796_000_000_000).unwrap();
    let first_id = {
        let store = Store::open(&path, &FixedClock).unwrap();
        let events = events();
        let outcome = serea_capability::install(&store, &events, &m, &registry, epoch).unwrap();
        assert!(outcome.activated());
        outcome.generation_id()
    };
    let reopened = Store::open(&path, &FixedClock).unwrap();
    let current = serea_capability::CapabilityRegistry::current_generation(&reopened)
        .unwrap()
        .unwrap();
    assert_eq!(current.generation_id(), first_id);
    assert_eq!(current.manifest_digest(), m.digest());
    assert_eq!(current.schema_catalog_digest(), m.catalog_digest());
    let members =
        serea_capability::CapabilityRegistry::generation_members(&reopened, first_id).unwrap();
    assert_eq!(members.len(), 1);
    assert_eq!(members[0].capability_id(), m.entries()[0].descriptor.id());
    let version = serea_capability::CapabilityRegistry::default_version(
        &reopened,
        first_id,
        m.entries()[0].descriptor.id(),
    )
    .unwrap();
    assert_eq!(
        version.expect("default version is pinned").as_str(),
        "1.0.0"
    );
    drop(reopened);
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn corrupt_member_facts_fail_closed() {
    let (m, c) = manifest();
    let registry = provider_registry_for();
    let store = store();
    let events = events();
    let epoch = EpochMillis::new(1_796_000_000_000).unwrap();
    // A complete, activatable generation that claims the validated manifest's
    // digests but carries a member for a different capability: P5B accepts it,
    // so install must detect the disagreement and fail closed rather than reuse
    // it or rebuild authority from provider advertisement.
    let intruder = descriptor("gmail.messages.send", "1.0.0", "gmail");
    let prepared = serea_capability::CapabilityRegistry::create_generation(
        &store,
        serea_storage::RegistryGenerationDraft {
            manifest_digest: m.digest().clone(),
            schema_catalog_digest: m.catalog_digest().clone(),
        },
    )
    .unwrap();
    let intruder_digest = descriptor_semantic_digest(&intruder, &c).unwrap();
    serea_capability::CapabilityRegistry::insert_descriptor_revision(
        &store,
        serea_storage::DescriptorRevisionDraft {
            descriptor_digest: intruder_digest.clone(),
            descriptor: intruder.clone(),
            input_schema_digest: c
                .document_digest(intruder.input_schema().as_str())
                .unwrap()
                .clone(),
            output_schema_digest: c
                .document_digest(intruder.output_schema().as_str())
                .unwrap()
                .clone(),
        },
    )
    .unwrap();
    serea_capability::CapabilityRegistry::add_generation_member(
        &store,
        serea_storage::GenerationMemberDraft {
            generation_id: prepared.generation_id(),
            descriptor_digest: intruder_digest,
            candidate_priority: 0,
        },
    )
    .unwrap();
    serea_capability::CapabilityRegistry::set_default_version(
        &store,
        prepared.generation_id(),
        intruder.id().clone(),
        intruder.version().clone(),
    )
    .unwrap();
    serea_capability::CapabilityRegistry::activate_generation(
        &store,
        &events,
        prepared.generation_id(),
        epoch,
    )
    .unwrap();
    let err = serea_capability::install(&store, &events, &m, &registry, epoch).unwrap_err();
    assert!(matches!(err, serea_capability::InstallError::Corruption(_)));
    // the corrupt generation stays exactly as it was; nothing was repaired
    let current = serea_capability::CapabilityRegistry::current_generation(&store)
        .unwrap()
        .unwrap();
    assert_eq!(current.generation_id(), prepared.generation_id());
}

#[test]
fn fresh_install_activates() {
    let (m, _c) = manifest();
    let registry = provider_registry_for();
    let store = store();
    let events = events();
    let outcome = serea_capability::install(
        &store,
        &events,
        &m,
        &registry,
        EpochMillis::new(1_796_000_000_000).unwrap(),
    )
    .unwrap();
    assert!(outcome.activated());
    let current = serea_capability::CapabilityRegistry::current_generation(&store)
        .unwrap()
        .unwrap();
    assert_eq!(current.manifest_digest(), m.digest());
    assert_eq!(current.schema_catalog_digest(), m.catalog_digest());
    assert_eq!(registry_change_count(&store, &events), 1);
}

#[test]
fn same_manifest_restart_reuses_generation() {
    let (m, _c) = manifest();
    let registry = provider_registry_for();
    let store = store();
    let events = events();
    let first = serea_capability::install(
        &store,
        &events,
        &m,
        &registry,
        EpochMillis::new(1_796_000_000_000).unwrap(),
    )
    .unwrap();
    assert!(first.activated());
    let second = serea_capability::install(
        &store,
        &events,
        &m,
        &registry,
        EpochMillis::new(1_796_000_000_000).unwrap(),
    )
    .unwrap();
    assert!(!second.activated());
    assert_eq!(second.generation_id(), first.generation_id());
    assert_eq!(registry_change_count(&store, &events), 1);
}

#[test]
fn changed_manifest_creates_new_generation() {
    let (m1, _c) = manifest();
    let registry = provider_registry_for();
    let store = store();
    let events = events();
    let first = serea_capability::install(
        &store,
        &events,
        &m1,
        &registry,
        EpochMillis::new(1_796_000_000_000).unwrap(),
    )
    .unwrap();
    assert!(first.activated());
    // changed manifest: add another capability
    let c = catalog();
    let d1 = descriptor("calendar.events.read", "1.0.0", "calendar");
    let d2 = descriptor("gmail.messages.send", "1.0.0", "gmail");
    let m2 = CapabilityManifestV1::build(
        vec![entry(&d1, 0, &c), entry(&d2, 0, &c)],
        vec![
            (d1.id().clone(), d1.version().clone()),
            (d2.id().clone(), d2.version().clone()),
        ],
        c,
    )
    .unwrap();
    let d3 = descriptor("gmail.messages.send", "1.0.0", "gmail");
    let p2 = Arc::new(serea_testkit::MockCapabilityProvider::new(
        ProviderId::new("gmail").unwrap(),
        vec![d3],
    ));
    let p1 = Arc::new(serea_testkit::MockCapabilityProvider::new(
        ProviderId::new("calendar").unwrap(),
        vec![d1.clone()],
    ));
    let registry2 = ProviderRegistry::build(vec![p1, p2]).unwrap();
    let second = serea_capability::install(
        &store,
        &events,
        &m2,
        &registry2,
        EpochMillis::new(1_796_000_000_000).unwrap(),
    )
    .unwrap();
    assert!(second.activated());
    assert_ne!(second.generation_id(), first.generation_id());
    assert_eq!(registry_change_count(&store, &events), 2);
    // old generation readable
    let old = serea_capability::CapabilityRegistry::generation(&store, first.generation_id())
        .unwrap()
        .unwrap();
    assert_eq!(old.manifest_digest(), m1.digest());
    let old_members =
        serea_capability::CapabilityRegistry::generation_members(&store, first.generation_id())
            .unwrap();
    assert_eq!(old_members.len(), 1);
}

#[test]
fn manifest_install_failure_writes_nothing() {
    let (m, _c) = manifest();
    let store = store();
    let events = events();
    // a provider advertising an unmanifested descriptor fails validation
    let rogue = descriptor("calendar.events.delete", "1.0.0", "calendar");
    let p = Arc::new(serea_testkit::MockCapabilityProvider::new(
        ProviderId::new("calendar").unwrap(),
        vec![rogue],
    ));
    let registry = ProviderRegistry::build(vec![p]).unwrap();
    let err = serea_capability::install(
        &store,
        &events,
        &m,
        &registry,
        EpochMillis::new(1_796_000_000_000).unwrap(),
    )
    .unwrap_err();
    assert!(matches!(err, serea_capability::InstallError::Provider(_)));
    assert!(
        serea_capability::CapabilityRegistry::current_generation(&store)
            .unwrap()
            .is_none()
    );
    assert_eq!(registry_change_count(&store, &events), 0);
}

#[test]
fn prepared_generation_never_authoritative() {
    let (m, _c) = manifest();
    let registry = provider_registry_for();
    let store = store();
    let events = events();
    let first = serea_capability::install(
        &store,
        &events,
        &m,
        &registry,
        EpochMillis::new(1_796_000_000_000).unwrap(),
    )
    .unwrap();
    // create a stray prepared generation (never activated)
    let draft = serea_storage::RegistryGenerationDraft {
        manifest_digest: serea_protocol::Digest::new(format!("sha256:{}", "9".repeat(64))).unwrap(),
        schema_catalog_digest: serea_protocol::Digest::new(format!("sha256:{}", "8".repeat(64)))
            .unwrap(),
    };
    let prepared = serea_capability::CapabilityRegistry::create_generation(&store, draft).unwrap();
    assert!(prepared.activated_at().is_none());
    assert_eq!(prepared.generation_id(), first.generation_id() + 1);
    // the active pointer, never MAX(generation_id), remains the authority
    let current = serea_capability::CapabilityRegistry::current_generation(&store)
        .unwrap()
        .unwrap();
    assert_eq!(current.generation_id(), first.generation_id());
    assert_eq!(current.manifest_digest(), m.digest());
}
