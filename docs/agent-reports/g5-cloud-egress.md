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

## Structured G5 final report and integration review

### Lane B provenance and status

```text
AGENT_ID: G5 Agent B — Cloud Egress Security
ACTIVE_PHASE: G5
OVERALL_STATUS: PARTIAL (pre-fix regression reproduced; root-owned fix independently accepted for integration)
BASE_SHA: fc1c0527eea8ea92509935dc8721d24b27fd7f9d
BASE_TREE: 756b6ee4112894f0317ee89bf0a5989bcc9d73c3
OWNED_BRANCH: codex/serea-g5-cloud-egress-20261028
WORKTREE: /workspace/Serea-g5-cloud-egress (clean at report)
OWNED_HEAD_BEFORE_THIS_CHECKPOINT_APPEND: e62d16e34a4227d0ff99d2a6af6da9894e960baa
OWNED_TREE_BEFORE_THIS_CHECKPOINT_APPEND: 087147e32f712e978888c936b175842b88432441
DRAFT_PR: none
PRODUCT_FILES_CHANGED_ON_OWNED_BRANCH: none
CI_RUN_IDS_FOR_OWNED_BRANCH: none (local Gradle only)
GITHUB_ARTIFACT_IDS_FOR_OWNED_BRANCH: none (JUnit/logs are committed locally in the branch)
```

Prior owned-branch milestones were `0b4db418acd5587e931903106827c7e414310b5c` (test/evidence), `928066233ce9cbc993e7f2a38f2fddfe78c283b4` (test-name clarification), and `e62d16e34a4227d0ff99d2a6af6da9894e960baa` (exact-head evidence/checkpoint). The exact matrix run used test-source HEAD `928066233ce9cbc993e7f2a38f2fddfe78c283b4` / tree `42367238f39a7a5c061fac53317cb9877ece1ec4`, with the product implementation inherited unchanged from `fc1c`.

### Changed files on the owned branch, relative to `fc1c`

- `android/app/src/test/java/app/serea/android/CloudEgressBodyLineageAttackMatrixTest.kt`
- `docs/agent-reports/g5-cloud-egress.md`
- `docs/agent-reports/evidence/g5-cloud-egress-20261028/SHA256SUMS`
- `docs/agent-reports/evidence/g5-cloud-egress-20261028/attack-matrix-before-product-patch.junit.xml`
- `docs/agent-reports/evidence/g5-cloud-egress-20261028/attack-matrix-renamed-exact-head.junit.xml`
- `docs/agent-reports/evidence/g5-cloud-egress-20261028/attack-matrix-renamed-exact-head.log`
- `docs/agent-reports/evidence/g5-cloud-egress-20261028/attack-matrix-renamed-pre-fix.junit.xml`
- `docs/agent-reports/evidence/g5-cloud-egress-20261028/attack-matrix-renamed-pre-fix.log`
- `docs/agent-reports/evidence/g5-cloud-egress-20261028/failure-run.txt`
- `docs/agent-reports/evidence/g5-cloud-egress-20261028/existing-egress-suite.log`
- `docs/agent-reports/evidence/g5-cloud-egress-20261028/existing-egress-suite-junit/TEST-app.serea.android.ChatViewModelTest.xml`
- `docs/agent-reports/evidence/g5-cloud-egress-20261028/existing-egress-suite-junit/TEST-app.serea.android.GoogleOriginEgressAdversarialTest.xml`
- `docs/agent-reports/evidence/g5-cloud-egress-20261028/existing-egress-suite-junit/TEST-app.serea.android.LocalFakeRoutingAdversarialTest.xml`
- `docs/agent-reports/evidence/g5-cloud-egress-20261028/existing-egress-suite-junit/TEST-app.serea.android.OllamaCloudAdapterTest.xml`
- `docs/agent-reports/evidence/g5-cloud-egress-20261028/existing-egress-suite-junit/TEST-app.serea.android.SecurityPortsTest.xml`

### Exact local test results and JUnit identities

All commands ran from `android/` with Gradle 8.14.6 and Temurin JDK 21.0.13+9, except the post-fix root-owned report cited below, which used its recorded Temurin JDK 17. The `fc1c` product source was unchanged in Agent B's runs.

1. Existing focused egress suite, source HEAD `fc1c0527eea8ea92509935dc8721d24b27fd7f9d` (product tree `756b6ee4112894f0317ee89bf0a5989bcc9d73c3`; only the test-only matrix file was untracked at that time and was not among selected tests):

   ```text
   ./gradlew --no-daemon -Dorg.gradle.java.home=/tmp/serea-g4-jdk/jdk-21.0.13+9 :app:testDebugUnitTest --rerun-tasks --tests 'app.serea.android.SecurityPortsTest' --tests 'app.serea.android.LocalFakeRoutingAdversarialTest' --tests 'app.serea.android.GoogleOriginEgressAdversarialTest' --tests 'app.serea.android.ChatViewModelTest' --tests 'app.serea.android.OllamaCloudAdapterTest'
   ```

   Result: **72/72 PASS, 0 failures, 0 errors, 0 skips.** JUnit suite identities/counts: `SecurityPortsTest` 10; `LocalFakeRoutingAdversarialTest` 3; `GoogleOriginEgressAdversarialTest` 1; `ChatViewModelTest` 36; `OllamaCloudAdapterTest` 22. Individual test names are in the five committed JUnit XML files under `existing-egress-suite-junit/`.

