//! `ClassifiedArgumentsV1` and `PreparedActionV1` (ADR-0035).
//!
//! Classification crosses a trusted boundary: the class is supplied by the
//! host from provenance, never by model JSON, never by a field name, and never
//! lowered. A `PreparedActionV1` is structurally valid and fully pinned. It is
//! not an approval, not a policy outcome and not an executable request: there
//! is no RequestId and no execution authority anywhere in this type.

use std::fmt;

use serde_json::{Map, Value};
use serea_protocol::{
    Authorization, CanonicalJsonError, CapabilityId, CostClass, DataClass, Digest, IdempotencyKey,
    IdempotencySupport, ImplementationId, ProviderId, ReplaySafety, RequestedBy, RiskClass,
    RootRequirement, SemVer, SideEffectClass, StepId, TaskId,
};

use crate::availability::{CapabilityAvailabilitySnapshotV1, ResolveError};

/// Arguments plus the trusted class of the source they came from.
///
/// The only constructor takes an explicit trusted class. There is no way to
/// build one from model output, and no field-name heuristic can prove safety.
#[derive(Clone, PartialEq, Eq)]
pub struct ClassifiedArgumentsV1 {
    arguments: Map<String, Value>,
    data_class: DataClass,
}

impl fmt::Debug for ClassifiedArgumentsV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ClassifiedArgumentsV1")
            .field("data_class", &self.data_class)
            .field("argument_field_count", &self.arguments.len())
            .finish()
    }
}

impl ClassifiedArgumentsV1 {
    /// Wraps already-validated arguments with the class of their trusted
    /// source. `Unknown` provenance is expressed by passing
    /// [`DataClass::Credential`], the fail-closed default.
    pub fn new_trusted(arguments: Map<String, Value>, data_class: DataClass) -> Self {
        Self {
            arguments,
            data_class,
        }
    }

    pub fn arguments(&self) -> &Map<String, Value> {
        &self.arguments
    }

    /// The exact trusted class of these arguments.
    pub fn data_class(&self) -> DataClass {
        self.data_class
    }
}

/// Why preparation failed. No `ActionResult` is ever fabricated for these.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PreparationError {
    /// The capability is not present in the frozen generation.
    Unknown { capability_id: CapabilityId },
    /// A known capability has no usable implementation right now.
    Unavailable { capability_id: CapabilityId },
    /// Persisted or advertised facts contradict the manifest.
    ContractFailure { capability_id: CapabilityId },
    /// CREDENTIAL-class arguments are refused at this boundary.
    CredentialClassRefused,
    /// The trusted argument class exceeds the descriptor ceiling.
    DataClassAboveCeiling {
        argument_class: DataClass,
        descriptor_ceiling: DataClass,
    },
    /// Arguments failed the descriptor's trusted input schema.
    SchemaInvalid,
    /// The arguments cannot be represented canonically, so no digest or IDK
    /// can be derived. Values are never rounded or coerced.
    ArgumentsNotCanonical,
    /// Every host deadline bound left no effective deadline.
    NoEffectiveDeadline,
    /// The descriptor revision digest could not be resolved for the bound
    /// candidate.
    DescriptorRevisionUnavailable,
}

impl fmt::Display for PreparationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Stable codes and classes only: never arguments, never model text.
        match self {
            Self::Unknown { capability_id } => {
                write!(f, "CAPABILITY_UNKNOWN:{capability_id}")
            }
            Self::Unavailable { capability_id } => {
                write!(f, "CAPABILITY_UNAVAILABLE:{capability_id}")
            }
            Self::ContractFailure { capability_id } => {
                write!(f, "CONTRACT_FAILURE:{capability_id}")
            }
            Self::CredentialClassRefused => f.write_str("CREDENTIAL_CLASS_REFUSED"),
            Self::DataClassAboveCeiling {
                argument_class,
                descriptor_ceiling,
            } => write!(
                f,
                "DATA_CLASS_ABOVE_CEILING:{argument_class}>{descriptor_ceiling}"
            ),
            Self::SchemaInvalid => f.write_str("ARGUMENTS_SCHEMA_INVALID"),
            Self::ArgumentsNotCanonical => f.write_str("ARGUMENTS_NOT_CANONICAL"),
            Self::NoEffectiveDeadline => f.write_str("NO_EFFECTIVE_DEADLINE"),
            Self::DescriptorRevisionUnavailable => f.write_str("DESCRIPTOR_REVISION_UNAVAILABLE"),
        }
    }
}
impl std::error::Error for PreparationError {}

