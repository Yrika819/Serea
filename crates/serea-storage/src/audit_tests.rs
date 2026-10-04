//! Audit seam contract tests, using the existing P2F-a fixtures and literal oracle.
use super::*;
use crate::audit::{
    AuditOperation, DurableTransition, JournalKind, JournalRecord, JournalRecords,
    TaskAuditParticipant, TestAudit,
};
use std::sync::atomic::AtomicUsize;

#[test]
fn private_audited_leases_refuse_before_delegated_sql() {
    for release in [false, true] {
        let s = memory();
        let guard = if release { Some(acquire(&s)) } else { None };
        s.conn
            .lock()
            .unwrap()
            .execute("UPDATE tasks SET data_class_rank=2", [])
            .unwrap();
        s.conn.lock().unwrap().execute_batch("CREATE TEMP TRIGGER forbid_private_lease BEFORE INSERT ON leases BEGIN SELECT RAISE(ROLLBACK,'must refuse before lease write'); END; CREATE TEMP TRIGGER forbid_private_release BEFORE UPDATE ON leases BEGIN SELECT RAISE(ROLLBACK,'must refuse before lease write'); END;").unwrap();
        let before = snapshot(&s);
        let c = Context::new();
        let audit = Counting::new("ok");
        let error = s.transact_with_audit(&audit, |tx| match guard {
            Some(g) => tx.release_audited(g, time(12), &c.view()),
            None => tx
                .acquire_audited(
                    TaskId::new(TASK).unwrap(),
                    StepId::new(STEP).unwrap(),
                    LeaseOwner::new(OWNER).unwrap(),
                    None,
                    time(10),
                    time(20),
                    &c.view(),
                )
                .map(|_| ()),
        });
        assert_eq!(error, Err(StoreError::AtRestProtectionUnavailable));
        assert_eq!(audit.calls.load(Ordering::SeqCst), 0);
        assert_eq!(snapshot(&s), before);
    }
}

#[test]
fn plain_audited_lease_wrappers_refuse_without_writing() {
    let s = memory();
    let c = Context::new();
    let before = snapshot(&s);
    assert_eq!(
        s.transact(|tx| tx.acquire_audited(
            TaskId::new(TASK).unwrap(),
            StepId::new(STEP).unwrap(),
            LeaseOwner::new(OWNER).unwrap(),
            None,
            time(10),
            time(20),
            &c.view()
        ))
        .err(),
        Some(StoreError::AuditRequired)
    );
    assert_eq!(snapshot(&s), before);
    let g = acquire(&s);
    let before = snapshot(&s);
    assert_eq!(
        s.transact(|tx| tx.release_audited(g, time(12), &c.view())),
        Err(StoreError::AuditRequired)
    );
    assert_eq!(snapshot(&s), before);
}

#[test]
fn late_outcome_write_failure_never_dispatches_mapper() {
    let s = memory();
    let g = running(&s);
    let c = Context::new();
    let audit = Counting::new("ok");
    let before = snapshot(&s);
    s.conn.lock().unwrap().execute_batch("CREATE TEMP TRIGGER fail_final_write BEFORE UPDATE ON leases BEGIN SELECT RAISE(ABORT,'controlled final write error'); END;").unwrap();
    s.transact_with_audit(&audit, |tx| {
        assert_eq!(
            tx.commit_step_outcome(
                g,
                StepOutcome::Succeeded {
                    result_json: b"{}",
                    receipt: None
                },
                time(12),
                &c.view()
            )
            .err(),
            Some(StoreError::ConstraintViolation)
        );
        Ok(())
    })
    .unwrap();
    assert_eq!(audit.calls.load(Ordering::SeqCst), 0);
    assert_eq!(snapshot(&s), before);
}

