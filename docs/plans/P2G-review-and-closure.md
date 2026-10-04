# P2G recovery — review and closure ledger

## Status

**CLOSED — P2G runtime implementation, independent review and release validation complete.**

Initial status was **OPEN — design reconciliation / implementation pending**. The preimplementation reconciliation below was frozen before production edits; the final measured disposition and evidence are recorded in the closure sections below. This closes P2G only, not P2H, P2I, P3 or full ADR ratification.

Starting HEAD: `7c05ffae3503115a069761d40201aaf84595679a`.
Starting branch: `p2/p2f-task-engine-core`; clean preflight.
Implementation branch: `p2/p2g-recovery`, created from the exact starting HEAD.

## Measured baseline

Commands completed separately, then repeated separately with retained local logs:

- `cargo test --workspace --all-features --offline`: **704 regular + 36 doctests = 740**, failed 0, ignored 0.
- `cargo +1.85.0 test --workspace --all-features --offline`: **704 regular + 36 doctests = 740**, failed 0, ignored 0.

Local evidence: `tmp/p2g-baseline-stable.log`, `tmp/p2g-baseline-msrv.log`. These are validation artifacts, not production source.

## Preimplementation reconciliation (frozen before production edits)

Two independent read-only audits challenged the historical table and the apparent READY-to-BLOCKED conflict. The direct edge remains forbidden. The following design choices do not widen the 121-pair oracle.

1. **Receipt condition:** migration 0001 refuses receipt INSERT unless the step is SUCCEEDED. EXECUTING plus receipt is corruption, not a normal recovery window. A SUCCEEDED step with a valid result/receipt and stale aggregate is schema-representable but not reachable as a partial committed P2F outcome. Repair requires corroborating complete outcome audit, released authority, valid plan/provenance and a uniquely established legal aggregate destination; otherwise quarantine/refuse. Never reconstruct a missing outcome batch.
2. **N6:** P2F atomically commits step/result/ref/receipt/task/journal/release. After COMMIT, durable truth is complete regardless of caller observation. A consistent committed task needs no repair, repeated outcome journal or effect. P2H owns the real process-crash proof.
3. **Journal-only idempotency:** existing RECOVERY_DECISION evidence is deduplicated in the same writer transaction by structural envelope fields plus a recomputed digest of relevant durable facts. Free-text payload is not authority. Identity includes task/step/revision/attempt/generation/lease/result/receipt/provenance and excludes caller time/actor and recovery journal sequence. Repairs identify the stable post-repair situation; materially changed durable facts permit a new decision. No marker table/column.
4. **Raw inspection:** storage owns a narrow recovery snapshot/operation, retaining original state/status and validated attribution. The engine receives neither Connection nor Tx internals nor arbitrary SQL. Malformed data is never coerced into an enum. Normal typed loading remains fail closed.
5. **FK preflight:** ANY foreign_key_check failure refuses the entire pass before mutation. Task-local semantic damage is handled only with intact relational attribution. Catalog/migration authority is rechecked in the recovery snapshot; recovery follows the selected recovery integrity tier (design §7.1), not an invented universal page-check claim. Bounded semantic inspection is part of classification; unreadable structural data refuses.
6. **Transaction policy:** one BEGIN IMMEDIATE transaction/snapshot for the whole pass, with a whole-pass savepoint so caught errors cannot commit partial repair. No external callbacks/effects or clock reads while holding the writer reservation. This favors coherent authority and all-or-nothing report publication over writer throughput; recovery is a finite administrative/startup pass, not a scheduler. Operation predicates compare observed durable authority. Any error rolls back every pass mutation and publishes no success report.
7. **Explicit time:** `TaskEngine::recover(&mut self, now: EpochMillis, context: &TransitionContext<'_>) -> Result<RecoveryReport, EngineError>`. No retained Clock or ambient time.
8. **Expired authority:** storage conditionally revokes the exact observed unreleased owner/generation/expiry with expiry <= now and matching task/step facts. Never reconstruct LeaseGuard. Revocation fences the old generation. SQLite serialization decides outcome-first versus recovery-first, reclaim-first versus recovery-first, and recovery versus recovery.
9. **Crash-only attempt ceiling:** no provider failure is invented. NeedsReconciliation; block with a reconciliation/invariant code. EXECUTING/VERIFYING/PLANNING can legally block directly. For a stranded READY plan, enter real reassessment using legal READY → PLANNING (REPLAN), then PLANNING → BLOCKED in the same audited atomic repair. No begin, started_at, model call, new revision or execution is invented. The exhaustive frozen transition oracle is unchanged. A source for which no truthful legal quarantine path exists is classified/refused rather than given a forbidden edge.
10. **Unknown values:** unknown raw task state uses the frozen Protocol Index compatibility-quarantine rule, preserving its original raw value in inspection and hashed evidence rather than fabricating a recognised source enum. Unknown step status is an execution quarantine with UNRECOGNISED_STATE; legal durable blocking applies where possible. A pending approval/user state must not be falsely resolved merely to reach BLOCKED. Terminal states stay immutable.
11. **One journal authority:** TaskJournal gains recovery semantics through existing storage facts/drafts/private sink. No recovery-only SQL mapper, engine INSERT, EventKind or SereaEvent. STEP_RECONCILED_ABSENT remains vocabulary without a writer unless confirmed-absence evidence exists.
12. **RECONCILED_ABSENT:** recovery has no provider read-back and cannot prove external absence from expiry/no receipt. It will not manufacture absent closure. Existing well-formed absence closures can be classified conservatively, never treated as successful predecessors. Evidence-submission/execution integration is not added merely to satisfy schema vocabulary.
13. **Terminal no-op:** COMPLETED, FAILED, CANCELLED receive only in-memory TerminalNoop. No updated_at normalization, lease release, journal or other task mutation, even for a cancelled task retaining in-flight work.
14. **Counter definitions (selected):** tasks_examined counts task identities inspected, including terminal tasks; tasks_resumed counts distinct tasks classified ResumeNormally (eligibility, not execution); repairs_committed counts distinct tasks whose recovery operation committed a durable change, including first-time decision audit; invariant_violations counts distinct attributable invalid tasks; decisions are ordered in-memory classifications and actual authority-revocation observations; pending_event_transitions is the final committed count of ALL task_journal rows, including recovery audit. It is not a queue/outbox and nothing is backfilled.
15. **Second-pass identity:** compare deterministic logical durable-state bytes (all task/step/lease/receipt/journal/revision/ref/blob rows), not SQLite/WAL file metadata. Same explicit now/context must cause no durable churn. Stable classifications are promised for unchanged post-repair situations; mutation observations such as a newly revoked lease need not recur.

