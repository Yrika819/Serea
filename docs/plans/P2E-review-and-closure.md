# P2E Lease Authority and Generation Fencing Review and Closure

## 1. Preflight and frozen pre-implementation gate

Starting branch: `p2/p2d-blobs-classification`. Starting HEAD:
`e6c57a03211d8493ac82cdd35ab402379ccf24da` (`feat: implement P2D blob classification`).
Git status was clean. Both requested offline baseline commands passed before
branch creation. Both actual baselines: **460 regular + 19 doctests = 479**, zero
failures or ignored executions; separately logged count-confirmation reruns also
matched. Stable Rust1.98.1 and Rust1.85.0, x86_64 host.
Created `p2/p2e-lease-fencing` from that exact HEAD; no amend or push.

The following findings and dispositions were frozen before production edits.
All BLOCKER/MAJOR design findings are resolved by these decisions; implementation
and runtime evidence remain separate gates.

| ID | Severity | Finding | Frozen disposition |
| --- | --- | --- | --- |
| E1 | BLOCKER | Phase prose claims outcome/receipt/journal fences without lifecycle methods | P2E owns lease authority only. H9–H13, outcome agreement in H15, begin half of H18 and outcome half of H22 belong to P2F. No fake commit API. |
| E2 | BLOCKER | Outer callers can catch an acquisition error and commit its partial writes | Internal SQLite savepoint encloses the complete acquisition. Explicit rollback/release on error. If savepoint cleanup fails, mark the outer Tx rollback-only and prevent its commit. No dependence on `?`. |
| E3 | MAJOR | `expected_old_generation` has no caller source | Public `expected_generation: Option<u32>`: None maps to SQL0; Some(n) is exact observed positive generation. Some(0) refuses as LeaseFenced. Step UPDATE retains exact equality; never substitute a fresh read. |
| E4 | MAJOR | Renewal can shorten or immediately expire | Current matching unreleased row must have expiry > now; new expiry must be strictly greater than authoritative old expiry. Equal/shorter gives InvalidLeaseInterval without mutation. Stale/released takes LeaseFenced precedence; matching expired takes LeaseExpired. |
| E5 | MAJOR | Release semantics and consuming-error behavior are unstated | Release changes only leases.released_at_ms; step copy deliberately remains unchanged. Guard is consumed on every result, including infrastructure failure; Err is NOT proof of durable release. Safety over retryability; eventual expiry/recovery is required. Outer transaction rollback also invalidates a returned guard's purported authority. |
| E6 | MAJOR | Overflow can become a generic CHECK error | Eligible max-u32 generation returns payload-free LeaseGenerationOverflow before increment, with SQL bounded increment predicate as defense. No wrapping, clamp, text parsing or schema weakening. |
| E7 | MAJOR | Attempt ceiling and begin ownership are mixed | Acquisition increments once and reads tasks.max_attempts_per_step durably in the same savepoint. Every reclaim spends an acquisition. P2F owns begin_attempt and its no-second-increment proof. |
| E8 | MAJOR | Invalid absolute intervals are not intentional errors | Explicit EpochMillis now/expiry, no retained clock. Acquire expiry <= now gives InvalidLeaseInterval. Release now before acquired_at likewise refuses InvalidLeaseInterval rather than generic CHECK failure. Core owns max_lease_seconds. |
| E9 | MAJOR | Guard shape could be mistaken for durable authority | Private fields, no constructor/Clone/Copy/Serde; acquired only on success. No owner formatter and no Drop side effects. SQLite alone authorizes renew/release. |
| E10 | MAJOR | Zero-row refusals are conflated | Held active lease => LeaseHeld; stale expected generation, wrong binding/missing/noneligible step, dead guard => LeaseFenced; matching expired renewal => LeaseExpired; otherwise exhausted budget => AttemptCeilingReached; otherwise max generation => LeaseGenerationOverflow. Reads remain under BEGIN IMMEDIATE/savepoint. |
| E11 | MAJOR | ADR acceptance is overstated by lease-only closure | ADR-0024 stays Proposed: begin and embedded outcome UPDATE fencing remain P2F. Historical probes are not production evidence. |

