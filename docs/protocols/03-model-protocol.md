# Model Protocol

Protocol ID: `PROTO-MODEL` · Surface: `serea.model/1` · Status: **FROZEN for P0**

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
  privileged model. `codex_allowed` is `false` by default and Codex is never a
  `ModelRouter` candidate. Ordinary task routing and failures never activate
  Codex; any future delegated use is a separate, explicitly gated GoalLatch
  adapter concern.
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
quirks, provider-specific retry logic — are confined to the provider crate.
The task engine sees only the types below.

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

### 3.1 `response_format`

| Type | Behaviour |
| --- | --- |
| `TEXT` | Free text for `CHAT` only. Its output is rendered to the user and is never parsed for authority. |
| `JSON_SCHEMA` | Output must validate against `schema` or the call fails. Used for planning, extraction, and tool proposals. |

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

`structured` is present only when `response_format.type == JSON_SCHEMA` and
validation succeeded. It has **not** passed capability validation at this
point — that is a separate, later, host-owned stage.

### 4.1 The trust boundary, stated precisely

Between `ModelResponse.structured` and `ActionRequest` there is a mandatory,
non-bypassable, host-only stage:

```
structured ──> schema validation ──> envelope validation ──> ActionRequest
                (already done)         (host-resolves capability_id,
                                        drops unknown fields, records
                                        MODEL_SCHEMA_VIOLATION)
```

A model's structured output is a **proposal**. It becomes an `ActionRequest`
only after the host has confirmed the capability exists, pinned its version
and risk class, computed digests, and derived the idempotency key.

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

Only `STRICT` models may be used for `PLANNING` and `EXTRACTION`. A
`BEST_EFFORT` model may serve `CHAT` and `ANALYSIS` only.

### 5.1 Initial model roster

| Model ID | Role | Capabilities | Provider | Notes |
| --- | --- | --- | --- | --- |
| `nemotron-3-nano-30b` | default assistant; planning; email/calendar analysis; memory extraction; proactive watcher | text, tools, `STRUCTURED_OUTPUT: STRICT`, fast | Ollama Cloud | Primary for nearly all work |
| `gpt-oss-20b` | strict structured-output fallback; JSON/tool-plan repair; alternate reasoning | text, tools, `STRUCTURED_OUTPUT: STRICT` | Ollama Cloud | Second position in the routing chain |
| `gemma-4-31b` | vision / screenshot interpretation | text, **vision**, tools | Ollama Cloud | Reached only when an input is an image |
| `codex` | GoalLatch-mediated local code work only | code_specialist | *(disabled in Serea)* | **Never a normal fallback** |

`codex` is registered as *known but disabled*. See §8.

## 6. Model routing

The `ModelRouter` selects a model for a call. Selection is deterministic given
`(purpose, required_capabilities, data_class, task_constraints, health)`:

1. Filter the roster to healthy models.
2. Filter by required capability flags. Vision work requires `vision: true`.
3. Filter by data class. See [Data Classification §5](09-data-classification-protocol.md#5-egress-rules).
4. Filter out disabled models and always exclude `codex`. Codex is never a
   `ModelRouter` candidate, regardless of task kind or failure state.
5. Order by the configured preference chain for that `purpose`.
6. Take the first surviving model.

No step consults the model about which model to use. No step uses provider
identity as a branch.

### 6.1 Routing is not escalation

If the primary model fails with a *retryable* error, the router may advance to
the next configured model in the preference chain. This is a **fallback**, and
it is:

- bounded by the task's `max_model_calls` budget,
- recorded as a `MODEL_FALLBACK` event with the reason,
- restricted to the configured chain, which never contains `codex`,
- never triggered by the *content* of a model response.

A model asking to be replaced with a more capable model has no effect.

## 7. Structured output validation and bounded repair

Invalid model output is an expected, routine failure, not an exception path.

### 7.1 The repair ladder

```
1. Validate against response_format.schema
2. FAIL → attempt repair
     ├─ 1st failure: one repair call to the configured
     │  structured-repair model (gpt-oss-20b), given only the
     │  schema and the invalid payload. Never the full conversation.
     │  (repair_attempts = 1)
     ├─ 2nd failure: one more repair call, with the validation
     │  errors appended. (repair_attempts = 2)
     └─ 3rd failure: HARD FAIL
3. HARD FAIL → the step fails with ActionErrorKind::VALIDATION
```

### 7.2 Hard constraints on repair

1. **Bounded.** At most `max_repair_attempts` (default 2) repair calls per
   step. The bound is host configuration, not model-tunable.
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

A model that cannot produce valid structured output **fails the task's
planning or extraction step** with `ActionErrorKind::VALIDATION`, emits
`MODEL_OUTPUT_INVALID`, and — where the task permits — may fall back to a
different configured model for a **fresh** attempt. It never proceeds with
guessed content.

## 8. Codex exclusion

`codex_allowed` defaults to `false` and is a **task-level** setting.

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
5. `codex_allowed` defaults to `false`; no normal task routing exception exists.
   Any future change to delegated adapter eligibility requires a separate
   explicit durable policy setting and audit event. It is not settable by model
   output or from the Android client.

These are testable invariants with named tests, not prose intentions.

## 9. Usage accounting and budgets

Every call records to `model_usage`: `task_id`, `model_id`, `purpose`, tokens
in/out, latency, `cost_class`, `repair_attempts`, `fallback_from`, timestamp.

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
| M7 | `codex_allowed` is task-level, host-owned, and not model- or device-settable. |
| M8 | Every model call is accounted with tokens, latency, purpose, and repair count. |
| M9 | Only `STRICT` structured-output models may serve `PLANNING` or `EXTRACTION`. |
| M10 | No test reaches a real model provider. |