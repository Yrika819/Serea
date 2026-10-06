# Serea P3 Preimplementation Audit

- **Status:** `P3A_CLOSURE_IN_PROGRESS`
- **Audit base:** `p3/preimplementation-audit` at `da117b5f9c4572bb989f5bf7f3d61b9cb2886164`
- **Audit branch:** `p3/preimplementation-audit`
- **Scope:** P3 architecture, protocols, docs, and the exact supported-surface registry update required by the `serea.device/2` replay response. No Event Bus/Scheduler runtime, schema migration, new dependency, or event-retention behavior change.
- **Authority reviewed:** repository contracts at the audit base; public branch and CI identities supplied with the task.

## 1. Executive disposition

P3's intended product semantics are substantially specified in frozen Event and
Scheduler Protocols, the Crate Map, and Accepted ADR-0008. P3 is an event log
with a durable ordered timeline and replay, plus a durable schedule/occurrence
trigger that admits or resumes ordinary tasks. Neither is a generic message
broker nor a second task engine.

The owner directions in this closure pass resolve the fixed transaction
composition (Accepted ADR-0025), Scheduler SQL/COMMIT ordering, P3 subscriber
scope and startup/replay boundary, event foreign-key policy, ADR-0017 ordering,
recurrence evaluator, operational bounds, cancellation/claim fence, and command
retry identity. The owner selected Option A in ADR-0026 on 2026-10-06.
Accepted semantics now separate minimal sequence accountability from
independently expirable complete event content, and define exact range-aware
replay, compacted-prefix expiry, and corruption. Architecture moves to
`serea-arch/2.0.0`; only the replay-bearing device surface moves to
`serea.device/2`; actual event objects remain `serea.event/1`.

No P2 behavior changes. P2's `task_journal` remains the pre-P3 non-event audit
record and is never backfilled into `SereaEvent`. P2's `RecoveryReport` field
`pending_event_transitions` continues to mean the count of P2 journal rows; it
is not a delivery queue or migration backlog.

## 2. Source-of-truth precedence and spec inventory

Classification meanings: **FROZEN** is normative protocol/architecture text;
**ACCEPTED_ADR** is an accepted architectural decision; **PROPOSED_ADR** is a
decision proposal that is not ratified; **HISTORICAL** records prior phase
evidence without creating a present guarantee; **NONCLAIM** is an explicit
exclusion; **AMBIGUOUS** lacks one implementation interpretation;
**CONTRADICTORY** has incompatible normative instructions.

