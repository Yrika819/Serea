//! Domain-separated host digests (ADR-0019/0034/0035).
//!
//! Descriptor semantic digests, schema digests, catalog digests and manifest
//! digests all derive from one deterministic canonical JSON projection and
//! SHA-256. The projection is SCJ-1-safe (strings, booleans, integers).

use std::fmt;

use serde_json::json;

use crate::schema_catalog::{canonical_schema_bytes, sha256_digest};
use serea_protocol::{CapabilityDescriptor, CapabilityId, Digest, SemVer};

use crate::schema_catalog::CapabilitySchemaCatalogV1;

/// Domain marker for descriptor semantic projection.
pub const DESCRIPTOR_DOMAIN: &str = "serea.capability-descriptor/1";
/// Domain marker for manifest projection.
pub const MANIFEST_DOMAIN: &str = "serea.capability-manifest/1";
/// Domain marker for catalog projection.
pub const CATALOG_DOMAIN: &str = "serea.capability-schema-catalog/1";

/// Digest computation failure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DigestError {
    MissingSchemaUri(String),
}

impl fmt::Display for DigestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}
impl std::error::Error for DigestError {}

/// Host-computed descriptor semantic digest.
///
/// All authority-bearing facts plus computed schema digests. Candidate
/// priority is generation metadata and is NOT included.
pub fn descriptor_semantic_digest(
    descriptor: &CapabilityDescriptor,
    catalog: &CapabilitySchemaCatalogV1,
) -> Result<Digest, DigestError> {
    let input_uri = descriptor.input_schema().as_str();
    let output_uri = descriptor.output_schema().as_str();
    let input_digest = catalog
        .document_digest(input_uri)
        .ok_or_else(|| DigestError::MissingSchemaUri(input_uri.to_string()))?;
    let output_digest = catalog
        .document_digest(output_uri)
        .ok_or_else(|| DigestError::MissingSchemaUri(output_uri.to_string()))?;
    let projection = json!({
        "kind": DESCRIPTOR_DOMAIN,
        "capability_id": descriptor.id().as_str(),
        "version": descriptor.version().as_str(),
        "provider_id": descriptor.provider_id().as_str(),
        "implementation_id": descriptor.implementation_id().map(|i| i.as_str()),
        "title": descriptor.title().as_str(),
        "description": descriptor.description().as_str(),
        "input_schema_uri": input_uri,
        "input_schema_digest": input_digest.as_str(),
        "output_schema_uri": output_uri,
        "output_schema_digest": output_digest.as_str(),
        "side_effect_class": serde_json::to_value(descriptor.side_effect_class()).unwrap(),
        "risk_class": serde_json::to_value(descriptor.risk_class()).unwrap(),
        "required_authorization": serde_json::to_value(descriptor.required_authorization()).unwrap(),
        "replay_safety": serde_json::to_value(descriptor.replay_safety()).unwrap(),
        "data_class": serde_json::to_value(descriptor.data_class()).unwrap(),
        "root_requirement": serde_json::to_value(descriptor.root_requirement()).unwrap(),
        "idempotency_support": serde_json::to_value(descriptor.idempotency_support()).unwrap(),
        "max_duration_ms": descriptor.max_duration_ms(),
        "cost_class": serde_json::to_value(descriptor.cost_class()).unwrap(),
        "experimental": descriptor.experimental(),
    });
    Ok(sha256_digest(&canonical_schema_bytes(&projection)))
}

/// One manifest entry, as stored host-owned.
#[derive(Debug, Clone, PartialEq)]
pub struct ManifestEntryV1 {
    pub descriptor: CapabilityDescriptor,
    pub candidate_priority: u32,
    pub descriptor_digest: Digest,
    pub input_schema_digest: Digest,
    pub output_schema_digest: Digest,
}

/// Ordering: CapabilityId, SemVer exact text, implementation presence
/// (None first), ImplementationId value, ProviderId, candidate priority.
fn entry_key(entry: &ManifestEntryV1) -> (String, String, u8, String, String, u32) {
    (
        entry.descriptor.id().as_str().to_string(),
        entry.descriptor.version().as_str().to_string(),
        entry.descriptor.implementation_id().is_some() as u8,
        entry
            .descriptor
            .implementation_id()
            .map(|i| i.as_str().to_string())
            .unwrap_or_default(),
        entry.descriptor.provider_id().as_str().to_string(),
        entry.candidate_priority,
    )
}

/// Manifest semantic digest. Deterministic under input permutation.
pub fn manifest_digest(
    schema_catalog_digest: &Digest,
    entries: &[ManifestEntryV1],
    defaults: &[(CapabilityId, SemVer)],
) -> Digest {
    let mut entries = entries.to_vec();
    entries.sort_by_key(entry_key);
    let mut defaults: Vec<(CapabilityId, SemVer)> = defaults.to_vec();
    defaults.sort_by(|a, b| a.0.as_str().cmp(b.0.as_str()));
    let projection = json!({
        "kind": MANIFEST_DOMAIN,
        "schema_catalog_digest": schema_catalog_digest.as_str(),
        "entries": entries.iter().map(|e| json!({
            "capability_id": e.descriptor.id().as_str(),
            "version": e.descriptor.version().as_str(),
            "implementation_id": e.descriptor.implementation_id().map(|i| i.as_str()),
            "provider_id": e.descriptor.provider_id().as_str(),
            "candidate_priority": e.candidate_priority,
            "descriptor_digest": e.descriptor_digest.as_str(),
        })).collect::<Vec<_>>(),
        "defaults": defaults.iter().map(|(id, v)| json!({
            "capability_id": id.as_str(),
            "version": v.as_str(),
        })).collect::<Vec<_>>(),
    });
    sha256_digest(&canonical_schema_bytes(&projection))
}
