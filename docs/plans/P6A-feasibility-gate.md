# P6A feasibility gate — multi-use grants against exact Step binding

Status: **RESOLVED BY OWNER DECISION R2, 2026-10-10** · Gate run: 2026-10-10 · Architecture: `serea-arch/2.6.0` → `2.7.0`

This document is the P6A hard gate required before ADR-0037 through ADR-0040 may be marked
Accepted and before migration 0005 may be created. It asked one question:

> Can one grant authorize several distinct Steps without weakening exact identity or exact
> arguments consent?

The first run answered **no**, and reported a BLOCKER (§1–§6 below are that proof, kept as
the record of why a decision was needed). On 2026-10-10 the owner then made the additional
decision recorded as **R2 — enumerated multi-action grant**
([§0b of the owner decision package](P6-owner-decision-package.md)), which resolves the
contradiction by making the authority identity of a multi-action grant the *enumerated set
of exactly the actions a human approved*. §7 below is the R2 model and the re-run proof.
The gate now passes.

Nothing in this document is implemented. No `serea-policy` crate, no migration 0005, no
evaluator, no approval runtime. P8 remains the first phase permitted to invoke
`CapabilityProvider::invoke`.

---

## 1. The premises

Every premise below is either FROZEN protocol text, an existing mechanical property of the
code, or an owner-ratified decision from the
[P6 owner decision package](P6-owner-decision-package.md). None is contested by this gate.

| # | Premise | Source |
| --- | --- | --- |
| P1 | One capability Step is exactly one capability action. A plan step carries one capability, one pinned descriptor revision, and one content-addressed input blob; `prepare_action` takes one `capability_id` and one `step_id` and returns exactly one `PreparedActionV1`. Three `calendar.event.create` calls in one task are three Steps. | Task Protocol §3, §5 rule 2; `crates/serea-capability/src/preparation.rs:247` |
| P2 | Consumption is a single atomic durable operation tied to the consuming `step_id`, and "a repeated consumption attempt for the same step is a no-op rather than a second decrement". Therefore at most one **effective** consumption exists per Step, ever. | Approval Protocol §4.2 |
| P3 | A step may execute under a grant **if and only if all** of seven conditions hold. Condition 2 is that the canonical `arguments_digest` equals the digest shown in the approved request; §4.1 requires equality with the digest in both `ApprovalRequest` and `ApprovalGrant`. | Approval Protocol §4 point 2, §4.1 |
| P4 | The grant carries the six bounds: capability + version, scope, `max_uses`, `expires_at`, `task_binding`, `arguments_digest`. The frozen `ApprovalGrant` example carries **no `step_id`** and exactly **one** `arguments_digest`. | Approval Protocol §3, §3.1 |
| P5 | "A plan that needs three `calendar.event.create` calls in the same task **can request a grant with `max_uses: 3`** and a matching scope, presented as one prompt naming all three." One grant, three calls, one prompt. | Approval Protocol §7.1 |
| P6 | P6 V1 multi-use grants are ratified **YES**, with `approval_grant_max_uses = 8`. | Owner decision package §14.2 |
| P7 | The ratified identity table marks `TaskId`, `StepId` **and** `arguments_digest` as `MUST_BIND` on the grant. | Owner decision package §6.2 |
| P8 | The execution mandate forbids: reducing multi-use to one-use; introducing a wildcard or aggregate grant broader than the approved individual actions; inventing an authority-bearing table or grant identity rule merely to make a test pass; and weakening exact identity or arguments consent. | P6 execution mandate §4 |

### 1.1 The case that must be realized

Approval Protocol §7.1 is FROZEN contract text, not a proposal. It is the only place in the
contract set that states what a multi-use grant is *for*, and it states three distinguishable
calls: "presented as one prompt naming all three" presupposes three actions a human can tell
apart, which means three distinct canonical argument sets, which by P1 means three distinct
`arguments_digest` values and three distinct `StepId` values.

The ratified package itself reads it that way. Its §6.4 states that the alternative to
structural scope projection — "exact action equality" — "would refuse the batch case in §7.1
that the protocol explicitly describes, and would make `max_uses: 3` unusable". That
statement is only true if the three calls have distinct arguments.

---

## 2. The theorem and proof

**Theorem.** P1–P7 cannot be satisfied simultaneously under P8.

