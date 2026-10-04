# P2D Blob Classification Review and Closure

## 1. Preflight and frozen pre-implementation design gate

Starting branch `p2/p2c-storage-foundation`, clean worktree, exact HEAD
`1d4519e36155eb0fa295354e2891782ed845a63e`
(`feat: implement P2C storage foundation`). Stable and Rust 1.85 offline
workspace all-feature baselines both passed **402 regular + 10 doctests = 412**.
Created `p2/p2d-blobs-classification` from that exact HEAD. No amend or push.

This finding table was written **before production code changes**. Required
P2C source, production migration, canonical/identifier/class implementations,
workspace smoke, testkit graph and the named contracts were inspected.

| ID / severity | Finding | Frozen resolution |
| --- | --- | --- |
| D1 MAJOR | Bytes sketch obscures the JSON domain | P2 PLAN/PLAN_REVISION/ARGUMENTS/INSTRUCTION/RESULT are JSON documents, not arbitrary binary. Accept original UTF-8 JSON bytes only, with SCJ-1 refusals. Model documents with fractional temperature remain noncanonicalizable under ADR-0019; no coercion or promise that every wire-valid JSON document is admissible |
| D2 BLOCKER | Dedupe before dispatch bypasses PRIVATE fail-closed | Decide class before any success: SECRET/CREDENTIAL refuse; absent PRIVATE backend refuses even for an existing row. Existing rows must pass the same read/unprotect/canonical/digest checks before reuse |
| D3 MAJOR | Caller wrong-digest write test is unreachable | No expected digest input or dead DigestMismatch variant. Replace G5 with corrupt-existing-row dedupe refusal; add missing/forged-ref/read-integrity cases |
| D4 MAJOR | Task/step reference helpers need forbidden parent mutation | P2D only Tx::put_blob/get_blob and BlobRef. Whole-transition attachment, role APIs, delete_task and blob+reference atomicity belong to P2F. Preserve/test composite FKs privately |
| D5 BLOCKER | Four prose TEXT fields lack a reversible protected representation and omit JSON extensions | Choose narrow disposition A: no put_classified_text. All future ordinary task/step/receipt/journal PRIVATE-bearing writes must fail closed before SQLite, even with a blob backend, until a complete row-surface design exists |
| D6 MAJOR | ADR acceptance could overclaim PRIVATE product support | ADR-0022 remains Proposed: P2D proves the blob seam/refusal portion only; complete ordinary-row protection is unresolved. No real backend ships |
| D7 MAJOR | Testkit-owned storage trait double changes layering / risks a cycle | Local cfg(test) storage double, not a reusable testkit API; no manifest/dependency/smoke-rule changes. Document the narrow architecture exception |
| D8 MAJOR | Borrowed protection infects Store lifetimes | Store owns Option<Arc<dyn AtRestProtection>>, trait Send + Sync. Normal constructors remain no-backend; two with_protection constructors delegate to the unchanged P2C open path. Tx retains access only during its existing transaction lifetime |
| D9 MAJOR | Generic classes/error seam underspecified | PRIVATE-only object-safe trait with protect/unprotect; no classes_protected list or class parameter. Presence means PRIVATE capability; backend refusal/failure maps to AtRestProtectionFailed, absence to Unavailable. Typed payload-free AtRestProtectionError; no dynamic backend diagnostics/source chain. Backend owns opaque envelope, may be nondeterministic, trusted to actually protect; storage cannot prove crypto |
| D10 MAJOR | Ciphertext identity conflicts with content addressing | SCJ-1 canonical plaintext -> SHA-256 plaintext digest. PUBLIC/PERSONAL content canonical plaintext/NONE; PRIVATE content backend bytes/AT_REST. Reads unprotect as required, canonicalize, verify plaintext digest and return canonical bytes |
| D11 MAJOR | size_bytes misread as logical length or bound | Keep name/schema: stored content byte length == length(content); PRIVATE includes backend envelope/expansion. Consistency only, no resource ceiling |
| D12 MAJOR | Conflict-success hides durable corruption | Exact composite lookup; verify existing content, marker and stored size, unprotect/canonicalize/digest before dedupe success. No INSERT OR IGNORE blessing |
| D13 MAJOR | BlobRef could be mistaken for authority | Private Digest/DataClass fields, public constructor from already-validated Digest plus DataClass, read accessors, safe Debug; no Serialize. Exact (digest,rank) lookup; lower-class substitution never resolves a higher-class-only row |
| D14 gate | Migration authority | Existing production 0001 faithfully supports this narrow blob contract. Do not change it, checksum or catalog; no 0002 |

