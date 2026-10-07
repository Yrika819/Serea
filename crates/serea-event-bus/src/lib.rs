//! Event semantics and identifier minting over Storage's private event sink.
#![forbid(unsafe_code)]

use std::sync::{Arc, Mutex};

use serde_json::{Map, Value};
use serea_protocol::{
    Actor, ActorId, ActorKind, DataClass, EnvelopeVersion, EpochMillis, EventId, EventKind,
    IdMinter, ScheduleId, SemVer, Seq, SereaEvent, TaskId, TaskState, Timestamp, Trace, UlidSource,
    WireSurface,
};
use serea_storage::{
    AuditOperation, DurableTransition, EventDraft, EventParticipant, Store, StoreError, Tx,
};

pub use serea_storage::{EventReplayPage, EventRetentionReport, ReplayItem};

/// Closed Scheduler lifecycle facts that Event Bus can author.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScheduleLifecycle {
    Created {
        schedule_id: ScheduleId,
        revision: u32,
    },
    Updated {
        schedule_id: ScheduleId,
        revision: u32,
    },
    Paused {
        schedule_id: ScheduleId,
        revision: u32,
    },
    Resumed {
        schedule_id: ScheduleId,
        revision: u32,
    },
    Cancelled {
        schedule_id: ScheduleId,
        revision: u32,
    },
    OccurrenceMissed {
        schedule_id: ScheduleId,
        occurrence_key: String,
    },
    CatchUpDeferred {
        schedule_id: ScheduleId,
        first_pending_key: String,
    },
}

const TASK_LIFECYCLE_RETENTION_MS: i64 = 30 * 24 * 60 * 60 * 1_000;

struct ErasedUlidSource(Box<dyn UlidSource + Send>);

impl UlidSource for ErasedUlidSource {
    fn next_ulid(&mut self) -> serea_protocol::UlidValue {
        self.0.next_ulid()
    }
}

/// Event semantic mapper with an injected ULID source. The source is required
/// so EventId minting is deterministic in tests and explicit at runtime. The
/// source is erased internally so Task Engine accepts only this fixed mapper,
/// not an arbitrary event callback implementation.
pub struct EventBus {
    ids: Arc<Mutex<IdMinter<ErasedUlidSource>>>,
}

impl Clone for EventBus {
    fn clone(&self) -> Self {
        Self {
            ids: Arc::clone(&self.ids),
        }
    }
}

