//! Private test-only Candidate C composition probe.
//!
//! The result deliberately records only that current durable pins were
//! observed. TaskEngine owns its transaction, so this module does not claim
//! cross-crate transaction atomicity or authorization.

use async_trait::async_trait;
use serea_capability::{
    CapabilityAvailabilitySnapshotV1, CapabilityManifestV1, CapabilitySchemaCatalogV1,
    HostEligibility, ManifestEntryV1, ProviderRegistry, descriptor_semantic_digest, install,
};
use serea_event_bus::EventBus;
use serea_protocol::provider::{CapabilityProvider, ProviderContext};
use serea_protocol::*;
use serea_storage::{LeaseGuard, Store};
use serea_task_engine::{NewTask, TaskEngine, TransitionContext};
use serea_testkit::DeterministicUlidSource;
use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

const SCHEMA: &str = "https://serea.local/schemas/";
static NEXT: AtomicU64 = AtomicU64::new(0);

fn at(value: i64) -> EpochMillis {
    EpochMillis::new(value).unwrap()
}

struct HostAdmission {
    session: u64,
    incarnation: u64,
    worker: String,
}

impl HostAdmission {
    fn admits(&self, session: u64, incarnation: u64, worker: &str) -> bool {
        self.session == session && self.incarnation == incarnation && self.worker == worker
    }
}

struct TestClock(EpochMillis);
impl Clock for TestClock {
    fn now_ms(&self) -> Result<EpochMillis, ProtocolError> {
        Ok(self.0)
    }
}
impl TestClock {
    fn unexpired(&self, expires_at: EpochMillis) -> bool {
        self.0 < expires_at
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TestHealth {
    Ready,
    Unready,
}

struct ProviderPin {
    provider: Arc<dyn CapabilityProvider>,
    id: ProviderId,
    generation: i64,
    health: TestHealth,
}

impl ProviderPin {
    fn matches(
        &self,
        candidate: &Arc<dyn CapabilityProvider>,
        id: &ProviderId,
        generation: i64,
    ) -> bool {
        Arc::ptr_eq(&self.provider, candidate)
            && &self.id == id
            && self.generation == generation
            && self.health == TestHealth::Ready
    }
}

struct FakeProvider(CapabilityDescriptor, Arc<AtomicU64>);
#[async_trait]
impl CapabilityProvider for FakeProvider {
    fn provider_id(&self) -> ProviderId {
        self.0.provider_id().clone()
    }
    fn capabilities(&self) -> Vec<CapabilityDescriptor> {
        vec![self.0.clone()]
    }
    async fn invoke(
        &self,
        _: &ActionRequest,
        _: &ProviderContext,
    ) -> Result<ActionResult, ActionError> {
        self.1.fetch_add(1, Ordering::SeqCst);
        panic!("Candidate C test-only probe must never invoke a Provider")
    }
    async fn health(&self) -> ProviderHealth {
        ProviderHealth::Ready
    }
}

struct TempDb(std::path::PathBuf);

fn database_fingerprint(path: &std::path::Path) -> Vec<(String, Vec<Vec<rusqlite::types::Value>>)> {
    let connection = rusqlite::Connection::open(path).unwrap();
    let mut tables = connection
        .prepare("SELECT name FROM sqlite_schema WHERE type='table' AND name NOT LIKE 'sqlite_%' ORDER BY name")
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
            let quoted = format!("\"{}\"", name.replace('\"', "\"\""));
            let mut statement = connection
                .prepare(&format!("SELECT * FROM {quoted}"))
                .unwrap();
            let columns = statement.column_count();
            let mut rows = statement.query([]).unwrap();
            let mut values = Vec::new();
            while let Some(row) = rows.next().unwrap() {
                values.push(
                    (0..columns)
                        .map(|index| row.get::<_, rusqlite::types::Value>(index).unwrap())
                        .collect(),
                );
            }
            (name, values)
        })
        .collect()
}

