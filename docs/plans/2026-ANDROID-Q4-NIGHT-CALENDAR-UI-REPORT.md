# Android Q4 Night Calendar UI report

## Scope and Git baseline

- Repository: `Yrika819/Serea`
- Feature branch: `codex/android-q4-night-calendar-ui-2026-10-21`
- Intended draft PR base: `codex/android-q4-abcdefg-integration-2026-10-20`
- Initial pinned HEAD: `072aa2b823523f7a92196299dd86cfb3509a78e3`
- PR #66 was verified OPEN and Draft at that same HEAD. Its own base remains `codex/android-m0-08-abcd-integration-2026-10-20`.
- Only the new Calendar UI package, its matching JVM tests, and this report are in scope.

Added files:

- `android/app/src/main/java/app/serea/android/calendar/ui/CalendarAgendaScreen.kt`
- `android/app/src/main/java/app/serea/android/calendar/ui/CalendarAgendaUiController.kt`
- `android/app/src/test/java/app/serea/android/calendar/ui/CalendarAgendaUiControllerTest.kt`
- `android/app/src/test/java/app/serea/android/calendar/ui/CalendarAgendaFormattingTest.kt`
- `android/app/src/test/java/app/serea/android/calendar/ui/CalendarAgendaUiSecurityTest.kt`
- `android/app/src/test/java/app/serea/android/calendar/ui/CalendarAgendaUiFake.kt`
- `docs/plans/2026-ANDROID-Q4-NIGHT-CALENDAR-UI-REPORT.md`

## Entry point and navigation handoff

`app.serea.android.calendar.ui.CalendarAgendaScreen` is the independent Compose Material 3 entry point. A host passes its back callback and may inject a `FakeCalendarProvider`, `Clock`, and display `ZoneId`:

```kotlin
CalendarAgendaScreen(
    onBack = { navController.popBackStack() },
    provider = FakeCalendarProvider(),
    clock = appClock,
    displayZone = ZoneId.of("Asia/Tokyo"),
)
```

The provider parameter is statically limited to `FakeCalendarProvider`; the screen does not initiate OAuth, select a real HTTP adapter, persist events, or expose any write operation. The screen labels itself `DEMO / 架空の予定` and `Mock Only · Google Calendar未接続`.

## UI state and behavior

The internal controller publishes `Loading`, `Agenda`, `Failure`, or `EventDetail` through a `StateFlow`. Agenda queries cover today, tomorrow, the next 30 days, and one explicitly selected date. Empty calendar lists and empty agendas have separate copy. Loading failures keep the selected route information so Retry can repeat the same agenda or detail request. Request cancellation plus a generation check prevents a late calendar A response from replacing calendar B. Detail reads reject keys outside the selected calendar before reaching a provider.

Typed authorization/read failures map to fixed Japanese strings. The UI never displays exception text. It distinguishes permission required, Mock Only requirement, denied, expired, missing scope, unknown timezone, timeout, rate limited, partial failure, unavailable/deleted-or-cancelled detail, invalid response, too many results, and generic unavailable. Retry is shown for failures; no retry path starts authorization.

Cancelled events are excluded from agenda results by the existing agenda service; a directly requested cancelled event detail is shown as cancelled. A deleted or unavailable event detail produces a generic unavailable message. Missing titles, descriptions, and locations use safe visible placeholders.

## Time and timezone rules

- `EventTime.AllDay` remains a pair of local dates. The exclusive end is shown as the last included date in the agenda and is identified as an exclusive boundary in details. No UTC conversion is applied to all-day dates.
- `EventTime.Timed` retains its `Instant` values and original `ZoneId`. Display timestamps use the injected display zone and include their UTC offset; detail separately names the original event timezone.
- Today, tomorrow, and selected-date query windows are half-open local-day ranges from `atStartOfDay(zone)` to the following date’s `atStartOfDay(zone)`. UTC and Tokyo boundary cases plus 23-hour and 25-hour New York days are covered.
- Timed display tests cover a DST gap and both sides of an overlap. Unknown timezones map to an explicit safe failure rather than a Tokyo fallback.
- The UI accepts an injected `Clock` and display `ZoneId`; tests use fixed time.

