# P3C Task Engine event inventory

This inventory records the mapping implemented by the P3 fixed Storage
composition: `TaskAuditParticipant` writes the P2 journal and `EventParticipant`
maps the same immutable successful-write facts to zero or one `serea.event/1`
objects. Both are inside the originating task operation savepoint and outer
transaction. The event is appended only after the operation has produced its
successful state facts; refusal/no-op paths produce no success event.

All emitted events take actor kind, actor ID, actor version, data class,
occurred-at time, optional causation ID, task ID, optional step ID, and attempt
from those immutable facts. Correlation ID is the TaskId. Trace contains the
TaskId and, when present, StepId and attempt. Every emitted payload includes
`task_id`. `TASK_STATE_CHANGED` and `TASK_RESUMED` also include `from` and `to`.
Terminal-specific payloads are `reason_code`, `blocked_reason`,
`result_digest`, or `cancelled_by` as applicable. Task titles, step inputs,
provider data, result bodies, and schedule data are never copied into task
lifecycle payloads. `serea.event/1` remains unchanged.

| Storage operation / Task Engine path | Successful mutation and emitted event | No-op, refusal, and rollback behavior |
| --- | --- | --- |
| `TaskInserted` / `create_task` | Inserts the task and P2 `TASK_INSERTED`; emits `TASK_CREATED`. | Existing TaskId, validation, or policy refusal emits no event. Journal, task, event, and sequence allocation roll back together on participant/append/transaction failure. Repeating a committed TaskId is refused without a second event. |
| `PlanningStarted` / `start_planning` | Changes the task state; from `RECEIVED` emits `TASK_STARTED`, from `BLOCKED` or `WAITING_USER` emits `TASK_RESUMED`, and another actual state edge emits `TASK_STATE_CHANGED`. | A same-state result emits no event. Stale expected state/revision or invalid transition emits no event. |
| `PlanPersisted` / `persist_plan` | Persists the new plan revision and steps, P2 `PLAN_PERSISTED`, and `TASK_STATE_CHANGED` (`PLANNING` to `READY`). | Invalid/oversized/duplicate plan or revision refusal emits no event. State, steps, journal, event, and sequence roll back together. |
| `LeaseAcquired` / `acquire` | Persists the step lease and P2 `STEP_LEASE_ACQUIRED`; emits no task event because task lifecycle state did not change. | Busy, stale, expired, or otherwise refused lease emits no event. |
| `LeaseReleased` / `release` | Releases the step lease and P2 `STEP_LEASE_RELEASED`; emits no task event. | Stale/invalid guard emits no event. |
| `AttemptStarted` / `start_attempt` | Persists attempt facts and P2 `STEP_ATTEMPT_STARTED`; an actual task state edge emits its lifecycle event (`TASK_STARTED` when entering work from no prior task state, `TASK_RESUMED` when leaving a blocked/waiting state, otherwise `TASK_STATE_CHANGED`). | Invalid or stale lease emits no event. A step-only write with no task state edge emits no task event. |
| `StepSucceeded` / successful outcome | Commits step success, receipt/evidence where applicable, and P2 `STEP_COMMITTED`/`RECEIPT_RECORDED`; task transition emits `TASK_COMPLETED` on terminal success or `TASK_STATE_CHANGED` for another state edge. `TASK_COMPLETED` may include only the result digest. | Duplicate/stale outcome or failed validation emits no event. No result body or receipt contents are copied into the event. |
| `StepFailed` / failed outcome | Commits step failure and P2 `STEP_FAILED`; task transition emits `TASK_FAILED` for terminal failure, `TASK_BLOCKED` on entry to `BLOCKED`, `TASK_RESUMED` when leaving blocked/waiting, or `TASK_STATE_CHANGED` for another actual edge. | Duplicate/stale outcome or failed validation emits no event. |
| `Blocked` / explicit block | Persists `BLOCKED` and emits `TASK_BLOCKED` with the typed reason code as `blocked_reason`. | Invalid transition emits no event. |
| `InvariantFailed` / invariant refusal transition | Persists the existing terminal failure transition and emits `TASK_FAILED` with `reason_code`. | If the invariant path refuses before a successful state write, no event is emitted. |
| `Cancelled` / `cancel_task` | Persists cancellation and P2 `TASK_CANCEL_REQUESTED`; emits `TASK_CANCELLED` with `cancelled_by`. | Repeated/terminal cancellation or refused transition emits no success event. |
| `RecoveryDecision` / recovery classification | Persists the recovery decision journal row; emits no event. It is diagnostic classification, not a user-visible lifecycle change. | Classification failure rolls back its transaction; no event is emitted. |
| `RecoveryStateChanged` / task recovery repair | Persists repaired task state and existing recovery journal semantics; emits a lifecycle event only for an actual task state edge, using the same started/resumed/specific terminal/generic mapping above. | No state edge emits no event. Recovery refusal/transaction failure emits no event and rolls state, journal, event, and sequence back together. |
| `delete_task` / explicit task-row deletion | Removes task-owned rows through the existing deletion API; emits no event. It is not a task lifecycle transition and no frozen `TASK_DELETED` kind exists. | Missing task is a write-free zero-count result. Storage failure rolls the deletion transaction back. P3 does not reinterpret this API as a right-to-delete cascade or emit `DELETION_CASCADE_COMPLETED`. |
| `load` / read-only query | Reads a task projection; emits no event and writes no journal row. | Missing/corrupt task returns the existing typed error; no durable state changes. |

Specific lifecycle kinds take precedence over `TASK_STATE_CHANGED`: completed,
failed, cancelled, and blocked outcomes use their frozen specific kind. A
transition cannot emit both a specific lifecycle kind and a generic duplicate.
Step lease operations and recovery classification do not create lifecycle
events. All event append errors, journal errors, savepoint failures, and outer
transaction failures abort the same transaction. There is no historical P2
journal backfill.

P3C test evidence is recorded in [P3 closure](P3-closure.md). This inventory
describes the currently implemented Task Engine paths; later Scheduler events
are covered by Scheduler Protocol and are not task transition events.
