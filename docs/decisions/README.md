# Serea Decision Index

Current frozen architecture: `serea-arch/2.6.0`; P0/P1 baseline
`serea-arch/0.2.0` remains historical. P2 is closed. P3 is closed on the open
`main` branch. Current P3 validation results and scope limits belong in the
[P3 closure record](../plans/P3-closure.md). ADR-0021 remains Proposed as an
architecture decision; ADR-0022/24 remain Proposed for their unclosed scopes.
P4A contract closure is accepted by ADR-0031 through ADR-0033; P4 runtime has
not started. P5A owner-decision closure is accepted by ADR-0034 through
ADR-0036; P5 runtime has not started. P6A contract closure is accepted by ADR-0037
through ADR-0040, following owner ratification of the P6 recommended package on
2026-10-10 and the additional owner decision **R2 — enumerated multi-action grant**
the same day, which resolved the P6A feasibility gate BLOCKER. P6 runtime has not
started. See [P6A-feasibility-gate.md](../plans/P6A-feasibility-gate.md).

## P6 decisions (accepted 2026-10-10)

Prepared for owner ratification; see the
[P6 owner decision package](../plans/P6-owner-decision-package.md). None of these is
Accepted, and none authorizes runtime work.

| ADR | Decision | Status |
| --- | --- | --- |
| [ADR-0037](ADR-0037-policy-semantics-and-revision-authority.md) | Class defaults as a fixed code constant; `priority DESC, rule_id ASC` precedence; `DENY` overrides `ALLOW`; closed match dimensions; monotonic revision plus digest; SQLite as sole authority | **Accepted** |
| [ADR-0038](ADR-0038-approval-identity-lifecycle-and-authenticated-response.md) | Grant binds generation and descriptor digest; **R2 enumerated multi-action grant over 1–8 individually approved Steps** with a normalized action-set digest and immutable membership; provider and implementation are transparency only; two-variable state machine; typed duplicate-response matrix; grant-only revocation with a Proposed `APPROVAL_REVOKED` event; authenticated response seam with clamping; `PERSONAL` persisted ceiling | **Accepted** |
| [ADR-0039](ADR-0039-p6-p8-revalidation-and-grant-consumption-boundary.md) | Grant consumption joins the P8 dispatch-intent transaction, reclassified from a P6 blocker to a P8 contract decision | **Accepted** |
| [ADR-0040](ADR-0040-p6-storage-retention-bounds-and-crate-ownership.md) | `serea-policy` owns policy and approval types; no `serea-capability -> serea-policy` edge; `PolicyInputV1` with no raw arguments and no `now`; `AuthorizationEvidenceV1`; eight-table migration 0005 concept including the R2 action and grant membership tables; four new bounds | **Accepted** |

## Accepted decisions

| ADR | Decision | Status |
| --- | --- | --- |
| [ADR-0001](ADR-0001-core-owns-orchestration.md) | Serea Core owns orchestration; GoalLatch is only a provider | Accepted |
| [ADR-0002](ADR-0002-structured-action-requests.md) | Structured `ActionRequest`; models have no authority | Accepted |
| [ADR-0003](ADR-0003-ollama-cloud-model-roles.md) | Ollama Cloud role roster; Codex excluded from normal routing | Accepted |
| [ADR-0004](ADR-0004-assistant-task-distinct-from-goal.md) | `AssistantTask` is distinct from GoalLatch `Goal` | Accepted |
| [ADR-0005](ADR-0005-sqlite-wal-and-migrations.md) | SQLite in WAL mode with ordered migrations | Accepted |
| [ADR-0006](ADR-0006-sqlite-fts5-memory.md) | SQLite + FTS5 memory; no vector database | Accepted |
| [ADR-0007](ADR-0007-rootless-first-android.md) | Rootless-first Android, isolated optional root variants | Accepted |
| [ADR-0008](ADR-0008-event-driven-durable-scheduler.md) | Event-driven scheduler with durable schedules and wake state | Accepted |
| [ADR-0009](ADR-0009-bounded-task-bound-approval-grants.md) | Approval grants are bounded and task-bound | Accepted |
| [ADR-0010](ADR-0010-data-classification-and-credential-exclusion.md) | Data classification is host-enforced; credentials are excluded | Accepted |
| [ADR-0011](ADR-0011-fake-goallatch-until-integration-gate.md) | Fake GoalLatch only until the explicit integration gate | Accepted |
| [ADR-0012](ADR-0012-capability-provider-protocol-boundary.md) | Capability/provider protocol is the sole effect boundary | Accepted |
| [ADR-0016](ADR-0016-proactive-watcher-is-read-only.md) | Proactive watcher is read-only | Accepted |
| [ADR-0017](ADR-0017-deletion-cascade-completed-event-kind.md) | `DELETION_CASCADE_COMPLETED` is a registered `EventKind` | Accepted |
| [ADR-0025](ADR-0025-p3-event-participant-composition.md) | Fixed two-participant event/journal composition inside the Storage transaction | **Accepted**; implemented in merged P3 |
| [ADR-0031](ADR-0031-model-roster-routing-and-egress-v1.md) | Model roster, routing requirements, and egress V1 | **Accepted**; architecture `serea-arch/2.5.0`; model and bounds surfaces unchanged |
| [ADR-0032](ADR-0032-model-dispatch-durability-and-accounting-v1.md) | Model dispatch durability, ambiguity, and accounting V1 | **Accepted**; migration 0003 concept only; event/model/bounds surfaces unchanged |
| [ADR-0033](ADR-0033-structured-validation-repair-and-fallback-v1.md) | Structured validation, repair, and fallback V1 | **Accepted**; architecture `serea-arch/2.5.0`; model and bounds surfaces unchanged |
| [ADR-0034](ADR-0034-capability-manifest-registry-and-pinning.md) | Host manifest, immutable registry generations, descriptor/implementation pinning and overlays | **Accepted**; architecture `serea-arch/2.6.0`; no wire-major change |
| [ADR-0035](ADR-0035-tool-proposal-schema-and-prepared-action.md) | Closed tool proposal, trusted schema catalog, classification and PreparedActionV1 | **Accepted**; architecture `serea-arch/2.6.0`; action/2 unchanged |
| [ADR-0036](ADR-0036-p5-p6-p8-authorization-and-dispatch.md) | P5/P6/P8 boundary, RequestId, accounting and execution deferral | **Accepted**; architecture `serea-arch/2.6.0`; P8 decisions are not P5 blockers |

