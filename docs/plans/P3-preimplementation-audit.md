# Serea P3 Preimplementation Audit

- **Status:** `BLOCKED_PENDING_OWNER_DECISION`
- **Audit base:** `main` at `cb580be8dd0057202c6b0f9c783391460bfc1875`
- **Audit branch:** `p3/preimplementation-audit`
- **Scope:** documentation and design only. No P3 runtime, schema migration, dependency, or behavior change.
- **Authority reviewed:** repository contracts at the audit base; public branch and CI identities supplied with the task.

## 1. Executive disposition

P3's intended product semantics are substantially specified in frozen Event and
Scheduler Protocols, the Crate Map, and Accepted ADR-0008. P3 is an event log
with a durable ordered timeline and replay, plus a durable schedule/occurrence
trigger that admits or resumes ordinary tasks. Neither is a generic message
broker nor a second task engine.

The implementation gate is **not closed**. The architecture cannot honestly be
called ready until the owner resolves the transaction-boundary conflicts in
§4, reconciles the accepted ADR-0017 wording recorded in §4, and
ratifies or rejects the proposed ADR-0025. There is also a
specific unresolved capability boundary for event subscribers and history
replay (§5). The frozen scheduler protocol explicitly includes local calendar
recurrence and IANA/DST behavior; this audit does not remove or silently narrow
that accepted contract. Its implementation dependency and timezone-data source
must be selected before recurrence implementation. No migration 0002 is created
by this audit.

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
| PROPOSED_ADR | `docs/decisions/ADR-0021-p2-p3-event-atomicity-seam.md` | E3 is forward-only; no retroactive P2 event fabrication; P2 journal is distinct; Event Bus appends events and allocates seq in the same Storage transaction using actual-write facts. P2-side seam is implemented, but the ADR is still Proposed and its P3 participant shape is not ratified. | Storage transaction; TaskEngine journal semantics; Event Bus event semantics | Must compose journal and event write within one transaction without making P2 journal an event log. Current `Store::transact_with_audit` accepts one audit participant. | State+journal+event+sequence success/rollback tests; no-commit publication; P2 rows remain unchanged. |
| ACCEPTED_ADR + CONTRADICTORY WORDING | `docs/decisions/ADR-0017-deletion-cascade-completed-event-kind.md` Context/Decision; Data Protocol §8.2; Event Protocol E3 | ADR-0017 describes the completion event as emitted “after the transaction … has committed”; Data Protocol §8.2 step 4 records the event within the single cascade operation, while E3 requires event and described state change in one transaction. | Deletion owner + Storage + Event Bus | Literal post-commit append violates E3; interpret “after” as after delete/tombstone statements but before commit, or amend the accepted ADR wording. Do not implement post-commit. | Cascade fault points prove delete/tombstone/event all commit or all roll back; explicit event absent on failed cascade. |
| HISTORICAL | `docs/plans/P2-closure.md` §§Workspace, nonclaims, next phase | P2 is closed at 0001 with no event bus/scheduler, no E3/E4, no backfill. `pending_event_transitions` is a committed journal-row count. | Storage + Task Engine | Migration 0001 is frozen; preserve report meaning; P3 event guarantee begins only for new transitions after its atomic participant is live. | Migration checksum unchanged; upgrade fixture proves existing count and journal unchanged. |
| HISTORICAL | `docs/plans/P2-test-matrix.md` §§E, F, H, M, O | P2 tests use explicit `EpochMillis`, injected `Clock`/`TestClock`, and no ambient system clock; P2 crash evidence is bounded process/fault evidence, not power-loss certification. | Protocol/Storage/TaskEngine | P3 deterministic core receives explicit time; hosted CI evidence remains environment-scoped. | P3 deterministic time injection; named process-crash tests, no broader durability claim. |
| NONCLAIM | `README.md` Current status/Evidence; P2 Closure §§ADR status/nonclaims | P3 runtime not implemented; no event bus, scheduler, migration 0002, event delivery, event backfill, provider runtime, or power-loss guarantee is claimed. | Project | Audit documentation cannot represent these as shipped. | Docs validation and repository/source inventory. |
| FROZEN | `docs/protocols/09-data-classification-protocol.md` §§2–3,6,8; `docs/decisions/ADR-0010-*` | Classification inherits maximum input class; CREDENTIAL only credential store, SECRET sealed store only, PRIVATE encrypted at rest and controlled cloud egress; unclassified defaults to CREDENTIAL. | Protocol + Storage + each egress owner | Event/schedule content requires declared inherited class, protected content reference policy, retention/deletion handling; P2 ordinary rows remain PRIVATE fail-closed. | Per-class storage refusal/protection/redaction tests; class cannot be lowered through event or schedule derivation. |
| FROZEN | `docs/protocols/10-bounds-protocol.md` §§2,2.4 | Catch-up limit 10 is frozen. Event/schedule counts, payload bytes, subscribers, replay batches, recovery scans, and storage size lack operational bounds; structural schema caps do not substitute. | Core owns bound config; enforcing crate owns each check | No invented numeric values; P3 implementation cannot claim resource boundedness until required workload bounds are chosen. | Boundary tests once owner chooses required bound values; pre-mutation refusal/no partial write. |
| AMBIGUOUS | Event Protocol §§5–8; Crate Map §3 | Device timeline replay is defined, but generic subscriber identity/durability, internal live notification mechanism, event-predicate scan cursor, subscriber deletion, and whether external consumers are admitted are not. | Event Bus; Scheduler as one internal consumer; Core device link | Choose only the device timeline plus internal scheduler consumer for P3 unless owner expands scope; durable event history is authoritative, but cursor persistence location remains open. | Replay/live race, subscriber restart, cursor commit crash, retention and deletion tests after scope decision. |

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
`EventId`; where its cursor lives and how it atomically advances with occurrence
work are unresolved.

