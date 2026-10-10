# P6 preimplementation audit — Policy and Approval

Status: **BLOCKED_PENDING_OWNER_DECISION** · Audit baseline: `e17fbfc7f3e75f0bc95a66d693fe11b57a6ced50` · Architecture: `serea-arch/2.6.0`

This is a documentation-only audit. No P6 runtime, policy/approval tables, `serea-policy` crate, migration 0005, or provider-dispatch path is implemented. P8 remains the first phase permitted to invoke `CapabilityProvider::invoke`.

## Executive boundary

P5 returns immutable `PreparedActionV1`. P6 may evaluate policy, create/load approval requests and grants, and return a typed authorization outcome. P6 stops there. It does not mint `RequestId`, create dispatch intent, authorize through an `approved=true` boolean, or call a provider. P8 must treat P6 output as a reference to evidence/state that can be revalidated, not as a permanent execution token.

```text
PreparedActionV1 + trusted Task/policy context
  -> immutable policy input snapshot
  -> deterministic policy decision
  -> approval request / grant matching where required
  -> typed authorization outcome + durable references
  -> STOP (P8 alone revalidates mutable state and dispatches)
```

## P6A closure status — 2026-10-10

The owner ratified the full recommended package on 2026-10-10
([§0a of the owner decision package](P6-owner-decision-package.md)). The P6A feasibility
gate then ran and reported a **BLOCKER** on its first pass:
[P6A-feasibility-gate.md](P6A-feasibility-gate.md) proved that the ratified approval identity
(`TaskId` + `StepId` + one exact `arguments_digest` on a grant) cannot coexist with the
ratified multi-use requirement and the frozen Approval Protocol §4 point 2 and §7.1 batch
example, and that every escape route was either prohibited by the execution mandate or a new
authority design requiring owner ratification.

On the same day the owner made the additional decision **R2 — enumerated multi-action grant**
([§0b of the owner decision package](P6-owner-decision-package.md)), which resolves the
contradiction by carrying `arguments_digest` and `scope` **per enumerated member** rather
than once per grant, over a bounded immutable set of 1 to 8 individually approved Steps. The
gate re-ran and **passed**. ADR-0037 through ADR-0040 are **Accepted**.

P6A closure state:

- Migration 0005 does **not** exist. No `serea-policy` crate exists. No evaluator, no
  approval runtime, no TaskEngine runtime modification.
- P6B starts only after this P6A slice is committed, pushed and verified at exact-head CI.

The separable contract closure has been performed: owner ratification, DC-1 through DC-4, the
Root mandatory-approval rule, the four new bounds, the `APPROVAL_REVOKED` event kind, the
request-state correction, the `PlainSummary` rationale correction, the crate ownership
corrections, and the finite `AutomationContext` derivation function.

## Current contract and Rust inventory

| Classification | Current fact | Source / owner / enforcement | Durability and tests | Owner decision? |
|---|---|---|---|---|
| FROZEN | `RiskClass` is host-authored, ordered OBSERVE through CREDENTIAL; it is a capability fact, not model input. | Policy Protocol §2; ADR-0035; `serea-protocol/src/types.rs` `RiskClass`; P6 evaluator. | Descriptor revision and Task row are durable; tests must exercise all ranks and ceiling. | No for ordering. |
| FROZEN | `Task.policy_class` is a host-assigned immutable ceiling; approval cannot raise it. | Task Protocol §§2, 4.3; Policy §4.2; `RiskClass::exceeds`; P6 evaluator and TaskEngine-owned lifecycle. | Durable on Task; tests deny above-ceiling even with grant and prove no mutation path. | No. |
| FROZEN | `DataClass`, `SideEffectClass`, `Authorization`, and `RequestedBy` are typed protocol enums. Requester provenance grants no authority. | `serea-protocol/src/types.rs`; Capability Protocol §§3–4; ADR-0035; P5 supplies `PreparedActionV1`. | P5 descriptor/Task facts durable; P6 consumes only immutable trusted values. | No for meanings; policy relevance/default mapping still needs decision below. |
| FROZEN | P5 `PreparedActionV1` has TaskId, StepId, semantic manifest digest, durable generation ID, descriptor digest, capability/version/provider/implementation, arguments/digest, IDK, DataClass, RequestedBy, deadline, side effect, risk, required authorization, replay/root/idempotency/cost facts. It has no RequestId or permission. | ADR-0035; `crates/serea-capability/src/preparation.rs`; P5 closure. | P5 action facts and bindings are durable; P6 must not mutate/reconstruct from model values. | No. |
| CURRENT_RUNTIME | Rust has `RiskClass`, `DataClass`, `SideEffectClass`, `Authorization`, `RequestedBy`, Task policy class/state, EventKind values, `ApprovalId`/`GrantId`, and `ApprovalLifecyclePayloadV1`. It does **not** currently define runtime `PolicyDecision`, `ApprovalRequest`, `ApprovalGrant`, or `AuthorizedAction` types; P5 `PreparedActionV1` lives in `serea-capability`. | `crates/serea-protocol/src/types.rs`, `ids.rs`, `scheduler_contracts.rs`, `lib.rs`; code search across protocol/capability/task-engine/storage. | Approval lifecycle routing is durable in Scheduler; authoritative approval state is absent. No P6 runtime tests exist. | No; implementation must add only the agreed typed seam. |
| FROZEN | EventKind includes `POLICY_CHANGED`, `APPROVAL_REQUIRED`, `APPROVAL_GRANTED`, `APPROVAL_DENIED`, `APPROVAL_EXPIRED`, `APPROVAL_CONSUMED`, `APPROVAL_EXPIRED_UNUSED`, `CAPABILITY_DENIED`, and `CAPABILITY_REGISTRY_CHANGED`. `ApprovalLifecyclePayloadV1` is routing identity only, not authority. | Event Protocol §§3.3–3.5; `types.rs`; `scheduler_contracts.rs`; ADR-0030. | Events/wakes are durable; P6 must load authoritative records and acknowledge wake after applying outcome. | No for existing kinds. Revocation event semantics are unresolved. |
| ACCEPTED_ADR | P6 owns deterministic policy and approval/grant lifecycle; cannot mutate PreparedAction facts or invoke provider. P8 owns actual dispatch and mutable pre-dispatch rechecks. | ADR-0036; Capability Protocol §10. | P6 state/evidence requirements below. Tests must prove no invoke. | No. |
| NONCLAIM | No P6 policy decision, approval grant, approval request authority, or provider effect exists in current runtime. | Same source inventory. | No P6 pass is claimed. | No. |

