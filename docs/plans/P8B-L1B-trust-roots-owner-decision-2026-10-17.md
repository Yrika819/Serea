# P8B L1-B trust-root decision package

**Status: PROPOSED / NOT RATIFIED.** This document presents review choices. It does not change runtime authorization, approve ADR-0046, or authorize L2/L3 implementation.

## Decision order

Each item is an independent trust root. Recommended review order is (1) Host Worker issuer and Lease binding, (2) Host clock, (3) Owner approval issuer, (4) exact Provider instance/readiness, and (5) no-Grant OBSERVE and complete Grant membership. A Worker token does not authenticate an Owner. Until each required decision is accepted, durable observations remain explanatory facts and cannot produce dispatch authority.

## 1. Worker principal and Lease owner binding

### Current boundary

`LeaseGuard` carries a private origin marker and the durable Task, Step, owner string, generation, and unreleased row are checked by the same Storage fence used by lease mutation. `observe_lease_facts` returns those row facts. It does not establish who issued the `LeaseOwner` string, whether a Worker process is still admitted, or whether the guard belongs to the current Host session. Expiry is returned as a timestamp; the Storage API has no trusted current time.

### Choices for Owner

| Choice | Contract | Advantages | Residual risk |
|---|---|---|---|
| A. Host-issued in-process principal (recommended) | A Host-owned admission component creates a private, non-serializable `WorkerPrincipal` when it starts/adopts a Worker. Lease acquisition accepts that principal and binds its opaque incarnation to TaskId, StepId, Host session, and generation. Only the Host admission module can issue or revoke it. | Smallest change; blocks caller-created owner strings and same-string impersonation through ordinary APIs. | Arbitrary code that already controls the trusted Host process can call or subvert the issuer. Process crash invalidates all in-memory principals; recovery must reclaim with a new incarnation and fence the old generation. |
| B. Authenticated worker session | A separately authenticated local Worker channel establishes an incarnation-bound session; the Host verifies admission and binds leases to that session. | Clearer process boundary and admission audit. | Requires an IPC identity and key/session lifecycle design; does not defend a fully compromised Host or OS account. |

**Minimum API if approved:** private `WorkerPrincipal` construction; explicit Host admission/revocation; `acquire_lease(principal, task, step, expected_generation, ...)`; guard binding to principal incarnation and Host session; rejection after revocation/restart; no public conversion from `LeaseOwner` text. Define shutdown, crash, replacement, and same-string tests. A random nonce by itself is not authentication and does not protect a compromised Host.

**Owner decision:** choose A or B; explicitly approve the issuing module, restart semantics, Worker death/replacement behavior, and whether one principal may own multiple Steps. Until then: **no authenticated Worker principal is established**.

## 2. Trusted Host clock

### Current boundary

`Store::open` samples its injected `Clock` for initial migration metadata, but Store does not retain it. Lifecycle, policy activation, approval response/revocation, lease acquire/release/renewal, scheduler, and recovery operations accept explicit `EpochMillis` from their caller. `observe_l1_facts` does not call a clock and does not compare Lease or Grant expiry to now. In particular, a caller can choose the value supplied to a storage mutation; that is deterministic storage input, not proof of trusted time.

### Choices for Owner

| Choice | Sampling point | Advantages | Residual risk |
|---|---|---|---|
| A. Host clock sampled after transaction acquisition (recommended for decisions) | A private Host-owned clock is sampled after `BEGIN IMMEDIATE` succeeds, then used for all expiry/deadline decisions in that transaction. | The timestamp does not become stale while waiting for SQLite's writer lock; all predicates in one decision use one instant. | Wall clock can jump; monotonic time cannot survive restart as an absolute expiry; suspend/resume and OS tampering need policy. Holding the transaction while obtaining time must be a bounded local operation. |
| B. Host clock sampled before transaction | Host captures one timestamp and passes it into the operation. | Simple and testable; no clock call inside the Storage transaction. | Lock wait can make the timestamp stale, allowing an operation that should have expired before lock acquisition. Requires a maximum lock-wait bound and a reject/retry rule when exceeded. |

