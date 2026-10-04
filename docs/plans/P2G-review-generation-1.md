# P2G Terminal Implementation Review — Generation 1

## Report Contract
- Report type: `code-review`
- Report ID: `cr-20261005-p2grecovery1`
- Review chain ID: `rc-20261005-p2grecovery`
- Review generation: `1`
- Review trigger: `post-implementation`
- Parent review report ID: `cr-20261005-p2grecovery0`
- Parent review report path: `docs/plans/P2G-review-generation-0.md`
- Parent resolution ID: `rr-20261005-p2grecovery`
- Parent resolution path: `docs/plans/P2G-review-resolution.md`
- Generated at: `2026-10-05T00:00:00Z`
- Report path: `docs/plans/P2G-review-generation-1.md`
- Source skill: `code-review`
- Status: `Review complete`
- Git mutation during review: `None`
- Scope fingerprint: `sha256:0d6b12a8ec8a9aca7a8d8fb801f933867726e7093e2758901e6e4e0fe5afc7c9`

This is a terminal immutable report. Editor date is not ambient runtime time evidence. The scope fingerprint is length-framed sorted relative paths and contents of the two reviewed crate trees. No further production remediation is authorized automatically by this report.

## Scope
- Review date: `2026-10-05`
- Scope kind: `working tree`
- Scope description: Generation-0 first-GREEN snapshot to final remedied storage/engine recovery delta, affected authority/classification/audit/tests, and six P2G plan/Proposed ADR annotations.
- Scope mode: `implementation delta plus affected execution chains`
- Baseline: `first-GREEN snapshot tmp/p2g-first-green-snapshot; original HEAD 7c05ffae3503115a069761d40201aaf84595679a`
- Target: `working tree`
- Changed paths: `12`
- Diff size: `Restricted capability, bounded semantic/predicate fixes and focused regressions; six scoped documentation corrections`
- Completion: `Complete within reviewed scope`
- Requirements consulted: complete frozen generation-0 report and resolution, current user P2G mission, frozen recovery ledger, task/index protocols, current P2F/lease/migration contracts.
- Prior resolution consulted: `rr-20261005-p2grecovery; docs/plans/P2G-review-resolution.md`
- Assumptions: unchanged first-GREEN choices remain settled; no external execution/P2H/P3/physical SQLite-byte claims.
- Excluded as unrelated: full original discovery frontier, later-phase execution/event/crash functionality, unrelated pre-existing source.

## Review Orchestration
- Assessment subagent: `Coordinator assessment - bounded independent closure of correctness, authority and architecture remediation chains`
- Orchestration decision: `Parallel specialists`
- Decision confidence: `high`
- Decision rationale: Reuse three original read-only specialist perspectives for disjoint affected chains, without repeating generation-0 discovery.
- Coordinator override: `None`
- Context or tool limits: First terminal attempts exhausted usage before verdict; resumed same sessions read-only. All three completed independently. No process-crash or cross-architecture experiment.

### Risk Dimensions
- False resume/counters, corrupt-row handling and verifier terminal behavior.
- Savepoint capability boundaries, receipt supersession/chronology and exact authority predicates.
- No execution/time/dependency/schema widening and mechanically adequate closure guards.

### Reviewer Assignments
| Reviewer | Angle | Owned surfaces | Mandatory cross-checks | Status |
| --- | --- | --- | --- | --- |
| R1 | Terminal correctness | F3/F4/F5/F7, T3/T4 | combined classification/count/idempotency and selected docs | Complete |
| R2 | Terminal authority/security | F1/F2/F3/F6, T2 | rollback, origin provenance, structural identity and winner predicates | Complete |
| R3 | Terminal architecture/scope | restricted capability, F6/F7, T1, docs/manifests | API no-execution/time, MSRV evidence and scope exclusions | Complete |

### Synthesis Statement
All three specialists read the complete parent report and resolution before reviewing. Each returned terminal PASS with no remaining concrete finding or standalone gap. Coordinator compared verdicts with the accepted remediation boundaries and independently inspected the final capability, supersession, chronology, CHECK/coherence/action predicates, source guards and report tests. No inherited intentional/disproved settlement was reopened; chronology overlap is the same resolved F3 issue.

## Review Snapshot
- Recommendation: `Pass`
- Completion: `Complete within reviewed scope`
- Why now: All F1–F7 and T1–T4 items are resolved; final stable/MSRV/four-mode evidence is GREEN and terminal affected-chain review found no remaining issue.
- Must-review now: `None`
- Findings count: `Blocker 0 | Major 0 | Minor 0 | Question 0`
- Standalone test gaps: `Blocker 0 | Major 0 | Minor 0`
- Coverage confidence: `high`
- Biggest blind spot: Empirical process-crash and Apple-Silicon execution are explicit later-phase/nonclaims, not P2G approval gaps.