## P6 spec inventory

| Semantics | Classification | Contract/source and owner | Enforcement point | Durable requirement and tests | Owner choice |
|---|---|---|---|---|---|
| Policy defaults | CONTRADICTORY | Policy §4.1 grants class defaults (OBSERVE/LOCAL_STATE allow; writes generally require approval; CREDENTIAL handoff); §4.2 says unmatched rules fall back to RequireApproval. | P6 deterministic evaluator. | Tests distinguish absent explicit rule from class-default rule for each class. | **Yes: D1** clarify whether built-in class defaults count as matching rules, and when generic fallback applies. |
| Rule order | CONTRADICTORY / AMBIGUOUS | Policy §4.2 says most-specific-first; §5 says priority descending then rule_id ascending. No formal specificity score is defined. | P6 evaluator; must not depend on Vec insertion, SQL row order, or HashMap iteration. | Table-driven conflict and permutation/restart tests. | **Yes: D2** select precedence and whether priority alone orders matching explicit rules. |
| DENY vs ALLOW conflict | AMBIGUOUS | Approval §4 states a grant cannot override a policy Deny; Policy §5 does not state conflict resolution between multiple matching allow/deny rules. | P6 evaluator before approval matching. | Matching allow/deny permutations always yield the selected deterministic result; grant cannot override final Deny. | **Yes: D3** choose deny-overrides or explicit ordered winner. |
| Policy input facts | FROZEN + PROPOSED | PreparedAction facts listed above are immutable; Task.policy_class, Task state/deadline/cancellation, policy revision, and trusted host automation context are additional host-owned inputs. Policy Protocol §6 allows descriptor, Task, ledger, context, declared classes/device state; model fields forbidden. | TaskEngine composes `PolicyInputV1` (or equivalent) from Task + PreparedAction + durable policy snapshot; no policy crate reads mutable provider/model fields. | Tests forge proposal fields/requester and demonstrate unchanged authority; snapshot identity captured. | **Yes: D4** decide explicit immutable `PolicyInputV1` representation and which runtime context/device facts are necessary. |
| RequestedBy semantics | FROZEN / AMBIGUOUS | MODEL, USER, SCHEDULER, PROACTIVE_WATCHER, SYSTEM are trusted enum variants; provenance alone grants nothing. Proactive writes are forbidden by Policy §4.3; scheduler defaults are not fully enumerated. | Trusted caller constructs input; P6 applies host context rules. | Tests prove no caller self-upgrades and each origin is evaluated deterministically. | **Yes: D5** set defaults for SCHEDULER and SYSTEM, and exact relationship of RequestedBy to INTERACTIVE/SCHEDULED/PROACTIVE context. |
| Task ceiling mutability | FROZEN | Task Protocol says immutable; P5 Task row stores policy_class. Existing storage mutations must not permit raising it. | TaskEngine/storage own Task mutation; P6 checks descriptor risk against exact Task ceiling before grants. | Negative update test and grant-does-not-bypass test. | No. |
| Policy revision / evidence | AMBIGUOUS | Policy §5 says durable, versioned rules; §7 mandates before/after audited POLICY_CHANGED; no immutable revision ID/digest schema is prescribed. | Policy store and P6 evaluation; retain the evaluated revision reference. | Restart and historical-decision tests; rule update is atomic with event. | **Yes: D6** choose monotonic revision ID, content digest, or both, and whether full historical rules or immutable snapshot evidence are retained. |
| Hot policy update | FROZEN direction, AMBIGUOUS detail | Approval §4.1 demands current policy decision before resuming; ADR-0036/P8 recheck policy/approval before dispatch. Historical evaluation evidence must not be rewritten. | Reevaluate against current policy before P8 dispatch; authorization records evaluated revision. | Allowed->deny update before dispatch refuses; old evidence remains unchanged. | **Yes: D7** decide whether P6 authorization is merely advisory and exact re-evaluation point/atomicity with P8 dispatch. |
| Approval requirement order | FROZEN | Policy Deny is terminal; a grant can satisfy RequireApproval only. Credential handoff is distinct and never becomes ordinary approval. | P6 policy decision before grant match. | Deny+grant remains denied; handoff cannot be resolved by approval response. | No. |
| Approval binding | FROZEN minimum + AMBIGUOUS exact P5 identity | Approval requires TaskId, StepId, capability ID/version, exact arguments digest, task binding, scope and expiry; changed arguments/version/plan revision invalidate. P5 also pins generation ID and descriptor digest/implementation. | P6 request/grant rows and match; P8 recomputes digest from durable Step input and checks current pinned facts. | Tests wrong Task/Step, replaced Step, digest, version, descriptor and generation; exact identity cannot be replaced. | **Yes: D8** explicitly ratify inclusion of generation ID + descriptor digest in durable approval identity and whether implementation ID also binds. |
| Scope | FROZEN / AMBIGUOUS extent | Approval §3.2 requires capability-defined structural exact scope; wildcard scopes forbidden. §7 permits `max_uses: 3` batch examples. | Capability schema validates scope; P6 structural matcher compares action; no generic wildcard/interpreter. | Exact equality, malformed scope and cross-scope refusal tests. | **Yes: D9** decide whether initial P6 supports only exact-action scope or a capability-defined scope projection, and whether multi-use batching is in scope. No broader grant scope is frozen. |
| Expiry | FROZEN | Approval §4 defines valid iff `now < expires_at`; 30-minute request/grant defaults. Protocol has `EpochMillis` and Clock port. | Injected Clock/TestClock; expiry checked in durable transaction. | Boundary tests at expiry−1, exactly expiry, expiry+1; no ambient system clock. | No. Exact duration configurability (default vs mandated) requires D10. |
| Grant consumption | FROZEN | Approval §4.2: atomic consumption tied to StepId; repeat same Step is idempotent, never another decrement. `uses_remaining > 0`. | One Store transaction for matching checks and consumption; no SELECT-then-later-UPDATE gap. | Independent Store connection race; crash before/after commit and retry/reopen. | No for atomicity. **Yes: D11** choose whether P8 dispatch intent and use consumption share one transaction and how consumed authority is recovered after caller loss. |
| Revocation | AMBIGUOUS | Current Approval protocol does not define grant-revocation state/operation or revocation event; it does require current validity before resume. | Durable approval store, then P8 recheck. | Revoke unused/part-used grant, revoke while waiting, revoke after P6 authorization before P8. | **Yes: D12** specify revocation semantics and event/payload. |
| Duplicate responses | AMBIGUOUS | Event wake delivery is at-least-once; approval payload routes by IDs. No authoritative response idempotency matrix is stated. | P6 response transaction keyed by ApprovalId and trusted authenticated actor/session. | approve twice, deny twice, approve then deny, deny then approve, expired/cancelled/replaced request; deterministic idempotent result with no second grant. | **Yes: D13** choose response conflict policy and idempotency response representation. |
| WAITING_APPROVAL | FROZEN ownership | Task Protocol state exists; TaskEngine owns lifecycle; ADR-0030 says Scheduler only creates routing wakes and never transitions Task from event kind. | P6 returns typed request-needed/result; TaskEngine performs durable WAITING_APPROVAL transition. Policy crate must not mutate Task. | Request/event and Task transition atomic composition; wake application restart/idempotency tests. | No for ownership; transaction composition detail covered by D14. |
| Request durability and summary | FROZEN direction / AMBIGUOUS privacy | Approval §2 requires durable request, host-written plain_summary, expiry/status; preview redacted at PRIVATE-or-higher. No authoritative model-written summary. | Host capability-specific summary builder and P6 request persistence; event metadata never raw args. | Restart/UI projection; sentinel tests for args/summary/event/log; failure if safe summary unavailable. | **Yes: D14** define approved summary/preview schema, privacy ceiling, and whether request creation + WAITING_APPROVAL + event is one atomic transaction. |
| Approval response authentication | FROZEN boundary / not implemented | Approval §5 requires paired-device session; elevated-device confirmation needs local biometric. Trust boundary must deliver verified principal/session; client JSON is not self-authenticating. | Core/device authenticated adapter validates response before P6 mutation. | Forged actor/session/request, wrong device, missing elevated confirmation tests. | **Yes: D15** define the typed trusted response port/authenticated actor seam (no Android transport implementation in P6). |
| Events | FROZEN existing vocabulary / AMBIGUOUS revocation | Required/granted/denied/expired/consumed/expired-unused and POLICY_CHANGED kinds already exist. ADR-0030 lifecycle events are routing-only and P3 wake consumption is explicit. | State change and event append in same transaction via upper-layer composition; storage never depends on Event Bus. | Inject event failure for each authoritative state transition and prove rollback. | **Yes: D12/D14** for revocation and exact payload/privacy/transaction mapping. |
| Cancellation/deadline | FROZEN direction | Task cancellation is terminal from any nonterminal state; Task deadline is immutable; Approval timeout fails explicitly. P8 checks cancellation/deadline again. | TaskEngine owns cancellation and waiting transitions; P6 expiry uses Clock; P8 refuses stale authorization. | cancel/expire while waiting; approve-after-cancel; deadline immediately before dispatch. | **Yes: D16** specify whether P6 immediately terminal-fails/marks request stale or leaves cleanup to TaskEngine when deadline/cancel occurs. |
| Credential handoff | FROZEN | `Authorization::CredentialHandoff`, RiskClass CREDENTIAL and RequireHandoff are not ordinary approval. Data Classification forbids credential bytes in Serea storage/events/model. | P6 returns typed handoff-required outcome only; credential-store/human channel out of P6. | Sentinel/token tests against rows/events/debug; approval cannot synthesize a credential handle. | No for separation; future channel details out of scope. |
| Retention/deletion | FROZEN broad Task cascade / AMBIGUOUS approval audit | Task Protocol says Task deletion cascades derived data; policy audit/history and grants have separate admin/audit retention; Approval says durable audit survives restart but no retention schedule. | P6 schema FK and retention policy. | Delete Task with pending/denied/approved/consumed state; audit linkage and replays. | **Yes: D17** choose cascade vs retain-with-null Task/Step IDs for each approval record and retention duration. |
| Bounds | AMBIGUOUS | Bounds Protocol has explicit global limits but no P6 max policy rules/grants/candidates/summary byte limits. No hidden constants allowed. | Validate at policy/admin write and request construction. | At/over bounds, adversarial scale tests. | **Yes: D18** choose whether P6 introduces explicit bounds and values; update Bounds Protocol if so. |
| Storage/source of truth | PROPOSED | Policy §5 says rules are durable; §7 says host-local admin only. No runtime store is implemented. Approval explicitly survives restart. | SQLite is likely sole durable authority; host config is a trusted input/admin interface, not a competing live rules store. State+events composed above Storage. | Restart, migration rollback, concurrent admin update and evaluation tests. | **Yes: D19** ratify SQLite as canonical policy authority and define admin config import/activation model. |
| Crate graph | PROPOSED / CONTRADICTORY | Crate Map places policy in L2, TaskEngine L3; it says `serea-capability -> serea-policy` is a future edge, while TaskEngine already depends on both. `serea-protocol/src/lib.rs` stale comment assigns ApprovalRequest/Grant to capability, unlike Crate Map §3.1 assigning PolicyDecision/approval lifecycle to `serea-policy`. | Prefer TaskEngine orchestration over `PreparedActionV1 -> serea-policy`; policy consumes protocol + storage + event-bus, with no upward TaskEngine dependency. Storage/EventBus remain below and never depend on policy. | `cargo metadata` acyclic graph; contract ownership tests/review. | **Yes: D20** choose owner crate for Approval types and whether capability needs any direct policy dependency. |
| Nonclaims | NONCLAIM | P6 does not dispatch, mint RequestId, invoke provider, process ActionResult/receipt/evidence, reconcile, store credential material, or implement Android transport. | Explicit end-of-phase gate. | Source search `.invoke(` in capability/task-engine/runtime policy has zero hits. | No. |

