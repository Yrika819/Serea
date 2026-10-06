# P2C Storage Foundation Review and Closure

## 1. Preflight and frozen design gate

Starting branch `p2/p2b-clock-time`, clean worktree; exact starting HEAD
`5d63c5f661670aa8ac2dea35f49a9e2762161bc8` (`feat: implement P2B clock and time`).
Both offline workspace all-feature baselines were executed twice, with captured
logs in `tmp/p2c-baseline-{stable,msrv}.log`: **336 regular + 5 doctests = 341**
on each toolchain, zero failures. Created `p2/p2c-storage-foundation` from that
exact HEAD. No P2A/P2B amend or push.

The coordinator inspected required source/design and reconciled independent
read-only gate session `f2c291f7-86bc-4b3d-8b4f-88b383b299fc`. The following table
is frozen **before production code**; implementation evidence is appended below.

| ID | Finding | Accepted decision |
| --- | --- | --- |
| D1 | P2C four-member exit contradicts engine prohibition/P2F ownership | Exactly protocol/storage/testkit; no placeholder engine. Correct plan, matrix, smoke and CI |
| D2 | TaskQueries names upper-layer StepPhase | Defer TaskQueries/view until P2F consumer design; no new persistence enum or string filter |
| D3 | All 14 SQL instants are unrestricted | Every durable instant CHECK uses exact EpochMillis inclusive MIN -62167219200000 / MAX 253402300799999; durations/counters unaffected |
| D4 | WAL before identity check mutates refused files | Inspect nonempty files read-only first: authority, newer schema, full ordered prefix/name/checksum, quick_check. Only absent/zero-length fresh. Revalidate writable handle before WAL and catalog under IMMEDIATE before migration |
| D5 | Drop cannot report TRUNCATE busy | Retryable checkpoint_for_close(&self), typed Busy, parse three-column row; memory NotApplicable. Drop only destroys connection best-effort |
| D6 | Clock borrow might infect Store lifetime | Read injected Clock at open before mutation, retain only validated instant for migration stamps; Store owns no Clock |
| D7 | Protection parameter prematurely implements P2D | Defer trait and constructor seam to P2D; no PRIVATE runtime claim |
| D8 | SQL is not canonical JSON | sha2 SHA-256 of exact include_str UTF-8 migration bytes including whitespace/comments/final newline; checked protocol Digest construction |
| D9 | Markdown DDL would duplicate production authority | Production 0001_initial.sql authoritative; remove whole Markdown executable copy, harness reads production file |
| D10 | Schema foundation is not runtime ADR acceptance | ADR-0021/22/24 remain Proposed; no participant/journal/classification/fencing runtime |

