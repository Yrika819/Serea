# P5 Preimplementation Audit — Capability Registry and Tool Router

Status: **Historical pre-closure audit; owner decisions closed by §17**
Baseline: `528806b0117c6ff385a79a7baaed6ab508527fe6`  
Architecture: `serea-arch/2.5.0` · SQLite schema: `3`  
Branch: `p5/preimplementation-audit`

> Sections 1–16 preserve the docs-only audit as it stood at the required
> starting HEAD. Their unresolved decisions, proposed execution slices, and
> conflicting proposal behavior are historical and superseded by the accepted
> P5A closure in §17 and ADR-0034 through ADR-0036. The current owner decision
> status is `READY_FOR_P5_IMPLEMENTATION`; P5B has not started.

## 1. Scope and method

This is a preimplementation architecture/security audit. It adds no runtime,
crate, migration, policy engine, approval runtime, provider integration, or
external effect. It compares the frozen protocol set, accepted ADRs, existing
Rust types and storage, and P4 closure evidence. “Current runtime” means code
actually present at the baseline; protocol text alone does not establish a
runtime guarantee.

The key source files inspected include `types.rs`, `provider.rs`, the action
request/result JSON Schemas, storage migrations 0001–0003, event bus, model
router interfaces, testkit provider, and P4 closure. P5 runtime is absent;
`serea-capability` is a planned crate only.

## 2. P5 scope and phase boundary

### Decision

P5 may implement a registry, trusted schema catalog/compiler, deterministic
tool-definition projection, proposal validation, and host construction of a
*prepared* `ActionRequest` (or a narrower validated-action value). It may persist
descriptor history, disabled state and the exact binding needed by a future
execution. It must stop before policy/approval outcome and provider invocation.

P5 may not claim that a prepared request is executable. It must not implement
allow-all policy, implicit approval, `approved=true`, an OBSERVE exception, or
call `CapabilityProvider::invoke`. Tool proposals must not acquire authority
from being schema-valid.

The frozen invocation order is:

```
proposal shape -> registry/descriptor and argument schema -> policy
-> required approval/grant consumption -> duplicate suppression
-> repeated-action bound -> provider dispatch -> result/evidence/receipt commit
```

This is the order in Capability Protocol §1 and Bounds Protocol B8. Since P6
owns policy and approval and P5 precedes P6, P5 cannot execute the tail of this
sequence. A registry-only P5 boundary is valid; an execution router is not
complete until P6 supplies a typed authorization handoff. P6 may add the
`serea-capability -> serea-policy` dependency. Initial P5 can depend on
`serea-protocol`, `serea-storage`, and `serea-event-bus`; there is no upward
edge to TaskEngine or ModelRouter, nor a reverse storage/event-bus edge.

P5 may define pure duplicate-query/reservation primitives only if they cannot
dispatch and do not count a request before the defined authorization point.
Because duplicate and repeat checks occur after policy and approval, actual
execution integration, reservation commit and those counters belong no earlier
than the P6-integrated router. P5 can establish their durable schema only after
the owner resolves what constitutes an invocation intent and what survives
retention.

Mock provider types in `serea-testkit` are test doubles, not permission to
invoke in P5. P8's mock external-provider integration remains later work.

### P5 ownership

- Registry startup composition and validation; durable descriptor revisions
  and disabled overlay; provider health/availability snapshot.
- Local schema catalog and compiler; canonical model tool projection.
- A minimal model-authored tool proposal; registry lookup; strict input
  validation; SCJ-1 digest and IDK-1 preparation when TaskEngine supplies its
  durable TaskId/StepId context.
- A typed, immutable handoff of pinned descriptor facts for P6 policy and
  approval; no invocation authority.

### Deferred

- Policy decisions, grant matching/consumption and approval lifecycle (P6).
- Provider invocation, credential resolution, real/mock provider integration,
  receipt validation/commit and external-effect reconciliation (at least P6/P8
  integration; exact phase split is an owner decision).
- Task state transitions, StepId creation, lifecycle, retry/recovery authority
  (TaskEngine); GoalLatch and P6+.

## 3. Spec inventory

Classification meanings: **FROZEN** = current protocol rule; **ACCEPTED_ADR** =
accepted clarification/implementation contract; **CURRENT_RUNTIME** = behavior
in baseline code; **PROPOSED** = audit recommendation, not ratified;
**HISTORICAL** = earlier/prose statement superseded or narrowed by later
contract/code; **AMBIGUOUS** = owner choice needed; **CONTRADICTORY** = current
sources disagree; **NONCLAIM** = explicitly not implemented/authorized.