#[test]
fn outcome_mapper_error_panic_and_wrong_cause_restore_complete_method() {
    for failure in [false, true] {
        for mode in ["reject", "panic", "wrong-reason"] {
            if !failure && mode == "wrong-reason" {
                continue;
            }
            let s = memory();
            let g = running(&s);
            let c = Context::new();
            let r = receipt();
            let audit = Counting::new(mode);
            let before = snapshot(&s);
            s.transact_with_audit(&audit, |tx| {
                tx.put_blob(b"1001", DataClass::Public)?;
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    if failure {
                        failure_call(tx, g, ActionErrorKind::ProviderError, None)
                    } else {
                        tx.commit_step_outcome(
                            g,
                            StepOutcome::Succeeded {
                                result_json: b"{\"ok\":true}",
                                receipt: Some(&r),
                            },
                            time(12),
                            &c.view(),
                        )
                    }
                }));
                if mode == "panic" {
                    assert!(result.is_err());
                } else {
                    assert_eq!(result.unwrap().err(), Some(StoreError::AuditRejected));
                }
                tx.put_blob(b"1002", DataClass::Public)?;
                Ok(())
            })
            .unwrap();
            let after = snapshot(&s);
            for i in [1, 2, 3, 5, 6, 7, 8, 9] {
                assert_eq!(after[i], before[i]);
            }
            assert_eq!(scalar::<i64>(&s, "SELECT count(*) FROM blobs"), 2);
            assert_eq!(audit.calls.load(Ordering::SeqCst), 1);
        }
    }
}

#[test]
fn audited_lease_mapper_panics_restore_method_inside_catching_transaction() {
    for release in [false, true] {
        let s = memory();
        let c = Context::new();
        let audit = Counting::new("panic");
        let guard = if release { Some(acquire(&s)) } else { None };
        let before = snapshot(&s);
        s.transact_with_audit(&audit, |tx| {
            tx.put_blob(b"1001", DataClass::Public)?;
            let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| match guard {
                Some(g) => tx.release_audited(g, time(12), &c.view()),
                None => tx
                    .acquire_audited(
                        TaskId::new(TASK).unwrap(),
                        StepId::new(STEP).unwrap(),
                        LeaseOwner::new(OWNER).unwrap(),
                        None,
                        time(10),
                        time(20),
                        &c.view(),
                    )
                    .map(|_| ()),
            }));
            assert!(panic.is_err());
            tx.put_blob(b"1002", DataClass::Public)?;
            Ok(())
        })
        .unwrap();
        let after = snapshot(&s);
        for i in [1, 2, 3, 5, 6, 7, 8, 9] {
            assert_eq!(after[i], before[i]);
        }
        assert_eq!(scalar::<i64>(&s, "SELECT count(*) FROM blobs"), 2);
    }
}

#[test]
fn audited_lease_body_failure_does_not_dispatch() {
    let s = memory();
    let g = acquire(&s);
    let c = Context::new();
    let audit = Counting::new("ok");
    let before = snapshot(&s);
    assert_eq!(
        s.transact_with_audit(&audit, |tx| tx.acquire_audited(
            TaskId::new(TASK).unwrap(),
            StepId::new(STEP).unwrap(),
            LeaseOwner::new(OWNER).unwrap(),
            Some(1),
            time(12),
            time(20),
            &c.view()
        ))
        .err(),
        Some(StoreError::LeaseHeld)
    );
    assert_eq!(
        s.transact_with_audit(&audit, |tx| tx.release_audited(g, time(9), &c.view())),
        Err(StoreError::InvalidLeaseInterval)
    );
    assert_eq!(audit.calls.load(Ordering::SeqCst), 0);
    assert_eq!(snapshot(&s), before);
}

#[test]
fn operation_savepoint_release_and_unwind_cleanup_failures_poison_outer_commit() {
    for mode in ["release", "panic"] {
        let s = memory();
        let before = snapshot(&s);
        assert_eq!(
            s.transact_with_audit(&TestAudit, |tx| {
                tx.put_blob(b"1001", DataClass::Public)?;
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    tx.operation_savepoint(|inner| {
                        inner.put_blob(b"1002", DataClass::Public)?;
                        inner.inner.execute_batch("RELEASE serea_operation")?;
                        if mode == "panic" {
                            panic!("controlled operation cleanup panic");
                        }
                        Ok(())
                    })
                }));
                if mode == "panic" {
                    assert!(result.is_err());
                } else {
                    assert_eq!(result.unwrap(), Err(StoreError::Sqlite));
                }
                assert!(tx.rollback_only);
                assert_eq!(
                    tx.put_blob(b"1003", DataClass::Public).err(),
                    Some(StoreError::Sqlite)
                );
                Ok(())
            }),
            Err(StoreError::Sqlite)
        );
        assert_eq!(snapshot(&s), before);
    }
}

