//! P4B fixed-point model cost primitives and durable accounting API.

use rusqlite::{Connection, OptionalExtension, params};
use serea_protocol::{
    CostClass, DataClass, Digest, EpochMillis, FinishReason, ModelId, ModelPurpose, ProviderId,
    RequestId, TaskId, TokenCount,
};

use crate::{BlobRef, Store, StoreError, Tx};

/// Maximum durable model dispatch intents charged to one Task in P4 V1.
pub const MAX_MODEL_CALLS_PER_TASK: u64 = 12;
/// Maximum top-level logical model operations charged as Task turns in P4 V1.
pub const MAX_MODEL_TURNS_PER_TASK: u64 = 12;
/// Maximum accepted response document size in bytes from Bounds Protocol.
pub const MAX_MODEL_RESPONSE_BYTES: usize = 262_144;
/// Maximum number of model attempt rows removed in one retention transaction.
pub const MAX_MODEL_RETENTION_BATCH: u16 = 512;
const ATTEMPT_RETENTION_MS: i64 = 30 * 24 * 60 * 60 * 1000;
const USAGE_RETENTION_MS: i64 = 365 * 24 * 60 * 60 * 1000;

/// Trusted deployment boundary configured by the host.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelDeploymentClass {
    /// A cloud deployment, including privately hosted cloud inference.
    Cloud,
    /// A local deployment.
    Local,
}

impl ModelDeploymentClass {
    const fn wire_name(self) -> &'static str {
        match self {
            Self::Cloud => "CLOUD",
            Self::Local => "LOCAL",
        }
    }
}

/// Durable provider-dispatch attempt state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelAttemptState {
    /// Commit proves durable intent only; it does not prove provider receipt.
    DispatchIntent,
    /// Provider result and host-validated accepted response were committed.
    Completed,
    /// A definite terminal failure was committed.
    Failed,
    /// Provider receipt/result is uncertain and the attempt is not retried.
    Ambiguous,
}

impl ModelAttemptState {
    const fn wire_name(self) -> &'static str {
        match self {
            Self::DispatchIntent => "DISPATCH_INTENT",
            Self::Completed => "COMPLETED",
            Self::Failed => "FAILED",
            Self::Ambiguous => "AMBIGUOUS",
        }
    }
}

/// Why a durable attempt follows its parent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelAttemptRelationKind {
    /// The attempt begins a normal logical operation.
    None,
    /// Deterministic normal-model fallback.
    Fallback,
    /// One structured-output repair attempt.
    Repair,
}

impl ModelAttemptRelationKind {
    const fn wire_name(self) -> &'static str {
        match self {
            Self::None => "NONE",
            Self::Fallback => "FALLBACK",
            Self::Repair => "REPAIR",
        }
    }
}

/// Integer day count from 1970-01-01T00:00:00Z using floor division.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct UtcAccountingDay(i64);

impl UtcAccountingDay {
    /// Derives the UTC calendar day from an explicit epoch-millisecond instant.
    pub fn from_epoch_millis(at: EpochMillis) -> Self {
        Self(at.get().div_euclid(86_400_000))
    }

    /// Returns signed days relative to the Unix epoch.
    pub const fn get(self) -> i64 {
        self.0
    }
}

/// Immutable trusted host facts used to create one dispatch intent.
#[derive(Debug, Clone)]
pub struct ModelCallAttemptDraft {
    /// Unique identifier for exactly one provider dispatch.
    pub request_id: RequestId,
    /// Owning Task, if this is not a maintenance call.
    pub task_id: Option<TaskId>,
    /// Host-assigned operation purpose.
    pub purpose: ModelPurpose,
    /// Configured model identity.
    pub model_id: ModelId,
    /// Configured provider identity.
    pub provider_id: ProviderId,
    /// Trusted configured deployment class.
    pub deployment_class: ModelDeploymentClass,
    /// Classification inherited from the prepared call.
    pub data_class: DataClass,
    /// Parent/fallback relationship for this attempt.
    pub relation_kind: ModelAttemptRelationKind,
    /// Parent attempt RequestId, if this is fallback or repair.
    pub parent_request_id: Option<RequestId>,
    /// Source model for a normal fallback attempt.
    pub fallback_from_model_id: Option<ModelId>,
    /// Immutable host price facts for this attempt.
    pub price: ModelPriceSnapshot,
    /// Configured model context capacity used for conservative reservation.
    pub max_context_tokens: u64,
    /// Effective output ceiling selected by the host for this request.
    pub effective_max_output_tokens: u64,
    /// Explicit host timestamp committed as dispatch intent.
    pub dispatch_intent_at: EpochMillis,
}

/// Already host-validated response document facts accepted for durable recovery.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelResponseStorage {
    /// SCJ-compatible JSON document representing accepted response content.
    pub canonical_json: Vec<u8>,
    /// Classification inherited from the prepared call or a trusted stronger rule.
    pub data_class: DataClass,
}

/// Trustworthy completed response facts supplied by the later router boundary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelCallCompletion {
    /// Provider-reported input count already validated by the host.
    pub input_tokens: TokenCount,
    /// Provider-reported output count already validated by the host.
    pub output_tokens: TokenCount,
    /// Non-negative integer latency in milliseconds.
    pub latency_ms: u64,
    /// Repair provider dispatches already consumed by this structured operation.
    pub repair_attempts: u8,
    /// Accepted provider finish reason.
    pub finish_reason: FinishReason,
    /// Explicit durable accounting timestamp.
    pub recorded_at: EpochMillis,
    /// Accepted response content; no prompt or unrelated context is stored.
    pub accepted_response: ModelResponseStorage,
}

/// Trustworthy metering facts returned with a definite failed provider call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelFailureUsage {
    /// Validated input token count, if the provider supplied trustworthy usage.
    pub input_tokens: TokenCount,
    /// Validated output token count, if the provider supplied trustworthy usage.
    pub output_tokens: TokenCount,
    /// Integer latency in milliseconds.
    pub latency_ms: u64,
    /// Repair attempts already consumed by the enclosing structured operation.
    pub repair_attempts: u8,
    /// Explicit usage record timestamp.
    pub recorded_at: EpochMillis,
}

/// Durable attempt facts returned to later routing/recovery phases.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelCallAttempt {
    /// One-dispatch RequestId.
    pub request_id: RequestId,
    /// Task association, nulled when the task is deleted.
    pub task_id: Option<TaskId>,
    /// Host-assigned purpose.
    pub purpose: ModelPurpose,
    /// Selected model identity.
    pub model_id: ModelId,
    /// Selected provider identity.
    pub provider_id: ProviderId,
    /// Host-owned deployment class.
    pub deployment_class: ModelDeploymentClass,
    /// Inherited data classification.
    pub data_class: DataClass,
    /// Durable attempt state.
    pub state: ModelAttemptState,
    /// Parent relation kind.
    pub relation_kind: ModelAttemptRelationKind,
    /// Parent RequestId for fallback/repair attempts.
    pub parent_request_id: Option<RequestId>,
    /// Source model for fallback attempts.
    pub fallback_from_model_id: Option<ModelId>,
    /// UTC day used for spend accounting.
    pub accounting_day: UtcAccountingDay,
    /// Immutable host price snapshot.
    pub price: ModelPriceSnapshot,
    /// Conservative reservation in micro-USD.
    pub reserved_cost_usd_micros: UsdMicros,
    /// Settled actual cost, when trustworthy usage exists.
    pub actual_cost_usd_micros: Option<UsdMicros>,
    /// Accepted response blob identity/class.
    pub response_blob: Option<BlobRef>,
    /// Whether task/privacy policy still permits retaining the response content.
    pub response_storage_allowed: bool,
    /// Dispatch intent instant.
    pub dispatch_intent_at: EpochMillis,
    /// Terminal instant, absent only while state is DISPATCH_INTENT.
    pub terminal_at: Option<EpochMillis>,
    /// Durable provider failure category, without free-text diagnostics.
    pub error_kind: Option<String>,
}

