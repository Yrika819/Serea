# G8 Agent A — Fake Google Connection UI / Calendar Session

Checkpoint: 2026-10-29 UTC

```text
AGENT_ID: A — Google OAuth Product UI / Trusted Calendar Integration
ACTIVE_PHASE: G8-A / Fake Calendar consent and read-only session integration
GOAL_STATUS: PARTIAL — focused JVM tests and isolated source compilation pass; emulator execution and parent campaign integration remain pending
BASE_SHA: 00384eb6468214362435140c9a2d848fa2e0f12e
CURRENT_HEAD: 692370a306d8db3583ca8190cd6ac297366091ac (source/test HEAD for the latest focused JVM and isolated compilation results)
TREE_SHA: 590d5c3425650cdfa7d35cca6bc00ccc53a193bd
WORKTREE: /workspace/Serea-g8-oauth-ui-mock
BRANCH: codex/serea-g8-oauth-ui-mock
DRAFT_PR: https://github.com/Yrika819/Serea/pull/145 (Draft; base `codex/serea-g7-calendar-readiness`, whose head is the declared BASE_SHA)
```

## Completed work

- Extended the existing `GoogleAuthorizationDemoPanel` with explicit synthetic Account A/B choices, Calendar read-only scope and use-purpose copy, and a consent preview that explains read-only access, disconnection, Cloud, and the separate Chat handoff decision.
- Added `CalendarDemoSessionFactory.openAfterSyntheticConsent(account, accountGeneration)` to the existing ViewModel boundary. The shipped default is `FakeCalendarDemoSessionFactory`, which can only create `CalendarProviderSessions.openFake()` sessions. It has no HTTP client or Google endpoint capability.
- The ViewModel publishes a Calendar session only after a successful synthetic confirm. Account selection, expiry, Calendar disconnect, shared grant revoke, and ViewModel owner teardown close and clear the old session. The selected account and generation are attached to the injected factory call.
- App-local disconnect continues to preserve the synthetic shared grant. Reconnect now requires a fresh displayed confirmation before reusing that grant; the old Calendar session stays invalid. Revoke and account selection clear stale synthetic grants.
- Added a nullable panel callback for parent navigation:

  ```kotlin
  GoogleAuthorizationDemoPanel(
      viewModel: GoogleAuthorizationDemoViewModel = viewModel(),
      onOpenCalendar: ((CalendarProviderSession) -> Unit)? = null,
  )
  ```

  The parent should pass only the ViewModel-provided Fake session to `CalendarAgendaScreen`, and should close the route when `calendarDemoSession` becomes null. No changes were made to `MainActivity`, `SereaAppShell`, `AppProviderComposition`, or Cloud routing.
- Added a test-only loopback factory and Compose harness for the UI → preview/confirm → session → `CalendarAgendaScreen` list/detail path. The harness uses synthetic gateways and `MockWebServer.server.url("/calendar/v3/")`. Account B's hostile description includes a same-server `server.url("/exfil")` and literal `<script>` text; the UI assertion requires plain non-clickable Compose text and exactly four expected localhost Calendar `GET`s, making an accidental resource fetch observable. After switching, Account A receives generation 3 and a distinct session ID, reads an Account A-only agenda marker, and does not see Account B's marker. The total allowlist is six localhost Calendar `GET`s across both accounts. Cancel/deny create no session and make no requests; switching and revoke invalidate the old session.
- Extended the JVM Calendar mock E2E to fetch both an event list and an event detail, then verify account switch prevents another dispatch.
- Added ViewModelStore tests for configuration-store retention, `onCleared()` teardown, and a fresh-store relaunch with no retained grant or session. This is a ViewModel lifecycle contract test, not a physical Android process-death test.
- Direct Calendar reconnect after simulated token expiry now discards the expired synthetic shared grant, opens a fresh exact-scope preview for the same synthetic account, and does not create a new session until explicit Confirm. The old Calendar session remains closed. The EXPIRED-state primary action is labeled `再接続して認可内容を確認`. Both JVM and Settings UI regressions cover this path.
- Added an isolated-variant-only `FakeGoogleProcessDeathHarnessActivity` and host helper. The Activity seeds a Fake Calendar session after explicit consent; the helper is intended to run after instrumentation exits, force-stops only `app.serea.android.isolated`, verifies its PID disappears, relaunches the Activity, and checks disconnected/no-preview/no-session state. It refuses to run while the `.isolated.test` process exists. No campaign script hook or real data mutation is included.
- Added a MainActivity UI test for Account B consent → the consent-owned Fake Calendar route → event detail → local Chat handoff confirmation → synthetic account switch → fail-closed Send. It asserts the consent session is distinct from the ordinary `AppProviderComposition` Calendar session, no assistant response is produced, and Usage counts are unchanged. This UI test compiles, but is not run on an emulator; it needs the parent-owned integration route and focused campaign test list on the exact integrated candidate.
- Fixed the delayed-revocation JVM test by running the expected failed request inside `supervisorScope`, starting its child undispatched, and waiting for MockWebServer to parse the request before revocation. `await()` now captures the expected `CalendarReadFailure` without cancelling the `runTest` parent; production authorization/session behavior is unchanged.