**Minimum contract:** define an injected Host-owned clock implementation and its issuer; define the canonical unit and exact boundary (`now >= expires_at` is expired); use monotonic elapsed time only within a boot/session and wall-clock anchors for durable timestamps; reject on detected rollback, unavailable clock, implausible forward jump, or uncertainty after suspend/restart according to an Owner-selected policy. A persisted high-water mark can detect some rollback but cannot authenticate a malicious OS clock. Do not let a request, Worker, or Provider supply authorization time.

**Owner decision:** select A or B and approve rollback/skew/suspend/restart behavior. Until then: **Grant, Task deadline, and Lease TTL checks are not trusted authorization checks**.

## 3. Owner issuer and authenticated approval

P6 `AuthenticatedApprovalResponseV1` is produced only after an `ApprovalAuthenticationVerifier` accepts a candidate; it then validates the response against the pending request and time supplied to validation. The verifier boundary exists, but the concrete Host issuer, credential/session source, freshness, revocation, audit, and replay protection remain Host responsibilities. This approval proves neither Worker admission nor process identity.

### Choices for Owner

- **A. Host-mediated OS/account session:** the trusted Host obtains a short-lived authenticated session from a configured platform authentication service and verifies a challenge bound to ApprovalId, decision, exact selected Step set, expiry, and nonce. The Host records issuer/session identity, auth strength, freshness, and replay outcome with the durable response.
- **B. Device-bound cryptographic approval:** an enrolled device signs the same challenge; a Host trust store verifies device key status, revocation, and freshness. Android/other device support requires a separate enrollment and recovery design.

Choice A is simpler for one local OS user. Choice B has stronger device separation but increases enrollment and recovery obligations. Neither protects against a fully compromised Host that can falsify its verifier or alter local storage. A shared OS user alone does not distinguish a person from another local process. Expired sessions must fail closed; retries must use the same approval identity and must not replay a consumed response.

**Minimum API:** concrete Host-owned verifier/issuer; challenge binding and nonce; session freshness and revocation check; durable audit fields; atomic response/replay rejection. Define supported device platforms and recovery. Keep Worker principal issuance separate.

**Owner decision:** choose A or B, define supported platforms including Android, session age, revocation source, and compromised-Host boundary. Until then: **P6 interface availability is not production Owner authentication**.

## 4. Host-owned Provider instance and readiness

A `ProviderId` string, Registry generation, Descriptor digest, and Overlay revision identify recorded selections; none proves that the Host still holds the exact live Provider object that was checked. `CapabilityProvider::health()` has a default Ready implementation, and a persisted readiness snapshot can become stale. Readiness can only be a bounded observation; it cannot promise future availability.

### Choices for Owner

- **A. Host-owned immutable registry entry (recommended):** Host resolves the exact `Arc<dyn CapabilityProvider>` by ProviderId plus Registry generation, Overlay revision, and Descriptor digest. A lifecycle lock pins that entry through pre-commit checking. Host checks fresh readiness before commit, then rechecks the same instance immediately before invocation.
- **B. Versioned provider session:** Provider admission returns a non-forgeable session/incarnation bound to the same registry pins. The Host checks the session before commit and invocation and revokes it on replacement/shutdown.

| Stage | Required observation | Failure handling if later approved |
|---|---|---|
| Before Intent commit | Exact live Host instance, pins match, fresh bounded health response | Reject before commit; no Permit exists |
| Immediately before invoke | Same instance/session and same pins; health checked again | After commit: consume/burn the one-shot Permit, record no success, and do not retry automatically |
| Provider call | Bounded request using already validated arguments | Result is an effect boundary; this package does not implement it |

**Minimum API:** Host-private registry handle retaining exact instance and pins; provider lifecycle synchronization; timeout-bounded readiness; explicit stale/replaced result. Never accept caller-supplied `ready: bool` as authority. A green health response is not a guarantee of future service or semantic honesty.

