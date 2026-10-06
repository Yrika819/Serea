# P2H Crash/Fault Injection — Frozen Generation 0 Review

## Report Contract

- Report type: `code-review`
- Report ID: `cr-20261006-p2hcrash0`
- Review chain ID: `rc-20261006-p2hcrash`
- Review generation: `0`
- Review trigger: `initial`
- Parent review report ID: `None`
- Parent review report path: `None`
- Parent resolution ID: `None`
- Parent resolution path: `None`
- Generated at: `2026-10-06T00:00:00Z` (editor/report date only)
- Report path: `docs/plans/P2H-review-generation-0.md`
- Source skill: `code-review`
- Status: `Review complete`
- Git mutation during review: `None`
- Scope fingerprint: `Unavailable - the initial uncommitted review snapshot was not retained as a separate object; baseline and changed paths are recorded below`

This is the frozen initial review record. Its findings describe the pre-remediation P2H implementation, not the current tree. The user-authorized implementation continued after this initial review. The actual current-code disposition and validation are recorded separately in the resolution and generation-1 report.

## Scope

- Review date: `2026-10-06`
- Scope kind: `working tree`
- Scope description: P2H test-only storage seam, child-process crash harness, fresh verifier, N1–N8, F25/F26, and release-exclusion proof script, versus exact P2G HEAD.
- Scope mode: `full frozen scope`
- Baseline: `f50fd8f0aa01ae8847c92506a61015fa586a69ec`
- Target: initial P2H working tree on `p2/p2h-crash-fault-injection` (uncommitted snapshot)
- Changed paths: 11 implementation/test/tool paths; no migration, vendor, lockfile or protocol changes
- Diff size: 75 tracked insertions / 5 tracked deletions, plus `fault.rs`, `crash.rs` and release proof script at initial review; subsequent bounded fixes are documented in resolution
- Completion: `Complete within reviewed scope`
- Requirements consulted: `docs/plans/P2-storage-task-engine.md` §15.8; `docs/plans/P2-test-matrix.md` §3, F25/F26 and N1–N8; P2G closure and current P2F storage/task-engine transaction contracts.
- Prior resolution consulted: `None`
- Assumptions: N7 is stress-only; no deterministic mid-COMMIT seam exists through the current rusqlite layer; P2H may add only test support and no new production behavior.
- Excluded as unrelated: P2I Group O, P3, migrations, production runtime features, model/provider/scheduler/event behavior.

## Review Orchestration

- Assessment subagent: `Coordinator assessment - separate durability, security/build-isolation, and harness-concurrency risk dimensions`
- Orchestration decision: `Parallel specialists`
- Decision confidence: `high`
- Decision rationale: real process death, production feature isolation and CI child lifecycle are independent high-risk dimensions and merit separate reviewers.
- Coordinator override: `None`
- Context or tool limits: Review A/B summaries were carried forward from the prior session; Review C returned a detailed independent static review. Initial snapshot bytes were not retained. Later test evidence is not substituted for initial review findings.

### Risk Dimensions

- SQLite transaction/crash boundaries and fresh-process durable truth.
- Fault-seam activation, production artifact exclusion, subprocess and environment isolation.
- Bounded waits, process cleanup, fixture identity, path portability and stress-test reliability.

### Reviewer Assignments

| Reviewer | Angle | Owned surfaces | Mandatory cross-checks | Status |
| --- | --- | --- | --- | --- |
| R1 | Crash/durability | N1–N8 verifier assertions and N6 recovery | real signal death, independent verifier, allowed durable states | Complete (summary and code evidence retained) |
| R2 | Security/scope | Cargo feature/dependency, cfg gates, production runtime, release proof | no public default production activation, no env/subprocess runtime path, unsafe | Complete (summary and code evidence retained) |
| R3 | Concurrency/portability | child harness, N7, F25/F26, waits and cleanup | bounded failure paths, no sleep authority, absolute/inherited path, MSRV scope | Complete (independent read-only static review) |

### Synthesis Statement

The coordinator rechecked all reported candidates against the frozen test matrix and source. Initial release-proof and lifecycle claims were not accepted solely from reviewer assertion; the script and harness were subsequently corrected and exercised. No schema or production transaction behavior was changed. The initial reports were not immutable machine snapshots; this provenance limitation is disclosed rather than claimed away.

## Review Snapshot

