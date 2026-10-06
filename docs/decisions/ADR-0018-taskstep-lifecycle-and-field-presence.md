# ADR-0018: TaskStep Lifecycle, Step-Status Set, and Field Presence

- Status: **Accepted** — wire/lifecycle architectural decision; P2A wire validation implemented, runtime deferred
- Architecture version: `serea-arch/1.0.0` (current frozen contract set)
- Decision date: 2026-10-03 — owner direction in the P2A reconciliation request
- Recorded by: P2 design preparation, from `007f038af19ae7855ad00b7e58389ce04d0fe727`
- Feeds: [P2 contract gap analysis](../plans/P2-contract-gap-analysis.md) §5.1,
  §5.1b, §5.2, §5.10

> Accepted on owner ratification after three corrected documentation gate reviews
> GREEN. P2A implementation includes checked wire construction/deserialization
> and schema parity. Generation decoding preserves raw numeric tokens and checks
> positive-u32 membership exactly, without f64 rounding. The coordinator records
> final workspace/MSRV validation, review and integration status in the closure
> record; earlier test counts are not final-tree evidence. Storage constraints, plan mutation and
> unknown-status execution blocking remain later-phase obligations, not accepted
> runtime implementation. See [frozen gate](../plans/P2A-review-and-closure.md).

## Context

At the P1 baseline, `TaskStep` in `crates/serea-protocol/src/types.rs` declared `idempotency_key`,
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
- and, for the five non-capability-shaped kinds, no `provider_id`,
  `capability_id`, `capability_version`, or `idempotency_key` to derive.

That baseline shape could not represent the state the frozen protocol mandates. The
only way to construct a conforming "unstarted" step then was to write a
fabricated timestamp and a fabricated digest, which is a lie in a durable audit
record and is precisely the class of defect the P1 review passes rejected.

Separately, `StepStatus` is an open code validated only as `^[A-Z][A-Z0-9_]*$`.
An engine needs a closed set to make transitions exhaustive and compile-checked,
and `PlanRevision` — a Crate Map §3 public type — has no definition anywhere in
the repository.

## Decision

### 1. One flat wire object, four fields become optional

`TaskStep` keeps its single-object shape. Only `idempotency_key`, `result_digest`,
`started_at` and `completed_at` become `Option`. `input_digest` remains required;
`provider_id`, `capability_id` and `capability_version` are already `Option`. Presence
becomes a checked invariant of a single `StepPresence` value that every checked
construction and deserialisation path routes through. Unlike
`CapabilityDescriptor`'s `serde(try_from = Draft)` attribute, `TaskStep` implements
`Deserialize` manually: decode `TaskStepDraft`, replace any draft decode error
with `invalid task step draft` without formatting rejected input, then call
`StepPresence::new`. The error boundary precedes presence validation; unchecked
draft deserialization is not independently claimed to sanitize errors.

The implemented public construction boundary is `TaskStepDraft` → `TaskStep`:
`TaskStep::new(draft)` / `TaskStep::try_from(draft)` route through
`StepPresence::new(draft)`; checked presence may also be converted into `TaskStep`.
Both validated wrappers keep their state private. Read-only dereferencing exposes
the draft fields, but there is no public mutation or `DerefMut` bypass. To change
a step, convert it to a draft and revalidate. Unknown extension members round-trip;
an extension key equal to any reserved step member is refused, including when
that optional member is absent, so flattening cannot smuggle in unchecked fields.
The generation field remains `Option<u32>`; its wire decoder admits JSON
integer-valued numeric spellings such as `1`, `1.0` and `1e0` in the positive u32
domain, matching schema integer admission. Missing/null yield None; zero,
fractions and overflow refuse. This is distinct from SCJ-1 canonical admission,
which refuses decimal/exponent spellings even when integer-valued.

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

P2A parses and round-trips unknown well-formed wire `StepStatus` codes. Known
statuses receive the presence matrix checks; step-kind invariants and validation
of every supplied value apply even for unknown statuses. The later engine must
never execute or persist an unknown status in its closed lifecycle: it blocks the
task with `UNRECOGNISED_STATE`. P2A does not implement that execution behavior.