| Class | Source and section | Exact semantic meaning | Owner | Implementation consequence | Required test evidence |
|---|---|---|---|---|---|
| FROZEN | `docs/protocols/06-event-protocol.md` §§1–2 | Event is structured durable evidence, not log prose; actor, causation, class, trace, payload, ID and host sequence are envelope facts. Sequence is gapless per host and allocated in the state transaction. | Event Bus + Storage | Persist typed event facts; no log scraping, detached sequence allocator, or model authority. | Schema/round-trip; commit/rollback sequence tests; two-store ordering; causation and actor validation. |
| FROZEN | Event Protocol §3 | Event kinds and meanings are closed; adding a kind is architecture-minor; task/model/capability/approval/device/memory/history/scheduler/provider kinds have distinct meanings. | Protocol; event constructors in Event Bus | Use registered `EventKind`; preserve kind semantics; forward-compatible clients skip unknown kinds while host parsing rejects unregistered ones. | Exhaustive known-kind serialization/schema tests; unknown-client skip and unknown-host-refusal tests. |
| FROZEN | Event Protocol §§5–6 | State and describing event commit together; device delivery is at-least-once with `message_id` dedup; ordering is total within task and globally sequence-ordered; timeline replays from caller's last-seen sequence; retention expiry differs from a retained-range gap. | Storage for atomic commit; Event Bus for log/query/delivery; Core for device link | Atomic append and source-state write; query/cursor semantics and retention/corruption errors; device delivery retries. | Crash matrix; sequence gap/expiry/high-water/replay tests; duplicate delivery tests. |
| FROZEN | Event Protocol §§7–9 | Events provide audit questions; external effects need receipt events; individual events are append-only, subject to whole-record retention; event classes have retention periods. | Event Bus + Storage; capability owns effect receipt facts | Retention must not rewrite payload; event references must not become a second effect authority; implement class-specific expiry. | Retention boundary tests; receipt/effect correlation; append-only API/source guard. |
| FROZEN | `docs/protocols/11-scheduler-protocol.md` §§1,7 | Schedule is authenticated host-owned durable trigger/template/policy ceiling; models can propose but cannot mutate it; no schedule grants approval or raises task authority. | Scheduler; Task Engine owns task authority | Validate owner and host-resolved template; call ordinary task path for every occurrence. | Unauthorized/model schedule mutations refused; policy/approval ceiling tests. |
| FROZEN | Scheduler Protocol §§2–3 | Supported triggers include calendar due, matching committed host event, device session, approval event, core recovery, and retry due. Wakes carry durable source identity. Occurrence key is ScheduleId plus canonical source/local occurrence/TaskId identity. Duplicate wake/recovery reuses the same task mapping. | Scheduler; Event Bus supplies committed events; Task Engine owns task rows | Durable occurrence identity/mapping and re-entry semantics; no callback authority from an in-memory timer. | Duplicate wake, restart, source-event replay, task mapping, and crash tests. |
| FROZEN | Scheduler Protocol §§4–6 | One fenced scheduler lease holder mutates an occurrence at a time; expiry permits reconciliation, not inference; no scheduler lease held through user/device wait. Missed policy is SKIP/RUN_ONCE/RUN_EACH with a per-wake catch-up ceiling; scheduler events share a transaction with schedule/occurrence change. | Scheduler + Storage; Task Engine for spawned task | Occurrence lease/fence must be distinct from TaskStep authority; bounded deterministic catch-up; coordinated schedule/task/event commit. | Two-worker stale-fence tests; cancellation-vs-claim; each missed policy, cap and retry cursor; atomicity tests. |
| FROZEN | Scheduler Protocol §§5,9; Bounds Protocol §2 | IANA local-calendar recurrence, persisted UTC due instant, deterministic DST gap/fold rules; `max_scheduler_catch_up_per_wake` is 10; missed occurrences are not silently discarded. | Scheduler | Recurrence evaluator and timezone rule source are necessary for full contract. `EpochMillis` stores due instants but does not calculate local recurrences. | Fixed-zone gap/fold, timezone update, UTC persistence, missed schedule and clock-jump tests against pinned timezone data. |
| ACCEPTED_ADR | `docs/decisions/ADR-0008-event-driven-durable-scheduler.md` | Scheduler is durable/event-driven, admits and wakes tasks; Task Engine owns task state, steps, leases, recovery; watcher remains read-only; no timer-only authority. | Scheduler + Task Engine | No second state machine or external side-effect path. | Source/API ownership checks; restart and watcher-read-only tests. |
| ACCEPTED_ADR | `docs/decisions/ADR-0017-deletion-cascade-completed-event-kind.md` | Registered `DELETION_CASCADE_COMPLETED` means one completed deletion-cascade transaction with counts; it is not per-item/task-retention event. | Event Protocol + deletion owner when implemented | Keep narrow kind semantics and privacy-aware payload. | Kind/schema and cascade-transaction atomicity test when that subsystem exists; not P3 scope by itself. |
| FROZEN | `docs/architecture/03-crate-map.md` §§1–3 | Acyclic lower-layer dependencies; protocol is L0, Storage/Event Bus L1, Task Engine/Scheduler L3; Event Bus owns event/seq/log/retention; Scheduler owns durable schedules/wakes; Core composes. | Each listed crate | Keep graph below and do not create a new core crate or Storage→TaskEngine edge. | Cargo normal-edge graph/smoke checks; no runtime testkit edge. |
| FROZEN | `docs/architecture/04-execution-pipeline.md` §§1–3,6 | Pipeline stages 9–10 commit receipt/evidence/event/task state together, then deliver via event timeline; worked examples and crash/recovery table make this contract concrete. | Task Engine + Event Bus + Storage; Core delivery | Build the P3 operation→event matrix from actual transition paths; delivery stays after commit and cannot share the state transaction. | For every listed transition, success/rollback event assertions plus delivery replay after commit; map each crash row to a named test. |
| ACCEPTED_ADR | `docs/decisions/ADR-0025-p3-event-participant-composition.md` | Fixed composition has exactly TaskAuditParticipant and EventParticipant; both execute in the same Storage transaction/savepoint over immutable successful-write facts. | Storage; Task Engine journal semantics; Event Bus event semantics | No registry/list, Store re-entry, SQL escape, or dependency inversion. Journal/event/seq/state roll back together. | State+journal+event+sequence success/rollback tests; no-op/refusal contract; P2 rows remain unchanged. |
| RESOLVED WORDING | `docs/decisions/ADR-0017-deletion-cascade-completed-event-kind.md`; Data Protocol §8.2; Event E3 | Delete/provenance/blob statements and tombstones succeed, then event insert occurs before the same transaction's one COMMIT. | Deletion owner + Storage + Event Bus | Post-commit append is forbidden; corrected ADR and protocol changelog record same-transaction order. | Cascade fault points prove delete/tombstone/event all commit or all roll back. |
| HISTORICAL | `docs/plans/P2-closure.md` §§Workspace, nonclaims, next phase | P2 is closed at 0001 with no event bus/scheduler, no E3/E4, no backfill. `pending_event_transitions` is a committed journal-row count. | Storage + Task Engine | Migration 0001 is frozen; preserve report meaning; P3 event guarantee begins only for new transitions after its atomic participant is live. | Migration checksum unchanged; upgrade fixture proves existing count and journal unchanged. |
| HISTORICAL | `docs/plans/P2-test-matrix.md` §§E, F, H, M, O | P2 tests use explicit `EpochMillis`, injected `Clock`/`TestClock`, and no ambient system clock; P2 crash evidence is bounded process/fault evidence, not power-loss certification. | Protocol/Storage/TaskEngine | P3 deterministic core receives explicit time; hosted CI evidence remains environment-scoped. | P3 deterministic time injection; named process-crash tests, no broader durability claim. |
| NONCLAIM | `README.md` Current status/Evidence; P2 Closure §§ADR status/nonclaims | P3 runtime not implemented; no event bus, scheduler, migration 0002, event delivery, event backfill, provider runtime, or power-loss guarantee is claimed. | Project | Audit documentation cannot represent these as shipped. | Docs validation and repository/source inventory. |
| FROZEN | `docs/protocols/09-data-classification-protocol.md` §§2–3,6,8; `docs/decisions/ADR-0010-*` | Classification inherits maximum input class; CREDENTIAL only credential store, SECRET sealed store only, PRIVATE encrypted at rest and controlled cloud egress; unclassified defaults to CREDENTIAL. | Protocol + Storage + each egress owner | Event/schedule content requires declared inherited class, protected content reference policy, retention/deletion handling; P2 ordinary rows remain PRIVATE fail-closed. | Per-class storage refusal/protection/redaction tests; class cannot be lowered through event or schedule derivation. |
| OWNER-SELECTED BOUNDS | `docs/protocols/10-bounds-protocol.md` §2 | Existing catch-up=10, concurrent tasks=8, and lease=120 remain; P3 operational limits now specify scope, refusal, visibility, and zero behavior. | Core owns bound config; enforcing crate owns each check. | Retained content and active sequence/range metadata are bounded separately under Accepted ADR-0026. | Boundary tests; pre-mutation refusal/no partial write; bounded scans/deletes. |
| OWNER-RESOLVED SCOPE | Event Protocol §§5–8; Scheduler Protocol §§2,6 | Device timeline remains replay surface; Scheduler is sole internal durable consumer; no generic external subscribers. Scheduler-specific singleton cursor; notification is wake-only; replay uses committed high-water snapshots. | Event Bus, Scheduler, device timeline | Startup order and replay boundary are specified. | Replay/live race, cursor/task mapping atomicity, startup recovery replay tests. |
| RESOLVED_BY_OWNER | Event Protocol §§2,6,8–10; Accepted ADR-0026 | Option A separates durable sequence accountability from independently expirable event content. | Event Bus + Storage + device replay + Scheduler | Exact interior expiry is a typed range; expired prefix and unexplained corruption are distinct. Architecture/2 and device/2 are recorded; event/1 remains unchanged. | P3B–P3G prove expiry interleaving, replay range, explicit deletion, device/Scheduler cursor, bounded storage. |

### Relevant current implementation facts

- Workspace is four members: `serea-protocol`, `serea-storage`,
  `serea-task-engine`, and dev-only `serea-testkit`.
- Storage owns SQLite `BEGIN IMMEDIATE`, migrations and `Tx`; its audited
  transaction accepts one `TaskAuditParticipant`. The Task Engine owns the
  journal's semantics. Storage and Task Engine do not expose a generic SQL hook.
- Migration 0001 has the P2 task/step/receipt/lease/blob/reference/revision/
  journal structures and the P2 integrity constraints. It has no event table,
  event sequence metadata, schedule tables, schedule occurrence records, or
  scheduler lease.
- `Clock` is `Send + Sync` and returns `EpochMillis`; `TestClock` is explicit
  and deterministic. `EpochMillis` is UTC epoch milliseconds, not a monotonic
  clock or local calendar representation.
