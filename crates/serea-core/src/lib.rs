//! Process-level construction and lifecycle coordination for Serea's durable
//! components. Core deliberately accepts time and identifier sources from its
//! host boundary; it does not invent ambient clocks or randomness.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

use std::path::{Path, PathBuf};

use serea_capability::CapabilityRegistry;
use serea_event_bus::EventBus;
use serea_protocol::TaskId;
use serea_protocol::{
    Authorization, CapabilityDescriptor, CapabilityDescriptorDraft, CapabilityId, Clock, DataClass,
    DescriptorDescription, DescriptorTitle, EpochMillis, IdempotencySupport, JsonSchemaRef,
    ReplaySafety, RiskClass, RootRequirement, SemVer, SideEffectClass, digest_of,
};
use serea_scheduler::{ScheduleError, Scheduler, SchedulerRecoveryReport};
use serea_storage::{
    CheckpointOutcome, DescriptorRevisionDraft, EventReplayPage, GenerationMemberDraft,
    RegistryGenerationDraft, Store, StoreError,
};
use serea_task_engine::{EngineError, TaskEngine, TaskRecord, TransitionContext};

/// Content-free audit sequence facts used by synthetic host verification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AuditSequenceSummary {
    /// Durable event records inspected in the returned page.
    pub event_count: u64,
    /// Number of durable TaskCreated events in the page.
    pub task_created_events: u64,
    /// Whether event sequence values are strictly increasing and gapless.
    pub strictly_ordered: bool,
}

/// Typed failures during composition and startup.
#[derive(Debug)]
pub enum CoreError {
    /// Durable storage could not be opened or validated.
    Storage(StoreError),
    /// Initial capability registry generation could not be installed.
    Registry(StoreError),
    /// Startup recovery refused the durable state.
    Recovery(ScheduleError),
    /// Task Engine refused a host operation.
    TaskEngine(EngineError),
    /// A host-supplied or synthetic protocol value was rejected.
    Protocol(serea_protocol::ProtocolError),
    /// Canonical JSON or digest construction failed.
    Canonical(serea_protocol::CanonicalJsonError),
}

impl std::fmt::Display for CoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Storage(_) => "core storage initialization failed",
            Self::Registry(_) => "core capability registry initialization failed",
            Self::Recovery(_) => "core startup recovery failed",
            Self::TaskEngine(_) => "core task operation failed",
            Self::Protocol(_) => "core protocol value was rejected",
            Self::Canonical(_) => "core canonical value was rejected",
        })
    }
}

impl std::error::Error for CoreError {}

/// Composed synchronous Core. SQLite work remains synchronous and each Store
/// method owns its bounded transaction; no write transaction crosses an await.
pub struct Core {
    path: PathBuf,
    store: Store,
    events: EventBus,
    tasks: TaskEngine,
}

impl Core {
    /// Opens the canonical durable store and wires the Event Bus and Task
    /// Engine. Opening Core does not install providers, capabilities, or
    /// authority implicitly.
    pub fn open(path: &Path, clock: &dyn Clock, events: EventBus) -> Result<Self, CoreError> {
        let store = Store::open(path, clock).map_err(CoreError::Storage)?;
        let task_store = Store::open(path, clock).map_err(CoreError::Storage)?;
        let tasks = TaskEngine::new(task_store, events.clone());
        Ok(Self {
            path: path.to_path_buf(),
            store,
            events,
            tasks,
        })
    }

    /// Explicitly installs the disabled synthetic registry fixture used by
    /// development smoke flows. Normal Core startup never invokes this. It
    /// refuses to replace any active registry generation.
    pub fn install_synthetic_registry_generation(&self, now: EpochMillis) -> Result<(), CoreError> {
        if CapabilityRegistry::current_generation(&self.store)
            .map_err(CoreError::Registry)?
            .is_some()
        {
            return Err(CoreError::Registry(StoreError::RegistryGenerationActivated));
        }
        let generation = CapabilityRegistry::create_generation(
            &self.store,
            RegistryGenerationDraft {
                manifest_digest: digest_of("\"serea-empty-synthetic-manifest-v1\"")
                    .map_err(CoreError::Canonical)?,
                schema_catalog_digest: digest_of("\"serea-empty-synthetic-catalog-v1\"")
                    .map_err(CoreError::Canonical)?,
            },
        )
        .map_err(CoreError::Registry)?;
        let descriptor = CapabilityDescriptor::new(CapabilityDescriptorDraft {
            id: CapabilityId::new("synthetic.echo.read").map_err(CoreError::Protocol)?,
            version: SemVer::new("1.0.0").map_err(CoreError::Protocol)?,
            title: DescriptorTitle::new("Synthetic no-op").map_err(CoreError::Protocol)?,
            description: DescriptorDescription::new("Synthetic-only no-op fixture")
                .map_err(CoreError::Protocol)?,
            provider_id: serea_protocol::ProviderId::new("synthetic")
                .map_err(CoreError::Protocol)?,
            implementation_id: None,
            input_schema: JsonSchemaRef::new("serea://synthetic/input")
                .map_err(CoreError::Protocol)?,
            output_schema: JsonSchemaRef::new("serea://synthetic/output")
                .map_err(CoreError::Protocol)?,
            side_effect_class: SideEffectClass::None,
            risk_class: RiskClass::Observe,
            required_authorization: Authorization::None,
            replay_safety: ReplaySafety::Idempotent,
            data_class: DataClass::Public,
            root_requirement: RootRequirement::NotRequired,
            idempotency_support: IdempotencySupport::None,
            max_duration_ms: 1_000,
            cost_class: serea_protocol::CostClass::Free,
            experimental: true,
        })
        .map_err(CoreError::Protocol)?;
        let descriptor_digest =
            digest_of("\"serea-synthetic-no-op-descriptor-v1\"").map_err(CoreError::Canonical)?;
        let schema_digest =
            digest_of("\"serea-synthetic-no-op-schema-v1\"").map_err(CoreError::Canonical)?;
        CapabilityRegistry::insert_descriptor_revision(
            &self.store,
            DescriptorRevisionDraft {
                descriptor_digest: descriptor_digest.clone(),
                descriptor: descriptor.clone(),
                input_schema_digest: schema_digest.clone(),
                output_schema_digest: schema_digest,
            },
        )
        .map_err(CoreError::Registry)?;
        CapabilityRegistry::add_generation_member(
            &self.store,
            GenerationMemberDraft {
                generation_id: generation.generation_id(),
                descriptor_digest,
                candidate_priority: 0,
            },
        )
        .map_err(CoreError::Registry)?;
        CapabilityRegistry::set_default_version(
            &self.store,
            generation.generation_id(),
            descriptor.id().clone(),
            descriptor.version().clone(),
        )
        .map_err(CoreError::Registry)?;
        CapabilityRegistry::activate_generation(
            &self.store,
            &self.events,
            generation.generation_id(),
            now,
        )
        .map_err(CoreError::Registry)?;
        Ok(())
    }

