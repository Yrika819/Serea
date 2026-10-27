# G2 — Google OAuth / Gmail / Calendar Readiness

Report date: 2026-10-27
Official documentation retrieved: 2026-10-27

## Agent checkpoint

```text
AGENT_ID: G2
ACTIVE_PHASE: G2 — isolated Google authorization lifecycle, revocation and cancellation safety
OVERALL_STATUS: PARTIAL — Gmail/Calendar in-flight reads and Identity authorization/revoke tasks fail closed across invalidation and caller cancellation; live Google readiness remains Owner-gated
BASE_SHA: 3924b36080ffa43331f065a15db80c604c243355
CURRENT_HEAD: 69c9bcb3118343fbcbc4d7595125525420744a7b (G1-accepted source/evidence commit; this report-only checkpoint is its fast-forward descendant)
HEAD_TREE_SHA: 7101d331ee116ee92e3c660b1df068e629267bf9 (accepted source/evidence tree)
TREE_SHA: source/evidence tree above; this checkpoint changes documentation only
WORKTREE: `Serea-g2-google-auth-readiness` (isolated; clean after the documentation-only commit/push)
BRANCH: codex/serea-g2-google-auth-readiness
REMOTE_REF: `origin/codex/serea-g2-google-auth-readiness`; accepted source/evidence commit `69c9bcb3118343fbcbc4d7595125525420744a7b` was verified remotely before this documentation-only fast-forward
DRAFT_PR: none — G4 PR #125 carries the accepted G2 source/evidence changes
CI_RUN_IDS: 40489486365, 40489675684, 40490686989, 40494126664, 40494480729, 40510449122 — `.github/workflows/android-personal-beta.yml`, feature-branch pushes, each with zero jobs (main-only workflow; not G2 test results); latest zero-job run: `40510449122`
ARTIFACT_IDS: none
```

The initial implementation commit `d5ef1a9faf56818f47a2db4012d05cf6614ffb89` (tree `420a818a2a4c97ac06c7df0e3ec11db0d3a3c314`) remains immutable. The report checkpoint was committed separately at `43c258b78bfc228a98c78609fa531210131588ea` (tree `41757fd3a497e4c9f4bb23e1961bab18ebf1886c`). G1's Fake-only authorization race repair is `efa24f660d28ec92cc3095cc868f27b1d0e576b9`; the app-disconnect/revoke semantic repair is `b87b39d9c86d94bc6121c0605aaccd9497e9694c`. This latest provider in-flight invalidation repair is `7c3af22605b202ec55eb61cc59e029948dd1217b` (tree `13cd093386321a6bb496dcd894a379538a08e7dd`). No prior commit was amended or rewritten. This report update is a documentation-only descendant checkpoint; the current branch ref is verified after push.

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
- `GoogleAuthorizationTest#appDisableDuringSuccessfulSharedRevokePersistsUntilExplicitReconnect`
- `GoogleAuthorizationTest#fakeRevokeClearsScopesAndInvalidatesLeasesButPreservesDisabledFeature`
- `GmailProviderSessionsTest#delayedGmailResponseIsRejectedAfterGrantRevocationOrFeatureDisable`
- `CalendarProviderSessionsTest#delayedCalendarResponseIsRejectedAfterGrantRevocationOrFeatureDisable`
- `GmailProviderSessionsTest#appFeatureDisableDoesNotRevokeSharedGoogleGrant` now checks that Gmail disable leaves Calendar lease validity unchanged and vice versa

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

G1 accepted this Fake-only repair after independently parsing the 75-test JUnit XML and confirming the isolated Android test-source compile. The next G1 review found two MEDIUM real-provider gaps, addressed in the latest source commit below.

### G1 review findings: in-flight read invalidation (MEDIUM)

G1 reproduced two real-provider contract gaps. First, a Gmail or Calendar read that had reached its HTTP endpoint could still return its response body after the authorization grant was revoked or the app feature disabled; the transport did not validate the token lease after reading the response. Second, a successful real `revokeGoogleGrant()` cleared `disabledFeatures`; a feature disabled while Play Services revocation was awaiting completion could therefore become enabled without an explicit reconnect. Both are fixed in source commit `7c3af22605b202ec55eb61cc59e029948dd1217b`:

- `AccessTokenLease.ensureValid()` rechecks the lease validity callback, expiry, and closed-token state without exposing token bytes. Gmail and Calendar token wrappers expose this check to their transports.
- Each HTTP transport reads the bounded response body, checks coroutine activity, then validates the lease before returning/parsing response data. Invalidated or expired leases map to the existing permission-denied path; the token is closed in `finally`.
- Provider sessions recheck controller current-session identity after the delegate returns, then check closed state under the session lock. The identity check is outside that lock to preserve controller-lock → session-lock ordering.
- Successful Play Services shared revoke clears known permissions and increments all feature versions via `applySuccessfulGoogleGrantRevocation`, but does not clear independently managed `disabledFeatures`. A concurrent `disableInApp()` therefore remains in force after revoke completes and invalidates pre-existing leases.
- Default provider remains Fake; Gmail and Calendar scopes remain read-only. No real OAuth, Google account, or Google endpoint was used.

Deterministic coverage includes delayed 2-second MockWebServer bodies for Gmail and Calendar. Each test waits until its GET reaches the local server, then revokes the synthetic grant or disables the corresponding feature before the body returns; all four scenarios return permission-denied rather than a message/calendar object. `GoogleAuthorizationTest#appDisableDuringSuccessfulSharedRevokePersistsUntilExplicitReconnect` models disable while revoke is pending, then checks permission-cache clearing, version increments, and persistence of the local disable until reconnect. `GoogleAuthorizationTest#fakeRevokeClearsScopesAndInvalidatesLeasesButPreservesDisabledFeature` checks the Fake contract. The existing `appFeatureDisableDoesNotRevokeSharedGoogleGrant` now also proves that disabling Gmail leaves a Calendar lease valid and vice versa.

Final JUnit class counts:

| Test class | Executed |
|---|---:|
| `GoogleAuthorizationTest` | 24 |
| `GmailProviderSessionsTest` | 9 |
| `GmailHttpReadPortTest` | 11 |
| `CalendarProviderSessionsTest` | 8 |
| `GoogleCalendarHttpReadPortTest` | 20 |
| **Total** | **72** |

Focused command:

```text
./gradlew --no-daemon :app:testDebugUnitTest --rerun-tasks --tests 'app.serea.android.googleauth.GoogleAuthorizationTest' --tests 'app.serea.android.gmail.GmailProviderSessionsTest' --tests 'app.serea.android.gmail.transport.GmailHttpReadPortTest' --tests 'app.serea.android.calendar.CalendarProviderSessionsTest' --tests 'app.serea.android.calendar.transport.GoogleCalendarHttpReadPortTest'
```

The affected-source run on `7c3af22605b202ec55eb61cc59e029948dd1217b` passed **72 tests**, 0 failures/errors/skips, exit code 0, Gradle `BUILD SUCCESSFUL`. Exact XML files in the worktree are:

- `android/app/build/test-results/testDebugUnitTest/TEST-app.serea.android.googleauth.GoogleAuthorizationTest.xml` (24 tests)
- `android/app/build/test-results/testDebugUnitTest/TEST-app.serea.android.gmail.GmailProviderSessionsTest.xml` (9 tests)
- `android/app/build/test-results/testDebugUnitTest/TEST-app.serea.android.gmail.transport.GmailHttpReadPortTest.xml` (11 tests)
- `android/app/build/test-results/testDebugUnitTest/TEST-app.serea.android.calendar.CalendarProviderSessionsTest.xml` (8 tests)
- `android/app/build/test-results/testDebugUnitTest/TEST-app.serea.android.calendar.transport.GoogleCalendarHttpReadPortTest.xml` (20 tests)

All five report 0 failures/errors/skips. No emulator, real Google account, OAuth credential, or Google network endpoint was used.

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

- Changes are limited to authorization lease validation, read-only Gmail/Calendar transports and session delivery gates, the real gateway's revoke-state application, and focused tests. No OAuth activation, app manifest, package, signing, external network endpoint, scope, or composition-root behavior changed.
- Fake remains the default provider. No production secrets, real accounts, or real user data were used.
- Real OAuth/API acceptance: **NOT_RUN / Owner-gated**.
- Ordinary UI real-provider connection: **NOT WIRED**.