- Existing numeric operational bound: `max_scheduler_catch_up_per_wake = 10`.
  Other P3 storage/replay/subscriber/scan limits named in §13 have no selected
  value.

## 3. Event Bus: Serea-specific contract

### What the bus is

It owns durable structured `SereaEvent` construction/persistence, the global
per-host gapless `seq`, append-only retained history, sequence-aware replay, and
device timeline fan-out. A transient in-process signal may be used as a wake-up
optimization, but it cannot be authoritative, replace the durable event, or be
the only way the Scheduler notices committed events. This exact division between
durable query and in-process notification is **not frozen**; see owner decisions.

The bus is not a generic broker contract. Event Protocol defines device
at-least-once delivery and the Scheduler Protocol defines internal event
triggering. P3 does not have authorization to add arbitrary external consumers.
The minimum safe scope is one device timeline query/delivery path and one
internal Scheduler consumer, pending owner decision on cursor persistence and
live wake mechanics.

### Commit, crash, delivery and ordering

Event/state append and sequence allocation must occur inside the same SQLite
transaction as the state transition they describe. P3 must not publish before
commit. A crash before commit leaves neither transition nor event; a crash
after commit leaves both durable. A crash before a device/internal consumer
observes the event leaves durable history to replay. A crash after delivery but
before consumer acknowledgement may redeliver; device delivery is explicitly
at-least-once and deduplicated by `message_id`. No generic exactly-once delivery
is promised.

Event production must have one event per committed semantic transition/event
fact, with stable event identity on replay of that operation and no event on a
refused/no-op/rolled-back write. Event Protocol's per-host sequence is the
global ordering domain; task events are consequently ordered. No separate
subscriber-local ordering domain is defined. Sequence gaps inside retained
history are integrity failures, not normal deduplication.

P2 `task_journal` is **not** Event Bus persistence and is not a publication
queue. It is non-wire P2 audit material for supported task-engine operations.
P3 reads no historic journal row to create an event, does not add `event_seq`,
and does not claim event coverage for lease-only operations or rows deleted by
P2 task cascade. `E3`/`E4` apply forward to operations actually wired to the
Event Bus participant; the set of all runtime mutations that must emit events
must be enumerated at implementation gate.

### Cursor, replay, retention, and unhandled cases

Timeline has a client-provided last verified `seq`; history expiry returns the
oldest retained sequence; a gap stops replay without advancing over it; a cursor
at/above high-water returns empty. Device dedup uses event ID. A durable cursor
table is **not required by the frozen device protocol** because it describes a
client cursor, but storage/retention must have enough state to determine oldest
retained sequence. Scheduler `HOST_EVENT` consumption must deduplicate by source
`EventId`; its durable Scheduler-specific singleton cursor advances atomically
with handled occurrence work. External generic subscriber cursors are out of P3.

No events are silently discarded except whole-record retention at their
configured horizon. Event rows never use `ON DELETE CASCADE` or `SET NULL` to
task/step rows; event identifiers are immutable opaque historical values and
task deletion does not mutate retained events. Accepted ADR-0026 defines the
minimal expiry metadata and typed replay outcomes.

Host parsers reject unknown `EventKind`; forward-compatible clients skip it and
continue. Payloads are structured validated event facts, not arbitrary model
output. The P3 limits are selected in Bounds Protocol §2. Retention count/bytes
are numeric limits; ADR-0026 Option A requires content and sequence metadata to
be enforced within separate deterministic bounds.

## 4. Atomicity contradiction and proposed ADR-0025

### Contradiction A: schedule mutation versus its event

- **Source A:** Scheduler Protocol §1: every mutation is persisted **before**
  its corresponding scheduler event is appended.
- **Source B:** Scheduler Protocol §6: lifecycle events are committed in the
  **same transaction** as the schedule/occurrence state change; Event Protocol
  E3 has the same transaction requirement.
- **Concrete conflict:** append-after-commit admits a durable schedule mutation
  with no event, while same-transaction append forbids that state.
- **Runtime consequence:** crash between state commit and append loses the
  schedule event, violating E3 and making auditing incomplete.
- **Resolution:** E3 and §6 govern. §1's “before” is SQL statement order within
  the same uncommitted transaction: mutate state, append event/allocate seq,
  COMMIT once. It never means two transactions.

### Contradiction B: accepted Storage API versus two participants

- **Source A:** P2 implementation has `Store::transact_with_audit` with exactly
  one `TaskAuditParticipant`; `DurableTransition` is constructed from actual
  successful writes and journal persistence happens inside the operation's
  savepoint.
- **Source B:** Proposed ADR-0021 says P3 adds an Event Bus transaction
  participant alongside the TaskJournal, both from the same facts/transaction,
  and says P2 transition functions do not change.
- **Concrete conflict:** current transaction method accepts one participant,
  while P3 needs both journal and event writes; separate transactions violate
  E3, a generic participant registry was explicitly rejected, and Storage may
  not depend upward on Task Engine/Event Bus.
- **Runtime consequence:** without a concrete adapter/API, every P2-mutating
  operation either misses events or introduces a cycle/second commit boundary.
- **Resolution:** Accepted ADR-0025 fixes exactly two participants in the same
  transaction/savepoint over immutable facts. Task Engine owns journal meaning;
  Event Bus owns event meaning; Storage owns transaction/facts. No registry,
  Store re-entry, post-commit publication, backfill, or upward Storage edge.

### Accepted ADR-0025 (runtime deferred)

**Title:** P3 Event Participant Composition and Atomic Publication

Status: Accepted 2026-10-06. It preserves forward-only E3; one SQLite
transaction commits state, TaskJournal and `SereaEvent` rows; seq is allocated
only there; no post-commit/outbox path, synthetic P2 events, generic mutable
registry, or Storage→TaskEngine/EventBus runtime edge. Participant failure
rolls back all rows and sequence allocation. Exact event coverage remains a P3
operation inventory, not an open composition decision.

### Contradiction C: deletion completion event after commit

- **Source A:** Accepted ADR-0017's Decision says the cascade completion event
  is emitted “after the transaction that deleted the items, the provenance
  rows, and the content-addressed blobs and wrote the tombstones has committed.”
- **Source B:** Data Classification Protocol §8.2 calls the forget operation
  one transaction and step 4 records `DELETION_CASCADE_COMPLETED` with counts;
  Event Protocol E3 requires event and described state to commit together.
- **Concrete conflict:** a post-commit event cannot be in the deletion
  transaction and can be absent after a crash between commit and append.
- **Runtime consequence:** literal ADR wording violates E3 and can leave a
  completed privacy deletion without its completion record.
- **Resolution:** delete statements, provenance/blob deletion, tombstones,
  completion-event insert, one COMMIT. ADR-0017 and Event Protocol changelog
  now state this order. Never use a post-commit append.

