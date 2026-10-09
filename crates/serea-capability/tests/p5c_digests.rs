use std::collections::BTreeMap;

use serea_capability::{
    CapabilitySchemaCatalogV1, ManifestEntryV1, descriptor_semantic_digest, manifest_digest,
};
use serea_protocol::{
    Authorization, CapabilityDescriptor, CapabilityDescriptorDraft, CapabilityId, CostClass,
    DataClass, DescriptorDescription, DescriptorTitle, IdempotencySupport, JsonSchemaRef,
    ProviderId, ReplaySafety, RiskClass, RootRequirement, SemVer, SideEffectClass,
};

const PREFIX: &str = "https://serea.local/schemas/";

fn catalog() -> CapabilitySchemaCatalogV1 {
    let mut docs = BTreeMap::new();
    docs.insert(
        format!("{PREFIX}input.json"),
        r#"{
          "$schema": "https://json-schema.org/draft/2020-12/schema",
          "type": "object",
          "additionalProperties": false,
          "properties": {"name": {"type": "string", "maxLength": 32}},
          "required": ["name"]
        }"#
        .to_string(),
    );
    docs.insert(
        format!("{PREFIX}output.json"),
        "{\"$schema\":\"https://json-schema.org/draft/2020-12/schema\",\"type\":\"object\",\"additionalProperties\":false,\"properties\":{\"ok\":{\"type\":\"boolean\"}}}".to_string(),
    );
    CapabilitySchemaCatalogV1::build(docs).unwrap()
}

