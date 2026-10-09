use std::collections::BTreeMap;

use serea_capability::{
    CapabilityManifestV1, CapabilitySchemaCatalogV1, ManifestEntryV1, ManifestError,
    descriptor_semantic_digest,
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
        r#"{"$schema":"https://json-schema.org/draft/2020-12/schema","type":"object","additionalProperties":false,"properties":{"name":{"type":"string","maxLength":32}},"required":["name"]}"#.to_string(),
    );
    docs.insert(
        format!("{PREFIX}output.json"),
        r#"{"$schema":"https://json-schema.org/draft/2020-12/schema","type":"object","additionalProperties":false,"properties":{"ok":{"type":"boolean"}}}"#.to_string(),
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

#[test]
fn one_valid_capability_version_implementation() {
    let c = catalog();
    let d = descriptor(
        "calendar.events.read",
        "1.0.0",
        "calendar",
        Some("calendar-local"),
    );
    let m = CapabilityManifestV1::build(
        vec![entry(&d, 0, &c)],
        vec![(d.id().clone(), d.version().clone())],
        c,
    )
    .unwrap();
    assert_eq!(
        m.default_version(d.id()).expect("pinned default").as_str(),
        "1.0.0"
    );
}

#[test]
fn multiple_versions() {
    let c = catalog();
    let d1 = descriptor("calendar.events.read", "1.0.0", "calendar", None);
    let d2 = descriptor("calendar.events.read", "2.0.0", "calendar", None);
    let m = CapabilityManifestV1::build(
        vec![entry(&d1, 0, &c), entry(&d2, 0, &c)],
        vec![(d1.id().clone(), d2.version().clone())],
        c,
    )
    .unwrap();
    assert_eq!(m.default_version(d1.id()).unwrap().as_str(), "2.0.0");
}

#[test]
fn explicit_prerelease_default() {
    let c = catalog();
    let d = descriptor("calendar.events.read", "1.0.0-rc.1", "calendar", None);
    let m = CapabilityManifestV1::build(
        vec![entry(&d, 0, &c)],
        vec![(d.id().clone(), d.version().clone())],
        c,
    )
    .unwrap();
    assert_eq!(m.default_version(d.id()).unwrap().as_str(), "1.0.0-rc.1");
}

#[test]
fn default_absent_from_entries() {
    let c = catalog();
    let d = descriptor("calendar.events.read", "1.0.0", "calendar", None);
    let err = CapabilityManifestV1::build(
        vec![entry(&d, 0, &c)],
        vec![(d.id().clone(), SemVer::new("2.0.0").unwrap())],
        c,
    )
    .unwrap_err();
    assert!(matches!(err, ManifestError::DefaultNotRepresented));
}

#[test]
fn duplicate_default() {
    let c = catalog();
    let d = descriptor("calendar.events.read", "1.0.0", "calendar", None);
    let err = CapabilityManifestV1::build(
        vec![entry(&d, 0, &c)],
        vec![
            (d.id().clone(), d.version().clone()),
            (d.id().clone(), d.version().clone()),
        ],
        c,
    )
    .unwrap_err();
    assert!(matches!(err, ManifestError::DuplicateDefault));
}

#[test]
fn duplicate_identity() {
    let c = catalog();
    let d = descriptor("calendar.events.read", "1.0.0", "calendar", None);
    let err = CapabilityManifestV1::build(
        vec![entry(&d, 0, &c), entry(&d, 1, &c)],
        vec![(d.id().clone(), d.version().clone())],
        c,
    )
    .unwrap_err();
    assert!(matches!(err, ManifestError::DuplicateIdentity));
}

#[test]
fn none_implementation_mixed_with_some() {
    let c = catalog();
    let d_none = descriptor("calendar.events.read", "1.0.0", "calendar", None);
    let d_some = descriptor(
        "calendar.events.read",
        "1.0.0",
        "calendar",
        Some("calendar-local"),
    );
    let err = CapabilityManifestV1::build(
        vec![entry(&d_none, 0, &c), entry(&d_some, 1, &c)],
        vec![(d_none.id().clone(), d_none.version().clone())],
        c,
    )
    .unwrap_err();
    assert!(matches!(err, ManifestError::NoneImplementationConflict));
}

#[test]
fn duplicate_candidate_priority() {
    let c = catalog();
    let d1 = descriptor(
        "calendar.events.read",
        "1.0.0",
        "calendar",
        Some("calendar-local"),
    );
    let d2 = descriptor(
        "calendar.events.read",
        "1.0.0",
        "calendar",
        Some("calendar-backup"),
    );
    let err = CapabilityManifestV1::build(
        vec![entry(&d1, 0, &c), entry(&d2, 0, &c)],
        vec![(d1.id().clone(), d1.version().clone())],
        c,
    )
    .unwrap_err();
    assert!(matches!(err, ManifestError::DuplicateCandidatePriority));
}

#[test]
fn ordinary_host_provider_rejected() {
    let c = catalog();
    let d = descriptor("host.goal.start", "1.0.0", "host", None);
    let err = CapabilityManifestV1::build(
        vec![entry(&d, 0, &c)],
        vec![(d.id().clone(), d.version().clone())],
        c,
    )
    .unwrap_err();
    assert!(matches!(
        err,
        ManifestError::OrdinaryHostProvider | ManifestError::HostCapabilityId
    ));
}

#[test]
fn host_capability_id_rejected() {
    let c = catalog();
    let d = descriptor("host.events.read", "1.0.0", "host", None);
    let err = CapabilityManifestV1::build(
        vec![entry(&d, 0, &c)],
        vec![(d.id().clone(), d.version().clone())],
        c,
    )
    .unwrap_err();
    assert!(matches!(
        err,
        ManifestError::OrdinaryHostProvider | ManifestError::HostCapabilityId
    ));
}

#[test]
fn schema_uri_missing() {
    let c = catalog();
    let mut draft = CapabilityDescriptorDraft::from(descriptor(
        "calendar.events.read",
        "1.0.0",
        "calendar",
        None,
    ));
    draft.input_schema = JsonSchemaRef::new(format!("{PREFIX}missing.json")).unwrap();
    let d = CapabilityDescriptor::new(draft).unwrap();
    // Build the entry's digests against a catalog where missing.json does exist,
    // then hand the manifest a catalog that lacks it.
    let mut docs = BTreeMap::new();
    docs.insert(
        format!("{PREFIX}missing.json"),
        r#"{"$schema":"https://json-schema.org/draft/2020-12/schema","type":"string","maxLength":4}"#.to_string(),
    );
    docs.insert(
        format!("{PREFIX}input.json"),
        r#"{"$schema":"https://json-schema.org/draft/2020-12/schema","type":"object","additionalProperties":false,"properties":{"name":{"type":"string","maxLength":32}},"required":["name"]}"#.to_string(),
    );
    docs.insert(
        format!("{PREFIX}output.json"),
        r#"{"$schema":"https://json-schema.org/draft/2020-12/schema","type":"object","additionalProperties":false,"properties":{"ok":{"type":"boolean"}}}"#.to_string(),
    );
    let other = CapabilitySchemaCatalogV1::build(docs).unwrap();
    let mut e = entry(&d, 0, &other);
    e.descriptor = d.clone();
    let err = CapabilityManifestV1::build(vec![e], vec![(d.id().clone(), d.version().clone())], c)
        .unwrap_err();
    assert!(matches!(err, ManifestError::SchemaUriMissing));
}

#[test]
fn input_digest_mismatch() {
    let c = catalog();
    let d = descriptor("calendar.events.read", "1.0.0", "calendar", None);
    let mut e = entry(&d, 0, &c);
    e.input_schema_digest =
        serea_protocol::Digest::new(format!("sha256:{}", "0".repeat(64))).unwrap();
    let err = CapabilityManifestV1::build(vec![e], vec![(d.id().clone(), d.version().clone())], c)
        .unwrap_err();
    assert!(matches!(err, ManifestError::SchemaDigestMismatch));
}

#[test]
fn output_digest_mismatch() {
    let c = catalog();
    let d = descriptor("calendar.events.read", "1.0.0", "calendar", None);
    let mut e = entry(&d, 0, &c);
    e.output_schema_digest =
        serea_protocol::Digest::new(format!("sha256:{}", "0".repeat(64))).unwrap();
    let err = CapabilityManifestV1::build(vec![e], vec![(d.id().clone(), d.version().clone())], c)
        .unwrap_err();
    assert!(matches!(err, ManifestError::SchemaDigestMismatch));
}

#[test]
fn descriptor_digest_mismatch() {
    let c = catalog();
    let d = descriptor("calendar.events.read", "1.0.0", "calendar", None);
    let mut e = entry(&d, 0, &c);
    e.descriptor_digest =
        serea_protocol::Digest::new(format!("sha256:{}", "1".repeat(64))).unwrap();
    let err = CapabilityManifestV1::build(vec![e], vec![(d.id().clone(), d.version().clone())], c)
        .unwrap_err();
    assert!(matches!(err, ManifestError::DescriptorDigestMismatch));
}