## Complete Findings Index
No code-review findings identified in the reviewed scope.

## Blocker
None.

## Major
None.

## Minor
None.

## Questions
None.

## Test Gaps
None.

## Review Coverage Ledger
| Area ID | Area / path | Touched files or entry points | Owner | Depth | Status | Result | Evidence / next step |
| --- | --- | --- | --- | --- | --- | --- | --- |
| A1 | Restricted recovery capability | storage recovery_pass/RecoveryPass and engine helpers | R1/R2/R3 | contract trace | Reviewed - no issue found | F1 boundary closed; exactly five narrow methods | eight compile-fail and one positive doctest |
| A2 | Receipt supersession | prove_receipt_repair | R2 | contract trace | Reviewed - no issue found | all later real task transitions count, including recovery edges | F2 regression; identity exclusion not authority exemption |
| A3 | Receipt chronology | current completion/observation/release/batch checks | R1/R2 | contract trace | Reviewed - no issue found | F3 current facts bound | contradictory time regressions reject repair |
| A4 | Empty aggregate/counts | semantic inspection and engine classification | R1 | runtime verified | Reviewed - no issue found | F4 invalid empty work not resumed | both source states and mixed-task tests GREEN |
| A5 | Independent CHECK damage | quarantine writability/journal-only path | R1 | runtime verified | Reviewed - no issue found | F5 residual reason preserves raw facts and pass continues | READY/RECEIVED regressions GREEN |
| A6 | Public action authority | validate_action | R2/R3 | contract trace | Reviewed - no issue found | F6 exhaustion exact; uncertain work anywhere prevents resume | contradiction refusals preserve durable state |
| A7 | Acquisition coherence | inspect_authority | R1/R3 | contract trace | Reviewed - no issue found | F7 counters/copy bounds match current P2 writers; renewal remains valid | malformed raw facts withheld from valid projection |
| A8 | Journal/rollback integration | unchanged audit/Tx/store/lease/outcome/journal supporting chains | R2/R3 | dependency trace | Reviewed - no issue found | no normal P2F authority or batch change during remediation | current supporting files byte-identical to first-GREEN snapshot |
| A9 | Closure guards | storage/engine source and dependency assertions | R3 | contract trace | Reviewed - no issue found | T1 full production closure, only open-time injected Clock allowed | no provider/model/network/subprocess/scheduler/GoalLatch |
| A10 | Race winner oracles | file-backed recovery tests | R2 | contract trace | Reviewed - no issue found | T2 deterministic orders now strict, barriers allow either serialized winner | all nine races pass four modes |
| A11 | Verifier public behavior | terminal repair/counters and authority cases | R1 | runtime verified | Reviewed - no issue found | T3/T4 no invented effects/outcomes; second pass no-op | 56 engine recovery tests GREEN |
| A12 | Scope/portability/docs | six plan/ADR annotations, frozen manifest/lock scope | R3 | contract trace | Reviewed - no issue found | no migration/vendor/new package/runtime or transition growth | real stable1.98.1 and exact MSRV1.85.0 results checked |

## Subagent Candidate Adjudication
| Candidate ID | Proposed by | Decision | Final ID | Coordinator evidence | Reason |
| --- | --- | --- | --- | --- | --- |
| R1 terminal | R1 | dismissed | None | all assigned fixes/tests and combined paths checked, 56/41/9 focused tests rerun | no remaining correctness candidate |
| R2 terminal | R2 | dismissed | None | restricted capability and complete supersession/chronology/action predicates verified | no remaining authority candidate |
| R3 terminal | R3 | dismissed | None | API/guard/doc delta plus final logs and manifest package-set checks | no remaining architecture/scope candidate |

## Evidence Appendix
### Diff Inventory
| File or area | Classification | Semantic review area considered |
| --- | --- | --- |
| storage recovery.rs/recovery_tests.rs/lib.rs | surface/test-only | restricted capability, semantic/predicate fixes, regressions |
| engine recovery.rs/tests/recovery.rs | surface/test-only | restricted helper types, mixed corruption, verifier tests and closure guards |
| six P2 plan/Proposed ADR docs | docs-only | current table, atomic-COMMIT interpretation, clock/identity/FK/count/scope reconciliation |
| unchanged first-GREEN source/manifests | dependency | affected P2F guard/outcome/journal contracts and frozen package set |

