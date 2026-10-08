# Model Protocol

Protocol ID: `PROTO-MODEL` · Surface: `serea.model/1` · Status: **FROZEN current contract set** · Architecture: `serea-arch/2.6.0`

This protocol defines the seam between Serea and any language model. Its
purpose is to make model choice a **configuration** decision rather than an
architectural one: switching Ollama Cloud models, adding a fallback, or
removing a provider must not require touching the task engine, the policy
engine, or the capability registry.

---

## 1. Core principle

> The model is an untrusted, replaceable, fallible component with zero
> authority.

Consequences that hold everywhere in this protocol:

- Model output is **data to be validated**, never instructions to be obeyed.
- A model cannot choose its own tools, change its own risk class, or widen
  any scope.
- A model failure never silently escalates to a more expensive or more
  privileged model. Codex is never a `ModelRouter` candidate. Ordinary task
  routing and failures never activate Codex; any future delegated use is a
  separate, explicitly gated GoalLatch adapter concern.
- Model self-report of having performed an action is **never** evidence. Only
  provider receipts are.

## 2. `ModelProvider`

```rust
#[async_trait]
pub trait ModelProvider: Send + Sync {
    fn provider_id(&self) -> ProviderId;
    fn models(&self) -> Vec<ModelDescriptor>;

    async fn generate(
        &self,
        request: &ModelRequest,
        ctx: &ModelCallContext,
    ) -> Result<ModelResponse, ModelError>;

    async fn health(&self) -> ProviderHealth { /* default: Ready */ }
}
```

Provider implementation details — HTTP clients, prompt templates, sampling
quirks, and provider-specific behavior — are confined to the provider crate.
The Model Router and caller exchange the types below; the router has no
`serea-task-engine` dependency.

For P4 V1, `generate()` returns one completed `ModelResponse`. Streaming is
out of scope; `supports_streaming` remains descriptive and does not define a
token stream or partial UI/durability contract.

## 3. `ModelRequest`

```json
{
  "request_id": "req_01JQ8ZA4H6NFG8K2M6RTV9XCWB",
  "model_id": "nemotron-3-nano-30b",
  "task_id": "tsk_…",
  "purpose": "PLANNING",
  "messages": [ { "role": "user", "content": "…" } ],
  "system": "…",
  "response_format": { "type": "JSON_SCHEMA", "schema": { } },
  "tools": [ ],
  "max_output_tokens": 2048,
  "temperature": 0.2,
  "deadline_ms": 30000,
  "data_class": "PERSONAL"
}
```

`purpose` ∈ `CHAT`, `PLANNING`, `EXTRACTION`, `ANALYSIS`, `PROACTIVE`,
`STRUCTURED_REPAIR`.

`purpose` drives routing and accounting. It is host-assigned from the task's
phase and the step's role, never chosen by the model.

`ModelRequest` is the provider-dispatch envelope after model selection. The host
constructs it with the selected `model_id`; there is no placeholder model ID
before routing. Its messages are text-only in P4 V1. `temperature` remains a
finite host-selected `f64` under existing wire semantics. P4 rejects NaN and
either infinity before dispatch. P4 does not digest raw ModelRequest, persist a
canonical request blob, truncate/round/stringify temperature for a digest, or
introduce a new numeric range where none is frozen. SCJ-1 does not apply to
ModelRequest in P4; no model wire bump is required.

### 3.1 `response_format`

| Purpose | Format | Structured requirement |
| --- | --- | --- |
| `CHAT` | `TEXT` only | `ANY` |
| `PLANNING` | `JSON_SCHEMA` only | `STRICT` |
| `EXTRACTION` | `JSON_SCHEMA` only | `STRICT` |
| `ANALYSIS` | `JSON_SCHEMA` only | `ANY` |
| `PROACTIVE` | `JSON_SCHEMA` only | `STRICT` |
| `STRUCTURED_REPAIR` | `JSON_SCHEMA` only | `STRICT` |

