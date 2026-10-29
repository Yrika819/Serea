# G9R Native Runtime Limits Follow-up

## Authority and scope

- Base source: PR #160 `025aff71effb03e0fe3bd540079530369e9beaf7`.
- Base tree: `73ff9019802aba4a7ac9437e52ff407842fbfcdc`.
- This branch adds Native diagnostics and CI only. It does not modify the app composition root or connect ordinary Chat.
- Current x86_64 inference evidence remains source-bound to the existing PR #160 run; this follow-up has no new emulator execution yet.

## Changes

- Default `LocalModelRuntimeLimits.contextTokens` is now 512, matching the current Kotlin/JNI backend bound. Unsupported explicit limits still fail closed before native allocation.
- `maxResidentMemoryBytes` is renamed `maxModelFileBytes`. The limit checks model-file length before hashing/loading. It is not RSS/PSS enforcement.
- Optional diagnostics sample Android process PSS in KiB around model load, each of the first two inference turns, and release. Cancellation-to-release latency is retained. `pss_measurement_status=PASS` requires every load, two-turn, and both release samples; otherwise it is `NOT_AVAILABLE`. All samples are allowlisted integers and contain no model output.
- Raw instrumentation and logcat files are removed before cleanup can publish success. The artifact upload now lists only the sanitized XML/status summaries, source/hash metadata, metrics summary, and cleanup boundary. The XML failure detail contains exception type only, with raw messages/stacks suppressed.
- Native opt-in is false by default and confined to the `isolated` build type. Build configuration rejects Native opt-in for debug, release, Beta, and aggregate packaging tasks, disables all non-isolated variants, and provides a focused variant-isolation task.
- Added a single 30-minute API 35 ARM64 guest experiment on an `ubuntu-24.04` x86_64 host, using software CPU emulation, pinned API image revision, bounded model fixture, explicit ABI identity, and an arm64 JNI package assertion. Its result must be reported as `ARM64_GUEST_SOFTWARE_EMULATED`; it cannot establish ARM device performance or Pixel acceptance.

## Deferred Product Chat Integration Proposal

This docs-only addendum is based on PR #166 exact source `7c8bfb05db14e8fbab17db75f4027ae13fc3b87e` / tree `eb3559263d679facf5afaf8bcc16a8b7d6244520`. At capture, PR #171 focused run `40915105551` remains queued for both x86_64 and ARM64 software-guest jobs; ARM64 guest inference is NOT_RUN. This proposal is implementation planning only, not runtime acceptance or product enablement.

No production wiring is included in this patch. Ordinary Chat remains on its current DEMO route. `LOCAL_MODEL_CHAT_ENABLED` is a proposed product flag only: it must be defined false by default, remain false in Beta/release variants, and be rejected by build configuration until the gates below are accepted. It is separate from `LOCAL_NATIVE_RUNTIME_ENABLED`, which is only an isolated diagnostic build flag. No `ModelChatPort`, Router, or composition change is authorized by this proposal.

### Proposed request path and authorization order

For a later implementation, use the existing model-port boundary and this exact order:

`RoutedModelChatPort → EgressCheckedModelChatPort(LOCAL_MODEL) → UsageControlledModelChatPort → LocalModelChatAdapter → LocalModelInferenceBackend`

The Router selects the explicitly chosen local model and binds its identity to the request. Egress validates the current conversation, content/source lineage, destination, model identity, and single-use local-model consent before Usage reserves any budget. Usage reserves the request against the selected local model and records provider `local-device` with monetary cost `UNKNOWN`; no price or successful token count may be invented. The adapter then rechecks model identity/integrity and dispatches only to the local backend. A rejected/stale consent or Egress denial must produce zero Usage reservation and zero native dispatch. Cloud is a separate disabled mode and cannot be selected as an automatic recovery path.

Google connection consent, Google-to-Chat handoff consent, local-model execution consent, and Cloud-egress consent remain independent. Google-origin content can reach this local route only through the existing validated handoff contract; Google-origin Cloud egress remains denied.

### Proposed runtime lifecycle and memory policy

An application-scoped local-runtime coordinator should own the selected adapter/backend and serialize load, inference, model changes, and release. A Chat ViewModel or Activity must not own a process-wide native model lease: Activity recreation must preserve request identity without creating overlapping model instances. The coordinator should expose explicit states such as `UNLOADED`, `LOADING`, `READY`, `GENERATING`, `EVICTING`, and `FAILED`, and reject a new dispatch while eviction or release is pending.

- Load lazily only after the explicit local-model mode is selected, Egress and one-use consent pass, Usage reservation succeeds, and the model file passes its pinned size/hash checks. Never pre-load in DEMO mode.
- On model switch or explicit mode exit, stop new requests, cancel active generation, close the backend, and verify release before loading another model. Do not allow overlapping model weights or silently switch models.
- On Activity/background transition, stop new model loads. At `TRIM_MEMORY_UI_HIDDEN`, let an active user-requested inference finish, then unload; if no request is active, unload immediately. More severe `Application.onTrimMemory` low/critical signals should request eviction, stop new starts, cancel active inference under a bounded policy, and release the backend on its worker. The callback must not block the main thread. If release does not complete within the bounded deadline, keep local inference unavailable and show a recoverable error; do not create a second backend or fall back to Cloud.
- After a successful memory-pressure eviction, remain `UNLOADED`. A later user-initiated local send may reload only after the coordinator confirms release completed, current local consent is valid, the selected model is unchanged, and the model file is revalidated. A new consent may be required by product policy; a prior one-use receipt must never be replayed.
- After OS process death, no native lease or in-flight request is restored. An in-flight Usage reservation whose completion was not durably recorded must be reconciled as `OUTCOME_UNKNOWN`. The next process starts `UNLOADED`, revalidates the model identity, and requires a fresh user request and consent. Persisted conversation history is not evidence that inference completed.

