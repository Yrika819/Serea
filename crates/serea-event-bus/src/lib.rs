//! Event semantics and identifier minting over Storage's private event sink.
#![forbid(unsafe_code)]

use std::sync::{Arc, Mutex};

use serde_json::{Map, Value};
use serea_protocol::{
    Actor, ActorId, ActorKind, CostClass, DataClass, EnvelopeVersion, EpochMillis, EventId,
    EventKind, FinishReason, IdMinter, ModelErrorCode, ModelId, ModelPurpose, ProviderId,
    RequestId, ScheduleId, SemVer, Seq, SereaEvent, TaskId, TaskState, Timestamp, Trace,
    UlidSource, WireSurface,
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
const MODEL_ACTIVITY_RETENTION_MS: i64 = 30 * 24 * 60 * 60 * 1_000;

/// Closed model-attempt relationship carried by content-free activity events.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelEventRelationV1 {
    /// A top-level normal operation.
    Normal,
    /// A deterministic fallback attempt.
    Fallback,
    /// A bounded structured repair attempt.
    Repair,
}

impl ModelEventRelationV1 {
    const fn wire_name(self) -> &'static str {
        match self {
            Self::Normal => "NORMAL",
            Self::Fallback => "FALLBACK",
            Self::Repair => "REPAIR",
        }
    }
}

/// Host-owned identity and classification facts for one model activity event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelEventMetadataV1 {
    /// One-dispatch request identity.
    pub request_id: RequestId,
    /// Host-selected configured model.
    pub model_id: ModelId,
    /// Registered provider identity.
    pub provider_id: ProviderId,
    /// Owning task, when present.
    pub task_id: Option<TaskId>,
    /// Host-assigned model purpose.
    pub purpose: ModelPurpose,
    /// Host-assigned relation to a prior model attempt.
    pub relation: ModelEventRelationV1,
    /// Classification of the prepared call.
    pub data_class: DataClass,
    /// Explicit host timestamp for this activity event.
    pub occurred_at: EpochMillis,
}

/// Trusted accounting metadata for one completed provider response event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelCompletedEventV1 {
    /// Common host-selected attempt facts.
    pub metadata: ModelEventMetadataV1,
    /// Provider finish reason, bound by the host to this attempt.
    pub finish_reason: FinishReason,
    /// Validated provider input tokens.
    pub input_tokens: u64,
    /// Validated provider output tokens.
    pub output_tokens: u64,
    /// Cost class from the host price snapshot.
    pub cost_class: CostClass,
    /// Settled cost in integer micro-USD, computed by the host.
    pub cost_usd_micros: u64,
    /// Immutable price configuration revision.
    pub price_revision: String,
    /// Host-derived structured repair dispatch count.
    pub repair_attempts: u8,
}

