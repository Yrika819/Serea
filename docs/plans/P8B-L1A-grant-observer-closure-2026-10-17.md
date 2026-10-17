# P8B L1-A grant observer closure campaign

**Status:** `L1A_OBSERVER_BARRIER_CONFIRMED_SYNTHETIC_ONLY` for the five tested SQLite writer races and observer no-write invariants on the tested fixtures; Grant authentication uses a test-only accepting verifier and does not prove production Owner authentication. `L1B_TRUST_ROOTS_PROPOSED`; `NATIVE_ROOT_CAUSE_UNCONFIRMED`.

## Goal and baseline

Continue PR #32 from `17d8d5e5059c720edb68d307a28282914b537b9a` in isolated branch `codex/p8b-l1a-grant-closure-2026-10-17`. The original `/workspace/Serea` checkout remains untouched on `work` at `e0acb0254820c381dd97828824bef86a1db5176c`. The dedicated clone/worktree is `/workspace/Serea-codex` and `/workspace/Serea-p8b`. Local identity is `Codex <noreply@github.com>`, `user.useConfigOnly=true`, `push.default=nothing`; hooks, commit preflight, and reachable-history identity check passed. PR #32 remains open, Draft, head `17d8d5e…`, base PR #31's branch; no review is recorded. No PR #33 existed at preflight.

The requested PR #32 head matches the mandate. `origin/main` was `e0acb0254820c381dd97828824bef86a1db5176c`. Open-stack metadata: #29→`p8b/owner-ratified-boundary-2026-10-17`; #30 and #31 both base on #29's branch, with #31 containing #30's work; #32 bases on #31's branch. Retargeting was not performed. The initial `mergeStateStatus=UNSTABLE` snapshot coincided with incomplete checks; after all 25 PR #32 jobs passed, GitHub reported `mergeable_state=clean`. PR #32 has no submitted reviews or review comments. Branch-protection read returned HTTP 403 `Resource not accessible by integration`, so required protection configuration could not be inspected.

## Changes and observations

### Writer contention classification

In `crates/serea-task-engine/tests/support/observer_barrier_tests.rs`, `run_race` now carries the typed writer's first result across the release barrier and requires exactly `Err(StoreError::Busy)`. A successful write, fixture/validation error, or generic SQL error no longer counts as the expected conflict. The observer is released and both threads joined before this assertion, so a failed assertion does not leave the writer blocked on the test channel. `serea_policy::activate_revision` preserves `PolicyError::Storage(StoreError::Busy)` rather than mapping all failures to generic `Sqlite`.

No public API or production code changed. The existing test-only private `#[cfg(test)]` observer seam remains private. The third connection continues to independently observe SQLite `DatabaseBusy`/`DatabaseLocked` using a zero-timeout `BEGIN IMMEDIATE`; it is separate corroboration, not a substitute for typed-writer classification.

The new `observer_success_and_fact_mismatch_leave_every_durable_row_unchanged` test captures every row/value in each ordinary SQLite table before observation, after a valid no-Grant observation, after a `StepInputDigest` mismatch observation, after a callback-forced transaction rollback, and after reopen. It verifies identical fingerprints and `Store::verify_integrity()`. The fake Provider panics if invoked. This includes all event and budget/accounting rows in the fixture. Each of the five held-race cases also fingerprints every ordinary table after the competing writer has returned `Busy` but before the observer transaction is released; all rows are unchanged at that point.

The Grant race builds an `Authorization::ScopedGrant` descriptor and a real typed ApprovalRequest → `AuthenticatedApprovalResponseV1` → ACTIVE sealed Grant using Storage APIs; it does not hand-insert SQL rows. A test verifier accepts the synthetic response, then `AuthenticatedApprovalRevocationV1` goes through `Store::revoke_approval_grant`. With the observer holding the SQLite transaction and the Grant id passed into observation, the separate lock witness sees busy and the typed revoke returns exactly `StoreError::Busy`. After release, retry succeeds. The observer's captured Grant snapshot remains ACTIVE and unused; reopened state is REVOKED with one remaining use, zero uses consumed, exactly one additional revoke event, unchanged Grant-use count, and valid DB integrity. This proves the database race and accounting behavior under the test verifier; it does not establish a real Owner identity/authentication source.

