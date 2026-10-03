# P2 Autonomous Pre-Implementation Audit

- **Branch:** `p2/autonomous-preimplementation-audit`
- **Base commit:** `ec4659c007a914e5d90bb3067d3858a4e299b797` (`p2/design-preparation`)
- **Audit commit:** `732b3ad92801dcb15d5b45548cbb23f325abafe0` — this document's
  own commit, on `p2/autonomous-preimplementation-audit`
- **Audit date:** 2026-10-03
- **Scope:** design hardening only. No production Rust, no runtime crate, no
  migration file used by production, no `Cargo.toml`/`Cargo.lock` change, no CI
  implementation change, and no frozen protocol text changed.
- **Status of the design package after this run:** the two BLOCKERs are fixed in
  the documentation, every MAJOR is either fixed or recorded as an owner
  decision, and the executable evidence is reproducible from the harness this
  run built.

This document is the audit record. It is **not** a closure record; P2
implementation has not started.

---

## 0. Preflight, as recorded

| Check | Result |
| --- | --- |
| cwd | `$HOME/Desktop/Serea` |
| Branch at start | `p2/design-preparation` |
| HEAD at start | `ec4659c007a914e5d90bb3067d3858a4e299b797` |
| Worktree | clean |
| P1 parent baseline | `c3737039e3e38dbba554dc0b9075025f87948358` (`p1/workspace-protocol-skeleton`) present |
| Host | macOS 15.7.7, `x86_64`, rustc 1.98.1, SQLite 3.43.2 (system + Python `sqlite3`), Node 24.21.0 |

The audit branch was created from the exact commit above. The
`design-preparation` commit was not rewritten or amended.

---

## 1. Method

Four audit modes, run in four rounds with different perspectives. Findings were
frozen at the end of each round before any document was edited, which is the
discipline the design package's own §13.3 identified as its repeated failure.

| Round | Mode | Perspective |
| --- | --- | --- |
| 1 | A — independent re-derivation | What would P2 need, derived from P0/P1/crate map/threat model *without* reading the P2 conclusions, then compared |
| 2 | B — executable adversarial validation | Run the DDL, the ADR statements, the lease sequences, the pragmas, the migration edge cases |
| 3 | C — cross-product state audit | `kind × status × lease × receipt × digest × error × journal × revision`, positive **and** negative |
| 4 | D — unknown-unknown / future-change | What would tomorrow's coding agent still have to invent, and what assumption dies at P3/P5/P12 |

Then: independent re-reviews (code, security, architecture/portability) over the
corrected package, and one final bounded sweep.

**The harness is not transcribed.** The DDL is extracted programmatically from
`P2-sqlite-schema.md` §4.0 at run time, so the executed schema is the document's
schema. Every experiment is re-runnable from `tmp/audit/` (gitignored).

### 1.1 Executable experiment inventory

| Harness | What it executes |
| --- | --- |
| `tmp/audit/ddl.py` | Extracts §4.0's migration verbatim, migrates a database, reports the object inventory |
| `tmp/audit/scj1.py`, `t8_vector.py`, `t8_idk.py` | Independent SCJ-1 and IDK-1 implementation from ADR-0019 prose; recomputes all 17 published vectors and §13.2's byte layout |
| `tmp/audit/catO.mjs`, `catO_rule.mjs`, `catO_final.mjs` | ADR-0023's category-O pattern and candidate replacements, under ECMA-262 (`node`) |
| `tmp/audit/passC_grid.py` | 8 step kinds × 7 statuses; all 121 task-transition pairs |
| `tmp/audit/passC_presence.py` | One positive/negative probe per ADR-0018 §3 presence-matrix cell |
| `tmp/audit/passB_lease.py` | ADR-0024's five statements verbatim, in the prescribed order; two connections; two processes; busy timeout |
| `tmp/audit/passB_storage.py` | Attempt ceiling, blob store, classification on every classified table, integrity pragmas, WAL profiles, `ignore_check_constraints`, migration edges, temp-name uniqueness |
| `tmp/audit/pragma_boundary.py` | Isolates whether foreign keys and triggers survive `ignore_check_constraints` |
| `tmp/audit/phantom_trigger.py` | Executes the §4.6 trigger that §4.0 does not contain |
| `tmp/audit/crash.py` | SIGKILL before/after COMMIT in a child process, stale WAL, two-process writers |
| `tmp/audit/f64probe` | Rust `f64` Display vs RFC 8785 Appendix B |

---

## 2. Findings

Severity: **BLOCKER** — P2 implementation would encode the wrong architecture or
be unable to satisfy a frozen invariant. **MAJOR** — a frozen claim, a stated
guarantee, or an implementability requirement is false. **MINOR** — an
inconsistency or an implementation trap with a bounded blast radius. **NOTE** —
recorded, no action.

---

### B1 — BLOCKER — ADR-0023's category-O pattern does not implement its own prose

**Affected:** `docs/decisions/ADR-0023-text-field-validation-categories.md`
lines 112–113, and the Consequences claim at line 128.

**Evidence.** The drafted pattern is

```
^(?![ \t\n\r\f\v]*$)(?!.*:(tsk|stp|apr|grt|req|evt|dev|sch|prop|rcp|ses)_)
 [^\u0000-\u001f\u007f]+$
```

The negative lookahead requires a **literal colon immediately before** the
prefix. No Serea identifier has one: `tsk_01JQ8Z9K3M7QWXR4V2T6YH0BNA` contains no
`:` anywhere. Executed under ECMA-262 (`node`, which is what JSON Schema `pattern`
is):

```
value                               drafted  prose
tsk_01JQ8Z9K3M7QWXR4V2T6YH0BNA      ACCEPT   REFUSE   <- divergence
stp_01JQ8Z9M3R2CVN8H5FWK7PQDSF      ACCEPT   REFUSE   <- divergence
rcp_01JQ8ZF4T7KMV2X9NPQ5RD8WCS      ACCEPT   REFUSE   <- divergence
idk_<64 hex>                        ACCEPT   REFUSE   <- divergence
sha256:<64 hex>                     ACCEPT   REFUSE   <- divergence
calendar.events.list                ACCEPT   REFUSE   <- divergence
nemotron-3-nano-30b                 ACCEPT   REFUSE   <- divergence
```

7 divergences out of a 14-case corpus. The rule fires **only** on strings of the
form `a:tsk_`, `x:stp_…`, `sha256:tsk_…` — none of which can occur as an
`ActorId`, `LeaseOwner` or `ProviderReference`. It includes the exact case
ADR-0023's own adversarial corpus names: *"a `stp_`-prefixed ULID in an O
field"*.

**Why it matters.** The impersonation rule is the only non-whitespace rule in
ADR-0023 and the ADR states its purpose: *"a `ProviderReference` that happens to
parse as a `stp_` ULID could be rendered in an audit row as though it identified
a step."* As drafted it prevents nothing, so P2A's test A2 cannot pass and the
ADR's claim *"Every field's rejection must be identical on both sides"* is false
in the one direction that matters.

**Disposition — accepted, and the intended rule had to be decided, not just
repaired.** Issue 7.3's three options were decided by execution, not by
preference:

| Option | Impersonations caught | False positives on legal opaque tokens |
| --- | --- | --- |
| A — Serea ULID family only | 11 / 16 | 0 / 14 |
| B — every registered identifier domain | 15 / 16 | **8 / 14** |
| **C — the syntactically distinctive domains** | **16 / 16** | **0 / 14** |

**B is untenable, and the evidence is specific.** `ProviderId` is
`[a-z][a-z0-9_]{1,31}` and `ModelId` is `^[a-z0-9]+(-[a-z0-9]+)*$`. Both subsume
ordinary words. Under B, the refused set includes `calendar`, `worker`,
`worker-1`, `host-a3f9`, `x`, `w` — that is, **every plausible `LeaseOwner`** and
most plausible `ProviderReference`. A rule that refuses its own intended input is
not a stricter rule; it is a dead one.

So the answer is **C**, narrowed, with the exclusion *stated* rather than silent:

> Category O refuses a value that parses as a **prefixed or fixed-shape** frozen
> identifier: the eleven ULID prefixes, `idk_` + 64 lowercase hex, `sha256:` + 64
> lowercase hex, or a `CapabilityId`. It does **not** refuse `ProviderId`,
> `ModelId` or `ImplementationId`, because those grammars subsume ordinary words
> and the rule would refuse every legitimate value.

**Correction applied.** ADR-0023 now carries the working pattern, generated from
the frozen lists rather than hand-written, and it **is** expressible in JSON
Schema, so Rust/JSON-Schema parity is real rather than asserted:

```
^(?![ \t\n\r\f\v]*$)
 (?!(tsk|stp|apr|grt|req|evt|dev|sch|prop|rcp|ses)_[0-9A-HJKMNP-TV-Z]{26}$)
 (?!(idk|sha256):?[0-9a-f]{64}$)
 (?!(?!goallatch\b)[a-z][a-z0-9_]{1,31}\.[a-z][a-z0-9_]{1,31}\.(list|read|search|open|control|write|create|send|delete|start|status|run|cancel|result)$)
 [^\u0000-\u001f\u007f]+$
```