## Policy input and output seam

The policy input must be built exclusively from immutable P5 output plus host-owned durable Task/policy context. Candidate fields: TaskId, StepId, Task policy ceiling, Task state/cancel/deadline snapshot, exact generation ID, descriptor digest, CapabilityId/version, ProviderId/ImplementationId (authority/transparency as decided), RiskClass, SideEffectClass, `Authorization`, exact DataClass, trusted RequestedBy/automation context, CostClass/deadline, argument digest, IDK, and policy revision identity. Arguments themselves should not be passed to generic policy rules; capability-specific scope/summary builders may receive only the trusted prepared values they need and must not persist raw values unnecessarily.

Current protocol gives `PolicyDecision::{Allow, RequireApproval(ApprovalRequest), RequireHandoff(HandoffRequest), Deny(DenyReason)}` as prose in Policy §3, not as a Rust type. Preserve these semantic outcomes; do not invent new wire strings. Expired/cancelled are task/request lifecycle outcomes, not an `ALLOW` variant or an implicit retry. Free-text reasons must not control behavior; use closed reason codes. P6 should return a typed result such as policy deny / approval pending / handoff required / authorized reference, but its exact public/internal type requires D4/D8/D20 and must contain no RequestId or dispatch permission.

## Approval identity, atomicity, and P8 TOCTOU contract

