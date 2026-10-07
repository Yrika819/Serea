use crate::{EngineError, TaskEngine, TaskJournal, TransitionContext};
use serea_protocol::{EpochMillis, ReasonCode, ReceiptId, StepId, StepKind, TaskId, TaskState};
use serea_storage::{RecoveryAction, RecoveryPass, RecoverySnapshot, StoreError};

/// Ordered classifications of durable work, not requests to execute it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecoveryDecision {
    TerminalNoop {
        task_id: TaskId,
        state: TaskState,
    },
    ResumeNormally {
        task_id: TaskId,
        next_step_id: Option<StepId>,
    },
    HeldLease {
        task_id: TaskId,
        step_id: StepId,
    },
    ExpiredLease {
        task_id: TaskId,
        step_id: StepId,
    },
    NeedsReconciliation {
        task_id: TaskId,
        step_id: StepId,
    },
    AwaitApproval {
        task_id: TaskId,
        step_id: Option<StepId>,
    },
    AwaitUser {
        task_id: TaskId,
    },
    BlockedTask {
        task_id: TaskId,
    },
    ReceiptAlreadyCommitted {
        task_id: TaskId,
        step_id: StepId,
        receipt_id: ReceiptId,
    },
    CorruptOrInvariantViolation {
        task_id: TaskId,
        reason: ReasonCode,
    },
}

/// Published only after the entire recovery transaction commits.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RecoveryReport {
    /// Inspected task identities, including terminal tasks.
    pub tasks_examined: u64,
    /// Distinct tasks eligible to resume; recovery itself executes nothing.
    pub tasks_resumed: u64,
    /// Distinct tasks changed, including first-time decision audit evidence.
    pub repairs_committed: u64,
    /// Distinct tasks with attributable semantic corruption.
    pub invariant_violations: u64,
    /// All committed journal rows, not a delivery queue or outbox.
    pub pending_event_transitions: u64,
    pub decisions: Vec<RecoveryDecision>,
}

impl TaskEngine {
    /// Inspect and recover one coherent durable snapshot using only explicit time.
    /// Any refusal rolls back the whole pass; no partial report is published.
    pub fn recover(
        &mut self,
        now: EpochMillis,
        context: &TransitionContext<'_>,
    ) -> Result<RecoveryReport, EngineError> {
        self.store
            .transact_with_participants(&TaskJournal, &self.event_bus, |tx| {
                tx.recovery_pass(|tx| {
                    let mut report = RecoveryReport::default();
                    for task_id in tx.recovery_tasks()? {
                        report.tasks_examined += 1;
                        recover_task(tx, &task_id, now, context, &mut report)?;
                    }
                    report.pending_event_transitions = tx.recovery_journal_count()?;
                    Ok(report)
                })
            })
            .map_err(EngineError::from)
    }
}

