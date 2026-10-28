# G6-A Google Trusted Handoff Checkpoint

DATE: 2026-10-28

## Latest Formal API 35 Diagnostic (same-day update)

```text
DIAGNOSTIC_CANDIDATE_SHA: 23b884cf2815a2f3287652d5ffd2c3913d09df76
DIAGNOSTIC_CANDIDATE_TREE: f77fea2f7800dbf8daa1f5f25175c52f951f31a4
FORMAL_RUN: 40710445577 — Android job 122898374388; API 26 PASS; Formal API 35 diagnostic stage FAILED before the 105-test suite; Process Death API 35 PASS; Identity skipped after Android failure
FORMAL_API35_ARTIFACTS: 12826779384 (JUnit), 12828003255 (emulator evidence), 12828590814 (reacceptance bundle)
FOCUSED_API35_RUN: 40710445643 — job 122900968742; artifact 12826649063; same candidate SHA; 40/40 tests PASS, including SereaAppShellTest#chatDraftSurvivesGmailAndCalendarDestinationChanges
FORMAL_API35_OBSERVED: UTP reported “Test run failed to complete. Expected 1 tests, received 0”; JUnit XML has an empty failure; lifecycle records START only, without FAIL, FINISH, or RUN_FINISH. UTP warned it retrieved additional outputs/logcat for 0/1 cases. The host-captured logcat reaches the IME switch boundary but records no assertion, exception, or completion.
DIAGNOSIS: Incomplete UTP/instrumentation result is supported; the evidence does not establish a test assertion failure or product regression. Same-SHA focused API 35 pass argues against changing the test or product based on this artifact alone.
SOURCE_CHANGE: None. The temporary focus-request experiment was reverted; no source change was committed.
SAME_SHA_RERUN: Parent has requested a rerun of the failed Android job at 23b884cf2815a2f3287652d5ffd2c3913d09df76; pending, no new run ID assigned yet.
```