## Selected recovery table

Global precedence is catalog authority, FK gate, then task inspection. Unsupported ordinary-row classes fail closed. Each row is still pending implementation/test evidence.

| Condition | Decision | Durable action |
| --- | --- | --- |
| Recognised terminal task | TerminalNoop | None, no journal |
| Invalid global catalog/FK/unreadable relational structure | Typed pass refusal | None anywhere |
| Attributable unknown state/status or semantic invariant | CorruptOrInvariantViolation / quarantine | Legal blocking or compatibility quarantine; no guessed enum or illegal known edge |
| Unexpired matching unreleased lease | Held/deferred | Preserve authority; classify, never execute |
| Expired matching unreleased lease | ExpiredLease then stable classification | Exact conditional revocation plus audit |
| LEASED, released authority, budget available | ResumeNormally | Journal once; leave acquired step facts intact |
| LEASED/EXECUTING, no outcome, ceiling exhausted after authority release | NeedsReconciliation / BlockedTask | Legal audited blocking; no provider failure |
| EXECUTING, released authority, no receipt/result | NeedsReconciliation | Journal once; no retry, result or receipt |
| Receipt with non-SUCCEEDED status | CorruptOrInvariantViolation | No conversion to success |
| SUCCEEDED + valid receipt/result, compatible aggregate | ReceiptAlreadyCommitted observation / normal eligibility | No outcome or receipt rewrite |
| SUCCEEDED + valid receipt/result, proven stale aggregate | ReceiptAlreadyCommitted repair | Conditional task-only legal repair and recovery audit |
| WAIT_APPROVAL waiting/task | AwaitApproval | Journal once; no render/device use |
| WAIT_USER/WAIT_SCHEDULE waiting/task | AwaitUser | Journal once; no input/scheduler action |
| RECEIVED/PLANNING/READY or VERIFYING eligible work | ResumeNormally | Journal once, no planning/execution callback |
| VERIFYING without verifier after ordinary success | Deferred verification / normal eligibility | Remain VERIFYING, never invent COMPLETED |
| BLOCKED | BlockedTask plus uncertain-work classification as applicable | Do not infer external block cleared |
| Existing RECONCILED_ABSENT | Conservative closed-absence classification | No re-execution or replacement synthesis |
| Missing/mismatched prior authority, unsupported outcome combination | CorruptOrInvariantViolation | Fail closed, no guessed repair |