No events are silently discarded except whole-record retention at their
configured horizon. Event Protocol defines retention classes but not a physical
deletion procedure, foreign-key policy, tombstone requirements, or coordination
with task deletion. Deletion/cascade must not rewrite events or allow a dangling
event reference to be mistaken for a retained task. These are migration and
retention design gates.

Host parsers reject unknown `EventKind`; forward-compatible clients skip it and
continue. Payloads are structured validated event facts, not arbitrary model
output. Maximum bytes, retained count, query/replay batch, simultaneous
consumers, and recovery scan bounds remain unselected.

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
- **Safest interpretation:** treat §1's “before” as draft ordering within one
  uncommitted transaction, with the event row written before the transaction
  commits; a crash rolls both back. This matches E3 and §6, but changes the
  literal interpretation of §1 and needs owner ratification.

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
- **Safest interpretation:** no outbox, no post-commit publication, no
  backfill, and no new generic registry. Prefer an explicit, narrowly scoped
  composition of the existing journal mapping and an event-specific storage
  capability around actual-write facts in the same transaction/savepoint.
  Precisely where that composition lives, how it preserves P2 savepoint
  failure guarantees, and whether `Tx` is extended are owner decisions.

### Proposed ADR-0025 (not accepted)

**Title:** P3 Event Participant Composition and Atomic Publication

**Status:** Proposed; owner decision required.

Proposed constraints: preserve forward-only E3; one SQLite transaction commits
state, TaskJournal and `SereaEvent` rows; allocate `seq` only there; no
post-commit/outbox path; no synthetic P2 events; no generic mutable hook
registry; no Storage→TaskEngine/EventBus runtime edge. Preserve failure rollback
and no-op behavior of current P2 operations. The ADR must choose the explicit
composition/API and enumerate which existing operations are event-producing.
This audit does not choose a public signature or mark the proposal accepted.

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
- **Safest interpretation:** “after” means after the delete/tombstone SQL
  operations within the still-open transaction, before COMMIT. Because ADR-0017
  is Accepted, amend its wording only through the project decision process;
  never silently use a second transaction.

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
derive them. Dependency/data-source selection is open; no dependency is added
here.

