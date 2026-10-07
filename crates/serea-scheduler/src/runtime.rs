use serde_json::Value;
use serea_event_bus::{EventBus, ScheduleLifecycle};
use serea_protocol::{
    ActorId, ActorKind, ApprovalLifecyclePayloadError, ApprovalLifecyclePayloadV1, AttemptBudget,
    DataClass, DeviceConnectedPayloadV1, Digest, EpochMillis, EventId, EventKind, EventPredicateV1,
    Extensions, RiskClass, ScheduleId, SemVer, Seq, TaskKind, TaskOrigin, TaskOriginKind,
    TaskTitle, canonicalize,
};
use serea_storage::{
    ApprovalLifecycleOutcome, ApprovalLifecycleWake, MissedOccurrencePolicy,
    ScheduleCommandOutcome, ScheduleDraft, ScheduleDueOccurrence, ScheduleOccurrenceDraft,
    ScheduleOccurrenceLease, ScheduleOwnerKind, ScheduleSnapshot, ScheduleTriggerKind,
    SchedulerConsumerLease, Store, StoreError, TransitionContext,
};
use serea_task_engine::{EngineError, NewTask, RecoveryReport, TaskEngine, TaskRecord};

use crate::{
    CalendarRecurrenceV1, EventCausality, EventPredicateError, ResolvedCalendarOccurrence,
    ScheduledTaskTemplateV1, TemplateError, matches_host_event,
};

const MAX_DEVICE_WAKE_MATERIALIZATION_PAGE: u16 = 256;
const MAX_CALENDAR_SCAN_PER_WAKE: u16 = 256;
const MAX_SCHEDULER_CATCH_UP_PER_WAKE: u16 = 10;

fn approval_outcome(kind: EventKind) -> Option<ApprovalLifecycleOutcome> {
    match kind {
        EventKind::ApprovalGranted => Some(ApprovalLifecycleOutcome::Granted),
        EventKind::ApprovalDenied => Some(ApprovalLifecycleOutcome::Denied),
        EventKind::ApprovalExpired => Some(ApprovalLifecycleOutcome::Expired),
        _ => None,
    }
}

/// Host-authenticated schedule definition input. JSON is retained as original
/// text until the Scheduler applies duplicate-aware protocol validation.
#[derive(Debug, Clone)]
pub struct ScheduleDefinition {
    pub schedule_id: ScheduleId,
    pub owner_kind: ScheduleOwnerKind,
    pub owner_id: String,
    pub trigger_kind: ScheduleTriggerKind,
    pub recurrence_json: Option<String>,
    pub event_predicate_json: Option<String>,
    pub template_json: Vec<u8>,
    pub title_data_class: DataClass,
    pub intent_data_class: DataClass,
    pub policy_class_rank: u8,
    pub approval_policy: Value,
    pub timezone: Option<String>,
    pub missed_policy: MissedOccurrencePolicy,
}

/// Payload-free Scheduler boundary error.
#[derive(Debug)]
pub enum ScheduleError {
    Invalid,
    Predicate(EventPredicateError),
    ApprovalPayload(ApprovalLifecyclePayloadError),
    Template(TemplateError),
    Recurrence(crate::RecurrenceError),
    TaskEngine(EngineError),
    Storage(StoreError),
}

impl std::fmt::Display for ScheduleError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Invalid => "invalid Scheduler command",
            Self::Predicate(error) => return error.fmt(f),
            Self::ApprovalPayload(error) => return error.fmt(f),
            Self::Template(error) => return error.fmt(f),
            Self::Recurrence(error) => return error.fmt(f),
            Self::TaskEngine(error) => return error.fmt(f),
            Self::Storage(error) => return error.fmt(f),
        })
    }
}
impl std::error::Error for ScheduleError {}
impl From<StoreError> for ScheduleError {
    fn from(value: StoreError) -> Self {
        Self::Storage(value)
    }
}
impl From<EngineError> for ScheduleError {
    fn from(value: EngineError) -> Self {
        Self::TaskEngine(value)
    }
}

/// Durable Scheduler orchestration. `store` and TaskEngine may use independent
/// SQLite connections to the same database; all cross-component writes recheck
/// the persisted fences in TaskEngine's single transaction.
pub struct Scheduler {
    store: Store,
    tasks: TaskEngine,
    events: EventBus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SchedulerRecoveryReport {
    pub task_recovery: RecoveryReport,
    pub expired_claims_recovered: u16,
    pub event_sequences_replayed: u64,
    pub host_event_tasks_created_or_reused: u64,
    pub device_wakes_processed: u64,
    pub stale_device_waits_removed: u64,
    pub calendar_tasks_created_or_reused: u16,
}

impl Scheduler {
    pub fn new(store: Store, tasks: TaskEngine, events: EventBus) -> Self {
        Self {
            store,
            tasks,
            events,
        }
    }

