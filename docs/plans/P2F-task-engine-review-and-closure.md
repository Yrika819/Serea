# P2F-b Task Engine Architecture Gate and Review/Closure Ledger

Status: **CLOSED for P2F-b TaskEngine core — all runtime/review/release gates PASS.**
Implementation, three independent reviews, accepted remediation, bounded terminal
re-review and all requested validation gates are complete. This record is finalized
by the single commit with subject `feat: implement P2F task engine core`; its identity
is read from Git after creation, not embedded self-referentially in this record. P2G is NOT STARTED.
The [P2F-a record](P2F-review-and-closure.md) remains byte-identical and continues
to own the atomic storage begin/known-outcome slice. Sections 1–2 preserve the
original gate; sections 3–5 preserve historical interruption evidence. **Section 6
is the authoritative final runtime closure**, superseding historical pending text.

## 1. Preflight and baseline

Starting branch: `p2/p2e-lease-fencing`. Starting HEAD:
`e82abd475596dce5a303ef2c3d7eaa3241b774be`
(`feat: implement P2F atomic outcome fencing`), with clean worktree.
Parent P2E: `df6088e247dcfb28dc0570d9afafda25bcde60a5`.
Created `p2/p2f-task-engine-core` from that exact HEAD. No amend or push.

Both commands passed before branch creation:

- `cargo test --workspace --all-features --offline`
- `cargo +1.85.0 test --workspace --all-features --offline`

Count-confirmation reruns on the new branch independently recorded **593 regular
+ 31 doctests = 624 executions**, zero failed/ignored, on both toolchains.
Local logs: `tmp/p2fb-baseline-stable.log`, `tmp/p2fb-baseline-msrv.log`.
These are baseline results, not evidence for the future engine implementation.

## 2. Frozen preimplementation findings and dispositions

The following decisions precede any engine production source. Two independent
read-only investigations covered journal/transaction layering and lifecycle/plan
contracts. They are architecture research, **not** the three required post-GREEN
reviews. The coordinator reconciles their recommendations with the bounded scope
here; recommendations to implement waits or reconciled-absent closure are not adopted.

| ID | Finding | Selected disposition |
| --- | --- | --- |
| B1 | Storage's private outcome journal and an engine journal would be two authorities | Replace private production semantic mapping with exactly one engine-owned `TaskJournal`. Storage constructs immutable actual-write facts and privately persists the participant's journal drafts in the SAME method savepoint/outer transaction. No fallback production mapper and no duplicate replay at outer commit. |
| B2 | Historical participant exposes raw rusqlite upward | Replace that sketch with a storage-owned, synchronous facts-to-journal-record port. Participant receives neither Connection, Transaction, Tx nor SQL executors. Engine runtime depends only on protocol/storage. |
| B3 | Outcome/context names already belong to storage | Engine intentionally re-exports `StepOutcome`, `StepFailure`, `StepCommit`, `TransitionContext`, and `LeaseGuard`. Storage remains the one authority for known-outcome/fencing semantics. |
| B4 | Old surface includes recovery and reconciled-absent | Omit both, including stubs, fake reports and empty placeholders. Recovery and `RECONCILED_ABSENT` writer belong to P2G absent an independent non-recovery requirement. |
| B5 | No operation connects RECEIVED to PLANNING | Creation inserts observable RECEIVED only. Explicit `start_planning` performs a conditional whole transition. For revisions it also permits the frozen READY/WAITING_USER/BLOCKED to PLANNING edges, with explicit expected state/revision. |
| B6 | Old signatures hide time and attribution | Every timestamp-bearing mutation uses caller-supplied `EpochMillis`; every audited mutation receives explicit `TransitionContext`. Creation carries created_at in NewTask and uses it for created/updated/audit time. No Clock retained by engine; no ambient time. |
| B7 | Engine has no accepted entropy authority | Caller supplies validated TaskId/StepId. Existing protocol IdMinter remains host-side, never implicitly invoked by engine. Revision identity is the contiguous integer, not a minted identifier. |
| B8 | Historical query API inverted layering | Storage owns TaskSnapshot/StepSnapshot/ReceiptSnapshot and narrow read-only, consistent-snapshot loading. Engine maps to TaskRecord/StepRecord. No engine names, StepPhase, Plan, EngineError, raw SQL or arbitrary filter language in storage. |
| B9 | matches! is claimed compile-exhaustive | Correct the claim. Implement an exhaustive OUTER match over every TaskState and independently test all 121 pairs against the literal frozen table. Pin the test's eleven names against TaskState::WIRE_NAMES; protocol has no TaskState::ALL today. |
| B10 | I3 refusal and I4 remediation are conflated | Pure illegal-edge validation refuses without persistence. Separate `fail_invariant` whole transition writes FAILED/INVARIANT_VIOLATION from an eligible expected nonterminal state. Terminal tasks never leave their state. No raw state setter, and no secret remediation side effect on an invalid request. |
| B11 | PRIVATE blob injection is mistaken for PRIVATE row support | PUBLIC/PERSONAL only for every ordinary-row writer, including all nested extensions, prose and audit payloads. PRIVATE/SECRET/CREDENTIAL fail closed before ordinary content is written, even with blob protection. ADR-0022 remains Proposed. |
| B12 | Planner could admit layouts outcomes cannot process | Sort by sequence for validation; ordinary prefix followed by VERIFY suffix only. All ordinary accepted; VERIFY then ordinary and alternating layouts refused. No verifier still ends ordinary completion at VERIFYING, not COMPLETED. Empty and VERIFY-only plans are refused until a legal entry/completion contract exists. |
| B13 | greater revision versus increments | Exactly current + 1 with checked arithmetic. Initial stored revision is 0; first plan is 1. Reject stale/skipped/overflow revisions. Persistence is PLANNING-only, with SQL expected state/revision predicates. |
| B14 | Global unreferenced sweep deletes standalone blobs | Capture this operation's candidate digest/class identities before cascade. Delete only those candidates with no surviving task refs, step refs OR revision references. Shared/other-task/unrelated standalone blobs survive. Same rule for removed planned steps. |
| B15 | Deletion audit row cannot survive task cascade | No TASK_DELETED kind. Return durable DeletionOutcome counts only after outer commit. Missing task is idempotent zero-count deletion. Existing journal rows disappear by design. |
| B16 | No capability descriptor exists for risk comparison | Persist immutable host-assigned policy_class. Descriptor comparison, provider-reference requirements and effect-required receipts remain P5. No registry or duplicated descriptor metadata. |
| B17 | task_steps has no extensions column | Retain complete checked step specifications in full plan-revision blobs. Storage reconstructs step extensions by original step plan_revision/StepId provenance, then combines them with current durable runtime columns. No silent extension loss and no migration change. Missing/inconsistent provenance fails closed. |
| B18 | Lease-only P2E has no audit attribution | Engine acquire/release use narrow audited storage whole-operation wrappers: delegate unchanged P2E predicates and inputs, build actual lease facts after successful writes, record via the same participant/savepoint. Keep existing low-level P2E APIs for their original lease-only contract. No engine-memory fence preflight. |

