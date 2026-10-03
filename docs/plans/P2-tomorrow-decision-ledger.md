# P2 Tomorrow Decision Ledger

- **Branch:** `p2/autonomous-preimplementation-audit`
- **Base commit:** `ec4659c007a914e5d90bb3067d3858a4e299b797` (`p2/design-preparation`)
  — the base this ledger was written against, **not** the audit commit
- **Audit commit:** `732b3ad92801dcb15d5b45548cbb23f325abafe0`
- **Purpose:** so that tomorrow's 6.1 Sol High implementation begins with a small
  number of genuine reasoning decisions rather than a hundred implicit ones.
- **Companion:** [P2 autonomous audit](P2-autonomous-audit.md), which carries the
  evidence for every `READY` row below.

## How to read this

`READY` means specified for implementation, grounded in a frozen contract,
owner direction or scoped executable evidence; it does **not** mean production
validation passed. Historical measurements below retain their original scope.
Current owner ratification after three corrected documentation re-reviews GREEN
accepts ADR-0018/19/20/23 within their stated scopes. P2A slices are implemented;
current final workspace/MSRV validation, test counts, bounded regression review
and integration status are coordinator-owned in the
[closure record](P2A-review-and-closure.md), not inferred from earlier runs. ADR-0018
runtime is deferred and ADR-0021/22/24 remain **Proposed** runtime; 0024 wire
generation only is implemented. The historical audit is not current closure.

| Status | Meaning |
| --- | --- |
| **READY** | Specified for implementation; evidence scope and deferred gates still apply |
| **READY_WITH_LIMITATION** | Evidence-determined, and the limit is named here and in the design |
| **NEEDS_RATIFICATION** | The analysis is done; a human must sign. Ratifying is not deciding |
| **NEEDS_REDESIGN** | Not resolvable at this level; see the audit |
| **SAFE_DEFER** | Deliberately deferred to a named phase, with the deferral enforced |
| **BLOCKED** | Cannot proceed until something outside P2 lands |

**No row is `NEEDS_RATIFICATION` because the analysis is incomplete.** Every
`NEEDS_RATIFICATION` row below is a completed analysis awaiting a signature or a
piece of prose.

---

## 1. P2A — ratified current contract and scoped implementation

Owner direction on 2026-10-03 ratifies current arch1/task2/action2/event1/envelope1
and MSRV 1.85. **IMPLEMENTED_SCOPED** retains the implementation disposition label;
it does not independently claim final validation or integration closure.
Deferred SQL/runtime rows remain READY only. [Closure record](P2A-review-and-closure.md)
retains unchanged G01–G19, historical counts and independent subagent GREEN
re-reviews under the coordinator, alongside coordinator tool-run validation and
frozen implementation review/remediation statuses. Final-tree counts and review
results must be evidenced there after the exact numeric follow-up.