    /// Performs the accepted P3 startup order with explicit time: validate the
    /// durable store/Event Bus, recover TaskEngine-owned state, reconcile
    /// expired Scheduler claims and device waits, replay the Scheduler cursor,
    /// process materialized device wakes, then admit bounded calendar work.
    /// Approval wakes are intentionally preserved for the future P6 consumer.
    pub fn recover(
        &mut self,
        now: EpochMillis,
        lease_owner: &str,
        page_limit: u16,
    ) -> Result<SchedulerRecoveryReport, ScheduleError> {
        if !(1..=MAX_CALENDAR_SCAN_PER_WAKE).contains(&page_limit) {
            return Err(ScheduleError::Invalid);
        }
        if self
            .store
            .schema_version()
            .map_err(ScheduleError::Storage)?
            != 2
        {
            return Err(ScheduleError::Invalid);
        }
        self.store
            .verify_integrity()
            .map_err(ScheduleError::Storage)?;

        let actor_id = ActorId::new("scheduler").map_err(|_| ScheduleError::Invalid)?;
        let actor_version = SemVer::new("1.0.0").map_err(|_| ScheduleError::Invalid)?;
        let context = TransitionContext {
            actor_kind: ActorKind::Scheduler,
            actor_id: &actor_id,
            actor_version: &actor_version,
            causation_id: None,
        };
        let task_recovery = self.tasks.recover(now, &context)?;

        let mut stale_device_waits_removed = 0_u64;
        loop {
            let removed = self.reconcile_device_resume_waits(512)?;
            stale_device_waits_removed += u64::from(removed);
            if removed < 512 {
                break;
            }
        }

        let mut expired_claims_recovered = 0_u16;
        loop {
            let expired = self
                .store
                .transact(|tx| tx.expired_claimed_schedule_occurrences(now, 512))?;
            if expired.is_empty() {
                break;
            }
            let mut made_progress = false;
            for occurrence in expired {
                let schedule = self.schedule(&occurrence.schedule_id)?;
                let expires_at = now
                    .get()
                    .checked_add(120_000)
                    .and_then(|millis| EpochMillis::new(millis).ok())
                    .ok_or(ScheduleError::Invalid)?;
                let lease = match self.store.transact(|tx| {
                    tx.claim_schedule_occurrence(
                        &occurrence.schedule_id,
                        &occurrence.occurrence_key,
                        schedule.revision,
                        lease_owner,
                        now,
                        expires_at,
                    )
                }) {
                    Ok(lease) => lease,
                    Err(StoreError::ScheduleOccurrenceNotClaimable)
                    | Err(StoreError::ScheduleNotActive)
                    | Err(StoreError::ScheduleRevisionConflict) => continue,
                    Err(error) => return Err(ScheduleError::Storage(error)),
                };
                self.dispatch_claimed_occurrence(&lease, now)?;
                expired_claims_recovered = expired_claims_recovered.saturating_add(1);
                made_progress = true;
            }
            if !made_progress {
                break;
            }
        }

        let lease_expiry = now
            .get()
            .checked_add(120_000)
            .and_then(|millis| EpochMillis::new(millis).ok())
            .ok_or(ScheduleError::Invalid)?;
        let consumer = self.claim_event_consumer(lease_owner, now, lease_expiry)?;
        let start_cursor = self
            .store
            .transact(|tx| tx.scheduler_cursor())?
            .last_processed_seq;
        loop {
            self.replay_host_events(&consumer, now, page_limit)?;
            let cursor = self.store.transact(|tx| tx.scheduler_cursor())?;
            if cursor.replay_high_water_seq.is_none() {
                break;
            }
        }
        let end_cursor = self
            .store
            .transact(|tx| tx.scheduler_cursor())?
            .last_processed_seq;

        let mut host_event_tasks_created_or_reused = 0_u64;
        loop {
            let attempted = self.process_host_event_occurrences(now, lease_owner, page_limit)?;
            host_event_tasks_created_or_reused =
                host_event_tasks_created_or_reused.saturating_add(u64::from(attempted));
            if attempted < page_limit {
                break;
            }
        }

        let mut device_wakes_processed = 0_u64;
        loop {
            let pending = self
                .store
                .transact(|tx| tx.pending_device_session_resume_wakes(256))?;
            if pending.is_empty() {
                break;
            }
            let processed = self.process_device_session_resume_wakes(now, 256)?;
            device_wakes_processed += u64::from(processed);
            if processed == 0 {
                // A stale wake is consumed even when it does not resume a
                // task. Query again to distinguish that from no progress.
                let remains = self
                    .store
                    .transact(|tx| tx.pending_device_session_resume_wakes(256))?;
                if remains.len() == pending.len() {
                    return Err(ScheduleError::Invalid);
                }
            }
        }

        let calendar_tasks_created_or_reused = self.process_calendar_due(now, lease_owner)?;
        Ok(SchedulerRecoveryReport {
            task_recovery,
            expired_claims_recovered,
            event_sequences_replayed: end_cursor.saturating_sub(start_cursor),
            host_event_tasks_created_or_reused,
            device_wakes_processed,
            stale_device_waits_removed,
            calendar_tasks_created_or_reused,
        })
    }

    /// Creates a schedule only after parsing all closed V1 values and resolving
    /// its first calendar label with the pinned bundled timezone database.
    pub fn create_schedule(
        &mut self,
        definition: ScheduleDefinition,
        command_id: &EventId,
        request_digest: &Digest,
        now: EpochMillis,
    ) -> Result<ScheduleCommandOutcome, ScheduleError> {
        let schedule_id = definition.schedule_id.clone();
        let draft = compile_definition(definition)?;
        let event = self.events.draft_schedule_lifecycle(
            ScheduleLifecycle::Created {
                schedule_id,
                revision: 1,
            },
            command_id,
            now,
        )?;
        self.store
            .transact(|tx| {
                tx.create_schedule_command(command_id, request_digest, draft, now, event)
            })
            .map_err(ScheduleError::Storage)
    }

