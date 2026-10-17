use super::*;
use async_trait::async_trait;
use serea_capability::{
    CapabilityAvailabilitySnapshotV1, CapabilityManifestV1, CapabilitySchemaCatalogV1,
    HostEligibility, ManifestEntryV1, ProviderRegistry, descriptor_semantic_digest, install,
};
use serea_protocol::provider::CapabilityProvider;
use serea_protocol::*;
use serea_storage::{CapabilityOverlayState, Store};
use serea_testkit::DeterministicUlidSource;
use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc;
use std::time::Duration;

const SCHEMA: &str = "https://serea.local/schemas/";
static NEXT: AtomicU64 = AtomicU64::new(0);

struct FixedClock;
impl Clock for FixedClock {
    fn now_ms(&self) -> Result<EpochMillis, ProtocolError> {
        Ok(at(1_796_000_000_000))
    }
}

fn at(value: i64) -> EpochMillis {
    EpochMillis::new(value).unwrap()
}
fn task_id() -> TaskId {
    TaskId::new("tsk_00000000000000000000000001").unwrap()
}
fn step_id() -> StepId {
    StepId::new("stp_00000000000000000000000001").unwrap()
}

struct TempDb(PathBuf);
impl TempDb {
    fn new() -> Self {
        let dir = std::env::temp_dir().join(format!(
            "serea-observer-barrier-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&dir).unwrap();
        Self(dir.join("store.sqlite"))
    }
    fn open(&self) -> Store {
        Store::open(&self.0, &FixedClock).unwrap()
    }
}
impl Drop for TempDb {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(self.0.parent().unwrap());
    }
}

fn event_bus() -> EventBus {
    let source = u64::from(std::process::id()) * 10_000 + NEXT.fetch_add(1, Ordering::Relaxed);
    EventBus::new(DeterministicUlidSource::starting_at(1_700_000_000_000 + source).unwrap())
}

fn descriptor() -> CapabilityDescriptor {
    CapabilityDescriptor::new(CapabilityDescriptorDraft {
        id: CapabilityId::new("calendar.events.read").unwrap(),
        version: SemVer::new("1.0.0").unwrap(),
        title: DescriptorTitle::new("Read events").unwrap(),
        description: DescriptorDescription::new("Observer barrier fixture").unwrap(),
        provider_id: ProviderId::new("calendar").unwrap(),
        implementation_id: None,
        input_schema: JsonSchemaRef::new(format!("{SCHEMA}cal-input.json")).unwrap(),
        output_schema: JsonSchemaRef::new(format!("{SCHEMA}cal-output.json")).unwrap(),
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

struct FixtureProvider(CapabilityDescriptor);
#[async_trait]
impl CapabilityProvider for FixtureProvider {
    fn provider_id(&self) -> ProviderId {
        self.0.provider_id().clone()
    }
    fn capabilities(&self) -> Vec<CapabilityDescriptor> {
        vec![self.0.clone()]
    }
    async fn invoke(
        &self,
        _: &ActionRequest,
        _: &serea_protocol::provider::ProviderContext,
    ) -> Result<ActionResult, ActionError> {
        panic!("observer must never invoke a Provider")
    }
    async fn health(&self) -> ProviderHealth {
        ProviderHealth::Ready
    }
}

struct Context {
    actor: ActorId,
    version: SemVer,
    cause: EventId,
}
impl Context {
    fn new() -> Self {
        Self {
            actor: ActorId::new("observer-test-host").unwrap(),
            version: SemVer::new("0.2.0").unwrap(),
            cause: EventId::new("evt_00000000000000000000000001").unwrap(),
        }
    }
    fn view(&self) -> TransitionContext<'_> {
        TransitionContext {
            actor_kind: ActorKind::Host,
            actor_id: &self.actor,
            actor_version: &self.version,
            causation_id: Some(&self.cause),
        }
    }
}

fn setup(
    temp: &TempDb,
) -> (
    TaskEngine,
    serea_capability::PreparedActionV1,
    LeaseGuard,
    EventBus,
) {
    let store = temp.open();
    let events = event_bus();
    let descriptor = descriptor();
    let mut documents = BTreeMap::new();
    documents.insert(
            format!("{SCHEMA}cal-input.json"),
            r#"{"$schema":"https://json-schema.org/draft/2020-12/schema","type":"object","additionalProperties":false,"properties":{"calendar":{"type":"string","maxLength":64}}}"#.to_owned(),
        );
    documents.insert(
            format!("{SCHEMA}cal-output.json"),
            r#"{"$schema":"https://json-schema.org/draft/2020-12/schema","type":"object","additionalProperties":false,"properties":{"ok":{"type":"boolean"}}}"#.to_owned(),
        );
    let catalog = CapabilitySchemaCatalogV1::build(documents).unwrap();
    let manifest = CapabilityManifestV1::build(
        vec![ManifestEntryV1 {
            descriptor: descriptor.clone(),
            candidate_priority: 0,
            descriptor_digest: descriptor_semantic_digest(&descriptor, &catalog).unwrap(),
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
    .unwrap();
    let providers: Vec<std::sync::Arc<dyn CapabilityProvider>> =
        vec![std::sync::Arc::new(FixtureProvider(descriptor.clone()))];
    let registry = ProviderRegistry::build(providers.clone()).unwrap();
    registry.validate_advertisements(&manifest).unwrap();
    install(&store, &events, &manifest, &registry, at(20)).unwrap();

    let context = Context::new();
    let mut engine = TaskEngine::new(store, events.clone());
    engine
        .create_task(
            NewTask {
                task_id: task_id(),
                kind: TaskKind::UserRequest,
                title: TaskTitle::new("observer barrier fixture").unwrap(),
                origin: TaskOrigin {
                    kind: TaskOriginKind::new("USER_MESSAGE").unwrap(),
                    device_id: None,
                    message_id: None,
                    extensions: [].into(),
                },
                data_class: DataClass::Personal,
                policy_class: RiskClass::Observe,
                attempt_budget: AttemptBudget {
                    max_model_calls: 2,
                    max_tool_calls: 2,
                    max_attempts_per_step: 3,
                    extensions: [].into(),
                },
                created_at: at(10),
                deadline_at: Some(at(1_000_000)),
                extensions: [].into(),
            },
            &context.view(),
        )
        .unwrap();
    engine
        .start_planning(task_id(), TaskState::Received, 0, at(21), &context.view())
        .unwrap();
    let generation = engine
        .store
        .current_registry_generation()
        .unwrap()
        .unwrap()
        .generation_id();
    let mut eligibility = HashMap::new();
    for entry in manifest.entries() {
        eligibility.insert(entry.descriptor_digest.clone(), true);
    }
    let availability = block_on(CapabilityAvailabilitySnapshotV1::build_for_generation(
        manifest,
        generation,
        &registry,
        &engine.store,
        HostEligibility::new(eligibility),
    ))
    .unwrap();
    let arguments = serea_capability::ClassifiedArgumentsV1::new_trusted(
        serde_json::json!({"calendar":"work"})
            .as_object()
            .unwrap()
            .clone(),
        DataClass::Personal,
    );
    engine
        .create_capability_step(
            &availability,
            task_id(),
            step_id(),
            1,
            descriptor.id(),
            &arguments,
            RequestedBy::Model,
            at(22),
            &context.view(),
        )
        .unwrap();
    let prepared = serea_capability::prepare_action(
        &availability,
        descriptor.id(),
        task_id(),
        step_id(),
        &arguments,
        RequestedBy::Model,
        None,
        None,
    )
    .unwrap();
    let guard = engine
        .store
        .transact(|tx| {
            tx.acquire_lease(
                task_id(),
                step_id(),
                LeaseOwner::new("worker-fixture").unwrap(),
                None,
                at(23),
                at(100),
            )
        })
        .unwrap();
    (engine, prepared, guard, events)
}

fn block_on<F: std::future::Future>(future: F) -> F::Output {
    use std::task::{Context as TaskContext, Poll, Wake, Waker};
    struct ThreadWake(std::thread::Thread);
    impl Wake for ThreadWake {
        fn wake(self: std::sync::Arc<Self>) {
            self.0.unpark();
        }
        fn wake_by_ref(self: &std::sync::Arc<Self>) {
            self.0.unpark();
        }
    }
    let waker = Waker::from(std::sync::Arc::new(ThreadWake(std::thread::current())));
    let mut context = TaskContext::from_waker(&waker);
    let mut future = Box::pin(future);
    loop {
        match future.as_mut().poll(&mut context) {
            Poll::Ready(value) => return value,
            Poll::Pending => std::thread::park(),
        }
    }
}

fn run_race(
    temp: &TempDb,
    engine: TaskEngine,
    prepared: serea_capability::PreparedActionV1,
    guard: LeaseGuard,
    writer: impl Fn(&Store) -> Result<(), StoreError> + Send + 'static,
    recheck_old_guard: bool,
) -> (
    L1FactObservation,
    serea_capability::PreparedActionV1,
    LeaseGuard,
) {
    let writer_store = temp.open();
    let writer_path = temp.0.clone();
    let witness = rusqlite::Connection::open(&temp.0).unwrap();
    witness.busy_timeout(Duration::ZERO).unwrap();
    let (held_tx, held_rx) = mpsc::sync_channel(1);
    let (release_tx, release_rx) = mpsc::sync_channel(1);
    let observer = std::thread::spawn(move || {
        let observation = engine
            .observe_l1_facts_while_transaction_held(&prepared, &guard, None, |facts| {
                held_tx.send(()).map_err(|_| StoreError::Sqlite)?;
                release_rx
                    .recv_timeout(Duration::from_secs(60))
                    .map_err(|_| StoreError::Sqlite)?;
                if facts.mismatches().is_empty() {
                    Ok(())
                } else {
                    Err(StoreError::InvalidPolicyRevision)
                }
            })
            .unwrap();
        (observation, prepared, guard)
    });
    held_rx.recv_timeout(Duration::from_secs(5)).unwrap();
    let busy = witness.execute_batch("BEGIN IMMEDIATE").unwrap_err();
    assert!(matches!(
        busy.sqlite_error_code(),
        Some(rusqlite::ErrorCode::DatabaseBusy | rusqlite::ErrorCode::DatabaseLocked)
    ));
    drop(witness);

    let (blocked_tx, blocked_rx) = mpsc::sync_channel(1);
    let (writer_release_tx, writer_release_rx) = mpsc::sync_channel(1);
    let writer_thread = std::thread::spawn(move || {
        let first = writer(&writer_store);
        blocked_tx.send(first.is_err()).unwrap();
        writer_release_rx
            .recv_timeout(Duration::from_secs(60))
            .unwrap();
        let retry = Store::open(&writer_path, &FixedClock).unwrap();
        writer(&retry).unwrap();
    });
    assert!(
        blocked_rx.recv_timeout(Duration::from_secs(30)).unwrap(),
        "writer unexpectedly committed under observer lock"
    );
    release_tx.send(()).unwrap();
    let (old_observation, prepared, guard) = observer.join().unwrap();
    assert!(old_observation.mismatches().is_empty());
    writer_release_tx.send(()).unwrap();
    writer_thread.join().unwrap();

    let reopened = temp.open();
    reopened.verify_integrity().unwrap();
    drop(reopened);
    if recheck_old_guard {
        let fresh_engine = TaskEngine::new(temp.open(), event_bus());
        let fresh = fresh_engine
            .observe_l1_facts(&prepared, &guard, None)
            .unwrap();
        assert!(fresh.mismatches().is_empty());
    }
    (old_observation, prepared, guard)
}

#[test]
fn overlay_writer_is_blocked_by_observer_transaction_and_commits_after_release() {
    let temp = TempDb::new();
    let (engine, prepared, guard, _) = setup(&temp);
    let (old_observation, _, _) = run_race(
        &temp,
        engine,
        prepared,
        guard,
        |store| {
            store.transact(|tx| {
                tx.set_capability_overlay(
                    CapabilityId::new("calendar.events.read").unwrap(),
                    0,
                    CapabilityOverlayState::Disabled,
                    false,
                )
                .map(|_| ())
            })
        },
        true,
    );
    assert_eq!(old_observation.overlay().unwrap().revision(), 0);
    assert_eq!(
        old_observation.overlay().unwrap().state(),
        CapabilityOverlayState::Enabled
    );
    let reopened = temp.open();
    assert_eq!(
        reopened
            .get_capability_overlay(&CapabilityId::new("calendar.events.read").unwrap())
            .unwrap()
            .revision(),
        1
    );
    assert_eq!(
        reopened
            .get_capability_overlay(&CapabilityId::new("calendar.events.read").unwrap())
            .unwrap()
            .state(),
        CapabilityOverlayState::Disabled
    );
}

#[test]
fn cancellation_writer_is_blocked_by_observer_transaction() {
    let temp = TempDb::new();
    let (engine, prepared, guard, events) = setup(&temp);
    let context = Context::new();
    let (old, _, _) = run_race(
        &temp,
        engine,
        prepared,
        guard,
        move |store| {
            store.transact_with_participants(&crate::journal::TaskJournal, &events, |tx| {
                tx.cancel_task(
                    &task_id(),
                    TaskOriginKind::new("USER_CANCEL").unwrap(),
                    at(30),
                    &context.view(),
                )
                .map(|_| ())
            })
        },
        true,
    );
    assert_eq!(old.task().task.state, TaskState::Ready);
    assert!(
        temp.open()
            .load_task(&task_id())
            .unwrap()
            .task
            .cancelled_at
            .is_some()
    );
}

#[test]
fn policy_activation_writer_is_blocked_by_observer_transaction() {
    let temp = TempDb::new();
    let (engine, prepared, guard, events) = setup(&temp);
    let policy_store = temp.open();
    let pending = serea_policy::prepare_revision(
        &policy_store,
        vec![
            serea_policy::PolicyRule::new(
                "allow_observer_fixture",
                10,
                true,
                serea_policy::RuleMatch {
                    capability_id: Some(CapabilityId::new("calendar.events.read").unwrap()),
                    ..Default::default()
                },
                serea_policy::RuleDecision::Allow,
            )
            .unwrap(),
        ],
        at(31),
        serea_storage::PolicyActorClass::Host,
        ActorId::new("policy-host").unwrap(),
        ReasonCode::new("POLICY_IMPORT").unwrap(),
    )
    .unwrap();
    let (old, _, _) = run_race(
        &temp,
        engine,
        prepared,
        guard,
        move |store| {
            serea_policy::activate_revision(
                store,
                &events,
                &pending,
                at(32),
                SemVer::new("0.2.0").unwrap(),
            )
            .map(|_| ())
            .map_err(|_| StoreError::Sqlite)
        },
        true,
    );
    assert!(old.policy().is_none());
    let reopened = temp.open();
    assert!(
        serea_policy::load_active_revision(&reopened)
            .unwrap()
            .is_some()
    );
}

#[test]
fn expired_lease_reclaim_is_blocked_by_observer_transaction_and_advances_fence() {
    let temp = TempDb::new();
    let (engine, prepared, guard, _) = setup(&temp);
    let old_generation = guard.generation();
    let new_generation = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0));
    let record_generation = new_generation.clone();
    let (old, prepared, old_guard) = run_race(
        &temp,
        engine,
        prepared,
        guard,
        move |store| {
            let new_guard = store.transact(|tx| {
                tx.acquire_lease(
                    task_id(),
                    step_id(),
                    LeaseOwner::new("replacement-worker").unwrap(),
                    Some(old_generation),
                    at(101),
                    at(200),
                )
            })?;
            record_generation.store(new_guard.generation(), Ordering::SeqCst);
            Ok(())
        },
        false,
    );
    assert_eq!(old.lease().generation(), old_generation);
    assert_eq!(new_generation.load(Ordering::SeqCst), old_generation + 1);
    let reopened = temp.open();
    reopened.verify_integrity().unwrap();
    assert_eq!(
        reopened.load_task(&task_id()).unwrap().steps[0]
            .step
            .lease_generation,
        Some(old_generation + 1)
    );
    assert!(
        TaskEngine::new(temp.open(), event_bus())
            .observe_l1_facts(&prepared, &old_guard, None)
            .is_err()
    );
}