`maxModelFileBytes` is a storage/file-integrity bound, not a resident-memory cap. PSS samples are observational and cannot guarantee an allocation ceiling. Before enabling this route, collect exact-device load/generation/release and memory-pressure evidence, including Pixel 7a acceptance when Owner-authorized; define any admission thresholds from that evidence rather than treating emulator PSS as a device limit. Until then, available-memory checks and Android trim signals are conservative admission/eviction inputs only, and the route remains disabled when resource suitability is unknown.

### Cancellation, failure, and user-visible recovery

Cancellation before native generation starts must release the Usage reservation and report a pre-dispatch cancellation. Because model hash, ABI/library, limit, and model-load checks occur after the Usage wrapper reserves but before generation dispatch, each such pre-dispatch failure must release that reservation; if durable release cannot be confirmed, record `OUTCOME_UNKNOWN` rather than silently clearing or charging it as completed. After native dispatch starts, cancellation or timeout must cancel/close the backend, settle the ledger as `OUTCOME_UNKNOWN` where completion cannot be proven, and never retry through another model or provider automatically. A later user retry is a new request with a new send ID, Egress evaluation, consent receipt, and Usage reservation.

Hash mismatch, ABI/library unavailability, unsupported limits, load failure, native exception, out-of-memory or memory-pressure eviction must fail closed, release native resources, and display a clear local-model error with an explicit retry/return-to-DEMO choice. Returning to DEMO requires a user action. There is no silent Cloud fallback, even when Cloud is configured for a different user-selected mode. Release/load failures must not leave a backend marked `READY`; retry is allowed only after the previous release completed and model integrity is rechecked.

### Blocking acceptance gates

Keep `LOCAL_MODEL_CHAT_ENABLED=false` until all applicable gates pass on the exact reviewed source:

1. Independent architecture/security review accepts the Router → Egress → Usage → local adapter path, consent and source-lineage checks, no-Cloud-fallback behavior, cancellation/unknown-outcome accounting, resource ownership, and release/error handling.
2. Existing API 26/API 35 Required identities remain unchanged and pass; focused local Chat tests cover mode selection, stale/used consent, Egress denial before reservation, Usage attribution, cancellation before/after dispatch, load/unload, repeated close, failed release, model switch, and no fallback.
3. Android lifecycle tests prove Activity recreation and actual OS process death do not revive a native lease, stale consent, or in-flight completion. Memory-pressure tests prove the coordinator blocks new dispatch during eviction, synchronizes release with active inference, and reloads only after release and a fresh user action.
4. Exact-source ARM64 Native build, runtime load, and two-turn inference pass with cleanup and provenance. An `ARM64_GUEST_SOFTWARE_EMULATED` pass is recorded as software-guest evidence only; it does not establish ARM hardware/NEON performance or Pixel acceptance. Product performance/resource acceptance and any Beta enablement additionally require the Owner-authorized Pixel 7a criteria and independent review.
5. The selected model has accepted license/redistribution terms and pinned revision/hash. Cloud stays disabled by default, and Google-origin Cloud egress remains denied.

No one of these gates enables the flag automatically. Until the Owner and reviewers accept the applicable integration and release gates, the product flag remains absent or false and ordinary Chat stays DEMO.

## ARM64 execution feasibility sources

Checked 2026-10-29 UTC:

- GitHub documents `ubuntu-24.04-arm` as a standard public-repository runner label with a 4-core, 16-GB ARM64 VM: <https://docs.github.com/en/actions/reference/runners/github-hosted-runners#standard-github-hosted-runners-for-public-repositories>.
- The published Android SDK repository lists Linux x64 emulator host archives (for example `emulator-linux_x64-16696583.zip`), not a Linux ARM64 host emulator archive: <https://dl.google.com/android/repository/repository2-1.xml>.
- The Google APIs catalog lists API 35 ARM64 guest image revision 9 (`system-images;android-35;google_apis;arm64-v8a`): <https://dl.google.com/android/repository/sys-img/google_apis/sys-img2-1.xml>.
- The pinned Android Emulator Runner action accepts `arm64-v8a` as a guest image architecture: <https://github.com/ReactiveCircus/android-emulator-runner/blob/a421e43855164a8197daf9d8d40fe71c6996bb0d/README.md>.

Thus the experiment uses the x86_64 runner to host the published x86_64 emulator binary and an ARM64 Android guest in software mode. Until the bounded job actually boots and the native instrumentation passes, ARM64 runtime remains NOT_RUN.

## Focused verification

- `:app:testDebugUnitTest --tests app.serea.android.LocalModelChatAdapterTest --tests app.serea.android.LocalModelNativeRuntimeTest`: 24 tests, 0 failures/errors/skips.
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
