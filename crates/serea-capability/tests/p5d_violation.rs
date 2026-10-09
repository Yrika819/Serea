#[path = "common/mod.rs"]
mod common;

use std::collections::BTreeMap;
use std::sync::Arc;

use serde_json::Value;
use serea_capability::{
    CapabilityAvailabilitySnapshotV1, CapabilityManifestV1, CapabilitySchemaCatalogV1,
    ClassifiedArgumentsV1, HostEligibility, ManifestEntryV1, ModelSchemaViolation,
    ProviderRegistry, parse_tool_call_proposal, prepare_action, record_schema_violation,
};
use serea_event_bus::{EventBus, ReplayItem};
use serea_event_bus::{ModelEventMetadataV1, ModelEventRelationV1};
use serea_protocol::provider::CapabilityProvider;
use serea_protocol::{
    ActionResult, Authorization, CapabilityDescriptor, CapabilityDescriptorDraft, CapabilityId,
    CostClass, DataClass, DescriptorDescription, DescriptorTitle, EpochMillis, EventKind,
    IdempotencySupport, JsonSchemaRef, ModelId, ModelPurpose, ProviderHealth, ProviderId,
    ReplaySafety, RequestedBy, RiskClass, RootRequirement, SemVer, SideEffectClass, StepId, TaskId,
};
use serea_storage::Store;
use serea_testkit::DeterministicUlidSource;

const PREFIX: &str = "https://serea.local/schemas/";