/// Durable trustworthy usage record; it contains accounting metadata only.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelUsageRecord {
    /// RequestId while its attempt detail row is retained.
    pub request_id: RequestId,
    /// Task association, nulled when the task is deleted.
    pub task_id: Option<TaskId>,
    /// Configured selected model.
    pub model_id: ModelId,
    /// Host-assigned purpose.
    pub purpose: ModelPurpose,
    /// Trustworthy input token count.
    pub input_tokens: TokenCount,
    /// Trustworthy output token count.
    pub output_tokens: TokenCount,
    /// Host-computed cost.
    pub cost_usd_micros: UsdMicros,
    /// Host-owned cost class.
    pub cost_class: CostClass,
    /// Integer response latency in milliseconds.
    pub latency_ms: u64,
    /// Structured repair attempts consumed by this call.
    pub repair_attempts: u8,
    /// Source model for fallback usage.
    pub fallback_from_model_id: Option<ModelId>,
    /// Explicit recorded-at instant.
    pub recorded_at: EpochMillis,
}

/// Durable non-negative integer USD amount in millionths of one US dollar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct UsdMicros(u64);

impl UsdMicros {
    /// Constructs a micro-USD amount representable by SQLite's signed INTEGER.
    pub fn new(value: u64) -> Result<Self, ModelAccountingError> {
        if value > i64::MAX as u64 {
            return Err(ModelAccountingError::Overflow);
        }
        Ok(Self(value))
    }

    /// Returns the integer micro-USD amount.
    pub const fn get(self) -> u64 {
        self.0
    }
}

/// Stable typed failure for model accounting operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelAccountingError {
    /// Checked arithmetic or SQLite INTEGER conversion overflowed.
    Overflow,
    /// Trusted price facts contradict the closed pricing contract.
    InvalidPriceSnapshot,
}

/// Immutable host-owned price facts snapshotted into each call attempt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelPriceSnapshot {
    cost_class: CostClass,
    price_revision: String,
    input_rate_microusd_per_million_tokens: u64,
    output_rate_microusd_per_million_tokens: u64,
}

impl ModelPriceSnapshot {
    /// Constructs the immutable price facts supplied by trusted host config.
    pub fn new(
        cost_class: CostClass,
        price_revision: impl Into<String>,
        input_rate_microusd_per_million_tokens: u64,
        output_rate_microusd_per_million_tokens: u64,
    ) -> Self {
        Self {
            cost_class,
            price_revision: price_revision.into(),
            input_rate_microusd_per_million_tokens,
            output_rate_microusd_per_million_tokens,
        }
    }

    /// Trusted cost class.
    pub const fn cost_class(&self) -> CostClass {
        self.cost_class
    }

    /// Immutable host price revision.
    pub fn price_revision(&self) -> &str {
        &self.price_revision
    }

    /// Input rate in integer micro-USD per million tokens.
    pub const fn input_rate_microusd_per_million_tokens(&self) -> u64 {
        self.input_rate_microusd_per_million_tokens
    }

    /// Output rate in integer micro-USD per million tokens.
    pub const fn output_rate_microusd_per_million_tokens(&self) -> u64 {
        self.output_rate_microusd_per_million_tokens
    }
}

/// Validates an immutable host price snapshot. Unknown prices must be rejected
/// by configuration before constructing this value; FREE is only valid at zero.
pub fn validate_price_snapshot(snapshot: &ModelPriceSnapshot) -> Result<(), ModelAccountingError> {
    let revision = snapshot.price_revision.as_bytes();
    if revision.is_empty()
        || revision.len() > 128
        || revision.iter().any(|byte| byte.is_ascii_control())
        || (snapshot.cost_class == CostClass::Free
            && (snapshot.input_rate_microusd_per_million_tokens != 0
                || snapshot.output_rate_microusd_per_million_tokens != 0))
        || snapshot.input_rate_microusd_per_million_tokens > i64::MAX as u64
        || snapshot.output_rate_microusd_per_million_tokens > i64::MAX as u64
    {
        return Err(ModelAccountingError::InvalidPriceSnapshot);
    }
    Ok(())
}

/// Computes actual host cost, rounding input and output charges upward
/// independently to the next micro-USD using integer arithmetic only.
pub fn calculate_cost_usd_micros(
    input_tokens: u64,
    output_tokens: u64,
    input_rate_microusd_per_million_tokens: u64,
    output_rate_microusd_per_million_tokens: u64,
) -> Result<UsdMicros, ModelAccountingError> {
    let input = component_cost(input_tokens, input_rate_microusd_per_million_tokens)?;
    let output = component_cost(output_tokens, output_rate_microusd_per_million_tokens)?;
    let total = input
        .checked_add(output)
        .ok_or(ModelAccountingError::Overflow)?;
    let total = u64::try_from(total).map_err(|_| ModelAccountingError::Overflow)?;
    UsdMicros::new(total)
}

/// Reserves the maximum charge allowed by model context and requested output
/// capacity; it deliberately does not estimate prompt tokenization.
pub fn calculate_reservation_usd_micros(
    max_context_tokens: u64,
    effective_max_output_tokens: u64,
    input_rate_microusd_per_million_tokens: u64,
    output_rate_microusd_per_million_tokens: u64,
) -> Result<UsdMicros, ModelAccountingError> {
    if max_context_tokens == 0
        || effective_max_output_tokens == 0
        || effective_max_output_tokens > max_context_tokens
    {
        return Err(ModelAccountingError::InvalidPriceSnapshot);
    }
    calculate_cost_usd_micros(
        max_context_tokens,
        effective_max_output_tokens,
        input_rate_microusd_per_million_tokens,
        output_rate_microusd_per_million_tokens,
    )
}

fn component_cost(tokens: u64, rate: u64) -> Result<u128, ModelAccountingError> {
    let product = u128::from(tokens)
        .checked_mul(u128::from(rate))
        .ok_or(ModelAccountingError::Overflow)?;
    let denominator = 1_000_000_u128;
    let quotient = product / denominator;
    let remainder = product % denominator;
    quotient
        .checked_add(u128::from(remainder != 0))
        .ok_or(ModelAccountingError::Overflow)
}

fn store_cost(value: Result<UsdMicros, ModelAccountingError>) -> Result<UsdMicros, StoreError> {
    value.map_err(|_| StoreError::ModelAccountingOverflow)
}