Unchanged migration checksum:
`sha256:d9068dccbc26ececb71be79c475080633166ba0163c62b2d98b9733512baefea`.
SQL protection marker acceptance is not encryption proof. CHECK/FK enforcement is
not a defense against a local file writer disabling it. BlobRef/classification is
not host authorization or content classification inference.

## 2. TDD and implementation evidence

### Actual RED, first GREEN and development failures

After the design table was frozen, the coordinator wrote the real PRIVATE
no-backend/zero-row test, PUBLIC canonical round-trip and SECRET refusal against
`Tx::put_blob/get_blob`. The first
`cargo test -p serea-storage --lib blob_tests --offline` returned **exit101/E0599**:
missing production methods and `AtRestProtectionUnavailable`/`ClassRefused`
variants. No permissive helper or substitute schema manufactured this RED.
After the remaining tests were written, the same command returned **exit101/E0432**
for absent `AtRestProtection`, `AtRestProtectionError` and `BlobRef` exports.
The constructor-test author independently observed that missing-export RED too.
Production code followed these tests, not the reverse.

First complete focused GREEN:
`cargo fmt --all && cargo test -p serea-storage --offline`:
**111 unit + 2 integration + 7 doctests = 120**, zero failures. Feature work
stopped immediately for three independent read-only reviews. The SQL author had
already passed 29 production-schema tests; that was structural evidence, not a
blob-runtime GREEN. Actual RED and first-GREEN outputs were observed in the editor
terminal; no claim is made that those initial outputs were saved to a log file.

During authorized review remediation, two real behavioral tests were added before
repair. `cargo test -p serea-storage --lib blob_tests::private_ --offline` ran eight
cases: **six passed, two failed**, returning `BlobMissing` and `BlobCorrupt` where
class-first PRIVATE reads required `AtRestProtectionUnavailable`. The repair
moved backend dispatch ahead of lookup/metadata. Repaired storage run passed
**122 unit + 2 integration + 14 doctests = 138**; workspace Clippy passed.
No other coordinator build/test failure or timeout occurred. A reviewer tried the
unavailable `1.85` alias; the required exact `1.85.0` succeeded. Paused agent
sessions were resumed by the owner without intervening scope changes.

### Implemented API and durable semantics

- Only `Tx::put_blob(&mut self, original_json: &[u8], class: DataClass)` and
  `Tx::get_blob(&mut self, blob: &BlobRef)` provide blob access. Neither method
  exists on Store. No raw connection/SQL capability is exposed.
- BlobRef privately holds validated protocol `Digest` and `DataClass`, with
  `new(Digest, DataClass)`, `digest()` and `class()`, safe Debug/Eq/Hash/Clone.
  No payload, key, database path or wire serialization implementation. It is
  identification, not host access authority or declassification permission.
- Input remains original UTF-8 JSON bytes, not a parsed Value or arbitrary binary.
  SCJ-1 refuses malformed text, decoded duplicate keys, fractional/exponent forms,
  negative zero, out-of-domain integers and excessive depth; errors retain none
  of the rejected data. A bytes API cannot prove caller raw-input provenance.
- Digest is SHA-256 over SCJ-1 canonical **plaintext**. PUBLIC/PERSONAL store those
  canonical bytes with NONE. PRIVATE stores backend output verbatim with AT_REST;
  nondeterministic envelopes never change content identity.
- Both put and get dispatch class/backend availability **before lookup**.
  SECRET/CREDENTIAL always ClassRefused; PRIVATE without a backend always
  AtRestProtectionUnavailable, including missing, corrupt or existing refs.
  Permitted lookup is exactly `(digest, data_class_rank)`, never digest-only.
