# G9RC Orchestrator Ledger — 2026-10-29

## Authority and protected boundaries

- Protected `main`: `e0acb0254820c381dd97828824bef86a1db5176c` (tree was confirmed unchanged; no merge or push to main).
- PR #155 base: `d4a36fb5d3613bca2d60151de0991a1baac72fc9`; HEAD `c6d9b3971c19d75c6824de616e703b105b6be71f`; tree `b5adb0cd42e33d4e3b065dc6f9e9e716bbee5448`; OPEN/DRAFT. Its current branch CI is partial. Exact-head Fast `40882853803`, RustSec `40882853433`, P7 synthetic Full job `123478701415` passed. Full Linux run `40882853605` remains in progress, Android run `40882853509` has some jobs queued, SQLite `40882853749` in progress, Native matrix `40882853542` partly queued/in progress. This docs-only HEAD has no Android product-test evidence.
- PR #159: Owner decision packet review ACCEPTED for its O-01–O-10 matrix. Exact source `982c8271698c5ea323e7a4d19f4dd662087db2c5`, tree `11e3c0ca61577bb435e0f69b1fcd24313857ff1f`; Android run `40881913899` docs and Required Gate contract passed, while product Build/Unit/Integration/Security/Audit/Emulator/Identity jobs were SKIPPED. This is not product acceptance.
- No real Google OAuth/account/data, Cloud API key or billing, production signing, physical Pixel 7a operation, release distribution, or personal data deletion occurred. Personal Beta remains NO-GO.

## Agent outcomes

### Agent A — Native Runtime

- Original Native worktree reached PR #160. Current source `025aff71effb03e0fe3bd540079530369e9beaf7`, tree `73ff9019802aba4a7ac9437e52ff407842fbfcdc`.
- Agent C independently ACCEPTED the corrective diagnostics patch: historical report/run/artifact evidence restored; raw instrumentation/logcat console tails removed; only allowlisted status/JUnit summaries printed; combined gate+cleanup failure harness checks that raw markers do not escape and evidence is removed.
- Exact-source local checks: `bash tools/g8-local-model-native/test-cleanup.sh` PASS; `bash -n tools/g8-local-model-native/run-e2e.sh` PASS; `git diff --check` PASS.
- Exact #160 focused Native run `40884851425` attempt 1/job `123485695046` remains QUEUED with no runner; Android run `40884851432` is PENDING with no jobs. No Native JUnit or inference artifact exists. Two-turn generation, Usage/Egress, cancellation runtime and recovery remain NOT_VERIFIED.
- Prior source `447e151738cb4e98c5d97cddf47b96e0d1035656` Android run `40882489925`, job `123480094333`, passed APK Build/Lint/Instrumentation compile. Artifact `12901867407`, 33,146,769 bytes, digest `sha256:5d887e5f5ff688675e813af2d7f43fe143cd7bc8f290bee81b094f0e13d12c89`; download was denied (HTTP 403), so APK hash/signer were not inspected. This build-only evidence is not transferred to `025aff7…`.
- `.github/workflows/android-personal-beta.yml` push run `40884839863` ended failure with `jobs=[]`; the workflow is dispatch-only/main-guarded. Classified as a no-job orchestration event, not a product/release failure; no dispatch/retry.
- **Review:** diagnostics patch ACCEPTED. Full Native/runtime gate PENDING_REVIEW / NOT_VERIFIED pending exact-head build and inference.

### Agent B — Android UI / Google / Gmail