**Case A — the grant binds one `StepId`, per P7.**

1. The grant names exactly one `StepId` S and exactly one `arguments_digest` D.
2. By P2 the only Step that can ever effectively consume it is S: a second consumption for S
   is a no-op, and no other Step is named.
3. Therefore `uses_remaining` can perform at most the transition 1 → 0. No authority
   corresponding to `max_uses = 2` is reachable.
4. P5 and P6 require `max_uses: 3` and up to 8 to be exercisable. Contradiction. ∎

**Case B — the grant binds no single `StepId`, per P4 (the frozen grant example).**

1. A Step may consume only if its canonical `arguments_digest` equals the grant's single
   `arguments_digest` (P3, P4).
2. By P1, distinct Steps carry distinct digests whenever their arguments differ.
3. Therefore one grant authorizes at most the set of Steps whose canonical arguments are
   byte-identical to the approved ones. For §7.1's three distinguishable calls, exactly one
   is authorizable.
4. P5 requires all three to be authorizable by that one grant. Contradiction. ∎

**Case C — the grant carries a set of digests, still step-agnostic.**

A grant whose authority is "any Step in this task whose `arguments_digest` is one of N
approved digests" does authorize §7.1's batch. It is broader than the actions the user
actually saw: a *newly appended* Step in the same plan revision whose arguments happen to
equal an approved digest would be authorized without ever having been displayed. P8 forbids
an aggregate grant broader than the approved individual actions. Contradiction. ∎

The only remaining shape — the grant binds an **enumerated, bounded set of
`(StepId, arguments_digest)` actions** — satisfies P1–P7 and P8 at once, but it requires a
new authority identity rule (a grant no longer binds one action) and a new authority-bearing
structure (the enumeration must be durable, so a table or a canonical JSON column on both
the request and the grant). P8 forbids inventing that to make a test pass, and the P6
execution mandate §15 names the exact condition for stopping:

> An approved multi-use requirement cannot coexist with exact Step binding without a new
> authority design.

That condition is met.

---

## 3. A third contradiction found while proving the two above

The contract set does not even agree on whether a grant binds a `StepId`.

| Source | Says |
| --- | --- |
| Approval Protocol §3 (frozen grant example) | no `step_id` field at all |
| Approval Protocol §3.1 (frozen six bounds) | `task_binding` is the binding bound; `StepId` is not among the six |
| ADR-0038 (decision, ratified with the package) | "A grant binds, in addition to the six frozen bounds, the registry `generation_id` and `descriptor_digest`" — again no `StepId` |
| Owner decision package §6.2 (ratified) | `TaskId`, `StepId` — `MUST_BIND` |
| Owner decision package §8.4 (conceptual DDL) | `approval_grants.step_id TEXT NOT NULL`, plus `UNIQUE (approval_id, task_id)` = at most one grant per approval |
| P6 execution mandate §4 item 1 | "A grant bound to exact TaskId/StepId" |

The conceptual DDL is decisive on its own terms: `UNIQUE (approval_id, task_id)` plus a
non-null `step_id` on the grant means **one approval, one grant, one Step**. Combined with P2,
`max_uses` above 1 is unreachable authority. That is the same contradiction as Case A,
produced by the proposed schema rather than by the prose.

This is recorded here rather than silently resolved, because resolving it changes the
authority identity of every grant P6 would ever mint.

---

## 4. Formal compatibility analysis

The twelve cases the mandate requires, evaluated against the ratified contract set. "Status"
names whether the case is decided, and where it is not, which premise is unstable.