#[test]
fn operation_savepoint_caught_panic_cleans_up_without_poisoning_outer_work() {
    let s = memory();
    s.transact_with_audit(&TestAudit, |tx| {
        tx.put_blob(b"1001", DataClass::Public)?;
        let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            tx.operation_savepoint(|inner| {
                inner.put_blob(b"1002", DataClass::Public)?;
                panic!("controlled operation panic");
                #[allow(unreachable_code)]
                Ok(())
            })
        }));
        assert!(panic.is_err());
        assert!(!tx.rollback_only);
        tx.put_blob(b"1003", DataClass::Public)?;
        Ok(())
    })
    .unwrap();
    assert_eq!(scalar::<i64>(&s, "SELECT count(*) FROM blobs"), 2);
    assert_eq!(
        scalar::<i64>(
            &s,
            "SELECT count(*) FROM blobs WHERE content=CAST('1002' AS BLOB)"
        ),
        0
    );
}

#[test]
fn participant_receives_one_actual_facts_batch_per_outcome_operation() {
    struct FactsAudit {
        calls: AtomicUsize,
    }
    impl TaskAuditParticipant for FactsAudit {
        fn records(&self, f: &DurableTransition) -> Result<JournalRecords, StoreError> {
            let n = self.calls.fetch_add(1, Ordering::SeqCst);
            assert_eq!(f.task_id().as_str(), TASK);
            assert_eq!(f.step_id().unwrap().as_str(), STEP);
            assert_eq!(f.attempt(), Some(1));
            assert_eq!(f.generation(), Some(1));
            assert_eq!(f.data_class(), DataClass::Personal);
            assert_eq!(f.actor_kind(), ActorKind::Host);
            assert_eq!(f.actor_id().as_str(), "p2f-test-host");
            assert_eq!(f.actor_version().as_str(), "0.2.0");
            assert_eq!(
                f.causation_id().unwrap().as_str(),
                "evt_01JQ8Z9M3R2CVN8H5FWK7PQDSF"
            );
            if n == 0 {
                assert_eq!(f.operation(), AuditOperation::AttemptStarted);
                assert_eq!(f.now(), time(11));
                assert_eq!(f.task_from(), Some(TaskState::Ready));
                assert_eq!(f.task_to(), TaskState::Executing);
                assert_eq!(f.step_from().unwrap().as_str(), "LEASED");
                assert_eq!(f.step_to().unwrap().as_str(), "EXECUTING");
                assert!(f.result().is_none());
                assert!(f.receipt_id().is_none());
            } else {
                assert_eq!(n, 1);
                assert_eq!(f.operation(), AuditOperation::StepSucceeded);
                assert_eq!(f.now(), time(12));
                assert_eq!(f.task_from(), Some(TaskState::Executing));
                assert_eq!(f.task_to(), TaskState::Verifying);
                assert_eq!(f.step_from().unwrap().as_str(), "EXECUTING");
                assert_eq!(f.step_to().unwrap().as_str(), "SUCCEEDED");
                assert_eq!(
                    f.result().unwrap(),
                    &serea_protocol::digest_of("{\"ok\":true}").unwrap()
                );
                assert_eq!(f.receipt_id().unwrap(), &receipt().receipt_id);
            }
            TestAudit.records(f)
        }
    }
    let s = memory();
    s.conn
        .lock()
        .unwrap()
        .execute("UPDATE tasks SET data_class_rank=1", [])
        .unwrap();
    let g = acquire(&s);
    let c = Context::new();
    let audit = FactsAudit {
        calls: AtomicUsize::new(0),
    };
    let cause = serea_protocol::EventId::new("evt_01JQ8Z9M3R2CVN8H5FWK7PQDSF").unwrap();
    let context = TransitionContext {
        causation_id: Some(&cause),
        ..c.view()
    };
    let r = receipt();
    s.transact_with_audit(&audit, |tx| {
        tx.begin_attempt(&g, time(11), &context)?;
        tx.commit_step_outcome(
            g,
            StepOutcome::Succeeded {
                result_json: b"{ \"ok\": true }",
                receipt: Some(&r),
            },
            time(12),
            &context,
        )
    })
    .unwrap();
    assert_eq!(audit.calls.load(Ordering::SeqCst), 2);
    assert_eq!(scalar::<i64>(&s, "SELECT count(*) FROM task_journal"), 5);
}