## Test-first and implementation evidence

First genuine RED: `cargo test -p serea-task-engine --test recovery --offline`, retained in `tmp/p2g-first-red.log`: E0432 missing RecoveryDecision/RecoveryReport and E0599 missing TaskEngine::recover. Comprehensive classification tests were added before production implementation.
First focused GREEN / feature freeze: `cargo test -p serea-task-engine --test recovery --offline`: **46 passed, 0 failed, 0 ignored**, recorded in `tmp/p2g-focused-first-green.log`. Full engine at this point: 93 regular + 3 doctests, all passed. FEATURES FROZEN. No P2H functionality will be added. Storage at integration: 340 unit + 2 integration + 28 doctests across stable/MSRV debug/release; final source revalidation still required.
M-group coverage, logical identity, file-backed races and no-execution assertions: **PASS**. Final engine recovery suite has **56 tests**; storage recovery has **41 unit tests and 9 capability doctests**. Every reconciled M1–M22 obligation is covered; impossible historical states are tested as corruption/refusal rather than produced by weakening the schema.

## Independent review lineage

Preimplementation design audits completed (two independent read-only perspectives). These are not the required post-GREEN reviews.

Generation 0 correctness / concurrency-security / architecture: **three independent reviews completed**. [Frozen generation 0](P2G-review-generation-0.md) records six Major findings, one Minor finding and four Minor test gaps; the report validator passed.

[Resolution](P2G-review-resolution.md): all **F1–F7 and T1–T4 accepted and remediated**, no deferred item. The actual rolled-back-acquisition guard escape was reproduced before restricting the recovery capability; corruption/predicate regressions first failed before repair. The normal P2F outcome and authority semantics were not weakened.

[Terminal generation 1](P2G-review-generation-1.md): **R1 correctness PASS, R2 transactions/security PASS, R3 architecture/scope PASS**. Zero remaining findings, test gaps, questions or uncovered review-relevant areas. The generation-1 validator passed with the full generation-0 report and resolution as parents. Interrupted terminal attempts produced no verdict and were not counted; the same three independent sessions resumed and completed. No production edits followed terminal PASS.

## Validation and closure

### Final implementation and API

Production surface:

```rust,ignore
impl TaskEngine {
    pub fn recover(
        &mut self,
        now: EpochMillis,
        context: &TransitionContext<'_>,
    ) -> Result<RecoveryReport, EngineError>;
}
```

- `crates/serea-task-engine/src/recovery.rs` owns public classification/report orchestration.
- `crates/serea-storage/src/recovery.rs` owns opaque raw snapshots, checked attribution/evidence, preflight and internally atomic conditional operations.
- `TaskJournal` remains the **one** production journal-semantic mapper; storage binds envelopes and persists through its existing private sink.
- The whole pass uses one finite `BEGIN IMMEDIATE` snapshot plus a pass savepoint. `RecoveryPass` exposes exactly five recovery methods, not unrestricted `Tx`, acquire/begin/outcome or SQL. This closes the generation-0 capability escape without changing ordinary guard provenance or P2F methods.
- A pass refusal returns a payload-free typed error, publishes no successful report and rolls back all pass writes. Each repair/audit operation is also failure-atomic. Reinspection rejects snapshots invalidated by earlier writes in the same transaction.
- Explicit caller time/context only: no retained Clock, wall clock, provider/model callback, executor, scheduler or worker loop.

### Final table dispositions and counters

The selected table above is implemented, with the following precise terminal dispositions:

| Durable condition | Final classification/action |
| --- | --- |
| COMPLETED / FAILED / CANCELLED | In-memory TerminalNoop; every task/step/lease/receipt/journal/time/ref/blob row unchanged |
| Invalid catalog or any FK failure | Typed whole-pass refusal before task mutation or audit |
| Writable task-attributable unknown raw state | Compatibility quarantine to BLOCKED/UNRECOGNISED_STATE; original source retained in raw inspection and digest-only observed evidence, never a guessed enum |
| Task-attributable unknown status or semantic corruption | Explicit invariant decision; legal blocking/reassessment where truthful. Known waits or independent task CHECK damage that cannot safely survive an UPDATE receive journal-only invariant evidence, preserving raw facts; no false approval/input resolution |
| Valid unreleased/unexpired lease | HeldLease; no revocation or execution, including an already spent last attempt |
| Valid expired unreleased lease | Exact observed conditional revocation; ExpiredLease records actual committed revocation. Old outcome authority is fenced |
| Released LEASED, budget available | ResumeNormally eligibility; preserve status/acquisition snapshots/attempt/generation |
| Released/expired EXECUTING, no known outcome | NeedsReconciliation once; no rerun, FAILED provider outcome, result, receipt or absence synthesis |
| Crash-only ceiling exhausted after authority release | NeedsReconciliation plus legal BLOCKED disposition. READY performs audited REPLAN then blocking in one atomic operation; no fake begin or new revision |
| Receipt with non-SUCCEEDED step | Attributable corruption, never automatic success |
| Consistent committed SUCCEEDED receipt outcome | ReceiptAlreadyCommitted observation and appropriate eligibility; no outcome/aggregate/receipt rewrite. A nonterminal task may receive first-time classification audit independent of caller observation; a terminal task is strictly write-free |
| SUCCEEDED receipt outcome with demonstrably stale aggregate | Task-only legal repair requires complete contiguous ordinary outcome audit, valid result/provenance, matching released authority, current completion=release=batch time, receipt observation<=batch time, unique destination and no later real task edge of any origin |
| Missing/contradictory receipt-repair proof | Corruption/quarantine, not reconstruction of a missing atomic batch |
| Approval / user / schedule wait | AwaitApproval / AwaitUser; journal once, no rendering/device/input/schedule action |
| Valid VERIFYING after ordinary success without a verifier | Eligibility/deferred verification; remains VERIFYING, never invented COMPLETED |
| Existing RECONCILED_ABSENT | Conservative uncertain/blocked classification; not a successful predecessor. No P2G absent writer, provider read-back or replacement synthesis |
| Empty EXECUTING/VERIFYING, inconsistent attempt/generation or acquisition-copy expiry bounds | Explicit semantic invariant classification, not false resume. Legitimate renewal remains supported |

`tasks_examined` counts inspected identities including terminal tasks. `tasks_resumed` counts distinct ResumeNormally eligibility classifications, **not execution**. `repairs_committed` counts distinct tasks with any committed durable recovery change, **including first-time decision audit**. `invariant_violations` counts attributable invalid tasks. `decisions` records ordered classifications and actual revocation observations. `pending_event_transitions` is the **final committed ALL-task_journal row count**, including recovery audit; no queue/outbox/backfill exists.

### Logical identity, races and execution exclusion

Deterministic full logical dumps include every durable table and raw typed value, including generated columns, plan revisions/references and blobs. The second pass at the same explicit now/context is byte-identical at that logical boundary: **no duplicate recovery decision, updated_at churn, repeated blocking or release-stamp rewrite**. Physical SQLite/WAL bytes are not claimed identical. Identity excludes invocation time/actor and recovery-generated journal rows, but changed durable state/generation can emit new evidence. Free-text payload is never classification or deduplication authority.

Nine file-backed tests pin both deterministic orders and barrier races for outcome versus recovery, reclaim versus recovery and two recovery callers. SQLite serialization decides the winner; no process-local race oracle or recovery mutex grants authority. Outcome-first preserves the known expired-unreclaimed outcome; recovery-first fences the old guard; reclaim-first preserves the newer generation; competing recoveries do not duplicate decisions/receipts or regress generation.

Source/dependency/call-surface guards cover engine and the storage runtime closure. The restricted capability has eight negative compile tests plus a positive usability test. **Provider/model/network/subprocess/scheduler/GoalLatch/capability invocation and ambient clock paths are absent**; receipt absence/expiry never proves external absence.

### Measured final validation

True stable is **Rust/Cargo 1.98.1**; MSRV is exact **1.85.0**. Every command completed separately with successful exit, not through a giant timeout wrapper.

| Gate | Stable | Rust 1.85.0 |
| --- | --- | --- |
| Workspace all-target/all-feature check | PASS | PASS |
| Workspace all-target tests | 801 regular, failed 0, ignored 0 | 801 regular, failed 0, ignored 0 |
| Workspace all-feature tests | **801 regular + 45 doctests = 846**, failed 0, ignored 0 | **801 regular + 45 doctests = 846**, failed 0, ignored 0 |
| Workspace all-target/all-feature Clippy, -D warnings | PASS | PASS |
| Storage debug, all features | **362 regular + 37 doctests = 399**, PASS | **399**, PASS |
| Storage release, all features | **399**, PASS | **399**, PASS |
| Engine debug, all features | **103 regular + 3 doctests = 106**, PASS | **106**, PASS |
| Engine release, all features | **106**, PASS | **106**, PASS |

