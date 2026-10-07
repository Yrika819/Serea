# P4 Preimplementation Audit — Model Router

**Mode:** preimplementation architecture audit; no P4 runtime authorized  
**Base:** `f6fe74cb68c643250e15dba4adbce3e2445d920a` (`serea-arch/2.4.0`)  
**Audit branch:** `p4/preimplementation-audit`  
**Audit date:** 2026-10-07  
**Result:** `BLOCKED_PENDING_OWNER_DECISION`

This is a docs-only audit. No router crate, provider, migration, dependency,
real model call, P5 behavior, or P3 runtime change is included. P3 is closed at
the stated base. The workspace at that base contains Protocol, Storage, Event
Bus, Task Engine, Scheduler, and testkit; the Crate Map describes the router
and later crates as planned architecture, not existing runtime.

## 1. Audit method and source inventory

The exact main SHA was fetched and verified before the audit branch was created.
The branch starts at that SHA. The worktree was clean. The repository's commit
identity guard passed before edits using a repo-local GitHub noreply identity.
Review proceeded in passes: frozen protocol text; accepted ADRs and current
architecture; actual Rust declarations and testkit; P3 event/storage behavior;
then cross-contract gaps and implementation implications.

Inventory statuses mean: **FROZEN** is an explicit current contract;
**ACCEPTED_ADR** is accepted decision authority; **CURRENT_RUNTIME** is code
that exists now; **HISTORICAL** is a superseded or phase-specific account;
**PROPOSED** is an audit recommendation only; **NONCLAIM** is explicitly
outside current guarantees; **AMBIGUOUS** lacks a complete contract;
**CONTRADICTORY** has incompatible requirements or representations.

