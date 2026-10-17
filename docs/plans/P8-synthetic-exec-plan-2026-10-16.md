# P8 Synthetic ExecPlan — 2026-10-16

**Repository:** `Yrika819/Serea`
**Status:** IN PROGRESS; development-only, no production effect
**Dependency base:** Draft PR #19, `codex/post-gate-c-review` at
`4032f1e2dfaa374e70da7777a141092057cb05dc`
**Observed main:** `e0acb0254820c381dd97828824bef86a1db5176c`

This plan records the limited Owner mandate dated 2026-10-16 in
`Serea_Codex_Cloud_P8_Long_Horizon_Mandate_2026-10-16.md`. Its authority
boundary is explicit: approved direction may guide synthetic development;
details that introduce additional state transitions, Receipt authority,
retention, or retry authority remain `PROPOSED` and their runtime paths remain
fail-closed.

## Wave status

| Wave | Status | Evidence / blocker |
| --- | --- | --- |
| 0 — PR #19 | BLOCKED, safely retained Draft | Exact base/head and all five workflows reverified below. Reviews list is empty. Branch-protection endpoint returned HTTP 403 `Resource not accessible by integration`, so required review and protection conditions could not be verified. No merge or Draft-to-ready transition. |
| 1 — P8A direction record | DIRECTION RECORDED; exact contract closure IN PROGRESS | ADR-0044 records the approved direction; Capability/Task Protocol and ADR-0036 retry text and architecture index now align. Receipt/reconciliation/retention details remain proposed and runtime-blocked. |
| 2 — P7 schema 0009 | PROTOTYPE REVIEW ONLY | Disposable v8→fixture suite passed 10/10. A distinct out-of-catalog candidate now passes 4 focused candidate checks and the full P7 migration selector passes 14/14. Existing migrations 0001–0008 remain byte-identical. Provisional caps, Owner authority, retention, FTS, provenance, Forget and quarantine-release policies block canonical promotion. See [candidate audit](P7-schema-0009-candidate-audit-2026-10-16.md). |
| 3 — P8B 0010+ | TEST-ONLY TRANSACTION PROTOTYPE; RUNTIME BLOCKED | Out-of-catalog 0010 candidate stays isolated. Synthetic `cfg(test)` transaction mechanics cover revalidation, atomic grant/budget/IDK/RequestId/Intent/Event writes, rollback injection and duplicate suppression. This is not the real Store/TaskEngine authority transaction; exact semantics and runtime gate remain PROPOSED. |
| 4 — P8C Fake Provider | ISOLATED SIMULATOR READY; CANONICAL EXECUTION BLOCKED | Deterministic manual clock, scripted fake effects, independent SQLite ledger reopen, ack loss, rejection and duplicate invocation probes exist under `cfg(test)`. No `CapabilityProvider`/TaskEngine integration or provider invocation is claimed. |
| 5 — CI / hostile review | IN PROGRESS | Exact-head PR #21/22 status rechecked; this branch requires targeted/workspace validation and its own Draft PR run. |

## Work completed on this branch

- Added this persistent plan and the [Owner direction record](P8-owner-authorized-directions-2026-10-16.md).
- Added [ADR-0044](../decisions/ADR-0044-p8-durable-dispatch-v1-directions.md)
  within the exact approved scope; amended ADR-0036 only to narrow retry
  authority. ADR-0039 Option B is unchanged.
- Updated Capability Protocol §4/§8, Task Protocol §§5–6 and changelogs to
  prohibit automatic same-Step effect retries and make AMBIGUOUS reconcile-only,
  including `NATIVE`. No new Task/Step transitions, reconciliation authority,
  Receipt authority or retention policy were accepted.
- Advanced architecture documentation from 2.7.0 to 2.8.0. Wire major
  versions remain unchanged.
- No Rust runtime, canonical migration, normal migration catalog entry,
  provider call path, production CredentialStore or production Memory change
  was added in this slice.

- Wave 2 added only a development candidate SQL file outside the migration
  catalog and test-only probes behind `p7-experimental`. It remains explicitly
  noncanonical; see the audit and the 14/14 selector result below.
- Wave 3 added a development-only 0010 dispatch-intent schema candidate and a
  migration-runner probe. The table captures request, Task/Step/Owner, Grant
  use, Policy, registry, overlay, event and commit facts; it has unique
  RequestId/IDK/Step keys and no mutable state or Receipt column. The probe
  does not yet test a complete grant-authorized intent transaction or provider
  effect. The ordinary migration catalog is unchanged.

## Local validation on current source state

- `cargo test --workspace --all-targets --locked --offline`: PASS on retry.
  First attempt exposed uncached optional `cfg_aliases v0.2.2` in the existing
  nested Cargo boundary test; `cargo fetch --locked` downloaded locked crates,
  the focused RED reproduction then passed, and full rerun passed.
- `cargo test --workspace --all-features --locked --offline`: PASS (includes
  explicit synthetic P7 Gate C tests only).
- `cargo +1.85.0 check --workspace --all-targets --all-features --locked
  --offline`: PASS.
- `cargo clippy --workspace --all-targets --all-features --locked --offline --
  -D warnings`: PASS. A vendored `serde_json` deprecation warning is emitted
  outside Clippy's denied-warning scope; vendored code was not changed.
- `cargo test -p serea-protocol --locked`: PASS; all protocol unit, integration
  and doctests passed.
- `cargo test -p serea-storage --features p7-experimental --locked
  p7_migration`: PASS, 10 tests. This verifies the existing synthetic fixture,
  plus the separate candidate probes (14 total); it does not certify a
  canonical migration.
- `cargo fmt --all -- --check`, `tools/validate_docs.py docs`,
  `git diff --check`, Core API boundary, Synthetic Host boundary and
  `workspace_smoke_tests.py` (82 tests): PASS.
- `PR #19` required workflows were independently rechecked at exact head:
  Fast 1/1, Full 5/5, Native 12/12, SQLite portability 4/4, Git Hooks 3/3.
  These runs do not validate this branch's documentation commit.
- Local Native/SQLite cross-architecture execution: NOT RUN. New branch GitHub
  Actions: NOT RUN yet. No independent security review or physical power-loss
  test is claimed.

## Wave 0 — live GitHub verification

At 2026-10-16, local dedicated clone `/workspace/Serea-codex` was created with
the repository's required Codex-only local identity and hooks. The original
`/workspace/Serea` clone and its inherited personal Git identity were not
changed.

- `origin/main`: `e0acb0254820c381dd97828824bef86a1db5176c` (matches mandate).
- PR #19: Open, Draft, base `main` at the same main SHA, head
  `4032f1e2dfaa374e70da7777a141092057cb05dc`, mergeable `true`,
  `mergeable_state=clean` (matches mandate).
