# Serea P8B overnight trust-boundary report — 2026-10-18

## Executive summary

**Status: L1-B feasibility remains PARTIAL / NOT RATIFIED; production dispatch is disabled.** The only code change is in the private Storage test module: a test-only Request-action corruption case and an independent SQLite lock witness were added. The full Grant probe remains disconnected from runtime authorization. ADR-0046 remains PROPOSED / NOT RATIFIED.

PR #35 was verified at its expected HEAD and the new branch was based directly on it. Its five required Actions workflows were successful on that exact baseline SHA. No change was made to PR #35, PR #34, `main`, or the source checkout. A single stacked Draft PR is intended only after final local validation; record the URL/HEAD below if publication succeeds.

## 1. Completion and repository state

- Repository: `Yrika819/Serea`.
- Baseline PR #35 HEAD: `b9a0ef070bbec396f107ccb2b5a43925b0e22ed0`; PR #35 is open Draft, base `codex/p8b-l1b-trust-root-2026-10-17`, head `codex/p8b-l1b-boundary-repair-2026-10-17`.
- PR #34 HEAD: `f3804a643e6334bc4f1dddfea1a0c096c1fd7a13`, ancestor of PR #35. `main`: `e0acb0254820c381dd97828824bef86a1db5176c`.
- No PR #36 was present at initial audit.
- Isolated worktree: `/workspace/Serea-overnight`; branch `codex/p8b-overnight-2026-10-18`, based at PR #35 HEAD. Source checkout `/workspace/Serea` stayed clean and on branch `work` at the start.
- Final branch HEAD / Draft PR / CI summary: **fill after final commit and Actions completion**.
- Changed files: `crates/serea-storage/src/migration_0005_tests.rs`; this report; `docs/plans/P8B-overnight-campaign-2026-10-18.md`.
- Production code, public APIs, migrations, schema catalog, ADR-0046 status, PR #35, PR #34, and `main` were not changed.

### Baseline Actions on exact PR #35 SHA

| Workflow | Run ID | Result |
|---|---:|---|
| Fast CI | 39044878214 | success |
| Full CI | 39044878234 | success |
| Native platform validation | 39044878224 | success; 12/12 jobs |
| Cross-architecture SQLite portability | 39044878258 | success |
| Git hooks Windows compatibility | 39044878218 | success |

These are baseline checks for `b9a0ef0…`, not checks for this branch. Exact final-HEAD Actions results are recorded separately below.

## 2. Track status

| Track | Status | Code path and evidence | Bypass / unproven boundary |
|---|---|---|---|
| G — Grant full-set integrity | `PROVEN_TEST_ONLY` after full local matrix confirms | `migration_0005_tests.rs::probe_grant_full_set`; full Request digest and approved-subset Grant digest are recomputed independently; all Grant members, order, Request links, selected Step, task/plan/capability/generation/descriptor pins, seal cardinality, use rows/counters, and integrity fixtures are inspected. Added Request action mutation and direct lock witness. | Test-only validator. Not invoked by Grant use or execution authorization. DB-file and schema attacker can rewrite rows and stored digests; no cryptographic authenticity. Seal records immutability fence, not signature. |
| W — Worker admission × Lease | `PARTIAL` | `Tx::acquire_lease`, LeaseGuard creation/reclaim/generation/origin and TaskEngine observer behavior reviewed; `LeaseGuard` private/non-Clone/origin-bound. | Public `Tx::acquire_lease` accepts caller-created `LeaseOwner` and caller time; `W_BYPASS_STILL_OPEN`. A valid guard proves a durable lease row, not Host-admitted Worker identity. No new admission architecture chosen. |
| T — Clock × writer reservation | `PARTIAL` | `Store::transact` opens `BEGIN IMMEDIATE` before closure; existing file-backed transaction clock test and strengthened lock witness establish closure ordering after lock grant. Inclusive expiry tests are present. | Public lease/approval APIs continue to accept caller `EpochMillis`; no trusted Host clock source or anti-rollback guarantee. `T_CLOCK_SOURCE_UNTRUSTED_OR_PARTIAL`. |
| O — Owner authentication | `BLOCKED_OWNER_DECISION` | Public candidate constructor, public `ApprovalAuthenticationVerifier`, `authenticate(candidate, verifier)`, private Candidate internals and absent session getter inspected. Existing external positive probe accepts arbitrary verifier; negative probe shows Host verifier cannot read session claim. | Arbitrary downstream caller can mint the authenticated wrapper. Wrapper type is not Owner identity. No OS/device issuer, credentials or production auth enabled. `O_OWNER_ISSUER_NOT_ESTABLISHED`. |
| P — Provider instance/readiness | `PARTIAL` | Real Capability registry tests show two separate `Arc` instances may share ProviderId; trait implementations are externally possible; `health()` defaults Ready. | ProviderId/descriptor alone does not establish exact Host-owned instance, lifecycle epoch, registry/overlay pin or fresh health. No Host live dispatch or invoke. `P_HOST_INSTANCE_BINDING_UNPROVEN`. |
| Native Windows ARM64 | `NOT_TESTED` on this Linux execution host; historical root cause `UNCONFIRMED` | Historical PR #34 failure and PR #35 success compared below. | No Windows runner telemetry or direct rerun available from local host. Successful later run does not identify the failure cause. |

