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
| Migrations 0001–0004 | byte-identical to the P6A parent |
| ADR-0037–0040 | all `Accepted`; architecture `serea-arch/2.7.0` |

The Cloud task initially opened at `e17fbfc` on branch `work`; that branch was left intact.
The runtime branch was fetched from GitHub and checked out directly at the supplied checkpoint.
The verification and fixes below are based on that checkpoint. The first publication used a local
Git email rejected by the repository's noreply-only identity guard; exact-head Fast and Full CI
reported that failure. Published history was preserved. The same reviewed tree is being republished
from the original checkpoint on `p6/policy-approval-runtime-clean` with an approved Codex noreply
identity, so the replacement branch does not inherit the rejected commit.

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

P6B remains **WIP** pending exact-head GitHub jobs and final published-revision review. All
required local gates and the release fault-exclusion proof passed after the final SQL/test
review. No P6C code has been started. No P8 provider,
dispatch or `RequestId` surface was added. This record makes no physical power-loss guarantee.