- Recommendation: `Changes requested`
- Completion: `Complete within reviewed scope`
- Why now: initial implementation contained incomplete durable assertions and unreliable failure-path cleanup/proof checks.
- Must-review now: `F1, F2, F3, F4`
- Findings count: `Blocker 0 | Major 4 | Minor 1 | Question 0`
- Standalone test gaps: `Blocker 0 | Major 1 | Minor 3`
- Coverage confidence: `medium`
- Biggest blind spot: exact original snapshot fingerprint and full raw R1/R2 review transcripts were not preserved across compaction.

## Complete Findings Index

| ID | Severity | Surface | Review risk | Confidence | Origin | Verification | Issue key | Issue fingerprint | Expected basis |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| F1 | Major | Fresh crash verifier | A crash-window verifier may accept incomplete committed or rolled-back truth | high | R1 | source/contract trace | `behavior; entry=fresh crash verifier; contract=assert window-specific durable transaction rows in independent process; effect=partial or absent committed state goes undetected` | `ifp-sha256:91d92bbcf792820dbc8241664ee9987e9b6fc15a9304220670d5182386a184b1` | `kind:hard-invariant; strength:authoritative; evidence:P2-test-matrix §3 N1–N6` |
| F2 | Major | N7 stress assertion | Timing-dependent stress evidence risks being represented as deterministic mid-COMMIT proof | high | R1 | matrix/source comparison | `behavior; entry=N7 stress; contract=report only observed crash/integrity guarantees; effect=timing-dependent sample overclaims exact commit durability` | `ifp-sha256:4214a0ec888039fd6375aa903b0e4596507a822b93cbaf5ccec0b72eb7b6f536` | `kind:approved-design; strength:authoritative; evidence:P2-test-matrix §3.1` |
| F3 | Major | Child process lifecycle | A bounded acknowledgement timeout can turn into an unbounded child wait or leak | high | R3 | source trace | `behavior; entry=child harness timeout; contract=every subprocess wait is bounded and child reaped on failure; effect=broken test child hangs or leaks CI` | `ifp-sha256:c6b39d0892c89535c0651b3eb0bd3a0bb451e7e140e169cf43242b78fc0100de` | `kind:owner-decision; strength:authoritative; evidence:user P2H §6 and §18` |
| F4 | Major | Release exclusion proof | The all-target check does not establish the engine test feature path or non-vacuous production exclusion | high | R2/R3 | script and manifest trace | `behavior; entry=release exclusion proof; contract=production artifacts exclude fault seam and test artifact proves feature path; effect=proof passes without evaluating correct separation` | `ifp-sha256:2e549ef1797c691d6d030245c55bfda277fc6079fdeebcbabe4452966ee82334` | `kind:owner-decision; strength:authoritative; evidence:user P2H §5 and §18` |
| F5 | Minor | Fault seam source | Compile-time unsafe prohibition was absent as defense-in-depth | medium | R2 | crate source inspection | `behavior; entry=release fault test code; contract=fault path contains no unsafe code; effect=unsafe fault seam can invalidate safety proof` | `ifp-sha256:effac036eb4d588d421fc1e223209375ec30ca98a5130f5b57796c04607d41ac` | `kind:requirement; strength:authoritative; evidence:user P2H requirement that seam uses no unsafe code` |

## Blocker

None.

## Major

### F1 Major - Fresh verifier does not fully pin every durable window

Impact: The verifier must establish transaction truth, not merely that SQLite opens. Initial review found incomplete row-set assertions, including result blob absence/presence around N4/N5/N6 and durable expectations delegated to coordinator-side checks.

Issue key: `behavior; entry=fresh crash verifier; contract=assert window-specific durable transaction rows in independent process; effect=partial or absent committed state goes undetected`
Issue fingerprint: `ifp-sha256:91d92bbcf792820dbc8241664ee9987e9b6fc15a9304220670d5182386a184b1`
Expected basis: `kind:hard-invariant; strength:authoritative; evidence:P2-test-matrix §3 N1–N6`
Confidence: high. Origin: R1. Coordinator verification: compared each mode against the matrix and inspected verifier placement.

Look here first: `crates/serea-task-engine/tests/crash.rs` in `verify_durable`.

Expected: the fresh verifier asserts every selected durable row set. Current at initial review: integrity checks alone or incomplete assertions could permit a transaction-row discrepancy to pass.

