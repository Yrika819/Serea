# Serea Architecture

Architecture version: `serea-arch/2.4.0` · Status: **FROZEN current contract set** · Ratified on 2026-10-07

This directory is the architecture package for Serea Core. It describes how the
frozen protocols in [`docs/protocols/`](../protocols/00-protocol-index.md) are
assembled into one system, which component owns which decision, and why the
seams sit where they do. It is **not** a restatement of the protocols.

---

## 1. Purpose

> Protocols say what must be true. Architecture says how it is built, in which
> crate, enforced at which line, so that it cannot become untrue.

Three questions this package answers, and no others:

| Question | Answered by |
| --- | --- |
| What is Serea, and who is allowed to decide what? | [System Overview](01-system-overview.md) |
| Where are the structural boundaries, and what crosses them? | [Trust Boundaries](02-trust-boundaries.md) |
| How is the code divided so the boundaries cannot erode? | [Crate Map](03-crate-map.md) |
| What exactly happens between utterance and durable outcome? | [Execution Pipeline](04-execution-pipeline.md) |

## 2. Document map

| # | Document | Scope | Frozen? |
| --- | --- | --- | --- |
| 00 | This file | Package index, doc map, phase-plan pointer | Yes |
| 01 | [System Overview](01-system-overview.md) | Scope, central diagram, component inventory, authority model, non-goals | Yes |
| 02 | [Trust Boundaries](02-trust-boundaries.md) | `TB-1`–`TB-15`, the execution authority flow, cross-boundary `DataClass` rules | Yes |
| 03 | [Crate Map](03-crate-map.md) | Dependency layers, per-crate public surface, test-double placement, deferred crates | Yes |
| 04 | [Execution Pipeline](04-execution-pipeline.md) | Ten-stage request trace, enforcement points, failure mapping, idempotency and recovery | Yes |

Companion packages, referenced but not authored here:

| Package | Directory | Owns |
| --- | --- | --- |
| Protocols | [`docs/protocols/`](../protocols/00-protocol-index.md) | Every wire type, frozen identifier grammar, frozen enum sets |
| Threat model | `docs/threat-model/` | Assets, adversaries, mitigations, threat-model-side `TB-n` alignment |
| Decisions | `docs/decisions/` | ADRs required by [Protocol Index §7](../protocols/00-protocol-index.md#7-change-control) |
| Plans | `docs/plans/` | Phase plan (P0 through the real GoalLatch adapter gate) |

## 3. How architecture relates to the protocols

The protocols are the **frozen interfaces**. Architecture is the **assembly**.
Neither is allowed to quietly widen the other.

| Relationship | Rule | Enforcement |
| --- | --- | --- |
| Protocol → architecture | Every protocol concept must be implemented by exactly one owning crate. | Crate table in [03-crate-map.md](03-crate-map.md#3-crate-inventory) names the owner. |
| Architecture → protocol | Architecture may add *no* identifier shape, wire enum member, or event kind. | [`tools/validate_docs.py`](../../tools/validate_docs.py) rejects malformed identifiers and placeholders. |
| Protocol ↔ protocol | Contracts cross-referenced from this package must resolve to a real heading. | Same validator, anchor check. |
| Architecture change | Any boundary move, layer change, or authority reassignment is an architecture-major change plus an ADR. | [Protocol Index §4.1](../protocols/00-protocol-index.md#41-semantics). |

**Three version axes, never collapsed** ([Protocol Index §4](../protocols/00-protocol-index.md#4-versioning)):

| Axis | Current value at `serea-arch/2.4.0` | Governs |
| --- | --- | --- |
| Architecture version | `serea-arch/2.4.0` | The whole contract set, including this package |
| Capability version | Per-descriptor SemVer, e.g. `calendar.events.list` at `1.2.0` | One capability's input/output contract |
| Wire protocol version | `serea.action/2`, `serea.task/2`; `serea.model/1`, `serea.policy/1`, `serea.approval/1`, `serea.event/1`, `serea.device/2`, `serea.goallatch/1`, `serea.data/1`, `serea.bounds/1`, `serea.scheduler/1` | One transport or serialization surface; envelope version stays 1 |

### 3.1 P2A contract baseline

This subsection records the P2A registry established on 2026-10-03. P0/P1 used
`serea-arch/0.2.0`; both dated baselines remain historical evidence. Owner
ratification on 2026-10-03 accepts ADR-0018 (wire/lifecycle architecture, runtime
deferred), ADR-0019 (implemented SCJ-1/digest/IDK-1), ADR-0020 (semantic B3
clarification) and ADR-0023 (complete implemented validation), after three corrected
documentation reviews GREEN. ADR-0021/22/24 runtime remains Proposed; only 0024's
wire generation member/validation is implemented. The coordinator owns final
workspace/MSRV 1.85 validation, bounded regression review and integration closure;
current command results and test counts are recorded in the closure record, not
inferred from earlier runs.
No store/engine/event runtime or Clock/P2B is delivered. Source, schemas, manifest,
versions, tests and these docs belong to one P2A integration, with no preliminary
docs-only commit. See [frozen gate](../plans/P2A-review-and-closure.md).

### 3.2 Architecture/2.0 history

Accepted ADR-0026 changes the replay contract to distinguish retained events,
intentional interior expiry ranges, expired prefixes, and unexplained
corruption. This change raised the architecture version to `serea-arch/2.0.0` and the
affected replay surface to `serea.device/2`. Actual event objects remain
`serea.event/1`; unrelated wire surfaces and envelope version 1 are unchanged.
The Protocol Index is the current registry authority.

### 3.3 Architecture/2.1 contract

Accepted ADR-0027 defines Serea calendar recurrence as the closed ONCE, DAILY,
and WEEKLY `CalendarRecurrenceV1` grammar. It advances the current architecture
to `serea-arch/2.1.0`; the Scheduler surface remains `serea.scheduler/1`. Jiff
resolves local timezone and DST rules but does not own recurrence semantics.

### 3.4 Current architecture/2.2 contract

Accepted ADR-0028 defines exact EventPredicateV1 and ScheduledTaskTemplateV1
values, excludes Scheduler-rooted events from HOST_EVENT matching, and adds the
bounded template-size rule. The architecture advances to `serea-arch/2.2.0`;
`serea.event/1` and `serea.scheduler/1` remain unchanged.

### 3.5 Architecture/2.3 contract history

Accepted ADR-0029 defines `DeviceConnectedPayloadV1` and the explicit durable
`DeviceResumeWaitV1` task eligibility and sequence-fenced resume wake. The
architecture advances to `serea-arch/2.3.0`; Event, Scheduler, and Task wire
surfaces remain unchanged.

### 3.6 Current architecture/2.4 contract

Accepted ADR-0030 defines closed approval lifecycle routing identity and durable
Scheduler handoff to future P6. Scheduler does not apply approval outcomes or
transition tasks. The architecture advances to `serea-arch/2.4.0`; Event,
Scheduler, Approval, and Task wire surfaces remain unchanged.

## 4. Identifier discipline in this package

Every identifier in these four documents is either a full 26-character
Crockford Base32 ULID behind a frozen prefix, a 64-hexadecimal
`IdempotencyKey`, a three-segment `CapabilityId`, or a `Digest`. The grammar is
fixed in [Protocol Index §2](../protocols/00-protocol-index.md#2-identifier-grammar)
and this package mints no shape of its own — in particular no memory-item prefix
and no adapter-goal prefix
([Data Classification §8](../protocols/09-data-classification-protocol.md#8-right-to-delete-and-provenance),
[GoalLatch Adapter §7](../protocols/08-goallatch-adapter-protocol.md#7-result-and-evidence-contract)).

Illustrative identifiers used throughout, all grammar-conformant:

```json
{
  "task_id": "tsk_01JQ8Z9K3M7QWXR4V2T6YH0BNA",
  "step_id": "stp_01JQ8Z9M3R2CVN8H5FWK7PQDSF",
  "approval_id": "apr_01JQ8ZA1D4NFG8K2M6RTV9XCWB",
  "event_id": "evt_01JQ8ZB7H2XKM9P4QW7NRT5YCD",
  "device_id": "dev_01JQ8ZC5N8TVG3K6MRQ2XW9JHF",
  "session_id": "ses_01JQ8ZG9V5MXK3N7QW2RTF8YHB",
  "receipt_id": "rcp_01JQ8ZF4T7KMV2X9NPQ5RD8WCS",
  "idempotency_key": "idk_9f2c1a7e4b6d0f8a3c5e9b1d7f2a4c6e8b0d3f5a7c9e1b4d6f8a0c2e4b6d8f9a",
  "capability_id": "calendar.events.list",
  "model_id": "nemotron-3-nano-30b"
}
```

`request_id` uses the `req_` + ULID `RequestId`; `evidence_id` uses the
`evt_` + ULID `EventId` shape. The request identifier is frozen by
[Capability Protocol §4](../protocols/01-capability-protocol.md#4-actionrequest)
and [§7](../protocols/01-capability-protocol.md#7-evidence) and is not a new
shape.

## 5. Where the protocols meet the architecture

| Protocol document | Its primary architecture home | The document that shows it working |
| --- | --- | --- |
| [Capability Protocol](../protocols/01-capability-protocol.md) | `serea-capability` | [04-execution-pipeline.md](04-execution-pipeline.md), stages 5–9 |
| [Task Protocol](../protocols/02-task-protocol.md) | `serea-task-engine` | [04-execution-pipeline.md](04-execution-pipeline.md), stages 1–2 and 10 |
| [Model Protocol](../protocols/03-model-protocol.md) | `serea-model-router` | [04-execution-pipeline.md](04-execution-pipeline.md), stage 3 |
| [Policy Protocol](../protocols/04-policy-protocol.md) | `serea-policy` | [04-execution-pipeline.md](04-execution-pipeline.md), stage 6 |
| [Approval Protocol](../protocols/05-approval-protocol.md) | `serea-capability` (ledger) + `serea-core` (delivery) | [04-execution-pipeline.md](04-execution-pipeline.md), stage 7 |
| [Event Protocol](../protocols/06-event-protocol.md) | `serea-event-bus` | [04-execution-pipeline.md](04-execution-pipeline.md), stage 9 |
| [Device Protocol](../protocols/07-device-protocol.md) | `serea-core` (link) + `serea-scheduler` (presence) | [02-trust-boundaries.md](02-trust-boundaries.md#tb-1-device-to-core) |
| [GoalLatch Adapter Protocol](../protocols/08-goallatch-adapter-protocol.md) | `serea-provider-goallatch` | [01-system-overview.md](01-system-overview.md#2-system-shape) |
| [Data Classification Protocol](../protocols/09-data-classification-protocol.md) | `serea-protocol` (types) + `serea-credential-store` (custody) | [02-trust-boundaries.md](02-trust-boundaries.md#4-cross-boundary-data-rules) |
| [Bounds Protocol](../protocols/10-bounds-protocol.md) | `serea-core` (config) + each owning crate (enforcement) | [04-execution-pipeline.md](04-execution-pipeline.md#5-failure-mapping) |
| [Scheduler Protocol](../protocols/11-scheduler-protocol.md) | `serea-scheduler` | [03-crate-map.md](03-crate-map.md#3-crate-inventory), `serea-scheduler` row |

## 6. Phase-plan pointer

This package is the **current architecture contract**, evolved from the P0
baseline; its runtime assembly remains phased. It is written against the phase plan under
`docs/plans/`, and the plan is the only thing that may change the order in which
these components ship. The architecture-relevant sequencing, restated here so a
reader of this package alone can orient:

| Phase | Scope | Architecture consequence |
| --- | --- | --- |
| P0 | This document set. Frozen contracts, crate boundaries, authority model. | Nothing ships before the boundaries are written down. |
| P1 | `serea-protocol` shared IDs/types/schemas, empty ports, test doubles, CI skeleton. | No task engine, durable task store, real provider, or GoalLatch connection. |
| P2 | SQLite storage and durable AssistantTask lifecycle/recovery. | Task state survives restart; no Gmail/Calendar integration. |
| P3 | Structured event bus and event-driven scheduler. | Schedule and scheduler recovery contracts become executable. |
| P4–P6 | Model router (P4), capability/tool router (P5), deterministic policy and approval (P6). | Authority chain is exercised against mocks; Codex remains disabled. |
| P7–P8 | SQLite/FTS5 memory (P7) and synthetic provider journey (P8). | External data remains synthetic and no real credentials are needed. |
| P9–P10 | Read-only Gmail (P9) and Calendar (P10). | Incremental sync with safe full-resync fallback. |
| P11–P12 | Read-only proactive watcher (P11) and Android client foundation (P12). | Watcher is `OBSERVE`/`LOCAL_STATE` only; Pixel 7a/API 37 acceptance is primary. |
| P13–P14 | Standard Android provider (P13) and isolated optional root provider (P14). | Root absence never blocks baseline use; root remains finite and approval-gated. |
| P15–P16 | Fake GoalLatch journey (P15) and pre-integration readiness closure (P16). | **No real adapter is scheduled or authorized here.** Any future real integration requires a separate explicit phase authorization after the six live verifications in [GoalLatch Adapter §9](../protocols/08-goallatch-adapter-protocol.md#9-real-adapter-readiness-gate). |

## 7. Reading order

Read [01-system-overview.md](01-system-overview.md) first. Then
[02-trust-boundaries.md](02-trust-boundaries.md) — every later claim about "the
host decides" is a claim about a named `TB-n`. Then
[03-crate-map.md](03-crate-map.md) for where those boundaries become module
boundaries. Then [04-execution-pipeline.md](04-execution-pipeline.md) for a single
request traced end to end, twice: once read-only, once effecting.
