# Capability Protocol

Protocol ID: `PROTO-CAP` · Surface: `serea.action/2` · Status: **FROZEN current contract**

This protocol defines capability authority and the future provider effect
boundary. P5 prepares an immutable action; P6 authorizes it; P8 is the first
phase permitted to invoke a provider.

---

## 1. Core principle

> A model may request an action. Only the host may cause one.

The execution flow is fixed and has no bypass:

```
ToolCallProposalV1 validation
  -> Task-pinned registry/schema resolution and classified arguments (P5)
  -> immutable PreparedActionV1 (P5)
  -> deterministic policy and approval authorization (P6)
  -> duplicate/repeat/tool-call checks and durable dispatch intent (P8)
  -> provider invoke (P8 only)
  -> result/receipt/evidence/reconciliation (P8 contract closure required)
```

Every arrow is a place where the host can refuse. There is no path from model
text to a side effect that skips one. In particular:

- There is **no** `execute_arbitrary_shell` capability, at any risk class, for
  any model.
- A capability that is not in the registry does not exist. There is no dynamic
  capability registration from model output, plugin discovery, or user
  prompts.
- `ActionRequest` has no field a model can populate to influence routing,
  policy class, approval requirement, or provider selection. Those are
  host-resolved from the `CapabilityDescriptor` and durable task state only.

## 2. Verb set

The frozen `verb` segment of a `CapabilityId`. Verbs are deliberately coarse;
granularity lives in the input schema, not in the identifier space.

| Verb | Meaning | Replay |
| --- | --- | --- |
| `list` | Enumerate existing items. No state change. | Idempotent |
| `read` | Fetch one existing item. No state change. | Idempotent |
| `search` | Query a read index. No state change. | Idempotent |
| `open` | Navigate the user to a surface. User-visible, reversible. | Idempotent |
| `control` | Drive a bounded, enumerated device control (media play/pause/next). | Idempotent per target state |
| `write` | Create or modify external state. | Conditional |
| `create` | Create a new external object. | Conditional |
| `send` | Transmit content to an external recipient. | Non-idempotent |
| `delete` | Remove external state. | Non-idempotent |
| `start` | Begin a host-managed process with a durable handle. | Conditional |
| `status` | Read progress of a host-managed process. | Idempotent |
| `run` | Advance a host-managed process. | Conditional |
| `cancel` | Request termination of a host-managed process. | Conditional |
| `result` | Read the terminal result of a host-managed process. | Idempotent |

Adding a verb is an architecture-minor change and requires an ADR. Removing a
verb or changing its replay semantics is architecture-major.

## 3. `CapabilityDescriptor`

The complete host-owned description of a capability. Providers may advertise
descriptors, but only a trusted host-reviewed `CapabilityManifestV1` authorizes
them. An advertisement must exactly match a manifest entry or that provider
registration fails. Nothing in a model proposal or provider advertisement can
alter descriptor authority. See [ADR-0034](../decisions/ADR-0034-capability-manifest-registry-and-pinning.md).

```json
{
  "id": "calendar.events.list",
  "version": "1.2.0",
  "title": "List calendar events",
  "description": "Lists events in a time range from a selected calendar.",
  "provider_id": "calendar",
  "input_schema": { "$ref": "https://serea.local/schemas/calendar.events.list.input.1.2.0.json" },
  "output_schema": { "$ref": "https://serea.local/schemas/calendar.events.list.output.1.2.0.json" },
  "side_effect_class": "NONE",
  "risk_class": "OBSERVE",
  "required_authorization": "NONE",
  "replay_safety": "IDEMPOTENT",
  "data_class": "PERSONAL",
  "root_requirement": "NOT_REQUIRED",
  "idempotency_support": "NATIVE",
  "max_duration_ms": 15000,
  "cost_class": "FREE",
  "experimental": false
}
```

### 3.1 Field semantics

