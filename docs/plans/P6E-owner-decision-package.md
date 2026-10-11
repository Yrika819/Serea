# P6E TaskPlan / WAIT_APPROVAL owner decision

Status: **OPTION A ACCEPTED BY OWNER (2026-10-11)** · Baseline: `e5eb44babe9bd84ba914f020f37d657e4b5caab5` · PR #10 remains Draft and open · See [ADR-0041](../decisions/ADR-0041-p6-initial-approval-gates.md)

## Decision requested

The current P6E request path requires a durable `WAIT_APPROVAL` Step before the
requested Capability Steps. The public TaskEngine API cannot create that mixed
plan, and an unused wait cannot be left `PLANNED` after Policy returns ALLOW.
Storage's authority checks remain correct and must not be relaxed.

**Recommendation: approve option A below.** Add a narrow initial mixed-plan
builder that persists host-owned approval-gate Steps and bound Capability Steps
in one complete Plan revision. When current Policy says `ALLOW` and all external
gates (including root approval, task ceiling, capability overlay and PROACTIVE
restrictions) permit continuation, TaskEngine may complete the corresponding
host-owned wait Step as `SUCCEEDED` with the canonical result document
`{"outcome":"NOT_REQUIRED"}`. Persist its result blob/reference and the normal
audited Step transition atomically. This writes no approval request, lifecycle
event, Grant, grant use, or fabricated human response. It never marks a
Capability Step successful.

**Owner decision:** Option A and the exact semantics above were approved on
2026-10-11. Option B and C remain rejected alternatives. No new StepStatus or
Plan-order relaxation is authorized.

## Source-grounded execution and authority map

| Stage | Source and behavior | Authority boundary |
| --- | --- | --- |
| Plan construction | `crates/serea-task-engine/src/engine.rs:439-748` exposes the capability-only API as a wrapper over the new initial mixed-plan builder. The builder prepares every Capability Step with `prepare_action`, builds host gate inputs from exact StepId/arguments-digest membership, and submits a complete Plan plus descriptor bindings. | Capability creation uses the P5 pinned registry snapshot and canonical binding path. A generic unbound Step is never mistaken for a Capability Step. |
| Durable Plan | `crates/serea-storage/src/task.rs:397-429` offers `put_plan_revision` and `put_capability_plan_revision`, both entering the same complete-plan writer. `task.rs:448-490` validates all inputs, requires exactly one descriptor binding for each new Capability Step, forbids bindings for non-capability Steps, and validates binding membership. `task.rs:503-555` preserves exact existing Steps, rejects duplicate/noncanonical revisions and low sequence values, and atomically writes the Plan, Step rows, blobs, bindings, Task state and audit. | The specialized writer can preserve mixed kinds in one revision without weakening Storage. Existing append-only Plan history remains intact. |
| Binding invariants | The mixed-plan writer's `CapabilityPlanBindingDraft` loop requires every new Capability Step to have exactly one binding; `bind_step_capability` resolves against the Task-pinned generation. `WAIT_APPROVAL` is a non-capability kind and receives no binding. `TaskStep` kind validation and the migration 0004 registry triggers enforce the capability-shaped fields and immutable binding. | A WAIT Step has no provider, capability, version, idempotency key, receipt, or registry binding. It is host control flow only. |
| Policy evaluation | `crates/serea-task-engine/src/engine.rs:23-155` reloads the durable Task and Step; checks current plan revision, exact input digest, pinned generation, immutable binding and descriptor; loads the active Policy revision and overlay; derives `PolicyInputDraft`; returns evidence only. Destructive enablement is false because no accepted durable source exists. | This is point-in-time evidence, not execution authority. Policy ALLOW cannot override task ceiling, overlay, root approval, PROACTIVE, or handoff restrictions. |
| Approval request | `engine.rs:155-243` checks that every enumerated requested action has matching `REQUIRE_APPROVAL` evidence, then joins Storage, TaskJournal and EventBus. `crates/serea-storage/src/approval.rs:503-648` rechecks active Policy identity and Task revision, requires one pending request at most, finds the minimum requested Capability sequence, requires exactly one still-PLANNED matching gate before it, and atomically transitions Task/Step, persists the R2 action set, audit and `APPROVAL_REQUIRED`. | R2 permits 1–8 explicitly enumerated, compatible actions; request membership is immutable and never inferred from scope/digest alone. The leading member is event routing identity only. |
| Authenticated response | `crates/serea-policy/src/approval.rs:360-445` creates `AuthenticatedApprovalResponseV1` only after the trusted verifier and checks identity, session, time, elevated confirmation, expiry and the enumerated subset. `crates/serea-storage/src/approval.rs:509-515` applies it through the durable response transaction. | Only authenticated human/admin response creates a Grant. ALLOW must not call this path or synthesize a Grant. |
| Scheduler wake | `crates/serea-scheduler/src/runtime.rs:471-487` parses committed lifecycle events and materializes routing-only wakes; `runtime.rs:605-625` forwards pending wakes to TaskEngine. Storage `scheduler.rs:714-806` deduplicates and reads wakes without consuming them. | Scheduler routes; it does not decide approval or grant authority. |
| Wake application | `crates/serea-task-engine/src/engine.rs:318-329` delegates to `Tx::apply_approval_lifecycle_wake`. Storage `scheduler.rs:857-1035` re-reads request, Grant, Task, membership and waiting Step; atomically completes/fails the wait, updates the Task, writes audit/events and acknowledges the wake. | Event kind alone grants nothing. A grant wake returns the Task to READY for fresh evaluation; no Capability Step is marked successful. |
| Resume | The caller must rerun `evaluate_prepared_action` against current durable state before any future P8 dispatch. | Policy/overlay/revocation/deadline changes during the wait remain authoritative. P6 never invokes a provider or mints a `RequestId`. |

