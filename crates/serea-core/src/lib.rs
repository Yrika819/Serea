//! Process-level construction and lifecycle coordination for Serea's durable
//! components. Core deliberately accepts time and identifier sources from its
//! host boundary; it does not invent ambient clocks or randomness.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

use std::path::{Path, PathBuf};

use serea_event_bus::EventBus;
use serea_protocol::TaskId;
use serea_protocol::{Clock, EpochMillis};
use serea_scheduler::{ScheduleError, Scheduler, SchedulerRecoveryReport};
use serea_storage::{CheckpointOutcome, EventReplayPage, Store, StoreError};
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
    /// Startup recovery refused the durable state.
    Recovery(ScheduleError),
    /// Task Engine refused a host operation.
    TaskEngine(EngineError),
}

impl std::fmt::Display for CoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Storage(_) => "core storage initialization failed",
            Self::Recovery(_) => "core startup recovery failed",
            Self::TaskEngine(_) => "core task operation failed",
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

    /// Processes one bounded due-calendar wake through the existing Scheduler.
    /// The Scheduler owns admission, claims, TaskEngine creation and audit
    /// attribution; this method exposes no direct storage mutation surface.
    pub fn process_due_calendar(
        &self,
        now: EpochMillis,
        lease_owner: &str,
    ) -> Result<u16, CoreError> {
        let scheduler_store =
            Store::open(&self.path, &FixedClock(now)).map_err(CoreError::Storage)?;
        let task_store = Store::open(&self.path, &FixedClock(now)).map_err(CoreError::Storage)?;
        let mut scheduler = Scheduler::new(
            scheduler_store,
            TaskEngine::new(task_store, self.events.clone()),
            self.events.clone(),
        );
        scheduler
            .process_calendar_due(now, lease_owner)
            .map_err(CoreError::Recovery)
    }

    /// Runs one bounded live Scheduler wake: replay a committed Host Event
    /// page, dispatch materialized Host Event occurrences, then process due
    /// calendar occurrences. The durable consumer fence is released only after
    /// the bounded pass completes successfully.
    pub fn process_scheduler_wake(
        &self,
        now: EpochMillis,
        lease_owner: &str,
    ) -> Result<(u16, u16), CoreError> {
        let scheduler_store =
            Store::open(&self.path, &FixedClock(now)).map_err(CoreError::Storage)?;
        let task_store = Store::open(&self.path, &FixedClock(now)).map_err(CoreError::Storage)?;
        let mut scheduler = Scheduler::new(
            scheduler_store,
            TaskEngine::new(task_store, self.events.clone()),
            self.events.clone(),
        );
        let expires_at = EpochMillis::new(
            now.get()
                .checked_add(120_000)
                .ok_or(CoreError::Recovery(ScheduleError::Invalid))?,
        )
        .map_err(|_| CoreError::Recovery(ScheduleError::Invalid))?;
        let consumer = scheduler
            .claim_event_consumer(lease_owner, now, expires_at)
            .map_err(CoreError::Recovery)?;
        scheduler
            .replay_host_events(&consumer, now, 64)
            .map_err(CoreError::Recovery)?;
        let event_tasks = scheduler
            .process_host_event_occurrences(now, lease_owner, 64)
            .map_err(CoreError::Recovery)?;
        let calendar_tasks = scheduler
            .process_calendar_due(now, lease_owner)
            .map_err(CoreError::Recovery)?;
        scheduler
            .release_event_consumer(&consumer)
            .map_err(CoreError::Recovery)?;
        Ok((event_tasks, calendar_tasks))
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
