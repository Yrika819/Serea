# ADR-0025: P3 Event Participant Composition

- **Status:** Proposed — owner decision required before P3 implementation
- **Architecture version:** `serea-arch/1.0.0`
- **Decision date:** not ratified
- **Scope:** Compose P2 task audit and P3 event persistence under one SQLite transaction.

## Context

P2 is closed with `serea-storage` owning SQLite transactions and private
`DurableTransition` facts, while `serea-task-engine` owns the semantic mapping
to `task_journal`. The implemented audited transaction accepts one
`TaskAuditParticipant`. That participant runs after successful state writes,
inside the operation savepoint, and participant failure rolls back the method.
There is no public SQL escape or generic hook registry.

The frozen Event Protocol E3 requires the event and described state transition
to commit together. E4 requires the per-host sequence to be allocated at commit.
ADR-0021 proposes that P3 add an event-specific participant to the same
transaction/facts, while preserving forward-only E3, no history backfill, and
the existing P2 task journal. ADR-0021 remains Proposed; this ADR does not
accept or change it.

The current API does not specify how the existing journal mapping and a second
event mapping compose. Separate transactions violate E3; a Storage dependency
on Event Bus or Task Engine inverts the frozen graph; a generic mutable
participant registry was rejected in ADR-0021; and a post-commit queue cannot
repair a missing atomic event.

## Decision required

The owner must select and ratify a narrow composition/API before runtime work.
The proposal is constrained as follows:

1. One SQLite transaction commits a covered state write, its P2 journal record
   when applicable, the event row, and event sequence allocation.
2. State facts are derived from successful storage writes; callers cannot
   submit detached or fabricated transition facts.
3. Any participant serialization/write failure or outer transaction failure
   rolls all covered rows and sequence changes back. Successful no-op/refusal
   writes no event.
4. P2 historical journal rows are never synthesized into events; migration
   does not add `task_journal.event_seq`.
5. Storage remains below Task Engine, Scheduler, and Event Bus. No dependency
   cycle, Store re-entry, separate transaction, or unbounded generic registry.
6. The accepted API must preserve P2 savepoint/error behavior and identify
   exactly which existing/new state operations emit which event kinds.

## Options to evaluate

| Option | Assessment |
|---|---|
| Explicit fixed composition that invokes journal and event-specific mapping from the same immutable successful-write facts and storage transaction | Preferred direction for owner review; signature, dispatch ownership, and savepoint placement remain unresolved. |
| Generic list/registry of arbitrary transaction participants | Reject unless owner explicitly reopens ADR-0021; it introduces participant ordering/lifecycle and risks weakening the deliberately narrow P2 seam. |
| Event append in a second/post-commit transaction or outbox | Rejected by E3 and ADR-0021; can leave committed state with no event. |
| Event Bus/Task Engine dependency from Storage | Rejected by Crate Map layering and acyclic dependency rule. |
| Reconstruct events from `task_journal` during migration | Rejected by ADR-0021; it cannot make historical E3 true and fabricates indistinguishable atomic events. |

## Consequences if ratified

- E3/E4 become enforceable only for transitions wired to the selected event
  participant from the first P3 migration onward.
- `task_journal` remains an independent P2 audit record, not Event Bus storage.
- P3 test gates must prove all-or-nothing state/journal/event/sequence behavior,
  including crashes before commit and caller loss after commit.
- The P3 storage/API design and migration 0002 must name event-producing
  operations without changing P2 migration 0001 or its historical semantics.

## Owner disposition

**Unresolved.** The P3 preimplementation audit records the concrete conflict and
safe interpretation in [P3 Preimplementation Audit §4](../plans/P3-preimplementation-audit.md#4-atomicity-contradiction-and-proposed-adr-0025).
This ADR must not be marked Accepted by the implementation agent.

## Frozen sources

- [Event Protocol §§2, 5, 9](../protocols/06-event-protocol.md#5-ordering-and-delivery)
- [Crate Map §§1–3](../architecture/03-crate-map.md#1-the-layering-rule)
- [ADR-0021 P2/P3 Event Atomicity Seam](ADR-0021-p2-p3-event-atomicity-seam.md)
- [P2 Closure](../plans/P2-closure.md)
