# P6 owner decision package — Policy and Approval

Status: **PROPOSED, NOT OWNER-RATIFIED** · Audit baseline: `318f15523cb94ff13e0c94da3d40788aaf2a1b6d` · Architecture: `serea-arch/2.6.0`

**No recommendation in this document is owner-approved until explicitly ratified.**

This is a documentation-only decision package. It creates no `serea-policy` crate, no
migration 0005, no policy/approval tables, no evaluator, no approval runtime, and no
provider dispatch. P8 remains the first phase permitted to invoke
`CapabilityProvider::invoke`. The companion
[P6 preimplementation audit](P6-preimplementation-audit.md) remains the record of the
original D1–D20 audit history and is not rewritten.

---

## 0. Owner response UX

> **Owner can ratify all recommended defaults by saying: `ACCEPT P6 RECOMMENDED PACKAGE`**

That phrase has **not** been said. Until it is, every entry in this document is a
proposal.

The package is deliberately built so one sentence ratifies the whole recommended
design. That one sentence **selects all six clusters R1–R6 and every published default
value in them**, including the six numeric or product defaults listed in §14.2. It is
not a partial acceptance: `approval_grant_max_uses = 8`, `max_active_policy_rules = 512`,
`max_retained_policy_revisions = 64`, `approval_grant_expiry_ms = 1800000`, multi-use
grants in P6 V1 = yes, and the `PERSONAL` persisted-summary ceiling are all adopted by
the single phrase.

An explicit owner override supersedes **only the named value**. If the owner were to reply
`ACCEPT P6 RECOMMENDED PACKAGE with approval_grant_max_uses = 3`, that reply would ratify
R1–R6 and change that one number; nothing else would move, and no other value would be
reopened. Overriding one of them is a one-line reply, not a re-opening of the design.

Earlier drafts of this section said the six values were "not settled" by the one
sentence. That reading was a drafting inconsistency and is corrected here: the sentence
adopts the published defaults, and an override is the only way to move one. §14.2 now
states the same rule in the same words. **No ratification is recorded here.**

---

## 1. How this package was derived

The first audit asked twenty questions and stopped. That was the correct conservative
move, but it produced a list that reads as twenty unrelated opinions. The reduction
below is analytical, not editorial: each decision is re-derived from the currently
accepted contract set, and each is placed in exactly one of four dispositions.

| Disposition | Meaning |
| --- | --- |
| `CLOSED_BY_EXISTING_CONTRACT` | One answer is already entailed by accepted protocols/ADRs, or by a mechanically enforced workspace gate. The first audit was conservative, not wrong. |
| `RECOMMENDED_OWNER_CHOICE` | A genuine preference remains. One recommendation is given with reasons. |
| `DEFER_TO_P8` | The choice only binds a phase that does not exist yet. P6 must not be blocked on it. |
| `IMPLEMENTATION_DETAIL_NOT_OWNER_CHOICE` | The choice is a Rust/API design fact, not a product preference. The owner is not asked. |

Two reductions are stronger than re-reading prose, and they are worth stating up front
because they do most of the work.

1. **`serea-capability` cannot depend on `serea-policy`.** `tests/workspace_smoke.py:370`
   fails the build if it does. The `CAP --> POLICY` edge that `docs/architecture/03-crate-map.md:108`
   describes is therefore not a design option but a workspace-gate violation, and the
   audit's preferred TaskEngine orchestration needs one smoke-test amendment, described
   in Cluster F.
2. **`serea-storage` refuses PRIVATE ordinary rows.** `crates/serea-storage/src/task.rs:94-99`
   returns `AtRestProtectionUnavailable` for `DataClass::Private`, and
   [ADR-0022](../decisions/ADR-0022-durable-private-data-at-rest.md) remains Proposed. A
   PRIVATE `plain_summary` column in `approval_requests` is therefore blocked today by
   code, not by preference. Cluster D recommends the design that does not need it.
   The refusal lives in the per-domain writer helper (`ordinary_class`), duplicated at
   `task.rs:94` and `outcome.rs:166`; there is no store-wide gate, so P6B's approval
   writer must apply the same check itself.

---

## 2. Decision dependency graph

Edges read "must be decided after, because it is constrained by".

```text
D1  fallback ──────────┬──> D2 precedence ──> D3 deny-override ──┐
                      │                                          │
D5  provenance ───────┘                                          │
                                                                 v
D6  revision identity <── D19 source of truth <── D7 hot update  A. policy semantics
        │                                    │
        │                                    └──> P8 revalidation (Cluster F)
        v
D8  approval identity <── D9 scope/uses <── D10 expiry ──> D12 revocation
        │                                                   │
        └──> D11 consume/dispatch ──> DEFER_TO_P8            v
                                                        C. approval identity
D14 summary/privacy <── D15 response seam <── D13 duplicate responses
        │                                                   │
        └──> D17 retention <── D18 bounds                    v
                                                   D. lifecycle / response / privacy
D4  policy input <── D20 crate ownership <── all of the above
```

Independent owner choices are exactly six. Everything else is either entailed,
deferred, or mechanical.

---

## 3. D1–D20 disposition

| Decision | Cluster | Disposition |
| --- | --- | --- |
| D1 fallback | A | `CLOSED_BY_EXISTING_CONTRACT` |
| D2 precedence | A | `CLOSED_BY_EXISTING_CONTRACT` |
| D3 deny-override | A | `RECOMMENDED_OWNER_CHOICE` |
| D4 `PolicyInputV1` | F | `IMPLEMENTATION_DETAIL_NOT_OWNER_CHOICE` |
| D5 provenance | A | `CLOSED_BY_EXISTING_CONTRACT` |
| D6 revision identity | B | `RECOMMENDED_OWNER_CHOICE` |
| D7 hot update | B | `CLOSED_BY_EXISTING_CONTRACT` |
| D8 approval binding | C | `CLOSED_BY_EXISTING_CONTRACT` |
| D9 scope and uses | C | `CLOSED_BY_EXISTING_CONTRACT` |
| D10 expiry | C | `CLOSED_BY_EXISTING_CONTRACT` |
| D11 consume/dispatch | D | `DEFER_TO_P8` |
| D12 revocation | D | `RECOMMENDED_OWNER_CHOICE` |
| D13 duplicate responses | D | `CLOSED_BY_EXISTING_CONTRACT` |
| D14 summary and request transaction | D | `RECOMMENDED_OWNER_CHOICE` |
| D15 response seam | D | `IMPLEMENTATION_DETAIL_NOT_OWNER_CHOICE` |
| D16 cancellation/deadline | D | `IMPLEMENTATION_DETAIL_NOT_OWNER_CHOICE` |
| D17 retention and deletion | E | `CLOSED_BY_EXISTING_CONTRACT` |
| D18 bounds | E | `RECOMMENDED_OWNER_CHOICE` |
| D19 source of truth | B | `CLOSED_BY_EXISTING_CONTRACT` |
| D20 crate ownership | F | `RECOMMENDED_OWNER_CHOICE` |

Totals: 20 original · 10 closed · 1 deferred · 3 implementation detail · 6 genuine owner
clusters.

---

## 4. Cluster A — policy semantics

### 4.1 Scope

Consolidates **D1, D2, D3, D5**. These are one decision because the fallback, the
precedence, the conflict rule and the provenance mapping all fix the same function: what
a rule set decides about a prepared action.

### 4.2 The three candidate models

**MODEL A — built-in class defaults first, explicit rules override, DENY always wins.**
Fail-closed: yes, via the fallback. Predictability: high, if the default table is fixed
and versioned. Authoring complexity: low. Accidental privilege: low, because a missing
rule is not an allow. Auditability: high. Effect on `OBSERVE`/`LOCAL_STATE`: exactly
`Allow`, which is what [Approval Protocol §7](../protocols/05-approval-protocol.md)
measures as success. Effect on `SCHEDULED`/`SYSTEM`: defaults apply unchanged; no
automatic allowance is created for them. P8 stale rechecks: unaffected — a recheck
re-runs the same pure function on current durable state.

**MODEL B — explicit rules only, unmatched becomes `RequireApproval`.**
Rejected. It makes [Policy Protocol §4.1](../protocols/04-policy-protocol.md) dead text,
and it makes the "ordinary day produces zero prompts for read-only work" measure in
Approval §7 impossible, because every `gmail.messages.list` call would prompt. It also
contradicts Policy §4.2 steps 5 and 6, which are a class default *followed by* a fallback.

**MODEL C — class defaults as synthetic rules with deterministic priority.**
Superficially like A, but it makes the default table mutable data, so an error in rule
priority could promote `CREDENTIAL` above an explicit deny. It also means an admin
revision can change the meaning of every capability at once. Rejected for those two
properties, not for its arithmetic.

**Recommended: MODEL A.** The class-default table stays a fixed, code-level, versioned
constant. Explicit rules are the only durable rule data.

### 4.3 Finite rule precedence

Policy §5 already fixes the rule ordering: `priority` descending, then `rule_id`
ascending. That is total and mechanically testable, provided `rule_id` is a unique
string within one revision. The "most specific first" wording in §4.2 is the *family*
order of the evaluation stages, not a specificity score over rules; the two are
different mechanisms and both are kept.

Recommended match dimensions, kept deliberately few. Every field is optional, and an
absent field matches everything, exactly as §5 states:

| Dimension | Values |
| --- | --- |
| `capability_id` | exact `CapabilityId` only — no glob in P6 |
| `risk_class` | one `RiskClass` |
| `side_effect_class` | one `SideEffectClass` |
| `authorization` | one `Authorization` |
| `automation_context` | one `AutomationContext` |
| `requested_by` | one `RequestedBy` |
| `enabled` | boolean |

No `data_class` dimension. It adds a field the model can influence indirectly, and the
egress question is already answered by Data Classification §5, which is a hard matrix
rather than a policy rule. No glob, no regex, no negation, no expression language. Adding
a glob later is an architecture-minor addition; removing one is not.

### 4.4 DENY semantics

