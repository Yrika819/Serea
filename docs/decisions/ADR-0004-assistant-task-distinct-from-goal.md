# ADR-0004: AssistantTask Is Distinct from GoalLatch Goal

- Status: **Accepted**
- Architecture version: `serea-arch/0.1.0`
- Decision date: 2026-10-01

## Context

Serea work includes conversation, planning, approvals, schedules, and durable recovery. GoalLatch's goal represents delegated local-PC/code work. Sharing types or lifecycle would make Serea dependent on another system's state model and could confuse a delegate handle with Serea authority.

## Decision

`AssistantTask` is Serea Core's durable unit of work. GoalLatch `Goal` is a separate, opaque delegated object owned by its provider. The only relationship is a capability call through `HostGoalProvider`; Serea retains its own task and step lifecycle and does not import GoalLatch semantics.

## Consequences

- A Serea task may include a delegation step, but GoalLatch never owns Serea planning, task state, policy, or recovery.
- Goal handles are passed back verbatim and are not parsed or used as Serea identifiers.
- Completion is based on a validated provider result, not a model statement or a GoalLatch summary alone.
- P1 defines shared Serea task types without implementing a task engine or real delegate.

## Rejected alternatives

- Alias `AssistantTask` to GoalLatch `Goal`: erases distinct owners and lifecycles.
- Store Serea progress in GoalLatch: breaks restart behavior and makes Serea task state non-authoritative.
- Interpret goal handles as Serea IDs: creates undocumented coupling and parsing dependencies.

## Frozen source docs

[Task Protocol preamble and §2](../protocols/02-task-protocol.md#2-assistanttask); [GoalLatch Adapter Protocol §§1–2, 5, 7](../protocols/08-goallatch-adapter-protocol.md#1-boundary-intent); [System Overview §§1, 4](../architecture/01-system-overview.md#4-the-authority-model); [Crate Map §2](../architecture/03-crate-map.md#2-dependency-layers).
