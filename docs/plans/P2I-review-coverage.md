# P2I Bounded Review Coverage Ledger

This is a finite supporting review artifact, **not a closure claim**. The generation-0 reports `P2I-review-correctness.md` and `P2I-review-security.md` remain unchanged and incomplete.

Baseline/target: parent of P2A `694c78af1edc59faa4073aca4ed2315cd7bf833d` through P2H `8417b1fff325e311120050f6c733f185111a90bc`; current dirty P2I delta is separately captured by current diff and closure record. The inventory is mechanically computed from all nine phase commits and contains 242 unique changed paths. Phase labels identify commits touching a path. Category assignment is inventory, not source review.

## Bounded A1 passes

| Pass | Scope and governing contract | Inspected sources / representative tests | Reviewer | Disposition |
|---|---|---|---|---|
| A1-1 | P2A protocol/schema/canonical/vendor; Protocol Index §§4–5, ADR-0019 SCJ-1/IDK-1, ADR-0023. | Protocol `types.rs`, `ids.rs`, `canonical.rs`, schemas, ADR-0019, canonical vector/property/parity and JSON-value preservation tests; vendored numeric patch. | Main agent | PASS: shape/registry consistency; SCJ-1 test vectors, decoded duplicate refusal, integer range/lexical limits, IDK named framing reviewed. |
| A1-2 | P2C/P2D/P2E/P2F-a; ADR-0005, migration 0001, ADR-0022, ADR-0024, frozen P2F atomicity. | `migrate.rs`, `store.rs`, `blob.rs`, `lease.rs`, `outcome.rs`; representative foundation/blob/schema/lease/outcome tests. | Main agent; a bounded second opinion is supporting material only. | PASS: migration authority/profile, exact class/dedupe order, lease generation fence, receipt/outcome atomicity and stale-authority semantics reviewed. |
| A1-3 | P2F-b/P2G/P2I; frozen 121-edge transition oracle, plan/revision provenance, one TaskJournal, RecoveryPass restrictions, final P2G table. | Engine transition/journal/recovery and storage recovery sources; core/workflow/recovery suites; two zero-attempt regressions. | Main agent | PASS: eligibility is not execution; recovery savepoint, action predicates, snapshot reinspection and logical repeat behavior reviewed. See A8 ledger. |
| A1-4 | P2H N1–N8/F25–F26, Group O, phase/ADR claims. | Crash harness, `fault.rs`, release proof script, O1–O15 guards, P2H closure, ADR-0021/index. | Main agent | PASS: real child death vs returned error, N6 durable truth, N7 stress-only, default-feature seam boundary and timeout/portability caveats reviewed. |

Bounded-pass PASS results are evidence for synthesis, not substitutes for the two terminal reviews.

## Required category index

| Category | Phases | Governing contract | Representative evidence | Reviewer | Disposition |
|---|---|---|---|---|---|
| PROTOCOL_SCHEMA | P2A/P2B | Protocol Index and frozen schemas | schema, parity and time tests | Main agent A1-1 | PASS |
| CANONICAL_VENDOR | P2A | ADR-0019 SCJ-1/IDK-1 | vectors/properties/vendor numeric regressions | Main agent A1-1/A12 | PASS with narrow documented patch |
| CLOCK_TIME | P2B | EpochMillis/explicit-time contracts | P2B protocol and testkit clock tests | Main agent A1-1 | PASS |
| STORAGE_MIGRATION | P2C | ADR-0005 and migration 0001 | foundation/schema/migration tests | Main agent A1-2 | PASS |
| BLOB_CLASSIFICATION | P2D | class dispatch and reference identity | blob/protection tests | Main agent A1-2 | PASS |
| LEASE_FENCING | P2E | ADR-0024 authority/revocation | lease and stale guard tests | Main agent A1-2 | PASS |
| OUTCOME_ATOMICITY | P2F-a | embedded fence/receipt ordering | outcome and exact-SQL regression tests | Main agent A1-2 | PASS |
| TASK_ENGINE | P2F-b | 121-edge task lifecycle and plans | core/workflow/review tests | Main agent A1-3 | PASS |
| JOURNAL_AUDIT | P2F-b/P2G | ADR-0021 single audit mapper | audit and recovery journal tests | Main agent A1-3 | PASS; claims narrowed |
| RECOVERY | P2G/P2I | final P2G dispositions | engine/storage recovery tests | Main agent A1-3/A8 | PASS; see separate matrix |
| CRASH_FAULT | P2H | N1–N8/F25–F26 | crash suite/release proof | Main agent A1-4/A9 | PASS within default-artifact claim |
| TEST_CI_SMOKE | P2A–P2H | Group O and CI bounds | O1–O15 and smoke | Main agent A1-4 | PASS for reviewed guards |
| ADR_DOC_CLAIMS | P2A–P2H | phase closures/ADR statuses/nonclaims | closure docs and decision index | Main agent A1-4/A11 | PASS after correction noted below |

