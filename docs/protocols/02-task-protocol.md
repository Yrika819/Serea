# Task Protocol

Protocol ID: `PROTO-TASK` · Surface: `serea.task/2` · Status: **FROZEN current architecture `serea-arch/2.3.0`**

The `AssistantTask` is Serea's unit of durable work. It is **not** GoalLatch's
`Goal`. The two are different concepts with different lifecycles, different
persistence, and different owners, and Serea imports none of GoalLatch's
semantics.

| | Serea `AssistantTask` | GoalLatch `Goal` |
| --- | --- | --- |
| Owner | Serea Core | GoalLatch / Local MCP |
| Purpose | Assistant reasoning and personal-assistant work | Local PC / code development work |
| Lifecycle | See §4 | Owned by GoalLatch; Serea does not define it |
| Persistence | Serea's SQLite | GoalLatch's |
| Relationship | May *delegate* to a GoalLatch capability | Reached only through `HostGoalProvider` |

A GoalLatch goal is reachable from Serea **only** as an opaque handle behind
the `host.goal.*` capability family, and only through the adapter in
[GoalLatch Adapter Protocol](08-goallatch-adapter-protocol.md).

---

## 1. Model conversation history is not task state

Conversation history is context. Context can be truncated, summarized,
compacted, or discarded. Task state cannot.

Therefore:

- **All** task progress lives in durable storage, addressed by `TaskId` and
  `StepId`.
- The model is given a *rendered view* of task state as context each turn. It
  is a read-only projection. The model cannot write it.
- Deleting all conversation history must not change any task's outcome.
- Restarting the host must not change any task's outcome.

Any design where "the conversation is the task" is rejected.

## 2. `AssistantTask`

```json
{
  "task_id": "tsk_01JQ8Z9K3M7QWXR4V2T6YH0BNA",
  "kind": "USER_REQUEST",
  "title": "Summarize today's mail",
  "state": "EXECUTING",
  "origin": { "kind": "USER_MESSAGE", "device_id": "dev_01JQ8ZC5N8TVG3K6MRQ2XW9JHF", "message_id": "…" },
  "data_class": "PERSONAL",
  "policy_class": "OBSERVE",
  "created_at": "2026-10-01T09:14:20.001Z",
  "updated_at": "2026-10-01T09:14:23.902Z",
  "deadline_at": null,
  "attempt_budget": { "max_model_calls": 12, "max_tool_calls": 24, "max_attempts_per_step": 3 },
  "steps": [ ],
  "blocked_reason": null,
  "result_summary": null
}
```

`kind` ∈ `USER_REQUEST`, `SCHEDULED`, `PROACTIVE`, `DELEGATED_HOST_GOAL`,
`MAINTENANCE`.

`policy_class` is the **maximum** risk class across all steps the task is
permitted to perform. It is host-assigned when the task is created and immutable
for that task. Neither a model plan, an ordinary capability approval, nor a host
policy change may raise it. A step whose capability risk class exceeds the
task's `policy_class` is rejected; if the user wants broader authority, an
explicit new request creates a separate task with its own host-assigned ceiling.

### 2.1 Why `policy_class` is on the task

It bounds blast radius at the task level, not merely per step. A task that has
been approved for `OBSERVE` cannot later execute a `DESTRUCTIVE` step just
because a model proposed one. The task-level class is a ceiling the whole plan
must fit under, which makes approval requests comprehensible to a human: "this
task may at most send email", not "this task may do these eleven things".

## 3. `TaskStep`

Every step carries enough information to be re-executed or verified after a
hard restart.

```json
{
  "step_id": "stp_01JQ8Z9M3R2CVN8H5FWK7PQDSF",
  "task_id": "tsk_01JQ8Z9K3M7QWXR4V2T6YH0BNA",
  "sequence": 3,
  "kind": "CAPABILITY",
  "status": "SUCCEEDED",
  "attempt": 1,
  "lease_generation": 1,
  "idempotency_key": "idk_9f2c…",
  "provider_id": "calendar",
  "capability_id": "calendar.events.list",
  "capability_version": "1.2.0",
  "input_digest": "sha256:…",
  "result_digest": "sha256:…",
  "side_effect_receipt": null,
  "started_at": "2026-10-01T09:14:22.100Z",
  "completed_at": "2026-10-01T09:14:22.512Z",
  "lease_owner": null,
  "lease_expires_at": null,
  "error": null
}
```

`kind` ∈ `CAPABILITY`, `MODEL_TURN`, `WAIT_APPROVAL`, `WAIT_USER`,
`WAIT_SCHEDULE`, `VERIFY`, `NOTIFY`, `DELEGATE`.