    /// Updates only unresolved future definition state. Pending, claimed, and
    /// mapped occurrences keep the snapshot captured when they were resolved.
    pub fn update_schedule(
        &mut self,
        definition: ScheduleDefinition,
        command_id: &EventId,
        request_digest: &Digest,
        expected_revision: u32,
        now: EpochMillis,
    ) -> Result<ScheduleCommandOutcome, ScheduleError> {
        let schedule_id = definition.schedule_id.clone();
        let next_revision = expected_revision
            .checked_add(1)
            .ok_or(ScheduleError::Invalid)?;
        let draft = compile_definition(definition)?;
        let event = self.events.draft_schedule_lifecycle(
            ScheduleLifecycle::Updated {
                schedule_id,
                revision: next_revision,
            },
            command_id,
            now,
        )?;
        self.store
            .transact(|tx| {
                tx.update_schedule_command(
                    command_id,
                    request_digest,
                    expected_revision,
                    draft,
                    now,
                    event,
                )
            })
            .map_err(ScheduleError::Storage)
    }

    pub fn change_state(
        &mut self,
        schedule_id: &ScheduleId,
        command_id: &EventId,
        request_digest: &Digest,
        expected_revision: u32,
        command: serea_storage::ScheduleStateCommand,
        now: EpochMillis,
    ) -> Result<ScheduleCommandOutcome, ScheduleError> {
        let revision = expected_revision
            .checked_add(1)
            .ok_or(ScheduleError::Invalid)?;
        let lifecycle = match command {
            serea_storage::ScheduleStateCommand::Pause => ScheduleLifecycle::Paused {
                schedule_id: schedule_id.clone(),
                revision,
            },
            serea_storage::ScheduleStateCommand::Resume => ScheduleLifecycle::Resumed {
                schedule_id: schedule_id.clone(),
                revision,
            },
            serea_storage::ScheduleStateCommand::Cancel => ScheduleLifecycle::Cancelled {
                schedule_id: schedule_id.clone(),
                revision,
            },
        };
        let event = self
            .events
            .draft_schedule_lifecycle(lifecycle, command_id, now)?;
        self.store
            .transact(|tx| {
                tx.change_schedule_state_command(serea_storage::ScheduleStateCommandRequest {
                    message_id: command_id.clone(),
                    request_digest: request_digest.clone(),
                    schedule_id: schedule_id.clone(),
                    expected_revision,
                    command,
                    now,
                    event,
                })
            })
            .map_err(ScheduleError::Storage)
    }

    /// HOST_EVENT's positive rule is exact EventKind equality. The causal-root
    /// check is derived from durable occurrence mappings, never event payload.
    pub fn matches_host_event(
        &self,
        predicate: &EventPredicateV1,
        event: &serea_protocol::SereaEvent,
    ) -> Result<bool, ScheduleError> {
        let correlation_is_scheduler_root = event
            .correlation_id
            .as_ref()
            .map(|task_id| self.store.schedule_task_provenance(task_id))
            .transpose()?
            .flatten()
            .is_some();
        let trace_is_scheduler_root = event
            .trace
            .as_ref()
            .and_then(|trace| trace.task_id.as_ref())
            .map(|task_id| self.store.schedule_task_provenance(task_id))
            .transpose()?
            .flatten()
            .is_some();
        let causality = if correlation_is_scheduler_root || trace_is_scheduler_root {
            EventCausality::SchedulerOccurrence
        } else {
            EventCausality::Independent
        };
        Ok(matches_host_event(predicate, event, causality))
    }

    /// Claims the singleton internal Scheduler Event Bus consumer. The lease
    /// is explicit so a worker must retain and present its fencing generation.
    pub fn claim_event_consumer(
        &self,
        owner: &str,
        now: EpochMillis,
        expires_at: EpochMillis,
    ) -> Result<SchedulerConsumerLease, ScheduleError> {
        self.store
            .transact(|tx| tx.claim_scheduler_consumer(owner, now, expires_at))
            .map_err(ScheduleError::Storage)
    }