## 3. Grant integrity, SQLite contention and mutation evidence

### Digest and member contract

The probe follows the implementation: `approval_requests.action_set_digest` covers **all requested actions**. `approval_grants.action_set_digest` covers the **approved subset**, as constructed in `approval.rs::apply_response`. The two digests are independently recomputed and compared to their own stored values; they are not equated. The schema's current maximum of eight is only borrowed for the test helper, not ratified as a permanent Grant protocol maximum.

The probe verifies full Request row count/order/unique Step and action scope digest; full Request digest; grant/request identity and pinned task/plan/capability/version/generation/descriptor facts; Grant declared count; every Grant member's position, approval ID, uniqueness, arguments/scope digest and inclusion in the Request; every member's current Step facts and capability binding; selected Step membership; exact one seal row; use rows/count/status consistency; and recomputed subset digest. Diagnostic row integrity is separate from eligibility: REVOKED/EXPIRED are structurally recognized statuses but cannot be treated as active execution grants.

Existing hostile fixtures cover missing Grant member, position gap/reordering, member arguments/scope digest corruption, missing seal, mismatched selected Step, invalid count/duplicate constrained by SQLite, rollback/reopen and `verify_integrity()`. This campaign adds Request action `arguments_digest` corruption with only its immutable-row trigger removed in an isolated disposable fixture. The probe rejects it because the Request digest no longer matches. These fixtures intentionally modify SQLite only in isolated tests; no normal Store API can author these malformed rows.

### Contention proof

`full_set_probe_is_serialized_with_an_independent_sqlite_writer` now opens a third connection to the same file-backed fixture, sets `busy_timeout(Duration::ZERO)`, and while the first typed Store transaction holds `BEGIN IMMEDIATE`, attempts an independent `BEGIN IMMEDIATE`. Only `DatabaseBusy` or `DatabaseLocked` counts as direct lock evidence. The prior channel message remains only a scheduler handshake; it is not interpreted as proof that the competitor already reached SQLite. The typed second writer is then observed blocked and succeeds after the barrier is released. Deadlock timeout remains a guard, not lock evidence.

### Read-only and RED/GREEN

The existing fixture compares Grant-use/Event/Request counts around the probe and tests rollback/reopen/integrity; this counter assertion alone does not establish all-table byte-for-byte invariance. The probe implementation is SELECT-only, and no runtime use path calls it.

The real predicate `member_arguments != action.arguments_digest.as_str()` was temporarily mutated to `false` in the isolated branch. The full-set corruption test failed at the `digest` hostile fixture, demonstrating that it detects this actual validation-logic mutation. The original predicate was restored and the same test passed GREEN. Scratch mutation was not committed.

Status when all final commands pass: `G_FULL_SET_TEST_ONLY_PROVEN`; runtime Grant authorization unchanged.

## 4. Worker, Lease, Clock and transaction order

The real Storage lease is a generation-fenced durable row. LeaseGuard's private fields, non-Clone shape and transaction-origin checks prevent constructing a valid guard from public fields or laundering a rolled-back acquisition. They do not prove which process/user called public `Tx::acquire_lease`; external Rust consumers can call it with a caller-selected `LeaseOwner` and time. Removing or narrowing that surface is a compatibility/API ownership decision and was not attempted.

