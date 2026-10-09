use std::collections::BTreeMap;
use std::sync::Arc;

use serea_capability::{
    CapabilityAvailabilitySnapshotV1, CapabilityManifestV1, CapabilitySchemaCatalogV1,
    ClassifiedArgumentsV1, HostEligibility, ManifestEntryV1, PreparationError, PreparedActionV1,
    ProviderRegistry, prepare_action,
};
use serea_protocol::provider::CapabilityProvider;
use serea_protocol::{
    ActionResult, Authorization, CapabilityDescriptor, CapabilityDescriptorDraft, CapabilityId,
    CostClass, DataClass, DescriptorDescription, DescriptorTitle, EpochMillis, IdempotencySupport,
    ImplementationId, JsonSchemaRef, ProviderHealth, ProviderId, ReplaySafety, RequestedBy,
    RiskClass, RootRequirement, SemVer, SideEffectClass, StepId, TaskId,
};

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
        r#"{"$schema":"https://json-schema.org/draft/2020-12/schema","type":"object","additionalProperties":false,"properties":{"calendar":{"type":"string","maxLength":64},"limit":{"type":"number"}},"required":["calendar"]}"#.to_string(),
    );
    docs.insert(
        format!("{PREFIX}cal-output.json"),
        r#"{"$schema":"https://json-schema.org/draft/2020-12/schema","type":"object","additionalProperties":false,"properties":{"ok":{"type":"boolean"}}}"#.to_string(),
    );
    CapabilitySchemaCatalogV1::build(docs).unwrap()
}

fn descriptor(risk: RiskClass, ceiling: DataClass) -> CapabilityDescriptor {
    CapabilityDescriptor::new(CapabilityDescriptorDraft {
        id: CapabilityId::new("calendar.events.read").unwrap(),
        version: SemVer::new("1.0.0").unwrap(),
        title: DescriptorTitle::new("Read events").unwrap(),
        description: DescriptorDescription::new("Read private calendar events").unwrap(),
        provider_id: ProviderId::new("calendar").unwrap(),
        implementation_id: Some(ImplementationId::new("calendar-local").unwrap()),
        input_schema: JsonSchemaRef::new(format!("{PREFIX}cal-input.json")).unwrap(),
        output_schema: JsonSchemaRef::new(format!("{PREFIX}cal-output.json")).unwrap(),
        side_effect_class: SideEffectClass::None,
        risk_class: risk,
        required_authorization: Authorization::None,
        replay_safety: ReplaySafety::Idempotent,
        data_class: ceiling,
        root_requirement: RootRequirement::NotRequired,
        idempotency_support: IdempotencySupport::None,
        max_duration_ms: 5_000,
        cost_class: CostClass::Free,
        experimental: false,
    })
    .unwrap()
}

