//! Narrow, storage-owned recovery inspection and conditionally fenced repairs.
//! Raw values remain raw; only a checked projection may drive ordinary recovery.

use rusqlite::{Connection, OptionalExtension, params, types::Value};
use serea_protocol::{
    DataClass, Digest, EpochMillis, ReasonCode, ReceiptId, StepId, TaskId, TaskState,
};
use sha2::{Digest as _, Sha256};

use crate::{
    AuditOperation, DurableTransition, Migrations, StoreError, TaskSnapshot, TransitionContext, Tx,
};

/// Opaque observation, never an independently constructible mutation capability.
pub struct RecoverySnapshot {
    task_id: TaskId,
    raw_state: String,
    state: Option<TaskState>,
    class: DataClass,
    projection: Option<TaskSnapshot>,
    corruption: Option<ReasonCode>,
    steps: Vec<RecoveryStep>,
    receipt_repair: Option<RecoveryReceiptRepair>,
    fingerprint: Digest,
    created: i64,
    updated: i64,
    writable_task: bool,
}
impl RecoverySnapshot {
    pub fn task_id(&self) -> &TaskId {
        &self.task_id
    }
    pub fn raw_state(&self) -> &str {
        &self.raw_state
    }
    pub fn state(&self) -> Option<TaskState> {
        self.state
    }
    pub fn projection(&self) -> Option<&TaskSnapshot> {
        self.projection.as_ref()
    }
    pub fn corruption(&self) -> Option<&ReasonCode> {
        self.corruption.as_ref()
    }
    pub fn steps(&self) -> &[RecoveryStep] {
        &self.steps
    }
    pub fn receipt_repair(&self) -> Option<&RecoveryReceiptRepair> {
        self.receipt_repair.as_ref()
    }
    pub fn fingerprint(&self) -> &Digest {
        &self.fingerprint
    }
}

pub struct RecoveryStep {
    step_id: StepId,
    raw_status: String,
    authority: Option<RecoveryAuthority>,
    attempt: u32,
    generation: u32,
    copy_expires: Option<i64>,
}
impl RecoveryStep {
    pub fn step_id(&self) -> &StepId {
        &self.step_id
    }
    pub fn raw_status(&self) -> &str {
        &self.raw_status
    }
    pub fn authority(&self) -> Option<&RecoveryAuthority> {
        self.authority.as_ref()
    }
}

/// Validated SQLite authority, not a LeaseGuard or a lease owner capability.
pub struct RecoveryAuthority {
    owner: String,
    generation: u32,
    acquired_at: EpochMillis,
    expires_at: EpochMillis,
    released_at: Option<EpochMillis>,
}
impl RecoveryAuthority {
    pub fn generation(&self) -> u32 {
        self.generation
    }
    pub fn acquired_at(&self) -> EpochMillis {
        self.acquired_at
    }
    pub fn expires_at(&self) -> EpochMillis {
        self.expires_at
    }
    pub fn released_at(&self) -> Option<EpochMillis> {
        self.released_at
    }
}