| Source | Classification | Semantic rule / current state | Owner and consequence | Test obligation | Owner decision? |
|---|---|---|---|---|---|
| Protocol Index §§3–5; Capability §1 | FROZEN | Capability IDs have exactly three segments and frozen verb set; no dynamic registration, model-created descriptor, or arbitrary shell action. | Protocol/registry; enforce grammar, verbs and reserved namespace at startup. | Invalid grammar, verb, foreign namespace and reserved ID refuse before visibility. | Yes for structural shell/reserved-namespace enforcement. |
| Capability §3, §3.2 | FROZEN | Descriptors are provider-authored, host-reviewed, durable, immutable for tasks that may reference them; updates apply to later tasks; removal disables new requests while pinned in-flight steps continue. | Capability plus durable task binding; textual rule lacks generation/pinning definition. | Restart, update, removal and existing task/new-step matrix. | Yes: define task snapshot and “new request.” |
| Capability §3.1; Protocol Index §5 | FROZEN | Input/output use JSON Schema 2020-12; closed objects, bounded strings/arrays, no patternProperties, ambiguous oneOf, unbounded recursion, or credential-class input. | Schema compiler; ordinary validator alone is insufficient. | Structural negative suite and local-ref tests. | Yes for catalog/resource/credential representation. |
| Capability §3.1 | FROZEN | Structural schema conditions are separate from operational numeric bounds (ADR-0020); no numeric limits are implicitly authorized. | Compiler must not hide limits. | Exact-boundary and refused-over-limit tests once ratified. | Yes for new schema byte/depth/complexity limits. |
| Capability §4.2; Policy §1; Event §3.3 | CONTRADICTORY | Old wording says model host fields are dropped and logged; action schema/Rust are closed with `deny_unknown_fields`; Model §4.1 says “drops unknown fields.” | Tool Router must reject authority fields and emit sanitized violation event; cannot silently continue after stripping. | Whole proposal rejected; safe diagnostic has no input values. | Yes on reject vs sanitize-and-continue; fail-closed recommendation is reject. |
| ADR-0002; Model §4.1; current ActionRequest | FROZEN / HISTORICAL | Model output is a proposal; host constructs ActionRequest after registry resolution. `ActionRequest` is not a model wire/proposal shape. | ToolRouter takes a dedicated minimal proposal, not ActionRequest deserialization. | Authority-injection payloads rejected before registry/policy. | Yes: freeze proposal wire shape and invalid-field behavior. |
| ActionRequest Rust/schema | CURRENT_RUNTIME | Closed request has request/task/step IDs, capability/version, args, digest, IDK, data class, requested_by and deadline; unknown fields reject. No provider, implementation, risk, effect class or authorization field. | Protocol foundation only; no runtime constructor exists. | Host constructor ownership and closed parse checks. | Yes: TaskEngine handoff and RequestId retry semantics. |
| Capability §4.1; RequestedBy enum | FROZEN | `requested_by` records provenance only and never grants authority. | Trusted caller context chooses MODEL/USER/SCHEDULER/PROACTIVE_WATCHER/SYSTEM. | Model cannot self-assert another provenance. | No. |
| Capability §8.2; ADR-0019 | ACCEPTED_ADR / CURRENT_RUNTIME | IDK-1 is domain-separated SCJ-1 over task_id, step_id, capability_id, version and canonical arguments; no attempt, provider or implementation. Implemented protocol derivation exists. | Reuse exactly; arguments outside SCJ-1 are refused. | Existing vectors plus P5 pipeline vector. | No, unless changing key contract. |
| Task §3; ADR-0018; storage 0001 | CURRENT_RUNTIME | Step pins provider_id, capability_id, version, IDK and input digest; attempt is durable. No implementation ID or descriptor snapshot/digest. | P5 cannot ensure restart-stable implementation or descriptor semantics from current Step. | Rootless/root swap and descriptor upgrade after restart. | Yes. |
| Capability §3.1 OPTIONAL_ROOT | FROZEN / AMBIGUOUS | Same CapabilityId/provider may have rootless and root implementation variants, differing in implementation_id; absence is CAPABILITY_UNAVAILABLE. No selection rule or unique logical key. | Registry must index variants independent of vector order; host chooses and pins variant. | Reverse provider/vector order; root changes across restart. | Yes: identity, selection, and pinning. |
| Capability §3.1, §9–10 | FROZEN / AMBIGUOUS | Provider advertises descriptors, may degrade/withdraw; authoritative durable registry survives provider absence. Health semantics and live-withdrawal protocol are unspecified. | Startup registry snapshot plus per-request availability check is a proposal, not frozen. | Ready/degraded/absent transitions and no mutation of descriptor history. | Yes: health race semantics. |
| Capability §10; Policy §7; Event §3.5 | CONTRADICTORY / AMBIGUOUS | Registry admin mutations are audited; `POLICY_CHANGED` describes policy rules or disabled-overlay changes. Policy does not exist in P5. | P5 registry must emit its own audit record without depending on P6, but event producer/transaction and event meaning require closure. | Registration/disable/enable/removal atomic event tests. | Yes: whether shared POLICY_CHANGED or new kind; new kind means architecture bump. |
| Capability §10; Policy §4.2 | FROZEN / AMBIGUOUS | Durable disabled overlay, no model enable path; Policy evaluates disable first. Scope “per capability” conflicts with multiple versions/implementations details. | Overlay identity and existing-step/retry behavior must be defined. | Disabled new Task/new Step/retry/recovery/admin re-enable. | Yes. |
| Capability §3.1 `experimental` | AMBIGUOUS | Boolean exists; no exact default, admin opt-in, listing, or task pinning semantics. It is not authorization. | Registry must not equate experimental with approval or policy. | Defaults, opt-in, visibility, in-flight after opt-out. | Yes. |
| Model §3, §4; P4 closure; ModelRequest Rust | CURRENT_RUNTIME / FROZEN | P4 validates host structured JSON and passes `ModelRequest.tools: Vec<Value>`; P5 owns capability tool semantics. P4 accepts schemas supplied by its caller but does not define capability tools. | P5 projects tools; P4 transports the exact host projection. | Stable projection/round trip, no provider schema substitution. | Shape of tool definition: yes. |
| Policy §4.2; Task §4.3 | FROZEN | Policy denies capabilities above immutable Task.policy_class. P5 must not hide this as visibility/authorization decision. | P6 policy; P5 may only apply registry-level availability and transparent task ceiling if caller explicitly requests prefiltering. | Above-ceiling proposal reaches policy denial or is refused at a documented host gate. | Yes whether ceiling-only filtering affects offered list. |
| Data §2–4; Capability §3.1 | FROZEN / AMBIGUOUS | Unknown/unclassified data is CREDENTIAL and refused; schemas need explicit allowlisted properties and defensive denylist. JSON Schema has no frozen field-class annotation; field-name heuristic cannot prove classification. | Need classified argument provenance or schema annotations and explicit composition rule. | Unknown field class refusal; mixed class composition; credential exclusion. | Yes. |
| Capability §3.1; Data §2 | AMBIGUOUS | Descriptor `data_class` is highest class capability transits/output bound; ActionRequest says highest class in arguments. Their relationship is not defined. | Proposed request class: max(trusted argument-class composition, descriptor floor); never underdeclare. | Lower/equal/higher, unknown and mixed class cases. | Yes. |
| Capability §5.1; Data §2 | AMBIGUOUS | Output is bare JSON. Consumer higher computation wins, but no output classifier/provenance exists; descriptor data_class cannot be assumed exact output class. | Need output class metadata/classified result and a rule for > descriptor. | Provider output higher than descriptor is refused/escalated, never down-classified. | Yes. |
| Capability §3.1 schema refs; P4 closure | PROPOSED | Resolve exact `$ref` URI only through trusted packaged/local catalog; no network, redirects or filesystem path. Pin Draft 2020-12. P4 model schema validator already denies remote refs but capability structural rules differ. | Capability compiler owns immutable catalog mapping. | Unknown/local/remote/redirect/fragment refs. | Yes for catalog version and size/complexity bounds. |
| Capability §3.1; ADR-0019 | ACCEPTED_ADR / AMBIGUOUS | SCJ-1 excludes fractions/exponents/negative zero and limits integers; valid JSON Schema may admit values SCJ-1 cannot digest. | Refuse noncanonicalizable arguments after validation; do not round/truncate. | Fractional schema value validates but preparation refuses exact canonical reason. | Yes only if expanding canonical numeric domain (ADR trigger). |
| Capability §8.3; Bounds B11 | FROZEN | Global 24-hour effecting duplicate key is `(capability_id, arguments_digest)`, deliberately excludes TaskId, StepId and version; prior result/receipt returned. | Host durable index/result; version upgrade may suppress changed semantics by frozen rule. | Cross-version matching and window boundary. | Yes whether the frozen cross-version behavior remains accepted; never silently add version. |
| Bounds B8/B11; Capability §1 | FROZEN / NONCLAIM | Policy and approval precede duplicate/repeat. No such P6 runtime exists. | Do not execute duplicate suppression in P5 before authorization. | Denied/pending request does not create reservation/count. | Yes on tool-call unit and reserve transaction. |
| Bounds §5.1 | FROZEN | Only SYSTEM + NATIVE may bypass duplicate window, with TOOL_DUPLICATE_WINDOW_BYPASSED. | Future execution router verifies trusted provenance and descriptor support. | All other combinations suppress/deny normally. | No. |
| Bounds B7–B9 | FROZEN | Repeated-action key per task is `(capability_id, arguments_digest)`, max 2; count only after registry, policy, approval and duplicate suppression. | Durable counter/history cannot depend on expiring event content. P5 cannot count early. | Counts 0/1/2/3; denied/pending/duplicate not counted. | No for key/order; yes for transaction unit definition. |
| Bounds §2.1, §7; AttemptBudget | FROZEN / AMBIGUOUS | max tool calls is 24 default and per-task ceiling is persisted in AttemptBudget; current tasks do not persist a used tool-call count. | Migration likely adds monotone task counter; which accepted/requested/dispatch unit consumes one is not defined here. | Limit/restart/prune and denied/duplicate boundaries. | Yes. |
| Capability §8.4; TaskStep.attempt | FROZEN / AMBIGUOUS | Max attempts per step 3; TaskStep already stores attempt. Bounds §6.3 says reconciliation attempts count, Capability says retries; dispatch-intent attempt semantics not exact. | TaskEngine owns sole durable counter; Capability reports typed outcomes. | Attempts across crash, reconcile and provider failure. | Yes if “attempt” needs clarify; do not add competing counter. |
| Capability §5–7; Rust result types | FROZEN / AMBIGUOUS | Receipt is only external-effect proof; provider result is closed typed data. Exact status/output/error/receipt/evidence matrix is not fully enumerated. | Host validates request ID, schema/digest, duration, status matrix, receipt/evidence bindings before commit. | Full valid/invalid result matrix, receipt mismatch/spoof tests. | Yes on missing matrix cells. |
| Capability §5.1 | FROZEN | Receipt must match capability and IDK, provider reference required except LOCAL_STATE, replay_safe reflects descriptor, observed time valid, unique receipt ID. Provider cannot widen replay safety. | Host compares all fields and replaces replay_safe from descriptor if contract chooses; provider is not authority. | Every mismatch, timestamp boundary, replay flag spoof. | Host validation mechanism: yes. |
| Capability §7; Event §3 | FROZEN / AMBIGUOUS | Evidence kinds include attempted, provider receipt, observation, goal result, policy denial, reconciliation; provider cannot author HOST/POLICY evidence. EvidenceId reuses EventId shape. | Host constructs host/policy evidence; provider observations are validated and persisted with classified payload refs. | Actor/kind authority and digest/reference class checks. | Yes on EventId global collision/identity semantics and payload retention. |
| Capability §6.2; Bounds §6.3 | FROZEN / AMBIGUOUS | Effecting ambiguous outcomes never blind retry; read-back through read-only capability then occurred/absent/unknown -> block. No descriptor field identifies reconciliation capability. | Capability returns typed reconciliation outcome; TaskEngine alone transitions task. | Crash points and all three reconciliation outcomes. | Yes: explicit reconciliation capability binding. |
| Capability §9; Data §3.1 | FROZEN / NONCLAIM | ProviderContext may carry a descriptor, deadline, cancellation and opaque CredentialHandle, never secret. No credential-store runtime exists. | P5 supplies no actual handle; credential-required capability stays unavailable absent later resolver. | No credential resolution/invocation in P5. | No. |
| Event §2–3; existing event bus | FROZEN / CURRENT_RUNTIME | Event vocabulary includes capability request/completion/deny/unavailable/duplicate/receipt/reconcile, model violation, bypass, bound exceeded. Event append is transaction scoped; no capability producer exists. | Future transaction owner emits content-free payload atomically with matching state. | Event/state atomicity, no raw args or schema text. | Yes on admin mutation event kind. |
| Architecture §1/§2; Crate Map §3 | FROZEN | Planned final capability crate depends on protocol/storage/event bus/policy and TaskEngine depends downward on capability. | Start with first three; add policy in P6; no cycle or storage/event reverse edge. | `cargo metadata` and dependency review. | No. |
| Architecture §1; GoalLatch §1/3/4 | NONCLAIM / FROZEN | `host.goal.*` is reserved for later sanctioned shim (P15); GoalLatch is not a CapabilityProvider. | Ordinary registry rejects reserved host namespace; no fake GoalLatch in P5. | Foreign provider registration attempt fails. | Yes on broader `host.*` reservation list. |
| P4 closure; Capability §1 | NONCLAIM | P4 provider/tool values are transport only; P5 does not imply provider invocation. P8 mock external integration remains future. | No Gmail/calendar/device/network effects. | Ensure tests use testkit only and router stops before invoke. | No. |

