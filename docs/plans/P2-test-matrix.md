# P2 Test Matrix

- **Branch:** `p2/design-preparation`
- **Base commit:** `c3737039e3e38dbba554dc0b9075025f87948358`
- **Status:** plan only. No test is implemented by this run.
- **Method:** Red/Green. Every group below states the RED observation that must be
  seen **before** any implementation change, so that a manufactured RED — a syntax
  error, a missing module — cannot pass for evidence.

## 1. Ground rules

Inherited from P1's closure and re-applied:

1. **No test reads a wall clock.** `.clippy.toml` bans `SystemTime::now` and
   `Instant::now` workspace-wide, so this is a compile error, not a review comment.
   Time comes from `TestClock` through the `Clock` trait.
2. **No network, no filesystem outside the test's own temporary directory, no
   subprocess** except the deliberate child-process crash harness.
3. **Every fixture is synthetic.** `.test` domains, as P1 established.
4. **No RED is manufactured by a syntax error.** Where a type or module must exist
   for a test to compile, the test is written against the *intended* signature and
   the failure is an assertion about behaviour.
5. **A fix without a test that fails on the pre-fix code is not a fix.** P1's
   Pass C found nine unpinned fixes by mutating each one out; this matrix assumes
   that discipline and is written to be mutation-checked.

## 2. Groups, and what each one pins

| Group | Subphase | Pins |
| --- | --- | --- |
| A. Text validation categories | P2A | ADR-0023 |
| B. Step presence and step-kind matrices | P2A | ADR-0018 §3, §4 |
| C. Canonical JSON and digest vectors | P2B | ADR-0019 §1 |
| D. Idempotency preimage vectors and the collision | P2B | ADR-0019 §2–§3 |
| E. Clock and time | P2B | §8 of the design |
| F. Migrations and connection policy | P2C | §7 of the design |
| G. Blobs and classification | P2D | ADR-0022, schema §5 |
| H. Leases and fencing | P2E | ADR-0024 |
| I. Task lifecycle and transitions | P2F | §10 of the design |
| J. Plan, sequence and parent binding | P2F | §10.5 |
| K. Receipts and `T4` ordering | P2F | §4 of the design |
| L. Cancellation and deletion | P2F | §10.6 |
| M. Recovery | P2G | §9 of the design |
| N. Crash and fault injection | P2H | §3 below |
| O. Non-negotiables | all | §6 below |

## 3. Crash and fault injection

**The rule: a crash is simulated by killing a child process, never by returning an
`Err` before commit.** An in-process `Err` exercises the rollback path, which is a
different property from durability, and treating it as a crash test is how a
storage layer passes a test suite and loses receipts in the field.

The harness re-invokes the test binary through `std::env::current_exe()` with an
environment variable naming the window. The child reaches the window, calls
`std::process::abort()`, and the parent then reopens the database in a **fresh
process** and asserts against durable expectations.

| # | Window | Child action | Parent assertion after reopen |
| --- | --- | --- | --- |
| N1 | Before `BEGIN` | abort | Nothing exists. `schema_version` is still 1, `tasks` is empty |
| N2 | After `BEGIN IMMEDIATE`, before any write | abort | No row; `PRAGMA quick_check` is `ok` |
| N3 | After `INSERT tasks`, before the journal insert | abort | **No task row.** The whole transaction rolled back |
| N4 | After the fenced step `UPDATE`, before the receipt insert | abort | Step is not `SUCCEEDED`, no receipt row. `T4` holds in the *conservative* direction: nothing advanced |
| N5 | After every write, before `COMMIT` | abort | Nothing is durable. Every row the transaction wrote is absent |
| N6 | After `COMMIT`, before the caller observes `Ok` | abort, without printing success | **The row is present**, and a recovery pass reports `ReceiptAlreadyCommitted` rather than re-effecting. This is the window that separates "committed" from "believed committed" |
| N7 | **`COMMIT` in flight** — reclassified by the P2 autonomous audit | `SIGKILL` from a sibling thread while `execute_batch("COMMIT")` runs | **A stress test, not a pin.** Assert only `PRAGMA quick_check` is `ok`, the database is openable, and `foreign_key_check` is empty. Never assert an exact row count: whether the WAL frame reached disk depends on timing |
| N8 | A fault injected between the fenced write and `rows_affected` inspection | return `Err` **after** the write, before the check | The transaction is rolled back by the drop of `Tx`; no receipt row exists. This is the one window an `Err` *does* model, and it is labelled as such rather than called a crash |

Two extra assertions that make N6 meaningful:

- **N6a.** The parent asserts the committed row exists **and** that the child's
  exit code indicates it never printed success. If the child could report success
  and the row were absent, the test fails; if it could report failure and the row
  were present, recovery must handle it, which N6's second half covers.
- **N6b.** Recovery after N6 must produce **zero** new external-effect
  representations: no second receipt row, no second `task_journal` entry for the
  same transition. `UNIQUE (step_id)` on `side_effect_receipts` and the
  `journal_seq` precondition together enforce this.

The fault hook is a `TxHook` the production build never populates. A test asserts
the hook list is empty in a release-configuration build, so an inert hook cannot
become a hidden code path.

### 3.1 N7 reclassified: a true mid-`COMMIT` abort is not injectable here

The P2 autonomous audit established, by working through every injection point the
planned stack offers, that **a deterministic abort in the middle of `COMMIT` cannot
be produced through `rusqlite` as planned.** The techniques, and what each reaches:

| Technique | Reaches | Classification |
| --- | --- | --- |
| Return `Err` before `execute_batch("COMMIT")` | nothing inside SQLite — identical to an ordinary rollback | **must-test directly**; this is N5 |
| Return `Err` **after** `COMMIT` returned | the caller never learns the outcome | **must-test directly**; this is N6, and the most valuable window in the table |
| `TxHook` between the engine writes and `COMMIT` | inside the transaction, pre-commit | **must-test directly**; this is N8 |
| `SIGKILL` while `COMMIT` is in flight | timing-dependent; not deterministic | **stress test only** — verified to work in practice (0 rows / 1 row, `quick_check` ok) but it cannot be pinned |
| `SQLITE_TESTCTRL`, a fault-injecting VFS, or a SQLite fault build | inside SQLite | **deferred specialized storage test** — needs a custom build or VFS, unavailable through `rusqlite` as shipped |
| A second writer forcing `SQLITE_BUSY` mid-commit | nothing; SQLite serialises writers and the second waits out `busy_timeout` | **not a technique** |

So N7 as originally written — "abort mid-`COMMIT`" asserted to leave "either the
whole transaction or none of it" — is a test that would only ever pass by accident,
and labelling it a pin would be the exact overclaim §1 rule 5 forbids. It is
reclassified as a stress test with weak assertions. The deterministic coverage the
matrix actually wants is N5 (pre-commit) plus N6 (post-commit), which between them
cover both sides of the only boundary Serea can actually control.

**No test may simulate a crash by returning an `Err` before commit and calling it
a crash.** That rule is unchanged; N7's reclassification is an application of it.

## 4. Group A — text validation categories

The shared adversarial corpus, asserted against **every** free-text field, from
both the Rust validator and the schema, with the **same verdict** required from
both:

| Input | A note on it |
| --- | --- |
| `""` | empty |
| `" "` | single space |
| `"   "` | **the P1 divergence.** Whitespace-only: the schema accepts it today, Rust refuses it |
| `"\t"`, `"\n"`, `"\r\n"` | control whitespace |
| `" x"`, `"x "` | leading and trailing whitespace |
| `"a\u{1}b"` | C0 control |
| `"a\u{7f}b"` | DEL |
| `"line1\nline2"` | legal in category P, illegal in L and O |
| `"line1\rline2"` | illegal everywhere, per the CR rule |
| `"stp_01JQ8Z9M3R2CVN8H5FWK7PQDSF"` | an identifier, in an O field |
| `"tsk_01JQ8Z9K3M7QWXR4V2T6YH0BNA"` | a different identifier, in an O field |
| `"explanation"` | ordinary prose, legal in all three |
| `"café — 日本語"` | non-ASCII, legal in all three |
| a 5000-character string | **no length ceiling is imposed in Rust.** Asserted *accepted* everywhere, so the P1 retraction cannot regress |

| # | Test | Pins |
| --- | --- | --- |
| A1 | `whitespace_only_free_text_is_refused_on_both_surfaces` | The P1 divergence, closed |
| A2 | `every_category_accepts_and_refuses_the_same_corpus_identically` | Rust and schema agreement across all 9 fields |
| A3 | `category_l_refuses_newline_because_a_plain_summary_is_a_consent_surface` | `PlainSummary` specifically |
| A4 | `category_p_permits_newline_because_a_provider_diagnostic_has_one` | `ErrorMessage` specifically |
| A5 | `category_p_refuses_bare_carriage_return` | The CR/LF-normalisation rule |
| A6 | `category_o_refuses_every_prefixed_and_fixed_shape_frozen_identifier` | The impersonation rule, as **decided by measurement** (rule C): all eleven ULID prefixes, `idk_`+64 hex, `sha256:`+64 hex, a real `CapabilityId` — 16/16 |
| A6a | `category_o_accepts_every_legal_opaque_token` | 0/14 false positives. `calendar`, `worker`, `worker-1`, `host-a3f9`, `session-42.worker`, `x`, `w`, a long reference, `provider:handle/1234` — the set rule B refused 8 of |
| A6b | `category_o_near_miss_identifier_shapes_are_accepted` | `tsk_`, a 27-char ULID body, a lowercase ULID body, `sha256:zz`, `calendar.events.reticulate` (unknown verb), `fake-goallatch.goal.run` (not a `ProviderId`, so not a `CapabilityId`) |
| A6c | `category_o_refuses_goallatch_as_a_capability_but_not_as_an_opaque_token` | The parity subtlety: `goallatch.goal.run` is **not** a valid `CapabilityId`, so the banned set must exclude it. A pattern stricter than `CapabilityId` is still a parity bug |
| A6d | `generated_category_o_pattern_matches_the_frozen_prefix_and_verb_lists` | Twelve prefixes **and** fourteen verbs come from `ids.rs`. A hand-written copy drifts, and a drifted copy means the schema accepts an `ActorId` Rust refuses |
| A7 | `no_free_text_field_is_length_capped_in_rust` | The P1 retraction of `MAX_VALUE_LENGTH`, pinned |
| A8 | `no_schema_imposes_a_free_text_length_ceiling` | The same on the schema side |
| A9 | `category_o_rejects_prose_with_a_colon_and_a_newline` | The single-line rule, in the token's own shape |

**RED for A1**, observed on the pre-fix code: the schema validation of `"   "` for
`title` returns `Ok`, while `TaskTitle::new("   ")` returns
`MalformedValue { field: TaskTitle, reason: Empty }`. The test asserts one verdict
and fails with both sides printed.

**RED for A6, and this is the finding the P2 autonomous audit produced.** The
pattern ADR-0023 previously published was

    ^(?![ \t\n\r\f\v]*$)(?!.*:(tsk|stp|apr|grt|req|evt|dev|sch|prop|rcp|ses)_)[^\u0000-\u001f\u007f]+$

