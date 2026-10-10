# ADR-0038: Approval identity, lifecycle, and authenticated response

Status: **Proposed** · Date: 2026-10-10 · Architecture: `serea-arch/2.6.0` → `2.7.0` if accepted

Surfaces affected: `serea.approval/1` semantics (prose only), `serea.event/1` gains one
Proposed kind. No wire-major change.

## Context

[Approval Protocol §3.1](../protocols/05-approval-protocol.md) freezes six bounds on a
grant: capability and version, scope, `max_uses`, `expires_at`, task binding, and
`arguments_digest`. §5 requires device-bound approval with on-device confirmation for
`ELEVATED_DEVICE`. [Device Protocol §5.2](../protocols/07-device-protocol.md) states that a
response whose `approval_id` is not `PENDING` is dropped and that a device's
`max_uses` and `expires_at` are ceiling proposals the host clamps.

Three questions were not answered by that set. D8 asked whether P5's registry facts join the
grant identity. D12 asked what revocation means, given that no revocation state, operation,
or event exists. D13 asked for an authoritative duplicate-response matrix, given that
delivery is at-least-once and the existing text is prose rather than a table.

The audit also asked D14, whether the request, the `WAITING_APPROVAL` transition, the event,
and the wake route are one transaction. That part is settled by
[ADR-0025](../decisions/ADR-0025-p3-event-participant-composition.md): state, journal, event
and sequence commit in one transaction through fixed upper-layer composition, with no
`serea-storage` → Event Bus edge. This ADR does not revisit it.

## Decision

**Proposed.**

### Identity

A grant binds, in addition to the six frozen bounds, the registry `generation_id` and
`descriptor_digest` from P5. Provider and implementation are `TRANSPARENCY_ONLY` and are not
authorization identity.

The reason is mechanical, not stylistic. [Task Protocol §3](../protocols/02-task-protocol.md)
makes a capability Step's binding immutable before P6 authorization, and migration 0004
enforces it with no-update and no-delete triggers. A grant that names a Step is already bound
to that Step's pinned provider and implementation, so duplicating them adds a second field to
keep consistent while adding no protection. The generation and descriptor digest are
different: without them, a registry change between approval and dispatch could ride on an old
grant.

`approval_grant_uses` carries a `PRIMARY KEY (grant_id, step_id)`, which is the atomicity
invariant [ADR-0039](ADR-0039-p6-p8-revalidation-and-grant-consumption-boundary.md) requires.
The conceptual schema also carries `UNIQUE (step_id)`, which is strictly stronger — it makes
a step consumable by at most one grant across the host's lifetime. That is offered as
defence-in-depth but must be confirmed in P6A: it also forbids the re-approval flow in which
a first grant for a step is invalidated and a second grant for the same `step_id` is later
consumed.

### Scope and uses

Scope is a capability-defined, structurally exact projection, per §3.2. Wildcards are
unsupported. `max_uses` is supported from the first P6 slice with an idempotent per-Step use
record, because the atomicity requirement is already frozen by §4.2 and the incremental cost
of `N > 1` over `N = 1` is one integer and a uniqueness constraint on
`(grant_id, step_id)`.

### State machine

Two independent state variables. Requests: `PENDING` → `APPROVED` / `DENIED` / `EXPIRED`.
Grants: `ACTIVE` → `EXHAUSTED` / `EXPIRED` / `REVOKED`. `PARTIALLY_CONSUMED` is
not a state; it is `uses_remaining < max_uses`. No new wire enum is introduced.

Revocation is grant-only (below), so a request is never `REVOKED`. A request `REVOKED`
state was recorded in earlier drafts and is removed here: it is unreachable rather than
merely unused, and the duplicate-response matrix below is therefore keyed on the grant's
state as well as the request's. The stored `approval_requests.status` domain keeps the
value so the schema is not narrowed, but no transition may write it.

### Duplicate response matrix

A response is applied in one transaction that re-reads the current request state. Every cell
resolves to idempotent success, a typed refusal, or a first-response commit. There is no
free-text outcome and no second grant in any cell.

| Request state | `GRANT` | `DENY` |
| --- | --- | --- |
| `PENDING` | commit `APPROVED`; at most one grant | commit `DENIED`; no grant |
| `APPROVED`, grant unused | idempotent success with the existing grant identity | typed refusal `APPROVAL_RESPONSE_CONFLICT` |
| `APPROVED`, grant exhausted | idempotent success with the same terminal reason | typed refusal `APPROVAL_RESPONSE_CONFLICT` |
| `APPROVED`, grant `REVOKED` | typed refusal `APPROVAL_REVOKED` | typed refusal `APPROVAL_REVOKED` |
| `DENIED` | typed refusal `APPROVAL_RESPONSE_CONFLICT` | idempotent success with the existing denial |
| `EXPIRED` | typed refusal `APPROVAL_REQUEST_EXPIRED` | typed refusal `APPROVAL_REQUEST_EXPIRED` |
| Task `CANCELLED` | typed refusal `TASK_NOT_APPROVABLE` | typed refusal `TASK_NOT_APPROVABLE` |
| Task `FAILED` | typed refusal `TASK_NOT_APPROVABLE` | typed refusal `TASK_NOT_APPROVABLE` |