## P2 decision disposition

Owner instruction on 2026-10-03 ratifies the following after the corrected docs
gate GREEN and coordinator implementation/scoped tests. Architectural acceptance and
implemented wire validation do not establish deferred runtime or full workspace/
MSRV verification. A green wire-member gate does not accept full runtime fencing.

| ADR | Decision | Status / implementation phase |
| --- | --- | --- |
| [ADR-0018](ADR-0018-taskstep-lifecycle-and-field-presence.md) | Four Option conversions, seven unconditional fields, open wire status and lifecycle presence | **Accepted**, P2A wire and scoped P2F/P2G runtime lifecycle/recovery implemented; no claim beyond the closed P2 phase scope |
| [ADR-0019](ADR-0019-canonical-json-and-idempotency-preimage.md) | Full SCJ-1/digest/IDK-1 primitives, sha2 0.11 no defaults | **Accepted**, full SCJ-1/digest/duplicate-aware parsing/IDK-1 implemented in P2A |
| [ADR-0020](ADR-0020-bounds-b3-scope-clarification.md) | B3 operational versus structural semantic clarification | **Accepted**, semantic B3 clarification; architecture-minor in isolation, no resource bounds |
| [ADR-0021](ADR-0021-p2-p3-event-atomicity-seam.md) | Shared-receiver immutable-successful-transition seam; E3 forward only, no backfill | **Proposed**; P3 runtime is merged; the historical ADR has not been ratified |
| [ADR-0022](ADR-0022-durable-private-data-at-rest.md) | Fail-closed PRIVATE at-rest dispatch | **Proposed**; P2D fail-closed blob dispatch implemented; real backend/key custody and ordinary-row PRIVATE representation outstanding |
| [ADR-0023](ADR-0023-text-field-validation-categories.md) | Complete O/L/P validation, pinned whitespace, exact identifier subtraction | **Accepted**, complete O/L/P validation implemented in P2A |
| [ADR-0024](ADR-0024-lease-fencing-and-commit-under-lease.md) | Authoritative unreleased lease fencing and revocation | **Proposed**; P2E authority, P2F outcome/engine integration and P2G recovery implemented; full architecture ratification remains outstanding |

## P3 decisions

| ADR | Decision | Status |
| --- | --- | --- |
| [ADR-0026](ADR-0026-event-retention-and-global-sequence.md) | Minimal sequence ledger, independently expirable content, bounded range-aware replay | **Accepted**; Option A. Architecture `serea-arch/2.0.0`, replay response `serea.device/2`, event objects `serea.event/1`. |
| [ADR-0027](ADR-0027-calendar-recurrence-grammar-v1.md) | Serea-owned structured calendar recurrence grammar V1 | **Accepted**; ONCE/DAILY/WEEKLY, architecture `serea-arch/2.1.0`; Scheduler surface unchanged. |
| [ADR-0028](ADR-0028-scheduler-predicate-and-template-v1.md) | Exact EventPredicateV1 and ScheduledTaskTemplateV1 contracts | **Accepted**; architecture `serea-arch/2.2.0`; Event and Scheduler surfaces unchanged. |
| [ADR-0029](ADR-0029-device-session-task-resume-waits.md) | Explicit durable device-session task resume waits | **Accepted**; architecture `serea-arch/2.3.0`; Event, Scheduler, and Task surfaces unchanged. |
| [ADR-0030](ADR-0030-durable-approval-lifecycle-wake.md) | Durable approval-lifecycle wake handoff to future P6 | **Accepted**; architecture `serea-arch/2.4.0`; Event, Scheduler, Approval, and Task surfaces unchanged. |

