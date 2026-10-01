//! Deterministic time and identifier minting.
//!
//! Model Protocol §10: "The `ModelProvider` trait takes no ambient state. All
//! variability — temperature, seed, the model roster, the clock, the provider's
//! responses — is injected." Everything here is an injection point.

use std::time::Duration;

use serea_protocol::ids::{IdMinter, TimestampMs, UlidSource, UlidValue};
use serea_protocol::{ProtocolError, Timestamp};

/// The frozen epoch every deterministic scenario starts from
/// (GoalLatch Adapter §6.1).
pub const FROZEN_EPOCH: &str = "2026-10-01T00:00:00.000Z";

/// The millisecond timestamp behind [`FROZEN_EPOCH`], used as the default
/// starting point for deterministic identifier minting. Well inside the frozen
/// 48-bit range (`2^48-1` is `281_474_976_710_655`).
const BASE_TIMESTAMP_MS: u64 = 1_789_862_400_000;

/// A clock that only moves when a test moves it.
///
/// It never reads a wall clock: `.clippy.toml` bans `SystemTime::now` and
/// `Instant::now` workspace-wide so this cannot regress into one.
#[derive(Debug, Clone)]
pub struct TestClock {
    year: i64,
    month: i64,
    day: i64,
    hour: i64,
    minute: i64,
    second: i64,
    millis: i64,
    /// Total fake milliseconds advanced since the clock was created, which is a
    /// monotonic counter independent of the calendar position.
    elapsed_ms: u64,
}

impl Default for TestClock {
    fn default() -> Self {
        Self::at_epoch()
    }
}

impl TestClock {
    /// A clock parked at the frozen epoch.
    pub fn at_epoch() -> Self {
        Self::at(FROZEN_EPOCH).unwrap_or_else(|error| {
            unreachable!("the frozen epoch is a valid timestamp by construction: {error:?}")
        })
    }

    /// A clock parked at `start`.
    ///
    /// Returns a typed error rather than panicking: a scripted epoch is caller
    /// input, and this is the only time source in the workspace.
    pub fn at(start: &str) -> Result<Self, ProtocolError> {
        let parts = parse_frozen_timestamp(start);
        match parts {
            Some(parts) => Ok(Self {
                year: parts.year,
                month: parts.month,
                day: parts.day,
                hour: parts.hour,
                minute: parts.minute,
                second: parts.second,
                millis: parts.millis,
                elapsed_ms: 0,
            }),
            None => Err(ProtocolError::MalformedValue {
                field: serea_protocol::ValueField::Timestamp,
                reason: serea_protocol::ValueRejection::Malformed,
            }),
        }
    }

    /// The current fake time. Never changes unless a test changes it.
    ///
    /// Total by construction: the only mutator is [`TestClock::advance`], which
    /// validates the resulting instant before committing it.
    pub fn now(&self) -> Timestamp {
        Timestamp::new(self.format()).unwrap_or_else(|error| {
            unreachable!("`advance` commits only a validated instant: {error:?}")
        })
    }

    /// Advances fake time by `delta` and returns the new time.
    ///
    /// Returns a typed error rather than panicking when the result would leave
    /// the frozen wire form — advancing past year 9999, for instance. The clock
    /// is left untouched in that case, so a test that overruns does not corrupt
    /// the state it was building.
    pub fn advance(&mut self, delta: Duration) -> Result<Timestamp, ProtocolError> {
        let overflow = || ProtocolError::MalformedValue {
            field: serea_protocol::ValueField::Timestamp,
            reason: serea_protocol::ValueRejection::OutOfRange,
        };
        // `Duration::as_millis` saturates at `u64::MAX`, which does not fit
        // `i64`, and clamping it to `i64::MAX` would then overflow the addition
        // below in a debug build. Reject the unrepresentable delta outright.
        let delta_ms = i64::try_from(delta.as_millis()).map_err(|_| overflow())?;
        let mut candidate = self.clone();
        candidate.elapsed_ms = self
            .elapsed_ms
            .saturating_add(u64::try_from(delta_ms).unwrap_or(u64::MAX));
        candidate.add_millis(delta_ms);
        // Validate before committing.
        Timestamp::new(candidate.format())?;
        *self = candidate;
        Ok(self.now())
    }

    fn format(&self) -> String {
        format!(
            "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}.{:03}Z",
            self.year, self.month, self.day, self.hour, self.minute, self.second, self.millis
        )
    }

    /// The total fake milliseconds advanced since the clock was created.
    pub fn elapsed_ms(&self) -> u64 {
        self.elapsed_ms
    }

    fn add_millis(&mut self, delta_ms: i64) {
        let total = self.millis + delta_ms;
        self.millis = total.rem_euclid(1_000);
        let mut carry = total.div_euclid(1_000);

        self.second += carry;
        carry = self.second.div_euclid(60);
        self.second = self.second.rem_euclid(60);

        self.minute += carry;
        carry = self.minute.div_euclid(60);
        self.minute = self.minute.rem_euclid(60);

        self.hour += carry;
        carry = self.hour.div_euclid(24);
        self.hour = self.hour.rem_euclid(24);

        self.day += carry;
        // Normalise the calendar, honouring month lengths and leap years.
        while self.day > i64::from(days_in_month(self.year, self.month)) {
            self.day -= i64::from(days_in_month(self.year, self.month));
            self.month += 1;
            if self.month > 12 {
                self.month = 1;
                self.year += 1;
            }
        }
    }
}

