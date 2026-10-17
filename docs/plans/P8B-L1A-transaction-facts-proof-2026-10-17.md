# P8B L1-A transaction facts proof — 2026-10-17

**Status:** `L1A_PARTIAL`; synthetic durable-fact evidence only. This plan does
not approve an architecture or provide dispatch authority.

## Baseline and isolation

- GitHub main at intake: `e0acb0254820c381dd97828824bef86a1db5176c`.
- PR #30: open Draft, head `ba16eaa7c45e51e30aa3185bd6f6a96484ba3c42`,
  base branch `p8b/authority-architecture-proof-2026-10-17`; PR #29 is its
  ancestor at `bb4c4d2b8a3bf015d735ab0ad519523f6b961299`.
- PR #31 or any later downstream Draft was not present at intake. PR #14 and
  #19–#30 were open Drafts; #30 exact-head Fast, Full, Native (12), SQLite
  portability (4), and Git Hooks checks were successful. These are baseline
  only. Branch-protection API returned HTTP 403, so protection state is unknown.
- Original checkout `/workspace/Serea` remains clean at main. Work is isolated
  at `/workspace/Serea-p8b-l1a`, branch
  `codex/p8b-l1a-facts-proof-2026-10-17`, starting at the exact #30 head.
- No pre-existing execution-record file was found at intake. Never delete or
  overwrite any user artifact.
- ADR-0046 remains Proposed. No canonical migration, Intent writer, Permit,
  CommitProof, provider invocation, credentials, production Memory, Receipt,
  Retry, or external effect is in scope.

## Fact provenance and limits

| Fact | Authority source / reader | Same transaction | Current trust class / limit |
|---|---|---:|---|
| Task ID, state, cancellation, deadline, data/risk ceiling, current plan revision, Step ID/kind/status/input digest, current membership, historical plan/input bytes | Durable SQLite; `Tx::load_task` validates Task, current plan history, Step immutable fields, blob ref and digest | Yes | `VERIFIED_DURABLE`; selector IDs must be compared with loaded values |
| Lease owner/generation/expiry/release | Durable `leases` row; private `lease::matching` checks guard origin and owner/generation/release | Existing transition calls only | `VERIFIED_DURABLE` row is readable internally; no public read-only `Tx` boundary yet. Owner/Worker identity and clock are not authenticated |
| Grant ID/status/member/action digest/remaining uses/expiry/consumption | Durable approval tables; `Tx::consume_grant_use` currently reads some facts while mutating | No read-only reader | `UNAVAILABLE_OR_UNRATIFIED` until a narrow read-only projection exists. Caller time is not a trusted clock |
| Active Policy revision and canonical rules digest/content | `Tx::current_policy_revision`; existing P6 evaluator via `evaluate_storage_snapshot` | Yes | `VERIFIED_DURABLE` snapshot; Policy decision still depends on caller-supplied host facts |
| Registry generation, activated manifest/schema digests, Step binding, immutable descriptor and overlay revision/state/opt-in | Existing typed `Tx` readers in `capability_registry.rs` | Yes | `VERIFIED_DURABLE`; compare all pinned fields, never trust `PreparedActionV1` as their source |
| Prepared arguments and their input digest | Caller `PreparedActionV1`; durable Step input digest and canonical bytes can be read by `load_task` | Read-only comparison possible | `HOST_PROVIDED_UNVERIFIED` proposal until exact DB digest/content binding is checked |
| `requested_by`, argument data class, device posture, destructive opt-in, automation context | Host/P5 or separate authenticated sources; not recoverable from current durable Task/Step projection as trusted facts | No | `HOST_PROVIDED_UNVERIFIED` or `UNAVAILABLE_OR_UNRATIFIED`; do not evaluate using caller relabels |
| Owner issuer/authentication, Worker origin, trusted current time, exact live Provider instance/readiness | No independently demonstrated issuer/lifecycle API | No | `UNAVAILABLE_OR_UNRATIFIED`; blocks L1-B/dispatch authority |
| Budget, Intent, IDK reservation, RequestId, commit-to-Permit | No approved runtime writer/factory | No | Out of scope; no write or effect |