Reviewer action: request complete verifier assertions.

### F2 Major - N7 stress claim must remain weak

Impact: A sibling SIGKILL during repeated transactions cannot establish that a particular kill landed inside SQLite COMMIT. No exact count or requirement to observe both categories is permitted.

Issue key: `behavior; entry=N7 stress; contract=report only observed crash/integrity guarantees; effect=timing-dependent sample overclaims exact commit durability`
Issue fingerprint: `ifp-sha256:4214a0ec888039fd6375aa903b0e4596507a822b93cbaf5ccec0b72eb7b6f536`
Expected basis: `kind:approved-design; strength:authoritative; evidence:P2-test-matrix §3.1`
Confidence: high. Origin: R1. Coordinator verification: matched stated evidence to actual child controller and durable assertions.

Look here first: `crates/serea-task-engine/tests/crash.rs` N7 block.

Expected: report only process signal death, database openability, quick_check, foreign_key_check and truthful observed counts. Current at initial review: prose/assertions could be read as stronger per-sample durability proof than measured.

Reviewer action: request claim narrowing and signal proof.

### F3 Major - Child failure paths can wait indefinitely or leak the writer

Impact: A missing ack followed by `wait_with_output` can block forever if the child hangs; N7 assertion unwinding can leave a writer running. The success path was bounded, but the failure path was not.

Issue key: `behavior; entry=child harness timeout; contract=every subprocess wait is bounded and child reaped on failure; effect=broken test child hangs or leaks CI`
Issue fingerprint: `ifp-sha256:c6b39d0892c89535c0651b3eb0bd3a0bb451e7e140e169cf43242b78fc0100de`
Expected basis: `kind:owner-decision; strength:authoritative; evidence:user P2H §6 and §18`
Confidence: high. Origin: R3. Coordinator verification: traced acknowledgement timeout, verifier waits and N7 progress failure.

Look here first: `crates/serea-task-engine/tests/crash.rs` child waits and N7 controller.

Expected: all waits bounded and active children killed/reaped on failure. Current at initial review: at least one failure branch could become unbounded.

Reviewer action: request kill/reap on bounded timeout.

### F4 Major - Release exclusion proof is not tied to the required test target

Impact: A storage-only `--all-targets` build does not traverse the engine dev-dependency feature edge, and a no-op `case`/ambiguous artifact scan does not prove the intended separation. The initial interrupted script run was not evidence.

Issue key: `behavior; entry=release exclusion proof; contract=production artifacts exclude fault seam and test artifact proves feature path; effect=proof passes without evaluating correct separation`
Issue fingerprint: `ifp-sha256:2e549ef1797c691d6d030245c55bfda277fc6079fdeebcbabe4452966ee82334`
Expected basis: `kind:owner-decision; strength:authoritative; evidence:user P2H §5 and §18`
Confidence: high. Origin: R2/R3. Coordinator verification: compared package/target arguments and feature declaration.

Look here first: `tools/prove_release_fault_exclusion.sh` and `crates/serea-task-engine/Cargo.toml`.

Expected: positive test-target evidence plus clean artifacts from actual default production builds. Current at initial review: the artifact selection/build target did not prove those facts.

Reviewer action: request target-bound, non-vacuous proof.

## Minor

### F5 Minor - Unsafe prohibition was not compile-time pinned

A source-level review found no unsafe hook, but a crate-wide deny-by-default guard is stronger than a reviewer assertion for a concurrency-sensitive test seam.

Issue key: `behavior; entry=release fault test code; contract=fault path contains no unsafe code; effect=unsafe fault seam can invalidate safety proof`
Issue fingerprint: `ifp-sha256:effac036eb4d588d421fc1e223209375ec30ca98a5130f5b57796c04607d41ac`
Expected basis: `kind:requirement; strength:authoritative; evidence:user P2H requirement that seam uses no unsafe code`
Confidence: medium. Origin: R2. Coordinator verification: checked the storage crate root.

Look here first: `crates/serea-storage/src/lib.rs`.

Expected: compile-time unsafe prohibition. Current at initial review: the crate had none. Reviewer action: add `#![forbid(unsafe_code)]`.

## Questions

None.

## Test Gaps

