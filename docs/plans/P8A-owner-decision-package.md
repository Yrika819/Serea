# P8A owner decision package — provider execution contract

**Status:** PREIMPLEMENTATION. P8-D01–D07 are **PROPOSED / NOT RATIFIED**; no decision below is ratified.
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

Every recommendation, matrix default, migration option, lifecycle rule, test gate, and API suggestion in this section is **PROPOSED / NOT RATIFIED**. Only items under “Accepted existing contract” are recorded as Accepted, and they are not amended here.

### P8-D01 — dedicated dispatch authorization and status matrix (**BLOCKER**)

#### Dispatch linearization and state transition proposal

This subsection makes the proposed state boundary explicit; it does not ratify a
new Task or Step transition. `Store` writes use SQLite `BEGIN IMMEDIATE`, so a
dispatch transaction and a cancellation transaction serialize. The proposed
linearization point is the successful commit of one transaction that re-reads
the Task, Step, plan revision, exact lease fence, and all authorization facts,
then commits the dispatch intent and required accounting together. A candidate
state transition is Task `READY` → `EXECUTING` (or remain `EXECUTING` for a
permitted parallel Step) and Step `LEASED` → `EXECUTING`; the existing
TaskEngine, not Storage or a provider, owns the Task transition. A `PLANNED`
Step first needs its normal lease acquisition. `WAITING`, terminal Step states,
and terminal/cancelled Tasks cannot enter a new dispatch intent. The existing
protocol does not yet freeze this exact matrix, so it remains an Owner choice.

If cancellation commits before dispatch admission, admission fails without an
intent or grant use. If dispatch admission commits first, cancellation cannot
erase or roll back that intent. There is still a real boundary between that
commit and provider IO: the proposed conservative rule is to check cancellation,
deadline, and live revocation immediately before invoke and never start IO after
one is observed. The Owner must decide whether a committed-but-not-started
intent is safely closed as not invoked or retained unresolved. A crash between
recording an invoke-start marker and the external call cannot prove absence;
recovery must treat it as possibly invoked. No SQLite transaction can include
the external provider call, so the design must preserve this uncertainty rather
than claim an atomic external effect.

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
| Task cancellation | Blocks new intent/retry and never reactivates Task. Preserve exact unresolved intent; delayed result cannot enter the normal outcome/Receipt path and remains non-authoritative until P8-D05's approved reconciliation path exists. |
| New Step after confirmed absence | New StepId/IDK and fresh Policy/Approval; any R2 grant membership is exact to its old enumerated Step and cannot transfer. |

Recommendation: narrow the meaning of a consumed grant to the one atomically committed dispatch intent/member. A new provider effect after that intent requires a separately authorized retry path. If a retry is itself a provider dispatch under the existing ADR-0036 RequestId contract, require a fresh authorization check and decide whether fresh approval is required; never infer permission from `uses_remaining` having been decremented or from the prior member row.

### P8-D04 — AMBIGUOUS and replay/idempotency (**BLOCKER; protocol amendment may be required**)

The Accepted texts conflict in more than one place. Capability Protocol §6.2 and
ADR-0036 say an AMBIGUOUS effect is never blindly retried; Capability Protocol
§8.1 says IDEMPOTENT retry on AMBIGUOUS is “Allowed”; Task Protocol §6 says an
expired-lease IDEMPOTENT Step is re-issued with the same key. For CONDITIONAL,
Capability Protocol §8.1 permits same-Step/same-key transport retry after
confirmed no-effect, but its AMBIGUOUS cell says close the old attempt and use a
new StepId/key after confirmed absence. These are frozen-contract conflicts,
not implementation choices. Do not silently rewrite any Accepted text.

**Recommendation (PROPOSED):** approve an explicit Protocol amendment before
any provider invocation. Until then, P8 must not issue a provider call. The
amendment should define `IDEMPOTENT` as effect semantics, not authorization to
retry; forbid automatic re-invocation after AMBIGUOUS; permit only read-only,
exact-intent reconciliation until an authoritative result exists; distinguish
a retry proven to precede any possible provider effect from an AMBIGUOUS
outcome; and resolve same-Step retry versus new-Step-after-absence, RequestId,
IDK, grant-use, budget and fresh-approval rules together. Provider-native key
retention and its expiry must be explicit; after that horizon the key gives no
deduplication guarantee. Alternative: prohibit every replay after an
invoke-start boundary and require reconciliation or human resolution, even for
NATIVE/IDEMPOTENT. This is simpler and safer but may leave more Tasks blocked.