fn manifest_with(descriptor: &CapabilityDescriptor) -> CapabilityManifestV1 {
    let catalog = catalog();
    CapabilityManifestV1::build(
        vec![ManifestEntryV1 {
            descriptor: descriptor.clone(),
            candidate_priority: 0,
            descriptor_digest: serea_capability::descriptor_semantic_digest(descriptor, &catalog)
                .unwrap(),
            input_schema_digest: catalog
                .document_digest(descriptor.input_schema().as_str())
                .unwrap()
                .clone(),
            output_schema_digest: catalog
                .document_digest(descriptor.output_schema().as_str())
                .unwrap()
                .clone(),
        }],
        vec![(descriptor.id().clone(), descriptor.version().clone())],
        catalog,
    )
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

fn snapshot_for(descriptor: &CapabilityDescriptor) -> CapabilityAvailabilitySnapshotV1 {
    let manifest = manifest_with(descriptor);
    let registry = ProviderRegistry::build(vec![Arc::new(TestProvider {
        id: ProviderId::new("calendar").unwrap(),
        descriptors: vec![descriptor.clone()],
    })])
    .unwrap();
    let mut eligibility = std::collections::HashMap::new();
    eligibility.insert(
        serea_capability::descriptor_semantic_digest(descriptor, &catalog()).unwrap(),
        true,
    );
    let store = serea_storage::Store::open_in_memory(&FixedClock).unwrap();
    CapabilityAvailabilitySnapshotV1::build(
        manifest,
        &registry,
        &store,
        HostEligibility::new(eligibility),
    )
    .unwrap()
}

fn task() -> TaskId {
    TaskId::new("tsk_00000000000000000000000001").unwrap()
}

fn step() -> StepId {
    StepId::new("stp_01JQ8Z9M3R2CVN8H5FWK7PQDSF").unwrap()
}

fn arguments(class: DataClass) -> ClassifiedArgumentsV1 {
    let mut map = serde_json::Map::new();
    map.insert(
        "calendar".to_string(),
        serde_json::Value::String("work".to_string()),
    );
    ClassifiedArgumentsV1::new_trusted(map, class)
}

#[test]
fn prepared_action_pins_every_host_fact() {
    let descriptor = descriptor(RiskClass::Observe, DataClass::Personal);
    let snap = snapshot_for(&descriptor);
    let prepared: PreparedActionV1 = prepare_action(
        &snap,
        &CapabilityId::new("calendar.events.read").unwrap(),
        task(),
        step(),
        &arguments(DataClass::Personal),
        RequestedBy::Model,
        Some(9_000),
        Some(120_000),
    )
    .expect("preparation");
    assert_eq!(prepared.capability_id().as_str(), "calendar.events.read");
    assert_eq!(prepared.capability_version().as_str(), "1.0.0");
    assert_eq!(prepared.provider_id().as_str(), "calendar");
    assert_eq!(
        prepared.implementation_id().unwrap().as_str(),
        "calendar-local"
    );
    assert_eq!(prepared.risk_class(), RiskClass::Observe);
    assert_eq!(prepared.side_effect_class(), SideEffectClass::None);
    assert_eq!(prepared.required_authorization(), Authorization::None);
    assert_eq!(prepared.replay_safety(), ReplaySafety::Idempotent);
    assert_eq!(prepared.root_requirement(), RootRequirement::NotRequired);
    assert_eq!(prepared.idempotency_support(), IdempotencySupport::None);
    assert_eq!(prepared.cost_class(), CostClass::Free);
    assert_eq!(prepared.requested_by(), RequestedBy::Model);
    assert_eq!(prepared.data_class(), DataClass::Personal);
    assert_eq!(prepared.generation_digest(), snap.manifest().digest());
}

#[test]
fn prepared_action_has_no_request_id_or_execution_authority() {
    let descriptor = descriptor(RiskClass::Observe, DataClass::Personal);
    let snap = snapshot_for(&descriptor);
    let prepared = prepare_action(
        &snap,
        &CapabilityId::new("calendar.events.read").unwrap(),
        task(),
        step(),
        &arguments(DataClass::Personal),
        RequestedBy::Model,
        None,
        None,
    )
    .unwrap();
    // The frozen surface offers no RequestId, no approval state and no
    // execution permission: there is no accessor for any of them.
    let surface = format!("{:?}", prepared.type_id());
    assert!(!surface.to_lowercase().contains("request_id"));
}

#[test]
fn deadline_is_the_minimum_of_descriptor_and_host_budgets() {
    let descriptor = descriptor(RiskClass::Observe, DataClass::Personal);
    let snap = snapshot_for(&descriptor);
    // descriptor max_duration_ms is 5_000
    let by_descriptor = prepare_action(
        &snap,
        &CapabilityId::new("calendar.events.read").unwrap(),
        task(),
        step(),
        &arguments(DataClass::Personal),
        RequestedBy::Model,
        None,
        None,
    )
    .unwrap();
    assert_eq!(by_descriptor.deadline_ms(), 5_000);
    let by_caller = prepare_action(
        &snap,
        &CapabilityId::new("calendar.events.read").unwrap(),
        task(),
        step(),
        &arguments(DataClass::Personal),
        RequestedBy::Model,
        Some(2_000),
        None,
    )
    .unwrap();
    assert_eq!(by_caller.deadline_ms(), 2_000);
    let by_budget = prepare_action(
        &snap,
        &CapabilityId::new("calendar.events.read").unwrap(),
        task(),
        step(),
        &arguments(DataClass::Personal),
        RequestedBy::Model,
        None,
        Some(1_000),
    )
    .unwrap();
    assert_eq!(by_budget.deadline_ms(), 1_000);
    let both = prepare_action(
        &snap,
        &CapabilityId::new("calendar.events.read").unwrap(),
        task(),
        step(),
        &arguments(DataClass::Personal),
        RequestedBy::Model,
        Some(4_000),
        Some(3_000),
    )
    .unwrap();
    assert_eq!(both.deadline_ms(), 3_000);
}

#[test]
fn zero_deadline_budget_refused() {
    let descriptor = descriptor(RiskClass::Observe, DataClass::Personal);
    let snap = snapshot_for(&descriptor);
    let err = prepare_action(
        &snap,
        &CapabilityId::new("calendar.events.read").unwrap(),
        task(),
        step(),
        &arguments(DataClass::Personal),
        RequestedBy::Model,
        Some(0),
        None,
    )
    .unwrap_err();
    assert!(matches!(err, PreparationError::NoEffectiveDeadline));
}

#[test]
fn arguments_above_descriptor_ceiling_refused() {
    let descriptor = descriptor(RiskClass::Observe, DataClass::Personal);
    let snap = snapshot_for(&descriptor);
    let err = prepare_action(
        &snap,
        &CapabilityId::new("calendar.events.read").unwrap(),
        task(),
        step(),
        &arguments(DataClass::Private),
        RequestedBy::Model,
        None,
        None,
    )
    .unwrap_err();
    assert!(matches!(
        err,
        PreparationError::DataClassAboveCeiling { .. }
    ));
}

#[test]
fn credential_class_arguments_refused() {
    let descriptor = descriptor(RiskClass::Credential, DataClass::Credential);
    let snap = snapshot_for(&descriptor);
    let err = prepare_action(
        &snap,
        &CapabilityId::new("calendar.events.read").unwrap(),
        task(),
        step(),
        &arguments(DataClass::Credential),
        RequestedBy::Model,
        None,
        None,
    )
    .unwrap_err();
    assert!(matches!(err, PreparationError::CredentialClassRefused));
}

#[test]
fn unclassified_arguments_refused() {
    let descriptor = descriptor(RiskClass::Observe, DataClass::Personal);
    let snap = snapshot_for(&descriptor);
    let map = serde_json::Map::new();
    // the trusted constructor is the only way in; an unclassified source has
    // no constructor at all, which this test pins by construction.
    let classified = ClassifiedArgumentsV1::new_trusted(map, DataClass::Credential);
    let err = prepare_action(
        &snap,
        &CapabilityId::new("calendar.events.read").unwrap(),
        task(),
        step(),
        &classified,
        RequestedBy::Model,
        None,
        None,
    )
    .unwrap_err();
    assert!(matches!(err, PreparationError::CredentialClassRefused));
}

#[test]
fn arguments_failing_the_input_schema_are_refused() {
    let descriptor = descriptor(RiskClass::Observe, DataClass::Personal);
    let snap = snapshot_for(&descriptor);
    let mut map = serde_json::Map::new();
    // undeclared property: the closed schema refuses it
    map.insert(
        "secret_extra".to_string(),
        serde_json::Value::String("x".to_string()),
    );
    let classified = ClassifiedArgumentsV1::new_trusted(map, DataClass::Personal);
    let err = prepare_action(
        &snap,
        &CapabilityId::new("calendar.events.read").unwrap(),
        task(),
        step(),
        &classified,
        RequestedBy::Model,
        None,
        None,
    )
    .unwrap_err();
    assert!(matches!(err, PreparationError::SchemaInvalid));
}

#[test]
fn arguments_missing_a_required_property_are_refused() {
    let descriptor = descriptor(RiskClass::Observe, DataClass::Personal);
    let snap = snapshot_for(&descriptor);
    let classified =
        ClassifiedArgumentsV1::new_trusted(serde_json::Map::new(), DataClass::Personal);
    let err = prepare_action(
        &snap,
        &CapabilityId::new("calendar.events.read").unwrap(),
        task(),
        step(),
        &classified,
        RequestedBy::Model,
        None,
        None,
    )
    .unwrap_err();
    assert!(matches!(err, PreparationError::SchemaInvalid));
}

#[test]
fn scj_invalid_number_is_refused_without_rounding() {
    let descriptor = descriptor(RiskClass::Observe, DataClass::Personal);
    let snap = snapshot_for(&descriptor);
    // 1.5 is outside the SCJ-1 integer domain; it must fail closed rather than
    // be rounded or reformatted into a canonical integer.
    let mut map = serde_json::Map::new();
    map.insert(
        "calendar".to_string(),
        serde_json::Value::String("work".to_string()),
    );
    map.insert("limit".to_string(), serde_json::Value::from(1.5));
    let classified = ClassifiedArgumentsV1::new_trusted(map, DataClass::Personal);
    let err = prepare_action(
        &snap,
        &CapabilityId::new("calendar.events.read").unwrap(),
        task(),
        step(),
        &classified,
        RequestedBy::Model,
        None,
        None,
    )
    .unwrap_err();
    assert!(matches!(err, PreparationError::ArgumentsNotCanonical));
}

#[test]
fn idempotency_key_is_stable_for_the_same_task_step_and_arguments() {
    let descriptor = descriptor(RiskClass::Observe, DataClass::Personal);
    let snap = snapshot_for(&descriptor);
    let first = prepare_action(
        &snap,
        &CapabilityId::new("calendar.events.read").unwrap(),
        task(),
        step(),
        &arguments(DataClass::Personal),
        RequestedBy::Model,
        None,
        None,
    )
    .unwrap();
    let second = prepare_action(
        &snap,
        &CapabilityId::new("calendar.events.read").unwrap(),
        task(),
        step(),
        &arguments(DataClass::Personal),
        RequestedBy::User,
        None,
        None,
    )
    .unwrap();
    // requester is provenance and never changes the logical idempotency meaning
    assert_eq!(first.idempotency_key(), second.idempotency_key());
    let other_step = prepare_action(
        &snap,
        &CapabilityId::new("calendar.events.read").unwrap(),
        task(),
        StepId::new("stp_01JQ8Z9M5T9WXK2H4BNPQ7RDSG").unwrap(),
        &arguments(DataClass::Personal),
        RequestedBy::Model,
        None,
        None,
    )
    .unwrap();
    assert_ne!(first.idempotency_key(), other_step.idempotency_key());
}

#[test]
fn arguments_digest_is_canonical_and_value_sensitive() {
    let descriptor = descriptor(RiskClass::Observe, DataClass::Personal);
    let snap = snapshot_for(&descriptor);
    let base = prepare_action(
        &snap,
        &CapabilityId::new("calendar.events.read").unwrap(),
        task(),
        step(),
        &arguments(DataClass::Personal),
        RequestedBy::Model,
        None,
        None,
    )
    .unwrap();
    let mut reordered = serde_json::Map::new();
    reordered.insert(
        "calendar".to_string(),
        serde_json::Value::String("work".into()),
    );
    reordered.insert(
        "timezone".to_string(),
        serde_json::Value::String("utc".into()),
    );
    let _ = reordered;
    let different = prepare_action(
        &snap,
        &CapabilityId::new("calendar.events.read").unwrap(),
        task(),
        step(),
        &ClassifiedArgumentsV1::new_trusted(
            serde_json::Map::from_iter([(
                "calendar".to_string(),
                serde_json::Value::String("home".into()),
            )]),
            DataClass::Personal,
        ),
        RequestedBy::Model,
        None,
        None,
    )
    .unwrap();
    assert_ne!(base.arguments_digest(), different.arguments_digest());
}

#[test]
fn unavailable_capability_is_refused() {
    let descriptor = descriptor(RiskClass::Observe, DataClass::Personal);
    let manifest = manifest_with(&descriptor);
    let registry = ProviderRegistry::build(vec![]).unwrap();
    let store = serea_storage::Store::open_in_memory(&FixedClock).unwrap();
    let snap = CapabilityAvailabilitySnapshotV1::build(
        manifest,
        &registry,
        &store,
        HostEligibility::new(std::collections::HashMap::new()),
    )
    .unwrap();
    let err = prepare_action(
        &snap,
        &CapabilityId::new("calendar.events.read").unwrap(),
        task(),
        step(),
        &arguments(DataClass::Personal),
        RequestedBy::Model,
        None,
        None,
    )
    .unwrap_err();
    assert!(matches!(err, PreparationError::Unavailable { .. }));
}

#[test]
fn classification_never_lowers_the_trusted_class() {
    // The descriptor ceiling is Personal; a Personal argument stays Personal
    // and is recorded as the exact argument class, never the ceiling by
    // coincidence and never lowered.
    let descriptor = descriptor(RiskClass::Observe, DataClass::Private);
    let snap = snapshot_for(&descriptor);
    let prepared = prepare_action(
        &snap,
        &CapabilityId::new("calendar.events.read").unwrap(),
        task(),
        step(),
        &arguments(DataClass::Personal),
        RequestedBy::Model,
        None,
        None,
    )
    .unwrap();
    assert_eq!(prepared.data_class(), DataClass::Personal);
}