**No `SUPERSEDED` status is added.** A revision that drops an unstarted step
deletes the row instead, because the previous plan's full content is retained as
a content-addressed blob referenced by the `plan_revisions` row. A step ever
leased or executed is never deleted; it is closed under its real status. The
status set stays at seven with no loss of auditability.

### 3. The presence matrix

`R` = required and non-null, `N` = absent (`None`), `-` = optional but validated.
On the wire, missing and explicit `null` both represent `None`; serialization
omits `None`. SQL `NULL` represents absence except for generation: SQL `0` maps
to wire `None`, positive generation maps to `Some(u32)`. Never emit wire `0`.
Refuse generation overflow above `u32::MAX`; never wrap, clamp or truncate.
The seven unconditional fields are `step_id`, `task_id`, `sequence`, `kind`,
`status`, `attempt`, `input_digest`.

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
| `lease_generation` (wire) | N | ≥1 | ≥1 | ≥1 | ≥1 | ≥1 | ≥1 |
| `side_effect_receipt` | N | N | N | N | - | N | N |
| `error` | N | N | N | N | N | R | N |

**Matrix intersection:** `side_effect_receipt` is **N for every non-capability
kind** (`MODEL_TURN`, `WAIT_APPROVAL`, `WAIT_USER`, `WAIT_SCHEDULE`, `NOTIFY`) on
**all statuses, including unknown codes**. The optional `SUCCEEDED` cell applies
only to `CAPABILITY`/`DELEGATE`/`VERIFY`. Non-capability steps cannot carry external
action semantics; this kind invariant is enforced before known-status validation
in Rust and independently in the task schema. Test obligation: for each of the
five kinds, refuse a supplied receipt on every known status and an unknown status
in both construction/deserialization and schema validation; missing/null receipts
remain absent and serialize by omission.

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
- **`side_effect_receipt` is free on capability-shaped `SUCCEEDED`** and required when the step
  declared an effecting `side_effect_class`. P2 cannot evaluate that condition —
  the class lives in a descriptor and there is no Capability Registry in P2 — so
  P2 wire validation enforces the known-status half it can: `receipt ⇒ SUCCEEDED`, and
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

## Ratified wire amendment and deferred runtime amendment to Task Protocol

The wire clauses below belong to the atomic P2A delivery; append-only runtime
plan revision behavior is a proposed design obligation for the green P2F gate,
not P2A code or an accepted runtime implementation.

1. §3 defines the seven known lifecycle codes and their presence matrix without
   closing the wire code domain. Unknown codes parse; kind invariants always
   apply. Runtime blocking of unknown execution is deferred to the engine.
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

## Phase-specific implementation gate

| File | Change |
| --- | --- |
| `crates/serea-protocol/src/types.rs` | The four fields become `Option`; `StepPresence` and both matrices validate construction and deserialization; `lease_generation: Option<u32>` is added; missing/null accepted and `None` omitted |
| Later P2C/P2E storage migration (not P2A) | Constraints and checked SQL-generation/wire conversion; runtime fencing remains ADR-0024 Proposed |
| `crates/serea-protocol/schemas/assistant-task.schema.json` | `$defs/step.required` reduced to the seven always-present fields; `if`/`then` clauses added for the step-kind and status matrices |
| `crates/serea-protocol/tests/{protocol_types,p2a_types,p2a_shape,p2a_parity}.rs` | Updated inline steps, checked construction, all kind/status cells, unknown-code and absence/receipt/reserved-extension boundaries |
| `crates/serea-protocol/tests/schema_contracts.rs` | The conditional requirements, and a negative case per kind |
| `docs/protocols/02-task-protocol.md` | The five amendments above |

## Compatibility

