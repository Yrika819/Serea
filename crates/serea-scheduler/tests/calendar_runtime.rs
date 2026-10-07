use serea_event_bus::EventBus;
use serea_protocol::*;
use serea_scheduler::{ScheduleDefinition, Scheduler, occurrence_identity_key};
use serea_storage::{MissedOccurrencePolicy, ScheduleOwnerKind, ScheduleTriggerKind, Store};
use serea_task_engine::TaskEngine;
use serea_testkit::DeterministicUlidSource;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

static NEXT_DB: AtomicUsize = AtomicUsize::new(0);
struct Fixed;
impl Clock for Fixed {
    fn now_ms(&self) -> Result<EpochMillis, ProtocolError> {
        EpochMillis::new(1_700_000_000_000)
    }
}
fn at(n: i64) -> EpochMillis {
    EpochMillis::new(n).unwrap()
}
fn path() -> PathBuf {
    std::env::temp_dir().join(format!(
        "serea-calendar-runtime-{}-{}.sqlite",
        std::process::id(),
        NEXT_DB.fetch_add(1, Ordering::Relaxed)
    ))
}

fn daily_schedule(
    scheduler: &mut Scheduler,
    schedule_id: &str,
    command_id: &str,
    policy: MissedOccurrencePolicy,
) -> ScheduleId {
    daily_schedule_at(
        scheduler,
        schedule_id,
        command_id,
        policy,
        "2020-01-01T00:00",
    )
}

fn daily_schedule_at(
    scheduler: &mut Scheduler,
    schedule_id: &str,
    command_id: &str,
    policy: MissedOccurrencePolicy,
    anchor: &str,
) -> ScheduleId {
    let schedule = ScheduleId::new(schedule_id).unwrap();
    scheduler
        .create_schedule(
            ScheduleDefinition {
                schedule_id: schedule.clone(),
                owner_kind: ScheduleOwnerKind::Host,
                owner_id: "host-test".into(),
                trigger_kind: ScheduleTriggerKind::Calendar,
                recurrence_json: Some(format!(
                    r#"{{"version":"1","kind":"DAILY","anchor_local":"{anchor}","interval":1}}"#
                )),
                event_predicate_json: None,
                template_json:
                    br#"{"version":"1","title":"Daily run","intent":"Perform scheduled work."}"#
                        .to_vec(),
                title_data_class: DataClass::Public,
                intent_data_class: DataClass::Public,
                policy_class_rank: 0,
                approval_policy: serde_json::json!({}),
                timezone: Some("UTC".into()),
                missed_policy: policy,
            },
            &EventId::new(command_id).unwrap(),
            &Digest::new(format!("sha256:{}", "b".repeat(64))).unwrap(),
            at(1),
        )
        .unwrap();
    schedule
}

#[test]
fn calendar_due_admission_is_atomic_and_once_exhausts_after_mapping() {
    let db = path();
    let bus = EventBus::new(DeterministicUlidSource::starting_at(1_700_300_000_000).unwrap());
    let store = Store::open(&db, &Fixed).unwrap();
    let mut scheduler = Scheduler::new(
        Store::open(&db, &Fixed).unwrap(),
        TaskEngine::new(Store::open(&db, &Fixed).unwrap(), bus.clone()),
        bus,
    );
    let schedule = ScheduleId::new("sch_00000000000000000000000031").unwrap();
    let recurrence = r#"{"version":"1","kind":"ONCE","anchor_local":"2020-01-02T03:04"}"#;
    scheduler
        .create_schedule(
            ScheduleDefinition {
                schedule_id: schedule.clone(),
                owner_kind: ScheduleOwnerKind::Host,
                owner_id: "host-test".into(),
                trigger_kind: ScheduleTriggerKind::Calendar,
                recurrence_json: Some(recurrence.into()),
                event_predicate_json: None,
                template_json:
                    br#"{"version":"1","title":"Run once","intent":"Perform the scheduled work."}"#
                        .to_vec(),
                title_data_class: DataClass::Public,
                intent_data_class: DataClass::Public,
                policy_class_rank: 0,
                approval_policy: serde_json::json!({}),
                timezone: Some("UTC".into()),
                missed_policy: MissedOccurrencePolicy::RunEach,
            },
            &EventId::new("evt_00000000000000000000000031").unwrap(),
            &Digest::new(format!("sha256:{}", "a".repeat(64))).unwrap(),
            at(1),
        )
        .unwrap();

    assert_eq!(
        scheduler
            .enqueue_due_calendar_occurrences(at(1_700_000_000_000))
            .unwrap(),
        1
    );
    assert_eq!(
        scheduler
            .enqueue_due_calendar_occurrences(at(1_700_000_000_000))
            .unwrap(),
        0
    );
    let occurrence_key = occurrence_identity_key("2020-01-02T03:04", "UTC").unwrap();
    let lease = store
        .transact(|tx| {
            tx.claim_schedule_occurrence(
                &schedule,
                &occurrence_key,
                1,
                "calendar-worker",
                at(1_700_000_000_000),
                at(1_700_000_000_100),
            )
        })
        .unwrap();
    let task = scheduler
        .dispatch_claimed_occurrence(&lease, at(1_700_000_000_000))
        .unwrap();
    assert_eq!(task.task.kind, TaskKind::Scheduled);
    assert_eq!(
        store
            .transact(|tx| tx.schedule_occurrence_task(&schedule, &occurrence_key))
            .unwrap(),
        Some(task.task.task_id)
    );
    assert_eq!(scheduler.schedule(&schedule).unwrap().next_due_at, None);
    assert_eq!(
        scheduler.schedule(&schedule).unwrap().next_local_label,
        None
    );
    drop(scheduler);
    drop(store);
    std::fs::remove_file(db).unwrap();
}

