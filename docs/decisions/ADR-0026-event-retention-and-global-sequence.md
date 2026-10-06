# ADR-0026: Event Retention and the Gapless Global Sequence

- **Status:** Proposed — owner semantic choice required before P3 implementation
- **Architecture version:** `serea-arch/1.0.0`
- **Decision date:** 2026-10-06 (proposal)
- **Scope:** Reconcile Event Protocol E2/E3/E4/E7 with class-specific retention,
  history expiry, deletion, task references, replay, and finite storage.

## Context and contradiction

Event Protocol §2 and E4 require one monotonically increasing, gapless per-host
sequence. §6 says a missing sequence inside retained history is corruption;
the only normal expiry case is a cursor older than the oldest retained event.
E2 calls the log append-only while allowing whole-record deletion at retention.
§8 assigns different horizons: 7 days, 30 days, task/schedule retention, and one
year. Events from those classes interleave in the same sequence. Deleting an
expired short-retention event at sequence N while N-1 and N+1 remain creates an
interior hole that §6/E4 define as corruption. Keeping the row to preserve
continuity may violate the class retention horizon and explicit “forget this”
deletion obligations. This is an architecture contradiction, not an index or
SQL implementation detail.

No option below is authorized for implementation by this Proposed ADR. Migration
0002 must wait for the owner choice because the choice determines durable schema,
replay semantics, and potentially wire compatibility.

## Options

### A. Immutable sequence/header ledger plus separately expirable event content

Allocate each global `seq` once and append an immutable minimal ledger entry.
Store payload/content in a separate row or content segment with its class and
expiry. Retention deletes the complete content record without changing the
ledger. The ledger entry (or compact range representation) distinguishes an
intentionally expired sequence from an unexplained missing sequence. It must
not retain task/step/device IDs, actor identity, payload digest, or other
content-derived data unless the owner explicitly classifies and retains it.

| Concern | Analysis |
|---|---|
| Wire `serea.event/1` | Existing event envelope remains unchanged when content exists. A reader encountering an expired interior seq needs a new response representation (expired range/marker or equivalent); current event/1 `HISTORY_EXPIRED` only reports a cursor before the oldest retained seq. Existing clients would call this interior absence corruption. |
| E2 | Content is removed whole; the minimal sequence ledger remains append-only. This requires redefining “event record” as content plus envelope and treating the ledger as separate protocol metadata. |
| E3 | State, ledger append, content append, and seq allocation remain in the same transaction. Retention is a separate transaction that removes content only. |
| E4 | Allocation remains globally gapless. The ledger proves every seq existed; content can be absent only with an explicit expiry record. The protocol's current equivalence of missing content and corruption must change. |
| E7 | Unknown event kinds still skip when content is available. Old clients cannot understand new interior-expiry responses, so unknown-kind forward compatibility alone does not solve it. |
| `HISTORY_EXPIRED` | Current oldest-retained response handles prefix expiry. Interior expiry needs a distinct range-aware response or cursor rule; expanding its current meaning risks clients advancing incorrectly. |
| Explicit “forget this” | Can physically remove event content while retaining only a minimal seq fact. This best supports deletion, subject to proving that the ledger contains no identifying or content-derived data. |
| Task deletion | Opaque TaskId/StepId values in retained event content are immutable historical identifiers. No event FK may cascade or set NULL. Task deletion removes task rows, not retained events; content can later expire under event retention. |
| Per-class retention | Each content record can expire independently without rewriting another. Scheduler-consumed source events must remain available until its durable cursor passes them or else a durable equivalent must preserve handled/unhandled meaning. |
| Device replay | Requires clients to receive an explicit expiry range/marker and advance only over that declared range. Existing replay rule treats an interior gap as corruption, so device behavior changes. |
| DB schema | `event_sequence_ledger(seq PRIMARY KEY, minimal append-only metadata)` plus `event_content(seq UNIQUE FK to ledger, event envelope/payload, class, expiry)` and an expiry/range index. No FK from retained content to task/step. Expiry transaction deletes whole content rows. |
| Storage bounds | Bound content rows/bytes and ledger/range bytes separately. To stay finite, compact an old contiguous ledger prefix into a high-water checkpoint; preserve detailed ledger entries only over the active retention window. Interior-expiry representation must itself have a bound and deterministic compaction. |
| Version impact | Not backward compatible with current replay semantics as stated. `serea.event/1` may remain the shape of actual events, but response/cursor semantics and E2/E4 architecture meaning change; require explicit architecture version/change-control decision. |

### B. Physical retention of only a contiguous sequence prefix

Delete only the longest sequence prefix for which every event is retention-
eligible. Never delete an eligible event after an ineligible earlier event.
The first surviving sequence defines the oldest retained seq; no interior holes
are produced.

