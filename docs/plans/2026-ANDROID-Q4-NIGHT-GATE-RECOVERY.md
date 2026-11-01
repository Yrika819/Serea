# Android Q4 Night Gate Recovery

This repair lane is based on the pinned integration source and keeps the current Google Calendar permission contract unchanged pending an owner decision.

## Calendar scope contract

`CalendarPermissionPolicy.requiredScopes(listCalendars = true)` currently treats Events read-only as the baseline and adds CalendarList read-only when calendar listing is requested. `mayListCalendars` also requires event-read authorization, so a CalendarList-only grant is rejected by the current policy and by the CalendarList endpoint guard.

| Granted scopes | Current Events read | Current calendar listing | Endpoint-minimum hypothetical: Events read | Endpoint-minimum hypothetical: calendar listing |
| --- | --- | --- | --- | --- |
| Events only | Allowed | Denied | Allowed | Denied |
| CalendarList only | Denied | Denied | Denied | Allowed |
| Both | Allowed | Allowed | Allowed | Allowed |
| None | Denied | Denied | Denied | Denied |

The endpoint-minimum column assumes each endpoint authorizes only its own read-only scope: Events endpoints require `calendar.events.readonly`; CalendarList endpoints require `calendar.calendarlist.readonly`. Under that contract, CalendarList-only would be accepted for listing. Changing to that model changes the authorization boundary and requires a separate owner decision. This repair adds contract coverage and leaves production policy unchanged.

## Verification evidence

The exact source commit, tree, repair branch base, test counts, API 26/API 35 emulator results, Process Death results, required gate status, review findings, and final merge readiness are recorded in the repair PR description and workflow evidence. Any API with no verified device run is reported as `NOT_RUN`; an uploaded artifact is not itself a passing test result.
