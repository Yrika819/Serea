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

## 4. Grant evaluation

A step may execute under a grant if and only if **all** hold:

1. `capability_id` and `capability_version` match exactly.
2. The canonical `arguments_digest` equals the digest shown in the approved
   request; any changed argument, capability version, or plan revision
   invalidates the grant and requires a fresh request.
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

## 11. Changelog

- 2026-10-07: ADR-0030 specifies routing-only durable handoff for approval
  lifecycle events. Architecture advances to `serea-arch/2.4.0`; the
  `serea.approval/1` surface remains unchanged and P6 retains approval authority.