fn recover_task(
    tx: &mut RecoveryPass<'_, '_>,
    task_id: &TaskId,
    now: EpochMillis,
    context: &TransitionContext<'_>,
    report: &mut RecoveryReport,
) -> Result<(), StoreError> {
    let mut snapshot = tx.inspect_recovery_task(task_id)?;
    if let Some(state) = snapshot.state().filter(|state| state.is_terminal()) {
        report.decisions.push(RecoveryDecision::TerminalNoop {
            task_id: task_id.clone(),
            state,
        });
        return Ok(());
    }

    let mut changed = false;
    if let Some(repair) = snapshot.receipt_repair() {
        let step_id = repair.step_id().clone();
        let receipt_id = repair.receipt_id().clone();
        apply(
            tx,
            &snapshot,
            RecoveryAction::ReceiptRepair {
                step_id: step_id.clone(),
            },
            now,
            context,
            report,
            &mut changed,
        )?;
        snapshot = tx.inspect_recovery_task(task_id)?;
        if snapshot.state().is_some_and(TaskState::is_terminal) {
            // The proven repair already recorded receipt evidence. Terminal
            // precedence forbids even decision writes on the resulting task.
            report
                .decisions
                .push(RecoveryDecision::ReceiptAlreadyCommitted {
                    task_id: task_id.clone(),
                    step_id,
                    receipt_id,
                });
            report.repairs_committed += u64::from(changed);
            return Ok(());
        }
        if snapshot.receipt_repair().is_some() {
            return Err(StoreError::InvalidRecoveryAction);
        }
    }

    let action = classify(&snapshot, now)?;
    let decision = public_decision(task_id, &action)?;
    report.tasks_resumed += u64::from(matches!(action, RecoveryAction::ResumeNormally { .. }));
    report.invariant_violations += u64::from(matches!(action, RecoveryAction::Quarantine { .. }));
    apply(tx, &snapshot, action, now, context, report, &mut changed)?;
    report.decisions.push(decision);
    snapshot = tx.inspect_recovery_task(task_id)?;

    if snapshot.state() == Some(TaskState::Blocked)
        && !matches!(
            report.decisions.last(),
            Some(RecoveryDecision::BlockedTask { .. })
        )
    {
        // Storage records this stable evidence when blocking. For an already
        // blocked task with uncertain/held work, record both classifications.
        apply(
            tx,
            &snapshot,
            RecoveryAction::BlockedTask,
            now,
            context,
            report,
            &mut changed,
        )?;
        report.decisions.push(RecoveryDecision::BlockedTask {
            task_id: task_id.clone(),
        });
        snapshot = tx.inspect_recovery_task(task_id)?;
    }

    // Observe receipts against FINAL facts. Recording pre-block/pre-repair facts
    // would create a new decision on the next unchanged pass.
    if let Some(model) = snapshot.projection() {
        let receipts: Vec<_> = model
            .steps
            .iter()
            .filter_map(|record| {
                if record.step.status.as_str() != "SUCCEEDED" {
                    return None;
                }
                record
                    .step
                    .side_effect_receipt
                    .as_ref()
                    .map(|receipt| (record.step.step_id.clone(), receipt.receipt_id.clone()))
            })
            .collect();
        for (step_id, receipt_id) in receipts {
            apply(
                tx,
                &snapshot,
                RecoveryAction::ReceiptAlreadyCommitted {
                    step_id: step_id.clone(),
                },
                now,
                context,
                report,
                &mut changed,
            )?;
            report
                .decisions
                .push(RecoveryDecision::ReceiptAlreadyCommitted {
                    task_id: task_id.clone(),
                    step_id,
                    receipt_id,
                });
            snapshot = tx.inspect_recovery_task(task_id)?;
        }
    }
    report.repairs_committed += u64::from(changed);
    Ok(())
}

fn apply(
    tx: &mut RecoveryPass<'_, '_>,
    snapshot: &RecoverySnapshot,
    action: RecoveryAction,
    now: EpochMillis,
    context: &TransitionContext<'_>,
    report: &mut RecoveryReport,
    changed: &mut bool,
) -> Result<(), StoreError> {
    let applied = tx.apply_recovery(snapshot, action, now, context)?;
    *changed |= applied.changed;
    for step_id in applied.revoked_steps {
        report.decisions.push(RecoveryDecision::ExpiredLease {
            task_id: snapshot.task_id().clone(),
            step_id,
        });
    }
    Ok(())
}