For one-time due triggers, due means `now >= due_at`. Overdue items are processed
according to their persisted missed policy and catch-up bound. Clock moving
forward can make multiple occurrences overdue and invokes that same bounded
policy. Clock moving backward does not undo committed occurrence mappings or
unprocess work; no due item becomes “un-due” after its claim. Duplicate wakeups
resolve through the durable occurrence key. A large jump is a bounded catch-up,
never an unbounded task burst. DST/timezone behavior follows Scheduler Protocol
§5. Exact behavior for timezone database changes is frozen for pending versus
processed occurrences; evaluator version/data retention remains unresolved.

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
tasks keep their frozen template/policy. The protocol is underspecified for a
cancel race with an already claimed but not yet mapped occurrence; safest
behavior is revalidate inside the mapping transaction so cancel-before-commit
prevents task creation and commit-before-cancel is an already-created task.
Owner should confirm this linearization point.

## 6. Transaction windows and resolution

| Crash/race window | Must be impossible, repairable, or out of P3 | Transaction/idempotency/recovery rule | Future evidence |
|---|---|---|---|
| Task/schedule state changed, event absent | Impossible for P3 transitions covered by E3 | Same `BEGIN IMMEDIATE` writes state, journal if applicable, event, and seq. A failed event mapper/write aborts the whole operation. | Fault injection after state write and after event insert; fresh-process verify neither row survives. |
| Event durable, described state unchanged | Impossible | Same transaction and rollback boundary as above. | Fault after event insert/before commit leaves no event or source-state write. |
| Schedule occurrence consumed, task not advanced/created | Must not strand a terminally consumed occurrence; committed processing marker may be repairable | Occurrence key and mapping are unique; mapping and task creation are one transaction when creating. Lease expiry reclaims and reads mapping. | Kill at each write; restart yields one mapping/task or no mapping/task. |
| Task advanced/created, schedule occurrence still active/unmapped | Impossible for schedule-created task | In one transaction write occurrence mapping, task creation/state, associated events and relevant cursor. | Triggered rollback tests with a failure at each participant; no orphan task. |
| Event cursor advanced, handler work not committed | Must not occur for durable Scheduler consumption | Cursor/occurrence work commits with task mapping, or cursor is not advanced until idempotent work commits; source EventId unique key prevents duplicate task. | Crash before/after cursor write and handler write; restart has neither skipped event nor duplicate task. |
| Handler work committed, cursor not advanced | Repairable only when work is idempotent | On replay, unique source EventId/occurrence mapping returns prior task, then advances cursor in same transaction. | Replay same source event after crash; exactly one mapping and task. |
| Delivery happened, acknowledgement/cursor missing | Allowed at-least-once delivery | Stable EventId/message_id dedup at consumer; no exactly-once claim. Device timeline client owns its last-seen cursor unless owner chooses durable server cursor. | Deliver then crash before ack; duplicate is harmless and ordered. |
| Schedule cancel races due claim | Linearize under Storage transaction/fence | Revalidate ACTIVE state while holding fenced occurrence transaction; cancel-before-claim commit prevents new task; claim/mapping commit first means existing task is retained. | Two-connection interleaving tests in both commit orders, including stale lease. |
| Event publish call retries after caller lost commit result | Must be idempotent by source operation identity | Deterministic semantic operation identity/dedupe key is required; EventId and sequence alone do not dedupe a reissued business operation. Exact P3 mapping to existing IDK-1 is unresolved. | Child killed after COMMIT before return; retry yields one event and one seq allocation. |

P2's existing `IDK-1` is defined for task/action idempotency preimages, not
automatically for schedule operations or subscriber cursors. Reuse it only when
the semantic preimage and domain match. Scheduler operation identity is
ScheduleId+canonical occurrence/source EventId; lifecycle create/update/cancel
needs an authenticated request identity or explicit version/command identity,
not an invented second hash scheme.

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

