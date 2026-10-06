# ADR-0026: Event Retention and the Gapless Global Sequence

- **Status:** Accepted — Option A, selected by owner on 2026-10-06
- **Architecture version:** `serea-arch/2.0.0`
- **Decision date:** 2026-10-06
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

The owner selected Option A and authorized range-aware replay, independently
expirable content, and the minimum replay-response/version change. Migration
0002 and P3 implementation follow the accepted semantics below.

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
| Wire `serea.event/1` | Actual event objects remain unchanged. The replay response is part of `serea.device/2`; it represents an intentionally expired range as a typed item. |
| E2 | Content is removed whole; the minimal sequence ledger remains append-only. This requires redefining “event record” as content plus envelope and treating the ledger as separate protocol metadata. |
| E3 | State, ledger append, content append, and seq allocation remain in the same transaction. Retention is a separate transaction that removes content only. |
| E4 | Allocation remains globally gapless. The ledger proves every seq existed; content can be absent only with an explicit expiry record. The protocol's current equivalence of missing content and corruption must change. |
| E7 | Unknown event kinds still skip when content is available. Old clients cannot understand new interior-expiry responses, so unknown-kind forward compatibility alone does not solve it. |
| `HISTORY_EXPIRED` | Prefix compaction is reported as `HISTORY_EXPIRED_PREFIX` with the new valid replay boundary. Interior expiry is `INTENTIONALLY_EXPIRED_RANGE`, never prefix expiry. |
| Explicit “forget this” | Can physically remove event content while retaining only a minimal seq fact. This best supports deletion, subject to proving that the ledger contains no identifying or content-derived data. |
| Task deletion | Opaque TaskId/StepId values in retained event content are immutable historical identifiers. No event FK may cascade or set NULL. Task deletion removes task rows, not retained events; content can later expire under event retention. |
| Per-class retention | Each content record can expire independently without rewriting another. Scheduler-consumed source events must remain available until its durable cursor passes them or else a durable equivalent must preserve handled/unhandled meaning. |
| Device replay | A page contains ordered retained event items and/or exact intentional-expiry ranges. A consumer advances its monotonic verified cursor over a declared range only; an unexplained absence is corruption and stops replay. |
| DB schema | Active sequence state plus event content, minimal detailed expiry ranges, and compact prefix high-water metadata. No FK from retained content to task/step. Expiry transaction records intentional absence and deletes whole content rows atomically. |
| Storage bounds | Bound content rows/bytes and active ledger/range bytes separately. Fold only a contiguous prefix whose detailed proof is no longer needed into a compact expired-prefix high-water; interior ranges stay explicit while needed. |
| Version impact | Architecture `serea-arch/2.0.0` records the breaking replay semantic change. Bump only the affected replay response surface to `serea.device/2`; keep actual event objects at `serea.event/1` and envelope version 1. Old device/1 clients are rejected for this response rather than silently downgraded. |

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

The owner selected Option A. It is intentionally not backward-compatible with
the prior interior-gap and single-prefix replay rules; Device Protocol/2 makes
that break explicit. Option B and Option C are rejected.

## Accepted Option A semantics

- Sequence allocation is monotonic, gapless at creation, per-host, and
  transactional. Every allocated sequence is accounted for durably.
- Sequence/integrity metadata contains only sequence existence and availability
  state. It must not retain task, step, device, actor, payload, schedule, or
  other content-derived identity or fingerprints.
- Event content is a complete independently expirable object. Expiry metadata
  and whole-content deletion commit atomically. Missing content without valid
  expiry metadata is corruption.
- Replay has typed outcomes: retained event, intentional expired range,
  expired prefix, or corruption. Consumers advance over only retained events
  they process or the exact declared intentional range. Prefix expiry returns
  the new valid boundary. Interior expiry is never `HISTORY_EXPIRED_PREFIX`.
- Detailed ledger/range state is compacted only when all detailed proof below
  a contiguous boundary is unnecessary. The compact prefix high-water remains
  bounded; it does not preserve one row per historical sequence.
- The Scheduler is the only P3 internal durable consumer. Retention policy
  takes precedence over Scheduler backlog. On intentional expiry Scheduler
  skips the exact range without fabricating a match; unexplained absence stops
  as corruption. No new event kind is introduced for this degradation.
- `serea.event/1` event objects and envelope version 1 are unchanged. Device
  timeline replay moves to `serea.device/2`; architecture is
  `serea-arch/2.0.0`. Unrelated wire surfaces do not change.

## Frozen sources

- [Event Protocol §§2, 5–9](../protocols/06-event-protocol.md)
- [Data Classification Protocol §8.2](../protocols/09-data-classification-protocol.md#82-deletion-cascades)
- [Task Protocol §8](../protocols/02-task-protocol.md#8-task-retention-and-privacy)
- [Bounds Protocol §2](../protocols/10-bounds-protocol.md#2-the-bound-set)
- [ADR-0017](ADR-0017-deletion-cascade-completed-event-kind.md)

Migration 0002 may implement these accepted semantics, subject to P3A
cross-document closure and validation.
