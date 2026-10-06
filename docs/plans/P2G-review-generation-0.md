# P2G Independent Implementation Review — Frozen Generation 0

## Report Contract
- Report type: `code-review`
- Report ID: `cr-20261005-p2grecovery0`
- Review chain ID: `rc-20261005-p2grecovery`
- Review generation: `0`
- Review trigger: `initial`
- Parent review report ID: `None`
- Parent review report path: `None`
- Parent resolution ID: `None`
- Parent resolution path: `None`
- Generated at: `2026-10-05T00:00:00Z`
- Report path: `docs/plans/P2G-review-generation-0.md`
- Source skill: `code-review`
- Status: `Review complete`
- Git mutation during review: `None`
- Scope fingerprint: `sha256:bbb080a7b1083e919c40165658d88246078289ef4d6cd5588b083e39ef199d32`

This is an immutable review input. Remediation and adjudication are separate. Editor date is not runtime time evidence.

## Scope
- Review date: `2026-10-05`
- Scope kind: `working tree`
- Scope description: First focused GREEN storage/engine recovery implementation and tests, existing SQLite dev-edge reuse, versus exact P2F HEAD. Snapshot includes both complete crate trees and frozen ledger.
- Scope mode: `full frozen scope`
- Baseline: `7625d206e1fe6794fd21c63ef9b97a8546289e5a`
- Target: `working tree`
- Changed paths: `14`
- Diff size: `169 tracked additions / 26 tracked deletions plus 4742 lines in four new Rust files and closure ledger`
- Completion: `Complete within reviewed scope`
- Requirements consulted: user overnight P2G mission, frozen recovery ledger, task/index protocols, ADR-0021/22/24, current migration and P2F writer contracts.
- Prior resolution consulted: `None`
- Assumptions: P2G cannot infer external absence; no external package addition; real process crashes remain P2H.
- Excluded as unrelated: P2H production, P3, providers/models, descriptor registry and at-rest backend implementation.

## Review Orchestration
- Assessment subagent: `Coordinator assessment - independent state-machine, SQLite authority/security and architecture/portability risks plus explicit three-pass requirement`
- Orchestration decision: `Parallel specialists`
- Decision confidence: `high`
- Decision rationale: Cross-layer repair touches irreversible effect evidence, guard provenance, raw corruption and durable identity; distinct specialists materially improve coverage.
- Coordinator override: `None`
- Context or tool limits: No newly instrumented Rust fixtures during read-only review; static candidates will receive test-first remediation. No real process crash or cross-architecture claim.

### Risk Dimensions
- Semantic classification, stale aggregate authority and exact public counters.
- Whole-pass rollback, guard lifetime, concurrent authority and receipt evidence.
- SQL-free engine, one journal authority, dependency/runtime exclusions and MSRV.

### Reviewer Assignments
| Reviewer | Angle | Owned surfaces | Mandatory cross-checks | Status |
| --- | --- | --- | --- | --- |
| R1 | PASS A correctness | engine classification and storage semantic proof | table, terminal/wait/ceiling/count/idempotency tests | Complete |
| R2 | PASS B transactions/security | storage recovery/audit/Tx/store and races | provenance, predicates, rollback, supersession, leakage | Complete |
| R3 | PASS C architecture/scope | manifests/lock/exports/guards | MSRV, no execution/migration/runtime, one mapper | Complete |

### Synthesis Statement
Coordinator independently re-read every accepted code candidate and its current contract. R1/R2 temporal proof candidates merge into F3. Public-action mismatch is accepted because storage operations validate durable authority, not caller policy. Authority coherence follow-up is accepted as bounded F7 under current P2 writers, not an imported-state assumption. All four distinct test gaps are accepted. No code changed during review; reports and snapshots are evidence artifacts.