## 4. Contradiction ledger and exact proposal boundary

### 4.1 Model-authored type

`ActionRequest` must never deserialize directly from model output. The current
type is closed and already carries host authority/correlation, and ADR-0002
requires host construction. The proposal shape is not yet frozen. Proposed
minimal `ToolCallProposalV1` has exactly:

```json
{ "capability_id": "calendar.events.list", "arguments": { } }
```

It has no request/task/step identity, version, provider/implementation,
risk/effect/authorization, digest, IDK, classification, provenance, deadline,
credential or schema. `capability_id` remains the canonical internal tool name;
provider-specific name translation is an adapter concern, not P5.

The tool response schema validates the proposal envelope first. Only after the
closed minimal shape passes does P5 look up the ID and validate arguments
against the pinned input schema. Unknown IDs never select or invoke arbitrary
schemas. Arguments must then pass duplicate-key-aware parsing, descriptor
validation and SCJ-1 canonicalization. A schema-valid but non-SCJ-1 value is
refused; no alternate digest is permitted.

### 4.2 Unknown fields and authority injection

Three interpretations exist: (A) reject whole proposal; (B) strip forbidden
fields and continue; (C) reject while retaining only safe diagnostic metadata.
The security recommendation is **C**: reject the whole proposal, emit
`MODEL_SCHEMA_VIOLATION`, and keep bounded diagnostic facts such as violation
category and field names only if field names are safe/allowlisted. Never retain
values, raw JSON, credentials, prompt text or arbitrary field names. Do not
continue with a sanitized proposal. This reconciles deny-unknown-fields,
Protocol Index security-sensitive closure, and audit visibility. It is a
proposed owner choice because Capability §4.2 and Model §4.1 currently say
drop-and-continue.

### 4.3 Host field ownership

| Field | Model-authored? | Trusted owner/resolver |
|---|---|---|
| `request_id` | No | Capability runtime mints per request/dispatch; retry identity decision remains open. |
| `task_id` | No | TaskEngine supplies immutable context. |
| `step_id` | No | TaskEngine mints and durably binds before capability preparation. |
| `capability_id` | Yes, as an untrusted proposal only | Registry validates exact registered ID. |
| `capability_version` | No | Host snapshot/version selector; exact selection rule is open. |
| `provider_id` | No | Registry descriptor/provider registration. |
| `implementation_id` | No | Host device/root/provider composition selects then pins; selection rule open. |
| `risk_class` | No | Registry descriptor; P6 policy consumes immutable fact. |
| `side_effect_class` | No | Registry descriptor. |
| `required_authorization` | No | Registry descriptor; P6 policy/approval applies. |
| `arguments_digest` | No | Host SCJ-1 digest after strict validation. |
| `idempotency_key` | No | Host IDK-1 from supplied task/step IDs and pinned capability/version/args. |
| `data_class` | No | Host classified-input composition plus descriptor relationship, not yet defined. |
| `requested_by` | No | Trusted caller context; provenance has no authority. |
| `deadline_ms` | No | Host min of descriptor, task remaining budget and trusted caller/provider cap. |

TaskEngine must provide TaskId, StepId, task policy ceiling, deadline/wall-clock
remaining, cancellation, and trusted `requested_by`. Capability must not depend
upward on TaskEngine to obtain them. Bounds §6 currently names
`model_call_deadline_ms` in the provider formula; this is stale/ambiguous for a
capability call. Proposal: use a generic host `call_deadline_ms` or omit the
model-only term and take min(descriptor max, task remaining, explicit caller
cap). Owner decision required; a model never supplies it. Retry may tighten
deadline as remaining task time shrinks but may not extend descriptor authority.

## 5. Tool definitions and visibility

P5 creates one deterministic host projection for P4 `ModelRequest.tools`.
Proposed visible definition includes canonical `CapabilityId`, human title and
description, and exact input schema. Output schema is not needed to propose a
call. Stable ordering is lexical by CapabilityId then selected version (after
version/variant selection is resolved); never HashMap/provider Vec order.
CapabilityId remains the internal ID; provider APIs translate names later.

