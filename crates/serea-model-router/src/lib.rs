//! Deterministic host-owned model selection.
//!
//! This crate contains P4C routing only. It never calls `ModelProvider::generate`.
//! `PreparedModelCallV1::from_host` is a trusted in-process host boundary: it
//! does not prove redaction and must not be exposed to Android, network, or
//! other untrusted callers.

#![forbid(unsafe_code)]
#![deny(missing_docs)]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

#[cfg(test)]
mod crash_tests;
mod structured;
#[cfg(test)]
mod structured_tests;

pub use structured::{
    StructuredValidationError, ValidationDiagnostic, validate_structured_response,
};

use std::collections::BTreeMap;
use std::io::{self, Write};
use std::sync::Arc;

use serea_event_bus::{
    EventBus, ModelBoundExceededEventV1, ModelBoundKindV1, ModelBudgetExhaustedEventV1,
    ModelCompletedEventV1, ModelEventMetadataV1, ModelEventRelationV1, ModelFailedEventV1,
    ModelFallbackEventV1, ModelFallbackExhaustedEventV1, ModelOutputInvalidEventV1,
    ModelRepairedEventV1,
};
use serea_protocol::provider::{ModelCallContext, ModelProvider};
use serea_protocol::{
    Clock, DataClass, FinishReason, JsonSchemaMode, ModelCapabilities, ModelDescriptor,
    ModelErrorCode, ModelId, ModelMessage, ModelPurpose, ModelRequest, ModelResponse,
    ProtocolError, ProviderHealth, ProviderId, ResponseFormat, TaskId, canonicalize,
};
use serea_storage::{
    MAX_MODEL_TURNS_PER_TASK, ModelAttemptRelationKind, ModelAttemptState, ModelCallAttemptDraft,
    ModelCallCompletion, ModelFailureUsage, ModelPriceSnapshot, ModelResponseStorage, Store,
    StoreError, UsdMicros, UtcAccountingDay, calculate_cost_usd_micros,
    calculate_reservation_usd_micros, validate_price_snapshot,
};

/// Maximum structural prompt size accepted before routing.
pub const MAX_MODEL_PROMPT_BYTES: usize = 1_048_576;
/// Maximum JSON Schema size accepted before routing.
pub const MAX_MODEL_SCHEMA_BYTES: usize = 65_536;
/// Maximum provider response bytes accepted by the host validator.
pub const MAX_MODEL_RESPONSE_BYTES: usize = serea_storage::MAX_MODEL_RESPONSE_BYTES;
/// Maximum nesting depth for model JSON input and host schemas.
pub const MAX_MODEL_JSON_DEPTH: usize = 64;
/// Maximum validation diagnostics retained for one response.
pub const MAX_MODEL_VALIDATION_ERRORS: usize = 32;
/// Maximum encoded validation diagnostic bytes retained for one response.
pub const MAX_MODEL_VALIDATION_ERROR_BYTES: usize = 16_384;
const AMBIGUOUS_PROVIDER_ERROR_KIND: &str = "AMBIGUOUS_DISPATCH";
/// Default host bound for per-call output tokens.
pub const DEFAULT_MAX_OUTPUT_TOKENS_PER_CALL: u32 = 2_048;
/// Frozen per-task total token bound from Bounds Protocol §2.1.
pub const MAX_TASK_TOTAL_TOKENS: u64 = 128_000;

/// Host deployment class. It is never inferred from provider or model names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelDeploymentClass {
    /// Provider is a cloud deployment.
    Cloud,
    /// Provider is a local deployment.
    Local,
}

/// Structured-output strength required by the trusted host call.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StructuredRequirementV1 {
    /// Any schema-capable provider, including best effort.
    Any,
    /// A provider advertising strict schema mode.
    Strict,
}

/// Closed host-owned model routing requirements (ADR-0031).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ModelRoutingRequirementsV1 {
    /// Whether a future typed image request is required.
    pub vision_required: bool,
    /// Whether host tools are required.
    pub tools_required: bool,
    /// Minimum acceptable context window.
    pub min_context_tokens: u32,
    /// Minimum acceptable output window.
    pub min_output_tokens: u32,
    /// Required structured output strength.
    pub structured_requirement: StructuredRequirementV1,
}

/// A trusted immutable host roster entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelRosterEntryV1 {
    model_id: ModelId,
    provider_id: ProviderId,
    deployment_class: ModelDeploymentClass,
    enabled: bool,
    allowed_capabilities: ModelCapabilities,
    cost_class: serea_protocol::CostClass,
}

impl ModelRosterEntryV1 {
    /// Constructs one host-owned roster entry.
    pub fn new(
        model_id: ModelId,
        provider_id: ProviderId,
        deployment_class: ModelDeploymentClass,
        enabled: bool,
        allowed_capabilities: ModelCapabilities,
        cost_class: serea_protocol::CostClass,
    ) -> Result<Self, RouterError> {
        if model_id.as_str() == "codex" || !known_model_id(model_id.as_str()) {
            return Err(RouterError::UnknownOrForbiddenModel);
        }
        Ok(Self {
            model_id,
            provider_id,
            deployment_class,
            enabled,
            allowed_capabilities,
            cost_class,
        })
    }

    /// Configured model identity.
    pub fn model_id(&self) -> &ModelId {
        &self.model_id
    }

    /// Configured provider identity.
    pub fn provider_id(&self) -> &ProviderId {
        &self.provider_id
    }

    /// Host-configured deployment class.
    pub fn deployment_class(&self) -> ModelDeploymentClass {
        self.deployment_class
    }

    /// Whether the host enabled this entry.
    pub fn enabled(&self) -> bool {
        self.enabled
    }

    /// Host capability ceiling.
    pub fn allowed_capabilities(&self) -> ModelCapabilities {
        self.allowed_capabilities
    }

    /// Host-owned cost class.
    pub fn cost_class(&self) -> serea_protocol::CostClass {
        self.cost_class
    }
}

/// Immutable roster snapshot, validated at host startup.
#[derive(Debug, Clone)]
pub struct ModelRosterV1 {
    entries: BTreeMap<String, ModelRosterEntryV1>,
}

impl ModelRosterV1 {
    /// Validates a host-provided roster, rejecting duplicate IDs.
    pub fn new(entries: Vec<ModelRosterEntryV1>) -> Result<Self, RouterError> {
        let mut indexed = BTreeMap::new();
        for entry in entries {
            let id = entry.model_id.as_str().to_owned();
            if indexed.insert(id, entry).is_some() {
                return Err(RouterError::DuplicateModelId);
            }
        }
        Ok(Self { entries: indexed })
    }

    /// Iterates configured entries in model-ID order; routing never uses this
    /// order as a preference chain.
    pub fn entries(&self) -> impl Iterator<Item = &ModelRosterEntryV1> {
        self.entries.values()
    }
}

/// Host-resolved egress fact for one prepared call.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ModelEgressPolicySnapshotV1 {
    private_cloud_egress_allowed: bool,
}

impl ModelEgressPolicySnapshotV1 {
    /// Constructs the immutable fact supplied by the trusted host.
    pub fn from_host(private_cloud_egress_allowed: bool) -> Self {
        Self {
            private_cloud_egress_allowed,
        }
    }

    /// Whether the host snapshot permits private cloud egress. P4 V1 still
    /// refuses PRIVATE model calls because durable result protection is absent.
    pub fn private_cloud_egress_allowed(self) -> bool {
        self.private_cloud_egress_allowed
    }
}

/// Typed host facts re-resolved before each provider dispatch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ModelDispatchGateSnapshotV1 {
    cancelled: bool,
    egress: ModelEgressPolicySnapshotV1,
    effective_deadline_ms: u32,
}

/// Failure to read the host's already-resolved dispatch-gate facts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelDispatchGateSourceError {
    /// The host could not provide a current snapshot.
    Unavailable,
}

impl ModelDispatchGateSnapshotV1 {
    /// Constructs a snapshot from already-resolved host facts.
    pub fn from_host(
        cancelled: bool,
        egress: ModelEgressPolicySnapshotV1,
        effective_deadline_ms: u32,
    ) -> Self {
        Self {
            cancelled,
            egress,
            effective_deadline_ms,
        }
    }
}

/// Narrow trusted host seam for refreshing cancellation, egress, and deadline
/// facts before a dispatch. Implementations return resolved facts; Router does
/// not evaluate policy or query Task Engine.
pub trait ModelDispatchGateSource: Send + Sync {
    /// Reads current facts for one task/data-class dispatch.
    fn snapshot(
        &self,
        task_id: Option<&TaskId>,
        data_class: DataClass,
    ) -> Result<ModelDispatchGateSnapshotV1, ModelDispatchGateSourceError>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ResolvedDispatchGate {
    deadline_ms: u32,
}

fn resolve_dispatch_gate(
    call: &PreparedModelCallV1,
    snapshot: ModelDispatchGateSnapshotV1,
    deployment: ModelDeploymentClass,
) -> Result<ResolvedDispatchGate, RouterError> {
    if snapshot.cancelled {
        return Err(RouterError::TaskCancelled);
    }
    let deadline_ms = call.deadline_ms.min(snapshot.effective_deadline_ms);
    if deadline_ms == 0 {
        return Err(RouterError::DeadlineExpired);
    }
    let egress = ModelEgressPolicySnapshotV1::from_host(
        call.egress.private_cloud_egress_allowed && snapshot.egress.private_cloud_egress_allowed,
    );
    if !deployment_permitted(call.data_class, deployment, egress) {
        return Err(RouterError::EgressRevoked);
    }
    Ok(ResolvedDispatchGate { deadline_ms })
}

/// Semantic inputs prepared by the host before routing.
#[derive(Clone, PartialEq)]
pub struct PreparedModelCallDraftV1 {
    /// Optional task accounting identity.
    pub task_id: Option<TaskId>,
    /// Host-assigned purpose.
    pub purpose: ModelPurpose,
    /// Host-prepared conversation messages.
    pub messages: Vec<ModelMessage>,
    /// Optional host-prepared system prompt.
    pub system: Option<String>,
    /// Host-selected output format.
    pub response_format: ResponseFormat,
    /// Host-supplied tool schemas.
    pub tools: Vec<serde_json::Value>,
    /// Host-selected per-call output limit.
    pub max_output_tokens: u32,
    /// Finite host-selected sampling temperature.
    pub temperature: f64,
    /// Host-resolved deadline.
    pub deadline_ms: u32,
    /// Host-resolved data class.
    pub data_class: DataClass,
    /// Host-resolved routing requirements.
    pub requirements: ModelRoutingRequirementsV1,
    /// Immutable host-resolved egress facts.
    pub egress: ModelEgressPolicySnapshotV1,
    /// Configured host output bound for this process.
    pub host_max_output_tokens: u32,
}

/// Validated host-prepared call accepted by the routing core.
///
/// Fields are private so callers cannot mutate trusted facts after validation.
#[derive(Clone, PartialEq)]
pub struct PreparedModelCallV1 {
    task_id: Option<TaskId>,
    purpose: ModelPurpose,
    messages: Vec<ModelMessage>,
    system: Option<String>,
    response_format: ResponseFormat,
    tools: Vec<serde_json::Value>,
    max_output_tokens: u32,
    temperature: f64,
    deadline_ms: u32,
    data_class: DataClass,
    requirements: ModelRoutingRequirementsV1,
    egress: ModelEgressPolicySnapshotV1,
}

impl PreparedModelCallV1 {
    /// Validates a call produced by the trusted in-process host boundary.
    ///
    /// This does not prove redaction. It must not be exposed directly to an
    /// untrusted device or network API.
    pub fn from_host(draft: PreparedModelCallDraftV1) -> Result<Self, RouterError> {
        validate_purpose_format(draft.purpose, &draft.response_format, draft.requirements)?;
        if !draft.temperature.is_finite() {
            return Err(RouterError::NonFiniteTemperature);
        }
        if draft.max_output_tokens == 0
            || draft.max_output_tokens > draft.host_max_output_tokens
            || draft.host_max_output_tokens > DEFAULT_MAX_OUTPUT_TOKENS_PER_CALL
            || draft.requirements.min_output_tokens > draft.max_output_tokens
        {
            return Err(RouterError::OutputLimitInvalid);
        }
        if matches!(
            draft.data_class,
            DataClass::Private | DataClass::Secret | DataClass::Credential
        ) {
            return Err(RouterError::DataClassRefused);
        }
        if let ResponseFormat::JsonSchema { schema } = &draft.response_format {
            let mut counter = BoundedCounter::new(MAX_MODEL_SCHEMA_BYTES);
            if serde_json::to_writer(&mut counter, schema).is_err() {
                return Err(RouterError::SchemaTooLarge);
            }
            structured::validate_json_schema(schema).map_err(|_| RouterError::InvalidJsonSchema)?;
        }
        let mut prompt_bytes = draft.system.as_ref().map_or(0, String::len);
        for message in &draft.messages {
            prompt_bytes = prompt_bytes.saturating_add(message.content.len());
        }
        for tool in &draft.tools {
            if prompt_bytes > MAX_MODEL_PROMPT_BYTES {
                return Err(RouterError::PromptTooLarge);
            }
            let mut counter = BoundedCounter::new(MAX_MODEL_PROMPT_BYTES - prompt_bytes);
            if serde_json::to_writer(&mut counter, tool).is_err() {
                return Err(RouterError::PromptTooLarge);
            }
            prompt_bytes = prompt_bytes.saturating_add(counter.bytes_written());
        }
        if prompt_bytes > MAX_MODEL_PROMPT_BYTES {
            return Err(RouterError::PromptTooLarge);
        }
        Ok(Self {
            task_id: draft.task_id,
            purpose: draft.purpose,
            messages: draft.messages,
            system: draft.system,
            response_format: draft.response_format,
            tools: draft.tools,
            max_output_tokens: draft.max_output_tokens,
            temperature: draft.temperature,
            deadline_ms: draft.deadline_ms,
            data_class: draft.data_class,
            requirements: draft.requirements,
            egress: draft.egress,
        })
    }

    /// The host-assigned purpose.
    pub fn purpose(&self) -> ModelPurpose {
        self.purpose
    }

    /// Optional task accounting identity.
    pub fn task_id(&self) -> Option<&TaskId> {
        self.task_id.as_ref()
    }

    /// Prepared conversation messages.
    pub fn messages(&self) -> &[ModelMessage] {
        &self.messages
    }

    /// Optional host-prepared system prompt.
    pub fn system(&self) -> Option<&str> {
        self.system.as_deref()
    }

    /// Host-selected output format.
    pub fn response_format(&self) -> &ResponseFormat {
        &self.response_format
    }

    /// Host-supplied tool schemas.
    pub fn tools(&self) -> &[serde_json::Value] {
        &self.tools
    }

    /// Host-selected output token limit.
    pub fn max_output_tokens(&self) -> u32 {
        self.max_output_tokens
    }

    /// Host-selected finite temperature, returned without normalization.
    pub fn temperature(&self) -> f64 {
        self.temperature
    }

    /// Host-resolved deadline.
    pub fn deadline_ms(&self) -> u32 {
        self.deadline_ms
    }

    /// Host-resolved egress facts.
    pub fn egress(&self) -> ModelEgressPolicySnapshotV1 {
        self.egress
    }

    /// The required routing capabilities.
    pub fn requirements(&self) -> ModelRoutingRequirementsV1 {
        self.requirements
    }

    /// The host-resolved data classification.
    pub fn data_class(&self) -> DataClass {
        self.data_class
    }
}

struct BoundedCounter {
    limit: usize,
    written: usize,
}

impl BoundedCounter {
    fn new(limit: usize) -> Self {
        Self { limit, written: 0 }
    }

    fn bytes_written(&self) -> usize {
        self.written
    }
}

impl Write for BoundedCounter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let Some(next) = self.written.checked_add(bytes.len()) else {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "size overflow"));
        };
        if next > self.limit {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "size limit exceeded",
            ));
        }
        self.written = next;
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// Typed refusal or configuration error from P4C.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RouterError {
    /// A purpose and response format do not match the frozen matrix.
    IllegalPurposeFormat,
    /// Temperature is NaN or infinite.
    NonFiniteTemperature,
    /// A host-supplied JSON Schema is invalid or cannot be resolved offline.
    InvalidJsonSchema,
    /// Per-call output limit exceeds the host bound or is zero.
    OutputLimitInvalid,
    /// Schema bytes exceed the frozen structural limit.
    SchemaTooLarge,
    /// Prepared prompt bytes exceed the frozen structural limit.
    PromptTooLarge,
    /// Data class is not dispatchable in P4 V1.
    DataClassRefused,
    /// Vision is required but this request surface has no image transport.
    VisionInputUnsupported,
    /// A model ID is outside the configured initial roster or is Codex.
    UnknownOrForbiddenModel,
    /// Duplicate configured model identity.
    DuplicateModelId,
    /// More than one provider object registered the same provider identity.
    DuplicateProviderId,
    /// A provider advertised a descriptor under an identity it does not own.
    ProviderIdentityMismatch,
    /// Provider discovery repeated a configured ModelId and is ambiguous.
    DuplicateDiscoveredModel,
    /// A provider response did not match the host-selected dispatch identity.
    ProviderProtocolFailure,
    /// The selected model is not the eligible result of the supplied session.
    ModelSelectionInvalid,
    /// The model price snapshot conflicts with host roster cost class.
    PriceConfigurationInvalid,
    /// The trusted host dispatch-gate source could not provide current facts.
    DispatchGateUnavailable,
    /// The host reports that the task has been cancelled.
    TaskCancelled,
    /// No deadline eligibility remains for a new dispatch.
    DeadlineExpired,
    /// A fresh host egress snapshot revoked the selected deployment.
    EgressRevoked,
}

enum ModelDispatchFailure {
    Refused(RouterError),
    Storage(StoreError),
    Clock(ProtocolError),
    Protocol(ProtocolError),
    NoEligibleModel,
    DefiniteProvider {
        request_id: serea_protocol::RequestId,
        error_kind: ModelErrorCode,
        retryable: bool,
    },
    AmbiguousProvider {
        request_id: serea_protocol::RequestId,
        error_kind: ModelErrorCode,
    },
    TerminalProvider {
        request_id: serea_protocol::RequestId,
        error_kind: ModelErrorCode,
    },
    StructuredOutputInvalid {
        request_id: serea_protocol::RequestId,
        // Transient in-process input for the repair ladder; never log or persist.
        #[allow(dead_code)]
        raw_response: String,
        validation_error: StructuredValidationError,
    },
    TokenBudgetExceeded {
        limit: u64,
        observed: u64,
    },
    ModelCallBudgetExceeded {
        limit: u64,
        observed: u64,
    },
    BoundExceeded {
        bound: ModelBoundKindV1,
        limit: u64,
        observed: u64,
    },
}

/// Sanitized typed failure returned across the trusted host-call boundary.
///
/// This intentionally has no raw provider content or prompt fields. Task
/// lifecycle ownership remains with the host caller.
#[derive(Debug)]
pub enum ModelRouterCallFailureV1 {
    /// Host-preparation or dispatch-gate refusal.
    Refused(RouterError),
    /// Durable storage or accounting failure.
    Storage(StoreError),
    /// Clock or protocol representation failure.
    Protocol(ProtocolError),
    /// No candidate survived the deterministic chain and filters.
    NoEligibleModel,
    /// Provider returned a definite failure; retryability is adapter supplied.
    DefiniteProvider {
        /// Durable dispatch identity.
        request_id: serea_protocol::RequestId,
        /// Stable typed error kind.
        error_kind: ModelErrorCode,
        /// Whether the adapter proved the call is retryable.
        retryable: bool,
    },
    /// Provider outcome is ambiguous and must not be retried automatically.
    AmbiguousProvider {
        /// Durable dispatch identity.
        request_id: serea_protocol::RequestId,
        /// Stable typed error kind.
        error_kind: ModelErrorCode,
    },
    /// Provider returned a terminal failure.
    TerminalProvider {
        /// Durable dispatch identity.
        request_id: serea_protocol::RequestId,
        /// Stable typed error kind.
        error_kind: ModelErrorCode,
    },
    /// Host validation rejected structured output. Raw output is not exposed.
    StructuredValidation {
        /// Durable dispatch identity.
        request_id: serea_protocol::RequestId,
        /// Bounded host validation classification.
        error: StructuredValidationError,
    },
    /// Per-task token or model-call budget refused the logical operation.
    TaskBudgetExceeded {
        /// Frozen bound kind.
        bound: ModelBoundKindV1,
        /// Configured maximum.
        limit: u64,
        /// Host-observed usage.
        observed: u64,
    },
    /// Frozen model-call count bound was exhausted before dispatch.
    ModelCallBudgetExceeded {
        /// Configured maximum.
        limit: u64,
        /// Host-observed number of reserved calls.
        observed: u64,
    },
    /// Generic host bounds outcome such as daily spend exhaustion.
    BoundExceeded {
        /// Frozen bound kind.
        bound: ModelBoundKindV1,
        /// Configured maximum.
        limit: u64,
        /// Host-observed usage.
        observed: u64,
    },
}

/// Failure while classifying unresolved dispatches during process startup.
#[derive(Debug)]
pub enum ModelRecoveryError {
    /// Durable read, state mutation, or audit-event insertion failed.
    Storage(StoreError),
    /// Host recovery timestamp could not be read or encoded.
    Protocol(ProtocolError),
}

struct ModelDispatchContext<'a> {
    store: &'a Store,
    events: &'a EventBus,
    price: ModelPriceSnapshot,
    prices: Option<&'a BTreeMap<String, ModelPriceSnapshot>>,
    max_daily_spend_usd_micros: UsdMicros,
    clock: &'a dyn Clock,
    gate: &'a dyn ModelDispatchGateSource,
}

/// Trusted in-process host context for one logical model operation.
///
/// This context is not an Android or network API. The host supplies resolved
/// storage, event, clock, and dispatch-gate handles. Immutable price and spend
/// configuration belongs to `ModelRouterProcessV1`. Router does not evaluate
/// policy or take ownership of Task lifecycle transitions.
pub struct ModelRouterHostContextV1<'a> {
    store: &'a Store,
    events: &'a EventBus,
    clock: &'a dyn Clock,
    gate: &'a dyn ModelDispatchGateSource,
}

impl<'a> ModelRouterHostContextV1<'a> {
    /// Binds the already-resolved host facts used by Router for this call.
    pub fn new(
        store: &'a Store,
        events: &'a EventBus,
        clock: &'a dyn Clock,
        gate: &'a dyn ModelDispatchGateSource,
    ) -> Self {
        Self {
            store,
            events,
            clock,
            gate,
        }
    }
}

impl ModelDispatchContext<'_> {
    fn price_for(&self, model_id: &ModelId) -> Result<&ModelPriceSnapshot, ModelDispatchFailure> {
        match self.prices {
            Some(prices) => prices
                .get(model_id.as_str())
                .ok_or(ModelDispatchFailure::Refused(
                    RouterError::PriceConfigurationInvalid,
                )),
            None => Ok(&self.price),
        }
    }
}

struct ModelFailureFacts<'a> {
    request_id: &'a serea_protocol::RequestId,
    metadata: ModelEventMetadataV1,
    error_kind: ModelErrorCode,
    retryable: bool,
    terminal_at: serea_protocol::EpochMillis,
    usage: Option<ModelFailureUsage>,
}

#[derive(Clone)]
struct PendingFallback {
    request_id: serea_protocol::RequestId,
    error_kind: ModelErrorCode,
    metadata: ModelEventMetadataV1,
}

struct DispatchLineage {
    relation_kind: ModelAttemptRelationKind,
    parent_request_id: Option<serea_protocol::RequestId>,
    repair_attempts: u8,
    fallback_from_model_id: Option<ModelId>,
    pending_fallback: Option<PendingFallback>,
    defer_retryable_failure: bool,
}

struct DispatchBudgetFacts<'a> {
    price: &'a ModelPriceSnapshot,
    daily_limit: UsdMicros,
    max_context_tokens: u64,
    max_output_tokens: u64,
    dispatch_at: serea_protocol::EpochMillis,
}

impl DispatchLineage {
    fn normal() -> Self {
        Self {
            relation_kind: ModelAttemptRelationKind::None,
            parent_request_id: None,
            repair_attempts: 0,
            fallback_from_model_id: None,
            pending_fallback: None,
            defer_retryable_failure: false,
        }
    }
}

/// Binds a provider response to the exact dispatch identity and removes fields
/// that are host-owned or untrusted at provider ingress.
///
/// The response content remains unvalidated. Callers must apply the purpose's
/// finish-reason and response validation rules before persisting or returning
/// it. In particular, provider-supplied `structured` and `repair_attempts`
/// values are never authority.
pub fn bind_provider_response(
    expected_request_id: &serea_protocol::RequestId,
    expected_model_id: &ModelId,
    expected_provider_id: &ProviderId,
    mut response: serea_protocol::ModelResponse,
) -> Result<serea_protocol::ModelResponse, RouterError> {
    if response.request_id != *expected_request_id
        || response.model_id != *expected_model_id
        || response.provider_id != *expected_provider_id
    {
        return Err(RouterError::ProviderProtocolFailure);
    }
    response.structured = None;
    response.repair_attempts = 0;
    Ok(response)
}

/// Immutable provider-health facts for one logical normal operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderHealthSnapshotV1 {
    health: BTreeMap<String, ProviderHealth>,
}

impl ProviderHealthSnapshotV1 {
    /// Health for one registered provider. Missing providers are degraded.
    pub fn health(&self, id: &ProviderId) -> ProviderHealth {
        self.health
            .get(id.as_str())
            .copied()
            .unwrap_or(ProviderHealth::Degraded)
    }
}

/// Deterministic route result and reusable health snapshot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoutingSessionV1 {
    /// Selected model, if a chain candidate survived all filters.
    decision: Option<ModelId>,
    /// One immutable health snapshot for this logical operation.
    health: ProviderHealthSnapshotV1,
}

impl RoutingSessionV1 {
    /// The deterministic selected model, if a candidate survived all filters.
    pub fn decision(&self) -> Option<&ModelId> {
        self.decision.as_ref()
    }

    /// The immutable health snapshot reused by later dispatch decisions.
    pub fn health(&self) -> &ProviderHealthSnapshotV1 {
        &self.health
    }
}

/// Host-owned routing instance with immutable roster, discovery and provider set.
pub struct ModelRouterV1 {
    roster: ModelRosterV1,
    providers: BTreeMap<String, (ProviderId, Arc<dyn ModelProvider>)>,
    discovered: BTreeMap<String, ModelDescriptor>,
}

/// Immutable process-lifetime Router, provider, price, and daily-spend config.
///
/// Reconfiguration requires constructing a new process router. In-flight calls
/// retain the exact per-model price snapshot used when their dispatch intents
/// were committed.
pub struct ModelRouterProcessV1 {
    router: ModelRouterV1,
    prices: BTreeMap<String, ModelPriceSnapshot>,
    max_daily_spend_usd_micros: UsdMicros,
}

impl ModelRouterProcessV1 {
    /// Constructs the immutable process config and validates every roster price.
    pub fn new(
        roster: ModelRosterV1,
        providers: Vec<Arc<dyn ModelProvider>>,
        prices: Vec<(ModelId, ModelPriceSnapshot)>,
        max_daily_spend_usd_micros: UsdMicros,
    ) -> Result<Self, RouterError> {
        let mut price_map = BTreeMap::new();
        for (model_id, price) in prices {
            let Some(entry) = roster.entries.get(model_id.as_str()) else {
                return Err(RouterError::PriceConfigurationInvalid);
            };
            if entry.cost_class != price.cost_class()
                || validate_price_snapshot(&price).is_err()
                || price_map
                    .insert(model_id.as_str().to_owned(), price)
                    .is_some()
            {
                return Err(RouterError::PriceConfigurationInvalid);
            }
        }
        if roster
            .entries
            .keys()
            .any(|model_id| !price_map.contains_key(model_id))
        {
            return Err(RouterError::PriceConfigurationInvalid);
        }
        let router = ModelRouterV1::new(roster, providers)?;
        Ok(Self {
            router,
            prices: price_map,
            max_daily_spend_usd_micros,
        })
    }

    /// Executes one trusted host-prepared logical model operation.
    ///
    /// Selection, fallback, and structured repair remain inside Router. The
    /// returned error omits raw provider content. This API is an in-process
    /// host boundary and must not be exposed directly to Android or network
    /// callers.
    pub async fn execute(
        &self,
        call: &PreparedModelCallV1,
        host: &ModelRouterHostContextV1<'_>,
    ) -> Result<ModelResponse, ModelRouterCallFailureV1> {
        let session = self
            .router
            .route(call)
            .await
            .map_err(ModelRouterCallFailureV1::Refused)?;
        let Some(default_price) = self.prices.values().next().cloned() else {
            return Err(ModelRouterCallFailureV1::Refused(
                RouterError::PriceConfigurationInvalid,
            ));
        };
        let dispatch = ModelDispatchContext {
            store: host.store,
            events: host.events,
            price: default_price,
            prices: Some(&self.prices),
            max_daily_spend_usd_micros: self.max_daily_spend_usd_micros,
            clock: host.clock,
            gate: host.gate,
        };
        let result = if call.purpose == ModelPurpose::Chat
            && matches!(call.response_format, ResponseFormat::Text)
        {
            self.router
                .dispatch_chat_text_with_fallback(call, &session, &dispatch)
                .await
        } else {
            self.router
                .dispatch_structured_with_fallback_and_repair(call, &session, &dispatch)
                .await
        };
        result.map_err(host_call_failure)
    }
}

impl ModelRouterV1 {
    /// Validates provider registration and snapshots discovery metadata.
    pub fn new(
        roster: ModelRosterV1,
        providers: Vec<Arc<dyn ModelProvider>>,
    ) -> Result<Self, RouterError> {
        let mut registered = BTreeMap::new();
        for provider in providers {
            let id = provider.provider_id();
            if registered
                .insert(id.as_str().to_owned(), (id, provider))
                .is_some()
            {
                return Err(RouterError::DuplicateProviderId);
            }
        }
        let mut discovered = BTreeMap::new();
        for (provider_id, provider) in registered.values() {
            for descriptor in provider.models() {
                if descriptor.provider_id != *provider_id {
                    return Err(RouterError::ProviderIdentityMismatch);
                }
                let key = descriptor.model_id.as_str().to_owned();
                let is_configured_for_provider = roster
                    .entries
                    .get(&key)
                    .is_some_and(|entry| entry.provider_id == *provider_id);
                if is_configured_for_provider && discovered.contains_key(&key) {
                    return Err(RouterError::DuplicateDiscoveredModel);
                }
                if is_configured_for_provider {
                    discovered.insert(key, descriptor);
                }
            }
        }
        Ok(Self {
            roster,
            providers: registered,
            discovered,
        })
    }

