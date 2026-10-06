//! Whole task lifecycle operations and task-scoped deletion. No raw state writer.
use rusqlite::{OptionalExtension, params};
use serea_protocol::{
    BlockedReason, DataClass, EpochMillis, FailureReason, ReasonCode, TaskId, TaskOriginKind,
    TaskState,
};

use crate::audit::{AuditOperation, DurableTransition};
use crate::task::TaskSnapshot;
use crate::{StoreError, TransitionContext, Tx};

/// Cancellation writes only task metadata; it does not revoke or undo effects.
/// A terminal task, including a second cancellation, is a write-free no-op.
/// Results are durable only after the enclosing transaction commits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CancellationOutcome {
    pub changed: bool,
    pub already_terminal: bool,
    /// The timestamp written by this operation, not a prior cancellation stamp.
    pub cancelled_at: Option<EpochMillis>,
}

/// Rows removed by this task deletion, including only its orphaned candidate blobs.
/// Counts use `u64` independently of the host pointer width. Missing tasks return
/// all zeroes. Results are durable only after the enclosing transaction commits.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DeletionOutcome {
    pub task_rows: u64,
    pub steps: u64,
    pub receipts: u64,
    pub leases: u64,
    pub revisions: u64,
    pub task_refs: u64,
    pub step_refs: u64,
    pub journal_rows: u64,
    pub blobs: u64,
}

struct LifecycleRow {
    state: TaskState,
    class: DataClass,
    created: i64,
    updated: i64,
}

impl LifecycleRow {
    fn validate_time(&self, now: EpochMillis) -> Result<(), StoreError> {
        if now.get() < self.created || now.get() < self.updated {
            Err(StoreError::InvalidTimestamp)
        } else {
            Ok(())
        }
    }
}

