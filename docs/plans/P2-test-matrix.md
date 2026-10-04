# P2 Test Matrix

- **Branch:** `p2/design-preparation`
- **Base commit:** `c3737039e3e38dbba554dc0b9075025f87948358`
- **Status:** plan only. No test is implemented by this run.
- **Method:** Red/Green. Each group specifies the RED to record **before**
  implementation, not an assertion that it was observed. A syntax error or fake
  permissive helper is manufactured RED; a P2D test against the intended new API
  may genuinely fail to compile because that production API is not yet present.

## Current P2A integration annotation (2026-10-03)

The plan-only header describes historical design preparation, not the current
P2A implementation. The coordinator owns actual final workspace/MSRV validation,
counts, bounded regression review and integration evidence in the
[closure record](P2A-review-and-closure.md); this matrix asserts no final count
or PASS. The numeric follow-up uses field-local `serde_json/raw_value` generation
decoding without f64 rounding, `jsonschema/arbitrary-precision`, an exact 0.58.3
pin and the narrow `vendor/jsonschema-value` integer-classification/checked-overflow
patch. [Launch §3](P2-6.1-sol-launch.md#3-dependency-lines-current-p2a-integration-and-p2c-candidate)
records its limited guarantee and debug/release, stable/Rust 1.85 regression
obligations, including direct schema validation and vendor helpers. It does not
establish unrestricted exact schema arithmetic or introduce SQLite/runtime.
The companion pinned `vendor/serde_json` transport patch preserves literal marker
objects and genuine precise numbers through raw/Value/flatten paths; ten tests
in `json_value_preservation.rs` cover that independent N3 regression.
Historical audit/probe measurements retain their original scope.

## Current P2D frozen-gate annotation (2026-10-04)

[P2D's frozen gate](P2D-review-and-closure.md) supersedes the old Group G
sketch. P2D is JSON blob put/get and BlobRef with an owned PRIVATE-only protection
seam. G10/G17/G18 and public references/roles/deletion/blob+reference atomicity
are P2F. G19 and other FK/corruption cases use private storage SQL fixtures only;
no public task/step mutation is added to make a test reachable. The local
`cfg(test)` double is NOT ENCRYPTION, NOT SECURITY, NEVER PRODUCTION; no testkit
API, dependency or smoke-rule change. Complete ordinary-row PRIVATE protection,
including extensions, stays deferred/fail-closed even with a blob backend and
ADR-0022 stays Proposed. No P2D RED/GREEN or runtime-test PASS is claimed here.

## Current P2E frozen-gate annotation (2026-10-04)

[P2E E1–E11](P2E-review-and-closure.md#1-preflight-and-frozen-pre-implementation-gate)
is the frozen **lease-authority gate**, now implemented with actual closure
implementation/review/remediation and stable/MSRV/debug/release evidence in that
record. **P2E is CLOSED for authority only.** P2E tests acquire/renew/release,
durable ceilings, pre-mutation overflow and caller-caught-error savepoint atomicity
only. P2F owns begin and embedded outcome fences: H9–H13, outcome H15/H22, begin
H18 and deletion H17. No fake P2E outcome API. Historical probes do not prove the
current gate; ADR-0024 remains Proposed even after lease-only GREEN. No schema change.

## Current P2F bounded storage annotation (2026-10-04)

[P2F evidence](P2F-review-and-closure.md) owns actual begin/known-outcome tests:
H9–H13, outcome H15/H22, begin H18, success RESULT reference/receipt/journal/task
atomicity and method/outer failure safety. Exact/past expiry current outcomes
commit; expiry reclaim/release fence them. Isolated production UPDATE tests and
an owner-only mutant control separately pin embedded generation predicates.
The supported layout is ordinary-prefix / VERIFY-suffix. Broader engine,
plan/query/delete/H17, waiting/cancellation, recovery and full participant seam
rows below remain historical future obligations, not PASS for this bounded slice.
No new event runtime or test count is inferred here; use the linked closure.

## 1. Ground rules

Inherited from P1's closure and re-applied:

1. **No test reads a wall clock.** `.clippy.toml` bans `SystemTime::now` and
   `Instant::now` workspace-wide, so this is a compile error, not a review comment.
   Time comes from `TestClock` through the `Clock` trait.
2. **No network, no filesystem outside the test's own temporary directory, no
   subprocess** except the deliberate child-process crash harness.
3. **Every fixture is synthetic.** `.test` domains, as P1 established.
4. **No RED is manufactured by a syntax error or permissive fake helper.** Write
   tests against the *intended production* signature. For P2D's genuinely absent
   blob API, a missing-API compile failure is valid initial RED; after the API
   exists, pin behaviour with assertions. Do not replace production dispatch with
   a helper and call its failure pre-fix evidence.
5. **A fix without a test that fails on the pre-fix code is not a fix.** P1's
   Pass C found nine unpinned fixes by mutating each one out; this matrix assumes
   that discipline and is written to be mutation-checked.

## 2. Groups, and what each one pins

| Group | Subphase | Pins |
| --- | --- | --- |
| A. Text validation categories | P2A | ADR-0023 |
| B. Step presence and step-kind matrices | P2A | ADR-0018 §3, §4 |
| C. Canonical JSON and digest vectors | P2A | ADR-0019 §1 |
| D. Idempotency preimage vectors and the collision | P2A | ADR-0019 §2–§3 |
| E. Clock and time | P2B | §8 of the design |
| F. Migrations and connection policy | P2C, except F25/F26 in P2H | §7 of the design |
| G. Blobs and classification | P2D except G10/G17/G18 in P2F | Frozen P2D gate, ADR-0022 (Proposed), schema §5; G19/FK fixtures private |
| H. Lease authority and outcome fencing | P2E authority / P2F begin, outcomes and deletion | Frozen P2E gate; ADR-0024 remains Proposed; exact H15/H18/H22 splits in §10 |
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
| A6d | `generated_category_o_pattern_matches_the_frozen_prefix_and_verb_lists` | Eleven ULID prefixes plus the idk_ domain **and** fourteen verbs come from `ids.rs`. A hand-written copy drifts, and a drifted copy means the schema accepts an `ActorId` Rust refuses |
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
| B12 | `an_unknown_wire_step_status_parses_with_kind_invariants` | Known-status presence only; engine refusal/blocking deferred P2F |
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
| D1–D7 | Five typed object vectors plus two historical A/B vectors only in low-level private raw-string framing tests | ADR-0019; typed public derivation rejects p.r.list, not valid SCJ-1 scalar roots |
| D8 | `legal_id_generic_scalar_naive_collision_is_separated_by_named_framing` | pp.rr.list, scalar -12/2; generic framing, not legal ActionRequests |
| D9 | `idk1_is_injective_over_the_frozen_grammar_corpus` | Rebuild with legal 2–32 char ID segments; distinguish typed generic derivation and object-root ActionRequest domains; preimage injectivity, not hash injectivity |
| D10 | `naive_concatenation_would_collide_on_the_pinned_pair` | Asserts the collision is *real*, so the test cannot be made vacuous by changing the encoding |
| D11 | `idk1_preimage_is_domain_separated_from_a_content_digest` | `sha256(scj1(x))` and `derive_idempotency_key` over the same document must differ |
| D12 | `idk1_rejects_a_non_capability_step_tuple` | The §5.2 boundary at the function level |
| D13 | `typed_derivation_accepts_all_scj1_roots_and_refuses_invalid_ids` | Valid pp.rr.list with scalar/object/array/string/bool/null roots; p.r.list rejection; ActionRequest separately refuses non-object arguments. Historical scalar pair is not legal-action collision evidence |

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

**Accepted design; tests still to be evidenced.** All P2B BLOCKER/MAJOR design
findings are accepted and resolved by the corrected frozen
[design §8](P2-storage-task-engine.md#8-clock-and-time-representation), before
production implementation. The cases below are requirements, not PASS results
or a P2B completion claim.

| # | Test | Pins |
| --- | --- | --- |
| E1 | `two_test_clocks_driven_identically_report_identical_time` | Determinism through injected `&dyn Clock`; compile-time `Clock: Send + Sync`, synchronous object safety and `Result<EpochMillis, ProtocolError>`; typed Clock errors remain representable, no ambient/system clock |
| E2 | `both_wire_forms_preserve_the_instant_and_epoch_output_is_canonical` | Seconds and `.mmmZ` accepted unchanged, year0000 legal. Epoch → Timestamp → epoch is exact; seconds → epoch → Timestamp yields `.000Z`, **not** original spelling. The original Timestamp retains its exact spelling in serialization/Eq/Hash. Explicit string evidence: `"2026-01-01T09:14:22Z" > "2026-01-01T09:14:22.000Z"`, but their EpochMillis values are **equal**; the two Timestamp objects remain unequal and keep their spelling-based hash behavior (do not require unequal hash outputs) |
| E3 | `lexicographic_timestamp_order_is_wrong_and_epoch_millis_is_not` | Explicit **string** comparison: `"2026-01-01T09:14:22Z" > "2026-01-01T09:14:22.100Z"`, while the converted EpochMillis values satisfy **<**. Never use Timestamp comparison operators: its PartialOrd/Ord are removed, with no repo consumers; negative epochs and epoch zero also order numerically |
| E4 | `wire_grammar_and_calendar_validation_are_unchanged` | Refuse `2026-02-30T00:00:00.000Z`, invalid leap days, offsets and fractional widths other than exactly three digits; accept both legal wire forms, year0000 (including its leap day), pre-1970, 2000 leap day and the final millisecond of year9999; refuse 1900 leap day. Conversion shares Timestamp validation, not a private parser |
| E5 | `epoch_millis_signed_bounds_and_private_construction_are_enforced` | Private `i64` field, checked constructor/get/numeric Ord; MIN=-62_167_219_200_000 ↔ `0000-01-01T00:00:00.000Z`, MAX=253_402_300_799_999 ↔ `9999-12-31T23:59:59.999Z`; endpoints accepted, MIN−1/MAX+1/i64 extremes refused. Pin -1/0 and 2038 boundaries plus no unchecked public construction path. TimestampMs remains unsigned48 ULID time, with its existing acceptance/refusal regressions unchanged. **Store-open clock-error behavior is deferred to P2C**, not an out-of-range successful P2B Clock reading |
| E6 | `test_clock_epoch_millis_is_the_single_authority` | `start_ms` and `now_ms` are EpochMillis; elapsed is their difference, not a separately mutable counter. `at` uses Timestamp validation/conversion and accepts seconds/year0000/negative epochs; `format` uses canonical `.mmmZ` conversion, no private calendar/parser. Zero and exact-MAX advance succeed; beyond-MAX, overflow and Duration::MAX return typed errors without changing current/elapsed time; duration magnitude checked before narrowing |
| E7 | `no_wall_clock_call_exists_in_current_p2b_sources` | Source-level assertion over current `serea-protocol`/`serea-testkit` P2B sources and tests, embedded examples and any build scripts present, complementing `.clippy.toml`; **no new storage or task-engine crate prerequisite**. Later storage/engine assertions belong to their phases/O3 |

Supporting API checks must pin the private EpochMillis field and absence of
Timestamp PartialOrd/Ord without manufacturing a syntax-error RED. Preserve
existing Timestamp wire-validation and unsigned48 TimestampMs/ULID tests. No
schema, ADR or protocol-version change, storage implementation or testkit-private
calendar/parser belongs to this group.

## 8. Group F — migrations and connection policy

Use a **file-backed** store for durability/reopen/connection-policy checks;
explicit memory-profile cases use memory and make no durability claim. These
are P2C requirements after the [frozen gate](P2C-review-and-closure.md), not PASS
evidence. Production `0001_initial.sql` is the sole schema authority.
**Applicable P2C set: F1–F35 minus F25/F26**, plus phase O4/O7. F25/F26 require
cross-binary/crash-child infrastructure and are explicitly **P2H**. Under owner
direction 21, P2C may add child infrastructure early but need not; “F green” means
the applicable set, not all 35 rows. No historical table below establishes PASS.

| # | Test | Pins |
| --- | --- | --- |
| F1 | `a_fresh_database_is_created_and_reaches_the_latest_version` | Accepted repair: absent/zero → common FK/FULL/timeout/capabilities in DELETE/FULL → initial migration + identity atomically under BEGIN IMMEDIATE → WAL/FULL. Every returned file Store is WAL/FULL; no in-progress nonempty unmarked WAL window. Add concurrent-init success/retryable Busy regression later, not a new tamper-defence claim |
| F2 | `reopening_after_migration_is_a_no_op` | Second `open` applies nothing |
| F3 | `a_newer_schema_is_refused_rather_than_downgraded` | `SchemaTooNew`; the version is left untouched. One ordered SELECT snapshot; any typed newer version anywhere in that snapshot wins over malformed-field/prefix errors, including concurrent upgrades |
| F4 | `a_non_serea_file_is_refused_rather_than_adopted` | `NotSereaStore`; any nonempty file without a catalog, including valid SQLite with **zero user tables**; read-only inspection before WAL, main bytes/journal mode unchanged. Transient SQLite WAL sidecars are not guaranteed absent |
| F5 | `a_zero_length_file_is_treated_as_fresh` | The fresh-database edge |
| F6 | `a_migration_checksum_mismatch_refuses_the_open` | `MigrationChecksumMismatch`; exact UTF-8 bytes including whitespace/comments/final newline, not canonical JSON; no tamper-evidence claim |
| F7 | `a_failed_migration_leaves_no_partial_schema_and_no_version_row` | The DDL and the version row share a transaction |
| F8 | `wal_is_enabled_and_asserted_at_open` | `PRAGMA journal_mode` |
| F9 | `foreign_keys_are_enabled_and_asserted_at_open` | `PRAGMA foreign_keys` returns 1 |
| F10 | `a_foreign_key_violation_is_refused` | The pragma actually works |
| F11 | `synchronous_is_full` | The durability policy |
| F12 | `json1_is_present` | `SELECT json_valid('{}')` succeeds; the open fails if not |
| F13 | `strict_tables_reject_a_wrong_column_type` | Lossless integer → TEXT is **accepted**, not refused. Test that control explicitly; use BLOB → TEXT or nonnumeric TEXT → INTEGER for a genuine datatype rejection |
| F14 | `a_rollback_inside_transact_leaves_no_row` | The ordinary in-process rollback path |
| F15 | `integrity_check_reports_a_healthy_database` | `verify_integrity` |
| F16 | `a_corrupted_file_is_refused_rather_than_recreated` | Corrupt bytes in the header; the store does **not** delete and recreate, which would destroy receipts |
| F17 | `two_store_instances_on_one_file_both_succeed` | No exclusive lock at open |
| F18 | `a_transaction_is_serialised_against_a_second_writer` | `BEGIN IMMEDIATE` plus `busy_timeout` |
| F19 | **`open_in_memory_reports_memory_not_wal_and_asserts_it`** | `TestMemoryProfile`. `open_in_memory` **cannot** satisfy ADR-0005, so it asserts what it actually is — a test cannot "pass" here by skipping the check |
| F20 | **`synchronous_is_not_asserted_in_the_memory_profile`** | `PRAGMA synchronous = 2` on `:memory:` returns **no row**: there is nothing to fsync. Asserting `FULL` there would be asserting nothing |
| F21 | **`foreign_keys_are_asserted_in_the_memory_profile_too`** | Bundled default is **ON** on both profiles, not OFF; set/assert ON explicitly anyway, never inside a transaction |
| F22 | **`foreign_key_check_is_run_after_each_migration`** | The tier the audit added. A migration that produced dangling references has failed in a way `quick_check` cannot see |
| F23 | **`quick_check_and_integrity_check_report_ok_on_an_fk_orphan`** | **The evidence F22 rests on.** Both are page-level checks; against a deliberately orphaned `task_steps` row both return `ok` and only `foreign_key_check` reports it. If this test ever fails, SQLite's semantics changed and the tier policy must be revisited |
| F24 | **`verify_integrity_runs_both_integrity_check_and_foreign_key_check`** | The admin tier's actual contract, which was previously unspecified |
| F25 | **`two_integration_test_binaries_get_distinct_temp_paths`** | **P2H, deferred from P2C.** `TempStore` cross-binary identity. A counter alone collides across binaries — historical probe: two binaries each counting from 0 produce the same three names |
| F26 | **`a_crash_child_reopens_the_parent_database_by_inherited_directory`** | **P2H, deferred from P2C.** Crash-child inherited-directory reopen. A pid alone is not unique *within* a process, and `cargo test` runs tests as threads |
| F27 | **`temp_paths_contain_no_wall_clock_and_no_rng`** | The `.clippy.toml` ban holds while uniqueness is still achieved: `<binary>-<pid>-<atomic-counter>` |
| F28 | **`the_migrated_object_inventory_is_10_tables_7_triggers_6_indexes`** | Asserted, not printed — so a phantom object cannot be reintroduced. **This is the direct regression for the `leases_generation_matches_step` trigger that §4.6 published and §4.0 never contained** |
| F29 | **`the_resolved_bundled_sqlite_is_at_least_3_37`** | Historical candidate probe used 3.53.2; verify actual resolved source/version/compile options/linking. LIBSQLITE3_SYS_USE_PKG_CONFIG=1 selects linked mode; SQLITE3_LIB_DIR is only a linked-branch search path, not a selector |
| F30 | `the_catalog_is_a_full_ordered_embedded_prefix` | One ordered SELECT result snapshot, typed newer priority across every row before prefix/field checks; gaps, wrong name, malformed checksum/version refused, never only MAX. Nonempty read-only preflight → RW revalidation → WAL/FULL → pending upgrades with catalog revalidation under IMMEDIATE; fresh bootstrap order is F1 |
| F31 | `all_fourteen_sql_instants_enforce_epoch_millis_bounds` | Exact inclusive MIN -62167219200000 / MAX 253402300799999; endpoints accepted where other invariants permit, adjacent/i64 extremes refused; nullable fields retain NULL; counters/durations unchanged |
| F32 | `open_reads_clock_before_mutation_and_does_not_retain_it` | Typed Clock error writes nothing; only validated migration stamp retained, no Store clock borrow/lifetime |
| F33 | `checkpoint_for_close_is_retryable_and_reports_typed_busy` | Three columns `(busy, log_frames, checkpointed_frames)` read; live-reader Busy is not success; release reader and retry same Store to Complete, then drop/copy. Drop alone is not reportable checkpoint |
| F34 | `memory_close_checkpoint_is_not_applicable` | API returns NotApplicable; SQLite raw pragma shape is `(0, -1, -1)`, not scalar 0 |
| F35 | `p2c_api_has_no_query_enum_or_protection_seam` | Opaque Tx only; TaskQueries/view/StepPhase deferred to P2F, no storage-owned lifecycle enum; AtRestProtection trait/constructor seam deferred to P2D |

## 9. Group G — blobs and classification

Required coverage, **not PASS evidence**. Applicable P2D set:
**G1–G9, G11–G16, G19–G35**. G10/G17/G18 are P2F. Composite FK/metadata
corruption tests use storage-private SQL fixtures, never public parent mutation.

| # | Test | Pins |
| --- | --- | --- |
| G1 | `a_blob_is_stored_under_the_digest_of_its_canonical_bytes` | SHA-256 of SCJ-1 canonical **plaintext**, never protected envelope bytes |
| G2 | `the_same_bytes_at_the_same_class_dedupe_to_one_row` | Existing row passes full read/unprotect/SCJ-1/digest verification before reuse |
| G3 | `the_same_bytes_at_two_classes_store_two_rows` | Composite key, including PRIVATE with the local double; PUBLIC/PERSONAL use NONE |
| G4 | `a_public_reference_cannot_read_a_private_blob` | Construct PUBLIC and PERSONAL refs for a PRIVATE-only digest: exact lookup yields `BlobMissing`, never lower-class substitution |
| G5 | `a_corrupt_existing_row_is_refused_on_dedupe` | **Replaces obsolete wrong-digest-write G5.** Privately edit content under an existing digest, then put the original JSON; `BlobCorrupt`, no overwrite or conflict-success. No expected-digest parameter / `DigestMismatch` variant |
| G6 | `a_corrupt_blob_is_detected_on_read` | Valid but wrong JSON and non-SCJ-1 stored bytes; `BlobCorrupt`, not silent data or caller-input error |
| G7 | `read_returns_bytes_identical_to_canonical_bytes` | PUBLIC/PERSONAL and test-double PRIVATE return canonical plaintext |
| G8 | `canonical_forms_that_differ_only_in_member_order_dedupe` | ADR-0019 SCJ-1; original raw JSON inputs, whitespace/order normalize |
| G9 | `a_rollback_after_put_blob_leaves_no_blob_row` | In-process blob rollback only; **not** orphan prevention, crash durability or blob+reference atomicity |
| G10 | `a_rollback_after_put_blob_leaves_no_reference_row` | **P2F deferred.** Whole-transition blob+reference rollback/atomicity, not a P2D reference API |
| G11 | `private_without_a_backend_is_refused_and_writes_nothing` | Put and get return `AtRestProtectionUnavailable`; no plaintext fallback. Existing-row cases G21 |
| G12 | `private_with_a_backend_stores_bytes_that_are_not_the_plaintext` | Local `cfg(test)` synthetic reversible double: NOT ENCRYPTION, NOT SECURITY, NEVER PRODUCTION; canonical plaintext digest and AT_REST marker |
| G13 | `secret_is_refused_on_every_blob_path` | Put/get including constructed SECRET refs return `ClassRefused`, even with a backend |
| G14 | `credential_is_refused_on_every_blob_path` | Put/get including constructed CREDENTIAL refs return `ClassRefused`, even with a backend |
| G15 | `a_hand_written_insert_of_a_secret_row_is_refused_by_the_check` | Both rank 3 and 4; unchanged production CHECK, not just Rust dispatch; full cap inventory G33 |
| G16 | `a_hand_written_insert_of_a_private_row_without_protection_is_refused` | Missing/wrong AT_REST **marker** refused; correctly labelled plaintext control is SQL-accepted. Not encryption/backend proof |
| G17 | `deleting_a_task_cascades_to_every_dependent_row_in_one_transaction` | **P2F deferred.** Task Protocol §8, no P2D delete_task |
| G18 | `an_unreferenced_blob_survives_a_task_delete_because_another_task_references_it` | **P2F deferred.** Shared-blob deletion behaviour; historical test name retained |
| G19 | `a_still_referenced_blob_cannot_be_deleted_directly` | P2D **private SQL fixture** for existing ON DELETE RESTRICT; no public reference/deletion writer |
| G20 | `size_bytes_matches_length_of_content` | Stored-content length, including PRIVATE envelope/expansion; not plaintext length or a bound; INSERT consistency and read/dedupe validation |
| G21 | `private_existing_row_cannot_bypass_missing_backend_on_read_or_dedupe` | Write with test backend, reopen without it, then put/get exact PRIVATE row: `AtRestProtectionUnavailable`, existing row unchanged |
| G22 | `backend_protect_and_unprotect_fail_closed` | Inject protect/unprotect refusal/failure; put/get/dedupe return `AtRestProtectionFailed` through the payload-free unit error; no successful reuse or inserted row |
| G23 | `unprotected_invalid_or_wrong_plaintext_is_corrupt` | Backend returns invalid UTF-8/JSON, SCJ-1-refused JSON or valid wrong-digest JSON: get/dedupe `BlobCorrupt`, distinct from backend failure |
| G24 | `stored_size_corruption_is_refused_on_read_and_dedupe` | Private fixture disables CHECK only to inject inconsistent size, restores it, then exercises PUBLIC/PERSONAL/PRIVATE verification |
| G25 | `protection_marker_corruption_is_refused_on_read_and_dedupe` | PRIVATE/NONE and PUBLIC/PERSONAL/AT_REST injected privately; `BlobCorrupt` even though lookup succeeds |
| G26 | `put_accepts_original_json_only_with_scj1_refusals` | Malformed JSON/UTF-8, nested/escaped duplicate names, fractional/exponent numbers, range and depth refusals; accepting scalar/object/array, integer, Unicode and whitespace controls. Fractional model temperature is not coerced |
| G27 | `nondeterministic_envelopes_keep_plaintext_identity_and_verified_dedupe` | Expanding/nondeterministic test transform round-trips; same canonical plaintext keeps digest and one same-class row. Stored length includes envelope; crypto is not inferred |
| G28 | `blob_ref_is_identification_not_authority` | Private validated Digest/DataClass fields, public constructor/read accessors/safe Debug, no Serialize; missing/forged exact refs yield BlobMissing, not alias or authority |
| G29 | `store_owns_protection_without_borrowed_lifetimes` | Drop caller Arc after constructor, use Store across transactions; object-safe Send + Sync trait and payload-free unit error; no class parameter/capability list |
| G30 | `protection_constructors_share_the_p2c_open_path` | File/memory protection constructors preserve migration/connection/Clock/checkpoint policy; normal constructors retain no backend, no new Store lifetime |
| G31 | `composite_reference_fks_reject_lower_class_substitution` | Private task/step/ref fixtures pin exact digest+rank FKs, including PLAN/PLAN_REVISION/ARGUMENTS/INSTRUCTION/RESULT schema roles. No public attachment API; FK-OFF control shows dangling ref acceptance, not successful PRIVATE read |
| G32 | `p2d_surface_excludes_text_reference_and_parent_mutation` | No text/role/attachment/delete_task/task/step/receipt/journal APIs or lease/engine runtime. Future ordinary-row PRIVATE writers must refuse even with blob protection until the full row design exists; no P2D runtime claim for absent writers |
| G33 | `p2d_preserves_migration_and_all_seven_class_caps` | Exact production 0001/catalog/checksum unchanged, no 0002; private fixtures exercise SECRET/CREDENTIAL caps on blobs/tasks/receipts/revisions/journal and both ref tables |
| G34 | `blob_and_backend_errors_disclose_no_payload_or_source_chain` | Display/Debug/source checks for canonical, missing/corrupt, class refusal and protection unavailable/failed categories; no rejected JSON, protected bytes or backend diagnostics |
| G35 | `blob_dispatch_calls_protection_only_for_private` | PUBLIC/PERSONAL never call configured protect/unprotect; SECRET/CREDENTIAL refuse without calling it. PRIVATE read/dedupe requires unprotect and verification; refusals preserve existing rows and insert none |

**P2D RED requirement, not an observed result.** Write tests against intended
`Tx::put_blob/get_blob`, `BlobRef` and protection constructors first. A genuine
compile failure naming those missing production APIs is valid initial RED; do
not create a permissive helper or unchecked substitute table. P2C's production
migration already has the relevant CHECKs, so claiming G15 RED against an
unchecked table would be fabricated evidence. Once APIs exist, behaviour cases
must pin failures directly, notably replacement G5 and PRIVATE existing-row
read/dedupe refusal. Record actual RED/GREEN commands in the P2D closure record;
this documentation run provides none.

## 10. Group H — lease authority (P2E) and outcome fencing (P2F)

Use **two separate file-backed `Store` instances on the same file** for authority
and stale-worker tests, not a local lease registry or an in-memory substitute.
P2E proves authority across connections; P2F separately proves embedded outcome
UPDATE fences. Private SQL fixtures may seed parents/budgets/corruption but do not
authorize public lifecycle/outcome methods in P2E. These are planned regressions,
not blanket observed RED/GREEN or runtime PASS. P2E authority and the bounded
P2F begin/outcome rows have their own linked closure evidence above.

| # | Phase | Test | Pins |
| --- | --- | --- | --- |
| H1 | P2E | `acquire_lease_succeeds_from_planned` | None -> SQL0 expectation; first generation/attempt 1 |
| H2 | P2E | `a_competing_acquisition_is_refused` | `LeaseHeld` before stale expectation on a bound eligible step; complete savepoint, not one statement |
| H3 | P2E | `an_expired_lease_is_reclaimable` | `expires_at_ms <= now`, including equality |
| H4 | P2E | `a_reclaim_increments_the_generation` | Exact positive caller expectation, strictly newer authority |
| H5 | P2E | `renew_extends_an_unexpired_lease` | New expiry strictly greater than authoritative old expiry; only leases expiry changes, step snapshot unchanged |
| H6 | P2E | `renew_is_refused_after_expiry` | Matching unreleased expiry <= now gives `LeaseExpired`; no resurrection |
| H7 | P2E | `renew_by_a_stale_generation_is_refused` | Stale/missing/released authority is `LeaseFenced`, not `LeaseExpired`, with precedence over interval refusal |
| H8 | P2E | `release_fences_the_guard_permanently` | Only released_at changes; expired matching lease may release, step copy unchanged; consumed on every result and later renew is fenced |
| H9 | P2F | `a_commit_under_a_current_lease_succeeds` | Actual whole-outcome API with embedded authoritative fence, not a P2E fake commit |
| H10 | P2F | **`a_stale_generation_cannot_commit_after_a_reclaim`** | Same-owner reclaim must fence old outcome |
| H11 | P2F | `a_stale_generation_commit_inserts_no_receipt_row` | Fenced UPDATE is first mutation; explicit zero-row error/rollback before receipt |
| H12 | P2F | `a_stale_generation_commit_inserts_no_journal_row` | No participant/journal append on refused outcome |
| H13 | P2F | `a_stale_generation_commit_leaves_the_task_state_untouched` | No task advancement or other outcome side effects |
| H14 | P2E | `two_acquisitions_by_the_same_owner_string_are_distinguished_by_the_generation` | Generation, not a removed token, distinguishes acquisitions; stale guard cannot renew/release |
| H14b | P2E | `the_step_generation_is_derived_from_the_leases_row_not_guessed` | Derived copy under the complete savepoint; no generation consistency trigger |
| H15 (acquisition) | P2E | `the_generation_and_step_column_agree_after_every_acquisition` | Derived-copy agreement after success; both copies unchanged after refusal |
| H15 (outcome) | P2F | `the_generation_and_step_column_agree_after_every_commit` | Real outcome integration; terminal generation retained, owner/expiry cleared |
| H16 | P2E | `no_error_or_guard_rendering_carries_a_lease_owner` | Payload-free categories and no owner formatter/source-chain leak; no removed-token assertion |
| H17 | P2F | `deleting_a_step_cascades_its_lease_row` | Physical deletion cascade, not release_lease; no P2E deletion API |
| H18 (acquisition) | P2E | `attempt_increments_once_on_acquisition` | Assert attempt 1 after acquisition, once per reclaim; refusal spends nothing |
| H18 (begin) | P2F | **`attempt_increments_exactly_once_across_acquire_and_begin_attempt`** | Assert attempt still 1 after borrowed begin; no second charge |
| H19 | P2E | **`a_ceiling_of_zero_leaves_the_step_planned_with_no_lease_row`** | Durable budget refusal; no acquisition or durable release occurred |
| H20 | P2E | **`a_refused_acquisition_reverts_to_the_prior_committed_state`** | Ceiling 2, third refused: `('LEASED', 2, 2)` and one leases row; also catch Err and return Ok from outer body |
| H21 | P2E | **`a_crash_only_loop_is_stopped_by_the_attempt_ceiling_with_zero_executions`** | Two acquisitions then refusal at ceiling 2; expiry simulates a pre-begin crash, not child-process durability proof |
| H22 (authority) | P2E | **`lease_authority_holds_across_two_independent_connections_on_one_file`** | Acquire/reclaim/renew/release against SQLite authority across separate Stores; no local registry |
| H22 (outcome) | P2F | **`the_outcome_fence_holds_across_two_independent_connections_on_one_file`** | Actual stale outcome UPDATE returns zero/LeaseFenced; H11–H13 assertions across connections |

**Planned RED, not historical proof:** H6 must fail if renewal lacks the expiry
conjunct; H14/H22 authority must fail if stale generations can renew/release.
P2F H10 must fail with owner-only outcome fencing, even for same-owner reclaim.
P2E cannot record H10 RED/GREEN without P2F's production outcome method.

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
| I10 | `the_attempt_ceiling_is_read_from_durable_state_at_the_check` | P2F integration of P2E's durable acquisition-budget gate, Bounds §2.1 |
| I11 | `the_attempt_ceiling_is_reached_after_max_attempts_per_step` | P2F integration; P2E owns acquisition refusal |
| I12 | `a_fenced_commit_and_an_attempt_ceiling_are_distinguishable_errors` | P2F outcome LeaseFenced versus P2E acquisition AttemptCeilingReached; neither is inferred from a generic constraint failure |
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
| O4 | `the_testkit_is_unreachable_from_either_runtime_crate` | Smoke checks every dependency table (including aliases, target/build tables); storage's only internal runtime dependency is protocol, testkit dev-only |
| O5 | `no_frozen_enum_set_grew` | `DataClass` 5, `RiskClass` 8, `TaskState` 11, `StepKind` 8, `ActionErrorKind` 13, `EventKind` 60 |
| O6 | `no_store_method_writes_state_outside_transact` | A source assertion over `serea-storage`'s `impl Store`. **This is the test that makes §3.2 of the design mechanical rather than aspirational** |
| O7 | `the_workspace_contains_exactly_the_phase_members` | **P2C: exactly three**, protocol/storage/testkit, asserted by smoke and CI; no placeholder engine. P2F adds engine as the fourth and updates both guards in that phase |
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
| P2A | A/B/C/D groups and added boundary cases below | Shape/schema planned-step RED; whitespace and canonical/framing failures | All four groups green atomically |
| P2B | Corrected E1–E7 plus API/ULID regressions | Signed bounds, instant-versus-spelling conversion, explicit string/epoch ordering and checked atomic TestClock failures | Record actual E/API/ULID and applicable workspace/MSRV results; no Store or new storage/engine crates; canonical groups already belong to P2A |
| P2C | Applicable F1–F35 **minus F25/F26**; phase O4/O7 | F1 against no store; F7 against DDL outside migration; fresh bootstrap/concurrent-init and one-snapshot newer-priority repairs; prefix/read-only/clock/instant/checkpoint boundaries | Implement foundation only; applicable F green, exactly three members; child infrastructure optional (owner direction 21) |
| P2D | G1–G9, G11–G16, G19–G35; private FK fixtures | Genuine compile RED against intended missing production blob/protection APIs, not a fake helper or unchecked table; actual evidence pending | Implement narrow blob seam and record actual focused/full stable/Rust 1.85 debug/release validation and independent reviews; ADR-0022 stays Proposed |
| P2E | H1–H8, H14/H14b, acquisition H15/H18/H22, H16, H19–H21 and §18.1 authority regressions | Intended missing-API RED if genuinely absent; H6 without expiry, stale renew/release, strict extension, caller-caught partial acquisition and explicit max-u32 refusal | Implement selected lease-authority gate only; record actual RED/GREEN/reviews/validation before closure. Partial implementation is not closure; no H9–H13 or fake outcome method, no schema change; ADR-0024 stays Proposed |
| P2F | H9–H13, outcome H15/H22, begin H18, deletion H17; I1–I15, J1–J15, K1–K8, L1–L10 plus G10/G17/G18 and whole-transition reference atomicity | H10 owner-only outcome fence and released authority despite unchanged step copy; H18 double charge; I1 omitted transition; J12 missing task_id | Implement begin and embedded whole-outcome fences with receipt/journal/task assertions plus engine/reference/role/deletion surface; PRIVATE ordinary-row writes fail closed even with a blob backend until complete row design |
| P2G | M1–M22 | M12 on the second-pass rewrite | Implement; M green |
| P2H | N1–N8 **plus F25/F26** | N4 and N6 on a `transact` with no injection point; cross-binary temp identity and crash-child inherited-directory reopen | Implement; N and deferred F25/F26 green |
| P2I | O1–O15 | O6 and O10 on a `Store` with a bare `update_task_state` | Remove; all green |

**Ordering rule.** Group O is written in P2I but its *intent* is checked
continuously: if P2A adds a wall clock, `cargo clippy` fails immediately, not in
P2I.

## 18. What this matrix does not test, and must not claim

| Not tested | Why | Deferred to |
| --- | --- | --- |
| That a real at-rest backend is sound | P2 ships none; local double is synthetic wiring only | The backend's own decision (ADR-0022, open question 1) |
| Complete ordinary-row PRIVATE support | P2D has no text API or full reversible row representation; future PRIVATE-bearing task/step/receipt/journal writes, including extensions, refuse even with a blob backend | ADR-0022 open question 3; ADR stays Proposed |
| P2D blob+reference atomicity, orphan prevention or crash survival | G9 is ordinary blob rollback only; P2D has no reference/deletion writer | P2F attachment/roles/deletion; P2H crash evidence |
| P2E begin/outcome fencing, receipt/journal/task atomicity or full ADR acceptance | Lease-authority tests and any partial implementation cannot exercise these real lifecycle APIs; historical probes are not current runtime proof | P2F H9–H13, outcome H15/H22, begin H18 and deletion H17; ADR-0024 remains Proposed |
| That a `SECRET` sealed store exists | It does not | Open question 2 |
| `C4`'s `side_effect_class != NONE` half | No Capability Registry | P5 |
| `T6`'s risk-class comparison | No descriptors | P5 |
| `E3`, `E4` | No event bus | P3 |
| Receipt absence on a `RECONCILED_ABSENT` step | A cross-table property; no `CHECK` can span tables. The *reverse* direction is a trigger, this direction is recovery's invariant scan | — |
| `origin` / `attempt_budget` extension round-trip | Three JSON columns, pinned by I14 and F-group schema checks | — |
| Any §2 bound other than `max_attempts_per_step` | Counters belong to `serea-core` | `serea-core` |
| Payload-byte, blob-byte and object-count limits | **Still P0's open gap.** P2 imposes none and this matrix claims none | The separate bounds decision |
| Retention / `max_retained_tasks` | `delete_task` is P2F, not P2D; the 30-day trigger is **P12's**, because the bound configuration it reads and the purge notification both land in `serea-core` then | P12 |
| `max_concurrent_steps_per_task` | The engine's one-effecting-step-at-a-time rule is a convention with no `CHECK` or trigger behind it, so it is not structural and is not claimed | `serea-core` (the bound), P5 (the router that serialises effecting calls) |
| Read-back reconciliation of an ambiguous effect | P2 has no capability | P5 |
| Approval re-render against a device roster | P2 has no device link | P6/P12 |

## 18.1 Reconciled gate regressions

| Group / phase | Additional mandatory cases |
| --- | --- |
| A / P2A | Every Unicode White_Space code point at both edges/all-whitespace; U+0085; all C1 refusals; LF/TAB/2028/2029 interior in P but not L/O; CR refused. Exact idk_/sha256:, out-of-range ULID first character, goallatch versus goallatch_foo/goallatch1; all nested provider_reference occurrences and event actor. Independently expected accepts/refusals, not parity alone. |
| B / P2A | Four conversions only; seven unconditional fields. TaskStepDraft → private validated TaskStep/StepPresence, no public mutation bypass; unknown extensions retained but every reserved step key refused even when absent. Missing/null -> None, serializer omission; known-status matrix plus unknown status with kind invariants. **All five non-capability kinds refuse a receipt on ALL statuses, including unknown:** exercise each known status and an unknown code in Rust construction/deserialization and schema; missing/null receipts remain absent and serialize by omission. The SUCCEEDED optional receipt cell is capability-shaped only, not external action semantics on host-only kinds. Wire generation zero/overflow refused; optional positive u32 retained terminally. |
| C/D / P2A | Duplicate names (nested and escaped equivalents) refused before Value; lost duplicates cannot be recovered. f64 model temperature remains wire-valid but SCJ-1 refuses it. Names and values each framed; 282-byte vector1. Historical p.r.list A/B low-level private raw framing only; typed invalid-ID rejection; valid-ID typed derivation accepts every SCJ-1 root, including pinned pp.rr.list scalars and legal objects; ActionRequest rejects nonobjects. |
| B/SQL / P2C/P2F | Each of six error columns singly populated outside FAILED refused; all partial mandatory-error subsets refused on FAILED; details optional on FAILED. SQL generation0 <-> wireNone, positive u32, overflow rollback. Historical 32-cell probes alone are insufficient. |
| H API / P2E | Absolute EpochMillis now/expiry, no retained Clock or TTL API; caller expected_generation Option: None -> SQL0, Some positive exact, Some(0) -> LeaseFenced. Stale observed value must not be replaced by a fresh read. Guard private, no constructor/Clone/Copy/Serde/owner formatter/Drop release; outer rollback invalidates purported returned authority. Pin initial/reclaim rollback followed by same-owner generation reuse, failed commit and panic/reopen. Private origin marker only rejects invalid capability; pending same-Tx operations and committed cross-Store use must still work through SQLite authority. |
| H acquisition refusals / P2E | Acquire expiry <= now -> InvalidLeaseInterval; missing/wrong-parent/noneligible step -> LeaseFenced. For a valid interval/generation input and bound PLANNED/LEASED/EXECUTING step: held active -> LeaseHeld before stale expectation -> LeaseFenced before durable ceiling -> AttemptCeilingReached before eligible max-u32 -> payload-free LeaseGenerationOverflow. Pin max-u32 released/expired fixtures and held/stale/ceiling precedence, no mutation and bounded SQL predicate; no text matching, wrap/clamp or schema change. |
| H atomicity / P2E | Complete acquisition savepoint includes lease upsert, step UPDATE, durable ceiling and all later failures. Induce a failure after the first write, catch Err, return Ok and commit unrelated outer work: lease/step bytes must be pre-call unchanged. Rollback/release cleanup failure must mark outer Tx rollback-only and prevent commit even after body Ok; no reliance on caller ?. |
| H renew / P2E | Matching unreleased expired-at/before-now -> LeaseExpired; stale/released/missing/wrong binding -> LeaseFenced even with invalid interval. Unexpired matching new expiry equal/shorter than authoritative old -> InvalidLeaseInterval unchanged, including still-future shortening. Strict extension succeeds after a prior renewal using authoritative expiry, not stale step snapshot; only leases expiry changes. |
| H release / P2E | Matching expired release succeeds; now before acquired_at -> InvalidLeaseInterval, stale/released -> LeaseFenced. Only released_at changes, all step columns unchanged. Guard consumed on every result including infrastructure failure; Err implies no durable-success inference, eventual expiry/recovery required. No Drop SQL; inner success lost on outer rollback is not durable release. |
| H begin / P2F | Borrowed begin uses embedded authoritative matching unreleased/unexpired fence and cannot use released or stale authority despite unchanged step copy. Expired-unreclaimed begin refuses; H18 proves no second attempt increment. |
| H outcomes / P2F | H9–H13 and outcome H15/H22 use actual embedded UPDATE fences: authoritative row absent/wrong owner/generation/released refuses; same-owner reclaim fences old result. Refusal leaves receipt/journal/task assertions unchanged, including across separate file-backed Stores. Expired-but-unreclaimed/unreleased known outcome succeeds; outcome consumes guard. P2E authority-only GREEN proves none of this. |
| H deletion / P2F | H17 physical step deletion cascades its leases row via owning lifecycle/deletion surface; no P2E deletion API or release inference from cascade. |
| G / P2D | Original JSON/SCJ-1 only; class-first PRIVATE refusal even on existing-row read/dedupe; SECRET/CREDENTIAL put/get refusal; full marker/stored-length/unprotect/canonical-plaintext-digest verification and backend unit-error boundary. Owned Arc/private BlobRef/local cfg(test) double; unchanged migration. No text/reference/role/deletion API or PRIVATE ordinary-row support. Actual runtime evidence pending. |
| Transaction / P2C | Opaque Tx body Err rolls back; no public raw SQL/connection escape. No participant/journal runtime or ADR acceptance. |
| Transaction / P2F/P2G | Body Err never calls participants; participant failure rolls back writes; no-op records nothing; successful body returns immutable actual transition(s), journal shares transaction; no event_seq/backfill and P2 pending count is journal row count. |
| Registry / P2A | Per-surface task/action2 with event/model/etc1, envelope1; unsupported major refused per surface. Protocol manifest/source/schema/docs agree. |

## 19. Verification command set

The same shape P1 used, to be run at the end of each subphase:

```text
python3 tests/workspace_smoke.py                  # Python 3.9-compatible stdlib-only parser; unsupported syntax fails closed
python3 -m unittest discover -s tests -p workspace_smoke_tests.py
cargo metadata --no-deps --format-version 1        # P2C: protocol/storage/testkit; engine only P2F
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