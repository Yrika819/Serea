//! P2B signed instants preserve the frozen Timestamp grammar, not wire spelling.

use serea_protocol::{EpochMillis, ProtocolError, Timestamp, ValueField, ValueRejection};

fn instant(text: &str) -> EpochMillis {
    Timestamp::new(text)
        .expect("legal timestamp")
        .to_epoch_millis()
}

#[test]
fn hashing_uses_original_wire_spelling_even_after_epoch_conversion() {
    use std::hash::{Hash, Hasher};

    #[derive(Default)]
    struct RecordingHasher(Vec<u8>);
    impl Hasher for RecordingHasher {
        fn write(&mut self, bytes: &[u8]) {
            self.0.extend_from_slice(bytes);
        }
        fn finish(&self) -> u64 {
            0 // Record input, not final hash inequality (collisions are legal).
        }
    }
    fn recorded(value: &impl Hash) -> Vec<u8> {
        let mut hasher = RecordingHasher::default();
        value.hash(&mut hasher);
        hasher.0
    }
    let seconds = Timestamp::new("2026-10-03T09:14:22Z").expect("seconds");
    let millis = Timestamp::new("2026-10-03T09:14:22.000Z").expect("millis");
    let before = recorded(&seconds);
    let epoch = seconds.to_epoch_millis();
    assert_eq!(Timestamp::from_epoch_millis(epoch), millis);
    assert_eq!(recorded(&seconds), before);
    assert_eq!(recorded(&seconds), recorded(&seconds.as_str().to_owned()));
    assert_eq!(recorded(&millis), recorded(&millis.as_str().to_owned()));
    assert_ne!(recorded(&seconds), recorded(&millis));
}

#[test]
fn the_2038_second_boundary_retains_exact_milliseconds() {
    for (text, expected) in [
        ("2038-01-19T03:14:07.999Z", 2_147_483_647_999),
        ("2038-01-19T03:14:08.000Z", 2_147_483_648_000),
    ] {
        assert_eq!(instant(text).get(), expected);
        assert_eq!(
            Timestamp::from_epoch_millis(EpochMillis::new(expected).expect("valid")).as_str(),
            text
        );
    }
    assert!(instant("2038-01-19T03:14:07.999Z") < instant("2038-01-19T03:14:08.000Z"));
}

#[test]
fn numeric_instant_order_crosses_epoch_zero_chronologically() {
    let negative = instant("1969-12-31T23:59:59.999Z");
    let zero = instant("1970-01-01T00:00:00Z");
    let positive = instant("1970-01-01T00:00:00.001Z");
    assert!(negative < zero);
    assert!(zero < positive);
    assert_eq!(negative.cmp(&zero), std::cmp::Ordering::Less);
    assert_eq!(
        positive.partial_cmp(&zero),
        Some(std::cmp::Ordering::Greater)
    );
}

#[test]
fn mixed_time_of_day_and_minute_hour_carries_match_independent_values() {
    for (text, expected) in [
        ("1970-01-01T01:02:03.004Z", 3_723_004),
        ("1969-12-31T01:02:03.004Z", -82_676_996),
        ("1970-01-01T00:00:59.999Z", 59_999),
        ("1970-01-01T00:01:00.000Z", 60_000),
        ("1970-01-01T00:59:59.999Z", 3_599_999),
        ("1970-01-01T01:00:00.000Z", 3_600_000),
    ] {
        assert_eq!(instant(text).get(), expected, "{text}");
        assert_eq!(
            Timestamp::from_epoch_millis(EpochMillis::new(expected).expect("valid")).as_str(),
            text
        );
    }
    for (before, after) in [(59_999, 60_000), (3_599_999, 3_600_000)] {
        let next = EpochMillis::new(before + 1).expect("next ms");
        assert_eq!(next.get(), after);
        assert_eq!(
            Timestamp::from_epoch_millis(next).to_epoch_millis().get(),
            after
        );
    }
}

#[test]
fn epoch_anchor_and_adjacent_milliseconds() {
    for (text, expected) in [
        ("1970-01-01T00:00:00Z", 0),
        ("1970-01-01T00:00:00.000Z", 0),
        ("1970-01-01T00:00:00.001Z", 1),
        ("1969-12-31T23:59:59.999Z", -1),
        ("1969-12-31T23:59:59.000Z", -1000),
        ("1969-12-31T23:59:58.999Z", -1001),
        ("1969-12-31T23:59:59.001Z", -999),
    ] {
        assert_eq!(instant(text).get(), expected, "{text}");
    }
}

