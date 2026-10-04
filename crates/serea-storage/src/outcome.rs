use std::sync::Arc;
use std::sync::atomic::Ordering;

use rusqlite::{OptionalExtension, named_params};
use serea_protocol::{
    ActionErrorKind, ActorId, ActorKind, DataClass, EpochMillis, ErrorCode, ErrorMessage, EventId,
    FailureReason, HostAction, LeaseOwner, ReasonCode, SemVer, SideEffectReceipt, StepId,
    StepStatus, TaskId, TaskState, canonicalize, digest_of,
};

use super::LeaseGuard;
use crate::audit::{AuditOperation, DurableTransition};
use crate::{BlobRef, StoreError, Tx};

/// Audit attribution, not lease authority. All values are validated protocol
/// types; the transition itself is constructed only after its fenced write.
/// No formatter exposes actor or causation identity.
pub struct TransitionContext<'a> {
    /// The source of the known outcome.
    pub actor_kind: ActorKind,
    /// Host-supplied actor identity, not the lease owner.
    pub actor_id: &'a ActorId,
    /// The actor's implementation version.
    pub actor_version: &'a SemVer,
    /// Optional cause of this transition.
    pub causation_id: Option<&'a EventId>,
}

/// A known final failure, not a retry request or an ambiguous external effect.
/// Details are original JSON object bytes so SCJ-1 can reject duplicate keys
/// before a lossy parse. Ordinary PRIVATE rows are not supported.
pub struct StepFailure<'a> {
    /// The protocol failure kind. AMBIGUOUS is refused here.
    pub kind: ActionErrorKind,
    /// Stable diagnostic category.
    pub code: &'a ErrorCode,
    /// Diagnostic prose; never consulted for control flow.
    pub message: &'a ErrorMessage,
    /// Provider metadata only; final FAILED does not schedule a retry.
    pub retryable: bool,
    /// Provider-suggested host action, recorded but not executed.
    pub host_action: &'a HostAction,
    /// Optional original SCJ-1 JSON object.
    pub details_json: Option<&'a [u8]>,
    /// Host-assigned terminal task reason.
    pub failure_reason: &'a FailureReason,
}

/// Facts supplied by the caller, never an independently fabricable lease fence.
/// Receipt presence describes a known effect; descriptor-dependent requirements
/// belong to P5, which is not implemented by storage.
pub enum StepOutcome<'a> {
    /// A known successful result. Original bytes must be UTF-8 SCJ-1 JSON.
    Succeeded {
        /// Original result document, canonicalized and stored atomically.
        result_json: &'a [u8],
        /// Durable proof when an effect occurred.
        receipt: Option<&'a SideEffectReceipt>,
    },
    /// A known final failure. No receipt or result reference is produced.
    Failed(StepFailure<'a>),
}

/// Inner outcome success. These facts are durable only after the enclosing
/// Store::transact_with_audit returns Ok; outer rollback/commit failure discards them.
#[derive(Debug)]
pub struct StepCommit {
    /// The committed step status.
    pub step_status: StepStatus,
    /// The derived aggregate task state.
    pub task_state: TaskState,
    /// Acquisition count, unchanged by begin and outcome.
    pub attempt: u32,
    /// Retained generation snapshot of the completed acquisition.
    pub generation: u32,
    /// Successful result's exact digest/class identity, if any.
    pub result: Option<BlobRef>,
}

// The step copy and leases row must both match. Expiry is deliberately absent:
// ADR-0024 permits known results from expired, unreclaimed, unreleased authority.
const FENCE: &str = "step_id=:step AND task_id=:task AND status=:pre_status
    AND lease_generation=:generation AND lease_owner=:owner
    AND EXISTS (SELECT 1 FROM leases l WHERE l.step_id=task_steps.step_id
        AND l.owner=:owner AND l.generation=:generation AND l.released_at_ms IS NULL)
    AND EXISTS (SELECT 1 FROM tasks t WHERE t.task_id=task_steps.task_id
        AND t.state=:expected_task AND t.updated_at_ms<=:now
        AND ((task_steps.kind='VERIFY' AND t.state='VERIFYING')
          OR (task_steps.kind<>'VERIFY' AND t.state IN ('READY','EXECUTING'))))
    AND NOT EXISTS (SELECT 1 FROM task_steps p WHERE p.task_id=task_steps.task_id
        AND p.sequence<task_steps.sequence AND p.status<>'SUCCEEDED')
    AND NOT EXISTS (SELECT 1 FROM task_steps p WHERE p.task_id=task_steps.task_id
        AND p.step_id<>task_steps.step_id AND p.status='EXECUTING')";