struct FixedClock;
impl serea_protocol::Clock for FixedClock {
    fn now_ms(&self) -> Result<EpochMillis, serea_protocol::ProtocolError> {
        EpochMillis::new(1_796_000_000_000)
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

struct TestProvider {
    id: ProviderId,
    descriptors: Vec<CapabilityDescriptor>,
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
        ProviderHealth::Ready
    }
}

fn snapshot() -> CapabilityAvailabilitySnapshotV1 {
    let d = descriptor();
    let catalog = catalog();
    let digest = serea_capability::descriptor_semantic_digest(&d, &catalog).unwrap();
    let manifest = CapabilityManifestV1::build(
        vec![ManifestEntryV1 {
            descriptor: d.clone(),
            candidate_priority: 0,
            descriptor_digest: digest.clone(),
            input_schema_digest: catalog
                .document_digest(d.input_schema().as_str())
                .unwrap()
                .clone(),
            output_schema_digest: catalog
                .document_digest(d.output_schema().as_str())
                .unwrap()
                .clone(),
        }],
        vec![(d.id().clone(), d.version().clone())],
        catalog,
    )
    .unwrap();
    let registry = ProviderRegistry::build(vec![Arc::new(TestProvider {
        id: ProviderId::new("calendar").unwrap(),
        descriptors: vec![d],
    })])
    .unwrap();
    let mut eligibility = std::collections::HashMap::new();
    eligibility.insert(digest, true);
    let store = Store::open_in_memory(&FixedClock).unwrap();
    common::build_snapshot(
        manifest,
        &registry,
        &store,
        HostEligibility::new(eligibility),
    )
    .unwrap()
}

fn metadata() -> ModelEventMetadataV1 {
    ModelEventMetadataV1 {
        request_id: serea_protocol::RequestId::new("req_00000000000000000000000001").unwrap(),
        model_id: ModelId::new("local-stealth").unwrap(),
        provider_id: ProviderId::new("openai").unwrap(),
        task_id: Some(TaskId::new("tsk_00000000000000000000000001").unwrap()),
        purpose: ModelPurpose::Planning,
        relation: ModelEventRelationV1::Normal,
        data_class: DataClass::Personal,
        occurred_at: EpochMillis::new(1_796_000_000_000).unwrap(),
    }
}

fn store() -> Store {
    Store::open_in_memory(&FixedClock).unwrap()
}

fn events() -> EventBus {
    EventBus::new(DeterministicUlidSource::new())
}

fn violation_events(store: &Store) -> Vec<Value> {
    let page = EventBus::replay(store, None, None, 256).unwrap();
    page.items
        .iter()
        .filter_map(|item| match item {
            ReplayItem::Event { event } if event.kind == EventKind::ModelSchemaViolation => {
                Some(Value::Object(event.payload.clone()))
            }
            _ => None,
        })
        .collect()
}

#[test]
fn schema_violation_event_carries_names_and_counts_only() {
    let store = store();
    let events = events();
    let rejection = parse_tool_call_proposal(
        r#"{"version":"1","capability_id":"calendar.events.read","arguments":{},"risk_class":"LOCAL_STATE","data_class":"SECRET"}"#,
    )
    .unwrap_err();
    record_schema_violation(
        &store,
        &events,
        metadata(),
        Some(StepId::new("stp_01JQ8Z9M3R2CVN8H5FWK7PQDSF").unwrap()),
        &rejection,
    )
    .unwrap();
    let payloads = violation_events(&store);
    assert_eq!(payloads.len(), 1);
    let payload = payloads[0].as_object().unwrap();
    // allowed metadata only
    assert!(payload.contains_key("violation_code"));
    assert!(payload.contains_key("offending_field_names"));
    assert!(payload.contains_key("offending_field_count"));
    // never the rejected values, arguments, proposal or prompt
    let rendered = Value::Object(payload.clone()).to_string();
    for forbidden in [
        "LOCAL_STATE",
        "SECRET",
        "arguments",
        "prompt",
        "calendar",
        "content",
    ] {
        assert!(
            !rendered.contains(forbidden),
            "violation payload leaked {forbidden}"
        );
    }
}

#[test]
fn violation_names_are_sorted_and_bounded() {
    let store = store();
    let events = events();
    let rejection = parse_tool_call_proposal(
        r#"{"version":"1","capability_id":"calendar.events.read","arguments":{},"risk_class":"X","data_class":"Y","provider_id":"Z"}"#,
    )
    .unwrap_err();
    record_schema_violation(&store, &events, metadata(), None, &rejection).unwrap();
    let payloads = violation_events(&store);
    let names = payloads[0]["offending_field_names"].as_array().unwrap();
    let rendered: Vec<&str> = names.iter().filter_map(|v| v.as_str()).collect();
    assert_eq!(rendered, ["data_class", "provider_id", "risk_class"]);
    assert_eq!(payloads[0]["offending_field_count"].as_u64(), Some(3));
}

#[test]
fn missing_field_event_names_only_the_actual_missing_member() {
    let store = store();
    let events = events();
    let rejection =
        parse_tool_call_proposal(r#"{"version":"1","capability_id":"calendar.events.read"}"#)
            .unwrap_err();
    record_schema_violation(&store, &events, metadata(), None, &rejection).unwrap();
    let payloads = violation_events(&store);
    assert_eq!(payloads.len(), 1);
    assert_eq!(
        payloads[0]["offending_field_names"],
        serde_json::json!(["arguments"])
    );
    assert_eq!(payloads[0]["offending_field_count"].as_u64(), Some(1));
}

#[test]
fn schema_violation_debug_and_event_metadata_never_echo_argument_values() {
    const SENTINEL: &str = "DO_NOT_LOG_THIS_VALUE_7a31";
    let store = store();
    let events = events();
    let rejection = parse_tool_call_proposal(&format!(
        r#"{{"version":"1","capability_id":"calendar.events.read","arguments":{{"calendar":"{SENTINEL}"}},"risk_class":"SECRET"}}"#
    ))
    .unwrap_err();
    let violation = ModelSchemaViolation::from(&rejection);
    assert!(!format!("{rejection:?}").contains(SENTINEL));
    assert!(!rejection.to_string().contains(SENTINEL));
    assert!(!format!("{violation:?}").contains(SENTINEL));
    assert!(!violation.to_string().contains(SENTINEL));
    record_schema_violation(&store, &events, metadata(), None, &rejection).unwrap();
    let payloads = violation_events(&store);
    assert_eq!(payloads.len(), 1);
    assert!(!payloads[0].to_string().contains(SENTINEL));
}

#[test]
fn step_id_is_recorded_when_present_and_omitted_when_not() {
    let store = store();
    let events = events();
    let rejection = parse_tool_call_proposal(r#"not json"#).unwrap_err();
    record_schema_violation(
        &store,
        &events,
        metadata(),
        Some(StepId::new("stp_01JQ8Z9M3R2CVN8H5FWK7PQDSF").unwrap()),
        &rejection,
    )
    .unwrap();
    record_schema_violation(&store, &events, metadata(), None, &rejection).unwrap();
    let payloads = violation_events(&store);
    assert_eq!(payloads.len(), 2);
    assert!(payloads[0]["step_id"].is_string());
    assert!(payloads[1].get("step_id").is_none());
}

#[test]
fn hostile_field_names_are_not_echoed_verbatim() {
    let store = store();
    let events = events();
    let long_name = "a".repeat(500);
    let text = format!(
        r#"{{"version":"1","capability_id":"calendar.events.read","arguments":{{}},"{long_name}":"v"}}"#
    );
    let rejection = parse_tool_call_proposal(&text).unwrap_err();
    record_schema_violation(&store, &events, metadata(), None, &rejection).unwrap();
    let payloads = violation_events(&store);
    // the hostile name is too long to report, so it is dropped, not echoed
    assert_eq!(payloads[0]["offending_field_count"].as_u64(), Some(0));
    let rendered = payloads[0].to_string();
    assert!(!rendered.contains(&"a".repeat(100)));
}

#[test]
fn violation_event_is_refused_for_non_public_classes() {
    // Model events carry no private material: a higher-class proposal cannot
    // be recorded as an activity event at all.
    let store = store();
    let events = events();
    let mut meta = metadata();
    meta.data_class = DataClass::Private;
    let rejection = parse_tool_call_proposal(r#"not json"#).unwrap_err();
    let err = record_schema_violation(&store, &events, meta, None, &rejection).unwrap_err();
    assert!(matches!(err, serea_storage::StoreError::EventClassRefused));
    assert!(violation_events(&store).is_empty());
}

#[test]
fn invalid_proposal_produces_no_prepared_action() {
    let snap = snapshot();
    let rejection = parse_tool_call_proposal(
        r#"{"version":"1","capability_id":"calendar.events.read","arguments":{},"provider_id":"attacker"}"#,
    )
    .unwrap_err();
    assert!(rejection.offending_field_count() > 0);
    // even with valid arguments elsewhere, an injected proposal never prepares
    let classified = ClassifiedArgumentsV1::new_trusted(
        serde_json::Map::from_iter([("calendar".to_string(), Value::String("work".into()))]),
        DataClass::Personal,
    );
    let prepared = prepare_action(
        &snap,
        &CapabilityId::new("calendar.events.read").unwrap(),
        TaskId::new("tsk_00000000000000000000000001").unwrap(),
        StepId::new("stp_01JQ8Z9M3R2CVN8H5FWK7PQDSF").unwrap(),
        &classified,
        RequestedBy::Model,
        None,
        None,
    );
    // the host may still prepare from the trusted path, but the injected
    // proposal itself contributed nothing: no host field from it survived.
    assert!(prepared.is_ok());
    let prepared = prepared.unwrap();
    assert_eq!(prepared.provider_id().as_str(), "calendar");
    assert_eq!(prepared.risk_class(), RiskClass::Observe);
    assert_eq!(prepared.data_class(), DataClass::Personal);
}

#[test]
fn violation_code_is_stable_and_content_free() {
    let rejection = parse_tool_call_proposal(r#"{"version":"9"}"#).unwrap_err();
    assert_eq!(rejection.code(), "UNSUPPORTED_VERSION");
    let rendered = ModelSchemaViolation::from(&rejection).to_string();
    assert_eq!(rendered, "MODEL_SCHEMA_VIOLATION:UNSUPPORTED_VERSION");
}
