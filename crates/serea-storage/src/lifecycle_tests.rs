//! Storage-private fixtures exercise states with no public P2F wait writer.
use super::{CancellationOutcome, DeletionOutcome};
use crate::audit::{DurableTransition, JournalRecords, TaskAuditParticipant, TestAudit};
use crate::{AtRestProtection, AtRestProtectionError, Store, StoreError, TransitionContext};
use rusqlite::{Connection, params, types::Value};
use serea_protocol::{
    ActorId, ActorKind, BlockedReason, Clock, DataClass, EpochMillis, EventId, ProtocolError,
    SemVer, TaskId, TaskOriginKind, TaskState,
};
use std::sync::Arc;

const TASK: &str = "tsk_01JQ8Z9K3M7QWXR4V2T6YH0BNA";
const OTHER: &str = "tsk_01JQ8Z9K3M7QWXR4V2T6YH0BNB";
const STEP: &str = "stp_01JQ8Z9M3R2CVN8H5FWK7PQDSF";
const EFFECT: &str = "stp_01JQ8Z9M3R2CVN8H5FWK7PQDSG";
const ACTIVE: &str = "stp_01JQ8Z9M3R2CVN8H5FWK7PQDSH";
const OTHER_STEP: &str = "stp_01JQ8Z9M3R2CVN8H5FWK7PQDSJ";
const STATES: [TaskState; 11] = [
    TaskState::Received,
    TaskState::Planning,
    TaskState::Ready,
    TaskState::Executing,
    TaskState::WaitingApproval,
    TaskState::WaitingUser,
    TaskState::Verifying,
    TaskState::Blocked,
    TaskState::Completed,
    TaskState::Failed,
    TaskState::Cancelled,
];
fn at(n: i64) -> EpochMillis {
    EpochMillis::new(n).unwrap()
}
fn id() -> TaskId {
    TaskId::new(TASK).unwrap()
}
fn by() -> TaskOriginKind {
    TaskOriginKind::new("USER_MESSAGE").unwrap()
}
fn reason() -> BlockedReason {
    BlockedReason::new("RESOURCE_UNAVAILABLE").unwrap()
}
struct Fixed;
impl Clock for Fixed {
    fn now_ms(&self) -> Result<EpochMillis, ProtocolError> {
        Ok(at(0))
    }
}
struct Context {
    actor: ActorId,
    version: SemVer,
    cause: EventId,
}
impl Context {
    fn new() -> Self {
        Self {
            actor: ActorId::new("lifecycle-test-host").unwrap(),
            version: SemVer::new("0.2.0").unwrap(),
            cause: EventId::new("evt_01JQ8Z9K3M7QWXR4V2T6YH0BNA").unwrap(),
        }
    }
    fn view(&self) -> TransitionContext<'_> {
        TransitionContext {
            actor_kind: ActorKind::Host,
            actor_id: &self.actor,
            actor_version: &self.version,
            causation_id: Some(&self.cause),
        }
    }
}
fn insert_task(c: &Connection, task: &str, state: TaskState, rank: u8) {
    c.execute(
        "INSERT INTO tasks(task_id,kind,title,state,origin_kind,data_class_rank,
         policy_class_rank,created_at_ms,updated_at_ms,deadline_at_ms,blocked_reason,
         failure_reason,cancelled_at_ms,cancelled_by,max_model_calls,max_tool_calls,
         max_attempts_per_step,origin_extensions,budget_extensions,extensions,result_summary)
         VALUES (?1,'USER_REQUEST','lifecycle fixture',?2,'USER_MESSAGE',?3,1,10,20,100,
         ?4,?5,?6,?7,12,24,3,'{\"future_origin\":true}',
         '{\"future_budget\":7}','{\"future_task\":null}','preserved summary')",
        params![
            task,
            state.wire_name(),
            rank,
            (state == TaskState::Blocked).then_some("RESOURCE_UNAVAILABLE"),
            (state == TaskState::Failed).then_some("BOUND_EXCEEDED_MODEL_CALLS"),
            (state == TaskState::Cancelled).then_some(20),
            (state == TaskState::Cancelled).then_some("SYSTEM")
        ],
    )
    .unwrap();
}
fn memory(state: TaskState) -> Store {
    let s = Store::open_in_memory(&Fixed).unwrap();
    insert_task(&s.conn.lock().unwrap(), TASK, state, 0);
    s
}
fn rows(c: &Connection, sql: &str) -> Vec<Vec<Value>> {
    let mut q = c.prepare(sql).unwrap();
    let n = q.column_count();
    q.query_map([], |r| (0..n).map(|i| r.get(i)).collect())
        .unwrap()
        .map(Result::unwrap)
        .collect()
}
fn table(s: &Store, name: &str) -> Vec<Vec<Value>> {
    rows(
        &s.conn.lock().unwrap(),
        &format!("SELECT * FROM {name} ORDER BY 1,2"),
    )
}
fn snapshot(s: &Store) -> Vec<Vec<Vec<Value>>> {
    [
        "tasks",
        "task_steps",
        "side_effect_receipts",
        "leases",
        "plan_revisions",
        "task_blob_refs",
        "step_blob_refs",
        "task_journal",
        "blobs",
    ]
    .map(|t| table(s, t))
    .to_vec()
}
fn scalar<T: rusqlite::types::FromSql>(s: &Store, sql: &str) -> T {
    s.conn
        .lock()
        .unwrap()
        .query_row(sql, [], |r| r.get(0))
        .unwrap()
}
fn add_steps(s: &Store) {
    let c = s.conn.lock().unwrap();
    let digest = format!("sha256:{}", "a".repeat(64));
    let key = format!("idk_{}", "a".repeat(64));
    c.execute(
        "INSERT INTO task_steps(step_id,task_id,sequence,kind,status,input_digest)
        VALUES (?1,?2,0,'NOTIFY','PLANNED',?3)",
        params![STEP, TASK, digest],
    )
    .unwrap();
    c.execute(
        "INSERT INTO task_steps(step_id,task_id,sequence,kind,status,input_digest,
        provider_id,capability_id,capability_version,idempotency_key,attempt,
        lease_generation,started_at_ms,completed_at_ms,result_digest)
        VALUES (?1,?2,1,'CAPABILITY','SUCCEEDED',?3,'calendar','calendar.events.list',
        '1.0.0',?4,1,1,11,12,?3)",
        params![EFFECT, TASK, digest, key],
    )
    .unwrap();
    c.execute(
        "INSERT INTO side_effect_receipts(receipt_id,task_id,step_id,capability_id,
        idempotency_key,effect_summary,observed_at_ms,replay_safe,data_class_rank)
        VALUES ('rcp_01JQ8Z9M3R2CVN8H5FWK7PQDSF',?1,?2,'calendar.events.list',?3,
        'known effect',12,1,0)",
        params![TASK, EFFECT, key],
    )
    .unwrap();
    c.execute(
        "INSERT INTO task_steps(step_id,task_id,sequence,kind,status,input_digest,
        attempt,lease_generation,started_at_ms,lease_owner,lease_expires_at_ms)
        VALUES (?1,?2,2,'NOTIFY','EXECUTING',?3,1,1,15,'worker',90)",
        params![ACTIVE, TASK, digest],
    )
    .unwrap();
    c.execute(
        "INSERT INTO leases(step_id,owner,generation,acquired_at_ms,expires_at_ms)
        VALUES (?1,'worker',1,14,90)",
        [ACTIVE],
    )
    .unwrap();
}
fn cancel(s: &Store, now: i64) -> Result<CancellationOutcome, StoreError> {
    let ctx = Context::new();
    s.transact_with_audit(&TestAudit, |tx| {
        tx.cancel_task(&id(), by(), at(now), &ctx.view())
    })
}
fn block(
    s: &Store,
    expected: TaskState,
    now: i64,
) -> Result<crate::task::TaskSnapshot, StoreError> {
    let ctx = Context::new();
    s.transact_with_audit(&TestAudit, |tx| {
        tx.block_task(&id(), expected, reason(), at(now), &ctx.view())
    })
}
fn invariant(
    s: &Store,
    expected: TaskState,
    now: i64,
) -> Result<crate::task::TaskSnapshot, StoreError> {
    let ctx = Context::new();
    s.transact_with_audit(&TestAudit, |tx| {
        tx.fail_task_invariant(&id(), expected, at(now), &ctx.view())
    })
}
fn delete(s: &Store) -> Result<DeletionOutcome, StoreError> {
    s.transact(|tx| tx.delete_task(&id()))
}

