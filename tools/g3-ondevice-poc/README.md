# Isolated llama.cpp Android feasibility PoC

This is a standalone test application under `tools/`. It is not included by the Serea Android Gradle settings, product APK, normal chat composition, or any existing CI gate. Its manifest has no Internet permission. It uses no real credentials or service accounts.

The Kotlin adapter has the same suspend `send(request): response` shape as Serea's `ModelChatPort`, but is a test-local structural contract; it does not import or replace the production interface. The adapter verifies the model's SHA-256 before passing a path to JNI. Its native runtime is pinned to llama.cpp commit `276378515ceebee5d14bcc8536603dd5c87491a2` (tag `b11927`).

## Fixed model candidate

The test fixture is the official `ggml-org/SmolLM2-135M-GGUF` repository at revision `1076b85d135017971845b19b8d07df1479155988`. The repo metadata declares `apache-2.0`; its model card identifies `HuggingFaceTB/SmolLM2-135M` as the base model. The selected Q4_K_M file is 101,016,128 bytes with SHA-256 `e3131339bf4e8065265593d4fd8f7bb7ff2d3abff1edb5618aa1197b89cad9f5`.

The weight file is deliberately absent from this repository and is not included in CI artifacts. It was fetched from this exact revision for the disposable API 35 run `40515091197`, size/SHA-256 checked, and staged into the test app's private files directory. That run's JUnit test failed at the native generation gate before producing a response; see `docs/agent-reports/g3-ai-feasibility.md`. The model bytes were not committed or uploaded. The later recovery run `40533524841` was cancelled before a JUnit result and does not establish inference or cleanup behavior.

## Build contract

Pinned build inputs used locally:

- Temurin JDK 17.0.21+9, verified archive SHA-256 `8b9b611d917e3dfd8419d3566b5949226f69b052fed7e109c1368add1dfe8711`
- Gradle wrapper 8.14.6 (distribution SHA is in `gradle/wrapper/gradle-wrapper.properties`)
- Android Gradle Plugin 8.12.3 and Kotlin 2.4.21
- Android NDK 28.2.13676358
- CMake 3.31.6 and SDK Ninja 1.12.1
- Android compile SDK 35, app minSdk 26
- llama.cpp commit `276378515ceebee5d14bcc8536603dd5c87491a2`

From this directory, set `G3_LLAMA_CPP_SOURCE` to that exact source checkout and run:

```bash
./gradlew --no-daemon --console=plain :app:testDebugUnitTest :app:assembleDebug :app:assembleDebugAndroidTest
```

The CMake arguments are pinned in `app/build.gradle.kts`: static llama/ggml libraries; tests, tools, examples, server, common utilities, app binary, OpenSSL, native CPU tuning, OpenMP, llamafile, dynamic backend loading, and KleidiAI disabled. Both `arm64-v8a` and `x86_64` are built. The command also requires AndroidX test dependencies from Google's Maven repository.

## Diagnostic workflow boundary

The non-required `.github/workflows/g3-ondevice-poc.yml` runs only for a push to the owned G3 branch whose commit message contains `[g3-poc-run]`. It records KVM/acceleration metadata, runs the synthetic download-bound check, and builds both the app and AndroidTest APKs before the Emulator step. If either APK build or AndroidTest compilation fails, the workflow stops before creating the Emulator or downloading model weights. Only after that build succeeds does the disposable API 35 `google_apis` x86_64 KVM Emulator step install and hash-check the APKs, fetch the exact bounded model fixture, stage it app-private, and invoke one exact non-skipped test. The cleanup trap removes both staged model copies; the workflow uploads evidence only, never model weights. This diagnostic remains separate from product Chat and all Required gates.

Both Gradle invocations run through `run_poc_gradle`, which changes to this standalone project directory before executing `./gradlew`. The pre-build `scripts/test-gradle-project-dir.py` regression check verifies both call sites, the instrumentation target-context API, and the `Long` PSS counters. Guest CPU pressure is optional: access denial is recorded and does not stop the diagnostic. The global EXIT cleanup records the primary status, removes any partial host/device model, and resets shared cleanup state. `scripts/test-runner-failure-paths.py` uses fake `adb` to test denied PSI and early-exit cleanup. These guards follow failures in runs `40510213740`, `40511062977`, and `40512775686`.

