#[path = "common/mod.rs"]
mod common;

use std::collections::BTreeMap;
use std::sync::Arc;

use serea_capability::{
    CapabilityAvailabilitySnapshotV1, CapabilityManifestV1, CapabilitySchemaCatalogV1,
    HostEligibility, ManifestEntryV1, ProviderRegistry, provider_tools,
};
use serea_protocol::provider::CapabilityProvider;
use serea_protocol::{
    ActionResult, Authorization, CapabilityDescriptor, CapabilityDescriptorDraft, CapabilityId,
    CostClass, DataClass, DescriptorDescription, DescriptorTitle, IdempotencySupport,
    JsonSchemaRef, ProviderHealth, ProviderId, ReplaySafety, RiskClass, RootRequirement, SemVer,
    SideEffectClass,
};

const PREFIX: &str = "https://serea.local/schemas/";

struct FixedClock;
impl serea_protocol::Clock for FixedClock {
    fn now_ms(&self) -> Result<serea_protocol::EpochMillis, serea_protocol::ProtocolError> {
        serea_protocol::EpochMillis::new(1_796_000_000_000)
    }
}

fn catalog() -> CapabilitySchemaCatalogV1 {
    let mut docs = BTreeMap::new();
    docs.insert(
        format!("{PREFIX}cal-read-input.json"),
        r#"{"$schema":"https://json-schema.org/draft/2020-12/schema","type":"object","additionalProperties":false,"properties":{"calendar":{"type":"string","maxLength":64}},"required":["calendar"]}"#.to_string(),
    );
    docs.insert(
        format!("{PREFIX}cal-read-output.json"),
        r#"{"$schema":"https://json-schema.org/draft/2020-12/schema","type":"object","additionalProperties":false,"properties":{"events":{"type":"array","maxItems":64,"items":{"type":"string","maxLength":128}}}}"#.to_string(),
    );
    CapabilitySchemaCatalogV1::build(docs).unwrap()
}

