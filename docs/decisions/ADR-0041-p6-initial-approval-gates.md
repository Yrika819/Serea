# ADR-0041: P6 initial mixed plans and approval-gate resolution

Status: **Accepted** · Date: 2026-10-11 (owner decision) · Architecture: `serea-arch/2.7.0`

Surfaces affected: P6 TaskEngine runtime semantics and Task Protocol explanatory
text. No StepStatus, wire shape, database schema, or migration changes.

## Decision

An initial TaskPlan may contain host-owned `WAIT_APPROVAL` gate Steps and
P5-bound `CAPABILITY` Steps in one complete Plan revision. Each gate identifies
its exact, explicitly enumerated action group and precedes every member of that
group. Capability Steps continue to be prepared against the Task-pinned P5
registry snapshot and receive immutable descriptor bindings through the
canonical Storage writer. Wait Steps have no provider, capability, version,
idempotency key, capability binding, receipt, or dispatch authority.

When the current Policy evaluation is `ALLOW` for every action in a gate's exact
group, and all other mandatory gates permit continuation, the host may complete
that gate through the ordinary audited Step lease, attempt, and result path. The
result is the canonical host-authored document `{"outcome":"NOT_REQUIRED"}`;
its RESULT Blob, reference, `result_digest`, and audit transition are committed
atomically. A partially allowed group is not completed this way. `NOT_REQUIRED`
records only that approval was not required at this evaluation. It is not a
human response, Grant, Grant use, or successful Capability Step. No approval
request or approval lifecycle event is created for this outcome.

`REQUIRE_APPROVAL` uses the authenticated approval request/response and durable
Scheduler wake path. R2 requests preserve exact StepId and arguments-digest
membership, with 1–8 individually eligible actions. Groups may not absorb an
`ALLOW`, `DENY`, `REQUIRE_HANDOFF`, or otherwise incompatible member. Each
request must select the sole still-PLANNED gate explicitly covering the full
requested set; ambiguous or stale gate membership fails closed. Previously
completed gates are immutable. A gate made unnecessary by policy resolution is
durably completed as above and cannot block later groups.

The seven ADR-0018 StepStatus values and all lifecycle field-presence rules stay
unchanged. Plan revisions remain append-only: no insertion, sequence rewriting,
or reordering is authorized. A later Policy change that newly requires approval
cannot reuse a prior `NOT_REQUIRED` result. If the current Plan has no safe,
unconsumed gate, processing fails closed and requires a new Plan or Task.

Policy `ALLOW` is not permission for an external effect. Root approval,
PROACTIVE, capability overlay, task/deadline, and every other mandatory gate
remain authoritative. P6 does not invoke a provider, mint a RequestId, create a
dispatch intent, or cause an external effect. A future P8 dispatcher must
re-evaluate current Task, Step, Policy, overlay, Grant, and other applicable
authority immediately before dispatch.

## Consequences

- Uses the existing complete Plan revision and P5 binding path; no migration or
  exception to append-only history is needed.
- `WAIT_APPROVAL` success has one additional explicit, typed host outcome, while
  human approval remains represented only by the authenticated response and
  Grant lifecycle.
- Plan construction and validation must preserve exact group membership and
  ordering; Storage remains authoritative for request-to-gate selection.
- Recovery reads the durable Step status/result and existing approval records;
  it must never infer approval from `NOT_REQUIRED`.

## Owner ratification

The owner approved **Option A — Initial Mixed Plan with audited NOT_REQUIRED
completion** on 2026-10-11. This ADR records that explicit decision; it does not
change the already accepted P5/P6 authority boundaries or introduce a new
lifecycle state.
