# G6-A Google Trusted Handoff Checkpoint

DATE: 2026-10-28

```text
AGENT_ID: A — Google Trusted Handoff
ACTIVE_PHASE: G6-A implementation, focused verification, and G6-SEC-01 final-send validation
GOAL_STATUS: PARTIAL — Fake provider contract, preview UI, binding-race checks, hostile tests, and one-use final Chat-send proof are implemented; G6-SEC-01 source is independently ACCEPTED, while exact-head JUnit/Android verification and parent send-boundary wiring remain pending
BASE_SHA: b84feb337cd57918346ab31cb3ea4af4ba880ffb
CURRENT_HEAD: 051fb4f3e4d4cf5af4155643869759b1cad43ebe (A implementation head before this checkpoint-only update)
TREE_SHA: 6ded62e7bdb8e2d6c4edd1205fe308ec3d4d8029
SOURCE_FIX_HEAD: 051fb4f3e4d4cf5af4155643869759b1cad43ebe (G6-SEC-01; Agent C independently ACCEPTED)
SOURCE_FIX_TREE: 6ded62e7bdb8e2d6c4edd1205fe308ec3d4d8029
PR137_PRIOR_HEAD: 89c48fc927000339666d108a76d8e8ce8dd1e95d / tree 1169990e3e597f9899904745e8b6c03d6ca87aca (A-REPORT-03 CI snapshot)
CHECKPOINT_HEAD: this docs-only commit advances PR #137 after the implementation snapshot above
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
  - Parent-owned composition and Chat routing are integrated in PR #138 candidate `d8ac3dbbfc40546b7fca9dd2f9a65a5567d54d24`; confirmation only prefills Chat and does not send a model request. The new G6-SEC-01 consume API is not yet called from its Chat send/Egress path.
CHANGED_FILES:
  - android/app/src/main/java/app/serea/android/handoff/GoogleTrustedHandoff.kt
  - android/app/src/main/java/app/serea/android/handoff/GoogleHandoffPreviewDialog.kt
  - android/app/src/main/java/app/serea/android/gmail/ui/GmailScreen.kt
  - android/app/src/main/java/app/serea/android/calendar/ui/CalendarAgendaScreen.kt
  - android/app/src/test/java/app/serea/android/handoff/GoogleTrustedHandoffTest.kt
  - android/app/src/androidTest/java/app/serea/android/GoogleTrustedHandoffUiTest.kt
  - android/app/src/androidTest/assets/serea-non-required-test-identities.txt
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
  - Android API 26 focused UI campaign — NOT_RUN: no emulator executable or connected Android device is available in this environment.
  - Android API 35 focused UI campaign — NOT_RUN: no emulator executable or connected Android device is available in this environment.
JUNIT_RESULT: NOT_RUN locally — Gradle 8.14.6 could not be downloaded. Added four coordinator tests; this test class now contains 20 focused JVM cases. Exact-head Android Unit job 122877925795 is in progress; no JUnit PASS is claimed yet.
CI_RUN_IDS: PR #137 exact source head `051fb4f3e4d4cf5af4155643869759b1cad43ebe` / tree `6ded62e7bdb8e2d6c4edd1205fe308ec3d4d8029`: Android 40705914293 — Change Scope 122877856507, Documentation 122877925438, Security 122877925597, Dependency Audit 122877925615, and Integration 122877925650 PASS; Build/Lint/Instrumentation Compile 122877925701 and Android Unit 122877925795 IN_PROGRESS. Fast 40705914478 PASS; RustSec 40705914481 IN_PROGRESS; Git Hooks 40705914457 PASS; Full 40705914373 and Native 40705914418 have jobs IN_PROGRESS/QUEUED; SQLite 40705914628 has both DB production jobs PASS and cross-architecture open jobs QUEUED. Current PR #138 head is `d8ac3dbbfc40546b7fca9dd2f9a65a5567d54d24`: Fast 40703690566, RustSec 40703690230, Git Hooks 40703690272, and Android Personal Beta Contract 40703690261 PASS; Android 40703690336, focused UI 40703690202, Full 40703690302, Native 40703690775, and SQLite 40703690245 CANCELLED. G6-SEC-01 API is not yet wired into that candidate's Chat send/Egress path. Earlier exact API 26 focused run 40695851735 on candidate `2501d70ace3a253baca6a53eb734ee786bfed678` failed 5/40; all four A-owned `GoogleTrustedHandoffUiTest` cases passed, while two ChatScreen tests and three parent-owned shell tests timed out waiting for restore-ready Chat state. Its artifact 12820381932 contains the JUnit results. Agent A's diagnosis identified the then-current `restoreLatest` generation increment/finally mismatch; root has since advanced PR #138 to `d8ac3db`. These runs are exact-head evidence only and are not substituted across SHAs.
ARTIFACT_IDS: For prior PR #137 snapshot `89c48fc` / run 40692059660: 12818210581 (`serea-android-dependency-audit`) and 12817537911 (`serea-android-pixel7a-debug-apk-candidate`), providing dependency audit and APK build/provenance only. For earlier PR #138 candidate `2501d70` / run 40695851735: 12820381932 (API 26 focused JUnit/logcat; 40 tests, 5 failures, all four A-owned UI cases pass; no physical-device evidence). Older PR #138 artifacts 12816715437 (provider-unit JUnit) and 12816109577 (pre-correction API 26 Compose compile failure) remain attributed to candidate `766d966` / run 40685979096. No JUnit artifact exists yet for A source head `051fb4f` or current PR #138 head `d8ac3db`.
SECURITY_IMPACT: Keeps Cloud egress disabled for Google lineage; rejection occurs before any Chat/Usage dispatch; actual session reads use the Fake-only provider session API. No credential, real Google account, network, cloud, or model request is used.
DEPENDENCIES: Parent must retain the exact accepted proof with the provider-prefilled Chat draft and call `consumeForLocalChatSend` after routing, immediately before the Usage delegate, passing the live conversation ID/generation, message, and source lineage. The API is not yet wired into current PR #138 `d8ac3db`; no shared shell/Egress file was changed in A's fix. Agent B's LocalModel proof validation remains a separate non-consuming `isCurrent()` contract.
KNOWN_ISSUES:
  - Source review is complete: Agent C ACCEPTED A-CI-02 source commit 00fedba. Parent composition and live Chat routing are integrated in the current PR #138 candidate.
  - A branch exact-head Android Build/Lint/Instrumentation Compile passed in run 40692059660 job 122829210280. This compile result does not substitute for the focused UI runtime campaign.
  - Agent C ACCEPTED G6-SEC-01 source at `051fb4f`; app-level use at the Chat send/Egress boundary remains pending in the parent candidate.
  - PR #137 Android Unit and Build/Lint/Instrumentation Compile jobs are in progress for `051fb4f`; no current exact-head JUnit artifact is available.
CURRENT_BLOCKERS: Parent integration must wire the accepted proof into the final Chat send boundary; exact-head coordinator JVM results and integrated API 26/35 verification remain pending. Local Gradle download is unavailable.
INTEGRATION_READY: NO — G6-SEC-01 coordinator API is independently ACCEPTED, but not wired into the parent send path; current exact-head JVM/Android verification is incomplete.
REVIEW_REQUEST: G6-SEC-01 source fix ACCEPTED by Agent C; parent integration review and exact-head CI remain pending.
NEXT_ACTION: Parent wires `consumeForLocalChatSend` before Usage dispatch; monitor exact-head Android Unit/compile results and rerun focused integration campaign on a fixed integration candidate.
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