/// Stable provider failure facts for one model failure event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelFailedEventV1 {
    /// Common host-selected attempt facts.
    pub metadata: ModelEventMetadataV1,
    /// Stable typed error code; free-text diagnostics are excluded.
    pub error_kind: ModelErrorCode,
    /// Whether the adapter proved the failure is retryable.
    pub retryable: bool,
}

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

    /// Mints a host-owned request identity for exactly one model dispatch.
    pub fn mint_model_request_id(&self) -> Result<RequestId, StoreError> {
        self.ids
            .lock()
            .map_err(|_| StoreError::LockPoisoned)
            .map(|mut ids| ids.next_request_id())
    }

    /// Builds the content-free `MODEL_CALLED` event draft. The caller must
    /// append it in the same storage transaction that persists the matching
    /// dispatch intent, before invoking a provider.
    pub fn draft_model_called(
        &self,
        metadata: ModelEventMetadataV1,
    ) -> Result<EventDraft, StoreError> {
        let payload = model_metadata_payload(&metadata);
        self.draft_model_event(metadata, EventKind::ModelCalled, payload)
    }

    /// Builds a content-free model completion event with validated usage and
    /// host-owned price facts only.
    pub fn draft_model_completed(
        &self,
        completion: ModelCompletedEventV1,
    ) -> Result<EventDraft, StoreError> {
        if completion.repair_attempts > 2
            || completion.price_revision.is_empty()
            || completion.price_revision.len() > 128
            || completion
                .price_revision
                .bytes()
                .any(|byte| byte.is_ascii_control())
        {
            return Err(StoreError::InvalidModelCall);
        }
        let mut payload = model_metadata_payload(&completion.metadata);
        payload.insert(
            "finish_reason".into(),
            Value::String(completion.finish_reason.wire_name().to_owned()),
        );
        payload.insert("input_tokens".into(), Value::from(completion.input_tokens));
        payload.insert(
            "output_tokens".into(),
            Value::from(completion.output_tokens),
        );
        payload.insert(
            "cost_class".into(),
            Value::String(completion.cost_class.wire_name().to_owned()),
        );
        payload.insert(
            "cost_usd_micros".into(),
            Value::from(completion.cost_usd_micros),
        );
        payload.insert(
            "price_revision".into(),
            Value::String(completion.price_revision),
        );
        payload.insert(
            "repair_attempts".into(),
            Value::from(completion.repair_attempts),
        );
        self.draft_model_event(completion.metadata, EventKind::ModelCompleted, payload)
    }

    /// Builds a content-free model failure event. Diagnostic text and model
    /// response content have no representation in this typed payload.
    pub fn draft_model_failed(
        &self,
        failure: ModelFailedEventV1,
    ) -> Result<EventDraft, StoreError> {
        let mut payload = model_metadata_payload(&failure.metadata);
        payload.insert(
            "error_kind".into(),
            Value::String(failure.error_kind.as_str().to_owned()),
        );
        payload.insert("retryable".into(), Value::Bool(failure.retryable));
        self.draft_model_event(failure.metadata, EventKind::ModelFailed, payload)
    }

    fn draft_model_event(
        &self,
        metadata: ModelEventMetadataV1,
        kind: EventKind,
        payload: Map<String, Value>,
    ) -> Result<EventDraft, StoreError> {
        if !matches!(metadata.data_class, DataClass::Public | DataClass::Personal) {
            return Err(StoreError::EventClassRefused);
        }
        let message_id = self
            .ids
            .lock()
            .map_err(|_| StoreError::LockPoisoned)?
            .next_event_id();
        let retention_at = metadata
            .occurred_at
            .get()
            .checked_add(MODEL_ACTIVITY_RETENTION_MS)
            .and_then(|value| EpochMillis::new(value).ok())
            .ok_or(StoreError::InvalidTimestamp)?;
        let task_id = metadata.task_id;
        Ok(EventDraft {
            event: SereaEvent {
                envelope_version: EnvelopeVersion::new("1")
                    .map_err(|_| StoreError::AuditRejected)?,
                surface: WireSurface::new(WireSurface::EVENT)
                    .map_err(|_| StoreError::AuditRejected)?,
                message_id,
                seq: Seq::new(0),
                kind,
                occurred_at: Timestamp::from_epoch_millis(metadata.occurred_at),
                correlation_id: task_id.clone(),
                causation_id: None,
                actor: Actor {
                    kind: ActorKind::Host,
                    id: ActorId::new("model-router").map_err(|_| StoreError::AuditRejected)?,
                    version: SemVer::new("1.0.0").map_err(|_| StoreError::AuditRejected)?,
                    extensions: Default::default(),
                },
                data_class: metadata.data_class,
                trace: Some(Trace {
                    task_id,
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

fn model_metadata_payload(metadata: &ModelEventMetadataV1) -> Map<String, Value> {
    let mut payload = Map::new();
    payload.insert(
        "request_id".into(),
        Value::String(metadata.request_id.as_str().to_owned()),
    );
    payload.insert(
        "model_id".into(),
        Value::String(metadata.model_id.as_str().to_owned()),
    );
    payload.insert(
        "provider_id".into(),
        Value::String(metadata.provider_id.as_str().to_owned()),
    );
    payload.insert(
        "purpose".into(),
        Value::String(metadata.purpose.wire_name().to_owned()),
    );
    payload.insert(
        "relation_kind".into(),
        Value::String(metadata.relation.wire_name().to_owned()),
    );
    payload
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
