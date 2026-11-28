//! Host-only diagnostic: Core writes a synthetic campaign marker, not an Android death test.
use serea_core::Core;
use serea_event_bus::EventBus;
use serea_protocol::*;
use serea_storage::{
    DescriptorRevisionDraft, GenerationMemberDraft, RegistryGenerationDraft, Store,
};
use serea_task_engine::{NewTask, TransitionContext};
use serea_testkit::{DeterministicUlidSource, TestClock};

fn install_registry(path: &std::path::Path, clock: &TestClock) {
    let store = Store::open(path, clock).unwrap();
    assert!(store.current_registry_generation().unwrap().is_none());
    let generation = store
        .create_registry_generation(RegistryGenerationDraft {
            manifest_digest: digest_of("\"serea-wal-fixture-manifest\"").unwrap(),
            schema_catalog_digest: digest_of("\"serea-wal-fixture-catalog\"").unwrap(),
        })
        .unwrap()
        .generation_id();
    let descriptor = CapabilityDescriptor::new(CapabilityDescriptorDraft {
        id: CapabilityId::new("synthetic.echo.read").unwrap(),
        version: SemVer::new("1.0.0").unwrap(),
        title: DescriptorTitle::new("Synthetic no-op").unwrap(),
        description: DescriptorDescription::new("Synthetic-only no-op fixture").unwrap(),
        provider_id: ProviderId::new("synthetic").unwrap(),
        implementation_id: None,
        input_schema: JsonSchemaRef::new("serea://synthetic/input").unwrap(),
        output_schema: JsonSchemaRef::new("serea://synthetic/output").unwrap(),
        side_effect_class: SideEffectClass::None,
        risk_class: RiskClass::Observe,
        required_authorization: Authorization::None,
        replay_safety: ReplaySafety::Idempotent,
        data_class: DataClass::Public,
        root_requirement: RootRequirement::NotRequired,
        idempotency_support: IdempotencySupport::None,
        max_duration_ms: 1_000,
        cost_class: CostClass::Free,
        experimental: true,
    })
    .unwrap();
    let digest = digest_of("\"serea-wal-fixture-descriptor\"").unwrap();
    let schema = digest_of("\"serea-wal-fixture-schema\"").unwrap();
    store
        .insert_descriptor_revision(DescriptorRevisionDraft {
            descriptor_digest: digest.clone(),
            descriptor: descriptor.clone(),
            input_schema_digest: schema.clone(),
            output_schema_digest: schema,
        })
        .unwrap();
    store
        .add_generation_membership(GenerationMemberDraft {
            generation_id: generation,
            descriptor_digest: digest,
            candidate_priority: 0,
        })
        .unwrap();
    store
        .set_generation_default_version(
            generation,
            descriptor.id().clone(),
            descriptor.version().clone(),
        )
        .unwrap();
    store
        .transact(|tx| {
            tx.activate_registry_generation(generation, clock.now_ms().unwrap())
                .map(|_| ())
        })
        .unwrap();
}

#[test]
fn core_campaign_wal_reopen_and_integrity() {
    let directory = std::env::temp_dir().join(format!(
        "serea-core-campaign-wal-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    std::fs::create_dir(&directory).expect("fresh synthetic directory required");
    let path = directory.join("campaign.sqlite");
    let wal = directory.join("campaign.sqlite-wal");
    let clock = TestClock::at_epoch();
    let events = EventBus::new(DeterministicUlidSource::new());
    install_registry(&path, &clock);
    let mut core = Core::open(&path, &clock, events.clone()).expect("open Core");
    let before = std::fs::metadata(&wal).map_or(0, |m| m.len());
    let id = TaskId::new("tsk_00000000000000000000000001").unwrap();
    let title = TaskTitle::new("s1b0123456789 synthetic WAL marker").unwrap();
    let actor = ActorId::new("serea-synthetic-host").unwrap();
    let actor_version = SemVer::new("0.1.0").unwrap();
    let cause = EventId::new("evt_00000000000000000000000001").unwrap();
    let context = TransitionContext {
        actor_kind: ActorKind::Host,
        actor_id: &actor,
        actor_version: &actor_version,
        causation_id: Some(&cause),
    };
    core.tasks()
        .create_task(
            NewTask {
                task_id: id.clone(),
                kind: TaskKind::UserRequest,
                title: title.clone(),
                origin: TaskOrigin {
                    kind: TaskOriginKind::new("SYNTHETIC").unwrap(),
                    device_id: None,
                    message_id: None,
                    extensions: Extensions::new(),
                },
                data_class: DataClass::Personal,
                policy_class: RiskClass::Observe,
                attempt_budget: AttemptBudget {
                    max_model_calls: 1,
                    max_tool_calls: 0,
                    max_attempts_per_step: 1,
                    extensions: Extensions::new(),
                },
                created_at: clock.now_ms().unwrap(),
                deadline_at: None,
                extensions: Extensions::new(),
            },
            &context,
        )
        .expect("Core commits the campaign task");
    let after = std::fs::metadata(&wal).expect("Core WAL exists").len();
    assert!(
        after > before && after > 32,
        "Core write must append WAL frames"
    );
    assert_eq!(core.load_task(id.clone()).unwrap().task.title, title);
    core.shutdown().expect("clean shutdown");
    let reopened = Core::open(&path, &clock, events).expect("reopen Core");
    assert_eq!(reopened.load_task(id).unwrap().task.title, title);
    let verification_store = Store::open(&path, &clock).expect("open verification store");
    verification_store
        .verify_integrity()
        .expect("SQLite and application integrity");
    drop(verification_store);
    reopened.shutdown().expect("clean shutdown after reopen");
}