#[test]
fn checked_receipt_insert_failure_never_dispatches_mapper() {
    let s = memory();
    let g = running(&s);
    let c = Context::new();
    let audit = Counting::new("ok");
    let mut r = receipt();
    r.capability_id = CapabilityId::new("calendar.events.create").unwrap();
    let before = snapshot(&s);
    assert_eq!(
        s.transact_with_audit(&audit, |tx| tx.commit_step_outcome(
            g,
            StepOutcome::Succeeded {
                result_json: b"{}",
                receipt: Some(&r)
            },
            time(12),
            &c.view()
        ))
        .err(),
        Some(StoreError::ConstraintViolation)
    );
    assert_eq!(audit.calls.load(Ordering::SeqCst), 0);
    assert_eq!(snapshot(&s), before);
}

#[test]
fn audited_reclaim_reports_actual_pre_status_and_new_acquisition_counters() {
    struct Reclaim;
    impl TaskAuditParticipant for Reclaim {
        fn records(&self, f: &DurableTransition) -> Result<JournalRecords, StoreError> {
            assert_eq!(f.operation(), AuditOperation::LeaseAcquired);
            assert_eq!(f.task_from(), Some(TaskState::Executing));
            assert_eq!(f.task_to(), TaskState::Executing);
            assert_eq!(f.step_from().unwrap().as_str(), "EXECUTING");
            assert_eq!(f.step_to().unwrap().as_str(), "LEASED");
            assert_eq!(f.attempt(), Some(2));
            assert_eq!(f.generation(), Some(2));
            TestAudit.records(f)
        }
    }
    let s = memory();
    let old = running(&s);
    let c = Context::new();
    let g = s
        .transact_with_audit(&Reclaim, |tx| {
            tx.acquire_audited(
                TaskId::new(TASK).unwrap(),
                StepId::new(STEP).unwrap(),
                LeaseOwner::new(OWNER).unwrap(),
                Some(1),
                time(20),
                time(30),
                &c.view(),
            )
        })
        .unwrap();
    assert_eq!(g.generation(), 2);
    fenced_unchanged(&s, old);
}

struct Counting {
    calls: AtomicUsize,
    mode: &'static str,
}
impl Counting {
    fn new(mode: &'static str) -> Self {
        Self {
            calls: AtomicUsize::new(0),
            mode,
        }
    }
}
impl TaskAuditParticipant for Counting {
    fn records(&self, facts: &DurableTransition) -> Result<JournalRecords, StoreError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        match self.mode {
            "reject" => Err(StoreError::AuditRejected),
            "panic" => panic!("controlled mapper panic"),
            "empty" => Ok(vec![]),
            "invalid-json" => Ok(vec![JournalRecord {
                kind: JournalKind::StepAttemptStarted,
                state_from: Some("LEASED".into()),
                state_to: Some("EXECUTING".into()),
                reason: None,
                payload_json: b"{\"a\":1,\"a\":2}".to_vec(),
            }]),
            "non-object" | "non-utf8" | "unproven-receipt" => {
                let mut rows = TestAudit.records(facts)?;
                match self.mode {
                    "non-object" => rows[0].payload_json = b"[]".to_vec(),
                    "non-utf8" => rows[0].payload_json = vec![0xff],
                    _ => rows[0].kind = JournalKind::ReceiptRecorded,
                }
                Ok(rows)
            }
            "wrong-operation" => {
                let mut rows = TestAudit.records(facts)?;
                rows[0].kind = JournalKind::StepLeaseAcquired;
                Ok(rows)
            }
            "wrong-reason" => {
                let mut rows = TestAudit.records(facts)?;
                rows[0].reason = Some(serea_protocol::ReasonCode::new("UNRELATED_CAUSE").unwrap());
                Ok(rows)
            }
            "wrong-state" => {
                let mut rows = TestAudit.records(facts)?;
                rows[0].state_to = Some("FAILED".into());
                Ok(rows)
            }
            "canonical" => {
                let mut rows = TestAudit.records(facts)?;
                for row in &mut rows {
                    row.payload_json = b"{ \"z\": 2, \"a\": 1 }".to_vec();
                }
                Ok(rows)
            }
            _ => TestAudit.records(facts),
        }
    }
}

