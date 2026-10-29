# G8-B — Local Native Runtime Candidate

**Checkpoint:** 2026-10-29 UTC
**Status:** PARTIAL — product JNI candidate and isolated diagnostic path are pushed in Draft PR #144. Parent review accepted the source provenance and cleanup-publication fixes. Exact-head focused JVM tests pass 22/22 per parent verification; the exact-head Native workflow had no completed runtime evidence at the latest checkpoint. A G9 evidence-only model/runtime license comparison is recorded in [g9-native-model-license-comparison.md](g9-native-model-license-comparison.md). The default product build and Chat route remain unchanged.

## Agent report

```text
AGENT_ID: B — Local AI / Native Runtime
ACTIVE_PHASE: G9 preflight — model/runtime license and distribution evidence
GOAL_STATUS: PARTIAL
BASE_SHA: 00384eb6468214362435140c9a2d848fa2e0f12e
CURRENT_HEAD: 07ea5c6b460cea2eb1ccccc49ccd4f74dc65bb59 (latest PR #144 docs-only tip)
TREE_SHA: ff721fc7ae7642cbaf226376c23df6541d7c8248
SOURCE_HEAD: 233dbdd4fdb64eeb86b43d48b8e71e5452d9d0a0
SOURCE_TREE: 515572da542c676c6957a47ad693cd78ee4cb5b3
CHECKPOINT_SCOPE: native source and config at 233dbdd4fdb64eeb86b43d48b8e71e5452d9d0a0; upstream license metadata checked 2026-10-29 UTC; no model bytes downloaded
WORKTREE: /workspace/Serea-g8-native-local-runtime
BRANCH: codex/serea-g8-native-local-runtime
DRAFT_PR: #144 — https://github.com/Yrika819/Serea/pull/144 (open, draft)
IMPLEMENTATION_COMMIT: 090b1e0b719b219765a9125dc93bf2afcb68781f; implementation tree 61166b5365ade770effb3fbb1346a671eb3bce9b
DOCS_ONLY_COMMITS: 2f04e5b1953741d150456dc0780dabb91d3fbf51; b8d0000c3657526071dc318e901fb5c88d739fe4; `07ea5c6b460cea2eb1ccccc49ccd4f74dc65bb59` (G9 model/runtime license evidence)
PROVENANCE_FIX_COMMIT: ee5c25c9ca7139db9d4fb387c87e5d3922b0c4d8; source tree 5b4f19c66bbb6f922f4e19f3d184e2bfe87aff83
COMPLETED_WORK: added an opt-in product LocalModelInferenceBackend JNI implementation derived from reviewed G3 source; pins llama.cpp source at 276378515ceebee5d14bcc8536603dd5c87491a2; supports bounded context/output/thread settings, per-process model lease, serialized generation, cancel/release, and native prompt/output token counts; added an API 35 x86_64 E2E that drives LocalModelChatAdapter through Egress and Usage using only synthetic user text; added an exact-head Non-Required JVM + native build + Emulator workflow; default build flag remains false; added G9 model/runtime license evidence using upstream metadata and pinned source notices without downloading model weights
CHANGED_FILES: docs/agent-reports/g8-agent-b-native-runtime.md; .github/workflows/g8-local-native-runtime-focused.yml; android/app/build.gradle.kts; android/app/src/main/cpp/CMakeLists.txt; android/app/src/main/cpp/llama_jni.cpp; android/app/src/main/java/app/serea/android/LocalModelNativeRuntime.kt; android/app/src/test/java/app/serea/android/LocalModelNativeRuntimeTest.kt; android/app/src/androidTest/java/app/serea/android/LocalModelNativeRuntimeE2ETest.kt; tools/g8-local-model-native/run-e2e.sh; tools/g8-local-model-native/verify-cmake-clean-checkout.sh
G9_DOCS_ONLY_FILES: docs/agent-reports/g9-native-model-license-comparison.md; docs/agent-reports/g8-agent-b-native-runtime.md (checkpoint update)
TEST_COMMANDS: With `JAVA_HOME=/workspace/.tools/temurin17` and matching PATH, `./gradlew --no-daemon --console=plain -Dhttp.proxyHost=proxy -Dhttp.proxyPort=8080 -Dhttps.proxyHost=proxy -Dhttps.proxyPort=8080 -Djavax.net.ssl.trustStore=/workspace/.tools/temurin17/codex-cacerts -Djavax.net.ssl.trustStorePassword=changeit :app:testDebugUnitTest --tests 'app.serea.android.LocalModelChatAdapterTest' --tests 'app.serea.android.LocalModelNativeRuntimeTest'`; `:app:generateDebugBuildConfig`; `git diff --check`; `bash -n tools/g8-local-model-native/run-e2e.sh tools/g8-local-model-native/verify-cmake-clean-checkout.sh`; Python source/JNI declaration contract check; Python YAML parse for the workflow; CMake dirty-source regression is queued on the post-update focused workflow
JUNIT_RESULT: Local focused JVM PASS using checksum-verified Temurin 17: `LocalModelChatAdapterTest` 21/21 and `LocalModelNativeRuntimeTest` 1/1, total 22 passed, zero failures/errors/skips. An initial Java 21 attempt could not run due to missing `javac`; this is superseded by the Temurin 17 pass. `:app:generateDebugBuildConfig` PASS validates Android Gradle DSL only, not Kotlin/JNI compilation.
CI_RUN_IDS: Exact current PR tip `07ea5c6b460cea2eb1ccccc49ccd4f74dc65bb59`: Native focused run `40803623345` QUEUED at the last bounded observation; no runtime/build/artifact result was available. Parent independently reran the focused JVM classes on exact source head `233dbdd4fdb64eeb86b43d48b8e71e5452d9d0a0` with `--rerun-tasks`, 22/22 PASS. Earlier Native run `40801491917` on the same source head also remained queued and is historical. Historic standalone G3 x86_64 diagnostic: run `40657508541` on PR #127 source `a2bafeea77160e8e2746bdfdd0d309348ac04aa7`.
ARTIFACT_IDS: Historic G3 PoC artifact `12802681672` (not evidence for this product JNI implementation). No exact-head G8 native runtime artifact. License metadata retrieved through upstream API without downloading weights.
SECURITY_IMPACT: `LOCAL_NATIVE_RUNTIME_ENABLED=false` by default; CMake/native libraries are built only with explicit diagnostic property `-PsereaLocalNativeRuntime=true`; no AppProviderComposition, MainActivity, SereaAppShell, normal Chat route, Google scope, Cloud route, or secret configuration was changed. The backend has no HTTP/credential API and the E2E passes a typed LOCAL_MODEL consent through Egress before Usage. Test data are synthetic user-authored strings; model weights are downloaded only by the isolated CI diagnostic and are not committed or uploaded.
KNOWN_ISSUES: no native CMake/JNI build or runtime result yet; new product-runtime active-JNI cancellation/recovery and Process Death paths are not Emulator-verified; `LocalModelRuntimeLimits` default context 2048 is above this reviewed backend's supported maximum 512, so any future composition must select an explicit 512-token configuration; the file-size guard is not an OS-enforced resident-memory ceiling; model quality, native peak memory, thermal/power, and Pixel 7a performance are unverified; complete linked-native dependency notices/SBOM and redistribution review are open.
CURRENT_BLOCKERS: parent exact-source focused JVM rerun passed 22/22. Local environment still has no Android NDK/CMake installation, `/dev/kvm`, Emulator binary, or attached device. Native build and x86_64 inference remain NOT_RUN; focused run `40803623345` was queued at the last bounded observation.
DEPENDENCIES: independent review of the G9 license comparison and Owner redistribution decision; exact-head API 35 x86_64 diagnostic; ARM64 build/runtime evidence; a future parent-owned composition proposal must provide the explicit model artifact/catalog, app-private model path, lifecycle owner, user-visible Demo/Local/Cloud identity, local consent UI, `RoutedModelChatPort → EgressCheckedModelChatPort(LOCAL_MODEL) → UsageControlledModelChatPort → LocalModelChatAdapter → LlamaCppLocalModelInferenceBackendFactory`, and must keep Google-origin lineage/account/session validation intact.
INTEGRATION_READY: NO — source candidate only; do not wire into regular Chat until review, exact-head Android build, x86_64 runtime, cancel/release, usage, integrity, and distribution evidence pass.
REVIEW_REQUEST: Parent and Agent C accepted the native source provenance and cleanup-publication changes. Parent independently checked the upstream model metadata, pinned llama.cpp MIT source, and relevant CMake targets in the G9 report. Redistribution, native APK inventory and legal clearance remain unresolved.
NEXT_ACTION: Parent monitors exact current PR run `40803623345`; use only its exact-head build/inference evidence if the runner executes. Keep ARM64 build/runtime and Pixel 7a separately NOT_RUN unless suitable CI/device evidence exists. Preserve Native default-off until product and distribution acceptance.
OWNER_APPROVAL_REQUIRED: Pixel 7a physical operation; production signing/distribution; main merge; Personal Beta GO. No real Google or Cloud provider approval is required for this Fake/Local diagnostic candidate, and none was used.
```

