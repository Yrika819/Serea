# G2 — Google OAuth / Gmail / Calendar Readiness

Report date: 2026-10-27
Official documentation retrieved: 2026-10-27

## Agent checkpoint

```text
AGENT_ID: G2
ACTIVE_PHASE: G2 — isolated authorization lifecycle and Google policy readiness
OVERALL_STATUS: PARTIAL — Fake authorization state now keeps app-local disconnect separate from shared grant revocation; focused coverage passes; real Google readiness remains Owner-gated
BASE_SHA: 3924b36080ffa43331f065a15db80c604c243355
CURRENT_HEAD: b87b39d9c86d94bc6121c0605aaccd9497e9694c (disconnect/revoke semantic repair; this report update is a subsequent documentation-only checkpoint)
TREE_SHA: 63b0593992ddfdfd3aca5b3da3ffab3bc3b83c80 (semantic-repair implementation tree)
WORKTREE: /workspace/Serea-g2-google-auth-readiness
BRANCH: codex/serea-g2-google-auth-readiness
DRAFT_PR: none
CI_RUN_IDS: 40489486365, 40489675684, 40490686989, 40494126664 — `.github/workflows/android-personal-beta.yml`, feature branch pushes, each with zero jobs (main-only workflow; not G2 test results)
ARTIFACT_IDS: none
```

The initial implementation commit `d5ef1a9faf56818f47a2db4012d05cf6614ffb89` (tree `420a818a2a4c97ac06c7df0e3ec11db0d3a3c314`) remains immutable. The report checkpoint was committed separately at `43c258b78bfc228a98c78609fa531210131588ea` (tree `41757fd3a497e4c9f4bb23e1961bab18ebf1886c`). G1's first review requested repair of two Fake-only lifecycle races; implementation commit `efa24f660d28ec92cc3095cc868f27b1d0e576b9` (tree `730a944b66bb46ec46a1e944db57619aca9bc944`) addressed them. A further G1 review of G4 found a disconnect/revoke state mismatch; implementation commit `b87b39d9c86d94bc6121c0605aaccd9497e9694c` (tree `63b0593992ddfdfd3aca5b3da3ffab3bc3b83c80`) addresses that separately. No prior commit was amended or rewritten. This report update is a documentation-only descendant checkpoint; the current branch ref is verified after push.

## Existing implementation and reuse decision

The base already contains an isolated `googleauth` module and service adapters, so this lane did not add another token/session framework.

- `GoogleAuthorizationGateway` separates the Play Services implementation from `FakeGoogleAuthorizationGateway`. The real gateway is default-disabled, requires an explicitly selected Android account, and only accepts allowlisted feature scopes.
- `AccessTokenLease` holds an access token transiently in memory, redacts its string form, checks validity and expiry at use time, and drops the token reference on close. It exposes no persistence API.
- `ConsentRequestRegistry` is a single-slot consent handoff. It rejects overlap and stale IDs, binds requested scopes and a launched resolution to the matching callback, and supports cancellation.
- `GmailProviderSessions` and `CalendarProviderSessions` default to Fake. Google HTTP sessions require both an explicit owner gate and a `PLAY_SERVICES` gateway. Replacing or closing a session cancels in-flight reads and gates result delivery so an old account/session cannot publish a late result.
- Gmail requests only `gmail.readonly`. Calendar event reads request `calendar.events.readonly`; calendar-list scope is separately optional and requested only for calendar discovery.
- Fake and MockWebServer tests already cover expiry, denial, revocation, account-switch during token acquisition, replacing a session after token acquisition, cancellation, redirect credential stripping, and fail-closed authorization.

### Ordinary UI reachability

The ordinary Gmail and Calendar UI cannot reach the real provider path at this revision. The app composition in `AppProviderComposition.kt` constructs Fake providers; it does not supply `PlayServicesGoogleAuthorizationGateway` or call `openGoogleReadOnly`. The authorization panel is explicitly Fake-only and says that it uses no real Google account, OAuth UI, network, or service data. The Google provider constructors are available as a future integration seam, but no user-facing connection flow is wired. This is a handoff to the integration owner; this lane did not edit `MainActivity.kt`, `SereaAppShell.kt`, or `AppProviderComposition.kt`.

## G2 implementation

The Settings-only Fake authorization walkthrough now exposes these explicit states:

| State | Meaning in the Fake walkthrough |
|---|---|
| `DISCONNECTED` | No authorization is active, or the app-local feature toggle was disconnected |
| `AUTHORIZATION_REQUIRED` | Consent is needed, declined/canceled consent must be retried, or the user reconnected an app-local feature |
| `AUTHORIZING` | A synthetic authorization action is in progress |
| `AUTHORIZED` | The Fake gateway supplied a synthetic short-lived lease; the walkthrough closes it immediately |
| `EXPIRED` | Synthetic authorization expired |
| `REVOKED` | The synthetic shared grant or account binding was revoked/replaced |
| `ERROR` | Denial or another non-recoverable authorization failure for the current attempt |

This is demo UI state only; it does not represent a real account or assert real OAuth readiness. Denial details remain in the synthetic status message while the public lifecycle state stays within the required state set. Explicit app-local disconnect is tracked separately from the shared grant so the UI can distinguish “not connected” from “app-local disconnect” without conflating it with Google revocation.

## Test inventory and behavior evidence

Existing focused test IDs relevant to the security contract:

- `GoogleAuthorizationTest#accountSwitchClearsTheOldAccountGrant`
- `GoogleAuthorizationTest#consentRegistryRejectsOverlapAndStaleResultsAndAllowsCancellation`
- `GoogleAuthorizationTest#googleGrantRevocationRemovesAllServiceScopes`
- `GoogleAuthorizationTest#expirationRequiresReauthorization`
- `GoogleAuthorizationTest#appFeatureDisableDoesNotRevokeSharedGoogleGrant`
- `GmailProviderSessionsTest#accountSwitchDuringTokenLeaseAcquisitionCannotDispatchAGmailRequest`
- `GmailProviderSessionsTest#replacingSessionCancelsConcurrentReadsWithoutCancellingTheirParentOrSiblingCommunication`
- `GmailProviderSessionsTest#responseAlreadyProducedByOldSessionIsDeniedWhenAccountSwitchWinsBeforeDelivery`
- `GmailHttpReadPortTest#accountSwitchDuringTokenLeaseAcquisitionRejectsBeforeHttpDispatch`
- `GmailHttpReadPortTest#invalidatedTokenLeaseDoesNotReachTheServer`
- `GoogleCalendarHttpReadPortTest#accountSwitchDuringTokenLeaseAcquisitionRejectsBeforeHttpDispatch`
- `CalendarProviderSessionsTest#replacingSessionCancelsItsReadWithoutCancellingParentOrSiblingCommunication`
- `CalendarProviderSessionsTest#replacingSessionImmediatelyAfterLeaseAcquisitionPreventsCalendarHttpDispatch`

These HTTP tests use MockWebServer and synthetic fixture tokens only. They are contract simulations, not proof of live Google OAuth or API operation.

Added focused test IDs:

- `GoogleAuthorizationLifecycleTest#requestedLifecycleStatesAreExplicitAndDistinct`
- `GoogleAuthorizationLifecycleTest#authorizationOutcomeMapsToUserFacingLifecycleWithoutLeakingFailureDetails`
- `GoogleAuthorizationLifecycleTest#fakeViewModelWalksThroughAuthorizationExpiryRevocationAndDisconnectStates`

### G1 review finding and repair

G1 reproduced two LOW-severity races in the synthetic Settings walkthrough with a `StandardTestDispatcher`; these were test/demo state-machine reliability issues, not evidence of a real OAuth exposure:

1. Two feature requests queued before the first coroutine ran could advance the generation twice. The first authorization outcome was then ignored, leaving a feature in `AUTHORIZING` with no consent handoff.
2. A queued synthetic grant could execute after revoke or app-local disconnect, mutating Fake gateway scopes even though its stale UI result was discarded.

The repair adds a synchronously set `inFlightFeature` guard before launching work, rejects a second operation without invalidating the first, disables competing feature actions while busy, and checks operation validity before the queued Fake gateway call. Cancellation, revoke, account switch/expiry, and feature disconnect invalidate the applicable operation; stale leases are closed. No real gateway/provider or scope was changed.

Added deterministic test IDs:

- `GoogleAuthorizationLifecycleTest#rapidFeatureRequestsKeepTheFirstAuthorizationAndCancellationReleasesTheGuard`
- `GoogleAuthorizationLifecycleTest#queuedGrantCannotRestoreFakeScopesAfterSharedRevocation`
- `GoogleAuthorizationLifecycleTest#queuedGrantCannotRestoreFakeScopesAfterInAppDisconnect`
- `GoogleAuthorizationLifecycleTest#sharedRevocationPreservesAppDisconnectUntilExplicitReconnect`

The first test requests Gmail and then Calendar before `runCurrent()` and verifies that Gmail remains coherent, the competing request is rejected, and cancellation releases the guard for a subsequent request. The other two queue a grant, invalidate it through revoke or app-local disconnect before `runCurrent()`, and verify that the Fake gateway remains without the stale grant.

