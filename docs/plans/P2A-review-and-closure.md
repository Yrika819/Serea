# P2A Review and Closure Gate

- Date: 2026-10-03
- Scope: final P2A contract implementation and integration record on `p2/p2a-protocol-corrections`, from checkpoint `484672227c7694ba7df110e05abb1cbccdcc1eed`. Earlier docs-only subagent evidence is retained in its original scope; this record now belongs to the coordinating implementation run.
- Initial evidence: findings from **three independent subagent reviewers under the coordinator**, frozen before disposition as G01–G19 below. Initial evidence IDs: `PASSA3973237e`, `PASSBfec63a0c`, `PASSCcf67af61`. Findings are pooled; individual finding attribution is not recorded and is not invented.
- Corrected documentation gate: **three independent subagent re-reviews GREEN**, performed under the coordinator and frozen inline in §4: `A3973237e`, `Bfec63a0c`, `Ccf67af61`. No unresolved documentation majors in that gate. Owner direction ratified the design contingent on GREEN; no user-supplied test results are claimed.
- Production state: **P2A implemented, independent final reviews GREEN, workspace tests 307/307 PASS** on stable and Rust 1.85. Original F1–F6 and numeric/object follow-ups N1/N3 are resolved; N2 error precedence is intentionally documented/tested. Actual final command evidence and limitations are in §4.7; no storage/task-engine/event runtime is delivered.
- Ratified current frozen registry: architecture `serea-arch/1.0.0`, task `serea.task/2`, action `serea.action/2`, event `serea.event/1`, envelope version `1`, other surface majors `1`, MSRV `1.85`. The `/1` task/action implementation is historical P0/P1 baseline, not current registry.
- Accepted: ADR-0018 wire/lifecycle architecture (runtime deferred), ADR-0019 implemented SCJ-1 + IDK-1, ADR-0020 semantic B3 clarification, ADR-0023 complete validation. ADR-0021/22/24 remain Proposed runtime; only ADR-0024's wire generation member/validation is implemented.

## 1. Read set and precedence

Read the [launch](P2-6.1-sol-launch.md), [design](P2-storage-task-engine.md),
[gap analysis](P2-contract-gap-analysis.md), [DDL](P2-sqlite-schema.md),
[test matrix](P2-test-matrix.md), [audit](P2-autonomous-audit.md),
[decision ledger](P2-tomorrow-decision-ledger.md), ADR-0018 through ADR-0024,
and their owning contracts: protocol index, capability, task, model, approval,
event, bounds, data classification and scheduler, with matching Rust/schema and
architecture/trust-boundary context. This gate reconciles
current guidance; dated audit experiments and historical P0/P1 evidence remain
historical evidence, not proof of corrected implementation.

## 2. Frozen findings and disposition