fn valid_code(code: &str) -> bool {
    !code.is_empty()
        && code.len() <= 128
        && code
            .bytes()
            .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit() || byte == b'_')
}

fn parse_class(value: i64) -> Result<DataClass, StoreError> {
    match value {
        0 => Ok(DataClass::Public),
        1 => Ok(DataClass::Personal),
        2 => Ok(DataClass::Private),
        3 => Ok(DataClass::Secret),
        4 => Ok(DataClass::Credential),
        _ => Err(StoreError::CorruptRow),
    }
}

fn parse_purpose(value: &str) -> Result<ModelPurpose, StoreError> {
    match value {
        "CHAT" => Ok(ModelPurpose::Chat),
        "PLANNING" => Ok(ModelPurpose::Planning),
        "EXTRACTION" => Ok(ModelPurpose::Extraction),
        "ANALYSIS" => Ok(ModelPurpose::Analysis),
        "PROACTIVE" => Ok(ModelPurpose::Proactive),
        "STRUCTURED_REPAIR" => Ok(ModelPurpose::StructuredRepair),
        _ => Err(StoreError::CorruptRow),
    }
}

fn parse_cost_class(value: &str) -> Result<CostClass, StoreError> {
    match value {
        "FREE" => Ok(CostClass::Free),
        "LOW" => Ok(CostClass::Low),
        "PAID" => Ok(CostClass::Paid),
        _ => Err(StoreError::CorruptRow),
    }
}

fn parse_state(value: &str) -> Result<ModelAttemptState, StoreError> {
    match value {
        "DISPATCH_INTENT" => Ok(ModelAttemptState::DispatchIntent),
        "COMPLETED" => Ok(ModelAttemptState::Completed),
        "FAILED" => Ok(ModelAttemptState::Failed),
        "AMBIGUOUS" => Ok(ModelAttemptState::Ambiguous),
        _ => Err(StoreError::CorruptRow),
    }
}

fn parse_relation(value: &str) -> Result<ModelAttemptRelationKind, StoreError> {
    match value {
        "NONE" => Ok(ModelAttemptRelationKind::None),
        "FALLBACK" => Ok(ModelAttemptRelationKind::Fallback),
        "REPAIR" => Ok(ModelAttemptRelationKind::Repair),
        _ => Err(StoreError::CorruptRow),
    }
}

fn parse_deployment(value: &str) -> Result<ModelDeploymentClass, StoreError> {
    match value {
        "CLOUD" => Ok(ModelDeploymentClass::Cloud),
        "LOCAL" => Ok(ModelDeploymentClass::Local),
        _ => Err(StoreError::CorruptRow),
    }
}

fn epoch(value: i64) -> Result<EpochMillis, StoreError> {
    EpochMillis::new(value).map_err(|_| StoreError::CorruptRow)
}

fn validate_draft(draft: &ModelCallAttemptDraft) -> Result<UsdMicros, StoreError> {
    if draft.data_class.rank() > DataClass::Personal.rank() {
        return Err(StoreError::ModelDataClassRefused);
    }
    validate_price_snapshot(&draft.price).map_err(|_| StoreError::InvalidModelCall)?;
    if draft.max_context_tokens == 0
        || draft.effective_max_output_tokens == 0
        || draft.effective_max_output_tokens > draft.max_context_tokens
        || draft.max_context_tokens > i64::MAX as u64
        || draft.effective_max_output_tokens > i64::MAX as u64
    {
        return Err(StoreError::InvalidModelCall);
    }
    let relation_valid = match draft.relation_kind {
        ModelAttemptRelationKind::None => {
            draft.parent_request_id.is_none() && draft.fallback_from_model_id.is_none()
        }
        ModelAttemptRelationKind::Fallback => {
            draft.parent_request_id.is_some()
                && draft.fallback_from_model_id.is_some()
                && draft.parent_request_id.as_ref() != Some(&draft.request_id)
        }
        ModelAttemptRelationKind::Repair => {
            draft.parent_request_id.is_some()
                && draft.fallback_from_model_id.is_none()
                && draft.parent_request_id.as_ref() != Some(&draft.request_id)
        }
    };
    if !relation_valid {
        return Err(StoreError::InvalidModelCall);
    }
    store_cost(calculate_reservation_usd_micros(
        draft.max_context_tokens,
        draft.effective_max_output_tokens,
        draft.price.input_rate_microusd_per_million_tokens,
        draft.price.output_rate_microusd_per_million_tokens,
    ))
}

fn read_attempt(
    conn: &Connection,
    request_id: &RequestId,
) -> Result<Option<ModelCallAttempt>, StoreError> {
    let row = conn
        .query_row(
            "SELECT request_id,task_id,purpose,model_id,provider_id,deployment_class,data_class_rank,
                    state,relation_kind,parent_request_id,fallback_from_model_id,accounting_day_utc,
                    cost_class,price_revision,input_rate_microusd_per_million,
                    output_rate_microusd_per_million,reserved_cost_usd_micros,
                    actual_cost_usd_micros,response_blob_digest,response_data_class_rank,
                    response_storage_allowed,
                    dispatch_intent_at_ms,terminal_at_ms,error_kind
             FROM model_call_attempts WHERE request_id=?1",
            [request_id.as_str()],
            |row| {
                Ok((
                    row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?,
                    row.get::<_, String>(2)?, row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?, row.get::<_, String>(5)?,
                    row.get::<_, i64>(6)?, row.get::<_, String>(7)?,
                    row.get::<_, String>(8)?, row.get::<_, Option<String>>(9)?,
                    row.get::<_, Option<String>>(10)?, row.get::<_, i64>(11)?,
                    row.get::<_, String>(12)?, row.get::<_, String>(13)?,
                    row.get::<_, i64>(14)?, row.get::<_, i64>(15)?,
                    row.get::<_, i64>(16)?, row.get::<_, Option<i64>>(17)?,
                    row.get::<_, Option<String>>(18)?, row.get::<_, Option<i64>>(19)?,
                    row.get::<_, i64>(20)?, row.get::<_, i64>(21)?,
                    row.get::<_, Option<i64>>(22)?, row.get::<_, Option<String>>(23)?,
                ))
            },
        )
        .optional()?;
    let Some((
        request,
        task,
        purpose,
        model,
        provider,
        deployment,
        class,
        state,
        relation,
        parent,
        fallback,
        day,
        cost_class,
        revision,
        input_rate,
        output_rate,
        reserved,
        actual,
        digest,
        response_class,
        response_storage_allowed,
        dispatched,
        terminal,
        error,
    )) = row
    else {
        return Ok(None);
    };
    let request_id = RequestId::new(request).map_err(|_| StoreError::CorruptRow)?;
    let task_id = task
        .map(TaskId::new)
        .transpose()
        .map_err(|_| StoreError::CorruptRow)?;
    let model_id = ModelId::new(model).map_err(|_| StoreError::CorruptRow)?;
    let provider_id = ProviderId::new(provider).map_err(|_| StoreError::CorruptRow)?;
    let parent_request_id = parent
        .map(RequestId::new)
        .transpose()
        .map_err(|_| StoreError::CorruptRow)?;
    let fallback_from_model_id = fallback
        .map(ModelId::new)
        .transpose()
        .map_err(|_| StoreError::CorruptRow)?;
    let price = ModelPriceSnapshot::new(
        parse_cost_class(&cost_class)?,
        revision,
        u64::try_from(input_rate).map_err(|_| StoreError::CorruptRow)?,
        u64::try_from(output_rate).map_err(|_| StoreError::CorruptRow)?,
    );
    validate_price_snapshot(&price).map_err(|_| StoreError::CorruptRow)?;
    let response_blob = match (digest, response_class) {
        (None, None) => None,
        (Some(digest), Some(class)) => {
            let digest = Digest::new(digest).map_err(|_| StoreError::CorruptRow)?;
            let class = parse_class(class)?;
            if class.rank() > DataClass::Personal.rank() {
                return Err(StoreError::CorruptRow);
            }
            Some(BlobRef::new(digest, class))
        }
        _ => return Err(StoreError::CorruptRow),
    };
    Ok(Some(ModelCallAttempt {
        request_id,
        task_id,
        purpose: parse_purpose(&purpose)?,
        model_id,
        provider_id,
        deployment_class: parse_deployment(&deployment)?,
        data_class: parse_class(class)?,
        state: parse_state(&state)?,
        relation_kind: parse_relation(&relation)?,
        parent_request_id,
        fallback_from_model_id,
        accounting_day: UtcAccountingDay(day),
        price,
        reserved_cost_usd_micros: UsdMicros::new(
            u64::try_from(reserved).map_err(|_| StoreError::CorruptRow)?,
        )
        .map_err(|_| StoreError::CorruptRow)?,
        actual_cost_usd_micros: actual
            .map(|value| {
                UsdMicros::new(u64::try_from(value).map_err(|_| StoreError::CorruptRow)?)
                    .map_err(|_| StoreError::CorruptRow)
            })
            .transpose()?,
        response_blob,
        response_storage_allowed: response_storage_allowed == 1,
        dispatch_intent_at: epoch(dispatched)?,
        terminal_at: terminal.map(epoch).transpose()?,
        error_kind: error,
    }))
}

