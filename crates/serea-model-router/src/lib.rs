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
    EventBus, ModelCompletedEventV1, ModelEventMetadataV1, ModelEventRelationV1, ModelFailedEventV1,
};
use serea_protocol::provider::{ModelCallContext, ModelProvider};
use serea_protocol::{
    Clock, DataClass, FinishReason, JsonSchemaMode, ModelCapabilities, ModelDescriptor,
    ModelErrorCode, ModelId, ModelMessage, ModelPurpose, ModelRequest, ModelResponse,
    ProtocolError, ProviderHealth, ProviderId, ResponseFormat, TaskId, canonicalize,
};
use serea_storage::{
    ModelAttemptRelationKind, ModelAttemptState, ModelCallAttemptDraft, ModelCallCompletion,
    ModelFailureUsage, ModelPriceSnapshot, ModelResponseStorage, Store, StoreError, UsdMicros,
    calculate_cost_usd_micros,
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
}

// The retryable/terminal details are consumed by the P4E orchestration ladder.
#[allow(dead_code)]
#[derive(Debug)]
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
    max_daily_spend_usd_micros: UsdMicros,
    clock: &'a dyn Clock,
}

struct ModelFailureFacts<'a> {
    request_id: &'a serea_protocol::RequestId,
    metadata: ModelEventMetadataV1,
    error_kind: ModelErrorCode,
    retryable: bool,
    terminal_at: serea_protocol::EpochMillis,
    usage: Option<ModelFailureUsage>,
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

    // Kept crate-private until P4E wraps all retries inside the router ladder.
    #[allow(dead_code)]
    pub(crate) async fn dispatch_chat_text(
        &self,
        call: &PreparedModelCallV1,
        session: &RoutingSessionV1,
        context: &ModelDispatchContext<'_>,
    ) -> Result<ModelResponse, ModelDispatchFailure> {
        let store = context.store;
        let events = context.events;
        let price = &context.price;
        let max_daily_spend_usd_micros = context.max_daily_spend_usd_micros;
        let clock = context.clock;
        if call.purpose != ModelPurpose::Chat
            || !matches!(call.response_format, ResponseFormat::Text)
        {
            return Err(ModelDispatchFailure::Refused(
                RouterError::IllegalPurposeFormat,
            ));
        }
        let selected = session
            .decision()
            .ok_or(ModelDispatchFailure::NoEligibleModel)?
            .clone();
        let entry =
            self.roster
                .entries
                .get(selected.as_str())
                .ok_or(ModelDispatchFailure::Refused(
                    RouterError::ModelSelectionInvalid,
                ))?;
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
        let request = self
            .build_request(call, session, &request_id, &selected)
            .map_err(ModelDispatchFailure::Refused)?;
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
            relation: ModelEventRelationV1::Normal,
            data_class: call.data_class,
            occurred_at: dispatch_at,
        };
        let called = events
            .draft_model_called(metadata.clone())
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
            relation_kind: ModelAttemptRelationKind::None,
            parent_request_id: None,
            fallback_from_model_id: None,
            price: price.clone(),
            max_context_tokens: u64::from(effective.max_context_tokens),
            effective_max_output_tokens: u64::from(effective_max_output_tokens),
            dispatch_intent_at: dispatch_at,
        };
        store
            .transact(|tx| {
                tx.reserve_model_call(attempt, max_daily_spend_usd_micros)?;
                tx.append_event(called.event, called.retention_at)?;
                Ok(())
            })
            .map_err(ModelDispatchFailure::Storage)?;

        let response = match provider
            .generate(
                &request,
                &ModelCallContext {
                    deadline_ms: call.deadline_ms,
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
            && response.content.len() <= serea_storage::MAX_MODEL_RESPONSE_BYTES;
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
            return Err(ModelDispatchFailure::TerminalProvider {
                request_id,
                error_kind,
            });
        }
        if response.finish_reason != FinishReason::Stop {
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
                repair_attempts: 0,
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
            return Err(ModelDispatchFailure::TerminalProvider {
                request_id,
                error_kind,
            });
        }
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
                    ..metadata
                },
                finish_reason: response.finish_reason,
                input_tokens: response.usage.input_tokens.get(),
                output_tokens: response.usage.output_tokens.get(),
                cost_class: price.cost_class(),
                cost_usd_micros: actual_cost.get(),
                price_revision: price.price_revision().to_owned(),
                repair_attempts: 0,
            })
            .map_err(ModelDispatchFailure::Storage)?;
        let response_json = serde_json::json!({"content": response.content.clone()});
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
                        repair_attempts: 0,
                        finish_reason: response.finish_reason,
                        recorded_at: completed_at,
                        accepted_response: ModelResponseStorage {
                            canonical_json: accepted_json,
                            data_class: call.data_class,
                        },
                    },
                )?;
                tx.append_event(completed.event, completed.retention_at)?;
                Ok(())
            })
            .map_err(ModelDispatchFailure::Storage)?;
        Ok(response)
    }
}