    /// Replays one committed Event Bus page and atomically commits all exact
    /// HOST_EVENT decisions with its Scheduler cursor advancement. Corruption
    /// aborts the complete page; declared expiry advances the cursor without
    /// manufacturing events or occurrences.
    pub fn replay_host_events(
        &self,
        lease: &SchedulerConsumerLease,
        now: EpochMillis,
        page_limit: u16,
    ) -> Result<u16, ScheduleError> {
        self.store
            .transact(|tx| {
                let cursor = tx.scheduler_cursor()?;
                let through = cursor.replay_high_water_seq.map(Seq::new);
                let page = tx.replay_events(
                    Some(Seq::new(cursor.last_processed_seq)),
                    through,
                    page_limit,
                )?;
                let high = page.snapshot_high_water_seq.get();
                let schedules = tx.active_host_event_schedules()?;
                let mut next_cursor = cursor.last_processed_seq;
                let mut inserted = 0_u16;
                let mut materialized = 0_u16;
                for item in page.items {
                    match item {
                        serea_storage::ReplayItem::Event { event } => {
                            if event.kind == EventKind::DeviceConnected {
                                let payload_json = serde_json::to_string(&event.payload)
                                    .map_err(|_| StoreError::CorruptRow)?;
                                if let Ok(payload) =
                                    DeviceConnectedPayloadV1::parse_json(&payload_json)
                                {
                                    let (page_count, complete) = tx
                                        .materialize_device_session_resume_wakes(
                                            &event.message_id,
                                            event.seq.get(),
                                            payload.device_id(),
                                            now,
                                            MAX_DEVICE_WAKE_MATERIALIZATION_PAGE,
                                        )?;
                                    materialized = materialized.saturating_add(page_count);
                                    if !complete {
                                        break;
                                    }
                                }
                            }
                            if let Some(outcome) = approval_outcome(event.kind) {
                                let payload_json = serde_json::to_string(&event.payload)
                                    .map_err(|_| StoreError::CorruptRow)?;
                                let payload = ApprovalLifecyclePayloadV1::parse_event(
                                    event.kind,
                                    &payload_json,
                                    event.correlation_id.as_ref(),
                                    event.trace.as_ref(),
                                )
                                .map_err(|_| StoreError::CorruptRow)?;
                                tx.materialize_approval_lifecycle_wake(
                                    &event.message_id,
                                    event.seq.get(),
                                    &payload,
                                    outcome,
                                    now,
                                )?;
                            }
                            let correlation_is_scheduler_root = event
                                .correlation_id
                                .as_ref()
                                .map(|task_id| tx.schedule_occurrence_for_task(task_id))
                                .transpose()?
                                .flatten()
                                .is_some();
                            let trace_is_scheduler_root = event
                                .trace
                                .as_ref()
                                .and_then(|trace| trace.task_id.as_ref())
                                .map(|task_id| tx.schedule_occurrence_for_task(task_id))
                                .transpose()?
                                .flatten()
                                .is_some();
                            let scheduler_rooted =
                                correlation_is_scheduler_root || trace_is_scheduler_root;
                            if !scheduler_rooted {
                                for schedule in &schedules {
                                    if schedule
                                        .event_predicate_after_seq
                                        .is_none_or(|after| event.seq.get() <= after)
                                    {
                                        continue;
                                    }
                                    let Some(raw_predicate) =
                                        schedule.event_predicate_json.as_deref()
                                    else {
                                        return Err(StoreError::CorruptRow);
                                    };
                                    let predicate = EventPredicateV1::parse_json(raw_predicate)
                                        .map_err(|_| StoreError::CorruptRow)?;
                                    if matches_host_event(
                                        &predicate,
                                        &event,
                                        EventCausality::Independent,
                                    ) {
                                        let draft = ScheduleOccurrenceDraft {
                                            schedule_id: schedule.schedule_id.clone(),
                                            occurrence_key: event.message_id.as_str().to_owned(),
                                            schedule_revision: schedule.revision,
                                            trigger_kind: ScheduleTriggerKind::HostEvent,
                                            source_event_id: Some(event.message_id.clone()),
                                            source_event_data_class: Some(event.data_class),
                                            intended_local_label: None,
                                            timezone: None,
                                            recurrence_evaluator: None,
                                            tzdb_version: None,
                                            due_at: None,
                                            not_before: None,
                                            created_at: now,
                                        };
                                        if tx.enqueue_schedule_occurrence(draft)? {
                                            inserted = inserted.saturating_add(1);
                                        }
                                    }
                                }
                            }
                            next_cursor = event.seq.get();
                        }
                        serea_storage::ReplayItem::ExpiredRange { last_seq, .. } => {
                            next_cursor = last_seq.get();
                        }
                        serea_storage::ReplayItem::HistoryExpiredPrefix {
                            new_replay_boundary,
                        } => {
                            next_cursor = new_replay_boundary.get();
                        }
                        serea_storage::ReplayItem::Corruption { .. } => {
                            return Err(StoreError::EventHistoryCorrupt);
                        }
                    }
                }
                tx.advance_scheduler_cursor(
                    lease,
                    cursor.last_processed_seq,
                    next_cursor,
                    high,
                    now,
                )?;
                Ok(inserted.saturating_add(materialized))
            })
            .map_err(ScheduleError::Storage)
    }

    pub fn schedule(&self, id: &ScheduleId) -> Result<ScheduleSnapshot, ScheduleError> {
        self.store
            .transact(|tx| tx.load_schedule(id))
            .map_err(ScheduleError::Storage)
    }

    /// Lists pending Approval lifecycle handoffs for future P6 consumers.
    /// Reading leaves every wake durable and unacknowledged.
    pub fn pending_approval_lifecycle_wakes(
        &self,
        limit: u16,
    ) -> Result<Vec<ApprovalLifecycleWake>, ScheduleError> {
        self.store
            .pending_approval_lifecycle_wakes(limit)
            .map_err(ScheduleError::Storage)
    }

    /// Reads one pending Approval lifecycle handoff without consuming it.
    pub fn approval_lifecycle_wake(
        &self,
        source_event_id: &EventId,
    ) -> Result<Option<ApprovalLifecycleWake>, ScheduleError> {
        self.store
            .approval_lifecycle_wake(source_event_id)
            .map_err(ScheduleError::Storage)
    }

    /// Explicitly acknowledges a wake after the future P6 consumer has durably
    /// applied its own result. Repeating acknowledgement is harmless.
    pub fn acknowledge_approval_lifecycle_wake(
        &self,
        source_event_id: &EventId,
    ) -> Result<bool, ScheduleError> {
        self.store
            .acknowledge_approval_lifecycle_wake(source_event_id)
            .map_err(ScheduleError::Storage)
    }

