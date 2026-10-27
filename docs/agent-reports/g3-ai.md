# G3 — Real AI / On-device Inference Feasibility

## Continuation update — isolated Android JNI PoC (2026-10-27)

The initial readiness report below predates toolchain provisioning and the isolated PoC. Its no-toolchain/Qwen-candidate statements are superseded by this section; provider-terms findings remain applicable. No product source or composition changed.

**AGENT_ID:** G3 — Real AI / On-device Inference Feasibility
**ACTIVE_PHASE:** isolated llama.cpp Android feasibility PoC
**OVERALL_STATUS:** PARTIAL — isolated adapter/JNI code and ARM64/x86_64 app APK build pass; host contract tests are 3/3. AndroidTest APK dependency resolution is blocked locally by unavailable Google Maven artifacts, so the new instrumentation assertions are NOT_COMPILED/NOT_RUN. Model inference is NOT_RUN.
**BASE_SHA:** `6d6f362da2380b56ea0fb7bc9372dbcceae67e90`
**CURRENT_HEAD:** `6d6f362da2380b56ea0fb7bc9372dbcceae67e90` (PoC files are uncommitted pending Orchestrator scope review)
**TREE_SHA:** `f4fe1cf534b855b58ea19683621bf3dc7f7a3cd6` (last committed tree; worktree has the G3 PoC, workflow proposal, report, and small build logs)
**WORKTREE:** `/workspace/Serea-g3-ai`
**BRANCH:** `codex/serea-g3-real-ai-feasibility`
**DRAFT_PR:** none
**CI_RUN_IDS:** none for this G3 PoC; the diagnostic workflow has not been pushed or run.
**ARTIFACT_IDS:** none for this G3 PoC; local build logs are listed below.

### INDEPENDENT_REVIEW

- G1 review status: **ACCEPTED** for the isolated G3 source, test, workflow, and evidence scope.
- Accepted contract: the JNI resources use a documented process-wide single-engine lease; second-owner load is rejected; cancellation identities include engine and request IDs; only the owner can free/cancel; close serializes with queued work; tests cover cross-engine isolation, reuse after close, and queued-load-versus-close.
- Review limitation: AndroidTest did not compile in this environment, so those assertions are source-reviewed only. The queued-load test does not separately exercise a load that has already entered native model loading before close.
- Preservation action: this owned branch is being committed and pushed for a diagnostic Draft PR. No model download, Emulator, real provider, or product integration is part of this preservation step.

### COMPLETED_WORK

