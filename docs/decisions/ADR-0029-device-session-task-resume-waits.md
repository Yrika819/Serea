# ADR-0029: Durable device-session task resume waits

- Status: **Accepted**
- Date: 2026-10-07
- Architecture version: `serea-arch/2.3.0`
- Event surface: `serea.event/1` (unchanged)
- Scheduler surface: `serea.scheduler/1` (unchanged)
- Task surface: `serea.task/2` (unchanged)
- Decision owner: Serea

## Context

The Scheduler contract assigns `DEVICE_SESSION_ESTABLISHED` the job of
resuming an existing task after its device reconnects. Event correlation,
task origin, task state, and device ownership do not independently prove that
a task is waiting for that particular session. Resume eligibility therefore
needs an explicit durable fact, and replay must distinguish a connection that
predates the wait from one that follows it.

## Decision

`DEVICE_CONNECTED` has a closed kind-specific payload,
`DeviceConnectedPayloadV1`, with exactly one member: `device_id`, validated by
the existing `DeviceId` grammar. It identifies the device whose session was
established. It contains no task selector or authority. The `SereaEvent` shape
is unchanged, so the Event surface remains `serea.event/1`.

Only an explicit durable `DeviceResumeWaitV1` makes an existing task eligible
for this wake. The wait is keyed by `TaskId` and records exactly one `DeviceId`,
the authoritative blocked task revision, the Event Bus high-water sequence
observed when registration commits, and its creation time. A wait is valid only
while the same task remains at that revision in `BLOCKED` with
`DEVICE_OFFLINE`. Task origin, `WAITING_USER`, arbitrary `BLOCKED`, and event
correlation are never eligibility signals.

Task Engine exposes a narrow `block_for_device` operation that atomically
validates the expected state/revision, transitions the task to
`BLOCKED/DEVICE_OFFLINE`, writes its journal and events, registers the wait,
and captures the committed Event Bus high-water. Generic `block` refuses
`DEVICE_OFFLINE`; callers must supply a DeviceId through this operation.

For a `DEVICE_CONNECTED` event at sequence `S`, a wait can be materialized
only when its registration high-water is strictly less than `S`. Scheduler
first creates durable internal `DeviceSessionResumeWake` rows, uniquely keyed
by `(source_event_id, task_id)`, in stable bounded batches. The Event Bus cursor
does not advance past the event until all eligible waits existing before `S`
have been materialized or classified stale. Later waits cannot be captured by
replay of an older event.

Each materialized wake is processed independently. One transaction rechecks
the wait, task state, blocked reason, revision, and device, then moves a valid
task from `BLOCKED` to `READY`, consumes the wait and wake, writes TaskJournal,
and emits the ordinary Task lifecycle event. A stale wake is consumed without
changing the task or emitting `TASK_RESUMED`. The Scheduler never creates a
replacement task, capability call, approval, or policy authority.

Task deletion removes its wait and pending wakes. Other task transitions make
the old wait stale through the task revision fence; stale records are
deterministically cleaned up. Materialized wakes survive source-event content
retention and are handled by Scheduler recovery. An intentionally expired
range has no recoverable device identity and never fabricates a wake.

## Compatibility and versioning

This is a backward-compatible architecture-minor addition: the existing
`DEVICE_CONNECTED` event gains a required kind-specific payload member, and
Scheduler gains an internal durable wait/wake contract. The architecture
advances from `serea-arch/2.2.0` to `serea-arch/2.3.0`. The `SereaEvent` object
shape is unchanged, so `serea.event/1` does not change. The existing Scheduler
and Task surfaces gain no breaking fields, so `serea.scheduler/1` and
`serea.task/2` remain unchanged. Migration 0002 may be amended before P3 is
released; migration 0001 remains immutable.

## Consequences

- Device session establishment satisfies only an explicit availability wait.
- Event sequence ordering gives wait registration and reconnect a deterministic
  race boundary without wall-clock comparisons.
- Fan-out is durable, bounded, replay-safe, and independent per task.
- P3 defines and consumes the durable event fact; transport, authentication,
  Android, and real device sessions remain future work.