## Invariants and adverse cases

The target observation must stay non-authorizing and must not be accepted by an
invoke API. One `Store::transact` callback is one real SQLite `BEGIN IMMEDIATE`
serialization boundary; all returned durable facts are read through its `Tx`.
Errors and missing/corrupt rows fail closed. No callback re-enters Store.

Evidence targets: (1) exact Task/Step/Plan/digest correspondence; (2) policy
snapshot and recomputation despite forged outer `PolicyEvaluation.decision`;
(3) Registry/Descriptor/Overlay pin equality; (4) read-only Lease fence and
Grant eligibility projections; (5) cancellation, policy, grant, and lease
writers on independent Store handles block behind a held writer and are
observed after release/reopen; (6) rollback, integrity/foreign-key check and
zero Grant-use/Event/Intent/Budget/IDK/RequestId/effect writes; (7) external
Cargo probes for public composition and lack of an invocation/proof factory.
Trusted provenance gaps must remain explicit blockers.

## Work log and validation

- Intake commands: `git status --short --branch`, `git rev-parse HEAD`,
  `git worktree list --porcelain`, `git fetch origin main
  refs/pull/30/head:refs/remotes/origin/pr/30
  refs/pull/29/head:refs/remotes/origin/pr/29`, `gh pr view 30`,
  `gh pr list --state open`, and `gh api repos/Yrika819/Serea/branches/main/protection`.
- Toolchains available: Rust 1.85.0 and stable. Commands run with
  `PATH=/workspace/.cargo/bin:$PATH`, `RUSTUP_HOME=/workspace/.rustup`,
  `CARGO_HOME=/workspace/.cargo`. `cargo +1.85.0 metadata --locked --offline
  --no-deps --format-version 1`: exit 0. Stable/Rust 1.85 version checks:
  PASS. Disk available: 29 GiB.
- Actual code/tests/RED-GREEN/QA and exact-head GitHub workflow evidence are
  recorded below as performed. No prior PR check is carried forward as proof
  for a new commit.

### Implemented read-only boundary

- `Tx::observe_lease_facts(&LeaseGuard)` uses the existing private
  `matching()` fence to validate origin, task/step, owner, generation, and
  unreleased state. It returns only the durable acquired/expiry values. It
  accepts no time, does not authenticate the worker, and does not mutate.
- `Tx::observe_grant_facts(grant_id, task_id, step_id)` returns a typed,
  bounded Grant projection: identity/pins, status, member/action digests,
  declared/observed member counts, seal presence, remaining uses, stored
  expiry, and exact selected-Step use. It never consumes a use. The selected
  Step member is visible; the entire member vector is not returned. Expiry
  validity cannot be classified without an accepted trusted clock.
- `TaskEngine::observe_l1_facts(prepared, lease_guard, optional_grant)` calls
  `Store::transact` once and uses its `Tx` for `load_task`, registry pin and
  generation, binding, descriptor, overlay, active Policy, Lease and optional
  Grant reads. It compares the prepared selector to durable Step and Registry
  pins, returning mismatch codes. It does not create a PolicyInput from
  caller-supplied facts or produce a PolicyEvaluation/authorization result.
  The result has private fields/no constructor and no Debug output. Lease owner
  is redacted from its Debug view.
- No schema, migration catalog, feature, dependency edge, Host invoke path,
  Intent, budget write, IDK reservation, RequestId, Permit, or CommitProof was
  added. ADR-0046 remains Proposed.

### Focused real-SQLite evidence