## Native ownership and lifecycle contract

The llama.cpp JNI resources are process-global in this PoC, so `NativeLlamaEngine` enforces one active engine lease per process. A second engine cannot load while another owns the model; only the matching engine ID may generate, cancel its `(engineId, generationId)` request, or free the native state. Closing releases the lease, after which a new engine may load. Per-engine single-thread executors serialize load, generation, and close. A load queued before `close()` checks lifecycle state before native entry and completes with an error when close wins. This is an explicit PoC singleton contract, not a product concurrency design.

The instrumentation source contains checks for second-engine rejection, same-numbered cross-engine cancel/free isolation, reuse after close, queued-load versus close, queued and in-flight generation cancellation, close-state rejection, and hash-matched corrupt GGUF rejection. The AndroidTest APK compiled and the exact lifecycle test executed once on API 35 in run `40515091197`; it failed before the generation gate, so the assertions after that gate were not reached. Recovery-source changes compiled before run `40533524841` was cancelled, but those fixes have not passed an Emulator runtime test. PSS is designed as an observational metric and is not asserted as proof that native memory was returned to the OS.

## Current evidence (2026-10-27)

- llama.cpp static `libllama.a` compiled for `arm64-v8a`, API 26, Release. SHA-256 `19f2c604e11333ca61a07655595a560c67c5be45fae0bce217b6087f4de2be8f`.
- The standalone Android app and JNI library compiled; host contract unit tests in `ModelChatPortContractTest` passed **3/3** (0 failures, 0 errors, 0 skipped). Final review rerun log: `artifacts/g3-ondevice-poc/gradle-unit-final-review.log`.
- Final debug APK assembled with arm64-v8a and x86_64 JNI libraries. APK SHA-256 `4115f121e329bc765ebb3b92242ac6789d9f0647e1e673efba65fb3a9506f4c3`. Final review assemble log: `artifacts/g3-ondevice-poc/gradle-assemble-final-review.log`.
- The instrumentation source pins model SHA-256 and size, requires target app-private storage, uses per-request cancellation IDs, has deterministic queued/in-flight cancellation gates, and covers native lease ownership and load/close serialization. The local Android instrumentation APK did not build because Google Maven was unreachable and AndroidX test artifacts could not resolve (`Network is unreachable`); failure occurred before AndroidTest source compilation. Hosted runs later compiled the AndroidTest APK. Run `40515091197` compiled and executed the test, failing at the first native-generation gate; run `40533524841` compiled the recovery APKs and staged the model but was cancelled before JUnit. Logs: `artifacts/g3-ondevice-poc/gradle-androidtest-network.log` and `gradle-build-failure.log`; hosted evidence is linked in `docs/agent-reports/g3-ai-feasibility.md`.
- The proposed workflow currently installs `emulator` and `system-images;android-35;google_apis;x86_64` by package name without pinning package revisions. The diagnostic must record the actual installed package/emulator versions; package resolution can change, so this workflow is not bit-for-bit reproducible.
- `/dev/kvm` is absent and `emulator -accel-check` reports no VMX/SVM; no local Emulator was launched.
- A pinned 101,016,128-byte model was downloaded and staged for run `40515091197`; it is not committed or included in the artifact. Native model load **PASS** on API 35 x86_64. Generation did not reach the gate and produced no response. Multi-turn, cancellation, release/recovery, corrupt-model rejection, offline inference, API 26 runtime, ARM64 runtime, and Pixel 7a behavior remain **NOT_RUN**. The recovery run `40533524841` was cancelled before JUnit and is not a pass.
- The test emits synthetic-safe Emulator ABI/API, model size, elapsed durations, and PSS metrics only after the lifecycle reaches its end. The failed run did not produce the final timing/PSS record; those measurements remain **NOT_RUN**.

The shared library's ARM64 build is not evidence of Android runtime compatibility or Pixel 7a performance. `x86_64` Emulator inference, if later run, is a harness/runtime check only. Pixel 7a throughput, resident/peak memory, thermal behavior, battery use, and storage impact remain unmeasured.