/// The immutable P5 output handed to P6.
#[derive(Clone, PartialEq)]
pub struct PreparedActionV1 {
    task_id: TaskId,
    step_id: StepId,
    generation_digest: Digest,
    generation_id: i64,
    descriptor_digest: Digest,
    capability_id: CapabilityId,
    capability_version: SemVer,
    provider_id: ProviderId,
    implementation_id: Option<ImplementationId>,
    arguments: Map<String, Value>,
    arguments_digest: Digest,
    idempotency_key: IdempotencyKey,
    data_class: DataClass,
    requested_by: RequestedBy,
    deadline_ms: u32,
    side_effect_class: SideEffectClass,
    risk_class: RiskClass,
    required_authorization: Authorization,
    replay_safety: ReplaySafety,
    root_requirement: RootRequirement,
    idempotency_support: IdempotencySupport,
    cost_class: CostClass,
}

impl fmt::Debug for PreparedActionV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PreparedActionV1")
            .field("task_id", &self.task_id)
            .field("step_id", &self.step_id)
            .field("generation_id", &self.generation_id)
            .field("capability_id", &self.capability_id)
            .field("data_class", &self.data_class)
            .field("arguments_digest", &self.arguments_digest)
            .field("argument_field_count", &self.arguments.len())
            .finish()
    }
}

impl PreparedActionV1 {
    pub fn task_id(&self) -> &TaskId {
        &self.task_id
    }
    pub fn step_id(&self) -> &StepId {
        &self.step_id
    }
    /// The registry generation this preparation is pinned to.
    pub fn generation_digest(&self) -> &Digest {
        &self.generation_digest
    }
    /// Exact durable generation used for resolution and binding.
    pub fn generation_id(&self) -> i64 {
        self.generation_id
    }
    /// The exact descriptor revision this preparation is pinned to.
    pub fn descriptor_digest(&self) -> &Digest {
        &self.descriptor_digest
    }
    pub fn capability_id(&self) -> &CapabilityId {
        &self.capability_id
    }
    pub fn capability_version(&self) -> &SemVer {
        &self.capability_version
    }
    pub fn provider_id(&self) -> &ProviderId {
        &self.provider_id
    }
    pub fn implementation_id(&self) -> Option<&ImplementationId> {
        self.implementation_id.as_ref()
    }
    pub fn arguments(&self) -> &Map<String, Value> {
        &self.arguments
    }
    pub fn arguments_digest(&self) -> &Digest {
        &self.arguments_digest
    }
    pub fn idempotency_key(&self) -> &IdempotencyKey {
        &self.idempotency_key
    }
    /// The exact trusted argument class, not the descriptor ceiling.
    pub fn data_class(&self) -> DataClass {
        self.data_class
    }
    /// Provenance for audit only; grants nothing.
    pub fn requested_by(&self) -> RequestedBy {
        self.requested_by
    }
    /// The effective host deadline in milliseconds.
    pub fn deadline_ms(&self) -> u32 {
        self.deadline_ms
    }
    pub fn side_effect_class(&self) -> SideEffectClass {
        self.side_effect_class
    }
    pub fn risk_class(&self) -> RiskClass {
        self.risk_class
    }
    pub fn required_authorization(&self) -> Authorization {
        self.required_authorization
    }
    pub fn replay_safety(&self) -> ReplaySafety {
        self.replay_safety
    }
    pub fn root_requirement(&self) -> RootRequirement {
        self.root_requirement
    }
    pub fn idempotency_support(&self) -> IdempotencySupport {
        self.idempotency_support
    }
    pub fn cost_class(&self) -> CostClass {
        self.cost_class
    }

    /// A stable type marker used by tests and diagnostics. It deliberately
    /// names no request, approval or execution concept.
    pub fn type_id(&self) -> &'static str {
        "serea.prepared-action/1"
    }
}

