# P8B Candidate C test-only proof report

**Status:** test-only, non-authorizing single-transaction observation is implemented behind an explicit non-default Cargo feature. This is not ADR-0046 ratification. Production authorization, dispatch, Provider invocation, Intent, Permit, credentials, canonical migrations, and external effects remain out of scope.

## Repository and identity recovery

- Repository: `Yrika819/Serea`; baseline `5220f2e1493a136ef248e82a730e506254065936`, the verified PR #37 head branch `codex/p8b-stabilization-integration-candidate-c-2026-10-18`.
- PR #38 source: `2bc39bdc1d77b6fba48bfb6cd292807dfec365b5`. Its single commit has a non-noreply author and committer. Fast CI run `39106778311` was recorded as failed; Full CI run `39106778304`, job `117614225399`, explicitly failed in `Commit identity guard` and printed that SHA. This is an identity-guard failure, not a Rust test failure. Native run `39106778357` subsequently completed successfully for all 12 Stable/MSRV × Linux/macOS/Windows x86_64/ARM64 targets. SQLite run `39106778279` and Hooks run `39106778296` also completed successfully. Fast and Full failed only at their identity-guard steps as detailed above.
- The PR #38 patch was generated from the exact PR #37 and PR #38 heads and applied as working-tree content in a new worktree `/tmp/Serea-p8b-c1`. The bad source commit is not an ancestor of the new branch. No existing PR or remote branch was changed.
- In this checkout, `git log --all` covers 1005 commits reachable from its refs and the identity script passes. The old PR #38 SHA is present only as an unreferenced fetched object, not in `show-ref` or `log --all`. However, repo workflows pin `actions/checkout` v7.0.1 with `fetch-depth: 0`. Its `getRefSpecForAllHistory` explicitly fetches `+refs/heads/*:refs/remotes/origin/*`; thus the existing PR #38 source branch and bad SHA will be visible to `git log --all` on a new PR run. This confirms `IDENTITY_GUARD_GLOBAL_REF_BLOCKED`. Do not bypass the guard or rewrite PR #38 without separate Owner authorization.

## Implementation

- The PR #38 Host probe, feature/dependency changes, proof documents, and campaign-note corrections are retained as patch content.
- Storage's existing full Grant request/member integrity validator was moved out of the migration test module into `candidate_c_test_only.rs`; existing corruption/subset tests now call that same implementation.
- A non-default `candidate-c-c1-test-only` feature is wired from Host through TaskEngine to Storage. Under this feature, TaskEngine reuses its existing typed Task/Step/Registry/Descriptor/Overlay/Policy/Lease/Grant reader inside the one `Store::transact` that Storage opens with `BEGIN IMMEDIATE`. It also validates all Grant request and subset members through a typed Storage method in that same transaction. An injected `Clock` is sampled after these reads and checks, before Storage commits.
- The result remains `L1FactObservation` plus a sample timestamp; neither is an authorization result. No Tx, rusqlite transaction, SQL executor, authorization token, Permit, or arbitrary transaction callback crosses into Host. The feature is not default. The production release no-default build must remain free of this code; explicit feature builds are experimental and are not production configurations.
- No schema/migration, normal lifecycle writer, Provider invocation path, ADR status, or production authorization API was changed.

## Test evidence and boundaries

- Host's file-backed SQLite test uses the real Host, TaskEngine, and Storage crates. A separate zero-timeout connection attempts its own `BEGIN IMMEDIATE` from the injected clock; it observes `DatabaseBusy`/`DatabaseLocked`, proving the clock runs while the Storage transaction holds the writer reservation. A new writer succeeds after observation returns.
- Before/after fingerprints include every ordinary SQLite table and row. They are equal for the integrated TaskEngine observation; the Fake Provider's invoke counter remains zero. A panic-on-invoke Fake Provider remains installed in the fixture.
- Existing Storage full-set tests now execute the extracted validator and cover full request digest versus approved subset digest, subset membership, corruption (member fields and missing seal), revoked status, and a separate SQLite writer competing with the validator. The validator is diagnostic structure checking only; statuses and expiry are not authorization decisions.
- RED/GREEN mutation checks: (1) moving clock sampling before the observer transaction made the independent writer test fail at `clock must run under BEGIN IMMEDIATE`; restoring sampling inside the transaction passed. (2) disabling the Grant seal-count predicate made the corruption test fail on the `seal` case; restoring the predicate passed. Mutations were not retained.
- Host test fixtures additionally cover fake Owner session/digest/selected-Step binding and nonce replay, stale real LeaseGuard after reclaim, same-ID different Provider `Arc`, Provider generation and readiness fixture mismatches, and zero invocation. These fixtures do not prove OS Worker identity, Owner authentication, production readiness, or post-observation effect safety.
- Observation is point-in-time only. It does not close the later TOCTOU window. The Host integrated fixture does not create a Grant; the full-member validator is exercised in Storage's real SQLite suite and is invoked by the TaskEngine integrated path when a Grant is supplied. A full Host + Grant fixture, exact expiry-equality test for a Grant inside the integrated path, policy/overlay/task cancellation race matrix, and corrupted grant fixture connected to Host remain incomplete.

