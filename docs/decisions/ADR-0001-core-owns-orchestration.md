# ADR-0001: Serea Core Owns Orchestration

- Status: **Accepted**
- Architecture version: `serea-arch/0.1.0`
- Decision date: 2026-10-01

## Context

Serea needs durable task state, model routing, policy, approvals, memory, schedules, device sessions, and an audit trail. Making a delegated work system the hub would transfer authority and lifecycle ownership across a boundary that must remain independently enforceable.

## Decision

Serea Core permanently owns orchestration, `AssistantTask` lifecycle, sequencing, budgets, recovery, registry, policy, approvals, memory, scheduler, device sessions, and event ordering. GoalLatch is only a delegate reached through `HostGoalProvider` and registered `host.goal.*` capabilities. The adapter boundary is protocol/data based, with no imported GoalLatch types or shared storage.

## Consequences

- Core remains the single authority-owning component; providers report facts and perform only authorized calls.
- Delegation follows the same validation, policy, approval, receipt, and persistence path as every capability.
- Integration can be swapped behind the adapter without making GoalLatch own Serea task state.
- This maintains an explicit dependency boundary and forbids direct database access or `local_mcp::*` dependencies.

## Rejected alternatives

- Let GoalLatch own Serea task lifecycle or planning: merges distinct authority and durability domains.
- Share GoalLatch storage or import its internal types: couples Core to undocumented implementation details and permits bypasses.
- Treat GoalLatch as a privileged orchestration tier: contradicts least authority and makes its output govern Serea.

## Frozen source docs

[System Overview §4](../architecture/01-system-overview.md#4-the-authority-model); [Trust Boundaries TB-5](../architecture/02-trust-boundaries.md#tb-5-host-to-goallatch-adapter); [Crate Map §6](../architecture/03-crate-map.md#6-dependency-inversion-rules-for-swappable-providers); [GoalLatch Adapter Protocol §§1–3](../protocols/08-goallatch-adapter-protocol.md#1-boundary-intent); [Task Protocol §2](../protocols/02-task-protocol.md#2-assistanttask).
