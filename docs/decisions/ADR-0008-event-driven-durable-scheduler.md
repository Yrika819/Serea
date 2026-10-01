# ADR-0008: Event-Driven Durable Scheduler

- Status: **Accepted**
- Architecture version: `serea-arch/0.1.0`
- Decision date: 2026-10-01

## Context

Scheduled work, device presence, and recovery need to trigger bounded orchestration across restarts. A timer-only or in-memory loop can lose scheduled work, duplicate effects after restart, or let wake-source code own task execution.

## Decision

Use a scheduler driven by durable schedules and explicit wake sources/events. The scheduler admits and wakes tasks, but the task engine owns task state, step sequencing, leases, and recovery. The proactive watcher remains read-only: automation can reach only `OBSERVE` and `LOCAL_STATE`, creates proposals, and never raises approvals.

## Consequences

- Wake sources are persisted/reconstructible and checked against host-owned bounds.
- Scheduler and task engine remain separate crates with one-way dependency direction.
- Event ordering and task-state changes are committed durably; restart processing must be idempotent.
- P1 defines no scheduler implementation; it provides protocol foundations and defers runtime scheduling to later phases.

## Rejected alternatives

- Volatile timers as the schedule authority: a restart can lose or replay work.
- Let wake sources run provider calls or own leases: creates a second task engine and bypasses policy.
- Proactive execution of writes: contradicts the read-only automation rule and approval design.

## Frozen source docs

[Crate Map §§2–3](../architecture/03-crate-map.md#2-dependency-layers); [Task Protocol §§5–6](../protocols/02-task-protocol.md#6-recovery); [Event Protocol §5](../protocols/06-event-protocol.md#5-ordering-and-delivery); [Policy Protocol §4.3](../protocols/04-policy-protocol.md#43-additional-standing-rules); [Bounds Protocol §2](../protocols/10-bounds-protocol.md#2-the-bound-set); [Threat Model AS-7](../threat-model/02-adversaries-and-attack-surface.md#as-7-scheduler-and-wake-sources).