## Commands and results

```text
Provider invalidation focused JVM: PASS — 72 tests, 0 failures, 0 errors, 0 skips, source commit `7c3af22605b202ec55eb61cc59e029948dd1217b`, Gradle session `24660`, `--rerun-tasks`
Android instrumentation source compile: PASS — `./gradlew --no-daemon :app:compileIsolatedAndroidTestKotlin`, exit code 0 on source commit `7c3af22605b202ec55eb61cc59e029948dd1217b`, Gradle session `69811`; run with a temporary Temurin JDK 21.0.13+9 outside the repository; no emulator was launched
Early delayed-test attempts: first run starved the child read by blocking `runBlocking` in `MockWebServer.takeRequest`; moved that wait to `Dispatchers.IO`. Second run passed the expected permission-denied checks but compared the shared server request counter to 1 for both loop iterations; changed assertion to compare against the pre-scenario counter. The final run passed.
Focused command: shown above
Gradle result: BUILD SUCCESSFUL, exit code 0
Artifact: none (local JUnit XML only)
CI Run `40489486365`: `.github/workflows/android-personal-beta.yml`, feature-branch push, completed with zero jobs; this main-only workflow did not execute a test job and is not treated as a G2 verification result
Initial attempted compile command: `./gradlew --no-daemon :app:compileDebugAndroidTestKotlin` — failed because that task does not exist for this project's variants; task discovery identified `isolatedAndroidTest` and that source compile then passed
Initial JVM attempt: exit 127 because installed OpenJDK runtime did not include `javac`
Initial JVM command (session 60245): `./gradlew --no-daemon :app:testDebugUnitTest --tests 'app.serea.android.googleauth.GoogleAuthorizationLifecycleTest' --tests 'app.serea.android.googleauth.GoogleAuthorizationTest' --tests 'app.serea.android.gmail.GmailProviderSessionsTest' --tests 'app.serea.android.gmail.transport.GmailHttpReadPortTest' --tests 'app.serea.android.calendar.CalendarProviderSessionsTest' --tests 'app.serea.android.calendar.transport.GoogleCalendarHttpReadPortTest'` — exit 127; Gradle reported the installed OpenJDK runtime lacked `[JAVA_COMPILER]`
JDK used for passing commands: Eclipse Temurin OpenJDK 21.0.13+9, downloaded from https://api.adoptium.net/v3/binary/latest/21/ga/linux/x64/jdk/hotspot/normal/eclipse (resolved asset: https://github.com/adoptium/temurin21-binaries/releases/download/jdk-21.0.13%2B9/OpenJDK21U-jdk_x64_linux_hotspot_21.0.13_9.tar.gz), stored/extracted temporarily outside the repository
JDK archive SHA-256: d7f5d5cf865a2902b70ff549b97586a7d3f234e016f041ab447ea740c7d66d1c
JDK stored/extracted temporarily outside the repository. Repository dependencies/configuration changed for JDK: no
```

`git diff --check` passed before source commit `7c3af22`. The focused Android instrumentation source compile passed, but instrumentation/E2E tests were **NOT_RUN** because this lane did not start an emulator. Full Formal Required was not run from this lane; parent integration owns those gates. The in-flight response patch awaits independent review.

## Handoff contract

- `REVIEW_REQUEST`: G1 accepted source/evidence commit `69c9bcb3118343fbcbc4d7595125525420744a7b` (tree `7101d331ee116ee92e3c660b1df068e629267bf9`); G4 PR #125 carries the integrated changes.
- `KNOWN_ISSUES`: Real account selection, OAuth disclosure, Activity Result consent wiring, connection/revocation UX, and normal UI provider selection are not wired. Production package/certificate and scope verification status remain unknown. Real revoke now preserves app-local disabled features in source, but live OAuth behavior remains untested.
- `INTEGRATION_DEPENDENCIES`: Integration owner must provide explicit dependency injection for a `PlayServicesGoogleAuthorizationGateway` plus account-bound Gmail/Calendar sessions only after Owner configuration/authorization; keep Fake defaults and fail closed.
- `NEXT_ACTION`: G4 carries the accepted G2 source/evidence commit in PR #125 and owns the exact-head integrated Required run. Keep live OAuth disabled pending separately approved real authorization and service acceptance.
- `OWNER_APPROVAL_REQUIRED`: Real OAuth/project configuration, Google account access, scope verification, production package/signing identity, and any live-service acceptance.