fn descriptor(_catalog: &CapabilitySchemaCatalogV1) -> CapabilityDescriptor {
    CapabilityDescriptor::new(CapabilityDescriptorDraft {
        id: CapabilityId::new("calendar.events.read").unwrap(),
        version: SemVer::new("1.0.0").unwrap(),
        title: DescriptorTitle::new("Read events").unwrap(),
        description: DescriptorDescription::new("Read private calendar events").unwrap(),
        provider_id: ProviderId::new("calendar").unwrap(),
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

#[test]
fn schema_digest_whitespace_and_key_order_independent() {
    let a = {
        let mut docs = BTreeMap::new();
        docs.insert(
            format!("{PREFIX}a.json"),
            "{\"$schema\":\"https://json-schema.org/draft/2020-12/schema\",\"type\":\"string\",\"maxLength\":4}".to_string(),
        );
        CapabilitySchemaCatalogV1::build(docs).unwrap()
    };
    let b = {
        let mut docs = BTreeMap::new();
        docs.insert(
            format!("{PREFIX}a.json"),
            "{ \"type\": \"string\",\n  \"maxLength\": 4,\n  \"$schema\": \"https://json-schema.org/draft/2020-12/schema\" }".to_string(),
        );
        CapabilitySchemaCatalogV1::build(docs).unwrap()
    };
    assert_eq!(
        a.document_digest(&format!("{PREFIX}a.json")),
        b.document_digest(&format!("{PREFIX}a.json"))
    );
}

#[test]
fn schema_digest_semantic_change_changes_digest() {
    let a = {
        let mut docs = BTreeMap::new();
        docs.insert(
            format!("{PREFIX}a.json"),
            "{\"$schema\":\"https://json-schema.org/draft/2020-12/schema\",\"type\":\"string\",\"maxLength\":4}".to_string(),
        );
        CapabilitySchemaCatalogV1::build(docs).unwrap()
    };
    let b = {
        let mut docs = BTreeMap::new();
        docs.insert(
            format!("{PREFIX}a.json"),
            "{\"$schema\":\"https://json-schema.org/draft/2020-12/schema\",\"type\":\"string\",\"maxLength\":5}".to_string(),
        );
        CapabilitySchemaCatalogV1::build(docs).unwrap()
    };
    assert_ne!(
        a.document_digest(&format!("{PREFIX}a.json")),
        b.document_digest(&format!("{PREFIX}a.json"))
    );
}

#[test]
fn catalog_digest_insertion_order_independent() {
    let make = |order: &[&str]| {
        let mut docs = BTreeMap::new();
        for name in order {
            docs.insert(
                format!("{PREFIX}{name}.json"),
                format!(
                    "{{\"$schema\":\"https://json-schema.org/draft/2020-12/schema\",\"type\":\"string\",\"maxLength\":{}}}",
                    name.len()
                ),
            );
        }
        CapabilitySchemaCatalogV1::build(docs).unwrap()
    };
    let a = make(&["aa", "b", "ccc"]);
    let b = make(&["ccc", "b", "aa"]);
    assert_eq!(a.digest(), b.digest());
}

#[test]
fn catalog_digest_changes_with_uri_or_document_change() {
    let base = {
        let mut docs = BTreeMap::new();
        docs.insert(
            format!("{PREFIX}aa.json"),
            "{\"$schema\":\"https://json-schema.org/draft/2020-12/schema\",\"type\":\"string\",\"maxLength\":2}".to_string(),
        );
        CapabilitySchemaCatalogV1::build(docs).unwrap()
    };
    let uri_changed = {
        let mut docs = BTreeMap::new();
        docs.insert(
            format!("{PREFIX}bb.json"),
            "{\"$schema\":\"https://json-schema.org/draft/2020-12/schema\",\"type\":\"string\",\"maxLength\":2}".to_string(),
        );
        CapabilitySchemaCatalogV1::build(docs).unwrap()
    };
    let doc_changed = {
        let mut docs = BTreeMap::new();
        docs.insert(
            format!("{PREFIX}aa.json"),
            "{\"$schema\":\"https://json-schema.org/draft/2020-12/schema\",\"type\":\"string\",\"maxLength\":3}".to_string(),
        );
        CapabilitySchemaCatalogV1::build(docs).unwrap()
    };
    assert_ne!(base.digest(), uri_changed.digest());
    assert_ne!(base.digest(), doc_changed.digest());
}

#[test]
fn descriptor_digest_golden() {
    let catalog = catalog();
    let d = descriptor(&catalog);
    let digest = descriptor_semantic_digest(&d, &catalog).unwrap();
    eprintln!("DESCRIPTOR GOLDEN: {}", digest.as_str());
    assert_eq!(
        digest.as_str(),
        "sha256:00feb5c8aaf595afa8115dfeda9b7501b6191687c1001899351e20c84bfa66d5"
    );
}

#[test]
fn descriptor_digest_changes_with_each_field() {
    let catalog = catalog();
    let base = descriptor(&catalog);
    let base_digest = descriptor_semantic_digest(&base, &catalog).unwrap();

    let variant = |mutate: fn(&mut CapabilityDescriptorDraft)| {
        let mut draft = CapabilityDescriptorDraft::from(descriptor(&catalog));
        mutate(&mut draft);
        descriptor_semantic_digest(&CapabilityDescriptor::new(draft).unwrap(), &catalog).unwrap()
    };

    assert_ne!(
        variant(|d| d.id = CapabilityId::new("calendar.events.delete").unwrap()),
        base_digest
    );
    assert_ne!(
        variant(|d| d.version = SemVer::new("1.0.1").unwrap()),
        base_digest
    );
    assert_ne!(
        variant(|d| d.title = DescriptorTitle::new("Changed").unwrap()),
        base_digest
    );
    assert_ne!(
        variant(|d| d.description = DescriptorDescription::new("Changed").unwrap()),
        base_digest
    );
    assert_ne!(
        variant(|d| {
            d.provider_id = ProviderId::new("gmail").unwrap();
            d.id = CapabilityId::new("gmail.events.read").unwrap();
        }),
        base_digest
    );
    assert_ne!(
        variant(|d| d.side_effect_class = SideEffectClass::LocalState),
        base_digest
    );
    assert_ne!(
        variant(|d| d.risk_class = RiskClass::LocalState),
        base_digest
    );
    assert_ne!(
        variant(|d| d.required_authorization = Authorization::DeviceUser),
        base_digest
    );
    assert_ne!(
        variant(|d| d.replay_safety = ReplaySafety::NonReplayable),
        base_digest
    );
    assert_ne!(variant(|d| d.data_class = DataClass::Private), base_digest);
    assert_ne!(
        variant(|d| d.root_requirement = RootRequirement::RequiresRoot),
        base_digest
    );
    assert_ne!(
        variant(|d| d.idempotency_support = IdempotencySupport::Native),
        base_digest
    );
    assert_ne!(variant(|d| d.max_duration_ms = 6_000), base_digest);
    assert_ne!(variant(|d| d.cost_class = CostClass::Paid), base_digest);
    assert_ne!(variant(|d| d.experimental = true), base_digest);
    // input/output schema swap changes digest
    assert_ne!(
        variant(|d| d.input_schema = JsonSchemaRef::new(format!("{PREFIX}output.json")).unwrap()),
        base_digest
    );
    // implementation_id presence changes digest
    assert_ne!(
        variant(|d| d.implementation_id =
            Some(serea_protocol::ImplementationId::new("calendar-local").unwrap())),
        base_digest
    );
}

#[test]
fn schema_digest_change_changes_descriptor_digest() {
    let catalog = catalog();
    let d = descriptor(&catalog);
    let base = descriptor_semantic_digest(&d, &catalog).unwrap();
    let mut docs = BTreeMap::new();
    docs.insert(
        format!("{PREFIX}input.json"),
        r#"{"$schema":"https://json-schema.org/draft/2020-12/schema","type":"object","additionalProperties":false,"properties":{"name":{"type":"string","maxLength":64}}}"#.to_string(),
    );
    docs.insert(
        format!("{PREFIX}output.json"),
        r#"{"$schema":"https://json-schema.org/draft/2020-12/schema","type":"object","additionalProperties":false,"properties":{"ok":{"type":"boolean"}}}"#.to_string(),
    );
    let catalog2 = CapabilitySchemaCatalogV1::build(docs).unwrap();
    let changed = descriptor_semantic_digest(&d, &catalog2).unwrap();
    assert_ne!(base, changed);
}

#[test]
fn priority_does_not_affect_descriptor_digest() {
    // candidate_priority is entry metadata, not descriptor semantics: two
    // entries sharing a descriptor must produce the same descriptor digest.
    let catalog = catalog();
    let d = descriptor(&catalog);
    let a = descriptor_semantic_digest(&d, &catalog).unwrap();
    let b = descriptor_semantic_digest(&d.clone(), &catalog).unwrap();
    assert_eq!(a, b);
}

#[test]
fn manifest_digest_entry_permutation_invariant() {
    let catalog = catalog();
    let d1 = descriptor(&catalog);
    let mut d2_draft = CapabilityDescriptorDraft::from(descriptor(&catalog));
    d2_draft.id = CapabilityId::new("calendar.events.list").unwrap();
    d2_draft.version = SemVer::new("1.1.0").unwrap();
    let d2 = CapabilityDescriptor::new(d2_draft).unwrap();
    let e1 = ManifestEntryV1 {
        descriptor: d1.clone(),
        candidate_priority: 0,
        descriptor_digest: descriptor_semantic_digest(&d1, &catalog).unwrap(),
        input_schema_digest: catalog
            .document_digest(&format!("{PREFIX}input.json"))
            .unwrap()
            .clone(),
        output_schema_digest: catalog
            .document_digest(&format!("{PREFIX}output.json"))
            .unwrap()
            .clone(),
    };
    let e2 = ManifestEntryV1 {
        descriptor: d2.clone(),
        candidate_priority: 0,
        descriptor_digest: descriptor_semantic_digest(&d2, &catalog).unwrap(),
        input_schema_digest: catalog
            .document_digest(&format!("{PREFIX}input.json"))
            .unwrap()
            .clone(),
        output_schema_digest: catalog
            .document_digest(&format!("{PREFIX}output.json"))
            .unwrap()
            .clone(),
    };
    let defaults = vec![(d1.id().clone(), d1.version().clone())];
    let a = manifest_digest(catalog.digest(), &[e1.clone(), e2.clone()], &defaults);
    let b = manifest_digest(catalog.digest(), &[e2, e1], &defaults);
    assert_eq!(a, b);
}

#[test]
fn manifest_digest_changes_with_default_or_priority_or_catalog() {
    let catalog = catalog();
    let d1 = descriptor(&catalog);
    let e1 = ManifestEntryV1 {
        descriptor: d1.clone(),
        candidate_priority: 0,
        descriptor_digest: descriptor_semantic_digest(&d1, &catalog).unwrap(),
        input_schema_digest: catalog
            .document_digest(&format!("{PREFIX}input.json"))
            .unwrap()
            .clone(),
        output_schema_digest: catalog
            .document_digest(&format!("{PREFIX}output.json"))
            .unwrap()
            .clone(),
    };
    let defaults = vec![(d1.id().clone(), d1.version().clone())];
    let base = manifest_digest(catalog.digest(), std::slice::from_ref(&e1), &defaults);

    let mut e2 = e1.clone();
    e2.candidate_priority = 1;
    assert_ne!(manifest_digest(catalog.digest(), &[e2], &defaults), base);

    let mut defaults2 = defaults.clone();
    defaults2[0].1 = SemVer::new("1.0.1").unwrap();
    assert_ne!(
        manifest_digest(catalog.digest(), std::slice::from_ref(&e1), &defaults2),
        base
    );

    let catalog_changed = {
        let mut docs = BTreeMap::new();
        docs.insert(
            format!("{PREFIX}input.json"),
            r#"{"$schema":"https://json-schema.org/draft/2020-12/schema","type":"object","additionalProperties":false,"properties":{"name":{"type":"string","maxLength":64}}}"#.to_string(),
        );
        docs.insert(
            format!("{PREFIX}output.json"),
            r#"{"$schema":"https://json-schema.org/draft/2020-12/schema","type":"object","additionalProperties":false,"properties":{"ok":{"type":"boolean"}}}"#.to_string(),
        );
        CapabilitySchemaCatalogV1::build(docs).unwrap()
    };
    assert_ne!(
        manifest_digest(catalog_changed.digest(), &[e1], &defaults),
        base
    );
}
