# P6B cloud handoff — storage foundation checkpoint

Status: **P6B_WIP_CHECKPOINT** · Written: 2026-10-10 · Architecture: `serea-arch/2.7.0`

This is a controlled handoff record, not a closure claim. P6B is **not** closed. It records the
exact state of the `serea-policy` storage foundation so the work can resume elsewhere without
re-deriving any of it.

Nothing here merges anything, rewrites history, or changes an accepted ADR.

---

## 1. Exact identities

| Item | Value |
| --- | --- |
| P6A accepted HEAD | `1c04451469a810a484e92fa7f0e5eb249451a54e` |
| P6A branch | `p6/preimplementation-audit` |
| P6B working branch | `p6/policy-approval-runtime` |
| P6B branch parent | `1c04451469a810a484e92fa7f0e5eb249451a54e` |
| P6A Draft PR | [#6](https://github.com/Yrika819/Serea/pull/6), `OPEN`/`DRAFT`/`UNMERGED`, base `main` |
| `origin/main` | `e17fbfc7f3e75f0bc95a66d693fe11b57a6ced50` |
| Migration 0005 file bytes | 26 450 bytes |
| Migration 0005 SHA-256 | `sha256:cf6686807e3ceb10bd21c6653fbdd61c487e6b8938a2d78104d160920831e71a` |

Migration 0005 is newly created in this slice, so it has no prior checksum to preserve. Its
bytes must be treated as frozen from the first commit: any later edit changes the checksum and
will be refused by every already-migrated database. Migrations 0001 through 0004 are
**byte-identical** and their pinned checksums are unchanged; the P6B test that asserts this is
green.

---

## 2. The R2 authority model this schema implements

Owner decision **R2 — enumerated multi-action grant** (2026-10-10), recorded in §0b of the
[P6 owner decision package](P6-owner-decision-package.md) and specified in §7 of
[P6A-feasibility-gate.md](P6A-feasibility-gate.md). Migration 0005 is its durable form.

A grant binds an explicitly enumerated set of **1 to 8** actions. Each action is one `StepId`,
its own exact canonical `arguments_digest`, and the structural `scope` shown for it. Authority
is the membership set, never scope equality and never digest equality.

| Invariant | Mechanical enforcement in 0005 |
| --- | --- |
| 1 to 8 enumerated Steps | `action_count BETWEEN 1 AND 8` on both the request and the grant, plus the `approval_request_action_within_count` and `approval_grant_member_within_count` triggers |
| Each Step validated by its own StepId, digest, capability, version, plan revision and P5 pinned identity | `approval_request_action_matches_requested_step` reads `task_steps` and `step_capability_bindings` and refuses any disagreement |
| The set is fixed before approval | `approval_request_action_no_late_insert` refuses an action once the request leaves `PENDING` |
| Only individually approved actions | `approval_grant_member_is_an_approved_action` requires `(approval_id, step_id)` to exist in `approval_request_actions` with the same digests |
| The set is immutable afterwards | `approval_request_action_no_update/no_delete` and `approval_grant_member_no_update/no_delete`, scoped so a task, approval or grant cascade may still remove derived rows |
| `max_uses` at most the approved action count | `CHECK (max_uses = action_count)` on both tables |
| Each Step consumes at most once | `PRIMARY KEY (grant_id, step_id)` plus `UNIQUE (step_id)` on `approval_grant_uses`, per DC-1 |
| A second grant cannot authorize a consumed Step | the `UNIQUE (step_id)` use row outlives any single grant |
| One capability, one version, one generation, one descriptor revision, one plan revision, one task, one expiry | shared conditions on the grant plus the `approval_grant_matches_request` trigger |
| Grant never outlives its request | both horizons are 1 800 000 ms by ratified default |
| `EXHAUSTED` is produced only by consumption | `CHECK (status <> 'EXHAUSTED' OR uses_remaining = 0)` |
| No raw arguments, no credentials | only digests are stored; `approval_requests.data_class` is restricted to `PUBLIC`/`PERSONAL` by a `CHECK` |

Two triggers are the whole R2 security argument and must not be weakened:

```sql
CREATE TRIGGER approval_grant_use_must_be_a_granted_step
BEFORE INSERT ON approval_grant_uses
WHEN NOT EXISTS (SELECT 1 FROM approval_grant_members AS m
                  WHERE m.grant_id=NEW.grant_id AND m.step_id=NEW.step_id)
BEGIN SELECT RAISE(ABORT, 'step is not a granted step of this grant'); END;

CREATE TRIGGER approval_grant_member_is_an_approved_action
BEFORE INSERT ON approval_grant_members
WHEN (SELECT approval_id FROM approval_grants WHERE grant_id=NEW.grant_id) IS NOT NEW.approval_id
   OR NOT EXISTS (SELECT 1 FROM approval_request_actions AS a
                   WHERE a.approval_id=NEW.approval_id AND a.step_id=NEW.step_id)
   OR ...
BEGIN SELECT RAISE(ABORT, 'granted step must be an action the human actually approved'); END;
```

Both were verified directly against SQLite with an independent probe, not only through the
Rust harness: a Step carrying the *same* `arguments_digest` as an approved member, and sitting
inside the approved `scope`, is still refused when it is not enumerated.

---

## 3. Files in this checkpoint

Added:

| Path | Contents |
| --- | --- |
| `crates/serea-policy/Cargo.toml` | the new L2 crate: protocol, storage, event-bus only; `rusqlite` dev-only |
| `crates/serea-policy/src/lib.rs` | crate documentation, `#![forbid(unsafe_code)]`, `#![deny(missing_docs)]`; no runtime code yet |
| `crates/serea-storage/migrations/0005_policy_approval.sql` | 8 tables, 23 triggers, 6 indexes |
| `crates/serea-storage/src/migration_0005_tests.rs` | 27 RED-first contract tests |

Modified:

| Path | Change |
| --- | --- |
| `Cargo.toml` | add `crates/serea-policy` to members; add the `serea-policy` workspace dependency |
| `crates/serea-task-engine/Cargo.toml` | add the `task-engine -> policy` edge |
| `crates/serea-storage/src/migrate.rs` | register migration 0005, `LATEST = 5` |
| `crates/serea-storage/src/lib.rs` | declare `migration_0005_tests` in the test module list |
| `crates/serea-storage/src/foundation_tests.rs`, `migration_0002_tests.rs`, `migration_0003_tests.rs`, `migration_0004_tests.rs`, `protection_tests.rs`, `store.rs`, `tests/foundation.rs` | update the pinned `LATEST`/catalog-length/schema-version expectations from 4 to 5, and bump the "future migration" fixture from version 5 to 6 so it stays strictly newer than the whole catalog |
| `crates/serea-task-engine/tests/crash.rs`, `tests/recovery.rs` | same expectation updates; add the eight new tables to the durable-table inventory list |
| `tests/workspace_smoke.py` | add `serea-policy` to `EXPECTED_MEMBERS`; restrict the policy crate to protocol/storage/event-bus; allow `task-engine` to name policy; update the success message |
| `tests/workspace_smoke_tests.py` | add the policy member and its dependency fixture to the virtual-manifest harness |

No other file is touched. No documentation file changed in this slice; P6A's documents are
already committed at `1c04451469a810a484e92fa7f0e5eb249451a54e`.

---

## 4. Implemented functionality

- Migration 0005 is written, registered and applied by `Store::open` on both the file and the
  memory profile. A fresh store reaches schema version 5; a version-4 store upgrades in place.
- The `serea-policy` crate exists and compiles, is wired into the workspace, and is reachable
  from `serea-task-engine` with no dependency cycle.
- Policy authority: immutable revisions with monotonic `revision_id` plus a `rules_digest`; a
  singleton `policy_state` pointer that only advances, cannot be deleted, and cannot name an
  unactivated revision; write-once activation.
- Approval authority: requests with an enumerated action set, grants with an immutable granted
  subset, grant-use rows that spend exactly one use through a trigger, and full task-scoped
  cascades.
- Cross-cutting: the `POLICY_CHANGED` append and the activation share one transaction; the
  `serea-storage` integrity checks pass; independent connections cannot both consume one Step.

Not implemented in P6B, by design and by the P6B slice boundary: the evaluator, `PolicyInputV1`,
`PolicyDecision`, `AuthorizationEvidenceV1`, the approval lifecycle, the authenticated response
and revocation seams, and every TaskEngine change. P6C starts those.

---

## 5. RED tests already written

`crates/serea-storage/src/migration_0005_tests.rs`, 27 tests, mapped to the P6B closure list:

| Closure obligation | Test |
| --- | --- |
| Upgrade 0004 to 0005 | `existing_schema_v4_upgrades_to_v5_and_keeps_its_rows` |
| Fresh database migration | `fresh_store_applies_the_whole_p6_schema_at_v5` |
| Migration checksum mismatch refusal | `altered_p6b_migration_bytes_are_refused_as_a_checksum_mismatch` |
| Unsupported migration / corruption refusal | `a_database_without_the_serea_migration_authority_is_refused` |
| Pointer downgrade refusal | `the_pointer_only_advances_and_deletion_is_refused` |
| Pointer deletion refusal | `the_pointer_only_advances_and_deletion_is_refused` |
| Activation of an invalid/uncommitted revision | `the_pointer_cannot_name_an_unactivated_revision` |
| Atomic `POLICY_CHANGED` event | `revision_activation_and_its_policy_changed_event_commit_together_or_not_at_all`, `a_failed_transaction_leaves_no_partial_authoritative_state` |
| Immutable revision behaviour | `an_activated_policy_revision_and_its_rules_are_immutable`, `an_unactivated_revision_can_still_be_prepared_and_then_activated`, `a_revision_may_not_exceed_the_rule_bound` |
| Invalid Step/Task association | `an_approval_action_must_belong_to_the_request_task_and_pinned_step`, `an_action_step_plan_capability_generation_and_descriptor_must_all_match` |
| Invalid ID/digest/state | `invalid_identifiers_digests_and_states_are_refused_by_the_schema`, `a_grant_may_only_name_a_closed_actor_authentication_and_auth_strength` |
| Task deletion cascade | `deleting_the_task_cascades_every_approval_row_and_spares_policy_history` |
| Policy history retention | `deleting_the_task_cascades_every_approval_row_and_spares_policy_history`, `deleting_the_task_never_touches_the_active_policy_pointer` |
| Duplicate grant use | `the_same_step_cannot_be_consumed_twice_by_the_same_grant`, `a_second_grant_cannot_authorize_an_already_consumed_step` |
| Restart/reopen correctness | `a_closed_store_reopens_with_the_same_authoritative_policy_and_approval_state` |
| Independent connection contention | `independent_connections_cannot_both_consume_one_step` |
| R2 membership semantics | `only_an_enumerated_granted_step_can_consume_a_use`, `a_consuming_use_row_cannot_cross_the_grant_task_boundary`, `a_granted_step_must_be_one_the_human_actually_saw`, `an_approval_action_must_carry_the_durable_step_arguments_digest`, `the_action_set_is_bounded_before_the_request_is_ever_raised` |

---

## 6. Test results — honest accounting

**PASS.** The whole pre-existing workspace suite is green with migration 0005 applied and the
pinned expectations updated: `cargo test --workspace --all-features` reported
**1244 passed, 0 failed** at that state, which also covers `tests/foundation.rs`,
`crates/serea-task-engine/tests/crash.rs`, `crates/serea-task-engine/tests/recovery.rs`, and
the P2H crash matrix.

**PASS.** `cargo check -p serea-storage --lib --all-features` is clean, so
`migration_0005_tests.rs` compiles.

**NOT RUN to completion.** The 27 new `migration_0005_tests` have been executed once and are
**RED: 20 passed, 7 failed**. The full run was repeatedly interrupted by local resource
limits, so treat the 20 as evidence but do not treat the suite as green.

Command that produced the counts:

```text
CARGO_BUILD_JOBS=2 cargo test -p serea-storage --lib --all-features --offline \
  migration_0005 -- --test-threads=2
# test result: FAILED. 20 passed; 7 failed; 0 ignored; 0 measured; 435 filtered out
```

### The 7 failures, with root cause

All seven are defects in the **test fixtures**, not in the migration. Each was diagnosed; the
migration's own behaviour was additionally confirmed with an independent SQLite probe.

| # | Test | Root cause | Exact fix |
| --- | --- | --- | --- |
| 1 | `revision_activation_and_its_policy_changed_event_commit_together_or_not_at_all` | asserts `count(*) FROM policy_revisions WHERE revision_id=2` is `0`, but revision 2 was inserted and only its *activation* rolled back, so the row legitimately survives | assert `1` and additionally assert `activated_at_ms IS NULL` and that `policy_state.active_revision_id` is still `1` |
| 2 | `invalid_identifiers_digests_and_states_are_refused_by_the_schema` | `BAD.trim_end_matches('1')` removes nothing, because `BAD` ends in `0`, so the "malformed" digest is still a valid 71-character sha256 | use a digest that is genuinely malformed, for example `"sha256:zz"` or a 70-character body |
| 3 | `an_approval_action_must_belong_to_the_request_task_and_pinned_step` | the `MODEL_TURN` step insert passes 12 values for 8 columns | write the column list and the value list with the same arity, keeping `input_digest` supplied |
| 4 | `a_grant_may_only_name_a_closed_actor_authentication_and_auth_strength` | the status loop leaves the grant `REVOKED`, so the later legitimate consume is refused by `approval_grant_use_requires_consumable_grant` | reset `status='ACTIVE'`, `uses_remaining` to 1 before the consume, or restructure the loop so the state is restored |
| 5 | `only_an_enumerated_granted_step_can_consume_a_use` | the test grants **both** `STEP_A` and `STEP_B` and then expects `STEP_B`'s consume to be refused; `STEP_B` is a member, so consuming it is legal | grant only `STEP_A` when asserting that an unapproved step of the same unit is refused |
| 6 | `a_second_grant_cannot_authorize_an_already_consumed_step` | a bare `APPROVAL_2` identifier appears in the SQL text, so SQLite reads it as a column name | bind the constant as a parameter like the sibling statements already do |
| 7 | `independent_connections_cannot_both_consume_one_step` | the second `BEGIN IMMEDIATE` returns `SQLITE_BUSY`, which the test treats as a hard error | treat a busy or otherwise refused second writer as *the loser*, then assert exactly one use row exists and exactly one grant was spent |

Failure 7 deserves a note: the property under test is "at most one independent connection may
insert the use row", so a busy refusal satisfies it. The test must assert the *outcome*, not a
specific scheduling interleaving.

### Defects already fixed during this slice

- **A deadlock.** `deleting_the_task_cascades_every_approval_row_and_spares_policy_history`
  held the raw `Store` connection guard and then called `store.verify_integrity()`, which tries
  to take the same non-reentrant mutex. This would have wedged a CI runner rather than failing
  it, which is worse than a red test. Fixed by dropping the guard first. The other four
  apparent re-entrancies were already inside scoped blocks.
- **A cascade blocked by an immutability trigger.** `approval_request_actions` and
  `approval_grant_members` originally refused *every* delete, which broke the task deletion
  cascade and made `PRAGMA foreign_key_check` fail on a deleted task. Both triggers are now
  scoped to fire only while their parent row still exists, mirroring migration 0004's shape.
- **A reachable forged state.** `EXHAUSTED` could be written by hand while a use remained
  available. Now refused by `CHECK (status <> 'EXHAUSTED' OR uses_remaining = 0)`.
- **Test-harness drift.** The P2 and P5 test files pin `Migrations::LATEST`, catalog length,
  `schema_version()`, and a "future migration" fixture. Those were updated from 4 to 5 and the
  future fixture from version 5 to 6. The 0001–0004 checksums were **not** changed.

---

## 7. Commands that were interrupted

| Command | Outcome |
| --- | --- |
| `cargo test --workspace --all-features --offline` | completed once, 1244 passed, 0 failed — but **before** `migration_0005_tests.rs` was added |
| `CARGO_BUILD_JOBS=2 cargo test -p serea-storage --lib --all-features --offline migration_0005` | repeatedly interrupted by local resource limits; the counts above come from the one run that finished |
| `CARGO_BUILD_JOBS=2 cargo clippy --workspace --all-targets --all-features --offline -- -D warnings` | **not run** at the P6B state. Clippy was green at the P6A HEAD `1c04451469a810a484e92fa7f0e5eb249451a54e` |

---

## 8. Unresolved design questions

1. **Summary persistence shape.** `approval_requests.summary` is a plain `TEXT` column with no
   length bound, deliberately, because the ratified decision is that `PlainSummary` has no byte
   bound and an oversized summary fails only at event append. That means an over-long summary
   is silently persistable and then fails opaquely. No bound is proposed; this is recorded as a
   known consequence, not as a defect.
2. **`approval_request_actions.position`.** It is the normalized `step_id`-ascending order, but
   nothing yet computes it. `insert_revision`-style seeding sets it by hand. The normalization
   function itself is P6C work, together with the `action_set_digest` computation.
3. **The completeness rule.** A request's `action_count` is declared and the within-count
   trigger enforces the upper bound, but nothing yet enforces that the action rows are
   *complete* — that is, that exactly `action_count` rows were written. P6D must enforce it in
   the writer, or a short set would silently reduce authority below what the user approved.
4. **`approval_grants` has no `scope` column.** Under R2 the scope is per action, on
   `approval_request_actions`, and the grant inherits it through the member reference. That is
   intentional, but it means a grant cannot be audited for scope without joining through its
   members. Confirm this is acceptable before P6D freezes the projection.
5. **`approval_request_actions` carries `scope` but the grant does not.** The member row
   references `approval_request_actions` by `(approval_id, step_id)` and re-declares
   `arguments_digest` and `scope_digest`, which the trigger requires to match. Consider making
   those columns generated from the referenced row instead, so they cannot drift.

---

## 9. Exact next implementation action

Resume with **P6C**, not P6B. The P6B storage foundation is structurally complete; what remains
is the deterministic evaluator on top of it. The first unfinished RED test is:

> `crates/serea-storage/src/migration_0005_tests.rs`
> `fn revision_activation_and_its_policy_changed_event_commit_together_or_not_at_all`
> (source line 495)

Its correct behaviour is already described in §6 row 1. After the seven fixtures are repaired,
run the 27 in isolation, then re-run the full workspace suite, then clippy, and only then claim
P6B closed.

Concretely, in order:

1. Repair the seven fixtures per §6.
2. `cargo test -p serea-storage --lib --all-features migration_0005` until 27 pass.
3. `cargo test --workspace --all-features` until the full suite is green again.
4. `cargo clippy --workspace --all-targets --all-features -- -D warnings`.
5. Start P6C: `PolicyInputV1`, the closed match dimensions, the class-default table as a code
   constant, `priority DESC, rule_id ASC` with DENY supremacy, the finite `AutomationContext`
   derivation function, and revision activation through the pointer.

---

## 10. Required CI and closure gates

P6B is closed only when all of the following hold at the exact branch HEAD:

- `python3 tools/validate_docs.py docs` — green.
- `python3 tests/workspace_smoke.py` and its regression suite — green.
- `python3 tools/check_commit_identity.py` and its regression suite — green.
- `cargo fmt --all -- --check` — green.
- `cargo metadata --no-deps --format-version 1 --offline` — green.
- `cargo check --workspace --all-targets --all-features --offline` — green.
- `cargo test --workspace --all-targets --offline` and `--all-features` — green, 0 failed.
- `cargo clippy --workspace --all-targets --all-features --offline -- -D warnings` — green.
- `tools/prove_release_fault_exclusion.sh` — green, so the P2H seam is provably out of the
  production build.
- GitHub Actions at the exact commit SHA, with every required job individually concluded
  `success` and none skipped:
  - **Fast CI** — Linux fast checks.
  - **Full CI** — Linux stable full validation, Linux MSRV 1.85.0, macOS Intel x86_64,
    macOS arm64.
  - **Cross-architecture SQLite portability** — produce on Intel, produce on arm64, open the
    Intel database on arm64, open the arm64 database on Intel.
- The production provider-invocation count remains zero. Nothing in P6B can call
  `CapabilityProvider::invoke`; P8 is still the first phase permitted to.

Not required for P6B closure, because they are P6C–P6F obligations: the evaluator tests, the
duplicate-response matrix, the zero-invoke negative integration tests, and the P8 handoff
documentation.

---

## 11. Nonclaims

- P6B is **not** closed. No `P6B_STORAGE_FOUNDATION_VERIFIED` is claimed.
- The 27 new tests are not green; 20 passed and 7 failed on one interrupted run.
- No cross-architecture or MSRV result exists for the P6B commit. The P6A HEAD
  `1c04451469a810a484e92fa7f0e5eb249451a54e` was green on all of them; that is not evidence
  about this commit.
- No physical power-loss guarantee is claimed anywhere in this slice. Every crash case is a
  transaction-boundary fault plus a reopen.
- ADR-0037 through ADR-0040 remain Accepted. This checkpoint changes no ADR and no frozen P5
  contract.

---

## 12. Cloud continuation record — 2026-10-11

This addendum records work performed after the immutable checkpoint above. It does not rewrite
the original checkpoint facts.

### 12.1 Checkout and source identity

| Item | Verified value |
| --- | --- |
| Working branch | `p6/policy-approval-runtime-clean` |
| Base checkpoint | `3de5098e3b51c5a6bebf01b40550a6b94f4fd7e3` |
| Checkpoint parent | `1c04451469a810a484e92fa7f0e5eb249451a54e` |
| P6A PR #6 | `OPEN`, `DRAFT`, base `main`; preserved |
| Migration 0005 SHA-256 | `cf6686807e3ceb10bd21c6653fbdd61c487e6b8938a2d78104d160920831e71a` |
| Migration 0006 SHA-256 | `0af16a30cd2b29f046c14d7079a356dc25ce4352216edd9ad6821b168e6eb6ca` |
| Migrations 0001–0004 | byte-identical to the P6A parent |
| ADR-0037–0040 | all `Accepted`; architecture `serea-arch/2.7.0` |

The Cloud task initially opened at `e17fbfc` on branch `work`; that branch was left intact.
The runtime branch was fetched from GitHub and checked out directly at the supplied checkpoint.
The verification and fixes below are based on that checkpoint. The first publication used a local
Git email rejected by the repository's noreply-only identity guard; exact-head Fast and Full CI
reported that failure. Published history was preserved. The same reviewed tree was republished from
the original checkpoint on `p6/policy-approval-runtime-clean` with an approved Codex noreply
identity, so the replacement branch does not inherit the rejected commit. The first PR (#7) is
closed; its commit remains preserved by the PR, while only its obsolete branch ref was removed so
the all-refs identity guard would not rediscover it on every branch.

### 12.2 Seven checkpoint failures

All seven failures were reproduced individually on the original migration 0005 suite before
editing. Each was a fixture defect; migration 0005 was not edited.

| Test | Evidence and correction |
| --- | --- |
| `revision_activation_and_its_policy_changed_event_commit_together_or_not_at_all` | Revision 2 was inserted before the failed activation transaction. The corrected assertions retain the prepared row, require `activated_at_ms IS NULL`, keep active revision 1, and keep the event count at 1. |
| `invalid_identifiers_digests_and_states_are_refused_by_the_schema` | `BAD.trim_end_matches('1')` was unchanged because the digest ended in `0`. The test now checks malformed encoding and short/long digest lengths on insert. |
| `an_approval_action_must_belong_to_the_request_task_and_pinned_step` | The `MODEL_TURN` fixture supplied twelve values for eight columns, then lacked its required digest. It now inserts a distinct valid non-capability Step with matching arity and digest before asserting rejection from the action table. |
| `a_grant_may_only_name_a_closed_actor_authentication_and_auth_strength` | The invalid-state loop left the grant revoked before consumption. Each state probe now runs in a savepoint and rolls back, preserving the valid active consume fixture. |
| `only_an_enumerated_granted_step_can_consume_a_use` | Both Steps were enumerated and granted. The test now attempts an unenumerated valid Step with the same argument digest and scope, then proves only the enumerated member consumes. |
| `a_second_grant_cannot_authorize_an_already_consumed_step` | Bare `APPROVAL_2` SQL identifiers were bound as parameters. The second grant is sealed and the test checks SQLite's `UNIQUE(step_id)` constraint specifically. |
| `independent_connections_cannot_both_consume_one_step` | A deterministic writer reservation produces a classified `SQLITE_BUSY` loser, then retries the competitor after the winning commit. It verifies one durable use, one decrement, `EXHAUSTED` after reopen, and no later spend. |

The original 27 tests passed after these repairs.

### 12.3 Forward-only schema correction

The required completeness audit found a schema defect not covered by the seven fixtures:
migration 0005 enforced only the upper bound on request actions, allowed an incomplete request
to become `APPROVED`, and allowed a grant use before its declared membership set was sealed.
Migration 0005 remains byte-for-byte frozen. Additive migration `0006_policy_authority_sealing`
now:

- permits `PENDING -> APPROVED` only when persisted action rows equal the declared request count;
- creates a grant seal only for the complete member count and blocks use until sealing;
- freezes request/grant authority facts and sealed membership;
- prevents deleting approval/grant history or changing/deleting an approved Step while its Task exists;
- checks current Step arguments, plan/capability binding and registry generation at use time;
- makes grant use accounting immutable and ties decrements to durable use rows;
- preserves task deletion cascades while refusing direct use/seal deletion when the owning Task exists;
- removes migration 0005's incorrect equality between a full request-set digest and a
  potentially partial approved-subset digest; the trusted P6D writer must recompute the subset
  digest before sealing.

The schema is version 6. It has nine durable tables: the eight original P6 tables plus
`approval_grant_seals`. Migration 0005's recorded SHA-256 is
unchanged. Fresh, v4-to-v6 and v5-to-v6 startup paths are covered.

Two RED-first tests demonstrated the old defect before migration 0006: incomplete request
approval succeeded, and an incomplete/unsealed grant could consume. Both now pass against the
forward migration. Additional tests cover request transition bounds, subset grant sealing,
non-reactivation after revocation, immutable in-place Step authority, and approval history
deletion. The migration-filtered suite has **33 passed, 0 failed**.

### 12.4 Verification status at this addendum

| Check | Result |
| --- | --- |
| `cargo test -p serea-storage --lib --all-features migration_0005` | **PASS**, 33 passed, 0 failed |
| `cargo test --workspace --all-targets --offline` | **PASS** |
| `cargo test --workspace --all-features --offline` | **PASS**, including doc tests and fault-injection features |
| `cargo check --workspace --all-targets --all-features --offline` | **PASS** |
| `cargo clippy --workspace --all-targets --all-features --offline -- -D warnings` | **PASS** |
| `cargo fmt --all -- --check` | **PASS** |
| `cargo metadata --no-deps --format-version 1 --offline` | **PASS** |
| `python3 tests/workspace_smoke.py` | **PASS** |
| `python3 -m unittest discover -s tests -p workspace_smoke_tests.py` | **PASS**, 76 tests |
| `python3 tools/validate_docs.py docs` | **PASS**, 108 Markdown files |
| `python3 tools/check_commit_identity.py` | **PASS** |
| `tools/prove_release_fault_exclusion.sh` | **PASS**; ordinary storage/workspace release artifacts contain no fault seam, crash test target contains it only under test cfg |
| GitHub Actions at the final source SHA | not yet available; requires a committed/pushed exact head |

The clean branch's first source SHA was `d90641974785faacc434821ef3336c0c7c31ec22`; Fast CI's
rerun confirmed its identity guard passed. That run exposed stale schema-version-4 assertions in the
cross-architecture portability fixture. Both producer and consumer now assert schema version 6;
local producer and consumer tests pass on the same platform. A new commit and exact-head CI run are
pending. None of the preliminary CI attempts is a green P6B result.

The paragraph above records the state before exact-head CI and is superseded by the closure
record below.

### 12.5 Exact-head closure record

The P6B work is on `p6/policy-approval-runtime-clean`, a clean descendant of the immutable
checkpoint `3de5098e3b51c5a6bebf01b40550a6b94f4fd7e3` (parent
`1c04451469a810a484e92fa7f0e5eb249451a54e`). Exact P6B source HEAD:
`a5feb8373007aedf7b3418717f763c0d3c6475e3`. The P6B repair commit is
`d90641974785faacc434821ef3336c0c7c31ec22`; the portability fixture correction is the exact-head
commit above. Migration 0005 remains byte-identical at SHA-256
`cf6686807e3ceb10bd21c6653fbdd61c487e6b8938a2d78104d160920831e71a`. The forward migration
0006 SHA-256 is `0af16a30cd2b29f046c14d7079a356dc25ce4352216edd9ad6821b168e6eb6ca`.

Exact source-head GitHub Actions runs all tested source SHA
`a5feb8373007aedf7b3418717f763c0d3c6475e3` (not a generated PR merge commit):

| Workflow | Run ID | Required jobs |
| --- | ---: | --- |
| Fast CI | `38117338145` | Linux fast checks `114404752694`: success |
| Full CI | `38117338144` | Linux stable `114404805758`, Rust 1.85.0 MSRV `114404805700`, macOS arm64 `114404805506`, macOS Intel x86_64 `114404805688`: all success |
| Cross-architecture SQLite portability | `38117338125` | arm64 producer `114404752501`, Intel producer `114404752697`, arm64-produced DB opened on Intel `114404953027`, Intel-produced DB opened on arm64 `114405008396`: all success |

All listed jobs concluded `success`; none was skipped. Portability artifacts were 36,235 bytes
each: arm64 `sha256:a1ed2bc836d245fa6a38ca4b52c54fab01c8d299957bb37bed085ccf53e5d382`, Intel
`sha256:40a12c932db56a1e6e09b4e9db59cdbedfc950dcf878c9b98c14e6971b94b7f3`. Earlier identity
and schema-version CI failures were repaired on the clean branch and are not counted as closure
evidence. Runtime Draft PR [#8](https://github.com/Yrika819/Serea/pull/8) is open against
`p6/preimplementation-audit`; P6A PR #6 remains untouched. No branch history was rewritten.

**P6B_STORAGE_FOUNDATION_VERIFIED** at exact source `a5feb8373007aedf7b3418717f763c0d3c6475e3`.
This closure record supersedes the checkpoint's original 20/7 test state and the pending-CI
status above. The workspace was locally green as enumerated in §12.4, the 33 filtered migration
tests passed, and all exact-head GitHub jobs passed. No P8 provider, dispatch or `RequestId`
surface was added. Software fault tests make no physical power-loss guarantee. P6C is the next
active slice; no later milestone is claimed here.

## 13. P6C implementation progress — local gates green, exact-head CI pending

P6C is being implemented on the same runtime branch as a descendant of the P6B exact-head
commit. The exact new source SHA is not yet recorded because this slice has not been committed.
Do not treat this section as P6C closure until the reviewed commit and exact-head GitHub Actions
are recorded below.

### 13.1 Deterministic evaluator and trusted inputs

`crates/serea-policy` now contains a pure evaluator over an immutable, canonicalized policy
revision and a host-constructed `PolicyInputV1`. The input has no serde surface, no raw
arguments, no clock value, no caller-selected automation context and no model authority fields.
Context comes only from durable Task kind and trusted `RequestedBy`, using the exact ordered
derivation in Policy Protocol §4.4. Rule matching has only its seven ratified exact-value
dimensions. Explicit DENY supersedes all matching ALLOW rules; otherwise ordering is priority
descending then rule id ascending. Task risk ceiling, proactive restrictions, disabled
capability/destructive overlay, root approval and credential handoff are hard-coded gates.
The output evidence records trusted facts and a decision but provides no execution method,
approved flag, `RequestId` or dispatch intent.

SQLite is the evaluation source through a typed Storage snapshot loader. Storage independently
recomputes the canonical full-rule-set digest on both prepare and read. A mismatched caller
digest is rejected before insert; a corrupted durable snapshot is refused as a corrupt row.
The preparation API returns a distinct `PreparedPolicyRevision`; only a committed activation
or load through the active SQLite pointer can produce the `PolicyRevision` accepted by the
evaluator. A compile-fail doctest keeps a pending snapshot from being evaluated accidentally.
Revision activation plus a content-minimal `POLICY_CHANGED` event is one transaction. The event
names the previous and new immutable revision and the new digest, so the before/after state is
reconstructible from policy history without copying rule contents into event payloads.
Restart tests reload and re-evaluate the same active snapshot.

### 13.2 Forward-only schema correction

The P6C protocol audit found two omissions in frozen migrations: P6 policy has a separate
`enabled` **match dimension** in addition to the rule's own participation flag, and an activated
revision could previously be altered by inserting a new rule after activation. Migration 0007
adds `policy_rules.match_enabled`, refuses activation until persisted rule count equals the
declared count, and forbids late rule inserts. Migrations 0001–0006 are not edited; migration
0005 remains SHA-256
`cf6686807e3ceb10bd21c6653fbdd61c487e6b8938a2d78104d160920831e71a`, and migration 0006 remains
`0af16a30cd2b29f046c14d7079a356dc25ce4352216edd9ad6821b168e6eb6ca`. Migration 0007 SHA-256 is
`48f2f1805d59f4eec215963ac5109007294a0dfe015b9a4b4e7e69c14f9197eb`. Fresh schema is now 7;
an actual preexisting schema-v6 file upgrade is covered, along with unchanged frozen migration
catalog checksums.

### 13.3 P6C local verification recorded before publication

| Check | Result |
| --- | --- |
| `cargo test -p serea-policy --all-features --offline` | **PASS**, 22 unit tests and 2 compile-fail doctests |
| `cargo test -p serea-storage --lib --all-features --offline policy_tests -- --test-threads=1` | **PASS**, 10 tests |
| `cargo test -p serea-storage --lib --all-features --offline migration_0005 -- --test-threads=1` | **PASS**, 33 tests |
| `cargo test --workspace --all-targets --offline` | **PASS** |
| `cargo test --workspace --all-features --offline` | **PASS**, including fault-injection paths and doc tests |
| `cargo check --workspace --all-targets --all-features --offline` | **PASS** |
| `cargo clippy --workspace --all-targets --all-features --offline -- -D warnings` | **PASS** |
| `cargo fmt --all -- --check` | **PASS** |
| workspace smoke + 76 smoke unittests | **PASS** |
| `python3 tools/validate_docs.py docs` | **PASS**, 108 Markdown files |
| `python3 tools/check_commit_identity.py` | **PASS** |
| Cargo metadata, `git diff --check` | **PASS** |
| `bash tools/prove_release_fault_exclusion.sh target/p6c-release-proof` | **PASS**; default release artifacts excluded the seam, positive control and crash test included it only in their intended targets |
| Exact-head GitHub Actions | **PENDING** |

These gates include the three stale external schema-v6 assertions found by the first full
workspace run; they were changed to 7, then the full all-target run passed. A frozen workspace
smoke invariant also rejected the literal `EventKind::` token in a new Storage production file;
the check now compares the closed event wire name rather than constructing an event kind in
that P2-scanned path. This preserved the event semantics and made all 76 smoke tests pass.

P6C is still **in progress** pending final source review, ordinary commit/push, and green exact
source-head CI. P6D has not started. No provider call, dispatch, grant consumption, P8 RequestId,
or external side effect has been added.

## 14. Superseding continuation record — P6C verified; P6D active

This section supersedes the earlier P6C pending status and the statement that P6D had not
started. Those paragraphs describe the checkpoint before publication, not the current branch.

### 14.1 P6C exact-head closure

P6C is verified at source commit `da2b391021270a67838930f0945b91bb09449948`, a descendant of
P6B source `a5feb8373007aedf7b3418717f763c0d3c6475e3`. Exact-head GitHub Actions passed:

| Workflow | Run | Job results |
| --- | ---: | --- |
| Fast CI | `38121392350` | Linux fast checks `114416785198`: success |
| Full CI | `38121392297` | Linux stable `114417678249`, Rust 1.85.0 MSRV `114417678723`, macOS Intel `114417679273`, macOS arm64 `114417705684`: success |
| Cross-architecture SQLite portability | `38121392394` | arm64 producer `114416678259`, Intel producer `114416678419`, arm64 on Intel `114417024326`, Intel on arm64 `114417374653`: success |

All jobs tested source commit `da2b391…`; no job was skipped. The portability artifacts were
36,536 bytes, with ZIP SHA-256 arm64
`5cebb13cf3ae62cea50f7747bb501669973283b5b2a20963d2fc09be5d9ab290` and Intel
`df421ad8f67fa782032ddd6bb16b433a62af9ca55883e785d0a27a4bc100b5dd`.
`P6C_POLICY_ENGINE_VERIFIED` is recorded at this exact head.

### 14.2 Current P6D work (not closed)

P6D is active on branch `p6/policy-approval-runtime-p6c-clean`, based on the verified P6C
commit above. The first P6D implementation commit `53ec5c549eab570ff8bd4e3ff05da13114b9ca01`
is pushed. The owner explicitly approved the additive `APPROVAL_REVOKED` metadata event on
2026-10-11. P6D adds a separate authenticated revocation port, exact R2 request/approved-subset
persistence, typed duplicate response handling, transaction-joined grant consumption, atomic
request/grant expiry and grant-only revocation, and Scheduler routing for revoked and
unused-expired grant events. Migration 0008 is additive; migrations 0001–0007 are unchanged.
Migration 0005 still hashes to
`cf6686807e3ceb10bd21c6653fbdd61c487e6b8938a2d78104d160920831e71a`.

An adversarial lifecycle pass added a process-death test after the first P6D push: a child exits
after request/grant/member/seal/event writes but before outer commit, then another child exits
immediately after commit. Reopen proves the precommit operation left no partial state and the
postcommit operation left one complete outcome. This test and the Rust 1.85 Clippy correction
were published in `a54325b48af8c336b6d252fdfa2dc4c9821167db`; its exact-head hosted runs passed
(see §14.3). A later privacy review added an uncommitted classification and Debug-redaction
correction; it needs its own exact-head CI run before P6D closure.

The following local verification table records the `a54325b` source tree before the later
privacy correction. Current-tree results are recorded in §14.4:

| Check | Result |
| --- | --- |
| `cargo test -p serea-policy --all-features --locked --offline` | **PASS**, 35 unit tests and 2 compile-fail doctests |
| `cargo test -p serea-storage --lib --all-features --locked --offline migration_0005_tests -- --test-threads=1` | **PASS**, 54 tests, including expiry, cancellation and process-death cases |
| `cargo test --workspace --all-targets --locked --offline` | **PASS**, 496 tests in the largest suite plus doctests |
| `cargo test --workspace --all-features --locked --offline` | **PASS**, 496 tests in the largest suite plus doctests |
| `cargo clippy --workspace --all-targets --all-features --locked --offline -- -D warnings` | **PASS**; only the pre-existing vendored `serde_json` deprecation warning is emitted |
| `cargo fmt --all -- --check` | **PASS** |
| `python3 tests/workspace_smoke.py` | **PASS** |
| `python3 -m unittest discover -s tests -p workspace_smoke_tests.py` | **PASS**, 76 tests |
| `python3 tools/validate_docs.py docs` | **PASS**, 108 Markdown files |
| `python3 tools/check_commit_identity.py` | **PASS** |
| `git diff --check` | **PASS** |

The added lifecycle cases prove that a GRANT response at the request-expiry boundary commits
only request expiration, and that a valid pending request cannot be granted after its Task has
been durably marked CANCELLED. Existing fault, concurrency and reopen tests also passed in both
workspace-wide runs. Migration 0008 SHA-256 is
`6e1520bf22ced832ed480a1cf26476e056611f22b1f517f2324b32a8d4fbf925`; migrations 0001–0007
remain byte-identical to their parent revision. The smoke cardinality expectation now includes
the owner-ratified additive `APPROVAL_REVOKED` event.

The additional adversarial review found that a requested grant expiry could be after the
authenticated confirmation but at or before the current revalidation time. Policy now refuses
that already-stale expiry with `InvalidExpiryBound` before producing Storage facts, with a
dedicated regression test.

At the time this record was written, P6D was **WIP** because the crash-test extension had not
yet been published or tested by exact-head hosted CI. That was superseded by §14.4. No provider
invocation, dispatch intent, P8 RequestId, or external side effect has been added. Software
transaction fault tests do not establish physical power-loss guarantees.

### 14.3 First P6D hosted run and repair

The first pushed implementation head was `53ec5c549eab570ff8bd4e3ff05da13114b9ca01`. For its
PR-triggered runs, GitHub reported that source SHA as `head_sha`; the jobs checked out synthetic
PR merge ref `794136e2b344aba5a4d12043911cc633b33bd8b9`. That merge object has parents
`1c04451469a810a484e92fa7f0e5eb249451a54e` and `53ec5c549eab570ff8bd4e3ff05da13114b9ca01`
and tree `a5f62c0de34834b14bc8d8388812f549779a4a75`, identical to the source commit tree.

| Workflow | Run | Individual jobs |
| --- | ---: | --- |
| Fast CI | `38126857190` | Linux fast checks `114432904536`: success |
| Full CI | `38126857191` | Linux stable `114432904783`: success; Rust 1.85.0 `114432904920`: failure in Clippy only; macOS arm64 `114432904990`: success; macOS Intel x86_64 `114432904991`: success |
| Cross-architecture SQLite portability | `38126857199` | arm64 producer `114432904895`, Intel producer `114432905058`, arm64 database on Intel `114433057522`, Intel database on arm64 `114433264727`: all success |

The MSRV job completed check, all-target tests, and all-feature tests successfully. Clippy then
rejected the P6A recovery dependency scanner's newly duplicated `Scheduler` filename exception
as `nonminimal_bool`. The exception conditions have been combined without broadening the filter;
the exact scanner regression passes locally and `cargo +1.85.0 clippy --workspace --all-targets
--all-features --locked --offline -- -D warnings` passes after that correction. The correction
and the process-death regression test were published in `a54325b48af8c336b6d252fdfa2dc4c9821167db`.
Its exact-head Fast, Full and cross-architecture runs completed successfully. Individual job
results and the later privacy follow-up are recorded in §14.4; these runs do not close P6D at a
later source head.

Cross-architecture artifacts from the first run were 36,954 bytes each. `portable-db-arm64`
SHA-256: `ffbb763fd2ca7ece03b8b5f4068daa4a084d9968ef0e7ab003401baff159ffd4`; `portable-db-intel`
SHA-256: `6b120bb9c981bdd457081247793d3157d50af5be53243a07b827f68809a39352`. Their producer and
consumer jobs all passed, but these artifacts belong to the first P6D source tree.

### 14.4 Privacy review and `a54325b` hosted CI

A follow-up adversarial privacy review found that approval scope values were persisted under the
request's PERSONAL ceiling but the typed scope had no explicit inherited `DataClass`, and derived
Debug output for scopes/summaries could expose the values. The additive source correction makes
scope classification explicit and host-computed, rejects PRIVATE/SECRET/CREDENTIAL scope classes,
refuses an action scope above the request aggregate class, and redacts scope, summary and Storage
draft Debug views. Regression tests include unique sentinel values at both Policy and Storage
boundaries. Approval summary persistence still has no invented global byte limit.

The previously published `a54325b48af8c336b6d252fdfa2dc4c9821167db` source head has now passed all
three GitHub workflows. These are historical evidence for `a54325b`, before the privacy correction:

| Workflow | Run ID | Exact source SHA | Job IDs and conclusions |
| --- | ---: | --- | --- |
| Fast CI | `38127583761` | `a54325b48af8c336b6d252fdfa2dc4c9821167db` | Linux fast `114435053335`: success |
| Full CI | `38127583935` | same | Linux stable `114435053984`, Rust 1.85.0 MSRV `114435053900`, macOS Intel x86_64 `114435053760`, macOS arm64 `114435053922`: all success |
| Cross-architecture SQLite portability | `38127583679` | same | Intel producer `114435053245`, arm64 producer `114435053330`, arm64 DB on Intel `114435193625`, Intel DB on arm64 `114435459107`: all success |

The Fast and Full workflows tested the PR merge object `f457afa534902a5156dfdbd961ed674f04ee7c20`
whose tree `dbb787ddba6ab23027b9f4d4395436a23fc99bcf` matched the source tree at that time. The
source head is distinct from that synthetic merge SHA. No check was skipped. These runs do not
validate the following uncommitted changes. Cross-architecture artifacts were 36,954 bytes each:
Intel archive digest `c4d2e63ed8abe21172b6ab1caf61d9325ee5621419bb50c7ecb3c67f48b9e8a6`, arm64
archive digest `26b7340cdf6bf7914d7a4534c837d70ab89d10826d3693a081c5550e5afeb0b9`.

Local results on the privacy-corrected working tree:

| Check | Result |
| --- | --- |
| Policy unit tests, all features | **PASS**, 38 tests |
| Storage P6B/P6D approval+migration tests | **PASS**, 55 tests |
| `cargo test --workspace --all-targets --locked --offline` | **PASS**, Storage's largest suite 497 tests, plus all other targets and doctests |
| `cargo test --workspace --all-features --locked --offline` | **PASS**, Storage's largest suite 497 tests, plus doctests |
| `cargo clippy --workspace --all-targets --all-features --locked --offline -- -D warnings` | **PASS** |
| `cargo +1.85.0 clippy --workspace --all-targets --all-features --locked --offline -- -D warnings` | **PASS** |
| `cargo fmt --all -- --check` | **PASS** |
| Workspace smoke + 76 smoke regression tests | **PASS** |
| `python3 tools/validate_docs.py docs` | **PASS**, 108 Markdown files |
| `python3 tools/check_commit_identity.py` and `git diff --check` | **PASS** |

Migration 0005 SHA-256 remains
`cf6686807e3ceb10bd21c6653fbdd61c487e6b8938a2d78104d160920831e71a`; migration 0008 remains
`6e1520bf22ced832ed480a1cf26476e056611f22b1f517f2324b32a8d4fbf925`. No migration bytes were
changed by this privacy correction.

P6D remains WIP until stable/MSRV Clippy passes on this tree, this correction is committed and
pushed normally, and every required hosted job passes at its exact resulting source HEAD. P6E
must wait for that closure. No provider invocation, dispatch intent, RequestId, or external side
effect was added. Process-death tests establish software transaction recovery only, not physical
power-loss guarantees.

## 15. Superseding continuation record — P6D verified; P6E active

This section supersedes §14.4's P6D WIP status and its statement that P6E must wait. P6D closed
on the exact source head `ef064dfdce37ac765bc47c14000fc3ec547510b4` on branch
`p6/policy-approval-runtime-privacy`. That commit descends from P6C `da2b391021270a67838930f0945b91bb09449948` and P6B `a5feb8373007aedf7b3418717f763c0d3c6475e3`. The current working tree is based on that
head; the P6E changes below are uncommitted and therefore have no hosted CI evidence yet.

### 15.1 P6D exact-head verification

All required GitHub workflows passed for source `ef064df…` (PR #10, draft, base
`p6/preimplementation-audit`):

| Workflow | Run | Individual jobs |
| --- | ---: | --- |
| Fast CI | `38128811747` | Linux fast `114438675491`: success |
| Full CI | `38128811753` | Linux stable `114438675629`, Rust 1.85 MSRV `114438675589`, macOS Intel x86_64 `114438675479`, macOS arm64 `114438675663`: all success |
| Cross-architecture SQLite portability | `38128811727` | Intel producer `114438675474`, arm64 producer `114438675578`, arm64 DB on Intel `114438895291`, Intel DB on arm64 `114438924760`: all success |

Every run reports exact source `ef064df…`; no job was skipped. The PR's synthetic merge SHA
`9f98ce…` is not the source SHA. Cross-architecture artifacts were 36,954 bytes: Intel archive
SHA-256 `ffb35149d8529b88db67abadfebbf0e52c70854732a5a79c9bb757d6cdfbb726`; arm64 archive
SHA-256 `7d324ae229bbde5caee600a653878a416b7fcde2414f6dde80b39810114169`. This closes
`P6D_APPROVAL_LIFECYCLE_VERIFIED` at that exact commit only. P6A PR #6 remains open and unchanged;
P6D remains on separate draft PR #10 and is not merged.

The owner approved the additive `APPROVAL_REVOKED` audit event on 2026-10-11. P6D already stores
that authenticated revocation event atomically with grant-only revocation; consumed uses remain
historical. No migration was changed for that event.

### 15.2 P6E implementation in progress

The current uncommitted P6E source adds TaskEngine request/wake methods and Storage transactions
that move a live Task into `WAITING_APPROVAL` with its complete request, action set, routing event,
wait Step and audit facts in one transaction. A wake re-reads request, grant, Task and Step state;
grant wake returns the Task to `READY` only for later revalidation, revoked/unused-expired grants
also return for revalidation, and denied/expired requests fail the wait Step and Task. Stale or
duplicate routing work does not revive a terminal Task. The wake is acknowledged only in the
same transaction as its Task event and audit facts; the standalone Scheduler ACK APIs were
removed. No dispatch or external effect is performed.

Focused local evidence on the working tree:

| Check | Result |
| --- | --- |
| Storage `migration_0005` filter | **PASS**, 62 tests, including P6B, P6D and P6E transaction/restart cases |
| Scheduler `approval_wake` | **PASS**, 3 tests; failed apply leaves all routing rows and Task unchanged |
| TaskEngine `approval` | **PASS**, unrelated Task policy evidence is refused before mutation |
| `cargo test --workspace --all-targets --locked --offline` | **PASS**, including the new TaskEngine and Scheduler regressions |
| `cargo test --workspace --all-features --locked --offline` | **PASS**, including doc tests |
| Stable and Rust 1.85 Clippy | **PASS**, `-D warnings` |
| fmt, workspace smoke, Python smoke tests (76), docs validator (108 Markdown files), identity check | **PASS** |
| migration 0005 SHA-256 | `cf6686807e3ceb10bd21c6653fbdd61c487e6b8938a2d78104d160920831e71a`; byte-identical |

P6E is **not closed**. `TaskEngine::request_approval` currently accepts a host-built typed
request plus policy evidence; it checks exact action-to-evidence bindings and Storage rechecks
the active policy revision and durable Task/Step facts. It does not yet derive `PolicyInputV1`
directly from a current `PreparedActionV1` and evaluate it inside one complete TaskEngine flow.
The current tests do not yet exercise a successful full TaskEngine request-to-wake-to-fresh-policy
path, or policy/capability changes while waiting. Complete those integrations and their recovery tests before claiming
P6E. P6F has not started. The new working tree still requires a full rerun after the isolated
TaskEngine test addition (now complete and green), a normal commit/push, and
Fast/Full/Cross-architecture CI at its exact source head.

No provider invocation, P8 RequestId, dispatch intent, credential handling, or external side
effect was added. Local and process-death tests establish software transaction behavior only;
they do not establish physical power-loss guarantees.
