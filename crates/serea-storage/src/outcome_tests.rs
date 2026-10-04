//! Private, schema-valid P2F fixtures. All transitions use the real public API.
#[path = "outcome_review_tests.rs"]
mod review;
use super::LeaseGuard;
use crate::{StepFailure, StepOutcome, Store, StoreError, TransitionContext, Tx};
use rusqlite::{Connection, params, types::Value};
use serea_protocol::{
    ActionErrorKind, ActorId, ActorKind, CapabilityId, Clock, DataClass, EffectSummary,
    EpochMillis, ErrorCode, ErrorMessage, FailureReason, HostAction, IdempotencyKey, LeaseOwner,
    ProtocolError, ReceiptId, SemVer, SideEffectReceipt, StepId, TaskId, TaskState, Timestamp,
};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

const TASK: &str = "tsk_01JQ8Z9K3M7QWXR4V2T6YH0BNA";
const OTHER_TASK: &str = "tsk_01JQ8Z9K3M7QWXR4V2T6YH0BNB";
const STEP: &str = "stp_01JQ8Z9M3R2CVN8H5FWK7PQDSF";
const NEXT_STEP: &str = "stp_01JQ8Z9M3R2CVN8H5FWK7PQDSG";
const OWNER: &str = "worker-private-sentinel";
const CAP: &str = "calendar.events.list";
static NEXT: AtomicU64 = AtomicU64::new(0);
struct Fixed;
impl Clock for Fixed {
    fn now_ms(&self) -> Result<EpochMillis, ProtocolError> {
        EpochMillis::new(0)
    }
}
fn time(n: i64) -> EpochMillis {
    EpochMillis::new(n).unwrap()
}
fn key() -> IdempotencyKey {
    IdempotencyKey::new(format!("idk_{}", "a".repeat(64))).unwrap()
}
struct Context {
    actor: ActorId,
    version: SemVer,
}
impl Context {
    fn new() -> Self {
        Self {
            actor: ActorId::new("p2f-test-host").unwrap(),
            version: SemVer::new("0.2.0").unwrap(),
        }
    }
    fn view(&self) -> TransitionContext<'_> {
        TransitionContext {
            actor_kind: ActorKind::Host,
            actor_id: &self.actor,
            actor_version: &self.version,
            causation_id: None,
        }
    }
}
fn receipt() -> SideEffectReceipt {
    SideEffectReceipt {
        receipt_id: ReceiptId::new("rcp_01JQ8Z9M3R2CVN8H5FWK7PQDSF").unwrap(),
        capability_id: CapabilityId::new(CAP).unwrap(),
        idempotency_key: key(),
        provider_reference: None,
        effect_summary: EffectSummary::new("known effect").unwrap(),
        observed_at: Timestamp::from_epoch_millis(time(12)),
        replay_safe: true,
    }
}
fn fixture(store: &Store) {
    let c = store.conn.lock().unwrap();
    c.execute("INSERT INTO tasks(task_id,kind,title,state,origin_kind,data_class_rank,policy_class_rank,created_at_ms,updated_at_ms,max_model_calls,max_tool_calls,max_attempts_per_step) VALUES (?1,'USER_REQUEST','outcome fixture','READY','USER_MESSAGE',0,0,0,0,0,0,3)", [TASK]).unwrap();
    c.execute("INSERT INTO task_steps(step_id,task_id,sequence,kind,status,input_digest,provider_id,capability_id,capability_version,idempotency_key) VALUES (?1,?2,0,'CAPABILITY','PLANNED',?3,'calendar',?4,'1.0.0',?5)", params![STEP,TASK,format!("sha256:{}", "a".repeat(64)), CAP,key().as_str()]).unwrap();
}
fn memory() -> Store {
    let s = Store::open_in_memory(&Fixed).unwrap();
    fixture(&s);
    s
}
fn acquire_at(
    tx: &mut Tx<'_>,
    id: &str,
    who: &str,
    generation: Option<u32>,
    now: i64,
    expiry: i64,
) -> Result<LeaseGuard, StoreError> {
    tx.acquire_lease(
        TaskId::new(TASK).unwrap(),
        StepId::new(id).unwrap(),
        LeaseOwner::new(who).unwrap(),
        generation,
        time(now),
        time(expiry),
    )
}
fn acquire(store: &Store) -> LeaseGuard {
    store
        .transact(|tx| acquire_at(tx, STEP, OWNER, None, 10, 20))
        .unwrap()
}
fn begin(store: &Store, g: &LeaseGuard, now: i64) -> Result<(), StoreError> {
    let c = Context::new();
    store.transact(|tx| tx.begin_attempt(g, time(now), &c.view()))
}
fn running(store: &Store) -> LeaseGuard {
    let g = acquire(store);
    begin(store, &g, 11).unwrap();
    g
}
fn success(store: &Store, g: LeaseGuard, now: i64) -> Result<crate::StepCommit, StoreError> {
    let c = Context::new();
    let r = receipt();
    store.transact(|tx| {
        tx.commit_step_outcome(
            g,
            StepOutcome::Succeeded {
                result_json: b"{\"ok\":true}",
                receipt: Some(&r),
            },
            time(now),
            &c.view(),
        )
    })
}
// Deliberate private duplicate only for terminal/repeated-use simulations.
// Public LeaseGuard remains nonclone and nonconstructible.
fn duplicate(g: &LeaseGuard) -> LeaseGuard {
    LeaseGuard {
        task_id: g.task_id.clone(),
        step_id: g.step_id.clone(),
        owner: g.owner.clone(),
        generation: g.generation,
        origin: g.origin.clone(),
    }
}
fn rows(c: &Connection, sql: &str) -> Vec<Vec<Value>> {
    let mut s = c.prepare(sql).unwrap();
    let n = s.column_count();
    s.query_map([], |r| (0..n).map(|i| r.get(i)).collect())
        .unwrap()
        .map(Result::unwrap)
        .collect()
}
fn snapshot(store: &Store) -> Vec<Vec<Vec<Value>>> {
    let c = store.conn.lock().unwrap();
    [
        "schema_migrations",
        "tasks",
        "task_steps",
        "leases",
        "blobs",
        "side_effect_receipts",
        "plan_revisions",
        "task_blob_refs",
        "step_blob_refs",
        "task_journal",
    ]
    .iter()
    .map(|t| rows(&c, &format!("SELECT * FROM {t} ORDER BY 1")))
    .collect()
}
fn scalar<T: rusqlite::types::FromSql>(store: &Store, sql: &str) -> T {
    store
        .conn
        .lock()
        .unwrap()
        .query_row(sql, [], |r| r.get(0))
        .unwrap()
}
fn add_next(store: &Store, kind: &str) {
    let c = store.conn.lock().unwrap();
    if kind == "VERIFY" {
        c.execute("INSERT INTO task_steps(step_id,task_id,sequence,kind,status,input_digest,provider_id,capability_id,capability_version,idempotency_key) VALUES (?1,?2,1,'VERIFY','PLANNED',?3,'calendar',?4,'1.0.0',?5)",params![NEXT_STEP,TASK,format!("sha256:{}","b".repeat(64)),CAP,format!("idk_{}","b".repeat(64))]).unwrap();
    } else {
        c.execute("INSERT INTO task_steps(step_id,task_id,sequence,kind,status,input_digest) VALUES (?1,?2,1,?3,'PLANNED',?4)",params![NEXT_STEP,TASK,kind,format!("sha256:{}","b".repeat(64))]).unwrap();
    }
}
fn fenced_unchanged(store: &Store, g: LeaseGuard) {
    let before = snapshot(store);
    assert_eq!(success(store, g, 30).err(), Some(StoreError::LeaseFenced));
    assert_eq!(snapshot(store), before);
}

