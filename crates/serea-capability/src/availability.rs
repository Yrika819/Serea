//! Immutable per-operation availability snapshot and deterministic
//! resolution (ADR-0034 §availability).
//!
//! One snapshot freezes: every provider health (one read each), every
//! provider advertisement (one read each), every live durable overlay and
//! the caller-supplied host eligibility for every manifest capability.
//! Nothing in the snapshot mutates authority facts.

use serea_protocol::{
    CapabilityDescriptor, CapabilityId, Digest, ImplementationId, ProviderHealth, ProviderId,
    SemVer,
};
use std::collections::HashMap;
use std::fmt;

use crate::manifest::CapabilityManifestV1;
use crate::provider::ProviderRegistry;
use crate::{CapabilityOverlayState, CapabilityRegistry};
use serea_storage::Store;

/// Caller-supplied trusted host eligibility, keyed by descriptor digest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostEligibility {
    entries: HashMap<Digest, bool>,
}

impl HostEligibility {
    pub fn new(entries: HashMap<Digest, bool>) -> Self {
        Self { entries }
    }

    pub fn eligible(&self, digest: &Digest) -> Option<bool> {
        self.entries.get(digest).copied()
    }
}

/// Snapshot construction failure (provider contract violation).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AvailabilityError {
    ProviderContractViolation,
    RegistryGenerationMismatch,
}

impl fmt::Display for AvailabilityError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

/// Deterministic per-operation resolution outcome.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolveError {
    Unknown {
        capability_id: CapabilityId,
    },
    Unavailable {
        capability_id: CapabilityId,
    },
    ContractFailure {
        capability_id: CapabilityId,
        reason: String,
    },
}

impl fmt::Display for ResolveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}
impl std::error::Error for ResolveError {}

/// Successful deterministic resolution.
#[derive(Debug, Clone, PartialEq)]
pub struct Resolution {
    pub capability_id: CapabilityId,
    pub version: SemVer,
    pub provider_id: ProviderId,
    pub implementation_id: Option<ImplementationId>,
    pub descriptor_digest: Digest,
    pub descriptor: CapabilityDescriptor,
}

/// The one immutable per-operation freeze of live facts.
#[derive(Debug, Clone)]
pub struct CapabilityAvailabilitySnapshotV1 {
    generation_id: i64,
    manifest: CapabilityManifestV1,
    health: HashMap<ProviderId, ProviderHealth>,
    advertised: HashMap<ProviderId, Vec<CapabilityDescriptor>>,
    overlays: HashMap<CapabilityId, (CapabilityOverlayState, bool)>,
    eligibility: HostEligibility,
}

impl CapabilityAvailabilitySnapshotV1 {
    /// Snapshots health/advertisement/overlay/eligibility once. The
    /// advertisement revalidation rejects mismatched or unmanifested
    /// advertisements with a typed contract error and freezes no invalid
    /// state.
    pub async fn build_for_generation(
        manifest: CapabilityManifestV1,
        generation_id: i64,
        registry: &ProviderRegistry,
        store: &Store,
        eligibility: HostEligibility,
    ) -> Result<Self, AvailabilityError> {
        // A digest identifies manifest semantics, not a durable generation.
        // Verify the caller-supplied identity and every persisted selection
        // fact; never substitute the active generation or infer an id.
        let generation = store
            .get_registry_generation(generation_id)
            .map_err(|_| AvailabilityError::RegistryGenerationMismatch)?
            .ok_or(AvailabilityError::RegistryGenerationMismatch)?;
        if generation.manifest_digest() != manifest.digest()
            || generation.schema_catalog_digest() != manifest.catalog_digest()
        {
            return Err(AvailabilityError::RegistryGenerationMismatch);
        }
        let members = store
            .list_generation_members(generation_id)
            .map_err(|_| AvailabilityError::RegistryGenerationMismatch)?;
        if members.len() != manifest.entries().len()
            || !manifest.entries().iter().all(|entry| {
                members.iter().any(|member| {
                    member.descriptor_digest() == &entry.descriptor_digest
                        && member.capability_id() == entry.descriptor.id()
                        && member.capability_version() == entry.descriptor.version()
                        && member.provider_id() == entry.descriptor.provider_id()
                        && member.implementation_id() == entry.descriptor.implementation_id()
                        && member.candidate_priority() == entry.candidate_priority
                })
            })
        {
            return Err(AvailabilityError::RegistryGenerationMismatch);
        }
        for member in &members {
            let durable_default = store
                .get_generation_default_version(generation_id, member.capability_id())
                .map_err(|_| AvailabilityError::RegistryGenerationMismatch)?;
            if durable_default.as_ref() != manifest.default_version(member.capability_id()) {
                return Err(AvailabilityError::RegistryGenerationMismatch);
            }
        }
        let mut health = HashMap::new();
        let mut advertised: HashMap<ProviderId, Vec<CapabilityDescriptor>> = HashMap::new();
        for provider in registry.providers() {
            let provider_id = provider.provider_id();
            let caps = provider.capabilities();
            for descriptor in &caps {
                if descriptor.provider_id() != &provider.provider_id() {
                    return Err(AvailabilityError::ProviderContractViolation);
                }
                let identity_exists = manifest.entries().iter().any(|e| {
                    e.descriptor.id() == descriptor.id()
                        && e.descriptor.version() == descriptor.version()
                        && e.descriptor.implementation_id() == descriptor.implementation_id()
                });
                if !identity_exists {
                    return Err(AvailabilityError::ProviderContractViolation);
                }
                let exact = manifest.entries().iter().any(|e| {
                    e.descriptor.id() == descriptor.id()
                        && e.descriptor.version() == descriptor.version()
                        && e.descriptor.implementation_id() == descriptor.implementation_id()
                        && e.descriptor == *descriptor
                });
                if !exact {
                    return Err(AvailabilityError::ProviderContractViolation);
                }
            }
            advertised.insert(provider_id.clone(), caps);
            health.insert(provider_id.clone(), provider.health().await);
        }
        let mut overlays = HashMap::new();
        let mut seen = HashMap::new();
        for entry in manifest.entries() {
            if seen.insert(entry.descriptor.id().clone(), ()).is_some() {
                continue;
            }
            let overlay = CapabilityRegistry::overlay(store, entry.descriptor.id())
                .map_err(|_| AvailabilityError::ProviderContractViolation)?;
            overlays.insert(
                entry.descriptor.id().clone(),
                (overlay.state(), overlay.experimental_opt_in()),
            );
        }
        Ok(Self {
            generation_id,
            manifest,
            health,
            advertised,
            overlays,
            eligibility,
        })
    }