Selected API (value parameters match validated protocol types):

```rust,ignore
pub fn acquire_lease(
    &mut self,
    task_id: TaskId,
    step_id: StepId,
    owner: LeaseOwner,
    expected_generation: Option<u32>,
    now: EpochMillis,
    expires_at: EpochMillis,
) -> Result<LeaseGuard, StoreError>;
pub fn renew_lease(&mut self, guard: &LeaseGuard, now: EpochMillis,
                   new_expiry: EpochMillis) -> Result<(), StoreError>;
pub fn release_lease(&mut self, guard: LeaseGuard, now: EpochMillis)
    -> Result<(), StoreError>;
```

Acquire accepts PLANNED/LEASED/EXECUTING only. Held authority takes precedence
over stale expected generation for an otherwise bound eligible step; stale caller
expectation is checked before budget/overflow. Release permits matching expired
leases; it revokes rather than transitions. Renewal mutates authoritative expiry
only; the step expiry copy is an acquisition snapshot, not authority. Generation
copy agreement is derived in SQL at acquisition. Migration 0001 is expected to
remain byte-identical; no 0002 is authorized.

Outcome fencing not yet proven; P2F owns the atomic outcome UPDATE.

## 2. TDD and implementation evidence

The first actual RED was `cargo test -p serea-storage --lib lease_tests --offline`,
exit101: missing root LeaseGuard, missing acquire_lease and missing LeaseHeld/Fenced
categories. The first three tests were written before implementation: PLANNED ->
LEASED/gen1/attempt1, competing refusal, and caught expected-generation failure.
The full initial authority suite was written before implementing the API.

First GREEN: same focused command, **35 passed**, zero failed/ignored. Feature
work stopped immediately for independent reviews. Subsequent additions are bounded
review remediation, not P2F feature development. Current storage remediation GREEN:
**172 unit + 2 integration + 24 doctests = 198**, including **50 lease tests**.

Acquisition uses an internal rusqlite savepoint under Store's BEGIN IMMEDIATE.
Preflight classifications and exact expected generation are not outside the writer
transaction. The upsert increments authoritative generation in SQL; the step
UPDATE derives its copy through SELECT from leases, never Rust arithmetic. The
ceiling comes from tasks in the same operation, with safe arithmetic preflight and
a post-write durable check. Explicit rollback/release restores every method write,
even when caught. Late ABORT, IGNORE/zero-step-update and post-write ceiling fixtures
prove this structurally; unrelated pre/post blob writes still commit. Cleanup
failure marks rollback-only; all public operations refuse thereafter and outer Ok
cannot commit, including when SQLite has already destroyed the transaction.

Guard fields are private task/step/owner, NonZeroU32 generation, and a private
Arc<AtomicBool> acquisition-origin marker. No public constructor, Clone, Copy,
Serde, Display or Drop side effect; Debug emits only LeaseGuard. Six compiler
negative guard doctests pin constructor/privacy/traits/consumption. Additional
actual offline rustc Serialize/Deserialize probes both fail specifically with E0277,
with a successful String Serde control, against existing build artifacts. No new
dependency is needed; a structural source test also guards absence of Serde/Drop.

The origin marker was added only after frozen independent finding F1. Successful
outer commit publishes it; rollback/panic/commit error leaves it permanently false.
Pending guard use is allowed only in its origin Tx. This prevents an escaped guard
from aliasing a later same-owner acquisition that reuses an uncommitted generation.
It is NOT process-local lease authority, a registry or an authorization precheck;
it only rejects invalid capabilities. SQLite fences remain mandatory for every
renew/release, including committed guard use via a different Store.