One parity subtlety is recorded rather than left to the implementer: the
`goallatch` exclusion. `serea-protocol`'s `CapabilityId` validator refuses the
`goallatch` provider namespace, so `goallatch.goal.run` is **not** a `CapabilityId`.
A banned set that ignored this would refuse a value Rust accepts. The
`(?!goallatch\b)` term makes the banned set exactly equal to `CapabilityId`'s
accept set — verified: 16/16 impersonations caught, 0/14 false positives,
including `calendar.events.reticulate` (unknown verb), `fake-goallatch.goal.run`
(not a ProviderId), `tsk_`, a 27-character ULID body, and a lowercase ULID body.

**Design docs changed:** yes — ADR-0023, and the gap analysis §5.6 restatement.

**Test that pins it tomorrow:** ADR-0023's shared-corpus test, extended so the
`stp_`-ULID case is joined by all eleven ULID prefixes, `idk_`, `sha256:`, a real
and a near-miss `CapabilityId`, and the six legal opaque tokens; plus the
generated-pattern-equals-frozen-lists test, which must now cover `CAPABILITY_VERBS`
as well as the prefix list.

---

### B2 — BLOCKER — §4.6 publishes a lease trigger that is not in the migration, and adding it makes the first lease acquisition impossible

**Affected:** `docs/plans/P2-sqlite-schema.md` §4.6, lines 680–696.

**Evidence.** Three documents disagree, and the executable consequence is
decisive.

| Source | Says |
| --- | --- |
| §4.0 "The whole migration" | `leases_generation_matches_step` occurs **0 times** |
| §7, "Two rows that are accepted by SQL on purpose" | *"The trigger was **removed**."* |
| ADR-0024, `acquire`, third bullet | *"The `leases_generation_matches_step` trigger an earlier draft specified was removed"* |
| Design §13.1, B2 | *"the consistency trigger removed"* |
| **§4.6** | prints the trigger's SQL and asserts *"it is kept consistent by a trigger rather than by a test"* and *"is verified — a mismatched generation is refused"* |

Executed both ways, with ADR-0024's two statements in the prescribed order:

```
WITHOUT the §4.6 trigger  -> first acquire OK (LEASED, attempt 1, generation 1)
                              expiry reclaim OK (generation 2)
WITH    the §4.6 trigger  -> first acquire REFUSED:
                              "lease generation must match the step"
```