- PR #161 initial source `0941043a1e687ec3b4e4778ce387c7f2c4b70ba1`, tree `e64ddd7bafd7c03de3898f2c2503f6ff1aff8604`, was independently reviewed. The HTTPS fixture boundary was ACCEPTED, but local compilation exposed invalid `Dns` SAM lambdas and a missing Compose extension import; a first compiled JVM run also exposed two assertions that expected an unavailable Host header. Those failed attempts are retained in the agent report and were fixed.
- PR #161 follow-up HEAD `ff42d9bcd08010dd7100d708d17fc676bcd5e739`, tree `eadb4800efcf837d63348eb9398d37310f364fb5`, remains OPEN/DRAFT. It changes tests/test helpers/report only; no production code, composition, app/test manifest, Required identity, or Required Gate was changed.
- The test-only client restricts initial requests to HTTPS `www.googleapis.com:443`, exact Calendar routes, GET and allowlisted query keys; it blocks redirects, rejects non-local DNS and rewrites to an ephemeral HTTPS localhost MockWebServer. The post-rewrite `RecordedRequest` asserts localhost URL. No real account/data is used.
- Official Adoptium Temurin JDK `17.0.21+9` was extracted under `/workspace/.tools` (no system Java change); official package SHA-256 matched `8b9b611d917e3dfd8419d3566b5949226f69b052fed7e109c1368add1dfe8711`.
- Exact #161 source local command `./gradlew :app:testDebugUnitTest :app:compileIsolatedAndroidTestKotlin --rerun-tasks` with JDK17 completed SUCCESS: 408 JVM tests, 0 failures, 0 errors, 0 skips; 34 JUnit XML files; aggregate per-file digest `6ec0718bebbe5a188b0eeb70fb41066c09931eb3200229be2d6a6622d82ad11b`. Isolated AndroidTest Kotlin compile passed in the same run (55 tasks executed).
- Android SDK has Build Tools 35.0.0, platform 35, and platform-tools 37.0.1 only; no API26 platform, NDK, or Emulator binary/device. Physical/API26/35 UI runtime was not run locally.
- Agent C review of exact `ff42d9b…` accepted the fixture boundary and assertion changes. Fresh GitHub Android run `40886083127` is PENDING with no jobs/artifacts; API26/35 UI and Required results remain NOT_RUN. Beta workflow `40886075461` also ended with zero jobs; no retry was issued.
- **Review:** test transport/security and assertion changes ACCEPTED; UI/runtime acceptance remains PENDING exact-head Android jobs.

### Agent C — CI / Security / Release

- Reviewed exact sources and run/artifact provenance for PRs #155/#159/#160/#161; did not modify files.
- PR #159 decision matrix ACCEPTED. PR #161 fixture boundary ACCEPTED. PR #160 diagnostics correction ACCEPTED; Native inference BLOCKED/NOT_VERIFIED. PR #155 formal CI remains partial.
- Owner gates remain closed for OAuth, Cloud, Restricted Gmail activation, production signing/distribution, Pixel, main merge and Beta GO.

## Parent integration candidate

- Started a new branch `codex/serea-g9rc-ui-accepted` from PR #155 exact head `c6d9b3971c19d75c6824de616e703b105b6be71f` (tree `b5adb0cd42e33d4e3b065dc6f9e9e716bbee5448`).
- Selectively cherry-picked only Agent C-accepted PR #161 UI-test changes: `0941043a1e687ec3b4e4778ce387c7f2c4b70ba1` and `ff42d9bcd08010dd7100d708d17fc676bcd5e739`. Result before this Ledger commit: HEAD `dd45ef48ae4a494235292f7d9f9b50d203a75537`, tree `3cf35a4379fb4628a49c3ec30961dc87a9ef4fea`.
- Native Runtime source was not integrated because no exact-head inference/runtime evidence exists. No composition-root changes were integrated.
- This branch is intended for a new Draft integration PR based on PR #155's branch. Exact integrated-source CI must still run. API26/35 UI acceptance stays PARTIAL until JUnit identities/counts/artifacts prove execution on both API levels.

## Current gates / next action

- G8 Evidence Closure: PARTIAL.
- Android Recovery: PARTIAL (local JVM and instrumentation compile pass; API26/35 UI runtime pending).
- Native Runtime: PARTIAL; Real Model Inference NOT_RUN.
- G9 Functional Candidate: PARTIAL.
- Model Selection Preflight: PARTIAL; static official-source review only, standard model UNDECIDED, no weights downloaded.
- G10 Preflight: PARTIAL; decision packet accepted, product release rehearsals and evidence remain.
- Personal Beta: NO-GO.
- Next: finish the exact integrated-source JDK checks, push this branch as a Draft PR, monitor its API26/35 Required/UI jobs without retries while runner-unassigned, and continue #160 exact-head Native build/inference when the focused runner starts. Re-review any changed source SHA before accepting. No main merge or owner-only action.

