# Cloud C-04 — Adaptive conversation context

## Scope and base

Branch `cloud/serea-v13-c04-adaptive-context` is based on C-03R HEAD `7f5e165d88348e4efb35a20e8d937b68eef10e42`. No `ModelChatRequest`, persistence, Egress Gate, Android UI, Gradle, or provider activation contract was changed. ADR-0044 remains PROPOSED / NOT RATIFIED.

## Implementation

- `RoutedModelChatPort` uses the selected descriptor and requested output budget to fit conversation history before a generated Egress contract is built and before the request enters the Egress/Usage/provider chain. Existing already-approved contracts are never silently rewritten; if fitting would change their covered context, routing rejects before dispatch.
- The router and builder share a conservative character budget: reserve requested output from the model context window, apply the optional input-token ceiling, account for the current message at four estimated tokens per UTF-16 code unit, then allocate remaining budget to history. This is an estimate, not tokenizer output. The legacy 4,096-character fixed cap remains the upper bound and the old builder API keeps its behavior.
- A missing model context limit uses the existing fixed character cap in the standalone builder. The router still rejects models with unknown context limits, so that fallback cannot authorize an unknown model for dispatch.
- History remains chronological while the newest completed messages consume budget first. Failed/incomplete turns, known other-conversation entries, and entries with unknown conversation ownership are excluded when a conversation ID is required. Trimming copies retained `ContextMessage` values and preserves role, classification, origin, source ID, conversation ID, and source lineage. UTF-16 truncation stops before a split surrogate pair.
- No tokenizer is present; character limits and input estimates are labeled/treated as estimates and this change does not expose an exact token count. No summarization or persistent memory was introduced.

## Verification

JDK 17 targeted test command:

```text
./gradlew :app:testDebugUnitTest --tests app.serea.android.ChatContextBuilderTest --tests app.serea.android.AdaptiveModelRouterTest --tests app.serea.android.CloudProviderRegistryTest --tests app.serea.android.OllamaCloudAdapterTest --tests app.serea.android.UsageLedgerCostTest --tests app.serea.android.LocalFakeRoutingAdversarialTest
```

Result: **PASS — 66 tests** before C-04R. C-04R adds regressions for unowned conversation IDs in builder/routing budgets and for refusing to rewrite already-approved Egress context. The C-04R local rerun is **NOT_RUN**: this workspace has JDK 21 only (the configured JDK 17 path is absent), and the Gradle wrapper distribution is not cached; fetching it was blocked by the execution network. A new PR workflow run is needed to verify these added tests. No real API key, provider traffic, personal data, or OAuth was used.

## Integration notes

C-04 is built on the C-03R provider-routing behavior and uses its selected output reservation unchanged. C-03R's GitHub Android Formal run is separate: the run could not execute API 26/35 tests because the downloaded APK artifact lacked the required provenance file, although Android Build/Instrumentation Compile and Android Unit passed after a Maven Central 429 retry. The original API 35 Compose IME timeout remains a local UI test issue outside this cloud change boundary.