#### Compatibility matrix

Each cell is a provisional safe default. `REPLAY` means same Step, same IDK-1, exact args/binding and new RequestId only for a separately committed retry intent; `RECONCILE` means no provider reinvocation until host-reviewed/provider-authoritative resolution; `FRESH` means old Step closed confirmed-absent and a new Step/IDK with fresh Policy/Approval. The provider's IdempotencySupport never overrides ReplaySafety.

| ReplaySafety \ IdempotencySupport | `NATIVE` | `EMULATED` | `NONE` |
| --- | --- | --- | --- |
| `IDEMPOTENT` | Same-key replay can be safe only if the provider contract guarantees deduplication for exact IDK across the declared retention horizon; AMBIGUOUS remains reconcile-only until an explicit amendment authorizes a narrow operation. | Host records IDK/dispatch uniquely and prevents second dispatch while prior intent may have reached provider. `RECONCILE`; host emulation alone cannot prove provider did not effect. | No duplicate guarantee; after invoke-started uncertainty, `RECONCILE`, never automatic replay. Confirmed absence → `FRESH`. |
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
| Task cancelled | No new intent/effect. Do not revive Task. Delayed outcomes use only the proposed P8-D05 reconciliation path; until that exists, remain unresolved and do not claim a canonical Receipt. |

Same-Step transport retry and recovery of one committed dispatch are distinct from a fresh action. A retry is a new provider IO and needs an explicit compatibility path; recovery that reads/query-resolves the old RequestId/IDK without issuing another effect is not a retry. A fresh action is a new Step with new IDK and fresh approvals. Required amendment candidate: clarify Capability Protocol §8.1 ambiguous retry cell, same-Step retry language, conditional retry Step identity, and interaction with ADR-0036. No amendment is Accepted here.

### P8-D05 — receipt, result and success transaction (**BLOCKER**)

The existing code does **not** support a late successful outcome on a cancelled Task. `commit_step_outcome` calls the lease-fenced normal path: the Step must be `EXECUTING`, the Task must be in the corresponding active state (`READY`/`EXECUTING`, or `VERIFYING` for verification), the lease generation/owner must still match and remain unreleased, and the transaction updates the Task as well as the Step. A cancelled Task fails that eligibility predicate. Migration `0001_initial.sql` also requires `side_effect_receipts.step_id` to be unique, the Task and Step FKs to match, the receipt IDK to equal the Step IDK, and the Step to already be `SUCCEEDED` before receipt insertion. The success transaction achieves this by updating the Step before inserting its Receipt inside one transaction. Cancellation itself changes Task metadata only and leaves Steps/receipts untouched. Therefore the ordinary P2 outcome API cannot persist a late result plus canonical Receipt while retaining `CANCELLED`; bypassing its fence or SQL trigger is prohibited.

#### Cancellation and late-result cases to resolve

| Case | Required proposed behavior / unresolved contract point |
| --- | --- |
| Cancel before intent commit | Cancellation wins the transaction race: no dispatch intent, grant use, RequestId, or provider IO. A later worker must re-read Task state and refuse. |
| Cancel after intent commit, before provider IO | Intent and grant use remain durable historical facts; cancellation is not rolled back. Re-read cancellation immediately before invoke; do not start new provider IO after cancellation is observed. Whether this already-committed intent is marked safely not-started or unresolved needs an intent transition definition. |
| Cancel after provider IO begins | External effect may occur; cancellation blocks new intent/retry but does not claim undo or absence. Keep the exact intent unresolved until result/reconciliation. |
| Cancel after effect, before acknowledgement | Effect may have happened. Do not dispatch again. Resolve only through exact RequestId/IDK/provider/account-bound reconciliation; no guessed Receipt. |
| Late valid result after cancellation | Validate exact committed RequestId, intent, Task/Step, IDK, capability/provider/account binding, schema, digest and Receipt identity. Keep Task `CANCELLED`. Current API cannot store it as normal success/Receipt. Proposed separate reconciliation operation must preserve the existing single canonical Receipt authority and unique-Step rule without silently changing Task or Step lifecycle. |
| Late invalid or mismatched result | Treat provider data as untrusted; quarantine or record content-free integrity failure against the exact intent. No result blob, success, trusted Receipt, second effect, or overwrite of prior evidence. |
| Result persistence fails after cancellation | Preserve intent as unresolved; no partial success/Receipt/event and no duplicate receipt. Retry persistence only for the exact validated response when its durable staging and identity are proven; otherwise reconcile the exact intent. |
| Process restarts while cancellation is unresolved | Load the immutable intent/RequestId and cancellation state; never reactivate Task or mint a replacement intent; resume read-only/result reconciliation under a fenced worker. |
| Duplicate late result or Receipt | Exact duplicate RequestId plus canonical digest may be idempotent only after comparison with the one stored outcome. Mismatch refuses. Unique `(step_id)` canonical Receipt authority forbids a second trusted Receipt. |
| Forget or retention before external effect is resolved | Do not erase the only evidence needed to determine whether an external effect occurred. Owner must define a minimum unresolved-intent retention hold, forget behavior, redaction/content-free evidence boundary, and backup reconciliation. Task deletion's existing cascade would delete its Step and Receipt rows, so unresolved P8 intent retention needs an explicit additive ownership/lifecycle design. |