## Changed files

- `android/app/src/main/java/app/serea/android/googleauth/CalendarDemoSessionFactory.kt` (new)
- `android/app/src/main/java/app/serea/android/googleauth/FakeGoogleAuthorizationGateway.kt`
- `android/app/src/main/java/app/serea/android/googleauth/GoogleAuthorizationDemoPanel.kt`
- `android/app/src/main/java/app/serea/android/googleauth/GoogleAuthorizationDemoViewModel.kt`
- `android/app/src/test/java/app/serea/android/calendar/CalendarOAuthMockE2ETest.kt`
- `android/app/src/test/java/app/serea/android/googleauth/GoogleAuthorizationLifecycleTest.kt`
- `android/app/src/test/java/app/serea/android/googleauth/GoogleCalendarDemoLoopbackIntegrationTest.kt` (new)
- `android/app/src/androidTest/java/app/serea/android/GoogleAuthorizationDemoUiTest.kt`
- `android/app/src/androidTest/java/app/serea/android/GoogleCalendarDemoLoopbackUiTest.kt` (new)
- `android/app/src/androidTest/java/app/serea/android/GoogleCalendarHandoffRevocationUiTest.kt` (new)
- `android/app/src/androidTest/assets/serea-non-required-test-identities.txt` (two additional non-required UI identities; Required and Additional manifests unchanged)
- `android/app/src/isolated/AndroidManifest.xml`
- `android/app/src/isolated/java/app/serea/android/FakeGoogleProcessDeathHarnessActivity.kt` (new; isolated variant only)
- `tools/run_android_fake_google_process_death.sh` (new host helper; not wired into a campaign script)
- `tests/android_emulator_workflow_tests.py`
- `docs/agent-reports/g8-agent-a-oauth-ui.md` (this checkpoint)

## Tests and execution evidence

Commands attempted:

- `JAVA_HOME=/usr/lib/jvm/java-21-openjdk-amd64 ./gradlew :app:testDebugUnitTest --tests app.serea.android.googleauth.GoogleAuthorizationLifecycleTest --tests app.serea.android.calendar.CalendarOAuthMockE2ETest` — could not resolve an Android Gradle plugin dependency because the Java process did not use the managed HTTP proxy.
- `JAVA_HOME=/usr/lib/jvm/java-21-openjdk-amd64 ./gradlew --no-daemon -Dhttp.proxyHost=proxy -Dhttp.proxyPort=8080 -Dhttps.proxyHost=proxy -Dhttps.proxyPort=8080 :app:testDebugUnitTest --tests app.serea.android.googleauth.GoogleAuthorizationLifecycleTest --tests app.serea.android.calendar.CalendarOAuthMockE2ETest` — dependency resolution progressed, then Gradle stopped before compilation because `/usr/lib/jvm/java-21-openjdk-amd64` is a JRE without `javac`; error: `Toolchain installation ... does not provide the required capabilities: [JAVA_COMPILER]`.
- `JAVA_HOME=/usr/lib/jvm/java-21-openjdk-amd64 ./gradlew --no-daemon -Dhttp.proxyHost=proxy -Dhttp.proxyPort=8080 -Dhttps.proxyHost=proxy -Dhttps.proxyPort=8080 :app:compileDebugKotlin` — Android resources and KSP ran, then `:app:compileDebugKotlin` stopped while configuring its associated Java compile target for the same missing `javac` capability. No Kotlin source compile or JUnit execution is claimed.
- `JAVA_HOME=/workspace/.tools/temurin17 PATH=/workspace/.tools/temurin17/bin:$PATH ./gradlew --no-daemon --console=plain -Dhttp.proxyHost=proxy -Dhttp.proxyPort=8080 -Dhttps.proxyHost=proxy -Dhttps.proxyPort=8080 -Djavax.net.ssl.trustStore=/workspace/.tools/temurin17/codex-cacerts -Djavax.net.ssl.trustStorePassword=changeit :app:testDebugUnitTest --tests app.serea.android.googleauth.GoogleAuthorizationLifecycleTest --tests app.serea.android.calendar.CalendarOAuthMockE2ETest --tests app.serea.android.googleauth.GoogleCalendarDemoLoopbackIntegrationTest` (run from `android/`) — PASS. `GoogleAuthorizationLifecycleTest`: 9/9; `CalendarOAuthMockE2ETest`: 1/1; `GoogleCalendarDemoLoopbackIntegrationTest`: 3/3; total 13 tests, 0 failures, 0 errors, 0 skipped.
- Latest exact-source JVM command on `692370a306d8db3583ca8190cd6ac297366091ac`: `JAVA_HOME=/workspace/.tools/temurin17 PATH=/workspace/.tools/temurin17/bin:$PATH ./gradlew --no-daemon --console=plain -Dhttp.proxyHost=proxy -Dhttp.proxyPort=8080 -Dhttps.proxyHost=proxy -Dhttps.proxyPort=8080 -Djavax.net.ssl.trustStore=/workspace/.tools/temurin17/codex-cacerts -Djavax.net.ssl.trustStorePassword=changeit :app:testDebugUnitTest --tests app.serea.android.googleauth.GoogleAuthorizationLifecycleTest --tests app.serea.android.calendar.CalendarOAuthMockE2ETest --tests app.serea.android.googleauth.GoogleCalendarDemoLoopbackIntegrationTest` (from `android/`) — PASS, 18/18: Lifecycle 14, Calendar mock E2E 1, loopback integration 3; 0 failures, 0 errors, 0 skipped.
- Latest isolated compilation command on the same source tree: `JAVA_HOME=/workspace/.tools/temurin17 PATH=/workspace/.tools/temurin17/bin:$PATH ./gradlew --no-daemon --console=plain -Dhttp.proxyHost=proxy -Dhttp.proxyPort=8080 -Dhttps.proxyHost=proxy -Dhttps.proxyPort=8080 -Djavax.net.ssl.trustStore=/workspace/.tools/temurin17/codex-cacerts -Djavax.net.ssl.trustStorePassword=changeit :app:compileIsolatedKotlin :app:compileIsolatedAndroidTestKotlin` — PASS. This compiles the harness Activity and the added Android test sources including the EXPIRED recovery label assertion; it does not execute instrumentation.
- `python3 -m unittest tests.android_emulator_workflow_tests.AndroidEmulatorWorkflowTests.test_fake_google_process_death_harness_is_host_driven_and_isolated` — PASS; `bash -n tools/run_android_fake_google_process_death.sh` — PASS; `git diff --check` — PASS.
- Latest JUnit XML SHA-256 on `692370a306d8db3583ca8190cd6ac297366091ac`: Lifecycle `c7d3f3200d5f9480aaa41c1eb4b3f8db6df8338162676ca91171903a88bde2b6`; Calendar mock E2E `a94500699d2c1160034228e75c17cec4248fbb521723c96395b0c70d7490bbcb`; loopback integration `3f11dab36316c906cd266b47699657026d76066eaf40badc23b0aeafd7428b04`.
- `JAVA_HOME=/workspace/.tools/temurin17 PATH=/workspace/.tools/temurin17/bin:$PATH ./gradlew --no-daemon --console=plain -Dhttp.proxyHost=proxy -Dhttp.proxyPort=8080 -Dhttps.proxyHost=proxy -Dhttps.proxyPort=8080 -Djavax.net.ssl.trustStore=/workspace/.tools/temurin17/codex-cacerts -Djavax.net.ssl.trustStorePassword=changeit --rerun-tasks :app:compileIsolatedAndroidTestKotlin` (run from `android/`) — PASS. The isolated Android instrumentation Kotlin sources compiled; no UI/device test execution was performed. Reported Kotlin warnings were in existing unrelated files; the new loopback UI test compiled without an error.
- JUnit XML SHA-256: `GoogleAuthorizationLifecycleTest` `582cf4a2c8b5a8a650d3881f0786a13ac69566dfd2f7d38c7dfc9728d72c97c0`; `CalendarOAuthMockE2ETest` `e00a633c0daec392f8369abc57db0784f8e2cc26014a96bd6e82d8f0938f9e26`; `GoogleCalendarDemoLoopbackIntegrationTest` `a699fe37355f25c4d0759dcf6e3a5d78aa8852d347a8fbbd2e80100134833644`.
- Static source-to-manifest accounting using `verify_android_instrumentation.collect_source_test_identities` and `read_test_manifest`: `Original Required Set=103`, `Additional Tests=2`, `explicit non-required=19`, `source_total=124`, `formal overlap=0`, `unclassified=0`, `missing_from_source=0`. Required and Additional manifests are unchanged. This validates identity accounting only; it does not execute instrumentation tests.
- `git diff --check` — PASS.