- Existing-row dedupe invokes the full read verifier before reuse: marker, stored
  size, unprotect when PRIVATE, SCJ-1 and plaintext digest. Corrupt/unreadable rows
  return BlobCorrupt or AtRestProtectionFailed rather than success or overwrite.
  No INSERT OR IGNORE path hides damage; a new row uses plain INSERT.
- Get returns canonical plaintext only after verification. Valid alternative JSON
  spelling with the same canonical identity is normalized, not returned verbatim.
  This is semantic JSON integrity, not ciphertext authentication or tamper evidence.
- Six new unit storage errors: CanonicalJson, BlobMissing, BlobCorrupt, ClassRefused,
  AtRestProtectionUnavailable and AtRestProtectionFailed. No unreachable expected-
  digest write error. Display/Debug are fixed categories; no parser/backend
  diagnostics or source chain are retained. Sentinel checks cover both formatters.
- `Store` owns `Option<Arc<dyn AtRestProtection>>`; Tx clones the Arc during its
  existing transaction lifetime. Object-safe PRIVATE-only Send + Sync trait has
  protect/unprotect returning `Result<Vec<u8>, AtRestProtectionError>`, a payload-
  free unit error. No class capability list, borrowed Store lifetime, Debug bound,
  key source, global state or real backend. The backend owns its opaque envelope
  and is trusted to actually protect; storage cannot establish crypto quality.
- Existing constructors remain no-backend. `open_with_protection` and
  `open_in_memory_with_protection` delegate to the identical P2C opening path
  before retaining the Arc. Constructor tests prove ownership/release, no backend
  call during open and preservation of Clock/preflight/catalog/profile/checkpoint.
- Local cfg(test) doubles in `blob_tests.rs` and `protection_tests.rs` are NOT
  ENCRYPTION, NOT SECURITY, NEVER PRODUCTION. A deterministic prefixed reversible
  transform expands content and supports scripted failure/incompatible identity;
  a changing fixed-width envelope tests nondeterminism. Testkit remains unchanged.
- `size_bytes` remains **stored-content length**: canonical plaintext length for
  PUBLIC/PERSONAL, backend envelope length for PRIVATE. It is not original input
  length, PRIVATE plaintext length or a resource bound.
- Insertion is transaction-scoped. Closure Err rolls back all newly inserted blobs,
  leaving no committed orphan from that failed transaction. Successful standalone
  blobs are allowed. Blob+parent-reference atomicity and crash evidence are not
  proved by this test. Caught refused operations also make zero transaction-local
  row/total-change writes while independent permitted work can commit.

### Phase and ADR dispositions

No put_task_blob/put_step_blob, role runtime, attachment, delete_task or parent
mutation. Those belong to P2F whole transitions. Private direct-SQL fixtures cover
both reference tables and all five roles; FK-OFF accepts dangling references but
exact lower-class blob reads still cannot resolve PRIVATE-only content.

No put_classified_text or protected ordinary-row representation is implemented.
Future task/step/receipt/journal writers must refuse every PRIVATE-bearing ordinary
row before SQLite, **even with a blob backend**, until a complete reversible design
covers prose, provider references, error/journal JSON and all extensions including
origin/budget. Protecting four prose fields would not close this surface.
**ADR-0022 remains Proposed** for that reason. Blob tests do not establish product
PRIVATE durable task support. P2 ships no real encryption backend.

Production migration/catalog source and checksum are unchanged: 0001 already
faithfully supports the narrowed contract. No 0002, development-DB compatibility
correction, dependency, lockfile, vendor or frozen wire-version change.

## 3. Independent reviews and remediation

The first-GREEN scope was frozen against P2C HEAD. Coordinator assessment chose
three parallel specialists for distinct integrity, privacy and constructor/scope
risks. They remained read-only and returned evidence-backed candidates; the
coordinator independently re-read code/contracts and merged duplicates.