#[allow(dead_code)] // Used by dispatch error paths; exercised by P4D scripted failures.
fn stable_model_error(value: &str) -> Result<ModelErrorCode, ModelDispatchFailure> {
    ModelErrorCode::new(value).map_err(ModelDispatchFailure::Protocol)
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
    _egress: ModelEgressPolicySnapshotV1,
) -> bool {
    (match data_class {
        DataClass::Public => true,
        // Prepared calls only enter through the trusted host seam.
        DataClass::Personal => true,
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
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use std::task::{Context, Poll, Waker};

    use async_trait::async_trait;
    use serea_event_bus::{EventBus, ReplayItem};
    use serea_protocol::provider::ModelCallContext;
    use serea_protocol::{
        Clock, CostClass, DataClass, EpochMillis, EventKind, FinishReason, ModelError,
        ModelErrorCode, ModelMessage, ModelResponse, ModelUsage, ProtocolError, ProviderId,
        ResponseFormat, TimestampMs, TokenCount, UlidSource, UlidValue,
    };
    use serea_storage::{
        ModelAttemptRelationKind, ModelAttemptState, ModelCallAttemptDraft,
        ModelDeploymentClass as StorageDeploymentClass, ModelPriceSnapshot, Store, StoreError,
        UsdMicros,
    };

    struct FixedClock;

    impl Clock for FixedClock {
        fn now_ms(&self) -> Result<EpochMillis, ProtocolError> {
            EpochMillis::new(1_767_225_600_000)
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
            max_daily_spend_usd_micros,
            clock: &FixedClock,
        }
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
        finish_next: Mutex<Option<FinishReason>>,
        last_request_id: Mutex<Option<serea_protocol::RequestId>>,
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
            _ctx: &ModelCallContext,
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
                content: "hello".into(),
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
            finish_next: Mutex::new(None),
            last_request_id: Mutex::new(None),
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
        let response = block_on(router.dispatch_chat_text(&call, &session, &context))
            .unwrap_or_else(|_| unreachable!());

        assert_eq!(provider.calls.load(Ordering::SeqCst), 1);
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
            UsdMicros::new(0).unwrap_or_else(|_| unreachable!()),
        );
        let reservation_failure =
            block_on(router.dispatch_chat_text(&call, &session, &no_spend_context));
        assert!(matches!(
            reservation_failure,
            Err(ModelDispatchFailure::Storage(
                StoreError::DailySpendExceeded
            ))
        ));
        assert_eq!(provider.calls.load(Ordering::SeqCst), 1);
        assert_eq!(
            EventBus::replay(&store, None, None, 16)
                .unwrap_or_else(|_| unreachable!())
                .items
                .len(),
            2
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
        assert_eq!(reopened_events.items.len(), 22);
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

    fn block_on<F: std::future::Future>(future: F) -> F::Output {
        let mut context = Context::from_waker(Waker::noop());
        let mut future = std::pin::pin!(future);
        match std::future::Future::poll(future.as_mut(), &mut context) {
            Poll::Ready(value) => value,
            Poll::Pending => unreachable!("model provider test future is immediately ready"),
        }
    }
}