### 2.1 Audit seam and failure semantics

Selected port sketch, **not runtime implementation evidence**:

```rust,ignore
pub trait TaskAuditParticipant: Send + Sync {
    fn records(&self, facts: &DurableTransition<'_>)
        -> Result<JournalRecords, StoreError>;
}
```

`DurableTransition` has storage-private construction and read-only accessors.
It describes actual writes in an open transaction, not already-durable results.
A closed operation discriminator distinguishes insertion, planning entry, plan
persistence, acquisition/release, begin, known success/failure, blocking,
cancellation and invariant failure. Facts carry bound task/optional step,
actual task/step pre/post states, optional acquisition attempt/generation,
actual result/revision/receipt identities, typed cause, explicit timestamp,
task-derived class and supplied actor/version/causation. They contain no guard,
owner authority, model/provider result prose or event object. Receipt evidence
is present only after a checked successful receipt INSERT.

`Store::transact_with_audit` supplies exactly ONE audit participant per outer
transaction. No list/registry/participant-local pending queue. Storage dispatches
it after each whole operation's successful state-write body, inside that
operation's savepoint, and privately persists the returned ordered nonempty
batch. Storage binds the envelope to the actual facts, canonicalizes payload
JSON, computes its digest, allocates task-local MAX+1 sequence, and checks every
expected single-row write. A mapper cannot independently append rows, change
lifecycle state, open another transaction or submit stale detached facts.

A failed operation body does not invoke its participant. Participant/late SQL
failure rolls back the entire method, even when caught by a caller that commits
unrelated surrounding work. Cleanup/RELEASE failure makes the outer transaction
rollback-only. Mapper panic receives the same cleanup/resumed-unwind behavior.
A later outer body Err may occur AFTER an earlier successful method's mapper ran;
all those SQL rows roll back. The historical absolute statement that outer body
Err never invokes any participant is therefore replaced by **no durable journal
on outer body Err**, and no invocation for a failed method's state-write body.
This timing reconciliation is necessary to preserve P2F-a's catchable method
journal-failure guarantees; an outer precommit-only mapper would weaken them.

Existing P2F-a kinds, ordering, attribution and payload contents remain equivalent:
begin produces STEP_ATTEMPT_STARTED and conditional TASK_STATE_CHANGED; success
produces STEP_COMMITTED, actual optional RECEIPT_RECORDED, conditional task change
and terminal evidence; failure produces STEP_FAILED, task change and terminal
evidence. Existing attempt/generation/result-digest payload remains intact.
No row on a no-op, and no second journal path at outer commit.

Migration of existing storage-only begin/outcome callers is explicit: supply the
participant through the audited transaction entry point. Plain transactions with
no participant must fail closed before an audited lifecycle write. Storage tests
use a local cfg(test) participant double with independent literal expectations;
engine integration tests separately use the REAL TaskJournal. No storage-to-engine
dependency, including a dev edge, is needed. A test double is not a production
fallback or a second runtime semantic authority. Preserve every existing fencing,
catch-and-continue, cleanup, panic, COMMIT failure, cross-Store and reopen test.