1. Open Store and apply migration 0002 transactionally.
2. Validate event sequence metadata/log invariants.
3. Run `TaskEngine::recover` once with explicit `EpochMillis` and publish its
   report only after its existing transaction commits.
4. Run Scheduler reconciliation with explicit time and event high-water/cursors;
   acquire/reclaim occurrence leases, reuse mapped task IDs, and process bounded
   due work.
5. Start transient notification and device delivery workers; they may replay
   committed events and cannot become authorities.

The precise interaction order between task recovery events and Scheduler's
replay boundary must ensure recovery events are not skipped. The safest initial
cursor is the last durably committed consumer cursor (or a full bounded scan
from retained history); cursor persistence and this startup fence are unresolved.
No independent Scheduler authority may race a P2 recovery pass on task rows.

## 8. Ownership / authority matrix

| Responsibility | Task Engine | Storage | Event Bus | Scheduler |
|---|---|---|---|---|
| Task state / state transitions | **Authoritative owner** | Atomic persistence/integrity mechanism | No ownership; emits resulting event facts | May request create/resume; cannot mutate directly |
| Schedule definitions/state | No ownership | Durable row/constraint authority only | No ownership | **Authoritative owner** |
| Occurrence and due processing state | No ownership | Durable transaction/unique/fence mechanism | Source event history only | **Authoritative semantic owner** |
| Event facts, sequence, history, retention | Supplies task transition facts | Atomic durable rows and seq counter; does not define EventKind | **Authoritative semantic owner** | Supplies schedule/occurrence event facts |
| Delivery cursor | No ownership | Cursor storage if selected | **Protocol owner**, persistence choice open | Owns its source EventId/catch-up cursor if selected |
| Task step retry / provider idempotency | **Authoritative owner**, existing IDK/receipt/recovery | Persists fences/receipts atomically | No ownership | No ownership |
| Schedule retry/catch-up | No ownership | Persists due/claim/fence/cursor | No ownership | **Authoritative owner**, bounded by frozen catch-up=10 |
| Clock access | Receives explicit EpochMillis; no ambient time | Persists validated instants; no hidden `now()` | Event timestamps passed in facts; no ambient time | Injects/receives Clock at runtime boundary; passes EpochMillis into core |
| Task recovery | **Sole task recovery authority** | Executes P2 storage classifications atomically | Validates/replays event history; no task repair | Reconciles schedules and occurrence mappings; calls Task Engine |
| Schedule recovery | No ownership | Fences/transactions | Supplies event history/cursor | **Sole schedule recovery authority** |
| Transaction boundary | Calls storage operation/participant API; not independent commit authority | **Sole SQLite transaction authority** | Supplies event participant/write facts within Storage Tx | Supplies schedule participant/write facts within Storage Tx |
| External side effects | Existing Task Engine→Capability path only | None | None | None; scheduled work follows normal Task Engine path |