| Case | Approval identity | Actual user-approved arguments | Permitted use | Durable keys | Grant use accounting | Allowed / rejected | Expected failure code | Status |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| Single Step / single use | `apr_…`, `tsk_…`, `stp_…`, capability + version, `generation_id`, `descriptor_digest` | exactly the one canonical argument set shown | one consume by the bound Step | `(grant_id, step_id)` | `max_uses` 1, `uses_remaining` 1 → 0 | allowed | — | **decided** |
| Single Step / repeated consume | unchanged | unchanged | none | `(grant_id, step_id)` exists | no second decrement (P2) | rejected | `APPROVAL_ALREADY_CONSUMED` | **decided** |
| Multiple Steps / same scope | task + capability + scope | not defined by the contract set | not determined | not defined | not determined | not determined | not defined | **BLOCKED — this is the gate** |
| Multiple Steps / different arguments | as above | each Step's own digest | as above | as above | as above | as above | as above | **BLOCKED — this is the gate** |
| Batch approval naming several explicitly identified Steps | as above | the union of the named actions' digests | as above | as above | as above | as above | as above | **BLOCKED — this is the gate** |
| Expired unused grant, then reapproval | new `apr_…` → new `grt_…` for the same Step | the same arguments, re-approved | one consume | `(new grant_id, step_id)`; `UNIQUE (step_id)` satisfied because no row exists for that Step | new grant, fresh `uses_remaining` | allowed | — | **decided** (DC-1 first limb) |
| Revoked unused grant, then reapproval | new `apr_…` → new `grt_…` for the same Step | as above | one consume | `(new grant_id, step_id)` | as above | allowed | — | **decided** (DC-1 first limb) |
| Already consumed Step, then retry | unchanged | unchanged | none: a previously consumed Step must not be authorized again by a second grant | `UNIQUE (step_id)` blocks `(second grant_id, step_id)` | no decrement on any grant | rejected | `APPROVAL_STEP_ALREADY_CONSUMED` | **decided** (DC-1 second limb) |
| Plan revision and Step replacement | new `plan_revision`, new `stp_…` | the replacement's own digest | old grant does not match | `plan_revision` bound | old accounting unchanged | rejected | `APPROVAL_STEP_SUPERSEDED` | **decided** |
| Same Task, new Step | new `stp_…` | the new Step's own digest | only via a new request | new `(grant_id, step_id)` | new accounting | rejected against the old grant | `APPROVAL_SCOPE_NOT_GRANTED` or `APPROVAL_STEP_NOT_BOUND` | **BLOCKED on the gate** — the ratified package's §16.4 expects refusal, Case C would permit it |
| Cross-task replay | different `tsk_…` | unchanged | none | `task_binding` mismatch | none | rejected | `APPROVAL_TASK_MISMATCH` | **decided** |
| Deadline and policy revision changes | new `expires_at` / new `policy_revision_id` | unchanged | none while stale | grant expiry; evidence revision identity | unchanged | rejected | `APPROVAL_EXPIRED`, `POLICY_REVISION_SUPERSEDED` | **decided** |

Cases marked decided are settled by P1–P4 plus DC-1 and do not depend on the blocked
choice. The three rows that do depend on it are marked blocked, and they are exactly the
rows the mandate singled out.

### 4.1 Why `max_uses` alone cannot close the blocked rows

Raising `max_uses` changes one integer in one column. It does not change which Steps may
consume the grant: that set is fixed by P3 (digest equality) and by whether the grant names
StepIds at all. With `max_uses = 8` and a single-Step grant, seven of the eight uses are
permanently unreachable. With `max_uses = 8` and a step-agnostic single-digest grant, seven
of the eight uses are reachable only by Steps whose arguments are byte-identical to the
approved one. Neither realizes §7.1.

### 4.2 The additional mandatory checks

| Check | Verdict |
| --- | --- |
| Root always-requires-approval versus an explicit ALLOW | **Resolved within accepted semantics.** Policy Protocol §4.3 and invariant P7 already forbid a rule auto-granting a root operation. P6A states it explicitly in policy and approval documents; ADR-0037 records it. Not a blocker. |
| DENY overriding ALLOW versus built-in class defaults | **Resolved.** Model A: fixed code-owned class defaults, explicit rules as the only durable data, any matching DENY overriding every ALLOW at any priority, the task ceiling already a deny outside the ordering. |
| Current policy revision evidence versus future P8 revalidation | **Resolved.** Evidence names `policy_revision_id` + `policy_rules_digest`; P8 re-reads the current pointer and treats a mismatch as a refusal, not a continuation. |
| `max_retained_policy_revisions = 64` versus historical audit and active-revision safety | **Resolved with one P6B obligation.** `policy_state.active_revision_id` is `ON DELETE RESTRICT`, so pruning can never remove the active revision. No other durable P6 row references `policy_revisions` in the conceptual shape, so pruning is FK-safe today. If P6B adds a revision reference to a grant, pruning must refuse to remove any referenced revision. Recorded as an obligation, not a contradiction. |
| `PERSONAL` persisted summaries versus existing storage writer restrictions | **Resolved.** `ordinary_class` admits `Public` and `Personal` and refuses `Private`, `Secret` and `Credential`; `event.rs` refuses `Secret`/`Credential` and `Private` events. `PERSONAL` is the highest class a persisted summary can carry today, and `CREDENTIAL` is mechanically refused. |
| Grant revocation versus already-consumed uses | **Resolved.** Revocation is a terminal grant transition; consumed use rows are historical and are not undone; future uses are refused. |
| Task kind and `RequestedBy` context mapping | **Resolved by DC-2**, which is explicit and finite: derive from both trusted Task kind and trusted provenance; a restrictive Task kind is never relaxed by a different `RequestedBy`; `SYSTEM` is not god mode; an invalid combination is rejected, never silently promoted. P6A must publish the exact function. |
| Approval request state versus grant-only revocation | **Resolved by DC-3.** Revocation is a separate authenticated, trusted internal operation with its own port and event; the request is never `REVOKED`. |
| Approval expiration versus Task deadline | **Resolved.** Bounds Protocol §6.2 pauses the task wall clock in `WAITING_APPROVAL`, so the approval's own `expires_at` is the only clock that runs. |