| Case | Command/test | Outcome and boundary |
|---|---|---|
| Task/Step/Plan/Policy/Registry/Descriptor/Overlay/Lease joined under `Store::transact` | `cargo +1.85.0 test -p serea-task-engine --test p5e_capability_binding task_engine_policy_input_is_derived_from_current_pinned_durable_facts --locked --offline` | PASS on MSRV and stable; fixture is file-backed, reads exact durable pins, sees active Policy and Lease in one transaction, returns input-digest mismatch for alternate prepared input, sees a later independently written disabled Overlay, then reopen + `verify_integrity()` succeeds. Mutating public `PolicyEvaluation.decision` does not enter the observation API. No Policy evaluation is part of this observation. |
| Lease origin/generation and no writes | `lease::lease_tests::lease_fact_observation_is_read_only_fenced_and_does_not_launder_rollback` | PASS; committed guard sees current row, released/reclaimed generation is re-read, rollback-origin guard is fenced, row is unchanged by observation. In-memory SQLite fixture; the joined TaskEngine proof uses a file-backed fixture. |
| Grant member/nonmember/unused/used/exhausted + reopen | `migration_0005_tests::read_only_grant_fact_projection_observes_member_and_use_without_consuming` | PASS; typed approval response creates fixture; read-only member and nonmember projections leave Grant-use and event-content counts unchanged; subsequent supported consumption is observed, and independent reopen + integrity verification retains it. File-backed SQLite. |
| Grant revoke state | `migration_0005_tests::authenticated_revocation_is_grant_only_atomic_and_preserves_prior_consumption` | PASS; projection sees durable `REVOKED`, sealed member count and selected member digest, without using it as authority. |
| Cancellation, policy, grant-revoke and lease-reclaim serialization | `storage_tests::active_policy_change_waits_for_transaction_scoped_p6_evaluation`; `migration_0005_tests::independent_connections_serialize_task_cancellation_against_authenticated_response`; `migration_0005_tests::independent_connections_serialize_r2_member_consumption_against_revocation`; `lease::outcome_tests::committed_reclaim_wins_before_cross_store_old_outcome_attempt` | PASS on independent SQLite handles. These are existing adjacent transaction fences, not a barrier-held race directly against `observe_l1_facts`; that narrower race remains untested. |
| Digest comparison RED/GREEN | Independent detached worktree `/workspace/Serea-p8b-l1a-red`, own `/tmp/p8b-l1a-red-target`; test `task_engine_policy_input_is_derived_from_current_pinned_durable_facts` | RED exit 101 after disabling exact Step input-digest comparison; expected `StepInputDigest` mismatch assertion fails. Restoring the check: GREEN exit 0. Scratch worktree/target removed. |

### External Cargo and local QA

- `tools/p8b_authority_cargo_probe/run.py`: PASS on stable 1.99.0 and MSRV
  1.85.0. A real external consumer can call the two typed read-only Tx
  projections. It cannot construct `L1FactObservation` (private fields), reach
  `Tx.inner` (E0616), add raw SQL (E0599), or import current Permit/CommitProof
  names (E0432). E0432 only means those current symbols are absent; it is not
  evidence about a future factory. Existing positive source compiles a
  caller-owned Provider with default Ready, relabels outer PolicyEvaluation,
  and composes Grant-use + Event. These compile-only attacks create no
  invocation because no relevant dispatcher API exists.
- MSRV 1.85.0: `fetch --locked`, `fmt --all -- --check`, workspace
  `check --all-targets --all-features`, workspace all-target/all-feature tests
  (exit 0), Clippy `-D warnings`, and release Host `check --no-default-features`
  all PASS. The full suite had 643 Storage unit tests; overall command exit 0.
- Stable 1.99.0: workspace all-target/all-feature tests (exit 0), fmt, check,
  Clippy `-D warnings`, and release Host no-default-features check PASS. Cargo
  printed the pre-existing vendored `serde_json` deprecation warning for
  `usize::max_value`; no warning is attributed to changed code.
- `cargo metadata --locked --offline --format-version 1` passed on both
  toolchains. `python3 tools/validate_docs.py docs` scanned 152 Markdown files
  and passed; workspace smoke and unittest passed (82); static Synthetic Host
  boundary passed. Core API boundary initially lacked Cargo on PATH, then
  passed with the explicit Rust environment. `git diff --check` passed.
