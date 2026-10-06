use serea_event_bus::{EventBus, ReplayItem};
use serea_protocol::{
    Actor, ActorId, ActorKind, DataClass, EnvelopeVersion, EpochMillis, EventId, EventKind, SemVer,
    Seq, SereaEvent, Timestamp, WireSurface,
};
use serea_storage::{Store, StoreError};

struct Fixed;
impl serea_protocol::Clock for Fixed {
    fn now_ms(&self) -> Result<EpochMillis, serea_protocol::ProtocolError> {
        Ok(at(0))
    }
}

fn at(value: i64) -> EpochMillis {
    EpochMillis::new(value).unwrap()
}

fn event(n: u32) -> SereaEvent {
    let mut payload = serde_json::Map::new();
    payload.insert("n".into(), serde_json::json!(n));
    SereaEvent {
        envelope_version: EnvelopeVersion::new("1").unwrap(),
        surface: WireSurface::new(WireSurface::EVENT).unwrap(),
        message_id: EventId::new(format!("evt_{n:026}")).unwrap(),
        seq: Seq::new(0),
        kind: EventKind::TaskCreated,
        occurred_at: Timestamp::from_epoch_millis(at(i64::from(n))),
        correlation_id: None,
        causation_id: None,
        actor: Actor {
            kind: ActorKind::Host,
            id: ActorId::new("event-bus-test").unwrap(),
            version: SemVer::new("1.0.0").unwrap(),
            extensions: Default::default(),
        },
        data_class: DataClass::Public,
        trace: None,
        payload,
        extensions: Default::default(),
    }
}

fn store() -> Store {
    Store::open_in_memory(&Fixed).unwrap()
}

fn append(store: &Store, n: u32, retention_at: Option<EpochMillis>) {
    store
        .transact(|tx| EventBus::append(tx, event(n), retention_at).map(|_| ()))
        .unwrap();
}

#[test]
fn replay_pages_are_ordered_and_hold_one_committed_high_water_snapshot() {
    let store = store();
    for n in 1..=3 {
        append(&store, n, None);
    }

    let first = EventBus::replay(&store, None, None, 2).unwrap();
    assert_eq!(first.snapshot_high_water_seq, Seq::new(3));
    assert_eq!(first.next_seq, Some(Seq::new(2)));
    assert!(matches!(first.items[0], ReplayItem::Event { event: ref e } if e.seq == Seq::new(1)));
    assert!(matches!(first.items[1], ReplayItem::Event { event: ref e } if e.seq == Seq::new(2)));

    append(&store, 4, None);
    let second = EventBus::replay(
        &store,
        first.next_seq,
        Some(first.snapshot_high_water_seq),
        2,
    )
    .unwrap();
    assert_eq!(second.snapshot_high_water_seq, Seq::new(3));
    assert_eq!(second.next_seq, Some(Seq::new(3)));
    assert!(
        matches!(second.items.as_slice(), [ReplayItem::Event { event }]
        if event.seq == Seq::new(3))
    );
    let next_pass = EventBus::replay(&store, second.next_seq, None, 2).unwrap();
    assert_eq!(next_pass.snapshot_high_water_seq, Seq::new(4));
    assert!(
        matches!(next_pass.items.as_slice(), [ReplayItem::Event { event }]
        if event.seq == Seq::new(4))
    );
}

#[test]
fn expiry_is_a_typed_interior_range_and_prefix_compaction_is_distinct() {
    let store = store();
    append(&store, 1, Some(at(10)));
    append(&store, 2, Some(at(30)));
    append(&store, 3, Some(at(10)));

    let deleted = EventBus::expire_eligible(&store, at(20), 512).unwrap();
    assert_eq!(deleted.content_records_deleted, 2);
    let page = EventBus::replay(&store, None, None, 10).unwrap();
    assert!(matches!(page.items.as_slice(),
        [ReplayItem::HistoryExpiredPrefix { new_replay_boundary }]
        if *new_replay_boundary == Seq::new(1)));

    let stale = EventBus::replay(&store, None, None, 10).unwrap();
    assert!(
        matches!(stale.items.as_slice(), [ReplayItem::HistoryExpiredPrefix { new_replay_boundary }]
        if *new_replay_boundary == Seq::new(1))
    );
    let after_prefix = EventBus::replay(&store, Some(Seq::new(1)), None, 10).unwrap();
    assert!(matches!(after_prefix.items.as_slice(),
        [ReplayItem::Event { event }, ReplayItem::ExpiredRange { first_seq, last_seq }]
        if event.seq == Seq::new(2)
            && *first_seq == Seq::new(3)
            && *last_seq == Seq::new(3)));
}