| ID | Severity | Surface | Missing coverage | Risk | Origin | Evidence | Issue key | Issue fingerprint | Expected basis |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| T1 | Minor | N4/N5 | Assert the result blob row itself is absent before commit/death | partial durable write could escape a row-level check | R1 | verifier SQL omitted blob row | `test-gap; entry=N4 and N5; contract=all transaction result blob writes are absent before commit and rolled back on death; gap=blob row state is not checked` | `ifp-sha256:dfb79ac05bbbc006873a92b8dfc714cd5ee1cdf94e73f106f264cfe9fb675474` | `kind:hard-invariant; strength:authoritative; evidence:P2-test-matrix N4/N5` |
| T2 | Minor | N6 nonterminal recovery | Capture non-audit journal count before recovery | a self-comparison after recovery cannot detect repeated outcome rows | R1 | count initially taken after operation | `test-gap; entry=N6 nonterminal recovery; contract=recovery does not rewrite P2F outcome journal; gap=before/after outcome journal counts are compared at same post-recovery time` | `ifp-sha256:730c50a7d33a84334cb0eeeac06605b99020946c78542d763a1be2f2e5e705d0` | `kind:hard-invariant; strength:authoritative; evidence:P2-test-matrix N6b` |
| T3 | Minor | F26 | Guarantee an absolute inherited path and a distinct child cwd for every valid runner cwd | path resolution test may not exercise cwd independence | R3 | workspace root could equal test runner cwd; temp root might be relative | `test-gap; entry=inherited child path; contract=child reopens inherited absolute fixture independent cwd; gap=child cwd may equal parent or inherited path relative` | `ifp-sha256:ac0a042867a797d9af11273aeb31c54444fbcebb88b0d96cba4dc8ac888a7339` | `kind:owner-decision; strength:authoritative; evidence:user F26 path/cwd requirements` |
| T4 | Minor | N7 per-task audit | Check exactly one insertion audit per durable task, not only matching totals | a missing+duplicate pair can cancel out in aggregate count | R3 | initial aggregate count assertion | `test-gap; entry=N7 audit consistency; contract=every durable task has exactly one insertion journal; gap=aggregate counts mask missing or duplicate pairs` | `ifp-sha256:903be6de6edb815f7a44f5caf3b63adef2fec1c77a0f82271d77e18c15c5f830` | `kind:hard-invariant; strength:authoritative; evidence:P2-test-matrix N7 atomic transaction contract` |

## Review Coverage Ledger

| Area ID | Area / path | Touched files or entry points | Owner | Depth | Status | Result | Evidence / next step |
| --- | --- | --- | --- | --- | --- | --- | --- |
| A1 | Storage fault module, feature and reach points | `fault.rs`, `lib.rs` | R2 | contract trace | Finding F5 | unsafe prohibition absent initially | crate-root inspection |
| A2 | Store/Tx/audit/outcome hooks | storage transaction modules | R1/R2 | contract trace | Finding F1 | named-stage verifier assertions incomplete | per-window matrix trace |
| A3 | Child role parser and spawn/ack/kill paths | `tests/crash.rs` | R3 | contract trace | Finding F3 | failure path could wait or leak | child waits and N7 failure path |
| A4 | Fresh verifier for N1–N6/N6b | `verify_durable` | R1 | contract trace | Finding F1 | T1/T2: row and journal assertions incomplete | verifier SQL and recovery test |
| A5 | N7 stress/controller | N7 writer/controller | R1/R3 | contract trace | Finding F2 | T4: stress language/aggregate assertion too weak | N7 code vs matrix §3.1 |
| A6 | TempStore fixture identity/F25 | fixture derivation/probe | R3 | runtime verified | Reviewed - no issue found | binary identity, pid and counter present | live distinct executables and equal-component derivation |
| A7 | F26 inherited fixture | child path/cwd | R3 | contract trace | Reviewed - no issue found | T3: path/cwd test gap recorded separately | F26 fixture and child args |
| A8 | Release proof/manifests | Cargo manifests and proof script | R2/R3 | contract trace | Finding F4 | initial check did not bind production/test artifact evidence | manifest edge and proof script |
| A9 | Migration/dependency/protocol scope | migration/Cargo/protocol files | Coordinator | diff-only | Reviewed - no issue found | no migration/schema/protocol/dependency lock changes | changed-file inventory |
| A10 | Runtime subprocess/environment closure | production runtime source | R2 | contract trace | Reviewed - no issue found | harness is test-only; runtime closure has no such path | runtime source guard inspection |

