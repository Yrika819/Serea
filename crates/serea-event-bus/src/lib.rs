//! Event semantics and identifier minting over Storage's private event sink.
#![forbid(unsafe_code)]

use std::sync::Mutex;

use serde_json::{Map, Value};
use serea_protocol::{
    Actor, EnvelopeVersion, EventKind, IdMinter, Seq, SereaEvent, TaskState, Trace, UlidSource,
    WireSurface,
};
use serea_storage::{AuditOperation, DurableTransition, EventParticipant, StoreError, Tx};

struct ErasedUlidSource(Box<dyn UlidSource + Send>);

impl UlidSource for ErasedUlidSource {
    fn next_ulid(&mut self) -> serea_protocol::UlidValue {
        self.0.next_ulid()
    }
}

/// Event semantic mapper with an injected ULID source. The source is required
/// so EventId minting is deterministic in tests and explicit at runtime. The
/// source is erased internally so Task Engine accepts only this fixed mapper,
/// not an arbitrary event callback implementation.
pub struct EventBus {
    ids: Mutex<IdMinter<ErasedUlidSource>>,
}

impl EventBus {
    pub fn new<S: UlidSource + Send + 'static>(source: S) -> Self {
        Self {
            ids: Mutex::new(IdMinter::new(ErasedUlidSource(Box::new(source)))),
        }
    }

    /// Appends an independently authored event in the caller's transaction.
    pub fn append(
        tx: &mut Tx<'_>,
        event: SereaEvent,
        retention_at: Option<serea_protocol::EpochMillis>,
    ) -> Result<SereaEvent, StoreError> {
        tx.append_event(event, retention_at)
    }
}

impl EventParticipant for EventBus {
    fn events(&self, facts: &DurableTransition) -> Result<Vec<SereaEvent>, StoreError> {
        let Some(kind) = task_event_kind(facts) else {
            return Ok(Vec::new());
        };
        let message_id = self
            .ids
            .lock()
            .map_err(|_| StoreError::LockPoisoned)?
            .next_event_id();
        let mut payload = Map::new();
        payload.insert(
            "task_id".into(),
            Value::String(facts.task_id().as_str().to_owned()),
        );
        if matches!(kind, EventKind::TaskStateChanged | EventKind::TaskResumed) {
            if let Some(from) = facts.task_from() {
                payload.insert("from".into(), Value::String(from.wire_name().into()));
            }
            payload.insert(
                "to".into(),
                Value::String(facts.task_to().wire_name().into()),
            );
        }
        match kind {
            EventKind::TaskFailed => {
                if let Some(reason) = facts.reason() {
                    payload.insert(
                        "reason_code".into(),
                        Value::String(reason.as_str().to_owned()),
                    );
                }
            }
            EventKind::TaskBlocked => {
                if let Some(reason) = facts.reason() {
                    payload.insert(
                        "blocked_reason".into(),
                        Value::String(reason.as_str().to_owned()),
                    );
                }
            }
            EventKind::TaskCompleted => {
                if let Some(result) = facts.result() {
                    payload.insert(
                        "result_digest".into(),
                        Value::String(result.as_str().to_owned()),
                    );
                }
            }
            EventKind::TaskCancelled => {
                if let Some(by) = facts.cancelled_by() {
                    payload.insert("cancelled_by".into(), Value::String(by.as_str().to_owned()));
                }
            }
            _ => {}
        }
        let surface =
            WireSurface::new(WireSurface::EVENT).map_err(|_| StoreError::AuditRejected)?;
        let envelope_version = EnvelopeVersion::new("1").map_err(|_| StoreError::AuditRejected)?;
        Ok(vec![SereaEvent {
            envelope_version,
            surface,
            message_id,
            seq: Seq::new(0),
            kind,
            occurred_at: serea_protocol::Timestamp::from_epoch_millis(facts.now()),
            correlation_id: Some(facts.task_id().clone()),
            causation_id: facts.causation_id().cloned(),
            actor: Actor {
                kind: facts.actor_kind(),
                id: facts.actor_id().clone(),
                version: facts.actor_version().clone(),
                extensions: Default::default(),
            },
            data_class: facts.data_class(),
            trace: Some(Trace {
                task_id: Some(facts.task_id().clone()),
                step_id: facts.step_id().cloned(),
                attempt: facts.attempt(),
                extensions: Default::default(),
            }),
            payload,
            extensions: Default::default(),
        }])
    }
}

fn task_event_kind(facts: &DurableTransition) -> Option<EventKind> {
    let from = facts.task_from();
    let to = facts.task_to();
    match facts.operation() {
        AuditOperation::TaskInserted => Some(EventKind::TaskCreated),
        AuditOperation::PlanningStarted => match from? {
            TaskState::Received => Some(EventKind::TaskStarted),
            TaskState::Blocked | TaskState::WaitingUser => Some(EventKind::TaskResumed),
            _ if from != Some(to) => Some(EventKind::TaskStateChanged),
            _ => None,
        },
        AuditOperation::PlanPersisted => Some(EventKind::TaskStateChanged),
        AuditOperation::LeaseAcquired | AuditOperation::LeaseReleased => None,
        AuditOperation::AttemptStarted
        | AuditOperation::StepSucceeded
        | AuditOperation::StepFailed
        | AuditOperation::RecoveryStateChanged => {
            if to == TaskState::Blocked {
                Some(EventKind::TaskBlocked)
            } else if to == TaskState::Failed {
                Some(EventKind::TaskFailed)
            } else if to == TaskState::Completed {
                Some(EventKind::TaskCompleted)
            } else if from != Some(to) {
                if matches!(
                    from,
                    Some(TaskState::Blocked | TaskState::WaitingUser | TaskState::WaitingApproval)
                ) {
                    Some(EventKind::TaskResumed)
                } else if from.is_none() && to == TaskState::Planning {
                    Some(EventKind::TaskStarted)
                } else {
                    Some(EventKind::TaskStateChanged)
                }
            } else {
                None
            }
        }
        AuditOperation::Blocked => Some(EventKind::TaskBlocked),
        AuditOperation::InvariantFailed => Some(EventKind::TaskFailed),
        AuditOperation::Cancelled => Some(EventKind::TaskCancelled),
        AuditOperation::RecoveryDecision => None,
    }
}