## Frozen-contract findings

1. **Mixed initial persistence is implemented through the public TaskEngine API.**
   `create_mixed_capability_plan` retains every Step kind and binds each new
   Capability Step in the same transaction. The capability-only API delegates
   through the same preparation/writer path.
2. **No insertion escape exists in the current Plan contract.**
   `task.rs:503-511` rejects a new Step whose sequence is at or below historical
   high-water. ADR-0018 §5 says revisions may append higher-sequence Steps or
   remove unstarted PLANNED Steps; they do not renumber or insert. Its named
   `insert_at` relaxation is explicitly future work, not current permission.
3. **WAIT authority is structurally absent.** A WAIT Step is a host instruction;
   it cannot carry provider fields, an immutable capability binding, a Grant,
   receipt, or dispatch authority. Storage request validation currently requires
   exactly one still-PLANNED wait before the first requested action, so mixed
   plans need deterministic gate grouping and cannot leave multiple candidate
   waits ahead of one approval unit.
4. **The accepted Step lifecycle has no SKIPPED terminal.** ADR-0018 §2 has
   seven statuses. `SUCCEEDED` requires a real host-written result document and
   its durable RESULT blob/reference (§3). This can encode a typed
   `NOT_REQUIRED` fact without claiming a human response, but current accepted
   text does not explicitly assign that outcome to `WAIT_APPROVAL`.
5. **R2 batching constrains gate placement.** A request set must contain only
   actions whose shared task/capability/version/plan/generation/descriptor
   conditions agree and whose evidence is `REQUIRE_APPROVAL`. ALLOW actions
   cannot be silently included in that set. The Plan builder must place one
   gate before each independently resolvable approval unit and ensure an unused
   gate cannot remain ahead of a later request.
6. **A second approval in the same Task is possible.** Each gate must be
   consumed or durably resolved before a later request. Storage's exact-one
   check correctly rejects ambiguous multiple PLANNED gates before the first
   requested Capability Step.

## Alternatives

| Design | Authority and Plan invariants | Schema / migration / ADR impact | Restart, R2 and P8 impact | Complexity |
| --- | --- | --- | --- | --- |
| **A. Initial mixed Plan + audited `NOT_REQUIRED` result (recommended)** | No new authority. Existing immutable bound Capability Steps and append-only sequences stay unchanged. WAIT remains host-owned and never gains provider authority. Adds an explicit non-human completion result to the approval-gate lifecycle. | No table or migration change is needed if the result uses existing `SUCCEEDED` fields, canonical result blob/reference, audit and journal. Requires owner ratification because the WAIT outcome is new/ambiguous under current prose; ADR-0018 status set remains unchanged. | Result and audit are durable before READY/replay. Must plan one gate per approval unit; only compatible R2 members are requested. P8 still re-evaluates fresh Policy and consumes grants only with dispatch intent. | Lowest code/schema cost; requires careful gate grouping and explicit ALLOW/REQUIRE/HANDOFF/DENY behavior. |
| **B. Dynamic WAIT insertion before a wholly-PLANNED suffix** | Keeps WAIT meaning limited to actual human approval, but relaxes append-only revision/order rules. A safe implementation must prove every shifted Step is still PLANNED and preserve old revision blobs and stable Step identity. | No table necessarily required, but this is a direct change to ADR-0018 §5 and the accepted Plan revision contract; owner ratification is required. Must not be approximated by raw SQLite writes. | No unused gates; supports R2 by inserting one gate before the chosen compatible group. On restart, the new complete revision is authoritative. P8 must use the resulting revision and revalidate. | More Plan-history, sequence and concurrent-replan cases; broader contract change than A. |
| **C. Task-level approval wait without a physical WAIT Step** | Removes the Step-level lifecycle link and changes which record represents the wait. Storage's exactly-one-Step guard must be replaced with a different authoritative invariant. | Changes accepted Task/Approval/Storage semantics and likely audit schema or Task event model; migration impact depends on how wait identity is persisted. Requires explicit owner decision. | Could express R2 membership directly at Task/request level, but request-to-Task recovery and multiple sequential requests need new durable identity. P8 still needs exact Step membership. | Broadest authority-model and recovery redesign; not recommended. |
| **D. Refuse tasks that need approval after planning** | Preserves all current Plan and Step semantics; no new authority. It cannot fulfill the approved approval workflow for ordinary model-proposed capability Steps. | No schema change, but contradicts the P6 objective and supported approval protocol. | R2 and wake implementation become unreachable through normal TaskEngine use; no P6E closure. | Small implementation, unacceptable product behavior. |

