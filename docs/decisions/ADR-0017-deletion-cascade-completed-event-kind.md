# ADR-0017: `DELETION_CASCADE_COMPLETED` Is a Registered Event Kind

- Status: **Accepted**
- Architecture version: `serea-arch/0.2.0`
- Decision date: 2026-10-02

## Context

The right-to-delete flow is frozen in
[Data Classification Protocol §8.2](../protocols/09-data-classification-protocol.md#82-deletion-cascades).
A "forget this" request is **one transaction**, and step 4 of that transaction
reads:

> Record `DELETION_CASCADE_COMPLETED` with the counts, so a partial failure is
> visible rather than silent.

The event log is the substrate for the activity timeline and the audit trail
([Event Protocol §1](../protocols/06-event-protocol.md#1-core-principle)), and
[Event Protocol §7](../protocols/06-event-protocol.md#7-audit-use) requires that
"what did Serea do?" be answerable from the event stream alone. A deletion
cascade that removes memory items, provenance rows, and content-addressed blobs
is exactly the kind of world-affecting change the event log exists to record.

## The existing inconsistency

`DELETION_CASCADE_COMPLETED` was **not** in the frozen `EventKind` table at
[Event Protocol §3](../protocols/06-event-protocol.md#3-event-kinds). The P1
implementation resolved the discrepancy by letting the frozen table win: the Rust
enum and `event.schema.json` both refused the name, and
`docs/plans/P1-closure.md` recorded it as a "P0 internal inconsistency noted, not
resolved".

That resolution is safe but it is not correct, and it cannot stay. The host is
left with two ways to satisfy §8.2 step 4, and both are contract violations:

1. **Emit no event**, and the cascade's deletion counts are recorded nowhere.
   A partial failure becomes silent — the exact outcome §8.2 step 4 exists to
   prevent — and the audit trail cannot answer whether a forget request
   completed.
2. **Emit a different kind**, most temptingly `MEMORY_ITEM_DELETED`. That would
   be a false record: that kind means *one item was removed, with reason*
   ([Event Protocol §3.7](../protocols/06-event-protocol.md#37-memory-and-proactive)),
   whereas a cascade is one transaction that also deleted provenance rows and
   blobs and wrote tombstones. Overloading it would make the timeline lie about
   both the granularity and the scope of the deletion, and it would silently
   repurpose a frozen kind — which
   [Protocol Index §3](../protocols/00-protocol-index.md#3-capability-identifier-grammar)
   and Event Protocol §3 make an architecture-major change.

## Decision

**`DELETION_CASCADE_COMPLETED` is added as an official `EventKind`**, registered
in [Event Protocol §3.7](../protocols/06-event-protocol.md#37-memory-and-proactive)
beside the memory and proactive kinds, and reflected in the Rust `EventKind`
enum, in `event.schema.json`, and in the protocol's changelog.

Its meaning is deliberately narrow:

- It is the **completion record of one cascade transaction**. The deletion
  statements, provenance/blob deletion, tombstones, and
  `DELETION_CASCADE_COMPLETED` insert occur in that order in one transaction,
  followed by one COMMIT. “After the deletion work succeeds” means after those
  statements succeed but before the same transaction commits. Its payload
  carries the counts and evidence §8.2 step 4 requires.
- It is **not** a generic deletion event. It does not report task deletion and
  does not report retention; both remain governed by
  [Task Protocol §8](../protocols/02-task-protocol.md#8-task-retention-and-privacy).
- It does not replace, rename, or repurpose `MEMORY_ITEM_DELETED`, any task
  deletion event, or any retention event. Those are unchanged.
- It proves only what its own transaction did. It carries no authority, and its
  presence is not evidence of an effect outside that transaction: an externally
  visible effect still requires its own receipt event (`E9`).

### Why Data Classification §8.2 remains authoritative

The requirement was not invented by this ADR; it was frozen in P0 and is not
being changed. §8.2 is the owning protocol for what a deletion cascade *must*
do, and Event Protocol §3 is the registry that names *how* the host records it.
A protocol that names an event the registry does not contain is an
inconsistency in the registry, not a licence to drop the requirement. Amending
§8.2 instead would mean weakening a frozen privacy obligation to fit a registry,
which is the wrong direction of travel for a right-to-delete guarantee.

### Why this is an architecture-minor change

[Protocol Index §4.1](../protocols/00-protocol-index.md#41-semantics) defines a
minor change as a backward-compatible addition, and names "a new event kind" as
an example. Two further clauses confirm it:

- §4.2 rule 4: "A new event kind is a minor change and older clients skip
  unknown kinds rather than failing the stream." The compatibility rule was
  written for precisely this case.
- Event Protocol §6 rule 2 and invariant `E7`: unknown kinds are skipped by
  clients, never fatal. Event Protocol §3 says the same in its own words:
  "Adding a kind is an architecture-minor change; renaming or repurposing one is
  major."

Nothing is renamed, repurposed, narrowed, or removed. No existing kind changes
meaning. No wire-protocol major moves: the surface stays `serea.event/1`. No
identifier grammar, task state, `RiskClass`, `DataClass`, `ActionErrorKind`, or
`ProviderId` set is touched, so none of the items Protocol Index §4.3 freezes
for P0 is affected. The architecture version therefore moves
`serea-arch/0.1.0` → `serea-arch/0.2.0`.

No migration note is required: §7 item 4 scopes a migration note to `Major`.

## Consequences

- The host can satisfy Data Classification §8.2 step 4 without emitting an event
  kind the frozen registry does not contain, and without overloading
  `MEMORY_ITEM_DELETED`.
- A right-to-delete cascade becomes answerable from the audit trail alone,
  including its counts and its partial failures.
- Every consumer of the event stream must tolerate one additional kind. Under
  §4.2 rule 4 and `E7` that is already required behaviour — skip, do not fail —
  so the addition is compatible by construction rather than by migration.
- Fail-closed parsing is unaffected. An event whose `kind` is outside the
  registered set is still refused by the host parse (Protocol Index §4.2 rule 3);
  only *registered* kinds changed, and the set of unregistered names that fail
  closed is unchanged.
- The deletion-cascade transaction itself is still unimplemented. This ADR
  registers a name; it does not create a runtime, a store, or a right-to-delete
  code path.

## Required consumers

| Consumer | What it must do |
| --- | --- |
| `serea-protocol` `EventKind` | Register `DeletionCascadeCompleted` in document order within §3.7. |
| `event.schema.json` | Add the wire name to the `eventKind` enum, in the same order. |
| `Event Protocol` §3.7 | Carry the row and the narrow-meaning note (done). |
| Any event-stream consumer, including a future Android client | Skip an unrecognised kind rather than failing the stream (§4.2 rule 4, §6 rule 2, `E7`). |
| The future right-to-delete implementation | Insert this kind once after the deletion/tombstone statements succeed and before the same transaction's single COMMIT, with the counts. |
| Payload validation for this kind | The event payload surface stays forward-compatible; the cascade counts are the emitting layer's obligation under §8.2 step 4. |

## Rejected alternatives

- **Remove the event from deletion-cascade semantics**, editing Data
  Classification §8.2 step 4 to stop naming it. Rejected: it deletes a frozen
  privacy obligation to make a registry tidy. The whole point of step 4 is that
  a partial cascade failure is visible; dropping the record makes a partial
  deletion indistinguishable from a complete one, which is the failure mode the
  step exists to prevent.
- **Reuse `MEMORY_ITEM_DELETED`.** Rejected: it is a false record at two
  granularities (one item vs. one transaction) and a silent repurposing of a
  frozen kind, which Event Protocol §3 makes architecture-major. It would also
  lose the counts, so the §8.2 obligation would still be unmet while the audit
  trail claimed something that did not happen.
- **Widen it into a general deletion event.** Rejected: it would merge cascade
  completion, per-item deletion, task deletion, and retention into one kind,
  destroying the ability to answer "what exactly was deleted, and in which
  transaction" from the timeline — the opposite of Event Protocol §7.
- **Express the cascade as a task state or an `ActionErrorKind`.** Rejected:
  neither is an event. Bound exhaustion and task outcomes are separate frozen
  mechanisms, and `ActionErrorKind` is frozen at
  `serea-arch/0.1.0` (Protocol Index §4.3, Bounds Protocol §4.4).

## Frozen source docs

[Protocol Index §§4.1, 4.2, 4.3, 7](../protocols/00-protocol-index.md#4-versioning);
[Event Protocol §§1, 3, 6, 7, 9](../protocols/06-event-protocol.md#3-event-kinds);
[Data Classification Protocol §8.2](../protocols/09-data-classification-protocol.md#82-deletion-cascades);
[Task Protocol §8](../protocols/02-task-protocol.md#8-task-retention-and-privacy);
[Bounds Protocol §4.4](../protocols/10-bounds-protocol.md#44-the-bound_exceeded_-family).
