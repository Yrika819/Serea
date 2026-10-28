# G8-B — Local Native Runtime Candidate

**Checkpoint:** 2026-10-28 UTC  
**Status:** PARTIAL — product JNI candidate and isolated diagnostic path are committed locally; exact-source JVM/native CI and Emulator execution are pending. The default product build and Chat route remain unchanged.

## Agent report

```text
AGENT_ID: B — Local AI / Native Runtime
ACTIVE_PHASE: G8-B native ModelChatPort backend candidate
GOAL_STATUS: PARTIAL
BASE_SHA: 00384eb6468214362435140c9a2d848fa2e0f12e
CURRENT_HEAD: 090b1e0b719b219765a9125dc93bf2afcb68781f
TREE_SHA: 61166b5365ade770effb3fbb1346a671eb3bce9b
WORKTREE: /workspace/Serea-g8-native-local-runtime
BRANCH: codex/serea-g8-native-local-runtime
DRAFT_PR: NOT_CREATED_YET
COMPLETED_WORK: added an opt-in product LocalModelInferenceBackend JNI implementation derived from reviewed G3 source; pins llama.cpp source at 276378515ceebee5d14bcc8536603dd5c87491a2; supports bounded context/output/thread settings, per-process model lease, serialized generation, cancel/release, and native prompt/output token counts; added an API 35 x86_64 E2E that drives LocalModelChatAdapter through Egress and Usage using only synthetic user text; added an exact-head Non-Required JVM + native build + Emulator workflow; default build flag remains false
CHANGED_FILES: .github/workflows/g8-local-native-runtime-focused.yml; android/app/build.gradle.kts; android/app/src/main/cpp/CMakeLists.txt; android/app/src/main/cpp/llama_jni.cpp; android/app/src/main/java/app/serea/android/LocalModelNativeRuntime.kt; android/app/src/test/java/app/serea/android/LocalModelNativeRuntimeTest.kt; android/app/src/androidTest/java/app/serea/android/LocalModelNativeRuntimeE2ETest.kt; tools/g8-local-model-native/run-e2e.sh
TEST_COMMANDS: `cd android && ./gradlew --no-daemon --console=plain :app:testDebugUnitTest --tests 'app.serea.android.LocalModelChatAdapterTest'`; `cd android && ./gradlew --no-daemon --console=plain :app:generateDebugBuildConfig`; `git diff --check`; `bash -n tools/g8-local-model-native/run-e2e.sh`; Python source/JNI declaration contract check; Python YAML parse for the new workflow
JUNIT_RESULT: Local focused JVM NOT_RUN. Gradle wrapper download succeeded after routing Java through the configured proxy, but task creation stopped because the only installed Java 21 runtime lacks `JAVA_COMPILER`/`javac`. No test case ran. `:app:generateDebugBuildConfig` PASS validates Android Gradle DSL only, not Kotlin/JNI compilation.
CI_RUN_IDS: G8 new workflow not run yet; it triggers on an Agent B Draft PR against `codex/serea-g7-calendar-readiness`. Historic standalone G3 x86_64 diagnostic: run `40657508541` on PR #127 source `a2bafeea77160e8e2746bdfdd0d309348ac04aa7`.
ARTIFACT_IDS: Historic G3 PoC artifact `12802681672` (not evidence for this product JNI implementation). No G8 artifact yet.
SECURITY_IMPACT: `LOCAL_NATIVE_RUNTIME_ENABLED=false` by default; CMake/native libraries are built only with explicit diagnostic property `-PsereaLocalNativeRuntime=true`; no AppProviderComposition, MainActivity, SereaAppShell, normal Chat route, Google scope, Cloud route, or secret configuration was changed. The backend has no HTTP/credential API and the E2E passes a typed LOCAL_MODEL consent through Egress before Usage. Test data are synthetic user-authored strings; model weights are downloaded only by the isolated CI diagnostic and are not committed or uploaded.
KNOWN_ISSUES: no exact-head Kotlin/JNI build or runtime result yet; new product-runtime active-JNI cancellation/recovery and Process Death paths are not Emulator-verified; `LocalModelRuntimeLimits` default context 2048 is above this reviewed backend's supported maximum 512, so any future composition must select an explicit 512-token configuration; the file-size guard is not an OS-enforced resident-memory ceiling; model quality, native peak memory, thermal/power, and Pixel 7a performance are unverified; complete linked-native dependency notices/SBOM and redistribution review are open.
CURRENT_BLOCKERS: local environment has Java 21 runtime without javac, no Android NDK, no CMake installation, no `/dev/kvm`, no Emulator binary, and no attached device. Focused/native verification must run on the isolated GitHub Actions workflow after Draft PR creation.
DEPENDENCIES: independent parent review; exact-head JVM and API 35 x86_64 diagnostic; ARM64 build evidence; a future parent-owned composition proposal must provide the explicit model artifact/catalog, app-private model path, lifecycle owner, user-visible Demo/Local/Cloud identity, local consent UI, `RoutedModelChatPort → EgressCheckedModelChatPort(LOCAL_MODEL) → UsageControlledModelChatPort → LocalModelChatAdapter → LlamaCppLocalModelInferenceBackendFactory`, and must keep Google-origin lineage/account/session validation intact.
INTEGRATION_READY: NO — source candidate only; do not wire into regular Chat until review, exact-head Android build, x86_64 runtime, cancel/release, usage, integrity, and distribution evidence pass.
REVIEW_REQUEST: Parent independent review pending for source/CMake/JNI lifecycle, test coverage, memory claims, and license/provenance.
NEXT_ACTION: push this branch and create a Draft PR against `codex/serea-g7-calendar-readiness`; verify exact-head focused JVM and new native workflow runs; fix failures on a new source HEAD; keep ARM64 runtime and Pixel 7a separately NOT_RUN unless a suitable runner/device exists.
OWNER_APPROVAL_REQUIRED: Pixel 7a physical operation; production signing/distribution; main merge; Personal Beta GO. No real Google or Cloud provider approval is required for this Fake/Local diagnostic candidate, and none was used.
```