The trigger is `BEFORE INSERT ON leases`, and the first acquisition is the
upsert's **INSERT** branch with `generation = 1`, while the step's
`lease_generation` is still `0`. So `1 <> 0` aborts. This is **exactly the
round-2 blocker the design records as fixed** ("No lease could be acquired, in
either order"), reintroduced in the one section that still carries the old SQL.

The trigger is also *semantically* wrong on its own terms, which ADR-0024 already
identified: it fires only on the INSERT branch, so it enforces the invariant in
one of the two branches and silently skips the other — the `leases` upsert's
`ON CONFLICT DO UPDATE` branch never reaches an `INSERT` trigger.

**Why it matters.** §4.6 is the prose an implementer reads to understand the
generation duplication. Following it produces a store in which no lease can ever
be acquired, and it does so *silently*, because §7's own table would then be
contradicted by §4.6 in the same document.

**Disposition — accepted.** §4.6's trigger block is deleted and replaced with the
derived-copy rationale ADR-0024 gives, cross-referenced to §7's acceptance row.
The claim "verified — a mismatched generation is refused" is removed with it,
because it was never true of the shipped schema.

**Design docs changed:** yes — `P2-sqlite-schema.md` §4.6.

**Test that pins it tomorrow:** the P2C migration test asserts the object
inventory of a migrated database — 10 tables, **7** triggers, 6 explicit indexes —
so a phantom trigger cannot be reintroduced without failing a test. The P2E
"first acquisition succeeds from `PLANNED`" test is the direct regression.

---

### B3 — MAJOR — ADR-0018's "a constraint for every row of the presence matrix" is false for 8 cells

**Affected:** `ADR-0018` §Consequences; `P2-sqlite-schema.md` §4.4; the DDL.

**Evidence.** ADR-0018 §3 marks a field `N` = "must be `null`". One probe per
such cell, using the kind that makes the status reachable:

| Matrix cell | Matrix says | SQL |
| --- | --- | --- |
| `completed_at` on `EXECUTING` | `N` | **accepted** |
| `result_digest` on `EXECUTING` | `N` | **accepted** |
| `completed_at` on `WAITING` | `N` | **accepted** |
| `result_digest` on `WAITING` | `N` | **accepted** |
| `lease_expires_at` on `WAITING` | `N` | **accepted** |
| `lease_expires_at` on `SUCCEEDED` | `N` | **accepted** |
| `lease_expires_at` on `FAILED` | `N` | **accepted** |
| `lease_expires_at` on `RECONCILED_ABSENT` | `N` | **accepted** |

All 24 other `N`/`0` cells are refused, and three control rows (a legal
`EXECUTING`, a legal `WAITING`, a legal `SUCCEEDED`) are accepted, so the harness
is not trivially refusing everything.

ADR-0018's stated consequence is *"a constraint for every row of the presence
matrix, so an inconsistent step is unconstructible by any writer, not only by
Rust."* For these eight cells an inconsistent step **is** constructible by a
writer, and `serea-task-engine`'s `StepPresence` validator — which does enforce
the matrix — will refuse to read such a row back. That is a
schema-accepts/Rust-refuses divergence, which is precisely the class of defect
this audit was chartered to find.

The `lease_expires_at` group is the more serious half. The schema already treats
`lease_owner` as a genuine biconditional —
`(status IN ('LEASED','EXECUTING')) = (lease_owner IS NOT NULL)` — so the *pair*
is half-constrained: a terminal step can carry a lease expiry with no owner.
ADR-0024's commit statement clears both columns together, so no designed path
produces it, but a future writer that clears only `lease_owner` passes the schema
and leaves a dangling expiry. That is the same hole the biconditional was added
to close, closed for one column of the pair.

**Why it matters.** The claim is load-bearing in three documents, and it is the
guarantee ADR-0018 exists to provide. It is also the fifth consecutive instance
of the design package's own named blind spot — asserting a constraint rather than
constructing the cell that would expose it.

**Disposition — accepted.** Two additive constraints close all eight cells
without weakening any positive case, and the claims are corrected to state what
is enforced and what is not:

```sql
CHECK (status NOT IN ('PLANNED','LEASED','EXECUTING','WAITING')
       OR completed_at_ms IS NULL),
CHECK (status NOT IN ('PLANNED','LEASED','EXECUTING','WAITING')
       OR result_digest IS NULL),
CHECK ((status IN ('LEASED','EXECUTING')) = (lease_expires_at_ms IS NOT NULL)),
```

Verified after the edit: the DDL still builds, all **51** legitimately
constructible `kind × status` cells still construct, all 37 legal task
transitions still construct, the three controls still construct, and all 32
matrix `N`/`0` cells are now refused.

The one matrix row that genuinely cannot be a `CHECK` — `side_effect_receipt`
absent on `RECONCILED_ABSENT`, a cross-table property — stays detected by
recovery, and ADR-0018 now says so without implying the rest was enforced.

**Design docs changed:** yes — the DDL in `P2-sqlite-schema.md` §4.0, §4.4,
§7; ADR-0018 §Consequences.

**Test that pins it tomorrow:** the P2A matrix suite gains a negative case per
cell, not only per row, and P2C's migration test re-asserts the full 32-cell
refusal set against a migrated database.

---

### B4 — MAJOR — `open_in_memory` cannot satisfy ADR-0005's WAL requirement, so §7.2's single connection policy is unimplementable as written

**Affected:** `P2-storage-task-engine.md` §7.2 (connection policy table),
§7.3 (test database policy), and therefore the P2C exit criteria.

**Evidence.** The **conclusion** of this finding stands and is unaffected: two
named profiles are required, and `Store::open_in_memory` cannot satisfy ADR-0005.
**Three of the five measured cells below were wrong and are corrected here** by the
final closure run; the normative table is
[design §7.2](P2-storage-task-engine.md#72-connection-policy).

| Property | `:memory:` | file-backed |
| --- | --- | --- |
| `PRAGMA journal_mode` | **`memory`** | `wal` |
| `PRAGMA journal_mode = WAL` (set) | returns `memory` — **silently ignored, not an error** | returns `wal` |
| `PRAGMA synchronous` (**read**) | returns a row, value **`2`** — *was recorded as `1`* | `0/1/2/3` honoured |
| `PRAGMA synchronous = FULL` (**set**) | returns **no row** | returns **no row** — *so this is ordinary assignment-pragma behaviour, not an in-memory quirk* |
| `PRAGMA foreign_keys` | **`1` under `bundled`** — *was recorded as `0` by default*; `libsqlite3-sys` compiles `-DSQLITE_DEFAULT_FOREIGN_KEYS=1`, while upstream SQLite's own default is `OFF` | same |
| `PRAGMA wal_checkpoint(TRUNCATE)` | **one row, `0`** — *was recorded as `(0, -1, -1)`* | one row, `0` — *was recorded as `(0, 0, 0)`* |

So §7.2's *"journal_mode: `WAL`, asserted at open"* cannot be asserted by
`Store::open_in_memory`, and *"synchronous: `FULL`"* is unenforceable there because
there is nothing to fsync. ADR-0005 is **frozen and accepted**; a constructor that
cannot satisfy it must not share its assertion list with the constructor that can.

Two corrections sharpen the conclusion rather than weaken it. `journal_mode = WAL`
on `:memory:` **does not error** — it returns `memory` — so a store that set the
pragma and read nothing back would believe it had entered WAL mode. And reading
`synchronous` on `:memory:` returns a truthful `2`, so asserting the *value* would
also pass while meaning nothing. **The in-memory profile must therefore assert
`journal_mode == "memory"` explicitly**, which is what §7.2 now specifies.

The dangerous consequence is not the in-memory store — it is the test. A P2C test
that asserts "WAL at open" through `open_in_memory` has two failure modes, and
both are bad: it fails for the wrong reason, or it is written to skip the
assertion, and then nothing in the suite pins ADR-0005 at all.

**Disposition — accepted.** Two named profiles replace one table:

| | `ProductionProfile` | `TestMemoryProfile` |
| --- | --- | --- |
| `journal_mode` | `WAL`, asserted | `memory`, asserted as `memory` |
| `synchronous` | `FULL`, asserted | **not asserted**; documented as a no-op |
| `foreign_keys` | `ON`, asserted | `ON`, asserted |
| `busy_timeout` | 5000 ms, asserted | asserted |
| clean-close checkpoint | `TRUNCATE` | not performed |
| usable for | everything | pure unit/property tests only |

§7.3 already forbids the in-memory profile for reopen, cascade,
migration-reopen and crash tests; that is now a property of the type rather than
a convention, and the durability-bound test list is stated as a positive rule.

**Design docs changed:** yes — `P2-storage-task-engine.md` §7.2, §7.3, §15.3.

**Test that pins it tomorrow:** P2C asserts `journal_mode = wal` and
`synchronous = 2` **through `Store::open` on a `TempStore` file**, never through
`open_in_memory`; and a separate test asserts that `open_in_memory` reports
`memory`, so the profile split cannot be quietly undone.

---

### M1 — MAJOR — Referential integrity is pragma-dependent, and the design's structural budget does not say so

**Affected:** `P2-sqlite-schema.md` §5.3, §7; `P2-storage-task-engine.md` §11
question 4; ADR-0022.

**Evidence.** §7 discusses exactly one boundary: `PRAGMA
ignore_check_constraints = ON`. Its claim that triggers and foreign keys survive
it is **verified true** — an FK-orphaned `task_steps` row is refused with
`foreign_keys = ON, ignore_check_constraints = ON`, and
`tasks_policy_class_immutable` still aborts.

But `PRAGMA foreign_keys` is an ordinary per-connection setting that **defaults
to `OFF`**, and one statement disables it:

```
PRAGMA foreign_keys = OFF;   -- outside a transaction: takes effect
BEGIN IMMEDIATE;
INSERT INTO task_steps (… task_id …) VALUES (…, 'tsk_…BNB', …);  -- ACCEPTED
COMMIT;
```

No pragma trickery, no special privilege beyond file write, and the orphan
lands. §7's framing — "the controls that survive are the authority-bearing ones,
`REFERENCES` among them" — is therefore true only for a writer who leaves
*both* settings alone, and the second setting is never named.

This matters more than a generic caveat, because of what rests on it. §5.3's
central safety claim is that `PRIMARY KEY (digest, data_class_rank)` plus the
composite foreign keys mean *"a reference at class `X` can only ever resolve bytes
stored at class `X`, and a reader can never widen its own view of a value's
class"*, which is the `DC3` anti-laundering property. That guarantee is enforced
**entirely** by `FOREIGN KEY` clauses and therefore entirely by a pragma.

**Disposition — accepted.** §5.3 and §7 now state the boundary in the same terms
ADR-0022 uses for `ignore_check_constraints`: the composite key is a *structural*
guarantee against a writer that leaves constraint enforcement enabled, and a
*pragma-dependent* one against a local file writer, which the threat model
already excludes from tamper-evidence ([Security Invariants §6](../threat-model/04-security-invariants.md)
records it "Not specified"). No new control is invented, because §11 question 4's
existing reasoning — spend the budget on controls that survive — is right and is
now applied symmetrically.

**Design docs changed:** yes — `P2-sqlite-schema.md` §5.3, §7; ADR-0022.

**Test that pins it tomorrow:** the existing O14/O15 pair is extended with the
`foreign_keys = OFF` case, asserting the orphan lands, so the boundary is a pinned
fact rather than a remembered caveat.

---

### M2 — MAJOR — `foreign_key_check` is in no defined check tier

**Affected:** `P2-storage-task-engine.md` §7.1, §9.1 row 3b; §3.4 `verify_integrity`.

**Evidence.** Each pragma was asked exactly what it verifies, using a database
holding one deliberately orphaned `task_steps` row:

```
PRAGMA quick_check        -> ok          <- page-level damage only
PRAGMA integrity_check    -> ok          <- page-level damage only
PRAGMA foreign_key_check  -> task_steps  <- the only one that sees it
```

`quick_check` and `integrity_check` are **page-level** checks. Neither has any
relationship to referential integrity, so neither can support a claim about
foreign keys. The design's existing wording — `quick_check` "catches page-level
damage" — is accurate and needs no correction; what is missing is the tier
policy. `foreign_key_check` appears only inside recovery row #3b, which means:

- nothing verifies foreign keys at **open**, even though a stale or tampered file
  is exactly the case open-time verification exists for;
- nothing verifies them **after a migration**, even though a migration is the
  moment a schema is created and FK enforcement is per-connection;
- `verify_integrity()` is named but not specified, so an admin invoking it has no
  documented contract.

**Disposition — accepted.** The four tiers are defined:

| Tier | Check | Cost | When |
| --- | --- | --- | --- |
| Normal open | `quick_check` | cheap | every open |
| Post-migration | `quick_check` **+ `foreign_key_check`** | cheap on a fresh schema | after each migration, inside the migration's own gate |
| Explicit admin | `integrity_check` **+ `foreign_key_check`** | O(database) | `verify_integrity()`, on demand |
| Recovery precondition | `foreign_key_check` | cheap | before classifying, so §9.1 row 3b's "spanning tables" case is decidable |

The post-migration `foreign_key_check` is the one that matters most and costs
almost nothing, because a migration that produced dangling references has failed
in a way `quick_check` cannot see.

**Design docs changed:** yes — `P2-storage-task-engine.md` §7.1, §3.4, §9.1.

**Test that pins it tomorrow:** P2C corrupts a file's referential integrity with
`foreign_keys = OFF`, reopens, and asserts the refusal names `foreign_key_check`
rather than a page check. A test that passes `quick_check` here is the test that
would have let M2 through.

---

### M3 — MAJOR — ADR-0021 claims `E3` "for free", and rejects the outbox shape it then adopts

**Affected:** ADR-0021 §"P3 registers a second hook and gets `E3` for free";
§"What P2 can honestly claim"; the rejected-alternatives row.

**Three distinct problems, in increasing order of seriousness.**

**(a) The internal contradiction.** ADR-0021 rejects *"A `pending_event` outbox
drained by P3"* with the reason: *"the state change is already committed without
its event, so `E3` stays violated and **the gap is permanent rather than
transitional**."* Then it adopts exactly that shape — `task_journal` with a
nullable `event_seq` that P3 drains — and calls it a back-fill obligation. The
rejection reasoning applies verbatim to the accepted design. One of the two is
wrong, and the design's own §13.3 lesson ("a disposition was recorded as landed,
and the edit reached one or two of the five documents that mention it") is the
same failure at the reasoning level rather than the editing level.

**(b) `E3` cannot be satisfied retroactively, and the ADR implies it can.** A P2
transition committed today and a `SereaEvent` written during a P3 migration are
**not in the same transaction**. `E3` reads *"An event and its state change commit
in one transaction — never one without the other."* For historical rows that
transaction does not exist and never will. ADR-0021's table already marks `E3`
**NOT claimed** for P2, which is correct; what is wrong is the P3 column's promise
that `E3` "becomes true at the moment P3 exists", which is true *forward only* and
is not what that sentence says.

**(c) Back-filling normal `SereaEvent` rows is the wrong move, and it is
avoidable.** A reconstructed event is indistinguishable from an atomically
committed one unless provenance is recorded — and provenance would mean a new
field on a frozen wire type. Meanwhile the historical material **already exists**
in `task_journal`, which is why the column list and `payload_json` exist at all.
Reconstructing `serea_events` rows duplicates data that is already durable, and
in doing so manufactures events that never happened in the transaction `E3`
describes.

**Disposition — accepted.** ADR-0021 is rewritten on one rule:

> `E3` is enforceable **forward only**, from the moment a real event participant
> exists. Historical P2 transitions are already durably recorded in
> `task_journal` and are **not** reconstructed into `serea_events`.

Concretely: P3's migration adds `serea_events` and `store_meta`; from then on, a
transition writes both in one transaction and `E3` holds. P3's upgrade path
**reads `task_journal` for pre-P3 history** and does not synthesise events.
`task_journal.event_seq` is therefore dropped rather than back-filled, and
`pending_event_transitions` keeps its meaning as the count of pre-P3 transitions
— which is exactly the operator visibility the ADR wanted, and is honest.

This also removes the `CommitHook` defect below from the critical path: with no
back-fill, the only thing the P3 participant must do is *append*, and the seam
does not have to carry reconstruction state.

**Design docs changed:** yes — ADR-0021; `P2-sqlite-schema.md` §4.9;
`P2-storage-task-engine.md` §9.1.

**Test that pins it tomorrow:** P2G's `pending_event_transitions > 0` assertion
stays; P3's obligation is restated as *"every `task_journal` row with
`event_seq IS NULL` is read as history, never materialised as an event"*, and the
P2 suite asserts no `serea_events` table exists — the grep the ADR already names.

---

### M4 — MAJOR — `CommitHook::append(&mut self, tx)` needs hidden state, and cannot be driven from `Store::transact(&self, …)` without interior mutability

**Affected:** ADR-0021 §Decision; `P2-storage-task-engine.md` §3.2, §11
question 6.

**Evidence and reasoning.** The sketch is

```rust
pub trait CommitHook {
    fn append(&mut self, tx: &mut Tx) -> Result<(), StoreError>;
}
```

The design's own §3.2 table puts the journal call **inside** `Tx` methods
(`insert_task` → "append `TASK_INSERTED`"), and `TaskJournal` lives in
`serea-task-engine`, a layer above `serea-storage`. So `serea-storage` must
invoke a higher-layer hook it cannot name — dependency inversion, and a real
reason for the trait to exist. But the signature defeats it:

1. **No transition identity.** `append` is told *that* a commit is happening, not
   *what* is being committed. Which transition it records must therefore live in
   the hook's own mutable state, set before the call. Two `transact` bodies in one
   `TaskEngine` that both return early can leave that state describing the
   previous transition, and the journal then records a lie. This is the
   "hidden mutable state" and "one hook seeing a different transition than
   another" hazard, and it is a *correctness* hazard, not a style one.
2. **The borrow does not work.** `Store::transact(&self, f)` takes `&self`; a
   `Box<dyn CommitHook>` stored inside the `Store` cannot be called as
   `&mut self` from behind a `&self`. The resolution is `RefCell` or a `Mutex`
   around the hook list — and ADR-0024 states *"The only mutex in `serea-storage`
   guards the single SQLite connection."* One of the two is false as written.
3. **Rollback leaves nothing to clean, but nothing to reset either.** A hook that
   buffered anything for the failed transaction has no signal to discard it.
4. **Ordering is registration order**, which for P3 means `seq` allocation order
   is a property of a `Vec`'s layout rather than of the transition.

**Nothing needs `&mut self`.** `TaskJournal` computes `journal_seq` as
`MAX(journal_seq) + 1` — SQL. P3's event participant allocates `seq` from a
`store_meta` counter — also SQL, inside the same transaction. Neither mutates
Rust state.

**Disposition — accepted.** The seam is kept (the layering need is real) but the
signature changes to an explicit, immutable transition value shared by every
participant:

```rust
/// Immutable description of the transition being committed. Built once, by the
/// `Tx` method performing the state write, and passed to every participant, so
/// no participant can disagree with another about what is being committed.
pub struct DurableTransition<'a> { /* occurred_at_ms, actor, causation, data_class, payload … */ }

pub trait TransactionParticipant {
    fn participate(&mut self, tx: &mut Tx, t: &DurableTransition<'_>)
                   -> Result<(), StoreError>;
}
```

Participants are held as `Arc<dyn TransactionParticipant>` assembled at
construction, so `transact(&self)` iterates them with no interior mutability and
no second mutex — ADR-0024's "no process-local mutex participates" survives
intact. The `&mut self` receiver is retained only so a participant may cache
something non-semantic; the transition itself is a parameter, never state.

**Is a generic hook registry needed at all?** No, and the audit says so. P2 has
exactly one participant. A registry would add ordering, interior mutability and a
lifecycle question in exchange for a capability P2 does not use. The narrower
API — `Tx` calls its registered participants explicitly, in a fixed order, at
the point of the state write — gives P3 the identical seam with less hidden
behaviour, and it is what the design now specifies. ADR-0021's rejected-alternatives
table gains the registry as a considered-and-rejected option, so the reason is on
record rather than implied by absence.

**Design docs changed:** yes — ADR-0021; `P2-storage-task-engine.md` §3.2.

**Test that pins it tomorrow:** the participant signature makes the transition a
parameter, so a compile-level check that two participants observe equal
`DurableTransition` fields is expressible; and P2's clippy/`-D warnings` run plus
a source assertion that `serea-storage` contains exactly one lock — the
`serea-storage` mutex count is now a test, not a claim.

---

### M5 — MAJOR — An expiry reclaim consumes an attempt, so a repeatedly-crashing host exhausts `max_attempts_per_step` without ever executing

**Affected:** ADR-0024 `acquire`; `P2-storage-task-engine.md` §10.4, §4.

**Evidence.** ADR-0024 increments `attempt` on **every** acquisition, "including
an expiry reclaim" (§"Consequences" and the `acquire` statement both say so).
The ceiling is `attempt > max_attempts_per_step`. Executed against
`max_attempts_per_step = 2`, with each acquisition standing in for a worker that
acquired a lease and then died before `begin_attempt`:

```
acquisitions before the ceiling refused .... 2
of which NONE ever called begin_attempt ..... 2
final step row ............................. ('LEASED', 2, 2)
```

So the budget is exhausted by two crashes. No step ever ran, no provider was ever
called, and the task reaches `AttemptCeilingReached` with `attempt = 2` and zero
executions.

This is *defensible* — Task Protocol §3.1 says `attempt` exists to "distinguish
the crash-recovered attempt from a deliberate retry", and counting a crash is the
only way that distinction is visible. It is also **nowhere stated**, and it is the
kind of operational consequence an implementer will otherwise discover in
production. A reader of §10.4 would reasonably conclude that
`max_attempts_per_step = 3` buys three executions.

**Disposition — accepted.** Stated as a named consequence in ADR-0024 and §10.4,
with the arithmetic made explicit: `max_attempts_per_step` bounds **acquisitions**,
so a host that crashes *N* times before `begin_attempt` spends *N* of them; the
effective execution budget is `max_attempts_per_step − crashes`. The recovery
classification that follows from exhaustion is named too: `NeedsReconciliation`
rows with `attempt` at the ceiling are `BLOCKED` with an invariant-violation
reason, not `FAILED`, because nothing was proven to have failed.

The related rollback semantics are also now stated precisely, since the design's
one sentence was true of only one case:

| Case | Result |
| --- | --- |
| First acquisition against `max_attempts_per_step = 0` | rolled back; step left `PLANNED`, `attempt = 0`, **no** `leases` row |
| Third acquisition against a ceiling of 2 | rolled back; step reverts to its **prior committed** state (`LEASED`, `attempt = 2`), and exactly one `leases` row remains |

Verified: the design's sentence *"with the step left `PLANNED` and the lease
released"* is correct only for the first case.

**Design docs changed:** yes — ADR-0024; `P2-storage-task-engine.md` §10.4.

**Test that pins it tomorrow:** P2E's ceiling test asserts the acquisition count,
not the execution count, and a crash-only loop is its own case so the arithmetic
cannot regress silently.

---

### M6 — MAJOR — Three ADRs take three positions on the version treatment, and ADR-0019's is wrong

**Affected:** ADR-0018 §Compatibility; ADR-0019 §"Change control and
compatibility"; ADR-0023 §Compatibility; `docs/decisions/README.md`;
`P2-contract-gap-analysis.md` open question 4.

**Analysis against Protocol Index §4.1**, which defines major as "a breaking
change to any frozen contract in this registry" and minor as "a
backward-compatible addition — a new optional field, …". §7 item 2 requires an
architecture-version bump for any registry change, and §4.1's axes are never
collapsed.

| ADR | Change | Classification | Rule that decides it |
| --- | --- | --- | --- |
| **0018** | five **required** `TaskStep` fields become optional | **major** on `serea.task/1` | §4.1's minor case is *a new optional field*. This is not new — it is a relaxation that "weakens validation for a consumer that relied on it" (the ADR's own words). §5 sets the precedent: a rename "is a breaking change with an alias field for one major version" |
| **0019** | `‖` made precise; SCJ-1 rule 6 refuses every `f64` | **major** on `serea.action/1`; the ADR's own "minor clarification" is wrong | SCJ-1 rule 6 **narrows** the frozen Protocol Index §5 sentence "numbers in shortest round-trip form". A narrowing is not a clarification |
| **0020** | `B3` scoped; no wire change | **editorial / patch** | §4.1: "Patch: editorial only. No contract meaning changes" |
| **0021** | note under `E3`/`E4`; `E3`/`E4` unchanged | **editorial / patch** | the ADR states the invariants are unchanged, which is correct |
| **0022** | note in Data Classification §5; a P2 refusal, no payload change | **editorial / patch** | no wire surface changes |
| **0023** | schema **narrowed** (whitespace-only now refused); Rust `ErrorMessage` **widened** (`\n`, `\t` now permitted) | **minor** on `serea.action/1` | a validation tightening is compatible for a consumer; the widening is a relaxation on a diagnostic field, and the producer that emits `\n` is new |
| **0024** | `lease_generation`, a new optional member | **minor** on `serea.task/1` | literally §4.1's minor case |

**Single coherent plan for P2A:**

| Axis | From | To | Driver |
| --- | --- | --- | --- |
| Architecture | `serea-arch/0.2.0` | **`serea-arch/1.0.0`** | ADR-0018 breaks a frozen contract; §7 item 2 |
| Task surface | `serea.task/1` | **`serea.task/2`** | ADR-0018 (relaxation) + ADR-0024 (new optional member, riding along) |
| Action surface | `serea.action/1` | **`serea.action/2`** | ADR-0019 (SCJ-1 narrows frozen §5) + ADR-0023 (`message` domain widened) |
| Event surface | `serea.event/1` | unchanged | ADR-0023's `actor.id` change is a validation tightening; ADR-0021 changes no event shape |

The major is cheap **now** for a reason that will not stay true: the migration note
required by §7 item 4 has almost nothing to name, because `serea-core` and the
Android client are P12 and no task document has ever crossed a host boundary. That
is precisely the argument for taking the major now rather than calling it minor —
after P12 the note has real consumers and the bump is expensive.

Note this also settles open question 4 in the gap analysis: it is **not** an open
owner choice between minor and major, because §4.1 decides it. What remains an
owner decision is only the *migration-note text*, which is P2A's to draft.

**Disposition — accepted.** ADR-0019's self-classification is corrected to defer
like the others; all three ADRs now cite the one table above instead of three
positions; `docs/decisions/README.md` records the plan; the gap analysis's open
question 4 is rewritten from "minor or major?" to "migration-note text".

**No version change is applied in this run**, per the audit's scope. The
authoritative documents still read `serea-arch/0.2.0` and `serea.task/1`; the plan
above is what P2A implements atomically with ADR-0018's code change.

**Test that pins it tomorrow:** a `protocol_types`/`schema_contracts` assertion
that the envelope and assistant-task schemas carry the new surface strings, so
the bump cannot land on the Rust side alone.

---

### M7 — MAJOR — ADR-0019 rejects shortest-round-trip floats for a reason that is false, and the real obstacle to JCS is different

**Affected:** ADR-0019 SCJ-1 rule 6, its "Stated cost of rule 6", and its
rejected-alternatives row *"Shortest round-trip `f64` form | No portable spelling;
two conforming implementations can differ."*

**Evidence.** RFC 8785 (JCS) §3.2.2.3 requires numbers to be serialized per
**ECMAScript §7.1.12.1 `Number::toString`, including the "Note 2" enhancement**,
and names **Ryu** as a reference implementation and V8 as a live reference.
ECMAScript's `Number::toString` is fully specified, is the shortest
round-tripping form, and is architecture-independent. **A single portable spelling
does exist**, so the ADR's stated reason is factually wrong.

Two further facts, both established by execution rather than by reading:

**Adopting JCS in Rust is not `format!("{}", v)`.** Against RFC 8785 Appendix B,
Rust's `f64` `Display` mismatches **5 of 12** reference values:

| Value | JCS | Rust `Display` |
| --- | --- | --- |
| `-0.0` | `0` | `-0` |
| `1e30` | `1e+30` | `1000000000000000000000000000000` |
| `1e-27` | `1e-27` | `0.000000000000000000000000001` |
| `1.7976931348623157e308` | `1.7976931348623157e+308` | 309 digits |
| `5e-324` | `5e-324` | 324 digits |
| `1424953923781206.25` | `1424953923781206.2` | `1424953923781206.3` |

The last is the interesting one: the round-to-even case, where Rust and
ECMAScript produce *different shortest* representations of the same `f64`. So a
JCS number rule needs `ryu-js` (MSRV 1.71, 1.0.3) — not `ryu`, whose
shortest-round-trip output is not the ECMAScript form — and not `std`.

**Full JCS adoption is nevertheless unavailable here, for a different reason.**
RFC 8785 §3.2.3 sorts object properties by **UTF-16 code units**, and explicitly
warns that "sorting data encoded in UTF-8 or UTF-32 would also work, but the
outcome for JSON data like above would differ and thus be incompatible with this
specification." Frozen Protocol Index §5 says **"keys sorted lexicographically by
UTF-8 code point"**. The two orderings genuinely disagree: in UTF-16 an astral
character is a surrogate pair starting `D800`, which sorts *before* `U+E000–U+FFFF`;
in UTF-8 it is a 4-byte sequence starting `F0`, which sorts *after*. Adopting JCS
wholesale would contradict a frozen contract.

**Decision comparison.**

| Option | Frozen-§5 compliance | Fractions | Cross-arch identical | Cost | Verdict |
| --- | --- | --- | --- | --- | --- |
| **SCJ-1 integer-only (keep)** | yes | refused | yes, with **zero** further work | none | **chosen for P2** |
| SCJ-1 + ES `Number::toString` | yes (numbers only) | yes | yes | `ryu-js` dependency + an unratified numeric range | deferred with a trigger |
| Full JCS | **no** — UTF-16 vs UTF-8 ordering | yes | yes | frozen amendment + loss of interop with every other JCS implementation | rejected |

The honest reason to keep integer-only is **not** "no portable spelling". It is:
no frozen Serea surface requires a fraction (`AssistantTask` has no float field;
Protocol Index §5 already routes 64-bit quantities to decimal strings), while
adopting ES number serialization *now* would add a formatting dependency and
ratify a numeric range before any P0 capability asks for one — and it would buy
nothing P2 can use.

**Disposition — accepted.** ADR-0019's rule 6 keeps its behaviour and gets an
accurate rationale, the JCS relationship and the UTF-16/UTF-8 divergence are
recorded as the *real* obstacle, and the named future trigger is written down so
P5 does not re-derive it: *a capability whose `input_schema` admits a fractional
number* forces the ES `Number::toString` rule and the `ryu-js` dependency at
that point. Cross-architecture determinism is asserted with the mechanism named
(ES `Number::toString` is exact-integer based, so no target-dependent FP is
involved), not with intuition.

**Design docs changed:** yes — ADR-0019 SCJ-1 rule 6, its stated cost, and its
rejected-alternatives table.

**Test that pins it tomorrow:** P2B's vector table gains SCJ-1 rule 6's boundary
cases (already recomputed in this run and recorded in the ADR), and the
cross-architecture suite asserts byte-identical canonical output rather than
merely "a digest was produced".

---

### N1 — MINOR — ADR-0024's rejected alternatives still argue for the removed `token`

**Affected:** ADR-0024, rejected-alternatives rows 292–293.

Two rows survive the token's removal:

- *"Comparing `owner` but not `token` | Two acquisitions by the same owner are
  indistinguishable without the token"* — this is an argument **for** the token,
  and ADR-0024's own `acquire` section refutes it: `generation` increments on
  every acquisition including an expiry reclaim, so two acquisitions by one owner
  are distinguishable.
- *"Rendering the token in `Debug` or in an error | `DC7`: … The token is not a
  credential, but the same discipline applies"* — defends a value that no longer
  exists.

**Disposition — accepted.** Both rows removed. ADR-0024's prose already explains
why the token went; a rejected-alternatives table that argues for the rejected
thing is worse than no table. This is the same class as the design's recorded
"`lease.token` survived in nine places" and "the fix was applied to two of the
five documents that mention it" — the disposition was applied to the decision and
not to the argument.

The full sweep found **no other token remnant** in any of the four documents: the
remaining hits are the historical explanation in §4.6 of the schema, the
Scheduler Protocol's own quoted sentence (which genuinely is about a token and is
correct), token *budgets*, and ADR-0023's use of "token" for opaque values.