Stable commands were explicitly pinned as `cargo +stable ...`; after final documentation annotations the requested unprefixed stable commands were also completed separately: `cargo check --workspace --all-targets --all-features --offline`, `cargo test --workspace --all-targets --offline`, `cargo test --workspace --all-features --offline`, and `cargo clippy --workspace --all-targets --all-features --offline -- -D warnings`. The unchanged rust-toolchain.toml selects stable1.98.1; counts remain 801 regular / 45 doctests, with zero failed/ignored. Logs: `tmp/p2g-final-default-{check,all-target,all-feature,clippy}.log`. MSRV commands use `cargo +1.85.0 ...`. Full command lines, actual exits and summary counts are retained in `tmp/p2g-final-real-stable-results.json` and `tmp/p2g-final-msrv-results.json`, with per-command logs. File-backed recovery races pass in all four engine modes.

Additional completed gates:

- `cargo fmt --all -- --check`: PASS.
- `python3 tests/workspace_smoke.py`: PASS.
- `python3 -m unittest discover -s tests -p workspace_smoke_tests.py`: **59 passed**.
- `python3 -m py_compile tools/validate_docs.py`: PASS.
- `python3 tools/validate_docs.py docs`: PASS, rerun after final closure annotations.
- `git diff --check`: PASS.
- `cargo metadata --no-deps --format-version 1 --offline`: PASS, four workspace members unchanged.
- Both immutable review-report validators: PASS; generation 1 validated with both parent artifacts.

Evidence caveats are retained, not hidden: an earlier storage-agent combined full MSRV run timed out and is not used as closure evidence; independently bounded final MSRV commands completed successfully. An initially mislabeled stable batch actually selected 1.85.0 and was **rejected as stable evidence**; all eight commands were rerun with explicit +stable and verified Rust1.98.1. Interrupted terminal attempts did not count as reviews. The local Python3 lacks tomllib; the baseline/package comparison used a dependency-free lockfile metadata comparison instead.

### Baseline preservation and scope

Baseline **704 regular + 36 doctests = 740**; final **801 + 45 = 846**: **97 new regular + 9 net doctests = 106 new executions**. All **704 baseline regular identities are preserved** on both toolchains. **35 unchanged doctest obligations** remain; the obsolete recovery-API-absence compile-fail obligation is deliberately replaced by a positive API-presence check at the same original engine doctest block/starting line. All three original engine doctest blocks still execute. No mechanically comparable baseline identity is missing; the intentional API-obligation migration is explicitly recorded rather than claimed unchanged. Comparison evidence: `tmp/p2g-counts-and-preservation.json`.

Migration 0001 is byte-identical to the parent, checksum `sha256:d9068dccbc26ececb71be79c475080633166ba0163c62b2d98b9733512baefea`; no 0002. Vendor, frozen protocol and 121-pair transition oracle are unchanged. No new external package/version or production dependency. Recovery integration fixtures reuse the existing frozen workspace `rusqlite` via **one dev-only dependency edge**; Cargo.lock changes only that workspace member's dependency list, not package metadata. This avoids exposing SQL through production Store/Tx or adding subprocess fixture machinery. Normal P2F outcome journal batches remain unchanged.

ADR-0021's **P2-side runtime gate is complete**; status stays **Proposed** because full architecture ratification/P3 event participant guarantees are not delivered. No E3/E4, event sequence/backfill or retroactive event guarantee is claimed. ADR-0022 stays Proposed and untouched. ADR-0024 remains Proposed/unratified; its expiry/known-outcome semantics are unchanged.

### Closure, commit and remaining phases

P2G is CLOSED at this scoped runtime gate. The user-authorized closure commit is the single commit containing this record, subject `feat: implement P2G recovery`, on `p2/p2g-recovery`, parent **7c05ffae3503115a069761d40201aaf84595679a**. Its actual SHA and clean-worktree verification are recorded in the final handoff; no amend or push is authorized.

P2H and P2I remain **NOT STARTED**. P2H still owns real child-process crash-window proof, bounded fault seams and deferred F25/F26 portability/crash harness work; P2I owns final non-negotiable sweep. No P2H production implementation, P3, provider/model runtime, external reconciliation, approval delivery, scheduler/worker loop, empirical Apple-Silicon execution or physical SQLite-file byte identity is claimed. Any optional post-commit P2H readiness audit is strictly read-only and reported only in the final handoff.
