# P8B Candidate C test-only proof report

**Status:** bounded private test probe implemented; cross-crate atomic authority
remains unproven and blocked. This report does not ratify ADR-0046 or enable
production authorization, dispatch, migration, or external effects.

## Baseline and isolation

- Repository: `Yrika819/Serea`.
- PR #37 was Open/Draft at the time of inspection, with exact HEAD
  `5220f2e1493a136ef248e82a730e506254065936` and actual head branch
  `codex/p8b-stabilization-integration-candidate-c-2026-10-18`.
- The probe branch is based directly on that commit. It lives in a separate
  worktree at `/tmp/serea-p8b-candidate-c`.
- `main` and the original `/workspace/Serea` checkout are not edited.

## Implemented bounded probe

`crates/serea-host/src/candidate_c_test_only.rs` is included only under
`#[cfg(all(test, feature = "synthetic-test-hooks"))]` from the Host binary.
It creates a real SQLite-backed Store, installs a manifest and fake provider
Arc, creates one Task/Capability Step through TaskEngine, and acquires a real
storage LeaseGuard. It then calls the existing public
`TaskEngine::observe_l1_facts` read path and checks that a real reclaimed lease
generation fences the stale guard. The fake provider panics if invoked and its
counter is checked at zero. Full ordinary-table SQLite row fingerprints around
the observation show that the observation itself made no durable writes.

The same private test module exercises a non-Clone synthetic Host admission
record, exact `Arc::ptr_eq` matching with same-ID replacement and generation
mismatch negatives, explicit test-health state, expiry equality (`now ==
expires_at` rejects), and a private fake Owner challenge model bound to session,
request digest, selected Steps, time window, and one-use nonce. These are
bounded test fixtures; they are not connected to a production issuer,
trusted clock, policy decision, Grant authority, or dispatch API.

## Transaction boundary finding

`TaskEngine::observe_l1_facts` calls `Store::transact` internally and returns
`L1FactObservation` after that transaction. Its transaction-held callback is
private to TaskEngine's own unit tests. Host cannot compose its private inputs
inside that callback, nor join a second Storage transaction to the observation.
`Tx::acquire_lease` can run in a real SQLite transaction but accepts caller
owner, generation, and timestamps. Grant complete-set verification remains a
private helper in `serea-storage`'s migration-0005 test module. Therefore the
Host probe uses separate transactions and **does not prove transaction-atomic
TaskEngine + Storage authorization**. No public seam was added.

Minimum seam for an Owner decision: choose a crate ownership boundary for a
single typed, read-only revalidation operation that loads Task/Step/Plan,
current Policy/Overlay/Registry facts, Lease fence, and every Grant member
inside one Storage-owned `BEGIN IMMEDIATE`, and returns a non-authorizing
observation to a Host-private coordinator. If cross-crate logic cannot be
placed without an unchecked public callback or caller-supplied proof, create a
separate explicitly owned coordination crate and review its public consumer
threat model before any authority semantics. The Host must sample its test or
trusted clock after writer reservation through a crate-owned typed clock seam.
Nonce consumption and response/Grant persistence are not part of this probe.

## Track status

| Track | Status | Evidence and limit |
|---|---|---|
| C1 Host-private composition | `PARTIAL` | Host-private test module calls real TaskEngine observation and real SQLite. Separate transaction boundary prevents atomic proof; no Candidate C authority result is issued. |
| C2 Worker and Lease | `PARTIAL` | Session/incarnation model rejects changed values; real LeaseGuard reclaim advances generation and fences the old guard. Public raw lease creation is still reachable and is not linked to Host admission. |
| C3 clock | `PARTIAL` | Test-owned `EpochMillis` equality semantics reject expiry. Sampling after `BEGIN IMMEDIATE`, lock wait, rollback, restart, and freshness continuity are not connected or proven. |
| C4 Owner fake | `PARTIAL` | Private model rejects session/digest/Step/time mismatches and nonce replay. Existing Policy tests include `approval::tests::authenticated_port_refuses_stale_session_before_lifecycle_mutation`, but the Host model does not use or harden the public `ApprovalAuthenticationVerifier`; no same-Tx nonce persistence is proven. |
| C5 Grant and Policy transaction | `BLOCKED` | The fixture has no Grant and no current Policy revision; full-member probe is private in Storage tests. Existing storage coverage includes `migration_0005_tests::test_only_full_grant_set_probe_checks_subset_and_rejects_corruption` and `grant_fact_projection_does_not_reject_a_corrupted_sealed_member_set`; these do not join Host checks. No Grant corruption/status or Policy TOCTOU claim is made by this probe. |
| C6 Provider instance/readiness | `PARTIAL` | Exact fake Arc identity, generation, and explicit test-health values are checked; replacement fails and invoke count remains zero. Health is not obtained through a trusted readiness protocol and there are no precommit/preinvoke stages. |