| Concern | Analysis |
|---|---|
| Wire `serea.event/1` | Fully preserves current envelope and replay/`HISTORY_EXPIRED` behavior. |
| E2 | Whole-record deletion remains append-only in the retained suffix. |
| E3 | No change; appends remain atomic with described state. |
| E4 | Preserved directly: retained history is always one contiguous suffix. |
| E7 | Unchanged. |
| `HISTORY_EXPIRED` | Existing oldest-retained behavior works exactly. |
| Explicit “forget this” | Cannot delete one event in the middle. It must wait until every earlier seq is eligible, potentially far beyond its own horizon; that conflicts with prompt privacy deletion. |
| Task deletion | Can keep opaque references until prefix expiry, but cannot remove task-related event content on task deletion without creating a hole. No event FKs should be used. |
| Per-class retention | Durations become minimum eligibility times, not actual deletion deadlines. One-year events can pin newer 7-day events; a single long-lived event can block all later cleanup. |
| Device replay | Existing clients and timeline rules remain valid. |
| DB schema | One append-only event table with seq primary/unique; retention can delete only a contiguous prefix and persist oldest-retained watermark. |
| Storage bounds | Prefix blocking can prevent pruning while appends continue. On cap, writes must stop or privacy deadlines be violated; boundedness is possible only by accepting availability loss. |
| Version impact | No wire/architecture change if longer-than-configured retention and delayed explicit deletion are accepted. That acceptance changes the privacy meaning of existing horizons and deletion promises. |

### C. Retention epochs with global sequence checkpoints and authenticated range proofs

Keep event content in immutable, class-scoped retention segments. At each
retention operation, erase complete expired segments and append an immutable
checkpoint describing the exact expired seq ranges plus a cryptographic hash
root for each range. Retain one compact chain of checkpoints and the active
event suffix; old checkpoints can be folded into a signed/high-water root.
Replay verifies active events against checkpoints and reports intentional
expired ranges distinctly from corruption. This makes deletion and integrity
evidence explicit while bounding per-event ledger state through range
compaction.

| Concern | Analysis |
|---|---|
| Wire `serea.event/1` | Event objects remain unchanged, but clients need new range-proof/replay response semantics. Exposing hashes may also create a durable content-derived identifier and requires classification review. |
| E2 | Event content is deleted whole; append-only history becomes append-only events plus append-only retention checkpoints. |
| E3 | State/event append/seq allocation stay atomic. Retention checkpoint and deletion commit together; checkpoint never claims an uncommitted deletion. |
| E4 | Global allocation remains gapless. Checkpoints prove ranges once held content; an unproved absent seq remains corruption. |
| E7 | Unknown event kinds stay skippable; clients additionally need range-checkpoint support. |
| `HISTORY_EXPIRED` | Existing prefix response is insufficient for interior ranges. A checkpoint-aware response/cursor rule is required. |
| Explicit “forget this” | Supports complete segment deletion with a durable range proof, but the checkpoint/hash may itself preserve a linkable fingerprint; privacy review may require unhashed opaque range-only metadata instead. |
| Task deletion | Retained opaque IDs remain immutable and have no cascading FK; expired segments erase their full event content. |
| Per-class retention | Segment class and eligibility can differ. Segment boundaries must not force retention past a class horizon; split segments or delete individual rows within segments. |
| Device replay | Requires clients to understand retention proofs/ranges and distinguish them from corruption. |
| DB schema | `event_segments`, per-event content, append-only `retention_checkpoints`, compact checkpoint/root metadata, seq allocator, and range proof validation. |
| Storage bounds | Checkpoint folding can bound metadata only if proof roots and audit obligations permit compaction. Segment indexes, active content, and checkpoint bytes each need independent caps. The cryptographic machinery adds failure modes and CPU cost. |
| Version impact | Changes response/cursor semantics and E2 interpretation; requires explicit architecture and likely protocol version decision. |

## Comparison and recommendation

Option A is the clearest fit for independently expiring event content and
minimal audit metadata. It has simpler integrity semantics than Option C and
better privacy control than Option B. Option B is wire compatible but can
violate the stated privacy deletion deadlines and can pin storage indefinitely;
it should be rejected unless the owner explicitly weakens those deadlines.
Option C offers stronger range-integrity evidence than A but retains more
content-derived metadata, requires more machinery, and does not preserve the
current client replay contract either.

Option A is recommended **only if** the owner authorizes range-aware replay and
retention metadata semantics. It is not backward-compatible with the frozen
Event Protocol §6/E4 rule that an interior missing seq is corruption and with
the current single-prefix `HISTORY_EXPIRED` behavior. No clear option is both
privacy-correct and backward-compatible under the current frozen wording.

## Exact owner choice required

Choose one:

1. **A — ledger plus expirable content**, and authorize an architecture change
   defining intentional interior expiry, its replay response/cursor behavior,
   and minimal retained ledger fields; or
2. **B — contiguous-prefix retention**, and explicitly accept that per-class
   horizons are minimums and “forget this”/task deletion may retain event
   content beyond its nominal horizon; or
3. **C — checkpointed retention epochs**, and authorize new proof/checkpoint
   semantics, a wire response change, and the associated privacy review.

Until selected, E2/E4, `HISTORY_EXPIRED`, deletion, retention schema, and
retained-count/byte enforcement are not jointly implementable. ADR remains
Proposed and migration 0002 is blocked.

## Frozen sources

- [Event Protocol §§2, 5–9](../protocols/06-event-protocol.md)
- [Data Classification Protocol §8.2](../protocols/09-data-classification-protocol.md#82-deletion-cascades)
- [Task Protocol §8](../protocols/02-task-protocol.md#8-task-retention-and-privacy)
- [Bounds Protocol §2](../protocols/10-bounds-protocol.md#2-the-bound-set)
- [ADR-0017](ADR-0017-deletion-cascade-completed-event-kind.md)