## Integrated candidate focused verification

Before this Ledger update, the new candidate source was `8ea137642ef6a0c7ef0e2a78905569ea7d598b58`, tree `779c2e05a9d2bd3c62b71566c5c2e049a3aed0f9`. On that exact source, Java 17 command `./gradlew :app:testDebugUnitTest :app:compileIsolatedAndroidTestKotlin --rerun-tasks` completed SUCCESS: JVM 408 tests, 0 failures, 0 errors, 0 skips, 34 JUnit XML files; AndroidTest Kotlin compile passed. Aggregate per-file JUnit digest: `3c4283818089c7c52f3ca531eae80e98bee3c93fe16d4fd75ecaa3490d46190e`. No emulator was run. This result is local focused verification and does not replace API26/35 Required/UI CI.

The new integration branch is being kept separate from PR #155. It contains the reviewed test-only UI transport fixes and this ledger. A new Draft PR and exact-head hosted CI are the next steps; Native diagnostic code remains only in PR #160 because Native inference has not been established.

## #162 current-source checkpoint — 2026-10-29

PR #162 is OPEN/DRAFT on `codex/serea-g9rc-ui-accepted`, based on PR #155's branch `codex/serea-g9r-integration-candidate`. Before this checkpoint the exact candidate HEAD was `6a37e090a228633faf319fd6c8fa1596e7ae9c9f`, tree `b697ce9f197cf826d343dfa6e60ecef8f2e1542d`. This source contains only the reviewed PR #161 test-only changes plus the new ledger atop PR #155; no Native Runtime source was integrated.

On that exact candidate source, JDK17 `./gradlew :app:testDebugUnitTest :app:compileIsolatedAndroidTestKotlin --rerun-tasks` completed SUCCESS: 408 tests, 0 failures/errors/skips, 34 JUnit XML files, isolated AndroidTest Kotlin compile PASS. Aggregate JUnit digest: `95c50242b30e61e08d11dec9a1c67b2e48cc473b3cd8a9791658aa9b553c7b59`. No Emulator was run locally.

Exact-head PR #162 checks started at run IDs: Android `40887554119` (Change Scope job `123495223453` QUEUED), Fast `40887553700`, Full `40887553860`, Native platform `40887553695`, SQLite `40887553607`, RustSec `40887553982`, Git Hooks `40887554005`; these were QUEUED at the latest query. They are bound to `6a37e09…` and are not yet Required/UI evidence.

PR #160 focused Native run `40884851425`, attempt 1/job `123485695046`, has advanced from QUEUED and started at `2026-10-29T14:01:15Z`. Exact source remains `025aff71effb03e0fe3bd540079530369e9beaf7`; at last query it was installing pinned Android build/Emulator packages. The inference step has not run yet; no result or artifact is claimed.

## Final resume head checkpoint

- Latest PR #162 source after the Ledger-only checkpoint: `df3dc8233897c29e6945377d4d8681b808aeea55`, tree `07e5e3a39d29370f7f44dad6fe6b32fb2ac35578`; PR remains OPEN/DRAFT. The local 408/408 + compile evidence above is bound to predecessor `6a37e09…` and is not relabeled as a test of `df3dc82…`.
- Exact `df3dc82…` PR workflows: Android `40887650949` PENDING/no jobs; Fast `40887651018` PENDING; RustSec `40887650761` PENDING; Full `40887650975`, Native platform `40887651037`, SQLite `40887650946`, Git Hooks `40887650864` QUEUED. No exact-head product artifact was present at this checkpoint.
- PR #160 exact Native run `40884851425`, attempt 1, source `025aff71effb03e0fe3bd540079530369e9beaf7`, job `123485695046`: setup, Android package install, clean-source guard, and source identity/focused JVM-contract stage completed; current step is the two-ABI JNI APK build. Model license/fixture verification, KVM/Emulator, inference, Usage/Egress, cancellation runtime, artifact upload and cleanup are not yet completed. No JUnit/artifact is available. No retry was issued.
- Stop/restart boundary: continue polling these exact runs on next resume; do not apply old artifacts to new sources. Keep G8/G9 PARTIAL, Native inference NOT_VERIFIED, G10 PARTIAL, and Personal Beta NO-GO until the named gates have executed.

