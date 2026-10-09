//! Host-owned authoritative manifest (ADR-0034).
//!
//! The manifest alone determines which capabilities may exist. Providers
//! only confirm or withdraw advertisements; they never create authority.

use std::collections::{BTreeMap, HashMap};
use std::fmt;

use serea_protocol::{CapabilityId, Digest, SemVer};

use crate::digests::{ManifestEntryV1, descriptor_semantic_digest, manifest_digest};
use crate::schema_catalog::CapabilitySchemaCatalogV1;

/// Manifest construction failure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ManifestError {
    DuplicateIdentity,
    DuplicateCandidatePriority,
    NoneImplementationConflict,
    DuplicateDefault,
    DefaultNotRepresented,
    ProviderNamespaceMismatch,
    OrdinaryHostProvider,
    HostCapabilityId,
    DescriptorDigestMismatch,
    SchemaUriMissing,
    SchemaDigestMismatch,
}

impl fmt::Display for ManifestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}
impl std::error::Error for ManifestError {}

/// The immutable, host-owned manifest.
#[derive(Debug, Clone)]
pub struct CapabilityManifestV1 {
    entries: Vec<ManifestEntryV1>,
    defaults: BTreeMap<String, SemVer>,
    catalog: CapabilitySchemaCatalogV1,
    digest: Digest,
}

impl CapabilityManifestV1 {
    /// Validates fully, then materializes. On any failure nothing escapes.
    pub fn build(
        entries: Vec<ManifestEntryV1>,
        defaults: Vec<(CapabilityId, SemVer)>,
        catalog: CapabilitySchemaCatalogV1,
    ) -> Result<Self, ManifestError> {
        // provider namespace + host guards, per entry
        for entry in &entries {
            let descriptor = &entry.descriptor;
            if descriptor.provider_id().as_str() == "host" {
                return Err(ManifestError::OrdinaryHostProvider);
            }
            if descriptor.id().as_str().starts_with("host.") {
                return Err(ManifestError::HostCapabilityId);
            }
            let id_provider = descriptor.id().as_str().split('.').next().unwrap_or("");
            if id_provider != descriptor.provider_id().as_str() {
                return Err(ManifestError::ProviderNamespaceMismatch);
            }
            // schema URIs must resolve in the catalog and digests must match
            let input_uri = descriptor.input_schema().as_str();
            let output_uri = descriptor.output_schema().as_str();
            let input_digest = catalog
                .document_digest(input_uri)
                .ok_or(ManifestError::SchemaUriMissing)?;
            if input_digest != &entry.input_schema_digest {
                return Err(ManifestError::SchemaDigestMismatch);
            }
            let output_digest = catalog
                .document_digest(output_uri)
                .ok_or(ManifestError::SchemaUriMissing)?;
            if output_digest != &entry.output_schema_digest {
                return Err(ManifestError::SchemaDigestMismatch);
            }
            // descriptor semantic digest must recompute exactly
            let computed = descriptor_semantic_digest(descriptor, &catalog)
                .map_err(|_| ManifestError::SchemaUriMissing)?;
            if computed != entry.descriptor_digest {
                return Err(ManifestError::DescriptorDigestMismatch);
            }
        }
        // duplicate logical identity
        let mut identities: HashMap<(&str, &str, Option<&str>), ()> = HashMap::new();
        for entry in &entries {
            let key = (
                entry.descriptor.id().as_str(),
                entry.descriptor.version().as_str(),
                entry.descriptor.implementation_id().map(|i| i.as_str()),
            );
            if identities.insert(key, ()).is_some() {
                return Err(ManifestError::DuplicateIdentity);
            }
        }
        // None/Some implementation conflict per (id, version)
        let mut per_version: HashMap<(&str, &str), Vec<Option<&str>>> = HashMap::new();
        for entry in &entries {
            per_version
                .entry((
                    entry.descriptor.id().as_str(),
                    entry.descriptor.version().as_str(),
                ))
                .or_default()
                .push(entry.descriptor.implementation_id().map(|i| i.as_str()));
        }
        for implementations in per_version.values() {
            let has_none = implementations.iter().any(Option::is_none);
            let has_some = implementations.iter().any(Option::is_some);
            if has_none && has_some {
                return Err(ManifestError::NoneImplementationConflict);
            }
            if implementations.iter().filter(|i| i.is_none()).count() > 1 {
                // duplicate identity already caught; this is defense in depth
                return Err(ManifestError::NoneImplementationConflict);
            }
        }
        // duplicate candidate priority per (id, version)
        let mut priorities: HashMap<(&str, &str), HashMap<u32, ()>> = HashMap::new();
        for entry in &entries {
            let map = priorities
                .entry((
                    entry.descriptor.id().as_str(),
                    entry.descriptor.version().as_str(),
                ))
                .or_default();
            if map.insert(entry.candidate_priority, ()).is_some() {
                return Err(ManifestError::DuplicateCandidatePriority);
            }
        }
        // defaults
        let mut defaults_map: BTreeMap<String, SemVer> = BTreeMap::new();
        for (id, version) in defaults {
            if defaults_map
                .insert(id.as_str().to_string(), version.clone())
                .is_some()
            {
                return Err(ManifestError::DuplicateDefault);
            }
            let represented = entries
                .iter()
                .any(|e| e.descriptor.id() == &id && e.descriptor.version() == &version);
            if !represented {
                return Err(ManifestError::DefaultNotRepresented);
            }
        }
        let mut sorted_entries = entries;
        sorted_entries.sort_by_key(entry_sort_key);
        let defaults_pairs: Vec<(CapabilityId, SemVer)> = defaults_map
            .iter()
            .map(|(id, v)| {
                (
                    CapabilityId::new(id.clone()).expect("validated id"),
                    v.clone(),
                )
            })
            .collect::<Vec<_>>();
        let digest = manifest_digest(catalog.digest(), &sorted_entries, &defaults_pairs);
        Ok(Self {
            entries: sorted_entries,
            defaults: defaults_map,
            catalog,
            digest,
        })
    }

