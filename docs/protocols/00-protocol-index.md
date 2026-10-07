# Serea Protocol Index

Status: **FROZEN current contract set** · Architecture version `serea-arch/2.4.0` · Ratified on 2026-10-07

P0/P1 implementation baseline was `serea-arch/0.2.0`; the current frozen registry
is architecture/2.4, task/2, action/2, device/2, event/1, scheduler/1 and
envelope version 1. Three corrected
independent subagent documentation gate reviews under the coordinator are GREEN;
owner direction ratified the design contingent on GREEN. P2A slices are
implemented. The coordinator records current final workspace/MSRV validation,
test counts, bounded regression review and integration status in the
[P2A closure record](../plans/P2A-review-and-closure.md); this index makes no
separate final-count or closure claim.
Architectural acceptance does not claim storage/engine/event runtime delivery.

This document is the naming and versioning authority for every other Serea
contract. When a type, field, or enum appears in more than one place in the
repository, this document and the protocol documents it links are the single
source of truth. Implementation code may not introduce a concept that is not
named here.

---

## 1. Protocol registry

| ID | Document | Owns |
| --- | --- | --- |
| `PROTO-CAP` | [Capability Protocol](01-capability-protocol.md) | `CapabilityDescriptor`, `ActionRequest`, `ActionResult`, `Evidence`, `SideEffectReceipt`, `ActionError` |
| `PROTO-TASK` | [Task Protocol](02-task-protocol.md) | `AssistantTask`, `TaskStep`, task state machine, idempotency keys, duplicate detection |
| `PROTO-MODEL` | [Model Protocol](03-model-protocol.md) | `ModelProvider`, `ModelRequest`, `ModelResponse`, `ModelCapabilities`, structured-output repair |
| `PROTO-POLICY` | [Policy Protocol](04-policy-protocol.md) | `PolicyEngine`, `RiskClass`, `PolicyDecision`, rule evaluation order |
| `PROTO-APPROVAL` | [Approval Protocol](05-approval-protocol.md) | `ApprovalRequest`, `ApprovalGrant`, scope, `max_uses`, expiry, task binding |
| `PROTO-EVENT` | [Event Protocol](06-event-protocol.md) | `SereaEvent`, event kinds, ordering, the Android activity timeline feed |
| `PROTO-DEVICE` | [Device Protocol](07-device-protocol.md) | Pixel ↔ Core transport, pairing, sessions, notifications, reconnection |
| `PROTO-GOALLATCH` | [GoalLatch Adapter Protocol](08-goallatch-adapter-protocol.md) | `HostGoalProvider`, fake semantics, the future real-adapter contract |
| `PROTO-DATA` | [Data Classification Protocol](09-data-classification-protocol.md) | `DataClass`, egress rules, credential redaction |
| `PROTO-BOUNDS` | [Bounds Protocol](10-bounds-protocol.md) | Every bound the host enforces on models, tools, retries, and tasks |
| `PROTO-SCHED` | [Scheduler Protocol](11-scheduler-protocol.md) | Durable schedules, wake events, occurrence identity, recovery, and scheduling authority |

---

## 2. Identifier grammar

All identifiers are opaque to every consumer except the subsystem that mints
them. No consumer may parse identifier structure for meaning.

| Type | Pattern | Example |
| --- | --- | --- |
| `TaskId` | `tsk_` + ULID | `tsk_01JQ8Z9K3M7QWXR4V2T6YH0BNA` |
| `StepId` | `stp_` + ULID | `stp_01JQ8Z9M3R2CVN8H5FWK7PQDSF` |
| `ApprovalId` | `apr_` + ULID | `apr_01JQ8ZA1D4NFG8K2M6RTV9XCWB` |
| `GrantId` | `grt_` + ULID | `grt_01JQ8ZA7B3KMW9Q4TVY7XN2RDP` |
| `RequestId` | `req_` + ULID | `req_01JQ8ZA4H6NFG8K2M6RTV9XCWB` |
| `EventId` | `evt_` + ULID | `evt_01JQ8ZB7H2XKM9P4QW7NRT5YCD` |
| `DeviceId` | `dev_` + ULID | `dev_01JQ8ZC5N8TVG3K6MRQ2XW9JHF` |
| `ScheduleId` | `sch_` + ULID | `sch_01JQ8ZD2P6WKR8T9XVY4NQ3ZMM` |
| `ProposalId` | `prop_` + ULID | `prop_01JQ8ZE8Q3YHF7M2KTW9XN6RPB` |
| `ReceiptId` | `rcp_` + ULID | `rcp_01JQ8ZF4T7KMV2X9NPQ5RD8WCS` |
| `IdempotencyKey` | `idk_` + 64 lowercase hex | `idk_9f2c1a7e4b6d0f8a3c5e9b1d7f2a4c6e8b0d3f5a7c9e1b4d6f8a0c2e4b6d8f9a` |
| `CapabilityId` | `<provider>.<resource>.<verb>` | `calendar.events.list` |
| `ModelId` | `^[a-z0-9]+(-[a-z0-9]+)*$` | `nemotron-3-nano-30b` |
| `Digest` | `sha256:` + 64 lowercase hex | `sha256:3b1f…` |
| `SessionId` | `ses_` + ULID | `ses_01JQ8ZG9V5MXK3N7QW2RTF8YHB` |