#[test]
fn plain_lifecycle_transactions_refuse_before_mutation() {
    let s = memory();
    let g = acquire(&s);
    let c = Context::new();
    let before = snapshot(&s);
    assert_eq!(
        s.transact(|tx| tx.begin_attempt(&g, time(11), &c.view())),
        Err(StoreError::AuditRequired)
    );
    assert_eq!(snapshot(&s), before);
    begin(&s, &g, 11).unwrap();
    let before = snapshot(&s);
    assert_eq!(
        s.transact(|tx| tx.commit_step_outcome(
            g,
            StepOutcome::Succeeded {
                result_json: b"{}",
                receipt: None
            },
            time(12),
            &c.view()
        ))
        .err(),
        Some(StoreError::AuditRequired)
    );
    assert_eq!(snapshot(&s), before);
}

#[test]
fn failed_operation_body_never_calls_mapper() {
    let s = memory();
    let g = acquire(&s);
    let c = Context::new();
    let audit = Counting::new("ok");
    let before = snapshot(&s);
    assert_eq!(
        s.transact_with_audit(&audit, |tx| tx.begin_attempt(&g, time(20), &c.view())),
        Err(StoreError::LeaseExpired)
    );
    assert_eq!(audit.calls.load(Ordering::SeqCst), 0);
    assert_eq!(snapshot(&s), before);
}

#[test]
fn rejected_or_invalid_drafts_restore_method_but_allow_surrounding_commit() {
    for mode in [
        "reject",
        "empty",
        "invalid-json",
        "wrong-state",
        "non-object",
        "non-utf8",
        "unproven-receipt",
        "wrong-operation",
    ] {
        let s = memory();
        let g = acquire(&s);
        let c = Context::new();
        let audit = Counting::new(mode);
        let before = snapshot(&s);
        s.transact_with_audit(&audit, |tx| {
            tx.put_blob(b"1001", DataClass::Public)?;
            assert_eq!(
                tx.begin_attempt(&g, time(11), &c.view()),
                Err(StoreError::AuditRejected)
            );
            tx.put_blob(b"1002", DataClass::Public)?;
            Ok(())
        })
        .unwrap();
        let after = snapshot(&s);
        for i in [1, 2, 3, 5, 6, 7, 8, 9] {
            assert_eq!(after[i], before[i], "{mode}");
        }
        assert_eq!(scalar::<i64>(&s, "SELECT count(*) FROM blobs"), 2);
        assert_eq!(audit.calls.load(Ordering::SeqCst), 1);
    }
}

#[test]
fn caught_mapper_panic_restores_method_and_resumes_unwind() {
    let s = memory();
    let g = acquire(&s);
    let c = Context::new();
    let audit = Counting::new("panic");
    let before = snapshot(&s);
    s.transact_with_audit(&audit, |tx| {
        tx.put_blob(b"1001", DataClass::Public)?;
        let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            tx.begin_attempt(&g, time(11), &c.view())
        }));
        assert!(panic.is_err());
        tx.put_blob(b"1002", DataClass::Public)?;
        Ok(())
    })
    .unwrap();
    let after = snapshot(&s);
    for i in [1, 2, 3, 5, 6, 7, 8, 9] {
        assert_eq!(after[i], before[i]);
    }
    assert_eq!(scalar::<i64>(&s, "SELECT count(*) FROM blobs"), 2);
}

#[test]
fn outer_error_rolls_back_earlier_mapped_success() {
    let s = memory();
    let g = acquire(&s);
    let c = Context::new();
    let audit = Counting::new("ok");
    let before = snapshot(&s);
    assert_eq!(
        s.transact_with_audit(&audit, |tx| {
            tx.begin_attempt(&g, time(11), &c.view())?;
            Err::<(), _>(StoreError::Sqlite)
        }),
        Err(StoreError::Sqlite)
    );
    assert_eq!(audit.calls.load(Ordering::SeqCst), 1);
    assert_eq!(snapshot(&s), before);
}

