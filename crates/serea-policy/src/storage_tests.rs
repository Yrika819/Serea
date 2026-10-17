use std::str::FromStr;

use serea_event_bus::EventBus;
use serea_protocol::{
    ActorId, Authorization, CapabilityId, EpochMillis, ReasonCode, RiskClass, SideEffectClass,
};
use serea_storage::{PolicyActorClass, Store};
use serea_testkit::{DeterministicUlidSource, TestClock};

use crate::{
    AutomationContext, PolicyDecision, PolicyRevision, PolicyRule, PreparedPolicyRevision,
    RuleDecision, RuleMatch, activate_revision, evaluate, load_active_revision, prepare_revision,
};

fn rule() -> PolicyRule {
    PolicyRule::new(
        "allow_mail_read",
        10,
        true,
        RuleMatch {
            capability_id: Some(CapabilityId::from_str("mail.messages.list").unwrap()),
            risk_class: Some(RiskClass::Observe),
            side_effect_class: Some(SideEffectClass::None),
            automation_context: Some(AutomationContext::Interactive),
            ..RuleMatch::default()
        },
        RuleDecision::Allow,
    )
    .unwrap()
}

fn event_bus() -> EventBus {
    EventBus::new(DeterministicUlidSource::new())
}

fn prepare(store: &Store) -> PreparedPolicyRevision {
    prepare_revision(
        store,
        vec![rule()],
        EpochMillis::new(1_000).unwrap(),
        PolicyActorClass::Host,
        ActorId::new("serea-core").unwrap(),
        ReasonCode::new("POLICY_IMPORT").unwrap(),
    )
    .unwrap()
}

fn sample_input(revision: &PolicyRevision) -> crate::PolicyInputV1 {
    use serea_protocol::{
        DataClass, Digest, ImplementationId, ProviderId, RequestedBy, RootRequirement, SemVer,
        StepId, TaskId, TaskKind, TaskState,
    };
    crate::PolicyInputV1::new(
        crate::PolicyInputDraft {
            task_id: TaskId::from_str("tsk_01JQ8ZB8H2XKM9P4QW7NRT5YCD").unwrap(),
            step_id: StepId::from_str("stp_01JQ8ZB8H2XKM9P4QW7NRT5YCE").unwrap(),
            task_kind: TaskKind::UserRequest,
            task_state: TaskState::Executing,
            task_cancelled: false,
            task_deadline_at: Some(EpochMillis::new(1_800_000).unwrap()),
            task_data_class: DataClass::Personal,
            task_policy_class: RiskClass::Observe,
            capability_id: CapabilityId::from_str("mail.messages.list").unwrap(),
            capability_version: SemVer::new("1.0.0").unwrap(),
            provider_id: ProviderId::new("mail").unwrap(),
            implementation_id: Some(ImplementationId::new("rootless").unwrap()),
            risk_class: RiskClass::Observe,
            side_effect_class: SideEffectClass::None,
            authorization: Authorization::None,
            root_requirement: RootRequirement::NotRequired,
            data_class: DataClass::Personal,
            requested_by: RequestedBy::Model,
            device_state: None,
            capability_enabled: true,
            destructive_enabled: false,
            plan_revision: 4,
            registry_generation: 9,
            generation_digest: Digest::new(format!("sha256:{}", "c".repeat(64))).unwrap(),
            descriptor_digest: Digest::new(format!("sha256:{}", "a".repeat(64))).unwrap(),
            arguments_digest: Digest::new(format!("sha256:{}", "b".repeat(64))).unwrap(),
        },
        revision,
    )
    .unwrap()
}

#[test]
fn prepared_revision_activates_with_audited_event_and_evaluates_from_sqlite_snapshot() {
    let store = Store::open_in_memory(&TestClock::default()).unwrap();
    let bus = event_bus();
    let prepared = prepare(&store);
    assert!(store.current_policy_revision().unwrap().is_none());
    let activated = activate_revision(
        &store,
        &bus,
        &prepared,
        EpochMillis::new(2_000).unwrap(),
        serea_protocol::SemVer::new("0.1.0").unwrap(),
    )
    .unwrap();
    assert_eq!(activated.revision_id(), prepared.revision_id());
    assert_eq!(activated.rules_digest(), prepared.rules_digest());
    assert_eq!(
        evaluate(&sample_input(&activated), &activated),
        PolicyDecision::Allow
    );
    assert_eq!(load_active_revision(&store).unwrap().unwrap(), activated);

    let facts = sample_input(&activated).facts().clone();
    let transaction_evaluation = store
        .transact(|tx| {
            let snapshot = tx
                .current_policy_revision()?
                .ok_or(serea_storage::StoreError::InvalidPolicyRevision)?;
            crate::evaluate_storage_snapshot(facts, &snapshot)
                .map_err(|_| serea_storage::StoreError::InvalidPolicyRevision)
        })
        .unwrap();
    assert_eq!(
        transaction_evaluation,
        crate::evaluate_with_evidence(&sample_input(&activated), &activated)
    );

    let snapshot = store.current_policy_revision().unwrap().unwrap();
    let mut tampered = snapshot.clone();
    tampered.rules_digest =
        serea_protocol::Digest::new(format!("sha256:{}", "0".repeat(64))).unwrap();
    assert!(
        crate::evaluate_storage_snapshot(sample_input(&activated).facts().clone(), &tampered)
            .is_err()
    );
}