Recommended: **any matching explicit DENY overrides every ALLOW, regardless of
priority.** Reasons:

- Approval §4 point 7 and invariant `A3` already say a grant cannot override a deny, and
  call deny rules absolute. Making that true *between rules* is the same guarantee one
  level down.
- The `priority DESC, rule_id ASC` ordering becomes the mechanism by which an admin writes
  an emergency deny that wins. A rule that can be out-ranked by a stale high-priority rule
  is not an emergency stop.
- Deny-override removes the entire "accidental high-priority allow" class.

Scope of the override, explicitly:

| Applied to | Recommendation |
| --- | --- |
| Explicit rule vs explicit rule | DENY wins |
| Explicit rule vs built-in class default | DENY wins over a default ALLOW |
| Built-in default DENY vs explicit ALLOW | DENY wins |
| Task `policy_class` ceiling | Already a deny, outside the rule ordering; unchanged |
| Capability disabled/removed overlay | Not a policy rule at all. `P8` of Policy §9 holds: a rule cannot re-enable it |
| Approval response | A grant can never override a final DENY. Unchanged |

The cost is that "deny everything except X" is not expressible: you write
`capability_id = X` as the allow and get the default deny for free. This is the
acceptable direction to be inexpressive in.

### 4.5 `RequestedBy` and automation context

`RequestedBy` is provenance, never authority (Capability §4.1). The mapping to a finite
policy context is one row per variant, with no self-upgrade:

| `RequestedBy` | `AutomationContext` | Default posture |
| --- | --- | --- |
| `USER` | `INTERACTIVE` | Ordinary defaults |
| `MODEL` | `INTERACTIVE` | Identical to `USER`. The model has no context of its own |
| `SCHEDULER` | `SCHEDULED` | Ordinary defaults, plus a task-kind restriction: a `SCHEDULED` task may not be created with a `policy_class` above `LOCAL_STATE` unless an explicit durable rule allows it |
| `PROACTIVE_WATCHER` | `PROACTIVE` | `OBSERVE` and `LOCAL_STATE` only, per `P5` and [ADR-0016](../decisions/ADR-0016-proactive-watcher-is-read-only.md). Raises no approvals |
| `SYSTEM` | `SYSTEM` | Narrow: internal maintenance only, never `CREDENTIAL` or `DESTRUCTIVE`, and never able to raise an approval for an effecting capability |

`SYSTEM` gets **no god-mode**. It receives a smaller default posture than `INTERACTIVE`,
which is the opposite of the intuitive default and the correct one. The single documented
`SYSTEM` bypass that exists today is the duplicate-window bypass in
[Bounds Protocol §5.1](../protocols/10-bounds-protocol.md), which is a P8 execution rule
with its own event, not a policy posture.

The `AutomationContext` enum is host-authored, host-supplied, and never derived from model
output. A caller cannot raise its own context: the context is a function of the durable
Task kind and the trusted requester, both of which are host-owned.

### 4.6 Compatibility with accepted ADRs

| ADR | Verdict |
| --- | --- |
| ADR-0016 (proactive read-only) | Preserved and strengthened: `PROACTIVE` is the only restricted context |
| ADR-0009 (bounded task-bound grants) | Preserved; grants remain additive only |
| ADR-0035 (`PreparedActionV1`) | Preserved; `PolicyInputV1` is built from it without mutation |
| ADR-0036 (P5/P6/P8) | Preserved; P6 returns a typed authorization outcome |

---

## 5. Cluster B — policy revision and source of truth

### 5.1 Scope

Consolidates **D6, D7, D19**.

### 5.2 Revision identity

Three options were considered: monotonic `revision_id` only, content digest only, or both.

Digest alone is insufficient: it cannot order revisions, it cannot distinguish an
identical rule set re-activated at a different time, and it gives the activation pointer
nothing monotonic to advance. `revision_id` alone is insufficient: it cannot detect
corruption or a hand-edited database, and it cannot prove that the rules an evaluation
used are the rules now on disk.

**Recommended: both.** `revision_id` is a monotonic integer that orders revisions and
drives the activation pointer. `rules_digest` is a SHA-256 over the canonical JSON of the
revision's complete rule array and lets any reader — including a P8 recheck — prove that a
durable snapshot it is holding is byte-identical to the one an evaluation named. This is the
same shape migration 0004 already uses for registry generations
(`capability_registry_generations` carries `manifest_digest` and `schema_catalog_digest`
alongside `generation_id`), so it is consistent rather than novel.

| Concern | Monotonic only | Digest only | Both |
| --- | --- | --- | --- |
| Ordering and activation pointer | Yes | No | Yes |
| Corruption detection | No | Yes | Yes |
| Historical evidence | Weak | Strong | Strong |
| Deterministic P8 revalidation | Partial | Strong | Strong |
| Restart | Yes | Yes | Yes |

### 5.3 Source of truth

Three options: SQLite only after import; host config only; config + SQLite as dual live
authorities.

Option C is rejected as split-brain. Two live authorities means the engine's answer can
depend on which one a code path happened to read, which breaks determinism (Policy `P1`)
in a way no test can close.

**Recommended: SQLite is the sole runtime authority.** Host configuration is a *trusted
import path*, not a live rules store. The admin flow is the one migration 0004 already
established for the registry, reused unchanged:

```text
trusted local admin request (host only, never model, never ordinary device settings)
  -> validate structurally (closed rule set, bounded, no glob/negation)
  -> create immutable revision rows + rules_digest, unactivated
  -> one transaction: activate (advance singleton pointer) + append POLICY_CHANGED
  -> return the new revision identity
```

SQLite becomes the runtime authority the moment the transaction commits. No configuration
file is read at evaluation time, so a partial edit of config while the host runs cannot
change a decision.

### 5.4 Hot update

The published direction is already frozen: Approval §4.1 requires the current policy
decision before resuming, ADR-0036 requires P8 to recheck, and Policy §8 requires live
overlay to win. What D7 actually asks is the atomicity point, and that is a P8 question.

**Recommended P6 semantics:** evaluation runs against one immutable revision and records
that revision's identity in the authorization evidence. P6 never re-evaluates a decision
it already made. P8 must independently re-evaluate against the *current* pointer before
dispatch, and a mismatch is a refusal, not an automatic continuation.

| R2 activates | Correct behaviour |
| --- | --- |
| Before the approval response is committed | The new revision governs the resume path; the request is re-evaluated |
| After the response, before P8 dispatch | P8 recheck refuses and P6 re-evaluates on a new revision |
| After dispatch intent commits | Nothing to do; the intent already named its revision |

Whether P6's authorization is "advisory evidence only": yes. P6 output is evidence about
state at a named revision, never a permission. The exact transaction that joins P8's
recheck to dispatch intent is a P8 contract decision (Cluster F).

### 5.5 Durability and P8 impact

| Concern | Consequence |
| --- | --- |
| Durability | `policy_revisions` plus `policy_rules` plus a singleton `policy_state` pointer survive restart, matching migration 0004's shape |
| Audit | `POLICY_CHANGED` before/after, actor, reason, per Policy §7 |
| P8 | P8 reads the current pointer and the revision named by the evidence; nothing else |

---

## 6. Cluster C — approval identity and scope

### 6.1 Scope

Consolidates **D8, D9, D10**.

### 6.2 Minimum approval identity

The frozen minimum is ApprovalId, TaskId, StepId, CapabilityId + version,
`arguments_digest`, scope, expiry, use ceiling, and task binding. The audit's open question
was whether P5's registry facts join it. They must, and the reason is mechanical:
P5 already pins them and the grant must not survive their change.

| Field | Classification | Reason |
| --- | --- | --- |
| `ApprovalId` | `MUST_BIND` | Grant identity |
| `TaskId`, `StepId` | `MUST_BIND` | Task binding is one of the six bounds |
| `CapabilityId`, `capability_version` | `MUST_BIND` | Grants are for one capability, not a family |
| `arguments_digest` | `MUST_BIND` | The consent is to exact arguments |
| `scope` | `MUST_BIND` | Narrowest scope that permits the call |
| `plan_revision` | `MUST_BIND` | A replacement plan invalidates the grant; P5's Step pinning already implies it |
| registry `generation_id` | `MUST_BIND` | Prevents a binding minted under an older generation from authorizing a newer one |
| `descriptor_digest` | `MUST_BIND` | Prevents a semantic descriptor change from riding on an old grant |
| `ProviderId`, `ImplementationId` | `TRANSPARENCY_ONLY` | See §6.3 |
| capability title | `TRANSPARENCY_ONLY` | Rendered from the descriptor, not stored as authority |
| `grant_digest` | `TRANSPARENCY_ONLY` | Audit convenience |

### 6.3 ProviderId and ImplementationId

Recommended: **not authorization identity.** The Step already pins the implementation
(Task Protocol §3; ADR-0034), and the audit correctly cites that P5 guarantee. A grant that
names a Step is already bound to that Step's pinned implementation. Duplicating the
implementation into the grant adds a second field that must be kept consistent and gives a
false impression that a grant could survive a legitimate implementation change — it cannot,
because the Step pin does not change.

Threat-model check:

| Threat | Blocked by |
| --- | --- |
| rootless → root implementation switch | The Step pin; `OPTIONAL_ROOT`/`REQUIRES_ROOT` is descriptor authority, not a runtime swap. A bound implementation never changes automatically |
| Provider replacement | New provider means new descriptor digest and new generation, both bound |
| Exact descriptor revision pin | `descriptor_digest` is bound |
| Same logical capability, new implementation | A new Step is required; a grant cannot follow it |

They remain in the request/grant rows as `TRANSPARENCY_ONLY` for audit rendering.

### 6.4 Scope

Recommended: **capability-defined exact structural projection, no wildcard.** This is
already the frozen rule; D9 asks only whether P6 V1 implements the projection or only
exact-action equality. Recommendation: implement structural projection as §3.2 defines it,
because the alternative — exact action equality — would refuse the batch case in §7.1 that
the protocol explicitly describes, and would make `max_uses: 3` unusable. A generic broader
scope system is rejected: it is the "allow everything" shape that A2 forbids.