| ID | Frozen finding | Disposition / acceptance criterion |
| --- | --- | --- |
| G01 | Five Option conversions and eight unconditional fields are wrong | Four conversions only: `idempotency_key`, `result_digest`, `started_at`, `completed_at`. `input_digest` required; provider/capability/version already Option. Seven unconditional fields: `step_id`, `task_id`, `sequence`, `kind`, `status`, `attempt`, `input_digest`. ADR-0018 and all current launch/design/ledger claims corrected. |
| G02 | None/generation boundary unspecified | Missing or null accepts None; serializer omits None. SQL generation 0 maps to wire None; positive generation maps to Some(u32), retained after a lease ends; wire zero refused; overflow refused without wrapping/clamping. Storage mapping tests deferred, wire tests P2A. |
| G03 | Open wire status accidentally narrowed | Unknown well-formed status parses and round-trips. Known-status presence validation only; kind invariants and supplied-value validation always, including non-capability receipt absence on ALL statuses, unknown codes included. Unknown engine execution blocking is P2F, not P2A. |
| G04 | Action/2 declared before its primitives | SCJ-1, digest, duplicate-aware parser, IDK-1 and `sha2 = { version = "0.11.0", default-features = false }` all move atomically to P2A. Clock/TestClock alone remain P2B. |
| G05 | Inventory omits affected surfaces | Include workspace and protocol Cargo manifests/lockfile, canonical module/re-exports, protocol manifest/registry, canonical vector/property tests, event schema, every nested `provider_reference`, and both migration notes. Text validators belong in `types.rs`, not `schema.rs`; schema code remains in scope for registry embedding/validation as needed. |
| G06 | Mixed majors treated globally | Per-surface registry accepts task/action 2 and unchanged other surfaces 1; envelope_version stays 1. Reject unsupported majors per surface, never silently downgrade. |
| G07 | IDK prose frames values only | 21-byte domain tag, u8 field count 5, then ordered **names AND values**, each name and each value length-prefixed u64 big-endian. Pin 282-byte vector-1 preimage. Encoding is injective; SHA-256 is not mathematically injective. |
| G08 | Collision corpus calls invalid IDs/scalars legal actions | `p.r.list` is invalid: provider/resource segments require 2–32 characters. Retain A/B hashes only in low-level private raw-string framing tests; typed public derivation rejects that ID and accepts any SCJ-1 root with valid typed identifiers. Add independently derived `pp.rr.list` generic scalar regression and legal object-root tests. SCJ-1 admits scalar roots; ActionRequest arguments remain object-root. Do not assert a demonstrated collision between legal ActionRequests. |
| G09 | Key collision claimed to transfer approval | Removed. Approval validates argument digest, capability and pinned version independently; key equality cannot authorize B. Different StepIds also affect the preimage, so the scalar pair is not a demonstrated two-step unique-index collision. |
| G10 | Text entry point claimed absolute provenance | Reject duplicates in original text before constructing Value, including nested/escaped-name duplicates. A string API cannot reconstruct already discarded duplicates or prove raw provenance. Raw caller boundary obligation is explicit. |
| G11 | No-fractions claim contradicts model | ModelRequest.temperature is f64 (wire example 0.2). SCJ-1 is an explicit integer-only limited domain, not a model-wire change or automatic coverage of every blob/model input. Future runtime digest paths reject noncanonical model documents; fractional support needs a future decision, never truncation. |
| G12 | P regex refuses its promised LF/TAB; O grammars are inexact | Exact patterns in ADR-0023. Preserve C1 refusals; L/O reject U+2028/U+2029. P permits interior LF/TAB/U+2028/U+2029, rejects CR, other C0, DEL and C1. Exact idk_/sha256: spelling, ULID [0-7] plus 25 Crockford chars, exact goallatch provider subtraction. |
| G13 | Rust/schema whitespace diverges | Pin identical Unicode White_Space list, including U+0085, in both implementations. Every category rejects leading/trailing whitespace and empty input. Exhaustive code-point and near-miss corpus, including every provider_reference occurrence. Event/1 O is tightened only: C1 is not newly admitted. |
| G14 | B3 called patch and resource gap closed | ADR-0020 is semantic clarification, architecture-minor, not patch. No resource bounds introduced; numeric payload/blob/attachment/count/decompression limits remain open. |
| G15 | Proposed/Accepted and same-commit governance contradict phases | After the corrected docs gate GREEN and owner ratification: Accepted 0018 wire/lifecycle decision (runtime deferred), 0019 implemented full primitives, 0020 clarification, 0023 implemented full validation. Proposed: 0021/22/24 runtime deferred. 0024 wire member belongs to P2A; full fencing is not accepted or implemented. Phase-specific gates replace promises to ship storage in P2A. |
| G16 | error=N SQL admits partial error/details | Require every error column, including optional details, NULL outside FAILED; FAILED requires five mandatory error members and allows absent details. Correct DDL only, no migration/runtime. Add single-column and partial-tuple regressions. |
| G17 | Participant sketch runs on failed body and retains backfill contradictions | Shared receiver `record(&self, tx, transition)`; successful body returns immutable transition(s), propagated with `?` **before** participants; same transaction, then commit. Failed body or participant rolls back; no journal on failure/no-op. No event_seq/backfill; P2 pending count is journal row count, not work queue. |
| G18 | Step-copy-only fence permits commit after release | Every outcome predicate also EXISTS authoritative leases row matching step, owner, generation and unreleased state. Release permanently revokes generation. Chosen policy: expired but unreclaimed/unreleased lease may commit known result; cannot renew or begin a new attempt; reclaim changes generation and then refuses old commit. No expiry-at-commit requirement or race-prone precheck. |
| G19 | begin_attempt consumes only nonclone guard | begin_attempt borrows `&LeaseGuard`; successful call leaves same guard for consuming outcome commit or release. No Clone/Copy; runtime tests deferred. |