- Latest #30 Actions (baseline only): Fast run `39008797531`, Full
  `39008797540`, Native `39008797538` (12 jobs), SQLite portability
  `39008797541` (4 jobs), Git Hooks `39008797528`; all reported success. These
  runs do not validate this branch. New branch final exact-head workflow
  details are recorded after push below.

### Residual facts and Owner decisions

Durable Task/Step/Plan, Policy snapshot, Registry/Descriptor/Overlay, current
Lease fence and selected Grant membership/use state can be read together. The
observation deliberately does not conclude that an action is allowed. It does
not evaluate canonical P6 because current inputs include Host provenance not
reconstructed from the database.

| Missing root | Source currently available | What would be needed before an L1-B authority claim |
|---|---|---|
| Owner issuer/authentication | Approval response API documents an external authenticated port, but this task has no independently demonstrated issuer instance/identity | Owner decision naming the issuer, typed identity/session proof source, freshness and revocation contract; evidence that the Host verifies it |
| Worker origin | `LeaseGuard` proves acquisition origin and durable generation only; owner string is an input | Owner decision and Host/OS worker principal source bound to the acquisition API and Task/Step; forged strings rejected |
| Trusted clock | `Store` has injected `Clock`, but the Tx reader has no trusted clock witness bound to the Host execution | Owner-approved Host clock authority and freshness/rollback contract; expiry/revocation tests use that source, not caller time |
| Provider instance/readiness | capability snapshot contains provider health by ID, and arbitrary consumers may implement default Ready; no Host private actual-instance token | Owner-approved Host-owned instance identity/lifecycle lock plus fresh precommit and preinvoke readiness checks bound to pins |
| No-Grant OBSERVE shape | no ratified exact behavior, no-Grant is represented only as absent optional observation | Owner decision for OBSERVE/no-approval authorization shape and exact durable constraints |
| Full Grant set | projection exposes commitment/count/seal and selected Step member, not all member identities | Decide whether the complete sealed set must be reloaded/verified on every selected action; if yes, authorize a bounded member-list reader or canonical proof comparison |
| Observer-specific writer race | existing separate policy/cancel/grant/lease Store races pass | Barrier-held second Store write directly against this observation API if stronger implementation-specific race evidence is required |

No same-process database-file tamper/restore, Host compromise, physical
power-loss, or external provider effect is covered. The next engineering action
recommended is one Owner decision defining authenticated Worker and trusted
Host time sources plus no-Grant OBSERVE shape; then add a barrier-held
`observe_l1_facts` race and evaluate only DB-derived P6 facts while leaving
unknown provenance blocked.

## Pending work

| Item | State | Next action |
|---|---|---|
| Typed read-only Lease/Grant boundary | Completed in working tree | `Tx::observe_lease_facts` and `Tx::observe_grant_facts`; bounded and read-only |
| Same-transaction cross-fact observation | Completed, non-authorizing | `TaskEngine::observe_l1_facts`; external trust facts remain unavailable |
| Real SQLite adversarial/race/reopen evidence | Partial | File-backed observation and Grant reopen; existing adjacent independent-Store races pass; observation-specific held-writer race remains pending |
| External consumer and RED/GREEN | Completed, synthetic | Stable/MSRV probes and one exact digest comparison RED/GREEN |
| Stable/MSRV QA and exact-head workflow | Implementation commit complete; all GitHub Actions passed | Fast `39017188986`, Full `39017188975`, Native 12 jobs `39017188954`, SQLite portability 4 jobs `39017188930`, Git hooks `39017188973`; all success on `5ceb23b53e9341af2d6af78c053a7abba20d98d6` |

## Next restart point

If further work is requested, resume from the final pushed SHA recorded in the
Japanese report and `/workspace/Serea-p8b-l1a`. The current implementation commit
is `5ceb23b53e9341af2d6af78c053a7abba20d98d6`; a docs-only evidence update may
advance the final branch HEAD. Start with the observer-specific
second-Store barrier race in `crates/serea-task-engine/tests/p5e_capability_binding.rs`,
then only proceed after Owner resolves the four trust roots in the table above.
Do not claim a Permit or complete P8B L1 authorization.
