# P2B Clock and Time Review and Closure

## 1. Preflight

Starting branch: `p2/p2a-protocol-corrections`.
Starting HEAD: `824c6ce1931cc5b4f9f77e72bdbedc600a2e31af`
(`feat: implement P2A protocol corrections`). Worktree was clean.
Both `cargo test --workspace --all-features --offline` and
`cargo +1.85.0 test --workspace --all-features --offline` passed **307/307**
with zero failures/ignored tests. Logs: `tmp/p2b-baseline-stable.log` and
`tmp/p2b-baseline-msrv.log` (local scratch, not committed).
Created `p2/p2b-clock-time` from that exact HEAD after the green baseline.

## 2. Frozen pre-implementation design findings

Frozen before production edits; coordinator inspected actual P2A source and
all requested design sections, then independently reconciled the read-only
review in session `6076c54c-d9a8-4663-95fe-39517534e339`.
Expected behavior is the owner's P2B mission and unchanged P2A wire grammar.

| ID | Severity | Evidence and finding | Accepted design disposition |
| --- | --- | --- | --- |
| D1 | BLOCKER | Design §8 / gap §5.11 falsely claim unsigned ULID `TimestampMs` covers all wire dates; validator accepts 0000–9999 including pre-1970 | Add private validated `EpochMillis(i64)`, inclusive MIN −62,167,219,200,000 / MAX 253,402,300,799,999; leave ULID type unchanged |
| D2 | MAJOR | `Timestamp` derives textual Ord; seconds form sorts after later fractional form; repository ordering search found no consumer needing it | Remove PartialOrd/Ord; compare numeric instants; Eq/Hash stay spelling-based |
| D3 | MAJOR | Old TestClock accepts i64::MAX delta before unchecked calendar addition and iterative month normalization | Checked constant-time epoch arithmetic; reject before committing |
| D4 | MODERATE | TestClock duplicates parser/leap/month arithmetic and rejects legal seconds form | Store start_ms/now_ms only; use protocol Timestamp validation/conversion |
| D5 | MODERATE | Planned Clock lacks provider-port Send/Sync bounds | Synchronous object-safe `Clock: Send + Sync`, Result<EpochMillis, ProtocolError> |
| D6 | MODERATE | E5 requires forbidden Store implementation; E7 refers to nonexistent new crates | P2B constructor/Clock failure tests and current-source inspection; Store tests deferred P2C |
| D7 | MINOR | ULID seed comment claims equality to frozen clock epoch but literal denotes 2026-09-20, not October 1 | Clarify independent legacy seed; preserve deterministic ID sequence |

Additional semantic correction: both wire spellings of an exact-second instant
map to the same epoch value. Reconstruction must canonicalize, not claim to
recover lost spelling. Existing Timestamp instances retain their exact input
when serialized. Epoch reconstruction always emits `YYYY-MM-DDTHH:MM:SS.mmmZ`.
No Timestamp grammar, schema, wire/architecture version, or ADR changes.

D1–D3 are resolved at the design gate by the accepted model; the five corrected
P2 plan documents carry this model before implementation. This table is frozen;
later evidence/dispositions are appended separately, not rewritten.

## 3. Implementation and TDD evidence

### Actual RED → GREEN

After the design gate, three tests were written first: legal pre-epoch conversion,
equal instant for both wire spellings, and canonical epoch reconstruction.
`cargo test -p serea-protocol --test p2b_time --offline` exited **101** with
E0432 (missing EpochMillis export) and E0599 (missing conversion methods).
Log: `tmp/p2b-first-red.log`. No deliberately broken helper or syntax error.
The expanded protocol/clock matrices were written before production time code.

First GREEN: **16** protocol time tests, **9** focused clock tests, all **29**
existing deterministic-fake tests, and **2** negative ordering doctests passed.
Independent-review remediation then added four test declarations and three
doctests; no production behavior changed in that remediation.

### Final time model

- `EpochMillis(i64)` has a private field, checked `new`, `get`, numeric
  PartialOrd/Ord/Eq/Hash and inclusive `MIN = -62_167_219_200_000`,
  `MAX = 253_402_300_799_999`. The bounds follow the unchanged four-digit
  proleptic Gregorian wire years 0000–9999. No serde/persistence bypass exists.
- `Timestamp::to_epoch_millis` is total for every validated Timestamp and
  preserves its instant without mutating its spelling.
  `Timestamp::from_epoch_millis` is total for validated EpochMillis and always
  emits canonical `.mmmZ`. Existing serde, spelling-based Eq/Hash and calendar
  validator are unchanged. PartialOrd/Ord are removed (compile-fail regressions).
- Constant-time civil-date conversion follows Howard Hinnant's March-based
  400-year-era algorithm. Euclidean division handles year0000 January/February
  and negative epoch days/milliseconds. Private bounded helper intermediates
  provably fit i64; epoch accumulation and external delta arithmetic are checked.
  No float, timezone, locale, libc, iterative calendar normalization or dependency.