#[test]
fn current_guard_commits_whole_atomic_set_and_no_second_charge() {
    let s = memory();
    let g = running(&s);
    let out = success(&s, g, 12).unwrap();
    assert_eq!(out.step_status.as_str(), "SUCCEEDED");
    assert_eq!(out.task_state, TaskState::Verifying);
    assert_eq!((out.attempt, out.generation), (1, 1));
    let blob = out.result.unwrap();
    assert_eq!(
        s.transact(|tx| tx.get_blob(&blob)).unwrap(),
        b"{\"ok\":true}"
    );
    assert_eq!(
        scalar::<i64>(&s, "SELECT count(*) FROM side_effect_receipts"),
        1
    );
    assert_eq!(
        scalar::<i64>(
            &s,
            "SELECT count(*) FROM step_blob_refs WHERE role='RESULT'"
        ),
        1
    );
    assert_eq!(scalar::<i64>(&s, "SELECT released_at_ms FROM leases"), 12);
    assert_eq!(
        scalar::<i64>(
            &s,
            "SELECT count(*) FROM task_steps WHERE lease_owner IS NULL AND lease_expires_at_ms IS NULL AND lease_generation=1 AND started_at_ms=11 AND completed_at_ms=12 AND attempt=1"
        ),
        1
    );
    assert_eq!(
        rows(
            &s.conn.lock().unwrap(),
            "SELECT journal_seq,journal_kind FROM task_journal ORDER BY journal_seq"
        ),
        vec![
            vec![
                Value::Integer(1),
                Value::Text("STEP_ATTEMPT_STARTED".into())
            ],
            vec![Value::Integer(2), Value::Text("TASK_STATE_CHANGED".into())],
            vec![Value::Integer(3), Value::Text("STEP_COMMITTED".into())],
            vec![Value::Integer(4), Value::Text("RECEIPT_RECORDED".into())],
            vec![Value::Integer(5), Value::Text("TASK_STATE_CHANGED".into())]
        ]
    );
    assert_eq!(
        scalar::<i64>(
            &s,
            "SELECT count(*) FROM task_journal WHERE payload_json LIKE '%\"generation\":1%' AND attempt=1 AND actor_id='p2f-test-host'"
        ),
        5
    );
}
#[test]
fn attempt_remains_one_across_acquire_and_begin() {
    let s = memory();
    let g = acquire(&s);
    assert_eq!(scalar::<i64>(&s, "SELECT attempt FROM task_steps"), 1);
    begin(&s, &g, 11).unwrap();
    assert_eq!(scalar::<i64>(&s, "SELECT attempt FROM task_steps"), 1);
    let before = snapshot(&s);
    assert_eq!(begin(&s, &g, 12), Err(StoreError::LeaseFenced));
    assert_eq!(snapshot(&s), before);
}
#[test]
fn exact_expiry_current_known_outcome_can_commit_receipt_and_journal() {
    let s = memory();
    let g = running(&s);
    success(&s, g, 20).unwrap();
    assert_eq!(
        scalar::<i64>(&s, "SELECT count(*) FROM side_effect_receipts"),
        1
    );
}
#[test]
fn past_expiry_current_known_outcome_can_commit_receipt_and_journal() {
    let s = memory();
    let g = running(&s);
    success(&s, g, 21).unwrap();
    assert_eq!(scalar::<i64>(&s, "SELECT released_at_ms FROM leases"), 21);
}
#[test]
fn exact_expiry_begin_is_lease_expired_without_charge_or_mutation() {
    let s = memory();
    let g = acquire(&s);
    let before = snapshot(&s);
    assert_eq!(begin(&s, &g, 20), Err(StoreError::LeaseExpired));
    assert_eq!(snapshot(&s), before);
}
#[test]
fn past_expiry_begin_is_lease_expired_without_charge_or_mutation() {
    let s = memory();
    let g = acquire(&s);
    let before = snapshot(&s);
    assert_eq!(begin(&s, &g, 21), Err(StoreError::LeaseExpired));
    assert_eq!(snapshot(&s), before);
}
#[test]
fn expiry_does_not_generalize_to_renewal() {
    let s = memory();
    let g = running(&s);
    assert_eq!(
        s.transact(|tx| tx.renew_lease(&g, time(20), time(30))),
        Err(StoreError::LeaseExpired)
    );
    success(&s, g, 21).unwrap();
}
#[test]
fn expired_old_guard_is_fenced_after_committed_different_owner_reclaim() {
    let s = memory();
    let old = running(&s);
    let g = s
        .transact(|tx| acquire_at(tx, STEP, "worker-B", Some(1), 20, 30))
        .unwrap();
    fenced_unchanged(&s, old);
    begin(&s, &g, 21).unwrap();
    let out = success(&s, g, 31).unwrap();
    assert_eq!((out.attempt, out.generation), (2, 2));
}
#[test]
fn same_owner_reclaim_fences_old_generation_without_receipt_journal_or_task_change() {
    let s = memory();
    let old = running(&s);
    let g = s
        .transact(|tx| acquire_at(tx, STEP, OWNER, Some(1), 20, 30))
        .unwrap();
    fenced_unchanged(&s, old);
    begin(&s, &g, 21).unwrap();
    success(&s, g, 22).unwrap();
}
#[test]
fn committed_release_fences_outcome_with_unchanged_step_copy() {
    let s = memory();
    let g = running(&s);
    let old = duplicate(&g);
    s.transact(|tx| tx.release_lease(g, time(12))).unwrap();
    fenced_unchanged(&s, old);
}
#[test]
fn duplicate_terminal_commit_cannot_manufacture_another_receipt() {
    let s = memory();
    let g = running(&s);
    let second = duplicate(&g);
    success(&s, g, 12).unwrap();
    fenced_unchanged(&s, second);
    assert_eq!(
        scalar::<i64>(&s, "SELECT count(*) FROM side_effect_receipts"),
        1
    );
}
#[test]
fn wrong_owner_and_task_binding_cannot_commit() {
    for wrong_task in [false, true] {
        let s = memory();
        let g = running(&s);
        let mut wrong = duplicate(&g);
        if wrong_task {
            wrong.task_id = TaskId::new(OTHER_TASK).unwrap();
        } else {
            wrong.owner = LeaseOwner::new("wrong-owner").unwrap();
        }
        fenced_unchanged(&s, wrong);
        success(&s, g, 12).unwrap();
    }
}
#[test]
fn step_copy_cannot_replace_authoritative_generation() {
    let s = memory();
    let g = running(&s);
    s.conn
        .lock()
        .unwrap()
        .execute("UPDATE leases SET generation=2", [])
        .unwrap();
    fenced_unchanged(&s, g);
}
#[test]
fn durable_authority_cannot_replace_step_generation() {
    let s = memory();
    let g = running(&s);
    s.conn
        .lock()
        .unwrap()
        .execute("UPDATE task_steps SET lease_generation=2", [])
        .unwrap();
    fenced_unchanged(&s, g);
}
#[test]
fn outcome_requires_executing_lifecycle_even_for_current_guard() {
    let s = memory();
    let g = acquire(&s);
    fenced_unchanged(&s, g);
}
#[test]
fn terminal_or_blocked_task_refuses_current_outcome() {
    for state in ["COMPLETED", "BLOCKED"] {
        let s = memory();
        let g = running(&s);
        s.conn
            .lock()
            .unwrap()
            .execute("UPDATE tasks SET state=?1", [state])
            .unwrap();
        fenced_unchanged(&s, g);
    }
}
#[test]
fn stale_and_expired_invalid_data_precedence_is_lease_fenced() {
    let s = memory();
    let old = running(&s);
    drop(
        s.transact(|tx| acquire_at(tx, STEP, OWNER, Some(1), 20, 30))
            .unwrap(),
    );
    let c = Context::new();
    let before = snapshot(&s);
    assert_eq!(
        s.transact(|tx| tx.commit_step_outcome(
            old,
            StepOutcome::Succeeded {
                result_json: b"bad",
                receipt: None
            },
            time(100),
            &c.view()
        ))
        .err(),
        Some(StoreError::LeaseFenced)
    );
    assert_eq!(snapshot(&s), before);
}
#[test]
fn rolled_back_initial_acquisition_origin_cannot_alias_same_owner_generation() {
    let s = memory();
    let mut old = None;
    assert_eq!(
        s.transact(|tx| {
            old = Some(acquire_at(tx, STEP, OWNER, None, 10, 20)?);
            Err::<(), _>(StoreError::Sqlite)
        }),
        Err(StoreError::Sqlite)
    );
    let g = running(&s);
    fenced_unchanged(&s, old.unwrap());
    success(&s, g, 12).unwrap();
}
#[test]
fn rolled_back_reclaim_origin_cannot_alias_same_owner_new_generation() {
    let s = memory();
    drop(running(&s));
    let mut old = None;
    assert_eq!(
        s.transact(|tx| {
            old = Some(acquire_at(tx, STEP, OWNER, Some(1), 20, 30)?);
            Err::<(), _>(StoreError::Sqlite)
        }),
        Err(StoreError::Sqlite)
    );
    let g = s
        .transact(|tx| acquire_at(tx, STEP, OWNER, Some(1), 20, 30))
        .unwrap();
    begin(&s, &g, 21).unwrap();
    fenced_unchanged(&s, old.unwrap());
    success(&s, g, 22).unwrap();
}
#[test]
fn pending_guard_can_begin_and_commit_only_in_origin_transaction() {
    let s = memory();
    let c = Context::new();
    let r = receipt();
    let out = s
        .transact(|tx| {
            let g = acquire_at(tx, STEP, OWNER, None, 10, 20)?;
            tx.begin_attempt(&g, time(11), &c.view())?;
            tx.commit_step_outcome(
                g,
                StepOutcome::Succeeded {
                    result_json: b"{}",
                    receipt: Some(&r),
                },
                time(21),
                &c.view(),
            )
        })
        .unwrap();
    assert_eq!(out.attempt, 1);
}
#[test]
fn outer_outcome_rollback_restores_all_durable_facts_and_original_lease() {
    let s = memory();
    let g = running(&s);
    let probe = duplicate(&g);
    let before = snapshot(&s);
    let c = Context::new();
    let r = receipt();
    assert_eq!(
        s.transact(|tx| {
            tx.commit_step_outcome(
                g,
                StepOutcome::Succeeded {
                    result_json: b"{}",
                    receipt: Some(&r),
                },
                time(12),
                &c.view(),
            )?;
            Err::<(), _>(StoreError::Sqlite)
        }),
        Err(StoreError::Sqlite)
    );
    assert_eq!(snapshot(&s), before);
    // Only a test-private duplicate remains. The consumed public guard is gone.
    s.transact(|tx| tx.renew_lease(&probe, time(13), time(30)))
        .unwrap();
}
#[test]
fn next_ordinary_step_makes_task_ready_without_scheduling() {
    let s = memory();
    add_next(&s, "NOTIFY");
    let g = running(&s);
    assert_eq!(success(&s, g, 12).unwrap().task_state, TaskState::Ready);
    assert_eq!(
        scalar::<String>(&s, "SELECT status FROM task_steps WHERE sequence=1"),
        "PLANNED"
    );
}
#[test]
fn verifier_becomes_eligible_then_completes_task_atomically() {
    let s = memory();
    add_next(&s, "VERIFY");
    let g = running(&s);
    assert_eq!(success(&s, g, 12).unwrap().task_state, TaskState::Verifying);
    let g = s
        .transact(|tx| acquire_at(tx, NEXT_STEP, OWNER, None, 13, 20))
        .unwrap();
    begin(&s, &g, 14).unwrap();
    let c = Context::new();
    let out = s
        .transact(|tx| {
            tx.commit_step_outcome(
                g,
                StepOutcome::Succeeded {
                    result_json: b"true",
                    receipt: None,
                },
                time(21),
                &c.view(),
            )
        })
        .unwrap();
    assert_eq!(out.task_state, TaskState::Completed);
    assert_eq!(
        scalar::<i64>(
            &s,
            "SELECT count(*) FROM task_journal WHERE journal_kind='TASK_TERMINAL'"
        ),
        1
    );
}
#[test]
fn verify_cannot_begin_before_verifying_task_state() {
    let s = memory();
    s.conn
        .lock()
        .unwrap()
        .execute("UPDATE task_steps SET kind='VERIFY'", [])
        .unwrap();
    let g = acquire(&s);
    let before = snapshot(&s);
    assert_eq!(begin(&s, &g, 11), Err(StoreError::LeaseFenced));
    assert_eq!(snapshot(&s), before);
}
#[test]
fn out_of_sequence_begin_refuses_without_second_charge() {
    let s = memory();
    add_next(&s, "NOTIFY");
    let g = s
        .transact(|tx| acquire_at(tx, NEXT_STEP, OWNER, None, 10, 20))
        .unwrap();
    let before = snapshot(&s);
    assert_eq!(begin(&s, &g, 11), Err(StoreError::LeaseFenced));
    assert_eq!(snapshot(&s), before);
}
#[test]
fn non_effect_success_has_result_and_no_receipt() {
    let s = memory();
    s.conn.lock().unwrap().execute_batch("DELETE FROM task_steps; INSERT INTO task_steps(step_id,task_id,sequence,kind,status,input_digest) VALUES ('stp_01JQ8Z9M3R2CVN8H5FWK7PQDSF','tsk_01JQ8Z9K3M7QWXR4V2T6YH0BNA',0,'NOTIFY','PLANNED','sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa');").unwrap();
    let g = running(&s);
    let c = Context::new();
    s.transact(|tx| {
        tx.commit_step_outcome(
            g,
            StepOutcome::Succeeded {
                result_json: b"{}",
                receipt: None,
            },
            time(12),
            &c.view(),
        )
    })
    .unwrap();
    assert_eq!(
        scalar::<i64>(&s, "SELECT count(*) FROM side_effect_receipts"),
        0
    );
    assert_eq!(
        scalar::<i64>(
            &s,
            "SELECT count(*) FROM task_journal WHERE journal_kind='RECEIPT_RECORDED'"
        ),
        0
    );
}
fn failure_call(
    tx: &mut Tx<'_>,
    g: LeaseGuard,
    kind: ActionErrorKind,
    details: Option<&[u8]>,
) -> Result<crate::StepCommit, StoreError> {
    let code = ErrorCode::new("KNOWN_FAILURE").unwrap();
    let message = ErrorMessage::new("known diagnostic sentinel").unwrap();
    let action = HostAction::new("STOP").unwrap();
    let reason = FailureReason::new("KNOWN_FAILURE").unwrap();
    let c = Context::new();
    tx.commit_step_outcome(
        g,
        StepOutcome::Failed(StepFailure {
            kind,
            code: &code,
            message: &message,
            retryable: false,
            host_action: &action,
            details_json: details,
            failure_reason: &reason,
        }),
        time(12),
        &c.view(),
    )
}
#[test]
fn known_final_failure_commits_complete_error_task_journal_and_release() {
    let s = memory();
    let g = running(&s);
    let out = s
        .transact(|tx| {
            failure_call(
                tx,
                g,
                ActionErrorKind::ProviderError,
                Some(b"{\"reason\":1}"),
            )
        })
        .unwrap();
    assert_eq!(out.step_status.as_str(), "FAILED");
    assert_eq!(out.task_state, TaskState::Failed);
    assert!(out.result.is_none());
    assert_eq!(scalar::<i64>(&s, "SELECT attempt FROM task_steps"), 1);
    assert_eq!(
        scalar::<String>(&s, "SELECT error_details FROM task_steps"),
        "{\"reason\":1}"
    );
    assert_eq!(
        scalar::<String>(&s, "SELECT failure_reason FROM tasks"),
        "KNOWN_FAILURE"
    );
    assert_eq!(scalar::<i64>(&s, "SELECT count(*) FROM blobs"), 0);
    assert_eq!(
        scalar::<i64>(&s, "SELECT count(*) FROM side_effect_receipts"),
        0
    );
    assert_eq!(
        scalar::<i64>(
            &s,
            "SELECT count(*) FROM task_journal WHERE journal_kind IN ('STEP_FAILED','TASK_STATE_CHANGED','TASK_TERMINAL')"
        ),
        4
    );
    assert_eq!(scalar::<i64>(&s, "SELECT released_at_ms FROM leases"), 12);
}
#[test]
fn ambiguous_provider_effect_is_not_guessed_as_known_final_failure() {
    let s = memory();
    let g = running(&s);
    let before = snapshot(&s);
    assert_eq!(
        s.transact(|tx| failure_call(tx, g, ActionErrorKind::Ambiguous, None))
            .err(),
        Some(StoreError::ConstraintViolation)
    );
    assert_eq!(snapshot(&s), before);
}
#[test]
fn non_object_error_details_are_refused_atomically() {
    let s = memory();
    let g = running(&s);
    let before = snapshot(&s);
    assert_eq!(
        s.transact(|tx| failure_call(tx, g, ActionErrorKind::ProviderError, Some(b"[]")))
            .err(),
        Some(StoreError::CanonicalJson)
    );
    assert_eq!(snapshot(&s), before);
}
#[test]
fn receipt_capability_and_idempotency_must_match_durable_step() {
    for wrong_key in [false, true] {
        let s = memory();
        let g = running(&s);
        let mut r = receipt();
        if wrong_key {
            r.idempotency_key = IdempotencyKey::new(format!("idk_{}", "b".repeat(64))).unwrap();
        } else {
            r.capability_id = CapabilityId::new("calendar.events.create").unwrap();
        }
        let c = Context::new();
        let before = snapshot(&s);
        assert_eq!(
            s.transact(|tx| tx.commit_step_outcome(
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
        assert_eq!(snapshot(&s), before);
    }
}
#[test]
fn malformed_result_caught_error_preserves_outer_unrelated_work() {
    late_failure(None, b"not JSON", StoreError::CanonicalJson);
}
fn late_failure(trigger: Option<&str>, bytes: &[u8], expected: StoreError) {
    let s = memory();
    let g = running(&s);
    if let Some(sql) = trigger {
        s.conn.lock().unwrap().execute_batch(sql).unwrap();
    }
    let before = snapshot(&s);
    let c = Context::new();
    let r = receipt();
    let refs = s
        .transact(|tx| {
            let a = tx.put_blob(b"1001", DataClass::Public)?;
            assert_eq!(
                tx.commit_step_outcome(
                    g,
                    StepOutcome::Succeeded {
                        result_json: bytes,
                        receipt: Some(&r)
                    },
                    time(12),
                    &c.view()
                )
                .err(),
                Some(expected)
            );
            let b = tx.put_blob(b"1002", DataClass::Public)?;
            Ok((a, b))
        })
        .unwrap();
    let after = snapshot(&s);
    for index in [0, 1, 2, 3, 5, 6, 7, 8, 9] {
        assert_eq!(
            after[index], before[index],
            "partial authoritative writes at table index {index}"
        );
    }
    assert_eq!(after[4].len(), before[4].len() + 2);
    assert_eq!(s.transact(|tx| tx.get_blob(&refs.0)).unwrap(), b"1001");
    assert_eq!(s.transact(|tx| tx.get_blob(&refs.1)).unwrap(), b"1002");
}
macro_rules! abort_case {
    ($name:ident,$timing:literal,$event:literal,$table:literal,$condition:literal) => {
        #[test]
        fn $name() {
            late_failure(
                Some(concat!(
                    "CREATE TEMP TRIGGER p2f_abort ",
                    $timing,
                    " ",
                    $event,
                    " ON ",
                    $table,
                    " WHEN ",
                    $condition,
                    " BEGIN SELECT RAISE(ABORT,'private diagnostic'); END;"
                )),
                b"{\"ok\":true}",
                StoreError::ConstraintViolation,
            );
        }
    };
}
abort_case!(
    caught_failure_after_fenced_step_update,
    "AFTER",
    "UPDATE",
    "task_steps",
    "NEW.status='SUCCEEDED'"
);
abort_case!(
    caught_failure_before_result_blob_insert,
    "BEFORE",
    "INSERT",
    "blobs",
    "NEW.content=CAST('{\"ok\":true}' AS BLOB)"
);
abort_case!(
    caught_failure_after_result_blob_insert,
    "AFTER",
    "INSERT",
    "blobs",
    "NEW.content=CAST('{\"ok\":true}' AS BLOB)"
);
abort_case!(
    caught_failure_before_result_reference_insert,
    "BEFORE",
    "INSERT",
    "step_blob_refs",
    "NEW.role='RESULT'"
);
abort_case!(
    caught_failure_after_result_reference_insert,
    "AFTER",
    "INSERT",
    "step_blob_refs",
    "NEW.role='RESULT'"
);
abort_case!(
    caught_failure_before_receipt_insert,
    "BEFORE",
    "INSERT",
    "side_effect_receipts",
    "1"
);
abort_case!(
    caught_failure_after_receipt_insert,
    "AFTER",
    "INSERT",
    "side_effect_receipts",
    "1"
);
abort_case!(
    caught_failure_after_task_transition,
    "AFTER",
    "UPDATE",
    "tasks",
    "NEW.state='VERIFYING'"
);
abort_case!(
    caught_failure_before_journal_insert,
    "BEFORE",
    "INSERT",
    "task_journal",
    "NEW.journal_kind='STEP_COMMITTED'"
);
abort_case!(
    caught_failure_after_receipt_journal_insert,
    "AFTER",
    "INSERT",
    "task_journal",
    "NEW.journal_kind='RECEIPT_RECORDED'"
);
abort_case!(
    caught_failure_before_lease_finalization,
    "BEFORE",
    "UPDATE",
    "leases",
    "NEW.released_at_ms IS NOT NULL"
);
abort_case!(
    caught_failure_after_lease_finalization,
    "AFTER",
    "UPDATE",
    "leases",
    "NEW.released_at_ms IS NOT NULL"
);
#[test]
fn ignored_fenced_step_update_does_not_continue_to_receipt_or_journal() {
    late_failure(
        Some(
            "CREATE TEMP TRIGGER p2f_ignore BEFORE UPDATE ON task_steps WHEN NEW.status='SUCCEEDED' BEGIN SELECT RAISE(IGNORE); END;",
        ),
        b"{}",
        StoreError::LeaseFenced,
    );
}
#[test]
fn ignored_task_update_rolls_back_preceding_outcome_and_receipt() {
    late_failure(
        Some(
            "CREATE TEMP TRIGGER p2f_ignore BEFORE UPDATE ON tasks WHEN NEW.state='VERIFYING' BEGIN SELECT RAISE(IGNORE); END;",
        ),
        b"{}",
        StoreError::LeaseFenced,
    );
}
#[test]
fn ignored_receipt_insert_cannot_manufacture_success_without_receipt() {
    late_failure(
        Some(
            "CREATE TEMP TRIGGER p2f_ignore BEFORE INSERT ON side_effect_receipts BEGIN SELECT RAISE(IGNORE); END;",
        ),
        b"{}",
        StoreError::ConstraintViolation,
    );
}
#[test]
fn ignored_journal_insert_cannot_manufacture_success_without_journal() {
    late_failure(
        Some(
            "CREATE TEMP TRIGGER p2f_ignore BEFORE INSERT ON task_journal WHEN NEW.journal_kind='STEP_COMMITTED' BEGIN SELECT RAISE(IGNORE); END;",
        ),
        b"{}",
        StoreError::ConstraintViolation,
    );
}
#[test]
fn failed_savepoint_cleanup_makes_outer_transaction_rollback_only() {
    let s = memory();
    let g = running(&s);
    let c = Context::new();
    let before = snapshot(&s);
    s.conn.lock().unwrap().execute_batch("CREATE TEMP TRIGGER p2f_rollback BEFORE INSERT ON side_effect_receipts BEGIN SELECT RAISE(ROLLBACK,'private diagnostic'); END;").unwrap();
    let r = receipt();
    assert_eq!(
        s.transact(|tx| {
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
                Some(StoreError::Sqlite)
            );
            assert_eq!(
                tx.put_blob(b"1002", DataClass::Public).err(),
                Some(StoreError::Sqlite)
            );
            Ok(())
        }),
        Err(StoreError::Sqlite)
    );
    assert_eq!(snapshot(&s), before);
}
#[test]
fn begin_late_failure_is_method_atomic_and_borrowed_guard_remains_usable() {
    let s = memory();
    let g = acquire(&s);
    let before = snapshot(&s);
    s.conn.lock().unwrap().execute_batch("CREATE TEMP TRIGGER p2f_abort BEFORE INSERT ON task_journal BEGIN SELECT RAISE(ABORT,'private diagnostic'); END;").unwrap();
    let c = Context::new();
    s.transact(|tx| {
        assert_eq!(
            tx.begin_attempt(&g, time(11), &c.view()),
            Err(StoreError::ConstraintViolation)
        );
        Ok(())
    })
    .unwrap();
    assert_eq!(snapshot(&s), before);
    s.conn
        .lock()
        .unwrap()
        .execute_batch("DROP TRIGGER p2f_abort")
        .unwrap();
    begin(&s, &g, 12).unwrap();
    success(&s, g, 13).unwrap();
}
#[test]
fn backward_begin_and_outcome_instants_are_refused_without_mutation() {
    let s = memory();
    let g = acquire(&s);
    let before = snapshot(&s);
    assert_eq!(begin(&s, &g, 9), Err(StoreError::InvalidLeaseInterval));
    assert_eq!(snapshot(&s), before);
    begin(&s, &g, 11).unwrap();
    let before = snapshot(&s);
    assert_eq!(
        success(&s, g, 10).err(),
        Some(StoreError::InvalidLeaseInterval)
    );
    assert_eq!(snapshot(&s), before);
}
#[test]
fn private_task_outcome_refuses_even_with_blob_backend() {
    struct Backend;
    impl crate::AtRestProtection for Backend {
        fn protect(&self, _: &[u8]) -> Result<Vec<u8>, crate::AtRestProtectionError> {
            panic!("ordinary PRIVATE writer must not call backend")
        }
        fn unprotect(&self, _: &[u8]) -> Result<Vec<u8>, crate::AtRestProtectionError> {
            panic!("ordinary PRIVATE writer must not call backend")
        }
    }
    let s = Store::open_in_memory_with_protection(&Fixed, std::sync::Arc::new(Backend)).unwrap();
    fixture(&s);
    let g = running(&s);
    s.conn
        .lock()
        .unwrap()
        .execute("UPDATE tasks SET data_class_rank=2", [])
        .unwrap();
    let before = snapshot(&s);
    assert_eq!(
        success(&s, g, 12).err(),
        Some(StoreError::AtRestProtectionUnavailable)
    );
    assert_eq!(snapshot(&s), before);
}
#[test]
fn retry_reclaim_spends_new_charge_and_ceiling_remains_acquisition_only() {
    let s = memory();
    s.conn
        .lock()
        .unwrap()
        .execute("UPDATE tasks SET max_attempts_per_step=1", [])
        .unwrap();
    let g = running(&s);
    assert_eq!(
        s.transact(|tx| acquire_at(tx, STEP, OWNER, Some(1), 20, 30))
            .err(),
        Some(StoreError::AttemptCeilingReached)
    );
    let out = success(&s, g, 21).unwrap();
    assert_eq!(out.attempt, 1);
}
fn deferred_tables(s: &Store) {
    s.conn.lock().unwrap().execute_batch("CREATE TABLE p2f_parent(id INTEGER PRIMARY KEY); CREATE TABLE p2f_child(id INTEGER REFERENCES p2f_parent(id) DEFERRABLE INITIALLY DEFERRED);").unwrap();
}
#[test]
fn actual_outer_commit_failure_rolls_back_success_receipt_journal_and_release() {
    let s = memory();
    let g = running(&s);
    let probe = duplicate(&g);
    deferred_tables(&s);
    let before = snapshot(&s);
    let c = Context::new();
    let r = receipt();
    assert_eq!(
        s.transact(|tx| {
            tx.commit_step_outcome(
                g,
                StepOutcome::Succeeded {
                    result_json: b"{}",
                    receipt: Some(&r),
                },
                time(12),
                &c.view(),
            )?;
            tx.inner.execute("INSERT INTO p2f_child VALUES(99)", [])?;
            Ok(())
        }),
        Err(StoreError::ConstraintViolation)
    );
    assert_eq!(snapshot(&s), before);
    assert_eq!(scalar::<i64>(&s, "SELECT count(*) FROM p2f_child"), 0);
    s.transact(|tx| tx.renew_lease(&probe, time(13), time(30)))
        .unwrap();
}
#[test]
fn failed_acquisition_commit_origin_never_authorizes_outcome() {
    let s = memory();
    deferred_tables(&s);
    let mut old = None;
    assert_eq!(
        s.transact(|tx| {
            old = Some(acquire_at(tx, STEP, OWNER, None, 10, 20)?);
            tx.inner.execute("INSERT INTO p2f_child VALUES(99)", [])?;
            Ok(())
        }),
        Err(StoreError::ConstraintViolation)
    );
    let g = running(&s);
    fenced_unchanged(&s, old.unwrap());
    success(&s, g, 12).unwrap();
}
struct FileFixture(PathBuf);
impl FileFixture {
    fn new() -> Self {
        loop {
            let dir = std::env::temp_dir().join(format!(
                "serea-p2f-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            match std::fs::create_dir(&dir) {
                Ok(()) => return Self(dir),
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(e) => panic!("file fixture: {e}"),
            }
        }
    }
    fn open(&self) -> Store {
        Store::open(&self.0.join("store.sqlite"), &Fixed).unwrap()
    }
}
impl Drop for FileFixture {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).unwrap();
    }
}
#[test]
fn two_stores_observe_reclaim_fencing_current_outcome_and_durable_reopen() {
    let file = FileFixture::new();
    let a = file.open();
    fixture(&a);
    let b = file.open();
    let old = running(&a);
    assert_eq!(
        b.transact(|tx| acquire_at(tx, STEP, "worker-B", Some(1), 12, 30))
            .err(),
        Some(StoreError::LeaseHeld)
    );
    let g = b
        .transact(|tx| acquire_at(tx, STEP, OWNER, Some(1), 20, 30))
        .unwrap();
    fenced_unchanged(&a, old);
    begin(&b, &g, 21).unwrap();
    let expected = success(&a, g, 31).unwrap();
    assert_eq!(
        scalar::<i64>(&b, "SELECT count(*) FROM side_effect_receipts"),
        1
    );
    assert_eq!(
        scalar::<i64>(
            &b,
            "SELECT count(*) FROM task_journal WHERE journal_kind='STEP_COMMITTED'"
        ),
        1
    );
    let state = snapshot(&b);
    drop(a);
    drop(b);
    let reopened = file.open();
    assert_eq!(snapshot(&reopened), state);
    assert_eq!(
        reopened
            .transact(|tx| tx.get_blob(&expected.result.unwrap()))
            .unwrap(),
        b"{\"ok\":true}"
    );
    reopened.verify_integrity().unwrap();
}
#[test]
fn committed_reclaim_wins_before_cross_store_old_outcome_attempt() {
    let file = FileFixture::new();
    let a = file.open();
    fixture(&a);
    let b = file.open();
    let old = running(&a);
    let g = b
        .transact(|tx| acquire_at(tx, STEP, "worker-B", Some(1), 20, 30))
        .unwrap();
    fenced_unchanged(&a, old);
    begin(&b, &g, 21).unwrap();
    success(&b, g, 22).unwrap();
}
#[test]
fn simultaneous_outcome_and_reclaim_serialize_without_partial_or_duplicate_facts() {
    let file = FileFixture::new();
    let a = file.open();
    fixture(&a);
    let b = file.open();
    let old = running(&a);
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
    let b1 = barrier.clone();
    let outcome = std::thread::spawn(move || {
        b1.wait();
        success(&a, old, 20)
    });
    let reclaim = std::thread::spawn(move || {
        barrier.wait();
        b.transact(|tx| acquire_at(tx, STEP, "worker-B", Some(1), 20, 30))
    });
    let outcome = outcome.join().unwrap();
    let reclaim = reclaim.join().unwrap();
    let s = file.open();
    match (outcome, reclaim) {
        (Ok(_), Err(StoreError::LeaseFenced)) => {
            assert_eq!(
                scalar::<i64>(&s, "SELECT count(*) FROM side_effect_receipts"),
                1
            );
            assert_eq!(
                scalar::<i64>(
                    &s,
                    "SELECT count(*) FROM task_journal WHERE journal_kind='STEP_COMMITTED'"
                ),
                1
            );
        }
        (Err(StoreError::LeaseFenced), Ok(g)) => {
            assert_eq!(
                scalar::<i64>(&s, "SELECT count(*) FROM side_effect_receipts"),
                0
            );
            assert_eq!(
                scalar::<i64>(
                    &s,
                    "SELECT count(*) FROM task_journal WHERE journal_kind='STEP_COMMITTED'"
                ),
                0
            );
            begin(&s, &g, 21).unwrap();
            success(&s, g, 22).unwrap();
        }
        _ => panic!("SQLite must choose one authority winner"),
    }
    s.verify_integrity().unwrap();
    assert_eq!(
        scalar::<i64>(&s, "SELECT count(*) FROM side_effect_receipts"),
        1
    );
}
#[test]
fn personal_result_receipt_and_journal_preserve_task_class() {
    let s = memory();
    s.conn
        .lock()
        .unwrap()
        .execute("UPDATE tasks SET data_class_rank=1", [])
        .unwrap();
    let g = running(&s);
    let out = success(&s, g, 12).unwrap();
    assert_eq!(out.result.unwrap().class(), DataClass::Personal);
    assert_eq!(
        scalar::<i64>(&s, "SELECT data_class_rank FROM side_effect_receipts"),
        1
    );
    assert_eq!(
        scalar::<i64>(
            &s,
            "SELECT count(*) FROM task_journal WHERE data_class_rank<>1"
        ),
        0
    );
}
#[test]
fn private_begin_fails_closed_without_task_or_journal_mutation() {
    let s = memory();
    let g = acquire(&s);
    s.conn
        .lock()
        .unwrap()
        .execute("UPDATE tasks SET data_class_rank=2", [])
        .unwrap();
    let before = snapshot(&s);
    assert_eq!(
        begin(&s, &g, 11),
        Err(StoreError::AtRestProtectionUnavailable)
    );
    assert_eq!(snapshot(&s), before);
}
#[test]
fn multiple_verifiers_keep_task_verifying_until_last_known_success() {
    let s = memory();
    add_next(&s, "VERIFY");
    let g = running(&s);
    success(&s, g, 12).unwrap();
    s.conn.lock().unwrap().execute("INSERT INTO task_steps(step_id,task_id,sequence,kind,status,input_digest,provider_id,capability_id,capability_version,idempotency_key) SELECT 'stp_01JQ8Z9M3R2CVN8H5FWK7PQDSH',task_id,2,kind,status,input_digest,provider_id,capability_id,capability_version,?1 FROM task_steps WHERE sequence=1",[format!("idk_{}","c".repeat(64))]).unwrap();
    let c = Context::new();
    for (id, now, expected) in [
        (NEXT_STEP, 13, TaskState::Verifying),
        ("stp_01JQ8Z9M3R2CVN8H5FWK7PQDSH", 16, TaskState::Completed),
    ] {
        let g = s
            .transact(|tx| acquire_at(tx, id, OWNER, None, now, 30))
            .unwrap();
        begin(&s, &g, now + 1).unwrap();
        let out = s
            .transact(|tx| {
                tx.commit_step_outcome(
                    g,
                    StepOutcome::Succeeded {
                        result_json: b"true",
                        receipt: None,
                    },
                    time(now + 2),
                    &c.view(),
                )
            })
            .unwrap();
        assert_eq!(out.task_state, expected);
    }
    assert_eq!(
        scalar::<i64>(
            &s,
            "SELECT count(*) FROM task_journal WHERE journal_kind='TASK_TERMINAL'"
        ),
        1
    );
}
#[test]
fn borrowed_begin_outer_rollback_preserves_guard_for_actual_committed_authority() {
    let s = memory();
    let g = acquire(&s);
    let c = Context::new();
    let before = snapshot(&s);
    assert_eq!(
        s.transact(|tx| {
            tx.begin_attempt(&g, time(11), &c.view())?;
            Err::<(), _>(StoreError::Sqlite)
        }),
        Err(StoreError::Sqlite)
    );
    assert_eq!(snapshot(&s), before);
    begin(&s, &g, 12).unwrap();
    success(&s, g, 13).unwrap();
}
#[test]
fn ignored_final_lease_update_rolls_back_every_outcome_fact() {
    late_failure(
        Some(
            "CREATE TEMP TRIGGER p2f_ignore BEFORE UPDATE ON leases WHEN NEW.released_at_ms IS NOT NULL BEGIN SELECT RAISE(IGNORE); END;",
        ),
        b"{}",
        StoreError::LeaseFenced,
    );
}
#[test]
fn ignored_result_reference_insert_refuses_complete_outcome() {
    late_failure(
        Some(
            "CREATE TEMP TRIGGER p2f_ignore BEFORE INSERT ON step_blob_refs BEGIN SELECT RAISE(IGNORE); END;",
        ),
        b"{}",
        StoreError::ConstraintViolation,
    );
}
#[test]
fn duplicate_receipt_identity_rolls_back_second_steps_whole_outcome() {
    let s = memory();
    add_next(&s, "VERIFY");
    let g = running(&s);
    success(&s, g, 12).unwrap();
    let g = s
        .transact(|tx| acquire_at(tx, NEXT_STEP, OWNER, None, 13, 30))
        .unwrap();
    begin(&s, &g, 14).unwrap();
    let before = snapshot(&s);
    let mut r = receipt();
    r.idempotency_key = IdempotencyKey::new(format!("idk_{}", "b".repeat(64))).unwrap();
    let c = Context::new();
    assert_eq!(
        s.transact(|tx| tx.commit_step_outcome(
            g,
            StepOutcome::Succeeded {
                result_json: b"true",
                receipt: Some(&r)
            },
            time(15),
            &c.view()
        ))
        .err(),
        Some(StoreError::ConstraintViolation)
    );
    assert_eq!(snapshot(&s), before);
}
#[test]
fn final_failure_with_late_journal_error_rolls_back_complete_error_and_task() {
    let s = memory();
    let g = running(&s);
    let before = snapshot(&s);
    s.conn.lock().unwrap().execute_batch("CREATE TEMP TRIGGER p2f_abort BEFORE INSERT ON task_journal WHEN NEW.journal_kind='TASK_TERMINAL' BEGIN SELECT RAISE(ABORT,'private diagnostic'); END;").unwrap();
    s.transact(|tx| {
        assert_eq!(
            failure_call(tx, g, ActionErrorKind::ProviderError, None).err(),
            Some(StoreError::ConstraintViolation)
        );
        Ok(())
    })
    .unwrap();
    assert_eq!(snapshot(&s), before);
}
#[test]
fn outer_panic_rolls_back_outcome_and_reopen_preserves_committed_lease() {
    let file = FileFixture::new();
    let s = file.open();
    fixture(&s);
    let g = running(&s);
    let before = snapshot(&s);
    let c = Context::new();
    let r = receipt();
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = s.transact(|tx| {
                tx.commit_step_outcome(
                    g,
                    StepOutcome::Succeeded {
                        result_json: b"{}",
                        receipt: Some(&r),
                    },
                    time(12),
                    &c.view(),
                )?;
                panic!("controlled outer panic");
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