## Test and security obligations after the owner choice

For option A, the RED-first suite must first fail on the missing public mixed
builder and on ALLOW leaving a PLANNED wait, then cover:

- Mixed initial Plan order, unique Step IDs/sequences, canonical revision and
  immutable P5 descriptor binding for each Capability Step.
- WAIT Step field absence and no provider/binding/receipt/Grant authority.
- Exact-one gate selection across successive R2 approval units; invalid or
  ambiguous ordering and extra/unlisted actions fail closed.
- ALLOW persists only `{"outcome":"NOT_REQUIRED"}` plus its result blob and
  audited Step transition; it creates no approval row, event, Grant or use and
  does not alter Capability Step status.
- REQUIRE_APPROVAL, DENY, REQUIRE_HANDOFF, mandatory Root approval,
  PROACTIVE restrictions, disabled overlay and task ceiling remain unchanged.
- Single and multi-action R2 requests; mixed ALLOW/REQUIRE_APPROVAL partitions;
  incompatible group, altered digest and unenumerated member refusals.
- Request/Task/WAIT/event atomic rollback, authenticated response replay,
  Scheduler duplicate wake and restart before response / after response / during
  wake, cancellation and deadline boundaries, policy revision change, overlay
  disable and revocation before re-evaluation.
- Independent Store connections, fault injection, close/reopen checks, privacy
  sentinels and zero provider calls, RequestIds, dispatch intents or external
  effects.

For option B or C, first ratify the corresponding Plan-order or task-level
authority change, then derive the test suite against that chosen contract.
Neither design may bypass canonical Storage APIs or alter migrations 0001–0008.

## Current verification notes

The focused approval wake regression initially failed because the P6E wake wrote
`result_digest` without a RESULT blob reference. The working-tree correction now
stores the canonical result blob and reference in the same transaction; the
regression is in `crates/serea-storage/src/migration_0005_tests.rs`. This is
independent of the owner decision above. The public mixed-plan API, R2 group
builder checks, ALLOW → audited `NOT_REQUIRED`, REQUIRE_APPROVAL request,
authenticated response, event-to-wake materialization, restart around response
and wake, TaskEngine wake application, policy revision change, overlay disable,
post-wake reevaluation, and mixed ALLOW / mandatory-root approval across two
ordered WAIT gates have focused coverage. The added
`crates/serea-scheduler/tests/task_engine_approval_e2e.rs` traverses the public
`Scheduler::process_approval_lifecycle_wakes` path, restarts with an unacknowledged
wake, confirms the exact R2 grant member and digest, re-evaluates current Policy,
and proves zero provider invocations. Exact-head CI must cover all subsequent
source changes before P6E closure.

## P6E closure record

**`P6E_TASK_ORCHESTRATION_VERIFIED`.** The owner-authorized Option A path is
closed. Public mixed-plan creation, ALLOW → durable `NOT_REQUIRED`,
REQUIRE_APPROVAL → atomic Task/request/WAIT/event state, authenticated response,
R2 Grant membership, public Scheduler wake routing, TaskEngine wake application,
and fresh Policy reevaluation passed the integration and recovery checks at
source commit `798c483c98ab0adaeb48e66030d9b1673087aae7`. The P6E code was then
included unchanged in the later exact-head P6F verification run. No Grant,
provider invocation, dispatch intent, `RequestId`, or external effect is created
by P6E.

The owner decision is recorded in [ADR-0041](../decisions/ADR-0041-p6-initial-approval-gates.md).
P6F separately adds concurrent Policy activation/snapshot, approval/expiry,
cancellation/approval, distinct R2-member consumption, and Grant-consumption /
revocation cases. Existing P6B–P6E attack, fault-injection, restart/reopen,
redaction, credential refusal, Plan-order, and zero-provider tests remain part
of the closure matrix. Software crash/reopen tests do not prove physical
power-loss behavior.
