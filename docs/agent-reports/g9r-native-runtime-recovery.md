# G9R Native Runtime Recovery — Agent A

**Checkpoint:** 2026-10-29 UTC
**Status:** PARTIAL. The AndroidJUnit4 test method validation defect is fixed and hosted evidence now reaches the test body, but the bounded native E2E run timed out before establishing model inference. Current lifecycle/offline test additions are committed locally and have not run on an emulator or hosted runner.
**Base:** G7 / PR #139 SHA `0560e1e7bcd13f0194eab8caca89d70ea5a2f142`.
**Worktree:** `/workspace/serea-g9r-native-recovery`.
**Branch:** `codex/serea-g9r-native-recovery`.
**Local source:** SHA `0f9926b9995b06c0651eb2dadc698064676d74bc`; tree `e1a9d625da149c3b3db4649dc26d0014067f8950`.
**Remote PR #158:** Open, Draft; last verified remote head `848fe67e4d8dd2de21ffb05793c4945127edcb06`. Local commits after `848fe67` are not yet pushed and have no hosted CI evidence.

## Runner diagnosis and hosted evidence

PR #147 run `40843204437`, attempt 1, source `36700f6c99d15db428c6d434c657ac76593df3f9`, passed x86_64 and ARM64 native APK builds, model fixture validation, and KVM setup. Instrumentation failed before test execution with `Failed to instantiate test runner class androidx.test.internal.runner.junit4.AndroidJUnit4ClassRunner`. That run did not contain a complete cause chain, so its original error alone did not identify the cause.

The later run `40858521185`, attempt 1, source `212a6773fd7038529526b18f51c20f9043e0f86f`, tree `b35e4f6a5006cfc0504b6c197c62cd0cb57d27c8`, isolated the defect: the no-op test on the same runner passed, while `LocalModelNativeRuntimeE2ETest` failed JUnit validation with `InvalidTestClassError`, because its expression-bodied `@Test` returned `Int` from the final `Log.i`. The test method was changed to explicitly return `Unit` and use `runBlocking<Unit>` in commit `848fe67e4d8dd2de21ffb05793c4945127edcb06`. C accepted that narrow source fix; acceptance did not include runtime success.

The subsequent run `40860331937`, attempt 1, job `123401642979`, was bound to `848fe67` / tree `57ab7cd83ef100cb5122dd39463186234cafc0d0` and ended **FAILURE**. Native CMake/static contracts, x86_64 and ARM64 APK builds, model metadata/license checks, fixture size/hash validation, and KVM setup passed. The no-op runner identity passed. The Native test body then failed with `TimeoutCancellationException: Timed out waiting for 15000 ms`; instrumentation reported 2 tests and 1 failure. The evidence does not identify which sub-operation timed out, so it does not prove either a successful load or generation.

Evidence artifact `12891353567` (24,280 bytes, SHA-256 `e4b829dc964697164ffb1a7844018a030b487819e9f745e0ece1d1fb8805acde`) records the no-op PASS, Native 15-second timeout, APK identities, and cleanup PASS: app-private model `ABSENT_VERIFIED`, host/device temporary files `ABSENT_VERIFIED`, and both packages uninstalled. Build artifact `12891333258` (5,979 bytes, SHA-256 `708ab18a0cac841b73785cf8dbe4aa543a2efd62d1c90766ee9f27872317d2d8`) is associated with that run. Keep this failed attempt in the run history.

An earlier diagnostic run `40855571172` failed in the harness after instrumentation parsing because a shell arithmetic comparison treated symbolic `PASS` as an unset variable. The fix split numeric and symbolic comparisons. That attempt also reported app-private cleanup as unverified; cleanup was not inferred from package uninstall. The later `848fe67` run above has the direct cleanup evidence.

## Current local candidate changes (not hosted-tested)

Commit `73fab8b02b15ac1ff7547a3257dccf7f2d8cb6a4` adds non-Required Android instrumentation coverage and JNI test seams for active-generation progress/cancellation, backend double-close followed by a fresh load and generation, and Android INTERNET permission denial for both isolated target and test APK. Commit `0f9926b9995b06c0651eb2dadc698064676d74bc` bounds the injected failure-seam model load to 60 seconds and ensures close/release is awaited in `finally`.