**Owner decision:** choose A or B and define replacement, timeout, crash, and post-commit mismatch semantics. Until then: **Provider identity/readiness cannot authorize invoke**.

## 5. OBSERVE without a Grant; complete Grant membership

### No-Grant OBSERVE choices

| Choice | Contract | Cost and residual risk |
|---|---|---|
| A. Policy-authorized read-only Provider Intent | Require a fresh P6 Policy `Allow`, exact Host Provider instance/readiness, Worker/clock trust roots, and a committed OBSERVE Intent. | Uses the D1 OBSERVE tool-call budget at Intent commit; even read-only providers may expose sensitive data. Owner approval required. |
| B. Authorization-None local observation only | Permit a separately enumerated pure local observation with no Provider call, no secret/root access, no handoff, and no destructive/effect semantics. | Must be structurally separated from Provider Intent and can still expose local data. Owner must define the exact operation set. |
| C. No no-Grant path | Require a valid Grant for every capability dispatch. | Simplest authorization rule but blocks benign read-only workflows; it does not remove Policy or Host trust-root requirements. |

Each is **Owner decision needed**. No Grant path is approved by this proposal. Root, secret, handoff, destructive, and effectful operations are excluded from any no-Grant exception. If A is selected, the D1 OBSERVE budget is consumed on commit and is not refunded after commit.

### Grant membership completeness

`Tx::observe_grant_facts` reads the selected Step's member digest, durable member count, seal presence, status, remaining uses, and selected-use marker. It does not enumerate and recompute every member row. The selected Step can therefore match while a different member row or the stored seal/count relationship is inconsistent.

Choices:

- **A. Trust the sealed aggregate:** acceptable only if the seal is generated from a canonical complete ordered member set in the same transaction, all membership writes are append/finalize fenced, every read validates seal version/count/digest, and Storage integrity checks recompute the aggregate. Existing behavior must be audited against all conditions before choosing this.
- **B. Read-only complete-set probe (recommended before dispatch):** enumerate every member in deterministic position order, validate uniqueness/count/digests/seal, and compare the selected member in the same `BEGIN IMMEDIATE` used for dispatch revalidation. No Grant use is consumed by the probe.

Choice B costs bounded reads proportional to the Grant's declared maximum membership and needs an explicit maximum. Corrupt or oversized sets fail closed. Neither choice adds a Grant dispatch writer. Existing writer invariants and corruption probes must be reviewed; test-only direct corruption does not establish a production write path.

**Owner decision:** approve A only with evidence for all aggregate invariants, otherwise approve B and its maximum member count. Until then: **selected-member projection is not proof of complete membership integrity**.

## Explicit no-go items and ADR status

This proposal does not accept ADR-0046, add an Intent writer or `CommitProof`, create a Permit, invoke a Provider, persist Credentials/Memory/Receipts/Retry state, add a canonical migration, or authorize external effects. Each trust root above remains `PROPOSED / NOT RATIFIED` pending explicit Owner decisions.

## Isolated language-level feasibility note (2026-10-17)

An isolated `/tmp` Rust sketch was compiled with stable `rustc` and run. A private module modeled a non-Clone `WorkerPrincipal`, a lease binding to principal incarnation plus owner text and generation, and a revocation bit. Assertions confirmed that a separately admitted principal with the same owner text, a stale generation, and a revoked principal do not satisfy the local predicate. A small expiry predicate treated `now == expires_at` and `now > expires_at` as expired, and `now < expires_at` as live.

This validates only that the proposed shape is expressible in Rust. It does not use Serea's `LeaseGuard`, prove an authenticated issuer, exercise SQLite, establish OS identity, validate clock sampling around transaction acquisition, or defend against Host compromise. The toy issuer is reachable only inside its private module in the sketch; production visibility and mint authority still require explicit Owner approval. No recommendation above changes and this note does not ratify any contract.
