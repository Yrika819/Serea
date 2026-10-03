//! P2B Clock port, single-authority time, and failure-atomic advancement.

use std::time::Duration;

use serea_protocol::{Clock, EpochMillis, ProtocolError, Timestamp, ValueField, ValueRejection};
use serea_testkit::{FROZEN_EPOCH, TestClock};

fn range_error() -> ProtocolError {
    ProtocolError::MalformedValue {
        field: ValueField::Timestamp,
        reason: ValueRejection::OutOfRange,
    }
}

#[test]
fn clock_is_send_sync_object_safe_and_stable_without_advancement() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<TestClock>();
    assert_send_sync::<EpochMillis>();
    assert_send_sync::<Box<dyn Clock>>();
    let clock = TestClock::at_epoch();
    let port: &dyn Clock = &clock;
    for _ in 0..10 {
        assert_eq!(
            port.now_ms().expect("valid"),
            clock.now_ms().expect("valid")
        );
        assert_eq!(port.now_ms().expect("valid"), clock.now().to_epoch_millis());
    }
    assert_eq!(clock.now().as_str(), FROZEN_EPOCH);
    assert_eq!(clock.now_ms().expect("valid").get(), 1_790_812_800_000);
    assert_eq!(clock.elapsed_ms(), 0);
}

#[test]
fn seconds_and_millisecond_initialization_share_protocol_grammar() {
    let seconds = TestClock::at("1969-12-31T23:59:59Z").expect("seconds form");
    let millis = TestClock::at("1969-12-31T23:59:59.000Z").expect("ms form");
    assert_eq!(seconds.now_ms(), millis.now_ms());
    assert_eq!(seconds.now(), millis.now());
    assert_eq!(seconds.now().as_str(), "1969-12-31T23:59:59.000Z");
    assert_eq!(seconds.elapsed_ms(), 0);
}

#[test]
fn advancing_zero_one_and_submillisecond_durations_uses_whole_milliseconds() {
    let mut clock = TestClock::at("1969-12-31T23:59:59.999Z").expect("valid");
    let initial = clock.now();
    assert_eq!(clock.advance(Duration::ZERO).expect("zero"), initial);
    assert_eq!(
        clock
            .advance(Duration::from_nanos(999_999))
            .expect("sub-ms truncation preserved"),
        initial
    );
    assert_eq!(clock.elapsed_ms(), 0);
    assert_eq!(
        clock
            .advance(Duration::from_millis(1))
            .expect("one")
            .as_str(),
        "1970-01-01T00:00:00.000Z"
    );
    assert_eq!(clock.now_ms().expect("valid").get(), 0);
    assert_eq!(clock.elapsed_ms(), 1);
}

#[test]
fn day_leap_and_year_rollovers_use_the_same_instant() {
    for (before, after) in [
        ("0000-02-28T23:59:59.999Z", "0000-02-29T00:00:00.000Z"),
        ("1900-02-28T23:59:59.999Z", "1900-03-01T00:00:00.000Z"),
        ("2000-02-28T23:59:59.999Z", "2000-02-29T00:00:00.000Z"),
        ("2000-02-29T23:59:59.999Z", "2000-03-01T00:00:00.000Z"),
        ("2004-02-28T23:59:59.999Z", "2004-02-29T00:00:00.000Z"),
        ("2024-02-28T23:59:59.999Z", "2024-02-29T00:00:00.000Z"),
        ("2026-02-28T23:59:59.999Z", "2026-03-01T00:00:00.000Z"),
        ("2100-02-28T23:59:59.999Z", "2100-03-01T00:00:00.000Z"),
        ("2026-10-03T23:59:59.999Z", "2026-10-04T00:00:00.000Z"),
        ("2026-12-31T23:59:59.999Z", "2027-01-01T00:00:00.000Z"),
    ] {
        let mut clock = TestClock::at(before).expect("valid");
        assert_eq!(
            clock
                .advance(Duration::from_millis(1))
                .expect("carry")
                .as_str(),
            after
        );
        assert_eq!(
            clock.now_ms().expect("valid"),
            Timestamp::new(after).expect("valid").to_epoch_millis()
        );
        assert_eq!(clock.elapsed_ms(), 1);
    }
}