fn read_usage(
    conn: &Connection,
    request_id: &RequestId,
) -> Result<Option<ModelUsageRecord>, StoreError> {
    let row = conn
        .query_row(
            "SELECT request_id,task_id,model_id,purpose,input_tokens,output_tokens,
                cost_usd_micros,cost_class,latency_ms,repair_attempts,
                fallback_from_model_id,recorded_at_ms
         FROM model_usage WHERE request_id=?1",
            [request_id.as_str()],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, Option<String>>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, i64>(4)?,
                    row.get::<_, i64>(5)?,
                    row.get::<_, i64>(6)?,
                    row.get::<_, String>(7)?,
                    row.get::<_, i64>(8)?,
                    row.get::<_, i64>(9)?,
                    row.get::<_, Option<String>>(10)?,
                    row.get::<_, i64>(11)?,
                ))
            },
        )
        .optional()?;
    let Some((
        request,
        task,
        model,
        purpose,
        input,
        output,
        cost,
        class,
        latency,
        repairs,
        fallback,
        recorded,
    )) = row
    else {
        return Ok(None);
    };
    Ok(Some(ModelUsageRecord {
        request_id: RequestId::new(request).map_err(|_| StoreError::CorruptRow)?,
        task_id: task
            .map(TaskId::new)
            .transpose()
            .map_err(|_| StoreError::CorruptRow)?,
        model_id: ModelId::new(model).map_err(|_| StoreError::CorruptRow)?,
        purpose: parse_purpose(&purpose)?,
        input_tokens: TokenCount::new(u64::try_from(input).map_err(|_| StoreError::CorruptRow)?),
        output_tokens: TokenCount::new(u64::try_from(output).map_err(|_| StoreError::CorruptRow)?),
        cost_usd_micros: UsdMicros::new(u64::try_from(cost).map_err(|_| StoreError::CorruptRow)?)
            .map_err(|_| StoreError::CorruptRow)?,
        cost_class: parse_cost_class(&class)?,
        latency_ms: u64::try_from(latency).map_err(|_| StoreError::CorruptRow)?,
        repair_attempts: u8::try_from(repairs).map_err(|_| StoreError::CorruptRow)?,
        fallback_from_model_id: fallback
            .map(ModelId::new)
            .transpose()
            .map_err(|_| StoreError::CorruptRow)?,
        recorded_at: epoch(recorded)?,
    }))
}

impl Store {
    /// Atomically reserves daily spend and task call capacity and writes one immutable dispatch intent.
    pub fn reserve_model_call(
        &self,
        draft: ModelCallAttemptDraft,
        max_daily_spend_usd_micros: UsdMicros,
    ) -> Result<ModelCallAttempt, StoreError> {
        self.transact(|tx| tx.reserve_model_call(draft, max_daily_spend_usd_micros))
    }

    /// Reads an attempt by its one-dispatch RequestId.
    pub fn get_model_call_attempt(
        &self,
        request_id: &RequestId,
    ) -> Result<Option<ModelCallAttempt>, StoreError> {
        let conn = self.conn.lock().map_err(|_| StoreError::LockPoisoned)?;
        read_attempt(&conn, request_id)
    }

    /// Lists the durable DISPATCH_INTENT rows requiring later recovery classification.
    pub fn list_unfinished_model_call_attempts(&self) -> Result<Vec<ModelCallAttempt>, StoreError> {
        let conn = self.conn.lock().map_err(|_| StoreError::LockPoisoned)?;
        let ids = conn.prepare(
            "SELECT request_id FROM model_call_attempts WHERE state='DISPATCH_INTENT' ORDER BY dispatch_intent_at_ms,request_id"
        )?.query_map([], |row| row.get::<_, String>(0))?
            .collect::<Result<Vec<_>, _>>()?;
        ids.into_iter()
            .map(|value| {
                let id = RequestId::new(value).map_err(|_| StoreError::CorruptRow)?;
                read_attempt(&conn, &id)?.ok_or(StoreError::CorruptRow)
            })
            .collect()
    }

