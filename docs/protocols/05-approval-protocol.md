# Approval Protocol

Protocol ID: `PROTO-APPROVAL` · Surface: `serea.approval/1` · Status: **FROZEN current contract set** · Architecture: `serea-arch/2.6.0`

P6 owns deterministic approval/grant lifecycle and matching. P5 may prepare an
immutable action but does not evaluate approval or grants; P6 cannot invoke a
provider. Approval is the point where Serea asks a human to lend authority it does not
have. The design goal is the narrowest possible ask: specific capability,
narrow scope, short life, small use count, bound to one task.

The anti-pattern this protocol exists to prevent is the familiar one — a dialog
that says "Allow Serea to access your accounts?" and, once accepted, unlocks
everything forever.

---

## 1. Core principle

> Authority is lent, bounded, expiring, and auditable. Authority is never
> transferred.

There is no "allow everything" state. There is no permanent grant. Every grant
names a capability, a scope, an expiry, a use ceiling, and a task binding.

## 2. `ApprovalRequest`

Raised by the policy engine's `RequireApproval` decision.

```json
{
  "approval_id": "apr_01JQ8ZA1D4NFG8K2M6RTV9XCWB",
  "task_id": "tsk_01JQ8Z9K3M7QWXR4V2T6YH0BNA",
  "step_id": "stp_01JQ8Z9M5T9WXK2H4BNPQ7RDSF",
  "capability_id": "calendar.event.create",
  "capability_version": "1.0.0",
  "risk_class": "EXTERNAL_WRITE",
  "requested_scope": { "calendar": "primary" },
  "arguments_digest": "sha256:…",
  "arguments_preview": { "title": "Design review", "starts_at": "2026-10-02T13:00:00Z" },
  "plain_summary": "Create a 1-hour event 'Design review' on your primary calendar tomorrow at 13:00.",
  "data_class": "PERSONAL",
  "raised_at": "2026-10-01T09:14:23.000Z",
  "expires_at": "2026-10-01T09:44:23.000Z",
  "status": "PENDING"
}
```

#### 2.0.1 An approval unit covers one to eight enumerated actions

The example above is the single-action case, in which the unit has exactly one
member. Under owner decision R2 a unit may instead enumerate 1 to 8 actions,
each with its own `step_id`, its own exact `arguments_digest` and its own
`scope`, all sharing the unit's task, capability, version, plan revision,
registry generation and descriptor revision. The set is normalized by
`step_id` ascending and committed by an `action_set_digest`. Everything in §3.5
applies. A single-action unit is not a special case in the schema — it is the
one-member case of the same structure — but it is the shape every frozen example
in this document shows, so the examples are left as they are.

### 2.1 The prompt must be specific enough to consent to

`plain_summary` is written by host code from the validated arguments, not by
the model. It states the capability, the exact object affected, and the
expected effect. A user must be able to approve from the summary alone,
without inspecting raw JSON.

The `arguments_preview` is redacted through the same redaction path as any
`PRIVATE`-or-higher data leaving Serea. A summary that would leak the content
of a secret is not a summary.

### 2.2 Expiry of the request

A pending request expires (default 30 minutes). An expired request is
`EXPIRED`, not `DENIED` — the distinction matters for retry behaviour. An
expired approval never silently re-raises; the step fails and the task reports
that approval was not obtained in time.

#### 2.2.1 The request status set is closed

`approval_requests.status` ∈ `PENDING` → `APPROVED` / `DENIED` / `EXPIRED`, and
nothing else. A request is **never** `REVOKED`. Revocation is a transition on the
grant, not on the request (§3.4), so a revoked consent leaves its request in
`APPROVED` and the audit trail can still answer "who withdrew this, and when".

Earlier drafts of the migration-0005 conceptual schema carried a `REVOKED` value
in the stored `status` domain "so the domain is not narrowed". That is removed.
An unreachable value in a stored domain is not conservatism; it is a state every
future reader must reason about and no transition can produce. Because migration
0005 has not been created, the value is excluded from the final normative schema
rather than preserved as a misleading artefact.