Minimum already specified identity: ApprovalId, TaskId, StepId, CapabilityId + exact version, exact canonical `arguments_digest`, exact structural scope, expiry and use ceiling. Approval protocol invalidates on argument, version, or plan-revision change and task-binds every grant. To prevent same-Step replacement or stale descriptor reuse, the audit recommends additionally persisting exact registry generation ID and descriptor digest from P5; whether implementation ID binds is D8. PreparedAction identity cannot be recreated from a human description.

Approval expiry is strict `now < expires_at`, sampled through injected Clock/`EpochMillis`. Consumption and its state/event must be atomic. A one-use grant must not be double-consumed through two Store connections; the idempotency key is grant + consuming Step, with uniqueness in durable state. P8 must recheck current policy revision, Task cancellation/deadline, exact durable Step and arguments digest, Task ceiling, exact pinned generation/descriptor, live capability overlay, provider availability, grant expiry/revocation/scope/use availability, and any required authentication context immediately before dispatch. Mutable checks and dispatch authorization/consumption need an agreed atomicity boundary (D7/D11); stale P6 booleans are not trusted.

Threat cases explicitly covered in future tests: policy allows then changes to deny; grant revoked before dispatch; action args changed after approval; replacement Step with same capability; generation/descriptor changed; capability disabled while waiting; Task cancelled/deadline elapsed; replayed/guessed approval ID; forged USER/SYSTEM provenance; provider/model attempts to inject approval; approval cannot override disabled capability or DENY; credential sentinel never enters policy/approval state or events; host-written summary cannot be model-spoofed.