## 3. Atomic P2A production inventory

Current implementation inventory, checked against the working tree and read-only
source; this is not a requirement to touch every inspected file:

- Workspace `Cargo.toml`, `Cargo.lock`, `crates/serea-protocol/Cargo.toml`: sha2 0.11 without defaults; serde_json raw_value and jsonschema arbitrary-precision; exact serde_json 1.0.151/jsonschema 0.58.3 pins; limited numeric/object-preservation vendor patches. MSRV remains 1.85. `vendor/serde_json` and `vendor/jsonschema-value` are excluded from workspace membership; original licenses/tests/source retained, optional upstream adapters are not enabled Serea runtime.
- Changed `crates/serea-protocol/src/{types,errors,ids,lib}.rs`, new `canonical.rs`: four Option conversions, `TaskStepDraft` → private checked `TaskStep`/`StepPresence` with no public mutation bypass, reserved-extension-key refusal, exact text categories, canonical primitives, typed derivation and mixed-major registry/manifest. `crates/serea-protocol/src/schema.rs`: version documentation only; embedding/validation unchanged. It retains five schema embeddings and is not the owner of text validators.
- **Four schemas changed**: `assistant-task.schema.json`, `action-request.schema.json`, `action-result.schema.json`, `event.schema.json`. Event actor O and nested receipt `provider_reference` are covered. `envelope.schema.json` is inspected/embedded but **unchanged**, not a fifth changed file. Envelope stays version 1; Rust dispatch validates per-surface majors. Task-schema kind constraints refuse non-capability receipts even on unknown status, matching Rust.
- Updated `crates/serea-protocol/tests/{protocol_types,schema_contracts}.rs`; added `canonical_vectors.rs`, `canonical_properties.rs`, `p2a_types.rs`, `p2a_shape.rs`, `p2a_parity.rs`, `p2a_schema_red.rs`, `text_parity.rs`, `json_value_preservation.rs`. Inline test documents cover parity both ways, independently expected accepts/refusals, unknown wire status, missing/null/omission, receipt/kind restrictions, reserved extensions, generation boundaries, duplicate-aware parsing, integer domain, named framing, typed invalid-ID rejection, valid-ID generic scalar/other SCJ-1 roots and legal object arguments; ActionRequest separately refuses nonobjects. Existing `tests/ids.rs` is **unchanged** and remains current identifier regression coverage. `fixtures/` remains empty: no fixture file was created.
- Protocol manifest/registry and version constants wherever currently embedded; update source/docs and contract snapshots atomically rather than adding a parallel naming authority. The registry/dispatch covers all 11 published wire surfaces, including `serea.scheduler/1`; this implements no scheduler runtime.
- Protocol index, task/capability/bounds/event contract annotations and changelogs, ADR headers/index, current architecture registry and two consumer migration notes in the launch package. Data-at-rest and event-participant runtime amendments remain later-phase Proposed decisions.

Do not ship action/2 while canonical implementation is still deferred. No storage,
engine, event bus, provider/model execution, SQL production migration, or Clock
implementation belongs to P2A. Source/schema/versions/migrations/tests/dependencies
and this record ship together in one atomic P2A commit; no preliminary docs commit
was made, no checkpoint was amended and no push is part of this run.

