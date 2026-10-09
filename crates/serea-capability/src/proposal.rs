//! `ToolCallProposalV1` parsing (ADR-0035).
//!
//! The frozen shape is exactly `version`, `capability_id` and an object-root
//! `arguments`. There is no RequestId, Task/Step id, version, provider,
//! implementation, risk, side-effect, authorization, replay, digest, IDK, data
//! class, requester, deadline, approval, policy or credential field. Any
//! undeclared member invalidates the whole proposal: extras are never dropped
//! and parsing never continues with a partial result.
//!
//! `ActionRequest` is never deserialized from model JSON. This parser produces
//! only the three declared facts; everything else is host-resolved afterwards.

use std::fmt;

use serde_json::{Map, Value};
use serea_protocol::{CapabilityId, ProtocolError};

use crate::strict_json::{StrictJsonError, parse_strict};

/// The only accepted proposal version.
pub const TOOL_CALL_PROPOSAL_VERSION: &str = "1";

const VERSION: &str = "version";
const CAPABILITY_ID: &str = "capability_id";
const ARGUMENTS: &str = "arguments";

/// The three members the frozen shape declares. Every other member name is an
/// undeclared member, including each host-resolved authority field.
const DECLARED_MEMBERS: [&str; 3] = [VERSION, CAPABILITY_ID, ARGUMENTS];

/// A structurally valid, closed model-authored tool call proposal.
#[derive(Debug, Clone, PartialEq)]
pub struct ToolCallProposalV1 {
    capability_id: CapabilityId,
    arguments: Map<String, Value>,
}

impl ToolCallProposalV1 {
    /// The capability the model asked for. The version, provider,
    /// implementation and every other authority fact are host-resolved.
    pub fn capability_id(&self) -> &CapabilityId {
        &self.capability_id
    }

    /// The model's arguments, still unvalidated against any schema and still
    /// unclassified. Nothing here has authority.
    pub fn arguments(&self) -> &Map<String, Value> {
        &self.arguments
    }
}

/// Why a proposal was refused.
///
/// Every variant is a whole-proposal refusal. Carrying only stable codes and
/// member names keeps rejected model content out of diagnostics.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProposalRejection {
    /// The text is not valid JSON.
    MalformedJson,
    /// The root is not a JSON object.
    RootNotObject,
    /// An object member name repeats at any depth.
    DuplicateMemberName,
    /// A declared member is absent.
    MissingField,
    /// A member outside the frozen shape is present, with its names.
    UndeclaredMember {
        /// Offending member names only; never values.
        names: Vec<String>,
    },
    /// `version` is present but is not the frozen value.
    UnsupportedVersion,
    /// `capability_id` is not a valid capability identifier.
    MalformedCapabilityId,
    /// `arguments` is present but is not a JSON object.
    ArgumentsNotObject,
}

/// Bound on how many offending member names one refusal reports.
const MAX_REPORTED_FIELD_NAMES: usize = 8;
/// Bound on the length of one reported member name.
const MAX_REPORTED_FIELD_NAME_BYTES: usize = 64;

impl ProposalRejection {
    /// Stable, content-free violation code for a `MODEL_SCHEMA_VIOLATION`
    /// event. The value never contains model output.
    pub fn code(&self) -> &'static str {
        match self {
            Self::MalformedJson => "MALFORMED_JSON",
            Self::RootNotObject => "ROOT_NOT_OBJECT",
            Self::DuplicateMemberName => "DUPLICATE_MEMBER_NAME",
            Self::MissingField => "MISSING_FIELD",
            Self::UndeclaredMember { .. } => "UNDECLARED_MEMBER",
            Self::UnsupportedVersion => "UNSUPPORTED_VERSION",
            Self::MalformedCapabilityId => "MALFORMED_CAPABILITY_ID",
            Self::ArgumentsNotObject => "ARGUMENTS_NOT_OBJECT",
        }
    }

    /// Offending member names, sorted and bounded, when the refusal names
    /// members. Never values, never the rejected text.
    pub fn offending_field_names(&self) -> Vec<&str> {
        match self {
            Self::MissingField => DECLARED_MEMBERS.to_vec(),
            Self::UndeclaredMember { names } => names.iter().map(String::as_str).collect(),
            _ => Vec::new(),
        }
    }

    /// How many member names this refusal reports.
    pub fn offending_field_count(&self) -> usize {
        self.offending_field_names().len()
    }
}

/// Keeps only short, identifier-like member names so a hostile proposal cannot
/// push arbitrary model text into an event payload.
fn reportable_names(object: &Map<String, Value>) -> Vec<String> {
    let mut names: Vec<String> = object
        .keys()
        .filter(|name| !DECLARED_MEMBERS.contains(&name.as_str()))
        .filter(|name| {
            name.len() <= MAX_REPORTED_FIELD_NAME_BYTES
                && !name.is_empty()
                && name
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
        })
        .cloned()
        .collect();
    names.sort();
    names.dedup();
    names.truncate(MAX_REPORTED_FIELD_NAMES);
    names
}

impl fmt::Display for ProposalRejection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.code())
    }
}
impl std::error::Error for ProposalRejection {}

/// Parses model-authored text into a [`ToolCallProposalV1`], or refuses the
/// whole proposal.
pub fn parse_tool_call_proposal(text: &str) -> Result<ToolCallProposalV1, ProposalRejection> {
    let value = parse_strict(text).map_err(|error| match error {
        StrictJsonError::DuplicateMemberName => ProposalRejection::DuplicateMemberName,
        StrictJsonError::Syntax => ProposalRejection::MalformedJson,
    })?;
    let object = match value {
        Value::Object(object) => object,
        _ => return Err(ProposalRejection::RootNotObject),
    };
    for name in object.keys() {
        if !DECLARED_MEMBERS.contains(&name.as_str()) {
            return Err(ProposalRejection::UndeclaredMember {
                names: reportable_names(&object),
            });
        }
    }
    let version = object.get(VERSION).ok_or(ProposalRejection::MissingField)?;
    if version.as_str() != Some(TOOL_CALL_PROPOSAL_VERSION) {
        return Err(ProposalRejection::UnsupportedVersion);
    }
    let capability_id = object
        .get(CAPABILITY_ID)
        .ok_or(ProposalRejection::MissingField)?;
    let raw_id = capability_id
        .as_str()
        .ok_or(ProposalRejection::MalformedCapabilityId)?;
    let capability_id = CapabilityId::new(raw_id).map_err(|error: ProtocolError| {
        // the rejected identifier text is not echoed
        let _ = error;
        ProposalRejection::MalformedCapabilityId
    })?;
    let arguments = object
        .get(ARGUMENTS)
        .ok_or(ProposalRejection::MissingField)?;
    let arguments = match arguments {
        Value::Object(map) => map.clone(),
        _ => return Err(ProposalRejection::ArgumentsNotObject),
    };
    Ok(ToolCallProposalV1 {
        capability_id,
        arguments,
    })
}
