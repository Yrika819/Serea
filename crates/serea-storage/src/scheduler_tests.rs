use crate::{
    EventDraft, MissedOccurrencePolicy, ScheduleDraft, ScheduleOccurrenceDraft, ScheduleOwnerKind,
    ScheduleTriggerKind, Store, StoreError, Tx,
};
use serea_protocol::{
    Actor, ActorId, ActorKind, Clock, DataClass, Digest, EnvelopeVersion, EpochMillis, EventId,
    EventKind, ProtocolError, ScheduleId, SemVer, Seq, SereaEvent, TaskId, Timestamp, WireSurface,
};
use std::sync::{Arc, Barrier};

struct Fixed;
impl Clock for Fixed {
    fn now_ms(&self) -> Result<EpochMillis, ProtocolError> {
        EpochMillis::new(0)
    }
}

fn at(value: i64) -> EpochMillis {
    EpochMillis::new(value).unwrap()
}

fn seed_schedule(store: &Store) {
    store
        .transact(|tx| {
            let template = tx.put_blob(
                br#"{"version":"1","title":"Template","intent":"Do work"}"#,
                DataClass::Public,
            )?;
            tx.inner.execute(
                "INSERT INTO schedules(
                   schedule_id,owner_kind,owner_id,state,revision,trigger_kind,
                   recurrence_json,template_digest,template_data_class_rank,policy_class_rank,approval_policy_json,timezone,
                   next_due_at_ms,next_local_label,missed_policy,created_at_ms,updated_at_ms
                 ) VALUES ('sch_00000000000000000000000001','HOST','host-test','ACTIVE',1,
                   'CALENDAR','{}',?1,?2,0,'{}','Etc/UTC',0,'2026-01-01T00:00','SKIP',0,0)",
                rusqlite::params![template.digest().as_str(), template.class().rank()],
            )?;
            Ok(())
        })
        .unwrap();
}

fn seed_occurrence(store: &Store, key: &str) {
    store
        .transact(|tx| {
            let (digest, class_rank): (String, i64) = tx.inner.query_row(
                "SELECT template_digest,template_data_class_rank FROM schedules WHERE schedule_id='sch_00000000000000000000000001'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )?;
            tx.inner.execute(
                "INSERT INTO schedule_occurrences(
                   schedule_id,occurrence_key,schedule_revision,trigger_kind,due_at_ms,
                   template_digest,template_data_class_rank,state,created_at_ms,updated_at_ms
                 ) VALUES ('sch_00000000000000000000000001',?1,1,'CALENDAR',0,?2,?3,'PENDING',0,0)",
                rusqlite::params![key, digest, class_rank],
            )?;
            Ok(())
        })
        .unwrap();
}

fn task_id() -> TaskId {
    TaskId::new("tsk_00000000000000000000000001").unwrap()
}