- Installed/used isolated toolchains: Temurin 17.0.21+9 (archive SHA-256 `8b9b611d917e3dfd8419d3566b5949226f69b052fed7e109c1368add1dfe8711`), NDK 28.2.13676358, CMake 3.31.6, Ninja 1.12.1, Gradle wrapper 8.14.6, AGP 8.12.3, Kotlin 2.4.21.
- Built llama.cpp source pinned at tag `b11927`, commit `276378515ceebee5d14bcc8536603dd5c87491a2`, using `ANDROID_PLATFORM=android-26`, `ANDROID_ABI=arm64-v8a`, Release, static libraries, no OpenMP/dynamic backends/KleidiAI/native CPU tuning. `libllama.a` SHA-256: `19f2c604e11333ca61a07655595a560c67c5be45fae0bce217b6087f4de2be8f`.
- Added standalone project `tools/g3-ondevice-poc/`, outside the normal Android Gradle settings. It contains a test-local suspend `ModelChatPort`-shaped adapter, JNI wrapper, native CPU generation path, manifest without network permission, host contract/hash tests, and instrumentation tests. It does not touch `MainActivity`, `SereaAppShell`, product composition, normal Chat routing, or provider settings.
- Final corrected Kotlin/JNI source builds the standalone debug app APK for `arm64-v8a` and `x86_64`; the exact APK SHA-256 and native library hashes are recorded below.
- Selected a smaller fixed model candidate from official `ggml-org/SmolLM2-135M-GGUF` metadata: repo revision `1076b85d135017971845b19b8d07df1479155988`, Q4_K_M artifact size `101016128` bytes, Apache-2.0 metadata, SHA-256 `e3131339bf4e8065265593d4fd8f7bb7ff2d3abff1edb5618aa1197b89cad9f5`. The weight file was not downloaded. `tools/g3-ondevice-poc/model-fixture.json` records the immutable source/revision/hash.
- Host contract and model-hash tests verify multi-turn prompt serialization and that a mismatched digest is rejected before native loading. They use a fake engine and synthetic bytes, so they do not claim inference.
- The final native resource contract is an explicit process-wide singleton lease: one `NativeLlamaEngine` owns the process-global llama.cpp model/context/sampler; a second active owner is rejected, engine IDs namespace request IDs, and only the owner can cancel or free native state. Per-engine executors serialize load/generate/close, and queued loads recheck closure before native entry. Instrumentation deterministically covers second-engine rejection, same-numbered cross-engine cancel/free isolation, successful new-engine use after close, and a blocked queued load versus close. It also retains queued-before-JNI and in-flight cancellation checks. This AndroidTest source remains NOT_COMPILED because AndroidX dependency resolution failed before source compilation.
- Added a standalone **non-required** workflow proposal and host runner at `.github/workflows/g3-ondevice-poc.yml` and `tools/g3-ondevice-poc/scripts/run-diagnostic.sh`. It is restricted to pushes on this owned branch whose head commit message contains `[g3-poc-run]`; adding/updating the workflow alone cannot fetch a model or start an Emulator. The proposed job pins existing Actions, uses read-only repository permission, requires readable/writable `/dev/kvm` and successful `emulator -accel-check`, provisions disposable API 35 `google_apis/x86_64`, runs the local synthetic oversize-transfer check, builds both APKs before model acquisition, validates the exact 101,016,128-byte model SHA before app-private staging, requests one exact instrumentation method and validates one non-skipped JUnit testcase, records resource/APK hash/log evidence, and removes possible partial and complete model copies in an exit trap. It does not change any Required workflow or gate, and does not upload model weights or APK binaries.
- Workflow/script are **proposed for Orchestrator review only**. The runner checks exactly one JUnit case with both expected class and method names, and requires zero skipped/failure/error counts. Static checks: `bash -n` PASS; PyYAML workflow syntax parse PASS; `git diff --check` PASS. `actionlint` is unavailable. The synthetic transfer check using a local 33-byte payload and 32-byte cap PASSed with curl status 63 and at most 32 partial bytes. Emulator and system-image package revisions are resolved by SDK package name rather than pinned; the workflow records the actual installed package and emulator versions, so bit-for-bit environment reproducibility is not claimed. No workflow, model download, or Emulator execution has occurred.

### CHANGED_FILES

- `tools/g3-ondevice-poc/` — standalone Gradle test app, JNI, instrumentation/unit tests, pinned fixture manifest, build notes.
- `.github/workflows/g3-ondevice-poc.yml` — non-required, commit-marker-gated diagnostic workflow proposal; NOT_RUN.
- `tools/g3-ondevice-poc/scripts/run-diagnostic.sh` — metadata/build/run/evidence/cleanup and bounded-transfer-test script; only static/synthetic checks run.
- `artifacts/g3-ondevice-poc/` — bounded Gradle build, AndroidX resolution failure, and host-test logs (no model weights or secrets).
- `docs/agent-reports/g3-ai.md` — this continuation checkpoint.

### TEST_COMMANDS / TEST_RESULTS