| Status | Source | Semantic meaning and owner | Implementation consequence | Required future evidence |
|---|---|---|---|---|
| FROZEN | Model Protocol §§1–2, 4–5; `serea-protocol` owns wire types/port | Model is untrusted, has zero authority; provider port is `generate` plus descriptors/health | Router validates every provider result and never grants actions | Spoofed identity, malformed output, and authority-boundary tests |
| FROZEN | Model Protocol §§6–8; ADR-0003 accepted | Deterministic host routing; strict structured models for planning/extraction; Codex excluded; fixed initial roster/roles | Router owns selection and fixed fallback/repair routing; no provider branches | Purpose matrix, tie-break, degraded health, and Codex-unreachability tests |
| FROZEN | Data Classification §§1–6; `serea-protocol` types + host at egress | Unknown class denies; class composes by maximum; PERSONAL/PRIVATE redaction; SECRET/CREDENTIAL never egress | Router must evaluate actual prepared prompt class and destination class before dispatch | Redaction/egress matrix and derivation/inheritance tests |
| FROZEN | Bounds §§2, 2.1–2.3, 7; Core owns config, caller enforces at call site | Calls, turns, token, repair, fallback, wall-clock, and daily-spend bounds; durable checks | Reservation and accounting must be transactional; repairs/fallback consume calls/tokens | Boundary, restart, task/global concurrency, UTC-day and budget-exhaustion tests |
| FROZEN | Event Protocol §§2–3, 5; Event Bus owns append/sequence | `MODEL_CALLED` is before dispatch; events and described state commit atomically; event append is at-least-once to consumers | Commit an intent/event before external call; event cannot certify dispatch/receipt | Crash fault injection at all external-call windows; payload/privacy checks |
| FROZEN | Task Protocol §§2, 4–8; Task Engine owns task lifecycle | TaskId, durable task state, attempt budget, lease, cancellation, task deletion; Task is distinct from Goal | Task Engine/Core resolves and passes narrow immutable call context; router does not depend on engine | Cross-component transaction and task-deletion tests |
| FROZEN | Protocol Index §§4–6; SCJ-1 / ADR-0019 | Canonical digest domain is integers-only; `ModelRequest.temperature` remains `f64` | Raw request cannot be claimed SCJ-1 canonical when fractional temperature is present | Refusal tests for fractional/exponent values on any future digest path |
| ACCEPTED_ADR | ADR-0003 | Frozen roster roles: Nemotron default/planning/analysis/extraction; GPT-OSS fallback/repair; Gemma vision; Codex disabled | Defines role intent, but does not completely spell all purpose chains or tie rules | Exact complete preference chain and roster lifecycle tests |
| ACCEPTED_ADR | ADR-0004 | AssistantTask and delegated Goal are distinct owners/lifecycles | Router carries task identity only for accounting/correlation; no delegation semantics | No GoalLatch dependency/reachability test |
| ACCEPTED_ADR | ADR-0011 | Only offline GoalLatch fake is planned at P15; real adapter gated; Codex remains false | P4 cannot add Codex or delegation behavior to fill absent task setting | Static dependency/chain exclusion and absence-of-real-provider checks |
| CURRENT_RUNTIME | `crates/serea-protocol/src/types.rs`, `provider.rs` | Current structs are the actual `/1` Rust shapes described in §2 below | P4 must not assume fields exist because prose names conceptual inputs | Compile-time API tests after any explicitly authorized contract closure |
| CURRENT_RUNTIME | `crates/serea-testkit/src/models.rs` | Deterministic scripted queue, fixed READY health, exact descriptors, success/error outcomes | Suitable base fake, but needs controlled health, request capture, and scenario scripting | Tests for mismatches, health, fallback/repair, latency/usage, no real network |
| CURRENT_RUNTIME | Event Bus/Storage P3 implementation and migrations 0001–0002 | Fixed transaction participants, transactional event sequence; task rows/deletion are real | Router can reuse existing event and transaction machinery; no second audit log | SQLite rollback, multiple-Store contention, reopen/recovery tests |
| CURRENT_RUNTIME | Cargo manifests | `jsonschema =0.58.3` Draft 2020-12 support exists in Protocol with remote/file resolution disabled | Prefer existing validator; no new dependency currently justified | Schema resource/size/depth/duplicate-key bounded validation tests |
| HISTORICAL | Earlier P2 planning/closure records and P3 closure phase history | P2/P3 scope and implementation records describe completed earlier phases; they establish current Store/Event Bus/Task Engine behavior but do not authorize or implement Model Router | Treat phase-specific statements as evidence of existing lower-layer contracts, not as a claim that P4 exists or as a router API | Recheck exact current `main` source and CI before depending on any historical behavior |
| CONTRADICTORY | Model Protocol §6 / ADR-0003 vs Rust `ModelRequest` and `ModelDescriptor` | Prose names `required_capabilities`, `task_constraints`, `health`; request carries no required-capability or task-constraint shape, descriptor lacks egress | No complete deterministic routing input exists | Resolve typed contract before router implementation; tests for each typed requirement |
| CONTRADICTORY | Model §5.1 / ADR-0003 vs descriptor shape | Cloud/local affects Data Classification, but descriptor has only model_id/provider_id/capabilities; provider identity branches forbidden | Router cannot mechanically enforce destination class without new authoritative information | Egress class and trusted roster-source decision; full fail-closed matrix |
| AMBIGUOUS | Model §6, ADR-0003, §6.1 and Bounds | General deterministic function is stated, but exact purpose chains and tie-breakers are absent; `gemma` “only when image” conflicts with no image request type | A deterministic runtime would invent architecture if it fills the gaps | Owner-pinned table and ordering tests for every purpose |
| AMBIGUOUS | Model §7; Event §§3.2, 3.5; Bounds §7 | No durable attempt state or external-call recovery semantics; `MODEL_CALLED` precedes an untransactional network call | Cannot claim exactly-once; unknown dispatch outcome must be represented | Owner decision on ambiguous attempts, retries, and events; crash matrix |
| CONTRADICTORY | Bounds §7.2 vs Rust `ModelUsage` | Durable table requires decimal `cost_usd`; type contains only token counts and `CostClass` | Exact cost and daily spend cannot be computed from current usage shape | Fixed-point unit, price snapshot/version, rounding, and task-deletion contract |
| AMBIGUOUS | Data Classification §5–6; no redaction service exists | Router sees `data_class`, but that does not prove prompt bytes are redacted or accurately classified | Caller must provide a trusted prepared prompt or an authorized redaction service must precede it | Define prepared-request boundary, class derivation and provenance tests |
| AMBIGUOUS | Bounds §§2, 7 and Core absent | “Per calendar day” has no timezone, reservation, unknown-cost, or concurrent-task definition | A usage-only query races and cannot reserve in-flight spend | Accounting-day and reservation policy decision; SQLite concurrency tests |
| PROPOSED | Audit recommendations in §§3–10 | Immutable host-owned roster/config and health snapshots; typed egress class; durable attempt ledger; fixed-point pricing; transactional reservations; fail-closed ambiguous recovery | Candidate contract closures only; no authority until owner accepts them into protocol/ADR | Contract tests and red-first recovery/concurrency evidence after owner ratification |
| NONCLAIM | P3 closure; Event Protocol §§5, 9 | SQLite/event atomicity says nothing about an external network side effect; no universal power-loss guarantee | No transaction can include model invocation | Explicitly document at-least-once attempt evidence and ambiguity; injected crash proof |
| NONCLAIM | ADR-0011 / GoalLatch P15 and Model §8 | No task-level `codex_allowed` field exists in current Model/Task types; normal router must always exclude Codex | Do not invent GoalLatch eligibility or task semantics | Compile/dependency/route test proves no Codex candidate |
| NONCLAIM | P3 closure / P5 not started | No capability registry; request `tools` is `Vec<Value>` | Router may forward trusted host schemas only; no tool authorization or ActionRequest conversion | Test model tool output remains inert data; no Capability/P5 dependency |
| NONCLAIM | ModelProvider trait and current plans | No real Ollama provider, service credential, or inference is part of this audit | Real provider belongs to a separately authorized provider phase after router contract closure | Scripted fake only in P4 tests; explicit real-provider gate later |

## 2. Actual Rust contract checked