    /// Takes one health read per registered provider and routes without calling
    /// `generate()`. The returned snapshot is retained for later P4E fallback.
    pub async fn route(&self, call: &PreparedModelCallV1) -> Result<RoutingSessionV1, RouterError> {
        validate_purpose_format(call.purpose, &call.response_format, call.requirements)?;
        if call.requirements.vision_required {
            return Err(RouterError::VisionInputUnsupported);
        }
        let mut health = BTreeMap::new();
        for (id, provider) in self.providers.values() {
            let state = provider.health().await;
            health.insert(id.as_str().to_owned(), state);
        }
        let snapshot = ProviderHealthSnapshotV1 { health };
        let chain = preference_chain(call.purpose);
        let decision = chain.iter().find_map(|model_id| {
            let id = ModelId::new(*model_id).ok()?;
            let entry = self.roster.entries.get(*model_id)?;
            let advertised = self.discovered.get(*model_id)?;
            let effective = intersect(entry.allowed_capabilities, advertised.capabilities);
            if !entry.enabled
                || snapshot.health(&entry.provider_id) != ProviderHealth::Ready
                || !deployment_permitted(call.data_class, entry.deployment_class, call.egress)
                || !capabilities_satisfy(
                    effective,
                    call.requirements,
                    matches!(call.response_format, ResponseFormat::JsonSchema { .. }),
                )
            {
                return None;
            }
            Some(id)
        });
        Ok(RoutingSessionV1 {
            decision,
            health: snapshot,
        })
    }

    /// Constructs a provider request only from a selected candidate in the
    /// exact routing session. All request semantics are copied from the
    /// immutable host-prepared call; the model identity is supplied by the
    /// routing decision, never by prepared or provider-authored content.
    pub fn build_request(
        &self,
        call: &PreparedModelCallV1,
        session: &RoutingSessionV1,
        request_id: &serea_protocol::RequestId,
        selected_model_id: &ModelId,
    ) -> Result<ModelRequest, RouterError> {
        validate_purpose_format(call.purpose, &call.response_format, call.requirements)?;
        if call.requirements.vision_required
            || session.decision.as_ref() != Some(selected_model_id)
            || !preference_chain(call.purpose).contains(&selected_model_id.as_str())
        {
            return Err(RouterError::ModelSelectionInvalid);
        }
        let entry = self
            .roster
            .entries
            .get(selected_model_id.as_str())
            .ok_or(RouterError::ModelSelectionInvalid)?;
        let advertised = self
            .discovered
            .get(selected_model_id.as_str())
            .ok_or(RouterError::ModelSelectionInvalid)?;
        let provider = self
            .providers
            .get(entry.provider_id.as_str())
            .ok_or(RouterError::ModelSelectionInvalid)?;
        let effective = intersect(entry.allowed_capabilities, advertised.capabilities);
        if !entry.enabled
            || provider.0 != entry.provider_id
            || session.health.health(&entry.provider_id) != ProviderHealth::Ready
            || !deployment_permitted(call.data_class, entry.deployment_class, call.egress)
            || !capabilities_satisfy(
                effective,
                call.requirements,
                matches!(call.response_format, ResponseFormat::JsonSchema { .. }),
            )
        {
            return Err(RouterError::ModelSelectionInvalid);
        }
        Ok(ModelRequest {
            request_id: request_id.clone(),
            model_id: selected_model_id.clone(),
            task_id: call.task_id.clone(),
            purpose: call.purpose,
            messages: call.messages.clone(),
            system: call.system.clone(),
            response_format: call.response_format.clone(),
            tools: call.tools.clone(),
            max_output_tokens: call.max_output_tokens,
            temperature: call.temperature,
            deadline_ms: call.deadline_ms,
            data_class: call.data_class,
        })
    }

    // Kept crate-private because callers must enter through execute().
    #[cfg(test)]
    pub(crate) async fn dispatch_chat_text(
        &self,
        call: &PreparedModelCallV1,
        session: &RoutingSessionV1,
        context: &ModelDispatchContext<'_>,
    ) -> Result<ModelResponse, ModelDispatchFailure> {
        if call.purpose != ModelPurpose::Chat
            || !matches!(call.response_format, ResponseFormat::Text)
        {
            return Err(ModelDispatchFailure::Refused(
                RouterError::IllegalPurposeFormat,
            ));
        }
        self.dispatch_prepared(call, session, context, DispatchLineage::normal())
            .await
    }

    /// Runs a CHAT/TEXT call with exactly one normal fallback. The normal
    /// health snapshot is reused, and the primary failure plus fallback intent
    /// are committed in the same transaction.
    pub(crate) async fn dispatch_chat_text_with_fallback(
        &self,
        call: &PreparedModelCallV1,
        session: &RoutingSessionV1,
        context: &ModelDispatchContext<'_>,
    ) -> Result<ModelResponse, ModelDispatchFailure> {
        if call.purpose != ModelPurpose::Chat
            || !matches!(call.response_format, ResponseFormat::Text)
        {
            return Err(ModelDispatchFailure::Refused(
                RouterError::IllegalPurposeFormat,
            ));
        }
        self.dispatch_with_fallback(call, session, context).await
    }

    async fn dispatch_with_fallback(
        &self,
        call: &PreparedModelCallV1,
        session: &RoutingSessionV1,
        context: &ModelDispatchContext<'_>,
    ) -> Result<ModelResponse, ModelDispatchFailure> {
        let is_chat_text = call.purpose == ModelPurpose::Chat
            && matches!(call.response_format, ResponseFormat::Text);
        let is_structured = call.purpose != ModelPurpose::Chat
            && matches!(call.response_format, ResponseFormat::JsonSchema { .. });
        if !is_chat_text && !is_structured {
            return Err(ModelDispatchFailure::Refused(
                RouterError::IllegalPurposeFormat,
            ));
        }
        let primary_model = session
            .decision()
            .cloned()
            .ok_or(ModelDispatchFailure::NoEligibleModel)?;
        let primary_result = self
            .dispatch_prepared(
                call,
                session,
                context,
                DispatchLineage {
                    defer_retryable_failure: true,
                    ..DispatchLineage::normal()
                },
            )
            .await;
        let (primary_request_id, primary_error_kind) = match primary_result {
            Ok(response) => return Ok(response),
            Err(ModelDispatchFailure::DefiniteProvider {
                request_id,
                error_kind,
                retryable: true,
            }) => (request_id, error_kind),
            Err(error) => return Err(error),
        };

        let primary_entry = self.roster.entries.get(primary_model.as_str()).ok_or(
            ModelDispatchFailure::Refused(RouterError::ModelSelectionInvalid),
        )?;
        let failed_at = context
            .clock
            .now_ms()
            .map_err(ModelDispatchFailure::Clock)?;
        let pending = PendingFallback {
            request_id: primary_request_id.clone(),
            error_kind: primary_error_kind.clone(),
            metadata: ModelEventMetadataV1 {
                request_id: primary_request_id,
                model_id: primary_model.clone(),
                provider_id: primary_entry.provider_id.clone(),
                task_id: call.task_id.clone(),
                purpose: call.purpose,
                relation: ModelEventRelationV1::Normal,
                data_class: call.data_class,
                occurred_at: failed_at,
            },
        };
        let Some(fallback_model) = self.fallback_candidate(call, session, &primary_model) else {
            persist_pending_primary_failure(context, &pending, failed_at)?;
            return Err(ModelDispatchFailure::DefiniteProvider {
                request_id: pending.request_id,
                error_kind: pending.error_kind,
                retryable: true,
            });
        };
        let fallback_session = RoutingSessionV1 {
            decision: Some(fallback_model),
            health: session.health.clone(),
        };
        let fallback_model = fallback_session
            .decision()
            .cloned()
            .ok_or(ModelDispatchFailure::NoEligibleModel)?;
        let primary_request_id = pending.request_id.clone();
        let result = self
            .dispatch_prepared(
                call,
                &fallback_session,
                context,
                DispatchLineage {
                    relation_kind: ModelAttemptRelationKind::Fallback,
                    parent_request_id: Some(pending.request_id.clone()),
                    fallback_from_model_id: Some(primary_model),
                    pending_fallback: Some(pending.clone()),
                    ..DispatchLineage::normal()
                },
            )
            .await;
        let fallback_committed = if result.is_err() {
            let state = context
                .store
                .get_model_call_attempt(&primary_request_id)
                .map_err(ModelDispatchFailure::Storage)?
                .map(|attempt| attempt.state);
            if state == Some(ModelAttemptState::DispatchIntent) {
                let terminal_at = context
                    .clock
                    .now_ms()
                    .map_err(ModelDispatchFailure::Clock)?;
                persist_pending_primary_failure(context, &pending, terminal_at)?;
                false
            } else {
                state == Some(ModelAttemptState::Failed)
            }
        } else {
            false
        };
        if fallback_committed {
            let occurred_at = context
                .clock
                .now_ms()
                .map_err(ModelDispatchFailure::Clock)?;
            let exhausted = context
                .events
                .draft_model_fallback_exhausted(ModelFallbackExhaustedEventV1 {
                    metadata: ModelEventMetadataV1 {
                        occurred_at,
                        ..pending.metadata.clone()
                    },
                    fallback_model_id: fallback_model,
                })
                .map_err(ModelDispatchFailure::Storage)?;
            context
                .store
                .transact(|tx| {
                    tx.append_event(exhausted.event, exhausted.retention_at)?;
                    Ok(())
                })
                .map_err(ModelDispatchFailure::Storage)?;
        }
        result
    }

    fn fallback_candidate(
        &self,
        call: &PreparedModelCallV1,
        session: &RoutingSessionV1,
        primary: &ModelId,
    ) -> Option<ModelId> {
        let mut after_primary = false;
        preference_chain(call.purpose)
            .iter()
            .copied()
            .find_map(|model| {
                if !after_primary {
                    if model == primary.as_str() {
                        after_primary = true;
                    }
                    return None;
                }
                let entry = self.roster.entries.get(model)?;
                let advertised = self.discovered.get(model)?;
                let effective = intersect(entry.allowed_capabilities, advertised.capabilities);
                (entry.enabled
                    && session.health.health(&entry.provider_id) == ProviderHealth::Ready
                    && deployment_permitted(call.data_class, entry.deployment_class, call.egress)
                    && capabilities_satisfy(
                        effective,
                        call.requirements,
                        matches!(call.response_format, ResponseFormat::JsonSchema { .. }),
                    ))
                .then(|| ModelId::new(model).ok())
                .flatten()
            })
    }

    #[allow(dead_code)] // The repair ladder will become its production caller in P4E.
    pub(crate) async fn dispatch_structured(
        &self,
        call: &PreparedModelCallV1,
        session: &RoutingSessionV1,
        context: &ModelDispatchContext<'_>,
    ) -> Result<ModelResponse, ModelDispatchFailure> {
        if call.purpose == ModelPurpose::Chat
            || !matches!(call.response_format, ResponseFormat::JsonSchema { .. })
        {
            return Err(ModelDispatchFailure::Refused(
                RouterError::IllegalPurposeFormat,
            ));
        }
        self.dispatch_prepared(call, session, context, DispatchLineage::normal())
            .await
    }

    /// Runs a structured operation with the bounded P4 repair ladder. The
    /// original host call remains the authority; repair requests contain only
    /// its schema, the bounded invalid response, and sanitized diagnostics.
    pub(crate) async fn dispatch_structured_with_repair(
        &self,
        call: &PreparedModelCallV1,
        session: &RoutingSessionV1,
        context: &ModelDispatchContext<'_>,
    ) -> Result<ModelResponse, ModelDispatchFailure> {
        let schema = match &call.response_format {
            ResponseFormat::JsonSchema { schema } if call.purpose != ModelPurpose::Chat => schema,
            _ => {
                return Err(ModelDispatchFailure::Refused(
                    RouterError::IllegalPurposeFormat,
                ));
            }
        };
        let initial = match self.dispatch_with_fallback(call, session, context).await {
            Ok(response) => return Ok(response),
            Err(error @ ModelDispatchFailure::StructuredOutputInvalid { .. }) => error,
            Err(error) => return Err(error),
        };

        let ModelDispatchFailure::StructuredOutputInvalid {
            request_id: original_request_id,
            raw_response,
            validation_error,
        } = initial
        else {
            return Err(ModelDispatchFailure::Refused(
                RouterError::ModelSelectionInvalid,
            ));
        };
        if raw_response.len() > MAX_MODEL_RESPONSE_BYTES
            || matches!(
                validation_error,
                StructuredValidationError::ResponseTooLarge
            )
        {
            return Err(ModelDispatchFailure::StructuredOutputInvalid {
                request_id: original_request_id,
                raw_response,
                validation_error,
            });
        }

        let Some(repair_session) = self.repair_session(call).await else {
            return Err(ModelDispatchFailure::StructuredOutputInvalid {
                request_id: original_request_id,
                raw_response,
                validation_error,
            });
        };
        let mut parent_request_id = original_request_id;
        let mut current_raw = raw_response;
        let mut current_validation_error = validation_error;

        for ordinal in 1..=2u8 {
            let repair_call =
                build_repair_call(call, schema, &current_raw, &current_validation_error)
                    .map_err(ModelDispatchFailure::Refused)?;
            match self
                .dispatch_prepared(
                    &repair_call,
                    &repair_session,
                    context,
                    DispatchLineage {
                        relation_kind: ModelAttemptRelationKind::Repair,
                        parent_request_id: Some(parent_request_id.clone()),
                        repair_attempts: ordinal,
                        ..DispatchLineage::normal()
                    },
                )
                .await
            {
                Ok(response) => return Ok(response),
                Err(ModelDispatchFailure::StructuredOutputInvalid {
                    request_id,
                    raw_response,
                    validation_error,
                }) if ordinal < 2
                    && raw_response.len() <= MAX_MODEL_RESPONSE_BYTES
                    && !matches!(
                        validation_error,
                        StructuredValidationError::ResponseTooLarge
                    ) =>
                {
                    parent_request_id = request_id;
                    current_raw = raw_response;
                    current_validation_error = validation_error;
                }
                Err(ModelDispatchFailure::DefiniteProvider { request_id, .. }) if ordinal < 2 => {
                    parent_request_id = request_id;
                }
                Err(ModelDispatchFailure::DefiniteProvider { request_id, .. }) => {
                    return Err(ModelDispatchFailure::StructuredOutputInvalid {
                        request_id,
                        raw_response: current_raw,
                        validation_error: current_validation_error,
                    });
                }
                Err(error) => return Err(error),
            }
        }
        Err(ModelDispatchFailure::StructuredOutputInvalid {
            request_id: parent_request_id,
            raw_response: current_raw,
            validation_error: current_validation_error,
        })
    }

    /// Runs a structured operation through its one-step fallback and bounded
    /// repair ladder.
    pub(crate) async fn dispatch_structured_with_fallback_and_repair(
        &self,
        call: &PreparedModelCallV1,
        session: &RoutingSessionV1,
        context: &ModelDispatchContext<'_>,
    ) -> Result<ModelResponse, ModelDispatchFailure> {
        self.dispatch_structured_with_repair(call, session, context)
            .await
    }

    #[allow(dead_code)] // Used by the P4E structured repair ladder.
    async fn repair_session(&self, original: &PreparedModelCallV1) -> Option<RoutingSessionV1> {
        let model_id = ModelId::new("gpt-oss-20b").ok()?;
        let entry = self.roster.entries.get(model_id.as_str())?;
        let advertised = self.discovered.get(model_id.as_str())?;
        let provider = self.providers.get(entry.provider_id.as_str())?;
        let health = provider.1.health().await;
        let effective = intersect(entry.allowed_capabilities, advertised.capabilities);
        let requirements = ModelRoutingRequirementsV1 {
            vision_required: false,
            tools_required: false,
            min_context_tokens: 1,
            min_output_tokens: 1,
            structured_requirement: StructuredRequirementV1::Strict,
        };
        let permitted = entry.enabled
            && provider.0 == entry.provider_id
            && health == ProviderHealth::Ready
            && deployment_permitted(original.data_class, entry.deployment_class, original.egress)
            && capabilities_satisfy(effective, requirements, true);
        let session = RoutingSessionV1 {
            decision: permitted.then(|| model_id.clone()),
            health: ProviderHealthSnapshotV1 {
                health: BTreeMap::from([(entry.provider_id.as_str().to_owned(), health)]),
            },
        };
        permitted.then_some(session)
    }

    async fn dispatch_prepared(
        &self,
        call: &PreparedModelCallV1,
        session: &RoutingSessionV1,
        context: &ModelDispatchContext<'_>,
        lineage: DispatchLineage,
    ) -> Result<ModelResponse, ModelDispatchFailure> {
        let DispatchLineage {
            relation_kind,
            parent_request_id,
            repair_attempts,
            fallback_from_model_id,
            pending_fallback,
            defer_retryable_failure,
        } = lineage;
        let store = context.store;
        let events = context.events;
        let max_daily_spend_usd_micros = context.max_daily_spend_usd_micros;
        let clock = context.clock;
        let is_chat_text = call.purpose == ModelPurpose::Chat
            && matches!(call.response_format, ResponseFormat::Text);
        let is_structured = call.purpose != ModelPurpose::Chat
            && matches!(call.response_format, ResponseFormat::JsonSchema { .. });
        if !is_chat_text && !is_structured {
            return Err(ModelDispatchFailure::Refused(
                RouterError::IllegalPurposeFormat,
            ));
        }
        enforce_task_token_bound(context, call)?;
        let selected = session
            .decision()
            .ok_or(ModelDispatchFailure::NoEligibleModel)?
            .clone();
        let price = context.price_for(&selected)?;
        let entry =
            self.roster
                .entries
                .get(selected.as_str())
                .ok_or(ModelDispatchFailure::Refused(
                    RouterError::ModelSelectionInvalid,
                ))?;
        let gate_snapshot = context
            .gate
            .snapshot(call.task_id.as_ref(), call.data_class)
            .map_err(|_| ModelDispatchFailure::Refused(RouterError::DispatchGateUnavailable))?;
        let gate = resolve_dispatch_gate(call, gate_snapshot, entry.deployment_class)
            .map_err(ModelDispatchFailure::Refused)?;
        if price.cost_class() != entry.cost_class {
            return Err(ModelDispatchFailure::Refused(
                RouterError::PriceConfigurationInvalid,
            ));
        }
        let advertised =
            self.discovered
                .get(selected.as_str())
                .ok_or(ModelDispatchFailure::Refused(
                    RouterError::ModelSelectionInvalid,
                ))?;
        let effective = intersect(entry.allowed_capabilities, advertised.capabilities);
        let effective_max_output_tokens = call.max_output_tokens.min(effective.max_output_tokens);
        let request_id = events
            .mint_model_request_id()
            .map_err(ModelDispatchFailure::Storage)?;
        let mut request = self
            .build_request(call, session, &request_id, &selected)
            .map_err(ModelDispatchFailure::Refused)?;
        request.deadline_ms = gate.deadline_ms;
        let provider = self
            .providers
            .get(entry.provider_id.as_str())
            .map(|(_, provider)| Arc::clone(provider))
            .ok_or(ModelDispatchFailure::Refused(
                RouterError::ModelSelectionInvalid,
            ))?;
        let dispatch_at = clock.now_ms().map_err(ModelDispatchFailure::Clock)?;
        let metadata = ModelEventMetadataV1 {
            request_id: request_id.clone(),
            model_id: selected.clone(),
            provider_id: entry.provider_id.clone(),
            task_id: call.task_id.clone(),
            purpose: call.purpose,
            relation: event_relation(relation_kind),
            data_class: call.data_class,
            occurred_at: dispatch_at,
        };
        let called = events
            .draft_model_called(metadata.clone())
            .map_err(ModelDispatchFailure::Storage)?;
        let fallback_transition = pending_fallback
            .as_ref()
            .map(|pending| {
                let failed = events.draft_model_failed(ModelFailedEventV1 {
                    metadata: pending.metadata.clone(),
                    error_kind: pending.error_kind.clone(),
                    retryable: true,
                })?;
                let fallback = events.draft_model_fallback(ModelFallbackEventV1 {
                    metadata: pending.metadata.clone(),
                    fallback_request_id: request_id.clone(),
                    fallback_model_id: selected.clone(),
                })?;
                Ok::<_, StoreError>((failed, fallback))
            })
            .transpose()
            .map_err(ModelDispatchFailure::Storage)?;
        let attempt = ModelCallAttemptDraft {
            request_id: request_id.clone(),
            task_id: call.task_id.clone(),
            purpose: call.purpose,
            model_id: selected.clone(),
            provider_id: entry.provider_id.clone(),
            deployment_class: match entry.deployment_class {
                ModelDeploymentClass::Cloud => serea_storage::ModelDeploymentClass::Cloud,
                ModelDeploymentClass::Local => serea_storage::ModelDeploymentClass::Local,
            },
            data_class: call.data_class,
            relation_kind,
            parent_request_id,
            fallback_from_model_id,
            price: price.clone(),
            max_context_tokens: u64::from(effective.max_context_tokens),
            effective_max_output_tokens: u64::from(effective_max_output_tokens),
            dispatch_intent_at: dispatch_at,
        };
        let intent_result = store.transact(|tx| {
            if let Some((failed, fallback)) = &fallback_transition {
                tx.fail_model_call(
                    &pending_fallback
                        .as_ref()
                        .ok_or(StoreError::InvalidModelCall)?
                        .request_id,
                    pending_fallback
                        .as_ref()
                        .ok_or(StoreError::InvalidModelCall)?
                        .error_kind
                        .as_str(),
                    dispatch_at,
                )?;
                tx.append_event(failed.event.clone(), failed.retention_at)?;
                tx.append_event(fallback.event.clone(), fallback.retention_at)?;
            }
            tx.reserve_model_call(attempt, max_daily_spend_usd_micros)?;
            tx.append_event(called.event, called.retention_at)?;
            Ok(())
        });
        if let Err(error) = intent_result {
            if let Some(failure) = record_dispatch_budget_refusal(
                context,
                call,
                &error,
                DispatchBudgetFacts {
                    price,
                    daily_limit: max_daily_spend_usd_micros,
                    max_context_tokens: u64::from(effective.max_context_tokens),
                    max_output_tokens: u64::from(effective_max_output_tokens),
                    dispatch_at,
                },
            )? {
                return Err(failure);
            }
            return Err(ModelDispatchFailure::Storage(error));
        }

        // The host state can change while the intent transaction commits. Read
        // it once more immediately before crossing the provider boundary.
        let before_provider = context
            .gate
            .snapshot(call.task_id.as_ref(), call.data_class)
            .map_err(|_| RouterError::DispatchGateUnavailable)
            .and_then(|snapshot| resolve_dispatch_gate(call, snapshot, entry.deployment_class));
        let latest_gate = match before_provider {
            Ok(gate) => gate,
            Err(error) => {
                let terminal_at = clock.now_ms().map_err(ModelDispatchFailure::Clock)?;
                let error_kind = stable_model_error("HOST_DISPATCH_BLOCKED")?;
                record_model_pre_dispatch_failure(
                    store,
                    events,
                    &request_id,
                    metadata,
                    error_kind,
                    terminal_at,
                )?;
                return Err(ModelDispatchFailure::Refused(error));
            }
        };
        let effective_deadline_ms = gate.deadline_ms.min(latest_gate.deadline_ms);
        request.deadline_ms = effective_deadline_ms;

        let response = match provider
            .generate(
                &request,
                &ModelCallContext {
                    deadline_ms: effective_deadline_ms,
                },
            )
            .await
        {
            Ok(response) => response,
            Err(error) => {
                let at = clock.now_ms().map_err(ModelDispatchFailure::Clock)?;
                if error.kind.as_str() == AMBIGUOUS_PROVIDER_ERROR_KIND {
                    record_model_ambiguity(
                        store,
                        events,
                        ModelFailureFacts {
                            request_id: &request_id,
                            metadata: ModelEventMetadataV1 {
                                occurred_at: at,
                                ..metadata
                            },
                            error_kind: error.kind.clone(),
                            retryable: false,
                            terminal_at: at,
                            usage: None,
                        },
                    )?;
                    return Err(ModelDispatchFailure::AmbiguousProvider {
                        request_id,
                        error_kind: error.kind,
                    });
                }
                if !(defer_retryable_failure && error.retryable) {
                    record_model_failure(
                        store,
                        events,
                        ModelFailureFacts {
                            request_id: &request_id,
                            metadata: ModelEventMetadataV1 {
                                occurred_at: at,
                                ..metadata
                            },
                            error_kind: error.kind.clone(),
                            retryable: error.retryable,
                            terminal_at: at,
                            usage: None,
                        },
                    )?;
                }
                return Err(ModelDispatchFailure::DefiniteProvider {
                    request_id,
                    error_kind: error.kind,
                    retryable: error.retryable,
                });
            }
        };
        let response =
            match bind_provider_response(&request_id, &selected, &entry.provider_id, response) {
                Ok(response) => response,
                Err(_) => {
                    let at = clock.now_ms().map_err(ModelDispatchFailure::Clock)?;
                    let error_kind = stable_model_error("PROVIDER_PROTOCOL_FAILURE")?;
                    record_model_failure(
                        store,
                        events,
                        ModelFailureFacts {
                            request_id: &request_id,
                            metadata: ModelEventMetadataV1 {
                                occurred_at: at,
                                ..metadata
                            },
                            error_kind: error_kind.clone(),
                            retryable: false,
                            terminal_at: at,
                            usage: None,
                        },
                    )?;
                    return Err(ModelDispatchFailure::TerminalProvider {
                        request_id,
                        error_kind,
                    });
                }
            };
        let usage_is_valid = response.usage.cost_class == price.cost_class()
            && response.usage.input_tokens.get() <= u64::from(effective.max_context_tokens)
            && response.usage.output_tokens.get() <= u64::from(effective_max_output_tokens)
            && (is_structured || response.content.len() <= serea_storage::MAX_MODEL_RESPONSE_BYTES);
        if !usage_is_valid {
            let at = clock.now_ms().map_err(ModelDispatchFailure::Clock)?;
            let error_kind = stable_model_error("PROVIDER_METADATA_INVALID")?;
            record_model_failure(
                store,
                events,
                ModelFailureFacts {
                    request_id: &request_id,
                    metadata: ModelEventMetadataV1 {
                        occurred_at: at,
                        ..metadata
                    },
                    error_kind: error_kind.clone(),
                    retryable: false,
                    terminal_at: at,
                    usage: None,
                },
            )?;
            enforce_task_token_bound(context, call)?;
            return Err(ModelDispatchFailure::TerminalProvider {
                request_id,
                error_kind,
            });
        }
        if response.finish_reason != FinishReason::Stop
            && !(is_structured && response.finish_reason == FinishReason::StructureInvalid)
        {
            let at = clock.now_ms().map_err(ModelDispatchFailure::Clock)?;
            let error_kind = stable_model_error(match response.finish_reason {
                FinishReason::Length => "MODEL_FINISH_LENGTH",
                FinishReason::ContentFilter => "MODEL_CONTENT_FILTER",
                FinishReason::Error => "MODEL_FINISH_ERROR",
                FinishReason::StructureInvalid => "MODEL_STRUCTURE_INVALID",
                FinishReason::Stop => "MODEL_FINISH_STOP",
            })?;
            let usage = ModelFailureUsage {
                input_tokens: response.usage.input_tokens,
                output_tokens: response.usage.output_tokens,
                latency_ms: u64::from(response.latency_ms),
                repair_attempts,
                recorded_at: at,
            };
            record_model_failure(
                store,
                events,
                ModelFailureFacts {
                    request_id: &request_id,
                    metadata: ModelEventMetadataV1 {
                        occurred_at: at,
                        ..metadata
                    },
                    error_kind: error_kind.clone(),
                    retryable: false,
                    terminal_at: at,
                    usage: Some(usage),
                },
            )?;
            enforce_task_token_bound(context, call)?;
            return Err(ModelDispatchFailure::TerminalProvider {
                request_id,
                error_kind,
            });
        }
        let structured = if let ResponseFormat::JsonSchema { schema } = &call.response_format {
            match validate_structured_response(schema, &response.content) {
                Ok(value) => Some(value),
                Err(validation_error) => {
                    let invalid_at = clock.now_ms().map_err(ModelDispatchFailure::Clock)?;
                    let error_kind = stable_model_error("MODEL_OUTPUT_INVALID")?;
                    let actual_cost = calculate_cost_usd_micros(
                        response.usage.input_tokens.get(),
                        response.usage.output_tokens.get(),
                        price.input_rate_microusd_per_million_tokens(),
                        price.output_rate_microusd_per_million_tokens(),
                    )
                    .map_err(|_| {
                        ModelDispatchFailure::Refused(RouterError::ProviderProtocolFailure)
                    })?;
                    let completed = events
                        .draft_model_completed(ModelCompletedEventV1 {
                            metadata: ModelEventMetadataV1 {
                                occurred_at: invalid_at,
                                ..metadata.clone()
                            },
                            finish_reason: response.finish_reason,
                            input_tokens: response.usage.input_tokens.get(),
                            output_tokens: response.usage.output_tokens.get(),
                            cost_class: price.cost_class(),
                            cost_usd_micros: actual_cost.get(),
                            price_revision: price.price_revision().to_owned(),
                            repair_attempts,
                        })
                        .map_err(ModelDispatchFailure::Storage)?;
                    let diagnostic_count = match &validation_error {
                        StructuredValidationError::InvalidOutput(diagnostics) => {
                            u8::try_from(diagnostics.len()).unwrap_or(32)
                        }
                        _ => 0,
                    };
                    let invalid = events
                        .draft_model_output_invalid(ModelOutputInvalidEventV1 {
                            metadata: ModelEventMetadataV1 {
                                occurred_at: invalid_at,
                                ..metadata.clone()
                            },
                            diagnostic_count,
                        })
                        .map_err(ModelDispatchFailure::Storage)?;
                    let usage = ModelFailureUsage {
                        input_tokens: response.usage.input_tokens,
                        output_tokens: response.usage.output_tokens,
                        latency_ms: u64::from(response.latency_ms),
                        repair_attempts,
                        recorded_at: invalid_at,
                    };
                    store
                        .transact(|tx| {
                            tx.fail_model_call_with_usage(
                                &request_id,
                                error_kind.as_str(),
                                invalid_at,
                                usage,
                            )?;
                            tx.append_event(completed.event, completed.retention_at)?;
                            tx.append_event(invalid.event, invalid.retention_at)?;
                            Ok(())
                        })
                        .map_err(ModelDispatchFailure::Storage)?;
                    enforce_task_token_bound(context, call)?;
                    return Err(ModelDispatchFailure::StructuredOutputInvalid {
                        request_id,
                        raw_response: response.content.clone(),
                        validation_error,
                    });
                }
            }
        } else {
            None
        };
        let actual_cost = calculate_cost_usd_micros(
            response.usage.input_tokens.get(),
            response.usage.output_tokens.get(),
            price.input_rate_microusd_per_million_tokens(),
            price.output_rate_microusd_per_million_tokens(),
        )
        .map_err(|_| ModelDispatchFailure::Refused(RouterError::ProviderProtocolFailure))?;
        let completed_at = clock.now_ms().map_err(ModelDispatchFailure::Clock)?;
        let completed = events
            .draft_model_completed(ModelCompletedEventV1 {
                metadata: ModelEventMetadataV1 {
                    occurred_at: completed_at,
                    ..metadata.clone()
                },
                finish_reason: response.finish_reason,
                input_tokens: response.usage.input_tokens.get(),
                output_tokens: response.usage.output_tokens.get(),
                cost_class: price.cost_class(),
                cost_usd_micros: actual_cost.get(),
                price_revision: price.price_revision().to_owned(),
                repair_attempts,
            })
            .map_err(ModelDispatchFailure::Storage)?;
        let repaired = if relation_kind == ModelAttemptRelationKind::Repair {
            Some(
                events
                    .draft_model_repaired(ModelRepairedEventV1 {
                        metadata: ModelEventMetadataV1 {
                            occurred_at: completed_at,
                            relation: ModelEventRelationV1::Repair,
                            ..metadata.clone()
                        },
                        repair_attempts,
                    })
                    .map_err(ModelDispatchFailure::Storage)?,
            )
        } else {
            None
        };
        let response_json = if let Some(structured) = &structured {
            serde_json::json!({
                "content": response.content.clone(),
                "structured": structured,
            })
        } else {
            serde_json::json!({"content": response.content.clone()})
        };
        let serialized = serde_json::to_string(&response_json)
            .map_err(|_| ModelDispatchFailure::Refused(RouterError::ProviderProtocolFailure))?;
        let accepted_json = canonicalize(&serialized)
            .map_err(|_| ModelDispatchFailure::Refused(RouterError::ProviderProtocolFailure))?;
        store
            .transact(|tx| {
                tx.complete_model_call(
                    &request_id,
                    ModelCallCompletion {
                        input_tokens: response.usage.input_tokens,
                        output_tokens: response.usage.output_tokens,
                        latency_ms: u64::from(response.latency_ms),
                        repair_attempts,
                        finish_reason: response.finish_reason,
                        recorded_at: completed_at,
                        accepted_response: ModelResponseStorage {
                            canonical_json: accepted_json,
                            data_class: call.data_class,
                        },
                    },
                )?;
                tx.append_event(completed.event, completed.retention_at)?;
                if let Some(repaired) = repaired {
                    tx.append_event(repaired.event, repaired.retention_at)?;
                }
                Ok(())
            })
            .map_err(ModelDispatchFailure::Storage)?;
        enforce_task_token_bound(context, call)?;
        Ok(ModelResponse {
            structured,
            repair_attempts: u32::from(repair_attempts),
            ..response
        })
    }
}

