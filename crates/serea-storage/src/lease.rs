use std::fmt;
use std::num::NonZeroU32;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use rusqlite::{Connection, OptionalExtension, named_params, params};
use serea_protocol::{EpochMillis, LeaseOwner, StepId, TaskId};

use crate::{StoreError, Tx};

#[path = "outcome.rs"]
mod outcome;
pub use outcome::{StepCommit, StepFailure, StepOutcome, TransitionContext};

#[cfg(test)]
#[path = "lease_tests.rs"]
mod lease_tests;

#[cfg(test)]
#[path = "outcome_tests.rs"]
mod outcome_tests;

/// Nonclone handle to a particular acquisition, not authority by itself.
/// Every operation checks SQLite's authoritative leases row. Dropping a guard
/// does **not** release its durable lease; this is required for crash semantics.
/// A pending guard can be used within its originating transaction. Use in any
/// later transaction requires successful acquisition-transaction commit. A
/// private commit marker permanently rejects guards escaping rollback/panic,
/// even when a later same-owner acquisition reuses the rolled-back generation.
/// The marker never grants lease authority: SQLite must still match the fence.
///
/// Callers cannot construct a guard or access its private fields:
/// ```compile_fail
/// use serea_storage::LeaseGuard;
/// let _ = LeaseGuard::new();
/// ```
/// ```compile_fail
/// use serea_storage::LeaseGuard;
/// fn forge(g: LeaseGuard) { let _ = LeaseGuard { ..g }; }
/// ```
/// ```compile_fail
/// use serea_storage::LeaseGuard;
/// fn inspect(g: &LeaseGuard) { let _ = &g.owner; }
/// ```
/// It is neither Clone nor Copy:
/// ```compile_fail
/// use serea_storage::LeaseGuard;
/// fn require_clone<T: Clone>() {}
/// require_clone::<LeaseGuard>();
/// ```
/// ```compile_fail
/// use serea_storage::LeaseGuard;
/// fn require_copy<T: Copy>() {}
/// require_copy::<LeaseGuard>();
/// ```
/// Release consumes it, including on error:
/// ```compile_fail
/// use serea_protocol::EpochMillis;
/// use serea_storage::{LeaseGuard, Tx};
/// fn twice(tx: &mut Tx<'_>, g: LeaseGuard, now: EpochMillis) {
///     let _ = tx.release_lease(g, now);
///     let _ = tx.release_lease(g, now);
/// }
/// ```
pub struct LeaseGuard {
    task_id: TaskId,
    step_id: StepId,
    owner: LeaseOwner,
    generation: NonZeroU32,
    origin: Arc<AtomicBool>,
}

impl LeaseGuard {
    /// Positive generation observed from SQLite after successful acquisition.
    /// This counter is not an authorization precheck.
    pub fn generation(&self) -> u32 {
        self.generation.get()
    }
}

impl fmt::Debug for LeaseGuard {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("LeaseGuard")
    }
}

struct Authority {
    owner: String,
    generation: u32,
    acquired_at: i64,
    expires_at: i64,
    released_at: Option<i64>,
}

fn authority(
    conn: &Connection,
    task: &TaskId,
    step: &StepId,
) -> Result<Option<Authority>, StoreError> {
    Ok(conn
        .query_row(
            "SELECT l.owner,l.generation,l.acquired_at_ms,l.expires_at_ms,l.released_at_ms
         FROM leases l JOIN task_steps s ON s.step_id=l.step_id
         WHERE l.step_id=?1 AND s.task_id=?2",
            params![step.as_str(), task.as_str()],
            |r| {
                Ok(Authority {
                    owner: r.get(0)?,
                    generation: r.get(1)?,
                    acquired_at: r.get(2)?,
                    expires_at: r.get(3)?,
                    released_at: r.get(4)?,
                })
            },
        )
        .optional()?)
}

fn matching(tx: &Tx<'_>, guard: &LeaseGuard) -> Result<Authority, StoreError> {
    if !guard.origin.load(Ordering::Acquire) && !Arc::ptr_eq(&guard.origin, &tx.origin) {
        return Err(StoreError::LeaseFenced);
    }
    authority(&tx.inner, &guard.task_id, &guard.step_id)?
        .filter(|row| {
            row.owner == guard.owner.as_str()
                && row.generation == guard.generation.get()
                && row.released_at.is_none()
        })
        .ok_or(StoreError::LeaseFenced)
}

