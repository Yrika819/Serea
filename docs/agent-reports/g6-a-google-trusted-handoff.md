# G6-A Google Trusted Handoff Checkpoint

DATE: 2026-10-28

```text
AGENT_ID: A — Google Trusted Handoff
ACTIVE_PHASE: G6-A implementation and focused verification
GOAL_STATUS: PARTIAL — Fake provider contract and both screen previews implemented; Gradle/JUnit and Android UI verification unavailable in this environment
BASE_SHA: b84feb337cd57918346ab31cb3ea4af4ba880ffb
CURRENT_HEAD: pending local commit
TREE_SHA: pending local commit
WORKTREE: /workspace/Serea-g6-agent-a
BRANCH: codex/serea-g6-google-trusted-handoff
DRAFT_PR: none
COMPLETED_WORK:
  - Added a typed, in-memory, one-use Fake Gmail/Calendar handoff preview and coordinator.
  - Bound preview/acceptance to synthetic account, provider session, generation, mock authorization, service scope, exact source revision and SHA-256 content identity.
  - Added destination conversation ID and selection generation checks, one-minute expiry, explicit local-only confirmation, cancellation, replay denial, and revalidation for the local-model adapter.
  - Added a source-lineage preserving Chat prefill text helper. Handoff confirmation does not send a model request.
  - Added Gmail and Calendar detail actions and a Japanese preview dialog naming selected content, source, purpose, local destination, cancel and local-only confirm.
  - Preview state uses Compose remember only; it is not saved across recreation or process death.
CHANGED_FILES:
  - android/app/src/main/java/app/serea/android/handoff/GoogleTrustedHandoff.kt
  - android/app/src/main/java/app/serea/android/handoff/GoogleHandoffPreviewDialog.kt
  - android/app/src/main/java/app/serea/android/gmail/ui/GmailScreen.kt
  - android/app/src/main/java/app/serea/android/calendar/ui/CalendarAgendaScreen.kt
  - android/app/src/test/java/app/serea/android/handoff/GoogleTrustedHandoffTest.kt
  - android/app/src/androidTest/java/app/serea/android/GoogleTrustedHandoffUiTest.kt
  - android/app/src/androidTest/assets/serea-non-required-test-identities.txt
  - tools/android-instrumentation-additional-tests.txt
  - docs/agent-reports/g6-a-google-trusted-handoff.md
TEST_COMMANDS:
  - `cd android && JAVA_HOME=/usr/lib/jvm/java-21-openjdk-amd64 ./gradlew :app:testDebugUnitTest --no-daemon` — attempted, Gradle wrapper distribution download failed (`services.gradle.org` connection refused)
  - `cd android && JAVA_HOME=/usr/lib/jvm/java-21-openjdk-amd64 ./gradlew :app:connectedDebugAndroidTest --no-daemon` — NOT_RUN; wrapper unavailable and no Android runtime attached
JUNIT_RESULT: NOT_RUN — Gradle 8.14.6 could not be downloaded; no test PASS is claimed
CI_RUN_IDS: none
ARTIFACT_IDS: none
SECURITY_IMPACT: Keeps Cloud egress disabled for Google lineage; rejection occurs before any Chat/Usage dispatch; actual session reads use the Fake-only provider session API. No credential, real Google account, network, cloud, or model request is used.
DEPENDENCIES: Parent composition must create the session-bound coordinator, provide the current conversation ID and selection generation, consume accepted proof, prefill `ChatViewModel` with `localChatText()` and origin, and retain proof for `isCurrent` if a local model route is later selected.
KNOWN_ISSUES:
  - Full source compilation and tests remain unverified because the environment has no installed Gradle and the wrapper distribution host rejected the connection.
  - Parent composition/UI routing and independent source review remain pending.
CURRENT_BLOCKERS: Local Gradle distribution unavailable; no Android UI runtime in this worktree.
INTEGRATION_READY: NO — implementation awaits compilation, focused JVM PASS, Android UI PASS, root composition wiring, and independent review
REVIEW_REQUEST: Review source, session/account/generation checks, content identity, cancellation, preview lifecycle, UI consent copy, and replay handling after commit.
NEXT_ACTION: Complete static review, commit the lane, provide exact API/checkpoint to parent, then rerun focused JVM and UI checks in an environment with Gradle and Android runtime.
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

`CalendarAgendaScreen` exposes the same callback contract. `confirmLocalOnly` accepts a preview plus the live current conversation ID/generation and returns `HandoffResult<TrustedGoogleHandoff>`. `TrustedGoogleHandoff` exposes `id`, `origin`, `service`, `syntheticAccountIdentifier`, `sessionIdentifier`, `sessionGeneration`, `scope`, `sourceItemIdentifier`, `sourceRevision`, `contentIdentity`, `expiresAtMillis`, `authorizationState`, `destination`, `conversationId`, `conversationGeneration`, `selectionState`, `consentState`, `selectedContent`, `purpose`, and `displayTitle`. `isCurrent` revalidates the exact consumed proof against current source state and propagates coroutine cancellation.

The screen callback must not automatically call the model. It should prefill the Chat editor with `handoff.localChatText()` and call `prefillEditorFromProvider(..., handoff.origin)` only after live destination checks pass. Cloud lineage remains denied by the existing egress gate; local model routing additionally needs Agent B's explicit proof validator.