`ModelRequest` currently has `request_id`, `model_id`, optional `task_id`,
`purpose`, text-only `messages: Vec<ModelMessage>`, optional `system`, closed
`ResponseFormat`, `tools: Vec<Value>`, `max_output_tokens`, `temperature: f64`,
`deadline_ms`, and `data_class`. It has no typed required-capability set,
task constraints, routing snapshot, redaction attestation, deployment class,
price version, or private-egress policy snapshot.

`ModelDescriptor` is exactly `{ model_id, provider_id, capabilities }`.
`ModelCapabilities` has booleans for vision/tools/structured output and other
features, `JsonSchemaMode`, and context/output maxima; it has no cloud/local
egress classification. `ModelPurpose` has CHAT, PLANNING, EXTRACTION, ANALYSIS,
PROACTIVE, STRUCTURED_REPAIR. `ResponseFormat` is TEXT or JSON_SCHEMA with a
schema, and its custom deserializer rejects extra fields. `FinishReason` is
STOP, LENGTH, CONTENT_FILTER, ERROR, STRUCTURE_INVALID. `ProviderHealth` is
READY or DEGRADED. `ModelUsage` contains `TokenCount` input/output and
`CostClass`; no exact cost, latency, or price version is in the Rust usage type
(the response separately has `latency_ms`). `ModelError` has typed
`ModelErrorCode`, diagnostic message, and retryable bool; prose must not drive
control flow. The `ModelProvider` port has `provider_id`, `models`, `generate`,
and `health`; `ModelCallContext` contains only deadline.

`AttemptBudget` persists max model calls, max tool calls, and per-step attempts.
It has no durable used-call counter. `RequestId` is a typed `req_` ULID-shaped
identifier but is only described as request/response correlation; it has no
provider idempotency guarantee. `codex_allowed` is not in these current types.

`MockModelProvider` scripts response or typed failure, repeats its last script
when exhausted, counts calls in memory, returns READY from health, and exposes a
caller-supplied roster. Its standard synthetic roster has only Nemotron and
GPT-OSS. It does not currently provide scripted health, captured request
history, deterministic latency, named response mismatch helpers, or dedicated
Gemma scenario support. It is a suitable base for a future test double, not a
complete P4 test harness today.

## 3. P4 authority and minimum crate boundary

**Router owns:** immutable roster snapshot and deterministic candidate
selection; strict eligibility filtering; provider dispatch coordination; host
response identity/shape validation; JSON Schema validation and bounded repair
orchestration if retained by Model Protocol; configured fallback selection;
model-call/usage bounds enforcement at its call site; durable call ledger and
model-call event participation. A routing result is a host choice, never a
model choice.

**Router does not own:** classification of source data, redaction, policy
setting persistence, capability authorization/registry, tool execution,
ActionRequest construction, Task lifecycle/leases/cancellation authority,
GoalLatch/Codex delegation, credentials, provider side effects, or real Ollama
inference. Caller/Core supplies a trusted, immutable host-resolved context with
TaskId (optional for maintenance), deadline, cancellation, constraints, task
budget ceiling, prepared prompt, and an already-resolved egress-policy view.
Task Engine owns TaskId lifecycle, budgets materialized on Task, deadline and
cancellation source; Core composes and snapshots configuration. The router
cannot depend on Task Engine.

The Crate Map explicitly plans Router → Protocol + Event Bus + Storage.
Storage is needed directly for attempt/usage and serialized counters; Event Bus
is needed for transaction-participating model events; Protocol supplies ports
and values. This is the minimum required set under current ownership. No direct
edge to Task Engine, Policy, Capability, provider crates, or testkit is allowed.
Event Bus → Router and Storage → Router are forbidden; `serea-testkit` remains
dev-only. No cycle is required. The future testkit dependency is dev-dependency
only. No P4 runtime crate exists in this audit.

## 4. Routing inputs, roster, and determinism

The purpose and data class fields exist. Health is observable from the provider
port, but no snapshot owner/time/validity rule exists. `required_capabilities`
and `task_constraints` have no frozen types. The descriptor has capability
flags but there is no frozen requirements struct, bitset, predicate language,
vision input marker, or output/context budget requirement shape. No stringly
routing criteria should be added.

Proposed minimum typed direction, pending owner choice: closed
`ModelRequirements` with explicit vision, tools-required, strict-schema-required,
context-token requirement, and output-token requirement, plus a closed
`TaskModelConstraints` containing only host policy limits and allowed purpose
response formats. `tools=true` should mean tools are required only when the
host explicitly sets that requirement; schemas in `ModelRequest.tools` alone
must not imply authorization. PLANNING and EXTRACTION force STRICT and
JSON_SCHEMA. TEXT is legal for CHAT only under current §3.1. ANALYSIS,
PROACTIVE, and STRUCTURED_REPAIR format combinations are not fully frozen;
current text says BEST_EFFORT may serve CHAT/ANALYSIS while §3.1 says TEXT is
CHAT only. Resolve the matrix before implementation.