Do not expose provider_id, implementation_id, risk, side-effect class,
authorization, root requirement, credential handle, replay/idempotency support,
policy internals, provider health details or schema catalog internals. A
definition is information, never authority; every returned proposal is
revalidated.

Registry-only visibility may exclude disabled entries, absent/unusable
implementations, and descriptors that failed startup validation. It cannot
make P6 policy decisions. Policy ceilings, experimental opt-in, approval
pending state, and task-context visibility are not currently precise enough to
filter safely. Disabled capabilities should not be offered; model proposal is
still refused if it names one. “Unavailable provider” visibility requires the
health contract decision: simplest safe behavior is omit from new tool list,
but retain durable descriptor for old pinned steps and report
CAPABILITY_UNAVAILABLE. A capability above the task policy ceiling must not
execute; whether it is hidden or shown with later denial is an owner decision.
No P5 decision may infer approval or expose a tool because it is OBSERVE.

`experimental` is not a permission or risk class. No default-disabled,
admin-opt-in, visibility or in-flight opt-out rule is frozen. Until decided,
experimental entries must not silently become model-visible or executable.

P4's `PreparedModelCallV1::from_host` counts serialized `tools` inside the
aggregate model prompt byte bound, so projected tool bytes already consume that
existing cap. It does not establish a separate maximum tool count or a
capability-schema compilation bound. `max_tool_calls_per_task` is a different
quantity. A new tool-count bound (and any separate schema catalog/compiler
byte/depth/complexity limit), plus its refusal/event behavior, is an operational
bound requiring owner decision under ADR-0020/B3. Do not truncate silently or
invent constants.

## 6. Registry identity, snapshots, versions and lifecycle

### 6.1 Identity model

CapabilityId alone is a logical family, not a unique descriptor: versions and
OPTIONAL_ROOT implementations can coexist. ProviderId is derived from the ID
namespace and validated against `CapabilityProvider::provider_id()`; it is not
an independent key. A candidate descriptor identity is
`(CapabilityId, SemVer, ImplementationId)` with one explicit primary variant
per version. `implementation_id` optionality is insufficient when variants
coexist; missing IDs must either be normalized to one frozen default or refused
when ambiguous. Duplicate exact identities conflict. Provider Vec order is
never a tie-break. Owner decision required.

### 6.2 Version and implementation selection

The model supplies no version or implementation. New-task selection must be
host deterministic: either one explicitly enabled version/variant or a
configured selector captured in a registry snapshot. “Highest SemVer” is not
frozen and is unsafe as an implicit default (prereleases and compatibility
changes). Old Tasks keep the exact descriptor and implementation identity they
captured. OPTIONAL_ROOT requires host root/device facts to select; model cannot
choose. Root absence yields CAPABILITY_UNAVAILABLE, not fallback that changes
an already pinned step. Owner choice is required for new-step selection,
prerelease inclusion and fallback behavior.

### 6.3 Pinning, hot update and removal

Current TaskStep contains `provider_id`, `capability_id`, `capability_version`,
IDK/input digest/attempt/receipt, but no `implementation_id`, descriptor digest,
schema digest, data/risk snapshot, or registry generation. Restart after provider
upgrade can therefore reinterpret schema, risk, replay or implementation.
Deterministic re-resolution is not safe if composition changes. A separate
durable binding row or TaskStep extension must pin descriptor snapshot identity
and implementation. Descriptor snapshot data/digest must be immutable and
available for each retained nonterminal step.

“Task created afterwards” plausibly means at task acceptance, but tasks plan and
replan later; protocols do not say whether a new Step inside an old Task sees
the task's original registry view or current registry. Define snapshot capture
at Task creation and apply it to every later/replanned step, or define a
per-step binding point. Do not mix. Registry generation alone is insufficient
unless it maps to immutable retained descriptor snapshots; avoid persisting both
generation and equivalent snapshots without need.

Removal disables new Task requests/new Steps; same-step retry/recovery of a
durably pinned in-flight step must retain its binding. A reconciled replacement
Step is a new Step and must use the then-authorized current registry and rerun
policy/approval. This lifecycle matrix is proposed; owner must ratify exact
meaning. Disabled overlay also needs exact key semantics (logical capability,
version, or implementation) and whether it blocks pinned retries. Policy says
disabled wins, while Capability says in-flight steps continue; the safest
reconciliation is disabled for new bindings, existing pinned step continues,
but it needs owner approval.

### 6.4 Provider absence and health

Keep durable descriptor history when a provider is absent. Distinguish typed
outcomes: malformed proposal/input -> VALIDATION; never registered ID/version
-> UNKNOWN_CAPABILITY; known but disabled -> policy denial/disabled outcome;
known pinned descriptor with provider absent, degraded, unsupported runtime,
root absent or implementation unavailable -> CAPABILITY_UNAVAILABLE. Do not
branch on free-text errors or conflate disabled with unknown. The frozen
ActionErrorKind does not include a distinct DISABLED/IMPLEMENTATION_UNAVAILABLE
kind; use typed registry/router outcome outside ActionError or map disabled to
P6 PolicyDenied. Owner must settle exact mapping.

Validate provider namespace, duplicate descriptor identities, SemVer,
implementation IDs, root invariants, authority consistency and schema refs at
registration. Namespace mismatch is a security contract failure; Rust returns
typed ContractViolation rather than the prose “panic.” Invalid descriptor or
schema is hard startup failure. Configured provider absent/degraded and root
absent are soft availability states; never rewrite descriptor authority.
Startup snapshot fixes descriptor authority; runtime health can affect
availability only. Per-request health check race semantics and provider
withdrawal after startup need an explicit rule; do not let `capabilities()`
mutate historical facts at runtime.

## 7. Schema catalog and classification gaps

### 7.1 Catalog/compiler

`JsonSchemaRef` is a reference, not embedded schema. Examples use
`https://serea.local/schemas/...`; P5 needs a trusted packaged/catalog mapping
from exact canonical URI to bytes and digest. Resolve only local catalog IDs;
no DNS, HTTP(S) fetch, redirects, arbitrary filesystem path, or provider
resolver. Unknown ref is startup failure. Pin Draft 2020-12. The existing P4
`jsonschema` setup is relevant but capability compiler must additionally
inspect schema structure before compiling: exact `$schema`, references only in
catalog, `additionalProperties:false` recursively for objects, explicit
property allowlists, bounded strings/arrays, forbid patternProperties,
prove `oneOf` branches do not overlap or reject such composition, and bound or
reject recursion. Output schema gets same structural/execution scrutiny.

The current protocol says “no recursive schema without an explicit depth
bound,” but JSON Schema has no frozen Serea depth annotation. Structural limits
are distinct from operational schema-byte/depth/property/compiled-complexity
limits. P4's model schema bounds are not capability schema limits. B3 says no
hidden resource limits; owner must specify capability schema bytes, nesting,
property count and compiler complexity or ratify a finite structural profile.
Schema bomb refusal must occur before visibility or model call.

Credential exclusion cannot be proven by JSON Schema property names. Data §4
requires allowlist plus forbidden-property defense, but no frozen field class
annotation or classified input type exists. Name heuristics are not security.
Add schema classification metadata or a trusted `Classified<T>` argument
provenance map; unknown class means CREDENTIAL and refusal. The exact model is
an owner decision.

### 7.2 Input and output data class

