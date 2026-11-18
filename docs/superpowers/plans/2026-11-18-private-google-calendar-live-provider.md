# Private Primary Google Calendar Live Provider Wiring Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Connect Private authorization to the existing Primary-only Calendar provider path while keeping every product variant's Google HTTP gate hardcoded off and validating the live provider route only through a test-only MockWebServer composition.

**Architecture:** Keep `PrivateGoogleCalendarComposition` as the only product provider-selection point, derive its Live permission from a new independent BuildConfig field that is statically false in every product variant, and reuse `CalendarProviderSessions.openPrimaryGoogleReadOnly()`. Inject OkHttp client, MockWebServer URL and synthetic authorization only through test construction of the internal ViewModel/composition seam; never add a user-facing or environment-driven HTTP enable switch.

**Tech Stack:** Kotlin, Android Gradle Plugin, Jetpack Compose, Lifecycle/ViewModel, Kotlin coroutines, OkHttp, MockWebServer, JUnit 4, Android Compose UI tests.

**Spec:** `docs/superpowers/specs/2026-11-18-private-google-calendar-live-provider-design.md`

## Global Constraints

- Base all changes on main `077246aeab909928fb57316f1f3aefc6be69c0b3`; do not modify or merge PRs #232, #239, or #243.
- Keep `PRIVATE_GOOGLE_CALENDAR_HTTP_ENABLED=false` for Public, Debug/Demo, Beta, and Private Beta; do not add an environment/property/UI setting that can change it.
- Keep OAuth and HTTP gates separate; OAuth authorization alone always leaves the product Provider Fake.
- Permit only `calendarId=primary` and `calendar.events.readonly`; reject CalendarList, Gmail and write operations before token acquisition or HTTP.
- Reuse `CalendarProviderSessions`, `GoogleCalendarAuthorizationAdapter`, `GoogleCalendarHttpReadPort`, and current Session invalidation/retry behavior.
- Test HTTP only against MockWebServer loopback using synthetic tokens; never execute real OAuth, Google HTTP, Cloud changes, signing, APK distribution, or physical-device operations.
- Preserve the existing Private Beta CI packaging restriction and all test counts/assertions; do not disable or weaken Required checks.
- Do not edit the dirty main worktree; work only in `.worktrees/h4-d1-d2-private-google-calendar` on `codex/h4-d1-d2-private-google-calendar`.

---

### Task 1: Freeze the default-off HTTP gate contract

**Files:**
- Modify: `android/app/build.gradle.kts`
- Modify: `android/app/src/testPrivateBeta/java/app/serea/android/googleauth/PrivateBuildOAuthGateTest.kt`
- Modify: `android/app/src/testDebug/java/app/serea/android/googleauth/PrivateGoogleAuthorizationViewModelTest.kt`

**Interfaces:**
- Produces BuildConfig Boolean `PRIVATE_GOOGLE_CALENDAR_HTTP_ENABLED`, generated as literal `false` in `defaultConfig`, `beta`, and `privateBeta`; no Gradle property or environment variable controls it.
- Preserves `OWNER_PRIVATE_BETA_GOOGLE_OAUTH_ENABLED` as an independent, existing OAuth-only gate.

- [ ] **Step 1: Add failing assertions before implementation.** In `PrivateBuildOAuthGateTest.oauthGateRequiresBothPrivateVariantAndCallsiteOptIn`, add `assertFalse(BuildConfig.PRIVATE_GOOGLE_CALENDAR_HTTP_ENABLED)` and retain the existing Private applicationId/version assertions. In the Debug test `publicDebugBuildDoesNotExposePrivateGoogleAuthorizationUi`, add the same HTTP-gate assertion.
- [ ] **Step 2: Run the two gate tests and confirm the new BuildConfig reference fails to compile.** Run from `android/` with JDK 17 and `ANDROID_HOME=/Users/yuta/Library/Android/sdk`:

```bash
JAVA_HOME=/Users/yuta/Library/Java/JavaVirtualMachines/temurin-17.jdk/Contents/Home ANDROID_HOME=/Users/yuta/Library/Android/sdk ANDROID_SDK_ROOT=/Users/yuta/Library/Android/sdk ./gradlew --no-daemon :app:testDebugUnitTest --tests 'app.serea.android.googleauth.PrivateGoogleAuthorizationViewModelTest.publicDebugBuildDoesNotExposePrivateGoogleAuthorizationUi'
```

