# G6-A Google Trusted Handoff Checkpoint

DATE: 2026-10-28

```text
AGENT_ID: A — Google Trusted Handoff
ACTIVE_PHASE: G6-A implementation and focused verification
GOAL_STATUS: PARTIAL — Fake provider contract, both screen previews, binding-race checks, and hostile tests are implemented; Android compile passed on the current A branch head, while runtime/JUnit and integrated API 26/35 acceptance remain pending
BASE_SHA: b84feb337cd57918346ab31cb3ea4af4ba880ffb
CURRENT_HEAD: 89c48fc927000339666d108a76d8e8ce8dd1e95d (PR #137 head at CI snapshot, before this checkpoint-only update)
TREE_SHA: 1169990e3e597f9899904745e8b6c03d6ca87aca
SOURCE_FIX_HEAD: 00fedba7aa1dd31a9017e33a2624b791b9dc0f68 (A-CI-02; independently ACCEPTED by Agent C)
SOURCE_FIX_TREE: bf248f9682ff9be5b682d5ba15d30447c5018060
CHECKPOINT_HEAD: this docs-only commit advances PR #137 after the CI snapshot above
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
  - Parent-owned composition and Chat routing have been integrated into current PR #138 candidate `e1b9803f66bf5082e8b62b54bb989a2f94b11722` (tree `217c838b86272759d1996447ecc90c6e53b265f9`); confirmation only prefills Chat and does not send a model request.
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
  - `cd android && JAVA_HOME=/usr/lib/jvm/java-21-openjdk-amd64 ./gradlew :app:testDebugUnitTest --tests app.serea.android.handoff.GoogleTrustedHandoffTest --no-daemon` — attempted on A-CI-01 source; Gradle 8.14.6 wrapper download failed (`services.gradle.org` connection refused)
  - `cd android && JAVA_HOME=/usr/lib/jvm/java-21-openjdk-amd64 ./gradlew :app:connectedDebugAndroidTest --no-daemon` — NOT_RUN; wrapper unavailable and no Android runtime attached
  - `bash -n tools/run_android_functional_focused.sh` — PASS
  - `JAVA_HOME=/usr/lib/jvm/java-21-openjdk-amd64 ./gradlew :app:compileIsolatedAndroidTestKotlin --no-daemon` — attempted on A-CI-02 source; Gradle wrapper distribution download failed (`services.gradle.org` connection refused)
  - A handoff UI test static scan — PASS: no `assertDoesNotExist` imports or `onNodeWithText(...).fetchSemanticsNodes()` remain; preview, cancel, selected item, replay, race, and session error test cases are retained.
  - Focused campaign static assertion — PASS: all four exact handoff UI identities are filtered and required; verifier exact/minimum count is 37.
  - Formal Required invariants — PASS: additional manifest contains exactly the original two identities; required manifest has 103 identities; `run_android_instrumentation_campaign.sh` remains pinned to 105 / 103 / 2; required-manifest and formal-runner diff is empty.
  - `git diff --check` — PASS (A-INT-01 follow-up)
  - `python3 tools/validate_docs.py docs` — PASS (192 Markdown files scanned)
  - Android API 26 focused UI campaign — NOT_RUN: no emulator executable or connected Android device is available in this environment.
  - Android API 35 focused UI campaign — NOT_RUN: no emulator executable or connected Android device is available in this environment.
JUNIT_RESULT: NOT_RUN — Gradle 8.14.6 could not be downloaded (`services.gradle.org` connection refused), and no Android runtime is available. No local test PASS is claimed. Prepared 18 focused JVM tests and 4 focused Android UI tests.
CI_RUN_IDS: Current PR #137 CI snapshot is for head `89c48fc927000339666d108a76d8e8ce8dd1e95d` / tree `1169990e3e597f9899904745e8b6c03d6ca87aca` (before this checkpoint-only update): Android 40692059660 — Change Scope, Build/Lint/Instrumentation Compile, and Dependency Audit PASS; Android Unit, Integration, Security, Documentation, and Storage/Emulator QUEUED. Fast 40692059693 QUEUED; Full 40692059689 has Windows Gate C IN_PROGRESS and other listed jobs QUEUED; Native 40692059659 has several platform jobs IN_PROGRESS and others QUEUED; SQLite 40692059657 has both DB production jobs PASS and cross-architecture open jobs QUEUED; RustSec 40692059612 PASS; Git Hooks 40692059669 QUEUED. Current PR #138 exact integration candidate is `e1b9803f66bf5082e8b62b54bb989a2f94b11722` / tree `217c838b86272759d1996447ecc90c6e53b265f9`: Android 40692611836, focused provider JVM/API 26/35 40692612063, and mock release contract 40692611975 QUEUED; Full 40692611918, Fast 40692612031, Native 40692612801, SQLite 40692611938, RustSec 40692612111, Git Hooks 40692611949, and Release Security 40692699335 QUEUED. Thus integrated instrumentation compile plus API 26 and API 35 focused executions remain pending on the final #138 SHA. Earlier #138 candidate `766d966ed89a75cf51580762ad60965852e9d443` evidence: focused run 40685979096 had provider JVM job 122808317749 PASS, API 26 job 122819545014 FAILED before A-CI-02, and API 35 job 122823421854 SKIPPED; Android run 40685978759 had Integration 122819737596, Docs 122819737615, Dependency Audit 122819737663, Security 122819737728 PASS; Android Unit 122819737642 and Build/Lint/Instrumentation Compile 122819737905 FAILED on the old test API; Required Gate 122824303535 FAILED. These prior-candidate results are not evidence for current #138 SHA.
ARTIFACT_IDS: For PR #137 CI snapshot `89c48fc` / run 40692059660: 12818210581 (`serea-android-dependency-audit`) and 12817537911 (`serea-android-pixel7a-debug-apk-candidate`). These provide dependency-audit and APK-build/provenance artifacts only; they contain no JUnit result or physical-device/runtime evidence. Prior PR #138 candidate `766d966` / run 40685979096: 12816715437 (provider-unit JUnit evidence) and 12816109577 (API 26 focused log documenting the pre-correction Compose compile failure). No artifact has yet been recorded for the current #138 exact integration candidate.
SECURITY_IMPACT: Keeps Cloud egress disabled for Google lineage; rejection occurs before any Chat/Usage dispatch; actual session reads use the Fake-only provider session API. No credential, real Google account, network, cloud, or model request is used.
DEPENDENCIES: Parent composition is integrated in current PR #138 candidate `e1b9803f66bf5082e8b62b54bb989a2f94b11722`: it checks the live destination, consumes one-use local handoff proof, preserves lineage, and prefills Chat without sending. Exact-head instrumentation compile and API 26/35 functional acceptance remain pending; Agent B's local-model proof mapping is a separate integration decision.
KNOWN_ISSUES:
  - Source review is complete: Agent C ACCEPTED A-CI-02 source commit 00fedba. Parent composition and live Chat routing are integrated in the current PR #138 candidate.
  - A branch exact-head Android Build/Lint/Instrumentation Compile passed in run 40692059660 job 122829210280. This compile result does not substitute for the focused UI runtime campaign.
  - A-CI-02 corrected instrumentation source has not yet compiled/executed on the current PR #138 integration SHA; those exact-head checks are queued.
CURRENT_BLOCKERS: Corrected integration compile and focused API 26/35 UI campaign on the current PR #138 exact head are pending. Local Gradle 8.14.6 download is unavailable and no Android runtime is attached to this worktree.
INTEGRATION_READY: NO — source review is accepted and parent composition is in the candidate, but corrected Android test compilation and focused API 26/35 execution remain pending.
REVIEW_REQUEST: A-CI-02 source correction ACCEPTED by Agent C; no further review request is open on A-owned changes.
NEXT_ACTION: Monitor current PR #138 exact-head Android and focused UI workflows; report/fix any A-owned failure after evidence arrives.
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

`CalendarAgendaScreen` exposes the same callback contract. `confirmLocalOnly` accepts a preview plus the live current conversation ID/generation and returns `HandoffResult<TrustedGoogleHandoff>`. `TrustedGoogleHandoff` exposes `id`, `origin`, `service`, `syntheticAccountIdentifier`, `sessionIdentifier`, `sessionGeneration`, `scope`, `sourceItemIdentifier`, `sourceRevision`, `contentIdentity`, `expiresAtMillis`, `authorizationState`, `destination`, `conversationId`, `conversationGeneration`, `selectionState`, `consentState`, `selectedContent`, `purpose`, and `displayTitle`. `expiresAtMillis` uses Android `SystemClock.elapsedRealtime()` monotonic milliseconds, not a wall-clock epoch; the proof is process-local and must not be serialized. `isCurrent` revalidates the exact consumed proof against current source state and propagates coroutine cancellation.

The screen callback must not automatically call the model. It should prefill the Chat editor with `handoff.localChatText()` and call `prefillEditorFromProvider(..., handoff.origin)` only after live destination checks pass. Cloud lineage remains denied by the existing egress gate; local model routing additionally needs Agent B's explicit proof validator.