## Boundary classification

| Area | Status | Limit |
|---|---|---|
| C1 Host + TaskEngine + Storage transaction | `PARTIAL` | Real integrated Task/Step/Policy/Lease observation, SQLite writer lock, clock order and no-write fingerprint pass. Integrated Host + Grant member fixture is not yet present; no authority or later commit/invoke proof. |
| C2 Worker and Lease | `PARTIAL` | Fake session/incarnation and real Lease reclaim are covered. Public `Tx::acquire_lease` still accepts caller-selected facts; no OS identity proof. |
| C3 Clock | `PARTIAL` | Injected time is sampled under the lock; fake Owner expiry equality is covered. Trusted production clock, restart monotonicity, grant expiry equality inside a Host+Grant case and fail-closed clock errors are not proven. |
| C4 Owner | `PARTIAL` | Fake verifier binding/replay only. Public verifier composition remains caller-controlled; nonce is not atomically persisted. |
| C5 Grant and Policy | `PARTIAL` | Full-set structure is checked with real Storage tests in a writer transaction and is wired into the integrated TaskEngine path. Host + Grant adversarial fixture, current Policy race matrix, and status/expiry execution eligibility are not proven. |
| C6 Provider | `PARTIAL` | Fake exact Arc/readiness pins and zero invokes are covered. No production health authority or invocation gate is added. |

## Verification and CI

Local verification completed on this patch:

- Stable Rust 1.99.1: workspace `cargo test --workspace --all-targets --all-features --locked` passed before the final `Tx::ensure_active()` guard was added. The final code passed Stable all-target/all-feature workspace `cargo check`, workspace Clippy with `-D warnings`, and focused Host C1 tests (4 passed, including the transaction-lock/no-write test). The guard itself was included in the full MSRV run below.
- Rust 1.85.0: workspace all-target/all-feature `cargo test --workspace --all-targets --all-features --locked` passed on final source: Storage 647 tests, TaskEngine's six observer barrier tests, Host and integration suites; two manual resource measurements were ignored by the existing suite. Workspace all-target/all-feature `cargo check` and Clippy with `-D warnings` passed.
- The external Cargo consumer probe passed with Stable and Rust 1.85.0: default feature surface does not expose the seam; enabling `candidate-c-c1-test-only` compiles its typed observation call. Existing negative API composition checks passed on both toolchains.
- Stable release `serea-host` built with `--no-default-features --locked`; `doctor` reported provider and capability invocation disabled. The binary had no Candidate C seam strings.
- `python3 tools/validate_docs.py docs` scanned 167 Markdown files successfully; `python3 tests/workspace_smoke.py`, local commit identity guard, and `git diff --check` passed. Hook/preflight and Python suites are recorded in the task completion report.

The attempted additional Stable focused Storage rerun was stopped by the 4.9 GiB temporary filesystem reaching its space limit while rebuilding a second debug profile; no test failure occurred. The same Storage corruption and independent-writer cases passed in the completed final-source MSRV workspace suite. Windows ARM64 diagnostic script execution is unavailable on this Linux host; only static inspection is possible. Earlier records supplied for PR #34 and #36 retain the correct indices 9 and 0 respectively, both `0xC0000142` with zero child output; no root cause is inferred.

## Owner decisions and remaining scope

No Owner decision is needed to keep this experiment test-only. Before any production use, an Owner must separately decide the Worker identity source, trusted clock, real Owner verifier and nonce persistence owner, Grant status/expiry semantics, Policy/overlay freshness, and Provider readiness/invalidation contract. ADR-0046 remains proposed and unratified.