- Review submissions: none returned by GitHub API.
- Required status runs all bind exact PR head `4032f1e2dfaa374e70da7777a141092057cb05dc`,
  event `pull_request`, attempt 1, completed success:
  - Fast `38899052188`, 1/1 jobs.
  - Full `38899052241`, 5/5 jobs.
  - Native `38899052239`, 12/12 jobs.
  - SQLite portability `38899052099`, 4/4 jobs.
  - Git Hooks `38899052363`, 3/3 jobs.
- `GET /repos/Yrika819/Serea/branches/main/protection` returned 403
  `Resource not accessible by integration`. Required review count, required
  checks and any other merge gate could therefore not be independently read.
- PR #14 is Open/Draft/Unmerged. No action taken. PR #15 is already merged at
  `e0acb0254820c381dd97828824bef86a1db5176c`.
- Result: conditions for normal protected merge are **not fully verifiable**;
  PR #19 remains Draft and unmerged. PR #14 was not closed or merged.

## Owner-approved direction versus unresolved contract

The authority record must list D01–D07 exactly as approved and preserve three
labels: `OWNER-APPROVED DIRECTION`, `PROPOSED DETAIL`, and `BLOCKED`. Do not
mark an exact clause Accepted merely because a restrictive runtime prototype
can be tested.

Approved: one Dispatch Transaction revalidates current facts and intent commit
is the authority point; ADR-0039 Option B remains the atomic transaction;
consumed grants are historical and do not authorize a repeat effect; AMBIGUOUS
is reconcile-only with no automatic same-Step effect retry; late outcomes aim
at Option A with Option C fail-closed until exact eligibility is approved; real
CredentialStore/OS auth/providers are deferred; deterministic no-network fake
provider fault testing is required; migration Option A reserves canonical P7
0009 and P8 0010+.

Blocked exact wording: provider invocation eligibility across the
commit-to-invoke gap; whether a committed but not invoked intent has a close
state; exact revalidation serialization for cancellation/deadline/policy;
receipt authority for late results; Step/Task transitions for recovery;
unresolved intent retention and Forget holds; any repeat or retry after the
first grant-consuming intent. In the meantime no ambiguous effect replay, no
normal success/Receipt path after Task cancellation, no Task resurrection and
no invented canonical Receipt.

## Next execution slices

1. Add the Owner authorization record and an exact Protocol/ADR consistency
   ledger; write any language that exceeds the approved direction as
   `PROPOSED`, with runtime denial explicitly retained.
2. Audit P7 0009 candidate DDL against the ratified P7 directions and unresolved
   policy. Keep fixture and candidate distinct; do not register in the normal
   catalog unless evidence supports all requested invariants without silently
   promoting a proposed policy.
3. Add disposable v8→v9 schema probes and integrity/concurrency/rollback/reopen
   tests. Record the candidate as prototype if any normative detail remains.
4. Close the exact P8 authority blockers before exposing a write API: atomic
   Policy/Grant/Lease/Task revalidation, bounded tool-budget semantics, and
   the commit-to-invoke eligibility rule. Keep the 0010 candidate out of the
   normal catalog meanwhile.
5. Implement typed atomic dispatch only through TaskEngine once the above
   checks are enforced by the SQLite transaction. Provider I/O must remain
   outside SQLite write transactions.
6. Add Fake Provider tests at the canonical execution boundary. Assert
   durable rows, effect ledger, event order, receipt cardinality and restart
   state. Tests that depend on unapproved lifecycle or retention semantics
   assert fail-closed behavior and are labeled accordingly.
7. Before any PR, run the repository's Codex identity/history guards, exact
   targeted and workspace validation, then record final SHA/base and precise
   CI. Draft PRs only; no PR other than #19 may be merged.

## Wave 3 — P8 0010 dispatch schema candidate

The separate candidate lives at
`crates/serea-storage/migrations/candidates/0010_p8_dispatch_candidate.sql`.
It is compiled/applied only by an explicit `p7-experimental` test against
temporary SQLite after the separate P7 candidate. It is not embedded. The
initial table shape records one immutable intent per Step with unique
RequestId/IDK, owner and grant-use bindings, plan/task/lease revisions,
  policy revision, registry descriptor, overlay revision, event correlation
  identifier and
commit time. It deliberately has no lifecycle state or Receipt authority.

The first schema probe passed. This only proves that SQLite can apply and
inspect the candidate schema and that its declared parent keys are valid. It
does not prove the atomic grant+budget+intent+event transaction, live provider
availability, Policy decision semantics, commit-to-invoke safety or any
effect/recovery path. The current TaskEngine has no provider invocation API;
P8C remains blocked until the P8B transaction safely exists. The direct
`CapabilityProvider::invoke` trait remains unreachable from TaskEngine. See
[the candidate audit](P8-dispatch-0010-candidate-audit-2026-10-16.md).
That audit includes concrete PROPOSED clauses for tool-budget counting,
commit-to-invoke crash behavior, single-transaction revalidation, event
correlation/retention, and result/Receipt handling. None is runtime authority.

## Follow-up: fail-closed candidate and test-only prototypes

On the isolated PR22-derived branch `p8/authority-transaction-prototype`,
candidate 0010 now uses `ON DELETE RESTRICT` for Task, Step and consumed
Grant-use references. The focused candidate test proves a missing Grant is
rejected, and proves Task/Step/Grant-use deletion fails while a synthetic
unresolved Intent remains. This is a conservative development-candidate guard,
not a retention or Forget policy; it stays outside the normal migration
catalog.

`p8_authority_transaction_tests.rs` is a synthetic `cfg(test)` SQLite-only
prototype exercising current-fact checks, atomic grant/budget/IDK/RequestId/
Intent/Event writes and rollback at four injected boundaries. Separate
connection assertions verify both all-or-nothing failure and all-at-once
success. It is not wired to production Store/TaskEngine and is not complete
authorization evidence. `p8_synthetic_recovery_tests.rs` supplies a manual
clock and scripted simulated provider backed by an independently reopened
SQLite effect ledger; it demonstrates visible duplicate invokes and no
automatic replay after ack loss. It does not implement the provider trait or
call TaskEngine. All remaining runtime, platform and contract gates remain
open.

## Production boundary evidence

This ExecPlan and any dependent PR are schema/test development only. They do
not enable production Memory or provider invocation. Owner OS authentication,
key custody, rollback-resistant authority, production migration/backfill, Gate
D, real credentials/providers and real external effects remain unauthorized.

## Handoff

The active chain is Draft PR #19 → Draft PR #20 → Draft PR #21 → Draft PR #22.
PR #20 is `p8/approved-contract-candidate` at
`5f78762e1b358ac928a42aab43d8bb215089194a`, based on PR #19. PR #21 is
`p7/schema-v9-candidate` at
`a56c7e204f854dd4382339cb43f7fe1396f85137`, based on PR #20. PR #22 is
`p8/durable-dispatch-schema-candidate`; its schema/test commit is
`9ea1e8911cd211f27c6e56848501c96d255915b4`, with follow-up contract proposal
documentation on top. PR #22 is based on PR #21. The P7 and P8 candidates are
not independently applicable to main and remain outside the production
migration catalog.