| Pass | Independent session | Initial findings / terminal disposition |
| --- | --- | --- |
| A blob/canonical/integrity | `49a01d00-f5bf-4fd4-be23-3d29809f842e` | Read-order mismatch plus regression gaps; bounded re-review GREEN |
| B classification/security | `6f8f712b-c5aa-47d9-97e6-6696ab0f06b3` | Same read-order mismatch, configured dispatch and swallowed-refusal gaps; bounded re-review GREEN |
| C architecture/scope/portability | `4ef2bd54-e5f0-4fca-b118-e8be2052982c` | Same read-order mismatch, false dependency premise, stale rank example, API/privacy pins; bounded re-review GREEN |

Initial canonical report `cr-20261004-p2dblob0`, chain
`rc-20261004-p2dblobs`, validated **3 Minor findings + 7 Minor test gaps, 17
coverage areas, Pass with caveat**, no Blocker/Major/Question. It was frozen before
any remediation. Separate resolution `rr-20261004-p2dblob0` accepted the bounded
repairs. Artifacts are in ignored `tmp/reviews/`; this table preserves the findings
and dispositions in committed form.

| ID | Frozen issue | Accepted repair |
| --- | --- | --- |
| F1 | PRIVATE read availability after SELECT/metadata contradicts class-first contract | Real failing missing/corrupt no-backend tests, then protection_for before SELECT |
| F2 | Architecture/ADR falsely assert an existing storage testkit dev edge | Correct to hypothetical added edges; add no dependency |
| F3 | Nearby conceptual SQL example permits ranks0..4 | Ordinary-store example0..2; five protocol classes unchanged; production migration untouched |
| T1 | Configured refused-class matrix incomplete; no unprotect counter | All non-PRIVATE put/get/dedupe/refusal paths with both counters zero |
| T2 | Row counts only after rollback mask writes-before-refusal | Catch refusals inside a committing closure; row count and total_changes unchanged; permissible blob persists |
| T3 | No production composite FK-OFF control | All task/step roles at PUBLIC/PERSONAL: ON rejects, OFF accepts dangling refs, restored ON reports them, exact API still missing |
| T4 | Storage Unicode/exact-depth/configured PRIVATE invalid controls absent | Raw/escaped Unicode, depth64 positive round-trip, invalid PRIVATE raw inputs before backend/insert |
| T5 | No positive PRIVATE read normalization control | Envelope contains noncanonical equivalent JSON; canonical read and verified dedupe preserve envelope |
| T6 | Deferred runtime surface absence not mechanically pinned | Public-method inventory plus representative compile-fail text/reference/deletion/lease checks |
| T7 | BlobRef fields/no-wire-codec not mechanically pinned | Two private-field compile-fail tests and structural no-serde/Serialize source guard |

Bounded terminal re-review examined implementation delta and affected chains only.
A independently passed42 blob tests,2 composite-FK tests and14 doctests; B passed42
blob tests; C passed full138 storage executions on stable and1.85 plus storage
Clippy/docs/smoke. All findings/gaps resolved; no inherited excluded claim reopened.
Terminal canonical `cr-20261004-p2dblob1` validates against the immutable initial
report and complete resolution: **0 findings, 0 gaps, 17 areas, Pass**. Lexical API/
serialization guards are repository-convention pins, not an exhaustive Rust AST or
serialization-trait proof; compile-fail cases and current source review accompany
them. No dependency was added solely to strengthen a negative test.

## 4. Final validation

Coordinator executed all commands below, not merely accepting reviewer results.
Actual environment: stable **Rust1.98.1**, **Rust1.85.0**, host
**x86_64-apple-darwin**. Python smoke tests:37; documentation validator:58 Markdown
files. Local logs `tmp/p2d-final-*.log` and grouped result JSON capture each exit.