#[test]
fn run_each_is_bounded_to_ten_and_retry_processes_durable_remainder() {
    let db = path();
    let bus = EventBus::new(DeterministicUlidSource::starting_at(1_700_300_000_000).unwrap());
    let store = Store::open(&db, &Fixed).unwrap();
    let mut scheduler = Scheduler::new(
        Store::open(&db, &Fixed).unwrap(),
        TaskEngine::new(Store::open(&db, &Fixed).unwrap(), bus.clone()),
        bus,
    );
    let schedule = daily_schedule(
        &mut scheduler,
        "sch_00000000000000000000000041",
        "evt_00000000000000000000000041",
        MissedOccurrencePolicy::RunEach,
    );
    // 2020-01-11 00:00 UTC: eleven daily labels from the anchor are due.
    let now = at(1_578_700_800_000);
    assert_eq!(
        scheduler
            .process_calendar_due(now, "catch-up-worker")
            .unwrap(),
        10
    );
    let first_ten: Vec<_> = (1..=10)
        .map(|day| format!("2020-01-{day:02}T00:00"))
        .collect();
    for label in first_ten {
        let key = occurrence_identity_key(&label, "UTC").unwrap();
        assert!(
            store
                .transact(|tx| tx.schedule_occurrence_task(&schedule, &key))
                .unwrap()
                .is_some(),
            "{label} should have a durable Task mapping"
        );
    }
    let eleventh = occurrence_identity_key("2020-01-11T00:00", "UTC").unwrap();
    assert!(
        store
            .transact(|tx| tx.schedule_occurrence_task(&schedule, &eleventh))
            .unwrap()
            .is_none()
    );
    assert!(
        store
            .transact(|tx| tx.due_schedule_occurrences(&schedule, now, 256))
            .unwrap()
            .is_empty(),
        "deferred occurrence must be fenced by not_before"
    );

    assert_eq!(
        scheduler
            .process_calendar_due(at(now.get() + 1), "catch-up-worker")
            .unwrap(),
        1,
        "the continuation must be discoverable even when next recurrence is in the future"
    );
    assert!(
        store
            .transact(|tx| tx.schedule_occurrence_task(&schedule, &eleventh))
            .unwrap()
            .is_some()
    );
    assert_eq!(
        scheduler
            .process_calendar_due(at(now.get() + 2), "catch-up-worker")
            .unwrap(),
        0
    );
    drop(scheduler);
    drop(store);
    std::fs::remove_file(db).unwrap();
}