---

## 5. The escape routes, and why each needs an owner decision

| Route | What it does | Why it is not available autonomously |
| --- | --- | --- |
| R1 — accept single-action grants | `approval_requests` and `approval_grants` each bind exactly one `(TaskId, StepId, arguments_digest, …)`. `max_uses` stays in the schema, pinned to 1 for P6 V1. §7.1's batch becomes N requests presented in one composite prompt and N grants. | Zero new authority structure and zero weakening, but it does **not** deliver the ratified "P6 V1 multi-use grants = YES". This is a product reduction and needs the owner. |
| R2 — enumerated multi-action grant | One grant bound to a bounded, explicitly enumerated set of `(StepId, arguments_digest)` actions, `max_uses = count`, each action consumed once, `UNIQUE (step_id)` retained on uses. | Delivers the ratified multi-use and preserves exact arguments consent; authority is exactly the enumerated actions. But it is a new authority identity rule and a new authority-bearing table or column on both request and grant — precisely what the mandate forbids inventing. |
| R3 — multi-use restricted to identical approved arguments | One grant, one `arguments_digest`, `max_uses > 1` authorizes N distinct Steps whose canonical arguments equal that digest. | Needs no schema or identity change and delivers "multi-use = YES" literally, but it silently redefines §7.1's batch as an identical-arguments-only batch, and it corrects the ratified package's §6.4 rather than following it. Also produces N copies of one external effect from one prompt, which is not what the prompt appears to ask for. |
| R4 — scope-only matching | The grant matches any call inside its scope; `arguments_digest` is retained for audit only. | Forbidden outright: it weakens exact arguments consent and is an aggregate grant broader than the approved individual actions. |

## 6. Recommendation and the exact owner decision requested

**Recommendation: R2**, because it is the only route that delivers the ratified multi-use
without weakening any frozen consent property and without widening authority past the
actions the user actually saw. Its cost is one new authority binding, which must be ratified
explicitly rather than inferred.

If the owner prefers not to extend the grant identity, **R1** is the smallest safe fallback.
R3 is recorded and is not recommended. R4 is refused.

The owner is asked to settle one question, in one line:

> **Does one approval grant bind exactly one Step, or may it bind a bounded, explicitly
> enumerated set of Steps each with its own exact `arguments_digest`?**

- *Exactly one Step* → P6 implements R1; multi-use stays in the schema at `max_uses = 1`;
  §7.1's batch is realized as several requests in one composite prompt.
- *A bounded enumerated set* → P6 implements R2; migration 0005 gains the enumeration table;
  §7.1 is realized literally.

Nothing else in P6A–P6F is reopened by either answer: every other P6A item is resolved in
§4.2 above, and the ADR-0037 policy semantics, ADR-0040 bounds, crate ownership and retention
decisions are unaffected.

## 6a. Owner response — R2 approved, 2026-10-10

The owner answered with the second option and formalized it as an additional decision:

> **OWNER DECISION: APPROVE R2 — Enumerated Multi-Action Grant.**

with twelve ratified invariants and a ratified security boundary, both recorded verbatim in
[§0b of the owner decision package](P6-owner-decision-package.md). The gate's BLOCKER is
therefore lifted, and the authority model R2 requires is specified in §7 below.