## 5. Scheduler: Serea-specific contract

### What is scheduled

A schedule is a durable trigger plus host-validated task template and immutable
per-created-task policy ceiling. It may:

1. create one ordinary `SCHEDULED` task for a calendar occurrence or matching
   committed `HOST_EVENT`;
2. resume an existing eligible task on device/approval/recovery wake;
3. defer bounded due work using a durable retry-due identity.

It does not schedule arbitrary callbacks, provider calls, or effects directly.
The watcher is a separate read-only proposal cycle, not an effecting schedule.
The Scheduler owns schedule/occurrence state, not `AssistantTask` state,
step sequencing, task leases, provider retry state, approval, or policy.

### Clock and time semantics

Use explicit `Clock::now_ms() -> Result<EpochMillis, ProtocolError>` at runtime
boundaries, with injected `TestClock` for deterministic tests. Durable due and
retry instants are UTC `EpochMillis`. Monotonic elapsed duration is a distinct
runtime concept and must not be persisted or substituted for UTC due time. The
frozen recurrence protocol additionally requires IANA local-calendar
calculation and deterministic DST gap/fold policy. That calculation is not
expressible with EpochMillis alone: EpochMillis is enough to persist and order
resolved instants, but a timezone rules source/recurrence evaluator is needed to
derive them. Scheduler Protocol §5 selects pinned Jiff with bundled TZDB for
conversion only; no dependency is added to Cargo in this docs-only closure.

For one-time due triggers, due means `now >= due_at`. Overdue items are processed
according to their persisted missed policy and catch-up bound. Clock moving
forward can make multiple occurrences overdue and invokes that same bounded
policy. Clock moving backward does not undo committed occurrence mappings or
unprocess work; no due item becomes “un-due” after its claim. Duplicate wakeups
resolve through the durable occurrence key. A large jump is a bounded catch-up,
never an unbounded task burst. DST/timezone behavior follows Scheduler Protocol
§5. Exact behavior for timezone database changes is frozen for pending versus
processed occurrences. Jiff `=0.2.38` with always-bundled `jiff-tzdb =0.1.9`
is selected for timezone/DST calculation only; persist evaluator and TZDB
version for each resolved occurrence.

Persisted due instants and lease expiry are evaluated with explicit UTC
`EpochMillis`. A backward wall-clock adjustment may delay lease reclamation; a
forward jump may make a lease appear expired early, so only a generation fence
and mapping reconciliation can make reclaim safe. A monotonic duration may
measure in-process waits but cannot replace persisted UTC due/expiry or survive a
restart. Tests must cover forward/backward jumps; the protocol does not define
maximum clock error or a trusted-clock source.

### Recurrence and operations

Recurrence is in the frozen protocol and cannot be silently omitted from full P3.
It uses the schedule's IANA timezone and local calendar rule, stores resolved
UTC due instants and local occurrence identity, assigns a nonexistent local
time to the first valid instant after a DST gap, and fires a repeated local time
once at the earlier UTC instant. There is no separate generic cron or arbitrary
callback requirement.

Pause/cancel prevent new occurrences after state is revalidated under the
occurrence fence; already-created tasks continue unless independently cancelled
through Task Protocol. Rescheduling changes future occurrences; already-created
tasks keep their frozen template/policy. Claim transactionally revalidates
ACTIVE state, expected schedule revision/generation, unmapped/unprocessed
occurrence, and current lease fence. Cancel-first refuses later claim;
claim-first may continue under ordinary reconciliation and is not silently
cancelled.

## 6. Transaction windows and resolution

| Crash/race window | Must be impossible, repairable, or out of P3 | Transaction/idempotency/recovery rule | Future evidence |
|---|---|---|---|
| Task/schedule state changed, event absent | Impossible for P3 transitions covered by E3 | Same `BEGIN IMMEDIATE` writes state, journal if applicable, event, and seq. A failed event mapper/write aborts the whole operation. | Fault injection after state write and after event insert; fresh-process verify neither row survives. |
| Event durable, described state unchanged | Impossible | Same transaction and rollback boundary as above. | Fault after event insert/before commit leaves no event or source-state write. |
| Schedule occurrence consumed, task not advanced/created | Must not strand a terminally consumed occurrence; committed processing marker may be repairable | Occurrence key and mapping are unique; mapping and task creation are one transaction when creating. Lease expiry reclaims and reads mapping. | Kill at each write; restart yields one mapping/task or no mapping/task. |
| Task advanced/created, schedule occurrence still active/unmapped | Impossible for schedule-created task | In one transaction write occurrence mapping, task creation/state, associated events and relevant cursor. | Triggered rollback tests with a failure at each participant; no orphan task. |
| Event cursor advanced, handler work not committed | Must not occur for durable Scheduler consumption | Cursor/occurrence work commits with task mapping, or cursor is not advanced until idempotent work commits; source EventId unique key prevents duplicate task. | Crash before/after cursor write and handler write; restart has neither skipped event nor duplicate task. |
| Handler work committed, cursor not advanced | Repairable only when work is idempotent | On replay, unique source EventId/occurrence mapping returns prior task, then advances cursor in same transaction. | Replay same source event after crash; exactly one mapping and task. |
| Delivery happened, acknowledgement/cursor missing | Allowed at-least-once delivery | Stable EventId/message_id dedup at consumer; no exactly-once claim. Device client owns its last-seen cursor; Scheduler uses its own singleton durable cursor. | Deliver then crash before ack; duplicate is harmless and ordered. |
| Schedule cancel races due claim | Linearize under Storage transaction/fence | Revalidate ACTIVE state while holding fenced occurrence transaction; cancel-before-claim commit prevents new task; claim/mapping commit first means existing task is retained. | Two-connection interleaving tests in both commit orders, including stale lease. |
| Event publish call retries after caller lost commit result | Must be idempotent by source operation identity | Authenticated common-envelope `message_id` is the durable command key, bound to request digest and stored result; same ID/different body refuses. IDK-1 is not reused because its task/action preimage semantics differ. | Child killed after COMMIT before return; retry yields one event and one seq allocation. |

P2's existing `IDK-1` is defined for task/action idempotency preimages, not
schedule lifecycle commands. Schedule occurrence identity remains
ScheduleId+canonical occurrence/source EventId. Authenticated lifecycle-command
retry identity reuses the envelope `message_id`; its durable receipt binds the
request digest and outcome, without inventing another identifier prefix.

## 7. Journal boundary and P2 compatibility

`TaskJournal != Event Bus persistence`. It is not event sequence-bearing, has a
different kind vocabulary, no wire EventKind/Actor/seq, and some journal rows
cascade with task deletion. It remains the P2 audit record and supports
P2 recovery. Event Bus does not replay it into the public stream. New P3
transitions that require both audit and event facts write both atomically. P3
must not claim that every historical journal row maps one-to-one to a future
event.