    /// Processes one bounded page of durable device-session resume wakes.
    /// Each task transition is independently fenced and committed by TaskEngine.
    pub fn process_device_session_resume_wakes(
        &mut self,
        now: EpochMillis,
        limit: u16,
    ) -> Result<u16, ScheduleError> {
        let wakes = self
            .store
            .transact(|tx| tx.pending_device_session_resume_wakes(limit))?;
        let actor_id = ActorId::new("scheduler").map_err(|_| ScheduleError::Invalid)?;
        let actor_version = SemVer::new("1.0.0").map_err(|_| ScheduleError::Invalid)?;
        let mut resumed = 0_u16;
        for wake in &wakes {
            let context = TransitionContext {
                actor_kind: ActorKind::Scheduler,
                actor_id: &actor_id,
                actor_version: &actor_version,
                causation_id: Some(&wake.source_event_id),
            };
            if self
                .tasks
                .resume_device_session_wake(wake, now, &context)?
                .is_some()
            {
                resumed = resumed.saturating_add(1);
            }
        }
        Ok(resumed)
    }

    /// Bounded recovery cleanup for TaskEngine revisions that invalidated a
    /// device wait while Scheduler was offline.
    pub fn reconcile_device_resume_waits(&self, limit: u16) -> Result<u16, ScheduleError> {
        self.store
            .transact(|tx| tx.reconcile_device_resume_waits(limit))
            .map_err(ScheduleError::Storage)
    }

    /// Dispatches one bounded page of already-materialized HOST_EVENT
    /// occurrences. The source Event and occurrence are durable before task
    /// creation, so interruption is recovered through the occurrence fence.
    pub fn process_host_event_occurrences(
        &mut self,
        now: EpochMillis,
        lease_owner: &str,
        limit: u16,
    ) -> Result<u16, ScheduleError> {
        let pending = self
            .store
            .transact(|tx| tx.pending_host_event_occurrences(now, limit))?;
        let attempted = u16::try_from(pending.len()).map_err(|_| ScheduleError::Invalid)?;
        for occurrence in pending {
            let schedule = match self.schedule(&occurrence.schedule_id) {
                Ok(schedule) => schedule,
                Err(ScheduleError::Storage(StoreError::ScheduleNotActive)) => continue,
                Err(error) => return Err(error),
            };
            let expires_at = now
                .get()
                .checked_add(120_000)
                .and_then(|millis| EpochMillis::new(millis).ok())
                .ok_or(ScheduleError::Invalid)?;
            let claim = self.store.transact(|tx| {
                tx.claim_schedule_occurrence(
                    &occurrence.schedule_id,
                    &occurrence.occurrence_key,
                    schedule.revision,
                    lease_owner,
                    now,
                    expires_at,
                )
            });
            let lease = match claim {
                Ok(lease) => lease,
                Err(StoreError::ScheduleOccurrenceNotClaimable)
                | Err(StoreError::ScheduleNotActive)
                | Err(StoreError::ScheduleRevisionConflict) => continue,
                Err(error) => return Err(ScheduleError::Storage(error)),
            };
            self.dispatch_claimed_occurrence(&lease, now)?;
        }
        Ok(attempted)
    }

    /// Admits one next due local-calendar occurrence for each due active
    /// schedule. Recurrence expansion is deliberately one identity per
    /// schedule per pass; repeated bounded passes catch up without an
    /// unbounded loop. The occurrence and next cursor are committed together.
    pub fn enqueue_due_calendar_occurrences(&self, now: EpochMillis) -> Result<u16, ScheduleError> {
        let due = self
            .store
            .transact(|tx| tx.due_calendar_schedules(now, MAX_CALENDAR_SCAN_PER_WAKE))?;
        let mut admitted = 0_u16;
        for schedule in due {
            if self.admit_next_due_calendar_occurrence(&schedule.schedule_id, now)? {
                admitted = admitted.saturating_add(1);
            }
        }
        Ok(admitted)
    }

    /// Processes one bounded calendar wake. SKIP accounts each due identity,
    /// RUN_ONCE retains only the latest candidate until recurrence expansion is
    /// complete, and RUN_EACH creates at most ten tasks before persisting a
    /// one-clock-tick RETRY_DUE continuation.
    pub fn process_calendar_due(
        &mut self,
        now: EpochMillis,
        lease_owner: &str,
    ) -> Result<u16, ScheduleError> {
        let schedules = self
            .store
            .transact(|tx| tx.due_calendar_schedules(now, MAX_CALENDAR_SCAN_PER_WAKE))?;
        let mut tasks_created_or_reused = 0_u16;
        for initial in schedules {
            if self
                .store
                .transact(|tx| tx.schedule_has_future_retry(&initial.schedule_id, now))?
            {
                continue;
            }
            match initial.missed_policy {
                MissedOccurrencePolicy::Skip => {
                    for _ in 0..MAX_CALENDAR_SCAN_PER_WAKE {
                        if let Some(due) = self.first_due_occurrence(&initial.schedule_id, now)? {
                            self.skip_calendar_occurrence(&initial.schedule_id, due, now)?;
                            continue;
                        }
                        if !self.admit_next_due_calendar_occurrence(&initial.schedule_id, now)? {
                            break;
                        }
                    }
                }
                MissedOccurrencePolicy::RunEach => {
                    let mut processed = 0_u16;
                    while processed < MAX_SCHEDULER_CATCH_UP_PER_WAKE {
                        let due = match self.first_due_occurrence(&initial.schedule_id, now)? {
                            Some(due) => due,
                            None if self.admit_next_due_calendar_occurrence(
                                &initial.schedule_id,
                                now,
                            )? =>
                            {
                                match self.first_due_occurrence(&initial.schedule_id, now)? {
                                    Some(due) => due,
                                    None => break,
                                }
                            }
                            None => break,
                        };
                        self.dispatch_due_occurrence(&initial.schedule_id, due, now, lease_owner)?;
                        processed += 1;
                        tasks_created_or_reused += 1;
                    }
                    if processed == MAX_SCHEDULER_CATCH_UP_PER_WAKE {
                        if self
                            .first_due_occurrence(&initial.schedule_id, now)?
                            .is_none()
                        {
                            self.admit_next_due_calendar_occurrence(&initial.schedule_id, now)?;
                        }
                        self.defer_calendar_catch_up(&initial.schedule_id, now)?;
                    }
                }
                MissedOccurrencePolicy::RunOnce => {
                    let mut expanded = 0_u16;
                    while expanded < MAX_CALENDAR_SCAN_PER_WAKE
                        && self.admit_next_due_calendar_occurrence(&initial.schedule_id, now)?
                    {
                        expanded += 1;
                    }
                    let snapshot = self.schedule(&initial.schedule_id)?;
                    if snapshot.next_due_at.is_some_and(|next| next <= now) {
                        self.defer_calendar_catch_up(&initial.schedule_id, now)?;
                        continue;
                    }
                    let mut due = self.store.transact(|tx| {
                        tx.due_schedule_occurrences(&initial.schedule_id, now, 256)
                    })?;
                    if due.len() > 1 {
                        let latest = due.pop().ok_or(ScheduleError::Invalid)?;
                        for older in due {
                            self.skip_calendar_occurrence(&initial.schedule_id, older, now)?;
                        }
                        due = vec![latest];
                    }
                    if let Some(latest) = due.pop() {
                        self.dispatch_due_occurrence(
                            &initial.schedule_id,
                            latest,
                            now,
                            lease_owner,
                        )?;
                        tasks_created_or_reused += 1;
                    }
                }
            }
        }
        Ok(tasks_created_or_reused)
    }

