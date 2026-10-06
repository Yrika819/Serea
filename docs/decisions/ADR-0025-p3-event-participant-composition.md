# ADR-0025: P3 Event Participant Composition

- **Status:** **Accepted** — fixed P3 transaction composition; runtime implementation remains deferred
- **Architecture version at acceptance:** `serea-arch/1.0.0` (current is `serea-arch/2.0.0`)
- **Decision date:** 2026-10-06
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

The accepted composition is fixed and deliberately contains exactly two
participants. Separate transactions violate E3; a Storage dependency on Event
Bus or Task Engine inverts the frozen graph; a generic mutable participant
registry was rejected in ADR-0021; and a post-commit queue cannot repair a
missing atomic event.

## Decision

Storage owns the transaction boundary and derives immutable successful-write
facts. Its fixed dispatch invokes exactly the existing `TaskAuditParticipant`
and the new `EventParticipant`, in the same transaction and operation savepoint,
against those same facts. There is no caller-supplied participant list, registry,
or arbitrary participant ordering.

Ownership is split by meaning: Task Engine owns the task-journal semantic
mapping; Event Bus owns `SereaEvent` semantic mapping and sequence allocation.
Storage owns neither mapping. It provides the fixed transaction/savepoint,
immutable facts, private persistence capability, and successful-write facts.
Neither participant receives a `Store`, connection, transaction, `Tx`, or SQL
executor, and neither may re-enter Storage.

For every covered write, Storage performs the state mutation first, derives the
immutable facts from successful writes, then invokes the fixed journal and event
participants within that operation's savepoint. The outer transaction commits
once. Any journal/event mapping, serialization, persistence, or commit failure
rolls back state, journal, event, and sequence allocation together. A successful
no-op or refusal emits neither a journal transition nor an event, except where
an already-frozen contract explicitly defines a refusal event. There is no
post-commit append and no second transaction.

P2 journal rows are never synthesized into events; migration does not add
`task_journal.event_seq`. Event coverage is forward-only and belongs to the
explicit transition inventory for P3.

## Options to evaluate

| Option | Assessment |
|---|---|
| Explicit fixed composition that invokes journal and event-specific mapping from the same immutable successful-write facts and storage transaction | **Accepted.** Exactly two participants, fixed dispatch, one transaction/savepoint. |
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

## Implementation boundary

This ADR settles composition only. It does not authorize P3 runtime, migration
0002, an Event Bus or Scheduler crate, or claim E3/E4 runtime proof. The exact
transition-to-event inventory and transaction-fault tests remain P3
implementation gates. The broader ADR-0021 runtime gate remains distinct.

## Frozen sources

- [Event Protocol §§2, 5, 9](../protocols/06-event-protocol.md#5-ordering-and-delivery)
- [Crate Map §§1–3](../architecture/03-crate-map.md#1-the-layering-rule)
- [ADR-0021 P2/P3 Event Atomicity Seam](ADR-0021-p2-p3-event-atomicity-seam.md)
- [P2 Closure](../plans/P2-closure.md)