P2 `TaskEngine::recover(now, context)` remains authoritative for task
classification/repair and is invoked after storage open and migrations. Scheduler
then independently reconciles schedules/occurrences and resumes by calling the
existing Task Engine with existing TaskIds; it never writes task recovery
decisions itself. Event Bus startup validates sequence/history integrity and
offers replay; it does not repair task state. Composition root startup order:

1. Open Store and apply the installed migrations transactionally.
2. Validate event sequence metadata/log invariants.
3. Run `TaskEngine::recover` once with explicit `EpochMillis` and publish its
   report only after its existing transaction commits.
4. Run Scheduler reconciliation with explicit time and event high-water/cursors;
   acquire/reclaim occurrence leases, reuse mapped task IDs, and process bounded
   due work.
5. Replay Scheduler events from the durable Scheduler cursor through a
   committed high-water snapshot. Events created while handling this replay
   wait for a later pass.
6. Enter live mode; transient notification is a wake optimization only and
   device delivery replays committed history.

Task recovery precedes Scheduler reconciliation and replay. Replay starts at the
durable Scheduler cursor and snapshots committed high-water after reconciliation;
it cannot miss a committed recovery event. No independent Scheduler authority
may race a P2 recovery pass on task rows.

## 8. Ownership / authority matrix

| Responsibility | Task Engine | Storage | Event Bus | Scheduler |
|---|---|---|---|---|
| Task state / state transitions | **Authoritative owner** | Atomic persistence/integrity mechanism | No ownership; emits resulting event facts | May request create/resume; cannot mutate directly |
| Schedule definitions/state | No ownership | Durable row/constraint authority only | No ownership | **Authoritative owner** |
| Occurrence and due processing state | No ownership | Durable transaction/unique/fence mechanism | Source event history only | **Authoritative semantic owner** |
| Event facts, sequence, history, retention | Supplies task transition facts | Atomic durable rows and seq counter; does not define EventKind | **Authoritative semantic owner** | Supplies schedule/occurrence event facts |
| Delivery cursor | No ownership | Persists Scheduler-specific singleton cursor | **Protocol owner**, replay semantics | Owns source EventId and durable internal cursor; device cursor remains client-side |
| Task step retry / provider idempotency | **Authoritative owner**, existing IDK/receipt/recovery | Persists fences/receipts atomically | No ownership | No ownership |
| Schedule retry/catch-up | No ownership | Persists due/claim/fence/cursor | No ownership | **Authoritative owner**, bounded by frozen catch-up=10 |
| Clock access | Receives explicit EpochMillis; no ambient time | Persists validated instants; no hidden `now()` | Event timestamps passed in facts; no ambient time | Injects/receives Clock at runtime boundary; passes EpochMillis into core |
| Task recovery | **Sole task recovery authority** | Executes P2 storage classifications atomically | Validates/replays event history; no task repair | Reconciles schedules and occurrence mappings; calls Task Engine |
| Schedule recovery | No ownership | Fences/transactions | Supplies event history/cursor | **Sole schedule recovery authority** |
| Transaction boundary | Calls storage operation/participant API; not independent commit authority | **Sole SQLite transaction authority** | Supplies event participant/write facts within Storage Tx | Supplies schedule participant/write facts within Storage Tx |
| External side effects | Existing Task Engine→Capability path only | None | None | None; scheduled work follows normal Task Engine path |

No ownership is duplicated; Accepted ADR-0025 fixes participant composition
within this matrix. Accepted ADR-0026 authorizes the range-aware retention and
replay schema described below.

## 9. Minimum crate graph

The minimum graph consistent with current Crate Map and P2 direction is:

```text
serea-protocol
       ↑                 (no internal dependency)
       ├── serea-storage
       ├── serea-event-bus ──> serea-storage
       ├── serea-task-engine ──> serea-storage
       │                      └> serea-event-bus  [required by frozen map]
       └── serea-scheduler ────> serea-storage
                              ├> serea-event-bus
                              └> serea-task-engine

serea-testkit: dev-dependency only
```

| Edge | Decision | Why |
|---|---|---|
| Event Bus → protocol | Required | Owns frozen event types, EventKind, IDs, Clock/data-class contracts. |
| Event Bus → storage | Required | Durable log, ordering, retention, transaction-local sequence. |
| Scheduler → protocol | Required | Schedule/occurrence IDs, event/time/task protocol types. |
| Scheduler → storage | Required | Durable schedule/occurrence state and claims. |
| Scheduler → event bus | Required by frozen Crate Map | Consumes durable host events and produces scheduler event facts. |
| Scheduler → Task Engine | Required | Uses normal task create/resume/recovery API; Task Engine owns task state. |
| Task Engine → Event Bus | Required by frozen Crate Map | Emits task transition event facts through its transaction path. |
| Task Engine → Storage | Existing required edge | Existing P2 state authority. |
| Storage → Task Engine/Event Bus/Scheduler | Forbidden | Would invert layer/close cycles; participant abstractions must point down. |
| Event Bus → Task Engine/Scheduler | Forbidden | Bus cannot invoke the orchestration layers whose facts it records. |
| Task Engine → Scheduler | Forbidden | Scheduler may request work; engine cannot depend back on Scheduler. |
| Runtime crate → testkit | Forbidden | Testkit is dev-only. |

Cases A–D from the audit request: A is accepted/requires EventBus→Storage and
Scheduler→Storage, both protocol edges; B is explicitly in the frozen graph;
C is forbidden because it cycles with TaskEngine→EventBus; D is explicitly in
the frozen graph. `serea-core` composes the graph but is not created in this
phase. No standalone “core” crate is warranted.

The Task Engine→Event Bus and Scheduler→Task Engine edges do not themselves
solve atomicity. Public APIs must pass narrowly scoped facts/ports downward or
through storage transaction capabilities; no crate calls back upward and no
nested `Store::transact` may be used.

## 10. Migration 0002: required, proposed durable objects

**Migration 0002 is required** if P3 implements the frozen durable event stream
and durable schedule/occurrence contract. Migration 0001 remains byte-for-byte
unchanged. Migration 0002 must be one ordered, checksummed append-only upgrade
registered by the existing catalog, and create event and schedule state in one
transaction. No speculative outbox or task_journal alteration is justified.

### Proposed minimum objects (conceptual, not executable DDL)