    /// Exposes the single Task Engine authority to Scheduler wake processing;
    /// callers cannot obtain or mutate its Storage connection.
    pub fn task_engine(&mut self) -> &mut TaskEngine {
        &mut self.tasks
    }

    /// Creates one ordinary SCHEDULED Task from a durably claimed occurrence.
    /// Task, journal, TaskCreated, occurrence mapping, and ScheduleTaskCreated
    /// commit through TaskEngine's fixed single-transaction composition.
    pub fn dispatch_claimed_occurrence(
        &mut self,
        lease: &ScheduleOccurrenceLease,
        now: EpochMillis,
    ) -> Result<TaskRecord, ScheduleError> {
        if let Some(task_id) = self
            .store
            .transact(|tx| tx.schedule_occurrence_task(&lease.schedule_id, &lease.occurrence_key))?
        {
            let mapped = self
                .tasks
                .load(task_id)
                .map_err(ScheduleError::TaskEngine)?;
            if mapped.task.kind != TaskKind::Scheduled {
                return Err(ScheduleError::Storage(StoreError::CorruptRow));
            }
            return Ok(mapped);
        }
        let (work, bytes) = self.store.transact(|tx| {
            let work = tx.schedule_occurrence_work(lease, now)?;
            let bytes = tx.get_blob(&work.template)?;
            Ok((work, bytes))
        })?;
        let input = std::str::from_utf8(&bytes).map_err(|_| ScheduleError::Invalid)?;
        let template =
            ScheduledTaskTemplateV1::parse_json(input).map_err(ScheduleError::Template)?;
        let data_class = DataClass::compose(
            work.template.class(),
            work.source_event_data_class.unwrap_or(DataClass::Public),
        );
        if matches!(data_class, DataClass::Secret | DataClass::Credential) {
            return Err(ScheduleError::Invalid);
        }
        let title = TaskTitle::new(template.title().as_str().to_owned())
            .map_err(|_| ScheduleError::Invalid)?;
        let task_id = self.events.mint_task_id()?;
        let origin_kind = TaskOriginKind::new("SCHEDULED").map_err(|_| ScheduleError::Invalid)?;
        let policy_class = risk_class(work.policy_class_rank).ok_or(ScheduleError::Invalid)?;
        let spec = NewTask {
            task_id: task_id.clone(),
            kind: TaskKind::Scheduled,
            title,
            origin: TaskOrigin {
                kind: origin_kind,
                device_id: None,
                message_id: work.source_event_id.clone(),
                extensions: Extensions::default(),
            },
            data_class,
            policy_class,
            attempt_budget: AttemptBudget {
                max_model_calls: 12,
                max_tool_calls: 24,
                max_attempts_per_step: 3,
                extensions: Extensions::default(),
            },
            created_at: now,
            deadline_at: None,
            extensions: Extensions::default(),
        };
        let event = self.events.draft_schedule_task_created(
            &work.schedule_id,
            &work.occurrence_key,
            &task_id,
            work.source_event_id.as_ref(),
            now,
            data_class,
        )?;
        let actor_id = ActorId::new("scheduler").map_err(|_| ScheduleError::Invalid)?;
        let actor_version = SemVer::new("1.0.0").map_err(|_| ScheduleError::Invalid)?;
        let context = TransitionContext {
            actor_kind: ActorKind::Scheduler,
            actor_id: &actor_id,
            actor_version: &actor_version,
            causation_id: work.source_event_id.as_ref(),
        };
        self.tasks
            .create_scheduled_task(spec, lease, now, event, &context)
            .map_err(ScheduleError::TaskEngine)
    }