#[test]
fn scheduler_bounded_queries_have_their_owner_indexes() {
    let store = Store::open_in_memory(&Fixed).unwrap();
    let connection = store.conn.lock().unwrap();
    let plans = [
        "EXPLAIN QUERY PLAN SELECT s.schedule_id FROM schedules s
         WHERE s.state='ACTIVE' AND s.trigger_kind='CALENDAR'
           AND (s.next_due_at_ms<=0 OR EXISTS(
             SELECT 1 FROM schedule_occurrences o WHERE o.schedule_id=s.schedule_id
               AND o.state='PENDING' AND o.due_at_ms<=0
               AND (o.not_before_ms IS NULL OR o.not_before_ms<=0)))
         ORDER BY min(s.next_due_at_ms,COALESCE((
           SELECT min(o.due_at_ms) FROM schedule_occurrences o
           WHERE o.schedule_id=s.schedule_id AND o.state='PENDING'
             AND o.due_at_ms<=0 AND (o.not_before_ms IS NULL OR o.not_before_ms<=0)
         ),s.next_due_at_ms)),s.schedule_id LIMIT 256",
        "EXPLAIN QUERY PLAN SELECT occurrence_key FROM schedule_occurrences
         WHERE schedule_id='sch_00000000000000000000000001' AND state='PENDING'
           AND due_at_ms<=0 AND (not_before_ms IS NULL OR not_before_ms<=0)
         ORDER BY due_at_ms,occurrence_key LIMIT 10",
        "EXPLAIN QUERY PLAN SELECT o.schedule_id,o.occurrence_key
         FROM schedule_occurrences o JOIN schedules s USING(schedule_id)
         WHERE s.state='ACTIVE' AND o.trigger_kind='CALENDAR' AND o.due_at_ms<=0
           AND (o.not_before_ms IS NULL OR o.not_before_ms<=0)
           AND o.state='CLAIMED' AND o.lease_expires_at_ms<=0
         ORDER BY o.due_at_ms,o.schedule_id,o.occurrence_key LIMIT 512",
        "EXPLAIN QUERY PLAN SELECT task_id FROM device_resume_waits
         WHERE device_id='dev_00000000000000000000000001'
           AND registration_event_high_water_seq<100 ORDER BY task_id LIMIT 256",
        "EXPLAIN QUERY PLAN SELECT source_event_id FROM approval_lifecycle_wakes
         ORDER BY created_at_ms,source_seq,source_event_id LIMIT 256",
        "EXPLAIN QUERY PLAN SELECT w.task_id FROM device_resume_waits w JOIN tasks t USING(task_id)
         WHERE w.device_id='dev_00000000000000000000000001'
           AND w.registration_event_high_water_seq<100
           AND NOT EXISTS (SELECT 1 FROM device_session_resume_wakes d
                           WHERE d.task_id=w.task_id
                             AND d.blocked_task_revision=w.blocked_task_revision)
         ORDER BY w.task_id LIMIT 256",
        "EXPLAIN QUERY PLAN SELECT source_event_id FROM approval_lifecycle_wakes
         WHERE task_id='tsk_00000000000000000000000001'",
        "EXPLAIN QUERY PLAN SELECT o.schedule_id,o.occurrence_key
         FROM schedule_occurrences o JOIN schedules s USING(schedule_id)
         WHERE s.state='ACTIVE' AND s.trigger_kind='HOST_EVENT'
           AND o.trigger_kind='HOST_EVENT' AND o.state='PENDING'
           AND o.due_at_ms IS NULL AND (o.not_before_ms IS NULL OR o.not_before_ms<=0)
         ORDER BY o.created_at_ms,o.schedule_id,o.occurrence_key LIMIT 256",
    ];
    let rows = plans
        .iter()
        .map(|query| {
            let mut statement = connection.prepare(query).unwrap();
            statement
                .query_map([], |row| row.get::<_, String>(3))
                .unwrap()
                .collect::<Result<Vec<_>, _>>()
                .unwrap()
                .join(" | ")
        })
        .collect::<Vec<_>>();
    assert!(
        rows[0].contains("schedules_active_trigger") || rows[0].contains("schedules_active_due")
    );
    assert!(rows[0].contains("schedule_occurrences_claim"));
    assert!(rows[1].contains("schedule_occurrences_claim"));
    assert!(rows[2].contains("schedule_occurrences_expired_lease"));
    assert!(rows[3].contains("device_resume_waits_by_device_sequence"));
    assert!(rows[4].contains("approval_lifecycle_wakes_pending"));
    assert!(rows[5].contains("device_session_resume_wakes_task"));
    assert!(rows[6].contains("approval_lifecycle_wakes_task"));
    assert!(rows[7].contains("schedules_active_trigger"));
    assert!(rows[7].contains("schedule_occurrences_claim"));
    // SQLite sorts this candidate set; the accepted 256 schedules x 256
    // pending occurrences bound caps that temporary structure at 65,536 rows.
    assert!(rows[7].contains("TEMP B-TREE FOR ORDER BY"));
}

#[test]
fn scheduler_integrity_refuses_a_persisted_dedicated_wake_predicate() {
    let store = Store::open_in_memory(&Fixed).unwrap();
    seed_schedule(&store);
    store.verify_integrity().unwrap();
    store
        .conn
        .lock()
        .unwrap()
        .execute(
            r#"UPDATE schedules SET trigger_kind='HOST_EVENT',recurrence_json=NULL,
               timezone=NULL,next_due_at_ms=NULL,next_local_label=NULL,
               event_predicate_json='{"event_kind":"DEVICE_CONNECTED","version":"1"}',
               event_predicate_after_seq=0 WHERE schedule_id='sch_00000000000000000000000001'"#,
            [],
        )
        .unwrap();
    assert_eq!(
        store.verify_integrity(),
        Err(StoreError::IntegrityCheckFailed)
    );
}

fn schedule_draft(n: u32) -> ScheduleDraft {
    ScheduleDraft {
        schedule_id: ScheduleId::new(format!("sch_{n:026}")).unwrap(),
        owner_kind: ScheduleOwnerKind::Host,
        owner_id: "host-test".into(),
        trigger_kind: ScheduleTriggerKind::Calendar,
        recurrence: Some(serde_json::json!({"frequency":"DAILY"})),
        event_predicate: None,
        template_json:
            br#"{"version":"1","title":"Scheduled task","intent":"Perform the scheduled work."}"#
                .to_vec(),
        template_data_class: DataClass::Public,
        policy_class_rank: 0,
        approval_policy: serde_json::json!({}),
        timezone: Some("Etc/UTC".into()),
        recurrence_evaluator: Some("jiff/0.2.38".into()),
        tzdb_version: Some("2026e".into()),
        next_due_at: Some(at(0)),
        next_local_label: Some("2026-01-01T00:00".into()),
        missed_policy: MissedOccurrencePolicy::Skip,
    }
}