The dedicated clone/worktree retains Codex local identity and repository
hooks. All PRs remain Draft. PR #19 cannot be readied or merged until main's
required review and protection conditions are independently verifiable and
the normal protected procedure can be followed. No PR other than #19 may be
merged; PR #14 remains untouched.

## P8B/P8C isolated follow-up (2026-10-16)

**Development branch:** `p8/authority-transaction-prototype`, based directly
on PR #22 head `58944dd02f498894567ec92b7440270a9985e5ec` and therefore
stacked on PR #21 → #20 → #19. The dedicated worktree is
`/workspace/Serea-p8-authority`; existing clones/worktrees were unchanged.
Implementation commit: `dfd9f5158fbdd62049ff79d1a7aaf3c527c41c1f`.
Draft PR #23: <https://github.com/Yrika819/Serea/pull/23>, base PR #22.

- P8 0010 remains outside the normal catalog. Its development-only Task, Step,
  owner-binding and consumed-Grant-use delete references now use RESTRICT. A
  synthetic disposable SQLite probe confirms Grant-less insert rejection and
  refusal of Task/Step/Grant-use deletion while an unresolved Intent is present.
  This is not an accepted retention or Forget policy.
- Added test-only transaction mechanics prototype and recovery simulator
  in `p8_authority_transaction_tests.rs` and
  `p8_synthetic_recovery_tests.rs`. Ten P8-focused tests pass, including
  separate-connection rollback visibility, one-transaction write grouping,
  synthetic Task/Step/Lease/Policy/Grant/Registry/Overlay/Provider/Budget fact
  checks, Grant expiry/revocation, duplicate reservation refusal, two independent
  dispatcher connections with one winner, independent effect-ledger reopen, ack
  loss, no automatic replay and deterministic deadline control. The simulator
  neither implements the provider trait nor calls TaskEngine. These tests do not
  represent production authority.
- `cargo test -p serea-storage --all-targets --all-features --locked --offline`:
  PASS, 625 unit tests, 6 integration tests passed, 2 manual resource tests
  ignored. Includes the P8 prototypes and the existing P7 candidate coverage.
- Workspace `cargo check --workspace --all-targets --all-features --locked
  --offline`: PASS. Rust `1.85.0` workspace check: PASS. Workspace Clippy with
  `-D warnings`: PASS. Format, documentation validator, Core API boundary,
  Synthetic Host boundary and workspace dependency smoke check: PASS.
- Workspace all-target/all-feature test linking was attempted but the Cloud
  filesystem filled (0 bytes free; linker returned Bus error). This is recorded
  as NOT COMPLETED, not PASS. Storage's complete suite did pass independently.
- PR #21 exact-head Actions all succeeded: Fast `38908107229`, Full
  `38908107318`, Native `38908107246`, SQLite `38908107819`, Git Hooks
  `38908107508`.
- PR #22 exact-head all succeeded: Fast `38910579631`, Full `38910579688`,
  Native `38910579808`, SQLite `38910579894`, Git Hooks `38910580008`. These
  runs do not validate PR #23.
- PR #23 documentation HEAD `0a23a9d7ebc729f26fe7271ebb619b2ff9a3dfb5`:
  Fast `38914990830` and Git Hooks `38914990583` succeeded; Native
  `38914990758` was in progress; Full `38914990535` and SQLite `38914990627`
  were queued at last poll. The current source follow-up will create a newer
  head, so those checks do not validate that newer head.
- No cloud-environment skill was available in the active tool catalog; the
  environment was inspected through Git, Rust/Cargo and GitHub connector
  observations.

## Remaining exact blockers

The `cfg(test)` transaction uses synthetic tables and is deliberately not
production Store code. Real grant-use schema, R2 membership, event participant,
policy decision source, registry/overlay authorities, TaskEngine-only public
entry and commit-to-invoke eligibility still require an integrated approved
design. Provider IO remains unreachable. `AMBIGUOUS` remains reconciliation
only; no automatic same-Step retry or canonical late Receipt exists. Production
Memory, OS Owner authentication, key custody, rollback-resistant authority,
real credentials/accounts/providers and Gate D remain disabled and
unauthorized.

## P8B real Storage boundary follow-up (2026-10-16)

**Branch:** `p8/real-storage-boundary-prototype`, based on PR #23 head
`b6667d19fd4b7082ed7b8d8e61d3fa56ee194f16`; dependent on PR #23 → #22 → #21 →
#20 → #19. Worktree: `/workspace/Serea-p8-real-storage`.
Draft PR #24: <https://github.com/Yrika819/Serea/pull/24>. Source commit
`75017d7be38eddb30eaa93037f1b6510f9c0b463`; formatting correction
`99b945e271c6773e54c66ef393c1e4a75dd9ab98`. The source-code checkpoint
was clean before this ExecPlan update. PR #24 is open,
Draft, and mergeable; its base SHA remains PR #23's
`b6667d19fd4b7082ed7b8d8e61d3fa56ee194f16`.

- Corrected the trailing comment in candidate 0010: Task deletion is refused
  while an Intent remains because the FK is `ON DELETE RESTRICT`. The former
  comment claimed Task deletion remained possible. The SQL still is a candidate
  only; this does not settle retention or Forget policy.
- Added a `#[cfg(test)]` Storage boundary probe to the existing P6 migration
  test module. Its disposable file DB applies candidate 0009/0010 SQL outside
  the migration catalog, seeds a synthetic Owner binding, and creates an actual
  P6 approval Grant using the typed Storage API. It acquires a real Storage
  Lease, then in one `Store::transact` calls `Tx::consume_grant_use`, checks
  current Task/Step/Lease/Policy/Registry/Overlay rows, inserts a candidate
  Intent, and appends an existing Protocol Event. It uses no public Intent
  writer and is unreachable outside test builds.
- Fault probes return errors after Grant consumption and after Intent insert;
  an SQLite trigger also fails event persistence. They check Grant-use,
  `uses_remaining`, Intent and Event state after dropping and reopening the
  file-backed Store. A success probe checks all four persisted records and then
  verifies duplicate same-Step use and Task deletion fail without removing the
  unresolved Intent.
- Additional refusal probes use actual Storage cancellation and P6 Grant
  revocation paths, change the active Policy revision, and reclaim the Lease
  under a second owner/generation. Deadline equality also refuses. Each refuses
  before candidate Intent commit. A two-connection race allows at most one
  Grant-use/Intent/Event transaction to commit.
