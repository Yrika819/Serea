# Scheduler Protocol

Protocol ID: `PROTO-SCHED` · Surface: `serea.scheduler/1` · Status: **FROZEN, current architecture `serea-arch/2.2.0`** · Implementation: **P3 in progress**

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
| `revision` | Monotonically increasing generation changed by each committed definition or state mutation; claims compare the expected revision under the transaction fence. |
| `trigger` | Exactly one validated `EventPredicateV1`, `CalendarRecurrenceV1`, or dedicated wake trigger form. |
| `task_template` | A host-validated `ScheduledTaskTemplateV1` containing title and planning intent only; V1 has no permitted action arguments. |
| `policy_class` | Fixed ceiling for spawned work; a scheduled task cannot raise it. |
| `approval_policy` | The ordinary policy/approval requirements for each action. No approval is pre-granted by a schedule. |
| `timezone` | IANA time-zone identifier for calendar recurrences; required when the recurrence is local-time based. |
| `created_at`, `updated_at` | Host timestamps persisted with each record change. |
| `next_due_at` | Next calculated due instant, persisted in UTC together with the recurrence's local-time interpretation. |
| `missed_occurrence_policy` | One of `SKIP`, `RUN_ONCE`, `RUN_EACH`; validated with the schedule and constrained as below. |
| `last_processed_occurrence` | Last occurrence key durably handled, or `null` before the first firing. |

Creation, change, pause, resume, and cancellation require an authenticated user
or host administrator action. Model output may propose a schedule but cannot
create or mutate durable schedule state. Mutation and its corresponding
scheduler event use one Storage transaction: execute the schedule/state SQL
mutation, append the event and allocate its sequence inside that same uncommitted
transaction, then COMMIT once. “Persisted before event appended” specifies SQL
statement order within this transaction. It never means two transactions or a
commit between the mutation and event.

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

### 2.1 EventPredicateV1

HOST_EVENT schedules store the closed SCJ-1 object
`{"event_kind":"<registered EventKind>","version":"1"}`. These are its
only fields. The host rejects unknown or missing fields, duplicate keys,
unregistered kinds, and versions other than `"1"`. Persisted JSON is
canonical; runtime decodes it to `EventPredicateV1` and never compares raw
source text.

The only positive match is exact EventKind equality. Payload fields, task/step
IDs, actor, correlation/causation, data class, timestamps, and other JSON do
not participate. HOST_EVENT excludes `DEVICE_CONNECTED`,
`APPROVAL_GRANTED`, `APPROVAL_DENIED`, and `APPROVAL_EXPIRED`, which belong to
dedicated wake paths. It also excludes every Scheduler lifecycle kind listed
in Event Protocol §3. A Scheduler-rooted event is ineligible for every
HOST_EVENT schedule. Scheduler derives that root from durable occurrence/task
provenance, never from predicate data. V1 has no recursion override.

## 3. Persistence and recovery

Schedule definitions, state, next due instant, timezone, occurrence-processing
records, and the mapping from an occurrence to its spawned `TaskId` are stored in
Serea's durable store in one transaction per state transition. The unique
occurrence key is the existing `ScheduleId` plus its canonical occurrence
identity: for calendar recurrence, the intended local date/time and timezone;
for `HOST_EVENT`, the source `EventId`; for resume/recovery wakeups, the existing
`TaskId`. This is a storage uniqueness rule, not a new identifier format.

Each occurrence captures the template digest and class current when it is
durably resolved. A later schedule edit cannot change an existing occurrence's
planning context. Given a mapped TaskId after restart, the occurrence mapping
resolves the Schedule, occurrence, source EventId if any, and exact historical
template version.

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

### 3.1 ScheduledTaskTemplateV1

The exact template shape is the closed SCJ-1 object
`{"intent":"<prose>","title":"<TaskTitle>","version":"1"}`. It has
exactly these fields. `title` uses existing `TaskTitle` validation; `intent`
uses existing prose validation and is planning context only, never trusted
instruction, authority, policy, or executable code. V1 has no action
arguments. Unknown fields, duplicate keys, malformed text, and unsupported
versions are refused. Raw and canonical UTF-8 bytes are bounded by
`max_schedule_template_bytes = 32768`.

Template JSON cannot carry policy class, approval data, capability/provider,
ActionRequest, TaskStep, Plan, idempotency key, receipt, credential handle,
GoalLatch goal, model route, bound override, admin authorization, or arbitrary
arguments. Schedule fields remain authoritative for policy ceiling, approval
policy, owner, and trigger. Normal future Task planning and capability
validation remain in force.

The host classifies title and intent and stores their maximum class. CREDENTIAL
is forbidden; SECRET is unsupported on this blob path; PRIVATE is accepted
only through the existing protected-blob capability and otherwise fails
closed. The digest provides content identity/integrity only. Schedule and
occurrence rows pin blobs through durable references; cleanup releases them
only after no existing task needs the historical intent.

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

### 5.1 Calendar recurrence grammar V1

Calendar schedules use the closed `CalendarRecurrenceV1` JSON value defined by
[ADR-0027](../decisions/ADR-0027-calendar-recurrence-grammar-v1.md). Serea owns
its recurrence semantics; Jiff owns timezone and DST resolution only. The
Schedule's existing `timezone` field is the authoritative IANA timezone and is
not repeated in recurrence JSON.

All values contain exactly `version`, `kind`, and `anchor_local`, plus the
fields required by the selected kind. `version` is the string `"1"`.
`anchor_local` is exactly `YYYY-MM-DDTHH:MM`: no offset, seconds, fractional
part, or timezone suffix. It must name a real Gregorian date in the supported
durable range 0000-01-01 through 9999-12-31 and a valid hour/minute.