## 4. Governance and closure evidence

### 4.1 Frozen corrected documentation re-reviews

| Reviewer evidence ID | Independent subagent corrected-doc result | Unresolved majors |
| --- | --- | --- |
| `A3973237e` | **GREEN** | None |
| `Bfec63a0c` | **GREEN** | None |
| `Ccf67af61` | **GREEN** | None |

Independent subagents under the coordinator performed all three re-reviews after
the initial G01–G19 corrections. They are frozen here independently of the initial
pooled findings; this docs agent did not perform them and invents no per-finding
attribution. Owner direction ratified the design contingent on this GREEN gate,
accepting ADR-0018/19/20/23 within the scopes in the header; the owner did not
supply or report test results. This does not accept deferred ADR-0018 runtime or
ADR-0021/22/24 runtime, or substitute for parent tool-run validation.

### 4.2 Historical parent tool-run RED → scoped GREEN evidence

| Evidence | Parent tool-run result / disposition | Limitation |
| --- | --- | --- |
| First Rust planned-step shape RED | **Four E0308 type errors** from the four required-to-Option conversions | Real compile RED, not an invented assertion failure |
| First schema planned-step counterpart | **FAILED / RED** under the old required-field schema | Separate schema-side RED |
| Canonical naive-framing canary | **RED**, then corrected named IDK-1 framing | Historical invalid-ID raw vectors retained privately; not a legal ActionRequest collision |
| Integration receipt parity | Rust non-capability restriction versus schema receipt mismatch found and **corrected by parent** | Receipt absence is now unconditional by kind, including unknown status |
| MSRV let-chain integration issue | **Corrected by parent** for Rust 1.85 compatibility | Correction is not proof that the full MSRV gate has passed |
| Scoped canonical tests | **GREEN: 63** | Parent handoff, not rerun by this docs agent |
| Scoped type tests | **GREEN: 104** | Parent handoff, not whole-workspace closure |
| Scoped schema tests | **GREEN: 63** | Parent handoff, not full MSRV validation |

### 4.3 Historical 289-test remediation checkpoint (superseded by §4.7)

The coordinator actually ran the following after the original F1–F6 fixes,
before N1/N3 follow-ups. The PENDING entries below describe that historical
checkpoint, not current final integration state. Final-tree results supersede
them in §4.7; these rows are retained rather than erasing the evidence timeline.

| Command / evidence | Actual result | Scope / limitation |
| --- | --- | --- |
| `cargo fmt --all -- --check` | **PASS** | Current parent implementation |
| `cargo check --workspace --all-targets --offline` | **PASS** | Current parent implementation |
| `cargo test --workspace --all-targets --offline` | **PASS: 289/289** | Post-remediation workspace |
| `cargo test --workspace --all-features --offline` | **PASS: 289/289** | Post-remediation workspace; workspace all-features alone does not establish consumer `serde_json/arbitrary_precision` unification |
| `cargo clippy --workspace --all-targets --all-features --offline -- -D warnings` | **PASS** | Post-remediation workspace |
| Rust 1.85 all-targets before remediation | **PASS: 281/281** | Earlier implementation baseline, not the final 289-test tree |
| Rust 1.85 targeted tests after remediation | **PASS: 110/110** | Targeted remediation coverage, not final all-targets validation |
| Final Rust 1.85 all-targets | **PENDING** | Parent running next; no all-289 MSRV PASS recorded |
| `python3 tests/workspace_smoke.py` | **PENDING** | Final integration smoke |
| Final bounded regression review | **PENDING** | Remediation is not a completed final re-review |
| Final Python/docs/metadata checks and `git diff --check` | **PENDING parent closure** | This docs pass's validator/diff results are separate in §6 |
| One final atomic P2A commit | **PENDING** | Source/schema/manifest/version/tests/docs together; no docs-only commit |