impl TempDb {
    fn new() -> Self {
        let directory = std::env::temp_dir().join(format!(
            "serea-candidate-c-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&directory).unwrap();
        Self(directory.join("store.sqlite"))
    }
    fn open(&self) -> Store {
        Store::open(&self.0, &TestClock(at(23))).expect("open real SQLite store")
    }
}
impl Drop for TempDb {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(self.0.parent().unwrap());
    }
}

fn descriptor() -> CapabilityDescriptor {
    CapabilityDescriptor::new(CapabilityDescriptorDraft {
        id: CapabilityId::new("calendar.events.read").unwrap(),
        version: SemVer::new("1.0.0").unwrap(),
        title: DescriptorTitle::new("Read events").unwrap(),
        description: DescriptorDescription::new("Candidate C private probe").unwrap(),
        provider_id: ProviderId::new("calendar").unwrap(),
        implementation_id: None,
        input_schema: JsonSchemaRef::new(format!("{SCHEMA}input.json")).unwrap(),
        output_schema: JsonSchemaRef::new(format!("{SCHEMA}output.json")).unwrap(),
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
    .unwrap()
}

struct Fixture {
    temp: TempDb,
    engine: TaskEngine,
    prepared: serea_capability::PreparedActionV1,
    guard: LeaseGuard,
    provider: Arc<dyn CapabilityProvider>,
    invoke_count: Arc<AtomicU64>,
    generation: i64,
}

fn fixture() -> Fixture {
    let temp = TempDb::new();
    let store = temp.open();
    let source = u64::from(std::process::id()) * 10_000 + NEXT.fetch_add(1, Ordering::Relaxed);
    let events =
        EventBus::new(DeterministicUlidSource::starting_at(1_700_000_000_000 + source).unwrap());
    let descriptor = descriptor();
    let mut documents = BTreeMap::new();
    documents.insert(
        format!("{SCHEMA}input.json"),
        r#"{"$schema":"https://json-schema.org/draft/2020-12/schema","type":"object","additionalProperties":false,"properties":{"calendar":{"type":"string","maxLength":64}}}"#.to_owned(),
    );
    documents.insert(
        format!("{SCHEMA}output.json"),
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
    let invoke_count = Arc::new(AtomicU64::new(0));
    let provider: Arc<dyn CapabilityProvider> =
        Arc::new(FakeProvider(descriptor.clone(), Arc::clone(&invoke_count)));
    let registry = ProviderRegistry::build(vec![Arc::clone(&provider)]).unwrap();
    registry.validate_advertisements(&manifest).unwrap();
    install(&store, &events, &manifest, &registry, at(20)).unwrap();
    let generation = store
        .current_registry_generation()
        .unwrap()
        .unwrap()
        .generation_id();
    let mut engine = TaskEngine::new(store, events.clone());
    let actor = ActorId::new("candidate-c-host-test").unwrap();
    let version = SemVer::new("0.2.0").unwrap();
    let cause = EventId::new("evt_00000000000000000000000001").unwrap();
    let context = TransitionContext {
        actor_kind: ActorKind::Host,
        actor_id: &actor,
        actor_version: &version,
        causation_id: Some(&cause),
    };
    let task_id = TaskId::new("tsk_00000000000000000000000001").unwrap();
    let step_id = StepId::new("stp_00000000000000000000000001").unwrap();
    engine
        .create_task(
            NewTask {
                task_id: task_id.clone(),
                kind: TaskKind::UserRequest,
                title: TaskTitle::new("Candidate C private probe").unwrap(),
                origin: TaskOrigin {
                    kind: TaskOriginKind::new("USER_MESSAGE").unwrap(),
                    device_id: None,
                    message_id: None,
                    extensions: [].into(),
                },
                data_class: DataClass::Personal,
                policy_class: RiskClass::Observe,
                attempt_budget: AttemptBudget {
                    max_model_calls: 1,
                    max_tool_calls: 1,
                    max_attempts_per_step: 3,
                    extensions: [].into(),
                },
                created_at: at(10),
                deadline_at: Some(at(1_000)),
                extensions: [].into(),
            },
            &context,
        )
        .unwrap();
    engine
        .start_planning(task_id.clone(), TaskState::Received, 0, at(21), &context)
        .unwrap();
    let mut eligibility = HashMap::new();
    for entry in manifest.entries() {
        eligibility.insert(entry.descriptor_digest.clone(), true);
    }
    let availability = block_on(CapabilityAvailabilitySnapshotV1::build_for_generation(
        manifest,
        generation,
        &registry,
        &temp.open(),
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
            task_id.clone(),
            step_id.clone(),
            1,
            descriptor.id(),
            &arguments,
            RequestedBy::Model,
            at(22),
            &context,
        )
        .unwrap();
    let prepared = serea_capability::prepare_action(
        &availability,
        descriptor.id(),
        task_id.clone(),
        step_id.clone(),
        &arguments,
        RequestedBy::Model,
        None,
        None,
    )
    .unwrap();
    let guard = temp
        .open()
        .transact(|tx| {
            tx.acquire_lease(
                task_id,
                step_id,
                LeaseOwner::new("candidate-c-host-worker").unwrap(),
                None,
                at(23),
                at(100),
            )
        })
        .unwrap();
    Fixture {
        temp,
        engine,
        prepared,
        guard,
        provider,
        invoke_count,
        generation,
    }
}

fn block_on<F: std::future::Future>(future: F) -> F::Output {
    use std::task::{Context, Poll, Wake, Waker};
    struct ThreadWake(std::thread::Thread);
    impl Wake for ThreadWake {
        fn wake(self: Arc<Self>) {
            self.0.unpark();
        }
        fn wake_by_ref(self: &Arc<Self>) {
            self.0.unpark();
        }
    }
    let waker = Waker::from(Arc::new(ThreadWake(std::thread::current())));
    let mut context = Context::from_waker(&waker);
    let mut future = Box::pin(future);
    loop {
        match future.as_mut().poll(&mut context) {
            Poll::Ready(value) => return value,
            Poll::Pending => std::thread::park(),
        }
    }
}

#[test]
fn private_host_composition_reads_real_engine_and_sqlite_facts_without_execution() {
    let fixture = fixture();
    let admission = HostAdmission {
        session: 7,
        incarnation: 4,
        worker: "candidate-c-worker".to_owned(),
    };
    assert!(admission.admits(7, 4, "candidate-c-worker"));
    assert!(!admission.admits(7, 5, "candidate-c-worker"));
    assert!(!admission.admits(8, 4, "candidate-c-worker"));

    let before = database_fingerprint(&fixture.temp.0);
    let observation = fixture
        .engine
        .observe_l1_facts(&fixture.prepared, &fixture.guard, None)
        .unwrap();
    assert!(observation.mismatches().is_empty());
    assert_eq!(before, database_fingerprint(&fixture.temp.0));
    assert_eq!(fixture.invoke_count.load(Ordering::SeqCst), 0);
    let clock = TestClock(at(24));
    assert!(clock.unexpired(at(100)));
    assert!(!TestClock(at(100)).unexpired(at(100)));

    let pin = ProviderPin {
        provider: Arc::clone(&fixture.provider),
        id: fixture.provider.provider_id(),
        generation: fixture.generation,
        health: TestHealth::Ready,
    };
    assert!(pin.matches(
        &fixture.provider,
        &fixture.provider.provider_id(),
        fixture.generation
    ));
    let replacement: Arc<dyn CapabilityProvider> =
        Arc::new(FakeProvider(descriptor(), Arc::new(AtomicU64::new(0))));
    assert!(!pin.matches(&replacement, &replacement.provider_id(), fixture.generation));
    assert!(!pin.matches(
        &fixture.provider,
        &fixture.provider.provider_id(),
        fixture.generation + 1
    ));
    assert!(
        !ProviderPin {
            health: TestHealth::Unready,
            ..pin
        }
        .matches(
            &fixture.provider,
            &fixture.provider.provider_id(),
            fixture.generation
        )
    );

    // The empty mismatch list is explicitly only a durable observation. This
    // test cannot couple it atomically to the Host's later checks.
    assert_eq!(fixture.invoke_count.load(Ordering::SeqCst), 0);
    drop(fixture.engine);
    drop(fixture.temp);
}

#[derive(Clone, PartialEq, Eq)]
struct FakeOwnerChallenge {
    session: u64,
    request_digest: [u8; 32],
    selected_steps: Vec<String>,
    nonce: u64,
    issued_at: EpochMillis,
    expires_at: EpochMillis,
}

#[derive(Default)]
struct FakeOwnerVerifier {
    consumed_nonces: std::collections::HashSet<u64>,
}

impl FakeOwnerVerifier {
    fn verify(
        &mut self,
        challenge: &FakeOwnerChallenge,
        session: u64,
        request_digest: [u8; 32],
        selected_steps: &[String],
        now: EpochMillis,
    ) -> bool {
        challenge.session == session
            && challenge.request_digest == request_digest
            && challenge.selected_steps == selected_steps
            && challenge.issued_at <= now
            && now < challenge.expires_at
            && self.consumed_nonces.insert(challenge.nonce)
    }
}

#[test]
fn fake_owner_challenge_is_bound_and_single_use_in_private_test_state() {
    let challenge = FakeOwnerChallenge {
        session: 7,
        request_digest: [3; 32],
        selected_steps: vec!["step-a".into(), "step-b".into()],
        nonce: 11,
        issued_at: at(20),
        expires_at: at(30),
    };
    let expected_steps = vec!["step-a".to_owned(), "step-b".to_owned()];
    let mut verifier = FakeOwnerVerifier::default();
    assert!(!verifier.verify(&challenge, 8, [3; 32], &expected_steps, at(24)));
    assert!(!verifier.verify(&challenge, 7, [4; 32], &expected_steps, at(24)));
    assert!(!verifier.verify(&challenge, 7, [3; 32], &["step-a".into()], at(24)));
    assert!(!verifier.verify(&challenge, 7, [3; 32], &expected_steps, at(30)));
    assert!(verifier.verify(&challenge, 7, [3; 32], &expected_steps, at(24)));
    assert!(!verifier.verify(&challenge, 7, [3; 32], &expected_steps, at(24)));
}

#[test]
fn reclaimed_real_sqlite_lease_fences_the_old_host_test_guard() {
    let fixture = fixture();
    let old_generation = fixture.guard.generation();
    let task_id = fixture.prepared.task_id().clone();
    let step_id = fixture.prepared.step_id().clone();
    let replacement = fixture
        .temp
        .open()
        .transact(|tx| {
            tx.acquire_lease(
                task_id,
                step_id,
                LeaseOwner::new("replacement-worker").unwrap(),
                Some(old_generation),
                at(100),
                at(200),
            )
        })
        .unwrap();
    assert_eq!(replacement.generation(), old_generation + 1);
    assert!(
        fixture
            .engine
            .observe_l1_facts(&fixture.prepared, &fixture.guard, None)
            .is_err()
    );
    let current = fixture
        .engine
        .observe_l1_facts(&fixture.prepared, &replacement, None)
        .unwrap();
    assert!(current.mismatches().is_empty());
}

struct LockProbeClock {
    path: std::path::PathBuf,
    busy_seen: Arc<std::sync::atomic::AtomicBool>,
}

impl Clock for LockProbeClock {
    fn now_ms(&self) -> Result<EpochMillis, ProtocolError> {
        let mut connection = rusqlite::Connection::open(&self.path).unwrap();
        connection.busy_timeout(std::time::Duration::ZERO).unwrap();
        let result = connection.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate);
        self.busy_seen.store(
            matches!(result, Err(rusqlite::Error::SqliteFailure(ref error, _)) if error.code == rusqlite::ErrorCode::DatabaseBusy || error.code == rusqlite::ErrorCode::DatabaseLocked),
            Ordering::SeqCst,
        );
        if let Ok(transaction) = result {
            transaction.rollback().unwrap();
        }
        Ok(at(24))
    }
}

#[test]
fn c1_observation_samples_clock_while_storage_holds_begin_immediate_and_is_read_only() {
    let fixture = fixture();
    let before = database_fingerprint(&fixture.temp.0);
    let busy_seen = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let clock = LockProbeClock {
        path: fixture.temp.0.clone(),
        busy_seen: Arc::clone(&busy_seen),
    };

    let (observation, sampled_at) = fixture
        .engine
        .observe_candidate_c_c1_test_only(&fixture.prepared, &fixture.guard, None, &clock)
        .unwrap();

    assert!(
        busy_seen.load(Ordering::SeqCst),
        "clock must run under BEGIN IMMEDIATE"
    );
    assert_eq!(sampled_at, at(24));
    assert!(observation.mismatches().is_empty());
    assert!(observation.candidate_c_grant_members().is_none());
    assert_eq!(database_fingerprint(&fixture.temp.0), before);
    assert_eq!(fixture.invoke_count.load(Ordering::SeqCst), 0);

    let mut released = rusqlite::Connection::open(&fixture.temp.0).unwrap();
    released.busy_timeout(std::time::Duration::ZERO).unwrap();
    released
        .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
        .unwrap()
        .rollback()
        .unwrap();
}