### Numbering note

`ADR-0013` through `ADR-0015` are absent from this repository. This index does not
know whether they were never allocated or were withdrawn, so the P2 proposals
continue from `ADR-0018` rather than filling the gap. Reusing a withdrawn number
would be the renumbering that
[Trust Boundaries §1](../architecture/02-trust-boundaries.md#1-boundary-index)
forbids for its own registry, applied here by the same reasoning.

### The P2A version plan, recorded once

The P2 autonomous audit found that these seven ADRs had taken **three different
positions** on the architecture-version treatment — ADR-0018 deferred, ADR-0019
self-classified as a "minor clarification", ADR-0023 deferred — and that ADR-0019's
position was wrong in a specific way: its integer-only number rule **narrows** the
frozen Protocol Index §5 sentence "numbers in shortest round-trip form", and a
narrowing is not a clarification.

One plan now applies to all of them, derived from Protocol Index §4.1 and §7:

| Axis | From | To | Driver |
| --- | --- | --- | --- |
| Architecture | `serea-arch/0.2.0` | `serea-arch/1.0.0` | ADR-0018 breaks a frozen contract; §7 item 2 requires the bump |
| Task surface | `serea.task/1` | `serea.task/2` | ADR-0018's relaxation; ADR-0024's `lease_generation` rides along |
| Action surface | `serea.action/1` | `serea.action/2` | ADR-0019's canonicalization narrowing; ADR-0023's `message` widening |
| Event surface | `serea.event/1` | unchanged | ADR-0023's `actor.id` change is a validation tightening; ADR-0021 changes no event shape |

ADR-0020 is an **architecture-minor semantic clarification**, not patch; no
resource bounds are introduced. At P2A, ADR-0021/22 runtime work was proposed
and outside that phase; P3 runtime status is summarized above. No event or data
wire major is raised.

**Current contracts use architecture/2.4, task/2, action/2, device/2, event/1,
and scheduler/1.** The runtime status of each P3 contract is recorded in the P3
closure; P3 is closed on the open candidate branch and not yet integrated into
`main`.
The design-preparation/audit phases did not change production; the coordinator
has implemented P2A slices, including the exact numeric follow-up. Final
workspace/MSRV validation, smoke, bounded regression review,
Python/docs/metadata/diff checks and atomic integration status are tracked in the
[closure record](../plans/P2A-review-and-closure.md); this index makes no separate
final-count or closure claim. Historical ADR headers and dated P0/P1 evidence
retain their versions.
Full reasoning: [the audit's M6](../plans/P2-autonomous-audit.md); phase-specific
status: [the decision ledger](../plans/P2-tomorrow-decision-ledger.md).

## Phase plans

- [P1 — Workspace and Protocol Skeleton](../plans/P1-workspace-and-protocol-skeleton.md) is
  limited to the workspace foundation, shared protocol types and schemas, error
  types, empty provider ports, initial tests, and a CI skeleton. It does not
  implement the SQLite store or task engine; those are deferred to P2 and later.
- [P2 — Storage and Task Engine Design](../plans/P2-storage-task-engine.md) is the
  design package for the SQLite store, the durable `AssistantTask` lifecycle, and
  restart recovery. P2A–P2H scoped storage, engine and recovery runtime is now
  implemented and closed. P2 does not implement an event bus or event delivery;
  those runtime contracts are implemented in P3 on the candidate branch. See
  [P2 closure](../plans/P2-closure.md) for evidence and nonclaims.

## Frozen sources and dependency note

The decisions are grounded in the available P0 architecture, protocols, and threat model:

- [Architecture index](../architecture/README.md), [system overview](../architecture/01-system-overview.md), [trust boundaries](../architecture/02-trust-boundaries.md), [crate map](../architecture/03-crate-map.md), and [execution pipeline](../architecture/04-execution-pipeline.md)
- [Protocol index](../protocols/00-protocol-index.md) and its linked protocols
- [Threat-model index](../threat-model/README.md), [assets and trust boundaries](../threat-model/01-assets-and-trust-boundaries.md), and [adversaries and attack surface](../threat-model/02-adversaries-and-attack-surface.md)

`03-abuse-cases-and-mitigations.md` and `04-security-invariants.md` were absent during the initial inventory, then appeared during this work and were read before finalizing. The complete available threat-model package confirms the fake-only GoalLatch boundary, read-only watcher, protocol change-control discipline, and the distinction between frozen design and implementation evidence. No source-document dependency remains unresolved at final review; the available frozen protocols remain normative. ADR-0017's historical minor change registered `DELETION_CASCADE_COMPLETED`; P2A established `serea-arch/1.0.0`; accepted ADR-0026 established `serea-arch/2.0.0`; ADR-0027 advances the current
contract set to `serea-arch/2.1.0`. These version changes do not claim deferred runtime implementation or completed whole-workspace/MSRV validation.