The grant has its own independent state variable, which is why the
duplicate-response matrix in Device Protocol §5.2 has to be keyed on the grant's
state as well as the request's.

## 3. `ApprovalGrant`

The user's affirmative response. This is the only object that authorizes a
step.

```json
{
  "grant_id": "grt_01JQ8ZA7B3KMW9Q4TVY7XN2RDP",
  "approval_id": "apr_01JQ8ZA1D4NFG8K2M6RTV9XCWB",
  "capability_id": "calendar.event.create",
  "capability_version": "1.0.0",
  "scope": { "calendar": "primary" },
  "arguments_digest": "sha256:…",
  "max_uses": 1,
  "uses_remaining": 1,
  "task_binding": "tsk_01JQ8Z9K3M7QWXR4V2T6YH0BNA",
  "granted_at": "2026-10-01T09:15:02.000Z",
  "expires_at": "2026-10-01T09:45:02.000Z",
  "granted_by": "USER",
  "grant_digest": "sha256:…"
}
```

### 3.1 The six bounds

Every grant carries all six. A grant missing any one is invalid.

| Bound | Default | Purpose |
| --- | --- | --- |
| `capability_id` + `version` | exact | Grants are for one capability, not a family. |
| `scope` | narrowest that permits the call | Limits *which* objects the capability may touch. |
| `max_uses` | 1 | Limits how many times it may be exercised. |
| `expires_at` | 30 minutes | Limits how long it lives. |
| `task_binding` | the requesting task | A grant cannot authorize a *different* task. |
| `arguments_digest` | exact digest of the approved request | A grant cannot authorize changed or reordered arguments. |

`task_binding` is what prevents a single approval from becoming standing
authority. "Yes, create that event" authorizes creating *that* event in *that*
task. It does not authorize Serea to create events later without asking.

### 3.2 `scope`

Scope is capability-defined and validated against the capability's own input
schema. A grant whose scope does not cover the actual call arguments is not a
match, and the call is refused.

Scope matching is **structural and exact**. A scope of `{"calendar":
"primary"}` does not match `{"calendar": "*"}` calls and does not match
`{"calendar": "work"}`. Wildcards are not supported in grants; if a user
genuinely wants breadth, they grant it per-scope, one at a time, each with its
own expiry and use ceiling.

### 3.3 `granted_by`

`USER` — a human on a paired device or the local admin surface.

`ADMIN` — local host configuration, for capabilities the user has globally
allowed (for example "Gmail read is always allowed"). `ADMIN` grants still
carry `expires_at` and `max_uses`; they are not permanent. They are refreshed
by explicit host configuration, which is auditable.

There is no `MODEL` value. There is no path by which a model mints a grant.

### 3.4 Grant state, and why it is not the request state

A grant has its own state variable, independent of its request:

| State | Meaning |
| --- | --- |
| `ACTIVE` | Consumable, subject to §4 |
| `EXHAUSTED` | `uses_remaining` reached 0 |
| `EXPIRED` | `now >= expires_at` |
| `REVOKED` | An authenticated principal withdrew consent (§5.1) |

`PARTIALLY_CONSUMED` is not a state; it is the counter read
`uses_remaining < max_uses`.

Revocation is a **grant-only** transition. It never writes the request's status,
so a revoked consent leaves the request `APPROVED`. This is deliberate: it keeps
"the user declined" (`DENIED`) and "the user withdrew a granted authority"
(`REVOKED` on the grant) distinguishable in the audit trail, and it keeps task
cancellation from being recorded as a human act — task cancellation is not
revocation (§5.1).

### 3.5 Enumerated multi-action grants (owner decision R2, 2026-10-10)

