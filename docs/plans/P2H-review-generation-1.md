# P2H Crash/Fault Injection — Terminal Generation 1 Review

## Report Contract

- Report type: `code-review`
- Report ID: `cr-20261006-p2hcrash1`
- Review chain ID: `rc-20261006-p2hcrash`
- Review generation: `1`
- Review trigger: `post-implementation`
- Parent review report ID: `cr-20261006-p2hcrash0`
- Parent review report path: `docs/plans/P2H-review-generation-0.md`
- Parent resolution ID: `rr-20261006-p2hcrash`
- Parent resolution path: `docs/plans/P2H-review-resolution.md`
- Generated at: `2026-10-06T00:00:00Z` (report date only)
- Report path: `docs/plans/P2H-review-generation-1.md`
- Source skill: `code-review`
- Status: `Review complete`
- Git mutation during review: `None`
- Scope fingerprint: `Unavailable - source tree was reviewed in the uncommitted working tree and no immutable target snapshot was retained`

This terminal review is limited to the generation-0 remediation and the affected crash-verifier, child-process, fixture, release-artifact and N7 execution chains. No P2I or P3 scope is included.

## Scope

- Review date: `2026-10-06`
- Scope kind: `working tree`
- Scope description: Current P2H implementation against P2H generation-0 findings and resolution: seam isolation, crash windows, fresh verifier, N7, fixture identity, child lifecycle and release proof.
- Scope mode: `implementation delta plus affected execution chains`
- Baseline: `6fe6bd3e6d60924257d9d222fcf01ae000d841b9` plus frozen initial report/resolution
- Target: current `p2/p2h-crash-fault-injection` working tree
- Changed paths: 11 implementation/test/tool files plus four P2H review/closure records
- Completion: `Complete within reviewed scope`
- Requirements consulted: generation-0 report and resolution; `P2-test-matrix.md` §3; P2 storage/engine and release-isolation requirements.
- Prior resolution consulted: `rr-20261006-p2hcrash; docs/plans/P2H-review-resolution.md`
- Assumptions: default stable/MSRV production configuration is the production release configuration; the explicit opt-in fault feature is reserved to the engine dev-dependency/test target. An arbitrary downstream release that explicitly enables this optional feature is outside the default-release proof and is not claimed seam-free.
- Excluded as unrelated: P2I Group O and all P3 design/implementation.

## Review Orchestration

- Assessment subagent: `Coordinator assessment - bounded review of three independent affected chains: durability, security/build isolation, and process/test reliability`
- Orchestration decision: `Parallel specialists`
- Decision confidence: `high`
- Decision rationale: durability claims, feature/build isolation, and process cleanup have independent evidence paths and require cross-checking the same small harness.
- Coordinator override: `None`
- Context or tool limits: Reviewers performed static review; coordinator ran the validation matrix. No Apple Silicon or non-macOS host test was performed. Review C does not fault-inject OS kill/wait syscall failures.

### Risk Dimensions

- N4/N5/N6 atomicity and the fact that only a different fresh process may establish final durable truth.
- No production/default runtime fault activation and no hidden runtime subprocess/environment route.
- Bounded child cleanup, exact inherited path, N7 honest stress-only scope and repeatability.

### Reviewer Assignments

| Reviewer | Angle | Owned surfaces | Mandatory cross-checks | Status |
| --- | --- | --- | --- | --- |
| R1 | Crash/durability semantics | `crash.rs` verifier, N1–N8, N6 recovery | complete task/result/ref/blob/receipt/journal/lease state; no false crash claim | Complete; initial G1 candidate followed by static PASS after fix |
| R2 | Security/scope | cfg gates, Cargo manifests, runtime source guards, release proof | default production artifact exclusion, exact test executable positive control, no unsafe/dependency/schema drift | Complete, PASS with explicit feature-config limitation |
| R3 | Concurrency/portability/test quality | process harness, F25/F26, N7 and waits | bounded failure paths, child cleanup, per-task assertion, cwd and path | Complete, PASS after fixes |

### Synthesis Statement

The coordinator independently rechecked the accepted G1 candidate against current N6 fixture topology and outcome semantics. The final verifier pins nonterminal `VERIFYING`, terminal `COMPLETED`, exact terminal two-step cardinality, and result digest/reference/blob joins including data-class rank. R2’s caveat is retained: the proof establishes default production release configurations; an intentionally explicit feature-enabled build is a test/development configuration, not claimed production-clean. No unresolved Blocker/Major/Minor finding, question or review-area gap remains in the scoped default-build contract.

## Review Snapshot

