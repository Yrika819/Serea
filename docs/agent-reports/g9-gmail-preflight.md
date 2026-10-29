# G9 Gmail Mock HTML Rendering Preflight

**AGENT_ID:** C — G9 Fake Gmail UI security preflight
**ACTIVE_PHASE:** G9 preflight
**GOAL_STATUS:** PARTIAL — the focused instrumentation source compiles on the exact G9 source head, but JUnit/UI execution remains NOT_RUN.
**BASE_SHA:** `3b20faac13907146858f7aa8ce5c2832f47175c4`
**BASE_TREE_SHA:** `8398b17aeaf4c4e3dd699b75ca9d26394de9808b`
**WORKTREE:** `/workspace/Serea-g9-gmail-preflight`
**BRANCH:** `codex/serea-g9-gmail-preflight`
**DRAFT_PR:** [#143](https://github.com/Yrika819/Serea/pull/143), OPEN / DRAFT, based on `codex/serea-g8-integration-candidate-20261029`.
**VERIFIED_COMPILE_HEAD:** `6afd718e8e2e3882f7f07ebc3e8d3b97ed67032a`
**VERIFIED_COMPILE_TREE_SHA:** `923aeaf2868ce886b433fcf8e1094722f746f398`
**COMPILE_RESULT:** Parent-run `:app:compileIsolatedAndroidTestKotlin` PASS, BUILD SUCCESSFUL, 26 tasks, on the exact source HEAD/tree above.
**JUNIT_RESULT:** NOT_RUN — compile success is not test execution; zero JUnit records were produced by this compile.
**ANDROID_UI_RESULT:** NOT_RUN — no emulator test was executed.
**OWNER_APPROVAL_REQUIRED:** None for this synthetic loopback test. Real Gmail, OAuth, Google data, Cloud AI, production signing, Pixel 7a and main merge remain Owner-controlled and were not used.

## Scope and design

Added one Android UI security test: `GmailHtmlInertUiSecurityTest#fakeGmailRendersAdversarialHtmlAsInertTextWithoutRemoteRequests`.

The test creates an adversarial synthetic `text/html` MIME body containing a loopback `<img src>`, an `onerror` handler, an external `<script src>`, and inline script with a fetch attempt. It passes the MIME JSON through the existing `GmailMimeParser`, supplies the parsed message via `GmailProviderSessions.openFake()`, opens the Fake Inbox in `GmailScreen`, and navigates to the synthetic message.

Assertions require the visible message body to contain only the safe text extracted by the parser. The body must not expose the tracking URL, HTML tags, script code, handler code, or injected cookie marker. A loopback `MockWebServer` waits for any accidental resource request; the test requires no request and `requestCount == 0`. The screen uses Compose `Text`; no WebView, renderer, provider or runtime network path was added or enabled.

The test identity was added to `android/app/src/androidTest/assets/serea-non-required-test-identities.txt`. The original Required manifest remains 103 tests and the Additional manifest remains 2 tests. This test is explicitly excluded from the Formal Required 105-test set and can run only through a focused class filter. The manifest/source identity check passes with 16 non-required tests and 121 total source test identities.

No parser code or parser tests were changed. Existing `GmailDomainParserTest` covers plain/alternative/mixed MIME, unsupported/incomplete parts, HTML stripping, attachment exclusion, declared-size mismatch, empty/missing bodies, JSON/body size limits, malformed Base64/UTF-8, nesting and MIME part-depth limits. The targeted uncovered evidence was UI presentation plus zero remote requests.

## Verification

### Local source/manifest checks

- `git diff --check`: PASS.
- Python source/manifest comparison using `tools.verify_android_instrumentation.collect_source_test_identities` and `read_test_manifest`: PASS — Required 103, Additional 2, non-required 16, total 121; all source identities exactly match the three manifests.
- The Required manifest files and thresholds were not modified.

### Focused Gradle attempt

The focused instrumentation command was attempted:

```text
./gradlew --no-daemon :app:connectedIsolatedAndroidTest \
  -Pandroid.testInstrumentationRunnerArguments.class=app.serea.android.GmailHtmlInertUiSecurityTest
```

The inherited `JAVA_HOME=/usr/lib/jvm/java-17-openjdk-amd64` pointed to a missing directory, so the command was retried with the installed Java 21 runtime:

```text
JAVA_HOME=/usr/lib/jvm/java-21-openjdk-amd64 ./gradlew --no-daemon \
  :app:connectedIsolatedAndroidTest \
  -Pandroid.testInstrumentationRunnerArguments.class=app.serea.android.GmailHtmlInertUiSecurityTest
```

The retry reached Gradle, then failed before compiling the test:

```text
Could not create task ':app:compileIsolatedAndroidTestJavaWithJavac'.
Toolchain installation '/usr/lib/jvm/java-21-openjdk-amd64' does not provide the required capabilities: [JAVA_COMPILER]
BUILD FAILED
```

`javac` is absent from the Java 21 installation and `adb devices -l` lists no emulator. The test was therefore **NOT_RUN**, with zero JUnit records. This is an environment limitation, not a test failure or pass. No system packages were installed and no emulator was launched.

### Parent-run instrumentation source compilation

The parent later compiled the instrumentation source on the exact G9 branch head `6afd718e8e2e3882f7f07ebc3e8d3b97ed67032a`, tree `923aeaf2868ce886b433fcf8e1094722f746f398`, using checksum-verified Temurin 17 with the managed proxy and truststore:

```text
cd /workspace/Serea-g9-gmail-preflight/android
JAVA_HOME=/workspace/.tools/temurin17 PATH=/workspace/.tools/temurin17/bin:$PATH ./gradlew --no-daemon --console=plain \
  -Dhttp.proxyHost=proxy -Dhttp.proxyPort=8080 \
  -Dhttps.proxyHost=proxy -Dhttps.proxyPort=8080 \
  -Djavax.net.ssl.trustStore=/workspace/.tools/temurin17/codex-cacerts \
  -Djavax.net.ssl.trustStorePassword=changeit \
  :app:compileIsolatedAndroidTestKotlin
```

Result: **PASS**, BUILD SUCCESSFUL, 26 tasks. This verifies Android instrumentation source compilation only. It did not run `connectedIsolatedAndroidTest`, launch an emulator, execute `GmailHtmlInertUiSecurityTest`, or produce JUnit results. JUnit and UI execution therefore remain **NOT_RUN**, with zero tests executed.

### Exact-SHA CI

At the PR head before this report update (`6afd718e8e2e3882f7f07ebc3e8d3b97ed67032a`, tree `923aeaf2868ce886b433fcf8e1094722f746f398`), GitHub queued Android `40790972547`, Full `40790972589`, SQLite `40790972540`, Fast `40790972554`, Native `40790972615`, RustSec `40790972663`, and Git Hooks `40790972567`. There is no JUnit result or artifact for these runs. The parent-run compile PASS above is local compile evidence, not a CI result. The earlier initial-head runs on `b07f91612b28a1f9338f04dd081ef3a94180c84f` are historical only. The formal Android Required workflow filters this test through the explicit non-required identity file, so a Required PASS would not prove the focused UI test ran. Do not report this test as PASS without a focused JUnit result.

## Checkpoint contract

**VERIFIED_SOURCE_HEAD:** `6afd718e8e2e3882f7f07ebc3e8d3b97ed67032a` (exact parent-run instrumentation compile source; this checkpoint update adds documentation only)
**VERIFIED_SOURCE_TREE_SHA:** `923aeaf2868ce886b433fcf8e1094722f746f398`
**CURRENT_REPORT_BRANCH_HEAD_BEFORE_THIS_UPDATE:** `6afd718e8e2e3882f7f07ebc3e8d3b97ed67032a`
**TREE_SHA_BEFORE_THIS_UPDATE:** `923aeaf2868ce886b433fcf8e1094722f746f398`
**CHANGED_FILES:** `android/app/src/androidTest/java/app/serea/android/GmailHtmlInertUiSecurityTest.kt`; `android/app/src/androidTest/assets/serea-non-required-test-identities.txt`; `docs/agent-reports/g9-gmail-preflight.md`
**TEST_COMMANDS:** Focused connected UI command above — execution NOT_RUN because no emulator is attached; parent-run `:app:compileIsolatedAndroidTestKotlin` with Temurin 17/proxy/truststore PASS, BUILD SUCCESSFUL (26 tasks) on `6afd718e8e2e3882f7f07ebc3e8d3b97ed67032a` / tree `923aeaf2868ce886b433fcf8e1094722f746f398`; `git diff --check` PASS; source/manifest identity comparison PASS.
**JUNIT_RESULT:** NOT_RUN, 0 tests executed. Compile success is not a JUnit result.
**ANDROID_UI_RESULT:** NOT_RUN; no emulator test was run.
**CI_RUN_IDS:** Pre-report PR head `6afd718e8e2e3882f7f07ebc3e8d3b97ed67032a`: Android `40790972547`, Full `40790972589`, SQLite `40790972540`, Fast `40790972554`, Native `40790972615`, RustSec `40790972663`, Git Hooks `40790972567` — queued at last check, no JUnit/artifact evidence. These IDs do not prove focused UI execution.
**ARTIFACT_IDS:** None.
**SECURITY_IMPACT:** Test and classification only; no production code, OAuth, HTTP provider or Cloud route changed. Mock HTTP is loopback-only.
**KNOWN_ISSUES:** The focused UI assertion has no execution evidence; exact-source compilation is confirmed, but a JDK-backed emulator run must execute the exact class for JUnit/UI evidence.
**CURRENT_BLOCKERS:** No local Android emulator; exact-head focused JUnit/UI execution is still pending.
**DEPENDENCIES:** Parent G8 integration candidate `3b20faac13907146858f7aa8ce5c2832f47175c4`; exact-SHA GitHub CI.
**INTEGRATION_READY:** PARTIAL pending independent source review and a focused JUnit result.
**REVIEW_REQUEST:** Inspect the loopback-only adversarial HTML, Fake provider path, parser/text boundary, and Required/non-required manifest accounting.
**NEXT_ACTION:** Parent review; inspect exact-SHA CI after the report update. Keep focused UI JUnit as NOT_RUN unless that exact class has a focused JUnit result.
**OWNER_APPROVAL_REQUIRED:** None for mock work; real service/device/release decisions remain pending separately.