File-backed evidence opens two independent Stores on one migrated database:
A gen1; B different/same-owner held refusals; B expiry reclaim gen2; A old renewal
and release fenced; B current renewal/release success; fresh reopen preserves gen2
and reacquires gen3. A separate barrier/thread test starts both independent Stores
simultaneously and proves exactly one successful acquisition and one LeaseHeld.
No global lease mutex/map participates; the per-connection mutex only guards SQLite.
This is cross-Store evidence, not a child-process/crash stress claim.

## 3. Independent reviews and remediation

Three independent read-only passes ran against the frozen first GREEN:
A SQL/concurrency; B transaction/security; C architecture/scope. All reported the
same rolled-back guard alias, merged as **F1 Major**. C also found **F2 Major**:
P2D's old lexical API inventory forbade the newly authorized lease module. Seven
Minor gaps (T1–T7) were merged by semantic failure/coverage, not reviewer count.
Canonical initial report is immutable in local tmp/reviews, validated as 2 Major,
7 Minor test gaps,9 coverage areas,Changes requested before production remediation.

Coordinator reproduced F1 through genuine Rust RED tests for initial and reclaimed
rollback followed by same-owner reacquisition: old renewal returned Ok instead of
LeaseFenced. F2's exact existing regression test failed on forbidden mod lease.
Accepted F1 repair is the origin marker described above; F2 admits and inventories
only P2E leases while retaining every P2D text/reference/engine/journal prohibition
and the existing test name. All seven gaps received focused tests or actual compiler
probes: late-failure/unrelated-work, authoritative second-renewal and interval
precedence, released-MAX/stale/ceiling ordering, release outer rollback, poisoned
operation continuation, late zero-row/post-write ceiling, and negative Serde traits.

Test-only private capability duplicates are restricted to repeated-use/revocation
simulations; they are not public Clone/constructibility or production evidence of
caller duplication. Actual escaped-rollback tests use only successful public
acquisition calls. Deferred commit-FK failure and panic/reopen regressions prove
that unsuccessful origin never publishes capability validity.

Bounded terminal re-review returned A PASS, B PASS and C all original items fixed.
C noted only that planned closure-status annotations still said implementation
pending while this record already contained evidence. Scheduled final closure
publication reconciles those five annotations (no further production repair or
review discovery cycle). Coordinator verified the delta and all review-required
surfaces; no unresolved Blocker/Major or coverage gap remains. The terminal
canonical report links the immutable initial report and completed resolution.

Review probes are scoped honestly: A reran all50 lease tests and the exact inventory
case; B reran13 affected tests; C inspected source/logs/protected-file bytes, without
claiming to have run the full suite. Coordinator executed the final commands below.
No reviewer edited production files or Git state.

## 4. Final validation

Actual stable **Rust1.98.1**, MSRV **Rust1.85.0**, **x86_64** host. All required
commands passed on the final production tree. Logs are local ignored
`tmp/p2e-final-*.log`; grouped result JSON records command exits and durations.
Three parallel batch wrappers hit their300-second tool limit after successfully
recording workspace tests and the first focused modes. No test failure was reported;
the uncompleted Clippy and MSRV focused commands were rerun in separately bounded
commands and passed. Timeout is recorded, not erased or counted as a test PASS.

| Required command | Actual result |
| --- | --- |
| cargo fmt --all -- --check | PASS |
| cargo check --workspace --all-targets --offline | PASS |
| cargo test --workspace --all-targets --offline | PASS510 regular |
| cargo test --workspace --all-features --offline | PASS510 regular+29doctests=539 |
| cargo clippy --workspace --all-targets --all-features --offline -- -D warnings | PASS separate completion run |
| cargo +1.85.0 check --workspace --all-targets --offline | PASS |
| cargo +1.85.0 test --workspace --all-features --offline | PASS510 regular+29doctests=539 |
| cargo +1.85.0 clippy --workspace --all-targets --all-features --offline -- -D warnings | PASS separate completion run |
| python3 tests/workspace_smoke.py | PASS exact3 members/layering |
| python3 -m unittest discover -s tests -p workspace_smoke_tests.py | PASS37 |
| python3 -m py_compile tools/validate_docs.py | PASS |
| python3 tools/validate_docs.py docs | PASS59 Markdown files |
| git diff --check | PASS |
| cargo metadata --no-deps --format-version 1 | PASS exact protocol/storage/testkit |