impl EventBus {
    pub fn new<S: UlidSource + Send + 'static>(source: S) -> Self {
        Self {
            ids: Arc::new(Mutex::new(IdMinter::new(ErasedUlidSource(Box::new(
                source,
            ))))),
        }
    }

    /// Mints a host-owned TaskId from the injected identifier source.
    pub fn mint_task_id(&self) -> Result<TaskId, StoreError> {
        self.ids
            .lock()
            .map_err(|_| StoreError::LockPoisoned)
            .map(|mut ids| ids.next_task_id())
    }

    /// Builds the fixed lifecycle fact emitted with a scheduled task mapping.
    /// The occurrence mapping remains the durable source of schedule identity.
    pub fn draft_schedule_task_created(
        &self,
        schedule_id: &ScheduleId,
        occurrence_key: &str,
        task_id: &TaskId,
        source_event_id: Option<&EventId>,
        now: EpochMillis,
        data_class: DataClass,
    ) -> Result<EventDraft, StoreError> {
        if !matches!(data_class, DataClass::Public | DataClass::Personal)
            || occurrence_key.is_empty()
            || occurrence_key.len() > 512
        {
            return Err(StoreError::ClassRefused);
        }
        let message_id = self
            .ids
            .lock()
            .map_err(|_| StoreError::LockPoisoned)?
            .next_event_id();
        let mut payload = Map::new();
        payload.insert(
            "schedule_id".into(),
            Value::String(schedule_id.as_str().into()),
        );
        payload.insert(
            "occurrence_key".into(),
            Value::String(occurrence_key.into()),
        );
        payload.insert("task_id".into(), Value::String(task_id.as_str().into()));
        if let Some(source) = source_event_id {
            payload.insert(
                "source_event_id".into(),
                Value::String(source.as_str().into()),
            );
        }
        let retention_at = now
            .get()
            .checked_add(TASK_LIFECYCLE_RETENTION_MS)
            .and_then(|value| EpochMillis::new(value).ok())
            .ok_or(StoreError::InvalidTimestamp)?;
        Ok(EventDraft {
            event: SereaEvent {
                envelope_version: EnvelopeVersion::new("1")
                    .map_err(|_| StoreError::AuditRejected)?,
                surface: WireSurface::new(WireSurface::EVENT)
                    .map_err(|_| StoreError::AuditRejected)?,
                message_id,
                seq: Seq::new(0),
                kind: EventKind::ScheduleTaskCreated,
                occurred_at: Timestamp::from_epoch_millis(now),
                correlation_id: Some(task_id.clone()),
                causation_id: source_event_id.cloned(),
                actor: Actor {
                    kind: ActorKind::Scheduler,
                    id: ActorId::new("scheduler").map_err(|_| StoreError::AuditRejected)?,
                    version: SemVer::new("1.0.0").map_err(|_| StoreError::AuditRejected)?,
                    extensions: Default::default(),
                },
                data_class,
                trace: Some(Trace {
                    task_id: Some(task_id.clone()),
                    step_id: None,
                    attempt: None,
                    extensions: Default::default(),
                }),
                payload,
                extensions: Default::default(),
            },
            retention_at: Some(retention_at),
        })
    }

    /// Builds one of the fixed Scheduler lifecycle events. The caller commits
    /// the resulting draft atomically with the described durable state change.
    pub fn draft_schedule_lifecycle(
        &self,
        lifecycle: ScheduleLifecycle,
        command_id: &EventId,
        now: EpochMillis,
    ) -> Result<EventDraft, StoreError> {
        self.draft_schedule_lifecycle_inner(lifecycle, Some(command_id), now)
    }

    /// Builds the frozen missed-occurrence event without inventing a command
    /// event as its cause. It must be committed with the skipped occurrence.
    pub fn draft_schedule_occurrence_missed(
        &self,
        schedule_id: &ScheduleId,
        occurrence_key: &str,
        now: EpochMillis,
    ) -> Result<EventDraft, StoreError> {
        self.draft_schedule_lifecycle_inner(
            ScheduleLifecycle::OccurrenceMissed {
                schedule_id: schedule_id.clone(),
                occurrence_key: occurrence_key.to_owned(),
            },
            None,
            now,
        )
    }

    /// Builds the frozen deferred-catch-up event without inventing a command
    /// event as its cause. It must be committed with durable retry state.
    pub fn draft_schedule_catch_up_deferred(
        &self,
        schedule_id: &ScheduleId,
        first_pending_key: &str,
        now: EpochMillis,
    ) -> Result<EventDraft, StoreError> {
        self.draft_schedule_lifecycle_inner(
            ScheduleLifecycle::CatchUpDeferred {
                schedule_id: schedule_id.clone(),
                first_pending_key: first_pending_key.to_owned(),
            },
            None,
            now,
        )
    }

    fn draft_schedule_lifecycle_inner(
        &self,
        lifecycle: ScheduleLifecycle,
        causation_id: Option<&EventId>,
        now: EpochMillis,
    ) -> Result<EventDraft, StoreError> {
        let (kind, schedule_id, detail): (EventKind, ScheduleId, Option<(&'static str, Value)>) =
            match lifecycle {
                ScheduleLifecycle::Created {
                    schedule_id,
                    revision,
                } => (
                    EventKind::ScheduleCreated,
                    schedule_id,
                    Some(("revision", Value::from(revision))),
                ),
                ScheduleLifecycle::Updated {
                    schedule_id,
                    revision,
                } => (
                    EventKind::ScheduleUpdated,
                    schedule_id,
                    Some(("revision", Value::from(revision))),
                ),
                ScheduleLifecycle::Paused {
                    schedule_id,
                    revision,
                } => (
                    EventKind::SchedulePaused,
                    schedule_id,
                    Some(("revision", Value::from(revision))),
                ),
                ScheduleLifecycle::Resumed {
                    schedule_id,
                    revision,
                } => (
                    EventKind::ScheduleResumed,
                    schedule_id,
                    Some(("revision", Value::from(revision))),
                ),
                ScheduleLifecycle::Cancelled {
                    schedule_id,
                    revision,
                } => (
                    EventKind::ScheduleCancelled,
                    schedule_id,
                    Some(("revision", Value::from(revision))),
                ),
                ScheduleLifecycle::OccurrenceMissed {
                    schedule_id,
                    occurrence_key,
                } => (
                    EventKind::ScheduleOccurrenceMissed,
                    schedule_id,
                    Some(("occurrence_key", Value::String(occurrence_key))),
                ),
                ScheduleLifecycle::CatchUpDeferred {
                    schedule_id,
                    first_pending_key,
                } => (
                    EventKind::ScheduleCatchUpDeferred,
                    schedule_id,
                    Some(("first_pending_key", Value::String(first_pending_key))),
                ),
            };
        if detail.as_ref().is_some_and(|(key, value)| {
            matches!(*key, "occurrence_key" | "first_pending_key")
                && value
                    .as_str()
                    .is_none_or(|text| text.is_empty() || text.len() > 512)
        }) {
            return Err(StoreError::InvalidSchedule);
        }
        let message_id = self
            .ids
            .lock()
            .map_err(|_| StoreError::LockPoisoned)?
            .next_event_id();
        let mut payload = Map::new();
        payload.insert(
            "schedule_id".into(),
            Value::String(schedule_id.as_str().into()),
        );
        if let Some((key, value)) = detail {
            payload.insert(key.into(), value);
        }
        let retention_at = now
            .get()
            .checked_add(TASK_LIFECYCLE_RETENTION_MS)
            .and_then(|value| EpochMillis::new(value).ok())
            .ok_or(StoreError::InvalidTimestamp)?;
        Ok(EventDraft {
            event: SereaEvent {
                envelope_version: EnvelopeVersion::new("1")
                    .map_err(|_| StoreError::AuditRejected)?,
                surface: WireSurface::new(WireSurface::EVENT)
                    .map_err(|_| StoreError::AuditRejected)?,
                message_id,
                seq: Seq::new(0),
                kind,
                occurred_at: Timestamp::from_epoch_millis(now),
                correlation_id: None,
                causation_id: causation_id.cloned(),
                actor: Actor {
                    kind: ActorKind::Scheduler,
                    id: ActorId::new("scheduler").map_err(|_| StoreError::AuditRejected)?,
                    version: SemVer::new("1.0.0").map_err(|_| StoreError::AuditRejected)?,
                    extensions: Default::default(),
                },
                data_class: DataClass::Public,
                trace: None,
                payload,
                extensions: Default::default(),
            },
            retention_at: Some(retention_at),
        })
    }

    /// Appends an independently authored event in the caller's transaction.
    pub fn append(
        tx: &mut Tx<'_>,
        event: SereaEvent,
        retention_at: Option<serea_protocol::EpochMillis>,
    ) -> Result<SereaEvent, StoreError> {
        tx.append_event(event, retention_at)
    }

    /// Returns a bounded page through the committed high-water captured for
    /// this read transaction.
    pub fn replay(
        store: &Store,
        after_seq: Option<Seq>,
        through_seq: Option<Seq>,
        limit: u16,
    ) -> Result<EventReplayPage, StoreError> {
        store.replay_events(after_seq, through_seq, limit)
    }

    /// Applies one bounded whole-record expiry batch irrespective of consumer
    /// backlog, then folds any now-contiguous expired prefix.
    pub fn expire_eligible(
        store: &Store,
        now: serea_protocol::EpochMillis,
        limit: u16,
    ) -> Result<EventRetentionReport, StoreError> {
        store.expire_eligible_events(now, limit)
    }
}

impl EventParticipant for EventBus {
    fn events(&self, facts: &DurableTransition) -> Result<Vec<EventDraft>, StoreError> {
        let Some(kind) = task_event_kind(facts) else {
            return Ok(Vec::new());
        };
        let message_id = self
            .ids
            .lock()
            .map_err(|_| StoreError::LockPoisoned)?
            .next_event_id();
        let mut payload = Map::new();
        payload.insert(
            "task_id".into(),
            Value::String(facts.task_id().as_str().to_owned()),
        );
        if matches!(kind, EventKind::TaskStateChanged | EventKind::TaskResumed) {
            if let Some(from) = facts.task_from() {
                payload.insert("from".into(), Value::String(from.wire_name().into()));
            }
            payload.insert(
                "to".into(),
                Value::String(facts.task_to().wire_name().into()),
            );
        }
        match kind {
            EventKind::TaskFailed => {
                if let Some(reason) = facts.reason() {
                    payload.insert(
                        "reason_code".into(),
                        Value::String(reason.as_str().to_owned()),
                    );
                }
            }
            EventKind::TaskBlocked => {
                if let Some(reason) = facts.reason() {
                    payload.insert(
                        "blocked_reason".into(),
                        Value::String(reason.as_str().to_owned()),
                    );
                }
            }
            EventKind::TaskCompleted => {
                if let Some(result) = facts.result() {
                    payload.insert(
                        "result_digest".into(),
                        Value::String(result.as_str().to_owned()),
                    );
                }
            }
            EventKind::TaskCancelled => {
                if let Some(by) = facts.cancelled_by() {
                    payload.insert("cancelled_by".into(), Value::String(by.as_str().to_owned()));
                }
            }
            _ => {}
        }
        let surface =
            WireSurface::new(WireSurface::EVENT).map_err(|_| StoreError::AuditRejected)?;
        let envelope_version = EnvelopeVersion::new("1").map_err(|_| StoreError::AuditRejected)?;
        let retention_at = facts
            .now()
            .get()
            .checked_add(TASK_LIFECYCLE_RETENTION_MS)
            .and_then(|millis| serea_protocol::EpochMillis::new(millis).ok())
            .ok_or(StoreError::InvalidTimestamp)?;
        Ok(vec![EventDraft {
            event: SereaEvent {
                envelope_version,
                surface,
                message_id,
                seq: Seq::new(0),
                kind,
                occurred_at: serea_protocol::Timestamp::from_epoch_millis(facts.now()),
                correlation_id: Some(facts.task_id().clone()),
                causation_id: facts.causation_id().cloned(),
                actor: Actor {
                    kind: facts.actor_kind(),
                    id: facts.actor_id().clone(),
                    version: facts.actor_version().clone(),
                    extensions: Default::default(),
                },
                data_class: facts.data_class(),
                trace: Some(Trace {
                    task_id: Some(facts.task_id().clone()),
                    step_id: facts.step_id().cloned(),
                    attempt: facts.attempt(),
                    extensions: Default::default(),
                }),
                payload,
                extensions: Default::default(),
            },
            retention_at: Some(retention_at),
        }])
    }
}

fn task_event_kind(facts: &DurableTransition) -> Option<EventKind> {
    let from = facts.task_from();
    let to = facts.task_to();
    match facts.operation() {
        AuditOperation::TaskInserted => Some(EventKind::TaskCreated),
        AuditOperation::DeviceSessionResumed => Some(EventKind::TaskResumed),
        AuditOperation::PlanningStarted => match from? {
            TaskState::Received => Some(EventKind::TaskStarted),
            TaskState::Blocked | TaskState::WaitingUser => Some(EventKind::TaskResumed),
            _ if from != Some(to) => Some(EventKind::TaskStateChanged),
            _ => None,
        },
        AuditOperation::PlanPersisted => Some(EventKind::TaskStateChanged),
        AuditOperation::LeaseAcquired | AuditOperation::LeaseReleased => None,
        AuditOperation::AttemptStarted
        | AuditOperation::StepSucceeded
        | AuditOperation::StepFailed
        | AuditOperation::RecoveryStateChanged => {
            if to == TaskState::Blocked {
                Some(EventKind::TaskBlocked)
            } else if to == TaskState::Failed {
                Some(EventKind::TaskFailed)
            } else if to == TaskState::Completed {
                Some(EventKind::TaskCompleted)
            } else if from != Some(to) {
                if matches!(
                    from,
                    Some(TaskState::Blocked | TaskState::WaitingUser | TaskState::WaitingApproval)
                ) {
                    Some(EventKind::TaskResumed)
                } else if from.is_none() && to == TaskState::Planning {
                    Some(EventKind::TaskStarted)
                } else {
                    Some(EventKind::TaskStateChanged)
                }
            } else {
                None
            }
        }
        AuditOperation::Blocked => Some(EventKind::TaskBlocked),
        AuditOperation::InvariantFailed => Some(EventKind::TaskFailed),
        AuditOperation::Cancelled => Some(EventKind::TaskCancelled),
        AuditOperation::RecoveryDecision => None,
    }
}