| Object | Proposed columns/constraints | Why it belongs to P3 | Open detail |
|---|---|---|---|
| `store_meta` singleton | singleton key; `next_seq INTEGER NOT NULL CHECK(next_seq > 0)` initialized to 1 | Proposed ADR-0021 explicitly assigns the Event Bus per-host `next_seq` counter here; transactional gapless allocator must have one authority. | SQLite signed-integer ceiling and sequence exhaustion behavior; no second schema version source. |
| Event sequence/content (Option A accepted by ADR-0026) | Global seq allocator; active minimal sequence state; complete event content; explicit expiry ranges; compacted-prefix high-water. | Durable event history, seq integrity, class retention and replay. | Minimal metadata holds no task/step/device/actor IDs, digest/fingerprint, user content, schedule args, or PRIVATE/SECRET/CREDENTIAL data. No task/step FKs with CASCADE/SET NULL. Frozen event/1 has no `BlobRef`; no hidden wire indirection. |
| `schedules` | `schedule_id PK`; owner identity; state; trigger type and canonical validated recurrence/event predicate; template reference or protected payload ref; `policy_class`; approval policy; timezone; created/updated UTC milliseconds; `next_due_at_ms`; local occurrence identity/version; missed policy; last processed marker; schedule generation/version; schedule lease generation/owner/expiry if lease is held here | Durable schedule definition/state and due instant required by Scheduler Protocol. | Store PERSONAL/PRIVATE fields policy, schedule/template bounds, exact recurrence serialization, index for active next_due. |
| `schedule_occurrences` | schedule ID + canonical occurrence/source key unique; due UTC ms; source event/task identity as relevant; state (pending/claimed/mapped/skipped); `not_before_ms`; lease owner/generation/expiry; mapped task ID; processing timestamps/reason | Dedup, bounded catch-up cursor, crash reconciliation and one occurrence→one task mapping. | Whether scheduler lease belongs per-schedule or per-occurrence, canonical key encoding, cancellation race version and resumable-vs-terminal occurrence states. |
| `schedule_command_receipts` | authenticated envelope `message_id` primary key; canonical request digest; command kind/schedule identity; durable result reference/status | Retry after caller loss returns the committed result; same ID with different body is refused. | Exact result serialization and receipt retention must be bounded; IDK-1 is not used. |

No independent `event_outbox` or synthetic backfill table. No generic
subscriber table is included. The sole Scheduler consumer has a
Scheduler-specific singleton durable cursor/state row and unique occurrence
source EventIds; device timeline cursors remain client-owned.

Candidate indexes, subject to exact query plans: unique event `seq`; event
retention/sequence range index; event source/task correlation index only where
queries need it; active schedule `(state, next_due_at_ms)`; unique
`(schedule_id, occurrence_key)`; due occurrence `(state, not_before_ms,
due_at_ms)`; unique source `EventId` where host-event schedules share a source
domain. No index may substitute for an owner/foreign-key check. Storage tests
must prove indexes match the actual deterministic claim/replay query.

### Upgrade, rollback and recovery

There are no deployed databases per existing P2 planning evidence; still,
upgrade must preserve all P2 rows and migration checksum. Apply 0002 under one
SQLite migration transaction, validate all created metadata before version
catalog insertion, and roll back all 0002 DDL/catalog writes on failure. After
commit, downgrade/rollback of executable code is not promised unless a future
release policy defines it; do not delete event/schedule data to imitate a
rollback. Restart after migration resumes from committed event high-water,
schedule rows, occurrence mappings and leases. Expired lease means re-claim and
reconcile, never assume no task/effect. Migration does not turn historical
P2 journal rows into events.

### Required atomic transaction scopes

1. Existing task transition: P2 state + current journal + event row + seq
   update; failure at any participant rolls all back.
2. Schedule create/update/pause/resume/cancel: schedule row + lifecycle event.
3. Due occurrence: claimed/processed identity + task creation/mapping +
   `SCHEDULE_TASK_CREATED`/related events + schedule cursor/next due update.
4. Miss/catch-up defer: occurrence processed/skipped/queued markers + emitted
   missed/deferred events + retry `not_before`/cursor update.
5. Scheduler source event: source EventId dedupe/mapping and internal cursor
   advance, if separate cursor exists, in the same durable transition.
6. Event retention: bounded whole-record deletion and oldest-retained metadata
   update in a transaction; never partially prune sequence range metadata.

## 11. Classification and deletion

Event envelope class is the maximum class of its payload and all derived facts.
The Event Protocol examples use PERSONAL, and §6 explicitly allows PRIVATE
payloads only after redaction before device egress. For P3 storage, ordinary
rows currently support PUBLIC/PERSONAL; PRIVATE ordinary-row persistence is
fail-closed under P2's current limitations. Therefore P3 cannot currently
persist PRIVATE event content. It must omit/redact that content or refuse the
operation until actual protected-backend and ordinary-row support are separately
accepted and implemented; a `BlobRef` marker alone is not enough. P2's current
PRIVATE marker seam is not proof of encryption/backend delivery (ADR-0022 is
Proposed). The event migration cannot silently widen that guarantee.

Event/1 defines `payload` as an object and defines no `BlobRef`/payload-reference
wire field. P3 may not smuggle a storage reference into the public payload or
pretend the current `task_journal.payload_ref_digest` is an event reference. If
large or PRIVATE payload indirection is required, its durable representation,
rendering, classification, and wire compatibility require an explicit design
decision. Until then only payload data supported by existing ordinary-row
classification rules may be committed.

`SECRET` event/schedule payload is forbidden in this P3 scope because P2 has no
sealed store and ordinary storage rejects it. `CREDENTIAL` content is always
forbidden from event/schedule payloads and belongs only in OS credential
custody; even its presence as opaque JSON or a digest derived from a credential
is not authorized. IDs, non-sensitive reason codes and digests can appear only
when their inherited class is correctly determined and they do not disclose
secret-derived material.

Schedule templates may contain PERSONAL intent and arguments; classification
must inherit source data and be persisted. PRIVATE template content needs
protected representation or must be refused/redacted. Retention: schedule
definition/occurrence retention is governed by the selected schedule horizon;
deletion must remove protected payload references and occurrence mappings
consistently. Event references are immutable opaque historical IDs; event rows
have no task/step `ON DELETE CASCADE` or `SET NULL`. Their own event-content
retention and sequence-gap behavior follow Accepted ADR-0026 Option A.

## 12. Idempotency and concurrency inventory