**Test that pins it tomorrow:** a docs-lint assertion is unnecessary; the check
is that this row is gone from the table.

---

### N2 — MINOR — ADR-0024's commit statement mixes `:named` and `?` placeholders

**Affected:** ADR-0024, `commit-under-lease`.

The statement uses `:digest`, `:now_ms`, `:generation` and `:owner` **and** four
positional `?`. `rusqlite` requires one binding style per call, so this cannot be
prepared as written. The predicate is unchanged; only the placeholders need
naming.

**Disposition — accepted.** All placeholders named in the ADR, with a note that
the mix was a transcription artefact and not a semantic choice — so the next
reader does not "simplify" it back.

---

### N3 — MINOR — `task_steps.plan_revision` is `NOT NULL` with no `DEFAULT`, unlike every sibling

**Affected:** `P2-sqlite-schema.md` §4.0.

`attempt` has `DEFAULT 0`, `lease_generation` has `DEFAULT 0`, and
`tasks.plan_revision` has `DEFAULT 0`. `task_steps.plan_revision` has no default.
The asymmetry is not a defect — every insert supplies it — but it is the kind of
inconsistency that makes a later writer omit it and get a bare
`NOT NULL constraint failed` instead of a meaningful error. It surfaced during
this audit as exactly that.

**Disposition — accepted.** `DEFAULT 0` added for consistency, with the
`tasks.plan_revision` semantics (`0` is the initial plan) named in the comment.