Focused `cargo [+1.85.0] test -p serea-storage [--release] --offline` passed all
four modes: stable debug, stable release,1.85 debug,1.85 release. Each executed
**172 unit+2 integration+24 doctests=198**, including all50 lease tests and the
file-backed shared-authority, simultaneous-two-Store and panic/reopen cases.
Zero failed/ignored in every completed final test command. No mode discrepancy.

Execution-name Counter comparison with line-normalized doctest names finds **zero
missing baseline executions**. Final539 minus baseline479 is **60 new executions:
50 regular+10 doctests**. Existing inventory test retains its name and assertions
except admitting the authorized P2E module/methods. P2D's obsolete missing-acquire
doctest is reconciled to missing-begin; no applicable protection, migration,
connection, clock, canonical, wire or testkit regression is discarded. Compile
failures from first RED and intentional review REDs are not final validation failures.

Migration disposition: `0001_initial.sql` byte-identical to starting HEAD;
SHA-256 **d9068dccbc26ececb71be79c475080633166ba0163c62b2d98b9733512baefea**
(catalog form adds sha256:). No0002, no checksum/catalog update or weakening.
All P2C migration/catalog/checksum/connection/integrity tests ran in the complete
storage suite in all four modes. Manifests/lockfile/vendor/protocol/testkit and
workspace membership are unchanged; no dependency added. P2D blob/protection
logic is unchanged except refusing use of an inactive/rollback-only Tx before
backend/SQL work; active-transaction protection behavior is covered and unchanged.

Final closure-status/document/diff/fmt checks are repeated after publication, and
only this P2E scope is eligible for the requested single commit.

## 5. Scope and explicit nonclaims

Exactly protocol/storage/testkit. No begin_attempt, task lifecycle/outcome commit,
receipt insertion, result reference attachment, journal append, recovery, engine,
provider, event bus, P2F or P3. No empirical Apple Silicon or child-process crash
claim. Cross-Store evidence uses separate file-backed connections, not a local
registry. No new wire version, migration weakening, or protection backend.
Outcome fencing not yet proven; P2F owns the atomic outcome UPDATE. Begin-attempt,
receipt/journal/result/task transition fencing is NOT proved by lease-only tests.
No public validate_guard/is_current_lease/check_fence helper is exposed.
ADR-0024 remains **Proposed**, not Accepted. ADR-0022 protection and ADR-0021
participant/journal dispositions remain unchanged.

### Changed-file inventory and stop

14 paths, all P2E:

- `crates/serea-storage/src/{lease,lease_tests}.rs` (new authority/tests);
- `crates/serea-storage/src/{lib,tx,store,error,blob,blob_tests}.rs` (wiring,
  fail-closed transaction validity, payload-free errors and P2D scope inventory);
- `docs/decisions/ADR-0024-lease-fencing-and-commit-under-lease.md`;
- `docs/plans/{P2-storage-task-engine,P2-test-matrix,P2-contract-gap-analysis,P2-tomorrow-decision-ledger,P2E-review-and-closure}.md`.

Dependencies added:0. Vendor changes:0. Migration changes:0. New workspace members:0.
Exactly protocol/storage/testkit; Rust MSRV1.85 and arch1/task2/action2/event1
remain unchanged. Public mutation stays Tx-scoped. No engine, begin/outcome,
recovery, event bus, P2F or P3 started.

**P2E CLOSED for lease authority. Ready for a fresh P2F context, not permission to
start P2F here.** One `feat: implement P2E lease fencing` commit with exact P2D
parent, no earlier amend and no push. The commit's own hash and clean-worktree
confirmation are reported from Git after commit, not embedded self-referentially
in this file. STOP.