P3 will extend the explicit dispatch with a separate event-specific capability,
using the same actual facts and transaction in fixed order. That capability is
NOT implemented now; journal drafts are not a generic future SQL hook.

### 2.2 Public surface and read representations

Chosen API sketch, **pending implementation; initial genuine RED tests now written**:

```rust,ignore
TaskEngine::new(store: Store) -> TaskEngine
create_task(spec: NewTask, context: &TransitionContext) -> Result<TaskRecord, EngineError>
start_planning(task_id: TaskId, expected_state: TaskState, expected_revision: u32,
               now: EpochMillis, context: &TransitionContext) -> Result<TaskRecord, EngineError>
persist_plan(task_id: TaskId, plan: Plan, now: EpochMillis,
             context: &TransitionContext) -> Result<PlanRevision, EngineError>
acquire(task_id: TaskId, step_id: StepId, owner: LeaseOwner,
        expected_generation: Option<u32>, now: EpochMillis, expires_at: EpochMillis,
        context: &TransitionContext) -> Result<LeaseGuard, EngineError>
begin_attempt(guard: &LeaseGuard, now: EpochMillis,
              context: &TransitionContext) -> Result<(), EngineError>
commit_step(guard: LeaseGuard, outcome: StepOutcome, now: EpochMillis,
            context: &TransitionContext) -> Result<StepCommit, EngineError>
release(guard: LeaseGuard, now: EpochMillis, context: &TransitionContext) -> Result<(), EngineError>
block(task_id: TaskId, expected_state: TaskState, reason: BlockedReason,
      now: EpochMillis, context: &TransitionContext) -> Result<TaskRecord, EngineError>
fail_invariant(task_id: TaskId, expected_state: TaskState,
               now: EpochMillis, context: &TransitionContext) -> Result<TaskRecord, EngineError>
cancel(task_id: TaskId, by: TaskOriginKind, now: EpochMillis,
       context: &TransitionContext) -> Result<CancellationOutcome, EngineError>
delete_task(task_id: TaskId) -> Result<DeletionOutcome, EngineError>
load(task_id: TaskId) -> Result<TaskRecord, EngineError>
```

All mutation receivers are `&mut self`; load uses `&self`. Begin returns unit;
callers explicitly load a read model when needed, rather than promising a stale
read-after-commit snapshot. Outcome returns the existing StepCommit only AFTER
outer commit. Outcome/release consume the guard on every result and never retry
or return replacement authority. Engine does not expose Store/Tx mutably.

NewTask contains TaskId, TaskKind, TaskTitle, TaskOrigin, DataClass, RiskClass,
AttemptBudget, explicit created_at EpochMillis, optional deadline_at EpochMillis
and protocol Extensions. No initial state/progress/lease/terminal fields.
TaskRecord holds an AssistantTask projection plus plan_revision and ordered
StepRecords; StepRecord holds a checked TaskStep plus its original plan_revision.
No mutable storage references. Extensions of task, origin, budget and steps
round-trip; reserved member collisions are refused, never flattened into authority.

Plan is a complete desired membership snapshot: caller revision plus ordered
`PlanStep { step: TaskStep, input_json: Vec<u8> }`. Validate every supplied step
and raw input BEFORE mutation. New steps are PLANNED/attempt0/generationNone;
wait kinds are constructible. Retained steps preserve parent, sequence, kind,
capability tuple, canonical input, key, extensions and ALL durable runtime facts.
They cannot be reset by submitting a planned copy. A semantic replacement needs
removal of a still-PLANNED step and a fresh appended StepId.

Capability-shaped input requires object-root original arguments JSON; other
inputs are SCJ-1 instruction documents. Supplied input_digest and capability key
must equal existing protocol digest/IDK-1 derivation. Duplicate IDs/sequences/keys,
wrong parents, unknown statuses, forged runtime fields, wrong layout or unsupported
classes refuse without writes. Input references use ARGUMENTS for capability-shaped
steps and INSTRUCTION otherwise. No fractional coercion/lossy raw-JSON admission.

Append eligibility uses the maximum historical sequence across retained full
revision blobs, not a maximum recomputed after deletions. Previously used StepIds
cannot be reintroduced. This lifetime high-water choice prevents suffix deletion
from making old sequence positions reusable without adding a migration column.
Gaps are allowed; numbering need not be contiguous. Old revision blobs remain
referenced. PLAN denotes the current full plan; PLAN_REVISION plus revision rows
preserve history. Whole-operation savepoint covers blobs, all refs, row changes,
revision, state and audit. Candidate sweeps are scoped to removed steps only.

### 2.3 Transition reasons, block, waits and cancellation

One exhaustive transition-rule function returns a typed rule or None; boolean
legality delegates to it. Rule classification distinguishes StartPlanning,
Replan, PlanPersisted, StartExecution, ContinueExecution, EnterVerification,
VerificationRequiresExecution, VerificationComplete, AwaitApproval, AwaitUser,
ResumeReady, Block, Cancel and Fail. One mapping renders stable codes; specific
BlockedReason/FailureReason causes remain validated protocol types, not prose.
No destination-only mapping erases supplied failure/bound causes.