which requires a **literal colon immediately before** the prefix. No Serea
identifier has one. Executed under ECMA-262, it fires only on strings of the form
`a:tsk_`, `x:stp_…`, `sha256:tsk_…` — none of which can occur as an `ActorId`,
`LeaseOwner` or `ProviderReference`. **Every** real frozen identifier was accepted:
all eleven ULID prefixes, `idk_`, `sha256:`, and every `CapabilityId`. Seven
divergences across a fourteen-case corpus, including the case this very corpus
already named. **A2 and A6 pass on agreement and still miss the defect**, because
both sides were wrong in the same direction — which is why A6a, A6b and A6c exist
as separate tests with hand-picked positive and negative cases rather than one
symmetric corpus.

## 5. Group B — step presence and step-kind matrices

Every cell of ADR-0018 §3 and §4, from both directions: the Rust constructor
refuses, and the schema refuses. 7 statuses × 15 fields, plus 8 kinds × 6 fields.

| # | Test | Pins |
| --- | --- | --- |
| B1 | `a_planned_step_carries_no_started_at_no_completed_at_and_no_result_digest` | The core anti-fabrication case |
| B2 | `a_succeeded_step_cannot_be_constructed_without_a_result_digest` | `SUCCEEDED ⇒ result_digest` |
| B3 | `a_planned_step_has_attempt_zero_and_every_other_status_at_least_one` | `attempt` |
| B4 | `lease_fields_are_present_exactly_while_leased_or_executing` | The lease row of the matrix |
| B5 | `waiting_is_reachable_only_for_the_three_wait_kinds` | The `WAITING` row |
| B6 | `a_capability_step_has_a_capability_id_version_provider_and_key` | Matrix §4 |
| B7 | `a_notify_step_has_none_of_those_four` | Matrix §4 |
| B8 | `a_model_turn_step_has_no_idempotency_key` | The §5.2 resolution |
| B9 | `a_verify_step_does_have_an_idempotency_key` | `VERIFY` is capability-shaped |
| B10 | `a_delegate_step_does_have_an_idempotency_key` | `DELEGATE` is capability-shaped |
| B11 | `no_step_kind_derives_a_key_from_a_reserved_pseudo_capability` | Guards the rejected alternative in §5.2 |
| B12 | `an_unrecognised_step_status_is_refused_and_yields_unrecognised_state` | Protocol Index §4.2 rule 5 at the step boundary |
| B13 | `lease_generation_round_trips_and_is_absent_on_an_unleased_step` | The new optional member |
| B14 | `every_matrix_cell_is_refused_by_the_schema_with_the_same_verdict_as_rust` | The two surfaces agree |
| B15 | **`every_one_of_the_32_absent_cells_is_refused_by_sql`** | All 32 of ADR-0018 §3's `N`/`0` cells, one probe each. **Was 24/32.** The eight gaps — `completed_at` and `result_digest` on `EXECUTING` and `WAITING`, `lease_expires_at` on `WAITING`, `SUCCEEDED`, `FAILED` and `RECONCILED_ABSENT` — were found by the P2 autonomous audit probing *cells* where earlier rounds had probed *rows* |
| B16 | **`lease_expiry_is_present_exactly_while_leased_or_executing`** | The matching biconditional for `lease_expires_at`. Without it a terminal step could carry a dangling expiry with **no owner**, because `lease_owner` was already biconditional — the pair was half-constrained |
| B17 | **all 56 `kind × status` cells are accounted for: 51 constructible, 5 refused** | The 5 are `WAITING` on a non-wait kind, which is ADR-0018's intent. Asserting 56/56 would be asserting the opposite of the contract |
| B18 | **all 37 legal task transitions are constructible from a real seeded row** | The positive half of the transition table. Earlier rounds asserted only that illegal pairs were refused — the pattern the audit names as this package's repeated blind spot |
| B19 | **three control cells construct: a legal `EXECUTING`, `WAITING` and `SUCCEEDED`** | So B15 and B16 are not passing by refusing everything |

**RED for B1**, observed on the pre-fix code: constructing a `TaskStep` for a plan
that has not executed does not fail — it *succeeds*, because the test must supply
`started_at` and `result_digest` and there is nothing stopping it from supplying a
fabricated pair. The failing assertion is therefore a schema-validation failure:
validating the serialised step against the checked-in schema fails on the
`started_at` *type*, because the test omits it to express "unstarted". That is the
right RED: it demonstrates that the current contract cannot represent the state, not
merely that a validator is strict.

## 6. Groups C and D — canonical JSON, digests, idempotency