## Reuse audit and provenance

- PR #126 exact PoC head: `8ca55a32096bc4449717c2fd3332b0602381d7d5`.
- PR #127 exact PoC head: `a2bafeea77160e8e2746bdfdd0d309348ac04aa7`.
- Existing G3 runtime source: `tools/g3-ondevice-poc/app/src/main/cpp/llama_jni.cpp`, `app/build.gradle.kts`, and `NativeLlamaEngine` in `GgufModelChatPort.kt`. The new app bridge is a selective port that removes PoC test gates, adds runtime context/thread parameters and token counts, and changes JNI ownership to the product backend class. The model file remains outside Git.
- Existing G6 adapter source: `LocalModelChatAdapter.kt` and `LocalModelSecurity.kt` already enforce model ID, hash, content/context/consent binding, supported ABI, prompt/output bounds, one-use local authorization, and lifecycle cleanup. The new runtime implements the existing backend interface; it does not duplicate the router, Egress, Usage, or consent boundaries.
- Historic G3 diagnostic run `40657508541` / artifact `12802681672` passed a pinned SmolLM2 inference lifecycle on API 35 x86_64, including cancellation at the pre-decode/native-start gate and resource close/reopen. The G3 checkpoint states that this run did **not** verify cancellation inside `llama_decode` through the installed abort callback. PR #127 source `a2bafeea77160e8e2746bdfdd0d309348ac04aa7` contains a later abort-callback test hook, but that source test has no matching Emulator PASS recorded. Neither G3 result verifies the new product JNI class.

## Candidate runtime boundary

The build declares `LOCAL_NATIVE_RUNTIME_ENABLED=false` unless an isolated build explicitly supplies `-PsereaLocalNativeRuntime=true` and a llama.cpp source checkout. CMake refuses any llama.cpp revision except `276378515ceebee5d14bcc8536603dd5c87491a2`. The diagnostic APK compiles `arm64-v8a` and `x86_64`; normal app and Beta builds do not compile or include this JNI runtime.

The JNI engine owns one process-wide llama.cpp model/context/sampler lease. Each Kotlin backend serializes load, generation, and release on one daemon executor. Generation cancellation maps to llama.cpp's abort callback; `close()` marks the backend closed, signals active cancellation, queues native free, and returns without blocking the Android lifecycle caller. The E2E test separately waits for release completion. **Active cancellation through this new product JNI and cancellation while inside `llama_decode` are NOT_RUN.** Process death drops the process-local native lease; no native context is persisted, but that behavior also lacks a new Android Process Death test.

For the pinned 135M fixture, the backend rejects a GGUF file whose byte size exceeds `maxResidentMemoryBytes`; independently, it accepts context sizes `128..512`, output `1..256`, and worker threads `1..2`, while the app adapter verifies exact GGUF size/SHA-256 before load. These are hard input/configuration limits only. `maxResidentMemoryBytes` does **not** set an operating-system RSS limit on llama.cpp allocations. The x86_64 historical PSS observations below are measurements from G3, not enforcement evidence for this JNI. This candidate therefore does not prove a hard peak-memory bound and must not be treated as device-resource acceptance.