    pub fn occurrence_draft(
        snapshot: &ScheduleSnapshot,
        key: String,
        resolved: &ResolvedCalendarOccurrence,
        created_at: EpochMillis,
    ) -> Result<ScheduleOccurrenceDraft, ScheduleError> {
        if snapshot.trigger_kind != ScheduleTriggerKind::Calendar
            || snapshot.timezone.as_deref() != Some(resolved.timezone.as_str())
        {
            return Err(ScheduleError::Invalid);
        }
        Ok(ScheduleOccurrenceDraft {
            schedule_id: snapshot.schedule_id.clone(),
            occurrence_key: key,
            schedule_revision: snapshot.revision,
            trigger_kind: ScheduleTriggerKind::Calendar,
            source_event_id: None,
            source_event_data_class: None,
            intended_local_label: Some(resolved.intended_local_label.clone()),
            timezone: Some(resolved.timezone.clone()),
            recurrence_evaluator: Some(resolved.evaluator_version.to_owned()),
            tzdb_version: Some(resolved.tzdb_version.to_owned()),
            due_at: Some(resolved.due_at),
            not_before: None,
            created_at,
        })
    }

    fn admit_next_due_calendar_occurrence(
        &self,
        schedule_id: &ScheduleId,
        now: EpochMillis,
    ) -> Result<bool, ScheduleError> {
        let schedule = self.schedule(schedule_id)?;
        if schedule.state != serea_storage::ScheduleCommandState::Active
            || schedule.trigger_kind != ScheduleTriggerKind::Calendar
        {
            return Ok(false);
        }
        let (Some(label), Some(timezone), Some(due_at)) = (
            schedule.next_local_label.as_deref(),
            schedule.timezone.as_deref(),
            schedule.next_due_at,
        ) else {
            return Ok(false);
        };
        if due_at > now {
            return Ok(false);
        }
        let recurrence = CalendarRecurrenceV1::parse_json(
            schedule
                .recurrence_json
                .as_deref()
                .ok_or(ScheduleError::Invalid)?,
        )
        .map_err(ScheduleError::Recurrence)?;
        let resolved = CalendarRecurrenceV1::resolve_local_label(label, timezone)
            .map_err(ScheduleError::Recurrence)?;
        if resolved.due_at != due_at || resolved.due_at > now {
            return Err(ScheduleError::Invalid);
        }
        let next_label = recurrence
            .next_local_label(Some(label))
            .map_err(ScheduleError::Recurrence)?;
        let next_resolved = next_label
            .as_deref()
            .map(|next| CalendarRecurrenceV1::resolve_local_label(next, timezone))
            .transpose()
            .map_err(ScheduleError::Recurrence)?;
        let key =
            crate::occurrence_identity_key(label, timezone).map_err(ScheduleError::Recurrence)?;
        let draft = Self::occurrence_draft(&schedule, key.clone(), &resolved, now)?;
        if schedule.missed_policy == MissedOccurrencePolicy::RunOnce {
            let previous = self.store.transact(|tx| {
                tx.due_schedule_occurrences(schedule_id, now, MAX_CALENDAR_SCAN_PER_WAKE)
            })?;
            let expected_previous_key = previous
                .last()
                .map(|occurrence| occurrence.occurrence_key.as_str());
            let missed_event = expected_previous_key
                .filter(|previous| *previous != key)
                .map(|previous| {
                    self.events
                        .draft_schedule_occurrence_missed(schedule_id, previous, now)
                })
                .transpose()?;
            self.store.transact(|tx| {
                tx.enqueue_run_once_candidate_and_advance(
                    draft,
                    label,
                    next_resolved.as_ref().map(|value| value.due_at),
                    next_label.as_deref(),
                    expected_previous_key,
                    missed_event,
                )
            })
        } else {
            self.store.transact(|tx| {
                tx.enqueue_calendar_occurrence_and_advance(
                    draft,
                    label,
                    next_resolved.as_ref().map(|value| value.due_at),
                    next_label.as_deref(),
                )
            })
        }
        .map_err(ScheduleError::Storage)
    }

    fn first_due_occurrence(
        &self,
        schedule_id: &ScheduleId,
        now: EpochMillis,
    ) -> Result<Option<ScheduleDueOccurrence>, ScheduleError> {
        self.store
            .transact(|tx| tx.due_schedule_occurrences(schedule_id, now, 1))
            .map(|mut due| due.pop())
            .map_err(ScheduleError::Storage)
    }

    fn skip_calendar_occurrence(
        &self,
        schedule_id: &ScheduleId,
        occurrence: ScheduleDueOccurrence,
        now: EpochMillis,
    ) -> Result<bool, ScheduleError> {
        let schedule = self.schedule(schedule_id)?;
        let event = self.events.draft_schedule_occurrence_missed(
            schedule_id,
            &occurrence.occurrence_key,
            now,
        )?;
        self.store
            .transact(|tx| {
                tx.skip_schedule_occurrence_with_event(
                    schedule_id,
                    &occurrence.occurrence_key,
                    schedule.revision,
                    now,
                    event,
                )
            })
            .map_err(ScheduleError::Storage)
    }

    fn dispatch_due_occurrence(
        &mut self,
        schedule_id: &ScheduleId,
        occurrence: ScheduleDueOccurrence,
        now: EpochMillis,
        lease_owner: &str,
    ) -> Result<TaskRecord, ScheduleError> {
        let schedule = self.schedule(schedule_id)?;
        let expires_at = now
            .get()
            .checked_add(120_000)
            .and_then(|millis| EpochMillis::new(millis).ok())
            .ok_or(ScheduleError::Invalid)?;
        let lease = self.store.transact(|tx| {
            tx.claim_schedule_occurrence(
                schedule_id,
                &occurrence.occurrence_key,
                schedule.revision,
                lease_owner,
                now,
                expires_at,
            )
        })?;
        self.dispatch_claimed_occurrence(&lease, now)
    }

