# G8 Agent A — Fake Google Connection UI / Calendar Session

Checkpoint: 2026-10-29 UTC

```text
AGENT_ID: A — Google OAuth Product UI / Trusted Calendar Integration
ACTIVE_PHASE: G8-A / Fake Calendar consent and read-only session integration
GOAL_STATUS: PARTIAL — implementation, source/manifest identity accounting, and integration tests authored; runtime execution blocked by missing JDK compiler; parent composition wiring pending
BASE_SHA: 00384eb6468214362435140c9a2d848fa2e0f12e
CURRENT_HEAD: 00384eb6468214362435140c9a2d848fa2e0f12e (uncommitted changes are in the named worktree)
TREE_SHA: base tree 10d07a8ec57d7f67fc94663c489347938cccbd58; changed-tree SHA will be recorded after source commit
WORKTREE: /workspace/Serea-g8-oauth-ui-mock
BRANCH: codex/serea-g8-oauth-ui-mock
DRAFT_PR: none yet — waiting for parent diff review before commit/push
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
- `android/app/src/androidTest/assets/serea-non-required-test-identities.txt` (two new non-required UI identities; Required and Additional manifests unchanged)
- `docs/agent-reports/g8-agent-a-oauth-ui.md` (this checkpoint)

## Tests and execution evidence

Commands attempted:

- `JAVA_HOME=/usr/lib/jvm/java-21-openjdk-amd64 ./gradlew :app:testDebugUnitTest --tests app.serea.android.googleauth.GoogleAuthorizationLifecycleTest --tests app.serea.android.calendar.CalendarOAuthMockE2ETest` — could not resolve an Android Gradle plugin dependency because the Java process did not use the managed HTTP proxy.
- `JAVA_HOME=/usr/lib/jvm/java-21-openjdk-amd64 ./gradlew --no-daemon -Dhttp.proxyHost=proxy -Dhttp.proxyPort=8080 -Dhttps.proxyHost=proxy -Dhttps.proxyPort=8080 :app:testDebugUnitTest --tests app.serea.android.googleauth.GoogleAuthorizationLifecycleTest --tests app.serea.android.calendar.CalendarOAuthMockE2ETest` — dependency resolution progressed, then Gradle stopped before compilation because `/usr/lib/jvm/java-21-openjdk-amd64` is a JRE without `javac`; error: `Toolchain installation ... does not provide the required capabilities: [JAVA_COMPILER]`.
- `JAVA_HOME=/usr/lib/jvm/java-21-openjdk-amd64 ./gradlew --no-daemon -Dhttp.proxyHost=proxy -Dhttp.proxyPort=8080 -Dhttps.proxyHost=proxy -Dhttps.proxyPort=8080 :app:compileDebugKotlin` — Android resources and KSP ran, then `:app:compileDebugKotlin` stopped while configuring its associated Java compile target for the same missing `javac` capability. No Kotlin source compile or JUnit execution is claimed.
- Static source-to-manifest accounting using `verify_android_instrumentation.collect_source_test_identities` and `read_test_manifest`: `Original Required Set=103`, `Additional Tests=2`, `explicit non-required=17`, `source_total=122`, `formal overlap=0`, `unclassified=0`, `missing_from_source=0`. This validates identity accounting only; it does not execute instrumentation tests or alter either formal manifest.
- `git diff --check` — PASS.

JUNIT_RESULT: NOT_RUN — no JUnit totals are available for this worktree.
ANDROID_UI_RESULT: NOT_RUN — the new Compose loopback test is authored but not executed.
CI_RUN_IDS: none for this worktree.
ARTIFACT_IDS: none.

## Coverage map (does not claim full A4 completion)

Existing exact-G7-source coverage inherited from base `00384eb` (the G7 runs do not verify this new patch):

- Consent, requested-scope boundary, account/session invalidation, typed denial/expiry, and stale request registry results: `GoogleAuthorizationTest.firstConsentRequestsOnlyRequestedScopesAndIsExplicit`, `missingScopeIsNotTreatedAsAuthorized`, `accountSwitchClearsTheOldAccountGrant`, `consentRegistryRejectsOverlapAndStaleResultsAndAllowsCancellation`, `disableThenReenableRejectsLateAuthorizationTaskResult`.
- Provider-session denial, revocation, delayed response, account switch, generation race, request cancellation, and stale-result delivery rejection: `CalendarProviderSessionsTest.deniedAndExpiredAuthorizationDoNotReachTheHttpServer`, `accountSwitchDuringTokenLeaseAcquisitionCannotDispatchACalendarRequest`, `delayedCalendarResponseIsDeniedWhileRevocationIsPendingAndAfterEveryOutcome`, `grantRevocationCancelsPendingCalendarHttpCallBeforeResponseBodyDelivery`, `delayedCalendarResponseIsDeniedAfterAppLocalDisable`, `replacingSessionImmediatelyAfterLeaseAcquisitionPreventsCalendarHttpDispatch`, `unknownAccountBindingIsNotAuthorizedAndCannotReachCalendarHttp`, `responseAlreadyProducedByOldSessionIsDeniedWhenAccountSwitchWinsBeforeDelivery`, `sharedGrantGenerationRejectsCalendarResultBeforeObserverNotification`.
- Calendar HTTP scope, failure, redirect, body-bound, and cancellation controls: `GoogleCalendarHttpReadPortTest.missingTokenAndInvalidAuthorizationAreRejectedBeforeNetwork`, `accountSwitchDuringTokenLeaseAcquisitionRejectsBeforeHttpDispatch`, `missingScopeDoesNotRequestToken`, `apiFailuresAreMappedWithoutLeakingResponseBody`, `quota403IsRateLimitedAndNoAutomaticRetryOccurs`, `injectedClientCannotForwardBearerTokenThroughRedirect`, `malformedOrOversizedJsonIsRejected`, `coroutineCancellationCancelsHttpCall`.
- Existing G7 one-event synthetic loopback E2E: `CalendarOAuthMockE2ETest.syntheticConsentAllowsOnlyCalendarReadAndAccountSwitchStopsFurtherDispatch`. It had one list GET in the G7 base; this branch adds detail GET coverage, which has not run.
- Existing Settings UI walked consent cancel/deny/confirm, account switch, expiry/shared revoke, and app-local disconnect/reconnect in `GoogleAuthorizationDemoUiTest`. This branch updates its reconnect expectation to include a new confirmation preview.

New coverage authored in this worktree, all NOT_RUN:

- `GoogleAuthorizationDemoUiTest.selectedSyntheticAccountIsBoundToTheConsentPreviewAndRevokedOnSwitch`.
- `GoogleAuthorizationLifecycleTest.selectingAnotherSyntheticAccountClosesItsCalendarSessionBeforeReauthorization` and `reconnectAfterAppLocalDisconnectRequiresConfirmationAndReplacesTheOldCalendarSession`.
- `GoogleCalendarDemoLoopbackIntegrationTest.defaultProductDemoSessionIsFakeAndHasNoHttpDispatchCapability`, `viewModelStoreRetainsDemoSessionForRecreationAndClearsOnOwnerTeardown`, and `consentConfirmOpensLoopbackListAndDetailThenAccountSwitchAndRevokeSuppressOldSession`.
- `GoogleCalendarDemoLoopbackUiTest.consentUiOpensLoopbackAgendaAndAccountSwitchDisconnectsIt`, including a synthetic Calendar description with prompt-injection instructions and `<script>` text. It asserts the payload is rendered as a literal Compose `Text` node with no click action and embeds an observable same-server `/exfil` URL; it expects exactly four localhost read-only `GET`s for Account B list/detail and two for Account A's separate generation 3 session. Account A's marker appears while Account B's marker is absent. This test is authored but NOT_RUN.

Remaining unverified by Agent A: a physical Activity recreation / process death run, execution of the UI-to-session, account-isolation, and hostile-description assertions, real OAuth, and any live service. These Compose assertions are authored but NOT_RUN because this environment lacks `javac`; they are not runtime-verified evidence. Cloud stays disabled; this lane does not enable or route Google content to Cloud.

## Security impact and integration patch

- Default path is Fake-only and cannot dispatch network requests. The loopback HTTP factory exists in test source only. No Google endpoint, real account, OAuth token, Cloud credential, or billing is used.
- Calendar read authorization does not grant Chat handoff or Cloud egress; the preview says these are separate choices. Existing Cloud policy and routing code are unchanged.
- **Parent Integration Patch:** when displaying the existing Settings panel, supply `onOpenCalendar = { session -> navigateToCalendar(session) }`; in the Calendar destination pass `session` as both read and authorization ports. Return to the connection screen on Back. Also observe `calendarDemoSession` invalidation and clear the destination/session so an old account's visible event state cannot remain after account switch or revoke. Keep `AppProviderComposition`'s ordinary default Fake; do not wire a real Play Services gateway from Settings.

SECURITY_IMPACT: Fake-only product composition; test-only synthetic token and localhost MockWebServer. Cloud remains disabled. Old sessions close on all account/revoke/disconnect/expiry/owner teardown paths.
KNOWN_ISSUES: Parent composition callback not wired yet. New tests cannot run in the current environment because Java compiler is missing.
CURRENT_BLOCKERS: JDK compiler/CI runtime required for execution evidence.
DEPENDENCIES: Parent integration in `SereaAppShell`/`MainActivity`, then focused JVM + Android UI verification at the integration SHA.
INTEGRATION_READY: PENDING parent review and composition patch.
REVIEW_REQUEST: Review `CalendarDemoSessionFactory`, account reconnect confirmation lifecycle, new JVM/Compose loopback tests, and the route invalidation integration note.
NEXT_ACTION: Commit with approved noreply identity, push isolated branch, open Draft PR if GitHub authorization permits, and record exact PR/CI state; parent composition wiring and runtime test evidence remain follow-up dependencies.
OWNER_APPROVAL_REQUIRED: None for Fake/loopback work. Real OAuth/account/data, Pixel 7a, production signing/distribution, main merge, and Personal Beta GO remain owner-controlled.
```
