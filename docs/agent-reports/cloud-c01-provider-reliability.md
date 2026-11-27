# Serea v1.3 C-01 — Provider Reliability

- Base: `serea-v13-s1-01b-core-reliability` at `4269c79c7b5847edc31e27f61cec26dbc6b9a560`.
- Scope: Ollama Cloud adapter transport, response parsing, usage telemetry, and regression tests only.

## Changes

- Force redirects and connection retries off even for injected OkHttp clients. This prevents authorization forwarding and duplicate POST attempts.
- Track whether OkHttp began writing the request body. Local connection failures before that point are pre-dispatch; timeouts after it retain an unknown remote outcome.
- Strictly decode bounded UTF-8 NDJSON lines by wire-byte count; reject malformed JSON, malformed UTF-8, data after terminal completion, and oversized input.
- Preserve only nonnegative integral token counts in `Int` range. Missing or invalid provider usage remains unknown.
- Keep cancellation response closure and generic failure messages; callback exceptions do not expose their text and do not poison later sends.
- Preserve the existing Egress → Usage → Dispatch composition and default-disabled provider configuration.

## Verification and safety

- `:app:testDebugUnitTest --tests app.serea.android.OllamaCloudAdapterTest` — **PASS**, 27 tests, 0 failures, 0 errors, 0 skipped, JDK 17.
- No provider credentials or external API calls were used; tests use loopback MockWebServer fixtures only.
- ADR-0044 remains **PROPOSED / NOT RATIFIED**. No activation, roster, egress, common contract, or non-C-01 files were changed.