2. Regression matrix before product fix, product HEAD `fc1c0527eea8ea92509935dc8721d24b27fd7f9d`; exact renamed test-source HEAD `928066233ce9cbc993e7f2a38f2fddfe78c283b4`:

   ```text
   ./gradlew --no-daemon -Dorg.gradle.java.home=/tmp/serea-g4-jdk/jdk-21.0.13+9 :app:testDebugUnitTest --rerun-tasks --tests 'app.serea.android.CloudEgressBodyLineageAttackMatrixTest'
   ```

   Result: **9 run, 4 failures, 0 errors, 0 skips.** Failing JUnit IDs: `gmailFirstTurnModelOutputInheritsCurrentBodyLineage`, `calendarFirstTurnModelOutputInheritsCurrentBodyLineage`, `conversationSwitchDoesNotClearCurrentProviderBodyTaint`, and `boundedContextCannotLaunderGoogleOriginThroughUnlineagedModelSummary`. Passing IDs: `currentUnknownAssistantClassificationStillStopsCloudBeforeCredentialAndHttp`, `modifiedExpiredOrGoogleTaintedCloudRequestsStopBeforeUsageCredentialAndHttp`, `providerBodyTaintSurvivesWholeTextFieldValueReplacementWhitespaceClearAndReentry`, `sameApprovedSendIdCannotDispatchTwice`, and `viewModelStoreReuseRetainsProviderBodyTaint`. JUnit: `attack-matrix-renamed-exact-head.junit.xml`, SHA-256 `a99f330ed982c36734613a3696ad921ba8708cdf1e4c7272db29040d5c98a021`. The earlier pre-fix JUnit (old method labels) remains unchanged, SHA-256 `3e52be226e5c4c0e81a511c055b40c4da06c1c4b6b84014c4c0d6f29489938b7`.

3. Root-owned post-fix focused evidence independently audited from PR #134 parent source HEAD `dd236abf75c26b8ba8f4c5db7c696b7b380a1548` / tree `2ef2610a320cfdab8e3f3e74a6170430848b27a6`:

   ```text
   JAVA_HOME=/tmp/serea-g5-jdk17 PATH=/tmp/serea-g5-jdk17/bin:$PATH ./gradlew --no-daemon :app:testDebugUnitTest --rerun-tasks --tests CloudEgressBodyLineageAttackMatrixTest --tests SecurityPortsTest --tests LocalFakeRoutingAdversarialTest --tests GoogleOriginEgressAdversarialTest --tests ChatViewModelTest --tests OllamaCloudAdapterTest
   ```

   Result: **81/81 PASS, 0 failures, 0 errors, 0 skips** (9 + 10 + 3 + 1 + 36 + 22). Every entry in `after-lineage-fix-dd236ab/SHA256SUMS` matched its artifact. The XML confirms 9/9 attack-matrix tests passed. This is local evidence at `dd236ab`, not a GitHub Actions artifact and not a test run on the later PR #134 head.

### Provenance, root cause, and security review