---

### N4 — MINOR — `rusqlite`'s bundled path raises the workspace MSRV, and two of its defaults are wrong for Serea

**Affected:** `P2-storage-task-engine.md` §7.4; open question 7.

> **The finding's headline was wrong, and the final closure run corrected it.** The
> premise — that `libsqlite3-sys` 0.38.x declares `rust-version = "1.88.0"` and is
> `edition = "2024"`, making an MSRV rise an owner decision — is **false**. That
> crate declares no `rust-version` at all and is `edition = "2021"`, and the chosen
> configuration compiles *and runs* on Rust **1.85.0**. There is no MSRV decision.
> Two default-feature facts in this section were also wrong and are fixed below.
> The normative text is [design §7.4](P2-storage-task-engine.md#74-rusqlite-and-the-alternatives).

Resolved from crates.io and the crate sources at audit time, then **re-verified from
crate source and by execution during the final closure run. No dependency is added
by this run.**

| Item | Value | Consequence |
| --- | --- | --- |
| Candidate | `rusqlite` **0.40.2** (2026-08-08) | — |
| License | MIT | acceptable |
| Declared MSRV | none on `rusqlite` itself | no declared number to inherit |
| **`libsqlite3-sys` MSRV** | **no `rust-version` field; `edition = "2021"`** | **corrected. Neither crate declares an MSRV; both publish the policy *"Latest stable Rust version at the time of release. It might compile with older versions."* `cargo +1.85.0 check` and `cargo +1.85.0 run` both succeed on the chosen configuration, compiling the amalgamation and reporting SQLite 3.53.2. The workspace keeps `1.85`; `Cargo.toml` and `.clippy.toml` are unchanged** |
| Bundled SQLite | **3.53.2** (`SQLITE_SOURCE_ID` `2026-06-03 19:12:13 d6e03d8c…`), from `libsqlite3-sys/sqlite3/sqlite3.h` and confirmed by `SELECT sqlite_version()` on a live connection | far above the 3.37.0 minimum, so the §8 `STRICT`/`GENERATED` fallback is **not** needed for this candidate |
| `rusqlite` default features | `["cache", "ffi-sqlite-wasm-rs"]` | `cache` pulls `hashlink`; `ffi-sqlite-wasm-rs` pulls **`sqlite-wasm-rs`**. Both unwanted — `default-features = false` is required, not optional |
| `libsqlite3-sys` default features | **`["min_sqlite_version_3_34_1"] = ["pkg-config", "vcpkg"]`** — corrected; the `3_45_3` name in the original row was wrong | defaults to **system SQLite**, exactly the failure mode §7.4 says to avoid. Note `rusqlite` declares `libsqlite3-sys` without `default-features = false`, so `pkg-config`/`vcpkg` are still *compiled*; `bundled` overrides the *discovery* path. Verified by `otool -L`: no `libsqlite3` in the binary's link list |
| Native build | C toolchain via `cc`; `bundled` → `modern_sqlite` → `bundled_bindings`, so `build.rs` copies `sqlite3/bindgen_bundled_version.rs` and needs no local `bindgen`/`libclang` | CI needs a C compiler — `ubuntu-latest` has one; `macos-latest` has Xcode CLT |
| `bundled-full` | 81 packages compiled on `aarch64-apple-darwin` against 20 for the chosen set; pulls `chrono`, `jiff`, `time`, `serde_json`, `url` (+`icu_*`/`idna`), `uuid`, `csv`, `series`, `vtab`, `window`, `load_extension`, `unlock_notify`, `column_metadata`, `trace`, `hooks`, … | **rejected.** Nothing in §7.2's table needs any of it |
| Apple Silicon / Intel macOS / Linux | `bundled` compiles from source, so all three get the identical 3.53.2 | satisfies the portability invariant by construction, and is the reason `bundled` beats system SQLite here |
| JSON1 | built into SQLite core by default since 3.38 | `json_valid` and `json_extract` both verified against the bundled build; still verified at open per schema §8 |

**Recommended feature set — minimal:**

```toml
rusqlite = { version = "0.40.2", default-features = false, features = ["bundled"] }
```

`bundled` expands to `libsqlite3-sys?/bundled` + `modern_sqlite`
(`bundled_bindings`), which is exactly what is needed: the pinned SQLite, and
bindings generated for it. `Connection::pragma_update`, `rows_affected`,
`execute_batch` and prepared-statement binding are all available without further
features, so §7.2's entire table is covered.

**Consequences the design must absorb.** First, and contrary to what this section
originally recorded, **there is no MSRV consequence**: the workspace's `1.85` pin
in both `Cargo.toml` and `.clippy.toml` is compatible with this candidate, so no
owner decision exists and no one-line change is pending. Second, the §8 fallback
path ("if the resolved bundled SQLite is below 3.37.0") is **not** needed for
0.40.2 and should be recorded as verified-unnecessary for this candidate rather
than left as a live branch. Third, and newly established by the closure run,
`bundled` compiles with `-DSQLITE_DEFAULT_FOREIGN_KEYS=1` and
`-DSQLITE_ENABLE_LOAD_EXTENSION=1`: the first makes `PRAGMA foreign_keys` default to
`ON` (upstream's own default is `OFF`), so the store asserts the pragma rather than
inheriting it; the second means the *C* extension-loading capability is compiled in
whether or not the `rusqlite` feature is enabled, so no Rust feature is enabled for
it and no claim that it is "not compiled in" may be made.

`bundled` also means a C toolchain in CI, which the design already accepted; §15's
"tests depend on shell commands" is unaffected because `cargo` invokes `cc`
itself.

---

### N5 — MINOR — A counter-derived `TempStore` name is not unique across test binaries

**Affected:** `P2-storage-task-engine.md` §7.3.

The design specifies "a **counter-derived** unique name — never a wall clock and
never an RNG, so `.clippy.toml`'s ban … is satisfied and parallel tests cannot
collide." The ban is satisfied; the uniqueness claim is not, and the failure mode
is a cross-process collision rather than a within-process one.

Executed:

```
two binaries, each counting 0,1,2 with a counter-only name
  -> {serea-0, serea-1, serea-2}          # 6 allocations, 3 distinct names
three iterations in one process with a pid-qualified name
  -> {serea-<pid>-1}                      # 3 allocations, 1 name
```

A process id alone is stable but not unique *within* a process, which matters
because `cargo test` runs tests as threads of one process and the §7.3 crash
harness spawns children that re-invoke `current_exe()` — a child inherits the
parent's pid-salted prefix only if the parent passes it, and the matrix's N-series
child/parent coordination is exactly where this bites.

**Disposition — accepted.** The identity is constructed from the three things that
are jointly unique without a clock or an RNG, and the design states the
construction rather than the intent:

```
<test-binary-identity>-<pid>-<atomic-counter>
```

where the counter is a process-wide `AtomicU64`, the binary identity is the test
name or the harness's own fixed label (so two integration-test binaries cannot
collide), and the parent passes its own directory to a crash child so parent and
child never share a prefix by accident. `TempStore::new()` takes the label;
`TempStore::child_inherited(dir)` takes the parent's, which is also how the crash
harness finds the file it must reopen. No `SystemTime::now`, no `Instant::now`, no
RNG — the `.clippy.toml` ban still holds.

**Test that pins it tomorrow:** two integration-test binaries each create
`TempStore::new("lease")` and assert their paths differ; and a crash child asserts
it can reopen the parent's file by the inherited directory. A counter-only
implementation fails the first; a pid-only one fails the second.

---

## 3. Verified true — negative results worth recording

These were checked and found **correct**. They are recorded so a later reader does
not re-open them, and so the audit's positive findings are not mistaken for a
general verdict on the package.

| Claim | Verdict | How |
| --- | --- | --- |
| ADR-0019 SCJ-1 vectors 1–7, 9, 10 | **reproduce exactly** | independent SCJ-1 implementation from prose; all canonical bytes and all ten `sha256:` values match |
| ADR-0019 IDK-1 vectors 1–5, A, B | **reproduce exactly** | all seven `idk_` values match |
| Design §13.2's preimage byte layout | **exact** | 21 / 1 / 53 / 53 / 49 / 39 / 66 = **282** bytes; every component matches; `sha256` = published vector 1 |
| §13.2's **rejection** of the security-review finding that the vectors do not reproduce | **upheld** | recomputed independently, field by field. The finding was wrong; the *methodological* half of it was right, and ADR-0019 now publishes the byte layout |
| ADR-0019's `arguments_digest` values for the A/B collision pair | **reproduce exactly** | `sha256:7ed00270…` and `sha256:d4735e3a…` |
| The migration builds | **yes** | §4.0 extracted verbatim: **10 tables, 7 triggers, 6 explicit indexes**. (Was 7 indexes; the audit removed the partial index on the deleted `event_seq` column) |
| Every `kind × status` cell | **51 constructible, 5 correctly refused** | 8 × 7 = 56; the 5 refusals are `WAITING` on a non-wait kind, which is ADR-0018's intent |
| All 37 legal task transitions | **constructible** | every pair built from a real seeded row |
| All 84 illegal pairs accepted by SQL | **expected** | the schema deliberately does not encode the transition table; `TaskEngine` owns it |
| Attempt ceiling with rollback | **works as designed** | `attempt = 3 > ceiling 2` ⇒ rollback; step reverts to its prior committed state; exactly one `leases` row remains |
| Lease acquire / reclaim / fence / renew / release | **all work as specified** | ADR-0024's five statements verbatim, in the prescribed order |
| Stale-generation commit | **0 rows, step unchanged** | and 0 rows across **two independent connections** on one file — no process-local mutex participates |
| Attempt incremented exactly once | **confirmed** | `attempt = 1` after acquire *and* after `begin_attempt` |
| Two OS processes, 40 writes | **all landed, `quick_check` ok** | `busy_timeout` serialises correctly |
| SIGKILL before / after `COMMIT` | **0 rows / 1 row** | and SQLite recovers the stale `-wal` on next open |
| `ignore_check_constraints` defeats every `CHECK` | **confirmed** | `data_class_rank` → 3 accepted, generated label became `SECRET` |
| …and every **trigger** and **foreign key** survives it | **confirmed true** | this is the design's claim, and it holds |
| `SECRET`/`CREDENTIAL` unconstructible on all classified tables | **confirmed** | refused on `tasks`, `blobs`, `side_effect_receipts`, `plan_revisions`, `task_journal` — all 8 probes |
| `PRIVATE` blob unprotected unconstructible | **confirmed** | `CHECK ((data_class_rank = 2) = (protection = 'AT_REST'))` |
| Cross-class laundering via `digest` alone | **prevented** | `PRIMARY KEY (digest, data_class_rank)`; same digest at two classes stores two rows |
| `size_bytes = length(content)` | **enforced** | mismatched length refused |
| Duplicate migration version / name | **refused** | `UNIQUE` on both |
| Zero-length file ⇒ fresh; foreign file ⇒ `NotSereaStore` | **both detectable** | 0 tables vs `['unrelated']` with no `schema_migrations` |
| Apple Silicon portability, current source | **clean** | **no** `cfg(target_arch)`, **no** `cfg(target_endian)`, **no** `repr(C)`, **no** `transmute`, **no** pointer casts, **no** raw struct persistence. Every `usize` is an in-memory length. The one binary encoding, `ids.rs:453`'s `u128::from_be_bytes`, is explicitly big-endian and therefore architecture-independent. **No** hardcoded `/usr/local`, `/opt/homebrew` or `/usr/bin` anywhere; the only absolute paths are `#!/usr/bin/env python3` shebangs. CI runs `ubuntu-latest` only |

---

## 4. Answers to the questions this audit was asked to settle

### 4.1 ADR-0019 vector 8 (§7.1)

**The input is the defective element.** Independently recomputed:

| | |
| --- | --- |
| Stated input | `{"k": "q\"b\\s\nt\tu\u0001v/é"}` |
| Stated canonical | `{"k":"q\"b\\s\nt\tu\u0001v\u007f/é"}` |
| Recomputed canonical of the stated input | `{"k":"q\"b\\s\nt\tu\u0001v/é"}` |
| `sha256(canonicalize(stated input))` | `52f38c8cf283fe4c27906193c127759a3dc55a2c09d3193e52aa4795fc859a3c` |
| `sha256(stated canonical bytes)` | `e1e4c6bf233f76ae93bbd29dcd61c9d7704064e2d8ab627ba2310118de3d7a16` |
| Published | `e1e4c6bf233f76ae93bbd29dcd61c9d7704064e2d8ab627ba2310118de3d7a16` |

Canonicalization is a function; it cannot introduce a character. The published
hash matches the stated canonical bytes, and those bytes contain a `U+007F` the
input never had.

Two repairs were available:

| | Result |
| --- | --- |
| **A — add `\u007f` to the input** | canonical becomes the stated bytes; `sha256` becomes the published value. **No constant changes.** |
| B — drop `\u007f` from the canonical bytes | hash becomes `52f38c8c…`; the published constant changes, **and the vector stops testing `U+007F` at all** |

**A is correct**, and the vector's own prose decides it: *"Vector 8 pins the
escape table: `\u0001` **and `\u007f`** are escaped to their lowercase four-digit
forms."* With the stated input, `\u007f` is never exercised, so the vector does not
do the one job it exists for. A fixes the input and keeps both the constant and
the coverage. ADR-0019's vector 8 is corrected accordingly; the hash is unchanged.

### 4.2 Crash-window realism (§7.7) — test N7

**A deterministic abort "mid-COMMIT under WAL" is not injectable through the
planned Rust/SQLite layer.** Classified, with the reachable alternative for each:

| Technique | Reaches | Classification |
| --- | --- | --- |
| Return `Err` before `execute_batch("COMMIT")` | nothing inside SQLite; identical to an ordinary rollback | **must-test directly** (it is a real window: "crash after the engine writes, before commit") |
| Return `Err` *after* `COMMIT` returned | the caller never learns the outcome | **must-test directly** — and it is the most valuable one, because recovery's `ReceiptAlreadyCommitted` row exists for exactly this |
| `TxHook` between the engine writes and `COMMIT` | inside the transaction, pre-commit | **must-test directly** |
| `SIGKILL` while `COMMIT` is in flight | timing-dependent; non-deterministic | **stress test only** — verified to work in practice (0 rows / 1 row, `quick_check` ok) but it is not a pin |
| `SQLITE_TESTCTRL` / fault-injecting VFS / SQLite fault build | inside SQLite | **deferred specialized storage test** — needs a custom build or VFS, unavailable through `rusqlite` as shipped |
| A second writer forcing `SQLITE_BUSY` mid-commit | nothing; SQLite serialises writers and the second waits out `busy_timeout` | **not a technique** |

**N7 as written must be reclassified.** A test that aborts before
`execute_batch("COMMIT")` and calls it "mid-COMMIT" is exactly the fake the design
forbids in §15.8, and the design's own prohibition — *"No test may simulate a
crash by returning an `Err` before commit and calling it a crash"* — already
covers it. Replaced with: three deterministic in-process windows, plus a
child-process `SIGKILL` suite classified as a stress test whose assertions are
"the database is consistent and `quick_check` is ok", never "exactly N rows".

### 4.3 TempStore identity (§7.6)

See **N5**. Construction: `<binary-identity>-<pid>-<atomic-counter>`, with an
inherited-directory variant for crash children. No clock, no RNG.

### 4.4 Integrity verification (§7.5)

See **M2**. `quick_check` and `integrity_check` are page-level and both report
`ok` on an FK-orphaned row; only `foreign_key_check` sees it. Four tiers defined,
with `foreign_key_check` added to open-adjacent and post-migration verification.

### 4.5 CommitHook (§7.9)

See **M4**. The signature cannot work as drawn; the transition must be an explicit
immutable parameter; and a generic registry is not needed — an explicit participant
list gives P3 the same seam with less hidden behaviour.

### 4.6 Canonical JSON numerics (§7.10)

See **M7**. Keep SCJ-1 integer-only for P2, with an accurate rationale; JCS is
unavailable because it sorts keys by UTF-16 code units while frozen Protocol Index
§5 says UTF-8 code point; a JCS number rule needs `ryu-js`, not `std` and not
`ryu`; and the future trigger is named so P5 does not re-derive it.

### 4.7 Category-O identifier rule (§7.3)

See **B1**. Answer: **C** — refuse the prefixed and fixed-shape frozen domains;
exclude `ProviderId`/`ModelId`/`ImplementationId` with a stated reason; exclude
the `goallatch` namespace so the banned set equals `CapabilityId`'s accept set.
Fully expressible in JSON Schema, so parity is real.

### 4.8 Apple Silicon / cross-architecture (§8)

**Target invariant:** Serea core logic and durable state migrate from
`x86_64-apple-darwin` to `aarch64-apple-darwin` without architectural redesign.

**Can the implementation described here be moved without redesigning storage,
task-engine, canonicalization, or recovery? Yes — and the support is named, not
intuitive:**

| Layer | Why it is architecture-independent | Evidence |
| --- | --- | --- |
| Durable time | `INTEGER` epoch milliseconds, integer comparison only; never `TEXT`, never a wire timestamp | design §8 |
| Durable identifiers | text in the wire form (`tsk_` + 26 Crockford chars); no native struct persisted | `ids.rs`, schema §4.3 |
| ULID encoding | `u128::from_be_bytes` + shifts and masks — explicit big-endian integer arithmetic | `ids.rs:451–468` |
| Canonical JSON | integer-only numbers; no float formatting at all in P2 | ADR-0019 SCJ-1 rule 6 |
| Digests | SHA-256 over bytes; defined by the standard, not by the platform | ADR-0019 |
| Idempotency keys | `u64` big-endian length prefixes; explicit endianness | ADR-0019 IDK-1 |
| SQLite types | `INTEGER` / `TEXT` / `BLOB` in a `STRICT` table; architecture-independent storage classes | schema §4.0 |
| Leases and fences | integer generation compared in SQL; no in-process comparison | ADR-0024 |
| Classification ranks | integers with generated labels | schema §3 |
| `rusqlite` | `bundled` compiles SQLite 3.53.2 from source, so every platform gets the identical version | N4 |
| Whole crate | no `cfg(target_arch)`, no `repr(C)`, no `transmute`, no persisted `usize`/`isize`, no native-endian encoding, no hardcoded toolchain paths | verified by exhaustive grep |

**Portability blockers: none found in the design or the current source.** Two
*requirements* are added:

1. **Durable binary fields must be explicit-format.** Fixed-width integers,
   explicit endianness, UTF-8 where specified, architecture-independent SQLite
   types. No host-native memory representation may become a durable format. This
   is a constraint on P2B's `canonical.rs` and on any future blob format, and it
   is already satisfied by every durable field the design names.
2. **SQLite version must be pinned by `bundled`, not discovered.** A system SQLite
   is a portability *and* reproducibility hazard across all three targets.

**Cross-architecture acceptance design (not applied in this run).**

CI gains two runners — `macos-13` (Intel x86_64) and `macos-14`/`macos-15`
(Apple Silicon aarch64) — alongside the existing `ubuntu-latest`. Three jobs:

**(a) Byte-identity job.** One fixture of vectors, run on all three, comparing
**exact bytes**, not "a digest was produced":

- canonical JSON bytes for all ten SCJ-1 vectors
- `Digest` values over those bytes
- `IdempotencyKey` values for all seven IDK-1 vectors
- the full 282-byte IDK-1 preimage, asserted byte for byte
- `ULID` encodings for the fixed timestamp/entropy pairs

**(b) Durable-fixture job.** A `TempStore` database generated **once** on
`ubuntu-latest`, committed as a small binary fixture, then opened on
`macos-13` and on Apple Silicon. Asserted identical:

- `PRAGMA schema_version` / `MAX(version) FROM schema_migrations`
- `sqlite_master` object inventory (10 tables, 7 triggers, 6 indexes) and each
  table's `sql` text
- every `TaskStep` serialization round-trip
- recovery classification for a seeded matrix of recovery cases
- state-machine behaviour: the 121-pair transition table and the 51 constructible
  `kind × status` cells
- blob digests re-verified on read

**The migration procedure must not translate database contents.** SQLite's file
format is architecture-independent, so the fixture is copied, not converted. What
*is* machine-specific is the set of sidecar files, and the handling must be
stated.

> **Superseded by the final closure run — the rule below was wrong.** It said to
> copy `serea.sqlite` **and** its `-wal` **and** its `-shm` together, claimed a
> `-wal` without its `-shm` "is discarded by SQLite on open", and treated a
> `-shm`-less WAL as able to "leave a torn tail". Upstream's own documentation
> contradicts all three, and the corrected rule is now normative in
> [design §7.5](P2-storage-task-engine.md#75-moving-a-serea-database-to-another-machine).

The corrected rule, stated once here so this record is not read as authority:

| File | Status | Carry it? |
| --- | --- | --- |
| `serea.sqlite` | durable migration asset; big-endian, cross-platform | **yes, always** |
| `serea.sqlite-wal` | durable migration asset **when committed frames are not yet checkpointed**; big-endian, cross-platform | **yes, if it exists** |
| `serea.sqlite-shm` | **transient, rebuildable artifact** — the wal-index, which upstream states "stores multi-byte values in the **native byte order of the host computer**" and "can use an architecture-specific format" | **no, never** |

The three findings behind the correction, all verified during the closure run
against SQLite's own documentation and by execution:

1. **A `-shm`-less `-wal` is safe, and is in fact the correct abnormal-case artifact
   set.** Deleting the `-shm` from a WAL database that held committed frames and
   reopening it rebuilds the wal-index from the `-wal`, recovers every committed
   row, and returns `quick_check = ok`. The earlier claim that the wal-index is
   "discarded" had it backwards.
2. **After a clean close there is nothing else to copy.** Both `-wal` and `-shm` are
   deleted by the last connection, so the single-file copy is not merely the common
   case, it is the only case after a clean stop.
3. **`-shm` must not be carried across `x86_64 → arm64`.** Not because a copy is
   known to corrupt anything — measured, a mismatched `-shm` caused no observable
   damage — but because upstream explicitly permits the wal-index to be
   architecture-specific, and "permitted" is not "guaranteed".

So: **stop Serea → confirm no other connection → checkpoint (`wal_checkpoint(TRUNCATE)`,
checking the `busy` column) → close all connections → copy `serea.sqlite` alone →
open on the destination → run the migration, integrity and recovery checks.** The
abnormal case, where committed frames remain in the `-wal` because Serea did not
stop cleanly, copies `serea.sqlite` **plus `serea.sqlite-wal`** and still not the
`-shm`.

Two macOS-specific facts are recorded so they are not discovered late: filesystem
paths are UTF-8 on both architectures, so no path-encoding assumption differs; and
`bundled` removes the ABI question entirely, because no system `libsqlite3` is
linked at all.

**Not verified in this run:** that the fixture actually opens on Apple Silicon.
This host is `x86_64`. The claim is that the design contains nothing
architecture-dependent, which the grep and the field-by-field table above
support; the empirical confirmation is the CI job in (b), and that job is the
deliverable, not an assumption.

---

## 5. Owner decision triage

Every open item, reduced. **A** = required before P2A · **B** = required before
its subphase · **C** = safe to defer beyond P2 · **D** = not an owner decision,
evidence determines it.

| # | Item | Class | Basis |
| --- | --- | --- | --- |
| 1 | Architecture-version and wire-surface bumps for P2A | **A — resolved by evidence, needs ratification only** | §4.1 decides it; the plan is in **M6**. The owner ratifies; the owner does not choose |
| 2 | Category-O rule (A/B/C) | **A — resolved by evidence** | **B1**. B is untenable (8/14 false positives); C is 16/16 and 0/14 |
| 3 | Canonical-number contract | **A — resolved by evidence** | **M7**. Integer-only is correct for P2; the real obstacle to JCS is UTF-16 vs UTF-8 key ordering, not number spelling |
| 4 | `rusqlite` feature set | **D**, and the recorded **A** consequence is **withdrawn** | **N4**. The feature string is evidence-determined. The consequence originally recorded here — that `libsqlite3-sys` requires Rust 1.88 while the workspace pins 1.85 — is **false**; the chain builds and runs on 1.85.0, so there is no owner decision |
| 5 | SHA-256 candidate | **D** | `sha2` **0.11.0**, MSRV **1.85** — exactly the workspace MSRV, so no conflict. MIT/Apache-2.0, pure Rust, `no_std`-capable, no network or clock, standard SHA-256 |
| 6 | Real `AtRestProtection` backend | **C** | ADR-0022 refuses `PRIVATE` with no backend and says so; the trait plus a test double prove the wiring. Consistent and explicitly documented |
| 7 | `SECRET` sealed store | **C** | No owning crate is named anywhere; P2 refuses `SECRET` at the storage layer and that is the correct posture |
| 8 | `NOTIFY` capability shape | **C** | ADR-0018 §4 already makes it host-internal and names the ADR that would change it |
| 9 | Resource-bound numeric values | **C**, separate ADR | §12 keeps P0's gap open and correctly declines to invent numbers |
| 10 | `foreign_key_check` tier policy | **A — resolved by evidence** | **M2**. The four tiers are defined from what each pragma verifiably checks |
| 11 | `foreign_keys = OFF` as a named boundary | **A — resolved by evidence** | **M1**. Stated the way `ignore_check_constraints` already is |
| 12 | Connection profiles split | **A — resolved by evidence** | **B4**. `:memory:` cannot be WAL; two profiles |
| 13 | `TempStore` identity construction | **D** | **N5**. Three-part construction; no clock, no RNG |
| 14 | Crash N7 classification | **D** | §4.2. Four techniques classified; the fake is named and forbidden |
| 15 | `E3` forward-only semantics | **A — resolved by evidence** | **M3**. `E3` cannot be retroactive; no event reconstruction |
| 16 | `CommitHook` shape | **A — resolved by evidence** | **M4**. Neither participant needs `&mut self`; no registry needed |
| 17 | Attempt/reclaim budget interaction | **A — resolved by evidence** | **M5**. Arithmetic stated; recovery outcome named |
| 18 | Presence-matrix enforcement gap | **A — resolved by evidence** | **B3**. Three additive constraints close all 8 cells |
| 19 | The §4.6 phantom trigger | **A — resolved by evidence** | **B2**. Removing it is the whole fix |
| 20 | Whether P2 raises the workspace MSRV | **RESOLVED — no rise. Not a decision.** Recorded here originally as the single remaining item that was a choice rather than a finding. It was not: the premise was a misreading of `libsqlite3-sys`'s manifest. `cargo +1.85.0` builds and runs the chosen configuration, so the workspace stays at `1.85` and `Cargo.toml`/`.clippy.toml` are unchanged |

---

## 6. Tomorrow's decision ledger

See [P2-tomorrow-decision-ledger.md](P2-tomorrow-decision-ledger.md).

---

## 7. What this audit did not do

Stated so the boundary is not read as an omission.

- **No production Rust, no runtime crate, no migration file used by production, no
  `Cargo.toml`/`Cargo.lock` change, no CI change.** Verified in §8 of the final
  report and by `git diff --cached --check` before commit.
- **No frozen protocol document changed.** The amendments in the seven ADRs remain
  drafted and unapplied, as the audit's scope requires.
- **No ADR moved to Accepted.** All seven stay **Proposed**; this run does not
  ratify.
- **P0's resource-bound gap is not closed.** §12 still says so, and this audit
  does not pretend otherwise.
- **No claim that `E3`, `E4`, `C4`'s second half, `T6`'s comparison, any §2 bound
  other than `max_attempts_per_step`, or any `PRIVATE` at-rest support is
  delivered.** The §9.3 non-claim table stands.
- **Apple Silicon was not empirically tested** — this host is `x86_64`. §4.8 says
  what is verified (an exhaustive source audit) and what is not (a fixture opened
  on the other architecture), and the second is a CI deliverable.
- **Not every item in §10's SQLite sweep needed a new decision.** Disk-full,
  permission failure, corrupt WAL, corrupt database and read-only filesystem are
  OS-level failures with no correct in-process answer beyond mapping them to
  `StoreError::Sqlite` and refusing to open. The design's §3.4 already provides
  that mapping. Recorded here rather than as eleven new findings.