- Recommendation: `Pass`
- Completion: `Complete within reviewed scope`
- Why now: all accepted generation-0 findings and the generation-1 N6 verifier candidate are fixed; current tests and default artifact proof pass.
- Must-review now: `None`
- Findings count: `Blocker 0 | Major 0 | Minor 0 | Question 0`
- Standalone test gaps: `Blocker 0 | Major 0 | Minor 0`
- Coverage confidence: `high` within the scoped macOS/default-release configuration
- Biggest blind spot: explicit downstream opt-in to `p2h-fault-injection` and non-macOS/Apple-Silicon execution are not asserted to be production-clean/empirically validated.

## Complete Findings Index

No code-review findings remain in the terminal reviewed scope.

## Blocker

None.

## Major

None.

## Minor

None.

## Questions

None.

## Test Gaps

None within the scoped contract. Explicit-feature downstream release configurations and other host architectures remain documented nonclaims, not silent coverage.

## Review Coverage Ledger

| Area ID | Area / path | Touched files or entry points | Owner | Depth | Status | Result | Evidence / next step |
| --- | --- | --- | --- | --- | --- | --- | --- |
| A1 | Storage fault module, cfg declaration and reach sites | fault module and cfg sites | R2 | contract trace | Reviewed - no issue found | feature gate; no Store callback; unsafe forbidden | source and manifest inspection |
| A2 | Cargo feature/dev edge | storage/engine manifests | R2 | dependency trace | Reviewed - no issue found | only dev edge requests feature; default production artifacts clean | Cargo metadata and release proof |
| A3 | Release proof A–D | proof script and artifact paths | R2 | runtime verified | Reviewed - no issue found | exact test executable and D artifacts checked | proof script exit 0 |
| A4 | N1–N3 verifier | mode verifier branches | R1 | runtime verified | Reviewed - no issue found | signal-killed children; absent rows/schema/integrity asserted | fresh verifier process tests |
| A5 | N4/N5 rollback | outcome rollback assertions | R1 | runtime verified | Reviewed - no issue found | conservative step/task/receipt/ref/blob/journal/lease state asserted | fresh verifier process tests |
| A6 | N6/N6a/N6b | post-COMMIT verifier and recovery tests | R1 | runtime verified | Reviewed - no issue found | complete aggregates and joined digest/ref/blob | N6 verifier plus strict terminal logical dump |
| A7 | N7 stress | writer/controller/verifier | R1/R3 | runtime verified | Reviewed - no issue found | signal, integrity/FK and per-task audit checked; no exact count claim | repeated focused suite |
| A8 | N8/audit/savepoint fault tests | fault rollback cases | R1/R2 | runtime verified | Reviewed - no issue found | explicitly fault-injection rollback, not crash evidence | focused suite |
| A9 | F25/F26 identity/path | temp fixtures and child path | R3 | runtime verified | Reviewed - no issue found | separate executable identity; canonical inherited path and alternate cwd | focused suite |
| A10 | Wait/kill/reap and safety | bounded process helpers | R3 | contract trace | Reviewed - no issue found | bounded wait, failure kill/reap, no sleep authority | source trace and tests |
| A11 | Migration/protocol/dependency/runtime scope | migration/Cargo/protocol/runtime files | Coordinator/R2 | dependency trace | Reviewed - no issue found | no migration/vendor/lock/wire/runtime dependency changes | git diff and cargo metadata |

## Subagent Candidate Adjudication

| Candidate | Proposed by | Decision | Final ID | Coordinator evidence | Reason |
| --- | --- | --- | --- | --- | --- |
| N6 nonterminal aggregate/result-association omission | R1 | fixed then dismissed as resolved | None | current verifier pins VERIFYING and joins result/ref/blob on digest+class | targeted gap no longer exists |
| N6 terminal extra succeeded row could escape join coverage | R1 follow-up | fixed then dismissed as resolved | None | exactly two task_steps, zero non-succeeded, two joined results | join covers full selected terminal fixture |
| Release script could accept B artifact for C | R2/R3 | dismissed as resolved | None | Cargo JSON executable field selects exact target named `crash` | no stale artifact substitution |
| Default workspace D could accept earlier A artifact | R2 | dismissed as resolved | None | cargo build JSON reports each exact production artifact inspected | D-bound verification |
| N7 mid-COMMIT timing | R1/R3 | dismissed as defect; retained limitation | None | code makes no deterministic claim and pins no exact counts | matches matrix §3.1 |
| Explicitly enabled feature release could include seam | R2 | caveat retained, not an unresolved defect under selected normal-build contract | None | optional feature is opt-in through dev dependency; A/D prove default release only | explicitly disclosed and not overclaimed |