One clarification the owner's answer settles that the gate deliberately left open: the gate
offered R2 as "a bounded, explicitly enumerated set of `(StepId, arguments_digest)` actions"
without fixing whether the set may be a subset of a larger request. Owner invariant 11
answers it — a partial approval is allowed, and the approved subset is recorded explicitly
and immutably. That is the shape specified in §7.5.

## 7. The R2 authority model, and the re-run proof

### 7.1 Definitions

| Term | Meaning |
| --- | --- |
| **Approval unit** | The object a human is shown and responds to. It covers 1 to 8 actions. |
| **Action** | One `StepId`, its exact canonical `arguments_digest`, and the structural scope shown for it. |
| **Grant** | The durable authority for exactly one approval unit. Its authority set *is* that unit's action set. |

### 7.2 Shared grant-level authority conditions

These are carried once on the request and once on the grant. Every member must be
compatible with all of them, and a member whose durable facts differ is refused. This is
owner invariant 10: a grant is one capability, one version, one plan revision, one registry
generation, one descriptor revision, one task, one expiry.

| Condition | Source of truth | Why it is shared |
| --- | --- | --- |
| `task_id` | `tasks.task_id` | A grant cannot authorize a different task (Approval §3.1, A4) |
| `capability_id`, `capability_version` | `step_capability_bindings` | Grants are for one capability, not a family (Approval §3.1) |
| `generation_id` | `step_capability_bindings` | Prevents a grant minted under an older generation |
| `descriptor_digest` | `step_capability_bindings` | Prevents a semantic descriptor change riding on an old grant |
| `plan_revision` | `task_steps.plan_revision` | A replacement plan invalidates the grant (Approval §4 point 2) |
| `expires_at` | host bound | A grant never outlives its request |
| `granted_by`, `auth_strength` | the authenticated seam | Who lent the authority |

### 7.3 Per-action properties

| Property | Source of truth | Rule |
| --- | --- | --- |
| `step_id` | `task_steps.step_id` | Must belong to `task_id`, must be a `CAPABILITY` step, must be bound in `step_capability_bindings` |
| `arguments_digest` | `task_steps.input_digest` | Must equal the durable canonical digest exactly. Recomputed at consumption, never trusted from the request |
| `scope` | capability-defined projection | The structural scope shown for *this* action; a member carries its own |

### 7.4 Normalization, digest and immutability

Owner invariant 8 requires all of these to be explicit:

- **Ordering.** Members are ordered by `step_id` ascending, byte-wise. `StepId` is a
  Crockford ULID, so byte order equals lexicographic order and needs no locale.
- **Action-set digest.** `action_set_digest` = SHA-256 over the canonical JSON of the
  ordered member array, each member `{"step_id":…,"arguments_digest":…,"scope":…}`, keys
  sorted, canonicalized under SCJ-1. It is the commitment to *which* actions were approved
  and is stored on both the request row and the grant row.
- **Duplicate rejection.** Two members with the same `step_id` are refused with
  `APPROVAL_ACTION_SET_DUPLICATE`.
- **Bound.** A member count outside 1..=8 is refused with
  `APPROVAL_ACTION_SET_BOUND_EXCEEDED`; more than 8 operations are split into another
  bounded approval unit or refused, never silently merged or expanded.
- **Incompatible shared conditions.** A member whose `capability_id`, `capability_version`,
  `generation_id`, `descriptor_digest` or `plan_revision` differs from the shared conditions
  is refused with `APPROVAL_ACTION_SET_INCONSISTENT`.
- **Immutability.** `approval_request_actions` and `approval_grant_members` carry no-update
  and no-delete triggers and accept no insert after their parent row is committed, mirroring
  the migration-0004 pattern. The set is fully determinate before approval (invariant 3).

### 7.5 Partial approval

An authenticated `GRANT` response may cover a **proper subset** of the request's actions.
The granted subset is written to `approval_grant_members` and is immutable thereafter, so
the granted authority is exactly the approved subset (invariant 11). Ungranted actions
receive no authority at all: they are not members, so no consume path can reach them, and
they remain requirable under a new approval unit. The subset is never widened after the
fact; a later wider approval is a new unit, not an edit.

### 7.6 `max_uses`