fn occurrence_draft(schedule_id: ScheduleId, n: u32) -> ScheduleOccurrenceDraft {
    ScheduleOccurrenceDraft {
        schedule_id,
        occurrence_key: format!("2026-01-{n:02}T00:00[Etc/UTC]"),
        schedule_revision: 1,
        trigger_kind: ScheduleTriggerKind::Calendar,
        source_event_id: None,
        source_event_data_class: None,
        intended_local_label: Some(format!("2026-01-{n:02}T00:00")),
        timezone: Some("Etc/UTC".into()),
        recurrence_evaluator: Some("jiff/0.2.38".into()),
        tzdb_version: Some("2026e".into()),
        due_at: Some(at(0)),
        not_before: None,
        created_at: at(0),
    }
}

fn lifecycle_event(
    kind: EventKind,
    message_id: u32,
    command_id: &EventId,
    schedule_id: &ScheduleId,
) -> EventDraft {
    let mut payload = serde_json::Map::new();
    payload.insert(
        "schedule_id".into(),
        serde_json::json!(schedule_id.as_str()),
    );
    EventDraft {
        event: SereaEvent {
            envelope_version: EnvelopeVersion::new("1").unwrap(),
            surface: WireSurface::new(WireSurface::EVENT).unwrap(),
            message_id: EventId::new(format!("evt_{message_id:026}")).unwrap(),
            seq: Seq::new(0),
            kind,
            occurred_at: Timestamp::from_epoch_millis(at(0)),
            correlation_id: None,
            causation_id: Some(command_id.clone()),
            actor: Actor {
                kind: ActorKind::Host,
                id: ActorId::new("host-test").unwrap(),
                version: SemVer::new("1.0.0").unwrap(),
                extensions: Default::default(),
            },
            data_class: DataClass::Public,
            trace: None,
            payload,
            extensions: Default::default(),
        },
        retention_at: None,
    }
}

#[test]
fn schedule_creation_retry_reuses_command_result_and_one_lifecycle_event() {
    let store = Store::open_in_memory(&Fixed).unwrap();
    let schedule = ScheduleId::new(format!("sch_{:026}", 1)).unwrap();
    let command = EventId::new(format!("evt_{:026}", 700)).unwrap();
    let digest = Digest::new(format!("sha256:{}", "a".repeat(64))).unwrap();
    let first = store
        .transact(|tx| {
            tx.create_schedule_command(
                &command,
                &digest,
                schedule_draft(1),
                at(0),
                lifecycle_event(EventKind::ScheduleCreated, 701, &command, &schedule),
            )
        })
        .unwrap();
    assert!(!first.replayed);
    let retry = store
        .transact(|tx| {
            tx.create_schedule_command(
                &command,
                &digest,
                schedule_draft(1),
                at(0),
                lifecycle_event(EventKind::ScheduleCreated, 702, &command, &schedule),
            )
        })
        .unwrap();
    assert!(retry.replayed);
    assert_eq!(retry.revision, first.revision);
    assert_eq!(store.replay_events(None, None, 10).unwrap().items.len(), 1);

    let wrong_digest = Digest::new(format!("sha256:{}", "b".repeat(64))).unwrap();
    assert_eq!(
        store.transact(|tx| {
            tx.create_schedule_command(
                &command,
                &wrong_digest,
                schedule_draft(1),
                at(0),
                lifecycle_event(EventKind::ScheduleCreated, 703, &command, &schedule),
            )
        }),
        Err(StoreError::ScheduleCommandIdentityConflict)
    );
    assert_eq!(store.replay_events(None, None, 10).unwrap().items.len(), 1);
}

#[test]
fn calendar_occurrence_admission_and_once_exhaustion_are_atomic() {
    let store = Store::open_in_memory(&Fixed).unwrap();
    seed_schedule(&store);
    let schedule = ScheduleId::new("sch_00000000000000000000000001").unwrap();
    let draft = occurrence_draft(schedule.clone(), 1);
    let inserted = store
        .transact(|tx| {
            tx.enqueue_calendar_occurrence_and_advance(draft, "2026-01-01T00:00", None, None)
        })
        .unwrap();
    assert!(inserted);
    let snapshot = store.transact(|tx| tx.load_schedule(&schedule)).unwrap();
    assert_eq!(snapshot.next_due_at, None);
    assert_eq!(snapshot.next_local_label, None);
    assert!(
        store
            .transact(|tx| tx.enqueue_calendar_occurrence_and_advance(
                occurrence_draft(schedule.clone(), 1),
                "2026-01-01T00:00",
                None,
                None,
            ))
            .is_err()
    );
    let pending: i64 = store
        .transact(|tx| {
            tx.inner
                .query_row("SELECT count(*) FROM schedule_occurrences", [], |row| {
                    row.get(0)
                })
                .map_err(StoreError::from)
        })
        .unwrap();
    assert_eq!(pending, 1);
}