- `TimestampMs` remains the unchanged validated unsigned 48-bit ULID type.
  Protocol ids.rs and the deterministic source implementation are byte-identical
  to P2A. The inaccurate seed comment is corrected; legacy seed and ID sequences
  are unchanged, independent of the frozen TestClock epoch.
- `Clock: Send + Sync` is synchronous, object-safe and injection-only:
  `fn now_ms(&self) -> Result<EpochMillis, ProtocolError>`. No wall-clock provider.
- TestClock state is exactly `start_ms: EpochMillis` (fixed origin) and
  `now_ms: EpochMillis` (sole current instant). Elapsed is derived with checked
  subtraction and conversion. All duplicate parser/calendar/formatter logic and
  the mutable elapsed counter are removed. Both wire spellings initialize it;
  output is canonical. `advance` truncates sub-ms Duration fractions as before,
  checks u128-to-i64 narrowing, addition and wire bounds, then commits only a
  validated candidate. Overflow/Duration::MAX leaves time and elapsed unchanged.

### Final tests and file inventory

New **29 regular tests**: 20 in `crates/serea-protocol/tests/p2b_time.rs`,
9 in `crates/serea-testkit/tests/p2b_clock.rs`. New **5 doctests**: one positive
EpochMillis API control, two privacy refusals, two Timestamp ordering refusals.
The independent running-day reference covers all 120,000 legal months, their
240,000 first/last instants, plus exact min/max/epoch/negative/2038/mixed-time
anchors. Clock tests cover full-span advance, all leap cases, huge deltas,
atomicity, replay, SendSync/dyn reads and typed port failures. Existing seconds
refusal is changed to acceptance; the i64::MAX duration case is added to its
existing regression. Counter-based comparison of actual baseline/final test executions confirms
**all 307 baseline cases retained, zero missing** (duplicate names preserved).
A separate source-declaration comparison retains all 264 explicit test functions;
43 macro-generated cases are covered by the execution comparison. Its initial
10-second source probe timed out; the bounded 60-second rerun succeeded.

Changed files (13):

- `crates/serea-protocol/src/clock.rs` (new)
- `crates/serea-protocol/src/types.rs`
- `crates/serea-protocol/src/lib.rs`
- `crates/serea-protocol/tests/p2b_time.rs` (new)
- `crates/serea-testkit/src/clock.rs`
- `crates/serea-testkit/tests/p2b_clock.rs` (new)
- `crates/serea-testkit/tests/fakes_are_deterministic.rs`
- `docs/plans/P2-storage-task-engine.md`
- `docs/plans/P2-test-matrix.md`
- `docs/plans/P2-contract-gap-analysis.md`
- `docs/plans/P2-tomorrow-decision-ledger.md`
- `docs/plans/P2-6.1-sol-launch.md`
- `docs/plans/P2B-review-and-closure.md` (new)

No Cargo manifest/lock, vendor, schema, wire version, architecture version, ADR
or .clippy.toml changes. Protocol/testkit remain the only workspace crates.

## 4. Independent implementation reviews

### Frozen first-green findings (before remediation)

Three separate read-only sessions completed after first green, before any fixes:

| Pass | Session | Frozen result |
| --- | --- | --- |
| A — time/math | `0663078e-8188-4e05-bb8f-b77dd5b3f18b` | No code defect; mixed time-of-day/minute/hour carry regression gap |
| B — API/contracts | `c9da7e51-d317-4ebf-9fce-b072d88777ac` | No code defect; privacy, hashing and explicit 2038 regression gaps |
| C — portability/regression | `a1be49c7-11e8-4497-aec3-35feb58f1175` | No code defect; same three gaps plus direct signed ordering across zero |

Coordinator independently verified and deduplicated candidates into five
**MINOR test gaps**, zero BLOCKER/MAJOR/code findings. Frozen canonical report:
`tmp/reviews/2026-10-03-code-review-report-p2btime0.md`, report ID
`cr-20261003-p2btime0`; validator PASS: zero findings, five gaps, fourteen areas.
This report is fixed; remediation and bounded re-review are separate artifacts.

| ID | Frozen gap | Accepted remediation boundary |
| --- | --- | --- |
| T1 | Private epoch construction/access lacks negative API test | External compile-fail snippets with valid positive controls |
| T2 | Spelling-based Timestamp Hash not exercised | Compare recorded hasher input with exact String input; no collision-freedom claim |
| T3 | Exact 2038 second threshold absent | Pin 2,147,483,647,999 → 2,147,483,648,000 ms conversion/reconstruction |
| T4 | Direct negative < zero < positive assertion absent | Signed epoch ordering regression |
| T5 | Independent mixed time-of-day and minute/hour carries absent | Exact positive/negative mixed-time values and 59.999/3599.999-second carries |

All five are accepted for test/doc-test-only remediation. Production conversion
and clock behavior require no change. This subsection freezes the first review;
final dispositions follow separately.

