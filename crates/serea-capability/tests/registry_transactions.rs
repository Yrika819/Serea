use serea_capability::CapabilityRegistry;
use serea_event_bus::EventBus;
use serea_protocol::{
    Authorization, CapabilityDescriptor, CapabilityDescriptorDraft, CapabilityId, Clock, CostClass,
    DataClass, DescriptorDescription, DescriptorTitle, Digest, EpochMillis, IdempotencySupport,
    JsonSchemaRef, ProtocolError, ProviderId, ReplaySafety, RiskClass, RootRequirement, SemVer,
    SideEffectClass,
};
use serea_storage::fault::{Action, Window};
use serea_storage::{
    CapabilityOverlayState, DescriptorRevisionDraft, GenerationMemberDraft,
    RegistryGenerationDraft, ReplayItem, Store, StoreError,
};
use serea_testkit::DeterministicUlidSource;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Barrier};
use std::thread;

static NEXT: AtomicU64 = AtomicU64::new(0);

struct TempDb(PathBuf);
impl TempDb {
    fn new() -> Self {
        let dir = std::env::temp_dir().join(format!(
            "serea-p5b-transactions-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&dir).unwrap();
        Self(dir.join("store.sqlite"))
    }
}
impl Drop for TempDb {
    fn drop(&mut self) {
        std::fs::remove_dir_all(self.0.parent().unwrap()).unwrap();
    }
}

struct FixedClock;
impl Clock for FixedClock {
    fn now_ms(&self) -> Result<EpochMillis, ProtocolError> {
        EpochMillis::new(1_796_000_000_000)
    }
}

fn digest(ch: char) -> Digest {
    Digest::new(format!("sha256:{}", ch.to_string().repeat(64))).unwrap()
}

fn descriptor() -> CapabilityDescriptor {
    CapabilityDescriptor::new(CapabilityDescriptorDraft {
        id: CapabilityId::new("calendar.events.read").unwrap(),
        version: SemVer::new("1.0.0").unwrap(),
        title: DescriptorTitle::new("Read events").unwrap(),
        description: DescriptorDescription::new("Read private calendar events").unwrap(),
        provider_id: ProviderId::new("calendar").unwrap(),
        implementation_id: None,
        input_schema: JsonSchemaRef::new("serea://calendar/input").unwrap(),
        output_schema: JsonSchemaRef::new("serea://calendar/output").unwrap(),
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

fn prepared(store: &Store) -> i64 {
    let generation = CapabilityRegistry::create_generation(
        store,
        RegistryGenerationDraft {
            manifest_digest: digest('c'),
            schema_catalog_digest: digest('d'),
        },
    )
    .unwrap();
    CapabilityRegistry::insert_descriptor_revision(
        store,
        DescriptorRevisionDraft {
            descriptor_digest: digest('1'),
            descriptor: descriptor(),
            input_schema_digest: digest('a'),
            output_schema_digest: digest('b'),
        },
    )
    .unwrap();
    CapabilityRegistry::add_generation_member(
        store,
        GenerationMemberDraft {
            generation_id: generation.generation_id(),
            descriptor_digest: digest('1'),
            candidate_priority: 0,
        },
    )
    .unwrap();
    CapabilityRegistry::set_default_version(
        store,
        generation.generation_id(),
        CapabilityId::new("calendar.events.read").unwrap(),
        SemVer::new("1.0.0").unwrap(),
    )
    .unwrap();
    generation.generation_id()
}

fn event_payloads(store: &Store) -> Vec<serea_protocol::SereaEvent> {
    let page = store.replay_events(None, None, 256).unwrap();
    page.items
        .into_iter()
        .filter_map(|item| match item {
            ReplayItem::Event { event } => Some(*event),
            _ => None,
        })
        .collect()
}

#[test]
fn activation_and_event_commit_once_and_event_is_metadata_only() {
    let store = Store::open_in_memory(&FixedClock).unwrap();
    let events = EventBus::new(DeterministicUlidSource::new());
    let generation_id = prepared(&store);
    CapabilityRegistry::activate_generation(
        &store,
        &events,
        generation_id,
        EpochMillis::new(1_796_000_000_000).unwrap(),
    )
    .unwrap();
    assert_eq!(
        CapabilityRegistry::current_generation(&store)
            .unwrap()
            .unwrap()
            .generation_id(),
        generation_id
    );
    assert_eq!(event_payloads(&store).len(), 1);
    let payload = &event_payloads(&store)[0];
    assert_eq!(payload.payload["change_kind"], "GENERATION_ACTIVATED");
    assert_eq!(payload.payload["generation_id"], generation_id);
    for forbidden in [
        "title",
        "description",
        "arguments",
        "schema",
        "secret",
        "credentials",
    ] {
        assert!(
            !payload.payload.contains_key(forbidden),
            "event leaked {forbidden}"
        );
    }
    assert_eq!(
        CapabilityRegistry::activate_generation(
            &store,
            &events,
            generation_id,
            EpochMillis::new(1_796_000_000_000).unwrap()
        ),
        Err(StoreError::RegistryGenerationActivated)
    );
    assert_eq!(event_payloads(&store).len(), 1);
}

#[test]
fn activation_rolls_back_when_event_append_fails() {
    let store = Store::open_in_memory(&FixedClock).unwrap();
    let events = EventBus::new(DeterministicUlidSource::new());
    let generation_id = prepared(&store);
    Window::BeforeEventAppend
        .arm(Action::Fail(StoreError::Sqlite))
        .unwrap();
    assert_eq!(
        CapabilityRegistry::activate_generation(
            &store,
            &events,
            generation_id,
            EpochMillis::new(1_796_000_000_000).unwrap()
        ),
        Err(StoreError::Sqlite)
    );
    assert!(
        CapabilityRegistry::current_generation(&store)
            .unwrap()
            .is_none()
    );
    assert_eq!(
        CapabilityRegistry::generation(&store, generation_id)
            .unwrap()
            .unwrap()
            .activated_at(),
        None
    );
    assert!(event_payloads(&store).is_empty());
}

#[test]
fn overlay_update_is_optimistic_atomic_and_metadata_only() {
    let store = Store::open_in_memory(&FixedClock).unwrap();
    let events = EventBus::new(DeterministicUlidSource::new());
    let capability = CapabilityId::new("calendar.events.read").unwrap();
    let overlay = CapabilityRegistry::set_overlay(
        &store,
        &events,
        capability.clone(),
        0,
        CapabilityOverlayState::Disabled,
        false,
        EpochMillis::new(1_796_000_000_000).unwrap(),
    )
    .unwrap();
    assert_eq!(overlay.revision(), 1);
    assert_eq!(event_payloads(&store)[0].payload["change_kind"], "DISABLED");
    assert_eq!(
        event_payloads(&store)[0].payload["capability_id"],
        "calendar.events.read"
    );
    assert_eq!(
        CapabilityRegistry::set_overlay(
            &store,
            &events,
            capability.clone(),
            0,
            CapabilityOverlayState::Enabled,
            false,
            EpochMillis::new(1_796_000_000_000).unwrap(),
        ),
        Err(StoreError::RegistryOverlayConflict)
    );
    let enabled = CapabilityRegistry::set_overlay(
        &store,
        &events,
        capability.clone(),
        1,
        CapabilityOverlayState::Enabled,
        true,
        EpochMillis::new(1_796_000_000_000).unwrap(),
    )
    .unwrap();
    assert_eq!(enabled.revision(), 2);
    let payloads = event_payloads(&store);
    assert_eq!(payloads[1].payload["change_kind"], "ENABLED");
    assert_eq!(payloads[2].payload["change_kind"], "EXPERIMENTAL_OPT_IN");
    for payload in payloads {
        for forbidden in [
            "title",
            "description",
            "schema_uri",
            "arguments",
            "credentials",
        ] {
            assert!(!payload.payload.contains_key(forbidden));
        }
    }
}

#[test]
fn overlay_write_rolls_back_when_event_append_fails() {
    let store = Store::open_in_memory(&FixedClock).unwrap();
    let events = EventBus::new(DeterministicUlidSource::new());
    let capability = CapabilityId::new("calendar.events.read").unwrap();
    Window::BeforeEventAppend
        .arm(Action::Fail(StoreError::Sqlite))
        .unwrap();
    assert_eq!(
        CapabilityRegistry::set_overlay(
            &store,
            &events,
            capability.clone(),
            0,
            CapabilityOverlayState::Removed,
            false,
            EpochMillis::new(1_796_000_000_000).unwrap(),
        ),
        Err(StoreError::Sqlite)
    );
    let current = CapabilityRegistry::overlay(&store, &capability).unwrap();
    assert_eq!(current.state(), CapabilityOverlayState::Enabled);
    assert_eq!(current.revision(), 0);
    assert!(event_payloads(&store).is_empty());
}

#[test]
fn overlay_removed_reactivated_and_experimental_opt_out_are_durable_events() {
    let temp = TempDb::new();
    let events = EventBus::new(DeterministicUlidSource::new());
    let capability = CapabilityId::new("calendar.events.read").unwrap();
    {
        let store = Store::open(&temp.0, &FixedClock).unwrap();
        let removed = CapabilityRegistry::set_overlay(
            &store,
            &events,
            capability.clone(),
            0,
            CapabilityOverlayState::Removed,
            false,
            EpochMillis::new(1_796_000_000_000).unwrap(),
        )
        .unwrap();
        let reactivated = CapabilityRegistry::set_overlay(
            &store,
            &events,
            capability.clone(),
            removed.revision(),
            CapabilityOverlayState::Enabled,
            true,
            EpochMillis::new(1_796_000_000_001).unwrap(),
        )
        .unwrap();
        let opted_out = CapabilityRegistry::set_overlay(
            &store,
            &events,
            capability.clone(),
            reactivated.revision(),
            CapabilityOverlayState::Enabled,
            false,
            EpochMillis::new(1_796_000_000_002).unwrap(),
        )
        .unwrap();
        assert_eq!(opted_out.revision(), 3);
        assert_eq!(
            event_payloads(&store)
                .iter()
                .map(|event| event.payload["change_kind"].as_str().unwrap())
                .collect::<Vec<_>>(),
            [
                "REMOVED",
                "REACTIVATED",
                "EXPERIMENTAL_OPT_IN",
                "EXPERIMENTAL_OPT_OUT"
            ],
        );
    }
    let reopened = Store::open(&temp.0, &FixedClock).unwrap();
    let overlay = CapabilityRegistry::overlay(&reopened, &capability).unwrap();
    assert_eq!(overlay.state(), CapabilityOverlayState::Enabled);
    assert!(!overlay.experimental_opt_in());
    assert_eq!(overlay.revision(), 3);
    assert_eq!(event_payloads(&reopened).len(), 4);
}

#[test]
fn prepared_generation_remains_inactive_across_close_and_reopen() {
    let temp = TempDb::new();
    let generation_id;
    {
        let store = Store::open(&temp.0, &FixedClock).unwrap();
        generation_id = prepared(&store);
    }
    let reopened = Store::open(&temp.0, &FixedClock).unwrap();
    assert!(
        CapabilityRegistry::current_generation(&reopened)
            .unwrap()
            .is_none()
    );
    assert_eq!(
        CapabilityRegistry::generation(&reopened, generation_id)
            .unwrap()
            .unwrap()
            .activated_at(),
        None
    );
}

#[test]
fn committed_activation_and_event_survive_caller_loss_and_reopen_once() {
    let temp = TempDb::new();
    let events = EventBus::new(DeterministicUlidSource::new());
    let generation_id;
    {
        let store = Store::open(&temp.0, &FixedClock).unwrap();
        generation_id = prepared(&store);
        CapabilityRegistry::activate_generation(
            &store,
            &events,
            generation_id,
            EpochMillis::new(1_796_000_000_000).unwrap(),
        )
        .unwrap();
    }
    let reopened = Store::open(&temp.0, &FixedClock).unwrap();
    assert_eq!(
        CapabilityRegistry::current_generation(&reopened)
            .unwrap()
            .unwrap()
            .generation_id(),
        generation_id
    );
    assert_eq!(event_payloads(&reopened).len(), 1);
    assert_eq!(
        event_payloads(&reopened)[0].payload["change_kind"],
        "GENERATION_ACTIVATED"
    );
}

#[test]
fn independent_connections_serialize_generation_activation_without_lost_event() {
    let temp = TempDb::new();
    let events = EventBus::new(DeterministicUlidSource::new());
    let (low, high);
    {
        let store = Store::open(&temp.0, &FixedClock).unwrap();
        low = prepared(&store);
        high = prepared(&store);
    }
    let first_store = Store::open(&temp.0, &FixedClock).unwrap();
    let second_store = Store::open(&temp.0, &FixedClock).unwrap();
    let barrier = Arc::new(Barrier::new(3));
    let first_barrier = Arc::clone(&barrier);
    let first_events = events.clone();
    let first = thread::spawn(move || {
        first_barrier.wait();
        CapabilityRegistry::activate_generation(
            &first_store,
            &first_events,
            low,
            EpochMillis::new(1_796_000_000_000).unwrap(),
        )
    });
    let second_barrier = Arc::clone(&barrier);
    let second_events = events.clone();
    let second = thread::spawn(move || {
        second_barrier.wait();
        CapabilityRegistry::activate_generation(
            &second_store,
            &second_events,
            high,
            EpochMillis::new(1_796_000_000_001).unwrap(),
        )
    });
    barrier.wait();
    let results = [first.join().unwrap(), second.join().unwrap()];
    let successes = results.iter().filter(|result| result.is_ok()).count();
    assert!(successes >= 1);
    let reopened = Store::open(&temp.0, &FixedClock).unwrap();
    assert_eq!(
        CapabilityRegistry::current_generation(&reopened)
            .unwrap()
            .unwrap()
            .generation_id(),
        high
    );
    let payloads = event_payloads(&reopened);
    assert_eq!(payloads.len(), successes);
    let event_generations: Vec<i64> = payloads
        .iter()
        .map(|event| event.payload["generation_id"].as_i64().unwrap())
        .collect();
    assert_eq!(
        event_generations,
        if successes == 2 {
            vec![low, high]
        } else {
            vec![high]
        }
    );
}

#[test]
fn independent_connections_allocate_distinct_monotonic_generation_ids() {
    let temp = TempDb::new();
    let first_store = Store::open(&temp.0, &FixedClock).unwrap();
    let second_store = Store::open(&temp.0, &FixedClock).unwrap();
    let barrier = Arc::new(Barrier::new(3));
    let first_barrier = Arc::clone(&barrier);
    let first = thread::spawn(move || {
        first_barrier.wait();
        CapabilityRegistry::create_generation(
            &first_store,
            RegistryGenerationDraft {
                manifest_digest: digest('1'),
                schema_catalog_digest: digest('a'),
            },
        )
        .unwrap()
        .generation_id()
    });
    let second_barrier = Arc::clone(&barrier);
    let second = thread::spawn(move || {
        second_barrier.wait();
        CapabilityRegistry::create_generation(
            &second_store,
            RegistryGenerationDraft {
                manifest_digest: digest('2'),
                schema_catalog_digest: digest('b'),
            },
        )
        .unwrap()
        .generation_id()
    });
    barrier.wait();
    let mut ids = [first.join().unwrap(), second.join().unwrap()];
    ids.sort_unstable();
    assert!(ids[0] > 0);
    assert!(ids[1] > ids[0]);
}

#[test]
fn independent_overlay_updates_with_one_expected_revision_conflict_cleanly() {
    let temp = TempDb::new();
    let events = EventBus::new(DeterministicUlidSource::new());
    let first_store = Store::open(&temp.0, &FixedClock).unwrap();
    let second_store = Store::open(&temp.0, &FixedClock).unwrap();
    let barrier = Arc::new(Barrier::new(3));
    let first_barrier = Arc::clone(&barrier);
    let first_events = events.clone();
    let first = thread::spawn(move || {
        first_barrier.wait();
        CapabilityRegistry::set_overlay(
            &first_store,
            &first_events,
            CapabilityId::new("calendar.events.read").unwrap(),
            0,
            CapabilityOverlayState::Disabled,
            false,
            EpochMillis::new(1_796_000_000_000).unwrap(),
        )
    });
    let second_barrier = Arc::clone(&barrier);
    let second_events = events.clone();
    let second = thread::spawn(move || {
        second_barrier.wait();
        CapabilityRegistry::set_overlay(
            &second_store,
            &second_events,
            CapabilityId::new("calendar.events.read").unwrap(),
            0,
            CapabilityOverlayState::Removed,
            false,
            EpochMillis::new(1_796_000_000_001).unwrap(),
        )
    });
    barrier.wait();
    let results = [first.join().unwrap(), second.join().unwrap()];
    assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
    assert_eq!(
        results
            .iter()
            .filter(|result| **result == Err(StoreError::RegistryOverlayConflict))
            .count(),
        1
    );
    let reopened = Store::open(&temp.0, &FixedClock).unwrap();
    assert_eq!(
        CapabilityRegistry::overlay(
            &reopened,
            &CapabilityId::new("calendar.events.read").unwrap()
        )
        .unwrap()
        .revision(),
        1
    );
    assert_eq!(event_payloads(&reopened).len(), 1);
}