Roster must be host-owned immutable configuration, validated and snapshotted
at Core startup. Provider discovery may verify a configured descriptor but may
not add entries or change priority. Exact duplicate ModelId and duplicate
(provider_id, model_id) behavior is not decided; recommendation is reject any
duplicate ID and any descriptor provider_id inconsistent with the provider
port. A roster version/digest, ordering, config reload semantics, and restart
semantics are also not frozen; recommendation is explicit versioned immutable
snapshot, with changes taking effect only on restart or explicit audited
snapshot replacement. No HashMap order, latency, price, randomness, or model
self-selection may determine order.

The accepted roster establishes four entries and role intent, not full ordered
chains for all six purposes. At minimum planning/extraction require strict
Nemotron then GPT-OSS as the accepted default/fallback intent; repair names
GPT-OSS; vision requires Gemma; Codex is never eligible. Those statements do
not define exact CHAT, ANALYSIS, PROACTIVE, STRUCTURED_REPAIR fallback ordering,
tie-breaks, or whether Gemma may serve non-vision work. Owner must ratify the
complete six-purpose ordered table. If equivalent candidates remain, stable
configured ordinal is the only safe tie-break, but that is a proposal.

A single health snapshot per routing decision is required. Core/Router should
collect each provider's health exactly once, bind it to a roster/config version
and decision, then evaluate candidates synchronously against that immutable
map. DEGRADED excludes all that provider's models. Health is not durable model
configuration and may change for the next decision; a fallback within one
logical operation needs explicit choice between pinned initial snapshot and a
fresh snapshot. Current contracts do not settle this.

## 5. Egress, privacy, and prepared input

**Cloud/local gap:** Data Classification §5 distinguishes cloud and local model
transit, but no descriptor field conveys that class and Model Protocol forbids
provider-name branches. A new typed deployment/egress property or an equivalent
trusted host configuration mapping is required. Options are: (a) a frozen
enum on `ModelDescriptor` (`LOCAL`, `CLOUD`, potentially future classes); (b)
a versioned roster entry property owned by Core and joined to provider/model
identity; (c) a provider attestation. Recommendation: host-owned configured
egress class included in a validated immutable roster entry, never dynamically
provider-discovered. Owner decision required; do not add a field in P4 without
contract closure.

Until a trusted destination class and resolved host setting exist, fail closed:
no model call with PRIVATE, SECRET, or CREDENTIAL input; unknown destination
class is denied. PUBLIC may egress; PERSONAL requires redacted projection;
PRIVATE requires redaction and explicit enabled host setting only for CLOUD;
SECRET/CREDENTIAL are always refused. Local models still cannot receive
SECRET/CREDENTIAL. The actual class is maximum of all prompt inputs and schema,
tools, repair payload, and derived content; caller-supplied `data_class` alone
is not proof.

`cloud_model_private_egress` is specified as durable, host-only, default false,
audited by POLICY_CHANGED, but P6 Policy runtime is absent. P4 must not persist
or own this setting. Core should resolve a fail-closed immutable policy snapshot
at call admission. If no authoritative setting provider exists, its effective
value is false. A mid-call change cannot retroactively unsend bytes; policy
snapshot timing and revocation semantics need owner decision.

Redaction happens before prompt creation; no redaction service exists in the
current runtime. Router must accept only a host-prepared request with trustworthy
transit class/provenance, or wait for the separately owned redaction service.
It must never claim `data_class` proves bytes safe. No ad-hoc router redactor.
Repair payload inherits the original invalid output class and ordinary egress
rules; no repair exemption. If privacy rules prevent sending exactly those
bytes to selected repair destination, refuse repair and follow explicit failure
policy. No raw prompts, invalid payloads, schemas, validator messages, or model
content in events or usage rows by default.

## 6. SCJ-1, request IDs, and response binding

`ModelRequest.temperature: f64` is intentionally outside SCJ-1 for fractional
values. P4 has no justified need for a request digest if attempt identity and
state can use RequestId plus typed columns. Recommendation is no ModelRequest
digest/canonical persistence in P4. If a future requirement needs idempotent
request comparison, owner must choose a separate explicit non-SCJ encoding or
future fixed-point/wire change. Never truncate, round, stringify ad hoc, or
claim the raw request is SCJ-1 canonical. No raw prompt persistence is needed
for correctness.

RequestId currently correlates request/response but does not define retry
identity. Recommendation is one fresh RequestId per network dispatch attempt;
fallback and each repair call get distinct IDs. It is not an idempotency key and
cannot make remote execution exactly once. A recovered pre-dispatch intent is
indistinguishable from a crash after bytes were sent unless stronger evidence
exists, so it must be marked ambiguous and must not be blindly resent under the
same ID. Whether a bounded new attempt is permitted after surfacing ambiguity
is an owner decision.

For every provider success, router must verify response request_id, model_id,
and provider_id exactly match selected request/provider. It must validate
finish_reason/format relation, usage range and maximums, output byte/token
limits, latency range, repair count, and content/structured relationship. The
provider cannot supply host routing metadata. `structured` from provider is
untrusted and must be parsed/validated by host even though the type comment says
validation succeeded. Fail closed on mismatch, malformed counters, invalid
format, excess output, or contradictory fields; record a typed failure without
persisting response bytes. Typed provider codes drive retry only through
explicit allowlisted codes; `message` never drives control flow. The current
`retryable: bool` and finish reasons do not define a complete retry matrix.