An illegal combination is refused before provider dispatch. `ANY` permits a
BEST_EFFORT or STRICT provider; host JSON Schema validation remains mandatory.

There is no third option. There is no "parse the prose and hope" path.

## 4. `ModelResponse`

```json
{
  "request_id": "req_01JQ8ZA4H6NFG8K2M6RTV9XCWB",
  "model_id": "nemotron-3-nano-30b",
  "provider_id": "ollama",
  "content": "…",
  "structured": { "kind": "ACTION_PLAN", "actions": [ ] },
  "finish_reason": "STOP",
  "usage": { "input_tokens": 1840, "output_tokens": 260, "cost_class": "FREE" },
  "latency_ms": 2410,
  "repair_attempts": 0
}
```

`finish_reason` ∈ `STOP`, `LENGTH`, `CONTENT_FILTER`, `ERROR`,
`STRUCTURE_INVALID`.

Provider-supplied `structured` is untrusted and is never proof of validation.
At untrusted ingress P4 ignores or clears it. For JSON_SCHEMA, provider
`content` is the raw source; the host duplicate-key-rejecting parser and host
JSON Schema validation produce the accepted/sanitized structured value.
STRICT provider claims do not replace host validation. An accepted structured
value has not passed capability validation — that is a separate, later,
host-owned stage.

When tools are present, `tools` contains canonical internal ToolDefinitionV1
projections sorted by CapabilityId UTF-8 byte order. Each definition exposes
only version `"1"`, capability_id, title, description, and input_schema.
Visibility uses the Task-pinned generation, current live overlay, experimental
opt-in, structural validity, and at least one eligible READY implementation.
It does not filter on policy or approval; visibility grants no authority.
Provider-specific function-name conversion belongs to model adapters.

### 4.1 The trust boundary, stated precisely

Between `ModelResponse.structured` and capability preparation there is a
mandatory, non-bypassable, host-only stage:

```
structured ──> schema validation ──> closed ToolCallProposalV1 validation
                (already done)         (unknown/authority field rejects whole
                                        proposal; sanitized violation metadata)
         ──> pinned registry/schema + trusted classification
         ──> PreparedActionV1 (P5)
```

A model's structured output is a **proposal**, not an ActionRequest. The exact
internal ToolCallProposalV1 shape and PreparedActionV1 handoff are frozen by
[ADR-0035](../decisions/ADR-0035-tool-proposal-schema-and-prepared-action.md).
Undeclared/authority-bearing fields reject the entire proposal; the earlier
instruction to drop fields and continue is superseded. The host records only
TaskId, optional StepId/model RequestId, stable code, offending field names,
and count. It never records proposal values, arguments, prompt, or raw content.
P5 produces immutable PreparedActionV1, not a final executable ActionRequest.
P6 authorizes it; P8 is first permitted to invoke a provider.

## 5. `ModelCapabilities`

Capabilities are **data**, never scattered `if provider == …` branches.

```json
{
  "vision": false,
  "tools": true,
  "structured_output": true,
  "json_schema_mode": "STRICT",
  "thinking": false,
  "long_context": false,
  "fast": true,
  "code_specialist": false,
  "max_context_tokens": 131072,
  "max_output_tokens": 8192,
  "supports_streaming": true,
  "supports_seeds": false
}
```

`json_schema_mode` ∈ `STRICT` (provider guarantees schema conformance),
`BEST_EFFORT` (provider is instructed but may deviate), `UNSUPPORTED`.

Eligibility follows the purpose matrix in §3.1. Effective capabilities are
the intersection of the configured host capability ceiling and any narrower
provider advertisement.

### 5.1 Initial model roster