| Command | Actual result |
| --- | --- |
| cargo fmt --all -- --check | PASS |
| cargo check --workspace --all-targets --offline | PASS |
| cargo test --workspace --all-targets --offline | PASS460 regular |
| cargo test --workspace --all-features --offline | PASS460 regular+19 doctests=479 |
| cargo clippy --workspace --all-targets --all-features --offline -- -D warnings | PASS |
| cargo +1.85.0 check --workspace --all-targets --offline | PASS |
| cargo +1.85.0 test --workspace --all-features --offline | PASS460 regular+19 doctests=479 |
| cargo +1.85.0 clippy --workspace --all-targets --all-features --offline -- -D warnings | PASS |
| python3 tests/workspace_smoke.py | PASS exact3 members/layering |
| python3 -m unittest discover -s tests -p workspace_smoke_tests.py | PASS37 |
| python3 -m py_compile tools/validate_docs.py | PASS |
| python3 tools/validate_docs.py docs | PASS58 |
| git diff --check | PASS |
| cargo metadata --no-deps --format-version 1 | PASS exact protocol/storage/testkit |
| python3 docs/plans/p2a-doc-probes.py | PASS unchanged production-schema/canonical probes |

Focused `cargo [+1.85.0] test -p serea-storage [--release] --offline` executed in
all four requested modes, **122 unit+2 integration+14 doctests=138 each**, zero
failed/ignored, no debug/release discrepancy. This reruns all applicable P2C tests,
not just new blobs: migrations/catalog/checksum, foreign/nonempty/corrupt/newer/WAL
refusals, WAL/FULL/FK/timeout, bootstrap and concurrent open, checkpoint Busy/retry,
error redaction, commit/body rollback, all14 EpochMillis SQL bounds and layering.

Execution-name Counter comparison (doctest line numbers normalized) retains **all
412 P2C baseline executions, zero missing**. Final479 means **58 new regular+9 new
doctests=67**:42 blob tests,14 protection-constructor tests,2 new schema tests and9
new compile-fail doctests. Existing schema tests were also expanded, not replaced.
No P2A/B/C test was deleted. Protocol/testkit, wire schemas/versions, vendor,
manifest/lock dependencies, migration/catalog, foundation/integration tests, smoke
rules and CI remain unchanged.

Separate production-source Python probe passed531 schema probes,126 instant/NULL
controls,9 checksum grammar controls,37/37 constructible legal task pairs,21
SCJ/IDK vectors and10,119,638 text assertions. Python SQLite3.43.2 is schema/design
evidence, not proof of bundled Store crash durability or engine implementation.
Final closure documentation and diff hygiene are rechecked before commit.

## 5. Scope and explicit nonclaims

P2D implements no task/step mutation, parent-reference transition, plan persistence,
receipt runtime, delete_task, lease, TaskEngine, recovery, participant, event bus,
policy, provider, scheduler, real encryption, Keychain, SECRET/credential store,
resource bounds, tamper evidence, or P2E/P3 work. No empirical ARM/Apple Silicon
portability claim. Rollback of a blob insertion is not a crash or blob+reference
atomicity proof. A synthetic reversible test transform is NOT ENCRYPTION, NOT
SECURITY, NEVER PRODUCTION. PRIVATE blob wiring is not production PRIVATE task
support. ADR-0022 remains Proposed until the complete ordinary-row question is
resolved.

### Changed-file inventory and stop

17 paths in this coherent implementation:

- `crates/serea-storage/src/{blob,classify,error,lib,store,tx}.rs`;
- `crates/serea-storage/src/{blob_tests,protection_tests,schema_tests}.rs`;
- `docs/plans/{P2-storage-task-engine,P2-sqlite-schema,P2-test-matrix,P2-tomorrow-decision-ledger,P2D-review-and-closure}.md`;
- `docs/decisions/ADR-0022-durable-private-data-at-rest.md`;
- `docs/architecture/03-crate-map.md`;
- `docs/protocols/09-data-classification-protocol.md` (implementation annotation only).

Dependencies added:0. Vendor changes:0. Migration changes:0. Real backend:0.
Current workspace remains exactly protocol/storage/testkit. Intel/ARM-neutral
canonical/digest/class/SQL representation is inspected and tested on x86_64 only;
backend envelope portability is the backend's responsibility, not a tested cipher
or cross-machine promise.

**P2D CLOSED. Ready for a fresh P2E context, not authorization to implement P2E
here.** One `feat: implement P2D blob classification` commit with exact P2C parent.
The commit's own hash and final clean worktree are recorded from Git after commit,
not embedded self-referentially in this file. No prior amend, no push. STOP.