Expected: compilation fails because `PRIVATE_GOOGLE_CALENDAR_HTTP_ENABLED` is not yet generated.
- [ ] **Step 3: Add the literal-false BuildConfig field.** Add `buildConfigField("boolean", "PRIVATE_GOOGLE_CALENDAR_HTTP_ENABLED", "false")` to `defaultConfig`, and explicit `false` declarations to `beta` and `privateBeta` beside their other Google fields. Do not read any environment variable or Gradle property for this field.
- [ ] **Step 4: Run the Debug gate test and the existing Private Beta gate test.** Run `:app:testDebugUnitTest --tests 'app.serea.android.googleauth.PrivateGoogleAuthorizationViewModelTest.publicDebugBuildDoesNotExposePrivateGoogleAuthorizationUi'` and `:app:testPrivateBetaUnitTest --tests 'app.serea.android.googleauth.PrivateBuildOAuthGateTest' -PsereaPrivateBetaVersionCode=2` with JDK 17, SDK path set, and `SEREA_OWNER_PRIVATE_BETA_GOOGLE_OAUTH_ENABLED=false`. Expected: both pass; the Private HTTP field remains false independently of OAuth.
- [ ] **Step 5: Commit the gate contract.** Commit only `android/app/build.gradle.kts` and the two gate test files with message `test(android): lock private calendar HTTP gate off`.

### Task 2: Route authorized Private sessions through the selected provider

**Files:**
- Modify: `android/app/src/main/java/app/serea/android/googleauth/PrivateGoogleCalendarComposition.kt`
- Modify: `android/app/src/main/java/app/serea/android/googleauth/PrivateGoogleAuthorizationViewModel.kt`
- Modify: `android/app/src/main/java/app/serea/android/googleauth/PrivateGoogleAuthorizationPanel.kt`
- Create: `android/app/src/testDebug/java/app/serea/android/googleauth/PrivateGoogleCalendarCompositionTest.kt`
- Modify: `android/app/src/testDebug/java/app/serea/android/googleauth/PrivateGoogleAuthorizationViewModelTest.kt`

**Interfaces:**
- `PrivateGoogleCalendarComposition.openAuthorizedPrimary(gateway: GoogleAuthorizationGateway): CalendarProviderSession` validates authorization and selects Fake when the product HTTP gate is false; Live selection calls `openPrimaryGoogleReadOnly()` only when the Private-variant and HTTP gates are both true.
- `PrivateGoogleAuthorizationViewModel.openPrimaryCalendar(): CalendarProviderSession?` replaces the Fake-specific opening method; the default factory always uses product composition with literal-false HTTP BuildConfig and Private-variant defaults.
- Add a `calendarCompositionFactory: () -> PrivateGoogleCalendarComposition` constructor seam with the production default. Tests may instantiate a separate composition with explicit test-only gate overrides, loopback client/base URL and synthetic gateway; product factory code supplies none.

- [ ] **Step 1: Write failing composition contract tests.** Assert (a) authorized `MOCK_ONLY` and `PLAY_SERVICES` gateways both open `FAKE` with `primaryCalendarOnly=true` under default product composition; (b) a test composition with `googleHttpEnabledByOwner=true`, MockWebServer URL and synthetic `PLAY_SERVICES` gateway returns `GOOGLE_READ_ONLY`; (c) HTTP-enabled composition rejects a `MOCK_ONLY` gateway and absent/stale session before status/token/HTTP; (d) missing required scope raises a readable failure without token or HTTP; (e) a selected Google session's transport failure remains a failure and is never replaced by Fake.
- [ ] **Step 2: Run `:app:testDebugUnitTest --tests 'app.serea.android.googleauth.PrivateGoogleCalendarCompositionTest'` and confirm the new tests fail because the API is not implemented.** Use JDK 17 and the existing SDK path. Do not add a test dependency; MockWebServer 4.12.0 is already present in `testImplementation`.
- [ ] **Step 3: Implement the composition with existing sessions.** Keep the private-primary Fake fixture; default constructor values are `privateVariantEnabled = BuildConfig.PRIVATE_GOOGLE_AUTH_UI_ENABLED` and `googleHttpEnabledByOwner = BuildConfig.PRIVATE_GOOGLE_CALENDAR_HTTP_ENABLED`, whose product value is literal false. Test-only construction may explicitly pass both gates as true plus an OkHttp client and loopback base URL. Construct `CalendarProviderSessions` with the conjunction of those two values. Check the exact `PrivatePrimaryCalendarScopeContract.scopes` set and current status before opening. When both gates select Live, check `gateway.mode == PLAY_SERVICES` and a nonblank authorization session before status; call `sessions.openPrimaryGoogleReadOnly(gateway, client, baseUrl)` and reject rather than accept any Fake session returned by the lower-level guard. Never catch a read failure and substitute a Fake provider.
- [ ] **Step 4: Inject composition into the ViewModel and rename the open action.** Replace `openFakePrimaryCalendarPreview()` with `openPrimaryCalendar()`, preserve the exact authorized-state and real-vs-fake gateway checks, close any previous session before replacement, and surface provider-construction exceptions through a redacted UI message while returning `null`. Continue closing sessions on account changes, reconnect, in-app disconnect, revoke, authorization failure and `onCleared()`.
- [ ] **Step 5: Update the Private panel to use neutral provider selection and honest provider labels.** Call `openPrimaryCalendar()` and choose the button label from `PrivateGoogleCalendarComposition.opensGoogleProvider`, which is false for every product build: product builds say `架空のPrimary予定を表示`; only a test/live-selected composition says `Google CalendarのPrimary予定を表示`. Keep `CalendarAgendaScreen` as the only agenda surface and retain its existing Fake/Google data labels.
- [ ] **Step 6: Run the composition and ViewModel tests.** Expected: Fake defaults, test-only Google provider selection, primary-only flag, exact status/token scopes and close semantics pass; existing authorization generation/revoke tests remain green.
- [ ] **Step 7: Commit the Provider selection change.** Commit only the composition, ViewModel, panel and focused tests with message `feat(android): wire private primary calendar provider selection`.