#### Design choices for a cancelled Task's late outcome

| Option | Shape | Compatibility / security consequences | Limitations |
| --- | --- | --- | --- |
| **A — Dedicated late-outcome reconciliation/history transaction (recommended proposal)** | Add an additive durable late-outcome record linked to the exact committed intent and RequestId, then a narrow typed transaction validates and installs at most one canonical row in existing `side_effect_receipts` under a revised, explicit eligibility rule. Task remains `CANCELLED`; no dispatch is authorized. The canonical Receipt table and `UNIQUE(step_id)` remain the only trusted Receipt authority; staged provider data is not authority. | Preserves Receipt identity and duplicate protection while making the late path auditable. Requires additive schema and new typed Storage/TaskEngine API; likely requires explicit clarification/amendment to Task/Step outcome and receipt lifecycle contract. | Owner must decide permitted Step status when Task is terminal, whether a late canonical Receipt can attach to a non-SUCCEEDED Step or whether a separate reconciled terminal status is needed, validation/retention/forget rules, and whether no-result staging can hold safely classified data. Do not reuse ordinary success API. |
| B — Step outcome transition while Task remains `CANCELLED` | Permit a lease-fenced or intent-fenced Step-only completion/receipt while preserving terminal Task state. | Avoids Task reactivation but adds a lifecycle exception: a succeeded Step and Receipt under a cancelled terminal Task. Requires status/trigger/API change and owner review against frozen Task/Step contracts. | Risks making `TaskState` and Step aggregate semantics inconsistent; current Step success API also advances Task and cannot be reused. Requires explicit Task Protocol and possibly ADR-0036 clarification. |
| C — Preserve unresolved evidence; require later reconciliation | Store no trusted Receipt or succeeded Step. Retain exact intent and content-minimized unresolved evidence until an approved reconciliation path exists. | Safest fail-closed interim behavior; no competing authority or false success. | Does not represent a verified external effect as a Receipt; may leave user-visible outcome unresolved and requires a retention hold/reconciliation operator path. It is a temporary boundary, not complete late-result support. |

**Recommendation (PROPOSED / OWNER DECISION REQUIRED): Option A, with Option C as the fail-closed behavior until its schema/API and lifecycle contract are approved and implemented.** The design must add a dedicated typed reconciliation path and additive persistence, keep the Task permanently `CANCELLED`, authorize no new external effect, and preserve `side_effect_receipts` as the sole trusted Receipt authority with no more than one Receipt per Step. A late response is evidence to validate against the committed intent; it can never authorize another dispatch or create authority by itself. The late-result validation and Receipt eligibility rule require owner review; this package does not implement them or claim P2 already supports them.

**Interim boundary (PROPOSED):** until the Owner approves the late-outcome lifecycle and its Task/Step Protocol changes, use Option C: retain only the exact unresolved intent and content-minimized integrity/reconciliation metadata; do not write a canonical Receipt, mark the Step `SUCCEEDED`, or mutate the cancelled Task. Option A is the preferred complete design, subject to a decision whether its canonical Receipt may attach to a non-SUCCEEDED Step or needs an explicitly approved separate terminal Step state. Option B is the simpler schema/API alternative but changes Task/Step aggregate semantics. The existing `side_effect_receipts` success trigger and Task Protocol's terminal-state/Forget cascade make neither path implementable by reusing ordinary P2 success code.

