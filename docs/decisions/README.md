# Serea Decision Index

Architecture version: `serea-arch/0.2.0` · Decision set: P0 baseline, plus the P1 corrective addition noted below

This index records the rationale behind the frozen P0 contracts. Protocol documents remain normative: these ADRs explain why the boundaries and choices exist and do not redefine their schemas, fields, variants, or behavior. The architecture version is `serea-arch/0.2.0`; the `0.1.0` baseline is preserved below, because ADR-0017 is the only change accepted since the P0 freeze and it moves the version by one minor step.

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

## P1 implementation plan

[P1 — Workspace and Protocol Skeleton](../plans/P1-workspace-and-protocol-skeleton.md) is limited to the workspace foundation, shared protocol types and schemas, error types, empty provider ports, initial tests, and a CI skeleton. It does not implement the SQLite store or task engine; those are deferred to P2 and later.

## Frozen sources and dependency note

The decisions are grounded in the available P0 architecture, protocols, and threat model:

- [Architecture index](../architecture/README.md), [system overview](../architecture/01-system-overview.md), [trust boundaries](../architecture/02-trust-boundaries.md), [crate map](../architecture/03-crate-map.md), and [execution pipeline](../architecture/04-execution-pipeline.md)
- [Protocol index](../protocols/00-protocol-index.md) and its linked protocols
- [Threat-model index](../threat-model/README.md), [assets and trust boundaries](../threat-model/01-assets-and-trust-boundaries.md), and [adversaries and attack surface](../threat-model/02-adversaries-and-attack-surface.md)

`03-abuse-cases-and-mitigations.md` and `04-security-invariants.md` were absent during the initial inventory, then appeared during this work and were read before finalizing. The complete available threat-model package confirms the fake-only GoalLatch boundary, read-only watcher, protocol change-control discipline, and the distinction between frozen design and implementation evidence. No source-document dependency remains unresolved at final review; the available frozen protocols remain normative. Keep `serea-arch/0.2.0` unchanged absent a separately accepted architecture change. The condition has been met exactly once: ADR-0017 registers `DELETION_CASCADE_COMPLETED` as an `EventKind`, which [Protocol Index §4.1](../protocols/00-protocol-index.md#41-semantics) classifies as a minor, backward-compatible addition.
