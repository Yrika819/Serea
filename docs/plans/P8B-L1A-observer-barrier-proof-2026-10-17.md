# P8B L1-A observer barrier closure plan

**Status: `L1A_OBSERVER_BARRIER_PARTIAL`; `L1B_DECISION_PACKAGE_COMPLETE`; `NATIVE_ROOT_CAUSE_UNCONFIRMED`.** Branch starts at PR #31 HEAD `1adb8e2662e80134d055cbef54a037ad1e076538`; PR #31 base is `p8b/authority-architecture-proof-2026-10-17` at `bb4c4d2b8a3bf015d735ab0ad519523f6b961299`. The worktree is `/workspace/Serea-p8b-worktree`; the original `/workspace/Serea` checkout remains on `work` at main HEAD `e0acb0254820c381dd97828824bef86a1db5176c` and was not edited.

## Scope and gates

This work targets only evidence for the existing read-only `TaskEngine::observe_l1_facts`, the L1-B Owner decision package, the Native Windows ARM64 failed child-process diagnostics, and read-only PR-stack auditing. No L2 Intent writer, L3 Permit, Provider invocation, Credentials, Production Memory, Receipt, Retry, canonical migration, external effect, or authorization ratification is in scope. ADR-0046 remains Proposed.

The production method delegates to a private transaction helper. A `#[cfg(test)]` private callback seam now runs only after all L1 fact reads and comparisons, while the `Store::transact` closure still owns its `BEGIN IMMEDIATE`. The public method supplies a no-op callback; no test hook is exported. The seam itself is not race evidence until exercised by crate unit tests with separate file-backed Store handles.

## Baseline audit

- Live GitHub PR snapshot agrees with the mandate: #31 Open/Draft, head `1adb8e2…`, base PR #29 branch at `bb4c4d2…`; #30 Open/Draft head `ba16eaa…`, same base.
- `git merge-base --is-ancestor refs/remotes/origin/pr/30/head refs/remotes/origin/pr/31/head` succeeds. #31 contains #30's four commits and adds two commits (the 5ceb23b feature and 1adb8e2 evidence); the tree delta `#30..#31` is 12 files, 1052 insertions, 0 deletions. The full `#29..#31` tree delta is 16 files, 1296 insertions, 12 deletions. Retargeting #31 to #30 is only simulated; no existing PR metadata or history is changed.
- A simulated stacked review therefore shows only the four L1-A feature/evidence commits after #30. Suggested merge order is #29, then #30, then #31 after independent review and branch-protection checks. Do not retarget or merge as part of this work.
- Native run `39019193381`, attempt 1 job `117357401817` is Stable Windows ARM64 on Windows 11 ARM64 runner image `20261013.183.4`. Cargo test fails at `crates/serea-host/tests/host_lifecycle.rs:659` in `model_accounting_is_bound_to_a_task_created_by_the_scheduler_task_engine`; the test's `Command::output().expect` succeeds, and the child reports only `stdout= stderr=`. The original assertion does not print `ExitStatus` or `status.code()`, so the child exit code and Windows exception status are unavailable from this log. Latest-attempt job `117362868585` (attempt 2) is success; a rerun pass does not establish a cause. The assertion now reports `ExitStatus`, decimal/hex `status.code()`, output byte counts, truncation markers, and up to 4096 bytes of each stream. Root cause remains unconfirmed.

## Observer race evidence to collect

The test fixture must use an on-disk SQLite database, an observer Store and an independent writer Store. A callback/channel barrier must prove that `observe_l1_facts` has finished its real readers/comparisons and remains inside its transaction. Do not use sleep alone as evidence. While held, use SQLite lock evidence plus the operation's actual typed writer path. Record the old observation, the writer result under contention, release/commit outcome, a new observation and close/reopen result. Finish with `verify_integrity`, compare Grant-use/Event and related write counters before/after, and use a scratch worktree with an independent `CARGO_TARGET_DIR` for assertion RED/GREEN.

| Writer category | Required operation | Status |
|---|---|---|
| Policy | Prepare and activate a valid revised policy through P6 API; old snapshot remains internally consistent; reopen sees new active revision. | **PASS** (crate unit test; Stable and 1.85.0) |
| Task cancellation | Typed lifecycle cancellation with TaskJournal/Event participants; old `READY` facts remain; reopen sees cancellation. | **PASS** (crate unit test; Stable and 1.85.0) |
| Grant revocation | Existing authenticated revocation path; no partial status/use update; reopen sees REVOKED. | **BLOCKED / NOT TESTED** (no Grant fixture in the observer unit fixture; Storage-only revocation tests are adjacent evidence and not counted) |
| Lease | Expired Lease reclaim through `Tx::acquire_lease`; old observer reports generation 1, post-commit generation is 2, and old guard fails on a new observation after reopen. | **PASS** (crate unit test; Stable and 1.85.0). Release itself is not exercised. |
| Registry Overlay | `Tx::set_capability_overlay` with expected revision; old snapshot is Enabled/revision 0; post-commit/reopen is Disabled/revision 1. | **PASS** (crate unit test; Stable and 1.85.0) |

