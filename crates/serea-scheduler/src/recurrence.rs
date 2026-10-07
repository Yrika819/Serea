use std::collections::BTreeSet;

use jiff::civil::Date;
use jiff::tz::{AmbiguousOffset, TimeZone};
use serde::Deserialize;
use serde_json::{Map, Value};
use serea_protocol::{EpochMillis, canonicalize};

const EVALUATOR_VERSION: &str = "jiff-0.2.38";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecurrenceError {
    Invalid,
    OutOfRange,
}

impl std::fmt::Display for RecurrenceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Invalid => "invalid CalendarRecurrenceV1",
            Self::OutOfRange => "calendar recurrence is outside the supported time domain",
        })
    }
}

impl std::error::Error for RecurrenceError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Weekday {
    Monday,
    Tuesday,
    Wednesday,
    Thursday,
    Friday,
    Saturday,
    Sunday,
}

impl Weekday {
    fn parse(value: &str) -> Result<Self, RecurrenceError> {
        match value {
            "MO" => Ok(Self::Monday),
            "TU" => Ok(Self::Tuesday),
            "WE" => Ok(Self::Wednesday),
            "TH" => Ok(Self::Thursday),
            "FR" => Ok(Self::Friday),
            "SA" => Ok(Self::Saturday),
            "SU" => Ok(Self::Sunday),
            _ => Err(RecurrenceError::Invalid),
        }
    }

