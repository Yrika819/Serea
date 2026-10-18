# P8B Owner decision packet — ADR-0046 Candidate C

**Decision status: ready for explicit Owner choices.** Candidate C is a
recommendation only. ADR-0046 remains PROPOSED / NOT RATIFIED and no production
authority path is enabled. Evidence baseline: PR #36 HEAD
`bfe47522d808388f82b55c67793184726273600d`.

For each row, choose the recommendation, an alternative, or “defer”; the next
implementation should begin only after the choices that define its trust
sources and compatibility boundary are recorded.

| Decision | Default recommendation | Alternatives | Main trade-off / risk | Adoption condition | Owner choice |
|---|---|---|---|---|---|
| **1. Authority owner and topology** | Approve a bounded proof of **Host-private coordinator → TaskEngine current lifecycle/Policy revalidation → Storage atomic transaction**. Host owns Worker admission, Clock, Owner verifier, and exact Provider binding. | Retain status quo and defer all authority work; request revised A/B/C comparison; place a typed service in a library crate with an explicitly named trust injector. | Host binary privacy best matches existing deployment boundary, but private Rust modules do not protect against arbitrary code inside that process. A public library service is easier to consume but makes verifier/issuer substitution harder to constrain. | Name the trusted process boundary, sole atomic-authority owner, and scope of the bounded proof. Approval authorizes tests only, not production activation. | `Approve C proof / revise / defer: ____` |
| **2. Worker admission and raw lease API** | For the first proof, one Host-owned Worker/session; Host derives an opaque admission and never accepts a caller WorkerId, owner string, generation, or `EpochMillis`. Keep Storage's generic lease mechanics but do not treat raw `Tx::acquire_lease` as admitted identity. | Narrow/remove the public raw lease API in a compatibility-breaking boundary; support multiple Workers via authenticated IPC and session renewal; defer admission. | Generic public Storage API permits external durable lease creation. Narrowing it can break consumers. A Host session only protects the sanctioned Host path and does not prove process isolation if other code can open the database. | Define attacker/process boundary, external Cargo compatibility expectations, restart invalidation, and session revocation owner. | `Single Host worker / IPC multi-worker / API break / defer: ____` |
| **3. Clock and expiry** | Host-owned clock. Monotonic time for in-boot freshness; wall time only for external expiry; preserve inclusive expiry (`now >= expires_at`); fail closed after restart/suspend when freshness continuity is unknown. Sample after writer reservation where transaction serialization matters. | Wall-clock-only with a tolerance window; persistent high-water clock; platform secure clock; defer expiry-sensitive dispatch. | Monotonic clocks reset across reboot; wall clocks can roll back or jump forward. Persistent high-water can be corrupted or unavailable. No threshold is currently authorized. | Choose trusted source, rollback/forward thresholds, suspend/reboot rule, high-water storage integrity and transaction sample point. | `Clock rule and thresholds: ____` |
| **4. Owner authentication issuer** | Host-owned verifier tied to a named OS/device trust source; challenge binds Owner principal, device/session, request digest, decision, exact selected Step set, nonce, freshness and expiry; nonce consumption/revocation is atomic with response persistence. | Local OS account session only; paired device key plus explicit local elevation; enterprise identity adapter; defer approval-backed effects. | A generic caller-supplied verifier can mint `AuthenticatedApprovalResponseV1`; wrapper type alone does not authenticate. Stronger device proof adds enrollment and recovery complexity. | Name the supported Owner identity source, challenge UX/confirmation level, nonce/replay model, revocation source and unavailable-platform behavior. | `Identity source and challenge policy: ____` |
| **5. Grant, Provider, and unresolved effect rules** | Validate the complete Grant set and full Request separately inside the serialized transaction. Bind exact Provider Arc + descriptor digest + registry/overlay generation + bounded fresh health through precommit and preinvoke. No Grantless OBSERVE exception. | Different permanent Grant member limit/digest; permit OBSERVE without Grant under a narrow policy; use an authenticated DB seal; define effect reconciliation before dispatch. | Existing schema seals are not cryptographic against a DB-file attacker; `ProviderId` and default Ready do not identify a live instance. Provider effects may succeed while durable completion is uncertain. | Decide Grant cardinality/digest/keying threat model, Provider replacement/readiness epoch, Grantless OBSERVE, and retry/reconciliation/retention separately. | `Grant / Provider / OBSERVE / effect decisions: ____` |

## Acceptance matrix for the next bounded proof

| Requirement | Accept when |
|---|---|
| Host route | Only Host-private coordinator can compose Worker, Clock, Owner, Policy, Grant and Provider facts for the sanctioned route. External public APIs are classified as generic storage/policy tools, not Host authority. |
| Worker and time | Caller cannot supply identity/time for the decision; restart/revocation and `now == expiry` behavior are deterministic and fail closed. |
| Owner | Host authority cannot be minted with an arbitrary downstream verifier; challenge fields are inspectable and bound to the current request; replay/revocation fail closed. |
| Grant | Full Request actions and approved subset are distinct; every sealed member and selected Step is checked in one storage transaction; malformed or stale state rejects before effect. |
| Provider | Same exact instance and generation is checked immediately before commit and invoke; replacement, stale health or default Ready cannot pass. |
| Compatibility and evidence | External Cargo positive/negative probes show reachable legacy bypasses and blocked Host-only constructors; tests remain test-only until a separate approval; no canonical migration or production effects. |

## Explicit no-go until separately approved

No ADR-0046 adoption, architecture version change, P7 Gate D, canonical
migration 0009/0010, production L2 Intent writer, L3 Permit factory, Production
Provider, credential path, Memory, Receipt, Retry, canonical migration,
external effect, Owner authentication activation, or production authorization.
ADR-0045 D1/D2/D3/D5 are directional constraints; D4 is still interim.

## Next smallest scoped work after the Owner response

If Candidate C proof is approved, start with test-only private Host composition
for one Worker and one fake Provider, add exact negative fixtures for stale
incarnation/time, forged verifier, complete Grant member set, and substituted
Provider Arc, and compile external Cargo probes that demonstrate public bypasses
remain outside Host authority. Keep all effect methods disabled. If an Owner
identity or clock source cannot be injected without a new public authentication
API, stop at that seam and return an explicit revised decision request.