## Adversarial matrix and current evidence

| Case | Expected | Observed result | Durable writes / execution |
|---|---|---|---|
| Normal proof fixture | Current real durable pins can be read for proof-only observation | `observe_l1_facts` returns no pin mismatch for the created Task/Step/Registry/Descriptor/Lease | Full ordinary-table fingerprint unchanged across observation; Provider invoke count 0 |
| Worker session/incarnation mismatch | Reject | Private admission model rejects another session or incarnation | No authorization object exists |
| Reclaimed lease / old generation | Reject stale guard | Real SQLite reclaim increments generation; old guard observation errors; current guard observes cleanly | No provider invocation |
| Raw lease forged by public consumer | Do not treat as Host admission | External Cargo probe confirms public API composition still compiles; no Host-authority inference is made | Public generic Storage API remains reachable |
| Expiry equality | Reject `now == expires_at` | Private test clock rejects equality | No clock is sampled inside Storage transaction |
| Fake Owner changed session/digest/Steps or replay | Reject | Private synthetic verifier rejects each mismatch and second nonce use | Fake in-memory state only; nonce atomicity with SQLite is unproven |
| Grant corruption/status/member mismatch | Reject | Not exercised by Host fixture; private Storage test helper is not callable here | No Grant is consumed or written |
| Old Policy / Task cancel / Overlay TOCTOU | Reject or mark unproven | TaskEngine observation reloads current selectors, but Policy revision is absent in the fixture and checks are outside a later Host action | No same-Tx revalidation claim |
| Provider same-ID replacement / generation / unready | Reject | Exact `Arc` replacement, changed generation, and explicit unready test state fail the private pin model | Fake provider `invoke` count stays zero; no precommit/preinvoke protocol |
| No-write | No unintended writes | SQLite table/value fingerprint around the TaskEngine observation is equal | Pass for that observation only |
| Production exclusion | No new synthetic authority entrypoint in normal Host | Module is `cfg(test)` plus test-hook feature; release check/string inspection pending | No public API was added |

## Public API surface still reachable

- `Tx::acquire_lease` accepts caller-selected owner, expected generation, and
  caller timestamps; its `LeaseGuard` fences a Storage row but proves no Host
  admission.
- `ApprovalResponseCandidateV1::new`, public
  `ApprovalAuthenticationVerifier`, and
  `AuthenticatedApprovalResponseV1::authenticate` still permit a downstream
  accept-all verifier to mint the wrapper.
- Policy facts/results remain caller-composable; they are not Host authority.
- `CapabilityProvider` remains publicly implementable and its default
  `health()` is `Ready`; an arbitrary in-process holder can call its own
  provider. Provider ID is not exact instance identity.
- The external Cargo probe compiles two positive API compositions and confirms
  private fields/Tx internals/absent permit types/private Grant helper/candidate
  session getter are inaccessible. These checks do not close the reachable
  generic APIs above.

## Windows ARM64 evidence

- PR #34 Stable ARM64 job `117407503289`: failure loop index is **9**.
- PR #36 Stable ARM64 job `117460408311`: failure loop index is **0**.
- Both records remain `0xC0000142` / `STATUS_DLL_INIT_FAILED`, child stdout and
  stderr 0 bytes. `ROOT_CAUSE_UNCONFIRMED` remains in force.
