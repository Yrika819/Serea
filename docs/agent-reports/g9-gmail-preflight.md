# G9 Gmail Mock HTML Rendering Preflight

**AGENT_ID:** C — G9 Fake Gmail UI security preflight
**ACTIVE_PHASE:** G9 preflight
**GOAL_STATUS:** PARTIAL — the focused instrumentation test is authored, but this environment cannot compile Android tests because its Java 21 install lacks a Java compiler.
**BASE_SHA:** `3b20faac13907146858f7aa8ce5c2832f47175c4`
**BASE_TREE_SHA:** `8398b17aeaf4c4e3dd699b75ca9d26394de9808b`
**WORKTREE:** `/workspace/Serea-g9-gmail-preflight`
**BRANCH:** `codex/serea-g9-gmail-preflight`
**DRAFT_PR:** [#143](https://github.com/Yrika819/Serea/pull/143), OPEN / DRAFT, based on `codex/serea-g8-integration-candidate-20261029`.
**JUNIT_RESULT:** NOT_RUN locally; Gradle stopped before Java test compilation.
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

### Exact-SHA CI

At initial PR head `b07f91612b28a1f9338f04dd081ef3a94180c84f`, GitHub queued Android `40790767803`, SQLite `40790767806`, RustSec `40790767711`, Full `40790767733`, Fast `40790767660`, Native `40790767676`, and Git Hooks `40790767715`. All were still QUEUED with no jobs started at the last check; there is no JUnit or artifact result yet. A report-metadata commit will produce a new HEAD, so these initial-head runs must not be carried forward as evidence for it. The formal Android Required workflow filters this test through the explicit non-required identity file, so a Required PASS would validate source compilation and manifest accounting but would not prove the focused UI test ran. Do not report the test as PASS without a focused JUnit result on the current HEAD.

## Checkpoint contract

**CURRENT_HEAD:** `3b20faac13907146858f7aa8ce5c2832f47175c4` before G9 test/report changes
**TREE_SHA:** `8398b17aeaf4c4e3dd699b75ca9d26394de9808b` before G9 test/report changes
**CHANGED_FILES:** `android/app/src/androidTest/java/app/serea/android/GmailHtmlInertUiSecurityTest.kt`; `android/app/src/androidTest/assets/serea-non-required-test-identities.txt`; `docs/agent-reports/g9-gmail-preflight.md`
**TEST_COMMANDS:** Focused command above — blocked before compilation by missing Java compiler; `git diff --check` PASS; source/manifest identity comparison PASS.
**JUNIT_RESULT:** NOT_RUN, 0 tests executed.
**CI_RUN_IDS:** Initial PR head `b07f91612b28a1f9338f04dd081ef3a94180c84f`: Android `40790767803`, SQLite `40790767806`, RustSec `40790767711`, Full `40790767733`, Fast `40790767660`, Native `40790767676`, Git Hooks `40790767715` — all QUEUED with no job started at last check. These runs are not evidence for any later report-metadata commit.
**ARTIFACT_IDS:** None.
**SECURITY_IMPACT:** Test and classification only; no production code, OAuth, HTTP provider or Cloud route changed. Mock HTTP is loopback-only.
**KNOWN_ISSUES:** Focused UI assertion has no execution evidence until a JDK-backed emulator CI run or equivalent test environment executes the exact class on the resulting HEAD.
**CURRENT_BLOCKERS:** No `javac` and no local Android emulator; initial PR-head GitHub workflows were queued and had not started at last check.
**DEPENDENCIES:** Parent G8 integration candidate `3b20faac13907146858f7aa8ce5c2832f47175c4`; exact-SHA GitHub CI.
**INTEGRATION_READY:** PARTIAL pending independent source review and a focused JUnit result.
**REVIEW_REQUEST:** Inspect the loopback-only adversarial HTML, Fake provider path, parser/text boundary, and Required/non-required manifest accounting.
**NEXT_ACTION:** Parent review; inspect newly triggered exact-SHA CI after the report update. The focused UI JUnit remains NOT_RUN unless that exact class has a focused JUnit result.
**OWNER_APPROVAL_REQUIRED:** None for mock work; real service/device/release decisions remain pending separately.
