//! Injected time (Crate Map §3; Model Protocol §10) and wire-bounded instants.
//!
//! Calendar conversion uses the proleptic Gregorian calendar of [`Timestamp`],
//! including year 0000. This module provides no wall-clock implementation.

use crate::{ProtocolError, Timestamp, ValueField, ValueRejection};

const MILLIS_PER_DAY: i64 = 86_400_000;

/// Unix epoch milliseconds in the complete frozen [`Timestamp`] domain.
///
/// Unlike ULID's unsigned 48-bit [`crate::TimestampMs`], this signed value can
/// represent pre-1970 instants and cannot exceed the four-digit wire year range.
/// Numeric ordering is chronological. The private field and validating
/// constructor keep every value convertible to a wire timestamp.
///
/// `i64` is architecture-independent and fits a future SQLite INTEGER; no
/// persistence or storage implementation is provided here.
///
/// Validated construction and reading work through the public API:
///
/// ```
/// use serea_protocol::EpochMillis;
/// let epoch = EpochMillis::new(0).unwrap();
/// assert_eq!(epoch.get(), 0);
/// ```
///
/// External callers cannot bypass range validation or read the private field:
///
/// ```compile_fail
/// use serea_protocol::EpochMillis;
/// let invalid = EpochMillis(i64::MIN);
/// ```
///
/// ```compile_fail
/// use serea_protocol::EpochMillis;
/// let epoch = EpochMillis::new(0).unwrap();
/// let raw = epoch.0;
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EpochMillis(i64);

impl EpochMillis {
    /// Inclusive minimum: `0000-01-01T00:00:00.000Z` (719,528 days before epoch).
    pub const MIN: i64 = -62_167_219_200_000;
    /// Inclusive maximum: `9999-12-31T23:59:59.999Z` (end of epoch day 2,932,896).
    pub const MAX: i64 = 253_402_300_799_999;

    /// Rejects values outside the existing Timestamp wire grammar's domain.
    pub fn new(millis: i64) -> Result<Self, ProtocolError> {
        if (Self::MIN..=Self::MAX).contains(&millis) {
            Ok(Self(millis))
        } else {
            Err(ProtocolError::MalformedValue {
                field: ValueField::Timestamp,
                reason: ValueRejection::OutOfRange,
            })
        }
    }

    /// The signed Unix epoch millisecond value, suitable for integer comparison.
    pub const fn get(self) -> i64 {
        self.0
    }
}

/// Synchronous, object-safe time injection point (Crate Map §3; Model §10).
///
/// Providers return a wire-representable instant or a typed error. This port
/// neither reads ambient state nor promises monotonicity: deterministic hosts
/// inject their own clock. No async runtime or real wall-clock provider is here.
pub trait Clock: Send + Sync {
    /// Reads the injected current instant without changing it.
    fn now_ms(&self) -> Result<EpochMillis, ProtocolError>;
}

pub(crate) fn to_epoch_millis(timestamp: &Timestamp) -> EpochMillis {
    // Only Timestamp's validator admits bytes: fixed ASCII digits, both legal
    // lengths, real dates, years 0000..9999. This extracts, not revalidates, them.
    let bytes = timestamp.as_str().as_bytes();
    let number = |start: usize, end: usize| -> i64 {
        bytes[start..end]
            .iter()
            .fold(0, |acc, digit| acc * 10 + i64::from(digit - b'0'))
    };
    let days = days_from_civil(number(0, 4), number(5, 7), number(8, 10));
    let millis = if bytes.len() == 24 { number(20, 23) } else { 0 };
    let value = days
        .checked_mul(MILLIS_PER_DAY)
        .and_then(|ms| ms.checked_add(number(11, 13) * 3_600_000))
        .and_then(|ms| ms.checked_add(number(14, 16) * 60_000))
        .and_then(|ms| ms.checked_add(number(17, 19) * 1_000))
        .and_then(|ms| ms.checked_add(millis))
        .unwrap_or_else(|| unreachable!("validated four-digit calendar arithmetic fits i64"));
    EpochMillis::new(value).unwrap_or_else(|error| {
        unreachable!("Timestamp's grammar defines EpochMillis bounds: {error:?}")
    })
}

pub(crate) fn canonical_wire(epoch: EpochMillis) -> String {
    // Euclidean division is essential before the epoch: -1 ms is the last
    // millisecond of day -1, not a negative fractional part of day 0.
    let days = epoch.get().div_euclid(MILLIS_PER_DAY);
    let within_day = epoch.get().rem_euclid(MILLIS_PER_DAY);
    let (year, month, day) = civil_from_days(days);
    let hour = within_day / 3_600_000;
    let minute = within_day / 60_000 % 60;
    let second = within_day / 1_000 % 60;
    let millis = within_day % 1_000;
    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}.{millis:03}Z")
}

// Howard Hinnant's days_from_civil / civil_from_days Gregorian algorithms
// (https://howardhinnant.github.io/date_algorithms.html), expressed with
// Euclidean era division. March starts each year so leap day is at its end;
// every 400-year era has exactly 146,097 days. The 719,468-day offset makes
// 1970-01-01 day zero. Conversion is constant-time, with no month/day iteration.
// These helpers are private and called only with validated wire dates/days:
// adjusted year -1..9999, era -1..24, day index -719528..2932896. All intermediate
// products/sums are bounded far inside i64; caller epoch accumulation is checked.
fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let year = year - i64::from(month <= 2);
    let era = year.div_euclid(400);
    let year_of_era = year - era * 400;
    let march_month = month + if month > 2 { -3 } else { 9 };
    let day_of_year = (153 * march_month + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - 719_468
}

fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let days = days + 719_468;
    let era = days.div_euclid(146_097);
    let day_of_era = days - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let march_month = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * march_month + 2) / 5 + 1;
    let month = march_month + if march_month < 10 { 3 } else { -9 };
    (year + i64::from(month <= 2), month, day)
}