struct Before {
    task_state: TaskState,
    class: DataClass,
    attempt: u32,
    acquired: i64,
    expires: i64,
    updated: i64,
    started: Option<i64>,
}

type BeforeRow = (String, u8, u32, i64, i64, i64, Option<i64>);

fn before(tx: &Tx<'_>, guard: &LeaseGuard, status: &str) -> Result<Before, StoreError> {
    if !guard.origin.load(Ordering::Acquire) && !Arc::ptr_eq(&guard.origin, &tx.origin) {
        return Err(StoreError::LeaseFenced);
    }
    let row: Option<BeforeRow> = tx.inner.query_row(
        "SELECT t.state,t.data_class_rank,s.attempt,l.acquired_at_ms,l.expires_at_ms,
                t.updated_at_ms,s.started_at_ms
         FROM task_steps s JOIN tasks t ON t.task_id=s.task_id
         JOIN leases l ON l.step_id=s.step_id
         WHERE s.step_id=:step AND s.task_id=:task AND s.status=:pre_status
           AND s.lease_owner=:owner AND s.lease_generation=:generation
           AND l.owner=:owner AND l.generation=:generation AND l.released_at_ms IS NULL
           AND ((s.kind='VERIFY' AND t.state='VERIFYING')
             OR (s.kind<>'VERIFY' AND t.state IN ('READY','EXECUTING')))
           AND NOT EXISTS (SELECT 1 FROM task_steps p WHERE p.task_id=s.task_id
             AND p.sequence<s.sequence AND p.status<>'SUCCEEDED')
           AND NOT EXISTS (SELECT 1 FROM task_steps p WHERE p.task_id=s.task_id
             AND p.step_id<>s.step_id AND p.status='EXECUTING')",
        named_params! { ":step":guard.step_id.as_str(), ":task":guard.task_id.as_str(),
            ":owner":guard.owner.as_str(), ":generation":guard.generation.get(), ":pre_status":status },
        |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,r.get(5)?,r.get(6)?)),
    ).optional()?;
    let (state, rank, attempt, acquired, expires, updated, started) =
        row.ok_or(StoreError::LeaseFenced)?;
    let task_state = match state.as_str() {
        "READY" if status == "LEASED" => TaskState::Ready,
        "EXECUTING" => TaskState::Executing,
        "VERIFYING" => TaskState::Verifying,
        _ => return Err(StoreError::LeaseFenced),
    };
    let class = match rank {
        0 => DataClass::Public,
        1 => DataClass::Personal,
        // A blob backend does not protect task/receipt/journal ordinary rows.
        2 => DataClass::Private,
        _ => return Err(StoreError::ClassRefused),
    };
    Ok(Before {
        task_state,
        class,
        attempt,
        acquired,
        expires,
        updated,
        started,
    })
}

fn valid_time(before: &Before, now: EpochMillis) -> Result<(), StoreError> {
    if now.get() < before.acquired
        || now.get() < before.updated
        || before.started.is_some_and(|start| now.get() < start)
    {
        return Err(StoreError::InvalidLeaseInterval);
    }
    Ok(())
}
fn ordinary_class(class: DataClass) -> Result<(), StoreError> {
    match class {
        DataClass::Public | DataClass::Personal => Ok(()),
        DataClass::Private => Err(StoreError::AtRestProtectionUnavailable),
        DataClass::Secret | DataClass::Credential => Err(StoreError::ClassRefused),
    }
}
fn one(changed: usize, error: StoreError) -> Result<(), StoreError> {
    if changed == 1 { Ok(()) } else { Err(error) }
}