The INTERNET assertions establish Android permission isolation only. They are not proof that all socket access is impossible. Cancellation, double-close/reload, and the new offline permission checks have not yet been executed on Android. The native test is diagnostic-only and is not added to either Required or Additional manifest.

## Exact-source local checks

The following checks ran on local HEAD `0f9926b9995b06c0651eb2dadc698064676d74bc`, tree `e1a9d625da149c3b3db4649dc26d0014067f8950`.

| Command / evidence | Result |
|---|---|
| From `android/`: `env JAVA_HOME=/workspace/.tools/temurin17 PATH=/workspace/.tools/temurin17/bin:$PATH ./gradlew --no-daemon --console=plain :app:processIsolatedMainManifest :app:processIsolatedAndroidTestManifest :app:compileIsolatedAndroidTestKotlin` | PASS, exit 0; 29 tasks up-to-date. Log: `/tmp/serea-native-0f9926-androidtest-compile.log`. |
| From `android/`: `env JAVA_HOME=/workspace/.tools/temurin17 PATH=/workspace/.tools/temurin17/bin:$PATH ./gradlew --no-daemon --console=plain :app:testIsolatedUnitTest --rerun-tasks --tests 'app.serea.android.LocalModelChatAdapterTest' --tests 'app.serea.android.LocalModelNativeRuntimeTest'` with a 15-minute timeout | PASS, exit 0; 41 tasks executed, 22 tests total, failures/errors/skips all 0. Log SHA-256 `b646fa2944af7d8061c9385d46c4193ae99e3da4360d75b3ea20e06a06659adf`. |
| `LocalModelChatAdapterTest` JUnit XML | 21 tests, 0 failures/errors/skips; SHA-256 `2ac66f3c1702ca169e3ceecaf6c5ef624bb913d3f24d591fa9ac06dc35b6044d`. |
| `LocalModelNativeRuntimeTest` JUnit XML | 1 test, 0 failures/errors/skips; SHA-256 `3d171942af5980ac065a2f9e02f873531004f155ecfb889401e46c802ce0c4c6`. |
| `bash -n tools/g8-local-model-native/run-e2e.sh` | PASS. |
| `python3 tools/g8-local-model-native/test-jni-contract.py` | PASS; 11 exports and failure seam checked. |
| `bash tools/g8-local-model-native/test-preflight.sh` | PASS; unsafe device/package/evidence states rejected. |
| `bash tools/g8-local-model-native/test-cleanup.sh` | PASS; cleanup/report gates remain fail-closed. |
| `bash tools/g8-local-model-native/test-instrumentation-gate.sh` | PASS; 10 cases. |
| `python3 tools/g8-local-model-native/test-instrumentation-stream.py` | PASS; 8 cases. |
| `git diff --check` | PASS after report whitespace cleanup. |

The JVM tests exercise adapter/runtime contracts and are not evidence of JNI execution or Android inference. The manifest and AndroidTest compile is source-bound but all tasks were up-to-date; hosted ABI APK and instrumentation execution remain required.

## Runtime, model, and security gates