### Race matrix

| Held observer / competing writer | Expected | Measured on baseline `17d8d5e…` after harness change |
|---|---|---|
| Policy activation | Busy while observer owns transaction; succeeds after release | PASS; typed first result exactly `StoreError::Busy`; retry commits |
| Task cancellation | Same; cancellation visible after commit/reopen | PASS; typed first result exactly Busy; retry commits |
| Lease reclaim | Same; new generation fences prior guard | PASS; typed first result exactly Busy; retry commits and fence advances |
| Overlay update | Same; new revision visible after commit/reopen | PASS; typed first result exactly Busy; retry commits |
| Authenticated Grant revocation | Same; old snapshot ACTIVE, fresh/reopened snapshot REVOKED; Grant use unchanged | PASS via typed synthetic test verifier; exact Busy, retry commits, state/event/use/integrity assertions pass |

The fixture gap was resolved without private SQL fixtures: the task-engine test now creates the Grant through the public typed request, authenticated-response, and revocation APIs, while retaining the private observer seam. The test-only verifier is intentionally synthetic. The harness's 30-second result and 60-second cleanup bounds remain deadlock guards, not proof that the protocol is healthy under every runner. A timeout or invalid writer result is a test failure.

### Commands and results

- `cargo +stable test -p serea-task-engine --lib observer_barrier_tests --locked --offline`: first attempt **NOT RUN** because the empty initial Cargo cache lacked `async-trait`; no source failure. After network dependency fetch, the pre-edit baseline run passed 4/4.
- Final targeted suite: `cargo +stable test -p serea-task-engine --lib observer_barrier_tests --locked` **PASS**, 6 passed, 0 failed; Rust 1.85.0 **PASS**, 6 passed, 0 failed. Both include all five competing writer categories, no-write/fact mismatch/rollback coverage, and the exact `StoreError::Busy` assertion.
- Rust stable and 1.85.0 were installed under `/workspace/.cargo`; the PATH used was `/workspace/.cargo/bin:$PATH`. `Cargo.lock` was not modified.
- New contention-assertion sensitivity check: detached worktree `/workspace/Serea-p8b-red`, independent `CARGO_TARGET_DIR=/tmp/p8b-red-target`. **RED** mutated the expected typed error from `StoreError::Busy` to `StoreError::Sqlite`; targeted Overlay test exited 101 at the intended assertion with actual `Err(Busy)` vs expected `Err(Sqlite)`. The mutation was restored. **GREEN** reran that exact test from the restored source and passed, 1 passed/0 failed, exit 0. This proves the assertion detects loss/misclassification of the SQLite contention error; it does not cover Grant semantics or other mismatch assertions.

Final `cargo +stable test --workspace --all-targets --all-features --locked` and the same command on `+1.85.0` both **PASS**, 1,480 passed, 0 failed, 2 ignored (69 result groups each). This is +2 tests from PR #32's 1,478 baseline. Stable workspace check, Stable/MSRV Clippy and check, release Host check, formatting, locked metadata, docs validation, workspace smoke guard plus 82 regression tests, external authority Cargo probe, static Core API and Synthetic Host boundary, and `git diff --check` passed during this campaign.

## Captured prior-HEAD incident detail

Downloaded logs for macOS ARM64 MSRV job `117378087612` of run `39026384475` show all four observer tests failing at the `blocked_rx.recv_timeout(Duration::from_secs(7)).unwrap()` line with `Err(Timeout)`. The independent `BEGIN IMMEDIATE` witness had already succeeded earlier in each test. When the parent test panicked, the observer callback's release channel closed and the worker reported its consequent Store error; that secondary panic is cleanup fallout, not an independent storage defect. This directly confirms the old bounded result wait expired before the typed writer returned. It still does not explain why the writer needed more than seven seconds under that runner's load.

The Windows ARM64 job `117357401817` remains as previously documented: the step fails during all-feature tests and the host lifecycle assertion only reports a non-success child `ExitStatus`; the fetched Actions API did not expose the old job log body. There is no exit code or stream output available to narrow the cause. PR #32's ARM64 jobs passed on its old HEAD; final campaign HEAD CI is pending.

## Independent L1-B scratch feasibility probe

