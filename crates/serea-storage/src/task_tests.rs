use super::*;
use crate::audit::{JournalRecords, TaskAuditParticipant, TestAudit};
use serea_protocol::{
    ActorId, ActorKind, AttemptBudget, Clock, ProtocolError, RiskClass, SemVer, StepStatus,
    TaskKind, TaskOrigin, TaskOriginKind, TaskTitle,
};

const TASK: &str = "tsk_01JQ8Z9K3M7QWXR4V2T6YH0BNA";
const STEP: &str = "stp_01JQ8Z9M3R2CVN8H5FWK7PQDSF";
const STEP2: &str = "stp_01JQ8Z9M3R2CVN8H5FWK7PQDSG";
fn at(n: i64) -> EpochMillis {
    EpochMillis::new(n).unwrap()
}
struct Fixed;
impl Clock for Fixed {
    fn now_ms(&self) -> Result<EpochMillis, ProtocolError> {
        Ok(at(0))
    }
}
fn store() -> Store {
    Store::open_in_memory(&Fixed).unwrap()
}
fn task() -> AssistantTask {
    AssistantTask {
        task_id: TaskId::new(TASK).unwrap(),
        kind: TaskKind::UserRequest,
        title: TaskTitle::new("storage task").unwrap(),
        state: TaskState::Received,
        origin: TaskOrigin {
            kind: TaskOriginKind::new("USER_MESSAGE").unwrap(),
            device_id: None,
            message_id: None,
            extensions: [("future_origin".into(), serde_json::json!({"a":[1,null]}))].into(),
        },
        data_class: DataClass::Personal,
        policy_class: RiskClass::Observe,
        created_at: Timestamp::from_epoch_millis(at(10)),
        updated_at: Timestamp::from_epoch_millis(at(10)),
        deadline_at: Some(Timestamp::from_epoch_millis(at(1000))),
        attempt_budget: AttemptBudget {
            max_model_calls: 12,
            max_tool_calls: 24,
            max_attempts_per_step: 3,
            extensions: [("future_budget".into(), serde_json::json!([true, 3]))].into(),
        },
        steps: vec![],
        blocked_reason: None,
        result_summary: None,
        cancelled_at: None,
        cancelled_by: None,
        failure_reason: None,
        extensions: [(
            "future_task".into(),
            serde_json::json!({"nested":[null,true]}),
        )]
        .into(),
    }
}
fn context(body: impl FnOnce(&TransitionContext<'_>)) {
    let actor = ActorId::new("storage-test").unwrap();
    let version = SemVer::new("0.1.0").unwrap();
    body(&TransitionContext {
        actor_kind: ActorKind::User,
        actor_id: &actor,
        actor_version: &version,
        causation_id: None,
    });
}
fn input(id: &str, sequence: u32) -> StepInput {
    let input_json = b"{\"instruction\":\"keep all extensions\"}".to_vec();
    StepInput {
        step: TaskStep::new(TaskStepDraft {
            step_id: StepId::new(id).unwrap(),
            task_id: TaskId::new(TASK).unwrap(),
            sequence,
            kind: StepKind::Notify,
            status: StepStatus::new("PLANNED").unwrap(),
            attempt: 0,
            idempotency_key: None,
            provider_id: None,
            capability_id: None,
            capability_version: None,
            input_digest: digest_of(std::str::from_utf8(&input_json).unwrap()).unwrap(),
            result_digest: None,
            side_effect_receipt: None,
            started_at: None,
            completed_at: None,
            lease_owner: None,
            lease_expires_at: None,
            lease_generation: None,
            error: None,
            extensions: [(
                "future_step".into(),
                serde_json::json!({"unchanged":[7,null]}),
            )]
            .into(),
        })
        .unwrap(),
        input_json,
    }
}
fn prepare(s: &Store, c: &TransitionContext<'_>) {
    s.transact_with_audit(&TestAudit, |tx| {
        tx.insert_task(&task(), c)?;
        tx.start_planning(
            &TaskId::new(TASK).unwrap(),
            TaskState::Received,
            0,
            at(20),
            c,
        )?;
        Ok(())
    })
    .unwrap();
}
fn persist(
    s: &Store,
    revision: u32,
    steps: Vec<StepInput>,
    c: &TransitionContext<'_>,
) -> Result<PlanRevisionSnapshot, StoreError> {
    s.transact_with_audit(&TestAudit, |tx| {
        tx.put_plan_revision(
            &TaskId::new(TASK).unwrap(),
            PlanWrite { revision, steps },
            at(30 + i64::from(revision)),
            c,
        )
    })
}
fn planning(s: &Store, revision: u32, c: &TransitionContext<'_>) {
    s.transact_with_audit(&TestAudit, |tx| {
        tx.start_planning(
            &TaskId::new(TASK).unwrap(),
            TaskState::Ready,
            revision,
            at(30 + i64::from(revision)),
            c,
        )
    })
    .unwrap();
}
fn counts(s: &Store) -> Vec<i64> {
    let conn = s.conn.lock().unwrap();
    [
        "tasks",
        "task_steps",
        "blobs",
        "plan_revisions",
        "task_blob_refs",
        "step_blob_refs",
        "task_journal",
    ]
    .map(|t| {
        conn.query_row(&format!("SELECT count(*) FROM {t}"), [], |r| r.get(0))
            .unwrap()
    })
    .to_vec()
}
#[test]
fn creation_and_both_loads_preserve_nested_extensions() {
    context(|c| {
        let s = store();
        let expected = task();
        let got = s
            .transact_with_audit(&TestAudit, |tx| tx.insert_task(&expected, c))
            .unwrap();
        assert_eq!(got.task, expected);
        assert_eq!(got.plan_revision, 0);
        assert!(got.steps.is_empty());
        assert_eq!(s.load_task(&expected.task_id).unwrap().task, expected);
        assert_eq!(
            s.transact(|tx| tx.load_task(&expected.task_id))
                .unwrap()
                .task,
            expected
        );
    });
}
#[test]
fn creation_requires_audit_and_refuses_initial_runtime() {
    context(|c| {
        let s = store();
        assert_eq!(
            s.transact(|tx| tx.insert_task(&task(), c)).unwrap_err(),
            StoreError::AuditRequired
        );
        let mut bad = task();
        bad.state = TaskState::Ready;
        assert!(
            s.transact_with_audit(&TestAudit, |tx| tx.insert_task(&bad, c))
                .is_err()
        );
        assert_eq!(counts(&s), vec![0; 7]);
    });
}
#[test]
fn ordinary_creation_refuses_all_protected_classes() {
    context(|c| {
        for class in [DataClass::Private, DataClass::Secret, DataClass::Credential] {
            let s = store();
            let mut bad = task();
            bad.data_class = class;
            assert!(
                s.transact_with_audit(&TestAudit, |tx| tx.insert_task(&bad, c))
                    .is_err()
            );
            assert_eq!(counts(&s), vec![0; 7]);
        }
    });
}
#[test]
fn reserved_task_origin_and_budget_extensions_fail_before_writes() {
    context(|c| {
        for layer in 0..3 {
            let s = store();
            let mut bad = task();
            match layer {
                0 => {
                    bad.extensions
                        .insert("state".into(), serde_json::json!("READY"));
                }
                1 => {
                    bad.origin
                        .extensions
                        .insert("kind".into(), serde_json::json!("FORGED"));
                }
                _ => {
                    bad.attempt_budget
                        .extensions
                        .insert("max_tool_calls".into(), serde_json::json!(999));
                }
            }
            assert!(
                s.transact_with_audit(&TestAudit, |tx| tx.insert_task(&bad, c))
                    .is_err()
            );
            assert_eq!(counts(&s), vec![0; 7]);
        }
    });
}
#[test]
fn planning_is_explicit_and_stale_predicates_do_not_write() {
    context(|c| {
        let s = store();
        prepare(&s, c);
        let before = counts(&s);
        assert_eq!(
            s.transact_with_audit(&TestAudit, |tx| tx.start_planning(
                &TaskId::new(TASK).unwrap(),
                TaskState::Received,
                0,
                at(25),
                c
            ))
            .unwrap_err(),
            StoreError::IllegalTaskTransition
        );
        assert_eq!(counts(&s), before);
    });
}
#[test]
fn initial_plan_roundtrips_step_extensions_and_reference_roles() {
    context(|c| {
        let s = store();
        prepare(&s, c);
        let expected = input(STEP, 10).step;
        let revision = persist(&s, 1, vec![input(STEP, 10)], c).unwrap();
        assert_eq!(revision.step_count, 1);
        let snapshot = s.load_task(&TaskId::new(TASK).unwrap()).unwrap();
        assert_eq!(snapshot.task.state, TaskState::Ready);
        assert_eq!(snapshot.plan_revision, 1);
        assert_eq!(snapshot.steps[0].step, expected);
        assert_eq!(snapshot.steps[0].plan_revision, 1);
        assert_eq!(snapshot.task.steps, vec![expected]);
        assert_eq!(counts(&s), vec![1, 1, 2, 1, 2, 1, 4]);
    });
}
#[test]
fn malformed_last_input_rolls_back_even_when_error_is_caught() {
    context(|c| {
        let s = store();
        prepare(&s, c);
        let before = counts(&s);
        let mut bad = input(STEP2, 20);
        bad.input_json = b"{\"x\":1,\"x\":2}".to_vec();
        s.transact_with_audit(&TestAudit, |tx| {
            assert!(
                tx.put_plan_revision(
                    &TaskId::new(TASK).unwrap(),
                    PlanWrite {
                        revision: 1,
                        steps: vec![input(STEP, 10), bad]
                    },
                    at(30),
                    c
                )
                .is_err()
            );
            tx.put_blob(b"{\"unrelated\":true}", DataClass::Public)?;
            Ok(())
        })
        .unwrap();
        let mut after = before;
        after[2] += 1;
        assert_eq!(counts(&s), after);
    });
}
#[test]
fn empty_plan_fails_closed() {
    context(|c| {
        let s = store();
        prepare(&s, c);
        let before = counts(&s);
        assert_eq!(
            persist(&s, 1, vec![], c).unwrap_err(),
            StoreError::InvalidPlanLayout
        );
        assert_eq!(counts(&s), before);
    });
}
#[test]
fn revisions_are_contiguous_and_only_planning_accepts_them() {
    context(|c| {
        let s = store();
        prepare(&s, c);
        assert_eq!(
            persist(&s, 2, vec![input(STEP, 10)], c).unwrap_err(),
            StoreError::PlanRevisionConflict
        );
        persist(&s, 1, vec![input(STEP, 10)], c).unwrap();
        let before = counts(&s);
        assert_eq!(
            persist(&s, 2, vec![input(STEP, 10)], c).unwrap_err(),
            StoreError::IllegalTaskTransition
        );
        assert_eq!(counts(&s), before);
    });
}
#[test]
fn retained_steps_keep_original_revision_and_cannot_change_specification() {
    context(|c| {
        let s = store();
        prepare(&s, c);
        persist(&s, 1, vec![input(STEP, 10)], c).unwrap();
        planning(&s, 1, c);
        let mut changed = input(STEP, 10);
        let mut draft = TaskStepDraft::from(changed.step);
        draft.extensions.insert("new".into(), serde_json::json!(1));
        changed.step = TaskStep::new(draft).unwrap();
        let before = counts(&s);
        assert_eq!(
            persist(&s, 2, vec![changed], c).unwrap_err(),
            StoreError::InvalidPlan
        );
        assert_eq!(counts(&s), before);
        persist(&s, 2, vec![input(STEP, 10), input(STEP2, 20)], c).unwrap();
        let loaded = s.load_task(&TaskId::new(TASK).unwrap()).unwrap();
        assert_eq!(loaded.steps[0].plan_revision, 1);
        assert_eq!(loaded.steps[1].plan_revision, 2);
    });
}
#[test]
fn removal_never_allows_historical_id_or_sequence_reuse() {
    context(|c| {
        let s = store();
        prepare(&s, c);
        persist(&s, 1, vec![input(STEP, 10), input(STEP2, 20)], c).unwrap();
        planning(&s, 1, c);
        persist(&s, 2, vec![input(STEP, 10)], c).unwrap();
        planning(&s, 2, c);
        assert_eq!(
            persist(&s, 3, vec![input(STEP, 10), input(STEP2, 30)], c).unwrap_err(),
            StoreError::InvalidPlan
        );
        let fresh = "stp_01JQ8Z9M3R2CVN8H5FWK7PQDSH";
        assert_eq!(
            persist(&s, 3, vec![input(STEP, 10), input(fresh, 20)], c).unwrap_err(),
            StoreError::InvalidPlan
        );
        persist(&s, 3, vec![input(STEP, 10), input(fresh, 30)], c).unwrap();
    });
}
#[test]
fn removed_input_sweep_keeps_unrelated_and_history() {
    context(|c| {
        let s = store();
        prepare(&s, c);
        let unrelated = s
            .transact(|tx| tx.put_blob(b"{\"standalone\":true}", DataClass::Public))
            .unwrap();
        persist(&s, 1, vec![input(STEP, 10), input(STEP2, 20)], c).unwrap();
        planning(&s, 1, c);
        persist(&s, 2, vec![input(STEP, 10)], c).unwrap();
        assert!(s.transact(|tx| tx.get_blob(&unrelated)).is_ok());
        assert_eq!(
            s.load_task(&TaskId::new(TASK).unwrap())
                .unwrap()
                .steps
                .len(),
            1
        );
    });
}
struct Reject;
impl TaskAuditParticipant for Reject {
    fn records(&self, _: &DurableTransition) -> Result<JournalRecords, StoreError> {
        Err(StoreError::AuditRejected)
    }
}
#[test]
fn late_audit_failure_rolls_back_complete_plan_and_allows_outer_work() {
    context(|c| {
        let s = store();
        prepare(&s, c);
        let before = counts(&s);
        s.transact_with_audit(&Reject, |tx| {
            assert_eq!(
                tx.put_plan_revision(
                    &TaskId::new(TASK).unwrap(),
                    PlanWrite {
                        revision: 1,
                        steps: vec![input(STEP, 10)]
                    },
                    at(30),
                    c
                )
                .unwrap_err(),
                StoreError::AuditRejected
            );
            tx.put_blob(b"{\"outer\":true}", DataClass::Public)?;
            Ok(())
        })
        .unwrap();
        let mut after = before;
        after[2] += 1;
        assert_eq!(counts(&s), after);
        assert_eq!(
            s.load_task(&TaskId::new(TASK).unwrap()).unwrap().task.state,
            TaskState::Planning
        );
    });
}
#[test]
fn missing_provenance_and_unknown_status_are_corrupt_not_coerced() {
    context(|c| {
        let s = store();
        prepare(&s, c);
        persist(&s, 1, vec![input(STEP, 10)], c).unwrap();
        {
            let conn = s.conn.lock().unwrap();
            conn.execute_batch("PRAGMA ignore_check_constraints=ON; UPDATE task_steps SET status='FUTURE'; PRAGMA ignore_check_constraints=OFF;").unwrap();
        }
        assert_eq!(
            s.load_task(&TaskId::new(TASK).unwrap()).unwrap_err(),
            StoreError::CorruptRow
        );
    });
}
#[test]
fn corrupt_numeric_task_field_is_not_truncated() {
    context(|c| {
        let s = store();
        prepare(&s, c);
        {
            let conn = s.conn.lock().unwrap();
            conn.execute("UPDATE tasks SET max_tool_calls=4294967296", [])
                .unwrap();
        }
        assert_eq!(
            s.load_task(&TaskId::new(TASK).unwrap()).unwrap_err(),
            StoreError::CorruptRow
        );
    });
}

#[test]
fn revision_overflow_refuses_before_provenance_or_writes() {
    context(|c| {
        let s = store();
        prepare(&s, c);
        s.conn
            .lock()
            .unwrap()
            .execute("UPDATE tasks SET plan_revision=4294967295", [])
            .unwrap();
        let before = counts(&s);
        assert_eq!(
            persist(&s, 0, vec![input(STEP, 10)], c).err(),
            Some(StoreError::PlanRevisionOverflow)
        );
        assert_eq!(counts(&s), before);
    });
}

struct RowBackend;
impl crate::AtRestProtection for RowBackend {
    fn protect(&self, _: &[u8]) -> Result<Vec<u8>, crate::AtRestProtectionError> {
        panic!("not row protection")
    }
    fn unprotect(&self, _: &[u8]) -> Result<Vec<u8>, crate::AtRestProtectionError> {
        panic!("not row protection")
    }
}
#[test]
fn ordinary_plan_and_revision_refuse_private_even_with_backend() {
    context(|c| {
        for revision in [0, 1] {
            let s = Store::open_in_memory_with_protection(&Fixed, std::sync::Arc::new(RowBackend))
                .unwrap();
            prepare(&s, c);
            if revision == 1 {
                persist(&s, 1, vec![input(STEP, 10)], c).unwrap();
                planning(&s, 1, c);
            }
            s.conn
                .lock()
                .unwrap()
                .execute("UPDATE tasks SET data_class_rank=2", [])
                .unwrap();
            let before = counts(&s);
            assert_eq!(
                persist(&s, revision + 1, vec![input(STEP, 10)], c).err(),
                Some(StoreError::AtRestProtectionUnavailable)
            );
            assert_eq!(counts(&s), before);
        }
    });
}

#[test]
fn failed_step_reference_insert_rolls_back_blobs_revision_steps_state_and_journal() {
    context(|c| {
        let s = store();
        prepare(&s, c);
        let before = counts(&s);
        s.conn.lock().unwrap().execute_batch("CREATE TEMP TRIGGER reject_ref BEFORE INSERT ON step_blob_refs BEGIN SELECT RAISE(ABORT,'reference refused'); END;").unwrap();
        s.transact_with_audit(&TestAudit, |tx| {
            assert_eq!(
                tx.put_plan_revision(
                    &TaskId::new(TASK).unwrap(),
                    PlanWrite {
                        revision: 1,
                        steps: vec![input(STEP, 10)]
                    },
                    at(30),
                    c
                )
                .err(),
                Some(StoreError::ConstraintViolation)
            );
            tx.put_blob(b"{\"unrelated\":true}", DataClass::Public)?;
            Ok(())
        })
        .unwrap();
        let mut after = before;
        after[2] += 1;
        assert_eq!(counts(&s), after);
        assert_eq!(
            s.load_task(&TaskId::new(TASK).unwrap()).unwrap().task.state,
            TaskState::Planning
        );
    });
}

#[test]
fn deleted_planned_step_remains_in_prior_revision_blob() {
    context(|c| {
        let s = store();
        prepare(&s, c);
        let prior = persist(&s, 1, vec![input(STEP, 10), input(STEP2, 20)], c).unwrap();
        planning(&s, 1, c);
        persist(&s, 2, vec![input(STEP, 10)], c).unwrap();
        let bytes = s.transact(|tx| tx.get_blob(&prior.blob)).unwrap();
        let document: PlanDocument = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(document.steps[1].step, input(STEP2, 20).step);
        assert_eq!(
            document.steps[1].input_json,
            std::str::from_utf8(&input(STEP2, 20).input_json).unwrap()
        );
    });
}

#[test]
fn missing_original_provenance_fails_closed() {
    context(|c| {
        let s = store();
        prepare(&s, c);
        persist(&s, 1, vec![input(STEP, 10)], c).unwrap();
        s.conn
            .lock()
            .unwrap()
            .execute("UPDATE task_steps SET plan_revision=2", [])
            .unwrap();
        assert_eq!(
            s.load_task(&TaskId::new(TASK).unwrap()).err(),
            Some(StoreError::CorruptRow)
        );
    });
}

#[test]
fn initial_plan_duplicate_sequence_and_wrong_parent_fail_before_any_write() {
    context(|c| {
        for mode in 0..3 {
            let s = store();
            prepare(&s, c);
            let before = counts(&s);
            let mut second = input(STEP2, 20);
            match mode {
                0 => second = input(STEP2, 10),
                1 => second = input(STEP, 20),
                _ => {
                    let mut draft = TaskStepDraft::from(second.step);
                    draft.task_id = TaskId::new("tsk_01JQ8Z9K3M7QWXR4V2T6YH0BNB").unwrap();
                    second.step = TaskStep::new(draft).unwrap();
                }
            }
            assert!(persist(&s, 1, vec![input(STEP, 10), second], c).is_err());
            assert_eq!(counts(&s), before);
        }
    });
}

#[test]
fn task_query_changes_no_connection_policy() {
    context(|c| {
        let s = store();
        prepare(&s, c);
        let policy = |s: &Store| {
            let conn = s.conn.lock().unwrap();
            [
                "PRAGMA foreign_keys",
                "PRAGMA synchronous",
                "PRAGMA busy_timeout",
                "PRAGMA query_only",
            ]
            .map(|sql| conn.query_row(sql, [], |r| r.get::<_, i64>(0)).unwrap())
        };
        let before = policy(&s);
        s.load_task(&TaskId::new(TASK).unwrap()).unwrap();
        assert_eq!(policy(&s), before);
    });
}

#[test]
fn failed_return_projection_never_dispatches_participant() {
    struct Count(std::sync::atomic::AtomicUsize);
    impl TaskAuditParticipant for Count {
        fn records(&self, f: &DurableTransition) -> Result<JournalRecords, StoreError> {
            self.0.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            TestAudit.records(f)
        }
    }
    context(|c| {
        for initial in [true, false] {
            let s = store();
            if !initial {
                s.transact_with_audit(&TestAudit, |tx| tx.insert_task(&task(), c))
                    .unwrap();
            }
            let before = counts(&s);
            let count = Count(std::sync::atomic::AtomicUsize::new(0));
            let trigger = if initial {
                "AFTER INSERT"
            } else {
                "AFTER UPDATE"
            };
            s.conn.lock().unwrap().execute_batch(&format!("CREATE TEMP TRIGGER corrupt_projection {trigger} ON tasks BEGIN UPDATE tasks SET extensions='{{\"state\":\"READY\"}}'; END;")).unwrap();
            s.transact_with_audit(&count, |tx| {
                let got = if initial {
                    tx.insert_task(&task(), c)
                } else {
                    tx.start_planning(
                        &TaskId::new(TASK).unwrap(),
                        TaskState::Received,
                        0,
                        at(20),
                        c,
                    )
                };
                assert_eq!(got.err(), Some(StoreError::CorruptRow));
                Ok(())
            })
            .unwrap();
            assert_eq!(count.0.load(std::sync::atomic::Ordering::SeqCst), 0);
            assert_eq!(counts(&s), before);
        }
    });
}

fn complete_rows(s: &Store) -> Vec<Vec<Vec<rusqlite::types::Value>>> {
    let conn = s.conn.lock().unwrap();
    [
        "tasks",
        "task_steps",
        "side_effect_receipts",
        "leases",
        "blobs",
        "plan_revisions",
        "task_blob_refs",
        "step_blob_refs",
        "task_journal",
    ]
    .map(|table| {
        let mut stmt = conn
            .prepare(&format!("SELECT * FROM {table} ORDER BY 1,2"))
            .unwrap();
        let columns = stmt.column_count();
        stmt.query_map([], |r| (0..columns).map(|i| r.get(i)).collect())
            .unwrap()
            .map(Result::unwrap)
            .collect()
    })
    .to_vec()
}

#[test]
fn corrupt_receipt_tuple_is_refused_by_both_snapshot_read_paths() {
    use serea_protocol::{
        CapabilityId, EffectSummary, LeaseOwner, ProviderId, ReceiptId, SideEffectReceipt,
    };
    context(|c| {
        for field in ["capability_id", "idempotency_key"] {
            let s = store();
            prepare(&s, c);
            let mut p = input(STEP, 10);
            let mut d = TaskStepDraft::from(p.step);
            d.kind = StepKind::Capability;
            let cap = CapabilityId::new("calendar.events.create").unwrap();
            let version = SemVer::new("1.0.0").unwrap();
            d.idempotency_key = Some(
                derive_idempotency_key(
                    &d.task_id,
                    &d.step_id,
                    &cap,
                    &version,
                    std::str::from_utf8(&p.input_json).unwrap(),
                )
                .unwrap(),
            );
            d.provider_id = Some(ProviderId::new("calendar").unwrap());
            d.capability_id = Some(cap);
            d.capability_version = Some(version);
            p.step = TaskStep::new(d).unwrap();
            let planned = p.step.clone();
            persist(&s, 1, vec![p], c).unwrap();
            let receipt = SideEffectReceipt {
                receipt_id: ReceiptId::new("rcp_01JQ8Z9M3R2CVN8H5FWK7PQDSF").unwrap(),
                capability_id: planned.capability_id.clone().unwrap(),
                idempotency_key: planned.idempotency_key.clone().unwrap(),
                provider_reference: None,
                effect_summary: EffectSummary::new("durable receipt").unwrap(),
                observed_at: Timestamp::from_epoch_millis(at(42)),
                replay_safe: true,
            };
            s.transact_with_audit(&TestAudit, |tx| {
                let g = tx.acquire_audited(
                    TaskId::new(TASK).unwrap(),
                    StepId::new(STEP).unwrap(),
                    LeaseOwner::new("test-worker").unwrap(),
                    None,
                    at(40),
                    at(60),
                    c,
                )?;
                tx.begin_attempt(&g, at(41), c)?;
                tx.commit_step_outcome(
                    g,
                    crate::StepOutcome::Succeeded {
                        result_json: b"{}",
                        receipt: Some(&receipt),
                    },
                    at(42),
                    c,
                )
            })
            .unwrap();
            let value = if field == "capability_id" {
                "calendar.events.list".into()
            } else {
                format!("idk_{}", "f".repeat(64))
            };
            s.conn
                .lock()
                .unwrap()
                .execute(
                    &format!("UPDATE side_effect_receipts SET {field}=?1"),
                    [value],
                )
                .unwrap();
            assert_eq!(
                s.load_task(&TaskId::new(TASK).unwrap()).err(),
                Some(StoreError::CorruptRow)
            );
            assert_eq!(
                s.transact(|tx| tx.load_task(&TaskId::new(TASK).unwrap()))
                    .err(),
                Some(StoreError::CorruptRow)
            );
        }
    });
}

#[test]
fn replacement_failure_restores_exact_rows_refs_and_swept_blob_identities() {
    context(|c| {
        for mode in ["audit", "reference", "ignored-plan-delete"] {
            let s = store();
            prepare(&s, c);
            let mut old = input(STEP2, 20);
            old.input_json = b"{\"superseded\":true}".to_vec();
            let mut draft = TaskStepDraft::from(old.step);
            draft.input_digest = digest_of(std::str::from_utf8(&old.input_json).unwrap()).unwrap();
            old.step = TaskStep::new(draft).unwrap();
            persist(&s, 1, vec![input(STEP, 10), old], c).unwrap();
            planning(&s, 1, c);
            let before = complete_rows(&s);
            if mode == "reference" {
                s.conn.lock().unwrap().execute_batch("CREATE TEMP TRIGGER reject_replacement BEFORE INSERT ON step_blob_refs BEGIN SELECT RAISE(ABORT,'reference fault'); END;").unwrap();
            }
            if mode == "ignored-plan-delete" {
                s.conn.lock().unwrap().execute_batch("CREATE TEMP TRIGGER ignore_plan_delete BEFORE DELETE ON task_blob_refs WHEN OLD.role='PLAN' BEGIN SELECT RAISE(IGNORE); END;").unwrap();
            }
            let mapper: &dyn TaskAuditParticipant =
                if mode == "audit" { &Reject } else { &TestAudit };
            let unrelated = s
                .transact_with_audit(mapper, |tx| {
                    let plan = PlanWrite {
                        revision: 2,
                        steps: vec![input(STEP, 10), input("stp_01JQ8Z9M3R2CVN8H5FWK7PQDSH", 30)],
                    };
                    let expected = if mode == "audit" {
                        StoreError::AuditRejected
                    } else {
                        StoreError::ConstraintViolation
                    };
                    assert_eq!(
                        tx.put_plan_revision(&TaskId::new(TASK).unwrap(), plan, at(40), c)
                            .err(),
                        Some(expected)
                    );
                    tx.put_blob(b"{\"unrelated_committed\":true}", DataClass::Public)
                })
                .unwrap();
            let mut after = complete_rows(&s);
            after[4].retain(|row| {
                row[0] != rusqlite::types::Value::Text(unrelated.digest().as_str().into())
            });
            assert_eq!(
                after, before,
                "{mode} must restore every prior identity and byte"
            );
        }
    });
}
