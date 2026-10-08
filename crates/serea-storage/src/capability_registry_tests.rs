//! P5B RED-first durable registry identity and generation tests.

use crate::{
    CapabilityOverlayState, DescriptorRevisionDraft, GenerationMemberDraft,
    RegistryGenerationDraft, Store, StoreError,
};
use serea_protocol::{
    Authorization, CapabilityDescriptor, CapabilityDescriptorDraft, CapabilityId, Clock, CostClass,
    DataClass, DescriptorDescription, DescriptorTitle, Digest, EpochMillis, IdempotencySupport,
    ImplementationId, JsonSchemaRef, ProtocolError, ProviderId, ReplaySafety, RiskClass,
    RootRequirement, SemVer, SideEffectClass, StepId, TaskId,
};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);

struct FixedClock;
impl Clock for FixedClock {
    fn now_ms(&self) -> Result<EpochMillis, ProtocolError> {
        EpochMillis::new(1_796_000_000_000)
    }
}

fn digest(ch: char) -> Digest {
    Digest::new(format!("sha256:{}", ch.to_string().repeat(64))).unwrap()
}

fn descriptor(version: &str, implementation: Option<&str>) -> CapabilityDescriptor {
    CapabilityDescriptor::new(CapabilityDescriptorDraft {
        id: CapabilityId::new("calendar.events.read").unwrap(),
        version: SemVer::new(version).unwrap(),
        title: DescriptorTitle::new("Read events").unwrap(),
        description: DescriptorDescription::new("Read calendar events").unwrap(),
        provider_id: ProviderId::new("calendar").unwrap(),
        implementation_id: implementation.map(|value| ImplementationId::new(value).unwrap()),
        input_schema: JsonSchemaRef::new("serea://calendar/read-input").unwrap(),
        output_schema: JsonSchemaRef::new("serea://calendar/read-output").unwrap(),
        side_effect_class: SideEffectClass::None,
        risk_class: RiskClass::Observe,
        required_authorization: Authorization::None,
        replay_safety: ReplaySafety::Idempotent,
        data_class: DataClass::Personal,
        root_requirement: RootRequirement::NotRequired,
        idempotency_support: IdempotencySupport::None,
        max_duration_ms: 5_000,
        cost_class: CostClass::Free,
        experimental: false,
    })
    .unwrap()
}

fn revision(ch: char, version: &str, implementation: Option<&str>) -> DescriptorRevisionDraft {
    DescriptorRevisionDraft {
        descriptor_digest: digest(ch),
        descriptor: descriptor(version, implementation),
        input_schema_digest: digest('a'),
        output_schema_digest: digest('b'),
    }
}

fn generation(store: &Store) -> i64 {
    store
        .create_registry_generation(RegistryGenerationDraft {
            manifest_digest: digest('c'),
            schema_catalog_digest: digest('d'),
        })
        .unwrap()
        .generation_id()
}