| # | Test | Vector |
| --- | --- | --- |
| C1–C10 | `scj1_vector_N` for each of the ten pinned vectors | [ADR-0019](../decisions/ADR-0019-canonical-json-and-idempotency-preimage.md) §SCJ-1 |
| C11 | `scj1_is_idempotent_over_a_generated_corpus` | `canonicalize(parse(canonicalize(x))) == canonicalize(x)` |
| C12 | `scj1_member_ordering_does_not_affect_the_digest` | Permutations of a 12-key object |
| C13 | `scj1_rejects_duplicate_object_keys` | `{"a":1,"a":2}` |
| C14 | `scj1_rejects_a_float` | `{"k":1.5}` |
| C15 | `scj1_rejects_an_exponent_form` | `{"k":1e2}` |
| C16 | `scj1_accepts_u64_max_and_refuses_u64_max_plus_one` | The P1 collapse, made detectable |
| C17 | `scj1_refuses_negative_zero` | `{"k":-0}` |
| C18 | `scj1_preserves_array_order` | `[1,2]` versus `[2,1]` digest differently |
| C19 | `scj1_refuses_nesting_past_the_instance_depth_bound` | 65 levels |
| C20 | `scj1_emits_no_trailing_newline_and_no_byte_order_mark` | Byte-level |
| C21 | `scj1_canonical_error_renders_no_payload_bytes` | `DC7` |
| D1–D7 | `idk1_vector_N` for each of the seven pinned vectors | ADR-0019 §IDK-1 |
| D8 | **`the_naive_collision_pair_derives_two_different_keys`** | Tuples **A** and **B**. This is the regression test for the whole ADR |
| D9 | `idk1_is_injective_over_the_frozen_grammar_corpus` | 113 400 triples: no two distinct tuples share a preimage |
| D10 | `naive_concatenation_would_collide_on_the_pinned_pair` | Asserts the collision is *real*, so the test cannot be made vacuous by changing the encoding |
| D11 | `idk1_preimage_is_domain_separated_from_a_content_digest` | `sha256(scj1(x))` and `derive_idempotency_key` over the same document must differ |
| D12 | `idk1_rejects_a_non_capability_step_tuple` | The §5.2 boundary at the function level |
| D13 | `a_naive_collision_canary_watches_the_frozen_grammars` | If a future grammar change makes the naive form collide, this fails loudly |

**RED for C1**, observed before any canonicalization exists: there is no
`canonicalize` function, so the test cannot compile. To avoid a manufactured RED,
the test is first written against a deliberately naive local helper that sorts keys
and serialises with `serde_json::to_string`, and C2 (array order) and C15 (exponent)
and C16 (`u64::MAX + 1`) each fail against it. Those three failures are the RED, and
they demonstrate that the naive helper is not SCJ-1.

**RED for D8**: against a naive implementation that concatenates the five fields,
D8 fails with tuple A and tuple B producing the *same* key, and D10 passes. D8's
failure is the collision, reproduced.

## 7. Group E — clock and time

| # | Test | Pins |
| --- | --- | --- |
| E1 | `two_test_clocks_driven_identically_report_identical_time` | Determinism |
| E2 | `epoch_millis_round_trips_through_the_wire_form_for_both_permitted_forms` | `…SSZ` and `…SS.mmmZ` |
| E3 | `lexicographic_timestamp_order_is_wrong_and_epoch_millis_is_not` | **The bug this group exists for.** Asserts `Timestamp("…T09:14:22Z") > Timestamp("…T09:14:22.100Z")` while the epoch-millisecond values order correctly, so the design's refusal to compare wire forms is pinned rather than asserted |
| E4 | `a_calendar_impossible_wire_value_is_refused` | `2026-02-30T00:00:00.000Z` |
| E5 | `a_clock_reading_outside_the_48_bit_range_is_refused_at_open` | `Store::open` validates once |
| E6 | `test_clock_epoch_millis_is_the_single_authority` | The structural change to `TestClock` |
| E7 | `no_wall_clock_call_exists_in_either_new_crate` | A source-level assertion, complementing `.clippy.toml` |

## 8. Group F — migrations and connection policy

All against a **file-backed** store, because in-memory SQLite has neither crash
durability nor reopen.

| # | Test | Pins |
| --- | --- | --- |
| F1 | `a_fresh_database_is_created_and_reaches_the_latest_version` | First migration |
| F2 | `reopening_after_migration_is_a_no_op` | Second `open` applies nothing |
| F3 | `a_newer_schema_is_refused_rather_than_downgraded` | `SchemaTooNew`; the version is left untouched |
| F4 | `a_non_serea_file_is_refused_rather_than_adopted` | `NotSereaStore`; the file's bytes are unchanged afterwards |
| F5 | `a_zero_length_file_is_treated_as_fresh` | The fresh-database edge |
| F6 | `a_migration_checksum_mismatch_refuses_the_open` | `MigrationChecksumMismatch` |
| F7 | `a_failed_migration_leaves_no_partial_schema_and_no_version_row` | The DDL and the version row share a transaction |
| F8 | `wal_is_enabled_and_asserted_at_open` | `PRAGMA journal_mode` |
| F9 | `foreign_keys_are_enabled_and_asserted_at_open` | `PRAGMA foreign_keys` returns 1 |
| F10 | `a_foreign_key_violation_is_refused` | The pragma actually works |
| F11 | `synchronous_is_full` | The durability policy |
| F12 | `json1_is_present` | `SELECT json_valid('{}')` succeeds; the open fails if not |
| F13 | `strict_tables_reject_a_wrong_column_type` | A raw `INSERT` with an integer into a `TEXT` column |
| F14 | `a_rollback_inside_transact_leaves_no_row` | The ordinary in-process rollback path |
| F15 | `integrity_check_reports_a_healthy_database` | `verify_integrity` |
| F16 | `a_corrupted_file_is_refused_rather_than_recreated` | Corrupt bytes in the header; the store does **not** delete and recreate, which would destroy receipts |
| F17 | `two_store_instances_on_one_file_both_succeed` | No exclusive lock at open |
| F18 | `a_transaction_is_serialised_against_a_second_writer` | `BEGIN IMMEDIATE` plus `busy_timeout` |
| F19 | **`open_in_memory_reports_memory_not_wal_and_asserts_it`** | `TestMemoryProfile`. `open_in_memory` **cannot** satisfy ADR-0005, so it asserts what it actually is — a test cannot "pass" here by skipping the check |
| F20 | **`synchronous_is_not_asserted_in_the_memory_profile`** | `PRAGMA synchronous = 2` on `:memory:` returns **no row**: there is nothing to fsync. Asserting `FULL` there would be asserting nothing |
| F21 | **`foreign_keys_are_asserted_in_the_memory_profile_too`** | It defaults to `0` in memory as well as on disk |
| F22 | **`foreign_key_check_is_run_after_each_migration`** | The tier the audit added. A migration that produced dangling references has failed in a way `quick_check` cannot see |
| F23 | **`quick_check_and_integrity_check_report_ok_on_an_fk_orphan`** | **The evidence F22 rests on.** Both are page-level checks; against a deliberately orphaned `task_steps` row both return `ok` and only `foreign_key_check` reports it. If this test ever fails, SQLite's semantics changed and the tier policy must be revisited |
| F24 | **`verify_integrity_runs_both_integrity_check_and_foreign_key_check`** | The admin tier's actual contract, which was previously unspecified |
| F25 | **`two_integration_test_binaries_get_distinct_temp_paths`** | `TempStore` identity. A counter alone collides across binaries — verified: two binaries each counting from 0 produce the same three names |
| F26 | **`a_crash_child_reopens_the_parent_database_by_inherited_directory`** | The other half. A pid alone is not unique *within* a process, and `cargo test` runs tests as threads |
| F27 | **`temp_paths_contain_no_wall_clock_and_no_rng`** | The `.clippy.toml` ban holds while uniqueness is still achieved: `<binary>-<pid>-<atomic-counter>` |
| F28 | **`the_migrated_object_inventory_is_10_tables_7_triggers_6_indexes`** | Asserted, not printed — so a phantom object cannot be reintroduced. **This is the direct regression for the `leases_generation_matches_step` trigger that §4.6 published and §4.0 never contained** |
| F29 | **`the_resolved_bundled_sqlite_is_at_least_3_37`** | Verified unnecessary for `rusqlite` 0.40.2 (bundles 3.53.4); retained so an older candidate cannot pass silently |