Additional independently verified documentation corrections: SQLite checkpoint
returns three columns (including memory's 0/-1/-1), STRICT affinity permits
lossless integer-to-TEXT conversion, bundled FK default is ON but explicit setting
is mandatory, ordinary triggers are independent of foreign_keys, commit hooks may
veto commit (not post-commit), unlock_notify is shared-cache notification, and
build environment overrides can defeat bundling. These are corrected without
expanding runtime scope. Catalog checksums detect binary/source mismatch, not
local-file tampering. Read-only WAL inspection may create transient SQLite
sidecars; no sidecar-noncreation guarantee is claimed.

## 2. Implementation and TDD evidence

### RED, first GREEN and development failures

Tests were written before the Store implementation. The first offline attempt
failed dependency resolution (`rusqlite` absent from the configured Cargo cache),
not a behavioral RED. Executed `cargo fetch` for build-time dependency setup,
then repeated `cargo test -p serea-storage --test foundation --offline`:
**exit101/E0432** for missing Store/CheckpointOutcome. Captured respectively in
`tmp/p2c-first-red.log` and `tmp/p2c-api-red.log`. No runtime networking.
Independent schema and foundation test modules were written before production
Store code. The first complete storage run exposed malformed TEXT version
comparison as SchemaTooNew (52/53 unit tests passed); corrected typed version
classification, not the test. First full workspace GREEN, captured in
`tmp/p2c-first-green.log`: **394 regular +5 doctests =399**. Feature work stopped
at first GREEN for independent review.

### Dependency ratification and provenance

- Root pins `rusqlite = "=0.40.2"`, defaults disabled, selected feature only
  `bundled`; implied `modern_sqlite`/bundled bindings are expected. No cache,
  wasm adapter, bundled-full, backup, hooks, trace, chrono, jiff, load_extension
  Rust API or unlock_notify enabled.
- Locked `libsqlite3-sys 0.38.2`; runtime tests pin SQLite **3.53.2**, source ID
  `2026-06-03 19:12:13 d6e03d8c777cfa2d35e3b60d8ec3e0187f3e9f99d8e2ee9cac695fd6fcdf1a24`.
  Production checks SQLite >=3.37.0 plus json_valid/json_type/json_extract;
  application compatibility does not depend on a patch source ID.
- Both SQLite Rust manifests declare no MSRV; empirical workspace1.85 results
  below establish compatibility. sha2 workspace0.11.0 is reused directly for SQL
  byte hashing. No testkit or tempfile dependency.
- Nine new locked external packages: rusqlite, libsqlite3-sys, cc1.6.0,
  find-msvc-tools0.1.14, shlex2.0.1, pkg-config0.3.34, vcpkg0.2.15,
  fallible-iterator0.3.0, fallible-streaming-iterator0.1.9. Existing package blocks
  retained unchanged; bitflags/smallvec already existed. Vendor sources unchanged.
- Coordinator inspected `otool -L` on storage unit and integration test binaries:
  only libiconv/libSystem, **no dynamic libsqlite3**. Native build selects bundled
  amalgamation; no SQLITE/LIBSQLITE build overrides in actual environment.
  `pkg-config`/vcpkg remain compiled defaults, not proof of linked system SQLite.
  C extension capability may be compiled in; no Rust loader API is exposed.

### Production schema, checksum and time

`crates/serea-storage/migrations/0001_initial.sql` is the sole production DDL
source (18,804 embedded UTF-8 bytes). Markdown no longer contains a whole-schema
executable duplicate; Rust and Python probes read this actual file. Initial
migration version1, name `0001_initial`, LATEST1. Checksum is exact raw bytes,
including leading/final newline, comments and whitespace, with no SCJ-1 wrapper,
trimming or normalization. Checked protocol Digest:
`sha256:d9068dccbc26ececb71be79c475080633166ba0163c62b2d98b9733512baefea`.
Known SHA256 vectors and whitespace/comment/newline changes are tested; modified
embedded SQL refuses before applying a pending migration.

Exactly **10 STRICT tables**, **6 explicit indexes**, **7 triggers**, with exact
names asserted. Tables: schema_migrations, blobs, tasks, task_steps,
side_effect_receipts, leases, plan_revisions, task_blob_refs, step_blob_refs,
task_journal. Indexes: task_steps_status_lease, task_steps_task_status,
tasks_state, step_blob_refs_digest, task_blob_refs_digest, plan_revisions_digest.
Triggers: tasks_policy_class_immutable, tasks_data_class_monotonic,
side_effect_receipts_key_matches_step, side_effect_receipts_task_matches_step,
side_effect_receipts_step_must_succeed, task_steps_idempotency_key_immutable,
task_journal_step_task_matches. No removed lease token/generation-match trigger,
event_seq or TASK_DELETED; journal vocabulary remains its13-value closed set.

All **14 durable instant columns** structurally enforce EpochMillis inclusive
**MIN=-62_167_219_200_000 / MAX=253_402_300_799_999**. Direct SQL tests cover exact
range admission, adjacent refusal, real persisted legal rows and nullable NULL.
Lease acquired_at=MAX or expires_at=MIN cannot form a real row because expiry
must exceed acquisition; tests explicitly distinguish individual range admission
from these relationally impossible endpoints. No wire-grammar narrowing or
counter/duration Epoch checks. applied_at comes only from one injected Clock
reading at open, including negative epochs; the Store retains no Clock borrow.

Positive constructibility against actual bundled production DDL covers all56
kind/status cells (51 accepted,5 refused), all448 error subsets,816 capability
subsets,176 task-state/presence tuples, cancellation/error forms, generated class
labels, u32 generations, lease baseline, plan/blob-reference and journal baseline.
Python's separate production-source design probe reruns531 schema probes,
126 epoch/NULL controls,9 checksum grammar controls,37/37 legal task-pair
constructions,21 canonical/IDK vectors and10,119,638 text assertions. That Python
SQLite3.43.2 run is design/schema evidence, not bundled Store durability proof.

### Store/Tx, preflight and profiles

Public Store surface: open(path,&dyn Clock), open_in_memory(&dyn Clock),
schema_version(), verify_integrity(), transact(opaque Tx closure),
checkpoint_for_close(&self). Migration source metadata/checksum are inspectable,
not executable through a Store public SQL capability. Four external negative
API doctests forbid connection/Tx extraction and SQL execution; one positive
opaque Tx control passes. No TaskQueries/view/engine phase type, storage enum,
protection trait/parameter, domain CRUD or participant registry.

Nonempty files: read-only identity/catalog/quick_check preflight, then writable
handle revalidation before persistent settings. Catalog inspection uses **one
ordered SELECT snapshot**, scans all typed versions for SchemaTooNew priority,
then validates exact contiguous prefix/version/name/checksum/time, not MAX alone.
Catalog/source validation precedes application; migration rechecks prefix under
BEGIN IMMEDIATE. Foreign valid nonempty zero-user-table SQLite is refused,
not considered fresh. Plain/binary/corrupt-header/unrelated/empty-schema/newer/
checksum-invalid files are refused without main-byte changes. Four live WAL
fixtures additionally preserve exact main/WAL bytes, logical contents and journal
mode. SQLite transient SHM behavior is not constrained by these claims.

**Review-driven bootstrap correction:** absent/zero-length fresh connections
first configure version/JSON/FK/FULL/timeout, commit initial DDL+catalog atomically
under SQLite's rollback journal, **then enable WAL**. This avoids publishing a
nonempty unmarked WAL header that a concurrent initializer would classify foreign.
Accepted existing stores configure full WAL policy before pending migrations.
Every successfully returned file Store is WAL/FULL/FK1/timeout5000. Initial
bootstrap is a narrow initialization ordering, not a read-only Store or foreign
adoption exception. Independent public-API probe12/12 opener pairs succeeded;
publication-stage and concurrent-open regressions are retained.

Each migration: BEGIN IMMEDIATE, execute authoritative SQL, insert catalog row,
quick_check + foreign_key_check, COMMIT. SQL/post-check failures roll back DDL and
row together; earlier successful migration commits remain. Custom multi-migration
fault tests use actual WAL/FULL profile and real files. No application_id or
user_version authority. All normal writes use Store::transact BEGIN IMMEDIATE:
Ok commits; body Err explicitly rolls back; real deferred-FK COMMIT failure is
returned typed and transaction rolled back. Panic unwinds rollback and poisons
mutex; next call yields LockPoisoned. Reentrant Store access from the closure is
explicitly prohibited/documented because it holds the connection mutex.

File profile asserts journal_mode=wal, synchronous=2(FULL), foreign_keys=1,
busy_timeout=5000. Memory asserts journal_mode=memory, FK1,timeout5000; no fsync,
WAL/reopen/restart/crash durability claim. Both production configuration paths are
tested starting FK OFF, proving explicit enablement rather than inherited default.
No temp_store or wal_autocheckpoint tuning in production.

Normal open consumes **all quick_check rows**, requiring every row ok. Each
migration adds separate FK verification. Admin verify_integrity uses bounded
integrity_check(100), consumes all rows and separately foreign_key_check. An
FK-orphan regression shows both page checks ok while FK check/admin verification
fail; normal-open quick-only intentionally accepts that orphan. Integrity checks
prove neither application invariants nor file authentication.

checkpoint_for_close(&self) runs three-column wal_checkpoint(TRUNCATE) for files,
checks busy and frame completion, returns typed Busy while retaining handle for
retry. Memory returns NotApplicable without issuing checkpoint. Reader snapshot
forces Busy; releasing reader and retrying same Store succeeds. Normal move:
quiesce clients/other connections, successful explicit checkpoint, drop all
connections, copy main file. Drop only destroys connection best-effort; never
proof of reported checkpoint success. Abnormal WAL portability procedure remains
main+WAL, never SHM; process-crash/architecture fixtures deferred.

StoreError is manual payload-free categorization. Display/Debug emit fixed safe
categories; raw SQLite messages/paths/SQL/bound values discarded and Error::source
is None. Every category including sentinel-bearing Clock payload, actual trigger,
constraint and bound-value errors is tested. No private prose appears in either
formatter or source chain.

## 3. Independent review, frozen findings and remediation

First-green reviewers were new independent read-only sessions:

| Pass | Session | First-green findings |
| --- | --- | --- |
| A SQLite/migration | f1f17f4c-0548-46e2-a59c-18aee25270af | initialization misclassified foreign; split-snapshot newer priority; file-profile/cascade coverage gap |
| B security/authority | 497566cf-5619-4b7a-99e1-fd983de085a8 | retained marker/default/error prose; API/source/WAL/explicit-FK test gaps |
| C architecture/portability | 1cb9fbe2-a3f9-4fc7-9c6c-dc119d3542fe | smoke formatting bypass; bundling selector prose; corrupt-header test gap; harness phase question |

Coordinator independently reread and adjudicated candidates before remediation.
Canonical frozen report `cr-20261004-p2cstorage0`, chain
`rc-20261004-p2cstorage`, validated **7 Minor findings +6 Minor test gaps,14 areas**
(Pass with caveat); no Blocker/Major. Both concurrency classifications reproduced
independently; no demonstrated data loss. Harness question settled from current
owner mission: P2C deterministic rollback/reopen required, true crash-child and
cross-binary harness proofs explicitly P2H, optional early infrastructure.

| ID | Frozen finding/gap | Remediation |
| --- | --- | --- |
| F1 | Nonempty unmarked WAL initialization window | Atomic DELETE/FULL bootstrap before WAL; independent12-round original probe passes |
| F2 | Two catalog SELECT snapshots misclassify concurrent newer row | One ordered snapshot, delayed failures/newer priority; independent mid-read probe gets coherent old snapshot then SchemaTooNew |
| F3 | Schema marker described as encryption proof | Marker-only prose, P2D runtime deferred |
| F4 | FK default OFF without upstream qualification | Upstream/bundled distinction |
| F5 | Prose claims SQLite source retained | Current payload-free errors documented; future variants deferred |
| F6 | Comment/indent TOML header bypasses layering smoke | Whole-input stdlib-only focused parser, unsupported syntax fails closed;37 tests on Python3.9.6 |
| F7 | LIB_DIR falsely described as bundling selector | Selector/search-path distinction |
| T1 | External isolation pin absent | Four negative+one positive doctests |
| T2 | Source-chain/Clock sentinel not pinned | Every category formatter/source None |
| T3 | WAL refusal not exercised | Four committed uncheckpointed WAL fixtures, exact main/WAL/state preservation |
| T4 | FK enablement could rely on default | Explicit OFF->ON production config and orphan refusal, both profiles |
| T5 | Custom migration/cascade durability profile gaps | Actual file profile plus cascade/restrict commit/rollback/reopen |
| T6 | Valid Store header corruption fixture absent | Header-only damage refused, exact corrupted bytes preserved |

Resolution `rr-20261004-p2cstorage0` is separate from the frozen report. Same three
sessions performed bounded delta/affected-chain re-review. A and B GREEN; C caught
four stale intermediate tomllib/Python3.11 passages left while preserving final
Python3.9 compatibility. That incomplete inherited F6 prose was frozen, corrected
only in the four documents and independently checked GREEN before canonical final
freeze. No new feature or implementation scope. Terminal canonical
`cr-20261004-p2cstorage1` validates with both parent artifacts: **0 findings,
0 gaps,14 areas, PASS**. Local review artifacts in ignored `tmp/reviews/`; frozen
findings and dispositions are preserved here in committed form.

## 4. Final validation and regression retention

Actual toolchains: stable **Rust1.98.1**, **Rust1.85.0**; host
**x86_64-apple-darwin**, Python3.9.6. Local logs `tmp/p2c-final-*.log` and grouped
JSON results. Every listed command actually executed; no reviewer result is used
as a substitute for coordinator validation.

| Required command | Result |
| --- | --- |
| cargo fmt --all -- --check | PASS |
| cargo check --workspace --all-targets --offline | PASS |
| cargo test --workspace --all-targets --offline | PASS **402 regular**,0 failed/ignored |
| cargo test --workspace --all-features --offline | PASS **402 regular+10 doctests=412** |
| cargo clippy --workspace --all-targets --all-features --offline -- -D warnings | PASS |
| cargo +1.85.0 check --workspace --all-targets --offline | PASS |
| cargo +1.85.0 test --workspace --all-features --offline | PASS **412** |
| cargo +1.85.0 clippy --workspace --all-targets --all-features --offline -- -D warnings | PASS |
| python3 tests/workspace_smoke.py | PASS exact3 membership/layering |
| python3 -m unittest discover -s tests -p workspace_smoke_tests.py | PASS37 |
| python3 -m py_compile tools/validate_docs.py | PASS |
| python3 tools/validate_docs.py docs | PASS57 Markdown files |
| python3 docs/plans/p2a-doc-probes.py | PASS actual production-source probes |
| git diff --check | PASS |
| cargo metadata --no-deps --format-version 1 | PASS exact protocol/storage/testkit |

Focused `cargo [+1.85.0] test -p serea-storage [--release] --offline`, all four
actual variants: **64 unit+2 integration+5 doctests=71 each**, debug/release agree
for migration/open/checkpoint/epoch bounds and all remaining storage tests.
Initial simultaneous release builds/runs timed out after240s including artifact
lock/build contention; these are **not PASS claims**. Subsequent sequential
bounded360s retries completed71/71 each. First generated policy test-module
placement triggered Clippy items_after_test_module; moved below production
items, reran successfully. Independent A's200-repeat initial review command also
timed out; completed bounded12/30 runs are recorded as empirical tests, not
exhaustive interleaving proof. All final gates green, no ignored/failing test.

Coordinator compared execution-name Counters: **all341 baseline executions
retained, zero missing**, final412; duplicate names retained. New **66 regular
storage tests +5 storage doctests=71**, plus37 Python smoke tests. Protocol,
testkit, wire schemas, frozen versions, vendor patches, .clippy.toml and ADR
status/source unchanged. No P2A/P2B regression deletion or dependency-version drift.

## 5. Inventory, nonclaims and stop

Implementation commit covers22 paths:

- root Cargo.toml/Cargo.lock and .github/workflows/ci.yml;
- crates/serea-storage/Cargo.toml;
- crates/serea-storage/src/{lib,error,migrate,store,tx,foundation_tests,schema_tests}.rs;
- crates/serea-storage/migrations/0001_initial.sql;
- crates/serea-storage/tests/foundation.rs;
- tests/workspace_smoke.py and tests/workspace_smoke_tests.py;
- docs/plans/P2-storage-task-engine.md, P2-sqlite-schema.md, P2-test-matrix.md,
  P2-tomorrow-decision-ledger.md, P2-6.1-sol-launch.md, p2a-doc-probes.py,
  P2C-review-and-closure.md.

**Nonclaims:** no blob/classification/encryption or PRIVATE runtime support,
lease APIs/fencing/revocation, TaskEngine, recovery/journal participants, event
bus/P3, providers/models, policy, approvals, scheduler, Android, GoalLatch or
Local MCP integration. No empirical Apple Silicon/Linux/cross-machine migration,
process-kill/crash/power-loss/fault-VFS/deterministic mid-COMMIT proof. Plain Err
rollback is not called a crash. No resource-bound policy, tamper evidence,
read-only Store or Drop checkpoint guarantee. ADR-0021/22/24 remain Proposed.
No temp filesystem crypto claim. SQL marker consistency is not encryption.

P2C CLOSED. Ready for a **fresh P2D context**, not authorization to start P2D here.
One coherent `feat: implement P2C storage foundation` commit with unchanged P2B
HEAD as parent. Self-referential commit hash comes from Git after commit, not
embedded in its own record. No amend or push. STOP.