#[test]
fn earliest_wire_instant_and_year_zero_leap_day() {
    assert_eq!(EpochMillis::MIN, -62_167_219_200_000);
    assert_eq!(instant("0000-01-01T00:00:00Z").get(), EpochMillis::MIN);
    assert_eq!(
        Timestamp::from_epoch_millis(EpochMillis::new(EpochMillis::MIN).expect("min")).as_str(),
        "0000-01-01T00:00:00.000Z"
    );
    assert_eq!(
        instant("0000-03-01T00:00:00Z").get() - instant("0000-02-29T00:00:00Z").get(),
        86_400_000
    );
}

#[test]
fn final_wire_instant_is_exactly_the_maximum() {
    assert_eq!(EpochMillis::MAX, 253_402_300_799_999);
    assert_eq!(instant("9999-12-31T23:59:59.999Z").get(), EpochMillis::MAX);
    assert_eq!(
        Timestamp::from_epoch_millis(EpochMillis::new(EpochMillis::MAX).expect("max")).as_str(),
        "9999-12-31T23:59:59.999Z"
    );
}

#[test]
fn out_of_wire_range_values_return_typed_errors() {
    for invalid in [
        EpochMillis::MIN - 1,
        EpochMillis::MAX + 1,
        i64::MIN,
        i64::MAX,
    ] {
        assert_eq!(
            EpochMillis::new(invalid),
            Err(ProtocolError::MalformedValue {
                field: ValueField::Timestamp,
                reason: ValueRejection::OutOfRange,
            })
        );
    }
}

#[test]
fn older_legal_dates_have_signed_known_values() {
    for (text, expected) in [
        ("0001-01-01T00:00:00Z", -62_135_596_800_000),
        ("1900-01-01T00:00:00Z", -2_208_988_800_000),
        ("2000-01-01T00:00:00Z", 946_684_800_000),
        ("2026-10-01T00:00:00Z", 1_790_812_800_000),
    ] {
        assert_eq!(instant(text).get(), expected, "{text}");
    }
}

#[test]
fn leap_and_non_leap_february_transitions() {
    for year in [0, 2000, 2004, 2024] {
        let feb28 = instant(&format!("{year:04}-02-28T00:00:00Z")).get();
        let feb29 = instant(&format!("{year:04}-02-29T00:00:00Z")).get();
        let march = instant(&format!("{year:04}-03-01T00:00:00Z")).get();
        assert_eq!(feb29 - feb28, 86_400_000);
        assert_eq!(march - feb29, 86_400_000);
    }
    for year in [1900, 2026, 2100] {
        assert!(Timestamp::new(format!("{year}-02-29T00:00:00Z")).is_err());
        assert_eq!(
            instant(&format!("{year}-03-01T00:00:00Z")).get()
                - instant(&format!("{year}-02-28T00:00:00Z")).get(),
            86_400_000
        );
    }
}

#[test]
fn millisecond_precision_and_second_day_year_carries() {
    for (before, after) in [
        ("1970-01-01T00:00:00.000Z", "1970-01-01T00:00:00.001Z"),
        ("1970-01-01T00:00:00.998Z", "1970-01-01T00:00:00.999Z"),
        ("1970-01-01T00:00:00.999Z", "1970-01-01T00:00:01.000Z"),
        ("2026-10-03T23:59:59.999Z", "2026-10-04T00:00:00.000Z"),
        ("1999-12-31T23:59:59.999Z", "2000-01-01T00:00:00.000Z"),
    ] {
        let next = EpochMillis::new(instant(before).get() + 1).expect("next millisecond");
        assert_eq!(next, instant(after));
        assert_eq!(Timestamp::from_epoch_millis(next).as_str(), after);
    }
}

#[test]
fn lexical_wire_order_is_not_chronological_but_epoch_order_is() {
    let earlier = Timestamp::new("2026-10-03T09:14:22Z").expect("valid");
    let later = Timestamp::new("2026-10-03T09:14:22.100Z").expect("valid");
    assert!(
        earlier.as_str() > later.as_str(),
        "wire byte order is inverted"
    );
    assert!(earlier.to_epoch_millis() < later.to_epoch_millis());
}

#[test]
fn converting_a_deserialized_timestamp_does_not_rewrite_its_wire_spelling() {
    for text in [
        "0000-01-01T00:00:00Z",
        "2026-10-03T09:14:22Z",
        "2026-10-03T09:14:22.000Z",
    ] {
        let json = serde_json::to_string(text).expect("json");
        let original: Timestamp = serde_json::from_str(&json).expect("timestamp");
        let reconstructed = Timestamp::from_epoch_millis(original.to_epoch_millis());
        assert_eq!(reconstructed.to_epoch_millis(), original.to_epoch_millis());
        assert_eq!(serde_json::to_string(&original).expect("serialize"), json);
        assert_eq!(reconstructed.as_str().len(), 24);
    }
}

