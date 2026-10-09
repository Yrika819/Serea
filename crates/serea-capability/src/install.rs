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
            verify_generation_matches_manifest(store, manifest, current.generation_id())?;
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
    let activation = CapabilityRegistry::activate_generation(
        store,
        events,
        generation.generation_id(),
        occurred_at,
    );
    if let Err(error) = activation {
        // Two startup callers can prepare identical generations concurrently.
        // A higher id may activate before the lower id reaches activation;
        // in that case reuse the now-active generation only after rechecking
        // its complete durable manifest facts.
        if matches!(&error, StoreError::RegistryGenerationOrder) {
            if let Some(current) = CapabilityRegistry::current_generation(store)? {
                if current.manifest_digest() == manifest.digest()
                    && current.schema_catalog_digest() == manifest.catalog_digest()
                {
                    verify_generation_matches_manifest(store, manifest, current.generation_id())?;
                    return Ok(InstallOutcome {
                        generation_id: current.generation_id(),
                        activated: false,
                    });
                }
            }
        }
        return Err(InstallError::Store(error));
    }
    Ok(InstallOutcome {
        generation_id: generation.generation_id(),
        activated: true,
    })
}

fn verify_generation_matches_manifest(
    store: &Store,
    manifest: &CapabilityManifestV1,
    generation_id: i64,
) -> Result<(), InstallError> {
    let members = CapabilityRegistry::generation_members(store, generation_id)?;
    if members.len() != manifest.entries().len() {
        return Err(InstallError::Corruption(format!(
            "member count {} != manifest entries {}",
            members.len(),
            manifest.entries().len()
        )));
    }
    for member in &members {
        let matches = manifest.entries().iter().any(|entry| {
            entry.descriptor.id() == member.capability_id()
                && entry.descriptor.version() == member.capability_version()
                && entry.descriptor.provider_id() == member.provider_id()
                && entry.descriptor.implementation_id() == member.implementation_id()
                && entry.descriptor_digest == *member.descriptor_digest()
                && entry.candidate_priority == member.candidate_priority()
        });
        if !matches {
            return Err(InstallError::Corruption(format!(
                "member {} does not match manifest descriptor or priority",
                member.descriptor_digest().as_str()
            )));
        }
    }
    for (id, version) in manifest.defaults() {
        if CapabilityRegistry::default_version(store, generation_id, &id)?.as_ref()
            != Some(&version)
        {
            return Err(InstallError::Corruption(format!(
                "default version for {} does not match manifest",
                id.as_str()
            )));
        }
    }
    Ok(())
}