fn classify(snapshot: &RecoverySnapshot, now: EpochMillis) -> Result<RecoveryAction, StoreError> {
    if let Some(reason) = snapshot.corruption() {
        return Ok(RecoveryAction::Quarantine {
            reason: reason.clone(),
        });
    }
    let state = snapshot.state().ok_or(StoreError::CorruptRow)?;
    let model = snapshot.projection().ok_or(StoreError::CorruptRow)?;

    // Storage validates attribution. Expiry is effective release for selecting
    // an action, but only apply_recovery may actually revoke that authority.
    if let Some(step) = snapshot.steps().iter().find(|step| {
        step.authority().is_some_and(|authority| {
            authority.released_at().is_none() && authority.expires_at() > now
        })
    }) {
        return Ok(RecoveryAction::HeldLease {
            step_id: step.step_id().clone(),
        });
    }

    match state {
        TaskState::WaitingApproval => {
            let step_id = model
                .steps
                .iter()
                .find(|record| {
                    record.step.kind == StepKind::WaitApproval
                        && record.step.status.as_str() == "WAITING"
                })
                .map(|record| record.step.step_id.clone());
            return Ok(RecoveryAction::AwaitApproval { step_id });
        }
        TaskState::WaitingUser => {
            let step_id = model
                .steps
                .iter()
                .find(|record| {
                    matches!(
                        record.step.kind,
                        StepKind::WaitUser | StepKind::WaitSchedule
                    ) && record.step.status.as_str() == "WAITING"
                })
                .map(|record| record.step.step_id.clone());
            return Ok(RecoveryAction::AwaitUser { step_id });
        }
        _ => (),
    }

    // Uncertain work anywhere in the plan must not be bypassed by a planned
    // successor. A closed absence is not a successful predecessor.
    if let Some(record) = model.steps.iter().find(|record| {
        matches!(
            record.step.status.as_str(),
            "EXECUTING" | "RECONCILED_ABSENT"
        ) || record.step.status.as_str() == "LEASED"
            && (state == TaskState::Blocked
                || record.step.attempt >= model.task.attempt_budget.max_attempts_per_step)
    }) {
        return Ok(RecoveryAction::NeedsReconciliation {
            step_id: record.step.step_id.clone(),
            block: record.step.status.as_str() != "RECONCILED_ABSENT"
                && record.step.attempt >= model.task.attempt_budget.max_attempts_per_step,
        });
    }
    if state == TaskState::Blocked {
        return Ok(RecoveryAction::BlockedTask);
    }

    let next = model
        .steps
        .iter()
        .find(|record| record.step.status.as_str() != "SUCCEEDED");
    if next.is_some_and(|record| {
        !matches!(record.step.status.as_str(), "PLANNED" | "LEASED")
            || record.step.attempt >= model.task.attempt_budget.max_attempts_per_step
    }) {
        // No truthful writer/action exists for this ordinary-row combination.
        // Refuse the whole pass rather than silently skipping it or guessing.
        return Err(StoreError::InvalidRecoveryAction);
    }
    match state {
        TaskState::Received
        | TaskState::Planning
        | TaskState::Ready
        | TaskState::Executing
        | TaskState::Verifying => Ok(RecoveryAction::ResumeNormally {
            next_step_id: next.map(|record| record.step.step_id.clone()),
        }),
        TaskState::WaitingApproval
        | TaskState::WaitingUser
        | TaskState::Blocked
        | TaskState::Completed
        | TaskState::Failed
        | TaskState::Cancelled => Err(StoreError::InvalidRecoveryAction),
    }
}

fn public_decision(
    task_id: &TaskId,
    action: &RecoveryAction,
) -> Result<RecoveryDecision, StoreError> {
    let task_id = task_id.clone();
    Ok(match action {
        RecoveryAction::ResumeNormally { next_step_id } => RecoveryDecision::ResumeNormally {
            task_id,
            next_step_id: next_step_id.clone(),
        },
        RecoveryAction::HeldLease { step_id } => RecoveryDecision::HeldLease {
            task_id,
            step_id: step_id.clone(),
        },
        RecoveryAction::NeedsReconciliation { step_id, .. } => {
            RecoveryDecision::NeedsReconciliation {
                task_id,
                step_id: step_id.clone(),
            }
        }
        RecoveryAction::AwaitApproval { step_id } => RecoveryDecision::AwaitApproval {
            task_id,
            step_id: step_id.clone(),
        },
        RecoveryAction::AwaitUser { .. } => RecoveryDecision::AwaitUser { task_id },
        RecoveryAction::BlockedTask => RecoveryDecision::BlockedTask { task_id },
        RecoveryAction::Quarantine { reason } => RecoveryDecision::CorruptOrInvariantViolation {
            task_id,
            reason: reason.clone(),
        },
        RecoveryAction::ReceiptAlreadyCommitted { .. } | RecoveryAction::ReceiptRepair { .. } => {
            return Err(StoreError::InvalidRecoveryAction);
        }
    })
}
