# G3 — Real AI / On-device Inference Feasibility

## Continuation update — isolated Android JNI PoC (2026-10-27)

The initial readiness report below predates toolchain provisioning and the isolated PoC. Its no-toolchain/Qwen-candidate statements are superseded by this section; provider-terms findings remain applicable. No product source or composition changed.

**AGENT_ID:** G3 — Real AI / On-device Inference Feasibility
**ACTIVE_PHASE:** isolated llama.cpp Android feasibility PoC
**OVERALL_STATUS:** PARTIAL — isolated adapter/JNI source, ARM64/x86_64 APK build, 3/3 host contract tests, and synthetic bounded-download check pass. AndroidTest APK remains NOT_COMPILED locally due to Google Maven network failure. G3 hosted diagnostic run `40510213740` failed before Gradle project configuration because the wrapper ran from the repository root; no APK build, Emulator, model download, or inference test occurred.
**BASE_SHA:** `3924b36080ffa43331f065a15db80c604c243355` (PR #123 head/base ref for this stacked G3 Draft PR)
**LAST_GOOD_SOURCE_HEAD:** `f67f29e83c76f8d5392316b66060b2d9a3d73d5b` (source/workflow before the standalone Gradle working-directory correction; prior G1 acceptance applies to the PoC source, and the current correction is isolated to its diagnostic runner/workflow)
**LAST_GOOD_SOURCE_TREE:** `5b5885d7a2e21568beba7801fd6580a3f6ef3424`
**WORKTREE:** owned isolated G3 worktree (repo-relative paths in this report)
**BRANCH:** `codex/serea-g3-real-ai-feasibility`
**DRAFT_PR:** [#126](https://github.com/Yrika819/Serea/pull/126) — Open/Draft; base ref `codex/serea-personal-beta-functional-integration-identity` at `3924b36080ffa43331f065a15db80c604c243355`.
**CI_RUN_IDS:** refreshed 2026-10-27 13:15 UTC; PR checks are on implementation HEAD `4e5ecddcb7a333d1732580fe546ee634b274138c`: Fast CI `40503535403` PASS; Android umbrella `40503535345` SUCCESS with only scope/documentation checks executed and product build/unit/integration/dependency/security/emulator/storage/identity jobs skipped (**Android Required NOT_RUN**, not PASS); RustSec `40503535332` PASS; Git Hooks Windows `40503535196` PASS; SQLite portability `40503535264` PASS; Full CI `40503535352` PASS; Native platform validation `40503535228` PASS. G3 run `40510213740` FAILED in the build step before Gradle configured a project (WORKFLOW/HARNESS working-directory error); emulator step skipped. Earlier G3 push run `40503475635` was SKIPPED (no marker, zero steps); main-only Beta workflow `40503473453` completed with zero jobs on this non-main ref.
**ARTIFACT_IDS:** SQLite portability `12733618204` (`portable-db-arm64`) and `12734452180` (`portable-db-intel`); RustSec `12733927790` (security report); G3 failed-run evidence `12736014521`, 6,609 bytes, SHA-256 `d5d05198156a34c6df9901dba6117f3a721910946c8dc26f190e4d882e8f7ea9`.

### G3 diagnostic run 40510213740 — failure classification

- **Classification:** WORKFLOW/HARNESS. The pinned `gradlew` executable was called by absolute path while the process remained at the repository root. That directory has no Gradle settings file; the standalone project root is `tools/g3-ondevice-poc/`.
- **Evidence:** artifact `12736014521` contains `gradle-build.log`, which reports that `/home/runner/work/Serea/Serea` does not contain a Gradle build; `model-status.txt` records exactly `model_downloaded=false`. KVM/Emulator acceleration and the synthetic bounded-transfer preflight passed. APK build did not begin, the Emulator step was skipped, and there is no JUnit report or instrumentation result.
- **Correction in progress:** both build and instrumentation Gradle calls now go through `run_poc_gradle`, which changes directory to the standalone project before invoking `./gradlew`. `test-gradle-project-dir.py` statically verifies the helper and both call sites. A rerun is pending this correction; no success is inferred from the failed run.

### INDEPENDENT_REVIEW

- G1 review status: **ACCEPTED** for the isolated G3 source, test, workflow, and evidence scope.
- Accepted contract: the JNI resources use a documented process-wide single-engine lease; second-owner load is rejected; cancellation identities include engine and request IDs; only the owner can free/cancel; close serializes with queued work; tests cover cross-engine isolation, reuse after close, and queued-load-versus-close.
- Review limitation: AndroidTest did not compile in this environment, so those assertions are source-reviewed only. The queued-load test does not separately exercise a load that has already entered native model loading before close.
- Preservation action: branch and diagnostic Draft PR #126 are committed/pushed. No model download, Emulator execution, real provider, or product integration was part of this preservation step.

### COMPLETED_WORK

- Installed/used isolated toolchains: Temurin 17.0.21+9 (archive SHA-256 `8b9b611d917e3dfd8419d3566b5949226f69b052fed7e109c1368add1dfe8711`), NDK 28.2.13676358, CMake 3.31.6, Ninja 1.12.1, Gradle wrapper 8.14.6, AGP 8.12.3, Kotlin 2.4.21.
- Built llama.cpp source pinned at tag `b11927`, commit `276378515ceebee5d14bcc8536603dd5c87491a2`, using `ANDROID_PLATFORM=android-26`, `ANDROID_ABI=arm64-v8a`, Release, static libraries, no OpenMP/dynamic backends/KleidiAI/native CPU tuning. `libllama.a` SHA-256: `19f2c604e11333ca61a07655595a560c67c5be45fae0bce217b6087f4de2be8f`.
- Added standalone project `tools/g3-ondevice-poc/`, outside the normal Android Gradle settings. It contains a test-local suspend `ModelChatPort`-shaped adapter, JNI wrapper, native CPU generation path, manifest without network permission, host contract/hash tests, and instrumentation tests. It does not touch `MainActivity`, `SereaAppShell`, product composition, normal Chat routing, or provider settings.
- Final corrected Kotlin/JNI source builds the standalone debug app APK for `arm64-v8a` and `x86_64`; the exact APK SHA-256 and native library hashes are recorded below.
- Selected a smaller fixed model candidate from official `ggml-org/SmolLM2-135M-GGUF` metadata: repo revision `1076b85d135017971845b19b8d07df1479155988`, Q4_K_M artifact size `101016128` bytes, Apache-2.0 metadata, SHA-256 `e3131339bf4e8065265593d4fd8f7bb7ff2d3abff1edb5618aa1197b89cad9f5`. The weight file was not downloaded. `tools/g3-ondevice-poc/model-fixture.json` records the immutable source/revision/hash.
- Host contract and model-hash tests verify multi-turn prompt serialization and that a mismatched digest is rejected before native loading. They use a fake engine and synthetic bytes, so they do not claim inference.
- The final native resource contract is an explicit process-wide singleton lease: one `NativeLlamaEngine` owns the process-global llama.cpp model/context/sampler; a second active owner is rejected, engine IDs namespace request IDs, and only the owner can cancel or free native state. Per-engine executors serialize load/generate/close, and queued loads recheck closure before native entry. Instrumentation deterministically covers second-engine rejection, same-numbered cross-engine cancel/free isolation, successful new-engine use after close, and a blocked queued load versus close. It also retains queued-before-JNI and in-flight cancellation checks. This AndroidTest source remains NOT_COMPILED because AndroidX dependency resolution failed before source compilation.
- Added a standalone **non-required** workflow and host runner at `.github/workflows/g3-ondevice-poc.yml` and `tools/g3-ondevice-poc/scripts/run-diagnostic.sh`. It is restricted to pushes on this owned branch whose head commit message contains `[g3-poc-run]`; the initial marker-free push `40503475635` was skipped. The workflow uses read-only repository permission, requires readable/writable `/dev/kvm` and successful `emulator -accel-check`, provisions disposable API 35 `google_apis/x86_64`, runs the local synthetic oversize-transfer check, builds both APKs before model acquisition, validates the exact fixture size/SHA before app-private staging, requests one exact instrumentation method and validates one non-skipped JUnit testcase, records resource/APK hash/log evidence, and removes partial/complete model copies in an exit trap. Run `40510213740` exposed that Gradle was launched from the repository root; the correction routes both build and instrumentation calls through the standalone project working directory. The regression check is `tools/g3-ondevice-poc/scripts/test-gradle-project-dir.py`. No Required workflow or gate is changed, and the artifact excludes model weights and APK binaries.
- The G3 diagnostic run `40510213740` **FAILED before project configuration** and is not a compile/test result. Its artifact `12736014521` records `model_downloaded=false`; no Emulator was started and no instrumentation/JUnit test ran. Static checks after the working-directory correction: regression check PASS; `bash -n` PASS; PyYAML workflow syntax/order PASS; `git diff --check` PASS. `actionlint` is unavailable. The 33-byte/32-byte synthetic transfer test in the failed run passed with curl status 63 and at most 32 partial bytes. Emulator and system-image package revisions are resolved by SDK package name rather than pinned; the workflow records actual installed package/emulator versions, so bit-for-bit reproducibility is not claimed. A single corrected diagnostic rerun is pending.

### CHANGED_FILES

- `tools/g3-ondevice-poc/` — standalone Gradle test app, JNI, instrumentation/unit tests, pinned fixture manifest, build notes.
- `.github/workflows/g3-ondevice-poc.yml` — non-required, commit-marker-gated diagnostic workflow proposal; NOT_RUN.
- `tools/g3-ondevice-poc/scripts/run-diagnostic.sh` — metadata/build/run/evidence/cleanup and bounded-transfer-test script; only static/synthetic checks run.
- `tools/g3-ondevice-poc/scripts/test-gradle-project-dir.py` — deterministic regression check that both Gradle call sites use the standalone project-root helper.
- `artifacts/g3-ondevice-poc/` — bounded Gradle build, AndroidX resolution failure, and host-test logs (no model weights or secrets).
- `docs/agent-reports/g3-ai.md` — this continuation checkpoint.

### TEST_COMMANDS / TEST_RESULTS

- **PASS (native cross-compile only):** with `LLAMA_CPP_SOURCE` pointing to the pinned llama.cpp checkout and `G3_BUILD_DIR` to an isolated build directory, run from the repository root:
  `cmake -S "$LLAMA_CPP_SOURCE" -B "$G3_BUILD_DIR" -DCMAKE_TOOLCHAIN_FILE="$ANDROID_SDK_ROOT/ndk/28.2.13676358/build/cmake/android.toolchain.cmake" -DANDROID_ABI=arm64-v8a -DANDROID_PLATFORM=android-26 -DCMAKE_BUILD_TYPE=Release -DBUILD_SHARED_LIBS=OFF -DLLAMA_BUILD_TESTS=OFF -DLLAMA_BUILD_EXAMPLES=OFF -DLLAMA_BUILD_SERVER=OFF -DLLAMA_BUILD_COMMON=OFF -DGGML_NATIVE=OFF -DGGML_OPENMP=OFF -DGGML_LLAMAFILE=OFF -DGGML_VULKAN=OFF -DGGML_BACKEND_DL=OFF` then `cmake --build "$G3_BUILD_DIR" --target llama -j2`.
- **PASS (final host JVM contract rerun):** from `tools/g3-ondevice-poc`, with Temurin JDK 17.0.21+9 on `PATH` and `G3_LLAMA_CPP_SOURCE` pointing to the pinned source checkout, run `./gradlew --offline --no-daemon --console=plain -Pg3LlamaCppSource="$G3_LLAMA_CPP_SOURCE" --rerun-tasks :app:testDebugUnitTest`; JUnit XML reports **3 tests, 0 failures, 0 errors, 0 skipped**. Log: `artifacts/g3-ondevice-poc/gradle-unit-final-review.log`, SHA-256 `a76a67c75ea176d5d51dc17664f250dff0e317779485f31f77f3ba736fdd8195`. JUnit XML: `artifacts/g3-ondevice-poc/host-junit.xml`, SHA-256 `7097139bbe1cd210436ddc209e16a64ce6e4da5800c1ddc7c4dfb1b60f3183f7`.
- **PASS (final corrected Android JNI/app build):** from `tools/g3-ondevice-poc`, with Temurin JDK 17.0.21+9 on `PATH` and the same `G3_LLAMA_CPP_SOURCE`, run `./gradlew --offline --no-daemon --console=plain -Pg3LlamaCppSource="$G3_LLAMA_CPP_SOURCE" :app:assembleDebug`; both ARM64 and x86_64 JNI builds and APK assembly passed. `app-debug.apk` is 14,557,726 bytes, SHA-256 `4115f121e329bc765ebb3b92242ac6789d9f0647e1e673efba65fb3a9506f4c3`. Final log: `artifacts/g3-ondevice-poc/gradle-assemble-final-review.log`, SHA-256 `587c9122f0039e497ab61c503625ba2676992e6c7f7df044de17adc347d384e9`.
- **PASS (bounded-download synthetic test, no external fetch):** from the repository root, with `G3_EVIDENCE_DIR` pointing to `artifacts/g3-ondevice-poc` and `RUNNER_TEMP` set to a disposable temporary directory, run `bash tools/g3-ondevice-poc/scripts/run-diagnostic.sh test-download-bound`; the local 33-byte payload was rejected by `curl --max-filesize 32` with expected status 63, with at most 32 partial bytes and no model content. Final review log `artifacts/g3-ondevice-poc/download-bound-final-review.log`, SHA-256 `a09990283dfdbda853613ef3d030583179a9d37cb44de96d4605d01a0f2b4aa1`.
- **BLOCKED (final AndroidTest APK compile attempt):** from `tools/g3-ondevice-poc`, with Temurin JDK 17.0.21+9 and `G3_LLAMA_CPP_SOURCE` configured, `./gradlew --no-daemon --console=plain -Pg3LlamaCppSource="$G3_LLAMA_CPP_SOURCE" :app:assembleDebugAndroidTest` failed at `:app:checkDebugAndroidTestAarMetadata` before AndroidTest Kotlin compilation. Google Maven returned `Network is unreachable`; `androidx.annotation:annotation:1.7.0-beta01` and other AndroidX test artifacts could not resolve. Therefore the new instrumentation source, including singleton and load/close race tests, is **NOT_COMPILED**, and no instrumentation method ran. Log: `artifacts/g3-ondevice-poc/gradle-androidtest-final-review.log`, SHA-256 `462971e670cdbcb185faf4e793715d1e22887820cab6f455347c3bc31e24e677`; earlier attempts are preserved in `gradle-build-failure.log` and `gradle-androidtest-network.log`. No successful Test APK exists.
- **PASS (static checks):** `bash -n tools/g3-ondevice-poc/scripts/run-diagnostic.sh`, PyYAML workflow parse, `git diff --check`, and source whitespace scan; `actionlint` unavailable.
- **PASS (standalone project root regression):** `python3 tools/g3-ondevice-poc/scripts/test-gradle-project-dir.py` verifies `prepare_build` and `run_model_test` both call the helper that changes to `POC_DIR` before executing `./gradlew`.
- **NOT_RUN (corrected hosted diagnostic):** run `40510213740` failed at Gradle root discovery before either APK task started. The model acquisition/Emulator/test steps were skipped, and `model-status.txt` is `model_downloaded=false`. A corrected marker run is pending; this failure is not AndroidTest compile status.
- **GitHub validation status (snapshot above):** Android umbrella success is scope-classifier/documentation validation only. Required product build, instrumentation compile, unit/integration/security, emulator/storage, and identity jobs were skipped, so Android Required is **NOT_RUN**, not Required PASS. Fast, Full, Native, RustSec, SQLite, and Git Hooks checks completed PASS on the implementation HEAD listed above.
- G3 marker-gated run `40503475635` was skipped with no steps or artifacts; no G3 diagnostic workflow job executed. The non-main personal-beta run `40503473453` had zero jobs, as expected for the main-only workflow; it is not a signing/distribution result.
- `aapt dump permissions` on the final APK reports only `package: app.serea.g3poc` and no permissions; target app manifest requests no INTERNET permission.
- Local Emulator availability: `/dev/kvm` absent, `emulator -accel-check` reports that CPU lacks VMX/SVM; `adb devices -l` empty. No Emulator was launched.
- **NOT_RUN:** model download/hash of model bytes, native model load, generation, Android instrumentation, cancellation/recovery, corrupt GGUF parse on-device, offline socket observation, emulator inference, Pixel 7a memory/latency/thermal/battery. A metadata hash is not a measurement of a downloaded weight artifact.

### Native output evidence

| ABI | Output | Size | SHA-256 | Evidence |
|---|---|---:|---|---|
| arm64-v8a | final unstripped `libserea-g3-llama.so` | 59,005,016 bytes | `beaac45f37ba3e7f9cb475879ece2cfe64ab6631418ee760bb00366cc0215f3d` | Android NDK 28.2.13676358; API 26 target; final engine-lease JNI |
| arm64-v8a | final packaged/stripped `libserea-g3-llama.so` | 6,308,144 bytes | `6e0a6fa041f69dcede352853446fc10b0d91891c1ee6a95223c6ed68e66f6309` | APK build output |
| x86_64 | final unstripped `libserea-g3-llama.so` | 57,520,360 bytes | `1a53ce1b918d0e726f61e4d5880ae579e4f928964eb2b61f13d5974ebb1688b7` | Android x86_64 Emulator ABI build |
| x86_64 | final packaged/stripped `libserea-g3-llama.so` | 6,787,224 bytes | `353a969f431af16a9d1414d587b3e4a393838074ad1c8c438c274f4cb811a6f7` | APK build output |

### SECURITY_IMPACT / COMPATIBILITY_IMPACT

- Product/provider behavior and normal Chat remain unchanged. The standalone manifest has no permissions, no exported components, and no Internet permission. The only model input is the synthetic Apache-licensed fixture described above, once a parent-approved Emulator run exists.
- The local adapter is structurally compatible with the `suspend send(request): response` shape, not linked to Serea's production interface. No local-model egress target or product catalog was added. It must not be integrated as `LOCAL_DEMO`.
- ARM64 API26 native compilation does not prove model load/runtime compatibility; the ABI APK can include either architecture, and x86_64 Emulator inference would not prove ARM64/Pixel 7a performance.

### CURRENT_BLOCKERS / NEXT_ACTION

- AndroidTest APK compilation is **NOT_COMPILED**: the build fails before Kotlin test-source compilation because Google Maven is unreachable (`Network is unreachable`) while resolving AndroidX test/plugin artifacts. All instrumentation assertions are therefore **NOT_RUN**.
- The Google Maven failure is observed only in this local environment. Hosted reproducibility is **UNKNOWN / NOT_RUN** because Android run `40503535345` classified this tool-only PR and skipped the build/instrumentation-compile job; hosted dependency resolution never ran. No dependency bypass or test skip was added.
- Model weights remain undownloaded. No Emulator or Pixel 7a was used. Native model load, single/multi-turn generation, cancellation, release/recovery, corrupt GGUF parsing, offline socket verification, API 26 runtime, and Pixel 7a memory/latency/thermal/battery are **NOT_RUN**.
- G1 accepted the source/workflow scope; its review notes the queued-load-vs-close test does not separately exercise a load already inside native model loading. No model/Emulator workflow is permitted until AndroidTest compiles and the separately coordinated G4 API35 Required campaign releases the KVM Emulator slot.
- Real cloud AI/provider integration remains disabled. Provider-specific terms are documented below; Owner account, audience age, launch region, budget, credential custody, and real provider authorization remain unverified/Owner-blocked.
- Next: use the corrected standalone project invocation in a single marker-gated disposable API 35 run after confirming G4 has no active Emulator campaign. AndroidTest APK compilation must succeed before any model acquisition; if it fails, preserve that failure and stop before the Emulator/model step. Do not use a live API/account or claim Pixel 7a performance from Emulator data.

Retrieved official provider/runtime pages: **2026-10-27 UTC**. This is a readiness review, not a legal opinion or a provider activation. No live account, API key, paid request, Google account, or model weight was used.

## Initial readiness inventory (historical; superseded where the continuation above differs)

The remaining sections preserve the initial read-only policy and integration research at source base `3924b36080ffa43331f065a15db80c604c243355`. Their toolchain inventory, Qwen candidate, unbuilt-project and no-test statements describe that earlier point in time. They are not the current G3 implementation or verification status. Current evidence is in the continuation above: NDK/CMake/Ninja, JNI/APK build, and 3/3 host tests now exist; the SmolLM2 fixture is pinned but not downloaded; AndroidTest, native inference, Emulator and Pixel 7a remain NOT_RUN. Provider policy observations remain useful, with Owner-specific eligibility still unverified.

### Historical agent-report snapshot

**AGENT_ID:** G3 — Real AI / On-device Inference Feasibility<br>
**ACTIVE_PHASE:** G3 research / feasibility<br>
**OVERALL_STATUS:** PARTIAL — initial official-source review and integration assessment completed; at that historical snapshot no native build tooling or standalone inference harness existed, and a 639 MB Qwen fixture was intentionally not downloaded. This status is superseded by the continuation at the top of this report.<br>
**HISTORICAL_BASE_SHA:** `3924b36080ffa43331f065a15db80c604c243355`<br>
**HISTORICAL_HEAD:** `3924b36080ffa43331f065a15db80c604c243355` (source revision inspected at initial research snapshot)<br>
**HISTORICAL_TREE_SHA:** `41cfce316c485abe31f86c827c6ca81a4d5c107b` (source tree inspected at initial research snapshot)<br>
**WORKTREE:** owned isolated G3 worktree (historical snapshot; host path omitted)<br>
**BRANCH:** `codex/serea-g3-real-ai-feasibility`<br>
**HISTORICAL_DRAFT_PR:** None at initial research snapshot<br>

#### Historical COMPLETED_WORK

- Inspected existing Chat contracts and composition on PR #123. `ModelChatPort.send(ModelChatRequest)` is suspend-based and returns a complete `ModelChatResponse`; `MockModelChatPort` is explicitly deterministic demo behavior.
- Existing normal Chat composition is local demo only: `RoutedModelChatPort → EgressCheckedModelChatPort (LOCAL_DEMO) → UsageControlledModelChatPort → LocalDemoModelChatPort → MockModelChatPort`.
- Existing cloud route has a fixed Ollama endpoint and requires a fresh, verified model catalog entry, exact per-send egress approval, a user-authored PUBLIC payload, credentials, and enabled settings. `CloudSettings.enabled` defaults to `false`. No provider call was made.
- Inspected the existing accepted ADR and AI activation plan. They already say that no real provider is enabled, and list Owner decisions required before activation.
- Compared the existing Ollama Cloud candidate, OpenAI API, Gemini API, ML Kit GenAI/AICore, and an isolated `llama.cpp` + official Qwen GGUF route against current official terms, age/region conditions, data handling, pricing model, hardware support, and code integration constraints.
- Determined why a real local inference PoC could not be validated in this worktree: the then-configured Android SDK had no NDK package; `cmake` and `ninja` were not installed/in PATH; no inference model was present. The official Qwen Q8 GGUF model file is **639,446,688 bytes**. It was not downloaded because this host lacked the native toolchain and a download alone cannot prove Android inference, cancellation, memory use, or Pixel 7a performance.
- No source code, dependency, build configuration, provider setting, or production composition was changed.

#### Historical CHANGED_FILES

- `docs/agent-reports/g3-ai.md` (this report only; no runtime or product changes).

#### Historical TEST_COMMANDS / TEST_RESULTS

- Source inspection: `rg -n 'ModelChatPort|CloudSettings|cloud.*enabled|RoutedModelChatPort|DemoChat' ...` and targeted `sed` reads — completed; findings above.
- Toolchain inventory: `command -v cmake`, `command -v ninja`, and inspection of the then-configured Android SDK's NDK packages — at that historical snapshot no CMake, Ninja, or NDK was available.
- Device inventory: `adb devices -l` — daemon started; no devices attached.
- Official model metadata: queried Hugging Face model metadata API for `Qwen/Qwen3-0.6B-GGUF` — `license=apache-2.0`; sole `.gguf` entry observed `Qwen3-0.6B-Q8_0.gguf`, size `639446688` bytes.
- No build or tests were run: no PoC implementation was added. There is no inference result, emulator result, ARM64 execution result, or Pixel 7a benchmark to report.

**HISTORICAL_CI_RUN_IDS:** None at initial research snapshot<br>
**HISTORICAL_ARTIFACT_IDS:** None at initial research snapshot

### SECURITY_IMPACT

- No credential, permission, manifest, network allowlist, egress policy, usage accounting, provider activation, model artifact, or app runtime behavior changed.
- A future cloud adapter must remain behind `RoutedModelChatPort → EgressCheckedModelChatPort → UsageControlledModelChatPort → provider`; preserve egress evaluation before reservation/dispatch. Google-origin email/calendar content must stay blocked from cloud egress absent a separately approved, explicit path.
- A future on-device model must have a distinct typed local target/model identity. The current security contract only defines `EgressTarget.LOCAL_DEMO` and binds it to `serea-demo/local-chat`; do not reuse that demo identity to disguise a real model. Add a separately reviewed local-model destination and provenance gate before integration.
- Native runtime and model weights require independent supply-chain review: pin an immutable upstream revision, audit bundled/transitive licenses, generate SBOM, verify downloaded model SHA-256 at load, reject untrusted model files, and keep weights out of ordinary app/diagnostic logs.

### COMPATIBILITY_IMPACT

- Product compatibility is unchanged; only a report was added.
- Existing `ModelChatPort` is a plausible adapter seam, but its return contract is whole-response (not token streaming). The adapter can still suspend and propagate cancellation; true streaming requires a separately reviewed contract change and UI work.
- `llama.cpp` documents an Android NDK `arm64-v8a` build using `ANDROID_PLATFORM=android-28`; Serea supports `minSdk=26`. API 26 runtime compatibility has not been established and requires an actual API 26 build/run test.
- ML Kit Prompt API documentation says Android API 26+, but current supported-device lists exclude Pixel 7a.

#### Historical AI route comparison (local Qwen candidate superseded by SmolLM2 fixture above)

| Route | Eligibility / terms | Execution / device and memory | License / model provenance | Integration / security | Pixel 7a unknowns | Decision |
|---|---|---|---|---|---|---|
| **Ollama Cloud** (existing fixed adapter) | Ollama Terms require a user of the Service to be 18+. Owner has not confirmed audience age, account eligibility, or release/user region; do not infer adults-only from “Personal Beta.” | Cloud inference; model memory is provider-side. Official privacy/pricing says hosting is primarily US and may route to Europe. Pay-per-token model pricing and plans can change. App/device inference latency is not measured. | Hosted model weights are not shipped by Serea. Exact model ID and the current hosted model provenance/licensing still require activation-time review; the frozen ADR roster may be stale relative to current catalog/prices. | Adapter exists with default disabled and exact egress gate. Cloud prompt/response processing is described as transient and not trained on; cost is token-based. Cross-border egress and provider account custody remain explicit risks. No network request occurred. | No device-local performance/memory claim. Pixel 7a network quality, end-to-end latency, battery, or data costs are untested. | **Conditional candidate only.** Do not enable pending audience/age and region determination, current model/pricing review, data-flow approval, account ownership, and budget. |
| **OpenAI API** (direct) | Services Agreement disallows minors using service without parent/guardian consent. Under-18 API guidance requires reasonable age-appropriate safety for organizations serving minors and says not to process personal data of children below 13 (or applicable digital-consent age) without approved Zero Data Retention. Access countries are maintained in an official list; Owner account and user/deployment countries are unverified. | Cloud inference; model memory is provider-side. Token-priced per model; rate limits/pricing change and must be rechecked before approval. No API call made, so no measured latency. | Hosted model weights are proprietary/provider-hosted; no weight redistribution. Select an exact currently supported model only after official source, data, and price review. | No OpenAI adapter exists in PR #123. API data is not used to train by default, but abuse logs may include content and default retention is up to 30 days; ZDR/MAM require prior approval and have limitations. Never embed a direct API key in the APK; key custody/backend requires a separate decision. Preserve Serea egress/usage ordering. | Network latency, availability, battery, carrier cost, and any API region/data-residency behavior from Pixel 7a are untested. | **Potential candidate only** after audience, region, safety, retention, model, billing, and credential-custody decisions. Not connected/authorized. |
| **Gemini API / AI Studio** | Additional Terms (effective 2026-03-23): developer must be 18+, API clients must not be directed to or likely accessed by under-18s, and service is for professional/business development, not consumer use. Owner account country and audience remain unverified. | Cloud inference; model memory is provider-side. Only listed regions allowed; EEA/UK/Switzerland clients must use paid services. On unpaid service, prompts/responses may be used to improve products and human reviewed; do not send sensitive/confidential/personal data. Paid access costs and requires explicit approval. | Hosted weights cannot be extracted or redistributed under the service terms. Exact model is not selected. | No adapter wired. Current no-unapproved-egress rule blocks usage. Google free-tier data use is incompatible with sending personal assistant content; paid terms do not itself authorize Serea's egress. | Pixel 7a bandwidth, latency, battery and supported-country service access not tested. | **Not suitable for this consumer Personal Beta under current terms.** No workaround/activation. |
| **ML Kit GenAI / Gemini Nano (AICore)** | ML Kit GenAI Additional Terms require developer age 18+ and prohibit apps directed to or likely accessed by under-18s. Prompt API is marked Beta; terms prohibit Preview/Experimental use in production. | On-device inference, with AICore managing model distribution and execution; memory/limits are model/device managed and no Pixel 7a budget is published here. Some feature-specific APIs only perform narrow tasks; Prompt API is Beta. | Google-managed model weights; no Serea model artifact or weight distribution. AICore/ML Kit terms and attribution/data disclosure obligations apply. | SDK reports input/output are processed on-device, but it may send API performance/utilization metrics to Google; user disclosure may be required. Uses AICore and has per-app quotas / foreground-use restrictions in official docs. | Pixel 7a is **not** on the current official Prompt or feature-specific supported-device lists. No runtime `checkStatus`, AICore availability, memory, latency, or API 26/35 test occurred. | **Not a valid Pixel 7a target-device route today.** Support, production status, and age conditions remain disqualifying/unresolved. |
| **Historical `llama.cpp` + Qwen3-0.6B GGUF candidate (superseded)** | The earlier review found no service-account age/region gate for local inference; model terms still apply. The runtime is MIT and Qwen's model card reports Apache-2.0. | Earlier candidate was the 639,446,688-byte Q8 file. It was not downloaded or measured. | Historical metadata only; no file hash/runtime audit. Replaced as the PoC fixture by the smaller pinned SmolLM2 candidate described above. | Product integration remains absent. A future local target needs a distinct reviewed egress/model identity, provenance, integrity verification, and cancellation/memory review. | Pixel 7a performance remains unmeasured. | **Superseded candidate; not the current PoC fixture.** |

### Official source register (retrieved 2026-10-27 UTC)

Provider policies are facts about provider conditions, not proof that the Owner’s own age, account, billing profile, or deployment/user region qualifies. Those Owner-specific facts remain **unverified**.

- Ollama Terms of Service (updated May 2026; age 18+): <https://ollama.com/terms>
- Ollama Privacy Policy (updated March 2026; local vs cloud handling): <https://ollama.com/privacy>
- Ollama pricing/model rates and hosting location: <https://ollama.com/pricing>
- OpenAI Services Agreement (minor users require parent/guardian consent): <https://openai.com/policies/services-agreement/>
- OpenAI API under-18 guidance: <https://developers.openai.com/api/docs/guides/safety-checks/under-18-api-guidance>
- OpenAI API data controls (training, abuse retention, ZDR eligibility): <https://developers.openai.com/api/docs/guides/your-data>
- OpenAI API supported countries: <https://help.openai.com/en/articles/5347006-openai-api-supported-countries-and-territories>
- OpenAI model pricing (dynamic model-specific rates): <https://developers.openai.com/api/docs/pricing>
- Gemini API Additional Terms (effective March 23, 2026; age, audience, business/consumer, region, free/paid data terms): <https://ai.google.dev/gemini-api/terms>
- Gemini API available regions: <https://ai.google.dev/gemini-api/docs/available-regions>
- Android Gemini Nano / AICore overview: <https://developer.android.com/ai/gemini-nano>
- ML Kit GenAI API overview/device support; docs last updated October 23, 2026: <https://developers.google.com/ml-kit/genai>
- ML Kit GenAI Additional Terms (age, preview/production, metrics disclosure): <https://developers.google.com/ml-kit/genai-terms>
- ML Kit Prompt API setup and API 26+ requirement: <https://developers.google.com/ml-kit/genai/prompt/android/get-started>
- `llama.cpp` Android arm64 NDK build documentation: <https://github.com/ggml-org/llama.cpp/blob/master/docs/build.md#arm-kleidiaitm>
- `llama.cpp` license: <https://github.com/ggml-org/llama.cpp/blob/master/LICENSE>
- Official Qwen3-0.6B GGUF model card/license: <https://huggingface.co/Qwen/Qwen3-0.6B-GGUF>

### Historical KNOWN_ISSUES (initial inventory only; see current status above)

- No approved/connected real AI provider exists in the inspected app composition. The ordinary Chat screen remains a deterministic local demo, not real AI.
- Cloud routes remain Owner-blocked for eligibility, age/audience, location, price/budget, credential custody, and data-transfer decisions. Provider-level terms have been documented separately from Owner facts.
- Gemini Nano/AICore does not list Pixel 7a as supported in the current official device list. The owner’s physical Pixel 7a is unavailable; no device query or performance test occurred.
- At the time of this initial inventory there was no native build/harness. That statement is superseded: an isolated SmolLM2/llama.cpp harness and ARM64/x86_64 app APK build now exist. Actual model load/generation, cancellation, recovery, corrupt-model parsing, offline behavior, Emulator inference, and Pixel 7a performance remain **NOT_RUN** as reported above.
- The current local egress identity is explicitly demo-only. A real local inference route needs a new separately reviewed local target/catalog/provenance contract and must not be made to appear as cloud AI or demo output.
- The PoC JNI model/context/sampler remain process-global behind the documented single-owner lease; support for parallel model instances is intentionally not provided. Instrumentation has not yet compiled, so cross-engine and load/close tests are source-level expectations, not verified passing results. PSS before/load/after-generation/after-close is logged as an Emulator diagnostic metric only; it is not asserted as proof of memory reclamation.
- Pinned library revision, transitive license inventory, SBOM, model SHA verification, model download/update plan, resource constraints, and safety evaluation remain open.

### Historical CURRENT_BLOCKERS (initial toolchain/model inventory; superseded for build/harness status)

- At the time of the initial inventory, NDK/CMake/Ninja were absent, and the Qwen Q8 model candidate was 639,446,688 bytes. This toolchain/candidate state is historical; the current pinned NDK build and SmolLM2 fixture are documented above. Model bytes remain intentionally undownloaded.
- No Android ARM64 device was attached in the initial inventory; the Pixel 7a remains unavailable for this campaign.
- Owner facts: intended user age/audience, provider account eligibility, launch regions, usage budget, model allowlist, and acceptance of each provider’s data terms are unknown. These do not block isolated Fake/offline development but block real service enablement.

### INTEGRATION_DEPENDENCIES

- G4 must not replace ordinary Chat with any real route on the basis of this research.
- A cloud integration requires Owner approval of a specific provider/model, intended user age and supported countries, data terms/retention, cost ceiling, and operator credential custody. It must retain Egress → Usage reservation → dispatch ordering and keep Google-origin data blocked by default.
- A local model integration requires a dedicated `LOCAL_ON_DEVICE` (or equally explicit) contract separate from `LOCAL_DEMO`, immutable runtime/model provenance, SHA verification, asset/update policy, Android API 26 compatibility evidence, and a model-sized device benchmark. The new project is test-only source; it is not an inference result or product integration.
- No modifications are requested to `MainActivity.kt`, `SereaAppShell.kt`, or composition root.

### Historical REVIEW_REQUEST (initial policy research review)

Please review the provider eligibility distinctions, especially the split between provider-level age/region rules and Owner-unverified account/audience facts. Confirm that the local route remains explicitly feasibility-only and that G4 does not treat this report as a PoC or real AI E2E.

### Historical recommended next engineering path (superseded by continuation above)

1. Continue product-independent Fake/offline Chat and test work; keep `CloudSettings.enabled=false` and do not bind a real API key/provider.
2. The initial recommendation to provision NDK/CMake/Ninja and a local PoC has been completed for the isolated SmolLM2 fixture harness; model acquisition and runtime are still blocked pending AndroidTest compilation and a coordinated KVM Emulator run.
3. The Qwen-specific fixture proposal in this initial plan is obsolete. The current fixture is the pinned 101,016,128-byte Apache-2.0 SmolLM2 Q4_K_M candidate above; its model bytes remain undownloaded.
4. The test-only adapter/JNI and host contract have been implemented. Instrumentation compile/runtime, real model load/generation, cancellation, native release, corrupt-model parsing, and offline behavior remain NOT_COMPILED/NOT_RUN as stated in the current checkpoint.
5. Collect separate ARM64-emulator and Pixel 7a measurements for peak RAM, startup, tokens/sec, thermal behavior, storage, cancellation latency, and process-death recovery. Only Pixel 7a data can answer Pixel 7a performance.
6. Independently audit the pinned runtime/native dependency licenses/SBOM and add a distinct reviewed `LOCAL_ON_DEVICE` egress/catalog identity before any product-composition work.
7. Revisit Cloud candidates only after Owner records exact target audience age, launch regions, account qualification, model allowlist, data terms/retention, spend cap, credential custody, and any guardian/safety requirements. Provider-term eligibility does not establish the Owner's account or users' eligibility.

### Historical no-test / NOT_RUN ledger (initial snapshot only; current status is above)

These are research observations, not passing tests:

| Check | State |
|---|---|
| Source/contract inspection | DONE (read-only) |
| Official policy/runtime/model metadata review | DONE (read-only, sources above) |
| Project build / compile | NOT_RUN at the initial `3924b360...` inventory; superseded by current PASS for the standalone Android app/JNI APK and host JVM contract tests |
| Model download / hash check | NOT_RUN (intentionally not downloaded) |
| Runtime/model load | NOT_RUN |
| Single-turn generation | NOT_RUN |
| Multi-turn generation | NOT_RUN |
| Cancellation / native context release | NOT_RUN |
| Corrupt-model rejection / recovery | NOT_RUN |
| Offline/no-socket verification | NOT_RUN |
| Android Emulator inference (any API/ABI) | NOT_RUN |
| Android ARM64 inference | NOT_RUN |
| Pixel 7a memory/latency/battery | NOT_RUN |
| Cloud provider connectivity / eligibility | NOT_RUN; provider eligibility conditions are documented, Owner account/audience/region are unverified |

### Historical NEXT_ACTION (replaced by current blockers and next action above)

The initial Qwen/no-toolchain action plan is superseded by the current continuation. Current action is to restore AndroidX dependency access, compile the instrumentation APK, and only then coordinate one disposable API35 KVM Emulator diagnostic; keep model weights, real providers, and Pixel 7a usage blocked until those conditions and Owner approvals are met.