### G1 follow-up G4-OAUTH-01: revoke initiation invalidation (MEDIUM)

G1 found that, during a real Play Services revoke request, existing Gmail and Calendar leases stayed valid until the remote revoke task completed. A delayed read could therefore return data after the user initiated revocation. The real gateway now invalidates cached permission state and increments every feature lease generation synchronously in the same `consentLock` section that claims the revoke operation, before checking Play Services or awaiting `Identity.revokeAccess()`. App-local feature-disable state is preserved. Remote failure or caller cancellation never restores old permissions or leases; `finally` only clears the in-flight flag.

The former mutable disabled-feature and version maps were extracted into `GoogleGrantLeaseState`, which is the state component owned by `PlayServicesGoogleAuthorizationGateway`. Production lease callbacks and the provider fixtures call the same `leaseVersions`, `isLeaseCurrent`, `invalidateGrant`, and `invalidateGoogleGrantAtRevokeStart` implementation. The local provider tests instantiate that same component/callback contract; they do not instantiate the Android-bound `PlayServicesGoogleAuthorizationGateway` or invoke Play Services. This avoids introducing a broad SDK abstraction solely for a test seam. Accordingly, the tests demonstrate the real shared lease-state and HTTP post-body contract, but do not claim live Play Services revoke execution.

The initial focused run on the new source completed 81 tests with 2 failures: the Gmail and Calendar delayed-read cases returned successfully. Preserved evidence is in `docs/agent-reports/evidence/g2-revoke-race-first-failure/`. A subsequent trace showed that the old lease was invalid and its captured version had incremented, but a 2-second MockWebServer body delay could expire during test setup. The test now uses a deterministic `MockWebServer.Dispatcher` barrier: the dispatcher signals request arrival and blocks before returning a response; the test starts revoke, verifies the lease is unusable and the request remains pending, then releases the response while the synthetic remote revoke gate is still pending. Lease identity, captured/revoked generation and post-body validity callback results are recorded and asserted. The second diagnostic run that established the body-delay timing issue is retained at `docs/agent-reports/evidence/g2-revoke-race-body-delay-diagnostic/`.

Exact delayed test IDs:

- `GmailProviderSessionsTest#delayedGmailResponseIsDeniedWhileRevocationIsPendingAndAfterEveryOutcome`
- `CalendarProviderSessionsTest#delayedCalendarResponseIsDeniedWhileRevocationIsPendingAndAfterEveryOutcome`

Each test releases the mock remote revoke as success, failure and cancellation. For every outcome, while the revoke gate is unresolved, the released Gmail/Calendar HTTP response is rejected as permission-denied; the post-body callback observes the bumped generation and no data object escapes. After each outcome, old leases remain unusable and cached permissions remain cleared. App-local disable-after-request tests remain in the suite, and the authorization unit test verifies that revoke invalidation preserves an independent app-local disable while invalidating both Gmail and Calendar generations.

### Previous focused verification (superseded by the final G2-OAUTH-02 verification below)

Forced six-class JVM command:

```text
./gradlew --no-daemon :app:testDebugUnitTest --rerun-tasks --tests 'app.serea.android.googleauth.GoogleAuthorizationLifecycleTest' --tests 'app.serea.android.googleauth.GoogleAuthorizationTest' --tests 'app.serea.android.gmail.GmailProviderSessionsTest' --tests 'app.serea.android.gmail.transport.GmailHttpReadPortTest' --tests 'app.serea.android.calendar.CalendarProviderSessionsTest' --tests 'app.serea.android.calendar.transport.GoogleCalendarHttpReadPortTest'
```

Result: **81/81 PASS, 0 failures, 0 errors, 0 skips**, exit code 0. Per-class XML counts:

| Test class | Executed |
|---|---:|
| `GoogleAuthorizationLifecycleTest` | 7 |
| `GoogleAuthorizationTest` | 24 |
| `GmailProviderSessionsTest` | 10 |
| `GmailHttpReadPortTest` | 11 |
| `CalendarProviderSessionsTest` | 9 |
| `GoogleCalendarHttpReadPortTest` | 20 |
| **Total** | **81** |

The six exact JUnit XML files and command summary are retained at `docs/agent-reports/evidence/g2-revoke-race-six-class-pass/`. The forced two-test run is at `docs/agent-reports/evidence/g2-revoke-race-final-targeted/` (2/2 PASS, 0 failures/errors/skips).

Android isolated instrumentation source compilation also passed:

```text
./gradlew --no-daemon :app:compileIsolatedAndroidTestKotlin
```

Exit code 0. No emulator was started; instrumentation/E2E and Play Services revoke remain **NOT_RUN**. The JDK was Eclipse Temurin 21.0.13+9, used temporarily outside the repository; repository Gradle/JDK configuration and dependencies were not changed. No live OAuth, real account, credential, Google endpoint, UI wiring, manifest, scopes, or Composition Root were changed.

This documentation supersedes the previous 72-test count and the prior description that real revoke invalidation occurred only after remote success. The corrected implementation is fail-closed at revoke initiation; G1 review is still required before integration. No CI run was initiated for this G2 repair.


### G1 follow-up G2-OAUTH-02: Identity task lifetime after caller cancellation (MEDIUM)

G1 identified that the Play Services `Identity.authorize()` and `Identity.revokeAccess()` tasks can outlive a caller coroutine. If cancellation released the shared operation gate before the underlying Google Task reached a terminal state, another authorization could begin while the old task could still complete and issue or revoke a grant. The gateway now wraps both task waits in the same `awaitGoogleIdentityTaskToTerminal` helper (`withContext(NonCancellable)`), and releases the shared gate only in `finally` after that wait returns or throws at the underlying task's terminal event. While the gate is held, `acquire()` returns `REQUEST_IN_PROGRESS` and competing authorization/revoke operations are rejected.

After a successful authorization task becomes terminal, the gateway checks caller activity before mapping the result, creating/caching a lease or returning a token. The exception path also checks caller activity before caching a classified outcome, so a late task failure cannot write state for a canceled caller. Revoke checks caller activity after both a successful terminal task and in the generic task-failure catch before classifying an error. A caller canceled during a remote task therefore receives cancellation only after terminal completion, not a normal `NETWORK`/`UNAVAILABLE` failure result. The existing synchronous revoke-start invalidation remains fail-closed, and remote success, failure, or caller cancellation never restores old permissions or leases. The `finally` path only releases the operation gate.

Deterministic regression IDs:

- `GoogleAuthorizationTest#cancelledRevokeCallerKeepsAuthorizationBlockedUntilRemoteTaskIsTerminal`
- `GoogleAuthorizationTest#cancelledAuthorizationWaitsForIdentityTaskBeforeReleasingSharedGate`
- `GoogleAuthorizationTest#cancelledRevokeCallerPropagatesCancellationAfterRemoteFailureBecomesTerminal`
- `GmailProviderSessionsTest#delayedGmailResponseIsDeniedWhileRevocationIsPendingAndAfterEveryOutcome`
- `CalendarProviderSessionsTest#delayedCalendarResponseIsDeniedWhileRevocationIsPendingAndAfterEveryOutcome`
- `GmailProviderSessionsTest#delayedGmailResponseIsDeniedAfterAppLocalDisable`
- `CalendarProviderSessionsTest#delayedCalendarResponseIsDeniedAfterAppLocalDisable`

The authorization tests cancel the caller while a synthetic task remains unresolved, verify the task waiter is still pending and the shared gate blocks new operations, then complete the task and verify the gate is released only afterward. The authorize regression verifies a late successful result is not mapped/cached for a canceled caller. The revoke-failure regression completes the still-pending synthetic task exceptionally after caller cancellation, then verifies cancellation wins over error classification, the gate remains held until terminal failure, and invalidated permissions/leases stay invalid. Gmail and Calendar use a MockWebServer dispatcher barrier to hold each response until revoke-start invalidation has occurred; they release the response while the synthetic remote revoke remains pending and verify permission-denied/no result for remote success, failure, and caller cancellation. Existing generation assertions also verify the lease is invalidated before response delivery.