For ordinary non-cancelled success, do not create a competing Receipt authority beside `side_effect_receipts`. Use an additive schema/API only after reviewing migration 0001's unique-Step binding and succeeded-Step trigger. A narrow success transaction validates exact `RequestId` ↔ dispatch intent ↔ Task/Step/IDK/capability; pinned-descriptor ActionResult schema; canonical output/digest; provider reference and Receipt provenance; and host-reviewed side-effect identity; then atomically stores result blob/reference, Receipt, Step `SUCCEEDED`, Task progress and events. Resolve trigger ordering through a reviewed migration/transaction protocol, never direct SQL bypass.

Invalid/absent Receipt on an effecting success is failure/ambiguity, never success. Duplicate result is idempotent only if RequestId and canonical digest exactly match the prior persisted result; mismatch is integrity refusal with no overwrite. Crash during persistence leaves no partial result/Receipt/Step/events; after restart reconcile the exact original intent. Classification inherits the maximum of request, response, Receipt metadata and provider reference. PRIVATE fails closed under current ADR-0022; SECRET/CREDENTIAL never enters ActionResult, Receipt, event, logs or ordinary SQLite. Task/Receipt retention and shared-Blob rules may not erase unresolved-effect evidence before the owner-selected reconciliation hold expires.

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
| Task cancellation/deadline race | If cancellation/deadline commits first, no intent; if intent commits first, cancellation still blocks provider IO when observed and never revives Task. In-flight or late outcomes follow the P8-D05 reconciliation path; current P2 cannot store late success against CANCELLED. |
| Failure during outer commit | No partial intent/use/budget/event; query on reopen decides whether commit actually landed before retry. |
| Result persistence failure | Intent remains unresolved; no duplicate receipt; original action reconciled. |

Named tests should be introduced in P8 implementation, not in this docs-only change: `p8_dispatch_state_matrix`, `p8_intent_grant_atomicity`, `p8_request_id_recovery`, `p8_r2_member_exhaustion`, `p8_ambiguous_replay_matrix`, `p8_receipt_success_atomicity`, `p8_late_result_after_cancel`, `p8_stale_lease_fencing`, `p8_policy_revoke_races`, `p8_fake_provider_restart_faults`, and `p8_credential_exclusion`. This is a test plan, not a claim those tests exist.

## Migration, compatibility, risk and exit criteria

### P8B design verification that can start after approval

No P8 schema or migration is present, so this is a validation plan rather than
DDL evidence. Under currently Accepted ADR-0043, the authorized order is P7
canonical migration `0009`, then P8 additive migrations `0010+`. P7's Task
Owner binding is available to later P8 tables; any owner-scoped P8 row should
carry `owner_id` and use an exact composite foreign key to the declared unique
parent key in `p7_task_owner_bindings`, not infer ownership from task input or
join through a non-unique prefix. Each proposed composite FK must be checked
against `PRAGMA table_info`, `index_list`, `index_info`, and
`foreign_key_list`; then adversarial inserts must show cross-Owner references
fail with `foreign_keys=ON`. The standalone P7 fixture is not the canonical
0009 and cannot be inserted in the normal catalog.

After D01–D05 contract approval, isolated P8B work can build disposable SQLite
fixtures and candidate additive SQL only: apply v8 → synthetic P7-owner fixture
→ candidate P8 DDL; check old v8 Tasks are quarantined/unbound until P7's
approved binding operation; validate fresh and pre-existing rows, checksums,
all FK parent keys, `quick_check`, `foreign_key_check`, uniqueness, delete and
retention behavior. Exercise transaction atomicity with separate connections
and fault points before/after each write and COMMIT. A deterministic fake
provider can live only in test code behind an explicit synthetic-only feature,
with a manual clock, scripted effect ledger keyed by exact IDK/RequestId,
separate SQLite connections, and process restart/fault hooks. It must have no
network client, credential dependency, real Provider registration, or default
feature reachability. P8B may test DDL and transaction mechanics after contract
approval; it does not permit invoke-path implementation, even to the fake,
until the Owner separately approves the P8B synthetic development scope.

Migration order B (P8 `0009`, P7 later) remains an ADR-0043 amendment proposal
only. It requires an Owner decision and a concrete dependency proof before any
catalog edit; specifically, P8 must not assume OwnerId or P7 Task binding if
those objects are later in the chain.

