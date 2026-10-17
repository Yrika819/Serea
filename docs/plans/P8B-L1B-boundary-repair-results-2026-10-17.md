# P8B L1-B boundary repair feasibility results

**Status: synthetic feasibility only.** Work starts from PR #34 HEAD
`f3804a643e6334bc4f1dddfea1a0c096c1fd7a13` and is intended to stack on PR #34.
It does not adopt ADR-0046, enable production authorization, or authorize
Provider invocation, an Intent writer, or external effects.

## Grant full-set probe

`crates/serea-storage/src/migration_0005_tests.rs` contains a test-module-only
`probe_grant_full_set`. It receives the existing `Tx` from `Store::transact`,
which has already acquired SQLite's `BEGIN IMMEDIATE` writer reservation. The
probe is not exported, is absent from non-test builds, and performs only
`SELECT` queries. It returns typed rows only to its tests and is not connected
to any authorization or dispatch path.

The probe checks the Grant and its source Request together: status and identity
fields, declared counts, the ordered full Request action list, every Grant
member, the selected Step, each Step's task/plan/capability facts and registry
binding, one seal row, and use rows/counters. Both Request and Grant digests are
recomputed independently from their rows. The Request digest covers all
requested actions. The Grant digest covers the approved subset.

This digest split follows the code on PR #34, even though the campaign input
describes a different current contract: `approval_requests.action_set_digest`
is the full-request digest, while `approval_grants.action_set_digest` is
computed from selected members in `approval.rs::apply_response`. Migration
0006 also says the Grant commits an independent subset digest. Neither stored
digest is keyed or protected against an attacker who can change the SQLite
file and schema. `approval_grant_seals` stores `sealed_at_ms`; it is an
immutability fence, not a cryptographic commitment.

The eight-member ceiling is used only by this experimental probe because the
current Request and Grant schema bounds it at eight. This campaign does not
ratify eight as a permanent protocol rule.

The test cases create Requests and Grants through the typed Storage writers,
then separately use SQL only in adversarial fixtures to remove the relevant
SQLite trigger and corrupt data. They cover a single member, a one-of-two
subset, a two-member Grant, a missing row, a non-contiguous position, a
position reorder, arguments and scope digest edits, a missing seal, selected
Step mismatch, reopen, rollback, read-only row counts, an independent
connection waiting behind the probe transaction, and count/duplicate
corruption rejected by schema constraints. `Store::verify_integrity` still
passes for the application-level corruptions while the probe rejects them.
Those direct-SQL fixtures model local DB/schema tampering and do not claim that
normal typed APIs can create malformed Grants.

Both writer-contention tests start the competing connection only after the
first `Store::transact` closure has entered its real writer-reserved
transaction. This handshake makes the ordering assertion deterministic across
OS schedulers.

## Other trust roots

| Track | Established by code/tests | Still unproved or bypassable |
|---|---|---|
| W | `LeaseGuard` has private fields, is non-Clone, and carries Store-origin/task/step/owner/generation facts. The same private Storage fence checks its live unreleased row. | `Tx::acquire_lease` remains public and accepts caller-created `LeaseOwner` and caller time. A guard proves a DB row, not an admitted Worker or Host session. External consumers can still acquire raw leases. |
| T | `Store::transact` reserves `BEGIN IMMEDIATE` before invoking its closure. A test with two file-backed Stores verifies that one caller-owned sample occurs only after the writer reservation becomes available. | Runtime lease and approval APIs still accept caller-supplied `EpochMillis`. This does not prove clock source authenticity, rollback/forward-jump policy, or restart freshness. |
| O | The policy/storage layers check typed request/response structure and exact selected Steps. | `ApprovalResponseCandidateV1::new` and `AuthenticatedApprovalResponseV1::authenticate(candidate, verifier)` remain public. An arbitrary external verifier can mint the wrapper; private candidate fields and missing session accessors prevent a Host verifier from checking provenance through the present API. Synthetic accept fixtures do not authenticate an Owner. |
| P | The real capability crate accepts provider implementations; its existing replacement test shows two distinct `Arc` instances with the same ProviderId. No new invoke path is exercised. | `CapabilityProvider::health()` defaults to Ready. Equal ProviderId does not prove exact Host-owned instance or lifecycle. Host remains binary-only. Exact live-instance binding and readiness ownership need an Owner-approved Host boundary. |

The external Cargo probe already checks current public surfaces: arbitrary
lease acquisition, caller-created verifier minting, Policy fact composition,
Provider implementation, private Candidate access failure, and absent future
Permit/CommitProof APIs. A successful external compile is API reachability
evidence only; process and database file access remain a separate trust
boundary.

## Native Windows ARM64

PR #34 Native run `39036558000`, Stable Windows ARM64 job `117407503289`, failed
`host_lifecycle::model_accounting_is_bound_to_a_task_created_by_the_scheduler_task_engine`
at `host_lifecycle.rs:659`. The ninth repeated `model-smoke` child returned
`0xC0000142` (`STATUS_DLL_INIT_FAILED`) through a successful
`Command::output()` result with zero stdout/stderr. The child therefore failed
before its Rust command handler could write output; this is distinct from a
test assertion returned by the child or a SQLite busy result.

The same PR #34 run passed Stable Windows x86_64 and Rust 1.85.0 Windows ARM64;
the preceding PR #33 Native run `39030546016`, Stable Windows ARM64 job
`117389941104`, also passed. The failure is intermittent on the recorded
evidence. The all-feature suite starts many tests and child processes, so
process/loader resource pressure is a hypothesis, not a finding. This
environment is Linux x86_64 and cannot reproduce the Windows loader failure
or inspect runner DLL/module state. Status remains
`NATIVE_DLL_INIT_ROOT_CAUSE_UNCONFIRMED`; no retry, skip, sleep, or feature
disable is used as a fix.

## Owner choices remain open

- Worker admission must be owned by trusted Host process/session logic, while
  Storage keeps the durable lease row and generation fence. Decide whether the
  raw public lease API can be restricted or whether a new crate boundary is
  required.
- Decide Host Clock ownership, inclusive expiry (`now >= expires_at`),
  rollback/forward jump behavior, suspend/restart handling, and freshness
  limits. The transaction-order experiment answers none of those source-trust
  questions.
- Ratify the permanent Grant cardinality, Request/subset digest versioning,
  and whether a future schema should store an authenticated set commitment.
- Choose Owner authentication provenance, challenge/nonce/replay/revocation
  contract, supported OS/device scope, and Host-owned Provider instance and
  readiness lifecycle.
- Decide whether no-Grant OBSERVE has any exception. This campaign creates no
  such exception.

Production dispatch remains disabled. Any acceptance here is limited to
`L1B_BOUNDARY_FEASIBILITY_PARTIAL_OR_SYNTHETIC_ONLY`.