JUNIT_RESULT: PASS — latest focused JVM suite 18/18, 0 failures, 0 errors, 0 skipped on source/test HEAD `692370a306d8db3583ca8190cd6ac297366091ac`, tree `590d5c3425650cdfa7d35cca6bc00ccc53a193bd`.
ANDROID_UI_COMPILE: PASS — `compileIsolatedKotlin` and `compileIsolatedAndroidTestKotlin` on the same HEAD; compile-only evidence.
ANDROID_UI_RESULT: NOT_RUN — no emulator or physical-device Compose execution is claimed.
OS_PROCESS_DEATH: NOT_RUN — host helper and isolated Activity compile; no disposable emulator was operated. Run only after instrumentation exits on a verified CI emulator.
CI_RUN_IDS: PR #145 exact source/test ref `692370a306d8db3583ca8190cd6ac297366091ac`: Android `40802450817` PENDING; Fast `40802450811` PENDING; Full `40802450922` PENDING; RustSec `40802450821` PENDING; SQLite `40802450841` PENDING; Native `40802450926` PENDING; Git Hooks `40802450823` PENDING. No exact-head Android result is available yet; earlier runs target older source and are not reused.
ARTIFACT_IDS: none.

## Coverage map (does not claim full A4 completion)

Existing exact-G7-source coverage inherited from base `00384eb` (the G7 runs do not verify this new patch):