## 7. Durable call state, events, and crash behavior

Model network dispatch cannot share a SQLite transaction. P3 E3 still requires
event and the durable state it describes to commit atomically. Therefore
`MODEL_CALLED` can prove only: “the host durably committed an intent for this
attempt before it began dispatch.” It cannot prove a socket write, provider
receipt, inference, or billing. Preserve Event Protocol's before-dispatch
wording. The intent/call-count reservation and MODEL_CALLED must be in one
Storage transaction with Event Bus as a fixed participant; commit it before
network dispatch.

Minimum proposed state is a durable `model_call_attempts` record keyed by
RequestId, with task_id nullable/FK behavior explicit, purpose, selected
model/provider (host selected), roster/health/policy/pricing snapshot refs,
state, created/updated timestamps, attempt/fallback/repair ordinal, bounded
error code, and accounting reservation. This table is a proposal, not an
approved schema. Keep model content out. State labels PREPARED,
DISPATCH_INTENT_COMMITTED, COMPLETED, FAILED, AMBIGUOUS are conceptual only.
Attempts and usage are different populations; a dispatched call may never
return usage, so `model_usage` row count cannot prove call count.

| Window | Durable facts | Recovery consequence |
|---|---|---|
| A. Intent/event commit, crash before dispatch | Intent exists; network outcome unknown to recovery | Conservative AMBIGUOUS; no automatic resend without owner-approved bounded policy |
| B. Provider received request, crash before response | Same intent; possible cost/output | AMBIGUOUS; no exactly-once or provider reconciliation claim |
| C. Provider response received, crash before response/usage commit | Same intent; response may be lost | AMBIGUOUS; do not pretend response can be reconstructed; charge/reserve conservatively |
| D. Response/usage completion commit, caller loses response | COMPLETED and usage durable | Return/reconcile by RequestId from minimum persisted non-content result/ref if supported; otherwise caller sees completion ambiguity |
| E. Fallback decision commit, crash before next call | Failed predecessor plus durable next routing decision | Recovery resumes only from pinned decision and reserved budget; never recalculate by new health/order silently |
| F. Repair intent/response crash | Repair attempt and counts recorded like any invocation | Same ambiguity rules; no reset of repair/fallback/call counters |

Events are projections of durable transitions, not a second audit log. Fixed
transaction groups proposed: before dispatch: call attempt + reservation +
MODEL_CALLED; successful response: attempt terminal state + usage + task/global
accounting + MODEL_COMPLETED; provider error: attempt state + MODEL_FAILED;
invalid output: validation outcome metadata + MODEL_OUTPUT_INVALID; repair
success: MODEL_REPAIRED; fallback decision and next pinned route +
MODEL_FALLBACK; bounds exhaustion state + required exhaustion event. Network
execution always occurs between those transactions. Existing events have no
fully specified model payload shapes/correlation policy for attempt-level
identity; avoid content and include RequestId only if accepted by opaque-token
rules and owner-approved payload schema. Avoid duplicate MODEL_FAILED and
MODEL_OUTPUT_INVALID for one semantic failure; exact event combinations need
closure.

Retry/fallback is never exactly once. Either (1) refuse blind automatic retry
for ambiguous outcomes and surface a typed unresolved status, or (2) permit a
bounded new RequestId retry while explicitly accepting duplicate provider cost
and potentially different output. Owner decision required. Provider contracts
currently offer no outcome lookup/idempotency receipt.

## 8. Accounting, schema, privacy, and concurrency

Bounds Protocol freezes `model_usage` fields but says decimal `cost_usd`; no
SQLite decimal type exists and Rust `ModelUsage` has no cost amount. Do not use
f64. Candidate exact representation is integer micro-USD or nano-USD with
specified rounding (or exact rational numerator/denominator); owner must choose
scale, rounding, overflow bounds, and whether usage is provider-reported or
price-derived. Cost comes from configured price table, not provider bill. No
price table/version/effective date exists. To make later audits deterministic,
price/model key, table version, rate values, and computed fixed-point amount
should be snapshotted with attempt admission; changes apply only to later calls.
FREE should be explicit zero price; unknown price fails closed for spend-bearing
calls unless an owner-defined conservative estimate applies.

The frozen table needs `usage_id` (EventId), task_id, model_id, purpose,
input/output tokens, cost amount, cost_class, latency_ms, repair_attempts,
fallback_from, recorded_at. Proposed SQLite: TEXT checked IDs/enums and UTC
`INTEGER` epoch milliseconds; token counts and fixed-point cost as nonnegative
INTEGER with explicit upper checks. `task_id` should be nullable with
`ON DELETE SET NULL` because Bounds requires row survival after Task deletion.
Retained usage is non-content metadata; do not retain content hashes or output.
Need explicit retention period (no current value) and indexes only for queries:
`(task_id, recorded_at)` task tokens; `(recorded_at, usage_id)` daily spend;
unique `usage_id`; attempt lookup on `model_call_attempts.request_id`; unfinished
state index `(state, updated_at, request_id)`; fallback lineage by task/chain
key if persisted. Exact task deletion metadata that remains is purpose/model,
token counts, cost, latency, repair/fallback relation, timestamp; whether that
is sufficiently non-identifying is owner/privacy review.