The earlier Rust 1.85 let-chain issue is fixed, not an ongoing MSRV failure.
The final 307-test MSRV result in §4.7, not the earlier 281/110/289 results,
establishes current MSRV validation.

This docs run's actual validation is recorded in §6. Historical
69/69 constructibility, 32-cell and 34-vector evidence is retained with its original
scope; it does not cover new partial-error/released-lease regressions or establish
production closure.

### 4.4 Frozen implementation review and remediation dispositions

| Gate | Frozen evidence | Current disposition / verification boundary |
| --- | --- | --- |
| G20 | `tmp/reviews/2026-10-03-code-review-report-f6962265.md` — report `cr-20261003-f6962265`, **4 Major / 2 Minor**, initial recommendation Changes requested | Independent contract/security/regression subagents under the coordinator; the persisted report records the adjudicated review, not a fresh artifact-writing review. The report and G01–G19 remain unchanged. Remediations below were implemented at this historical checkpoint; final post-follow-up independent reviews are recorded separately in §4.7. |

| Finding | Frozen severity / issue | Remediation status |
| --- | --- | --- |
| F1 | Major — integral generation spelling parity | **PARENT FIXED:** Rust storage remains `Option<u32>`; wire generation accepts JSON integer-valued numeric spellings such as `1`, `1.0`, `1e0` within the positive u32 domain. Missing/null remain None; zero/fraction/overflow refuse. This wire domain is separate from SCJ-1, which still refuses decimal/exponent spellings. |
| F2 | Major — scheduler/1 missing from dispatch | **PARENT FIXED:** registry/dispatch now includes all 11 published wire surfaces, including scheduler/1. No scheduler runtime delivered. |
| F3 | Major — rejected draft bytes in decode errors | **PARENT FIXED:** manual `TaskStep::deserialize` maps `TaskStepDraft` decode failure to `invalid task step draft` before `StepPresence::new`, without formatting the rejected draft error. This is the checked wire boundary, not a claim that unchecked draft errors are independently sanitized. |
| F4 | Major — feature-dependent SCJ-1 admission | **PARENT FIXED:** raw lexical numeric preflight runs before the duplicate-aware visitor, preserving the SCJ-1 numeric domain even with consumer-unified `serde_json/arbitrary_precision`. No decimal/exponent/negative-zero admission or synthetic-number-map reinterpretation. |
| F5 | Minor — terminal example lacks generation | **PARENT FIXED:** current Task Protocol SUCCEEDED example already contains `lease_generation: 1`; not re-edited by this docs cleanup. |
| F6 | Minor — changed schema.rs called unchanged | **DOCS FIXED:** current launch/gate inventories now say version documentation only; embedding/validation unchanged. Four JSON schemas changed; the fifth, envelope, remains unchanged. |

These are historical remediation dispositions, not a rewrite of the initial
report. Later numeric and object-preservation defects were frozen separately,
then corrected and independently reviewed. No runtime scope was added.

### 4.5 Frozen final numeric follow-up (before further remediation)

- **N1 Major:** f64 generation decoding rounded nonintegral JSON numbers (for example `1.00000000000000000001`) into positive u32 values. Exact wire membership, not Rust/schema agreement alone, is required. Four added regression tests were RED. Field-local RawValue decoding and precision-enabled schema validation correct rounding, but expose an upstream checked-arithmetic defect: `1.0e-9223372036854775807` reaches `-i64::MIN`; related exponent subtraction also overflows. Frozen pre-vendor-patch evidence: **291 PASS / 2 FAIL out of 293**, stable and Rust 1.85. This historical RED is not current final status. Remediation must preserve mathematically integral decimal/exponent spellings and direct schema validation, without float truncation, panic masking or removed tests.
- **N2 diagnostic precedence:** canonical numeric lexical preflight precedes duplicate-aware parsing. A duplicate document whose value contains a forbidden numeric spelling can return NonInteger (or InvalidJson for malformed numeric syntax), not DuplicateKey. All such inputs still refuse; no acceptance or authority bypass. **Intentional disposition:** preflight-first error precedence is documented and tested; duplicate detection is not universally the first diagnostic.
- This follow-up leaves the frozen G20/F1–F6 report untouched. A minimal offline source patch to `jsonschema-value` is justified only for exact integer classification and checked conversion arithmetic needed by existing bounded wire integers. It does not claim exact unrestricted schema arithmetic or introduce storage/runtime.