impl Tx<'_> {
    // No caller callback runs except this storage-owned closure. Panic cleanup
    // also protects callers that catch an unwind inside the outer transaction.
    fn outcome_savepoint<T>(
        &mut self,
        operation: impl FnOnce(&mut Self) -> Result<T, StoreError>,
    ) -> Result<T, StoreError> {
        self.ensure_active()?;
        self.inner.execute_batch("SAVEPOINT serea_outcome")?;
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| operation(self)));
        match result {
            Ok(Ok(value)) => match self.inner.execute_batch("RELEASE serea_outcome") {
                Ok(()) => Ok(value),
                Err(error) => {
                    self.rollback_only = true;
                    Err(error.into())
                }
            },
            Ok(Err(error)) => {
                if self
                    .inner
                    .execute_batch("ROLLBACK TO serea_outcome; RELEASE serea_outcome")
                    .is_err()
                {
                    self.rollback_only = true;
                    return Err(StoreError::Sqlite);
                }
                Err(error)
            }
            Err(panic) => {
                if self
                    .inner
                    .execute_batch("ROLLBACK TO serea_outcome; RELEASE serea_outcome")
                    .is_err()
                {
                    self.rollback_only = true;
                }
                std::panic::resume_unwind(panic)
            }
        }
    }

    /// Starts a leased attempt without charging a second acquisition. A matching
    /// lease must be unexpired. A borrowed guard can be retried after a cleaned-up
    /// method error, but its SQLite fence/lifecycle must still match. Requires
    /// an explicit participant through Store::transact_with_audit; a plain
    /// transaction refuses with AuditRequired before any write.
    pub fn begin_attempt(
        &mut self,
        guard: &LeaseGuard,
        now: EpochMillis,
        context: &TransitionContext<'_>,
    ) -> Result<(), StoreError> {
        self.require_audit()?;
        self.outcome_savepoint(|tx| {
            let before = before(tx, guard, "LEASED")?;
            if before.expires <= now.get() { return Err(StoreError::LeaseExpired); }
            valid_time(&before, now)?;
            ordinary_class(before.class)?;
            let changed = tx.inner.execute(
                &format!("UPDATE task_steps SET status='EXECUTING',started_at_ms=:now WHERE {FENCE}
                    AND EXISTS (SELECT 1 FROM leases l WHERE l.step_id=task_steps.step_id
                        AND l.expires_at_ms>:now AND l.acquired_at_ms<=:now)"),
                named_params! { ":now":now.get(), ":step":guard.step_id.as_str(), ":task":guard.task_id.as_str(),
                    ":owner":guard.owner.as_str(), ":generation":guard.generation.get(), ":pre_status":"LEASED", ":expected_task":before.task_state.wire_name() },
            )?;
            one(changed, StoreError::LeaseFenced)?;
            let to = if before.task_state == TaskState::Verifying { TaskState::Verifying } else { TaskState::Executing };
            tx.outcome_task(guard, &before, to, now, None)?;
            let mut facts = DurableTransition::task(AuditOperation::AttemptStarted, &guard.task_id,
                Some(before.task_state), to, before.class, now, context);
            facts.step_id = Some(guard.step_id.clone());
            facts.step_from = Some(StepStatus::new("LEASED").map_err(|_| StoreError::CorruptRow)?);
            facts.step_to = Some(StepStatus::new("EXECUTING").map_err(|_| StoreError::CorruptRow)?);
            facts.attempt = Some(before.attempt);
            facts.generation = Some(guard.generation.get());
            tx.record_transition(&facts)?;
            Ok(())
        })
    }

    /// Atomically records one known outcome under current durable SQLite authority.
    /// Expiry alone does not fence an outcome; committed reclaim or release does.
    /// The guard is consumed on **every** result, including infrastructure errors.
    /// Requires Store::transact_with_audit; plain transactions fail before writes.
    /// Inner Ok is not durable until that audited transaction returns Ok. No replacement
    /// capability is returned after rollback or an ambiguous commit failure.
    ///
    /// Outcome and release cannot be composed as two terminal uses:
    /// ```compile_fail
    /// use serea_storage::{Tx, LeaseGuard, StepOutcome, TransitionContext};
    /// use serea_protocol::EpochMillis;
    /// fn terminal(tx: &mut Tx<'_>, g: LeaseGuard, now: EpochMillis, c: &TransitionContext<'_>) {
    ///     let _ = tx.commit_step_outcome(g, StepOutcome::Succeeded { result_json: b"{}", receipt: None }, now, c);
    ///     let _ = tx.release_lease(g, now);
    /// }
    /// ```
    /// An error also consumes the guard, so ambiguous results cannot be retried:
    /// ```compile_fail
    /// use serea_storage::{Tx, LeaseGuard, StepOutcome, TransitionContext};
    /// use serea_protocol::EpochMillis;
    /// fn retry(tx: &mut Tx<'_>, g: LeaseGuard, now: EpochMillis, c: &TransitionContext<'_>) {
    ///     if tx.commit_step_outcome(g, StepOutcome::Succeeded { result_json: b"{}", receipt: None }, now, c).is_err() {
    ///         let _ = tx.commit_step_outcome(g, StepOutcome::Succeeded { result_json: b"{}", receipt: None }, now, c);
    ///     }
    /// }
    /// ```
    pub fn commit_step_outcome(
        &mut self,
        guard: LeaseGuard,
        outcome: StepOutcome<'_>,
        now: EpochMillis,
        context: &TransitionContext<'_>,
    ) -> Result<StepCommit, StoreError> {
        self.require_audit()?;
        self.outcome_savepoint(|tx| tx.commit_outcome_in(&guard, outcome, now, context))
    }

    fn commit_outcome_in(
        &mut self,
        guard: &LeaseGuard,
        outcome: StepOutcome<'_>,
        now: EpochMillis,
        context: &TransitionContext<'_>,
    ) -> Result<StepCommit, StoreError> {
        let before = before(self, guard, "EXECUTING")?;
        valid_time(&before, now)?;
        ordinary_class(before.class)?;
        let (status, result, to, reason, receipt_id, operation) = match outcome {
            StepOutcome::Succeeded {
                result_json,
                receipt,
            } => {
                let text =
                    std::str::from_utf8(result_json).map_err(|_| StoreError::CanonicalJson)?;
                let digest = digest_of(text).map_err(|_| StoreError::CanonicalJson)?;
                let changed = self.inner.execute(
                    &success_update_sql(),
                    named_params! { ":digest":digest.as_str(), ":now":now.get(), ":step":guard.step_id.as_str(), ":task":guard.task_id.as_str(),
                        ":owner":guard.owner.as_str(), ":generation":guard.generation.get(), ":pre_status":"EXECUTING", ":expected_task":before.task_state.wire_name() },
                )?;
                one(changed, StoreError::LeaseFenced)?;
                let blob = self.put_blob(result_json, before.class)?;
                one(self.inner.execute(
                    "INSERT INTO step_blob_refs(step_id,role,digest,data_class_rank) VALUES (:step,'RESULT',:digest,:rank)",
                    named_params! { ":step":guard.step_id.as_str(), ":digest":blob.digest().as_str(), ":rank":before.class.rank() },
                )?, StoreError::ConstraintViolation)?;
                if let Some(receipt) = receipt {
                    self.outcome_receipt(guard, receipt, &before, now)?;
                }
                let to = self.success_task_state(guard, &before)?;
                self.outcome_task(guard, &before, to, now, None)?;
                (
                    "SUCCEEDED",
                    Some(blob),
                    to,
                    None,
                    receipt.map(|r| r.receipt_id.clone()),
                    AuditOperation::StepSucceeded,
                )
            }
            StepOutcome::Failed(failure) => {
                if failure.kind == ActionErrorKind::Ambiguous {
                    return Err(StoreError::ConstraintViolation);
                }
                let details = failure
                    .details_json
                    .map(|bytes| {
                        let text =
                            std::str::from_utf8(bytes).map_err(|_| StoreError::CanonicalJson)?;
                        let bytes = canonicalize(text).map_err(|_| StoreError::CanonicalJson)?;
                        if bytes.first() != Some(&b'{') {
                            return Err(StoreError::CanonicalJson);
                        }
                        String::from_utf8(bytes).map_err(|_| StoreError::CanonicalJson)
                    })
                    .transpose()?;
                let changed = self.inner.execute(
                    &failure_update_sql(),
                    named_params! { ":now":now.get(), ":step":guard.step_id.as_str(), ":task":guard.task_id.as_str(),
                        ":owner":guard.owner.as_str(), ":generation":guard.generation.get(), ":pre_status":"EXECUTING", ":expected_task":before.task_state.wire_name(),
                        ":kind":failure.kind.wire_name(), ":code":failure.code.as_str(), ":message":failure.message.as_str(),
                        ":retryable":failure.retryable, ":action":failure.host_action.as_str(), ":details":details },
                )?;
                one(changed, StoreError::LeaseFenced)?;
                self.outcome_task(
                    guard,
                    &before,
                    TaskState::Failed,
                    now,
                    Some(failure.failure_reason),
                )?;
                (
                    "FAILED",
                    None,
                    TaskState::Failed,
                    Some(
                        ReasonCode::new(failure.failure_reason.as_str())
                            .map_err(|_| StoreError::ConstraintViolation)?,
                    ),
                    None,
                    AuditOperation::StepFailed,
                )
            }
        };

        one(self.inner.execute(
            "UPDATE leases SET released_at_ms=:now WHERE step_id=:step AND owner=:owner
                AND generation=:generation AND released_at_ms IS NULL AND acquired_at_ms<=:now
                AND EXISTS (SELECT 1 FROM task_steps s WHERE s.step_id=leases.step_id
                    AND s.task_id=:task AND s.lease_generation=:generation AND s.status=:status)",
            named_params! { ":now":now.get(), ":step":guard.step_id.as_str(), ":owner":guard.owner.as_str(),
                ":generation":guard.generation.get(), ":task":guard.task_id.as_str(), ":status":status },
        )?, StoreError::LeaseFenced)?;
        let step_status = StepStatus::new(status).map_err(|_| StoreError::ConstraintViolation)?;
        let mut facts = DurableTransition::task(
            operation,
            &guard.task_id,
            Some(before.task_state),
            to,
            before.class,
            now,
            context,
        );
        facts.step_id = Some(guard.step_id.clone());
        facts.step_from = Some(StepStatus::new("EXECUTING").map_err(|_| StoreError::CorruptRow)?);
        facts.step_to = Some(step_status.clone());
        facts.attempt = Some(before.attempt);
        facts.generation = Some(guard.generation.get());
        facts.result = result.as_ref().map(|b| b.digest().clone());
        facts.receipt_id = receipt_id;
        facts.reason = reason;
        self.record_transition(&facts)?;
        Ok(StepCommit {
            step_status,
            task_state: to,
            attempt: before.attempt,
            generation: guard.generation.get(),
            result,
        })
    }

    fn success_task_state(
        &self,
        guard: &LeaseGuard,
        before: &Before,
    ) -> Result<TaskState, StoreError> {
        let (remaining, ordinary): (i64, i64) = self.inner.query_row(
            "SELECT count(*),coalesce(sum(kind<>'VERIFY'),0) FROM task_steps
                WHERE task_id=:task AND status<>'SUCCEEDED'",
            named_params! { ":task":guard.task_id.as_str() },
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?;
        if before.task_state == TaskState::Verifying {
            if ordinary != 0 {
                return Err(StoreError::LeaseFenced);
            }
            Ok(if remaining == 0 {
                TaskState::Completed
            } else {
                TaskState::Verifying
            })
        } else {
            Ok(if ordinary == 0 {
                TaskState::Verifying
            } else {
                TaskState::Ready
            })
        }
    }

    fn outcome_task(
        &self,
        guard: &LeaseGuard,
        before: &Before,
        to: TaskState,
        now: EpochMillis,
        failure: Option<&FailureReason>,
    ) -> Result<(), StoreError> {
        one(self.inner.execute(
            "UPDATE tasks SET state=:to,updated_at_ms=:now,failure_reason=:failure
                WHERE task_id=:task AND state=:from AND updated_at_ms<=:now",
            named_params! { ":to":to.wire_name(), ":now":now.get(), ":failure":failure.map(FailureReason::as_str),
                ":task":guard.task_id.as_str(), ":from":before.task_state.wire_name() },
        )?, StoreError::LeaseFenced)
    }

    fn outcome_receipt(
        &self,
        guard: &LeaseGuard,
        receipt: &SideEffectReceipt,
        before: &Before,
        now: EpochMillis,
    ) -> Result<(), StoreError> {
        if receipt.observed_at.to_epoch_millis() > now {
            return Err(StoreError::InvalidLeaseInterval);
        }
        one(self.inner.execute(
            "INSERT INTO side_effect_receipts(receipt_id,task_id,step_id,capability_id,idempotency_key,
                provider_reference,effect_summary,observed_at_ms,replay_safe,data_class_rank)
             SELECT :receipt,:task,:step,:capability,:key,:reference,:summary,:observed,:safe,:rank
             FROM task_steps s WHERE s.step_id=:step AND s.task_id=:task AND s.status='SUCCEEDED'
                AND s.capability_id=:capability AND s.idempotency_key=:key AND s.lease_generation=:generation",
            named_params! { ":receipt":receipt.receipt_id.as_str(), ":task":guard.task_id.as_str(), ":step":guard.step_id.as_str(),
                ":capability":receipt.capability_id.as_str(), ":key":receipt.idempotency_key.as_str(),
                ":reference":receipt.provider_reference.as_ref().map(|r|r.as_str()), ":summary":receipt.effect_summary.as_str(),
                ":observed":receipt.observed_at.to_epoch_millis().get(), ":safe":receipt.replay_safe, ":rank":before.class.rank(),
                ":generation":guard.generation.get() },
        )?, StoreError::ConstraintViolation)
    }
}