    pub fn digest(&self) -> &Digest {
        &self.digest
    }

    pub fn catalog_digest(&self) -> &Digest {
        self.catalog.digest()
    }

    pub fn entries(&self) -> &[ManifestEntryV1] {
        &self.entries
    }

    pub fn default_version(&self, id: &CapabilityId) -> Option<&SemVer> {
        self.defaults.get(id.as_str())
    }

    pub fn candidates(&self, id: &CapabilityId, version: &SemVer) -> Vec<&ManifestEntryV1> {
        let mut v: Vec<&ManifestEntryV1> = self
            .entries
            .iter()
            .filter(|e| e.descriptor.id() == id && e.descriptor.version() == version)
            .collect();
        v.sort_by(|a, b| {
            a.candidate_priority
                .cmp(&b.candidate_priority)
                .then_with(|| {
                    a.descriptor
                        .provider_id()
                        .as_str()
                        .cmp(b.descriptor.provider_id().as_str())
                })
                .then_with(|| {
                    a.descriptor
                        .implementation_id()
                        .map(|i| i.as_str())
                        .cmp(&b.descriptor.implementation_id().map(|i| i.as_str()))
                })
        });
        v
    }

    pub fn catalog(&self) -> &CapabilitySchemaCatalogV1 {
        &self.catalog
    }

    pub fn defaults(&self) -> Vec<(CapabilityId, SemVer)> {
        self.defaults
            .iter()
            .map(|(id, v)| (CapabilityId::new(id.clone()).expect("validated"), v.clone()))
            .collect()
    }
}

fn entry_sort_key(entry: &ManifestEntryV1) -> (String, String, u8, String, String, u32) {
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