## Reuse audit and provenance

- PR #126 exact PoC head: `8ca55a32096bc4449717c2fd3332b0602381d7d5`.
- PR #127 exact PoC head: `a2bafeea77160e8e2746bdfdd0d309348ac04aa7`.
- Existing G3 runtime source: `tools/g3-ondevice-poc/app/src/main/cpp/llama_jni.cpp`, `app/build.gradle.kts`, and `NativeLlamaEngine` in `GgufModelChatPort.kt`. The new app bridge is a selective port that removes PoC test gates, adds runtime context/thread parameters and token counts, and changes JNI ownership to the product backend class. The model file remains outside Git.
- Existing G6 adapter source: `LocalModelChatAdapter.kt` and `LocalModelSecurity.kt` already enforce model ID, hash, content/context/consent binding, supported ABI, prompt/output bounds, one-use local authorization, and lifecycle cleanup. The new runtime implements the existing backend interface; it does not duplicate the router, Egress, Usage, or consent boundaries.
- Historic G3 diagnostic run `40657508541` / artifact `12802681672` passed a pinned SmolLM2 inference lifecycle on API 35 x86_64, including cancellation at the pre-decode/native-start gate and resource close/reopen. The G3 checkpoint states that this run did **not** verify cancellation inside `llama_decode` through the installed abort callback. PR #127 source `a2bafeea77160e8e2746bdfdd0d309348ac04aa7` contains a later abort-callback test hook, but that source test has no matching Emulator PASS recorded. Neither G3 result verifies the new product JNI class.