Block sources are PLANNING/EXECUTING/VERIFYING only, conditional on expected state.
It updates task metadata/audit, not effects or steps. No speculative resume-ready
method. Planning entry provides the explicit permitted BLOCKED to PLANNING path.
WAITING_USER/WAITING_APPROVAL edges remain in the relation, but no settled named
P2F operation owns entering/resolving them; do NOT invent public wait/scheduler/
approval writers. Test cancellation from such states using storage-private fixtures.
Unknown durable state/status repair is P2G; reads fail closed, not fabricated quarantine.

Cancellation from all eight nonterminal states sets state/time/by and clears
incompatible block/failure fields, touching NO step or receipt. All three terminal
states return changed=false/already_terminal=true/cancelled_at=None with no timestamp,
updated_at rewrite or audit append. Second cancellation is the same no-op.
Deletion returns counts for task, steps, receipts, leases, revisions, task/step
refs, journal and removed candidate blobs; no surviving task-journal deletion row.

### 2.4 Frozen literal oracle

Ordering below is independent of protocol enum declaration order:
RECEIVED, PLANNING, READY, EXECUTING, WAITING_APPROVAL, WAITING_USER, VERIFYING,
BLOCKED, COMPLETED, FAILED, CANCELLED.

```text
       R P Y E A U V B C F X
R      0 1 0 0 0 0 0 0 0 1 1
P      0 0 1 0 1 1 0 1 0 1 1
Y      0 1 0 1 1 0 0 0 0 1 1
E      0 0 1 0 1 1 1 1 0 1 1
A      0 0 1 0 0 0 0 0 0 1 1
U      0 1 1 0 0 0 0 0 0 1 1
V      0 0 0 1 0 0 0 1 1 1 1
B      0 1 1 0 0 0 0 0 0 1 1
C      0 0 0 0 0 0 0 0 0 0 0
F      0 0 0 0 0 0 0 0 0 0 0
X      0 0 0 0 0 0 0 0 0 0 0
```

37 legal, 84 illegal; all eleven self-edges illegal. A step mutation that leaves
the task state unchanged is bookkeeping, not a legal task self-transition.

## 3. Historical TDD and interruption ledger

This section preserves the recorded test-first and first-eight-GREEN boundaries;
its absent-library and pending-review statements describe those earlier points,
not the final tree. See §6 for completed implementation and review evidence.

First genuine engine RED: `cargo test -p serea-task-engine --test core --offline`.
Actual compiler error **E0432: unresolved import serea_task_engine**, because the
new test-only package has no production library. A confirmation rerun records
Cargo's actual exit **101** in `tmp/p2fb-last-red-result.json` and compiler output
in `tmp/p2fb-last-red.log`; first output is `tmp/p2fb-first-red.log`.
Eight real tests in `crates/serea-task-engine/tests/core.rs` precede production
code: literal 121-pair relation/reason oracle, terminal edges, creation projection,
load/nested-extension round-trip, explicit planning/stale-writer refusal, initial
plan/step-extension persistence, unsupported ordinary classes and unstarted wait
kind constructibility. They are **not yet executed successfully**; the oracle's
121 assertions must not be reported as 121/121 PASS.

The crate currently contains only Cargo.toml and tests/core.rs; metadata explicitly
shows a test target and NO library target. No deliberately broken implementation,
placeholder methods, recovery stub or fake report exists.

Separate smoke TDD: the delegated integration contributor ran all 37 old Python
cases first, added 22 cases before checker edits, observed 59 tests/84 failures
(including subtests), then 59 passed after checker changes. The coordinator then
ran the real four-member smoke and all 59 cases successfully. All 37 original
methods were preserved. This is workspace-checker GREEN only, **not** engine GREEN.

First eight-test engine GREEN reached on resume: `cargo test -p serea-task-engine
--test core --offline` completed with **8 passed, 0 failed/ignored** under stable
`rustc 1.98.1 (48a229cea 2026-09-01)`. The original eight tests are unchanged;
all 121 relation/reason assertions now pass. This is **not overall P2F-b GREEN**.

Resume integration defects encountered and repaired before this milestone:
storage's new modules were not wired into lib.rs; deletion attempted direct
`u64` SQLite decoding unsupported by rusqlite (now checked i64-to-u64); the
old whole-operation inventory needed to admit the selected P2F-b surface.
Deletion now shares candidate capture/sweep mechanics with plan revision.
Storage stable debug unit suite at this point: **306 passed, 0 failed/ignored**,
including all existing outcome/fencing cases and the explicit audit seam tests.
Production engine creation/planning/persistence/load and typed relation are now
implemented. Wrappers and lifecycle integration, expanded tests, all independent
reviews and final validation remain pending. No closure or final commit claim.

Required post-GREEN lifecycle/plan/journal, transaction/security and architecture/
scope reviews: **not started**. No frozen postimplementation findings, remediation
or terminal re-review exist. Preimplementation research must not be counted as them.

