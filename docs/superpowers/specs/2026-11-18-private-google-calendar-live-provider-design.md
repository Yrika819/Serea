# Private Primary Google Calendar Provider Wiring — Design

**Status:** Owner-approved implementation direction; implementation is not yet approved for real Google access.  
**Baseline:** `Yrika819/Serea` main `077246aeab909928fb57316f1f3aefc6be69c0b3`, Tree `0548dba87b34d7b10e5d6c5b591442156abc7b08`.  
**Scope:** H4-D1/D2 Private authorization-to-provider wiring and deterministic MockWebServer validation only.

## Goal

Wire the existing Private Google authorization UI to a Primary-only, read-only Calendar provider path while preserving Fake as the product default. Prove the integration using synthetic authorization and loopback HTTP, without authorizing a real Google account or enabling real Google HTTP in any product APK.

## Existing components to reuse

- `PrivateGoogleAuthorizationViewModel` already binds consent results to account/session generations and closes provider sessions on account changes, disconnect, failed authorization and ViewModel clearing.
- `PrivateGoogleCalendarComposition` currently validates the Calendar event-read scope and always opens `openPrimaryFake()`.
- `CalendarProviderSessions.openPrimaryGoogleReadOnly()` already selects the Primary-only Google transport and owns request cancellation, provider replacement, grant invalidation and 401 session invalidation.
- `GoogleCalendarAuthorizationAdapter` supplies status and short-lived token leases while rejecting non-Primary scope requests.
- `GoogleCalendarHttpReadPort` already limits the Primary path to `calendarId=primary`, `calendar.events.readonly`, GET-only calls, bounded pagination/retries, no redirects and no CalendarList access.
- `CalendarAgendaScreen` already distinguishes Fake/demo data from Google-origin read-only data based on provider authorization state.
- Existing provider, transport, authorization-session and Demo loopback tests cover most low-level contract and failure behavior. The new E2E coverage will focus on the missing Private UI → provider → agenda connection.

## Provider-selection and gate contract

1. Public, Debug/Demo, and Beta product variants remain Fake-only.
2. Private Beta authorization remains independently gated by the existing `PRIVATE_GOOGLE_AUTH_UI_ENABLED` and `OWNER_PRIVATE_BETA_GOOGLE_OAUTH_ENABLED` fields.
3. Add a separate `PRIVATE_GOOGLE_CALENDAR_HTTP_ENABLED` BuildConfig value that is explicitly `false` in every product variant in this change. It has no environment-variable, Gradle-property, UI, persisted-preference, or runtime user toggle. Turning it on requires a separate Owner-approved code change.
4. Product composition passes the conjunction of the Private UI variant and the HTTP gate to the existing `CalendarProviderSessions` gate. Since the HTTP gate is always false here, successful OAuth still opens the Primary Fake provider.
5. A narrow ViewModel composition-factory seam allows Android tests to inject a test-only composition that uses `CalendarProviderSessions(googleHttpEnabledByOwner=true)`, a synthetic Play-Services-mode gateway, a test OkHttp client and a MockWebServer base URL. These test values and implementations live only in the `isolated` test source set and are never constructed by the product ViewModel factory. Explicit Live composition overrides are rejected unless `BuildConfig.BUILD_TYPE == "isolated"`, so product variants cannot use the test activation path.
6. The product Live gate is hardcoded false in this change. The default non-Live selection therefore opens Fake. In an isolated test-only Live selection (and any future separately Owner-approved product activation after an explicit code change), composition must fail closed before token/network work if the Private variant, independent Live gate, real-gateway mode, current authorization session or exact Primary event-read scope requirement is not satisfied. Once Live is selected, authorization or HTTP failures remain failures and never fall back to Fake data.
7. The only production operation represented is Primary Calendar event reading. CalendarList, write operations, Gmail scopes and token persistence remain unsupported.

## UI and lifecycle

- Keep the current Private UI Fake label and Fake fixtures for every product build in this phase, including OAuth-authorized but HTTP-disabled Private Beta.
- Route an explicitly opened Private Calendar session through the existing `CalendarAgendaScreen`; its Google label is valid only when the session is actually Google-backed. A Google request error is displayed as an error, not as a Fake agenda.
- Preserve generation-bound authorization results and close the previous provider before replacing it. Account change, local disconnect, Google grant revoke, 401 invalidation, request cancellation, process death and ViewModel clearing must not retain or revive an old session.
- Google-origin events remain excluded from automatic Cloud AI dispatch. Do not weaken Egress → Usage → Dispatch checks or add an automatic chat handoff.

## Test strategy

- Add regression tests first for the static-false product HTTP gate, Private OAuth-authorized Fake default, explicit test-only Live selection, exact scope and gateway/session preconditions, and no-Fake-fallback failure behavior.
- Add a Private authorization-panel-to-agenda MockWebServer E2E using a synthetic Gateway. Cover Primary event listing, pagination, event details, all-day/timezone conversion, empty results, manual refresh and normal close.
- Reuse and run existing adversarial transport/session tests for non-Primary and CalendarList preflight rejection, missing scope, invalid/expired token, 401/403/429, timeout, malformed JSON, repeated page token, account/session replacement, cancellation, grant invalidation and stale-session races. Add focused cases only where the integration boundary is not already asserted.
- Preserve the existing isolated Android runner and Private Beta CI packaging restriction. Do not add OAuth scopes, reduce test counts, disable required checks, or add test credentials/data to CI artifacts.
- Run focused JVM tests, Android instrumentation where the host supports it, variant-isolation checks, required CI, `git diff --check`, documentation validation and a secrets scan. Record unrun tests as `NOT_RUN` or `HOST_INFRA`, not as passed.

## Future Owner-controlled activation prerequisites

This change does not configure Google Cloud, perform OAuth, or enable product HTTP. A later, separately approved change must first verify the selected Cloud project and audience, Calendar API enablement, exact Android OAuth package/signing-certificate identity, allowed `calendar.events.readonly` scope and Google classification/verification requirements. It must then obtain separate Owner approval to change the currently hardcoded-off Private HTTP gate and separately authorize a real account/API test. The exact checklist is delivered in `docs/security/private-google-calendar-live-activation-checklist.md`.
