use std::collections::BTreeMap;
use std::sync::Arc;

use serea_capability::{
    CapabilityManifestV1, CapabilitySchemaCatalogV1, ManifestEntryV1, ProviderError,
    ProviderRegistry, descriptor_semantic_digest,
};
use serea_protocol::{
    Authorization, CapabilityDescriptor, CapabilityDescriptorDraft, CapabilityId, CostClass,
    DataClass, DescriptorDescription, DescriptorTitle, IdempotencySupport, ImplementationId,
    JsonSchemaRef, ProviderId, ReplaySafety, RiskClass, RootRequirement, SemVer, SideEffectClass,
};

const PREFIX: &str = "https://serea.local/schemas/";

fn catalog() -> CapabilitySchemaCatalogV1 {
    let mut docs = BTreeMap::new();
    docs.insert(
        format!("{PREFIX}input.json"),
        r#"{"$schema":"https://json-schema.org/draft/2020-12/schema","type":"object","additionalProperties":false,"properties":{"name":{"type":"string","maxLength":32}},"required":["name"]}"#
            .to_string(),
    );
    docs.insert(
        format!("{PREFIX}output.json"),
        r#"{"$schema":"https://json-schema.org/draft/2020-12/schema","type":"object","additionalProperties":false,"properties":{"ok":{"type":"boolean"}}}"#
            .to_string(),
    );
    CapabilitySchemaCatalogV1::build(docs).unwrap()
}