The `APPROVED`, grant `REVOKED` row is required by the grant-only revocation rule below.
Without it a repeated `GRANT` would take the idempotent-success path and return the identity
of a grant whose consent has been withdrawn. Earlier drafts keyed the whole matrix on the
request only and had a bare `REVOKED` request row; both are replaced by the grant-keyed row
above.

### Revocation

Revocation is an explicit, durable, terminal transition on the **grant**, performed by an
authenticated user or admin through the same seam as an approval response, or from the local
admin surface.

**Unresolved seam gap.** The `AuthenticatedApprovalResponseV1` seam below carries
`decision: ResponseDecision` with `GRANT | DENY` only, and
[Device Protocol §5.2](../protocols/07-device-protocol.md) carries `GRANT | DENY | DEFER`
only. Neither can express a revocation, so "through the same seam as an approval response"
is not realizable as written. P6A must either widen `ResponseDecision` with a `REVOKE`
variant — with the device surface added later as its own change — or narrow this
recommendation to the local admin surface in P6 V1. The grant-only, human-initiated,
durable, terminal semantics are unaffected either way.

Task cancellation does **not** revoke. A cancelled task makes its grants unusable — P8
refuses a cancelled task — but the grant row keeps its `ACTIVE` state and is removed by the
task-deletion cascade. Conflating the two would make `REVOKED` mean both "a human withdrew
consent" and "the task ended", and the audit trail could no longer answer who withdrew what.
A policy change likewise does not revoke; it is a revalidation outcome.

Revoking a partially consumed grant is legal. Already-consumed uses are historical and are
not undone; future uses are refused.

`APPROVAL_REVOKED` is a **Proposed** new `EventKind`. It does not exist today. Adding it is
an architecture-minor addition per Protocol Index §4.2 rule 4, and older clients skip
unknown kinds rather than failing the stream. Its payload carries `approval_id`, `task_id`,
`step_id`, `grant_id`, actor class, authenticated device and session, and `revoked_at` —
matching the closed `ApprovalLifecyclePayloadV1` routing shape so the Scheduler wake path
needs no change.

### Authenticated response seam

P6 receives a typed internal value, never device JSON.

```rust
pub struct AuthenticatedApprovalResponseV1 {
    pub approval_id: ApprovalId,
    pub decision: ResponseDecision,          // GRANT | DENY
    pub principal: AuthenticatedPrincipal,   // PairingPrincipal | AdminPrincipal
    pub device_id: DeviceId,
    pub session_id: SessionId,
    pub authentication_strength: AuthenticationStrength, // SESSION | ELEVATED_CONFIRMED
    pub confirmed_at: EpochMillis,
    pub elevated_confirmation: Option<ElevatedConfirmationRef>, // digest handle only
    pub requested_max_uses: Option<u32>,
    pub requested_expires_at: Option<EpochMillis>,
}
```

`ElevatedConfirmationRef` is a digest of the attestation and never biometric material,
consistent with [Data Classification §3](../protocols/09-data-classification-protocol.md),
which puts biometric templates in the OS or TEE only. `requested_max_uses` and
`requested_expires_at` are clamped to the request's own bounds, and a response that exceeds
either is refused.

### Expiry

`now < expires_at` is unchanged. `approval_request_expiry_ms = 1800000` is already a named
bound in [Bounds Protocol §2](../protocols/10-bounds-protocol.md) and stays. A second bound
`approval_grant_expiry_ms = 1800000` is proposed so that a grant cannot outlive its request;
that value is proposed in ADR-0040.

### Privacy ceiling

The persisted approval row carries no raw arguments and no PRIVATE text. `CREDENTIAL` and
`SECRET` are refused by `serea-storage` mechanically. The summary is built by a host
capability-specific builder from classified arguments and is a deterministic projection of
the Step's durable blob, re-derivable rather than duplicated.

The ceiling is `PERSONAL` for anything persisted, for a mechanical reason: `serea-storage`
returns `AtRestProtectionUnavailable` for `PRIVATE` ordinary rows, and ADR-0022 is still
Proposed. Permitting PRIVATE summaries is gated on ADR-0022, which would make a
security-relevant change through an unrelated ADR.

## Consequences

- The audit trail can answer "who withdrew this and when", because revocation is a human act
  with a human actor.
- P6 needs no cancellation path, which removes the policy-crate-to-Task-engine coupling that
  D16 implied.
- `ELEVATED_DEVICE` approval keeps its no-fallback requirement.

## Verification obligations (future P6D)

- The full duplicate matrix, including concurrent connections, and including the
  `APPROVED`-with-revoked-grant cell — no cell may return a revoked grant identity.
- Expiry boundary at `expires_at - 1`, `expires_at`, and `expires_at + 1`.
- Clamping tests for proposed `max_uses` and `expires_at`.
- Digest re-derivation against changed arguments.
- Cancellation-is-not-revocation, asserted on the grant row itself.

## Status

**Proposed. Not accepted.** The `APPROVAL_REVOKED` event kind, the two grant states, and the
`PERSONAL` persisted ceiling are proposals. No P6 runtime exists.
