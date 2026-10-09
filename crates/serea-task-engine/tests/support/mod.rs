use serea_event_bus::EventBus;
use serea_protocol::{
    Authorization, CapabilityDescriptor, CapabilityDescriptorDraft, CapabilityId, CostClass,
    DataClass, DescriptorDescription, DescriptorTitle, IdempotencySupport, JsonSchemaRef,
    ProviderId, ReplaySafety, RiskClass, RootRequirement, SemVer, SideEffectClass,
};
use serea_storage::{
    DescriptorRevisionDraft, GenerationMemberDraft, RegistryGenerationDraft, Store,
};
use serea_task_engine::TaskEngine;
use serea_testkit::DeterministicUlidSource;
use std::sync::atomic::{AtomicU64, Ordering};

#[allow(dead_code)]
static NEXT_EVENT_SOURCE: AtomicU64 = AtomicU64::new(0);

#[allow(dead_code)]
pub fn event_bus() -> EventBus {
    let offset = NEXT_EVENT_SOURCE.fetch_add(1_000, Ordering::Relaxed);
    let process = u64::from(std::process::id());
    let start = 1_700_000_000_000 + process * 10_000 + offset;
    EventBus::new(DeterministicUlidSource::starting_at(start).unwrap())
}

/// Explicitly seeds post-P5 registry state for a new runtime fixture.
/// Reopening a fixture must call `engine`, never this function implicitly.
pub fn seed_active_registry_generation(store: &Store) {
    if store.current_registry_generation().unwrap().is_none() {
        let descriptor = CapabilityDescriptor::new(CapabilityDescriptorDraft {
            id: CapabilityId::new("calendar.events.create").unwrap(),
            version: SemVer::new("1.0.0").unwrap(),
            title: DescriptorTitle::new("Fixture").unwrap(),
            description: DescriptorDescription::new("Fixture descriptor").unwrap(),
            provider_id: ProviderId::new("calendar").unwrap(),
            implementation_id: None,
            input_schema: JsonSchemaRef::new("https://serea.local/schemas/test-input.json")
                .unwrap(),
            output_schema: JsonSchemaRef::new("https://serea.local/schemas/test-output.json")
                .unwrap(),
            side_effect_class: SideEffectClass::None,
            risk_class: RiskClass::Observe,
            required_authorization: Authorization::None,
            replay_safety: ReplaySafety::Idempotent,
            data_class: DataClass::Personal,
            root_requirement: RootRequirement::NotRequired,
            idempotency_support: IdempotencySupport::None,
            max_duration_ms: 1_000,
            cost_class: CostClass::Free,
            experimental: false,
        })
        .unwrap();
        let descriptor_digest = serea_protocol::digest_of("\"test-fixture-descriptor\"").unwrap();
        let schema_digest = serea_protocol::digest_of("\"test-fixture-schema\"").unwrap();
        let generation = store
            .create_registry_generation(RegistryGenerationDraft {
                manifest_digest: serea_protocol::digest_of("\"test-fixture-manifest\"").unwrap(),
                schema_catalog_digest: schema_digest.clone(),
            })
            .unwrap();
        store
            .insert_descriptor_revision(DescriptorRevisionDraft {
                descriptor_digest: descriptor_digest.clone(),
                descriptor: descriptor.clone(),
                input_schema_digest: schema_digest.clone(),
                output_schema_digest: schema_digest,
            })
            .unwrap();
        store
            .add_generation_membership(GenerationMemberDraft {
                generation_id: generation.generation_id(),
                descriptor_digest,
                candidate_priority: 0,
            })
            .unwrap();
        store
            .set_generation_default_version(
                generation.generation_id(),
                descriptor.id().clone(),
                descriptor.version().clone(),
            )
            .unwrap();
        store
            .transact(|tx| {
                tx.activate_registry_generation(
                    generation.generation_id(),
                    serea_protocol::EpochMillis::new(0).unwrap(),
                )
                .map(|_| ())
            })
            .unwrap();
    }
}

#[allow(dead_code)]
pub fn fixture_descriptor_digest() -> serea_protocol::Digest {
    serea_protocol::digest_of("\"test-fixture-descriptor\"").unwrap()
}

/// Pure constructor. Reopening a database through this helper performs no
/// writes and does not repair or activate registry state.
#[allow(dead_code)]
pub fn engine(store: Store, events: EventBus) -> TaskEngine {
    TaskEngine::new(store, events)
}

/// Convenience for simple post-P5 tests that are creating a fresh fixture.
/// Crash/recovery tests should seed explicitly before their fault window and
/// use `engine` for every open thereafter.
#[allow(dead_code)]
pub fn post_p5_engine(store: Store, events: EventBus) -> TaskEngine {
    seed_active_registry_generation(&store);
    TaskEngine::new(store, events)
}
