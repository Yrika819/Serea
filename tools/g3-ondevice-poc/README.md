# Isolated llama.cpp Android feasibility PoC

This is a standalone test application under `tools/`. It is not included by the Serea Android Gradle settings, product APK, normal chat composition, or any existing CI gate. Its manifest has no Internet permission. It uses no real credentials or service accounts.

The Kotlin adapter has the same suspend `send(request): response` shape as Serea's `ModelChatPort`, but is a test-local structural contract; it does not import or replace the production interface. The adapter verifies the model's SHA-256 before passing a path to JNI. Its native runtime is pinned to llama.cpp commit `276378515ceebee5d14bcc8536603dd5c87491a2` (tag `b11927`).

## Fixed model candidate

The test fixture is the official `ggml-org/SmolLM2-135M-GGUF` repository at revision `1076b85d135017971845b19b8d07df1479155988`. The repo metadata declares `apache-2.0`; its model card identifies `HuggingFaceTB/SmolLM2-135M` as the base model. The selected Q4_K_M file is 101,016,128 bytes with SHA-256 `e3131339bf4e8065265593d4fd8f7bb7ff2d3abff1edb5618aa1197b89cad9f5`.

The weight file is deliberately absent from this repository. An explicitly selected disposable Emulator run fetches only this exact revision and file, checks both size and SHA-256, stages it to app-private Emulator storage, and passes the same SHA as an instrumentation argument. The instrumentation test fails if either argument or the model file is missing; it does not skip inference. The test runner removes the app-private and temporary copies during cleanup, and the workflow uploads evidence only, never model weights.

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

The non-required `.github/workflows/g3-ondevice-poc.yml` accepts an explicit `workflow_dispatch` choice of API 26 or 35. Its push path remains gated by `[g3-poc-run]` and now also requires exactly one API marker, `[g3-api=26]` or `[g3-api=35]`; this lets a draft-branch diagnostic select one API without rerunning the other. The workflow records KVM/acceleration metadata, runs the synthetic download-bound check, and builds both APKs before creating an Emulator or downloading model weights. It uses the matching `google_apis;x86_64` system image and refuses to continue without hardware acceleration. The API 26 x86_64 package is listed by the local Android SDK repository; the disposable runner still verifies installation and usable KVM. The diagnostic installs and hash-checks both APKs, fetches the exact bounded model fixture, stages it app-private, and invokes one exact non-skipped test. The cleanup trap removes both staged model copies. This diagnostic remains separate from product Chat and all Required gates. Since this workflow is currently on an unmerged G3 Draft branch, the immediate API 26 run uses the marked-push route; GitHub manual dispatch is available when the workflow exists on the default branch.

Both Gradle invocations run through `run_poc_gradle`, which changes to this standalone project directory before executing `./gradlew`. The pre-build `scripts/test-gradle-project-dir.py` regression check verifies both call sites, the instrumentation target-context API, and the `Long` PSS counters. Guest CPU pressure is optional: access denial is recorded and does not stop the diagnostic. The global EXIT cleanup records the primary status, removes any partial host/device model, and resets shared cleanup state. `scripts/test-runner-failure-paths.py` uses fake `adb` to test denied PSI and early-exit cleanup. These guards follow failures in runs `40510213740`, `40511062977`, and `40512775686`.

## Native ownership and lifecycle contract

The llama.cpp JNI resources are process-global in this PoC, so `NativeLlamaEngine` enforces one active engine lease per process. A second engine cannot load while another owns the model; only the matching engine ID may generate, cancel its `(engineId, generationId)` request, or free the native state. Closing releases the lease, after which a new engine may load. Per-engine single-thread executors serialize load, generation, and close. A load queued before `close()` checks lifecycle state before native entry and completes with an error when close wins. This is an explicit PoC singleton contract, not a product concurrency design.

The instrumentation source contains deterministic checks for second-engine rejection, same-numbered cross-engine cancel/free isolation, successful reuse after close, queued-load versus close (with a 15-second bounded await), queued and in-flight generation cancellation, close-state rejection, and hash-matched corrupt GGUF rejection. An expected close rejection is converted to a value inside its async child so it cannot cancel the test parent; cancellation and unexpected failures remain failures. The instrumentation logs process PSS as an observational metric before/load/after generation/after close; PSS is not asserted as proof that native memory was returned to the OS.

## Current evidence (2026-10-27)

- llama.cpp static `libllama.a` compiled for `arm64-v8a`, API 26, Release. SHA-256 `19f2c604e11333ca61a07655595a560c67c5be45fae0bce217b6087f4de2be8f`.
- The standalone Android app and JNI library compiled; host contract unit tests in `ModelChatPortContractTest` passed **3/3** (0 failures, 0 errors, 0 skipped). Final review rerun log: `artifacts/g3-ondevice-poc/gradle-unit-final-review.log`.
- Final debug APK assembled with arm64-v8a and x86_64 JNI libraries. APK SHA-256 `4115f121e329bc765ebb3b92242ac6789d9f0647e1e673efba65fb3a9506f4c3`. Final review assemble log: `artifacts/g3-ondevice-poc/gradle-assemble-final-review.log`.
- API 35 disposable Emulator run `40584263964` on source `f0a9d92878c804aae3a2d0b5192b957186e06807` passed the exact lifecycle test **1/1**, failures/errors/skips `0/0/0`. Artifact `12772451027`, verified ZIP SHA-256 `bd17edf299f08c601a29c019a57a146a73730ab610941cc37fba43e3e6eb2907`. The target and AndroidTest APK SHA-256 values were `2bf7ac78626e77432d96d029cbede1fe172b246413e11bf2899d5c48b6230a92` and `1564e276ce72b71050929da0f9397daeb21250c8412d464a2162b7fabc4f4bd2`; installed hashes matched. Model revision, byte count, and digest matched the fixed fixture. The test exercised model load, single/multi-turn generation, cancellation, recovery, release, and corrupt-model rejection. Cleanup confirmed app-private model removal by target uninstall, temporary model removal and verification, and state reset.
- API 35 environment in run `40584263964`: Google APIs x86_64 image, `ranchu`, boot completed, KVM v12 installed and usable. Guest CPU pressure was unavailable to the cleanup script, but this did not affect JUnit or cleanup. The matching API 26 x86_64 image package `android-26;google_apis;x86_64` is listed by the local Android SDK repository; each hosted API run separately records installation, boot, and KVM evidence.
- Local AndroidTest APK compilation remains unavailable because this workspace lacks the pinned llama.cpp checkout and could not resolve AndroidX dependencies in a prior local attempt. The exact API 35 hosted build and AndroidTest source compilation passed.
- The workflow installs `emulator` and the selected `system-images;android-${G3_API_LEVEL};google_apis;x86_64` by package name without pinning package revisions. It records the actual installed package/emulator versions; package resolution can change, so the workflow is not bit-for-bit reproducible.
- `/dev/kvm` is absent and `emulator -accel-check` reports no VMX/SVM; no local Emulator was launched.
- Model weights were downloaded only by the disposable API 35 CI run, hash-checked, staged in the app-private data directory, and removed during cleanup; they are not stored in the repository or artifact. The instrumentation logs Emulator ABI/API, model size, elapsed durations, and process PSS; it does not log prompts or generated text.

The shared library's ARM64 build and API 35 `x86_64` Emulator pass are not evidence of Pixel 7a performance. Any `x86_64` Emulator inference result is a harness/runtime check only. Pixel 7a throughput, resident/peak memory, thermal behavior, battery use, and storage impact remain unmeasured.