## A11 — phase record reconciliation

`git show -s --format=%H\ %P\ %s` verified all nine commits exist and form direct-parent sequence P2A→P2B→P2C→P2D→P2E→P2F-a→P2F-b→P2G→P2H. P2C introduced migration 0001; subsequent closures pin unchanged SHA-256 `d9068dccbc26ececb71be79c475080633166ba0163c62b2d98b9733512baefea`. Protocol version changes are in P2A. The P2I delta has no migration, vendor, Cargo.lock resolution or protocol surface changes.

| Closure | Commit / parent | Scope and Git truth | Current status, timeout, nonclaim reconciliation |
|---|---|---|---|
| P2A | `10622803` / `48467222` | Protocol/schema/vendor; no storage migration. | Intermediate PENDING and timeout evidence explicitly historical/superseded by §4.7; timeouts not PASS. |
| P2B | `98a038e` / P2A | Clock/time, no migration. | Bounded probe/enumeration timeouts explicitly excluded from PASS claims. |
| P2C | `1d4519e` / P2B | Migration 0001 introduction/checksum. | Timeouts retained as non-PASS; SQLite portability/durability limits remain bounded. |
| P2D | `ec3b785` / P2C | Blob/classification; migration unchanged. | Reviewer timeout not counted as PASS; PRIVATE encryption/backend claims excluded. |
| P2E | `df6088e` / P2D | Lease authority; no schema/protocol expansion claimed. | Pending and timeout statements are historical lineage; expiry limits retained. |
| P2F-a | `e82abd4` / P2E | Atomic outcome fencing; no migration/new protocol version. | Timeout distinctions retained; crash-window guarantee deferred to P2H. |
| P2F-b | `7c05ffa` / P2F-a | Engine/journal; closure §6 is final authority. | Sections 3–5 explicitly historical and superseded; timeouts not PASS; no P3/event claim. |
| P2G | `6fe6bd3` / P2F-b | Recovery; migration unchanged; dev-edge dependency metadata accurately called out. | Corrected stale present-tense preimplementation sentence in §4; final dispositions/validation authoritative. |
| P2H | `14017e3` / P2G | Fault/crash harness; migration/vendor/lock/protocol unchanged. | First release timeout explicitly non-PASS; later A/B/C/D exit 0; default-only and platform limits retained. |

**A11 = PASS:** no remaining material closure-record contradiction with present Git/source truth; historical evidence remains labeled historical.

## Mechanically derived P2A–P2H changed-path union