#[test]
fn run_each_catch_up_boundaries_cover_zero_one_nine_ten_and_eleven_due() {
    let db = path();
    let bus = EventBus::new(DeterministicUlidSource::starting_at(1_700_300_000_000).unwrap());
    let store = Store::open(&db, &Fixed).unwrap();
    let mut scheduler = Scheduler::new(
        Store::open(&db, &Fixed).unwrap(),
        TaskEngine::new(Store::open(&db, &Fixed).unwrap(), bus.clone()),
        bus,
    );
    let cases = [
        ("51", "61", "2020-01-12T00:00"),
        ("52", "62", "2020-01-11T00:00"),
        ("53", "63", "2020-01-03T00:00"),
        ("54", "64", "2020-01-02T00:00"),
        ("55", "65", "2020-01-01T00:00"),
        ("56", "66", "2019-12-01T00:00"),
    ];
    for (schedule_suffix, event_suffix, anchor) in cases {
        daily_schedule_at(
            &mut scheduler,
            &format!("sch_000000000000000000000000{schedule_suffix}"),
            &format!("evt_000000000000000000000000{event_suffix}"),
            MissedOccurrencePolicy::RunEach,
            anchor,
        );
    }
    let now = at(1_578_700_800_000); // 2020-01-11 00:00 UTC
    assert_eq!(
        scheduler
            .process_calendar_due(now, "boundary-worker")
            .unwrap(),
        40
    );
    let eleventh_schedule = ScheduleId::new("sch_00000000000000000000000055").unwrap();
    let eleventh_key = occurrence_identity_key("2020-01-11T00:00", "UTC").unwrap();
    assert!(
        store
            .transact(|tx| tx.schedule_occurrence_task(&eleventh_schedule, &eleventh_key))
            .unwrap()
            .is_none()
    );
    assert_eq!(
        scheduler
            .process_calendar_due(at(now.get() + 1), "boundary-worker")
            .unwrap(),
        11
    );
    for (offset, expected) in [(2, 10), (3, 10), (4, 2)] {
        assert_eq!(
            scheduler
                .process_calendar_due(at(now.get() + offset), "boundary-worker")
                .unwrap(),
            expected
        );
    }
    drop(scheduler);
    drop(store);
    std::fs::remove_file(db).unwrap();
}

#[test]
fn run_once_maps_only_latest_missed_identity_and_skip_creates_no_tasks() {
    let db = path();
    let bus = EventBus::new(DeterministicUlidSource::starting_at(1_700_300_000_000).unwrap());
    let store = Store::open(&db, &Fixed).unwrap();
    let mut scheduler = Scheduler::new(
        Store::open(&db, &Fixed).unwrap(),
        TaskEngine::new(Store::open(&db, &Fixed).unwrap(), bus.clone()),
        bus,
    );
    let now = at(1_578_700_800_000);
    let run_once = daily_schedule(
        &mut scheduler,
        "sch_00000000000000000000000042",
        "evt_00000000000000000000000042",
        MissedOccurrencePolicy::RunOnce,
    );
    assert_eq!(
        scheduler
            .process_calendar_due(now, "run-once-worker")
            .unwrap(),
        1
    );
    let latest = occurrence_identity_key("2020-01-11T00:00", "UTC").unwrap();
    assert!(
        store
            .transact(|tx| tx.schedule_occurrence_task(&run_once, &latest))
            .unwrap()
            .is_some()
    );
    for day in 1..11 {
        let key = occurrence_identity_key(&format!("2020-01-{day:02}T00:00"), "UTC").unwrap();
        assert!(
            store
                .transact(|tx| tx.schedule_occurrence_task(&run_once, &key))
                .unwrap()
                .is_none()
        );
    }

    let skip = daily_schedule(
        &mut scheduler,
        "sch_00000000000000000000000043",
        "evt_00000000000000000000000043",
        MissedOccurrencePolicy::Skip,
    );
    assert_eq!(
        scheduler.process_calendar_due(now, "skip-worker").unwrap(),
        0
    );
    assert!(
        store
            .transact(|tx| tx.due_schedule_occurrences(&skip, now, 256))
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        scheduler
            .process_calendar_due(at(now.get() + 1), "skip-worker")
            .unwrap(),
        0
    );
    drop(scheduler);
    drop(store);
    std::fs::remove_file(db).unwrap();
}