- **PASS (native cross-compile only):**
  `cmake -S /tmp/serea-g3-llama.cpp -B /tmp/serea-g3-llama-arm64-build -DCMAKE_TOOLCHAIN_FILE=/workspace/android-sdk/ndk/28.2.13676358/build/cmake/android.toolchain.cmake -DANDROID_ABI=arm64-v8a -DANDROID_PLATFORM=android-26 -DCMAKE_BUILD_TYPE=Release -DBUILD_SHARED_LIBS=OFF -DLLAMA_BUILD_TESTS=OFF -DLLAMA_BUILD_EXAMPLES=OFF -DLLAMA_BUILD_SERVER=OFF -DLLAMA_BUILD_COMMON=OFF -DGGML_NATIVE=OFF -DGGML_OPENMP=OFF -DGGML_LLAMAFILE=OFF -DGGML_VULKAN=OFF -DGGML_BACKEND_DL=OFF` then `cmake --build /tmp/serea-g3-llama-arm64-build --target llama -j2`.
- **PASS (final host JVM contract rerun):** `JAVA_HOME=/tmp/temurin17 PATH=/tmp/temurin17/bin:$PATH ./gradlew --offline --no-daemon --console=plain -Pg3LlamaCppSource=/tmp/serea-g3-llama.cpp --rerun-tasks :app:testDebugUnitTest`; JUnit XML reports **3 tests, 0 failures, 0 errors, 0 skipped**. Log: `artifacts/g3-ondevice-poc/gradle-unit-final-review.log`, SHA-256 `a76a67c75ea176d5d51dc17664f250dff0e317779485f31f77f3ba736fdd8195`. JUnit XML: `artifacts/g3-ondevice-poc/host-junit.xml`, SHA-256 `7097139bbe1cd210436ddc209e16a64ce6e4da5800c1ddc7c4dfb1b60f3183f7`.
- **PASS (final corrected Android JNI/app build):** `JAVA_HOME=/tmp/temurin17 PATH=/tmp/temurin17/bin:$PATH ./gradlew --offline --no-daemon --console=plain -Pg3LlamaCppSource=/tmp/serea-g3-llama.cpp :app:assembleDebug`; both ARM64 and x86_64 JNI builds and APK assembly passed. `app-debug.apk` is 14,557,726 bytes, SHA-256 `4115f121e329bc765ebb3b92242ac6789d9f0647e1e673efba65fb3a9506f4c3`. Final log: `artifacts/g3-ondevice-poc/gradle-assemble-final-review.log`, SHA-256 `587c9122f0039e497ab61c503625ba2676992e6c7f7df044de17adc347d384e9`.
- **PASS (bounded-download synthetic test, no external fetch):** `G3_EVIDENCE_DIR="$PWD/artifacts/g3-ondevice-poc" RUNNER_TEMP=/tmp bash tools/g3-ondevice-poc/scripts/run-diagnostic.sh test-download-bound`; local 33-byte payload was rejected by `curl --max-filesize 32` with expected status 63; partial file was 32 bytes, no model content. Final review log `artifacts/g3-ondevice-poc/download-bound-final-review.log`, SHA-256 `a09990283dfdbda853613ef3d030583179a9d37cb44de96d4605d01a0f2b4aa1`.
- **BLOCKED (final AndroidTest APK compile attempt):** `JAVA_HOME=/tmp/temurin17 PATH=/tmp/temurin17/bin:$PATH ./gradlew --no-daemon --console=plain -Pg3LlamaCppSource=/tmp/serea-g3-llama.cpp :app:assembleDebugAndroidTest` failed at `:app:checkDebugAndroidTestAarMetadata` before AndroidTest Kotlin compilation. Google Maven returned `Network is unreachable`; `androidx.annotation:annotation:1.7.0-beta01` and other AndroidX test artifacts could not resolve. Therefore the new instrumentation source, including singleton and load/close race tests, is **NOT_COMPILED**, and no instrumentation method ran. Log: `artifacts/g3-ondevice-poc/gradle-androidtest-final-review.log`, SHA-256 `462971e670cdbcb185faf4e793715d1e22887820cab6f455347c3bc31e24e677`; earlier attempts are preserved in `gradle-build-failure.log` and `gradle-androidtest-network.log`. No successful Test APK exists.
- **PASS (static proposal checks only):** `bash -n tools/g3-ondevice-poc/scripts/run-diagnostic.sh`, PyYAML workflow parse, and `git diff --check`; `actionlint` unavailable. No Actions workflow execution, model download, or Emulator execution.
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

