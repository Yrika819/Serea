# G9 Fake Google / Gmail UI Test Fixes

**AGENT_ID:** B — Android Product / Google / Gmail
**ACTIVE_PHASE:** G9 Fake UI reliability
**GOAL_STATUS:** PARTIAL — test source compiles on the PR #151 successor; updated emulator JUnit/UI evidence is pending.
**BASE_SHA:** `d4a36fb5d3613bca2d60151de0991a1baac72fc9` (PR #151)
**BASE_TREE_SHA:** `b78a84b60ef5a2d344eabdda85b1154e62be46d7`
**VERIFIED_TEST_SOURCE_SHA:** `b553d68325b7c18dfbecaea2a9e92ebd15adad27`
**VERIFIED_TEST_SOURCE_TREE:** `c494e20d34a961d16d67c4c882bc2eeb7b3b70f0`
**BRANCH:** `codex/serea-g9-google-gmail-ui-fixes-r2`
**DRAFT_PR:** [#152](https://github.com/Yrika819/Serea/pull/152), OPEN / DRAFT, stacked on PR #151 head `d4a36fb5d3613bca2d60151de0991a1baac72fc9`.
**WORKTREE:** `/workspace/serea-gmail-fix-r2`
**OWNER_APPROVAL_REQUIRED:** None for Fake-only UI tests. No real Google accounts/data, OAuth, Cloud AI, production signing or device actions were used.

## Exact failing baseline

PR #149 Android run `40810824093`, attempt 1, completed `FAILURE` on source `3a0f0c1897f2115549e7682d6fcc1783c44604f3`, tree `6a1f4d382907a7deb7d6dbf8147e6490aa054e81`. The API 26 artifact is `12872143095`, SHA-256 `ebdd9e7a9f5343e9dfacba91abed7ba75b4bf56a338fa35cbe3582160ae4f34e`. The API 35 artifact is `12872631908`, SHA-256 `bd32cb1a21e26a257c429bc74bd0158f41c819072af835204f953232dc4f8978`.

API 26 Required instrumentation passed **105/105, 0 failures, 0 errors, 0 skips**. Process Death and data-isolation/sentinel evidence passed. Its separate Fake Google/Gmail UI suite ran **16 tests, 5 failures, 0 errors, 0 skips**:

1. `GoogleAuthorizationDemoUiTest#accountSwitchExpiryAndSharedRevocationInvalidateDemoAuthorization`: text selector `架空アカウント B` matched three nodes (intro, account button, status message).
2. `GoogleAuthorizationDemoUiTest#expiredCalendarReconnectShowsFreshPreviewAndWaitsForConfirm`: reconnect copy matched both Gmail and Calendar actions.
3. `GoogleCalendarDemoLoopbackUiTest#consentUiOpensLoopbackAgendaAndAccountSwitchDisconnectsIt`: `createAndroidComposeRule<MainActivity>()` had already populated content before the test called `setContent` again.
4. `GoogleAuthorizationAppIntegrationTest#settingsConsentOpensCalendarSessionAndActivityRecreationReturnsToConsentPanel`: timed out at the wait for `Design review`, before Activity recreation.
5. `GoogleCalendarHandoffRevocationUiTest#confirmedCalendarHandoffCannotSendAfterSyntheticAccountSwitch`: `assertTextContains("B")` used exact matching against the full selected-account label.

For failure 4, the Fake session is account-bound in authorization state but always opens the same `FakeCalendarProvider` fixture. I inspected `FakeCalendarDemoSessionFactory`, `FakeCalendarProvider.Fixtures`, `CalendarAgendaUiController`, and `CalendarAgendaScreen`: the session lists `demo-primary` first; the `Design review` fixture starts at `2026-10-20T00:00:00Z`; and a selected date is queried in `ZoneId.systemDefault()`. The shell session wrapper does not expose the fake provider clock, so the screen uses its UTC fallback clock, while the selected-date range uses the device timezone. The baseline artifact does not record the emulator timezone. Therefore a device-zone date mismatch is plausible, but the original timeout cause is **not confirmed**.

API 35 has a separate bounded Test IME cursor diagnostic failure in `SereaAppShellTest#chatDraftSurvivesGmailAndCalendarDestinationChanges`. API 35 Required instrumentation and the Google/Gmail UI stage were `NOT_RUN`; Data Isolation and Process Death passed. This shared diagnostic is outside this patch and remains a blocker to API 35 UI acceptance.

## Test-only changes

The patch changes four Android instrumentation test files only:

- Google authorization account-switch and reconnect assertions now use their unique semantics tags.
- The loopback Calendar Compose test uses `createComposeRule()` because it supplies the content itself.
- Handoff account-label assertion uses substring matching.
- The two tests that open `Design review` derive its query date by converting the fixture instant to the app device timezone. The app-shell integration test verifies the authorized Fake session lists `demo-primary` and that “Demo Personal” is selected. Failure context includes the timezone and date used.

No application or shared composition code, Required test identity, test threshold, or gate predicate changed. The timezone change is intended to make the test query deterministic under the app’s existing device-local-date behavior; an exact emulator run is still needed to confirm that this explains the prior timeout.

## Verification on the successor source

On `b553d68325b7c18dfbecaea2a9e92ebd15adad27` / tree `c494e20d34a961d16d67c4c882bc2eeb7b3b70f0`:

- `git diff --check d4a36fb5d3613bca2d60151de0991a1baac72fc9..HEAD`: PASS.
- `bash tools/assert_exact_source_checkout.sh b553d68325b7c18dfbecaea2a9e92ebd15adad27`: PASS.
- `bash tools/test_android_update_compatibility_guard.sh`: PASS, including all fake-ADB rejection cases and the isolated mocked API 35 update case.
- Android instrumentation compile: PASS, `BUILD SUCCESSFUL` (26 tasks), using Temurin 17.0.21 and the managed proxy/truststore:

```text
cd /workspace/serea-gmail-fix-r2/android
JAVA_HOME=/workspace/.tools/temurin17 PATH=/workspace/.tools/temurin17/bin:$PATH ./gradlew --no-daemon --console=plain \
  -Dhttp.proxyHost=proxy -Dhttp.proxyPort=8080 \
  -Dhttps.proxyHost=proxy -Dhttps.proxyPort=8080 \
  -Djavax.net.ssl.trustStore=/etc/ssl/certs/java/cacerts \
  -Djavax.net.ssl.trustStorePassword=changeit \
  :app:compileIsolatedAndroidTestKotlin
```

- **JUnit/UI:** `NOT_RUN`; `adb devices -l` listed no devices. Compile success is not execution evidence. The API 26 failure artifact above remains the only UI run for the baseline, and its result is bound to the PR #149 source, not this successor.
- **CI/artifacts for successor:** None yet; do not reuse baseline run or artifacts as pass evidence for the successor.

## Review and next action

**REVIEW_STATUS:** ACCEPTED by Lane C for the test-only source at `b553d68325b7c18dfbecaea2a9e92ebd15adad27` / tree `c494e20d34a961d16d67c4c882bc2eeb7b3b70f0`.
**SECURITY_IMPACT:** Test-only selector/determinism changes; Fake providers and the existing explicit consent/handoff boundaries remain unchanged.
**KNOWN_ISSUES:** Date-timeout root cause remains uncertain until the new exact-head API 26 UI run reports emulator timezone/date and result. API 35 UI remains blocked by its shared IME diagnostic.
**NEXT_ACTION:** Inspect #152 exact-head API 26/35 CI evidence. Keep UI status `NOT_RUN` until a focused emulator JUnit result exists; update the date-timeout assessment only from the new artifact.
