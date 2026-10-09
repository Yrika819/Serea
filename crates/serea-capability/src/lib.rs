//! P5B durable registry preparation plus the P5C host-owned capability
//! manifest, trusted schema catalog, provider matching, availability snapshot
//! and deterministic resolution.
//!
//! The manifest is the sole authority for which capabilities may exist
//! (ADR-0034). Providers may confirm, withdraw, or report health; they never
//! create authority. Schema resolution is in-memory only under
//! `https://serea.local/schemas/`. Model-authored proposals are parsed as a
//! closed three-field shape, classified across a trusted boundary, and
//! prepared into an immutable `PreparedActionV1` (ADR-0035). No provider is
//! ever invoked here: P8 is the first phase permitted to call
//! `CapabilityProvider::invoke`.

#![forbid(unsafe_code)]

use serea_event_bus::{CapabilityRegistryChangeKindV1, CapabilityRegistryChangeV1, EventBus};
use serea_protocol::{CapabilityId, Digest, EpochMillis, SemVer};
use serea_storage::{Store, StoreError, Tx};

pub mod availability;
pub mod digests;
pub mod install;
pub mod manifest;
pub mod preparation;
pub mod proposal;
pub mod provider;
pub mod schema_catalog;
pub mod strict_json;
pub mod tool_definition;
pub mod violation;

pub use install::{InstallError, InstallOutcome, install};
pub use preparation::{ClassifiedArgumentsV1, PreparationError, PreparedActionV1, prepare_action};
pub use proposal::{
    ProposalRejection, TOOL_CALL_PROPOSAL_VERSION, ToolCallProposalV1, parse_tool_call_proposal,
};
pub use tool_definition::{
    TOOL_DEFINITION_VERSION, ToolDefinitionV1, ToolProjectionError, provider_tools,
};
pub use violation::{ModelSchemaViolation, record_schema_violation};

pub use availability::{
    AvailabilityError, CapabilityAvailabilitySnapshotV1, HostEligibility, Resolution, ResolveError,
};
pub use digests::{
    CATALOG_DOMAIN, DESCRIPTOR_DOMAIN, DigestError, MANIFEST_DOMAIN, ManifestEntryV1,
    descriptor_semantic_digest, manifest_digest,
};
pub use manifest::{CapabilityManifestV1, ManifestError};
pub use provider::{ProviderError, ProviderRegistry};
pub use schema_catalog::{CapabilitySchemaCatalogV1, CatalogError};

pub use serea_storage::{
    CapabilityOverlay, CapabilityOverlayState, DescriptorRevision, DescriptorRevisionDraft,
    GenerationMember, GenerationMemberDraft, RegistryGeneration, RegistryGenerationDraft,
    StepCapabilityBinding,
};

/// Host/admin-facing capability registry storage operations. State changes
/// that affect active authority always append their registry event in the
/// same Store transaction.
pub struct CapabilityRegistry;

impl CapabilityRegistry {
    pub fn create_generation(
        store: &Store,
        draft: RegistryGenerationDraft,
    ) -> Result<RegistryGeneration, StoreError> {
        store.create_registry_generation(draft)
    }

    pub fn insert_descriptor_revision(
        store: &Store,
        draft: DescriptorRevisionDraft,
    ) -> Result<DescriptorRevision, StoreError> {
        store.insert_descriptor_revision(draft)
    }

    pub fn add_generation_member(
        store: &Store,
        draft: GenerationMemberDraft,
    ) -> Result<GenerationMember, StoreError> {
        store.add_generation_membership(draft)
    }

    pub fn set_default_version(
        store: &Store,
        generation_id: i64,
        capability_id: CapabilityId,
        version: SemVer,
    ) -> Result<(), StoreError> {
        store.set_generation_default_version(generation_id, capability_id, version)
    }

    pub fn activate_generation(
        store: &Store,
        events: &EventBus,
        generation_id: i64,
        activated_at: EpochMillis,
    ) -> Result<RegistryGeneration, StoreError> {
        store.transact(|tx| {
            let generation = tx.activate_registry_generation(generation_id, activated_at)?;
            events.append_capability_registry_changed(
                tx,
                CapabilityRegistryChangeV1 {
                    change_kind: CapabilityRegistryChangeKindV1::GenerationActivated,
                    generation_id: Some(generation_id),
                    capability_id: None,
                    overlay_revision: None,
                    manifest_digest: Some(generation.manifest_digest().clone()),
                    schema_catalog_digest: Some(generation.schema_catalog_digest().clone()),
                },
                activated_at,
            )?;
            Ok(generation)
        })
    }

    pub fn current_generation(store: &Store) -> Result<Option<RegistryGeneration>, StoreError> {
        store.current_registry_generation()
    }

