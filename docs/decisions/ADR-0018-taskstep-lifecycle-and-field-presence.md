# ADR-0018: TaskStep Lifecycle, Step-Status Set, and Field Presence

- Status: **Proposed** — pending implementation and owner ratification
- Architecture version: `serea-arch/0.2.0` at the time of writing
- Decision date: not yet ratified
- Recorded by: P2 design preparation, from `c3737039e3e38dbba554dc0b9075025f87948358`
- Feeds: [P2 contract gap analysis](../plans/P2-contract-gap-analysis.md) §5.1,
  §5.1b, §5.2, §5.10

> This ADR changes no frozen protocol text and no code. The amendments below are
> **drafted, not applied**. `docs/protocols/02-task-protocol.md` is untouched by
> this run, because
> [Protocol Index §7](../protocols/00-protocol-index.md#7-change-control) requires
> an ADR, an architecture-version bump, and a changelog entry to land together,
> and this run may not change code.

## Context

`TaskStep` in `crates/serea-protocol/src/types.rs` declares `idempotency_key`,
`input_digest`, `result_digest`, `started_at` and `completed_at` as non-optional,
and `assistant-task.schema.json` lists the same five in `$defs/step.required`.

[Task Protocol §4.3](../protocols/02-task-protocol.md#43-planning-rules) requires
a plan to be persisted **before** execution, and §3.1 requires `attempt` to
distinguish a crash-recovered attempt from a deliberate retry. A step that exists
in a persisted plan but has not run therefore has:

- no `started_at`, because it did not start;
- no `completed_at`, because it did not complete;
- no result, hence no `result_digest` — and fabricating one would destroy the
  field's stated purpose, "Detects result corruption or partial writes on
  recovery";
- and, for seven of the eight `StepKind` values, no `capability_id`,
  `capability_version`, or `idempotency_key` to derive.

The current shape cannot represent the state the frozen protocol mandates. The
only way to construct a conforming "unstarted" step today is to write a
fabricated timestamp and a fabricated digest, which is a lie in a durable audit
record and is precisely the class of defect the P1 review passes rejected.

Separately, `StepStatus` is an open code validated only as `^[A-Z][A-Z0-9_]*$`.
An engine needs a closed set to make transitions exhaustive and compile-checked,
and `PlanRevision` — a Crate Map §3 public type — has no definition anywhere in
the repository.

## Decision

### 1. One flat wire object, five fields become nullable

`TaskStep` keeps its single-object shape. `input_digest`, `result_digest`,
`started_at`, `completed_at` and `idempotency_key` become `Option`, and presence
becomes a checked invariant of a single `StepPresence` value that every
construction and deserialisation path routes through — the same pattern
`CapabilityDescriptor` already uses via `serde(try_from = Draft)`.

**Why not split into `TaskStep` plus `StepExecution`.** Evaluated and rejected:

1. `SideEffectReceipt` requires `capability_id` and `idempotency_key`, and
   `ActionError` nests inside the step. Splitting turns "receipt non-null
   exactly when an effect occurred" from a field-presence rule into a
   cross-object invariant.
2. `AssistantTask.steps` is `Vec<TaskStep>`. A new element type changes the wire
   shape of a frozen surface and needs a second schema document, which is a new
   named concept in the Protocol Index §1 registry.
3. Task Protocol §3 wants one object that carries enough to re-execute or verify
   after a restart. Two objects invite persisting the second without the first.

### 2. The seven-status step lifecycle

| Status | Meaning | Terminal |
| --- | --- | --- |
| `PLANNED` | Persisted from the plan. Never leased, never attempted. | No |
| `LEASED` | A lease is held; the attempt has not begun. | No |
| `EXECUTING` | An attempt is in flight. | No |
| `WAITING` | Suspended on a human, a grant, or a schedule. | No |
| `SUCCEEDED` | Completed successfully. | Yes |
| `FAILED` | Completed unsuccessfully. | Yes |
| `RECONCILED_ABSENT` | Closed as confirmed-absent by read-back ([Bounds Protocol §6.3](../protocols/10-bounds-protocol.md#63-deadline-exceeded-on-an-effecting-capability-reconciles)). | Yes |

The wire `StepStatus` stays an open code. `serea-task-engine` defines the closed
lifecycle for its own state machine, mirroring how `AssistantTask.state` is a
closed `TaskState` while `StepStatus` is a code: the closed set is an
implementation's exhaustiveness aid, not a wire narrowing.

`WAITING` is reachable **only** for `StepKind::WaitApproval`, `WaitUser` and
`WaitSchedule`. A capability step is `PLANNED` while its sibling `WAIT_APPROVAL`
step is `WAITING`, because Task Protocol §5 rule 3 and the Execution Pipeline both
require the execution lease to be released before a wait.

**`started_at` is `N` for `LEASED`, and that was a correction.** An earlier draft
of this matrix said `R`, which is wrong: `LEASED` is precisely the window in which
the lease is held and the attempt has **not** begun. Getting this wrong produced a
schema in which `LEASED` refused a `started_at` and `PLANNED` was itself
unconstructible; see [P2 SQLite schema §4.4](../plans/P2-sqlite-schema.md#44-task_steps).
`started_at` becomes required from `EXECUTING` onward.

**`attempt` increments once, at lease acquisition, and `PLANNED` keeps
`attempt = 0`.** That combination is what makes the matrix implementable as a
schema: a step that has begun an attempt is never `PLANNED` again, so a retry or an
expiry reclaim moves `LEASED`/`EXECUTING` to `LEASED` rather than back to `PLANNED`.
An earlier draft incremented `attempt` at acquisition *and* again at
`begin_attempt`, so `max_attempts_per_step = 3` bought a single attempt — which is
precisely the distinction Task Protocol §3.1 says the field exists to make.

A `StepStatus` outside the seven is never persisted and never executed: the task
moves `BLOCKED` with `blocked_reason: UNRECOGNISED_STATE`, reusing a code Event
Protocol §4 already names. That is Protocol Index §4.2 rule 5 applied at the step
boundary.

**No `SUPERSEDED` status is added.** A revision that drops an unstarted step
deletes the row instead, because the previous plan's full content is retained as
a content-addressed blob referenced by the `plan_revisions` row. A step ever
leased or executed is never deleted; it is closed under its real status. The
status set stays at seven with no loss of auditability.

### 3. The presence matrix

`R` = required, `N` = must be `null`, `-` = free but validated.

| Field | `PLANNED` | `LEASED` | `EXECUTING` | `WAITING` | `SUCCEEDED` | `FAILED` | `RECONCILED_ABSENT` |
| --- | --- | --- | --- | --- | --- | --- | --- |
| `step_id`, `task_id`, `sequence`, `kind`, `status` | R | R | R | R | R | R | R |
| `attempt` | `0` | ≥1 | ≥1 | ≥1 | ≥1 | ≥1 | ≥1 |
| `input_digest` | R | R | R | R | R | R | R |
| `idempotency_key` | §3 below | §3 | §3 | §3 | §3 | §3 | §3 |
| `provider_id` | §3 | §3 | §3 | §3 | §3 | §3 | §3 |
| `capability_id` | §3 | §3 | §3 | §3 | §3 | §3 | §3 |
| `capability_version` | §3 | §3 | §3 | §3 | §3 | §3 | §3 |
| `result_digest` | N | N | N | N | **R** | - | - |
| `started_at` | N | **N** | R | R | R | R | R |
| `completed_at` | N | N | N | N | **R** | R | R |
| `lease_owner` | N | R | R | N | N | N | N |
| `lease_expires_at` | N | R | R | N | N | N | N |
| `lease_generation` | `0` | ≥1 | ≥1 | ≥1 | ≥1 | ≥1 | ≥1 |
| `side_effect_receipt` | N | N | N | N | - | N | N |
| `error` | N | N | N | N | N | R | N |

Two rows deserve their reasoning stated.

- **`input_digest` is required in every status** because a step's input is known
  at plan time: for `CAPABILITY` it is the arguments blob, and for every other
  kind it is the host-written instruction document.
- **`result_digest` is required exactly when `SUCCEEDED`**, and every `SUCCEEDED`
  step persists a real host-written result document — for `WAIT_USER` that is
  `{"answered": true}`, for `VERIFY` the verification outcome, for `MODEL_TURN`
  the structured output. This is how the row is satisfied **without a single
  fabricated digest**. For `FAILED` and `RECONCILED_ABSENT` it is free: a failed
  attempt may or may not have persisted a partial result.
- **`side_effect_receipt` is free on `SUCCEEDED`** and required when the step
  declared an effecting `side_effect_class`. P2 cannot evaluate that condition —
  the class lives in a descriptor and there is no Capability Registry in P2 — so
  P2 enforces only the half it can: `receipt ⇒ SUCCEEDED`, and
  `RECONCILED_ABSENT ⇒ no receipt`.

### 4. Step-kind × required-field matrix

| `StepKind` | `provider_id` | `capability_id` | `capability_version` | `idempotency_key` | `input_digest` |
| --- | --- | --- | --- | --- | --- |
| `CAPABILITY` | R | R | R | **R** | R |
| `DELEGATE` | R | R | R | **R** | R |
| `VERIFY` | R | R | R | **R** | R |
| `MODEL_TURN` | N | N | N | **N** | R |
| `WAIT_APPROVAL` | N | N | N | **N** | R |
| `WAIT_USER` | N | N | N | **N** | R |
| `WAIT_SCHEDULE` | N | N | N | **N** | R |
| `NOTIFY` | N | N | N | **N** | R |

`DELEGATE` is capability-shaped because it reaches `host.goal.*` through the
same `ActionRequest` path (GoalLatch Adapter §4, §6), so the same
external-effect and same-key rules apply. `VERIFY` is capability-shaped because a
verify step reads back through a read-only capability (Bounds Protocol §6.3), so
it is an invocation whose `side_effect_class` is `NONE`, not a host-only
inspection.

`NOTIFY` is host-internal in P2: the step records that a notification is owed and
`serea-core` renders it in P12. If a later phase routes `NOTIFY` through
`device.*` capabilities, this row changes by ADR.

### 5. Plan revisions are append-only in P2

A revision may append steps at higher `sequence` values and delete `PLANNED`
steps. It may not renumber, reorder, or insert between existing sequences,
because each of those changes the meaning of Task Protocol §3.2's prerequisite
rule ("Steps with `sequence < n` that are required for step `n` must be
`SUCCEEDED`") for a step that may already have run. A revision that would drop a
step ever leased or executed is refused.

The named relaxation, when the need is demonstrated, is an `insert_at` that
renumbers only a wholly-`PLANNED` suffix. The cost is stated rather than hidden:
in P2 V1, inserting a step into the middle of a plan requires cancelling and
creating a new task.

## Proposed amendment to Task Protocol

Applied to `docs/protocols/02-task-protocol.md` only in the same commit that
implements it. Nothing here is applied by this run.

1. §3 gains a sentence after the `kind` enumeration: *"`status` takes exactly the
   seven values `PLANNED`, `LEASED`, `EXECUTING`, `WAITING`, `SUCCEEDED`,
   `FAILED`, `RECONCILED_ABSENT`. `SUCCEEDED`, `FAILED` and `RECONCILED_ABSENT`
   are terminal for the step. A `status` outside this set is never executed; the
   task moves to `BLOCKED` with `blocked_reason: UNRECOGNISED_STATE`."*
2. §3 gains an optional member: *"`lease_generation` — a monotonically
   increasing per-step counter, incremented on every lease acquisition including
   an expiry reclaim. A commit carrying a stale generation is refused. Absent on
   a step that has never been leased."*
3. §3.1's table gains three rows making the conditional requirements explicit,
   reproducing the two matrices above.
4. §4.3 gains a sentence recording the append-only revision rule and the refusal
   of a revision that would drop an executed step.
5. A §11 changelog section, created, because Protocol Index §7 item 3 requires an
   entry in the affected protocol's changelog.

## Code change, same commit

| File | Change |
| --- | --- |
| `crates/serea-protocol/src/types.rs` | The five fields become `Option`; `StepPresence` and the two matrices are added as a validating constructor plus a `serde(try_from = Draft)`-style route; `lease_generation: Option<u32>` is added |
| `crates/serea-storage/migrations/0001_initial.sql` | One constraint per matrix row, executed and verified |
| `crates/serea-protocol/schemas/assistant-task.schema.json` | `$defs/step.required` reduced to the eight always-present fields; `if`/`then` clauses added for the step-kind and status matrices |
| `crates/serea-protocol/tests/protocol_types.rs` | Every matrix cell, in both the Rust and the schema direction |
| `crates/serea-protocol/tests/schema_contracts.rs` | The conditional requirements, and a negative case per kind |
| `docs/protocols/02-task-protocol.md` | The five amendments above |

## Compatibility

**This is a relaxation, and saying otherwise would be dishonest.**
[Protocol Index §4.1](../protocols/00-protocol-index.md#41-semantics) names "a new
optional field" as a minor change; this is five existing required fields becoming
optional, which weakens validation for a consumer that relied on it.

Two facts make a minor step defensible:

1. `serea.task/1` has exactly one producer and one consumer in V1 — the host
   itself. No deployed consumer can begin accepting something it previously
   rejected, because there is no cross-host consumer of a task document.
2. The relaxation is not uniform. For `CAPABILITY`, `DELEGATE` and `VERIFY` the
   schema **gains** `if`/`then` clauses that make `capability_id`,
   `capability_version`, `provider_id` and `idempotency_key` conditionally
   required. Fail-closedness increases exactly where an external effect is
   reachable, and decreases only where nothing external can happen.

The owner must still choose between a minor bump and a major bump with a migration
note naming every consumer. This ADR does not choose. Open question 4 in the gap
analysis records it.

## Consequences

- `serea-storage` gets a constraint for every row of the presence matrix, so an
  inconsistent step is unconstructible by any writer, not only by Rust. The DDL was
  **executed** during design preparation against SQLite 3.43.2 with one insert per
  cell: 46 checks, all passing. Two matrix rows needed extra work to be genuinely
  enforced — `lease_generation >= 1` for every non-`PLANNED` step, and
  `idempotency_key` immutability — and one row, receipt absence on
  `RECONCILED_ABSENT`, is a cross-table property that no `CHECK` can express and is
  therefore detected by recovery rather than prevented.
- Every new `StepStatus` value a future phase adds is a new matrix row and a new
  `CHECK`, which is the intended friction: it forces the author to decide field
  presence rather than inherit a default.
- `P2` gains no new wire member beyond `lease_generation`. The other five were
  already members; only their requiredness changes.
- Frozen sets `StepKind` (8) and `TaskState` (11) are untouched, so the P1
  enum-pinning tests need no change.

## Rejected alternatives

| Alternative | Why rejected |
| --- | --- |
| Fabricate `started_at`/`result_digest` for an unstarted step | A durable audit record that lies; destroys `result_digest`'s stated purpose |
| A sentinel digest such as `sha256:` of the empty string | Indistinguishable from a real empty-document digest; would silently pass any check that only tests shape |
| Keep the fields required and add a separate "planned" boolean | A second source of truth for a state the `status` code already carries, and it can disagree with it |
| Split into `TaskStep` + `StepExecution` | See §1 above |
| Derive an `idempotency_key` for non-capability steps from `task_id ‖ step_id` | Reuses the one field whose meaning is "this is the same action" for actions that are not capability actions, which is what Capability Protocol §8.2 forbids |
| A reserved pseudo-capability such as `serea.host.model` | A new capability namespace owned by no registered `ProviderId`; pre-empts P5's registry (Protocol Index §3) |
| A second identifier prefix for a non-capability key | The identifier registry is frozen (Protocol Index §2) and adding a prefix requires an ADR |
| Allow renumbering on a plan revision | Changes the meaning of Task Protocol §3.2's prerequisite rule for a step that may have run |