# Native Runtime Limits and ARM64 Integration Proposal

Status: proposal only; no runtime or composition-root change is made here.

## Source authority

- Native baseline: PR #160 source `025aff71effb03e0fe3bd540079530369e9beaf7`, tree `73ff9019802aba4a7ac9437e52ff407842fbfcdc`.
- Static model metadata matrix: PR #163 source `2c109276bcbdd4e14fb2b6e9558962516f2df32c`, tree `b04164da95bf66f43fd08e5537e4d0e242d6a566`.
- At the time of this inspection, PR #160 remained OPEN at the source above; its API 35 x86_64 Native focused run `40884851425` had passed the exact SmolLM2 fixture, two model generations, Usage/Egress, cancellation/release, and cleanup. That is not ARM64 runtime evidence.
- No newer source was substituted for either identifier.

## Review findings

1. `LocalModelRuntimeLimits.contextTokens` defaults to 2,048 in `LocalModelChatAdapter.kt`, while `LlamaCppLocalModelInferenceBackend.load()` and JNI `nativeLoad()` both accept only 128–512. A default-constructed adapter therefore reaches a deterministic pre-allocation rejection. The current end-to-end test avoids this by explicitly selecting 512. Align the native diagnostic profile with its implemented bound, or negotiate a backend capability before creating a request. Do not silently clamp a caller's requested context.
2. `maxResidentMemoryBytes` does not measure or cap resident memory. The backend compares it only with `File.length()`. It is presently a model-file input-size ceiling. Rename it to `maxModelFileBytes` in a versioned API, or preserve a deprecated alias while migrating; do not claim RSS/PSS enforcement.
3. The adapter verifies the model's expected file length and SHA-256 before load. Those checks establish artifact identity/integrity, not the native memory footprint. The current JNI adapter does not expose measured RSS/PSS or a pre-load memory estimate.
4. The environment inspected here contains Android platform 35, build-tools 35.0.0, and platform-tools. It has no `emulator` binary, no installed system images, no accessible `/dev/kvm`, and no usable `javac` at the configured JAVA_HOME. There is no local ARM64 Android emulator route. The existing focused workflow configures only an API 35 x86_64 emulator; APK packaging includes arm64-v8a but does not execute it.

## Proposed safe integration boundary

- Keep normal Chat in its current DEMO behavior. Add no composition-root wiring in this proposal.
- Any later diagnostic exposure must be behind an explicit build-time feature flag that defaults off in normal and release builds. The flag may expose only a diagnostic model route and must not create a Cloud fallback.
- Have a backend capability record declare supported context range, output-token range, worker-thread range, supported ABI, and model-format/revision constraints. Reject unsupported requested limits before backend allocation; no silent truncation or implicit context change.
- For the currently proven llama.cpp adapter, the diagnostic configuration must explicitly use `contextTokens <= 512`, `maxOutputTokens <= 256`, `workerThreads <= 2`, and an artifact whose size/revision/hash/license are pinned. Keep x86_64 runtime evidence labeled separately from arm64-v8a build evidence.
- Replace the misleading memory field with separate concepts:
  - `maxModelFileBytes`: hard pre-load byte-size gate, checked against the immutable artifact manifest and actual file length.
  - `estimatedNativeMemoryBudgetBytes`: a pre-load admission budget only when a backend can derive a documented conservative estimate from model metadata, quantization, context/KV settings, and native workspace. Unknown estimate means reject for that profile; do not infer it from GGUF file size alone.
  - `observedPeakPssBytes` / `observedPeakRssBytes`: measurements collected during bounded emulator/device tests. These are evidence and post-load guards/telemetry, not a pre-allocation guarantee unless the runtime provides an enforceable allocator limit.
- Before enabling any new profile, independently review the memory estimate against the exact llama.cpp revision and model metadata. Exercise load failure, memory-pressure failure, cancellation, double close, reload, and cleanup on each runtime/ABI.
- If Android process-level memory enforcement is unavailable, state that explicitly. A post-load PSS threshold can fail acceptance and trigger release, but cannot prevent a transient allocation peak.
- A separately provisioned ARM64 Android emulator runner may be added only as a bounded, exact-source CI job. It must assert emulator identity, `arm64-v8a` runtime ABI, pinned model hash, 2-turn inference, Usage/Egress, cancellation, release, cleanup, and artifact provenance. Until such a job actually executes and is independently reviewed, ARM64 runtime stays NOT_RUN.

## Gate recommendation

- `X86_64_BUILD`: PASS from the source-bound PR #160 run.
- `X86_64_RUNTIME`: PASS for the pinned SmolLM2 diagnostic scenario on API 35 emulator only.
- `ARM64_BUILD`: PASS from the same run's APK ABI assertions.
- `ARM64_RUNTIME`: NOT_RUN.
- `PIXEL7A_PHYSICAL`: NOT_RUN; owner approval remains required.
- Ordinary Chat integration: BLOCKED pending backend capability/limit alignment, ARM64 runtime evidence, and independent security review.
- Standard local model selection: UNDECIDED; this proposal contains no model recommendation.

## Verification performed for this proposal

- Source inspection only; no model weights downloaded and no runtime was launched.
- Confirmed the 2,048-token Kotlin default, 128–512 Kotlin/JNI checks, and file-length-only use of `maxResidentMemoryBytes` in PR #160 source.
- Confirmed the local SDK inventory and absence of emulator/system-image/KVM/JDK compiler prerequisites for ARM64 emulator inference.
- Independent reviewer: pending. This report is the Native lane's source-bound review proposal, not an independent acceptance of an implementation.
