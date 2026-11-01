# G5 Agent B — Cloud Egress lineage checkpoint

## Status

**PARTIAL — regression reproduced; no product patch applied.** This report is for the independently owned branch based on `fc1c0527eea8ea92509935dc8721d24b27fd7f9d`. The production source remained unchanged during both focused runs.

| Field | Value |
|---|---|
| Agent | G5 Agent B — Cloud Egress Security |
| Base / tested product HEAD | `fc1c0527eea8ea92509935dc8721d24b27fd7f9d` |
| Base tree | `756b6ee4112894f0317ee89bf0a5989bcc9d73c3` |
| Worktree | `/workspace/Serea-g5-cloud-egress` |
| Branch | `codex/serea-g5-cloud-egress-20261028` |
| Draft PR | None; requested independent review before integration |
| Product source changes | None |
| Real credentials / Google accounts / Cloud service | Not used |
| CI run / Artifact IDs | None; evidence is local JUnit XML and Gradle logs below |

The remote source branch `codex/serea-g4-egress-body-lineage` resolved to the stated base SHA at audit time. This branch is an independent regression-test patch; it must not be treated as a green product candidate while its intended regression assertions fail.

After review clarification, the exact test-source HEAD is `928066233ce9cbc993e7f2a38f2fddfe78c283b4` (tree `42367238f39a7a5c061fac53317cb9877ece1ec4`). Product source remains unchanged at `fc1c0527eea8ea92509935dc8721d24b27fd7f9d`.

## Findings

### Confirmed test-harness observation on the unchanged `fc1c` product source

For first-turn Gmail and Calendar prefills, `ChatViewModel.sendMessage()` constructs the assistant `sourceLineage` from `MODEL_OUTPUT`, `USER_AUTHORED`, and prior `context`. The current turn's `userMessage` is not in prior context. Thus the assistant response drops the current Google-origin source:

```kotlin
sourceLineage = buildSet {
    add(DataOrigin.MODEL_OUTPUT)
    add(DataOrigin.USER_AUTHORED)
    context.forEach { addAll(it.sourceLineage) }
}
```

The test `modelOutputAndFutureContextRetainGoogleSourceLineage` covers a synthetic assistant message that already has Google lineage. It does not exercise lineage transfer from a newly sent, provider-prefilled current user message.

**Root cause confidence: confirmed for the lineage propagation omission.** Suggested minimal fix for the root-owned product branch: include `userMessage.sourceLineage` in this assistant lineage union, alongside prior context. No production patch was made in this lane.

### Current Cloud denial and latent laundering distinction

The generated assistant message currently defaults to `DataClassification.UNKNOWN`. A real synthetic follow-up Cloud request whose context includes that assistant is rejected by `ChatEgressGate` as `CLASSIFICATION_DENIED`, before usage reservation, credential read, or HTTP dispatch. The focused test verified all three side effects were zero.

A separate bounded-context regression constructs the actual unlineaged summary, changes only a test copy's classification to `PUBLIC` as a future-path hypothesis, then builds context so the original user message falls outside the context budget. The gate returns `APPROVED`, because the summary lacks Gmail lineage. This demonstrates a **latent laundering condition under a future/alternate PUBLIC classification path**, not an observed current Cloud disclosure. Cloud activation must remain disabled until lineage is retained end-to-end and the regression passes.

## Test evidence

Environment: Gradle 8.14.6; Temurin JDK 21.0.13+9 selected with `-Dorg.gradle.java.home`; synthetic-only fixtures; loopback MockWebServer only.

### Existing lineage / egress baseline on `fc1c`

Command from `android/`:

```text
./gradlew --no-daemon -Dorg.gradle.java.home=/tmp/serea-g4-jdk/jdk-21.0.13+9 :app:testDebugUnitTest --rerun-tasks --tests 'app.serea.android.SecurityPortsTest' --tests 'app.serea.android.LocalFakeRoutingAdversarialTest' --tests 'app.serea.android.GoogleOriginEgressAdversarialTest' --tests 'app.serea.android.ChatViewModelTest' --tests 'app.serea.android.OllamaCloudAdapterTest'
```

**72 tests, 0 failures, 0 errors, 0 skips.** Class counts: SecurityPorts 10; LocalFakeRoutingAdversarial 3; GoogleOriginEgressAdversarial 1; ChatViewModel 36; OllamaCloudAdapter 22. The JUnit XML files and complete Gradle output are in `evidence/g5-cloud-egress-20261028/existing-egress-suite-junit/` and `existing-egress-suite.log`.