## Evidence Appendix

### Verification Commands

- `cargo test -p serea-task-engine --test crash --offline` -> **21 passed, 0 failed, 0 ignored**, repeated after final assertions.
- `cargo +1.85.0 test -p serea-task-engine --test crash --offline` -> **21 passed, 0 failed, 0 ignored**, after final assertions.
- `cargo test --workspace --all-targets --offline` and `cargo +1.85.0 test --workspace --all-targets --offline` -> pass; final workspace baseline **822 regular**.
- `cargo test --workspace --all-features --offline` and `cargo +1.85.0 test --workspace --all-features --offline` -> pass; **822 regular + 45 doctests = 867**, zero failures/ignored.
- Stable/MSRV all-target/all-feature `check` and `clippy -D warnings` -> pass.
- Stable/MSRV storage and engine debug/release all-feature tests -> pass.
- `bash tools/prove_release_fault_exclusion.sh /tmp/p2h-relproof-final` -> exit 0; exact crash test executable contains seam; Cargo-reported default workspace production storage/engine artifacts clean.
- `cargo fmt --all -- --check`, smoke, 59 Python unittests, docs validator, offline metadata and `git diff --check` -> pass.

### Dismissed Coordinator Candidates

| Candidate | Decision | Evidence |
| --- | --- | --- |
| N7 does not prove a particular in-flight COMMIT was killed | retained as explicit stress limitation | matrix §3.1 and test comments prohibit deterministic claim |
| Default release exclusion means every explicit feature build is clean | dismissed as overbroad claim | report limits itself to default production builds; explicit feature build is the positive control |

### Blind Spots

| Area ID | Blind spot | Decision risk | What resolves it |
| --- | --- | --- | --- |
| A12 | Non-macOS and Apple Silicon not tested | no cross-host empirical portability claim | run same suite on the target host when available |
| A13 | Explicit release configuration opting into test feature is not claimed safe | downstream could deliberately activate the public optional feature | keep feature off in release manifests/build commands; if threat model requires universal exclusion, redesign feature delivery before production distribution |

## Prior Resolution Reconciliation

| Parent issue | Parent disposition | Current evidence | Decision |
| --- | --- | --- | --- |
| F1 fresh verifier completeness | Accepted | `crash.rs` per-mode verifier assertions and final N6 joins | kept closed |
| F2 N7 stress claim | Accepted | timing-dependent comment, signal check, no exact count | kept closed |
| F3 child waits/cleanup | Accepted | bounded helper and kill/reap failure paths | kept closed |
| F4 release proof target/vacuity | Accepted | exact cargo JSON executable/artifact selection | kept closed |
| F5 unsafe prohibition | Accepted | `#![forbid(unsafe_code)]` | kept closed |
| T1 result blob rollback | Accepted | N4/N5/N6 blob-row checks | kept closed |
| T2 N6 journal baseline | Accepted | before/after count around recovery | kept closed |
| T3 F26 cwd/path | Accepted | canonical temp root, unique child cwd, inherited exact path | kept closed |
| T4 N7 per-task audit | Accepted | grouped exact-one insertion audit query | kept closed |
| G1 N6 aggregate/result linkage | New candidate in generation 1; remediated before terminal report | VERIFYING/COMPLETED, exact terminal cardinality and digest/ref/blob joins | resolved; no reopening |

## Receiving Handoff

- Handoff status: `Terminal post-review - return to user/owner`
- Automatic receiving permitted: `No`
- Source report ID: `cr-20261006-p2hcrash1`
- Scope fingerprint to recheck: `Unavailable - source tree was reviewed in the uncommitted working tree and no immutable target snapshot was retained`
- Actionable finding IDs: `None`
- Deferred finding IDs: `None`
- Actionable test-gap IDs: `None`
- Deferred test-gap IDs: `None`
- Open question IDs: `None`
- Open coverage area IDs: `None`
- Highest-risk verification to repeat: `full stable/MSRV suite plus release exclusion`
- Suggested implementation boundaries: `None`
- Re-review note: `Generation 1 is terminal; later changes require a new generation-0 chain.`
- Chain rule: `No automatic receiving-code-review after terminal report.`

## Report Self-Check

- Actual assessment mode/rationale recorded: yes.
- Every changed review-relevant area has a coverage row: yes.
- No unresolved finding/test-gap/question: yes.
- Parent settlements reconciled: yes.
- Explicit-feature and host limitations are disclosed: yes.
- Reviewers made no file/Git changes: yes.
- Report validator: pending validation with generation-0 and resolution parents.