- The test intentionally does **not** claim P8 authorization. Budget accounting
  has no accepted durable call counter; provider availability has no accepted
  durable authority source; TaskEngine has no dispatch entry point; the test
  does not create a commit-to-invoke path. Candidate Owner binding is synthetic
  and is not OS authentication. The probe demonstrates transaction composition
  and revalidation boundaries only.
- Local verification environment has no `cargo`, `rustc`, or `rustfmt` binary,
  so Rust test/format execution is NOT RUN locally. The existing build caches
  were left intact. Python SQLite applied migrations 0001–0008 followed by
  candidate 0009/0010 with `foreign_key_check` empty; this is SQL parser/schema
  evidence only, not the Rust integration-test result.
- RED/GREEN evidence is therefore pending exact-head CI. Do not mark this slice
  complete until CI compiles and runs the new P6/P8 tests, and run formatting,
  all-target/all-feature, MSRV and Clippy gates on this branch head.
- Exact-head Actions for `99b945e271c6773e54c66ef393c1e4a75dd9ab98` were
  submitted as Fast `38920572913`, Full `38920573172`, Native
  `38920573301`, SQLite portability `38920572461`, and Git Hooks
  `38920572998`. At 2026-10-16 15:05 UTC, Git Hooks Windows and Ubuntu jobs
  had succeeded while its macOS job was queued; all other workflows were
  queued. This is pending, not PASS. The exact-head statuses must be refreshed
  after the next commit because any plan update changes the tested HEAD.
- PR #23's exact-head Fast `38916026696`, Git Hooks `38916026814`, and SQLite
  portability `38916026673` succeeded; Full `38916026730` and Native
  `38916027180` remained in progress at the same poll. This is separate from
  PR #24 validation.

### Restart point and owner decisions

Resume by checking this branch's exact-head Fast/Full/Native/SQLite/Git Hooks
workflows and correcting any concrete compile/test failure without widening
the candidate schema. If they pass, review real TaskEngine/Storage reachability
again and add only test-only cases supported by existing authority APIs.
Runtime integration remains blocked pending an accepted Budget source and
accounting rule, Provider availability evidence, and commit-to-invoke/recovery
eligibility. Retention/Forget, Receipt authority, retry and late-outcome rules
remain fail-closed as recorded above.

## P8B contract closure follow-up (2026-10-16)

**Development branch:** `p8/contract-closure-prototype`, created from the
latest PR #24 head `fab5ee91c3942e8e601338774492600e9853dad2`. It is intended
to remain stacked on PR #24 → #23 → #22 → #21 → #20 → #19. The other
worktrees were clean and left untouched at branch creation.

- Added [the contract closure package](P8B-contract-closure-package-2026-10-16.md)
  classifying current facts as ACCEPTED, PROPOSED or BLOCKED and narrowing
  Owner choices for Budget, Provider availability, commit-to-invoke and
  Intent retention/Forget. It provides recommended clauses, alternatives,
  API shape and executable acceptance cases. No ADR, Protocol or production
  migration was promoted or edited.
- Code audit confirmed that ADR-0036 accepts one `max_tool_calls` unit per
  committed provider dispatch Intent, while migrations 0001–0008 have no
  durable tool-call use counter. Migration 0003's `model_call_count` is model
  accounting and `max_attempts_per_step` counts lease/attempt lifecycle facts;
  neither substitutes for P8 provider Intent accounting.
- Added a test-only budget guard that counts candidate Intents and reads the
  actual Task `max_tool_calls` within the same `Store::transact` write
  transaction. A zero-budget disposable Task must refuse before Grant use;
  after Store reopen all grant-use, Intent and event rows must remain absent.
  The candidate-only count depends on immutable Intents and is not yet a
  canonical retention design.
- The Storage test appends `CapabilityRequested` only as an atomic Event
  participant exercise. It is not accepted as the semantic P8 Intent event;
  no new EventKind was added. The contract package records this gap and the
  required acceptance test before Protocol/runtime integration.
- Added a private `P8TestProviderReadiness` enum to the test-only Storage
  probe. Unknown and unavailable readiness refuse before `Store::transact`;
  reopened P6 Grant-use/Intent/Event tables remain empty. The explicit
  synthetic-ready variant permits only the test composition case and is not
  wired to CapabilityProvider or Host authority.
- Extended the existing `p8_synthetic_recovery_tests` simulator with an
  independent synthetic Intent ledger. New tests reopen Intent and Effect
  SQLite files after ACK loss, check exact RequestId/IDK matching without
  replay, detect a same RequestId with wrong IDK as a mismatch, and crash
  after Intent commit/before fake invoke with no recovery replay. This is a
  reopen simulation, not an OS process-kill test. It remains simulator-only
  and does not claim canonical Receipt validation.
- RED/GREEN finding: on the PR #24 candidate SQL, a direct `DELETE FROM
  dispatch_intents` removed the only Intent despite Task/Step/Grant-use
  `ON DELETE RESTRICT`. Added a candidate-only no-delete trigger and a Storage
  schema test. A Python SQLite replay of migrations 0001–0008 plus candidates
  0009/0010 reproduced deletion on parent `fab5ee9` (RED) and confirmed the
  changed candidate rejects deletion and retains the row (GREEN). The trigger
  is not a production Forget guarantee.
- PR #24 exact-head Fast `38921134228`, Full `38921134186`, and Git Hooks
  `38921134153` succeeded. Native `38921134154` and SQLite portability
  `38921134163` remained queued at the latest poll; they are pending, not PASS.
  PR #24 base/head remain unchanged. These runs do not validate this follow-up
  branch.
- The environment has Rust 1.85.0/stable toolchains but only 106 MiB disk
  free; shared Cargo target uses 5.2 GiB. To avoid damaging shared caches or
  exhausting the filesystem, Cargo build/tests and Clippy were NOT RUN. The
  Core API boundary script was attempted but is NOT RUN because its subprocess
  could not find `cargo` on PATH. Direct stable `rustfmt --check` on all three
  changed Rust files passed. Documentation validation, Synthetic Host
  boundary, workspace dependency smoke and `git diff --check` passed.
  Exact-head child PR CI must compile/run the tests.
- Scope review: the source changes are candidate SQL and `#[cfg(test)]` code
  only. The TaskEngine still has no dispatch/invoke method. No Fake readiness
  becomes authorization, no Provider call is added, and no normal migration
  catalog or production feature is changed.

**Remaining Owner decisions:** explicitly confirm whether OBSERVE provider
dispatches consume `max_tool_calls` (recommended: yes, as the unqualified
accepted “provider dispatch intent” rule); whether a post-commit one-shot
Permit may invoke despite later cancellation/revocation (recommended: commit
linearizes this Intent; post-commit cancellation stops later work); what
authoritative live readiness means when the current health snapshot is
non-durable; and how unresolved versus resolved Intents survive Task expiry
and authenticated Forget. No new state transitions, retry, Receipt, retention
horizon or production authority was implemented.