A grant may bind an explicitly enumerated set of **1 to 8** Steps within one task. This is
owner decision R2, recorded verbatim in
[§0b of the owner decision package](../plans/P6-owner-decision-package.md), and it is the
shape that makes §7.1's batch realizable. It does not weaken exact arguments consent: the
authority of a grant is exactly the set of individually displayed actions, and nothing else.

**Shared conditions.** These are carried once and every member must be compatible with all
of them. A member whose durable facts differ makes the whole approval unit invalid, because
a grant is one capability, one version, one plan revision, one generation, one descriptor
revision, one task and one expiry. Ambiguous authority merging is refused.

| Shared condition | Durable source of truth |
| --- | --- |
| `task_id` | `tasks.task_id` |
| `capability_id`, `capability_version` | `step_capability_bindings` |
| `generation_id` | `step_capability_bindings` |
| `descriptor_digest` | `step_capability_bindings` |
| `plan_revision` | `task_steps.plan_revision` |
| `expires_at` | host bound, equal to the request horizon |

**Per-action properties.** Each member carries its own `step_id`, its own exact
`arguments_digest` — which must equal the durable `task_steps.input_digest` and is
recomputed at consumption — and the structural `scope` that was shown for that action.

**Normalization and the action-set digest.**

- Members are ordered by `step_id` ascending, byte-wise. `StepId` is a Crockford ULID, so
  byte order equals lexicographic order and needs no locale.
- `action_set_digest` is SHA-256 over the canonical JSON of the ordered member array, each
  member `{"step_id":…,"arguments_digest":…,"scope":…}`, keys sorted, canonicalized under
  SCJ-1. It is the commitment to *which* actions were approved and is stored on both the
  request row and the grant row.
- Two members with the same `step_id` are refused with `APPROVAL_ACTION_SET_DUPLICATE`.
- A member count outside 1..=8 is refused with `APPROVAL_ACTION_SET_BOUND_EXCEEDED`; more
  than 8 operations are split into another bounded approval unit or refused.
- A member whose shared conditions differ is refused with
  `APPROVAL_ACTION_SET_INCONSISTENT`.

**Immutability.** The action set is fully determinate before approval and is immutable
afterwards. The membership tables carry no-update and no-delete triggers and accept no
insert after their parent row is committed. There is no API by which a Step is added to a
grant after the fact.

**Partial approval.** A `GRANT` response may cover a proper subset of the request's actions.
The granted subset is written to the grant's membership table and is immutable thereafter, so
the granted authority is exactly the approved subset. Ungranted actions receive no authority
at all: they are not members, no consume path can reach them, and they remain requirable
under a new approval unit. A later wider approval is a new unit, never an edit.

**`max_uses`** equals the number of granted members and is therefore at most the number of
individually approved Steps. `uses_remaining` can only reach 0 once every listed Step has
consumed. An unlisted Step can never consume, because membership — not the counter — is what
authorizes it.

**Consumption.** Membership is the authorization test, never scope or digest alone:

```text
1. Load the grant; require status ACTIVE.
2. Require now < expires_at.
3. Require uses_remaining > 0.
4. Require step_id to be a member of this grant.
5. Validate the durable Step against the shared conditions and the member's
   arguments_digest, recomputed from task_steps.input_digest.
6. Require that no use row exists for step_id (UNIQUE(step_id)).
7. In one transaction: insert (grant_id, step_id) and decrement uses_remaining.
```

**Event routing.** A multi-action unit emits its lifecycle events with the **leading
member** — the member with the smallest `step_id` under the ordering above. That value is
routing identity only and carries no authority. One event per member would multiply durable
wakes and risk `max_events_per_transaction`, and would change the 1:1
source-event-to-wake mapping ADR-0030 froze.

**Proof of feasibility.** The contract set as frozen before R2 could not realize §7.1: §4
point 2 binds each executing Step to the digest in the request and the grant, §3 carries one
such digest, and §4.2 makes a repeat consumption for the same Step a no-op — so a
single-Step grant makes every use above the first unreachable. The full proof and the twelve
compatibility cases are in
[P6A-feasibility-gate.md](../plans/P6A-feasibility-gate.md). R2 resolves it by carrying the
digest per member instead of once per grant.

