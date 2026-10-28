# G6-A Google Trusted Handoff Checkpoint

DATE: 2026-10-28

```text
AGENT_ID: A — Google Trusted Handoff
ACTIVE_PHASE: G6-A implementation and focused verification
GOAL_STATUS: PARTIAL — Fake provider contract, both screen previews, binding-race checks, and hostile tests are implemented; Gradle/JUnit and Android UI verification unavailable in this environment
BASE_SHA: b84feb337cd57918346ab31cb3ea4af4ba880ffb
CURRENT_HEAD: 76cd89d112f3c4a4e017fbc33def302c2a947939 (PR #137 branch head before this checkpoint-only update)
TREE_SHA: 7575962d9b64576cd8ebef60f232ffdf673676b3
SOURCE_FIX_HEAD: 00fedba7aa1dd31a9017e33a2624b791b9dc0f68
SOURCE_FIX_TREE: bf248f9682ff9be5b682d5ba15d30447c5018060
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
CI_RUN_IDS: PR #137 head `76cd89d112f3c4a4e017fbc33def302c2a947939`: Fast CI 40691864327 queued; Android 40691864677 pending with no jobs materialized; Full CI 40691864339 pending; Native 40691864405 pending; SQLite 40691864313, RustSec 40691864478, and Git Hooks 40691864340 queued. Prior A source-head run 40691291278 Fast CI PASS; Android Change Scope 122826464719 PASS, then remaining Android jobs and other workflows were cancelled after checkpoint-head update. PR #138 head `766d966ed89a75cf51580762ad60965852e9d443`: focused run 40685979096 provider JVM job 122808317749 PASS; API 26 job 122819545014 failed before A-CI-02 correction, API 35 job 122823421854 skipped. Android run 40685978759 Integration 122819737596, Docs 122819737615, Dependency Audit 122819737663, and Security 122819737728 PASS; Android Unit 122819737642 and Build/Lint/Instrumentation Compile 122819737905 failed on the old test API; Required Gate 122824303535 failed. Corrected #138 exact-head compile and API 26/35 campaign have not run.
ARTIFACT_IDS: 12816715437 (PR #138 focused provider-unit JUnit evidence, run 40685979096); 12816109577 (PR #138 API 26 focused evidence with pre-correction Compose compile failure, run 40685979096).
SECURITY_IMPACT: Keeps Cloud egress disabled for Google lineage; rejection occurs before any Chat/Usage dispatch; actual session reads use the Fake-only provider session API. No credential, real Google account, network, cloud, or model request is used.
DEPENDENCIES: Parent composition is integrated in the PR #138 candidate: it checks the live destination, consumes one-use local handoff proof, preserves lineage, and prefills Chat without sending. Exact-head instrumentation compile and API 26/35 functional acceptance remain pending; Agent B's local-model proof mapping is a separate integration decision.
KNOWN_ISSUES:
  - Source review is complete: Agent C ACCEPTED A-CI-02 source commit 00fedba. Parent composition and live Chat routing are in the PR #138 candidate.
  - The A-CI-02 corrected instrumentation source has not yet compiled on an exact updated integration head; local Gradle 8.14.6 download is unavailable.
CURRENT_BLOCKERS: Local Gradle distribution unavailable; no Android UI runtime in this worktree; corrected exact-head compile and integrated API 26/35 campaign pending. Parent-owned `GoogleTrustedHandoffShellTest` API correction and exact-40 run remain pending.
INTEGRATION_READY: NO — source review is accepted and parent composition is in the candidate, but corrected Android test compilation and focused API 26/35 execution remain pending.
REVIEW_REQUEST: A-CI-02 source correction ACCEPTED by Agent C; no further review request is open on A-owned changes.
NEXT_ACTION: Run exact-head Android compile and focused API 26/35 CI after the parent-owned shell-test API fix and A-CI-02 source are on the integrated candidate.
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