**Restart point:** run exact-head Fast/Full/Native/SQLite/Git Hooks on the
child branch, fix any compile/test defects, then continue test-only availability
and recovery cross-checks. Keep the provider invocation path blocked until the
Permit, readiness and cancellation contracts are accepted and implementable.

## Overnight continuation — PR #25 follow-up (2026-10-17)

This entry supersedes earlier environment observations above where they differ.
Work is on a dedicated clean clone `/workspace/Serea-overnight`, branch
`p8b/overnight-2026-10-17`, created at PR #25 head
`73e38fddf4a203582f728eff2cf675cc35e46b2b`. The user's `/workspace/Serea`
clone remains untouched. The dedicated clone has local-only Codex identity
(`Codex <noreply@github.com>`), `push.default=nothing`, installed repository
hooks, and a passing commit preflight. No existing branch history is rewritten.

Live GitHub audit at this continuation:

- `main`: `e0acb0254820c381dd97828824bef86a1db5176c`.
- Draft/open dependency chain: #19 `4032f1e2dfaa374e70da7777a141092057cb05dc`
  → #20 `5f78762e1b358ac928a42aab43d8bb215089194a` → #21
  `a56c7e204f854dd4382339cb43f7fe1396f85137` → #22
  `58944dd02f498894567ec92b7440270a9985e5ec` → #23
  `b6667d19fd4b7082ed7b8d8e61d3fa56ee194f16` → #24
  `fab5ee91c3942e8e601338774492600e9853dad2` → #25
  `73e38fddf4a203582f728eff2cf675cc35e46b2b`.
- #25 remains Draft/open with base #24 at `fab5ee91c3942e8e601338774492600e9853dad2`.
  Exact-head Fast `38925789524`, Full `38925789470`, Native `38925789362`,
  SQLite portability `38925790040`, and Git Hooks `38925789317` are all
  completed success. Native finished after the initial poll; no pending job is
  being counted as success.
- #24's exact-head Native `38921134154` and SQLite `38921134163` later completed
  success, joining its Fast `38921134228`, Full `38921134186`, and Hooks
  `38921134153`. These are historical parent-HEAD results, not validation of
  new source.

| Wave | Status | Evidence / boundary |
| --- | --- | --- |
| A — adversarial review | PARTIAL | Reviewed #23–#25 transaction, migration-candidate and Fake Recovery code. Existing candidate direct-delete flaw has prior RED/GREEN evidence below; no new accepted-contract production defect found. |
| B — Storage Budget | PASS, test-only | Added real-Store candidate probe with two independently leased and approved Steps racing at `max_tool_calls=1`; only one Grant-use/Intent/Event commits after reopen. Existing `max_tool_calls=0` refusal still leaves all rows absent after reopen. No production counter or write API. |
| C — availability | PARTIAL | Existing real-Store candidate test rejects Unknown/Unavailable before writing. Resolver health/overlay snapshots remain point-in-time evidence; no durable or live invoke authority was added. Provider instance identity and commit-gap readiness remain proposed/blocked. |
| D — commit-to-invoke | PARTIAL | Existing independent Intent/Effect ledger tests cover ACK-loss and commit-before-invoke reopen with no automatic re-invoke. A proposed non-serializable Permit now exists only in the synthetic test module and binds exact RequestId/IDK, Step, lease generation, Owner and provider instance; production authorization and post-commit disposition remain blocked. |
| E — retention/Forget | PASS for candidate guard only | Added second-Store connection check: `PRAGMA foreign_keys=1`; Task, Step, consumed Grant-use, and direct Intent deletion fail; normal Event expiry removes Event content while RequestId/IDK Intent remains after another reopen. Not a canonical Forget rule. |
| F — Event meaning | PARTIAL | Exact Protocol review found Event Protocol §3.3 already defines `CAPABILITY_REQUESTED` as durable P8 Intent commit. The accepted kind is reused; its full bounded P8 payload remains proposed. No EventKind or protocol version change. |
| G — Fake Provider recovery | PARTIAL | Storage P8 selector passes 13/13, including mismatch, ACK-loss and pre-invoke crash simulations. These use independent SQLite ledgers, not a child-process kill or a TaskEngine invoke path. |
| H — integration stack | PASS for read-only audit | #19–#25 heads, bases, draft state and ancestry match the stack. No migrations 0001–0008 or normal catalog changed; no PR merge/close. |
| I — hostile regression | RUNNING | Added focused candidate boundary and concurrency tests; further review/validation follows. |
| J — local/CI gates | PASS locally; new-head CI pending | Rust 1.85.0 all-target suite: 1,347 passed, 0 failed/ignored; all-features: 1,514 passed, 0 failed, 2 manual resource benchmarks ignored; all-target/all-feature `check` and Clippy `-D warnings` pass. Exact-head CI for the new branch awaits commit/push. |

### New executable evidence on this branch

- `crates/serea-storage/src/migration_0005_tests.rs` adds
  `real_storage_task_budget_limit_one_serializes_two_eligible_steps`. The
  fixture uses the actual Store, task/step rows, P6 Grant-use API, leases,
  SQLite transaction and the separate 0010 candidate. Two independent Store
  connections race distinct eligible Steps on one Task at limit 1. The
  post-reopen assertion requires exactly one Intent, Grant-use and correlated
  candidate Event; the refused contender leaves no partial Event. This tests
  candidate composition only and does not establish a canonical budget
  schema or production dispatch authorization.
- The same file adds
  `real_storage_unresolved_intent_survives_second_store_deletes_and_event_expiry`.
  It confirms each reopened Store has foreign keys enabled, Task/Step/consumed
  Grant-use/direct Intent delete fail, and configured Event content expiry
  does not erase the unresolved candidate Intent's RequestId/IDK evidence.
- `docs/plans/P8B-contract-closure-package-2026-10-16.md` now includes section
  E for P8 Intent Event semantics, with accepted facts, a proposed payload
  clause, alternatives, attack cases, API/schema impact, tests, residual risks,
  and Owner choice. Exact review corrected an earlier package assertion:
  Event Protocol §3.3 already assigns `CAPABILITY_REQUESTED` to committed P8
  Intent. It records commit, not provider invocation, effect success, or
  Receipt. Exact payload fields remain proposed; no EventKind/protocol change.
- RED/GREEN from the earlier P8B slice remains specific: PR #24 candidate SQL
  allowed direct Intent deletion after parent-row RESTRICT checks (RED); the
  out-of-catalog candidate no-delete trigger caused the same deletion attempt
  to fail while preserving the row (GREEN). This does not claim production
  retention safety.

### Current verification checkpoint