## V3 recovery checkpoint — 2026-10-29 14:10 UTC

This section supersedes the preceding resume checkpoint for current status. GitHub refs confirmed protected `main` remains `e0acb0254820c381dd97828824bef86a1db5176c`; no main changes were made.

### Current integration source before this documentation repair

- PR #162 is OPEN/DRAFT, base PR #155 HEAD `c6d9b3971c19d75c6824de616e703b105b6be71f`; exact current source `333e11a25e15ccf3efc923f3ac66b0f245ad306b`, tree `94d3122f4908a2f892dc37b9b3a42adf3453701e`.
- Independent Android UI review: ACCEPTED for the selected PR #161 test-only changes. Diff against base contains only Android/JVM test sources and reports; no production source, manifests, Gradle gate, Required identity manifest, test list, or composition root changed. UI test sources match PR #161 follow-up `ff42d9bcd08010dd7100d708d17fc676bcd5e739`.
- Independent CI/security review: CHANGES_REQUESTED for documentation reconciliation only. The legacy Owner ledger had a competing O-01…O-06 table and the current-head/run checkpoint was stale. This repair imports PR #159's canonical O-01…O-10 ledger and changes the historical G8 release report to link to that authority; PR #159 remains separately OPEN/DRAFT and is not merged.
- On exact `333e11a…`, local isolated Temurin JDK 17.0.21+9 command `./gradlew :app:testDebugUnitTest :app:compileIsolatedAndroidTestKotlin --rerun-tasks` completed SUCCESS (Gradle 55 tasks executed): 408 JVM tests, 0 failures/errors/skips across 34 XML files; isolated AndroidTest Kotlin compilation PASS. This is local focused evidence only. No API26/API35 emulator ran locally.
- Exact-head current Actions at the time of this checkpoint: Android run `40888081858` attempt 1, Change Scope job `123497539347` QUEUED; no API26/35 or JUnit artifacts. Full `40888081982` queued/partly in progress; Fast `40888082032` queued; SQLite `40888081856` queued; Git Hooks `40888081851` queued; Native platform `40888081930` queued/partly in progress; RustSec `40888081887` queued. There are no PR #162 Android artifacts yet. The beta workflow run `40888070402` ended failure with no jobs; no product result and no retry.

### Native evidence recovered