### Task 3: Add Private UI-to-agenda MockWebServer E2E coverage

**Files:**
- Create: `android/app/src/androidTest/java/app/serea/android/PrivateGoogleCalendarLoopbackUiTest.kt`
- Modify: `android/app/src/androidTest/assets/serea-non-required-test-identities.txt` only if the new instrumentation identity is required by the existing runner's explicit identity manifest.
- Reuse: `PrivateGoogleAuthorizationPanel`, `PrivateGoogleAuthorizationViewModel`, `CalendarProviderSessions`, `CalendarAgendaScreen`, existing MockWebServer/OkHttp test dependencies.

**Interfaces:**
- The synthetic test Gateway returns only `calendar.events.readonly`, a short-lived in-memory synthetic token and deterministic session identity. It never instantiates Play Services or contacts Google.
- The test composition injects `server.url("/calendar/v3/")` and a test OkHttp client into the Private ViewModel; HTTP host must be loopback and the server is shut down after each test.

- [ ] **Step 1: Write the E2E tests before implementation updates.** Add one test for the Private panel's explicit authorization action opening a Google-backed agenda and traversing a two-page Primary event list into event detail; add cases for all-day event/date semantics, offset time plus zone, zero events, manual refresh, and closing the provider session. Queue only synthetic Calendar-compatible JSON.
- [ ] **Step 2: Run the focused instrumentation test and confirm the expected failure is the missing provider/open action or missing test composition seam.** Use the existing isolated instrumentation variant, not `privateBeta` APK packaging.
- [ ] **Step 3: Complete the injected test composition and synthetic authorization fixture.** Call the same `CalendarProviderSessions.openPrimaryGoogleReadOnly()` API used by production; record requested/status scopes and token-call count in memory, with no token/account/event-content logging.
- [ ] **Step 4: Assert the external contract.** Verify every server request is GET to `/calendar/v3/calendars/primary/events`, has only the synthetic bearer token, and contains no CalendarList, Gmail, non-Primary ID or write operation. Verify UI labels `Google Calendarから取得 · 変更はできません` only for `GOOGLE_READ_ONLY` and Fake labels for default product composition.
- [ ] **Step 5: Run the focused E2E on available API 26 and API 35 emulators.** If an emulator/API image or runner is unavailable, record `NOT_RUN`/`HOST_INFRA` and preserve that gap; do not claim a pass.
- [ ] **Step 6: Commit the E2E.** Commit only the new E2E and any explicitly-required identity-manifest entry with message `test(android): cover private calendar loopback UI flow`.

### Task 4: Close integration-boundary and regression coverage

**Files:**
- Modify only when a directly relevant assertion is missing: `android/app/src/test/java/app/serea/android/calendar/transport/GoogleCalendarHttpReadPortTest.kt`, `android/app/src/test/java/app/serea/android/calendar/CalendarProviderSessionsTest.kt`, `android/app/src/test/java/app/serea/android/calendar/CalendarOAuthMockE2ETest.kt`, or `android/app/src/testDebug/java/app/serea/android/googleauth/PrivateGoogleAuthorizationViewModelTest.kt`.

**Interfaces:**
- Existing transport/session tests remain the contract for preflight primary-only checks, retry budgets, cancellation, stale-session handling, revoke invalidation and bounded error mapping. Do not duplicate these in a broad new test harness.