Descriptor `data_class` means highest class the capability transits and bounds
output flow. ActionRequest says “highest class in arguments,” but raw JSON
values carry no class. Task inputs have provenance potentially, but P5 lacks a
mapping into field-level JSON values. Proposed rule is the max of every
argument's trusted class and descriptor transit floor; missing/unknown is
CREDENTIAL and refused. This may make a capability's declared class a floor,
not an exact argument class; owner must define it.

Provider output is bare JSON and has no per-value class. Descriptor class can
be treated as a declared upper bound only if host can independently compute
actual class; no current mechanism does. Data protocol says consumer's higher
computation wins. Add result classification/provenance or treat unclassifiable
output as CREDENTIAL/refuse onward flow. Reject or escalate provider output
that computes above descriptor; never downgrade. This is an architecture-level
gap, not implementation detail.

## 8. Action construction, attempts and bounds

After minimal proposal validation and registry resolution, host validates
arguments against selected input schema, performs duplicate-aware parse,
SCJ-1 canonicalization, computes `arguments_digest`, and derives exact IDK-1
from TaskId/StepId/capability/version/arguments. TaskEngine owns TaskId and
StepId; P5 accepts them through immutable context. No implementation ID/provider
is added to the frozen IDK. OPTIONAL_ROOT variants may intentionally share a
logical key; therefore implementation cannot switch on retry without an
explicit safety decision. Pin implementation for all same-step retries.

`RequestId` semantics across retry are not frozen: ActionRequest says it
correlates one request/result, protocol says same IDK across attempts. P4 model
accounting uses one RequestId per dispatch intent; that does not settle action
requests. Recommend each distinct provider dispatch attempt gets a new
RequestId, while same Step/IDK and monotonically incremented TaskStep.attempt
remain stable binding. A repeated response must match that RequestId. Owner
decision required, including provider response/evidence identity.

Deadline is host min of descriptor maximum and remaining task wall clock, with
trusted caller/provider cap if defined. A retry can only tighten deadline.
TaskStep.attempt is the sole per-step durable attempt count; no second counter.
Whether reconciliation lookup consumes one of three attempts is ambiguous
(Bounds §6.3 says yes); confirm the definition and count provider dispatch and
read-only reconciliation consistently.

`max_tool_calls_per_task` is 24 and `AttemptBudget.max_tool_calls` exists, but
SQLite tasks have no used counter (migration 0003 only adds model counters).
Add durable `tool_call_count` if this quantity survives action detail/event
retention. Bound docs fail to define whether construction, policy denial,
pending approval, duplicate suppression, authorized dispatch intent, or provider
dispatch consumes a unit. Recommendation: count committed authorized provider
dispatch intents; denied/pending/duplicate-suppressed proposals consume zero;
one dispatch consumes exactly one even if timeout/ambiguous. Requires owner
decision and consistency with B8.

## 9. Duplicate, repeat, idempotency and crash semantics

### 9.1 Duplicate and repeat

Frozen duplicate key is `(capability_id, arguments_digest)` globally for 24h
effecting calls; version is intentionally absent. A version 2 action with same
ID/args may therefore return a v1 result and receipt. This is an explicit
consequence of current frozen rule and a potentially surprising semantic
collision; flag for owner confirmation without silently changing key.

Durability must preserve enough to reconstruct prior `ActionResult`, output,
receipt, evidence references and completion/effect timestamp after restart.
TaskStep has only result digest and optional receipt; complete output/result is
not there. A content-addressed classified result blob plus indexed action row
may suffice. Retention must not delete the result while it is inside duplicate
window or referenced by a nonterminal step/evidence. Duplicate matching needs
transactionally serialized reserve/lookup after policy/approval: concurrent A
and B cannot both observe absence. A reservation must have explicit states and
recoverable transitions; never hold SQLite write txn across provider call.

Repeated-action count is per Task, same `(capability_id, arguments_digest)`,
max two; persist independent of event pruning. Count point is after policy,
required approval and duplicate suppression; denied, pending, expired,
duplicate-suppressed requests count zero. SYSTEM/NATIVE is the only duplicate
window bypass and emits the exact event; it does not bypass repeat bounds.

### 9.2 NATIVE, EMULATED and NONE

- NATIVE: provider deduplicates the same IDK; host still owns dispatch history,
  receipt validation and duplicate-window protection.
- EMULATED: host persists unique IDK reservation and suppresses another dispatch
  of same key. This is different from the 24h cross-task duplicate window.
- NONE: no idempotency guarantee; never describe as deduplicated; same-step
  retry after ambiguous effect must reconcile/block, not blindly re-dispatch.

Persist distinct `RESERVED`, `DISPATCHED`, `COMPLETED`, and `AMBIGUOUS` states.
A crash after reservation but before dispatch is known-safe only if durable
dispatch-intent commit is the exact point after which provider may be called.
The crash after external effect/response loss is ambiguous. No SQLite txn can
atomically include the external effect; never claim exactly-once. For duplicate
race, reserve under SQLite serialization before dispatch, commit intent, then
call provider outside txn; on completion atomically persist result/receipt and
release/finalize reservation. Recovery treats any dispatched effecting call
without durable result as ambiguous unless NATIVE reconciliation proves
otherwise.

### 9.3 Crash points and result boundary

Future state machine must cover: committed intent then crash before call;
provider may effect then response lost; response received but receipt commit
lost; receipt committed but caller lost reply. Return a typed capability
outcome (prepared, unavailable, completed, ambiguous/reconcile-needed) to
TaskEngine; capability runtime never mutates task lifecycle. Reconciliation
uses explicit descriptor-bound read-only capability ID or host-approved
natural-key procedure; current descriptors do not say which one, so no
name-based inference is allowed.

Validate provider result against request and immutable descriptor before
storage: RequestId exact; legal status; output shape and SCJ digest; error/status
consistency; duration <= host-measured/declared bounds; receipt capability and
IDK match; provider_reference rule; unique ReceiptId; observed_at plausibility;
`replay_safe` equals host descriptor (provider cannot choose); evidence IDs,
kind/actor and data classes are permitted; payload digest/blob class agree.
Result `SUCCEEDED + NONE` requires no receipt; effecting success requires
receipt. `FAILED` likely requires error; `REJECTED` and `UNAVAILABLE` should be
host outcomes rather than provider claims; duplicate-suppressed returns prior
output/receipt with no fresh provider evidence. Full matrix is not frozen and
must be ratified before invocation.

Providers may supply provider observations/receipt payload only. Host creates
ACTION_ATTEMPTED, POLICY_DENIAL, and reconciliation facts. Provider cannot
forge HOST/POLICY actor evidence. EvidenceId reuses EventId format but global
uniqueness/relationship to event IDs is unclear. Payload references must point
to content-addressed blobs with data class at least actual payload class and
correct at-rest protection; no raw sensitive event payloads.

## 10. Migration 0004 minimum plan (not created)

Schema 3 currently persists tasks, steps, blobs, events, leases and model
accounting only. Task row has attempt budgets and model counters but no tool
counter. Step has provider/capability/version/IDK/input digest/result digest/
receipt/attempt but no implementation/snapshot binding or complete result.
No registry, descriptor, disabled overlay, action attempt/result, receipt,
duplicate or idempotency ledger exists.