- PR #160 exact source `025aff71effb03e0fe3bd540079530369e9beaf7`, tree `73ff9019802aba4a7ac9437e52ff407842fbfcdc`; run `40884851425`, attempt 1, job `123485695046`, completed SUCCESS at 14:09:19 UTC. All workflow steps passed, including strict source guard/JVM contracts, x86_64 + ARM64 JNI APK assembly, bounded diagnostic upload, pinned model license check, fixture byte/hash verification, KVM, exact API35 x86_64 instrumentation, sanitized runtime evidence upload, and model fixture cleanup.
- The instrumentation gate accepts exactly the no-op runner identity `SereaNativeRunnerNoOpTest#androidJUnit4RunnerInstantiatesAndTargetsIsolatedApp` plus Native identity `LocalModelNativeRuntimeE2ETest#pinnedModelRunsThroughLocalModelEgressUsageAndNativeBackend`; success requires AndroidJUnitRunner code `-1`, two identities, no unexpected/duplicate identities, and both terminal success markers. The test source performs two sequential generated turns through EgressCheckedModelChatPort → UsageControlledModelChatPort → ModelChatPort/JNI, checks per-turn usage attribution/settlement and unknown cost, model-load failure release, double-close/fresh generation, cancellation/release and no INTERNET permission. Exact workflow success means the gate passed. JUnit evidence is **2 identities, 0 failures/errors/skips** by the parser/gate contract. The artifact could not be downloaded in this environment (GitHub artifact endpoint returned HTTP 403), so its XML/APK SHA details are unavailable locally.
- Verified fixture identity: SmolLM2-135M Q4_K_M, Hugging Face revision `1076b85d135017971845b19b8d07df1479155988`, 101,016,128 bytes, SHA-256 `e3131339bf4e8065265593d4fd8f7bb7ff2d3abff1edb5618aa1197b89cad9f5`; llama.cpp commit `276378515ceebee5d14bcc8536603dd5c87491a2`.
- Artifacts: runtime evidence `12904425666`, 30,965 bytes, digest `sha256:6791ecd3e7ae1127f8ee28726489ed0af2971752ee6bd548e05dfaad3375e016`; build diagnostics `12902609513`, 6,055 bytes, digest `sha256:8b73c186d01d748a4bc72e55268fdfa60e908d9be86bc181f0f2aeca7d230617`. Both artifact metadata are visible, but attempted runtime evidence download returned 403. Artifact/APK byte hash and signer were not inspected.
- Gate status: `X86_64_BUILD=PASS`; `X86_64_RUNTIME=PASS`; `ARM64_BUILD=PASS` (ABI packaging only); `ARM64_RUNTIME=NOT_RUN`; `PIXEL7A_PHYSICAL=NOT_RUN`. Performance figures, on-device memory samples and cancellation timing values were not recovered from the inaccessible artifact; record as NOT_RUN. This is a diagnostic branch result, not product Chat integration, API26/35 UI acceptance, or Pixel evidence. Independent Native security review is still pending.

### Owner and campaign status

- `docs/plans/2026-10-26-personal-beta-owner-decision-ledger.md` is the canonical O-ID source; the prior `g8-agent-c-release-preflight.md` now links to it and no longer defines a competing O-ID matrix. All O-01…O-10 remain pending/blocked/undecided as specified; no approval is implied.
- G8 Evidence Closure: PARTIAL. Android Recovery / G9 UI: PARTIAL; Required 105/105 and API26/35 UI remain NOT_RUN at integration source. Native Runtime: PARTIAL with first exact JNI runtime/inference diagnostic PASS, pending independent review and broader lifecycle/product integration. G9 Functional Candidate: PARTIAL. Model selection: UNDECIDED; static candidate comparison in progress. G10: PARTIAL. Personal Beta: NO-GO.
- No model weights, credentials, account data, or artifact contents were committed. No main merge, real OAuth/data, Cloud credentials/billing, production signing/distribution, or physical Pixel use occurred.
- Next: record the successor PR #162 head and its new run IDs after this documentation-only fix; finish independent Native review; continue API26/35 only when GitHub schedules the jobs; integrate no Native code into Chat until review accepts its evidence.

## Accepted review and successor checkpoint — 2026-10-29 14:14 UTC