The production gateway and these deterministic tests share `GoogleGrantLeaseState` and `awaitGoogleIdentityTaskToTerminal`; the tests exercise those exact components, not a duplicate model. They use `CompletableDeferred` to represent a suspended remote task and do not instantiate the Android-bound Play Services client or execute an actual `com.google.android.gms.tasks.Task`. Therefore actual Google Task completion/cancellation behavior and live OAuth remain **NOT_RUN**. The new code does not attempt to cancel the remote task; it waits for terminal completion to preserve serialization, and can keep the authorization/revoke gate held if Play Services never completes its task.

#### Exact-source verification before final review follow-up

Forced six-class JVM suite, run from `android/` on the working tree above:

```text
./gradlew --no-daemon :app:testDebugUnitTest --rerun-tasks --tests 'app.serea.android.googleauth.GoogleAuthorizationLifecycleTest' --tests 'app.serea.android.googleauth.GoogleAuthorizationTest' --tests 'app.serea.android.gmail.GmailProviderSessionsTest' --tests 'app.serea.android.gmail.transport.GmailHttpReadPortTest' --tests 'app.serea.android.calendar.CalendarProviderSessionsTest' --tests 'app.serea.android.calendar.transport.GoogleCalendarHttpReadPortTest'
```

#### Review follow-up verification and timing-race classification

A forced six-class run after adding the canceled-caller/failed-revoke regression completed 84 tests with one failure: `CalendarProviderSessionsTest#delayedCalendarResponseIsDeniedAfterAppLocalDisable` reported `Expected CalendarReadFailure` (Gradle session `49888`). The preserved JUnit XML and sanitized summary are at `docs/agent-reports/evidence/g2-oauth-02-six-class-first-failure/`. The failing test still used a 1-second MockWebServer body delay without a response barrier; JUnit reported 1.202 seconds for the test. The read returned before the assertion expecting a permission failure. This is classified as a **TEST_HARNESS timing race, supported but not directly timestamp-proven**: the test did not guarantee the app-local disable happened before the delayed body completed. It is not evidence that a disabled lease passed the post-body validity check under the intended ordering.

To establish that ordering explicitly, the Calendar and symmetric Gmail app-disable tests now use the existing bounded `GatedResponseDispatcher`. Each asserts that the request reached the dispatcher and the read remains pending before disabling the feature, asserts the lease is invalid before release, then releases the response and verifies permission-denied plus a post-body validity trace of `false`. The barrier is released in `finally`. The targeted trio passed 3/3 (Gradle session `70702`), with XML under `docs/agent-reports/evidence/g2-oauth-02-deterministic-targets/`.

The final forced six-class suite on the settled candidate passed **84/84**, 0 failures, 0 errors, 0 skips, exit code 0 (`BUILD SUCCESSFUL`), Gradle session `10019`. Counts: authorization lifecycle 7, authorization 27, Gmail sessions 10, Gmail HTTP 11, Calendar sessions 9, Calendar HTTP 20. The exact six JUnit XMLs and sanitized summary are at `docs/agent-reports/evidence/g2-oauth-02-six-class-pass/`.

The isolated Android test source compile was rerun with `--rerun-tasks` on the settled candidate:

```text
./gradlew --no-daemon :app:compileIsolatedAndroidTestKotlin --rerun-tasks
```

It passed with exit code 0 in Gradle session `71674` (`BUILD SUCCESSFUL`, 26 tasks executed). Both isolated main Kotlin and isolated Android test Kotlin compilation executed. Compile summary is at `docs/agent-reports/evidence/g2-oauth-02-six-class-pass/compile-summary.txt`; no emulator was started.

The additional canceled-revoke failure regression passed alone (1/1, session `40281`); it is also included in the final 84-test result. The first failed six-class run remains preserved separately and was not overwritten. No CI run was initiated. No live account, OAuth UI, Google API endpoint, token, or secret was used. The compiler used temporary Temurin JDK 21.0.13+9 outside the repository; repository dependencies and JDK configuration were unchanged. `git diff --check` passes. G1 review is still required before integration; no changes are staged, committed, or pushed.
