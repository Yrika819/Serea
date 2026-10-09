use serea_protocol::CapabilityId;
use serea_storage::StoreError;
use std::fmt;

/// Payload-free errors. Neither formatting nor source chains reveal titles,
/// arguments, results, error prose, raw JSON or backend diagnostics.
#[derive(Clone, PartialEq, Eq)]
pub enum EngineError {
    Store(StoreError),
    TaskNotFound,
    TaskExists,
    IllegalTaskTransition,
    PlanRevisionConflict,
    PlanRevisionOverflow,
    PlanRevisionWouldDropExecutedStep,
    InvalidPlan,
    InvalidPlanLayout,
    DuplicateStepId,
    DuplicateSequence,
    DuplicateIdempotencyKey,
    UnsupportedDataClass,
    InvalidTimestamp,
    /// No registry generation is active, so a post-P5 Task cannot be pinned.
    NoActiveCapabilityGeneration,
    /// A Task predating P5 has no pinned generation and must not be bound to
    /// the current one implicitly.
    UnpinnedTaskCannotBindCapability,
    /// The capability is known but has no usable implementation right now.
    CapabilityUnavailable {
        capability_id: CapabilityId,
    },
}

impl From<StoreError> for EngineError {
    fn from(error: StoreError) -> Self {
        match error {
            StoreError::TaskNotFound => Self::TaskNotFound,
            StoreError::TaskExists => Self::TaskExists,
            StoreError::IllegalTaskTransition => Self::IllegalTaskTransition,
            StoreError::PlanRevisionConflict => Self::PlanRevisionConflict,
            StoreError::PlanRevisionOverflow => Self::PlanRevisionOverflow,
            StoreError::PlanRevisionWouldDropExecutedStep => {
                Self::PlanRevisionWouldDropExecutedStep
            }
            StoreError::InvalidPlan => Self::InvalidPlan,
            StoreError::InvalidPlanLayout => Self::InvalidPlanLayout,
            StoreError::DuplicateStepId => Self::DuplicateStepId,
            StoreError::DuplicateSequence => Self::DuplicateSequence,
            StoreError::DuplicateIdempotencyKey => Self::DuplicateIdempotencyKey,
            StoreError::ClassRefused | StoreError::AtRestProtectionUnavailable => {
                Self::UnsupportedDataClass
            }
            StoreError::InvalidTimestamp => Self::InvalidTimestamp,
            other => Self::Store(other),
        }
    }
}

impl fmt::Display for EngineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Store(_) => "Store",
            Self::TaskNotFound => "TaskNotFound",
            Self::TaskExists => "TaskExists",
            Self::IllegalTaskTransition => "IllegalTaskTransition",
            Self::PlanRevisionConflict => "PlanRevisionConflict",
            Self::PlanRevisionOverflow => "PlanRevisionOverflow",
            Self::PlanRevisionWouldDropExecutedStep => "PlanRevisionWouldDropExecutedStep",
            Self::InvalidPlan => "InvalidPlan",
            Self::InvalidPlanLayout => "InvalidPlanLayout",
            Self::DuplicateStepId => "DuplicateStepId",
            Self::DuplicateSequence => "DuplicateSequence",
            Self::DuplicateIdempotencyKey => "DuplicateIdempotencyKey",
            Self::UnsupportedDataClass => "UnsupportedDataClass",
            Self::InvalidTimestamp => "InvalidTimestamp",
            Self::NoActiveCapabilityGeneration => "NoActiveCapabilityGeneration",
            Self::UnpinnedTaskCannotBindCapability => "UnpinnedTaskCannotBindCapability",
            // Only the capability identifier, never arguments or provider text.
            Self::CapabilityUnavailable { .. } => "CapabilityUnavailable",
        })
    }
}
impl fmt::Debug for EngineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}
impl std::error::Error for EngineError {}