## 9. Group G — blobs and classification

| # | Test | Pins |
| --- | --- | --- |
| G1 | `a_blob_is_stored_under_the_digest_of_its_canonical_bytes` | Content addressing |
| G2 | `the_same_bytes_at_the_same_class_dedupe_to_one_row` | Deduplication |
| G3 | `the_same_bytes_at_two_classes_store_two_rows` | The composite key |
| G4 | `a_public_reference_cannot_read_a_private_blob` | **The laundering case.** A `BlobRef` at `PERSONAL` cannot resolve a blob stored `PRIVATE` |
| G5 | `a_wrong_digest_is_refused_on_write` | `DigestMismatch` |
| G6 | `a_corrupt_blob_is_detected_on_read` | Bytes edited under the digest; `BlobCorrupt` |
| G7 | `read_returns_bytes_identical_to_canonical_bytes` | Round-trip |
| G8 | `canonical_forms_that_differ_only_in_member_order_dedupe` | ADR-0019 §1 rule 3 |
| G9 | `a_rollback_after_put_blob_leaves_no_blob_row` | **Orphan prevention, structurally.** No orphan can exist from P2's own writes |
| G10 | `a_rollback_after_put_blob_leaves_no_reference_row` | Same |
| G11 | `private_without_a_backend_is_refused_and_writes_nothing` | ADR-0022's core case |
| G12 | `private_with_a_backend_stores_bytes_that_are_not_the_plaintext` | Using the test double, which is labelled as not encryption |
| G13 | `secret_is_refused_on_every_write_path` | `ClassRefused` |
| G14 | `credential_is_refused_on_every_write_path` | `ClassRefused` |
| G15 | `a_hand_written_insert_of_a_secret_row_is_refused_by_the_check` | The SQL enforcement, not just the Rust path |
| G16 | `a_hand_written_insert_of_a_private_row_without_protection_is_refused` | The second `CHECK` |
| G17 | `deleting_a_task_cascades_to_every_dependent_row_in_one_transaction` | Task Protocol §8 |
| G18 | `an_unreferenced_blob_survives_a_task_delete_because_another_task_references_it` | Shared blobs |
| G19 | `a_still_referenced_blob_cannot_be_deleted_directly` | `ON DELETE RESTRICT` |
| G20 | `size_bytes_matches_length_of_content` | The consistency `CHECK` |

**RED for G11**, observed on pre-fix code: there is no `put_blob` at all, so the
test is written first against a deliberately permissive local helper that writes
whatever class it is handed. G11 and G13 fail, and G15 fails because the table has
no `CHECK`. Those three failures are the RED.

## 10. Group H — leases and fencing

The stale-worker tests use **two separate `Store` instances on the same file**, so a
process-local mutex cannot participate and the test would pass trivially if one did.

