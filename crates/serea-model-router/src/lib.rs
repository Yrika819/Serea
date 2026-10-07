//! Deterministic host-owned model selection.
//!
//! This crate contains P4C routing only. It never calls `ModelProvider::generate`.
//! `PreparedModelCallV1::from_host` is a trusted in-process host boundary: it
//! does not prove redaction and must not be exposed to Android, network, or
//! other untrusted callers.

#![forbid(unsafe_code)]
#![deny(missing_docs)]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::collections::BTreeMap;
use std::io::{self, Write};
use std::sync::Arc;

use serea_protocol::provider::ModelProvider;
use serea_protocol::{
    DataClass, JsonSchemaMode, ModelCapabilities, ModelDescriptor, ModelId, ModelMessage,
    ModelPurpose, ProviderHealth, ProviderId, ResponseFormat, TaskId,
};

/// Maximum structural prompt size accepted before routing.
pub const MAX_MODEL_PROMPT_BYTES: usize = 1_048_576;
/// Maximum JSON Schema size accepted before routing.
pub const MAX_MODEL_SCHEMA_BYTES: usize = 65_536;
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
    pub decision: Option<ModelId>,
    /// One immutable health snapshot for this logical operation.
    pub health: ProviderHealthSnapshotV1,
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
}
