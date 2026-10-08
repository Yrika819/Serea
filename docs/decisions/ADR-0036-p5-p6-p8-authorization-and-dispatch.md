# ADR-0036: P5 preparation, P6 authorization, and P8 dispatch

Status: **Accepted** · Date: 2026-10-08 · Architecture: `serea-arch/2.6.0`

## Decision

P5 owns durable immutable registry state, manifest matching, schema catalog
and compiler, deterministic tool projection, proposal validation, classified
argument validation, PreparedActionV1, typed availability/refusal outcomes,
and P6 handoff. P5 does not evaluate PolicyDecision, match approvals/grants,
check duplicate windows, count repeated execution or tool calls, reserve
idempotency dispatch, invoke providers, accept results, commit receipts/evidence,
or reconcile ambiguous effects.

P6 owns deterministic policy, approval/grant lifecycle, and authorization of
immutable PreparedActionV1. P6 cannot mutate bound facts or invoke a provider;
it returns a typed authorization outcome.

P8 is the first phase permitted to invoke `CapabilityProvider::invoke`, using
deterministic mock external providers. Before each dispatch, P8 rechecks Task
cancellation, live capability enabled/removal, provider availability, deadline,
and approval/grant validity and consumability. It does not re-resolve pinned
descriptor facts. Its frozen order is proposal validation, registry/schema,
P6 policy, P6 approval, duplicate suppression, repeated-action bound,
tool-call budget and durable dispatch intent, provider invoke, result/receipt/
evidence, reconciliation if needed. One SQLite transaction commits
duplicate/repeat/tool-call checks, applicable IDK reservation, intent, budget
consumption, and intent events. Provider IO starts after commit; no SQLite
write transaction spans provider IO. Recovery centers on DISPATCH_INTENT,
COMPLETED, FAILED, and AMBIGUOUS. No exactly-once claim.

RequestId is per actual provider dispatch attempt and minted by P8 for each
committed dispatch intent. Same-Step retry retains TaskId, StepId, capability/
version, arguments, digest, and IDK-1, but has a new RequestId. IDK-1 omits
provider, implementation, RequestId, and attempt. Alternate implementations
of one logical capability/version preserve the same logical idempotency meaning.

`max_tool_calls_per_task` is consumed once per durably committed provider
dispatch intent, including primary invoke, same-Step retry, and reconciliation
invoke. Proposal validation/preparation, policy denial, approval pending/
denial/expiry, duplicate suppression, pre-dispatch refusal, and unavailable
provider consume no unit. A committed intent is never refunded. Migration 0004
has no tool-call count column.

Duplicate suppression remains global across tasks, for effecting capabilities
only, key `(CapabilityId, arguments_digest)`, window 86,400,000 ms, with
version intentionally excluded. Cross-version suppression is accepted. P5
does not implement duplicate/idempotency dispatch state. NATIVE means the
provider promises same-IDK dedupe. EMULATED means the host durably prevents a
second provider dispatch with the same active/completed IDK under applicable
retention. NONE has no dedupe guarantee. These are future P8 requirements,
distinct from the 24-hour duplicate window.

P5 freezes only future result invariants: exact RequestId response binding,
host output-schema validation, output digest verification, valid receipt for
effecting success, provider cannot widen replay safety or fabricate host/policy
evidence, raw result does not self-authorize, AMBIGUOUS effect is never blindly
retried, and TaskEngine owns Task lifecycle. Full ActionResult status matrix,
receipt timing, evidence persistence, and reconciliation execution are deferred
to P8 contract closure before first provider invoke. Effecting reconciliation
requires explicit host-reviewed binding; no naming inference or provider-
selected target. No production shortcut exists; `serea.action/2` is unchanged.

## Consequences

P5 ends at a typed prepared action; P6 ends at authorization; P8 alone may
cross the provider effect boundary after result/receipt/ambiguity closure.
Deferred P8 decisions are not P5 blockers.
