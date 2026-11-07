# G10-A — Default-off Local Model Chat Composition Spike (spec only)

## Scope

Connect the existing main `LocalModelChatAdapter` to a new explicitly selected test route using a deterministic Fake Backend. Keep the normal `MainActivity` Demo composition and response unchanged. No JNI/native library, real model artifact/download, cloud provider, real inference, or external network; an inert generated test fixture is allowed only for Adapter integrity checks.

## Composition boundary

- Keep `MainActivity.kt` unchanged: it must continue composing Demo only. Exercise the explicit, default-off selector through the injected `ChatViewModel.factory(ModelChatPort, ...)` seam in tests; production UI/activation is outside this spike. No local route is automatically restored after process death.
- `android/app/src/debug/java/app/serea/android/LocalModelChatFeatureRouter.kt` (new): Debug/isolated-only selector; route only when the explicit test enablement and local selection are both present; deny if disabled. It is not packaged in release/beta source sets.
- `android/app/src/main/java/app/serea/android/LocalModelChatAdapter.kt`: reuse as-is unless focused tests identify a correctness defect; do not create a second adapter/lifecycle implementation.
- `android/app/src/main/java/app/serea/android/LocalModelSecurity.kt` and `CloudProvider.kt`: compose `LocalModelAuthorizationRegistry` with existing `EgressCheckedModelChatPort` and `UsageControlledModelChatPort`; no bypass or fallback.

Test files:
- `android/app/src/test/java/app/serea/android/LocalModelChatFeatureRouterTest.kt` (new)
- `android/app/src/test/java/app/serea/android/LocalModelChatAdapterTest.kt` (existing Fake Backend, Egress, Usage and lifecycle regressions reused)
- `android/app/src/androidTest/java/app/serea/android/LocalModelChatViewModelIntegrationAndroidTest.kt` (new, test-only Fake Backend)
- `android/app/src/debug/java/app/serea/android/ProcessDeathHarnessReceiver.kt` and `tools/run_android_process_death_harness.sh`: extend the existing isolated-emulator force-stop harness with synthetic proof invalidation and fresh-process default-off route checks.

## Required behavior/tests

1. Default composition remains `DemoChatComposition` and returns Demo response; local route disabled by default.
2. Explicit test selection routes to Fake Backend only; disabled or missing selection fails closed.
3. Missing, wrong-model, wrong-payload, expired, replayed, or process-restored consent is refused before Usage reservation/backend dispatch.
4. Valid content-bound one-use proof is consumed once through the existing Egress boundary before Usage reservation; second send/replay fails. Preserve wrapper order `EgressCheckedModelChatPort(UsageControlledModelChatPort(Adapter))`. No Gmail/Calendar-derived content in spike.
5. Usage reservation/settlement records success and cancellation/error according to existing contract; no untracked dispatch.
6. Cancellation before backend start never enters fake load/generate; cancellation during generation unwinds. `close()` is synchronous/non-blocking: it marks closed and requests `cancelActive`; assert new starts are denied, the active send coroutine unwinds/joins, then backend release runs exactly once in the send cleanup. Do not call a nonexistent `evictModel()` API. Test only the current adapter `close()` lifecycle; actual memory-trim eviction/reload and fresh re-consent remain a gate for a later trim bridge/real-runtime change. No overlapping backends or double release.
7. Existing Chat UI instrumentation retains ordinary history through Activity recreation. The test-only route is injected through `ChatViewModel.factory` and is not persisted. The isolated OS force-stop harness verifies that a fresh process cannot consume the pre-death proof and that a fresh route instance denies Local Model selection by default. Since current main has no eviction API, post-eviction reload/re-consent is explicitly not claimed as verified by this spike.
8. Because the existing Adapter verifies a model file (reads/hashes it and passes its canonical path to `backend.load`), use a tiny generated inert test fixture with exact size/hash metadata. Assert the Fake Backend does not read model weights, load JNI/native code, create a network client, or access anything beyond the fixture passed by the Adapter.

## Verification

- Focused JVM: router default-off, valid explicit route, all fail-closed consent cases, replay, expiry, payload/model mismatch, cancellation race, non-blocking close order and release-after-unwind. Test one-use consent replay at the registry/Egress layer; memory-eviction re-consent is deferred until an eviction API is introduced.
- Android integration: inject ports via `ChatViewModel.factory` without changing `MainActivity`; assert valid test-only local selection dispatches once, missing consent dispatches nothing, and cancellation does not save an assistant reply. Existing formal Chat screen tests cover Demo/history Activity recreation; the OS process-death harness separately checks consent invalidation and a fresh default-off route.
- Run repository Formal Required Gate on API 26 and API 35; require actual JUnit XMLs to name the established formal tests and exact HEAD. Run the three G10-A Android tests through the separate focused API 26/35 campaign; keep them listed as non-required so the established 105-test formal identity set does not change. No skipped-as-pass substitution.
- No Pixel real-model acceptance in this spike. Before real runtime: separately approve model/license and run ARM64 runtime/preflight, memory/thermal, trim/eviction and process-death acceptance on a clean authorized device.

## Stop conditions and owner decisions

Stop if local route can be enabled without explicit selection, a proof survives replay/process death, Egress or Usage is bypassed, cancellation can race into backend start, or tests need a model/provider/network. Owner approval remains necessary before selecting a real model/license, accepting its data flow, enabling real inference, or using a device containing personal data.