```text
AGENT_ID: A — Google Trusted Handoff
ACTIVE_PHASE: G6-A implementation complete; exact-head Android verification and formal API 35 UTP rerun pending
GOAL_STATUS: PARTIAL — Fake provider contract, preview UI, binding-race checks, hostile tests, and one-use final Chat-send proof are implemented and independently reviewed; the same-SHA focused API 35 run passes, but Formal API 35 produced an incomplete UTP result and its same-SHA rerun is pending
BASE_SHA: b84feb337cd57918346ab31cb3ea4af4ba880ffb
CURRENT_HEAD: 6d5e4aab60eca3dc2db2367bb825016f65acbcbb (dedicated branch source/checkpoint base before this docs-only follow-up)
TREE_SHA: c752cc29a4a3ce5aa5a5249ca81afa56651b3ebd
SOURCE_FIX_HEAD: 051fb4f3e4d4cf5af4155643869759b1cad43ebe (G6-SEC-01; Agent C independently ACCEPTED)
SOURCE_FIX_TREE: 6ded62e7bdb8e2d6c4edd1205fe308ec3d4d8029
PR137_PRIOR_HEAD: 89c48fc927000339666d108a76d8e8ce8dd1e95d / tree 1169990e3e597f9899904745e8b6c03d6ca87aca (A-REPORT-03 CI snapshot)
CHECKPOINT_HEAD: this Python identity-test and checkpoint follow-up advances PR #137 after the implementation snapshot above
WORKTREE: /workspace/Serea-g6-agent-a
BRANCH: codex/serea-g6-google-trusted-handoff
DRAFT_PR: #137 — OPEN/DRAFT, base codex/serea-g5-evidence-closure-20261028
COMPLETED_WORK:
  - Added a typed, in-memory, one-use Fake Gmail/Calendar handoff preview and coordinator.
  - Bound preview/acceptance to synthetic account, provider session, generation, mock authorization, service scope, exact source revision and SHA-256 content identity.
  - Added destination conversation ID and selection generation checks, one-minute elapsedRealtime expiry, explicit local-only confirmation, cancellation, replay denial, and revalidation for the local-model adapter.
  - Captured provider/account/session/generation/scope before reads and rechecked after each asynchronous authorization/source callback; added delayed switch tests across preview, confirmation, and local-model validation.
  - Rechecked the source item identifier after the confirmation read; a provider response for a different item is rejected.
  - Added source request sequencing so an older preview cannot replace a newer selection; late previews are canceled. Provider cancellation while the UI remains active shows a fail-closed error and resets busy state; Compose-scope cancellation propagates.
  - Clamped long display titles while retaining full selected text; added oversized Gmail and Calendar title regressions.
  - Added process-restart preview replay denial and a Cloud Egress lineage denial check.
  - Added a source-lineage preserving Chat prefill text helper. Handoff confirmation does not send a model request.
  - Added Gmail and Calendar detail actions and a Japanese preview dialog naming selected content, source, purpose, local destination, cancel and local-only confirm.
  - Preview state uses Compose remember only; it is not saved across recreation or process death.
  - Addressed review issue A-INT-01: restored Formal Additional to its original two identities, kept all four handoff UI identities in the non-required identity asset, and added their exact filter/required assertions to the independent focused UI campaign (33 → 37 expected tests). Formal Required remains 105 total (103 required + 2 additional).
  - Addressed A-CI-01: the nested Source fixture now initializes its default snapshot directly, avoiding an invalid implicit reference to the outer test instance's helper.
  - Addressed A-CI-02 in A-owned instrumentation tests: removed unavailable `assertDoesNotExist`, retained explicit absence checks using empty all-node query results, and used `onAllNodesWithText` for collection polling.
  - Agent C independently ACCEPTED A-CI-02 source commit `00fedba7aa1dd31a9017e33a2624b791b9dc0f68`; only the A-owned UI test source changed in that commit.
  - Addressed G6-SEC-01 with `consumeForLocalChatSend(handoff, currentConversationId, currentConversationGeneration, message, sourceLineage)`: atomically removes the exact accepted handoff before validation/suspension and then verifies exact payload and lineage, current local destination, Fake provider/account/session/generation/scope, mock authorization, source item/revision/content digest, and monotonic expiry. Rejections and cancellation consume the proof; cancellation propagates.
  - Kept `isCurrent()` separate as the existing non-consuming validator for the LocalModel path. Agent C independently ACCEPTED source head `051fb4f3e4d4cf5af4155643869759b1cad43ebe` / tree `6ded62e7bdb8e2d6c4edd1205fe308ec3d4d8029`.
  - Corrected the Python instrumentation-identity regression expectation from 8 to 12 non-required identities for the four A focused UI tests; Formal Required remains 103 identities and Formal Additional remains 2.
  - Parent-owned composition and Chat routing are integrated in PR #138 candidate `d8ac3dbbfc40546b7fca9dd2f9a65a5567d54d24`; confirmation only prefills Chat and does not send a model request. The new G6-SEC-01 consume API is not yet called from its Chat send/Egress path.
CHANGED_FILES:
  - android/app/src/main/java/app/serea/android/handoff/GoogleTrustedHandoff.kt
  - android/app/src/main/java/app/serea/android/handoff/GoogleHandoffPreviewDialog.kt
  - android/app/src/main/java/app/serea/android/gmail/ui/GmailScreen.kt
  - android/app/src/main/java/app/serea/android/calendar/ui/CalendarAgendaScreen.kt
  - android/app/src/test/java/app/serea/android/handoff/GoogleTrustedHandoffTest.kt
  - android/app/src/androidTest/java/app/serea/android/GoogleTrustedHandoffUiTest.kt
  - android/app/src/androidTest/assets/serea-non-required-test-identities.txt
  - tests/android_instrumentation_results_tests.py
  - tools/android-instrumentation-additional-tests.txt
  - tools/run_android_functional_focused.sh
  - docs/agent-reports/g6-a-google-trusted-handoff.md
TEST_COMMANDS:
  - `cd android && JAVA_HOME=/usr/lib/jvm/java-21-openjdk-amd64 ./gradlew :app:testDebugUnitTest --tests app.serea.android.handoff.GoogleTrustedHandoffTest --no-daemon` — latest attempt on G6-SEC-01 head `051fb4f`; blocked before task execution because the Gradle 8.14.6 wrapper download from `services.gradle.org` returned `ConnectException: Connection refused`
  - `cd android && JAVA_HOME=/usr/lib/jvm/java-21-openjdk-amd64 ./gradlew :app:connectedDebugAndroidTest --no-daemon` — NOT_RUN; wrapper unavailable and no Android runtime attached
  - `bash -n tools/run_android_functional_focused.sh` — PASS
  - `JAVA_HOME=/usr/lib/jvm/java-21-openjdk-amd64 ./gradlew :app:compileIsolatedAndroidTestKotlin --no-daemon` — attempted on A-CI-02 source; Gradle wrapper distribution download failed (`services.gradle.org` connection refused)
  - A handoff UI test static scan — PASS: no `assertDoesNotExist` imports or `onNodeWithText(...).fetchSemanticsNodes()` remain; preview, cancel, selected item, replay, race, and session error test cases are retained.
  - Focused campaign static assertion — PASS: all four exact handoff UI identities are filtered and required; verifier exact/minimum count is 37.
  - Formal Required invariants — PASS: additional manifest contains exactly the original two identities; required manifest has 103 identities; `run_android_instrumentation_campaign.sh` remains pinned to 105 / 103 / 2; required-manifest and formal-runner diff is empty.
  - `git diff --check` — PASS (A-INT-01, G6-SEC-01, and current checkpoint)
  - `python3 tools/validate_docs.py docs` — PASS (192 Markdown files scanned)
  - `python3 -m unittest discover -s tests -p android_instrumentation_results_tests.py` — PASS (13 tests) after updating the non-required identity expectation to 12.
  - Android API 26 focused UI campaign — NOT_RUN: no emulator executable or connected Android device is available in this environment.
  - Android API 35 focused UI campaign — NOT_RUN: no emulator executable or connected Android device is available in this environment.
JUNIT_RESULT: Local Gradle run NOT_RUN because Gradle 8.14.6 could not be downloaded. Exact-source artifact for `051fb4f` reports `GoogleTrustedHandoffTest`: 20 tests, 0 failures, 0 errors, 0 skipped. The same-SHA focused API 35 campaign on candidate `23b884c` passed 40/40, including the destination-change shell test. Formal API 35 produced an incomplete UTP result rather than a usable assertion failure; same-SHA rerun is pending.
CI_RUN_IDS: PR #137 source head `051fb4f3e4d4cf5af4155643869759b1cad43ebe` / tree `6ded62e7bdb8e2d6c4edd1205fe308ec3d4d8029`: Android 40705914293 — Change Scope 122877856507, Documentation 122877925438, Security 122877925597, Dependency Audit 122877925615, and Integration 122877925650 PASS; Build/Lint/Instrumentation Compile 122877925701 was IN_PROGRESS at last snapshot; Android Unit 122877925795 FAILED after `:app:testDebugUnitTest` BUILD SUCCESSFUL, because `test_103_required_plus_two_additional_testcases_are_verified_by_identity` expected 8 non-required identities but found 12 (artifact 12824057707). The assertion is now 12 and the focused Python regression suite passes; corrected exact-head Android Unit rerun is pending on this docs/test follow-up commit. Fast 40705914478 PASS; RustSec 40705914481 IN_PROGRESS; Git Hooks 40705914457 PASS; Full 40705914373 and Native 40705914418 have jobs IN_PROGRESS/QUEUED; SQLite 40705914628 has both DB production jobs PASS and cross-architecture open jobs QUEUED. PR #138 candidate `23b884cf2815a2f3287652d5ffd2c3913d09df76`: Formal Android run 40710445577 / job 122898374388 — API 26 PASS; API 35 diagnostic stage failed with incomplete UTP result before the 105-test suite; Process Death API 35 PASS; Identity skipped. Same-SHA focused API 35 run 40710445643 / job 122900968742 PASS (40/40). Parent same-SHA rerun is pending; its run ID is not assigned yet. These runs are exact-head evidence only and are not substituted across SHAs.
ARTIFACT_IDS: For PR #138 candidate `23b884c` / formal run 40710445577: 12826779384 (API 35 JUnit; empty failure and incomplete UTP run), 12828003255 (emulator evidence), and 12828590814 (reacceptance bundle). For same-SHA focused run 40710445643: 12826649063 (API 35 focused JUnit/logcat, 40/40 PASS). For A source head `051fb4f` / Android run 40705914293: 12824057707 (`serea-android-unit-junit`); `GoogleTrustedHandoffTest` reports 20/20 passing, while the overall Android Unit job failed at the Python assertion before its expected-count correction. The corrected exact-head Android Unit run will produce a separate artifact after rerun. For prior PR #137 snapshot `89c48fc` / run 40692059660: 12818210581 (`serea-android-dependency-audit`) and 12817537911 (`serea-android-pixel7a-debug-apk-candidate`), providing dependency audit and APK build/provenance only. For earlier PR #138 candidate `2501d70` / run 40695851735: 12820381932 (API 26 focused JUnit/logcat; 40 tests, 5 failures, all four A-owned UI cases pass; no physical-device evidence). Older PR #138 artifacts 12816715437 (provider-unit JUnit) and 12816109577 (pre-correction API 26 Compose compile failure) remain attributed to candidate `766d966`. No evidence from different SHAs is reused.
SECURITY_IMPACT: Keeps Cloud egress disabled for Google lineage; rejection occurs before any Chat/Usage dispatch; actual session reads use the Fake-only provider session API. No credential, real Google account, network, cloud, or model request is used.
DEPENDENCIES: The parent candidate `23b884cf2815a2f3287652d5ffd2c3913d09df76` wires `consumeForLocalChatSend` after routing and before Usage, passing live conversation ID/generation, message, and source lineage. The exact-head Formal API 35 rerun remains pending. Agent B's LocalModel proof validation remains a separate non-consuming `isCurrent()` contract.
KNOWN_ISSUES:
  - Source review is complete: Agent C ACCEPTED A-CI-02 source commit 00fedba. Parent composition and live Chat routing are integrated in the current PR #138 candidate.
  - A branch exact-head Android Build/Lint/Instrumentation Compile passed in run 40692059660 job 122829210280. This compile result does not substitute for the focused UI runtime campaign.
  - Agent C ACCEPTED G6-SEC-01 source at `051fb4f`; app-level use at the Chat send/Egress boundary is present in candidate `23b884c`.
  - Exact-head Android Unit job's Kotlin JUnit task passed; its enclosing job failed the Python non-required identity count before the fix. Expected count is now 12 and the focused Python suite passes; corrected exact-head Android Unit result is pending.
  - Android Build/Lint/Instrumentation Compile was still in progress for `051fb4f` at last snapshot.
  - On candidate `23b884c`, Formal API 35's filtered diagnostic stage has no completed instrumentation result, while same-SHA focused API 35 passed 40/40. No source change is supported by the current evidence; the parent same-SHA rerun is pending.
CURRENT_BLOCKERS: The parent same-SHA rerun for Formal API 35 is pending. Corrected exact-head Android Unit/compile results and integrated API 26/35 verification remain pending; local Gradle download is unavailable.
INTEGRATION_READY: NO — G6-SEC-01 coordinator source is independently ACCEPTED and 20/20 focused JVM tests pass in CI; Formal API 35 has an incomplete UTP result requiring same-SHA rerun, and integrated verification remains pending.
REVIEW_REQUEST: G6-SEC-01 source fix ACCEPTED by Agent C; parent integration review and exact-head CI remain pending.
NEXT_ACTION: Parent reruns the failed Formal API 35 job at exact candidate SHA `23b884cf2815a2f3287652d5ffd2c3913d09df76`; retain source unchanged unless the rerun provides a reproducible test or product failure.
OWNER_APPROVAL_REQUIRED: None for Fake/Local development. Real OAuth/accounts/data, Cloud AI, production signing/distribution, Pixel 7a, main merge, and Personal Beta GO remain outside this lane.
```

