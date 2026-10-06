# P2F Atomic Outcome Commit / Receipt and Journal Fencing

## 1. Authoritative start and corrected frozen design

Branch `p2/p2e-lease-fencing`; starting HEAD
`d3413af80a0f2b82926c643607a0ffad1f55fcb5` (`feat: implement P2E lease fencing`).
Exact branch/HEAD and clean worktree verified before edits and again on continuation.
Stable Rust 1.98.1 and MSRV 1.85.0 baselines each passed **510 regular + 29 doctests
= 539 executions**, zero failures/ignored. Execution identities retained in ignored
`tmp/p2f-baseline-{stable,msrv}-identities.json`; counters match across toolchains.

The initial P2F task instruction incorrectly requested strict TTL rejection for
outcome commit. Pre-implementation contract review detected that this contradicted
ADR-0024 and the frozen P2 design. The existing contract was retained unchanged:
expiry enables reclaim but does not independently revoke a current generation's
known-result commit authority. Owner equality never exempts a stale generation.
This correction is not a new architecture decision or ADR acceptance.

The following design is frozen **before production implementation**.

### F1 — Scope and API

Storage-only whole transitions, not a task engine, executor, scheduler or recovery.
No new workspace member, dependency, SQL escape hatch, migration or protocol version.

```rust,ignore
pub fn begin_attempt(&mut self, guard: &LeaseGuard, now: EpochMillis,
                     context: &TransitionContext<'_>) -> Result<(), StoreError>;
pub fn commit_step_outcome(&mut self, guard: LeaseGuard,
                           outcome: StepOutcome<'_>, now: EpochMillis,
                           context: &TransitionContext<'_>) -> Result<StepCommit, StoreError>;
```

`StepOutcome` has Succeeded { original result_json bytes, optional protocol
SideEffectReceipt } and Failed(StepFailure) for a **known final failure**.
StepFailure uses validated protocol kind/code/message/host_action/failure_reason,
retryable metadata and optional original details JSON object bytes. AMBIGUOUS is
not a known final outcome and is refused; reconciliation/block/resume are later
lifecycle work, not an invitation to guess or re-execute. No RECONCILED_ABSENT
writer or waiting/cancellation/planning/deletion API is added in this bounded slice.
No descriptor registry: receipt presence reflects the supplied known effect;
P5 still owns descriptor-dependent receipt requirements/provider-reference rules.

TransitionContext supplies validated actor kind/id/version and optional causation
identity, not lease identity or detached transition authority. Journal transition
identity is constructed privately from the actual writes. StepCommit describes
inner success (step status, task state, attempt, generation, optional result ref);
it is durable only when Store::transact returns Ok.

### F2 — BEGIN_ATTEMPT and attempts

P2E acquisition charges exactly once, including expiry/released reclaim. Begin
borrows the guard, stamps LEASED -> EXECUTING and task/journal facts, and charges
**zero**. Outcome, outcome refusal and stale worker charge **zero**. A new attempt
requires a successful acquisition, hence another P2E charge; durable ceiling and
P2E arithmetic/precedence are unchanged. No second mutable attempt counter.

### F3 — Ownership and commit ambiguity

Outcome consumes LeaseGuard on **every result** (including Sqlite/Busy), just like
P2E release. Release and outcome are mutually exclusive terminal capability uses.
No returned replacement capability. Method failure, outer rollback, panic or
outer commit failure sacrifices retry with that guard, even if SQL rolled back.
A caller must not infer durability from an inner Ok, or absence of an effect from
Err. On an ambiguous outer commit the caller retains no reusable outcome guard;
SQLite lifecycle/generation/receipt uniqueness also prevent duplicate writes.
A future attempt needs a new successful acquisition after expiry/release, subject
to lifecycle eligibility and ceiling. No recovery loop is implemented.

P2E origin semantics remain exact: pending guard use is permitted in its origin Tx;
other Tx/Store requires the confirmed acquisition commit marker. Rolled-back,
panicking or failed-commit acquisition origins never become valid, even if durable
owner/generation later happen to match. No public validity precheck.

### F4 — SQL authority and precedence