| # | Test | Pins |
| --- | --- | --- |
| H1 | `acquire_lease_succeeds_from_planned` | The happy path |
| H2 | `a_competing_acquisition_is_refused` | `LeaseHeld`; one atomic statement |
| H3 | `an_expired_lease_is_reclaimable` | The `expires_at_ms <= now` branch |
| H4 | `a_reclaim_increments_the_generation` | The fence's monotonicity |
| H5 | `renew_extends_an_unexpired_lease` | |
| H6 | `renew_is_refused_after_expiry` | **The clause most easily forgotten.** A stalled worker must not resurrect its own fence |
| H7 | `renew_by_a_stale_generation_is_refused` | |
| H8 | `release_fences_the_guard_permanently` | |
| H9 | `a_commit_under_a_current_lease_succeeds` | |
| H10 | **`a_stale_generation_cannot_commit_after_a_reclaim`** | The property ADR-0024 exists for |
| H11 | `a_stale_generation_commit_inserts_no_receipt_row` | The ordering requirement: the fenced write is first, and the check is explicit because SQLite does not roll back on a zero-row `UPDATE` |
| H12 | `a_stale_generation_commit_inserts_no_journal_row` | Same |
| H13 | `a_stale_generation_commit_leaves_the_task_state_untouched` | Same |
| H14 | `two_acquisitions_by_the_same_owner_string_are_distinguished_by_the_generation` | The `token` column was **removed**; `generation` alone is the discriminator |
| H14b | `the_step_generation_is_derived_from_the_leases_row_not_guessed` | The acquire statement reads `lease_generation` from `leases` in the same transaction, so the two copies cannot diverge observably. The trigger that used to enforce this was removed: it fired only on the upsert's insert branch |
| H15 | `the_generation_and_the_step_column_agree_after_every_acquisition_and_commit` | The deliberate duplication's consistency trigger |
| H16 | `no_error_rendering_carries_a_lease_identity_beyond_the_step_id_and_generation` | `DC7`. Rewritten: the original asserted a property of the removed `token` column, so it **could not fail**, which in a matrix whose §1 rule 5 forbids unfailable tests is worse than no test |
| H17 | `a_lease_is_released_when_its_step_is_deleted` | The cascade |
| H18 | **`attempt_increments_exactly_once_across_acquire_and_begin_attempt`** | Assert `attempt == 1` after *both*. Double-charging makes `max_attempts_per_step = 3` buy one attempt |
| H19 | **`a_ceiling_of_zero_leaves_the_step_planned_with_no_lease_row`** | The rollback case the design's prose describes |
| H20 | **`a_refused_acquisition_reverts_to_the_prior_committed_state`** | The rollback case the prose does *not* describe: a third acquisition against a ceiling of 2 leaves `('LEASED', 2, 2)` and exactly one `leases` row |
| H21 | **`a_crash_only_loop_is_stopped_by_the_attempt_ceiling_with_zero_executions`** | An expiry reclaim spends an attempt, so `max_attempts_per_step` bounds *acquisitions*. Assert the acquisition count, not the execution count |
| H22 | **`the_fence_holds_across_two_independent_connections_on_one_file`** | The stale-generation commit returns 0 rows across two connections, so a process-local mutex cannot be what makes H10 pass |

**RED for H10**: against an implementation that fences only on
`lease_owner = ?`, H10 fails — the stale worker's commit succeeds — and H14 also
fails, because the same owner string matches. H6 fails against an implementation
whose `renew` lacks the expiry conjunct. Three independent failures.

## 11. Group I — task lifecycle and transitions

| # | Test | Pins |
| --- | --- | --- |
| I1 | `every_one_of_the_121_task_state_pairs_matches_the_frozen_table` | Task Protocol §4.2, transcribed literally and compared in both directions |
| I2 | `terminal_states_have_no_outgoing_transition` | `T8`, as the absence of an arm |
| I3 | `an_illegal_transition_is_refused_and_persists_nothing` | The illegal state is never written |
| I4 | `an_illegal_transition_fails_the_task_with_an_invariant_reason` | The frozen consequence |
| I5 | `policy_class_is_immutable_by_trigger` | A raw `UPDATE` is refused, not just a Rust error |
| I6 | `policy_class_is_set_at_creation_and_readable_after_reopen` | |
| I7 | `data_class_may_be_raised_but_never_lowered` | The monotonic trigger |
| I8 | `the_generated_class_label_always_agrees_with_the_rank` | Including a hand-written mismatched pair |
| I9 | `an_unrecognised_task_state_yields_blocked_unrecognised_state` | Protocol Index §4.2 rule 5 |
| I10 | `the_attempt_ceiling_is_read_from_durable_state_at_the_check` | Bounds §2.1 |
| I11 | `the_attempt_ceiling_is_reached_after_max_attempts_per_step` | |
| I12 | `a_fenced_commit_and_an_attempt_ceiling_are_distinguishable_errors` | Why the two statements are separate |
| I13 | `a_task_survives_a_full_store_drop_and_reopen` | `T3` |
| I14 | `a_reloaded_task_equals_the_created_task_field_for_field` | Round-trip equality |
| I15 | `a_task_stored_at_the_frozen_example_values_validates_against_the_checked_in_schema` | The row and the wire agree |

**RED for I1**: against an implementation whose `matches!` omits
`(Executing, Blocked)`, the test fails on that one pair and reports both the
expected and the actual set. Because the test compares against a literal
transcription of the document, the failure names the missing transition rather
than a boolean.

## 12. Group J — plan, sequence and parent binding

| # | Test | Pins |
| --- | --- | --- |
| J1 | `a_plan_is_persisted_before_any_execution` | Task Protocol §4.3 |
| J2 | `an_unstarted_step_is_readable_after_reopen` | The §5.1 case, end to end |
| J3 | `an_in_flight_step_is_readable_after_reopen` | |
| J4 | `a_terminal_step_is_readable_after_reopen` | |
| J5 | `(task_id, sequence)` is unique | |
| J6 | `a_plan_cannot_renumber_an_existing_sequence` | ADR-0018 §5 |
| J7 | `a_revision_may_append_at_higher_sequences` | |
| J8 | `a_revision_may_delete_a_planned_step` | |
| J9 | `a_revision_that_would_drop_an_executed_step_is_refused` | |
| J10 | `a_deleted_superseded_planned_step_remains_recoverable_from_the_revision_blob` | The auditability argument for deletion |
| J11 | `a_step_belongs_to_exactly_one_task` | The foreign key |
| J12 | `one_task_cannot_mutate_another_tasks_step_by_id` | **Security question 8.** Every task-scoped predicate carries `task_id` |
| J13 | `a_lease_guard_from_another_task_cannot_commit` | `LeaseGuard` carries `task_id` |
| J14 | `two_steps_in_one_task_cannot_share_an_idempotency_key` | The unique index |
| J15 | `two_capability_steps_in_different_tasks_may_share_a_key_value` | `NULL` distinctness is not the point here; the composite scoping is |

