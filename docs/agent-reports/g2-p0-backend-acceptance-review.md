# G2 P0 Google Backend Acceptance Review

Review date: 2026-10-27

## Checkpoint

```text
AGENT_ID: G2
ACTIVE_PHASE: G2 backend acceptance review at immutable PR #132 source
OVERALL_STATUS: PARTIAL — safe Fake/Mock backend contracts are present; real account/UI/service acceptance remains Owner-gated
BASE_SHA: 373b8be15f407d951157d2a80a632e86d979acdf
BASE_TREE_SHA: 52b6b0f7b9f0449a9a78553bab235e62cdd97068
WORKTREE: /workspace/Serea-g2-google-readiness-hardening
BRANCH: codex/g2-google-readiness-hardening
DRAFT_PR: not created yet
```

This is an acceptance review of the authorization and provider-session backend at the fixed PR #132 source. It does not certify real Google OAuth or API operation. No source changes are recommended in this review: the backend already has fail-closed defaults, read-only scopes, account-bound leases, revocation invalidation, and Fake/Mock regression coverage. The missing P0 connection is the normal user-facing account/consent lifecycle, which belongs to Lane A’s Composition Root/UI and requires Owner-controlled Google setup before activation.

## Backend review

| Contract | Source evidence | Review |
|---|---|---|
| Safe default | `AppProviderComposition.fakeDefault()` constructs Fake Gmail and Calendar sessions; both provider session factories default to Fake. | ACCEPT |
| No accidental live OAuth | `PlayServicesGoogleAuthorizationGateway` defaults `realOAuthEnabled=false` and `selectedAccount=null`; authorization and token acquisition fail closed. | ACCEPT |
| No accidental Google HTTP | `GmailProviderSessions` and `CalendarProviderSessions` default `googleHttpEnabledByOwner=false`; Google transport selection also requires `PLAY_SERVICES`. A mock gateway cannot select the real adapter. | ACCEPT |
| Least privilege | Only `gmail.readonly`, `calendar.events.readonly`, and optional `calendar.calendarlist.readonly` are declared. Calendar event token acquisition requests only the event scope; calendar-list access is requested by discovery operations. | ACCEPT |
| Account binding | The real gateway requires an explicit Android `Account`; it creates a fresh opaque in-memory session ID. Adapters capture it and reject missing/changed IDs or a lease bound to another session. Account changes require a new gateway and replacement of affected provider sessions. | ACCEPT, with Lane A replacement obligation below |
| Consent handoff | A single-slot `ConsentRequestRegistry` binds request ID, scopes, granted-scope snapshot, launch, completion and cancellation; overlapping/stale results are rejected. | ACCEPT |
| Revocation and in-flight results | Revoke initiation synchronously increments the gateway generation, clears permission cache and invalidates feature leases before waiting for the Play Services task. Provider sessions cancel in-flight HTTP and check session/gateway generations at final result delivery. | ACCEPT for deterministic Fake/Mock contract; real Play Services behavior remains unverified |
| Token lifetime | `AccessTokenLease` keeps the access-token value in memory, redacts `toString()`, checks expiration/lease validity and drops the value on close. There is no token persistence API. | ACCEPT as current adapter boundary; do not add refresh-token storage without separate review |
| Scope/account data leakage | Status does not include account name or token; logs/tests use synthetic fixture tokens and test accounts. | ACCEPT within reviewed scope |

No changes to `MainActivity.kt`, `SereaAppShell.kt`, `AppProviderComposition.kt`, manifests, OAuth configuration, account selection, or provider activation are made by this lane.

## Required Lane A UI connection contract

Before ordinary Gmail or Calendar UI selects these providers, Lane A must implement and test this flow:

1. Keep `fakeDefault()` and both owner activation gates disabled in release configuration until the Owner has supplied the final Cloud project, OAuth audience/verification decision, package name, signing certificate SHA-1, privacy/disclosure materials, and explicit activation approval.
2. Start authorization only after the user selects a specific Gmail or Calendar feature. Request the smallest feature scope. Request `calendar.calendarlist.readonly` only when the user invokes calendar discovery; event reads require only `calendar.events.readonly`.
3. Invoke the gateway's `authorize(scopes)` from a foreground user action. For `ConsentRequired`, preserve the opaque request ID and launch only its matching one-shot `PendingIntent` through the Activity Result API. Pass that exact result to `completeConsentResult`; call `cancelConsentRequest` if the user cancels or abandons the flow. Do not infer consent from Activity dismissal or navigation.
4. After an account selection/change, create a new account-bound gateway and replace both provider sessions. Close the old sessions so their requests and late results are rejected. Never log the Google account name, access token, or authorization session ID.
5. Call `openGoogleReadOnly` only after the per-build Owner gate is deliberately enabled and a configured Play Services gateway is injected. Keep the UI on Fake when either condition is absent; do not silently turn configuration errors into a live provider.
6. Present separate feature-disconnect and shared Google-account-disconnect actions. Feature disconnect calls `disableInApp` and invalidates that feature’s reads without revoking the other service. Shared disconnect explains that Gmail and Calendar grants are revoked together, then calls `revokeGoogleGrant`; do not relabel feature disconnect as Google revocation.
7. Map consent denial, partial grants, expired/revoked access, Play Services unavailable/outdated, `REQUEST_IN_PROGRESS`, and cancellation to explicit recoverable UI states. Disable the relevant feature on denied scope and request it again only after renewed user intent.
8. On Activity/process recreation, rebuild transient gateway/session state from a new explicit account/authorization path or a correctly retained owner. Never revive an old session by copying an account ID or token. Verify that replacement cancels reads and prevents old account data from rendering.
9. Keep Google-derived Gmail/Calendar data out of Cloud AI by default. Any future transfer needs a separately reviewed, user-visible feature and consent that satisfies Google’s Limited Use terms and the selected AI provider’s data terms; generic model training is not allowed under the cited policy.

## Focused verification contract

Existing tests cover the backend invariants with Fake gateways, synthetic fixture tokens, and MockWebServer. Important focused classes are:

- `GoogleAuthorizationLifecycleTest`
- `GoogleAuthorizationTest`
- `GmailProviderSessionsTest`
- `GmailHttpReadPortTest`
- `CalendarProviderSessionsTest`
- `GoogleCalendarHttpReadPortTest`

The exact-head GitHub Android Unit artifact for PR #132 run `40599571275` is artifact `12778060790` (`sha256:a5ed666eaeea203e30cdeea648a271c19a6639ea572fc82806a52ff5b2d48430`). Its JUnit packet contains 335 cases total; these six Google authorization/session/transport classes contribute 92 tests, 0 failures, 0 errors and 0 skips. This is exact-head JVM/Mock evidence only, not live OAuth or service acceptance.

Local focused JVM test on this review branch is recorded in the handoff after it runs. Emulator tests, real Google accounts/OAuth, credentials, Gmail/Calendar endpoints, and Play Services authorization/revoke are **NOT_RUN**.

## External acceptance and Owner decisions

- The final package ID and certificate SHA-1 for the Android OAuth client are unresolved.
- The Google Cloud project(s), OAuth audience and publishing tier, allowed user audience, brand verification, and whether a personal-use or other verification exception actually applies are unresolved.
- Public privacy policy, Limited Use statement, in-context data disclosure, help/deletion flow, and live consent UX are not wired or reviewed.
- `gmail.readonly` is a Restricted scope. The actual Gmail permitted-use and verification assessment for the intended Personal Beta audience must be established before activation.
- CASA and any independent/periodic security assessment obligations depend on the final restricted-data path, server capability, scope, grants/users and Google’s review. No assessment is claimed.
- Real authorization, live account access, service HTTP, real token revocation, and account-deletion data cleanup are **NOT_RUN**.
- Google-data transfer to any Cloud AI remains disabled pending separate policy, provider-data-terms and Owner review.

These are activation blockers rather than authorization to loosen the current defaults. Main merge and production OAuth activation remain outside this lane.