`Store::transact` reserves SQLite's writer lock before calling its synchronous closure. Existing test `transaction_closure_clock_sample_occurs_only_after_writer_reservation` records one caller-owned sample in the transaction closure after reservation; the new zero-timeout witness makes SQLite lock ownership direct. This proves sampling location relative to lock acquisition only. It does not authenticate the clock.

Expiry code has inclusive-boundary coverage (`now >= expires_at`). Caller-supplied `EpochMillis` remains in acquisition, renewal, release, approval and related operations. No model of trusted monotonic time, wall-clock correction, suspend, process restart, or persistent high-water mark was promoted to production. Suggested Owner contract: use a monotonic source for in-process elapsed/freshness, wall time only for externally defined expiry where needed, fail closed when reboot/suspend makes freshness uncertain, and explicitly decide tolerated rollback/forward-jump thresholds and loss of high-water evidence.

## 5. Owner authentication and Provider

An external consumer can implement `ApprovalAuthenticationVerifier` with unconditional success and call the public `authenticate` method to obtain `AuthenticatedApprovalResponseV1`. The wrapper's private fields constrain direct construction but do not distinguish a trusted verifier. Candidate fields include approval ID/decision/selected Steps/session claims, but the current verifier cannot read the private session claim. Making getters public alone would improve inspectability, not verifier provenance. A test-only Host issuer could bind approval ID, decision, exact selected Steps, nonce, expiry, session freshness, replay and revocation, but actual OS/device authentication needs a separately approved source and contract.

The capability registry test `replacement_registry_can_hold_a_distinct_instance_with_the_same_provider_id` proves identity by ProviderId can alias distinct `Arc` instances. `CapabilityProvider` is publicly implementable; default `health() == Ready` is not fresh evidence. A Host-private handle would need to bind exact instance identity/epoch, Registry generation, descriptor digest, Overlay revision, Host lifecycle epoch and an explicit timely health response, with independent precommit and preinvoke checks. Only pure/test logic was reviewed; no invocation path was added.

## 6. Windows ARM64 `0xC0000142`

Historical PR #34 run `39036558000`, Stable Windows ARM64 job `117407503289`, failed during “Test all workspace features.” Its output reported the ninth repeated `model-smoke` child exited with `0xC0000142 / STATUS_DLL_INIT_FAILED` and zero stdout/stderr, indicating failure before the Rust child handler wrote output. Other jobs in that run passed, including Rust 1.85.0 Windows ARM64 (`117407503223`). Parallel all-feature tests and child-process starts make loader/resource pressure a hypothesis only; no runner resource telemetry or DLL-load trace establishes it.

PR #35 exact HEAD's Native run `39044878224` passed all 12 Stable/MSRV × Linux/macOS/Windows × x86_64/ARM64 jobs, including Stable Windows ARM64 `117431513197` and Rust 1.85.0 Windows ARM64 `117431513238`. This non-recurrence does not resolve the root cause. No sleep/retry/skip/CI exclusion or dependency substitution was introduced. Final branch Native results must be filled below. Preserve `NATIVE_DLL_INIT_ROOT_CAUSE_UNCONFIRMED` unless direct reproduction and loader evidence are obtained.

## 7. Tests, probes and CI

### Local commands

