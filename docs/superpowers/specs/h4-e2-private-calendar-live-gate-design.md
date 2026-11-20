# H4-E2 design — Private Calendar Live Gate

**Status:** Proposal for H4-E2; not implemented or enabled by H4-E1.
**Baseline audited:** `487ca82a95e460baa4b7e451bac96cc1032a2212`.

## Objective and recommendation

Add the smallest safe product route for Owner-approved Private Calendar reads: a **dedicated Private-only compile-time Live HTTP gate**, independent from OAuth authorization. Keep Public, Beta, Debug/Demo, isolated product compositions, and all default builds incapable of Google product HTTP. The gate’s checked-in/default value remains false; enabling it requires a separately approved, dedicated Private build configuration and review. No environment variable, Gradle property, remote flag, UI switch, or runtime preference may unlock it.

OAuth consent and HTTP permission are distinct capabilities. OAuth being enabled or consent succeeding must never imply HTTP is enabled. Once Live is selected, any Live failure is an error—not an implicit Fake fallback.

## Gate-on provider-selection truth table

This table is the required provider-selection contract. “Fail visibly” means expose a sanitized, actionable error state before constructing a provider; it does not mean select Fake.

| Private HTTP gate | OAuth/session state | Gateway | Required result |
|---|---|---|---|
| OFF | Any state | Any state | Construct the default, explicitly labeled Private Fake provider. OAuth/session availability must not select Live. |
| ON | OAuth unavailable | Any gateway | Fail visibly before provider creation; do not choose Fake. |
| ON | OAuth/session state missing, invalid, expired, revoked, stale, wrong-account, wrong-scope, or otherwise ineligible | Any gateway | Fail visibly before provider creation; do not choose Fake. |
| ON | OAuth/session state valid | Missing, wrong, or ineligible gateway | Fail visibly before provider creation; do not choose Fake. |
| ON | Valid current authorization session with the exact owner account and events-readonly scope | Eligible Play Services Google authorization gateway | Construct Live. Live request/session/transport errors remain visible errors and never fall back to Fake. |


## Options considered

| Option | Assessment |
|---|---|
| Dedicated compile-time Private build field, default false, guarded by variant identity | **Recommended.** Explicit auditable product boundary; no runtime unlock; preserves current false default. Still requires CI assertions for all variants and careful release artifact control. |
| Runtime Owner toggle/environment/property | Rejected. Can unlock from an arbitrary runtime/build environment and is difficult to prove absent in distributed variants. |
| Reuse isolated MockWebServer gate for product | Rejected. Test-only loopback exceptions must not enter product composition or permit caller-chosen URLs. |
| Enable HTTP whenever OAuth is enabled | Rejected. Collapses separate consent and network permissions and violates the current safety boundary. |

## Intended changes (H4-E2 only, subject to review)

1. In `android/app/build.gradle.kts`, introduce a separately named Private Live HTTP build field with literal `false` for defaults, Public Beta, Beta, Debug/Demo and isolated variants. Only an explicitly selected Private product build may have a compile-time `true` value, gated by exact Private application ID. Keep OAuth input independently controlled. Do not change either field in H4-E1.
2. In `PrivateGoogleCalendarComposition` / provider session construction, require both the dedicated Private HTTP compile-time permission and the independent OAuth/session authorization. Fail configuration closed if the Live gate is on for any non-Private application ID. Keep product endpoint fixed to Google Calendar HTTPS; no URL override.
3. Keep loopback HTTP/MockWebServer allowance in test-only composition and test source sets. Add a regression assertion proving it cannot be passed into product composition.
4. Revalidate on every request the exact owner account binding, active authorization session/generation, exact events-readonly scope, unrevoked/current grant, and `primary` calendar ID. Reject CalendarList, non-primary IDs, and all writes before auth token acquisition/network transmission. Preserve current Google HTTPS host/path constraints, GET-only API, disabled redirects and bounded retry behavior.
5. Never log access tokens, refresh tokens, authorization codes, account identifiers, event titles/content, or raw private response bodies. Return sanitized errors only.
6. Google-origin calendar data remains local and must not be automatically egressed to Cloud AI. Any future AI use requires a separate policy/authorization lane and explicit data egress review.

## Fail-closed lifecycle

401, 403 (including denied scope), 429/rate-limit exhaustion, transport disconnect, revoked/expired grant, explicit disconnect, account switch, request cancellation, session generation mismatch, and process death must invalidate or reject the Live session and surface an actionable failure. Do not silently display Fake data as if it were Live. A user may explicitly choose a separately labeled Fake/demo session after the failed Live state is cleared; this is not automatic fallback. Reauthorization creates a new validated session; old session/token handles must not remain usable.

Process death must not restore an authenticated Live session from stale UI state. Persist no bearer token or authorization code in ordinary saved state/logs; require session revalidation/reauthorization according to the existing credential-safe design.

## Verification plan and acceptance tests

- BuildConfig/variant matrix: Public, Beta, Debug/Demo, isolated and default all assert HTTP false. Private default false. OAuth false and HTTP false are independently asserted. Only an explicitly approved Private test configuration can compile HTTP true.
- Negative construction tests: HTTP true with non-Private app ID fails closed; product composition rejects loopback, arbitrary host, cleartext, redirects and caller URL overrides; MockWebServer exception remains test-only.
- Gate-on provider-selection regression tests: HTTP gate OFF chooses the default explicitly labeled Fake provider for every OAuth/gateway state; HTTP gate ON with OAuth unavailable, missing/invalid grant, invalid/stale session, wrong account/scope, or missing/wrong/ineligible gateway fails visibly before provider creation and never chooses Fake; HTTP gate ON with a valid eligible OAuth/session/gateway combination chooses Live; every Live error remains visible and never falls back to Fake.
- Authorization tests: wrong account mock, wrong/missing scope, stale/revoked session, account switch and process-death session all reject before token/HTTP access. No real second-account attempt.
- Boundary tests: non-primary ID, CalendarList and writes rejected before authorization/token/network; only exact events-readonly scope accepted.
- Transport tests: Google HTTPS only; token/session validity checked per request; 401/403/429/disconnect/revoke/cancel are sanitized errors and never switch to Fake; bounded paging and retry do not leak credentials.
- Calendar behavior tests: successful approved test connection renders events, handles pagination, time zones and all-day events. Exercise using fixtures/mocks until H4-F real connection is separately approved.
- Lifecycle tests: disconnect/revoke/re-authorization/account change invalidate previous session; Live failure is visible and not mislabeled as Fake.
- Privacy tests: log capture contains no token, code, account identity or event content; no Calendar-to-Cloud-AI dispatch path.

No production gate is enabled by this design. H4-E2 must run relevant Android unit/integration/security tests plus required variant/CI checks and review current source. H4-F real connection is separate and requires the approvals in the [gate matrix](../../plans/h4-e1-to-h4-f-gate-matrix.md).