Store's BEGIN IMMEDIATE serializes SQLite writers. Every first step UPDATE binds
step_id/task_id, step owner/generation, expected LEASED (begin) or EXECUTING
(outcome), and EXISTS the matching authoritative owner/generation/unreleased
leases row, parent task expected lifecycle, and sequential prerequisites.
Outcome **does not require unexpired TTL**; begin requires leases.expires_at_ms >
now. Both require now >= authoritative acquisition and begin/outcome timestamps
not to run backward. Origin rejection is additional rejection, never SQL authority.
Task UPDATE retains the actual expected task state. Final lease UPDATE retains
owner/generation/task/unreleased authority. Every expected one-row write is checked.

Under the same locked snapshot, invalid origin/stale/missing/released/binding or
step/task lifecycle => LeaseFenced first. Matching valid begin with expiry <= now
=> LeaseExpired; matching backward time => InvalidLeaseInterval. Outcome expiry
alone has no refusal category. These precede malformed outcome/class/receipt
validation. Duplicate terminal commit is LeaseFenced (before receipt insertion).
Known current bad data uses existing ConstraintViolation/CanonicalJson categories;
no new StoreError category or disclosure/source chain.

### F5 — Supported transition table

Conservative sequential policy: every earlier sequence must be SUCCEEDED; no
parallel admission or scheduling. Kind VERIFY begins only in VERIFYING; other
kinds begin in READY/EXECUTING. Wait-kind success here is a supplied known result,
not an implementation of wait orchestration.

| Operation | Allowed task / step pre-state | Result step | Result task | Receipt / journal | Lease | Eligibility / retry |
| --- | --- | --- | --- | --- | --- | --- |
| begin ordinary | READY or EXECUTING / LEASED | EXECUTING | EXECUTING | no receipt; STEP_ATTEMPT_STARTED and TASK_STATE_CHANGED only if changed | retained | no second charge; expired begin refused |
| begin verification | VERIFYING / LEASED VERIFY | EXECUTING | VERIFYING | no receipt; STEP_ATTEMPT_STARTED | retained | borrowed guard; no execution engine |
| success with ordinary work remaining | EXECUTING / EXECUTING non-VERIFY | SUCCEEDED | READY | optional effect receipt; STEP_COMMITTED, optional RECEIPT_RECORDED, TASK_STATE_CHANGED | released, step owner/expiry cleared | next ordinary step may begin after acquire; no scheduling |
| success with only VERIFY work remaining or no work remaining | EXECUTING / EXECUTING non-VERIFY | SUCCEEDED | VERIFYING | same success rows | same | verification becomes possible; absence of verifier does not fabricate COMPLETED |
| verification success | VERIFYING / EXECUTING VERIFY | SUCCEEDED | VERIFYING if more VERIFY remains, otherwise COMPLETED | success rows; TASK_STATE_CHANGED/TASK_TERMINAL on completion | same | only remaining VERIFY permitted; terminal has no outgoing transition |
| known final failure | EXECUTING or VERIFYING / EXECUTING | FAILED with complete error | FAILED with failure_reason | no receipt; STEP_FAILED, TASK_STATE_CHANGED, TASK_TERMINAL | same | no retry of terminal step/task; error_retryable is recorded metadata only |

No caller chooses task destination; it is derived from durable remaining steps.
No EXECUTING -> COMPLETED shortcut. A new acquire before final outcome (released/
expired attempt) is the P2E retry/reclaim path and spends one charge.

### F6 — Atomic set and method failure safety

Internal SAVEPOINT encloses the complete begin/outcome method. Fenced step UPDATE
is the **first mutation**; zero rows returns LeaseFenced before blob/receipt/journal.
Success stores SCJ-1 RESULT using existing put_blob, exact task-derived class,
step RESULT reference, optional receipt bound to step capability/key, task update,
required private journal rows, lease release, retaining generation/attempt/started
snapshot and stamping completed/updated/released instants. Failed outcome stores
complete error/details instead of blob/receipt, plus task failure/journal/release.
PRIVATE-bearing task/receipt/journal rows fail closed even with a blob backend;
PUBLIC/PERSONAL supported. No ordinary-row PRIVATE protection claim.