**This is a relaxation, and saying otherwise would be dishonest.**
[Protocol Index §4.1](../protocols/00-protocol-index.md#41-semantics) names "a new
optional field" as a minor change; this is four existing required fields becoming
optional, which weakens validation for a consumer that relied on it.

Historical considerations raised in the earlier minor-bump discussion:

1. `serea.task/1` has exactly one producer and one consumer in V1 — the host
   itself. No deployed consumer can begin accepting something it previously
   rejected, because there is no cross-host consumer of a task document.
2. The relaxation is not uniform. For `CAPABILITY`, `DELEGATE` and `VERIFY` the
   schema **gains** `if`/`then` clauses that make `capability_id`,
   `capability_version`, `provider_id` and `idempotency_key` conditionally
   required. Fail-closedness increases exactly where an external effect is
   reachable, and decreases only where nothing external can happen.

The owner has ratified the major bump and migration-note direction in the P2A
reconciliation request; no minor/major choice remains. **The P2 autonomous audit resolved the choice against
"minor", so what remains for the owner is the migration-note text, not the
classification.** Protocol Index §4.1 names "a new optional field" as the minor
case; this is not a new field — it is four existing required fields becoming
optional, which weakens validation for any consumer that relied on it. Protocol
Index §5 sets the precedent for exactly this shape: a rename "is a breaking change
with an alias field for one major version". The coherent plan —
`serea-arch/0.2.0 → 1.0.0`, `serea.task/1 → 2`, with ADR-0024's new optional
`lease_generation` riding along — is recorded once in
[the audit](../plans/P2-autonomous-audit.md) and
[the ledger](../plans/P2-tomorrow-decision-ledger.md).

The major is cheap **now** for a reason that will not stay true: the migration note
required by Protocol Index §7 item 4 has almost nothing to name, because
`serea-core` and the Android client are P12 and no task document has ever crossed a
host boundary. That is precisely the argument for taking the major now rather than
calling it minor — after P12 the note has real consumers and the bump is expensive.

## Consequences

- The deferred storage gate must enforce the presence matrix for all error
  shapes, not only one representative tuple per cell. Historical DDL probes against
  SQLite 3.43.2 exercised 32 selected `N`/`0` cases, not every partial-error shape. **That audit found this claim false as first written:** eight of the
  matrix's thirty-two `N`/`0` cells were accepted — `completed_at` and
  `result_digest` on `EXECUTING` and on `WAITING`, and `lease_expires_at` on
  `WAITING`, `SUCCEEDED`, `FAILED` and `RECONCILED_ABSENT`. Three additive
  constraints closed all eight in that historical corpus; all thirty-two selected
  cases were refused, all fifty-one legitimately constructible `kind × status`
  cells and all thirty-seven legal task transitions constructed. This does not
  establish single-column/partial-error absence: current DDL additionally requires
  all six error columns NULL outside FAILED and five mandatory members on FAILED,
  with optional details; dedicated probes and later runtime tests are required. See
  [P2 SQLite schema §4.4](../plans/P2-sqlite-schema.md#44-task_steps).
- The `lease_expires_at` half was the substantive one, and it is why the audit
  probed per cell rather than per row. The schema already treated `lease_owner` as
  a genuine biconditional, so the *pair* was half-constrained: a terminal step could
  carry a lease expiry with **no owner**. ADR-0024's commit statement clears both
  columns together, so no designed path produces it — but a future writer clearing
  only `lease_owner` would pass the schema. That is the hole the biconditional
  closed for one column, left open for the other.
- Two matrix rows remain **detected rather than prevented**, and saying so is part
  of the claim rather than a caveat on it. `side_effect_receipt` absent on
  `RECONCILED_ABSENT` is a cross-table property no `CHECK` can express, and is
  recovery's invariant scan. `lease_generation` above the stored value is a *floor*,
  not a ceiling: a corrupt row may carry `99`, and a `SUCCEEDED` step may name a
  generation for which no `leases` row exists, because the dependency is one-way.
- Every new `StepStatus` value a future phase adds is a new matrix row and a new
  `CHECK`, which is the intended friction: it forces the author to decide field
  presence rather than inherit a default.
- `P2` gains no new wire member beyond `lease_generation`. The other four were
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