`max_uses` equals the number of granted members and is bounded by
`approval_grant_max_uses = 8`. It is therefore always ≤ the number of individually approved
Steps (invariant 7), and `uses_remaining` can only reach 0 by every listed Step having
consumed. An unlisted Step can never consume, because membership — not the counter — is what
authorizes it.

### 7.7 Consumption

The consume operation is atomic, idempotent and transaction-joinable, and it is the only
path that spends a grant:

```text
1. Load the grant by grant_id; require status ACTIVE.
2. Require now < expires_at.
3. Require uses_remaining > 0.
4. Require step_id to be a member of this grant (membership, not scope or digest match).
5. Validate the durable Step against the shared conditions and the member's
   arguments_digest, recomputing from task_steps.input_digest.
6. Require that no use row exists for step_id (UNIQUE(step_id)).
7. In one transaction: insert (grant_id, step_id) and decrement uses_remaining.
```

- A repeated consume for the same Step is a no-op with no second decrement (invariant 6,
  Approval §4.2).
- A Step already consumed under an earlier grant is refused under a second grant, because
  the use row survives (DC-1 second limb).
- A Step with no use row whose earlier grant was EXPIRED, REVOKED or DENIED **unused** may
  be re-approved, because no row exists (DC-1 first limb).
- The operation is joinable so that P8 can compose it with a dispatch intent in one
  transaction (ADR-0039). P6 does not create that intent.

### 7.8 Event routing under a multi-action unit

Approval lifecycle events carry exactly one `step_id` because
`ApprovalLifecyclePayloadV1` is a closed routing object. For a multi-action unit it is the
**leading member** — the member with the smallest `step_id` under the §7.4 ordering. It is
routing identity only, carries no authority, and is documented as such. Emitting one event
per member instead would multiply wakes and risk `max_events_per_transaction`, and would
change the 1:1 source-event-to-wake mapping that ADR-0030 froze.

### 7.9 Re-run of the twelve cases

| Case | Outcome under R2 |
| --- | --- |
| Single Step / single use | Allowed. One member, `max_uses = 1`. |
| Single Step / repeated consume | Rejected as a no-op; no second decrement. |
| Multiple Steps / same scope | Allowed **only** for Steps enumerated as members. Scope never adds a Step. |
| Multiple Steps / different arguments | Allowed; each member carries its own `arguments_digest`, recomputed from the durable Step. |
| Batch naming several Steps | Allowed; one unit, one prompt, `max_uses` = member count. §7.1 realized literally. |
| Expired unused grant, then reapproval | Allowed; no use row exists for the Step. |
| Revoked unused grant, then reapproval | Allowed; no use row exists for the Step. |
| Already consumed Step, then retry | Rejected; `UNIQUE(step_id)` blocks the second grant. |
| Plan revision and Step replacement | Rejected; `plan_revision` is a shared condition and the new Step is not a member. |
| Same Task, new Step | Rejected; a Step not enumerated is never authorized. |
| Cross-task replay | Rejected; `task_id` is a shared condition. |
| Deadline and policy revision changes | Rejected; `now < expires_at` and the evidence's `policy_revision_id`. |

Every case is now decided by durable state, and none requires weakening exact identity or
exact arguments consent. The frozen Approval §4 point 2 is preserved literally: the digest a
Step must match is the one carried on its own member row, which was shown to the human
before approval.

### 7.10 Consequences and nonclaims

R2 narrows nothing that the frozen text allowed and widens nothing the human did not see:
the authority of a grant is exactly the set of individually displayed actions. It does add
one authority-bearing membership table per side, which is why it required an owner decision
rather than an engineering inference. Migration 0005 is still not created here, and no
runtime exists.

## 8. Consequences for P6A

The gate **passed** on re-run under R2:

- ADR-0037 through ADR-0040 are marked **Accepted**.
- Migration 0005 is not created here. P6B creates it, from the conceptual shape in ADR-0040
  extended by the R2 membership tables.
- P6B through P6F are authorized to proceed sequentially under the already-granted
  long-running engineering instruction, once this P6A slice is committed and its exact-head
  CI is green.

## 9. Nonclaims

This gate claims no implementation. It creates no crate, no migration, no table, no
evaluator and no test. §1 through §6 are the record of a proof that found a real
contradiction; §6a through §7 are the record of the owner decision that resolved it and of
the model it selected.