## 4. Grant evaluation

A step may execute under a grant if and only if **all** hold:

1. `capability_id` and `capability_version` match exactly.
2. The canonical `arguments_digest` equals the digest carried on that
   `step_id`'s own member row of the grant; any changed argument, capability
   version, or plan revision invalidates the grant and requires a fresh
   request. There is no scope-only match and no digest-only match: a Step that
   is not an enumerated member is refused even if its arguments happen to
   equal an approved digest, and even if it sits inside an approved scope.
3. The call's arguments satisfy the grant's `scope`.
4. `uses_remaining > 0`.
5. `now < expires_at`.
6. `task_binding` equals the executing task.
7. The capability is not in a `Deny` policy state (a grant cannot override a
   deny — it can only satisfy an approval requirement).

Point 6 is important: **grants are additive to policy, never subtractive.** A
grant can turn `RequireApproval` into `Allow`. It can never turn `Deny` into
`Allow`. Deny rules are absolute.

### 4.1 Request digest is revalidated before consumption

Immediately before resuming the effecting step, the host recomputes the
canonical `arguments_digest` from the durable step input and verifies equality
with the digest in both `ApprovalRequest` and `ApprovalGrant`. It also confirms
the step identity/order, capability ID and pinned version, current descriptor,
task ceiling, and current policy decision. A mismatch invalidates the pending
grant; it is never repaired or widened by the model. The user must approve a
new request that displays the new arguments.

This check is a host-side contract, not a prompt instruction. It binds consent
to the exact arguments the user saw.

### 4.2 Consumption is atomic

Consuming a use is a single atomic durable operation, tied to the step's
`step_id`. A crash between "grant validated" and "grant consumed" must not
allow a double use. The consumption record names the `step_id` that consumed
it, so a repeated consumption attempt for the same step is a no-op rather than
a second decrement.

### 4.3 Grants are not transferable

A grant is bound to a task, and a task's steps are host-determined. There is no
API by which a grant can be attached to a different task, exported, or
replayed. Grant objects never leave the host except as audit records.

### 4.4 Root operations

A root operation always requires explicit approval. A matching Policy rule whose
decision is `ALLOW` does not satisfy it, and neither does an existing grant:
Policy §4.3 and invariant P7 make it non-delegable, and §4 point 7 above means a
grant can only satisfy an approval requirement, never bypass one. Where trusted
device confirmation is additionally required, that requirement is preserved and
is not relaxed by any approval that arrives without it.

## 5. Device-bound approval

An approval granted on the Android client is authorized by that device's
session. The request travels over the authenticated device link
([Device Protocol §4](07-device-protocol.md)) and the response is signed into
the audit record with the device id and session id.

For `ELEVATED_DEVICE` capabilities, approval additionally requires a
**local** biometric or device-credential confirmation on the device
(`BiometricPrompt`), so that possession of an unlocked-but-unverified session is
not sufficient for the highest-risk actions. The host verifies the
confirmation result through the device link; it never sees or stores the
biometric material itself.

If no device with a confirmed secure context is reachable, the approval cannot
be granted and the task stays in `WAITING_APPROVAL` or moves to `BLOCKED`. It
does not fall back to a weaker approval path.

### 5.1 The authenticated response seam, and revocation

P6 receives approval responses through an internal, typed, non-wire seam. A
device frame is never self-authenticating: the Core/device adapter validates the
authenticated principal, the device session and, where required, the elevated
confirmation, and only then hands P6 a typed
`AuthenticatedApprovalResponseV1` carrying `approval_id`, `decision`
(`GRANT` | `DENY`), the principal, the device id, the session id, the
authentication strength (`SESSION` | `ELEVATED_CONFIRMED`), `confirmed_at`, an
elevated-confirmation **digest handle** and never biometric material, and the
device's ceiling proposals for `max_uses` and `expires_at`. The host **clamps**
those proposals to the request's own bounds and refuses a response that exceeds
either, so a device cannot widen an approval by editing JSON.