## Minimal future storage proposal (not implemented)

Migration 0005 is likely required because policy rules/revisions and approval requests/grants/consumption must survive restart. Keep one source of truth and do not mirror mutable rules in both config and SQLite.

Proposed minimum concepts, subject to D6/D12/D17/D19:

1. Immutable policy revision header (revision identity, digest, activation time/actor) plus normalized typed rules or canonical bounded rule document; a current-revision pointer. Preserve historical revisions referenced by evaluations.
2. Durable approval request (exact action identity, scope, safe host summary/projection, minimal DataClass, created/expiry, status, trusted response identity, terminal reason). Avoid raw arguments where digest and safe summary suffice.
3. Durable grant (request link, exact binding, scope, created/expiry, max/remaining uses, trusted granted_by, revocation/terminal status).
4. Grant-use rows unique by `(grant_id, step_id)` and linked to dispatch handoff; atomic checks/consume and corresponding event in the agreed transaction.
5. Evaluation evidence need not duplicate full PreparedAction if events/audit store immutable policy revision + action identity/digest and Task/Step links; owner must choose retention/deletion before schema freezes.

Do not add P8 dispatch intent, RequestId, result/receipt, duplicate suppression or repeat counters to the P6 migration. Do not store credentials, OAuth tokens, passwords, API keys, or raw CREDENTIAL values in any policy/approval row/event.

## Event and Task integration

Policy changes must update authoritative rules/revision and append metadata-only `POLICY_CHANGED` in one transaction. Approval request/grant/deny/expire/revoke/consume state and event must commit atomically where the event represents that transition. `serea-storage` must remain independent of Event Bus; fixed upper-layer composition coordinates participants, no SQL callbacks. `APPROVAL_GRANTED/DENIED/EXPIRED` lifecycle payloads remain routing only. P6 loads the authoritative request/grant, applies outcome, commits TaskEngine's legal `WAITING_APPROVAL` transition and then acknowledges the P3 durable wake; Scheduler does not mutate Tasks.

## Required future concurrency and crash matrix

| Race/fault | Required invariant |
|---|---|
| Two policy evaluations while rules activate | Each decision names one immutable revision; no mixed rule snapshot. |
| Approve vs deny duplicate response | One deterministic terminal outcome and at most one grant. |
| Approve vs request expiry | Exactly one transaction wins; boundary is `now < expires_at`. |
| Approve vs revoke / revoke vs dispatch | Revocation is durable; P8 cannot use stale authorization. |
| Two consumers of final one-use grant | At most one different Step consumes it. |
| Same Step retries consume | Idempotent same-Step use record; no second decrement. |
| Task cancel/deadline vs response | No cancelled/expired Task becomes dispatchable. |
| Policy change vs P6 authorization/P8 dispatch | Current policy is revalidated; stale revision detectable. |
| Request/event append failure | Request and event roll back together. |
| WAITING_APPROVAL transition failure | No orphan actionable request/wake; operation retry is idempotent. |
| Approval response commits, caller loses result | Reopen returns same terminal response/grant identity; no duplicate grant. |
| Use decrement/event failure; revoke/event failure | State and event rollback together. |
| Policy revision activation failure | Old revision remains authoritative; no event-only change. |
| Replacement Step response | Old request cannot authorize new Step even with same capability. |

All concurrency tests use independent Store connections. Crash tests inject failures at transaction boundaries and reopen the database; no hardware power-loss claim is implied.

## RED-first future TDD and implementation slices

- **P6A — owner/contract closure:** resolve D1–D20; update Policy, Approval, Task, Event, Bounds, architecture and accepted/proposed ADRs. No runtime.
- **P6B — durable foundation:** only after choices, migration 0005 with minimal policy revision/rule and approval request/grant/use state; atomic event composition, rollback/reopen/FK/integrity tests.
- **P6C — deterministic policy evaluator:** no rule/default, per-class defaults, ALLOW/DENY/REQUIRE_APPROVAL/HANDOFF, rule conflict/permutation, ceiling, context, RequestedBy, DataClass, risk/effect, overlays; model/provider spoof tests.
- **P6D — approval lifecycle:** safe request summary, authenticated response seam, expiry, duplicate response, exact grant matching, revocation and atomic use consume; wrong Task/Step/args/version/generation tests.
- **P6E — TaskEngine orchestration:** WAITING_APPROVAL ownership, durable wake processing/ack, cancellation/deadline, restart/idempotency; no provider call.
- **P6F — security/concurrency/crash closure:** full matrices above, immutable P8 revalidation references, independent connection races, final zero-invoke source gate.

At every slice, `CapabilityProvider::invoke` remains absent. P8 starts only after a separate approved P8 contract and dispatch gate.