| # | Decision | Status | Phase / acceptance evidence |
| --- | --- | --- | --- |
| 1.1 | arch1/task2/action2/event1; envelope1; per-surface mixed-major registry | **RATIFIED / IMPLEMENTED_SCOPED** | Frozen current contract; all 11 published wire surfaces, including scheduler/1, implemented in registry/dispatch; no scheduler runtime. Coordinator records current final workspace/MSRV validation, test counts, review and integration evidence in the closure record |
| 1.2 | Two migration notes naming all consumers, including manifests, canonical tests, event schema and testkit ports | **RATIFIED** | Launch §4 current inventory; four changed schemas; schema.rs version documentation only, embedding/validation unchanged; envelope schema and ID tests unchanged, inline tests/no fixture file; one P2A integration only |
| 1.3 | Four TaskStep Option conversions; input_digest required; provider/capability/version already Option; seven unconditional fields | **IMPLEMENTED_SCOPED** | Accepted ADR-0018 wire/lifecycle architecture; TaskStepDraft → private validated TaskStep/StepPresence, no public mutation, reserved extension keys refused; no storage runtime |
| 1.4 | Missing/null both None; serializer omits None; SQL0 maps to wireNone; positive u32 generation, overflow refused | **IMPLEMENTED_SCOPED** wire / **READY** SQL | Wire field remains Option<u32>; RawValue token analysis accepts exact integer-valued 1.0/1e0 without f64 rounding and refuses near-integer fractions/overflow, unlike SCJ-1 spelling rules; direct schema validation uses arbitrary-precision plus the narrow patch in launch §3; SQL conversions P2C/P2E deferred |
| 1.5 | Unknown well-formed wire status parses; known-status presence and unconditional kind invariants, non-capability receipts absent on ALL statuses including unknown | **IMPLEMENTED_SCOPED** | Rust restriction and schema mismatch corrected in coordinator integration; wire P2A, unknown execution blocking P2F deferred |
| 1.6 | Exact O/L/P rules, C1 refusals retained, Unicode White_Space pinned identically Rust/schema | **IMPLEMENTED_SCOPED** | Accepted ADR-0023 complete validation; types.rs validators, every provider_reference and event.schema covered |
| 1.7 | ULID [0-7] then25; exact idk_/sha256:; exact goallatch subtraction | **IMPLEMENTED_SCOPED** | Generated ECMA-262 patterns and independent expected cases; no event1 O widening |
| 1.8 | B3 structural/operational distinction; no resource bounds added | **ACCEPTED** | ADR-0020 semantic clarification; minor in isolation, not patch |
| 1.9 | All SCJ-1/digest/duplicate parser/IDK-1 + sha2 0.11 no defaults | **IMPLEMENTED_SCOPED** | Accepted ADR-0019 full primitive decision; canonical module/root API, names AND values framed, no action2 without primitives |
| 1.10 | Historical p.r.list A/B retained privately, typed API rejects invalid ID; typed pp.rr.list generic scalar + legal objects tested | **IMPLEMENTED_SCOPED** | Any SCJ-1 root in generic derivation, ActionRequest remains object-root; no legal-ActionRequest collision or approval-transfer claim |
| 1.11 | Canonical text parser rejects duplicates before Value; raw provenance is caller boundary obligation | **IMPLEMENTED_SCOPED_WITH_LIMITATION** | Cannot reconstruct discarded duplicate keys or prove original raw text at type level |
| 1.12 | Integer-only SCJ-1 limited domain; model temperature f64 wire unchanged | **IMPLEMENTED_SCOPED_WITH_LIMITATION** | Future runtime refuses noncanonical model digest documents; future fraction decision, no truncation |
| 1.13 | Accepted 0018/19/20/23 after corrected docs GREEN; runtime 0021/22/24 remain Proposed | **RATIFIED** | 0018 architectural/wire acceptance, runtime deferred; 0024 wire member implemented, not full fencing or full workspace/MSRV closure |

## 2. Before P2B — Clock only

Clock/time conversions and single TestClock now_ms authority remain P2B. No
canonical primitive or sha2 dependency can be deferred to this phase. Model f64
support does not imply canonical float support; a future decision must specify
fraction range/encoding before digested storage admits it.

---

## 3. Before P2C — historical dependency evidence and current direction