### Remediation and bounded final review

All T1–T5 were fixed with tests/doc-tests only; focused coordinator execution
passed 20/20 protocol tests and 5/5 doctests. Resolution artifact:
`tmp/reviews/2026-10-03-resolution-p2btime0.md`, ID `rr-20261003-p2btime0`.
Same three independent sessions then read both parent artifacts and checked only
the remediation delta/affected chains. **A, B and C each PASS**, zero remaining
findings/gaps, no design issue reopened and no further feature work.
Final canonical generation-1 report:
`tmp/reviews/2026-10-03-code-review-report-p2btime1.md`, ID
`cr-20261003-p2btime1`; validator **PASS** with parent report/resolution:
zero findings, zero gaps, fourteen areas, recommendation Pass.
Report files are local ignored scratch; the frozen findings and dispositions
are preserved here in the committed closure record.

Math reviewer independently completed integer Python checks of a full 146,097-day
Gregorian era, 40,060 all-year/year0000 anchors and 604,800 time-of-day cases;
these are attributed independent reference evidence, not Rust execution. Its
whole-domain enumeration timed out at 60 seconds; that enumeration is not claimed
completed. All newly added fixed fixtures were independently rechecked.

## 5. Final validation and non-claims

Toolchains actually used: stable **Rust 1.98.1**, MSRV **Rust 1.85.0**;
host/installed target **x86_64-apple-darwin**. Final command logs and structured
results are in `tmp/p2b-final-*.log` / `tmp/p2b-final-results.json` (ignored local
scratch). Every command below was run, not inferred from reviewer approval.

| Required command | Result |
| --- | --- |
| `cargo fmt --all -- --check` | PASS |
| `cargo check --workspace --all-targets --offline` | PASS |
| `cargo test --workspace --all-targets --offline` | PASS: 336 regular tests, zero failed/ignored |
| `cargo test --workspace --all-features --offline` | PASS: 336 regular + 5 doctests = 341 |
| `cargo clippy --workspace --all-targets --all-features --offline -- -D warnings` | PASS |
| `cargo +1.85.0 check --workspace --all-targets --offline` | PASS |
| `cargo +1.85.0 test --workspace --all-features --offline` | PASS: 336 regular + 5 doctests = 341 |
| `cargo +1.85.0 clippy --workspace --all-targets --all-features --offline -- -D warnings` | PASS |
| `python3 tests/workspace_smoke.py` | PASS: crate-map layering |
| `python3 -m py_compile tools/validate_docs.py` | PASS |
| `python3 tools/validate_docs.py docs` | PASS: 56 Markdown files |
| `git diff --check` | PASS; staged diff also checked before commit |
| `cargo metadata --no-deps --format-version 1` | PASS; inspected exactly protocol/testkit, both rust-version 1.85; dependencies unchanged |

Focused commands, each executed in **stable debug, stable release, Rust 1.85
debug, Rust 1.85 release**:

- `cargo [+1.85.0] test -p serea-protocol [--release] --offline --test p2b_time`:
  **20/20 each**, including min/max/pre-epoch/canonicalization and all-month reference.
- `cargo [+1.85.0] test -p serea-testkit [--release] --offline --test p2b_clock --test fakes_are_deterministic`:
  **38/38 each** (9 P2B + 29 existing).
- `cargo [+1.85.0] test -p serea-protocol [--release] --doc --offline`:
  **5/5 each**. Brackets denote the four actually executed variants, not literal
  command arguments. Total **63 passing focused executions per combination**;
  no debug/release discrepancy.

Initial MSRV Clippy invocation failed because cargo-clippy was not installed.
Executed `rustup component add --toolchain 1.85.0-x86_64-apple-darwin clippy`,
then repeated the entire matrix successfully. One remediation command improperly
combined --doc with --test; Cargo rejected the options, and both correct separate
commands passed. The deliberate first RED is retained separately. No final test
failure, ignored test, lint error or unresolved review finding remains.

Source-level assertion and independent full-file inspection cover the new time
files, changed exports/types, tests and embedded examples, including the scanning
test itself; no first-party build script exists. Both ambient constructors remain
forbidden by unchanged .clippy.toml. No alias-based ambient use was found.

**Non-claims:** no empirical aarch64 execution (only x86_64 target installed),
cross-machine/storage persistence proof, real wall-clock provider or monotonicity
promise. Architecture-independent behavior is supported by explicit i64/u64/u128
semantics and bounded indexing, not an invented second-platform run. P2A vendor
patches receive no new certification; they and all dependencies are unchanged.

No storage, SQLite, TaskEngine, leases, recovery, P2C or P3 started. P2B is
complete; ready for a **fresh P2C context**, not authorization to begin it here.
This record accompanies one `feat: implement P2B clock and time` commit with the
exact untouched P2A HEAD as parent. The self-referential commit hash is read from
Git after commit, not embedded in its own contents. No amend or push. STOP.