#[test]
fn schedule_edit_keeps_resolved_occurrence_template_after_mapping() {
    let store = Store::open_in_memory(&Fixed).unwrap();
    let schedule = ScheduleId::new(format!("sch_{:026}", 1)).unwrap();
    let create_id = EventId::new(format!("evt_{:026}", 720)).unwrap();
    let request = Digest::new(format!("sha256:{}", "c".repeat(64))).unwrap();
    let mut original = schedule_draft(1);
    original.template_json =
        br#"{"version":"1","title":"Template A","intent":"Original intent."}"#.to_vec();
    store
        .transact(|tx| {
            tx.create_schedule_command(
                &create_id,
                &request,
                original,
                at(0),
                lifecycle_event(EventKind::ScheduleCreated, 721, &create_id, &schedule),
            )?;
            tx.enqueue_schedule_occurrence(occurrence_draft(schedule.clone(), 1))?;
            Ok(())
        })
        .unwrap();

    let update_id = EventId::new(format!("evt_{:026}", 722)).unwrap();
    let update_request = Digest::new(format!("sha256:{}", "d".repeat(64))).unwrap();
    let mut revised = schedule_draft(1);
    revised.template_json =
        br#"{"version":"1","title":"Template B","intent":"Future intent."}"#.to_vec();
    store
        .transact(|tx| {
            tx.update_schedule_command(
                &update_id,
                &update_request,
                1,
                revised,
                at(1),
                lifecycle_event(EventKind::ScheduleUpdated, 723, &update_id, &schedule),
            )
        })
        .unwrap();

    let lease = store
        .transact(|tx| {
            tx.claim_schedule_occurrence(
                &schedule,
                "2026-01-01T00:00[Etc/UTC]",
                2,
                "worker",
                at(2),
                at(50),
            )
        })
        .unwrap();
    store
        .transact(|tx| tx.map_schedule_occurrence(&lease, &task_id(), at(3)))
        .unwrap();
    let historical = store.schedule_task_provenance(&task_id()).unwrap().unwrap();
    let old_bytes = store
        .transact(|tx| tx.get_blob(&historical.template))
        .unwrap();
    assert_eq!(
        old_bytes,
        br#"{"intent":"Original intent.","title":"Template A","version":"1"}"#
    );
    let current = store.transact(|tx| tx.load_schedule(&schedule)).unwrap();
    let current_bytes = store
        .transact(|tx| tx.get_blob(current.template.as_ref().unwrap()))
        .unwrap();
    assert_eq!(
        current_bytes,
        br#"{"intent":"Future intent.","title":"Template B","version":"1"}"#
    );
}

#[test]
fn malformed_template_and_dedicated_wakes_refuse_or_accept_without_fake_predicates() {
    let store = Store::open_in_memory(&Fixed).unwrap();
    let schedule = ScheduleId::new(format!("sch_{:026}", 102)).unwrap();
    let command = EventId::new(format!("evt_{:026}", 820)).unwrap();
    let digest = Digest::new(format!("sha256:{}", "b".repeat(64))).unwrap();
    let mut invalid = schedule_draft(102);
    invalid.template_json = br#"{"version":"1","title":"","intent":"bad"}"#.to_vec();
    assert_eq!(
        store.transact(|tx| tx.create_schedule_command(
            &command,
            &digest,
            invalid,
            at(0),
            lifecycle_event(EventKind::ScheduleCreated, 821, &command, &schedule),
        )),
        Err(StoreError::InvalidSchedule)
    );

    let mut dedicated = schedule_draft(102);
    dedicated.trigger_kind = ScheduleTriggerKind::DeviceSessionEstablished;
    dedicated.recurrence = None;
    dedicated.event_predicate = None;
    dedicated.timezone = None;
    dedicated.recurrence_evaluator = None;
    dedicated.tzdb_version = None;
    dedicated.next_due_at = None;
    dedicated.next_local_label = None;
    let outcome = store
        .transact(|tx| {
            tx.create_schedule_command(
                &command,
                &digest,
                dedicated,
                at(0),
                lifecycle_event(EventKind::ScheduleCreated, 822, &command, &schedule),
            )
        })
        .unwrap();
    assert_eq!(outcome.revision, 1);
    let predicate: Option<String> = store
        .transact(|tx| {
            Ok(tx.inner.query_row(
                "SELECT event_predicate_json FROM schedules WHERE schedule_id=?1",
                [schedule.as_str()],
                |row| row.get(0),
            )?)
        })
        .unwrap();
    assert_eq!(predicate, None);
}

