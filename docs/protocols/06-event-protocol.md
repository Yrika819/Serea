# Event Protocol

Protocol ID: `PROTO-EVENT` · Surface: `serea.event/1` · Status: **FROZEN for P0** · Architecture: `serea-arch/2.3.0`

Events are Serea's structured record of what it did and what it observed. They
are the substrate for the Android Activity Timeline, the audit trail, and the
post-incident reconstruction.

The rule that matters: **a human-readable log string is not an event.** Logs
are for humans reading a terminal. Events are for machines rendering a
timeline, correlating a cause chain, and proving what happened. Serea has both,
and they are different things.

---

## 1. Core principle

> Events are structured, durable, ordered, and append-only. Evidence is a
> fact. Narrative is a rendering.

Three consequences:

1. **Events are the protocol.** The Android client renders them. Any UI that
   reconstructs state by scraping text is doing it wrong.
2. **Event content is append-only and immutable while retained.** Retention
   deletes the complete content object. Separate minimal sequence metadata
   records intentional expiry and is not event content.
3. **Every event is attributable.** Each carries an `actor` and a `causation`
   chain back to a user instruction or a durable schedule.

## 2. `SereaEvent`

```json
{
  "envelope_version": "1",
  "surface": "serea.event/1",
  "message_id": "evt_01JQ8ZB7H2XKM9P4QW7NRT5YCD",
  "seq": "10427",
  "kind": "CAPABILITY_COMPLETED",
  "occurred_at": "2026-10-01T09:14:23.902Z",
  "correlation_id": "tsk_01JQ8Z9K3M7QWXR4V2T6YH0BNA",
  "causation_id": "evt_01JQ8ZB5G1XKP7N9M3QRT2V8WC",
  "actor": { "kind": "HOST", "id": "serea-core", "version": "0.1.0" },
  "data_class": "PERSONAL",
  "trace": { "task_id": "tsk_…", "step_id": "stp_…", "attempt": 1 },
  "payload": {
    "capability_id": "calendar.events.list",
    "status": "SUCCEEDED",
    "duration_ms": 412,
    "output_digest": "sha256:…"
  }
}
```