| # | Decision | Status | Evidence | Owner / phase | Needs 6.1 Sol reasoning? |
| --- | --- | --- | --- | --- | --- |
| 3.1 | **The workspace MSRV stays at 1.85. No rise is required.** | **READY** | The previous row here claimed `libsqlite3-sys` 0.38.x "declares `rust-version = "1.88.0"` and `edition = "2024"`" and made an MSRV rise the one genuine owner decision in P2. **Both halves are false.** `libsqlite3-sys-0.38.2/Cargo.toml` has **no `rust-version` field** and is **`edition = "2021"`**; `rusqlite-0.40.2` likewise declares no MSRV. Decisive: **`cargo +1.85.0 check` and `cargo +1.85.0 run` both succeed** on `rusqlite 0.40.2` with `default-features = false, features = ["bundled"]`, compiling the SQLite amalgamation and returning `sqlite_version() = 3.53.2`. Both crates publish the policy *"Latest stable Rust version at the time of release. It might compile with older versions."* — for a 2026-08-08 release that is 1.97.1, which is a floor on their CI, not on this workspace. `Cargo.toml` and `.clippy.toml` are **unchanged** | **RATIFIED by owner direction.** MSRV remains 1.85; historical probe evidence retained | **No** |
| 3.2 | `rusqlite` **0.40.2**, `default-features = false, features = ["bundled"]` | **READY** | Bundles SQLite **3.53.2** (`SQLITE_SOURCE_ID` `2026-06-03 19:12:13 d6e03d8c…`, read from `sqlite3.h`, `sqlite3.c`, and a live `SELECT sqlite_version()`). The earlier `3.53.4` / `2026-07-24` was a transcription error. 20 packages compiled on `aarch64-apple-darwin`; minimal and sufficient | P2C | No |
| 3.3 | `default-features = false` is **required**, not tidiness | **READY** | `rusqlite`'s defaults are `["cache", "ffi-sqlite-wasm-rs"]`, pulling `hashlink`+`hashbrown`+`foldhash` and `sqlite-wasm-rs`. Measured: 11 packages compiled with defaults, 20 without (the chosen set is larger only because it adds `cc` to compile SQLite — which is the point) | P2C | No |
| 3.4 | `libsqlite3-sys`'s defaults select **system SQLite**; `bundled` overrides the *discovery path* | **READY, with a correction** | Its default feature is **`min_sqlite_version_3_34_1`**, expanding to `["pkg-config", "vcpkg"]` — the earlier row's `min_sqlite_version_3_45_3` was **wrong**. Precision that matters: `rusqlite` declares `libsqlite3-sys` **without** `default-features = false`, so those two build deps are still *compiled*; what `bundled` overrides is that `build.rs` never *consults* them. Verified: `otool -L` on the built binary lists **no `libsqlite3`** and the bundled source-id string is present — statically linked | P2C | No |
| 3.5 | `bundled-full` rejected | **READY** | **81** packages compiled on `aarch64-apple-darwin` against 20 for the chosen set: `chrono`, `jiff`, `time`, `serde_json`, `url` (+ the whole `icu_*`/`idna` tree), `uuid`, `csv`, `series`, `vtab`, `window`, `load_extension`, `unlock_notify`, `column_metadata`, `trace`, `hooks`, `backup`, `collation`, `limits`. Nothing in §7.2's table needs any of it | P2C | No |
| 3.6 | **Two connection profiles, not one** | **READY, with corrected measurements** | `:memory:` reports `journal_mode = memory`, and `PRAGMA journal_mode=WAL` there returns `memory` — **silently ignored, not an error**, which is why the profile asserts by read-back. Two earlier cells were wrong: reading `PRAGMA synchronous` on `:memory:` returns a row with value **`2`**, not `1`; and `wal_checkpoint(TRUNCATE)` there returns **one row `0`**, not `(0, -1, -1)`. Also: *setting* `synchronous` returns no row on the **file-backed** profile too, so that is ordinary assignment-pragma behaviour, not an in-memory quirk. `open_in_memory` still cannot satisfy frozen ADR-0005 — for the stronger reason that it has no WAL, no sidecars, cannot be reopened, and loses the schema on close (`no such table`) | P2C | No |
| 3.7 | The in-memory profile asserts `memory`, and does **not** assert `synchronous` | **READY** | Asserting `FULL` where nothing can be fsynced asserts nothing. A test that skips the assertion is the failure mode this closes. The profile is **never** described as durable, persistent or WAL-backed anywhere in the package | P2C | No |
| 3.8 | **Four integrity tiers, with `foreign_key_check` added** | **READY** | Measured: on a database with an FK-orphaned row, `quick_check` → `ok`, `integrity_check` → `ok`, `foreign_key_check` → reports `("q", rowid 1, "p", fkid 0)`. The first two are **page-level** checks and cannot support any claim about referential integrity. Additionally: both page-level pragmas return **multiple rows** on damage, so the check is "no row differs from `ok`", never "the first row equals `ok`"; and `SELECT count(*)` returned the correct `500` on a database with two overwritten pages, so **a successful read is not an integrity signal** | P2C | No |
| 3.9 | `foreign_key_check` belongs in the **post-migration** tier | **READY** | A migration that produced dangling references has failed in a way `quick_check` cannot see, and this schema leans on foreign keys for both the cascade delete and the anti-laundering property | P2C | No |
| 3.10 | `PRAGMA foreign_keys` is set **explicitly**, and its bundled default is `ON`, not `OFF` | **READY, with a correction** | The earlier row said it "defaults to `OFF`". Under `bundled` it defaults to **`ON`**, because `libsqlite3-sys` compiles with `-DSQLITE_DEFAULT_FOREIGN_KEYS=1`; upstream SQLite's own default is `OFF`. The store still sets and asserts it, so the guarantee is *observed* rather than inherited from a build flag. The **claim** is unchanged: one line turns it off and then an orphan is accepted, so every `CHECK`/trigger/`FOREIGN KEY` holds only against a writer who leaves it `ON` — and the threat model already excludes a local file writer. Measured: `PRAGMA foreign_keys` is a no-op **inside** a transaction and the setting is **discarded**, not deferred, which is why it is set at open and never inside a migration | P2C | No |
| 3.11 | `temp_store` stays default, not `MEMORY` | **READY** | A temp table spills to a file that is not at-rest protected, and ADR-0022's protection covers `blobs.content`, not SQLite's scratch space | P2C | No |
| 3.12 | **`TempStore` identity is `<binary>-<pid>-<atomic-counter>`** | **READY** | Measured: a counter alone collides across binaries (two binaries each counting from 0 produce the same three names); a pid alone is not unique within a process, and `cargo test` runs tests as threads while the crash harness spawns children. No clock, no RNG — the `.clippy.toml` ban holds | P2C | No |
| 3.13 | The migrated object inventory is **10 tables, 7 triggers, 6 indexes** | **READY** | Extracted from the document and built. Asserting it is what makes a phantom object impossible — the direct regression for the `leases_generation_matches_step` trigger §4.6 published and §4.0 never contained | P2C | No |
| 3.14 | **`bundled` beats system SQLite on a concrete corruption bug, not only on reproducibility** | **READY** | SQLite's WAL documentation records the **WAL-reset bug** as present in all versions "from 3.7.0 … through 3.51.2 (2026-01-09)", fixed in **3.51.3 (2026-03-13)** and later; published backports are `3.44.6` and `3.50.7`. It can corrupt a WAL-mode database when two connections write and checkpoint concurrently — Serea's shape, given the second connection used for lease and recovery tests. Bundled **3.53.2** is past the fix; this host's system SQLite is **3.43.2**, which is not, and no backport covers it | P2C | No |
| 3.15 | `sha2` **0.11.0**, `default-features = false` | **IMPLEMENTED_SCOPED** in P2A | `rust-version = "1.85"` — **exactly** the workspace MSRV, and unlike the SQLite crates this one is actually pinned. MIT OR Apache-2.0. `default-features = false` suffices: P2 computes a SHA-256 digest, so neither `alloc` nor `oid` is needed. Two implementation traps recorded in [design §7.4](P2-storage-task-engine.md#74-rusqlite-and-the-alternatives): `finalize()` returns `Array<u8, …>` which **does not implement `LowerHex`** (measured compile error — a break from 0.10), and `cpufeatures` selects an `aarch64-sha2`/`x86-sha` hardware backend at runtime that yields byte-identical digests | P2A | No |

Current P2A precision wiring is separate from the historical SQLite probes above:
`serde_json/raw_value`, `jsonschema/arbitrary-precision`, exact `jsonschema =0.58.3`
and the narrow `vendor/jsonschema-value` patch are implemented in the coordinator's
tree. [Launch §3](P2-6.1-sol-launch.md#3-dependency-lines-current-p2a-integration-and-p2c-candidate)
records exact integer-classification/checked-overflow scope and limits; this is
not unrestricted exact schema arithmetic. SQLite remains a P2C candidate, with
no storage/runtime introduced by P2A. N3 also requires the exact
`serde_json =1.0.151` pin and companion `vendor/serde_json` transport patch:
private synthetic keys use a Serde newtype, while literal JSON keys remain ordinary
object members through raw parsing, Value replay and flatten buffers. This preserves
opaque JSON without disabling precision or reserving wire keys.

---

## 4. Before P2D–P2E — blobs, classification, leases

| # | Decision | Status | Evidence | Owner / phase | Needs 6.1 Sol reasoning? |
| --- | --- | --- | --- | --- | --- |
| 4.1 | `put_blob` takes `&[u8]`; there is no `put_blob_value` | **READY** | Requires original text at the raw-input boundary; a bytes API alone cannot prove provenance | P2D | No |
| 4.2 | `PRIMARY KEY (digest, data_class_rank)` is sufficient for P2 | **READY** | Measured: the same digest at two classes stores two rows; the same digest at one class twice is refused. Cross-class laundering via a `PUBLIC` reference resolving a `PRIVATE` blob is prevented **while `foreign_keys` is on** | P2D | No |
| 4.3 | `StoreError::ClassEscalationRequired` is removed, not left as a dead arm | **READY** | Unreachable by construction once the composite key pins a reference's class to the blob's | P2D | No |
| 4.4 | `SECRET`/`CREDENTIAL` are unconstructible on **all five** classified tables | **READY** | Measured, all 8 probes: `tasks`, `blobs`, `side_effect_receipts`, `plan_revisions`, `task_journal`. The earlier draft's claim was true of the blob store and false of everything else | P2D | No |
| 4.5 | `PRIVATE` with no backend is **refused**, with nothing written | **READY** | ADR-0022. Fail-closed means precisely that: an error, and no plaintext fallback, no "encrypt later", no warning-and-continue | P2D | No |
| 4.6 | `size_bytes = length(content)` is a **consistency** invariant, not a bound | **READY** | ADR-0020: it rejects a torn length without inventing a ceiling. Measured: a mismatched length is refused | P2D | No |
| 4.7 | Four classified `TEXT` columns are enforced by **one write chokepoint**, and that is weaker | **READY_WITH_LIMITATION** | `title`, `result_summary`, `error_message`, `effect_summary` can hold `PRIVATE` prose and a `CHECK` cannot record a per-column class. The guarantee is `Tx::put_classified_text` being the only path — claimed as weaker, not as equivalent | P2D | No. Do not upgrade the claim |
| 4.8 | `acquire_lease` is two statements in ADR-0024's order | **READY** | Executed verbatim. First acquisition → `LEASED`, `attempt = 1`, `generation = 1`; expiry reclaim → `generation` 1→2, `attempt` 1→2. **With the §4.6 trigger added, the first acquisition aborts** | P2E | No |
| 4.9 | There is **no** `leases_generation_matches_step` trigger | **READY** | §4.6 published one; §4.0, §7, ADR-0024 and the design's §13.1 had all removed it. Following §4.6 made the first acquisition fail — the round-2 blocker, reintroduced | P2E | No |
| 4.10 | The stale-generation commit returns **0 rows** | **READY** | Verified within one connection and **across two independent connections on one file**, so a process-local mutex cannot be what makes the test pass | P2E | No |
| 4.11 | `attempt` increments at acquisition **only** | **READY** | Measured `attempt == 1` after both `acquire` and `begin_attempt`. Double-charging made `max_attempts_per_step = 3` buy one | P2E | No |
| 4.12 | **An expiry reclaim spends an attempt**, so the bound is on acquisitions | **READY_WITH_LIMITATION** | Measured: a crash-only loop against a ceiling of 2 is stopped after 2 acquisitions with **0 executions**. Correct — counting a crash is the only way `attempt` distinguishes a crash from a retry — but the effective execution budget is `max_attempts_per_step − crashes`, and exhaustion by crashes yields `BLOCKED`, not `FAILED` | P2E | No |
| 4.13 | The ceiling refusal **rolls back**, and what it leaves differs by case | **READY** | Ceiling 0: step `PLANNED`, `attempt = 0`, **no `leases` row**. Ceiling 2, third acquisition: step reverts to `('LEASED', 2, 2)` with one `leases` row | P2E | No |
| 4.14 | `renew` refuses an expired lease, including at exactly `now` | **READY** | Executed: `expires_at_ms > :now_ms` accepts before expiry and returns 0 rows at expiry and after | P2E | No |

---

## 5. Before P2F–P2G — engine and recovery

| # | Decision | Status | Evidence | Owner / phase | Needs 6.1 Sol reasoning? |
| --- | --- | --- | --- | --- | --- |
| 5.1 | The 121-pair transition table is one exhaustive `matches!` | **READY** | `COMPLETED`/`FAILED`/`CANCELLED` have no arm, so `T8` is a property of absence. **All 37 legal pairs constructible; all 84 illegal pairs are accepted by SQL and refused by the engine** — the schema deliberately does not encode the table | P2F | No |
| 5.2 | `Tx` exposes whole transitions, never row-level updates | **READY** | There is no `update_task_state`, no `set_step_status`, no `insert_receipt`, so `T4` cannot be composed wrongly | P2F | No |
| 5.3 | **Historical 32 presence-matrix `N`/`0` probes refused by SQL; not complete error-shape coverage** | **READY** | Was 24/32. Single-column/partial-error cases require the additional gate in §10. The eight gaps — `completed_at`/`result_digest` on `EXECUTING` and `WAITING`, `lease_expires_at` on `WAITING`/`SUCCEEDED`/`FAILED`/`RECONCILED_ABSENT` — closed by three additive constraints. All 51 constructible cells and all 37 transitions still construct | P2F | No |
| 5.4 | `lease_expires_at` gets a **biconditional**, not an implication | **READY** | `lease_owner` was already biconditional, so the pair was half-constrained: a terminal step could carry an expiry with no owner. ADR-0024 clears both together, so no designed path produces it — but a future writer clearing only `lease_owner` would pass | P2F | No |
| 5.5 | All 56 `kind × status` cells: **51 constructible, 5 correctly refused** | **READY** | The 5 are `WAITING` on a non-wait kind. Asserting 56/56 would assert the opposite of ADR-0018 | P2F | No |
| 5.6 | `receipt ⇒ SUCCEEDED` is a trigger; `RECONCILED_ABSENT ⇒ no receipt` is recovery's scan | **READY_WITH_LIMITATION** | A cross-table property no `CHECK` can express. Stated as detected-not-prevented | P2G | No |
| 5.7 | Recovery runs `foreign_key_check` **before** classifying | **READY** | Without it, §9.1 row 3b's "spanning tables" case is not decidable. It is the only integrity pragma that sees a referential violation | P2G | No |
| 5.8 | Recovery never executes | **READY** | `ExpiredLease` never becomes blind re-execution. P2 cannot read `replay_safety` — it lives in a descriptor and there is no registry — so P2 records the decision durably and P5 acts on it | P2G | No |
| 5.9 | `pending_event_transitions` needs no column | **READY** | With `event_seq` dropped, in P2 every journal row is pre-event history, so the count is the row count. It is the size of the window during which `E3` did not hold — a fact to display, not a queue to drain | P2G | No |

---

## 6. Before P2H — fault injection

| # | Decision | Status | Evidence | Owner / phase | Needs 6.1 Sol reasoning? |
| --- | --- | --- | --- | --- | --- |
| 6.1 | **A deterministic mid-`COMMIT` abort is not injectable through `rusqlite`** | **READY** | Every injection point was worked through. Returning `Err` before `execute_batch("COMMIT")` reaches nothing inside SQLite; `SIGKILL` in flight is timing-dependent; `SQLITE_TESTCTRL` / a fault VFS / a SQLite fault build needs a custom build; a second writer cannot interrupt a commit because SQLite serialises writers | P2H | No |
| 6.2 | N7 is a **stress test with weak assertions** | **READY** | Assert `quick_check` is `ok`, the database opens, `foreign_key_check` is empty. **Never** an exact row count. N5 (pre-commit) plus N6 (post-commit) are the deterministic coverage | P2H | No |
| 6.3 | N6 — crash after `COMMIT`, before the caller observes `Ok` — is the most valuable window | **READY** | Measured with `SIGKILL` in a child process: 0 rows before commit, 1 row after, `quick_check` ok, and SQLite recovers the stale `-wal` on next open | P2H | No |
| 6.4 | Two OS processes writing one file both succeed | **READY** | Measured: 40 of 40 writes landed, `quick_check` ok, `busy_timeout` serialising correctly | P2H | No |
| 6.5 | The injection point must be inert when unused | **READY** | A test asserts the hook list is empty in a release-configuration build, so an inert hook cannot become a hidden code path | P2H | No |

---

## 7. Deferred on purpose

| # | Item | Status | Why deferring is safe |
| --- | --- | --- | --- |
| 7.1 | A real `AtRestProtection` backend | **SAFE_DEFER** | ADR-0022 refuses `PRIVATE` with no backend, writes nothing, and says so. The trait plus a test-only double prove the wiring. P2 can receive PRIVATE values and must refuse them without a backend |
| 7.2 | The `SECRET` sealed store | **SAFE_DEFER** | No crate owns it anywhere in the architecture. P2 refuses `SECRET` at the storage layer, which is the correct posture, not a gap |
| 7.3 | `NOTIFY`'s eventual capability shape | **SAFE_DEFER** | ADR-0018 §4 makes it host-internal and names the ADR that would change it. The obligation is recorded |
| 7.4 | Resource-bound numeric values | **SAFE_DEFER** | P0's gap, still open. Inventing a number with no measurement behind it is the `MAX_VALUE_LENGTH` mistake P1 already retracted. §12 keeps this visible and does **not** claim closure |
| 7.5 | `insert_at` plan revisions | **SAFE_DEFER** | P2 V1 is append-only; the cost is that a mid-plan insertion needs a new task, and ADR-0018 §5 names the relaxation |
| 7.6 | `max_concurrent_steps_per_task` | **SAFE_DEFER** | Correctly **not** enforced. It is an engine convention with no `CHECK`, trigger or partial index, so it is not structural and is not claimed |
| 7.7 | The remaining §2 bounds | **SAFE_DEFER** | Counters and configuration belong to `serea-core`. P2 exposes the durable facts each bound's owner needs |
| 7.8 | Retention (the 30-day trigger) | **SAFE_DEFER** | P12's, because the notification surface and the bound configuration are both `serea-core`'s. P2 provides `delete_task`; P2 enforces no retention bound, and that is a non-claim |
| 7.9 | A canonical-number dependency | **SAFE_DEFER** | SCJ-1 refuses every `f64`, so P2A needs no float formatter. Fraction encoding/range needs a future decision |
| 7.10 | Empirical Apple Silicon verification | **SAFE_DEFER** to CI | The design contains nothing architecture-dependent — established by exhaustive source audit — but this host is `x86_64`. The confirmation is the cross-architecture fixture job, and that job is the deliverable |

---

## 8. Explicitly not claimed

Restating these so tomorrow's closure document cannot drift into them by
accident. Each is a **non-claim**, not a pending item.

| Not claimed | Why it would be false |
| --- | --- |
| `E3` | P2 writes no `SereaEvent`. `E3` holds forward from P3's first migration and **never held** for P2-era transitions |
| `E4` | No `seq` exists in P2 |
| `C4`'s second half | `side_effect_class` lives in a descriptor and there is no registry in P2 |
| `T6`'s comparison | The plan path checks `risk_class ≤ policy_class` only when a descriptor is available, and in P2 it never is. Recorded as a non-claim, not a partial enforcement |
| Any §2 bound other than `max_attempts_per_step` | See §7 |
| Any `PRIVATE` at-rest support | P2 ships the trait and the refusal, not a backend |
| The resource-bound gap | P0's gap, unchanged |
| Tamper-evidence against a local file writer | Security Invariants §6 records it "Not specified". Every `CHECK`, trigger and `FOREIGN KEY` here holds against a writer who leaves `foreign_keys = ON` and `ignore_check_constraints = OFF`, and that is the whole of the claim |
| `NOTIFY` rendering | `serea-core` renders it in P12 |

---

## 9. Historical handoff decisions (superseded by owner direction)

**Historical handoff text below; not current open decisions.** Owner request
ratifies version/migration direction and MSRV 1.85; current production obligations
are in §1 and the frozen gate. The earlier list was three items and became two. Item 2 below used to be an MSRV
decision. It was not one: the premise was a misreading of `libsqlite3-sys`'s
manifest, and `cargo +1.85.0` builds and runs the chosen configuration. See row 3.1
and [design §7.4](P2-storage-task-engine.md#74-rusqlite-and-the-alternatives).

1. **1.1** — ratify the version plan (`serea-arch/0.2.0 → 1.0.0`,
   `serea.task/1 → 2`, `serea.action/1 → 2`, `serea.event/1` unchanged). §4.1
   decides the classification; the owner signs it.
2. **1.2** — ratify the migration notes for **both** `serea.task/2` **and**
   `serea.action/2`. Drafts for both now exist in
   [the launch package](P2-6.1-sol-launch.md#4-migration-note-drafts), so this is
   reading and signing prose rather than writing it. Note the correction: the
   earlier version of this ledger described the major migration-note task as
   "mainly `serea.task/2`", which understated it — `serea.action/2` is equally
   major and needs its own note.
3. **3.1** — ratify the MSRV **finding**: the workspace stays at `1.85`, and
   `Cargo.toml` and `.clippy.toml` need no change at all. There is no
   `NEEDS_RATIFICATION` row left in this ledger, because there is no longer a
   choice to make — the evidence closed it.
4. **Implementation ordering within P2A.** The ADRs list their code changes; the
   only ordering question is whether the Rust `StepPresence` matrices or the JSON
   Schema `if`/`then` clauses land first, and they must land in **one commit**, so
   this is sequencing rather than design. The recommended sequence is in
   [the launch package](P2-6.1-sol-launch.md#9-p2a-recommended-implementation-order).

Everything else in this ledger is a `READY` row pointing at executable evidence or
a frozen contract. Nothing is `NEEDS_REDESIGN`. Nothing is `BLOCKED`. **No
unexplained open placeholder remains.**

### 9.1 Corrections made by the final closure run

Recorded here so that a reader who trusts an earlier row knows which rows moved, and
why. Full evidence in [design §7](P2-storage-task-engine.md#7-migrations-and-connection-policy)
and [§7.4](P2-storage-task-engine.md#74-rusqlite-and-the-alternatives).

| # | Earlier claim | Verified reality |
| --- | --- | --- |
| 5.1 | `libsqlite3-sys` requires Rust 1.88 / `edition = "2024"`, so the MSRV must rise | **No `rust-version` field; `edition = "2021"`; builds and runs on 1.85.0.** No rise |
| 5.2 | Bundled SQLite is `3.53.4` (2026-07-24) | **`3.53.2`**, `SQLITE_SOURCE_ID` `2026-06-03 19:12:13` — read from source and confirmed at runtime |
| 5.3 | `libsqlite3-sys` default is `min_sqlite_version_3_45_3` | **`min_sqlite_version_3_34_1`**. `bundled` overrides the discovery path, not the compilation of `pkg-config`/`vcpkg` |
| 5.4 | `PRAGMA foreign_keys` defaults to `OFF` | **`ON` under `bundled`** (`-DSQLITE_DEFAULT_FOREIGN_KEYS=1`). The integrity claim is unchanged; the store asserts the pragma rather than inheriting it |
| 5.5 | `:memory:` returns `synchronous = 1`, and `wal_checkpoint(TRUNCATE)` returns `(0, -1, -1)` | **Reading** `synchronous` returns a row valued `2`; `wal_checkpoint(TRUNCATE)` returns **one row `0`**. Separately, *setting* `synchronous` returns no row on the **file-backed** profile too, so that is ordinary pragma behaviour, not an in-memory quirk |
| 5.6 | Copy `serea.sqlite` + `-wal` + `-shm` as a three-file unit | **`-shm` is a transient, rebuildable artifact** in native byte order; the normal path copies the main file alone after a clean stop, and the abnormal path copies main + `-wal` and still never `-shm` |
| 5.7 | The major migration-note task is "mainly `serea.task/2`" | **`serea.action/2` is equally major** and needs its own note. Both drafts now exist |

The two BLOCKERs the earlier audit found — the inert category-O pattern and the
phantom lease trigger — remain fixed in the documents, and each has a named test that
fails without the fix: **A6/A6a/A6b/A6c** and **F28 + H1**.

## 10. Deferred design corrections, not P2A runtime claims

- SQL error=N means every error column including details absent outside FAILED;
  FAILED requires five mandatory fields and permits absent details. Single-column
  and partial-tuple regressions must supplement historical 32-cell evidence.
- TransactionParticipant uses shared record(&self); successful body returns immutable
  transition(s), body error propagates before participants, then same-transaction
  participants and commit. No detached prebuilt identity, no event_seq/backfill;
  P2 pending count is journal row count, not a queue.
- Outcome fence EXISTS authoritative lease matching owner/generation/unreleased.
  Release permanently revokes that generation. Expired but unreclaimed/unreleased
  may commit known outcome; renewal/begin require unexpired lease. begin_attempt
  borrows nonclone guard. Tests and runtime remain P2E/P2F.
- Prior READY/69-of-69/32-cell experiment evidence is historical and does not
  establish these added regressions or production implementation.
