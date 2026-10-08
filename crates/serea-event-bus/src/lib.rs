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
const REGISTRY_ADMIN_RETENTION_MS: i64 = 365 * 24 * 60 * 60 * 1_000;

/// Stable, content-free change kind for the P5 capability registry audit event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CapabilityRegistryChangeKindV1 {
    GenerationActivated,
    Disabled,
    Enabled,
    Removed,
    Reactivated,
    ExperimentalOptIn,
    ExperimentalOptOut,
}

impl CapabilityRegistryChangeKindV1 {
    const fn wire_name(self) -> &'static str {
        match self {
            Self::GenerationActivated => "GENERATION_ACTIVATED",
            Self::Disabled => "DISABLED",
            Self::Enabled => "ENABLED",
            Self::Removed => "REMOVED",
            Self::Reactivated => "REACTIVATED",
            Self::ExperimentalOptIn => "EXPERIMENTAL_OPT_IN",
            Self::ExperimentalOptOut => "EXPERIMENTAL_OPT_OUT",
        }
    }
}

/// Metadata-only facts for one registry or admin overlay change.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapabilityRegistryChangeV1 {
    pub change_kind: CapabilityRegistryChangeKindV1,
    pub generation_id: Option<i64>,
    pub capability_id: Option<serea_protocol::CapabilityId>,
    pub overlay_revision: Option<u64>,
    pub manifest_digest: Option<serea_protocol::Digest>,
    pub schema_catalog_digest: Option<serea_protocol::Digest>,
}

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

/// Content-free host validation failure facts for a structured provider response.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelOutputInvalidEventV1 {
    /// Common host-selected attempt facts.
    pub metadata: ModelEventMetadataV1,
    /// Number of bounded host validation diagnostics, from zero through 32.
    pub diagnostic_count: u8,
}

/// Content-free host fact that a structured response was accepted after repair.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelRepairedEventV1 {
    /// Common host-selected repair attempt facts.
    pub metadata: ModelEventMetadataV1,
    /// Host-derived number of repair dispatches used by this result.
    pub repair_attempts: u8,
}

/// Content-free host facts linking a failed primary model call to its fallback.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelFallbackEventV1 {
    /// The failed primary attempt facts.
    pub metadata: ModelEventMetadataV1,
    /// New RequestId reserved for the fallback attempt.
    pub fallback_request_id: RequestId,
    /// Host-selected next model in the frozen routing chain.
    pub fallback_model_id: ModelId,
}

/// Content-free host facts that the single permitted fallback did not yield
/// a usable result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelFallbackExhaustedEventV1 {
    /// The failed primary attempt facts.
    pub metadata: ModelEventMetadataV1,
    /// Host-selected fallback model that was attempted.
    pub fallback_model_id: ModelId,
}

/// Frozen generic host bound that may be exhausted by P4 model accounting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelBoundKindV1 {
    /// Durable total trustworthy task tokens.
    TaskTotalTokens,
    /// Global daily spend reservation.
    DailySpendUsd,
}

impl ModelBoundKindV1 {
    const fn wire_name(self) -> &'static str {
        match self {
            Self::TaskTotalTokens => "max_task_total_tokens",
            Self::DailySpendUsd => "max_daily_spend_usd",
        }
    }
}

/// Content-free generic bounds event for a model operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelBoundExceededEventV1 {
    /// Task whose model operation encountered the bound.
    pub task_id: Option<TaskId>,
    /// Classification of the host-prepared operation.
    pub data_class: DataClass,
    /// Explicit host event timestamp.
    pub occurred_at: EpochMillis,
    /// Frozen bound identifier.
    pub bound: ModelBoundKindV1,
    /// Configured bound value.
    pub limit: u64,
    /// Durable or attempted observed value.
    pub observed: u64,
}