`/tmp/p8b_l1b_synthetic.rs` was compiled with `rustc +stable --edition 2021` and run. Its private module model uses non-Clone Worker principal instances and lease bindings with incarnation and generation; it demonstrated that a second principal with the same owner text, a stale generation, and a revoked principal fail the local predicate. A simple inclusive expiration predicate passed `now < expires_at`, `now == expires_at`, and `now > expires_at` examples. Result: `SYNTHETIC_PASS`.

This experiment is an isolated language-level sketch, not a Serea API prototype, real authentication, trusted clock, LeaseGuard provenance test, cross-process boundary, or protection from a compromised Host. Its public surface is intentionally inaccessible outside the private module. The selected recommendation and Owner choices remain in [P8B L1-B decision package](P8B-L1B-trust-roots-owner-decision-2026-10-17.md), still `PROPOSED / NOT RATIFIED`.

## Native incidents and Actions baseline

- PR #32 SHA `17d8d5e…`: Fast CI run `39026866678` **success** (1/1 job); Hooks `39026866630` **success** (3/3); SQLite portability `39026866638` **success** (4/4); Full `39026866743` **success** (5/5); Native `39026866679` **success** (12/12). Final Native poll showed all Stable/MSRV targets passing, including Windows ARM64 and macOS Intel, so the longer observer bounds passed on this HEAD. That is positive regression evidence for the timeout adjustment, not proof of why the old 7-second wait expired. Windows ARM64's changed child diagnostics did not accompany a recurrent failure in this successful run; its previous root cause remains unconfirmed. These checks are all for PR #32's `17d8d5e…`, not this worktree's future HEAD.
- Prior Windows ARM64 incident run `39019193381`, attempt 1, job `117357401817`: the failing step was “Test all workspace features” on Stable Windows ARM64. The child process spawn succeeded; the test assertion observed a non-success `ExitStatus`, but old diagnostics omitted status code and both streams were empty. Exact root cause remains unavailable. Attempt 2 passed, which does not establish a repair. The newer diagnostic code in PR #32 must be evaluated only on a recurrence; no recurrence is not a root cause.
- macOS observer incident: previous HEAD `c06ac369…` had all four observer tests fail waiting for writer-result channels under macOS ARM64/MSRV and Intel/Stable runs. Independent SQLite lock witnesses succeeded, indicating the competing writer was still waiting within the configured busy timeout when the test's receive deadline elapsed. The timeout was then raised to 30 seconds, with 60 seconds for cleanup. This is consistent with test scheduling/SQLite busy wait timing under parallel load, but does not establish a root cause. PR #32's revised bounds passed on that old HEAD; campaign final-HEAD CI remains pending.

## Limits, result classification, next step

- `L1A_OBSERVER_BARRIER_CONFIRMED_SYNTHETIC_ONLY`: all five writer categories show typed SQLite Busy while observation is held and successful retry after release; the full ordinary-table fingerprint remains identical while the observer is held in every race. Successful, mismatch, and forced-rollback observations preserve the same fingerprint on the no-Grant fixture. The Grant test proves ACTIVE→REVOKED retry semantics, revoke Event delta, use invariance, reopen, and integrity through typed Storage APIs with a synthetic test verifier. The RED/GREEN assertion mutation proves the Busy-classification assertion rejects Sqlite instead.
- `L1B_TRUST_ROOTS_PROPOSED`; `L2_AUTHORIZED_ATOMIC_COMMIT_NOT_PROVED`; `L3_HOST_PERMIT_NOT_PROVED`; `PRODUCTION_DISPATCH_DISABLED`; `ADR_0046_PROPOSED`.
- `NATIVE_ROOT_CAUSE_UNCONFIRMED` for Windows ARM64 and macOS timeout causes.
- The original PR #32 HEAD has no changes from this campaign; its checks are baseline only. PR #32 still pointed at `17d8d5e…` with base `codex/p8b-l1a-facts-proof-2026-10-17`. Branch protection inspection through the app returned HTTP 403 `Resource not accessible by integration`; required checks/reviews cannot be confirmed through that endpoint. Existing PRs have not been changed.

Next work: commit and push the isolated branch, open at most one Draft PR stacked on PR #32, then inspect Actions for the exact campaign HEAD. Do not report CI success for a different SHA as final validation.
