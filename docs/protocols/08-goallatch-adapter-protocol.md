# GoalLatch Adapter Protocol

Protocol ID: `PROTO-GOALLATCH` · Surface: `serea.goallatch/1` · Status: **FROZEN for P0**

This protocol defines the only sanctioned seam between Serea Core and GoalLatch /
Local MCP. P0 freezes the contract only; no GoalLatch provider is implemented or
registered. P15 is planned to implement `FakeGoalLatchProvider` as a deterministic,
in-memory, offline test double. A real adapter does not exist, and real integration
is out of scope for this document and for P0–P16; any later real phase requires
separate explicit authorization after the readiness gate in §9 passes.

---

## 1. Boundary intent

> **Core principle:** GoalLatch is a delegate, not a hub. Serea Core remains the
> only system that decides what work exists and what authority it carries.

The most consequential thing this document must prevent is GoalLatch becoming
Serea's central orchestrator — a failure mode that presents itself as
convenience ("let the goal runner keep the plan", "let GoalLatch own the
lifecycle") and that Serea can never undo without a rewrite.

Serea Core owns all of the following, permanently: assistant task state and the
task state machine ([Task Protocol §4](02-task-protocol.md#4-task-state-machine));
step planning, sequencing, leases and recovery
([Task Protocol §3](02-task-protocol.md#3-taskstep)); model routing and budgets
([Model Protocol §6](03-model-protocol.md#6-model-routing)); policy evaluation
([Policy Protocol §4](04-policy-protocol.md#4-rule-evaluation)); approval and grant
evaluation ([Approval Protocol §4](05-approval-protocol.md#4-grant-evaluation));
the capability registry
([Capability Protocol §10](01-capability-protocol.md#10-capability-registry));
memory extraction and retention; the proactive watcher; device sessions and the
activity timeline; and event ordering and the audit trail.

GoalLatch will own exactly one thing: local-PC and code development work — the
execution of a goal on the host machine. That is a substantial contribution, and
it is why the integration is worth building. It is not a contribution to
orchestration. This mirrors the ownership split frozen in
[Task Protocol §2](02-task-protocol.md#2-assistanttask): a Serea `AssistantTask`
is not a GoalLatch `Goal`, and a goal is reachable from Serea only as an opaque
handle behind the `host.goal.*` family. The boundary is adapter- and
protocol-based, never crate- or type-based; Serea does not embed, fork or share a
database with GoalLatch. Every question about GoalLatch behaviour must therefore
be answerable from *this document plus observed runtime output* — anything that
requires GoalLatch's source belongs in the §9 gate, not in a design discussion.

---

## 2. Forbidden couplings

> **Core principle:** Serea knows that a goal exists. Serea does not know what a
> goal *is*.

Each item below is a place where the two systems would otherwise fuse. All are
prohibited in every phase, including after a real adapter exists.

1. **No GoalLatch internal Rust types.** No import, re-export, alias or
   pattern-match on a type declared in a GoalLatch or Local MCP crate. A
   GoalLatch struct in a Serea signature means the build is wrong, not the
   design.
2. **No `local_mcp::*` path, at any depth.** `local_mcp::goal::*`,
   `local_mcp::task::*` and every other `local_mcp::*` path are forbidden. The
   dependency graph of Serea Core contains no Local MCP edge.
3. **No direct reads of GoalLatch storage.** Serea must not open, stat, lock,
   mount or copy any GoalLatch database file, nor hard-code its schema. The one
   narrow exception is host workspace state GoalLatch itself hands over as a
   result artifact; Serea stores what it was given and never hunts for more.
4. **No assumption that the running Local MCP v3 schema matches GitHub
   `main`.** The running binary is authoritative; the repository branch is a
   hypothesis. A checked-in schema, README, docstring or release note is not the
   contract.
5. **No shared types between the fake and a real adapter.** They may exchange
   only the data types in §3.1. No helper crate, trait-object alias, convenience
   wrapper or test fixture may be imported by both.
6. **No goal-handle parsing.** `GoalHandle` is an opaque string. Serea reads no
   structure into it, derives no date from it, branches on no prefix, and uses it
   to look up nothing.
7. **No bypass of the capability path.** No internal function, admin command or
   event handler reaches GoalLatch without `CapabilityRegistry` →
   `PolicyEngine` → approval → provider. `HostGoalProvider` is called from
   exactly one place: the shim in §3.2.
8. **No GoalLatch-derived authority.** Nothing GoalLatch reports about *who* may
   act, *what was approved* or *what is permitted* is authoritative. GoalLatch
   states facts about goals; Serea decides authority.

---

## 3. `HostGoalProvider` interface

```rust
#[async_trait]
pub trait HostGoalProvider: Send + Sync {
    /// The adapter's own identity. Not a registered capability namespace:
    /// no `CapabilityDescriptor` may begin with `goallatch.`.
    fn provider_id(&self) -> ProviderId {
        ProviderId::new("goallatch")
    }

    fn implementation_id(&self) -> ImplementationId;

    async fn start(&self, request: &ActionRequest, ctx: &ProviderContext)
        -> Result<ActionResult, ActionError>;
    async fn status(&self, request: &ActionRequest, ctx: &ProviderContext)
        -> Result<ActionResult, ActionError>;
    async fn run(&self, request: &ActionRequest, ctx: &ProviderContext)
        -> Result<ActionResult, ActionError>;
    async fn cancel(&self, request: &ActionRequest, ctx: &ProviderContext)
        -> Result<ActionResult, ActionError>;
    async fn result(&self, request: &ActionRequest, ctx: &ProviderContext)
        -> Result<ActionResult, ActionError>;

    async fn health(&self) -> ProviderHealth { /* default: Ready */ }
}
```

All five methods take the standard
[`ActionRequest`](01-capability-protocol.md#4-actionrequest) and return the
standard [`ActionResult`](01-capability-protocol.md#5-actionresult) or
[`ActionError`](01-capability-protocol.md#6-actionerror). There is no
goal-shaped request type and no goal-shaped result type: the types in §3.1 travel
*inside* `arguments` and `output` and are validated by the descriptor's JSON
Schema like everything else.

### 3.1 Adapter data types

These five types are the entire vocabulary shared between a fake adapter and a
real one. Nothing else crosses.

| Type | Shape | Meaning |
| --- | --- | --- |
| `GoalHandle` | newtype over `String` | The provider's opaque handle for a goal. Minted by the adapter, never by Serea, never parsed by Serea. |
| `GoalObservedState` | enum | What the host *observed*, not GoalLatch's own lifecycle enum. A state the adapter cannot map is an error, never a guess. |
| `GoalEvidenceRef` | `evidence_id` + `payload_reference` | One host-observed observation backing the outcome. |
| `GoalArtifactRef` | `name` + `media_type` + `byte_length` + `payload_reference` | An opaque content-addressed handle. Serea stores what it is given; it does not open the host filesystem for more. |
| `GoalSummary` | `String` | Host-written one-line outcome. Length-bounded by schema, redacted before egress, never raw remote prose. |

`GoalObservedState` has exactly six variants and is frozen exactly as the
[`ActionErrorKind`](01-capability-protocol.md#61-frozen-actionerrorkind-set) set
is frozen in the capability protocol: `PENDING` (exists, not begun), `RUNNING`
(executing now), `WAITING_APPROVAL` (blocked on a human decision), `COMPLETED`
and `FAILED` (terminal, as observed by the host), and `CANCELLED` (terminal,
deliberately stopped). A real adapter maps GoalLatch's state machine onto these
six and may not add variants. A state it cannot map is reported as
`ActionErrorKind::PROVIDER_ERROR`, `retryable: false`, code
`GOAL_STATE_UNRECOGNISED`, and moves the Serea task to `BLOCKED` with
`blocked_reason: UNRECOGNISED_STATE`. It is never silently coerced into
`RUNNING`.

### 3.2 Provider registration

`HostGoalProvider` is **not** a
[`CapabilityProvider`](01-capability-protocol.md#9-provider-interface) and
registers no descriptors. A thin registry shim does, and the shim is the only
caller of the trait: `host.goal.start` → `start`, `host.goal.status` → `status`,
`host.goal.run` → `run`, `host.goal.cancel` → `cancel`, `host.goal.result` →
`result`. All five descriptors carry `provider_id: "host"`.

The split exists because the capability namespace and the adapter identity are
different things. `host` is the registered namespace and follows
[Protocol Index §3](00-protocol-index.md#3-capability-identifier-grammar), where
the first segment must equal the registering provider's `ProviderId`;
`goallatch` is the adapter identity used to select and configure the
implementation. No `CapabilityDescriptor` in this repository carries `goallatch`
as its first segment, and adding one is prohibited rather than merely
discouraged.

### 3.3 Invariants

1. Every method honours the `CapabilityDescriptor` it is registered against. A
   provider that cannot honour its declared descriptor fails closed as
   `Degraded`, exactly as the capability protocol requires.
2. `ctx.credential_handle()` is `None` for every `goallatch` call, so the
   provider receives no ambient authority and no secret.
3. Every effecting method returns a real receipt. `SUCCEEDED` with
   `side_effect_class != NONE` and a null receipt is a host invariant violation
   that aborts the task to `BLOCKED`.
4. Exactly one `goallatch` implementation is registered at a time; two providers
   advertising the same `CapabilityId` would make receipts unattributable and
   are refused at startup.

---

## 4. The `host.goal.*` capability family

Five capabilities, and no more. A goal is reached by starting it and then
progressing it; there is no "create and run in one call", because collapsing
them would make the approval boundary unplaceable.

| `CapabilityId` | `risk_class` | `replay_safety` | `side_effect_class` | `required_authorization` | `data_class` |
| --- | --- | --- | --- | --- | --- |
| `host.goal.start` | `EXTERNAL_WRITE` | `CONDITIONAL` | `EXTERNAL_WRITE` | `SCOPED_GRANT` | `PERSONAL` |
| `host.goal.status` | `OBSERVE` | `IDEMPOTENT` | `NONE` | `NONE` | `PERSONAL` |
| `host.goal.run` | `EXTERNAL_WRITE` | `CONDITIONAL` | `EXTERNAL_WRITE` | `SCOPED_GRANT` | `PERSONAL` |
| `host.goal.cancel` | `EXTERNAL_WRITE` | `CONDITIONAL` | `EXTERNAL_WRITE` | `SCOPED_GRANT` | `PERSONAL` |
| `host.goal.result` | `OBSERVE` | `IDEMPOTENT` | `NONE` | `NONE` | `PERSONAL` |

`host.goal.cancel` is `EXTERNAL_WRITE`: it changes the state of a process
running on the user's machine, and it does not reverse work already committed
by that goal. Cancellation is not an inverse for completed delegated effects;
see [Task Protocol §7](02-task-protocol.md#7-cancellation). Every
`required_authorization` is `SCOPED_GRANT`, never `NONE`, for the three effecting
verbs: the policy engine may deny before approval is raised, but no policy rule
can turn an effecting goal call into an automatic allow
([Policy Protocol §4.3](04-policy-protocol.md#43-additional-standing-rules)).
`cost_class` is `PAID` on all five, even under the fake, because a real delegated
goal consumes local model compute; declaring it now means the descriptor needs no
re-versioning when the real adapter arrives, which is what makes §10 cheap.

The remaining descriptor fields are fixed for all five: `version` `1.0.0`,
`provider_id` `host`, `root_requirement` `NOT_REQUIRED`, `experimental` `true`;
`implementation_id` is `fake-goallatch` for the planned P15 fake; any future real
implementation would require separate explicit authorization after the §9 gate; `idempotency_support` is `NATIVE` for `start`, `run` and `cancel`
and `NONE` for `status` and `result`; `max_duration_ms` is `start` 15000,
`status` 5000, `run` 300000, `cancel` 10000, `result` 10000. A full descriptor,
for `host.goal.run`:

```json
{
  "id": "host.goal.run",
  "version": "1.0.0",
  "title": "Advance a delegated host goal",
  "description": "Advances a goal previously started through host.goal.start. The goal executes on the host machine under GoalLatch control. Serea observes progress and outcomes; it performs no part of the work itself.",
  "provider_id": "host",
  "implementation_id": "fake-goallatch",
  "input_schema": { "$ref": "https://serea.local/schemas/host.goal.run.input.1.0.0.json" },
  "output_schema": { "$ref": "https://serea.local/schemas/host.goal.run.output.1.0.0.json" },
  "side_effect_class": "EXTERNAL_WRITE",
  "risk_class": "EXTERNAL_WRITE",
  "required_authorization": "SCOPED_GRANT",
  "replay_safety": "CONDITIONAL",
  "data_class": "PERSONAL",
  "root_requirement": "NOT_REQUIRED",
  "idempotency_support": "NATIVE",
  "max_duration_ms": 300000,
  "cost_class": "PAID",
  "experimental": true
}
```

> **Core principle:** These five descriptors define the complete `host.goal.*`
> contract. Before P15, no GoalLatch provider is available and the capabilities
> are unavailable. In P15, the offline fake may back them for deterministic
> testing; that does not authorize real host work or a real adapter.

---

## 5. Delegation model

A delegated host goal is an `AssistantTask` of kind `DELEGATED_HOST_GOAL`
([Task Protocol §2](02-task-protocol.md#2-assistanttask)) whose `policy_class` is
host-assigned when the task is created and immutable for that task. An ordinary
capability approval cannot raise the ceiling; broader authority requires a
separate explicit user request creating a new task. Its execution path is the path in
[Capability Protocol §1](01-capability-protocol.md#1-core-principle), in full:

```
Delegated plan
  -> Structured ActionRequest          (host.goal.run, schema-pinned)
  -> Schema validation                 (fail closed)
  -> Capability Registry lookup        (host.goal.run @ 1.0.0 must be enabled)
  -> Policy Engine                     (task policy_class ceiling checked)
  -> Approval decision                 (SCOPED_GRANT, task-bound, single-use)
  -> Provider invoke                   -> HostGoalProvider::run
  -> Evidence / Receipt                (host-observed, not model-reported)
  -> Durable state update              (persisted before advancing)
```

A `host.goal.run` request on that path, in full:

```json
{
  "request_id": "req_01JQ8ZA4H6NFG8K2M6RTV9XCWB",
  "task_id": "tsk_01JQ8Z9K3M7QWXR4V2T6YH0BNA",
  "step_id": "stp_01JQ8Z9M3R2CVN8H5FWK7PQDM4",
  "capability_id": "host.goal.run",
  "capability_version": "1.0.0",
  "arguments": { "goal": "fakegoal_1" },
  "arguments_digest": "sha256:7c9e1b4d6f8a0c2e4b6d8f0a2c4e6b8d3f5a7c9e1b4d6f8a0c2e4b6d8f0a2c4e",
  "idempotency_key": "idk_4b6d0f8a3c5e9b1d7f2a4c6e8b0d3f5a7c9e1b4d6f8a0c2e4b6d8f0a2c4e6b8d",
  "data_class": "PERSONAL",
  "requested_by": "USER",
  "deadline_ms": 300000
}
```

`arguments` carries the `GoalHandle` and nothing else. The goal's objective was
fixed once, in the `host.goal.start` arguments, and a `host.goal.run` cannot
widen or rewrite it: a model cannot smuggle new intent past the descriptor's
`input_schema` by parking it in a later call.

There is no second path, no privileged entry point, and no "the task is already
delegated, so the check is redundant" exemption. A delegated goal is a
`DELEGATE`-kind step ([Task Protocol §3](02-task-protocol.md#3-taskstep)) like any
other and obeys
[invariants T6 and T7](02-task-protocol.md#9-invariants-summary) exactly as every
other step does. `requested_by` on a `host.goal.*` request is `USER`, `SCHEDULER`
or `SYSTEM`; it records provenance and confers nothing
([Capability Protocol §4.1](01-capability-protocol.md#41-requested_by)). A plan is
built the way any plan is built — the model proposes, the host validates,
persisting the plan changes no policy decision
([Task Protocol §4.3](02-task-protocol.md#43-planning-rules)) — so a model may
propose a delegation and name the capability, but may not select the
`implementation_id`, the `risk_class`, the `required_authorization`, the
`deadline_ms`, or whether approval is needed.

### 5.1 `codex_allowed`

`codex_allowed` defaults to `false` and is a task-level setting, exactly as
frozen in [Model Protocol §8](03-model-protocol.md#8-codex-exclusion). This
protocol adds nothing and relaxes nothing:

1. A `DELEGATED_HOST_GOAL` task does not imply `codex_allowed = true`. The
   default is `false` and the setting is not derived from task kind.
2. Setting it to `true` requires an explicit durable policy setting and emits
   `POLICY_CHANGED`. It is not settable by model output and not settable from
   the Android client — the same two exclusions the model protocol imposes.
3. A future Codex-mediated delegated use, if separately authorized, remains a
   GoalLatch adapter concern and never a ModelRouter route or ordinary failure
   fallback. Before P15, no GoalLatch implementation is available; P15 provides
   only the offline fake. A real adapter remains unscheduled and requires separate
   explicit phase authorization after the §9 gate passes.
4. Because the model router cannot reach `codex` under any input, a model cannot
   route itself to codex by proposing a delegation. Delegation changes what
   capability the goal *would* use; it changes nothing about who authorizes it.

If a delegated goal's approval is granted, the grant authorizes exactly one
`host.goal.run` at one version, in that one task, for that one scope
([Approval Protocol §3.1](05-approval-protocol.md#31-the-six-bounds)). It is
not standing authority to delegate again, and it does not survive the task.

---

## 6. `FakeGoalLatchProvider` specification

`FakeGoalLatchProvider` is specified here as the planned P15 implementation of
`HostGoalProvider`. P0 freezes this contract only; no provider is implemented or
registered. P15 will exercise the delegation path — planning, approval,
invocation, receipt, cancellation, recovery and result reporting — without a
network, a GPU, a checkout or a wall clock.

> **Core principle:** The fake returns real results. A test double that returns
> structurally-simplified results tests nothing, and the bugs it hides are the
> ones that matter.

### 6.1 Configuration

The fake is constructed from host configuration, never from model output:

```json
{
  "implementation_id": "fake-goallatch",
  "scenario": "ApprovedCompletion",
  "scenario_binding": "DELEGATED_TASK",
  "clock": { "epoch": "2026-10-01T00:00:00.000Z", "follows_wall_clock": false },
  "io": { "network": "DISABLED", "filesystem": "DISABLED", "subprocess": "DISABLED" },
  "credential_handle": null
}
```

`scenario_binding: DELEGATED_TASK` means the named scenario is bound to the
`TaskId` at the moment of the first `host.goal.start` and written to durable task
state. Later calls read the binding from durable state, not from this
configuration, so changing the configuration mid-flight cannot change a goal's
behaviour. That is what makes restart-replay tests meaningful.

### 6.2 The three scenarios

Exactly three scenarios exist. They are selected by configuration and are the
complete set of behaviours the delegation path must handle.

| Scenario | Terminal state | Approval step | Terminal error | Total fake elapsed |
| --- | --- | --- | --- | --- |
| `ApprovedCompletion` | `COMPLETED` | yes, at `status` | none | 1500 ms |
| `MidRunFailure` | `FAILED` | no | `GOAL_EXECUTION_FAILED` | 1450 ms |
| `OperatorCancellation` | `CANCELLED` | no | none | 1050 ms |

**(a) `ApprovedCompletion` — Start → Running → Approval → Completed**

| # | Clock | Call | `status` | Observed state | Receipt | Duration |
| --- | --- | --- | --- | --- | --- | --- |
| 1 | `0 ms` | `host.goal.start` | `SUCCEEDED` | `PENDING` | yes | 200 ms |
| 2 | `200 ms` | `host.goal.run` | `SUCCEEDED` | `RUNNING` | yes | 600 ms |
| 3 | `800 ms` | `host.goal.status` | `SUCCEEDED` | `WAITING_APPROVAL` | no | 50 ms |
| 4 | `850 ms` | `host.goal.run` | `SUCCEEDED` | `RUNNING` → `COMPLETED` | yes | 600 ms |
| 5 | `1450 ms` | `host.goal.result` | `SUCCEEDED` | `COMPLETED` | no | 50 ms |

At step 3 the host sees `WAITING_APPROVAL`, moves the task to `WAITING_APPROVAL`,
and raises an `ApprovalRequest`
([Approval Protocol §2](05-approval-protocol.md#2-approvalrequest)) bound to the
current task. Step 4 is a *new* `stp_` with a *new* `idempotency_key`, because a
step that must genuinely be executed twice carries a distinct `StepId`
([Capability Protocol §8.2](01-capability-protocol.md#82-idempotency-key-derivation)).
The human approval happens at `850 ms` and does **not** advance the fake clock:
approval is host work, and the clock advances only on fake work.

**(b) `MidRunFailure` — Start → Running → Failed**

| # | Clock | Call | `status` | Observed state | Receipt | Duration |
| --- | --- | --- | --- | --- | --- | --- |
| 1 | `0 ms` | `host.goal.start` | `SUCCEEDED` | `PENDING` | yes | 200 ms |
| 2 | `200 ms` | `host.goal.run` | `SUCCEEDED` | `RUNNING` | yes | 600 ms |
| 3 | `800 ms` | `host.goal.run` | `FAILED` | `FAILED` | no | 600 ms |
| 4 | `1400 ms` | `host.goal.result` | `SUCCEEDED` | `FAILED` | no | 50 ms |

Step 3 returns `ActionErrorKind::PROVIDER_ERROR`, `code: GOAL_EXECUTION_FAILED`,
`retryable: false`, `host_action: NONE`. A failed goal is an *observable
outcome*, not an exception: step 4 still succeeds, because reading a terminal
failure is a successful read. This is the case most often implemented wrongly,
and the fake exists to make it impossible to get wrong.

**(c) `OperatorCancellation` — Start → Running → Cancelled**

| # | Clock | Call | `status` | Observed state | Receipt | Duration |
| --- | --- | --- | --- | --- | --- | --- |
| 1 | `0 ms` | `host.goal.start` | `SUCCEEDED` | `PENDING` | yes | 200 ms |
| 2 | `200 ms` | `host.goal.run` | `SUCCEEDED` | `RUNNING` | yes | 600 ms |
| 3 | `800 ms` | `host.goal.cancel` | `SUCCEEDED` | `CANCELLED` | yes | 200 ms |
| 4 | `1000 ms` | `host.goal.result` | `SUCCEEDED` | `CANCELLED` | no | 50 ms |

`host.goal.cancel` carries a receipt because it has
`side_effect_class: EXTERNAL_WRITE`. A cancelled goal keeps every receipt already
produced; cancellation never implies undo
([Task Protocol §7](02-task-protocol.md#7-cancellation)).

### 6.3 Determinism

The clock is fixed at epoch `2026-10-01T00:00:00.000Z` and advanced only by
fake work, so `duration_ms` is a scripted constant per step and never measured.
Goal handles come from a monotonic counter in the shared simulated-service
ledger, formatted `fakegoal_<n>` starting at `<n> = 1`. `output_digest` is computed over the
canonical JSON of `output` per
[Protocol Index §5](00-protocol-index.md#5-serialization), so it is byte-stable
across runs. The fake harness provides a deterministic simulated external-service ledger.
It is shared by provider instances across reconstruction of Serea Core in the
test harness, so the harness can model the crash-after-effect/before-receipt
window: a repeat call with the same key returns the stored `ActionResult` and
does not perform a second simulated effect. This ledger is test-harness state,
not provider process state: `FakeGoalLatchProvider` itself is not process-durable,
and the contract makes no claim that an in-memory provider instance survives a
process crash. The harness ledger uses no network, filesystem, or subprocess.
The fake introduces no concurrency — calls on one goal serialise, calls on
distinct goals are independent.

### 6.4 `fakegoal_<n>`

`fakegoal_<n>` is a **provider-namespace** handle, not a Serea identifier, and the
only handle in the repository that is parseable. It exists because a test
asserting `fakegoal_1` is more useful than one asserting a random opaque string,
and because the fake is explicitly a test double. Its parseability does not leak:
Serea stores it in `SideEffectReceipt.provider_reference` and passes it back
verbatim, no Serea code branches on it, and the cross-restart identity of a goal
comes from the persisted `idempotency_key`, not from the handle. A real adapter's
goal handles are opaque and §3.3 rule 6 applies to them in full.

The goal counter belongs to the simulated-service ledger and is shared across
Core reconstruction within the test harness, preserving handle identity across
the simulated crash window. It is not durable across termination of the test
harness itself and does not represent provider process durability. Persisted
receipts may short-circuit a retry when present; the harness ledger separately
covers the critical effect-before-receipt window.

### 6.5 Isolation and receipts

The fake module depends on `core`, `alloc`, Serea's own data types and the
deterministic clock. It opens no socket, no file and no subprocess, and holds no
`CredentialHandle`. Its `ProviderContext` carries an IO capability set that is
never armed, so an accidental future network call fails at compile time rather
than in a sandboxed test that quietly succeeds.

The fake produces real `ActionResult` values: a populated `status`, a
schema-valid `output`, a computed `output_digest`, an `evidence` array, and a
`SideEffectReceipt` with `receipt_id`, `capability_id`, `idempotency_key`,
`provider_reference` (the `GoalHandle`), `effect_summary`, an `observed_at` drawn
from the fake clock, and `replay_safe` taken from the descriptor. The
receipt/duplicate-suppression path is therefore exercised in every fake test
rather than bypassed — a fake returning `receipt: null` would let a
receipt-handling regression ship green.

---

## 7. Result and evidence contract

A delegated goal's outcome comes back through `host.goal.result`, whose
`side_effect_class` is `NONE` and whose output is schema-pinned:

```json
{
  "goal": "fakegoal_1",
  "state": "COMPLETED",
  "summary": "Applied the schema change and the two failing-test fixes; full suite green.",
  "evidence": [
    {
      "evidence_id": "evt_01JQ8ZP4R7TX2M9KPQ5RD8WCSN",
      "kind": "GOAL_RESULT",
      "observed_at": "2026-10-01T00:00:01.450Z",
      "payload_reference": "sha256:4b6d0f8a3c5e9b1d7f2a4c6e8b0d3f5a7c9e1b4d6f8a0c2e4b6d8f0a2c4e6b8d"
    }
  ],
  "artifacts": [
    {
      "name": "changes.diff",
      "media_type": "text/x-diff",
      "byte_length": 20481,
      "payload_reference": "sha256:1a7e4b6d0f8a3c5e9b1d7f2a4c6e8b0d3f5a7c9e1b4d6f8a0c2e4b6d8f0a2c4e"
    }
  ],
  "execution_root": null
}
```

| Field | Obligation |
| --- | --- |
| `goal` | The `GoalHandle` returned by `host.goal.start`, echoed unparsed. |
| `state` | The `GoalObservedState`. A non-terminal state is legal here; it means the goal is still going. |
| `summary` | `GoalSummary`, host-written and length-bounded. Never raw remote prose, never model-edited. |
| `evidence` | Zero or more `GoalEvidenceRef` records. Every `evidence_id` is an `EventId` per [Protocol Index §2](00-protocol-index.md#2-identifier-grammar); this family introduces no new prefix. |
| `artifacts` | Opaque `GoalArtifactRef` handles into Serea's own content-addressed store. Serea stores what it is given and does not enumerate the host workspace for more. |
| `execution_root` | The worktree or directory the goal ran in, or `null` when the adapter cannot establish one. Unestablished is reported as `null`, never guessed. |

Two rules are absolute. First, **a model-reported "done" is never accepted**: a
`DELEGATED_HOST_GOAL` task reaches `COMPLETED` only through a `VERIFY`-kind step
whose input is a `host.goal.result` that parsed against the descriptor's
`output_schema` and whose `state` is `COMPLETED`. A model turn asserting success,
a goal summary saying "finished", and a `ModelResponse` claiming the work is
done are all ignored for this purpose. The capability protocol states the rule
generally — the provider produces the proof, the model produces the intent — and
this family is where it is tested hardest.

Second, **`GOAL_RESULT` evidence must be host-observed**: the record is produced
by the provider invocation, carries `produced_at` from the execution environment
rather than from the model's turn, and is referenced by the receipt path. A
`GOAL_RESULT` record originating from model output is invalid and raises
`MODEL_SCHEMA_VIOLATION`
([Capability Protocol §4.2](01-capability-protocol.md#42-host-resolved-fields)).
`GOAL_RESULT` is a member of the frozen evidence-kind vocabulary owned by the
[Capability Protocol §7](01-capability-protocol.md#7-evidence). The Event
Protocol owns `SereaEvent.kind`; it does not own evidence kinds.

---

## 8. Cancellation and timeout semantics

Cancelling a Serea task that has delegated a goal is a two-step operation,
because the in-flight call and the remote work are different things.

| Situation | Host behaviour |
| --- | --- |
| Task cancelled while `host.goal.run` is in flight | The in-flight call is aborted at the deadline boundary, cooperative, per [Task Protocol §5](02-task-protocol.md#5-execution-rules). The step records whatever is known. |
| Task cancelled while no `host.goal.run` is in flight | A new `DELEGATE`-kind step issues `host.goal.cancel`, taking its own policy, approval, lease and receipt. |
| Task cancelled after the goal reached `COMPLETED` | The task is `CANCELLED`; the goal's receipts and evidence are retained. Nothing is undone. |

The remote work is never abandoned silently. If the host does not know whether
the goal is still running, it must find out via `host.goal.status` — a read
capability needing no approval, which is exactly why it exists:

| `host.goal.status` reports | Host decision |
| --- | --- |
| `RUNNING` or `PENDING` | Issue `host.goal.cancel` as a new step. |
| `WAITING_APPROVAL` | Cancel the goal; a pending approval for a cancelled task is withdrawn, not left to expire. |
| `COMPLETED`, `FAILED`, `CANCELLED` | Terminal. The task is `CANCELLED`; the goal's own outcome is recorded as evidence. |
| `ActionErrorKind::AMBIGUOUS` | Propagate. Task → `BLOCKED`, `blocked_reason: AMBIGUOUS_EFFECT`, human resolution required. |

**Ambiguous cancellation** is the case this protocol is most careful about.
`host.goal.cancel` is `CONDITIONAL`, not `IDEMPOTENT`: a goal may be mid-write,
and a timeout followed by a blind re-cancel can leave the host machine in a state
neither system can describe. Therefore: the host must never automatically
re-issue `host.goal.cancel` after an `AMBIGUOUS` result, and the rule in
[Capability Protocol §6.2](01-capability-protocol.md#62-the-ambiguous-rule)
applies without relaxation. Reconciliation is a read-back through
`host.goal.status` and `host.goal.result`, which are `IDEMPOTENT` and safe to
repeat. Confirmed running → cancel is now a fresh decision requiring a fresh
`StepId` and a fresh `idempotency_key`; confirmed terminal → record and stop;
still unknown → `BLOCKED` with `AMBIGUOUS_EFFECT`. The host never guesses which
of the three it is.

**Timeouts** are host-side and host-owned. `deadline_ms` on the `ActionRequest`
is resolved from the descriptor's `max_duration_ms` and is never model-supplied
([Capability Protocol §4.2](01-capability-protocol.md#42-host-resolved-fields)).
Exceeding it cancels the call and yields `ActionErrorKind::PROVIDER_TIMEOUT` with
`retryable: true`. Because `host.goal.run` is `CONDITIONAL`, that `retryable`
flag does **not** authorise a blind re-invocation: the host first reconciles via
`host.goal.status` and re-invokes only if the goal is confirmed not to have
progressed, as a **new** `StepId` carrying a **new** `idempotency_key`.
Re-using a key to mean "try again" is prohibited
([Capability Protocol §8.2](01-capability-protocol.md#82-idempotency-key-derivation)).
The per-step attempt ceiling from
[Capability Protocol §8.4](01-capability-protocol.md#84-per-step-attempt-ceiling)
bounds all of this; exhausting it fails the task with the last error preserved.

---

## 9. Real adapter readiness gate

> **Core principle:** Documentation is a claim about a contract. The running
> binary is the contract. Integration begins from observation, never from a
> README.

No real adapter may be written until all six verifications below are complete,
each from a live artifact and not from a description. P15 implements and tests
the fake adapter journey; P16 records pre-integration readiness and stops. A real
adapter is neither scheduled nor authorized by P0–P16 and requires a separate
explicit future-phase authorization after the gate is green.

| # | Verification | Live artifact required |
| --- | --- | --- |
| 1 | Running Local MCP binary version and build SHA | `local-mcp --version` output plus the commit the binary reports, captured from the machine that will actually run it. |
| 2 | GitHub `local-mcp` `main` SHA | The current commit SHA of the upstream default branch, recorded independently of the binary. |
| 3 | Actual `tools/list` schema | The MCP `tools/list` response fetched from the **running binary** — not a checked-in schema file, a generated client, or a docs example. |
| 4 | Managed-worktree execution-root status | Direct observation of whether a goal actually runs in a managed worktree, what the execution root is, and whether the adapter creates it or merely assumes it. |
| 5 | `goal_result` and evidence contract | A real goal's terminal payload, showing what it returns as evidence, what it returns as artifacts, and what a human can verify from it. |
| 6 | Session authority behaviour | Which session owns a goal, whether authority transfers between sessions, and what happens to a goal across a host restart. |

**Documentation must not be used to infer any of the six.** A README, a
docstring, a changelog entry, a release note, a GitHub issue or a checked-in JSON
schema is a hypothesis about the contract. It is admissible as a *question*,
never as an *answer*. Verifications 1 and 2 are recorded separately and compared:
a binary SHA that is not on `main` is a normal, healthy state — a fork, a local
build, a pinned release — and what would be a problem is not knowing. Verification
3 supersedes every published schema: the schema the running binary reports is the
schema the adapter is written against, even where it contradicts the repository,
and divergence is recorded rather than reconciled silently.

**If the runtime contract and the source contract differ, integration does not
begin.** The two are recorded side by side, the divergence is written up as an
ADR under `docs/decisions/` per
[Protocol Index §7](00-protocol-index.md#7-change-control), and the question of
which is authoritative is settled before any adapter code is written. Since the
running binary is by definition what Serea will call, the practical answer is
usually the binary — but "usually" is not a procedure, and the ADR is. A
verification that cannot be performed is a failed verification: there is no
partial pass. If the binary cannot be built, the schema cannot be retrieved, or
the evidence contract cannot be observed, the gate is red and the fake remains
the only implementation. The six records together are the adapter's
specification; until they exist, the fake is not a stand-in for "a simple
implementation", it is the only described one.

---

## 10. Version and compatibility policy

Three version axes apply, and this protocol does not collapse them
([Protocol Index §4](00-protocol-index.md#4-versioning)).

| Axis | Value | Governs |
| --- | --- | --- |
| Architecture version | `serea-arch/0.2.0` | The whole contract set, including this document. |
| Capability version | `1.0.0` on all five `host.goal.*` descriptors | Each capability's input/output contract. |
| Wire surface | `serea.goallatch/1` | The adapter-internal hop from `HostGoalProvider` to its backing GoalLatch transport. |

The `serea.action/1` surface is **unchanged** by GoalLatch integration. A
delegated goal is an ordinary capability call wrapped in an ordinary envelope;
nothing about it is special on the wire. `serea.goallatch/1` exists only between
an adapter and GoalLatch, only the adapter speaks it, and no Serea component
outside that implementation has any awareness that the surface exists.

| Change | Effect |
| --- | --- |
| A new real adapter ships | New `implementation_id` (`mcp-goallatch`). Same five `CapabilityId`s, same descriptor versions `1.0.0`, same `risk_class`, `required_authorization`, `replay_safety`, `data_class`. |
| Registry switchover | Host configuration plus restart. Exactly one implementation active. Audited as an admin-plane event. |
| The fake is retired | Removed from the build's registration path. It is never shadow-running beside a real adapter; two providers for one `CapabilityId` would make receipts unattributable. |
| A real adapter cannot honour `1.0.0` output | Descriptor version bump to `1.1.0` or `2.0.0` with an ADR and, for a major, an architecture version bump and a migration note. |
| A new `host.goal.*` capability | Architecture-**minor**: an ADR, plus a new verb only if the verb is not already in the frozen verb set. |
| Removing or renaming a `host.goal.*` capability | Architecture-**major**, with a deprecation window, because it changes the `ProviderId` namespace set. |

The design intent behind keeping descriptors identical across implementations is
that a swap from fake to real is a configuration change with zero contract
change. If that ever stops being true, the descriptors were wrong, and the fix is
an ADR — not an exception carved out for the real adapter.

---

## 11. Invariants summary

| # | Invariant |
| --- | --- |
| G1 | `FakeGoalLatchProvider` performs no network access, no filesystem access and no subprocess execution; its module depends only on `core`, `alloc`, Serea data types and the deterministic clock. |
| G2 | A delegated host goal crosses the full normal path — validation, registry, policy, approval, provider — with no bypass of any stage. |
| G3 | No `local_mcp::*` path and no GoalLatch-internal Rust type appears anywhere in Serea's dependency graph or public signatures. |
| G4 | A real adapter may not be written until all six §9 verifications are complete from live runtime and live source; a verification that cannot be performed is a failed verification. |
| G5 | The fake produces real `ActionResult` values and real `SideEffectReceipt` records, so the receipt and duplicate-suppression path is exercised rather than bypassed. |
| G6 | A model cannot self-authorize codex through delegation: `codex_allowed` defaults `false`, is not settable by model output, and is not settable from the Android client. |
| G7 | A goal is reachable from Serea only through the five `host.goal.*` capabilities; no other path to GoalLatch exists in the codebase. |
| G8 | A model-reported "done" is never accepted as a goal outcome; `COMPLETED` requires a schema-valid `host.goal.result` read. |
| G9 | `GOAL_RESULT` evidence is host-observed; evidence originating from model output is invalid and raises `MODEL_SCHEMA_VIOLATION`. |
| G10 | A `GoalHandle` is opaque. Serea never parses one, with `fakegoal_<n>` as the single declared exception scoped to the test double. |
| G11 | Cancellation never implicitly undoes host work; a cancelled goal's receipts and evidence are retained, and remote state is established by `host.goal.status` before any cancel is issued. |
| G12 | An `AMBIGUOUS` cancel or run is never blindly retried; it reconciles through `IDEMPOTENT` reads or moves the task to `BLOCKED` with `AMBIGUOUS_EFFECT`. |
| G13 | Exactly one `goallatch` implementation is registered at a time, and no `CapabilityDescriptor` carries `goallatch` as its first segment. |
