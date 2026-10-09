//! `ToolDefinitionV1` projection (ADR-0035).
//!
//! A tool definition carries exactly `version`, `capability_id`, `title`,
//! `description` and `input_schema`. Provider, implementation, capability
//! version, risk, side effect, authorization, replay safety, root,
//! idempotency, credential and policy facts are deliberately absent: a model
//! must never see them. Visibility requires a resolvable candidate in the
//! frozen snapshot, which already applies the live overlay, experimental
//! opt-in, provider health, exact advertisement and host eligibility. It does
//! not evaluate policy or approval and grants no authority.

use std::fmt;

use serde_json::{Map, Value};
use serea_protocol::CapabilityId;

use crate::availability::CapabilityAvailabilitySnapshotV1;

/// The frozen wire version of a projected tool definition.
pub const TOOL_DEFINITION_VERSION: &str = "1";

/// One projected tool definition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolDefinitionV1 {
    capability_id: CapabilityId,
    title: String,
    description: String,
    input_schema: Value,
}

impl ToolDefinitionV1 {
    pub fn capability_id(&self) -> &CapabilityId {
        &self.capability_id
    }

    /// Always [`TOOL_DEFINITION_VERSION`].
    pub fn version(&self) -> &str {
        TOOL_DEFINITION_VERSION
    }

    pub fn title(&self) -> &str {
        &self.title
    }

    pub fn description(&self) -> &str {
        &self.description
    }

    pub fn input_schema(&self) -> &Value {
        &self.input_schema
    }

    /// The exact closed object a model may see. Callers serialize this; it is
    /// never parsed back into authority.
    pub fn to_json_value(&self) -> Value {
        let mut object = Map::new();
        object.insert(
            "version".into(),
            Value::String(TOOL_DEFINITION_VERSION.to_owned()),
        );
        object.insert(
            "capability_id".into(),
            Value::String(self.capability_id.as_str().to_owned()),
        );
        object.insert("title".into(), Value::String(self.title.clone()));
        object.insert(
            "description".into(),
            Value::String(self.description.clone()),
        );
        object.insert("input_schema".into(), self.input_schema.clone());
        Value::Object(object)
    }
}

/// Why the tool projection could not be completed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolProjectionError {
    /// A resolvable capability's trusted input schema is not in the catalog.
    SchemaUnavailable,
}

impl fmt::Display for ToolProjectionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}
impl std::error::Error for ToolProjectionError {}

/// Projects the visible tool definitions of one frozen snapshot.
///
/// Ordered by CapabilityId UTF-8 byte order. The set is a projection of the
/// snapshot only: it never mutates authority and never consults policy,
/// approval or grants.
pub fn provider_tools(
    snapshot: &CapabilityAvailabilitySnapshotV1,
) -> Result<Vec<ToolDefinitionV1>, ToolProjectionError> {
    let mut projected: Vec<ToolDefinitionV1> = Vec::new();
    for capability_id in snapshot.manifest_capability_ids() {
        // A capability with no usable candidate is simply not visible; only a
        // resolvable capability whose trusted input schema is missing is a
        // projection failure.
        let Ok(resolution) = snapshot.resolve(&capability_id) else {
            continue;
        };
        let input_schema = snapshot
            .catalog_schema(resolution.descriptor.input_schema().as_str())
            .ok_or(ToolProjectionError::SchemaUnavailable)?;
        projected.push(ToolDefinitionV1 {
            capability_id: capability_id.clone(),
            title: resolution.descriptor.title().as_str().to_owned(),
            description: resolution.descriptor.description().as_str().to_owned(),
            input_schema,
        });
    }
    projected.sort_by(|a, b| {
        a.capability_id
            .as_str()
            .as_bytes()
            .cmp(b.capability_id.as_str().as_bytes())
    });
    Ok(projected)
}