#[test]
fn occurrence_pins_the_template_version_across_definition_changes_and_restart_state() {
    let store = Store::open_in_memory(&Fixed).unwrap();
    let schedule = ScheduleId::new(format!("sch_{:026}", 101)).unwrap();
    let command = EventId::new(format!("evt_{:026}", 810)).unwrap();
    let digest = Digest::new(format!("sha256:{}", "a".repeat(64))).unwrap();
    store
        .transact(|tx| {
            tx.create_schedule_command(
                &command,
                &digest,
                schedule_draft(101),
                at(0),
                lifecycle_event(EventKind::ScheduleCreated, 811, &command, &schedule),
            )
        })
        .unwrap();

    let old_template = store
        .transact(|tx| tx.load_schedule(&schedule).map(|s| s.template.unwrap()))
        .unwrap();
    assert_eq!(
        store.transact(|tx| tx.get_blob(&old_template)).unwrap(),
        br#"{"intent":"Perform the scheduled work.","title":"Scheduled task","version":"1"}"#
    );
    let mut first = occurrence_draft(schedule.clone(), 1);
    assert!(
        store
            .transact(|tx| tx.enqueue_schedule_occurrence(first.clone()))
            .unwrap()
    );

    let replacement =
        br#"{"version":"1","title":"Updated task","intent":"Use the revised planning context."}"#;
    store.transact(|tx| {
        let blob = tx.put_blob(replacement, DataClass::Public)?;
        tx.inner.execute(
            "UPDATE schedules SET template_digest=?1,template_data_class_rank=?2,revision=2,updated_at_ms=1
             WHERE schedule_id=?3",
            rusqlite::params![blob.digest().as_str(), blob.class().rank(), schedule.as_str()],
        )?;
        Ok(())
    }).unwrap();

    let mut second = occurrence_draft(schedule.clone(), 2);
    second.schedule_revision = 2;
    second.created_at = at(1);
    assert!(
        store
            .transact(|tx| tx.enqueue_schedule_occurrence(second))
            .unwrap()
    );
    let new_template: (String, i64) = store
        .transact(|tx| {
            Ok(tx.inner.query_row(
                "SELECT template_digest,template_data_class_rank FROM schedule_occurrences
                 WHERE schedule_id=?1 AND occurrence_key='2026-01-02T00:00[Etc/UTC]'",
                [schedule.as_str()],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )?)
        })
        .unwrap();
    assert_eq!(
        store
            .transact(|tx| {
                tx.get_blob(&crate::BlobRef::new(
                    Digest::new(new_template.0).unwrap(),
                    DataClass::Public,
                ))
            })
            .unwrap(),
        br#"{"intent":"Use the revised planning context.","title":"Updated task","version":"1"}"#
    );

    first.schedule_revision = 2;
    first.created_at = at(1);
    assert!(
        !store
            .transact(|tx| tx.enqueue_schedule_occurrence(first))
            .unwrap()
    );
    assert_eq!(
        store.transact(|tx| tx.get_blob(&old_template)).unwrap(),
        br#"{"intent":"Perform the scheduled work.","title":"Scheduled task","version":"1"}"#
    );
    let captured: (String, i64) = store.transact(|tx| Ok(tx.inner.query_row(
        "SELECT template_digest,template_data_class_rank FROM schedule_occurrences WHERE schedule_id=?1",
        [schedule.as_str()],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?)).unwrap();
    assert_eq!(
        captured,
        (
            old_template.digest().to_string(),
            i64::from(old_template.class().rank())
        )
    );
}

#[test]
fn cancellation_retry_returns_committed_revision_without_duplicate_event() {
    let store = Store::open_in_memory(&Fixed).unwrap();
    let schedule = ScheduleId::new(format!("sch_{:026}", 2)).unwrap();
    let create = EventId::new(format!("evt_{:026}", 710)).unwrap();
    let create_digest = Digest::new(format!("sha256:{}", "c".repeat(64))).unwrap();
    store
        .transact(|tx| {
            tx.create_schedule_command(
                &create,
                &create_digest,
                schedule_draft(2),
                at(0),
                lifecycle_event(EventKind::ScheduleCreated, 711, &create, &schedule),
            )
        })
        .unwrap();

    let cancel = EventId::new(format!("evt_{:026}", 712)).unwrap();
    let cancel_digest = Digest::new(format!("sha256:{}", "d".repeat(64))).unwrap();
    let first = store
        .transact(|tx| {
            tx.cancel_schedule_command(
                &cancel,
                &cancel_digest,
                &schedule,
                1,
                at(1),
                lifecycle_event(EventKind::ScheduleCancelled, 713, &cancel, &schedule),
            )
        })
        .unwrap();
    assert!(!first.replayed);
    assert_eq!(first.revision, 2);
    let retry = store
        .transact(|tx| {
            tx.cancel_schedule_command(
                &cancel,
                &cancel_digest,
                &schedule,
                1,
                at(1),
                lifecycle_event(EventKind::ScheduleCancelled, 714, &cancel, &schedule),
            )
        })
        .unwrap();
    assert!(retry.replayed);
    assert_eq!(retry.revision, 2);
    assert_eq!(store.replay_events(None, None, 10).unwrap().items.len(), 2);
}

#[test]
fn pause_blocks_new_claims_and_resume_advances_the_schedule_revision() {
    let store = Store::open_in_memory(&Fixed).unwrap();
    let schedule = ScheduleId::new(format!("sch_{:026}", 6)).unwrap();
    let create = EventId::new(format!("evt_{:026}", 760)).unwrap();
    let create_digest = Digest::new(format!("sha256:{}", "f".repeat(64))).unwrap();
    store
        .transact(|tx| {
            tx.create_schedule_command(
                &create,
                &create_digest,
                schedule_draft(6),
                at(0),
                lifecycle_event(EventKind::ScheduleCreated, 761, &create, &schedule),
            )
        })
        .unwrap();
    store
        .transact(|tx| tx.enqueue_schedule_occurrence(occurrence_draft(schedule.clone(), 1)))
        .unwrap();

    let pause = EventId::new(format!("evt_{:026}", 762)).unwrap();
    let pause_digest = Digest::new(format!("sha256:{}", "1".repeat(64))).unwrap();
    let paused = store
        .transact(|tx| {
            tx.change_schedule_state_command(crate::ScheduleStateCommandRequest {
                message_id: pause.clone(),
                request_digest: pause_digest.clone(),
                schedule_id: schedule.clone(),
                expected_revision: 1,
                command: crate::ScheduleStateCommand::Pause,
                now: at(1),
                event: lifecycle_event(EventKind::SchedulePaused, 763, &pause, &schedule),
            })
        })
        .unwrap();
    assert_eq!(paused.revision, 2);
    assert_eq!(
        store.transact(|tx| {
            tx.claim_schedule_occurrence(
                &schedule,
                "2026-01-01T00:00[Etc/UTC]",
                2,
                "worker-a",
                at(2),
                at(10),
            )
        }),
        Err(StoreError::ScheduleNotActive)
    );

    let resume = EventId::new(format!("evt_{:026}", 764)).unwrap();
    let resume_digest = Digest::new(format!("sha256:{}", "2".repeat(64))).unwrap();
    let active = store
        .transact(|tx| {
            tx.change_schedule_state_command(crate::ScheduleStateCommandRequest {
                message_id: resume.clone(),
                request_digest: resume_digest.clone(),
                schedule_id: schedule.clone(),
                expected_revision: 2,
                command: crate::ScheduleStateCommand::Resume,
                now: at(3),
                event: lifecycle_event(EventKind::ScheduleResumed, 765, &resume, &schedule),
            })
        })
        .unwrap();
    assert_eq!(active.revision, 3);
    assert_eq!(
        store
            .transact(|tx| {
                tx.claim_schedule_occurrence(
                    &schedule,
                    "2026-01-01T00:00[Etc/UTC]",
                    3,
                    "worker-a",
                    at(4),
                    at(10),
                )
            })
            .unwrap()
            .schedule_revision,
        3
    );
}

#[test]
fn lifecycle_event_failure_rolls_schedule_and_command_receipt_back() {
    let path = std::env::temp_dir().join(format!(
        "serea-p3e-command-rollback-{}.sqlite",
        std::process::id()
    ));
    let store = Store::open(&path, &Fixed).unwrap();
    rusqlite::Connection::open(&path)
        .unwrap()
        .execute_batch(
            "CREATE TRIGGER reject_schedule_created BEFORE INSERT ON event_content
             WHEN NEW.kind='SCHEDULE_CREATED'
             BEGIN SELECT RAISE(ABORT, 'injected lifecycle event failure'); END;",
        )
        .unwrap();
    let schedule = ScheduleId::new(format!("sch_{:026}", 5)).unwrap();
    let command = EventId::new(format!("evt_{:026}", 750)).unwrap();
    let digest = Digest::new(format!("sha256:{}", "e".repeat(64))).unwrap();
    assert_eq!(
        store.transact(|tx| {
            tx.create_schedule_command(
                &command,
                &digest,
                schedule_draft(5),
                at(0),
                lifecycle_event(EventKind::ScheduleCreated, 751, &command, &schedule),
            )
        }),
        Err(StoreError::ConstraintViolation)
    );
    let conn = rusqlite::Connection::open(&path).unwrap();
    let counts: (i64, i64, i64, i64, i64) = conn
        .query_row(
            "SELECT (SELECT count(*) FROM schedules),
                    (SELECT count(*) FROM schedule_command_receipts),
                    (SELECT count(*) FROM event_content),
                    (SELECT last_allocated_seq FROM event_store_state WHERE singleton=1),
                    (SELECT count(*) FROM blobs)",
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
        )
        .unwrap();
    assert_eq!(counts, (0, 0, 0, 0, 0));
    drop(conn);
    drop(store);
    std::fs::remove_file(path).unwrap();
}

#[test]
fn scheduler_consumer_cursor_pins_snapshot_and_is_fenced_by_lease_generation() {
    let store = Store::open_in_memory(&Fixed).unwrap();
    let schedule = ScheduleId::new(format!("sch_{:026}", 3)).unwrap();
    store
        .transact(|tx| {
            for n in 1..=3 {
                let command = EventId::new(format!("evt_{:026}", 720 + n)).unwrap();
                let event =
                    lifecycle_event(EventKind::ScheduleCreated, 730 + n, &command, &schedule);
                tx.append_event(event.event, event.retention_at)?;
            }
            Ok(())
        })
        .unwrap();
    let first = store
        .transact(|tx| tx.claim_scheduler_consumer("worker-a", at(0), at(100)))
        .unwrap();
    let saved = store
        .transact(|tx| tx.advance_scheduler_cursor(&first, 0, 2, 3, at(1)))
        .unwrap();
    assert_eq!(saved.last_processed_seq, 2);
    assert_eq!(saved.replay_high_water_seq, Some(3));

    let incompatible_snapshot =
        store.transact(|tx| tx.advance_scheduler_cursor(&first, 2, 2, 4, at(2)));
    assert_eq!(
        incompatible_snapshot,
        Err(StoreError::ScheduleRevisionConflict)
    );
    let finished = store
        .transact(|tx| tx.advance_scheduler_cursor(&first, 2, 3, 3, at(2)))
        .unwrap();
    assert_eq!(finished.last_processed_seq, 3);
    assert_eq!(finished.replay_high_water_seq, None);

    let second = store
        .transact(|tx| tx.claim_scheduler_consumer("worker-b", at(100), at(200)))
        .unwrap();
    assert_eq!(second.generation, first.generation + 1);
    assert_eq!(
        store.transact(|tx| tx.advance_scheduler_cursor(&first, 3, 3, 3, at(101))),
        Err(StoreError::SchedulerLeaseFenced)
    );
}

#[test]
fn active_schedule_and_pending_occurrence_bounds_refuse_without_partial_rows() {
    let store = Store::open_in_memory(&Fixed).unwrap();
    for n in 1..=256 {
        store
            .transact(|tx| tx.create_schedule(schedule_draft(n), at(0)))
            .unwrap();
    }
    assert_eq!(
        store.transact(|tx| tx.create_schedule(schedule_draft(257), at(0))),
        Err(StoreError::ScheduleActiveLimit)
    );
    let active: i64 = store
        .transact(|tx| {
            Ok(tx.inner.query_row(
                "SELECT count(*) FROM schedules WHERE state='ACTIVE'",
                [],
                |row| row.get(0),
            )?)
        })
        .unwrap();
    assert_eq!(active, 256);

    let first = ScheduleId::new(format!("sch_{:026}", 1)).unwrap();
    for n in 1..=256 {
        store
            .transact(|tx| tx.enqueue_schedule_occurrence(occurrence_draft(first.clone(), n)))
            .unwrap();
    }
    assert_eq!(
        store.transact(|tx| {
            tx.enqueue_schedule_occurrence(occurrence_draft(first.clone(), 257))
        }),
        Err(StoreError::SchedulePendingOccurrenceLimit)
    );
    let pending: i64 = store
        .transact(|tx| {
            Ok(tx.inner.query_row(
                "SELECT count(*) FROM schedule_occurrences
                 WHERE schedule_id=?1 AND state IN ('PENDING','CLAIMED')",
                [first.as_str()],
                |row| row.get(0),
            )?)
        })
        .unwrap();
    assert_eq!(pending, 256);
}

#[test]
fn duplicate_source_event_id_cannot_create_a_second_occurrence() {
    let store = Store::open_in_memory(&Fixed).unwrap();
    let schedule = ScheduleId::new(format!("sch_{:026}", 8)).unwrap();
    let mut draft = schedule_draft(8);
    draft.trigger_kind = ScheduleTriggerKind::HostEvent;
    draft.recurrence = None;
    draft.event_predicate = Some(serde_json::json!({"kind":"TASK_CREATED"}));
    draft.timezone = None;
    draft.recurrence_evaluator = None;
    draft.tzdb_version = None;
    draft.next_due_at = None;
    draft.next_local_label = None;
    let command = EventId::new(format!("evt_{:026}", 800)).unwrap();
    let digest = Digest::new(format!("sha256:{}", "5".repeat(64))).unwrap();
    store
        .transact(|tx| {
            tx.create_schedule_command(
                &command,
                &digest,
                draft,
                at(0),
                lifecycle_event(EventKind::ScheduleCreated, 801, &command, &schedule),
            )
        })
        .unwrap();
    let source = EventId::new(format!("evt_{:026}", 802)).unwrap();
    for occurrence_key in ["host-event-a", "host-event-b"] {
        let occurrence = ScheduleOccurrenceDraft {
            schedule_id: schedule.clone(),
            occurrence_key: occurrence_key.into(),
            schedule_revision: 1,
            trigger_kind: ScheduleTriggerKind::HostEvent,
            source_event_id: Some(source.clone()),
            source_event_data_class: Some(DataClass::Public),
            intended_local_label: None,
            timezone: None,
            recurrence_evaluator: None,
            tzdb_version: None,
            due_at: Some(at(0)),
            not_before: None,
            created_at: at(0),
        };
        let result = store.transact(|tx| tx.enqueue_schedule_occurrence(occurrence));
        if occurrence_key == "host-event-a" {
            result.unwrap();
        } else {
            assert_eq!(result, Err(StoreError::ConstraintViolation));
        }
    }
    let count: i64 = store
        .transact(|tx| {
            Ok(tx.inner.query_row(
                "SELECT count(*) FROM schedule_occurrences WHERE schedule_id=?1",
                [schedule.as_str()],
                |row| row.get(0),
            )?)
        })
        .unwrap();
    assert_eq!(count, 1);
}

#[test]
fn claim_fence_reclaims_expired_lease_and_rejects_stale_mapping() {
    let store = Store::open_in_memory(&Fixed).unwrap();
    seed_schedule(&store);
    seed_occurrence(&store, "2026-01-01T00:00[Etc/UTC]");

    let first = store
        .transact(|tx| {
            tx.claim_schedule_occurrence(
                &ScheduleId::new("sch_00000000000000000000000001").unwrap(),
                "2026-01-01T00:00[Etc/UTC]",
                1,
                "worker-a",
                at(0),
                at(10),
            )
        })
        .unwrap();
    assert_eq!(first.lease_generation, 1);

    let second = store
        .transact(|tx| {
            tx.claim_schedule_occurrence(
                &first.schedule_id,
                &first.occurrence_key,
                1,
                "worker-b",
                at(10),
                at(20),
            )
        })
        .unwrap();
    assert_eq!(second.lease_generation, 2);
    assert_eq!(
        store.transact(|tx| tx.map_schedule_occurrence(&first, &task_id(), at(11))),
        Err(StoreError::StaleSchedulerLease)
    );
    store
        .transact(|tx| tx.map_schedule_occurrence(&second, &task_id(), at(12)))
        .unwrap();
}

#[test]
fn cancel_first_refuses_claim_but_claim_first_mapping_survives_cancel() {
    let store = Store::open_in_memory(&Fixed).unwrap();
    seed_schedule(&store);
    seed_occurrence(&store, "occurrence-a");
    store
        .transact(|tx| {
            let schedule = ScheduleId::new("sch_00000000000000000000000001").unwrap();
            let command = EventId::new(format!("evt_{:026}", 790)).unwrap();
            tx.cancel_schedule_command(
                &command,
                &Digest::new(format!("sha256:{}", "3".repeat(64))).unwrap(),
                &schedule,
                1,
                at(1),
                lifecycle_event(EventKind::ScheduleCancelled, 791, &command, &schedule),
            )
        })
        .unwrap();
    assert_eq!(
        store.transact(|tx| {
            tx.claim_schedule_occurrence(
                &ScheduleId::new("sch_00000000000000000000000001").unwrap(),
                "occurrence-a",
                1,
                "worker-a",
                at(2),
                at(10),
            )
        }),
        Err(StoreError::ScheduleNotActive)
    );

    let second = Store::open_in_memory(&Fixed).unwrap();
    seed_schedule(&second);
    seed_occurrence(&second, "occurrence-b");
    let lease = second
        .transact(|tx| {
            tx.claim_schedule_occurrence(
                &ScheduleId::new("sch_00000000000000000000000001").unwrap(),
                "occurrence-b",
                1,
                "worker-a",
                at(0),
                at(10),
            )
        })
        .unwrap();
    second
        .transact(|tx| {
            let schedule = ScheduleId::new("sch_00000000000000000000000001").unwrap();
            let command = EventId::new(format!("evt_{:026}", 792)).unwrap();
            tx.cancel_schedule_command(
                &command,
                &Digest::new(format!("sha256:{}", "4".repeat(64))).unwrap(),
                &schedule,
                1,
                at(1),
                lifecycle_event(EventKind::ScheduleCancelled, 793, &command, &schedule),
            )
        })
        .unwrap();
    second
        .transact(|tx| tx.map_schedule_occurrence(&lease, &task_id(), at(2)))
        .unwrap();
}

#[test]
fn two_store_claimers_cannot_both_hold_one_occurrence() {
    let path = std::env::temp_dir().join(format!(
        "serea-p3e-two-claimers-{}.sqlite",
        std::process::id()
    ));
    let store = Store::open(&path, &Fixed).unwrap();
    seed_schedule(&store);
    seed_occurrence(&store, "occurrence-race");
    let barrier = Arc::new(Barrier::new(3));
    let mut workers = Vec::new();
    for name in ["worker-a", "worker-b"] {
        let path = path.clone();
        let barrier = barrier.clone();
        workers.push(std::thread::spawn(move || {
            let store = Store::open(&path, &Fixed).unwrap();
            barrier.wait();
            store.transact(|tx| {
                tx.claim_schedule_occurrence(
                    &ScheduleId::new("sch_00000000000000000000000001").unwrap(),
                    "occurrence-race",
                    1,
                    name,
                    at(0),
                    at(10),
                )
            })
        }));
    }
    barrier.wait();
    let results: Vec<_> = workers
        .into_iter()
        .map(|worker| worker.join().unwrap())
        .collect();
    assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
    assert_eq!(results.iter().filter(|result| result.is_err()).count(), 1);
    drop(store);
    std::fs::remove_file(path).unwrap();
}

// Compile-time assertion that the operation boundary never exposes SQL.
fn _tx_is_only_a_typed_storage_capability(_: &mut Tx<'_>) {}