    /// Returns the durable daily occupancy for one integer UTC day.
    pub fn utc_day_spend_occupancy(&self, day: UtcAccountingDay) -> Result<UsdMicros, StoreError> {
        let conn = self.conn.lock().map_err(|_| StoreError::LockPoisoned)?;
        let amount: i64 = conn
            .query_row(
                "SELECT COALESCE(SUM(CASE
                WHEN state IN ('DISPATCH_INTENT','AMBIGUOUS') OR actual_cost_usd_micros IS NULL
                  THEN reserved_cost_usd_micros
                ELSE actual_cost_usd_micros END),0)
             FROM model_call_attempts WHERE accounting_day_utc=?1",
                [day.get()],
                |row| row.get(0),
            )
            .map_err(|_| StoreError::ModelAccountingOverflow)?;
        UsdMicros::new(u64::try_from(amount).map_err(|_| StoreError::CorruptRow)?)
            .map_err(|_| StoreError::CorruptRow)
    }

    /// Counts every committed dispatch intent for the task, including failed and ambiguous attempts.
    pub fn task_model_call_count(&self, task_id: &TaskId) -> Result<u64, StoreError> {
        let conn = self.conn.lock().map_err(|_| StoreError::LockPoisoned)?;
        let count: Option<i64> = conn
            .query_row(
                "SELECT model_call_count FROM tasks WHERE task_id=?1",
                [task_id.as_str()],
                |row| row.get(0),
            )
            .optional()?;
        u64::try_from(count.ok_or(StoreError::TaskNotFound)?).map_err(|_| StoreError::CorruptRow)
    }

    /// Returns the durable count of primary top-level model operations.
    /// Fallback and repair attempts do not increment this counter.
    pub fn task_model_turn_count(&self, task_id: &TaskId) -> Result<u64, StoreError> {
        let conn = self.conn.lock().map_err(|_| StoreError::LockPoisoned)?;
        let count: Option<i64> = conn
            .query_row(
                "SELECT model_turn_count FROM tasks WHERE task_id=?1",
                [task_id.as_str()],
                |row| row.get(0),
            )
            .optional()?;
        u64::try_from(count.ok_or(StoreError::TaskNotFound)?).map_err(|_| StoreError::CorruptRow)
    }

    /// Sums only trustworthy token counts durably returned in model_usage.
    pub fn task_model_token_usage(&self, task_id: &TaskId) -> Result<TokenCount, StoreError> {
        let conn = self.conn.lock().map_err(|_| StoreError::LockPoisoned)?;
        let mut statement = conn.prepare(
            "SELECT input_tokens,output_tokens FROM model_usage WHERE task_id=?1 ORDER BY usage_id",
        )?;
        let mut rows = statement.query([task_id.as_str()])?;
        let mut total = 0_u64;
        while let Some(row) = rows.next()? {
            let input = u64::try_from(row.get::<_, i64>(0)?).map_err(|_| StoreError::CorruptRow)?;
            let output =
                u64::try_from(row.get::<_, i64>(1)?).map_err(|_| StoreError::CorruptRow)?;
            total = total
                .checked_add(input)
                .and_then(|value| value.checked_add(output))
                .ok_or(StoreError::ModelAccountingOverflow)?;
        }
        Ok(TokenCount::new(total))
    }

    /// Reads trustworthy usage while the associated attempt detail is retained.
    pub fn model_usage_for_request(
        &self,
        request_id: &RequestId,
    ) -> Result<Option<ModelUsageRecord>, StoreError> {
        let conn = self.conn.lock().map_err(|_| StoreError::LockPoisoned)?;
        read_usage(&conn, request_id)
    }

    /// Recovers the accepted response blob for a completed attempt.
    pub fn get_model_call_response(
        &self,
        request_id: &RequestId,
    ) -> Result<Option<Vec<u8>>, StoreError> {
        self.transact(|tx| {
            let Some(attempt) = read_attempt(&tx.inner, request_id)? else {
                return Ok(None);
            };
            let Some(blob) = attempt.response_blob else {
                return Ok(None);
            };
            tx.get_blob(&blob).map(Some)
        })
    }

    /// Atomically commits trustworthy usage, host cost settlement, response reference and COMPLETED.
    pub fn complete_model_call(
        &self,
        request_id: &RequestId,
        completion: ModelCallCompletion,
    ) -> Result<ModelCallAttempt, StoreError> {
        self.transact(|tx| tx.complete_model_call(request_id, completion))
    }

    /// Records a definite terminal provider failure without inventing usage.
    pub fn fail_model_call(
        &self,
        request_id: &RequestId,
        error_kind: &str,
        terminal_at: EpochMillis,
    ) -> Result<ModelCallAttempt, StoreError> {
        self.transact(|tx| tx.fail_model_call(request_id, error_kind, terminal_at))
    }

    /// Terminalizes a dispatch intent when the host proves no provider call
    /// occurred, settling spend to zero without inventing provider usage.
    pub fn fail_model_call_before_dispatch(
        &self,
        request_id: &RequestId,
        error_kind: &str,
        terminal_at: EpochMillis,
    ) -> Result<ModelCallAttempt, StoreError> {
        self.transact(|tx| tx.fail_model_call_before_dispatch(request_id, error_kind, terminal_at))
    }

    /// Records a definite failure and settles only explicitly trustworthy usage.
    pub fn fail_model_call_with_usage(
        &self,
        request_id: &RequestId,
        error_kind: &str,
        terminal_at: EpochMillis,
        usage: ModelFailureUsage,
    ) -> Result<ModelCallAttempt, StoreError> {
        self.transact(|tx| {
            tx.fail_model_call_with_usage(request_id, error_kind, terminal_at, usage)
        })
    }

    /// Marks a dispatch whose external outcome is uncertain; reservation remains occupied.
    pub fn mark_model_call_ambiguous(
        &self,
        request_id: &RequestId,
        error_kind: &str,
        terminal_at: EpochMillis,
    ) -> Result<ModelCallAttempt, StoreError> {
        self.transact(|tx| tx.mark_model_call_ambiguous(request_id, error_kind, terminal_at))
    }

    /// Removes bounded terminal attempt/result details after 30 days. Ambiguous
    /// attempts linked to an active Task remain available to later recovery.
    pub fn retain_model_call_attempts(
        &self,
        now: EpochMillis,
        limit: u16,
    ) -> Result<u64, StoreError> {
        if limit == 0 || limit > MAX_MODEL_RETENTION_BATCH {
            return Err(StoreError::InvalidModelRetention);
        }
        self.transact(|tx| {
            let cutoff = now.get().saturating_sub(ATTEMPT_RETENTION_MS);
            let candidates: Vec<(String, String, u8)> = tx
                .inner
                .prepare(
                    "SELECT request_id,response_blob_digest,response_data_class_rank
                     FROM model_call_attempts a
                     WHERE terminal_at_ms IS NOT NULL AND terminal_at_ms<?1
                       AND NOT (state='AMBIGUOUS' AND task_id IS NOT NULL AND EXISTS(
                         SELECT 1 FROM tasks t WHERE t.task_id=a.task_id
                           AND t.state NOT IN ('COMPLETED','FAILED','CANCELLED')))
                     ORDER BY terminal_at_ms,request_id LIMIT ?2",
                )?
                .query_map(params![cutoff, i64::from(limit)], |row| {
                    Ok((
                        row.get(0)?,
                        row.get::<_, Option<String>>(1)?.unwrap_or_default(),
                        row.get::<_, Option<u8>>(2)?.unwrap_or_default(),
                    ))
                })?
                .collect::<Result<_, _>>()?;
            let mut removed = 0_u64;
            let mut blobs = Vec::new();
            for (request, digest, rank) in candidates {
                if !digest.is_empty() {
                    blobs.push((digest, rank));
                }
                let deleted = u64::try_from(tx.inner.execute(
                    "DELETE FROM model_call_attempts WHERE request_id=?1 AND terminal_at_ms<?2",
                    params![request, cutoff],
                )?)
                .map_err(|_| StoreError::ModelAccountingOverflow)?;
                removed = removed
                    .checked_add(deleted)
                    .ok_or(StoreError::ModelAccountingOverflow)?;
            }
            crate::task::sweep_blob_candidates(&tx.inner, &blobs)?;
            Ok(removed)
        })
    }

    /// Removes usage metadata after 365 days; prompt/output content is never in this table.
    pub fn retain_model_usage(&self, now: EpochMillis, limit: u16) -> Result<u64, StoreError> {
        if limit == 0 || limit > MAX_MODEL_RETENTION_BATCH {
            return Err(StoreError::InvalidModelRetention);
        }
        self.transact(|tx| {
            let cutoff = now.get().saturating_sub(USAGE_RETENTION_MS);
            let deleted = tx.inner.execute(
                "DELETE FROM model_usage WHERE usage_id IN (
                   SELECT usage_id FROM model_usage WHERE recorded_at_ms<?1
                   ORDER BY recorded_at_ms,usage_id LIMIT ?2)",
                params![cutoff, i64::from(limit)],
            )?;
            u64::try_from(deleted).map_err(|_| StoreError::CorruptRow)
        })
    }
}