    pub fn generation(
        store: &Store,
        generation_id: i64,
    ) -> Result<Option<RegistryGeneration>, StoreError> {
        store.get_registry_generation(generation_id)
    }

    pub fn descriptor_revision(
        store: &Store,
        digest: &Digest,
    ) -> Result<Option<DescriptorRevision>, StoreError> {
        store.get_descriptor_revision(digest)
    }

    pub fn generation_members(
        store: &Store,
        generation_id: i64,
    ) -> Result<Vec<GenerationMember>, StoreError> {
        store.list_generation_members(generation_id)
    }

    pub fn default_version(
        store: &Store,
        generation_id: i64,
        capability_id: &CapabilityId,
    ) -> Result<Option<SemVer>, StoreError> {
        store.get_generation_default_version(generation_id, capability_id)
    }

    pub fn overlay(
        store: &Store,
        capability_id: &CapabilityId,
    ) -> Result<CapabilityOverlay, StoreError> {
        store.get_capability_overlay(capability_id)
    }

    pub fn set_overlay(
        store: &Store,
        events: &EventBus,
        capability_id: CapabilityId,
        expected_revision: u64,
        state: CapabilityOverlayState,
        experimental_opt_in: bool,
        occurred_at: EpochMillis,
    ) -> Result<CapabilityOverlay, StoreError> {
        store.transact(|tx| {
            let before = tx.get_capability_overlay(&capability_id)?;
            let after = tx.set_capability_overlay(
                capability_id.clone(),
                expected_revision,
                state,
                experimental_opt_in,
            )?;
            if before == after {
                return Ok(after);
            }
            if before.state() != after.state() {
                let kind = overlay_change_kind(before.state(), after.state());
                append_overlay_event(
                    events,
                    tx,
                    kind,
                    &capability_id,
                    after.revision(),
                    occurred_at,
                )?;
            }
            if before.experimental_opt_in() != after.experimental_opt_in() {
                let kind = if after.experimental_opt_in() {
                    CapabilityRegistryChangeKindV1::ExperimentalOptIn
                } else {
                    CapabilityRegistryChangeKindV1::ExperimentalOptOut
                };
                append_overlay_event(
                    events,
                    tx,
                    kind,
                    &capability_id,
                    after.revision(),
                    occurred_at,
                )?;
            }
            Ok(after)
        })
    }

    pub fn task_generation(
        store: &Store,
        task_id: &serea_protocol::TaskId,
    ) -> Result<Option<i64>, StoreError> {
        store.get_task_registry_generation(task_id)
    }

    pub fn pin_task_generation(
        store: &Store,
        task_id: &serea_protocol::TaskId,
        generation_id: i64,
    ) -> Result<(), StoreError> {
        store.pin_task_registry_generation(task_id, generation_id)
    }

    pub fn step_binding(
        store: &Store,
        task_id: &serea_protocol::TaskId,
        step_id: &serea_protocol::StepId,
    ) -> Result<Option<StepCapabilityBinding>, StoreError> {
        store.get_step_capability_binding(task_id, step_id)
    }

    pub fn bind_step(
        store: &Store,
        task_id: &serea_protocol::TaskId,
        step_id: &serea_protocol::StepId,
        digest: &Digest,
    ) -> Result<StepCapabilityBinding, StoreError> {
        store.bind_step_capability(task_id, step_id, digest)
    }
}

fn overlay_change_kind(
    before: CapabilityOverlayState,
    after: CapabilityOverlayState,
) -> CapabilityRegistryChangeKindV1 {
    match (before, after) {
        (_, CapabilityOverlayState::Removed) => CapabilityRegistryChangeKindV1::Removed,
        (CapabilityOverlayState::Removed, CapabilityOverlayState::Enabled) => {
            CapabilityRegistryChangeKindV1::Reactivated
        }
        (_, CapabilityOverlayState::Disabled) => CapabilityRegistryChangeKindV1::Disabled,
        (_, CapabilityOverlayState::Enabled) => CapabilityRegistryChangeKindV1::Enabled,
    }
}

fn append_overlay_event(
    events: &EventBus,
    tx: &mut Tx<'_>,
    kind: CapabilityRegistryChangeKindV1,
    capability_id: &CapabilityId,
    revision: u64,
    occurred_at: EpochMillis,
) -> Result<(), StoreError> {
    events.append_capability_registry_changed(
        tx,
        CapabilityRegistryChangeV1 {
            change_kind: kind,
            generation_id: None,
            capability_id: Some(capability_id.clone()),
            overlay_revision: Some(revision),
            manifest_digest: None,
            schema_catalog_digest: None,
        },
        occurred_at,
    )?;
    Ok(())
}