/// Complete corroboration of a task-only stale outcome aggregate.
pub struct RecoveryReceiptRepair {
    step_id: StepId,
    receipt_id: ReceiptId,
    destination: TaskState,
}
impl RecoveryReceiptRepair {
    pub fn step_id(&self) -> &StepId {
        &self.step_id
    }
    pub fn receipt_id(&self) -> &ReceiptId {
        &self.receipt_id
    }
    pub fn destination(&self) -> TaskState {
        self.destination
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecoveryAction {
    ResumeNormally { next_step_id: Option<StepId> },
    HeldLease { step_id: StepId },
    NeedsReconciliation { step_id: StepId, block: bool },
    AwaitApproval { step_id: Option<StepId> },
    AwaitUser { step_id: Option<StepId> },
    BlockedTask,
    ReceiptAlreadyCommitted { step_id: StepId },
    Quarantine { reason: ReasonCode },
    ReceiptRepair { step_id: StepId },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecoveryApplied {
    pub changed: bool,
    pub revoked_steps: Vec<StepId>,
    pub state: Option<TaskState>,
    pub blocked: bool,
    pub receipt_repaired: bool,
}

fn reason(code: &str) -> ReasonCode {
    ReasonCode::new(code).expect("static recovery reason is protocol-shaped")
}
fn state(raw: &str) -> Option<TaskState> {
    Some(match raw {
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
        _ => return None,
    })
}
fn known_status(raw: &str) -> bool {
    matches!(
        raw,
        "PLANNED"
            | "LEASED"
            | "EXECUTING"
            | "WAITING"
            | "SUCCEEDED"
            | "FAILED"
            | "RECONCILED_ABSENT"
    )
}
fn ordinary_class(rank: i64) -> Result<DataClass, StoreError> {
    match rank {
        0 => Ok(DataClass::Public),
        1 => Ok(DataClass::Personal),
        2 => Err(StoreError::AtRestProtectionUnavailable),
        _ => Err(StoreError::ClassRefused),
    }
}
// This reserved structural reference distinguishes recovery task-edge evidence
// from normal outcome evidence without trusting participant payload prose.
pub(crate) fn recovery_edge_marker() -> Digest {
    Migrations::checksum("serea:p2g:recovery-task-edge:v1")
}

/// Borrowed, recovery-only capability for an atomic pass. It cannot acquire a
/// guard or access ordinary transaction writers, even if the pass error is caught.
///
/// ```compile_fail
/// use serea_storage::RecoveryPass;
/// let _ = RecoveryPass::acquire_audited;
/// ```
/// ```compile_fail
/// use serea_storage::RecoveryPass;
/// let _ = RecoveryPass::acquire_lease;
/// ```
/// ```compile_fail
/// use serea_storage::RecoveryPass;
/// let _ = RecoveryPass::begin_attempt;
/// ```
/// ```compile_fail
/// use serea_storage::RecoveryPass;
/// let _ = RecoveryPass::commit_step_outcome;
/// ```
/// ```compile_fail
/// use serea_storage::RecoveryPass;
/// fn sql(pass: &mut RecoveryPass<'_, '_>) {
///     pass.execute_batch("DELETE FROM tasks").unwrap();
/// }
/// ```
/// ```compile_fail
/// use serea_storage::RecoveryPass;
/// fn escape(pass: &mut RecoveryPass<'_, '_>) { let _ = &mut pass.tx; }
/// ```
/// ```compile_fail
/// use serea_storage::{RecoveryPass, Tx};
/// fn construct(tx: &mut Tx<'_>) { let _ = RecoveryPass { tx }; }
/// ```
/// ```
/// use serea_protocol::{Clock, EpochMillis, ProtocolError};
/// use serea_storage::{RecoveryPass, Store, StoreError};
/// struct Fixed;
/// impl Clock for Fixed {
///     fn now_ms(&self) -> Result<EpochMillis, ProtocolError> { EpochMillis::new(0) }
/// }
/// let store = Store::open_in_memory(&Fixed).unwrap();
/// store.transact(|tx| tx.recovery_pass(|pass: &mut RecoveryPass<'_, '_>| {
///     pass.recovery_preflight()?;
///     assert!(pass.recovery_tasks()?.is_empty());
///     assert_eq!(pass.recovery_journal_count()?, 0);
///     Ok::<_, StoreError>(())
/// })).unwrap();
/// ```
pub struct RecoveryPass<'tx, 'conn> {
    tx: &'tx mut Tx<'conn>,
}
impl RecoveryPass<'_, '_> {
    pub fn recovery_preflight(&self) -> Result<(), StoreError> {
        self.tx.recovery_preflight()
    }
    pub fn recovery_tasks(&self) -> Result<Vec<TaskId>, StoreError> {
        self.tx.recovery_tasks()
    }
    pub fn recovery_journal_count(&self) -> Result<u64, StoreError> {
        self.tx.recovery_journal_count()
    }
    pub fn inspect_recovery_task(&self, task_id: &TaskId) -> Result<RecoverySnapshot, StoreError> {
        self.tx.inspect_recovery_task(task_id)
    }
    pub fn apply_recovery(
        &mut self,
        snapshot: &RecoverySnapshot,
        action: RecoveryAction,
        now: EpochMillis,
        context: &TransitionContext<'_>,
    ) -> Result<RecoveryApplied, StoreError> {
        self.tx.apply_recovery(snapshot, action, now, context)
    }
}

impl Tx<'_> {
    /// Whole-pass rollback even when the caller catches the returned pass error.
    /// Run only finite storage/classification work here, never external effects.
    /// The callback cannot obtain an unrestricted transaction:
    /// ```compile_fail
    /// use serea_storage::Tx;
    /// fn unrestricted(tx: &mut Tx<'_>) {
    ///     let _ = tx.recovery_pass(|pass| {
    ///         let _: &mut Tx<'_> = pass;
    ///         Ok(())
    ///     });
    /// }
    /// ```
    pub fn recovery_pass<T>(
        &mut self,
        body: impl FnOnce(&mut RecoveryPass<'_, '_>) -> Result<T, StoreError>,
    ) -> Result<T, StoreError> {
        self.operation_savepoint(|tx| {
            tx.recovery_preflight()?;
            body(&mut RecoveryPass { tx })
        })
    }

    /// Catalog and ALL relational attribution precede any task-local write.
    /// Deliberately not quick_check: task CHECK damage is classified locally.
    pub fn recovery_preflight(&self) -> Result<(), StoreError> {
        self.ensure_active()?;
        crate::migrate::validate_catalog(Migrations::embedded())?;
        let count = crate::migrate::inspect(&self.inner, Migrations::embedded(), false)?;
        if count != Migrations::embedded().len() {
            return Err(StoreError::MigrationCatalogInvalid);
        }
        crate::migrate::foreign_key_check(&self.inner)
    }

    pub fn recovery_tasks(&self) -> Result<Vec<TaskId>, StoreError> {
        self.ensure_active()?;
        self.inner
            .prepare("SELECT task_id FROM tasks ORDER BY task_id")?
            .query_map([], |r| r.get::<_, String>(0))?
            .map(|row| TaskId::new(row?).map_err(|_| StoreError::IntegrityCheckFailed))
            .collect()
    }

    pub fn recovery_journal_count(&self) -> Result<u64, StoreError> {
        self.ensure_active()?;
        let count: i64 = self
            .inner
            .query_row("SELECT count(*) FROM task_journal", [], |r| r.get(0))?;
        u64::try_from(count).map_err(|_| StoreError::IntegrityCheckFailed)
    }

    pub fn inspect_recovery_task(&self, task_id: &TaskId) -> Result<RecoverySnapshot, StoreError> {
        self.ensure_active()?;
        let (raw_state, rank, created, updated): (String, i64, i64, i64) = self.inner.query_row(
            "SELECT state,data_class_rank,created_at_ms,updated_at_ms FROM tasks WHERE task_id=?1",
            [task_id.as_str()], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        ).optional()?.ok_or(StoreError::TaskNotFound)?;
        let class = ordinary_class(rank)?;
        let writable_task = task_checks_permit_quarantine(&self.inner, task_id)?;
        let typed_state = state(&raw_state);
        validate_related_classes(&self.inner, task_id)?;
        let fingerprint = fingerprint(&self.inner, task_id)?;
        let mut corruption = if typed_state.is_none() {
            Some(reason("UNRECOGNISED_STATE"))
        } else {
            None
        };
        let mut statement = self.inner.prepare(
            "SELECT step_id,status,attempt,lease_generation,lease_owner,lease_expires_at_ms FROM task_steps
                WHERE task_id=?1 ORDER BY sequence,step_id")?;
        let raw_steps = statement
            .query_map([task_id.as_str()], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, i64>(2)?,
                    r.get::<_, i64>(3)?,
                    r.get::<_, Option<String>>(4)?,
                    r.get::<_, Option<i64>>(5)?,
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        let mut steps = Vec::new();
        for (id, status, attempt, generation, owner, copy_expires) in raw_steps {
            let step_id = StepId::new(id).map_err(|_| StoreError::IntegrityCheckFailed)?;
            if !known_status(&status) {
                corruption = Some(reason("UNRECOGNISED_STATE"));
            }
            let attempt = u32::try_from(attempt);
            let generation = u32::try_from(generation);
            let authority = match (attempt.as_ref(), generation.as_ref()) {
                (Ok(attempt), Ok(generation)) => inspect_authority(
                    &self.inner,
                    &step_id,
                    &status,
                    *attempt,
                    *generation,
                    owner.as_deref(),
                    copy_expires,
                ),
                _ => Err(StoreError::CorruptRow),
            };
            let authority = match authority {
                Ok(value) => value,
                Err(StoreError::CorruptRow) => {
                    if corruption.is_none() {
                        corruption = Some(reason("INVARIANT_VIOLATION"));
                    }
                    None
                }
                Err(error) => return Err(error),
            };
            steps.push(RecoveryStep {
                step_id,
                raw_status: status,
                authority,
                attempt: attempt.unwrap_or(0),
                generation: generation.unwrap_or(0),
                copy_expires,
            });
        }
        // A terminal task is strictly immutable even when retained work is damaged.
        // Raw fingerprinting still detects unreadable relational structures.
        if typed_state.is_some_and(TaskState::is_terminal) {
            return Ok(RecoverySnapshot {
                task_id: task_id.clone(),
                raw_state,
                state: typed_state,
                class,
                projection: None,
                corruption: None,
                steps,
                receipt_repair: None,
                fingerprint,
                created,
                updated,
                writable_task,
            });
        }
        if !writable_task && corruption.is_none() {
            corruption = Some(reason("INVARIANT_VIOLATION"));
        }
        let projection = if corruption.is_none() {
            match self.load_task(task_id) {
                Ok(projection) => Some(projection),
                Err(
                    StoreError::CorruptRow
                    | StoreError::BlobMissing
                    | StoreError::BlobCorrupt
                    | StoreError::CanonicalJson
                    | StoreError::InvalidPlan
                    | StoreError::InvalidPlanLayout,
                ) => {
                    corruption = Some(reason("INVARIANT_VIOLATION"));
                    None
                }
                Err(error) => return Err(error),
            }
        } else {
            None
        };
        let mut receipt_repair = None;
        if let Some(model) = &projection {
            if !result_refs_match(&self.inner, model)?
                || (model.task.state == TaskState::Received && model.plan_revision != 0)
                || (matches!(
                    model.task.state,
                    TaskState::Executing | TaskState::Verifying
                ) && (model.plan_revision == 0 || model.steps.is_empty()))
                || (model.task.state == TaskState::Ready
                    && !model.steps.iter().any(|s| {
                        s.step.kind != serea_protocol::StepKind::Verify
                            && s.step.status.as_str() != "SUCCEEDED"
                    }))
            {
                corruption = Some(reason("INVARIANT_VIOLATION"));
            }
            if model.steps.iter().any(|s| {
                s.step.side_effect_receipt.is_some() && s.step.status.as_str() != "SUCCEEDED"
            }) {
                corruption = Some(reason("INVARIANT_VIOLATION"));
            } else {
                let stale = stale_destination(model);
                if let Some(destination) = stale {
                    receipt_repair = prove_receipt_repair(&self.inner, model, &steps, destination)?;
                    if receipt_repair.is_none() {
                        corruption = Some(reason("INVARIANT_VIOLATION"));
                    }
                }
                if model.task.state == TaskState::Verifying
                    && model.steps.iter().any(|s| {
                        s.step.kind != serea_protocol::StepKind::Verify
                            && s.step.status.as_str() != "SUCCEEDED"
                    })
                {
                    corruption = Some(reason("INVARIANT_VIOLATION"));
                }
            }
        }
        if corruption.is_some() {
            receipt_repair = None;
        }
        Ok(RecoverySnapshot {
            task_id: task_id.clone(),
            raw_state,
            state: typed_state,
            class,
            projection: if corruption.is_none() {
                projection
            } else {
                None
            },
            corruption,
            steps,
            receipt_repair,
            fingerprint,
            created,
            updated,
            writable_task,
        })
    }

    /// Conditional, internally atomic operation. Reinspection also fences stale
    /// snapshots after another operation in the SAME transaction.
    pub fn apply_recovery(
        &mut self,
        snapshot: &RecoverySnapshot,
        action: RecoveryAction,
        now: EpochMillis,
        context: &TransitionContext<'_>,
    ) -> Result<RecoveryApplied, StoreError> {
        self.operation_savepoint(|tx| {
            tx.recovery_preflight()?;
            let current = tx.inspect_recovery_task(snapshot.task_id())?;
            if current.fingerprint != snapshot.fingerprint {
                return Err(StoreError::RecoverySnapshotStale);
            }
            if current.state.is_some_and(TaskState::is_terminal) {
                return Ok(RecoveryApplied { changed: false, revoked_steps: vec![], state: current.state,
                    blocked: false, receipt_repaired: false });
            }
            let journal_only_corruption = matches!(action, RecoveryAction::Quarantine { .. })
                && !current.writable_task && current.state.is_some();
            if (!journal_only_corruption && (now.get() < current.created || now.get() < current.updated))
                || current.steps.iter().any(|s| s.authority.as_ref().is_some_and(|a|
                    a.acquired_at > now || a.released_at.is_some_and(|released| released > now))) {
                return Err(StoreError::InvalidTimestamp);
            }
            validate_action(&current, &action, now)?;
            tx.require_audit()?;
            let before_count = tx.recovery_journal_count()?;
            let mut revoked_steps = Vec::new();
            if current.corruption.is_none() {
                for step in &current.steps {
                    if let Some(authority) = &step.authority {
                        if authority.released_at.is_none() && authority.expires_at <= now
                            && matches!(step.raw_status.as_str(), "LEASED" | "EXECUTING") {
                            let changed = tx.inner.execute(
                                "UPDATE leases SET released_at_ms=?1 WHERE step_id=?2 AND owner=?3
                                    AND generation=?4 AND acquired_at_ms=?5 AND expires_at_ms=?6
                                    AND expires_at_ms<=?1 AND released_at_ms IS NULL
                                    AND EXISTS(SELECT 1 FROM task_steps s JOIN tasks t ON t.task_id=s.task_id
                                        WHERE s.step_id=leases.step_id AND s.task_id=?7 AND s.status=?8
                                        AND s.attempt=?9 AND s.lease_generation=?4 AND s.lease_owner=?3
                                        AND t.state=?10 AND s.lease_expires_at_ms IS ?11)",
                                params![now.get(), step.step_id.as_str(), authority.owner, authority.generation,
                                    authority.acquired_at.get(), authority.expires_at.get(), current.task_id.as_str(),
                                    step.raw_status, step.attempt, current.raw_state, step.copy_expires],
                            )?;
                            if changed != 1 { return Err(StoreError::RecoverySnapshotStale); }
                            revoked_steps.push(step.step_id.clone());
                        }
                    }
                }
            }
            let mut task_changed = false;
            let mut receipt_repaired = false;
            match &action {
                RecoveryAction::Quarantine { reason } => {
                    task_changed = tx.recovery_block(&current, reason, now, context)?;
                }
                RecoveryAction::NeedsReconciliation { block: true, .. } => {
                    task_changed = tx.recovery_block(&current, &reason("NEEDS_RECONCILIATION"), now, context)?;
                }
                RecoveryAction::ReceiptRepair { .. } => {
                    let repair = current.receipt_repair.as_ref().ok_or(StoreError::InvalidRecoveryAction)?;
                    tx.recovery_task_edge(&current, repair.destination, &reason("RECEIPT_REPAIR"), now, context)?;
                    task_changed = true;
                    receipt_repaired = true;
                }
                _ => (),
            }
            let after = tx.inspect_recovery_task(&current.task_id)?;
            tx.recovery_decision(&after, &action, Some(&current), now, context)?;
            if task_changed && after.state == Some(TaskState::Blocked) {
                tx.recovery_decision(&after, &RecoveryAction::BlockedTask, None, now, context)?;
            }
            Ok(RecoveryApplied {
                changed: task_changed || !revoked_steps.is_empty() || tx.recovery_journal_count()? != before_count,
                revoked_steps, state: after.state, blocked: after.state == Some(TaskState::Blocked), receipt_repaired,
            })
        })
    }

    fn recovery_block(
        &mut self,
        snapshot: &RecoverySnapshot,
        cause: &ReasonCode,
        now: EpochMillis,
        context: &TransitionContext<'_>,
    ) -> Result<bool, StoreError> {
        if !snapshot.writable_task {
            // Do not disable CHECKs or erase other damaged raw fields to block.
            // A recognised state still admits truthful invariant decision evidence.
            return if snapshot.state.is_some() {
                Ok(false)
            } else {
                Err(StoreError::CorruptRow)
            };
        }
        match snapshot.state {
            Some(TaskState::Blocked | TaskState::WaitingApproval | TaskState::WaitingUser) => {
                Ok(false)
            }
            Some(TaskState::Ready | TaskState::Received) => {
                let planning_reason = if snapshot.state == Some(TaskState::Received) {
                    "PLAN_REQUESTED"
                } else {
                    "REPLAN"
                };
                self.recovery_task_edge(
                    snapshot,
                    TaskState::Planning,
                    &reason(planning_reason),
                    now,
                    context,
                )?;
                let planning = self.inspect_recovery_task(&snapshot.task_id)?;
                self.recovery_task_edge(&planning, TaskState::Blocked, cause, now, context)?;
                Ok(true)
            }
            Some(TaskState::Planning | TaskState::Executing | TaskState::Verifying) | None => {
                self.recovery_task_edge(snapshot, TaskState::Blocked, cause, now, context)?;
                Ok(true)
            }
            _ => Err(StoreError::InvalidRecoveryAction),
        }
    }

    fn recovery_task_edge(
        &mut self,
        snapshot: &RecoverySnapshot,
        to: TaskState,
        cause: &ReasonCode,
        now: EpochMillis,
        context: &TransitionContext<'_>,
    ) -> Result<(), StoreError> {
        if now.get() < snapshot.created || now.get() < snapshot.updated {
            return Err(StoreError::InvalidTimestamp);
        }
        let changed = self.inner.execute(
            "UPDATE tasks SET state=?1,blocked_reason=CASE WHEN ?1='BLOCKED' THEN ?2 ELSE blocked_reason END,
                updated_at_ms=?3 WHERE task_id=?4 AND state=?5 AND updated_at_ms=?6 AND created_at_ms=?7",
            params![to.wire_name(), cause.as_str(), now.get(), snapshot.task_id.as_str(),
                snapshot.raw_state, snapshot.updated, snapshot.created],
        )?;
        if changed != 1 {
            return Err(StoreError::RecoverySnapshotStale);
        }
        if snapshot.state.is_some() {
            let mut facts = DurableTransition::task(
                AuditOperation::RecoveryStateChanged,
                &snapshot.task_id,
                snapshot.state,
                to,
                snapshot.class,
                now,
                context,
            );
            facts.reason = Some(cause.clone());
            facts.recovery_identity = Some(recovery_edge_marker());

            self.record_transition(&facts)?;
        }
        // Unknown source has no recognised enum edge. Its original raw value is
        // included in the pre-repair fingerprint, never fabricated as a source.
        Ok(())
    }

    fn recovery_decision(
        &mut self,
        snapshot: &RecoverySnapshot,
        action: &RecoveryAction,
        observed: Option<&RecoverySnapshot>,
        now: EpochMillis,
        context: &TransitionContext<'_>,
    ) -> Result<(), StoreError> {
        let (kind, step_id, cause) = action_identity(action);
        let to = snapshot.state.ok_or(StoreError::InvalidRecoveryAction)?;
        let mut facts = DurableTransition::task(
            AuditOperation::RecoveryDecision,
            &snapshot.task_id,
            Some(to),
            to,
            snapshot.class,
            now,
            context,
        );
        facts.recovery_identity = Some(snapshot.fingerprint.clone());
        facts.recovery_decision = Some(kind.into());

        facts.recovery_observed_fingerprint = observed.map(|s| s.fingerprint.clone());
        facts.reason = Some(cause);
        facts.step_id = step_id.cloned();
        if let Some(step) = step_id.and_then(|id| snapshot.steps.iter().find(|s| &s.step_id == id))
        {
            facts.attempt = Some(step.attempt);
            facts.generation = Some(step.generation);

            if let Some(model) = &snapshot.projection {
                if let Some(s) = model.steps.iter().find(|s| s.step.step_id == step.step_id) {
                    facts.result = s.step.result_digest.clone();
                    facts.receipt_id = s
                        .step
                        .side_effect_receipt
                        .as_ref()
                        .map(|r| r.receipt_id.clone());
                    facts.revision = Some(s.plan_revision);
                }
            }
        }
        self.record_transition(&facts)
    }
}

fn action_identity(action: &RecoveryAction) -> (&'static str, Option<&StepId>, ReasonCode) {
    match action {
        RecoveryAction::ResumeNormally { next_step_id } => (
            "ResumeNormally",
            next_step_id.as_ref(),
            reason("RESUME_NORMALLY"),
        ),
        RecoveryAction::HeldLease { step_id } => ("HeldLease", Some(step_id), reason("HELD_LEASE")),
        RecoveryAction::NeedsReconciliation { step_id, .. } => (
            "NeedsReconciliation",
            Some(step_id),
            reason("NEEDS_RECONCILIATION"),
        ),
        RecoveryAction::AwaitApproval { step_id } => {
            ("AwaitApproval", step_id.as_ref(), reason("AWAIT_APPROVAL"))
        }
        RecoveryAction::AwaitUser { step_id } => {
            ("AwaitUser", step_id.as_ref(), reason("AWAIT_USER"))
        }
        RecoveryAction::BlockedTask => ("BlockedTask", None, reason("BLOCKED_TASK")),
        RecoveryAction::ReceiptAlreadyCommitted { step_id }
        | RecoveryAction::ReceiptRepair { step_id } => (
            "ReceiptAlreadyCommitted",
            Some(step_id),
            reason("RECEIPT_ALREADY_COMMITTED"),
        ),
        RecoveryAction::Quarantine { reason } => {
            ("CorruptOrInvariantViolation", None, reason.clone())
        }
    }
}

fn validate_action(
    snapshot: &RecoverySnapshot,
    action: &RecoveryAction,
    now: EpochMillis,
) -> Result<(), StoreError> {
    if let RecoveryAction::Quarantine { reason } = action {
        return if snapshot.corruption.as_ref() == Some(reason) {
            Ok(())
        } else {
            Err(StoreError::InvalidRecoveryAction)
        };
    }
    if snapshot.corruption.is_some() && !matches!(action, RecoveryAction::BlockedTask)
        || snapshot.receipt_repair.is_some()
            && !matches!(action, RecoveryAction::ReceiptRepair { .. })
    {
        return Err(StoreError::InvalidRecoveryAction);
    }
    let step = |id: &StepId| snapshot.steps.iter().find(|s| &s.step_id == id);
    let model_step = |id: &StepId| {
        snapshot
            .projection
            .as_ref()
            .and_then(|m| m.steps.iter().find(|s| &s.step.step_id == id))
    };
    let held = |s: &RecoveryStep| {
        s.authority
            .as_ref()
            .is_some_and(|a| a.released_at.is_none() && a.expires_at > now)
    };
    let valid = match action {
        RecoveryAction::HeldLease { step_id } => step(step_id).is_some_and(held),
        RecoveryAction::NeedsReconciliation { step_id, block } => step(step_id).is_some_and(|s| {
            !held(s)
                && matches!(
                    s.raw_status.as_str(),
                    "LEASED" | "EXECUTING" | "RECONCILED_ABSENT"
                )
                && snapshot.projection.as_ref().is_some_and(|m| {
                    *block
                        == (s.raw_status != "RECONCILED_ABSENT"
                            && s.attempt >= m.task.attempt_budget.max_attempts_per_step)
                })
        }),
        RecoveryAction::BlockedTask => snapshot.state == Some(TaskState::Blocked),
        RecoveryAction::ReceiptRepair { step_id } => snapshot
            .receipt_repair
            .as_ref()
            .is_some_and(|r| &r.step_id == step_id),
        RecoveryAction::ReceiptAlreadyCommitted { step_id } => {
            model_step(step_id).is_some_and(|s| {
                s.step.status.as_str() == "SUCCEEDED" && s.step.side_effect_receipt.is_some()
            })
        }
        RecoveryAction::AwaitApproval { step_id } => {
            snapshot.state == Some(TaskState::WaitingApproval)
                && step_id.as_ref().is_none_or(|id| {
                    model_step(id).is_some_and(|s| {
                        s.step.kind == serea_protocol::StepKind::WaitApproval
                            && s.step.status.as_str() == "WAITING"
                    })
                })
        }
        RecoveryAction::AwaitUser { step_id } => {
            snapshot.state == Some(TaskState::WaitingUser)
                && step_id.as_ref().is_none_or(|id| {
                    model_step(id).is_some_and(|s| {
                        matches!(
                            s.step.kind,
                            serea_protocol::StepKind::WaitUser
                                | serea_protocol::StepKind::WaitSchedule
                        ) && s.step.status.as_str() == "WAITING"
                    })
                })
        }
        RecoveryAction::ResumeNormally { next_step_id } => {
            let allowed = matches!(
                snapshot.state,
                Some(
                    TaskState::Received
                        | TaskState::Planning
                        | TaskState::Ready
                        | TaskState::Executing
                        | TaskState::Verifying
                )
            );
            allowed
                && snapshot.projection.as_ref().is_some_and(|m| {
                    let next = m
                        .steps
                        .iter()
                        .find(|s| s.step.status.as_str() != "SUCCEEDED");
                    next.map(|s| &s.step.step_id) == next_step_id.as_ref()
                        && next.is_none_or(|s| {
                            matches!(s.step.status.as_str(), "PLANNED" | "LEASED")
                                && step(&s.step.step_id).is_some_and(|raw| !held(raw))
                                && s.step.attempt < m.task.attempt_budget.max_attempts_per_step
                        })
                        && !m.steps.iter().any(|s| {
                            matches!(
                                s.step.status.as_str(),
                                "EXECUTING" | "RECONCILED_ABSENT" | "FAILED" | "WAITING"
                            ) || s.step.status.as_str() == "LEASED"
                                && s.step.attempt >= m.task.attempt_budget.max_attempts_per_step
                        })
                        && !snapshot.steps.iter().any(held)
                })
        }
        RecoveryAction::Quarantine { .. } => false,
    };
    if valid {
        Ok(())
    } else {
        Err(StoreError::InvalidRecoveryAction)
    }
}

type AuthorityRow = (String, i64, i64, i64, Option<i64>);
fn inspect_authority(
    conn: &Connection,
    step: &StepId,
    status: &str,
    attempt: u32,
    generation: u32,
    owner: Option<&str>,
    copy_expires: Option<i64>,
) -> Result<Option<RecoveryAuthority>, StoreError> {
    if attempt != generation {
        return Err(StoreError::CorruptRow);
    }
    let row: Option<AuthorityRow> = conn.query_row(
        "SELECT owner,generation,acquired_at_ms,expires_at_ms,released_at_ms FROM leases WHERE step_id=?1",
        [step.as_str()], |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?)),
    ).optional()?;
    let Some((lease_owner, lease_generation, acquired, expires, released)) = row else {
        return if generation == 0 && status == "PLANNED" {
            Ok(None)
        } else {
            Err(StoreError::CorruptRow)
        };
    };
    if generation == 0
        || i64::from(generation) != lease_generation
        || lease_owner.is_empty()
        || owner.is_some_and(|owner| owner != lease_owner)
        || matches!(status, "LEASED" | "EXECUTING")
            && (owner.is_none()
                || copy_expires.is_none_or(|copy| copy <= acquired || copy > expires))
        || matches!(
            status,
            "WAITING" | "SUCCEEDED" | "FAILED" | "RECONCILED_ABSENT"
        ) && released.is_none()
        || expires <= acquired
        || released.is_some_and(|r| r < acquired)
    {
        return Err(StoreError::CorruptRow);
    }
    let epoch = |value| EpochMillis::new(value).map_err(|_| StoreError::CorruptRow);
    Ok(Some(RecoveryAuthority {
        owner: lease_owner,
        generation,
        acquired_at: epoch(acquired)?,
        expires_at: epoch(expires)?,
        released_at: released.map(epoch).transpose()?,
    }))
}

fn stale_destination(model: &TaskSnapshot) -> Option<TaskState> {
    if model.task.state == TaskState::Executing
        && !model
            .steps
            .iter()
            .any(|s| matches!(s.step.status.as_str(), "EXECUTING" | "LEASED"))
        && model
            .steps
            .iter()
            .any(|s| s.step.status.as_str() == "SUCCEEDED")
    {
        Some(
            if model.steps.iter().any(|s| {
                s.step.kind != serea_protocol::StepKind::Verify
                    && s.step.status.as_str() != "SUCCEEDED"
            }) {
                TaskState::Ready
            } else {
                TaskState::Verifying
            },
        )
    } else if model.task.state == TaskState::Verifying
        && model
            .steps
            .iter()
            .all(|s| s.step.status.as_str() == "SUCCEEDED")
        && model
            .steps
            .iter()
            .any(|s| s.step.kind == serea_protocol::StepKind::Verify)
    {
        Some(TaskState::Completed)
    } else {
        None
    }
}

// Verify complete contiguous normal outcome evidence, not isolated/free-text
// assertions. Its envelope, canonical payload digest, generation and actual
// result must agree. Later task transitions invalidate the proposed repair.
fn prove_receipt_repair(
    conn: &Connection,
    model: &TaskSnapshot,
    steps: &[RecoveryStep],
    destination: TaskState,
) -> Result<Option<RecoveryReceiptRepair>, StoreError> {
    let mut proven = None;
    for step in &model.steps {
        let Some(receipt) = &step.step.side_effect_receipt else {
            continue;
        };
        let Some(authority) = steps
            .iter()
            .find(|s| s.step_id == step.step.step_id)
            .and_then(|s| s.authority.as_ref())
        else {
            continue;
        };
        let Some(released) = authority.released_at else {
            continue;
        };
        if step
            .step
            .completed_at
            .as_ref()
            .map(|stamp| stamp.to_epoch_millis())
            != Some(released)
            || receipt.observed_at.to_epoch_millis() > released
        {
            continue;
        }
        let Some(result) = &step.step.result_digest else {
            continue;
        };
        let rows = raw_rows(
            conn,
            "SELECT * FROM task_journal WHERE task_id=?1 ORDER BY journal_seq",
            &model.task.task_id,
        )?;
        // P2F release is atomic but has no outcome-specific release journal
        // draft. Corroborate it with the authoritative row and batch stamp.
        for batch in rows.windows(3) {
            // Column indices are fixed by frozen migration 0001.
            fn text(row: &[Value], i: usize) -> Option<&str> {
                match row.get(i) {
                    Some(Value::Text(s)) => Some(s.as_str()),
                    _ => None,
                }
            }
            let integer = |row: &[Value], i| match row.get(i) {
                Some(Value::Integer(n)) => Some(*n),
                _ => None,
            };
            let expected = ["STEP_COMMITTED", "RECEIPT_RECORDED", "TASK_STATE_CHANGED"];
            if !batch.iter().zip(expected).all(|(r, kind)| {
                text(r, 4) == Some(kind)
                    && text(r, 2) == Some(step.step.step_id.as_str())
                    && integer(r, 7) == Some(i64::from(step.step.attempt))
                    && integer(r, 14) == Some(released.get())
                    && integer(r, 13) == Some(i64::from(model.task.data_class.rank()))
                    && (9..13).all(|i| r[i] == batch[0][i])
            }) {
                continue;
            }
            if !batch
                .windows(2)
                .all(|p| integer(&p[0], 3).and_then(|n| n.checked_add(1)) == integer(&p[1], 3))
            {
                continue;
            }
            if !batch[..2].iter().all(|r| {
                text(r, 5) == Some("EXECUTING")
                    && text(r, 6) == Some("SUCCEEDED")
                    && text(r, 17) == Some(result.as_str())
            }) || text(&batch[2], 5) != Some(model.task.state.wire_name())
                || text(&batch[2], 6) != Some(destination.wire_name())
            {
                continue;
            }
            let complete_payloads = batch.iter().all(|r| {
                let (Some(payload), Some(digest)) = (text(r, 16), text(r, 15)) else {
                    return false;
                };
                if serea_protocol::canonicalize(payload).ok().as_deref() != Some(payload.as_bytes())
                    || serea_protocol::digest_of(payload)
                        .ok()
                        .as_ref()
                        .map(Digest::as_str)
                        != Some(digest)
                {
                    return false;
                }
                let Ok(value) = serde_json::from_str::<serde_json::Value>(payload) else {
                    return false;
                };
                value.get("attempt").and_then(serde_json::Value::as_u64)
                    == Some(u64::from(step.step.attempt))
                    && value.get("generation").and_then(serde_json::Value::as_u64)
                        == Some(u64::from(authority.generation))
                    && value
                        .get("result_digest")
                        .and_then(serde_json::Value::as_str)
                        == Some(result.as_str())
            });
            if !complete_payloads {
                continue;
            }
            let seq = integer(&batch[2], 3).ok_or(StoreError::CorruptRow)?;
            let terminal_seq = if destination == TaskState::Completed {
                let terminal = rows.iter().find(|r| integer(r, 3) == seq.checked_add(1));
                let Some(terminal) = terminal else {
                    continue;
                };
                if text(terminal, 4) != Some("TASK_TERMINAL")
                    || text(terminal, 5) != Some(model.task.state.wire_name())
                    || text(terminal, 6) != Some("COMPLETED")
                    || !((7..18).all(|i| terminal[i] == batch[2][i]))
                    || text(terminal, 2) != Some(step.step.step_id.as_str())
                {
                    continue;
                }
                seq + 1
            } else {
                seq
            };
            let superseding: bool = conn.query_row(
                "SELECT EXISTS(SELECT 1 FROM task_journal WHERE task_id=?1 AND journal_seq>?2
                    AND journal_kind IN ('TASK_STATE_CHANGED','TASK_TERMINAL'))",
                params![model.task.task_id.as_str(), terminal_seq],
                |r| r.get(0),
            )?;
            if superseding {
                continue;
            }
            if proven.is_some() {
                return Ok(None);
            }
            proven = Some(RecoveryReceiptRepair {
                step_id: step.step.step_id.clone(),
                receipt_id: receipt.receipt_id.clone(),
                destination,
            });
        }
    }
    Ok(proven)
}

fn task_checks_permit_quarantine(conn: &Connection, task: &TaskId) -> Result<bool, StoreError> {
    // Bounded to one attributed row, not a global quick_check. Independent
    // frozen task CHECK damage cannot be repaired by changing state alone.
    conn.query_row(
        "SELECT kind IN ('USER_REQUEST','SCHEDULED','PROACTIVE','DELEGATED_HOST_GOAL','MAINTENANCE')
          AND policy_class_rank BETWEEN 0 AND 7
          AND created_at_ms BETWEEN -62167219200000 AND 253402300799999
          AND updated_at_ms BETWEEN -62167219200000 AND 253402300799999
          AND updated_at_ms>=created_at_ms
          AND (deadline_at_ms IS NULL OR (deadline_at_ms BETWEEN -62167219200000 AND 253402300799999 AND deadline_at_ms>=created_at_ms))
          AND failure_reason IS NULL AND cancelled_at_ms IS NULL AND cancelled_by IS NULL
          AND (state='BLOCKED' OR blocked_reason IS NULL)
          AND (blocked_reason IS NULL OR (blocked_reason NOT GLOB '*[^A-Z0-9_]*' AND substr(blocked_reason,1,1) BETWEEN 'A' AND 'Z'))
          AND max_model_calls>=0 AND max_tool_calls>=0 AND max_attempts_per_step>=0 AND plan_revision>=0
          AND CASE WHEN json_valid(origin_extensions) THEN json_type(origin_extensions)='object' ELSE 0 END
          AND CASE WHEN json_valid(budget_extensions) THEN json_type(budget_extensions)='object' ELSE 0 END
          AND CASE WHEN json_valid(extensions) THEN json_type(extensions)='object' ELSE 0 END
          AND origin_kind NOT GLOB '*[^A-Z0-9_]*' AND substr(origin_kind,1,1) BETWEEN 'A' AND 'Z'
          AND (origin_device_id IS NULL OR (length(origin_device_id)=30 AND substr(origin_device_id,1,4)='dev_'))
          AND (origin_message_id IS NULL OR (length(origin_message_id)=30 AND substr(origin_message_id,1,4)='evt_'))
         FROM tasks WHERE task_id=?1", [task.as_str()], |r| r.get(0),
    ).map_err(StoreError::from)
}

fn result_refs_match(conn: &Connection, model: &TaskSnapshot) -> Result<bool, StoreError> {
    for step in &model.steps {
        let mut statement = conn.prepare(
            "SELECT digest,data_class_rank FROM step_blob_refs WHERE step_id=?1 AND role='RESULT' ORDER BY digest")?;
        let rows = statement
            .query_map([step.step.step_id.as_str()], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        let expected = step.step.result_digest.as_ref().map(|digest| {
            (
                digest.as_str().to_owned(),
                i64::from(model.task.data_class.rank()),
            )
        });
        if rows != expected.into_iter().collect::<Vec<_>>() {
            return Ok(false);
        }
    }
    Ok(true)
}

fn validate_related_classes(conn: &Connection, task: &TaskId) -> Result<(), StoreError> {
    let ranks = raw_rows(conn,
        "SELECT data_class_rank FROM side_effect_receipts WHERE task_id=?1
         UNION SELECT data_class_rank FROM plan_revisions WHERE task_id=?1
         UNION SELECT data_class_rank FROM task_blob_refs WHERE task_id=?1
         UNION SELECT r.data_class_rank FROM step_blob_refs r JOIN task_steps s ON s.step_id=r.step_id WHERE s.task_id=?1
         UNION SELECT data_class_rank FROM task_journal WHERE task_id=?1", task)?;
    for row in ranks {
        let Some(Value::Integer(rank)) = row.first() else {
            return Err(StoreError::IntegrityCheckFailed);
        };
        ordinary_class(*rank)?;
    }
    Ok(())
}

fn raw_rows(conn: &Connection, sql: &str, task: &TaskId) -> Result<Vec<Vec<Value>>, StoreError> {
    let mut statement = conn.prepare(sql)?;
    let columns = statement.column_count();
    let rows = statement
        .query_map([task.as_str()], |r| {
            (0..columns)
                .map(|i| r.get::<_, Value>(i))
                .collect::<Result<Vec<_>, _>>()
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

fn fingerprint(conn: &Connection, task: &TaskId) -> Result<Digest, StoreError> {
    // Typed, length-framed SQLite values preserve malformed raw text and blobs.
    // No JSON parsing, rowid dependence, recovery prose, actor or time identity.
    let mut hash = Sha256::new();
    hash.update(b"serea:p2g:recovery-snapshot:v1");
    let marker = recovery_edge_marker();
    let queries = [
        "SELECT * FROM tasks WHERE task_id=?1 ORDER BY task_id",
        "SELECT * FROM task_steps WHERE task_id=?1 ORDER BY step_id",
        "SELECT l.* FROM leases l JOIN task_steps s ON s.step_id=l.step_id WHERE s.task_id=?1 ORDER BY l.step_id",
        "SELECT * FROM side_effect_receipts WHERE task_id=?1 OR step_id IN (SELECT step_id FROM task_steps WHERE task_id=?1) ORDER BY receipt_id",
        "SELECT * FROM plan_revisions WHERE task_id=?1 ORDER BY plan_revision",
        "SELECT * FROM task_blob_refs WHERE task_id=?1 ORDER BY role,digest",
        "SELECT r.* FROM step_blob_refs r JOIN task_steps s ON s.step_id=r.step_id WHERE s.task_id=?1 ORDER BY r.step_id,r.role,r.digest",
        "SELECT b.* FROM blobs b WHERE (b.digest,b.data_class_rank) IN (
            SELECT digest,data_class_rank FROM task_blob_refs WHERE task_id=?1 UNION
            SELECT r.digest,r.data_class_rank FROM step_blob_refs r JOIN task_steps s ON s.step_id=r.step_id WHERE s.task_id=?1 UNION
            SELECT plan_digest,data_class_rank FROM plan_revisions WHERE task_id=?1 UNION
            SELECT input_digest,t.data_class_rank FROM task_steps s JOIN tasks t ON t.task_id=s.task_id WHERE s.task_id=?1 UNION
            SELECT result_digest,t.data_class_rank FROM task_steps s JOIN tasks t ON t.task_id=s.task_id WHERE s.task_id=?1)
            ORDER BY b.digest,b.data_class_rank",
    ];
    for (index, sql) in queries.iter().enumerate() {
        hash.update((index as u64).to_be_bytes());
        hash_rows(&mut hash, &raw_rows(conn, sql, task)?);
    }
    let journal = format!("SELECT * FROM task_journal WHERE task_id=?1 AND journal_kind<>'RECOVERY_DECISION'
        AND NOT (journal_kind IN ('TASK_STATE_CHANGED','TASK_TERMINAL') AND payload_ref_digest IS '{}') ORDER BY journal_seq", marker.as_str());
    hash_rows(&mut hash, &raw_rows(conn, &journal, task)?);
    let digest = hash.finalize();
    let mut encoded = String::from("sha256:");
    for byte in digest {
        use std::fmt::Write;
        write!(&mut encoded, "{byte:02x}").expect("String write");
    }
    Digest::new(encoded).map_err(|_| StoreError::CorruptRow)
}
fn hash_rows(hash: &mut Sha256, rows: &[Vec<Value>]) {
    hash.update((rows.len() as u64).to_be_bytes());
    for row in rows {
        hash.update((row.len() as u64).to_be_bytes());
        for value in row {
            let (tag, bytes): (u8, Vec<u8>) = match value {
                Value::Null => (0, vec![]),
                Value::Integer(n) => (1, n.to_be_bytes().to_vec()),
                Value::Real(n) => (2, n.to_bits().to_be_bytes().to_vec()),
                Value::Text(s) => (3, s.as_bytes().to_vec()),
                Value::Blob(b) => (4, b.clone()),
            };
            hash.update([tag]);
            hash.update((bytes.len() as u64).to_be_bytes());
            hash.update(bytes);
        }
    }
}

#[cfg(test)]
#[path = "recovery_tests.rs"]
mod recovery_tests;