`max_daily_spend_usd=5.00` has no accounting timezone. Recommend UTC day
boundaries, but owner must ratify. Usage-only accounting happens too late and
races across tasks. Use a serialized durable reservation in the same SQLite
transaction as call admission; settle against returned usage on completion,
and preserve a conservative reservation for ambiguous calls until explicitly
reconciled/expired under policy. A stale in-memory counter is forbidden. Price
snapshot version is pinned for the call. Need policy for calls costing more
than reservation and overage after completion.

Task token budget counts returned usage across calls, repairs and fallbacks, but
a call that disappears after dispatch has unknown token usage. Reservation must
include bounded max input/output tokens or a conservative estimate; prompt
size/context/token estimator and accounting of unknown usage are not frozen.
Counters that enforce max calls increment before dispatch reservation, not only
after usage; repairs and fallback count. Model turns mean plan revisions, not
calls. Per-task durable state currently has only configured ceiling, not used
count. Maximum 12 calls, 12 turns, 128000 tokens, 2048 output tokens, 2 repairs,
fallback depth 1/chain length 2 must be enforced without resetting on restart.
Task and global counters must serialize in Storage.

JSON Schema draft is 2020-12; existing `jsonschema =0.58.3` dependency disables
remote/file resolution. P4 should reuse it. No model-output schema compile
cache policy or schema size/depth/reference/regex resource bound is frozen.
`$ref` must not resolve off-process; invalid schema fails before dispatch.
Duplicate JSON keys must be detected before conversion to a map (serde Value
may erase duplicates); exact numeric semantics need preserve integer precision
using existing arbitrary precision. Unknown schema fields follow Draft 2020-12
semantics; output additional-property behavior comes from caller schema. Owner
must set schema/input/output byte, nesting, validation-time and error-count
limits before implementation. Validator errors sent to repair must be bounded
and sanitized; do not log raw errors that echo payload.

Private storage is currently incomplete for ordinary PRIVATE rows. Durable
attempt/usage metadata should be PUBLIC/PERSONAL only unless composition raises
it; never persist request, response, invalid payload, schema, or repair bytes in
P4 merely for debugging. A model call attempt references task, provider, and
model but not prompt content. P2's limits do not imply private content storage.

Concurrency: SQLite `BEGIN IMMEDIATE`/Store transaction serializes call/token
reservation, daily spend reservation, attempt state and event sequence. Network
runs outside lock/transaction. Task's one active step lease remains Task
Engine responsibility and can serialize calls per task if owner contract says
so; router cannot depend on Task Engine. Concurrent route requests must carry a
single pinned health/config snapshot. A fallback or repair cannot race a retry
for the same logical call; serialize by task/operation key stored in durable
state. Price updates affect only later snapshots. Cancellation after dispatch
does not erase usage/reservation; completion is recorded even if caller no
longer awaits it, or the attempt remains ambiguous. Deadline is host-resolved;
router enforces timeout and passes remaining deadline to provider, but
`deadline_ms` currently has no absolute-time semantics. Deadline/cancel after
dispatch needs the same ambiguity treatment as transport loss.

## 9. Repair, fallback, formats, vision, and tools

Structured output validation is host-owned and explicitly required. Current
protocol does not say exactly whether provider sends raw text, a JSON value, or
both, and `ModelResponse.structured` is described as already validated even
though provider is untrusted. Future API must clarify raw output parsing and
whether content/structured may coexist. Legal matrix currently explicit:
CHAT+TEXT allowed; PLANNING+JSON_SCHEMA and EXTRACTION+JSON_SCHEMA required;
TEXT restricted to CHAT; BEST_EFFORT allowed only CHAT/ANALYSIS; STRICT only
PLANNING/EXTRACTION. PROACTIVE, ANALYSIS+JSON_SCHEMA, and STRUCTURED_REPAIR
format details remain incomplete. Reject unsupported combinations before
network dispatch.

Repair max is two calls per step, isolated to schema, invalid payload, and
validator errors, no conversation/tools/task context. Each is a normal model
call with unique RequestId and counts toward max_model_calls_per_task,
task/global token and spend budgets, repair limit, and fallback accounting.
The protocol names GPT-OSS as repair model but does not define its unhealthiness,
repair fallback, whether the same model can repair its output, and fallback
interaction. The existing `repair_attempts` response field and usage table
column semantics differ: response appears cumulative per step; usage row says
repair calls on this step. Pin exact counts and failure/fallback rules. Invalid
output after two repairs may permit fresh fallback “where task permits”, but no
host permission input defining “permits” exists. No fallback on prose/content.