- The final corrected JNI/app build and three host JVM tests pass. AndroidTest APK compilation is blocked before source compilation by unreachable Google Maven (`Network is unreachable`); the instrumentation tests are NOT_COMPILED/NOT_RUN. The model remains undownloaded, and no Emulator has run.
- G1 source review is accepted. AndroidTest APK compilation remains blocked by unreachable Google Maven, and API35 inference requires the separately coordinated KVM-backed Emulator slot after the active G4 API35 Required campaign. Do not download the model before that run is authorized and the test APK builds. Keep AndroidTest, model runtime, API26 inference, and Pixel 7a performance NOT_RUN until directly tested.
- No AI route is activated. Cloud eligibility facts for the Owner/audience remain unverified separately from provider terms; the earlier route comparison and official source register above remain applicable.

Retrieved official provider/runtime pages: **2026-10-27 UTC**. This is a readiness review, not a legal opinion or a provider activation. No live account, API key, paid request, Google account, or model weight was used.

## Initial readiness inventory (historical; superseded where the continuation above differs)

## Agent report contract

**AGENT_ID:** G3 — Real AI / On-device Inference Feasibility<br>
**ACTIVE_PHASE:** G3 research / feasibility<br>
**OVERALL_STATUS:** PARTIAL — official-source review and integration assessment completed; a valid inference PoC is blocked by missing native build tooling and an intentionally undownloaded 639 MB model artifact.<br>
**BASE_SHA:** `3924b36080ffa43331f065a15db80c604c243355`<br>
**CURRENT_HEAD:** `3924b36080ffa43331f065a15db80c604c243355` (source revision inspected; report-only commit follows)<br>
**TREE_SHA:** `41cfce316c485abe31f86c827c6ca81a4d5c107b` (source tree inspected)<br>
**WORKTREE:** `/workspace/Serea-g3-ai`<br>
**BRANCH:** `codex/serea-g3-real-ai-feasibility`<br>
**DRAFT_PR:** None<br>

### COMPLETED_WORK

- Inspected existing Chat contracts and composition on PR #123. `ModelChatPort.send(ModelChatRequest)` is suspend-based and returns a complete `ModelChatResponse`; `MockModelChatPort` is explicitly deterministic demo behavior.
- Existing normal Chat composition is local demo only: `RoutedModelChatPort → EgressCheckedModelChatPort (LOCAL_DEMO) → UsageControlledModelChatPort → LocalDemoModelChatPort → MockModelChatPort`.
- Existing cloud route has a fixed Ollama endpoint and requires a fresh, verified model catalog entry, exact per-send egress approval, a user-authored PUBLIC payload, credentials, and enabled settings. `CloudSettings.enabled` defaults to `false`. No provider call was made.
- Inspected the existing accepted ADR and AI activation plan. They already say that no real provider is enabled, and list Owner decisions required before activation.
- Compared the existing Ollama Cloud candidate, OpenAI API, Gemini API, ML Kit GenAI/AICore, and an isolated `llama.cpp` + official Qwen GGUF route against current official terms, age/region conditions, data handling, pricing model, hardware support, and code integration constraints.
- Determined why a real local inference PoC could not be validated in this worktree: `/workspace/android-sdk` has no Android NDK installation; `cmake` and `ninja` are not installed/in PATH; no inference model is present. The official Qwen Q8 GGUF model file is **639,446,688 bytes**. It was not downloaded because this host lacks the native toolchain and a download alone cannot prove Android inference, cancellation, memory use, or Pixel 7a performance.
- No source code, dependency, build configuration, provider setting, or production composition was changed.

### CHANGED_FILES

- `docs/agent-reports/g3-ai.md` (this report only; no runtime or product changes).

### TEST_COMMANDS / TEST_RESULTS

