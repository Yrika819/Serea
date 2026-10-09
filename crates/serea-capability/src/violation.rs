//! Sanitized `MODEL_SCHEMA_VIOLATION` emission (ADR-0035).
//!
//! A refused model-authored proposal produces a content-free event carrying
//! the stable violation code, the offending member NAMES and their count. It
//! never carries offending values, arguments, the raw proposal, the prompt or
//! any model content. If the event cannot be appended, the proposal still
//! produces no prepared action.

use std::fmt;

use serea_event_bus::{EventBus, ModelEventMetadataV1, ModelSchemaViolationEventV1};
use serea_protocol::StepId;
use serea_storage::EventDraft;
use serea_storage::{Store, StoreError};

use crate::proposal::ProposalRejection;

/// The sanitized, content-free shape of one violation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelSchemaViolation {
    violation_code: &'static str,
    offending_field_names: Vec<String>,
    offending_field_count: usize,
}

impl From<&ProposalRejection> for ModelSchemaViolation {
    fn from(rejection: &ProposalRejection) -> Self {
        let mut names: Vec<String> = rejection
            .offending_field_names()
            .iter()
            .map(|name| (*name).to_owned())
            .collect();
        names.sort();
        names.dedup();
        Self {
            violation_code: rejection.code(),
            offending_field_count: rejection.offending_field_count(),
            offending_field_names: names,
        }
    }
}

impl fmt::Display for ModelSchemaViolation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "MODEL_SCHEMA_VIOLATION:{}", self.violation_code)
    }
}

impl ModelSchemaViolation {
    pub fn violation_code(&self) -> &'static str {
        self.violation_code
    }
    pub fn offending_field_names(&self) -> &[String] {
        &self.offending_field_names
    }
    pub fn offending_field_count(&self) -> usize {
        self.offending_field_count
    }
}

/// Records one sanitized `MODEL_SCHEMA_VIOLATION` in the caller's Store
/// transaction context.
///
/// The event is an activity record, so it commits on its own. A caller that
/// must fail the operation on an append error receives the typed error; either
/// way the refused proposal yields no prepared action.
pub fn record_schema_violation(
    store: &Store,
    events: &EventBus,
    metadata: ModelEventMetadataV1,
    step_id: Option<StepId>,
    rejection: &ProposalRejection,
) -> Result<(), StoreError> {
    let violation = ModelSchemaViolation::from(rejection);
    let draft = events.draft_model_schema_violation(ModelSchemaViolationEventV1 {
        metadata,
        step_id,
        violation_code: violation.violation_code.to_owned(),
        offending_field_names: violation.offending_field_names.clone(),
        offending_field_count: violation.offending_field_count as u32,
    })?;
    let EventDraft {
        event,
        retention_at,
    } = draft;
    store.transact(|tx| tx.append_event(event, retention_at).map(|_| ()))
}