All four tests use the observer-private post-read barrier, an observer Store and independent writer Store on a file-backed database. A third SQLite connection with zero busy timeout attempts `BEGIN IMMEDIATE` while the barrier is held and receives `SQLITE_BUSY`/`SQLITE_LOCKED`. Each actual typed writer call then runs to the Storage connection's 5-second busy timeout and returns an error; after observer release the same writer path is retried and commits. This combines direct lock-state evidence with the typed writer's failed attempt; the bounded timeout is not inferred from a short sleep. Tests verify the old observation, integrity after writer commit, and category-specific post-commit state. For Lease reclaim, the prior guard is also rejected after generation advances.

Stable and Rust 1.85.0: `cargo test -p serea-task-engine --lib observer_barrier_tests --locked --offline` **PASS**, 4 passed, 0 failed, each run about 10.6 seconds. The full workspace test suite on each toolchain also **PASS**, 1,478 passed, 0 failed, 2 ignored. Stable and MSRV `check --workspace --all-targets --all-features`, Clippy with `-D warnings`, release `serea-host --no-default-features`, fetch, fmt, metadata, external Cargo probe, docs validation, workspace smoke, Python smoke unittest, and `git diff --check` passed. The external Cargo probe confirms: read-only observer projections compile; public `PolicyEvaluation.decision` relabeling compiles; downstream Grant-use + Event append composition compiles; caller-owned Provider implementations inherit default Ready; private observation fields, `Tx.inner`, and arbitrary SQL are inaccessible. `E0432` for `SyntheticDispatchPermit`/`CommitProof` only demonstrates those types do not exist.

The initial full-workspace attempt caught the test-only `CapabilityProvider` and `Instant::now()` because static scans inspect every source path; moving the fixture under `tests/support` resolved the source-closure issue. Clippy then rejected the test's elapsed-time check (`Instant::now`); it was removed. Final full-workspace tests and Clippy were rerun after these corrections and passed. The lock witness is direct `SQLITE_BUSY` evidence; the typed writer's bounded error while held, followed by successful retry and post-commit projection, corroborates the competing operation.

Observer-specific regression sensitivity was checked in detached scratch worktree `/workspace/Serea-p8b-scratch` with independent `CARGO_TARGET_DIR=/tmp/serea-p8b-red-target`. Mutating the Storage transaction behavior from `Immediate` to `Deferred` caused the Overlay observer test to fail at the lock-witness assertion (`unwrap_err` received `Ok(())`), exit 101 (**RED**). Restoring `Immediate` made the same test pass, 1 passed/0 failed, exit 0 (**GREEN**). The scratch source was restored and `git status` was clean before removing the scratch worktree.

The first Native validation on commit `c06ac369339db7f089263b5e3d1083b152be4507` exposed a bounded-test-timeout issue on Rust 1.85 macOS ARM64: all four observer tests timed out waiting for the competing writer's SQLite busy result at the 7-second receive timeout, while the independent lock witness had succeeded. The observer/writer protocol was not failing its state assertions; the timeout was too short under that runner's parallel test load. The test now waits up to 30 seconds for the writer result and up to 60 seconds for barrier cleanup/release. Stable and 1.85.0 local observer tests both pass with the revised bounds. New-HEAD CI must confirm the cross-platform fix.

The previous standalone Storage and policy transaction tests are adjacent evidence only and do not satisfy this table. No category is marked PASS until the observer's actual transaction is held.

## Remaining owner/test work and CI

1. Add a normal-path Grant fixture and exercise authenticated revocation through the same observer barrier. Do not use forced SQL updates. Until this exists, overall L1-A remains PARTIAL.
2. Complete remaining observer-specific negative checks where the fixture/API supports them: mismatched pins, missing Grant, tampered caller `PolicyEvaluation`, and explicit Grant-use/Event/Budget/RequestId/IDK before/after accounting. The external probe confirms caller-mutable policy surface and mutation composition but does not itself instrument an observer runtime ledger.
3. The latest pushed commit must pass GitHub Actions Fast, Full, Native, SQLite, and Hooks at its exact SHA. CI on the superseded `c06ac36…` commit is diagnostic only; the revised 30/60-second test bounds need final-HEAD Native validation.

## Classification rule

Use `L1A_OBSERVER_BARRIER_CONFIRMED` only if the five categories, read-only properties, reopen/integrity, and observer-specific RED/GREEN are evidenced. Otherwise use `L1A_OBSERVER_BARRIER_PARTIAL` or `BLOCKED` and name the missing categories. The L1-B document is separately `PROPOSED / NOT RATIFIED`; its completion does not authorize runtime behavior.