    /// Returns the active canonical schema version.
    pub fn schema_version(&self) -> Result<u32, CoreError> {
        self.store.schema_version().map_err(CoreError::Storage)
    }

    /// Returns one bounded ordered page of durable audit events.
    pub fn replay_events(&self, limit: u16) -> Result<EventReplayPage, CoreError> {
        self.store
            .replay_events(None, None, limit)
            .map_err(CoreError::Storage)
    }

    /// Summarizes the beginning of the audit log without exposing event payloads.
    pub fn audit_sequence_summary(&self, limit: u16) -> Result<AuditSequenceSummary, CoreError> {
        use serea_protocol::EventKind;
        use serea_storage::ReplayItem;

        let page = self.replay_events(limit)?;
        let mut event_count = 0_u64;
        let mut task_created_events = 0_u64;
        let mut previous_seq = 0_u64;
        let mut strictly_ordered = true;
        for item in page.items {
            match item {
                ReplayItem::Event { event } => {
                    let current_seq = event.seq.get();
                    strictly_ordered &= current_seq == previous_seq + 1;
                    previous_seq = current_seq;
                    event_count += 1;
                    task_created_events += u64::from(event.kind == EventKind::TaskCreated);
                }
                _ => strictly_ordered = false,
            }
        }
        Ok(AuditSequenceSummary {
            event_count,
            task_created_events,
            strictly_ordered,
        })
    }

    /// Runs ordered durable Scheduler and Task recovery at an explicit instant.
    /// Recovery classifies and repairs durable state; it never executes tasks.
    pub fn recover(
        &mut self,
        now: EpochMillis,
        lease_owner: &str,
    ) -> Result<SchedulerRecoveryReport, CoreError> {
        let scheduler_store =
            Store::open(&self.path, &FixedClock(now)).map_err(CoreError::Storage)?;
        let task_store = Store::open(&self.path, &FixedClock(now)).map_err(CoreError::Storage)?;
        let scheduler = Scheduler::new(
            scheduler_store,
            TaskEngine::new(task_store, self.events.clone()),
            self.events.clone(),
        );
        let mut scheduler = scheduler;
        scheduler
            .recover(now, lease_owner, 64)
            .map_err(CoreError::Recovery)
    }

    /// Provides the existing Task Engine API to the trusted host composition.
    pub fn tasks(&mut self) -> &mut TaskEngine {
        &mut self.tasks
    }

    /// Loads one durable task projection through the existing Task Engine.
    pub fn load_task(&self, task_id: TaskId) -> Result<TaskRecord, CoreError> {
        self.tasks.load(task_id).map_err(CoreError::TaskEngine)
    }

    /// Prepares a file-backed store for normal shutdown, then drops the
    /// composition and its SQLite connections.
    pub fn shutdown(self) -> Result<CheckpointOutcome, CoreError> {
        self.store
            .checkpoint_for_close()
            .map_err(CoreError::Storage)
    }

    /// Starts planning using host-supplied attribution and time.
    pub fn start_planning(
        &mut self,
        task_id: TaskId,
        expected_state: serea_protocol::TaskState,
        expected_revision: u32,
        now: EpochMillis,
        context: &TransitionContext<'_>,
    ) -> Result<TaskRecord, CoreError> {
        self.tasks
            .start_planning(task_id, expected_state, expected_revision, now, context)
            .map_err(CoreError::TaskEngine)
    }
}

struct FixedClock(EpochMillis);

impl Clock for FixedClock {
    fn now_ms(&self) -> Result<EpochMillis, serea_protocol::ProtocolError> {
        Ok(self.0)
    }
}