| Decision | Impact |
| --- | --- |
| P8-D01 state/dispatch gate | BLOCKER: additive dispatch-intent schema and narrow API; no change to existing StepStatus enum. Must preserve Task Protocol transitions and immutable pinned PreparedAction. |
| P8-D02 transaction boundary | BLOCKER: new additive intent/request/budget state; atomic grant-use + intent; write APIs cannot bypass gate. No provider IO in transaction. |
| P8-D03 consumed grant semantics | BLOCKER: no second effect based on historical use; fresh Step after confirmed absence uses new key and fresh approval. |
| P8-D04 replay ambiguity | BLOCKER: explicit owner decision and any required protocol amendment before `invoke`; existing Accepted wording stays untouched until ratified. |
| P8-D05 result/receipt | BLOCKER: preserve unique Step receipt authority; additive migration/API resolves existing success trigger ordering; output schemas and digests are mandatory. |
| P8-D06 credentials | BLOCKER for real providers; fake tests use no real credentials. New credential-store production boundary and platform review required. |
| P8-D07 fake-provider verification | MAJOR: required before considering real integrations; deterministic, no-network, fault-injected tests and state assertions. |

### Owner review checklist

All recommendations are proposals; no row below is ratified by this package. Required tests describe future evidence only.

| Decision | Accepted baseline / proposed choice and alternative | Compatibility / security impact | Dependency and tests | Blocks next phase? | Exact unresolved owner question |
| --- | --- | --- | --- | --- | --- |
| P8-D01 state and dispatch gate | ADR-0036 order and existing Task/Step states; proposed one dedicated dispatch entry point. Alternative: add a new explicit lifecycle state only through a separate contract decision. | Evaluation is not authority; preserve exact Task/Step lifecycle, pinned action and fences. | Current Policy/Approval, Task/Step/deadline/lease/registry checks; state matrix and stale-fence tests. | **Yes**, blocks any invoke. | Which exact Task states and lease/deadline linearization point permit first intent commit and postcommit recovery? |
| P8-D02 transaction boundary | ADR-0039 Option B is Accepted and unchanged; proposed atomic grant-use/budget/IDK/intent commit before IO. Alternatives A/C in ADR-0039 remain rejected. | No partial authorization; no SQL write across provider IO; no public bypass API. | Typed Storage/TaskEngine API; rollback, crash/commit uncertainty and R2 member tests. | **Yes**, blocks any invoke. | What exact narrow command/API owns the transaction and where are current Policy facts rechecked? |
| P8-D03 grant use and retries | ADR-0038/R2 exact Step membership and ADR-0039 one use per Step; proposed prior use authorizes only the committed logical action. Alternative: fresh approval per retry or restrict retries to reconciliation. | Prevents grant transfer, repeat use and policy bypass; same-Step IDK differs from a fresh Step/IDK. | P8-D04 semantics, fresh Policy/Approval and member exhaustion tests. | **Yes**, blocks retry and any first-invoke retry contract. | Can same-Step retry ever issue provider IO; if so, what fresh authorization/approval and provider conditions apply? |
| P8-D04 replay / AMBIGUOUS | ADR-0036 forbids blind AMBIGUOUS retry; Capability Protocol §8.1 currently says IDEMPOTENT retry allowed; proposed restricted exact same-key recovery matrix. Alternative: prohibit all replay after possible effect. | NATIVE/EMULATED/NONE never independently means exactly-once; provider claim is not host proof. | Owner selects matrix and any protocol amendment; fake effect-ledger, ambiguity and confirmed-absence tests. | **Yes**, blocks first invoke where protocol wording conflicts and every ambiguous retry. | What does “allowed” mean for IDEMPOTENT after AMBIGUOUS, and when can same-Step versus new-Step retry occur? |
| P8-D05 result, Receipt, cancellation | Existing unique-Step `side_effect_receipts` and SUCCEEDED trigger; proposed Option A typed reconciliation/history path, with Option C fail-closed interim. Alternatives B/C above. | Preserve sole Receipt authority; exact RequestId/intent binding; Task never reactivates; status/trigger semantics need explicit contract change. | Additive schema/API; success/late validation, duplicate, mismatch, restart, persistence-failure and unresolved-retention tests. | **Yes**, blocks trusted late Receipt/result and completion of result contract. | May a late verified Receipt attach while Task stays CANCELLED, and what Step status, retention hold, forget and restore rules govern it? |
| P8-D06 credential custody | Crate Map reserves CredentialStore as secret boundary; proposed immutable host provider/account binding and opaque scoped handles. Alternative: keep all real-provider integration deferred. | Secrets never enter model/ordinary rows/logs/ActionResult; authorization bound to owner/provider/account/scope/revocation. | CredentialStore production design, platform keychain/keystore review; exclusion, revoke, rotation and scope tests. | **Yes** for real providers; **no** for fake-provider contract work. | Which owner authentication/elevation policy, account-binding issuer, scope vocabulary and revocation/rotation semantics are required? |
| P8-D07 fake and fault model | ADR-0036 requires deterministic mock providers before production; proposed no-network scripted ledger and crash hooks. Alternative: narrower suite only if every omitted fault is explicitly justified. | No live credentials/network; validates durable truth, not exactly-once external behavior. | D01–D05 contracts; all named fake-provider restart, concurrency, cancellation and Receipt tests. | **Yes** before real integration; not a P8A ratification prerequisite to begin scoped fake implementation. | Which failure points and durable assertions are mandatory before any real provider review? |