### 6.5 Multi-use grants

Recommended: **`max_uses` from day one, with an idempotent per-Step use record.** The
atomicity requirement is already frozen (Approval §4.2), so the incremental cost of `N > 1`
is one integer and a uniqueness constraint on `(grant_id, step_id)` — which one-use needs
anyway. Deferring multi-use would mean building the table twice.

Compatibility path if the owner prefers one-use V1: the same schema carries
`max_uses = 1`, and a later phase widens the bound with no migration. That is the one
genuinely deferrable item in this cluster, and it is offered as an override, not a
recommendation.

### 6.6 Expiry

`now < expires_at` is frozen. `approval_request_expiry_ms = 1800000` is already a named
bound in [Bounds Protocol §2](../protocols/10-bounds-protocol.md), with scope and exhaustion
behaviour, so D10 is closed as written. Recommended: keep it a bound, set by trusted host
policy like every other bound, with the same lowering-is-free, raising-is-admin rule from
Bounds §3.

Bounds to publish: `approval_request_expiry_ms = 1800000` (unchanged), plus a new
`approval_grant_expiry_ms = 1800000` (grant and request share the 30-minute horizon so a
grant cannot outlive the request it came from), plus
`approval_grant_max_uses = 8` (a bound on `max_uses`; see Cluster E).

---

## 7. Cluster D — approval lifecycle, response and privacy

### 7.1 Scope

Consolidates **D11, D12, D13, D14, D15, D16**.

### 7.2 State machine

Two independent state variables, deliberately not merged:

| Object | States |
| --- | --- |
| Approval request | `PENDING` → `APPROVED` / `DENIED` / `EXPIRED` |
| Approval grant | `ACTIVE` → `EXHAUSTED` / `EXPIRED` / `REVOKED` |

No new wire enum is invented. `PENDING`, `APPROVED`, `DENIED`, `EXPIRED` are the protocol's
existing request statuses. `ACTIVE`, `EXHAUSTED`, `EXPIRED`, `REVOKED` are internal Rust
states on a type that never crosses a wire, so they require no protocol change.

`PARTIALLY_CONSUMED` is not a state. It is `uses_remaining < max_uses`, a counter read.

A request `REVOKED` state was previously listed here. It is removed, because §7.4 places
revocation on the grant only: under that recommendation a request is never `REVOKED`, so
the state is unreachable rather than merely unused, and the §7.3 matrix is keyed on the
grant's state instead. The `approval_requests.status` column in §8.4 keeps the value so the
stored domain is not narrowed, but no P6 transition may write it.

### 7.3 Duplicate response matrix

`approval_id` is unique per host. Responses are keyed by `(approval_id, authenticated
principal, authenticated session)` and applied inside one Store transaction that re-reads
the current request state. A response whose target is not `PENDING` is dropped.

| Current request state | Incoming `GRANT` | Incoming `DENY` |
| --- | --- | --- |
| `PENDING` | Commit `APPROVED`, mint at most one grant | Commit `DENIED`, no grant |
| `APPROVED`, grant unused | **Idempotent success.** Return the existing grant identity; no second grant | **Typed refusal.** `APPROVAL_RESPONSE_CONFLICT` |
| `APPROVED`, grant exhausted | **Idempotent success** with the same terminal reason | **Typed refusal.** `APPROVAL_RESPONSE_CONFLICT` |
| `APPROVED`, grant `REVOKED` | **Typed refusal.** `APPROVAL_REVOKED` | **Typed refusal.** `APPROVAL_REVOKED` |
| `DENIED` | **Typed refusal.** `APPROVAL_RESPONSE_CONFLICT` | **Idempotent success.** Return the existing denial |
| `EXPIRED` | **Typed refusal.** `APPROVAL_REQUEST_EXPIRED` | **Typed refusal.** `APPROVAL_REQUEST_EXPIRED` |
| Task `CANCELLED` | **Typed refusal.** `TASK_NOT_APPROVABLE` | **Typed refusal.** `TASK_NOT_APPROVABLE` |
| Task `FAILED` | **Typed refusal.** `TASK_NOT_APPROVABLE` | **Typed refusal.** `TASK_NOT_APPROVABLE` |

