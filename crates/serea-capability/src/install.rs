//! Validate-then-install P5C manifests through the P5B durable registry
//! semantics: full validation before any write, active pointer as the only
//! authority, same-manifest restart reuses the active generation silently.

use std::fmt;

use serea_event_bus::EventBus;
use serea_protocol::EpochMillis;
use serea_storage::{
    DescriptorRevisionDraft, GenerationMemberDraft, RegistryGenerationDraft, Store, StoreError,
};

use crate::{
    CapabilityManifestV1, CapabilityRegistry, ManifestError, ProviderError, ProviderRegistry,
};

/// Installation outcome. If `activated` is false, the returned id is the
/// reused active generation (no event was emitted).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstallOutcome {
    generation_id: i64,
    activated: bool,
}

impl InstallOutcome {
    pub fn generation_id(&self) -> i64 {
        self.generation_id
    }
    pub fn activated(&self) -> bool {
        self.activated
    }
}

/// Install failure.
#[derive(Debug)]
pub enum InstallError {
    Manifest(ManifestError),
    Provider(ProviderError),
    Corruption(String),
    Store(StoreError),
}

impl fmt::Display for InstallError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}
impl std::error::Error for InstallError {}
impl From<StoreError> for InstallError {
    fn from(e: StoreError) -> Self {
        Self::Store(e)
    }
}
impl From<ManifestError> for InstallError {
    fn from(e: ManifestError) -> Self {
        Self::Manifest(e)
    }
}
impl From<ProviderError> for InstallError {
    fn from(e: ProviderError) -> Self {
        Self::Provider(e)
    }
}

/// Installs and optionally activates a validated manifest.
///
/// Order of operations:
///   1. validate provider advertisements against the manifest (all-or-nothing)
///   2. check the active generation: exact digest + member/digest match → reuse
///   3. otherwise: durable preparation (generation, revisions, members,
///      defaults) + atomic activation + exactly one event.
pub fn install(
    store: &Store,
    events: &EventBus,
    manifest: &CapabilityManifestV1,
    registry: &ProviderRegistry,
    occurred_at: EpochMillis,
) -> Result<InstallOutcome, InstallError> {
    registry
        .validate_advertisements(manifest)
        .map_err(InstallError::Provider)?;
    let current = CapabilityRegistry::current_generation(store)?;
    if let Some(current) = current {
        if current.manifest_digest() == manifest.digest()
            && current.schema_catalog_digest() == manifest.catalog_digest()
        {
            // verify durable member facts agree with the manifest
            let members = CapabilityRegistry::generation_members(store, current.generation_id())?;
            if members.len() != manifest.entries().len() {
                return Err(InstallError::Corruption(format!(
                    "member count {} != manifest entries {}",
                    members.len(),
                    manifest.entries().len()
                )));
            }
            for member in &members {
                let matches = manifest.entries().iter().any(|e| {
                    e.descriptor.id() == member.capability_id()
                        && e.descriptor.version() == member.capability_version()
                        && e.descriptor.implementation_id() == member.implementation_id()
                        && e.descriptor_digest == *member.descriptor_digest()
                });
                if !matches {
                    return Err(InstallError::Corruption(format!(
                        "member {} does not match manifest descriptor",
                        member.descriptor_digest().as_str()
                    )));
                }
            }
            return Ok(InstallOutcome {
                generation_id: current.generation_id(),
                activated: false,
            });
        }
    }
    // new prepared generation
    let generation = CapabilityRegistry::create_generation(
        store,
        RegistryGenerationDraft {
            manifest_digest: manifest.digest().clone(),
            schema_catalog_digest: manifest.catalog_digest().clone(),
        },
    )?;
    for entry in manifest.entries() {
        CapabilityRegistry::insert_descriptor_revision(
            store,
            DescriptorRevisionDraft {
                descriptor_digest: entry.descriptor_digest.clone(),
                descriptor: entry.descriptor.clone(),
                input_schema_digest: entry.input_schema_digest.clone(),
                output_schema_digest: entry.output_schema_digest.clone(),
            },
        )?;
        CapabilityRegistry::add_generation_member(
            store,
            GenerationMemberDraft {
                generation_id: generation.generation_id(),
                descriptor_digest: entry.descriptor_digest.clone(),
                candidate_priority: entry.candidate_priority,
            },
        )?;
    }
    for (id, version) in manifest.defaults() {
        CapabilityRegistry::set_default_version(
            store,
            generation.generation_id(),
            id.clone(),
            version.clone(),
        )?;
    }
    CapabilityRegistry::activate_generation(
        store,
        events,
        generation.generation_id(),
        occurred_at,
    )?;
    Ok(InstallOutcome {
        generation_id: generation.generation_id(),
        activated: true,
    })
}
