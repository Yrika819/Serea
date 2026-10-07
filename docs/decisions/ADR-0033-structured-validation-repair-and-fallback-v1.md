# ADR-0033: Structured validation, repair, and fallback V1

- Status: **Accepted**
- Date: 2026-10-07
- Architecture version: `serea-arch/2.5.0`
- Affected surfaces: `serea.model/1` and `serea.bounds/1` unchanged
- Owners: Model Router, Protocols, Core

## Context

The existing model contract did not define which output is authoritative,
duplicate-key handling, validation resource bounds, or the conditions under
which repair and normal fallback are safe. This decision makes structured
output validation a host responsibility and pins bounded, nonrecursive
recovery. It authorizes no runtime implementation.

## Host validation

P4 V1 uses JSON Schema Draft 2020-12. External network resolution is disabled;
remote `$ref` retrieval is forbidden. Existing `jsonschema` dependency may be
used only after implementation review confirms MSRV, license, and no unwanted
external resolution.

The Bounds Protocol adds fail-closed defaults:

| Bound | Default |
| --- | ---: |
| `max_model_prompt_bytes` | 1,048,576 |
| `max_model_schema_bytes` | 65,536 |
| `max_model_response_bytes` | 262,144 |
| `max_model_json_depth` | 64 |
| `max_model_validation_errors` | 32 |
| `max_model_validation_error_bytes` | 16,384 |
| `model_usage_retention_days` | 365 |

Schema bytes are bounded before compilation. Parsing and validation enforce JSON
depth, response bytes, and bounded diagnostics. Malformed/oversized schema is a
host configuration/request failure before provider dispatch. Malformed or
oversized output is validation failure and enters repair only when the
structured repair rules permit. Semantic JSON is never silently truncated.
Diagnostic count/bytes may be capped; diagnostics never echo the full invalid
payload or large raw fragments.

Structured output is taken from provider `content`, treated as untrusted raw
text. The host parses it with a duplicate-object-key-rejecting parser before
schema validation. Parsing directly into `serde_json::Value` through a path
that silently keeps one duplicate is forbidden. Provider
`ModelResponse.structured` is never proof of validation and is ignored or
cleared at the untrusted ingress boundary. The host creates/sanitizes the
accepted structured value only after its own parse and JSON Schema validation.
STRICT provider claims never replace host validation.

## Repair ladder

Only a schema-invalid structured output enters repair. P4 V1 allows at most two
repair provider dispatches, always to configured `gpt-oss-20b`; there is no
alternate repair chain. Each dispatch has a new RequestId, purpose
`STRUCTURED_REPAIR`, and consumes one call, its trustworthy tokens, and spend.
Each repair attempt is durably accounted like every other call.

A repair request contains only the schema, bounded invalid payload, and
bounded sanitized validator errors. It receives no full conversation, tools,
or unrelated task context. Repair payload inherits the invalid output's data
class and all ordinary egress rules. SECRET and CREDENTIAL are refused;
PRIVATE dispatch is refused in P4 V1 for the durable-storage limitation.

The repair health snapshot is taken at the beginning of the repair ladder and
reused for both allowed attempts. If `gpt-oss-20b` is degraded or unavailable,
no repair dispatch occurs and the structured operation fails validation. A
definite failed repair dispatch consumes one attempt; if one attempt remains,
the second may target the same configured repair model, with fresh policy
resolution, new RequestId, and new dispatch intent. No normal fallback runs
inside repair. Ambiguous repair dispatch stops immediately; no duplicate repair
copy is sent. After two repair dispatch attempts total, the result is hard
validation failure. `MODEL_REPAIRED` is emitted only after a repair output
passes host validation; `MODEL_OUTPUT_INVALID` reports host validation
failure.

## Normal fallback

Automatic fallback occurs only when the initially selected normal model
returns a definite `ModelError` with `retryable = true`. The adapter may set
retryable true only when a fresh attempt is semantically safe. Ambiguous
outcomes are retryable false with stable `AMBIGUOUS_DISPATCH` or another
closed typed reason. Diagnostic message text never controls fallback.

Fallback does not occur for CONTENT_FILTER, LENGTH, finish_reason ERROR,
schema validation failure, repair exhaustion, ambiguous dispatch, task
cancellation, or budget exhaustion. CONTENT_FILTER terminates the logical
call. LENGTH has no fallback and no truncated-output repair; it is an explicit
validation/failure outcome. Schema invalidity enters the repair ladder and,
when exhausted, is hard validation failure; it does not create a fresh
original-request fallback. The normal chain has at most one fallback:
`max_fallback_depth = 1`, `max_fallback_chain_length = 2`. If that fallback
fails, record/emit fallback exhaustion and stop. Filter candidates against the
single health snapshot for the logical operation. A fallback attempt is a new
dispatch and must receive a fresh egress-policy snapshot before dispatch.
Budget/deadline gates can refuse it but do not reorder the chain.

## Provider response metadata

All provider results are untrusted. `request_id`, selected `model_id`, and
selected `provider_id` must exactly match the dispatch envelope. Any mismatch
is a provider-protocol failure; content is rejected and spoofed values do not
influence routing. Host configuration owns cost_class. Provider-supplied
`ModelUsage.cost_class` disagreement is invalid response metadata, while
accounting uses the trusted host price snapshot.

Validate nonnegative integer input/output token counts without overflow,
`output_tokens <= effective max_output_tokens`, a nonnegative integer
millisecond latency representation without overflow, repair relationship, and
selected identity. TokenCount remains integer. Do not fabricate usage when
unknown.

## Consequences and nonclaims

- Every structured result is host parsed and schema validated regardless of
  provider mode.
- There is no prose parsing, duplicate-key ambiguity, hidden third fallback,
  or recursive repair/fallback.
- The contract is deterministic and testable with scripted providers. No real
  model provider or network test is required for P4.

Future dev-only scripted provider support must cover success, definite
retryable and terminal errors, ambiguous errors, request/model/provider ID
mismatches, malformed JSON, duplicate keys, schema-invalid and oversized
content, CONTENT_FILTER, LENGTH, READY/DEGRADED health, fallback, repair, and
latency/usage fixtures. It must permit deterministic request capture and
health scripting. These are testkit contract needs, not runtime code in P4A.

## References

- [Model Protocol](../protocols/03-model-protocol.md)
- [Bounds Protocol](../protocols/10-bounds-protocol.md)
- [P4 preimplementation audit](../plans/P4-preimplementation-audit.md)