On caught error explicitly ROLLBACK TO and RELEASE savepoint: unrelated prior/later
outer writes may commit but none of the method's writes may. Cleanup failure marks
Tx rollback-only; existing Store::transact cannot commit even if caller returns Ok.
Outer rollback/failed COMMIT/panic removes the entire atomic set; P2E marker remains
unpublished for uncommitted acquisitions. No Drop side effect on LeaseGuard.

### F7 — Receipt and journal semantics / phase seam

Receipt identity is receipt_id PRIMARY KEY and UNIQUE(step_id), with durable
step task/key/SUCCEEDED triggers. Also bind capability_id to the durable step.
Receipt is durable idempotency evidence, not cache. No receipt row writer exposed.

Use existing task_journal vocabulary, task-local MAX(journal_seq)+1 inside SQLite
writer transaction, journal_id derived from task+sequence (not a wire EventId).
Persist injected actor/time/causation and generation/attempt/result transition
snapshot; journal is required inside the method savepoint, not queued after body.
No SereaEvent, event_seq, host Seq, event table, backfill or version change.

The full higher-layer TaskJournal/TransactionParticipant seam of Proposed ADR-0021
requires later consumer design. This storage-only slice uses a fixed private
journal append, not a generic public registry or raw Transaction participant API.
ADR-0021 remains Proposed; this slice does not claim its full acceptance or P3 E3/E4.
ADR-0024/0022 remain Proposed; do not ratify architecture through a coding closure.

## 2. Test-first and first GREEN evidence

The initial **58-case** authority/failure suite preceded production implementation;
eight additional contract cases were added before the first focused GREEN66.
Actual first RED: `cargo test -p serea-storage --lib outcome_tests --offline`,
exit101, E0432/E0425/E0599 for missing intended outcome types and methods; no fake
assertion. Logged in `tmp/p2f-first-red.log`. First implementation compile run
also exposed incorrect enum accessor spelling; fixed to existing `wire_name()`.
First focused GREEN: same command, **66 passed**, zero failed/ignored, in
`tmp/p2f-focused-second-run.log`. Feature development froze immediately.

Initial test-writing delegation was unavailable due to usage limit; coordinator
wrote the tests. Subsequent three independent review sessions ran successfully.
Bounded remediation adds 17 tests: **83 P2F tests** total. The isolated first-write
suite executes the exact production success/failure statements without the
classification SELECT. It covers 13 authority/lifecycle cases for each statement;
an owner-only mutant control accepts stale same-owner authority while the actual
production statements reject it. No public raw SQL/test API was introduced.

## 3. Frozen independent reviews and bounded remediation

Three independent bounded read-only reviews ran on the same frozen first GREEN:
A SQL/concurrency; B transaction/security; C architecture/scope. No reviewer edited
files or Git. A independently exercised 19 extracted in-memory SQL predicate cases
(Python SQLite, not Rust bundled runtime); B/C performed static dependency/contract
review. No production defect was established. Coordinator verified all candidates
against source and frozen contracts before deduplication.

The immutable generation0 report is
`tmp/reviews/2026-10-04-code-review-report-p2fgreen0.md`, validated as **0 findings,
9 Minor test gaps, 10 coverage areas, Pass with caveat** before remediation.
Resolution and generation1 terminal re-review are separate artifacts, not rewrites.
The first-GREEN source snapshot is `tmp/p2f-first-green-snapshot`.

| Gap | Bounded resolution |
| --- | --- |
| T1 embedded predicate shadowed by classification | Exact production UPDATEs isolated from preclassification; owner-only mutant control |
| T2 cleanup/RELEASE error with active outer Tx | Test-private wrapper probe loses its savepoint but keeps outer Tx active; flag asserted; all P2F/blob operations refused; outer Ok rolls back |
| T3 caught method unwind | Panic inside method envelope; successful cleanup preserves unrelated writes; failed cleanup makes active Tx rollback-only |
| T4 reused/corrupt result | Existing corrupt row refuses after step UPDATE; committed and same-outer-Tx valid blobs survive later receipt failure |
| T5 attribution | Non-HOST actor, version, causation and payload digest persistence |
| T6 begin-specific late/zero-row composition | Step/task/journal IGNORE with unrelated before/after writes |
| T7 known failure additional paths | Late release refusal, actual deferred-FK COMMIT refusal, file-backed panic/reopen |
| T8 future receipt instant | Late refusal rolls back full outcome and preserves unrelated outer work |
| T9 ignored blob INSERT | Composite RESULT FK refuses outcome; method rolls back |