## 13. Group K — receipts and `T4` ordering

| # | Test | Pins |
| --- | --- | --- |
| K1 | `a_receipt_and_the_step_success_commit_in_one_transaction` | `T4` |
| K2 | `the_task_state_does_not_advance_before_the_step_commit` | `T4` |
| K3 | `a_receipt_survives_restart` | |
| K4 | `a_receipt_row_for_a_non_succeeded_step_is_detected_by_recovery` | The `C4` half P2 can check |
| K5 | `a_receipt_idempotency_key_must_equal_its_step_key` | The `side_effect_receipts_key_matches_step` **trigger**. A `CHECK` cannot do this: SQLite prohibits subqueries in `CHECK`, which made the original migration unbuildable |
| K5b | `a_receipt_may_only_be_recorded_for_a_succeeded_step` | The `side_effect_receipts_step_must_succeed` trigger |
| K5c | `a_steps_idempotency_key_cannot_be_changed_after_insert` | The `task_steps_idempotency_key_immutable` trigger, which is what makes K5 sound |
| K6 | `a_second_receipt_for_the_same_step_is_refused` | `UNIQUE (step_id)` |
| K7 | `a_receipt_without_a_provider_reference_is_accepted_and_the_local_state_rule_is_deferred_to_p5` | **A constraint was deleted, not implemented.** The substituted rule both refused a legal `LOCAL_STATE` receipt with `replay_safe: true` and accepted an illegal non-`LOCAL_STATE` one with `replay_safe: false`. P2 has no registry, so the real condition is a P5 obligation |
| K7b | `a_corrupt_swept_blob_scope_leaves_a_referenced_blob_alone` | The `NOT EXISTS` subqueries in the sweep must be correlated on `blobs.digest`; uncorrelated they are trivially true and delete every unreferenced blob in the file |
| K8 | `the_receipt_is_absent_after_a_rolled_back_commit` | Group N4 |

## 14. Group L — cancellation and deletion

| # | Test | Pins |
| --- | --- | --- |
| L1 | `cancelling_from_each_non_terminal_state_succeeds` | |
| L2 | `cancelling_stamps_cancelled_at_and_cancelled_by` | Task Protocol §7 |
| L3 | `cancelling_a_terminal_task_is_an_explicit_no_op` | `changed: false, already_terminal: true` |
| L4 | `a_terminal_cancellation_commits_no_journal_row` | The idempotent no-op writes nothing |
| L5 | `cancellation_survives_restart` | |
| L6 | `cancelling_twice_is_still_a_no_op` | Idempotency |
| L7 | `cancellation_preserves_already_succeeded_step_receipts` | `T9` |
| L8 | `cancellation_does_not_touch_step_rows` | `T9` |
| L9 | `delete_task_removes_steps_receipts_leases_revisions_refs_and_journal` | Task Protocol §8 |
| L10 | `delete_task_reports_the_counts_it_deleted` | Data Classification §8.2 step 4 |

## 15. Group M — recovery

| # | Test | Pins |
| --- | --- | --- |
| M1–M11 | One test per row of the recovery table | §9.1 of the design |
| M12 | **a_recovery_pass_changes_durable_state_the_first_time_and_nothing_the_second** | `T5` |
| M13 | `byte_identical_durable_state_after_the_second_pass` | The strongest form of M12: a full dump comparison |
| M14 | `an_expired_in_flight_lease_yields_needs_reconciliation_and_no_execution` | The blind-re-execution prohibition |
| M15 | `a_receipt_present_with_an_incomplete_transition_is_repaired_without_re_effect` | Task Protocol §6 |
| M16 | `the_repair_produces_no_second_receipt_row` | |
| M17 | `a_corrupt_row_yields_invariant_violation_not_a_silent_skip` | Protocol Index §4.2 rule 5 |
| M18 | `an_unrecognised_status_yields_blocked_unrecognised_state` | The exact frozen code string |
| M19 | `pending_event_transitions_is_greater_than_zero_after_a_task_creation` | ADR-0021's visible `E3` debt |
| M20 | `recovery_invokes_nothing` | **A source-level assertion**: no `CapabilityProvider`, `ModelProvider`, or `HostGoalProvider` symbol is reachable from the recovery module's call graph |
| M21 | `recovery_reads_the_clock_only_through_the_injected_trait` | No ambient clock |
| M22 | `recovery_on_an_empty_database_is_a_no_op` | |

**RED for M12**: the test runs two passes and compares two full dumps. Against an
implementation whose second pass re-writes `updated_at_ms`, the dumps differ and the
test reports the differing rows.

## 16. Group O — non-negotiables

These are the tests whose *absence* would let a real defect ship, so each is written
even where a more specific group seems to cover it.

