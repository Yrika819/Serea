# ADR-0027: Serea Calendar Recurrence Grammar V1

- Status: **Accepted**
- Date: 2026-10-06
- Architecture version: `serea-arch/2.1.0`
- Scheduler surface: `serea.scheduler/1` (unchanged)
- Decision owner: Serea

## Context

The Scheduler Protocol requires a Serea-owned recurrence grammar but did not
define its fields, validation, canonical representation, or local occurrence
identity. Implementing recurrence without those rules would make persisted
`recurrence_json` values ambiguous across versions and hosts.

## Decision

Adopt a closed structured JSON value named `CalendarRecurrenceV1`. Its only
`kind` values are `ONCE`, `DAILY`, and `WEEKLY`. It is not RFC 5545 RRULE, cron,
natural language, or model-interpreted prose. Serea defines recurrence
semantics. Jiff is limited to local-time and timezone/DST resolution.

Every value contains `version: "1"`, `kind`, and `anchor_local`, an exact local
civil minute in `YYYY-MM-DDTHH:MM` form without offset, seconds, fractional
part, or zone. Dates use the supported durable civil range 0000-01-01 through
9999-12-31 and must be real Gregorian dates. Hours are 00–23 and minutes 00–59.
The Schedule's `timezone` is authoritative and is not duplicated in the value.

| Kind | Exact fields | Semantics |
|---|---|---|
| `ONCE` | `version`, `kind`, `anchor_local` | One occurrence for the intended local label, then exhausted. |
| `DAILY` | Common fields plus integer `interval` ≥ 1 | Preserve anchor time; date difference from anchor modulo interval is zero. |
| `WEEKLY` | Common fields plus integer `interval` ≥ 1 and non-empty unique `weekdays` | ISO Monday-based weeks; anchor week is week zero; preserve anchor time; dates before anchor are excluded. |

There is no implicit interval default. Weekday values are `MO`, `TU`, `WE`,
`TH`, `FR`, `SA`, `SU`; canonical order is that sequence. Unknown kinds,
unknown or missing fields, duplicate JSON keys, duplicate weekdays, wrong
types, fractional numbers, malformed local labels, and unsupported fields are
rejected. V1 has no end rule, count, until, exception date, or monthly/yearly
form.

Persist a closed compact SCJ-1 JSON object: reject duplicate keys before
decoding, use integer-only numeric fields, normalize weekday order, and
serialize object keys in canonical order. Control flow uses the decoded value,
never raw JSON spelling. Equivalent inputs therefore persist in one form.

A calendar occurrence is owned by its `ScheduleId` and keyed by intended local
date/time plus the Schedule's IANA timezone. Its key is a deterministic
structured encoding, not a new global identifier prefix. Persist the intended
label, timezone, resolved UTC instant, evaluator version, and timezone-data
version. TZDB changes affect unresolved future occurrences only. Resolved,
pending, claimed, mapped, and processed occurrences retain their persisted
label and instant through schedule edits. An edited recurrence that yields the
same ScheduleId, label, and timezone reuses that occurrence identity.

For a DST gap, resolve to the first valid instant after the gap. For a fold,
emit one occurrence at the earlier UTC instant. The intended local label
remains the occurrence identity in both cases.

P3 pins Jiff `0.2.38` with `default-features = false` and features `std` and
`tzdb-bundle-always`. The exact Jiff dependency pins `jiff-tzdb 0.1.9`, which
bundles IANA data version `2026e`. Both crates declare Rust 1.70 and license
`Unlicense OR MIT`, compatible with the workspace MSRV and project license.
No host zoneinfo or concatenated TZDB feature is enabled, so Linux, macOS
Intel, and macOS arm64 use the same bundled source. The evaluator does not
define recurrence grammar or policy.

## Compatibility and version impact

This is a backward-compatible architecture-minor contract addition:
`serea-arch/2.0.0` → `serea-arch/2.1.0`. No Scheduler wire major is required;
`serea.scheduler/1` remains unchanged, as do event and device surfaces. The
crate dependency direction is unchanged. A runtime crate may depend on
protocol, storage, Event Bus, and Task Engine under the accepted Crate Map; no
lower-layer reverse edge is authorized.

## Consequences

Implementations use the existing migration 0002 recurrence and occurrence
columns; this contract adds no schema migration. Implementations must validate
and canonicalize recurrence before durable storage. Schedule edits cannot rewrite
durable occurrences. Additional
recurrence kinds require a later ADR and architecture change-control.
