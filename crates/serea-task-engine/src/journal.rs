use crate::task_transition_reason;
use serea_protocol::ReasonCode;
use serea_storage::{
    AuditOperation, DurableTransition, JournalKind, JournalRecord, JournalRecords, StoreError,
    TaskAuditParticipant,
};

/// The single production journal-semantic authority. Storage constructs actual
/// facts and privately persists these ordered drafts inside the same savepoint.
#[derive(Debug, Clone, Copy, Default)]
pub struct TaskJournal;

impl TaskAuditParticipant for TaskJournal {
    fn records(&self, facts: &DurableTransition) -> Result<JournalRecords, StoreError> {
        // Only validated digests and numeric evidence are rendered; no ordinary
        // prose or raw effect payload is copied into journal diagnostics.
        let payload = format!(
            "{{\"attempt\":{},\"generation\":{},\"result_digest\":{}}}",
            facts
                .attempt()
                .map_or_else(|| "null".into(), |n| n.to_string()),
            facts
                .generation()
                .map_or_else(|| "null".into(), |n| n.to_string()),
            facts
                .result()
                .map_or_else(|| "null".into(), |d| format!("\"{}\"", d.as_str())),
        )
        .into_bytes();
        let step = |kind| JournalRecord {
            kind,
            state_from: facts.step_from().map(|s| s.as_str().to_owned()),
            state_to: facts.step_to().map(|s| s.as_str().to_owned()),
            reason: facts.reason().cloned(),
            payload_json: payload.clone(),
        };
        // P2F-a outcomes retain their existing null-or-specific-cause reason and
        // exact payload. Other task operations use the authoritative typed edge map.
        let reason = match facts.reason() {
            Some(cause) => Some(cause.clone()),
            None if matches!(
                facts.operation(),
                AuditOperation::AttemptStarted
                    | AuditOperation::StepSucceeded
                    | AuditOperation::StepFailed
            ) =>
            {
                None
            }
            None => facts
                .task_from()
                .and_then(|from| task_transition_reason(from, facts.task_to()))
                .map(|rule| {
                    ReasonCode::new(rule.wire_name()).map_err(|_| StoreError::AuditRejected)
                })
                .transpose()?,
        };
        let task = |kind| JournalRecord {
            kind,
            state_from: facts.task_from().map(|s| s.wire_name().to_owned()),
            state_to: Some(facts.task_to().wire_name().to_owned()),
            reason: reason.clone(),
            payload_json: match facts.revision() {
                Some(revision) if facts.operation() == AuditOperation::PlanPersisted => {
                    format!("{{\"revision\":{revision}}}").into_bytes()
                }
                _ => payload.clone(),
            },
        };
        let mut rows = match facts.operation() {
            AuditOperation::TaskInserted => vec![task(JournalKind::TaskInserted)],
            AuditOperation::PlanPersisted => vec![task(JournalKind::PlanPersisted)],
            AuditOperation::LeaseAcquired => vec![step(JournalKind::StepLeaseAcquired)],
            AuditOperation::LeaseReleased => vec![step(JournalKind::StepLeaseReleased)],
            AuditOperation::AttemptStarted => vec![step(JournalKind::StepAttemptStarted)],
            AuditOperation::StepSucceeded => {
                let mut rows = vec![step(JournalKind::StepCommitted)];
                if facts.receipt_id().is_some() {
                    rows.push(step(JournalKind::ReceiptRecorded));
                }
                rows
            }
            AuditOperation::StepFailed => vec![step(JournalKind::StepFailed)],
            AuditOperation::Cancelled => vec![task(JournalKind::TaskCancelRequested)],
            AuditOperation::PlanningStarted
            | AuditOperation::Blocked
            | AuditOperation::InvariantFailed => vec![],
        };
        if let Some(from) = facts.task_from().filter(|from| *from != facts.task_to()) {
            if !crate::legal_task_transition(from, facts.task_to()) {
                return Err(StoreError::AuditRejected);
            }
            rows.push(task(JournalKind::TaskStateChanged));
            if facts.task_to().is_terminal() {
                rows.push(task(JournalKind::TaskTerminal));
            }
        }
        Ok(rows)
    }
}