| # | Test | Pins |
| --- | --- | --- |
| O1 | `no_network_symbol_is_reachable_from_either_crate` | A source-level assertion. The P1 pattern |
| O2 | `no_subprocess_is_spawned_outside_the_crash_harness` | `TB-6`: P0 has no subprocesses |
| O3 | `no_wall_clock_call_exists` | `.clippy.toml`, plus a source assertion |
| O4 | `the_testkit_is_unreachable_from_either_runtime_crate` | `tests/workspace_smoke.py`, which globs `crates/*` and so covers the new members without a list to keep in sync |
| O5 | `no_frozen_enum_set_grew` | `DataClass` 5, `RiskClass` 8, `TaskState` 11, `StepKind` 8, `ActionErrorKind` 13, `EventKind` 60 |
| O6 | `no_store_method_writes_state_outside_transact` | A source assertion over `serea-storage`'s `impl Store`. **This is the test that makes §3.2 of the design mechanical rather than aspirational** |
| O7 | `the_workspace_contains_exactly_four_members` | CI's existing exact-member-list check, extended: `serea-protocol`, `serea-storage`, `serea-task-engine`, `serea-testkit`. P2 adds **two** crates, so "three" was wrong |
| O8 | `no_dependency_outside_the_named_set_was_added` | The P1 pattern, extended for `sha2` and `rusqlite` |
| O9 | `no_event_kind_is_constructed_in_p2` | ADR-0021's grep, as a test |
| O10 | `no_serea_events_table_exists_in_migration_0001` | Same, on the schema |
| O11 | `no_production_code_path_registers_a_commit_hook_other_than_the_journal` | ADR-0021 |
| O12 | `no_error_rendering_quotes_a_blob_payload_or_a_lease_token` | `DC7`, `AB-13` |
| O13 | `every_test_is_offline_and_deterministic_under_repeated_runs` | Run the suite twice and compare output |
| O14 | `every_class_cap_and_presence_constraint_is_a_check_and_that_is_the_documented_boundary` | Pins the one limitation the package does not mitigate: `PRAGMA ignore_check_constraints` disables every `CHECK`. A test that names the boundary is worth more than one that implies it does not exist |
| O15 | `every_authority_bearing_constraint_is_a_trigger_or_a_foreign_key_not_a_check` | The counterpart to O14. These are the controls that survive the pragma — `policy_class` immutability, `data_class` monotonicity, the three receipt triggers, `idempotency_key` immutability, journal/step parent agreement — and a future author moving one into a `CHECK` would silently weaken it |

## 17. Red/Green sequencing, per subphase

| Subphase | Tests written first | Observed RED | Then |
| --- | --- | --- | --- |
| P2A | A1–A9, B1–B14 | A1 on the whitespace divergence; B1 on the unconstructible unstarted step | Implement; A and B green |
| P2B | C1–C21, D1–D13, E1–E7 | C2, C15, C16 against a naive local helper; D8 against naive concatenation | Implement; C, D, E green |
| P2C | F1–F18 | F1 against no store; F7 against DDL outside the migration transaction | Implement; F green |
| P2D | G1–G20 | G11, G13 against a permissive helper; G15 against an unchecked table | Implement; G green |
| P2E | H1–H17 | H6, H10, H14 against owner-only fencing | Implement; H green |
| P2F | I1–I15, J1–J15, K1–K8, L1–L10 | I1 on the omitted transition; J12 on the missing `task_id` predicate | Implement; I, J, K, L green |
| P2G | M1–M22 | M12 on the second-pass rewrite | Implement; M green |
| P2H | N1–N8 | N4 and N6 on a `transact` with no injection point | Implement; N green |
| P2I | O1–O15 | O6 and O10 on a `Store` with a bare `update_task_state` | Remove; all green |

**Ordering rule.** Group O is written in P2I but its *intent* is checked
continuously: if P2A adds a wall clock, `cargo clippy` fails immediately, not in
P2I.

## 18. What this matrix does not test, and must not claim

| Not tested | Why | Deferred to |
| --- | --- | --- |
| That a real at-rest backend is sound | P2 ships none | The backend's own decision (ADR-0022, open question 1) |
| That a `SECRET` sealed store exists | It does not | Open question 2 |
| `C4`'s `side_effect_class != NONE` half | No Capability Registry | P5 |
| `T6`'s risk-class comparison | No descriptors | P5 |
| `E3`, `E4` | No event bus | P3 |
| Receipt absence on a `RECONCILED_ABSENT` step | A cross-table property; no `CHECK` can span tables. The *reverse* direction is a trigger, this direction is recovery's invariant scan | — |
| `origin` / `attempt_budget` extension round-trip | Three JSON columns, pinned by I14 and F-group schema checks | — |
| Any §2 bound other than `max_attempts_per_step` | Counters belong to `serea-core` | `serea-core` |
| Payload-byte, blob-byte and object-count limits | **Still P0's open gap.** P2 imposes none and this matrix claims none | The separate bounds decision |
| Retention / `max_retained_tasks` | `delete_task` exists; the 30-day trigger is **P12's**, because the bound configuration it reads and the purge notification both land in `serea-core` then | P12 |
| `max_concurrent_steps_per_task` | The engine's one-effecting-step-at-a-time rule is a convention with no `CHECK` or trigger behind it, so it is not structural and is not claimed | `serea-core` (the bound), P5 (the router that serialises effecting calls) |
| Read-back reconciliation of an ambiguous effect | P2 has no capability | P5 |
| Approval re-render against a device roster | P2 has no device link | P6/P12 |

## 19. Verification command set

The same shape P1 used, to be run at the end of each subphase:

```text
python3 tests/workspace_smoke.py
cargo metadata --no-deps --format-version 1        # assert exactly four members
cargo fmt --all -- --check
cargo check --workspace --all-targets --offline
cargo test --workspace --all-targets --offline
cargo test --workspace --all-features --offline
cargo clippy --workspace --all-targets --all-features --offline -- -D warnings
python3 -m py_compile tools/validate_docs.py
python3 tools/validate_docs.py docs
git diff --check
```

Every command is run `--offline` after the initial `cargo fetch --locked`, so no
test can reach a service even by accident.

## 20. Cross-references

- The design these tests implement: [P2 storage and task
  engine](P2-storage-task-engine.md)
- The DDL they pin: [P2 SQLite schema](P2-sqlite-schema.md)
- The contract gaps each group answers: [P2 contract gap
  analysis](P2-contract-gap-analysis.md)