## Review Snapshot
- Recommendation: `Changes requested`
- Completion: `Complete within reviewed scope`
- Why now: Focused GREEN misses six major recovery correctness/authority defects and one narrow corruption classifier gap.
- Must-review now: `F1, F2, F3`
- Findings count: `Blocker 0 | Major 6 | Minor 1 | Question 0`
- Standalone test gaps: `Blocker 0 | Major 0 | Minor 4`
- Coverage confidence: `high`
- Biggest blind spot: New Rust regression reproduction and final stable/MSRV/four-mode validation remain pending.

## Complete Findings Index
| ID | Severity | Surface | Review risk | Confidence | Origin | Verification | Issue key | Issue fingerprint | Expected basis |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| F1 | Major | Recovery pass capability | Rolled-back recovery acquisition can regain guard authority | high | R2 | independent source/contract trace | `behavior; entry=recovery pass; contract=uncommitted acquisition never regains authority; effect=escaped guard authorizes later reused generation` | `ifp-sha256:6d5992d5ec5ea43289169e8ad118563dcd0a4baab95e20ebd5f736aa0960eb63` | `kind:hard-invariant; strength:authoritative; evidence:ADR-0024 guard provenance and user no fabricated authority` |
| F2 | Major | Receipt supersession | Receipt repair ignores later recovery blocking transitions | high | R2 | independent source/contract trace | `behavior; entry=receipt repair; contract=never overwrite superseding durable transitions; effect=old receipt audit overwrites newer quarantine` | `ifp-sha256:3160a13f4ea8aaffb13461497946ca617800d5120eb44cea5cca9887393e8f61` | `kind:hard-invariant; strength:authoritative; evidence:Frozen ledger item 1 and user section 9` |
| F3 | Major | Receipt temporal proof | Receipt repair omits current outcome chronology | high | R1/R2 | independent source/contract trace | `behavior; entry=receipt repair; contract=complete current outcome facts corroborate atomic audit; effect=contradictory completion or receipt time authorizes repair` | `ifp-sha256:394c5cdad7eee4b1cdb6078aee7a20f70b35cbe2248c0860d4aa45a91b3c8462` | `kind:hard-invariant; strength:authoritative; evidence:Frozen ledger item 1 and unchanged P2F outcome atomic-write rules` |
| F4 | Major | Aggregate classification | Empty execution or verification aggregate is reported resumable | high | R1 | independent source/contract trace | `behavior; entry=recovery classification; contract=resume only semantically valid aggregate; effect=unplanned task hides corruption as resumed` | `ifp-sha256:d2123015ea34cf42059b6e86af80ad0a107125b92738f03b763d839a634d4500` | `kind:hard-invariant; strength:authoritative; evidence:Task Protocol state meanings and selected recovery table` |
| F5 | Major | Task-local CHECK handling | Residual blocked reason aborts legal reassessment | high | R1 | independent source/contract trace | `behavior; entry=task corruption recovery; contract=classify attributable damage without false mutation; effect=one corrupt task aborts unrelated recoveries` | `ifp-sha256:c525e42d67dea40f02afe88d4df26c20310e195a21be1e3d40d15c2543999680` | `kind:hard-invariant; strength:authoritative; evidence:User corruption separation and selected invariant-only handling` |
| F6 | Major | Public recovery action validation | Public actions admit contradictory recovery policy | high | R3 | independent source/contract trace | `behavior; entry=recovery storage action; contract=durable predicates enforce selected disposition; effect=caller bypasses exhausted blocking or uncertain predecessor` | `ifp-sha256:d2fef98491b03e108b0deebb27205db09ac8927f5fa3ab0575e76189bee0c5d3` | `kind:hard-invariant; strength:authoritative; evidence:Frozen selected table and storage authority ownership` |
| F7 | Minor | Authority semantic inspection | Impossible acquisition history can be treated as valid eligibility | high | R1 | independent source/contract trace | `behavior; entry=recovery authority inspection; contract=current acquisition arithmetic and renewal bounds remain coherent; effect=impossible lease history is reported resumable` | `ifp-sha256:92d0d431f62a5677a993ecc79e976729770287392258f39010e641859d99ea39` | `kind:hard-invariant; strength:authoritative; evidence:Current P2E acquisition and strict-renewal contracts` |