The owner must separately resolve details not implied by these recommendations: retry authorization and approval, exact native idempotency retention horizon, provider/account binding, postcommit cancellation behavior, unresolved-effect retention/forget, PRIVATE result handling, and late Step/Receipt lifecycle. No broad recommendation ratifies these silently.

### Phase gates (proposed; no implementation gate is complete)

**P8A — contract ratification gate.** Owner reviews and ratifies or explicitly defers D01–D07; exact Task/Step/deadline/lease dispatch boundary and one-way provider IO boundary are specified; ADR-0039 Option B remains unchanged; same-Step retry versus new-Step execution and replay-safety wording are resolved, with any separate protocol/ADR amendment identified for owner approval; result/Receipt identity, duplicate authority and cancelled-Task late-result contract are selected; credential-custody boundary and fake-provider requirements are defined sufficiently for their respective future scopes. Conceptual review of constraints and API responsibilities belongs here. P8A does **not** require executable migrations, runtime implementation, or implementation tests to pass.

**P8B — schema implementation gate.** Before treating P8 persistence as validated, require actual additive migration validation; matching UNIQUE/PRIMARY KEY parent proof for every composite FK; runtime SQLite integrity and duplicate-intent/RequestId/Receipt constraints; grant-use+intent atomicity; cancellation/late-result/forget retention constraints; rollback and fault-injection evidence. No P8B gate is complete here.

**P8C+ — runtime and adversarial closure.** Require the single dispatch API, current Policy/Approval/lease/deadline/availability checks, provider-IO-after-commit proof, deterministic fake-provider matrix, exact result/Receipt validation, late-result and unresolved-effect recovery, restart/concurrency/revocation tests, credential exclusion, and independent adversarial review before any separately authorized real-provider slice. No P8C+ gate is complete here.

Owner approval selects a contract; conceptual proof reviews the proposal; executable migration proof validates real DDL/keys; runtime validation tests implementation; production readiness additionally requires adversarial, recovery, portability and operational evidence. These evidence levels are distinct. No implementation gate is marked complete, and contract preparation alone permits no provider invocation.

## Deferred future work

Production providers and accounts; real Gmail/Calendar/Android/Local MCP; P15 GoalLatch integration; exactly-once guarantees; global provider reconciliation service; live credential custody; physical secure erase. A P15 GoalLatch fake remains outside P8 scope.

## Exact source references

`crates/serea-task-engine/src/engine.rs` (`evaluate_prepared_action`, approval planning); `crates/serea-storage/migrations/0001_initial.sql` (`task_steps`, `side_effect_receipts`, receipt triggers); `crates/serea-storage/migrations/0004_capability_registry.sql` (pinned binding and overlay); `crates/serea-storage/migrations/0005_policy_approval.sql` and `0006_policy_authority_sealing.sql` (approval membership/use constraints); `crates/serea-storage/src/lease.rs` (LeaseGuard, owner/generation checks); `crates/serea-storage/src/store.rs` (`transact_with_participants`); `crates/serea-storage/src/outcome.rs` (lease-fenced outcome commits); `crates/serea-capability/src/preparation.rs` (immutable PreparedAction); `crates/serea-protocol/src/types.rs` (ReplaySafety/IdempotencySupport); [Capability Protocol §8](../protocols/01-capability-protocol.md#8-replay-and-idempotency); [Bounds Protocol §§2, 5](../protocols/10-bounds-protocol.md#2-the-bound-set).
