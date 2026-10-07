use serea_protocol::{
    Actor, ActorId, ActorKind, DataClass, EnvelopeVersion, EpochMillis, EventId, EventKind, SemVer,
    Seq, SereaEvent, Timestamp, WireSurface,
};
use serea_scheduler::{EventCausality, EventPredicateV1, matches_host_event};

fn event(kind: EventKind) -> SereaEvent {
    let at = EpochMillis::new(0).unwrap();
    SereaEvent {
        envelope_version: EnvelopeVersion::new("1").unwrap(),
        surface: WireSurface::new(WireSurface::EVENT).unwrap(),
        message_id: EventId::new("evt_00000000000000000000000001").unwrap(),
        seq: Seq::new(1),
        kind,
        occurred_at: Timestamp::from_epoch_millis(at),
        correlation_id: None,
        causation_id: None,
        actor: Actor {
            kind: ActorKind::Host,
            id: ActorId::new("predicate-test").unwrap(),
            version: SemVer::new("1.0.0").unwrap(),
            extensions: Default::default(),
        },
        data_class: DataClass::Public,
        trace: None,
        payload: serde_json::json!({"ignored": "payload must not match"})
            .as_object()
            .unwrap()
            .clone(),
        extensions: Default::default(),
    }
}

fn parse(kind: &str) -> Result<EventPredicateV1, serea_scheduler::EventPredicateError> {
    EventPredicateV1::parse_json(&format!(r#"{{"version":"1","event_kind":"{kind}"}}"#))
}

#[test]
fn only_exact_registered_kind_matches_and_payload_is_ignored() {
    let predicate = parse("TASK_CREATED").unwrap();
    assert!(matches_host_event(
        &predicate,
        &event(EventKind::TaskCreated),
        EventCausality::Independent
    ));
    assert!(!matches_host_event(
        &predicate,
        &event(EventKind::TaskFailed),
        EventCausality::Independent
    ));
}

#[test]
fn scheduler_causal_root_never_matches() {
    let predicate = parse("TASK_CREATED").unwrap();
    assert!(!matches_host_event(
        &predicate,
        &event(EventKind::TaskCreated),
        EventCausality::SchedulerOccurrence
    ));
}

#[test]
fn predicate_is_closed_versioned_and_rejects_duplicate_keys() {
    for input in [
        r#"{"version":"2","event_kind":"TASK_CREATED"}"#,
        r#"{"version":"1","event_kind":"NO_SUCH_EVENT"}"#,
        r#"{"version":"1","event_kind":"TASK_CREATED","payload":{}}"#,
        r#"{"version":"1","version":"1","event_kind":"TASK_CREATED"}"#,
        r#"{"version":"1","event_kind":"TASK_CREATED","extra":1.0}"#,
    ] {
        assert!(
            EventPredicateV1::parse_json(input).is_err(),
            "accepted {input}"
        );
    }
}

#[test]
fn canonical_round_trip_is_stable() {
    let first =
        EventPredicateV1::parse_json(r#"{ "event_kind" : "TASK_CREATED", "version" : "1" }"#)
            .unwrap();
    let second = EventPredicateV1::parse_json(first.canonical_json()).unwrap();
    assert_eq!(first, second);
    assert_eq!(
        first.canonical_json(),
        r#"{"event_kind":"TASK_CREATED","version":"1"}"#
    );
}

#[test]
fn every_registered_event_kind_has_pinned_predicate_eligibility() {
    let excluded = [
        "APPROVAL_GRANTED",
        "APPROVAL_DENIED",
        "APPROVAL_EXPIRED",
        "DEVICE_CONNECTED",
        "SCHEDULE_CREATED",
        "SCHEDULE_UPDATED",
        "SCHEDULE_PAUSED",
        "SCHEDULE_RESUMED",
        "SCHEDULE_CANCELLED",
        "SCHEDULE_OCCURRENCE_MISSED",
        "SCHEDULE_TASK_CREATED",
        "SCHEDULE_CATCH_UP_DEFERRED",
    ];
    for token in EventKind::WIRE_NAMES {
        let predicate = parse(token);
        assert_eq!(
            predicate.is_ok(),
            !excluded.contains(token),
            "unexpected HOST_EVENT eligibility for {token}"
        );
    }
}