No ownership is duplicated; the outstanding participant API proposal must honor
this matrix.

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
| `serea_events` | wire `message_id`/EventId primary key; `seq INTEGER UNIQUE NOT NULL`; `kind`; `occurred_at_ms`; `correlation_id`; `causation_id`; actor kind/id/version; `data_class_rank`; trace task/step/attempt fields; validated `payload_json`; retention class/expiry metadata; constraints and query indexes | Durable append-only Event Protocol log, sequence order, class retention, replay and event predicates. | Exact SQL names/types, payload byte cap, FK behavior for task/step deletion, per-class expiry field, unknown-kind persistence compatibility, and index set must be finalized. Frozen event/1 has no `BlobRef` member; adding an internal payload reference/render indirection needs an explicit design and must not silently widen the wire surface. |
| `schedules` | `schedule_id PK`; owner identity; state; trigger type and canonical validated recurrence/event predicate; template reference or protected payload ref; `policy_class`; approval policy; timezone; created/updated UTC milliseconds; `next_due_at_ms`; local occurrence identity/version; missed policy; last processed marker; schedule generation/version; schedule lease generation/owner/expiry if lease is held here | Durable schedule definition/state and due instant required by Scheduler Protocol. | Store PERSONAL/PRIVATE fields policy, schedule/template bounds, exact recurrence serialization, index for active next_due. |
| `schedule_occurrences` | schedule ID + canonical occurrence/source key unique; due UTC ms; source event/task identity as relevant; state (pending/claimed/mapped/skipped); `not_before_ms`; lease owner/generation/expiry; mapped task ID; processing timestamps/reason | Dedup, bounded catch-up cursor, crash reconciliation and one occurrence→one task mapping. | Whether scheduler lease belongs per-schedule or per-occurrence, canonical key encoding, cancellation race version and resumable-vs-terminal occurrence states. |

No independent `event_outbox` or synthetic backfill table. No separate generic
subscriber table is included until durable external subscribers are authorized.
If Scheduler needs an independent durable cursor, it may be represented by a
P3 consumer cursor row keyed to the one internal Scheduler identity, or by
unique occurrence source EventIds; owner must select after replay semantics are
closed. Avoid a cursor table if the occurrence uniqueness record fully prevents
skips and duplicates.

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
definition/occurrence retention is unspecified beyond Event Protocol's
schedule-retention class; deletion must remove protected payload references and
all occurrence mappings consistently. Event Protocol says retained events are
append-only and expires whole records; the FK/cascade and whether task deletion
deletes or redacts old event rows remain unresolved. Safest interim rule: don't
FK event rows with `ON DELETE CASCADE` to task rows, and never rewrite a retained
event to remove a foreign key. Owner must decide tombstone/reference behavior
before schema freeze.

## 12. Idempotency and concurrency inventory

| Operation | Identity/dedupe domain | Retry/crash rule | Fencing/concurrency |
|---|---|---|---|
| Event append | Stable source operation identity + event kind/ordinal; EventId for resulting event. Existing IDK-1 applies only if preimage/domain is identical. | On committed retry return/reuse existing event and sequence; on rollback no event/seq allocation. Exact source operation key not frozen. | SQLite writer transaction serializes sequence assignment; avoid independent in-memory counter. |
| Schedule create | ScheduleId plus authenticated create command/request identity | Retry of same command returns same schedule; ID reuse forbidden; whether create request identity is durable is open. | Unique ScheduleId; serialization in Storage. |
| Schedule update/reschedule | ScheduleId plus expected version/generation and command identity | Stale update refuses; retry same committed version is idempotent. | Version compare-and-swap under immediate transaction. |
| Schedule cancel/pause/resume | ScheduleId plus desired state/command version | Repeating same state is durable no-op/no duplicate event; cancellation is not task cancellation. | Schedule generation plus transaction linearization vs claim. |
| Due-item claim | ScheduleId + canonical occurrence key (or source EventId) | Expiry allows reclaim then mapping check; duplicate claim cannot create second occurrence. | Scheduler Protocol requires existing `max_lease_seconds` (120 s default); scheduler-specific owner/generation fence still needed. |
| Due-item execution/task creation | Unique occurrence key → TaskId mapping | Task+mapping atomic; replay returns same TaskId; never infer external effect from lease. | Scheduler claim + Storage transaction; TaskEngine owns task write. |
| Subscriber delivery | EventId/message_id; device client dedup | At-least-once only; replay after uncertain ack. | Global sequence order; no durable generic subscriber model specified. |
| Scheduler cursor commit | Source EventId/high-water plus consumer identity if a cursor row is adopted | Handler+cursor atomic, or handler idempotent before cursor advances; retain source identity to repair. | One Scheduler consumer lease or SQLite compare-and-swap. |

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
| Outstanding active schedules | No | Owner must choose before accepting unbounded schedule creation or explicitly define a structural operational policy. |
| Pending occurrence queue / missed history | No | Choose retention and backlog cap or accept potentially unbounded durable growth. |
| Events per transaction / payload bytes | No | Choose write and storage limits consistent with event payload schemas. |
| Subscribers / replay page / per-consumer backlog | No | Decide P3 subscriber scope, page cap and slow-consumer policy. |
| Event/task/schedule recovery scan | No | Specify deterministic indexed bounded scans and continuation cursor. |
| Retained event count/bytes | No | Retention durations exist; physical volume bound and pruning batch are absent. |
| Retry loop | Per-task attempts and per-wake catch-up exist; scheduler operation retry ceiling absent | Define retry policy/exhaustion for recurring storage/provider/transient scheduler failures. |