| Phase commit(s) | Assigned category | Path |
|---|---|---|
|P2C, P2F-b|TEST_CI_SMOKE|`.github/workflows/ci.yml`|
|P2A, P2C, P2F-b, P2G|PROTOCOL_SCHEMA, TEST_CI_SMOKE|`Cargo.lock`|
|P2A, P2C, P2F-b|PROTOCOL_SCHEMA, TEST_CI_SMOKE|`Cargo.toml`|
|P2A|PROTOCOL_SCHEMA|`crates/serea-protocol/Cargo.toml`|
|P2A|PROTOCOL_SCHEMA|`crates/serea-protocol/schemas/action-request.schema.json`|
|P2A|PROTOCOL_SCHEMA|`crates/serea-protocol/schemas/action-result.schema.json`|
|P2A|PROTOCOL_SCHEMA|`crates/serea-protocol/schemas/assistant-task.schema.json`|
|P2A|PROTOCOL_SCHEMA|`crates/serea-protocol/schemas/event.schema.json`|
|P2A|CANONICAL_VENDOR|`crates/serea-protocol/src/canonical.rs`|
|P2B|PROTOCOL_SCHEMA, CLOCK_TIME|`crates/serea-protocol/src/clock.rs`|
|P2A|PROTOCOL_SCHEMA|`crates/serea-protocol/src/errors.rs`|
|P2A|PROTOCOL_SCHEMA|`crates/serea-protocol/src/ids.rs`|
|P2A, P2B|PROTOCOL_SCHEMA|`crates/serea-protocol/src/lib.rs`|
|P2A|PROTOCOL_SCHEMA|`crates/serea-protocol/src/schema.rs`|
|P2A, P2B|PROTOCOL_SCHEMA|`crates/serea-protocol/src/types.rs`|
|P2A|CANONICAL_VENDOR, TEST_CI_SMOKE|`crates/serea-protocol/tests/canonical_properties.rs`|
|P2A|CANONICAL_VENDOR, TEST_CI_SMOKE|`crates/serea-protocol/tests/canonical_vectors.rs`|
|P2A|CANONICAL_VENDOR, TEST_CI_SMOKE|`crates/serea-protocol/tests/json_value_preservation.rs`|
|P2A|PROTOCOL_SCHEMA, TEST_CI_SMOKE|`crates/serea-protocol/tests/p2a_parity.rs`|
|P2A|PROTOCOL_SCHEMA, TEST_CI_SMOKE|`crates/serea-protocol/tests/p2a_schema_red.rs`|
|P2A|PROTOCOL_SCHEMA, TEST_CI_SMOKE|`crates/serea-protocol/tests/p2a_shape.rs`|
|P2A|PROTOCOL_SCHEMA, TEST_CI_SMOKE|`crates/serea-protocol/tests/p2a_types.rs`|
|P2B|PROTOCOL_SCHEMA, TEST_CI_SMOKE|`crates/serea-protocol/tests/p2b_time.rs`|
|P2A|PROTOCOL_SCHEMA, TEST_CI_SMOKE|`crates/serea-protocol/tests/protocol_types.rs`|
|P2A|PROTOCOL_SCHEMA, TEST_CI_SMOKE|`crates/serea-protocol/tests/schema_contracts.rs`|
|P2A|CANONICAL_VENDOR, TEST_CI_SMOKE|`crates/serea-protocol/tests/text_parity.rs`|
|P2C, P2F-b, P2H|ADR_DOC_CLAIMS|`crates/serea-storage/Cargo.toml`|
|P2C|STORAGE_MIGRATION|`crates/serea-storage/migrations/0001_initial.sql`|
|P2F-b, P2G, P2H|JOURNAL_AUDIT|`crates/serea-storage/src/audit.rs`|
|P2F-b|JOURNAL_AUDIT, TEST_CI_SMOKE|`crates/serea-storage/src/audit_tests.rs`|
|P2D, P2E|BLOB_CLASSIFICATION|`crates/serea-storage/src/blob.rs`|
|P2D, P2E, P2F-a, P2F-b, P2G|BLOB_CLASSIFICATION, TEST_CI_SMOKE|`crates/serea-storage/src/blob_tests.rs`|
|P2D|BLOB_CLASSIFICATION|`crates/serea-storage/src/classify.rs`|
|P2C, P2D, P2E, P2F-b, P2G|STORAGE_MIGRATION|`crates/serea-storage/src/error.rs`|
|P2H|CRASH_FAULT|`crates/serea-storage/src/fault.rs`|
|P2C|STORAGE_MIGRATION, TEST_CI_SMOKE|`crates/serea-storage/src/foundation_tests.rs`|
|P2E, P2F-a|LEASE_FENCING|`crates/serea-storage/src/lease.rs`|
|P2E|LEASE_FENCING, TEST_CI_SMOKE|`crates/serea-storage/src/lease_tests.rs`|
|P2C, P2D, P2E, P2F-a, P2F-b, P2G, P2H|STORAGE_MIGRATION|`crates/serea-storage/src/lib.rs`|
|P2F-b|TASK_ENGINE|`crates/serea-storage/src/lifecycle.rs`|
|P2F-b|TASK_ENGINE, TEST_CI_SMOKE|`crates/serea-storage/src/lifecycle_tests.rs`|
|P2C|STORAGE_MIGRATION|`crates/serea-storage/src/migrate.rs`|
|P2F-a, P2F-b, P2H|OUTCOME_ATOMICITY|`crates/serea-storage/src/outcome.rs`|
|P2F-a, P2F-b|OUTCOME_ATOMICITY, TEST_CI_SMOKE|`crates/serea-storage/src/outcome_review_tests.rs`|
|P2F-a, P2F-b|OUTCOME_ATOMICITY, TEST_CI_SMOKE|`crates/serea-storage/src/outcome_tests.rs`|
|P2D|BLOB_CLASSIFICATION, TEST_CI_SMOKE|`crates/serea-storage/src/protection_tests.rs`|
|P2G|RECOVERY|`crates/serea-storage/src/recovery.rs`|
|P2G|RECOVERY, TEST_CI_SMOKE|`crates/serea-storage/src/recovery_tests.rs`|
|P2C, P2D, P2F-b|STORAGE_MIGRATION, TEST_CI_SMOKE|`crates/serea-storage/src/schema_tests.rs`|
|P2C, P2D, P2E, P2F-a, P2F-b, P2H|STORAGE_MIGRATION|`crates/serea-storage/src/store.rs`|
|P2F-b, P2H|TASK_ENGINE|`crates/serea-storage/src/task.rs`|
|P2F-b|TASK_ENGINE, TEST_CI_SMOKE|`crates/serea-storage/src/task_tests.rs`|
|P2C, P2D, P2E, P2F-a, P2F-b, P2H|STORAGE_MIGRATION|`crates/serea-storage/src/tx.rs`|
|P2C|STORAGE_MIGRATION, TEST_CI_SMOKE|`crates/serea-storage/tests/foundation.rs`|
|P2F-b, P2G, P2H|TASK_ENGINE|`crates/serea-task-engine/Cargo.toml`|
|P2F-b, P2G|TASK_ENGINE|`crates/serea-task-engine/src/engine.rs`|
|P2F-b|TASK_ENGINE|`crates/serea-task-engine/src/error.rs`|
|P2F-b, P2G|JOURNAL_AUDIT|`crates/serea-task-engine/src/journal.rs`|
|P2F-b, P2G|TASK_ENGINE|`crates/serea-task-engine/src/lib.rs`|
|P2G|RECOVERY|`crates/serea-task-engine/src/recovery.rs`|
|P2F-b|TASK_ENGINE|`crates/serea-task-engine/src/transition.rs`|
|P2F-b|TASK_ENGINE|`crates/serea-task-engine/src/types.rs`|
|P2F-b|TASK_ENGINE, TEST_CI_SMOKE|`crates/serea-task-engine/tests/core.rs`|
|P2H|CRASH_FAULT, TEST_CI_SMOKE|`crates/serea-task-engine/tests/crash.rs`|
|P2G|RECOVERY, TEST_CI_SMOKE|`crates/serea-task-engine/tests/recovery.rs`|
|P2F-b|TASK_ENGINE, TEST_CI_SMOKE|`crates/serea-task-engine/tests/review.rs`|
|P2F-b|TASK_ENGINE, TEST_CI_SMOKE|`crates/serea-task-engine/tests/workflow.rs`|
|P2B|CLOCK_TIME, TEST_CI_SMOKE|`crates/serea-testkit/src/clock.rs`|
|P2B|CLOCK_TIME, TEST_CI_SMOKE|`crates/serea-testkit/tests/fakes_are_deterministic.rs`|
|P2B|CLOCK_TIME, TEST_CI_SMOKE|`crates/serea-testkit/tests/p2b_clock.rs`|
|P2A|ADR_DOC_CLAIMS|`docs/architecture/01-system-overview.md`|
|P2A|ADR_DOC_CLAIMS|`docs/architecture/02-trust-boundaries.md`|
|P2A, P2D|ADR_DOC_CLAIMS|`docs/architecture/03-crate-map.md`|
|P2A|ADR_DOC_CLAIMS|`docs/architecture/04-execution-pipeline.md`|
|P2A|ADR_DOC_CLAIMS|`docs/architecture/README.md`|
|P2A|ADR_DOC_CLAIMS|`docs/decisions/ADR-0018-taskstep-lifecycle-and-field-presence.md`|
|P2A|ADR_DOC_CLAIMS|`docs/decisions/ADR-0019-canonical-json-and-idempotency-preimage.md`|
|P2A|ADR_DOC_CLAIMS|`docs/decisions/ADR-0020-bounds-b3-scope-clarification.md`|
|P2A, P2F-b, P2G|ADR_DOC_CLAIMS|`docs/decisions/ADR-0021-p2-p3-event-atomicity-seam.md`|
|P2A, P2D|ADR_DOC_CLAIMS|`docs/decisions/ADR-0022-durable-private-data-at-rest.md`|
|P2A|ADR_DOC_CLAIMS|`docs/decisions/ADR-0023-text-field-validation-categories.md`|
|P2A, P2E, P2F-a, P2F-b, P2G|ADR_DOC_CLAIMS|`docs/decisions/ADR-0024-lease-fencing-and-commit-under-lease.md`|
|P2A|ADR_DOC_CLAIMS|`docs/decisions/README.md`|
|P2A, P2B, P2C|ADR_DOC_CLAIMS|`docs/plans/P2-6.1-sol-launch.md`|
|P2A|ADR_DOC_CLAIMS|`docs/plans/P2-autonomous-audit.md`|
|P2A, P2B, P2E, P2F-a, P2F-b, P2G|ADR_DOC_CLAIMS|`docs/plans/P2-contract-gap-analysis.md`|
|P2A, P2C, P2D|ADR_DOC_CLAIMS|`docs/plans/P2-sqlite-schema.md`|
|P2A, P2B, P2C, P2D, P2E, P2F-a, P2F-b, P2G|ADR_DOC_CLAIMS|`docs/plans/P2-storage-task-engine.md`|
|P2A, P2B, P2C, P2D, P2E, P2F-a, P2F-b, P2G|ADR_DOC_CLAIMS|`docs/plans/P2-test-matrix.md`|
|P2A, P2B, P2C, P2D, P2E, P2F-a, P2F-b, P2G|ADR_DOC_CLAIMS|`docs/plans/P2-tomorrow-decision-ledger.md`|
|P2A|ADR_DOC_CLAIMS|`docs/plans/P2A-review-and-closure.md`|
|P2B|ADR_DOC_CLAIMS|`docs/plans/P2B-review-and-closure.md`|
|P2C|ADR_DOC_CLAIMS|`docs/plans/P2C-review-and-closure.md`|
|P2D|ADR_DOC_CLAIMS|`docs/plans/P2D-review-and-closure.md`|
|P2E|ADR_DOC_CLAIMS|`docs/plans/P2E-review-and-closure.md`|
|P2F-a|ADR_DOC_CLAIMS|`docs/plans/P2F-review-and-closure.md`|
|P2F-b|ADR_DOC_CLAIMS|`docs/plans/P2F-task-engine-review-and-closure.md`|
|P2F-b|ADR_DOC_CLAIMS|`docs/plans/P2F-task-engine-review-generation-0.md`|
|P2F-b|ADR_DOC_CLAIMS|`docs/plans/P2F-task-engine-review-generation-1.md`|
|P2F-b|ADR_DOC_CLAIMS|`docs/plans/P2F-task-engine-review-resolution.md`|
|P2G|ADR_DOC_CLAIMS|`docs/plans/P2G-review-and-closure.md`|
|P2G|ADR_DOC_CLAIMS|`docs/plans/P2G-review-generation-0.md`|
|P2G|ADR_DOC_CLAIMS|`docs/plans/P2G-review-generation-1.md`|
|P2G|ADR_DOC_CLAIMS|`docs/plans/P2G-review-resolution.md`|
|P2H|ADR_DOC_CLAIMS|`docs/plans/P2H-review-and-closure.md`|
|P2H|ADR_DOC_CLAIMS|`docs/plans/P2H-review-generation-0.md`|
|P2H|ADR_DOC_CLAIMS|`docs/plans/P2H-review-generation-1.md`|
|P2H|ADR_DOC_CLAIMS|`docs/plans/P2H-review-resolution.md`|
|P2A, P2C|ADR_DOC_CLAIMS|`docs/plans/p2a-doc-probes.py`|
|P2A|PROTOCOL_SCHEMA, ADR_DOC_CLAIMS|`docs/protocols/00-protocol-index.md`|
|P2A|PROTOCOL_SCHEMA, ADR_DOC_CLAIMS|`docs/protocols/01-capability-protocol.md`|
|P2A|PROTOCOL_SCHEMA, ADR_DOC_CLAIMS|`docs/protocols/02-task-protocol.md`|
|P2A|PROTOCOL_SCHEMA, ADR_DOC_CLAIMS|`docs/protocols/03-model-protocol.md`|
|P2A|PROTOCOL_SCHEMA, ADR_DOC_CLAIMS|`docs/protocols/06-event-protocol.md`|
|P2A|PROTOCOL_SCHEMA, ADR_DOC_CLAIMS|`docs/protocols/08-goallatch-adapter-protocol.md`|
|P2D|PROTOCOL_SCHEMA, ADR_DOC_CLAIMS|`docs/protocols/09-data-classification-protocol.md`|
|P2A|PROTOCOL_SCHEMA, ADR_DOC_CLAIMS|`docs/protocols/10-bounds-protocol.md`|
|P2C, P2F-b|TEST_CI_SMOKE|`tests/workspace_smoke.py`|
|P2C, P2F-b|TEST_CI_SMOKE|`tests/workspace_smoke_tests.py`|
|P2H|TEST_CI_SMOKE|`tools/prove_release_fault_exclusion.sh`|
|P2A|CANONICAL_VENDOR|`vendor/jsonschema-value/.cargo_vcs_info.json`|
|P2A|CANONICAL_VENDOR|`vendor/jsonschema-value/Cargo.lock`|
|P2A|CANONICAL_VENDOR|`vendor/jsonschema-value/Cargo.toml`|
|P2A|CANONICAL_VENDOR|`vendor/jsonschema-value/Cargo.toml.orig`|
|P2A|CANONICAL_VENDOR|`vendor/jsonschema-value/LICENSE`|
|P2A|CANONICAL_VENDOR|`vendor/jsonschema-value/README.md`|
|P2A|CANONICAL_VENDOR|`vendor/jsonschema-value/README.vendor.md`|
|P2A|CANONICAL_VENDOR|`vendor/jsonschema-value/build.rs`|
|P2A|CANONICAL_VENDOR|`vendor/jsonschema-value/regression-tests/Cargo.lock`|
|P2A|CANONICAL_VENDOR|`vendor/jsonschema-value/regression-tests/Cargo.toml`|
|P2A|CANONICAL_VENDOR|`vendor/jsonschema-value/src/cmp.rs`|
|P2A|CANONICAL_VENDOR|`vendor/jsonschema-value/src/conformance.rs`|
|P2A|CANONICAL_VENDOR|`vendor/jsonschema-value/src/jsonb/encode.rs`|
|P2A|CANONICAL_VENDOR|`vendor/jsonschema-value/src/jsonb/mod.rs`|
|P2A|CANONICAL_VENDOR|`vendor/jsonschema-value/src/lib.rs`|
|P2A|CANONICAL_VENDOR|`vendor/jsonschema-value/src/magnus.rs`|
|P2A|CANONICAL_VENDOR|`vendor/jsonschema-value/src/numeric.rs`|
|P2A|CANONICAL_VENDOR|`vendor/jsonschema-value/src/numeric_check.rs`|
|P2A|CANONICAL_VENDOR|`vendor/jsonschema-value/src/pyo3.rs`|
|P2A|CANONICAL_VENDOR|`vendor/jsonschema-value/src/serde_json.rs`|
|P2A|CANONICAL_VENDOR|`vendor/jsonschema-value/src/serde_number.rs`|
|P2A|CANONICAL_VENDOR|`vendor/jsonschema-value/src/types.rs`|
|P2A|CANONICAL_VENDOR|`vendor/jsonschema-value/src/unique.rs`|
|P2A|CANONICAL_VENDOR|`vendor/jsonschema-value/tests/conformance.rs`|
|P2A|CANONICAL_VENDOR|`vendor/jsonschema-value/tests/fixtures/jsonb-corpus-be.tsv`|
|P2A|CANONICAL_VENDOR|`vendor/jsonschema-value/tests/fixtures/jsonb-corpus-input.jsonl`|
|P2A|CANONICAL_VENDOR|`vendor/jsonschema-value/tests/fixtures/jsonb-corpus-reader-only.jsonl`|
|P2A|CANONICAL_VENDOR|`vendor/jsonschema-value/tests/fixtures/jsonb-corpus.tsv`|
|P2A|CANONICAL_VENDOR|`vendor/jsonschema-value/tests/jsonb.rs`|
|P2A|CANONICAL_VENDOR|`vendor/jsonschema-value/tests/jsonb_live.rs`|
|P2A|CANONICAL_VENDOR|`vendor/jsonschema-value/tests/numeric.rs`|
|P2A|CANONICAL_VENDOR|`vendor/jsonschema-value/tests/serea_numeric.rs`|
|P2A|CANONICAL_VENDOR|`vendor/serde_json/.cargo_vcs_info.json`|
|P2A|CANONICAL_VENDOR|`vendor/serde_json/.github/workflows/ci.yml`|
|P2A|CANONICAL_VENDOR|`vendor/serde_json/.gitignore`|
|P2A|CANONICAL_VENDOR|`vendor/serde_json/CONTRIBUTING.md`|
|P2A|CANONICAL_VENDOR|`vendor/serde_json/Cargo.lock`|
|P2A|CANONICAL_VENDOR|`vendor/serde_json/Cargo.toml`|
|P2A|CANONICAL_VENDOR|`vendor/serde_json/Cargo.toml.orig`|
|P2A|CANONICAL_VENDOR|`vendor/serde_json/LICENSE-APACHE`|
|P2A|CANONICAL_VENDOR|`vendor/serde_json/LICENSE-MIT`|
|P2A|CANONICAL_VENDOR|`vendor/serde_json/README.md`|
|P2A|CANONICAL_VENDOR|`vendor/serde_json/README.vendor.md`|
|P2A|CANONICAL_VENDOR|`vendor/serde_json/build.rs`|
|P2A|CANONICAL_VENDOR|`vendor/serde_json/src/de.rs`|
|P2A|CANONICAL_VENDOR|`vendor/serde_json/src/error.rs`|
|P2A|CANONICAL_VENDOR|`vendor/serde_json/src/io/core.rs`|
|P2A|CANONICAL_VENDOR|`vendor/serde_json/src/io/mod.rs`|
|P2A|CANONICAL_VENDOR|`vendor/serde_json/src/iter.rs`|
|P2A|CANONICAL_VENDOR|`vendor/serde_json/src/lexical/algorithm.rs`|
|P2A|CANONICAL_VENDOR|`vendor/serde_json/src/lexical/bhcomp.rs`|
|P2A|CANONICAL_VENDOR|`vendor/serde_json/src/lexical/bignum.rs`|
|P2A|CANONICAL_VENDOR|`vendor/serde_json/src/lexical/cached.rs`|
|P2A|CANONICAL_VENDOR|`vendor/serde_json/src/lexical/cached_float80.rs`|
|P2A|CANONICAL_VENDOR|`vendor/serde_json/src/lexical/digit.rs`|
|P2A|CANONICAL_VENDOR|`vendor/serde_json/src/lexical/errors.rs`|
|P2A|CANONICAL_VENDOR|`vendor/serde_json/src/lexical/exponent.rs`|
|P2A|CANONICAL_VENDOR|`vendor/serde_json/src/lexical/float.rs`|
|P2A|CANONICAL_VENDOR|`vendor/serde_json/src/lexical/large_powers.rs`|
|P2A|CANONICAL_VENDOR|`vendor/serde_json/src/lexical/large_powers32.rs`|
|P2A|CANONICAL_VENDOR|`vendor/serde_json/src/lexical/large_powers64.rs`|
|P2A|CANONICAL_VENDOR|`vendor/serde_json/src/lexical/math.rs`|
|P2A|CANONICAL_VENDOR|`vendor/serde_json/src/lexical/mod.rs`|
|P2A|CANONICAL_VENDOR|`vendor/serde_json/src/lexical/num.rs`|
|P2A|CANONICAL_VENDOR|`vendor/serde_json/src/lexical/parse.rs`|
|P2A|CANONICAL_VENDOR|`vendor/serde_json/src/lexical/rounding.rs`|
|P2A|CANONICAL_VENDOR|`vendor/serde_json/src/lexical/shift.rs`|
|P2A|CANONICAL_VENDOR|`vendor/serde_json/src/lexical/small_powers.rs`|
|P2A|CANONICAL_VENDOR|`vendor/serde_json/src/lib.rs`|
|P2A|CANONICAL_VENDOR|`vendor/serde_json/src/macros.rs`|
|P2A|CANONICAL_VENDOR|`vendor/serde_json/src/map.rs`|
|P2A|CANONICAL_VENDOR|`vendor/serde_json/src/number.rs`|
|P2A|CANONICAL_VENDOR|`vendor/serde_json/src/raw.rs`|
|P2A|CANONICAL_VENDOR|`vendor/serde_json/src/read.rs`|
|P2A|CANONICAL_VENDOR|`vendor/serde_json/src/ser.rs`|
|P2A|CANONICAL_VENDOR|`vendor/serde_json/src/value/de.rs`|
|P2A|CANONICAL_VENDOR|`vendor/serde_json/src/value/from.rs`|
|P2A|CANONICAL_VENDOR|`vendor/serde_json/src/value/index.rs`|
|P2A|CANONICAL_VENDOR|`vendor/serde_json/src/value/mod.rs`|
|P2A|CANONICAL_VENDOR|`vendor/serde_json/src/value/partial_eq.rs`|
|P2A|CANONICAL_VENDOR|`vendor/serde_json/src/value/ser.rs`|
|P2A|CANONICAL_VENDOR|`vendor/serde_json/tests/compiletest.rs`|
|P2A|CANONICAL_VENDOR|`vendor/serde_json/tests/debug.rs`|
|P2A|CANONICAL_VENDOR|`vendor/serde_json/tests/lexical.rs`|
|P2A|CANONICAL_VENDOR|`vendor/serde_json/tests/lexical/algorithm.rs`|
|P2A|CANONICAL_VENDOR|`vendor/serde_json/tests/lexical/exponent.rs`|
|P2A|CANONICAL_VENDOR|`vendor/serde_json/tests/lexical/float.rs`|
|P2A|CANONICAL_VENDOR|`vendor/serde_json/tests/lexical/math.rs`|
|P2A|CANONICAL_VENDOR|`vendor/serde_json/tests/lexical/num.rs`|
|P2A|CANONICAL_VENDOR|`vendor/serde_json/tests/lexical/parse.rs`|
|P2A|CANONICAL_VENDOR|`vendor/serde_json/tests/lexical/rounding.rs`|
|P2A|CANONICAL_VENDOR|`vendor/serde_json/tests/macros/mod.rs`|
|P2A|CANONICAL_VENDOR|`vendor/serde_json/tests/map.rs`|
|P2A|CANONICAL_VENDOR|`vendor/serde_json/tests/regression.rs`|
|P2A|CANONICAL_VENDOR|`vendor/serde_json/tests/regression/issue1004.rs`|
|P2A|CANONICAL_VENDOR|`vendor/serde_json/tests/regression/issue520.rs`|
|P2A|CANONICAL_VENDOR|`vendor/serde_json/tests/regression/issue795.rs`|
|P2A|CANONICAL_VENDOR|`vendor/serde_json/tests/regression/issue845.rs`|
|P2A|CANONICAL_VENDOR|`vendor/serde_json/tests/regression/issue953.rs`|
|P2A|CANONICAL_VENDOR|`vendor/serde_json/tests/regression/issue979.rs`|
|P2A|CANONICAL_VENDOR|`vendor/serde_json/tests/stream.rs`|
|P2A|CANONICAL_VENDOR|`vendor/serde_json/tests/test.rs`|
|P2A|CANONICAL_VENDOR|`vendor/serde_json/tests/ui/missing_colon.rs`|
|P2A|CANONICAL_VENDOR|`vendor/serde_json/tests/ui/missing_colon.stderr`|
|P2A|CANONICAL_VENDOR|`vendor/serde_json/tests/ui/missing_comma.rs`|
|P2A|CANONICAL_VENDOR|`vendor/serde_json/tests/ui/missing_comma.stderr`|
|P2A|CANONICAL_VENDOR|`vendor/serde_json/tests/ui/missing_value.rs`|
|P2A|CANONICAL_VENDOR|`vendor/serde_json/tests/ui/missing_value.stderr`|
|P2A|CANONICAL_VENDOR|`vendor/serde_json/tests/ui/not_found.rs`|
|P2A|CANONICAL_VENDOR|`vendor/serde_json/tests/ui/not_found.stderr`|
|P2A|CANONICAL_VENDOR|`vendor/serde_json/tests/ui/parse_expr.rs`|
|P2A|CANONICAL_VENDOR|`vendor/serde_json/tests/ui/parse_expr.stderr`|
|P2A|CANONICAL_VENDOR|`vendor/serde_json/tests/ui/parse_key.rs`|
|P2A|CANONICAL_VENDOR|`vendor/serde_json/tests/ui/parse_key.stderr`|
|P2A|CANONICAL_VENDOR|`vendor/serde_json/tests/ui/unexpected_after_array_element.rs`|
|P2A|CANONICAL_VENDOR|`vendor/serde_json/tests/ui/unexpected_after_array_element.stderr`|
|P2A|CANONICAL_VENDOR|`vendor/serde_json/tests/ui/unexpected_after_map_entry.rs`|
|P2A|CANONICAL_VENDOR|`vendor/serde_json/tests/ui/unexpected_after_map_entry.stderr`|
|P2A|CANONICAL_VENDOR|`vendor/serde_json/tests/ui/unexpected_colon.rs`|
|P2A|CANONICAL_VENDOR|`vendor/serde_json/tests/ui/unexpected_colon.stderr`|
|P2A|CANONICAL_VENDOR|`vendor/serde_json/tests/ui/unexpected_comma.rs`|
|P2A|CANONICAL_VENDOR|`vendor/serde_json/tests/ui/unexpected_comma.stderr`|