## Candidate runtime boundary

The build declares `LOCAL_NATIVE_RUNTIME_ENABLED=false` unless an isolated build explicitly supplies `-PsereaLocalNativeRuntime=true` and a llama.cpp source checkout. CMake refuses any llama.cpp revision except `276378515ceebee5d14bcc8536603dd5c87491a2` and rejects nonempty `git status --porcelain=v1 --untracked-files=all --ignore-submodules=none` output so both tracked and untracked nonignored changes stop configuration before compilation. `tools/g8-local-model-native/verify-cmake-clean-checkout.sh` creates dirty worktrees to test both rejection paths in CI. The diagnostic APK compiles `arm64-v8a` and `x86_64`; normal app and Beta builds do not compile or include this JNI runtime.

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
| Model | `ggml-org/SmolLM2-135M-GGUF` revision `1076b85d135017971845b19b8d07df1479155988`; upstream metadata reports 101,016,128 bytes and LFS SHA-256 `e3131339bf4e8065265593d4fd8f7bb7ff2d3abff1edb5618aa1197b89cad9f5`; model card declares `apache-2.0` and names base repository `HuggingFaceTB/SmolLM2-135M` | The G9 comparison records authoritative API/card URLs and the absence of a separate GGUF LICENSE/NOTICE file. Metadata only; this audit did not fetch or re-hash weights. Redistribution clearance is not established. |
| llama.cpp | exact source revision `276378515ceebee5d14bcc8536603dd5c87491a2`; upstream license MIT | Linked dependency notices and native binary inventory still need a complete SBOM/license check. |
| Existing x86_64 runtime | G3 run `40657508541`, artifact `12802681672`, standalone PoC; earlier measured load 928 ms, first turn 3,242 ms, next turn 2,166 ms, PSS about 162–166 MB during inference and about 54 MB after close in that Emulator | Historical PoC metrics do not transfer to the new product JNI, nor to a real device. G8 metrics remain NOT_RUN. |
| ARM64 build/runtime | G3 source reported cross-build success in its own isolated diagnostic history | `ARM64_BUILD=NOT_RUN` for this new app bridge until G8 CI runs; `ARM64_RUNTIME=NOT_RUN`; `PIXEL7A_PHYSICAL=NOT_RUN`. |
| License/SBOM | Detailed model/runtime license comparison at [g9-native-model-license-comparison.md](g9-native-model-license-comparison.md); Android Gradle SBOM scope is documented in the existing generator | Complete linked-native dependency inventory, APK-bound native SBOM/NOTICE, attribution package, model redistribution decision, and provenance remain open. |