impl Tx<'_> {
    /// Acquires first authority or reclaims released/expired authority.
    /// `None` means never leased (SQL0); `Some(n)` must be the exact previously
    /// observed positive step generation. Same owner strings get no exemption.
    /// Expiry must be strictly later than the supplied absolute `now`.
    ///
    /// This method is failure-atomic even if its error is caught and the outer
    /// transaction subsequently commits. A private savepoint restores all its
    /// writes. Failed savepoint cleanup makes the outer Tx rollback-only.
    /// Successful acquisition spends one durable attempt, including a reclaim
    /// before any execution. Begin/outcome never charge this acquisition again.
    #[allow(clippy::too_many_arguments)]
    pub fn acquire_lease(
        &mut self,
        task_id: TaskId,
        step_id: StepId,
        owner: LeaseOwner,
        expected_generation: Option<u32>,
        now: EpochMillis,
        expires_at: EpochMillis,
    ) -> Result<LeaseGuard, StoreError> {
        self.ensure_active()?;
        let origin = self.origin.clone();
        let mut savepoint = self.inner.savepoint()?;
        let result = acquire_in(
            &savepoint,
            task_id,
            step_id,
            owner,
            expected_generation,
            now,
            expires_at,
            origin,
        );
        match result {
            Ok(guard) => match savepoint.commit() {
                Ok(()) => Ok(guard),
                Err(error) => {
                    self.rollback_only = true;
                    Err(error.into())
                }
            },
            Err(error) => {
                // Rollback is method-local, not delegated to the caller's `?`.
                // Release the rolled-back savepoint so later operations compose.
                if savepoint
                    .rollback()
                    .and_then(|()| savepoint.commit())
                    .is_err()
                {
                    self.rollback_only = true;
                    return Err(StoreError::Sqlite);
                }
                Err(error)
            }
        }
    }

    /// Strictly extends matching, unreleased and unexpired authority.
    /// Stale/released => LeaseFenced; matching expired => LeaseExpired;
    /// equal/shorter requested expiry => InvalidLeaseInterval.
    /// Only the authoritative expiry changes; the step copy is an acquisition
    /// snapshot and no lifecycle transition is performed.
    pub fn renew_lease(
        &mut self,
        guard: &LeaseGuard,
        now: EpochMillis,
        new_expiry: EpochMillis,
    ) -> Result<(), StoreError> {
        self.ensure_active()?;
        let row = matching(self, guard)?;
        if row.expires_at <= now.get() {
            return Err(StoreError::LeaseExpired);
        }
        if new_expiry.get() <= row.expires_at {
            return Err(StoreError::InvalidLeaseInterval);
        }
        let changed = self.inner.execute(
            "UPDATE leases SET expires_at_ms=:expiry
             WHERE step_id=:step AND owner=:owner AND generation=:generation
             AND released_at_ms IS NULL AND expires_at_ms>:now AND :expiry>expires_at_ms
             AND EXISTS (SELECT 1 FROM task_steps s WHERE s.step_id=leases.step_id AND s.task_id=:task)",
            named_params! { ":expiry":new_expiry.get(), ":step":guard.step_id.as_str(), ":owner":guard.owner.as_str(), ":generation":guard.generation.get(), ":now":now.get(), ":task":guard.task_id.as_str() },
        )?;
        if changed != 1 {
            return Err(StoreError::LeaseFenced);
        }
        Ok(())
    }

    /// Revokes this generation without changing the step's lifecycle or copy.
    /// A matching expired lease may be released; a dead generation is fenced.
    /// The guard is consumed on **every** result. An error, including Busy/Sqlite,
    /// is not proof of durable release and sacrifices retry capability: eventual
    /// expiry/recovery is required. Success is durable only after outer commit.
    pub fn release_lease(&mut self, guard: LeaseGuard, now: EpochMillis) -> Result<(), StoreError> {
        self.ensure_active()?;
        let row = matching(self, &guard)?;
        if now.get() < row.acquired_at {
            return Err(StoreError::InvalidLeaseInterval);
        }
        let changed = self.inner.execute(
            "UPDATE leases SET released_at_ms=:now
             WHERE step_id=:step AND owner=:owner AND generation=:generation AND released_at_ms IS NULL
             AND EXISTS (SELECT 1 FROM task_steps s WHERE s.step_id=leases.step_id AND s.task_id=:task)",
            named_params! { ":now":now.get(), ":step":guard.step_id.as_str(), ":owner":guard.owner.as_str(), ":generation":guard.generation.get(), ":task":guard.task_id.as_str() },
        )?;
        if changed != 1 {
            return Err(StoreError::LeaseFenced);
        }
        Ok(())
    }
}