No arbitrary numeric values are proposed. Slow consumer cannot block task
execution by architecture: delivery is downstream from durable commit; queue or
network pressure may defer delivery but cannot hold the task transaction open.
Exact queue cap and backpressure behavior are open.

## 14. Test-first implementation plan

No tests are added in this audit. Proposed RED-first sequence follows dependency
and transaction prerequisites; phases may be split further only after ADR-0025
and the subscriber/cursor decision close.

| Phase | Scope | Expected production files (future only) | Test groups | Migration | Closure gate | Execution |
|---|---|---|---|---|---|---|
| P3A — Contract and transaction gate | Resolve contradictions, event participant composition, subscriber scope, data/retention rules, recurrence evaluator source, bounds; accept needed ADRs. | Docs/protocol/ADR only | Cross-doc inventory, compile/API contract sketches, operation→event matrix; no runtime RED tests yet. | None | Owner decisions recorded; exact P3 operations and event semantics unambiguous. | CLOUD |
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

## 16. Owner decisions required

1. Ratify or reject Proposed ADR-0025's event/journal transaction participant
   composition; name the exact narrow API and event-producing task transition
   inventory while preserving acyclic edges and P2 failure guarantees.
2. Resolve Scheduler Protocol §1 vs §6: confirm schedule state plus event are
   one transaction and interpret §1's “persist before appended” as within that
   uncommitted transaction.
3. Define Event Bus live wake/replay boundary and the sole Scheduler consumer's
   cursor ownership/persistence, including replay-vs-live and startup after
   TaskEngine recovery. Confirm whether any external consumers are in P3
   (safest scope: none).
4. Resolve task deletion/retention versus append-only event references: event
   FK/cascade/tombstone semantics and whether task-linked payload is redacted,
   expired, or retained without dangling references.
5. Reconcile Accepted ADR-0017's “after transaction has committed” language
   with Event Protocol E3 and Data Classification §8.2; confirm the completion
   event is inserted after deletion statements but before the same COMMIT, or
   authorize a reviewed ADR amendment.
6. Confirm recurrence remains required in P3 and choose a reproducible IANA
   timezone rule source/evaluator/version policy; EpochMillis alone cannot
   implement the frozen local calendar rules. No dependency selected here.
7. Select missing operational bounds for outstanding schedules, occurrence
   backlog/retention, event payload/transaction volume, replay batches,
   subscribers and recovery scans; or explicitly stage/limit their use before
   implementation.
8. Confirm schedule cancel-vs-claim linearization and how authenticated create,
   update, pause/resume and cancellation commands deduplicate when caller loses
   the commit response.

## 17. Final audit status

- P3 implementation started: **NO**
- Migration 0002 created: **NO**
- New runtime crate created: **NO**
- P2 runtime behavior changed: **NO**
- Local Mac used: **NO**
- Local Mac required for P3: **NO**
- Readiness: **BLOCKED_PENDING_OWNER_DECISION**

The safest implementable direction is documented, but the owner decisions above
are required before the implementation prompt can be deterministic. This audit
does not fabricate closure or authorize P3 implementation.
