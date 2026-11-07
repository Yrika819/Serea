# G10-A — Default-off Local Model Chat Composition Spike (spec only)

## Scope

Connect the existing main `LocalModelChatAdapter` to a new explicitly selected test route using a deterministic Fake Backend. Keep the normal `MainActivity` Demo composition and response unchanged. No JNI/native library, model files, downloads, cloud provider, real inference, or external network.

## Production files to touch

- `android/app/src/main/java/app/serea/android/MainActivity.kt`: add explicit, non-restored route selection input to ChatViewModel composition; default always Demo. Do not automatically restore local route after process death.
- `android/app/src/main/java/app/serea/android/LocalModelChatFeatureRouter.kt` (new): route only when the explicit feature toggle and local selection are both present; deny if disabled.
- `android/app/src/main/java/app/serea/android/LocalModelChatAdapter.kt`: reuse as-is unless focused tests identify a correctness defect; do not create a second adapter/lifecycle implementation.
- `android/app/src/main/java/app/serea/android/LocalModelSecurity.kt` and `CloudProvider.kt`: compose `LocalModelAuthorizationRegistry` with existing `EgressCheckedModelChatPort` and `UsageControlledModelChatPort`; no bypass or fallback.

Test files:
- `android/app/src/test/java/app/serea/android/LocalModelChatFeatureRouterTest.kt` (new)
- `android/app/src/test/java/app/serea/android/LocalModelChatAdapterTest.kt` (extend fake-backend cases)
- `android/app/src/androidTest/java/app/serea/android/LocalModelChatViewModelIntegrationAndroidTest.kt` (new, test-only Fake Backend)
- `android/app/src/androidTest/java/app/serea/android/LocalModelProcessDeathConsentTest.kt` (new or folded into integration test)

## Required behavior/tests

1. Default composition remains `DemoChatComposition` and returns Demo response; local route disabled by default.
2. Explicit test selection routes to Fake Backend only; disabled or missing selection fails closed.
3. Missing, wrong-model, wrong-payload, expired, replayed, or process-restored consent is refused before Usage reservation/backend dispatch.
4. Valid content-bound one-use proof is consumed once through the existing Egress boundary; second send/replay fails. No Gmail/Calendar-derived content in spike.
5. Usage reservation/settlement records success and cancellation/error according to existing contract; no untracked dispatch.
6. Cancellation before backend start never enters fake load/generate; cancellation during generation unwinds; `close`/`evictModel` waits for active work and never double-closes or overlaps backends.
7. Activity recreation retains ordinary history but does not silently restore local route or consent. Simulated process death starts Demo/default-off and requires a new explicit selection and fresh consent.
8. Fake backend can count load/generate/cancel/close and assert no filesystem model open or network client is instantiated.

## Verification

- Focused JVM: router default-off, valid explicit route, all fail-closed consent cases, replay, expiry, payload/model mismatch, cancellation race and close/eviction serialization.
- Android integration: send via ChatViewModel using fake; assert demo remains default, valid local selection dispatches once, process recreation drops route/consent, cancellation/history behavior is correct.
- Run repository Formal Required Gate on API 26 and API 35; require actual JUnit XMLs to name the tests and exact HEAD. No skipped-as-pass substitution.
- No Pixel real-model acceptance in this spike. Before real runtime: separately approve model/license and run ARM64 runtime/preflight, memory/thermal, trim/eviction and process-death acceptance on a clean authorized device.

## Stop conditions and owner decisions

Stop if local route can be enabled without explicit selection, a proof survives replay/process death, Egress or Usage is bypassed, cancellation can race into backend start, or tests need a model/provider/network. Owner approval remains necessary before selecting a real model/license, accepting its data flow, enabling real inference, or using a device containing personal data.