- Consent, requested-scope boundary, account/session invalidation, typed denial/expiry, and stale request registry results: `GoogleAuthorizationTest.firstConsentRequestsOnlyRequestedScopesAndIsExplicit`, `missingScopeIsNotTreatedAsAuthorized`, `accountSwitchClearsTheOldAccountGrant`, `consentRegistryRejectsOverlapAndStaleResultsAndAllowsCancellation`, `disableThenReenableRejectsLateAuthorizationTaskResult`.
- Provider-session denial, revocation, delayed response, account switch, generation race, request cancellation, and stale-result delivery rejection: `CalendarProviderSessionsTest.deniedAndExpiredAuthorizationDoNotReachTheHttpServer`, `accountSwitchDuringTokenLeaseAcquisitionCannotDispatchACalendarRequest`, `delayedCalendarResponseIsDeniedWhileRevocationIsPendingAndAfterEveryOutcome`, `grantRevocationCancelsPendingCalendarHttpCallBeforeResponseBodyDelivery`, `delayedCalendarResponseIsDeniedAfterAppLocalDisable`, `replacingSessionImmediatelyAfterLeaseAcquisitionPreventsCalendarHttpDispatch`, `unknownAccountBindingIsNotAuthorizedAndCannotReachCalendarHttp`, `responseAlreadyProducedByOldSessionIsDeniedWhenAccountSwitchWinsBeforeDelivery`, `sharedGrantGenerationRejectsCalendarResultBeforeObserverNotification`.
- Calendar HTTP scope, failure, redirect, body-bound, and cancellation controls: `GoogleCalendarHttpReadPortTest.missingTokenAndInvalidAuthorizationAreRejectedBeforeNetwork`, `accountSwitchDuringTokenLeaseAcquisitionRejectsBeforeHttpDispatch`, `missingScopeDoesNotRequestToken`, `apiFailuresAreMappedWithoutLeakingResponseBody`, `quota403IsRateLimitedAndNoAutomaticRetryOccurs`, `injectedClientCannotForwardBearerTokenThroughRedirect`, `malformedOrOversizedJsonIsRejected`, `coroutineCancellationCancelsHttpCall`.
- Existing G7 one-event synthetic loopback E2E: `CalendarOAuthMockE2ETest.syntheticConsentAllowsOnlyCalendarReadAndAccountSwitchStopsFurtherDispatch`. It had one list GET in the G7 base; this branch adds detail GET coverage, which has not run.
- Existing Settings UI walked consent cancel/deny/confirm, account switch, expiry/shared revoke, and app-local disconnect/reconnect in `GoogleAuthorizationDemoUiTest`. This branch updates its reconnect expectation to include a new confirmation preview.

New coverage authored in this worktree; runtime status is listed per suite:

- JVM PASS: `GoogleAuthorizationLifecycleTest.selectingAnotherSyntheticAccountClosesItsCalendarSessionBeforeReauthorization`, `reconnectAfterAppLocalDisconnectRequiresConfirmationAndReplacesTheOldCalendarSession`; `GoogleCalendarDemoLoopbackIntegrationTest.defaultProductDemoSessionIsFakeAndHasNoHttpDispatchCapability`, `viewModelStoreRetainsDemoSessionForRecreationAndClearsOnOwnerTeardown`, and `consentConfirmOpensLoopbackListAndDetailThenAccountSwitchAndRevokeSuppressOldSession`.
- Android source compile PASS, runtime NOT_RUN: `GoogleAuthorizationDemoUiTest.selectedSyntheticAccountIsBoundToTheConsentPreviewAndRevokedOnSwitch` and `GoogleCalendarDemoLoopbackUiTest.consentUiOpensLoopbackAgendaAndAccountSwitchDisconnectsIt`. The latter includes a synthetic Calendar description with prompt-injection instructions and `<script>` text. It asserts literal non-clickable Compose `Text` and embeds a same-server `/exfil` URL; it expects four localhost read-only `GET`s for Account B list/detail and two for Account A's separate generation 3 session. Account A's marker appears while Account B's marker is absent.
- JVM PASS: `GoogleAuthorizationLifecycleTest.expiredCalendarReconnectOpensFreshPreviewAndKeepsOldSessionClosedUntilConfirm` verifies no session before Confirm, a fresh scope preview and reauthorized session, and persistent closure of the old session. `GoogleAuthorizationDemoUiTest.expiredCalendarReconnectShowsFreshPreviewAndWaitsForConfirm` also asserts the direct reconnect label; source-compiled but runtime NOT_RUN.
- Android source compile PASS, runtime NOT_RUN: `GoogleCalendarHandoffRevocationUiTest.confirmedCalendarHandoffCannotSendAfterSyntheticAccountSwitch`. It covers the integration candidate's consent-owned Settings→Calendar route and stale-after-confirm send boundary. It is intentionally distinct from the ordinary Fake Calendar fixture route.
- Static harness contract PASS: `AndroidEmulatorWorkflowTests.test_fake_google_process_death_harness_is_host_driven_and_isolated`. OS process death remains NOT_RUN.