### New attack matrix, run before any product patch

Command from `android/`:

```text
./gradlew --no-daemon -Dorg.gradle.java.home=/tmp/serea-g4-jdk/jdk-21.0.13+9 :app:testDebugUnitTest --rerun-tasks --tests 'app.serea.android.CloudEgressBodyLineageAttackMatrixTest'
```

**9 tests, 4 failures, 0 errors, 0 skips.** Kotlin compilation succeeded; Gradle failed only on the recorded assertions. Exact JUnit XML: `evidence/g5-cloud-egress-20261028/attack-matrix-before-product-patch.junit.xml`.

Following the requested naming clarification, the same 9-test matrix was rerun at exact test-source HEAD `928066233ce9cbc993e7f2a38f2fddfe78c283b4`: **9 tests, 4 failures, 0 errors, 0 skips**. The failures remain the Gmail, Calendar, conversation-switch, and bounded-context regression assertions; the two renamed coverage-only tests pass. Exact-head JUnit XML: `evidence/g5-cloud-egress-20261028/attack-matrix-renamed-exact-head.junit.xml`; command output: `attack-matrix-renamed-exact-head.log`.

| JUnit test | Result | Evidence / interpretation |
|---|---|---|
| `gmailFirstTurnModelOutputInheritsCurrentBodyLineage` | FAIL | Actual assistant lineage `{MODEL_OUTPUT, USER_AUTHORED}`; expected additionally `GMAIL`. |
| `calendarFirstTurnModelOutputInheritsCurrentBodyLineage` | FAIL | Actual assistant lineage `{MODEL_OUTPUT, USER_AUTHORED}`; expected additionally `CALENDAR`. |
| `conversationSwitchDoesNotClearCurrentProviderBodyTaint` | FAIL | Provider taint survives editor and sent user body; assistant drops it after the saved-conversation switch. Same omission, separate lifecycle path. |
| `boundedContextCannotLaunderGoogleOriginThroughUnlineagedModelSummary` | FAIL | Under the explicit hypothetical PUBLIC classification and bounded context, gate returns `APPROVED` instead of `SOURCE_LINEAGE_DENIED`. Latent only; current UNKNOWN case separately denies. |
| `currentUnknownAssistantClassificationStillStopsCloudBeforeCredentialAndHttp` | PASS | `CLASSIFICATION_DENIED`; 0 usage reservations, 0 credential reads, 0 loopback HTTP dispatches. |
| `modifiedExpiredOrGoogleTaintedCloudRequestsStopBeforeUsageCredentialAndHttp` | PASS | Changed body, wrong destination, expired intent, Gmail-origin body, UNKNOWN lineage, model-origin claim, and model text claiming consent with no explicit intent are rejected; 0 reservation/read/dispatch across attempts. |
| `providerBodyTaintSurvivesWholeTextFieldValueReplacementWhitespaceClearAndReentry` | PASS | Gmail and Calendar taint remain on replacement text, whitespace, clear, and re-entry; sent body carries lineage. It replaces the entire `TextFieldValue`; this does not test character-level edits or IME input. |
| `sameApprovedSendIdCannotDispatchTwice` | PASS | First synthetic request dispatches once; duplicate send ID is rejected before a second credential read or HTTP dispatch. |
| `viewModelStoreReuseRetainsProviderBodyTaint` | PASS | A second provider lookup on the same `ViewModelStore` returns the same VM instance. This is **not** ActivityScenario recreation and does not prove Activity lifecycle persistence. |

## Attack-case coverage and limits