Rules that hold for every identifier:

1. **ULID is the only minting scheme.** 26 characters of Crockford Base32,
   48-bit millisecond timestamp, 80 bits of randomness. Lexicographic order
   equals creation order.
2. **Prefixes are part of the wire format**, not decoration. They exist so a
   mis-routed value fails loudly at validation instead of being silently
   accepted by the wrong subsystem.
3. **Identifiers are never reused**, including after deletion, cancellation,
   or task failure. A deleted task's ID stays burned.
4. **Identifiers carry no meaning.** No timestamps may be recovered from a
   `StepId` by a consumer, even though the ULID body is time-ordered.

## 3. Capability identifier grammar

```
CapabilityId := <provider>.<resource>.<verb>

provider  := [a-z][a-z0-9_]{1,31}
resource  := [a-z][a-z0-9_]{1,31}
verb      := [a-z][a-z0-9_]{1,31}
```

Exactly three segments in V1. The split is mandatory:

- `provider` must equal the `ProviderId` of the `CapabilityProvider` that
  registers the descriptor. A provider may not register a capability outside
  its own namespace.
- `resource` names the noun being acted on (`events`, `messages`, `app`,
  `notification`, `goal`).
- `verb` is one of the frozen verb set in
  [Capability Protocol §2](01-capability-protocol.md#2-verb-set).

Adding a verb is an architecture-version-visible change (see §7). Renaming a
provider or resource is a breaking change and requires a new
`CapabilityDescriptor` version plus a deprecation window.

## 4. Versioning

Three independent version axes. They are never collapsed into one number.

| Axis | Form | Governs |
| --- | --- | --- |
| Architecture version | `serea-arch/<major>.<minor>.<patch>` | Contract set as a whole |
| Capability version | SemVer on the descriptor | One capability's input/output contract |
| Wire protocol version | `serea.<surface>/<major>` | A transport or serialization surface |

### 4.1 Semantics

- **Major**: a breaking change to any frozen contract in this registry.
- **Minor**: a backward-compatible addition — a new optional field, a new
  event kind, a new capability, a new risk class, a new non-terminal task
  state.
- **Patch**: editorial only. No contract meaning changes.

ADR-0028 adds backward-compatible closed Scheduler subordinate values and one
structural template-size bound. It advances the architecture from
`serea-arch/2.1.0` to `serea-arch/2.2.0`; `serea.event/1` and
`serea.scheduler/1` remain unchanged.

ADR-0029 adds the required `device_id` member to the closed
`DEVICE_CONNECTED` kind-specific payload and an internal durable task-wait and
resume-wake contract. It advances the architecture to `serea-arch/2.3.0`;
`serea.event/1`, `serea.scheduler/1`, and `serea.task/2` remain unchanged.

ADR-0030 defines the closed approval lifecycle routing payload and durable
Scheduler handoff to future P6. Scheduler materializes and exposes the wake but
does not evaluate approval authority or transition tasks. It advances the
architecture to `serea-arch/2.4.0`; `serea.event/1`, `serea.scheduler/1`,
`serea.approval/1`, and `serea.task/2` remain unchanged.

### 4.2 Compatibility rules

1. A consumer must reject a payload whose major wire-protocol version it does
   not implement. Silent downgrade is forbidden.
2. A provider must accept any `CapabilityDescriptor` version it declares in
   its `supported_capability_versions` set, and reject all others with
   `ActionErrorKind::Validation`.
3. On forward-compatible envelope surfaces, unknown fields are ignored for
   semantic processing and preserved in an opaque extension set for exact
   round-trip auditing. On security-sensitive closed schemas (including
   capability/action inputs), unknown fields are rejected before policy or
   provider execution. Unknown *closed enum variants* fail closed. Open codes such as StepStatus
   retain unknown well-formed values; engine execution still fails closed.
4. A new event kind is a minor change and older clients skip unknown kinds
   rather than failing the stream.
5. A new task state is a minor change. A state transition the host does not
   recognise is a `BLOCKED` task with reason `UNRECOGNISED_STATE`, never a
   crash and never a silent skip.

### 4.2a Per-surface major registry at P2A

| Surface | Supported major |
| --- | --- |
| `serea.action` | 2 |
| `serea.task` | 2 |
| `serea.model`, `serea.policy`, `serea.approval`, `serea.event` | 1 |
| `serea.device` | 2 |
| `serea.goallatch`, `serea.data`, `serea.bounds`, `serea.scheduler` | 1 |

Dispatch/version validation consults this registry per surface, not one global
major value. Envelope version remains 1; `serea.envelope` is not a surface.
Unknown/unsupported surface majors are refused without silent downgrade. This
current registry agrees with the implemented per-surface source/manifest dispatch;
current final workspace/MSRV verification and integration status is recorded by
the coordinator in the [closure record](../plans/P2A-review-and-closure.md).

### 4.3 Frozen for P0

At `serea-arch/0.1.0` the following are frozen and changing any of them
requires an ADR:

- every identifier pattern in §2, including `RequestId`
- the `CapabilityId` three-segment grammar in §3
- the task state machine in [Task Protocol §4](02-task-protocol.md#4-task-state-machine)
- the `RiskClass` and `DataClass` variant sets
- the `ActionErrorKind` variant set
- the `ProviderId` namespace set

## 5. Serialization

- **JSON** for all wire surfaces (device link, provider adapters, event
  payloads, model tool schemas).
- **JSON Schema 2020-12** for every `input_schema` and `output_schema`.
- Field names are `snake_case`. No field name is ever renamed for style; a
  rename is a breaking change with an alias field for one major version.
- Integers are JSON numbers; 64-bit quantities that could exceed
  JavaScript's exact integer range (`seq`, `model_usage` counts) are
  serialized as decimal **strings** and validated on parse.
- Forward-compatible envelope fields unknown to this version are retained in
  an opaque extension set and round-tripped unchanged, but are not interpreted.
  Security-sensitive inner payloads are separately validated against their
  closed schema and reject unknown fields before any authority decision.
- Canonical digest input is **SCJ-1**, defined in
  [ADR-0019](../decisions/ADR-0019-canonical-json-and-idempotency-preimage.md):
  UTF-8-byte key ordering, fixed escaping, no insignificant whitespace/BOM/trailing
  newline, integers only in -2^63 through 2^64-1. Reject exponent/float/-0 and
  duplicates before constructing Value. Scalar roots are valid generic digest
  inputs; ActionRequest arguments remain object-root. A raw-text caller must
  preserve duplicates until validation; no API can reconstruct discarded ones.
- SCJ-1 is a limited canonical domain, not a change to model/1: ModelRequest
  temperature remains f64. Future runtime canonical digest paths refuse model
  documents outside SCJ-1; not every blob/model input is automatically covered.
  Fractional support requires a future decision, never truncation.

## 6. Envelope

Every cross-boundary message — provider call, device message, event,
approval prompt — is wrapped in a common envelope so that transport, auth, and
audit concerns are uniform.

```json
{
  "envelope_version": "1",
  "surface": "serea.action/2",
  "message_id": "evt_01JQ8ZB7H2XKM9P4QW7NRT5YCD",
  "correlation_id": "tsk_01JQ8Z9K3M7QWXR4V2T6YH0BNA",
  "causation_id": "evt_01JQ8Z9M4SBDT6K8H2WNRQVPXF",
  "issued_at": "2026-10-01T09:14:22.418Z",
  "data_class": "PERSONAL",
  "trace": { "task_id": "…", "step_id": "…" },
  "payload": { }
}
```

- `correlation_id` follows the causal chain of the *task*.
- `causation_id` names the specific message that caused this one. Absent only
  for user-originated messages.
- `data_class` is the classification of `payload`, declared by the producer
  and re-verified by the consumer. A consumer that computes a higher class
  than declared must treat the payload as the higher class.
- Envelopes are authenticated, never merely encrypted: the transport layer
  supplies integrity, and `message_id` plus `causation_id` form a verifiable
  chain that the audit trail can walk end to end.

## 7. Change control

Any change to this registry requires:

1. An ADR under `docs/decisions/` recording the change, its motivation, and
   its compatibility impact.
2. An architecture version bump per §4.1.
3. An entry in the affected protocol document's changelog.
4. For `Major`, a migration note naming every consumer that must change.

An implementation PR that introduces a contract change without all four is
rejected at review.

## 8. Cross-references

- Trust boundaries and the authority model:
  [Trust Boundaries](../architecture/02-trust-boundaries.md)
- Assets, adversaries, and mitigations: [Threat Model Index](../threat-model/README.md)
- Where each decision is recorded: [Decision Index](../decisions/README.md)
## 9. Changelog

- 2026-10-07: ADR-0030 adds closed approval lifecycle routing identity and a
  durable Scheduler-to-P6 wake handoff. Architecture advances to
  `serea-arch/2.4.0`; Event, Scheduler, Approval, and Task wire surfaces are
  unchanged.

- 2026-10-06: accepted architecture/2 and device/2 replay semantics under ADR-0026;
  task/2, action/2, event/1 and envelope version 1 remain unchanged. ADR-0018/19/20/23 Accepted in their
  stated architectural/wire/primitive/validation scopes; P2A slices implemented.
  Current validation, test counts, bounded regression review and integration
  status is coordinator-owned in the [closure record](../plans/P2A-review-and-closure.md).
  ADR-0021/22/24 runtime stays Proposed; 0024 wire generation only is implemented.
- 2026-10-06: ADR-0027 accepts the Serea calendar recurrence grammar V1 and
  advances the architecture to `serea-arch/2.1.0`; `serea.scheduler/1` and all
  other wire surfaces are unchanged.