fn host_call_failure(failure: ModelDispatchFailure) -> ModelRouterCallFailureV1 {
    match failure {
        ModelDispatchFailure::Refused(error) => ModelRouterCallFailureV1::Refused(error),
        ModelDispatchFailure::Storage(error) => ModelRouterCallFailureV1::Storage(error),
        ModelDispatchFailure::Clock(error) | ModelDispatchFailure::Protocol(error) => {
            ModelRouterCallFailureV1::Protocol(error)
        }
        ModelDispatchFailure::NoEligibleModel => ModelRouterCallFailureV1::NoEligibleModel,
        ModelDispatchFailure::DefiniteProvider {
            request_id,
            error_kind,
            retryable,
        } => ModelRouterCallFailureV1::DefiniteProvider {
            request_id,
            error_kind,
            retryable,
        },
        ModelDispatchFailure::AmbiguousProvider {
            request_id,
            error_kind,
        } => ModelRouterCallFailureV1::AmbiguousProvider {
            request_id,
            error_kind,
        },
        ModelDispatchFailure::TerminalProvider {
            request_id,
            error_kind,
        } => ModelRouterCallFailureV1::TerminalProvider {
            request_id,
            error_kind,
        },
        ModelDispatchFailure::StructuredOutputInvalid {
            request_id,
            raw_response: _,
            validation_error,
        } => ModelRouterCallFailureV1::StructuredValidation {
            request_id,
            error: validation_error,
        },
        ModelDispatchFailure::TokenBudgetExceeded { limit, observed } => {
            ModelRouterCallFailureV1::TaskBudgetExceeded {
                bound: ModelBoundKindV1::TaskTotalTokens,
                limit,
                observed,
            }
        }
        ModelDispatchFailure::ModelCallBudgetExceeded { limit, observed } => {
            ModelRouterCallFailureV1::ModelCallBudgetExceeded { limit, observed }
        }
        ModelDispatchFailure::BoundExceeded {
            bound,
            limit,
            observed,
        } => ModelRouterCallFailureV1::BoundExceeded {
            bound,
            limit,
            observed,
        },
    }
}

fn event_relation(relation: ModelAttemptRelationKind) -> ModelEventRelationV1 {
    match relation {
        ModelAttemptRelationKind::None => ModelEventRelationV1::Normal,
        ModelAttemptRelationKind::Fallback => ModelEventRelationV1::Fallback,
        ModelAttemptRelationKind::Repair => ModelEventRelationV1::Repair,
    }
}

#[allow(dead_code)] // Used by the P4E structured repair ladder.
fn build_repair_call(
    original: &PreparedModelCallV1,
    schema: &serde_json::Value,
    raw_response: &str,
    validation_error: &StructuredValidationError,
) -> Result<PreparedModelCallV1, RouterError> {
    let diagnostics = match validation_error {
        StructuredValidationError::InvalidOutput(diagnostics) => {
            serde_json::to_value(diagnostics).map_err(|_| RouterError::InvalidJsonSchema)?
        }
        error => serde_json::json!({
            "kind": match error {
                StructuredValidationError::InvalidSchema => "INVALID_SCHEMA",
                StructuredValidationError::ResponseTooLarge => "RESPONSE_TOO_LARGE",
                StructuredValidationError::MalformedJson => "MALFORMED_JSON",
                StructuredValidationError::DuplicateKey => "DUPLICATE_KEY",
                StructuredValidationError::TooDeep => "JSON_TOO_DEEP",
                StructuredValidationError::InvalidOutput(_) => "INVALID_OUTPUT",
            }
        }),
    };
    let prompt = serde_json::to_string(&serde_json::json!({
        "schema": schema,
        "invalid_response": raw_response,
        "validation_errors": diagnostics,
    }))
    .map_err(|_| RouterError::InvalidJsonSchema)?;
    PreparedModelCallV1::from_host(PreparedModelCallDraftV1 {
        task_id: original.task_id.clone(),
        purpose: ModelPurpose::StructuredRepair,
        messages: vec![ModelMessage {
            role: serea_protocol::MessageRole::new("user")
                .map_err(|_| RouterError::IllegalPurposeFormat)?,
            content: prompt,
        }],
        system: None,
        response_format: ResponseFormat::JsonSchema {
            schema: schema.clone(),
        },
        tools: Vec::new(),
        max_output_tokens: original.max_output_tokens,
        temperature: original.temperature,
        deadline_ms: original.deadline_ms,
        data_class: original.data_class,
        requirements: ModelRoutingRequirementsV1 {
            vision_required: false,
            tools_required: false,
            min_context_tokens: 1,
            min_output_tokens: 1,
            structured_requirement: StructuredRequirementV1::Strict,
        },
        egress: original.egress,
        host_max_output_tokens: DEFAULT_MAX_OUTPUT_TOKENS_PER_CALL,
    })
}

#[allow(dead_code)] // Used by dispatch error paths; exercised by P4D scripted failures.
fn stable_model_error(value: &str) -> Result<ModelErrorCode, ModelDispatchFailure> {
    ModelErrorCode::new(value).map_err(ModelDispatchFailure::Protocol)
}

fn enforce_task_token_bound(
    context: &ModelDispatchContext<'_>,
    call: &PreparedModelCallV1,
) -> Result<(), ModelDispatchFailure> {
    let Some(task_id) = call.task_id.as_ref() else {
        return Ok(());
    };
    let observed = context
        .store
        .task_model_token_usage(task_id)
        .map_err(ModelDispatchFailure::Storage)?
        .get();
    if observed < MAX_TASK_TOTAL_TOKENS {
        return Ok(());
    }
    let occurred_at = context
        .clock
        .now_ms()
        .map_err(ModelDispatchFailure::Clock)?;
    let exceeded = context
        .events
        .draft_model_bound_exceeded(ModelBoundExceededEventV1 {
            task_id: Some(task_id.clone()),
            data_class: call.data_class,
            occurred_at,
            bound: ModelBoundKindV1::TaskTotalTokens,
            limit: MAX_TASK_TOTAL_TOKENS,
            observed,
        })
        .map_err(ModelDispatchFailure::Storage)?;
    context
        .store
        .transact(|tx| {
            tx.append_event(exceeded.event, exceeded.retention_at)?;
            Ok(())
        })
        .map_err(ModelDispatchFailure::Storage)?;
    Err(ModelDispatchFailure::TokenBudgetExceeded {
        limit: MAX_TASK_TOTAL_TOKENS,
        observed,
    })
}

fn record_dispatch_budget_refusal(
    context: &ModelDispatchContext<'_>,
    call: &PreparedModelCallV1,
    error: &StoreError,
    facts: DispatchBudgetFacts<'_>,
) -> Result<Option<ModelDispatchFailure>, ModelDispatchFailure> {
    let (task_id, limit, observed) = match error {
        StoreError::ModelCallBudgetExceeded => {
            let Some(task_id) = call.task_id.as_ref() else {
                return Ok(None);
            };
            (
                task_id.clone(),
                context
                    .store
                    .task_model_call_limit(task_id)
                    .map_err(ModelDispatchFailure::Storage)?,
                context
                    .store
                    .task_model_call_count(task_id)
                    .map_err(ModelDispatchFailure::Storage)?,
            )
        }
        StoreError::ModelTurnBudgetExceeded => {
            let Some(task_id) = call.task_id.as_ref() else {
                return Ok(None);
            };
            (
                task_id.clone(),
                MAX_MODEL_TURNS_PER_TASK,
                context
                    .store
                    .task_model_turn_count(task_id)
                    .map_err(ModelDispatchFailure::Storage)?,
            )
        }
        StoreError::DailySpendExceeded => {
            let reservation = calculate_reservation_usd_micros(
                facts.max_context_tokens,
                facts.max_output_tokens,
                facts.price.input_rate_microusd_per_million_tokens(),
                facts.price.output_rate_microusd_per_million_tokens(),
            )
            .map_err(|_| ModelDispatchFailure::Refused(RouterError::ProviderProtocolFailure))?;
            let day = UtcAccountingDay::from_epoch_millis(facts.dispatch_at);
            let occupied = context
                .store
                .utc_day_spend_occupancy(day)
                .map_err(ModelDispatchFailure::Storage)?;
            let observed = occupied.get().checked_add(reservation.get()).ok_or(
                ModelDispatchFailure::Refused(RouterError::ProviderProtocolFailure),
            )?;
            let limit = facts.daily_limit.get();
            let occurred_at = context
                .clock
                .now_ms()
                .map_err(ModelDispatchFailure::Clock)?;
            let event = context
                .events
                .draft_model_bound_exceeded(ModelBoundExceededEventV1 {
                    task_id: call.task_id.clone(),
                    data_class: call.data_class,
                    occurred_at,
                    bound: ModelBoundKindV1::DailySpendUsd,
                    limit,
                    observed,
                })
                .map_err(ModelDispatchFailure::Storage)?;
            context
                .store
                .transact(|tx| {
                    tx.append_event(event.event, event.retention_at)?;
                    Ok(())
                })
                .map_err(ModelDispatchFailure::Storage)?;
            return Ok(Some(ModelDispatchFailure::BoundExceeded {
                bound: ModelBoundKindV1::DailySpendUsd,
                limit,
                observed,
            }));
        }
        _ => return Ok(None),
    };
    let occurred_at = context
        .clock
        .now_ms()
        .map_err(ModelDispatchFailure::Clock)?;
    let event = context
        .events
        .draft_model_budget_exhausted(ModelBudgetExhaustedEventV1 {
            task_id,
            data_class: call.data_class,
            occurred_at,
            limit,
            observed,
        })
        .map_err(ModelDispatchFailure::Storage)?;
    context
        .store
        .transact(|tx| {
            tx.append_event(event.event, event.retention_at)?;
            Ok(())
        })
        .map_err(ModelDispatchFailure::Storage)?;
    Ok(Some(ModelDispatchFailure::ModelCallBudgetExceeded {
        limit,
        observed,
    }))
}

fn persist_pending_primary_failure(
    context: &ModelDispatchContext<'_>,
    pending: &PendingFallback,
    terminal_at: serea_protocol::EpochMillis,
) -> Result<(), ModelDispatchFailure> {
    record_model_failure(
        context.store,
        context.events,
        ModelFailureFacts {
            request_id: &pending.request_id,
            metadata: ModelEventMetadataV1 {
                occurred_at: terminal_at,
                ..pending.metadata.clone()
            },
            error_kind: pending.error_kind.clone(),
            retryable: true,
            terminal_at,
            usage: None,
        },
    )
}

#[allow(dead_code)] // Used by dispatch error paths; exercised by P4D scripted failures.
fn record_model_failure(
    store: &Store,
    events: &EventBus,
    facts: ModelFailureFacts<'_>,
) -> Result<(), ModelDispatchFailure> {
    let failed = events
        .draft_model_failed(ModelFailedEventV1 {
            metadata: facts.metadata,
            error_kind: facts.error_kind.clone(),
            retryable: facts.retryable,
        })
        .map_err(ModelDispatchFailure::Storage)?;
    store
        .transact(|tx| {
            match facts.usage {
                Some(usage) => {
                    tx.fail_model_call_with_usage(
                        facts.request_id,
                        facts.error_kind.as_str(),
                        facts.terminal_at,
                        usage,
                    )?;
                }
                None => {
                    tx.fail_model_call(
                        facts.request_id,
                        facts.error_kind.as_str(),
                        facts.terminal_at,
                    )?;
                }
            }
            tx.append_event(failed.event, failed.retention_at)?;
            Ok(())
        })
        .map_err(ModelDispatchFailure::Storage)
}

fn record_model_pre_dispatch_failure(
    store: &Store,
    events: &EventBus,
    request_id: &serea_protocol::RequestId,
    metadata: ModelEventMetadataV1,
    error_kind: ModelErrorCode,
    terminal_at: serea_protocol::EpochMillis,
) -> Result<(), ModelDispatchFailure> {
    let failed = events
        .draft_model_failed(ModelFailedEventV1 {
            metadata,
            error_kind: error_kind.clone(),
            retryable: false,
        })
        .map_err(ModelDispatchFailure::Storage)?;
    store
        .transact(|tx| {
            tx.fail_model_call_before_dispatch(request_id, error_kind.as_str(), terminal_at)?;
            tx.append_event(failed.event, failed.retention_at)?;
            Ok(())
        })
        .map_err(ModelDispatchFailure::Storage)
}

#[allow(dead_code)] // Used for typed adapter outcomes with uncertain processing.
fn record_model_ambiguity(
    store: &Store,
    events: &EventBus,
    mut facts: ModelFailureFacts<'_>,
) -> Result<(), ModelDispatchFailure> {
    facts.retryable = false;
    facts.usage = None;
    let failed = events
        .draft_model_failed(ModelFailedEventV1 {
            metadata: facts.metadata,
            error_kind: facts.error_kind.clone(),
            retryable: false,
        })
        .map_err(ModelDispatchFailure::Storage)?;
    store
        .transact(|tx| {
            tx.mark_model_call_ambiguous(
                facts.request_id,
                facts.error_kind.as_str(),
                facts.terminal_at,
            )?;
            tx.append_event(failed.event, failed.retention_at)?;
            Ok(())
        })
        .map_err(ModelDispatchFailure::Storage)
}

/// Conservatively marks every unresolved dispatch as ambiguous during startup.
///
/// Call this before serving new model operations. Each attempt's state change
/// and content-free audit event commit in one Store transaction. Repeated or
/// concurrent recovery passes do not append duplicate events.
pub fn recover_unresolved_model_calls(
    store: &Store,
    events: &EventBus,
    clock: &dyn Clock,
) -> Result<usize, ModelRecoveryError> {
    let attempts = store
        .list_unfinished_model_call_attempts()
        .map_err(ModelRecoveryError::Storage)?;
    let error_kind =
        ModelErrorCode::new("AMBIGUOUS_DISPATCH").map_err(ModelRecoveryError::Protocol)?;
    let mut recovered = 0;
    for attempt in attempts {
        let occurred_at = clock.now_ms().map_err(ModelRecoveryError::Protocol)?;
        let relation = match attempt.relation_kind {
            ModelAttemptRelationKind::None => ModelEventRelationV1::Normal,
            ModelAttemptRelationKind::Fallback => ModelEventRelationV1::Fallback,
            ModelAttemptRelationKind::Repair => ModelEventRelationV1::Repair,
        };
        let failed = events
            .draft_model_failed(ModelFailedEventV1 {
                metadata: ModelEventMetadataV1 {
                    request_id: attempt.request_id.clone(),
                    model_id: attempt.model_id.clone(),
                    provider_id: attempt.provider_id.clone(),
                    task_id: attempt.task_id.clone(),
                    purpose: attempt.purpose,
                    relation,
                    data_class: attempt.data_class,
                    occurred_at,
                },
                error_kind: error_kind.clone(),
                retryable: false,
            })
            .map_err(ModelRecoveryError::Storage)?;
        let result = store.transact(|tx| {
            tx.mark_model_call_ambiguous(&attempt.request_id, error_kind.as_str(), occurred_at)?;
            tx.append_event(failed.event, failed.retention_at)?;
            Ok(())
        });
        match result {
            Ok(()) => recovered += 1,
            // Another recovery worker or a concurrent terminal operation won.
            // Its transaction owns the state/event pair.
            Err(StoreError::InvalidModelCallTransition) => {}
            Err(error) => return Err(ModelRecoveryError::Storage(error)),
        }
    }
    Ok(recovered)
}

/// Rebuilds a completed CHAT/TEXT result after the original caller lost it.
///
/// This reads only a durable accepted response and trusted usage/identity facts;
/// it never calls a provider. Missing, non-completed, or content-pruned requests
/// return `Ok(None)`. A completed CHAT/TEXT row missing required durable facts
/// is treated as storage corruption.
pub fn recover_completed_chat_text_response(
    store: &Store,
    request_id: &serea_protocol::RequestId,
) -> Result<Option<ModelResponse>, ModelRecoveryError> {
    let Some(attempt) = store
        .get_model_call_attempt(request_id)
        .map_err(ModelRecoveryError::Storage)?
    else {
        return Ok(None);
    };
    if attempt.state != ModelAttemptState::Completed || attempt.purpose != ModelPurpose::Chat {
        return Ok(None);
    }
    let Some(response_bytes) = store
        .get_model_call_response(request_id)
        .map_err(ModelRecoveryError::Storage)?
    else {
        return Ok(None);
    };
    let usage = store
        .model_usage_for_request(request_id)
        .map_err(ModelRecoveryError::Storage)?
        .ok_or(ModelRecoveryError::Storage(StoreError::CorruptRow))?;
    if usage.cost_class != attempt.price.cost_class() {
        return Err(ModelRecoveryError::Storage(StoreError::CorruptRow));
    }
    let document: serde_json::Value = serde_json::from_slice(&response_bytes)
        .map_err(|_| ModelRecoveryError::Storage(StoreError::CorruptRow))?;
    let object = document
        .as_object()
        .filter(|object| object.len() == 1)
        .ok_or(ModelRecoveryError::Storage(StoreError::CorruptRow))?;
    let content = object
        .get("content")
        .and_then(serde_json::Value::as_str)
        .ok_or(ModelRecoveryError::Storage(StoreError::CorruptRow))?;
    let latency_ms = u32::try_from(usage.latency_ms)
        .map_err(|_| ModelRecoveryError::Storage(StoreError::CorruptRow))?;
    Ok(Some(ModelResponse {
        request_id: attempt.request_id,
        model_id: attempt.model_id,
        provider_id: attempt.provider_id,
        content: content.to_owned(),
        structured: None,
        finish_reason: FinishReason::Stop,
        usage: serea_protocol::ModelUsage {
            input_tokens: usage.input_tokens,
            output_tokens: usage.output_tokens,
            cost_class: usage.cost_class,
        },
        latency_ms,
        repair_attempts: u32::from(usage.repair_attempts),
    }))
}

/// Rebuilds a host-validated structured result after caller loss.
///
/// Recovery uses only the durable accepted response and attempt/accounting
/// facts. It never calls a provider or repeats schema validation because the
/// stored `COMPLETED` response was accepted by the host before commit.
pub fn recover_completed_structured_response(
    store: &Store,
    request_id: &serea_protocol::RequestId,
) -> Result<Option<ModelResponse>, ModelRecoveryError> {
    let Some(attempt) = store
        .get_model_call_attempt(request_id)
        .map_err(ModelRecoveryError::Storage)?
    else {
        return Ok(None);
    };
    if attempt.state != ModelAttemptState::Completed || attempt.purpose == ModelPurpose::Chat {
        return Ok(None);
    }
    let Some(response_bytes) = store
        .get_model_call_response(request_id)
        .map_err(ModelRecoveryError::Storage)?
    else {
        return Ok(None);
    };
    let usage = store
        .model_usage_for_request(request_id)
        .map_err(ModelRecoveryError::Storage)?
        .ok_or(ModelRecoveryError::Storage(StoreError::CorruptRow))?;
    if usage.cost_class != attempt.price.cost_class() || usage.repair_attempts > 2 {
        return Err(ModelRecoveryError::Storage(StoreError::CorruptRow));
    }
    let document: serde_json::Value = serde_json::from_slice(&response_bytes)
        .map_err(|_| ModelRecoveryError::Storage(StoreError::CorruptRow))?;
    let object = document
        .as_object()
        .filter(|object| object.len() == 2)
        .ok_or(ModelRecoveryError::Storage(StoreError::CorruptRow))?;
    let content = object
        .get("content")
        .and_then(serde_json::Value::as_str)
        .ok_or(ModelRecoveryError::Storage(StoreError::CorruptRow))?;
    let structured = object
        .get("structured")
        .cloned()
        .ok_or(ModelRecoveryError::Storage(StoreError::CorruptRow))?;
    let latency_ms = u32::try_from(usage.latency_ms)
        .map_err(|_| ModelRecoveryError::Storage(StoreError::CorruptRow))?;
    Ok(Some(ModelResponse {
        request_id: attempt.request_id,
        model_id: attempt.model_id,
        provider_id: attempt.provider_id,
        content: content.to_owned(),
        structured: Some(structured),
        finish_reason: FinishReason::Stop,
        usage: serea_protocol::ModelUsage {
            input_tokens: usage.input_tokens,
            output_tokens: usage.output_tokens,
            cost_class: usage.cost_class,
        },
        latency_ms,
        repair_attempts: u32::from(usage.repair_attempts),
    }))
}

/// Explicit deterministic preference chain for a purpose.
pub fn preference_chain(purpose: ModelPurpose) -> &'static [&'static str] {
    match purpose {
        ModelPurpose::Chat
        | ModelPurpose::Planning
        | ModelPurpose::Extraction
        | ModelPurpose::Analysis
        | ModelPurpose::Proactive => &["nemotron-3-nano-30b", "gpt-oss-20b"],
        ModelPurpose::StructuredRepair => &["gpt-oss-20b"],
    }
}

/// Pure vision candidate rule for test and future typed image input. Production
/// routing refuses vision requirements until an image transport exists.
pub fn vision_candidate_chain() -> &'static [&'static str] {
    &["gemma-4-31b"]
}

fn validate_purpose_format(
    purpose: ModelPurpose,
    format: &ResponseFormat,
    requirements: ModelRoutingRequirementsV1,
) -> Result<(), RouterError> {
    let allowed = matches!(
        (purpose, format),
        (ModelPurpose::Chat, ResponseFormat::Text)
    ) || matches!(
        (purpose, format),
        (
            ModelPurpose::Planning
                | ModelPurpose::Extraction
                | ModelPurpose::Analysis
                | ModelPurpose::Proactive
                | ModelPurpose::StructuredRepair,
            ResponseFormat::JsonSchema { .. }
        )
    );
    let strength = match purpose {
        ModelPurpose::Chat | ModelPurpose::Analysis => StructuredRequirementV1::Any,
        ModelPurpose::Planning
        | ModelPurpose::Extraction
        | ModelPurpose::Proactive
        | ModelPurpose::StructuredRepair => StructuredRequirementV1::Strict,
    };
    if allowed && requirements.structured_requirement == strength {
        Ok(())
    } else {
        Err(RouterError::IllegalPurposeFormat)
    }
}

fn deployment_permitted(
    data_class: DataClass,
    deployment: ModelDeploymentClass,
    egress: ModelEgressPolicySnapshotV1,
) -> bool {
    (match data_class {
        DataClass::Public => true,
        // Personal cloud egress is permitted only by the trusted host fact.
        DataClass::Personal => {
            deployment == ModelDeploymentClass::Local || egress.private_cloud_egress_allowed
        }
        DataClass::Private | DataClass::Secret | DataClass::Credential => false,
    }) && matches!(
        deployment,
        ModelDeploymentClass::Cloud | ModelDeploymentClass::Local
    )
}

fn capabilities_satisfy(
    c: ModelCapabilities,
    r: ModelRoutingRequirementsV1,
    structured: bool,
) -> bool {
    if r.vision_required && !c.vision {
        return false;
    }
    if r.tools_required && !c.tools {
        return false;
    }
    if c.max_context_tokens < r.min_context_tokens || c.max_output_tokens < r.min_output_tokens {
        return false;
    }
    if structured {
        if !c.structured_output {
            return false;
        }
        match r.structured_requirement {
            StructuredRequirementV1::Any => c.json_schema_mode != JsonSchemaMode::Unsupported,
            StructuredRequirementV1::Strict => c.json_schema_mode == JsonSchemaMode::Strict,
        }
    } else {
        true
    }
}

fn intersect(host: ModelCapabilities, provider: ModelCapabilities) -> ModelCapabilities {
    let mode = if schema_mode_strength(host.json_schema_mode)
        < schema_mode_strength(provider.json_schema_mode)
    {
        host.json_schema_mode
    } else {
        provider.json_schema_mode
    };
    ModelCapabilities {
        vision: host.vision && provider.vision,
        tools: host.tools && provider.tools,
        structured_output: host.structured_output && provider.structured_output,
        json_schema_mode: mode,
        thinking: host.thinking && provider.thinking,
        long_context: host.long_context && provider.long_context,
        fast: host.fast && provider.fast,
        code_specialist: host.code_specialist && provider.code_specialist,
        max_context_tokens: host.max_context_tokens.min(provider.max_context_tokens),
        max_output_tokens: host.max_output_tokens.min(provider.max_output_tokens),
        supports_streaming: host.supports_streaming && provider.supports_streaming,
        supports_seeds: host.supports_seeds && provider.supports_seeds,
    }
}

fn schema_mode_strength(mode: JsonSchemaMode) -> u8 {
    match mode {
        JsonSchemaMode::Unsupported => 0,
        JsonSchemaMode::BestEffort => 1,
        JsonSchemaMode::Strict => 2,
    }
}

