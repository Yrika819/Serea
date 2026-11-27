# Cloud C-02 — Multi-provider foundation

## Scope

Implemented on `cloud/serea-v13-c02-multi-provider-foundation`, based on the identity-corrected C-01 branch at `24c8e6a1ccd4039a050ca26b6ba35a62d785001a` (itself based on `4269c79c7b5847edc31e27f61cec26dbc6b9a560`). This change introduces a provider transport seam and synthetic verification only. It does not enable a real cloud provider, approve a roster, or change ADR-0044 (`PROPOSED / NOT RATIFIED`).

## Implementation

- `CloudChatProvider` separates provider wire behavior from the shared approved Chat payload.
- `CloudProviderCapabilities` describes supported roles, streaming support, and local input/output bounds.
- `CloudProviderRegistry` resolves exactly one configured transport from the selected, fresh, authorized catalog model. Unknown/unapproved providers, missing or stale models, wrong destinations, unsupported roles, and oversized payloads fail before transport invocation. There is no registry fallback or retry.
- `OllamaCloudAdapter` implements the transport seam while retaining its fixed production endpoint, loopback-only test endpoint, no-retry/no-redirect client, bounded NDJSON parsing, and existing credential handling.
- `DirectProviderChatComposition` can compose a registry for synthetic tests; the normal production composition still creates only the Ollama adapter. Egress validation remains before Usage reservation, which remains before dispatch.

## Verification and integration boundary

JDK 17 command:

```text
./gradlew :app:testDebugUnitTest --tests app.serea.android.CloudProviderRegistryTest --tests app.serea.android.OllamaCloudAdapterTest
```

Result: **PASS** — `CloudProviderRegistryTest` 4/4 and `OllamaCloudAdapterTest` 27/27; 31 total, no failures, errors, or skips. Tests use synthetic transports and MockWebServer; no real provider, API key, user data, or OAuth was used.

`ChatEgressGate` currently allows the single `https://ollama.com/api/chat` destination. It was intentionally left unchanged. Registry routing and distinct provider destinations are verified synthetically at the provider boundary; production requests to another host remain blocked by the common gate until a separately approved, narrowly scoped gate contract change is integrated. The synthetic adapters do not grant authorization to any real provider.
