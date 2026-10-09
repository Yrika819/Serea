use serea_protocol::{
    Authorization, CapabilityDescriptor, CapabilityDescriptorDraft, CapabilityId, CostClass,
    DataClass, DescriptorDescription, DescriptorTitle, EpochMillis, IdempotencySupport,
    JsonSchemaRef, ProviderId, ReplaySafety, RiskClass, RootRequirement, SemVer, SideEffectClass,
    digest_of,
};
use serea_storage::{
    DescriptorRevisionDraft, GenerationMemberDraft, RegistryGenerationDraft, Store,
};

/// Seeds stable post-P5 registry state during initial test fixture setup.
/// Reopen/recovery paths must use a pure TaskEngine constructor and never call this.
pub fn seed_active_registry(store: &Store) {
    if store.current_registry_generation().unwrap().is_some() {
        return;
    }
    let descriptor = CapabilityDescriptor::new(CapabilityDescriptorDraft {
        id: CapabilityId::new("calendar.events.create").unwrap(),
        version: SemVer::new("1.0.0").unwrap(),
        title: DescriptorTitle::new("Scheduler fixture").unwrap(),
        description: DescriptorDescription::new("Scheduler fixture descriptor").unwrap(),
        provider_id: ProviderId::new("calendar").unwrap(),
        implementation_id: None,
        input_schema: JsonSchemaRef::new("serea://scheduler/input").unwrap(),
        output_schema: JsonSchemaRef::new("serea://scheduler/output").unwrap(),
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
    let generation_id = store
        .create_registry_generation(RegistryGenerationDraft {
            manifest_digest: digest_of("\"scheduler-manifest\"").unwrap(),
            schema_catalog_digest: digest_of("\"scheduler-catalog\"").unwrap(),
        })
        .unwrap()
        .generation_id();
    let descriptor_digest = digest_of("\"scheduler-descriptor\"").unwrap();
    let schema_digest = digest_of("\"scheduler-schema\"").unwrap();
    store
        .insert_descriptor_revision(DescriptorRevisionDraft {
            descriptor_digest: descriptor_digest.clone(),
            descriptor,
            input_schema_digest: schema_digest.clone(),
            output_schema_digest: schema_digest,
        })
        .unwrap();
    store
        .add_generation_membership(GenerationMemberDraft {
            generation_id,
            descriptor_digest,
            candidate_priority: 0,
        })
        .unwrap();
    store
        .set_generation_default_version(
            generation_id,
            CapabilityId::new("calendar.events.create").unwrap(),
            SemVer::new("1.0.0").unwrap(),
        )
        .unwrap();
    store
        .transact(|tx| {
            tx.activate_registry_generation(generation_id, EpochMillis::new(0).unwrap())
                .map(|_| ())
        })
        .unwrap();
}
