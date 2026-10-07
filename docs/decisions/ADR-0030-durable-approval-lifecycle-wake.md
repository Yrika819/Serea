# ADR-0030: Durable approval-lifecycle wake handoff

- Status: **Accepted**
- Date: 2026-10-07
- Architecture version: `serea-arch/2.4.0`
- Affected surfaces: `serea.event/1`, `serea.scheduler/1`, `serea.approval/1`, and `serea.task/2` remain unchanged
- Owners: Protocols, Event Bus, Scheduler, future Core/P6 handoff

## Context

P3 consumes the durable Event Bus and must not lose approval lifecycle events
between event replay and the later Approval implementation. P3 does not persist
or evaluate authoritative `ApprovalRequest` or `ApprovalGrant` state. An event
kind alone therefore cannot authorize Scheduler to resume, fail, or otherwise
change a task. Approval outcome application belongs to the future P6 Policy and
Approval runtime.

## Decision

Approval lifecycle events use one closed `ApprovalLifecyclePayloadV1` routing
payload with exactly `approval_id`, `task_id`, and `step_id`. The existing
`ApprovalId`, `TaskId`, and `StepId` grammars apply. For
`APPROVAL_GRANTED`, `APPROVAL_DENIED`, and `APPROVAL_EXPIRED`, the host requires
`correlation_id == payload.task_id` and a trace whose `task_id` and `step_id`
match the payload. Canonical serialization follows SCJ-1. These identifiers
route a handoff; they carry no grant authority.

The Scheduler materializes one internal `ApprovalLifecycleWake` per source
event, uniquely keyed by `source_event_id`, with source sequence, the three
routing IDs, a closed outcome kind, and creation time. It commits the wake and
Scheduler event cursor together. Duplicate replay is idempotent. An intentional
expired range advances without fabricating a wake; unexplained corruption
stops replay before the gap.

Reading a wake does not consume it. Future Core/P6 code can list or claim the
pending typed wake, load authoritative Approval state, apply the outcome through
the proper Policy/Approval and Task paths, then explicitly acknowledge the wake
after its own durable operation. Delivery is at least once; P3 makes no exactly
once claim about P6 application.

P3 Scheduler does not validate grants, consume grants, evaluate approval
authority, create tasks or occurrences, call providers, or transition tasks
from `WAITING_APPROVAL` based on an event. It only validates routing identity,
materializes, deduplicates, preserves, and exposes the durable handoff.

## Version impact

This is a backward-compatible addition to the architecture contract set and
advances `serea-arch/2.3.0` to `serea-arch/2.4.0`. The event envelope and the
Event, Scheduler, Approval, and Task surface majors do not change. Existing
clients that do not interpret the kind-specific payload retain ordinary event
rendering behavior; a host acting on these event kinds validates the closed
payload and trace before materialization.

## Consequences

- Approval lifecycle delivery survives restart and source-event content expiry
  once materialized.
- Scheduler replay and wake acknowledgement are distinct operations.
- A malformed or inconsistent routing payload cannot create a wake or advance
  the cursor past that event.
- P6 remains the sole owner of approval authority and outcome application.

## References

- [Approval Protocol](../protocols/05-approval-protocol.md)
- [Event Protocol](../protocols/06-event-protocol.md)
- [Scheduler Protocol](../protocols/11-scheduler-protocol.md)
- [Protocol Index §4 and §7](../protocols/00-protocol-index.md#4-versioning)