- Source inspection: `rg -n 'ModelChatPort|CloudSettings|cloud.*enabled|RoutedModelChatPort|DemoChat' ...` and targeted `sed` reads — completed; findings above.
- Toolchain inventory: `command -v cmake`, `command -v ninja`, and `find /workspace/android-sdk ... -name 'ndk*'` — no CMake, Ninja, or NDK found.
- Device inventory: `adb devices -l` — daemon started; no devices attached.
- Official model metadata: queried Hugging Face model metadata API for `Qwen/Qwen3-0.6B-GGUF` — `license=apache-2.0`; sole `.gguf` entry observed `Qwen3-0.6B-Q8_0.gguf`, size `639446688` bytes.
- No build or tests were run: no PoC implementation was added. There is no inference result, emulator result, ARM64 execution result, or Pixel 7a benchmark to report.

**CI_RUN_IDS:** None<br>
**ARTIFACT_IDS:** None

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

### AI route comparison

| Route | Eligibility / terms | Execution / device and memory | License / model provenance | Integration / security | Pixel 7a unknowns | Decision |
|---|---|---|---|---|---|---|
| **Ollama Cloud** (existing fixed adapter) | Ollama Terms require a user of the Service to be 18+. Owner has not confirmed audience age, account eligibility, or release/user region; do not infer adults-only from “Personal Beta.” | Cloud inference; model memory is provider-side. Official privacy/pricing says hosting is primarily US and may route to Europe. Pay-per-token model pricing and plans can change. App/device inference latency is not measured. | Hosted model weights are not shipped by Serea. Exact model ID and the current hosted model provenance/licensing still require activation-time review; the frozen ADR roster may be stale relative to current catalog/prices. | Adapter exists with default disabled and exact egress gate. Cloud prompt/response processing is described as transient and not trained on; cost is token-based. Cross-border egress and provider account custody remain explicit risks. No network request occurred. | No device-local performance/memory claim. Pixel 7a network quality, end-to-end latency, battery, or data costs are untested. | **Conditional candidate only.** Do not enable pending audience/age and region determination, current model/pricing review, data-flow approval, account ownership, and budget. |
| **OpenAI API** (direct) | Services Agreement disallows minors using service without parent/guardian consent. Under-18 API guidance requires reasonable age-appropriate safety for organizations serving minors and says not to process personal data of children below 13 (or applicable digital-consent age) without approved Zero Data Retention. Access countries are maintained in an official list; Owner account and user/deployment countries are unverified. | Cloud inference; model memory is provider-side. Token-priced per model; rate limits/pricing change and must be rechecked before approval. No API call made, so no measured latency. | Hosted model weights are proprietary/provider-hosted; no weight redistribution. Select an exact currently supported model only after official source, data, and price review. | No OpenAI adapter exists in PR #123. API data is not used to train by default, but abuse logs may include content and default retention is up to 30 days; ZDR/MAM require prior approval and have limitations. Never embed a direct API key in the APK; key custody/backend requires a separate decision. Preserve Serea egress/usage ordering. | Network latency, availability, battery, carrier cost, and any API region/data-residency behavior from Pixel 7a are untested. | **Potential candidate only** after audience, region, safety, retention, model, billing, and credential-custody decisions. Not connected/authorized. |
| **Gemini API / AI Studio** | Additional Terms (effective 2026-03-23): developer must be 18+, API clients must not be directed to or likely accessed by under-18s, and service is for professional/business development, not consumer use. Owner account country and audience remain unverified. | Cloud inference; model memory is provider-side. Only listed regions allowed; EEA/UK/Switzerland clients must use paid services. On unpaid service, prompts/responses may be used to improve products and human reviewed; do not send sensitive/confidential/personal data. Paid access costs and requires explicit approval. | Hosted weights cannot be extracted or redistributed under the service terms. Exact model is not selected. | No adapter wired. Current no-unapproved-egress rule blocks usage. Google free-tier data use is incompatible with sending personal assistant content; paid terms do not itself authorize Serea's egress. | Pixel 7a bandwidth, latency, battery and supported-country service access not tested. | **Not suitable for this consumer Personal Beta under current terms.** No workaround/activation. |
| **ML Kit GenAI / Gemini Nano (AICore)** | ML Kit GenAI Additional Terms require developer age 18+ and prohibit apps directed to or likely accessed by under-18s. Prompt API is marked Beta; terms prohibit Preview/Experimental use in production. | On-device inference, with AICore managing model distribution and execution; memory/limits are model/device managed and no Pixel 7a budget is published here. Some feature-specific APIs only perform narrow tasks; Prompt API is Beta. | Google-managed model weights; no Serea model artifact or weight distribution. AICore/ML Kit terms and attribution/data disclosure obligations apply. | SDK reports input/output are processed on-device, but it may send API performance/utilization metrics to Google; user disclosure may be required. Uses AICore and has per-app quotas / foreground-use restrictions in official docs. | Pixel 7a is **not** on the current official Prompt or feature-specific supported-device lists. No runtime `checkStatus`, AICore availability, memory, latency, or API 26/35 test occurred. | **Not a valid Pixel 7a target-device route today.** Support, production status, and age conditions remain disqualifying/unresolved. |
| **`llama.cpp` + official Qwen3-0.6B GGUF** | Technically no service-account age or region requirement for local inference; applicable local model use terms still apply. `llama.cpp` code is MIT; official Qwen model card reports Apache-2.0. | Inference can be fully local/offline. Official Q8 GGUF file is **639,446,688 bytes**. Model resident memory, KV/cache, peak/free RAM, thermals, token rate, and storage/update impact are **unknown**; weight bytes are not a memory benchmark. Upstream documents Android `arm64-v8a` NDK build, with its example targeting API 28. | Official Qwen repo/model card is clear; exact Q8 file was identified through official model metadata, but it was **not downloaded**, SHA-256 not recorded, and redistribution/notices/transitive runtime licenses are not yet audited. | No app integration. A real local adapter must get a distinct egress target/model identity from `LOCAL_DEMO`, keep requests offline, verify artifact hash at load, reject corrupt files, honor coroutine cancellation, free native context, and preserve Usage behavior. NDK/JNI and SBOM review required. | No ARM64 model-load/generation run, no API 26 compatibility proof, and no Pixel 7a RAM/latency/battery evidence. Host/x86 or emulator throughput cannot substitute for Pixel 7a. | **Feasibility only.** Technically promising but PoC is blocked in this environment; no PASS/performance claim. |

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