G1's later G4 review found that a shared-grant revoke cleared the Fake gateway's feature-disabled flag but left the ViewModel flag set. The intended invariant is that shared authorization revocation clears scopes and invalidates leases, while an independent app-local feature disconnect persists until the user explicitly reconnects that feature. The Fake gateway now preserves the app-local disabled set on shared revoke; the ViewModel blocks even a direct consent request while disabled; and the visible feature label says “アプリ内切断（デモ）” while the shared status message reports the revoke. The deterministic regression verifies the feature stays disabled, the scope remains `NEEDS_CONSENT`, consent is not queued while disconnected, explicit reconnect re-enables the request action, and a fresh consent request still remains pending rather than silently granting a scope.

G1 accepted this Fake-only repair after independently parsing the 75-test JUnit XML and confirming the isolated Android test-source compile. G1 also found a remaining parity issue outside this lane's change scope: `PlayServicesGoogleAuthorizationGateway.revokeGoogleGrant()` currently clears its `disabledFeatures` set after a successful revoke (`GoogleAuthorization.kt:358–361`), while the Fake now preserves app-local disconnect. Do not wire the real OAuth flow until this contract is reconciled and tested. No real gateway code was changed in this phase.

Final JUnit class counts:

| Test class | Executed |
|---|---:|
| `GoogleAuthorizationLifecycleTest` | 7 |
| `GoogleAuthorizationTest` | 22 |
| `GmailProviderSessionsTest` | 8 |
| `GmailHttpReadPortTest` | 11 |
| `CalendarProviderSessionsTest` | 7 |
| `GoogleCalendarHttpReadPortTest` | 20 |
| **Total** | **75** |

Focused command:

```text
JAVA_HOME=/tmp/serea-g2-jdk PATH=/tmp/serea-g2-jdk/bin:$PATH ./gradlew --no-daemon :app:testDebugUnitTest --rerun-tasks --tests 'app.serea.android.googleauth.GoogleAuthorizationLifecycleTest' --tests 'app.serea.android.googleauth.GoogleAuthorizationTest' --tests 'app.serea.android.gmail.GmailProviderSessionsTest' --tests 'app.serea.android.gmail.transport.GmailHttpReadPortTest' --tests 'app.serea.android.calendar.CalendarProviderSessionsTest' --tests 'app.serea.android.calendar.transport.GoogleCalendarHttpReadPortTest'
```

The initial pre-review run on `d5ef1a9faf56818f47a2db4012d05cf6614ffb89` passed 71 tests (3 lifecycle tests plus the five existing provider/HTTP classes), 0 failures/errors/skips. After the request-race repair, `efa24f6` passed 74 tests. After G1's disconnect/revoke review, final source commit `b87b39d9c86d94bc6121c0605aaccd9497e9694c` passed **75 tests**, 0 failures/errors/skips, exit code 0, Gradle `BUILD SUCCESSFUL`. Exact XML files in the worktree are:

- `android/app/build/test-results/testDebugUnitTest/TEST-app.serea.android.googleauth.GoogleAuthorizationLifecycleTest.xml` (7 tests)
- `android/app/build/test-results/testDebugUnitTest/TEST-app.serea.android.googleauth.GoogleAuthorizationTest.xml` (22 tests)
- `android/app/build/test-results/testDebugUnitTest/TEST-app.serea.android.gmail.GmailProviderSessionsTest.xml` (8 tests)
- `android/app/build/test-results/testDebugUnitTest/TEST-app.serea.android.gmail.transport.GmailHttpReadPortTest.xml` (11 tests)
- `android/app/build/test-results/testDebugUnitTest/TEST-app.serea.android.calendar.CalendarProviderSessionsTest.xml` (7 tests)
- `android/app/build/test-results/testDebugUnitTest/TEST-app.serea.android.calendar.transport.GoogleCalendarHttpReadPortTest.xml` (20 tests)

All six report 0 failures/errors/skips. No emulator, real Google account, OAuth credential, or Google network endpoint was used.

## Official Google policy and setup review

This is an implementation-readiness summary, not legal advice. Requirements below are distinguished from Google's recommended setup guidance. Google can update these sources; recheck them against the selected Cloud project before enabling OAuth.

### Requirements / policy conditions