struct TempDb(PathBuf);
impl TempDb {
    fn new() -> Self {
        let dir = std::env::temp_dir().join(format!(
            "serea-p5b-registry-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&dir).unwrap();
        Self(dir.join("store.sqlite"))
    }
}
impl Drop for TempDb {
    fn drop(&mut self) {
        std::fs::remove_dir_all(self.0.parent().unwrap()).unwrap();
    }
}

fn activated_candidate_store(
    store: &Store,
    implementations: &[(&str, char)],
) -> (i64, Vec<Digest>) {
    let generation_id = generation(store);
    let mut digests = Vec::new();
    for (priority, (implementation, ch)) in implementations.iter().enumerate() {
        let digest = digest(*ch);
        store
            .insert_descriptor_revision(revision(*ch, "1.0.0", Some(implementation)))
            .unwrap();
        store
            .add_generation_membership(GenerationMemberDraft {
                generation_id,
                descriptor_digest: digest.clone(),
                candidate_priority: u32::try_from(priority).unwrap(),
            })
            .unwrap();
        digests.push(digest);
    }
    store
        .set_generation_default_version(
            generation_id,
            CapabilityId::new("calendar.events.read").unwrap(),
            SemVer::new("1.0.0").unwrap(),
        )
        .unwrap();
    store
        .transact(|tx| {
            tx.activate_registry_generation(
                generation_id,
                EpochMillis::new(1_796_000_000_000).unwrap(),
            )
        })
        .unwrap();
    (generation_id, digests)
}

fn task_and_step(store: &Store, task_id: &str, step_id: &str, generation_id: Option<i64>) {
    let conn = store.conn.lock().unwrap();
    conn.execute(
        "INSERT INTO tasks(task_id,kind,title,state,origin_kind,data_class_rank,policy_class_rank,created_at_ms,updated_at_ms,max_model_calls,max_tool_calls,max_attempts_per_step)
         VALUES (?1,'MAINTENANCE','binding fixture','RECEIVED','SYSTEM',0,0,0,0,12,0,0)",
        [task_id],
    ).unwrap();
    if let Some(generation_id) = generation_id {
        conn.execute(
            "UPDATE tasks SET capability_registry_generation=?1 WHERE task_id=?2",
            rusqlite::params![generation_id, task_id],
        )
        .unwrap();
    }
    conn.execute(
        "INSERT INTO task_steps(step_id,task_id,sequence,kind,status,provider_id,capability_id,capability_version,idempotency_key,input_digest)
         VALUES (?1,?2,0,'CAPABILITY','PLANNED','calendar','calendar.events.read','1.0.0',?3,?4)",
        rusqlite::params![step_id, task_id, format!("idk_{}", "c".repeat(64)), digest('f').as_str()],
    ).unwrap();
}

#[test]
fn generation_ids_are_positive_monotonic_and_active_pointer_is_not_max() {
    let store = Store::open_in_memory(&FixedClock).unwrap();
    let first = generation(&store);
    store
        .insert_descriptor_revision(revision('1', "1.0.0", None))
        .unwrap();
    store
        .add_generation_membership(GenerationMemberDraft {
            generation_id: first,
            descriptor_digest: digest('1'),
            candidate_priority: 0,
        })
        .unwrap();
    store
        .set_generation_default_version(
            first,
            CapabilityId::new("calendar.events.read").unwrap(),
            SemVer::new("1.0.0").unwrap(),
        )
        .unwrap();
    store
        .transact(|tx| {
            tx.activate_registry_generation(first, EpochMillis::new(1_796_000_000_000).unwrap())
        })
        .unwrap();

    let prepared = generation(&store);
    assert!(prepared > first);
    assert_eq!(
        store
            .current_registry_generation()
            .unwrap()
            .unwrap()
            .generation_id(),
        first
    );
    assert_eq!(
        store
            .get_registry_generation(prepared)
            .unwrap()
            .unwrap()
            .activated_at(),
        None
    );
}

#[test]
fn descriptor_revisions_reconstruct_exact_facts_and_reject_digest_conflicts() {
    let store = Store::open_in_memory(&FixedClock).unwrap();
    let original = revision('1', "1.0.0", None);
    store.insert_descriptor_revision(original.clone()).unwrap();
    store.insert_descriptor_revision(original.clone()).unwrap();
    assert_eq!(
        store
            .get_descriptor_revision(&digest('1'))
            .unwrap()
            .unwrap()
            .descriptor(),
        &original.descriptor
    );
    assert_eq!(
        store.insert_descriptor_revision(revision('1', "2.0.0", None)),
        Err(StoreError::RegistryRevisionConflict)
    );
}

#[test]
fn generation_members_keep_versions_implementations_priorities_and_default_explicit() {
    let store = Store::open_in_memory(&FixedClock).unwrap();
    let generation_id = generation(&store);
    for (ch, version, implementation, priority) in [
        ('1', "1.0.0", Some("rootless"), 0),
        ('2', "1.0.0", Some("rooted"), 1),
        ('3', "2.0.0", None, 0),
    ] {
        store
            .insert_descriptor_revision(revision(ch, version, implementation))
            .unwrap();
        store
            .add_generation_membership(GenerationMemberDraft {
                generation_id,
                descriptor_digest: digest(ch),
                candidate_priority: priority,
            })
            .unwrap();
    }
    store
        .set_generation_default_version(
            generation_id,
            CapabilityId::new("calendar.events.read").unwrap(),
            SemVer::new("2.0.0").unwrap(),
        )
        .unwrap();
    let members = store.list_generation_members(generation_id).unwrap();
    assert_eq!(members.len(), 3);
    assert_eq!(members[0].candidate_priority(), 0);
    assert_eq!(members[1].candidate_priority(), 1);
    assert_eq!(
        store
            .get_generation_default_version(
                generation_id,
                &CapabilityId::new("calendar.events.read").unwrap()
            )
            .unwrap()
            .unwrap()
            .as_str(),
        "2.0.0"
    );
}

#[test]
fn null_implementation_is_rejected_when_version_has_multiple_candidates() {
    let store = Store::open_in_memory(&FixedClock).unwrap();
    let generation_id = generation(&store);
    for (ch, implementation, priority) in [('1', Some("rootless"), 0), ('2', None, 1)] {
        store
            .insert_descriptor_revision(revision(ch, "1.0.0", implementation))
            .unwrap();
        if ch == '1' {
            store
                .add_generation_membership(GenerationMemberDraft {
                    generation_id,
                    descriptor_digest: digest(ch),
                    candidate_priority: priority,
                })
                .unwrap();
        } else {
            assert!(
                store
                    .add_generation_membership(GenerationMemberDraft {
                        generation_id,
                        descriptor_digest: digest(ch),
                        candidate_priority: priority,
                    })
                    .is_err()
            );
        }
    }
}

#[test]
fn overlay_absence_means_enabled_without_experimental_opt_in() {
    let store = Store::open_in_memory(&FixedClock).unwrap();
    let capability_id = CapabilityId::new("calendar.events.read").unwrap();
    let overlay = store.get_capability_overlay(&capability_id).unwrap();
    assert_eq!(overlay.state(), CapabilityOverlayState::Enabled);
    assert!(!overlay.experimental_opt_in());
    assert_eq!(overlay.revision(), 0);
}

#[test]
fn step_binding_persists_exact_revision_and_retries_reuse_it() {
    let temp = TempDb::new();
    let (task_id, step_id, digest_value);
    {
        let store = Store::open(&temp.0, &FixedClock).unwrap();
        let (generation_id, digests) =
            activated_candidate_store(&store, &[("one", '1'), ("two", '2')]);
        task_id = TaskId::new("tsk_00000000000000000000000001").unwrap();
        step_id = StepId::new("stp_00000000000000000000000001").unwrap();
        digest_value = digests[0].clone();
        task_and_step(
            &store,
            task_id.as_str(),
            step_id.as_str(),
            Some(generation_id),
        );
        let first = store
            .bind_step_capability(&task_id, &step_id, &digest_value)
            .unwrap();
        assert_eq!(first.implementation_id().unwrap().as_str(), "one");
        assert_eq!(
            store
                .bind_step_capability(&task_id, &step_id, &digest_value)
                .unwrap(),
            first
        );
        assert_eq!(
            store.bind_step_capability(&task_id, &step_id, &digests[1]),
            Err(StoreError::RegistryBindingConflict)
        );
        assert!(
            store
                .conn
                .lock()
                .unwrap()
                .execute(
                    "UPDATE tasks SET capability_registry_generation=NULL WHERE task_id=?1",
                    [task_id.as_str()],
                )
                .is_err()
        );
        assert!(store.conn.lock().unwrap().execute(
            "UPDATE step_capability_bindings SET descriptor_digest=?1 WHERE task_id=?2 AND step_id=?3",
            rusqlite::params![digests[1].as_str(), task_id.as_str(), step_id.as_str()],
        ).is_err());
        assert!(store.conn.lock().unwrap().execute(
            "UPDATE task_steps SET task_id='tsk_00000000000000000000000003' WHERE step_id=?1",
            [step_id.as_str()],
        ).is_err());
        assert!(
            store
                .conn
                .lock()
                .unwrap()
                .execute(
                    "UPDATE task_steps SET kind='MODEL_TURN' WHERE step_id=?1",
                    [step_id.as_str()],
                )
                .is_err()
        );
        assert!(
            store
                .conn
                .lock()
                .unwrap()
                .execute(
                    "DELETE FROM capability_registry_generations WHERE generation_id=?1",
                    [first.generation_id()],
                )
                .is_err()
        );
        store.verify_integrity().unwrap();
    }
    let reopened = Store::open(&temp.0, &FixedClock).unwrap();
    let binding = reopened
        .get_step_capability_binding(&task_id, &step_id)
        .unwrap()
        .unwrap();
    assert_eq!(binding.descriptor_digest(), &digest_value);
    assert_eq!(
        reopened.get_task_registry_generation(&task_id).unwrap(),
        Some(binding.generation_id())
    );
    assert_eq!(binding.implementation_id().unwrap().as_str(), "one");
    assert_eq!(binding.provider_id().as_str(), "calendar");
    assert_eq!(binding.capability_version().as_str(), "1.0.0");
}

#[test]
fn binding_refuses_unpinned_tasks_unmembered_revisions_and_wrong_task() {
    let store = Store::open_in_memory(&FixedClock).unwrap();
    let (generation_id, digests) = activated_candidate_store(&store, &[("one", '1')]);
    let task_id = TaskId::new("tsk_00000000000000000000000002").unwrap();
    let step_id = StepId::new("stp_00000000000000000000000002").unwrap();
    task_and_step(&store, task_id.as_str(), step_id.as_str(), None);
    assert_eq!(store.get_task_registry_generation(&task_id).unwrap(), None);
    assert_eq!(
        store.bind_step_capability(&task_id, &step_id, &digests[0]),
        Err(StoreError::RegistryTaskUnpinned)
    );
    store
        .pin_task_registry_generation(&task_id, generation_id)
        .unwrap();
    assert_eq!(
        store.pin_task_registry_generation(&task_id, generation_id),
        Err(StoreError::RegistryTaskAlreadyPinned)
    );

    // Revision exists, but membership is in another generation.
    let other_generation = generation(&store);
    store
        .insert_descriptor_revision(revision('3', "1.0.0", Some("other")))
        .unwrap();
    store
        .add_generation_membership(GenerationMemberDraft {
            generation_id: other_generation,
            descriptor_digest: digest('3'),
            candidate_priority: 0,
        })
        .unwrap();
    assert_eq!(
        store.bind_step_capability(&task_id, &step_id, &digest('3')),
        Err(StoreError::RegistryBindingRefused)
    );

    let other_task = TaskId::new("tsk_00000000000000000000000003").unwrap();
    task_and_step(
        &store,
        other_task.as_str(),
        "stp_00000000000000000000000003",
        Some(generation_id),
    );
    assert_eq!(
        store.bind_step_capability(&other_task, &step_id, &digests[0]),
        Err(StoreError::RegistryBindingRefused)
    );
}

#[test]
fn overlay_blocks_new_binding_but_does_not_mutate_revision_or_generation() {
    let store = Store::open_in_memory(&FixedClock).unwrap();
    let (generation_id, digests) = activated_candidate_store(&store, &[("one", '1')]);
    let task_id = TaskId::new("tsk_00000000000000000000000004").unwrap();
    let step_id = StepId::new("stp_00000000000000000000000004").unwrap();
    task_and_step(
        &store,
        task_id.as_str(),
        step_id.as_str(),
        Some(generation_id),
    );
    store
        .transact(|tx| {
            tx.set_capability_overlay(
                CapabilityId::new("calendar.events.read").unwrap(),
                0,
                CapabilityOverlayState::Disabled,
                false,
            )
        })
        .unwrap();
    assert_eq!(
        store.bind_step_capability(&task_id, &step_id, &digests[0]),
        Err(StoreError::RegistryCapabilityUnavailable)
    );
    let generation_before = store.get_registry_generation(generation_id).unwrap();
    let revision_before = store.get_descriptor_revision(&digests[0]).unwrap();
    assert_eq!(
        store.list_generation_members(generation_id).unwrap().len(),
        1
    );
    assert_eq!(
        store.get_registry_generation(generation_id).unwrap(),
        generation_before
    );
    assert_eq!(
        store.get_descriptor_revision(&digests[0]).unwrap(),
        revision_before
    );
}

#[test]
fn experimental_descriptor_cannot_bind_until_explicit_opt_in() {
    let store = Store::open_in_memory(&FixedClock).unwrap();
    let mut draft = serea_protocol::CapabilityDescriptorDraft::from(descriptor("1.0.0", None));
    draft.experimental = true;
    let experimental_descriptor = CapabilityDescriptor::new(draft).unwrap();
    let generation_id = generation(&store);
    store
        .insert_descriptor_revision(DescriptorRevisionDraft {
            descriptor_digest: digest('8'),
            descriptor: experimental_descriptor,
            input_schema_digest: digest('a'),
            output_schema_digest: digest('b'),
        })
        .unwrap();
    store
        .add_generation_membership(GenerationMemberDraft {
            generation_id,
            descriptor_digest: digest('8'),
            candidate_priority: 0,
        })
        .unwrap();
    store
        .set_generation_default_version(
            generation_id,
            CapabilityId::new("calendar.events.read").unwrap(),
            SemVer::new("1.0.0").unwrap(),
        )
        .unwrap();
    store
        .transact(|tx| {
            tx.activate_registry_generation(
                generation_id,
                EpochMillis::new(1_796_000_000_000).unwrap(),
            )
        })
        .unwrap();
    let task_id = TaskId::new("tsk_00000000000000000000000008").unwrap();
    let step_id = StepId::new("stp_00000000000000000000000008").unwrap();
    task_and_step(
        &store,
        task_id.as_str(),
        step_id.as_str(),
        Some(generation_id),
    );
    assert_eq!(
        store.bind_step_capability(&task_id, &step_id, &digest('8')),
        Err(StoreError::RegistryCapabilityUnavailable),
    );
    store
        .transact(|tx| {
            tx.set_capability_overlay(
                CapabilityId::new("calendar.events.read").unwrap(),
                0,
                CapabilityOverlayState::Enabled,
                true,
            )
        })
        .unwrap();
    assert!(
        store
            .bind_step_capability(&task_id, &step_id, &digest('8'))
            .is_ok()
    );
}
