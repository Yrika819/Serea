use crate::audit::{AuditOperation, DurableTransition};
use crate::{EventDraft, StoreError, TransitionContext, Tx};
use rusqlite::Connection;
use serea_protocol::{
    ApprovalId, ApprovalLifecyclePayloadV1, DataClass, DeviceId, Digest, EpochMillis, EventId,
    EventKind, ScheduleId, ScheduledTaskTemplateV1, StepId, TaskId, canonicalize,
};

const MAX_LEASE_MS: i64 = 120_000;
const MAX_KEY_BYTES: usize = 512;
const MAX_OWNER_BYTES: usize = 128;
const MAX_ACTIVE_SCHEDULES: i64 = 256;
const MAX_PENDING_OCCURRENCES: i64 = 256;
const MAX_SCHEDULER_EVENT_SCAN_PAGE: u16 = 256;
const MAX_RECOVERY_ROWS_PER_BATCH: u16 = 512;
type ClaimOccurrenceRow = (String, i64, Option<i64>, Option<String>, i64);
type OccurrenceMappingRow = (String, Option<String>, i64, Option<i64>, i64);
type ScheduleProjectionRow = (
    String,
    i64,
    String,
    String,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<i64>,
    i64,
    String,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<i64>,
    Option<String>,
    String,
    Option<i64>,
);
type ScheduleOccurrenceWorkRow = (
    String,
    Option<String>,
    Option<i64>,
    i64,
    Option<i64>,
    Option<String>,
    Option<i64>,
    i64,
    Option<String>,
);
type ScheduleTaskProvenanceRow = (
    String,
    String,
    Option<String>,
    Option<String>,
    Option<String>,
    String,
    i64,
);
type ScheduleTaskEventValidationRow = (Option<String>, i64, Option<i64>, Option<String>, i64);
type DeviceWakeValidationRow = (
    i64,
    String,
    i64,
    i64,
    i64,
    Option<String>,
    String,
    i64,
    Option<String>,
    Option<i64>,
);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScheduleOwnerKind {
    Host,
    Device,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScheduleTriggerKind {
    Calendar,
    HostEvent,
    DeviceSessionEstablished,
    ApprovalEvent,
}

impl ScheduleTriggerKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::Calendar => "CALENDAR",
            Self::HostEvent => "HOST_EVENT",
            Self::DeviceSessionEstablished => "DEVICE_SESSION_ESTABLISHED",
            Self::ApprovalEvent => "APPROVAL_EVENT",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MissedOccurrencePolicy {
    Skip,
    RunOnce,
    RunEach,
}

impl MissedOccurrencePolicy {
    fn as_str(self) -> &'static str {
        match self {
            Self::Skip => "SKIP",
            Self::RunOnce => "RUN_ONCE",
            Self::RunEach => "RUN_EACH",
        }
    }
}

/// Validated-at-the-boundary schedule data needed by migration 0002. The
/// Scheduler owns semantic validation; Storage enforces durable shape, caps,
/// and transaction ordering.
#[derive(Debug, Clone)]
pub struct ScheduleDraft {
    pub schedule_id: ScheduleId,
    pub owner_kind: ScheduleOwnerKind,
    pub owner_id: String,
    pub trigger_kind: ScheduleTriggerKind,
    pub recurrence: Option<serde_json::Value>,
    pub event_predicate: Option<serde_json::Value>,
    pub template_json: Vec<u8>,
    pub template_data_class: DataClass,
    pub policy_class_rank: u8,
    pub approval_policy: serde_json::Value,
    pub timezone: Option<String>,
    pub recurrence_evaluator: Option<String>,
    pub tzdb_version: Option<String>,
    pub next_due_at: Option<EpochMillis>,
    pub next_local_label: Option<String>,
    pub missed_policy: MissedOccurrencePolicy,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScheduleSnapshot {
    pub schedule_id: ScheduleId,
    pub revision: u32,
    pub state: ScheduleCommandState,
    pub trigger_kind: ScheduleTriggerKind,
    pub recurrence_json: Option<String>,
    pub event_predicate_json: Option<String>,
    /// Event sequence high-water captured when this predicate revision became
    /// durable. Older committed events are not retroactively matched on replay.
    pub event_predicate_after_seq: Option<u64>,
    pub template: Option<crate::BlobRef>,
    pub policy_class_rank: u8,
    pub approval_policy_json: String,
    pub timezone: Option<String>,
    pub recurrence_evaluator: Option<String>,
    pub tzdb_version: Option<String>,
    pub next_due_at: Option<EpochMillis>,
    pub next_local_label: Option<String>,
    pub missed_policy: MissedOccurrencePolicy,
}

/// Durable reverse provenance needed to find the historical template for a
/// mapped scheduled task after a schedule edit or restart.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScheduleTaskProvenance {
    pub schedule_id: ScheduleId,
    pub occurrence_key: String,
    pub task_id: TaskId,
    pub source_event_id: Option<EventId>,
    pub intended_local_label: Option<String>,
    pub timezone: Option<String>,
    pub template: crate::BlobRef,
}

/// Immutable occurrence input for scheduled task creation. The template blob
/// reference is captured when the occurrence is admitted, never read from the
/// schedule's latest revision.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScheduleOccurrenceWork {
    pub schedule_id: ScheduleId,
    pub occurrence_key: String,
    pub source_event_id: Option<EventId>,
    pub source_event_data_class: Option<DataClass>,
    pub template: crate::BlobRef,
    pub policy_class_rank: u8,
}

/// One due, unclaimed occurrence in deterministic due-time order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScheduleDueOccurrence {
    pub occurrence_key: String,
    pub schedule_revision: u32,
    pub due_at: EpochMillis,
}

/// Pending HOST_EVENT occurrence selected for bounded Scheduler dispatch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingHostEventOccurrence {
    pub schedule_id: ScheduleId,
    pub occurrence_key: String,
    pub schedule_revision: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecoverableScheduleOccurrence {
    pub schedule_id: ScheduleId,
    pub occurrence_key: String,
    pub schedule_revision: u32,
    pub due_at: EpochMillis,
}

/// One durable occurrence identity. The Scheduler resolves recurrence and
/// predicate semantics before calling this typed Storage admission method.
#[derive(Debug, Clone)]
pub struct ScheduleOccurrenceDraft {
    pub schedule_id: ScheduleId,
    pub occurrence_key: String,
    pub schedule_revision: u32,
    pub trigger_kind: ScheduleTriggerKind,
    pub source_event_id: Option<EventId>,
    pub source_event_data_class: Option<DataClass>,
    pub intended_local_label: Option<String>,
    pub timezone: Option<String>,
    pub recurrence_evaluator: Option<String>,
    pub tzdb_version: Option<String>,
    pub due_at: Option<EpochMillis>,
    pub not_before: Option<EpochMillis>,
    pub created_at: EpochMillis,
}

/// Storage-authoritative Scheduler occurrence fence. It carries only the
/// schedule/occurrence identity and generation required to reject stale work.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScheduleOccurrenceLease {
    pub schedule_id: ScheduleId,
    pub occurrence_key: String,
    pub schedule_revision: u32,
    pub lease_owner: String,
    pub lease_generation: u32,
    pub lease_expires_at: EpochMillis,
}

/// Explicit durable authority for one task to resume when one device reconnects.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceResumeWait {
    pub task_id: TaskId,
    pub device_id: DeviceId,
    pub blocked_task_revision: u64,
    pub registration_event_high_water_seq: u64,
    pub created_at: EpochMillis,
}

/// Durable internal work materialized from a DEVICE_CONNECTED event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceSessionResumeWake {
    pub source_event_id: EventId,
    pub source_seq: u64,
    pub task_id: TaskId,
    pub device_id: DeviceId,
    pub blocked_task_revision: u64,
    pub created_at: EpochMillis,
}

/// Closed kind of approval lifecycle event carried only for future P6 routing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApprovalLifecycleOutcome {
    Granted,
    Denied,
    Expired,
}

impl ApprovalLifecycleOutcome {
    fn as_str(self) -> &'static str {
        match self {
            Self::Granted => "GRANTED",
            Self::Denied => "DENIED",
            Self::Expired => "EXPIRED",
        }
    }

    fn from_str(value: &str) -> Result<Self, StoreError> {
        match value {
            "GRANTED" => Ok(Self::Granted),
            "DENIED" => Ok(Self::Denied),
            "EXPIRED" => Ok(Self::Expired),
            _ => Err(StoreError::CorruptRow),
        }
    }
}

/// Durable routing-only handoff of one approval lifecycle event. It carries no
/// ApprovalGrant, policy decision, or task-transition authority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApprovalLifecycleWake {
    pub source_event_id: EventId,
    pub source_seq: u64,
    pub approval_id: ApprovalId,
    pub task_id: TaskId,
    pub step_id: StepId,
    pub outcome: ApprovalLifecycleOutcome,
    pub created_at: EpochMillis,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScheduleCommandOutcome {
    pub schedule_id: ScheduleId,
    pub revision: u32,
    pub state: ScheduleCommandState,
    pub replayed: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScheduleCommandState {
    Active,
    Paused,
    Cancelled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScheduleStateCommand {
    Pause,
    Resume,
    Cancel,
}

pub struct ScheduleStateCommandRequest {
    pub message_id: EventId,
    pub request_digest: Digest,
    pub schedule_id: ScheduleId,
    pub expected_revision: u32,
    pub command: ScheduleStateCommand,
    pub now: EpochMillis,
    pub event: EventDraft,
}

impl ScheduleStateCommand {
    fn command_kind(self) -> &'static str {
        match self {
            Self::Pause => "PAUSE",
            Self::Resume => "RESUME",
            Self::Cancel => "CANCEL",
        }
    }

    fn result_state(self) -> ScheduleCommandState {
        match self {
            Self::Pause => ScheduleCommandState::Paused,
            Self::Resume => ScheduleCommandState::Active,
            Self::Cancel => ScheduleCommandState::Cancelled,
        }
    }

    fn event_kind(self) -> EventKind {
        match self {
            Self::Pause => EventKind::SchedulePaused,
            Self::Resume => EventKind::ScheduleResumed,
            Self::Cancel => EventKind::ScheduleCancelled,
        }
    }
}

fn command_state_as_str(state: ScheduleCommandState) -> &'static str {
    match state {
        ScheduleCommandState::Active => "ACTIVE",
        ScheduleCommandState::Paused => "PAUSED",
        ScheduleCommandState::Cancelled => "CANCELLED",
    }
}

fn trigger_from_db(value: &str) -> Result<ScheduleTriggerKind, StoreError> {
    match value {
        "CALENDAR" => Ok(ScheduleTriggerKind::Calendar),
        "HOST_EVENT" => Ok(ScheduleTriggerKind::HostEvent),
        "DEVICE_SESSION_ESTABLISHED" => Ok(ScheduleTriggerKind::DeviceSessionEstablished),
        "APPROVAL_EVENT" => Ok(ScheduleTriggerKind::ApprovalEvent),
        _ => Err(StoreError::CorruptRow),
    }
}