Every cell is one of exactly three typed outcomes: idempotent success, typed refusal, or —
for a first response to `PENDING` — a commit. No free-text behaviour, no silent success, no
second grant. The Device Protocol §5.2 rules ("a response whose `approval_id` is not
`PENDING` is dropped") are the wire-level expression of this matrix and need no change.

Idempotency is by durable state, not by message identity: a replayed frame and a
re-submitted response produce the same result because the request row already says so.

The `APPROVED`, grant `REVOKED` row was previously missing and is the reason this matrix
must be keyed on the grant as well as the request. Revocation is grant-only (§7.4), so a
revoked consent leaves the request in `APPROVED`. Without this row a repeated `GRANT` would
take the "`APPROVED` (unused)" idempotent-success path and hand back the identity of a grant
that has been withdrawn. The row is required to make §7.4 implementable as written.

### 7.4 Revocation

Recommended: revocation is an explicit, durable, terminal transition on the **grant**, not
on the request.

- **Who may revoke:** the authenticated user or admin, through the same trusted response
  seam as an approval response, plus the local admin surface.

  **Unresolved seam gap, recorded rather than designed around.** The seam type in §7.7
  carries `decision: ResponseDecision` with only `GRANT | DENY`, and the device wire type
  in Device Protocol §5.2 carries only `GRANT | DENY | DEFER`. Neither can express a
  revocation, so "through the same trusted response seam" is not currently realizable:
  there is no value a caller could put in that field to mean *revoke*. P6A must settle
  this by either (a) adding a `REVOKE` variant to the internal `ResponseDecision` and
  stating that the device surface is added later as its own change, or (b) narrowing the
  recommendation to "the local admin surface only in P6 V1, with device revocation
  deferred". The design point that revocation is grant-only, human-initiated, durable and
  terminal is unaffected either way; only the transport is open.
- **Task cancellation does not revoke.** A cancelled task makes its grants unusable — P8
  refuses a cancelled task — but the grant row keeps its `ACTIVE` state and is removed by
  the task-deletion cascade. Conflating the two would make "revoked" mean both "a human
  withdrew consent" and "the task ended", and then the audit trail cannot answer "who
  withdrew this and when".
- **A policy change does not revoke either.** It is a revalidation outcome: the grant stays
  `ACTIVE`, and P8's recheck refuses. Revocation is a human act with a human actor.
- **Partial use:** revoking a partially consumed grant is legal; already-consumed uses are
  historical facts and are not undone. Future uses are refused.
- **Events:** `APPROVAL_REVOKED` is a **Proposed** new `EventKind`. It does not exist today
  and adding it is an architecture-minor addition per Protocol Index §4.2 rule 4. The
  payload carries only `approval_id`, `task_id`, `step_id`, `grant_id`, actor class,
  authenticated device/session, and `revoked_at`.

### 7.5 Approved request projection

Recommended minimal durable projection. Everything here is metadata, and nothing is
untaken from model output:

| Field | Why |
| --- | --- |
| `approval_id`, `task_id`, `step_id` | Identity and task binding |
| `capability_id`, `capability_version` | Which capability |
| `risk_class`, `side_effect_class`, `authorization`, `data_class` | The classified facts, for rendering and for policy re-derivation |
| `requested_by` | Provenance |
| `arguments_digest` | Consent is to exact arguments |
| `scope` | The authorized structural projection |
| `descriptor_digest`, `generation_id` | Prevent descriptor/generation drift |
| `raised_at`, `expires_at` | Expiry, bounded by the named bound |
| `max_uses` | Use ceiling |
| `status`, terminal `reason_code` | Durable lifecycle |
| summary and preview | See §7.6 — **not stored as raw arguments** |

`arguments` themselves are not stored in the approval row. The durable Step already holds
the canonical arguments in the blob store, referenced by `input_digest`, and P8 recomputes
the digest from there (Approval §4.1). Storing a second copy creates a second thing to keep
consistent and a second place for a credential-shaped value to land.

### 7.6 Summary source and privacy ceiling

**Source: capability-specific host summary builder.** Model-written text is never
authoritative; this is already frozen (Approval §2.1). Between the three candidates, the
generic host summary and the capability-specific builder, the capability-specific builder is
recommended because §2.1 requires the summary to state "the exact object affected", which
only a builder that knows the capability's object model can do.

**If a safe summary cannot be built:** refuse the request. `APPROVAL_SUMMARY_UNAVAILABLE`
with a typed reason code. A generic metadata-only prompt is rejected as a fallback because
it defeats the specific-enough-to-consent property and would silently allow an action the
user cannot evaluate. Silence never becomes consent.

**Privacy ceiling, per destination:**

| Destination | Maximum class |
| --- | --- |
| Approval summary persisted in SQLite | `PERSONAL` |
| Approval summary rendered to the device | `PERSONAL`, redacted per Data Classification §6 |
| `arguments_preview` persisted | **Not persisted at all**; derived from the Step's blob at render time |
| Event payload | `PERSONAL`, metadata only |
| Notification | `PERSONAL`, redacted; no inline actions |
| Any row, any column | `SECRET` and `CREDENTIAL` are refused by `serea-storage` mechanically |

`CREDENTIAL` never appears anywhere. This is not a convention: `serea-storage` refuses
`SECRET`/`CREDENTIAL` (`ClassRefused`) and refuses `PRIVATE` ordinary rows
(`AtRestProtectionUnavailable`, `crates/serea-storage/src/task.rs:94-99`). That refusal is why
the persisted summary ceiling is `PERSONAL` — a PRIVATE column would fail at runtime today
and would silently become permitted the day ADR-0022 ships, which is a security-relevant
change made by an unrelated ADR.

The `CREDENTIAL`/`SECRET`/`PRIVATE` refusals are per-domain writer checks
(`ordinary_class` at `task.rs:94` and `outcome.rs:166`; event refusals at `event.rs:90-96`),
not a store-wide gate. P6B's approval writer must apply them itself.

### 7.7 Authenticated response seam

P6 receives an internal, typed, non-wire seam. It is not JSON from the device, and a device
frame is never self-authenticating.

```rust
pub struct AuthenticatedApprovalResponseV1 {
    pub approval_id: ApprovalId,
    pub decision: ResponseDecision,        // GRANT | DENY
    pub principal: AuthenticatedPrincipal, // PairingPrincipal | AdminPrincipal
    pub device_id: DeviceId,
    pub session_id: SessionId,
    pub authentication_strength: AuthenticationStrength, // SESSION | ELEVATED_CONFIRMED
    pub confirmed_at: EpochMillis,
    pub elevated_confirmation: Option<ElevatedConfirmationRef>, // digest handle only
    pub requested_max_uses: Option<u32>,
    pub requested_expires_at: Option<EpochMillis>,
}
```

`ElevatedConfirmationRef` carries a digest of the attestation, never biometric material —
consistent with Data Classification §3, which puts biometric templates in the OS/TEE only.
`requested_max_uses` and `requested_expires_at` are the ceiling proposals from Device
§5.2; P6 **clamps** them to the request's own bounds and refuses a response that exceeds
either. A device cannot widen an approval by editing the JSON.

The seam is declared in `serea-policy` so the policy crate owns the trust contract for its
own input; the device wire type stays in `serea-protocol` where it already is.

### 7.8 Cancellation and deadline while waiting

Recommended: **no revocation, no Task mutation, no orphaned wake.**

- Task cancellation is terminal from any non-terminal state (Task Protocol §4.2), so
  cancellation is already handled by the Task Engine. P6 does not need a cancel hook, and
  adding one would give the policy crate a reason to reach back into Task state.
- Deadline: the task wall clock is **paused** in `WAITING_APPROVAL` (Bounds §6.2), so no
  deadline fires while waiting. The approval's own `expires_at` is the only clock that
  matters.
- On expiry, P6 marks the request `EXPIRED`, emits `APPROVAL_EXPIRED`, and lets the existing
  ADR-0030 wake handoff drive the Task transition. No new wake kind.

This removes the coupling that D16 implied: P6 needs no cancellation path at all, because
the Task Engine's cascade plus P8's pre-dispatch recheck make the authority unusable without
P6 having to know the task was cancelled.

---

## 8. Cluster E — storage, retention and bounds

### 8.1 Scope

Consolidates **D17, D18**.

### 8.2 Retention and deletion

Recommended: cascade with the Task, retain nothing independently.

| Table | Deletion on task delete | Rationale |
| --- | --- | --- |
| `policy_revisions` | **Not task-scoped; never cascades** | Policy history is a host audit artifact, not a task artifact. Event Protocol §8 already gives `POLICY_CHANGED` one year and states that this outliving its task is intentional |
| `policy_rules` | **Never cascades** | Belongs to an immutable revision |
| `approval_requests` | `ON DELETE CASCADE` | Derived task state |
| `approval_grants` | `ON DELETE CASCADE` | Task-bound authority; a grant for a deleted task must not linger |
| `approval_grant_uses` | `ON DELETE CASCADE` | Derived from the grant and the task |

Event content is never given a task/step foreign key (ADR-0026), so nothing here changes
event behaviour. Event retention classes already exist and are sufficient; no P6-specific
event retention is added.

Growth bound: `max_retained_policy_revisions` caps the policy tables, and task retention
already caps everything task-scoped.

### 8.3 Bounds

Existing relevant bounds are used first, and none is duplicated:

| Existing bound | Value | Reused for |
| --- | --- | --- |
| `max_pending_approvals_per_task` | 5 | Pending approvals per task — unchanged |
| `approval_request_expiry_ms` | 1800000 | Request expiry — unchanged |
| `max_event_payload_bytes` | 32768 | Event payload ceiling — unchanged |
| `max_events_per_transaction` | 16 | Atomicity composition — unchanged |
| `task_retention_days` | 30 | Task-scoped approval rows — unchanged |

Four new bounds are proposed, because each one guards a resource no existing bound names:

| Proposed bound | Proposed default | Rationale |
| --- | --- | --- |
| `max_active_policy_rules` | 512 | Bounds the rule table an admin can build and therefore the candidate set per evaluation. 512 is far above any plausible hand-authored set and keeps a full-table scan inside one transaction. It is a capacity guard, not a design target |
| `max_retained_policy_revisions` | 64 | Bounds growth of `policy_revisions`/`policy_rules` while keeping enough history for the one-year `POLICY_CHANGED` audit class |
| `approval_grant_expiry_ms` | 1800000 | A grant must not outlive its request; equal horizons make that structural |
| `approval_grant_max_uses` | 8 | Caps `max_uses`. The protocol's own batch example is 3; 8 is a ceiling on `max_uses` per P6, not a promise that 8 will be used |

Two further values are **not** proposed as bounds and should not be added:

- **A per-evaluation candidate cap.** `max_active_policy_rules` already bounds it.
- **A summary byte cap.** `max_event_payload_bytes` already bounds the event, and
  `PlainSummary`'s `Label` category refuses control characters and multi-line content.
  Stated precisely: `PlainSummary` has **no length ceiling at all** — P1's
  `MAX_VALUE_LENGTH = 4096` was retracted by owner decision and never replaced
  (`docs/plans/P1-closure.md`, ADR-0020, ADR-0023). A summary longer than
  `max_event_payload_bytes` is therefore persistable and fails only at event append,
  where `event.rs:101-103` returns `EventPayloadTooLarge` and the whole request
  transaction rolls back. That is fail-closed but opaque to the user. A byte bound is
  not recommended here because there is no measured resource behind a number, but P6A must
  state this explicitly rather than imply that `Label` validation bounds size.

If the owner wants different numbers, each is a one-line override.

### 8.4 Migration 0005 conceptual schema

**Not a DDL proposal and not implemented.** This is the shape the implementation phase
would build, restricted to what P6 must survive restart.

```sql
-- Immutable policy revision header. Activation is a separate, guarded transition.
policy_revisions (
  revision_id      INTEGER PRIMARY KEY AUTOINCREMENT,   -- monotonic, D6
  rules_digest     TEXT NOT NULL,                       -- sha256 over canonical rule array
  activated_at_ms  INTEGER,                             -- NULL until activated; write-once
  actor_class      TEXT NOT NULL,                       -- closed ActorKind subset
  actor_id         TEXT NOT NULL,
  reason_code      TEXT NOT NULL
)

-- Normalized typed rules. One row per rule per revision; no globs, no expressions.
policy_rules (
  revision_id            INTEGER NOT NULL REFERENCES policy_revisions(revision_id),
  rule_id                TEXT NOT NULL,                 -- stable, unique within a revision
  priority               INTEGER NOT NULL,
  capability_id          TEXT,                          -- exact or NULL (matches all)
  risk_class             TEXT,                          -- exact or NULL
  side_effect_class      TEXT,                          -- exact or NULL
  authorization          TEXT,                          -- exact or NULL
  automation_context     TEXT,                          -- exact or NULL
  requested_by           TEXT,                          -- exact or NULL
  decision               TEXT NOT NULL,                 -- ALLOW | DENY | REQUIRE_APPROVAL | REQUIRE_HANDOFF
  reason_code            TEXT NOT NULL,
  enabled                INTEGER NOT NULL,              -- 0 | 1
  PRIMARY KEY (revision_id, rule_id),
  UNIQUE (revision_id, priority, rule_id)
)

-- Singleton current pointer. This is the only live authority. Mirroring migration
-- 0004's capability_registry_state requires more than the FK: 0004 also carries a
-- no-delete trigger (0004:21), an advances-only trigger (0004:214), and a
-- requires-activated-generation trigger (0004:208). The conceptual shape below omits
-- them on purpose -- the triggers ARE the "pointer only advances" guarantee that 16.3
-- tests, and without them a direct SQL edit of active_revision_id succeeds.
policy_state (
  singleton           INTEGER PRIMARY KEY CHECK (singleton = 1),
  active_revision_id  INTEGER REFERENCES policy_revisions(revision_id) ON DELETE RESTRICT
)
-- P6B must also add: policy_state_no_delete, policy_state_advances_only, and
-- policy_state_requires_activated_revision, each a BEFORE UPDATE/DELETE trigger
-- modelled on the 0004 lines cited above, plus a STRICT table type and a
-- sha256-shape CHECK on rules_digest as 0004:6-11 already does for registry digests.

-- Durable approval request. Metadata only; no raw arguments.
approval_requests (
  approval_id       TEXT PRIMARY KEY,                   -- apr_ + ULID
  task_id           TEXT NOT NULL REFERENCES tasks(task_id) ON DELETE CASCADE,
  step_id           TEXT NOT NULL,
  plan_revision     INTEGER NOT NULL,
  capability_id     TEXT NOT NULL,
  capability_version TEXT NOT NULL,
  generation_id     INTEGER NOT NULL,
  descriptor_digest TEXT NOT NULL,
  provider_id       TEXT,                               -- transparency only
  implementation_id TEXT,                               -- transparency only
  risk_class        TEXT NOT NULL,
  side_effect_class TEXT NOT NULL,
  authorization     TEXT NOT NULL,
  data_class        TEXT NOT NULL,                      -- the request's classified class
  requested_by      TEXT NOT NULL,
  automation_context TEXT NOT NULL,
  arguments_digest  TEXT NOT NULL,
  scope             TEXT NOT NULL,                      -- canonical JSON projection
  scope_digest      TEXT NOT NULL,
  summary_kind      TEXT NOT NULL,                      -- closed: capability builder identity
  raised_at_ms      INTEGER NOT NULL,
  expires_at_ms     INTEGER NOT NULL,                   -- raised_at + approval_request_expiry_ms
  max_uses          INTEGER NOT NULL,
  status            TEXT NOT NULL,                      -- PENDING | APPROVED | DENIED | EXPIRED | REVOKED
  terminal_reason   TEXT
)

-- Durable grant. The only object that authorizes a step.
approval_grants (
  grant_id          TEXT PRIMARY KEY,                   -- grt_ + ULID
  approval_id       TEXT NOT NULL REFERENCES approval_requests(approval_id) ON DELETE CASCADE,
  task_id           TEXT NOT NULL REFERENCES tasks(task_id) ON DELETE CASCADE,
  step_id           TEXT NOT NULL,
  generation_id     INTEGER NOT NULL,
  descriptor_digest TEXT NOT NULL,
  arguments_digest  TEXT NOT NULL,
  scope_digest      TEXT NOT NULL,
  max_uses          INTEGER NOT NULL,
  uses_remaining    INTEGER NOT NULL CHECK (uses_remaining >= 0),
  granted_at_ms     INTEGER NOT NULL,
  expires_at_ms     INTEGER NOT NULL,
  granted_by        TEXT NOT NULL,                      -- USER | ADMIN
  actor_device_id   TEXT,
  actor_session_id  TEXT,
  auth_strength     TEXT NOT NULL,                      -- SESSION | ELEVATED_CONFIRMED
  elevated_ref      TEXT,                               -- digest handle only; never biometric bytes
  status            TEXT NOT NULL,                      -- ACTIVE | EXHAUSTED | EXPIRED | REVOKED
  UNIQUE (approval_id, task_id),
  CHECK (uses_remaining <= max_uses)
)

-- One row per consuming Step. Uniqueness is the idempotency guarantee.
approval_grant_uses (
  grant_id      TEXT NOT NULL REFERENCES approval_grants(grant_id) ON DELETE CASCADE,
  step_id       TEXT NOT NULL,
  task_id       TEXT NOT NULL REFERENCES tasks(task_id) ON DELETE CASCADE,
  consumed_at_ms INTEGER NOT NULL,
  PRIMARY KEY (grant_id, step_id),
  UNIQUE (step_id)
)
-- NOTE ON THE TWO CONSTRAINTS. The PRIMARY KEY (grant_id, step_id) is the ADR-0039
-- invariant: at most one use row per (grant, step). The UNIQUE (step_id) is strictly
-- stronger -- it makes a step consumable by at most one grant, ever, across all
-- grants. The original comment said "one use per Step per grant", which describes the
-- PRIMARY KEY and not the UNIQUE. That stronger constraint is offered as
-- defence-in-depth (a step cannot be authorised twice), but it must be confirmed in
-- P6A: it also blocks the legitimate re-approval flow in which a first grant for a
-- step is invalidated and a second grant for the same step_id is later consumed.
-- If re-approval must be possible, keep the PRIMARY KEY and drop the UNIQUE.
-- step_id has no REFERENCES task_steps(step_id) here, unlike grant_id and task_id;
-- P6B should add it, matching migration 0004's step_capability_bindings shape, or
-- PRAGMA foreign_key_check will not notice a step that does not exist.
```

Deliberately **absent** from migration 0005: dispatch intents, `RequestId`, provider
attempts, duplicate-suppression state, repeated-action counters, `ActionResult`, receipts,
reconciliation state. Those are P8's and do not belong in the P6 phase
(ADR-0036; Bounds §2.4). `policy_evaluations` is also absent: the evaluation is a pure
function of immutable inputs plus the immutable revision, so an evaluation log duplicates
what `policy_revisions` and the request row already prove.

One integrity guard the schema needs and does not yet have: `approval_requests.approval_id`
must be checked for the `apr_` prefix and 26-character Crockford ULID body, exactly as
`tasks.task_id` is in migration 0001. Same for `grt_` on grants. Noted for P6B.

Three further P6B schema obligations implied by the code above:

- `approval_requests.step_id` and `approval_grants.step_id` carry no
  `REFERENCES task_steps(step_id)`, so an orphaned step reference is possible and the
  task-delete cascade does not reach it.
- `approval_grants` carries no `plan_revision`, although §6.2 makes `plan_revision`
  `MUST_BIND` and §9.9 has P8 re-check it. P8 can read it through `approval_id`, but the
  join is implicit and undocumented.
- No table is `STRICT` and no digest column carries the `sha256:` shape `CHECK` that
  migration 0004 lines 6–11 apply to registry digests, so `rules_digest` and
  `descriptor_digest` are unvalidated text in the conceptual shape.

### 8.5 Atomicity

State and event commit together, above Storage, with no `serea-storage` → Event Bus edge
(ADR-0025). Every transition in Cluster D is one Store transaction with its event:

| Transition | Event in the same transaction |
| --- | --- |
| Revision activation | `POLICY_CHANGED` |
| Request raised | `APPROVAL_REQUIRED` |
| Grant minted | `APPROVAL_GRANTED` |
| Denial committed | `APPROVAL_DENIED` |
| Request expired unused | `APPROVAL_EXPIRED` |
| Grant reached expiry with uses remaining | `APPROVAL_EXPIRED_UNUSED` — an existing kind this table originally omitted |
| Revocation committed | `APPROVAL_REVOKED` (Proposed) |
| Use consumed | `APPROVAL_CONSUMED` |

Two mappings are still unspecified and must be settled in P6A. `EXHAUSTED` is a grant
state with no existing event kind of its own, and Approval Protocol §8's audit table has
no row for it; either it emits `APPROVAL_CONSUMED` for the final use and nothing further,
or it needs a new kind. `EXPIRED` is likewise ambiguous between the two existing kinds
(`APPROVAL_EXPIRED` = "request expired unused", `APPROVAL_EXPIRED_UNUSED` = "grant hit
expiry with uses remaining"), so the transition must name which object expired, not just
that something did.

---

## 9. Cluster F — crate ownership, API and the P8 seam

### 9.1 Scope

Resolves **D20**, and settles the type questions **D4** and **D8** depend on.

### 9.2 The contradiction

Three documents disagree:

| Source | Says |
| --- | --- |
| `crates/serea-protocol/src/lib.rs:39-45` (comment) | `PolicyDecision`/`DenyReason` owned by `serea-policy`; `ApprovalRequest`/`ApprovalGrant` owned by `serea-capability` |
| `03-crate-map.md:186` (§3.1) | `PolicyEngine`, `PolicyDecision`, rule store, `PolicyContext`, `AutomationContext`, `RuleStore`, `PolicyChange` owned by `serea-policy` |
| `03-crate-map.md:182` (§3 inventory) | `serea-protocol`'s public API lists `PolicyDecision`, `DenyReason`, `ApprovalRequest`, `ApprovalGrant` |
| `03-crate-map.md:188` | `serea-capability`'s P6 authorization handoff "may depend on `serea-policy`" |

Only one of these is a workspace gate, and it decides the question.

### 9.3 The mechanical fact

`tests/workspace_smoke.py:370` fails the build if `serea-capability` depends on
`serea-policy`. The `CAP --> POLICY` edge cannot be created at all, and the comment in
`lib.rs` that attributes `ApprovalRequest`/`ApprovalGrant` to `serea-capability` is
therefore describing an impossible graph. Meanwhile `tests/workspace_smoke.py:360` permits
`serea-task-engine` to depend on protocol, storage, event-bus and capability — and nothing
else, so a `serea-task-engine → serea-policy` edge needs an amendment too.

Three designs:

**DESIGN A — `serea-policy` owns runtime policy and approval types.** The evaluator, the
rule store, `PolicyDecision`, `DenyReason`, `ApprovalRequest`, `ApprovalGrant`,
`ApprovalGrantUse`, the lifecycle state machine and the authenticated response seam all live
in one crate. `serea-capability` owns `PreparedActionV1` and nothing about policy.

**DESIGN B — `serea-protocol` owns neutral wire/domain records; `serea-policy` owns
behaviour.** Rejected. It would move mutable runtime records into the frozen contract
crate, which `lib.rs` explicitly says holds "no orchestration, no persistence". It also puts
`ApprovalGrant` — a type with a live counter — in L0, where changing it means a wire change.

**DESIGN C — `serea-capability` owns approval types.** Rejected twice over: mechanically
forbidden by `workspace_smoke.py`, and wrong in principle because a capability crate that
owns approval authority is the shape ADR-0035 spent effort preventing.

**Recommended: DESIGN A.**

### 9.4 The resulting DAG

```text
L0  serea-protocol        (frozen enums, IDs, Clock, ports)
L1  serea-storage, serea-event-bus
L2  serea-capability, serea-policy, serea-model-router
L3  serea-task-engine, serea-scheduler, serea-memory
L4  serea-core

serea-capability --> serea-protocol, serea-storage, serea-event-bus
serea-policy      --> serea-protocol, serea-storage, serea-event-bus
serea-task-engine --> serea-protocol, serea-storage, serea-event-bus,
                     serea-capability, serea-policy      <-- one new edge
serea-scheduler   --> serea-protocol, serea-storage, serea-event-bus, serea-task-engine
```

**No `serea-capability → serea-policy` edge.** TaskEngine already depends on both and
already orchestrates capability preparation (`crates/serea-task-engine/src/engine.rs:181`
calls `prepare_action`), so the P6 composition is:

```text
TaskEngine
  -> serea-capability::prepare_action(...)      -> PreparedActionV1 (immutable)
  -> build PolicyInputV1 from Task + PreparedActionV1
  -> serea_policy::evaluate(input, current revision)
  -> typed authorization outcome
```

Acyclic. Verified mechanically by `cargo metadata`. The one required amendment is adding
`serea-policy` to `serea-task-engine`'s non-dev dependencies in
`tests/workspace_smoke.py`, plus the `serea-policy` row itself when the crate is created in
P6B.

### 9.5 Type placement

| Type | Owning crate | Why |
| --- | --- | --- |
| `PolicyRule`, `PolicyDecision`, `DenyReason`, `HandoffRequest` | `serea-policy` | Behaviour, not wire |
| `ApprovalRequest`, `ApprovalGrant`, `ApprovalGrantUse` | `serea-policy` | Runtime records with live state |
| `ApprovalLifecyclePayloadV1` | `serea-protocol` (unchanged) | Already frozen; routing only |
| `PolicyInputV1` | `serea-policy` | It is the evaluator's input contract; defining it next to the evaluator keeps them from drifting |
| `AuthenticatedApprovalResponseV1` | `serea-policy` | P6 owns the trust contract for its own input |
| `AuthorizationEvidenceV1` | `serea-policy` | P6's output; consumed by P8 |

### 9.6 `PolicyInputV1`

Immutable, constructed by TaskEngine, read only by `serea-policy`. No raw arguments.

```rust
pub struct PolicyInputV1 {
    // PreparedAction identity (all from P5, none mutable by policy)
    pub task_id: TaskId,
    pub step_id: StepId,
    pub plan_revision: u32,
    pub generation_id: i64,
    pub descriptor_digest: Digest,
    pub capability_id: CapabilityId,
    pub capability_version: SemVer,
    pub risk_class: RiskClass,
    pub side_effect_class: SideEffectClass,
    pub authorization: Authorization,
    pub data_class: DataClass,
    pub arguments_digest: Digest,
    // Task snapshot (host-owned)
    pub task_policy_class: RiskClass,
    pub task_state: TaskState,
    pub task_cancelled: bool,
    pub task_deadline_at: Option<EpochMillis>,
    pub task_data_class: DataClass,
    // Trusted context
    pub requested_by: RequestedBy,
    pub automation_context: AutomationContext,
    pub device_state: Option<DeviceTrustState>,
    // Policy snapshot
    pub policy_revision_id: i64,
    pub policy_rules_digest: Digest,
}
```

`DeviceTrustState` is included because Policy §6 explicitly permits the device registry
state to be an input. It is a host-supplied enum, never a live query: paired, trusted,
online. Policy §6 also forbids wall-clock time in policy, so `now` is **not** a field of
`PolicyInputV1`; expiry is checked by the approval layer, not the rule evaluator, which is
the only reading consistent with both documents.

### 9.7 `PolicyDecision`

Closed internal outcome, matching Policy §3 prose:

```rust
pub enum PolicyDecision {
    Allow,
    RequireApproval(ApprovalRequestDraft),
    RequireHandoff(HandoffRequest),
    Deny(DenyReason),
}
```

No `Execute`, no `Dispatch`, no `RequestId`. `DenyReason` is a closed `ReasonCode` — the
existing `^[A-Z][A-Z0-9_]*$` grammar — so free text can never control behaviour and a
reason can drive a test assertion.

### 9.8 Authorization evidence, not permission

Recommended naming: **`AuthorizationEvidenceV1`**, not `AuthorizedAction`.

```rust
pub struct AuthorizationEvidenceV1 {
    pub task_id: TaskId,
    pub step_id: StepId,
    pub policy_revision_id: i64,
    pub policy_rules_digest: Digest,
    pub requirement: ApprovalRequirement,   // NONE | GRANT_BOUND | HANDOFF_REQUIRED
    pub approval_id: Option<ApprovalId>,
    pub grant_id: Option<GrantId>,
    pub evaluated_at: EpochMillis,
}
```

`AuthorizedAction` is rejected as a name because it reads as a permanent execution
permission, which is precisely what ADR-0036 forbids P6 from returning. There is no
`approved: bool` and no `RequestId`. P8 must revalidate, and the shape above carries enough
identity to do it.

### 9.9 The P8 revalidation contract

P6 must make these revalidatable; P8 must independently check each against *current* state:

| Check | P6 records | P8 re-checks |
| --- | --- | --- |
| Action identity | TaskId, StepId, plan revision, capability, version | The durable Step's current binding |
| Policy revision used | `policy_revision_id`, `policy_rules_digest` | Against the current `policy_state` pointer |
| Approval identity | `approval_id`, `grant_id` | That both rows still exist and are non-terminal |
| Grant use state | `uses_remaining`, `approval_grant_uses` | That a use is available for this Step |
| Expiry | `expires_at` | `now < expires_at`, same injected Clock |
| Task/Step binding | `task_id`, `step_id` | Task is non-terminal, not cancelled, deadline not passed |
| Descriptor and generation | `descriptor_digest`, `generation_id` | Against the Task's pinned generation and the live overlay |

No boolean is sufficient, and P6 must not be written so that P8 *can* be satisfied by one.

### 9.10 What P6 must not contain

No provider invocation. No `RequestId` minting. No dispatch intent. No duplicate
suppression. No repeated-action counter. No `ActionResult`. No reconciliation. No
credential material, not even a handle.

---

## 10. Concurrency and crash requirements

Independent Store connections for every race. Fault injection through the existing
`p2h-fault-injection` storage feature, which is already how P2H proved its crash matrix.

### 10.1 Race matrix

| Race | Invariant |
| --- | --- |
| Policy activation vs evaluation | Every decision names one immutable revision; no mixed snapshot |
| Approve vs deny, concurrent | Exactly one terminal outcome; at most one grant |
| Approve vs expire at the boundary | One transaction wins; `approval_grant_expiry_ms` decides, `now < expires_at` |
| Approve vs revoke | One terminal state wins; the loser gets a typed refusal |
| Revoke vs P8 handoff | Either the use record exists and P8 dispatches, or it does not and P8 refuses |
| Two consumers of the final use | At most one different `step_id` consumes; the `UNIQUE (grant_id, step_id)` row and the `uses_remaining` decrement are in one transaction |
| Same-Step repeated consume | Idempotent; no second decrement |
| Cancel vs approve | A cancelled task is not approvable; the response gets `TASK_NOT_APPROVABLE` |
| Deadline vs approve | The task clock is paused while waiting, so the approval expiry is the only clock; an expired request refuses |
| Replacement Step vs old grant | The old `step_id` cannot satisfy a new `step_id` |
| Revision write vs activation pointer | Either the old revision stays active or the new one is active with its rows committed |

### 10.2 Crash matrix

No hardware power-loss claim; every case is a transaction-boundary fault plus reopen.

| Fault | Required outcome |
| --- | --- |
| Revision rows written, `POLICY_CHANGED` append fails | Nothing activates; no `policy_state` change |
| `policy_state` pointer update fails | Old revision remains authoritative |
| Request insert, event append fails | No request row; no `APPROVAL_REQUIRED` |
| `WAITING_APPROVAL` transition fails | No orphan actionable request; retry is idempotent |
| Grant insert, `APPROVAL_GRANTED` append fails | No grant; request stays `PENDING` |
| Denial commit, event append fails | No denial row; request stays `PENDING` |
| Revocation commit, event append fails | Grant stays `ACTIVE`; revocation retried |
| Consume commit, `APPROVAL_CONSUMED` append fails | No use decrement; retry is idempotent per `(grant_id, step_id)` |
| Task cancel lands while a response is committing | The response transaction sees the terminal task and refuses |
| Caller loses the result after commit | Reopen returns the same terminal state; no second grant |

---

## 11. Security review

| Threat | Mechanically blocked by |
| --- | --- |
| Model injects an approval | P6 reads only `PolicyInputV1` and the authenticated seam; no model field exists on either. `ToolCallProposalV1` has no approval field at all |
| Model lowers a risk class | `risk_class` is not model-writable; ADR-0035 and Capability §4.2 |
| Forged `SYSTEM` provenance | `RequestedBy` is host-supplied on the internal seam, and `SYSTEM` gets a *narrower* default posture |
| Provider weakens authorization | A provider cannot reach policy or approval; the manifest pins authorization (ADR-0034) |
| Stale grant replay | Grant binding includes generation, descriptor digest, arguments digest, scope and task; P8 re-checks all of them |
| Approval ID guessing | `apr_` + ULID, 80 bits of randomness; plus the seam requires an authenticated principal and session |
| Response for the wrong Step | `step_id` is bound on the grant and on the use record |
| Changed arguments | `arguments_digest` is recomputed from the durable Step input |
| Replacement Step | A new `step_id` cannot match an old use record or an old grant |
| Policy rollback | The activation pointer only advances; `policy_state` is a singleton with `ON DELETE RESTRICT` |
| Grant double-spend | One transaction decrements `uses_remaining` and inserts the unique `(grant_id, step_id)` use row |
| Capability disabled | The live overlay blocks new bindings and is rechecked by P8; no policy rule re-enables it |
| Task cancelled | Task Engine cascade plus P8's pre-dispatch recheck |
| Deadline expired | Grant and request both carry `expires_at`; P8 re-checks |
| Credential leakage | `serea-storage` refuses `SECRET`/`CREDENTIAL` mechanically; no raw arguments or biometric bytes are stored |
| Summary spoofing | The summary is built by a host capability-specific builder from classified arguments, never by a model |
| Malicious notification content | No inline actions; `PERSONAL` ceiling; redaction before render |

Two residual risks are unchanged by this package and are recorded, not hidden: a local
database writer can forge durable state (AB-15/AB-16, no integrity seal is specified), and
an attentive user can still consent to a harmful action accurately described.

---

## 12. Android future compatibility

Not implemented here. Design properties that keep host/node neutrality open:

| Property | How this package preserves it |
| --- | --- |
| Authority is not tied to the Mac | Approval authority is `AuthenticatedPrincipal` — an abstract principal with a device and session reference, not `MacUser` |
| Grant identity is not tied to a host instance | A grant binds Task/Step/capability/arguments, none of which are host-shaped |
| Local Android memory | Not forbidden structurally, but not designed for either; the durable authority stays wherever `policy_state` lives |
| Notification-only approval | Explicitly refused; no notification action can approve |
| Adding a second node | Requires a new revision-authority decision, flagged below |

`SYSTEM` is the one variant that reads as host-internal. It is deliberately *not* given
god-mode for exactly this reason.

**Flagged for a later ADR, before P7/P12:** execution-domain semantics. If a future Android
node ever evaluates policy or holds grants, the current design's "one `policy_state`
singleton per host" becomes a multi-writer question. That needs its own ADR and its own
owner decision; this package does not pre-decide it and does not assume the Mac is the only
possible authority node.

---

## 13. Protocol change preview

What changes if the recommended package is accepted. Nothing here is edited now.

| Document | Change |
| --- | --- |
| Policy §4.1 | Restate that the class-default table is a fixed code constant, not rule data |
| Policy §4.2 | Replace "most specific first" with the family order, so it cannot be read as a specificity score over rules |
| Policy §5 | Add the closed match dimension list; state explicitly that `priority DESC, rule_id ASC` is the complete winner rule and that DENY overrides it |
| Policy §3 | Note that `PolicyDecision` is an internal Rust type in `serea-policy`, not a wire type |
| Approval §3.1 | Confirm the six bounds include generation and descriptor digest |
| Approval §4 | Add the explicit rule-vs-rule conflict outcome (DENY wins) |
| Approval §5 | Reference the internal `AuthenticatedApprovalResponseV1` seam |
| Approval §8 | Add `APPROVAL_REVOKED` (Proposed) and its payload |
| Task §3 | Restate that the Step's immutable binding is the authority for provider and implementation, so approvals need not duplicate it |
| Event §3.4 | Add `APPROVAL_REVOKED` to the approval activity table |
| Event §8 | Note that P6 approval rows follow task retention; policy tables do not |
| Bounds §2 | Add `max_active_policy_rules`, `max_retained_policy_revisions`, `approval_grant_expiry_ms`, `approval_grant_max_uses` |
| Crate Map §3, §3.1 | Correct the ownership lines; add the `task-engine → policy` edge; remove the `capability → policy` edge |
| `serea-protocol/src/lib.rs:39-45` | Correct the stale comment |

Architecture version: `serea-arch/2.7.0` if accepted. No wire major.

---

## 14. Recommended package and remaining owner overrides

### 14.1 The package

| ID | Contents | Status |
| --- | --- | --- |
| **R1 — POLICY** | MODEL A class defaults; `priority DESC, rule_id ASC` precedence; DENY overrides ALLOW; closed match dimensions; five `RequestedBy` values mapped to five contexts with `SYSTEM` narrowed | `RECOMMENDED`, `NOT OWNER-RATIFIED` |
| **R2 — APPROVAL IDENTITY** | Six frozen bounds plus generation ID and descriptor digest; provider and implementation are transparency only; structural scope projection; `max_uses` from day one; 30-minute grant and request expiry | `RECOMMENDED`, `NOT OWNER-RATIFIED` |
| **R3 — LIFECYCLE / CONCURRENCY** | Two-variable state machine; the eight-row duplicate-response matrix; grant-only revocation with a Proposed event kind; authenticated response seam with clamping; no P6 cancellation path | `RECOMMENDED`, `NOT OWNER-RATIFIED` |
| **R4 — STORAGE / RETENTION** | Six tables; task cascade for approval rows and no cascade for policy tables; `PERSONAL` ceiling on persisted summaries; raw arguments never stored | `RECOMMENDED`, `NOT OWNER-RATIFIED` |
| **R5 — CRATE / API** | DESIGN A; no `capability → policy` edge; `PolicyInputV1` without raw arguments and without `now`; `AuthorizationEvidenceV1` with no boolean and no `RequestId` | `RECOMMENDED`, `NOT OWNER-RATIFIED` |
| **R6 — BOUNDS** | Four new named bounds with the values in §8.3 | `RECOMMENDED`, `NOT OWNER-RATIFIED` |

### 14.2 The six defaults carried by the same sentence

`ACCEPT P6 RECOMMENDED PACKAGE` — the one sentence in §0 — **settles R1 through R6 and adopts
every default below.** The six are listed here so an override can name one, not because the
sentence leaves them open. Each carries a recommended default that is already applied above.

1. `approval_grant_max_uses` — adopted default `8`. A choice about how much batch breadth the
   owner wants to permit in one prompt. Purely a product number.
2. `max_active_policy_rules` — adopted default `512`. A capacity guard; any value the owner
   prefers is safe.
3. `max_retained_policy_revisions` — adopted default `64`. A storage-audit trade-off.
4. `approval_grant_expiry_ms` — adopted default `1800000` (equal to the request). This is the
   only one with a security consequence: a longer grant horizon means a longer window in
   which a grant remains usable.
5. Multi-use grants in P6 V1 at all — adopted default `yes`. If the owner prefers one-use V1,
   the schema is unchanged and a later phase widens `max_uses`.
6. The `PERSONAL` ceiling on persisted summaries — adopted default `yes`. This is the only
   override that changes what a user can be shown: a PRIVATE summary would not be persistable
   today, and permitting it is gated on ADR-0022.

An explicit override supersedes **only the value it names**: for example, if the owner were to
send `ACCEPT P6 RECOMMENDED PACKAGE with approval_grant_max_uses = 3`, that reply would
ratify R1–R6 and change that one number. No other value would be reopened, and no ADR would
be accepted by the phrase.

All other D1–D20 items are closed, deferred, or implementation detail, and do not need an
answer. **Nothing in this section is a ratification.**

---

## 15. Minimal P6 V1 alternative

Offered for comparison, not recommended. Complexity and risk against the full package:

| Aspect | Minimal V1 | Full package |
| --- | --- | --- |
| Class defaults | MODEL A | Same |
| Rule dimensions | Capability ID and risk class only | Six dimensions |
| Approval identity | Six frozen bounds, no generation or descriptor digest | Adds generation and descriptor digest |
| Grant uses | 1, no use table | `max_uses` with a use table |
| Scope | Exact action equality | Structural projection |
| Revocation | None; task cancellation covers it | Grant-only revocation with an event |
| New bounds | 1 | 4 |

Minimal V1 is materially weaker in exactly three places, and each is a real loss:

1. **No generation or descriptor digest binding.** A registry change between approval and
   dispatch could ride on an old grant. P8's recheck catches it, but only if the evidence
   records it, which is the whole point of binding it.
2. **No use table.** One-use-only avoids the table, but it also makes Approval §7.1's batch
   case unimplementable later without a migration.
3. **No revocation.** Cancellation covers it in practice, but there is then no way to
   withdraw a single granted-but-unconsumed authorisation without cancelling the whole task.

The recommendation is the full package. Minimal V1 is recorded because a weaker model is
only worth choosing when the weakness is free, and here it is not.

---

## 16. Future P6 RED test plan

No runtime exists. These are the executable-style cases the implementation phase writes
first. Each is written so a failure names one property.

### 16.1 Policy semantics

```text
GIVEN a capability with RiskClass OBSERVE and no explicit rule
WHEN policy evaluates with automation_context INTERACTIVE
THEN the decision is Allow
```

```text
GIVEN a capability with RiskClass OBSERVE and automation_context PROACTIVE
WHEN policy evaluates
THEN the decision is Allow, because OBSERVE is inside the PROACTIVE allowance
  and the OBSERVE class default applies; only a non-OBSERVE/non-LOCAL_STATE
  capability denies in PROACTIVE (Policy Protocol 4.2 step 2, 4.3, 8; ADR-0016)
```

```text
GIVEN a capability with RiskClass EXTERNAL_WRITE and automation_context PROACTIVE
WHEN policy evaluates
THEN the decision is Deny(AUTOMATED_ACTION_FORBIDDEN), because EXTERNAL_WRITE is
  outside the OBSERVE/LOCAL_STATE allowance
```

```text
GIVEN an explicit rule set where rule A matches with decision DENY and priority 10
  and rule B matches with decision ALLOW and priority 20
WHEN policy evaluates
THEN the decision is Deny, and the reason code names rule A
```

```text
GIVEN two matching ALLOW rules at priority 40 with rule_ids pol_0007 and pol_0003
WHEN policy evaluates
THEN rule pol_0003 wins, because the priorities tie and rule_id ASC breaks the tie
  (pol_0003 sorts before pol_0007); priority DESC alone does not decide a tie
```

```text
GIVEN a capability whose RiskClass exceeds the Task policy_class
WHEN policy evaluates
THEN the decision is Deny(TASK_POLICY_CEILING_EXCEEDED), before any rule is consulted
```

```text
GIVEN a capability with required_authorization CREDENTIAL_HANDOFF
WHEN policy evaluates
THEN the decision is RequireHandoff, and no approval response can satisfy it
```

```text
GIVEN a rule set of 1000 identical rules
WHEN policy evaluates 1000 times across a restart
THEN every decision is identical
```

### 16.2 Provenance

```text
GIVEN RequestedBy MODEL and RequestedBy USER on identical PreparedAction facts
WHEN policy evaluates both
THEN both decisions are identical
```

```text
GIVEN RequestedBy SCHEDULER on a SCHEDULED task at RiskClass EXTERNAL_WRITE
WHEN policy evaluates
THEN the decision is RequireApproval, not Allow, because the EXTERNAL_WRITE class
  default applies unchanged and SCHEDULED adds no automatic allowance
```

```text
GIVEN a SCHEDULED task being created with a policy_class above LOCAL_STATE
WHEN no explicit durable rule allows it
THEN task creation is refused; 4.5's task-kind restriction is a creation-time
  check, not an evaluation-time one
```

```text
GIVEN RequestedBy SYSTEM on a DESTRUCTIVE capability
WHEN policy evaluates
THEN the decision is not Allow
```

```text
GIVEN a caller that reports RequestedBy USER while the Task kind is PROACTIVE
WHEN the context is derived
THEN the derived context is PROACTIVE, and non-OBSERVE/LOCAL_STATE denies
```

Note: the last example asserts the Task-kind-restricts reading of 4.5's final
paragraph. 4.5's table is keyed on `RequestedBy` alone ("one row per variant"),
so the same passage can be read as a pure `RequestedBy` mapping, under which this
case derives `INTERACTIVE`. Which input dominates is an open P6A question and is
recorded as a finding rather than settled here.

### 16.3 Revision identity and activation

```text
GIVEN revision 3 active and a rule set with digest D3
WHEN an evaluation runs
THEN the evidence names revision 3 and digest D3
```

```text
GIVEN revision 4 prepared with digest D4 and not activated
WHEN any evaluation runs
THEN the evidence still names revision 3
```

```text
GIVEN revision 4 activated in the same transaction as a POLICY_CHANGED append
WHEN the append fails
THEN neither the revision activation nor the pointer change commits
```

```text
GIVEN a Python edit of policy_state to point at revision 2
WHEN activation is attempted
THEN it is refused, because the pointer only advances
```

Note: this example is only satisfiable if migration 0005 carries the three
`policy_state` triggers listed under §8.4. A conceptual schema with the FK and the
singleton check alone lets the Python edit succeed, so the trigger is what the test
proves, not the pointer column.

### 16.4 Approval identity and binding

```text
GIVEN a grant bound to Step A on generation 5 and descriptor digest X
WHEN a request is made for Step B with identical capability and arguments
THEN the grant does not match, and a new approval request is raised
```

```text
GIVEN a grant bound to Step A
WHEN Step A's arguments change so the digest differs
THEN the grant is invalidated, and Approval§4.1 re-derivation refuses
```

```text
GIVEN a grant whose provider_id is P1 and implementation_id I1
WHEN the descriptor digest and generation are unchanged but the transparent fields differ
THEN the grant still matches, because those fields are transparency only
```

```text
GIVEN a grant whose scope is {calendar: primary}
WHEN the call arguments specify {calendar: work}
THEN the grant does not match
```

```text
GIVEN a grant whose scope is {calendar: primary}
WHEN the call arguments specify {calendar: *}
THEN the grant does not match, because wildcards are unsupported
```

### 16.5 Expiry

```text
GIVEN a grant expiring at T
WHEN now is T-1
THEN the grant is valid
```

```text
GIVEN a grant expiring at T
WHEN now is exactly T
THEN the grant is invalid, because validity is now < expires_at
```

```text
GIVEN an APPROVAL_RESPONSE proposing expires_at beyond the request's own horizon
WHEN P6 applies the response
THEN the proposal is clamped to the request bound, not accepted
```

```text
GIVEN an APPROVAL_RESPONSE proposing max_uses 100 against approval_grant_max_uses 8
WHEN P6 applies the response
THEN the proposal is clamped to 8
```

### 16.6 Duplicate responses

```text
GIVEN a request in state PENDING
WHEN two GRANT responses arrive concurrently on independent connections
THEN exactly one grant exists and one response is idempotent success
```

```text
GIVEN a request in state APPROVED with an unused grant
WHEN a GRANT arrives again
THEN the result is idempotent success carrying the existing grant identity, with no new grant
```

```text
GIVEN a request in state APPROVED
WHEN a DENY arrives
THEN the result is typed refusal APPROVAL_RESPONSE_CONFLICT
```

```text
GIVEN a request in state EXPIRED
WHEN a GRANT arrives
THEN the result is typed refusal APPROVAL_REQUEST_EXPIRED
```

```text
GIVEN a task in state CANCELLED
WHEN a GRANT arrives for its pending request
THEN the result is typed refusal TASK_NOT_APPROVABLE
```

```text
GIVEN a request in state APPROVED whose grant has been revoked
WHEN a GRANT arrives again
THEN the result is typed refusal APPROVAL_REVOKED, not idempotent success, and no
  revoked grant identity is returned to any caller
```

This last cell is the one the grant-only revocation rule in §7.4 forces into the matrix.
The request stays `APPROVED` after a grant is revoked, so a matrix keyed on the request
alone would take the idempotent-success path and hand back a withdrawn grant.

### 16.7 Revocation

```text
GIVEN an ACTIVE grant with uses_remaining 2
WHEN an authenticated principal revokes it through the P6A-resolved revoke path
THEN the grant is REVOKED, APPROVAL_REVOKED commits in the same transaction, and the task does not fail
```

The path itself is not yet fixed: §7.4 records that neither the internal seam in §7.7 nor
Device Protocol §5.2 can carry a REVOKE, so P6A must choose between widening the internal
`ResponseDecision` and narrowing the recommendation to the local admin surface.

```text
GIVEN a REVOKED grant
WHEN P8 attempts pre-dispatch validation
THEN P8 refuses
```

```text
GIVEN a REVOKED grant with one consumed use
WHEN audit reads the use record
THEN the consumed use is retained, and no future use is permitted
```

```text
GIVEN a cancelled Task with an ACTIVE grant
WHEN the grant row is read
THEN the grant is still ACTIVE, because cancellation is not revocation, and the task cascade removes it on task deletion
```

### 16.8 Use consumption

```text
GIVEN a grant with max_uses 1 and uses_remaining 1
WHEN two independent connections attempt to consume it for different steps concurrently
THEN exactly one consumes, and the other is refused
```

```text
GIVEN a grant already consumed by Step A
WHEN Step A retries the consume
THEN it is a no-op with no second decrement
```

```text
GIVEN a grant consumed by Step A
WHEN Step B attempts to consume the same grant
THEN it is refused, because PRIMARY KEY (grant_id, step_id) admits no second Step row
  and uses_remaining is decremented inside the same transaction
```

The original wording cited `UNIQUE (step_id)` for this case. That constraint is
stronger than the case needs and is the one whose retention is an open P6A question
(§8.4). Under it, a *different* grant for Step A is also refused, which is the
re-approval flow that must be confirmed or excluded in P6A.

### 16.9 Privacy

```text
GIVEN a capability whose classified arguments are PRIVATE
WHEN a request is raised
THEN the persisted row carries no PRIVATE text, and no raw arguments
```

```text
GIVEN a CREDENTIAL-classified value reaching the approval path
WHEN the request is built
THEN serea-storage refuses the row, and no event carries the value
```

```text
GIVEN an APPROVAL_REQUEST event
WHEN it is appended
THEN its payload is metadata only, within max_event_payload_bytes
```

```text
GIVEN an ELEVATED_DEVICE approval without biometric confirmation
WHEN P6 applies the response
THEN the response is discarded with APPROVAL_BIOMETRIC_REQUIRED
```

### 16.10 Stale P8 revalidation

```text
GIVEN P6 authorization evidence naming revision 3
WHEN P8 dispatches after revision 5 activates
THEN P8 refuses, because the evidence names a superseded revision
```

```text
GIVEN P6 authorization evidence naming an APPROVED grant
WHEN P8 dispatches after the grant is revoked
THEN P8 refuses
```

```text
GIVEN P6 authorization evidence naming a valid grant
WHEN P8 dispatches after the Task is cancelled
THEN P8 refuses
```

```text
GIVEN an AuthorizationEvidenceV1
WHEN its public API is inspected
THEN it exposes no approved boolean and no RequestId
```

---

## 17. Future P6 slices

| Slice | Contents | Production files likely touched | Migration | Closure gate | Remains prohibited |
| --- | --- | --- | --- | --- | --- |
| **P6A** | Owner ratification; correct the four contradictory ownership statements; add the four bounds; add the Proposed event kind | `docs/protocols/*`, `docs/architecture/03-crate-map.md`, `crates/serea-protocol/src/lib.rs` (comment only), `docs/decisions/README.md` | None | Docs validator green; owner phrase recorded; no ADR marked Accepted without the owner | All runtime |
| **P6B** | Create `serea-policy`; migration 0005; add the `task-engine → policy` edge and the smoke-test row | `crates/serea-policy/**` (new), `crates/serea-task-engine/Cargo.toml`, `Cargo.toml`, `tests/workspace_smoke.py`, `crates/serea-storage/migrations/0005_*.sql` | 0005 created | `cargo metadata` acyclic; `cargo fmt` clean; smoke green; rollback/reopen/FK tests | No evaluator, no lifecycle yet |
| **P6C** | Deterministic evaluator, `PolicyInputV1`, `PolicyDecision`, revision activation and pointer | `crates/serea-policy/src/**`, `crates/serea-task-engine/src/engine.rs` | None | Precedence permutation, restart determinism, deny-override, provenance and ceiling tests | No provider invoke |
| **P6D** | Approval lifecycle, authenticated seam, summary builder port, expiry, duplicate matrix, revocation, atomic consume | `crates/serea-policy/src/**`, `crates/serea-capability/**` only if the builder port lands there | None | All §16.4–16.8 tests; zero-invoke source gate | No provider invoke |
| **P6E** | TaskEngine orchestration, wait/ack, startup recovery for expired requests | `crates/serea-task-engine/src/**`, `crates/serea-scheduler/**` | None | Restart-idempotency, no-orphan-wake, cancellation-is-not-revocation tests | No provider invoke |
| **P6F** | Security, concurrency and crash closure; §10 and §11 fully green | Tests across `crates/**` | None | Full matrices green; `Policy`/`Approval`/`Task`/`Event`/`Bounds`/Crate Map consistent; Fast and Full CI green | P8 still not started |

---

## 18. P8 decision reclassification

| Decision | P6 needs it? | Why |
| --- | --- | --- |
| D11 consume/dispatch transaction boundary | **No** | The invariant P6 needs is that a use is consumed atomically and idempotently per `(grant_id, step_id)`. Whether that consumption shares a transaction with the dispatch intent is a P8 property, because the intent is P8's object. P6 must expose a consume operation that can join a caller's transaction |
| Dispatch intent shape, reservation, IDK reservation | No | P8 |
| Reconciliation matrix | No | P8 |
| Provider availability recheck | No | P8 |

---

## 19. Final decision count

| Metric | Count |
| --- | --- |
| Original owner decisions | 20 |
| Closed by existing contract | 10 |
| Deferred to P8 | 1 |
| Implementation detail, not an owner choice | 3 |
| Genuine P6 owner clusters remaining | 6 |

---

## 20. Nonclaims

This document claims no runtime. It creates no crate, no migration, no table, no evaluator,
no lifecycle, no test. It does not mark any ADR Accepted. It does not start P8. It records
that the recommended package is a proposal awaiting a single owner sentence.