    /// Every CapabilityId the frozen manifest knows, ordered by UTF-8 byte
    /// order so projections over it are deterministic.
    pub fn manifest_capability_ids(&self) -> Vec<CapabilityId> {
        let mut ids: Vec<CapabilityId> = Vec::new();
        for entry in self.manifest.entries() {
            if !ids.iter().any(|id| id == entry.descriptor.id()) {
                ids.push(entry.descriptor.id().clone());
            }
        }
        ids.sort_by(|a, b| a.as_str().as_bytes().cmp(b.as_str().as_bytes()));
        ids
    }

    /// The parsed trusted schema document for `uri`, or `None` when the frozen
    /// catalog does not hold it. Callers receive catalog bytes only; nothing
    /// here resolves a URI outside the catalog.
    pub fn catalog_schema(&self, uri: &str) -> Option<serde_json::Value> {
        self.manifest.catalog().document(uri).cloned()
    }

    /// The frozen manifest this snapshot was built from.
    pub fn manifest(&self) -> &CapabilityManifestV1 {
        &self.manifest
    }

    /// Exact durable generation represented by this frozen snapshot.
    pub fn generation_id(&self) -> i64 {
        self.generation_id
    }

    /// A compiled validator for one trusted catalog document, or `None` when
    /// the catalog does not hold it or it does not compile. Resolution is
    /// in-memory only; nothing here reads a URI outside the catalog.
    pub fn input_schema_validator(&self, uri: &str) -> Option<jsonschema::Validator> {
        self.manifest.catalog().validator(uri)
    }

    /// Version from manifest default; implementation from candidate_priority
    /// ordering. Never falls back to another version.
    pub fn resolve(&self, capability_id: &CapabilityId) -> Result<Resolution, ResolveError> {
        let known = self
            .manifest
            .entries()
            .iter()
            .any(|e| e.descriptor.id() == capability_id);
        if !known {
            return Err(ResolveError::Unknown {
                capability_id: capability_id.clone(),
            });
        }
        let default_version = self
            .manifest
            .default_version(capability_id)
            .ok_or_else(|| ResolveError::ContractFailure {
                capability_id: capability_id.clone(),
                reason: "default version absent".into(),
            })?;
        let candidates = self.manifest.candidates(capability_id, default_version);
        if candidates.is_empty() {
            return Err(ResolveError::ContractFailure {
                capability_id: capability_id.clone(),
                reason: "default version has no candidate".into(),
            });
        }
        let (overlay_state, opt_in) = match self.overlays.get(capability_id) {
            Some(v) => *v,
            None => (CapabilityOverlayState::Enabled, false),
        };
        if overlay_state != CapabilityOverlayState::Enabled {
            return Err(ResolveError::Unavailable {
                capability_id: capability_id.clone(),
            });
        }
        for candidate in candidates {
            let d = &candidate.descriptor;
            if d.experimental() && !opt_in {
                continue;
            }
            // provider registered and READY
            let provider_id = d.provider_id();
            if !self.health.contains_key(provider_id) {
                continue;
            }
            let health_ok = self.health.get(provider_id) == Some(&ProviderHealth::Ready);
            if !health_ok {
                continue;
            }
            // descriptor currently advertised exactly
            let advertised_exact = self
                .advertised
                .get(provider_id)
                .map(|v| v.contains(d))
                .unwrap_or(false);
            if !advertised_exact {
                continue;
            }
            // host eligibility true
            if self.eligibility.eligible(&candidate.descriptor_digest) != Some(true) {
                continue;
            }
            return Ok(Resolution {
                capability_id: capability_id.clone(),
                version: default_version.clone(),
                provider_id: d.provider_id().clone(),
                implementation_id: d.implementation_id().cloned(),
                descriptor_digest: candidate.descriptor_digest.clone(),
                descriptor: d.clone(),
            });
        }
        Err(ResolveError::Unavailable {
            capability_id: capability_id.clone(),
        })
    }
}