### KNOWN_ISSUES

- No approved/connected real AI provider exists in the inspected app composition. The ordinary Chat screen remains a deterministic local demo, not real AI.
- Cloud routes remain Owner-blocked for eligibility, age/audience, location, price/budget, credential custody, and data-transfer decisions. Provider-level terms have been documented separately from Owner facts.
- Gemini Nano/AICore does not list Pixel 7a as supported in the current official device list. The owner’s physical Pixel 7a is unavailable; no device query or performance test occurred.
- There is no valid local inference PoC. Model load/generation, multi-turn, cancellation, release, recovery, corrupt-file rejection, offline behavior, memory, latency, API 26 compatibility, emulator/ARM64 execution, and Pixel 7a performance are all **NOT_RUN**.
- The current local egress identity is explicitly demo-only. A real local inference route needs a new separately reviewed local target/catalog/provenance contract and must not be made to appear as cloud AI or demo output.
- The PoC JNI model/context/sampler remain process-global behind the documented single-owner lease; support for parallel model instances is intentionally not provided. Instrumentation has not yet compiled, so cross-engine and load/close tests are source-level expectations, not verified passing results. PSS before/load/after-generation/after-close is logged as an Emulator diagnostic metric only; it is not asserted as proof of memory reclamation.
- Pinned library revision, transitive license inventory, SBOM, model SHA verification, model download/update plan, resource constraints, and safety evaluation remain open.

### CURRENT_BLOCKERS