#[test]
fn overflow_and_enormous_durations_leave_both_time_and_elapsed_unchanged() {
    for start in [
        "0000-01-01T00:00:00.500Z",
        "2026-10-01T00:00:00.500Z",
        "9999-12-31T23:59:59.998Z",
    ] {
        let mut clock = TestClock::at(start).expect("valid");
        clock
            .advance(Duration::from_millis(1))
            .expect("room for one");
        let before = (clock.now(), clock.now_ms(), clock.elapsed_ms());
        for delta in [
            Duration::MAX,
            Duration::from_millis(u64::MAX),
            Duration::from_millis(i64::MAX as u64),
            Duration::from_millis(i64::MAX as u64 - 1000),
        ] {
            assert_eq!(
                clock.advance(delta),
                Err(range_error()),
                "{start}: {delta:?}"
            );
            assert_eq!((clock.now(), clock.now_ms(), clock.elapsed_ms()), before);
        }
    }
    let mut max = TestClock::at("9999-12-31T23:59:59.999Z").expect("max");
    assert_eq!(max.advance(Duration::from_millis(1)), Err(range_error()));
    assert_eq!(max.now_ms().expect("valid").get(), EpochMillis::MAX);
    assert_eq!(max.elapsed_ms(), 0);
    assert_eq!(
        max.advance(Duration::ZERO).expect("zero at max").as_str(),
        "9999-12-31T23:59:59.999Z"
    );
}

#[test]
fn entire_wire_range_can_be_advanced_in_one_checked_operation() {
    let mut clock = TestClock::at("0000-01-01T00:00:00Z").expect("min");
    let span: u64 = 315_569_519_999_999;
    assert_eq!(
        clock
            .advance(Duration::from_millis(span))
            .expect("full range")
            .as_str(),
        "9999-12-31T23:59:59.999Z"
    );
    assert_eq!(clock.elapsed_ms(), span);
    assert_eq!(clock.advance(Duration::from_millis(1)), Err(range_error()));
    assert_eq!(clock.elapsed_ms(), span);
}

#[test]
fn identical_scripts_replay_identical_instants_wire_values_and_elapsed_times() {
    let script = [0, 1, 999, 86_400_000, 31_536_000_000];
    let run = || {
        let mut clock = TestClock::at("1969-12-31T23:59:59.999Z").expect("valid");
        script.map(|ms| {
            let timestamp = clock.advance(Duration::from_millis(ms)).expect("valid");
            (
                timestamp,
                clock.now_ms().expect("valid"),
                clock.elapsed_ms(),
            )
        })
    };
    assert_eq!(run(), run());
}

#[test]
fn a_failing_injected_clock_returns_its_typed_error_through_the_port() {
    struct RefusingClock;
    impl Clock for RefusingClock {
        fn now_ms(&self) -> Result<EpochMillis, ProtocolError> {
            Err(range_error())
        }
    }
    let port: &dyn Clock = &RefusingClock;
    assert_eq!(port.now_ms(), Err(range_error()));
}

#[test]
fn p2b_sources_do_not_call_ambient_clocks_and_clippy_keeps_both_bans() {
    let sources = [
        include_str!("../../serea-protocol/src/clock.rs"),
        include_str!("../../serea-protocol/src/types.rs"),
        include_str!("../../serea-protocol/src/lib.rs"),
        include_str!("../src/clock.rs"),
        include_str!("../src/lib.rs"),
        include_str!("../../serea-protocol/tests/p2b_time.rs"),
        include_str!("fakes_are_deterministic.rs"),
    ];
    for source in sources {
        let compact: String = source.chars().filter(|c| !c.is_whitespace()).collect();
        for banned in ["SystemTime::now(", "Instant::now("] {
            assert!(!compact.contains(banned), "ambient call {banned}");
        }
        assert!(!compact.contains("to_ne_bytes("));
        assert!(!compact.contains("from_ne_bytes("));
    }
    let config = include_str!("../../../.clippy.toml");
    assert!(config.contains("std::time::SystemTime::now"));
    assert!(config.contains("std::time::Instant::now"));
    // This assertion's own banned strings are data, not clock calls. Inspect
    // this test file as part of the final source-level review as well.
}