A's mixed VERIFY layout question and C's matching observation are reconciled with
F5 as a **supported-input limitation**: ordinary prefix then VERIFY suffix.
Arbitrary interleaved VERIFY/ordinary plans are not proved or accepted by this
slice; a regression records refusal without automatic scheduling. No planner or
universal layout-validation claim. No verifier means VERIFYING, not fabricated
COMPLETED. Existing larger historical P2F engine/deletion/query/participant plans
remain deferred and are not closed by this storage-only evidence.

Production remediation is limited to factoring the exact success/failure SQL for
isolated tests, cfg(test)-only private wrapper access, and a named SQL row type to
resolve Clippy type_complexity (initial lint failure retained). No authority/expiry
or lifecycle feature expansion. Remediation focused GREEN83 and complete storage
255 unit + 2 integration + 26 doctests passed. All three bounded terminal
re-reviews confirm their original items resolved and no new delta issues. A
independently proves FENCE byte identity, SQL whitespace equivalence and unchanged
production named bindings; B confirms unchanged real method envelope and active
cleanup/unwind/reuse matrix; C confirms original F1–F7 unchanged and protected
architecture bytes. The terminal generation1 report
`tmp/reviews/2026-10-04-code-review-report-p2fterminal1.md` links the immutable
initial report and `tmp/reviews/2026-10-04-p2f-resolution.md`; canonical validator
PASS: **0 findings, 0 test gaps, 8 affected coverage areas, Pass**. No unresolved
Blocker/Major/Minor, approval question or scoped test gap. Terminal reviewers did
not claim they reran Cargo; coordinator performed all final commands below.

## 4. Final validation and baseline preservation

Actual stable Rust **1.98.1**, MSRV **1.85.0**, macOS x86_64 host. Offline command
logs and per-command exit/duration JSON are ignored `tmp/p2f-final-*`. All final
production commands passed after the alias-only lint repair and terminal reviews.

| Command | Result |
| --- | --- |
| cargo fmt --all -- --check | PASS |
| cargo check --workspace --all-targets --all-features --offline | PASS |
| cargo test --workspace --all-targets --offline | PASS593 regular |
| cargo test --workspace --all-features --offline | PASS593 regular+31doctests=624 |
| cargo clippy --workspace --all-targets --all-features --offline -- -D warnings | PASS |
| cargo +1.85.0 check --workspace --all-targets --all-features --offline | PASS |
| cargo +1.85.0 test --workspace --all-targets --offline | PASS593 regular |
| cargo +1.85.0 test --workspace --all-features --offline | PASS593 regular+31doctests=624 |
| cargo +1.85.0 clippy --workspace --all-targets --all-features --offline -- -D warnings | PASS |
| python3 tests/workspace_smoke.py | PASS exact3 members and layering |
| python3 -m unittest discover -s tests -p workspace_smoke_tests.py | PASS37 |
| Python compile: doc validator and both workspace-smoke Python files | PASS |
| python3 tools/validate_docs.py docs | PASS60 Markdown files |
| git diff --check | PASS |
| cargo metadata --no-deps --format-version 1 --offline | PASS exact protocol/storage/testkit |

Focused `cargo [+1.85.0] test -p serea-storage [--release] --offline`:

| Mode | Unit | Integration | Doctests | Total | P2F regular |
| --- | --- | --- | --- | --- | --- |
| stable debug | 255 | 2 | 26 | 283 | 83 |
| stable release | 255 | 2 | 26 | 283 | 83 |
| Rust1.85 debug | 255 | 2 | 26 | 283 | 83 |
| Rust1.85 release | 255 | 2 | 26 | 283 | 83 |

