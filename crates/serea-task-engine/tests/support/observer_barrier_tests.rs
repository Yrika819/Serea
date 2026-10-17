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

/// Test-only logical snapshot of every ordinary SQLite table and value. This
/// is used to prove that observation changes no durable row, not just counts.
fn database_fingerprint(path: &std::path::Path) -> Vec<(String, Vec<Vec<rusqlite::types::Value>>)> {
    let connection = rusqlite::Connection::open(path).unwrap();
    let mut tables = connection
        .prepare(
            "SELECT name FROM sqlite_schema
             WHERE type='table' AND name NOT LIKE 'sqlite_%' ORDER BY name",
        )
        .unwrap();
    let names = tables
        .query_map([], |row| row.get::<_, String>(0))
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    drop(tables);

    names
        .into_iter()
        .map(|name| {
            let quoted = format!("\"{}\"", name.replace('"', "\"\""));
            let mut statement = connection
                .prepare(&format!("SELECT * FROM {quoted}"))
                .unwrap();
            let column_count = statement.column_count();
            let mut rows = statement.query([]).unwrap();
            let mut values = Vec::new();
            while let Some(row) = rows.next().unwrap() {
                let row_values = (0..column_count)
                    .map(|index| row.get::<_, rusqlite::types::Value>(index).unwrap())
                    .collect();
                values.push(row_values);
            }
            (name, values)
        })
        .collect()
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

fn grant_descriptor() -> CapabilityDescriptor {
    CapabilityDescriptor::new(CapabilityDescriptorDraft {
        id: CapabilityId::new("calendar.events.read").unwrap(),
        version: SemVer::new("1.0.0").unwrap(),
        title: DescriptorTitle::new("Read events").unwrap(),
        description: DescriptorDescription::new("Grant observer race fixture").unwrap(),
        provider_id: ProviderId::new("calendar").unwrap(),
        implementation_id: None,
        input_schema: JsonSchemaRef::new(format!("{SCHEMA}cal-input.json")).unwrap(),
        output_schema: JsonSchemaRef::new(format!("{SCHEMA}cal-output.json")).unwrap(),
        side_effect_class: SideEffectClass::ExternalWrite,
        risk_class: RiskClass::ExternalWrite,
        required_authorization: Authorization::ScopedGrant,
        replay_safety: ReplaySafety::Idempotent,
        data_class: DataClass::Personal,
        root_requirement: RootRequirement::NotRequired,
        idempotency_support: IdempotencySupport::Native,
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
    CapabilityAvailabilitySnapshotV1,
) {
    setup_for(temp, descriptor(), RiskClass::Observe)
}

fn setup_for(
    temp: &TempDb,
    descriptor: CapabilityDescriptor,
    task_policy_class: RiskClass,
) -> (
    TaskEngine,
    serea_capability::PreparedActionV1,
    LeaseGuard,
    EventBus,
    CapabilityAvailabilitySnapshotV1,
) {
    let store = temp.open();
    let events = event_bus();
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
                policy_class: task_policy_class,
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
    (engine, prepared, guard, events, availability)
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

#[test]
fn observer_success_and_fact_mismatch_leave_every_durable_row_unchanged() {
    let temp = TempDb::new();
    let (engine, prepared, guard, _, availability) = setup(&temp);
    let before = database_fingerprint(&temp.0);

    let observed = engine.observe_l1_facts(&prepared, &guard, None).unwrap();
    assert!(observed.mismatches().is_empty());
    assert!(observed.grant().is_none());

    let mut alternate = serde_json::Map::new();
    alternate.insert(
        "calendar".into(),
        serde_json::Value::String("observer-mismatch".into()),
    );
    let alternate_args =
        serea_capability::ClassifiedArgumentsV1::new_trusted(alternate, DataClass::Personal);
    let mismatched = serea_capability::prepare_action(
        &availability,
        &CapabilityId::new("calendar.events.read").unwrap(),
        task_id(),
        step_id(),
        &alternate_args,
        RequestedBy::Model,
        None,
        None,
    )
    .unwrap();
    let mismatch = engine.observe_l1_facts(&mismatched, &guard, None).unwrap();
    assert!(
        mismatch
            .mismatches()
            .contains(&DurableFactMismatch::StepInputDigest)
    );

    let rollback = engine.observe_l1_facts_while_transaction_held(&prepared, &guard, None, |_| {
        Err(StoreError::Sqlite)
    });
    assert!(rollback.is_err());

    assert_eq!(before, database_fingerprint(&temp.0));
    let reopened = temp.open();
    reopened.verify_integrity().unwrap();
    assert_eq!(before, database_fingerprint(&temp.0));
}

fn run_race(
    temp: &TempDb,
    engine: TaskEngine,
    prepared: serea_capability::PreparedActionV1,
    guard: LeaseGuard,
    grant_id: Option<serea_protocol::GrantId>,
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
    let durable_before = database_fingerprint(&temp.0);
    let (held_tx, held_rx) = mpsc::sync_channel(1);
    let (release_tx, release_rx) = mpsc::sync_channel(1);
    let observer_grant_id = grant_id.clone();
    let observer = std::thread::spawn(move || {
        let observation = engine
            .observe_l1_facts_while_transaction_held(
                &prepared,
                &guard,
                observer_grant_id.as_ref(),
                |facts| {
                    held_tx.send(()).map_err(|_| StoreError::Sqlite)?;
                    release_rx
                        .recv_timeout(Duration::from_secs(60))
                        .map_err(|_| StoreError::Sqlite)?;
                    if facts.mismatches().is_empty() {
                        Ok(())
                    } else {
                        Err(StoreError::InvalidPolicyRevision)
                    }
                },
            )
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
        // Storage preserves SQLITE_BUSY/LOCKED as StoreError::Busy. Keep the
        // actual typed result across the barrier so a fixture error cannot be
        // mistaken for contention.
        blocked_tx.send(first).unwrap();
        writer_release_rx
            .recv_timeout(Duration::from_secs(60))
            .unwrap();
        let retry = Store::open(&writer_path, &FixedClock).unwrap();
        writer(&retry).unwrap();
    });
    let blocked_result = blocked_rx.recv_timeout(Duration::from_secs(30)).unwrap();
    let durable_unchanged_while_blocked = durable_before == database_fingerprint(&temp.0);
    release_tx.send(()).unwrap();
    let (old_observation, prepared, guard) = observer.join().unwrap();
    assert!(old_observation.mismatches().is_empty());
    writer_release_tx.send(()).unwrap();
    writer_thread.join().unwrap();
    assert_eq!(
        blocked_result,
        Err(StoreError::Busy),
        "typed writer must fail specifically because the observer holds SQLite's writer lock"
    );
    assert!(
        durable_unchanged_while_blocked,
        "observer and blocked writer must leave every durable row unchanged"
    );

    let reopened = temp.open();
    reopened.verify_integrity().unwrap();
    drop(reopened);
    if recheck_old_guard {
        let fresh_engine = TaskEngine::new(temp.open(), event_bus());
        let fresh = fresh_engine
            .observe_l1_facts(&prepared, &guard, grant_id.as_ref())
            .unwrap();
        assert!(fresh.mismatches().is_empty());
    }
    (old_observation, prepared, guard)
}

#[test]
fn overlay_writer_is_blocked_by_observer_transaction_and_commits_after_release() {
    let temp = TempDb::new();
    let (engine, prepared, guard, _, _) = setup(&temp);
    let (old_observation, _, _) = run_race(
        &temp,
        engine,
        prepared,
        guard,
        None,
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
    let (engine, prepared, guard, events, _) = setup(&temp);
    let context = Context::new();
    let (old, _, _) = run_race(
        &temp,
        engine,
        prepared,
        guard,
        None,
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
    let (engine, prepared, guard, events, _) = setup(&temp);
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
        None,
        move |store| {
            serea_policy::activate_revision(
                store,
                &events,
                &pending,
                at(32),
                SemVer::new("0.2.0").unwrap(),
            )
            .map(|_| ())
            .map_err(|error| match error {
                serea_policy::PolicyError::Storage(storage_error) => storage_error,
                _ => StoreError::Sqlite,
            })
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
    let (engine, prepared, guard, _, _) = setup(&temp);
    let old_generation = guard.generation();
    let new_generation = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0));
    let record_generation = new_generation.clone();
    let (old, prepared, old_guard) = run_race(
        &temp,
        engine,
        prepared,
        guard,
        None,
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

#[test]
fn authenticated_grant_revocation_is_blocked_by_grant_observer_transaction() {
    let temp = TempDb::new();
    let (engine, prepared, guard, events, _) =
        setup_for(&temp, grant_descriptor(), RiskClass::ExternalWrite);
    let approval_id = ApprovalId::new("apr_01JQ8ZB8H2XKM9P4QW7NRT5YCD").unwrap();
    let grant_id = GrantId::new("grt_01JQ8ZB8H2XKM9P4QW7NRT5YCE").unwrap();
    let actor = ActorId::new("user-observer-fixture").unwrap();
    let device = DeviceId::new("dev_01JQ8ZB8H2XKM9P4QW7NRT5YCD").unwrap();
    let session = SessionId::new("ses_01JQ8ZB8H2XKM9P4QW7NRT5YCD").unwrap();
    let action = serea_policy::ApprovalActionV1::new(
        step_id(),
        prepared.arguments_digest().clone(),
        serde_json::json!({"calendar":"work"}),
        DataClass::Personal,
    )
    .unwrap();
    let request = serea_policy::ApprovalRequestV1::new(serea_policy::ApprovalRequestDraftV1 {
        approval_id: approval_id.clone(),
        task_id: task_id(),
        task_kind: TaskKind::UserRequest,
        plan_revision: 1,
        capability_id: prepared.capability_id().clone(),
        capability_version: prepared.capability_version().clone(),
        registry_generation: prepared.generation_id(),
        descriptor_digest: prepared.descriptor_digest().clone(),
        risk_class: RiskClass::ExternalWrite,
        side_effect_class: SideEffectClass::ExternalWrite,
        authorization: Authorization::ScopedGrant,
        data_class: DataClass::Personal,
        requested_by: RequestedBy::Model,
        actions: serea_policy::ApprovalActionSetV1::new(vec![action]).unwrap(),
        summary: serea_policy::SafeApprovalSummaryV1::from_host_builder(
            "calendar.events.read",
            "Read calendar events",
            DataClass::Personal,
        )
        .unwrap(),
        raised_at: at(24),
        expires_at: at(1_000),
    })
    .unwrap();
    let required_event = events
        .draft_approval_lifecycle(
            serea_event_bus::ApprovalLifecycleKindV1::Required,
            &approval_id,
            &task_id(),
            &step_id(),
            ActorKind::Host,
            ActorId::new("observer-test-host").unwrap(),
            SemVer::new("0.2.0").unwrap(),
            at(24),
        )
        .unwrap();
    let store = temp.open();
    store
        .create_approval_request(request.storage_draft(), required_event)
        .unwrap();

    struct AcceptResponse;
    impl serea_policy::ApprovalAuthenticationVerifier for AcceptResponse {
        fn verify(
            &self,
            _: &serea_policy::ApprovalResponseCandidateV1,
        ) -> Result<(), serea_policy::ApprovalAuthenticationError> {
            Ok(())
        }
    }
    let candidate = serea_policy::ApprovalResponseCandidateV1::new(
        approval_id.clone(),
        serea_policy::ApprovalResponseDecisionV1::Grant,
        serea_policy::ApprovalPrincipalV1::User,
        Some(device.clone()),
        Some(session.clone()),
        serea_policy::ApprovalAuthenticationStrengthV1::Session,
        at(25),
        None,
        vec![step_id()],
        Some(1),
        Some(at(900)),
    );
    let authenticated =
        serea_policy::AuthenticatedApprovalResponseV1::authenticate(candidate, &AcceptResponse)
            .unwrap();
    let response = authenticated.validate_for(&request, at(26)).unwrap();
    let response_actor = actor.clone();
    let grant_event = events
        .draft_approval_lifecycle(
            serea_event_bus::ApprovalLifecycleKindV1::Granted,
            &approval_id,
            &task_id(),
            &step_id(),
            ActorKind::User,
            response_actor.clone(),
            SemVer::new("0.2.0").unwrap(),
            at(26),
        )
        .unwrap();
    store
        .respond_to_approval(
            response.storage_draft(Some(grant_id.clone()), response_actor),
            grant_event,
        )
        .unwrap();

    let active = engine
        .observe_l1_facts(&prepared, &guard, Some(&grant_id))
        .unwrap();
    assert!(active.mismatches().is_empty());
    let active_grant = active.grant().unwrap();
    assert_eq!(
        active_grant.status(),
        serea_storage::GrantFactStatus::Active
    );
    assert_eq!(active_grant.action_count(), 1);
    assert_eq!(active_grant.member_count(), 1);
    assert!(active_grant.sealed());
    assert!(!active_grant.already_used());
    assert_eq!(active_grant.uses_remaining(), 1);

    struct AcceptRevocation;
    impl serea_policy::ApprovalRevocationAuthenticationVerifier for AcceptRevocation {
        fn verify(
            &self,
            _: &serea_policy::ApprovalRevocationCandidateV1,
        ) -> Result<(), serea_policy::ApprovalAuthenticationError> {
            Ok(())
        }
    }
    let revocation = serea_policy::AuthenticatedApprovalRevocationV1::authenticate(
        serea_policy::ApprovalRevocationCandidateV1::new(
            approval_id.clone(),
            grant_id.clone(),
            serea_policy::ApprovalPrincipalV1::User,
            actor.clone(),
            Some(device.clone()),
            Some(session.clone()),
            at(30),
        ),
        &AcceptRevocation,
    )
    .unwrap()
    .validate_for(&approval_id, &grant_id, at(31))
    .unwrap();
    let revoke_events = events.clone();
    let revoke_actor = actor;
    let revoke_approval_id = approval_id.clone();
    let revoke_grant_id = grant_id.clone();
    let before_revoke = database_fingerprint(&temp.0);
    let connection = rusqlite::Connection::open(&temp.0).unwrap();
    let events_before: i64 = connection
        .query_row("SELECT count(*) FROM event_content", [], |row| row.get(0))
        .unwrap();
    let uses_before: i64 = connection
        .query_row("SELECT count(*) FROM approval_grant_uses", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(uses_before, 0);
    drop(connection);

    let (old_observation, _, _) = run_race(
        &temp,
        engine,
        prepared,
        guard,
        Some(grant_id.clone()),
        move |store| {
            let event = revoke_events
                .draft_approval_revoked(
                    &revoke_approval_id,
                    &revoke_grant_id,
                    &task_id(),
                    &step_id(),
                    "USER",
                    revoke_actor.clone(),
                    ActorKind::User,
                    Some(&device),
                    Some(&session),
                    SemVer::new("0.2.0").unwrap(),
                    at(30),
                )
                .unwrap();
            store
                .revoke_approval_grant(revocation.storage_draft(), event)
                .map(|_| ())
        },
        true,
    );
    let old_grant = old_observation.grant().unwrap();
    assert_eq!(old_grant.status(), serea_storage::GrantFactStatus::Active);
    assert_eq!(old_grant.uses_remaining(), 1);
    assert!(!old_grant.already_used());

    let reopened = temp.open();
    reopened.verify_integrity().unwrap();
    let fresh = reopened
        .transact(|tx| tx.observe_grant_facts(&grant_id, &task_id(), &step_id()))
        .unwrap()
        .unwrap();
    assert_eq!(fresh.status(), serea_storage::GrantFactStatus::Revoked);
    assert_eq!(fresh.uses_remaining(), 1);
    assert!(!fresh.already_used());
    let connection = rusqlite::Connection::open(&temp.0).unwrap();
    let events_after: i64 = connection
        .query_row("SELECT count(*) FROM event_content", [], |row| row.get(0))
        .unwrap();
    let uses_after: i64 = connection
        .query_row("SELECT count(*) FROM approval_grant_uses", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(events_after, events_before + 1);
    assert_eq!(uses_after, uses_before);
    assert_ne!(before_revoke, database_fingerprint(&temp.0));
}