| Operation | Identity/dedupe domain | Retry/crash rule | Fencing/concurrency |
|---|---|---|---|
| Event append | Stable source operation identity + event kind/ordinal; EventId for resulting event. Existing IDK-1 applies only if preimage/domain is identical. | On committed retry return/reuse existing event and sequence; on rollback no event/seq allocation. Exact source operation key not frozen. | SQLite writer transaction serializes sequence assignment; avoid independent in-memory counter. |
| Schedule create | Authenticated common-envelope `message_id` plus request digest; ScheduleId for the created object | Retry of same message ID returns durable result; same ID/different body refuses; a new message ID is a new command. | Unique ScheduleId; serialized Storage transaction. |
| Schedule update/reschedule | Envelope `message_id` plus request digest and expected revision/generation | Stale update refuses; retry returns committed result. | Revision compare-and-swap under immediate transaction. |
| Schedule cancel/pause/resume | Envelope `message_id` plus request digest and expected revision/generation | Repeating the same command returns its durable result; cancellation prevents future occurrences, not already-mapped tasks. | Transactional cancel/claim order; claim checks ACTIVE, revision, occurrence status and lease fence. |
| Due-item claim | ScheduleId + canonical occurrence key (or source EventId) | Expiry allows reclaim then mapping check; duplicate claim cannot create second occurrence. | Scheduler Protocol requires existing `max_lease_seconds` (120 s default); scheduler-specific owner/generation fence still needed. |
| Due-item execution/task creation | Unique occurrence key → TaskId mapping | Task+mapping atomic; replay returns same TaskId; never infer external effect from lease. | Scheduler claim + Storage transaction; TaskEngine owns task write. |
| Device timeline delivery | EventId/message_id; device client dedup | At-least-once only; replay after uncertain ack. | Device client cursor; no server generic subscriber state. |
| Scheduler cursor commit | Scheduler-specific singleton cursor plus source EventId/high-water | Handler/occurrence work and cursor advance are atomic or safely idempotent; never advance over unhandled work. | Sole internal durable consumer; SQLite compare-and-swap/fence. |

Concurrency decisions: two scheduler workers require occurrence or schedule
lease generation and stale-fence refusal; two Event Bus delivery workers can
share read-only ordered pages but cursor advancement needs a single owner/CAS if
server durable; recovery must share the same scheduler fence; replay/live must
use one global `seq` and stable high-water snapshot; task transition/event
append share the Storage transaction; cancellation is linearized as §5 says.
Do not reuse TaskStep `LeaseGuard` unless entity, owner, generation, interval,
and authority semantics exactly match. Reuse the proven `max_lease_seconds`
bound only as an explicit decision, not by type cargo-culting.

## 13. P3-specific bounds

| Resource/work dimension | Frozen value? | Gate |
|---|---|---|
| Due recurrence catch-up per schedule per wake | Yes: 10 | Enforce before task creation; defer remainder durably with retry event. |
| Concurrent scheduled tasks | Existing global `max_concurrent_tasks = 8` | Use ordinary Task Engine admission; no scheduler shadow counter. |
| Scheduler lease duration | Yes: existing `max_lease_seconds = 120` default is explicitly reused by Scheduler Protocol §4 | Enforce this bound; lease generation/owner semantics remain scheduler-specific, and UTC clock-jump behavior is called out in §5. |
| Active schedules | 256 | Global active rows; create/reactivation refuses at cap with bounded `BOUND_EXCEEDED` or typed error; zero disables active schedules. |
| Pending occurrences per schedule | 256 | Pending/claimed/due-unmapped; leave due identity and cursor for retry; never drop; zero refuses admission/claim. |
| Event payload / per-transaction events | 32768 bytes / 16 | Canonical UTF-8 payload bytes / all events in one outer transaction; refusal rolls back state+journal+event+seq; zero permits no payload/events. |
| Device replay / Scheduler scan page | 256 / 256 | Returned/read rows, not matches; preserve cursor at last returned/handled seq; zero disables page operation. |
| Recovery / retention-delete batch | 512 / 512 | Rows examined / complete records deleted; deterministic continuation; zero recovery work; retention batch zero is invalid when expiry is enabled. |
| Retained event content / event-store bytes | 1000000 / 536870912 | Capacity refusal after eligible pruning; typed error if still full; no silent drop. Minimal sequence/range metadata is separately bounded and compacted under ADR-0026 Option A. |
| Retry loop | Per-task attempts and per-wake catch-up exist; scheduler operation retry ceiling absent | Define retry policy/exhaustion for recurring storage/provider/transient scheduler failures. |

The Bounds Protocol defines exact row scopes, exhaustion/error visibility and
zero semantics. Slow device replay cannot block task execution: delivery is
downstream from durable commit. Retained content count/byte enforcement and
sequence/range metadata compaction follow Accepted ADR-0026 Option A.

## 14. Test-first implementation plan

No P3 runtime tests are added in this audit. The P3A closure validates docs and
the version-registry alignment; P3B onward follows the RED-first implementation
sequence under the now-accepted ADR-0026 contract.

| Phase | Scope | Expected production files (future only) | Test groups | Migration | Closure gate | Execution |
|---|---|---|---|---|---|---|
| P3A — Contract and transaction gate | Close accepted retention/sequence semantics and version registry; other owner directions incorporated. | Docs/protocol/ADR and exact device surface registry update only | Cross-doc inventory, protocol version registry checks, operation→event matrix; no runtime RED tests yet. | None | ADR-0026 Accepted; architecture/device versions and replay semantics consistent; prescribed validation and paired platform CI GREEN. | CLOUD |
| P3B — Migration and durable event log | 0002 event metadata/log, event schema mapping, sequence allocation, append-only constraints, retention metadata/query primitives. | `crates/serea-storage/{migrations,src}`; `crates/serea-event-bus` | 0001 immutable; upgrade/rollback; seq gaplessness/rollback; append-only; schema, class and query ordering. | Yes, migration 0002 | Crash-safe storage primitives with no P2 runtime changes or backfill. | CLOUD; GITHUB_ACTIONS |
| P3C — Atomic TaskEngine event participation | Wire existing task transition operation facts to EventBus while preserving TaskJournal and savepoint guarantees. | `crates/serea-storage/src/{store,tx}`; `crates/serea-task-engine/src`; `crates/serea-event-bus/src` | Event+state+journal all-or-nothing; no-op/refusal; COMMIT caller-loss retry; no historical backfill. | No new migration beyond P3B | E3/E4 forward proof over enumerated task transitions; no one-participant API escape. | CLOUD; GITHUB_ACTIONS |
| P3D — Event query/replay and delivery | Ordered history pages, retention expiry/corruption responses, internal scheduler source API, device timeline at-least-once boundary as authorized. | `crates/serea-event-bus/src`; later Core/device integration only if included | cursor/replay/live races, duplicate delivery, unknown kinds, slow consumer isolation, retention. | Maybe cursor table only if decision selects it | No loss across replay/retention; resource caps selected. | CLOUD; GITHUB_ACTIONS |
| P3E — Durable Scheduler storage and claims | Schedule/occurrence rows, unique identities, claim fence, state mutations, cancellation linearization, catch-up cursor. | `crates/serea-storage/migrations/0002*` (if same release integration), `crates/serea-storage/src`, `crates/serea-scheduler` | Two claims, stale generation, due boundary, cancel race, duplicate operation, cap=10, rollback. | Same 0002 if schema integrated; avoid 0003 without need | All schedule state transitions atomic, no task ownership in Storage. | CLOUD; GITHUB_ACTIONS |
| P3F — Scheduler runtime and recurrence | Calendar and HOST_EVENT triggers; due handling; task create/resume via TaskEngine; recovery and bounded missed semantics. | `crates/serea-scheduler/src`; protocol-time support only if selected | forward/backward/large clock jumps, DST gap/fold, tzdata change, duplicate source event, RUN_* policies, approval/device resume, restart. | No schema beyond finalized 0002 | Deterministic one occurrence→one task mapping and no policy/approval bypass. | CLOUD; GITHUB_ACTIONS |
| P3G — Integrated crash/concurrency/closure | Cross-crate transaction crash proof, startup order, migration upgrade, public claims/docs, final independent sequential audit. | Runtime hardening/docs only | All named crash windows, two Stores/workers, recovery twice, failpoints, resource bounds, public CI. | No new speculative schema | All owner decisions accepted; RED-first groups green; stable+MSRV and platform CI pass; no P2 nonclaim widened. | CLOUD; GITHUB_ACTIONS |

