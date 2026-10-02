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

### `serea-storage` owns the transaction and a commit-hook registry

```rust
pub trait CommitHook {
    /// Write this participant's rows into the caller's open transaction.
    fn append(&mut self, tx: &mut Tx) -> Result<(), StoreError>;
}
```

`Store::transact` opens `BEGIN IMMEDIATE`, creates one `Tx`, calls each registered
hook's `append`, then `COMMIT`s. A hook's rows therefore commit or roll back
exactly with the state change — which is the *mechanism* `E3` requires. What a
hook writes is the hook's business.

Every P2 mutation path is built as `transact(|tx| { …engine writes…; journal.append(tx)?; Ok(()) })`.
There is no public `Store` method that writes state on its own, so no P2 caller
can bypass the journal.

### P2 registers exactly one hook: `TaskJournal`

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

### P3 registers a second hook and gets `E3` for free

`serea-event-bus` implements `CommitHook`. Its `append` writes `serea_events` and
allocates `seq` from a `store_meta.next_seq` counter, **inside the same `Tx`**.
Because both hooks run inside one `BEGIN IMMEDIATE`, `E3` becomes true at the
moment P3 exists, and **not one P2 state-transition function changes**. P3's last
obligation is back-filling `task_journal.event_seq` for rows where it is `NULL`.

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
`pending_event_transitions: u64`, counting `task_journal` rows whose `event_seq IS
NULL`. A P2 test asserts the count is greater than zero after a task creation. An
operator can therefore see the `E3` debt from inside the product, and a P3 upgrade
can measure exactly what it must back-fill.

## What P2 must not create

- **No `serea_events` table.** It belongs to P3. P3's first migration is `0002`.
- **No `store_meta` table and no `next_seq` counter.** `seq` is P3's per
  [Crate Map §3](../architecture/03-crate-map.md#3-crate-inventory).
- **No `EventKind` construction, no `Actor` construction, no `Seq` allocation.**

This is why the P2 schema has no event table and no store-metadata table, and it
is a deliberate decision rather than an omission. The prompt asks for the "storage
metadata needed for migrations/recovery"; the answer is that P2 needs none, because
`schema_migrations` is the single authority and recovery idempotency is structural
rather than marker-based.

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
| `crates/serea-storage/src/tx.rs` | `CommitHook`, the hook registry on `Store`, `transact` invoking them inside the transaction |
| `crates/serea-storage/src/store.rs` | Every mutating path becomes `transact`-shaped; no public method writes alone |
| `crates/serea-task-engine/src/journal.rs` | `TaskJournal` implementing `CommitHook` |
| `crates/serea-task-engine/src/recovery.rs` | `RecoveryReport.pending_event_transitions` |
| `crates/serea-storage/migrations/0001_initial.sql` | `task_journal` with the envelope columns, `payload_json`, `payload_ref_digest`, and `event_seq` nullable. Full DDL in [P2 SQLite schema §4.9](../plans/P2-sqlite-schema.md#49-task_journal) |

## Consequences

- P2 produces a complete, replayable audit trail even though it produces no
  events. `T5` and `T4` are provable without P3.
- `E3` is satisfied structurally rather than by convention: the transaction that
  carries the state change is the transaction the hook writes into.
- The cost is one extra durable table and one extra write per transition in P2.
  That write is what makes P3's back-fill lossless.
- A reviewer can check the honesty of P2's closure claim by grepping for exactly
  one thing: `serea_events` must not appear anywhere in the P2 diff.

## Rejected alternatives

| Alternative | Why rejected |
| --- | --- |
| Create `serea-event-bus` in P2 | Inverts the phase plan and pre-empts P3's fan-out and retention design. Crate Map §4.1's reason for the crate is that it is *separate* |
| Write real `SereaEvent` rows from P2 | `EventKind`, `Actor` and `Seq` are all in `serea-protocol`, so this is *possible* — which is exactly why it is rejected. Crate Map §3.1 assigns the append-only log to the event bus, and a P2 that grows its own will be hard to remove once P3 needs it |
| Two-phase commit across P2 and P3 | SQLite is single-writer and local; a two-phase protocol would buy nothing and add a failure mode |
| A `pending_event` outbox drained by P3 | Same defect as writing real events in P2: the state change is already committed without its event, so `E3` stays violated and the gap is permanent rather than transitional |
| No durable trace in P2 | Destroys the audit trail and blocks `T5` |
| Have P2 own `store_meta.next_seq` | `seq` is `serea-event-bus`'s per Crate Map §3.1, and a per-host gapless counter is precisely the state that must not exist outside the commit transaction |