The diagnostic E2E composes:

```text
RoutedModelChatPort (proposal only; not created in this test)
  → EgressCheckedModelChatPort(LOCAL_MODEL, one-use local consent)
  → UsageControlledModelChatPort
  → LocalModelChatAdapter (exact artifact/hash check)
  → LlamaCppLocalModelInferenceBackend
```

The current E2E covers the exact Adapter/Egress/Usage/native sequence with synthetic `USER_AUTHORED` input, a fixed model ID, and no Google-origin content. Native token counts are recorded as provider usage; monetary cost remains `UNKNOWN` without a verified price. No Cloud fallback is present. Normal Chat continues to use its existing Demo composition.

## Model, runtime, and distribution evidence

| Item | Recorded evidence | Remaining gap |
|---|---|---|
| Model | `ggml-org/SmolLM2-135M-GGUF` revision `1076b85d135017971845b19b8d07df1479155988`; fixture 101,016,128 bytes; expected SHA-256 `e3131339bf4e8065265593d4fd8f7bb7ff2d3abff1edb5618aa1197b89cad9f5`; repository metadata declares `apache-2.0` and base model `HuggingFaceTB/SmolLM2-135M` | The G8 workflow re-fetches and verifies model metadata/bytes; that workflow has not run yet. Metadata is a repository declaration, not a legal redistribution opinion. |
| llama.cpp | exact source revision `276378515ceebee5d14bcc8536603dd5c87491a2`; upstream license MIT | Linked dependency notices and native binary inventory still need a complete SBOM/license check. |
| Existing x86_64 runtime | G3 run `40657508541`, artifact `12802681672`, standalone PoC; earlier measured load 928 ms, first turn 3,242 ms, next turn 2,166 ms, PSS about 162–166 MB during inference and about 54 MB after close in that Emulator | Historical PoC metrics do not transfer to the new product JNI, nor to a real device. G8 metrics remain NOT_RUN. |
| ARM64 build/runtime | G3 source reported cross-build success in its own isolated diagnostic history | `ARM64_BUILD=NOT_RUN` for this new app bridge until G8 CI runs; `ARM64_RUNTIME=NOT_RUN`; `PIXEL7A_PHYSICAL=NOT_RUN`. |
| License/SBOM | Model and llama.cpp declarations recorded above | Complete linked-native dependency licenses/NOTICE, APK-bound SBOM, attribution package, redistribution terms, and provenance remain open. |

## Product Chat Integration Proposal

Keep the native backend unavailable in normal Chat while the candidate is reviewed. If later accepted, create a distinct model catalog identity `serea-local/SmolLM2-135M-Q4_K_M` with a verified app-private artifact; use explicit `LocalModelRuntimeLimits(contextTokens=512, maxOutputTokens<=256, workerThreads<=2)`; route only through the existing Router, local Egress consent, and Usage wrappers; and label Demo, Local Model, and Cloud as separate destinations. Never silently route a failed local model to Cloud. A Google authorization or Google→Chat handoff does not grant Local Model or Cloud consent. Google-origin input stays blocked until the live trusted handoff proof, account/session generation, and data-selection binding are revalidated at Egress.

Acceptance before a composition change: exact-head JVM suite; exact-head Android native build; single-case API35 x86_64 model E2E with JUnit/cleanup/APK provenance; verified cancel/release and recovery; ARM64 build/runtime status reported separately; measured native memory and device thermal/power evidence; complete license/SBOM/attribution; and independent security review. Pixel 7a operation remains Owner-controlled.

## Local verification

- `:app:generateDebugBuildConfig`: PASS, confirming the optional Gradle BuildConfig configuration only.
- `git diff --check`: PASS.
- `bash -n tools/g8-local-model-native/run-e2e.sh`: PASS.
- Python check matching Kotlin external JNI declarations to native symbols and asserting no network client in the backend: PASS.
- Python YAML parse of `g8-local-native-runtime-focused.yml`: PASS.
- Focused JVM JUnit: **NOT_RUN**. Gradle failed before task execution because `/usr/lib/jvm/java-21-openjdk-amd64` has no Java compiler.
- Native build / x86_64 runtime: **NOT_RUN** locally. No NDK/CMake, emulator binary, KVM, or attached device is present.

Personal Beta remains **NO-GO**. This checkpoint does not authorize a live Google connection, a real account, Cloud AI, physical device operation, release signing, distribution, main merge, or runtime enablement in normal Chat.