fn known_model_id(id: &str) -> bool {
    matches!(id, "nemotron-3-nano-30b" | "gpt-oss-20b" | "gemma-4-31b")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::VecDeque;
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use std::sync::mpsc::{self, Receiver, SyncSender};
    use std::task::{Context, Poll, Waker};
    use std::time::Duration;

    use async_trait::async_trait;
    use serea_event_bus::{EventBus, ReplayItem};
    use serea_protocol::provider::ModelCallContext;
    use serea_protocol::{
        ActorId, ActorKind, AssistantTask, AttemptBudget, Clock, CostClass, DataClass, EpochMillis,
        EventId, EventKind, FinishReason, ModelError, ModelErrorCode, ModelMessage, ModelResponse,
        ModelUsage, ProtocolError, ProviderId, ResponseFormat, RiskClass, TaskKind, TaskOrigin,
        TaskOriginKind, TaskState, TaskTitle, Timestamp, TimestampMs, TokenCount, UlidSource,
        UlidValue,
    };
    use serea_storage::{
        AuditOperation, DurableTransition, EventDraft, EventParticipant, JournalKind,
        JournalRecord, JournalRecords, ModelAttemptRelationKind, ModelAttemptState,
        ModelCallAttemptDraft, ModelDeploymentClass as StorageDeploymentClass, ModelFailureUsage,
        ModelPriceSnapshot, Store, StoreError, TaskAuditParticipant, TransitionContext, UsdMicros,
    };

    struct FixedClock;

    impl Clock for FixedClock {
        fn now_ms(&self) -> Result<EpochMillis, ProtocolError> {
            EpochMillis::new(1_767_225_600_000)
        }
    }

    struct OpenDispatchGate;

    impl ModelDispatchGateSource for OpenDispatchGate {
        fn snapshot(
            &self,
            _task_id: Option<&TaskId>,
            _data_class: DataClass,
        ) -> Result<ModelDispatchGateSnapshotV1, ModelDispatchGateSourceError> {
            Ok(ModelDispatchGateSnapshotV1::from_host(
                false,
                ModelEgressPolicySnapshotV1::from_host(true),
                997,
            ))
        }
    }

    static OPEN_DISPATCH_GATE: OpenDispatchGate = OpenDispatchGate;

    struct CancelledDispatchGate;

    impl ModelDispatchGateSource for CancelledDispatchGate {
        fn snapshot(
            &self,
            _task_id: Option<&TaskId>,
            _data_class: DataClass,
        ) -> Result<ModelDispatchGateSnapshotV1, ModelDispatchGateSourceError> {
            Ok(ModelDispatchGateSnapshotV1::from_host(
                true,
                ModelEgressPolicySnapshotV1::from_host(true),
                1000,
            ))
        }
    }

    static CANCELLED_DISPATCH_GATE: CancelledDispatchGate = CancelledDispatchGate;

    struct StoreTaskGate {
        store: Arc<Store>,
        cancel_on_snapshot: Option<usize>,
        snapshots: AtomicUsize,
        suffix: &'static str,
    }

    impl StoreTaskGate {
        fn cancel_task(&self, task_id: &TaskId) -> Result<(), ModelDispatchGateSourceError> {
            let actor = ActorId::new("host-boundary-test")
                .map_err(|_| ModelDispatchGateSourceError::Unavailable)?;
            let version = serea_protocol::SemVer::new("1.0.0")
                .map_err(|_| ModelDispatchGateSourceError::Unavailable)?;
            let cause = EventId::new(format!("evt_000000000000000000000000{}", self.suffix))
                .map_err(|_| ModelDispatchGateSourceError::Unavailable)?;
            let transition = TransitionContext {
                actor_kind: ActorKind::Host,
                actor_id: &actor,
                actor_version: &version,
                causation_id: Some(&cause),
            };
            self.store
                .transact_with_participants(&TestTaskAudit, &TestTaskEvents, |tx| {
                    tx.cancel_task(
                        task_id,
                        TaskOriginKind::new("HOST_TEST").map_err(|_| StoreError::CorruptRow)?,
                        EpochMillis::new(1_767_225_600_000).map_err(|_| StoreError::CorruptRow)?,
                        &transition,
                    )
                    .map(|_| ())
                })
                .map_err(|_| ModelDispatchGateSourceError::Unavailable)
        }
    }

    impl ModelDispatchGateSource for StoreTaskGate {
        fn snapshot(
            &self,
            task_id: Option<&TaskId>,
            _data_class: DataClass,
        ) -> Result<ModelDispatchGateSnapshotV1, ModelDispatchGateSourceError> {
            let task_id = task_id.ok_or(ModelDispatchGateSourceError::Unavailable)?;
            let read = self.snapshots.fetch_add(1, Ordering::SeqCst);
            if self.cancel_on_snapshot == Some(read) {
                self.cancel_task(task_id)?;
            }
            let task = self
                .store
                .load_task(task_id)
                .map_err(|_| ModelDispatchGateSourceError::Unavailable)?;
            Ok(ModelDispatchGateSnapshotV1::from_host(
                task.task.state == TaskState::Cancelled,
                ModelEgressPolicySnapshotV1::from_host(true),
                1000,
            ))
        }
    }

    struct TestTaskAudit;

    impl TaskAuditParticipant for TestTaskAudit {
        fn records(&self, facts: &DurableTransition) -> Result<JournalRecords, StoreError> {
            let record = |kind| JournalRecord {
                kind,
                state_from: facts.task_from().map(|state| state.wire_name().to_owned()),
                state_to: Some(facts.task_to().wire_name().to_owned()),
                reason: facts.reason().cloned(),
                payload_json: b"{}".to_vec(),
            };
            match facts.operation() {
                AuditOperation::TaskInserted => Ok(vec![record(JournalKind::TaskInserted)]),
                AuditOperation::Cancelled => Ok(vec![
                    record(JournalKind::TaskCancelRequested),
                    record(JournalKind::TaskStateChanged),
                    record(JournalKind::TaskTerminal),
                ]),
                _ => Err(StoreError::AuditRejected),
            }
        }
    }

    struct TestTaskEvents;

    impl EventParticipant for TestTaskEvents {
        fn events(&self, _facts: &DurableTransition) -> Result<Vec<EventDraft>, StoreError> {
            Ok(Vec::new())
        }
    }

    fn insert_budget_task(store: &Store, task_id: TaskId, suffix: &str) -> bool {
        let actor = ActorId::new("budget-test").unwrap_or_else(|_| unreachable!());
        let version = serea_protocol::SemVer::new("1.0.0").unwrap_or_else(|_| unreachable!());
        let cause = EventId::new(format!("evt_000000000000000000000000{suffix}"))
            .unwrap_or_else(|_| unreachable!());
        let transition = TransitionContext {
            actor_kind: ActorKind::Host,
            actor_id: &actor,
            actor_version: &version,
            causation_id: Some(&cause),
        };
        let created =
            Timestamp::from_epoch_millis(EpochMillis::new(1).unwrap_or_else(|_| unreachable!()));
        let task = AssistantTask {
            task_id,
            kind: TaskKind::UserRequest,
            title: TaskTitle::new("model budget fixture").unwrap_or_else(|_| unreachable!()),
            state: TaskState::Received,
            origin: TaskOrigin {
                kind: TaskOriginKind::new("USER_MESSAGE").unwrap_or_else(|_| unreachable!()),
                device_id: None,
                message_id: None,
                extensions: Default::default(),
            },
            data_class: DataClass::Public,
            policy_class: RiskClass::Communication,
            created_at: created.clone(),
            updated_at: created,
            deadline_at: Some(Timestamp::from_epoch_millis(
                EpochMillis::new(1000).unwrap_or_else(|_| unreachable!()),
            )),
            attempt_budget: AttemptBudget {
                max_model_calls: 12,
                max_tool_calls: 0,
                max_attempts_per_step: 0,
                extensions: Default::default(),
            },
            steps: Vec::new(),
            blocked_reason: None,
            result_summary: None,
            cancelled_at: None,
            cancelled_by: None,
            failure_reason: None,
            extensions: Default::default(),
        };
        store
            .transact_with_participants(&TestTaskAudit, &TestTaskEvents, |tx| {
                tx.insert_task(&task, &transition).map(|_| ())
            })
            .is_ok()
    }

    fn seed_terminal_task_model_calls(store: &Store, task_id: &TaskId, count: u8) {
        for sequence in 1..=count {
            let request_id = serea_protocol::RequestId::new(format!(
                "req_000000000000000000000000{sequence:02}"
            ))
            .unwrap_or_else(|_| unreachable!());
            store
                .reserve_model_call(
                    ModelCallAttemptDraft {
                        request_id: request_id.clone(),
                        task_id: Some(task_id.clone()),
                        purpose: ModelPurpose::Chat,
                        model_id: ModelId::new("nemotron-3-nano-30b")
                            .unwrap_or_else(|_| unreachable!()),
                        provider_id: ProviderId::new("provider").unwrap_or_else(|_| unreachable!()),
                        deployment_class: StorageDeploymentClass::Local,
                        data_class: DataClass::Public,
                        relation_kind: ModelAttemptRelationKind::None,
                        parent_request_id: None,
                        fallback_from_model_id: None,
                        price: ModelPriceSnapshot::new(
                            CostClass::Paid,
                            "seed-price",
                            1_000_000,
                            1_000_000,
                        ),
                        max_context_tokens: 1000,
                        effective_max_output_tokens: 16,
                        dispatch_intent_at: FixedClock.now_ms().unwrap_or_else(|_| unreachable!()),
                    },
                    UsdMicros::new(10_000_000).unwrap_or_else(|_| unreachable!()),
                )
                .unwrap_or_else(|_| unreachable!());
            store
                .fail_model_call(
                    &request_id,
                    "SCRIPTED_FAILURE",
                    FixedClock.now_ms().unwrap_or_else(|_| unreachable!()),
                )
                .unwrap_or_else(|_| unreachable!());
        }
    }

    fn prepared_chat_for_task(task_id: TaskId) -> PreparedModelCallV1 {
        PreparedModelCallV1::from_host(PreparedModelCallDraftV1 {
            task_id: Some(task_id),
            purpose: ModelPurpose::Chat,
            messages: vec![ModelMessage {
                role: serea_protocol::MessageRole::new("user").unwrap_or_else(|_| unreachable!()),
                content: "host prepared task input".into(),
            }],
            system: None,
            response_format: ResponseFormat::Text,
            tools: Vec::new(),
            max_output_tokens: 16,
            temperature: 0.0,
            deadline_ms: 1000,
            data_class: DataClass::Public,
            requirements: ModelRoutingRequirementsV1 {
                vision_required: false,
                tools_required: false,
                min_context_tokens: 1,
                min_output_tokens: 1,
                structured_requirement: StructuredRequirementV1::Any,
            },
            egress: ModelEgressPolicySnapshotV1::from_host(false),
            host_max_output_tokens: 2048,
        })
        .unwrap_or_else(|_| unreachable!())
    }

    fn prepared_analysis_for_task(task_id: TaskId) -> PreparedModelCallV1 {
        PreparedModelCallV1::from_host(PreparedModelCallDraftV1 {
            task_id: Some(task_id),
            purpose: ModelPurpose::Analysis,
            messages: vec![ModelMessage {
                role: serea_protocol::MessageRole::new("user").unwrap_or_else(|_| unreachable!()),
                content: "host prepared structured input".into(),
            }],
            system: Some("host system".into()),
            response_format: ResponseFormat::JsonSchema {
                schema: serde_json::json!({
                    "type": "object",
                    "properties": {"count": {"type": "integer"}},
                    "required": ["count"],
                    "additionalProperties": false
                }),
            },
            tools: Vec::new(),
            max_output_tokens: 16,
            temperature: 0.0,
            deadline_ms: 1000,
            data_class: DataClass::Public,
            requirements: ModelRoutingRequirementsV1 {
                vision_required: false,
                tools_required: false,
                min_context_tokens: 1,
                min_output_tokens: 1,
                structured_requirement: StructuredRequirementV1::Any,
            },
            egress: ModelEgressPolicySnapshotV1::from_host(false),
            host_max_output_tokens: 2048,
        })
        .unwrap_or_else(|_| unreachable!())
    }

    struct CancelAfterIntentGate(AtomicUsize);

    impl ModelDispatchGateSource for CancelAfterIntentGate {
        fn snapshot(
            &self,
            _task_id: Option<&TaskId>,
            _data_class: DataClass,
        ) -> Result<ModelDispatchGateSnapshotV1, ModelDispatchGateSourceError> {
            let read = self.0.fetch_add(1, Ordering::SeqCst);
            Ok(ModelDispatchGateSnapshotV1::from_host(
                read > 0,
                ModelEgressPolicySnapshotV1::from_host(true),
                1000,
            ))
        }
    }

    struct CancelBeforeRepairGate(AtomicUsize);

    impl ModelDispatchGateSource for CancelBeforeRepairGate {
        fn snapshot(
            &self,
            _task_id: Option<&TaskId>,
            _data_class: DataClass,
        ) -> Result<ModelDispatchGateSnapshotV1, ModelDispatchGateSourceError> {
            let read = self.0.fetch_add(1, Ordering::SeqCst);
            Ok(ModelDispatchGateSnapshotV1::from_host(
                read >= 2,
                ModelEgressPolicySnapshotV1::from_host(true),
                1000,
            ))
        }
    }

    fn dispatch_context<'a>(
        store: &'a Store,
        events: &'a EventBus,
        price: ModelPriceSnapshot,
        max_daily_spend_usd_micros: UsdMicros,
    ) -> ModelDispatchContext<'a> {
        ModelDispatchContext {
            store,
            events,
            price,
            prices: None,
            max_daily_spend_usd_micros,
            clock: &FixedClock,
            gate: &OPEN_DISPATCH_GATE,
        }
    }

    fn blocking_process(
        provider: Arc<BlockingProvider>,
        daily_limit: UsdMicros,
    ) -> ModelRouterProcessV1 {
        one_model_process(provider, daily_limit)
    }

    fn one_model_process(
        provider: Arc<dyn ModelProvider>,
        daily_limit: UsdMicros,
    ) -> ModelRouterProcessV1 {
        let roster = ModelRosterV1::new(vec![
            ModelRosterEntryV1::new(
                ModelId::new("nemotron-3-nano-30b").unwrap_or_else(|_| unreachable!()),
                provider.provider_id(),
                ModelDeploymentClass::Local,
                true,
                caps(false, JsonSchemaMode::Strict, 1000, 1000),
                CostClass::Paid,
            )
            .unwrap_or_else(|_| unreachable!()),
        ])
        .unwrap_or_else(|_| unreachable!());
        ModelRouterProcessV1::new(
            roster,
            vec![provider],
            vec![(
                ModelId::new("nemotron-3-nano-30b").unwrap_or_else(|_| unreachable!()),
                ModelPriceSnapshot::new(CostClass::Paid, "blocking-price", 1_000_000, 1_000_000),
            )],
            daily_limit,
        )
        .unwrap_or_else(|_| unreachable!())
    }

    struct IncrementingIds(u64);

    impl UlidSource for IncrementingIds {
        fn next_ulid(&mut self) -> UlidValue {
            self.0 += 1;
            let timestamp = TimestampMs::new(self.0).unwrap_or_else(|_| unreachable!());
            UlidValue::new(timestamp, [self.0 as u8; 10])
        }
    }

    struct DispatchFakeProvider {
        store: Arc<Store>,
        calls: AtomicUsize,
        called_event_visible_before_generate: AtomicBool,
        fail_next: AtomicBool,
        ambiguous_next: AtomicBool,
        mismatched_identity_next: AtomicBool,
        mismatched_request_id_next: AtomicBool,
        mismatched_provider_id_next: AtomicBool,
        wrong_cost_class_next: AtomicBool,
        scripted_content: Mutex<String>,
        finish_next: Mutex<Option<FinishReason>>,
        last_request_id: Mutex<Option<serea_protocol::RequestId>>,
        last_deadline_ms: Mutex<Option<u32>>,
    }

    struct RepairFakeProvider {
        requests: Mutex<Vec<ModelRequest>>,
        responses: Mutex<VecDeque<Result<String, ModelError>>>,
        finishes: Mutex<VecDeque<FinishReason>>,
        health_script: Mutex<VecDeque<ProviderHealth>>,
        health_calls: AtomicUsize,
    }

    struct BlockingProvider {
        started: SyncSender<serea_protocol::RequestId>,
        release: Mutex<Receiver<()>>,
        calls: AtomicUsize,
    }

    enum RepairScript {
        Respond(String),
        Fail(ModelError),
    }

    impl RepairFakeProvider {
        fn new(responses: Vec<String>) -> Self {
            Self {
                requests: Mutex::new(Vec::new()),
                responses: Mutex::new(responses.into_iter().map(Ok).collect()),
                finishes: Mutex::new(VecDeque::new()),
                health_script: Mutex::new(VecDeque::new()),
                health_calls: AtomicUsize::new(0),
            }
        }

        fn requests(&self) -> Vec<ModelRequest> {
            self.requests
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .clone()
        }

        fn push_script(&self, script: RepairScript) {
            let result = match script {
                RepairScript::Respond(content) => Ok(content),
                RepairScript::Fail(error) => Err(error),
            };
            self.responses
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .push_back(result);
        }

        fn push_health(&self, health: ProviderHealth) {
            self.health_script
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .push_back(health);
        }

        fn push_finish(&self, finish: FinishReason) {
            self.finishes
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .push_back(finish);
        }
    }

    #[async_trait]
    impl ModelProvider for RepairFakeProvider {
        fn provider_id(&self) -> ProviderId {
            ProviderId::new("provider").unwrap_or_else(|_| unreachable!())
        }

        fn models(&self) -> Vec<ModelDescriptor> {
            ["nemotron-3-nano-30b", "gpt-oss-20b"]
                .into_iter()
                .map(|model_id| ModelDescriptor {
                    model_id: ModelId::new(model_id).unwrap_or_else(|_| unreachable!()),
                    provider_id: self.provider_id(),
                    capabilities: caps(false, JsonSchemaMode::Strict, 1000, 1000),
                })
                .collect()
        }

        async fn generate(
            &self,
            request: &ModelRequest,
            _ctx: &ModelCallContext,
        ) -> Result<ModelResponse, ModelError> {
            self.requests
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .push(request.clone());
            let outcome = self
                .responses
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .pop_front()
                .unwrap_or_else(|| Ok(String::new()))?;
            let finish_reason = self
                .finishes
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .pop_front()
                .unwrap_or(FinishReason::Stop);
            Ok(ModelResponse {
                request_id: request.request_id.clone(),
                model_id: request.model_id.clone(),
                provider_id: self.provider_id(),
                content: outcome,
                structured: Some(serde_json::json!({"provider_proposal": true})),
                finish_reason,
                usage: ModelUsage {
                    input_tokens: TokenCount::new(3),
                    output_tokens: TokenCount::new(2),
                    cost_class: CostClass::Paid,
                },
                latency_ms: 7,
                repair_attempts: 0,
            })
        }

        async fn health(&self) -> ProviderHealth {
            self.health_calls.fetch_add(1, Ordering::SeqCst);
            self.health_script
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .pop_front()
                .unwrap_or(ProviderHealth::Ready)
        }
    }

    #[async_trait]
    impl ModelProvider for BlockingProvider {
        fn provider_id(&self) -> ProviderId {
            ProviderId::new("blocking_provider").unwrap_or_else(|_| unreachable!())
        }

        fn models(&self) -> Vec<ModelDescriptor> {
            vec![ModelDescriptor {
                model_id: ModelId::new("nemotron-3-nano-30b").unwrap_or_else(|_| unreachable!()),
                provider_id: self.provider_id(),
                capabilities: caps(false, JsonSchemaMode::Strict, 1000, 1000),
            }]
        }

        async fn generate(
            &self,
            request: &ModelRequest,
            _ctx: &ModelCallContext,
        ) -> Result<ModelResponse, ModelError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            self.started
                .send(request.request_id.clone())
                .map_err(|_| ModelError {
                    kind: ModelErrorCode::new("TEST_CHANNEL_CLOSED")
                        .unwrap_or_else(|_| unreachable!()),
                    message: serea_protocol::ErrorMessage::new("test channel closed")
                        .unwrap_or_else(|_| unreachable!()),
                    retryable: false,
                })?;
            self.release
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .recv()
                .map_err(|_| ModelError {
                    kind: ModelErrorCode::new("TEST_RELEASE_CLOSED")
                        .unwrap_or_else(|_| unreachable!()),
                    message: serea_protocol::ErrorMessage::new("test release closed")
                        .unwrap_or_else(|_| unreachable!()),
                    retryable: false,
                })?;
            Ok(ModelResponse {
                request_id: request.request_id.clone(),
                model_id: request.model_id.clone(),
                provider_id: self.provider_id(),
                content: "concurrent result".into(),
                structured: None,
                finish_reason: FinishReason::Stop,
                usage: ModelUsage {
                    input_tokens: TokenCount::new(1),
                    output_tokens: TokenCount::new(1),
                    cost_class: CostClass::Paid,
                },
                latency_ms: 1,
                repair_attempts: 0,
            })
        }

        async fn health(&self) -> ProviderHealth {
            ProviderHealth::Ready
        }
    }

    #[async_trait]
    impl ModelProvider for DispatchFakeProvider {
        fn provider_id(&self) -> ProviderId {
            ProviderId::new("provider").unwrap_or_else(|_| unreachable!())
        }

        fn models(&self) -> Vec<ModelDescriptor> {
            vec![ModelDescriptor {
                model_id: ModelId::new("nemotron-3-nano-30b").unwrap_or_else(|_| unreachable!()),
                provider_id: self.provider_id(),
                capabilities: caps(false, JsonSchemaMode::Strict, 1000, 1000),
            }]
        }

        async fn generate(
            &self,
            request: &ModelRequest,
            ctx: &ModelCallContext,
        ) -> Result<ModelResponse, ModelError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            let attempt = self
                .store
                .get_model_call_attempt(&request.request_id)
                .unwrap_or_else(|_| unreachable!());
            let page =
                EventBus::replay(&self.store, None, None, 16).unwrap_or_else(|_| unreachable!());
            let called = page.items.iter().any(|item| {
                matches!(item, ReplayItem::Event { event } if event.kind == EventKind::ModelCalled)
            });
            self.called_event_visible_before_generate
                .store(attempt.is_some() && called, Ordering::SeqCst);
            *self
                .last_request_id
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner()) =
                Some(request.request_id.clone());
            *self
                .last_deadline_ms
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(ctx.deadline_ms);
            if self.fail_next.swap(false, Ordering::SeqCst) {
                return Err(ModelError {
                    kind: ModelErrorCode::new("UPSTREAM_UNAVAILABLE")
                        .unwrap_or_else(|_| unreachable!()),
                    message: serea_protocol::ErrorMessage::new("sensitive diagnostic text")
                        .unwrap_or_else(|_| unreachable!()),
                    retryable: true,
                });
            }
            if self.ambiguous_next.swap(false, Ordering::SeqCst) {
                return Err(ModelError {
                    kind: ModelErrorCode::new(AMBIGUOUS_PROVIDER_ERROR_KIND)
                        .unwrap_or_else(|_| unreachable!()),
                    message: serea_protocol::ErrorMessage::new("outcome may have been processed")
                        .unwrap_or_else(|_| unreachable!()),
                    retryable: true,
                });
            }
            let model_id = if self.mismatched_identity_next.swap(false, Ordering::SeqCst) {
                ModelId::new("gpt-oss-20b").unwrap_or_else(|_| unreachable!())
            } else {
                request.model_id.clone()
            };
            let request_id = if self
                .mismatched_request_id_next
                .swap(false, Ordering::SeqCst)
            {
                serea_protocol::RequestId::new("req_00000000000000000000000001")
                    .unwrap_or_else(|_| unreachable!())
            } else {
                request.request_id.clone()
            };
            let provider_id = if self
                .mismatched_provider_id_next
                .swap(false, Ordering::SeqCst)
            {
                ProviderId::new("other_provider").unwrap_or_else(|_| unreachable!())
            } else {
                self.provider_id()
            };
            let cost_class = if self.wrong_cost_class_next.swap(false, Ordering::SeqCst) {
                CostClass::Free
            } else {
                CostClass::Paid
            };
            let finish_reason = self
                .finish_next
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .take()
                .unwrap_or(FinishReason::Stop);
            Ok(ModelResponse {
                request_id,
                model_id,
                provider_id,
                content: self
                    .scripted_content
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner())
                    .clone(),
                structured: Some(serde_json::json!({"untrusted": true})),
                finish_reason,
                usage: ModelUsage {
                    input_tokens: TokenCount::new(3),
                    output_tokens: TokenCount::new(2),
                    cost_class,
                },
                latency_ms: 7,
                repair_attempts: 2,
            })
        }
    }

    #[test]
    fn dispatch_commits_before_provider_and_recovers_caller_lost_response() {
        let db_path = std::env::temp_dir().join(format!(
            "serea-router-dispatch-{}-{}.sqlite",
            std::process::id(),
            NEXT_RECOVERY_TEST_DB.fetch_add(1, Ordering::SeqCst)
        ));
        let store = Arc::new(Store::open(&db_path, &FixedClock).unwrap_or_else(|_| unreachable!()));
        let provider = Arc::new(DispatchFakeProvider {
            store: Arc::clone(&store),
            calls: AtomicUsize::new(0),
            called_event_visible_before_generate: AtomicBool::new(false),
            fail_next: AtomicBool::new(false),
            ambiguous_next: AtomicBool::new(false),
            mismatched_identity_next: AtomicBool::new(false),
            mismatched_request_id_next: AtomicBool::new(false),
            mismatched_provider_id_next: AtomicBool::new(false),
            wrong_cost_class_next: AtomicBool::new(false),
            scripted_content: Mutex::new("hello".into()),
            finish_next: Mutex::new(None),
            last_request_id: Mutex::new(None),
            last_deadline_ms: Mutex::new(None),
        });
        let bus = EventBus::new(IncrementingIds(0));
        let roster = ModelRosterV1::new(vec![
            ModelRosterEntryV1::new(
                ModelId::new("nemotron-3-nano-30b").unwrap_or_else(|_| unreachable!()),
                provider.provider_id(),
                ModelDeploymentClass::Local,
                true,
                caps(false, JsonSchemaMode::Strict, 1000, 1000),
                CostClass::Paid,
            )
            .unwrap_or_else(|_| unreachable!()),
        ])
        .unwrap_or_else(|_| unreachable!());
        let router =
            ModelRouterV1::new(roster, vec![provider.clone()]).unwrap_or_else(|_| unreachable!());
        let call = PreparedModelCallV1::from_host(PreparedModelCallDraftV1 {
            task_id: None,
            purpose: ModelPurpose::Chat,
            messages: vec![ModelMessage {
                role: serea_protocol::MessageRole::new("user").unwrap_or_else(|_| unreachable!()),
                content: "hi".into(),
            }],
            system: Some("system".into()),
            response_format: ResponseFormat::Text,
            tools: Vec::new(),
            max_output_tokens: 128,
            temperature: 0.25,
            deadline_ms: 1000,
            data_class: DataClass::Public,
            requirements: ModelRoutingRequirementsV1 {
                vision_required: false,
                tools_required: false,
                min_context_tokens: 1,
                min_output_tokens: 1,
                structured_requirement: StructuredRequirementV1::Any,
            },
            egress: ModelEgressPolicySnapshotV1::from_host(false),
            host_max_output_tokens: 2048,
        })
        .unwrap_or_else(|_| unreachable!());
        let session = block_on(router.route(&call)).unwrap_or_else(|_| unreachable!());
        let context = dispatch_context(
            &store,
            &bus,
            ModelPriceSnapshot::new(CostClass::Paid, "price-1", 1_000_000, 1_000_000),
            UsdMicros::new(10_000_000).unwrap_or_else(|_| unreachable!()),
        );
        let cancelled_context = ModelDispatchContext {
            gate: &CANCELLED_DISPATCH_GATE,
            ..context
        };
        assert!(matches!(
            block_on(router.dispatch_chat_text(&call, &session, &cancelled_context)),
            Err(ModelDispatchFailure::Refused(RouterError::TaskCancelled))
        ));
        assert_eq!(provider.calls.load(Ordering::SeqCst), 0);
        let before_cancelled_dispatch =
            EventBus::replay(&store, None, None, 16).unwrap_or_else(|_| unreachable!());
        assert!(!before_cancelled_dispatch.items.iter().any(
            |item| matches!(item, ReplayItem::Event { event } if event.kind == EventKind::ModelCalled)
        ));

        let context = dispatch_context(
            &store,
            &bus,
            ModelPriceSnapshot::new(CostClass::Paid, "price-1", 1_000_000, 1_000_000),
            UsdMicros::new(10_000_000).unwrap_or_else(|_| unreachable!()),
        );
        let response = block_on(router.dispatch_chat_text(&call, &session, &context))
            .unwrap_or_else(|_| unreachable!());

        assert_eq!(provider.calls.load(Ordering::SeqCst), 1);
        assert_eq!(
            *provider
                .last_deadline_ms
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner()),
            Some(997)
        );
        assert!(
            provider
                .called_event_visible_before_generate
                .load(Ordering::SeqCst)
        );
        assert_eq!(response.content, "hello");
        assert_eq!(response.structured, None);
        assert_eq!(response.repair_attempts, 0);
        let recovered_response = recover_completed_chat_text_response(&store, &response.request_id)
            .unwrap_or_else(|_| unreachable!())
            .unwrap_or_else(|| unreachable!());
        assert_eq!(recovered_response, response);
        let attempt = store
            .get_model_call_attempt(&response.request_id)
            .unwrap_or_else(|_| unreachable!())
            .unwrap_or_else(|| unreachable!());
        assert_eq!(attempt.state, ModelAttemptState::Completed);
        let events = EventBus::replay(&store, None, None, 16).unwrap_or_else(|_| unreachable!());
        assert!(matches!(events.items.as_slice(), [
            ReplayItem::Event { event: called },
            ReplayItem::Event { event: completed },
        ] if called.kind == EventKind::ModelCalled && completed.kind == EventKind::ModelCompleted));

        let wrong_price_context = dispatch_context(
            &store,
            &bus,
            ModelPriceSnapshot::new(CostClass::Free, "price-2", 0, 0),
            UsdMicros::new(10_000_000).unwrap_or_else(|_| unreachable!()),
        );
        let wrong_price =
            block_on(router.dispatch_chat_text(&call, &session, &wrong_price_context));
        assert!(matches!(
            wrong_price,
            Err(ModelDispatchFailure::Refused(
                RouterError::PriceConfigurationInvalid
            ))
        ));

        let second_bus = EventBus::new(IncrementingIds(100));
        let no_spend_context = dispatch_context(
            &store,
            &second_bus,
            ModelPriceSnapshot::new(CostClass::Paid, "price-1", 1_000_000, 1_000_000),
            UsdMicros::new(1).unwrap_or_else(|_| unreachable!()),
        );
        let reservation_failure =
            block_on(router.dispatch_chat_text(&call, &session, &no_spend_context));
        assert!(matches!(
            reservation_failure,
            Err(ModelDispatchFailure::BoundExceeded {
                bound: ModelBoundKindV1::DailySpendUsd,
                limit: 1,
                observed,
            }) if observed > 1
        ));
        assert_eq!(provider.calls.load(Ordering::SeqCst), 1);
        assert_eq!(
            EventBus::replay(&store, None, None, 16)
                .unwrap_or_else(|_| unreachable!())
                .items
                .len(),
            3
        );

        provider.fail_next.store(true, Ordering::SeqCst);
        let retry_session = block_on(router.route(&call)).unwrap_or_else(|_| unreachable!());
        let retry_context = dispatch_context(
            &store,
            &bus,
            ModelPriceSnapshot::new(CostClass::Paid, "price-1", 1_000_000, 1_000_000),
            UsdMicros::new(10_000_000).unwrap_or_else(|_| unreachable!()),
        );
        let failure = block_on(router.dispatch_chat_text(&call, &retry_session, &retry_context));
        assert!(matches!(
            failure,
            Err(ModelDispatchFailure::DefiniteProvider {
                error_kind,
                retryable: true,
                ..
            }) if error_kind.as_str() == "UPSTREAM_UNAVAILABLE"
        ));
        let failed_request = provider
            .last_request_id
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
            .unwrap_or_else(|| unreachable!());
        assert_eq!(
            store
                .get_model_call_attempt(&failed_request)
                .unwrap_or_else(|_| unreachable!())
                .unwrap_or_else(|| unreachable!())
                .state,
            ModelAttemptState::Failed
        );
        let final_events =
            EventBus::replay(&store, None, None, 16).unwrap_or_else(|_| unreachable!());
        assert!(matches!(
            final_events.items.last(),
            Some(ReplayItem::Event { event }) if event.kind == EventKind::ModelFailed
                && event.payload["error_kind"] == "UPSTREAM_UNAVAILABLE"
                && event.payload["retryable"] == true
        ));
        let serialized_events =
            serde_json::to_string(&final_events.items).unwrap_or_else(|_| unreachable!());
        assert!(!serialized_events.contains("sensitive diagnostic text"));

        provider.ambiguous_next.store(true, Ordering::SeqCst);
        let ambiguous_session = block_on(router.route(&call)).unwrap_or_else(|_| unreachable!());
        let ambiguous_context = dispatch_context(
            &store,
            &bus,
            ModelPriceSnapshot::new(CostClass::Paid, "price-1", 1_000_000, 1_000_000),
            UsdMicros::new(10_000_000).unwrap_or_else(|_| unreachable!()),
        );
        let ambiguous =
            block_on(router.dispatch_chat_text(&call, &ambiguous_session, &ambiguous_context));
        assert!(matches!(
            ambiguous,
            Err(ModelDispatchFailure::AmbiguousProvider { error_kind, .. })
                if error_kind.as_str() == AMBIGUOUS_PROVIDER_ERROR_KIND
        ));
        let ambiguous_request = provider
            .last_request_id
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
            .unwrap_or_else(|| unreachable!());
        let ambiguous_attempt = store
            .get_model_call_attempt(&ambiguous_request)
            .unwrap_or_else(|_| unreachable!())
            .unwrap_or_else(|| unreachable!());
        assert_eq!(ambiguous_attempt.state, ModelAttemptState::Ambiguous);
        assert_eq!(ambiguous_attempt.actual_cost_usd_micros, None);
        assert!(
            store
                .model_usage_for_request(&ambiguous_request)
                .unwrap_or_else(|_| unreachable!())
                .is_none()
        );
        let ambiguity_events =
            EventBus::replay(&store, None, None, 16).unwrap_or_else(|_| unreachable!());
        assert!(matches!(
            ambiguity_events.items.last(),
            Some(ReplayItem::Event { event }) if event.kind == EventKind::ModelFailed
                && event.payload["error_kind"] == AMBIGUOUS_PROVIDER_ERROR_KIND
                && event.payload["retryable"] == false
        ));

        provider
            .mismatched_identity_next
            .store(true, Ordering::SeqCst);
        let spoof_session = block_on(router.route(&call)).unwrap_or_else(|_| unreachable!());
        let spoof_context = dispatch_context(
            &store,
            &bus,
            ModelPriceSnapshot::new(CostClass::Paid, "price-1", 1_000_000, 1_000_000),
            UsdMicros::new(10_000_000).unwrap_or_else(|_| unreachable!()),
        );
        let spoof_result =
            block_on(router.dispatch_chat_text(&call, &spoof_session, &spoof_context));
        assert!(matches!(
            spoof_result,
            Err(ModelDispatchFailure::TerminalProvider { error_kind, .. })
                if error_kind.as_str() == "PROVIDER_PROTOCOL_FAILURE"
        ));
        let spoof_request = provider
            .last_request_id
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
            .unwrap_or_else(|| unreachable!());
        let spoof_attempt = store
            .get_model_call_attempt(&spoof_request)
            .unwrap_or_else(|_| unreachable!())
            .unwrap_or_else(|| unreachable!());
        assert_eq!(spoof_attempt.state, ModelAttemptState::Failed);
        assert_eq!(spoof_attempt.response_blob, None);
        assert!(
            store
                .model_usage_for_request(&spoof_request)
                .unwrap_or_else(|_| unreachable!())
                .is_none()
        );

        for (request_mismatch, expected_calls) in [(true, 5), (false, 6)] {
            if request_mismatch {
                provider
                    .mismatched_request_id_next
                    .store(true, Ordering::SeqCst);
            } else {
                provider
                    .mismatched_provider_id_next
                    .store(true, Ordering::SeqCst);
            }
            let identity_session = block_on(router.route(&call)).unwrap_or_else(|_| unreachable!());
            let identity_context = dispatch_context(
                &store,
                &bus,
                ModelPriceSnapshot::new(CostClass::Paid, "price-1", 1_000_000, 1_000_000),
                UsdMicros::new(10_000_000).unwrap_or_else(|_| unreachable!()),
            );
            let identity_result =
                block_on(router.dispatch_chat_text(&call, &identity_session, &identity_context));
            assert!(matches!(
                identity_result,
                Err(ModelDispatchFailure::TerminalProvider { error_kind, .. })
                    if error_kind.as_str() == "PROVIDER_PROTOCOL_FAILURE"
            ));
            let identity_request = provider
                .last_request_id
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .clone()
                .unwrap_or_else(|| unreachable!());
            let identity_attempt = store
                .get_model_call_attempt(&identity_request)
                .unwrap_or_else(|_| unreachable!())
                .unwrap_or_else(|| unreachable!());
            assert_eq!(identity_attempt.state, ModelAttemptState::Failed);
            assert_eq!(identity_attempt.response_blob, None);
            assert!(
                store
                    .model_usage_for_request(&identity_request)
                    .unwrap_or_else(|_| unreachable!())
                    .is_none()
            );
            let events =
                EventBus::replay(&store, None, None, 32).unwrap_or_else(|_| unreachable!());
            assert!(matches!(
                events.items.last(),
                Some(ReplayItem::Event { event })
                    if event.kind == EventKind::ModelFailed
                        && event.payload["error_kind"] == "PROVIDER_PROTOCOL_FAILURE"
                        && event.payload["retryable"] == false
            ));
            assert_eq!(provider.calls.load(Ordering::SeqCst), expected_calls);
            drop(identity_context);
        }

        provider.wrong_cost_class_next.store(true, Ordering::SeqCst);
        let cost_session = block_on(router.route(&call)).unwrap_or_else(|_| unreachable!());
        let cost_context = dispatch_context(
            &store,
            &bus,
            ModelPriceSnapshot::new(CostClass::Paid, "price-1", 1_000_000, 1_000_000),
            UsdMicros::new(10_000_000).unwrap_or_else(|_| unreachable!()),
        );
        let cost_result = block_on(router.dispatch_chat_text(&call, &cost_session, &cost_context));
        assert!(matches!(
            cost_result,
            Err(ModelDispatchFailure::TerminalProvider { error_kind, .. })
                if error_kind.as_str() == "PROVIDER_METADATA_INVALID"
        ));
        let cost_request = provider
            .last_request_id
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
            .unwrap_or_else(|| unreachable!());
        let cost_attempt = store
            .get_model_call_attempt(&cost_request)
            .unwrap_or_else(|_| unreachable!())
            .unwrap_or_else(|| unreachable!());
        assert_eq!(cost_attempt.state, ModelAttemptState::Failed);
        assert_eq!(cost_attempt.response_blob, None);
        assert!(
            store
                .model_usage_for_request(&cost_request)
                .unwrap_or_else(|_| unreachable!())
                .is_none()
        );

        for (finish_reason, expected_kind) in [
            (FinishReason::ContentFilter, "MODEL_CONTENT_FILTER"),
            (FinishReason::Length, "MODEL_FINISH_LENGTH"),
            (FinishReason::Error, "MODEL_FINISH_ERROR"),
            (FinishReason::StructureInvalid, "MODEL_STRUCTURE_INVALID"),
        ] {
            *provider
                .finish_next
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(finish_reason);
            let finish_session = block_on(router.route(&call)).unwrap_or_else(|_| unreachable!());
            let finish_context = dispatch_context(
                &store,
                &bus,
                ModelPriceSnapshot::new(CostClass::Paid, "price-1", 1_000_000, 1_000_000),
                UsdMicros::new(10_000_000).unwrap_or_else(|_| unreachable!()),
            );
            let finish_result =
                block_on(router.dispatch_chat_text(&call, &finish_session, &finish_context));
            assert!(matches!(
                finish_result,
                Err(ModelDispatchFailure::TerminalProvider { error_kind, .. })
                    if error_kind.as_str() == expected_kind
            ));
            let finish_request = provider
                .last_request_id
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .clone()
                .unwrap_or_else(|| unreachable!());
            let finish_attempt = store
                .get_model_call_attempt(&finish_request)
                .unwrap_or_else(|_| unreachable!())
                .unwrap_or_else(|| unreachable!());
            assert_eq!(finish_attempt.state, ModelAttemptState::Failed);
            assert_eq!(finish_attempt.response_blob, None);
            let usage = store
                .model_usage_for_request(&finish_request)
                .unwrap_or_else(|_| unreachable!())
                .unwrap_or_else(|| unreachable!());
            assert_eq!(usage.input_tokens.get(), 3);
            assert_eq!(usage.output_tokens.get(), 2);
            assert_eq!(usage.cost_usd_micros.get(), 5);
            let events =
                EventBus::replay(&store, None, None, 32).unwrap_or_else(|_| unreachable!());
            assert!(matches!(
                events.items.last(),
                Some(ReplayItem::Event { event })
                    if event.kind == EventKind::ModelFailed
                        && event.payload["error_kind"] == expected_kind
                        && event.payload["retryable"] == false
            ));
            drop(finish_context);
        }

        let calls_before_reopen = provider.calls.load(Ordering::SeqCst);
        drop(router);
        drop(provider);
        drop(context);
        drop(wrong_price_context);
        drop(no_spend_context);
        drop(retry_context);
        drop(ambiguous_context);
        drop(spoof_context);
        drop(cost_context);
        drop(store);
        let reopened = Store::open(&db_path, &FixedClock).unwrap_or_else(|_| unreachable!());
        let recovered = recover_completed_chat_text_response(&reopened, &response.request_id)
            .unwrap_or_else(|_| unreachable!())
            .unwrap_or_else(|| unreachable!());
        assert_eq!(recovered, response);
        assert_eq!(calls_before_reopen, 11);
        assert_eq!(
            reopened
                .model_usage_for_request(&response.request_id)
                .unwrap_or_else(|_| unreachable!())
                .unwrap_or_else(|| unreachable!())
                .cost_usd_micros
                .get(),
            5
        );
        let reopened_events =
            EventBus::replay(&reopened, None, None, 32).unwrap_or_else(|_| unreachable!());
        assert_eq!(reopened_events.items.len(), 23);
        let serialized_reopened_events =
            serde_json::to_string(&reopened_events.items).unwrap_or_else(|_| unreachable!());
        assert!(!serialized_reopened_events.contains("hello"));
        assert!(!serialized_reopened_events.contains("outcome may have been processed"));
        drop(reopened);
        let _ = std::fs::remove_file(&db_path);
        let _ = std::fs::remove_file(db_path.with_extension("sqlite-wal"));
        let _ = std::fs::remove_file(db_path.with_extension("sqlite-shm"));
    }

    #[test]
    fn gate_is_rechecked_after_intent_before_provider_call() {
        let db_path = std::env::temp_dir().join(format!(
            "serea-router-gate-race-{}-{}.sqlite",
            std::process::id(),
            NEXT_RECOVERY_TEST_DB.fetch_add(1, Ordering::SeqCst)
        ));
        let store = Arc::new(Store::open(&db_path, &FixedClock).unwrap_or_else(|_| unreachable!()));
        let provider = Arc::new(DispatchFakeProvider {
            store: Arc::clone(&store),
            calls: AtomicUsize::new(0),
            called_event_visible_before_generate: AtomicBool::new(false),
            fail_next: AtomicBool::new(false),
            ambiguous_next: AtomicBool::new(false),
            mismatched_identity_next: AtomicBool::new(false),
            mismatched_request_id_next: AtomicBool::new(false),
            mismatched_provider_id_next: AtomicBool::new(false),
            wrong_cost_class_next: AtomicBool::new(false),
            scripted_content: Mutex::new("hello".into()),
            finish_next: Mutex::new(None),
            last_request_id: Mutex::new(None),
            last_deadline_ms: Mutex::new(None),
        });
        let provider_id = provider.provider_id();
        let roster = ModelRosterV1::new(vec![
            ModelRosterEntryV1::new(
                ModelId::new("nemotron-3-nano-30b").unwrap_or_else(|_| unreachable!()),
                provider_id,
                ModelDeploymentClass::Local,
                true,
                caps(false, JsonSchemaMode::Strict, 1000, 1000),
                CostClass::Paid,
            )
            .unwrap_or_else(|_| unreachable!()),
        ])
        .unwrap_or_else(|_| unreachable!());
        let router =
            ModelRouterV1::new(roster, vec![provider.clone()]).unwrap_or_else(|_| unreachable!());
        let call = PreparedModelCallV1::from_host(PreparedModelCallDraftV1 {
            task_id: None,
            purpose: ModelPurpose::Chat,
            messages: Vec::new(),
            system: None,
            response_format: ResponseFormat::Text,
            tools: Vec::new(),
            max_output_tokens: 8,
            temperature: 0.0,
            deadline_ms: 1000,
            data_class: DataClass::Public,
            requirements: ModelRoutingRequirementsV1 {
                vision_required: false,
                tools_required: false,
                min_context_tokens: 1,
                min_output_tokens: 1,
                structured_requirement: StructuredRequirementV1::Any,
            },
            egress: ModelEgressPolicySnapshotV1::from_host(false),
            host_max_output_tokens: 2048,
        })
        .unwrap_or_else(|_| unreachable!());
        let session = block_on(router.route(&call)).unwrap_or_else(|_| unreachable!());
        let bus = EventBus::new(IncrementingIds(0));
        let gate = CancelAfterIntentGate(AtomicUsize::new(0));
        let context = ModelDispatchContext {
            store: &store,
            events: &bus,
            price: ModelPriceSnapshot::new(CostClass::Paid, "price-1", 1_000_000, 1_000_000),
            prices: None,
            max_daily_spend_usd_micros: UsdMicros::new(10_000_000)
                .unwrap_or_else(|_| unreachable!()),
            clock: &FixedClock,
            gate: &gate,
        };

        assert!(matches!(
            block_on(router.dispatch_chat_text(&call, &session, &context)),
            Err(ModelDispatchFailure::Refused(RouterError::TaskCancelled))
        ));
        assert_eq!(provider.calls.load(Ordering::SeqCst), 0);
        let page = EventBus::replay(&store, None, None, 16).unwrap_or_else(|_| unreachable!());
        assert_eq!(
            page.items
                .iter()
                .filter(|item| matches!(item, ReplayItem::Event { event } if event.kind == EventKind::ModelCalled))
                .count(),
            1
        );
        let failed = page.items.iter().find_map(|item| match item {
            ReplayItem::Event { event } if event.kind == EventKind::ModelFailed => Some(event),
            _ => None,
        });
        let failed = failed.unwrap_or_else(|| unreachable!());
        assert_eq!(failed.payload["error_kind"], "HOST_DISPATCH_BLOCKED");
        assert_eq!(failed.payload["retryable"], false);
        let request_id = serea_protocol::RequestId::new(
            failed.payload["request_id"]
                .as_str()
                .unwrap_or_else(|| unreachable!()),
        )
        .unwrap_or_else(|_| unreachable!());
        let attempt = store
            .get_model_call_attempt(&request_id)
            .unwrap_or_else(|_| unreachable!())
            .unwrap_or_else(|| unreachable!());
        assert_eq!(attempt.state, ModelAttemptState::Failed);
        assert_eq!(
            attempt.actual_cost_usd_micros,
            Some(UsdMicros::new(0).unwrap_or_else(|_| unreachable!()))
        );
        assert!(
            store
                .model_usage_for_request(&request_id)
                .unwrap_or_else(|_| unreachable!())
                .is_none()
        );
        assert_eq!(provider.calls.load(Ordering::SeqCst), 0);
        drop(context);
        drop(router);
        drop(provider);
        drop(store);
        let _ = std::fs::remove_file(&db_path);
        let _ = std::fs::remove_file(db_path.with_extension("sqlite-wal"));
        let _ = std::fs::remove_file(db_path.with_extension("sqlite-shm"));
    }

    #[test]
    fn structured_dispatch_returns_host_validated_value_and_persists_it() {
        let db_path = std::env::temp_dir().join(format!(
            "serea-router-structured-{}-{}.sqlite",
            std::process::id(),
            NEXT_RECOVERY_TEST_DB.fetch_add(1, Ordering::SeqCst)
        ));
        let store = Arc::new(Store::open(&db_path, &FixedClock).unwrap_or_else(|_| unreachable!()));
        let provider = Arc::new(DispatchFakeProvider {
            store: Arc::clone(&store),
            calls: AtomicUsize::new(0),
            called_event_visible_before_generate: AtomicBool::new(false),
            fail_next: AtomicBool::new(false),
            ambiguous_next: AtomicBool::new(false),
            mismatched_identity_next: AtomicBool::new(false),
            mismatched_request_id_next: AtomicBool::new(false),
            mismatched_provider_id_next: AtomicBool::new(false),
            wrong_cost_class_next: AtomicBool::new(false),
            scripted_content: Mutex::new(r#"{"count":7}"#.into()),
            finish_next: Mutex::new(None),
            last_request_id: Mutex::new(None),
            last_deadline_ms: Mutex::new(None),
        });
        let bus = EventBus::new(IncrementingIds(0));
        let roster = ModelRosterV1::new(vec![
            ModelRosterEntryV1::new(
                ModelId::new("nemotron-3-nano-30b").unwrap_or_else(|_| unreachable!()),
                provider.provider_id(),
                ModelDeploymentClass::Local,
                true,
                caps(false, JsonSchemaMode::Strict, 1000, 1000),
                CostClass::Paid,
            )
            .unwrap_or_else(|_| unreachable!()),
        ])
        .unwrap_or_else(|_| unreachable!());
        let router =
            ModelRouterV1::new(roster, vec![provider.clone()]).unwrap_or_else(|_| unreachable!());
        let schema = serde_json::json!({
            "type": "object",
            "properties": {"count": {"type": "integer"}},
            "required": ["count"],
            "additionalProperties": false
        });
        let raw = r#"{"count":7}"#;
        let call = PreparedModelCallV1::from_host(PreparedModelCallDraftV1 {
            task_id: None,
            purpose: ModelPurpose::Analysis,
            messages: vec![ModelMessage {
                role: serea_protocol::MessageRole::new("user").unwrap_or_else(|_| unreachable!()),
                content: "extract the count".into(),
            }],
            system: None,
            response_format: ResponseFormat::JsonSchema { schema },
            tools: Vec::new(),
            max_output_tokens: 128,
            temperature: 0.25,
            deadline_ms: 1000,
            data_class: DataClass::Public,
            requirements: ModelRoutingRequirementsV1 {
                vision_required: false,
                tools_required: false,
                min_context_tokens: 1,
                min_output_tokens: 1,
                structured_requirement: StructuredRequirementV1::Any,
            },
            egress: ModelEgressPolicySnapshotV1::from_host(false),
            host_max_output_tokens: 2048,
        })
        .unwrap_or_else(|_| unreachable!());
        let session = block_on(router.route(&call)).unwrap_or_else(|_| unreachable!());
        let context = dispatch_context(
            &store,
            &bus,
            ModelPriceSnapshot::new(CostClass::Paid, "price-1", 1_000_000, 1_000_000),
            UsdMicros::new(10_000_000).unwrap_or_else(|_| unreachable!()),
        );

        let response = block_on(router.dispatch_structured(&call, &session, &context))
            .unwrap_or_else(|_| unreachable!());

        assert_eq!(provider.calls.load(Ordering::SeqCst), 1);
        assert_eq!(response.content, raw);
        assert_eq!(response.structured, Some(serde_json::json!({"count":7})));
        assert_eq!(response.repair_attempts, 0);
        let attempt = store
            .get_model_call_attempt(&response.request_id)
            .unwrap_or_else(|_| unreachable!())
            .unwrap_or_else(|| unreachable!());
        assert_eq!(attempt.state, ModelAttemptState::Completed);
        let stored = store
            .get_model_call_response(&response.request_id)
            .unwrap_or_else(|_| unreachable!())
            .unwrap_or_else(|| unreachable!());
        let stored: serde_json::Value =
            serde_json::from_slice(&stored).unwrap_or_else(|_| unreachable!());
        assert_eq!(stored["content"], raw);
        assert_eq!(stored["structured"], serde_json::json!({"count":7}));
        let events = EventBus::replay(&store, None, None, 16).unwrap_or_else(|_| unreachable!());
        assert!(matches!(events.items.as_slice(), [
            ReplayItem::Event { event: called },
            ReplayItem::Event { event: completed },
        ] if called.kind == EventKind::ModelCalled && completed.kind == EventKind::ModelCompleted));

        let raw_invalid = r#"{"count":"private_fragment_marker"}"#;
        *provider
            .scripted_content
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = raw_invalid.into();
        let invalid_result = block_on(router.dispatch_structured(&call, &session, &context));
        let Err(ModelDispatchFailure::StructuredOutputInvalid {
            request_id: invalid_request_id,
            validation_error: StructuredValidationError::InvalidOutput(diagnostics),
            ..
        }) = invalid_result
        else {
            unreachable!()
        };
        assert!(
            !serde_json::to_string(&diagnostics)
                .unwrap_or_else(|_| unreachable!())
                .contains("private_fragment_marker")
        );
        let invalid_attempt = store
            .get_model_call_attempt(&invalid_request_id)
            .unwrap_or_else(|_| unreachable!())
            .unwrap_or_else(|| unreachable!());
        assert_eq!(invalid_attempt.state, ModelAttemptState::Failed);
        assert_eq!(invalid_attempt.response_blob, None);
        let invalid_usage = store
            .model_usage_for_request(&invalid_request_id)
            .unwrap_or_else(|_| unreachable!())
            .unwrap_or_else(|| unreachable!());
        assert_eq!(invalid_usage.input_tokens.get(), 3);
        assert_eq!(invalid_usage.output_tokens.get(), 2);
        assert_eq!(invalid_usage.cost_usd_micros.get(), 5);
        let events = EventBus::replay(&store, None, None, 16).unwrap_or_else(|_| unreachable!());
        let event_json = serde_json::to_string(
            &events
                .items
                .iter()
                .filter_map(|item| match item {
                    ReplayItem::Event { event } => Some(&event.payload),
                    _ => None,
                })
                .collect::<Vec<_>>(),
        )
        .unwrap_or_else(|_| unreachable!());
        assert!(!event_json.contains("private_fragment_marker"));
        assert!(matches!(events.items.as_slice(), [
            ReplayItem::Event { event: called },
            ReplayItem::Event { event: completed },
            ReplayItem::Event { event: called_invalid },
            ReplayItem::Event { event: completed_invalid },
            ReplayItem::Event { event: output_invalid },
        ] if called.kind == EventKind::ModelCalled
            && completed.kind == EventKind::ModelCompleted
            && called_invalid.kind == EventKind::ModelCalled
            && completed_invalid.kind == EventKind::ModelCompleted
            && output_invalid.kind == EventKind::ModelOutputInvalid));
        assert_eq!(provider.calls.load(Ordering::SeqCst), 2);
        drop(context);
        drop(router);
        drop(provider);
        drop(store);
        let reopened = Store::open(&db_path, &FixedClock).unwrap_or_else(|_| unreachable!());
        let recovered = recover_completed_structured_response(&reopened, &response.request_id)
            .unwrap_or_else(|_| unreachable!())
            .unwrap_or_else(|| unreachable!());
        assert_eq!(recovered, response);
        drop(reopened);
        let _ = std::fs::remove_file(&db_path);
        let _ = std::fs::remove_file(db_path.with_extension("sqlite-wal"));
        let _ = std::fs::remove_file(db_path.with_extension("sqlite-shm"));
    }

    #[test]
    fn structured_invalid_output_repairs_with_minimal_context_and_host_lineage() {
        let db_path = std::env::temp_dir().join(format!(
            "serea-router-repair-{}-{}.sqlite",
            std::process::id(),
            NEXT_RECOVERY_TEST_DB.fetch_add(1, Ordering::SeqCst)
        ));
        let store = Arc::new(Store::open(&db_path, &FixedClock).unwrap_or_else(|_| unreachable!()));
        let provider = Arc::new(RepairFakeProvider::new(vec![
            r#"{"count":"private_fragment_marker"}"#.into(),
            r#"{"count":7}"#.into(),
        ]));
        let bus = EventBus::new(IncrementingIds(0));
        let caps = caps(false, JsonSchemaMode::Strict, 1000, 1000);
        let provider_id = provider.provider_id();
        let roster = ModelRosterV1::new(vec![
            ModelRosterEntryV1::new(
                ModelId::new("nemotron-3-nano-30b").unwrap_or_else(|_| unreachable!()),
                provider_id.clone(),
                ModelDeploymentClass::Local,
                true,
                caps,
                CostClass::Paid,
            )
            .unwrap_or_else(|_| unreachable!()),
            ModelRosterEntryV1::new(
                ModelId::new("gpt-oss-20b").unwrap_or_else(|_| unreachable!()),
                provider_id,
                ModelDeploymentClass::Local,
                true,
                caps,
                CostClass::Paid,
            )
            .unwrap_or_else(|_| unreachable!()),
        ])
        .unwrap_or_else(|_| unreachable!());
        let router =
            ModelRouterV1::new(roster, vec![provider.clone()]).unwrap_or_else(|_| unreachable!());
        let schema = serde_json::json!({
            "type": "object",
            "properties": {"count": {"type": "integer"}},
            "required": ["count"],
            "additionalProperties": false
        });
        let call = PreparedModelCallV1::from_host(PreparedModelCallDraftV1 {
            task_id: None,
            purpose: ModelPurpose::Analysis,
            messages: vec![ModelMessage {
                role: serea_protocol::MessageRole::new("user").unwrap_or_else(|_| unreachable!()),
                content: "unrelated conversation marker".into(),
            }],
            system: Some("unrelated system marker".into()),
            response_format: ResponseFormat::JsonSchema {
                schema: schema.clone(),
            },
            tools: vec![serde_json::json!({"name":"unrelated-tool"})],
            max_output_tokens: 128,
            temperature: 0.25,
            deadline_ms: 1000,
            data_class: DataClass::Public,
            requirements: ModelRoutingRequirementsV1 {
                vision_required: false,
                tools_required: false,
                min_context_tokens: 1,
                min_output_tokens: 1,
                structured_requirement: StructuredRequirementV1::Any,
            },
            egress: ModelEgressPolicySnapshotV1::from_host(false),
            host_max_output_tokens: 2048,
        })
        .unwrap_or_else(|_| unreachable!());
        let session = block_on(router.route(&call)).unwrap_or_else(|_| unreachable!());
        let context = dispatch_context(
            &store,
            &bus,
            ModelPriceSnapshot::new(CostClass::Paid, "price-1", 1_000_000, 1_000_000),
            UsdMicros::new(10_000_000).unwrap_or_else(|_| unreachable!()),
        );

        let response = block_on(router.dispatch_structured_with_repair(&call, &session, &context))
            .unwrap_or_else(|_| unreachable!());
        assert_eq!(provider.health_calls.load(Ordering::SeqCst), 2);

        let requests = provider.requests();
        assert_eq!(requests.len(), 2);
        assert_eq!(requests[0].model_id.as_str(), "nemotron-3-nano-30b");
        assert_eq!(requests[1].model_id.as_str(), "gpt-oss-20b");
        assert_eq!(requests[1].purpose, ModelPurpose::StructuredRepair);
        assert_eq!(
            requests[1].response_format,
            ResponseFormat::JsonSchema {
                schema: schema.clone()
            }
        );
        assert!(requests[1].tools.is_empty());
        assert!(requests[1].system.is_none());
        assert_eq!(requests[1].messages.len(), 1);
        assert!(
            !requests[1].messages[0]
                .content
                .contains("unrelated conversation marker")
        );
        assert!(
            !requests[1].messages[0]
                .content
                .contains("unrelated system marker")
        );
        assert!(!requests[1].messages[0].content.contains("unrelated-tool"));
        assert!(
            requests[1].messages[0]
                .content
                .contains("private_fragment_marker")
        );
        let repair_prompt: serde_json::Value =
            serde_json::from_str(&requests[1].messages[0].content)
                .unwrap_or_else(|_| unreachable!());
        assert_eq!(repair_prompt.as_object().map(serde_json::Map::len), Some(3));
        assert_eq!(repair_prompt["schema"], schema);
        assert_eq!(
            repair_prompt["invalid_response"],
            r#"{"count":"private_fragment_marker"}"#
        );
        assert!(repair_prompt["validation_errors"].is_array());
        assert!(
            !repair_prompt["validation_errors"]
                .to_string()
                .contains("private_fragment_marker")
        );
        assert_eq!(response.model_id.as_str(), "gpt-oss-20b");
        assert_eq!(response.structured, Some(serde_json::json!({"count": 7})));
        assert_eq!(response.repair_attempts, 1);

        let primary = store
            .get_model_call_attempt(&requests[0].request_id)
            .unwrap_or_else(|_| unreachable!())
            .unwrap_or_else(|| unreachable!());
        let repair = store
            .get_model_call_attempt(&requests[1].request_id)
            .unwrap_or_else(|_| unreachable!())
            .unwrap_or_else(|| unreachable!());
        assert_eq!(primary.state, ModelAttemptState::Failed);
        assert_eq!(primary.response_blob, None);
        assert_eq!(repair.state, ModelAttemptState::Completed);
        assert_eq!(repair.relation_kind, ModelAttemptRelationKind::Repair);
        assert_eq!(
            repair.parent_request_id.as_ref(),
            Some(&requests[0].request_id)
        );

        let events = EventBus::replay(&store, None, None, 16).unwrap_or_else(|_| unreachable!());
        let kinds = events
            .items
            .iter()
            .filter_map(|item| match item {
                ReplayItem::Event { event } => Some(event.kind),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(
            kinds,
            [
                EventKind::ModelCalled,
                EventKind::ModelCompleted,
                EventKind::ModelOutputInvalid,
                EventKind::ModelCalled,
                EventKind::ModelCompleted,
                EventKind::ModelRepaired,
            ]
        );
        let repair_completed = match &events.items[4] {
            ReplayItem::Event { event } => event,
            _ => unreachable!(),
        };
        assert_eq!(repair_completed.payload["repair_attempts"], 1);
        let repaired_event = match &events.items[5] {
            ReplayItem::Event { event } => event,
            _ => unreachable!(),
        };
        assert_eq!(repaired_event.payload["relation_kind"], "REPAIR");
        assert_eq!(repaired_event.payload["repair_attempts"], 1);
        let event_json = serde_json::to_string(&events.items).unwrap_or_else(|_| unreachable!());
        assert!(!event_json.contains("private_fragment_marker"));
        assert!(!event_json.contains("unrelated conversation marker"));

        provider.push_script(RepairScript::Respond(r#"{"count":"first-invalid"}"#.into()));
        provider.push_script(RepairScript::Fail(ModelError {
            kind: ModelErrorCode::new("UPSTREAM_UNAVAILABLE").unwrap_or_else(|_| unreachable!()),
            message: serea_protocol::ErrorMessage::new("private provider diagnostic")
                .unwrap_or_else(|_| unreachable!()),
            retryable: false,
        }));
        provider.push_script(RepairScript::Respond(r#"{"count":9}"#.into()));
        let next_session = block_on(router.route(&call)).unwrap_or_else(|_| unreachable!());
        let repaired_after_failure =
            block_on(router.dispatch_structured_with_repair(&call, &next_session, &context))
                .unwrap_or_else(|_| unreachable!());
        assert_eq!(repaired_after_failure.repair_attempts, 2);
        assert_eq!(
            repaired_after_failure.structured,
            Some(serde_json::json!({"count": 9}))
        );
        assert_eq!(provider.health_calls.load(Ordering::SeqCst), 4);
        let requests = provider.requests();
        assert_eq!(requests.len(), 5);
        let repair_one = store
            .get_model_call_attempt(&requests[3].request_id)
            .unwrap_or_else(|_| unreachable!())
            .unwrap_or_else(|| unreachable!());
        let repair_two = store
            .get_model_call_attempt(&requests[4].request_id)
            .unwrap_or_else(|_| unreachable!())
            .unwrap_or_else(|| unreachable!());
        assert_eq!(repair_one.state, ModelAttemptState::Failed);
        assert_eq!(repair_one.response_blob, None);
        assert_eq!(repair_one.relation_kind, ModelAttemptRelationKind::Repair);
        assert_eq!(
            repair_one.parent_request_id.as_ref(),
            Some(&requests[2].request_id)
        );
        assert_eq!(repair_two.state, ModelAttemptState::Completed);
        assert_eq!(repair_two.relation_kind, ModelAttemptRelationKind::Repair);
        assert_eq!(
            repair_two.parent_request_id.as_ref(),
            Some(&requests[3].request_id)
        );
        let final_usage = store
            .model_usage_for_request(&requests[4].request_id)
            .unwrap_or_else(|_| unreachable!())
            .unwrap_or_else(|| unreachable!());
        assert_eq!(final_usage.repair_attempts, 2);

        for invalid in [
            r#"{"count":"primary-invalid"}"#,
            r#"{"count":"repair-one-invalid"}"#,
            r#"{"count":"repair-two-invalid"}"#,
        ] {
            provider.push_script(RepairScript::Respond(invalid.into()));
        }
        let exhausted_session = block_on(router.route(&call)).unwrap_or_else(|_| unreachable!());
        let exhausted =
            block_on(router.dispatch_structured_with_repair(&call, &exhausted_session, &context));
        assert!(matches!(
            exhausted,
            Err(ModelDispatchFailure::StructuredOutputInvalid { .. })
        ));
        assert_eq!(provider.health_calls.load(Ordering::SeqCst), 6);
        let requests = provider.requests();
        assert_eq!(requests.len(), 8);
        let first_repair = store
            .get_model_call_attempt(&requests[6].request_id)
            .unwrap_or_else(|_| unreachable!())
            .unwrap_or_else(|| unreachable!());
        let second_repair = store
            .get_model_call_attempt(&requests[7].request_id)
            .unwrap_or_else(|_| unreachable!())
            .unwrap_or_else(|| unreachable!());
        assert_eq!(
            first_repair.parent_request_id.as_ref(),
            Some(&requests[5].request_id)
        );
        assert_eq!(
            second_repair.parent_request_id.as_ref(),
            Some(&requests[6].request_id)
        );
        let events = EventBus::replay(&store, None, None, 32).unwrap_or_else(|_| unreachable!());
        assert_eq!(
            events
                .items
                .iter()
                .filter(|item| matches!(item, ReplayItem::Event { event } if event.kind == EventKind::ModelRepaired))
                .count(),
            2
        );

        provider.push_script(RepairScript::Respond(
            r#"{"count":"invalid-with-degraded-repair-model"}"#.into(),
        ));
        provider.push_health(ProviderHealth::Ready);
        provider.push_health(ProviderHealth::Degraded);
        let degraded_session = block_on(router.route(&call)).unwrap_or_else(|_| unreachable!());
        let degraded =
            block_on(router.dispatch_structured_with_repair(&call, &degraded_session, &context));
        assert!(matches!(
            degraded,
            Err(ModelDispatchFailure::StructuredOutputInvalid { .. })
        ));
        assert_eq!(provider.requests().len(), 9);
        assert_eq!(provider.health_calls.load(Ordering::SeqCst), 8);

        provider.push_script(RepairScript::Respond(
            r#"{"count":"invalid-before-cancellation"}"#.into(),
        ));
        let cancellation_session = block_on(router.route(&call)).unwrap_or_else(|_| unreachable!());
        let cancellation_gate = CancelBeforeRepairGate(AtomicUsize::new(0));
        let cancellation_context = ModelDispatchContext {
            store: &store,
            events: &bus,
            price: context.price.clone(),
            prices: None,
            max_daily_spend_usd_micros: context.max_daily_spend_usd_micros,
            clock: &FixedClock,
            gate: &cancellation_gate,
        };
        let cancelled = block_on(router.dispatch_structured_with_repair(
            &call,
            &cancellation_session,
            &cancellation_context,
        ));
        assert!(matches!(
            cancelled,
            Err(ModelDispatchFailure::Refused(RouterError::TaskCancelled))
        ));
        assert_eq!(provider.requests().len(), 10);
        assert_eq!(cancellation_gate.0.load(Ordering::SeqCst), 3);
        let events = EventBus::replay(&store, None, None, 64).unwrap_or_else(|_| unreachable!());
        assert_eq!(
            events
                .items
                .iter()
                .filter(|item| matches!(item, ReplayItem::Event { event } if event.kind == EventKind::ModelCalled))
                .count(),
            10
        );

        let oversized = format!("\"{}\"", "x".repeat(MAX_MODEL_RESPONSE_BYTES));
        provider.push_script(RepairScript::Respond(oversized));
        let oversized_session = block_on(router.route(&call)).unwrap_or_else(|_| unreachable!());
        let oversized_result =
            block_on(router.dispatch_structured_with_repair(&call, &oversized_session, &context));
        assert!(matches!(
            oversized_result,
            Err(ModelDispatchFailure::StructuredOutputInvalid {
                validation_error: StructuredValidationError::ResponseTooLarge,
                ..
            })
        ));
        assert_eq!(provider.requests().len(), 11);

        provider.push_script(RepairScript::Respond(
            r#"{"count":"invalid-before-ambiguous-repair"}"#.into(),
        ));
        provider.push_script(RepairScript::Fail(ModelError {
            kind: ModelErrorCode::new(AMBIGUOUS_PROVIDER_ERROR_KIND)
                .unwrap_or_else(|_| unreachable!()),
            message: serea_protocol::ErrorMessage::new("outcome may have been processed")
                .unwrap_or_else(|_| unreachable!()),
            retryable: true,
        }));
        let ambiguous_session = block_on(router.route(&call)).unwrap_or_else(|_| unreachable!());
        let ambiguous =
            block_on(router.dispatch_structured_with_repair(&call, &ambiguous_session, &context));
        let requests = provider.requests();
        assert_eq!(requests.len(), 13);
        let ambiguous_id = requests[12].request_id.clone();
        assert!(matches!(
            ambiguous,
            Err(ModelDispatchFailure::AmbiguousProvider { request_id, .. })
                if request_id == ambiguous_id
        ));
        let ambiguous_attempt = store
            .get_model_call_attempt(&ambiguous_id)
            .unwrap_or_else(|_| unreachable!())
            .unwrap_or_else(|| unreachable!());
        assert_eq!(ambiguous_attempt.state, ModelAttemptState::Ambiguous);

        provider.push_script(RepairScript::Respond(
            r#"{"count":"invalid-before-two-failures"}"#.into(),
        ));
        for _ in 0..2 {
            provider.push_script(RepairScript::Fail(ModelError {
                kind: ModelErrorCode::new("UPSTREAM_UNAVAILABLE")
                    .unwrap_or_else(|_| unreachable!()),
                message: serea_protocol::ErrorMessage::new("private provider diagnostic")
                    .unwrap_or_else(|_| unreachable!()),
                retryable: false,
            }));
        }
        let failed_session = block_on(router.route(&call)).unwrap_or_else(|_| unreachable!());
        let failed_twice =
            block_on(router.dispatch_structured_with_repair(&call, &failed_session, &context));
        assert!(matches!(
            failed_twice,
            Err(ModelDispatchFailure::StructuredOutputInvalid { .. })
        ));
        assert_eq!(provider.requests().len(), 16);
        let all_events =
            EventBus::replay(&store, None, None, 64).unwrap_or_else(|_| unreachable!());
        let all_event_json =
            serde_json::to_string(&all_events.items).unwrap_or_else(|_| unreachable!());
        assert!(!all_event_json.contains("private provider diagnostic"));
        assert!(!all_event_json.contains("outcome may have been processed"));

        provider.push_script(RepairScript::Respond(
            r#"{"count":"provider-says-invalid"}"#.into(),
        ));
        provider.push_finish(FinishReason::StructureInvalid);
        provider.push_script(RepairScript::Respond(r#"{"count":11}"#.into()));
        let structure_invalid_session =
            block_on(router.route(&call)).unwrap_or_else(|_| unreachable!());
        let structure_invalid = block_on(router.dispatch_structured_with_repair(
            &call,
            &structure_invalid_session,
            &context,
        ))
        .unwrap_or_else(|_| unreachable!());
        assert_eq!(
            structure_invalid.structured,
            Some(serde_json::json!({"count": 11}))
        );
        assert_eq!(structure_invalid.repair_attempts, 1);
        let requests = provider.requests();
        assert_eq!(requests.len(), 18);
        assert_eq!(requests[16].model_id.as_str(), "nemotron-3-nano-30b");
        assert_eq!(requests[17].model_id.as_str(), "gpt-oss-20b");

        let accepted_repair_request_id = provider.requests()[1].request_id.clone();
        drop(cancellation_context);
        drop(context);
        drop(router);
        drop(provider);
        drop(store);
        let reopened = Store::open(&db_path, &FixedClock).unwrap_or_else(|_| unreachable!());
        let recovered =
            recover_completed_structured_response(&reopened, &accepted_repair_request_id)
                .unwrap_or_else(|_| unreachable!())
                .unwrap_or_else(|| unreachable!());
        assert_eq!(recovered, response);
        drop(reopened);
        let _ = std::fs::remove_file(&db_path);
        let _ = std::fs::remove_file(db_path.with_extension("sqlite-wal"));
        let _ = std::fs::remove_file(db_path.with_extension("sqlite-shm"));
    }

    #[test]
    fn startup_recovery_marks_intents_ambiguous_with_one_atomic_event() {
        let db_path = std::env::temp_dir().join(format!(
            "serea-router-recovery-{}-{}.sqlite",
            std::process::id(),
            NEXT_RECOVERY_TEST_DB.fetch_add(1, Ordering::SeqCst)
        ));
        let store = Store::open(&db_path, &FixedClock).unwrap_or_else(|_| unreachable!());
        let bus = EventBus::new(IncrementingIds(500));
        let request_id = bus
            .mint_model_request_id()
            .unwrap_or_else(|_| unreachable!());
        let price = ModelPriceSnapshot::new(CostClass::Paid, "price-1", 1_000_000, 1_000_000);
        store
            .reserve_model_call(
                ModelCallAttemptDraft {
                    request_id: request_id.clone(),
                    task_id: None,
                    purpose: ModelPurpose::Chat,
                    model_id: ModelId::new("nemotron-3-nano-30b")
                        .unwrap_or_else(|_| unreachable!()),
                    provider_id: ProviderId::new("provider").unwrap_or_else(|_| unreachable!()),
                    deployment_class: StorageDeploymentClass::Local,
                    data_class: DataClass::Public,
                    relation_kind: ModelAttemptRelationKind::None,
                    parent_request_id: None,
                    fallback_from_model_id: None,
                    price,
                    max_context_tokens: 1000,
                    effective_max_output_tokens: 100,
                    dispatch_intent_at: FixedClock.now_ms().unwrap_or_else(|_| unreachable!()),
                },
                UsdMicros::new(10_000_000).unwrap_or_else(|_| unreachable!()),
            )
            .unwrap_or_else(|_| unreachable!());
        let reserved = store
            .get_model_call_attempt(&request_id)
            .unwrap_or_else(|_| unreachable!())
            .unwrap_or_else(|| unreachable!())
            .reserved_cost_usd_micros;

        assert_eq!(
            recover_unresolved_model_calls(&store, &bus, &FixedClock)
                .unwrap_or_else(|_| unreachable!()),
            1
        );
        drop(store);
        let reopened = Store::open(&db_path, &FixedClock).unwrap_or_else(|_| unreachable!());
        let recovered = reopened
            .get_model_call_attempt(&request_id)
            .unwrap_or_else(|_| unreachable!())
            .unwrap_or_else(|| unreachable!());
        assert_eq!(recovered.state, ModelAttemptState::Ambiguous);
        assert_eq!(recovered.reserved_cost_usd_micros, reserved);
        assert_eq!(recovered.actual_cost_usd_micros, None);
        assert!(
            reopened
                .model_usage_for_request(&request_id)
                .unwrap_or_else(|_| unreachable!())
                .is_none()
        );
        assert_eq!(
            recover_unresolved_model_calls(&reopened, &bus, &FixedClock)
                .unwrap_or_else(|_| unreachable!()),
            0
        );
        let events = EventBus::replay(&reopened, None, None, 16).unwrap_or_else(|_| unreachable!());
        assert!(
            matches!(events.items.as_slice(), [ReplayItem::Event { event }] if event.kind == EventKind::ModelFailed
            && event.payload["error_kind"] == "AMBIGUOUS_DISPATCH"
            && event.payload["retryable"] == false)
        );
        drop(reopened);
        let _ = std::fs::remove_file(&db_path);
        let _ = std::fs::remove_file(db_path.with_extension("sqlite-wal"));
        let _ = std::fs::remove_file(db_path.with_extension("sqlite-shm"));
    }

    static NEXT_RECOVERY_TEST_DB: AtomicUsize = AtomicUsize::new(0);

    #[test]
    fn chains_are_frozen_and_codex_is_absent() {
        for purpose in [
            ModelPurpose::Chat,
            ModelPurpose::Planning,
            ModelPurpose::Extraction,
            ModelPurpose::Analysis,
            ModelPurpose::Proactive,
        ] {
            assert_eq!(
                preference_chain(purpose),
                &["nemotron-3-nano-30b", "gpt-oss-20b"]
            );
        }
        assert_eq!(
            preference_chain(ModelPurpose::StructuredRepair),
            &["gpt-oss-20b"]
        );
        assert!(
            preference_chain(ModelPurpose::Chat)
                .iter()
                .all(|id| *id != "codex")
        );
    }

    #[test]
    fn effective_capabilities_never_widen_provider_or_host() {
        let mut host = caps(false, JsonSchemaMode::BestEffort, 80, 20);
        host.tools = false;
        host.structured_output = false;
        let provider = caps(true, JsonSchemaMode::Strict, 100, 40);
        let result = intersect(host, provider);
        assert!(!result.vision);
        assert!(!result.tools);
        assert!(!result.structured_output);
        assert_eq!(result.json_schema_mode, JsonSchemaMode::BestEffort);
        assert_eq!(result.max_context_tokens, 80);
        assert_eq!(result.max_output_tokens, 20);
    }

    #[test]
    fn dispatch_gate_refuses_cancelled_expired_and_revoked_calls_before_intent() {
        let call = PreparedModelCallV1::from_host(PreparedModelCallDraftV1 {
            task_id: None,
            purpose: ModelPurpose::Chat,
            messages: Vec::new(),
            system: None,
            response_format: ResponseFormat::Text,
            tools: Vec::new(),
            max_output_tokens: 8,
            temperature: 0.0,
            deadline_ms: 100,
            data_class: DataClass::Personal,
            requirements: ModelRoutingRequirementsV1 {
                vision_required: false,
                tools_required: false,
                min_context_tokens: 1,
                min_output_tokens: 1,
                structured_requirement: StructuredRequirementV1::Any,
            },
            egress: ModelEgressPolicySnapshotV1::from_host(true),
            host_max_output_tokens: 2048,
        })
        .unwrap_or_else(|_| unreachable!());

        let cancelled = ModelDispatchGateSnapshotV1::from_host(
            true,
            ModelEgressPolicySnapshotV1::from_host(true),
            100,
        );
        assert_eq!(
            resolve_dispatch_gate(&call, cancelled, ModelDeploymentClass::Cloud),
            Err(RouterError::TaskCancelled)
        );

        let expired = ModelDispatchGateSnapshotV1::from_host(
            false,
            ModelEgressPolicySnapshotV1::from_host(true),
            0,
        );
        assert_eq!(
            resolve_dispatch_gate(&call, expired, ModelDeploymentClass::Cloud),
            Err(RouterError::DeadlineExpired)
        );

        let revoked = ModelDispatchGateSnapshotV1::from_host(
            false,
            ModelEgressPolicySnapshotV1::from_host(false),
            50,
        );
        assert_eq!(
            resolve_dispatch_gate(&call, revoked, ModelDeploymentClass::Cloud),
            Err(RouterError::EgressRevoked)
        );
    }

    fn caps(
        vision: bool,
        json_schema_mode: JsonSchemaMode,
        max_context_tokens: u32,
        max_output_tokens: u32,
    ) -> ModelCapabilities {
        ModelCapabilities {
            vision,
            tools: true,
            structured_output: true,
            json_schema_mode,
            thinking: false,
            long_context: false,
            fast: false,
            code_specialist: false,
            max_context_tokens,
            max_output_tokens,
            supports_streaming: false,
            supports_seeds: false,
        }
    }

    #[test]
    fn retryable_primary_failure_uses_frozen_health_snapshot_and_fallback() {
        let db_path = std::env::temp_dir().join(format!(
            "serea-router-fallback-red-{}-{}.sqlite",
            std::process::id(),
            NEXT_RECOVERY_TEST_DB.fetch_add(1, Ordering::SeqCst)
        ));
        let store = Store::open(&db_path, &FixedClock).unwrap_or_else(|_| unreachable!());
        let provider = Arc::new(RepairFakeProvider::new(Vec::new()));
        provider.push_script(RepairScript::Fail(ModelError {
            kind: ModelErrorCode::new("UPSTREAM_UNAVAILABLE").unwrap_or_else(|_| unreachable!()),
            message: serea_protocol::ErrorMessage::new("private diagnostic")
                .unwrap_or_else(|_| unreachable!()),
            retryable: true,
        }));
        provider.push_script(RepairScript::Respond("fallback answer".into()));
        provider.push_health(ProviderHealth::Ready);
        provider.push_health(ProviderHealth::Degraded);
        let provider_id = provider.provider_id();
        let capabilities = caps(false, JsonSchemaMode::Strict, 1000, 1000);
        let roster = ModelRosterV1::new(
            ["nemotron-3-nano-30b", "gpt-oss-20b"]
                .into_iter()
                .map(|id| {
                    ModelRosterEntryV1::new(
                        ModelId::new(id).unwrap_or_else(|_| unreachable!()),
                        provider_id.clone(),
                        ModelDeploymentClass::Local,
                        true,
                        capabilities,
                        CostClass::Paid,
                    )
                    .unwrap_or_else(|_| unreachable!())
                })
                .collect(),
        )
        .unwrap_or_else(|_| unreachable!());
        let router =
            ModelRouterV1::new(roster, vec![provider.clone()]).unwrap_or_else(|_| unreachable!());
        let call = PreparedModelCallV1::from_host(PreparedModelCallDraftV1 {
            task_id: None,
            purpose: ModelPurpose::Chat,
            messages: vec![ModelMessage {
                role: serea_protocol::MessageRole::new("user").unwrap_or_else(|_| unreachable!()),
                content: "hello".into(),
            }],
            system: None,
            response_format: ResponseFormat::Text,
            tools: Vec::new(),
            max_output_tokens: 32,
            temperature: 0.0,
            deadline_ms: 1000,
            data_class: DataClass::Public,
            requirements: ModelRoutingRequirementsV1 {
                vision_required: false,
                tools_required: false,
                min_context_tokens: 1,
                min_output_tokens: 1,
                structured_requirement: StructuredRequirementV1::Any,
            },
            egress: ModelEgressPolicySnapshotV1::from_host(true),
            host_max_output_tokens: 2048,
        })
        .unwrap_or_else(|_| unreachable!());
        let bus = EventBus::new(IncrementingIds(0));
        let context = dispatch_context(
            &store,
            &bus,
            ModelPriceSnapshot::new(CostClass::Paid, "price-1", 1_000_000, 1_000_000),
            UsdMicros::new(10_000_000).unwrap_or_else(|_| unreachable!()),
        );
        let session = block_on(router.route(&call)).unwrap_or_else(|_| unreachable!());

        let response = block_on(router.dispatch_chat_text_with_fallback(&call, &session, &context))
            .unwrap_or_else(|_| unreachable!());
        assert_eq!(response.content, "fallback answer");

        let requests = provider.requests();
        assert_eq!(requests.len(), 2);
        let accepted_fallback_request_id = requests[1].request_id.clone();
        assert_eq!(requests[0].model_id.as_str(), "nemotron-3-nano-30b");
        assert_eq!(requests[1].model_id.as_str(), "gpt-oss-20b");
        assert_ne!(requests[0].request_id, requests[1].request_id);
        assert_eq!(provider.health_calls.load(Ordering::SeqCst), 1);
        let primary = store
            .get_model_call_attempt(&requests[0].request_id)
            .unwrap_or_else(|_| unreachable!())
            .unwrap_or_else(|| unreachable!());
        let fallback = store
            .get_model_call_attempt(&requests[1].request_id)
            .unwrap_or_else(|_| unreachable!())
            .unwrap_or_else(|| unreachable!());
        assert_eq!(primary.state, ModelAttemptState::Failed);
        assert_eq!(fallback.state, ModelAttemptState::Completed);
        assert_eq!(fallback.relation_kind, ModelAttemptRelationKind::Fallback);
        assert_eq!(fallback.parent_request_id, Some(primary.request_id.clone()));
        assert_eq!(
            fallback.fallback_from_model_id,
            Some(primary.model_id.clone())
        );
        let events = EventBus::replay(&store, None, None, 16).unwrap_or_else(|_| unreachable!());
        let kinds = events
            .items
            .iter()
            .filter_map(|item| match item {
                ReplayItem::Event { event } => Some(event.kind),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(
            kinds,
            vec![
                EventKind::ModelCalled,
                EventKind::ModelFailed,
                EventKind::ModelFallback,
                EventKind::ModelCalled,
                EventKind::ModelCompleted,
            ]
        );
        provider.push_health(ProviderHealth::Ready);
        provider.push_script(RepairScript::Fail(ModelError {
            kind: ModelErrorCode::new("UPSTREAM_UNAVAILABLE").unwrap_or_else(|_| unreachable!()),
            message: serea_protocol::ErrorMessage::new("private diagnostic two")
                .unwrap_or_else(|_| unreachable!()),
            retryable: true,
        }));
        provider.push_script(RepairScript::Fail(ModelError {
            kind: ModelErrorCode::new("UPSTREAM_UNAVAILABLE").unwrap_or_else(|_| unreachable!()),
            message: serea_protocol::ErrorMessage::new("private diagnostic three")
                .unwrap_or_else(|_| unreachable!()),
            retryable: false,
        }));
        let _degraded_session = block_on(router.route(&call)).unwrap_or_else(|_| unreachable!());
        let second_session = block_on(router.route(&call)).unwrap_or_else(|_| unreachable!());
        assert!(
            block_on(router.dispatch_chat_text_with_fallback(&call, &second_session, &context))
                .is_err()
        );
        assert_eq!(provider.requests().len(), 4);
        assert_eq!(provider.health_calls.load(Ordering::SeqCst), 3);
        let events = EventBus::replay(&store, None, None, 16).unwrap_or_else(|_| unreachable!());
        let kinds = events
            .items
            .iter()
            .filter_map(|item| match item {
                ReplayItem::Event { event } => Some(event.kind),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert!(kinds.ends_with(&[
            EventKind::ModelCalled,
            EventKind::ModelFailed,
            EventKind::ModelFallback,
            EventKind::ModelCalled,
            EventKind::ModelFailed,
            EventKind::ModelFallbackExhausted,
        ]));
        provider.push_script(RepairScript::Fail(ModelError {
            kind: ModelErrorCode::new("UPSTREAM_UNAVAILABLE").unwrap_or_else(|_| unreachable!()),
            message: serea_protocol::ErrorMessage::new("nonretryable diagnostic")
                .unwrap_or_else(|_| unreachable!()),
            retryable: false,
        }));
        let third_session = block_on(router.route(&call)).unwrap_or_else(|_| unreachable!());
        assert!(
            block_on(router.dispatch_chat_text_with_fallback(&call, &third_session, &context))
                .is_err()
        );
        assert_eq!(provider.requests().len(), 5);
        provider.push_script(RepairScript::Fail(ModelError {
            kind: ModelErrorCode::new(AMBIGUOUS_PROVIDER_ERROR_KIND)
                .unwrap_or_else(|_| unreachable!()),
            message: serea_protocol::ErrorMessage::new("ambiguous diagnostic")
                .unwrap_or_else(|_| unreachable!()),
            retryable: true,
        }));
        let fourth_session = block_on(router.route(&call)).unwrap_or_else(|_| unreachable!());
        assert!(
            block_on(router.dispatch_chat_text_with_fallback(&call, &fourth_session, &context))
                .is_err()
        );
        assert_eq!(provider.requests().len(), 6);
        assert_eq!(provider.health_calls.load(Ordering::SeqCst), 5);
        assert!(
            !serde_json::to_string(&events.items)
                .unwrap_or_else(|_| unreachable!())
                .contains("private diagnostic")
        );
        assert!(
            !serde_json::to_string(&events.items)
                .unwrap_or_else(|_| unreachable!())
                .contains("hello")
        );
        drop(context);
        drop(router);
        drop(provider);
        drop(store);
        let reopened = Store::open(&db_path, &FixedClock).unwrap_or_else(|_| unreachable!());
        let recovered =
            recover_completed_chat_text_response(&reopened, &accepted_fallback_request_id)
                .unwrap_or_else(|_| unreachable!())
                .unwrap_or_else(|| unreachable!());
        assert_eq!(recovered.content, "fallback answer");
        assert_eq!(recovered.request_id, accepted_fallback_request_id);
        drop(reopened);
        let _ = std::fs::remove_file(&db_path);
        let _ = std::fs::remove_file(db_path.with_extension("sqlite-wal"));
        let _ = std::fs::remove_file(db_path.with_extension("sqlite-shm"));
    }

    #[test]
    fn structured_fallback_invalid_output_repairs_from_fallback_lineage() {
        let db_path = std::env::temp_dir().join(format!(
            "serea-router-structured-fallback-red-{}-{}.sqlite",
            std::process::id(),
            NEXT_RECOVERY_TEST_DB.fetch_add(1, Ordering::SeqCst)
        ));
        let store = Arc::new(Store::open(&db_path, &FixedClock).unwrap_or_else(|_| unreachable!()));
        let provider = Arc::new(RepairFakeProvider::new(Vec::new()));
        provider.push_script(RepairScript::Fail(ModelError {
            kind: ModelErrorCode::new("UPSTREAM_UNAVAILABLE").unwrap_or_else(|_| unreachable!()),
            message: serea_protocol::ErrorMessage::new("private primary diagnostic")
                .unwrap_or_else(|_| unreachable!()),
            retryable: true,
        }));
        provider.push_script(RepairScript::Respond(
            r#"{"count":"fallback_invalid_marker"}"#.into(),
        ));
        provider.push_script(RepairScript::Respond(r#"{"count":7}"#.into()));
        let provider_id = provider.provider_id();
        let capabilities = caps(false, JsonSchemaMode::Strict, 1000, 1000);
        let roster = ModelRosterV1::new(
            ["nemotron-3-nano-30b", "gpt-oss-20b"]
                .into_iter()
                .map(|id| {
                    ModelRosterEntryV1::new(
                        ModelId::new(id).unwrap_or_else(|_| unreachable!()),
                        provider_id.clone(),
                        ModelDeploymentClass::Local,
                        true,
                        capabilities,
                        CostClass::Paid,
                    )
                    .unwrap_or_else(|_| unreachable!())
                })
                .collect(),
        )
        .unwrap_or_else(|_| unreachable!());
        let router =
            ModelRouterV1::new(roster, vec![provider.clone()]).unwrap_or_else(|_| unreachable!());
        let schema = serde_json::json!({
            "type": "object",
            "properties": {"count": {"type": "integer"}},
            "required": ["count"],
            "additionalProperties": false
        });
        let call = PreparedModelCallV1::from_host(PreparedModelCallDraftV1 {
            task_id: None,
            purpose: ModelPurpose::Analysis,
            messages: vec![ModelMessage {
                role: serea_protocol::MessageRole::new("user").unwrap_or_else(|_| unreachable!()),
                content: "conversation marker".into(),
            }],
            system: Some("system marker".into()),
            response_format: ResponseFormat::JsonSchema {
                schema: schema.clone(),
            },
            tools: vec![serde_json::json!({"name":"unrelated-tool"})],
            max_output_tokens: 128,
            temperature: 0.25,
            deadline_ms: 1000,
            data_class: DataClass::Public,
            requirements: ModelRoutingRequirementsV1 {
                vision_required: false,
                tools_required: false,
                min_context_tokens: 1,
                min_output_tokens: 1,
                structured_requirement: StructuredRequirementV1::Any,
            },
            egress: ModelEgressPolicySnapshotV1::from_host(false),
            host_max_output_tokens: 2048,
        })
        .unwrap_or_else(|_| unreachable!());
        let bus = EventBus::new(IncrementingIds(0));
        let context = dispatch_context(
            &store,
            &bus,
            ModelPriceSnapshot::new(CostClass::Paid, "price-1", 1_000_000, 1_000_000),
            UsdMicros::new(10_000_000).unwrap_or_else(|_| unreachable!()),
        );
        let session = block_on(router.route(&call)).unwrap_or_else(|_| unreachable!());

        let response = block_on(
            router.dispatch_structured_with_fallback_and_repair(&call, &session, &context),
        )
        .unwrap_or_else(|_| unreachable!());

        assert_eq!(response.structured, Some(serde_json::json!({"count":7})));
        assert_eq!(response.repair_attempts, 1);
        assert_eq!(provider.health_calls.load(Ordering::SeqCst), 2);
        let requests = provider.requests();
        assert_eq!(requests.len(), 3);
        assert_eq!(requests[0].model_id.as_str(), "nemotron-3-nano-30b");
        assert_eq!(requests[1].model_id.as_str(), "gpt-oss-20b");
        assert_eq!(requests[2].model_id.as_str(), "gpt-oss-20b");
        assert_eq!(requests[2].purpose, ModelPurpose::StructuredRepair);
        assert!(requests[2].tools.is_empty());
        assert!(requests[2].system.is_none());
        assert!(
            requests[2].messages[0]
                .content
                .contains("fallback_invalid_marker")
        );
        assert!(
            !requests[2].messages[0]
                .content
                .contains("conversation marker")
        );
        assert!(!requests[2].messages[0].content.contains("system marker"));
        let primary = store
            .get_model_call_attempt(&requests[0].request_id)
            .unwrap_or_else(|_| unreachable!())
            .unwrap_or_else(|| unreachable!());
        let fallback = store
            .get_model_call_attempt(&requests[1].request_id)
            .unwrap_or_else(|_| unreachable!())
            .unwrap_or_else(|| unreachable!());
        let repair = store
            .get_model_call_attempt(&requests[2].request_id)
            .unwrap_or_else(|_| unreachable!())
            .unwrap_or_else(|| unreachable!());
        assert_eq!(primary.state, ModelAttemptState::Failed);
        assert_eq!(fallback.state, ModelAttemptState::Failed);
        assert_eq!(fallback.response_blob, None);
        assert!(
            store
                .model_usage_for_request(&fallback.request_id)
                .unwrap_or_else(|_| unreachable!())
                .is_some()
        );
        assert_eq!(fallback.relation_kind, ModelAttemptRelationKind::Fallback);
        assert_eq!(fallback.parent_request_id, Some(primary.request_id.clone()));
        assert_eq!(repair.relation_kind, ModelAttemptRelationKind::Repair);
        assert_eq!(repair.parent_request_id, Some(fallback.request_id.clone()));
        let events = EventBus::replay(&store, None, None, 32).unwrap_or_else(|_| unreachable!());
        let event_kinds = events
            .items
            .iter()
            .filter_map(|item| match item {
                ReplayItem::Event { event } => Some(event.kind),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(
            event_kinds,
            vec![
                EventKind::ModelCalled,
                EventKind::ModelFailed,
                EventKind::ModelFallback,
                EventKind::ModelCalled,
                EventKind::ModelCompleted,
                EventKind::ModelOutputInvalid,
                EventKind::ModelFallbackExhausted,
                EventKind::ModelCalled,
                EventKind::ModelCompleted,
                EventKind::ModelRepaired,
            ]
        );
        let event_json = serde_json::to_string(&events.items).unwrap_or_else(|_| unreachable!());
        assert!(!event_json.contains("fallback_invalid_marker"));
        assert!(!event_json.contains("private primary diagnostic"));
        let accepted_request_id = response.request_id.clone();
        drop(context);
        drop(router);
        drop(provider);
        drop(store);
        let reopened = Store::open(&db_path, &FixedClock).unwrap_or_else(|_| unreachable!());
        let recovered = recover_completed_structured_response(&reopened, &accepted_request_id)
            .unwrap_or_else(|_| unreachable!())
            .unwrap_or_else(|| unreachable!());
        assert_eq!(recovered, response);
        drop(reopened);
        let _ = std::fs::remove_file(&db_path);
        let _ = std::fs::remove_file(db_path.with_extension("sqlite-wal"));
        let _ = std::fs::remove_file(db_path.with_extension("sqlite-shm"));
    }

    #[test]
    fn task_call_budget_emits_model_budget_exhausted_without_dispatch() {
        let store = Arc::new(Store::open_in_memory(&FixedClock).unwrap_or_else(|_| unreachable!()));
        let task_id =
            TaskId::new("tsk_00000000000000000000000043").unwrap_or_else(|_| unreachable!());
        assert!(insert_budget_task(&store, task_id.clone(), "43"));
        for sequence in 1..=12u8 {
            let request_id = serea_protocol::RequestId::new(format!(
                "req_000000000000000000000000{sequence:02}"
            ))
            .unwrap_or_else(|_| unreachable!());
            store
                .reserve_model_call(
                    ModelCallAttemptDraft {
                        request_id: request_id.clone(),
                        task_id: Some(task_id.clone()),
                        purpose: ModelPurpose::Chat,
                        model_id: ModelId::new("nemotron-3-nano-30b")
                            .unwrap_or_else(|_| unreachable!()),
                        provider_id: ProviderId::new("provider").unwrap_or_else(|_| unreachable!()),
                        deployment_class: StorageDeploymentClass::Local,
                        data_class: DataClass::Public,
                        relation_kind: ModelAttemptRelationKind::None,
                        parent_request_id: None,
                        fallback_from_model_id: None,
                        price: ModelPriceSnapshot::new(CostClass::Free, "free-v1", 0, 0),
                        max_context_tokens: 20_000,
                        effective_max_output_tokens: 2_048,
                        dispatch_intent_at: EpochMillis::new(2).unwrap_or_else(|_| unreachable!()),
                    },
                    UsdMicros::new(0).unwrap_or_else(|_| unreachable!()),
                )
                .unwrap_or_else(|_| unreachable!());
            store
                .fail_model_call(
                    &request_id,
                    "FIXTURE_FAILURE",
                    EpochMillis::new(3).unwrap_or_else(|_| unreachable!()),
                )
                .unwrap_or_else(|_| unreachable!());
        }
        let provider = Arc::new(RepairFakeProvider::new(vec!["must not dispatch".into()]));
        let provider_id = provider.provider_id();
        let capabilities = caps(false, JsonSchemaMode::Strict, 20_000, 2_048);
        let roster = ModelRosterV1::new(vec![
            ModelRosterEntryV1::new(
                ModelId::new("nemotron-3-nano-30b").unwrap_or_else(|_| unreachable!()),
                provider_id,
                ModelDeploymentClass::Local,
                true,
                capabilities,
                CostClass::Free,
            )
            .unwrap_or_else(|_| unreachable!()),
        ])
        .unwrap_or_else(|_| unreachable!());
        let router =
            ModelRouterV1::new(roster, vec![provider.clone()]).unwrap_or_else(|_| unreachable!());
        let call = PreparedModelCallV1::from_host(PreparedModelCallDraftV1 {
            task_id: Some(task_id.clone()),
            purpose: ModelPurpose::Chat,
            messages: vec![ModelMessage {
                role: serea_protocol::MessageRole::new("user").unwrap_or_else(|_| unreachable!()),
                content: "budget check".into(),
            }],
            system: None,
            response_format: ResponseFormat::Text,
            tools: Vec::new(),
            max_output_tokens: 32,
            temperature: 0.0,
            deadline_ms: 1000,
            data_class: DataClass::Public,
            requirements: ModelRoutingRequirementsV1 {
                vision_required: false,
                tools_required: false,
                min_context_tokens: 1,
                min_output_tokens: 1,
                structured_requirement: StructuredRequirementV1::Any,
            },
            egress: ModelEgressPolicySnapshotV1::from_host(true),
            host_max_output_tokens: 2048,
        })
        .unwrap_or_else(|_| unreachable!());
        let bus = EventBus::new(IncrementingIds(0));
        let context = dispatch_context(
            &store,
            &bus,
            ModelPriceSnapshot::new(CostClass::Free, "free-v1", 0, 0),
            UsdMicros::new(0).unwrap_or_else(|_| unreachable!()),
        );
        let session = block_on(router.route(&call)).unwrap_or_else(|_| unreachable!());
        assert!(matches!(
            block_on(router.dispatch_chat_text(&call, &session, &context)),
            Err(ModelDispatchFailure::ModelCallBudgetExceeded {
                limit: 12,
                observed: 12,
            })
        ));
        assert!(provider.requests().is_empty());
        let events = EventBus::replay(&store, None, None, 16).unwrap_or_else(|_| unreachable!());
        assert!(matches!(events.items.as_slice(), [
            ReplayItem::Event { event }
        ] if event.kind == EventKind::ModelBudgetExhausted
            && event.payload["limit"] == 12
            && event.payload["observed"] == 12));
    }

    #[test]
    fn task_token_bound_returns_after_persisting_the_threshold_crossing_response() {
        let db_path = std::env::temp_dir().join(format!(
            "serea-router-token-bound-red-{}-{}.sqlite",
            std::process::id(),
            NEXT_RECOVERY_TEST_DB.fetch_add(1, Ordering::SeqCst)
        ));
        let store = Arc::new(Store::open(&db_path, &FixedClock).unwrap_or_else(|_| unreachable!()));
        let task_id =
            TaskId::new("tsk_00000000000000000000000042").unwrap_or_else(|_| unreachable!());
        let actor = ActorId::new("budget-test").unwrap_or_else(|_| unreachable!());
        let version = serea_protocol::SemVer::new("1.0.0").unwrap_or_else(|_| unreachable!());
        let cause =
            EventId::new("evt_00000000000000000000000042").unwrap_or_else(|_| unreachable!());
        let transition = TransitionContext {
            actor_kind: ActorKind::Host,
            actor_id: &actor,
            actor_version: &version,
            causation_id: Some(&cause),
        };
        let created =
            Timestamp::from_epoch_millis(EpochMillis::new(1).unwrap_or_else(|_| unreachable!()));
        let task = AssistantTask {
            task_id: task_id.clone(),
            kind: TaskKind::UserRequest,
            title: TaskTitle::new("token budget fixture").unwrap_or_else(|_| unreachable!()),
            state: TaskState::Received,
            origin: TaskOrigin {
                kind: TaskOriginKind::new("USER_MESSAGE").unwrap_or_else(|_| unreachable!()),
                device_id: None,
                message_id: None,
                extensions: Default::default(),
            },
            data_class: DataClass::Public,
            policy_class: RiskClass::Communication,
            created_at: created.clone(),
            updated_at: created,
            deadline_at: Some(Timestamp::from_epoch_millis(
                EpochMillis::new(1000).unwrap_or_else(|_| unreachable!()),
            )),
            attempt_budget: AttemptBudget {
                max_model_calls: 12,
                max_tool_calls: 0,
                max_attempts_per_step: 0,
                extensions: Default::default(),
            },
            steps: Vec::new(),
            blocked_reason: None,
            result_summary: None,
            cancelled_at: None,
            cancelled_by: None,
            failure_reason: None,
            extensions: Default::default(),
        };
        let inserted = store.transact_with_participants(&TestTaskAudit, &TestTaskEvents, |tx| {
            tx.insert_task(&task, &transition).map(|_| ())
        });
        assert!(inserted.is_ok(), "test task insertion failed: {inserted:?}");
        for sequence in 1..=11u8 {
            let request_id = serea_protocol::RequestId::new(format!(
                "req_000000000000000000000000{sequence:02}"
            ))
            .unwrap_or_else(|_| unreachable!());
            store
                .reserve_model_call(
                    ModelCallAttemptDraft {
                        request_id: request_id.clone(),
                        task_id: Some(task_id.clone()),
                        purpose: ModelPurpose::Chat,
                        model_id: ModelId::new("nemotron-3-nano-30b")
                            .unwrap_or_else(|_| unreachable!()),
                        provider_id: ProviderId::new("provider").unwrap_or_else(|_| unreachable!()),
                        deployment_class: StorageDeploymentClass::Local,
                        data_class: DataClass::Public,
                        relation_kind: ModelAttemptRelationKind::None,
                        parent_request_id: None,
                        fallback_from_model_id: None,
                        price: ModelPriceSnapshot::new(CostClass::Paid, "price-1", 0, 0),
                        max_context_tokens: 20_000,
                        effective_max_output_tokens: 2_048,
                        dispatch_intent_at: EpochMillis::new(2).unwrap_or_else(|_| unreachable!()),
                    },
                    UsdMicros::new(0).unwrap_or_else(|_| unreachable!()),
                )
                .unwrap_or_else(|_| unreachable!());
            let input_tokens = if sequence == 11 { 11_645 } else { 11_635 };
            store
                .fail_model_call_with_usage(
                    &request_id,
                    "FIXTURE_FAILURE",
                    EpochMillis::new(3).unwrap_or_else(|_| unreachable!()),
                    ModelFailureUsage {
                        input_tokens: TokenCount::new(input_tokens),
                        output_tokens: TokenCount::new(0),
                        latency_ms: 1,
                        repair_attempts: 0,
                        recorded_at: EpochMillis::new(3).unwrap_or_else(|_| unreachable!()),
                    },
                )
                .unwrap_or_else(|_| unreachable!());
        }
        assert_eq!(
            store.task_model_token_usage(&task_id).ok(),
            Some(TokenCount::new(127_995))
        );
        let provider = Arc::new(RepairFakeProvider::new(vec!["reached token limit".into()]));
        let provider_id = provider.provider_id();
        let capabilities = caps(false, JsonSchemaMode::Strict, 20_000, 2_048);
        let roster = ModelRosterV1::new(vec![
            ModelRosterEntryV1::new(
                ModelId::new("nemotron-3-nano-30b").unwrap_or_else(|_| unreachable!()),
                provider_id.clone(),
                ModelDeploymentClass::Local,
                true,
                capabilities,
                CostClass::Paid,
            )
            .unwrap_or_else(|_| unreachable!()),
        ])
        .unwrap_or_else(|_| unreachable!());
        let router =
            ModelRouterV1::new(roster, vec![provider.clone()]).unwrap_or_else(|_| unreachable!());
        let call = PreparedModelCallV1::from_host(PreparedModelCallDraftV1 {
            task_id: Some(task_id.clone()),
            purpose: ModelPurpose::Chat,
            messages: vec![ModelMessage {
                role: serea_protocol::MessageRole::new("user").unwrap_or_else(|_| unreachable!()),
                content: "hello".into(),
            }],
            system: None,
            response_format: ResponseFormat::Text,
            tools: Vec::new(),
            max_output_tokens: 32,
            temperature: 0.0,
            deadline_ms: 1000,
            data_class: DataClass::Public,
            requirements: ModelRoutingRequirementsV1 {
                vision_required: false,
                tools_required: false,
                min_context_tokens: 1,
                min_output_tokens: 1,
                structured_requirement: StructuredRequirementV1::Any,
            },
            egress: ModelEgressPolicySnapshotV1::from_host(true),
            host_max_output_tokens: 2048,
        })
        .unwrap_or_else(|_| unreachable!());
        let bus = EventBus::new(IncrementingIds(0));
        let context = dispatch_context(
            &store,
            &bus,
            ModelPriceSnapshot::new(CostClass::Paid, "price-1", 1_000_000, 1_000_000),
            UsdMicros::new(10_000_000).unwrap_or_else(|_| unreachable!()),
        );
        let session = block_on(router.route(&call)).unwrap_or_else(|_| unreachable!());
        let result = block_on(router.dispatch_chat_text(&call, &session, &context));
        assert!(result.is_err());
        let request_id = provider
            .requests()
            .first()
            .map(|request| request.request_id.clone())
            .unwrap_or_else(|| unreachable!());
        let attempt = store
            .get_model_call_attempt(&request_id)
            .unwrap_or_else(|_| unreachable!())
            .unwrap_or_else(|| unreachable!());
        assert_eq!(attempt.state, ModelAttemptState::Completed);
        assert_eq!(
            store.task_model_token_usage(&task_id).ok(),
            Some(TokenCount::new(128_000))
        );
        assert!(
            store
                .model_usage_for_request(&request_id)
                .ok()
                .flatten()
                .is_some()
        );
        assert!(
            store
                .get_model_call_response(&request_id)
                .ok()
                .flatten()
                .is_some()
        );
        let events = EventBus::replay(&store, None, None, 32).unwrap_or_else(|_| unreachable!());
        assert!(events.items.iter().any(|item| matches!(item,
            ReplayItem::Event { event } if event.kind == EventKind::BoundExceeded
        )));
        drop(context);
        drop(router);
        drop(provider);
        drop(store);
        let _ = std::fs::remove_file(&db_path);
        let _ = std::fs::remove_file(db_path.with_extension("sqlite-wal"));
        let _ = std::fs::remove_file(db_path.with_extension("sqlite-shm"));
    }

    #[test]
    fn trusted_host_call_boundary_accounts_task_without_owning_lifecycle() {
        let db_path = std::env::temp_dir().join(format!(
            "serea-router-host-boundary-red-{}-{}.sqlite",
            std::process::id(),
            NEXT_RECOVERY_TEST_DB.fetch_add(1, Ordering::SeqCst)
        ));
        let store = Store::open(&db_path, &FixedClock).unwrap_or_else(|_| unreachable!());
        let task_id =
            TaskId::new("tsk_00000000000000000000000050").unwrap_or_else(|_| unreachable!());
        assert!(insert_budget_task(&store, task_id.clone(), "50"));
        let provider = Arc::new(RepairFakeProvider::new(vec!["trusted result".into()]));
        let roster = ModelRosterV1::new(vec![
            ModelRosterEntryV1::new(
                ModelId::new("nemotron-3-nano-30b").unwrap_or_else(|_| unreachable!()),
                provider.provider_id(),
                ModelDeploymentClass::Local,
                true,
                caps(false, JsonSchemaMode::Strict, 1000, 1000),
                CostClass::Paid,
            )
            .unwrap_or_else(|_| unreachable!()),
        ])
        .unwrap_or_else(|_| unreachable!());
        let price = ModelPriceSnapshot::new(CostClass::Paid, "price-host", 1_000_000, 1_000_000);
        let daily = UsdMicros::new(10_000_000).unwrap_or_else(|_| unreachable!());
        let router = ModelRouterProcessV1::new(
            roster,
            vec![provider.clone()],
            vec![(
                ModelId::new("nemotron-3-nano-30b").unwrap_or_else(|_| unreachable!()),
                price,
            )],
            daily,
        )
        .unwrap_or_else(|_| unreachable!());
        let call = PreparedModelCallV1::from_host(PreparedModelCallDraftV1 {
            task_id: Some(task_id.clone()),
            purpose: ModelPurpose::Chat,
            messages: vec![ModelMessage {
                role: serea_protocol::MessageRole::new("user").unwrap_or_else(|_| unreachable!()),
                content: "host prepared input".into(),
            }],
            system: None,
            response_format: ResponseFormat::Text,
            tools: Vec::new(),
            max_output_tokens: 16,
            temperature: 0.0,
            deadline_ms: 1000,
            data_class: DataClass::Public,
            requirements: ModelRoutingRequirementsV1 {
                vision_required: false,
                tools_required: false,
                min_context_tokens: 1,
                min_output_tokens: 1,
                structured_requirement: StructuredRequirementV1::Any,
            },
            egress: ModelEgressPolicySnapshotV1::from_host(false),
            host_max_output_tokens: 2048,
        })
        .unwrap_or_else(|_| unreachable!());
        let bus = EventBus::new(IncrementingIds(0));
        let host = ModelRouterHostContextV1::new(&store, &bus, &FixedClock, &OPEN_DISPATCH_GATE);

        let result = block_on(router.execute(&call, &host)).unwrap_or_else(|_| unreachable!());

        assert_eq!(result.content, "trusted result");
        assert_eq!(result.structured, None);
        assert_eq!(provider.requests().len(), 1);
        assert_eq!(provider.requests()[0].task_id.as_ref(), Some(&task_id));
        assert_eq!(store.task_model_call_count(&task_id).ok(), Some(1));
        assert_eq!(store.task_model_turn_count(&task_id).ok(), Some(1));
        assert_eq!(
            store.load_task(&task_id).ok().map(|task| task.task.state),
            Some(TaskState::Received)
        );
        drop(router);
        drop(provider);
        drop(store);
        let _ = std::fs::remove_file(&db_path);
        let _ = std::fs::remove_file(db_path.with_extension("sqlite-wal"));
        let _ = std::fs::remove_file(db_path.with_extension("sqlite-shm"));
    }

    #[test]
    fn host_gate_uses_real_task_cancellation_and_deletion_state() {
        let db_path = std::env::temp_dir().join(format!(
            "serea-router-host-task-gate-{}-{}.sqlite",
            std::process::id(),
            NEXT_RECOVERY_TEST_DB.fetch_add(1, Ordering::SeqCst)
        ));
        let store = Arc::new(Store::open(&db_path, &FixedClock).unwrap_or_else(|_| unreachable!()));
        let cancelled_before =
            TaskId::new("tsk_00000000000000000000000051").unwrap_or_else(|_| unreachable!());
        let cancelled_after_intent =
            TaskId::new("tsk_00000000000000000000000052").unwrap_or_else(|_| unreachable!());
        let deleted_before =
            TaskId::new("tsk_00000000000000000000000053").unwrap_or_else(|_| unreachable!());
        assert!(insert_budget_task(&store, cancelled_before.clone(), "51"));
        assert!(insert_budget_task(
            &store,
            cancelled_after_intent.clone(),
            "52"
        ));
        assert!(insert_budget_task(&store, deleted_before.clone(), "53"));

        let provider = Arc::new(RepairFakeProvider::new(vec!["must not dispatch".into()]));
        let roster = ModelRosterV1::new(vec![
            ModelRosterEntryV1::new(
                ModelId::new("nemotron-3-nano-30b").unwrap_or_else(|_| unreachable!()),
                provider.provider_id(),
                ModelDeploymentClass::Local,
                true,
                caps(false, JsonSchemaMode::Strict, 1000, 1000),
                CostClass::Paid,
            )
            .unwrap_or_else(|_| unreachable!()),
        ])
        .unwrap_or_else(|_| unreachable!());
        let price = ModelPriceSnapshot::new(CostClass::Paid, "price-host", 1_000_000, 1_000_000);
        let daily = UsdMicros::new(10_000_000).unwrap_or_else(|_| unreachable!());
        let router = ModelRouterProcessV1::new(
            roster,
            vec![provider.clone()],
            vec![(
                ModelId::new("nemotron-3-nano-30b").unwrap_or_else(|_| unreachable!()),
                price,
            )],
            daily,
        )
        .unwrap_or_else(|_| unreachable!());
        let bus = EventBus::new(IncrementingIds(0));

        let before_gate = StoreTaskGate {
            store: store.clone(),
            cancel_on_snapshot: None,
            snapshots: AtomicUsize::new(0),
            suffix: "54",
        };
        before_gate
            .cancel_task(&cancelled_before)
            .unwrap_or_else(|_| unreachable!());
        let before_host = ModelRouterHostContextV1::new(&store, &bus, &FixedClock, &before_gate);
        assert!(matches!(
            block_on(router.execute(
                &prepared_chat_for_task(cancelled_before.clone()),
                &before_host
            )),
            Err(ModelRouterCallFailureV1::Refused(
                RouterError::TaskCancelled
            ))
        ));
        assert_eq!(store.task_model_call_count(&cancelled_before).ok(), Some(0));
        assert_eq!(
            store
                .load_task(&cancelled_before)
                .ok()
                .map(|task| task.task.state),
            Some(TaskState::Cancelled)
        );

        let after_gate = StoreTaskGate {
            store: store.clone(),
            cancel_on_snapshot: Some(1),
            snapshots: AtomicUsize::new(0),
            suffix: "55",
        };
        let after_host = ModelRouterHostContextV1::new(&store, &bus, &FixedClock, &after_gate);
        assert!(matches!(
            block_on(router.execute(
                &prepared_chat_for_task(cancelled_after_intent.clone()),
                &after_host
            )),
            Err(ModelRouterCallFailureV1::Refused(
                RouterError::TaskCancelled
            ))
        ));
        assert_eq!(
            store.task_model_call_count(&cancelled_after_intent).ok(),
            Some(1)
        );
        assert_eq!(
            store.task_model_turn_count(&cancelled_after_intent).ok(),
            Some(1)
        );
        assert_eq!(
            store
                .load_task(&cancelled_after_intent)
                .ok()
                .map(|task| task.task.state),
            Some(TaskState::Cancelled)
        );

        store
            .transact(|tx| tx.delete_task(&deleted_before).map(|_| ()))
            .unwrap_or_else(|_| unreachable!());
        let deleted_gate = StoreTaskGate {
            store: store.clone(),
            cancel_on_snapshot: None,
            snapshots: AtomicUsize::new(0),
            suffix: "56",
        };
        let deleted_host = ModelRouterHostContextV1::new(&store, &bus, &FixedClock, &deleted_gate);
        let deleted_result =
            block_on(router.execute(&prepared_chat_for_task(deleted_before), &deleted_host));
        assert!(
            matches!(
                &deleted_result,
                Err(ModelRouterCallFailureV1::Storage(StoreError::TaskNotFound))
            ),
            "deleted task must fail closed: {deleted_result:?}"
        );
        assert!(provider.requests().is_empty());
        assert_eq!(
            store.task_model_call_count(&cancelled_after_intent).ok(),
            Some(1)
        );
        let events = EventBus::replay(&store, None, None, 16).unwrap_or_else(|_| unreachable!());
        let kinds = events
            .items
            .iter()
            .filter_map(|item| match item {
                ReplayItem::Event { event } => Some(event.kind),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(kinds, vec![EventKind::ModelCalled, EventKind::ModelFailed]);
        drop(router);
        drop(provider);
        drop(store);
        let _ = std::fs::remove_file(&db_path);
        let _ = std::fs::remove_file(db_path.with_extension("sqlite-wal"));
        let _ = std::fs::remove_file(db_path.with_extension("sqlite-shm"));
    }

    #[test]
    fn price_changes_apply_only_to_a_new_process_router_snapshot() {
        let db_path = std::env::temp_dir().join(format!(
            "serea-router-price-snapshot-{}-{}.sqlite",
            std::process::id(),
            NEXT_RECOVERY_TEST_DB.fetch_add(1, Ordering::SeqCst)
        ));
        let store = Store::open(&db_path, &FixedClock).unwrap_or_else(|_| unreachable!());
        let provider = Arc::new(RepairFakeProvider::new(vec![
            "first result".into(),
            "second result".into(),
            "third result".into(),
        ]));
        let roster = ModelRosterV1::new(vec![
            ModelRosterEntryV1::new(
                ModelId::new("nemotron-3-nano-30b").unwrap_or_else(|_| unreachable!()),
                provider.provider_id(),
                ModelDeploymentClass::Local,
                true,
                caps(false, JsonSchemaMode::Strict, 1000, 1000),
                CostClass::Paid,
            )
            .unwrap_or_else(|_| unreachable!()),
        ])
        .unwrap_or_else(|_| unreachable!());
        let model_id = ModelId::new("nemotron-3-nano-30b").unwrap_or_else(|_| unreachable!());
        let daily = UsdMicros::new(10_000_000).unwrap_or_else(|_| unreachable!());
        let first_process = ModelRouterProcessV1::new(
            roster.clone(),
            vec![provider.clone()],
            vec![(
                model_id.clone(),
                ModelPriceSnapshot::new(CostClass::Paid, "price-v1", 1_000_000, 1_000_000),
            )],
            daily,
        )
        .unwrap_or_else(|_| unreachable!());
        let call = PreparedModelCallV1::from_host(PreparedModelCallDraftV1 {
            task_id: None,
            purpose: ModelPurpose::Chat,
            messages: Vec::new(),
            system: None,
            response_format: ResponseFormat::Text,
            tools: Vec::new(),
            max_output_tokens: 16,
            temperature: 0.0,
            deadline_ms: 1000,
            data_class: DataClass::Public,
            requirements: ModelRoutingRequirementsV1 {
                vision_required: false,
                tools_required: false,
                min_context_tokens: 1,
                min_output_tokens: 1,
                structured_requirement: StructuredRequirementV1::Any,
            },
            egress: ModelEgressPolicySnapshotV1::from_host(false),
            host_max_output_tokens: 2048,
        })
        .unwrap_or_else(|_| unreachable!());
        let bus = EventBus::new(IncrementingIds(0));
        let host = ModelRouterHostContextV1::new(&store, &bus, &FixedClock, &OPEN_DISPATCH_GATE);
        let first =
            block_on(first_process.execute(&call, &host)).unwrap_or_else(|_| unreachable!());
        let second_process = ModelRouterProcessV1::new(
            roster,
            vec![provider.clone()],
            vec![(
                model_id.clone(),
                ModelPriceSnapshot::new(CostClass::Paid, "price-v2", 2_000_000, 3_000_000),
            )],
            daily,
        )
        .unwrap_or_else(|_| unreachable!());
        let second =
            block_on(second_process.execute(&call, &host)).unwrap_or_else(|_| unreachable!());
        let third =
            block_on(first_process.execute(&call, &host)).unwrap_or_else(|_| unreachable!());

        let first_attempt = store
            .get_model_call_attempt(&first.request_id)
            .unwrap_or_else(|_| unreachable!())
            .unwrap_or_else(|| unreachable!());
        let second_attempt = store
            .get_model_call_attempt(&second.request_id)
            .unwrap_or_else(|_| unreachable!())
            .unwrap_or_else(|| unreachable!());
        let third_attempt = store
            .get_model_call_attempt(&third.request_id)
            .unwrap_or_else(|_| unreachable!())
            .unwrap_or_else(|| unreachable!());
        assert_eq!(first_attempt.price.price_revision(), "price-v1");
        assert_eq!(second_attempt.price.price_revision(), "price-v2");
        assert_eq!(
            second_attempt
                .price
                .input_rate_microusd_per_million_tokens(),
            2_000_000
        );
        assert_eq!(
            second_attempt
                .price
                .output_rate_microusd_per_million_tokens(),
            3_000_000
        );
        assert_eq!(third_attempt.price.price_revision(), "price-v1");
        assert_eq!(first_attempt.model_id, model_id);
        drop(second_process);
        drop(first_process);
        drop(provider);
        drop(store);
        let _ = std::fs::remove_file(&db_path);
        let _ = std::fs::remove_file(db_path.with_extension("sqlite-wal"));
        let _ = std::fs::remove_file(db_path.with_extension("sqlite-shm"));
    }

    #[test]
    fn fallback_uses_its_own_price_and_reuses_the_process_health_snapshot() {
        let db_path = std::env::temp_dir().join(format!(
            "serea-router-fallback-price-{}-{}.sqlite",
            std::process::id(),
            NEXT_RECOVERY_TEST_DB.fetch_add(1, Ordering::SeqCst)
        ));
        let store = Store::open(&db_path, &FixedClock).unwrap_or_else(|_| unreachable!());
        let provider = Arc::new(RepairFakeProvider::new(Vec::new()));
        provider.push_script(RepairScript::Fail(ModelError {
            kind: ModelErrorCode::new("UPSTREAM_UNAVAILABLE").unwrap_or_else(|_| unreachable!()),
            message: serea_protocol::ErrorMessage::new("scripted retryable failure")
                .unwrap_or_else(|_| unreachable!()),
            retryable: true,
        }));
        provider.push_script(RepairScript::Respond("fallback answer".into()));
        let roster = ModelRosterV1::new(vec![
            ModelRosterEntryV1::new(
                ModelId::new("nemotron-3-nano-30b").unwrap_or_else(|_| unreachable!()),
                provider.provider_id(),
                ModelDeploymentClass::Local,
                true,
                caps(false, JsonSchemaMode::Strict, 1000, 1000),
                CostClass::Paid,
            )
            .unwrap_or_else(|_| unreachable!()),
            ModelRosterEntryV1::new(
                ModelId::new("gpt-oss-20b").unwrap_or_else(|_| unreachable!()),
                provider.provider_id(),
                ModelDeploymentClass::Local,
                true,
                caps(false, JsonSchemaMode::Strict, 1000, 1000),
                CostClass::Paid,
            )
            .unwrap_or_else(|_| unreachable!()),
        ])
        .unwrap_or_else(|_| unreachable!());
        let price =
            |revision| ModelPriceSnapshot::new(CostClass::Paid, revision, 1_000_000, 1_000_000);
        let process = ModelRouterProcessV1::new(
            roster,
            vec![provider.clone()],
            vec![
                (
                    ModelId::new("nemotron-3-nano-30b").unwrap_or_else(|_| unreachable!()),
                    price("primary-price"),
                ),
                (
                    ModelId::new("gpt-oss-20b").unwrap_or_else(|_| unreachable!()),
                    price("fallback-price"),
                ),
            ],
            UsdMicros::new(10_000_000).unwrap_or_else(|_| unreachable!()),
        )
        .unwrap_or_else(|_| unreachable!());
        let call = prepared_chat_for_task(
            TaskId::new("tsk_00000000000000000000000057").unwrap_or_else(|_| unreachable!()),
        );
        assert!(insert_budget_task(
            &store,
            call.task_id().cloned().unwrap_or_else(|| unreachable!()),
            "57"
        ));
        let bus = EventBus::new(IncrementingIds(0));
        let host = ModelRouterHostContextV1::new(&store, &bus, &FixedClock, &OPEN_DISPATCH_GATE);

        let result = block_on(process.execute(&call, &host)).unwrap_or_else(|_| unreachable!());

        let requests = provider.requests();
        assert_eq!(requests.len(), 2);
        assert_eq!(result.model_id.as_str(), "gpt-oss-20b");
        let primary = store
            .get_model_call_attempt(&requests[0].request_id)
            .unwrap_or_else(|_| unreachable!())
            .unwrap_or_else(|| unreachable!());
        let fallback = store
            .get_model_call_attempt(&requests[1].request_id)
            .unwrap_or_else(|_| unreachable!())
            .unwrap_or_else(|| unreachable!());
        assert_eq!(primary.price.price_revision(), "primary-price");
        assert_eq!(fallback.price.price_revision(), "fallback-price");
        assert_eq!(provider.health_calls.load(Ordering::SeqCst), 1);
        drop(process);
        drop(provider);
        drop(store);
        let _ = std::fs::remove_file(&db_path);
        let _ = std::fs::remove_file(db_path.with_extension("sqlite-wal"));
        let _ = std::fs::remove_file(db_path.with_extension("sqlite-shm"));
    }

    #[test]
    fn fallback_daily_spend_exhaustion_creates_no_fallback_child() {
        let db_path = std::env::temp_dir().join(format!(
            "serea-router-fallback-spend-{}-{}.sqlite",
            std::process::id(),
            NEXT_RECOVERY_TEST_DB.fetch_add(1, Ordering::SeqCst)
        ));
        let store = Store::open(&db_path, &FixedClock).unwrap_or_else(|_| unreachable!());
        let task_id =
            TaskId::new("tsk_00000000000000000000000073").unwrap_or_else(|_| unreachable!());
        assert!(insert_budget_task(&store, task_id.clone(), "73"));
        let provider = Arc::new(RepairFakeProvider::new(Vec::new()));
        provider.push_script(RepairScript::Fail(ModelError {
            kind: ModelErrorCode::new("UPSTREAM_UNAVAILABLE").unwrap_or_else(|_| unreachable!()),
            message: serea_protocol::ErrorMessage::new("retryable spend test failure")
                .unwrap_or_else(|_| unreachable!()),
            retryable: true,
        }));
        provider.push_script(RepairScript::Respond("must not dispatch fallback".into()));
        let roster = ModelRosterV1::new(vec![
            ModelRosterEntryV1::new(
                ModelId::new("nemotron-3-nano-30b").unwrap_or_else(|_| unreachable!()),
                provider.provider_id(),
                ModelDeploymentClass::Local,
                true,
                caps(false, JsonSchemaMode::Strict, 1000, 1000),
                CostClass::Paid,
            )
            .unwrap_or_else(|_| unreachable!()),
            ModelRosterEntryV1::new(
                ModelId::new("gpt-oss-20b").unwrap_or_else(|_| unreachable!()),
                provider.provider_id(),
                ModelDeploymentClass::Local,
                true,
                caps(false, JsonSchemaMode::Strict, 1000, 1000),
                CostClass::Paid,
            )
            .unwrap_or_else(|_| unreachable!()),
        ])
        .unwrap_or_else(|_| unreachable!());
        let process = ModelRouterProcessV1::new(
            roster,
            vec![provider.clone()],
            vec![
                (
                    ModelId::new("nemotron-3-nano-30b").unwrap_or_else(|_| unreachable!()),
                    ModelPriceSnapshot::new(CostClass::Paid, "primary", 1_000_000, 1_000_000),
                ),
                (
                    ModelId::new("gpt-oss-20b").unwrap_or_else(|_| unreachable!()),
                    ModelPriceSnapshot::new(CostClass::Paid, "fallback", 1_000_000, 1_000_000),
                ),
            ],
            UsdMicros::new(1_500).unwrap_or_else(|_| unreachable!()),
        )
        .unwrap_or_else(|_| unreachable!());
        let bus = EventBus::new(IncrementingIds(0));
        let host = ModelRouterHostContextV1::new(&store, &bus, &FixedClock, &OPEN_DISPATCH_GATE);

        assert!(matches!(
            block_on(process.execute(&prepared_chat_for_task(task_id.clone()), &host)),
            Err(ModelRouterCallFailureV1::BoundExceeded {
                bound: ModelBoundKindV1::DailySpendUsd,
                limit: 1_500,
                ..
            })
        ));
        assert_eq!(provider.requests().len(), 1);
        assert_eq!(store.task_model_call_count(&task_id).ok(), Some(1));
        let primary_request = provider.requests()[0].request_id.clone();
        let primary = store
            .get_model_call_attempt(&primary_request)
            .unwrap_or_else(|_| unreachable!())
            .unwrap_or_else(|| unreachable!());
        assert_eq!(primary.state, ModelAttemptState::Failed);
        assert_eq!(primary.relation_kind, ModelAttemptRelationKind::None);
        assert!(
            store
                .list_unfinished_model_call_attempts()
                .unwrap_or_else(|_| unreachable!())
                .is_empty()
        );
        let events = EventBus::replay(&store, None, None, 16).unwrap_or_else(|_| unreachable!());
        let kinds = events
            .items
            .iter()
            .filter_map(|item| match item {
                ReplayItem::Event { event } => Some(event.kind),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(
            kinds,
            vec![
                EventKind::ModelCalled,
                EventKind::BoundExceeded,
                EventKind::ModelFailed
            ]
        );
        drop(process);
        drop(provider);
        drop(store);
        let _ = std::fs::remove_file(&db_path);
        let _ = std::fs::remove_file(db_path.with_extension("sqlite-wal"));
        let _ = std::fs::remove_file(db_path.with_extension("sqlite-shm"));
    }

    #[test]
    fn independent_store_calls_serialize_same_task_and_allow_distinct_tasks() {
        let db_path = std::env::temp_dir().join(format!(
            "serea-router-task-concurrency-{}-{}.sqlite",
            std::process::id(),
            NEXT_RECOVERY_TEST_DB.fetch_add(1, Ordering::SeqCst)
        ));
        let setup = Store::open(&db_path, &FixedClock).unwrap_or_else(|_| unreachable!());
        let same_task =
            TaskId::new("tsk_00000000000000000000000062").unwrap_or_else(|_| unreachable!());
        assert!(insert_budget_task(&setup, same_task.clone(), "62"));
        let distinct_a =
            TaskId::new("tsk_00000000000000000000000063").unwrap_or_else(|_| unreachable!());
        let distinct_b =
            TaskId::new("tsk_00000000000000000000000064").unwrap_or_else(|_| unreachable!());
        assert!(insert_budget_task(&setup, distinct_a.clone(), "63"));
        assert!(insert_budget_task(&setup, distinct_b.clone(), "64"));
        drop(setup);

        let same_a =
            Arc::new(Store::open(&db_path, &FixedClock).unwrap_or_else(|_| unreachable!()));
        let same_b =
            Arc::new(Store::open(&db_path, &FixedClock).unwrap_or_else(|_| unreachable!()));
        let (started_tx, started_rx) = mpsc::sync_channel(2);
        let (release_tx, release_rx) = mpsc::channel();
        let provider = Arc::new(BlockingProvider {
            started: started_tx,
            release: Mutex::new(release_rx),
            calls: AtomicUsize::new(0),
        });
        let process = Arc::new(blocking_process(
            provider.clone(),
            UsdMicros::new(10_000_000).unwrap_or_else(|_| unreachable!()),
        ));
        let same_bus_a = EventBus::new(IncrementingIds(0));
        let same_bus_b = EventBus::new(IncrementingIds(100));
        let same_host_a =
            ModelRouterHostContextV1::new(&same_a, &same_bus_a, &FixedClock, &OPEN_DISPATCH_GATE);
        let same_host_b =
            ModelRouterHostContextV1::new(&same_b, &same_bus_b, &FixedClock, &OPEN_DISPATCH_GATE);
        let same_call = prepared_chat_for_task(same_task.clone());
        std::thread::scope(|scope| {
            let first_process = process.clone();
            let first_call = same_call.clone();
            let first_host = &same_host_a;
            let first =
                scope.spawn(move || block_on(first_process.execute(&first_call, first_host)));
            let first_request = started_rx
                .recv_timeout(Duration::from_secs(10))
                .unwrap_or_else(|_| {
                    unreachable!("first same-task dispatch never reached provider")
                });

            let second_process = process.clone();
            let second_call = same_call.clone();
            let second_host = &same_host_b;
            let second =
                scope.spawn(move || block_on(second_process.execute(&second_call, second_host)));
            let second_result = second
                .join()
                .unwrap_or_else(|_| unreachable!("second same-task caller panicked"));
            assert!(matches!(
                second_result,
                Err(ModelRouterCallFailureV1::Storage(
                    StoreError::ModelCallInFlight
                ))
            ));
            assert_eq!(provider.calls.load(Ordering::SeqCst), 1);
            release_tx
                .send(())
                .unwrap_or_else(|_| unreachable!("provider release channel closed"));
            let first_result = first
                .join()
                .unwrap_or_else(|_| unreachable!("first same-task caller panicked"))
                .unwrap_or_else(|_| unreachable!("first same-task call failed"));
            assert_eq!(first_result.request_id, first_request);
        });
        assert_eq!(same_a.task_model_call_count(&same_task).ok(), Some(1));
        assert_eq!(same_a.task_model_turn_count(&same_task).ok(), Some(1));

        let distinct_a_store =
            Arc::new(Store::open(&db_path, &FixedClock).unwrap_or_else(|_| unreachable!()));
        let distinct_b_store =
            Arc::new(Store::open(&db_path, &FixedClock).unwrap_or_else(|_| unreachable!()));
        let (entered_tx, entered_rx) = mpsc::sync_channel(2);
        let (resume_tx, resume_rx) = mpsc::channel();
        let distinct_provider = Arc::new(BlockingProvider {
            started: entered_tx,
            release: Mutex::new(resume_rx),
            calls: AtomicUsize::new(0),
        });
        let distinct_process = Arc::new(blocking_process(
            distinct_provider.clone(),
            UsdMicros::new(10_000_000).unwrap_or_else(|_| unreachable!()),
        ));
        let distinct_bus_a = EventBus::new(IncrementingIds(200));
        let distinct_bus_b = EventBus::new(IncrementingIds(300));
        let distinct_host_a = ModelRouterHostContextV1::new(
            &distinct_a_store,
            &distinct_bus_a,
            &FixedClock,
            &OPEN_DISPATCH_GATE,
        );
        let distinct_host_b = ModelRouterHostContextV1::new(
            &distinct_b_store,
            &distinct_bus_b,
            &FixedClock,
            &OPEN_DISPATCH_GATE,
        );
        let barrier = Arc::new(std::sync::Barrier::new(3));
        let distinct_results = std::thread::scope(|scope| {
            let a_process = distinct_process.clone();
            let a_call = prepared_chat_for_task(distinct_a.clone());
            let a_barrier = barrier.clone();
            let a_host = &distinct_host_a;
            let a = scope.spawn(move || {
                a_barrier.wait();
                block_on(a_process.execute(&a_call, a_host))
            });
            let b_process = distinct_process.clone();
            let b_call = prepared_chat_for_task(distinct_b.clone());
            let b_barrier = barrier.clone();
            let b_host = &distinct_host_b;
            let b = scope.spawn(move || {
                b_barrier.wait();
                block_on(b_process.execute(&b_call, b_host))
            });
            barrier.wait();
            let first_entered = entered_rx
                .recv_timeout(Duration::from_secs(10))
                .unwrap_or_else(|_| unreachable!("first distinct task did not reach provider"));
            let second_entered = entered_rx
                .recv_timeout(Duration::from_secs(10))
                .unwrap_or_else(|_| unreachable!("second distinct task did not reach provider"));
            assert_ne!(first_entered, second_entered);
            resume_tx
                .send(())
                .unwrap_or_else(|_| unreachable!("first provider release channel closed"));
            resume_tx
                .send(())
                .unwrap_or_else(|_| unreachable!("second provider release channel closed"));
            [
                a.join()
                    .unwrap_or_else(|_| unreachable!("first distinct caller panicked")),
                b.join()
                    .unwrap_or_else(|_| unreachable!("second distinct caller panicked")),
            ]
        });
        assert!(distinct_results.iter().all(Result::is_ok));
        assert_eq!(distinct_provider.calls.load(Ordering::SeqCst), 2);
        assert_eq!(
            distinct_a_store.task_model_call_count(&distinct_a).ok(),
            Some(1)
        );
        assert_eq!(
            distinct_b_store.task_model_call_count(&distinct_b).ok(),
            Some(1)
        );
        drop(distinct_process);
        drop(distinct_provider);
        drop(distinct_a_store);
        drop(distinct_b_store);
        drop(process);
        drop(provider);
        drop(same_a);
        drop(same_b);
        let _ = std::fs::remove_file(&db_path);
        let _ = std::fs::remove_file(db_path.with_extension("sqlite-wal"));
        let _ = std::fs::remove_file(db_path.with_extension("sqlite-shm"));
    }

    #[test]
    fn daily_spend_reservation_race_refuses_second_task_before_provider() {
        let db_path = std::env::temp_dir().join(format!(
            "serea-router-spend-race-{}-{}.sqlite",
            std::process::id(),
            NEXT_RECOVERY_TEST_DB.fetch_add(1, Ordering::SeqCst)
        ));
        let setup = Store::open(&db_path, &FixedClock).unwrap_or_else(|_| unreachable!());
        let first_task =
            TaskId::new("tsk_00000000000000000000000065").unwrap_or_else(|_| unreachable!());
        let second_task =
            TaskId::new("tsk_00000000000000000000000066").unwrap_or_else(|_| unreachable!());
        assert!(insert_budget_task(&setup, first_task.clone(), "65"));
        assert!(insert_budget_task(&setup, second_task.clone(), "66"));
        drop(setup);

        let first_store =
            Arc::new(Store::open(&db_path, &FixedClock).unwrap_or_else(|_| unreachable!()));
        let second_store =
            Arc::new(Store::open(&db_path, &FixedClock).unwrap_or_else(|_| unreachable!()));
        let (started_tx, started_rx) = mpsc::sync_channel(2);
        let (release_tx, release_rx) = mpsc::channel();
        let provider = Arc::new(BlockingProvider {
            started: started_tx,
            release: Mutex::new(release_rx),
            calls: AtomicUsize::new(0),
        });
        let process = Arc::new(blocking_process(
            provider.clone(),
            UsdMicros::new(1_500).unwrap_or_else(|_| unreachable!()),
        ));
        let first_bus = EventBus::new(IncrementingIds(0));
        let second_bus = EventBus::new(IncrementingIds(100));
        let first_host = ModelRouterHostContextV1::new(
            &first_store,
            &first_bus,
            &FixedClock,
            &OPEN_DISPATCH_GATE,
        );
        let second_host = ModelRouterHostContextV1::new(
            &second_store,
            &second_bus,
            &FixedClock,
            &OPEN_DISPATCH_GATE,
        );
        let first_call = prepared_chat_for_task(first_task.clone());
        let second_call = prepared_chat_for_task(second_task.clone());
        std::thread::scope(|scope| {
            let first_process = process.clone();
            let first_host_ref = &first_host;
            let first =
                scope.spawn(move || block_on(first_process.execute(&first_call, first_host_ref)));
            let _first_request = started_rx
                .recv_timeout(Duration::from_secs(10))
                .unwrap_or_else(|_| unreachable!("first spend reservation never dispatched"));

            let second_process = process.clone();
            let second_host_ref = &second_host;
            let second = scope
                .spawn(move || block_on(second_process.execute(&second_call, second_host_ref)));
            let second_result = second
                .join()
                .unwrap_or_else(|_| unreachable!("second spend caller panicked"));
            assert!(matches!(
                second_result,
                Err(ModelRouterCallFailureV1::BoundExceeded {
                    bound: ModelBoundKindV1::DailySpendUsd,
                    limit: 1_500,
                    ..
                })
            ));
            assert_eq!(provider.calls.load(Ordering::SeqCst), 1);
            release_tx
                .send(())
                .unwrap_or_else(|_| unreachable!("provider release channel closed"));
            first
                .join()
                .unwrap_or_else(|_| unreachable!("first spend caller panicked"))
                .unwrap_or_else(|_| unreachable!("first spend call failed"));
        });
        assert_eq!(first_store.task_model_call_count(&first_task).ok(), Some(1));
        assert_eq!(
            second_store.task_model_call_count(&second_task).ok(),
            Some(0)
        );
        let second_events =
            EventBus::replay(&second_store, None, None, 16).unwrap_or_else(|_| unreachable!());
        assert!(second_events.items.iter().any(|item| matches!(
            item,
            ReplayItem::Event { event }
                if event.kind == EventKind::BoundExceeded
                    && event.payload["bound_name"] == "max_daily_spend_usd"
        )));
        drop(process);
        drop(provider);
        drop(first_store);
        drop(second_store);
        let _ = std::fs::remove_file(&db_path);
        let _ = std::fs::remove_file(db_path.with_extension("sqlite-wal"));
        let _ = std::fs::remove_file(db_path.with_extension("sqlite-shm"));
    }

    #[test]
    fn cancellation_stops_new_fallback_and_repair_dispatches_for_real_tasks() {
        let db_path = std::env::temp_dir().join(format!(
            "serea-router-retry-cancel-{}-{}.sqlite",
            std::process::id(),
            NEXT_RECOVERY_TEST_DB.fetch_add(1, Ordering::SeqCst)
        ));
        let store = Arc::new(Store::open(&db_path, &FixedClock).unwrap_or_else(|_| unreachable!()));
        let fallback_task =
            TaskId::new("tsk_00000000000000000000000067").unwrap_or_else(|_| unreachable!());
        let repair_task =
            TaskId::new("tsk_00000000000000000000000068").unwrap_or_else(|_| unreachable!());
        assert!(insert_budget_task(&store, fallback_task.clone(), "67"));
        assert!(insert_budget_task(&store, repair_task.clone(), "68"));

        let provider = Arc::new(RepairFakeProvider::new(Vec::new()));
        provider.push_script(RepairScript::Fail(ModelError {
            kind: ModelErrorCode::new("UPSTREAM_UNAVAILABLE").unwrap_or_else(|_| unreachable!()),
            message: serea_protocol::ErrorMessage::new("retryable test failure")
                .unwrap_or_else(|_| unreachable!()),
            retryable: true,
        }));
        provider.push_script(RepairScript::Respond(r#"{"count":"invalid"}"#.into()));
        let roster = ModelRosterV1::new(vec![
            ModelRosterEntryV1::new(
                ModelId::new("nemotron-3-nano-30b").unwrap_or_else(|_| unreachable!()),
                provider.provider_id(),
                ModelDeploymentClass::Local,
                true,
                caps(false, JsonSchemaMode::Strict, 1000, 1000),
                CostClass::Paid,
            )
            .unwrap_or_else(|_| unreachable!()),
            ModelRosterEntryV1::new(
                ModelId::new("gpt-oss-20b").unwrap_or_else(|_| unreachable!()),
                provider.provider_id(),
                ModelDeploymentClass::Local,
                true,
                caps(false, JsonSchemaMode::Strict, 1000, 1000),
                CostClass::Paid,
            )
            .unwrap_or_else(|_| unreachable!()),
        ])
        .unwrap_or_else(|_| unreachable!());
        let price =
            |revision| ModelPriceSnapshot::new(CostClass::Paid, revision, 1_000_000, 1_000_000);
        let process = ModelRouterProcessV1::new(
            roster,
            vec![provider.clone()],
            vec![
                (
                    ModelId::new("nemotron-3-nano-30b").unwrap_or_else(|_| unreachable!()),
                    price("primary"),
                ),
                (
                    ModelId::new("gpt-oss-20b").unwrap_or_else(|_| unreachable!()),
                    price("alternate"),
                ),
            ],
            UsdMicros::new(10_000_000).unwrap_or_else(|_| unreachable!()),
        )
        .unwrap_or_else(|_| unreachable!());
        let bus = EventBus::new(IncrementingIds(0));
        let fallback_gate = StoreTaskGate {
            store: store.clone(),
            cancel_on_snapshot: Some(2),
            snapshots: AtomicUsize::new(0),
            suffix: "69",
        };
        let fallback_host =
            ModelRouterHostContextV1::new(&store, &bus, &FixedClock, &fallback_gate);
        assert!(matches!(
            block_on(process.execute(
                &prepared_chat_for_task(fallback_task.clone()),
                &fallback_host
            )),
            Err(ModelRouterCallFailureV1::Refused(
                RouterError::TaskCancelled
            ))
        ));
        assert_eq!(store.task_model_call_count(&fallback_task).ok(), Some(1));
        assert_eq!(
            store
                .load_task(&fallback_task)
                .ok()
                .map(|task| task.task.state),
            Some(TaskState::Cancelled)
        );

        let repair_gate = StoreTaskGate {
            store: store.clone(),
            cancel_on_snapshot: Some(2),
            snapshots: AtomicUsize::new(0),
            suffix: "70",
        };
        let repair_host = ModelRouterHostContextV1::new(&store, &bus, &FixedClock, &repair_gate);
        assert!(matches!(
            block_on(process.execute(
                &prepared_analysis_for_task(repair_task.clone()),
                &repair_host
            )),
            Err(ModelRouterCallFailureV1::Refused(
                RouterError::TaskCancelled
            ))
        ));
        assert_eq!(store.task_model_call_count(&repair_task).ok(), Some(1));
        assert_eq!(
            store
                .load_task(&repair_task)
                .ok()
                .map(|task| task.task.state),
            Some(TaskState::Cancelled)
        );
        assert_eq!(provider.requests().len(), 2);
        let attempts = provider
            .requests()
            .iter()
            .map(|request| {
                store
                    .get_model_call_attempt(&request.request_id)
                    .unwrap_or_else(|_| unreachable!())
                    .unwrap_or_else(|| unreachable!())
            })
            .collect::<Vec<_>>();
        assert!(
            attempts
                .iter()
                .all(|attempt| attempt.relation_kind == ModelAttemptRelationKind::None)
        );
        drop(process);
        drop(provider);
        drop(store);
        let _ = std::fs::remove_file(&db_path);
        let _ = std::fs::remove_file(db_path.with_extension("sqlite-wal"));
        let _ = std::fs::remove_file(db_path.with_extension("sqlite-shm"));
    }

    #[test]
    fn independent_recovery_workers_emit_one_ambiguity_transition() {
        let db_path = std::env::temp_dir().join(format!(
            "serea-router-recovery-race-{}-{}.sqlite",
            std::process::id(),
            NEXT_RECOVERY_TEST_DB.fetch_add(1, Ordering::SeqCst)
        ));
        let setup = Store::open(&db_path, &FixedClock).unwrap_or_else(|_| unreachable!());
        let request_id = serea_protocol::RequestId::new("req_00000000000000000000000071")
            .unwrap_or_else(|_| unreachable!());
        setup
            .reserve_model_call(
                ModelCallAttemptDraft {
                    request_id: request_id.clone(),
                    task_id: None,
                    purpose: ModelPurpose::Chat,
                    model_id: ModelId::new("nemotron-3-nano-30b")
                        .unwrap_or_else(|_| unreachable!()),
                    provider_id: ProviderId::new("provider").unwrap_or_else(|_| unreachable!()),
                    deployment_class: StorageDeploymentClass::Local,
                    data_class: DataClass::Public,
                    relation_kind: ModelAttemptRelationKind::None,
                    parent_request_id: None,
                    fallback_from_model_id: None,
                    price: ModelPriceSnapshot::new(
                        CostClass::Paid,
                        "recovery-price",
                        1_000_000,
                        1_000_000,
                    ),
                    max_context_tokens: 1000,
                    effective_max_output_tokens: 16,
                    dispatch_intent_at: FixedClock.now_ms().unwrap_or_else(|_| unreachable!()),
                },
                UsdMicros::new(10_000_000).unwrap_or_else(|_| unreachable!()),
            )
            .unwrap_or_else(|_| unreachable!());
        drop(setup);

        let first_store =
            Arc::new(Store::open(&db_path, &FixedClock).unwrap_or_else(|_| unreachable!()));
        let second_store =
            Arc::new(Store::open(&db_path, &FixedClock).unwrap_or_else(|_| unreachable!()));
        let barrier = Arc::new(std::sync::Barrier::new(3));
        let results = std::thread::scope(|scope| {
            let first_store = first_store.clone();
            let first_barrier = barrier.clone();
            let first = scope.spawn(move || {
                first_barrier.wait();
                recover_unresolved_model_calls(
                    &first_store,
                    &EventBus::new(IncrementingIds(700)),
                    &FixedClock,
                )
            });
            let second_store = second_store.clone();
            let second_barrier = barrier.clone();
            let second = scope.spawn(move || {
                second_barrier.wait();
                recover_unresolved_model_calls(
                    &second_store,
                    &EventBus::new(IncrementingIds(800)),
                    &FixedClock,
                )
            });
            barrier.wait();
            [
                first
                    .join()
                    .unwrap_or_else(|_| unreachable!("first recovery worker panicked")),
                second
                    .join()
                    .unwrap_or_else(|_| unreachable!("second recovery worker panicked")),
            ]
        });
        let recovered = results
            .into_iter()
            .map(|result| result.unwrap_or_else(|_| unreachable!()))
            .sum::<usize>();
        assert_eq!(recovered, 1);
        let attempt = first_store
            .get_model_call_attempt(&request_id)
            .unwrap_or_else(|_| unreachable!())
            .unwrap_or_else(|| unreachable!());
        assert_eq!(attempt.state, ModelAttemptState::Ambiguous);
        assert_eq!(attempt.actual_cost_usd_micros, None);
        assert!(
            first_store
                .model_usage_for_request(&request_id)
                .unwrap_or_else(|_| unreachable!())
                .is_none()
        );
        let events =
            EventBus::replay(&first_store, None, None, 8).unwrap_or_else(|_| unreachable!());
        assert!(
            matches!(events.items.as_slice(), [ReplayItem::Event { event }] if event.kind == EventKind::ModelFailed)
        );
        assert_eq!(
            recover_unresolved_model_calls(
                &first_store,
                &EventBus::new(IncrementingIds(900)),
                &FixedClock,
            )
            .unwrap_or_else(|_| unreachable!()),
            0
        );

        drop(first_store);
        drop(second_store);
        let _ = std::fs::remove_file(&db_path);
        let _ = std::fs::remove_file(db_path.with_extension("sqlite-wal"));
        let _ = std::fs::remove_file(db_path.with_extension("sqlite-shm"));
    }

    #[test]
    fn last_call_and_turn_slots_are_consumed_once_across_store_connections() {
        let db_path = std::env::temp_dir().join(format!(
            "serea-router-last-budget-slot-{}-{}.sqlite",
            std::process::id(),
            NEXT_RECOVERY_TEST_DB.fetch_add(1, Ordering::SeqCst)
        ));
        let setup = Store::open(&db_path, &FixedClock).unwrap_or_else(|_| unreachable!());
        let task_id =
            TaskId::new("tsk_00000000000000000000000072").unwrap_or_else(|_| unreachable!());
        assert!(insert_budget_task(&setup, task_id.clone(), "72"));
        seed_terminal_task_model_calls(&setup, &task_id, 11);
        assert_eq!(setup.task_model_call_count(&task_id).ok(), Some(11));
        assert_eq!(setup.task_model_turn_count(&task_id).ok(), Some(11));
        drop(setup);

        let first_store =
            Arc::new(Store::open(&db_path, &FixedClock).unwrap_or_else(|_| unreachable!()));
        let second_store =
            Arc::new(Store::open(&db_path, &FixedClock).unwrap_or_else(|_| unreachable!()));
        let provider = Arc::new(RepairFakeProvider::new(vec!["last slot result".into()]));
        let process = Arc::new(one_model_process(
            provider.clone(),
            UsdMicros::new(10_000_000).unwrap_or_else(|_| unreachable!()),
        ));
        let first_bus = EventBus::new(IncrementingIds(0));
        let second_bus = EventBus::new(IncrementingIds(100));
        let first_host = ModelRouterHostContextV1::new(
            &first_store,
            &first_bus,
            &FixedClock,
            &OPEN_DISPATCH_GATE,
        );
        let second_host = ModelRouterHostContextV1::new(
            &second_store,
            &second_bus,
            &FixedClock,
            &OPEN_DISPATCH_GATE,
        );
        let barrier = Arc::new(std::sync::Barrier::new(3));
        let results = std::thread::scope(|scope| {
            let first_process = process.clone();
            let first_call = prepared_chat_for_task(task_id.clone());
            let first_barrier = barrier.clone();
            let first_host_ref = &first_host;
            let first = scope.spawn(move || {
                first_barrier.wait();
                block_on(first_process.execute(&first_call, first_host_ref))
            });
            let second_process = process.clone();
            let second_call = prepared_chat_for_task(task_id.clone());
            let second_barrier = barrier.clone();
            let second_host_ref = &second_host;
            let second = scope.spawn(move || {
                second_barrier.wait();
                block_on(second_process.execute(&second_call, second_host_ref))
            });
            barrier.wait();
            [
                first
                    .join()
                    .unwrap_or_else(|_| unreachable!("first last-slot caller panicked")),
                second
                    .join()
                    .unwrap_or_else(|_| unreachable!("second last-slot caller panicked")),
            ]
        });
        assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
        assert_eq!(
            results
                .iter()
                .filter(|result| matches!(
                    result,
                    Err(ModelRouterCallFailureV1::Storage(
                        StoreError::ModelCallInFlight
                    )) | Err(ModelRouterCallFailureV1::ModelCallBudgetExceeded {
                        limit: 12,
                        observed: 12
                    })
                ))
                .count(),
            1
        );
        assert_eq!(provider.requests().len(), 1);
        assert_eq!(first_store.task_model_call_count(&task_id).ok(), Some(12));
        assert_eq!(first_store.task_model_turn_count(&task_id).ok(), Some(12));
        drop(process);
        drop(provider);
        drop(first_store);
        drop(second_store);
        let _ = std::fs::remove_file(&db_path);
        let _ = std::fs::remove_file(db_path.with_extension("sqlite-wal"));
        let _ = std::fs::remove_file(db_path.with_extension("sqlite-shm"));
    }

    #[test]
    fn dispatched_late_response_is_accounted_after_real_task_cancellation() {
        let db_path = std::env::temp_dir().join(format!(
            "serea-router-late-cancel-{}-{}.sqlite",
            std::process::id(),
            NEXT_RECOVERY_TEST_DB.fetch_add(1, Ordering::SeqCst)
        ));
        let setup = Store::open(&db_path, &FixedClock).unwrap_or_else(|_| unreachable!());
        let task_id =
            TaskId::new("tsk_00000000000000000000000074").unwrap_or_else(|_| unreachable!());
        assert!(insert_budget_task(&setup, task_id.clone(), "74"));
        drop(setup);

        let dispatch_store =
            Arc::new(Store::open(&db_path, &FixedClock).unwrap_or_else(|_| unreachable!()));
        let cancel_store =
            Arc::new(Store::open(&db_path, &FixedClock).unwrap_or_else(|_| unreachable!()));
        let (started_tx, started_rx) = mpsc::sync_channel(1);
        let (release_tx, release_rx) = mpsc::channel();
        let provider = Arc::new(BlockingProvider {
            started: started_tx,
            release: Mutex::new(release_rx),
            calls: AtomicUsize::new(0),
        });
        let process = Arc::new(blocking_process(
            provider.clone(),
            UsdMicros::new(10_000_000).unwrap_or_else(|_| unreachable!()),
        ));
        let dispatch_bus = EventBus::new(IncrementingIds(0));
        let dispatch_gate = StoreTaskGate {
            store: dispatch_store.clone(),
            cancel_on_snapshot: None,
            snapshots: AtomicUsize::new(0),
            suffix: "75",
        };
        let cancel_gate = StoreTaskGate {
            store: cancel_store.clone(),
            cancel_on_snapshot: None,
            snapshots: AtomicUsize::new(0),
            suffix: "76",
        };
        let dispatch_host = ModelRouterHostContextV1::new(
            &dispatch_store,
            &dispatch_bus,
            &FixedClock,
            &dispatch_gate,
        );
        let call = prepared_chat_for_task(task_id.clone());
        let response = std::thread::scope(|scope| {
            let call_process = process.clone();
            let call_ref = &call;
            let host_ref = &dispatch_host;
            let call_thread =
                scope.spawn(move || block_on(call_process.execute(call_ref, host_ref)));
            let request_id = started_rx
                .recv_timeout(Duration::from_secs(10))
                .unwrap_or_else(|_| unreachable!("provider was not entered"));
            cancel_gate
                .cancel_task(&task_id)
                .unwrap_or_else(|_| unreachable!("host cancellation failed"));
            release_tx
                .send(())
                .unwrap_or_else(|_| unreachable!("provider release channel closed"));
            let response = call_thread
                .join()
                .unwrap_or_else(|_| unreachable!("late provider call panicked"))
                .unwrap_or_else(|_| unreachable!("late response was not accounted"));
            (request_id, response)
        });
        assert_eq!(response.0, response.1.request_id);
        assert_eq!(provider.calls.load(Ordering::SeqCst), 1);
        assert_eq!(
            cancel_store
                .load_task(&task_id)
                .ok()
                .map(|task| task.task.state),
            Some(TaskState::Cancelled)
        );
        let attempt = cancel_store
            .get_model_call_attempt(&response.0)
            .unwrap_or_else(|_| unreachable!())
            .unwrap_or_else(|| unreachable!());
        assert_eq!(attempt.state, ModelAttemptState::Completed);
        assert!(
            cancel_store
                .model_usage_for_request(&response.0)
                .unwrap_or_else(|_| unreachable!())
                .is_some()
        );
        assert!(
            cancel_store
                .get_model_call_response(&response.0)
                .unwrap_or_else(|_| unreachable!())
                .is_some()
        );
        assert_eq!(cancel_store.task_model_call_count(&task_id).ok(), Some(1));
        assert_eq!(cancel_store.task_model_turn_count(&task_id).ok(), Some(1));
        let events =
            EventBus::replay(&cancel_store, None, None, 8).unwrap_or_else(|_| unreachable!());
        assert!(
            matches!(events.items.as_slice(), [ReplayItem::Event { event: called }, ReplayItem::Event { event: completed }] if called.kind == EventKind::ModelCalled && completed.kind == EventKind::ModelCompleted)
        );
        drop(process);
        drop(provider);
        drop(dispatch_store);
        drop(cancel_store);
        let _ = std::fs::remove_file(&db_path);
        let _ = std::fs::remove_file(db_path.with_extension("sqlite-wal"));
        let _ = std::fs::remove_file(db_path.with_extension("sqlite-shm"));
    }

    fn block_on<F: std::future::Future>(future: F) -> F::Output {
        let mut context = Context::from_waker(Waker::noop());
        let mut future = std::pin::pin!(future);
        match std::future::Future::poll(future.as_mut(), &mut context) {
            Poll::Ready(value) => value,
            Poll::Pending => unreachable!("model provider test future is immediately ready"),
        }
    }
}