Mandatory coverage still pending: all applicable I1–I15/J1–J15/K1–K8/L1–L10,
H wrappers and G reference/deletion integration, plus §18.1 regressions. I9 and K4
recovery scans are P2G; K8 crash-window proof is P2H, with ordinary rollback here.
No baseline test may be removed or ignored to accommodate audit migration.

## 4. Historical interrupted-tree validation and ADR status

The following is the preserved earlier scaffold validation, not final validation.
Current release results and ADR disposition are in §6.

Baseline GREEN is recorded above. On this interrupted test-first tree:

| Check | Actual evidence |
| --- | --- |
| cargo fmt --all followed by cargo fmt --all -- --check | PASS |
| python3 tests/workspace_smoke.py | PASS exact four manifests and current dependency boundaries |
| python3 -m unittest discover -s tests -p workspace_smoke_tests.py | PASS59; 37 preserved, 22 added |
| python3 -m py_compile tools/validate_docs.py | PASS |
| python3 tools/validate_docs.py docs | PASS61 Markdown files, including a successful post-publication rerun |
| git diff --check | PASS separate completion run |
| cargo metadata --no-deps --format-version 1 --offline | PASS four packages; engine has ONLY test target |
| storage stable debug | PASS255 unit + 2 integration + 26 doctests = 283 |
| storage stable release | PASS255 unit + 2 integration + 26 doctests = 283 |
| storage Rust1.85 debug | PASS255 unit + 2 integration + 26 doctests = 283 |
| storage Rust1.85 release | PASS255 unit + 2 integration + 26 doctests = 283 |
| engine focused core test | RED E0432/exit101, absent library/API |

All four storage modes include the 83 P2F-a cases, zero failures/ignored, and no
count discrepancy. Logs are `tmp/p2fb-storage-{stable,msrv}-{debug,release}.log`.
No storage production/test source, migration, vendor or frozen protocol source
was edited, and P2F-a's closure record remains byte-identical.

Additional preservation command is explicitly **excluding the absent engine**:
`cargo test --workspace --exclude serea-task-engine --all-features --offline`.
Stable completed successfully with all 624 executions; MSRV companion reported
all 593 regular tests passed and reached the doctest stage when the
180-second combined wrapper timed out. Do not call the timed-out combined command
PASS. Both original preflight baselines and the separate MSRV storage modes above
are complete results. The excluded-engine run is not final four-crate validation.

An earlier 20-second smoke/docs/diff wrapper also timed out AFTER reporting
smoke, 59 tests and doc validator PASS. Separate fmt/diff/metadata completion ran
successfully; both wrapper timeouts are retained rather than erased.

Full final check/all-target/all-feature/Clippy/MSRV and engine debug/release gates
have NOT passed; no engine runtime or P2F-b closure PASS is claimed. Do not conceal
the absent implementation behind successful baseline/storage/smoke tests.

ADR-0021 remains **Proposed**: this records a selected P2-side seam, not implemented
participant/recovery/P3 guarantees. ADR-0022 remains **Proposed**. ADR-0024 remains
**Proposed**: P2E/P2F-a runtime evidence exists, but engine integration/deletion and
full architecture ratification cannot be inferred from it.

## 5. Historical remaining obligations and stop discipline

The following pending/resume statements record the original interruption only.
They are superseded by §6; deferred P2G/P3 obligations remain deferred.

P2F-b is NOT CLOSED and NOT ready for P2G. RecoveryReport/RecoveryDecision,
reconciled-absent writer, quarantine, authority revocation, recovery idempotence
and recovery journal accounting remain P2G. Crash/SIGKILL is P2H. Event participant,
E3/E4, sequence, event tables/backfill, provider/model calls, workers, approvals,
scheduler, registry comparison, production PRIVATE rows and empirical Apple Silicon
validation are explicitly unclaimed and not started.

Do not create the feature commit until all implementation, three independent
reviews, bounded terminal re-review and final validation gates pass. Do not amend
P2F-a or push. This run stops at an explicit **test-first interruption boundary**:
no next feature or P2G work is begun, no feature commit is created, and P2F-b is OPEN.

### Historical exact resume point

Branch `p2/p2f-task-engine-core`; HEAD remains
`e82abd475596dce5a303ef2c3d7eaa3241b774be`. Worktree intentionally dirty with this
gate, corrected architecture docs, test-only engine scaffold and four-member
workspace/smoke/CI integration. First/last known engine result is the same E0432 RED;
there is no engine GREEN or completed postimplementation review.

Resume by reading this frozen gate and `crates/serea-task-engine/tests/core.rs`.
Implement the narrow storage audit port/private sink with test-first seam cases;
remove the production outcome_journal mapper only when all audited callers/tests
are migrated coherently. Implement the actual engine library/typed transition
relation, consumer-led snapshots and whole lifecycle/plan/deletion operations under
that one audit authority. Preserve P2F-a semantics. Expand mandatory contract tests
before corresponding features. No empty library or permissive fallback counts as
the repair. Once the COMPLETE focused engine suite first goes GREEN, freeze
features, run the three independent read-only reviews and follow the requested
bounded remediation/final validation/one-commit closure procedure.

