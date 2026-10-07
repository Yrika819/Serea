use serea_scheduler::CalendarRecurrenceV1;

fn parse(json: &str) -> Result<CalendarRecurrenceV1, serea_scheduler::RecurrenceError> {
    CalendarRecurrenceV1::parse_json(json)
}

#[test]
fn once_daily_and_weekly_are_the_only_valid_kinds() {
    assert!(parse(r#"{"version":"1","kind":"ONCE","anchor_local":"2024-02-29T09:15"}"#).is_ok());
    assert!(
        parse(r#"{"version":"1","kind":"DAILY","anchor_local":"2024-02-29T09:15","interval":1}"#)
            .is_ok()
    );
    assert!(parse(r#"{"version":"1","kind":"WEEKLY","anchor_local":"2024-02-29T09:15","interval":2,"weekdays":["MO","TH"]}"#).is_ok());
    assert!(
        parse(r#"{"version":"1","kind":"MONTHLY","anchor_local":"2024-02-29T09:15"}"#).is_err()
    );
    assert!(parse(r#"{"version":"1","kind":"HOURLY","anchor_local":"2024-02-29T09:15"}"#).is_err());
}

#[test]
fn grammar_is_closed_and_duplicate_json_keys_are_refused() {
    assert!(
        parse(r#"{"version":"1","kind":"ONCE","anchor_local":"2024-02-29T09:15","extra":true}"#)
            .is_err()
    );
    assert!(
        parse(r#"{"version":"1","version":"1","kind":"ONCE","anchor_local":"2024-02-29T09:15"}"#)
            .is_err()
    );
    assert!(
        parse(r#"{"version":"1","kind":"ONCE","anchor_local":"2024-02-29T09:15","interval":1}"#)
            .is_err()
    );
}

#[test]
fn local_label_requires_exact_civil_minute_and_real_date() {
    for invalid in [
        "2024-02-29T09:15:01",
        "2024-02-29T09:15Z",
        "2024-02-29T09:15+01:00",
        "2023-02-29T09:15",
        "2024-13-01T09:15",
        "2024-02-29T24:00",
        "2024-02-29T09:60",
    ] {
        let json = format!(r#"{{"version":"1","kind":"ONCE","anchor_local":"{invalid}"}}"#);
        assert!(parse(&json).is_err(), "accepted {invalid}");
    }
}

#[test]
fn intervals_and_weekdays_are_validated_and_canonicalized() {
    assert!(
        parse(r#"{"version":"1","kind":"DAILY","anchor_local":"2024-01-01T09:00","interval":0}"#)
            .is_err()
    );
    assert!(parse(r#"{"version":"1","kind":"WEEKLY","anchor_local":"2024-01-01T09:00","interval":1,"weekdays":[]}"#).is_err());
    assert!(parse(r#"{"version":"1","kind":"WEEKLY","anchor_local":"2024-01-01T09:00","interval":1,"weekdays":["MO","MO"]}"#).is_err());
    let input = r#"{"version":"1","kind":"WEEKLY","anchor_local":"2024-01-01T09:00","interval":1,"weekdays":["SU","MO","WE"]}"#;
    let parsed = parse(input).unwrap();
    assert_eq!(
        parsed.canonical_json(),
        r#"{"anchor_local":"2024-01-01T09:00","interval":1,"kind":"WEEKLY","version":"1","weekdays":["MO","WE","SU"]}"#
    );
}

#[test]
fn equivalent_inputs_have_one_stable_serialization() {
    let first = parse(r#"{"version":"1","kind":"WEEKLY","anchor_local":"2024-01-01T09:00","interval":1,"weekdays":["FR","MO"]}"#).unwrap();
    let second = parse(r#"{ "weekdays":["MO","FR"],"interval":1,"anchor_local":"2024-01-01T09:00","kind":"WEEKLY","version":"1" }"#).unwrap();
    assert_eq!(first.canonical_json(), second.canonical_json());
}

#[test]
fn daily_occurrences_use_local_calendar_days_and_cover_calendar_boundaries() {
    let every_day =
        parse(r#"{"version":"1","kind":"DAILY","anchor_local":"2024-02-28T09:15","interval":1}"#)
            .unwrap();
    assert_eq!(
        every_day.next_local_label(None).unwrap().as_deref(),
        Some("2024-02-28T09:15")
    );
    assert_eq!(
        every_day
            .next_local_label(Some("2024-02-28T09:15"))
            .unwrap()
            .as_deref(),
        Some("2024-02-29T09:15")
    );
    assert_eq!(
        every_day
            .next_local_label(Some("2024-02-29T09:15"))
            .unwrap()
            .as_deref(),
        Some("2024-03-01T09:15")
    );
    let interval =
        parse(r#"{"version":"1","kind":"DAILY","anchor_local":"2023-12-31T23:59","interval":2}"#)
            .unwrap();
    assert_eq!(
        interval
            .next_local_label(Some("2024-01-01T23:59"))
            .unwrap()
            .as_deref(),
        Some("2024-01-02T23:59")
    );
}

#[test]
fn weekly_occurrences_use_iso_weeks_and_do_not_emit_before_anchor() {
    let weekly = parse(r#"{"version":"1","kind":"WEEKLY","anchor_local":"2024-01-03T08:30","interval":2,"weekdays":["MO","WE"]}"#).unwrap();
    assert_eq!(
        weekly.next_local_label(None).unwrap().as_deref(),
        Some("2024-01-03T08:30")
    );
    assert_eq!(
        weekly
            .next_local_label(Some("2024-01-03T08:30"))
            .unwrap()
            .as_deref(),
        Some("2024-01-15T08:30")
    );
    assert_eq!(
        weekly
            .next_local_label(Some("2024-01-15T08:30"))
            .unwrap()
            .as_deref(),
        Some("2024-01-17T08:30")
    );
    let only_monday = parse(r#"{"version":"1","kind":"WEEKLY","anchor_local":"2024-01-03T08:30","interval":1,"weekdays":["MO"]}"#).unwrap();
    assert_eq!(
        only_monday.next_local_label(None).unwrap().as_deref(),
        Some("2024-01-08T08:30")
    );
}

#[test]
fn once_has_one_label_and_no_successor() {
    let once = parse(r#"{"version":"1","kind":"ONCE","anchor_local":"2024-01-03T08:30"}"#).unwrap();
    assert_eq!(
        once.next_local_label(None).unwrap().as_deref(),
        Some("2024-01-03T08:30")
    );
    assert_eq!(
        once.next_local_label(Some("2024-01-03T08:30")).unwrap(),
        None
    );
    assert!(once.next_local_label(Some("not-a-local-label")).is_err());
}

#[test]
fn dst_gap_resolves_to_first_valid_minute_and_fold_uses_earlier_utc() {
    let gap =
        CalendarRecurrenceV1::resolve_local_label("2024-03-10T02:30", "America/New_York").unwrap();
    assert_eq!(gap.intended_local_label, "2024-03-10T02:30");
    assert_eq!(gap.due_at.get(), 1_710_054_000_000);
    assert_eq!(gap.tzdb_version, "2026e");
    let fold =
        CalendarRecurrenceV1::resolve_local_label("2024-11-03T01:30", "America/New_York").unwrap();
    assert_eq!(fold.due_at.get(), 1_730_611_800_000);
    assert_eq!(fold.evaluator_version, "jiff-0.2.38");
}

#[test]
fn identity_uses_local_label_and_zone_as_canonical_structured_data() {
    let key =
        serea_scheduler::occurrence_identity_key("2024-11-03T01:30", "America/New_York").unwrap();
    assert_eq!(
        key,
        r#"{"intended_local_label":"2024-11-03T01:30","timezone":"America/New_York"}"#
    );
    assert!(CalendarRecurrenceV1::resolve_local_label("2024-11-03T01:30", "+01:00").is_err());
    assert!(serea_scheduler::occurrence_identity_key("2024-11-03T01:30", "+01:00").is_err());
}