### 4.6 Frozen collateral JSON follow-up

Fresh independent bounded regression report `tmp/reviews/2026-10-03-user-visible-regression-report-47586732.md` found **one Major / Block**: newly enabled serde_json precision/raw features reinterpret literal private marker keys in ordinary payloads and opaque extensions. A separate security reviewer reproduced raw JSON and `from_value::<Trace>` transformations (`{"future":{"$serde_json::private::Number":"1"}}` became `{"future":1}`), plus changed schema verdicts. All original F1–F6 and exact-generation N1 paths were otherwise resolved; N2 precedence is intentional.

This is a distinct collateral failure, **N3**, not a reopening of canonical F4: the custom canonical visitor already preserves these objects. The fix must not reserve legal keys, disable precision or replace raw-input tests with constructor tests. A tested scratch prototype carries internal marker keys as Serde newtypes; literal string keys stay literal through raw parsing, Value replay and derive-flatten Content buffering. At this freeze production adoption and full stable/MSRV validation were still required. Both were subsequently completed; see §4.7. Frozen reports remain immutable.

### 4.7 Final integration verification and independent closure

Final-tree results below were executed by the coordinator after N1/N3 adoption,
not supplied by the owner and not inferred from a subagent's confidence. All
193 baseline test declarations remain in their original files; a Counter-based
source comparison retains duplicate function names in separate modules. The
first two retention-probe attempts incorrectly deduplicated those names (189),
then the corrected probe observed **193 retained, zero missing**. Workspace
`--list` independently reports **307 tests: baseline 193 + 114 added**.

| Required command | Final result |
| --- | --- |
| `cargo fmt --all -- --check` | PASS |
| `cargo check --workspace --all-targets --offline` | PASS |
| `cargo test --workspace --all-targets --offline` | PASS: 307/307, zero failed/ignored |
| `cargo test --workspace --all-features --offline` | PASS: 307/307, zero failed/ignored; doc-tests also PASS |
| `cargo clippy --workspace --all-targets --all-features --offline -- -D warnings` | PASS |
| `python3 tests/workspace_smoke.py` | PASS: crate-map layering |
| `python3 -m py_compile tools/validate_docs.py` | PASS |
| `python3 tools/validate_docs.py docs` | PASS: 55 Markdown files |
| `git diff --check` | PASS; staged whitespace checked again before commit |
| `cargo metadata --no-deps --format-version 1` | Inspected: exactly serea-protocol + serea-testkit, both rust-version 1.85; no P2 runtime crate |

Additional executed evidence:

- `cargo +1.85.0 check --workspace --all-targets --offline --locked`: PASS.
- `cargo +1.85.0 test --workspace --all-targets --all-features --offline --locked`: **307/307 PASS**.
- Stable and Rust 1.85 `--release` protocol parity + JSON-preservation suites: **23/23 PASS each**. Earlier release compilation timeouts were superseded by these bounded 600-second runs, not hidden or claimed as passes.
- Separate numeric-helper harness: **4/4 PASS** on stable/Rust 1.85, debug/release, using the same local serde_json patch; upstream test suites remain preserved but not wholly executed.
- Cargo dependency tree: one local serde_json 1.0.151 with raw_value/arbitrary_precision; jsonschema 0.58.3 precision and local jsonschema-value 0.58.3; sha2 0.11 without defaults. No resolver/TLS/SQLite/runtime feature introduced.
- Documentation probes re-executed: **10,119,638** regex assertions, **531** in-memory SQL design probes, **21** reconstructed vectors PASS. Their exclusions in §6 remain unchanged.
- Final canonical code-review report validator: **PASS**, zero findings/gaps and ten coverage areas.