fn data_class_from_rank(rank: u8) -> Result<DataClass, StoreError> {
    match rank {
        0 => Ok(DataClass::Public),
        1 => Ok(DataClass::Personal),
        2 => Ok(DataClass::Private),
        _ => Err(StoreError::CorruptRow),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SchedulerConsumerLease {
    pub owner: String,
    pub generation: u32,
    pub expires_at: EpochMillis,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SchedulerCursorSnapshot {
    pub last_processed_seq: u64,
    pub replay_high_water_seq: Option<u64>,
}

impl Tx<'_> {
    /// Lists bounded expired calendar claims for startup reconciliation.
    /// Pending work is handled by the normal bounded due pass; unexpired claims
    /// are deliberately excluded.
    pub fn expired_claimed_schedule_occurrences(
        &self,
        now: EpochMillis,
        limit: u16,
    ) -> Result<Vec<RecoverableScheduleOccurrence>, StoreError> {
        self.ensure_active()?;
        if !(1..=MAX_RECOVERY_ROWS_PER_BATCH).contains(&limit) {
            return Err(StoreError::InvalidSchedule);
        }
        let rows = {
            let mut statement = self.inner.prepare(
                "SELECT o.schedule_id,o.occurrence_key,o.schedule_revision,o.due_at_ms
                 FROM schedule_occurrences o JOIN schedules s USING(schedule_id)
                 WHERE s.state='ACTIVE' AND o.trigger_kind='CALENDAR'
                   AND o.due_at_ms<=?1
                   AND (o.not_before_ms IS NULL OR o.not_before_ms<=?1)
                   AND o.state='CLAIMED' AND o.lease_expires_at_ms<=?1
                 ORDER BY o.due_at_ms,o.schedule_id,o.occurrence_key LIMIT ?2",
            )?;
            statement
                .query_map(rusqlite::params![now.get(), i64::from(limit)], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, i64>(2)?,
                        row.get::<_, i64>(3)?,
                    ))
                })?
                .collect::<Result<Vec<_>, _>>()?
        };
        rows.into_iter()
            .map(|(schedule, key, revision, due)| {
                Ok(RecoverableScheduleOccurrence {
                    schedule_id: ScheduleId::new(schedule).map_err(|_| StoreError::CorruptRow)?,
                    occurrence_key: key,
                    schedule_revision: u32::try_from(revision)
                        .map_err(|_| StoreError::CorruptRow)?,
                    due_at: EpochMillis::new(due).map_err(|_| StoreError::CorruptRow)?,
                })
            })
            .collect()
    }

    /// Lists bounded pending occurrences whose due and retry times have passed.
    pub fn due_schedule_occurrences(
        &self,
        schedule_id: &ScheduleId,
        now: EpochMillis,
        limit: u16,
    ) -> Result<Vec<ScheduleDueOccurrence>, StoreError> {
        self.ensure_active()?;
        if !(1..=MAX_PENDING_OCCURRENCES as u16).contains(&limit) {
            return Err(StoreError::InvalidSchedule);
        }
        let rows = {
            let mut statement = self.inner.prepare(
                "SELECT occurrence_key,schedule_revision,due_at_ms
                 FROM schedule_occurrences
                 WHERE schedule_id=?1 AND state='PENDING'
                   AND (due_at_ms IS NULL OR due_at_ms<=?2)
                   AND (not_before_ms IS NULL OR not_before_ms<=?2)
                 ORDER BY due_at_ms,occurrence_key LIMIT ?3",
            )?;
            statement
                .query_map(
                    rusqlite::params![schedule_id.as_str(), now.get(), i64::from(limit)],
                    |row| {
                        Ok((
                            row.get::<_, String>(0)?,
                            row.get::<_, i64>(1)?,
                            row.get::<_, Option<i64>>(2)?,
                        ))
                    },
                )?
                .collect::<Result<Vec<_>, _>>()?
        };
        rows.into_iter()
            .map(|(occurrence_key, revision, due)| {
                Ok(ScheduleDueOccurrence {
                    occurrence_key,
                    schedule_revision: u32::try_from(revision)
                        .map_err(|_| StoreError::CorruptRow)?,
                    due_at: EpochMillis::new(due.ok_or(StoreError::CorruptRow)?)
                        .map_err(|_| StoreError::CorruptRow)?,
                })
            })
            .collect()
    }

    /// Returns whether a schedule has pending work fenced behind a future
    /// `not_before` instant. This prevents retry callers from busy-looping or
    /// inventing a replacement occurrence before its continuation is due.
    pub fn schedule_has_future_retry(
        &self,
        schedule_id: &ScheduleId,
        now: EpochMillis,
    ) -> Result<bool, StoreError> {
        self.ensure_active()?;
        self.inner
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM schedule_occurrences
                 WHERE schedule_id=?1 AND state='PENDING' AND not_before_ms>?2)",
                rusqlite::params![schedule_id.as_str(), now.get()],
                |row| row.get(0),
            )
            .map_err(StoreError::from)
    }

    /// Loads the task's current explicit device-session wait, if any.
    pub fn device_resume_wait(
        &self,
        task_id: &TaskId,
    ) -> Result<Option<DeviceResumeWait>, StoreError> {
        self.ensure_active()?;
        let row: Option<(String, i64, i64, i64)> = self
            .inner
            .query_row(
                "SELECT device_id,blocked_task_revision,registration_event_high_water_seq,created_at_ms
                 FROM device_resume_waits WHERE task_id=?1",
                [task_id.as_str()],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .optional()?;
        row.map(|(device, revision, high, created)| {
            Ok(DeviceResumeWait {
                task_id: task_id.clone(),
                device_id: DeviceId::new(device).map_err(|_| StoreError::CorruptRow)?,
                blocked_task_revision: u64::try_from(revision)
                    .map_err(|_| StoreError::CorruptRow)?,
                registration_event_high_water_seq: u64::try_from(high)
                    .map_err(|_| StoreError::CorruptRow)?,
                created_at: EpochMillis::new(created).map_err(|_| StoreError::CorruptRow)?,
            })
        })
        .transpose()
    }

    /// Removes waits whose TaskEngine lifecycle fence is stale. The revision
    /// check makes such rows inert immediately; this bounded sweep keeps
    /// cancelled or otherwise transitioned tasks from retaining dead waits.
    pub fn reconcile_device_resume_waits(&mut self, limit: u16) -> Result<u16, StoreError> {
        self.ensure_active()?;
        if !(1..=512).contains(&limit) {
            return Err(StoreError::InvalidSchedule);
        }
        self.operation_savepoint(|tx| {
            let stale = {
                let mut statement = tx.inner.prepare(
                    "SELECT w.task_id,w.blocked_task_revision
                     FROM device_resume_waits w LEFT JOIN tasks t USING(task_id)
                     WHERE t.task_id IS NULL OR t.state<>'BLOCKED'
                       OR t.blocked_reason<>'DEVICE_OFFLINE'
                       OR t.state_revision<>w.blocked_task_revision
                     ORDER BY w.task_id LIMIT ?1",
                )?;
                statement
                    .query_map([i64::from(limit)], |row| {
                        Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
                    })?
                    .collect::<Result<Vec<_>, _>>()?
            };
            for (task_id, revision) in &stale {
                tx.inner.execute(
                    "DELETE FROM device_resume_waits WHERE task_id=?1 AND blocked_task_revision=?2",
                    rusqlite::params![task_id, revision],
                )?;
            }
            u16::try_from(stale.len()).map_err(|_| StoreError::CorruptRow)
        })
    }

    /// Reads pending materialized device wakes in deterministic bounded order.
    pub fn pending_device_session_resume_wakes(
        &self,
        limit: u16,
    ) -> Result<Vec<DeviceSessionResumeWake>, StoreError> {
        self.ensure_active()?;
        if !(1..=MAX_SCHEDULER_EVENT_SCAN_PAGE).contains(&limit) {
            return Err(StoreError::InvalidSchedule);
        }
        let rows = {
            let mut statement = self.inner.prepare(
                "SELECT source_event_id,source_seq,task_id,device_id,blocked_task_revision,created_at_ms
                 FROM device_session_resume_wakes
                 ORDER BY created_at_ms,source_event_id,task_id LIMIT ?1",
            )?;
            statement
                .query_map([i64::from(limit)], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, i64>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, i64>(4)?,
                        row.get::<_, i64>(5)?,
                    ))
                })?
                .collect::<Result<Vec<_>, _>>()?
        };
        rows.into_iter()
            .map(|(source, seq, task, device, revision, created)| {
                Ok(DeviceSessionResumeWake {
                    source_event_id: EventId::new(source).map_err(|_| StoreError::CorruptRow)?,
                    source_seq: u64::try_from(seq).map_err(|_| StoreError::CorruptRow)?,
                    task_id: TaskId::new(task).map_err(|_| StoreError::CorruptRow)?,
                    device_id: DeviceId::new(device).map_err(|_| StoreError::CorruptRow)?,
                    blocked_task_revision: u64::try_from(revision)
                        .map_err(|_| StoreError::CorruptRow)?,
                    created_at: EpochMillis::new(created).map_err(|_| StoreError::CorruptRow)?,
                })
            })
            .collect()
    }

    /// Materializes at most one Event Scan page. `false` means more eligible
    /// waits may remain, so the caller must leave this event at the cursor head
    /// and replay it after committing this page.
    pub fn materialize_device_session_resume_wakes(
        &mut self,
        source_event_id: &EventId,
        source_seq: u64,
        device_id: &DeviceId,
        now: EpochMillis,
        limit: u16,
    ) -> Result<(u16, bool), StoreError> {
        self.ensure_active()?;
        if source_seq == 0
            || source_seq > i64::MAX as u64
            || !(1..=MAX_SCHEDULER_EVENT_SCAN_PAGE).contains(&limit)
        {
            return Err(StoreError::InvalidSchedule);
        }
        let rows = {
            let mut statement = self.inner.prepare(
                "SELECT w.task_id,w.blocked_task_revision,t.state,t.blocked_reason,t.state_revision
                 FROM device_resume_waits w JOIN tasks t USING(task_id)
                 WHERE w.device_id=?1 AND w.registration_event_high_water_seq<?2
                   AND NOT EXISTS (
                     SELECT 1 FROM device_session_resume_wakes d
                     WHERE d.task_id=w.task_id
                       AND d.blocked_task_revision=w.blocked_task_revision
                   )
                 ORDER BY w.task_id LIMIT ?3",
            )?;
            statement
                .query_map(
                    rusqlite::params![device_id.as_str(), source_seq as i64, i64::from(limit)],
                    |row| {
                        Ok((
                            row.get::<_, String>(0)?,
                            row.get::<_, i64>(1)?,
                            row.get::<_, String>(2)?,
                            row.get::<_, Option<String>>(3)?,
                            row.get::<_, i64>(4)?,
                        ))
                    },
                )?
                .collect::<Result<Vec<_>, _>>()?
        };
        for (task_raw, blocked_revision_raw, state, reason, state_revision_raw) in &rows {
            let task_id = TaskId::new(task_raw.clone()).map_err(|_| StoreError::CorruptRow)?;
            let blocked_revision =
                u64::try_from(*blocked_revision_raw).map_err(|_| StoreError::CorruptRow)?;
            let state_revision =
                u64::try_from(*state_revision_raw).map_err(|_| StoreError::CorruptRow)?;
            if state == "BLOCKED"
                && reason.as_deref() == Some("DEVICE_OFFLINE")
                && state_revision == blocked_revision
            {
                self.inner.execute(
                    "INSERT INTO device_session_resume_wakes(
                       source_event_id,source_seq,task_id,device_id,blocked_task_revision,created_at_ms
                     ) VALUES (?1,?2,?3,?4,?5,?6)
                     ON CONFLICT(source_event_id,task_id) DO NOTHING",
                    rusqlite::params![source_event_id.as_str(), source_seq as i64, task_id.as_str(), device_id.as_str(), *blocked_revision_raw, now.get()],
                )?;
            } else {
                self.inner.execute(
                    "DELETE FROM device_resume_waits WHERE task_id=?1 AND blocked_task_revision=?2",
                    rusqlite::params![task_id.as_str(), *blocked_revision_raw],
                )?;
            }
        }
        Ok((
            u16::try_from(rows.len()).map_err(|_| StoreError::CorruptRow)?,
            rows.len() < usize::from(limit),
        ))
    }

    /// Materializes one typed approval lifecycle handoff. Missing tasks are
    /// structurally stale and are classified without creating a dangling row.
    /// Repeated delivery of the same source event must carry identical facts.
    pub fn materialize_approval_lifecycle_wake(
        &mut self,
        source_event_id: &EventId,
        source_seq: u64,
        payload: &ApprovalLifecyclePayloadV1,
        outcome: ApprovalLifecycleOutcome,
        created_at: EpochMillis,
    ) -> Result<bool, StoreError> {
        self.ensure_active()?;
        if source_seq == 0 || source_seq > i64::MAX as u64 {
            return Err(StoreError::InvalidSchedule);
        }
        self.operation_savepoint(|tx| {
            let task_exists: bool = tx.inner.query_row(
                "SELECT EXISTS(SELECT 1 FROM tasks WHERE task_id=?1)",
                [payload.task_id().as_str()],
                |row| row.get(0),
            )?;
            if !task_exists {
                return Ok(false);
            }
            tx.inner.execute(
                "INSERT INTO approval_lifecycle_wakes(
                   source_event_id,source_seq,approval_id,task_id,step_id,outcome_kind,created_at_ms
                 ) VALUES (?1,?2,?3,?4,?5,?6,?7)
                 ON CONFLICT(source_event_id) DO NOTHING",
                rusqlite::params![
                    source_event_id.as_str(),
                    source_seq as i64,
                    payload.approval_id().as_str(),
                    payload.task_id().as_str(),
                    payload.step_id().as_str(),
                    outcome.as_str(),
                    created_at.get()
                ],
            )?;
            let actual: (i64, String, String, String, String) = tx.inner.query_row(
                "SELECT source_seq,approval_id,task_id,step_id,outcome_kind
                 FROM approval_lifecycle_wakes WHERE source_event_id=?1",
                [source_event_id.as_str()],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                    ))
                },
            )?;
            if actual.0 != source_seq as i64
                || actual.1 != payload.approval_id().as_str()
                || actual.2 != payload.task_id().as_str()
                || actual.3 != payload.step_id().as_str()
                || actual.4 != outcome.as_str()
            {
                return Err(StoreError::CorruptRow);
            }
            Ok(true)
        })
    }

    /// Lists unacknowledged approval lifecycle wakes in deterministic order.
    /// Reading never consumes or acknowledges a wake.
    pub fn pending_approval_lifecycle_wakes(
        &self,
        limit: u16,
    ) -> Result<Vec<ApprovalLifecycleWake>, StoreError> {
        self.ensure_active()?;
        if !(1..=MAX_SCHEDULER_EVENT_SCAN_PAGE).contains(&limit) {
            return Err(StoreError::InvalidSchedule);
        }
        let rows = {
            let mut statement = self.inner.prepare(
                "SELECT source_event_id,source_seq,approval_id,task_id,step_id,outcome_kind,created_at_ms
                 FROM approval_lifecycle_wakes
                 ORDER BY created_at_ms,source_seq,source_event_id LIMIT ?1",
            )?;
            statement
                .query_map([i64::from(limit)], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, i64>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, String>(4)?,
                        row.get::<_, String>(5)?,
                        row.get::<_, i64>(6)?,
                    ))
                })?
                .collect::<Result<Vec<_>, _>>()?
        };
        rows.into_iter().map(approval_wake_from_row).collect()
    }

    /// Reads one pending approval wake without consuming it.
    pub fn approval_lifecycle_wake(
        &self,
        source_event_id: &EventId,
    ) -> Result<Option<ApprovalLifecycleWake>, StoreError> {
        self.ensure_active()?;
        let row = self
            .inner
            .query_row(
                "SELECT source_event_id,source_seq,approval_id,task_id,step_id,outcome_kind,created_at_ms
                 FROM approval_lifecycle_wakes WHERE source_event_id=?1",
                [source_event_id.as_str()],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, i64>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, String>(4)?,
                        row.get::<_, String>(5)?,
                        row.get::<_, i64>(6)?,
                    ))
                },
            )
            .optional()?;
        row.map(approval_wake_from_row).transpose()
    }

    /// Explicitly acknowledges one handoff after a future consumer has durably
    /// applied its result. Repeating an acknowledgement is a no-op.
    pub fn acknowledge_approval_lifecycle_wake(
        &mut self,
        source_event_id: &EventId,
    ) -> Result<bool, StoreError> {
        self.ensure_active()?;
        self.operation_savepoint(|tx| {
            let changed = tx.inner.execute(
                "DELETE FROM approval_lifecycle_wakes WHERE source_event_id=?1",
                [source_event_id.as_str()],
            )?;
            Ok(changed == 1)
        })
    }

    /// Atomically resumes a valid device wait or consumes stale work without
    /// emitting a Task event or changing the Task.
    pub fn resume_device_session_wake(
        &mut self,
        wake: &DeviceSessionResumeWake,
        now: EpochMillis,
        context: &TransitionContext<'_>,
    ) -> Result<Option<crate::TaskSnapshot>, StoreError> {
        self.ensure_active()?;
        self.operation_savepoint(|tx| {
            if wake.blocked_task_revision == 0
                || wake.blocked_task_revision > i64::MAX as u64
                || wake.source_seq == 0
                || wake.source_seq > i64::MAX as u64
            {
                return Err(StoreError::InvalidSchedule);
            }
            let row: Option<DeviceWakeValidationRow> = tx.inner
                .query_row(
                    "SELECT w.source_seq,w.device_id,w.blocked_task_revision,w.created_at_ms,
                        t.state_revision,t.blocked_reason,t.state,t.updated_at_ms,
                        v.device_id,v.blocked_task_revision
                     FROM device_session_resume_wakes w JOIN tasks t USING(task_id)
                     LEFT JOIN device_resume_waits v USING(task_id)
                     WHERE w.source_event_id=?1 AND w.task_id=?2",
                    rusqlite::params![wake.source_event_id.as_str(), wake.task_id.as_str()],
                    |row| Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?,row.get(4)?,row.get(5)?,row.get(6)?,row.get(7)?,row.get(8)?,row.get(9)?)),
                ).optional()?;
            let valid = row.as_ref().is_some_and(|(source_seq, device, blocked_revision, created, current_revision, reason, state, updated, _wait_device, _wait_revision)| {
                *source_seq == wake.source_seq as i64
                    && device == wake.device_id.as_str()
                    && *blocked_revision == wake.blocked_task_revision as i64
                    && *created == wake.created_at.get()
                    && *current_revision == wake.blocked_task_revision as i64
                    && reason.as_deref() == Some("DEVICE_OFFLINE")
                    && state == "BLOCKED"
                    && now.get() >= *updated
            });
            if !valid {
                tx.inner.execute(
                    "DELETE FROM device_session_resume_wakes WHERE source_event_id=?1 AND task_id=?2",
                    rusqlite::params![wake.source_event_id.as_str(), wake.task_id.as_str()],
                )?;
                if row.is_some() {
                    tx.inner.execute(
                        "DELETE FROM device_resume_waits WHERE task_id=?1 AND blocked_task_revision=?2",
                        rusqlite::params![wake.task_id.as_str(), i64::try_from(wake.blocked_task_revision).map_err(|_| StoreError::CorruptRow)?],
                    )?;
                }
                return Ok(None);
            }
            tx.require_audit()?;
            let changed = tx.inner.execute(
                "UPDATE tasks SET state='READY',blocked_reason=NULL,updated_at_ms=?1
                 WHERE task_id=?2 AND state='BLOCKED' AND blocked_reason='DEVICE_OFFLINE'
                   AND state_revision=?3 AND updated_at_ms<=?1",
                rusqlite::params![now.get(),wake.task_id.as_str(),i64::try_from(wake.blocked_task_revision).map_err(|_| StoreError::CorruptRow)?],
            )?;
            if changed != 1 {
                return Err(StoreError::RecoverySnapshotStale);
            }
            tx.inner.execute(
                "DELETE FROM device_resume_waits WHERE task_id=?1 AND blocked_task_revision=?2",
                rusqlite::params![wake.task_id.as_str(),i64::try_from(wake.blocked_task_revision).map_err(|_| StoreError::CorruptRow)?],
            )?;
            tx.inner.execute(
                "DELETE FROM device_session_resume_wakes WHERE source_event_id=?1 AND task_id=?2",
                rusqlite::params![wake.source_event_id.as_str(),wake.task_id.as_str()],
            )?;
            let mut facts = DurableTransition::task(
                AuditOperation::DeviceSessionResumed,
                &wake.task_id,
                Some(serea_protocol::TaskState::Blocked),
                serea_protocol::TaskState::Ready,
                tx.load_task(&wake.task_id)?.task.data_class,
                now,
                context,
            );
            facts.reason = Some(serea_protocol::ReasonCode::new("DEVICE_OFFLINE").map_err(|_| StoreError::CorruptRow)?);
            tx.record_transition(&facts)?;
            Ok(Some(tx.load_task(&wake.task_id)?))
        })
    }

    /// Resolves the immutable template and current schedule ceiling for a live
    /// claimed occurrence. The final mapping operation rechecks the fence.
    pub fn schedule_occurrence_work(
        &self,
        lease: &ScheduleOccurrenceLease,
        now: EpochMillis,
    ) -> Result<ScheduleOccurrenceWork, StoreError> {
        self.ensure_active()?;
        let row: Option<ScheduleOccurrenceWorkRow> = self
            .inner
            .query_row(
                "SELECT o.state,o.source_event_id,o.source_event_data_class_rank,
                    o.lease_generation,o.lease_expires_at_ms,o.template_digest,
                    o.template_data_class_rank,s.policy_class_rank,o.lease_owner
             FROM schedule_occurrences o JOIN schedules s USING(schedule_id)
             WHERE o.schedule_id=?1 AND o.occurrence_key=?2",
                rusqlite::params![lease.schedule_id.as_str(), lease.occurrence_key],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                        row.get(5)?,
                        row.get(6)?,
                        row.get(7)?,
                        row.get(8)?,
                    ))
                },
            )
            .optional()?;
        let Some((
            state,
            source,
            source_class,
            generation,
            expiry,
            digest,
            class,
            policy_rank,
            owner,
        )) = row
        else {
            return Err(StoreError::ScheduleOccurrenceNotClaimable);
        };
        if state != "CLAIMED"
            || generation != i64::from(lease.lease_generation)
            || expiry.is_none_or(|value| value <= now.get())
            || lease.lease_expires_at.get() <= now.get()
            || owner.as_deref() != Some(lease.lease_owner.as_str())
        {
            return Err(StoreError::StaleSchedulerLease);
        }
        let (Some(digest), Some(class)) = (digest, class) else {
            return Err(StoreError::CorruptRow);
        };
        Ok(ScheduleOccurrenceWork {
            schedule_id: lease.schedule_id.clone(),
            occurrence_key: lease.occurrence_key.clone(),
            source_event_id: source
                .map(EventId::new)
                .transpose()
                .map_err(|_| StoreError::CorruptRow)?,
            source_event_data_class: source_class
                .map(|rank| {
                    data_class_from_rank(u8::try_from(rank).map_err(|_| StoreError::CorruptRow)?)
                })
                .transpose()?,
            template: crate::BlobRef::new(
                Digest::new(digest).map_err(|_| StoreError::CorruptRow)?,
                data_class_from_rank(u8::try_from(class).map_err(|_| StoreError::CorruptRow)?)?,
            ),
            policy_class_rank: u8::try_from(policy_rank).map_err(|_| StoreError::CorruptRow)?,
        })
    }

    /// Loads a checked schedule projection for Scheduler semantic decisions.
    pub fn load_schedule(&self, schedule_id: &ScheduleId) -> Result<ScheduleSnapshot, StoreError> {
        self.ensure_active()?;
        let row: ScheduleProjectionRow = self.inner.query_row(
            "SELECT state,revision,trigger_kind,owner_kind,recurrence_json,event_predicate_json,
                    template_digest,template_data_class_rank,policy_class_rank,approval_policy_json,
                    timezone,recurrence_evaluator,tzdb_version,next_due_at_ms,next_local_label,missed_policy,
                    event_predicate_after_seq
             FROM schedules WHERE schedule_id=?1",
            [schedule_id.as_str()],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?, row.get(5)?,
                row.get(6)?, row.get(7)?, row.get(8)?, row.get(9)?, row.get(10)?, row.get(11)?,
                row.get(12)?, row.get(13)?, row.get(14)?, row.get(15)?, row.get(16)?)),
        ).optional()?.ok_or(StoreError::ScheduleNotActive)?;
        let state = match row.0.as_str() {
            "ACTIVE" => ScheduleCommandState::Active,
            "PAUSED" => ScheduleCommandState::Paused,
            "CANCELLED" => ScheduleCommandState::Cancelled,
            _ => return Err(StoreError::CorruptRow),
        };
        let trigger_kind = trigger_from_db(&row.2)?;
        let revision = u32::try_from(row.1).map_err(|_| StoreError::CorruptRow)?;
        let class_rank = row
            .7
            .map(|rank| u8::try_from(rank).map_err(|_| StoreError::CorruptRow))
            .transpose()?;
        let template = match (row.6, class_rank) {
            (Some(digest), Some(rank)) => Some(crate::BlobRef::new(
                Digest::new(digest).map_err(|_| StoreError::CorruptRow)?,
                data_class_from_rank(rank)?,
            )),
            (None, None) => None,
            _ => return Err(StoreError::CorruptRow),
        };
        let policy_class_rank = u8::try_from(row.8).map_err(|_| StoreError::CorruptRow)?;
        if policy_class_rank > 7 || !matches!(row.3.as_str(), "HOST" | "DEVICE") {
            return Err(StoreError::CorruptRow);
        }
        let approval_json =
            std::str::from_utf8(&canonicalize(&row.9).map_err(|_| StoreError::CorruptRow)?)
                .map_err(|_| StoreError::CorruptRow)?
                .to_owned();
        let next_due_at = row
            .13
            .map(EpochMillis::new)
            .transpose()
            .map_err(|_| StoreError::CorruptRow)?;
        let missed_policy = match row.15.as_str() {
            "SKIP" => MissedOccurrencePolicy::Skip,
            "RUN_ONCE" => MissedOccurrencePolicy::RunOnce,
            "RUN_EACH" => MissedOccurrencePolicy::RunEach,
            _ => return Err(StoreError::CorruptRow),
        };
        let event_predicate_after_seq = row
            .16
            .map(|seq| u64::try_from(seq).map_err(|_| StoreError::CorruptRow))
            .transpose()?;
        if (trigger_kind == ScheduleTriggerKind::Calendar) != row.4.is_some()
            || (trigger_kind == ScheduleTriggerKind::HostEvent) != row.5.is_some()
            || (trigger_kind == ScheduleTriggerKind::HostEvent)
                != event_predicate_after_seq.is_some()
            || (trigger_kind == ScheduleTriggerKind::Calendar) != row.10.is_some()
            || (next_due_at.is_some() != row.14.is_some())
            || (trigger_kind != ScheduleTriggerKind::Calendar
                && (next_due_at.is_some() || row.14.is_some()))
        {
            return Err(StoreError::CorruptRow);
        }
        Ok(ScheduleSnapshot {
            schedule_id: schedule_id.clone(),
            revision,
            state,
            trigger_kind,
            recurrence_json: row.4,
            event_predicate_json: row.5,
            event_predicate_after_seq,
            template,
            policy_class_rank,
            approval_policy_json: approval_json,
            timezone: row.10,
            recurrence_evaluator: row.11,
            tzdb_version: row.12,
            next_due_at,
            next_local_label: row.14,
            missed_policy,
        })
    }

    /// Returns active calendar schedules due at the injected instant, in
    /// deterministic due/identity order and within the storage query bound.
    pub fn due_calendar_schedules(
        &self,
        now: EpochMillis,
        limit: u16,
    ) -> Result<Vec<ScheduleSnapshot>, StoreError> {
        self.ensure_active()?;
        if !(1..=MAX_ACTIVE_SCHEDULES as u16).contains(&limit) {
            return Err(StoreError::InvalidSchedule);
        }
        let ids = {
            let mut statement = self.inner.prepare(
                "SELECT s.schedule_id FROM schedules s
                 WHERE s.state='ACTIVE' AND s.trigger_kind='CALENDAR'
                   AND (s.next_due_at_ms<=?1 OR EXISTS(
                     SELECT 1 FROM schedule_occurrences o
                     WHERE o.schedule_id=s.schedule_id AND o.state='PENDING'
                       AND o.due_at_ms<=?1
                       AND (o.not_before_ms IS NULL OR o.not_before_ms<=?1)
                   ))
                 ORDER BY min(s.next_due_at_ms,COALESCE((
                   SELECT min(o.due_at_ms) FROM schedule_occurrences o
                   WHERE o.schedule_id=s.schedule_id AND o.state='PENDING'
                     AND o.due_at_ms<=?1
                     AND (o.not_before_ms IS NULL OR o.not_before_ms<=?1)
                 ),s.next_due_at_ms)),s.schedule_id LIMIT ?2",
            )?;
            statement
                .query_map(rusqlite::params![now.get(), i64::from(limit)], |row| {
                    row.get::<_, String>(0)
                })?
                .collect::<Result<Vec<_>, _>>()?
        };
        ids.iter()
            .map(|raw| {
                let id = ScheduleId::new(raw.clone()).map_err(|_| StoreError::CorruptRow)?;
                self.load_schedule(&id)
            })
            .collect()
    }

    /// Loads the bounded active HOST_EVENT schedule set in stable id order.
    pub fn active_host_event_schedules(&self) -> Result<Vec<ScheduleSnapshot>, StoreError> {
        self.ensure_active()?;
        let ids = {
            let mut statement = self.inner.prepare(
                "SELECT schedule_id FROM schedules
                 WHERE state='ACTIVE' AND trigger_kind='HOST_EVENT'
                 ORDER BY schedule_id LIMIT ?1",
            )?;
            statement
                .query_map([MAX_ACTIVE_SCHEDULES], |row| row.get::<_, String>(0))?
                .collect::<Result<Vec<_>, _>>()?
        };
        ids.iter()
            .map(|raw| {
                let id = ScheduleId::new(raw.clone()).map_err(|_| StoreError::CorruptRow)?;
                self.load_schedule(&id)
            })
            .collect()
    }

    /// Lists a globally bounded page of durable HOST_EVENT work. Event
    /// occurrences have no calendar due instant; their committed source event
    /// makes them immediately eligible while `not_before` still fences retry.
    /// The active schedule cap (256) times the per-schedule pending cap (256)
    /// bounds the ordered candidate set at 65,536 rows before this page limit.
    pub fn pending_host_event_occurrences(
        &self,
        now: EpochMillis,
        limit: u16,
    ) -> Result<Vec<PendingHostEventOccurrence>, StoreError> {
        self.ensure_active()?;
        if !(1..=MAX_SCHEDULER_EVENT_SCAN_PAGE).contains(&limit) {
            return Err(StoreError::InvalidSchedule);
        }
        let rows = {
            let mut statement = self.inner.prepare(
                "SELECT o.schedule_id,o.occurrence_key,o.schedule_revision
                 FROM schedule_occurrences o JOIN schedules s USING(schedule_id)
                 WHERE s.state='ACTIVE' AND s.trigger_kind='HOST_EVENT'
                   AND o.trigger_kind='HOST_EVENT' AND o.state='PENDING'
                   AND o.due_at_ms IS NULL
                   AND (o.not_before_ms IS NULL OR o.not_before_ms<=?1)
                 ORDER BY o.created_at_ms,o.schedule_id,o.occurrence_key LIMIT ?2",
            )?;
            statement
                .query_map(rusqlite::params![now.get(), i64::from(limit)], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, i64>(2)?,
                    ))
                })?
                .collect::<Result<Vec<_>, _>>()?
        };
        rows.into_iter()
            .map(|(schedule, key, revision)| {
                Ok(PendingHostEventOccurrence {
                    schedule_id: ScheduleId::new(schedule).map_err(|_| StoreError::CorruptRow)?,
                    occurrence_key: key,
                    schedule_revision: u32::try_from(revision)
                        .map_err(|_| StoreError::CorruptRow)?,
                })
            })
            .collect()
    }

    /// Claims the one durable internal Event Bus consumer lease.
    pub fn claim_scheduler_consumer(
        &mut self,
        owner: &str,
        now: EpochMillis,
        expires_at: EpochMillis,
    ) -> Result<SchedulerConsumerLease, StoreError> {
        self.ensure_active()?;
        if owner.is_empty()
            || owner.len() > MAX_OWNER_BYTES
            || expires_at.get() <= now.get()
            || expires_at.get() - now.get() > MAX_LEASE_MS
        {
            return Err(StoreError::InvalidLeaseInterval);
        }
        self.operation_savepoint(|tx| {
            let current: (Option<String>, i64, Option<i64>) = tx.inner.query_row(
                "SELECT lease_owner,lease_generation,lease_expires_at_ms
                 FROM scheduler_consumer_state WHERE singleton=1",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )?;
            if current.0.is_some() && current.2.is_some_and(|expiry| expiry > now.get()) {
                return Err(StoreError::LeaseHeld);
            }
            let generation = u32::try_from(current.1)
                .map_err(|_| StoreError::CorruptRow)?
                .checked_add(1)
                .ok_or(StoreError::SchedulerLeaseGenerationOverflow)?;
            let changed = tx.inner.execute(
                "UPDATE scheduler_consumer_state SET lease_owner=?1,lease_generation=?2,
                   lease_expires_at_ms=?3 WHERE singleton=1 AND lease_generation=?4",
                rusqlite::params![owner, i64::from(generation), expires_at.get(), current.1],
            )?;
            if changed != 1 {
                return Err(StoreError::SchedulerLeaseFenced);
            }
            Ok(SchedulerConsumerLease {
                owner: owner.to_owned(),
                generation,
                expires_at,
            })
        })
    }

    /// Reads the singleton consumer cursor while holding the caller's
    /// transaction snapshot.
    pub fn scheduler_cursor(&mut self) -> Result<SchedulerCursorSnapshot, StoreError> {
        self.ensure_active()?;
        let (last, high): (i64, Option<i64>) = self.inner.query_row(
            "SELECT last_processed_seq,replay_high_water_seq
             FROM scheduler_consumer_state WHERE singleton=1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?;
        Ok(SchedulerCursorSnapshot {
            last_processed_seq: u64::try_from(last).map_err(|_| StoreError::CorruptRow)?,
            replay_high_water_seq: high
                .map(u64::try_from)
                .transpose()
                .map_err(|_| StoreError::CorruptRow)?,
        })
    }

    /// Advances the durable cursor in the same transaction as the decision
    /// produced for the source event/range. The snapshot is pinned until fully
    /// consumed; cursor and high-water remain monotonic and bounded.
    pub fn advance_scheduler_cursor(
        &mut self,
        lease: &SchedulerConsumerLease,
        expected_cursor: u64,
        next_cursor: u64,
        snapshot_high_water: u64,
        now: EpochMillis,
    ) -> Result<SchedulerCursorSnapshot, StoreError> {
        self.ensure_active()?;
        if next_cursor < expected_cursor || next_cursor > snapshot_high_water {
            return Err(StoreError::ScheduleRevisionConflict);
        }
        self.operation_savepoint(|tx| {
            let current: (i64, Option<i64>, Option<String>, i64, Option<i64>) =
                tx.inner.query_row(
                    "SELECT last_processed_seq,replay_high_water_seq,lease_owner,
                        lease_generation,lease_expires_at_ms
                 FROM scheduler_consumer_state WHERE singleton=1",
                    [],
                    |row| {
                        Ok((
                            row.get(0)?,
                            row.get(1)?,
                            row.get(2)?,
                            row.get(3)?,
                            row.get(4)?,
                        ))
                    },
                )?;
            if current.0 != i64::try_from(expected_cursor).map_err(|_| StoreError::CorruptRow)?
                || current
                    .1
                    .is_some_and(|high| i64::try_from(snapshot_high_water).ok() != Some(high))
            {
                return Err(StoreError::ScheduleRevisionConflict);
            }
            if current.2.as_deref() != Some(lease.owner.as_str())
                || current.3 != i64::from(lease.generation)
                || current.4.is_none_or(|expiry| expiry <= now.get())
                || lease.expires_at.get() <= now.get()
            {
                return Err(StoreError::SchedulerLeaseFenced);
            }
            let changed = tx.inner.execute(
                "UPDATE scheduler_consumer_state SET last_processed_seq=?1,
                   replay_high_water_seq=CASE WHEN ?1>=?2 THEN NULL ELSE ?2 END
                 WHERE singleton=1 AND last_processed_seq=?3
                   AND lease_owner=?4 AND lease_generation=?5
                   AND lease_expires_at_ms>?6
                   AND (replay_high_water_seq IS NULL OR replay_high_water_seq=?2)
                   AND ?2 <= (SELECT last_allocated_seq FROM event_store_state WHERE singleton=1)",
                rusqlite::params![
                    i64::try_from(next_cursor).map_err(|_| StoreError::CorruptRow)?,
                    i64::try_from(snapshot_high_water).map_err(|_| StoreError::CorruptRow)?,
                    i64::try_from(expected_cursor).map_err(|_| StoreError::CorruptRow)?,
                    lease.owner,
                    i64::from(lease.generation),
                    now.get()
                ],
            )?;
            if changed != 1 || lease.expires_at.get() <= now.get() {
                return Err(StoreError::SchedulerLeaseFenced);
            }
            tx.scheduler_cursor()
        })
    }

    /// Creates a schedule, emits its specific lifecycle event, and persists the
    /// authenticated retry result atomically.
    pub fn create_schedule_command(
        &mut self,
        message_id: &EventId,
        request_digest: &Digest,
        draft: ScheduleDraft,
        now: EpochMillis,
        event: EventDraft,
    ) -> Result<ScheduleCommandOutcome, StoreError> {
        self.ensure_active()?;
        let schedule_id = draft.schedule_id.clone();
        if event.event.kind != EventKind::ScheduleCreated
            || event.event.causation_id.as_ref() != Some(message_id)
            || event
                .event
                .payload
                .get("schedule_id")
                .and_then(serde_json::Value::as_str)
                != Some(schedule_id.as_str())
        {
            return Err(StoreError::AuditRejected);
        }
        self.operation_savepoint(|tx| {
            let existing: Option<(String, String, String, i64, String)> = tx
                .inner
                .query_row(
                    "SELECT request_digest,command_kind,schedule_id,result_revision,result_state
                     FROM schedule_command_receipts WHERE message_id=?1",
                    [message_id.as_str()],
                    |row| {
                        Ok((
                            row.get(0)?,
                            row.get(1)?,
                            row.get(2)?,
                            row.get(3)?,
                            row.get(4)?,
                        ))
                    },
                )
                .optional()?;
            if let Some((saved_digest, command_kind, saved_id, revision, state)) = existing {
                if saved_digest != request_digest.to_string()
                    || command_kind != "CREATE"
                    || saved_id != schedule_id.as_str()
                    || state != "ACTIVE"
                {
                    return Err(StoreError::ScheduleCommandIdentityConflict);
                }
                return Ok(ScheduleCommandOutcome {
                    schedule_id,
                    revision: u32::try_from(revision).map_err(|_| StoreError::CorruptRow)?,
                    state: ScheduleCommandState::Active,
                    replayed: true,
                });
            }
            tx.create_schedule(draft, now)?;
            tx.append_event(event.event, event.retention_at)?;
            tx.inner.execute(
                "INSERT INTO schedule_command_receipts(
                   message_id,request_digest,command_kind,schedule_id,result_revision,
                   result_state,committed_at_ms
                 ) VALUES (?1,?2,'CREATE',?3,1,'ACTIVE',?4)",
                rusqlite::params![
                    message_id.as_str(),
                    request_digest.to_string(),
                    schedule_id.as_str(),
                    now.get()
                ],
            )?;
            Ok(ScheduleCommandOutcome {
                schedule_id,
                revision: 1,
                state: ScheduleCommandState::Active,
                replayed: false,
            })
        })
    }

    /// Replaces only the schedule's future definition. Existing occurrence rows
    /// keep their captured resolved instant and template reference.
    pub fn update_schedule_command(
        &mut self,
        message_id: &EventId,
        request_digest: &Digest,
        expected_revision: u32,
        draft: ScheduleDraft,
        now: EpochMillis,
        event: EventDraft,
    ) -> Result<ScheduleCommandOutcome, StoreError> {
        self.ensure_active()?;
        if event.event.kind != EventKind::ScheduleUpdated
            || event.event.causation_id.as_ref() != Some(message_id)
            || event
                .event
                .payload
                .get("schedule_id")
                .and_then(serde_json::Value::as_str)
                != Some(draft.schedule_id.as_str())
        {
            return Err(StoreError::AuditRejected);
        }
        let recurrence = canonical_object(draft.recurrence.as_ref())?;
        let predicate = canonical_object(draft.event_predicate.as_ref())?;
        let approval =
            canonical_object(Some(&draft.approval_policy))?.ok_or(StoreError::InvalidSchedule)?;
        let template_source =
            std::str::from_utf8(&draft.template_json).map_err(|_| StoreError::InvalidSchedule)?;
        let template = ScheduledTaskTemplateV1::parse_json(template_source)
            .map_err(|_| StoreError::InvalidSchedule)?;
        if matches!(
            draft.template_data_class,
            DataClass::Secret | DataClass::Credential
        ) || draft.owner_id.is_empty()
            || draft.owner_id.len() > MAX_OWNER_BYTES
            || draft.policy_class_rank > 7
            || (draft.trigger_kind == ScheduleTriggerKind::Calendar) != recurrence.is_some()
            || (draft.trigger_kind == ScheduleTriggerKind::HostEvent) != predicate.is_some()
            || (draft.trigger_kind == ScheduleTriggerKind::Calendar) != draft.timezone.is_some()
            || (draft.trigger_kind == ScheduleTriggerKind::Calendar)
                != draft.recurrence_evaluator.is_some()
            || (draft.trigger_kind == ScheduleTriggerKind::Calendar) != draft.tzdb_version.is_some()
            || (draft.trigger_kind == ScheduleTriggerKind::Calendar) != draft.next_due_at.is_some()
            || (draft.trigger_kind == ScheduleTriggerKind::Calendar)
                != draft.next_local_label.is_some()
        {
            return Err(StoreError::InvalidSchedule);
        }
        self.operation_savepoint(|tx| {
            let existing: Option<(String, String, String, i64, String)> = tx
                .inner
                .query_row(
                    "SELECT request_digest,command_kind,schedule_id,result_revision,result_state
                 FROM schedule_command_receipts WHERE message_id=?1",
                    [message_id.as_str()],
                    |row| {
                        Ok((
                            row.get(0)?,
                            row.get(1)?,
                            row.get(2)?,
                            row.get(3)?,
                            row.get(4)?,
                        ))
                    },
                )
                .optional()?;
            if let Some((saved_digest, kind, id, revision, state)) = existing {
                if saved_digest != request_digest.to_string()
                    || kind != "UPDATE"
                    || id != draft.schedule_id.as_str()
                {
                    return Err(StoreError::ScheduleCommandIdentityConflict);
                }
                return Ok(ScheduleCommandOutcome {
                    schedule_id: draft.schedule_id.clone(),
                    revision: u32::try_from(revision).map_err(|_| StoreError::CorruptRow)?,
                    state: match state.as_str() {
                        "ACTIVE" => ScheduleCommandState::Active,
                        "PAUSED" => ScheduleCommandState::Paused,
                        "CANCELLED" => ScheduleCommandState::Cancelled,
                        _ => return Err(StoreError::CorruptRow),
                    },
                    replayed: true,
                });
            }
            let current: Option<(String, i64, String, String, i64, i64)> = tx
                .inner
                .query_row(
                    "SELECT state,revision,owner_kind,owner_id,created_at_ms,updated_at_ms
                 FROM schedules WHERE schedule_id=?1",
                    [draft.schedule_id.as_str()],
                    |row| {
                        Ok((
                            row.get(0)?,
                            row.get(1)?,
                            row.get(2)?,
                            row.get(3)?,
                            row.get(4)?,
                            row.get(5)?,
                        ))
                    },
                )
                .optional()?;
            let Some((state, revision, owner_kind, owner_id, created, updated)) = current else {
                return Err(StoreError::ScheduleNotActive);
            };
            if !matches!(state.as_str(), "ACTIVE" | "PAUSED")
                || revision != i64::from(expected_revision)
                || now.get() < created
                || now.get() < updated
                || owner_id != draft.owner_id
                || owner_kind
                    != match draft.owner_kind {
                        ScheduleOwnerKind::Host => "HOST",
                        ScheduleOwnerKind::Device => "DEVICE",
                    }
            {
                return Err(StoreError::ScheduleRevisionConflict);
            }
            let next_revision = expected_revision
                .checked_add(1)
                .ok_or(StoreError::PlanRevisionOverflow)?;
            let event_predicate_after_seq = if draft.trigger_kind == ScheduleTriggerKind::HostEvent
            {
                Some(tx.inner.query_row(
                    "SELECT last_allocated_seq FROM event_store_state WHERE singleton=1",
                    [],
                    |row| row.get::<_, i64>(0),
                )?)
            } else {
                None
            };
            let blob = tx.put_blob(
                template.canonical_json().as_bytes(),
                draft.template_data_class,
            )?;
            let changed = tx.inner.execute(
                "UPDATE schedules SET revision=?2,trigger_kind=?3,recurrence_json=?4,
                   event_predicate_json=?5,template_digest=?6,template_data_class_rank=?7,
                   policy_class_rank=?8,approval_policy_json=?9,timezone=?10,
                   recurrence_evaluator=?11,tzdb_version=?12,next_due_at_ms=?13,
                   next_local_label=?14,missed_policy=?15,updated_at_ms=?16,
                   event_predicate_after_seq=?19
                 WHERE schedule_id=?1 AND revision=?17 AND state=?18",
                rusqlite::params![
                    draft.schedule_id.as_str(),
                    i64::from(next_revision),
                    draft.trigger_kind.as_str(),
                    recurrence,
                    predicate,
                    blob.digest().as_str(),
                    i64::from(blob.class().rank()),
                    i64::from(draft.policy_class_rank),
                    approval,
                    draft.timezone,
                    draft.recurrence_evaluator,
                    draft.tzdb_version,
                    draft.next_due_at.map(EpochMillis::get),
                    draft.next_local_label,
                    draft.missed_policy.as_str(),
                    now.get(),
                    i64::from(expected_revision),
                    state,
                    event_predicate_after_seq
                ],
            )?;
            if changed != 1 {
                return Err(StoreError::ScheduleRevisionConflict);
            }
            tx.append_event(event.event, event.retention_at)?;
            tx.inner.execute(
                "INSERT INTO schedule_command_receipts(message_id,request_digest,command_kind,
                   schedule_id,result_revision,result_state,committed_at_ms)
                 VALUES (?1,?2,'UPDATE',?3,?4,?5,?6)",
                rusqlite::params![
                    message_id.as_str(),
                    request_digest.to_string(),
                    draft.schedule_id.as_str(),
                    i64::from(next_revision),
                    state,
                    now.get()
                ],
            )?;
            Ok(ScheduleCommandOutcome {
                schedule_id: draft.schedule_id,
                revision: next_revision,
                state: if state == "ACTIVE" {
                    ScheduleCommandState::Active
                } else {
                    ScheduleCommandState::Paused
                },
                replayed: false,
            })
        })
    }

    /// Applies a schedule state command, appends its specific lifecycle event,
    /// and stores the authenticated command result in the same transaction.
    pub fn cancel_schedule_command(
        &mut self,
        message_id: &EventId,
        request_digest: &Digest,
        schedule_id: &ScheduleId,
        expected_revision: u32,
        now: EpochMillis,
        event: EventDraft,
    ) -> Result<ScheduleCommandOutcome, StoreError> {
        self.change_schedule_state_command(ScheduleStateCommandRequest {
            message_id: message_id.clone(),
            request_digest: request_digest.clone(),
            schedule_id: schedule_id.clone(),
            expected_revision,
            command: ScheduleStateCommand::Cancel,
            now,
            event,
        })
    }

    pub fn change_schedule_state_command(
        &mut self,
        request: ScheduleStateCommandRequest,
    ) -> Result<ScheduleCommandOutcome, StoreError> {
        self.ensure_active()?;
        let ScheduleStateCommandRequest {
            message_id,
            request_digest,
            schedule_id,
            expected_revision,
            command,
            now,
            event,
        } = request;
        let result_state = command.result_state();
        if event.event.kind != command.event_kind()
            || event.event.causation_id.as_ref() != Some(&message_id)
            || event
                .event
                .payload
                .get("schedule_id")
                .and_then(serde_json::Value::as_str)
                != Some(schedule_id.as_str())
        {
            return Err(StoreError::AuditRejected);
        }
        self.operation_savepoint(|tx| {
            let existing: Option<(String, String, String, i64, String)> = tx
                .inner
                .query_row(
                    "SELECT request_digest,command_kind,schedule_id,result_revision,result_state
                     FROM schedule_command_receipts WHERE message_id=?1",
                    [message_id.as_str()],
                    |row| {
                        Ok((
                            row.get(0)?,
                            row.get(1)?,
                            row.get(2)?,
                            row.get(3)?,
                            row.get(4)?,
                        ))
                    },
                )
                .optional()?;
            if let Some((saved_digest, command_kind, saved_id, revision, state)) = existing {
                if saved_digest != request_digest.to_string()
                    || command_kind != command.command_kind()
                    || saved_id != schedule_id.as_str()
                    || state != command_state_as_str(result_state)
                {
                    return Err(StoreError::ScheduleCommandIdentityConflict);
                }
                return Ok(ScheduleCommandOutcome {
                    schedule_id: schedule_id.clone(),
                    revision: u32::try_from(revision).map_err(|_| StoreError::CorruptRow)?,
                    state: result_state,
                    replayed: true,
                });
            }

            let current: Option<(String, i64, i64, i64, Option<i64>)> = tx
                .inner
                .query_row(
                    "SELECT state,revision,created_at_ms,updated_at_ms,event_predicate_after_seq
                 FROM schedules WHERE schedule_id=?1",
                    [schedule_id.as_str()],
                    |row| {
                        Ok((
                            row.get(0)?,
                            row.get(1)?,
                            row.get(2)?,
                            row.get(3)?,
                            row.get(4)?,
                        ))
                    },
                )
                .optional()?;
            let Some((state, current_revision, created_at, updated_at, current_event_after)) =
                current
            else {
                return Err(StoreError::ScheduleNotActive);
            };
            if current_revision != i64::from(expected_revision) {
                return Err(StoreError::ScheduleRevisionConflict);
            }
            let allowed = match command {
                ScheduleStateCommand::Pause => state == "ACTIVE",
                ScheduleStateCommand::Resume => state == "PAUSED",
                ScheduleStateCommand::Cancel => matches!(state.as_str(), "ACTIVE" | "PAUSED"),
            };
            if !allowed {
                return Err(StoreError::ScheduleNotActive);
            }
            if now.get() < created_at || now.get() < updated_at {
                return Err(StoreError::InvalidTimestamp);
            }
            if command == ScheduleStateCommand::Resume {
                let active: i64 = tx.inner.query_row(
                    "SELECT count(*) FROM schedules WHERE state='ACTIVE'",
                    [],
                    |row| row.get(0),
                )?;
                if active >= MAX_ACTIVE_SCHEDULES {
                    return Err(StoreError::ScheduleActiveLimit);
                }
            }
            let revision = expected_revision
                .checked_add(1)
                .ok_or(StoreError::PlanRevisionOverflow)?;
            let event_predicate_after_seq =
                if command == ScheduleStateCommand::Resume && current_event_after.is_some() {
                    let high_water: i64 = tx.inner.query_row(
                        "SELECT last_allocated_seq FROM event_store_state WHERE singleton=1",
                        [],
                        |row| row.get(0),
                    )?;
                    if high_water < 0 {
                        return Err(StoreError::CorruptRow);
                    }
                    Some(high_water)
                } else {
                    current_event_after
                };
            let next_state = command_state_as_str(result_state);
            let changed = tx.inner.execute(
                "UPDATE schedules SET state=?2,revision=?3,updated_at_ms=?4,
                   cancelled_at_ms=CASE WHEN ?2='CANCELLED' THEN ?4 ELSE NULL END,
                   event_predicate_after_seq=?7
                 WHERE schedule_id=?1 AND revision=?5 AND state=?6",
                rusqlite::params![
                    schedule_id.as_str(),
                    next_state,
                    i64::from(revision),
                    now.get(),
                    current_revision,
                    state,
                    event_predicate_after_seq
                ],
            )?;
            if changed != 1 {
                return Err(StoreError::ScheduleRevisionConflict);
            }
            tx.append_event(event.event, event.retention_at)?;
            tx.inner.execute(
                "INSERT INTO schedule_command_receipts(
                   message_id,request_digest,command_kind,schedule_id,result_revision,
                   result_state,committed_at_ms
                 ) VALUES (?1,?2,?3,?4,?5,?6,?7)",
                rusqlite::params![
                    message_id.as_str(),
                    request_digest.to_string(),
                    command.command_kind(),
                    schedule_id.as_str(),
                    i64::from(revision),
                    next_state,
                    now.get()
                ],
            )?;
            Ok(ScheduleCommandOutcome {
                schedule_id: schedule_id.clone(),
                revision,
                state: result_state,
                replayed: false,
            })
        })
    }

    /// Inserts one schedule definition after enforcing the serialized global
    /// active-record bound. It does not mint identifiers or author events.
    pub(crate) fn create_schedule(
        &mut self,
        draft: ScheduleDraft,
        now: EpochMillis,
    ) -> Result<(), StoreError> {
        self.ensure_active()?;
        if draft.owner_id.is_empty() || draft.owner_id.len() > MAX_OWNER_BYTES {
            return Err(StoreError::InvalidSchedule);
        }
        let recurrence = canonical_object(draft.recurrence.as_ref())?;
        let predicate = canonical_object(draft.event_predicate.as_ref())?;
        let approval =
            canonical_object(Some(&draft.approval_policy))?.ok_or(StoreError::InvalidSchedule)?;
        let template_source =
            std::str::from_utf8(&draft.template_json).map_err(|_| StoreError::InvalidSchedule)?;
        let template = ScheduledTaskTemplateV1::parse_json(template_source)
            .map_err(|_| StoreError::InvalidSchedule)?;
        if matches!(
            draft.template_data_class,
            DataClass::Secret | DataClass::Credential
        ) {
            return Err(StoreError::ClassRefused);
        }
        if (draft.trigger_kind == ScheduleTriggerKind::Calendar) != recurrence.is_some()
            || (draft.trigger_kind == ScheduleTriggerKind::HostEvent) != predicate.is_some()
            || (draft.trigger_kind == ScheduleTriggerKind::Calendar) != draft.timezone.is_some()
            || (draft.trigger_kind == ScheduleTriggerKind::Calendar)
                != draft.recurrence_evaluator.is_some()
            || (draft.trigger_kind == ScheduleTriggerKind::Calendar) != draft.tzdb_version.is_some()
            || (draft.trigger_kind == ScheduleTriggerKind::Calendar) != draft.next_due_at.is_some()
            || (draft.trigger_kind == ScheduleTriggerKind::Calendar)
                != draft.next_local_label.is_some()
            || draft.policy_class_rank > 7
        {
            return Err(StoreError::InvalidSchedule);
        }
        let timestamp = now.get();
        self.operation_savepoint(|tx| {
            let event_high_water: i64 = tx.inner.query_row(
                "SELECT last_allocated_seq FROM event_store_state WHERE singleton=1",
                [],
                |row| row.get(0),
            )?;
            if event_high_water < 0 {
                return Err(StoreError::CorruptRow);
            }
            let event_predicate_after_seq =
                (draft.trigger_kind == ScheduleTriggerKind::HostEvent).then_some(event_high_water);
            let template_blob = tx.put_blob(
                template.canonical_json().as_bytes(),
                draft.template_data_class,
            )?;
            let active: i64 = tx.inner.query_row(
                "SELECT count(*) FROM schedules WHERE state='ACTIVE'",
                [],
                |row| row.get(0),
            )?;
            if active >= MAX_ACTIVE_SCHEDULES {
                return Err(StoreError::ScheduleActiveLimit);
            }
            tx.inner.execute(
                "INSERT INTO schedules(
                   schedule_id,owner_kind,owner_id,state,revision,trigger_kind,
                   recurrence_json,event_predicate_json,template_digest,
                   template_data_class_rank,policy_class_rank,approval_policy_json,
                   timezone,recurrence_evaluator,tzdb_version,next_due_at_ms,
                   next_local_label,missed_policy,created_at_ms,updated_at_ms,event_predicate_after_seq
                 ) VALUES (?1,?2,?3,'ACTIVE',1,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?17,?18)",
                rusqlite::params![
                    draft.schedule_id.as_str(),
                    match draft.owner_kind { ScheduleOwnerKind::Host => "HOST", ScheduleOwnerKind::Device => "DEVICE" },
                    draft.owner_id,
                    draft.trigger_kind.as_str(), recurrence, predicate,
                    template_blob.digest().to_string(),
                    i64::from(template_blob.class().rank()),
                    i64::from(draft.policy_class_rank), approval,
                    draft.timezone, draft.recurrence_evaluator, draft.tzdb_version,
                    draft.next_due_at.map(EpochMillis::get), draft.next_local_label,
                    draft.missed_policy.as_str(), timestamp, event_predicate_after_seq
                ],
            )?;
            Ok(())
        })
    }

    /// Admits one durable occurrence identity without exceeding the
    /// per-schedule pending/claimed/unmapped ceiling.
    pub fn enqueue_schedule_occurrence(
        &mut self,
        draft: ScheduleOccurrenceDraft,
    ) -> Result<bool, StoreError> {
        self.ensure_active()?;
        if draft.occurrence_key.is_empty() || draft.occurrence_key.len() > MAX_KEY_BYTES {
            return Err(StoreError::InvalidSchedule);
        }
        let local = draft.intended_local_label.is_some();
        if (draft.trigger_kind == ScheduleTriggerKind::Calendar) != local
            || (draft.trigger_kind == ScheduleTriggerKind::Calendar)
                != draft.source_event_id.is_none()
            || draft.source_event_id.is_some() != draft.source_event_data_class.is_some()
            || (draft.trigger_kind == ScheduleTriggerKind::Calendar) != draft.timezone.is_some()
            || (draft.trigger_kind == ScheduleTriggerKind::Calendar)
                != draft.recurrence_evaluator.is_some()
            || (draft.trigger_kind == ScheduleTriggerKind::Calendar) != draft.tzdb_version.is_some()
            || draft
                .source_event_data_class
                .is_some_and(|class| class.rank() > 1)
        {
            return Err(StoreError::InvalidSchedule);
        }
        self.operation_savepoint(|tx| {
            let current: Option<(String, i64)> = tx
                .inner
                .query_row(
                    "SELECT state,revision FROM schedules WHERE schedule_id=?1",
                    [draft.schedule_id.as_str()],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .optional()?;
            let Some((state, revision)) = current else {
                return Err(StoreError::ScheduleNotActive);
            };
            if state != "ACTIVE" {
                return Err(StoreError::ScheduleNotActive);
            }
            if revision != i64::from(draft.schedule_revision) {
                return Err(StoreError::ScheduleRevisionConflict);
            }
            let count: i64 = tx.inner.query_row(
                "SELECT count(*) FROM schedule_occurrences
                 WHERE schedule_id=?1 AND state IN ('PENDING','CLAIMED')",
                [draft.schedule_id.as_str()],
                |row| row.get(0),
            )?;
            if count >= MAX_PENDING_OCCURRENCES {
                return Err(StoreError::SchedulePendingOccurrenceLimit);
            }
            let trigger: (String, Option<String>, Option<String>, Option<i64>) = tx.inner.query_row(
                "SELECT trigger_kind,timezone,template_digest,template_data_class_rank FROM schedules WHERE schedule_id=?1",
                [draft.schedule_id.as_str()],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )?;
            if trigger.0 != draft.trigger_kind.as_str() || trigger.1 != draft.timezone {
                return Err(StoreError::InvalidSchedule);
            }
            let (Some(template_digest), Some(template_class_rank)) = (trigger.2, trigger.3) else {
                return Err(StoreError::CorruptRow);
            };
            let inserted = tx.inner.execute(
                "INSERT INTO schedule_occurrences(
                   schedule_id,occurrence_key,schedule_revision,trigger_kind,source_event_id,
                   intended_local_label,timezone,recurrence_evaluator,tzdb_version,due_at_ms,
                   not_before_ms,template_digest,template_data_class_rank,source_event_data_class_rank,
                   state,created_at_ms,updated_at_ms
                 ) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,'PENDING',?15,?15)
                 ON CONFLICT(schedule_id,occurrence_key) DO NOTHING",
                rusqlite::params![
                    draft.schedule_id.as_str(),
                    draft.occurrence_key,
                    i64::from(draft.schedule_revision),
                    draft.trigger_kind.as_str(),
                    draft.source_event_id.as_ref().map(ToString::to_string),
                    draft.intended_local_label,
                    draft.timezone,
                    draft.recurrence_evaluator,
                    draft.tzdb_version,
                    draft.due_at.map(EpochMillis::get),
                    draft.not_before.map(EpochMillis::get),
                    template_digest,
                    template_class_rank,
                    draft.source_event_data_class.map(|class| i64::from(class.rank())),
                    draft.created_at.get()
                ],
            )?;
            if inserted == 0 {
                let prior: (String, Option<String>, Option<String>, Option<String>, Option<i64>) = tx.inner.query_row(
                    "SELECT trigger_kind,source_event_id,intended_local_label,timezone,source_event_data_class_rank
                     FROM schedule_occurrences WHERE schedule_id=?1 AND occurrence_key=?2",
                    rusqlite::params![draft.schedule_id.as_str(), draft.occurrence_key],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?)),
                )?;
                if prior.0 != draft.trigger_kind.as_str()
                    || prior.1 != draft.source_event_id.as_ref().map(ToString::to_string)
                    || prior.2 != draft.intended_local_label
                    || prior.3 != draft.timezone
                    || prior.4
                        != draft.source_event_data_class.map(|class| i64::from(class.rank()))
                {
                    return Err(StoreError::InvalidSchedule);
                }
            }
            Ok(inserted == 1)
        })
    }

    /// Atomically admits the currently due calendar identity and advances the
    /// durable local recurrence cursor. ONCE exhaustion is represented by a
    /// NULL/NULL cursor after its sole occurrence has been admitted.
    pub fn enqueue_calendar_occurrence_and_advance(
        &mut self,
        draft: ScheduleOccurrenceDraft,
        expected_current_label: &str,
        next_due_at: Option<EpochMillis>,
        next_local_label: Option<&str>,
    ) -> Result<bool, StoreError> {
        self.ensure_active()?;
        if draft.trigger_kind != ScheduleTriggerKind::Calendar
            || draft.intended_local_label.as_deref() != Some(expected_current_label)
            || next_due_at.is_some() != next_local_label.is_some()
        {
            return Err(StoreError::InvalidSchedule);
        }
        self.operation_savepoint(|tx| {
            let current: Option<(i64, Option<String>)> = tx
                .inner
                .query_row(
                    "SELECT revision,next_local_label FROM schedules
                 WHERE schedule_id=?1 AND state='ACTIVE' AND trigger_kind='CALENDAR'",
                    [draft.schedule_id.as_str()],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .optional()?;
            let Some((revision, current_label)) = current else {
                return Err(StoreError::ScheduleNotActive);
            };
            if revision != i64::from(draft.schedule_revision)
                || current_label.as_deref() != Some(expected_current_label)
            {
                return Err(StoreError::ScheduleRevisionConflict);
            }
            let inserted = tx.enqueue_schedule_occurrence(draft.clone())?;
            let changed = tx.inner.execute(
                "UPDATE schedules SET next_due_at_ms=?3,next_local_label=?4
                 WHERE schedule_id=?1 AND state='ACTIVE' AND revision=?2
                   AND next_local_label=?5",
                rusqlite::params![
                    draft.schedule_id.as_str(),
                    i64::from(draft.schedule_revision),
                    next_due_at.map(EpochMillis::get),
                    next_local_label,
                    expected_current_label,
                ],
            )?;
            if changed != 1 {
                return Err(StoreError::ScheduleRevisionConflict);
            }
            Ok(inserted)
        })
    }

    /// For RUN_ONCE recurrence expansion, supersedes the prior pending
    /// candidate in the same transaction that admits the newer identity and
    /// advances the recurrence cursor. Older candidates are durably SKIPPED
    /// with their required lifecycle event.
    pub fn enqueue_run_once_candidate_and_advance(
        &mut self,
        draft: ScheduleOccurrenceDraft,
        expected_current_label: &str,
        next_due_at: Option<EpochMillis>,
        next_local_label: Option<&str>,
        expected_previous_key: Option<&str>,
        missed_event: Option<EventDraft>,
    ) -> Result<bool, StoreError> {
        self.ensure_active()?;
        self.operation_savepoint(|tx| {
            let (state, revision, policy, current_label): (String, i64, String, Option<String>) =
                tx.inner.query_row(
                    "SELECT state,revision,missed_policy,next_local_label FROM schedules
                     WHERE schedule_id=?1 AND trigger_kind='CALENDAR'",
                    [draft.schedule_id.as_str()],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
                )?;
            if state != "ACTIVE"
                || revision != i64::from(draft.schedule_revision)
                || policy != "RUN_ONCE"
                || current_label.as_deref() != Some(expected_current_label)
            {
                return Err(StoreError::ScheduleRevisionConflict);
            }
            let previous: Option<(String, i64)> = tx
                .inner
                .query_row(
                    "SELECT occurrence_key,updated_at_ms FROM schedule_occurrences
                     WHERE schedule_id=?1 AND state='PENDING' AND trigger_kind='CALENDAR'
                     ORDER BY due_at_ms DESC,occurrence_key DESC LIMIT 1",
                    [draft.schedule_id.as_str()],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .optional()?;
            if previous.as_ref().map(|row| row.0.as_str()) != expected_previous_key {
                return Err(StoreError::ScheduleRevisionConflict);
            }
            if let Some((previous_key, _)) = previous {
                if previous_key != draft.occurrence_key {
                    let event = missed_event.ok_or(StoreError::AuditRejected)?;
                    let changed = tx.inner.execute(
                        "UPDATE schedule_occurrences SET state='SKIPPED',processed_at_ms=?3,
                           updated_at_ms=?3,outcome_code='COALESCED_RUN_ONCE'
                         WHERE schedule_id=?1 AND occurrence_key=?2 AND state='PENDING'",
                        rusqlite::params![
                            draft.schedule_id.as_str(),
                            previous_key,
                            draft.created_at.get()
                        ],
                    )?;
                    if changed != 1 {
                        return Err(StoreError::ScheduleOccurrenceNotClaimable);
                    }
                    tx.append_event(event.event, event.retention_at)?;
                }
            }
            tx.enqueue_calendar_occurrence_and_advance(
                draft,
                expected_current_label,
                next_due_at,
                next_local_label,
            )
        })
    }

    /// Marks one due occurrence skipped and appends SCHEDULE_OCCURRENCE_MISSED
    /// atomically. Duplicate processing of an already-skipped row is a no-op.
    pub fn skip_schedule_occurrence_with_event(
        &mut self,
        schedule_id: &ScheduleId,
        occurrence_key: &str,
        expected_revision: u32,
        now: EpochMillis,
        event: EventDraft,
    ) -> Result<bool, StoreError> {
        self.ensure_active()?;
        self.operation_savepoint(|tx| {
            let schedule: Option<(String, i64)> = tx
                .inner
                .query_row(
                    "SELECT state,revision FROM schedules WHERE schedule_id=?1",
                    [schedule_id.as_str()],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .optional()?;
            let Some((state, revision)) = schedule else {
                return Err(StoreError::ScheduleNotActive);
            };
            if state != "ACTIVE" || revision != i64::from(expected_revision) {
                return Err(StoreError::ScheduleRevisionConflict);
            }
            let due: Option<(String, Option<i64>)> = tx
                .inner
                .query_row(
                    "SELECT state,due_at_ms FROM schedule_occurrences
                     WHERE schedule_id=?1 AND occurrence_key=?2",
                    rusqlite::params![schedule_id.as_str(), occurrence_key],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .optional()?;
            let Some((occurrence_state, due_at)) = due else {
                return Err(StoreError::ScheduleOccurrenceNotClaimable);
            };
            if occurrence_state == "SKIPPED" {
                return Ok(false);
            }
            if occurrence_state != "PENDING" || due_at.is_none_or(|due_at| due_at > now.get()) {
                return Err(StoreError::ScheduleOccurrenceNotClaimable);
            }
            let changed = tx.inner.execute(
                "UPDATE schedule_occurrences SET state='SKIPPED',processed_at_ms=?3,
                   updated_at_ms=?3,outcome_code='MISSED_POLICY_SKIP'
                 WHERE schedule_id=?1 AND occurrence_key=?2 AND state='PENDING'",
                rusqlite::params![schedule_id.as_str(), occurrence_key, now.get()],
            )?;
            if changed != 1 {
                return Err(StoreError::ScheduleOccurrenceNotClaimable);
            }
            tx.append_event(event.event, event.retention_at)?;
            Ok(true)
        })
    }

    /// Defers all currently due pending occurrences and emits one durable
    /// SCHEDULE_CATCH_UP_DEFERRED event in the same transaction.
    pub fn defer_due_schedule_occurrences(
        &mut self,
        schedule_id: &ScheduleId,
        expected_revision: u32,
        expected_first_key: &str,
        now: EpochMillis,
        not_before: EpochMillis,
        event: EventDraft,
    ) -> Result<u16, StoreError> {
        self.ensure_active()?;
        if not_before <= now {
            return Err(StoreError::InvalidSchedule);
        }
        self.operation_savepoint(|tx| {
            let (state, revision): (String, i64) = tx.inner.query_row(
                "SELECT state,revision FROM schedules WHERE schedule_id=?1",
                [schedule_id.as_str()],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )?;
            if state != "ACTIVE" || revision != i64::from(expected_revision) {
                return Err(StoreError::ScheduleRevisionConflict);
            }
            let first: Option<String> = tx
                .inner
                .query_row(
                    "SELECT occurrence_key FROM schedule_occurrences
                     WHERE schedule_id=?1 AND state='PENDING'
                       AND due_at_ms<=?2 AND (not_before_ms IS NULL OR not_before_ms<=?2)
                     ORDER BY due_at_ms,occurrence_key LIMIT 1",
                    rusqlite::params![schedule_id.as_str(), now.get()],
                    |row| row.get(0),
                )
                .optional()?;
            let Some(first) = first else { return Ok(0) };
            if first != expected_first_key {
                return Err(StoreError::ScheduleRevisionConflict);
            }
            tx.inner.execute(
                "UPDATE schedule_occurrences SET not_before_ms=?3,updated_at_ms=?2
                 WHERE schedule_id=?1 AND state='PENDING' AND due_at_ms<=?2
                   AND (not_before_ms IS NULL OR not_before_ms<=?2)",
                rusqlite::params![schedule_id.as_str(), now.get(), not_before.get()],
            )?;
            let count: i64 = tx
                .inner
                .query_row("SELECT changes()", [], |row| row.get(0))?;
            tx.append_event(event.event, event.retention_at)?;
            u16::try_from(count).map_err(|_| StoreError::CorruptRow)
        })
    }

    /// Claims one already-durable occurrence under the schedule revision and
    /// occurrence lease-generation fence. Expiry permits reconciliation only.
    pub fn claim_schedule_occurrence(
        &mut self,
        schedule_id: &ScheduleId,
        occurrence_key: &str,
        expected_revision: u32,
        lease_owner: &str,
        now: EpochMillis,
        lease_expires_at: EpochMillis,
    ) -> Result<ScheduleOccurrenceLease, StoreError> {
        self.ensure_active()?;
        if occurrence_key.is_empty()
            || occurrence_key.len() > MAX_KEY_BYTES
            || lease_owner.is_empty()
            || lease_owner.len() > MAX_OWNER_BYTES
            || expected_revision == 0
            || lease_expires_at.get() <= now.get()
            || lease_expires_at.get() - now.get() > MAX_LEASE_MS
        {
            return Err(StoreError::InvalidLeaseInterval);
        }
        self.operation_savepoint(|tx| {
            let schedule: Option<(String, i64)> = tx
                .inner
                .query_row(
                    "SELECT state,revision FROM schedules WHERE schedule_id=?1",
                    [schedule_id.as_str()],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .optional()?;
            let Some((state, revision)) = schedule else {
                return Err(StoreError::ScheduleNotActive);
            };
            if state != "ACTIVE" {
                return Err(StoreError::ScheduleNotActive);
            }
            if revision != i64::from(expected_revision) {
                return Err(StoreError::ScheduleRevisionConflict);
            }

            let occurrence: Option<ClaimOccurrenceRow> = tx
                .inner
                .query_row(
                    "SELECT state,schedule_revision,lease_expires_at_ms,lease_owner,lease_generation
                     FROM schedule_occurrences WHERE schedule_id=?1 AND occurrence_key=?2",
                    rusqlite::params![schedule_id.as_str(), occurrence_key],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?)),
                )
                .optional()?;
            let Some((occurrence_state, occurrence_revision, expires, _owner, generation)) =
                occurrence
            else {
                return Err(StoreError::ScheduleOccurrenceNotClaimable);
            };
            if occurrence_revision > revision {
                return Err(StoreError::ScheduleRevisionConflict);
            }
            let pending: i64 = tx.inner.query_row(
                "SELECT count(*) FROM schedule_occurrences
                 WHERE schedule_id=?1 AND state IN ('PENDING','CLAIMED')",
                [schedule_id.as_str()],
                |row| row.get(0),
            )?;
            if pending > MAX_PENDING_OCCURRENCES {
                return Err(StoreError::SchedulePendingOccurrenceLimit);
            }
            let reclaimable = occurrence_state == "PENDING"
                || (occurrence_state == "CLAIMED"
                    && expires.is_some_and(|until| until <= now.get()));
            if !reclaimable {
                return Err(StoreError::ScheduleOccurrenceNotClaimable);
            }
            let generation = u32::try_from(generation)
                .map_err(|_| StoreError::CorruptRow)?
                .checked_add(1)
                .ok_or(StoreError::SchedulerLeaseGenerationOverflow)?;
            let due: bool = tx.inner.query_row(
                "SELECT (due_at_ms IS NULL OR due_at_ms<=?3)
                        AND (not_before_ms IS NULL OR not_before_ms<=?3)
                 FROM schedule_occurrences WHERE schedule_id=?1 AND occurrence_key=?2",
                rusqlite::params![schedule_id.as_str(), occurrence_key, now.get()],
                |row| row.get(0),
            )?;
            if !due {
                return Err(StoreError::ScheduleOccurrenceNotClaimable);
            }
            let changed = tx.inner.execute(
                "UPDATE schedule_occurrences SET state='CLAIMED',lease_owner=?3,
                   lease_generation=?4,lease_expires_at_ms=?5,updated_at_ms=?6
                 WHERE schedule_id=?1 AND occurrence_key=?2
                   AND schedule_revision=?7 AND state=?8
                   AND lease_generation=?9",
                rusqlite::params![
                    schedule_id.as_str(),
                    occurrence_key,
                    lease_owner,
                    i64::from(generation),
                    lease_expires_at.get(),
                    now.get(),
                    occurrence_revision,
                    occurrence_state,
                    generation - 1
                ],
            )?;
            if changed != 1 {
                return Err(StoreError::ScheduleOccurrenceNotClaimable);
            }
            Ok(ScheduleOccurrenceLease {
                schedule_id: schedule_id.clone(),
                occurrence_key: occurrence_key.to_owned(),
                schedule_revision: expected_revision,
                lease_owner: lease_owner.to_owned(),
                lease_generation: generation,
                lease_expires_at,
            })
        })
    }

    /// Persists the occurrence-to-task mapping under the latest occurrence
    /// fence. Repeating the committed mapping with the same TaskId is harmless.
    /// A cancellation after claim does not erase or cancel this occurrence.
    pub fn map_schedule_occurrence(
        &mut self,
        lease: &ScheduleOccurrenceLease,
        task_id: &TaskId,
        now: EpochMillis,
    ) -> Result<(), StoreError> {
        self.ensure_active()?;
        self.operation_savepoint(|tx| {
            let current: Option<OccurrenceMappingRow> = tx
                .inner
                .query_row(
                    "SELECT state,mapped_task_id,lease_generation,lease_expires_at_ms,updated_at_ms
                     FROM schedule_occurrences WHERE schedule_id=?1 AND occurrence_key=?2",
                    rusqlite::params![lease.schedule_id.as_str(), lease.occurrence_key],
                    |row| {
                        Ok((
                            row.get(0)?,
                            row.get(1)?,
                            row.get(2)?,
                            row.get(3)?,
                            row.get(4)?,
                        ))
                    },
                )
                .optional()?;
            let Some((state, mapped, generation, expires, updated_at)) = current else {
                return Err(StoreError::StaleSchedulerLease);
            };
            if state == "MAPPED" {
                return if mapped.as_deref() == Some(task_id.as_str()) {
                    Ok(())
                } else {
                    Err(StoreError::ScheduleOccurrenceAlreadyMapped)
                };
            }
            if state != "CLAIMED"
                || generation != i64::from(lease.lease_generation)
                || expires.is_none_or(|until| until <= now.get())
                || lease.lease_expires_at.get() <= now.get()
                || now.get() < updated_at
            {
                return Err(StoreError::StaleSchedulerLease);
            }
            let changed = tx.inner.execute(
                "UPDATE schedule_occurrences SET state='MAPPED',mapped_task_id=?3,
                   lease_owner=NULL,lease_expires_at_ms=NULL,processed_at_ms=?4,updated_at_ms=?4
                 WHERE schedule_id=?1 AND occurrence_key=?2 AND state='CLAIMED'
                   AND lease_owner=?5 AND lease_generation=?6
                   AND lease_expires_at_ms>?4",
                rusqlite::params![
                    lease.schedule_id.as_str(),
                    lease.occurrence_key,
                    task_id.as_str(),
                    now.get(),
                    lease.lease_owner,
                    i64::from(lease.lease_generation)
                ],
            )?;
            if changed != 1 {
                return Err(StoreError::StaleSchedulerLease);
            }
            Ok(())
        })
    }

    /// Maps a task and appends the matching Scheduler lifecycle event in one
    /// caller transaction. This is the composition point used by TaskEngine.
    pub fn map_schedule_occurrence_with_event(
        &mut self,
        lease: &ScheduleOccurrenceLease,
        task_id: &TaskId,
        now: EpochMillis,
        event: EventDraft,
    ) -> Result<(), StoreError> {
        self.operation_savepoint(|tx| {
            let row: Option<ScheduleTaskEventValidationRow> = tx
                .inner
                .query_row(
                    "SELECT source_event_id,template_data_class_rank,source_event_data_class_rank,
                        mapped_task_id,lease_generation
                 FROM schedule_occurrences WHERE schedule_id=?1 AND occurrence_key=?2",
                    rusqlite::params![lease.schedule_id.as_str(), lease.occurrence_key],
                    |row| {
                        Ok((
                            row.get(0)?,
                            row.get(1)?,
                            row.get(2)?,
                            row.get(3)?,
                            row.get(4)?,
                        ))
                    },
                )
                .optional()?;
            let Some((source_id, data_class_rank, source_class_rank, _, _)) = row else {
                return Err(StoreError::ScheduleOccurrenceNotClaimable);
            };
            let template_class = data_class_from_rank(
                u8::try_from(data_class_rank).map_err(|_| StoreError::CorruptRow)?,
            )?;
            let source_class = source_class_rank
                .map(|rank| {
                    data_class_from_rank(u8::try_from(rank).map_err(|_| StoreError::CorruptRow)?)
                })
                .transpose()?;
            if source_id.is_some() != source_class.is_some() {
                return Err(StoreError::CorruptRow);
            }
            let expected_class =
                DataClass::compose(template_class, source_class.unwrap_or(DataClass::Public));
            let source_id = source_id
                .map(EventId::new)
                .transpose()
                .map_err(|_| StoreError::CorruptRow)?;
            let payload = &event.event.payload;
            if event.event.kind != EventKind::ScheduleTaskCreated
                || event.event.data_class != expected_class
                || event.event.correlation_id.as_ref() != Some(task_id)
                || event.event.causation_id != source_id
                || payload
                    .get("schedule_id")
                    .and_then(serde_json::Value::as_str)
                    != Some(lease.schedule_id.as_str())
                || payload
                    .get("occurrence_key")
                    .and_then(serde_json::Value::as_str)
                    != Some(lease.occurrence_key.as_str())
                || payload.get("task_id").and_then(serde_json::Value::as_str)
                    != Some(task_id.as_str())
                || payload
                    .get("source_event_id")
                    .and_then(serde_json::Value::as_str)
                    != source_id.as_ref().map(EventId::as_str)
            {
                return Err(StoreError::AuditRejected);
            }
            let task_row: Option<(String, i64)> = tx
                .inner
                .query_row(
                    "SELECT kind,data_class_rank FROM tasks WHERE task_id=?1",
                    [task_id.as_str()],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .optional()?;
            let Some((kind, rank)) = task_row else {
                return Err(StoreError::StaleSchedulerLease);
            };
            if kind != "SCHEDULED" || rank != i64::from(expected_class.rank()) {
                return Err(StoreError::InvalidPlan);
            }
            tx.map_schedule_occurrence(lease, task_id, now)?;
            tx.append_event(event.event, event.retention_at)?;
            Ok(())
        })
    }

    /// Returns the already-committed task for an occurrence. A mapped row with
    /// a missing/invalid TaskId is corruption, never a cue to mint another task.
    pub fn schedule_occurrence_task(
        &self,
        schedule_id: &ScheduleId,
        occurrence_key: &str,
    ) -> Result<Option<TaskId>, StoreError> {
        self.ensure_active()?;
        let row: Option<(String, Option<String>)> = self
            .inner
            .query_row(
                "SELECT state,mapped_task_id FROM schedule_occurrences
             WHERE schedule_id=?1 AND occurrence_key=?2",
                rusqlite::params![schedule_id.as_str(), occurrence_key],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;
        match row {
            None => Err(StoreError::ScheduleOccurrenceNotClaimable),
            Some((state, Some(id))) if state == "MAPPED" => TaskId::new(id)
                .map(Some)
                .map_err(|_| StoreError::CorruptRow),
            Some((state, None)) if state != "MAPPED" => Ok(None),
            Some(_) => Err(StoreError::CorruptRow),
        }
    }

    /// Resolves a mapped scheduled task back to the exact immutable template
    /// version captured by its occurrence. Returns `None` for ordinary tasks.
    pub fn schedule_occurrence_for_task(
        &self,
        task_id: &TaskId,
    ) -> Result<Option<ScheduleTaskProvenance>, StoreError> {
        self.ensure_active()?;
        let row: Option<ScheduleTaskProvenanceRow> = self
            .inner
            .query_row(
                "SELECT schedule_id,occurrence_key,source_event_id,intended_local_label,timezone,
                        template_digest,template_data_class_rank
                 FROM schedule_occurrences WHERE mapped_task_id=?1 AND state='MAPPED'",
                [task_id.as_str()],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                        row.get(5)?,
                        row.get(6)?,
                    ))
                },
            )
            .optional()?;
        row.map(|(schedule, key, source, label, timezone, digest, rank)| {
            Ok(ScheduleTaskProvenance {
                schedule_id: ScheduleId::new(schedule).map_err(|_| StoreError::CorruptRow)?,
                occurrence_key: key,
                task_id: task_id.clone(),
                source_event_id: source
                    .map(EventId::new)
                    .transpose()
                    .map_err(|_| StoreError::CorruptRow)?,
                intended_local_label: label,
                timezone,
                template: crate::BlobRef::new(
                    Digest::new(digest).map_err(|_| StoreError::CorruptRow)?,
                    data_class_from_rank(u8::try_from(rank).map_err(|_| StoreError::CorruptRow)?)?,
                ),
            })
        })
        .transpose()
    }
}

impl crate::Store {
    /// Resolves durable scheduler provenance for a task; ordinary tasks return
    /// `None`. The referenced bytes remain separately readable via `get_blob`.
    pub fn schedule_task_provenance(
        &self,
        task_id: &TaskId,
    ) -> Result<Option<ScheduleTaskProvenance>, StoreError> {
        self.transact(|tx| tx.schedule_occurrence_for_task(task_id))
    }

    /// Lists pending approval handoffs without acknowledging or consuming them.
    pub fn pending_approval_lifecycle_wakes(
        &self,
        limit: u16,
    ) -> Result<Vec<ApprovalLifecycleWake>, StoreError> {
        self.transact(|tx| tx.pending_approval_lifecycle_wakes(limit))
    }

    /// Reads a pending approval handoff. Reading leaves the durable wake intact.
    pub fn approval_lifecycle_wake(
        &self,
        source_event_id: &EventId,
    ) -> Result<Option<ApprovalLifecycleWake>, StoreError> {
        self.transact(|tx| tx.approval_lifecycle_wake(source_event_id))
    }

    /// Explicitly acknowledges a pending handoff after the consumer's own
    /// durable operation. A repeated acknowledgement returns `false`.
    pub fn acknowledge_approval_lifecycle_wake(
        &self,
        source_event_id: &EventId,
    ) -> Result<bool, StoreError> {
        self.transact(|tx| tx.acknowledge_approval_lifecycle_wake(source_event_id))
    }
}

use rusqlite::OptionalExtension;

fn approval_wake_from_row(
    row: (String, i64, String, String, String, String, i64),
) -> Result<ApprovalLifecycleWake, StoreError> {
    Ok(ApprovalLifecycleWake {
        source_event_id: EventId::new(row.0).map_err(|_| StoreError::CorruptRow)?,
        source_seq: u64::try_from(row.1).map_err(|_| StoreError::CorruptRow)?,
        approval_id: ApprovalId::new(row.2).map_err(|_| StoreError::CorruptRow)?,
        task_id: TaskId::new(row.3).map_err(|_| StoreError::CorruptRow)?,
        step_id: StepId::new(row.4).map_err(|_| StoreError::CorruptRow)?,
        outcome: ApprovalLifecycleOutcome::from_str(&row.5)?,
        created_at: EpochMillis::new(row.6).map_err(|_| StoreError::CorruptRow)?,
    })
}

/// Checks cross-row invariants introduced by P3 which SQLite constraints alone
/// cannot express. Event sources may have expired content only when their
/// sequence is covered by the intentional expiry ledger.
pub(crate) fn validate_scheduler_integrity(conn: &Connection) -> Result<(), StoreError> {
    let last_seq: i64 = conn.query_row(
        "SELECT last_allocated_seq FROM event_store_state WHERE singleton=1",
        [],
        |row| row.get(0),
    )?;
    let corrupt_task_revision: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM tasks WHERE state_revision < 1)",
        [],
        |row| row.get(0),
    )?;
    let corrupt_device_wait: bool = conn.query_row(
        "SELECT EXISTS(
           SELECT 1 FROM device_resume_waits w
           LEFT JOIN tasks t USING(task_id)
           WHERE t.task_id IS NULL OR t.state <> 'BLOCKED'
             OR t.blocked_reason <> 'DEVICE_OFFLINE'
             OR t.state_revision <> w.blocked_task_revision
         ) OR EXISTS(
           SELECT 1 FROM tasks t
           WHERE t.state='BLOCKED' AND t.blocked_reason='DEVICE_OFFLINE'
             AND NOT EXISTS(SELECT 1 FROM device_resume_waits w WHERE w.task_id=t.task_id)
         )",
        [],
        |row| row.get(0),
    )?;
    let corrupt_cursor: bool = conn.query_row(
        "SELECT EXISTS(
           SELECT 1 FROM scheduler_consumer_state c
           JOIN event_store_state e ON e.singleton=c.singleton
           WHERE c.last_processed_seq > e.last_allocated_seq
              OR (c.replay_high_water_seq IS NOT NULL AND
                  (c.replay_high_water_seq < c.last_processed_seq OR
                   c.replay_high_water_seq > e.last_allocated_seq))
         )",
        [],
        |row| row.get(0),
    )?;
    let corrupt_wake: bool = conn.query_row(
        "SELECT EXISTS(
           SELECT 1 FROM device_session_resume_wakes w
           WHERE w.source_seq > ?1 OR
             (EXISTS(SELECT 1 FROM event_content c WHERE c.seq=w.source_seq) AND NOT EXISTS(
               SELECT 1 FROM event_content c WHERE c.seq=w.source_seq
                 AND c.message_id=w.source_event_id AND c.kind='DEVICE_CONNECTED'
                 AND json_extract(c.event_json,'$.payload.device_id')=w.device_id
             )) OR
             (NOT EXISTS(SELECT 1 FROM event_content c WHERE c.seq=w.source_seq) AND
               w.source_seq > (SELECT expired_prefix_through FROM event_store_state WHERE singleton=1)
               AND NOT EXISTS(SELECT 1 FROM event_expired_ranges r
                              WHERE r.first_seq<=w.source_seq AND r.last_seq>=w.source_seq))
           UNION ALL
           SELECT 1 FROM approval_lifecycle_wakes w
           WHERE w.source_seq > ?1 OR
             (EXISTS(SELECT 1 FROM event_content c WHERE c.seq=w.source_seq) AND NOT EXISTS(
               SELECT 1 FROM event_content c WHERE c.seq=w.source_seq
                 AND c.message_id=w.source_event_id
                 AND c.kind=CASE w.outcome_kind
                   WHEN 'GRANTED' THEN 'APPROVAL_GRANTED'
                   WHEN 'DENIED' THEN 'APPROVAL_DENIED'
                   WHEN 'EXPIRED' THEN 'APPROVAL_EXPIRED' END
                 AND json_extract(c.event_json,'$.payload.approval_id')=w.approval_id
                 AND json_extract(c.event_json,'$.payload.task_id')=w.task_id
                 AND json_extract(c.event_json,'$.payload.step_id')=w.step_id
                 AND json_extract(c.event_json,'$.correlation_id')=w.task_id
                 AND json_extract(c.event_json,'$.trace.task_id')=w.task_id
                 AND json_extract(c.event_json,'$.trace.step_id')=w.step_id
             )) OR
             (NOT EXISTS(SELECT 1 FROM event_content c WHERE c.seq=w.source_seq) AND
               w.source_seq > (SELECT expired_prefix_through FROM event_store_state WHERE singleton=1)
               AND NOT EXISTS(SELECT 1 FROM event_expired_ranges r
                              WHERE r.first_seq<=w.source_seq AND r.last_seq>=w.source_seq))
         )",
        [last_seq],
        |row| row.get(0),
    )?;
    if corrupt_task_revision || corrupt_device_wait || corrupt_cursor || corrupt_wake {
        return Err(StoreError::IntegrityCheckFailed);
    }

    let schedule_values = {
        let mut statement =
            conn.prepare("SELECT recurrence_json,event_predicate_json FROM schedules")?;
        statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, Option<String>>(0)?,
                    row.get::<_, Option<String>>(1)?,
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?
    };
    for (recurrence, predicate) in schedule_values {
        for value in recurrence.iter().chain(predicate.iter()) {
            if canonicalize(value).ok().as_deref() != Some(value.as_bytes()) {
                return Err(StoreError::IntegrityCheckFailed);
            }
        }
        if predicate
            .as_deref()
            .is_some_and(|value| serea_protocol::EventPredicateV1::parse_json(value).is_err())
        {
            return Err(StoreError::IntegrityCheckFailed);
        }
    }
    Ok(())
}

fn canonical_object(value: Option<&serde_json::Value>) -> Result<Option<String>, StoreError> {
    let Some(value) = value else { return Ok(None) };
    if !value.is_object() {
        return Err(StoreError::InvalidSchedule);
    }
    let source = serde_json::to_string(value).map_err(|_| StoreError::CanonicalJson)?;
    let bytes = serea_protocol::canonicalize(&source).map_err(|_| StoreError::CanonicalJson)?;
    String::from_utf8(bytes)
        .map(Some)
        .map_err(|_| StoreError::CanonicalJson)
}
