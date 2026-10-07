# ADR-0032: Model dispatch durability, ambiguity, and accounting V1

- Status: **Accepted**
- Date: 2026-10-07
- Architecture version: `serea-arch/2.5.0`
- Affected surfaces: `serea.model/1`, `serea.event/1`, and `serea.bounds/1` unchanged
- Owners: Model Router, Storage, Event Bus, Core

## Context

SQLite can record Serea's intent but cannot atomically commit a remote provider
side effect. P4 therefore needs an explicit per-dispatch identity, crash
recovery state, conservative spend reservation, and response durability without
claiming exactly-once model calls. This ADR freezes the minimum conceptual
contract for future migration 0003; it does not create that migration.

## Identity and external-call guarantee

One `RequestId` identifies exactly one provider dispatch attempt. A normal
fallback, repair attempt, or fresh caller retry always receives a new
RequestId. A RequestId is never reused for a second network dispatch after any
uncertainty. P4 makes no exactly-once model-call guarantee. A committed
dispatch intent proves only that Serea durably intended this exact attempt;
`MODEL_CALLED` does not prove provider receipt, inference, billing, or response.

The semantic attempt states are `DISPATCH_INTENT`, `COMPLETED`, `FAILED`, and
`AMBIGUOUS`; exact Rust/SQL spelling may follow project conventions. No durable
PREPARED state is required. Before intent commit there is no network call. On
restart, an unresolved DISPATCH_INTENT becomes AMBIGUOUS. It is never
automatically redispatched and never automatically followed by a fallback.
The caller receives a typed ambiguous-dispatch failure. Any later upper-layer
choice to start a new logical call is outside automatic P4 recovery and uses a
new RequestId and budget.

Existing `ModelError.retryable = true` means the adapter knows a fresh attempt
is semantically safe. It is false for ambiguous outcomes. Ambiguity uses a
stable kind such as `AMBIGUOUS_DISPATCH`; no control flow inspects diagnostic
message text.

## Durable intent and outcome transactions

Before dispatch, one SQLite transaction atomically validates eligibility,
enforces the one-in-flight-call-per-non-null-TaskId rule, reserves/increments
model-call budget, reserves daily spend, writes the attempt, appends
`MODEL_CALLED`, and commits. Only after commit may `provider.generate()` begin.

For a definite retryable failure from the initially selected normal model, the
preferred fallback transaction marks the first attempt FAILED, records its
failure event, creates the next attempt with a new RequestId, records
`MODEL_FALLBACK`, reserves fallback budget/spend, records fallback
`MODEL_CALLED`/dispatch intent, and commits before dispatch. This prevents
recovery from losing a deterministic fallback decision. If architecture review
finds this cannot safely be one transaction, that exact alternative must be
documented before P4B.

After a valid successful response, one transaction persists the COMPLETED
attempt, accepted response blob reference, model_usage row, actual cost
settlement, `MODEL_COMPLETED`, and applicable budget counters before success is
returned. Caller response loss is recoverable from the durable result. Definite
failure records FAILED, typed error metadata, trustworthy cost/usage settlement
only where available, and `MODEL_FAILED`. Ambiguity records AMBIGUOUS and
stable failure metadata while retaining the full spend reservation; it creates
no fabricated usage row. Free-text error messages are diagnostics only.

Provider responses are untrusted. Exact `request_id`, selected `model_id`, and
selected `provider_id` equality is required. Mismatch fails closed and is
persisted as provider-protocol failure; content is not accepted and spoofed
identity cannot affect routing. Host roster/price snapshot owns cost_class.
A provider usage cost_class mismatch is invalid metadata; accounting uses the
host snapshot.

## Transactional bounds and cancellation

Every committed DISPATCH_INTENT consumes one call-budget unit, even if the
provider fails, the response is lost, or recovery marks it AMBIGUOUS. It is
never refunded. P4 V1 serializes active dispatches per TaskId; another attempt
for the same task receives typed busy/refusal. Before dispatch, known persisted
task token use at or above the configured bound refuses. After a trustworthy
response, usage is persisted atomically; reaching or exceeding the token bound
returns an explicit bound outcome. Prompts/output are never silently shortened.

Only trustworthy returned token counts enter task token totals. Repair and
fallback counts are included. Unknown ambiguous usage is never fabricated and
cannot enable an automatic retry loop. Caller cancellation and effective
deadline are checked before a new intent. Once dispatch begins, cancellation
does not erase call count, reservation, returned usage, or cost. A late valid
response is durably accounted even if the caller declines its result; no
fallback or repair starts after cancellation. P4 does not promise remote
provider cancellation. A timeout after dispatch is AMBIGUOUS unless the
adapter can prove non-processing.

## Fixed-point money and price snapshots

Durable money uses non-negative integer USD_MICROS (`1 USD = 1,000,000
micro-USD`) represented by a checked integer suitable for SQLite signed
INTEGER. Multiplication uses wider intermediates; overflow fails closed. No
floating-point money is persisted.

Actual cost is computed with integer arithmetic:

```
ceil(input_tokens * input_rate_microusd_per_million / 1_000_000)
+ ceil(output_tokens * output_rate_microusd_per_million / 1_000_000)
```