An `ELEVATED_DEVICE` capability whose response arrives without an elevated
confirmation is discarded with `APPROVAL_BIOMETRIC_REQUIRED`. There is no
host-side waiver.

**Revocation is not a response decision.** It is a separate, authenticated,
trusted internal operation: the change widens the internal port with a dedicated
revocation request type and does **not** overload `GRANT`/`DENY` with an
undocumented `REVOKE` value. Only an authenticated user or admin principal can
revoke; a model-authored JSON object is never a trusted revocation request; the
device wire surface for revocation is a later change and P6 does not implement
the Android transport for it. Revocation is a terminal transition on the grant
only (§3.4), it never writes the request status, and already-consumed uses are
historical facts that are not undone. Task cancellation is **not** revocation: a
cancelled task makes its grants unusable but leaves the grant row `ACTIVE`, and
the task-deletion cascade removes it. A policy change is likewise a
revalidation outcome, not a revocation.

### 5.2 Duplicate-response matrix

A response is applied in one store transaction that re-reads the current request
**and grant** state. Every cell resolves to exactly one of an idempotent
success, a typed refusal, or a first-response commit. There is no free-text
outcome and no second grant in any cell.

| Request state | Grant state | Incoming `GRANT` | Incoming `DENY` |
| --- | --- | --- | --- |
| `PENDING` | — | commit `APPROVED`; at most one grant | commit `DENIED`; no grant |
| `APPROVED` | unused | idempotent success with the existing grant identity | typed refusal `APPROVAL_RESPONSE_CONFLICT` |
| `APPROVED` | exhausted | idempotent success with the same terminal reason | typed refusal `APPROVAL_RESPONSE_CONFLICT` |
| `APPROVED` | `REVOKED` | typed refusal `APPROVAL_REVOKED` | typed refusal `APPROVAL_REVOKED` |
| `DENIED` | — | typed refusal `APPROVAL_RESPONSE_CONFLICT` | idempotent success with the existing denial |
| `EXPIRED` | — | typed refusal `APPROVAL_REQUEST_EXPIRED` | typed refusal `APPROVAL_REQUEST_EXPIRED` |
| Task `CANCELLED` | — | typed refusal `TASK_NOT_APPROVABLE` | typed refusal `TASK_NOT_APPROVABLE` |
| Task `FAILED` | — | typed refusal `TASK_NOT_APPROVABLE` | typed refusal `TASK_NOT_APPROVABLE` |

The `APPROVED` / grant-`REVOKED` row is what grant-only revocation forces into
the matrix. Without it a repeated `GRANT` would take the idempotent-success path
and hand back the identity of a grant whose consent has been withdrawn. Idempotency
is by durable state, not by message identity: a replayed frame and a
re-submitted response produce the same result because the row already says so.

## 6. Denial

Denial is a normal, frequent outcome and is treated as one.

- A denied approval fails the step with `APPROVAL_DENIED`.
- Denial is recorded with the reason the user selected ("wrong target",
  "not now", "never for this capability").
- Denial is **not** retried automatically. A task may re-plan around it, but
  only under the task's step budget, and each re-plan that would re-request the
  same capability must go through approval again.
- Denial of a capability can optionally persist as a durable
  `denied_capability` entry, so the system stops asking for something the user
  has said no to. This is user-controlled and reversible.

## 7. Approval fatigue

Design measures that keep the ask rate low without weakening the guarantee:

1. **Batch within a task where semantics allow.** A plan that needs three
   `calendar.event.create` calls in the same task can request a grant with
   `max_uses: 3` and a matching scope, presented as one prompt naming all
   three. The grant is still bounded, still expiring, still task-bound.
   **Feasibility note.** This case is realized by the enumerated multi-action
   grant of §3.5: the unit enumerates all three Steps with their own
   `arguments_digest`, `max_uses` equals 3, and each of the three Steps consumes
   exactly one use. It is not realized by scope matching alone, which would
   authorize calls the user never saw. The proof that the unextended contract
   could not realize it, and the owner decision that resolved it, are in
   [P6A-feasibility-gate.md](../plans/P6A-feasibility-gate.md).
2. **Reuse within scope, not beyond.** The second event in the same task, in
   the same calendar, within the same prompt, needs no new ask.
3. **Nothing is granted implicitly by silence.** No prompt auto-expires into a
   grant. Timeout means no.
4. **Admin defaults for pure reads.** `Allow` for `OBSERVE` on already-paired
   providers, so the common case asks nothing.
5. **The watcher never asks.** The proactive watcher is read-only and raises no
   approvals at all; it produces proposals, and the user acts.

The measure of success is: **an ordinary day of assistant use produces zero
approval prompts for read-only work, and a small, specific, comprehensible
number for anything that writes.**

## 8. Audit

Every approval lifecycle event is recorded:

| Event | When |
| --- | --- |
| `APPROVAL_REQUIRED` | Request raised |
| `APPROVAL_GRANTED` | User approved; grant created |
| `APPROVAL_DENIED` | User denied |
| `APPROVAL_EXPIRED` | Request or grant expired unused |
| `APPROVAL_CONSUMED` | A use was consumed by a step |
| `APPROVAL_EXPIRED_UNUSED` | Grant reached expiry with uses remaining |
| `APPROVAL_REVOKED` *(Proposed)* | An authenticated principal revoked the grant |

`APPROVAL_EXPIRED` names the request; `APPROVAL_EXPIRED_UNUSED` names the grant.
The transition must record which object expired, not merely that something did.

`APPROVAL_REVOKED` is a **Proposed** new event kind. It does not exist in the
current `EventKind` set and adding it is an architecture-minor addition; older
clients skip unknown kinds rather than failing the stream. Its payload carries
only `approval_id`, `task_id`, `step_id`, `grant_id`, the actor class, the
authenticated device and session, and `revoked_at`, matching the closed
`ApprovalLifecyclePayloadV1` routing shape so the Scheduler wake path needs no
change.

The audit trail records the grant digest, the consuming `step_id`, and the
device/session that authorized it. Approvals are part of the durable state that
survives restart, so an approval granted before a crash is still consumable
afterwards, exactly once.

## 9. Invariants summary

| # | Invariant |
| --- | --- |
| A1 | Every grant carries capability, scope, expiry, use ceiling, and task binding. |
| A2 | No grant is unbounded, permanent, or wildcard. |
| A3 | A grant can satisfy an approval requirement but can never override a deny. |
| A4 | A grant authorizes only its bound task. |
| A5 | Grant consumption is atomic and idempotent per `step_id`. |
| A6 | Silence and timeout never produce a grant. |
| A7 | Elevated-device approval additionally requires on-device biometric confirmation. |
| A8 | The model can neither mint nor widen a grant. |
| A9 | Denial is terminal for the step and never auto-retried. |
| A10 | The proactive watcher raises no approvals. |
| A11 | A grant is bound to the exact canonical arguments the user approved; any digest mismatch invalidates it before execution. |
| A12 | A root operation always requires explicit approval; no rule or grant bypasses it. |
| A13 | Revocation is an authenticated, grant-only, terminal transition. Task cancellation and policy change are not revocation. |
| A14 | A multi-action grant authorizes exactly its enumerated members, and nothing else. Scope or digest equality never adds a Step. |
| A15 | The approved Step set is deterministic before approval and immutable afterwards; `max_uses` never exceeds the number of individually approved Steps. |

## 10. P3 lifecycle-event handoff boundary