impl Tx<'_> {
    /// Reserves one immutable model dispatch intent inside this transaction.
    pub fn reserve_model_call(
        &mut self,
        draft: ModelCallAttemptDraft,
        max_daily_spend_usd_micros: UsdMicros,
    ) -> Result<ModelCallAttempt, StoreError> {
        self.ensure_active()?;
        let reservation = validate_draft(&draft)?;
        let exists: i64 = self.inner.query_row(
            "SELECT EXISTS(SELECT 1 FROM model_call_attempts WHERE request_id=?1)",
            [draft.request_id.as_str()],
            |row| row.get(0),
        )?;
        if exists != 0 {
            return Err(StoreError::DuplicateModelRequestId);
        }
        if let Some(task_id) = &draft.task_id {
            let active: i64 = self.inner.query_row(
                "SELECT EXISTS(SELECT 1 FROM model_call_attempts WHERE task_id=?1 AND state='DISPATCH_INTENT')",
                [task_id.as_str()],
                |row| row.get(0),
            )?;
            if active != 0 {
                return Err(StoreError::ModelCallInFlight);
            }
            let task_limits: Option<(i64, i64, i64)> = self
                .inner
                .query_row(
                    "SELECT max_model_calls,model_call_count,model_turn_count FROM tasks WHERE task_id=?1",
                    [task_id.as_str()],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
                )
                .optional()?;
            let (max_calls, used, turns) = task_limits.ok_or(StoreError::TaskNotFound)?;
            let limit = max_calls.min(MAX_MODEL_CALLS_PER_TASK as i64);
            if used >= limit {
                return Err(StoreError::ModelCallBudgetExceeded);
            }
            if draft.relation_kind == ModelAttemptRelationKind::None
                && turns >= MAX_MODEL_TURNS_PER_TASK as i64
            {
                return Err(StoreError::ModelTurnBudgetExceeded);
            }
        }
        let day = UtcAccountingDay::from_epoch_millis(draft.dispatch_intent_at);
        let occupied: i64 = self
            .inner
            .query_row(
                "SELECT COALESCE(SUM(CASE
                WHEN state IN ('DISPATCH_INTENT','AMBIGUOUS') OR actual_cost_usd_micros IS NULL
                  THEN reserved_cost_usd_micros
                ELSE actual_cost_usd_micros END),0)
             FROM model_call_attempts WHERE accounting_day_utc=?1",
                [day.get()],
                |row| row.get(0),
            )
            .map_err(|_| StoreError::ModelAccountingOverflow)?;
        let occupied = u64::try_from(occupied).map_err(|_| StoreError::CorruptRow)?;
        if occupied
            .checked_add(reservation.get())
            .ok_or(StoreError::ModelAccountingOverflow)?
            > max_daily_spend_usd_micros.get()
        {
            return Err(StoreError::DailySpendExceeded);
        }
        if let Some(task_id) = &draft.task_id {
            let is_primary_turn = i64::from(draft.relation_kind == ModelAttemptRelationKind::None);
            let changed = self.inner.execute(
                "UPDATE tasks SET model_call_count=model_call_count+1,
                                  model_turn_count=model_turn_count+?2
                 WHERE task_id=?1 AND model_call_count<min(max_model_calls,12)
                   AND (?2=0 OR model_turn_count<12)",
                params![task_id.as_str(), is_primary_turn],
            )?;
            if changed != 1 {
                return Err(if is_primary_turn == 1 {
                    StoreError::ModelTurnBudgetExceeded
                } else {
                    StoreError::ModelCallBudgetExceeded
                });
            }
        }
        self.inner.execute(
            "INSERT INTO model_call_attempts(
              request_id,task_id,purpose,model_id,provider_id,deployment_class,data_class_rank,
              state,relation_kind,parent_request_id,fallback_from_model_id,accounting_day_utc,
              cost_class,price_revision,input_rate_microusd_per_million,
              output_rate_microusd_per_million,max_context_tokens,effective_max_output_tokens,
              reserved_cost_usd_micros,dispatch_intent_at_ms)
             VALUES (?1,?2,?3,?4,?5,?6,?7,'DISPATCH_INTENT',?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19)",
            params![
                draft.request_id.as_str(),
                draft.task_id.as_ref().map(TaskId::as_str),
                draft.purpose.wire_name(),
                draft.model_id.as_str(),
                draft.provider_id.as_str(),
                draft.deployment_class.wire_name(),
                draft.data_class.rank(),
                draft.relation_kind.wire_name(),
                draft.parent_request_id.as_ref().map(RequestId::as_str),
                draft.fallback_from_model_id.as_ref().map(ModelId::as_str),
                day.get(),
                draft.price.cost_class.wire_name(),
                draft.price.price_revision,
                i64::try_from(draft.price.input_rate_microusd_per_million_tokens).map_err(|_| StoreError::ModelAccountingOverflow)?,
                i64::try_from(draft.price.output_rate_microusd_per_million_tokens).map_err(|_| StoreError::ModelAccountingOverflow)?,
                i64::try_from(draft.max_context_tokens).map_err(|_| StoreError::ModelAccountingOverflow)?,
                i64::try_from(draft.effective_max_output_tokens).map_err(|_| StoreError::ModelAccountingOverflow)?,
                i64::try_from(reservation.get()).map_err(|_| StoreError::ModelAccountingOverflow)?,
                draft.dispatch_intent_at.get(),
            ],
        ).map_err(|error| match StoreError::from(error) {
            StoreError::ConstraintViolation => StoreError::InvalidModelCall,
            other => other,
        })?;
        #[cfg(feature = "p2h-fault-injection")]
        crate::fault::reach(crate::fault::Window::AfterModelAttemptInsert)?;
        read_attempt(&self.inner, &draft.request_id)?.ok_or(StoreError::CorruptRow)
    }

    /// Commits accepted response, usage and settlement as one terminal state change.
    pub fn complete_model_call(
        &mut self,
        request_id: &RequestId,
        completion: ModelCallCompletion,
    ) -> Result<ModelCallAttempt, StoreError> {
        self.ensure_active()?;
        let mut attempt =
            read_attempt(&self.inner, request_id)?.ok_or(StoreError::ModelCallNotFound)?;
        if attempt.state != ModelAttemptState::DispatchIntent {
            return Err(StoreError::InvalidModelCallTransition);
        }
        if completion.accepted_response.data_class.rank() < attempt.data_class.rank() {
            return Err(StoreError::ModelDataClassRefused);
        }
        if completion.accepted_response.data_class.rank() > DataClass::Personal.rank() {
            return Err(StoreError::ModelDataClassRefused);
        }
        if completion.accepted_response.canonical_json.len() > MAX_MODEL_RESPONSE_BYTES {
            return Err(StoreError::ModelUsageExceedsBound);
        }
        if completion.output_tokens.get() > self.inner.query_row(
            "SELECT effective_max_output_tokens FROM model_call_attempts WHERE request_id=?1",
            [request_id.as_str()],
            |row| row.get::<_, i64>(0),
        ).map_err(|_| StoreError::CorruptRow)? as u64
            || completion.input_tokens.get() > self.inner.query_row(
                "SELECT max_context_tokens FROM model_call_attempts WHERE request_id=?1",
                [request_id.as_str()],
                |row| row.get::<_, i64>(0),
            ).map_err(|_| StoreError::CorruptRow)? as u64
            || completion.repair_attempts > 2
            || completion.latency_ms > i64::MAX as u64
        {
            return Err(StoreError::ModelUsageExceedsBound);
        }
        let actual = store_cost(calculate_cost_usd_micros(
            completion.input_tokens.get(),
            completion.output_tokens.get(),
            attempt.price.input_rate_microusd_per_million_tokens,
            attempt.price.output_rate_microusd_per_million_tokens,
        ))?;
        if actual > attempt.reserved_cost_usd_micros {
            return Err(StoreError::ModelUsageExceedsBound);
        }
        let response_blob = if attempt.response_storage_allowed {
            Some(self.put_blob(
                &completion.accepted_response.canonical_json,
                completion.accepted_response.data_class,
            )?)
        } else {
            None
        };
        #[cfg(feature = "p2h-fault-injection")]
        crate::fault::reach(crate::fault::Window::AfterModelResponseBlob)?;
        let response_rank = response_blob
            .as_ref()
            .map(|blob| i64::from(blob.class().rank()));
        self.inner.execute(
            "INSERT INTO model_usage(
              request_id,task_id,model_id,purpose,input_tokens,output_tokens,cost_usd_micros,
              cost_class,latency_ms,repair_attempts,fallback_from_model_id,recorded_at_ms)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12)",
            params![
                request_id.as_str(),
                attempt.task_id.as_ref().map(TaskId::as_str),
                attempt.model_id.as_str(),
                attempt.purpose.wire_name(),
                i64::try_from(completion.input_tokens.get())
                    .map_err(|_| StoreError::ModelAccountingOverflow)?,
                i64::try_from(completion.output_tokens.get())
                    .map_err(|_| StoreError::ModelAccountingOverflow)?,
                i64::try_from(actual.get()).map_err(|_| StoreError::ModelAccountingOverflow)?,
                attempt.price.cost_class.wire_name(),
                i64::try_from(completion.latency_ms)
                    .map_err(|_| StoreError::ModelAccountingOverflow)?,
                completion.repair_attempts,
                attempt.fallback_from_model_id.as_ref().map(ModelId::as_str),
                completion.recorded_at.get(),
            ],
        )?;
        #[cfg(feature = "p2h-fault-injection")]
        crate::fault::reach(crate::fault::Window::AfterModelUsageInsert)?;
        let changed = self.inner.execute(
            "UPDATE model_call_attempts
             SET actual_cost_usd_micros=?2,finish_reason=?3,response_blob_digest=?4,
                 response_data_class_rank=?5,state='COMPLETED',terminal_at_ms=?6
             WHERE request_id=?1 AND state='DISPATCH_INTENT'",
            params![
                request_id.as_str(),
                i64::try_from(actual.get()).map_err(|_| StoreError::ModelAccountingOverflow)?,
                completion.finish_reason.wire_name(),
                response_blob.as_ref().map(|blob| blob.digest().as_str()),
                response_rank,
                completion.recorded_at.get(),
            ],
        )?;
        if changed != 1 {
            return Err(StoreError::InvalidModelCallTransition);
        }
        #[cfg(feature = "p2h-fault-injection")]
        crate::fault::reach(crate::fault::Window::AfterModelAttemptTerminal)?;
        attempt = read_attempt(&self.inner, request_id)?.ok_or(StoreError::CorruptRow)?;
        Ok(attempt)
    }

    /// Records a definite failure while preserving full reservation when usage is unknown.
    pub fn fail_model_call(
        &mut self,
        request_id: &RequestId,
        error_kind: &str,
        terminal_at: EpochMillis,
    ) -> Result<ModelCallAttempt, StoreError> {
        self.terminal_model_call(
            request_id,
            error_kind,
            terminal_at,
            ModelAttemptState::Failed,
            None,
        )
    }

    /// Terminalizes an intent when the provider is known not to have been
    /// invoked. No synthetic usage row is written and reserved spend settles
    /// to zero.
    pub fn fail_model_call_before_dispatch(
        &mut self,
        request_id: &RequestId,
        error_kind: &str,
        terminal_at: EpochMillis,
    ) -> Result<ModelCallAttempt, StoreError> {
        self.ensure_active()?;
        if !valid_code(error_kind) {
            return Err(StoreError::InvalidModelCall);
        }
        let attempt =
            read_attempt(&self.inner, request_id)?.ok_or(StoreError::ModelCallNotFound)?;
        if attempt.state != ModelAttemptState::DispatchIntent {
            return Err(StoreError::InvalidModelCallTransition);
        }
        let changed = self.inner.execute(
            "UPDATE model_call_attempts SET state='FAILED',error_kind=?2,terminal_at_ms=?3,
                                             actual_cost_usd_micros=0
             WHERE request_id=?1 AND state='DISPATCH_INTENT'",
            params![request_id.as_str(), error_kind, terminal_at.get()],
        )?;
        if changed != 1 {
            return Err(StoreError::InvalidModelCallTransition);
        }
        #[cfg(feature = "p2h-fault-injection")]
        crate::fault::reach(crate::fault::Window::AfterModelAttemptTerminal)?;
        read_attempt(&self.inner, request_id)?.ok_or(StoreError::CorruptRow)
    }

    /// Records metering facts with a definite failure without persisting content.
    pub fn fail_model_call_with_usage(
        &mut self,
        request_id: &RequestId,
        error_kind: &str,
        terminal_at: EpochMillis,
        usage: ModelFailureUsage,
    ) -> Result<ModelCallAttempt, StoreError> {
        self.terminal_model_call(
            request_id,
            error_kind,
            terminal_at,
            ModelAttemptState::Failed,
            Some(usage),
        )
    }

    /// Marks a dispatch ambiguous while preserving its full reservation.
    pub fn mark_model_call_ambiguous(
        &mut self,
        request_id: &RequestId,
        error_kind: &str,
        terminal_at: EpochMillis,
    ) -> Result<ModelCallAttempt, StoreError> {
        self.terminal_model_call(
            request_id,
            error_kind,
            terminal_at,
            ModelAttemptState::Ambiguous,
            None,
        )
    }

    fn terminal_model_call(
        &mut self,
        request_id: &RequestId,
        error_kind: &str,
        terminal_at: EpochMillis,
        state: ModelAttemptState,
        usage: Option<ModelFailureUsage>,
    ) -> Result<ModelCallAttempt, StoreError> {
        self.ensure_active()?;
        if !valid_code(error_kind) {
            return Err(StoreError::InvalidModelCall);
        }
        let attempt =
            read_attempt(&self.inner, request_id)?.ok_or(StoreError::ModelCallNotFound)?;
        if attempt.state != ModelAttemptState::DispatchIntent {
            return Err(StoreError::InvalidModelCallTransition);
        }
        let actual = if let Some(usage) = &usage {
            if usage.output_tokens.get() > self.inner.query_row(
                "SELECT effective_max_output_tokens FROM model_call_attempts WHERE request_id=?1",
                [request_id.as_str()], |row| row.get::<_, i64>(0),
            ).map_err(|_| StoreError::CorruptRow)? as u64
                || usage.input_tokens.get() > self.inner.query_row(
                    "SELECT max_context_tokens FROM model_call_attempts WHERE request_id=?1",
                    [request_id.as_str()], |row| row.get::<_, i64>(0),
                ).map_err(|_| StoreError::CorruptRow)? as u64
                || usage.latency_ms > i64::MAX as u64
                || usage.repair_attempts > 2
            {
                return Err(StoreError::ModelUsageExceedsBound);
            }
            let cost = store_cost(calculate_cost_usd_micros(
                usage.input_tokens.get(),
                usage.output_tokens.get(),
                attempt.price.input_rate_microusd_per_million_tokens,
                attempt.price.output_rate_microusd_per_million_tokens,
            ))?;
            if cost > attempt.reserved_cost_usd_micros {
                return Err(StoreError::ModelUsageExceedsBound);
            }
            self.inner.execute(
                "INSERT INTO model_usage(
                  request_id,task_id,model_id,purpose,input_tokens,output_tokens,cost_usd_micros,
                  cost_class,latency_ms,repair_attempts,fallback_from_model_id,recorded_at_ms)
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12)",
                params![
                    request_id.as_str(),
                    attempt.task_id.as_ref().map(TaskId::as_str),
                    attempt.model_id.as_str(),
                    attempt.purpose.wire_name(),
                    i64::try_from(usage.input_tokens.get())
                        .map_err(|_| StoreError::ModelAccountingOverflow)?,
                    i64::try_from(usage.output_tokens.get())
                        .map_err(|_| StoreError::ModelAccountingOverflow)?,
                    i64::try_from(cost.get()).map_err(|_| StoreError::ModelAccountingOverflow)?,
                    attempt.price.cost_class.wire_name(),
                    i64::try_from(usage.latency_ms)
                        .map_err(|_| StoreError::ModelAccountingOverflow)?,
                    usage.repair_attempts,
                    attempt.fallback_from_model_id.as_ref().map(ModelId::as_str),
                    usage.recorded_at.get(),
                ],
            )?;
            Some(cost)
        } else {
            None
        };
        let changed = self.inner.execute(
            "UPDATE model_call_attempts SET state=?2,error_kind=?3,terminal_at_ms=?4,
                                            actual_cost_usd_micros=?5
             WHERE request_id=?1 AND state='DISPATCH_INTENT'",
            params![
                request_id.as_str(),
                state.wire_name(),
                error_kind,
                terminal_at.get(),
                actual
                    .map(|value| i64::try_from(value.get()))
                    .transpose()
                    .map_err(|_| StoreError::ModelAccountingOverflow)?
            ],
        )?;
        if changed != 1 {
            let exists: i64 = self.inner.query_row(
                "SELECT EXISTS(SELECT 1 FROM model_call_attempts WHERE request_id=?1)",
                [request_id.as_str()],
                |row| row.get(0),
            )?;
            return Err(if exists == 0 {
                StoreError::ModelCallNotFound
            } else {
                StoreError::InvalidModelCallTransition
            });
        }
        read_attempt(&self.inner, request_id)?.ok_or(StoreError::CorruptRow)
    }
}