The initial manifest snapshot test failed after the exact jsonschema pin, and
six documentation anchors failed after a heading changed. Both were repaired
without weakening resolver checks or document validation. A combined final Cargo
run timed out while competing with a release build for the shared target lock;
the subsequently serialized final command chain passed. None of these partial
runs is represented as final success.

| Independent gate | Frozen final artifact | Result / coverage limit |
| --- | --- | --- |
| Contract/security delta review, different session from patch implementer | `tmp/reviews/2026-10-03-code-review-report-99abda3c.md` | **PASS**, zero findings/gaps; 120 passing executions / 106 unique focused tests; no broader optional-format certification |
| Bounded post-N3 regression review, same independent reviewer who found N3 | `tmp/reviews/2026-10-03-user-visible-regression-report-282e6ebc.md` | **PASS**, Block/Discuss/Watch 0; 117 passing focused executions; all affected shared carriers traced, not each independently runtime-fixtured |

Review interruptions were resumed and completed before these artifacts were
accepted. The frozen original reports and their Blocks remain immutable; current
GREEN follows causal remediation, not report rewriting. Original independent
code/contract, security and regression/scope passes covered the P2A implementation;
these final reviews are deliberately limited to remediations and affected chains.

N1: exact RawValue generation + exact schema integer predicate and checked
conversion patch. N2: intentional preflight-first diagnostics, three regression
tests. N3: provenance-aware synthetic keys and exact numeric replay in three
serde_json source files, ten literal/general-JSON regression tests. No panic
catching, rounded admission, reserved marker namespace or lost unknown field.

The vendor changes are limited and required to make the promised contract
constructibly correct. Their notes record provenance, licenses, compatibility
limits and removal criteria. They do **not** certify unrestricted schema numeric
arithmetic, arbitrary custom Serde deserializers or optional upstream adapters.
SCJ-1/IDK-1 durable bytes use UTF-8 and u64 big-endian framing, never native-endian
struct/usize persistence. **Apple Silicon empirical validation is not claimed**;
that remains future CI proof, not a storage/task-engine redesign requirement.

Atomic delivery: this record ships with the sole `feat: implement P2A protocol
corrections` commit whose first parent is the untouched checkpoint. The exact
commit hash is read from Git after committing rather than embedded recursively
in its own contents. No preliminary docs commit, amend, push or P2B start.

## 5. Unresolved / deliberately deferred

- No unresolved P2A contract/code/security finding remains. Final integration evidence is in §4.7; the following are deliberate later-phase deferrals, not missing P2A runtime.
- P2B Clock/time; P2C–P2G SQL mappings, participant ordering, lease revocation/fencing, engine unknown execution and recovery: no runtime delivered here.
- Fractional canonicalization decision for model documents and future capabilities; model wire remains unchanged and noncanonical digest inputs must be refused.
- Real PRIVATE at-rest backend/key custody and SECRET sealed-store ownership; resource bounds and empirical cross-architecture CI remain open as previously recorded.
- E3/E4 never claimed for P2-era history; P3 forward-only participant semantics do not repair historical debt.

## 6. Documentation validation

Executed on 2026-10-03 on `p2/p2a-protocol-corrections`:

| Command / evidence | Result | Scope / limitation |
| --- | --- | --- |
| `python3 docs/plans/p2a-doc-probes.py` — Node 24.21.0 | **PASS: 10,119,638 regex assertions** | Published fragments expanded from ADR-0023; every Unicode scalar at leading/interior/trailing positions in O/L/P, all 25 pinned whitespace code points, exact ULID/idk_/sha256:/CapabilityId exclusions and near misses; independent expected predicates. Not production Rust/schema parity or field-occurrence coverage |
| Same command — Python SQLite 3.43.2 | **PASS: 531 probes** | Verbatim documented migration; 56 kind/status cases, all 64 error-column subsets for each of seven statuses, step u32 boundaries, all three published leases DDL blocks, six authoritative outcome scenarios and max-generation acquisition refusal. Live/expired-unreclaimed outcomes accept; released/reclaimed/wrong-owner/missing-authority outcomes refuse; release cannot be repeated and expired/released renewal refuses |
| Same command — Python SHA-256 / u64 big-endian framing | **PASS: 21 published vectors** | Ten SCJ-1 vectors, seven historical IDK-1 vectors (A/B private raw framing only), four valid-ID scalar/object vectors. Vector-1 preimage 282 bytes; pp.rr.list scalar/object preimages 244/250 bytes. Not proof of public Rust API behavior, canonical parser completeness, legal-action collision or SHA-256 injectivity |
| `python3 tools/validate_docs.py docs` | **PASS: 55 Markdown files** | Rerun in this final focused docs cleanup; identifiers, JSON, placeholders and cross-references |
| `git --no-pager diff --check` | **PASS** | Rerun in this final focused docs cleanup; whole current tracked diff whitespace check, not a claim that the parent working tree contains only docs |

The first probe attempts found two harness mistakes (omitted U+2009/U+200A in
the independent whitespace list; vector-table header parsed as JSON). Both were
corrected; the complete rerun passed. No documented hash or regex was changed to
force a passing probe.

SQLite evidence is an in-memory **documentation experiment**, not production
storage or bundled-SQLite verification. It does not prove Rust guards, borrowed
begin_attempt, checked wire/SQL mapping, transaction participant ordering,
failed/reconciled outcome implementations, receipt/journal rollback, cross-process
races, or duplicate-aware parsing. Those runtime tests remain later-phase work.
Historical scoped Cargo results are in §4.2/§4.3; actual final integration command
results are separately in §4.7. No production storage, runtime/CI or empirical
Apple Silicon validation is claimed.

## 7. Documentation corrections completed / final handoff

The previous stop-point's six documentation items are resolved:

- The audit has a top corrigendum for conversion/field counts, semantic B3 minor,
  model fractions, superseded patterns/corpus counts and historical evidence scope;
  the original observations below it are preserved.
- Current ADR/design/DDL/launch/ledger prose no longer treats historical 32-cell
  or 69-check success as exhaustive partial-error/release/overflow coverage.
- Both repeated leases DDL blocks, ADR-0024's DDL and the step column have bounded
  u32 generation CHECKs; private LeaseGuard generation is u32. SQL 0/wire None and
  positive retained generations remain explicit; overflow refuses and rolls back.
- Phase-specific gates replace all-runtime P2A delivery promises. ADR-0021/22/24
  remain Proposed for deferred runtime; 0024's wire member is P2A only. The
  subsequent corrected-doc GREEN and owner ratification accept 0018/19/20/23
  only in the explicitly stated scopes.
- Published O/L/P fragments have a mathematical explanation and executable Node
  checks with independent pinned-whitespace and exact-identifier expectations;
  documented DDL partial-error/release/overflow cases and framing vectors were
  checked separately. Evidence and exclusions are in §6.
- Historical raw p.r.list A/B vectors are private low-level framing tests only;
  typed generic derivation with valid IDs permits any SCJ-1 root. ActionRequest
  continues to require object arguments.

**Unresolved documentation findings: none.** Three independent corrected-doc
re-reviews are frozen in §4.1; independent implementation reviews, follow-up
findings and separate final GREEN reviews are preserved in §4.4–§4.7. Tool-run
validation is distinguished from owner direction and review conclusions.

This record accompanies implementation in **one P2A integration commit** from the
untouched checkpoint, not a preliminary docs commit. No runtime or P2B work is
delivered here; deferrals remain in §5. STOP after committed clean P2A.