**`id`** — grammar per [Protocol Index §3](00-protocol-index.md#3-capability-identifier-grammar).
The `provider_id` field must equal the first segment. A mismatch is typed
provider-registration failure, never a panic or runtime warning.

**`version`** — SemVer of this descriptor's input/output contract. The host
manifest explicitly selects the default version for new bindings. The model
never selects a version; providers do not establish authority by declaring
support.

**`input_schema` / `output_schema`** — JSON Schema 2020-12. Validation is
fail-closed on both directions. Output that fails its schema is a provider
fault: the step fails, and the result is **not** passed onward as if valid.

Additional closed-world constraints beyond JSON Schema:

- `additionalProperties: false` on every object.
- No `patternProperties`, no `oneOf` with overlapping branches that would
  permit ambiguous interpretation.
- Cyclic `$ref` graphs are unsupported in P5 V1 and fail registration.
- Every array has `maxItems`.
- Every string has `maxLength`.
- Runtime trusted classification is the primary credential-exclusion control.
  Closed schemas and reviewed property allowlists are defense in depth; a
  property name cannot prove a value safe. Unknown/unclassified arguments are
  `CREDENTIAL` and refused even when schema-valid. See
  [Data Classification Protocol §4](09-data-classification-protocol.md#4-credential-exclusion-and-classified-capability-arguments).

Schema structural limits under ADR-0020 are 65,536 canonical UTF-8 bytes per
document, nesting depth 64, 4,096 schema nodes total, and 256 properties per
object. Only exact trusted local catalog URIs are resolved; no network or
filesystem resolver is permitted. Limits are structural, not B3 work counters;
overflow is typed refusal, never truncation. See
[ADR-0035](../decisions/ADR-0035-tool-proposal-schema-and-prepared-action.md).

Text fields use the exact categories and pinned Unicode White_Space set in
[ADR-0023](../decisions/ADR-0023-text-field-validation-categories.md): O for
ActorId/LeaseOwner/ProviderReference, L for TaskTitle/DescriptorTitle/EffectSummary/
PlainSummary, P for ErrorMessage/DescriptorDescription. O/L preserve all C1 refusals
and reject U+2028/U+2029; P permits interior LF/TAB/U+2028/U+2029 but rejects CR,
other C0, DEL and C1. Every category rejects boundary whitespace and empty input.
Every provider_reference schema occurrence, including nested receipts, is in scope.
Exact O impersonation rejection matches frozen identifier parsing, not near misses.

**`side_effect_class`** — what changes in the world, independent of risk:

| Value | Meaning |
| --- | --- |
| `NONE` | Pure read of durable or cached state. |
| `LOCAL_STATE` | Mutates Serea's own durable state only. |
| `DEVICE_STATE` | Changes observable device UI or device-side settings. |
| `EXTERNAL_WRITE` | Creates or changes state visible outside this host. |
| `COMMUNICATION` | Transmits content to a party outside this host. |
| `ELEVATED_DEVICE` | Requires privileges above the app's normal grant. |

**`risk_class`** — how dangerous, per the frozen
[Policy Protocol](04-policy-protocol.md#2-risk-class) set. A model may not
select, suggest, lower, or annotate it.

**`required_authorization`** — one of `NONE`, `DEVICE_USER`, `SCOPED_GRANT`,
`CREDENTIAL_HANDOFF`. See [Approval Protocol](05-approval-protocol.md).

**`replay_safety`** — one of `IDEMPOTENT`, `CONDITIONAL`, `NON_REPLAYABLE`,
governing automatic retry. See §8.

**`data_class`** — the maximum class this capability is reviewed to transit.
Actual trusted argument class must be less than or equal to this ceiling. See
[Data Classification Protocol](09-data-classification-protocol.md).

**`root_requirement`** — `NOT_REQUIRED`, `OPTIONAL_ROOT`, `REQUIRES_ROOT`.
Capabilities with `OPTIONAL_ROOT` must be registered in both the rootless and
root variants with the *same* `CapabilityId`, differing only in
`implementation_id`. Root absence yields `ActionErrorKind::CapabilityUnavailable`.

**`idempotency_support`** — `NATIVE` (provider dedupes by key), `EMULATED`
(host records the key and suppresses duplicates), or `NONE`.

**`max_duration_ms`** — hard host-side deadline. Exceeding it cancels the call
and records `PROVIDER_TIMEOUT` evidence.

**`cost_class`** — `FREE`, `LOW`, `PAID`. Advisory for policy; never used to
select a model.

### 3.2 Invariants

1. A descriptor is immutable for the lifetime of a task that may reference it.
   Hot-descriptor updates take effect only for tasks created afterwards.
2. A capability's `risk_class` may be raised by an ADR at any time. It may be
   lowered only with an ADR plus a migration that re-evaluates existing grants.
3. Removing a capability from the registry disables it immediately for new
   requests. In-flight steps holding a reference continue against their pinned
   version and complete or fail normally — they are never abandoned
   mid-flight.

## 4. `ActionRequest`

`ActionRequest` is not the model proposal and is not directly parsed from model
JSON. P5 prepares immutable internal `PreparedActionV1`; P6 authorizes it.
The final executable ActionRequest belongs to the future P8 dispatch boundary.

`request_id` is a `RequestId` (`req_` + ULID; see [Protocol Index §2](00-protocol-index.md#2-identifier-grammar)). It is per actual provider dispatch attempt and is echoed by that dispatch's corresponding `ActionResult`. P5 PreparedActionV1 does not contain a RequestId; P8 mints a new one for every committed dispatch intent, including same-Step retry. The same Step retains IDK-1 and pinned facts.

```json
{
  "request_id": "req_01JQ8ZA4H6NFG8K2M6RTV9XCWB",
  "task_id": "tsk_01JQ8Z9K3M7QWXR4V2T6YH0BNA",
  "step_id": "stp_01JQ8Z9M3R2CVN8H5FWK7PQDSF",
  "capability_id": "calendar.events.list",
  "capability_version": "1.2.0",
  "arguments": { "range": "tomorrow" },
  "arguments_digest": "sha256:…",
  "idempotency_key": "idk_9f2c…",
  "data_class": "PERSONAL",
  "requested_by": "MODEL",
  "deadline_ms": 15000
}
```

### 4.1 `requested_by`

An enum, never a string from model output:

| Value | Meaning |
| --- | --- |
| `MODEL` | Synthesized from validated model output. Authority: zero. |
| `USER` | Directly instructed by the device user in this session. |
| `SCHEDULER` | Triggered by a durable schedule wake. |
| `PROACTIVE_WATCHER` | Triggered by the read-only watcher. |
| `SYSTEM` | Host-internal maintenance (sync, retention). |

`requested_by` records provenance for audit. It **never** grants authority.
A `MODEL`-requested `calendar.event.create` requires exactly the same
approval as a `USER`-requested one.

### 4.2 Host-resolved fields

These are set by the host after validation and are **absent** from
model-authored input. A model that emits them has produced an invalid
  request; the whole proposal is rejected and a sanitized
  `MODEL_SCHEMA_VIOLATION` event is recorded. No action is prepared.

`capability_version`, `risk_class`, `side_effect_class`,
`required_authorization`, `provider_id`, `arguments_digest`, `data_class`,
`deadline_ms`.

### 4.3 `arguments_digest`

`sha256` over the canonical JSON of `arguments`
([Protocol Index §5](00-protocol-index.md#5-serialization)). Used for
duplicate detection and idempotency derivation. Two requests with the same
`capability_id` and the same `arguments_digest` are *equivalent actions*
regardless of who requested them.

## 5. `ActionResult`

Provider-result status semantics, receipt timing, evidence persistence, and
reconciliation execution are future P8 contract-closure work before the first
provider invocation. The following existing definitions do not authorize P5 or
P6 execution and must be reconciled by P8 closure; P5A freezes only the
non-negotiable invariants in ADR-0036.

The result echoes the originating request's `RequestId`.

```json
{
  "request_id": "req_01JQ8ZA4H6NFG8K2M6RTV9XCWB",
  "status": "SUCCEEDED",
  "output": { "events": [ ] },
  "output_digest": "sha256:…",
  "evidence": [ { "evidence_id": "evt_…", "kind": "PROVIDER_RECEIPT", "produced_at": "2026-10-01T09:14:23.902Z" } ],
  "receipt": null,
  "error": null,
  "duration_ms": 412
}
```

`status` is one of `SUCCEEDED`, `FAILED`, `REJECTED`, `CANCELLED`,
`UNAVAILABLE`, `DUPLICATE_SUPPRESSED`.

`UNAVAILABLE` is a **first-class success-adjacent outcome**, not an error
path: it means the capability exists but its backing condition is absent
(offline device, missing root, revoked credential, provider outage). It is
structurally distinguishable so the system can degrade gracefully rather than
retrying a call that cannot succeed. `DUPLICATE_SUPPRESSED` means an
equivalent action already completed and its receipt was returned; no new effect
occurred.

### 5.1 `receipt`

Non-null exactly when `status == "SUCCEEDED"` and `side_effect_class !=
"NONE"`. A receipt is the only accepted proof that an externally visible
effect occurred. A `SUCCEEDED` result with `side_effect_class != "NONE"` and a
null receipt is a **host invariant violation** and aborts the task to
`BLOCKED`.

Receipt shape is defined per capability family; the common envelope is:

```json
{
  "receipt_id": "rcp_…",
  "capability_id": "gmail.drafts.create",
  "idempotency_key": "idk_9f2c…",
  "provider_reference": "r-3f9a…",
  "effect_summary": "Created draft in mailbox primary",
  "observed_at": "2026-10-01T09:14:23.880Z",
  "replay_safe": true
}
```

`provider_reference` is the external system's own handle for the created or
changed object — the identifier needed to reconcile later. A receipt without
one is valid only for `side_effect_class: LOCAL_STATE`.

## 6. `ActionError`

```json
{
  "kind": "PROVIDER_ERROR",
  "code": "GMAIL_HISTORY_EXPIRED",
  "message": "Stored historyId 98211 is no longer available",
  "retryable": false,
  "host_action": "FULL_RESYNC",
  "details": { }
}
```

### 6.1 Frozen `ActionErrorKind` set

| Kind | Retryable | Meaning |
| --- | --- | --- |
| `VALIDATION` | no | Arguments failed schema or host-side validation. |
| `UNKNOWN_CAPABILITY` | no | Not in the registry, or version unsupported. |
| `POLICY_DENIED` | no | Policy engine returned `DENY`. |
| `APPROVAL_REQUIRED` | no | Approval needed and not granted. Transitions task to `WAITING_APPROVAL`. |
| `APPROVAL_DENIED` | no | Human rejected the request. Terminal for the step. |
| `CAPABILITY_UNAVAILABLE` | no | Backing condition absent (offline, no root, revoked). |
| `PROVIDER_ERROR` | depends on `retryable` | External provider failed. |
| `PROVIDER_TIMEOUT` | yes | Deadline exceeded. |
| `RATE_LIMITED` | yes | Provider throttled. Host backs off. |
| `AUTH_EXPIRED` | no | Credential needs re-authorization. |
| `DUPLICATE_SUPPRESSED` | n/a | Equivalent action already completed. |
| `AMBIGUOUS` | **no** | Effect may or may not have occurred; unknown. |
| `INTERNAL` | no | Host fault. |

### 6.2 The `AMBIGUOUS` rule

This is a future P8 requirement. P5/P6 do not invoke or reconcile. Exact
reconciliation binding, result statuses, receipt timing, evidence persistence,
and recovery state are deferred to P8 contract closure before first provider
invocation. There is no capability-name inference, provider-selected target,
or synthesized host receipt. TaskEngine remains lifecycle owner.

`AMBIGUOUS` is the most consequential kind in the protocol. It means the
provider cannot determine whether the effect happened — a dropped connection
after a write, a timeout with no response, an ambiguous provider status.

**Rule:** an ambiguous effect is never blindly retried. P8 must define
host-reviewed reconciliation bindings and typed occurred/absent/unknown results
before invoking providers. A read-back cannot manufacture a provider receipt;
unknown remains blocked for human resolution under the eventual P8 contract.

Blunt retry on `AMBIGUOUS` is the single highest-severity anti-pattern in this
system, because it converts one uncertain effect into two certain ones.

## 7. Evidence

Evidence is the append-only record of what was attempted and observed. Its `kind` is selected from this frozen capability-owned vocabulary:

| Evidence kind | Meaning |
| --- | --- |
| `ACTION_ATTEMPTED` | Host recorded an action attempt, including a denied attempt. |
| `PROVIDER_RECEIPT` | Provider supplied a receipt proving an effect. |
| `CAPABILITY_OBSERVATION` | Provider returned a validated observation without an external effect. |
| `GOAL_RESULT` | HostGoalProvider returned a validated delegated-goal result. |
| `POLICY_DENIAL` | Policy refused the action request. |
| `RECONCILIATION` | A read-back resolved an ambiguous effect. |

This evidence-kind vocabulary is distinct from `SereaEvent.kind`; the [Event Protocol](06-event-protocol.md) owns event kinds, not evidence kinds.
 It is
separate from receipts because evidence exists even for actions that produced
no effect (a denied call is evidence; a successful read is evidence).

Every evidence record carries: `evidence_id`, `kind`, `capability_id`,
`task_id`, `step_id`, `attempt`, `produced_at`, `actor`, `data_class`,
`payload_digest`, `payload_reference`.

Evidence is what the activity timeline renders and what a post-incident audit
walks. It is never human-readable log text — it is structured data with a
stable schema.

## 8. Replay and idempotency

### 8.1 `replay_safety` values

| Value | Automatic retry on `retryable` failure | Retry on `AMBIGUOUS` |
| --- | --- | --- |
| `IDEMPOTENT` | Allowed | Allowed |
| `CONDITIONAL` | Only when the provider confirms no effect; replay the same step with the same key for transport retry | After confirmed-absent, close the old attempt and create a new `StepId` and key under fresh policy/approval |
| `NON_REPLAYABLE` | Never | Never; reconcile or block |

### 8.2 Idempotency key derivation

The key uses the domain-separated **IDK-1** encoding in
[ADR-0019](../decisions/ADR-0019-canonical-json-and-idempotency-preimage.md#idk-1-the-idempotency-preimage):
21-byte domain tag `serea.idempotency.v1\0`, u8 field count 5, then u64-big-endian
length-prefixed field **name AND value** in order task_id, step_id, capability_id,
capability_version, arguments_canonical. Arguments are canonical SCJ-1 bytes; hash
with SHA-256 and render idk_ plus 64 lowercase hex. Framing is injective; no claim
of mathematical SHA-256 injectivity. Typed public derivation validates IDs and
accepts any SCJ-1 root with valid identifiers (including pp.rr.list scalar roots).
Invalid p.r.list historical vectors belong only in low-level private raw-string
framing tests; public derivation refuses that ID. ActionRequest separately
requires object-root arguments.

Key is required only for CAPABILITY, DELEGATE and VERIFY steps; other kinds derive
no key. Approval checks argument digest/capability/pinned version independently;
key equality does not transfer approval. Full SCJ-1/digest/IDK-1/sha2 0.11 without
defaults land with action/2 atomically in P2A, not P2B.

The key is derived from the *request*, not from the attempt. Every attempt of
the same step produces the same key. This is what makes duplicate detection
and crash recovery work: after a restart, a step that may have executed is
re-issued with the same key, and the provider or host dedupe layer recognises
it.

Crash replay or a transport retry of the same durable step always uses the
same key. If reconciliation confirms an ambiguous effect is absent and policy
permits another execution, the old step is closed as `RECONCILED_ABSENT` and
the host creates a distinct `step_id`, hence a distinct key. That replacement
step must pass policy and any required approval again. The host never reuses a
key to mean "again".

### 8.3 Duplicate detection

This describes a future P8 execution gate; P5/P6 do not implement it. The
duplicate key is global across Tasks and is exactly
`(CapabilityId, arguments_digest)` for effecting capabilities during the
86,400,000 ms window. Version is intentionally excluded, so cross-version
suppression is accepted. See [ADR-0036](../decisions/ADR-0036-p5-p6-p8-authorization-and-dispatch.md).

Before invoking a provider, the host checks whether a step with the same
`(capability_id, arguments_digest)` has already completed in this task, or in
any task within the duplicate window. If so:

1. The host does **not** invoke the provider.
2. It returns the prior `ActionResult` with `status: DUPLICATE_SUPPRESSED`,
   preserving the original receipt.
3. It emits `CAPABILITY_DUPLICATE_SUPPRESSED`.

The duplicate window is a host-configured bound
([Bounds Protocol §5](10-bounds-protocol.md#5-duplicate-suppression)), not a
per-call decision. Within the window, repeated equivalent actions are
detectable by construction.

### 8.4 Per-step attempt ceiling

Every step has a hard attempt ceiling and a per-attempt backoff. Exhausting
the ceiling moves the task to `FAILED` with the last error preserved. Ceilings
are host bounds, never model-supplied.

## 9. Provider interface

```rust
#[async_trait]
pub trait CapabilityProvider: Send + Sync {
    fn provider_id(&self) -> ProviderId;

    fn capabilities(&self) -> Vec<CapabilityDescriptor>;

    async fn invoke(
        &self,
        request: &ActionRequest,
        ctx: &ProviderContext,
    ) -> Result<ActionResult, ActionError>;

    async fn health(&self) -> ProviderHealth { /* default: Ready */ }
}
```

`ProviderContext` carries the deadline, cancellation token, the resolved
`CapabilityDescriptor`, and a `CredentialHandle` — a reference to a secret in
the OS credential store, never the secret itself. A provider receives no
ambient authority: it cannot read policy, approve itself, escalate, or reach
another provider's credentials.

A provider that cannot honour its declared descriptor — because a backing API
changed shape — fails closed: it marks itself `Degraded`, stops advertising
the capability, and the registry treats it as `UNAVAILABLE`. It does not
return loosely-shaped data and hope the schema check downstream is lenient.

## 10. Capability registry and P5/P6/P8 boundary

The trusted host `CapabilityManifestV1`, not provider advertisement, is
authoritative for capabilities that may exist. Provider advertisements must
exactly match manifest entries; unmanifested descriptors fail registration.
Registry generations, descriptor revisions, task snapshot semantics, live
overlays, deterministic version/implementation selection, and
`CAPABILITY_REGISTRY_CHANGED` are defined by [ADR-0034](../decisions/ADR-0034-capability-manifest-registry-and-pinning.md).

P5 validates closed `ToolCallProposalV1`, trusted schemas and classification,
and produces immutable `PreparedActionV1`; it does not evaluate policy,
approval, duplicate/repeat execution checks, tool-call accounting, reserve
dispatch, invoke providers, accept results, or reconcile. P6 owns policy,
approval/grant lifecycle, and typed authorization of PreparedActionV1; it also
does not invoke providers. P8 is first permitted to invoke, after its result,
receipt, evidence, dispatch, and reconciliation contract closure. See
[ADR-0035](../decisions/ADR-0035-tool-proposal-schema-and-prepared-action.md)
and [ADR-0036](../decisions/ADR-0036-p5-p6-p8-authorization-and-dispatch.md).

P5 migration 0004 is registry/binding-only. It contains no policy, approval,
tool-call count, duplicate/repeat reservation, provider dispatch, result,
receipt, or reconciliation tables. `serea.action/2` remains unchanged.

## 11. Invariants summary

These are the security and correctness properties this protocol guarantees. Each
is independently testable and each has a named test obligation in the phase
plans.

| # | Invariant |
| --- | --- |
| C1 | No model output can cause a side effect without passing schema validation, registry lookup, policy evaluation, and approval. |
| C2 | No capability performs arbitrary command or shell execution. |
| C3 | `risk_class` is host-owned and model-uninfluenced. |
| C4 | An externally visible effect is proven only by a provider-produced receipt. |
| C5 | `AMBIGUOUS` results are never blindly retried; they reconcile or block. |
| C6 | Identical `(capability_id, arguments_digest)` within the duplicate window never produces two effects. |
| C7 | Provider failure to honour its descriptor degrades to `UNAVAILABLE`, never to a laxer result. |
| C8 | Credentials reach providers only as opaque `CredentialHandle`s. |
| C9 | Root absence yields a structured `CAPABILITY_UNAVAILABLE`, never a crash or degraded startup. |
| C10 | Capability removal disables new requests without stranding in-flight steps. |
## 11. P2A migration note and changelog

- 2026-10-03: frozen current action/2 with implemented SCJ-1 limited integer
  domain, named IDK-1 framing and complete text-category validation (Accepted
  ADR-0019/23). Accepted B3 semantic clarification is architecture-minor in
  isolation (ADR-0020), not patch. The coordinator records current final
  workspace/MSRV validation, test counts, review and integration status in the
  [closure record](../plans/P2A-review-and-closure.md). No runtime delivered.
- Both consumer migration notes are in the
  [launch package](../plans/P2-6.1-sol-launch.md#4-migration-note-drafts).
  Model temperature remains f64 on model/1; no automatic canonical model coverage.
