# P2 SQLite Schema

- **Branch:** `p2/design-preparation`
- **Base commit:** `c3737039e3e38dbba554dc0b9075025f87948358`
- **Status:** historical design preparation, reconciled after the frozen P2C
  gate. Production `0001_initial.sql` is now the sole DDL authority (§4.0).
  This disjoint schema/docs slice adds no Rust runtime or dependency.
- **Authority:** [ADR-0005](../decisions/ADR-0005-sqlite-wal-and-migrations.md) for
  SQLite + WAL + ordered migrations; [P2 contract gap
  analysis](P2-contract-gap-analysis.md) for the contract gaps this schema has to
  satisfy; [ADR-0018](../decisions/ADR-0018-taskstep-lifecycle-and-field-presence.md),
  [ADR-0021](../decisions/ADR-0021-p2-p3-event-atomicity-seam.md),
  [ADR-0022](../decisions/ADR-0022-durable-private-data-at-rest.md) and
  [ADR-0024](../decisions/ADR-0024-lease-fencing-and-commit-under-lease.md) for
  the invariants the constraints enforce.

## Current P2D frozen-gate annotation (2026-10-04)

[P2D's frozen gate](P2D-review-and-closure.md) supersedes the old blob/text
sketches. P2D is only JSON blob put/get, `BlobRef` and PRIVATE-only protection;
reference attachment/roles, deletion and blob+reference atomicity are P2F.
Ordinary-row PRIVATE protection, including JSON extensions, remains deferred and
future writers must fail closed even with a blob backend. ADR-0022 remains
**Proposed**. Historical SQL probes below are preserved; this reconciliation
claims no P2D runtime-test PASS.

## 1. Tables in P2, and the ones deliberately absent

| Table | In P2? | Why |
| --- | --- | --- |
| `schema_migrations` | **Yes** | The single authority for the schema version |
| `tasks` | **Yes** | Task Protocol §2 |
| `task_steps` | **Yes** | Task Protocol §3 |
| `side_effect_receipts` | **Yes** | Capability Protocol §5.1, normalised out of the step |
| `leases` | **Yes** | ADR-0024 |
| `plan_revisions` | **Yes** | Task Protocol §4.3 rule 5 requires revisions recorded; `PlanRevision` is a Crate Map §3 public type |
| `blobs` | **Yes** | Task Protocol §3.1's content-addressed store |
| `task_blob_refs`, `step_blob_refs` | **Yes** | Two tables rather than one polymorphic one, so both can carry a foreign key |
| `task_journal` | **Yes** | ADR-0021's commit hook; the durable audit record that makes `T5` provable |
| `store_meta` | **No** | P2 needs no host metadata. `next_seq` belongs to P3, which adds it in migration `0002` |
| `serea_events` | **No** | `serea-event-bus` is P3. ADR-0021's last rejected alternative is P2 growing this table |
| `approvals`, `grants` | **No** | `serea-capability`, P6 |
| `schedules`, occurrence records | **No** | `serea-scheduler`, P3 |
| `memory_items`, tombstones | **No** | `serea-memory`, P7 |
| `model_usage` | **No** | `serea-model-router`, P4. The *counters* are named in Bounds §2.1 as living on the task, and P2 keeps `max_model_calls` / `max_tool_calls` on the task because they are part of the frozen `attempt_budget` shape — but the `*_used` columns are P4's, and P2 has nothing to increment them with |
| `capability_registry`, `disabled` overlay | **No** | `serea-capability`, P5 |
| `policy_rules` | **No** | `serea-policy`, P6 |
| `device_sessions`, `pairings` | **No** | `serea-core`, P12 |
| `proposals` | **No** | `serea-scheduler`, P11 |

`data/` does not appear either: `serea-storage` maps
`TaskOrigin.device_id` to a `dev_` identifier and does not own a device roster.

**The one judgement call in this table** is `plan_revisions`. It is not in the
prompt's list, and it is not in P0's table list either. It is included because
Task Protocol §4.3 rule 5 requires each revision to be recorded as evidence, and
because ADR-0018's decision to *delete* superseded `PLANNED` steps is only
auditable if the prior plan content survives somewhere. That somewhere is a
content-addressed blob referenced by the revision row.

## 2. Normalised versus JSON

**The rule: a field is JSON only if nothing ever needs to compare, order, or
predicate on it.**

Five fields must be SQL-addressable, or the design does not work:

| Field | Predicate that needs it |
| --- | --- |
| `task_steps.status` | The recovery classification predicate |
| `task_steps.lease_generation` | The ADR-0024 fence |
| `task_steps.attempt` | The `max_attempts_per_step` ceiling, read from durable state at the check |
| `side_effect_receipts.step_id`, `UNIQUE` | `ReceiptAlreadyCommitted` during recovery — a single indexed existence check |
| `task_steps.task_id` | Parent binding, so one task cannot mutate another's step |

Anything storing an `AssistantTask` as one JSON document is explicitly refused: it
would turn all five into application-side read-modify-write and lose the atomicity
`TB-7` buys.

Stored as JSON, and why each is safe:

| Column | Why JSON is safe |
| --- | --- |
| `tasks.extensions` | `Protocol Index` §4.2 rule 3: unknown fields are retained, not interpreted. Nothing needs to query it |
| `ActionError.details` | The frozen shape is a closed object whose `details` is an open JSON object; the schema already caps it with `maxProperties: 64` |
| `ActionRequest.arguments` and `ActionResult.output` | **Not stored as JSON columns at all.** They are content-addressed blobs, which is what Task Protocol §3.1 requires and what makes dedup, digest verification, and right-to-delete possible |

Everything else is a column. These normalization/JSON choices do **not** imply
at-rest protection. Opaque extensions and error/journal JSON can carry PRIVATE
content just as prose can; all belong to the deferred complete ordinary-row
surface (§4.3), not the P2D blob backend.

## 3. Class ranks are integers, labels are generated

This ordinary-store illustration admits only ranks 0–2. The protocol still has
all five ranks: `PUBLIC` (0), `PERSONAL` (1), `PRIVATE` (2), `SECRET` (3) and
`CREDENTIAL` (4); the latter two are refused by the ordinary-store cap, not
removed from the protocol.

```sql
data_class_rank    INTEGER NOT NULL CHECK (data_class_rank BETWEEN 0 AND 2),
data_class         TEXT GENERATED ALWAYS AS (
                       CASE data_class_rank
                         WHEN 0 THEN 'PUBLIC'
                         WHEN 1 THEN 'PERSONAL'
                         ELSE 'PRIVATE'
                       END) STORED,
policy_class_rank  INTEGER NOT NULL CHECK (policy_class_rank BETWEEN 0 AND 7),
policy_class       TEXT GENERATED ALWAYS AS (…eight RiskClass values…) STORED,
```

**Why.** `PUBLIC < PERSONAL < PRIVATE < SECRET < CREDENTIAL` and the eight-class
`RiskClass` ordering are both *semantic* orderings that the labels do not
reproduce — alphabetically `CREDENTIAL < PERSONAL < PRIVATE < PUBLIC < SECRET`.
Storing the label and comparing it would compare alphabetically and get the
ordering wrong; storing the rank and the label separately lets them disagree. A
generated column removes the disagreement by construction: one authority, and the
`CHECK` validates the same value the reader sees.

This is a security property, not a convenience. A corrupt or hand-edited row that
disagreed with its own class would be a corrupt row that *widens authority* — a
task's `policy_class` ceiling is `T6`, and a ceiling compared alphabetically
against `RiskClass::rank()` is a ceiling that `DESTRUCTIVE` passes when it should
not.

A test pins the Rust `DataClass::rank()` and `RiskClass::rank()` against the two
`CASE` expressions, in both directions, so the generated columns cannot drift from
the enums.

## 4. The DDL

Minimum SQLite version **3.37.0**, for `STRICT` tables (3.37) and
`GENERATED … STORED` (3.31). Both are load-bearing for §3 and §5, so the fallback
if the resolved SQLite is older is recorded in
[P2 design §7](P2-storage-task-engine.md#7-migrations-and-connection-policy)
rather than left implicit here.

**Historical DDL execution:** the then-current Markdown DDL was built and
exercised against SQLite 3.43.2 during design preparation across four rounds,
and re-extracted after the P2 autonomous audit's edits. Those runs remain
historical evidence, not validation of the new production migration. The current
`p2a-doc-probes.py` harness reads the production file referenced by §4.0; fresh
run output is recorded separately below, without enlarging the old counts.

Four rounds of defects were found this way, and the pattern is the point:

| Round | What was wrong | Why a reading missed it |
| --- | --- | --- |
| 1 | `PLANNED` unconstructible — two biconditionals contradicted each other | Individually well-reasoned; collectively impossible |
| 1 | `CANCELLED` unconstructible — two `CHECK`s on `cancelled_at_ms` disagreed | The **negative** case was tested; the **positive** never was |
| 1 | The whole migration did not build — SQLite prohibits subqueries in `CHECK` | Asserted, not executed |
| 2 | No lease could be acquired, in either order | The upsert was tested against a hand-prepared step, not against the sequence the document prescribes |
| 2 | `SECRET`/`CREDENTIAL` reachable through `task_journal` | Four tables were enumerated and the fifth omitted |
| 3 | The `WAITING` biconditional made the three wait kinds unplannable | The negative (`WAITING` on a `CAPABILITY` step) was tested; the positive (`PLANNED` on a `WAIT_APPROVAL` step) never was |
| 4 | A partially populated capability tuple (`NOTIFY` with only `capability_id`) was accepted | A single biconditional over four columns is not the same as four biconditionals. Found by running the full 8 kinds × 7 statuses grid rather than the two cells either side of the earlier bug |
| 3 | `TASK_DELETED` was "removed" in prose but still in the DDL | The disposition was applied to one of the two places it named |
| 3 | The `cancel` statement in the design doc aborted on a `BLOCKED` task | The schema was fixed; the statement in the *other* document was not |
| 3 | The lease `token` "removed everywhere" survived in nine places | The fix was applied to two of the five documents that mention it |
| **5 (audit)** | **§4.6 still published the removed `leases_generation_matches_step` trigger** | Four documents said it was gone. Following §4.6 makes the *first* lease acquisition abort — the round-2 blocker, reintroduced |
| **5 (audit)** | **8 of ADR-0018 §3's 32 `N`/`0` presence cells were accepted** | §4.4 claimed "a constraint for every row of the presence matrix". Same shape as round 4: the neighbouring cells were probed, these were not |

The consistent blind spot was **negative-only testing**: every "is this refused?"
case was exercised and almost no "is this accepted?" case was. Round 3 exists
because of it — it constructs every legal transition, executes every prescribed
statement sequence in the order §4 prescribes, and confirms the positive side of
every constraint. See §7.

### 4.0 The whole migration

The sole executable schema authority is
[`crates/serea-storage/migrations/0001_initial.sql`](../../crates/serea-storage/migrations/0001_initial.sql).
P2C embeds that file verbatim; the probe harness reads the same production file,
not a Markdown SQL copy. All snippets below are **non-authoritative explanatory
fragments**, not standalone migrations. Consult the production file for the full
constraints, triggers and indexes; never assemble a migration from these snippets.

P2D keeps this production file, catalog and checksum unchanged; no 0002:
`sha256:d9068dccbc26ececb71be79c475080633166ba0163c62b2d98b9733512baefea`.
The existing schema supports the narrow blob seam; PRIVATE row protection is a
separate unresolved design, not a reason to relabel plaintext as protected.

Every one of the **14 durable instants** has an inclusive `EpochMillis` CHECK:
`BETWEEN -62167219200000 AND 253402300799999`. Nullable instants remain nullable;
existing presence and ordering checks still apply. Counters, generation, sizes
and durations do not get instant bounds. `schema_migrations.checksum` also checks
its exact `sha256:` prefix, length 71 and lowercase hexadecimal suffix, rejecting
embedded NUL (SQLite text length/GLOB can otherwise stop there). This is a narrow
structural defense, not tamper-evidence.

### 4.1 `schema_migrations`

The **single** authority for the schema version. P2 does not mirror it into
`PRAGMA user_version`, because two sources for one fact invite disagreement.
Open validates the **full ordered catalog prefix** (contiguous versions starting
at 1, exact embedded name/checksum at each version), not only `MAX(version)`.
Checksum means SHA-256 of the exact embedded UTF-8 migration bytes, including
comments, whitespace and final newline; this is not SCJ-1 canonical JSON.

### 4.2 `blobs`

| Constraint | Job |
| --- | --- |
| `data_class_rank BETWEEN 0 AND 2` | A `SECRET` or `CREDENTIAL` row is **unconstructible**, so `DC5` holds at the storage layer and not merely in Rust |
| `(data_class_rank = 2) = (protection = 'AT_REST')` | Enforces the `PRIVATE`/`AT_REST` **marker pairing only**. A direct `INSERT` can label unchanged plaintext `AT_REST`; this CHECK does not prove encryption or backend use. Protection enforcement belongs to P2D, not P2C |
| `size_bytes = length(content)` | Stored-content byte length: PUBLIC/PERSONAL canonical plaintext, PRIVATE backend bytes including envelope/expansion. Keep the name/schema; this is a **consistency** check, not logical plaintext length or a resource bound (ADR-0020) |
| `PRIMARY KEY (digest, data_class_rank)` | Deduplication *within* a class, and no cross-class laundering — see §5.3 |

**The digest predicate is three clauses, not one.** `substr(digest, 8) NOT GLOB
'*[^0-9a-f]*'` rather than `digest NOT GLOB '*[^0-9a-f]*'`: the prefix `sha256:`
contains `s` and `h`, which are not hexadecimal, so a single negated class over the
whole string rejects every valid digest. The prefix is checked separately with
`substr(digest, 1, 7) = 'sha256:'`. The same three-clause form appears on
`input_digest`, `result_digest`, `payload_digest` and the journal's
`idempotency_key`, where the prefix is `idk_` and the body starts at position 5.

`digest` is stored as text rather than as a 32-byte `BLOB` because the wire form is
`sha256:` + 64 hex and storing the wire form means the database and the protocol
cannot disagree about representation. The cost is 71 bytes per blob instead of 32,
paid on a table whose payload is far larger.

### 4.3 `tasks`

**Every class column in `tasks` is capped at rank 2.** `SECRET` and `CREDENTIAL`
are not merely refused by the Rust dispatch — they are unconstructible rows. This
is the `DC5` enforcement ADR-0022 claims, and it had to be widened from `blobs`
alone to every table that stores classified content, or the claim would have been
true of the blob store and false of everything else.

Five decisions worth stating.

**`policy_class` immutability is a trigger, not a Rust check.**

```sql
CREATE TRIGGER tasks_policy_class_immutable
BEFORE UPDATE OF policy_class_rank ON tasks
WHEN NEW.policy_class_rank IS NOT OLD.policy_class_rank
BEGIN SELECT RAISE(ABORT, 'policy_class is immutable for a task'); END;
```

`T6` says "no step, approval or plan may raise it". A Rust check can be bypassed by
any future writer; a trigger cannot. The `WHEN` clause is load-bearing: without it
`BEFORE UPDATE OF` fires on the *set list*, so a generic "touch `updated_at_ms`"
statement that also names the column would abort, and `UPDATE tasks SET
policy_class_rank = policy_class_rank` would abort too. Both were verified: the
no-op update succeeds with the `WHEN` and fails without it.

**`data_class` may be raised, never lowered.** A second trigger with
`WHEN NEW.data_class_rank < OLD.data_class_rank`. Raising is the safe direction
under `DC3`'s derivation rule; lowering is the laundering direction.

**Three extension columns, not one.** `origin_extensions` and
`budget_extensions` exist alongside `extensions` because
[Protocol Index §4.2 rule 3](../protocols/00-protocol-index.md#42-compatibility-rules)
requires an unknown member of a forward-compatible surface to be **retained and
round-tripped unchanged**, and `assistant-task.schema.json` leaves both `origin`
and `attempt_budget` open. A single column would silently drop them on a
read-modify-write.

**The derived-state `CHECK`s make inconsistent terminal rows unconstructible.** A
`CANCELLED` task without `cancelled_at_ms` cannot exist; a `FAILED` task without a
`failure_reason` cannot exist; `updated_at_ms` can never precede `created_at_ms`;
and `failure_reason` is confined to `FAILED` so a row cannot carry a failure reason
while still executing. These are consistency invariants, not bounds.

**The code-grammar `CHECK`s.** `origin_kind`, `blocked_reason`, `failure_reason`
and `cancelled_by` are `^[A-Z][A-Z0-9_]*$` in Rust. `GLOB` cannot express a
character-class negation plus a first-character constraint in one pattern, so it is
two clauses. The value of having them is that a corrupt row in one of these fields
is a corrupt row that changes a control-flow decision — Event Protocol §4 requires
a code to be a stable machine-readable value.

**What is *not* protected here, stated plainly.** The earlier four-field text
chokepoint was incomplete and is superseded by P2D D5: there is **no
`Tx::put_classified_text` API** or reversible ordinary-row representation in P2D.
`tasks.title`, `tasks.result_summary`, `task_steps.error_message` and
`side_effect_receipts.effect_summary` are only examples. The complete surface
also includes error details, journal payloads, provider references and every
PRIVATE-bearing task/step/receipt/journal JSON extension, including task origin
and budget extensions. `json_valid`, class caps and the blob protection marker
cannot protect those bytes.

Until a complete row-surface design exists, every future ordinary-row
PRIVATE-bearing writer must fail closed **before SQLite, even with a blob
backend configured**. SECRET/CREDENTIAL remain refused. P2D supplies no parent,
receipt or journal writer and cannot claim enforcement of a text chokepoint that
does not exist. A SQL-accepted PRIVATE row in the historical probes below is
schema evidence, not encryption or production PRIVATE task support. ADR-0022
remains **Proposed**.

### 4.4 `task_steps`

The presence and step-kind matrices of ADR-0018 §3 and §4, as status
implications plus per-field biconditionals. The distinction is not stylistic:

| Shape | Why it is used |
| --- | --- |
| `status <> 'X' OR (…all-null…)` | Says what `X` requires. Compose several of these and you get the whole matrix without an accidental biconditional |
| `(status IN ('LEASED','EXECUTING')) = (lease_owner IS NOT NULL)` | Genuinely two-way: a lease exists exactly while leased or executing |
| `((kind IN ('CAPABILITY','DELEGATE','VERIFY')) = (<one field> IS NOT NULL))`, **one clause per field** | Genuinely two-way. Written as a *single* biconditional over all four fields it admits a partially populated tuple: `NOTIFY` with `capability_id` set and the other three null evaluates `0 = 0` and is accepted. Four clauses are strictly stronger than the one it replaces |
| `status <> 'FAILED' OR (five mandatory error fields non-null)` plus `status = 'FAILED' OR (all six error fields null)` | FAILED requires five mandatory members and allows optional details; every member is absent otherwise. A single tuple biconditional admits partial errors outside FAILED |

The one biconditional that had to be **removed** is the `WAITING` one. Written as
`(status = 'WAITING') = (kind IN ('WAIT_APPROVAL','WAIT_USER','WAIT_SCHEDULE'))`
it means a `WAIT_APPROVAL` step may *only ever* be `WAITING` — so it could not be
`PLANNED`, could not be `SUCCEEDED`, and could not be `FAILED`. That made a third of
the step lifecycle unreachable for three of the eight kinds, and made
`persist_plan` abort on any plan containing a wait step, which is Task Protocol
§4.3's central requirement. The correct form is one-directional: a step may be
`WAITING` only if it is a wait step; a wait step may be in any other status.

### Eight matrix cells were not enforced, and now are

[ADR-0018](../decisions/ADR-0018-taskstep-lifecycle-and-field-presence.md)
claimed that `serea-storage` gets "a constraint for every row of the presence
matrix, so an inconsistent step is unconstructible by any writer". The P2
autonomous audit probed **every** cell the matrix marks `N` or `0`, using the kind
that makes each status reachable, and found **eight** that SQL accepts:

| Matrix cell | Before | Now |
| --- | --- | --- |
| `completed_at` on `EXECUTING` | accepted | **refused** |
| `result_digest` on `EXECUTING` | accepted | **refused** |
| `completed_at` on `WAITING` | accepted | **refused** |
| `result_digest` on `WAITING` | accepted | **refused** |
| `lease_expires_at` on `WAITING` | accepted | **refused** |
| `lease_expires_at` on `SUCCEEDED` | accepted | **refused** |
| `lease_expires_at` on `FAILED` | accepted | **refused** |
| `lease_expires_at` on `RECONCILED_ABSENT` | accepted | **refused** |

The other 24 `N`/`0` cells were already refused, and three control cells (a legal
`EXECUTING`, a legal `WAITING`, a legal `SUCCEEDED`) were accepted, so the probe
was not simply refusing everything.

The `lease_expires_at` half is the more serious. The schema already treats
`lease_owner` as a genuine biconditional, so the *pair* was half-constrained: a
terminal step could carry a lease expiry with no owner. ADR-0024's commit statement
clears both columns together, so no designed path produces it — but a future
writer that clears only `lease_owner` would pass the schema and leave a dangling
expiry. That is the same hole the biconditional closed for one column, left open
for the other. The fix is the matching biconditional, and it is two clauses rather
than one:

```sql
CHECK ((status IN ('LEASED','EXECUTING')) = (lease_expires_at_ms IS NOT NULL))
```

**Historical probe result: all 32 selected matrix `N`/`0` cases were refused,
all 51 legitimately constructible `kind × status` cells and all 37 legal task
transitions constructed.** This did not exhaust error shapes: current DDL
additionally requires all six error columns NULL outside FAILED, including details,
and five mandatory members on FAILED with optional details. Single-column and
partial-tuple probes supplement, not retroactively enlarge, the historical run. This is the fifth instance of the package's own
named blind spot — asserting a constraint rather than constructing the cell that
would expose it — and the corrective is the same one the design already prescribes:
probe every cell, in both directions.

**An earlier draft used biconditionals where only implications were correct**, and
`PLANNED` became unconstructible: a biconditional `(status = 'LEASED') =
(started_at_ms IS NULL)` contradicts a `PLANNED` row, whose `started_at_ms` is
also null. The corrected form is
`status NOT IN ('EXECUTING','WAITING','SUCCEEDED','FAILED','RECONCILED_ABSENT') OR
started_at_ms IS NOT NULL`. ADR-0018 §3's matrix is corrected to match: see the
`started_at` row, which is `N` for `LEASED` and `R` from `EXECUTING` onward.

`UNIQUE (task_id, idempotency_key)` is the index that turns a *wrong* key
derivation into a visible failure. SQLite treats `NULL`s as distinct, so the many
non-capability steps that carry no key are unaffected.

Two constraints have no counterpart in ADR-0018 and are stated here:

- **`idempotency_key` is immutable.** A trigger refuses any `UPDATE` of it. It is
  derived once at plan time from the request, and Capability Protocol §8.2's whole
  property depends on every attempt of a step reusing it.
- **`lease_generation >= 1` for every non-`PLANNED` step.** This closes a real
  hole: an earlier draft's biconditional short-circuited on
  `lease_owner IS NOT NULL`, so a `SUCCEEDED` step with `lease_generation = 0` was
  accepted. Verified: `0` is now refused.

  **What the constraints do not do, stated precisely.** The column CHECK also
  imposes the u32 ceiling `4294967295`; PLANNED uses SQL 0 / wire None, and
  all later generations are positive u32, retained after release. Eligible
  acquisition at the maximum fails and rolls back, never wraps or clamps.
  Boundedness does not prove provenance: a corrupt row can carry generation 99
  or a terminal generation with no matching `leases` row. There is no reverse
  foreign key. Recovery must detect this; every outcome predicate must also
  EXISTS an authoritative lease matching step/owner/generation and unreleased
  state. Missing authority makes that outcome affect zero rows. Neither the
  step copy nor the u32 bound alone proves ownership.

**What is still not enforceable in `CHECK`, and is therefore detected rather than
prevented:** `side_effect_receipt` on a `RECONCILED_ABSENT` step. Receipt presence
is a cross-table property — `side_effect_receipts.step_id`, `UNIQUE` — and no
`CHECK` can span tables. A trigger on the *receipt* side covers the direction that
matters (a receipt may only be recorded for a `SUCCEEDED` step, verified below);
the reverse direction is recovery's invariant scan.

### 4.5 `side_effect_receipts`

Three decisions:

**`task_steps` has no `receipt_id` column.** The step's receipt is reached by
`side_effect_receipts.step_id`, which is `UNIQUE`. This removes what would
otherwise be a **circular foreign key** and removes a duplicated fact. The wire
`TaskStep.side_effect_receipt` is assembled on read from this join. ADR-0024's
commit statement has been corrected to match.

**Two triggers replace a `CHECK` that SQLite does not permit.** An earlier draft
used a subquery inside `CHECK` to require the receipt's `idempotency_key` to equal
its step's. **`CREATE TABLE` fails outright** with *"subqueries prohibited in CHECK
constraints"*, so the entire migration was unbuildable. The triggers below are the
working form:

```sql
CREATE TRIGGER side_effect_receipts_key_matches_step
BEFORE INSERT ON side_effect_receipts
WHEN (SELECT idempotency_key FROM task_steps WHERE step_id = NEW.step_id)
     IS NOT NEW.idempotency_key
BEGIN SELECT RAISE(ABORT, 'receipt idempotency_key must equal its step key'); END;

CREATE TRIGGER side_effect_receipts_step_must_succeed
BEFORE INSERT ON side_effect_receipts
WHEN (SELECT status FROM task_steps WHERE step_id = NEW.step_id) <> 'SUCCEEDED'
BEGIN SELECT RAISE(ABORT, 'a receipt may only be recorded for a SUCCEEDED step'); END;
```

The first is sound because the second makes a step's key stable: a step's
`idempotency_key` is immutable (§4.4) and `status` only ever moves forward, so a
receipt's key cannot drift from its step's after the fact. Both are verified —
a mismatched key and a receipt against a non-`SUCCEEDED` step are both refused.

`IS NOT` rather than `=` because `NULL <> NULL` is `NULL`, which a `WHEN` treats as
false, and a receipt whose step row is missing would otherwise slip past. The
`REFERENCES` foreign key catches that case separately, but the trigger must not
depend on it.

**`UNIQUE (step_id)` is the durable duplicate-suppression seam** for the P2 slice:
a second receipt for the same step is refused, so a retried commit cannot append a
second proof of the same effect. The broader duplicate *window*
(`(capability_id, arguments_digest)` across tasks,
[Bounds Protocol §5](../protocols/10-bounds-protocol.md#5-duplicate-suppression))
belongs to `serea-capability` in P5 and gets its own index there.

**A constraint that was removed rather than weakened into fiction.** An earlier
draft required `provider_reference IS NOT NULL OR replay_safe = 0`, justified as
encoding Capability Protocol §5.1's "A receipt without one is valid only for
`side_effect_class: LOCAL_STATE`". It does not encode that. `side_effect_class` is
a descriptor field and P2 has no registry; the substitute both refuses a legal
`LOCAL_STATE` receipt with `replay_safe: true` and accepts an illegal
non-`LOCAL_STATE` receipt with `replay_safe: false`. Nothing in §5.1 or in
§3.1's `replay_safe` semantics says a `LOCAL_STATE` receipt is not replay-safe —
mutating Serea's own state is among the most replayable cases there is. The
constraint is deleted and `provider_reference ⇒ side_effect_class == LOCAL_STATE`
is recorded as a **P5 obligation**, consistent with how the other half of `C4` is
deferred.

### 4.6 `leases`

```sql
CREATE TABLE leases (
  step_id        TEXT    PRIMARY KEY REFERENCES task_steps(step_id) ON DELETE CASCADE,
  owner          TEXT    NOT NULL,
  generation     INTEGER NOT NULL CHECK (generation BETWEEN 1 AND 4294967295),
  acquired_at_ms INTEGER NOT NULL,
  expires_at_ms  INTEGER NOT NULL,
  released_at_ms INTEGER
) STRICT;
```

**The `token` column has been removed.** ADR-0024 originally carried a 16-byte
unguessable `token` alongside `generation`, and simultaneously conceded that
`generation` alone already discriminates two acquisitions of the same owner after
a reclaim. A second value that must be kept consistent with the first, and that no
statement actually needed, is overengineering. The fence is `(step_id, task_id,
generation, owner)`, and it is complete: `generation` is monotonic, durable, and
increments on every acquisition including an expiry reclaim.

`task_steps.lease_generation` and `leases.generation` are two copies of one fact.
That duplication is deliberate — the step-side copy is what makes the fence
predicate address the step directly — outcomes additionally EXISTS the indexed,
authoritative unreleased lease row — and **it is kept consistent
by derivation, not by a trigger**:

```sql
-- statement 2 of acquire, in ADR-0024's order
UPDATE task_steps
   SET ...,
       lease_generation = (SELECT generation FROM leases WHERE step_id = :step_id)
 WHERE ...
```

Both writes are inside one `BEGIN IMMEDIATE`, and the step-side copy is *read
from* the `leases` row rather than guessed, so no observer can see them disagree.

**There is no `leases_generation_matches_step` trigger, and this section
previously published one.** The P2 autonomous audit removed it and recorded why,
because the earlier text was not merely redundant — following it breaks lease
acquisition outright:

| | |
| --- | --- |
| Occurrences of the trigger in §4.0's migration | **0** |
| First acquisition, without it | succeeds — `LEASED`, `attempt = 1`, `generation = 1` |
| First acquisition, with it | **`REFUSED: lease generation must match the step`** |
| Expiry reclaim, with it | succeeds, because the trigger is `BEFORE INSERT` and the upsert's `ON CONFLICT DO UPDATE` branch never reaches it |

Three documents already said the trigger was removed — §7's acceptance table,
[ADR-0024](../decisions/ADR-0024-lease-fencing-and-commit-under-lease.md)'s
`acquire` section, and the design's §13.1 row B2 — and this one section still
carried its SQL and the claim *"it is verified — a mismatched generation is
refused"*. That claim was never true of the shipped schema.

The failure is structural, not a typo. The trigger is `BEFORE INSERT ON leases`,
and a first acquisition is the upsert's **INSERT** branch carrying
`generation = 1`, while the step's `lease_generation` is still `0`. So `1 <> 0`
aborts, and **no lease can ever be acquired**. This is exactly the round-2 defect
the design records as fixed, reintroduced by an undisposed section. And because
the trigger only guards the INSERT branch, it would have enforced the invariant in
one of the two branches and silently skipped the other — which is the fragility
that motivated its removal in the first place.

### 4.7 `plan_revisions`

`plan_revision = 0` is the initial plan, so there is no separate `plans` table. The
`ON DELETE RESTRICT` to `blobs` means a plan document cannot be deleted while a
revision references it, which is what makes ADR-0018's "delete a superseded
`PLANNED` step" decision auditable.

`data_class_rank` is capped at `BETWEEN 0 AND 2`, matching `blobs`. An earlier
draft allowed 0–4 here while the foreign key could only ever be satisfied up to 2,
which left the column's own `CHECK` dead for its top two values. There is no
generated label here: nothing reads a class label off this table.

### 4.8 `task_blob_refs` and `step_blob_refs`

**Two tables, not one polymorphic `blob_refs(owner_kind, owner_id, …)`.** A
polymorphic owner cannot carry a foreign key, so right-to-delete would depend on
remembering to delete the references by hand — and Data Classification §8.2 step 2
requires the cascade to be correct in **one transaction**. Two tables cost a
duplicate definition and buy real referential integrity.

P2D adds no public reference/role API or task/step mutation. The composite FKs
remain in production 0001 and may be exercised with private SQL fixtures; the
whole-transition writers and blob+reference atomicity proof belong to **P2F**.

**`step_blob_refs` has three roles, all reserved for P2F writers.** `ARGUMENTS`
carries a `CAPABILITY`, `DELEGATE` or `VERIFY` step's arguments; `INSTRUCTION`
carries the host-written input document whose digest is `input_digest` for every
other kind; `RESULT` carries the result document. Without `INSTRUCTION` a
`MODEL_TURN` or `NOTIFY` step's mandatory `input_digest` would have no blob to
point at, and `PLANNED MODEL_TURN` would be unimplementable.

`task_blob_refs` has **two** roles, `PLAN` and `PLAN_REVISION`, and **no
`RESULT`** — an earlier draft listed a task-level `RESULT` role that nothing writes,
because `tasks.result_summary` is a `TEXT` column. Roles that no writer uses are
schema surface waiting to be misused.

The `EVIDENCE_PAYLOAD` role that Capability Protocol §7's `payload_reference`
implies is **not** included, because P2 writes no evidence. P5 adds it by
migration, which is a `CHECK` widening and not a table rewrite.

### 4.9 `task_journal`

ADR-0021's proposed design owns this table's later-phase semantics; its complete
DDL lives only in the production migration. This slice adds no journal writer.

```sql
CREATE TABLE task_journal (…)
```

**The `journal_kind` vocabulary is deliberately NOT `EventKind`.** An earlier
draft reused `TASK_CREATED`, `TASK_CANCELLED` and `TASK_COMPLETED` — three frozen
`EventKind` variants — inside a package whose own test O9 forbids P2 constructing
an `EventKind`. That is a second naming authority for values the Protocol Index §1
registry already assigns to `serea-event-bus`. The closed set here uses `TASK_*`,
`STEP_*`, `RECEIPT_*` and `RECOVERY_*` spellings that name **what the engine did**,
not events: `TASK_INSERTED`, `PLAN_PERSISTED`, `TASK_STATE_CHANGED`,
`STEP_LEASE_ACQUIRED`, `STEP_LEASE_RELEASED`, `STEP_ATTEMPT_STARTED`,
`STEP_COMMITTED`, `STEP_FAILED`, `STEP_RECONCILED_ABSENT`, `RECEIPT_RECORDED`,
`RECOVERY_DECISION`, `TASK_CANCEL_REQUESTED`, `TASK_TERMINAL`.

`TASK_DELETED` is **absent, deliberately**. `task_journal.task_id` carries
`ON DELETE CASCADE`, so a deletion journal row written before the delete is erased by
it, and one written after it is refused by the foreign key — the kind cannot survive
its own transaction. Deletion is instead reported by `DeletionOutcome`'s counts and by
the absence of rows, which is what Data Classification §8.2's "partial failure is
visible" actually needs. An earlier draft listed the kind in prose and in the DDL;
prose and DDL now agree.

**`attempt` is a column** because
[Event Protocol §2](../protocols/06-event-protocol.md#2-sereaevent) puts it in
`trace`, and a `STEP_ATTEMPT_STARTED` or `STEP_COMMITTED` row cannot be turned into
an event without it.

**`payload_json` exists, so the journal is a complete record — not so that events
can be fabricated from it.** An earlier draft asserted that the journal's columns
are "precisely the fields an `EventKind`-specific payload needs", so a P3 upgrade
could materialise every event without inventing data. That is false:
`CAPABILITY_COMPLETED`'s payload carries `duration_ms` and `output_digest`;
`BOUND_EXCEEDED` requires `bound_name`, the limit and the observed value;
`MODEL_CALLED` requires `model_id`, `purpose` and a token estimate. None was in the
column list. `payload_json` — validated JSON, carrying whatever the transition's
event payload will need — is the honest fix, and `payload_ref_digest` points at a
blob for large payloads.

The P2 autonomous audit then removed the *purpose* the columns were being
justified by. Since ADR-0021 no longer reconstructs events, "the payload has
somewhere to live" is not a back-fill property at all. What the columns actually
buy is that **the pre-P3 history is complete and readable** — which is the real
requirement, and the one `T5` depends on. A journal that could not answer "what
happened to this task" would make recovery's idempotence claim unverifiable, and
that is worth the columns whether or not an event bus ever exists.

**`event_seq` is absent, and P2 never wanted it.** An earlier draft carried a
nullable `event_seq` for P3 to back-fill, plus a partial index to count the
backlog. The P2 autonomous audit removed both, because ADR-0021 no longer performs
that back-fill: a `SereaEvent` reconstructed in a later transaction cannot satisfy
`E3` for a transition whose transaction is gone, and the historical material is
already durable here. A column reserved for a write that must never happen is a
second source of truth for "did this transition get an event", which is the kind of
disagreement this schema refuses everywhere else.

`pending_event_transitions` therefore needs no column. In a build with no event
participant — that is, P2 — **every** journal row is pre-event history, so the
count is the row count. It remains a real number with a real meaning: it is the
size of the period during which `E3` did not hold, visible from inside the product.
It is not a queue to drain.

`journal_seq` is gapless **per task**, computed inside the transaction as
`MAX(journal_seq) + 1` for that `task_id`. It is a different thing from `Seq`: `seq`
is per-*host* and gapless by `E4`, and belongs to P3. Keeping the two distinct
avoids a P2 counter masquerading as the event sequence.

## 5. Content-addressed blobs

P2D accepts **original UTF-8 JSON bytes**, not arbitrary binary or a parsed
`Value`. PLAN/PLAN_REVISION/ARGUMENTS/INSTRUCTION/RESULT are JSON documents;
ADR-0019 SCJ-1 refusals apply, including duplicate keys, invalid UTF-8/JSON,
non-integer numbers and structural depth. Fractional model temperature remains
wire-valid but noncanonicalizable; no coercion or universal wire-JSON admission
is promised. Raw-byte provenance remains a caller obligation.

### 5.1 Write path

```text
Tx::put_blob(bytes, class)
  1. class dispatch, before lookup or any dedupe success:
       SECRET | CREDENTIAL                 -> StoreError::ClassRefused
       PRIVATE and no backend              -> StoreError::AtRestProtectionUnavailable
       PUBLIC | PERSONAL | PRIVATE+backend  -> continue
  2. canonical := canonicalize(original JSON bytes)  -- ADR-0019 SCJ-1
       input refusal -> StoreError::CanonicalJson
  3. digest := sha256(canonical plaintext)
  4. exact lookup of (digest, data_class_rank):
       existing row -> verify by the full read path below, then reuse BlobRef
       missing row  -> continue
  5. PUBLIC | PERSONAL -> content = canonical, protection = 'NONE'
     PRIVATE           -> content = protect(canonical), protection = 'AT_REST'
       backend refusal/failure -> StoreError::AtRestProtectionFailed
  6. size_bytes := stored content byte length; INSERT INTO blobs (...)
       never treat a conflict as success without the same existing-row verification
  7. return BlobRef; caller's transact controls COMMIT/rollback
```

The PRIVATE-only object-safe `AtRestProtection: Send + Sync` has
`protect/unprotect(&self, &[u8]) -> Result<Vec<u8>, AtRestProtectionError>`; the
error is a payload-free unit, with no backend diagnostic/source chain. No class
parameter or capability list is needed. Store owns
`Option<Arc<dyn AtRestProtection>>`; default constructors have no backend and
the two protection constructors reuse P2C's unchanged open path. Backend bytes
are an opaque, possibly nondeterministic envelope; the digest remains the
canonical plaintext digest, never a ciphertext identity.

Dedupe verifies protection marker, stored length, unprotect, SCJ-1 and digest;
`INSERT OR IGNORE` is not integrity verification. There is no expected-digest
write parameter or `DigestMismatch` variant; G5 now covers corrupt-existing-row
dedupe refusal. Standalone blob rows are allowed in P2D. It offers no reference
writer, so blob rollback proves neither orphan prevention nor blob+reference
atomicity. P2F must attach references within the same whole-transition transaction;
crash evidence belongs to P2H.

### 5.2 Read path

```text
Tx::get_blob(ref)
  1. dispatch ref.class as on write: SECRET/CREDENTIAL refuse;
       PRIVATE without backend refuses, even for an existing row
  2. SELECT content, protection, size_bytes, data_class_rank
       FROM blobs WHERE digest = ref.digest AND data_class_rank = ref.class.rank()
       zero rows -> StoreError::BlobMissing
  3. verify marker (NONE for PUBLIC/PERSONAL, AT_REST for PRIVATE)
       and size_bytes == stored content byte length; invalid -> StoreError::BlobCorrupt
  4. PRIVATE -> plaintext := unprotect(content)
       backend refusal/failure -> StoreError::AtRestProtectionFailed
     PUBLIC | PERSONAL -> plaintext := content
  5. re-canonicalize plaintext and verify sha256(canonical) == ref.digest
       invalid SCJ-1 or digest mismatch -> StoreError::BlobCorrupt
  6. return canonical plaintext bytes
```

The same verification governs dedupe success. A digest never re-verified detects
nothing; neither marker acceptance nor a successful SQL lookup proves content
integrity. Later P2G recovery can classify this failure as
`CorruptOrInvariantViolation`; that consumer is not implemented by P2D.

### 5.3 Classification and laundering

The composite primary key `(digest, data_class_rank)` is the safety-relevant
choice. Keyed by `digest` alone, a reference classified `PERSONAL` could resolve a
blob stored `PRIVATE`, laundering a private payload downward — which `DC3` forbids
and which no `CHECK` on a single class column would catch. With a composite key, a
reference at class `X` uses an exact lookup and can only resolve a row stored at
class `X`; no digest-only fallback is permitted. `BlobRef` privately holds a
validated `Digest` and `DataClass`, with a public constructor, read accessors and
payload-safe `Debug`, but no `Serialize`. It is identification, not host
authorization or content-classification inference. A forged PUBLIC or PERSONAL
reference to a PRIVATE-only row yields `BlobMissing`, not lower-class access.

Deduplication still works for the case that matters: the same arguments written
twice at the same class are stored once. Storing identical bytes at two classes is
wasteful and safe, which is the correct trade for a store that decides what may be
persisted.

**A consequence worth stating:** `StoreError::ClassEscalationRequired`, which an
earlier draft carried, has no role in this API. Exact lookup never substitutes a
higher-class row, and composite FKs reject mismatched durable references when
enforcement is enabled. A caller can construct an identifying `BlobRef`, but
that does not authorize declassification or make an absent exact row exist.

**The guarantee's real boundary, which the P2 autonomous audit added.** The
anti-laundering property above rests on `PRIMARY KEY (digest, data_class_rank)`
**and on the composite `FOREIGN KEY` clauses** that carry a reference's class to
the blob's row. `PRAGMA foreign_keys` is an ordinary per-connection setting:
**upstream SQLite defaults to `OFF`; the selected bundled build defaults to `ON`**
(`SQLITE_DEFAULT_FOREIGN_KEYS=1`). Store must still set and assert `ON` explicitly;
neither default protects against a local writer, and one statement disables it:

```sql
PRAGMA foreign_keys = OFF;   -- outside a transaction: takes effect
BEGIN IMMEDIATE;
INSERT INTO task_blob_refs (task_id, role, digest, data_class_rank)
VALUES (?, 'PLAN', ?, 0);   -- references a blob stored at class 2: ACCEPTED
COMMIT;
```

No pragma trickery and no privilege beyond file write: the historical SQL probe
accepts a **dangling PUBLIC reference** when only a PRIVATE blob row exists. That
breaks referential integrity; it does not make P2D's exact `(digest, rank)` read
resolve the PRIVATE row. The FK guarantee's honest boundary is the same one
ADR-0022 makes for `ignore_check_constraints`, applied symmetrically:

> The composite FKs provide a **structural, pragma-dependent** guarantee that
> durable references have matching blob rows while `foreign_keys = ON`. A local
> file writer can disable that guarantee with one line. P2D separately requires
> exact `(digest, rank)` lookup and content verification; it claims no protection
> against a local writer rewriting rows or classifying copied content. See §7.

This does not weaken the design; it names the boundary the threat model already
excludes. [Trust Boundaries §2 `TB-7`](../architecture/02-trust-boundaries.md#tb-7-core-to-durable-store)
puts filesystem permissions at "defence in depth, not the mechanism" and
[Security Invariants §6](../threat-model/04-security-invariants.md) records
tamper-evidence against a local file writer as "Not specified". Inventing a
defence against an attacker the threat model excludes would re-open the question
ADR-0020 just closed. What was missing was not a control — it was the sentence.

### 5.4 Deletion

Deferred **P2F** whole-transition design, not a P2D API or runtime proof.
Task Protocol §8 and Data Classification §8.2 step 2, in one transaction:

```sql
DELETE FROM tasks WHERE task_id = ?;      -- cascades to task_steps,
                                           -- side_effect_receipts, leases,
                                           -- plan_revisions, task_blob_refs,
                                           -- task_journal; step_blob_refs
                                           -- cascades from task_steps
```

then, **scoped to this digest and class**:

```sql
DELETE FROM blobs WHERE data_class_rank <= 2
  AND NOT EXISTS (SELECT 1 FROM step_blob_refs r
                   WHERE r.digest = blobs.digest AND r.data_class_rank = blobs.data_class_rank)
  AND NOT EXISTS (SELECT 1 FROM task_blob_refs r
                   WHERE r.digest = blobs.digest AND r.data_class_rank = blobs.data_class_rank)
  AND NOT EXISTS (SELECT 1 FROM plan_revisions r
                   WHERE r.plan_digest = blobs.digest AND r.data_class_rank = blobs.data_class_rank);
```

**The `NOT EXISTS` subqueries must be correlated on `blobs.digest`.** Written
without the correlation, each is a full cross-product scan that is trivially true,
and the statement deletes *every* unreferenced blob in the file — including blobs a
future phase wrote to a table that does not exist yet. Verified: a referenced blob
survives the sweep and an unreferenced one is removed.

**Retention is not P2's to enforce.** `max_retained_tasks` is a
[Bounds Protocol §2](../protocols/10-bounds-protocol.md#2-the-bound-set) bound and
Crate Map §3.1 gives bound configuration and global counters to `serea-core`.
P2F provides `delete_task`; the **30-day retention trigger is P12's**, because the
notification surface and the bound configuration are both `serea-core`'s and
arriving together in P12 keeps the trigger with the configuration it reads. Task
Protocol §8's retention *interval* is honoured by P2 only as a value
`serea-core` supplies. P2 enforces no retention bound, and this is a non-claim.

## 6. Indexes

| Index | Table | Serves |
| --- | --- | --- |
| `UNIQUE (task_id, sequence)` | `task_steps` | Step ordering and Task Protocol §3.2's "required predecessors" read |
| `UNIQUE (task_id, idempotency_key)` | `task_steps` | Duplicate-key detection within a task |
| `task_steps (status, lease_expires_at_ms)` | `task_steps` | Recovery: expired in-flight leases |
| `task_steps (task_id, status)` | `task_steps` | Per-task progress reads |
| `tasks (state)` | `tasks` | Recovery: load non-terminal tasks |
| `step_blob_refs (digest)` | `step_blob_refs` | The correlated `NOT EXISTS` in the §5.4 sweep |
| `task_blob_refs (digest)` | `task_blob_refs` | Same |
| `plan_revisions (plan_digest)` | `plan_revisions` | Same, and the `ON DELETE RESTRICT` lookup |
| `UNIQUE (task_id, journal_seq)` on `task_journal` | `task_journal` | The ordered `journal_for_task` read. ADR-0021's `pending_event_transitions` needs no index: with no `event_seq` column the count is the row count |

Indexes **deliberately absent**, each because no stated query needs it:

- **No index on `blobs.content`.** Nothing queries a blob by content.
- **No index on `leases (expires_at_ms)`.** The same recovery query is served by
  `task_steps (status, lease_expires_at_ms)`; `leases` is keyed by `step_id`.
- **No index on `side_effect_receipts (task_id)` or `(idempotency_key)`.** P2's
  only receipt query is `receipt_for_step`, served by `UNIQUE (step_id)`. An
  earlier draft justified the key index as serving "reconciliation", but
  reconciliation is explicitly **not** claimed in P2.
- **No index on `task_journal` beyond its `UNIQUE (task_id, journal_seq)`.** That
  unique constraint serves the ordered read. An earlier draft also carried a
  partial index on `WHERE event_seq IS NULL`, described as serving "a bounded
  set"; the column it indexed no longer exists, and `task_journal` grows with a
  task's lifetime, so the bound was unearned in any case.
- **No index for `pending_event_transitions`.** With no `event_seq` column the
  count is the row count, which is `COUNT(*)` over the table — a metric an operator
  reads, not a predicate a query runs.

## 7. Verified behaviour

The following table preserves **historical** probe results, not a claim of
production coverage or of complete current presence/fence validation. Original
32-cell/69-check runs did not exercise partial errors, release-before-outcome or
u32 overflow. Current additions are checked by the docs probe linked from
[the gate](P2A-review-and-closure.md); runtime tests remain deferred.

### Current production-migration probe (2026-10-04)

Executed from the workspace root **after the frozen P2C gate**, using the final
production SQL bytes (Python 3.9.6's SQLite **3.43.2**, Node **v24.21.0**).
This is not a bundled rusqlite/Store test or a P2C runtime completion claim.
The existing tracked harness was rerun against production; its positive corpus
includes all 51 legal step cells. Added task-pair construction covers all 37
legal pairs, not engine enforcement. The old 69/69 run remains historical and
is **not** being claimed as rerun or enlarged.

Command: `PYTHONDONTWRITEBYTECODE=1 python3 docs/plans/p2a-doc-probes.py`

```text
ECMA-262 v24.21.0: 10119638 assertions; all Unicode scalars in three positions/categories, pinned whitespace and exact identifier/near-miss corpus PASS
Production migration: 10 tables, 7 triggers, 6 explicit indexes PASS; SHA-256 d9068dccbc26ececb71be79c475080633166ba0163c62b2d98b9733512baefea
EpochMillis: 126 exact production-column boundary/NULL probes across all 14 instants PASS (other table invariants tested separately)
Migration checksum grammar: 9 positive/negative probes PASS
SQLite semantics: integer→TEXT accepted; BLOB→TEXT/nonnumeric TEXT→INTEGER refused; memory checkpoint (0, -1, -1) PASS
SQLite 3.43.2: 531 probes against production migration; kind/status (51 legal cells), all error subsets, bounded explanatory u32 parity, outcome/release/expiry and overflow PASS
Positive task constructibility: 37/37 documented legal transitions PASS against production migration (not engine runtime validation)
SCJ-1/IDK-1: 21 published vectors reconstructed PASS; vector-1 282 bytes; raw A/B private framing only, valid-ID scalar/object lengths pinned
```

Additional checks executed **at that historical probe snapshot** (not current
smoke-parser or integrated-workspace evidence):

- Extraction parity against original HEAD §4.0: exact bytes after removing only
  the 14 instant bounds and narrow checksum hex/NUL defense. Full corrected
  objects and counter/duration constraints unchanged.
- Both then-current Python sources compiled on Python 3.9 without bytecode output.
  Final F6 smoke remediation preserves Python 3.9 compatibility with a stdlib-only
  focused TOML parser. It consumes the entire supported input and fails closed on
  unsupported syntax; 37 parser/layering regression cases pass on Python 3.9.6.
- Workspace smoke synthetic in-memory fixtures: **12** positive/negative cases
  PASS, covering exact membership, duplicate/extra/missing member, path/package
  aliases, workspace inheritance and target/build/dev tables. No fixture files
  written.
- `python3 tools/validate_docs.py docs`: **57 Markdown files**, identifiers/JSON/
  placeholders/cross-references valid. `git diff --check`: no findings.

**Historical integration problems at that disjoint probe snapshot, not current
workspace status or hidden GREENs:** the then-current
`python3 tests/workspace_smoke.py` exited 1 with:

```text
FAIL: expected exactly P2C members ['crates/serea-protocol', 'crates/serea-storage', 'crates/serea-testkit'], got ['crates/serea-protocol', 'crates/serea-testkit']; no engine until P2F
FAIL: required member has no manifest: crates/serea-storage/Cargo.toml

2 workspace invariant failure(s)
```

`cargo metadata --offline --no-deps --format-version 1` succeeds but lists only
protocol/testkit, so the new CI exact-three assertion exits 1 too. Cargo/Rust
wiring is deliberately outside this disjoint slice; do not weaken the guards to
make this partial tree GREEN. No Rust runtime/MSRV suite or GitHub CI execution
is claimed here. No closure/ADR change, P2D runtime or commit was made.

### Historical acceptance table

**Round 4** follows the third review pass and adds the positive cases it asked for
— including *each wait kind at `PLANNED`*, which rounds 1–3 all missed because every
one of them tested `WAITING` on a non-wait kind and none tested a non-`WAITING`
status on a wait kind. That single omission had made three of eight step kinds
unplannable.

| Case | Result |
| --- | --- |
| **The migration builds at all** | **10 tables, 7 triggers, 6 explicit indexes** — asserted, not printed, so a phantom object cannot be reintroduced. The seventh was the partial index on `event_seq`, removed with the column |
| Each of the seven lifecycle statuses is constructible | accepted |
| **All 8 step kinds × 7 statuses: 56 cells accounted for** | **51 constructible, 5 correctly refused** — the 5 are `WAITING` on a non-wait kind, which is ADR-0018's intent, not a gap |
| **All 37 legal Task Protocol §4.2 transitions are accepted** | 37/37 |
| All 84 illegal pairs were exercised | the schema does **not** and must **not** encode the transition table — that is `TaskEngine`'s `legal_task_transition`, pinned by test I1 |
| **All 32 presence-matrix `N`/`0` cells are refused** | 32/32 — **this was 24/32 before the audit**, and the eight gaps are listed in §4.4 |
| **Three control cells** (legal `EXECUTING`, legal `WAITING`, legal `SUCCEEDED`) | accepted, so the probe is not refusing everything |
| `SUCCEEDED` without `result_digest` | refused |
| `PLANNED` with `attempt = 1`, `lease_generation = 1`, or a lease owner | refused |
| `LEASED` with `started_at_ms` set | refused |
| `WAITING` on a `CAPABILITY` step | refused |
| **Each of `WAIT_APPROVAL`, `WAIT_USER`, `WAIT_SCHEDULE` at `PLANNED`** | accepted — the round-3 blocker. A wait step must be plannable before it can wait |
| **`WAIT_USER` reaching `WAITING`, `SUCCEEDED` (with a result), and `WAIT_APPROVAL` reaching `FAILED`** | accepted |
| `TASK_DELETED` as a journal kind | refused — the kind was removed |
| **`BLOCKED → CANCELLED` clearing `blocked_reason`** | accepted; and *without* the clear it aborts, which is why §4's cancel statement sets it |
| `lease_generation = 0` on a terminal step | refused; `99` is accepted, and that is stated rather than claimed fixed |
| `FAILED` without the five error fields | refused |
| `FAILED` with `details` omitted | **accepted** — the frozen `actionError.required` list omits `details` |
| `CAPABILITY` without `capability_id`; `NOTIFY` with one | refused |
| **`NOTIFY` with only `capability_id` set** — the partial tuple the single biconditional admitted | refused — the round-4 finding |
| `MODEL_TURN` with no `idempotency_key`; `VERIFY` with one | accepted |
| `capability_id` in the `goallatch` namespace | refused |
| `input_digest` of 71 colons; bad prefix; uppercase hex | refused |
| `policy_class` no-op update / actual change | accepted / refused |
| `data_class` raise / lower | accepted / refused |
| **`CANCELLED` from every non-terminal state** | accepted — the round-1 blocker |
| `CANCELLED` without `cancelled_at_ms`; a non-`CANCELLED` task carrying one | refused |
| **`BLOCKED → READY` and `BLOCKED → CANCELLED`, clearing `blocked_reason`** | accepted — the round-2 finding |
| `READY` carrying a `blocked_reason` | refused |
| `SECRET` or `CREDENTIAL` on **`tasks`, `blobs`, `side_effect_receipts`, `plan_revisions`, `task_journal`** | refused — all five, all 8 probes |
| `PRIVATE` journal row | accepted |
| Receipt key or `task_id` disagreeing with its step; receipt on a non-`SUCCEEDED` step | refused |
| Journal `step_id` belonging to another task | refused |
| Second receipt for one step | refused |
| Lease expiring at or before its acquisition | refused |
| **`PLANNED → LEASED` in the documented order** | accepted, `attempt = 1`, `generation = 1` |
| **Reclaim after a released lease** | accepted, `generation` 1→2, `attempt` 1→2, `started_at_ms` cleared, both copies agreeing |
| **Worker A at generation 1 after B reclaimed at 2: A's commit** | **0 rows**, step unchanged |
| **The same fence across two independent connections on one file** | **0 rows** — no process-local mutex participates |
| **`attempt` after acquire *and* after `begin_attempt`** | **1** — incremented exactly once |
| **Attempt ceiling with rollback**, `max_attempts_per_step = 0` | step left `PLANNED`, `attempt = 0`, **no `leases` row** |
| **Attempt ceiling with rollback**, ceiling 2, third acquisition | step reverts to `('LEASED', 2, 2)`; exactly one `leases` row remains |
| **A crash-only loop against `max_attempts_per_step = 2`** | **2 acquisitions refused, 0 executions** — an expiry reclaim spends an attempt |
| Referenced / unreferenced blob through the §5.4 sweep | survives / removed |
| Duplicate migration `version` / `name` | refused / refused |
| Zero-length file ⇒ fresh; foreign file ⇒ `NotSereaStore` | Historical probe distinguished 0 tables vs `['unrelated']`. Current gate uses **file length**, not table count: any nonempty SQLite file without the catalog is foreign, including zero user tables |
| Two OS processes, 40 writes between them | all 40 landed, `quick_check` ok |

### Two rows that are accepted by SQL on purpose

| Case | Why accepted |
| --- | --- |
| `BLOCKED` with no `blocked_reason` | The schema check is one-way (`state = 'BLOCKED' OR blocked_reason IS NULL`) because a biconditional strands any `BLOCKED → X` transition. The *engine's* `block` statement always sets the reason; SQL cannot enforce presence on entry and absence on exit simultaneously |
| A `leases` row whose `generation` disagrees with its step | There is **no trigger** — see §4.6, where the P2 autonomous audit explains at length why publishing one made acquisition impossible. The step-side copy is *derived* — `lease_generation = (SELECT generation FROM leases WHERE step_id = ?)` — and both writes happen inside one `BEGIN IMMEDIATE`, so no observer can see them disagree |

### The pragma boundary, in full

This schema's structural guarantees are per-connection settings, not properties of
the file. Two settings matter, and **both** must be named. The P2 autonomous audit
verified each independently against a migrated database.

**`PRAGMA ignore_check_constraints = ON`** disables every `CHECK` here. Confirmed
by execution: a local writer set `tasks.data_class` to `SECRET` through it.

What still holds under that pragma, because triggers and foreign keys are not
`CHECK`s: `tasks_policy_class_immutable`, `tasks_data_class_monotonic`,
`side_effect_receipts_key_matches_step`, `side_effect_receipts_task_matches_step`,
`side_effect_receipts_step_must_succeed`,
`task_steps_idempotency_key_immutable`, `task_journal_step_task_matches`, and every
`REFERENCES`. **Each was verified to fire with the pragma set** — this design's
claim, and it holds. The structural budget is therefore spent on the controls that
survive, which are the authority-bearing ones.

**`PRAGMA foreign_keys = OFF`** disables foreign-key enforcement, and it is the
more direct of the two: one explicit setting turns it off. Upstream SQLite's
default is `OFF`, but the selected bundled build defaults to **ON**; Store must
still set and assert ON explicitly. Confirmed by historical execution: with it off,
a `task_steps` row
referencing a non-existent task is accepted. This is what makes §5.3's
cross-class anti-laundering guarantee pragma-dependent rather than structural.

So the design's structural budget is real but bounded, and the boundary is stated
once rather than implied:

> `CHECK`s require `ignore_check_constraints = OFF`; foreign keys require
> `foreign_keys = ON`. Ordinary triggers are independent of both settings.
> A local
> file writer can disable either with one line and needs no privilege beyond write
> access. [Trust Boundaries §2 `TB-7`](../architecture/02-trust-boundaries.md#tb-7-core-to-durable-store)
> already states that filesystem permissions are "defence in depth, not the
> mechanism", and [Security Invariants §6](../threat-model/04-security-invariants.md)
> records tamper-evidence against a local file writer as "Not specified". No defence
> against that writer is claimed, and none is invented.

ADR-0022 remains **Proposed**, as do ADR-0021/0024. O14/O15 are later runtime
test obligations for this boundary; schema construction is not runtime acceptance.

## 8. Open at implementation time

| # | Question | Resolution |
| --- | --- | --- |
| 1 | Is the resolved SQLite >= 3.37.0 | **Resolved by the P2 autonomous audit and re-verified by the final closure run.** `rusqlite` 0.40.2 with `bundled` compiles SQLite **3.53.2** (`SQLITE_SOURCE_ID` `2026-06-03 19:12:13 d6e03d8c…`) from source — read from `libsqlite3-sys/sqlite3/sqlite3.h`, and confirmed by `SELECT sqlite_version()` on a live connection. The `STRICT` / `GENERATED … STORED` fallback below is therefore **verified unnecessary for this candidate**, and is retained only as a branch for a hypothetical future `rusqlite` whose bundled SQLite predates 3.37.0. It is **not** contingent on any MSRV decision: the closure run established that the chosen configuration builds and runs on the workspace's existing `1.85` MSRV — see [the design §7.4](P2-storage-task-engine.md#74-rusqlite-and-the-alternatives) |
| 2 | Is JSON1 present | Verified by `SELECT json_valid('{}')` at open time; the open fails if absent, because `tasks.extensions` depends on it. Bundled SQLite 3.53.2 has the JSON functions compiled in by default |
| 3 | Does `length()` count bytes on a `BLOB` | **Closed.** Verified: `length(X'7B7D')` is 2, so `CHECK (size_bytes = length(content))` accepts a two-byte blob with `size_bytes = 2`. Not open |
| 4 | Does `PRAGMA foreign_key_check` belong in the open path | **Added by the P2 autonomous audit.** It is the only one of the three integrity pragmas that sees a referential violation; see [the design §7.1](P2-storage-task-engine.md#71-migrations) for the four tiers |

## 9. Cross-references

- The engine and API that use this schema:
  [P2 storage and task engine](P2-storage-task-engine.md)
- The tests that prove it: [P2 test matrix](P2-test-matrix.md)
- The decisions this schema implements: [ADR-0018](../decisions/ADR-0018-taskstep-lifecycle-and-field-presence.md),
  [ADR-0021](../decisions/ADR-0021-p2-p3-event-atomicity-seam.md),
  [ADR-0022](../decisions/ADR-0022-durable-private-data-at-rest.md),
  [ADR-0024](../decisions/ADR-0024-lease-fencing-and-commit-under-lease.md)