## Owner decisions required

1. **D1 — fallback:** Does a class default count as a matching rule, with RequireApproval only when neither explicit nor class rule applies, or does any missing explicit rule require approval (overriding OBSERVE/LOCAL_STATE defaults)?
2. **D2 — precedence:** Is explicit `priority DESC, rule_id ASC` the complete winner rule, or must specificity precede priority? If so define a finite specificity order.
3. **D3 — conflicts:** Does any matching DENY override every matching ALLOW, regardless of priority, or does the deterministic priority winner govern?
4. **D4 — policy input:** Ratify an immutable `PolicyInputV1` and its exact host context fields, including whether policy receives raw validated arguments or only digest/typed facts.
5. **D5 — provenance defaults:** Ratify behavior for SCHEDULER and SYSTEM and map each RequestedBy to automation context; confirm proactive restrictions and caller authentication.
6. **D6 — policy identity:** Choose monotonic revision, immutable digest, or both; define retained historical rule snapshots/evaluation evidence.
7. **D7 — mutable policy:** Confirm current-policy re-evaluation before P8 and define atomic boundary with dispatch; define treatment of already-authorized historical decisions.
8. **D8 — approval exact binding:** Ratify generation ID + descriptor digest binding; decide whether ProviderId/ImplementationId are authorization identity or transparency only.
9. **D9 — scopes/uses:** Confirm exact-action-only vs capability-defined structural scope projection; decide whether P6 supports multi-use grants at all or starts with one use only.
10. **D10 — expiry configuration:** Is 30 minutes a fixed rule or default configurable by trusted host policy? Strict `now < expiry` remains frozen.
11. **D11 — consume/dispatch:** Define whether grant consumption and P8 dispatch-intent commit share the same Store transaction, and recovery semantics after commit/caller loss.
12. **D12 — revocation:** Define revocation authority, partial-use behavior, terminal semantics, event kind/payload, and race with dispatch.
13. **D13 — duplicate responses:** Choose approve/deny conflict ordering and idempotent result for repeats/late responses.
14. **D14 — summary/request transaction:** Define safe structured summary/preview constraints and atomic transaction spanning request, WAITING_APPROVAL state, event and wake routing.
15. **D15 — trusted response seam:** Define typed identity/session proof passed by Core to P6; elevated-device biometric evidence semantics remain outside raw P6 secrets.
16. **D16 — waiting cancellation/deadline:** Define immediate request/grant invalidation/expiry and TaskEngine cleanup when cancel/deadline occurs while waiting.
17. **D17 — deletion/retention:** Choose cascade vs anonymized retained audit for requests, grants, use records and policy evaluations; define retention durations.
18. **D18 — resource bounds:** Choose explicit maximum rules, grants, candidates and user-facing summary bytes (or explicitly rely on existing named bounds with cited values).
19. **D19 — policy source of truth:** Ratify SQLite durable authority and how local admin config changes/activates durable revisions without dual authority.
20. **D20 — ownership/graph:** Resolve `serea-protocol/src/lib.rs` stale ownership statement (Approval types attributed to capability) vs Crate Map (Policy owns PolicyDecision and approval lifecycle). Decide if `serea-capability -> serea-policy` is needed; preferred composition is TaskEngine calls capability preparation then policy, avoiding that edge.

## Audit conclusion

The high-level P5 → Policy → Approval → typed outcome → STOP boundary is accepted and unambiguous. However D1–D20 include behavior and authority choices not uniquely dictated by accepted contracts; several current prose clauses also conflict (notably fallback defaults, rule specificity/priority, and approval type ownership). Per the no-guessing gate, P6 implementation is **BLOCKED_PENDING_OWNER_DECISION**. This audit does not implement P6.

---

# Decision reduction and dependency analysis

Added 2026-10-10 at `318f15523cb94ff13e0c94da3d40788aaf2a1b6d`. This section does not
rewrite the audit above; the original D1–D20 record and its conclusions stand as written.
It records the analytical pass that reduced the twenty questions to six clusters, and the
companion [P6 owner decision package](P6-owner-decision-package.md) carries the reasoning.
Nothing here is owner-approved.

## Method

Each decision was re-derived against the currently accepted contract set and placed in
exactly one disposition. Two reductions rest on mechanically enforced facts rather than on
reading prose, and those do most of the work:

1. `tests/workspace_smoke.py:370` fails the build if `serea-capability` depends on
   `serea-policy`, so the `CAP --> POLICY` edge in
   `docs/architecture/03-crate-map.md:108` is not an available design. The audit's preferred
   TaskEngine orchestration additionally needs a smoke-test row for the new
   `task-engine -> policy` edge.
2. `ordinary_class` at `crates/serea-storage/src/task.rs:94-99` returns
   `AtRestProtectionUnavailable` for
   `DataClass::Private` ordinary rows (the same helper is duplicated at
   `crates/serea-storage/src/outcome.rs:166`; neither is a store-wide gate), and ADR-0022
   remains Proposed. A PRIVATE
   `plain_summary` column in an approval table fails at runtime today.