/// Content-free per-task model call/turn budget exhaustion event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelBudgetExhaustedEventV1 {
    /// Task whose budget was exhausted.
    pub task_id: TaskId,
    /// Classification of the host-prepared operation.
    pub data_class: DataClass,
    /// Explicit host event timestamp.
    pub occurred_at: EpochMillis,
    /// Bound limit.
    pub limit: u64,
    /// Current durable count.
    pub observed: u64,
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

    /// Appends `CAPABILITY_REGISTRY_CHANGED` inside the caller's Storage
    /// transaction. The caller composes this with the registry mutation.
    pub fn append_capability_registry_changed(
        &self,
        tx: &mut Tx<'_>,
        change: CapabilityRegistryChangeV1,
        occurred_at: EpochMillis,
    ) -> Result<SereaEvent, StoreError> {
        use serde_json::Value;
        let generation = change.change_kind == CapabilityRegistryChangeKindV1::GenerationActivated;
        if generation {
            if change.generation_id.is_none()
                || change.capability_id.is_some()
                || change.overlay_revision.is_some()
                || change.manifest_digest.is_none()
                || change.schema_catalog_digest.is_none()
            {
                return Err(StoreError::InvalidRegistryEvent);
            }
        } else if change.generation_id.is_some()
            || change.capability_id.is_none()
            || change.overlay_revision.is_none()
            || change.manifest_digest.is_some()
            || change.schema_catalog_digest.is_some()
        {
            return Err(StoreError::InvalidRegistryEvent);
        }
        let mut payload = Map::new();
        payload.insert(
            "change_kind".into(),
            Value::String(change.change_kind.wire_name().to_owned()),
        );
        if let Some(id) = change.generation_id {
            if id <= 0 {
                return Err(StoreError::InvalidRegistryEvent);
            }
            payload.insert("generation_id".into(), Value::from(id));
        }
        if let Some(id) = change.capability_id {
            payload.insert("capability_id".into(), Value::String(id.to_string()));
        }
        if let Some(revision) = change.overlay_revision {
            payload.insert("overlay_revision".into(), Value::from(revision));
        }
        if let Some(digest) = change.manifest_digest {
            payload.insert("manifest_digest".into(), Value::String(digest.to_string()));
        }
        if let Some(digest) = change.schema_catalog_digest {
            payload.insert(
                "schema_catalog_digest".into(),
                Value::String(digest.to_string()),
            );
        }
        let message_id = self
            .ids
            .lock()
            .map_err(|_| StoreError::LockPoisoned)?
            .next_event_id();
        let retention_at = EpochMillis::new(
            occurred_at
                .get()
                .checked_add(REGISTRY_ADMIN_RETENTION_MS)
                .ok_or(StoreError::InvalidTimestamp)?,
        )
        .map_err(|_| StoreError::InvalidTimestamp)?;
        EventBus::append(
            tx,
            SereaEvent {
                envelope_version: EnvelopeVersion::new("1")
                    .map_err(|_| StoreError::InvalidRegistryEvent)?,
                surface: WireSurface::new(WireSurface::EVENT)
                    .map_err(|_| StoreError::InvalidRegistryEvent)?,
                message_id,
                seq: Seq::new(0),
                kind: EventKind::CapabilityRegistryChanged,
                occurred_at: Timestamp::from_epoch_millis(occurred_at),
                correlation_id: None,
                causation_id: None,
                actor: Actor {
                    kind: ActorKind::Host,
                    id: ActorId::new("capability-registry")
                        .map_err(|_| StoreError::InvalidRegistryEvent)?,
                    version: SemVer::new("1.0.0").map_err(|_| StoreError::InvalidRegistryEvent)?,
                    extensions: Default::default(),
                },
                data_class: DataClass::Public,
                trace: None,
                payload,
                extensions: Default::default(),
            },
            Some(retention_at),
        )
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

    /// Builds a bounded, content-free structured-output validation event.
    pub fn draft_model_output_invalid(
        &self,
        failure: ModelOutputInvalidEventV1,
    ) -> Result<EventDraft, StoreError> {
        if failure.diagnostic_count > 32 {
            return Err(StoreError::InvalidModelCall);
        }
        let mut payload = model_metadata_payload(&failure.metadata);
        payload.insert(
            "diagnostic_count".into(),
            Value::from(failure.diagnostic_count),
        );
        self.draft_model_event(failure.metadata, EventKind::ModelOutputInvalid, payload)
    }

    /// Builds the bounded, content-free `MODEL_REPAIRED` event for an accepted
    /// host-validated repair result.
    pub fn draft_model_repaired(
        &self,
        repaired: ModelRepairedEventV1,
    ) -> Result<EventDraft, StoreError> {
        if !(1..=2).contains(&repaired.repair_attempts)
            || repaired.metadata.relation != ModelEventRelationV1::Repair
        {
            return Err(StoreError::InvalidModelCall);
        }
        let mut payload = model_metadata_payload(&repaired.metadata);
        payload.insert(
            "repair_attempts".into(),
            Value::from(repaired.repair_attempts),
        );
        self.draft_model_event(repaired.metadata, EventKind::ModelRepaired, payload)
    }

    /// Builds a content-free fallback decision event. The caller commits it
    /// with primary failure and the fallback dispatch intent.
    pub fn draft_model_fallback(
        &self,
        fallback: ModelFallbackEventV1,
    ) -> Result<EventDraft, StoreError> {
        if fallback.metadata.relation != ModelEventRelationV1::Normal
            || fallback.fallback_model_id == fallback.metadata.model_id
            || fallback.fallback_request_id == fallback.metadata.request_id
        {
            return Err(StoreError::InvalidModelCall);
        }
        let mut payload = model_metadata_payload(&fallback.metadata);
        payload.insert(
            "fallback_request_id".into(),
            Value::String(fallback.fallback_request_id.as_str().to_owned()),
        );
        payload.insert(
            "fallback_model_id".into(),
            Value::String(fallback.fallback_model_id.as_str().to_owned()),
        );
        self.draft_model_event(fallback.metadata, EventKind::ModelFallback, payload)
    }

    /// Builds the content-free event that closes the one-step fallback ladder.
    pub fn draft_model_fallback_exhausted(
        &self,
        exhausted: ModelFallbackExhaustedEventV1,
    ) -> Result<EventDraft, StoreError> {
        if exhausted.metadata.relation != ModelEventRelationV1::Normal
            || exhausted.fallback_model_id == exhausted.metadata.model_id
        {
            return Err(StoreError::InvalidModelCall);
        }
        let mut payload = model_metadata_payload(&exhausted.metadata);
        payload.insert(
            "fallback_model_id".into(),
            Value::String(exhausted.fallback_model_id.as_str().to_owned()),
        );
        self.draft_model_event(
            exhausted.metadata,
            EventKind::ModelFallbackExhausted,
            payload,
        )
    }

    /// Builds a generic content-free `BOUND_EXCEEDED` event for model bounds.
    pub fn draft_model_bound_exceeded(
        &self,
        bound: ModelBoundExceededEventV1,
    ) -> Result<EventDraft, StoreError> {
        if bound.observed < bound.limit {
            return Err(StoreError::InvalidModelCall);
        }
        let mut payload = Map::new();
        payload.insert(
            "bound_name".into(),
            Value::String(bound.bound.wire_name().into()),
        );
        payload.insert("limit".into(), Value::from(bound.limit));
        payload.insert("observed".into(), Value::from(bound.observed));
        self.draft_model_task_event(
            bound.task_id,
            bound.data_class,
            bound.occurred_at,
            EventKind::BoundExceeded,
            payload,
        )
    }

    /// Builds a content-free `MODEL_BUDGET_EXHAUSTED` event for task call/turn
    /// exhaustion.
    pub fn draft_model_budget_exhausted(
        &self,
        budget: ModelBudgetExhaustedEventV1,
    ) -> Result<EventDraft, StoreError> {
        if budget.observed < budget.limit || budget.limit == 0 {
            return Err(StoreError::InvalidModelCall);
        }
        let mut payload = Map::new();
        payload.insert("limit".into(), Value::from(budget.limit));
        payload.insert("observed".into(), Value::from(budget.observed));
        self.draft_model_task_event(
            Some(budget.task_id),
            budget.data_class,
            budget.occurred_at,
            EventKind::ModelBudgetExhausted,
            payload,
        )
    }

    fn draft_model_task_event(
        &self,
        task_id: Option<TaskId>,
        data_class: DataClass,
        occurred_at: EpochMillis,
        kind: EventKind,
        payload: Map<String, Value>,
    ) -> Result<EventDraft, StoreError> {
        if !matches!(data_class, DataClass::Public | DataClass::Personal) {
            return Err(StoreError::EventClassRefused);
        }
        let message_id = self
            .ids
            .lock()
            .map_err(|_| StoreError::LockPoisoned)?
            .next_event_id();
        let retention_at = occurred_at
            .get()
            .checked_add(MODEL_ACTIVITY_RETENTION_MS)
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
                occurred_at: Timestamp::from_epoch_millis(occurred_at),
                correlation_id: task_id.clone(),
                causation_id: None,
                actor: Actor {
                    kind: ActorKind::Host,
                    id: ActorId::new("model-router").map_err(|_| StoreError::AuditRejected)?,
                    version: SemVer::new("1.0.0").map_err(|_| StoreError::AuditRejected)?,
                    extensions: Default::default(),
                },
                data_class,
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