    fn wire(self) -> &'static str {
        match self {
            Self::Monday => "MO",
            Self::Tuesday => "TU",
            Self::Wednesday => "WE",
            Self::Thursday => "TH",
            Self::Friday => "FR",
            Self::Saturday => "SA",
            Self::Sunday => "SU",
        }
    }

    fn index(self) -> u8 {
        match self {
            Self::Monday => 0,
            Self::Tuesday => 1,
            Self::Wednesday => 2,
            Self::Thursday => 3,
            Self::Friday => 4,
            Self::Saturday => 5,
            Self::Sunday => 6,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CalendarRecurrenceKind {
    Once,
    Daily {
        interval: u32,
    },
    Weekly {
        interval: u32,
        weekdays: Vec<Weekday>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CalendarRecurrenceV1 {
    kind: CalendarRecurrenceKind,
    anchor_local: String,
    anchor_date: Date,
    anchor_hour: i8,
    anchor_minute: i8,
    canonical_json: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedCalendarOccurrence {
    pub intended_local_label: String,
    pub timezone: String,
    pub due_at: EpochMillis,
    pub evaluator_version: &'static str,
    pub tzdb_version: &'static str,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawRecurrence {
    version: String,
    kind: String,
    anchor_local: String,
    interval: Option<u32>,
    weekdays: Option<Vec<String>>,
}

impl CalendarRecurrenceV1 {
    pub fn parse_json(input: &str) -> Result<Self, RecurrenceError> {
        let canonical_input = canonicalize(input).map_err(|_| RecurrenceError::Invalid)?;
        let value: Value =
            serde_json::from_slice(&canonical_input).map_err(|_| RecurrenceError::Invalid)?;
        let object = value.as_object().ok_or(RecurrenceError::Invalid)?;
        let raw: RawRecurrence =
            serde_json::from_value(value.clone()).map_err(|_| RecurrenceError::Invalid)?;
        if raw.version != "1" {
            return Err(RecurrenceError::Invalid);
        }
        let (anchor_date, anchor_hour, anchor_minute) = parse_local_minute(&raw.anchor_local)?;
        let expected = match raw.kind.as_str() {
            "ONCE" => ["anchor_local", "kind", "version"].into_iter().collect(),
            "DAILY" => ["anchor_local", "interval", "kind", "version"]
                .into_iter()
                .collect(),
            "WEEKLY" => ["anchor_local", "interval", "kind", "version", "weekdays"]
                .into_iter()
                .collect(),
            _ => return Err(RecurrenceError::Invalid),
        };
        let actual: BTreeSet<&str> = object.keys().map(String::as_str).collect();
        if actual != expected {
            return Err(RecurrenceError::Invalid);
        }
        let kind = match raw.kind.as_str() {
            "ONCE" => CalendarRecurrenceKind::Once,
            "DAILY" => CalendarRecurrenceKind::Daily {
                interval: raw
                    .interval
                    .filter(|v| *v > 0)
                    .ok_or(RecurrenceError::Invalid)?,
            },
            "WEEKLY" => {
                let interval = raw
                    .interval
                    .filter(|v| *v > 0)
                    .ok_or(RecurrenceError::Invalid)?;
                let mut weekdays = raw
                    .weekdays
                    .ok_or(RecurrenceError::Invalid)?
                    .iter()
                    .map(|day| Weekday::parse(day))
                    .collect::<Result<Vec<_>, _>>()?;
                if weekdays.is_empty() {
                    return Err(RecurrenceError::Invalid);
                }
                weekdays.sort_by_key(|day| day.index());
                if weekdays.windows(2).any(|pair| pair[0] == pair[1]) {
                    return Err(RecurrenceError::Invalid);
                }
                CalendarRecurrenceKind::Weekly { interval, weekdays }
            }
            _ => return Err(RecurrenceError::Invalid),
        };
        let canonical_json = canonicalize(&serialize(&raw.anchor_local, &kind)?)
            .map_err(|_| RecurrenceError::Invalid)?;
        let canonical_json =
            String::from_utf8(canonical_json).map_err(|_| RecurrenceError::Invalid)?;
        Ok(Self {
            kind,
            anchor_local: raw.anchor_local,
            anchor_date,
            anchor_hour,
            anchor_minute,
            canonical_json,
        })
    }

    pub fn kind(&self) -> &CalendarRecurrenceKind {
        &self.kind
    }

    pub fn anchor_local(&self) -> &str {
        &self.anchor_local
    }

    pub fn canonical_json(&self) -> &str {
        &self.canonical_json
    }

    /// Returns the first recurrence label strictly after `after_local`; `None`
    /// starts at the anchor. The result is a local civil minute without zone.
    pub fn next_local_label(
        &self,
        after_local: Option<&str>,
    ) -> Result<Option<String>, RecurrenceError> {
        if let Some(value) = after_local {
            parse_local_minute(value)?;
        }
        if matches!(self.kind, CalendarRecurrenceKind::Once) {
            return match after_local {
                None => Ok(Some(self.anchor_local.clone())),
                Some(after) if after < self.anchor_local.as_str() => {
                    Ok(Some(self.anchor_local.clone()))
                }
                Some(_) => Ok(None),
            };
        }
        let after = match after_local {
            Some(value) => Some(parse_local_minute(value)?),
            None => None,
        };
        let mut date = after
            .map(|(date, hour, minute)| {
                if (hour, minute) >= (self.anchor_hour, self.anchor_minute) {
                    date.checked_add(jiff::Span::new().days(1))
                } else {
                    Ok(date)
                }
            })
            .transpose()
            .map_err(|_| RecurrenceError::OutOfRange)?
            .unwrap_or(self.anchor_date);
        if date < self.anchor_date {
            date = self.anchor_date;
        }
        match self.kind {
            CalendarRecurrenceKind::Once => unreachable!(),
            CalendarRecurrenceKind::Daily { interval } => {
                let days = date
                    .since(self.anchor_date)
                    .map_err(|_| RecurrenceError::OutOfRange)?
                    .get_days();
                let delta = if days <= 0 {
                    0
                } else {
                    let period = i64::from(interval);
                    (i64::from(days) + period - 1) / period * period
                };
                let candidate = self
                    .anchor_date
                    .checked_add(
                        jiff::Span::new()
                            .days(i32::try_from(delta).map_err(|_| RecurrenceError::OutOfRange)?),
                    )
                    .map_err(|_| RecurrenceError::OutOfRange)?;
                Ok(Some(format_local(
                    candidate,
                    self.anchor_hour,
                    self.anchor_minute,
                )))
            }
            CalendarRecurrenceKind::Weekly {
                interval,
                ref weekdays,
            } => {
                let anchor_weekday = i64::from(self.anchor_date.weekday().to_monday_zero_offset());
                let delta_days = i64::from(
                    date.since(self.anchor_date)
                        .map_err(|_| RecurrenceError::OutOfRange)?
                        .get_days(),
                );
                let weekday = i64::from(date.weekday().to_monday_zero_offset());
                let mut week = (delta_days + anchor_weekday - weekday).div_euclid(7);
                if week < 0 {
                    week = 0;
                }
                let period = i64::from(interval);
                let remainder = week.rem_euclid(period);
                if remainder != 0 {
                    week = week
                        .checked_add(period - remainder)
                        .ok_or(RecurrenceError::OutOfRange)?;
                }
                loop {
                    let monday_offset = week
                        .checked_mul(7)
                        .and_then(|days| days.checked_sub(anchor_weekday))
                        .ok_or(RecurrenceError::OutOfRange)?;
                    let monday = self
                        .anchor_date
                        .checked_add(
                            jiff::Span::new().days(
                                i32::try_from(monday_offset)
                                    .map_err(|_| RecurrenceError::OutOfRange)?,
                            ),
                        )
                        .map_err(|_| RecurrenceError::OutOfRange)?;
                    for weekday in weekdays {
                        let candidate = monday
                            .checked_add(jiff::Span::new().days(i32::from(weekday.index())))
                            .map_err(|_| RecurrenceError::OutOfRange)?;
                        if candidate >= date && candidate >= self.anchor_date {
                            return Ok(Some(format_local(
                                candidate,
                                self.anchor_hour,
                                self.anchor_minute,
                            )));
                        }
                    }
                    week = week
                        .checked_add(period)
                        .ok_or(RecurrenceError::OutOfRange)?;
                }
            }
        }
    }

    pub fn resolve_local_label(
        local_label: &str,
        timezone: &str,
    ) -> Result<ResolvedCalendarOccurrence, RecurrenceError> {
        let (date, hour, minute) = parse_local_minute(local_label)?;
        if timezone.is_empty()
            || timezone.len() > 255
            || timezone.starts_with('+')
            || timezone.starts_with('-')
        {
            return Err(RecurrenceError::Invalid);
        }
        let zone = TimeZone::get(timezone).map_err(|_| RecurrenceError::Invalid)?;
        let civil = date.at(hour, minute, 0, 0);
        let ambiguous = zone.to_ambiguous_timestamp(civil);
        let timestamp = match ambiguous.offset() {
            AmbiguousOffset::Gap { .. } => {
                // The two candidate instants straddle the transition. Find the
                // first millisecond using the post-gap offset, including zones
                // whose historical transitions were not minute-aligned.
                let AmbiguousOffset::Gap { after, .. } = ambiguous.offset() else {
                    unreachable!("matched the gap variant")
                };
                let mut before_ms = ambiguous
                    .earlier()
                    .map_err(|_| RecurrenceError::OutOfRange)?
                    .as_millisecond();
                let mut after_ms = ambiguous
                    .later()
                    .map_err(|_| RecurrenceError::OutOfRange)?
                    .as_millisecond();
                while after_ms - before_ms > 1 {
                    let middle = before_ms + (after_ms - before_ms) / 2;
                    let instant = jiff::Timestamp::from_millisecond(middle)
                        .map_err(|_| RecurrenceError::OutOfRange)?;
                    if instant.to_zoned(zone.clone()).offset() == after {
                        after_ms = middle;
                    } else {
                        before_ms = middle;
                    }
                }
                jiff::Timestamp::from_millisecond(after_ms)
                    .map_err(|_| RecurrenceError::OutOfRange)?
            }
            AmbiguousOffset::Fold { .. } | AmbiguousOffset::Unambiguous { .. } => ambiguous
                .earlier()
                .map_err(|_| RecurrenceError::OutOfRange)?,
        };
        let due_at = EpochMillis::new(timestamp.as_millisecond())
            .map_err(|_| RecurrenceError::OutOfRange)?;
        let tzdb_version = jiff_tzdb::VERSION.ok_or(RecurrenceError::Invalid)?;
        Ok(ResolvedCalendarOccurrence {
            intended_local_label: local_label.to_owned(),
            timezone: timezone.to_owned(),
            due_at,
            evaluator_version: EVALUATOR_VERSION,
            tzdb_version,
        })
    }
}

pub fn occurrence_identity_key(
    local_label: &str,
    timezone: &str,
) -> Result<String, RecurrenceError> {
    parse_local_minute(local_label)?;
    if timezone.is_empty()
        || timezone.len() > 255
        || timezone.starts_with('+')
        || timezone.starts_with('-')
    {
        return Err(RecurrenceError::Invalid);
    }
    let mut object = Map::new();
    object.insert(
        "intended_local_label".into(),
        Value::String(local_label.to_owned()),
    );
    object.insert("timezone".into(), Value::String(timezone.to_owned()));
    let json =
        serde_json::to_string(&Value::Object(object)).map_err(|_| RecurrenceError::Invalid)?;
    let canonical = canonicalize(&json).map_err(|_| RecurrenceError::Invalid)?;
    String::from_utf8(canonical).map_err(|_| RecurrenceError::Invalid)
}

fn serialize(anchor: &str, kind: &CalendarRecurrenceKind) -> Result<String, RecurrenceError> {
    let mut object = Map::new();
    object.insert("anchor_local".into(), Value::String(anchor.to_owned()));
    match kind {
        CalendarRecurrenceKind::Once => {
            object.insert("kind".into(), Value::String("ONCE".into()));
        }
        CalendarRecurrenceKind::Daily { interval } => {
            object.insert("interval".into(), Value::from(*interval));
            object.insert("kind".into(), Value::String("DAILY".into()));
        }
        CalendarRecurrenceKind::Weekly { interval, weekdays } => {
            object.insert("interval".into(), Value::from(*interval));
            object.insert("kind".into(), Value::String("WEEKLY".into()));
            object.insert(
                "weekdays".into(),
                Value::Array(
                    weekdays
                        .iter()
                        .map(|day| Value::String(day.wire().into()))
                        .collect(),
                ),
            );
        }
    }
    object.insert("version".into(), Value::String("1".into()));
    serde_json::to_string(&Value::Object(object)).map_err(|_| RecurrenceError::Invalid)
}

fn parse_local_minute(value: &str) -> Result<(Date, i8, i8), RecurrenceError> {
    let bytes = value.as_bytes();
    if bytes.len() != 16
        || bytes[4] != b'-'
        || bytes[7] != b'-'
        || bytes[10] != b'T'
        || bytes[13] != b':'
    {
        return Err(RecurrenceError::Invalid);
    }
    if ![0..4, 5..7, 8..10, 11..13, 14..16]
        .iter()
        .all(|range| bytes[range.clone()].iter().all(u8::is_ascii_digit))
    {
        return Err(RecurrenceError::Invalid);
    }
    let year = value[0..4]
        .parse::<i16>()
        .map_err(|_| RecurrenceError::Invalid)?;
    let month = value[5..7]
        .parse::<i8>()
        .map_err(|_| RecurrenceError::Invalid)?;
    let day = value[8..10]
        .parse::<i8>()
        .map_err(|_| RecurrenceError::Invalid)?;
    let hour = value[11..13]
        .parse::<i8>()
        .map_err(|_| RecurrenceError::Invalid)?;
    let minute = value[14..16]
        .parse::<i8>()
        .map_err(|_| RecurrenceError::Invalid)?;
    if !(0..=9999).contains(&year) || !(0..=23).contains(&hour) || !(0..=59).contains(&minute) {
        return Err(RecurrenceError::Invalid);
    }
    let date = Date::new(year, month, day).map_err(|_| RecurrenceError::Invalid)?;
    Ok((date, hour, minute))
}

fn format_local(date: Date, hour: i8, minute: i8) -> String {
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}",
        date.year(),
        date.month(),
        date.day(),
        hour,
        minute
    )
}