Fallback: retryable provider failure may advance configured chain; content
failure can allow fresh fallback only by the structured-output clause. There is
no exact matrix for timeout, transport, CONTENT_FILTER, ERROR finish, LENGTH,
STRUCTURE_INVALID, validation failure, or DEGRADED health. Retryable bool is
provider-controlled and insufficient without typed allowlist. No fallback on
response content. Depth is 1 and chain length 2; owner must settle whether
repair/fallback share or nest these limits. A fresh fallback is a new network
attempt with new RequestId, charged budget, durable decision, and MODEL_FALLBACK
before next call.

Vision is a capability flag and Gemma roster role, but `ModelMessage` is text
only and no image reference/blob transport exists. P4 can filter on a future
vision requirement only after request requirements are typed; image transport
is deferred. No image semantics should be invented here.

`tools: Vec<Value>` carries caller-supplied schemas. P4 may use an explicit
`tools_required` capability filter and forward host schemas, but does not
validate authorization, discover capabilities, turn model output into
ActionRequest, invoke tools, or depend on P5. `supports_streaming` is a
capability flag while `generate` returns one `ModelResponse`; no streaming
transport exists. P4 implements no streaming.

## 10. Migration 0003 and minimum storage proposal

Migration 0003 is **needed for a P4 implementation** if the frozen requirement
to durably count every attempt, recover ambiguity, and calculate usage/daily
spend remains. It is not created by this audit. Minimum proposed tables:

1. `model_call_attempts`: one row per RequestId/dispatch attempt. Non-content
   routing identity, pinned config/health/egress/price version references,
   durable state and timestamps, task/operation/fallback/repair ordinal, bounded
   typed failure code, call/token/cost reservation. Unique RequestId; indexes
   for task calls and unfinished recovery. Task deletion must not cascade away
   evidence; likely `task_id ON DELETE SET NULL` after privacy review.
2. `model_usage`: one row only when counted provider usage is durably received;
   frozen fields plus fixed-point cost and price snapshot reference. Unique
   usage ID and unique attempt RequestId/FK if contract adds it. Retained after
   task deletion with `task_id ON DELETE SET NULL`.
3. A singleton/global accounting row only if atomic reservation cannot be
   derived from attempts. Avoid a redundant table if reservation totals are
   transactionally queryable by accounting day and status.

Storage operations: insert attempt+reservation+MODEL_CALLED event; complete
attempt+usage+settlement+MODEL_COMPLETED; fail attempt+MODEL_FAILED; pin
fallback/repair next state+event; query task call/token totals; query daily
spend/reservations for accounting period; find attempt by RequestId; page
unfinished attempts for recovery; delete eligible usage per approved retention.
Indexes above map directly to those queries; no speculative indexes. Tables
hold only non-content accounting/routing metadata, subject to privacy approval.

Exact FK, retention, data class, task deletion, reservation release, price
snapshot, and event payload semantics are unresolved and must be accepted
before migration implementation. Existing task deletion cascades child rows, so
new usage/attempt FKs must explicitly avoid accidental cascade. Migration must
preserve 0001/0002 checksums and pass fresh 0001→0003, reopen, integrity, Intel
and arm64 CI.

## 11. Security review

| Threat | Existing/proposed invariant |
|---|---|
| Prompt injection attempts to change router authority | Purpose, requirements, roster and successor are host-only typed inputs; model response is data |
| Model chooses its successor | Fixed host preference chain; never parse prose/tool output for route |
| Provider descriptor spoofing or capability widening | Validate provider identity, configured roster allowlist, uniqueness and immutable startup snapshot; discovery cannot add/change capabilities |
| Response claims another model/provider/request | Exact three-field response binding; reject closed/fail-closed |
| Codex escalation | Codex absent from enabled roster and all chains; no task-level delegation switch in P4 |
| SECRET/CREDENTIAL exfiltration | Compose actual prompt class and deny both before provider dispatch for all egress classes |
| PRIVATE cloud bypass | Typed destination class + resolved default-false policy snapshot; unknown fails closed |
| Repair leakage | Repair gets only schema/payload/bounded sanitized errors; inherits data class and egress policy |
| Schema bomb / oversized prompt/output | Pre-dispatch byte/depth/schema limits and output/token bound required; currently unresolved constants |
| Spend race / retry loop | SQLite reservation and durable attempt count; finite repair/fallback/call budgets; ambiguous dispatch is not blindly retried |
| Content leaks through events/logs | Event/accounting payloads carry no prompt, output, schema, raw error, or content hashes by default |

Security mitigations requiring new fields/limits are proposals only; no field or
limit is silently ratified by this table.

## 12. Proposed sequential implementation slices (not authorized)

Every future behavior-bearing slice uses Cloud implementation and GitHub Actions
authoritative validation. No Local Mac is required; full required GitHub matrix
is Linux stable, Linux MSRV 1.85, macOS Intel, macOS arm64, release fault proof.
SQLite migration changes additionally require both Mac jobs, with no
cross-architecture SQLite artifact gate unless storage workflow changes.