- Rust 1.85.0 focused Storage `p8_` selector: PASS, 15 passed, 0 failed,
  622 filtered, 0 ignored. Includes child-process exits at commit-before-invoke
  and after fake effect/before ACK; reopened intent/effect ledgers match exact
  RequestId/IDK and repeated reopen adds no effect.
- RED/GREEN mutation evidence for the distinct-Step Budget guard: temporarily
  changed the test-only check from `committed_dispatches >= max_tool_calls` to
  `>`; the new two-connection test failed with two successful commits where one
  was expected (RED, exit 101). Restored `>=`; the same test passed (GREEN).
  This shows the acceptance test detects the exact-limit off-by-one, not a
  production runtime fix.
- `cargo +1.85.0 test --workspace --all-targets --locked --offline`: PASS,
  1,347 passed across 69 test binaries, 0 failed, 0 ignored. The full log
  includes the new child-process crash/reopen cases.
- `cargo +1.85.0 test --workspace --all-features --locked --offline`: PASS,
  1,514 passed across 79 test binaries, 0 failed, 2 ignored manual resource
  benchmarks (`measure_checkpoint_advance_and_reopen_history_growth` and
  `measure_ledger_append_replay_and_history_growth`). The full run includes
  P7 Gate C, P8 candidate, Fake Recovery and child-process tests.
- `cargo +1.85.0 check --workspace --all-targets --all-features --locked
  --offline`: PASS, exit 0.
- `cargo +1.85.0 clippy --workspace --all-targets --all-features --locked
  --offline -- -D warnings`: PASS, exit 0.
- The initial offline targeted test attempt had no cached `jsonschema` crate;
  `cargo +1.85.0 fetch --locked` completed and all later offline gates passed.
- Rust 1.85.0 real-Storage selector: PASS, 7 passed, 0 failed, 628 filtered,
  0 ignored. Includes the two newly added cases.
- `cargo +1.85.0 fmt --all -- --check` and `git diff --check`: PASS after
  formatting the test file.
- `tools/validate_docs.py docs`: PASS, 145 Markdown files scanned.
- `tests/workspace_smoke_tests.py`: PASS, 82 tests. Synthetic Host static
  boundary: PASS. Core API boundary compile check: PASS; an external consumer
  cannot access the removed synthetic-registry fixture API.
- Local requested Rust gates are complete. New branch Actions are not yet
  submitted because this checkpoint has not been committed/pushed.
- At audit time, dedicated checkout started with 30 GiB free. Cargo target and
  temporary boundary build are isolated to this clone and `/tmp`; shared
  caches in `/workspace/Serea` were not modified or deleted.

**Current safe boundary:** only test code and decision-support documentation
are changed. Production Storage migrations/catalog, TaskEngine/Host Provider
invocation, Credentials, OS Owner authentication, production Memory, canonical
Receipt, retry authority and schema versions remain unchanged/fail-closed.

**Next checkpoint:** finish workspace/MSRV/Clippy/boundary validation; record
all outcomes and exact new HEAD in this plan; run Git Safety preflight; create a
normal commit on `p8b/overnight-2026-10-17`. Push only this new branch and open a
Draft PR stacked on PR #25 to obtain exact-head CI; never update or rewrite PR
#25's branch. If CI reveals a reproducible code defect, record RED, fix
minimally, rerun focused and full regressions, and update this plan before the
next commit.

### Overnight continuation checkpoint — synthetic one-shot permit

The first normal commit is `acd1dc5e3c8c990bb7795917d0fb9ffb31f3fff6`
(`test(p8): deepen budget and recovery boundaries`), parented on PR #25's exact
head `73e38fddf4a203582f728eff2cf675cc35e46b2b`. Draft PR #26 is open against
`p8/contract-closure-prototype`. The first commit's exact-head Actions are:

| Workflow | Run | Status at this checkpoint |
| --- | ---: | --- |
| Fast CI | 38935044346 | success |
| Git hooks Windows compatibility | 38935044413 | success |
| Full CI | 38935044416 | in progress |
| Native platform validation | 38935044857 | queued |
| Cross-architecture SQLite portability | 38935044264 | queued |

Added an explicitly proposed, non-serializable `SyntheticDispatchPermit` to
`p8_synthetic_recovery_tests.rs`. It is isolated to the test module and
consumes itself before calling the scripted Fake Provider. The regression
checks mismatched RequestId and IDK, Step, lease generation, Owner, and
provider instance all refuse with zero provider effects; one exact binding
may invoke once, an ambiguous outcome consumes the permit, and a repeat is
refused. It does not establish production authorization, durable permit
semantics, or an actual TaskEngine/Host invoke path.

After this addition, the focused Storage `p8_` selector passes 16/16 (622
filtered, 0 ignored); Storage Clippy with `-D warnings` passes; format and
`git diff --check` pass. Full workspace Clippy is being rerun against this
working tree. The full workspace test/check results above predate this final
test-only addition. The synthetic Permit is currently uncommitted and not yet
included in PR #26 CI; it will be recorded in the next normal commit, after
which exact-head CI must run again.

**Resume:** finish full Clippy and documentation/boundary gates, update this
checkpoint with exact results, commit the permit test and plan normally, push
only `p8b/overnight-2026-10-17`, then wait on all five workflows for the new
PR #26 head. Continue safe independent review while they run. Keep Wave D
PARTIAL and all production invocation/Receipt/retry authority fail-closed.

#### Final pre-commit verification for the permit slice

- `cargo +1.85.0 clippy --workspace --all-targets --all-features --locked
  --offline -- -D warnings`: PASS after the Permit addition.
- `cargo +1.85.0 test --workspace --all-targets --locked --offline`: PASS
  after the Permit addition. Relative to the previous checkpoint, the exact
  new Permit regression is included; no failure occurred.
- `cargo +1.85.0 test -p serea-storage --all-targets --all-features --locked
  --offline`: PASS, 638 unit tests plus 6 integration tests passed, 2 manual
  resource benchmarks ignored, 0 failures. The test command completed in
  114.71 seconds for the unit-test binary.
- Docs validation scanned 145 Markdown files and passed. Formatting,
  `git diff --check`, external Core API boundary, Synthetic Host boundary,
  and reachable commit identity checks all passed.
- Git commit preflight was run while the two intended files were still
  unstaged and correctly refused that state. It must be rerun after staging;
  no commit was made at that point.
- First-head PR #26 run `38935044416` remains in progress after successful
  Fast CI and Git Hooks. At the last job-level inspection, the Windows P7 Gate
  C and macOS arm64 jobs were successful; Linux MSRV, Linux stable full, and
  macOS Intel were still running. Native `38935044857` and cross-architecture
  SQLite `38935044264` remained queued. These refer to the earlier exact head
  `acd1dc5...`; the Permit slice still requires its own pushed-head Actions.