// Keep predicates independent of snapshot/provenance loading. Cancellation and
// deletion do not need to reconstruct a plan or silently repair durable steps.
fn lifecycle_row(tx: &Tx<'_>, task_id: &TaskId) -> Result<Option<LifecycleRow>, StoreError> {
    let row: Option<(String, i64, i64, i64)> = tx
        .inner
        .query_row(
            "SELECT state,data_class_rank,created_at_ms,updated_at_ms FROM tasks WHERE task_id=?1",
            [task_id.as_str()],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .optional()?;
    let Some((state, rank, created, updated)) = row else {
        return Ok(None);
    };
    let state = match state.as_str() {
        "RECEIVED" => TaskState::Received,
        "PLANNING" => TaskState::Planning,
        "READY" => TaskState::Ready,
        "EXECUTING" => TaskState::Executing,
        "WAITING_APPROVAL" => TaskState::WaitingApproval,
        "WAITING_USER" => TaskState::WaitingUser,
        "VERIFYING" => TaskState::Verifying,
        "BLOCKED" => TaskState::Blocked,
        "COMPLETED" => TaskState::Completed,
        "FAILED" => TaskState::Failed,
        "CANCELLED" => TaskState::Cancelled,
        _ => return Err(StoreError::CorruptRow),
    };
    let class = match rank {
        0 => DataClass::Public,
        1 => DataClass::Personal,
        // An injected PRIVATE blob backend does not protect ordinary task rows.
        2 => return Err(StoreError::AtRestProtectionUnavailable),
        _ => return Err(StoreError::ClassRefused),
    };
    if updated < created || EpochMillis::new(created).is_err() || EpochMillis::new(updated).is_err()
    {
        return Err(StoreError::CorruptRow);
    }
    Ok(Some(LifecycleRow {
        state,
        class,
        created,
        updated,
    }))
}

fn nonterminal(state: TaskState) -> bool {
    match state {
        TaskState::Received
        | TaskState::Planning
        | TaskState::Ready
        | TaskState::Executing
        | TaskState::WaitingApproval
        | TaskState::WaitingUser
        | TaskState::Verifying
        | TaskState::Blocked => true,
        TaskState::Completed | TaskState::Failed | TaskState::Cancelled => false,
    }
}

fn one_task(changed: usize) -> Result<(), StoreError> {
    if changed == 1 {
        Ok(())
    } else {
        Err(StoreError::IllegalTaskTransition)
    }
}

impl Tx<'_> {
    /// Blocks only PLANNING, EXECUTING or VERIFYING at the expected state.
    /// Steps, receipts, leases and unrelated task fields remain unchanged.
    pub fn block_task(
        &mut self,
        task_id: &TaskId,
        expected_state: TaskState,
        reason: BlockedReason,
        now: EpochMillis,
        context: &TransitionContext<'_>,
    ) -> Result<TaskSnapshot, StoreError> {
        self.operation_savepoint(|tx| {
            let before = lifecycle_row(tx, task_id)?.ok_or(StoreError::TaskNotFound)?;
            if before.state != expected_state
                || !matches!(
                    expected_state,
                    TaskState::Planning | TaskState::Executing | TaskState::Verifying
                )
            {
                return Err(StoreError::IllegalTaskTransition);
            }
            before.validate_time(now)?;
            tx.require_audit()?;
            one_task(tx.inner.execute(
                "UPDATE tasks SET state='BLOCKED',blocked_reason=?1,failure_reason=NULL,
                    cancelled_at_ms=NULL,cancelled_by=NULL,updated_at_ms=?2
                 WHERE task_id=?3 AND state=?4 AND created_at_ms=?5 AND updated_at_ms=?6
                    AND data_class_rank=?7",
                params![
                    reason.as_str(),
                    now.get(),
                    task_id.as_str(),
                    expected_state.wire_name(),
                    before.created,
                    before.updated,
                    before.class.rank()
                ],
            )?)?;
            let model = tx.load_task(task_id)?;
            let mut facts = DurableTransition::task(
                AuditOperation::Blocked,
                task_id,
                Some(before.state),
                TaskState::Blocked,
                before.class,
                now,
                context,
            );
            facts.reason =
                Some(ReasonCode::new(reason.as_str()).map_err(|_| StoreError::CorruptRow)?);
            tx.record_transition(&facts)?;
            Ok(model)
        })
    }

    /// Explicit invariant remediation, separate from illegal-edge refusal.
    /// All eight nonterminal sources are eligible; terminal tasks never change.
    pub fn fail_task_invariant(
        &mut self,
        task_id: &TaskId,
        expected_state: TaskState,
        now: EpochMillis,
        context: &TransitionContext<'_>,
    ) -> Result<TaskSnapshot, StoreError> {
        self.operation_savepoint(|tx| {
            let before = lifecycle_row(tx, task_id)?.ok_or(StoreError::TaskNotFound)?;
            if before.state != expected_state || !nonterminal(expected_state) {
                return Err(StoreError::IllegalTaskTransition);
            }
            before.validate_time(now)?;
            tx.require_audit()?;
            let cause =
                FailureReason::new("INVARIANT_VIOLATION").map_err(|_| StoreError::CorruptRow)?;
            one_task(tx.inner.execute(
                "UPDATE tasks SET state='FAILED',failure_reason=?1,blocked_reason=NULL,
                    cancelled_at_ms=NULL,cancelled_by=NULL,updated_at_ms=?2
                 WHERE task_id=?3 AND state=?4 AND created_at_ms=?5 AND updated_at_ms=?6
                    AND data_class_rank=?7",
                params![
                    cause.as_str(),
                    now.get(),
                    task_id.as_str(),
                    expected_state.wire_name(),
                    before.created,
                    before.updated,
                    before.class.rank()
                ],
            )?)?;
            let model = tx.load_task(task_id)?;
            let mut facts = DurableTransition::task(
                AuditOperation::InvariantFailed,
                task_id,
                Some(before.state),
                TaskState::Failed,
                before.class,
                now,
                context,
            );
            facts.reason =
                Some(ReasonCode::new(cause.as_str()).map_err(|_| StoreError::CorruptRow)?);
            tx.record_transition(&facts)?;
            Ok(model)
        })
    }

    /// Cancels every nonterminal state without changing any step or receipt.
    /// Terminal no-ops ignore `now` and do not require or dispatch an audit mapper.
    pub fn cancel_task(
        &mut self,
        task_id: &TaskId,
        by: TaskOriginKind,
        now: EpochMillis,
        context: &TransitionContext<'_>,
    ) -> Result<CancellationOutcome, StoreError> {
        self.operation_savepoint(|tx| {
            let before = lifecycle_row(tx, task_id)?.ok_or(StoreError::TaskNotFound)?;
            if !nonterminal(before.state) {
                return Ok(CancellationOutcome {
                    changed: false,
                    already_terminal: true,
                    cancelled_at: None,
                });
            }
            before.validate_time(now)?;
            tx.require_audit()?;
            one_task(tx.inner.execute(
                "UPDATE tasks SET state='CANCELLED',cancelled_at_ms=?1,cancelled_by=?2,
                    blocked_reason=NULL,failure_reason=NULL,updated_at_ms=?1
                 WHERE task_id=?3 AND state=?4 AND created_at_ms=?5 AND updated_at_ms=?6
                    AND data_class_rank=?7",
                params![
                    now.get(),
                    by.as_str(),
                    task_id.as_str(),
                    before.state.wire_name(),
                    before.created,
                    before.updated,
                    before.class.rank()
                ],
            )?)?;
            let mut facts = DurableTransition::task(
                AuditOperation::Cancelled,
                task_id,
                Some(before.state),
                TaskState::Cancelled,
                before.class,
                now,
                context,
            );
            facts.cancelled_by = Some(by.clone());
            tx.record_transition(&facts)?;
            Ok(CancellationOutcome {
                changed: true,
                already_terminal: false,
                cancelled_at: Some(now),
            })
        })
    }

    /// Deletes the task and its FK-owned rows, then only its captured orphaned
    /// blobs. No TASK_DELETED journal row can survive the task cascade.
    pub fn delete_task(&mut self, task_id: &TaskId) -> Result<DeletionOutcome, StoreError> {
        self.operation_savepoint(|tx| {
            if lifecycle_row(tx, task_id)?.is_none() {
                return Ok(DeletionOutcome::default());
            }
            let mut outcome = tx.deletion_counts(task_id)?;
            // UNION deduplicates exact digest/class identities BEFORE the cascade.
            let candidates = crate::task::task_blob_candidates(&tx.inner, task_id)?;
            one_task(
                tx.inner
                    .execute("DELETE FROM tasks WHERE task_id=?1", [task_id.as_str()])?,
            )?;
            outcome.task_rows = 1;
            outcome.blobs = crate::task::sweep_blob_candidates(&tx.inner, &candidates)?;
            Ok(outcome)
        })
    }

    fn deletion_counts(&self, task_id: &TaskId) -> Result<DeletionOutcome, StoreError> {
        self.inner.query_row(
            "SELECT
                (SELECT count(*) FROM task_steps WHERE task_id=?1),
                (SELECT count(*) FROM side_effect_receipts WHERE task_id=?1),
                (SELECT count(*) FROM leases l JOIN task_steps s ON s.step_id=l.step_id WHERE s.task_id=?1),
                (SELECT count(*) FROM plan_revisions WHERE task_id=?1),
                (SELECT count(*) FROM task_blob_refs WHERE task_id=?1),
                (SELECT count(*) FROM step_blob_refs r JOIN task_steps s ON s.step_id=r.step_id WHERE s.task_id=?1),
                (SELECT count(*) FROM task_journal WHERE task_id=?1)",
            [task_id.as_str()],
            |r| {
                let count = |i| -> Result<u64, StoreError> {
                    let value: i64 = r.get(i).map_err(|_| StoreError::CorruptRow)?;
                    u64::try_from(value).map_err(|_| StoreError::CorruptRow)
                };
                Ok((|| Ok(DeletionOutcome {
                    steps: count(0)?, receipts: count(1)?, leases: count(2)?,
                    revisions: count(3)?, task_refs: count(4)?, step_refs: count(5)?, journal_rows: count(6)?,
                    ..DeletionOutcome::default()
                }))())
            },
        )?
    }
}

#[cfg(test)]
#[path = "lifecycle_tests.rs"]
mod lifecycle_tests;
