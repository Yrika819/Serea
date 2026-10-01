# Scheduler Protocol

Protocol ID: `PROTO-SCHED` · Surface: `serea.scheduler/1` · Status: **FROZEN for P0** · Implementation: **Deferred to P3**

This protocol defines durable schedules and event-driven wakeups that may create
or resume Serea tasks. The scheduler is a trigger and persistence subsystem, not
an authority source: every resulting action still passes through task, capability,
policy, approval, bounds, and provider contracts. No scheduler implementation or
external service integration is authorized in P0.

---

## 1. Schedule record and authority

A schedule is host-owned durable state. Its `schedule_id` is a `ScheduleId`
([Protocol Index §2](00-protocol-index.md#2-identifier-grammar)); its record
contains at least:

| Field | Contract |
| --- | --- |
| `schedule_id` | Stable `ScheduleId`, never reused. |
| `owner_device_id` | The paired device/user identity that created the schedule, or the host admin identity for a host-created schedule. |
| `state` | `ACTIVE`, `PAUSED`, or `CANCELLED`. |
| `trigger` | A validated event predicate or calendar recurrence, exactly one trigger form per schedule. |
| `task_template` | Host-validated task intent and permitted arguments; a schedule does not persist model-authored authority. |
| `policy_class` | Fixed ceiling for spawned work; a scheduled task cannot raise it. |
| `approval_policy` | The ordinary policy/approval requirements for each action. No approval is pre-granted by a schedule. |
| `timezone` | IANA time-zone identifier for calendar recurrences; required when the recurrence is local-time based. |
| `created_at`, `updated_at` | Host timestamps persisted with each record change. |
| `next_due_at` | Next calculated due instant, persisted in UTC together with the recurrence's local-time interpretation. |
| `missed_occurrence_policy` | One of `SKIP`, `RUN_ONCE`, `RUN_EACH`; validated with the schedule and constrained as below. |
| `last_processed_occurrence` | Last occurrence key durably handled, or `null` before the first firing. |

Creation, change, pause, resume, and cancellation require an authenticated user
or host administrator action. Model output may propose a schedule but cannot
create or mutate durable schedule state. Every mutation is persisted before its
corresponding scheduler event is appended.

A schedule's policy ceiling and task template are immutable for already-created
tasks. Editing a schedule affects future occurrences only. Raising its policy
ceiling or widening its permitted action scope requires explicit human approval
and explicit schedule re-authorization; lowering or pausing takes effect
immediately. No schedule grants standing capability approval.

## 2. Event-driven wake types

The scheduler wakes from durable events, not polling loops or model self-wakes.
Supported wake types are:

| Wake type | Source | Behaviour |
| --- | --- | --- |
| `CALENDAR_DUE` | Persisted calendar recurrence reaches its calculated due instant | Process the due occurrence according to its missed-occurrence policy. |
| `HOST_EVENT` | A committed Serea event matches the schedule's validated event predicate | Process once for that source event; duplicate delivery is deduplicated. |
| `DEVICE_SESSION_ESTABLISHED` | `DEVICE_CONNECTED` is committed for a paired device after a disconnect | Resume the eligible existing task; do not create a second task for the same occurrence. |
| `APPROVAL_EVENT` | `APPROVAL_GRANTED`, `APPROVAL_DENIED`, or `APPROVAL_EXPIRED` is committed for the waiting task | Resume or terminate the already existing task according to Approval and Task Protocol; never mint another occurrence task. |
| `CORE_RECOVERY` | Core starts or recovers durable state | Recalculate due state and reconcile interrupted occurrence processing without duplicating a task or effect. |
| `RETRY_DUE` | A bounded catch-up batch leaves due occurrences queued, or a retryable scheduler/provider operation reaches its durable retry time | Carry `ScheduleId`, due occurrence identity, and `not_before`; deduplicate by that tuple and process at most the remaining per-wake bound. Persist the next wake atomically with the deferred cursor.

Each wake carries its source identity from durable state (the source `EventId`,
`ScheduleId` and due instant, or existing `TaskId` as applicable). A wake is only
a request to evaluate the schedule/task; it does not bypass policy or grant
approval.

## 3. Persistence and recovery

Schedule definitions, state, next due instant, timezone, occurrence-processing
records, and the mapping from an occurrence to its spawned `TaskId` are stored in
Serea's durable store in one transaction per state transition. The unique
occurrence key is the existing `ScheduleId` plus its canonical occurrence
identity: for calendar recurrence, the intended local date/time and timezone;
for `HOST_EVENT`, the source `EventId`; for resume/recovery wakeups, the existing
`TaskId`. This is a storage uniqueness rule, not a new identifier format.

Before dispatch, the scheduler durably records that an occurrence is being
processed. Creating a scheduled task and recording the occurrence-to-task
mapping are atomic. Repeated delivery or recovery therefore returns the existing
`TaskId` rather than creating another task. The spawned task uses `kind:
SCHEDULED`, `requested_by: SCHEDULER`, and its own normal `TaskId`, `StepId`, and
`IdempotencyKey` contracts.

On Core recovery:

1. Load active schedules and uncompleted occurrence records.
2. Recalculate due instants using the stored timezone and recurrence rule.
3. Reconcile any occurrence with a committed task mapping by reusing that task.
4. Process due, unmapped occurrences once according to the missed-occurrence
   policy and atomically record each mapping.
5. Acquire the scheduler lease before mutating occurrence state or dispatching
   work; an expired lease is reclaimed through the same unique occurrence key.

Schedule durability does not make an external effect exactly-once. Task recovery
and provider idempotency/reconciliation govern effect safety after task creation.
An ambiguous provider outcome blocks or reconciles under the Capability and Task
Protocols; the scheduler never starts a replacement task to guess whether an
effect happened.

## 4. Lease and concurrency

Only one scheduler lease holder may process a schedule occurrence at a time.
The lease has a host-configured bounded duration and a monotonically changing
lease owner token stored with the occurrence record. Lease expiry permits
reclamation, not assumption that work did not happen. A new holder reads the
occurrence/task mapping and resumes that task or processes the still-unmapped
occurrence atomically.

The scheduler lease uses the existing `max_lease_seconds` bound (120 seconds
by default; [Bounds Protocol §2](10-bounds-protocol.md#2-the-bound-set)). Scheduler
locks are not held while waiting for a human approval or a device. The scheduler
persists the pending task state, releases its lease, and relies on an approval
lifecycle event or `DEVICE_SESSION_ESTABLISHED` wake to resume that same task.
Each lease holder revalidates schedule state and task eligibility before work. A paused or cancelled schedule prevents new occurrences but does not
silently cancel already-created tasks; cancellation of such a task follows the
Task Protocol and does not undo completed effects.

## 5. Timezone, daylight-saving transitions, and missed occurrences

Recurrence definitions use an IANA timezone and local calendar fields; due
instants are computed and persisted in UTC. A timezone database update affects
future calculations only; it never rewrites a processed occurrence or changes
the UTC instant already assigned to a pending occurrence.

For a local time that does not exist during a daylight-saving gap, the occurrence
is assigned to the first valid local instant after the gap. For a local time that
occurs twice during a daylight-saving fold, it fires once, at the earlier UTC
instant. The persisted occurrence identity prevents the repeated local label
from creating two tasks.

When Core is unavailable past a due instant, or a calendar fires late, apply the
schedule's stored missed policy, subject to the following bounds:

| Policy | Recovery behaviour |
| --- | --- |
| `SKIP` | Mark each missed occurrence processed without creating a task; emit `SCHEDULE_OCCURRENCE_MISSED`. |
| `RUN_ONCE` | Coalesce all missed occurrences into one task for the latest missed occurrence; mark the older ones skipped and emit `SCHEDULE_OCCURRENCE_MISSED` for each skipped occurrence. |
| `RUN_EACH` | Create at most `max_scheduler_catch_up_per_wake` tasks in chronological order. Remaining due occurrences stay durably queued and schedule a `RETRY_DUE` wake; emit `SCHEDULE_CATCH_UP_DEFERRED`. Occurrences are not discarded by this per-wake limit. |

A due occurrence is never silently discarded. Catch-up work obeys the ordinary
task concurrency, call, time, repeat, and spend bounds, plus the authoritative
per-wake ceiling in [Bounds Protocol §2](10-bounds-protocol.md#2-the-bound-set).
When that ceiling is reached, durable remaining occurrences are resumed by a
later `RETRY_DUE` wake rather than skipped or executed in an unbounded burst.

## 6. Idempotency and event contract

Occurrence deduplication uses the durable unique key defined in §3, not a newly
minted identifier format. A `SCHEDULE_TASK_CREATED` event is committed atomically
with the occurrence-to-task mapping. The event payload identifies the
`ScheduleId`, occurrence identity, and `TaskId`; it contains no raw secret or
unredacted private content. Repeated wake delivery emits no second
`SCHEDULE_TASK_CREATED` event and does not create a second task.

Scheduler lifecycle events are owned by the [Event Protocol](06-event-protocol.md#3-event-kinds):
`SCHEDULE_CREATED`, `SCHEDULE_UPDATED`, `SCHEDULE_PAUSED`, `SCHEDULE_RESUMED`,
`SCHEDULE_CANCELLED`, `SCHEDULE_OCCURRENCE_MISSED`, and
`SCHEDULE_TASK_CREATED`. They are committed in the same transaction as the
schedule/occurrence state change they describe.

## 7. Approval interaction and authority

A scheduled task follows exactly the same policy and approval evaluation as an
interactive task. `requested_by: SCHEDULER` records provenance and grants no
authority ([Capability Protocol §4.1](01-capability-protocol.md#41-requested_by)).
Any action requiring approval enters `WAITING_APPROVAL`; approval is bound to the
specific request, task, scope, and expiration under the Approval Protocol. A
schedule cannot approve, extend, or consume a grant itself. Denial or expiry is
handled as for any other task and does not count as a successful occurrence.

A scheduled task whose risk exceeds its schedule-bound task `policy_class` is
denied. Raising either ceiling requires explicit human approval and explicit
re-authorization before future execution. Ordinary failure, model fallback,
recovery, or a later schedule firing cannot raise authority.

## 8. Proactive watcher is read-only

A proactive watcher is not a scheduler for effecting work. It may wake only to
observe eligible data and create a suggestion/proposal. Its policy context
remains restricted to `OBSERVE` and `LOCAL_STATE` under the Policy Protocol;
proactive execution cannot send, write externally, delegate a GoalLatch goal, or
turn a proposal into an action without a separate user instruction and the full
normal authorization path. A proposal is recorded as `PROPOSAL_CREATED` and
requires explicit user action before any effecting task is created.

## 9. Phase boundary and invariants

The contract is frozen in P0; implementation is deferred to P3. P0 and P1 must
not connect an external scheduler or perform real scheduled effects.

| # | Invariant |
| --- | --- |
| S1 | Schedule definitions and occurrence mappings are durable; wakeups alone are never authoritative. |
| S2 | A schedule has one durable occurrence-to-task mapping; duplicate wake or Core recovery reuses it. |
| S3 | Scheduling provenance grants no policy authority and no approval. |
| S4 | Lease expiry permits reconciliation only; it never proves that no effect occurred. |
| S5 | Timezone and DST resolution are deterministic and persisted per occurrence. |
| S6 | Missed occurrences follow the stored bounded policy and are never silently discarded. |
| S7 | Approval waits release scheduler leases and resume the same task. |
| S8 | Proactive watcher execution remains read-only; proposals require a separate user action to cause effects. |
| S9 | Scheduler implementation and real scheduled effects remain deferred to P3. |
