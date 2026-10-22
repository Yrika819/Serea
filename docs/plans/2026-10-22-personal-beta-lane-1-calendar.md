# Personal Beta Lane 1: Calendar provider boundary

## State

Prepared for integration review. No live Google account or Google API was used. The real provider is not selected by default, this lane is not wired into the Calendar UI, and this work does not establish Beta readiness.

## Review basis

Reviewed the current integration candidates and open PR stack (#82–#86), the Calendar read domain and Fake provider, the existing GoogleCalendarHttpReadPort, the GoogleCalendarAuthorizationAdapter, the ADRs governing approval and provider boundaries, SECURITY.md, and docs/plans/Serea-high-privilege-security-gates.md. PR #86 is the branch base for this lane. Existing decisions keep real provider effects disabled until separate authorization and security gates are met.

## Implementation

CalendarProviderSessions supplies a composition boundary for CalendarReadPort and CalendarAuthorizationPort:

- openDefault() always creates a Fake session.
- openGoogleReadOnly() returns Fake unless an explicit owner enable flag is passed and the injected gateway is a Play Services gateway.
- The enabled path binds the existing authorization adapter and HTTP read adapter into one session.
- Replacing or closing the session invalidates its old ports and cancels active coroutine jobs.
- This is an injection boundary only; no production UI or app composition root opts into it.

The existing HTTP adapter remains read-only. It requests only the Calendar scopes needed by each operation, fails closed on missing or expired authorization, bounds responses, rejects redirects and retries, and maps provider failures to typed errors without returning provider bodies.

## Security contract

- Default provider: Fake. Real HTTP requires a caller to pass the explicit owner gate and a PLAY_SERVICES gateway.
- Credentials are held in a short-lived authorization lease and are not stored, logged, or returned to domain/UI code.
- Reads only; no create, update, delete, or write endpoint.
- Session replacement/close revokes the old read boundary and cancels reads in flight.
- Google OAuth/app verification and real-account testing remain separate owner decisions. No Security Gate was relaxed.

## Validation

CalendarProviderSessionsTest uses a Fake gateway and MockWebServer to cover default Fake selection, Mock-only authorization rejection, explicit enabled injection, denial, expiry, and session replacement. GoogleCalendarHttpReadPortTest covers success, missing scopes, 401/403, transport failure, timeout, cancellation, pagination, response bounds, and calendar/event timezones. No request reaches Google.

Local checks passed: targeted testDebugUnitTest (44 tests) for CalendarProviderSessionsTest, GoogleCalendarHttpReadPortTest and GoogleAuthorizationTest; lintDebug.

## Owner decisions

- Approve any Google OAuth app configuration, consent text, account choice, scopes and privacy disclosures before connecting a real account.
- Approve the real Calendar activation gate after independent security review and revocation/recovery review.

## Integration

Keep this as a bridge PR based on the latest security-evidence base. After UI repair PRs #82–#85 and base PR #86 are resolved, integrate the adapter/session boundary before a separate Calendar UI integration. The UI integration must keep the Fake path as the default and add user-visible account/scope/revocation behavior. Do not call this Beta Ready until the final integration HEAD contains all reviewed work.