fn success_update_sql() -> String {
    format!(
        "UPDATE task_steps SET status='SUCCEEDED',result_digest=:digest,completed_at_ms=:now,
        lease_owner=NULL,lease_expires_at_ms=NULL WHERE {FENCE} AND started_at_ms<=:now"
    )
}

fn failure_update_sql() -> String {
    format!("UPDATE task_steps SET status='FAILED',completed_at_ms=:now,
        lease_owner=NULL,lease_expires_at_ms=NULL,error_kind=:kind,error_code=:code,
        error_message=:message,error_retryable=:retryable,error_host_action=:action,error_details=:details
        WHERE {FENCE} AND started_at_ms<=:now")
}

// Private test access to the exact production statements and method envelope.
// These helpers do not exist in non-test builds or on the public API.
#[cfg(test)]
pub(super) fn first_write_sql(failure: bool) -> String {
    if failure {
        failure_update_sql()
    } else {
        success_update_sql()
    }
}

#[cfg(test)]
impl Tx<'_> {
    pub(super) fn probe_outcome_scope<T>(
        &mut self,
        operation: impl FnOnce(&mut Self) -> Result<T, StoreError>,
    ) -> Result<T, StoreError> {
        self.outcome_savepoint(operation)
    }
}

impl Tx<'_> {
    /// Audited whole acquisition, delegating the unchanged P2E authority checks.
    #[allow(clippy::too_many_arguments)]
    pub fn acquire_audited(
        &mut self,
        task_id: TaskId,
        step_id: StepId,
        owner: LeaseOwner,
        expected_generation: Option<u32>,
        now: EpochMillis,
        expires_at: EpochMillis,
        context: &TransitionContext<'_>,
    ) -> Result<LeaseGuard, StoreError> {
        self.require_audit()?;
        self.operation_savepoint(|tx| {
            let prior = tx.lease_audit_row(&task_id, &step_id)?;
            lease_audit_class(prior.1)?;
            let guard = tx.acquire_lease(
                task_id,
                step_id,
                owner,
                expected_generation,
                now,
                expires_at,
            )?;
            let after = tx.lease_audit_row(&guard.task_id, &guard.step_id)?;
            let facts = lease_facts(
                AuditOperation::LeaseAcquired,
                &guard.task_id,
                &guard.step_id,
                prior,
                after,
                now,
                context,
            )?;
            tx.record_transition(&facts)?;
            Ok(guard)
        })
    }

    /// Consumes the guard on every result, just like the low-level P2E release.
    pub fn release_audited(
        &mut self,
        guard: LeaseGuard,
        now: EpochMillis,
        context: &TransitionContext<'_>,
    ) -> Result<(), StoreError> {
        self.require_audit()?;
        self.operation_savepoint(|tx| {
            let task_id = guard.task_id.clone();
            let step_id = guard.step_id.clone();
            let generation = guard.generation.get();
            let prior = tx.lease_audit_row(&task_id, &step_id)?;
            lease_audit_class(prior.1)?;
            tx.release_lease(guard, now)?;
            let after = tx.lease_audit_row(&task_id, &step_id)?;
            let mut facts = lease_facts(
                AuditOperation::LeaseReleased,
                &task_id,
                &step_id,
                prior,
                after,
                now,
                context,
            )?;
            // Successful release checked this generation against leases, not the step copy.
            facts.generation = Some(generation);
            tx.record_transition(&facts)
        })
    }

    fn lease_audit_row(&self, task: &TaskId, step: &StepId) -> Result<LeaseAuditRow, StoreError> {
        self.inner
            .query_row(
                "SELECT t.state,t.data_class_rank,s.status,s.attempt,s.lease_generation
             FROM tasks t JOIN task_steps s ON s.task_id=t.task_id
             WHERE t.task_id=?1 AND s.step_id=?2",
                [task.as_str(), step.as_str()],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
            )
            .optional()?
            .ok_or(StoreError::LeaseFenced)
    }
}