fn descriptor(
    id: &str,
    version: &str,
    provider: &str,
    impl_id: Option<&str>,
) -> CapabilityDescriptor {
    CapabilityDescriptor::new(CapabilityDescriptorDraft {
        id: CapabilityId::new(id).unwrap(),
        version: SemVer::new(version).unwrap(),
        title: DescriptorTitle::new("T").unwrap(),
        description: DescriptorDescription::new("D").unwrap(),
        provider_id: ProviderId::new(provider).unwrap(),
        implementation_id: impl_id.map(|s| ImplementationId::new(s).unwrap()),
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

fn manifest() -> CapabilityManifestV1 {
    let c = catalog();
    let d1 = descriptor("calendar.events.read", "1.0.0", "calendar", None);
    let d2 = descriptor("calendar.events.create", "1.0.0", "calendar", None);
    let d3 = descriptor("gmail.messages.send", "1.0.0", "gmail", None);
    CapabilityManifestV1::build(
        vec![entry(&d1, 0, &c), entry(&d2, 0, &c), entry(&d3, 0, &c)],
        vec![
            (d1.id().clone(), d1.version().clone()),
            (d2.id().clone(), d2.version().clone()),
            (d3.id().clone(), d3.version().clone()),
        ],
        c,
    )
    .unwrap()
}

#[test]
fn exact_advertised_descriptor_accepted() {
    let m = manifest();
    let d = descriptor("calendar.events.read", "1.0.0", "calendar", None);
    let p = Arc::new(serea_testkit::MockCapabilityProvider::new(
        ProviderId::new("calendar").unwrap(),
        vec![d],
    ));
    let r = ProviderRegistry::build(vec![p]).unwrap();
    r.validate_advertisements(&m).unwrap();
}

#[test]
fn missing_provider_is_not_manifest_failure() {
    let m = manifest();
    let d = descriptor("gmail.messages.send", "1.0.0", "gmail", None);
    let p = Arc::new(serea_testkit::MockCapabilityProvider::new(
        ProviderId::new("gmail").unwrap(),
        vec![d],
    ));
    let r = ProviderRegistry::build(vec![p]).unwrap();
    r.validate_advertisements(&m).unwrap();
}

#[test]
fn provider_omits_one_entry_ok() {
    let m = manifest();
    let d = descriptor("calendar.events.read", "1.0.0", "calendar", None);
    let p = Arc::new(serea_testkit::MockCapabilityProvider::new(
        ProviderId::new("calendar").unwrap(),
        vec![d],
    ));
    let r = ProviderRegistry::build(vec![p]).unwrap();
    r.validate_advertisements(&m).unwrap();
}

#[test]
fn extra_unmanifested_descriptor_rejected() {
    let m = manifest();
    let d = descriptor("calendar.events.delete", "1.0.0", "calendar", None);
    let p = Arc::new(serea_testkit::MockCapabilityProvider::new(
        ProviderId::new("calendar").unwrap(),
        vec![d],
    ));
    let r = ProviderRegistry::build(vec![p]).unwrap();
    assert!(matches!(
        r.validate_advertisements(&m).unwrap_err(),
        ProviderError::UnmanifestedAdvertisement
    ));
}

#[test]
fn lower_risk_rejected() {
    let m = manifest();
    let mut draft = CapabilityDescriptorDraft::from(descriptor(
        "calendar.events.read",
        "1.0.0",
        "calendar",
        None,
    ));
    draft.risk_class = RiskClass::LocalState;
    let d = CapabilityDescriptor::new(draft).unwrap();
    let p = Arc::new(serea_testkit::MockCapabilityProvider::new(
        ProviderId::new("calendar").unwrap(),
        vec![d],
    ));
    let r = ProviderRegistry::build(vec![p]).unwrap();
    assert!(matches!(
        r.validate_advertisements(&m).unwrap_err(),
        ProviderError::AdvertisementMismatch
    ));
}

#[test]
fn higher_risk_rejected() {
    let m = manifest();
    let mut draft = CapabilityDescriptorDraft::from(descriptor(
        "calendar.events.read",
        "1.0.0",
        "calendar",
        None,
    ));
    draft.risk_class = RiskClass::Credential;
    draft.data_class = DataClass::Credential;
    let d = CapabilityDescriptor::new(draft).unwrap();
    let p = Arc::new(serea_testkit::MockCapabilityProvider::new(
        ProviderId::new("calendar").unwrap(),
        vec![d],
    ));
    let r = ProviderRegistry::build(vec![p]).unwrap();
    assert!(matches!(
        r.validate_advertisements(&m).unwrap_err(),
        ProviderError::AdvertisementMismatch
    ));
}

#[test]
fn changed_authorization_rejected() {
    let m = manifest();
    let mut draft = CapabilityDescriptorDraft::from(descriptor(
        "calendar.events.read",
        "1.0.0",
        "calendar",
        None,
    ));
    draft.required_authorization = Authorization::DeviceUser;
    let d = CapabilityDescriptor::new(draft).unwrap();
    let p = Arc::new(serea_testkit::MockCapabilityProvider::new(
        ProviderId::new("calendar").unwrap(),
        vec![d],
    ));
    let r = ProviderRegistry::build(vec![p]).unwrap();
    assert!(matches!(
        r.validate_advertisements(&m).unwrap_err(),
        ProviderError::AdvertisementMismatch
    ));
}

#[test]
fn changed_replay_safety_rejected() {
    let m = manifest();
    let mut draft = CapabilityDescriptorDraft::from(descriptor(
        "calendar.events.read",
        "1.0.0",
        "calendar",
        None,
    ));
    draft.replay_safety = ReplaySafety::NonReplayable;
    let d = CapabilityDescriptor::new(draft).unwrap();
    let p = Arc::new(serea_testkit::MockCapabilityProvider::new(
        ProviderId::new("calendar").unwrap(),
        vec![d],
    ));
    let r = ProviderRegistry::build(vec![p]).unwrap();
    assert!(matches!(
        r.validate_advertisements(&m).unwrap_err(),
        ProviderError::AdvertisementMismatch
    ));
}

#[test]
fn changed_schema_ref_rejected() {
    let m = manifest();
    let mut draft = CapabilityDescriptorDraft::from(descriptor(
        "calendar.events.read",
        "1.0.0",
        "calendar",
        None,
    ));
    draft.input_schema = JsonSchemaRef::new(format!("{PREFIX}output.json")).unwrap();
    let d = CapabilityDescriptor::new(draft).unwrap();
    let p = Arc::new(serea_testkit::MockCapabilityProvider::new(
        ProviderId::new("calendar").unwrap(),
        vec![d],
    ));
    let r = ProviderRegistry::build(vec![p]).unwrap();
    assert!(matches!(
        r.validate_advertisements(&m).unwrap_err(),
        ProviderError::AdvertisementMismatch
    ));
}

#[test]
fn duplicate_provider_id_rejected() {
    let d1 = descriptor("calendar.events.read", "1.0.0", "calendar", None);
    let p1 = Arc::new(serea_testkit::MockCapabilityProvider::new(
        ProviderId::new("calendar").unwrap(),
        vec![d1],
    ));
    let p2 = Arc::new(serea_testkit::MockCapabilityProvider::new(
        ProviderId::new("calendar").unwrap(),
        vec![],
    ));
    assert!(matches!(
        ProviderRegistry::build(vec![p1, p2]).unwrap_err(),
        ProviderError::DuplicateProviderId
    ));
}

#[test]
fn host_provider_rejected() {
    let p = Arc::new(serea_testkit::MockCapabilityProvider::new(
        ProviderId::new("host").unwrap(),
        vec![],
    ));
    assert!(matches!(
        ProviderRegistry::build(vec![p]).unwrap_err(),
        ProviderError::HostProvider
    ));
}

#[test]
fn provider_order_permutations_identical_registry_result() {
    // registry validation result must not depend on Vec order of providers
    let m = manifest();
    let dc = descriptor("calendar.events.read", "1.0.0", "calendar", None);
    let dd = descriptor("gmail.messages.send", "1.0.0", "gmail", None);
    let p1 = Arc::new(serea_testkit::MockCapabilityProvider::new(
        ProviderId::new("calendar").unwrap(),
        vec![dc],
    ));
    let p2 = Arc::new(serea_testkit::MockCapabilityProvider::new(
        ProviderId::new("gmail").unwrap(),
        vec![dd],
    ));
    let a = ProviderRegistry::build(vec![p1.clone(), p2.clone()]).unwrap();
    let b = ProviderRegistry::build(vec![p2, p1]).unwrap();
    assert!(a.validate_advertisements(&m).is_ok());
    assert!(b.validate_advertisements(&m).is_ok());
}
