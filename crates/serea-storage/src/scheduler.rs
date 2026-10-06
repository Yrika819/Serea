use crate::{EventDraft, StoreError, Tx};
use serea_protocol::{Digest, EpochMillis, EventId, EventKind, ScheduleId, TaskId};

const MAX_LEASE_MS: i64 = 120_000;
const MAX_KEY_BYTES: usize = 512;
const MAX_OWNER_BYTES: usize = 128;
const MAX_ACTIVE_SCHEDULES: i64 = 256;
const MAX_PENDING_OCCURRENCES: i64 = 256;
type ClaimOccurrenceRow = (String, i64, Option<i64>, Option<String>, i64);
type OccurrenceMappingRow = (String, Option<String>, i64, Option<i64>, i64);

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
    pub template_digest: Option<Digest>,
    pub template_data_class_rank: Option<u8>,
    pub policy_class_rank: u8,
    pub approval_policy: serde_json::Value,
    pub timezone: Option<String>,
    pub recurrence_evaluator: Option<String>,
    pub tzdb_version: Option<String>,
    pub next_due_at: Option<EpochMillis>,
    pub next_local_label: Option<String>,
    pub missed_policy: MissedOccurrencePolicy,
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

    /// Applies a cancellation command, appends its lifecycle event, and stores
    /// the authenticated command result in this same transaction. A retry with
    /// the same envelope ID/digest returns the committed result without
    /// appending another event.
    pub fn cancel_schedule_command(
        &mut self,
        message_id: &EventId,
        request_digest: &Digest,
        schedule_id: &ScheduleId,
        expected_revision: u32,
        now: EpochMillis,
        event: EventDraft,
    ) -> Result<ScheduleCommandOutcome, StoreError> {
        self.ensure_active()?;
        if event.event.kind != EventKind::ScheduleCancelled
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
            if let Some((saved_digest, command_kind, saved_schedule_id, revision, state)) = existing
            {
                if saved_digest != request_digest.to_string()
                    || command_kind != "CANCEL"
                    || saved_schedule_id != schedule_id.as_str()
                    || state != "CANCELLED"
                {
                    return Err(StoreError::ScheduleCommandIdentityConflict);
                }
                return Ok(ScheduleCommandOutcome {
                    schedule_id: schedule_id.clone(),
                    revision: u32::try_from(revision).map_err(|_| StoreError::CorruptRow)?,
                    state: ScheduleCommandState::Cancelled,
                    replayed: true,
                });
            }

            let revision = tx.cancel_schedule(schedule_id, expected_revision, now)?;
            tx.append_event(event.event, event.retention_at)?;
            tx.inner.execute(
                "INSERT INTO schedule_command_receipts(
                   message_id,request_digest,command_kind,schedule_id,result_revision,
                   result_state,committed_at_ms
                 ) VALUES (?1,?2,'CANCEL',?3,?4,'CANCELLED',?5)",
                rusqlite::params![
                    message_id.as_str(),
                    request_digest.to_string(),
                    schedule_id.as_str(),
                    i64::from(revision),
                    now.get()
                ],
            )?;
            Ok(ScheduleCommandOutcome {
                schedule_id: schedule_id.clone(),
                revision,
                state: ScheduleCommandState::Cancelled,
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
        if (draft.trigger_kind == ScheduleTriggerKind::Calendar) != recurrence.is_some()
            || (draft.trigger_kind != ScheduleTriggerKind::Calendar) != predicate.is_some()
            || (draft.trigger_kind == ScheduleTriggerKind::Calendar) != draft.timezone.is_some()
            || (draft.trigger_kind == ScheduleTriggerKind::Calendar)
                != draft.recurrence_evaluator.is_some()
            || (draft.trigger_kind == ScheduleTriggerKind::Calendar) != draft.tzdb_version.is_some()
            || (draft.trigger_kind == ScheduleTriggerKind::Calendar) != draft.next_due_at.is_some()
            || (draft.trigger_kind == ScheduleTriggerKind::Calendar)
                != draft.next_local_label.is_some()
            || draft.policy_class_rank > 7
            || draft.template_data_class_rank.is_some_and(|rank| rank > 2)
            || draft.template_digest.is_some() != draft.template_data_class_rank.is_some()
        {
            return Err(StoreError::InvalidSchedule);
        }
        let timestamp = now.get();
        self.operation_savepoint(|tx| {
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
                   next_local_label,missed_policy,created_at_ms,updated_at_ms
                 ) VALUES (?1,?2,?3,'ACTIVE',1,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?17)",
                rusqlite::params![
                    draft.schedule_id.as_str(),
                    match draft.owner_kind { ScheduleOwnerKind::Host => "HOST", ScheduleOwnerKind::Device => "DEVICE" },
                    draft.owner_id,
                    draft.trigger_kind.as_str(), recurrence, predicate,
                    draft.template_digest.map(|digest| digest.to_string()),
                    draft.template_data_class_rank.map(i64::from),
                    i64::from(draft.policy_class_rank), approval,
                    draft.timezone, draft.recurrence_evaluator, draft.tzdb_version,
                    draft.next_due_at.map(EpochMillis::get), draft.next_local_label,
                    draft.missed_policy.as_str(), timestamp
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
    ) -> Result<(), StoreError> {
        self.ensure_active()?;
        if draft.occurrence_key.is_empty() || draft.occurrence_key.len() > MAX_KEY_BYTES {
            return Err(StoreError::InvalidSchedule);
        }
        let local = draft.intended_local_label.is_some();
        if (draft.trigger_kind == ScheduleTriggerKind::Calendar) != local
            || (draft.trigger_kind == ScheduleTriggerKind::Calendar)
                != draft.source_event_id.is_none()
            || (draft.trigger_kind == ScheduleTriggerKind::Calendar) != draft.timezone.is_some()
            || (draft.trigger_kind == ScheduleTriggerKind::Calendar)
                != draft.recurrence_evaluator.is_some()
            || (draft.trigger_kind == ScheduleTriggerKind::Calendar) != draft.tzdb_version.is_some()
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
            let trigger: (String, Option<String>) = tx.inner.query_row(
                "SELECT trigger_kind,timezone FROM schedules WHERE schedule_id=?1",
                [draft.schedule_id.as_str()],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )?;
            if trigger.0 != draft.trigger_kind.as_str() || trigger.1 != draft.timezone {
                return Err(StoreError::InvalidSchedule);
            }
            tx.inner.execute(
                "INSERT INTO schedule_occurrences(
                   schedule_id,occurrence_key,schedule_revision,trigger_kind,source_event_id,
                   intended_local_label,timezone,recurrence_evaluator,tzdb_version,due_at_ms,
                   not_before_ms,state,created_at_ms,updated_at_ms
                 ) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,'PENDING',?12,?12)",
                rusqlite::params![
                    draft.schedule_id.as_str(),
                    draft.occurrence_key,
                    i64::from(draft.schedule_revision),
                    draft.trigger_kind.as_str(),
                    draft.source_event_id.map(|id| id.to_string()),
                    draft.intended_local_label,
                    draft.timezone,
                    draft.recurrence_evaluator,
                    draft.tzdb_version,
                    draft.due_at.map(EpochMillis::get),
                    draft.not_before.map(EpochMillis::get),
                    draft.created_at.get()
                ],
            )?;
            Ok(())
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
            if occurrence_revision != revision {
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
                    revision,
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

    /// Linearizes cancellation with claim through SQLite's immediate writer
    /// transaction. Existing claims remain reconcilable after cancellation.
    pub(crate) fn cancel_schedule(
        &mut self,
        schedule_id: &ScheduleId,
        expected_revision: u32,
        now: EpochMillis,
    ) -> Result<u32, StoreError> {
        self.ensure_active()?;
        if expected_revision == 0 {
            return Err(StoreError::ScheduleRevisionConflict);
        }
        self.operation_savepoint(|tx| {
            let current: Option<(String, i64, i64, i64)> = tx
                .inner
                .query_row(
                    "SELECT state,revision,created_at_ms,updated_at_ms FROM schedules WHERE schedule_id=?1",
                    [schedule_id.as_str()],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
                )
                .optional()?;
            let Some((state, revision, created_at, updated_at)) = current else {
                return Err(StoreError::ScheduleNotActive);
            };
            if revision != i64::from(expected_revision) {
                return Err(StoreError::ScheduleRevisionConflict);
            }
            if state == "CANCELLED" {
                return Err(StoreError::ScheduleNotActive);
            }
            if now.get() < created_at || now.get() < updated_at {
                return Err(StoreError::InvalidTimestamp);
            }
            let next = expected_revision
                .checked_add(1)
                .ok_or(StoreError::PlanRevisionOverflow)?;
            let changed = tx.inner.execute(
                "UPDATE schedules SET state='CANCELLED',revision=?2,updated_at_ms=?3,
                   cancelled_at_ms=?3 WHERE schedule_id=?1 AND revision=?4
                   AND state IN ('ACTIVE','PAUSED')",
                rusqlite::params![schedule_id.as_str(), i64::from(next), now.get(), revision],
            )?;
            if changed != 1 {
                return Err(StoreError::ScheduleRevisionConflict);
            }
            Ok(next)
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
}

use rusqlite::OptionalExtension;

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