## Subagent Candidate Adjudication

| Candidate | Proposed by | Decision | Final ID | Coordinator evidence | Reason |
| --- | --- | --- | --- | --- | --- |
| Verifier durability completeness | R1 | accepted | F1/T1 | source-to-matrix check | independent verifier must prove durable truth |
| N7 claim strength | R1 | accepted | F2 | N7 contract and source | no deterministic in-COMMIT observation |
| Child cleanup / unbounded waits | R3 | accepted | F3 | exact child wait paths | CI must fail boundedly and clean up |
| Release proof target/vacuity | R2/R3 | accepted | F4 | Cargo metadata, build targets, script | original check did not cover the requested path |
| F26 cwd/path uncertainty | R3 | accepted | T3 | fixture creation and cwd choice | avoid environment-dependent vacuity |
| N7 total counts do not prove per-task cardinality | R3 | accepted | T4 | grouped SQL semantics | missing/duplicate pair could cancel |
| Missing unsafe prohibition | R2 | accepted | F5 | crate root | inexpensive defense in depth |

## Evidence Appendix

### Diff Inventory

| Area | Classification | Review surface |
| --- | --- | --- |
| `crates/serea-storage/src/fault.rs` and cfg reach points | surface/test-only | fault actions, synchronization and transaction windows |
| `crates/serea-storage/Cargo.toml`, `crates/serea-task-engine/Cargo.toml` | config | test-only feature edge/dependency graph |
| `crates/serea-task-engine/tests/crash.rs` | test-only | N1–N8, F25/F26, child lifecycle and fresh verifier |
| `tools/prove_release_fault_exclusion.sh` | test/tool | default/test artifact assertions |
| storage lib root | surface | unsafe guard and conditional export |

### Verification Commands

- Initial P2H focused suite: 21 tests passed on the stable toolchain before reviews; later current-tree results are in `P2H-review-resolution.md`.
- Initial release proof was interrupted; it is not counted as evidence. Current exact rerun result is recorded in the resolution.
- Generation-0 R3 review was static. It did not independently execute tests or builds.

### Supporting Links

- [Crash harness](../../crates/serea-task-engine/tests/crash.rs)
- [Fault seam](../../crates/serea-storage/src/fault.rs)
- [Release proof](../../tools/prove_release_fault_exclusion.sh)
- [Crash contracts](P2-test-matrix.md#3-crash-and-fault-injection)

### Blind Spots

| Area | Blind spot | Decision risk | Resolution |
| --- | --- | --- | --- |
| Initial snapshot | Raw R1/R2 transcripts and exact content hash not retained across compaction | provenance is weaker than a newly captured immutable artifact | disclose in closure; fresh bounded reviews and current source evidence are retained |
| Host portability | Only current macOS host tested | cannot claim x86_64 or Apple Silicon portability empirically | explicitly scope claim to current host |

## Prior Resolution Reconciliation

None - initial review generation.

## Receiving Handoff

- Handoff status: `Ready for receiving-code-review`
- Automatic receiving permitted: `Yes`
- Source report ID: `cr-20261006-p2hcrash0`
- Scope fingerprint to recheck: `Unavailable - the initial uncommitted review snapshot was not retained as a separate object; baseline and changed paths are recorded below`
- Actionable finding IDs: `F1, F2, F3, F4, F5`
- Deferred finding IDs: `None`
- Actionable test-gap IDs: `T1, T2, T3, T4`
- Deferred test-gap IDs: `None`
- Open question IDs: `None`
- Open coverage area IDs: `None`
- Highest-risk verification to repeat: `N4/N5/N6 fresh-process row assertions and release-exclusion proof`
- Suggested implementation boundaries: `test harness/assertions, feature-isolation script and crate unsafe lint only`
- Re-review note: `Treat each initial finding as a claim, recheck against current code, and record any new generation-1 finding separately.`
- Chain rule: `Generation 1 is terminal; unresolved issues return to the owner.`

## Report Self-Check

- Assessment mode and rationale recorded: yes.
- Changed review areas covered: yes; initial snapshot provenance caveat stated.
- Findings and test gaps have stable IDs, canonical keys, fingerprints and expected bases: yes.
- No production code was edited during review: yes.
- Validator status: pending report-chain validation during P2H closure.