| Command | Result |
|---|---|
| `cargo +stable test -p serea-storage --lib full_set_probe_is_serialized_with_an_independent_sqlite_writer --locked` | PASS, 1 test; direct lock witness and typed writer release path |
| `cargo +stable test -p serea-storage --lib test_only_full_grant_set_probe_checks_subset_and_rejects_corruption --locked` | PASS, 1 test after Request-action hostile fixture |
| Same Grant corruption test with real arguments predicate temporarily disabled | Expected RED: failed at `digest` hostile fixture; source restored; restored source then passed GREEN |
| `cargo +stable fmt --all -- --check` | PASS |
| `cargo +stable test --workspace --all-targets --all-features --locked` | PASS on isolated retry; 647 Storage unit tests passed, all other reported test binaries completed without failure. First attempt hit 4.9 GiB `/tmp` quota during parallel linking; retry used workspace overlay and `CARGO_BUILD_JOBS=2`. Two manual resource benchmarks are marked ignored by the project. |
| `cargo +1.85.0 test --workspace --all-targets --all-features --locked` | PASS; 647 Storage unit tests passed, all other reported test binaries completed without failure. Two manual resource benchmarks are marked ignored by the project. |
| Stable workspace `check` and `clippy -- -D warnings` | PASS |
| Rust 1.85.0 workspace `check` and `clippy -- -D warnings` | PASS |
| `cargo +stable check -p serea-host --release --no-default-features --locked --offline` | PASS |
| `cargo +stable metadata --no-deps --format-version 1 --offline` | PASS |
| `python3 tools/validate_docs.py docs` | PASS; 159 Markdown files scanned |
| `python3 tools/p8b_authority_cargo_probe/run.py` | PASS; 2 external positive compiles, 7 expected negative compiles matched their diagnostics. Includes arbitrary verifier positive, raw Lease API reachability, private Candidate/Tx restrictions, no public CommitProof/Permit/probe, and no Candidate session getter. |
| Git identity, preflight and hook unit suites | PASS; 6 identity tests, 28 preflight tests, 4 launcher/hook tests; installed repo-local pre-commit/pre-push hooks and verified Codex noreply identity |
| `git diff --check` | PASS before report completion; repeat after final report edit |

### Final exact HEAD Actions

Record every workflow and exact run ID for the final pushed HEAD. Do not copy baseline SHA outcomes into this section.

| Workflow | Run ID | Head SHA | Conclusion |
|---|---:|---|---|
| Fast CI | pending | pending | pending |
| Full CI | pending | pending | pending |
| Native platform validation (12 jobs) | pending | pending | pending |
| Cross-architecture SQLite portability | pending | pending | pending |
| Git hooks Windows compatibility | pending | pending | pending |

No failing code-level CI is considered fixed without a reproducer, log-based cause, bounded correction and successful final-SHA rerun. Keep queued/runner-unavailable jobs as pending.

## 8. Owner decisions (maximum five)

1. **Authority ownership topology:** approve a bounded proof of ADR-0046 candidate C (Host binary private coordinator, TaskEngine revalidation, Storage atomic mechanics), request revised A/B/C, or decline. Name the owner of atomic authorization and private one-shot Permit. Approval would authorize only the next proof, not production activation.
2. **Worker admission and Storage API:** decide whether raw public `Tx::acquire_lease` is narrowed in a compatibility-breaking major boundary, or Storage remains generic and Host owns a separately unforgeable admission capability. Specify attacker/process boundary and external consumer expectations.
3. **Clock and expiry:** select monotonic vs wall-time roles, rollback/forward-jump tolerances, inclusive expiry, and fail-closed behavior after suspend/reboot or loss of high-water data. Name the trusted clock owner and where it is sampled.
4. **Owner authentication:** name the Host-owned issuer, OS/device trust source, challenge fields (approval/decision/selected Steps/nonce/expiry), freshness, replay and revocation semantics. Do not approve arbitrary public verifier as production identity.
5. **Grant and Provider commitments:** decide permanent Grant set cardinality/digest version/authenticity and seal semantics, plus exact Host Provider instance/readiness lifecycle binding (freshness window, Registry/Overlay revision and shutdown behavior). Keep them separate if a single decision would over-bundle contracts.

## 9. Next step and hard gates

After Owner decisions, the smallest next implementation proof is the selected Host-private admission/authentication and Provider handle prototype with external Cargo positive/negative probes, while Storage continues to expose only a safe typed atomic seam. Until then, maintain `L1B_BOUNDARY_PARTIAL / NOT_RATIFIED`, `L2_AUTHORIZED_ATOMIC_COMMIT_NOT_PROVEN`, `L3_HOST_PRIVATE_PERMIT_NOT_PROVEN`, and `PRODUCTION_DISPATCH_DISABLED`.

L2 Intent writer, L3 Permit factory, Production Provider, credentials, Memory, Receipt, retry, canonical migration, external effect and production authorization remain prohibited by the campaign boundary.