pub(crate) fn validate_model_storage_integrity(conn: &Connection) -> Result<(), StoreError> {
    let inconsistent: i64 = conn.query_row(
        "SELECT
           (SELECT count(*) FROM model_call_attempts a
             WHERE (a.cost_class='FREE' AND
                    (a.input_rate_microusd_per_million<>0 OR a.output_rate_microusd_per_million<>0))
                OR (a.state='COMPLETED' AND NOT EXISTS(
                    SELECT 1 FROM model_usage u WHERE u.request_id=a.request_id))
                OR (a.state='COMPLETED' AND a.response_storage_allowed=1 AND a.response_blob_digest IS NULL)
                OR (a.state='DISPATCH_INTENT' AND a.terminal_at_ms IS NOT NULL)
                OR (a.state<>'DISPATCH_INTENT' AND a.terminal_at_ms IS NULL))
           +
           (SELECT count(*) FROM model_usage u JOIN model_call_attempts a ON a.request_id=u.request_id
             WHERE a.state NOT IN ('COMPLETED','FAILED') OR u.model_id<>a.model_id OR u.purpose<>a.purpose
                OR u.task_id IS NOT a.task_id
                OR u.cost_class<>a.cost_class
                OR u.output_tokens>a.effective_max_output_tokens)
           +
           (SELECT count(*) FROM tasks t
             WHERE t.model_call_count>t.max_model_calls OR t.model_call_count>12
                OR t.model_turn_count>12 OR t.model_turn_count>t.model_call_count
                OR t.model_call_count<(SELECT count(*) FROM model_call_attempts a WHERE a.task_id=t.task_id))",
        [],
        |row| row.get(0),
    )?;
    if inconsistent != 0 {
        return Err(StoreError::IntegrityCheckFailed);
    }
    let mut statement = conn.prepare(
        "SELECT a.request_id,u.input_tokens,u.output_tokens,u.cost_usd_micros,
                a.input_rate_microusd_per_million,a.output_rate_microusd_per_million,
                a.actual_cost_usd_micros
         FROM model_usage u JOIN model_call_attempts a ON a.request_id=u.request_id",
    )?;
    let mut rows = statement.query([])?;
    while let Some(row) = rows.next()? {
        let input = u64::try_from(row.get::<_, i64>(1)?).map_err(|_| StoreError::CorruptRow)?;
        let output = u64::try_from(row.get::<_, i64>(2)?).map_err(|_| StoreError::CorruptRow)?;
        let input_rate =
            u64::try_from(row.get::<_, i64>(4)?).map_err(|_| StoreError::CorruptRow)?;
        let output_rate =
            u64::try_from(row.get::<_, i64>(5)?).map_err(|_| StoreError::CorruptRow)?;
        let expected = store_cost(calculate_cost_usd_micros(
            input,
            output,
            input_rate,
            output_rate,
        ))?;
        let usage_cost =
            u64::try_from(row.get::<_, i64>(3)?).map_err(|_| StoreError::CorruptRow)?;
        let actual_cost =
            u64::try_from(row.get::<_, i64>(6)?).map_err(|_| StoreError::CorruptRow)?;
        if expected.get() != usage_cost || expected.get() != actual_cost {
            return Err(StoreError::IntegrityCheckFailed);
        }
    }
    Ok(())
}