`APPROVAL_GRANTED`, `APPROVAL_DENIED`, and `APPROVAL_EXPIRED` events carry the
closed `ApprovalLifecyclePayloadV1` routing object with exactly `approval_id`,
`task_id`, and `step_id`. The event's `correlation_id` must equal the payload
`task_id`; its trace is required and must carry the same `task_id` and
`step_id`. These fields identify the Approval/Task/Step records for routing;
they are not an `ApprovalGrant` and carry no authority.

P3 Scheduler validates the routing identity and materializes one durable,
deduplicated `ApprovalLifecycleWake` per source event. Reading the wake does not
acknowledge it. Future P6 loads and validates authoritative Approval state,
applies the outcome through Policy/Approval and Task contracts, then explicitly
acknowledges the handoff. P3 does not validate or consume grants, decide whether
an outcome is authoritative, approve a capability, or transition a task based
on the event kind alone. See [ADR-0030](../decisions/ADR-0030-durable-approval-lifecycle-wake.md).

## 10.1 Storage, retention and privacy ceiling

Approval rows are task-scoped derived state and cascade with the Task:
`approval_requests`, `approval_request_actions`, `approval_grants`,
`approval_grant_members` and `approval_grant_uses` are all `ON DELETE CASCADE`
from `tasks`. The membership tables are the durable authority binding of the exact
approved Step set, for both the request and the grant, as owner decision R2
permits. Policy history never cascades — `policy_revisions` and `policy_rules`
are host audit artefacts, and Event Protocol §8 already states that policy
history outliving its task is intentional.

Raw arguments are **never** persisted in an approval row. The durable Step already
holds the canonical arguments in the content-addressed blob store referenced by
`input_digest`, and the digest is recomputed from there; a second copy would be a
second thing to keep consistent and a second place for a credential-shaped value
to land.

The privacy ceiling on anything persisted is `PERSONAL`, per destination:

| Destination | Maximum class |
| --- | --- |
| Approval summary persisted in SQLite | `PERSONAL` |
| Approval summary rendered to the device | `PERSONAL`, redacted per Data Classification §6 |
| `arguments_preview` persisted | not persisted at all; derived from the Step's blob at render time |
| Event payload | `PERSONAL`, metadata only |
| Any row, any column | `SECRET` and `CREDENTIAL` are refused mechanically by `serea-storage` |

The ceiling is mechanical rather than aspirational: `serea-storage` refuses
`SECRET`/`CREDENTIAL` outright and returns `AtRestProtectionUnavailable` for
`PRIVATE` ordinary rows. A `CREDENTIAL` value never appears in a request, a
grant, an event, a summary or a log. `ElevatedConfirmationRef` is a digest of the
attestation and never biometric material.

## 11. Changelog

- 2026-10-07: ADR-0030 specifies routing-only durable handoff for approval
  lifecycle events. Architecture advances to `serea-arch/2.4.0`; the
  `serea.approval/1` surface remains unchanged and P6 retains approval authority.
- 2026-10-10: owner ratified the P6 approval package, then ratified owner decision
  **R2 — enumerated multi-action grant**. §2.0.1 adds the one-to-eight action unit; §3.5
  specifies the R2 authority model, its shared conditions, normalization, action-set digest,
  immutability, partial approval, `max_uses`, consumption and event routing; §4 point 2 now
  binds each Step to its own member row's digest and rules out scope-only and digest-only
  matching; §7.1 records how the batch case is realized; §9 adds A14 and A15; §10.1 adds the
  membership tables to the cascade set. Earlier the same day: §2.2.1 closes the request status
  set and removes the unreachable request `REVOKED` value; §3.4 adds the grant state variable
  and the grant-only revocation rule; §4.4 states that a root operation is never bypassed by a
  rule or a grant; §5.1 and §5.2 add the authenticated response seam, the dedicated revocation
  port and the grant-keyed duplicate-response matrix; §8 adds `APPROVAL_REVOKED` (Proposed).