- Tooling: Android NDK/CMake/Ninja are absent from this workspace; there is no Android ARM64 device available. This prevents an Android JNI build and on-device runtime validation here.
- Exact tool/device evidence: `command -v cmake` and `command -v ninja` emitted no executable path; `/workspace/android-sdk` contained no `ndk*` directory; `adb devices -l` listed no devices.
- Artifact: the smallest verified GGUF file found in the selected official repository is Q8 at 639,446,688 bytes. It was intentionally not downloaded because without native toolchain/device the large artifact would add no valid inference evidence.
- Owner facts: intended user age/audience, provider account eligibility, launch regions, usage budget, model allowlist, and acceptance of each provider’s data terms are unknown. These do not block isolated Fake/offline development but block real service enablement.

### INTEGRATION_DEPENDENCIES

- G4 must not replace ordinary Chat with any real route on the basis of this research.
- A cloud integration requires Owner approval of a specific provider/model, intended user age and supported countries, data terms/retention, cost ceiling, and operator credential custody. It must retain Egress → Usage reservation → dispatch ordering and keep Google-origin data blocked by default.
- A local model integration requires a dedicated `LOCAL_ON_DEVICE` (or equally explicit) contract separate from `LOCAL_DEMO`, immutable runtime/model provenance, SHA verification, asset/update policy, Android API 26 compatibility evidence, and a model-sized device benchmark. The new project is test-only source; it is not an inference result or product integration.
- No modifications are requested to `MainActivity.kt`, `SereaAppShell.kt`, or composition root.

### REVIEW_REQUEST

Please review the provider eligibility distinctions, especially the split between provider-level age/region rules and Owner-unverified account/audience facts. Confirm that the local route remains explicitly feasibility-only and that G4 does not treat this report as a PoC or real AI E2E.

### Recommended next engineering path

1. Continue product-independent Fake/offline Chat and test work; keep `CloudSettings.enabled=false` and do not bind a real API key/provider.
2. For a local PoC, provision a pinned NDK/CMake/Ninja toolchain and an isolated ARM64 emulator/device environment first. Confirm build output/API compatibility against Serea's API 26 minimum before obtaining weights.
3. If that setup is available, fetch the official Qwen artifact only under a recorded source revision, license copy, exact size, and SHA-256. Keep it as an external test fixture, not inside product APK or normal test artifacts.
4. Implement a test-only `ModelChatPort` adapter and test load/single-turn/multi-turn, cancellation, native release, corrupt/hash-mismatch rejection, error recovery, and zero-network behavior. Treat every test as NOT_RUN until it actually runs with the real weights.
5. Collect separate ARM64-emulator and Pixel 7a measurements for peak RAM, startup, tokens/sec, thermal behavior, storage, cancellation latency, and process-death recovery. Only Pixel 7a data can answer Pixel 7a performance.
6. Independently audit the pinned runtime/native dependency licenses/SBOM and add a distinct reviewed `LOCAL_ON_DEVICE` egress/catalog identity before any product-composition work.
7. Revisit Cloud candidates only after Owner records exact target audience age, launch regions, account qualification, model allowlist, data terms/retention, spend cap, credential custody, and any guardian/safety requirements. Provider-term eligibility does not establish the Owner's account or users' eligibility.

### Explicit no-test / NOT_RUN ledger

These are research observations, not passing tests:

| Check | State |
|---|---|
| Source/contract inspection | DONE (read-only) |
| Official policy/runtime/model metadata review | DONE (read-only, sources above) |
| Project build / compile | NOT_RUN |
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

### NEXT_ACTION

Proceed with Fake/offline development. For a future isolated local PoC, first provision a reproducible native toolchain (pinned NDK/CMake/Ninja) and a controlled ARM64 emulator/device run. Then acquire the official Qwen model only under an explicit artifact-size/hash/license record; implement test-only `ModelChatPort` adapter and lifecycle/corrupt/offline tests. Before product integration, prove API 26 compatibility and obtain Pixel 7a memory/latency evidence. Keep real Cloud use disabled until Owner eligibility and data/budget decisions are recorded.