#[test]
fn private_sink_canonicalizes_and_binds_payload_digest() {
    let s = memory();
    let g = acquire(&s);
    let c = Context::new();
    let audit = Counting::new("canonical");
    s.transact_with_audit(&audit, |tx| tx.begin_attempt(&g, time(11), &c.view()))
        .unwrap();
    let payload = "{\"a\":1,\"z\":2}";
    assert_eq!(
        scalar::<String>(&s, "SELECT payload_json FROM task_journal LIMIT 1"),
        payload
    );
    assert_eq!(
        scalar::<String>(&s, "SELECT payload_digest FROM task_journal LIMIT 1"),
        serea_protocol::digest_of(payload).unwrap().as_str()
    );
    assert_eq!(
        scalar::<i64>(
            &s,
            "SELECT count(*) FROM task_journal WHERE actor_id='p2f-test-host' AND occurred_at_ms=11 AND attempt=1 AND data_class_rank=0"
        ),
        2
    );
}

#[test]
fn audited_acquire_and_release_preserve_lease_facts_and_order() {
    let s = memory();
    let c = Context::new();
    let g = s
        .transact_with_audit(&TestAudit, |tx| {
            tx.acquire_audited(
                TaskId::new(TASK).unwrap(),
                StepId::new(STEP).unwrap(),
                LeaseOwner::new(OWNER).unwrap(),
                None,
                time(10),
                time(20),
                &c.view(),
            )
        })
        .unwrap();
    s.transact_with_audit(&TestAudit, |tx| tx.release_audited(g, time(12), &c.view()))
        .unwrap();
    assert_eq!(
        scalar::<String>(
            &s,
            "SELECT group_concat(journal_kind, ',') FROM (SELECT journal_kind FROM task_journal ORDER BY journal_seq)"
        ),
        "STEP_LEASE_ACQUIRED,STEP_LEASE_RELEASED"
    );
    assert_eq!(scalar::<i64>(&s, "SELECT attempt FROM task_steps"), 1);
    assert_eq!(
        scalar::<String>(&s, "SELECT status FROM task_steps"),
        "LEASED"
    );
    assert_eq!(scalar::<i64>(&s, "SELECT released_at_ms FROM leases"), 12);
    assert_eq!(
        scalar::<i64>(&s, "SELECT lease_generation FROM task_steps"),
        1
    );
    assert_eq!(scalar::<i64>(&s, "SELECT count(*) FROM task_journal"), 2);
    assert_eq!(
        scalar::<String>(
            &s,
            "SELECT payload_json FROM task_journal WHERE journal_kind='STEP_LEASE_RELEASED'"
        ),
        "{\"attempt\":1,\"generation\":1,\"result_digest\":null}"
    );
}

#[test]
fn audited_release_reports_authoritative_generation_despite_step_copy() {
    for now in [12, 25] {
        let s = memory();
        let g = acquire(&s);
        assert_eq!(g.generation(), 1);
        let repeated = duplicate(&g);
        s.conn
            .lock()
            .unwrap()
            .execute("UPDATE task_steps SET lease_generation=2", [])
            .unwrap();
        let c = Context::new();
        let audit = Counting::new("ok");
        s.transact_with_audit(&audit, |tx| tx.release_audited(g, time(now), &c.view()))
            .unwrap();
        assert_eq!(scalar::<i64>(&s, "SELECT generation FROM leases"), 1);
        assert_eq!(scalar::<i64>(&s, "SELECT released_at_ms FROM leases"), now);
        assert_eq!(
            scalar::<i64>(&s, "SELECT lease_generation FROM task_steps"),
            2
        );
        assert_eq!(scalar::<i64>(&s, "SELECT attempt FROM task_steps"), 1);
        assert_eq!(
            scalar::<String>(&s, "SELECT status FROM task_steps"),
            "LEASED"
        );
        assert_eq!(scalar::<i64>(&s, "SELECT count(*) FROM task_journal"), 1);
        assert_eq!(
            scalar::<String>(
                &s,
                "SELECT payload_json FROM task_journal WHERE journal_kind='STEP_LEASE_RELEASED'"
            ),
            "{\"attempt\":1,\"generation\":1,\"result_digest\":null}"
        );
        let before = snapshot(&s);
        assert_eq!(
            s.transact_with_audit(&audit, |tx| {
                tx.release_audited(repeated, time(now + 1), &c.view())
            }),
            Err(StoreError::LeaseFenced)
        );
        assert_eq!(snapshot(&s), before);
        assert_eq!(audit.calls.load(Ordering::SeqCst), 1);
    }
}

