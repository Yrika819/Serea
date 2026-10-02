# ADR-0021: The P2/P3 Event-Atomicity Seam

- Status: **Proposed** — pending implementation and owner ratification
- Architecture version: `serea-arch/0.2.0` at the time of writing
- Decision date: not yet ratified
- Recorded by: P2 design preparation, from `c3737039e3e38dbba554dc0b9075025f87948358`
- Feeds: [P2 contract gap analysis](../plans/P2-contract-gap-analysis.md) §5.7

> This ADR changes no frozen protocol text and no code. The amendments below are
> **drafted, not applied**.

## Context

Two Event Protocol invariants constrain P2 directly:

- `E3`: "An event and its state change commit in one transaction — never one
  without the other."
- `E4`: "`seq` is gapless and monotonic, assigned at commit."

P2 must implement a durable task lifecycle, which means mutating state. But
[Crate Map §3.1](../architecture/03-crate-map.md#31-ownership-of-each-protocol-contract)
gives `serea-event-bus` sole ownership of `SereaEvent` construction, gapless `seq`
assignment, the append-only log, and retention classes. Crate Map §4.1 states the
reason the crate exists: without it, "`seq` could be assigned outside the commit
transaction, breaking `E3` and `E4`". The architecture README's phase table puts
the event bus in **P3** and P2's scope at "SQLite storage and durable
`AssistantTask` lifecycle/recovery".

So P2 must either mutate state without events, or take an L1 crate that belongs to
a later phase.

Three options:

| Option | Verdict |
| --- | --- |
| Create `serea-event-bus` in P2 | **Rejected.** It inverts the phase plan, pre-empts P3's design of the fan-out queue and retention classes, and widens P2's declared slice from `serea-storage` + `serea-task-engine` to three crates. Crate Map §4.1's entire justification is that event append is *separate* work |
| Mutate state with no durable trace | **Rejected.** Violates `E3`, destroys the audit trail Event Protocol §7 depends on, and breaks Task Protocol §6 recovery, which needs to know what was already decided |
| A commit primitive in P2 that P3 fills without rewriting P2 | **Accepted** |

## Decision

### `E3` is enforceable forward only, and that is the whole shape of the seam

One rule governs this ADR, and the P2 autonomous audit established it by
reasoning that cannot be argued around:

> A `SereaEvent` and a state transition are in one transaction only if they were
> written in one transaction. For a transition P2 already committed, that
> transaction is gone. `E3` — *"An event and its state change commit in one
> transaction — never one without the other"* — is therefore **enforceable from
> the moment a real event participant exists, and never retroactively.**

So P2 mutates state and records a durable, non-event audit row. P3 adds a real
event participant. Between those two points `E3` did not hold, and no amount of
back-filling changes that.

### `serea-storage` owns the transaction and an explicit participant seam

The layering problem is real and worth stating precisely, because it is why a
seam exists at all: `serea-storage`'s `Tx` methods perform the state writes, while
the journal is owned by `serea-task-engine`, a layer above. `serea-storage` cannot
name it. The seam is dependency inversion, not indirection for its own sake.

```rust
/// Immutable description of the transition being committed.
///
/// Built once, by the `Tx` method performing the state write, and passed to
/// every participant. It is a parameter rather than participant state, so no
/// participant can record a different transition from any other, and a stale
/// value cannot survive a rolled-back transaction.
pub struct DurableTransition<'a> { /* occurred_at_ms, actor, causation_id,
                                      data_class, payload_digest, payload_json */ }

/// Writes this participant's rows into the caller's open transaction.
pub trait TransactionParticipant {
    fn participate(&mut self, tx: &mut Tx, t: &DurableTransition<'_>)
                   -> Result<(), StoreError>;
}
```

`Store::transact` opens `BEGIN IMMEDIATE`, creates one `Tx`, and the `Tx` method
performing the state write calls each participant with the transition it just
performed. Every participant therefore commits or rolls back exactly with the state
change — which is the *mechanism* `E3` requires. What a participant writes is its
own business.

**Four properties this signature has and the earlier one did not**, each of which
was a defect in the sketch the audit replaced:

| Property | Why |
| --- | --- |
| The transition is a **parameter**, not participant state | The earlier `fn append(&mut self, tx)` was told *that* a commit was happening but not *what* was being committed, so the identity had to live in the hook's mutable state — set before the call, and stale if a `transact` body returned early. That is a correctness hazard in an audit record, not a style one |
| `&self` on `Store::transact` needs **no interior mutability** | Participants are held as `Arc<dyn TransactionParticipant>` assembled at construction. A `Box<dyn CommitHook>` behind a `&self` store could only be called as `&mut self` through a `RefCell` or a `Mutex`, which would have contradicted ADR-0024's "the only mutex in `serea-storage` guards the single SQLite connection" |
| **Neither participant needs `&mut self` semantically** | `TaskJournal` computes `journal_seq` as `MAX(journal_seq) + 1` — SQL. P3's event participant allocates `seq` from a `store_meta` counter — also SQL, in the same transaction. Neither mutates Rust state, so no lock is needed for the guarantee |
| **Rollback needs no cleanup** | The transition is owned by the caller, not buffered by the participant, so a rolled-back transaction leaves nothing to discard |

**A generic hook registry is not needed, and is rejected.** P2 registers exactly
one participant. A registry would add ordering, interior mutability and a
lifecycle question in exchange for a capability P2 does not use. The narrower
form — `Tx` calls its participants explicitly, in a fixed order, at the point of
the state write — gives P3 the identical seam with less hidden behaviour, so that
is what is specified.

### P2 registers exactly one participant: `TaskJournal`

`serea-task-engine` owns `TaskJournal`, which writes append-only `task_journal`
rows. `task_journal` is **not** an event log: it has no `EventKind`, no `Actor` in
the frozen enum, no `seq`, and it is not on any wire surface.

That last point is easy to state and easy to get wrong, because an earlier draft
of this ADR named its journal kinds `TASK_CREATED`, `TASK_CANCELLED` and
`TASK_COMPLETED` — three frozen `EventKind` variants — in a package whose own test
O9 forbids P2 constructing an `EventKind`. That would be a second naming authority
for values the Protocol Index §1 registry already assigns to `serea-event-bus`. The
journal kind vocabulary is therefore its own closed set, naming **what the engine
did**: `TASK_INSERTED`, `PLAN_PERSISTED`, `TASK_STATE_CHANGED`,
`STEP_LEASE_ACQUIRED`, `STEP_LEASE_RELEASED`, `STEP_ATTEMPT_STARTED`,
`STEP_COMMITTED`, `STEP_FAILED`, `STEP_RECONCILED_ABSENT`, `RECEIPT_RECORDED`,
`RECOVERY_DECISION`, `TASK_CANCEL_REQUESTED`, `TASK_TERMINAL`.

`TASK_DELETED` is deliberately absent: `task_journal.task_id` cascades with the task,
so a deletion record cannot survive its own transaction. `DeletionOutcome`'s counts
are the record instead.

### P3 adds a second participant, and `E3` holds from that point forward

`serea-event-bus` implements `TransactionParticipant`. Its `participate` writes
`serea_events` and allocates `seq` from a `store_meta.next_seq` counter, **inside
the same `Tx`**, from the same `DurableTransition` the journal received. Because
both participants run inside one `BEGIN IMMEDIATE`, `E3` is satisfied for every
transition from that moment on, and **not one P2 state-transition function
changes**.

### No event is reconstructed, and that is the decision the earlier draft missed

The earlier draft made P3's obligation "back-filling `task_journal.event_seq` for
rows where it is `NULL`". The P2 autonomous audit rejected that on three grounds,
and the reasoning is recorded because the draft was self-contradictory rather
than merely wrong:

1. **It is the rejected outbox under another name.** This ADR rejected *"A
   `pending_event` outbox drained by P3"* because "the state change is already
   committed without its event, so `E3` stays violated and **the gap is permanent
   rather than transitional**." `task_journal` with a nullable `event_seq` that P3
   drains *is* that outbox. One of the two positions was wrong; the rejection
   reasoning was the sound one, and it applies verbatim to the accepted design.
2. **It cannot make `E3` true.** A reconstructed event was written in a different
   transaction, months later, from a different process. For those rows the
   transaction `E3` describes does not exist and never will.
3. **The historical material already exists.** `task_journal` is a complete,
   durable record of every P2 transition, which is the entire reason for its
   column list and for `payload_json`. Synthesising `serea_events` rows duplicates
   data that is already stored, and manufactures events that never happened in the
   transaction `E3` describes — and a reconstructed event is **indistinguishable**
   from an atomically committed one unless provenance is added to a frozen wire
   type.

So: **`task_journal.event_seq` is dropped rather than back-filled.** P3's upgrade
path *reads* `task_journal` for pre-P3 history and does not synthesise events.
`pending_event_transitions` keeps its meaning — the count of transitions committed
before an event participant existed — which is exactly the operator visibility the
count was introduced for, and is honest.

### Why the journal schema is shaped for that back-fill

A P2 database upgraded to P3 must be able to materialise every missing event
without inventing data. So `task_journal` carries, from P2:

`actor_kind`, `actor_id`, `actor_version`, `causation_id`, `data_class_rank`,
`payload_digest`, `payload_ref_digest`, `occurred_at_ms`, `reason_code`,
`state_from`, `state_to`, `attempt`, `journal_kind`.

Those are the *envelope* fields an event needs. They are **not** the whole of it,
and an earlier draft of this ADR claimed they were, which was false:
`CAPABILITY_COMPLETED`'s payload carries `duration_ms` and `output_digest`;
`BOUND_EXCEEDED` requires `bound_name`, the limit and the observed value;
`MODEL_CALLED` requires `model_id`, `purpose` and a token estimate. None was in the
column list, so the "no fabrication at upgrade time" property did not hold.

The fix is `task_journal.payload_json`, validated JSON carrying whatever the
transition's future event payload will need, plus `payload_ref_digest` pointing at
a blob for large payloads. The back-fill property then holds because the payload has
somewhere to live, not because the columns happened to be complete.

## What P2 can honestly claim at closure

This is the part that must not be fudged, so it is stated as a table rather than
prose.

| Invariant | P2 status | Why |
| --- | --- | --- |
| `T1` task state is durable and authoritative | **Claimed** | `tasks.state` is a `CHECK`-constrained column and the only authority; `T2` and `T3` follow from it |
| `T4` a step's success and receipt persist before the task advances | **Claimed** | Both are in the same `Tx`; the step write precedes the task write and both commit together |
| `T5` recovery is idempotent | **Claimed** | Every mutation is a single conditional statement carrying its full precondition |
| `TB-7` cross-record atomicity for task, step, receipt and journal rows | **Claimed** | One `BEGIN IMMEDIATE` per transition |
| `E3` an event and its state change commit together | **NOT claimed** | Deferred to P3 by construction. P2 writes no `SereaEvent` |
| `E4` `seq` gapless, assigned at commit | **NOT claimed** | No `seq` exists in P2 |
| `E1`, `E2`, `E5`–`E10` | **NOT claimed** | All belong to `serea-event-bus` or the device link |

To make the deferral **visible rather than silent**, `RecoveryReport` carries
`pending_event_transitions: u64`, counting `task_journal` rows that no event
participant has acknowledged — `event_seq IS NULL` in P2, and whatever P3's
migration leaves non-`NULL` thereafter. A P2 test asserts the count is greater than
zero after a task creation. An operator can therefore see the `E3` debt from
inside the product, and a P3 upgrade can measure exactly how much history predates
it.

**The honest reading of that number**, since it is the one thing a reader will
misinterpret: a non-zero count means those transitions committed **without** an
event, which is precisely the `E3` debt. It is not a queue to drain and not a
migration backlog to clear. It is a permanent, visible record of a period during
which `E3` did not hold.

## What P2 must not create

- **No `serea_events` table.** It belongs to P3. P3's first migration is `0002`.
- **No `store_meta` table and no `next_seq` counter.** `seq` is P3's per
  [Crate Map §3](../architecture/03-crate-map.md#3-crate-inventory).
- **No `EventKind` construction, no `Actor` construction, no `Seq` allocation.**
- **No `task_journal.event_seq` to back-fill.** The column is dropped rather than
  reserved, because a reserved column that must never be written is a second source
  of truth for "did this transition get an event", which is exactly the kind of
  disagreement this design refuses elsewhere.

This is why the P2 schema has no event table and no store-metadata table, and it
is a deliberate decision rather than an omission.

## Proposed amendment

Applied to `docs/protocols/06-event-protocol.md` only in the same commit that
implements it. Nothing here is applied by this run.

A single note added under `E3` and `E4`, and a changelog section:

> `E3` and `E4` become enforceable once `serea-event-bus` supplies an event
> participant to the same transaction that carries the state change. Before that
> crate exists, a `serea-task-engine` phase may commit durable state and a
> non-event audit record in one transaction; it may not claim `E3` or `E4`, and
> neither is weakened by the deferral. See ADR-0021.

`E3` and `E4` themselves are **unchanged**. This ADR adds an implementation
sequencing note, not a relaxation.

## Code change, same commit

| File | Change |
| --- | --- |
| `crates/serea-storage/src/tx.rs` | `DurableTransition`, `TransactionParticipant`, the participant list on `Store`; `transact` invoking them inside the transaction, each with the transition its caller just performed |
| `crates/serea-storage/src/store.rs` | Every mutating path becomes `transact`-shaped; no public method writes alone |
| `crates/serea-task-engine/src/journal.rs` | `TaskJournal` implementing `TransactionParticipant` |
| `crates/serea-task-engine/src/recovery.rs` | `RecoveryReport.pending_event_transitions` |
| `crates/serea-storage/migrations/0001_initial.sql` | `task_journal` with the envelope columns, `payload_json` and `payload_ref_digest`, and **no `event_seq`** — the earlier draft reserved it for a back-fill this ADR no longer performs. Full DDL in [P2 SQLite schema §4.9](../plans/P2-sqlite-schema.md#49-task_journal) |

## Consequences

- P2 produces a complete, replayable audit trail even though it produces no
  events. `T5` and `T4` are provable without P3.
- `E3` holds **forward** from P3's first migration, and is recorded as never having
  held for P2-era transitions. No event is fabricated to paper over the gap.
- The cost is one extra durable table and one extra write per transition in P2.
  That write is what makes the pre-P3 history readable rather than reconstructed.
- A reviewer can check the honesty of P2's closure claim by grepping for exactly
  one thing: `serea_events` must not appear anywhere in the P2 diff.

## Rejected alternatives

| Alternative | Why rejected |
| --- | --- |
| Create `serea-event-bus` in P2 | Inverts the phase plan and pre-empts P3's fan-out and retention design. Crate Map §4.1's reason for the crate is that it is *separate* |
| Write real `SereaEvent` rows from P2 | `EventKind`, `Actor` and `Seq` are all in `serea-protocol`, so this is *possible* — which is exactly why it is rejected. Crate Map §3.1 assigns the append-only log to the event bus, and a P2 that grows its own will be hard to remove once P3 needs it |
| Two-phase commit across P2 and P3 | SQLite is single-writer and local; a two-phase protocol would buy nothing and add a failure mode |
| A `pending_event` outbox drained by P3 | The state change is already committed without its event, so `E3` stays violated and the gap is permanent rather than transitional. **This ADR previously adopted exactly this shape as `task_journal.event_seq` and called it a back-fill obligation; the P2 autonomous audit identified the contradiction and it is now rejected on these grounds** |
| **Back-fill `task_journal.event_seq` and materialise `serea_events`** | Rejected by the P2 autonomous audit on three grounds: it is the outbox above under another name; it cannot make `E3` true for a transition whose transaction is gone; and the historical material already exists in `task_journal`, so synthesising events duplicates durable data and manufactures events indistinguishable from atomically committed ones |
| A generic commit-hook **registry** with `append(&mut self, tx)` | Rejected by the P2 autonomous audit. The transition identity would have to live in mutable hook state, so a `transact` body returning early could leave the journal describing the previous transition; a `Box<dyn CommitHook>` behind a `&self` store needs interior mutability, contradicting ADR-0024's single-mutex claim; and P2 registers exactly one participant, so a registry buys ordering questions and a lifecycle in exchange for nothing. The explicit `DurableTransition` parameter gives the same P3 seam with none of that |
| Have P2 own `store_meta.next_seq` | `seq` is `serea-event-bus`'s per Crate Map §3.1, and a per-host gapless counter is precisely the state that must not exist outside the commit transaction |