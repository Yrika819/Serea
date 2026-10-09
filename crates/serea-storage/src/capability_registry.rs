//! Durable P5B capability registry facts. Interpretation and provider
//! discovery remain outside Storage.

use rusqlite::{OptionalExtension, params};
use serea_protocol::{
    CapabilityDescriptor, CapabilityDescriptorDraft, CapabilityId, DescriptorDescription,
    DescriptorTitle, Digest, EpochMillis, ImplementationId, JsonSchemaRef, ProviderId, SemVer,
    StepId, StepKind, TaskId, TaskState, TaskStep,
};

use crate::task::one;
use crate::{Store, StoreError, TransitionContext, Tx};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegistryGenerationDraft {
    pub manifest_digest: Digest,
    pub schema_catalog_digest: Digest,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegistryGeneration {
    generation_id: i64,
    manifest_digest: Digest,
    schema_catalog_digest: Digest,
    activated_at: Option<EpochMillis>,
}

impl RegistryGeneration {
    pub fn generation_id(&self) -> i64 {
        self.generation_id
    }
    pub fn manifest_digest(&self) -> &Digest {
        &self.manifest_digest
    }
    pub fn schema_catalog_digest(&self) -> &Digest {
        &self.schema_catalog_digest
    }
    pub fn activated_at(&self) -> Option<EpochMillis> {
        self.activated_at
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct DescriptorRevisionDraft {
    pub descriptor_digest: Digest,
    pub descriptor: CapabilityDescriptor,
    pub input_schema_digest: Digest,
    pub output_schema_digest: Digest,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DescriptorRevision {
    descriptor_digest: Digest,
    descriptor: CapabilityDescriptor,
    input_schema_digest: Digest,
    output_schema_digest: Digest,
}

impl DescriptorRevision {
    pub fn descriptor_digest(&self) -> &Digest {
        &self.descriptor_digest
    }
    pub fn descriptor(&self) -> &CapabilityDescriptor {
        &self.descriptor
    }
    pub fn input_schema_digest(&self) -> &Digest {
        &self.input_schema_digest
    }
    pub fn output_schema_digest(&self) -> &Digest {
        &self.output_schema_digest
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GenerationMemberDraft {
    pub generation_id: i64,
    pub descriptor_digest: Digest,
    pub candidate_priority: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GenerationMember {
    generation_id: i64,
    descriptor_digest: Digest,
    capability_id: CapabilityId,
    capability_version: SemVer,
    provider_id: ProviderId,
    implementation_id: Option<ImplementationId>,
    candidate_priority: u32,
}

impl GenerationMember {
    pub fn generation_id(&self) -> i64 {
        self.generation_id
    }
    pub fn descriptor_digest(&self) -> &Digest {
        &self.descriptor_digest
    }
    pub fn capability_id(&self) -> &CapabilityId {
        &self.capability_id
    }
    pub fn capability_version(&self) -> &SemVer {
        &self.capability_version
    }
    pub fn provider_id(&self) -> &ProviderId {
        &self.provider_id
    }
    pub fn implementation_id(&self) -> Option<&ImplementationId> {
        self.implementation_id.as_ref()
    }
    pub fn candidate_priority(&self) -> u32 {
        self.candidate_priority
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CapabilityOverlayState {
    Enabled,
    Disabled,
    Removed,
}

impl CapabilityOverlayState {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Enabled => "ENABLED",
            Self::Disabled => "DISABLED",
            Self::Removed => "REMOVED",
        }
    }
    fn parse(value: &str) -> Result<Self, StoreError> {
        match value {
            "ENABLED" => Ok(Self::Enabled),
            "DISABLED" => Ok(Self::Disabled),
            "REMOVED" => Ok(Self::Removed),
            _ => Err(StoreError::CorruptRow),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapabilityOverlay {
    capability_id: CapabilityId,
    state: CapabilityOverlayState,
    experimental_opt_in: bool,
    revision: u64,
}

impl CapabilityOverlay {
    pub fn capability_id(&self) -> &CapabilityId {
        &self.capability_id
    }
    pub fn state(&self) -> CapabilityOverlayState {
        self.state
    }
    pub fn experimental_opt_in(&self) -> bool {
        self.experimental_opt_in
    }
    pub fn revision(&self) -> u64 {
        self.revision
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StepCapabilityBinding {
    task_id: TaskId,
    step_id: StepId,
    generation_id: i64,
    descriptor_digest: Digest,
    capability_id: CapabilityId,
    capability_version: SemVer,
    provider_id: ProviderId,
    implementation_id: Option<ImplementationId>,
}

impl StepCapabilityBinding {
    pub fn task_id(&self) -> &TaskId {
        &self.task_id
    }
    pub fn step_id(&self) -> &StepId {
        &self.step_id
    }
    pub fn generation_id(&self) -> i64 {
        self.generation_id
    }
    pub fn descriptor_digest(&self) -> &Digest {
        &self.descriptor_digest
    }
    pub fn capability_id(&self) -> &CapabilityId {
        &self.capability_id
    }
    pub fn capability_version(&self) -> &SemVer {
        &self.capability_version
    }
    pub fn provider_id(&self) -> &ProviderId {
        &self.provider_id
    }
    pub fn implementation_id(&self) -> Option<&ImplementationId> {
        self.implementation_id.as_ref()
    }
}

fn registry_error(error: rusqlite::Error) -> StoreError {
    if error.sqlite_error_code() == Some(rusqlite::ErrorCode::ConstraintViolation) {
        StoreError::RegistryConflict
    } else {
        error.into()
    }
}

fn decode_generation(
    generation_id: i64,
    manifest_digest: String,
    schema_catalog_digest: String,
    activated_at_ms: Option<i64>,
) -> Result<RegistryGeneration, StoreError> {
    Ok(RegistryGeneration {
        generation_id,
        manifest_digest: Digest::new(manifest_digest).map_err(|_| StoreError::CorruptRow)?,
        schema_catalog_digest: Digest::new(schema_catalog_digest)
            .map_err(|_| StoreError::CorruptRow)?,
        activated_at: activated_at_ms
            .map(EpochMillis::new)
            .transpose()
            .map_err(|_| StoreError::CorruptRow)?,
    })
}

struct RawDescriptorRevision {
    descriptor_digest: String,
    capability_id: String,
    capability_version: String,
    provider_id: String,
    implementation_id: Option<String>,
    title: String,
    description: String,
    input_schema_uri: String,
    input_schema_digest: String,
    output_schema_uri: String,
    output_schema_digest: String,
    side_effect_class: String,
    risk_class: String,
    required_authorization: String,
    replay_safety: String,
    data_class: String,
    root_requirement: String,
    idempotency_support: String,
    max_duration_ms: i64,
    cost_class: String,
    experimental: i64,
}

fn decode_revision(raw: RawDescriptorRevision) -> Result<DescriptorRevision, StoreError> {
    let RawDescriptorRevision {
        descriptor_digest,
        capability_id,
        capability_version,
        provider_id,
        implementation_id,
        title,
        description,
        input_schema_uri,
        input_schema_digest,
        output_schema_uri,
        output_schema_digest,
        side_effect_class,
        risk_class,
        required_authorization,
        replay_safety,
        data_class,
        root_requirement,
        idempotency_support,
        max_duration_ms,
        cost_class,
        experimental,
    } = raw;
    fn valid<T>(result: Result<T, serea_protocol::ProtocolError>) -> Result<T, StoreError> {
        result.map_err(|_| StoreError::CorruptRow)
    }
    fn enum_value<T: serde::de::DeserializeOwned>(value: String) -> Result<T, StoreError> {
        serde_json::from_value(serde_json::Value::String(value)).map_err(|_| StoreError::CorruptRow)
    }
    let descriptor = CapabilityDescriptor::new(CapabilityDescriptorDraft {
        id: valid(CapabilityId::new(capability_id))?,
        version: valid(SemVer::new(capability_version))?,
        provider_id: valid(ProviderId::new(provider_id))?,
        implementation_id: implementation_id
            .map(ImplementationId::new)
            .transpose()
            .map_err(|_| StoreError::CorruptRow)?,
        title: valid(DescriptorTitle::new(title))?,
        description: valid(DescriptorDescription::new(description))?,
        input_schema: valid(JsonSchemaRef::new(input_schema_uri))?,
        output_schema: valid(JsonSchemaRef::new(output_schema_uri))?,
        side_effect_class: enum_value(side_effect_class)?,
        risk_class: enum_value(risk_class)?,
        required_authorization: enum_value(required_authorization)?,
        replay_safety: enum_value(replay_safety)?,
        data_class: enum_value(data_class)?,
        root_requirement: enum_value(root_requirement)?,
        idempotency_support: enum_value(idempotency_support)?,
        max_duration_ms: u32::try_from(max_duration_ms).map_err(|_| StoreError::CorruptRow)?,
        cost_class: enum_value(cost_class)?,
        experimental: match experimental {
            0 => false,
            1 => true,
            _ => return Err(StoreError::CorruptRow),
        },
    })
    .map_err(|_| StoreError::CorruptRow)?;
    Ok(DescriptorRevision {
        descriptor_digest: Digest::new(descriptor_digest).map_err(|_| StoreError::CorruptRow)?,
        descriptor,
        input_schema_digest: Digest::new(input_schema_digest)
            .map_err(|_| StoreError::CorruptRow)?,
        output_schema_digest: Digest::new(output_schema_digest)
            .map_err(|_| StoreError::CorruptRow)?,
    })
}

impl Store {
    pub fn create_registry_generation(
        &self,
        draft: RegistryGenerationDraft,
    ) -> Result<RegistryGeneration, StoreError> {
        self.transact(|tx| tx.create_registry_generation(draft))
    }
    pub fn get_registry_generation(
        &self,
        generation_id: i64,
    ) -> Result<Option<RegistryGeneration>, StoreError> {
        self.transact(|tx| tx.get_registry_generation(generation_id))
    }
    pub fn current_registry_generation(&self) -> Result<Option<RegistryGeneration>, StoreError> {
        self.transact(|tx| tx.current_registry_generation())
    }
    pub fn insert_descriptor_revision(
        &self,
        draft: DescriptorRevisionDraft,
    ) -> Result<DescriptorRevision, StoreError> {
        self.transact(|tx| tx.insert_descriptor_revision(draft))
    }
    pub fn get_descriptor_revision(
        &self,
        digest: &Digest,
    ) -> Result<Option<DescriptorRevision>, StoreError> {
        self.transact(|tx| tx.get_descriptor_revision(digest))
    }
    pub fn add_generation_membership(
        &self,
        draft: GenerationMemberDraft,
    ) -> Result<GenerationMember, StoreError> {
        self.transact(|tx| tx.add_generation_membership(draft))
    }
    pub fn list_generation_members(
        &self,
        generation_id: i64,
    ) -> Result<Vec<GenerationMember>, StoreError> {
        self.transact(|tx| tx.list_generation_members(generation_id))
    }
    pub fn set_generation_default_version(
        &self,
        generation_id: i64,
        capability_id: CapabilityId,
        version: SemVer,
    ) -> Result<(), StoreError> {
        self.transact(|tx| tx.set_generation_default_version(generation_id, capability_id, version))
    }
    pub fn get_generation_default_version(
        &self,
        generation_id: i64,
        capability_id: &CapabilityId,
    ) -> Result<Option<SemVer>, StoreError> {
        self.transact(|tx| tx.get_generation_default_version(generation_id, capability_id))
    }
    pub fn get_capability_overlay(
        &self,
        capability_id: &CapabilityId,
    ) -> Result<CapabilityOverlay, StoreError> {
        self.transact(|tx| tx.get_capability_overlay(capability_id))
    }
    pub fn get_task_registry_generation(
        &self,
        task_id: &TaskId,
    ) -> Result<Option<i64>, StoreError> {
        self.transact(|tx| tx.get_task_registry_generation(task_id))
    }
    pub fn pin_task_registry_generation(
        &self,
        task_id: &TaskId,
        generation_id: i64,
    ) -> Result<(), StoreError> {
        self.transact(|tx| tx.pin_task_registry_generation(task_id, generation_id))
    }
    pub fn get_step_capability_binding(
        &self,
        task_id: &TaskId,
        step_id: &StepId,
    ) -> Result<Option<StepCapabilityBinding>, StoreError> {
        self.transact(|tx| tx.get_step_capability_binding(task_id, step_id))
    }
    pub fn bind_step_capability(
        &self,
        task_id: &TaskId,
        step_id: &StepId,
        descriptor_digest: &Digest,
    ) -> Result<StepCapabilityBinding, StoreError> {
        self.transact(|tx| tx.bind_step_capability(task_id, step_id, descriptor_digest))
    }
}

impl Tx<'_> {
    pub fn create_registry_generation(
        &mut self,
        draft: RegistryGenerationDraft,
    ) -> Result<RegistryGeneration, StoreError> {
        self.ensure_active()?;
        self.inner.execute(
            "INSERT INTO capability_registry_generations(manifest_digest,schema_catalog_digest) VALUES (?1,?2)",
            params![draft.manifest_digest.as_str(), draft.schema_catalog_digest.as_str()],
        ).map_err(registry_error)?;
        let id = self.inner.last_insert_rowid();
        decode_generation(
            id,
            draft.manifest_digest.to_string(),
            draft.schema_catalog_digest.to_string(),
            None,
        )
    }

    pub fn get_registry_generation(
        &mut self,
        generation_id: i64,
    ) -> Result<Option<RegistryGeneration>, StoreError> {
        self.ensure_active()?;
        self.inner.query_row(
            "SELECT generation_id,manifest_digest,schema_catalog_digest,activated_at_ms FROM capability_registry_generations WHERE generation_id=?1",
            [generation_id], |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?)),
        ).optional()?.map(|(id,m,s,a)| decode_generation(id,m,s,a)).transpose()
    }

    pub fn current_registry_generation(
        &mut self,
    ) -> Result<Option<RegistryGeneration>, StoreError> {
        self.ensure_active()?;
        self.inner
            .query_row(
                "SELECT g.generation_id,g.manifest_digest,g.schema_catalog_digest,g.activated_at_ms
               FROM capability_registry_state AS h JOIN capability_registry_generations AS g
                 ON g.generation_id=h.active_generation_id WHERE h.singleton=1",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            )
            .optional()?
            .map(|(id, m, s, a)| decode_generation(id, m, s, a))
            .transpose()
    }

    pub fn insert_descriptor_revision(
        &mut self,
        draft: DescriptorRevisionDraft,
    ) -> Result<DescriptorRevision, StoreError> {
        self.ensure_active()?;
        let d = &draft.descriptor;
        let existing = self.get_descriptor_revision(&draft.descriptor_digest)?;
        let proposed = DescriptorRevision {
            descriptor_digest: draft.descriptor_digest.clone(),
            descriptor: draft.descriptor.clone(),
            input_schema_digest: draft.input_schema_digest.clone(),
            output_schema_digest: draft.output_schema_digest.clone(),
        };
        if let Some(existing) = existing {
            return if existing == proposed {
                Ok(existing)
            } else {
                Err(StoreError::RegistryRevisionConflict)
            };
        }
        self.inner.execute(
            "INSERT INTO capability_descriptor_revisions(descriptor_digest,capability_id,capability_version,provider_id,implementation_id,title,description,input_schema_uri,input_schema_digest,output_schema_uri,output_schema_digest,side_effect_class,risk_class,required_authorization,replay_safety,data_class,root_requirement,idempotency_support,max_duration_ms,cost_class,experimental) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19,?20,?21)",
            params![draft.descriptor_digest.as_str(),d.id().as_str(),d.version().as_str(),d.provider_id().as_str(),d.implementation_id().map(ImplementationId::as_str),d.title().as_str(),d.description().as_str(),d.input_schema().as_str(),draft.input_schema_digest.as_str(),d.output_schema().as_str(),draft.output_schema_digest.as_str(),d.side_effect_class().wire_name(),d.risk_class().wire_name(),d.required_authorization().wire_name(),d.replay_safety().wire_name(),d.data_class().wire_name(),d.root_requirement().wire_name(),d.idempotency_support().wire_name(),d.max_duration_ms(),d.cost_class().wire_name(),d.experimental()],
        ).map_err(registry_error)?;
        Ok(proposed)
    }

    pub fn get_descriptor_revision(
        &mut self,
        digest: &Digest,
    ) -> Result<Option<DescriptorRevision>, StoreError> {
        self.ensure_active()?;
        self.inner.query_row(
            "SELECT descriptor_digest,capability_id,capability_version,provider_id,implementation_id,title,description,input_schema_uri,input_schema_digest,output_schema_uri,output_schema_digest,side_effect_class,risk_class,required_authorization,replay_safety,data_class,root_requirement,idempotency_support,max_duration_ms,cost_class,experimental FROM capability_descriptor_revisions WHERE descriptor_digest=?1",
            [digest.as_str()], |r| Ok(RawDescriptorRevision {
                descriptor_digest: r.get(0)?,
                capability_id: r.get(1)?,
                capability_version: r.get(2)?,
                provider_id: r.get(3)?,
                implementation_id: r.get(4)?,
                title: r.get(5)?,
                description: r.get(6)?,
                input_schema_uri: r.get(7)?,
                input_schema_digest: r.get(8)?,
                output_schema_uri: r.get(9)?,
                output_schema_digest: r.get(10)?,
                side_effect_class: r.get(11)?,
                risk_class: r.get(12)?,
                required_authorization: r.get(13)?,
                replay_safety: r.get(14)?,
                data_class: r.get(15)?,
                root_requirement: r.get(16)?,
                idempotency_support: r.get(17)?,
                max_duration_ms: r.get(18)?,
                cost_class: r.get(19)?,
                experimental: r.get(20)?,
            }),
        ).optional()?.map(decode_revision).transpose()
    }

    pub fn add_generation_membership(
        &mut self,
        draft: GenerationMemberDraft,
    ) -> Result<GenerationMember, StoreError> {
        self.ensure_active()?;
        let revision = self
            .get_descriptor_revision(&draft.descriptor_digest)?
            .ok_or(StoreError::RegistryRevisionNotFound)?;
        let d = revision.descriptor();
        self.inner.execute(
            "INSERT INTO capability_generation_members(generation_id,descriptor_digest,capability_id,capability_version,provider_id,implementation_id,candidate_priority) VALUES (?1,?2,?3,?4,?5,?6,?7)",
            params![draft.generation_id,draft.descriptor_digest.as_str(),d.id().as_str(),d.version().as_str(),d.provider_id().as_str(),d.implementation_id().map(ImplementationId::as_str),i64::from(draft.candidate_priority)],
        ).map_err(registry_error)?;
        Ok(GenerationMember {
            generation_id: draft.generation_id,
            descriptor_digest: draft.descriptor_digest,
            capability_id: d.id().clone(),
            capability_version: d.version().clone(),
            provider_id: d.provider_id().clone(),
            implementation_id: d.implementation_id().cloned(),
            candidate_priority: draft.candidate_priority,
        })
    }

    pub fn list_generation_members(
        &mut self,
        generation_id: i64,
    ) -> Result<Vec<GenerationMember>, StoreError> {
        self.ensure_active()?;
        let mut stmt = self.inner.prepare("SELECT descriptor_digest,capability_id,capability_version,provider_id,implementation_id,candidate_priority FROM capability_generation_members WHERE generation_id=?1 ORDER BY capability_id,capability_version,candidate_priority,descriptor_digest")?;
        let raw = stmt
            .query_map([generation_id], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, String>(3)?,
                    r.get::<_, Option<String>>(4)?,
                    r.get::<_, i64>(5)?,
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        raw.into_iter()
            .map(|x| {
                Ok(GenerationMember {
                    generation_id,
                    descriptor_digest: Digest::new(x.0).map_err(|_| StoreError::CorruptRow)?,
                    capability_id: CapabilityId::new(x.1).map_err(|_| StoreError::CorruptRow)?,
                    capability_version: SemVer::new(x.2).map_err(|_| StoreError::CorruptRow)?,
                    provider_id: ProviderId::new(x.3).map_err(|_| StoreError::CorruptRow)?,
                    implementation_id: x
                        .4
                        .map(ImplementationId::new)
                        .transpose()
                        .map_err(|_| StoreError::CorruptRow)?,
                    candidate_priority: u32::try_from(x.5).map_err(|_| StoreError::CorruptRow)?,
                })
            })
            .collect()
    }

    pub fn set_generation_default_version(
        &mut self,
        generation_id: i64,
        capability_id: CapabilityId,
        version: SemVer,
    ) -> Result<(), StoreError> {
        self.ensure_active()?;
        self.inner.execute("INSERT INTO capability_generation_defaults(generation_id,capability_id,capability_version) VALUES (?1,?2,?3)", params![generation_id,capability_id.as_str(),version.as_str()]).map_err(registry_error)?;
        Ok(())
    }
    pub fn get_generation_default_version(
        &mut self,
        generation_id: i64,
        capability_id: &CapabilityId,
    ) -> Result<Option<SemVer>, StoreError> {
        self.ensure_active()?;
        let v: Option<String> = self.inner.query_row("SELECT capability_version FROM capability_generation_defaults WHERE generation_id=?1 AND capability_id=?2", params![generation_id,capability_id.as_str()], |r| r.get(0)).optional()?;
        v.map(SemVer::new)
            .transpose()
            .map_err(|_| StoreError::CorruptRow)
    }

    pub fn activate_registry_generation(
        &mut self,
        generation_id: i64,
        activated_at: EpochMillis,
    ) -> Result<RegistryGeneration, StoreError> {
        self.ensure_active()?;
        let generation = self
            .get_registry_generation(generation_id)?
            .ok_or(StoreError::RegistryGenerationNotFound)?;
        if generation.activated_at.is_some() {
            return Err(StoreError::RegistryGenerationActivated);
        }
        let members = self.list_generation_members(generation_id)?;
        if members.is_empty() {
            return Err(StoreError::RegistryGenerationIncomplete);
        }
        let mut capability_ids: Vec<&CapabilityId> = Vec::new();
        for member in &members {
            if !capability_ids.contains(&&member.capability_id) {
                capability_ids.push(&member.capability_id);
            }
        }
        for id in capability_ids {
            let default = self.get_generation_default_version(generation_id, id)?;
            let Some(default) = default else {
                return Err(StoreError::RegistryGenerationIncomplete);
            };
            if !members
                .iter()
                .any(|m| &m.capability_id == id && m.capability_version == default)
            {
                return Err(StoreError::RegistryGenerationIncomplete);
            }
        }
        let active = self.current_registry_generation()?;
        if active
            .as_ref()
            .is_some_and(|g| g.generation_id >= generation_id)
        {
            return Err(StoreError::RegistryGenerationOrder);
        }
        self.inner.execute("UPDATE capability_registry_generations SET activated_at_ms=?1 WHERE generation_id=?2 AND activated_at_ms IS NULL", params![activated_at.get(),generation_id]).map_err(registry_error)?;
        self.inner
            .execute(
                "UPDATE capability_registry_state SET active_generation_id=?1 WHERE singleton=1",
                [generation_id],
            )
            .map_err(registry_error)?;
        self.get_registry_generation(generation_id)?
            .ok_or(StoreError::CorruptRow)
    }

    pub fn get_capability_overlay(
        &mut self,
        capability_id: &CapabilityId,
    ) -> Result<CapabilityOverlay, StoreError> {
        self.ensure_active()?;
        let row: Option<(String,i64,i64)> = self.inner.query_row("SELECT availability_state,experimental_opt_in,revision FROM capability_overlays WHERE capability_id=?1", [capability_id.as_str()], |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?))).optional()?;
        // An absent row is the explicit stable default: enabled, no
        // experimental opt-in, revision zero.
        let (state, opt, revision) = row.unwrap_or_else(|| ("ENABLED".into(), 0, 0));
        Ok(CapabilityOverlay {
            capability_id: capability_id.clone(),
            state: CapabilityOverlayState::parse(&state)?,
            experimental_opt_in: match opt {
                0 => false,
                1 => true,
                _ => return Err(StoreError::CorruptRow),
            },
            revision: u64::try_from(revision).map_err(|_| StoreError::CorruptRow)?,
        })
    }

    pub fn set_capability_overlay(
        &mut self,
        capability_id: CapabilityId,
        expected_revision: u64,
        state: CapabilityOverlayState,
        experimental_opt_in: bool,
    ) -> Result<CapabilityOverlay, StoreError> {
        self.ensure_active()?;
        let current = self.get_capability_overlay(&capability_id)?;
        if current.revision != expected_revision {
            return Err(StoreError::RegistryOverlayConflict);
        }
        if current.state == state && current.experimental_opt_in == experimental_opt_in {
            return Ok(current);
        }
        let revision = current
            .revision
            .checked_add(1)
            .ok_or(StoreError::RegistryOverlayConflict)?;
        self.inner.execute("INSERT INTO capability_overlays(capability_id,availability_state,experimental_opt_in,revision) VALUES (?1,?2,?3,?4) ON CONFLICT(capability_id) DO UPDATE SET availability_state=excluded.availability_state,experimental_opt_in=excluded.experimental_opt_in,revision=excluded.revision", params![capability_id.as_str(),state.as_str(),experimental_opt_in,i64::try_from(revision).map_err(|_| StoreError::RegistryOverlayConflict)?]).map_err(registry_error)?;
        Ok(CapabilityOverlay {
            capability_id,
            state,
            experimental_opt_in,
            revision,
        })
    }

    pub fn get_task_registry_generation(
        &mut self,
        task_id: &TaskId,
    ) -> Result<Option<i64>, StoreError> {
        self.ensure_active()?;
        self.inner
            .query_row(
                "SELECT capability_registry_generation FROM tasks WHERE task_id=?1",
                [task_id.as_str()],
                |r| r.get(0),
            )
            .optional()?
            .ok_or(StoreError::TaskNotFound)
    }

    pub fn pin_task_registry_generation(
        &mut self,
        task_id: &TaskId,
        generation_id: i64,
    ) -> Result<(), StoreError> {
        self.ensure_active()?;
        let generation = self
            .get_registry_generation(generation_id)?
            .ok_or(StoreError::RegistryGenerationNotFound)?;
        if generation.activated_at.is_none() {
            return Err(StoreError::RegistryGenerationNotActive);
        }
        let changed = self.inner.execute("UPDATE tasks SET capability_registry_generation=?1 WHERE task_id=?2 AND capability_registry_generation IS NULL", params![generation_id,task_id.as_str()])?;
        if changed != 1 {
            return Err(StoreError::RegistryTaskAlreadyPinned);
        }
        Ok(())
    }

    /// Whether any registry generation row has ever existed.
    ///
    /// This distinguishes a pre-P5 database, where a NULL Task pin is the
    /// accepted legacy state, from a post-P5 database whose active generation
    /// is missing, which must fail closed.
    pub fn has_registry_generations(&mut self) -> Result<bool, StoreError> {
        self.ensure_active()?;
        self.inner
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM capability_registry_generations)",
                [],
                |r| r.get(0),
            )
            .map_err(|error| error.into())
    }

    /// Appends one capability Step row and binds it to `descriptor_digest` in
    /// the caller's transaction, so no half-created capability Step is ever
    /// observable.
    ///
    /// This is a narrow, typed operation, not a generic SQL seam: it takes one
    /// Step and one already-resolved descriptor digest, reuses the ordinary
    /// plan input validation, writes the input blob and step row through the
    /// existing blob path, and refuses a Step that already exists. The caller
    /// resolves the candidate; this method re-checks the pinned generation, the
    /// descriptor revision, the live overlay and any existing binding exactly
    /// as `bind_step_capability` does.
    pub fn append_capability_step(
        &mut self,
        task_id: &TaskId,
        step: &TaskStep,
        input_json: &[u8],
        descriptor_digest: &Digest,
        now: EpochMillis,
        context: &TransitionContext<'_>,
    ) -> Result<StepCapabilityBinding, StoreError> {
        let class = crate::task::task_class(&self.inner, task_id)?;
        let state: String = self.inner.query_row(
            "SELECT state FROM tasks WHERE task_id=?1",
            [task_id.as_str()],
            |r| r.get(0),
        )?;
        if state != TaskState::Planning.wire_name() {
            return Err(StoreError::IllegalTaskTransition);
        }
        let before = self.load_task(task_id)?;
        let next = before
            .plan_revision
            .checked_add(1)
            .ok_or(StoreError::PlanRevisionOverflow)?;
        if now < before.task.updated_at.to_epoch_millis() {
            return Err(StoreError::InvalidTimestamp);
        }
        if &step.task_id != task_id {
            return Err(StoreError::InvalidPlan);
        }
        // A Step already present durably, or already bound, is never rewritten.
        let occupied: bool = self.inner.query_row(
            "SELECT EXISTS(SELECT 1 FROM task_steps WHERE step_id=?1)",
            [step.step_id.as_str()],
            |r| r.get(0),
        )?;
        if occupied
            || self
                .get_step_capability_binding(task_id, &step.step_id)?
                .is_some()
        {
            return Err(StoreError::InvalidPlan);
        }
        let sequence_taken: bool = self.inner.query_row(
            "SELECT EXISTS(SELECT 1 FROM task_steps WHERE task_id=?1 AND sequence=?2)",
            rusqlite::params![task_id.as_str(), step.sequence],
            |r| r.get(0),
        )?;
        if sequence_taken {
            return Err(StoreError::DuplicateSequence);
        }
        // Validate everything that needs no write before any write happens.
        // validate_input canonicalises; the blob and the plan document then
        // store that exact text, so they agree byte for byte.
        let stored_input = crate::task::validate_input(task_id, step, input_json)?;
        let revision = self
            .get_descriptor_revision(descriptor_digest)?
            .ok_or(StoreError::RegistryRevisionNotFound)?;
        let descriptor = revision.descriptor().clone();
        // The overlay, the pinned generation and the candidate facts are
        // re-checked before the step row exists, so a refused binding leaves no
        // step row behind.
        let overlay = self.get_capability_overlay(descriptor.id())?;
        if overlay.state != CapabilityOverlayState::Enabled
            || (descriptor.experimental() && !overlay.experimental_opt_in)
        {
            return Err(StoreError::RegistryCapabilityUnavailable);
        }
        if step.kind != StepKind::Capability
            || step.capability_id.as_ref() != Some(descriptor.id())
            || step.capability_version.as_ref() != Some(descriptor.version())
            || step.provider_id.as_ref() != Some(descriptor.provider_id())
        {
            return Err(StoreError::RegistryBindingRefused);
        }
        let input_blob = self.put_blob(stored_input.as_bytes(), class)?;
        let changed = self.inner.execute(
            "INSERT INTO task_steps(step_id,task_id,sequence,kind,status,attempt,plan_revision,
                provider_id,capability_id,capability_version,idempotency_key,input_digest)
                VALUES(?1,?2,?3,?4,'PLANNED',0,?5,?6,?7,?8,?9,?10)",
            rusqlite::params![
                step.step_id.as_str(),
                task_id.as_str(),
                step.sequence,
                step.kind.wire_name(),
                next,
                step.provider_id.as_ref().map(|v| v.as_str()),
                step.capability_id.as_ref().map(|v| v.as_str()),
                step.capability_version.as_ref().map(|v| v.as_str()),
                step.idempotency_key.as_ref().map(|v| v.as_str()),
                step.input_digest.as_str()
            ],
        )?;
        one(changed, StoreError::InvalidPlan)?;
        self.inner.execute(
            "INSERT INTO step_blob_refs(step_id,role,digest,data_class_rank) VALUES(?1,?2,?3,?4)",
            rusqlite::params![
                step.step_id.as_str(),
                crate::task::input_role(step.kind),
                input_blob.digest().as_str(),
                class.rank()
            ],
        )?;
        let updated = self.inner.execute(
            "UPDATE tasks SET plan_revision=?1,updated_at_ms=?2 WHERE task_id=?3 AND state='PLANNING' AND plan_revision=?4",
            rusqlite::params![next, now.get(), task_id.as_str(), before.plan_revision],
        )?;
        one(updated, StoreError::PlanRevisionConflict)?;
        // The plan revision row is what keeps `plan_revision` and history
        // consistent: an appended Step is a new plan revision, so its document
        // records the steps this revision introduces alongside the ones it
        // retains.
        let document = crate::task::PlanDocument {
            task_id: task_id.clone(),
            revision: next,
            steps: vec![crate::task::StoredInput {
                step: step.clone(),
                input_json: stored_input,
            }],
        };
        let plan_json = crate::task::json_bytes(&document)?;
        let blob = self.put_blob(&plan_json, class)?;
        one(
            self.inner.execute(
                "INSERT INTO plan_revisions(task_id,plan_revision,created_at_ms,plan_digest,data_class_rank,step_count)
                 VALUES(?1,?2,?3,?4,?5,?6)",
                rusqlite::params![
                    task_id.as_str(),
                    next,
                    now.get(),
                    blob.digest().as_str(),
                    class.rank(),
                    u32::try_from(document.steps.len()).map_err(|_| StoreError::PlanRevisionOverflow)?
                ],
            )?,
            StoreError::PlanRevisionConflict,
        )?;
        for role in ["PLAN", "PLAN_REVISION"] {
            self.inner.execute(
                "INSERT INTO task_blob_refs(task_id,role,digest,data_class_rank) VALUES(?1,?2,?3,?4)",
                rusqlite::params![
                    task_id.as_str(),
                    role,
                    blob.digest().as_str(),
                    class.rank()
                ],
            )?;
        }
        // The step row now exists, so the binding trigger can verify it.
        let binding = self.bind_step_capability(task_id, &step.step_id, descriptor_digest)?;
        let _ = context;
        Ok(binding)
    }

    pub fn get_step_capability_binding(
        &mut self,
        task_id: &TaskId,
        step_id: &StepId,
    ) -> Result<Option<StepCapabilityBinding>, StoreError> {
        self.ensure_active()?;
        let row = self.inner.query_row("SELECT generation_id,descriptor_digest,capability_id,capability_version,provider_id,implementation_id FROM step_capability_bindings WHERE task_id=?1 AND step_id=?2", params![task_id.as_str(),step_id.as_str()], |r| Ok((r.get::<_,i64>(0)?,r.get::<_,String>(1)?,r.get::<_,String>(2)?,r.get::<_,String>(3)?,r.get::<_,String>(4)?,r.get::<_,Option<String>>(5)?))).optional()?;
        row.map(|x| {
            Ok(StepCapabilityBinding {
                task_id: task_id.clone(),
                step_id: step_id.clone(),
                generation_id: x.0,
                descriptor_digest: Digest::new(x.1).map_err(|_| StoreError::CorruptRow)?,
                capability_id: CapabilityId::new(x.2).map_err(|_| StoreError::CorruptRow)?,
                capability_version: SemVer::new(x.3).map_err(|_| StoreError::CorruptRow)?,
                provider_id: ProviderId::new(x.4).map_err(|_| StoreError::CorruptRow)?,
                implementation_id: x
                    .5
                    .map(ImplementationId::new)
                    .transpose()
                    .map_err(|_| StoreError::CorruptRow)?,
            })
        })
        .transpose()
    }

    pub fn bind_step_capability(
        &mut self,
        task_id: &TaskId,
        step_id: &StepId,
        descriptor_digest: &Digest,
    ) -> Result<StepCapabilityBinding, StoreError> {
        self.ensure_active()?;
        if let Some(existing) = self.get_step_capability_binding(task_id, step_id)? {
            return if existing.descriptor_digest == *descriptor_digest {
                Ok(existing)
            } else {
                Err(StoreError::RegistryBindingConflict)
            };
        }
        let generation_id: Option<i64> = self
            .inner
            .query_row(
                "SELECT capability_registry_generation FROM tasks WHERE task_id=?1",
                [task_id.as_str()],
                |r| r.get(0),
            )
            .optional()?
            .ok_or(StoreError::TaskNotFound)?;
        let generation_id = generation_id.ok_or(StoreError::RegistryTaskUnpinned)?;
        let revision = self
            .get_descriptor_revision(descriptor_digest)?
            .ok_or(StoreError::RegistryRevisionNotFound)?;
        let overlay = self.get_capability_overlay(revision.descriptor().id())?;
        if overlay.state != CapabilityOverlayState::Enabled
            || (revision.descriptor().experimental() && !overlay.experimental_opt_in)
        {
            return Err(StoreError::RegistryCapabilityUnavailable);
        }
        let d = revision.descriptor();
        self.inner.execute("INSERT INTO step_capability_bindings(task_id,step_id,generation_id,descriptor_digest,capability_id,capability_version,provider_id,implementation_id) VALUES (?1,?2,?3,?4,?5,?6,?7,?8)", params![task_id.as_str(),step_id.as_str(),generation_id,descriptor_digest.as_str(),d.id().as_str(),d.version().as_str(),d.provider_id().as_str(),d.implementation_id().map(ImplementationId::as_str)]).map_err(|e| if e.sqlite_error_code()==Some(rusqlite::ErrorCode::ConstraintViolation) { StoreError::RegistryBindingRefused } else { e.into() })?;
        self.get_step_capability_binding(task_id, step_id)?
            .ok_or(StoreError::CorruptRow)
    }
}
