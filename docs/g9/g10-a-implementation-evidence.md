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

## G10-A-R1 cancellation race

The first post-undraft API 26 Focused run failed on the original G10-A code SHA `69a5056c611d6d0b7325175a86e221e641183ea1`: [workflow run 42358932765](https://github.com/Yrika819/Serea/actions/runs/42358932765), job `128363349094`, [API 26 Focused evidence artifact](https://github.com/Yrika819/Serea/actions/runs/42358932765/artifacts/13568943334). The JUnit XML reports 43 cases, one failure and zero errors/skips. The sole failed assertion is `cancelCalls > 0` at `LocalModelChatViewModelIntegrationAndroidTest.kt:130`; the preceding assertion that no assistant message was persisted passed.

The failure was a test synchronization race, not a production cancellation defect. `ChatViewModel.cancelCurrent()` cancels the active coroutine. The fake `generate()` completes `generationFinished` in its `finally` as cancellation unwinds. `LocalModelChatAdapter.send()` then catches that `CancellationException` and invokes `cancelActive()`. Completing `generationFinished` therefore does not mean the adapter's subsequent `cancelActive()` call has already run; the failed assertion observed that gap.

The test fake now exposes `cancellationObserved`, completed by its `cancelActive()`. After the existing bounded wait for `generationFinished`, the test waits up to 15 seconds for this explicit call notification before checking `cancelCalls`. The assistant-not-persisted assertion, `OUTCOME_UNKNOWN` Usage contract, bounded waits, and single backend close assertion remain. No production code, Egress/Usage behavior, Demo composition, timeout policy, or Formal test identity was changed.

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

Evidence below distinguishes the current PR HEAD checked for this report from the implementation commit. Do not count a skipped test as a pass.

| Check | Result | Evidence |
|---|---|---|
| Current checked PR HEAD | `4e61dc5b682b669c54b40b74d0c68b95bc100260` | Source and evidence-document commit checked by workflow run `42365498110`; Android checkout provenance records this exact source SHA |
| `git diff --check` | PASS | clean worktree and diff check on `4e61dc5` |
| Commit identity | PASS | Current PR HEAD uses `143304522+Yrika819@users.noreply.github.com`; PR HEAD identity gate passed |
| Gradle JVM/unit tests locally | NOT_RUN | Gradle 8.14.6 distribution download failed with `Connection refused`; no local emulator is configured |
| Android Unit and Build | PASS | Android workflow run `42365498110`, jobs `128382609435` and `128382609492` |
| API 26 G10-A focused instrumentation | PASS, 3 runs × 43 tests, 0 failures/errors/skips | focused workflow run `42365498132`; attempt 1 job `128382925116`, attempt 2 job `128385030388`, attempt 3 job `128387207818`; latest artifact `13571754423` |
| API 35 G10-A focused instrumentation | PASS, 3 runs × 43 tests, 0 failures/errors/skips | focused workflow run `42365498132`; attempt 1 job `128382575998`, attempt 2 job `128385028603`, attempt 3 job `128387208811`; latest artifact `13572610193` |
| API 26 Formal instrumentation | PASS, 105 tests, 0 failures/errors/skips | Android run `42365498110`, job `128384220216`, artifact `13572142580`; process-death recovery PASS |
| API 35 Formal instrumentation | PASS, 105 tests, 0 failures/errors/skips | Android run `42365498110`, job `128384220364`, artifact `13573000004`; process-death recovery PASS |
| Required gates | PASS, 4/4 | Linux fast run `42365498104`; Linux MSRV and stable full run `42365498081`; Android Emulator Required Gate run `42365498110`, job `128390717384` |
| Independent review | COMMENTED, no critical findings | PR review `5811688735` on exact HEAD `4e61dc5b682b669c54b40b74d0c68b95bc100260` |

The cancellation-race implementation is in `952edd9ceddcfca99cd8c9900d05c6501606b466`; the evidence above binds its behavior to the checked source SHA `4e61dc5b682b669c54b40b74d0c68b95bc100260`. Subsequent changes are limited to this evidence document and must be evaluated against their own PR-head checks and review before merge.

No local emulator/device is configured in this workspace. Local Gradle/JUnit, API 26/35 instrumentation, and process-death results are not claimed as PASS.

## G10-A limits and G10-B proposal

This spike proves only test composition and security/lifecycle boundaries with a Fake Backend. It does not prove model runtime, ARM64 device inference, actual Android ABI compatibility, memory/thermal performance, Android memory-trim eviction, reload after eviction, or owner consent UI. The adapter test passes an ABI supported by the fake fixture; it is not device-ABI evidence.

G10-B should add only a reviewed runtime lifecycle bridge after the owner selects and approves a specific model/runtime/license and a disposable ARM64 test device is available. Required work should include a concrete eviction API, cancellation/close serialization, fresh consent after eviction, memory/thermal measurement, and API 26/35 regression gates. Do not add weights or activate real inference as part of this spike.