fn descriptor(id: &str, provider: &str, experimental: bool) -> CapabilityDescriptor {
    CapabilityDescriptor::new(CapabilityDescriptorDraft {
        id: CapabilityId::new(id).unwrap(),
        version: SemVer::new("1.0.0").unwrap(),
        title: DescriptorTitle::new("Read events").unwrap(),
        description: DescriptorDescription::new("Read private calendar events").unwrap(),
        provider_id: ProviderId::new(provider).unwrap(),
        implementation_id: None,
        input_schema: JsonSchemaRef::new(format!("{PREFIX}cal-read-input.json")).unwrap(),
        output_schema: JsonSchemaRef::new(format!("{PREFIX}cal-read-output.json")).unwrap(),
        side_effect_class: SideEffectClass::None,
        risk_class: RiskClass::Observe,
        required_authorization: Authorization::None,
        replay_safety: ReplaySafety::Idempotent,
        data_class: DataClass::Personal,
        root_requirement: RootRequirement::NotRequired,
        idempotency_support: IdempotencySupport::None,
        max_duration_ms: 5_000,
        cost_class: CostClass::Free,
        experimental,
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
            descriptor_digest: serea_capability::descriptor_semantic_digest(d, &catalog).unwrap(),
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
        _request: &serea_protocol::ActionRequest,
        _ctx: &serea_protocol::provider::ProviderContext,
    ) -> Result<ActionResult, serea_protocol::ActionError> {
        panic!("P5D must never invoke a provider")
    }
    async fn health(&self) -> ProviderHealth {
        self.health
    }
}

fn snapshot(
    manifest: CapabilityManifestV1,
    providers: Vec<Arc<dyn CapabilityProvider>>,
    eligible: bool,
) -> CapabilityAvailabilitySnapshotV1 {
    let registry = ProviderRegistry::build(providers).unwrap();
    let mut entries = std::collections::HashMap::new();
    for entry in manifest.entries() {
        entries.insert(entry.descriptor_digest.clone(), eligible);
    }
    let store = serea_storage::Store::open_in_memory(&FixedClock).unwrap();
    common::build_snapshot(manifest, &registry, &store, HostEligibility::new(entries)).unwrap()
}

#[test]
fn tool_definition_shape_is_closed() {
    let d = descriptor("calendar.events.read", "calendar", false);
    let manifest = manifest_for(vec![d.clone()]);
    let snap = snapshot(
        manifest,
        vec![Arc::new(TestProvider {
            id: ProviderId::new("calendar").unwrap(),
            descriptors: vec![d],
            health: ProviderHealth::Ready,
        })],
        true,
    );
    let tools = provider_tools(&snap).unwrap();
    assert_eq!(tools.len(), 1);
    let tool = &tools[0];
    assert_eq!(tool.capability_id().as_str(), "calendar.events.read");
    assert_eq!(tool.version(), "1");
    assert_eq!(tool.title(), "Read events");
    // the projected JSON carries exactly the four declared members
    let value = tool.to_json_value();
    let object = value.as_object().expect("tool definition is an object");
    let mut keys: Vec<&str> = object.keys().map(String::as_str).collect();
    keys.sort_unstable();
    assert_eq!(
        keys,
        [
            "capability_id",
            "description",
            "input_schema",
            "title",
            "version"
        ]
    );
}

#[test]
fn tool_definitions_sort_by_capability_id_bytes() {
    let first = descriptor("calendar.events.read", "calendar", false);
    let second = descriptor("calendar.events.list", "calendar", false);
    let third = descriptor("calendar.agenda.list", "calendar", false);
    let manifest = manifest_for(vec![first.clone(), second.clone(), third.clone()]);
    let snap = snapshot(
        manifest,
        vec![Arc::new(TestProvider {
            id: ProviderId::new("calendar").unwrap(),
            descriptors: vec![first, second, third],
            health: ProviderHealth::Ready,
        })],
        true,
    );
    let tools = provider_tools(&snap).unwrap();
    let ids: Vec<&str> = tools.iter().map(|t| t.capability_id().as_str()).collect();
    assert_eq!(
        ids,
        [
            "calendar.agenda.list",
            "calendar.events.list",
            "calendar.events.read"
        ]
    );
}

#[test]
fn experimental_tool_hidden_without_opt_in() {
    let d = descriptor("calendar.events.read", "calendar", true);
    let manifest = manifest_for(vec![d.clone()]);
    let snap = snapshot(
        manifest,
        vec![Arc::new(TestProvider {
            id: ProviderId::new("calendar").unwrap(),
            descriptors: vec![d],
            health: ProviderHealth::Ready,
        })],
        true,
    );
    // without an experimental opt-in the capability is not visible
    assert!(provider_tools(&snap).unwrap().is_empty());
}

#[test]
fn degraded_provider_tools_are_hidden() {
    let d = descriptor("calendar.events.read", "calendar", false);
    let manifest = manifest_for(vec![d.clone()]);
    let snap = snapshot(
        manifest,
        vec![Arc::new(TestProvider {
            id: ProviderId::new("calendar").unwrap(),
            descriptors: vec![d],
            health: ProviderHealth::Degraded,
        })],
        true,
    );
    assert!(provider_tools(&snap).unwrap().is_empty());
}

#[test]
fn ineligible_host_tool_hidden() {
    let d = descriptor("calendar.events.read", "calendar", false);
    let manifest = manifest_for(vec![d.clone()]);
    let snap = snapshot(
        manifest,
        vec![Arc::new(TestProvider {
            id: ProviderId::new("calendar").unwrap(),
            descriptors: vec![d],
            health: ProviderHealth::Ready,
        })],
        false,
    );
    assert!(provider_tools(&snap).unwrap().is_empty());
}

#[test]
fn unknown_provider_tools_are_unavailable_not_visible() {
    // a manifest entry whose provider never registered must not be projected
    let d = descriptor("calendar.events.read", "calendar", false);
    let manifest = manifest_for(vec![d]);
    let snap = snapshot(manifest, vec![], true);
    assert!(provider_tools(&snap).unwrap().is_empty());
}

#[test]
fn tool_definition_excludes_authority_fields() {
    let d = descriptor("calendar.events.read", "calendar", false);
    let manifest = manifest_for(vec![d.clone()]);
    let snap = snapshot(
        manifest,
        vec![Arc::new(TestProvider {
            id: ProviderId::new("calendar").unwrap(),
            descriptors: vec![d],
            health: ProviderHealth::Ready,
        })],
        true,
    );
    let tools = provider_tools(&snap).unwrap();
    let rendered = serde_json::to_string(&tools[0].to_json_value()).unwrap();
    for forbidden in [
        "provider_id",
        "implementation_id",
        "capability_version",
        "risk_class",
        "side_effect_class",
        "required_authorization",
        "replay_safety",
        "root_requirement",
        "idempotency_support",
        "credential",
        "policy",
    ] {
        assert!(
            !rendered.contains(forbidden),
            "tool definition leaked {forbidden}"
        );
    }
}