| Order | Slice and exit evidence | Execution |
|---|---|---|
| P4A | Owner closes typed routing, egress, format, limits, redaction, cost, and recovery contracts; update protocol/ADR only | CLOUD; ACTIONS contract CI; LOCAL_REQUIRED NONE |
| P4B | Migration 0003 and typed Storage ledger/reservations/recovery queries, red-first crash/atomicity tests | CLOUD; ACTIONS full matrix + storage gates; LOCAL_REQUIRED NONE |
| P4C | Immutable roster/config validation, health snapshot, deterministic pure routing core, Codex unreachable | CLOUD; ACTIONS Linux stable/MSRV/Intel/arm64; LOCAL_REQUIRED NONE |
| P4D | Provider dispatch boundary, request/response binding, timeout/cancellation, durable event transitions, ambiguity recovery | CLOUD; ACTIONS full matrix/release fault proof; LOCAL_REQUIRED NONE |
| P4E | Schema validation, isolated two-call repair ladder, typed fallback matrix and accounting integration | CLOUD; ACTIONS full matrix; LOCAL_REQUIRED NONE |
| P4F | Core composition/Task Engine call-context integration and integrated crash/concurrency/security closure | CLOUD; ACTIONS full matrix and paired Mac evidence; LOCAL_REQUIRED NONE |

A real Ollama Cloud provider and credentials are not part of P4 router tests;
the Crate Map places provider implementation at a later leaf phase. P4 uses
scripted deterministic fake only. No action in this audit starts these slices.

## 13. Future red-first test plan (not written)

- **Routing:** six exact purpose chains, capability/context/output filters,
  strict-schema derivation, vision requirement, data-class eligibility,
  DEGRADED health, duplicate/tie ordering, immutable roster snapshot, Codex
  impossible.
- **Egress:** PUBLIC; PERSONAL already redacted; PRIVATE default-off and
  enabled snapshot; unknown destination; SECRET/CREDENTIAL always refused;
  inherited repair class.
- **Dispatch:** MODEL_CALLED intent commit before dispatch; fault at each A–F
  window; exact identity binding; malformed/overflow usage; timeout,
  cancellation, retryable allowlist and nonretryable failures.
- **Accounting:** call vs usage rows; call/token reservations; task and global
  ceilings; price version/rounding; UTC boundary if ratified; two concurrent
  tasks at ceiling; lost response conservative settlement.
- **Structured output:** Draft 2020-12 valid, malformed/duplicate-key/invalid
  schema, invalid payload, bounded validator errors, exactly two repairs,
  repair isolation/privacy, repair failure and allowed fresh fallback.
- **Recovery:** before network attempt, after possible provider receipt, after
  response before commit, after completion commit/caller loss, fallback intent,
  repair intent; repeated startup recovery produces no unbounded dispatch.
- **Security:** spoofed descriptors/response IDs, no dynamic capability widening,
  no content logging, bounds against huge schema/output, no model-controlled
  route or tool authority.

## 14. Owner decisions required before P4 implementation

1. **Typed routing contract:** define exact `required_capabilities` and
   `task_constraints` types, vision/tool/strict-schema derivation, context and
   output-limit participation, and purpose/response-format matrix.
2. **Egress classification:** choose host-owned deployment class representation
   and its trust/validation source; define roster version, duplicate rules,
   deterministic ordering, reload/restart and provider discovery semantics.
3. **Policy/redaction seam:** define immutable resolved
   `cloud_model_private_egress` snapshot timing/revocation; define the trusted
   prepared/redacted request boundary and proof/class derivation. Until then,
   PRIVATE is denied and no ad-hoc redaction is allowed.
4. **Deterministic chains and health:** ratify ordered chain for every purpose,
   tie-break, fallback snapshot vs fresh health, degraded behavior, and exact
   fallback trigger matrix.
5. **External-call recovery:** define one RequestId per dispatch semantics,
   ambiguous attempt recovery, automatic retry policy, caller-lost response
   behavior, and event payload/transaction groups. Exactly-once is unavailable.
6. **Usage and money:** choose fixed-point USD unit/rounding/overflow, price
   table ownership/version/key/effective semantics and snapshot, unknown/FREE
   pricing, daily accounting timezone, reservation/settlement/ambiguous charge,
   and task token reservation semantics.
7. **Durable schema/privacy:** approve attempt vs usage table fields, FK deletion,
   retention, data class, indexes, accounting row strategy, event contents and
   migration 0003 need. Do not retain prompt/output/content hashes by default.
8. **SCJ-1:** confirm no ModelRequest digest/canonical persistence in P4, or
   authorize a separate explicit encoding/future wire change; fractional f64
   remains refused by SCJ-1.
9. **Structured output/repair:** clarify provider raw-vs-structured response,
   schema resource bounds, duplicate JSON key handling, validation error
   bounding, repair retry/fallback/health, exact repair counter semantics, and
   when invalid-output fresh fallback is permitted.
10. **Integration ownership:** pin immutable Task Engine/Core context fields,
    task concurrency/cancellation/deadline and mid-call policy change behavior;
    preserve no Router→Task Engine edge.

Until these decisions are incorporated into authoritative contracts, the safe
result is to block P4 runtime work. They are not accepted by this audit.
