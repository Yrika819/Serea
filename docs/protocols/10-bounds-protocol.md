# Bounds Protocol

Protocol ID: `PROTO-BOUNDS` · Surface: `serea.bounds/1` · Status: **FROZEN for P0**

This protocol makes "bounded orchestration" concrete. It owns every bound the
host enforces on models, tools, retries, tasks, approvals, and the device link.
A bound is a **host-set limit on how much work may happen**; it is never a
statement about what the work is *allowed* to do.

---

## 1. Why the host owns the loop

> The model may propose that it is finished. Only the host decides that asking
> has stopped.

The model has **no self-termination authority**. Its one lever is proposing a
terminal outcome — `COMPLETE` — in structured output, which is a proposal the
host evaluates like any other. The host alone decides whether to stop, continue,
re-plan, or fail. Concretely, a model cannot:

- declare the task finished by ceasing to produce output;
- extend its own budget, or ask for a larger one as part of a plan;
- choose to retry forever, or choose to retry at all;
- set, lower, or raise any bound in this document;
- learn that a bound is near its limit and "conserve" — the model is never told
  a bound, so it cannot game it.

This is the same shape as the rest of the architecture: the model proposes, the
host disposes
([Capability Protocol §1](01-capability-protocol.md#1-core-principle),
[Model Protocol §1](03-model-protocol.md#1-core-principle)). A loop whose
termination condition lives inside the loop is a loop with no bound.

The second reason the host owns the loop is **cost and blast radius**. A model
in a retry loop burns money and produces user-visible side effects at a rate no
user can supervise. Bounds are the mechanism that makes "it got stuck" a
detectable, bounded, reported event rather than a surprise on the credit card.

---

## 2. The bound set

The authoritative list. Every bound the host enforces appears here; a bound
enforced anywhere else is a bug.

| Bound name | Default | Scope | Exhaustion behaviour |
| --- | --- | --- | --- |
| `max_model_calls_per_task` | 12 | per-task | Task → `FAILED`, reason `BOUND_EXCEEDED_MODEL_CALLS` |
| `max_tool_calls_per_task` | 24 | per-task | Task → `FAILED`, reason `BOUND_EXCEEDED_TOOL_CALLS` |
| `max_model_turns_per_task` | 12 | per-task | Task → `FAILED`, reason `BOUND_EXCEEDED_MODEL_TURNS` |
| `max_output_tokens_per_call` | 2048 | per-call | Call truncated, `finish_reason: LENGTH`; the step then fails `VALIDATION`, never proceeds on partial structured output |
| `max_attempts_per_step` | 3 | per-step | Step gives up with the last error preserved; task → `FAILED` if the step is terminal |
| `max_repair_attempts` | 2 | per-step | Step → `ActionErrorKind::VALIDATION`; no partial acceptance |
| `max_fallback_depth` | 1 | per-step | The next model failure ends the step |
| `max_fallback_chain_length` | 2 | per-step | A third model in the chain for one step ends it |
| `max_task_wall_clock_ms` | 900000 | per-task (active time) | Task → `FAILED`, reason `BOUND_EXCEEDED_WALL_CLOCK` |
| `max_identical_action_repeats` | 2 | per-task | Loop stops; task → `FAILED`, reason `BOUND_EXCEEDED_REPEATED_ACTION` |
| `max_pending_approvals_per_task` | 5 | per-task | Task → `BLOCKED`, `blocked_reason: APPROVAL_BACKLOG`; resumes when one resolves |
| `max_replan_revisions_per_task` | 4 | per-task | Further revisions rejected; the current plan proceeds or fails |
| `max_concurrent_tasks` | 8 | global | Excess tasks stay in `RECEIVED`, queued and visible |
| `max_concurrent_steps_per_task` | 1 | per-task | Additional independent steps queue rather than run |
| `max_lease_seconds` | 120 | per-step | Lease expires; the step is reclaimable, reconciled by idempotency key |
| `max_proactive_proposals_per_day` | 20 | global (per calendar day) | Watcher pauses until the next day; further candidates are dropped with an event |
| `max_events_per_minute_per_device` | 600 | per-device | Frame refused; device backs off; dependent tasks → `BLOCKED` |
| `max_concurrent_device_sessions` | 4 | global | Refusal to open beyond the limit |
| `max_task_total_tokens` | 128000 | per-task | Task → `FAILED`, reason `BOUND_EXCEEDED_TOKEN_BUDGET` |
| `max_daily_spend_usd` | 5.00 | global (per calendar day) | Task → `FAILED`, reason `BOUND_EXCEEDED_DAILY_SPEND` |
| `max_retained_tasks` | 500 | global | Oldest terminal tasks purged per retention; never non-terminal |
| `max_scheduler_catch_up_per_wake` | 10 | per-schedule per wake | Remaining due occurrences stay durably queued for a `RETRY_DUE` wake; emit `SCHEDULE_CATCH_UP_DEFERRED`; no occurrence is silently dropped |
| `duplicate_window_ms` | 86400000 (24 h) | global | Not an exhaustion; a match returns `DUPLICATE_SUPPRESSED` (§5) |
| `approval_request_expiry_ms` | 1800000 (30 min) | per-approval | Approval → `EXPIRED`, never a grant |
| `task_retention_days` | 30 | per-task | Task body deleted; counters survive |

### 2.1 Where these live in durable state

`max_model_calls_per_task`, `max_tool_calls_per_task`, and
`max_attempts_per_step` are materialised on the task itself as
`attempt_budget` at task creation ([Task Protocol §2](02-task-protocol.md#2-assistanttask)).
A bound is read from durable state at the moment of the check, never from a
cached value in memory that a restart could invalidate.

### 2.2 `max_model_calls` versus `max_model_turns`

They are separate counters because they count different things and they fail
differently:

- `max_model_calls_per_task` counts **every** billable invocation: planning
  turns, repair calls, and fallback attempts.
- `max_model_turns_per_task` counts **plan revisions** — model turns that
  produced or revised the action plan.

In the default configuration the call budget binds first, and it always binds
whenever a repair or fallback occurs, because those consume a call without
consuming a turn. Both are 12 at default so that neither is a surprise limiter;
an operator tuning cost down tunes `max_model_calls_per_task`, not the turn
count.

### 2.3 Zero and negative values

A bound of `0` means the dimension is disabled — a task may not call a model at
all, or the host may not run tasks at all. A negative bound is a configuration
error rejected at startup, not clamped to zero. Clamping a typo into a working
value is how a bound quietly stops bounding.

---

## 3. Who may set a bound

> Bounds are host configuration and durable policy. Nothing else. Not the model,
> not the phone.

| Actor | May lower | May raise | Audited |
| --- | --- | --- | --- |
| User, local admin surface | Yes, always, immediately | Only with explicit admin confirmation | Yes |
| Administrator | Yes | Yes, with a reason string | Yes, with before/after diff |
| Durable policy rule | Yes | Yes | Yes |
| The model | Never | Never | Not applicable |
| Android client, ordinary settings screen | Notification channel and timeline display preferences only — **no bound** | Never | Yes for the preferences |
| A device-signed request | Never | Never | Refused |
| A capability provider | Never | Never | Refused |

- **Lowering is always permitted** and takes effect immediately, including
  mid-task. There is no admin gate on making the system more restrictive; a user
  who wants a smaller blast radius gets one without asking anyone.
- **Raising is an admin action.** It requires the local admin surface, a stated
  reason, and emits `BOUNDS_CHANGED` with the before/after diff, the actor, and
  the reason ([Policy Protocol §7](04-policy-protocol.md#7-policy-changes-are-audited)).
- **Device bounds are not user-editable.** The Android client has no screen that
  writes a bound. This is deliberate and matches
  [Model Protocol §8](03-model-protocol.md#8-codex-exclusion): settings that
  change the host's behaviour belong on the host, where they are logged.

### 3.1 Per-task tightening is monotonic

An individual task may carry a ceiling **lower** than the global default. It may
never carry a higher one:

```
effective_bound = min(global_default, task_ceiling)
```

| Attempted change | Result |
| --- | --- |
| Task requests `max_model_calls_per_task = 6` against a default of 12 | Accepted, recorded in the task's `bound_overrides` with the actor |
| Task requests `max_model_calls_per_task = 50` against a default of 12 | Rejected at task creation, `BoundConfigurationError` |
| Administrator lowers the global default to 6 while a task has a ceiling of 12 | Effective becomes 6 |
| Administrator raises the global default while a task has a ceiling of 6 | The task keeps 6 until it finishes |

The last row is why tightening is one-way: a task's ceiling is a promise the
user was shown, and no later configuration change may quietly remove the limit
they were told about. Only a new task sees the new value.

---

## 4. Repeated-action detection

This is the mechanism behind `max_identical_action_repeats`, and it exists
because a *loop* — not a single call — is how a model burns a budget.

### 4.1 The window

The host keeps, per task, a rolling window keyed on
`(capability_id, arguments_digest)`. A digest is `sha256` over the canonical
JSON of the arguments
([Capability Protocol §4.3](01-capability-protocol.md#43-arguments_digest)), so
the key is stable across restarts and comparable across steps, tasks, and
requesters. The counter measures authorized execution attempts, not requests
that policy or approval refused.

| Property | Value |
| --- | --- |
| Key | `(capability_id, arguments_digest)` |
| Scope | Per task. Never global — two unrelated tasks doing the same read is not a loop. |
| Window | Retains the last `max_identical_action_repeats + 1` occurrences (3 at default) |
| Applies to | Authorized capability execution attempts, including reads |
| Checked by | The host, after policy and required approval have authorized the request, before provider invocation |
| Survives restart | The window is durable state, keyed by `task_id` |

### 4.2 Authoritative action-validation and count order

Every action request follows this single order; no other protocol section may
move repeated-action evaluation ahead of authorization:

1. Validate the request shape and resolve the registered capability/version.
2. Evaluate policy, including the task risk ceiling, automation context, data
   class, and scope. A denied request returns a typed denial, emits the applicable
   denial event, and does not change the repeated-action counter.
3. Obtain any required approval. `APPROVAL_REQUIRED` suspends without counting
   an execution attempt; a denied, expired, or absent grant does not consume the
   counter.
4. Check duplicate suppression. A previously completed duplicate returns its
   prior result and does not consume the repeated-action execution count.
5. Check the per-task repeated-action bound. If allowed, atomically record this
   authorized attempt in the counter immediately before provider invocation.
6. Invoke the provider and persist its result/evidence.

This order is authoritative for action validation and repeated-action accounting.
An authorized attempt refused at the repeat bound is not invoked and is not
incremented beyond the attempted occurrence used to determine the bound.

### 4.3 On exceeding the bound

> The host does **not** silently drop the action, and it does **not** execute it
> and hope. It stops the loop and fails the task explicitly.

1. The offending action is **not** invoked. No provider call occurs.
2. The step fails with code `BOUND_EXCEEDED_REPEATED_ACTION`, carrying the
   `capability_id`, the `arguments_digest`, the occurrence count, and the bound.
3. The task stops taking steps and moves to `FAILED`
   ([Task Protocol §4.2](02-task-protocol.md#42-transitions)).
4. A `BOUND_EXCEEDED` event is emitted with `bound_name: max_identical_action_repeats`, the limit, observed occurrence count, capability id, and arguments digest. `BOUND_EXCEEDED` is the generic bound event; the `bound_name` distinguishes this from other exhausted bounds.
5. The failure is visible in the activity timeline and in the CLI. A loop that
   ends quietly is indistinguishable from a task that finished.

Silent suppression would be worse than the loop: the model would receive a
plausible-looking result for a call that never ran, and would build the rest of
its plan on it.

### 4.4 The `BOUND_EXCEEDED_*` family

Bound exhaustion is recorded as a **task-level failure reason**, not as a new
`ActionErrorKind`. The error-kind set is frozen at `serea-arch/0.1.0`
([Protocol Index §4.3](00-protocol-index.md#43-frozen-for-p0)), and this
protocol does not add a member to it.

| Code | Bound | Where recorded | Task outcome |
| --- | --- | --- | --- |
| `BOUND_EXCEEDED_MODEL_CALLS` | `max_model_calls_per_task` | `failure_reason` + `MODEL_BUDGET_EXHAUSTED` event | `FAILED` |
| `BOUND_EXCEEDED_MODEL_TURNS` | `max_model_turns_per_task` | `failure_reason` + event | `FAILED` |
| `BOUND_EXCEEDED_TOOL_CALLS` | `max_tool_calls_per_task` | `failure_reason` + event | `FAILED` |
| `BOUND_EXCEEDED_STEP_ATTEMPTS` | `max_attempts_per_step` | `ActionError` (underlying kind preserved) + event | Step ends; task `FAILED` if terminal |
| `BOUND_EXCEEDED_REPAIR_ATTEMPTS` | `max_repair_attempts` | `ActionErrorKind::VALIDATION` | Step fails |
| `BOUND_EXCEEDED_FALLBACK_DEPTH` | `max_fallback_depth` | `ActionError` + `MODEL_FALLBACK_EXHAUSTED` event | Step ends |
| `BOUND_EXCEEDED_REPEATED_ACTION` | `max_identical_action_repeats` | `ActionError.code` + `BOUND_EXCEEDED` event (`bound_name: max_identical_action_repeats`) | `FAILED` |
| `BOUND_EXCEEDED_WALL_CLOCK` | `max_task_wall_clock_ms` | `failure_reason` + event | `FAILED` |
| `BOUND_EXCEEDED_PLAN_REVISIONS` | `max_replan_revisions_per_task` | `ActionError.code` + event | Revision refused; task proceeds or fails |
| `BOUND_EXCEEDED_PENDING_APPROVALS` | `max_pending_approvals_per_task` | `blocked_reason: APPROVAL_BACKLOG` + event | `BLOCKED`, resumable |
| `BOUND_EXCEEDED_TOKEN_BUDGET` | `max_task_total_tokens` | `failure_reason` + event | `FAILED` |
| `BOUND_EXCEEDED_DAILY_SPEND` | `max_daily_spend_usd` | `failure_reason` + event | `FAILED` |
| `BOUND_EXCEEDED_PROACTIVE_PROPOSALS` | `max_proactive_proposals_per_day` | Watcher state + event | Watcher pauses to next day |
| `BOUND_EXCEEDED_CONCURRENCY` | `max_concurrent_tasks` | Queue position + event | Task stays `RECEIVED` |
| `BOUND_EXCEEDED_LEASE` | `max_lease_seconds` | `lease_expires_at` + event | Step reclaimable, reconciled |
| `BOUND_EXCEEDED_DEVICE_EVENT_RATE` | `max_events_per_minute_per_device` | Refusal frame + event | Device backs off; dependent tasks `BLOCKED` |

---

## 5. Duplicate suppression

Duplicate suppression answers a different question from §4: not "is this task
looping?" but "has this exact effect already happened?"

Per [Capability Protocol §8.3](01-capability-protocol.md#83-duplicate-detection),
before invoking a provider the host checks whether a step with the same
`(capability_id, arguments_digest)` has already completed in this task, or in
any task within the duplicate window.

| Property | Value |
| --- | --- |
| `duplicate_window_ms` | 86400000 (24 hours) |
| Scope | Global — crosses task boundaries by design |
| Checked | Before provider invocation |
| Non-matching capabilities | `side_effect_class: NONE` capabilities are **not** windowed, because suppressing a read can only return stale data and can never prevent a harmful effect |

### 5.1 Interaction with idempotency keys

The two mechanisms overlap deliberately, and the distinction matters:

| | Idempotency key ([Capability Protocol §8.2](01-capability-protocol.md#82-idempotency-key-derivation)) | Duplicate window |
| --- | --- | --- |
| Derived from | `task_id ‖ step_id ‖ capability_id ‖ capability_version ‖ canonical_json(arguments)` | `(capability_id, arguments_digest)` |
| Scope | One step, across attempts and restarts | Any task, within 24 h |
| Purpose | Crash recovery and retry safety — "this is the same call, not a new one" | Blast-radius control — "this effect already exists, do not repeat it" |
| Enforced by | Provider (`NATIVE`) or host (`EMULATED`) | Host only |

Because the idempotency key includes `task_id` and `step_id`, two different
steps in two different tasks with identical arguments produce **different**
keys. The duplicate window is the only mechanism that catches that case, which
is precisely why it is global and why it has a duration rather than being
per-task. A genuine second execution must carry a distinct `step_id` and
therefore a distinct key, and must therefore be distinguishable by
`arguments_digest` or it is a duplicate.

One narrow bypass exists, because a 24-hour blanket ban would break legitimate
re-syncs: a `SYSTEM`-requested resync may bypass the window **only** for
capabilities whose descriptor declares `idempotency_support: NATIVE`, where the
provider dedupes by key itself. Every bypass emits
`TOOL_DUPLICATE_WINDOW_BYPASSED`. There is no bypass for `EMULATED` or `NONE`
idempotency support.

### 5.2 Suppression is not rejection

This is the distinction the two halves of §4 and §5 exist to keep sharp:

| | Duplicate suppression (§5) | Repeated-action detection (§4) |
| --- | --- | --- |
| Trigger | Exact `(capability_id, arguments_digest)` match inside the window | The bound is exceeded; *n* near-identical actions |
| Result returned | The **prior** `ActionResult`, `status: DUPLICATE_SUPPRESSED`, original receipt preserved | No result — the call never ran |
| Effect | **None.** The prior effect already happened. | **None.** The call is refused. |
| Step outcome | **Succeeds.** The task continues. | **Fails**, and the task fails |
| User experience | Invisible; the user gets what they asked for | A visible failure explaining what repeated |
| Event | `CAPABILITY_DUPLICATE_SUPPRESSED` | `BOUND_EXCEEDED` (`bound_name: max_identical_action_repeats`) |

A suppressed duplicate is a *success*: the user asked for the state, and the
state is what they got. Refusing it would be wrong — "send this again" after
losing the receipt is not a request to fail. A repeated action that is *not*
byte-identical has no prior result to return, and returning one would be a lie,
so it fails instead.

---

## 6. Timeouts and deadlines

### 6.1 Deadline propagation

The host resolves one deadline per call and passes it down. It is never a number
the model supplied ([Capability Protocol §4.2](01-capability-protocol.md#42-host-resolved-fields)).

```
effective_deadline_ms = min(
    capability_descriptor.max_duration_ms,
    model_call_deadline_ms,               // default 30000
    remaining_task_wall_clock_ms           // active time remaining (§6.2)
)
```

A deadline never extends the remaining task budget, and a step's deadline never
exceeds it. A task with 4 s of active time left issues no 15 s provider call; it
fails with `BOUND_EXCEEDED_WALL_CLOCK` and records what it knew, per
[Task Protocol §5](02-task-protocol.md#5-execution-rules).

### 6.2 The wall-clock bound measures active time

`max_task_wall_clock_ms` bounds **host work**, not elapsed calendar time. The
task clock is **paused** in `WAITING_APPROVAL`, `WAITING_USER`, `WAIT_SCHEDULE`,
and `BLOCKED`, and **running** in `PLANNING`, `READY`, `EXECUTING`, and
`VERIFYING`.

Without this rule the two frozen defaults would contradict each other: an
approval request is valid for 30 minutes
([Approval Protocol §2.2](05-approval-protocol.md#22-expiry-of-the-request))
while a task's wall clock is 15. A task would fail for having patiently waited
on a human — which would make the approval protocol unusable and would punish
the user for not being present. Waiting time is bounded separately, by the
approval's own `expires_at`, and an approval that expires fails the step
explicitly as `EXPIRED`, not `DENIED`.

### 6.3 Deadline exceeded on an effecting capability reconciles

A `PROVIDER_TIMEOUT` on a capability whose `side_effect_class != "NONE"` means
the provider may or may not have effected the change. The host therefore
**reconciles rather than retries**:

1. The call is abandoned; the step records `PROVIDER_TIMEOUT` evidence with the
   descriptor's `max_duration_ms` and the elapsed time.
2. The host attempts **read-back** through a read-only capability, using the
   stored `provider_reference` or a natural-key lookup
   ([Capability Protocol §6.2](01-capability-protocol.md#62-the-ambiguous-rule)).
3. Confirmed occurred → synthesize a receipt and continue. Confirmed absent →
   close the attempted step as `RECONCILED_ABSENT`; if `replay_safety` permits
   another execution, create a new `StepId` and derive a new idempotency key.
   The replacement step passes policy and any required approval again. Crash
   replay of the same still-ambiguous step keeps its original key; it is never
   a new attempt against an uncertain effect. Still unknown → task to `BLOCKED`
   with `blocked_reason: AMBIGUOUS_EFFECT` and a human resolves it.
4. Retrying blind would convert one uncertain effect into two certain ones. A
   generous timeout never changes this; `max_attempts_per_step` counts the
   reconciliation attempts, not the effect.

A deadline on a `side_effect_class: NONE` capability is uninteresting: there is
no effect to reconcile, the call is simply retried under
`max_attempts_per_step`.

---

## 7. Cost bounds

### 7.1 Token budgets

| Bound | Default | Scope | Note |
| --- | --- | --- | --- |
| `max_output_tokens_per_call` | 2048 | per-call | Set on `ModelRequest.max_output_tokens` |
| `max_task_total_tokens` | 128000 | per-task | Sum of input and output across all calls, including repair and fallback |
| `max_daily_spend_usd` | 5.00 | global, per calendar day | Derived from `cost_class` and the provider price table |

Token accounting is not advisory. The counter increments inside the same
transaction that records usage, so a crash cannot lose a call from the budget.

### 7.2 The `model_usage` table

Every model call writes one row, per
[Model Protocol §9](03-model-protocol.md#9-usage-accounting-and-budgets). This
is the data every cost bound is computed from.

| Column | Type | Purpose |
| --- | --- | --- |
| `usage_id` | `EventId` | `evt_` + ULID; never reused |
| `task_id` | `TaskId` | `null` for maintenance calls, which charge to the global budget only |
| `model_id` | `ModelId` | Which model actually served the call |
| `purpose` | enum | `PLANNING`, `CHAT`, `EXTRACTION`, `ANALYSIS`, `PROACTIVE`, `STRUCTURED_REPAIR` |
| `input_tokens` | integer | Counted, not estimated |
| `output_tokens` | integer | Counted, not estimated |
| `cost_usd` | decimal | From the configured price table, not from provider billing |
| `cost_class` | enum | `FREE`, `LOW`, `PAID` |
| `latency_ms` | integer | Wall time of the call |
| `repair_attempts` | integer | Repair calls on this step, for cost attribution |
| `fallback_from` | `ModelId` | `null` unless this call was a fallback |
| `recorded_at` | timestamp | Host clock |

Rows are append-only and are retained past task deletion: the counters and
non-identifying metrics survive where the task body does not
([Task Protocol §8](02-task-protocol.md#8-task-retention-and-privacy)). A row
references `task_id`; deleting the task nulls the reference and keeps the row.

### 7.3 Exhaustion fails; it never silently degrades

When a cost bound is reached, the task fails with `BOUND_EXCEEDED_TOKEN_BUDGET`
or `BOUND_EXCEEDED_DAILY_SPEND`, and the run stops. It does **not**:

- swap to a cheaper model to stretch the remaining budget;
- shorten output and pretend the result is complete;
- retry with a shorter prompt, which hides the fact that the budget was spent;
- continue against a local model "for free" without asking.

Silently changing the model would break the determinism guarantee in
[Model Protocol §6](03-model-protocol.md#6-model-routing) — same durable state
and same inputs must produce the same routing decision — and it would mean a
task's capability and cost profile depended on a budget the user cannot see.
Asking is cheap; an unannounced downgrade is not.

---

## 8. Bounds are not security policy

> A generous bound does not authorize anything. A strict policy does not make an
> unbounded loop acceptable. These are two different jobs and neither is a
> substitute for the other.

| | Bounds (this document) | Policy ([Policy Protocol](04-policy-protocol.md)) |
| --- | --- | --- |
| Question answered | "How much of this may happen?" | "May this happen at all?" |
| Governs | Quantity: calls, tokens, time, retries, concurrency | Authority: risk class, scope, grants, automation context |
| Set by | Host configuration and durable policy | Host policy rules and admin configuration |
| Exceeded | Work stops, explicitly | Nothing happens, permanently, for that request |
| Failure mode it prevents | Cost, runaway loops, unbounded blast radius | Unauthorised effects, escalation, capability creep |
| Model influence | None | None ([Policy Protocol §6](04-policy-protocol.md#6-what-policy-may-and-may-not-depend-on)) |

Three consequences that hold together:

1. **A generous bound never authorizes an unapproved action.** Raising
   `max_tool_calls_per_task` from 24 to 240 does not make the 25th
   `gmail.send` permissible. Policy still decides every call, and approval is
   still required
   ([Approval Protocol §4](05-approval-protocol.md#4-grant-evaluation)). A bound
   is a counter, not a permission.
2. **A strict policy never makes an unbounded loop acceptable.** A policy that
   denies everything does not excuse an agent that retries 10,000 times against
   the denial — each attempt still consumes calls, tokens, and money, and still
   produces user-visible timeline noise. A system that cannot say "enough"
   eventually reaches the limit that stops it for it.
3. **They compose, and the order matters.** Policy and any required approval
   run before repeated-action accounting; denied or pending requests do not
   consume its execution count. Bounds then stop authorized work that was
   otherwise permitted. Neither layer can be skipped by the other, so a
   bug in one is contained by the other. That containment is the point of
   keeping them separate documents.

---

## 9. Exhaustion is always explicit

> No bound may be exhausted silently. If the user did not hear about it, the
> host did not stop.

Every exhaustion produces all three of the following, as durable records:

| Artifact | Where it lands | What it carries |
| --- | --- | --- |
| An **event** | The activity timeline, the device notification stream, the audit trail | `evt_` + ULID, bound name, limit, observed value, task id, timestamp |
| A **structured error or reason** | The step's `ActionError.code`, or the task's `failure_reason` / `blocked_reason` | The `BOUND_EXCEEDED_*` code, never a free-text string |
| A **task-level outcome** | The task's durable state | `FAILED`, or `BLOCKED` where the condition is genuinely resumable |

What "explicit" rules out:

- **Truncation without record.** `max_output_tokens_per_call` truncates, and
  truncation is recorded as `finish_reason: LENGTH`
  ([Model Protocol §9](03-model-protocol.md#9-usage-accounting-and-budgets)). A
  truncated structured response then fails validation rather than being
  partially accepted — the host does not "best effort" a plan into shape
  ([Model Protocol §7.2](03-model-protocol.md#72-hard-constraints-on-repair)).
- **Dropping a frame without an event.** A refused device frame is counted and
  the device is told; the device backs off rather than retrying blindly.
- **Suppressing a duplicate silently.** A suppression returns the prior result
  and emits `CAPABILITY_DUPLICATE_SUPPRESSED`. It is invisible to the user and
  explicit in the record.
- **Queueing without visibility.** A task waiting on `max_concurrent_tasks`
  stays in `RECEIVED` with a queue position, and the device shows it as queued.
- **Pausing a watcher silently.** `max_proactive_proposals_per_day` pauses the
  watcher with an event. The user learns that Serea stopped volunteering.

A caller that hits a bound receives a **typed refusal naming the bound**, not a
generic failure. "The task exceeded `max_identical_action_repeats` (2) on
`calendar.events.list`" is actionable; "something went wrong" is not, and an
unexplained failure is indistinguishable from a bug.

---

## 10. Invariants summary

| # | Invariant |
| --- | --- |
| B1 | The host decides when the loop stops; the model's only termination lever is proposing `COMPLETE`. |
| B2 | No bound is model-settable, model-readable, or model-influenced. |
| B3 | This table is the complete bound set; a bound enforced anywhere else is a bug. |
| B4 | Per-task ceilings may only tighten: `effective_bound = min(global_default, task_ceiling)`, never the reverse. |
| B5 | Raising a bound is an audited admin action; lowering one is always permitted and immediate. |
| B6 | A bound of `0` disables the dimension; a negative bound is a startup configuration error, never clamped. |
| B7 | Repeated-action detection is keyed on `(capability_id, arguments_digest)`, scoped per task, and durable across restart. |
| B8 | The validation order is shape/registry, policy, required approval, duplicate suppression, repeated-action bound, then provider; denied, pending, expired, or duplicate-suppressed requests do not consume the repeated-action execution count. |
| B9 | Exceeding `max_identical_action_repeats` refuses the call, fails the task with `BOUND_EXCEEDED_REPEATED_ACTION`, and emits `BOUND_EXCEEDED` with `bound_name: max_identical_action_repeats`. |
| B10 | Bound exhaustion is a task-level failure reason; no `ActionErrorKind` is added, because that set is frozen. |
| B11 | Identical `(capability_id, arguments_digest)` within the duplicate window produces at most one effect; a suppressed duplicate returns the prior result and preserves its receipt. |
| B12 | Suppression succeeds silently to the user and is explicit in the record; repeated-but-not-identical actions fail explicitly. |
| B13 | Deadlines propagate host-resolved, never extend the task budget, and an exceeded deadline on an effecting capability reconciles rather than retries. |
| B14 | The task wall-clock bound measures active time and pauses in waiting states, so waiting on a human is not a bound violation. |
| B15 | Cost-bound exhaustion fails the task; it never silently switches model, shortens output, or substitutes a free model. |
| B16 | Bounds limit quantity; policy limits authority. A generous bound authorizes nothing, and a strict policy does not excuse an unbounded loop. |
| B17 | Every exhausted task bound produces an event, a structured error or reason, and a durable task outcome — or it did not happen. |
| B18 | Scheduler catch-up is capped per wake; remaining due occurrences stay durable and are processed by later bounded wakes. |