#[test]
fn ordered_scheduler_recovery_twice_reuses_the_committed_once_mapping() {
    let db = path();
    let bus = EventBus::new(DeterministicUlidSource::starting_at(1_700_300_000_000).unwrap());
    let store = Store::open(&db, &Fixed).unwrap();
    let mut scheduler = Scheduler::new(
        Store::open(&db, &Fixed).unwrap(),
        TaskEngine::new(Store::open(&db, &Fixed).unwrap(), bus.clone()),
        bus,
    );
    let schedule = ScheduleId::new("sch_00000000000000000000000044").unwrap();
    scheduler
        .create_schedule(
            ScheduleDefinition {
                schedule_id: schedule.clone(),
                owner_kind: ScheduleOwnerKind::Host,
                owner_id: "host-test".into(),
                trigger_kind: ScheduleTriggerKind::Calendar,
                recurrence_json: Some(
                    r#"{"version":"1","kind":"ONCE","anchor_local":"2020-01-02T03:04"}"#.into(),
                ),
                event_predicate_json: None,
                template_json: br#"{"version":"1","title":"Once","intent":"Perform work."}"#
                    .to_vec(),
                title_data_class: DataClass::Public,
                intent_data_class: DataClass::Public,
                policy_class_rank: 0,
                approval_policy: serde_json::json!({}),
                timezone: Some("UTC".into()),
                missed_policy: MissedOccurrencePolicy::RunEach,
            },
            &EventId::new("evt_00000000000000000000000044").unwrap(),
            &Digest::new(format!("sha256:{}", "c".repeat(64))).unwrap(),
            at(1),
        )
        .unwrap();

    let first = scheduler
        .recover(at(1_700_000_000_000), "startup-a", 256)
        .unwrap();
    assert_eq!(first.calendar_tasks_created_or_reused, 1);
    let key = occurrence_identity_key("2020-01-02T03:04", "UTC").unwrap();
    let first_task = store
        .transact(|tx| tx.schedule_occurrence_task(&schedule, &key))
        .unwrap()
        .unwrap();

    // A later process may claim the singleton cursor only after the previous
    // lease expires. It must observe the durable mapping and do no new work.
    let second = scheduler
        .recover(at(1_700_120_000_001), "startup-b", 256)
        .unwrap();
    assert_eq!(second.calendar_tasks_created_or_reused, 0);
    assert_eq!(
        store
            .transact(|tx| tx.schedule_occurrence_task(&schedule, &key))
            .unwrap(),
        Some(first_task)
    );
    drop(scheduler);
    drop(store);
    std::fs::remove_file(db).unwrap();
}

#[test]
fn recovery_reclaims_only_expired_claim_and_keeps_mapping_idempotent() {
    let db = path();
    let bus = EventBus::new(DeterministicUlidSource::starting_at(1_700_300_000_000).unwrap());
    let store = Store::open(&db, &Fixed).unwrap();
    let mut scheduler = Scheduler::new(
        Store::open(&db, &Fixed).unwrap(),
        TaskEngine::new(Store::open(&db, &Fixed).unwrap(), bus.clone()),
        bus,
    );
    let schedule = daily_schedule(
        &mut scheduler,
        "sch_00000000000000000000000045",
        "evt_00000000000000000000000045",
        MissedOccurrencePolicy::RunEach,
    );
    let now = at(1_577_836_800_000); // 2020-01-01 00:00 UTC
    assert_eq!(scheduler.enqueue_due_calendar_occurrences(now).unwrap(), 1);
    let key = occurrence_identity_key("2020-01-01T00:00", "UTC").unwrap();
    store
        .transact(|tx| {
            tx.claim_schedule_occurrence(
                &schedule,
                &key,
                1,
                "crashed-worker",
                now,
                at(now.get() + 10),
            )
        })
        .unwrap();

    let recovered = scheduler
        .recover(at(now.get() + 11), "recovery-worker", 256)
        .unwrap();
    assert_eq!(recovered.expired_claims_recovered, 1);
    let task_id = store
        .transact(|tx| tx.schedule_occurrence_task(&schedule, &key))
        .unwrap()
        .unwrap();
    assert_eq!(
        scheduler
            .process_calendar_due(at(now.get() + 12), "recovery-worker")
            .unwrap(),
        0
    );
    assert_eq!(
        store
            .transact(|tx| tx.schedule_occurrence_task(&schedule, &key))
            .unwrap(),
        Some(task_id)
    );
    drop(scheduler);
    drop(store);
    std::fs::remove_file(db).unwrap();
}