| Kind | Exact additional fields | Candidate rule |
|---|---|---|
| `ONCE` | None | Exactly the anchor local label; it has no next occurrence after durable processing. |
| `DAILY` | Integer `interval` ≥ 1 | Candidate date is on or after the anchor date and its whole local-calendar-day difference modulo interval is zero. Preserve anchor HH:MM. |
| `WEEKLY` | Integer `interval` ≥ 1; non-empty unique `weekdays` | ISO Monday-based week index from the anchor week is divisible by interval. Emit selected weekdays on or after the anchor date at anchor HH:MM. |

There is no implicit interval default. Weekday tokens are `MO`, `TU`, `WE`,
`TH`, `FR`, `SA`, and `SU`; storage and output order them canonically in that
sequence. Reject unknown kinds or fields, missing fields, duplicate JSON keys,
duplicate weekdays, non-integer numbers, interval zero, malformed local labels,
impossible dates, invalid times, offsets, seconds, and timezone text embedded in
`anchor_local`. V1 defines no `COUNT`, `UNTIL`, end date, exception dates,
`BYSETPOS`, monthly, yearly, hourly, or smaller-unit recurrence.

Persist recurrence as a closed compact SCJ-1 object with integer-only numeric
fields, duplicate-key rejection, canonical object serialization, and weekday
normalization. Equivalent semantic input is stored in one canonical form;
runtime control flow uses the decoded value rather than raw JSON text.

An occurrence identity is the owning `ScheduleId` plus the intended local label
and the Schedule's IANA timezone. It is not the resolved UTC instant, TZDB
version, lease, TaskId, or observed clock time. Encode the local label and zone
deterministically as structured data; do not introduce a global identifier
prefix. Persist the intended label, timezone, resolved UTC instant, evaluator
version, and TZDB version. Schedule edits affect future unresolved candidates
only. Existing resolved, pending, claimed, mapped, and processed rows keep
their label and instant; a new recurrence resolving to the same ScheduleId,
label, and timezone reuses the existing identity.

An ONCE recurrence has no next occurrence after its single occurrence is durably
processed; it never converts to DAILY and adds no Schedule lifecycle state.
Recurring schedules stop through the existing `PAUSED` or `CANCELLED` lifecycle.

Due instants are computed and persisted in UTC. A TZDB update affects future
unresolved occurrences only; it never changes an instant assigned to a
resolved occurrence.

### 5.2 Timezone evaluator and DST

P3 selects Jiff `=0.2.38` as the timezone conversion and gap/fold evaluator,
with `default-features = false` and exactly the required `std` and
`tzdb-bundle-always` features. This excludes
`tzdb-zoneinfo` and `tzdb-concatenated`, so authoritative recurrence resolution
cannot silently consult host OS TZDB. Jiff 0.2.38 resolves exact
`jiff-tzdb =0.1.9`; its embedded IANA database reports version `2026e`.
Crates.io metadata checked 2026-10-06 reports both packages as `Unlicense OR
MIT`, each declaring Rust 1.70; both are compatible with the workspace's Rust
1.85 MSRV. Pin the evaluator, bundled data crate, and TZDB data version and
persist evaluator/TZDB versions per resolved occurrence. Since the bundled source
is forced on every supported OS and host-database features are disabled, Linux,
macOS Intel, and macOS arm64 share the same authoritative TZDB. Jiff does not
own or expand Serea's recurrence grammar or occurrence policy.

For a local time that does not exist during a daylight-saving gap, the occurrence
is assigned to the first valid local instant after the gap. For a local time that
occurs twice during a daylight-saving fold, it fires once, at the earlier UTC
instant. The persisted intended-label occurrence identity prevents the repeated local
label from creating two tasks.

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

For schedule cancellation and occurrence claim, each claim transaction
revalidates that the schedule is `ACTIVE`, its expected revision/generation is
current, the occurrence is unmapped and unprocessed, and the scheduler lease
fence is current. If cancellation commits first, a later claim refuses. If the
claim and task mapping commit first, that already-created task may continue
under normal Task Protocol reconciliation; cancellation prevents future
occurrences and does not silently cancel the task.

Retries of an authenticated schedule command reuse the common authenticated
envelope's stable `message_id` as the durable command-deduplication identity.
The receipt binds that ID to the authenticated envelope/request digest and
stored outcome; reusing an ID with different request content is refused. A new
message ID is a new command. IDK-1 is not reused because its frozen task/action
preimage fields and semantics do not match schedule lifecycle commands.

P3 external generic subscribers are out of scope. The device timeline remains
the frozen replay surface. The Scheduler is the sole internal durable Event Bus
consumer and uses a Scheduler-specific singleton cursor/state row, not a generic
subscriber registry. In-memory notifications are wake optimizations with zero
authority. Startup order is: open/migrate Store; validate event metadata;
recover Task Engine; reconcile Scheduler occurrences; replay Scheduler events
from its durable cursor; enter live mode. Each replay pass snapshots committed
high-water `seq` and processes only through that value. Events produced while
handling the batch are processed on a later pass.

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

## 10. Changelog

- 2026-10-06: ADR-0027 defines `CalendarRecurrenceV1` with ONCE, DAILY, and
  WEEKLY only; architecture advances to `serea-arch/2.1.0`. The Scheduler
  surface remains `serea.scheduler/1`.
- 2026-10-07: ADR-0028 defines EventPredicateV1, ScheduledTaskTemplateV1,
  Scheduler causal-loop exclusion, and `max_schedule_template_bytes = 32768`;
  architecture advances to `serea-arch/2.2.0`. Event and Scheduler surfaces
  remain `/1`.