- PR #162 reconciliation at `0ca371854812d78fa51d41c6acdf143173b005da` / tree `c4a017fe4b115302b92155c4a29640d159f9ceb3`: independent CI/security review ACCEPTED the canonical O-01…O-10 ledger and source-bound UI report reconciliation. The Android/UI source selection review is ACCEPTED; no Required manifest, CI gate, app production code, or composition root changed. `git diff --check` passed.
- Exact `0ca3718…` local verification with isolated Temurin JDK 17.0.21+9 and command `./gradlew :app:testDebugUnitTest :app:compileIsolatedAndroidTestKotlin --rerun-tasks`: BUILD SUCCESSFUL, 55 Gradle tasks executed; 408 JVM tests, 0 failures, 0 errors, 0 skips, 34 XML files; isolated AndroidTest Kotlin compilation PASS. No local Emulator/API26/API35 execution.
- PR #160 Native baseline review: **ACCEPTED** for diagnostic x86_64 inference only at `025aff71effb03e0fe3bd540079530369e9beaf7` / tree `73ff9019802aba4a7ac9437e52ff407842fbfcdc`, run `40884851425` attempt 1/job `123485695046`. Reviewer confirmed native RAII/error containment, serialized ownership, cancellation/release, model/context/consent binding, Usage/Egress wrapper order, and test/cleanup semantics. The review does not accept general product integration: generic context default 2048 exceeds the backend's 128–512 support, `maxResidentMemoryBytes` only caps model file size and does not enforce/measure RSS/PSS, and runtime isolation is permission-denied in the isolated APK rather than universal socket denial. Keep Native code out of ordinary Chat pending configuration, memory enforcement and broader review.
- PR #163 static model matrix from Agent A: branch `codex/serea-model-static-matrix`, source `2c109276bcbdd4e14fb2b6e9558962516f2df32c`, tree `b04164da95bf66f43fd08e5537e4d0e242d6a566`, Draft. Report `docs/agent-reports/g9r-static-model-matrix.md` records pinned public revisions, declared terms/formats and NOT_VERIFIED findings without model weights or selection. FunctionGemma detailed license terms and the mapping of `tiny_garden.litertlm` are NOT_VERIFIED; Bonsai's PrismML fork requirement is recorded, ARM64 compatibility unverified. Independent review is pending.
- API26/API35 Required and Google/Gmail UI remain **NOT_RUN** on #162. Exact `0ca3718…` Android run `40889121454` passed Change Scope, with Android docs/build/unit/security/dependency/integration jobs queued; no API26/35 JUnit is available. Fast CI `40889121524` PASS. Full `40889121231`, Native platform `40889121223`, SQLite `40889121359`, Git Hooks `40889121534`, RustSec `40889121421` remain queued. The workflow `40889111875` failed with no jobs and has no product evidence.
- This checkpoint binds the local 408/408 result to `0ca3718…` only. The ledger-only commit that stores this checkpoint becomes the new PR #162 head and must receive its own exact-head CI; do not transfer this local test or the preceding run IDs to that successor SHA.

## Current PR #162 source checkpoint — 2026-10-29 14:16 UTC

- Exact source before this report-only append: `0b2afe7b354dcd88977ab511fc1c596f26ebb482`, tree `9a7eafdfd2c3a1ba78dc82b71bbd5284351adb16`; PR #162 OPEN/DRAFT. This source contains no Android/code changes beyond the reviewed candidate at `0ca3718…`; the delta is the Ledger checkpoint. The local 408/408 + compile result remains bound to `0ca3718…`, not `0b2afe7…`.
- Exact `0b2afe7…` Actions at capture: Android run `40889688732` PENDING with `jobs=[]` (so API26/35 have no result); Full `40889688632` QUEUED; Fast `40889688599` QUEUED; Native platform `40889688750` PENDING; SQLite `40889688684` QUEUED; Git Hooks `40889688584` QUEUED; RustSec `40889688654` QUEUED. Beta workflow `40889674557` failed with no product test evidence. These runs are all bound to `0b2afe7…`.
- PR #163 static model matrix independent review: ACCEPTED at source `2c109276bcbdd4e14fb2b6e9558962516f2df32c`, tree `b04164da95bf66f43fd08e5537e4d0e242d6a566`; metadata-only, no weights, no candidate measurements/default selection. FunctionGemma terms and the LiteRT sample-to-model mapping remain NOT_VERIFIED; all Android/Serea runtime and quality metrics remain NOT_RUN.
- Historical Runner failure `40843204437` on `36700f6…` logged only `LocalModelNativeRuntimeE2ETest > initializationError` and `Failed to instantiate test runner class`; no cause chain was emitted. Later source explicitly types the coroutine test as `Unit = runBlocking<Unit>`, and the exact `025aff7…` run passes both the NoOp runner and Native test through the same AndroidJUnitRunner. This is a successful recovery; the historical root cause is **not conclusively confirmed** from the old log, so do not report a specific cause as proven.
- The next ledger commit is documentation-only and will become the PR #162 successor SHA. Its CI must be read from new runs on that SHA; no evidence above is transferred.