| Durable fact/table (candidate) | Purpose / authority | Key, FK, indexes and crash invariant | Class / retention | Phase |
|---|---|---|---|---|
| `capability_descriptors` immutable revision/snapshot | Provider descriptor bytes or canonical snapshot + digest, schema refs/digests, identity, authority; owned by registry. | PK candidate descriptor identity + revision; no cascade while TaskStep references; index by logical ID/version and provider. Registration and admin event atomic. | Descriptor metadata PUBLIC unless descriptions reveal more; retain while pinned task/step, duplicate result, receipt/evidence references it. Existing `blobs` accepts only PUBLIC/PERSONAL/PRIVATE; SECRET/CREDENTIAL require refusal or a separately authorized sealed store. | P5 foundation, exact schema owner decision. |
| `capability_disabled` overlay | Durable admin enabled/disabled state and reason/actor. | Key unresolved (logical ID vs version/implementation); FK restrictive to registered logical descriptor; updated with audit event atomically. | PUBLIC/admin audit; retain current + audit retention. | P5. |
| task registry snapshot / `step_capability_bindings` | Bind task/step to immutable descriptor revision, implementation ID, selected version and relevant authority facts. | PK TaskId+StepId; FK task `ON DELETE CASCADE`, descriptor `ON DELETE RESTRICT`; lookup by descriptor. Binding committed before policy/approval; immutable thereafter. | Authority metadata PUBLIC; retain while step/task, result/receipt/evidence needs it. | P5, depends owner pin model. |
| `tasks.tool_call_count` | Durable task-wide B3 budget after detail pruning. | Monotonic bounded integer; task row; update in same intent transaction. | Non-content metadata, task lifecycle retention. | P5/P6 integration after count semantics decision. |
| action invocation/result record + result blob ref | RequestId, task/step/attempt, descriptor binding, IDK, state, output/result digest/blob, receipt/evidence refs, times/status. | RequestId unique; task/step FK behavior must preserve effects; unique active `(task,step,attempt)`; indexes duplicate key/time and recovery state. Intent and state transition atomic; provider outside txn. | Result follows max output DataClass; retain at least duplicate window and while referenced; no early blob GC. | Later P6/P8 execution; not necessary for registry-only P5. |
| duplicate/idempotency reservation/index | Concurrency-safe global duplicate and EMULATED same-IDK reservation with RESERVED/DISPATCHED/terminal distinction. | unique active IDK for EMULATED; duplicate lookup `(capability_id, arguments_digest, effect_at)`; serialized reservation. | Metadata plus result reference; retain duplicate window + pending ambiguity; no deletion while unresolved. | Later execution; owner decision on shared model/table. |
| repeat history/counter | Per-task identical action count after authorized, nonduplicate attempt. | Either durable normalized counts key `(task_id, capability_id, arguments_digest)` or reconstructable active history. Task FK cascade; indexed by task/key. Increment atomically with dispatch intent. | Digest metadata; retain through task active/step retention. | P6 integration. |
| evidence/receipt tables | Durable audit proof and payload references when TaskStep single receipt is insufficient. | ReceiptId unique; EvidenceId unique; links to action attempt and descriptor; restrict delete while effect unresolved. | payload class and retention per data/event contracts. | Invocation phase; exact payload storage decision open. |

This is a minimum *conceptual* set, not a migration DDL proposal. Avoid separate
tables when an immutable binding plus action attempt/result row can represent
the facts safely. Do not add policy/grant tables to P5 migration merely because
the final crate owns them; P6 owns those. Migration 0004 likely exists for P5
registry/binding durability, but implementation should wait for exact snapshot
and overlay decisions.

## 11. Events and transaction owners

| Event | Producer / atomic state | Payload constraints |
|---|---|---|
| `CAPABILITY_REQUESTED` | Capability router; action preparation/authorized intent transaction must be specified. | IDs, capability, status/classified digest only; no raw arguments. |
| `CAPABILITY_COMPLETED` | Capability result committer; same txn as validated result/receipt. | status/duration/digests, no raw output. |
| `CAPABILITY_DENIED` | P6 policy/router, same txn as denial/outcome. | reason code and identity; no raw arguments. |
| `CAPABILITY_UNAVAILABLE` | Registry/router availability gate and durable outcome. | typed availability category, IDs. |
| `CAPABILITY_DUPLICATE_SUPPRESSED` | Duplicate result transaction. | prior result identity/digest; preserves original receipt. |
| `CAPABILITY_RECEIPT_RECORDED` | Result commit transaction. | receipt ID/digest, no effect content. |
| `CAPABILITY_RECONCILED` | Reconciliation commit; TaskEngine owns lifecycle edge. | typed occurred/absent/unknown and refs. |
| `MODEL_SCHEMA_VIOLATION` | Tool proposal validator; transaction with refusal record. | sanitized category/field names only; no offending values/raw JSON. |
| `TOOL_DUPLICATE_WINDOW_BYPASSED` | Host duplicate gate; same authorized reservation transaction. | SYSTEM provenance, capability, digest, reason code. |
| `BOUND_EXCEEDED` | Bounds owner; same task failure/update transaction where applicable. | bound name, limit, observed, task/step only. |
| `POLICY_CHANGED` | Current docs say policy/rules/disabled overlay; no P5 policy dependency allowed. | Registry mutation semantics currently overloaded; decide shared kind or new registry event. |

Do not log the same registry mutation in two semantically duplicate event
kinds. If a new event is required, update Event Protocol, ADR and architecture
version. All state/event writes must share one SQLite transaction; event append
uses event bus over storage, not a reverse dependency.

## 12. Security threat model

| Threat | Required control / audit result |
|---|---|
| Model injects risk/provider/version/implementation/deadline/provenance | Minimal closed proposal; reject whole proposal and emit sanitized violation; host resolver owns every field. |
| Model creates capability or arbitrary shell command | No dynamic registration; verb/namespace constraints plus reserved IDs and reviewed built-in registry. Grammar alone does not prohibit `system.shell.run`; structural deny/reservation needed. |
| Provider claims foreign namespace or widens authority | Compare provider ID, ID prefix and descriptor; reject startup. Provider cannot modify descriptor after snapshot. |
| Schema remote-ref SSRF/path traversal or schema bomb | Exact local catalog only; structural validation and ratified byte/depth/complexity bounds. |
| Credential smuggling | Closed object schemas + explicit allowlist and typed classification; denylist/field-name heuristics are defense only. Unknown class => CREDENTIAL/refusal. |
| PRIVATE/SECRET/CREDENTIAL underclassification | Trusted field provenance/class metadata and max composition; descriptor relation and output classifier need owner decisions. |
| Provider spoofs RequestId/receipt/replay safety/output | Exact host comparison and schema/digest/status validation; replay_safe replaced/checked against descriptor. |
| Root variant changes on restart | Pin implementation identity with descriptor snapshot for same Step; no iteration-order selection. |
| Duplicate race across tasks | Serialized durable reservation after policy/approval and before provider dispatch; no network in transaction. |
| Experimental capability auto-enable | Explicit admin opt-in/visibility decision; never equate experimental with approval. |
| `host.goal.*` claimed by ordinary provider | Reserve `host.*` (at minimum `host.goal.*`) against ordinary providers; sanctioned shim only in P15. |
| Raw sensitive args/results leak through events/diagnostics | Digests/IDs and bounded safe categories only; payload blobs carry computed class and retention. |

## 13. Future implementation slices

- **P5A — contract closure:** owner decisions below; amend protocols/ADRs and
  architecture version only after acceptance.
- **P5B — migration and durable registry:** descriptor history, disabled
  overlay, task/step binding model; no runtime effects.
- **P5C — catalog/compiler/registry:** deterministic startup registration,
  structural schema checks, health and typed availability, immutable snapshots.
- **P5D — projection and proposal:** stable host tool definition, minimal
  proposal validation, safe violation event, classified arguments and prepared
  ActionRequest foundation. Stop before policy/approval/provider.
