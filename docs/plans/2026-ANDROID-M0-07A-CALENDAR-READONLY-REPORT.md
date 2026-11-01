# AND-M0-07A — Android Calendar Read-only Foundation

Date: 2026-10-20
Status: implementation candidate; real Google integration is not enabled.
Base: `94492d281fe0ee583744c932259c3525d5c34f86` (`codex/android-m0-04-integration-candidate-2026-10-20`)

## Current analysis and boundaries

The Android app already defines `DataOrigin.CALENDAR`. Its existing `ChatEgressGate` only accepts `USER_AUTHORED` and `MODEL_OUTPUT`; Calendar-origin data is denied. ADR-0044 and ADR-0045 both remain **PROPOSED / NOT RATIFIED**. ADR-0027 defines Serea's closed Scheduler recurrence grammar, so this Android package does not use it to parse Google RRULE values.

This change adds a UI-independent package at `android/app/src/main/java/app/serea/android/calendar/`. It does not edit `MainActivity.kt`, `ChatContext.kt`, `SecurityPorts.kt`, `EncryptedStorage.kt`, the manifest, Room, or task/chat/provider code. It adds no Calendar network client, OAuth SDK, Android permission, token store, cache, database migration, or event write operation.

**No Google account was connected. OAuth was not initiated, no token was received or stored, and no Google Calendar API request was made. All successful reads described here are from fictional in-memory fixtures.**

## Android Calendar architecture

`CalendarReadPort` exposes `listCalendars`, `getCalendar`, `getEvent`, and one-page `listEventsPage`. It contains no create, update, delete, move, attendee, or permission mutation method. `CalendarAuthorizationPort` reports authorization state without exposing token APIs. `PermissionCheckedCalendarReadPort` checks read state and the appropriate scope before delegating.

`CalendarAgendaService` reads every page from every listed calendar, enforces page and event limits, deduplicates stable identities, filters by interval overlap, and sorts by UTC instant and stable identity. It throws on a page failure, timeout, repeated token, or limit breach, so the caller never gets a partial list labelled complete. Cancellation remains a coroutine cancellation. Convenience methods use an injected `Clock` and device `ZoneId` for today and tomorrow; they use local date boundaries rather than adding 24 hours. Next-upcoming results are bounded to a caller-selected horizon and limit.

The package has no ViewModel or Compose dependency. Android lifecycle instrumentation is not applicable to this isolated P0 domain seam; no Calendar UI is presented.

## Domain model and identity

The model retains Calendar ID, display name, IANA timezone, optional ARGB color, visibility, and read state. An event retains the `(CalendarId, EventId)` pair, optional title/description/location, time, all-day semantics, status, recurrence parent ID, original recurrence start, update time, visibility, source, `DataOrigin.CALENDAR`, and `DataClassification.PERSONAL` by default.

Google Event IDs are not treated as globally unique. Expanded recurrence instances have a stable identity composed from the owning Calendar ID, recurring event ID, and original start (date or instant plus zone), so a moved instance keeps its identity. `originalStartTime` is represented as a single date or instant; the model does not invent an end or duration for it. Cancelled events can omit times and details. The agenda omits cancelled events from display, while the parser preserves the cancellation tombstone and recurrence metadata.

## Google Event JSON mapping

`GoogleCalendarEventParser` reads only the required v3 subset: `id`, `summary`, `description`, `location`, `status`, `start`, `end`, `recurringEventId`, `originalStartTime`, `updated`, and (at page level) `items` and `nextPageToken`. It enforces a 512 KiB UTF-8 payload limit, strict JSON parsing, identifier bounds, date/time parsing, timezone validation, increasing interval validation, and required-field rules. Unknown fields (including future response metadata) are ignored. Unknown event status maps to `UNKNOWN`; absent event status also remains unknown. Invalid JSON, malformed date values, invalid timezone IDs, conflicting offset/zone pairs, invalid IDs, and non-cancelled events missing start/end fail closed as `INVALID_RESPONSE`.