### Verification Commands
- R1 independently reran stable engine recovery: 56 passed; stable storage recovery: 41 passed; stable capability doctests: 9 passed; exact Rust1.85.0 storage recovery: 41 passed.
- Coordinator/validation agents completed explicit stable1.98.1 and exact +1.85.0 workspace check, all-target tests, all-feature tests, and Clippy -D warnings separately. Workspace tests: 801 regular +45 doctests =846, zero failed/ignored.
- Each toolchain debug/release: storage362 regular+37 doctests=399; engine103 regular+3 doctests=106. Nine file-backed race cases pass in all four modes.
- Evidence: tmp/p2g-final-real-stable-results.json and tmp/p2g-final-msrv-results.json and their command logs. Earlier wrong-labeled stable logs are rejected, not used as stable evidence.
- Read-only git diff --check and snapshot/current supporting-source comparisons passed.

### Supporting Code Links
| ID | Role | Link | Why it matters |
| --- | --- | --- | --- |
| A1 | capability | [restricted pass](../../crates/serea-storage/src/recovery.rs#L184) | no acquisition/general Tx API |
| A3 | proof | [chronology](../../crates/serea-storage/src/recovery.rs#L955) | current outcome facts must corroborate batch |
| A10 | race oracle | [forced ordering](../../crates/serea-task-engine/tests/recovery.rs#L1936) | first writer has prescribed winner |

### Dismissed Coordinator Candidates
| Candidate | Decision | Evidence |
| --- | --- | --- |
| Reopening intentional first-GREEN choices | dismissed | no governing contract/new evidence changed for absence deferral, dev SQLite reuse, explicit time, selected FK tier or legal reassessment |

### Blind Spots
No changed review-relevant area is uncovered. P2H process crash, actual Apple-Silicon execution, P3 event guarantees and external provider reconciliation remain expressly unclaimed.

## Prior Resolution Reconciliation
| Issue key | Issue fingerprint | Parent item/verdict | Relevant change or new evidence | Decision |
| --- | --- | --- | --- | --- |
| `behavior; entry=recovery pass; contract=uncommitted acquisition never regains authority; effect=escaped guard authorizes later reused generation` | `ifp-sha256:6d5992d5ec5ea43289169e8ad118563dcd0a4baab95e20ebd5f736aa0960eb63` | F1 Accepted, resolved | kind:code; ref:crates/serea-storage/src/recovery.rs; change:Expose only a restricted recovery capability, with no acquisition/begin/outcome APIs, and compile-test its boundary. | kept closed |
| `behavior; entry=receipt repair; contract=never overwrite superseding durable transitions; effect=old receipt audit overwrites newer quarantine` | `ifp-sha256:3160a13f4ea8aaffb13461497946ca617800d5120eb44cea5cca9887393e8f61` | F2 Accepted, resolved | kind:code; ref:crates/serea-storage/src/recovery.rs; change:Count all later task transitions for supersession, irrespective of journal origin. | kept closed |
| `behavior; entry=receipt repair; contract=complete current outcome facts corroborate atomic audit; effect=contradictory completion or receipt time authorizes repair` | `ifp-sha256:394c5cdad7eee4b1cdb6078aee7a20f70b35cbe2248c0860d4aa45a91b3c8462` | F3 Accepted, resolved | kind:code; ref:crates/serea-storage/src/recovery.rs; change:Require completed_at=release=batch time and receipt observed_at<=batch time before repair. | kept closed |
| `behavior; entry=recovery classification; contract=resume only semantically valid aggregate; effect=unplanned task hides corruption as resumed` | `ifp-sha256:d2123015ea34cf42059b6e86af80ad0a107125b92738f03b763d839a634d4500` | F4 Accepted, resolved | kind:code; ref:crates/serea-storage/src/recovery.rs; change:Classify missing-plan/missing-step execution or verification aggregates as attributable invariants. | kept closed |
| `behavior; entry=task corruption recovery; contract=classify attributable damage without false mutation; effect=one corrupt task aborts unrelated recoveries` | `ifp-sha256:c525e42d67dea40f02afe88d4df26c20310e195a21be1e3d40d15c2543999680` | F5 Accepted, resolved | kind:code; ref:crates/serea-storage/src/recovery.rs; change:Mark independent reason damage unwriteable for state-only quarantine; journal invariant evidence without erasing original data. | kept closed |
| `behavior; entry=recovery storage action; contract=durable predicates enforce selected disposition; effect=caller bypasses exhausted blocking or uncertain predecessor` | `ifp-sha256:d2fef98491b03e108b0deebb27205db09ac8927f5fa3ab0575e76189bee0c5d3` | F6 Accepted, resolved | kind:code; ref:crates/serea-storage/src/recovery.rs; change:Enforce exact exhausted block disposition and refuse normal resume with uncertain work anywhere. | kept closed |
| `behavior; entry=recovery authority inspection; contract=current acquisition arithmetic and renewal bounds remain coherent; effect=impossible lease history is reported resumable` | `ifp-sha256:92d0d431f62a5677a993ecc79e976729770287392258f39010e641859d99ea39` | F7 Accepted, resolved | kind:code; ref:crates/serea-storage/src/recovery.rs; change:Classify mismatched counters or expiry snapshot bounds as attributable corruption, without rewriting raw values. | kept closed |
| `test-gap; entry=recovery closure guards; contract=no external execution or ambient clock; gap=storage recovery closure is not scanned` | `ifp-sha256:1f195ef3b1fd51522e8bdda2dcd028d4e16c66c48277bc7c55389e4e6d7809ed` | T1 Accepted, resolved | kind:code; ref:crates/serea-task-engine/tests/recovery.rs; change:Scan storage production recovery/audit/lease/outcome/Tx closure as well as engine; allow existing Store open-time Clock only. | kept closed |
| `test-gap; entry=outcome recovery race; contract=known expired unreclaimed outcome wins when first; gap=forced outcome first permits either winner` | `ifp-sha256:1fb66d46cc112fb80be0f5746d55896dc66e859b1f3c4cc980ba51c9ba6598fd` | T2 Accepted, resolved | kind:code; ref:crates/serea-task-engine/tests/recovery.rs; change:Require Ok for deterministic outcome-first and LeaseFenced for deterministic recovery-first; barrier remains either-winner. | kept closed |
| `test-gap; entry=verifier receipt recovery; contract=terminal aggregate repair counters truthful and second pass noop; gap=public engine terminal repair is not asserted` | `ifp-sha256:015346705f70f8a2318028db6fa8e0e714b2aa2b72caaf98450d6adae943ff7e` | T3 Accepted, resolved | kind:code; ref:crates/serea-task-engine/tests/recovery.rs; change:Add full engine verifier-receipt stale aggregate repair through COMPLETED and public counters/second-pass terminal noop. | kept closed |
| `test-gap; entry=verifier authority recovery; contract=held released and exhausted verifier work classify without execution; gap=combined verifier authority cases not pinned` | `ifp-sha256:5d80b68aa2ba67b213ef7bdb73742b8bf0b327845c680d971917b3b462f725c9` | T4 Accepted, resolved | kind:code; ref:crates/serea-task-engine/tests/recovery.rs; change:Add held/released/expired exhausted verifier authority cases and logical identity. | kept closed |

## Receiving Handoff
- Handoff status: `Terminal post-review - return to user/owner`
- Automatic receiving permitted: `No`
- Source report ID: `cr-20261005-p2grecovery1`
- Scope fingerprint to recheck: `sha256:0d6b12a8ec8a9aca7a8d8fb801f933867726e7093e2758901e6e4e0fe5afc7c9`
- Actionable finding IDs: `None`
- Deferred finding IDs: `None`
- Actionable test-gap IDs: `None`
- Deferred test-gap IDs: `None`
- Open question IDs: `None`
- Open coverage area IDs: `None`
- Highest-risk verification to repeat: None before already-authorized documentation closure/commit; do not change reviewed production code.
- Suggested implementation boundaries: final closure bookkeeping and single user-authorized P2G commit only.
- Re-review note: Treat every finding as a claim to verify. Challenges require a counterclaim, argument, evidence, limits, and settlement criterion.
- Chain rule: Generation 1 is terminal. Do not automatically invoke receiving-code-review; return remaining findings to the user or product owner.

## Report Self-Check
- yes Actual assessment/rationale recorded.
- yes Each review-relevant delta and affected chain accounted for.
- yes No new indexed finding or standalone test gap.
- yes Prior F1-F7/T1-T4 keys and fingerprints reconciled.
- yes Every meaningful specialist terminal result adjudicated.
- yes No uncovered review area.
- yes Recommendation Pass follows mapping.
- yes Terminal generation and no automatic receiving consistent.
- yes Parent report/resolution complete and fixed inputs.
- yes Git and source state unchanged during review.
- yes Validator must pass before closure claim.