### Exact-head CI completion — `27f3b14cdd8fdf58764d0b7e34196a1c9b916462`

The one-shot Permit regression was committed as `27f3b14` and pushed to Draft
PR #26. All five workflows completed success against that exact SHA; no
queued, skipped, cancelled, or running result is counted as a pass. This CI
checkpoint is recorded in the present ExecPlan update:

| Workflow | Run | Final result |
| --- | ---: | --- |
| Fast CI | 38936046309 | success |
| Full CI | 38936046460 | success |
| Native platform validation | 38936046279 | success, all target jobs |
| Cross-architecture SQLite portability | 38936046530 | success, both cross-open directions |
| Git hooks Windows compatibility | 38936046688 | success |

Full CI's Linux stable job passed all-target/all-feature checks, tests, Clippy
and release fault-seam exclusion; Linux MSRV 1.85.0 passed check, both test
modes and Clippy; Windows P7 Gate C and macOS Intel/ARM64 validation passed.
Native validation's stable and Rust 1.85.0 jobs passed across available Linux,
macOS and Windows targets, including release builds and synthetic Core/Host
startup and child-process lifecycle probes. Cross-architecture SQLite
generated databases on ARM64 and Intel and opened each on the opposite Mac
architecture. No CI-observed defect required a source fix.