#[allow(clippy::too_many_arguments)]
fn acquire_in(
    conn: &Connection,
    task_id: TaskId,
    step_id: StepId,
    owner: LeaseOwner,
    expected_generation: Option<u32>,
    now: EpochMillis,
    expires_at: EpochMillis,
    origin: Arc<AtomicBool>,
) -> Result<LeaseGuard, StoreError> {
    if expires_at <= now {
        return Err(StoreError::InvalidLeaseInterval);
    }
    if expected_generation == Some(0) {
        return Err(StoreError::LeaseFenced);
    }
    let step: Option<(u32, u32, i64)> = conn
        .query_row(
            "SELECT s.lease_generation,s.attempt,t.max_attempts_per_step
         FROM task_steps s JOIN tasks t ON t.task_id=s.task_id
         WHERE s.step_id=?1 AND s.task_id=?2 AND s.status IN ('PLANNED','LEASED','EXECUTING')",
            params![step_id.as_str(), task_id.as_str()],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .optional()?;
    let (old_generation, attempt, ceiling) = step.ok_or(StoreError::LeaseFenced)?;
    let prior = authority(conn, &task_id, &step_id)?;
    if let Some(row) = &prior {
        if row.released_at.is_none() && row.expires_at > now.get() {
            return Err(StoreError::LeaseHeld);
        }
    }
    if old_generation != expected_generation.unwrap_or(0)
        || prior.as_ref().map_or(0, |row| row.generation) != old_generation
    {
        return Err(StoreError::LeaseFenced);
    }
    // Preflight the arithmetic without parsing a CHECK diagnostic. The durable
    // post-update ceiling check below still covers the operation as a whole.
    if i64::from(attempt) >= ceiling || attempt == u32::MAX {
        return Err(StoreError::AttemptCeilingReached);
    }
    if prior.as_ref().is_some_and(|row| row.generation == u32::MAX) {
        return Err(StoreError::LeaseGenerationOverflow);
    }
    let changed = conn.execute(
        "INSERT INTO leases(step_id,owner,generation,acquired_at_ms,expires_at_ms)
         VALUES (:step,:owner,1,:now,:expiry)
         ON CONFLICT(step_id) DO UPDATE SET owner=excluded.owner,generation=leases.generation+1,
         acquired_at_ms=excluded.acquired_at_ms,expires_at_ms=excluded.expires_at_ms,released_at_ms=NULL
         WHERE (leases.released_at_ms IS NOT NULL OR leases.expires_at_ms<=:now)
         AND leases.generation<4294967295",
        named_params! { ":step":step_id.as_str(), ":owner":owner.as_str(), ":now":now.get(), ":expiry":expires_at.get() },
    )?;
    if changed != 1 {
        return Err(StoreError::LeaseFenced);
    }
    let changed = conn.execute(
        "UPDATE task_steps SET status='LEASED',lease_owner=:owner,lease_expires_at_ms=:expiry,
         lease_generation=(SELECT generation FROM leases WHERE step_id=:step),attempt=attempt+1,
         started_at_ms=NULL,result_digest=NULL,completed_at_ms=NULL
         WHERE step_id=:step AND task_id=:task AND status IN ('PLANNED','LEASED','EXECUTING')
         AND lease_generation=:expected",
        named_params! { ":owner":owner.as_str(), ":expiry":expires_at.get(), ":step":step_id.as_str(), ":task":task_id.as_str(), ":expected":expected_generation.unwrap_or(0) },
    )?;
    if changed != 1 {
        return Err(StoreError::LeaseFenced);
    }
    let within_ceiling: bool = conn.query_row(
        "SELECT s.attempt<=t.max_attempts_per_step FROM task_steps s JOIN tasks t ON t.task_id=s.task_id
         WHERE s.step_id=?1 AND s.task_id=?2", params![step_id.as_str(), task_id.as_str()], |r|r.get(0),
    )?;
    if !within_ceiling {
        return Err(StoreError::AttemptCeilingReached);
    }
    let generation: u32 = conn.query_row(
        "SELECT generation FROM leases WHERE step_id=?1",
        [step_id.as_str()],
        |r| r.get(0),
    )?;
    let generation = NonZeroU32::new(generation).ok_or(StoreError::LeaseFenced)?;
    Ok(LeaseGuard {
        task_id,
        step_id,
        owner,
        generation,
        origin,
    })
}
