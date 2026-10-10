# ADR-0040: P6 durable storage, retention, bounds, and crate ownership

Status: **Proposed** · Date: 2026-10-10 · Architecture: `serea-arch/2.6.0` → `2.7.0` if accepted

Surfaces affected: `serea.bounds/1` gains four proposed bounds; architecture
`docs/architecture/03-crate-map.md` and a `serea-protocol` module comment are corrected.
No wire-major change.

## Context

The P6 audit recorded D17, D18 and D20. All three are ownership questions that were left open
because they were entangled.

D20 is the entangled one. Four places disagree about who owns the approval types:

| Source | Says |
| --- | --- |
| `crates/serea-protocol/src/lib.rs:39-45` (comment) | `PolicyDecision` and `DenyReason` owned by `serea-policy`; `ApprovalRequest` and `ApprovalGrant` owned by `serea-capability` |
| `docs/architecture/03-crate-map.md:186` | `PolicyEngine`, `PolicyDecision`, `PolicyContext`, `AutomationContext`, `RuleStore`, `PolicyChange` owned by `serea-policy` |
| `docs/architecture/03-crate-map.md:182` | `serea-protocol`'s public API lists `PolicyDecision`, `DenyReason`, `ApprovalRequest`, `ApprovalGrant` |
| `docs/architecture/03-crate-map.md:108,188` | `serea-capability --> serea-policy` is a future edge, and `serea-capability`'s P6 handoff "may depend on `serea-policy`" |

Only one of these is mechanically enforced, and it decides the question.
`tests/workspace_smoke.py:370` fails the build if `serea-capability` depends on
`serea-policy`. The `CAP --> POLICY` edge cannot exist, so the `lib.rs` comment describes an
impossible graph, and putting `ApprovalRequest` and `ApprovalGrant` in `serea-capability` is
not a design option at all.

The same file permits `serea-task-engine` to depend on protocol, storage, event-bus and
capability, and nothing else. A `task-engine -> policy` edge needs one added row.

## Options

**A — `serea-policy` owns the runtime policy and approval types.** The evaluator, rule store,
`PolicyDecision`, `DenyReason`, `ApprovalRequest`, `ApprovalGrant`, `ApprovalGrantUse`, the
lifecycle, and the authenticated response seam in one crate.

**B — `serea-protocol` owns neutral records; `serea-policy` owns behaviour.** Rejected. It
moves runtime records with live counters into the frozen contract crate, which by its own
words holds "no orchestration, no persistence", and it makes any change to a grant a wire
change.

**C — `serea-capability` owns the approval types.** Rejected twice: mechanically forbidden,
and wrong in principle, since ADR-0035 exists to keep authority out of the capability crate.

## Decision

**Proposed: Option A, with no `serea-capability -> serea-policy` edge.**

Resulting dependency shape:

```text
serea-capability --> serea-protocol, serea-storage, serea-event-bus
serea-policy      --> serea-protocol, serea-storage, serea-event-bus
serea-task-engine --> serea-protocol, serea-storage, serea-event-bus,
                     serea-capability, serea-policy      (one new edge)
serea-scheduler   --> serea-protocol, serea-storage, serea-event-bus, serea-task-engine
```

`crates/serea-task-engine/src/engine.rs:181` already calls
`serea_capability::prepare_action`, so the P6 composition is a straight extension of an
existing seam rather than a new orchestration:

```text
TaskEngine
  -> serea-capability::prepare_action(...)   -> PreparedActionV1 (immutable)
  -> build PolicyInputV1 from Task + PreparedActionV1
  -> serea_policy::evaluate(input, current revision)
  -> typed authorization outcome
```

`PolicyInputV1`, `PolicyDecision`, `DenyReason`, `ApprovalRequest`, `ApprovalGrant`,
`ApprovalGrantUse`, `AuthenticatedApprovalResponseV1` and `AuthorizationEvidenceV1` all live
in `serea-policy`. `ApprovalLifecyclePayloadV1` stays in `serea-protocol`, unchanged.

`PolicyInputV1` carries the prepared identity, the trusted classified facts, the Task
snapshot, the trusted context, and the policy revision identity. It carries **no raw
arguments**, because Policy §6 forbids model-derived input and a rule language over raw
arguments is an interpreter. It carries **no `now`**, because Policy §6 forbids wall-clock
time in policy; expiry is the approval layer's job.

The P6 output type is named `AuthorizationEvidenceV1` and not `AuthorizedAction`, because the
latter reads as a permanent execution permission, which ADR-0036 forbids P6 from returning.
It carries no `approved` boolean and no `RequestId`. P8 revalidates every mutable fact
independently.

## Storage and retention

Migration **0005 is not created here.** The proposed conceptual shape is six tables:
`policy_revisions`, `policy_rules`, `policy_state`, `approval_requests`, `approval_grants`,
`approval_grant_uses`. `policy_state` is a singleton whose pointer only advances, mirroring
`capability_registry_state` in migration 0004.

Deliberately absent: dispatch intents, `RequestId`, provider attempts, duplicate suppression,
repeated-action counters, `ActionResult`, receipts, reconciliation state, and a
`policy_evaluations` table. An evaluation log would duplicate what the immutable revision and
the request row already prove.

Deletion: approval tables cascade with the Task; policy tables never cascade, matching
[Event Protocol §8](../protocols/06-event-protocol.md), which already gives policy events one
year and states that policy history outliving its task is intentional.

## Bounds

Four new bounds are proposed, because each guards a resource no existing bound names. Their
values are product preferences and are listed here as recommendations only.

| Bound | Proposed default | Rationale |
| --- | --- | --- |
| `max_active_policy_rules` | 512 | Bounds the rule table and therefore the candidate set per evaluation. A capacity guard, not a design target |
| `max_retained_policy_revisions` | 64 | Bounds growth while retaining enough history for the one-year `POLICY_CHANGED` audit class |
| `approval_grant_expiry_ms` | 1800000 | A grant must not outlive its request; equal horizons make that structural |
| `approval_grant_max_uses` | 8 | Caps `max_uses`. The protocol's own batch example is 3; this is a ceiling, not a promise of use |

Existing bounds are reused rather than duplicated: `max_pending_approvals_per_task = 5`,
`approval_request_expiry_ms = 1800000`, `max_event_payload_bytes = 32768`,
`max_events_per_transaction = 16`, `task_retention_days = 30`.

No summary byte bound and no per-evaluation candidate bound are proposed. The first is
already bounded by `max_event_payload_bytes` and by `PlainSummary`'s `Label` category
validation; the second is bounded by `max_active_policy_rules`.

## Consequences

- Four contradictory ownership statements collapse to one, and the graph becomes acyclic by
  construction.
- Creating `serea-policy` in P6B requires one added row in `tests/workspace_smoke.py` and one
  new non-dev dependency edge in `serea-task-engine/Cargo.toml`. Nothing else changes shape.
- The `lib.rs` comment and the Crate Map ownership lines must be corrected in the same slice,
  or the contradiction returns.

## Verification obligations (future P6B)

- `cargo metadata --no-deps --format-version 1` acyclic.
- `tests/workspace_smoke.py` green with the new row.
- Rollback, reopen, foreign-key and integrity tests on migration 0005.
- No `invoke(` in `serea-policy`, `serea-capability` or `serea-task-engine`.

## Status

**Proposed. Not accepted. No `serea-policy` crate, no migration 0005, and no runtime exists.**