type LeaseAuditRow = (String, u8, String, u32, u32);

fn lease_audit_class(rank: u8) -> Result<DataClass, StoreError> {
    let class = match rank {
        0 => DataClass::Public,
        1 => DataClass::Personal,
        2 => DataClass::Private,
        _ => return Err(StoreError::ClassRefused),
    };
    ordinary_class(class)?;
    Ok(class)
}

#[allow(clippy::too_many_arguments)]
fn lease_facts(
    operation: AuditOperation,
    task: &TaskId,
    step: &StepId,
    prior: LeaseAuditRow,
    after: LeaseAuditRow,
    now: EpochMillis,
    context: &TransitionContext<'_>,
) -> Result<DurableTransition, StoreError> {
    let state = |s: &str| match s {
        "RECEIVED" => Ok(TaskState::Received),
        "PLANNING" => Ok(TaskState::Planning),
        "READY" => Ok(TaskState::Ready),
        "EXECUTING" => Ok(TaskState::Executing),
        "WAITING_APPROVAL" => Ok(TaskState::WaitingApproval),
        "WAITING_USER" => Ok(TaskState::WaitingUser),
        "VERIFYING" => Ok(TaskState::Verifying),
        "BLOCKED" => Ok(TaskState::Blocked),
        "COMPLETED" => Ok(TaskState::Completed),
        "FAILED" => Ok(TaskState::Failed),
        "CANCELLED" => Ok(TaskState::Cancelled),
        _ => Err(StoreError::CorruptRow),
    };
    let class = lease_audit_class(after.1)?;
    let mut facts = DurableTransition::task(
        operation,
        task,
        Some(state(&prior.0)?),
        state(&after.0)?,
        class,
        now,
        context,
    );
    facts.step_id = Some(step.clone());
    facts.step_from = Some(StepStatus::new(prior.2).map_err(|_| StoreError::CorruptRow)?);
    facts.step_to = Some(StepStatus::new(after.2).map_err(|_| StoreError::CorruptRow)?);
    facts.attempt = Some(after.3);
    facts.generation = Some(after.4);
    Ok(facts)
}