Remaining unverified by Agent A: exact-head Android Compose execution, the host-driven OS process-death Activity run, real OAuth, and any live service. The passing JVM lifecycle test models ViewModelStore recreation and owner teardown, not physical process death. Cloud stays disabled; this lane does not enable or route Google content to Cloud.

## Security impact and integration patch

- Default path is Fake-only and cannot dispatch network requests. The loopback HTTP factory exists in test source only. No Google endpoint, real account, OAuth token, Cloud credential, or billing is used.
- Calendar read authorization does not grant Chat handoff or Cloud egress; the preview says these are separate choices. Existing Cloud policy and routing code are unchanged.
- **Parent Integration Patch already represented on candidate `062efc795919677800d7140c4f70991a30f65750`:** keep the Settings panel's `onOpenCalendar` callback bound to the exact ViewModel-owned Fake session; the nested Calendar route and handoff coordinator must use that same session and close on account switch/revoke. `GoogleCalendarHandoffRevocationUiTest` targets this route. Do not substitute the ordinary `AppProviderComposition` Calendar fixture or a real Play Services gateway.
- **Parent campaign patch proposal (not applied in this lane):** in the parent candidate's `tools/run_android_instrumentation_campaign.sh`, add `GoogleCalendarDemoUiTest#expiredCalendarReconnectShowsFreshPreviewAndWaitsForConfirm` and `GoogleCalendarHandoffRevocationUiTest#confirmedCalendarHandoffCannotSendAfterSyntheticAccountSwitch` to the API 35 `google_demo_ui_tests` here-doc, add the new UI test class to `google_demo_ui_filter`, and update focused expected count from 7 to 9. Immediately after `run_diagnostic_stage google-calendar-and-gmail-ui ...` returns successfully and before the post-stage uninstall loop (currently near lines 724–733), invoke `run_android_fake_google_process_death.sh` with `EXPECTED_API_LEVEL`, `SEREA_ISOLATED_INSTRUMENTATION_FINISHED=1`, a run/attempt-unique token, and `SEREA_FAKE_GOOGLE_DEATH_EVIDENCE_DIR="$evidence/stages/google-calendar-and-gmail-ui/fake-google-process-death"`. Require the helper status to pass before uninstalling. This is after the Gradle instrumentation process has exited; the helper independently refuses to continue while `.isolated.test` has a PID. Expected files: `status.txt`, `host-guard.txt`, `seed-am-start.txt`, `seed-logcat.txt`, `seed-pid.txt`, `post-force-stop-pid.txt`, `verify-am-start.txt`, `verify-logcat.txt`, and `verify-pid.txt`, under the already uploaded API evidence tree.

SECURITY_IMPACT: Fake-only product composition; test-only synthetic token and localhost MockWebServer. Cloud remains disabled. Old sessions close on all account/revoke/disconnect/expiry/owner teardown paths.
KNOWN_ISSUES: API 35 campaign has not yet added the two new non-required UI identities to its focused-stage list or invoked the host process-death helper; no emulator test has run.
CURRENT_BLOCKERS: Exact integrated candidate CI/emulator run is required for MainActivity UI and host process-death evidence.
DEPENDENCIES: Parent integration candidate commit `062efc795919677800d7140c4f70991a30f65750`, updated focused UI campaign list, then exact-head API 35 CI.
INTEGRATION_READY: READY_FOR_PARENT_REVIEW; host campaign hook and CI evidence remain parent-owned.
REVIEW_REQUEST: Review expired shared-grant recovery, the new consent-owned MainActivity handoff test, isolated Activity manifest exposure/host gating, and the proposed campaign insertion point.
NEXT_ACTION: Source/test HEAD `692370a306d8db3583ca8190cd6ac297366091ac` is pushed to PR #145; parent adds the two UI identities and host helper invocation to the API 35 campaign, then reruns exact-head focused UI and process-death stages.
OWNER_APPROVAL_REQUIRED: None for Fake/loopback work. Real OAuth/account/data, Pixel 7a, production signing/distribution, main merge, and Personal Beta GO remain owner-controlled.
```