    fn defer_calendar_catch_up(
        &self,
        schedule_id: &ScheduleId,
        now: EpochMillis,
    ) -> Result<u16, ScheduleError> {
        let Some(first) = self.first_due_occurrence(schedule_id, now)? else {
            return Ok(0);
        };
        // EpochMillis has millisecond resolution. One tick is the smallest
        // deterministic future retry and prevents a same-clock busy loop.
        let not_before = now
            .get()
            .checked_add(1)
            .and_then(|millis| EpochMillis::new(millis).ok())
            .ok_or(ScheduleError::Invalid)?;
        let schedule = self.schedule(schedule_id)?;
        let event = self.events.draft_schedule_catch_up_deferred(
            schedule_id,
            &first.occurrence_key,
            now,
        )?;
        self.store
            .transact(|tx| {
                tx.defer_due_schedule_occurrences(
                    schedule_id,
                    schedule.revision,
                    &first.occurrence_key,
                    now,
                    not_before,
                    event,
                )
            })
            .map_err(ScheduleError::Storage)
    }
}

fn compile_definition(definition: ScheduleDefinition) -> Result<ScheduleDraft, ScheduleError> {
    let template_text =
        std::str::from_utf8(&definition.template_json).map_err(|_| ScheduleError::Invalid)?;
    let template =
        ScheduledTaskTemplateV1::parse_json(template_text).map_err(ScheduleError::Template)?;
    let template_class =
        DataClass::compose(definition.title_data_class, definition.intent_data_class);
    if matches!(template_class, DataClass::Secret | DataClass::Credential) {
        return Err(ScheduleError::Invalid);
    }

    let (recurrence, predicate, resolved) = match definition.trigger_kind {
        ScheduleTriggerKind::Calendar => {
            if definition.event_predicate_json.is_some() {
                return Err(ScheduleError::Invalid);
            }
            let source = definition
                .recurrence_json
                .as_deref()
                .ok_or(ScheduleError::Invalid)?;
            let recurrence =
                CalendarRecurrenceV1::parse_json(source).map_err(ScheduleError::Recurrence)?;
            let timezone = definition
                .timezone
                .as_deref()
                .ok_or(ScheduleError::Invalid)?;
            let label = recurrence
                .next_local_label(None)
                .map_err(ScheduleError::Recurrence)?
                .ok_or(ScheduleError::Invalid)?;
            let resolved = CalendarRecurrenceV1::resolve_local_label(&label, timezone)
                .map_err(ScheduleError::Recurrence)?;
            (
                Some(value_from_canonical(recurrence.canonical_json())?),
                None,
                Some(resolved),
            )
        }
        ScheduleTriggerKind::HostEvent => {
            if definition.recurrence_json.is_some() || definition.timezone.is_some() {
                return Err(ScheduleError::Invalid);
            }
            let source = definition
                .event_predicate_json
                .as_deref()
                .ok_or(ScheduleError::Invalid)?;
            let predicate =
                EventPredicateV1::parse_json(source).map_err(ScheduleError::Predicate)?;
            (
                None,
                Some(value_from_canonical(predicate.canonical_json())?),
                None,
            )
        }
        // Dedicated device and approval wakes operate on existing task waits
        // or create a P6 handoff. They are not schedule definitions and must
        // never create a fresh scheduled task from an event.
        ScheduleTriggerKind::DeviceSessionEstablished | ScheduleTriggerKind::ApprovalEvent => {
            return Err(ScheduleError::Invalid);
        }
    };

    let (timezone, evaluator, tzdb, next_due, next_label) = match resolved {
        Some(result) => (
            definition.timezone.clone(),
            Some(result.evaluator_version.to_owned()),
            Some(result.tzdb_version.to_owned()),
            Some(result.due_at),
            Some(result.intended_local_label),
        ),
        None => (None, None, None, None, None),
    };
    Ok(ScheduleDraft {
        schedule_id: definition.schedule_id,
        owner_kind: definition.owner_kind,
        owner_id: definition.owner_id,
        trigger_kind: definition.trigger_kind,
        recurrence,
        event_predicate: predicate,
        template_json: template.canonical_json().as_bytes().to_vec(),
        template_data_class: template_class,
        policy_class_rank: definition.policy_class_rank,
        approval_policy: definition.approval_policy,
        timezone,
        recurrence_evaluator: evaluator,
        tzdb_version: tzdb,
        next_due_at: next_due,
        next_local_label: next_label,
        missed_policy: definition.missed_policy,
    })
}

fn value_from_canonical(source: &str) -> Result<Value, ScheduleError> {
    let canonical = canonicalize(source).map_err(|_| ScheduleError::Invalid)?;
    serde_json::from_slice(&canonical).map_err(|_| ScheduleError::Invalid)
}

fn risk_class(rank: u8) -> Option<RiskClass> {
    Some(match rank {
        0 => RiskClass::Observe,
        1 => RiskClass::LocalState,
        2 => RiskClass::ReversibleWrite,
        3 => RiskClass::ExternalWrite,
        4 => RiskClass::Communication,
        5 => RiskClass::ElevatedDevice,
        6 => RiskClass::Destructive,
        7 => RiskClass::Credential,
        _ => return None,
    })
}