#[test]
fn active_policy_change_waits_for_transaction_scoped_p6_evaluation() {
    use std::sync::mpsc;
    use std::time::Duration;

    let path = std::env::temp_dir().join(format!(
        "serea-policy-transaction-{}-{}.sqlite",
        std::process::id(),
        EpochMillis::new(2_000).unwrap().get()
    ));
    let _ = std::fs::remove_file(&path);
    let bus = event_bus();
    let setup = Store::open(&path, &TestClock::default()).unwrap();
    let allow = prepare(&setup);
    let active = activate_revision(
        &setup,
        &bus,
        &allow,
        EpochMillis::new(2_000).unwrap(),
        serea_protocol::SemVer::new("0.1.0").unwrap(),
    )
    .unwrap();
    let facts = sample_input(&active).facts().clone();
    let deny = PolicyRule::new(
        "deny_mail_read",
        100,
        true,
        RuleMatch {
            capability_id: Some(CapabilityId::from_str("mail.messages.list").unwrap()),
            risk_class: Some(RiskClass::Observe),
            ..RuleMatch::default()
        },
        RuleDecision::Deny(crate::DenyReason::PolicyRuleDenied),
    )
    .unwrap();
    let pending_deny = prepare_revision(
        &setup,
        vec![deny],
        EpochMillis::new(3_000).unwrap(),
        PolicyActorClass::Host,
        ActorId::new("serea-core").unwrap(),
        ReasonCode::new("POLICY_IMPORT").unwrap(),
    )
    .unwrap();
    let pending_deny_id = pending_deny.revision_id();
    drop(setup);

    let reader_store = Store::open(&path, &TestClock::default()).unwrap();
    let writer_store = Store::open(&path, &TestClock::default()).unwrap();
    let (held_tx, held_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let (writer_started_tx, writer_started_rx) = mpsc::channel();
    let (writer_done_tx, writer_done_rx) = mpsc::channel();
    let writer_bus = bus;
    let reader = std::thread::spawn(move || {
        reader_store
            .transact(|tx| {
                let snapshot = tx
                    .current_policy_revision()?
                    .ok_or(serea_storage::StoreError::InvalidPolicyRevision)?;
                let decision = crate::evaluate_storage_snapshot(facts, &snapshot)
                    .map_err(|_| serea_storage::StoreError::InvalidPolicyRevision)?
                    .decision;
                held_tx.send(decision).unwrap();
                release_rx.recv().unwrap();
                Ok(decision)
            })
            .unwrap()
    });
    assert_eq!(held_rx.recv().unwrap(), PolicyDecision::Allow);
    let writer = std::thread::spawn(move || {
        writer_started_tx.send(()).unwrap();
        let result = activate_revision(
            &writer_store,
            &writer_bus,
            &pending_deny,
            EpochMillis::new(4_000).unwrap(),
            serea_protocol::SemVer::new("0.1.0").unwrap(),
        );
        writer_done_tx.send(result.is_ok()).unwrap();
    });
    writer_started_rx.recv().unwrap();
    assert!(
        writer_done_rx
            .recv_timeout(Duration::from_millis(100))
            .is_err()
    );
    release_tx.send(()).unwrap();
    assert_eq!(reader.join().unwrap(), PolicyDecision::Allow);
    assert!(writer_done_rx.recv_timeout(Duration::from_secs(5)).unwrap());
    writer.join().unwrap();

    let reopened = Store::open(&path, &TestClock::default()).unwrap();
    let current = load_active_revision(&reopened).unwrap().unwrap();
    assert_eq!(current.revision_id(), pending_deny_id);
    assert_eq!(
        evaluate(&sample_input(&current), &current),
        PolicyDecision::Deny(crate::DenyReason::PolicyRuleDenied)
    );
    drop(reopened);
    let _ = std::fs::remove_file(&path);
    let _ = std::fs::remove_file(path.with_extension("sqlite-wal"));
    let _ = std::fs::remove_file(path.with_extension("sqlite-shm"));
}

#[test]
fn reopened_sqlite_policy_has_same_digest_and_decision() {
    let path = std::env::temp_dir().join(format!(
        "serea-policy-restart-{}-{}.sqlite",
        std::process::id(),
        EpochMillis::new(1_000).unwrap().get()
    ));
    let _ = std::fs::remove_file(&path);
    let bus = event_bus();
    let first = Store::open(&path, &TestClock::default()).unwrap();
    let prepared = prepare(&first);
    let activated = activate_revision(
        &first,
        &bus,
        &prepared,
        EpochMillis::new(2_000).unwrap(),
        serea_protocol::SemVer::new("0.1.0").unwrap(),
    )
    .unwrap();
    let expected = evaluate(&sample_input(&activated), &activated);
    let expected_digest = activated.rules_digest().clone();
    drop(first);

    let reopened = Store::open(&path, &TestClock::default()).unwrap();
    let loaded = load_active_revision(&reopened).unwrap().unwrap();
    assert_eq!(loaded.rules_digest(), &expected_digest);
    assert_eq!(evaluate(&sample_input(&loaded), &loaded), expected);
    drop(reopened);
    let _ = std::fs::remove_file(&path);
    let _ = std::fs::remove_file(path.with_extension("sqlite-wal"));
    let _ = std::fs::remove_file(path.with_extension("sqlite-shm"));
}

#[test]
fn prepared_revision_stays_inactive_and_snapshot_digest_mismatch_is_rejected() {
    let store = Store::open_in_memory(&TestClock::default()).unwrap();
    let prepared = prepare(&store);
    assert!(prepared.revision_id() > 0);
    assert!(
        PolicyRevision::from_snapshot(
            prepared.revision_id(),
            serea_protocol::Digest::new(format!("sha256:{}", "0".repeat(64))).unwrap(),
            vec![rule()],
        )
        .is_err()
    );
    assert!(store.current_policy_revision().unwrap().is_none());
    assert!(load_active_revision(&store).unwrap().is_none());
}