- Pre-fix root cause confidence: **CONFIRMED**. On `fc1c`, `ChatViewModel.sendMessage()` formed assistant lineage from `MODEL_OUTPUT`, `USER_AUTHORED`, and prior context, omitting the current `userMessage.sourceLineage`. The first-turn tests show the exact Gmail/Calendar lineage loss; a saved-conversation switch reproduces the same lost assistant ancestry.
- Pre-fix impact: no observed current Cloud disclosure. Generated assistant classification was `UNKNOWN`; the separate test verified Cloud `CLASSIFICATION_DENIED`, with 0 usage reservations, 0 credential reads, and 0 HTTP dispatches. The bounded-context failure is latent: a test-only PUBLIC copy of the unlineaged summary is approved after the source user message falls outside the context limit.
- Root fix reviewed in PR #134: commit `9b9e7fa53b744c991fa575e73ba739b2d0c66553` adds `addAll(userMessage.sourceLineage)` to assistant lineage. Commit `3ba450ec1d6c4adf0b29daf42909e28fa242f087` adds an explicit Gmail-lineage assertion after bounded-context construction. Exact source code in PR #134 at `e6f3a2593ce9ad652a34e7eda68374d70d5d95e1` was independently reviewed; the 81 local tests passed at its parent `dd236ab`.
- Gate contract inspected: Cloud source lineage is restricted to `USER_AUTHORED` and `MODEL_OUTPUT`; `UNKNOWN`, Gmail, and Calendar ancestry is denied. The gate requires the fixed Ollama destination, non-expired explicit-intent timestamp, PUBLIC classification, raw preparation, and valid bounded payload. `EgressCheckedModelChatPort` matches request body/context/send/conversation/classification/model/lineage to the contract before its delegate; `OllamaModelChatPort` rechecks and passes only a gate-created payload with exact destination/model/send/lineage. The outer Egress wrapper precedes UsageControlled; credentials are read and HTTP is built/dispatched only after adapter validation. Tests assert zero reservation/read/dispatch on denials. LOCAL_DEMO uses a separate local contract.
- Approval limitation: there is no production caller of `DirectProviderChatComposition.create` or production Cloud `EgressContract` construction. The contract binds body and destination values to the routed request and timestamp/expiry, but there is no live consent UI or opaque approval token; therefore actual user-consent creation/replay semantics are NOT_RUN and must be designed before enabling Cloud.
- Other limits: no Gmail/Calendar-to-Chat production handoff or source-session/account generation binding; arbitrary clipboard provenance is not inferred; no real Google account, Cloud credential, or external request; whole-`TextFieldValue` replacement test is not IME character-level editing; ViewModelStore reuse test is not Activity recreation.

### Integration recommendation and latest descendant check

**ACCEPT** the narrow root lineage fix and focused test evidence for integration, while keeping Cloud disabled. Do not integrate Agent B's red test-only branch alone as a green candidate; its purpose is to preserve the failing-before-fix regression evidence. Real Cloud activation and Personal Beta remain out of scope/not approved.

PR #134 exact reviewed revision was `e6f3a2593ce9ad652a34e7eda68374d70d5d95e1` / tree `729abc9587194733483b3bb187eca01c45783f12`. During follow-up, PR #134 advanced to `b84feb337cd57918346ab31cb3ea4af4ba880ffb` / tree `9133d4a3ba2ab870d126e56cdd22336b856c94e1`. The exact e6-to-b84 diff contains only `.github/workflows/android-api35-real-ime-independent-bc.yml` and `tools/run_serea_phase_r3_single_ime_case.sh`; the workflow/script changes bind the IME diagnostic build and APK provenance to the expected PR-head SHA. There are no Cloud product or Cloud test changes in that delta. Thus this narrow Cloud acceptance remains applicable at b84, but not to any later unseen commit.

The exact-e6 GitHub workflow snapshot (queried during review) was not an all-green candidate: Fast CI `40662590940`, Git hooks `40662590692`, RustSec `40662590863`, and Android Personal Beta contract tests `40662590766` succeeded; Android Required `40662590743` failed at its final required-check verifier with several jobs cancelled; Real IME diagnostic `40662590463` failed its APK-build job and skipped the API cases; Full `40662590523`, SQLite `40662590720`, and Native `40662590661` were cancelled. The main-only Beta workflow Run `40662543653` failed with zero jobs. These runs are not Agent B evidence and do not transfer to the current PR #134 head. GitHub artifacts observed for those PR checks: `12804023663` IME build evidence; `12804528138` Android security JUnit; `12804432729` Android dependency audit; `12804098038` Rust security. Agent B itself started no GitHub Actions run and has no remote artifact ID.

### Exact-b84 GitHub JUnit artifact audit

Read-only inspection of Android Run `40664188621`, source HEAD `b84feb337cd57918346ab31cb3ea4af4ba880ffb`: Android Unit and Android Security jobs were successful; the overall run was still in progress at inspection. Artifact digests matched the downloaded ZIP bytes:

| Artifact | JUnit result | ZIP SHA-256 |
|---|---|---|
| `12805840704` — `serea-android-unit-junit` | 350 tests; 0 failures, 0 errors, 0 skips. `CloudEgressBodyLineageAttackMatrixTest`: 9/9 pass. | `bab79ecf7f04f5f3b5c76adf5898a640681d770e3eecad4c82966e67de9da34b` |
| `12806135450` — `serea-android-security-junit` | 26 tests; 0 failures, 0 errors, 0 skips. | `bb641c916186549698d40070213d904db03c22a27adda6666831422413bc51d0` |

This confirms the lineage regression suite on exact PR #134 head `b84feb337cd57918346ab31cb3ea4af4ba880ffb` and strengthens the conditional acceptance of the fix. It does not close the overall Required run or enable Cloud: real Google consent/session handoff, real Cloud credentials, and external dispatch remain untested, and Cloud must remain disabled.

### Checkpoint ownership

The dedicated checkpoint already exists at `docs/agent-reports/g5-cloud-egress.md` on the owned branch `codex/serea-g5-cloud-egress-20261028`. This structured appendix is on that branch only. No PR #132/#133 or orchestrator worktree was modified.