#[test]
fn cancel_all_eight_nonterminals_preserves_steps_receipts_and_leases() {
    let mut names = STATES.map(TaskState::wire_name).to_vec();
    names.sort_unstable();
    let mut protocol = TaskState::WIRE_NAMES.to_vec();
    protocol.sort_unstable();
    assert_eq!(names, protocol);
    for state in &STATES[..8] {
        let s = memory(*state);
        add_steps(&s);
        let before = snapshot(&s);
        assert_eq!(
            cancel(&s, 30).unwrap(),
            CancellationOutcome {
                changed: true,
                already_terminal: false,
                cancelled_at: Some(at(30)),
            }
        );
        assert_eq!(scalar::<String>(&s, "SELECT state FROM tasks"), "CANCELLED");
        assert_eq!(
            scalar::<String>(&s, "SELECT cancelled_by FROM tasks"),
            "USER_MESSAGE"
        );
        assert_eq!(scalar::<i64>(&s, "SELECT cancelled_at_ms FROM tasks"), 30);
        assert_eq!(scalar::<i64>(&s, "SELECT updated_at_ms FROM tasks"), 30);
        assert_eq!(
            scalar::<i64>(
                &s,
                "SELECT count(*) FROM tasks WHERE blocked_reason IS NULL AND failure_reason IS NULL"
            ),
            1
        );
        let after = snapshot(&s);
        for i in 1..7 {
            assert_eq!(after[i], before[i], "{state:?}, table {i}");
        }
        assert_eq!(after[8], before[8]);
        assert_eq!(
            scalar::<i64>(
                &s,
                "SELECT count(*) FROM task_journal WHERE actor_id='lifecycle-test-host' AND actor_kind='HOST' AND actor_version='0.2.0' AND causation_id='evt_01JQ8Z9K3M7QWXR4V2T6YH0BNA' AND occurred_at_ms=30 AND state_from IS NOT NULL AND state_to='CANCELLED'"
            ),
            3
        );
        let repeated = snapshot(&s);
        assert_eq!(
            cancel(&s, 0).unwrap(),
            CancellationOutcome {
                changed: false,
                already_terminal: true,
                cancelled_at: None,
            }
        );
        assert_eq!(snapshot(&s), repeated);
    }
}
#[test]
fn every_terminal_cancel_is_noop_even_with_old_time_and_no_audit() {
    for state in &STATES[8..] {
        let s = memory(*state);
        add_steps(&s);
        let before = snapshot(&s);
        let ctx = Context::new();
        let out = s
            .transact(|tx| tx.cancel_task(&id(), by(), at(0), &ctx.view()))
            .unwrap();
        assert_eq!(
            out,
            CancellationOutcome {
                changed: false,
                already_terminal: true,
                cancelled_at: None
            }
        );
        assert_eq!(snapshot(&s), before);
    }
}
#[test]
fn block_only_three_sources_and_illegal_requests_do_not_remediate() {
    for state in STATES {
        let s = memory(state);
        let before = snapshot(&s);
        let allowed = matches!(
            state,
            TaskState::Planning | TaskState::Executing | TaskState::Verifying
        );
        let result = block(&s, state, 30);
        if allowed {
            let model = result.unwrap();
            assert_eq!(model.task.state, TaskState::Blocked);
            assert_eq!(model.task.blocked_reason, Some(reason()));
            assert_eq!(model.task.failure_reason, None);
            assert_eq!(
                scalar::<i64>(
                    &s,
                    "SELECT count(*) FROM task_journal WHERE reason_code='RESOURCE_UNAVAILABLE' AND state_to='BLOCKED'"
                ),
                1
            );
        } else {
            assert_eq!(result.err(), Some(StoreError::IllegalTaskTransition));
            assert_eq!(snapshot(&s), before);
        }
    }
}
#[test]
fn invariant_explicitly_fails_eight_nonterminals_with_typed_frozen_cause() {
    for (index, state) in STATES.into_iter().enumerate() {
        let s = memory(state);
        let before = snapshot(&s);
        let result = invariant(&s, state, 30);
        if index < 8 {
            let model = result.unwrap();
            assert_eq!(model.task.state, TaskState::Failed);
            assert_eq!(
                model.task.failure_reason.as_ref().map(|r| r.as_str()),
                Some("INVARIANT_VIOLATION")
            );
            assert_eq!(model.task.blocked_reason, None);
            assert_eq!(model.task.cancelled_at, None);
            assert_eq!(model.task.cancelled_by, None);
            assert_eq!(
                scalar::<i64>(
                    &s,
                    "SELECT count(*) FROM task_journal WHERE reason_code='INVARIANT_VIOLATION' AND state_to='FAILED'"
                ),
                2
            );
        } else {
            assert_eq!(result.err(), Some(StoreError::IllegalTaskTransition));
            assert_eq!(snapshot(&s), before);
        }
    }
}
#[test]
fn expected_state_and_nondecreasing_time_are_enforced_without_writes() {
    for operation in 0..3 {
        let s = memory(TaskState::Planning);
        let before = snapshot(&s);
        for now in [9, 19] {
            let error = match operation {
                0 => block(&s, TaskState::Planning, now).err(),
                1 => invariant(&s, TaskState::Planning, now).err(),
                _ => cancel(&s, now).err(),
            };
            assert_eq!(error, Some(StoreError::InvalidTimestamp));
            assert_eq!(snapshot(&s), before);
        }
    }
    let s = memory(TaskState::Executing);
    let before = snapshot(&s);
    assert_eq!(
        block(&s, TaskState::Planning, 30).err(),
        Some(StoreError::IllegalTaskTransition)
    );
    assert_eq!(
        invariant(&s, TaskState::Ready, 30).err(),
        Some(StoreError::IllegalTaskTransition)
    );
    assert_eq!(snapshot(&s), before);
    assert!(block(&s, TaskState::Executing, 20).is_ok());
}
#[test]
fn block_and_invariant_preserve_unrelated_task_columns() {
    for fail in [false, true] {
        let s = memory(TaskState::Planning);
        let sql = "SELECT task_id,kind,title,origin_kind,origin_device_id,origin_message_id,
            origin_extensions,data_class_rank,policy_class_rank,created_at_ms,deadline_at_ms,
            result_summary,max_model_calls,max_tool_calls,max_attempts_per_step,
            budget_extensions,plan_revision,extensions FROM tasks";
        let before = rows(&s.conn.lock().unwrap(), sql);
        if fail {
            invariant(&s, TaskState::Planning, 30).unwrap();
        } else {
            block(&s, TaskState::Planning, 30).unwrap();
        }
        assert_eq!(rows(&s.conn.lock().unwrap(), sql), before);
    }
}
struct Reject;
impl TaskAuditParticipant for Reject {
    fn records(&self, _: &DurableTransition) -> Result<JournalRecords, StoreError> {
        Err(StoreError::AuditRejected)
    }
}
#[test]
fn audit_refusal_and_late_sql_failure_restore_whole_method_when_caught() {
    for late_sql in [false, true] {
        for operation in 0..3 {
            let s = memory(TaskState::Planning);
            let before = snapshot(&s);
            if late_sql {
                s.conn.lock().unwrap().execute_batch("CREATE TEMP TRIGGER refuse_journal BEFORE INSERT ON task_journal BEGIN SELECT RAISE(ABORT,'controlled failure'); END;").unwrap();
            }
            let audit: &dyn TaskAuditParticipant = if late_sql { &TestAudit } else { &Reject };
            let ctx = Context::new();
            s.transact_with_audit(audit, |tx| {
                let error = match operation {
                    0 => tx
                        .block_task(&id(), TaskState::Planning, reason(), at(30), &ctx.view())
                        .err(),
                    1 => tx
                        .fail_task_invariant(&id(), TaskState::Planning, at(30), &ctx.view())
                        .err(),
                    _ => tx.cancel_task(&id(), by(), at(30), &ctx.view()).err(),
                };
                assert_eq!(
                    error,
                    Some(if late_sql {
                        StoreError::ConstraintViolation
                    } else {
                        StoreError::AuditRejected
                    })
                );
                Ok(())
            })
            .unwrap();
            assert_eq!(snapshot(&s), before);
        }
    }
}
#[test]
fn plain_transaction_refuses_each_audited_mutation() {
    let s = memory(TaskState::Planning);
    let before = snapshot(&s);
    let ctx = Context::new();
    s.transact(|tx| {
        assert_eq!(
            tx.block_task(&id(), TaskState::Planning, reason(), at(30), &ctx.view())
                .err(),
            Some(StoreError::AuditRequired)
        );
        assert_eq!(
            tx.fail_task_invariant(&id(), TaskState::Planning, at(30), &ctx.view())
                .err(),
            Some(StoreError::AuditRequired)
        );
        assert_eq!(
            tx.cancel_task(&id(), by(), at(30), &ctx.view()).err(),
            Some(StoreError::AuditRequired)
        );
        Ok(())
    })
    .unwrap();
    assert_eq!(snapshot(&s), before);
}
#[test]
fn missing_task_errors_and_missing_delete_has_zero_counts() {
    let s = Store::open_in_memory(&Fixed).unwrap();
    assert_eq!(cancel(&s, 30).err(), Some(StoreError::TaskNotFound));
    assert_eq!(
        block(&s, TaskState::Planning, 30).err(),
        Some(StoreError::TaskNotFound)
    );
    assert_eq!(
        invariant(&s, TaskState::Planning, 30).err(),
        Some(StoreError::TaskNotFound)
    );
    assert_eq!(delete(&s).unwrap(), DeletionOutcome::default());
}
struct NeverProtection;
impl AtRestProtection for NeverProtection {
    fn protect(&self, _: &[u8]) -> Result<Vec<u8>, AtRestProtectionError> {
        panic!("not a row backend")
    }
    fn unprotect(&self, _: &[u8]) -> Result<Vec<u8>, AtRestProtectionError> {
        panic!("not a row backend")
    }
}
#[test]
fn unsupported_ordinary_classes_refuse_even_with_blob_backend() {
    for backend in [false, true] {
        for rank in [2, 3, 4] {
            let s = if backend {
                Store::open_in_memory_with_protection(&Fixed, Arc::new(NeverProtection)).unwrap()
            } else {
                Store::open_in_memory(&Fixed).unwrap()
            };
            {
                let c = s.conn.lock().unwrap();
                // Higher classes cannot normally reach this schema; prove fail-closed decoding.
                c.execute_batch("PRAGMA ignore_check_constraints=ON")
                    .unwrap();
                insert_task(&c, TASK, TaskState::Planning, rank);
                c.execute_batch("PRAGMA ignore_check_constraints=OFF")
                    .unwrap();
            }
            let before = snapshot(&s);
            let error = if rank == 2 {
                StoreError::AtRestProtectionUnavailable
            } else {
                StoreError::ClassRefused
            };
            assert_eq!(cancel(&s, 30).err(), Some(error));
            assert_eq!(block(&s, TaskState::Planning, 30).err(), Some(error));
            assert_eq!(invariant(&s, TaskState::Planning, 30).err(), Some(error));
            assert_eq!(delete(&s).err(), Some(error));
            assert_eq!(snapshot(&s), before);
        }
    }
}
#[test]
fn personal_rows_are_supported_and_audit_uses_task_class() {
    let s = memory(TaskState::Planning);
    s.conn
        .lock()
        .unwrap()
        .execute("UPDATE tasks SET data_class_rank=1", [])
        .unwrap();
    block(&s, TaskState::Planning, 30).unwrap();
    invariant(&s, TaskState::Blocked, 31).unwrap();
    assert_eq!(
        scalar::<i64>(
            &s,
            "SELECT count(*) FROM task_journal WHERE data_class_rank=1"
        ),
        3
    );
    assert_eq!(delete(&s).unwrap().journal_rows, 3);
}