1. **Register the correct platform OAuth client.** Google says each app using its OAuth infrastructure must have a registered client and a separate client for each platform. For an Android client, the Cloud Console requires the `AndroidManifest.xml` package name and a signing certificate SHA-1 fingerprint. The final pair is unresolved until the release package and Owner-controlled signing identity are selected. Do not register a guessed/debug fingerprint as the Beta identity. [Create access credentials](https://developers.google.com/workspace/guides/create-credentials), [Client authentication](https://developers.google.com/android/guides/client-auth), retrieved 2026-10-27.

2. **Use the narrowest scope required and match declared scopes.** Gmail marks `https://www.googleapis.com/auth/gmail.readonly` as **Restricted**. Restricted-scope verification is required before requesting those scopes from Google Accounts unless an exception applies. The documented personal-use exception is narrowly framed: one user, or fewer than 100 users all personally known, who may accept the unverified-app warning and user cap. This exception is not assumed to cover a wider beta audience. If restricted Google data is stored or transmitted on a server, Google requires a security assessment under the applicable policy. [Gmail API scopes](https://developers.google.com/workspace/gmail/api/auth/scopes), [Restricted scope verification](https://developers.google.com/identity/protocols/oauth2/production-readiness/restricted-scope-verification), retrieved 2026-10-27.

3. **Calendar scopes are read-only at the API boundary.** The current allowlist is `calendar.events.readonly` for events and `calendar.calendarlist.readonly` only for calendar discovery. The Calendar documentation says to choose the narrowest required scope and that public apps using scopes permitting access to user data must complete the applicable verification process. The exact verification classification/applicability must be checked for the selected project in Google Auth Platform; this report does not infer that “read-only” means exempt. [Calendar API scopes](https://developers.google.com/workspace/calendar/api/auth), retrieved 2026-10-27.

4. **Disclosure, affirmative consent, limited use, secure handling, and deletion controls apply.** Google Workspace policy requires a clear in-app disclosure immediately before consent, affirmative user action, an accurate privacy policy, user-facing approved use, data deletion/help controls, and secure handling. It restricts transfer of Workspace user data and prohibits using it to train/improve a general AI model; any exception for a specific user-facing feature still requires the conditions in the policy, including user consent. Restricted data stored on portable devices, at rest, or transferred externally has encryption/security requirements. [Workspace user data and developer policy](https://developers.google.com/workspace/workspace-api-user-data-developer-policy), retrieved 2026-10-27.

5. **Support disconnection and revocation.** The Play Services `AuthorizationClient` reference states that apps must let signed-in users disconnect their Google account; deleting the user's account requires deleting information obtained from Google APIs. Google OAuth policy says refresh-token revocation/expiry can occur and the application must handle it; no claim is made that the current module observes all out-of-band revocations automatically. [AuthorizationClient](https://developers.google.com/android/reference/com/google/android/gms/auth/api/identity/AuthorizationClient), [OAuth 2.0 Policies](https://developers.google.com/identity/protocols/oauth2/policies), retrieved 2026-10-27.

### Recommendations / project-dependent behavior

- Google recommends distinct Cloud projects for testing and production. This is a recommendation in the policy guide, not an implemented requirement in this repository.
- External OAuth apps in `Testing` have a 100-user test cap and Google documents seven-day refresh-token expiry in applicable Testing cases, except when only basic identity scopes are requested. The current Android adapter asks for access tokens and does not request/store a server auth code or refresh token, so this caveat is not evidence that this implementation currently stores a refresh token. It becomes relevant if the future flow adds one. [OAuth app state overview](https://developers.google.com/identity/protocols/oauth2/production-readiness/overview), [Using OAuth 2.0](https://developers.google.com/identity/protocols/oauth2/), retrieved 2026-10-27.
- Google recommends incremental, contextual authorization and least privilege. The code requests per-feature scopes and leaves Calendar list scope optional; the app still needs a real user-facing disclosure and connection flow before live use.

## Security / compatibility impact

- Changes are limited to the synthetic authorization walkthrough and its lifecycle tests. No provider, token, persistence, OAuth activation, app manifest, package, signing, network endpoint, scope, Gmail/Calendar adapter, or composition-root behavior changed.
- Fake remains the default provider. No production secrets, real accounts, or real user data were used.
- Real OAuth/API acceptance: **NOT_RUN / Owner-gated**.
- Ordinary UI real-provider connection: **NOT WIRED**.

## Commands and results

```text
Pre-review focused JVM: PASS — 71 tests, 0 failures, 0 errors, 0 skips, immutable implementation commit `d5ef1a9faf56818f47a2db4012d05cf6614ffb89`, `--rerun-tasks`
Request-race repair JVM: PASS — 74 tests, 0 failures, 0 errors, 0 skips, implementation commit `efa24f660d28ec92cc3095cc868f27b1d0e576b9`, `--rerun-tasks`
Disconnect/revoke repair JVM: PASS — 75 tests, 0 failures, 0 errors, 0 skips, source commit `b87b39d9c86d94bc6121c0605aaccd9497e9694c`, Gradle session `16750`, `--rerun-tasks`
One earlier draft run failed test compilation because the new lifecycle assertion omitted the `assertTrue` import; the import was added before the final 75-test run.
Focused command: shown above
Gradle result: BUILD SUCCESSFUL, exit code 0
Artifact: none (local JUnit XML only)
CI Run `40489486365`: `.github/workflows/android-personal-beta.yml`, feature-branch push, completed with zero jobs; this main-only workflow did not execute a test job and is not treated as a G2 verification result
Android instrumentation source compile after repair: PASS — `JAVA_HOME=/tmp/serea-g2-jdk PATH=/tmp/serea-g2-jdk/bin:$PATH ./gradlew --no-daemon :app:compileIsolatedAndroidTestKotlin`, exit code 0 on final source commit `b87b39d9c86d94bc6121c0605aaccd9497e9694c`, Gradle session `6229`; no emulator was launched
Initial attempted compile command: `JAVA_HOME=/tmp/serea-g2-jdk PATH=/tmp/serea-g2-jdk/bin:$PATH ./gradlew --no-daemon :app:compileDebugAndroidTestKotlin` — failed because that task does not exist for this project's variants; task discovery identified `isolatedAndroidTest` and that source compile then passed
Initial JVM attempt: exit 127 because installed OpenJDK runtime did not include `javac`
Initial JVM command (session 60245): `./gradlew --no-daemon :app:testDebugUnitTest --tests 'app.serea.android.googleauth.GoogleAuthorizationLifecycleTest' --tests 'app.serea.android.googleauth.GoogleAuthorizationTest' --tests 'app.serea.android.gmail.GmailProviderSessionsTest' --tests 'app.serea.android.gmail.transport.GmailHttpReadPortTest' --tests 'app.serea.android.calendar.CalendarProviderSessionsTest' --tests 'app.serea.android.calendar.transport.GoogleCalendarHttpReadPortTest'` — exit 127; Gradle reported `/usr/lib/jvm/java-21-openjdk-amd64` lacked `[JAVA_COMPILER]`
JDK used for passing commands: Eclipse Temurin OpenJDK 21.0.13+9, downloaded from https://api.adoptium.net/v3/binary/latest/21/ga/linux/x64/jdk/hotspot/normal/eclipse (resolved asset: https://github.com/adoptium/temurin21-binaries/releases/download/jdk-21.0.13%2B9/OpenJDK21U-jdk_x64_linux_hotspot_21.0.13_9.tar.gz), stored/extracted only under `/tmp`
JDK archive SHA-256: d7f5d5cf865a2902b70ff549b97586a7d3f234e016f041ab447ea740c7d66d1c
Repository dependencies/configuration changed for JDK: no
```

`git diff --check` passed before source commit `b87b39d`. The focused Android instrumentation source compile passed, but instrumentation/E2E tests were **NOT_RUN** because this lane did not start an emulator. Full Formal Required was not run from this lane; parent integration owns those gates. G1 accepted the Fake-only source repair; real OAuth/provider parity remains unresolved.

## Handoff contract

- `REVIEW_REQUEST`: G1 read-only review accepted Fake-only commit `b87b39d9c86d94bc6121c0605aaccd9497e9694c`; before real OAuth flow wiring, reconcile real gateway revoke behavior with the Fake app-local disconnect contract and add corresponding contract coverage.
- `KNOWN_ISSUES`: Real account selection, OAuth disclosure, Activity Result consent wiring, connection/revocation UX, and normal UI provider selection are not wired. Production package/certificate and scope verification status remain unknown. Real `PlayServicesGoogleAuthorizationGateway.revokeGoogleGrant()` clears `disabledFeatures` after successful revoke while the Fake preserves app-local disabled features; reconcile semantics and tests before wiring the real flow.
- `INTEGRATION_DEPENDENCIES`: Integration owner must provide explicit dependency injection for a `PlayServicesGoogleAuthorizationGateway` plus account-bound Gmail/Calendar sessions only after Owner configuration/authorization; keep Fake defaults and fail closed.
- `NEXT_ACTION`: G1 may cherry-pick the accepted Fake-only fix and report checkpoint. Reconcile and test real-gateway revocation semantics before enabling a real OAuth flow; keep all OAuth wiring in the integration-owned Composition Root.
- `OWNER_APPROVAL_REQUIRED`: Real OAuth/project configuration, Google account access, scope verification, production package/signing identity, and any live-service acceptance.