| Scenario | Status | Existing/new test |
|---|---|---|
| Google-source taint after whole-`TextFieldValue` replacement, blank/whitespace, clear, re-entry | Covered at ViewModel level | New `providerBodyTaintSurvivesWholeTextFieldValueReplacementWhitespaceClearAndReentry`; this is not character-level editing. Existing `ChatViewModelTest.providerPrefillLineageSurvivesEditorChangesAndIsBoundToTheSentBody` is also ViewModel-level. |
| First-turn Gmail/Calendar source to generated assistant | Covered; fails | New Gmail and Calendar first-turn tests |
| Conversation switching with provider-prefilled text | Covered; assistant lineage fails | New `conversationSwitchDoesNotClearCurrentProviderBodyTaint` |
| Bounded-context laundering when an unlineaged summary is treated as PUBLIC | Covered as a hypothetical; gate approves | New `boundedContextCannotLaunderGoogleOriginThroughUnlineagedModelSummary` |
| Current UNKNOWN assistant classification | Covered; fail-closed | New `currentUnknownAssistantClassificationStillStopsCloudBeforeCredentialAndHttp` |
| Stale editor callback | Covered separately at ViewModel level | `ChatViewModelTest.staleEditorCallbackFromSentComposerCannotRestoreDraft`; not Google session/account callback binding |
| Stale conversation callback / request ownership | Partially covered separately | `ChatViewModelTest.newConversationCannotReceiveLateReply`; no source-session/account generation is part of this contract |
| Duplicate dispatch | Covered for same usage `sendId` | New `sameApprovedSendIdCannotDispatchTwice` |
| Approval/body/destination/expiry/source lineage binding | Covered by existing baseline and new matrix | New `modifiedExpiredOrGoogleTaintedCloudRequestsStopBeforeUsageCredentialAndHttp`; baseline `SecurityPortsTest.typedEgressContractRequiresPublicClassificationOriginIntentAndFreshness` |
| Model output text pretending to grant consent | Covered; no explicit send intent is still required | New `modifiedExpiredOrGoogleTaintedCloudRequestsStopBeforeUsageCredentialAndHttp` |
| Session revocation / account switch during provider token acquisition | Covered in Google read adapters, separate from Chat source lineage | `GmailProviderSessionsTest.accountSwitchDuringTokenLeaseAcquisitionCannotDispatchAGmailRequest`; `CalendarProviderSessionsTest.accountSwitchDuringTokenLeaseAcquisitionCannotDispatchACalendarRequest`; HTTP adapter account-switch tests |
| Session/account generation bound from Google prefill into Chat and checked at send | Not covered | No UI/Composition Root integration found; provider session tests are separate |
| Actual Activity recreation for lineage | Not covered | New `viewModelStoreReuseRetainsProviderBodyTaint` test is only same-`ViewModelStore` reuse, not Activity recreation |
| True Compose/IME character-by-character partial edits | Not covered by this new suite | New ViewModel test changes a complete `TextFieldValue`; Real IME diagnostic is a separate lane |
| Real Google account/provider data or actual Cloud request | Not run / prohibited | All fixtures synthetic; no credentials or Google account used |

## Integration assessment / next action

`fc1c` retains meaningful defense in depth: Google-origin current prompts are refused at the Cloud gate; source mismatch, UNKNOWN lineage, expired approval, wrong destination, and content mismatch are blocked before usage/credential/HTTP; LOCAL_DEMO remains a separate local route (covered by existing `LocalFakeRoutingAdversarialTest.routedFakePassesEgressAndUsageAndDeniedRequestsStopBeforeReservation`).

However, **do not accept `fc1c` as complete Cloud lineage closure or enable Cloud**: the new first-turn tests prove a missing lineage edge, and a bounded context can make the unlineaged summary appear egress-eligible under the hypothetical PUBLIC path. This lane's branch is a red regression-test patch awaiting the root-owned minimal product fix and independent review. After the fix, rerun the 4 new failing tests plus the 72-test baseline and then the broader integration/security suite on the same HEAD.

No Activity recreation, delayed Google-session callback, account-generation-to-Chat binding, real service dispatch, or UI integration claim is made here.

## Evidence files

- `evidence/g5-cloud-egress-20261028/attack-matrix-before-product-patch.junit.xml`
- `evidence/g5-cloud-egress-20261028/attack-matrix-renamed-exact-head.junit.xml`
- `evidence/g5-cloud-egress-20261028/attack-matrix-renamed-exact-head.log`
- `evidence/g5-cloud-egress-20261028/failure-run.txt`
- `evidence/g5-cloud-egress-20261028/existing-egress-suite.log`
- `evidence/g5-cloud-egress-20261028/existing-egress-suite-junit/`
- Test-only source: `android/app/src/test/java/app/serea/android/CloudEgressBodyLineageAttackMatrixTest.kt`

No GitHub Actions run was started in this lane; CI run and artifact IDs are therefore **none**.

Review clarification: the original pre-fix JUnit artifact is preserved unchanged as `attack-matrix-before-product-patch.junit.xml` (it contains the prior method labels). An intermediate working-tree run is preserved separately as `attack-matrix-renamed-pre-fix.junit.xml`; the report's authoritative renamed-method result is `attack-matrix-renamed-exact-head.junit.xml` at the exact test-source HEAD above.