The exact field-presence and kind matrices are part of this contract in
[ADR-0018 §3/§4](../decisions/ADR-0018-taskstep-lifecycle-and-field-presence.md#3-the-presence-matrix).
Seven fields are unconditionally required: step_id, task_id, sequence, kind,
status, attempt, input_digest. Four conversions to Option are idempotency_key,
result_digest, started_at, completed_at; provider/capability/version already Option.
Missing and null both mean None; serialization omits None. A required matrix cell
is non-null; an absent cell accepts missing/null and refuses a supplied value.

Wire status remains an open uppercase code. Known statuses PLANNED, LEASED,
EXECUTING, WAITING, SUCCEEDED, FAILED, RECONCILED_ABSENT receive lifecycle presence
validation; unknown well-formed status parses and round-trips with supplied-value
validation. Kind invariants always apply: provider/capability/version/key required
for CAPABILITY/DELEGATE/VERIFY, absent for the other five kinds.
`side_effect_receipt` must also be absent for all five non-capability kinds on
**every status, including unknown codes**: those steps cannot carry external
action semantics. The optional SUCCEEDED receipt matrix cell is capability-shaped
only. Rust and schema must both refuse supplied receipts for each such kind across
all known statuses and an unknown status; missing/null is absent and serializes
by omission. Future engine
execution blocks unknown status with UNRECOGNISED_STATE; no engine exists in P2A.

Optional lease_generation is None on never-leased PLANNED, positive u32 after
acquisition (including terminal outcomes). The Rust field remains `Option<u32>`;
wire decoding uses field-local RawValue tokens to check positive-u32 membership
exactly, without f64 rounding. Mathematically integral JSON numeric spellings
such as `1.0`/`1e0` accept; wire zero, true fractions (including near-integers)
and overflow refuse. Value inputs preserve numeric text through
`serde_json/arbitrary_precision`; direct schema validation uses
`jsonschema/arbitrary-precision` with an exact 0.58.3 pin and the narrow
`vendor/jsonschema-value` integer-classification/checked-conversion patch.
[Launch §3](../plans/P2-6.1-sol-launch.md#3-dependency-lines-current-p2a-integration-and-p2c-candidate)
states its limited guarantee, not unrestricted exact schema arithmetic.
SCJ-1 canonical hashing separately refuses decimal/exponent spellings; wire
acceptance does not widen that domain. SQL0 maps to wireNone; SQL positive maps
to checked Some(u32), with SQL mapping/runtime still deferred. Full fencing remains
Proposed ADR-0024 runtime design, not a guarantee implemented by this wire member.

Rust construction uses `TaskStepDraft` → `TaskStep::new`/`TryFrom`, validated by
`StepPresence`. Checked state is private; fields are read-only through `Deref`,
with no public mutation bypass. Changes require a draft and revalidation. Unknown
extensions are preserved, but keys equal to reserved step members are refused,
even if the corresponding optional member is absent. The companion pinned
`vendor/serde_json` transport patch distinguishes internal synthetic numeric/raw
keys from literal object keys through Serde buffering, preserving opaque extension
values on raw and owned/borrowed Value paths; it reserves no new wire names.

### 3.1 Field obligations

| Field | Why it exists |
| --- | --- |
| `idempotency_key` | Lets a post-crash re-issue be recognized as the same action, not a new one. |
| `attempt` | Bounds retries; distinguishes the crash-recovered attempt from a deliberate retry. |
| `input_digest` | Duplicate detection without retaining full arguments. |
| `result_digest` | Detects result corruption or partial writes on recovery. |
| `side_effect_receipt` | The proof of external effect. Non-null iff an effect occurred. |
| `lease_owner` / `lease_expires_at` | Prevents two workers executing one step concurrently. |
| `error` | Last failure, preserved so a terminal `FAILED` task explains itself. |

`arguments` themselves are retained in a separate content-addressed blob store,
referenced by `input_digest`, so the steps table stays small and so PII-bearing
arguments can be redacted or expired independently of task metadata.

### 3.2 Step ordering and concurrency

`sequence` gives a total order. Steps with `sequence < n` that are required for
step `n` must be `SUCCEEDED`. Independent steps *may* run concurrently only
when both are `IDEMPOTENT` and neither is an approval step. Every other
configuration runs strictly in sequence order.

This is a deliberate simplicity choice: sequential execution is trivially
correct and trivially recoverable. Parallelism can be added later behind the
same step record without changing the contract.

## 4. Task state machine

### 4.1 States

| State | Meaning |
| --- | --- |
| `RECEIVED` | Task accepted and durably recorded. No work begun. |
| `PLANNING` | Deriving a step plan. Model turns happen here. |
| `READY` | Plan complete, no step in flight, ready to execute. |
| `EXECUTING` | One or more steps in flight. |
| `WAITING_APPROVAL` | Blocked on a human or a scoped grant. |
| `WAITING_USER` | Blocked on user input that is not an approval. |
| `VERIFYING` | Confirming the outcome of completed steps. |
| `COMPLETED` | Terminal success. |
| `FAILED` | Terminal failure. |
| `BLOCKED` | Cannot proceed without an external change (device offline, credential revoked, ambiguous effect). |
| `CANCELLED` | Terminal, user- or system-initiated. |

#### 4.1.1 DeviceResumeWaitV1

`BLOCKED` with reason `DEVICE_OFFLINE` is resumable on a device session only
when an explicit durable `DeviceResumeWaitV1` exists for the task. The wait is
keyed by `TaskId` and contains one `DeviceId`, the task's blocked revision, the
Event Bus committed high-water sequence captured when the wait is registered,
and `created_at`. It means exactly that this task is blocked waiting for this
specific device session.

Task Engine's `block_for_device` transition atomically enters
`BLOCKED/DEVICE_OFFLINE`, writes the task journal and lifecycle events, and
registers the wait with the current Event Bus high-water. Generic `block` with
`DEVICE_OFFLINE` is refused because it does not supply a device identity.
Neither `TaskOrigin.device_id`, task kind/title, `WAITING_USER`, nor arbitrary
`BLOCKED` state grants resume eligibility.

The wait is valid only while the same task remains `BLOCKED/DEVICE_OFFLINE` at
the recorded task revision. Any other task transition makes it stale; TaskId
deletion removes it. A matching `DEVICE_CONNECTED` is considered only when
its Event Bus sequence is strictly greater than the wait's registration
high-water. Device reconnect moves the existing task from `BLOCKED` to
`READY`; it creates no task, plan, capability request, or approval authority.

Terminal states: `COMPLETED`, `FAILED`, `CANCELLED`.

`BLOCKED` is **not** terminal. A blocked task holds its state and resumes when
the blocking condition clears. This distinction matters: a task blocked by an
offline phone must survive indefinitely and complete once reconnected, whereas
a `FAILED` task is finished.

### 4.2 Transitions

```
RECEIVED ──> PLANNING ──> READY ──> EXECUTING ──> VERIFYING ──> COMPLETED
                 │           ↑         │  │
                 │           │         │  └──> WAITING_USER ──> READY
                 │           │         │
                 │           │         └──> BLOCKED ──> READY
                 │           │
                 └───────────┴──> WAITING_APPROVAL ──> READY
                                                └──> FAILED

Any non-terminal state ──> CANCELLED
EXECUTING/VERIFYING ──> FAILED (on step budget exhaustion)
PLANNING ──> FAILED (on unrecoverable planning failure)
```

Legal transitions, exhaustively:

| From | To |
| --- | --- |
| `RECEIVED` | `PLANNING`, `CANCELLED`, `FAILED` |
| `PLANNING` | `READY`, `WAITING_USER`, `WAITING_APPROVAL`, `BLOCKED`, `CANCELLED`, `FAILED` |
| `READY` | `EXECUTING`, `PLANNING`, `WAITING_APPROVAL`, `CANCELLED`, `FAILED` |
| `EXECUTING` | `VERIFYING`, `WAITING_APPROVAL`, `WAITING_USER`, `BLOCKED`, `READY`, `CANCELLED`, `FAILED` |
| `WAITING_APPROVAL` | `READY`, `CANCELLED`, `FAILED` |
| `WAITING_USER` | `READY`, `PLANNING`, `CANCELLED`, `FAILED` |
| `VERIFYING` | `COMPLETED`, `EXECUTING`, `BLOCKED`, `CANCELLED`, `FAILED` |
| `BLOCKED` | `READY`, `PLANNING`, `CANCELLED`, `FAILED` |
| `COMPLETED` | — |
| `FAILED` | — |
| `CANCELLED` | — |

Any transition not listed is a host bug and fails the task to `FAILED` with an
invariant-violation error rather than persisting an illegal state.

### 4.3 Planning rules

1. Planning is bounded by `attempt_budget.max_model_calls`.
2. A plan is a list of steps, each with a capability (or a model-turn /
   wait step). A plan step with no capability is not executable.
3. Planning that proposes a capability above the task's `policy_class` is
   rejected with `TASK_POLICY_CEILING_EXCEEDED`; an ordinary capability
   approval cannot override this denial. The task ceiling is immutable after
   task creation. If the user wants broader authority, they must submit a new
   explicit request that creates a new task with a higher host-assigned ceiling.
   The model cannot initiate or authorize that new task.
4. **A plan is not authority.** Persisting a plan changes no policy decision.
5. A model may revise a plan. Each revision increments `plan_revision` and
   is recorded as evidence. Revising a plan never re-executes a completed
   step automatically.

## 5. Execution rules

1. **Persist before advancing.** A step's success is committed, with its
   receipt, before the task state moves to the next step. A crash between
   effect and commit is handled by idempotency-key reconciliation, not by
   hope.
2. **One effecting step at a time** per task.
3. **Lease before execute.** A worker takes a lease (`lease_owner`,
   `lease_expires_at`) before invoking a provider. An expired lease means the
   prior worker is presumed dead and the step is reclaimable — reconciled via
   idempotency key.
4. **Ambiguity blocks, never guesses.** See
   [Capability Protocol §6.2](01-capability-protocol.md#62-the-ambiguous-rule).
5. **Cancellation is cooperative.** A cancelled task stops taking new steps;
   an in-flight provider call is aborted at the deadline boundary, and its
   step records whatever is known.

## 6. Recovery

On startup the host:

1. Opens durable storage and applies pending migrations.
2. Loads tasks whose state is non-terminal.
3. For each `EXECUTING` task with an expired lease, reconciles the
   in-flight step:
   - No receipt and `replay_safety: IDEMPOTENT` → re-issue with the same
     idempotency key.
   - No receipt and `NON_REPLAYABLE`/`CONDITIONAL` → re-issue only under the
     §6.2 reconciliation rule.
   - Receipt present → treat as succeeded; ensure the state transition is
     committed.
4. For each `WAITING_APPROVAL` task, re-renders the pending approval against
   the live device roster. Approvals for absent devices remain pending, never
   auto-granted and never auto-denied.
5. Emits `TASK_STARTED` for each resumed task so the activity timeline is
   truthful about what happened.

Recovery is **idempotent by construction**: running it twice changes nothing
the second time.

## 7. Cancellation

Cancellation is a first-class user capability and must work from the Android
client, from the CLI, and from an approval prompt.

- Cancelling sets `CANCELLED` and stamps `cancelled_at` and
  `cancelled_by`.
- Already-succeeded steps keep their receipts. Serea does not attempt to undo
  external effects; if a capability has an inverse, that inverse is a *new
  capability* requiring its own policy and approval, never an implicit
  rollback.
- Cancellation emits `TASK_CANCELLED`.

## 8. Task retention and privacy

Tasks contain PERSONAL-or-higher data by construction. Retention bounds are
host configuration:

- Default task retention: 30 days.
- On expiry, task bodies are deleted; aggregate counters and non-identifying
  metrics survive.
- Deleting a task cascades to its steps, evidence payloads, and conversation
  references — but **not** to memory items that were explicitly extracted,
  which have their own retention and provenance.
- A user-visible "forget this" request deletes task data and any memory items
  derived from that task in one operation.

## 9. Invariants summary

| # | Invariant |
| --- | --- |
| T1 | Task state is durable and authoritative; conversation history is not. |
| T2 | Deleting all conversation history does not change any task outcome. |
| T3 | Restarting the host does not change any task outcome. |
| T4 | A step's success and receipt are persisted before the task advances. |
| T5 | Recovery is idempotent; running it twice is a no-op. |
| T6 | Task `policy_class` is a ceiling no step may exceed. |
| T7 | Only the listed transitions are legal; anything else fails the task explicitly. |
| T8 | Terminal states have no outgoing transitions. |
| T9 | Cancellation never implicitly undoes an external effect. |
| T10 | Task deletion cascades to derived data; retained memory items retain provenance. |
## 10. P2A migration and phase boundary

[Launch migration note](../plans/P2-6.1-sol-launch.md#41-sereatask1-sereatask2)
names every current consumer; no deployed database exists to rewrite. P2A owns
wire validation only. Append-only runtime plan revisions, leases, receipt/journal
atomicity, recovery and unknown-status execution are later-phase obligations.
They must not be reported implemented or tested from these docs-only changes.

## 11. Changelog

- 2026-10-03: frozen current task/2 field presence per Accepted ADR-0018:
  four Option conversions, seven unconditional fields, checked private state,
  reserved-extension-key refusal, optional lease_generation and non-capability
  receipt absence on all statuses including unknown. Missing/null/omission and
  open status preserved. P2A wire/schema slices implement exact generation
  decoding without f64 rounding, with precision dependency scope as above. The
  coordinator records current final workspace/MSRV validation, test counts,
  review and integration status in the [closure record](../plans/P2A-review-and-closure.md).
  Full fencing remains Proposed.