| Gate | Status | Evidence / limit |
|---|---|---|
| x86_64 build | PASS on hosted `40860331937` at `848fe67` | Both ABI APK builds passed; no new hosted build evidence for `0f9926b`. |
| ARM64 build | PASS on hosted `40860331937` at `848fe67` | Build only; ARM64 runtime is separate. |
| Fixture identity | PASS on hosted `40860331937` at `848fe67` | SmolLM2 GGUF revision `1076b85d135017971845b19b8d07df1479155988`, size 101,016,128 bytes, SHA-256 `e3131339bf4e8065265593d4fd8f7bb7ff2d3abff1edb5618aa1197b89cad9f5`. |
| llama.cpp identity | Pinned | `276378515ceebee5d14bcc8536603dd5c87491a2`. |
| x86_64 native runtime / inference | NOT_ESTABLISHED | E2E body ran at `848fe67` but failed at the inherited 15-second deadline. Which operation timed out is unknown; do not claim inference. |
| Usage/egress, two-turn inference, cancellation, native release/reload | NOT_RUN on Android | Only JVM contracts and test code exist for the latest candidate. |
| Offline enforcement | PARTIAL / NOT_RUN on Android | The candidate asserts INTERNET permission denial for the two APK contexts. No hosted execution and no general socket-denial proof. |
| Process-death recovery | NOT_RUN | This test does not kill and relaunch the process owning the backend or demonstrate product-level native model rehydration. |
| Load time, TTFT, prefill/decode rate, peak/residual memory, thermals | NOT_RUN | Existing timing/PSS instrumentation is not a reliable completed measurement set. |
| ARM64 runtime | NOT_RUN | Build does not establish runtime. |
| Pixel 7a physical acceptance | NOT_RUN | No device operation was performed. |

The model evaluation document at `docs/agent-reports/g8n-native-bonsai-evaluation.md` was corrected on docs-only head `943fb4e0b409da3d7f342f9d18a60188d40cd262` / tree `2d2d2e60e2cf0ee12d2d1cb75ac796acdf4204b2`; C accepted that documentation review. It covers the official metadata, formats, licenses, and static compatibility checks for LFM3-1.7B, Bonsai 2 2B/4B/9B, Qwen3.5 2B, Gemma 4 E2B, and FunctionGemma 270M. No additional weights were downloaded. Serea execution and performance remain NOT_RUN; standard model selection remains UNDECIDED pending independent review and Pixel 7a evidence.

## Current checkpoint and next action

**AGENT_ID:** A — Native Runtime Recovery
**ACTIVE_PHASE:** Runner recovery / native instrumentation
**GOAL_STATUS:** PARTIAL
**BASE_SHA:** `0560e1e7bcd13f0194eab8caca89d70ea5a2f142`
**CURRENT_HEAD:** `0f9926b9995b06c0651eb2dadc698064676d74bc`
**TREE_SHA:** `e1a9d625da149c3b3db4649dc26d0014067f8950`
**WORKTREE:** `/workspace/serea-g9r-native-recovery`
**BRANCH:** `codex/serea-g9r-native-recovery`
**DRAFT_PR:** #158, remote head remains `848fe67e4d8dd2de21ffb05793c4945127edcb06` until this candidate is pushed.
**COMPLETED_WORK:** Fixed JUnit test signature; bounded current failure-seam load; added non-Required JNI lifecycle/cancellation/offline instrumentation; exact-source AndroidTest compile and selected JVM rerun pass.
**CHANGED_FILES:** `android/app/src/androidTest/AndroidManifest.xml`; `android/app/src/androidTest/java/app/serea/android/LocalModelNativeRuntimeE2ETest.kt`; `android/app/src/isolated/AndroidManifest.xml`; `android/app/src/main/cpp/llama_jni.cpp`; this report.
**TEST_COMMANDS:** As listed above; exact-source Android/JVM outcomes are source checks, not real inference.
**JUNIT_RESULT:** 22 selected JVM tests, 0 failures/errors/skips; no current-head Android instrumentation JUnit.
**CI_RUN_IDS:** Historical: `40843204437`, `40855571172`, `40858521185`, `40860331937`. No run for current local head.
**ARTIFACT_IDS:** `12891353567` evidence artifact and `12891333258` build artifact belong to failed run `40860331937`, not current local head.
**SECURITY_IMPACT:** No Required gate changed. Offline permission assertions are not broad socket isolation. Cleanup must remain fail-closed.
**REVIEW_STATUS:** C accepted the narrow source fix at `848fe67` and the docs-only model report at `943fb4e`; lifecycle/offline additions at `0f9926b` await exact-head review.
**KNOWN_ISSUES:** No hosted test on current head; native inference not established; process death and performance not measured.
**CURRENT_BLOCKERS:** Push and exact-head hosted Native workflow; independent C review.
**NEXT_ACTION:** Run `git diff --check`, commit this report, push normally as a fast-forward to the existing Draft PR branch, verify PR state/head, then request C review and exact-head Native workflow. Preserve the failed `848fe67` run in history.
**OWNER_APPROVAL_REQUIRED:** Pixel 7a use and any later Beta/production actions remain outside this lane; no such action taken.

