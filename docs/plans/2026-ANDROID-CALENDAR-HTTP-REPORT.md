# Calendar read-only HTTP adapter report

## Scope

Implemented lane B on `codex/android-q4-calendar-http-2026-10-20`, based on the pinned PR #58 head `239aaa181cdcfea756cf6c9ebb5eca9194caf7cf`. The adapter is confined to the new `calendar/transport` source and JVM test directories. Existing Calendar domain, parser, policy, app wiring, dependency files, workflows, manifest, and Git identity/security code were not edited.

## Implementation

- Added `GoogleCalendarHttpReadPort`, a read-only OkHttp adapter for calendar list, calendar get, event get, and paged event list operations. It constructs requests only beneath the fixed `https://www.googleapis.com/calendar/v3/` endpoint, disables redirects and automatic connection retries, and sets 5 second connect, 10 second read, and 20 second total call timeouts.
- Calendar and event parsing delegates to the existing `GoogleCalendarEventParser`; calendar-list metadata is parsed locally. Event list requests use `[timeMin, timeMax)`, `singleEvents=true`, `showDeleted=true`, and `orderBy=startTime`. Page tokens are bounded and encoded by OkHttp. The existing agenda service enforces aggregate event/page limits, rejects repeated page tokens, and deduplicates recurrence instances.
- Added `CalendarAccessTokenPort`; each request asks the host authorization layer for a short-lived token. The adapter requires `AUTHORIZED` state and the endpoint's read-only scope. Missing, expired, denied, or mock-only authorization does not reach Google. Token-provider exceptions are reduced to a generic permission failure. OAuth acquisition, refresh, and revocation remain outside this adapter.
- Responses are capped at the existing parser's 512 KiB limit; error bodies are read only up to 16 KiB for status classification and are not included in failures. HTTP errors map to the existing `CalendarReadFailure` codes. No event write method or cache was added. Parsed events retain `DataOrigin.CALENDAR` and `PERSONAL` classification through the existing parser.
- A package-internal loopback-only base URL seam supports MockWebServer. It cannot redirect the production client to arbitrary hosts.

## Verification

- `git diff --check`: PASS.
- Requested JVM unit command: `./gradlew :app:testDebugUnitTest --tests 'app.serea.android.calendar.transport.GoogleCalendarHttpReadPortTest' --tests 'app.serea.android.calendar.CalendarDomainParserTest' --tests 'app.serea.android.calendar.CalendarAgendaTest' --tests 'app.serea.android.calendar.CalendarSecurityTest'` — NOT_RUN. Gradle wrapper could not download Gradle 8.11.1 because the distribution request failed with `java.net.ConnectException: Connection refused`; no Gradle tasks started.
- Android build and lint: NOT_RUN for the same unavailable Gradle distribution.
- New transport suite contains 11 MockWebServer test methods for URLs, headers, pagination, authorization/token handling, status mapping, redirects, malformed/oversized responses, timeout, cancellation, and repeated page tokens. Execution status: NOT_RUN; there are no real Google requests.
- Existing Calendar regression suites: NOT_RUN. No CI workflow was triggered; CI Run ID and final-head status are unavailable.
- Real Google OAuth, API access, personal calendar data, and device/ADB verification: NOT_RUN.

## Integration notes

- The host authorization implementation should implement `CalendarAuthorizationPort` and `CalendarAccessTokenPort`, return only a short-lived access token after user-approved authorization, refresh or revoke it in that owner-controlled layer, and report missing scopes accurately. Required read scopes remain `calendar.events.readonly` and, for calendar listing, `calendar.calendarlist.readonly`.
- No production wiring was added, so the adapter is not active in the app. OAuth/UI integration and a workflow base-branch filter update remain owner-lane dependencies. The current `.github/workflows/android-calendar-readonly.yml` triggers only for PRs targeting `codex/android-m0-04-integration-candidate-2026-10-20`, while this lane's required PR base is `codex/android-m0-08-abcd-integration-2026-10-20`; its path filter does include both new source and test directories, but the branch filter currently prevents an automatic run for this PR. No Gradle dependency changes were needed; OkHttp, Gson, coroutines, and MockWebServer are already declared.
- Event timezone fallback currently follows the existing parser's supplied zone (`UTC` in this transport). If owner integration requires calendar-specific fallback for events lacking timezone fields, the domain seam needs a timezone input or the transport needs an authorized calendar timezone source.
- API calls are intentionally not automatically retried. A 401 maps to permission denied, 403 distinguishes missing scope/quota when the bounded error body identifies the reason, 404 to not found, 410 to expired sync token, 429 to rate limited, and 5xx to unavailable.

## Git and delivery

- Starting PR #58 was verified OPEN and Draft with head `239aaa181cdcfea756cf6c9ebb5eca9194caf7cf`; the specified base branch exists at that same SHA. The worktree was clean before starting.
- Repository-local Codex identity settings and versioned hooks were configured according to `docs/git-safety.md`; preflight and reachable-history identity checks passed before edits.
- Final branch/commit, Git tree, push, and Draft PR details are recorded in the task handoff after delivery.