## Stable integration API

The parent root supplies these screen callbacks:

```kotlin
GmailScreen(
    conversationId = currentConversationId,
    conversationGeneration = currentConversationSelectionGeneration,
    onCreateHandoffPreview = coordinator::preparePreview,
    onHandoffConfirmed = { preview -> /* coordinator.confirmLocalOnly + prefill */ },
    onCancelHandoffPreview = { preview -> coordinator.cancelPreview(preview) },
)
```

`CalendarAgendaScreen` exposes the same callback contract. `confirmLocalOnly` accepts a preview plus the live current conversation ID/generation and returns `HandoffResult<TrustedGoogleHandoff>`. `TrustedGoogleHandoff` exposes `id`, `origin`, `service`, `syntheticAccountIdentifier`, `sessionIdentifier`, `sessionGeneration`, `scope`, `sourceItemIdentifier`, `sourceRevision`, `contentIdentity`, `expiresAtMillis`, `authorizationState`, `destination`, `conversationId`, `conversationGeneration`, `selectionState`, `consentState`, `selectedContent`, `purpose`, and `displayTitle`. `expiresAtMillis` uses Android `SystemClock.elapsedRealtime()` monotonic milliseconds, not a wall-clock epoch; the proof is process-local and must not be serialized. `isCurrent()` is a non-consuming validator for LocalModel. At the final local Chat send boundary call `consumeForLocalChatSend(handoff, currentConversationId, currentConversationGeneration, message, sourceLineage)`; it consumes the accepted proof even on rejection, returns `true` only for the exact expected local Chat body and sole Google origin after fresh session/account/generation/scope/auth/source/digest/expiry checks, and propagates coroutine cancellation.

The screen callback must not automatically call the model. It should prefill the Chat editor with `handoff.localChatText()` and call `prefillEditorFromProvider(..., handoff.origin)` only after live destination checks pass. Cloud lineage remains denied by the existing egress gate; local model routing additionally needs Agent B's explicit proof validator.
