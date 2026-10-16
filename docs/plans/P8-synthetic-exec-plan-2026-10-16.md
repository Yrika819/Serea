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
| 3 — P8B 0010+ | DATA-SHAPE PROTOTYPE ONLY | A separate 0010 dispatch-intent candidate is being tested after the P7 candidate; it remains outside the ordinary catalog. There is no dispatch write API or provider call. Exact Budget accounting, commit-to-invoke eligibility and dynamic revalidation remain PROPOSED. |
| 4 — P8C Fake Provider | BLOCKED ON P8B AUTHORITY GATE | Current TaskEngine has no CapabilityProvider invocation path. No fake effect is reachable until one atomic authorization transaction and the commit-to-invoke rule are implemented and tested. No network, credentials or real accounts. |
| 5 — CI / hostile review | NOT STARTED | Runs must match each final SHA and its actual base. |

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

## Production boundary evidence

This ExecPlan and any dependent PR are schema/test development only. They do
not enable production Memory or provider invocation. Owner OS authentication,
key custody, rollback-resistant authority, production migration/backfill, Gate
D, real credentials/providers and real external effects remain unauthorized.

## Handoff

Last dependency head is the #19 SHA above. Local work branch is
`p8/approved-contract-candidate`. The task-owned clone has Codex local identity
and repository hooks. The branch must remain a Draft stacked on
`codex/post-gate-c-review` until #19 is merged through protected procedure.