Dirty tracked paths: root Cargo.toml/Cargo.lock; .github/workflows/ci.yml;
ADR-0021; P2-storage-task-engine/P2-test-matrix/P2-contract-gap-analysis/
P2-tomorrow-decision-ledger; tests/workspace_smoke.py/workspace_smoke_tests.py.
New paths: engine Cargo.toml/tests/core.rs and this OPEN ledger. No new third-party
version or vendor change: serde_json is dev-only in the new test package; runtime
manifest dependencies are protocol/storage only.

## 6. Final P2F-b runtime closure

### 6.1 TDD, feature freeze and baseline preservation

The genuine E0432/exit101 RED above remains the first RED. No fake implementation
or weakened initial assertion was used. The original eight test names and
assertions remain; formatting is not a behavioral change. First-eight GREEN was
8/8 on stable `rustc 1.98.1 (48a229cea 2026-09-01)`. Expanded focused tests went
GREEN before feature freeze:

- First overall focused GREEN: `cargo test -p serea-storage -p serea-task-engine
  --offline`, **381 executions**, `tmp/p2fb-focused-first-green.log`.
- Accepted-remediation focused GREEN: same command, **399 executions**,
  `tmp/p2fb-remediation-green.log`.
- Final full workspace, stable and Rust 1.85: **704 regular + 36 doctests = 740**,
  **0 failed, 0 ignored**. Baseline was **593 + 31 = 624**: **116 new executions,
  0 missing baseline identities** on each toolchain. Identity comparison preserves
  multiplicity and normalizes doctest line-number shifts, not names or outcomes.
- All **83 P2F-a outcome/fencing cases** remain GREEN. Its closure file is unchanged.

Before first overall GREEN, real integration defects included module wiring,
unsupported direct SQLite u64 decoding, absent error-details decoding (omitted
rather than null), class/overflow refusal precedence and validating the returning
projection before invoking the mapper. These were repaired without widening scope.

### 6.2 Delivered ownership, public API and behavior

Production files are `crates/serea-task-engine/src/{lib,engine,error,journal,
transition,types}.rs`. Runtime dependencies are **serea-protocol and serea-storage
only**; serde_json is dev-only. Storage adds the existing workspace serde/serde_json
versions for checked persistence; no new third-party version or vendor change.
There is no storage-to-engine edge, including dev dependencies, and no runtime
edge to testkit. The exact workspace is:

```text
serea-protocol
serea-storage -> serea-protocol
serea-task-engine -> serea-storage, serea-protocol
serea-testkit -> serea-protocol (test utility; consumer use is dev-only)
```

Public named operations: `new`, `create_task`, `start_planning`, `persist_plan`,
`acquire`, `begin_attempt`, `commit_step`, `release`, `block`, `fail_invariant`,
`cancel`, `delete_task`, `load`. No arbitrary state setter, recovery, reconciled-
absent, wait-state writer, execution, scheduler, provider or worker API exists.
Compile-fail doctests pin absent recovery/execution and private storage capability.

- **Creation:** caller TaskId, observable RECEIVED, immutable host policy class,
  explicit timestamps/context and complete nested extensions. Duplicate ID refuses
  deterministically. Task and TASK_INSERTED audit commit atomically.
- **Planning:** RECEIVED/READY/WAITING_USER/BLOCKED entry to PLANNING uses expected
  state and revision inside SQL predicates. Stale writers persist nothing.
- **Plan persistence/revision:** contiguous checked +1 revision, PLANNING only;
  complete protocol presence/capability/raw-input/digest/IDK validation before SQL.
  Unique parent/ID/sequence/key, lifetime sequence high-water and no removed-ID
  reuse; retained runtime/specifications/provenance preserved. Only still-PLANNED
  rows may be removed; append cannot renumber or change prerequisite meaning.
  Ordinary prefix / VERIFY suffix required. Empty/VERIFY-only and interleaving
  refuse. Plans that would publish unadvanceable READY membership also refuse;
  no outcome predicate or state-machine edge was changed to accommodate them.
- **Atomic plan set:** full canonical plan/revision blob, revision row, PLAN and
  PLAN_REVISION refs, step rows/input refs, task revision/state and audit. Late ref,
  audit or required-delete failures restore the complete prior set even when caught.
  Previous revisions retain removed-step specifications and inputs.
- **Reads:** storage owns checked TaskSnapshot/StepSnapshot/PlanRevisionSnapshot;
  engine maps TaskRecord/StepRecord. Narrow consistent-snapshot load, no raw SQL or
  arbitrary query language. Current runtime plus original revision provenance
  reconstructs step extensions. Missing/inconsistent provenance, unknown states,
  corrupt receipt bindings and malformed rows fail closed. Individual documents
  retain their bounds; assembling a projection does not impose an accidental deeper
  canonical-JSON bound on an already-admitted P2F-a failure document.
