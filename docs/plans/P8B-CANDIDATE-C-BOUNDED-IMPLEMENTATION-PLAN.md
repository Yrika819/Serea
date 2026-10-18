# P8B Candidate C bounded implementation plan

**Scope:** Owner-approved, test-only investigation on PR #37 HEAD
`5220f2e1493a136ef248e82a730e506254065936`. ADR-0046 remains
`PROPOSED / NOT RATIFIED`; production authorization and execution remain off.

## Planned minimum slice

- Keep all new coordination private to the `serea-host` test harness; use one
  host worker/session, test-owned clock, synthetic verifier, real SQLite-backed
  storage where the public API permits, and a fake provider `Arc`.
- Reuse current TaskEngine fact observation and Storage lease/grant reads only
  as observations. Never reinterpret a fact snapshot, empty mismatch list,
  policy `Allow`, grant seal, or lease guard as execution authority.
- Verify whether TaskEngine and Storage can be observed and revalidated inside
  one `BEGIN IMMEDIATE` transaction without adding a public unchecked seam.
  If crate privacy prevents it, record `C1_TRANSACTION_BOUNDARY_BLOCKED` and
  propose the smallest typed, crate-owned seam for an Owner decision.
- Exercise bounded negative cases for worker incarnation, clock expiry/order,
  fake verifier replay/binding, grant status/integrity, policy freshness, and
  exact provider instance/readiness. No provider invocation or production
  authority token is in scope.
- Correct the Windows ARM64 loop-index documentation from the exact Actions
  jobs: PR #34 job `117407503289` index 9; PR #36 job `117460408311` index 0.

## Known integration constraints from source review

- `TaskEngine::observe_l1_facts` owns its `Store::transact` internally; its
  transaction-held callback is private and compiled only for TaskEngine's own
  unit tests. Host cannot call it or join that transaction through public API.
- `Tx::acquire_lease` is public and accepts caller-selected owner/generation/time.
  Its guard is a storage fence, not Host admission.
- Public `ApprovalAuthenticationVerifier` and `AuthenticatedApprovalResponseV1::authenticate`
  allow downstream accept-all verifiers. They are not Host authentication.
- Provider health defaults to `Ready`; exact instance requires an `Arc` identity
  comparison held by the private harness.
- `probe_grant_full_set` is private to Storage's migration-0005 test module.

## Strict claims and prohibited changes

- Do not claim cross-crate transactional authorization unless one real SQLite
  transaction contains current TaskEngine checks and Storage lease/Grant checks.
- Do not add public production authority APIs, migrations, Intent/Permit,
  invocation, credentials, effects, Receipt, Retry, Memory, or canonical migration.
- Do not mark ADR-0046 accepted. Existing public APIs remain generic reachable
  APIs, not a resolved Host authority boundary.

## Verification commands

Run the requested Stable 1.99.1 and MSRV 1.85.0 fmt/check/clippy/workspace
tests, release Host check, external Cargo probe, documentation validator, and
`git diff --check`. Run the Host test-only harness with and without
`synthetic-test-hooks` as appropriate. Record unavailable toolchains or CI as
blocked/pending rather than success. Inspect exact-head GitHub Actions after
publishing any Draft PR.
