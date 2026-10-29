# G9R Native Runtime Limits Follow-up

## Authority and scope

- Base source: PR #160 `025aff71effb03e0fe3bd540079530369e9beaf7`.
- Base tree: `73ff9019802aba4a7ac9437e52ff407842fbfcdc`.
- This branch adds Native diagnostics and CI only. It does not modify the app composition root or connect ordinary Chat.
- Current x86_64 inference evidence remains source-bound to the existing PR #160 run; this follow-up has no new emulator execution yet.

## Changes

- Default `LocalModelRuntimeLimits.contextTokens` is 512, matching the current Kotlin/JNI backend bound. The limits object rejects context above 512, output above 256, or more than two worker threads; the Native backend retains its own pre-allocation checks. Widening these values requires an explicit backend capability contract and focused tests.
- `maxResidentMemoryBytes` is renamed `maxModelFileBytes`. The limit checks model-file length before hashing/loading. It is not RSS/PSS enforcement.
- Optional diagnostics sample Android process PSS in KiB around model load, each of the first two inference turns, and release. Cancellation-to-release latency is retained. `pss_measurement_status=PASS` requires every load, two-turn, and both release samples; otherwise it is `NOT_AVAILABLE`. All samples are allowlisted integers and contain no model output.
- Raw instrumentation and logcat files are removed before cleanup can publish success. The artifact upload now lists only the sanitized XML/status summaries, source/hash metadata, metrics summary, and cleanup boundary. The XML failure detail contains exception type only, with raw messages/stacks suppressed.
- Native opt-in is false by default and confined to the `isolated` build type. Build configuration rejects Native opt-in for debug, release, Beta, and aggregate packaging tasks, disables all non-isolated variants, and provides a focused variant-isolation task.
- Added a single 30-minute API 35 ARM64 guest experiment on an `ubuntu-24.04` x86_64 host, using software CPU emulation, pinned API image revision, bounded model fixture, explicit ABI identity, and an arm64 JNI package assertion. Its result must be reported as `ARM64_GUEST_SOFTWARE_EMULATED`; it cannot establish ARM device performance or Pixel acceptance.

## Deferred Product Chat Integration Proposal

No production wiring is included in this patch. Once ARM64 runtime evidence and independent security review are accepted, a follow-up may add an explicit local-model choice to the existing `ModelChatPort`/Model Router boundary behind a separate `LOCAL_MODEL_CHAT_ENABLED` product feature flag, default false and disallowed in Beta/release builds until acceptance. This product flag is separate from `LOCAL_NATIVE_RUNTIME_ENABLED`, which remains only the isolated diagnostic build flag. Until both gates pass, normal Chat remains on its existing DEMO route and the product flag must stay off. Any future route must preserve the existing Egress and Usage wrappers, require the local-model consent contract, retain cancellation and model identity checks, surface local-runtime errors, and fail closed with no silent Cloud fallback. Google-origin data remains blocked from Cloud Egress; Google connection/Handoff consent never implies local-model or Cloud consent. The proposal requires product UI and lifecycle tests before it can be considered for integration.

## ARM64 execution feasibility sources

Checked 2026-10-29 UTC:

- GitHub documents `ubuntu-24.04-arm` as a standard public-repository runner label with a 4-core, 16-GB ARM64 VM: <https://docs.github.com/en/actions/reference/runners/github-hosted-runners#standard-github-hosted-runners-for-public-repositories>.
- The published Android SDK repository lists Linux x64 emulator host archives (for example `emulator-linux_x64-16696583.zip`), not a Linux ARM64 host emulator archive: <https://dl.google.com/android/repository/repository2-1.xml>.
- The Google APIs catalog lists API 35 ARM64 guest image revision 9 (`system-images;android-35;google_apis;arm64-v8a`): <https://dl.google.com/android/repository/sys-img/google_apis/sys-img2-1.xml>.
- The pinned Android Emulator Runner action accepts `arm64-v8a` as a guest image architecture: <https://github.com/ReactiveCircus/android-emulator-runner/blob/a421e43855164a8197daf9d8d40fe71c6996bb0d/README.md>.

Thus the experiment uses the x86_64 runner to host the published x86_64 emulator binary and an ARM64 Android guest in software mode. Until the bounded job actually boots and the native instrumentation passes, ARM64 runtime remains NOT_RUN.

## Focused verification

- `:app:testDebugUnitTest --tests app.serea.android.LocalModelChatAdapterTest --tests app.serea.android.LocalModelNativeRuntimeTest`: 25 tests, 0 failures/errors/skips (including an explicit context/output/thread capability boundary test).
- `:app:compileIsolatedAndroidTestKotlin`: PASS.
- `-PsereaLocalNativeRuntime=true ... :app:verifyLocalNativeRuntimeVariantIsolation`: PASS; only `isolated` remains enabled.
- `-PsereaLocalNativeRuntime=true ... :app:assembleBeta`: rejected by the expected diagnostic-only guard.
- Runtime-metric parser tests: 4/4 PASS.
- Instrumentation-stream parser tests: 8/8 PASS; failure messages/stacks are not emitted into JUnit XML.
- Upload allowlist, JNI contract, preflight, cleanup, shell syntax, YAML parse, and `git diff --check`: PASS.

## Gate state

- x86_64 API 35 inference: prior exact PR #160 evidence remains PASS for the pinned SmolLM2 diagnostic scenario; this follow-up's instrumentation changes still require a new exact-source CI run.
- ARM64 APK build: prior PR #160 evidence remains PASS for APK inclusion only; follow-up workflow will assert the arm64 `.so` again.
- ARM64 guest inference: NOT_RUN pending the bounded workflow.
- Native RSS/PSS cap: NOT_IMPLEMENTED; PSS is observational evidence only.
- Pixel 7a physical runtime: NOT_RUN; owner authorization remains required.
- Ordinary Chat integration: NOT_CONNECTED pending ARM64 runtime evidence and independent review.
- Standard model selection: UNDECIDED; no model recommendation is made here.