fn deletion_fixture() -> (Store, Vec<String>) {
    let s = memory(TaskState::Executing);
    add_steps(&s);
    let digests = s
        .transact(|tx| {
            (0..7)
                .map(|n| {
                    tx.put_blob(n.to_string().as_bytes(), DataClass::Public)
                        .map(|b| b.digest().as_str().to_owned())
                })
                .collect::<Result<Vec<_>, _>>()
        })
        .unwrap();
    // Same digest, different class is unrelated unless captured at that class.
    s.transact(|tx| tx.put_blob(b"1", DataClass::Personal))
        .unwrap();
    {
        let c = s.conn.lock().unwrap();
        insert_task(&c, OTHER, TaskState::Received, 0);
        c.execute(
            "INSERT INTO task_steps(step_id,task_id,sequence,kind,status,input_digest)
            VALUES (?1,?2,0,'NOTIFY','PLANNED',?3)",
            params![OTHER_STEP, OTHER, digests[6]],
        )
        .unwrap();
        for (task, role, index) in [
            (TASK, "PLAN", 0),
            (TASK, "PLAN_REVISION", 3),
            (OTHER, "PLAN", 3),
        ] {
            c.execute(
                "INSERT INTO task_blob_refs(task_id,role,digest,data_class_rank)
                VALUES (?1,?2,?3,0)",
                params![task, role, digests[index]],
            )
            .unwrap();
        }
        for (step, role, index) in [
            (STEP, "INSTRUCTION", 1),
            (EFFECT, "RESULT", 4),
            (OTHER_STEP, "INSTRUCTION", 4),
        ] {
            c.execute(
                "INSERT INTO step_blob_refs(step_id,role,digest,data_class_rank)
                VALUES (?1,?2,?3,0)",
                params![step, role, digests[index]],
            )
            .unwrap();
        }
        for (task, revision, index) in [(TASK, 1, 2), (TASK, 2, 5), (OTHER, 1, 5)] {
            c.execute(
                "INSERT INTO plan_revisions(task_id,plan_revision,created_at_ms,
                plan_digest,data_class_rank,step_count) VALUES (?1,?2,20,?3,0,0)",
                params![task, revision, digests[index]],
            )
            .unwrap();
        }
        c.execute(
            "INSERT INTO task_journal(journal_id,task_id,journal_seq,journal_kind,
            actor_kind,actor_id,actor_version,data_class_rank,occurred_at_ms)
            VALUES ('fixture',?1,1,'TASK_INSERTED','HOST','fixture','0.2.0',0,10)",
            [TASK],
        )
        .unwrap();
    }
    (s, digests)
}
#[test]
fn deletion_counts_cascades_and_sweeps_only_unreferenced_captured_candidates() {
    let (s, digests) = deletion_fixture();
    let out = delete(&s).unwrap();
    assert_eq!(
        out,
        DeletionOutcome {
            task_rows: 1,
            steps: 3,
            receipts: 1,
            leases: 1,
            revisions: 2,
            task_refs: 2,
            step_refs: 2,
            journal_rows: 1,
            blobs: 3
        }
    );
    assert_eq!(scalar::<i64>(&s, "SELECT count(*) FROM tasks"), 1);
    assert_eq!(scalar::<i64>(&s, "SELECT count(*) FROM task_journal"), 0);
    let c = s.conn.lock().unwrap();
    for (i, digest) in digests.iter().enumerate() {
        let count: i64 = c
            .query_row(
                "SELECT count(*) FROM blobs WHERE digest=?1 AND data_class_rank=0",
                [digest],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, if i < 3 { 0 } else { 1 });
    }
    assert_eq!(
        c.query_row(
            "SELECT count(*) FROM blobs WHERE digest=?1 AND data_class_rank=1",
            [&digests[1]],
            |r| r.get::<_, i64>(0)
        )
        .unwrap(),
        1
    );
    drop(c);
    assert_eq!(delete(&s).unwrap(), DeletionOutcome::default());
}
#[test]
fn deletion_blob_failure_restores_cascaded_rows_when_caught_and_outer_commits() {
    let (s, _) = deletion_fixture();
    let before = snapshot(&s);
    s.conn.lock().unwrap().execute_batch("CREATE TEMP TRIGGER refuse_blob_delete BEFORE DELETE ON blobs BEGIN SELECT RAISE(ABORT,'controlled sweep failure'); END;").unwrap();
    s.transact(|tx| {
        assert_eq!(
            tx.delete_task(&id()).err(),
            Some(StoreError::ConstraintViolation)
        );
        tx.put_blob(b"99", DataClass::Public)?;
        Ok(())
    })
    .unwrap();
    let after = snapshot(&s);
    assert_eq!(&after[..8], &before[..8]);
    assert_eq!(after[8].len(), before[8].len() + 1);
}
#[test]
fn deletion_outer_error_restores_complete_operation() {
    let (s, _) = deletion_fixture();
    let before = snapshot(&s);
    assert_eq!(
        s.transact(|tx| {
            assert_eq!(tx.delete_task(&id())?.task_rows, 1);
            Err::<(), _>(StoreError::CanonicalJson)
        }),
        Err(StoreError::CanonicalJson)
    );
    assert_eq!(snapshot(&s), before);
}
#[test]
fn duplicate_candidates_are_counted_once_and_unreferenced_input_is_not_a_candidate() {
    let s = memory(TaskState::Received);
    let blob = s
        .transact(|tx| tx.put_blob(b"{}", DataClass::Public))
        .unwrap();
    let standalone = s
        .transact(|tx| tx.put_blob(b"[]", DataClass::Public))
        .unwrap();
    {
        let c = s.conn.lock().unwrap();
        c.execute(
            "INSERT INTO task_steps(step_id,task_id,sequence,kind,status,input_digest)
            VALUES (?1,?2,0,'NOTIFY','PLANNED',?3)",
            params![STEP, TASK, standalone.digest().as_str()],
        )
        .unwrap();
        for role in ["PLAN", "PLAN_REVISION"] {
            c.execute(
                "INSERT INTO task_blob_refs(task_id,role,digest,data_class_rank)
                VALUES (?1,?2,?3,0)",
                params![TASK, role, blob.digest().as_str()],
            )
            .unwrap();
        }
        c.execute(
            "INSERT INTO step_blob_refs(step_id,role,digest,data_class_rank)
            VALUES (?1,'INSTRUCTION',?2,0)",
            params![STEP, blob.digest().as_str()],
        )
        .unwrap();
    }
    let out = delete(&s).unwrap();
    assert_eq!(out.blobs, 1);
    assert_eq!(out.task_refs, 2);
    assert_eq!(out.step_refs, 1);
    assert_eq!(s.transact(|tx| tx.get_blob(&standalone)).unwrap(), b"[]");
}