- **Lease/outcome wrappers:** exact caller inputs delegate to P2E/P2F-a SQL authority;
  no in-memory fence validation/cache. Begin borrows and does not charge attempts;
  outcome/release consume guard on every result. Results publish only after outer
  commit; no retry, reusable authority on failure or effect inference.
- **Block/invariant:** block only PLANNING/EXECUTING/VERIFYING, typed reason and
  expected-state write. Explicit fail_invariant is separate FAILED remediation.
- **Cancellation:** all eight nonterminals supported; typed actor/time, incompatible
  metadata cleared. All three terminals and second cancel are true no-ops: no
  fabricated timestamp, updated_at rewrite or audit; steps/receipts untouched.
- **Deletion:** deterministic counts for task, steps, receipts, leases, revisions,
  refs, journal and swept blobs. Missing task returns zero-count no-op. Capture only
  this task's candidate identities before cascade; sweep only if no surviving task,
  step or revision reference exists. Shared/another-task/unrelated standalone blobs
  survive. No global GC and no surviving TASK_DELETED journal row.
- **Classification:** PUBLIC/PERSONAL ordinary persistence accepted; PRIVATE,
  SECRET and CREDENTIAL fail closed, including extensions/prose/audit and even with
  a PRIVATE blob backend. No complete ordinary-row protection claim.
- **Errors:** reachable EngineError categories wrap/map storage refusals with safe
  Display/Debug/source; no title, argument/result/error prose, raw JSON or backend
  diagnostic disclosure.

No protocol source, migration or vendor file changed. Migration 0001 is unchanged;
no new migration, task_steps extension column, event table or event sequence exists.

### 6.3 One journal authority and transaction failure safety

**TaskJournal in task-engine is the one production semantic mapper.** Storage's
old private outcome_journal mapping is removed; cfg(test) TestAudit is explicit
storage test infrastructure, never a production fallback. StepOutcome/StepFailure/
StepCommit/TransitionContext/LeaseGuard remain storage-owned and intentionally
re-exported; lease and known-outcome semantics have one authority.

`Store::transact_with_audit` supplies exactly one SQL-free participant. Storage
privately creates immutable actual-write facts, invokes the participant inside the
whole operation's savepoint, binds and validates drafts, computes canonical payload
digests/task-local sequence and inserts through private SQL. No Connection,
Transaction, Tx, executor, public append or detached fact submission leaks upward.
No generic registry or second outer-commit replay. Begin/outcome order, attribution,
causation, attempt/generation/result evidence and optional receipt rows are retained.
Audited release generation is guard evidence only after authoritative SQL succeeds.

Tests prove: failed bodies do not dispatch; mapper Err/invalid drafts/late SQL errors
roll back the method even when caught; panic cleans up and resumes unwind; cleanup
failure poisons outer commit; later outer Err rolls back earlier state AND audit;
no-op/fenced operations append nothing; failed COMMIT publishes no durable success.
No EventKind, Seq or SereaEvent is constructed. P3's explicit event extension remains
design only; no forward or retroactive E3/E4 claim.

### 6.4 Matrix coverage and independent closure

Applicable groups are closed by the engine integration tests plus storage's private
schema/transaction/corruption fixtures; SQL is not exposed for test convenience:

| Matrix | Runtime coverage / disposition |
| --- | --- |
| I1–I8 | Independent literal 121-pair legality AND typed reason/code oracle; 37 legal/84 illegal; terminal outgoing zero; refusal distinct from remediation; immutable policy/monotonic classification/generated-label schema fixtures |
| I9 | Recovery's unknown-state quarantine writer deferred to P2G; current reads refuse corruption |
| I10–I15 | Durable ceilings/fence distinction through wrappers, reopen/projection equality and checked-in task-schema validation |
| J1–J15 | Initial/unstarted/in-flight/terminal/reopen; unique sequences/parents; append/removal/high-water/renumber/drop refusal; historical revision recovery; wrong-task fencing; same-task key refusal and production-schema non-null cross-task key acceptance |
| K1–K3, K5/K5b/K5c, K6/K7/K7b | Receipt/success/task ordering and restart, owning capability/key checks, receipt/key schema constraints, provider-reference policy deferred to P5, correlated candidate-only sweep |
| K4 | Recovery scan deferred to P2G; checked current snapshot binding still refuses inconsistent receipts |
| K8 | Ordinary method/outer rollback pinned now; crash/SIGKILL window proof is P2H, not claimed here |
| L1–L10 | Eight-state cancel, terminal/second no-op, metadata/reopen/receipt preservation; deletion cascades/counts and candidate-only sweep |
| G10/G17/G18 and H integration | Atomic owned blob/ref attachment, shared/standalone survival, engine delegated acquire/begin/outcome/release/delete and cross-Store stale-worker tests |
| §18.1 applicable P2F regressions | Wait PLANNED construction, original raw input/IDK, retained extensions/provenance, protected-class refusal, failure atomicity and baseline fencing preservation |