## Blocker
None.

## Major

### F1 Major - Rolled-back recovery acquisition can regain guard authority
Impact: Public recovery_pass exposes unrestricted Tx. A caught savepoint failure can roll back an acquisition while the outer commit publishes its origin; later same-owner generation reuse admits the escaped guard.
Review reason: Violates the selected classification or durable authority contract.
Surface: Recovery pass capability
Issue key: `behavior; entry=recovery pass; contract=uncommitted acquisition never regains authority; effect=escaped guard authorizes later reused generation`
Issue fingerprint: `ifp-sha256:6d5992d5ec5ea43289169e8ad118563dcd0a4baab95e20ebd5f736aa0960eb63`
Expected basis: `kind:hard-invariant; strength:authoritative; evidence:ADR-0024 guard provenance and user no fabricated authority`
Confidence: high
Origin: R2
Coordinator verification: Source trace through operation_savepoint, Store origin publication and lease matching confirms candidate; actual Rust reproduction follows remediation.

Look here first:
- [affected path](../../crates/serea-storage/src/recovery.rs#L187)

Failure mode:
- Expected: Expose only a restricted recovery capability, with no acquisition/begin/outcome APIs, and compile-test its boundary.
- Current: Public recovery_pass exposes unrestricted Tx. A caught savepoint failure can roll back an acquisition while the outer commit publishes its origin; later same-owner generation reuse admits the escaped guard.

Evidence:
- Source trace through operation_savepoint, Store origin publication and lease matching confirms candidate; actual Rust reproduction follows remediation.

Assumptions and limits:
- Engine follows the intended classification today; corruption fixtures and alternate public storage callers expose the listed paths. No external effect is executed.

Reviewer action: request focused test-first fix.

### F2 Major - Receipt repair ignores later recovery blocking transitions
Impact: Superseding predicate excludes all recovery-marked task edges, including real BLOCKED transitions; corrupted stale aggregate can be repaired over a later quarantine.
Review reason: Violates the selected classification or durable authority contract.
Surface: Receipt supersession
Issue key: `behavior; entry=receipt repair; contract=never overwrite superseding durable transitions; effect=old receipt audit overwrites newer quarantine`
Issue fingerprint: `ifp-sha256:3160a13f4ea8aaffb13461497946ca617800d5120eb44cea5cca9887393e8f61`
Expected basis: `kind:hard-invariant; strength:authoritative; evidence:Frozen ledger item 1 and user section 9`
Confidence: high
Origin: R2
Coordinator verification: Re-read marker write and superseding query; confirmed blanket exclusion, distinct from legitimate fingerprint exclusion.

Look here first:
- [affected path](../../crates/serea-storage/src/recovery.rs#L941)

Failure mode:
- Expected: Count all later task transitions for supersession, irrespective of journal origin.
- Current: Superseding predicate excludes all recovery-marked task edges, including real BLOCKED transitions; corrupted stale aggregate can be repaired over a later quarantine.

Evidence:
- Re-read marker write and superseding query; confirmed blanket exclusion, distinct from legitimate fingerprint exclusion.

Assumptions and limits:
- Engine follows the intended classification today; corruption fixtures and alternate public storage callers expose the listed paths. No external effect is executed.

Reviewer action: request focused test-first fix.

### F3 Major - Receipt repair omits current outcome chronology
Impact: Proof matches release and journal time but not current completed_at or receipt observation. A completion stamp 43 or observed_at 900 can corroborate an outcome batch committed at 42.
Review reason: Violates the selected classification or durable authority contract.
Surface: Receipt temporal proof
Issue key: `behavior; entry=receipt repair; contract=complete current outcome facts corroborate atomic audit; effect=contradictory completion or receipt time authorizes repair`
Issue fingerprint: `ifp-sha256:394c5cdad7eee4b1cdb6078aee7a20f70b35cbe2248c0860d4aa45a91b3c8462`
Expected basis: `kind:hard-invariant; strength:authoritative; evidence:Frozen ledger item 1 and unchanged P2F outcome atomic-write rules`
Confidence: high
Origin: R1/R2
Coordinator verification: Independently traced normal outcome success/release stamps and receipt future refusal; loader only checks completion versus start.

Look here first:
- [affected path](../../crates/serea-storage/src/recovery.rs#L844)

Failure mode:
- Expected: Require completed_at=release=batch time and receipt observed_at<=batch time before repair.
- Current: Proof matches release and journal time but not current completed_at or receipt observation. A completion stamp 43 or observed_at 900 can corroborate an outcome batch committed at 42.

Evidence:
- Independently traced normal outcome success/release stamps and receipt future refusal; loader only checks completion versus start.

Assumptions and limits:
- Engine follows the intended classification today; corruption fixtures and alternate public storage callers expose the listed paths. No external effect is executed.

Reviewer action: request focused test-first fix.

### F4 Major - Empty execution or verification aggregate is reported resumable
Impact: Revision-zero task with no steps changed to EXECUTING/VERIFYING passes vacuous ordinary-success tests and increments tasks_resumed.
Review reason: Violates the selected classification or durable authority contract.
Surface: Aggregate classification
Issue key: `behavior; entry=recovery classification; contract=resume only semantically valid aggregate; effect=unplanned task hides corruption as resumed`
Issue fingerprint: `ifp-sha256:d2123015ea34cf42059b6e86af80ad0a107125b92738f03b763d839a634d4500`
Expected basis: `kind:hard-invariant; strength:authoritative; evidence:Task Protocol state meanings and selected recovery table`
Confidence: high
Origin: R1
Coordinator verification: Re-read load_history revision zero, inspection conditions and classify next=None path.

Look here first:
- [affected path](../../crates/serea-storage/src/recovery.rs#L332)

Failure mode:
- Expected: Classify missing-plan/missing-step execution or verification aggregates as attributable invariants.
- Current: Revision-zero task with no steps changed to EXECUTING/VERIFYING passes vacuous ordinary-success tests and increments tasks_resumed.

Evidence:
- Re-read load_history revision zero, inspection conditions and classify next=None path.

Assumptions and limits:
- Engine follows the intended classification today; corruption fixtures and alternate public storage callers expose the listed paths. No external effect is executed.

Reviewer action: request focused test-first fix.

### F5 Major - Residual blocked reason aborts legal reassessment
Impact: Writability predicate omits blocked_reason CHECKs; READY/RECEIVED corruption routes through PLANNING while preserving a non-null reason, causing CHECK refusal and whole-pass rollback.
Review reason: Violates the selected classification or durable authority contract.
Surface: Task-local CHECK handling
Issue key: `behavior; entry=task corruption recovery; contract=classify attributable damage without false mutation; effect=one corrupt task aborts unrelated recoveries`
Issue fingerprint: `ifp-sha256:c525e42d67dea40f02afe88d4df26c20310e195a21be1e3d40d15c2543999680`
Expected basis: `kind:hard-invariant; strength:authoritative; evidence:User corruption separation and selected invariant-only handling`
Confidence: high
Origin: R1
Coordinator verification: Re-read predicate/update and migration CHECK; reviewer in-memory SQL confirmed predicate true followed by rejected UPDATE.

Look here first:
- [affected path](../../crates/serea-storage/src/recovery.rs#L968)

Failure mode:
- Expected: Mark independent reason damage unwriteable for state-only quarantine; journal invariant evidence without erasing original data.
- Current: Writability predicate omits blocked_reason CHECKs; READY/RECEIVED corruption routes through PLANNING while preserving a non-null reason, causing CHECK refusal and whole-pass rollback.

Evidence:
- Re-read predicate/update and migration CHECK; reviewer in-memory SQL confirmed predicate true followed by rejected UPDATE.

Assumptions and limits:
- Engine follows the intended classification today; corruption fixtures and alternate public storage callers expose the listed paths. No external effect is executed.

Reviewer action: request focused test-first fix.

### F6 Major - Public actions admit contradictory recovery policy
Impact: NeedsReconciliation block=false accepts exhausted work and ResumeNormally only validates the first unfinished step, ignoring uncertain work later. Engine correctly chooses stronger actions; public storage validation does not.
Review reason: Violates the selected classification or durable authority contract.
Surface: Public recovery action validation
Issue key: `behavior; entry=recovery storage action; contract=durable predicates enforce selected disposition; effect=caller bypasses exhausted blocking or uncertain predecessor`
Issue fingerprint: `ifp-sha256:d2fef98491b03e108b0deebb27205db09ac8927f5fa3ab0575e76189bee0c5d3`
Expected basis: `kind:hard-invariant; strength:authoritative; evidence:Frozen selected table and storage authority ownership`
Confidence: high
Origin: R3
Coordinator verification: Re-read constructible action API and both validation predicates; accepted as correctness defect, not provider-execution claim.

Look here first:
- [affected path](../../crates/serea-storage/src/recovery.rs#L665)

Failure mode:
- Expected: Enforce exact exhausted block disposition and refuse normal resume with uncertain work anywhere.
- Current: NeedsReconciliation block=false accepts exhausted work and ResumeNormally only validates the first unfinished step, ignoring uncertain work later. Engine correctly chooses stronger actions; public storage validation does not.

Evidence:
- Re-read constructible action API and both validation predicates; accepted as correctness defect, not provider-execution claim.

Assumptions and limits:
- Engine follows the intended classification today; corruption fixtures and alternate public storage callers expose the listed paths. No external effect is executed.

Reviewer action: request focused test-first fix.

## Minor
### F7 Minor - Impossible acquisition history can be treated as valid eligibility
Impact: Inspection does not pin attempt equal to acquisition generation or acquisition-copy expiry within acquired<copy<=authoritative expiry. Current valid P2E writers establish these relationships; a forged released history can be falsely resumed.
Review reason: Current P2 writer arithmetic is authority for semantic validity.
Surface: Authority semantic inspection
Issue key: `behavior; entry=recovery authority inspection; contract=current acquisition arithmetic and renewal bounds remain coherent; effect=impossible lease history is reported resumable`
Issue fingerprint: `ifp-sha256:92d0d431f62a5677a993ecc79e976729770287392258f39010e641859d99ea39`
Expected basis: `kind:hard-invariant; strength:authoritative; evidence:Current P2E acquisition and strict-renewal contracts`
Confidence: high
Origin: R1
Coordinator verification: Coordinator read acquisition increments both counters from zero and renew only extends authoritative expiry; no import writer is part of P2.

Look here first:
- [affected path](../../crates/serea-storage/src/recovery.rs#L258)

Failure mode:
- Expected: Classify mismatched counters or expiry snapshot bounds as attributable corruption, without rewriting raw values.
- Current: Inspection does not pin attempt equal to acquisition generation or acquisition-copy expiry within acquired<copy<=authoritative expiry. Current valid P2E writers establish these relationships; a forged released history can be falsely resumed.

Evidence:
- Coordinator read acquisition increments both counters from zero and renew only extends authoritative expiry; no import writer is part of P2.

Assumptions and limits:
- No external import writer exists in P2; terminal tasks remain immutable regardless of retained work.

Reviewer action: request bounded classifier regression.

## Questions
None.

## Test Gaps
| ID | Severity | Surface | Missing coverage | Risk | Origin | Evidence | Issue key | Issue fingerprint | Expected basis |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| T1 | Minor | No-execution closure guard | Scan storage production recovery/audit/lease/outcome/Tx closure as well as engine; allow existing Store open-time Clock only. | regression can evade advertised oracle | R3 | independent trace of tests/recovery.rs guards/races | `test-gap; entry=recovery closure guards; contract=no external execution or ambient clock; gap=storage recovery closure is not scanned` | `ifp-sha256:1f195ef3b1fd51522e8bdda2dcd028d4e16c66c48277bc7c55389e4e6d7809ed` | `kind:owner-decision; strength:authoritative; evidence:user sections 7,12,13 and frozen ledger` |
| T2 | Minor | Forced race winner oracle | Require Ok for deterministic outcome-first and LeaseFenced for deterministic recovery-first; barrier remains either-winner. | regression can evade advertised oracle | R2 | independent trace of tests/recovery.rs guards/races | `test-gap; entry=outcome recovery race; contract=known expired unreclaimed outcome wins when first; gap=forced outcome first permits either winner` | `ifp-sha256:1fb66d46cc112fb80be0f5746d55896dc66e859b1f3c4cc980ba51c9ba6598fd` | `kind:owner-decision; strength:authoritative; evidence:user sections 7,12,13 and frozen ledger` |
| T3 | Minor | Verifier receipt terminal report | Add full engine verifier-receipt stale aggregate repair through COMPLETED and public counters/second-pass terminal noop. | regression can evade advertised oracle | R1 | independent trace of tests/recovery.rs guards/races | `test-gap; entry=verifier receipt recovery; contract=terminal aggregate repair counters truthful and second pass noop; gap=public engine terminal repair is not asserted` | `ifp-sha256:015346705f70f8a2318028db6fa8e0e714b2aa2b72caaf98450d6adae943ff7e` | `kind:owner-decision; strength:authoritative; evidence:user sections 7,12,13 and frozen ledger` |
| T4 | Minor | Verifier authority classifications | Add held/released/expired exhausted verifier authority cases and logical identity. | regression can evade advertised oracle | R1 | independent trace of tests/recovery.rs guards/races | `test-gap; entry=verifier authority recovery; contract=held released and exhausted verifier work classify without execution; gap=combined verifier authority cases not pinned` | `ifp-sha256:5d80b68aa2ba67b213ef7bdb73742b8bf0b327845c680d971917b3b462f725c9` | `kind:owner-decision; strength:authoritative; evidence:user sections 7,12,13 and frozen ledger` |

## Review Coverage Ledger
| Area ID | Area / path | Touched files or entry points | Owner | Depth | Status | Result | Evidence / next step |
| --- | --- | --- | --- | --- | --- | --- | --- |
| A1 | Public orchestration/capability | engine recovery and storage recovery_pass | R1/R2 | contract trace | Finding F1 | unrestricted savepoint capability escape | restrict capability and compile regression |
| A2 | Receipt supersession | receipt proof | R2 | contract trace | Finding F2 | recovery edges mistakenly ignored | honor later real task edges |
| A3 | Receipt chronology | receipt proof/outcome loader context | R1/R2 | contract trace | Finding F3 | current stamps not bound | pin contradictory timestamps |
| A4 | State classification/report | raw inspection and classify | R1 | dependency trace | Finding F4 | empty invalid aggregates resumable | explicit semantic rejection |
| A5 | Task CHECK attribution | quarantine writability | R1 | runtime verified | Finding F5 | residual reason CHECK failure | journal-only damaged row |
| A6 | Action validation | public RecoveryAction and apply | R3 | contract trace | Finding F6 | contradictory selections accepted | reject invalid caller dispositions |
| A7 | Lease coherence | authority inspection | R1/Coordinator | dependency trace | Finding F7 | impossible histories accepted | preserve raw, classify corrupt |
| A8 | Journal persistence/semantics | audit.rs and TaskJournal | R2/R3 | contract trace | Reviewed - no issue found | one mapper and structural dedup; digest-only safe payload | outcome mapper unchanged |
| A9 | Error/export/API inventory | error.rs/lib.rs/blob_tests.rs/engine.rs | R3 | dependency trace | Reviewed - no issue found | typed payload-free errors and opaque snapshots | no raw SQL export |
| A10 | Dependencies/lock | task-engine manifest and Cargo.lock | R3 | contract trace | Reviewed - no issue found | existing dev-only SQLite edge; zero new packages | no runtime dependency change |
| A11 | Test coverage | recovery tests in both crates | R1/R2/R3 | runtime verified | Reviewed - no issue found | 46+21 pass; accepted standalone gaps T1-T4 | remediate gaps with findings |
| A12 | Frozen scope/portability | protocol/migration/vendor unchanged context | R3 | contract trace | Reviewed - no issue found | no P2H/P3/provider/model clock additions | final MSRV validation pending |
| A13 | Closure ledger | selected design evidence | Coordinator | contract trace | Reviewed - no issue found | pending validation explicitly labelled | final documentation update after review |

## Subagent Candidate Adjudication
| Candidate ID | Proposed by | Decision | Final ID | Coordinator evidence | Reason |
| --- | --- | --- | --- | --- | --- |
| F1 candidate | R2 | accepted | F1 | Source trace through operation_savepoint, Store origin publication and lease matching confirms candidate; actual Rust reproduction follows remediation. | current authoritative contract requires correction |
| F2 candidate | R2 | accepted | F2 | Re-read marker write and superseding query; confirmed blanket exclusion, distinct from legitimate fingerprint exclusion. | current authoritative contract requires correction |
| F3 candidate | R1/R2 | accepted | F3 | Independently traced normal outcome success/release stamps and receipt future refusal; loader only checks completion versus start. | current authoritative contract requires correction |
| F4 candidate | R1 | accepted | F4 | Re-read load_history revision zero, inspection conditions and classify next=None path. | current authoritative contract requires correction |
| F5 candidate | R1 | accepted | F5 | Re-read predicate/update and migration CHECK; reviewer in-memory SQL confirmed predicate true followed by rejected UPDATE. | current authoritative contract requires correction |
| F6 candidate | R3 | accepted | F6 | Re-read constructible action API and both validation predicates; accepted as correctness defect, not provider-execution claim. | current authoritative contract requires correction |
| F7 candidate | R1 | accepted | F7 | Coordinator read acquisition increments both counters from zero and renew only extends authoritative expiry; no import writer is part of P2. | current authoritative contract requires correction |
| T1 candidate | R3 | accepted | T1 | test call-surface/race trace | standalone distinct coverage gap |
| T2 candidate | R2 | accepted | T2 | test call-surface/race trace | standalone distinct coverage gap |
| T3 candidate | R1 | accepted | T3 | test call-surface/race trace | standalone distinct coverage gap |
| T4 candidate | R1 | accepted | T4 | test call-surface/race trace | standalone distinct coverage gap |
| Temporal duplicate | R1/R2 | merged | F3 | both point to same unbound time predicates | one failure mode |
| Invalid numeric placeholder authority | R3 | dismissed | None | conversion marks corruption and suppresses authority/projection before ordinary action | placeholders do not become valid enums/authority |
| Missing recovery quick_check | R2 | dismissed | None | selected recovery integrity tier is catalog/FK then semantic scan | no unsupported page-integrity claim |
| Raw prose as authority | R3 | dismissed | None | structural envelope plus recomputed fingerprint, raw values digest-only | free text neither classifies nor authorizes |
| Missing later-phase behavior | R1/R3 | dismissed | None | P2H/P3/provider exclusions explicit | outside requested production scope |

## Evidence Appendix
### Diff Inventory
| File or area | Classification | Semantic review area considered |
| --- | --- | --- |
| storage recovery.rs/audit.rs | surface | authority, raw inspection, predicate writes, audit |
| storage recovery_tests.rs/blob_tests.rs | test-only | rollback, corruption, private API inventory |
| storage error.rs/lib.rs | surface | failures and exports |
| engine recovery.rs/journal.rs/engine.rs/lib.rs | surface | orchestration, counters, single journal mapping, doctest transition |
| engine tests/recovery.rs | test-only | table, identity, file-backed races and no-execution guards |
| task-engine Cargo.toml/Cargo.lock | dependency | dev-only reuse, no new package/version |
| closure ledger | docs-only | frozen decisions and actual evidence |

### Verification Commands
- `cargo test -p serea-task-engine --test recovery --offline --locked` -> R1 46 passed.
- `cargo test -p serea-storage recovery --offline --locked` -> R1 21 passed.
- `cargo test --workspace --all-features --offline` -> coordinator successful pre-remediation workspace run retained in tmp/p2g-review-workspace-green.log.
- Reviewers used in-memory-only SQLite probes for superseding marker/writability/acquisition rollback; no crash experiment.
- Snapshot comparisons found no crate changes during review.

### Supporting Code Links
| ID | Role | Link | Why it matters |
| --- | --- | --- | --- |
| F1 | authority | [origin matching](../../crates/serea-storage/src/lease.rs#L119) | outer origin publication can revive rolled-back guard |
| F3 | actual writer | [normal outcome](../../crates/serea-storage/src/outcome.rs#L293) | complete outcome uses one supplied timestamp |

### Dismissed Coordinator Candidates
| Candidate | Decision | Evidence |
| --- | --- | --- |
| new dependency package | dismissed | existing rusqlite package/version is unchanged; dev-only edge changes lock metadata |
| direct illegal READY blocking | dismissed | legal real reassessment pair remains frozen |

### Blind Spots
No review-relevant area is uncovered. Empirical process crash, Apple-Silicon portability, provider reconciliation and P3 delivery are explicit nonclaims, not P2G approval gaps. New candidate runtime repros and final validation follow remediation.

## Prior Resolution Reconciliation
None - initial review generation.

## Receiving Handoff
- Handoff status: `Ready for receiving-code-review`
- Automatic receiving permitted: `Yes`
- Source report ID: `cr-20261005-p2grecovery0`
- Scope fingerprint to recheck: `sha256:bbb080a7b1083e919c40165658d88246078289ef4d6cd5588b083e39ef199d32`
- Actionable finding IDs: `F1, F2, F3, F4, F5, F6, F7`
- Deferred finding IDs: `None`
- Actionable test-gap IDs: `T1, T2, T3, T4`
- Deferred test-gap IDs: `None`
- Open question IDs: `None`
- Open coverage area IDs: `None`
- Highest-risk verification to repeat: restricted pass capability, superseding receipt authority, contradictory stamps, full logical identity and two-Store winner oracles.
- Suggested implementation boundaries: recovery modules/tests and narrow audit/API handoff only; no normal P2F outcome semantic widening.
- Re-review note: Treat every finding as a claim to verify. Challenges require a counterclaim, argument, evidence, limits, and settlement criterion.
- Chain rule: Generation 1 is terminal. Do not automatically invoke receiving-code-review; return remaining findings to the user or product owner.

## Report Self-Check
- yes Actual assessment and rationale recorded.
- yes Every changed review-relevant area accounted for.
- yes Every finding indexed and carded once.
- yes Every finding area references a real ID.
- yes All standalone gaps identified and classified.
- yes All issue keys/fingerprints and authoritative bases recorded.
- yes Initial generation and authorized receiving handoff consistent.
- yes Generation 1 inheritance not applicable yet.
- yes Every accepted item actionable exactly once.
- yes Every meaningful candidate adjudicated.
- yes No uncovered review area.
- yes Recommendation follows mapping.
- yes Validator will be run before receiving.
- yes No Git mutation during review.