Migration sequencing note: P3B and P3E share migration 0002 if schema is
co-designed before either lands. Do not create a second version merely to match
subphase labels. If P3C tests reveal another necessary durable object, amend
the Proposed schema and owner-review it before SQL.

## 15. Execution location and CI

| Work | Location |
|---|---|
| Audit, protocol reconciliation, docs, Rust implementation, focused Rust tests, workspace checks | **CLOUD** |
| Clean Linux stable/MSRV 1.85, macOS Intel, macOS arm64, crash/recovery matrix | **GITHUB_ACTIONS** |
| Actual host/device/credential/provider integration | **LOCAL_REQUIRED only if a later phase demonstrates necessity**; none is required or planned by this audit. |

The supplied public baseline is Fast CI PASS `37477567529`, Full CI PASS
`37477783465`, Linux stable PASS, Linux MSRV 1.85.0 PASS, macOS Intel x86_64
PASS, GitHub-hosted macOS arm64 PASS, release fault-seam proof PASS. These are
baseline facts supplied for this audit; this docs-only branch does not rerun the
full Rust matrix. Future clean cross-platform checks belong to GitHub Actions,
not the 8 GiB Intel Mac.

P3 requires no Gmail/Calendar OAuth, Android secrets, Local MCP credentials,
user-private database, external provider, or Actions secret. Local Mac use and
Local MCP are not required.

## 16. Owner-direction closure and remaining choice

The following owner directions are incorporated in the normative source
documents and this audit. The older analysis in §§3–15 is retained as audit
lineage; where it calls these items unresolved, this section supersedes it.

| Direction | Closure record |
|---|---|
| Fixed transaction composition | Accepted ADR-0025: exactly the existing TaskAuditParticipant and new EventParticipant; same Storage transaction/savepoint and immutable successful-write facts. Task Engine owns journal semantics, Event Bus owns event semantics, Storage owns transaction and facts. Failures roll back state/journal/event/seq; no-op/refusal writes neither unless a frozen refusal event applies. No registry, Store re-entry, or Storage dependency upward. |
| Scheduler §1/§6 | One transaction and one COMMIT. Schedule/state mutation SQL precedes event append/seq allocation inside the same uncommitted transaction. Never two transactions. |
| Subscriber scope and startup | No external generic subscribers. Device timeline is replay surface. Scheduler is sole durable internal consumer with a Scheduler-specific singleton cursor/state row. In-memory signal is wake-only. Startup is Store open/migrate → validate event metadata → TaskEngine recovery → Scheduler occurrence reconciliation → Scheduler replay → live. Replay snapshots committed high-water seq and stops at it; batch-generated events wait for next pass. |
| Event references and task deletion | TaskId/StepId and related event identifiers are immutable opaque historical values. Event rows have no `ON DELETE CASCADE` or `SET NULL` relation to task/step rows. Task deletion does not mutate retained events. |
| ADR-0017 | Delete statements, provenance/blob deletion, tombstones, completion-event insert, then one COMMIT. Event insert is after successful deletion work but before that transaction commits. |
| Recurrence evaluator | Serea owns recurrence grammar/policy. Jiff `=0.2.38` owns timezone/DST conversion only; require `default-features=false`, `std` + `tzdb-bundle-always` (no host zoneinfo/concatenated features), bundled `jiff-tzdb =0.1.9` carrying IANA TZDB `2026e`. Both crates: `Unlicense OR MIT`, declared MSRV 1.70, compatible with 1.85. Persist intended local label, timezone, resolved UTC instant, evaluator/TZDB version; TZDB updates affect unresolved future occurrences only. Metadata/source checked 2026-10-06. |
| Operational bounds | Bounds Protocol §2 now records all ten new requested values with exact scopes, exhaustion/durable visibility/error behavior and zero semantics. Existing catch-up 10, concurrent tasks 8, lease 120 are unchanged. No silent truncation/drop. |
| Cancel/claim and command retry | Claim revalidates ACTIVE state, expected revision/generation, unmapped/unprocessed occurrence, and current lease fence transactionally. Cancel-first blocks later claim; claim-first task may continue and is not silently cancelled. Authenticated common envelope `message_id` is the durable command-dedupe key, bound to request digest/outcome; different body under same ID refuses. IDK-1 is not reused because its preimage semantics differ. |
| Retention and global seq | **Owner-selected and accepted.** ADR-0026 Option A separates minimal sequence existence/availability metadata from independently expirable complete content; replay declares exact interior expiry ranges; compacted-prefix history expiry and unexplained corruption are separate typed outcomes. Architecture is `serea-arch/2.0.0`, replay surface is `serea.device/2`, event objects remain `serea.event/1`; B and C are rejected. |

### P3A gate

Event Protocol E2/E4, §6, and §8 are reconciled by Accepted ADR-0026 Option A.
The remaining P3A gate is repository consistency plus the specified docs,
smoke, metadata, formatting, and GitHub Actions validation. P3B may start only
after those checks pass on the exact P3A closure commit.

## 17. Final audit status

- P3 implementation started: **NO**
- Migration 0002 created: **NO**
- New runtime crate created: **NO**
- P2 runtime behavior changed: **NO**
- Local Mac used: **NO**
- Local Mac required for P3: **NO**
- Readiness: **P3A_CLOSURE_IN_PROGRESS** — no owner semantic decision remains; validation and commit/CI gates are pending

Owner authorization selects Option A and explicitly authorizes P3 implementation
after P3A's repository and CI gates. This audit records the current contract;
runtime and migration work remains unstarted until that gate is GREEN.