/// Splits a frozen-format timestamp into its calendar parts.
fn parse_frozen_timestamp(value: &str) -> Option<CalendarParts> {
    if value.len() != 24 {
        return None;
    }
    let bytes = value.as_bytes();
    if bytes[4] != b'-'
        || bytes[7] != b'-'
        || bytes[10] != b'T'
        || bytes[13] != b':'
        || bytes[16] != b':'
        || bytes[19] != b'.'
        || bytes[23] != b'Z'
    {
        return None;
    }
    let number = |range: std::ops::Range<usize>| -> Option<i64> {
        range.clone().try_fold(0i64, |accumulator, index| {
            let digit = *bytes.get(index)?;
            if digit.is_ascii_digit() {
                Some(accumulator * 10 + i64::from(digit - b'0'))
            } else {
                None
            }
        })
    };
    let parts = CalendarParts {
        year: number(0..4)?,
        month: number(5..7)?,
        day: number(8..10)?,
        hour: number(11..13)?,
        minute: number(14..16)?,
        second: number(17..19)?,
        millis: number(20..23)?,
    };
    let valid = (1..=12).contains(&parts.month)
        && parts.day >= 1
        && parts.day <= i64::from(days_in_month(parts.year, parts.month))
        && parts.hour <= 23
        && parts.minute <= 59
        && parts.second <= 59;
    valid.then_some(parts)
}

#[derive(Debug, Clone, Copy)]
struct CalendarParts {
    year: i64,
    month: i64,
    day: i64,
    hour: i64,
    minute: i64,
    second: i64,
    millis: i64,
}

/// The proleptic Gregorian leap rule.
fn is_leap_year(year: i64) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}

fn days_in_month(year: i64, month: i64) -> u32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if is_leap_year(year) => 29,
        2 => 28,
        _ => 0,
    }
}

/// Identifier minting driven by a counter rather than by randomness.
///
/// Protocol Index §2 rule 1 makes a ULID the only minting scheme. Supplying the
/// 48-bit timestamp from an injected counter is what makes a minting sequence
/// reproducible across runs and across processes, which no test can rely on
/// from a wall clock or an unseeded RNG.
#[derive(Debug, Clone)]
pub struct DeterministicUlidSource {
    /// The validated starting millisecond.
    starting_ms: TimestampMs,
    /// The millisecond the next mint carries. Holds at the frozen range end.
    next_timestamp_ms: TimestampMs,
    /// Mints so far. Drives the entropy so uniqueness never depends on the
    /// timestamp staying strictly increasing.
    counter: u64,
    entropy_seed: u8,
}

impl Default for DeterministicUlidSource {
    fn default() -> Self {
        Self::new()
    }
}

impl DeterministicUlidSource {
    /// A source that starts at the frozen base timestamp.
    pub fn new() -> Self {
        // The base is a literal inside the frozen 48-bit range, so the only
        // failure below is unreachable; the alternative would be a `const fn`
        // validator, and this is the single place the assumption lives.
        let base = match TimestampMs::new(BASE_TIMESTAMP_MS) {
            Ok(base) => base,
            Err(error) => unreachable!("the frozen base timestamp is in range: {error:?}"),
        };
        Self {
            starting_ms: base,
            next_timestamp_ms: base,
            counter: 0,
            entropy_seed: 0x5a,
        }
    }

    /// A source whose first identifier carries `timestamp_ms` exactly, for a
    /// scenario that needs its identifiers to sort after another scenario's.
    ///
    /// Returns a typed error for a value outside the frozen 48-bit range rather
    /// than panicking on it.
    pub fn starting_at(timestamp_ms: u64) -> Result<Self, ProtocolError> {
        let base = TimestampMs::new(timestamp_ms)?;
        Ok(Self {
            starting_ms: base,
            next_timestamp_ms: base,
            counter: 0,
            entropy_seed: 0x5a,
        })
    }

    /// Returns the source to its starting state, so two harnesses can replay the
    /// same script.
    pub fn reset(&mut self) {
        self.next_timestamp_ms = self.starting_ms;
        self.counter = 0;
    }

    /// The number of identifiers minted so far.
    pub fn minted(&self) -> u64 {
        self.counter
    }

    /// The millisecond the next mint will carry.
    pub fn next_timestamp_ms(&self) -> u64 {
        self.next_timestamp_ms.get()
    }
}

impl UlidSource for DeterministicUlidSource {
    fn next_ulid(&mut self) -> UlidValue {
        let timestamp_ms = self.next_timestamp_ms;
        // Hold at the range end rather than overflowing. A script that mints
        // more identifiers than the 48-bit millisecond range holds is not a
        // scenario, and holding keeps the timestamps monotonic; the
        // counter-derived entropy below keeps every mint distinct either way.
        self.next_timestamp_ms = self
            .next_timestamp_ms
            .checked_next()
            .unwrap_or(self.next_timestamp_ms);
        self.counter += 1;
        let entropy = [
            self.entropy_seed.wrapping_add(self.counter as u8),
            (self.counter >> 8) as u8,
            (self.counter >> 16) as u8,
            self.entropy_seed,
            self.entropy_seed,
            self.entropy_seed.wrapping_add((self.counter >> 24) as u8),
            (self.counter >> 32) as u8,
            (self.counter >> 40) as u8,
            (self.counter >> 48) as u8,
            (self.counter >> 56) as u8,
        ];
        UlidValue::new(timestamp_ms, entropy)
    }
}

/// A minter wired to a deterministic source, for tests that need stable
/// identifiers across runs.
pub fn deterministic_minter() -> IdMinter<DeterministicUlidSource> {
    IdMinter::new(DeterministicUlidSource::new())
}