| Model ID | Role | Capabilities | Provider | Notes |
| --- | --- | --- | --- | --- |
| `nemotron-3-nano-30b` | default assistant; planning; email/calendar analysis; memory extraction; proactive watcher | text, tools, `STRUCTURED_OUTPUT: STRICT`, fast | Ollama Cloud | Primary for nearly all work |
| `gpt-oss-20b` | strict structured-output fallback; JSON/tool-plan repair; alternate reasoning | text, tools, `STRUCTURED_OUTPUT: STRICT` | Ollama Cloud | Second position in the routing chain |
| `gemma-4-31b` | vision | text, **vision**, tools | Host-configured | Eligible only for a future typed image-input request |
| `codex` | known but disabled | code_specialist | *(disabled in Serea)* | Never routable by ModelRouter |

This table names model roles only. Trusted host roster configuration supplies
provider and deployment facts; provider discovery cannot widen or reorder it.

`codex` is registered as *known but disabled*. See §8.

## 6. Model routing

The pre-routing host boundary is an internal `PreparedModelCallV1` (or
equivalent): a host-prepared prompt after data classification, required
redaction, purpose and format assignment, and routing-requirement construction.
Raw user text is not accepted as trusted router input. Production construction
uses a trusted host seam; a public caller-set `redaction_verified: bool` is not
proof. P4 does not implement a general redaction engine.

The typed `ModelRoutingRequirementsV1` contains exactly
`vision_required: bool`, `tools_required: bool`, `min_context_tokens: u32`,
`min_output_tokens: u32`, and `structured_requirement: ANY | STRICT`. No
arbitrary JSON, free-form constraint map, provider-specific field, or
model-authored requirement exists. The complete routing inputs are purpose,
these requirements, data class, an immutable host-resolved egress-policy
snapshot, immutable configured roster snapshot, one health snapshot, and
host-owned dispatch/budget/deadline gates. A budget or deadline may refuse a
call but cannot reorder preference.

`ModelRosterV1` is immutable trusted host configuration loaded and validated
at Core/process startup. There is no P4 V1 hot reload. Duplicate ModelId
rejects startup; one ID identifies exactly one provider/model entry. Each
entry contains host-owned model/provider IDs, `ModelDeploymentClass` (`CLOUD`
or `LOCAL`), enabled state, allowed capability ceiling, and cost class.
Deployment class is not inferred from IDs, hostname, provider discovery, or
model output. Provider discovery can confirm existence, narrow capabilities,
or make an entry unavailable; it cannot add models, add Codex, widen
capabilities, alter order/deployment/cost class, or change preference.

Normal ordered chains are `nemotron-3-nano-30b`, then `gpt-oss-20b` for CHAT,
PLANNING, EXTRACTION, ANALYSIS, and PROACTIVE. STRUCTURED_REPAIR has only
`gpt-oss-20b`. For `vision_required`, `gemma-4-31b` is the only initial
eligible model, but only for a future request surface with typed image input.
Current ModelRequest is text-only: a request needing actual image bytes is
refused, and no hidden image channel exists. Text-only calls do not route to
Gemma merely for vision. Codex is excluded from every chain and cannot become
eligible through health, errors, or model output.

Take exactly one logical health snapshot before route selection. Each
configured provider/model candidate is READY or DEGRADED; a health-read
failure is DEGRADED. Reuse the snapshot for initial selection and normal
fallback. Repair takes a snapshot at the start of its ladder and reuses it for
both repair attempts; degraded/unavailable repair model means no repair
dispatch and validation failure. Filter candidates and take the first
survivor of the explicit chain. No dynamic tie-breaking by latency, price,
usage, randomness, health score beyond READY/DEGRADED eligibility, provider
response, or model recommendation is allowed.

Budgets/deadlines reject a call without silently reordering the chain.
Fallback and repair are new dispatch attempts and require a fresh egress
policy snapshot. `ModelEgressPolicySnapshotV1` is immutable host-resolved
policy with at least `private_cloud_egress_allowed: bool`; it is not
model/provider settable and P4 does not persist policy authority. Revocation
prevents future dispatches but cannot undo an already sent request.