`seq` is a **monotonically increasing, gapless, per-host sequence number**,
serialized as a decimal string (it will exceed JavaScript's safe integer). It
is assigned at commit time, inside the same transaction as the state change the
event describes.

This is what makes the Activity Timeline resumable: a device that reconnects
supplies its verified cursor and receives retained events plus explicit
intentional-expiry ranges through a committed high-water snapshot. Expired
content is never reconstructed.

### 2.1 Actor

| `kind` | Meaning |
| --- | --- |
| `HOST` | Serea Core itself — the scheduler, recovery, retention. |
| `USER` | A human, on a named device. |
| `MODEL` | A model call, quoted by `model_id`. Never an authority. |
| `PROVIDER` | An external provider. |
| `SCHEDULER` | A durable schedule firing. |
| `SYSTEM` | Maintenance: sync, migration, cleanup. |

An `actor` of kind `MODEL` records that a model was involved. It never records
that a model *decided* something.

P2A implements Accepted ADR-0023 category O to actor.id on unchanged event/1,
including exact identifier subtraction and pinned boundary whitespace. This is a
validation tightening only: C1 remains refused, and U+2028/U+2029 are prohibited;
no event/1 opaque-token domain widening is authorized. Event schema and every
other ActorId occurrence must share the Rust verdicts.

## 3. Event kinds

Frozen at P0. Adding a kind is an architecture-minor change; renaming or
repurposing one is major.

### 3.1 Task lifecycle

| Kind | When |
| --- | --- |
| `TASK_CREATED` | Task durably accepted |
| `TASK_STARTED` | Task left `RECEIVED` and began work (including on recovery) |
| `TASK_STATE_CHANGED` | Any legal state transition |
| `TASK_COMPLETED` | Terminal success |
| `TASK_FAILED` | Terminal failure, with `reason_code` |
| `TASK_CANCELLED` | Cancelled, with `cancelled_by` |
| `TASK_BLOCKED` | Entered `BLOCKED`, with `blocked_reason` |
| `TASK_RESUMED` | Left `BLOCKED` or `WAITING_*` |

### 3.2 Model activity

| Kind | When |
| --- | --- |
| `MODEL_CALLED` | Before dispatch, with `model_id`, `purpose`, token estimate |
| `MODEL_COMPLETED` | After response, with usage and `finish_reason` |
| `MODEL_FAILED` | Provider error, with `ModelError` kind |
| `MODEL_OUTPUT_INVALID` | Structured output failed validation; `repair_attempts` recorded |
| `MODEL_REPAIRED` | A repair call succeeded |
| `MODEL_FALLBACK` | Routing advanced to a different model, with `fallback_from` and `reason` |

### 3.3 Capability activity

| Kind | When |
| --- | --- |
| `CAPABILITY_REQUESTED` | An `ActionRequest` was constructed |
| `CAPABILITY_COMPLETED` | A provider returned; status in payload |
| `CAPABILITY_DENIED` | Policy returned `Deny` |
| `CAPABILITY_UNAVAILABLE` | Backing condition absent |
| `CAPABILITY_DUPLICATE_SUPPRESSED` | An equivalent action was already done; the prior result/receipt is returned and no provider invocation occurs |
| `CAPABILITY_RECEIPT_RECORDED` | A `SideEffectReceipt` was persisted |
| `CAPABILITY_RECONCILED` | An `AMBIGUOUS` result was resolved by read-back |
| `MODEL_SCHEMA_VIOLATION` | Model-authored data included invalid host-resolved fields or attempted to supply authority fields |
| `TOOL_DUPLICATE_WINDOW_BYPASSED` | A permitted `SYSTEM` resync bypassed duplicate suppression under Bounds Protocol §5.1 |

### 3.4 Approval activity

| Kind | When |
| --- | --- |
| `APPROVAL_REQUIRED` | Request raised |
| `APPROVAL_GRANTED` | Grant created |
| `APPROVAL_DENIED` | User refused |
| `APPROVAL_EXPIRED` | Request expired unused |
| `APPROVAL_CONSUMED` | A use was consumed by a step |
| `APPROVAL_EXPIRED_UNUSED` | Grant hit expiry with uses remaining |

### 3.5 Policy and bounds

| Kind | When |
| --- | --- |
| `POLICY_CHANGED` | Rules or the disabled overlay changed |
| `BOUND_EXCEEDED` | A host bound was hit; payload includes `bound_name`, limit, and observed value. For repeated-action exhaustion, `bound_name` is `max_identical_action_repeats`. |
| `BOUNDS_CHANGED` | An administrator raised a bound; includes before/after diff, actor, and reason |
| `POLICY_VIOLATION_ATTEMPT` | A request tried something the policy forbids |
| `MODEL_BUDGET_EXHAUSTED` | The model-call budget was exhausted |
| `MODEL_FALLBACK_EXHAUSTED` | The configured fallback bound was exhausted |

### 3.6 Device activity

| Kind | When |
| --- | --- |
| `DEVICE_CONNECTED` | Device session established; payload is the closed `DeviceConnectedPayloadV1` object `{"device_id":"<DeviceId>"}` |
| `DEVICE_DISCONNECTED` | Session ended, with reason |
| `DEVICE_PAIRED` | New device bound |
| `DEVICE_UNPAIRED` | Device removed |
| `DEVICE_CAPABILITIES_REPORTED` | Device reported its capability set |
| `DEVICE_REVOKED` | Host revoked the device credential and sessions |

`DEVICE_CONNECTED` uses the closed `DeviceConnectedPayloadV1` payload with
exactly one required member, `device_id`, validated using the existing
`DeviceId` grammar. The payload identifies only which device session became
established; task eligibility comes only from `DeviceResumeWaitV1` under
Scheduler Protocol. A host that uses this event for Scheduler work must reject
missing, malformed, or unknown payload members and must not resume a task from
event correlation or origin metadata. Clients that do not interpret this
kind-specific payload continue to follow the ordinary unknown-payload and
timeline rendering rules; the `SereaEvent` object and `serea.event/1` surface
are unchanged.

### 3.7 Memory and proactive

| Kind | When |
| --- | --- |
| `MEMORY_ITEM_WRITTEN` | A memory item was created, with provenance |
| `MEMORY_ITEM_UPDATED` | An item was superseded |
| `MEMORY_ITEM_DELETED` | An item was removed, with reason |
| `DELETION_CASCADE_COMPLETED` | One right-to-delete cascade transaction committed, with the deletion counts and tombstones it wrote ([Data Classification §8.2](09-data-classification-protocol.md#82-deletion-cascades) step 4) |
| `PROPOSAL_CREATED` | The proactive watcher produced a suggestion |
| `PROPOSAL_DISMISSED` | The user dismissed a proposal |

`DELETION_CASCADE_COMPLETED` is the **completion record of one cascade
transaction**, and its meaning is deliberately narrow:

- The delete statements, provenance/blob deletion, tombstones, and event insert
  occur in that order in one transaction, followed by one COMMIT. The event is
  inserted after deletion work succeeds but before that same transaction
  commits. The payload carries the counts, so a partial failure is visible
  rather than silent.
- It is **not** a per-item deletion event. `MEMORY_ITEM_DELETED` records that
  one item was removed and why; `DELETION_CASCADE_COMPLETED` records that a
  whole cascade committed. Neither replaces the other, and this kind does not
  report task deletion or retention, which are governed by
  [Task Protocol §8](02-task-protocol.md#8-task-retention-and-privacy).
- It proves only what its transaction did. It carries no authority, and the
  presence of one says nothing about an effect outside that transaction
  (`E9`: an externally visible effect still needs its own receipt event).

It was registered by [ADR-0017](../decisions/ADR-0017-deletion-cascade-completed-event-kind.md),
which records why Data Classification §8.2 remains authoritative for the
requirement and why adding an event kind is an architecture-minor change.

### 3.8 Event history and sequence integrity

| Kind | When |
| --- | --- |
| `EVENT_HISTORY_EXPIRED` | A cursor predates the compacted sequence prefix; response identifies the new valid replay boundary |
| `EVENT_SEQUENCE_CORRUPTION` | A sequence with no content and no valid intentional-expiry metadata is absent; replay stops and reports integrity failure |

### 3.9 Scheduler activity

| Kind | When |
| --- | --- |
| `SCHEDULE_CREATED` | A schedule was durably created |
| `SCHEDULE_UPDATED` | A schedule definition or state was durably changed |
| `SCHEDULE_PAUSED` | A schedule was paused by its owner or host policy |
| `SCHEDULE_RESUMED` | A paused schedule was resumed |
| `SCHEDULE_CANCELLED` | A schedule was cancelled |
| `SCHEDULE_OCCURRENCE_MISSED` | A due occurrence was skipped under its frozen missed-occurrence policy |
| `SCHEDULE_TASK_CREATED` | A due occurrence was durably deduplicated and created its scheduled task |
| `SCHEDULE_CATCH_UP_DEFERRED` | A per-wake catch-up ceiling was reached; remaining due occurrences remain queued for a later wake |

### 3.10 Provider sync

| Kind | When |
| --- | --- |
| `PROVIDER_SYNC_STARTED` | An incremental sync cycle began |
| `PROVIDER_SYNC_COMPLETED` | Cycle finished, with counts |
| `PROVIDER_SYNC_DEGRADED` | Cycle fell back to full resynchronization |

## 4. `reason_code` and `blocked_reason`

Both are stable machine-readable enums, never free text. Free-text detail lives
in a separate, explicitly optional `detail` field that is never used for control
flow.

This is the same principle as the capability protocol: **a control-flow
decision must never be made by parsing prose.**

`reason_code` examples: `PROVIDER_ERROR`, `BOUND_EXCEEDED`, `POLICY_DENIED`,
`APPROVAL_DENIED`, `MODEL_BUDGET_EXHAUSTED`, `SCHEDULER_CANCELLED`,
`INVARIANT_VIOLATION`.

`blocked_reason` examples: `DEVICE_OFFLINE`, `CREDENTIAL_REVOKED`,
`AMBIGUOUS_EFFECT`, `PROVIDER_OUTAGE`, `UNRECOGNISED_STATE`,
`CAPABILITY_UNAVAILABLE`.

## 5. Ordering and delivery

- `seq` is assigned at commit and is strictly increasing with no gaps at
  creation. After retention, replay gaps are resolved only by typed
  `INTENTIONALLY_EXPIRED_RANGE` metadata; sequence values alone cannot
  distinguish intentional expiry from corruption.
- Commit atomicity: the event and the state change it describes are written in
  **one transaction**. An event that exists always describes a change that
  happened; a change that happened always has its event.
- Delivery to devices is **at-least-once**. Devices deduplicate by
  `message_id`. Exactly-once delivery is neither claimed nor needed, because
  every event carries a stable id.
- Within a task, events are totally ordered. Across tasks, only `seq` order is
  guaranteed, and that is sufficient.

The P3 Storage API accepts a typed `SereaEvent` draft, replaces its sequence
with the next host sequence, and writes the canonical complete object and
sequence state through the caller's existing transaction. It enforces the
32,768-byte canonical payload bound and 16-event outer-transaction bound before
commit. An append failure rolls back its sequence allocation; an enclosing
transaction or savepoint rollback removes both the event and allocation.
Storage refuses PRIVATE content without an at-rest protection backend and
refuses SECRET/CREDENTIAL event content.

The event-store byte bound uses deterministic logical accounting rather than
SQLite file size: canonical event-object UTF-8 bytes plus 8 bytes per active
sequence-ledger row and 16 bytes per detailed intentional-expiry range. Fixed
singleton metadata, SQLite page/index/WAL overhead, and Scheduler tables are
excluded. See [Bounds Protocol §2](10-bounds-protocol.md#2-the-bound-set).

## 6. The Activity Timeline

The Android Activity Timeline is a projection of the event stream.

Required behaviour:

1. It renders `seq`-ordered events, resumable from the client's last-seen
   `seq`.
2. **Unknown kinds are skipped, not fatal.** A client that does not recognise
   `kind: "PROVIDER_SYNC_DEGRADED"` renders nothing for it and continues. It
   does not crash and does not break the stream.
3. `data_class` is enforced before render: a `PRIVATE`-class event payload is
   redacted per the data-classification protocol before it crosses the device
   link.
4. Replay uses the `serea.device/2` result representation. A page is ordered and
   contains retained event items and/or exact `INTENTIONALLY_EXPIRED_RANGE`
   items. For a retained event, the consumer processes/deduplicates it. For an
   intentional range, it advances the monotonic verified cursor across exactly
   the declared inclusive range without inventing event content. Range items
   never carry payload, identity, or content-derived data.
5. A cursor older than the compacted prefix receives typed
   `HISTORY_EXPIRED_PREFIX` with `new_replay_boundary`, equal to the
   compacted-through high-water, and emits `EVENT_HISTORY_EXPIRED`. The client
   displays an explicit history-expired marker and uses that value as its
   exclusive `after_seq` cursor on the next request. Prefix expiry is distinct
   from an interior intentional range.
6. An allocated sequence with neither retained content nor valid expiry
   metadata is `CORRUPTION`. The host emits `EVENT_SEQUENCE_CORRUPTION`, stops
   without advancing beyond the unexplained sequence, and the client retains
   its last verified cursor and reports an integrity warning. It must not
   relabel the absence as retention.
7. A cursor at or beyond the current committed high-water mark returns an empty
   page and leaves the cursor unchanged.
8. Events are shown with their `actor` and causation, so "why did this happen"
   is answerable from the timeline alone.

## 7. Audit use

The event log is the audit trail. Three questions must be answerable from it
alone:

1. **What did Serea do?** — replay the `seq` stream.
2. **On whose authority?** — walk `causation_id` back to a `USER` actor or a
   `SCHEDULER` trigger, crossing `APPROVAL_GRANTED` events for anything
   requiring approval.
3. **What did it cost and touch?** — sum `model_usage`, and enumerate
   `CAPABILITY_RECEIPT_RECORDED` events for external effects.

An effect with no receipt event did not verifiably happen. An approval with no
corresponding `APPROVAL_CONSUMED` event was never exercised.

## 8. Retention

| Event class | Retention |
| --- | --- |
| Task lifecycle, capability, approval | Task retention (default 30 days) |
| Model activity | 30 days |
| Provider sync | 7 days |
| Device connect/disconnect | 30 days |
| `POLICY_CHANGED`, `POLICY_VIOLATION_ATTEMPT`, `BOUNDS_CHANGED` | 1 year |
| Scheduler lifecycle and occurrence events | Schedule retention; an occurrence's task events follow task retention |
| Event history and sequence-integrity events | 1 year |

`POLICY_CHANGED` outliving the task it relates to is intentional: policy history
is an audit artifact, not a task artifact.

Identifiers such as `TaskId`, `StepId`, and related trace IDs in an event are
immutable opaque historical values. Event content must not use task/step foreign
keys with `ON DELETE CASCADE` or `ON DELETE SET NULL`; deleting a task never
mutates a retained event. Event retention removes complete content objects
independently. Minimal sequence metadata contains no task/step/device/actor
identity, payload digest, payload-derived fingerprint, user content, schedule
arguments, or PRIVATE/SECRET/CREDENTIAL data. Prefix compaction may remove
detailed sequence state below a contiguous high-water only after no detailed
proof below it is needed; the compact boundary remains sufficient to return
`HISTORY_EXPIRED_PREFIX`. See [ADR-0026](../decisions/ADR-0026-event-retention-and-global-sequence.md).

## 9. Invariants summary

| # | Invariant |
| --- | --- |
| E1 | Events are structured data with stable kinds; prose is never the protocol. |
| E2 | Retained event content is immutable; expiry deletes complete content and records minimal intentional-expiry metadata atomically. |
| E3 | An event and its state change commit in one transaction — never one without the other. |
| E4 | Per-host `seq` allocation is monotonic and gapless at creation, transactionally assigned; every allocated seq is durably accounted for as retained content, declared expiry, or compacted prefix. |
| E5 | Every event has an `actor` and a `causation` chain to a user or a schedule. |
| E6 | Control flow never depends on parsing `reason_code` prose; codes are enums. |
| E7 | Unknown event kinds are skipped by clients, never fatal. |
| E8 | `data_class` is enforced before an event crosses the device link. |
| E9 | An externally visible effect has a receipt event; absence means unverified. |
| E10 | `MODEL` as an actor records involvement, never authority. |
---

## 10. Changelog

Entries required by [Protocol Index §7](00-protocol-index.md#7-change-control)
for every change to this protocol. A change to the `EventKind` set also moves the
architecture version per [§4.1](00-protocol-index.md#41-semantics).

| Architecture version | Change | Kind | Authority |
| --- | --- | --- | --- |
| `serea-arch/0.2.0` | Added `DELETION_CASCADE_COMPLETED` to §3.7. It is the completion record of one right-to-delete cascade transaction, carrying the deletion counts, as required by [Data Classification §8.2](09-data-classification-protocol.md#82-deletion-cascades) step 4. No existing kind was renamed, repurposed, or removed; the wire surface remains `serea.event/1`; unknown kinds still fail closed on a host parse and are still skipped by clients (§4.2 rules 3 and 4, §6 rule 2, `E7`). | Minor — a backward-compatible addition | [ADR-0017](../decisions/ADR-0017-deletion-cascade-completed-event-kind.md) |
| `serea-arch/2.0.0` | Accepted Option A: separates minimal sequence accountability from independently expirable complete content; replay distinguishes exact interior intentional-expiry ranges, compacted-prefix history expiry, and unexplained corruption. `serea.event/1` objects remain unchanged; replay response moves to `serea.device/2`. | Major architecture/replay semantics; event surface unchanged | [ADR-0026](../decisions/ADR-0026-event-retention-and-global-sequence.md) |
| 2026-10-06 | Clarified that deletion work and `DELETION_CASCADE_COMPLETED` insert precede the one COMMIT inside the same transaction. This reconciles ADR-0017 wording with E3 and Data Classification §8.2; a post-commit append is forbidden. | Editorial clarification of accepted transaction semantics | Owner direction; E3; Data Classification §8.2 |

## 9. P2A validation changelog and deferred runtime seam

- 2026-10-03: event/1 actor.id category O tightening ratified and implemented.
  The coordinator records current final workspace/MSRV validation, test counts,
  review and integration status in the [closure record](../plans/P2A-review-and-closure.md).
  Event major and envelope version stay 1, no event runtime delivered.
- ADR-0021 remains Proposed for runtime: P2 non-event journal/history does not
  establish E3/E4. Future event participant runs in the successful state transaction;
  E3 is forward-only from P3, never historical event_seq backfill. P2 pending
  transition count is journal row count, not a queue. No event runtime lands P2A.
