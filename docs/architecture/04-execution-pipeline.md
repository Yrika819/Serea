# Execution Pipeline

Architecture version: `serea-arch/2.4.0` · Status: **FROZEN current contract set** · Ratified on 2026-10-07

This document traces one user request from utterance to durable outcome, stage
by stage, with two worked examples: a read-only task that needs no approval, and
an effecting task that does. It names the crate that owns each stage, the failure
each stage can produce, and exactly where a crash is safe, needs an idempotency
key, or needs reconciliation.

The authority flow these stages implement is
[Capability Protocol §1](../protocols/01-capability-protocol.md#1-core-principle).

---

## 1. The ten stages

> Persist before advancing. Every arrow below is a durable commit or a refusal.

| # | Stage | Owning crate | Governing protocol | Can produce |
| --- | --- | --- | --- | --- |
| 1 | Ingest and authenticate the device frame | `serea-core` | [Device §4](../protocols/07-device-protocol.md#4-sessions-and-authentication), §5 | Session refusal; duplicate drop |
| 2 | Create the task | `serea-task-engine` | [Task §2](../protocols/02-task-protocol.md#2-assistanttask) | `BOUND_EXCEEDED_CONCURRENCY` |
| 3 | Plan | `serea-task-engine` + `serea-model-router` | [Model §7](../protocols/03-model-protocol.md#7-structured-output-validation-and-bounded-repair) | `MODEL_OUTPUT_INVALID`, `MODEL_BUDGET_EXHAUSTED` |
| 4 | Validate structured output and build `ActionRequest` | `serea-task-engine` | [Model §4.1](../protocols/03-model-protocol.md#41-the-trust-boundary-stated-precisely), [Capability §4.2](../protocols/01-capability-protocol.md#42-host-resolved-fields) | `VALIDATION`, `MODEL_SCHEMA_VIOLATION` |
| 5 | Resolve the capability | `serea-capability` | [Capability §10](../protocols/01-capability-protocol.md#10-capability-registry) | `UNKNOWN_CAPABILITY`, `CAPABILITY_UNAVAILABLE` |
| 6 | Evaluate policy | `serea-policy` via `serea-capability` | [Policy §4.2](../protocols/04-policy-protocol.md#42-evaluation-order) | `POLICY_DENIED` |
| 7 | Obtain approval, if required | `serea-capability` (ledger) + `serea-core` (delivery) | [Approval §2](../protocols/05-approval-protocol.md#2-approvalrequest), §4 | `APPROVAL_REQUIRED`, `APPROVAL_DENIED`, `APPROVAL_EXPIRED` |
| 8 | Invoke the provider | `serea-capability` → `providers/*` | [Capability §9](../protocols/01-capability-protocol.md#9-provider-interface) | `PROVIDER_ERROR`, `PROVIDER_TIMEOUT`, `RATE_LIMITED`, `AUTH_EXPIRED`, `AMBIGUOUS` |
| 9 | Receipt, evidence, event, durable commit | `serea-capability` + `serea-event-bus` + `serea-storage` | [Capability §5.1](../protocols/01-capability-protocol.md#51-receipt), [Event §5](../protocols/06-event-protocol.md#5-ordering-and-delivery) | Host invariant violation → `BLOCKED` |
| 10 | Deliver to the device and advance the task | `serea-core` + `serea-event-bus` | [Device §8](../protocols/07-device-protocol.md#8-activity-timeline-feed) | Delivery deferred; task `BLOCKED` on `DEVICE_OFFLINE` |

---

## 2. Worked example A — a read-only request

> **Utterance:** "what's on my calendar tomorrow?"
> Task `tsk_01JQ8Z9K3M7QWXR4V2T6YH0BNA`, `kind: USER_REQUEST`, `policy_class: OBSERVE`.

### Stage 1 — Ingest and authenticate

```json
{
  "envelope_version": "1",
  "surface": "serea.device/2",
  "message_id": "evt_01JQ8ZK5H4NQW9T2XR7BV3M8DF",
  "correlation_id": "tsk_01JQ8Z9K3M7QWXR4V2T6YH0BNA",
  "causation_id": null,
  "issued_at": "2026-10-01T09:14:20.118Z",
  "data_class": "PERSONAL",
  "payload": {
    "kind": "CHAT_MESSAGE",
    "device_id": "dev_01JQ8ZC5N8TVG3K6MRQ2XW9JHF",
    "session_id": "ses_01JQ8ZG9V5MXK3N7QW2RTF8YHB",
    "client_seq": 4182,
    "content": "what's on my calendar tomorrow?"
  }
}
```

`serea-core` verifies the per-frame Ed25519 signature against the registered
public key, checks `issued_at` skew (±120 s), and checks `message_id` against the
session's retained inbound set. `correlation_id` is host-resolved — a device
pre-populating it is dropped as an invalid field. The task is `RECEIVED` and
committed with a `TASK_CREATED` event.

**Failure surface:** an unsigned frame, a stale frame, or a replayed
`message_id` is dropped with no event and no result. A valid frame on a device
exceeding `max_events_per_minute_per_device` is refused and the device backs off.

### Stage 2 — Task creation

```json
{
  "task_id": "tsk_01JQ8Z9K3M7QWXR4V2T6YH0BNA",
  "kind": "USER_REQUEST",
  "title": "What's on my calendar tomorrow",
  "state": "RECEIVED",
  "origin": { "kind": "USER_MESSAGE", "device_id": "dev_01JQ8ZC5N8TVG3K6MRQ2XW9JHF", "message_id": "evt_01JQ8ZK5H4NQW9T2XR7BV3M8DF" },
  "data_class": "PERSONAL",
  "policy_class": "OBSERVE",
  "attempt_budget": { "max_model_calls": 12, "max_tool_calls": 24, "max_attempts_per_step": 3 },
  "steps": [ ],
  "blocked_reason": null,
  "result_summary": null
}
```

`serea-task-engine` reads `max_concurrent_tasks` from durable state. With 8
tasks already active, this one stays `RECEIVED` with a queue position and a
`BOUND_EXCEEDED_CONCURRENCY` event — visible on the device, never silent.

`policy_class` is host-assigned from the *kind* of request, before any model
call, and is the ceiling the whole plan must fit under.

### Stage 3 — Planning

`serea-task-engine` calls `serea-model-router.route(purpose: PLANNING,
required_capabilities: [tools, structured_output: STRICT], data_class: PERSONAL)`.
Routing is deterministic and selects `nemotron-3-nano-30b` first position.
The prompt contains the redacted projection of the conversation and the rendered
view of task state; the model cannot write either.

```json
{
  "request_id": "req_01JQ8ZA4H6NFG8K2M6RTV9XCWB",
  "model_id": "nemotron-3-nano-30b",
  "task_id": "tsk_01JQ8Z9K3M7QWXR4V2T6YH0BNA",
  "purpose": "PLANNING",
  "response_format": { "type": "JSON_SCHEMA", "schema": { "type": "object", "additionalProperties": false, "required": ["actions", "complete"], "properties": { "actions": { "type": "array", "maxItems": 8, "items": { "type": "object", "additionalProperties": false, "required": ["capability_id", "arguments"], "properties": { "capability_id": { "type": "string", "maxLength": 96 }, "arguments": { "type": "object", "additionalProperties": false } } } }, "complete": { "type": "boolean" } } } },
  "max_output_tokens": 2048,
  "temperature": 0.2,
  "deadline_ms": 30000,
  "data_class": "PERSONAL"
}
```

`MODEL_CALLED` is emitted before dispatch. The model returns `structured` with
one action: `calendar.events.list` with `{"range": "tomorrow", "limit": 25}`.

**Repair ladder, if the first response fails validation:** one call to the
configured structured-repair model (`gpt-oss-20b`) given only the schema, the
invalid payload, and the validator's errors — never the conversation, never tool
definitions. Second failure → one more call with the errors appended. Third
failure → hard fail with `ActionErrorKind::VALIDATION`. `MODEL_OUTPUT_INVALID`
and `MODEL_REPAIRED` record the attempt count. No partial acceptance, ever.

### Stage 4 — Schema validation and `ActionRequest` construction

This is the mandatory non-bypassable stage between model output and anything
else. The host resolves `capability_version`, `risk_class`, `side_effect_class`,
`required_authorization`, `provider_id`, `arguments_digest`, `data_class` and
`deadline_ms`. Any of these appearing in model output is dropped and recorded as
`MODEL_SCHEMA_VIOLATION`.

```json
{
  "request_id": "req_01JQ8ZA4H6NFG8K2M6RTV9XCWB",
  "task_id": "tsk_01JQ8Z9K3M7QWXR4V2T6YH0BNA",
  "step_id": "stp_01JQ8Z9M3R2CVN8H5FWK7PQDSF",
  "capability_id": "calendar.events.list",
  "capability_version": "1.2.0",
  "arguments": { "range": "tomorrow", "limit": 25 },
  "arguments_digest": "sha256:7c9e1b4d6f8a0c2e4b6d8f0a2c4e6b8d3f5a7c9e1b4d6f8a0c2e4b6d8f0a2c4e",
  "idempotency_key": "idk_9f2c1a7e4b6d0f8a3c5e9b1d7f2a4c6e8b0d3f5a7c9e1b4d6f8a0c2e4b6d8f9a",
  "data_class": "PERSONAL",
  "requested_by": "MODEL",
  "deadline_ms": 15000
}
```

`idempotency_key` is derived from `task_id ‖ step_id ‖ capability_id ‖
capability_version ‖ canonical_json(arguments)` — from the *request*, not the
attempt, so every attempt of this step produces the same key.
`requested_by: MODEL` records provenance and confers authority zero.

The plan is persisted and the task moves `PLANNING → READY → EXECUTING`.

### Stage 5 — Capability resolution
## 5. Capability resolution

`serea-capability` looks up `calendar.events.list` at `1.2.0` in the durable
registry, validates the pinned descriptor and schema, and confirms it is not in
the disabled overlay. This is resolution only: **no lease is acquired and no
duplicate/repeated-action counter is checked before policy evaluation.**

**The host resolves the date itself.** `range: "tomorrow"` is resolved against
the host clock and the user's timezone into a concrete UTC interval *before*
schema validation of the resolved form; the model never supplies an absolute
time and never learns whether resolution succeeded beyond the step's outcome.

### Stage 6 — Policy evaluation

```text
1. disabled overlay        -> no match; continue
2. version retirement      -> 1.2.0 is current; continue
3. task policy ceiling     -> OBSERVE capability <= OBSERVE task; continue
4. context rules           -> INTERACTIVE; continue
5. data-class rules        -> PERSONAL egress is permitted; continue
6. scope rules             -> no prior grant needed for OBSERVE
7. class default           -> OBSERVE => Allow
```

Result: `PolicyDecision::Allow`. No prompt is raised. An ordinary day of
assistant use produces zero approval prompts for read-only work.

### Stage 7 — Approval

Skipped. `required_authorization` is `NONE` for `calendar.events.list`.

### Stage 8 — Pre-invocation checks and provider invocation

After policy permits the call and any required approval has been granted, the
host performs duplicate suppression, then the durable repeated-action check,
then acquires the execution lease immediately before invocation. A suppressed
duplicate returns the prior receipt without a new provider call; denied or
pending actions do not consume the repeated-action count.

`serea-capability` issues `CapabilityProvider::invoke` to
`providers/serea-provider-calendar`. The provider resolves its `CredentialHandle`
inside the credential-store boundary, uses the bytes for one call, and discards
them. Output is validated against `output_schema` — fail-closed; invalid output
is a provider fault and the step fails rather than passing a laxer result onward.

```json
{
  "request_id": "req_01JQ8ZA4H6NFG8K2M6RTV9XCWB",
  "status": "SUCCEEDED",
  "output": { "events": [ { "event_ref": "evt_01JQ8ZQ6K3V8NPT2W5RX7BHD4M", "starts_at": "2026-10-02T09:00:00Z", "busy": true } ], "next_cursor": null },
  "output_digest": "sha256:1b3f9d2a7c5e0b4d6f8a2c9e1d3f5a7b9c0e2d4f6a8b1c3e5d7f9a0b2c4e6d8f0a2c",
  "evidence": [ { "evidence_id": "evt_01JQ8ZR7M4W9QTV3X6SY8CKE5N", "kind": "CAPABILITY_OBSERVATION", "produced_at": "2026-10-01T09:14:23.902Z" } ],
  "receipt": null,
  "error": null,
  "duration_ms": 412
}
```

`receipt` is `null` and correct: `side_effect_class: NONE`. A receipt would be a
protocol violation here.

### Stage 9 — Receipt, evidence, event, commit

One transaction writes: the step row as `SUCCEEDED` with `result_digest`, the
evidence row, `CAPABILITY_COMPLETED` with `seq` assigned inside the transaction,
and the task state `EXECUTING → VERIFYING`. `model_usage` already has its row
from stage 3.

The task moves to `COMPLETED`; `TASK_COMPLETED` commits with the available
result digest. Task Engine lifecycle events do not copy result bodies into the
event payload.

### Stage 10 — Delivery

`serea-event-bus` fans the events to the device's `TIMELINE_PAGE` cursor. The app
renders them. The final assistant answer is produced by a `MODEL_TURN` step with
`purpose: CHAT`, whose `TEXT` output is rendered verbatim and never parsed for
authority — the calendar facts in the answer came from stage 8's receipt-bearing
result, not from the model.

**Total: 1 model call for planning, 1 model call for the chat turn, 1 capability
call, 0 approvals, 0 receipts required.**

---

## 3. Worked example B — an effecting action

> **Utterance:** "put a design review on my calendar tomorrow at 2pm for an hour."
> Task `tsk_01JQ8Z9N4T8WBK2H6YRD5M3XQF` is created with the host-assigned
> `policy_class: EXTERNAL_WRITE` ceiling for the requested operation. The model
> cannot change that immutable task ceiling.

### Stage 1–4 — Identical shape

Planning produces `calendar.event.create` at `1.0.0`. Validation is the same
mandatory stage. The `ActionRequest` is built with `requested_by: USER` — the
user instructed it directly in this session — but `requested_by` still confers no
authority. The step is fully planned, validated, and persisted without taking an
execution lease; leases are reserved for work about to be invoked.

### Stage 5 — Resolution and pre-invocation checks

The descriptor is `side_effect_class: EXTERNAL_WRITE`, `risk_class:
EXTERNAL_WRITE`, `replay_safety: CONDITIONAL`, `required_authorization:
SCOPED_GRANT`, `idempotency_support: EMULATED`, `max_duration_ms: 15000`.
`capability_version` is pinned at `1.0.0`. The host resolves and validates the
step but takes **no execution lease** before policy evaluation or while approval
is pending. Policy and the required approval are evaluated first; duplicate
suppression and repeated-action counting occur only after authorization, in the
frozen order from Bounds Protocol §4.2.

### Stage 6 — Policy evaluation

```text
1. disabled overlay        -> continue
2. version retirement      -> 1.0.0 current; continue
3. task policy ceiling     -> EXTERNAL_WRITE <= EXTERNAL_WRITE; continue
4. context rules           -> INTERACTIVE; continue
5. data-class rules        -> PERSONAL; continue
6. scope rules             -> no grant covers this yet
7. class default           -> EXTERNAL_WRITE => RequireApproval
```

`PolicyDecision::RequireApproval(ApprovalRequest)`. The host constructs the
`ApprovalRequest` and its `plain_summary` **from the validated arguments in host
code**. The model did not write the sentence the user is about to consent to.

```json
{
  "approval_id": "apr_01JQ8ZA1D4NFG8K2M6RTV9XCWB",
  "task_id": "tsk_01JQ8Z9N4T8WBK2H6YRD5M3XQF",
  "step_id": "stp_01JQ8Z9N5T9WXK2H4BNPQ7RDSF",
  "capability_id": "calendar.event.create",
  "capability_version": "1.0.0",
  "risk_class": "EXTERNAL_WRITE",
  "requested_scope": { "calendar": "primary" },
  "arguments_digest": "sha256:3b7e0d4f8a2c6b9e1d5f3a7c0e4b8d2f6a9c1e5b7d3f0a8c2e6b4d9f1a7c3e5b",
  "plain_summary": "Create a 1-hour event 'Design review' on your primary calendar tomorrow at 14:00.",
  "data_class": "PERSONAL",
  "raised_at": "2026-10-01T09:14:23.000Z",
  "expires_at": "2026-10-01T09:44:23.000Z",
  "status": "PENDING"
}
```

### Stage 7 — Approval grant, in sequence

No execution lease is held while awaiting the human. The validated plan and
approval request remain durable; the task moves to `WAITING_APPROVAL`, freeing
the worker and lease for other work.

| Order | Operation | Durable artifact | Idempotency property |
| --- | --- | --- | --- |
| 1 | Task transitions `EXECUTING → WAITING_APPROVAL`; pending approval is recorded | `TASK_STATE_CHANGED` + `APPROVAL_REQUIRED` in one transaction; no execution lease is retained | Recovery re-renders; never auto-grants or auto-denies |
| 2 | `serea-core` delivers `APPROVAL_REQUEST` to the device | Device session frame, deduped by `message_id` | At-least-once, safe |
| 3 | Notification posted to channel `serea.approvals`, importance `HIGH`, **inline actions: none** | — | Tapping opens the in-app screen |
| 4 | User reads the screen and taps Approve; host clamps `max_uses` and `expires_at` to the request's own bounds | — | A device cannot widen by editing JSON |
| 5 | `APPROVAL_RESPONSE` arrives signed, `decision: GRANT`, `biometric.performed: true` | Device frames deduped by `message_id` | A duplicate response is a no-op; a second `GRANT` never consumes two uses |
| 6 | Host validates all six bounds, including the exact approved `arguments_digest`, and durably mints the grant | `APPROVAL_GRANTED`, `ApprovalGrant` with `max_uses: 1`, expiry, and task binding | Grants are not transferable, exportable, or replayable |
| 7 | Before resuming execution, host revalidates the current step identity/order, capability ID and pinned version, canonical `arguments_digest`, descriptor/schema, task ceiling, current policy decision, and grant bounds/expiry. Any mismatch invalidates the approval and requires a fresh plan/request | Validation result is derived from durable state | Approval never authorizes changed arguments, reordered steps, retired capabilities, or newly denied policy |
| 8 | Task transitions `WAITING_APPROVAL → READY → EXECUTING`; the separate `WAIT_APPROVAL` step is marked `SUCCEEDED` | `TASK_STATE_CHANGED` + `TASK_RESUMED` | The effecting step retains its original `step_id` and request-derived idempotency key |
| 9 | Acquire the execution lease for the effecting step, then atomically consume the grant for that `step_id` immediately before invoking the provider | Lease and `APPROVAL_CONSUMED` record | Consumption is durable and idempotent per `step_id`; no provider call occurs before successful consumption |
| 10 | Invoke `ActionRequest` using the existing idempotency key | Step attempt and provider outcome | A crash after invocation follows the replay/reconciliation rules in §6; never blindly repeat an ambiguous effect |

The execution order is explicitly:

```text
plan and validate (no lease)
  -> policy requires approval
  -> persist WAITING_APPROVAL (no lease held)
  -> receive and validate grant
  -> revalidate exact request digest and current policy
  -> acquire fresh execution lease
  -> atomically consume the grant for this step
  -> invoke provider
```

The sequence is load-bearing: the request is planned and validated with no lease;
the lease is dropped before entering `WAITING_APPROVAL`; and after a grant, the
host revalidates current durable facts, acquires a fresh lease, then consumes the
grant immediately before invocation. A crash before grant minting leaves only a
pending request. A crash after grant minting but before consumption leaves the
unused grant available until expiry. A crash after consumption but before the
provider call does **not** permit another use or a new step: recovery resumes the
same durable step and key under the lease/replay rules. A crash after a possible
provider effect is reconciled, never blindly retried. Duplicate response frames
and repeated consumption for the same `step_id` are no-ops; later or reordered
steps cannot inherit the grant.

### Stage 8 — Provider invocation and receipt

```json
{
  "request_id": "req_01JQ8ZA4H6NFG8K2M6RTV9XCWB",
  "status": "SUCCEEDED",
  "output": { "event_ref": "evt_01JQ8ZQ6K3V8NPT2W5RX7BHD4M" },
  "output_digest": "sha256:9f2c1a7e4b6d0f8a3c5e9b1d7f2a4c6e8b0d3f5a7c9e1b4d6f8a0c2e4b6d8f9a",
  "evidence": [ { "evidence_id": "evt_01JQ8ZS8N5X0RWY4Y7TZ9DK6PQ", "kind": "PROVIDER_RECEIPT", "produced_at": "2026-10-01T09:15:04.221Z" } ],
  "receipt": {
    "receipt_id": "rcp_01JQ8ZF6X2HM8N4PRQ9TB7W3KD",
    "capability_id": "calendar.event.create",
    "idempotency_key": "idk_4b6d0f8a3c5e9b1d7f2a4c6e8b0d3f5a7c9e1b4d6f8a0c2e4b6d8f0a2c4e6b8d",
    "provider_reference": "evt_01JQ8ZQ6K3V8NPT2W5RX7BHD4M",
    "effect_summary": "Created 'Design review' on primary calendar, 2026-10-02T13:00:00Z to 2026-10-02T14:00:00Z",
    "observed_at": "2026-10-01T09:15:04.180Z",
    "replay_safe": false
  },
  "error": null,
  "duration_ms": 611
}
```

### Stage 9 — Receipt persistence, then the advance

In **one** transaction: the step row becomes `SUCCEEDED` with
`side_effect_receipt` populated and `result_digest` set; a
`CAPABILITY_RECEIPT_RECORDED` event is appended with its `seq`; the task moves
`EXECUTING → VERIFYING`. The task advances **only after** this commit. The
`VERIFY` step reads the event back through `calendar.event.read` and confirms the
event exists with the expected `starts_at`; `COMPLETED` is reached only through
that verification, not through the model's assertion that it worked.

If `status: SUCCEEDED` ever arrived with a null receipt on an
`EXTERNAL_WRITE` capability, that is a host invariant violation and the task
aborts to `BLOCKED`.

---

## 4. Enforcement points

| Stage | Enforced by | What a bug here produces | Named test obligation |
| --- | --- | --- | --- |
| 1 | `serea-core` signature + skew + dedupe checks | Unauthenticated frame processed as authenticated | D3, D4, D5 |
| 2 | `serea-task-engine` bound read from durable state | Task exceeds `max_concurrent_tasks` invisibly | B3, B16 |
| 3 | `serea-model-router` repair ladder and budgets | Unbounded repair; partial plan acceptance | Model §7.2 constraints 1–3 |
| 4 | Host-resolved field list | Model output influences routing, risk class, or approval need | C1, C3, P2 |
| 5 | `serea-capability` registry + `policy_class` ceiling | A step above the task's ceiling executes | P4, T6, C1 |
| 6 | `serea-policy` fixed evaluation order | An unmatched rule allows instead of requiring approval | P1, P3, P8 |
| 7 | `serea-capability` six-bound check including argument digest + atomic consumption | A grant authorizes two uses, or another task, or a wider scope | A1–A5, A8 |
| 8 | `serea-capability` deadline + per-step attempt ceiling | An effecting call retries blindly after `AMBIGUOUS` | C5, B12 |
| 9 | `serea-storage` single-transaction commit | An event exists without a state change, or vice versa | E3, E4, T4 |
| 10 | `serea-event-bus` cursor replay | Timeline gap or duplication across reconnect | D11, E7 |

---

## 5. Failure mapping

| Failure | `ActionErrorKind` | Task state transition | Event | User-visible outcome |
| --- | --- | --- | --- | --- |
| Model output fails schema after repair ladder | `VALIDATION` | `PLANNING → FAILED` | `MODEL_OUTPUT_INVALID` | "I couldn't produce a valid plan for that." No partial plan accepted |
| Plan exceeds task `policy_class` | — | Step refused; `PLANNING → FAILED` | `CAPABILITY_DENIED` | "That would go beyond what this task is allowed to do." |
| Capability not in registry | `UNKNOWN_CAPABILITY` | `EXECUTING → FAILED` | `CAPABILITY_REQUESTED` then `TASK_FAILED` | "That capability isn't available." |
| Capability version retired | `UNKNOWN_CAPABILITY` | `EXECUTING → FAILED` | `CAPABILITY_DENIED` with `RETIRED_CAPABILITY_VERSION` | Same |
| Backing condition absent (device offline, no root, revoked credential) | `CAPABILITY_UNAVAILABLE` | `EXECUTING → BLOCKED` (`blocked_reason: CAPABILITY_UNAVAILABLE`) | `CAPABILITY_UNAVAILABLE` | "I'll resume when the phone is back." Resumable, not terminal |
| Disabled capability | `POLICY_DENIED` | `EXECUTING → FAILED` | `CAPABILITY_DENIED` with `CAPABILITY_DISABLED` | "You've turned that off." |
| Automation context attempting a write | `POLICY_DENIED` | `PROACTIVE → FAILED` | `CAPABILITY_DENIED` with `AUTOMATED_ACTION_FORBIDDEN` | Proposal never raised |
| Approval needed, none granted | `APPROVAL_REQUIRED` | `EXECUTING → WAITING_APPROVAL` | `APPROVAL_REQUIRED` | Prompt on the device |
| User denies | `APPROVAL_DENIED` | `WAITING_APPROVAL → FAILED` | `APPROVAL_DENIED` | "Understood — I won't do that." Never auto-retried |
| Approval expires unused | `APPROVAL_EXPIRED` | `WAITING_APPROVAL → FAILED` | `APPROVAL_EXPIRED` | "The request timed out before you answered." Explicit, not a denial |
| Provider transient error, retryable, `IDEMPOTENT` | `PROVIDER_ERROR` (`retryable: true`) | `EXECUTING → EXECUTING` (attempt+1) | `CAPABILITY_COMPLETED` with `FAILED` status | None; invisible retry under the ceiling |
| Provider throttles | `RATE_LIMITED` | `EXECUTING → EXECUTING` | same | None; host backs off |
| Deadline exceeded on a read | `PROVIDER_TIMEOUT` | `EXECUTING → EXECUTING` then `FAILED` at the attempt ceiling | `CAPABILITY_COMPLETED` with `FAILED` status and the typed `PROVIDER_TIMEOUT` error | "That took too long." |
| Deadline exceeded on an effecting call | `PROVIDER_TIMEOUT` then **reconcile** | `EXECUTING → BLOCKED` if still unknown | `CAPABILITY_RECONCILED` on success | "I'm not sure whether that went through." Never guesses |
| **Effect possibly occurred** | `AMBIGUOUS` | `EXECUTING → BLOCKED` (`blocked_reason: AMBIGUOUS_EFFECT`) | `CAPABILITY_RECONCILED` or `TASK_BLOCKED` | Human resolution required. Never blindly retried |
| Credential revoked | `AUTH_EXPIRED` | `EXECUTING → BLOCKED` (`blocked_reason: CREDENTIAL_REVOKED`) | `CAPABILITY_UNAVAILABLE` | "Reconnect your Gmail account." |
| Identical action repeated past `max_identical_action_repeats` | `BOUND_EXCEEDED_REPEATED_ACTION` | `EXECUTING → FAILED` | `BOUND_EXCEEDED` with `bound_name: max_identical_action_repeats` | "I started repeating the same lookup and stopped." |
| Model call budget exhausted | `MODEL_BUDGET_EXHAUSTED` | `EXECUTING → FAILED` | `MODEL_BUDGET_EXHAUSTED` | "This took more model calls than allowed." |
| Task wall clock exhausted | `BOUND_EXCEEDED` (`bound_name: max_task_wall_clock_ms`) | `EXECUTING → FAILED` | `BOUND_EXCEEDED` with `bound_name: max_task_wall_clock_ms` | "This took too long." Counts *active* time only |
| Provider returned output failing `output_schema` | Provider fault | `EXECUTING → FAILED` | `CAPABILITY_COMPLETED` with the failure | "The provider returned something I couldn't verify." Never accepted loosely |
| `SUCCEEDED` with null receipt on an effecting call | Host invariant violation | → `BLOCKED` | `TASK_BLOCKED` with `INVARIANT_VIOLATION` | "Something went wrong internally." No effect is claimed |
| Illegal state transition attempted | Host invariant violation | → `FAILED` | `TASK_FAILED` with `INVARIANT_VIOLATION` | Internal failure; the illegal state is never persisted |
| Unrecognised task state after a version skew | — | → `BLOCKED` (`blocked_reason: UNRECOGNISED_STATE`) | `TASK_BLOCKED` | Never a crash, never a silent skip |
| Device offline while a step needs it | `CAPABILITY_UNAVAILABLE` | `EXECUTING → BLOCKED` (`DEVICE_OFFLINE`) | `DEVICE_DISCONNECTED` | Resumes on reconnect. `BLOCKED`, never `FAILED` |
| User cancels | — | any non-terminal → `CANCELLED` | `TASK_CANCELLED` | "Cancelled." Already-succeeded steps keep their receipts; nothing is implicitly undone |

Two mappings deserve emphasis. **`DUPLICATE_SUPPRESSED` is not a failure** — the
step *succeeds*, returning the prior result and its original receipt, because the
user asked for the state and the state is what they got. And **`UNAVAILABLE` is
not an error path** — it is a success-adjacent structural outcome, so the system
degrades gracefully instead of retrying a call that cannot succeed.

---

## 6. Idempotency and recovery

### 6.1 Where a crash is safe

| Window | Why it is safe | Recovery action |
| --- | --- | --- |
| Before the task row commits | Nothing durable exists | Device retries; the frame is deduped by `message_id`, then re-accepted |
| Task in `RECEIVED`, `READY`, `PLANNING` | No step in flight, no effect possible | Reload and resume |
| Task in `WAITING_APPROVAL` or `WAITING_USER` | No provider call in flight | Re-render the pending approval against the live device roster; never auto-grant or auto-deny |
| Task in `BLOCKED` | Held deliberately | Resume when the blocking condition clears |
| Terminal states | No outgoing transitions | Nothing |

### 6.2 Where an idempotency key is required

The key is derived from the request, so every attempt of a step produces the same
one, and a post-crash re-issue is recognised as the same action rather than a new
one.

| Situation | Key behaviour |
| --- | --- |
| Crash after provider success, before commit | The step re-issues with the same key. `idempotency_support: NATIVE` providers dedupe and return the stored result; `EMULATED` hosts suppress via the recorded key |
| Lease expired mid-call, step reclaimed | Same key, so a reclaim is never a second effect |
| Deliberate second execution | A **new** `StepId`, therefore a new key. The host never reuses a key to mean "again" |
| `CONDITIONAL` capability retried on a *retryable* failure | Re-issue the same durable step with the **same** key. Only after reconciliation confirms absence may the host close the old step and create a new `StepId` and key, subject to fresh policy/approval |

### 6.3 Where reconciliation is required

> Blunt retry on `AMBIGUOUS` is the single highest-severity anti-pattern in this
> system, because it converts one uncertain effect into two certain ones.

| Trigger | Reconciliation procedure | Outcomes |
| --- | --- | --- |
| `AMBIGUOUS` on `NON_REPLAYABLE` or `CONDITIONAL` | Read back through an `IDEMPOTENT` read-only capability, using the stored `provider_reference` or a natural-key lookup | **Occurred** → synthesize a receipt, `CAPABILITY_RECONCILED`, continue. **Absent** → retry is now safe only if `replay_safety` permits it. **Unknown** → `BLOCKED` with `AMBIGUOUS_EFFECT` |
| `PROVIDER_TIMEOUT` on an effecting capability | Identical to the `AMBIGUOUS` path | Same three outcomes |
| Crash between grant validation and grant consumption | Consumption is keyed to `step_id`; a repeat consumption is a no-op | Exactly one decrement, ever |
| Crash between receipt persistence and task advance | Receipt present in durable state means the effect happened | Commit the state transition; the task proceeds |
| Crash after `WAITING_APPROVAL` delivery, before any response | Approval row is `PENDING` | Re-render on reconnect; never auto-resolve |
| Stale arguments while a device was away | The stored `arguments_digest` no longer matches | Invalidate the old request, raise a new one; the user never consents to a stale preview |

### 6.4 Recovery is idempotent by construction

> Running recovery twice changes nothing the second time.

The startup sequence is: open storage and apply migrations; load non-terminal
tasks; for each `EXECUTING` task with an expired lease, reconcile the in-flight
step; for each `WAITING_APPROVAL` task, re-render against the live device roster;
emit `TASK_STARTED` per resumed task. Each of those steps is a read-then-conditionally-write keyed on durable state, so a second pass finds nothing to do.

---

## 7. Second-order cases

### 7.1 The delegated goal path

A future `DELEGATED_HOST_GOAL` task runs stages 1–10 **with no stage removed**.
The descriptor set is `host.goal.*` with `provider_id: host` and
`required_authorization: SCOPED_GRANT` on the three effecting verbs. P0 freezes
this contract only; P15 plans to exercise it through `FakeGoalLatchProvider` on a
fixed clock with no network, filesystem, or subprocess. `COMPLETED` is reached
only through a `VERIFY` step whose input is a `host.goal.result` parsed against
`output_schema` with `state: COMPLETED` — never a model assertion.

### 7.2 The proactive watcher

The watcher runs stages 3–5 only, under `purpose: PROACTIVE`, in a
`PROACTIVE_WATCHER` automation context. Policy rule 4 restricts that context to
`OBSERVE` and `LOCAL_STATE`; anything else is `Deny(AUTOMATED_ACTION_FORBIDDEN)`.
The watcher raises **no approvals at all** — it produces a `Proposal`, and the
user acts. Reaching `max_proactive_proposals_per_day` pauses it with an event, so
"it stopped volunteering" is never silent.

### 7.3 Cancellation mid-flight

Cancellation is cooperative. The in-flight provider call is aborted at the
deadline boundary and the step records whatever is known. Already-succeeded steps
keep their receipts. If a capability has an inverse, that inverse is a **new
capability** with its own policy and approval — never an implicit rollback.

---

## 8. Cross-references

- Where each stage's authority comes from: [Trust Boundaries §3](02-trust-boundaries.md#3-the-authority-model)
- Which crate owns each stage: [Crate Map §3.1](03-crate-map.md#31-ownership-of-each-protocol-contract)
- Test doubles used at every stage: [Crate Map §5](03-crate-map.md#5-test-doubles-serea-testkit-not-a-testing-module)