The parser ignores `nextSyncToken` during short-lived agenda reads. It does not implement event RRULE parsing.

## All-day, timezone, and range semantics

An all-day event stores `start.date` and the exclusive `end.date`; for 2026-10-20 only, the API interval is 2026-10-20 through (exclusive) 2026-10-21. Its IANA timezone is the supplied event timezone or the calendar timezone. Range intersection converts those date boundaries at the calendar/event zone's local start of day, so DST days may be shorter or longer than 24 hours.

Timed events parse RFC3339 offset date-times to `Instant`. A provided IANA timezone is retained and its rules must allow the supplied offset for that local date/time; otherwise parsing is rejected. With no timezone ID, the explicit RFC3339 offset is retained as a fixed-offset zone. Sorting and overlap use instants, not local text. Today/tomorrow use the injected device zone. The parser and tests cover Tokyo, New York, London, UTC, Apia/date-line handling, DST start and end, and a same-instant event from different Calendars.

Date ranges are half-open `[start, end)` and include events whose interval overlaps the query. This includes a multi-day event in each intersecting day and excludes an event ending exactly at range start or beginning exactly at range end.

## Recurrence, exceptions, and pagination

The integration contract expects Google Calendar API expanded instances (for a future `events.list`, `singleEvents=true`, with a bounded time range). This code does not calculate RRULE occurrences. It preserves `recurringEventId` and `originalStartTime` for normal, rescheduled, and cancelled exception instances and deduplicates using recurrence identity.

The API adapter seam exposes opaque `pageToken` input and `nextPageToken` output. Aggregation permits configurable maximum pages/events, empty intermediate pages, and stable deduplication. A future HTTP adapter must request deleted entries where needed to observe cancellation exceptions, propagate cancellation/timeouts, and fail the whole agenda on a page error. No partial cache is written.

The Calendar API documents `nextSyncToken` on the final list page. Incremental sync requests must preserve initial query parameters; `timeMin`, `timeMax`, `updatedMin`, `orderBy`, `q`, and extended-property filters cannot be combined with `syncToken`. An expired sync token returns HTTP 410 GONE. This milestone recognizes `SYNC_TOKEN_EXPIRED` but does not persist tokens, synchronize, clear cached data, or implement the required full-resync behavior. Short-term range Agenda and future incremental sync remain separate request contracts.

## Fake provider

`FakeCalendarProvider` is deterministic, in-memory, and network-free. Its explicitly fictional fixtures include timed and all-day events, a multi-day all-day event, same-instant events in different calendars, expanded recurring and moved instances, a cancelled exception, a private event, a missing title, Asia/Tokyo / America/New_York / Pacific/Apia calendars, DST boundary examples, and 245 paged synthetic events. It supports configurable page size, empty pages, repeated events, order reversal, permission failure, page failure, timeout, cancellation, and delay. It also supports empty calendars and missing-calendar/event failures. It reports `MOCK_ONLY`, never `AUTHORIZED`, and has no credential input or storage.

## OAuth authorization plan — no OAuth implementation

Official documentation was rechecked on 2026-10-20:

- [Calendar API authorization scopes](https://developers.google.com/workspace/calendar/api/auth)
- [Events resource](https://developers.google.com/workspace/calendar/api/v3/reference/events)
- [events.list](https://developers.google.com/workspace/calendar/api/v3/reference/events/list)
- [calendarList.list](https://developers.google.com/workspace/calendar/api/v3/reference/calendarList/list)
- [Android AuthorizationClient](https://developers.google.com/android/reference/com/google/android/gms/auth/api/identity/AuthorizationClient)

The least-scope plan is `https://www.googleapis.com/auth/calendar.events.readonly` for event reads. Add `https://www.googleapis.com/auth/calendar.calendarlist.readonly` only if calendar discovery/listing is enabled; the agenda needs a selected set of Calendar IDs, so a later product decision can choose discovery or an explicit single Calendar. Do not request the broad `calendar.readonly` scope by default. `calendarList.list` accepts the calendar-list read-only scope; `events.list` accepts the events read-only scope.

If separately approved, use Google Play services `AuthorizationClient` with an explicit requested-scope list and an Android user-mediated account/consent flow. Handle any resolution intent in the foreground, distinguish not configured, not connected, authorization required, authorized, denied, revoked, expired, error, and mock-only states, and never render `MOCK_ONLY` as connected. No OAuth Client, Cloud Project, consent screen, account selection, or authorization request was created here.

Before production, register the Android package and release signing certificate with the chosen OAuth client configuration, configure the consent screen/audience, verify test and release account behavior, and complete Google verification for scopes and distribution if required. Public use of scopes permitting access to user data can require OAuth verification. Release channel and privacy disclosures remain Owner decisions.

Access tokens must not enter Domain objects, ViewModels, SavedState, analytics, logs, crash reports, test artifacts, or GitHub artifacts. A future adapter should keep access tokens transient/in-memory for the shortest possible request lifetime; it must define expiry and reauthorization. Use AuthorizationClient's `clearToken`/`revokeAccess` where the approved flow requires them, then invalidate access to any app-held Calendar data. Android Keystore is not a reason to persist Google tokens by default. No token renewal or revocation was implemented.

Future API error handling should distinguish unauthenticated/authorization-required, denied/missing scope, missing Calendar, quota/rate limit (including 403/429 reasons), transient service/network failure, malformed response, and 410 expired sync token. Retry/backoff with jitter and quota behavior require a later adapter policy. No API request was made, so quota behavior is not measured here.

## Data classification and AI egress boundary

Events are initialized as `DataOrigin.CALENDAR` and `DataClassification.PERSONAL`, including missing-title and private events. They cannot be promoted automatically to `PUBLIC`. `CalendarPermissionPolicy.canSendToExternalModel` rejects Calendar origin. The existing `SecurityPorts.kt` and `ChatEgressGate` were not changed; their existing origin allowlist rejects Calendar data. No Calendar object is inserted into ChatContext, ModelChatRequest, provider payloads, Usage Ledger, AI tool arguments, logging, or analytics. No external model call was made.

Any future AI integration needs a separate explicit user intent, selected fields, trusted minimization, classification review, and an Egress Gate approval. Read authorization alone does not authorize AI egress.

## Cache, retention, and revoke

Agenda results stay in memory. There is no event cache, Room table, migration, token persistence, account ID persistence, plaintext event storage, or Calendar diagnostic logging. On future disconnect/revoke, stop Calendar reads immediately and invalidate any retained Calendar data access. This milestone has no persistent Calendar data to delete; it does not claim a deletion flow exists.

## Tests and CI evidence

Automated tests added under `android/app/src/test/java/app/serea/android/calendar/` cover model identity/classification, parser mapping and rejection, all-day exclusivity, timezones/DST/date-line, injected-clock Today/Tomorrow, range boundaries, multi-calendar ordering, recurrence identity, duplicate handling, empty pages, pagination/event limits, timeout, cancellation, permission denial, malformed fake pages, no partial return on page failure, narrow scopes, and Calendar egress denial. The dedicated workflow ran all 22 Calendar test methods successfully on final implementation SHA `914d35eb7631e981df93a345a2ac58e399d8ba42`.

Local Gradle execution was attempted. It first found an invalid preconfigured `JAVA_HOME`; rerunning with installed Java 21 downloaded Gradle but stopped because this Codex Cloud environment has no Android SDK at `/workspace/android-sdk` and no `android/local.properties`. Thus local Gradle/Android build/lint and API 26/35 instrumentation are **NOT_RUN/BLOCKED**, not PASS. The Android lifecycle instrumentation cases are not applicable to this UI-free seam.

| Verification | Result | Run ID / evidence |
|---|---|---|
| Calendar JVM unit/parser/agenda/security (22 tests) | PASS | [39373831346](https://github.com/Yrika819/Serea/actions/runs/39373831346), final implementation SHA `914d35e` |
| Android build, lint, dependency audit, unit, integration, security | PASS | [39373831330](https://github.com/Yrika819/Serea/actions/runs/39373831330), final implementation SHA `914d35e` |
| API 26 / API 35 instrumentation, process death, Android Emulator Required Gate | PASS | Android run `39373831330`; API 26 and 35 emulator job `118470644273`, required gate `118472916518` |
| Fast CI | PASS | [39373831338](https://github.com/Yrika819/Serea/actions/runs/39373831338), final implementation SHA `914d35e` |
| Full CI | QUEUED overall; Linux MSRV, Linux stable, and Windows synthetic job PASS; macOS jobs queued | [39373831400](https://github.com/Yrika819/Serea/actions/runs/39373831400) |
| Native platform validation | QUEUED overall; Linux, Windows x86_64 and macOS x86_64 jobs passed; Windows ARM64 jobs in progress; other macOS jobs queued | [39373831356](https://github.com/Yrika819/Serea/actions/runs/39373831356) |
| SQLite portability | QUEUED overall; Intel DB production passed; macOS ARM64 jobs queued | [39373831376](https://github.com/Yrika819/Serea/actions/runs/39373831376) |
| Windows Hooks compatibility | QUEUED overall; Ubuntu and Windows jobs passed; macOS queued | [39373831333](https://github.com/Yrika819/Serea/actions/runs/39373831333) |
| Commit Identity | PASS: repository-local identity, preflight before commits, post-commit identity scan | `python tools/git_preflight.py --mode commit`; `tools/check_commit_identity.py` on bounded PR range |

Those CI runs target code SHA `914d35eb7631e981df93a345a2ac58e399d8ba42`. The report-only follow-up commit will trigger a fresh workflow set; its runs must be checked separately before final approval. Local Android Gradle could not run because the cloud environment has no Android SDK, so the remote Android run is the source of Android build/emulator evidence.

A new `android-calendar-readonly.yml` workflow runs only Calendar unit/mock tests and does not replace, skip, weaken, or alter the existing Android Emulator Required Gate. Its duplicate `push` trigger was removed so publishing the branch does not unnecessarily start a second Calendar runner alongside pull request CI.

## A/B/C integration and owner decisions

The change is isolated to the new `calendar/` source/test package, the dedicated workflow, and this report. It does not merge A/B/C branches. No shared application, task, ChatViewModel, encrypted-storage, provider, or Gate file is edited. Since the protected PR #50 base is the only common integration parent, draft PR [#57](https://github.com/Yrika819/Serea/pull/57) is targeted to `codex/android-m0-04-integration-candidate-2026-10-20`, not `main`. Base SHA is `94492d281fe0ee583744c932259c3525d5c34f86`; latest published implementation SHA at the time of this evidence update is `914d35eb7631e981df93a345a2ac58e399d8ba42`.

Owner decisions before OAuth: whether to support calendar discovery or selected Calendar IDs; release channel and OAuth verification path; consent/privacy disclosure; exact revoke and reauthorization UX; and whether event title/location/description may ever enter an explicitly approved AI preparation path. Calendar-to-task mapping and Calendar-to-AI context remain separate later changes.

## Unimplemented work and Beta follow-up

- Real Google authorization, API transport, account picker, token lifecycle, quota/backoff, and integration tests.
- Persistent sync, `syncToken` storage, 410 recovery, cache invalidation, and event retention policy.
- Calendar UI, user calendar selection, attendee/reminder handling, notifications, task linking, writes, and AI context integration.
- Recheck all workflows on the report-only final commit SHA; runs listed above test implementation SHA `914d35e`.

The result is a Mock read-only foundation only. It must not be described as connected to Google Calendar.
