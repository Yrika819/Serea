# P8A owner decision package — provider execution contract

**Status:** PREIMPLEMENTATION / PROPOSED; no decision below is ratified.
**Boundary:** P8 alone may invoke a provider, and only after a dedicated dispatch authorization transaction commits. This document does not implement or enable provider execution.
**Severity:** BLOCKER prevents first invoke; MAJOR is a high-risk contract choice; MINOR is bounded; NOTE records a known limitation.

## Accepted existing contract

- [ADR-0019](../decisions/ADR-0019-canonical-json-and-idempotency-preimage.md) fixes SCJ-1 and IDK-1, where all attempts of the same Step use one stable IDK; reconciliation that proves absence closes the old Step as `RECONCILED_ABSENT` and a fresh action uses a new StepId and key.
- [ADR-0032](../decisions/ADR-0032-model-dispatch-durability-and-accounting-v1.md) governs **model** dispatch accounting and is not provider dispatch authority.
- [ADR-0036](../decisions/ADR-0036-p5-p6-p8-authorization-and-dispatch.md) assigns provider invocation, duplicate/repeat/budget checks, durable intent, result, receipt and reconciliation to P8. RequestId is per committed dispatch attempt. Provider IO occurs after intent commit; no SQLite write transaction spans IO; no exactly-once claim.
- [ADR-0038](../decisions/ADR-0038-approval-identity-lifecycle-and-authenticated-response.md) fixes authenticated approvals, R2 bounded enumerated action-set membership, and grant lifecycle. A grant is not a blanket permission.
- [ADR-0039](../decisions/ADR-0039-p6-p8-revalidation-and-grant-consumption-boundary.md) accepts Option B: grant-use consumption, budget/IDK reservation and dispatch intent commit atomically.
- [ADR-0041](../decisions/ADR-0041-p6-initial-approval-gates.md) fixes P6 initial approval planning. Approval and grant facts require re-read at P8 dispatch.
- [Capability Protocol §§5–9](../protocols/01-capability-protocol.md#5-actionresult) fixes typed ActionResult, provider receipt/evidence and currently says IDEMPOTENT may retry after AMBIGUOUS; the consistency question is explicitly proposed for owner ratification below.
- [Task Protocol](../protocols/02-task-protocol.md), [Approval Protocol](../protocols/05-approval-protocol.md), [Bounds Protocol](../protocols/10-bounds-protocol.md), and [Data Classification](../protocols/09-data-classification-protocol.md) govern lifecycle, R2 grant use, quotas, and data/credential ceilings.

## Verified current implementation

1. `TaskEngine::evaluate_prepared_action` in `crates/serea-task-engine/src/engine.rs` loads current Task, Step, pinned registry binding, descriptor, overlay and policy facts and returns a point-in-time evaluation. Its own comment says it is not execution authority. It does not gate on all dispatch conditions such as exact lease fence and post-commit intent identity.
2. P6 has durable approval requests/grants and joinable grant consumption, including R2 enumerated member constraints and global `UNIQUE(step_id)` use. P6 has no provider dispatch intent or `RequestId` minting.
3. Storage has `transact_with_participants` and a durable lease generation/owner fence. Current `side_effect_receipts` in migration 0001 have unique Step binding, task/step FKs, an idempotency-key match trigger, task match trigger and a trigger requiring the Step already be `SUCCEEDED`. This legacy insert ordering is not by itself a P8 atomic success+receipt API.
4. `CapabilityProvider` is a protocol port; no production provider execution is authorized by this package. Crate Map places orchestration above Storage, Policy and Capability. `serea-capability -> serea-policy` is forbidden.
5. Credential-store crate is specified in Crate Map but is not yet production-ready provider integration. No live credential or external-provider access occurred in this audit.

## Proposed owner decisions

### P8-D01 — dedicated dispatch authorization and status matrix (**BLOCKER**)

Policy explanation, approval planning, dispatch eligibility, retry eligibility and reconciliation eligibility are distinct outputs. A `PolicyEvaluation` or `AuthorizationEvidenceV1` is explanatory, point-in-time evidence and is never a dispatch permit.

| Step status at request time | Policy explanation | Approval planning | New dispatch eligibility | Same-Step retry | Reconciliation |
| --- | --- | --- | --- | --- | --- |
| `PLANNED` | Yes, if prepared binding is current | Yes; can create/refresh an approval plan under P6 rules | No. First acquire lease and validate current Task/plan/Step; intent commit transitions to `EXECUTING` | No prior intent/attempt to retry | No dispatch to reconcile |
| `LEASED` | Yes | Yes, if grant/request remains needed | Candidate only. Atomically re-read current facts, exact lease owner+generation+expiry and Task; commit intent/use/budgets and `EXECUTING` together. Provider call only after commit | A reclaimed lease is not proof of no effect. Inspect prior intent first | Yes if durable prior intent exists and may have reached provider |
| `EXECUTING` | Yes as explanation only | No new approval plan may silently replace an in-flight approved action | No second fresh intent for same logical action; recovery attaches to existing committed intent | Only through explicit recovery rule and existing intent identity; never a fresh effect authorization merely because grant use exists | Yes; reconcile committed/invoked/ambiguous action |
| `WAITING` | Yes | Depends on wait kind; approval response/wake uses P6 lifecycle | No | No | Only if separate existing dispatch intent proves an earlier effect may have occurred; wait alone is insufficient |
| `SUCCEEDED` | Historical only | No | No | No | Read/verify existing receipt and result; no re-invocation |
| `FAILED` | Historical/current explanation | No new authority for failed logical action | No | Only if an explicit retryable failure was durably classified as **confirmed no-effect** and Task Protocol permits reuse; otherwise new Step after reconciliation and fresh policy/approval | Yes when failure/timeout left an effect uncertain; no if durable record proves no intent or pre-invoke failure |
| `RECONCILED_ABSENT` | Historical only | New Step may be planned | No on old Step. New Step gets fresh preparation/policy/approval/IDK | No; closed terminal Step | Reconciliation is complete as confirmed absent; audit binding retained |

Task-level gate: only a nonterminal, uncancelled Task in an execution-permitted state with `now < deadline_at` (when present), exact current `plan_revision`, remaining tool-call/repeat bounds and matching Step can dispatch. A stale/cancelled/expired Task cannot be rescued by a grant. Exact permitted Task-state list must be frozen against Task Protocol transitions before implementation; recommendation is `EXECUTING` only for an already admitted execution, with `READY` requiring the owning TaskEngine to atomically transition into `EXECUTING` as part of admission/dispatch orchestration.

**Recommendation (proposed):** add a dedicated P8 TaskEngine entry point (e.g. `TaskEngine::dispatch_prepared_action(PreparedActionV1, LeaseGuard, ...)`) that never accepts policy evidence as authority. It verifies: TaskId/StepId; current Task state, cancellation and deadline; immutable current plan revision and Step revision; attempt; input/arguments digest; IDK; exact lease owner and generation and unexpired lease; prepared action binding; task-pinned registry generation, descriptor digest/provider/implementation/schema; live overlay and availability; current policy decision; current approval request/grant and R2 membership/expiry/revocation/remaining uses; duplicate and repeated-action policy; task/tool/IDK budgets. Grant use is valid only for the exact enumerated member and exact digest. Any mutable fact change refuses dispatch and reevaluates from current data.

Recovery identity must include the committed intent ID and RequestId; the original TaskId/StepId, IDK-1 and exact immutable PreparedAction identity are compared. `RequestId` does not replace IDK and is not reused for a new committed dispatch attempt. A recovery path cannot mint a new intent while an unresolved intent exists. No new `StepStatus` is proposed; a separate intent lifecycle stores `COMMITTED`, `INVOKING`, `AMBIGUOUS`, `RESULT_STAGED`, `COMPLETED`, `FAILED`/confirmed-no-effect, and `RECONCILED_ABSENT` as appropriate. Step lifecycle remains within the existing enum and is atomically mapped to its allowed states.

### P8-D02 — transaction owner and dispatch-intent API (**BLOCKER**)

**Preserve ADR-0039 Option B.** One SQLite write transaction must re-read current Task/Step/lease and capability binding; verify current Policy and Approval/Grant; check descriptor/registry, overlay/availability, exact R2 set membership, duplicate suppression, repeated-action count, tool-call budget and IDK state; mint RequestId; reserve IDK; consume applicable grant use; write dispatch intent and required events/audit; update execution/attempt state. All either commit or roll back. Provider IO begins after commit. There is no open SQLite write transaction across provider IO. Intent commit is the authorization linearization point.

**Recommendation (proposed):** `serea-task-engine` owns the P8 orchestration and the only public `dispatch` API. `serea-storage` owns transaction mechanics and a narrow typed dispatch-intent transaction operation; do not expose a generic “insert intent/consume grant” API or arbitrary SQL closure to provider callers. Compose Policy/Approval participants through the existing `Store::transact_with_participants` boundary, with P8-specific transaction command types that carry the exact prepared binding and lease fence. Storage re-reads DB facts inside the transaction; it does not trust booleans supplied by the caller. Policy recomputation/read must use the current active revision in that transaction or a revision token rechecked there. Internal write primitives should remain private to their owning crate; no public API may commit intent without performing the complete gate. If current visibility does not permit such encapsulation, adjust the API boundary before P8 runtime work.

Failure before commit means no durable intent, grant use, budget decrement, IDK reservation, RequestId or provider IO. Failure after commit but before invocation leaves a recoverable committed intent and a consumed grant use; it does not imply provider effect. Commit result uncertainty after SQLite reports an error is resolved by reopening and querying the exact intent ID, never by assuming rollback and reissuing.

### P8-D03 — grant consumption and same-Step recovery (**BLOCKER**)

| Case | Proposed rule |
| --- | --- |
| Initial dispatch | Consume exactly one R2 member use with intent in same transaction; `UNIQUE(step_id)` remains binding. |
| Transport retry before provider may have observed request | Reuse same Task/Step/IDK; new RequestId only with a newly committed retry intent if provider semantics and protocol permit. The existing grant use does not create a second use row. |
| Repeated RequestId issuance | Never mint twice for the same committed intent. A new committed attempt gets a new RequestId and retains same Step IDK. |
| IDK-1 reuse | Same logical Step only. No fresh Step or semantically changed arguments reuse it. New Step after confirmed absence has new key. |
| Previously consumed grant | A use row proves historical authorization for the one committed logical action only. It does not authorize an unrelated second effect, retry after revocation/policy change, changed args, or new Step. |
| Grant expired/revoked | Blocks a **new** dispatch/retry intent. It does not erase the committed old intent; recovery may inspect/reconcile that exact action without new external effect. Whether a native same-IDK duplicate-return query counts as an effecting retry needs explicit P8 policy. |
| Policy changes | Re-evaluate current Policy for any new intent. Existing intent recovery is scoped to discovering/completing the already committed logical action and must not expand authority. A deny/cancel blocks fresh effect; reconciliation may remain necessary to learn what already happened. |
| Task cancellation | Blocks new intent/retry. Do not discard an in-flight result/receipt or stop necessary reconciliation. Delayed result can be stored as historical outcome without moving cancelled Task to success. |
| New Step after confirmed absence | New StepId/IDK and fresh Policy/Approval; any R2 grant membership is exact to its old enumerated Step and cannot transfer. |

Recommendation: narrow the meaning of a consumed grant to the one atomically committed dispatch intent/member. A new provider effect after that intent requires a separately authorized retry path. If a retry is itself a provider dispatch under the existing ADR-0036 RequestId contract, require a fresh authorization check and decide whether fresh approval is required; never infer permission from `uses_remaining` having been decremented or from the prior member row.

### P8-D04 — AMBIGUOUS and replay/idempotency (**BLOCKER; protocol amendment may be required**)

The Capability Protocol §8.1 table says `IDEMPOTENT` retry on AMBIGUOUS is “Allowed.” ADR-0036 separately freezes “AMBIGUOUS effect is never blindly retried.” Do not silently rewrite either Accepted text. Proposed reading: “allowed” can mean allowed only after the P8-specific same-action recovery gate and documented idempotency contract, not blind reinvocation. Owner must ratify a clarification/amendment before the first invoke.

#### Compatibility matrix

Each cell is a provisional safe default. `REPLAY` means same Step, same IDK-1, exact args/binding and new RequestId only for a separately committed retry intent; `RECONCILE` means no provider reinvocation until host-reviewed/provider-authoritative resolution; `FRESH` means old Step closed confirmed-absent and a new Step/IDK with fresh Policy/Approval. The provider's IdempotencySupport never overrides ReplaySafety.

| ReplaySafety \ IdempotencySupport | `NATIVE` | `EMULATED` | `NONE` |
| --- | --- | --- | --- |
| `IDEMPOTENT` | Same-key replay can be safe only if the provider contract guarantees deduplication for exact IDK across the declared retention horizon; after AMBIGUOUS, first reconcile or invoke an explicitly ratified same-key recovery operation. Not “blind retry.” | Host records IDK/dispatch uniquely and prevents second dispatch while prior intent may have reached provider. `RECONCILE`; host emulation alone cannot prove provider did not effect. | No duplicate guarantee; after invoke-started uncertainty, `RECONCILE`, never automatic replay. Confirmed absence → `FRESH`. |
| `CONDITIONAL` | Retry same Step/key only after authoritative no-effect confirmation, per Protocol §8.1. AMBIGUOUS → `RECONCILE`; confirmed absent closes old Step then `FRESH` under current policy/approval (must resolve the protocol's “same step transport retry” wording). | No-effect confirmation required; emulation blocks duplicate intent but cannot substitute for provider no-effect. AMBIGUOUS → `RECONCILE`, then `FRESH`. | Retry only after authoritative no-effect confirmation; otherwise `RECONCILE`/block; confirmed absent → `FRESH`. |
| `NON_REPLAYABLE` | Never replay a provider effect; `RECONCILE` or block. Native key does not widen replay safety. | Never replay; `RECONCILE` or block. | Never replay; `RECONCILE` or block. |

#### Failure-stage compatibility

| Failure stage | Durable state and required action |
| --- | --- |
| Before intent commit | Transaction rolls back all dispatch-side changes; provider was not called. Re-evaluate current facts; fresh approval if state changed. |
| After intent commit, before invoke | Query exact committed intent after restart. It is authorized to begin that already committed action only if Task is still uncancelled/unexpired, lease fence/intent generation is valid, overlay/provider availability permits, and current revocation/policy rule does not prohibit starting a new external effect. If any gate invalid, mark/refuse intent without invoke; consumed use remains an audit fact. Owner must decide expired/revoked-after-commit behavior. |
| Invocation started | Treat effect as possibly occurred. No new intent/dispatch until provider result or reconciliation resolves it. |
| Timeout | Timeout is not absence. Persist AMBIGUOUS; reconcile. |
| Effect occurred, acknowledgement lost | Recover via native idempotency lookup/replay only under exact contract; else host-reviewed reconciliation. No guessed receipt. |
| Response received, result commit failed | Keep original RequestId/intent, stage response only if securely durable; otherwise outcome is ambiguous and reconcile. Never create duplicate receipt. |
| Process restart | Enumerate unresolved intents in deterministic order; lease-fence worker and query provider/reconcile per matrix before effect. |
| Concurrent worker | One intent per logical Step/IDK and current lease generation; stale generation cannot invoke, commit result or receipt. |
| Grant revoked | Blocks fresh effect. Existing intent can be reconciled/read; do not re-use consumed grant for a new intent. |
| Policy changed | Fresh intent recomputes current policy. Existing committed intent recovery is limited to that intent; an explicit deny blocks new external effects. |
| Task cancelled | No new intent/effect. Preserve delayed outcome and receipt as history; do not revive Task. |

Same-Step transport retry and recovery of one committed dispatch are distinct from a fresh action. A retry is a new provider IO and needs an explicit compatibility path; recovery that reads/query-resolves the old RequestId/IDK without issuing another effect is not a retry. A fresh action is a new Step with new IDK and fresh approvals. Required amendment candidate: clarify Capability Protocol §8.1 ambiguous retry cell, same-Step retry language, conditional retry Step identity, and interaction with ADR-0036. No amendment is Accepted here.

### P8-D05 — receipt, result and success transaction (**BLOCKER**)

Do not create a competing receipt authority beside `side_effect_receipts`. Extend/use that durable authority through an additive schema/API only after reviewing migration 0001's unique-Step binding and succeeded-Step trigger. Propose a narrow transaction that validates exact `RequestId` ↔ dispatch intent ↔ Task/Step/IDK/capability; validates ActionResult schema against pinned descriptor; canonicalizes output and verifies canonical digest; checks provider reference and receipt provenance; validates host-reviewed side-effect identity; then atomically stores result blob/reference, receipt, Step `SUCCEEDED`, Task progress and events. Resolve trigger ordering by a new migration/transaction protocol—not by direct SQL bypass.

Invalid/absent receipt on an effecting success is a failure/ambiguous outcome; never mark success. Duplicate result is idempotent only if RequestId and canonical digest exactly match prior persisted result; mismatched duplicate becomes integrity/security refusal and no overwrite. Delayed result after cancellation is recorded as late historical outcome with receipt if valid, while Task remains CANCELLED. Crash during persistence rolls back result/receipt/step/events together; after restart reconcile the exact original intent. Classification inherits the maximum of request, response, receipt metadata and provider reference. PRIVATE fails closed under current ADR-0022; SECRET/CREDENTIAL never enters `ActionResult`, receipt, event, logs or ordinary SQLite. Apply task/receipt retention and shared-Blob reference rules without storing credentials.

### P8-D06 — credential custody (**BLOCKER before real provider integration**)

**Recommendation (proposed):** host selects an immutable provider/account binding; validated capability descriptor binds allowed account class and operation scope. Model output may request only a host-exposed opaque binding ID from the enumerated task context; it cannot select credential bytes, arbitrary CredentialHandle, arbitrary account, OAuth scopes or keychain item. CredentialStore resolves handle only after host authorization and only inside provider adapter. Require elevated user authentication for high-impact scopes (owner policy determines exact classes); bind sessions/devices where policy requires; never use a device ID as user ownership.

`serea-credential-store` is the sole Keychain/Keystore boundary, implementing platform-specific secure storage behind protocol-defined opaque handles. Provider and Core receive scoped handles, not secret bytes in model-visible structures. Validate handle owner/provider/account/scope, expiry, revocation epoch and operation before use. Revocation immediately blocks new dispatch and in-flight token refresh; rotation creates a new handle/version, invalidates old handle and records outcome. Audit metadata is content-free: provider, pseudonymous account binding, handle ID/version, scope code, actor/session/device, operation, timestamps, outcome; never token, authorization header, secret-derived unkeyed digest or raw credential error. Secret exclusion applies to model context, logs, SQLite, ordinary ActionResult, receipts, events, Memory and crash reports.

No real credential access, account connection or Keychain operation is authorized in this audit.

### P8-D07 — deterministic fake provider and fault model (**MAJOR**)

Before any real provider, test only deterministic in-process fake providers with a scripted effect ledger keyed by provider/account + IDK and RequestId; manual clock; separate SQLite connections; hooks at transaction begin, each state write, commit-before-invoke, invoke-start, effect-before-ack, ack-before-result-commit, receipt insert, event append and process reopen. Fake may simulate effect and lost response but must never issue network traffic.

| Scenario | Expected durable result |
| --- | --- |
| Success | One committed intent/use, exact RequestId, one fake effect, schema-valid digest, one receipt, Step SUCCEEDED and task progress/events atomic. |
| Typed refusal before effect | Intent records typed refusal; no receipt; no success; no retry unless no-effect is authoritative and policy allows. |
| Timeout / effect then response lost | Intent remains AMBIGUOUS; no second effect; grant use remains consumed; reconciliation required. |
| Duplicate dispatch | Unique logical intent/IDK guard blocks second invoke; exact duplicate result can return prior receipt; mismatched digest refuses. |
| Invalid receipt / digest mismatch | No success/receipt; transaction rollback or durable integrity-failure state; reconciliation if effect may have occurred. |
| Provider unavailable | Refuse before invoke, no intent/use/budget unless availability becomes false after intent commit; post-commit case remains recoverable and is not refunded. |
| Restart at every fault point | Reload exact intent/RequestId/lease generation; no duplicate intent/effect; deterministic reconciliation. |
| Concurrent workers / stale lease generation | Exactly one lease-fenced committed intent; stale worker cannot invoke or commit outcome. |
| Grant revoke/current-policy race | SQLite transaction serializes winning order; revoke/deny before dispatch commit blocks intent; after commit blocks future intent but preserves recovery. |
| Task cancellation/deadline race | If cancellation/deadline commits first, no intent; if intent commits first, no new effect after cancellation is observed; in-flight completion is stored without reviving Task. |
| Failure during outer commit | No partial intent/use/budget/event; query on reopen decides whether commit actually landed before retry. |
| Result persistence failure | Intent remains unresolved; no duplicate receipt; original action reconciled. |

Named tests should be introduced in P8 implementation, not in this docs-only change: `p8_dispatch_state_matrix`, `p8_intent_grant_atomicity`, `p8_request_id_recovery`, `p8_r2_member_exhaustion`, `p8_ambiguous_replay_matrix`, `p8_receipt_success_atomicity`, `p8_late_result_after_cancel`, `p8_stale_lease_fencing`, `p8_policy_revoke_races`, `p8_fake_provider_restart_faults`, and `p8_credential_exclusion`. This is a test plan, not a claim those tests exist.

## Migration, compatibility, risk and exit criteria

| Decision | Impact |
| --- | --- |
| P8-D01 state/dispatch gate | BLOCKER: additive dispatch-intent schema and narrow API; no change to existing StepStatus enum. Must preserve Task Protocol transitions and immutable pinned PreparedAction. |
| P8-D02 transaction boundary | BLOCKER: new additive intent/request/budget state; atomic grant-use + intent; write APIs cannot bypass gate. No provider IO in transaction. |
| P8-D03 consumed grant semantics | BLOCKER: no second effect based on historical use; fresh Step after confirmed absence uses new key and fresh approval. |
| P8-D04 replay ambiguity | BLOCKER: explicit owner decision and any required protocol amendment before `invoke`; existing Accepted wording stays untouched until ratified. |
| P8-D05 result/receipt | BLOCKER: preserve unique Step receipt authority; additive migration/API resolves existing success trigger ordering; output schemas and digests are mandatory. |
| P8-D06 credentials | BLOCKER for real providers; fake tests use no real credentials. New credential-store production boundary and platform review required. |
| P8-D07 fake-provider verification | MAJOR: required before considering real integrations; deterministic, no-network, fault-injected tests and state assertions. |

**P8A exit criteria (proposed):** owner ratifies D01–D07; replay semantics are reconciled with Capability Protocol §8.1 by explicit amendment if needed; exact task-state/lease/deadline linearization is specified; SQL parent uniqueness, unique IDK/intent/request/result/receipt constraints are proven; a single public TaskEngine dispatch API is defined; receipt trigger/transaction ordering is designed; credential backend and provider/account binding boundary is reviewed; full fake-provider durable-state matrix is accepted. Only then can a first P8 implementation slice be authorized. No real provider is enabled by contract preparation alone.

## Deferred future work

Production providers and accounts; real Gmail/Calendar/Android/Local MCP; P15 GoalLatch integration; exactly-once guarantees; global provider reconciliation service; live credential custody; physical secure erase. A P15 GoalLatch fake remains outside P8 scope.

## Exact source references

`crates/serea-task-engine/src/engine.rs` (`evaluate_prepared_action`, approval planning); `crates/serea-storage/migrations/0001_initial.sql` (`task_steps`, `side_effect_receipts`, receipt triggers); `crates/serea-storage/migrations/0004_capability_registry.sql` (pinned binding and overlay); `crates/serea-storage/migrations/0005_policy_approval.sql` and `0006_policy_authority_sealing.sql` (approval membership/use constraints); `crates/serea-storage/src/lease.rs` (LeaseGuard, owner/generation checks); `crates/serea-storage/src/store.rs` (`transact_with_participants`); `crates/serea-storage/src/outcome.rs` (lease-fenced outcome commits); `crates/serea-capability/src/preparation.rs` (immutable PreparedAction); `crates/serea-protocol/src/types.rs` (ReplaySafety/IdempotencySupport); [Capability Protocol §8](../protocols/01-capability-protocol.md#8-replay-and-idempotency); [Bounds Protocol §§2, 5](../protocols/10-bounds-protocol.md#2-the-bound-set).