After first overall focused GREEN, features froze. Three independent read-only
actual-implementation reviews completed: **A lifecycle/plan/journal**, **B transaction/
security**, **C architecture/scope/portability**. Their fixed synthesis is
[generation 0](P2F-task-engine-review-generation-0.md). Four Major and one Minor code
findings plus six Minor test gaps were accepted and frozen before remediation.
Every accepted defect had a genuine regression RED before its causal fix. No item
was deferred and no new feature/recovery work was authorized.

[Resolution](P2F-task-engine-review-resolution.md) records F1 READY membership,
F2 receipt tuple, F3 depth composition, F4 release authority facts, F5 required PLAN
reference count, and T1–T6 literal mapper/reason/runtime/rollback/admission/key tests.
The independent bounded [terminal generation 1](P2F-task-engine-review-generation-1.md)
reports **PASS: zero findings, test gaps, open questions or open coverage areas**.
Both fixed reports passed their lineage validators. They are not rewritten as
part of this documentation-only closure update.

### 6.5 Final stable/MSRV/debug/release evidence

All following commands completed separately with exit0 on the reviewed production
tree. Documentation-only final annotations are followed by hygiene/fmt reruns.

| Command | Result |
| --- | --- |
| cargo fmt --all -- --check | PASS |
| cargo check --workspace --all-targets --all-features --offline | PASS |
| cargo test --workspace --all-targets --offline | PASS704 regular |
| cargo test --workspace --all-features --offline | PASS740 executions |
| cargo clippy --workspace --all-targets --all-features --offline -- -D warnings | PASS |
| cargo +1.85.0 check --workspace --all-targets --all-features --offline | PASS |
| cargo +1.85.0 test --workspace --all-targets --offline | PASS704 regular |
| cargo +1.85.0 test --workspace --all-features --offline | PASS740 executions |
| cargo +1.85.0 clippy --workspace --all-targets --all-features --offline -- -D warnings | PASS |
| python3 tests/workspace_smoke.py | PASS exact four-member graph |
| python3 -m unittest discover -s tests -p workspace_smoke_tests.py | PASS59; all37 original cases retained |
| python3 -m py_compile tools/validate_docs.py | PASS |
| python3 tools/validate_docs.py docs | PASS |
| git diff --check | PASS |
| cargo metadata --no-deps --format-version 1 --offline | PASS; exact graph independently asserted |

The focused commands use `cargo test -p serea-storage --offline` and `cargo test
-p serea-task-engine --offline`, each also run with `--release` and with
`cargo +1.85.0` in both profiles:

| Crate | Stable debug | Stable release | Rust1.85 debug | Rust1.85 release |
| --- | --- | --- | --- | --- |
| serea-storage | PASS349 | PASS349 | PASS349 | PASS349 |
| serea-task-engine | PASS50 | PASS50 | PASS50 | PASS50 |

Storage each: 319 unit + 2 integration + 28 doctests. Engine each: 47 integration
+ 3 compile-fail doctests. No debug/release discrepancy, failure or ignored test.

Evidence logs/result ledgers: `tmp/p2fb-final-{stable,msrv,focused,hygiene}-results.json`,
`tmp/p2fb-final-*.log` and `tmp/p2fb-counts-and-preservation.json` (local ignored
artifacts; durable counts/commands are recorded here). The combined validation
wrapper hit its 20-minute bound while compiling MSRV storage release, AFTER full
stable/MSRV groups and the first five focused modes completed. That wrapper is
**not** labeled PASS. Remaining focused modes were explicitly rerun to completion
with `python3 tmp/p2fb-validate.py focused --remaining`; all eight mode results are
complete exit0. Earlier historical timeouts in §4 likewise remain recorded.

### 6.6 ADR disposition, remaining obligations and final stop

- **ADR-0021 Proposed:** P2 state/task-journal seam and participant runtime gate
  substantially satisfied. P2G recovery accounting/idempotence and P3 E3/E4 remain
  unimplemented; no mechanical acceptance.
- **ADR-0022 Proposed:** PRIVATE blob capability does not protect ordinary rows;
  complete PRIVATE representation remains unavailable.
- **ADR-0024 Proposed:** P2E authority, P2F-a fences and P2F-b wrapper/deletion
  integration verified. Full architecture ratification/recovery is not inferred.

P2F-b's runtime/review/release gates are complete; the single final feature commit
is authorized on `p2/p2f-task-engine-core`, directly parented by starting e82abd4.
After commit and clean-worktree confirmation, **P2F-b is CLOSED and ready for P2G**.
This does not start P2G. No amend, preliminary feature commit or push is authorized.

Remaining P2G work: RecoveryReport/RecoveryDecision, unknown-state quarantine,
reconciliation/reconciled-absent writer, recovery authority revocation, idempotence
and pending-event/journal accounting. Crash/SIGKILL harness remains P2H. P3 event
participant/E3/E4/event seq/backfill, execution, model/provider runtime, worker loop,
scheduler, approvals/capability registry and P5 risk comparison are not delivered.
No production PRIVATE task support or empirical Apple Silicon validation is claimed.
**STOP after the single commit and clean worktree.**