The data-class matrix for P4 dispatch is PUBLIC → CLOUD/LOCAL; PERSONAL →
CLOUD only from trusted prepared/redacted call and LOCAL permitted; PRIVATE →
dispatch refused in P4 V1 even if cloud policy eligibility is true, because
durable PRIVATE result protection is incomplete; SECRET and CREDENTIAL →
refused. This is a P4 fail-closed implementation limit and does not alter the
global matrix in [Data Classification §5](09-data-classification-protocol.md#5-egress-rules).

### 6.1 Routing is not escalation

Fallback occurs only for a definite `ModelError` with `retryable = true` from
the initially selected normal model. `retryable = true` means the adapter can
declare a fresh attempt semantically safe; ambiguous outcomes use
`retryable = false` and a stable kind such as `AMBIGUOUS_DISPATCH`. Control
flow never parses free-text messages. Fallback advances to the next configured
model in the preference chain. This is a **fallback**, and
it is:

- bounded by the task's `max_model_calls` budget,
- recorded as a `MODEL_FALLBACK` event with the reason,
- restricted to the configured chain, which never contains `codex`,
- never triggered by the *content* of a model response.

No fallback occurs for CONTENT_FILTER, LENGTH, finish_reason ERROR, schema
validation failure, repair exhaustion, ambiguous dispatch, cancellation, or
budget exhaustion. Fallback depth is 1 and chain length is 2. If fallback
fails, record fallback exhaustion and stop.

A model asking to be replaced with a more capable model has no effect.

## 7. Structured output validation and bounded repair

Invalid model output is an expected, routine failure, not an exception path.

### 7.1 The repair ladder

```
1. Parse provider `content` with duplicate-key rejection, then validate it
   against response_format.schema (Draft 2020-12)
2. FAIL → attempt repair
     ├─ 1st failure: one repair dispatch to gpt-oss-20b, given only the
     │  schema, bounded invalid payload, and bounded sanitized validator errors.
     │  (repair_attempts = 1)
     ├─ 2nd failure: one more repair call, with the validation
     │  errors appended. (repair_attempts = 2)
     └─ 3rd failure: HARD FAIL
3. HARD FAIL → the step fails with ActionErrorKind::VALIDATION
```

### 7.2 Hard constraints on repair

1. **Bounded.** At most `max_repair_attempts` (default 2) repair dispatches per
   structured operation. The bound is host configuration, not model-tunable.
2. **Isolated.** A repair call receives the schema, the invalid payload, and
   the validator's error list. It does **not** receive the conversation, the
   user's personal data beyond what is inside the invalid payload, or any
   tool definitions.
3. **No guessing.** If repair fails, the step fails. The host does not
   partially accept a malformed plan, does not skip unparseable actions, and
   does not "best effort" a plan into shape. A partially-understood plan is
   more dangerous than a failed one, because the user cannot tell which parts
   were understood.
4. **Deterministic validation.** Validation is host code against a
   host-defined schema. It is never delegated to a model asking "is this
   valid?"

### 7.3 Failure is a first-class outcome

A schema-invalid output enters the bounded repair ladder; exhaustion is hard
validation failure and does not start a fresh original-request fallback.
CONTENT_FILTER is terminal. LENGTH has no fallback and no repair of truncated
structured output. A repair failure consumes an attempt; a second dispatch to
the same repair model is allowed only after a definite failure and only when
one repair attempt remains. Ambiguous repair dispatch stops immediately. Every
repair has a new RequestId, is accounted as a model call, and obeys normal
data-class egress restrictions. `MODEL_REPAIRED` means host validation passed,
not that the model claimed success.

## 8. Codex exclusion

Codex is known but disabled and is never routable by ModelRouter.

1. Codex is never a `ModelRouter` candidate, and no routing chain for any
   `purpose` contains it.
2. A provider error, timeout, content filter, or other ordinary task failure
   never results in a call to Codex.
3. Codex is unreachable from the model router under any input, including direct
   model output attempting to request it.
4. Any future Codex-mediated delegated use, if separately authorized, belongs
   solely to the GoalLatch adapter contract; it is not model routing and is not
   activated by ordinary task failure. P0 has no GoalLatch provider; P15 plans
   only the offline fake. A real adapter remains unscheduled and requires
   separate explicit authorization after the readiness gate.
5. P4 does not add `codex_allowed`. Any later GoalLatch/Codex work remains a
   separate provider boundary with separate authorization.

These are testable invariants with named tests, not prose intentions.

## 9. Usage accounting and budgets

Every successful call with trustworthy usage records to `model_usage`:
`task_id`, `model_id`, `purpose`, tokens in/out, latency, host-owned
`cost_usd_micros`, host-owned `cost_class`, repair relationship,
`fallback_from`, and timestamp. Provider cost_class is not authoritative and
no f64 money is durable. Each committed dispatch intent consumes call budget;
unknown usage is not fabricated. See [ADR-0032](../decisions/ADR-0032-model-dispatch-durability-and-accounting-v1.md)
for attempt durability, reservation, privacy and retention semantics.

Budgets are per task, host-enforced, and cannot be raised by the model:

| Bound | Default | Effect on exhaustion |
| --- | --- | --- |
| `max_model_calls` | 12 | Task → `FAILED`, reason `MODEL_BUDGET_EXHAUSTED` |
| `max_output_tokens` (per call) | 2048 | Call truncated, `finish_reason: LENGTH` |
| `max_repair_attempts` | 2 | Step → `VALIDATION` failure |
| `max_fallback_depth` | 1 | Next model failure ends the step |

See [Bounds Protocol](10-bounds-protocol.md) for the full set.

## 10. Determinism and testing

- The `ModelProvider` trait takes no ambient state. All variability —
  temperature, seed, the model roster, the clock, the provider's responses —
  is injected.
- `MockModelProvider` scripts exact responses, including malformed JSON,
  schema-violating objects, injected `finish_reason` values, and transport
  errors. It is the primary tool for every failure-path test in the system.
- No test may reach a real model provider. Real-provider tests are a separate,
  explicitly credentialed suite that never runs in untrusted CI.

## 11. Invariants summary

| # | Invariant |
| --- | --- |
| M1 | Model output never causes an effect without passing schema validation, registry lookup, policy, and approval. |
| M2 | Provider identity never appears as a branch in the task engine, policy engine, or capability registry. |
| M3 | Model self-report is never accepted as evidence of an action. |
| M4 | Structured output repair is bounded, isolated, and ends in explicit failure. |
| M5 | No routing chain, for any purpose, reaches `codex`. |
| M6 | Model budget exhaustion fails the task explicitly; it never degrades to a privileged model. |
| M7 | P4 has no `codex_allowed`; Codex remains outside every ModelRouter chain. |
| M8 | Every model call is accounted with tokens, latency, purpose, and repair count. |
| M9 | The purpose/format/structured-requirement matrix in §3.1 is enforced before dispatch. |
| M10 | No test reaches a real model provider. |
| M11 | Provider content is duplicate-key-rejected and host schema-validated; provider `structured` is not validation proof. |
| M12 | One RequestId identifies one provider dispatch attempt; ambiguity is not automatically retried. |

## 12. P4A contract closure

The P4 V1 routing, dispatch, validation, repair, fallback, accounting, and
durability contracts are recorded in Accepted [ADR-0031](../decisions/ADR-0031-model-roster-routing-and-egress-v1.md),
[ADR-0032](../decisions/ADR-0032-model-dispatch-durability-and-accounting-v1.md),
and [ADR-0033](../decisions/ADR-0033-structured-validation-repair-and-fallback-v1.md).
They advance the architecture contract set to `serea-arch/2.5.0` without
changing `serea.model/1`. Acceptance closes architecture decisions only; P4
runtime remains unstarted.