## Product Chat Integration Proposal

Keep the native backend unavailable in normal Chat while the candidate is reviewed. If later accepted, create a distinct model catalog identity `serea-local/SmolLM2-135M-Q4_K_M` with a verified app-private artifact; use explicit `LocalModelRuntimeLimits(contextTokens=512, maxOutputTokens<=256, workerThreads<=2)`; route only through the existing Router, local Egress consent, and Usage wrappers; and label Demo, Local Model, and Cloud as separate destinations. Never silently route a failed local model to Cloud. A Google authorization or Google→Chat handoff does not grant Local Model or Cloud consent. Google-origin input stays blocked until the live trusted handoff proof, account/session generation, and data-selection binding are revalidated at Egress.

Acceptance before a composition change: exact-head JVM suite; exact-head Android native build; single-case API35 x86_64 model E2E with JUnit/cleanup/APK provenance; verified cancel/release and recovery; ARM64 build/runtime status reported separately; measured native memory and device thermal/power evidence; complete license/SBOM/attribution; and independent security review. Pixel 7a operation remains Owner-controlled.

## Local verification

- `:app:generateDebugBuildConfig`: PASS, confirming the optional Gradle BuildConfig configuration only.
- `git diff --check`: PASS.
- `bash -n tools/g8-local-model-native/verify-cmake-clean-checkout.sh`: PASS.
- CMake dirty-source runtime regression: NOT_RUN locally because CMake is unavailable; exact-head workflow includes tracked and untracked dirty-worktree cases.
- `bash -n tools/g8-local-model-native/run-e2e.sh`: PASS.
- Python check matching Kotlin external JNI declarations to native symbols and asserting no network client in the backend: PASS.
- Python YAML parse of `g8-local-native-runtime-focused.yml`: PASS.
- Focused JVM JUnit: **PASS**, 22 tests total with zero failures/errors/skips using `/workspace/.tools/temurin17`.
- Native build / x86_64 runtime: **NOT_RUN** locally. No NDK/CMake, emulator binary, KVM, or attached device is present.

Personal Beta remains **NO-GO**. This checkpoint does not authorize a live Google connection, a real account, Cloud AI, physical device operation, release signing, distribution, main merge, or runtime enablement in normal Chat.
