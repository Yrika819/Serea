//! Only the frozen independent-review coverage gaps; no new features.
use super::*;

fn isolated_update(tx: &mut Tx<'_>, sql: &str, g: &LeaseGuard) -> usize {
    let mut statement = tx.inner.prepare(sql).unwrap();
    let values = [
        (":step", Value::Text(g.step_id.as_str().into())),
        (":task", Value::Text(g.task_id.as_str().into())),
        (":owner", Value::Text(g.owner.as_str().into())),
        (":generation", Value::Integer(i64::from(g.generation.get()))),
        (":pre_status", Value::Text("EXECUTING".into())),
        (":expected_task", Value::Text("EXECUTING".into())),
        (":now", Value::Integer(40)),
        (":digest", Value::Text(format!("sha256:{}", "c".repeat(64)))),
        (":kind", Value::Text("PROVIDER_ERROR".into())),
        (":code", Value::Text("KNOWN_FAILURE".into())),
        (":message", Value::Text("test diagnostic".into())),
        (":retryable", Value::Integer(0)),
        (":action", Value::Text("STOP".into())),
        (":details", Value::Null),
    ];
    for (name, value) in values {
        if let Some(index) = statement.parameter_index(name).unwrap() {
            statement.raw_bind_parameter(index, value).unwrap();
        }
    }
    statement.raw_execute().unwrap()
}
fn rollback_isolated(s: &Store, sql: &str, g: &LeaseGuard, expected: usize) {
    let before = snapshot(s);
    assert_eq!(
        s.transact_with_audit(&crate::audit::TestAudit, |tx| {
            assert_eq!(isolated_update(tx, sql, g), expected);
            Err::<(), _>(StoreError::Sqlite)
        }),
        Err(StoreError::Sqlite)
    );
    assert_eq!(snapshot(s), before);
}
#[test]
fn exact_production_success_and_failure_updates_fence_without_classification() {
    for failure in [false, true] {
        let sql = super::super::outcome::first_write_sql(failure);
        for case in [
            "current-expired",
            "same-owner-reclaim",
            "other-owner-reclaim",
            "released",
            "wrong-task",
            "wrong-step",
            "wrong-owner",
            "step-generation",
            "lease-generation",
            "missing-authority",
            "blocked-parent",
            "wrong-status",
            "unsucceeded-prerequisite",
        ] {
            let s = memory();
            let mut g = running(&s);
            match case {
                "same-owner-reclaim" | "other-owner-reclaim" => {
                    let next = s
                        .transact_with_audit(&crate::audit::TestAudit, |tx| {
                            acquire_at(
                                tx,
                                STEP,
                                if case == "same-owner-reclaim" {
                                    OWNER
                                } else {
                                    "worker-B"
                                },
                                Some(1),
                                20,
                                30,
                            )
                        })
                        .unwrap();
                    begin(&s, &next, 21).unwrap();
                }
                "released" => {
                    let release = duplicate(&g);
                    s.transact_with_audit(&crate::audit::TestAudit, |tx| {
                        tx.release_lease(release, time(12))
                    })
                    .unwrap();
                }
                "wrong-task" => g.task_id = TaskId::new(OTHER_TASK).unwrap(),
                "wrong-step" => g.step_id = StepId::new(NEXT_STEP).unwrap(),
                "wrong-owner" => g.owner = LeaseOwner::new("wrong-worker").unwrap(),
                "step-generation" => {
                    s.conn
                        .lock()
                        .unwrap()
                        .execute("UPDATE task_steps SET lease_generation=2", [])
                        .unwrap();
                }
                "lease-generation" => {
                    s.conn
                        .lock()
                        .unwrap()
                        .execute("UPDATE leases SET generation=2", [])
                        .unwrap();
                }
                "missing-authority" => {
                    s.conn
                        .lock()
                        .unwrap()
                        .execute("DELETE FROM leases", [])
                        .unwrap();
                }
                "blocked-parent" => {
                    s.conn
                        .lock()
                        .unwrap()
                        .execute("UPDATE tasks SET state='BLOCKED'", [])
                        .unwrap();
                }
                "wrong-status" => {
                    s.conn
                        .lock()
                        .unwrap()
                        .execute(
                            "UPDATE task_steps SET status='LEASED',started_at_ms=NULL",
                            [],
                        )
                        .unwrap();
                }
                "unsucceeded-prerequisite" => {
                    s.conn
                        .lock()
                        .unwrap()
                        .execute("UPDATE task_steps SET sequence=1", [])
                        .unwrap();
                    s.conn.lock().unwrap().execute("INSERT INTO task_steps(step_id,task_id,sequence,kind,status,input_digest) VALUES (?1,?2,0,'NOTIFY','PLANNED',?3)",params![NEXT_STEP,TASK,format!("sha256:{}","b".repeat(64))]).unwrap();
                }
                "current-expired" => {}
                _ => unreachable!(),
            }
            rollback_isolated(&s, &sql, &g, usize::from(case == "current-expired"));
        }
    }
}
#[test]
fn owner_only_update_mutant_is_killed_by_same_owner_new_generation_case() {
    for failure in [false, true] {
        let s = memory();
        let old = running(&s);
        let current = s
            .transact_with_audit(&crate::audit::TestAudit, |tx| {
                acquire_at(tx, STEP, OWNER, Some(1), 20, 30)
            })
            .unwrap();
        begin(&s, &current, 21).unwrap();
        let sql = super::super::outcome::first_write_sql(failure);
        rollback_isolated(&s, &sql, &old, 0);
        let weakened = sql
            .replace("AND lease_generation=:generation", "")
            .replace("AND l.generation=:generation", "");
        assert_ne!(weakened, sql);
        // Real mutation control: the same schema-valid stale case succeeds if
        // both generation predicates are removed, and fails with production SQL.
        rollback_isolated(&s, &weakened, &old, 1);
    }
}
fn poison_probe(mode: &str) {
    let s = memory();
    let g = running(&s);
    let later = duplicate(&g);
    let c = Context::new();
    let before = snapshot(&s);
    assert_eq!(
        s.transact_with_audit(&crate::audit::TestAudit, |tx| {
            tx.put_blob(b"1001", DataClass::Public)?;
            let result: Result<(), StoreError> = tx.probe_outcome_scope(|inner| {
                inner.put_blob(b"1002", DataClass::Public)?;
                inner.inner.execute_batch("RELEASE serea_outcome")?;
                match mode {
                    "cleanup-error" => Err(StoreError::ConstraintViolation),
                    "release-error" => Ok(()),
                    "panic-cleanup-error" => {
                        panic!("controlled method unwind after lost savepoint")
                    }
                    _ => unreachable!(),
                }
            });
            assert_eq!(result, Err(StoreError::Sqlite));
            assert!(!tx.inner.is_autocommit(), "outer SQLite Tx is still active");
            assert!(tx.rollback_only);
            assert_eq!(
                tx.begin_attempt(&later, time(12), &c.view()),
                Err(StoreError::Sqlite)
            );
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
                Some(StoreError::Sqlite)
            );
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
#[test]
fn active_cleanup_error_sets_rollback_only_and_blocks_new_p2f_operations() {
    poison_probe("cleanup-error");
}
#[test]
fn successful_method_release_error_sets_rollback_only_even_with_active_outer_tx() {
    poison_probe("release-error");
}
#[test]
fn caught_method_unwind_restores_method_writes_but_commits_unrelated_outer_work() {
    let s = memory();
    let before = snapshot(&s);
    s.transact_with_audit(&crate::audit::TestAudit, |tx| {
        tx.put_blob(b"1001", DataClass::Public)?;
        let unwind = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _: Result<(), StoreError> = tx.probe_outcome_scope(|inner| {
                inner.put_blob(b"1002", DataClass::Public)?;
                inner
                    .inner
                    .execute("UPDATE tasks SET title='temporary method write'", [])?;
                panic!("controlled method unwind");
            });
        }));
        assert!(unwind.is_err());
        assert!(!tx.rollback_only);
        assert!(!tx.inner.is_autocommit());
        tx.put_blob(b"1003", DataClass::Public)?;
        Ok(())
    })
    .unwrap();
    let after = snapshot(&s);
    for i in [0, 1, 2, 3, 5, 6, 7, 8, 9] {
        assert_eq!(after[i], before[i]);
    }
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
fn caught_method_unwind_failed_cleanup_keeps_active_outer_tx_rollback_only() {
    let s = memory();
    let g = running(&s);
    let before = snapshot(&s);
    let c = Context::new();
    assert_eq!(
        s.transact_with_audit(&crate::audit::TestAudit, |tx| {
            tx.put_blob(b"1001", DataClass::Public)?;
            let unwind = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let _: Result<(), StoreError> = tx.probe_outcome_scope(|inner| {
                    inner.put_blob(b"1002", DataClass::Public)?;
                    inner.inner.execute_batch("RELEASE serea_outcome")?;
                    panic!("controlled cleanup-failure unwind");
                });
            }));
            assert!(unwind.is_err());
            assert!(tx.rollback_only);
            assert!(!tx.inner.is_autocommit());
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
                Some(StoreError::Sqlite)
            );
            Ok(())
        }),
        Err(StoreError::Sqlite)
    );
    assert_eq!(snapshot(&s), before);
}
#[test]
fn corrupt_existing_result_blob_refuses_after_update_and_preserves_unrelated_work() {
    let s = memory();
    let g = running(&s);
    s.transact_with_audit(&crate::audit::TestAudit, |tx| {
        tx.put_blob(b"{\"ok\":true}", DataClass::Public)
    })
    .unwrap();
    s.conn
        .lock()
        .unwrap()
        .execute(
            "UPDATE blobs SET content=CAST('null' AS BLOB),size_bytes=4",
            [],
        )
        .unwrap();
    let before = snapshot(&s);
    let c = Context::new();
    let r = receipt();
    s.transact_with_audit(&crate::audit::TestAudit, |tx| {
        tx.put_blob(b"1001", DataClass::Public)?;
        assert_eq!(
            tx.commit_step_outcome(
                g,
                StepOutcome::Succeeded {
                    result_json: b"{\"ok\":true}",
                    receipt: Some(&r)
                },
                time(12),
                &c.view()
            )
            .err(),
            Some(StoreError::BlobCorrupt)
        );
        tx.put_blob(b"1002", DataClass::Public)?;
        Ok(())
    })
    .unwrap();
    let after = snapshot(&s);
    for i in [0, 1, 2, 3, 5, 6, 7, 8, 9] {
        assert_eq!(after[i], before[i]);
    }
    assert_eq!(after[4].len(), 3);
    assert_eq!(
        scalar::<i64>(
            &s,
            "SELECT count(*) FROM blobs WHERE content=CAST('null' AS BLOB)"
        ),
        1
    );
}
#[test]
fn reused_committed_result_blob_survives_late_outcome_failure() {
    let s = memory();
    let g = running(&s);
    let blob = s
        .transact_with_audit(&crate::audit::TestAudit, |tx| {
            tx.put_blob(b"{\"ok\":true}", DataClass::Public)
        })
        .unwrap();
    let before = snapshot(&s);
    s.conn.lock().unwrap().execute_batch("CREATE TEMP TRIGGER p2f_abort BEFORE INSERT ON side_effect_receipts BEGIN SELECT RAISE(ABORT,'private diagnostic'); END;").unwrap();
    let c = Context::new();
    let r = receipt();
    s.transact_with_audit(&crate::audit::TestAudit, |tx| {
        assert_eq!(
            tx.commit_step_outcome(
                g,
                StepOutcome::Succeeded {
                    result_json: b"{\"ok\":true}",
                    receipt: Some(&r)
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
    assert_eq!(snapshot(&s), before);
    assert_eq!(
        s.transact_with_audit(&crate::audit::TestAudit, |tx| tx.get_blob(&blob))
            .unwrap(),
        b"{\"ok\":true}"
    );
}
#[test]
fn result_blob_written_before_method_in_same_outer_tx_survives_caught_failure() {
    let s = memory();
    let g = running(&s);
    let before = snapshot(&s);
    s.conn.lock().unwrap().execute_batch("CREATE TEMP TRIGGER p2f_abort BEFORE INSERT ON side_effect_receipts BEGIN SELECT RAISE(ABORT,'private diagnostic'); END;").unwrap();
    let c = Context::new();
    let r = receipt();
    let blob = s
        .transact_with_audit(&crate::audit::TestAudit, |tx| {
            let blob = tx.put_blob(b"{\"ok\":true}", DataClass::Public)?;
            assert_eq!(
                tx.commit_step_outcome(
                    g,
                    StepOutcome::Succeeded {
                        result_json: b"{\"ok\":true}",
                        receipt: Some(&r)
                    },
                    time(12),
                    &c.view()
                )
                .err(),
                Some(StoreError::ConstraintViolation)
            );
            Ok(blob)
        })
        .unwrap();
    let after = snapshot(&s);
    for i in [0, 1, 2, 3, 5, 6, 7, 8, 9] {
        assert_eq!(after[i], before[i]);
    }
    assert_eq!(after[4].len(), 1);
    assert_eq!(
        s.transact_with_audit(&crate::audit::TestAudit, |tx| tx.get_blob(&blob))
            .unwrap(),
        b"{\"ok\":true}"
    );
}
#[test]
fn non_default_actor_version_causation_and_payload_identity_are_persisted() {
    let s = memory();
    let g = acquire(&s);
    let actor = ActorId::new("named-user").unwrap();
    let version = SemVer::new("1.2.3-alpha.1").unwrap();
    let cause = serea_protocol::EventId::new("evt_01JQ8Z9M3R2CVN8H5FWK7PQDSF").unwrap();
    let context = TransitionContext {
        actor_kind: ActorKind::User,
        actor_id: &actor,
        actor_version: &version,
        causation_id: Some(&cause),
    };
    let r = receipt();
    s.transact_with_audit(&crate::audit::TestAudit, |tx| {
        tx.begin_attempt(&g, time(11), &context)?;
        tx.commit_step_outcome(
            g,
            StepOutcome::Succeeded {
                result_json: b"{}",
                receipt: Some(&r),
            },
            time(12),
            &context,
        )
    })
    .unwrap();
    assert_eq!(
        scalar::<i64>(
            &s,
            "SELECT count(*) FROM task_journal WHERE actor_kind='USER' AND actor_id='named-user' AND actor_version='1.2.3-alpha.1' AND causation_id='evt_01JQ8Z9M3R2CVN8H5FWK7PQDSF'"
        ),
        5
    );
    let c = s.conn.lock().unwrap();
    let mut statement = c
        .prepare("SELECT payload_json,payload_digest FROM task_journal")
        .unwrap();
    for row in statement
        .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))
        .unwrap()
    {
        let (payload, digest) = row.unwrap();
        assert_eq!(
            serea_protocol::digest_of(&payload).unwrap().as_str(),
            digest
        );
    }
}
#[test]
fn begin_zero_row_and_journal_failure_preserve_before_and_after_outer_writes() {
    for (trigger, error) in [
        (
            "CREATE TEMP TRIGGER p2f_fail BEFORE UPDATE ON task_steps BEGIN SELECT RAISE(IGNORE); END;",
            StoreError::LeaseFenced,
        ),
        (
            "CREATE TEMP TRIGGER p2f_fail BEFORE UPDATE ON tasks BEGIN SELECT RAISE(IGNORE); END;",
            StoreError::LeaseFenced,
        ),
        (
            "CREATE TEMP TRIGGER p2f_fail BEFORE INSERT ON task_journal BEGIN SELECT RAISE(IGNORE); END;",
            StoreError::ConstraintViolation,
        ),
    ] {
        let s = memory();
        let g = acquire(&s);
        let before = snapshot(&s);
        s.conn.lock().unwrap().execute_batch(trigger).unwrap();
        let c = Context::new();
        s.transact_with_audit(&crate::audit::TestAudit, |tx| {
            tx.put_blob(b"1001", DataClass::Public)?;
            assert_eq!(tx.begin_attempt(&g, time(11), &c.view()), Err(error));
            tx.put_blob(b"1002", DataClass::Public)?;
            Ok(())
        })
        .unwrap();
        let after = snapshot(&s);
        for i in [0, 1, 2, 3, 5, 6, 7, 8, 9] {
            assert_eq!(after[i], before[i]);
        }
        assert_eq!(after[4].len(), 2);
    }
}
#[test]
fn known_final_failure_release_error_restores_full_error_task_journal_set() {
    let s = memory();
    let g = running(&s);
    let before = snapshot(&s);
    s.conn.lock().unwrap().execute_batch("CREATE TEMP TRIGGER p2f_fail BEFORE UPDATE ON leases BEGIN SELECT RAISE(ABORT,'private diagnostic'); END;").unwrap();
    s.transact_with_audit(&crate::audit::TestAudit, |tx| {
        tx.put_blob(b"1001", DataClass::Public)?;
        assert_eq!(
            failure_call(tx, g, ActionErrorKind::ProviderError, None).err(),
            Some(StoreError::ConstraintViolation)
        );
        tx.put_blob(b"1002", DataClass::Public)?;
        Ok(())
    })
    .unwrap();
    let after = snapshot(&s);
    for i in [0, 1, 2, 3, 5, 6, 7, 8, 9] {
        assert_eq!(after[i], before[i]);
    }
    assert_eq!(after[4].len(), 2);
}
#[test]
fn known_final_failure_actual_outer_commit_refusal_restores_all_facts() {
    let s = memory();
    let g = running(&s);
    deferred_tables(&s);
    let before = snapshot(&s);
    assert_eq!(
        s.transact_with_audit(&crate::audit::TestAudit, |tx| {
            failure_call(tx, g, ActionErrorKind::ProviderError, None)?;
            tx.inner.execute("INSERT INTO p2f_child VALUES(99)", [])?;
            Ok(())
        }),
        Err(StoreError::ConstraintViolation)
    );
    assert_eq!(snapshot(&s), before);
}
#[test]
fn known_final_failure_outer_panic_rolls_back_and_reopens_prior_authority() {
    let file = FileFixture::new();
    let s = file.open();
    fixture(&s);
    let g = running(&s);
    let before = snapshot(&s);
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = s.transact_with_audit(&crate::audit::TestAudit, |tx| {
                failure_call(tx, g, ActionErrorKind::ProviderError, None)?;
                panic!("controlled outer final-failure panic");
                #[allow(unreachable_code)]
                Ok(())
            });
        }))
        .is_err()
    );
    drop(s);
    let reopened = file.open();
    assert_eq!(snapshot(&reopened), before);
    reopened.verify_integrity().unwrap();
}
#[test]
fn future_receipt_observation_refuses_complete_outcome_and_preserves_outer_writes() {
    let s = memory();
    let g = running(&s);
    let before = snapshot(&s);
    let mut r = receipt();
    r.observed_at = Timestamp::from_epoch_millis(time(50));
    let c = Context::new();
    s.transact_with_audit(&crate::audit::TestAudit, |tx| {
        tx.put_blob(b"1001", DataClass::Public)?;
        assert_eq!(
            tx.commit_step_outcome(
                g,
                StepOutcome::Succeeded {
                    result_json: b"{}",
                    receipt: Some(&r)
                },
                time(12),
                &c.view()
            )
            .err(),
            Some(StoreError::InvalidLeaseInterval)
        );
        tx.put_blob(b"1002", DataClass::Public)?;
        Ok(())
    })
    .unwrap();
    let after = snapshot(&s);
    for i in [0, 1, 2, 3, 5, 6, 7, 8, 9] {
        assert_eq!(after[i], before[i]);
    }
    assert_eq!(after[4].len(), 2);
}
#[test]
fn ignored_fresh_result_blob_insert_is_refused_by_composite_reference_fk() {
    late_failure(
        Some(
            "CREATE TEMP TRIGGER p2f_ignore BEFORE INSERT ON blobs WHEN NEW.content=CAST('{\"ok\":true}' AS BLOB) BEGIN SELECT RAISE(IGNORE); END;",
        ),
        b"{\"ok\":true}",
        StoreError::ConstraintViolation,
    );
}
#[test]
fn interleaved_verification_layout_is_outside_supported_slice_not_auto_scheduled() {
    let s = memory();
    add_next(&s, "VERIFY");
    s.conn.lock().unwrap().execute("INSERT INTO task_steps(step_id,task_id,sequence,kind,status,input_digest) VALUES ('stp_01JQ8Z9M3R2CVN8H5FWK7PQDSH',?1,2,'NOTIFY','PLANNED',?2)",params![TASK,format!("sha256:{}","c".repeat(64))]).unwrap();
    let g = running(&s);
    assert_eq!(success(&s, g, 12).unwrap().task_state, TaskState::Ready);
    let verifier = s
        .transact_with_audit(&crate::audit::TestAudit, |tx| {
            acquire_at(tx, NEXT_STEP, OWNER, None, 13, 30)
        })
        .unwrap();
    let before = snapshot(&s);
    assert_eq!(begin(&s, &verifier, 14), Err(StoreError::LeaseFenced));
    assert_eq!(snapshot(&s), before);
}