## JVM tests and reviews

The Calendar UI JVM suite contains 18 tests across the controller (13), formatting (3), and security (2) classes. It uses the existing `FakeCalendarProvider`, a Calendar UI fake for paging/auth/error injection, fixed clocks, and coroutine test dispatchers. Coverage includes today/tomorrow/upcoming/specified date, empty states, calendar selection and A-to-B races, pagination with duplicate and reverse-ordered responses, recurring stable identity, all-day exclusive end, UTC/Tokyo/DST boundaries, null text fields, cancelled events, permission and typed failures, retry/cancellation, and Calendar-to-model-egress policy denial.

Four independent read-only reviews were run and repeated after fixes:

- Domain: an initial wrong-package test import was reproduced by Gradle and removed. A later cross-calendar detail guard was initially placed in the agenda loader; the compiler reproduced the undefined-key failure, and the guard was moved to detail loading. Final review confirmed no new P0/P1 and preserved paging/deduplication and range semantics.
- Timezone: no P0/P1. Added all-day DST coverage; final review confirmed local dates, exclusive end, display zone, offsets, and DST boundaries.
- Android UX: fixed the blank specified-date input/query mismatch, made `onBack` mandatory so the visible close action cannot be a no-op, and disabled mode/date/calendar controls while Loading. Final review confirmed Back remains available and found no P0/P1. The integration lane should still verify the disabled states through Compose semantics.
- Security: an initial review found detail reads could target another calendar; a regression test now proves such a key is rejected before `getEvent`. Final review confirmed Fake-only entry, Mock source enforcement, Calendar origin/classification preservation, and no OAuth/HTTP/persistence/model-egress path, with no remaining P0/P1.

## Compose instrumentation handoff

No AndroidTest class was added to preserve PR #66’s strict 71-check/shared-workflow contract. The Navigation integration lane should add Compose instrumentation coverage for:

1. Render the demo/mock labels and agenda content; select today, tomorrow, upcoming, and an ISO specified date.
2. Select calendar A then B while A’s read is delayed; assert only B’s cards remain and A’s late response is ignored.
3. Tap an event, verify title/time/all-day/description/location/calendar/status/original timezone, then return using both the screen button and OS Back.
4. Render empty calendar, empty agenda, Loading, each representative safe error, Retry, cancelled detail, and missing event detail.
5. Recreate the activity in agenda and detail states; verify date, selected calendar, route, scroll behavior, and detail state restore.
6. Verify scrolling, long title/description wrapping, large font scale, TalkBack labels/actions, and light/dark themes.
7. During Loading, verify mode/date/calendar controls cannot trigger repeated reads; verify Back remains available.

## Verification record

Final verification after the Loading-state change:

| Check | Result |
|---|---|
| Calendar UI JVM suite | PASS — 18/18, 0 failures |
| Full Android JVM suite | PASS — 184/184, 0 failures, errors, or skips |
| `:app:assembleDebug` | PASS |
| `:app:assembleRelease` | PASS |
| `:app:assembleDebugAndroidTest` | PASS — APK compile only |
| `:app:lint` | PASS |
| AndroidTest runtime / emulator | NOT_RUN — no device was attached |
| Git diff check | PASS — staged diff clean before commit |
| Git identity / hooks | PASS — `Codex <noreply@github.com>` accepted by the commit guard; no custom hooks path is configured |
| PR workflow CI | PENDING — checks had started for draft PR #67 at last inspection; a pushed report update will trigger another run |

Gradle emitted a non-fatal warning that it could not strip `libandroidx.graphics.path.so`; it packaged the library unchanged. No workflow, signing, manifest, or Gradle file was changed.

## Remaining integration and beta blockers

- The screen is intentionally not wired from `MainActivity.kt`; the Navigation lane must add a route and supply `onBack`.
- Compose instrumentation and emulator rendering/accessibility checks are deferred to that lane. No existing real-data device is to be used.
- The integration should retain the `FakeCalendarProvider` until a separate reviewed product decision changes the mock-only boundary.