## Successor checkpoint — 2026-10-29

The history above is retained from PR #158. This section supersedes its old `CURRENT_HEAD`, `DRAFT_PR`, test, CI and next-action fields for the successor only.

- **AGENT_ID:** A — Native Runtime Recovery
- **GOAL:** Prove local SmolLM2 inference through Serea's Native Backend and keep cleanup fail-closed.
- **STATUS:** PARTIAL / PENDING_REVIEW
- **BASE_SHA:** `80e9990625b000e9d56a78743a7460ac120d4979`
- **SOURCE_HEAD:** `447e151738cb4e98c5d97cddf47b96e0d1035656`
- **TREE_SHA:** `8787bbe744c2b67fc2950f2e58930297b65c8447`
- **BRANCH:** `codex/serea-g9r-native-recovery-followup`
- **WORKTREE:** `/workspace/serea-g9r-native-recovery`
- **DRAFT_PR:** #160
- **CHANGED_FILES:** `LocalModelNativeRuntimeE2ETest.kt`, `llama_jni.cpp`, `run-e2e.sh`, `test-cleanup.sh`, this report.
- **TESTS:** cleanup harness PASS; shell syntax PASS; diff check PASS. Android/JNI compilation and runtime not locally executed yet.
- **CI_RUNS:** Android `40882489925` reached debug/release/instrumentation APK build and was linting as of the latest query; Native focused `40882490072` remains queued. These are bound to source `447e151…`.
- **ARTIFACTS:** no exact-head inference artifact. Earlier artifacts above remain tied to their original sources.
- **SECURITY_REVIEW:** CHANGES_REQUESTED for loss of report history and raw instrumentation/logcat tails emitted before cleanup verification. Both issues are being fixed in a successor commit.
- **KNOWN_FAILURES:** prior run `40866167732` ended with failed ADB package query and device-temp cleanup; no runtime artifact; inference remains NOT_VERIFIED.
- **BLOCKERS:** hosted focused Native runner queued; local environment lacks JDK compiler, NDK, and Emulator; reviewer findings require successor fix.
- **NEXT_ACTION:** preserve this report history, defer raw log output until cleanup is verified or emit only bounded sanitized diagnostics, add combined-failure coverage, then wait for exact-source Android/Native runs and re-review.
- **OWNER_APPROVAL_REQUIRED:** physical Pixel 7a remains prohibited without Owner approval.

No runtime PASS is claimed.

## Review findings and successor correction

The independent review of #160 source `447e151738cb4e98c5d97cddf47b96e0d1035656` returned **CHANGES_REQUESTED** for two findings. First, that source had replaced this report with a short summary, hiding the historical run, JUnit, artifact and model evidence above; this successor restores the history and appends the current checkpoint. Second, the E2E gate-failure branch printed raw instrumentation and logcat tails before cleanup could fail. The successor removes raw console tails, prints only allowlisted status/JUnit fields, and adds a combined instrumentation-gate + cleanup-failure harness case that proves raw markers are not emitted and evidence is removed.

On exact #160 source `447e151…`, Android run `40882489925`, job `123480094333`, completed Build/Lint/Instrumentation APK compile successfully. Build artifact `12901867407` (`serea-android-pixel7a-debug-apk-candidate`, 33,146,769 bytes, digest `sha256:5d887e5f5ff688675e813af2d7f43fe143cd7bc8f290bee81b094f0e13d12c89`) is build-only evidence; artifact download returned HTTP 403, so APK hash/signer was not inspected. Native focused run `40882490072` remained queued without an assigned runner; no JUnit/runtime artifact exists and inference remains NOT_VERIFIED.

The cleanup harness passes after the correction, including the combined gate/cleanup failure check. The corrected successor's exact SHA/tree and fresh formal CI are recorded in the Parent Ledger; no previous-source result transfers to it.