Every mode includes all authority, cross-Store, competing outcome/reclaim,
reopen, actual COMMIT refusal, panic and review regressions. **Zero failures and
zero ignored executions** in every completed final test command. No test-mode
or stable/MSRV identity discrepancy.

Baseline execution-name Counter comparison (only doctest line numbers normalized):
**P2E510 regular+29doctests=539**, final **593+31=624**, new **83 regular+2doctests=85**;
**missing baseline regular0, missing baseline doctests0**. All-target regular
identities equal all-feature regular identities. Existing inventory and negative
Tx doctest names remain; obsolete missing-begin example now proves immutable Tx
cannot begin. No baseline test/doctest disappears or is ignored. Machine evidence:
`tmp/p2f-counts-and-scope.json` and final/baseline identities JSON.

All four validation batches completed inside their600-second tool bounds and
all child commands completed inside300 seconds: no test/build wrapper timeout.
A separate initial **scope-audit** invocation with `timeout_ms=10000` timed out
with no captured output while comparing protected files; rerun with120000 passed.
This is preserved as a wrapper timeout, not a test failure or erased evidence.
Initial RED and the observed pre-repair lint failure are likewise separate from
final test failures (zero).

Final scope audit verifies **165 protected files byte-identical** to starting HEAD
(manifests/lockfile/vendor/protocol/testkit/migrations/CI/smoke and blob/error logic).
0001 checksum remains
`sha256:d9068dccbc26ececb71be79c475080633166ba0163c62b2d98b9733512baefea`.
No0002, migration/catalog change, dependency, vendor, workspace or version change.
arch1/task2/action2/event1 remain unchanged. ADR-0021/0022/0024 remain Proposed.
Only current-slice annotations are reconciled; historical/future obligations and
P2E closure itself are not rewritten. Publication is followed by another
fmt/doc/smoke/metadata/diff/scope check before the single commit.

## 5. Closure and stop

**P2F CLOSED for the owner-directed atomic storage begin/known-outcome slice.**
Ready for the next separately authorized phase; no next phase is started here.
This proves current/stale/released/reclaimed/same-owner/invalid-origin outcome
fencing, current exact/past-expiry known-result acceptance, no-second-charge,
receipt/journal/result/task/lease method atomicity, outer rollback/actual COMMIT
refusal, consuming-error safety and file-backed two-Store/reopen authority.

It does **not** prove external effects exactly once, a provider/runtime, real I/O
ambiguous-commit rollback, crash durability under SIGKILL, production PRIVATE
ordinary rows, arbitrary interleaved verification plans, full TaskEngine lifecycle,
creation/planning/queries/deletion, waiting/cancellation/reconciliation writers,
full ADR-0021 participant composition, scheduler/recovery or P3 E3/E4. Physical
SQLite durability follows the existing WAL/FULL Store contract, not new hardware
or child-process evidence. Outcome expiry semantics are intentionally different
from begin/renew, not a P2E redesign.

### Changed-file inventory and commit discipline

14 P2F-authorized paths:

- `crates/serea-storage/src/{outcome,outcome_tests,outcome_review_tests}.rs`;
- `crates/serea-storage/src/{lease,lib,store,tx,blob_tests}.rs` (wiring, scoped
  documentation, compile-fail and method-inventory reconciliation only);
- `docs/plans/{P2F-review-and-closure,P2-storage-task-engine,P2-test-matrix,P2-contract-gap-analysis,P2-tomorrow-decision-ledger}.md`;
- `docs/decisions/ADR-0024-lease-fencing-and-commit-under-lease.md` (implementation
  evidence annotations only; Proposed and expiry contract retained).

One `feat: implement P2F atomic outcome fencing` commit after all gates and final
diff audit, exact P2E parent, no amend or push. Its exact SHA and clean-worktree/
branch confirmation are reported after commit, not embedded self-referentially.
Dependencies added0; vendor changes0; migration changes0; new members0;
protocol-version changes0. Scheduler created **NO**; worker loop created **NO**;
recovery started **NO**; P3 started **NO**; next phase started **NO**. STOP.