**Resume point:** branch `p8b/overnight-2026-10-17`, PR #26, exact head
`27f3b14...` (base PR #25, `73e38fdd...`), clean local worktree. The next
independent work requires an unblocked test-only falsification slice or an
Owner decision on proposed contract details. Keep candidate migrations
out-of-catalog and production invoke, Receipt, retry, and canonical retention
authority fail-closed. Do not alter or merge PRs #19–#25.

### Autonomous continuation — PR #26 authority boundary (2026-10-17)

This continuation starts from the live PR #26 head `72be5ac56d01765b8c9dea79ee3e4f9d1fc5bbc4`,
base branch `p8/contract-closure-prototype` at PR #25 head
`73e38fddf4a203582f728eff2cf675cc35e46b2b`. A separate clone
`/workspace/Serea-codex` and branch `p8b/authority-boundary-2026-10-17` were
created; the original `/workspace/Serea` worktree remains clean and unchanged.
The new branch begins exactly at #26 head. PRs #19–#26 were re-fetched and
remain Open/Draft/unmerged with their current bases and heads; #24–#26 exact
head status checks are complete success, subject to the Windows ARM64 attempt
history below.

Live CI audit at this continuation:

| PR / exact head | Base | Workflow run | Result / attempt |
| --- | --- | ---: | --- |
| #24 `fab5ee91` | `p8/authority-transaction-prototype` | Fast `38921134228`; Full `38921134186`; Native `38921134154`; SQLite `38921134163`; Hooks `38921134153` | All completed success, attempt 1 |
| #25 `73e38fdd` | `p8/real-storage-boundary-prototype` | Fast `38925789524`; Full `38925789470`; Native `38925789362`; SQLite `38925790040`; Hooks `38925789317` | All completed success, attempt 1 |
| #26 `72be5ac5` | `p8/contract-closure-prototype` | Fast `38940339170`; Full `38940339177`; Native `38940339189`; SQLite `38940339248`; Hooks `38940391991` | All latest check runs completed success; Native run attempt 2 |

For Native run `38940339189`, API metadata reports `run_attempt=2`, overall
success on #26 SHA `72be5ac...`. Attempt 1 failed job
`117106059399`, `Stable Windows ARM64 (aarch64-pc-windows-msvc)`, at step 8,
`Test all workspace features`; all earlier setup/dependency/default checks
passed and subsequent release/Host steps were skipped. Attempt 2's corresponding
Stable Windows ARM64 job `117120563141` succeeded. Rust 1.85.0 Windows ARM64
job `117106059966` also succeeded. The attempt-1 job-log endpoint returned
403 and `gh run view --attempt 1 --job 117106059399 --log` exposed no body.
The check annotation reports only the generic `Process completed with exit
code 1`; the test name, assertion and stderr remain unavailable.
No Windows ARM64 host exists in this execution environment. Disposition:
**UNRESOLVED / monitor**; do not infer a transient runner failure or claim
reproduction. Do not restart CI solely to improve appearance.

#### Test-only Permit binding RED/GREEN

`p8_synthetic_recovery_tests.rs` now binds the proposed private synthetic
Permit to `SyntheticRegistryPin { generation, descriptor_digest,
overlay_revision }` alongside existing RequestId/IDK, Step, Lease generation,
Owner and provider instance bindings. The acceptance matrix changes each new
pin independently and expects its exact typed refusal and zero external Fake
ledger effects.

- **RED:** temporarily removed all three new equality guards. The focused test
  failed on the Registry generation case: it returned an acknowledged result
  and wrote one Fake effect where `RegistryGenerationMismatch` and zero effect
  were required. The file was restored immediately from the saved GREEN copy;
  no mutant remains.
- **GREEN:** with all guards restored, the Storage `p8_` selector passed 16
  tests, 518 filtered, 0 ignored. The separately run guard-removal mutant was
  not counted as a product/test failure.
- Added `provider_unavailability_after_intent_commit_consumes_permit_without_effect_or_replay`.
  It commits the independent synthetic Intent, scripts Provider unavailability
  before the ledger write, observes the exact RequestId/IDK Intent still
  present, counts zero matching effect rows through a separately opened ledger
  query, and confirms a second attempt returns `AlreadyConsumed` with zero
  effects. This is simulator behavior, not a Host readiness guarantee.
- Added `mismatched_attempt_does_not_consume_permit_before_a_later_exact_binding`.
  Current private mutable test Permit rejects wrong IDK with zero effects,
  remains unconsumed, then permits exactly one later matching call (one effect).
  The accepted contract does not say whether a mismatched attempted use should
  burn the token. The decision package recommends a future non-Clone, by-value
  `invoke_once(self, ...)` that consumes on every attempted use; that API detail
  remains `PROPOSED` for Owner ratification and is not implemented in runtime.

The decision package now records these behaviors and their Owner choices.
Provider availability remains an in-memory snapshot with unavoidable
post-commit TOCTOU; a persisted Ready bit would not be authority. No production
Provider API, TaskEngine/Host call path, Credentials, Receipt, retry permission,
canonical migration or state transition was added. Existing migrations
0001–0008, the ordinary migration catalog and candidate SQL files are unchanged.

#### Local validation on this continuation branch

Rust 1.85.0 was installed in the dedicated execution account because the
provided environment initially had no `cargo`, `rustc` or `rustup` on PATH.
Locked dependencies were fetched with `cargo +1.85.0 fetch --locked`; all
subsequent Cargo commands used the clone-local target directory and offline
mode. Exact observed gates:

- `cargo +1.85.0 test -p serea-storage --all-targets --locked --offline p8_`:
  PASS 16, failed 0, filtered 518, ignored 0.
- `cargo +1.85.0 test -p serea-storage --all-targets --all-features --locked --offline`:
  PASS 640 unit + 6 integration tests, failed 0; 2 manual resource benchmarks
  ignored. This run includes the new RED/GREEN regression in its restored state.
- `cargo +1.85.0 check --workspace --all-targets --all-features --locked --offline`:
  PASS.
- `cargo +1.85.0 clippy --workspace --all-targets --all-features --locked --offline -- -D warnings`:
  PASS.
- `cargo +1.85.0 fmt --all -- --check`, `git diff --check`,
  `python3 tools/validate_docs.py docs` (145 Markdown files),
  `python3 tests/workspace_smoke.py`,
  `python3 -m unittest discover -s tests -p workspace_smoke_tests.py` (82),
  `python3 tools/check_synthetic_host_boundary.py`: PASS.
- `tools/check_core_api_boundary.py`: PASS using a separate external consumer;
  expected `no method named install_synthetic_registry_generation` refusal.
- `cargo +1.85.0 test --workspace --all-targets --locked --offline` was
  executed; every emitted suite completed without a failure, including the
  P8 Storage and TaskEngine recovery/lifecycle suites. The tool session closed
  before its final summary/exit code could be captured, so retain this as
  **PASS by observed suites, final aggregate exit code unavailable**.
- `cargo +1.85.0 test --workspace --all-features --locked --offline`:
  **PASS**, exit 0, 79 test/doctest summaries, 1,517 passed, 0 failed, 2
  ignored manual benchmarks. The separate all-features Storage command above
  includes `--all-targets`; workspace all-features run completed all binaries,
  integration suites and doctests.

#### Owner decisions still blocking runtime integration

Keep the existing five decision groups and their recommended selections in
the [contract closure package](P8B-contract-closure-package-2026-10-16.md):
provider Budget classification; concrete provider availability and pinning;
commit-to-invoke Permit and cancellation/revocation ordering; unresolved and
resolved Intent retention/Forget; and bounded `CAPABILITY_REQUESTED` payload.
The current extra unresolved detail is whether a mismatched in-process Permit
attempt burns the Permit. Separate P7 blockers remain synthetic Owner issuer
authentication, 0009 canonical policy/schema approval and independent security
review. None is implied approved by this unattended engineering instruction.

#### Normal commit checkpoint

The cohesive code slice was committed normally as `132d2d6`
(`test(p8): pin synthetic permit readiness boundaries`), parented directly on
PR #26 head `72be5ac56d01765b8c9dea79ee3e4f9d1fc5bbc4`. It contains only the
Storage test-only Permit/readiness coverage. The current uncommitted change to
this ExecPlan records the complete local evidence; it will be committed as a
second normal checkpoint after format/docs/boundary validation. The original
clone is untouched. No remote ref has been pushed and no new PR has yet been
created at this checkpoint.

**Resume point:** finish this plan-only checkpoint commit on
`p8b/authority-boundary-2026-10-17`, fetch/review the #26 base, run explicit
Git pre-push safety checks, and push this branch only. Open at most one Draft
PR with base `p8b/overnight-2026-10-17` (PR #26), then capture exact new-head
Fast/Full/Native/SQLite/Hooks run IDs, attempts and results. Keep PRs #19–#26
unchanged and do not merge or close any PR. Continue only with safe test-only
slices; runtime invoke and all blocked contracts remain closed.

#### Follow-up Budget and contract-text audit — 2026-10-17

After creating Draft PR #27, an additional safe test gap was closed locally:
`real_storage_task_budget_limit_two_commits_exactly_two_eligible_steps` races
two distinct approved Steps through separate real `Store` connections with
`max_tool_calls=2`. After reopen it requires exactly two Intent rows, two P6
Grant-use rows, two Event-content rows and two distinct Steps; the Task limit
is 2, the Grant has zero uses remaining, and `verify_integrity()` succeeds.
Rust 1.85.0 focused test passed (1 passed, 640 filtered); the adjacent
`real_storage_task_budget_` selector passed both limit-one refusal and limit-
two exact-fill tests. This completes zero, one, and N=2 boundary cases without
adding a production counter or API.

Hostile document review found stale §A text in the Owner package implying a
new `DISPATCH_INTENT_COMMITTED` EventKind was still required. This contradicted
Event Protocol §3.3 and the package's own §E. The stale text is corrected to
reuse the already accepted `CAPABILITY_REQUESTED` for durable Intent commit;
payload details remain proposed and the kind does not prove invocation or
effect.

The initial PR #27 head `371d7a00d1351b5db1940aa718ab593e8ffa812c` had Fast
`38984492101` and Git Hooks `38984492069` completed success when inspected;
Full `38984492097`, Native `38984492091`, and SQLite `38984492077` were
incomplete or later cancelled. The budget test and Event correction were
recorded in normal commit `558791db03312e7933769ec859b49fc5e1e9aeb4`, pushed
to the same branch, and the existing Draft PR #27 description was updated.
That commit supersedes the initial-head CI results.

**Checkpoint before the final Storage rerun:** PR #27 remained Open/Draft, based directly on PR #26,
with remote head `558791db03312e7933769ec859b49fc5e1e9aeb4`. At the last
checkpoint, exact-head Git Hooks `38984901012` had completed success; Fast
`38984900980` and SQLite `38984900983` were in progress; Full `38984900981`
and Native `38984900996` were pending. Query these five runs again and record
their final attempts/conclusions before treating CI as complete. The worktree
was clean after the push. No further code task is authorized without an
approved runtime contract; preserve fail-closed behavior and record any new
safe independent test opportunity in this plan before changing scope. Keep
PRs #19–#26 untouched and do not create another PR.

#### Final Storage regression rerun — 2026-10-17

After the budget-limit-two test was added, the complete command
`cargo +1.85.0 test -p serea-storage --all-targets --all-features --locked --offline`
was rerun locally: **641 unit tests and 6 integration tests passed, 0 failed;
2 manual resource benchmarks were ignored**. This includes the process-death
and crash-recovery suites; several child-process tests exceeded 60 seconds
before completing successfully. The focused real-Storage budget selector also
passed both limit-one and limit-two concurrency cases. Formatting passed after
these reruns. No runtime authority or schema changed.

The preceding PR head `18182a5eadc0dcca54526cd3a154a8aa35ab5a15` had latest
workflow IDs Fast `38985010024`, Full `38985009991`, Native `38985009990`,
SQLite `38985009972`, and Git Hooks `38985009961`; statuses were still
in-progress, pending, queued, pending, and in-progress respectively when last
queried. A documentation-only checkpoint records this rerun and supersedes
that head, so re-fetch PR #27's current head and use only its exact-SHA runs
for the final CI disposition.