/// Prepares one action from a frozen availability snapshot.
///
/// Order: resolve the capability, validate the trusted argument class against
/// the descriptor ceiling, validate arguments against the descriptor's trusted
/// input schema, canonicalize and digest, derive IDK-1, then pin every
/// authority fact. Nothing here approves, allows or executes anything.
#[allow(clippy::too_many_arguments)]
pub fn prepare_action(
    snapshot: &CapabilityAvailabilitySnapshotV1,
    capability_id: &CapabilityId,
    task_id: TaskId,
    step_id: StepId,
    classified: &ClassifiedArgumentsV1,
    requested_by: RequestedBy,
    caller_deadline_ms: Option<u32>,
    remaining_task_budget_ms: Option<u32>,
) -> Result<PreparedActionV1, PreparationError> {
    let resolution = snapshot
        .resolve(capability_id)
        .map_err(|error| match error {
            ResolveError::Unknown { capability_id } => PreparationError::Unknown { capability_id },
            ResolveError::Unavailable { capability_id } => {
                PreparationError::Unavailable { capability_id }
            }
            ResolveError::ContractFailure { capability_id, .. } => {
                PreparationError::ContractFailure { capability_id }
            }
        })?;
    let descriptor = &resolution.descriptor;
    let argument_class = classified.data_class();
    // Credential material never crosses this boundary.
    if argument_class == DataClass::Credential {
        return Err(PreparationError::CredentialClassRefused);
    }
    if argument_class.rank() > descriptor.data_class().rank() {
        return Err(PreparationError::DataClassAboveCeiling {
            argument_class,
            descriptor_ceiling: descriptor.data_class(),
        });
    }
    // The descriptor's trusted input schema validates the arguments.
    let validator = snapshot
        .input_schema_validator(descriptor.input_schema().as_str())
        .ok_or(PreparationError::SchemaInvalid)?;
    let instance = Value::Object(classified.arguments().clone());
    if !validator.is_valid(&instance) {
        return Err(PreparationError::SchemaInvalid);
    }
    // Canonicalize for digest and IDK-1. SCJ-1 refuses numbers outside its
    // integer domain; those arguments fail closed instead of being coerced.
    let arguments_text = instance.to_string();
    let canonical = serea_protocol::canonicalize(&arguments_text).map_err(canonical_error)?;
    let canonical_text =
        String::from_utf8(canonical).map_err(|_| PreparationError::ArgumentsNotCanonical)?;
    let arguments_digest = serea_protocol::digest_of(&canonical_text)
        .map_err(|_| PreparationError::ArgumentsNotCanonical)?;
    let idempotency_key = serea_protocol::derive_idempotency_key(
        &task_id,
        &step_id,
        descriptor.id(),
        &resolution.version,
        &canonical_text,
    )
    .map_err(|_| PreparationError::ArgumentsNotCanonical)?;
    // The effective deadline is the strictest host bound. Zero never becomes a
    // deadline and nothing overflows.
    let mut deadline_ms = descriptor.max_duration_ms();
    if let Some(bound) = caller_deadline_ms {
        deadline_ms = deadline_ms.min(bound);
    }
    if let Some(bound) = remaining_task_budget_ms {
        deadline_ms = deadline_ms.min(bound);
    }
    if deadline_ms == 0 {
        return Err(PreparationError::NoEffectiveDeadline);
    }
    Ok(PreparedActionV1 {
        task_id,
        step_id,
        generation_digest: snapshot.manifest().digest().clone(),
        generation_id: snapshot.generation_id(),
        descriptor_digest: resolution.descriptor_digest.clone(),
        capability_id: descriptor.id().clone(),
        capability_version: resolution.version.clone(),
        provider_id: descriptor.provider_id().clone(),
        implementation_id: descriptor.implementation_id().cloned(),
        arguments: classified.arguments().clone(),
        arguments_digest,
        idempotency_key,
        data_class: argument_class,
        requested_by,
        deadline_ms,
        side_effect_class: descriptor.side_effect_class(),
        risk_class: descriptor.risk_class(),
        required_authorization: descriptor.required_authorization(),
        replay_safety: descriptor.replay_safety(),
        root_requirement: descriptor.root_requirement(),
        idempotency_support: descriptor.idempotency_support(),
        cost_class: descriptor.cost_class(),
    })
}

fn canonical_error(_error: CanonicalJsonError) -> PreparationError {
    PreparationError::ArgumentsNotCanonical
}
