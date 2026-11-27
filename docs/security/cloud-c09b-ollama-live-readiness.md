# C-09B — Ollama Cloud live readiness

Date: 2026-11-27  
Branch: `cloud/serea-v13-c09b-ollama-live-readiness`  
Base: C-09A PR #274 HEAD `64e65673f9f95948628b71d457e42795079497f2`

## Owner decision and official-source review

Ollama Cloud / Gemma 4 is the first selected provider for the Owner's personal Serea. The application API uses model ID `gemma4` at `https://ollama.com/api/chat`; the CLI example uses tag `gemma4:cloud`. These names are not interchangeable.

The official model page lists a 256K context window. The pricing page, reviewed 2026-11-27, lists USD 0.14 per million uncached input tokens, USD 0.05 per million cached input tokens and USD 0.40 per million output tokens. Its PAYG description says cloud models are charged per token. The application therefore records the published input/output prices for Usage estimates, but does not claim that its local caps ensure zero account spend. The current provider response parser reports aggregate prompt/evaluation token counts, not cached-token counts, so the ledger applies the uncached input rate to all provider-reported input tokens. This can overestimate cached-input cost; it is not a billing guarantee.

The API introduction and Cloud documentation describe account/API-key based application requests. The Terms require eligibility and authorized access, and restrict unauthorized automated access. They do not state a special blanket permission for embedding in a personal Android app. The implementation uses only the documented endpoint and bearer-key API; the Owner must accept the current Terms. This is an implementation record, not legal advice. The privacy policy says prompt/response data is processed to fulfill requests and is not used for model training; account, usage and device/network metadata may be processed. No real account or key was inspected here.

Official sources reviewed:

- <https://ollama.com/library/gemma4:cloud>
- <https://ollama.com/pricing>
- <https://ollama.com/terms>
- <https://ollama.com/privacy>
- <https://docs.ollama.com/cloud>
- <https://docs.ollama.com/api/introduction>

## Implementation

- Added a dated, expiring Gemma 4 catalog and USD price snapshot. The selected API model ID, HTTPS destination and request output ceiling are fixed. The catalog's 1024 output limit is Serea's Owner-approved cap, not a provider-published model maximum.
- Added a one-shot Cloud settings probe composed through the existing adaptive router, Egress gate, durable Usage reservation/settlement and Ollama provider registry. It sends only a fixed synthetic PUBLIC / USER_AUTHORED message, with no chat input or history. Ordinary chat remains on Demo and Cloud defaults remain disabled.
- The settings screen requires an ephemeral Owner confirmation that the account shows free Gemma 4 availability and no paid subscription, purchased credit or automatic charge. It also checks configured credentials and hardware-backed Android Keystore. The key remains entered and stored on the device; the probe sends no key in its body and does not print provider response details or credentials.
- Enforced the selected request ceilings in Usage accounting: 20 requests/day, 200/month, 4096 estimated input tokens/request, 1024 output tokens/request and one unresolved dispatch at a time. Existing lower user-configured caps remain in force; higher stored caps are bounded by these limits. These are local application limits only and cannot control account billing or enforce provider-side quotas.
- Input token count is conservatively estimated as four tokens per UTF-16 character across sent message content. No Gemma tokenizer is present, so 4096 is an estimate, not an exact tokenizer guarantee. The provider transport independently rejects requests above the estimate before credential access/network dispatch.
- HTTP 401, 402, 403/404, 429, network and timeout outcomes are distinguished without automatic retry/fallback. A post-dispatch cancellation or timeout remains `OUTCOME_UNKNOWN`; cancellation is propagated and the reservation is retained for review.
- Kept Android Chat composition and the ordinary Demo path unchanged. No live request, real API key, account access, charge, Memory enablement, OAuth, personal data, migration, or ADR-0044 ratification occurred.

## Verification

Added a MockWebServer integration test that exercises the synthetic probe through routing, Egress, Usage, and Ollama transport, including model ID and secret non-exposure. Added Usage cap/concurrency coverage. Existing adapter tests cover response parsing, failure mapping, no redirects/retries, cancellation and dispatch evidence.

The focused Gradle tests could not run in this environment. The wrapper attempted to download Gradle 8.14.6, but the configured external connection refused the request before Gradle started. This is an environment dependency-resolution failure; no product test result was produced. GitHub CI at the resulting C-09B HEAD is the next executable verification. No claim of test pass is made here.

## Owner actions before a real request

1. On the Ollama account, privately confirm that `gemma4` is available under a free allowance with remaining quota and that no paid plan, purchased credit or automatic charge will be used. If this is not clear, do not use the probe. Serea cannot verify the billing source.
2. On the exact target Android device, confirm hardware-backed Keystore support. Enter/register the API key directly in Cloud Settings; never paste it into chat, GitHub, logs or diagnostics.
3. Review the fixed endpoint, model, synthetic prompt, current Terms and Privacy policy. Select the in-screen free-use confirmation and run one synthetic probe only when ready. Do not put a real conversation into this route.
4. If the provider reports unavailable/free quota exhausted, stop. There is no paid fallback, model switch, redirect or automatic retry. Inspect Usage for `OUTCOME_UNKNOWN` after timeout/cancellation before considering a separate explicit retry.

ADR-0043's development-only boundary remains in force. ADR-0044 remains `PROPOSED / NOT RATIFIED`; this O-02 decision selects only the initial Owner-personal provider lane.