- Diagnosis and campaign documents were corrected and include Actions job
  references. The Actions log download endpoint returned HTTP Forbidden in
  this execution, so the index correction follows the supplied Owner-recorded
  job evidence and the existing diagnosis metadata; it was not independently
  re-fetched from raw log bytes here.
- The PowerShell diagnostic script is statically reviewed only. This Linux
  environment has no Windows ARM64 or `pwsh`; script execution is
  `SCRIPT_REVIEWED_NOT_WINDOWS_VERIFIED`. Static review notes argument quoting
  with `Start-Process -ArgumentList` and temp-file cleanup on early process
  start exceptions as follow-up hardening points. No DLL faulting module or
  resource-pressure cause is established.

## Verification and CI

The isolated filesystem had a 4.9 GiB `/tmp` limit; the first full test and
Clippy attempts ran out of space. The complete Stable workspace suite passed on
retry with reduced test debug artifacts and Cargo incremental output disabled;
Rust test logic and selected targets were unchanged. Remaining checks are marked
by their exact result below.

| Check | Result |
|---|---|
| Stable 1.99.1 Host Candidate C focused tests | PASS: 3 tests (including real lease reclaim) |
| MSRV 1.85.0 Host Candidate C tests | PASS: all-target/all-feature workspace suite ran all 3 Host Candidate C tests |
| Stable workspace check, all targets/features | PASS: `cargo +stable check --workspace --all-targets --all-features --locked` |
| MSRV workspace check, all targets/features | PASS: `cargo +1.85.0 check --workspace --all-targets --all-features --locked` |
| Stable workspace test, all targets/features | PASS: `cargo +stable test --workspace --all-targets --all-features --locked` with `CARGO_PROFILE_TEST_DEBUG=0 CARGO_INCREMENTAL=0`; 2 existing manual resource measurements remain ignored |
| MSRV workspace test, all targets/features | PASS: `cargo +1.85.0 test --workspace --all-targets --all-features --locked` with `CARGO_PROFILE_TEST_DEBUG=0 CARGO_INCREMENTAL=0`; 2 existing manual resource measurements remain ignored |
| Stable workspace Clippy | PASS: `cargo +stable clippy --workspace --all-targets --all-features --locked -- -D warnings` |
| MSRV workspace Clippy | PASS: `cargo +1.85.0 clippy --workspace --all-targets --all-features --locked -- -D warnings` |
| Stable release Host no-default-features | PASS: release check and build; `doctor` reports `external_providers=disabled` and `capability_invocation=disabled`; `strings` found no Candidate C test-only symbols |
| External Cargo authority probe | PASS: both toolchain positive compositions compile; expected private-surface negative probes match diagnostics |
| `validate_docs.py docs` and `git diff --check` | PASS (167 Markdown files scanned) |
| Stable and MSRV fmt checks | PASS: `cargo +stable fmt --all -- --check` and `cargo +1.85.0 fmt --all -- --check` |
| New Draft PR Actions at final HEAD | pending |

## Owner decision and next steps

1. Decide the crate that owns the typed same-transaction revalidation seam and
   whether its read-only observation may be public or must live behind a
   separately reviewed internal coordination crate.
2. Decide the actual Host Worker admission source and restart/incarnation rule;
   decide the trusted clock source and post-writer-reservation sample contract.
3. Decide the real Owner identity source and nonce persistence/atomicity owner;
   public generic verifier wrappers remain untrusted.
4. Decide Grant full-set ownership/status policy and exact Provider health /
   registry-overlay generation invalidation contract.

Recommended next sequence: (PR A) implement only the Owner-approved typed
revalidation seam and test-only cross-crate SQLite barrier proof; (PR B) bind
Host-private worker, clock, fake Owner, Grant, and exact Provider checks to that
same transaction while keeping `invoke` unreachable; (PR C) request a separate
Owner review of any production contract and L1-B gate evidence. L2 Intent,
L3 Permit, canonical migration, credentials, Memory, Receipt, Retry, provider
effects, and production authorization remain withheld. ADR-0046 stays
`PROPOSED / NOT RATIFIED`.