Each component rounds upward to a micro-USD. Trusted immutable host price
configuration is keyed by `(provider_id, model_id)` and includes cost_class,
input/output microusd-per-million-token rates, and price_revision. It has no
web lookup or hot reload. Each attempt snapshots exact rates and revision.
Every enabled model requires a price entry. FREE requires both rates zero;
contradiction or unknown price is startup/configuration failure. Provider
reported price is never authority.

Daily spend uses UTC calendar days with an indexed integer day representation.
The existing `max_daily_spend_usd` bound is converted at configuration load to
its exact USD_MICROS integer value (the default `$5.00` is 5,000,000
micros); dispatch and settlement use no floating-point money.
Before each dispatch, conservative maximum charge is reserved using configured
maximum context capacity, request max_output_tokens, and trusted rates. Checked
arithmetic must ensure reservation is not smaller than the maximum permitted
dispatch charge. Conceptually it is the separately rounded-up input charge for
maximum configured context plus output charge for effective max_output_tokens,
using the settlement rates and divisor. Reservation and intent creation share
the serialized SQLite transaction; concurrent tasks cannot pass on stale
remaining spend. Exceeding
`max_daily_spend_usd` refuses before dispatch. Settled spend counts actual
cost; unresolved/ambiguous attempts count the reservation. On valid usage,
unused reservation is released logically. When usage is unknown, retain the
full reservation for the original UTC day and do not estimate usage/cost.

## Migration 0003 conceptual schema

P4 implementation is authorized to use one migration `0003`; this decision
does not create it. Minimum `model_call_attempts` semantics:

| Field | Meaning |
| --- | --- |
| `request_id` | Primary key; one provider dispatch attempt |
| `task_id` | Nullable FK to tasks, `ON DELETE SET NULL` |
| `purpose`, `model_id`, `provider_id`, `deployment_class`, `data_class_rank` | Selected host facts |
| `state` | DISPATCH_INTENT / COMPLETED / FAILED / AMBIGUOUS |
| `relation_kind` | NONE / FALLBACK / REPAIR |
| `parent_request_id`, `fallback_from_model_id` | Attempt relationship where relevant |
| `accounting_day_utc` | Indexed UTC accounting day |
| `cost_class`, `price_revision`, input/output rates | Trusted price snapshot |
| `reserved_cost_usd_micros`, nullable `actual_cost_usd_micros` | Reservation and settlement |
| nullable `finish_reason`, `error_kind` | Stable terminal metadata |
| nullable `response_blob_digest`, `response_data_class_rank` | Existing blob/content reference and classification only |
| `dispatch_intent_at`, nullable `terminal_at` | Recovery/retention timestamps |

No prompt, system, messages, tools, or response content is stored in this
table. `model_usage` has minimum semantics: `usage_id`, unique `request_id`,
nullable `task_id`, `model_id`, `purpose`, input/output tokens,
`cost_usd_micros`, cost_class, latency_ms, repair_attempts,
nullable fallback_from_model_id, and recorded_at. Its task FK is
`ON DELETE SET NULL`. Usage has no prompt/output content, content hash, user
identifier, conversation ID, device ID, title, or intent.

Recoverable accepted responses use the existing durable blob/content path when
classification permits; attempts retain only reference/classification
metadata. Raw content is never put in usage rows, events, or logs. P4 V1
refuses PRIVATE dispatch because ordinary PRIVATE durable result protection is
not complete; it neither stores PRIVATE plaintext nor pretends a digest alone
is a recoverable result.

Minimum query-driven index coverage: RequestId lookup; one active
DISPATCH_INTENT per TaskId (prefer a partial uniqueness strategy); task token
aggregation; UTC-day spend aggregation; terminal retention scan; and
nonterminal recovery scan. Add no speculative indexes. Attempt metadata uses
the minimum inherited classification required by current classification
rules. Nonterminal/AMBIGUOUS attempts required for active recovery are not
deleted merely for age while referenced by active task state. Terminal attempt
and result detail retain 30 days, subject to task/privacy deletion. Usage
detail retains 365 days; after task deletion its task_id is nulled while
non-identifying accounting remains. Add
`model_usage_retention_days = 365` to Bounds/retention contract.

## Event meaning

`MODEL_CALLED` means Serea durably committed a model dispatch intent. It does
not claim bytes reached a provider, provider acceptance, inference, billing,
or a response. `MODEL_FALLBACK` records deterministic advancement to the next
configured normal model after a definite retryable failure, with source,
destination, and stable typed reason but no model content.
`MODEL_FALLBACK_EXHAUSTED` records exhaustion of the one allowed fallback.
`MODEL_OUTPUT_INVALID` means host validation failed. `MODEL_REPAIRED` means a
repair call produced host-validated structured output; a model's claim is not
enough.

## Consequences and nonclaims

- Network dispatch remains outside SQLite; exactly-once is not claimed.
- Migration 0003 and all durable runtime behavior remain future P4B work.
- Attempt/usage retention and response persistence do not weaken ADR-0022 or
  the Data Classification matrix.

## References

- [Model Protocol](../protocols/03-model-protocol.md)
- [Bounds Protocol](../protocols/10-bounds-protocol.md)
- [Event Protocol](../protocols/06-event-protocol.md)
- [Data Classification Protocol](../protocols/09-data-classification-protocol.md)