- **P5E — durable execution foundations:** only after owner choices, design
  duplicate, repeat, idempotency reservations, result/evidence persistence and
  call counters. No provider dispatch until P6 ordering is present.
- **P5F — P6 integration boundary:** typed immutable policy context, post-approval
  revalidation of mutable availability, policy/approval/duplicate/repeat order,
  crash and concurrency closure. Provider invocation only when the full
  authorization sequence is implemented; P8 remains provider integration.

## 14. Future RED-first test plan

- **Registry:** exact duplicate identity, multiple versions/implementations,
  namespace mismatch, malformed authority, provider absent/degraded, overlay
  disable/re-enable, startup vs request health, update/restart and pinned old
  task/step.
- **Schema:** missing/local/remote refs, wrong Draft, open nested object,
  unbounded strings/arrays, patternProperties, overlapping oneOf, recursive
  schema, schema bomb/resource boundary, credential-class property, malformed
  output schema, no network/filesystem resolver.
- **Proposal:** valid minimal object, unknown field, every host-field injection,
  unknown capability, invalid args, duplicate keys, invalid SCJ-1, stable digest
  and exact IDK-1 vector; diagnostics contain no input values.
- **Classification:** known class, mixed max, unknown->refusal, descriptor floor
  conflict, output class above descriptor, no credential reachability.
- **Duplicate/idempotency:** 24h exact boundary, read not suppressed,
  cross-version interaction, SYSTEM/NATIVE bypass only, EMULATED same-IDK,
  concurrent two-task reserve race, crash at RESERVED/DISPATCHED/result/receipt,
  restart and prior result/receipt reconstruction.
- **Repeat/bounds:** 0/1/2/3, count after policy and approval only, denied/pending/
  expired/duplicate not counted, tool-call count at every candidate boundary,
  retention and restart.
- **Pinning:** descriptor/schema update after Task creation, new Step in old
  Task, provider disappearance, optional-root selection change, same-Step
  retry after restart, replacement Step after reconciled absence.
- **Provider result:** mismatched RequestId, invalid output schema/digest,
  every status matrix cell, missing/mismatched receipt, replay_safe spoof,
  evidence actor/kind mismatch, duration and data class mismatch, ambiguous
  result no blind retry.

All P5 router tests use deterministic local testkit providers only for metadata
or result-validation unit tests. They must assert zero `invoke` calls before
P6; no external integrations belong here.

`MockCapabilityProvider` accepts multiple descriptors and scripted success or
typed error outcomes, so tests can provide malformed/mismatched result objects
and `AMBIGUOUS` errors. Its health is always `Ready`; absence is represented by
not registering it, and degraded-health transitions need a small test-only
provider implementation or a later testkit extension. Runtime must not depend
on `serea-testkit`.

## 15. Owner decisions required

These are architecture-level choices; until resolved, status is
`BLOCKED_PENDING_OWNER_DECISION`.

1. **Descriptor identity and pin model:** exact uniqueness key; required or
   default `implementation_id`; Task registry snapshot vs per-step binding;
   update/replan/new-Step/removal/retry/recovery semantics; snapshot retention.
2. **Selection and availability:** version/prerelease selection, OPTIONAL_ROOT
   implementation selection, task creation snapshot, health sampling and
   provider withdrawal race; disabled overlay key/scope and in-flight behavior.
3. **Tool proposal and diagnostics:** accept/freeze minimal `ToolCallProposalV1`
   (or other exact shape); reject-vs-strip contradiction; safe event payload;
   tool visibility for unavailable, above-ceiling, experimental and pending
   approval capabilities.
4. **Schema catalog and bounds:** canonical local URI/catalog ownership,
   recursion profile, byte/depth/property/compiled-complexity limits and
   overflow outcomes; structural credential-class proof mechanism.
5. **Data classification:** source of per-argument classes; relation between
   descriptor transit class and ActionRequest data_class; output classification
   and behavior above descriptor; unknown classification refusal boundary.
6. **Execution identity and bounds:** RequestId across retries; implementation
   stability when IDK omits implementation; provider attempt vs reconciliation
   counting; what consumes task tool-call unit; tool-list bounds.
7. **Duplicate/idempotency persistence:** cross-version 24h suppression
   confirmation; prior ActionResult retention; reservation state machine and
   atomic transaction boundaries for duplicate race and EMULATED IDK.
8. **Result/reconciliation/evidence:** full ActionResult status matrix,
   receipt validation/identity/time rules, evidence actor authority and ID
   uniqueness, explicit reconciliation-capability binding and typed outcome
   handoff to TaskEngine.
9. **Registry admin audit / reserved IDs:** reuse POLICY_CHANGED or add registry
   event (architecture version impact); reserved provider namespaces, including
   `host.*` beyond `host.goal.*`; mechanical no-shell registration control.
10. **P5/P6 execution seam:** whether P5 ends at prepared action (recommended)
    and whether P6 integrates invocation after policy+approval; exact
    unauthorized outcome type without manufacturing ActionError/status.

Likely ADR clusters: registry snapshot/implementation/version identity;
proposal/request and sanitized rejection; schema catalog/resource/classification;
durable action/idempotency/result semantics. Do not mark these Accepted until
the owner chooses. Any contract choice may require an architecture version
bump; no bump is made by this audit.

## 16. Audit conclusion

P5 is **not ready to implement an executable Capability Router**. Safe P5 work
can close registry/schema/tool proposal preparation only after decisions 1–5
and the P5/P6 seam are resolved. Provider invocation must wait until policy and
approval are in the enforced call path. There are material contract/code gaps
in descriptor/implementation pinning, classification, tool proposal semantics,
schema resource bounds, durable result/duplicate state, reconciliation binding,
event ownership and action-attempt identity.

No architecture version is bumped here. No runtime, crate, migration or
provider invocation is started.

## 17. P5A owner-decision closure — 2026-10-08

This section supersedes §§10–16 wherever they describe an open owner choice,
P5 dispatch work, provider-authored registration authority, proposal-extra
stripping, version selection, schema recursion, classification proof, registry
events, or migration scope. Owner decisions are recorded as Accepted in
[ADR-0034](../decisions/ADR-0034-capability-manifest-registry-and-pinning.md),
[ADR-0035](../decisions/ADR-0035-tool-proposal-schema-and-prepared-action.md),
and [ADR-0036](../decisions/ADR-0036-p5-p6-p8-authorization-and-dispatch.md).
Architecture advances `serea-arch/2.5.0` to `serea-arch/2.6.0`; `serea.action/2`
does not change. P5 runtime remains unstarted; no crate or migration 0004 is
created here.

### 17.1 Frozen phase boundary

P5 owns durable registry, host manifest matching, descriptor history/pinning,
trusted local schemas/compiler, deterministic model-tool projection,
ToolCallProposalV1 validation, classified arguments, PreparedActionV1, typed
availability/refusal outcomes, and typed P6 handoff. P5 does not evaluate
policy, approval/grant matching, duplicate suppression, repeated-action
execution state, tool-call accounting, dispatch reservation, provider invoke,
result acceptance, receipt/evidence commit, or ambiguity reconciliation.
P6 owns deterministic policy, approval/grant lifecycle, and authorization of
PreparedActionV1; P6 cannot invoke providers. P8 is first allowed to invoke
CapabilityProvider, with deterministic mock external providers.

P8 frozen order: proposal validation -> registry/schema -> P6 policy -> P6
approval -> duplicate suppression -> repeat bound -> tool-call budget and
durable dispatch intent -> provider invoke -> result/receipt/evidence ->
reconciliation when required. No production shortcut exists.