#[test]
fn expiry_does_not_wait_for_a_consumer_and_limit_is_bounded() {
    let store = store();
    for n in 1..=3 {
        append(&store, n, Some(at(1)));
    }
    assert_eq!(
        EventBus::replay(&store, None, None, 0).unwrap_err(),
        StoreError::InvalidEventReplayPage
    );
    assert_eq!(
        EventBus::replay(&store, None, None, 257).unwrap_err(),
        StoreError::InvalidEventReplayPage
    );
    let deleted = EventBus::expire_eligible(&store, at(1), 512).unwrap();
    assert_eq!(deleted.content_records_deleted, 3);
    assert_eq!(deleted.expired_prefix_through, Seq::new(3));
    assert_eq!(
        EventBus::replay(&store, None, None, 256).unwrap().items,
        [ReplayItem::HistoryExpiredPrefix {
            new_replay_boundary: Seq::new(3)
        }]
    );
    store.verify_integrity().unwrap();
    assert_eq!(
        EventBus::expire_eligible(&store, at(1), 0).unwrap_err(),
        StoreError::InvalidRetentionDeleteBatch
    );
    assert_eq!(
        EventBus::expire_eligible(&store, at(1), 513).unwrap_err(),
        StoreError::InvalidRetentionDeleteBatch
    );
}

#[test]
fn unexplained_absence_is_corruption_and_does_not_advance_past_it() {
    let path = std::env::temp_dir().join(format!("serea-p3d-gap-{}.sqlite", std::process::id()));
    let store = Store::open(&path, &Fixed).unwrap();
    append(&store, 1, None);
    append(&store, 2, None);
    append(&store, 3, None);
    rusqlite::Connection::open(&path)
        .unwrap()
        .execute("DELETE FROM event_content WHERE seq=2", [])
        .unwrap();

    let page = EventBus::replay(&store, None, None, 10).unwrap();
    assert_eq!(page.next_seq, Some(Seq::new(1)));
    assert!(matches!(page.items.as_slice(),
        [ReplayItem::Event { event: a }, ReplayItem::Corruption { first_unexplained_seq }, ..]
        if a.seq == Seq::new(1) && *first_unexplained_seq == Seq::new(2)));
    assert_eq!(
        store.verify_integrity(),
        Err(StoreError::EventHistoryCorrupt)
    );
    drop(store);
    std::fs::remove_file(path).unwrap();
}

#[test]
fn expiry_metadata_and_content_deletion_roll_back_together() {
    let path = std::env::temp_dir().join(format!(
        "serea-p3d-retention-rollback-{}.sqlite",
        std::process::id()
    ));
    let store = Store::open(&path, &Fixed).unwrap();
    append(&store, 1, Some(at(10)));
    let initial_bytes: i64 = rusqlite::Connection::open(&path)
        .unwrap()
        .query_row(
            "SELECT event_store_bytes FROM event_store_state WHERE singleton=1",
            [],
            |row| row.get(0),
        )
        .unwrap();
    rusqlite::Connection::open(&path)
        .unwrap()
        .execute_batch(
            "CREATE TRIGGER reject_event_expiry BEFORE DELETE ON event_content
             BEGIN SELECT RAISE(ABORT, 'injected content deletion failure'); END;",
        )
        .unwrap();

    assert_eq!(
        EventBus::expire_eligible(&store, at(10), 512),
        Err(StoreError::ConstraintViolation)
    );
    let conn = rusqlite::Connection::open(&path).unwrap();
    let state: (i64, i64, i64, i64, i64) = conn
        .query_row(
            "SELECT (SELECT count(*) FROM event_content),
                    (SELECT count(*) FROM event_sequence_ledger),
                    (SELECT count(*) FROM event_expired_ranges),
                    retained_count,event_store_bytes
             FROM event_store_state WHERE singleton=1",
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
    assert_eq!(state, (1, 1, 0, 1, initial_bytes));
    drop(conn);
    store.verify_integrity().unwrap();
    drop(store);
    std::fs::remove_file(path).unwrap();
}

#[test]
fn replay_discriminator_serializes_the_frozen_device_v2_shapes() {
    let range = serde_json::to_value(ReplayItem::ExpiredRange {
        first_seq: Seq::new(4),
        last_seq: Seq::new(9),
    })
    .unwrap();
    assert_eq!(
        range,
        serde_json::json!({
            "kind": "INTENTIONALLY_EXPIRED_RANGE",
            "first_seq": "4",
            "last_seq": "9"
        })
    );
    let prefix = serde_json::to_value(ReplayItem::HistoryExpiredPrefix {
        new_replay_boundary: Seq::new(9),
    })
    .unwrap();
    assert_eq!(
        prefix,
        serde_json::json!({
            "kind": "HISTORY_EXPIRED_PREFIX",
            "new_replay_boundary": "9"
        })
    );
    let corruption = serde_json::to_value(ReplayItem::Corruption {
        first_unexplained_seq: Seq::new(10),
    })
    .unwrap();
    assert_eq!(
        corruption,
        serde_json::json!({
            "kind": "CORRUPTION",
            "first_unexplained_seq": "10"
        })
    );
}