#[test]
fn audited_release_stale_generation_never_dispatches_or_journals() {
    let s = memory();
    let stale = acquire(&s);
    let current = s
        .transact(|tx| acquire_at(tx, STEP, OWNER, Some(1), 20, 30))
        .unwrap();
    assert_eq!(current.generation(), 2);
    let before = snapshot(&s);
    let c = Context::new();
    let audit = Counting::new("ok");
    assert_eq!(
        s.transact_with_audit(&audit, |tx| tx.release_audited(stale, time(21), &c.view())),
        Err(StoreError::LeaseFenced)
    );
    assert_eq!(snapshot(&s), before);
    assert_eq!(audit.calls.load(Ordering::SeqCst), 0);
    assert_eq!(scalar::<i64>(&s, "SELECT count(*) FROM task_journal"), 0);
}

#[test]
fn audited_lease_mapper_errors_are_method_atomic() {
    let s = memory();
    let c = Context::new();
    let reject = Counting::new("reject");
    let before = snapshot(&s);
    s.transact_with_audit(&reject, |tx| {
        assert_eq!(
            tx.acquire_audited(
                TaskId::new(TASK).unwrap(),
                StepId::new(STEP).unwrap(),
                LeaseOwner::new(OWNER).unwrap(),
                None,
                time(10),
                time(20),
                &c.view()
            )
            .err(),
            Some(StoreError::AuditRejected)
        );
        Ok(())
    })
    .unwrap();
    assert_eq!(snapshot(&s), before);
    let g = acquire(&s);
    let before = snapshot(&s);
    s.transact_with_audit(&reject, |tx| {
        assert_eq!(
            tx.release_audited(g, time(12), &c.view()),
            Err(StoreError::AuditRejected)
        );
        Ok(())
    })
    .unwrap();
    assert_eq!(snapshot(&s), before);
}

#[test]
fn operation_savepoint_cleanup_failure_is_rollback_only() {
    let s = memory();
    let before = snapshot(&s);
    assert_eq!(
        s.transact_with_audit(&TestAudit, |tx| {
            tx.put_blob(b"1001", DataClass::Public)?;
            assert_eq!(
                tx.operation_savepoint(|inner| {
                    inner.put_blob(b"1002", DataClass::Public)?;
                    inner.inner.execute_batch("RELEASE serea_operation")?;
                    Err::<(), _>(StoreError::ConstraintViolation)
                }),
                Err(StoreError::Sqlite)
            );
            Ok(())
        }),
        Err(StoreError::Sqlite)
    );
    assert_eq!(snapshot(&s), before);
}

#[test]
fn facts_constructor_defaults_optional_fields_and_copies_context() {
    let c = Context::new();
    let task = TaskId::new(TASK).unwrap();
    let f = DurableTransition::task(
        AuditOperation::PlanningStarted,
        &task,
        Some(TaskState::Received),
        TaskState::Planning,
        DataClass::Personal,
        time(4),
        &c.view(),
    );
    assert_eq!(f.task_id(), &task);
    assert_eq!(f.operation(), AuditOperation::PlanningStarted);
    assert_eq!(f.task_from(), Some(TaskState::Received));
    assert_eq!(f.task_to(), TaskState::Planning);
    assert!(f.step_id().is_none());
    assert!(f.step_from().is_none());
    assert!(f.step_to().is_none());
    assert!(f.attempt().is_none());
    assert!(f.generation().is_none());
    assert!(f.result().is_none());
    assert!(f.revision().is_none());
    assert!(f.receipt_id().is_none());
    assert!(f.reason().is_none());
    assert_eq!(f.data_class(), DataClass::Personal);
    assert_eq!(f.now(), time(4));
    assert_eq!(f.actor_kind(), ActorKind::Host);
    assert_eq!(f.actor_id(), &c.actor);
    assert_eq!(f.actor_version(), &c.version);
    assert!(f.causation_id().is_none());
}