## Disposition map

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
| D16 cancellation and deadline | D | `IMPLEMENTATION_DETAIL_NOT_OWNER_CHOICE` |
| D17 retention and deletion | E | `CLOSED_BY_EXISTING_CONTRACT` |
| D18 bounds | E | `RECOMMENDED_OWNER_CHOICE` |
| D19 source of truth | B | `CLOSED_BY_EXISTING_CONTRACT` |
| D20 crate ownership | F | `RECOMMENDED_OWNER_CHOICE` |

Counts: 10 closed by existing contract, 1 deferred to P8, 3 implementation detail, 6 genuine
owner clusters.

## Why each was closed

| Decision | Existing contract that settles it |
| --- | --- |
| D1 | Policy §4.1 defines the class-default table and §4.2 steps 5 and 6 place it before the fallback. Approval §7 measures success as zero prompts for read-only work, which forbids MODEL B. The table is therefore a fixed code constant, not rule data |
| D2 | Policy §5 states `priority` descending then `rule_id` ascending. That is a total, mechanically testable ordering. The "most specific first" wording in §4.2 is the family order of evaluation stages, a different mechanism |
| D5 | Capability §4.1 states `requested_by` records provenance and never grants authority, and ADR-0016 fixes the proactive restriction. The one genuine residual is that `SYSTEM` gets a narrower posture, which is a Cluster A recommendation rather than a separate question |
| D7 | Approval §4.1 requires the current policy decision before resuming; ADR-0036 requires P8 to recheck; Policy §8 requires the live overlay to win. Direction is frozen. Only the transaction boundary is open, and that is P8's |
| D8 | Task Protocol §3 makes the Step binding immutable before P6 authorization, and migration 0004 enforces it with no-update and no-delete triggers. Generation and descriptor digest are therefore already bound by construction; provider and implementation are transparency only |
| D9 | Approval §3.2 freezes structural exact scope with no wildcards, and §3.1 plus §7.1 freeze `max_uses` with a batch example of 3. `max_uses` is already in the contract, so deferring multi-use would build the table twice |
| D10 | `approval_request_expiry_ms = 1800000` is already a named bound with scope and exhaustion behaviour in Bounds §2, governed by Bounds §3 |
| D13 | Device Protocol §5.2 already states that a response whose `approval_id` is not `PENDING` is dropped, that a duplicate is idempotent, and that a second `GRANT` never consumes two uses. AS-3 repeats it. The matrix in the decision package is a restatement with typed codes |
| D17 | Event Protocol §8 already assigns approval events to task retention and policy events to one year, and states that policy history outliving its task is intentional. ADR-0026 forbids event-to-task foreign keys. Task Protocol §8 already cascades derived data. Migration 0004's `ON DELETE CASCADE` on `task_id` is the established shape |
| D19 | Policy §5 makes rules durable data, §7 makes the host admin surface the only mutation path, and migration 0004 already implements the admin-to-immutable-generation-to-singleton-pointer pattern for the registry. SQLite as sole authority is precedent, not preference |

## Why each is deferred or is an implementation detail

| Decision | Reason |
| --- | --- |
| D11 | The P6 contract is that a use is consumed atomically and idempotently per `(grant_id, step_id)`. Whether that consumption shares a transaction with the dispatch intent is a property of the intent, which is P8's object. P6 only needs an operation that can join a caller's transaction |
| D4 | `PolicyInputV1` is an internal Rust type. Whether policy receives raw arguments is answered mechanically: Policy §6 forbids model output, and a rule language over raw arguments is an interpreter. Digest and typed facts only, no `now` |
| D15 | The seam's contents are fully determined by Device §5.2, Approval §5, AS-3 and Data Classification §3. It is a design fact with no preference left in it |
| D16 | Task cancellation is already terminal from any non-terminal state, the task wall clock is paused in `WAITING_APPROVAL` per Bounds §6.2, and P8 re-checks. P6 needs no cancellation path at all |

## Remaining genuine owner clusters

1. **Cluster A — policy semantics** (D1, D2, D3, D5). Class-default model, precedence, deny-override, provenance mapping.
2. **Cluster B — policy revision and source of truth** (D6, D7, D19). Revision identity, hot update, SQLite authority.
3. **Cluster C — approval identity and scope** (D8, D9, D10). Exact binding, scope projection, expiry bounds.
4. **Cluster D — lifecycle, response and privacy** (D11, D12, D13, D14, D15, D16). State machine, duplicate matrix, revocation, summary ceiling, seam.
5. **Cluster E — storage, retention and bounds** (D17, D18). Table shape, cascade, four new bounds.
6. **Cluster F — crate ownership, API and P8 seam** (D20, D4). DESIGN A, no capability-to-policy edge, `PolicyInputV1` and `AuthorizationEvidenceV1`.

## Owner response

The decision package is written so that one sentence — `ACCEPT P6 RECOMMENDED PACKAGE` —
ratifies the whole recommended design, including the six numeric and product defaults that
the package publishes. That sentence has not been said. An explicit owner override supersedes
only the value it names; nothing else is reopened by it. Six values are listed in the package
so an override can name one, and each has a recommended default already applied.

## 2026-10-10 — the response arrived, and what it did not settle

The owner replied **"承認します。"** on 2026-10-10. That ratified all six clusters, all six
defaults and product values, and the four explicit decisions DC-1 through DC-4, plus the Root
mandatory-approval rule. The ratification record is §0a of the
[P6 owner decision package](P6-owner-decision-package.md).

