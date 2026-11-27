# Cloud C-03 — Routing and usage integration

## Scope

Branch `cloud/serea-v13-c03-routing-usage` is based on C-02 HEAD `12efc37c16d1db92f2f4bda0237fbd0cc310b6f8`. No common `ModelChatRequest` contract or Egress Gate change is included. ADR-0044 remains PROPOSED / NOT RATIFIED; no real provider is activated.

## Implementation

- `AdaptiveModelRouter` now uses the configured registry's provider output ceiling when evaluating candidates. Model maximum output and the caller's requested output limit remain enforced by the router; MANUAL selection never silently substitutes another model.
- The selected output limit travels in a provider-only coroutine context, avoiding a change to the shared request DTO. The same value is used for Usage reservation and the provider request. `CloudProviderRegistry` validates it against both the selected model maximum and provider maximum before transport invocation. Ollama sends that exact value as `num_predict`.
- `CloudProviderRequestContext` tracks when HTTP request headers begin leaving the client. Cancellation before that point releases the reservation; cancellation after possible dispatch keeps it as OUTCOME_UNKNOWN. Without dispatch evidence, the generic cancellation path remains conservative.
- Missing, negative, input-over-reservation, and output-over-reservation usage is recorded as USAGE_UNKNOWN with the full reservation retained. Reported usage is never clamped or guessed into a settled amount.
- C-01 token telemetry parsing now bounds decimal values before integer conversion. Extremely large exponents are rejected as unknown without expanding a huge integer.
- Existing send-ID reservation uniqueness continues to prevent a second dispatch after settlement or an uncertain result.

## Verification

JDK 17, synthetic providers and MockWebServer only:

```text
./gradlew :app:testDebugUnitTest --tests app.serea.android.CloudProviderRegistryTest --tests app.serea.android.OllamaCloudAdapterTest --tests app.serea.android.UsageLedgerCostTest --tests app.serea.android.LocalFakeRoutingAdversarialTest
```

Result: **PASS** — Cloud provider registry/routing 9/9, Ollama adapter 29/29, usage ledger 9/9, local fake routing 4/4, and adaptive router 10/10; 61 total with no failures, errors, or skips. No real API key, provider traffic, personal data, or OAuth was used.

## Integration notes

C-03 depends on C-02's provider registry and C-01's hardened Ollama transport. The common Egress Gate still approves only the existing Ollama URL; this PR does not authorize other destinations or providers. Egress-before-Usage-before-Dispatch order and durable ledger semantics remain in place.