- [ ] **Step 1: Inventory existing test names and assertions for non-Primary preflight, CalendarList denial, scope/token failure, 401, 403, 429 retry bounds, timeout, malformed JSON, duplicate page token, account/session switch, cancellation, grant revoke and stale-session delivery.** Read `@Test` bodies; do not infer coverage from names alone.
- [ ] **Step 2: Add only missing focused tests before changing behavior.** For each added case, assert zero status/token/HTTP calls when a disallowed operation is rejected; for 401, assert only the rejected credential generation is invalidated; for 403/429, assert the existing bounded attempt count; for selected-Live failures, assert no Fake data reaches the agenda.
- [ ] **Step 3: Run all focused Calendar, HTTP, authorization and Private ViewModel test classes.** Preserve cancellation propagation and all existing failure assertions.
- [ ] **Step 4: Commit focused security regressions separately.** Use message `test(android): preserve primary calendar failure boundaries`.

### Task 5: Deliver the future-activation prerequisite and Owner checklist

**Files:**
- Create: `docs/security/private-google-calendar-live-activation-checklist.md`
- Modify the approved design document only if the implementation reveals a factual mismatch.

**Interfaces:**
- The checklist explains Cloud project/audience and Calendar API prerequisites, exact Android package/certificate identity matching, minimum scope, verification/privacy review, live-gate code change approval, test account approval and explicit real-HTTP approval.

- [ ] **Step 1: Write the checklist with no credentials or account/event data.** State that this PR makes no Cloud changes and the code gate stays false. Require the Owner to confirm the final Private Beta application ID and Owner-controlled signing certificate SHA-1 before creating an Android OAuth client; do not generate/sign with keys.
- [ ] **Step 2: Explicitly list the allowed scope (`calendar.events.readonly`), prohibited CalendarList/Gmail/write scopes, consent audience/test-user decision, Calendar API enablement, sensitive-scope policy review, data-retention/egress review, separate Owner authorization to change the code gate, and separate approval before real OAuth/API access.
- [ ] **Step 3: Run the repository documentation validator and `git diff --check`.** Do not edit the open roadmap PR or its files.
- [ ] **Step 4: Commit the checklist.** Use message `docs(security): record private calendar activation prerequisites`.

### Task 6: Verify, review independently, push and open a Draft PR

**Files:**
- No additional product files unless a directly relevant test failure requires a focused fix.

**Interfaces:**
- The final Draft PR targets `main`, includes only this feature's code/tests/docs, and keeps all gates off.
- Independent review covers the modified Provider composition and direct dependencies; it is not represented as GitHub APPROVED.

- [ ] **Step 1: Run focused JVM tests with JDK 17 and Android SDK path explicitly set.** Include `PrivateGoogleAuthorizationViewModelTest`, `PrivateGoogleCalendarCompositionTest`, `PrivateBuildOAuthGateTest`, `CalendarOAuthMockE2ETest`, `CalendarProviderSessionsTest` and `GoogleCalendarHttpReadPortTest`.
- [ ] **Step 2: Run applicable Public/Private variant-isolation tests and Debug unit tests.** Keep the existing Private Beta CI task restricted to allowlisted JVM tests; do not invoke or authorize Private APK packaging in CI.
- [ ] **Step 3: Run API 26 and API 35 Formal/instrumented tests where emulator infrastructure is available.** Record each run with exact variant, API, commit SHA and test counts; distinguish `NOT_RUN`, `HOST_INFRA`, `TEST_FAILURE` and `DELIVERED_REJECTED`.
- [ ] **Step 4: Run documentation validation, secret/token/account-data scans, `git diff --check`, and inspect the complete diff against `origin/main`.** Confirm no `gmail.readonly`, write scope, real token, account name, event text in generated artifact, build-flag override, signer operation or unrelated signing/Tasks/AI changes entered the patch.
- [ ] **Step 5: Request an independent scoped review** of the final diff and direct dependencies. Resolve Critical/High findings, then rerun the affected tests and review. Report Medium/Low findings and coverage limits.
- [ ] **Step 6: Fetch latest `main` and PR statuses before publication.** If `main` advanced, merge the new main into this branch without rewriting history and rerun affected validations. Never update PRs #232, #239, or #243.
- [ ] **Step 7: Commit remaining task changes, push the dedicated branch and create a new Draft PR against `main`.** Do not remove Draft status, merge, force-push, sign or distribute an APK.
- [ ] **Step 8: Wait for and record same-HEAD Required CI (all four checks), build provenance, non-Required failures, PR URL, HEAD/Tree, independent-review findings and outstanding Owner decisions.** If checks/review/API tests are incomplete, report `PARTIAL` or `BLOCKED`; report `READY_FOR_OWNER_REVIEW` only when all declared acceptance gates are evidenced.