Ratification did not close P6, because the P6A feasibility gate then ran and found a
contradiction inside the ratified design itself. The short form:

- One capability Step is exactly one `PreparedActionV1` with one `arguments_digest`
  (`preparation.rs` takes one capability and one Step). A plan with three
  `calendar.event.create` calls is three Steps with three distinct digests.
- Approval Protocol §4 point 2 requires each executing Step's canonical `arguments_digest`
  to equal the digest in the approved request and in the grant. §3 carries exactly one such
  digest.
- Approval Protocol §7.1 — frozen contract text — endorses one grant with `max_uses: 3`
  covering three calls presented as one prompt.
- DC-1 ratifies `UNIQUE(step_id)` and `PRIMARY KEY(grant_id, step_id)`, and §4.2 makes a
  repeat consumption for the same Step a no-op, so one Step consumes a grant at most once.

The owner then resolved it the same day with **decision R2**, which is recorded in §0b of the
owner decision package and specified in §7 of
[P6A-feasibility-gate.md](P6A-feasibility-gate.md). The specific owner question the gate asked
was:

> **Does one approval grant bind exactly one Step, or may it bind a bounded, explicitly
> enumerated set of Steps each with its own exact `arguments_digest`?**

The owner answered with the second option and added twelve ratified invariants and a ratified
security boundary. P6A is therefore closed and P6B through P6F are authorized to proceed.

## 2026-10-10 — RED-first test obligations added by R2

Owner invariant 4 of R2 — only the individual actions a human actually approved are covered,
and scope-only or digest-only matching must never add authorization for a new Step — introduces
attack paths the earlier R-test plan did not cover. Each of the following is a RED-first
obligation for P6B through P6D. Every one must fail for the intended reason before the
implementation that satisfies it is written.

| # | Attack path | RED assertion | Enforcement point |
| --- | --- | --- | --- |
| RT1 | A Step **not enumerated** in the grant, with arguments identical to an enumerated member, attempts to consume | refused with `APPROVAL_ACTION_NOT_ENUMERATED`; no use row, no decrement | membership lookup before the decrement |
| RT2 | A Step not enumerated, sitting inside an approved `scope`, attempts to consume | refused; scope never authorizes | membership lookup, not the scope matcher |
| RT3 | A member row is inserted after the grant is committed | insert refused by trigger and by the Rust writer | no-late-insert trigger plus a typed writer check |
| RT4 | A member row is updated or deleted after the grant is committed | both refused by trigger | no-update, no-delete triggers |
| RT5 | A request unit enumerates the same `step_id` twice | refused with `APPROVAL_ACTION_SET_DUPLICATE` | request builder |
| RT6 | A request unit enumerates 9 Steps | refused with `APPROVAL_ACTION_SET_BOUND_EXCEEDED`; not silently split | request builder, against `approval_grant_max_uses` |
| RT7 | A unit mixes Steps with different `capability_id`, `version`, `generation`, `descriptor_digest` or `plan_revision` | refused with `APPROVAL_ACTION_SET_INCONSISTENT` | request builder, against `step_capability_bindings` |
| RT8 | `action_set_digest` does not match the canonicalized ordered member array | the grant is refused and an evaluation against it is refused | digest recomputation at write and at read |
| RT9 | A member's stored `arguments_digest` differs from the durable `task_steps.input_digest` | the request is refused before it is ever shown to a human | request builder, reading durable state |
| RT10 | A `GRANT` response grants a proper subset and a caller then tries to widen it | widening refused; only the frozen subset has authority | no-update trigger plus the Rust writer |
| RT11 | An unapproved Step of a partially granted unit attempts to consume | refused; it is not a member | membership lookup |
| RT12 | A member Step consumes, then a second grant for the same Step is offered | refused by `UNIQUE(step_id)`; no second independent consumption | use-table uniqueness |
| RT13 | A Step whose earlier grant was revoked **unused** is re-approved and consumes | allowed; no use row existed | DC-1 first limb |
| RT14 | An enrolled member's durable `plan_revision` advances between approval and consumption | refused; `plan_revision` is a shared condition | pre-consume revalidation |
| RT15 | A cross-task replay of an enumerated `step_id` | refused; `task_id` is a shared condition and the Step belongs to another task | membership and shared-condition check |
| RT16 | `max_uses` exceeds the granted member count, or a response proposes more than 8 | refused; `max_uses` equals the member count | grant minting, with clamping to the ratified bound |
| RT17 | A member Step is replaced by a new `step_id` with identical arguments | refused; the new Step is not enumerated | membership lookup |
| RT18 | Concurrent consumption by two enumerated members of a two-member grant | exactly two use rows, exactly two decrements, no third | one transaction per consume, independent connections |
| RT19 | A nondeterministic member ordering produces two different `action_set_digest` values for the same set | refused; ordering is `step_id` ascending byte-wise | normalization test, plus a restart determinism test |
| RT20 | Approval lifecycle events for a multi-action unit | exactly one event per transition, routed on the leading member, `max_events_per_transaction` respected | event composition, no per-member fan-out |

RT1, RT2 and RT11 are the ones R2 exists to make true, and RT3, RT4, RT10 and RT12 are the
ones that would silently reintroduce the "approved: bool" architecture if they were skipped.