#[test]
fn deterministic_epoch_corpus_round_trips_exactly() {
    for ms in [
        EpochMillis::MIN,
        EpochMillis::MIN + 1,
        -62_135_596_800_000,
        -2_208_988_800_000,
        -86_400_001,
        -86_400_000,
        -86_399_999,
        -1001,
        -1000,
        -999,
        -1,
        0,
        1,
        999,
        1000,
        86_399_999,
        86_400_000,
        946_684_800_000,
        1_790_812_800_000,
        EpochMillis::MAX - 1,
        EpochMillis::MAX,
    ] {
        let epoch = EpochMillis::new(ms).expect("in wire domain");
        let timestamp = Timestamp::from_epoch_millis(epoch);
        assert_eq!(timestamp.to_epoch_millis(), epoch, "{ms}: {timestamp}");
    }
}

#[test]
fn all_month_boundaries_match_an_independent_running_day_reference() {
    // Test-only reference: count months from the pinned MIN, independently of
    // production's March-based era algorithm. Covers all 120,000 legal months.
    let mut reference_ms = EpochMillis::MIN;
    for year in 0..=9999 {
        for month in 1..=12 {
            let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
            let days = match month {
                2 if leap => 29,
                2 => 28,
                4 | 6 | 9 | 11 => 30,
                _ => 31,
            };
            for (text, expected) in [
                (format!("{year:04}-{month:02}-01T00:00:00Z"), reference_ms),
                (
                    format!("{year:04}-{month:02}-{days:02}T23:59:59.999Z"),
                    reference_ms + days * 86_400_000 - 1,
                ),
            ] {
                let timestamp = Timestamp::new(&text).expect("legal boundary");
                assert_eq!(timestamp.to_epoch_millis().get(), expected, "{text}");
                let canonical =
                    Timestamp::from_epoch_millis(EpochMillis::new(expected).expect("in range"));
                assert_eq!(canonical.to_epoch_millis(), timestamp.to_epoch_millis());
                assert_eq!(&canonical.as_str()[..19], &text[..19]);
            }
            reference_ms += days * 86_400_000;
        }
    }
    assert_eq!(reference_ms - 1, EpochMillis::MAX);
}

#[test]
fn timestamp_grammar_still_refuses_offsets_fractions_and_impossible_dates() {
    for invalid in [
        "-001-01-01T00:00:00Z",
        "10000-01-01T00:00:00Z",
        "2026-02-29T00:00:00Z",
        "1970-01-01T00:00:60Z",
        "1970-01-01T00:00:00+00:00",
        "1970-01-01T00:00:00.1Z",
        "1970-01-01T00:00:00.0000Z",
        "２０２６-10-03T09:14:22Z",
    ] {
        assert!(Timestamp::new(invalid).is_err(), "{invalid}");
    }
}

#[test]
fn ulid_timestamp_range_remains_separate_and_unchanged() {
    assert_eq!(serea_protocol::TimestampMs::MAX, (1_u64 << 48) - 1);
    assert!(serea_protocol::TimestampMs::new(serea_protocol::TimestampMs::MAX).is_ok());
    assert!(serea_protocol::TimestampMs::new(serea_protocol::TimestampMs::MAX + 1).is_err());
    assert!(
        EpochMillis::new(i64::try_from(serea_protocol::TimestampMs::MAX).expect("fits i64"))
            .is_err()
    );
}

#[test]
fn pre_epoch_timestamp_converts_to_signed_milliseconds() {
    let timestamp = Timestamp::new("1969-12-31T23:59:59.999Z").expect("legal pre-epoch wire date");
    assert_eq!(timestamp.to_epoch_millis().get(), -1);
}

#[test]
fn both_wire_spellings_denote_the_same_instant() {
    let seconds = Timestamp::new("2026-10-03T09:14:22Z").expect("seconds form");
    let millis = Timestamp::new("2026-10-03T09:14:22.000Z").expect("millisecond form");
    assert_eq!(seconds.to_epoch_millis(), millis.to_epoch_millis());
    assert_ne!(seconds, millis, "Timestamp equality remains spelling-based");
    assert!(
        seconds.as_str() > millis.as_str(),
        "even equal instants have distinct lexical order"
    );
    assert_eq!(seconds.as_str(), "2026-10-03T09:14:22Z");
}

#[test]
fn epoch_reconstruction_emits_canonical_millisecond_spelling() {
    let epoch = EpochMillis::new(0).expect("epoch is representable");
    assert_eq!(
        Timestamp::from_epoch_millis(epoch).as_str(),
        "1970-01-01T00:00:00.000Z"
    );
}