### 17.2 Registry, manifest, and binding decisions

CapabilityManifestV1 is trusted host authority. Provider advertisement only
confirms/withdraws availability; unmanifested descriptors fail that provider
registration. Manifest entries identify CapabilityId, SemVer, ProviderId,
optional ImplementationId, descriptor semantic digest, input/output catalog
schema identity/digests, and candidate eligibility. Ordinary providers cannot
register `host` or `host.*`; no P5 host builtin is registered. The host manifest
allowlist is the shell exclusion proof.

Logical identity is CapabilityId + SemVer + optional ImplementationId. None is
legal only for one implementation per ID/version/generation; multiple variants
each require distinct Some IDs. Duplicate exact identity rejects activation.
Descriptor revisions are immutable and host-digested across authority-bearing
facts and schema digests. A registry generation snapshots revisions, manifest
defaults/priorities, and schema catalog revision. New Tasks pin one generation;
pre-P5 NULL-generation Tasks cannot create capability Steps. Existing Steps
pin revision/provider/implementation before P6; retries never switch.

Live enabled/removed and experimental opt-in overlays are keyed by CapabilityId
across versions and implementations. Current disable/removal blocks every new
binding, including old Tasks; already-bound Steps can recover on pinned facts.
Experimental requires durable local-admin opt-in; model/device cannot enable.
Manifest explicitly selects one default version; no runtime latest/provider
order/model selection. Prerelease only when that exact version is manifest
default. Implementation selection is the first ordered manifest candidate
eligible in one immutable host-availability and provider-health snapshot.
Health changes availability only; a bound unavailable implementation returns
CAPABILITY_UNAVAILABLE without failover.

### 17.3 Proposal, schema, and classification decisions

ToolCallProposalV1 has exactly `{version:"1", capability_id, arguments}` with
object-root arguments. Any unknown or authority-bearing field rejects the
whole proposal. No ActionRequest/PreparedAction results. MODEL_SCHEMA_VIOLATION
metadata is limited to TaskId, optional StepId/model RequestId, stable code,
offending field names, and count; never values, prompt, proposal, or arguments.

ToolDefinitionV1 has exactly version, capability_id, title, description, and
input_schema; sorted by CapabilityId UTF-8 bytes. It exposes no authority
fields. Visibility uses task-pinned registry, current overlay, experimental
opt-in, structural validity, and eligible READY implementation; policy and
approval are not visibility filters.

CapabilitySchemaCatalogV1 permits only exact `https://serea.local/schemas/`
references, no network/filesystem/redirect/DNS/traversal, exact trusted
catalog references and same-document pointers, Draft 2020-12. P5 V1 limits:
65,536 canonical bytes, depth 64, 4,096 schema nodes, 256 properties/object;
no cyclic refs, no open objects, no patternProperties, arrays require maxItems,
strings require maxLength, and unprovably disjoint oneOf is refused. Overflow
is typed fail-closed registration refusal; no truncation. Structural limits
are not B3 counters.

ClassifiedArgumentsV1 receives trusted provenance. Unknown is CREDENTIAL and
refused. Model output inherits source class and projections never lower it.
Direct USER/SCHEDULER/PROACTIVE_WATCHER/SYSTEM callers supply classified
arguments. Descriptor data_class is a maximum; actual request class is exact
and must not exceed it. Runtime classification is primary credential
exclusion; names/schema are defense in depth. Output classification attaches
at a trusted adapter boundary; unknown is CREDENTIAL and P8 closes the detail.

PreparedActionV1 is immutable and carries Task/Step, pinned generation/revision,
capability/version/provider/implementation, validated args/digest, IDK-1,
trusted class, host requester, effective deadline and immutable descriptor
facts. It has no RequestId or execution authority. P6 cannot mutate it. RequestId
is minted per actual P8 dispatch intent; same-Step retry keeps IDK and facts but
gets a new RequestId.

### 17.4 Registry events, migration, and deferred P8 contracts

`CAPABILITY_REGISTRY_CHANGED` covers generation, disabled/removal, experimental
opt-in/out, removal/reactivation. It is metadata only and commits atomically
with its durable mutation in one SQLite transaction through fixed upper-layer
composition. `POLICY_CHANGED` is only policy-rule change.

Migration 0004 later contains registry generations, descriptor revisions,
membership/default/priority metadata, trusted schema reference metadata,
admin overlays, nullable Task generation for legacy rows, and immutable Step
bindings. It contains no policy, approval, tool_call_count, duplicate/repeat,
dispatch, ActionResult, receipt, or reconciliation tables.

Duplicate behavior remains global `(CapabilityId, arguments_digest)`, 24 hours,
effecting capabilities only, version excluded; cross-version suppression is
accepted. P8 consumes one tool-call unit at each durably committed provider
dispatch intent, never refunds it, and does not count preparation, denied or
pending requests, duplicates, or pre-dispatch refusals. NATIVE, EMULATED, and
NONE idempotency implementation remains P8. P8 also closes ActionResult status
matrix, result persistence, receipt/evidence timing/storage, explicit
reconciliation bindings/execution, and dispatch attempt schema. These are not
P5 blockers.

### 17.5 P5 implementation slices after closure

- **P5B:** migration 0004; registry generations, descriptor revisions,
  overlays, Task generation and Step binding storage.
- **P5C:** CapabilityManifestV1, schema catalog/compiler, deterministic
  registry and availability snapshots.
- **P5D:** ToolDefinitionV1, ToolCallProposalV1 validation,
  MODEL_SCHEMA_VIOLATION, classified argument validation, PreparedActionV1.
- **P5E:** TaskEngine generation pinning, step binding, restart/hot-update/
  removal behavior.
- **P5F:** concurrency/recovery/security/crash closure and typed P6 handoff.

No P5 slice invokes providers or implements dispatch/duplicate execution.

### 17.6 Future P5 RED-first test obligations

- **Manifest:** reject unmanifested descriptors and digest mismatch; foreign
  provider namespace; ordinary `host.*`; reversed provider vector order does
  not change selection.
- **Registry:** immutable generations; duplicate exact identity; multiple
  variants and None rule; task pin; old Task/new Step after update; new Task
  after update; removal blocks new binding; bound Step survives removal;
  disable/re-enable; experimental opt-in/out.
- **Version/implementation:** explicit default; prerelease never auto-selected;
  model cannot select version; deterministic priority; root availability
  change; retry cannot switch implementation.
- **Schema:** exact local refs; no network/filesystem; byte limit 65,536; depth
  64; nodes 4,096; properties 256; cyclic refs, open objects,
  patternProperties, unbounded strings/arrays, and ambiguous oneOf rejected.
- **Proposal:** exact V1; unknown field and every host authority injection
  reject whole proposal; sanitized event; stable digest; exact IDK-1.
- **Classification:** trusted PUBLIC/PERSONAL/PRIVATE; unknown -> CREDENTIAL;
  CREDENTIAL refusal; above descriptor ceiling refused; model cannot set/lower
  class.
- **Pinning:** descriptor update, provider disappearance, disable after
  binding, retry/recovery same revision, and no implementation switch.

### 17.7 Closure disposition

Owner decisions remaining for P5: **NONE**. P8 deferred result/receipt/
evidence/reconciliation/dispatch contracts are **NOT P5 BLOCKERS**. Final
status: `READY_FOR_P5_IMPLEMENTATION`. P5B, P6 runtime, and P8 have not started.
