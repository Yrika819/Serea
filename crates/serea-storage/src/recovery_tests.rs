use super::*;
use crate::audit::TestAudit;
use crate::{
    CapabilityPlanBindingDraft, DescriptorRevisionDraft, GenerationMemberDraft, JournalKind,
    JournalRecord, JournalRecords, PlanWrite, RegistryGenerationDraft, StepInput, StepOutcome,
    Store, TaskAuditParticipant,
};
use serea_protocol::*;

fn at(n: i64) -> EpochMillis {
    EpochMillis::new(n).unwrap()
}
struct Fixed;
impl Clock for Fixed {
    fn now_ms(&self) -> Result<EpochMillis, ProtocolError> {
        Ok(at(0))
    }
}
fn tid() -> TaskId {
    TaskId::new("tsk_00000000000000000000000001").unwrap()
}
fn sid() -> StepId {
    StepId::new("stp_00000000000000000000000001").unwrap()
}
fn context(body: impl FnOnce(&TransitionContext<'_>)) {
    let actor = ActorId::new("recovery-test").unwrap();
    let version = SemVer::new("0.2.0").unwrap();
    body(&TransitionContext {
        actor_kind: ActorKind::Host,
        actor_id: &actor,
        actor_version: &version,
        causation_id: None,
    });
}
fn task(ceiling: u32) -> AssistantTask {
    AssistantTask {
        task_id: tid(),
        kind: TaskKind::UserRequest,
        title: TaskTitle::new("private title sentinel").unwrap(),
        state: TaskState::Received,
        origin: TaskOrigin {
            kind: TaskOriginKind::new("USER_MESSAGE").unwrap(),
            device_id: None,
            message_id: None,
            extensions: Default::default(),
        },
        data_class: DataClass::Personal,
        policy_class: RiskClass::Communication,
        created_at: Timestamp::from_epoch_millis(at(10)),
        updated_at: Timestamp::from_epoch_millis(at(10)),
        deadline_at: None,
        blocked_reason: None,
        result_summary: None,
        cancelled_at: None,
        cancelled_by: None,
        failure_reason: None,
        attempt_budget: AttemptBudget {
            max_model_calls: 1,
            max_tool_calls: 1,
            max_attempts_per_step: ceiling,
            extensions: Default::default(),
        },
        steps: vec![],
        extensions: Default::default(),
    }
}
fn input() -> StepInput {
    let raw = b"{\"input\":true}";
    let cap = CapabilityId::new("calendar.events.create").unwrap();
    let version = SemVer::new("1.0.0").unwrap();
    StepInput {
        input_json: raw.to_vec(),
        step: TaskStep::new(TaskStepDraft {
            step_id: sid(),
            task_id: tid(),
            sequence: 1,
            kind: StepKind::Capability,
            status: StepStatus::new("PLANNED").unwrap(),
            attempt: 0,
            idempotency_key: Some(
                derive_idempotency_key(
                    &tid(),
                    &sid(),
                    &cap,
                    &version,
                    std::str::from_utf8(raw).unwrap(),
                )
                .unwrap(),
            ),
            provider_id: Some(ProviderId::new("calendar").unwrap()),
            capability_id: Some(cap),
            capability_version: Some(version),
            input_digest: digest_of(std::str::from_utf8(raw).unwrap()).unwrap(),
            result_digest: None,
            side_effect_receipt: None,
            started_at: None,
            completed_at: None,
            lease_owner: None,
            lease_expires_at: None,
            lease_generation: None,
            error: None,
            extensions: Default::default(),
        })
        .unwrap(),
    }
}
fn fixture(ceiling: u32, c: &TransitionContext<'_>) -> Store {
    fixture_steps(ceiling, vec![input()], c)
}
fn fixture_steps(ceiling: u32, steps: Vec<StepInput>, c: &TransitionContext<'_>) -> Store {
    let store = Store::open_in_memory(&Fixed).unwrap();
    let descriptor = CapabilityDescriptor::new(CapabilityDescriptorDraft {
        id: CapabilityId::new("calendar.events.create").unwrap(),
        version: SemVer::new("1.0.0").unwrap(),
        title: DescriptorTitle::new("Recovery fixture").unwrap(),
        description: DescriptorDescription::new("Recovery fixture descriptor").unwrap(),
        provider_id: ProviderId::new("calendar").unwrap(),
        implementation_id: None,
        input_schema: JsonSchemaRef::new("serea://recovery/input").unwrap(),
        output_schema: JsonSchemaRef::new("serea://recovery/output").unwrap(),
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
    .unwrap();
    let generation_id = store
        .create_registry_generation(RegistryGenerationDraft {
            manifest_digest: digest_of("\"recovery-manifest\"").unwrap(),
            schema_catalog_digest: digest_of("\"recovery-catalog\"").unwrap(),
        })
        .unwrap()
        .generation_id();
    let descriptor_digest = digest_of("\"recovery-descriptor\"").unwrap();
    let schema_digest = digest_of("\"recovery-schema\"").unwrap();
    store
        .insert_descriptor_revision(DescriptorRevisionDraft {
            descriptor_digest: descriptor_digest.clone(),
            descriptor,
            input_schema_digest: schema_digest.clone(),
            output_schema_digest: schema_digest,
        })
        .unwrap();
    store
        .add_generation_membership(GenerationMemberDraft {
            generation_id,
            descriptor_digest: descriptor_digest.clone(),
            candidate_priority: 0,
        })
        .unwrap();
    store
        .set_generation_default_version(
            generation_id,
            CapabilityId::new("calendar.events.create").unwrap(),
            SemVer::new("1.0.0").unwrap(),
        )
        .unwrap();
    store
        .transact(|tx| {
            tx.activate_registry_generation(generation_id, at(1))
                .map(|_| ())
        })
        .unwrap();
    let bindings = steps
        .iter()
        .filter(|step| step.step.kind == StepKind::Capability)
        .map(|step| CapabilityPlanBindingDraft {
            step_id: step.step.step_id.clone(),
            descriptor_digest: descriptor_digest.clone(),
        })
        .collect::<Vec<_>>();
    store
        .transact_with_audit(&TestAudit, |tx| {
            tx.insert_task(&task(ceiling), c)?;
            tx.pin_task_registry_generation(&tid(), generation_id)?;
            tx.start_planning(&tid(), TaskState::Received, 0, at(20), c)?;
            tx.put_capability_plan_revision(
                &tid(),
                PlanWrite { revision: 1, steps },
                &bindings,
                at(30),
                c,
            )?;
            Ok(())
        })
        .unwrap();
    store
}
fn acquire(store: &Store, c: &TransitionContext<'_>) -> crate::LeaseGuard {
    store
        .transact_with_audit(&TestAudit, |tx| {
            tx.acquire_audited(
                tid(),
                sid(),
                LeaseOwner::new("worker").unwrap(),
                None,
                at(40),
                at(50),
                c,
            )
        })
        .unwrap()
}
fn dump(store: &Store) -> Vec<Vec<Vec<Value>>> {
    let conn = store.conn.lock().unwrap();
    [
        "tasks",
        "task_steps",
        "leases",
        "side_effect_receipts",
        "plan_revisions",
        "task_blob_refs",
        "step_blob_refs",
        "task_journal",
        "blobs",
        "schema_migrations",
    ]
    .iter()
    .map(|table| {
        let mut stmt = conn
            .prepare(&format!("SELECT * FROM {table} ORDER BY 1,2"))
            .unwrap();
        let n = stmt.column_count();
        stmt.query_map([], |r| {
            (0..n)
                .map(|i| r.get::<_, Value>(i))
                .collect::<Result<Vec<_>, _>>()
        })
        .unwrap()
        .map(Result::unwrap)
        .collect()
    })
    .collect()
}
struct LateReject;
impl TaskAuditParticipant for LateReject {
    fn records(&self, facts: &DurableTransition) -> Result<JournalRecords, StoreError> {
        if facts.operation() == AuditOperation::RecoveryDecision {
            Err(StoreError::AuditRejected)
        } else {
            TestAudit.records(facts)
        }
    }
}
struct Reject;
impl TaskAuditParticipant for Reject {
    fn records(&self, _: &DurableTransition) -> Result<JournalRecords, StoreError> {
        Err(StoreError::AuditRejected)
    }
}

#[test]
fn remediation_f1_ordinary_rollback_guard_never_regains_authority() {
    context(|c| {
        let store = fixture(3, c);
        let before = dump(&store);
        let mut escaped = None;
        let result: Result<(), StoreError> = store.transact_with_audit(&TestAudit, |tx| {
            escaped = Some(tx.acquire_audited(
                tid(),
                sid(),
                LeaseOwner::new("worker").unwrap(),
                None,
                at(40),
                at(50),
                c,
            )?);
            Err(StoreError::AuditRejected)
        });
        assert_eq!(result, Err(StoreError::AuditRejected));
        assert_eq!(dump(&store), before);
        let old = escaped.unwrap();
        let fresh = acquire(&store, c);
        assert_eq!(old.generation(), 1);
        assert_eq!(fresh.generation(), 1);
        let before = dump(&store);
        let result = store.transact_with_audit(&TestAudit, |tx| tx.begin_attempt(&old, at(41), c));
        assert_eq!(result, Err(StoreError::LeaseFenced));
        assert_eq!(dump(&store), before);
        store
            .transact_with_audit(&TestAudit, |tx| tx.begin_attempt(&fresh, at(41), c))
            .unwrap();
    });
}

#[test]
fn remediation_f2_recovery_block_supersedes_old_receipt_outcome() {
    context(|c| {
        let store = fixture(3, c);
        committed(&store, c);
        store.conn.lock().unwrap().execute_batch("PRAGMA ignore_check_constraints=ON; UPDATE task_steps SET status='FUTURE_STATUS'; PRAGMA ignore_check_constraints=OFF").unwrap();
        store
            .transact_with_audit(&TestAudit, |tx| {
                let s = tx.inspect_recovery_task(&tid())?;
                assert!(
                    tx.apply_recovery(
                        &s,
                        RecoveryAction::Quarantine {
                            reason: reason("UNRECOGNISED_STATE")
                        },
                        at(50),
                        c
                    )?
                    .blocked
                );
                Ok(())
            })
            .unwrap();
        assert!(outcome_kinds(&store, 50).contains(&"TASK_STATE_CHANGED".into()));
        store.conn.lock().unwrap().execute_batch("UPDATE task_steps SET status='SUCCEEDED'; UPDATE tasks SET state='EXECUTING',blocked_reason=NULL").unwrap();
        let before = dump(&store);
        store
            .transact(|tx| {
                let s = tx.inspect_recovery_task(&tid())?;
                assert_eq!(s.corruption(), Some(&reason("INVARIANT_VIOLATION")));
                assert!(s.receipt_repair().is_none());
                Ok(())
            })
            .unwrap();
        assert_eq!(dump(&store), before);
    });
}

fn contradictory_receipt_time(sql: &str) {
    context(|c| {
        let store = fixture(3, c);
        committed(&store, c);
        store
            .conn
            .lock()
            .unwrap()
            .execute_batch(&format!("UPDATE tasks SET state='EXECUTING'; {sql}"))
            .unwrap();
        let before = dump(&store);
        store
            .transact_with_audit(&TestAudit, |tx| {
                let s = tx.inspect_recovery_task(&tid())?;
                assert_eq!(s.corruption(), Some(&reason("INVARIANT_VIOLATION")));
                assert!(s.receipt_repair().is_none() && s.projection().is_none());
                assert!(matches!(
                    tx.apply_recovery(
                        &s,
                        RecoveryAction::ReceiptRepair { step_id: sid() },
                        at(1000),
                        c
                    ),
                    Err(StoreError::InvalidRecoveryAction)
                ));
                Ok(())
            })
            .unwrap();
        assert_eq!(dump(&store), before);
    });
}
#[test]
fn remediation_f3_completion_must_match_release_and_batch() {
    contradictory_receipt_time("UPDATE task_steps SET completed_at_ms=43");
}
#[test]
fn remediation_f3_receipt_observation_cannot_follow_batch() {
    contradictory_receipt_time("UPDATE side_effect_receipts SET observed_at_ms=900");
}

fn empty_execution(state: &str) {
    context(|c| {
        let store = Store::open_in_memory(&Fixed).unwrap();
        store
            .transact_with_audit(&TestAudit, |tx| tx.insert_task(&task(3), c))
            .unwrap();
        store
            .conn
            .lock()
            .unwrap()
            .execute("UPDATE tasks SET state=?1", [state])
            .unwrap();
        store
            .transact_with_audit(&TestAudit, |tx| {
                let s = tx.inspect_recovery_task(&tid())?;
                assert_eq!(s.corruption(), Some(&reason("INVARIANT_VIOLATION")));
                assert!(s.projection().is_none());
                assert!(matches!(
                    tx.apply_recovery(
                        &s,
                        RecoveryAction::ResumeNormally { next_step_id: None },
                        at(50),
                        c
                    ),
                    Err(StoreError::InvalidRecoveryAction)
                ));
                assert!(
                    tx.apply_recovery(
                        &s,
                        RecoveryAction::Quarantine {
                            reason: reason("INVARIANT_VIOLATION")
                        },
                        at(50),
                        c
                    )?
                    .blocked
                );
                Ok(())
            })
            .unwrap();
    });
}
#[test]
fn remediation_f4_empty_executing_is_invalid() {
    empty_execution("EXECUTING");
}
#[test]
fn remediation_f4_empty_verifying_is_invalid() {
    empty_execution("VERIFYING");
}

#[test]
fn remediation_f5_independent_blocked_reason_damage_is_journal_only() {
    context(|c| {
        for (state, code) in [
            ("READY", "RESIDUAL_REASON"),
            ("RECEIVED", "RESIDUAL_REASON"),
            ("BLOCKED", "invalid reason"),
        ] {
            let store = fixture(3, c);
            let healthy_id = TaskId::new("tsk_00000000000000000000000002").unwrap();
            let mut healthy = task(3);
            healthy.task_id = healthy_id.clone();
            store
                .transact_with_audit(&TestAudit, |tx| tx.insert_task(&healthy, c))
                .unwrap();
            store
                .conn
                .lock()
                .unwrap()
                .execute_batch("PRAGMA ignore_check_constraints=ON")
                .unwrap();
            store
                .conn
                .lock()
                .unwrap()
                .execute(
                    "UPDATE tasks SET state=?1,blocked_reason=?2 WHERE task_id=?3",
                    params![state, code, tid().as_str()],
                )
                .unwrap();
            store
                .conn
                .lock()
                .unwrap()
                .execute_batch("PRAGMA ignore_check_constraints=OFF")
                .unwrap();
            let raw_before = dump(&store)[0].clone();
            store
                .transact_with_audit(&TestAudit, |tx| {
                    tx.recovery_pass(|pass| {
                        let s = pass.inspect_recovery_task(&tid())?;
                        assert_eq!(s.corruption(), Some(&reason("INVARIANT_VIOLATION")));
                        let a = pass.apply_recovery(
                            &s,
                            RecoveryAction::Quarantine {
                                reason: reason("INVARIANT_VIOLATION"),
                            },
                            at(50),
                            c,
                        )?;
                        assert!(a.changed && a.revoked_steps.is_empty());
                        assert_eq!(a.state, super::state(state));
                        let healthy = pass.inspect_recovery_task(&healthy_id)?;
                        assert!(
                            pass.apply_recovery(
                                &healthy,
                                RecoveryAction::ResumeNormally { next_step_id: None },
                                at(50),
                                c
                            )?
                            .changed
                        );
                        Ok(())
                    })
                })
                .unwrap();
            assert_eq!(dump(&store)[0], raw_before);
            assert_eq!(
                store
                    .conn
                    .lock()
                    .unwrap()
                    .query_row("PRAGMA ignore_check_constraints", [], |r| r
                        .get::<_, i64>(0))
                    .unwrap(),
                0
            );
            assert!(
                store
                    .conn
                    .lock()
                    .unwrap()
                    .execute(
                        "UPDATE tasks SET state='PLANNING' WHERE task_id=?1",
                        [tid().as_str()]
                    )
                    .is_err()
            );
        }
    });
}

fn wrong_reconciliation_block(ceiling: u32, block: bool) {
    context(|c| {
        let store = fixture(ceiling, c);
        let g = acquire(&store, c);
        store
            .transact_with_audit(&TestAudit, |tx| tx.begin_attempt(&g, at(41), c))
            .unwrap();
        let before = dump(&store);
        store
            .transact_with_audit(&TestAudit, |tx| {
                let s = tx.inspect_recovery_task(&tid())?;
                assert!(s.corruption().is_none());
                assert!(matches!(
                    tx.apply_recovery(
                        &s,
                        RecoveryAction::NeedsReconciliation {
                            step_id: sid(),
                            block
                        },
                        at(60),
                        c
                    ),
                    Err(StoreError::InvalidRecoveryAction)
                ));
                Ok(())
            })
            .unwrap();
        assert_eq!(dump(&store), before);
    });
}
#[test]
fn remediation_f6_exhausted_work_requires_block() {
    wrong_reconciliation_block(1, false);
}
#[test]
fn remediation_f6_available_budget_forbids_block() {
    wrong_reconciliation_block(3, true);
}

#[test]
fn remediation_f6_absence_requires_conservative_nonblocking_disposition() {
    context(|c| {
        for ceiling in [1, 3] {
            let store = fixture(ceiling, c);
            let g = acquire(&store, c);
            store
                .transact_with_audit(&TestAudit, |tx| {
                    tx.begin_attempt(&g, at(41), c)?;
                    tx.release_audited(g, at(42), c)
                })
                .unwrap();
            store.conn.lock().unwrap().execute_batch("UPDATE task_steps SET status='RECONCILED_ABSENT',completed_at_ms=42,lease_owner=NULL,lease_expires_at_ms=NULL").unwrap();
            let before = dump(&store);
            store
                .transact_with_audit(&TestAudit, |tx| {
                    let s = tx.inspect_recovery_task(&tid())?;
                    assert!(s.corruption().is_none());
                    assert!(matches!(
                        tx.apply_recovery(
                            &s,
                            RecoveryAction::NeedsReconciliation {
                                step_id: sid(),
                                block: true
                            },
                            at(60),
                            c
                        ),
                        Err(StoreError::InvalidRecoveryAction)
                    ));
                    Ok(())
                })
                .unwrap();
            assert_eq!(dump(&store), before);
            store
                .transact_with_audit(&TestAudit, |tx| {
                    let s = tx.inspect_recovery_task(&tid())?;
                    let a = tx.apply_recovery(
                        &s,
                        RecoveryAction::NeedsReconciliation {
                            step_id: sid(),
                            block: false,
                        },
                        at(60),
                        c,
                    )?;
                    assert!(!a.blocked && a.revoked_steps.is_empty());
                    assert_eq!(a.state, Some(TaskState::Executing));
                    Ok(())
                })
                .unwrap();
            assert_eq!(dump(&store)[1], before[1]);
            assert!(!outcome_kinds(&store, 60).contains(&"STEP_RECONCILED_ABSENT".into()));
        }
    });
}

fn later_uncertain(status: &str, ceiling: u32) {
    context(|c| {
        let later_id = StepId::new("stp_00000000000000000000000002").unwrap();
        let mut later = input();
        let mut draft = TaskStepDraft::from(later.step);
        draft.step_id = later_id.clone();
        draft.sequence = 2;
        draft.idempotency_key = Some(
            derive_idempotency_key(
                &tid(),
                &later_id,
                draft.capability_id.as_ref().unwrap(),
                draft.capability_version.as_ref().unwrap(),
                std::str::from_utf8(&later.input_json).unwrap(),
            )
            .unwrap(),
        );
        if status == "WAITING" {
            draft.kind = StepKind::WaitUser;
            draft.provider_id = None;
            draft.capability_id = None;
            draft.capability_version = None;
            draft.idempotency_key = None;
        }
        later.step = TaskStep::new(draft).unwrap();
        let store = fixture_steps(ceiling, vec![input(), later], c);
        {
            let conn = store.conn.lock().unwrap();
            let active = matches!(status, "LEASED" | "EXECUTING");
            conn.execute("UPDATE task_steps SET status=?1,attempt=1,lease_generation=1,lease_owner=?2,lease_expires_at_ms=?3,started_at_ms=?4,completed_at_ms=?5,error_kind=?6,error_code=?7,error_message=?8,error_retryable=?9,error_host_action=?10 WHERE step_id=?11", params![status, active.then_some("worker"), active.then_some(50), (status != "LEASED").then_some(41), matches!(status,"FAILED"|"RECONCILED_ABSENT").then_some(42), (status=="FAILED").then_some("PROVIDER_ERROR"), (status=="FAILED").then_some("KNOWN_FAILURE"), (status=="FAILED").then_some("diagnostic"), (status=="FAILED").then_some(false), (status=="FAILED").then_some("STOP"), later_id.as_str()]).unwrap();
            conn.execute("INSERT INTO leases(step_id,owner,generation,acquired_at_ms,expires_at_ms,released_at_ms) VALUES(?1,'worker',1,40,50,42)", [later_id.as_str()]).unwrap();
        }
        let before = dump(&store);
        store
            .transact_with_audit(&TestAudit, |tx| {
                let s = tx.inspect_recovery_task(&tid())?;
                assert!(
                    s.corruption().is_none(),
                    "fixture must isolate action validation: {status}"
                );
                assert!(
                    matches!(
                        tx.apply_recovery(
                            &s,
                            RecoveryAction::ResumeNormally {
                                next_step_id: Some(sid())
                            },
                            at(60),
                            c
                        ),
                        Err(StoreError::InvalidRecoveryAction)
                    ),
                    "later {status} cannot be ignored"
                );
                Ok(())
            })
            .unwrap();
        assert_eq!(dump(&store), before);
    });
}
#[test]
fn remediation_f6_resume_cannot_ignore_later_executing() {
    later_uncertain("EXECUTING", 3);
}
#[test]
fn remediation_f6_resume_cannot_ignore_later_absent() {
    later_uncertain("RECONCILED_ABSENT", 3);
}
#[test]
fn remediation_f6_resume_cannot_ignore_later_exhausted_lease() {
    later_uncertain("LEASED", 1);
}
#[test]
fn remediation_f6_resume_cannot_ignore_later_failed() {
    later_uncertain("FAILED", 3);
}
#[test]
fn remediation_f6_resume_cannot_ignore_later_waiting() {
    later_uncertain("WAITING", 3);
}

fn incoherent_acquisition(sql: &str) {
    context(|c| {
        for executing in [false, true] {
            let store = fixture(3, c);
            let g = acquire(&store, c);
            if executing {
                store
                    .transact_with_audit(&TestAudit, |tx| tx.begin_attempt(&g, at(41), c))
                    .unwrap();
            }
            store.conn.lock().unwrap().execute_batch(sql).unwrap();
            let before = dump(&store);
            store
                .transact_with_audit(&TestAudit, |tx| {
                    let s = tx.inspect_recovery_task(&tid())?;
                    assert_eq!(s.corruption(), Some(&reason("INVARIANT_VIOLATION")));
                    assert!(s.projection().is_none() && s.steps()[0].authority().is_none());
                    let a = tx.apply_recovery(
                        &s,
                        RecoveryAction::Quarantine {
                            reason: reason("INVARIANT_VIOLATION"),
                        },
                        at(60),
                        c,
                    )?;
                    assert!(a.blocked && a.revoked_steps.is_empty());
                    Ok(())
                })
                .unwrap();
            let after = dump(&store);
            assert_eq!(after[1], before[1]);
            assert_eq!(after[2], before[2]);
        }
    });
}
#[test]
fn remediation_f7_attempt_must_equal_current_generation() {
    incoherent_acquisition("UPDATE task_steps SET attempt=2");
}
#[test]
fn remediation_f7_released_acquisition_counters_remain_authoritative() {
    context(|c| {
        for damage in [
            "UPDATE task_steps SET attempt=2",
            "UPDATE task_steps SET lease_generation=2; UPDATE leases SET generation=2",
        ] {
            let store = fixture(3, c);
            let g = acquire(&store, c);
            store
                .transact_with_audit(&TestAudit, |tx| tx.release_audited(g, at(42), c))
                .unwrap();
            store.conn.lock().unwrap().execute_batch(damage).unwrap();
            let before = dump(&store);
            store
                .transact_with_audit(&TestAudit, |tx| {
                    let s = tx.inspect_recovery_task(&tid())?;
                    assert_eq!(s.corruption(), Some(&reason("INVARIANT_VIOLATION")));
                    assert!(s.projection().is_none() && s.steps()[0].authority().is_none());
                    assert!(matches!(
                        tx.apply_recovery(
                            &s,
                            RecoveryAction::ResumeNormally {
                                next_step_id: Some(sid())
                            },
                            at(60),
                            c
                        ),
                        Err(StoreError::InvalidRecoveryAction)
                    ));
                    Ok(())
                })
                .unwrap();
            assert_eq!(dump(&store), before);
        }
    });
}

#[test]
fn remediation_f7_copy_expiry_must_follow_acquisition() {
    incoherent_acquisition("UPDATE task_steps SET lease_expires_at_ms=40");
}
#[test]
fn remediation_f7_copy_expiry_cannot_exceed_authority() {
    incoherent_acquisition("UPDATE task_steps SET lease_expires_at_ms=51");
}
#[test]
fn remediation_f7_ordinary_renewal_preserves_valid_copy_expiry() {
    context(|c| {
        let store = fixture(3, c);
        let g = acquire(&store, c);
        store
            .transact_with_audit(&TestAudit, |tx| {
                tx.begin_attempt(&g, at(41), c)?;
                tx.renew_lease(&g, at(45), at(80))?;
                let s = tx.inspect_recovery_task(&tid())?;
                assert!(s.corruption().is_none() && s.projection().is_some());
                assert_eq!(s.steps()[0].authority().unwrap().expires_at(), at(80));
                assert!(
                    tx.apply_recovery(&s, RecoveryAction::HeldLease { step_id: sid() }, at(60), c)?
                        .revoked_steps
                        .is_empty()
                );
                Ok(())
            })
            .unwrap();
    });
}

#[test]
fn raw_unknown_task_state_is_not_coerced_and_quarantine_is_stable() {
    context(|c| {
        let store = fixture(3, c);
        store.conn.lock().unwrap().execute_batch("PRAGMA ignore_check_constraints=ON; UPDATE tasks SET state='FUTURE_STATE'; PRAGMA ignore_check_constraints=OFF").unwrap();
        store
            .transact_with_audit(&TestAudit, |tx| {
                tx.recovery_pass(|tx| {
                    let snapshot = tx.inspect_recovery_task(&tid())?;
                    assert_eq!(snapshot.raw_state(), "FUTURE_STATE");
                    assert_eq!(snapshot.state(), None);
                    assert!(snapshot.projection().is_none());
                    assert_eq!(snapshot.corruption(), Some(&reason("UNRECOGNISED_STATE")));
                    let applied = tx.apply_recovery(
                        &snapshot,
                        RecoveryAction::Quarantine {
                            reason: reason("UNRECOGNISED_STATE"),
                        },
                        at(60),
                        c,
                    )?;
                    assert!(applied.changed && applied.blocked);
                    Ok(())
                })
            })
            .unwrap();
        let before = dump(&store);
        store
            .transact_with_audit(&TestAudit, |tx| {
                let snapshot = tx.inspect_recovery_task(&tid())?;
                assert_eq!(snapshot.state(), Some(TaskState::Blocked));
                assert!(
                    !tx.apply_recovery(&snapshot, RecoveryAction::BlockedTask, at(100), c)?
                        .changed
                );
                Ok(())
            })
            .unwrap();
        assert_eq!(before, dump(&store));
        let payload: String = store.conn.lock().unwrap().query_row(
            "SELECT payload_json FROM task_journal WHERE reason_code='UNRECOGNISED_STATE' AND journal_kind='RECOVERY_DECISION'", [], |r| r.get(0)).unwrap();
        let value: serde_json::Value = serde_json::from_str(&payload).unwrap();
        assert!(
            value
                .get("observed_fingerprint")
                .and_then(serde_json::Value::as_str)
                .is_some_and(|digest| Digest::new(digest).is_ok())
        );
        assert!(!payload.contains("FUTURE_STATE"));
        assert!(value.get("raw_state").is_none() && value.get("raw_status").is_none());
        assert!(!payload.contains("private title sentinel"));
    });
}

#[test]
fn raw_unknown_status_and_missing_authority_are_attributable_semantics() {
    context(|c| {
        let store = fixture(3, c);
        let g = acquire(&store, c);
        store
            .transact_with_audit(&TestAudit, |tx| tx.begin_attempt(&g, at(41), c))
            .unwrap();
        store
            .transact_with_audit(&TestAudit, |tx| tx.release_audited(g, at(42), c))
            .unwrap();
        store.conn.lock().unwrap().execute_batch("PRAGMA ignore_check_constraints=ON; UPDATE task_steps SET status='FUTURE_STATUS'; PRAGMA ignore_check_constraints=OFF").unwrap();
        store
            .transact_with_audit(&TestAudit, |tx| {
                let s = tx.inspect_recovery_task(&tid())?;
                assert_eq!(s.steps()[0].raw_status(), "FUTURE_STATUS");
                assert!(s.projection().is_none());
                assert_eq!(s.corruption(), Some(&reason("UNRECOGNISED_STATE")));
                tx.apply_recovery(
                    &s,
                    RecoveryAction::Quarantine {
                        reason: reason("UNRECOGNISED_STATE"),
                    },
                    at(60),
                    c,
                )?;
                Ok(())
            })
            .unwrap();
        let raw: String = store
            .conn
            .lock()
            .unwrap()
            .query_row("SELECT status FROM task_steps", [], |r| r.get(0))
            .unwrap();
        assert_eq!(raw, "FUTURE_STATUS");
        let before = dump(&store);
        store
            .transact_with_audit(&TestAudit, |tx| {
                let s = tx.inspect_recovery_task(&tid())?;
                assert!(
                    !tx.apply_recovery(
                        &s,
                        RecoveryAction::Quarantine {
                            reason: reason("UNRECOGNISED_STATE")
                        },
                        at(90),
                        c
                    )?
                    .changed
                );
                Ok(())
            })
            .unwrap();
        assert_eq!(dump(&store), before);
    });
}

#[test]
fn arbitrary_raw_state_and_status_remain_snapshot_only_with_bound_digest_evidence() {
    context(|c| {
        let raw = "private corrupt value sentinel\n\"not A_CODE\" ☃";
        for task_state in [true, false] {
            let store = fixture(3, c);
            {
                let conn = store.conn.lock().unwrap();
                conn.execute_batch("PRAGMA ignore_check_constraints=ON")
                    .unwrap();
                conn.execute(
                    if task_state {
                        "UPDATE tasks SET state=?1"
                    } else {
                        "UPDATE task_steps SET status=?1"
                    },
                    [raw],
                )
                .unwrap();
                conn.execute_batch("PRAGMA ignore_check_constraints=OFF")
                    .unwrap();
            }
            let observed = store
                .transact_with_audit(&TestAudit, |tx| {
                    let snapshot = tx.inspect_recovery_task(&tid())?;
                    assert_eq!(
                        if task_state {
                            snapshot.raw_state()
                        } else {
                            snapshot.steps()[0].raw_status()
                        },
                        raw
                    );
                    let observed = snapshot.fingerprint().clone();
                    let applied = tx.apply_recovery(
                        &snapshot,
                        RecoveryAction::Quarantine {
                            reason: reason("UNRECOGNISED_STATE"),
                        },
                        at(60),
                        c,
                    )?;
                    assert!(applied.blocked);
                    Ok(observed)
                })
                .unwrap();
            let payload: String = store.conn.lock().unwrap().query_row(
                "SELECT payload_json FROM task_journal WHERE journal_kind='RECOVERY_DECISION' AND reason_code='UNRECOGNISED_STATE'", [], |r| r.get(0)).unwrap();
            let value: serde_json::Value = serde_json::from_str(&payload).unwrap();
            assert_eq!(
                value["observed_fingerprint"].as_str(),
                Some(observed.as_str())
            );
            assert!(value.get("raw_state").is_none() && value.get("raw_status").is_none());
            let conn = store.conn.lock().unwrap();
            let rows = raw_rows(
                &conn,
                "SELECT state_from,state_to,payload_json FROM task_journal WHERE task_id=?1",
                &tid(),
            )
            .unwrap();
            for row in rows {
                for field in row {
                    if let Value::Text(text) = field {
                        assert!(!text.contains("private corrupt value sentinel"));
                    }
                }
            }
            if !task_state {
                let retained: String = conn
                    .query_row("SELECT status FROM task_steps", [], |r| r.get(0))
                    .unwrap();
                assert_eq!(retained, raw);
            }
        }
    });
}

#[test]
fn audit_sink_rejects_raw_recovery_members_and_known_outcome_release_drafts() {
    struct RawMember(&'static str);
    impl TaskAuditParticipant for RawMember {
        fn records(&self, facts: &DurableTransition) -> Result<JournalRecords, StoreError> {
            let mut rows = TestAudit.records(facts)?;
            if facts.operation() == AuditOperation::RecoveryDecision {
                let mut value: serde_json::Value =
                    serde_json::from_slice(&rows[0].payload_json).unwrap();
                value[self.0] = serde_json::json!("private corrupt value sentinel");
                rows[0].payload_json = serde_json::to_vec(&value).unwrap();
            }
            Ok(rows)
        }
    }
    struct OutcomeRelease;
    impl TaskAuditParticipant for OutcomeRelease {
        fn records(&self, facts: &DurableTransition) -> Result<JournalRecords, StoreError> {
            Ok(vec![JournalRecord {
                kind: JournalKind::StepLeaseReleased,
                state_from: facts.step_from().map(|s| s.as_str().into()),
                state_to: facts.step_to().map(|s| s.as_str().into()),
                reason: facts.reason().cloned(),
                payload_json: b"{}".to_vec(),
            }])
        }
    }
    context(|c| {
        for key in ["raw_state", "raw_status"] {
            let store = fixture(3, c);
            let before = dump(&store);
            store
                .transact_with_audit(&RawMember(key), |tx| {
                    let snapshot = tx.inspect_recovery_task(&tid())?;
                    assert_eq!(
                        tx.apply_recovery(
                            &snapshot,
                            RecoveryAction::ResumeNormally {
                                next_step_id: Some(sid())
                            },
                            at(50),
                            c
                        )
                        .unwrap_err(),
                        StoreError::AuditRejected
                    );
                    Ok(())
                })
                .unwrap();
            assert_eq!(dump(&store), before);
        }
        for operation in [AuditOperation::StepSucceeded, AuditOperation::StepFailed] {
            let store = fixture(3, c);
            let before = dump(&store);
            store
                .transact_with_audit(&OutcomeRelease, |tx| {
                    // Even a well-shaped outcome release draft is not permitted.
                    let mut facts = DurableTransition::task(
                        operation,
                        &tid(),
                        Some(TaskState::Executing),
                        TaskState::Verifying,
                        DataClass::Personal,
                        at(50),
                        c,
                    );
                    facts.step_id = Some(sid());
                    facts.step_from = Some(StepStatus::new("EXECUTING").unwrap());
                    facts.step_to = Some(
                        StepStatus::new(if operation == AuditOperation::StepSucceeded {
                            "SUCCEEDED"
                        } else {
                            "FAILED"
                        })
                        .unwrap(),
                    );
                    facts.attempt = Some(1);
                    facts.generation = Some(1);
                    assert_eq!(tx.record_transition(&facts), Err(StoreError::AuditRejected));
                    Ok(())
                })
                .unwrap();
            assert_eq!(dump(&store), before);
        }
    });
}

#[test]
fn savepoint_restores_revocation_reassessment_and_journal_when_audit_rejects() {
    context(|c| {
        let store = fixture(1, c);
        drop(acquire(&store, c));
        let before = dump(&store);
        store
            .transact_with_audit(&Reject, |tx| {
                let s = tx.inspect_recovery_task(&tid())?;
                assert_eq!(
                    tx.apply_recovery(
                        &s,
                        RecoveryAction::NeedsReconciliation {
                            step_id: sid(),
                            block: true
                        },
                        at(50),
                        c
                    )
                    .unwrap_err(),
                    StoreError::AuditRejected
                );
                Ok(()) // Caught error must not commit a partial revocation or edge.
            })
            .unwrap();
        assert_eq!(dump(&store), before);
        store
            .transact_with_audit(&LateReject, |tx| {
                let s = tx.inspect_recovery_task(&tid())?;
                assert_eq!(
                    tx.apply_recovery(
                        &s,
                        RecoveryAction::NeedsReconciliation {
                            step_id: sid(),
                            block: true
                        },
                        at(50),
                        c
                    )
                    .unwrap_err(),
                    StoreError::AuditRejected
                );
                Ok(()) // Both successful edge audits must also roll back.
            })
            .unwrap();
        assert_eq!(dump(&store), before);
    });
}

#[test]
fn whole_pass_rollback_restores_previously_successful_task_operation() {
    context(|c| {
        let store = fixture(3, c);
        let before = dump(&store);
        store
            .transact_with_audit(&TestAudit, |tx| {
                let result: Result<(), StoreError> = tx.recovery_pass(|tx| {
                    let s = tx.inspect_recovery_task(&tid())?;
                    assert!(
                        tx.apply_recovery(
                            &s,
                            RecoveryAction::ResumeNormally {
                                next_step_id: Some(sid())
                            },
                            at(50),
                            c
                        )?
                        .changed
                    );
                    Err(StoreError::InvalidRecoveryAction)
                });
                assert_eq!(result, Err(StoreError::InvalidRecoveryAction));
                Ok(())
            })
            .unwrap();
        assert_eq!(dump(&store), before);
    });
}

#[test]
fn preflight_rechecks_catalog_and_all_foreign_keys_without_global_check_coercion() {
    context(|c| {
        let store = fixture(3, c);
        {
            let conn = store.conn.lock().unwrap();
            conn.execute_batch("PRAGMA foreign_keys=OFF").unwrap();
            conn.execute("INSERT INTO task_blob_refs(task_id,role,digest,data_class_rank) VALUES (?1,'PLAN',?2,1)",
                params![tid().as_str(), digest_of("{}").unwrap().as_str()]).unwrap();
            conn.execute_batch("PRAGMA foreign_keys=ON").unwrap();
        }
        let before = dump(&store);
        assert_eq!(
            store.transact_with_audit(&TestAudit, |tx| tx.recovery_pass(|_| Ok(()))),
            Err(StoreError::IntegrityCheckFailed)
        );
        assert_eq!(dump(&store), before);
        store
            .conn
            .lock()
            .unwrap()
            .execute(
                "DELETE FROM task_blob_refs WHERE digest=?1",
                [digest_of("{}").unwrap().as_str()],
            )
            .unwrap();
        store
            .conn
            .lock()
            .unwrap()
            .execute(
                "UPDATE schema_migrations SET checksum=?1",
                [Migrations::checksum("incorrect").as_str()],
            )
            .unwrap();
        let before = dump(&store);
        assert_eq!(
            store.transact(|tx| tx.recovery_preflight()),
            Err(StoreError::MigrationChecksumMismatch)
        );
        assert_eq!(dump(&store), before);
    });
}

#[test]
fn conditional_reinspection_rejects_same_tx_renewal_and_raw_provenance_change() {
    context(|c| {
        let store = fixture(3, c);
        let g = acquire(&store, c);
        store
            .transact_with_audit(&TestAudit, |tx| {
                let s = tx.inspect_recovery_task(&tid())?;
                tx.renew_lease(&g, at(45), at(80))?;
                assert_eq!(
                    tx.apply_recovery(
                        &s,
                        RecoveryAction::ResumeNormally {
                            next_step_id: Some(sid())
                        },
                        at(50),
                        c
                    )
                    .unwrap_err(),
                    StoreError::RecoverySnapshotStale
                );
                let renewed = tx.inspect_recovery_task(&tid())?;
                assert_eq!(renewed.steps()[0].authority().unwrap().expires_at(), at(80));
                assert!(
                    tx.apply_recovery(
                        &renewed,
                        RecoveryAction::HeldLease { step_id: sid() },
                        at(50),
                        c
                    )?
                    .revoked_steps
                    .is_empty()
                );
                let held = tx.inspect_recovery_task(&tid())?;
                tx.inner
                    .execute("DELETE FROM step_blob_refs WHERE role='ARGUMENTS'", [])?;
                assert_eq!(
                    tx.apply_recovery(
                        &held,
                        RecoveryAction::HeldLease { step_id: sid() },
                        at(50),
                        c
                    )
                    .unwrap_err(),
                    StoreError::RecoverySnapshotStale
                );
                Ok(())
            })
            .unwrap();
    });
}

#[test]
fn expiry_is_exact_and_second_pass_preserves_all_durable_bytes() {
    context(|c| {
        let store = fixture(3, c);
        let g = acquire(&store, c);
        store
            .transact_with_audit(&TestAudit, |tx| {
                let s = tx.inspect_recovery_task(&tid())?;
                let a = tx.apply_recovery(
                    &s,
                    RecoveryAction::ResumeNormally {
                        next_step_id: Some(sid()),
                    },
                    at(50),
                    c,
                )?;
                assert_eq!(a.revoked_steps, vec![sid()]);
                assert!(a.changed);
                assert_eq!(
                    tx.begin_attempt(&g, at(50), c),
                    Err(StoreError::LeaseFenced)
                );
                Ok(())
            })
            .unwrap();
        let before = dump(&store);
        store
            .transact_with_audit(&TestAudit, |tx| {
                let s = tx.inspect_recovery_task(&tid())?;
                assert!(
                    !tx.apply_recovery(
                        &s,
                        RecoveryAction::ResumeNormally {
                            next_step_id: Some(sid())
                        },
                        at(900),
                        c
                    )?
                    .changed
                );
                Ok(())
            })
            .unwrap();
        assert_eq!(dump(&store), before);
    });
}

#[test]
fn exhausted_ready_uses_two_real_edges_and_stable_post_repair_identity() {
    context(|c| {
        let store = fixture(1, c);
        drop(acquire(&store, c));
        store
            .transact_with_audit(&TestAudit, |tx| {
                let s = tx.inspect_recovery_task(&tid())?;
                let a = tx.apply_recovery(
                    &s,
                    RecoveryAction::NeedsReconciliation {
                        step_id: sid(),
                        block: true,
                    },
                    at(50),
                    c,
                )?;
                assert!(a.blocked);
                assert_eq!(a.revoked_steps, vec![sid()]);
                Ok(())
            })
            .unwrap();
        let edges: Vec<(String,String,String)> = store.conn.lock().unwrap().prepare(
            "SELECT state_from,state_to,reason_code FROM task_journal WHERE payload_ref_digest=?1 AND journal_kind='TASK_STATE_CHANGED' ORDER BY journal_seq").unwrap()
            .query_map([recovery_edge_marker().as_str()], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?))).unwrap().map(Result::unwrap).collect();
        assert_eq!(
            edges,
            vec![
                ("READY".into(), "PLANNING".into(), "REPLAN".into()),
                (
                    "PLANNING".into(),
                    "BLOCKED".into(),
                    "NEEDS_RECONCILIATION".into()
                )
            ]
        );
        let before = dump(&store);
        store
            .transact_with_audit(&TestAudit, |tx| {
                let s = tx.inspect_recovery_task(&tid())?;
                assert!(
                    !tx.apply_recovery(
                        &s,
                        RecoveryAction::NeedsReconciliation {
                            step_id: sid(),
                            block: true
                        },
                        at(60),
                        c
                    )?
                    .changed
                );
                assert!(
                    !tx.apply_recovery(&s, RecoveryAction::BlockedTask, at(60), c)?
                        .changed
                );
                Ok(())
            })
            .unwrap();
        assert_eq!(dump(&store), before);
    });
}

#[test]
fn dangling_result_reference_is_semantic_damage_not_resume_eligibility() {
    context(|c| {
        let store = fixture(3, c);
        store.transact(|tx| {
            let blob = tx.put_blob(b"{}", DataClass::Personal)?;
            tx.inner.execute("INSERT INTO step_blob_refs(step_id,role,digest,data_class_rank) VALUES (?1,'RESULT',?2,1)",
                params![sid().as_str(), blob.digest().as_str()])?;
            Ok(())
        }).unwrap();
        store
            .transact_with_audit(&TestAudit, |tx| {
                let s = tx.inspect_recovery_task(&tid())?;
                assert_eq!(s.corruption(), Some(&reason("INVARIANT_VIOLATION")));
                assert!(s.projection().is_none());
                assert!(
                    tx.apply_recovery(
                        &s,
                        RecoveryAction::Quarantine {
                            reason: reason("INVARIANT_VIOLATION")
                        },
                        at(50),
                        c
                    )?
                    .blocked
                );
                let result_refs: i64 = tx.inner.query_row(
                    "SELECT count(*) FROM step_blob_refs WHERE role='RESULT'",
                    [],
                    |r| r.get(0),
                )?;
                assert_eq!(result_refs, 1); // Preserve raw damage; never fabricate an outcome.
                Ok(())
            })
            .unwrap();
    });
}

#[test]
fn task_local_check_damage_gets_invariant_evidence_without_disabling_constraints() {
    context(|c| {
        for damage in [
            "max_attempts_per_step=-1",
            "origin_extensions='malformed'",
            "updated_at_ms=-62167219200001",
        ] {
            let store = fixture(3, c);
            store.conn.lock().unwrap().execute_batch(&format!(
                "PRAGMA ignore_check_constraints=ON; UPDATE tasks SET {damage}; PRAGMA ignore_check_constraints=OFF")).unwrap();
            let raw_before = dump(&store)[0].clone();
            store
                .transact_with_audit(&TestAudit, |tx| {
                    tx.recovery_pass(|tx| {
                        let s = tx.inspect_recovery_task(&tid())?;
                        assert_eq!(s.corruption(), Some(&reason("INVARIANT_VIOLATION")));
                        let a = tx.apply_recovery(
                            &s,
                            RecoveryAction::Quarantine {
                                reason: reason("INVARIANT_VIOLATION"),
                            },
                            at(50),
                            c,
                        )?;
                        assert!(a.changed && !a.blocked);
                        assert_eq!(a.state, Some(TaskState::Ready));
                        Ok(())
                    })
                })
                .unwrap();
            assert_eq!(dump(&store)[0], raw_before);
            let checks: bool = store
                .conn
                .lock()
                .unwrap()
                .query_row("PRAGMA ignore_check_constraints", [], |r| r.get(0))
                .unwrap();
            assert!(!checks);
            let before = dump(&store);
            store
                .transact_with_audit(&TestAudit, |tx| {
                    let s = tx.inspect_recovery_task(&tid())?;
                    assert!(
                        !tx.apply_recovery(
                            &s,
                            RecoveryAction::Quarantine {
                                reason: reason("INVARIANT_VIOLATION")
                            },
                            at(90),
                            c
                        )?
                        .changed
                    );
                    Ok(())
                })
                .unwrap();
            assert_eq!(dump(&store), before);
        }
    });
}

#[test]
fn stale_outcome_without_receipt_is_unsupported_not_guessed_into_a_task_repair() {
    context(|c| {
        for aggregate in ["EXECUTING", "READY", "RECEIVED"] {
            let store = fixture(3, c);
            let g = acquire(&store, c);
            store
                .transact_with_audit(&TestAudit, |tx| {
                    tx.begin_attempt(&g, at(41), c)?;
                    tx.commit_step_outcome(
                        g,
                        StepOutcome::Succeeded {
                            result_json: b"{\"ok\":true}",
                            receipt: None,
                        },
                        at(42),
                        c,
                    )?;
                    Ok(())
                })
                .unwrap();
            store
                .conn
                .lock()
                .unwrap()
                .execute("UPDATE tasks SET state=?1", [aggregate])
                .unwrap();
            store
                .transact_with_audit(&TestAudit, |tx| {
                    let s = tx.inspect_recovery_task(&tid())?;
                    assert_eq!(s.corruption(), Some(&reason("INVARIANT_VIOLATION")));
                    assert!(s.receipt_repair().is_none() && s.projection().is_none());
                    assert!(
                        tx.apply_recovery(
                            &s,
                            RecoveryAction::Quarantine {
                                reason: reason("INVARIANT_VIOLATION")
                            },
                            at(50),
                            c
                        )?
                        .blocked
                    );
                    Ok(())
                })
                .unwrap();
        }
    });
}

fn outcome_kinds(store: &Store, now: i64) -> Vec<String> {
    store
        .conn
        .lock()
        .unwrap()
        .prepare(
            "SELECT journal_kind FROM task_journal WHERE occurred_at_ms=?1 ORDER BY journal_seq",
        )
        .unwrap()
        .query_map([now], |r| r.get(0))
        .unwrap()
        .map(Result::unwrap)
        .collect()
}

fn committed(store: &Store, c: &TransitionContext<'_>) {
    let g = acquire(store, c);
    store
        .transact_with_audit(&TestAudit, |tx| {
            tx.begin_attempt(&g, at(41), c)?;
            let step = tx.load_task(&tid())?.steps.remove(0).step;
            let receipt = SideEffectReceipt {
                receipt_id: ReceiptId::new("rcp_00000000000000000000000001").unwrap(),
                capability_id: step.capability_id.clone().unwrap(),
                idempotency_key: step.idempotency_key.clone().unwrap(),
                provider_reference: None,
                effect_summary: EffectSummary::new("private effect sentinel").unwrap(),
                observed_at: Timestamp::from_epoch_millis(at(42)),
                replay_safe: false,
            };
            tx.commit_step_outcome(
                g,
                StepOutcome::Succeeded {
                    result_json: b"{\"ok\":true}",
                    receipt: Some(&receipt),
                },
                at(42),
                c,
            )?;
            Ok(())
        })
        .unwrap();
}

#[test]
fn receipt_repair_requires_complete_outcome_evidence_and_never_rewrites_effect() {
    context(|c| {
        let store = fixture(3, c);
        committed(&store, c);
        assert_eq!(
            outcome_kinds(&store, 42),
            ["STEP_COMMITTED", "RECEIPT_RECORDED", "TASK_STATE_CHANGED"]
        );
        store
            .conn
            .lock()
            .unwrap()
            .execute("UPDATE tasks SET state='EXECUTING'", [])
            .unwrap();
        store
            .transact_with_audit(&TestAudit, |tx| {
                let s = tx.inspect_recovery_task(&tid())?;
                assert!(s.corruption().is_none());
                assert_eq!(
                    s.receipt_repair().unwrap().destination(),
                    TaskState::Verifying
                );
                let a = tx.apply_recovery(
                    &s,
                    RecoveryAction::ReceiptRepair { step_id: sid() },
                    at(50),
                    c,
                )?;
                assert!(a.receipt_repaired && a.changed);
                Ok(())
            })
            .unwrap();
        let before = dump(&store);
        store
            .transact_with_audit(&TestAudit, |tx| {
                let s = tx.inspect_recovery_task(&tid())?;
                assert!(s.receipt_repair().is_none());
                assert!(
                    !tx.apply_recovery(
                        &s,
                        RecoveryAction::ReceiptAlreadyCommitted { step_id: sid() },
                        at(60),
                        c
                    )?
                    .changed
                );
                Ok(())
            })
            .unwrap();
        assert_eq!(dump(&store), before);
        store.conn.lock().unwrap().execute_batch("UPDATE tasks SET state='EXECUTING'; UPDATE task_journal SET journal_kind='RECOVERY_DECISION' WHERE journal_kind='RECEIPT_RECORDED'").unwrap();
        store
            .transact(|tx| {
                let s = tx.inspect_recovery_task(&tid())?;
                assert_eq!(s.corruption(), Some(&reason("INVARIANT_VIOLATION")));
                assert!(s.receipt_repair().is_none());
                Ok(())
            })
            .unwrap();
    });
}

#[test]
fn receipt_repair_requires_authoritative_release_matching_the_normal_outcome_stamp() {
    context(|c| {
        for release in ["NULL", "43"] {
            let store = fixture(3, c);
            committed(&store, c);
            assert_eq!(
                outcome_kinds(&store, 42),
                ["STEP_COMMITTED", "RECEIPT_RECORDED", "TASK_STATE_CHANGED"]
            );
            store
                .conn
                .lock()
                .unwrap()
                .execute_batch(&format!(
                    "UPDATE tasks SET state='EXECUTING'; UPDATE leases SET released_at_ms={release}"
                ))
                .unwrap();
            let before = dump(&store);
            store
                .transact(|tx| {
                    let snapshot = tx.inspect_recovery_task(&tid())?;
                    assert_eq!(snapshot.corruption(), Some(&reason("INVARIANT_VIOLATION")));
                    assert!(snapshot.receipt_repair().is_none());
                    Ok(())
                })
                .unwrap();
            assert_eq!(dump(&store), before);
        }
    });
}

#[test]
fn mismatched_prior_authority_is_not_reconstructed_and_never_revoked() {
    context(|c| {
        for sql in [
            "DELETE FROM leases",
            "UPDATE leases SET owner='other-worker'",
            "UPDATE leases SET generation=2",
        ] {
            let store = fixture(3, c);
            drop(acquire(&store, c));
            store.conn.lock().unwrap().execute_batch(sql).unwrap();
            store
                .transact_with_audit(&TestAudit, |tx| {
                    let s = tx.inspect_recovery_task(&tid())?;
                    assert_eq!(s.corruption(), Some(&reason("INVARIANT_VIOLATION")));
                    assert!(s.projection().is_none() && s.steps()[0].authority().is_none());
                    let a = tx.apply_recovery(
                        &s,
                        RecoveryAction::Quarantine {
                            reason: reason("INVARIANT_VIOLATION"),
                        },
                        at(50),
                        c,
                    )?;
                    assert!(a.blocked && a.revoked_steps.is_empty());
                    Ok(())
                })
                .unwrap();
        }
    });
}

#[test]
fn recovery_payload_prose_is_not_authority_and_context_does_not_change_identity() {
    context(|c| {
        let store = fixture(3, c);
        store
            .transact_with_audit(&TestAudit, |tx| {
                let s = tx.inspect_recovery_task(&tid())?;
                tx.apply_recovery(
                    &s,
                    RecoveryAction::ResumeNormally {
                        next_step_id: Some(sid()),
                    },
                    at(50),
                    c,
                )?;
                Ok(())
            })
            .unwrap();
        let fake = "{\"decision\":\"TerminalNoop\"}";
        store.conn.lock().unwrap().execute("UPDATE task_journal SET payload_json=?1,payload_digest=?2 WHERE journal_kind='RECOVERY_DECISION'",
            params![fake, digest_of(fake).unwrap().as_str()]).unwrap();
        let before = dump(&store);
        let actor = ActorId::new("other-host").unwrap();
        let version = SemVer::new("9.0.0").unwrap();
        let other = TransitionContext {
            actor_kind: ActorKind::Host,
            actor_id: &actor,
            actor_version: &version,
            causation_id: None,
        };
        store
            .transact_with_audit(&TestAudit, |tx| {
                let s = tx.inspect_recovery_task(&tid())?;
                assert_eq!(s.state(), Some(TaskState::Ready));
                assert!(s.corruption().is_none());
                assert!(
                    !tx.apply_recovery(
                        &s,
                        RecoveryAction::ResumeNormally {
                            next_step_id: Some(sid())
                        },
                        at(90),
                        &other
                    )?
                    .changed
                );
                Ok(())
            })
            .unwrap();
        assert_eq!(dump(&store), before);
    });
}

#[test]
fn unsupported_ordinary_classes_and_backward_now_fail_closed() {
    context(|c| {
        for sql in [
            "UPDATE tasks SET data_class_rank=2",
            "UPDATE plan_revisions SET data_class_rank=2",
            "UPDATE task_journal SET data_class_rank=2",
        ] {
            let store = fixture(3, c);
            {
                let conn = store.conn.lock().unwrap();
                conn.execute_batch("PRAGMA foreign_keys=OFF").unwrap();
                conn.execute_batch(sql).unwrap();
                conn.execute_batch("PRAGMA foreign_keys=ON").unwrap();
            }
            assert_eq!(
                store.transact(|tx| tx.inspect_recovery_task(&tid())).err(),
                Some(StoreError::AtRestProtectionUnavailable)
            );
        }
        let store = fixture(3, c);
        let before = dump(&store);
        store
            .transact_with_audit(&TestAudit, |tx| {
                let s = tx.inspect_recovery_task(&tid())?;
                assert_eq!(
                    tx.apply_recovery(
                        &s,
                        RecoveryAction::ResumeNormally {
                            next_step_id: Some(sid())
                        },
                        at(29),
                        c
                    )
                    .unwrap_err(),
                    StoreError::InvalidTimestamp
                );
                Ok(())
            })
            .unwrap();
        assert_eq!(dump(&store), before);
    });
}

#[test]
fn successful_verifier_receipt_can_repair_only_its_proven_completion_edge() {
    context(|c| {
        let store = fixture(3, c);
        let verifier_id = StepId::new("stp_00000000000000000000000002").unwrap();
        let mut verifier = input();
        let mut draft = TaskStepDraft::from(verifier.step);
        draft.step_id = verifier_id.clone();
        draft.sequence = 2;
        draft.kind = StepKind::Verify;
        draft.idempotency_key = Some(
            derive_idempotency_key(
                &tid(),
                &verifier_id,
                draft.capability_id.as_ref().unwrap(),
                draft.capability_version.as_ref().unwrap(),
                std::str::from_utf8(&verifier.input_json).unwrap(),
            )
            .unwrap(),
        );
        verifier.step = TaskStep::new(draft).unwrap();
        store
            .transact_with_audit(&TestAudit, |tx| {
                tx.start_planning(&tid(), TaskState::Ready, 1, at(31), c)?;
                tx.put_plan_revision(
                    &tid(),
                    PlanWrite {
                        revision: 2,
                        steps: vec![input(), verifier],
                    },
                    at(32),
                    c,
                )?;
                Ok(())
            })
            .unwrap();
        committed(&store, c);
        store
            .transact_with_audit(&TestAudit, |tx| {
                let g = tx.acquire_audited(
                    tid(),
                    verifier_id.clone(),
                    LeaseOwner::new("verifier").unwrap(),
                    None,
                    at(43),
                    at(60),
                    c,
                )?;
                tx.begin_attempt(&g, at(44), c)?;
                let s = tx.load_task(&tid())?.steps.remove(1).step;
                let receipt = SideEffectReceipt {
                    receipt_id: ReceiptId::new("rcp_00000000000000000000000002").unwrap(),
                    capability_id: s.capability_id.clone().unwrap(),
                    idempotency_key: s.idempotency_key.clone().unwrap(),
                    provider_reference: None,
                    effect_summary: EffectSummary::new("verified").unwrap(),
                    observed_at: Timestamp::from_epoch_millis(at(45)),
                    replay_safe: true,
                };
                tx.commit_step_outcome(
                    g,
                    StepOutcome::Succeeded {
                        result_json: b"{\"verified\":true}",
                        receipt: Some(&receipt),
                    },
                    at(45),
                    c,
                )?;
                Ok(())
            })
            .unwrap();
        assert_eq!(
            outcome_kinds(&store, 45),
            [
                "STEP_COMMITTED",
                "RECEIPT_RECORDED",
                "TASK_STATE_CHANGED",
                "TASK_TERMINAL"
            ]
        );
        store
            .conn
            .lock()
            .unwrap()
            .execute("UPDATE tasks SET state='VERIFYING'", [])
            .unwrap();
        store
            .transact_with_audit(&TestAudit, |tx| {
                let s = tx.inspect_recovery_task(&tid())?;
                assert!(s.corruption().is_none());
                assert_eq!(
                    s.receipt_repair().unwrap().destination(),
                    TaskState::Completed
                );
                assert_eq!(s.receipt_repair().unwrap().step_id(), &verifier_id);
                let a = tx.apply_recovery(
                    &s,
                    RecoveryAction::ReceiptRepair {
                        step_id: verifier_id.clone(),
                    },
                    at(50),
                    c,
                )?;
                assert!(a.receipt_repaired);
                assert_eq!(a.state, Some(TaskState::Completed));
                Ok(())
            })
            .unwrap();
        let before = dump(&store);
        store
            .transact_with_audit(&TestAudit, |tx| {
                let s = tx.inspect_recovery_task(&tid())?;
                assert!(
                    !tx.apply_recovery(
                        &s,
                        RecoveryAction::ReceiptRepair {
                            step_id: verifier_id
                        },
                        at(90),
                        c
                    )?
                    .changed
                );
                Ok(())
            })
            .unwrap();
        assert_eq!(dump(&store), before);
        {
            let conn = store.conn.lock().unwrap();
            conn.execute("UPDATE tasks SET state='VERIFYING'", [])
                .unwrap();
            conn.execute(
                "UPDATE task_journal SET journal_kind='RECOVERY_DECISION'
                WHERE journal_kind='TASK_TERMINAL' AND payload_ref_digest IS NOT ?1",
                [recovery_edge_marker().as_str()],
            )
            .unwrap();
        }
        store
            .transact(|tx| {
                let snapshot = tx.inspect_recovery_task(&tid())?;
                assert_eq!(snapshot.corruption(), Some(&reason("INVARIANT_VIOLATION")));
                assert!(snapshot.receipt_repair().is_none());
                Ok(())
            })
            .unwrap();
    });
}

#[test]
fn terminal_retained_inflight_authority_is_a_strict_noop() {
    context(|c| {
        let store = fixture(3, c);
        let g = acquire(&store, c);
        store
            .transact_with_audit(&TestAudit, |tx| {
                tx.begin_attempt(&g, at(41), c)?;
                tx.cancel_task(&tid(), TaskOriginKind::new("HOST").unwrap(), at(42), c)?;
                Ok(())
            })
            .unwrap();
        let before = dump(&store);
        store
            .transact_with_audit(&Reject, |tx| {
                let s = tx.inspect_recovery_task(&tid())?;
                let a = tx.apply_recovery(&s, RecoveryAction::BlockedTask, at(100), c)?;
                assert!(!a.changed);
                assert!(a.revoked_steps.is_empty());
                Ok(())
            })
            .unwrap();
        assert_eq!(dump(&store), before);
    });
}

#[test]
fn waiting_corruption_is_not_falsely_resolved_and_blocked_authority_can_be_revoked() {
    context(|c| {
        let store = fixture(3, c);
        let g = acquire(&store, c);
        store
            .transact_with_audit(&TestAudit, |tx| tx.begin_attempt(&g, at(41), c))
            .unwrap();
        store.conn.lock().unwrap().execute_batch("UPDATE tasks SET state='WAITING_APPROVAL'; PRAGMA ignore_check_constraints=ON; UPDATE task_steps SET status='FUTURE_STATUS'; PRAGMA ignore_check_constraints=OFF").unwrap();
        store
            .transact_with_audit(&TestAudit, |tx| {
                let s = tx.inspect_recovery_task(&tid())?;
                let a = tx.apply_recovery(
                    &s,
                    RecoveryAction::Quarantine {
                        reason: reason("UNRECOGNISED_STATE"),
                    },
                    at(50),
                    c,
                )?;
                assert!(!a.blocked);
                assert!(a.revoked_steps.is_empty());
                assert_eq!(a.state, Some(TaskState::WaitingApproval));
                Ok(())
            })
            .unwrap();
        let other = fixture(3, c);
        let g = acquire(&other, c);
        other
            .transact_with_audit(&TestAudit, |tx| {
                tx.begin_attempt(&g, at(41), c)?;
                tx.block_task(
                    &tid(),
                    TaskState::Executing,
                    BlockedReason::new("POLICY_DENIED").unwrap(),
                    at(42),
                    c,
                )?;
                let s = tx.inspect_recovery_task(&tid())?;
                let a = tx.apply_recovery(&s, RecoveryAction::BlockedTask, at(50), c)?;
                assert_eq!(a.revoked_steps, vec![sid()]);
                Ok(())
            })
            .unwrap();
    });
}
