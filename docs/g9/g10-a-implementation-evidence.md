# G10-A implementation evidence

## Scope and baseline

- Repository: `Yrika819/Serea`
- Base `main` at implementation start: `9a32b4e7b0f5357ae4ac88f94aa45ac7ec80e672`
- Scope: default-off Local Model Chat composition spike with a deterministic injected Fake Backend.
- No model weights, runtime download, JNI/native loading, network client, real provider, OAuth, personal data, signing, or distribution is included.
- `MainActivity` is unchanged and continues to use `DemoChatComposition`.

## Composition

```text
ChatViewModel.factory(injectedPort)                 MainActivity factory
                 |                                         |
                 v                                         v
Debug-only LocalModelChatFeatureRouter            DemoChatComposition
                 |                                         |
                 | explicit test selection                | normal launch
                 v                                         v
RoutedModelChatPort                                  MockModelChatPort
                 |
                 v
EgressCheckedModelChatPort
                 |
                 v
UsageControlledModelChatPort
                 |
                 v
existing LocalModelChatAdapter
                 |
                 v
instrumentation-injected deterministic Fake Backend
```

`LocalModelChatFeatureRouter` is in `src/debug`, so it is absent from release and beta variants. Its constructor defaults to disabled; `null` selection, missing port, invalid model ID, or mismatched model ID fails closed. There is no production UI or persistent route setting. Only the Android test composition injects and enables the fake route. The Demo port is never used as a fallback for a rejected local selection.

The Android test composition reuses the existing `LocalModelChatAdapter`, authorization registry, Egress wrapper, Usage wrapper, catalog/router and ChatViewModel factory. `RoutedModelChatPort` now passes the configured `requestedOutputTokens` into model selection so catalog capability checks use the active routing policy. The `LocalModelChatAdapterTest` suite already exercises proof expiry/replay/binding, usage outcomes, cancellation, timeout and close/unwind races; the new instrumentation test adds the ChatViewModel-to-fake-backend boundary. No existing adapter lifecycle code was duplicated.

The generated fixture is a five-byte inert file in the app cache, with its exact size, SHA-256, canonical path and fixture model identity supplied to the existing adapter. The Fake Backend records load/generate/cancel/close, returns a constant response, and has no network or native-loading implementation.

## Consent and accounting boundary

The fake test composition constructs:

```text
RoutedModelChatPort(
  EgressCheckedModelChatPort(
    UsageControlledModelChatPort(LocalModelChatAdapter)
  )
)
```

The test's one-time approval hook issues a proof for the routed model, exact send/payload and current conversation generation. Egress consumes it against the current conversation binding before its delegate can reserve Usage. Missing consent therefore yields zero reservations and zero backend loads. The successful send settles the Usage ledger; cancellation during generation follows the existing `OUTCOME_UNKNOWN` contract.

No Gmail/Calendar-originated data is used. The Android process-death harness retains the issued registry/proof in a Debug-only process singleton and stores only a synthetic proof identifier in the disposable isolated package. After host `force-stop` and relaunch, it first requires the singleton to be empty (so a second broadcast without a process restart fails), then verifies that a fresh in-memory registry rejects the reconstructed pre-death proof and a fresh default-off route rejects Local Model selection. This is a real OS process boundary in the existing emulator harness; it does not simulate actual Activity/process death with a ViewModel-only test.

## Test inventory

| Test surface | Coverage | Formal set |
|---|---|---|
| `LocalModelChatFeatureRouterTest` | Default-off, explicit fake selection, rejected IDs, absent injected port, no Demo fallback | Android JVM unit suite |
| Existing `LocalModelChatAdapterTest` | Integrity, typed consent, Egress, Usage, cancellation, load/generate/close races, timeout and backend release | Android JVM unit suite |
| `LocalModelChatViewModelIntegrationAndroidTest` | ViewModel factory injection, one fake response and settled Usage, missing-consent no-dispatch, cancellation/no assistant response | Focused API 26/35 only; explicitly listed outside formal 105-test set |
| Existing Chat screen tests | Normal Demo launch and ordinary conversation/history retention through Activity recreation | Formal 105-test set |
| Existing OS process-death harness, extended here | Pre-death proof invalidation and fresh route default-off after force-stop/restart | Existing API 26/35 instrumentation campaign; no new Formal JUnit ID |

The three G10-A Android test identities are in `serea-non-required-test-identities.txt`; the focused runner opts into them explicitly. The formal instrumentation runner excludes their class when no explicit focused class filter is provided. The existing 105 formal test identities and counts are unchanged.

## Execution evidence

Evidence must be populated from CI for the final PR HEAD. Do not count a skipped test as a pass.

| Check | Result | Evidence |
|---|---|---|
| `git diff --check` | PASS (local) | implementation worktree |
| Shell syntax for focused/process-death scripts | PASS (local) | `bash -n` |
| Gradle JVM/unit tests | NOT_RUN locally | Gradle 8.14.6 wrapper download failed with connection refused |
| API 26 G10-A focused instrumentation | PENDING CI | Must include all three named Android tests and JUnit XML |
| API 35 G10-A focused instrumentation | PENDING CI | Must include all three named Android tests and JUnit XML |
| API 26 / API 35 Formal instrumentation | PENDING CI | Existing 105-test set; process-death harness includes consent check |
| Required checks 4/4 | PENDING CI | Must match final PR HEAD SHA |
| Independent review | PENDING | Must be recorded against final PR HEAD; resolve material findings |

No local emulator/device is configured in this workspace. Local Gradle/JUnit, API 26/35 instrumentation, and process-death results are not claimed as PASS.

## G10-A limits and G10-B proposal

This spike proves only test composition and security/lifecycle boundaries with a Fake Backend. It does not prove model runtime, ARM64 device inference, actual Android ABI compatibility, memory/thermal performance, Android memory-trim eviction, reload after eviction, or owner consent UI. The adapter test passes an ABI supported by the fake fixture; it is not device-ABI evidence.

G10-B should add only a reviewed runtime lifecycle bridge after the owner selects and approves a specific model/runtime/license and a disposable ARM64 test device is available. Required work should include a concrete eviction API, cancellation/close serialization, fresh consent after eviction, memory/thermal measurement, and API 26/35 regression gates. Do not add weights or activate real inference as part of this spike.
