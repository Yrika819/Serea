# P2 Storage and Task Engine Design

- **Branch:** `p2/design-preparation`
- **Base commit:** `c3737039e3e38dbba554dc0b9075025f87948358`
- **Scope:** "SQLite storage and durable `AssistantTask` lifecycle/recovery"
- **Status:** design only. No production Rust, no SQLite code, no
  `serea-storage` or `serea-task-engine` source, and no dependency change by this
  run.
- **Companions:** [contract gap analysis](P2-contract-gap-analysis.md),
  [SQLite schema](P2-sqlite-schema.md), [test matrix](P2-test-matrix.md)

## 1. Scope, and the crates in it

P2 creates exactly two runtime crates and no others:

```text
serea-protocol        (exists)
   ↓
serea-storage         (new, L1)
   ↓
serea-task-engine     (new, L3)
```

`[Crate Map §2](../architecture/03-crate-map.md#2-dependency-layers)` permits
`serea-task-engine → serea-storage` and `serea-task-engine → serea-protocol`.
`serea-task-engine` does **not** name `serea-event-bus`, `serea-policy`,
`serea-model-router`, `serea-capability` or `serea-memory` in P2, because none of
those crates exists yet and [Crate Map §4.4](../architecture/03-crate-map.md#44-planned-crates-this-architecture-would-defer)
declines to create a crate whose contract has not earned its shape.

Explicitly **not** created, in either a placeholder or a stub form:

| Crate | Why not |
| --- | --- |
| `serea-event-bus` | P3. ADR-0021 |
| `serea-policy` | P6 |
| `serea-model-router` | P4 |
| `serea-capability` | P5 |
| `serea-memory` | P7 |
| `serea-scheduler` | P3 |
| `serea-core` | P12 |

A placeholder crate that is never called is not a smaller slice; it is a public API
that must then be preserved, and a second owner for whatever it declares. §2.2 of
the Crate Map is the rule: ports live in the lowest layer, implementations in the
highest, and P2 implements neither.

The **final** dependency graph in Crate Map §2 describes *allowed eventual*
dependencies. P2 is not required to realise it.

## 2. What P2 does and does not do

| P2 does | P2 does not |
| --- | --- |
| Own durable task, step, receipt, lease, plan and journal state | Call a model |
| Own ordered migrations and the store's connection policy | Call a provider |
| Own the content-addressed blob store and class enforcement | Evaluate policy |
| Own the transition table, attempt accounting and cancellation | Consume or evaluate an approval |
| Own lease acquisition with fencing | Run a scheduler wake |
| Own restart recovery as durable classification | Connect GoalLatch or Local MCP |
| Own `delete_task` and the blob cascade | Enforce retention (the 30-day trigger is **P12's**), or any §2 bound other than `max_attempts_per_step` |

The single sentence that keeps P2 from growing: **P2 records outcomes; it never
produces them.** `TaskEngine::commit_step` takes a `StepOutcome` that something
else already produced. The provider boundary stays in P5.

## 3. `serea-storage` public API

Crate Map §3 names `Store`, `StoreError`, `Tx`, `LeaseGuard`, `BlobRef`,
`BlobStore`, `Migrations`. Six of the seven are kept. One is deliberately
**dropped**, and the deviation is stated here before any implementation rather
than discovered during it.

```rust
pub struct Store { … }
impl Store {
    pub fn open(path: &Path, clock: &dyn Clock,
               protection: Option<&dyn AtRestProtection>) -> Result<Self, StoreError>;
    pub fn open_in_memory(clock: &dyn Clock,
                          protection: Option<&dyn AtRestProtection>) -> Result<Self, StoreError>;
    pub fn schema_version(&self) -> Result<u32, StoreError>;
    pub fn verify_integrity(&self) -> Result<(), StoreError>;
    pub fn view<T>(&self, f: impl FnOnce(&dyn TaskQueries) -> Result<T, StoreError>)
                  -> Result<T, StoreError>;
    pub fn transact<T>(&self, f: impl FnOnce(&mut Tx) -> Result<T, StoreError>)
                      -> Result<T, StoreError>;
}

pub struct Tx<'c> { … }
pub struct LeaseGuard { … }          // ADR-0024; not Clone, taken by value
pub struct BlobRef { digest: Digest, class: DataClass }   // Copy, Debug-safe
pub struct Migrations { … }
pub trait AtRestProtection { … }     // ADR-0022
```

### 3.1 The dropped name: `BlobStore`

Crate Map §3 lists `BlobStore` as a `serea-storage` type. **P2 does not define
it**, and the reason is a correctness one rather than a preference.

A `BlobStore` that owns its own connection or its own transaction can write a blob
*outside* the caller's transaction. That is exactly the torn write this design
exists to prevent: a blob committed with no reference is an orphan, and a
reference committed with no blob is a dangling row. In the filesystem variant of
the same mistake the window is unavoidable; in SQLite it is avoidable and choosing
`BlobStore` would choose the window.

So the blob operations are `Tx::put_blob` and `Tx::get_blob`, and `BlobRef` — the
type that identifies a blob — is kept, because it appears in the public API for
callers that hold one across transactions.

`LeaseOwner` is a `serea-protocol` type, so `serea-storage` names no new
identifier. `Clock` and `AtRestProtection` are traits `serea-storage` *consumes*;
`Clock` is declared in `serea-protocol` per Crate Map §3.1, `AtRestProtection` is
declared in `serea-storage` because no other crate needs it.

### 3.2 The property that makes unsafe sequences impossible

The prompt's bad example:

```text
update_step_success()
later_update_task()
later_append_receipt()
```

P2 has **no** such methods. `Store` has exactly two operation kinds:

- `view` — read-only, cannot write;
- `transact` — the only write path, and it owns `BEGIN IMMEDIATE … COMMIT`.

And `Tx` exposes **whole transitions**, never row-level updates:

| `Tx` method | What one call does |
| --- | --- |
| `insert_task` | Validate, `INSERT tasks`, append `TASK_INSERTED` |
| `put_plan_revision` | Validate the whole plan, insert the revision blob, insert every step, transition the task, append `PLAN_PERSISTED` |
| `acquire_lease` | `leases` upsert (the generation authority) → `status = 'LEASED'` with the generation **read back from `leases`** → `STEP_LEASE_ACQUIRED` |
| `renew_lease` | Fenced expiry extension. `LeaseExpired`, never a silent renewal |
| `release_lease` | Fenced release → `STEP_LEASE_RELEASED` |
| `begin_attempt` | Fenced `status = 'EXECUTING'`, `started_at_ms` stamped, task → `EXECUTING`, journal. **`attempt` is not touched** — it was consumed at acquisition |
| `commit_step_succeeded` | Fenced step write **+ receipt + result blob + task transition + `STEP_COMMITTED`/`RECEIPT_RECORDED`**, one transaction |
| `commit_step_failed` | Fenced step write with the error + task transition + `STEP_FAILED` |
| `close_step_reconciled_absent` | Fenced step close + task transition + `STEP_RECONCILED_ABSENT` |
| `delete_task` | Cascade + blob sweep. **No journal row** — `task_journal.task_id` cascades with the task, so a deletion record cannot survive its own transaction. `DeletionOutcome`'s counts are the record |
| `append_journal` | One journal row, called by the above |

There is no `update_task_state`, no `set_step_status`, no `insert_receipt`. A
caller cannot compose `T4` wrongly because it cannot compose it at all.

### 3.3 Reads

Read queries live on a `TaskQueries` trait implemented by both `Store` and `Tx`,
so a caller inside a transaction sees the same API as a caller outside it without
duplicating every query:

```rust
pub trait TaskQueries {
    fn task(&self, task_id: TaskId) -> Result<Option<TaskRow>, StoreError>;
    fn task_with_steps(&self, task_id: TaskId) -> Result<Option<TaskWithSteps>, StoreError>;
    /// `StepPhase` is `serea-task-engine`'s **closed** seven-value lifecycle, not
    /// the open `StepStatus` wire code. A signature taking the open code could
    /// express a value `task_steps.status`'s `CHECK` refuses, which is exactly the
    /// boundary ADR-0018 wants to keep. See schema §4.4.
    fn steps_in_phase(&self, phases: &[StepPhase]) -> Result<Vec<StepRow>, StoreError>;
    fn expired_leases(&self, now_ms: u64) -> Result<Vec<LeaseRow>, StoreError>;
    fn receipt_for_step(&self, step_id: StepId) -> Result<Option<ReceiptRow>, StoreError>;
    fn journal_for_task(&self, task_id: TaskId) -> Result<Vec<JournalRow>, StoreError>;
    fn pending_event_transitions(&self) -> Result<u64, StoreError>;   // ADR-0021
    fn plan_revision(&self, task_id: TaskId, revision: u32) -> Result<Option<PlanRevisionRow>, StoreError>;
}
```

### 3.4 `StoreError`

Hand-written, no `thiserror`, matching the P1 decision in `Cargo.toml`: a derive
macro puts a `Display` impl next to the rejected values and makes it easy to add a
field that formats untrusted input.

`Open`, `Sqlite`, `NotSereaStore`, `SchemaTooNew`, `MigrationChecksumMismatch`,
`IntegrityCheckFailed`, `Busy`, `ConstraintViolation`, `CanonicalJson`,
`DigestMismatch`, `BlobMissing`, `BlobCorrupt`, `ClassRefused { class }`,
`AtRestProtectionUnavailable { class }`,
`LeaseHeld`, `LeaseFenced`, `LeaseExpired`, `AttemptCeilingReached`,
`IllegalTaskTransition`, `IllegalStepTransition`, `DuplicateIdempotencyKey`,
`PolicyClassImmutable`, `Protocol(ProtocolError)`, `Clock(ProtocolError)`.

Two rules, both inherited from `errors.rs`:

1. **No rejected value appears in any `Display` or `Debug`.** The `Sqlite` variant
   carries the underlying error but renders a machine-readable cause only, because
   SQLite's own message can quote a bound value, and a bound value on this path can
   be `PRIVATE` prose.
2. **`StoreError` carries no lease identity beyond the step's own id and generation
   counter**, neither of which is replayable on its own.

## 4. Transaction boundaries

One row per required operation, each a single `transact`. "Atomic set" is the
guarantee, not an aspiration.

| Operation | One transaction contains |
| --- | --- |
| **create task** | Validate the spec and the frozen class/risk enums → `INSERT tasks` → `append_journal(TASK_INSERTED)` |
| **add / persist plan** | Validate every step against the ADR-0018 matrices → canonicalise and store each `ARGUMENTS` blob → `INSERT plan_revisions` → `INSERT task_steps` for all of them → `UPDATE tasks SET state = 'READY' WHERE task_id = ? AND state = 'PLANNING'` → `append_journal(PLAN_PERSISTED)` |
| **acquire lease** | The atomic `leases` upsert (generation authority) → `UPDATE task_steps SET status='LEASED', …, lease_generation=(SELECT generation FROM leases …), attempt=attempt+1 WHERE … AND lease_generation = <expected old> AND status IN ('PLANNED','LEASED','EXECUTING')` → `append_journal(STEP_LEASE_ACQUIRED)`. The order and the `status` set are load-bearing; see [ADR-0024](../decisions/ADR-0024-lease-fencing-and-commit-under-lease.md) |
| **begin attempt** | **Fenced** `UPDATE task_steps SET status='EXECUTING', started_at_ms = ? WHERE … fence … AND status = 'LEASED'` — **`attempt` is not touched**; it was consumed at acquisition → `UPDATE tasks SET state='EXECUTING' WHERE task_id = ? AND state IN ('READY','PLANNING','VERIFYING')` → `append_journal(STEP_ATTEMPT_STARTED)` |
| **commit successful step** | **Fenced** `UPDATE task_steps SET status='SUCCEEDED', result_digest, completed_at_ms, lease_owner=NULL, lease_expires_at_ms=NULL WHERE … fence … AND status='EXECUTING'` → zero rows ⇒ return `LeaseFenced` **here**, before anything else → `put_blob(RESULT)` → `INSERT side_effect_receipts` → `UPDATE tasks SET state = ? WHERE task_id = ? AND state = ?` → `append_journal(STEP_COMMITTED, RECEIPT_RECORDED)` |
| **commit failed attempt** | **Fenced** `UPDATE task_steps SET status='FAILED', error_*, completed_at_ms WHERE … fence …` → `UPDATE tasks SET state='FAILED' WHERE task_id = ? AND state = ?` → `append_journal(STEP_FAILED)` |
| **block task** | `UPDATE tasks SET state='BLOCKED', blocked_reason = ? WHERE task_id = ? AND state = ?` → `append_journal(TASK_STATE_CHANGED)` with `reason_code` set |
| **cancel task** | `UPDATE tasks SET state='CANCELLED', blocked_reason = NULL, cancelled_at_ms, cancelled_by WHERE task_id = ? AND state NOT IN ('COMPLETED','FAILED','CANCELLED')` → zero rows ⇒ `CancellationOutcome { changed: false }` and **no journal row** → `append_journal(TASK_CANCEL_REQUESTED)`. The `blocked_reason = NULL` is required: the schema's check is one-way, so a task leaving `BLOCKED` must clear it, or the statement aborts instead of returning `changed: true` |
| **terminal completion** | The successful-step fenced write **plus** `UPDATE tasks SET state='COMPLETED', result_summary = ? WHERE task_id = ? AND state = 'VERIFYING'` → `append_journal(TASK_TERMINAL)` |
| **recovery repair** | One transaction per task: read the task, its steps and its lease, classify against the recovery table, then **at most one** conditional write whose `WHERE` clause carries every precondition the read observed |

Cancellation deliberately touches **no step rows**. Task Protocol §9 `T9`: already
succeeded steps keep their receipts and Serea never undoes an external effect. An
in-flight step's lease is released separately, by the worker or by recovery.

## 5. Bounds P2 enforces, and the ones it does not

[Crate Map §3.1](../architecture/03-crate-map.md#31-ownership-of-each-protocol-contract)
splits bound ownership: `serea-core` owns configuration and global counters; "the
crate that owns the call" enforces the bound at the call site.

| §2 bound | P2 | Why |
| --- | --- | --- |
| `max_attempts_per_step` | **Enforced** | Materialised on the task as `max_attempts_per_step` per Bounds §2.1, and P2 owns the step attempt. Read from durable state in the `begin_attempt` predicate |
| `max_concurrent_steps_per_task` | **Not enforced** | An earlier draft called this "**Structurally**" on the strength of the engine refusing to lease a second step of a task. That is an application-side convention with no `CHECK`, no trigger and no partial unique index, so it is not structural and it is not claimed. One effecting step at a time (Task Protocol §5 rule 2) is a P2 **engine convention**, with the bound itself owned by `serea-core` |
| `max_concurrent_tasks`, `max_task_wall_clock_ms`, `max_retained_tasks`, `max_replan_revisions_per_task` | **Not enforced** | Counters and configuration belong to `serea-core`. P2 exposes `plan_revision`, `recovery_duration_ms`, and `plan_revision_count()` so the owner of each bound can check it from durable state |
| `max_model_calls_per_task`, `max_tool_calls_per_task` | **Not enforced, columns present** | They are part of the frozen `attempt_budget` shape and must round-trip. The `*_used` counters are P4's and P2 has nothing to increment them with, so P2 does **not** add those columns |
| `max_lease_seconds` | **Not enforced** | The TTL is supplied by the caller from `BoundConfig`. P2 owns the mechanism; `serea-core` owns the number |
| Every §4.4 `BOUND_EXCEEDED_*` event | **Not emitted** | `EventKind` construction is P3's. P2 writes the durable code into `failure_reason` / `blocked_reason` / `error_code`, which is the durable half of Bounds §9 |

`B3` therefore holds: P2 enforces exactly one operational bound, and it is listed.

## 6. Blob store

Full mechanics are in [P2 SQLite schema §5](P2-sqlite-schema.md#5-content-addressed-blobs).
The API-level decisions:

```rust
impl Tx<'_> {
    /// The only blob entry point. Takes **bytes**, never a `Value` (ADR-0019).
    pub fn put_blob(&mut self, bytes: &[u8], class: DataClass) -> Result<BlobRef, StoreError>;
    pub fn get_blob(&mut self, r: &BlobRef) -> Result<Vec<u8>, StoreError>;
    pub fn put_step_blob(&mut self, step_id: StepId, role: StepBlobRole,
                         bytes: &[u8], class: DataClass) -> Result<BlobRef, StoreError>;
    pub fn put_task_blob(&mut self, task_id: TaskId, role: TaskBlobRole,
                         bytes: &[u8], class: DataClass) -> Result<BlobRef, StoreError>;

    /// The **only** path that writes a classified free-text value
    /// (`title`, `result_summary`, `error_message`, `effect_summary`).
    ///
    /// Dispatches exactly like `put_blob`: `PRIVATE` with no configured backend is
    /// refused with `AtRestProtectionUnavailable`, `SECRET` and `CREDENTIAL` with
    /// `ClassRefused`. A SQLite `CHECK` cannot record a per-column class, so this
    /// single function *is* the enforcement for those columns — which is weaker than
    /// the blob store's schema-level cap, and is claimed as weaker. ADR-0022.
    pub fn put_classified_text(&mut self, slot: ClassifiedTextSlot,
                               value: &str, class: DataClass) -> Result<(), StoreError>;
}
```

`put_blob` canonicalises, digests, classifies, and inserts the blob row. It does
**not** create a reference, and it does not commit either: it takes `&mut Tx`, so
the caller's reference insert and the caller's `COMMIT` are the same transaction.
There is no `put_blob` on `Store`, only on `Tx`. That is what makes orphan
prevention structural rather than a sweeper's job — a crash between the two
inserts rolls back both.

`get_blob` re-canonicalises and re-digests, returning `BlobCorrupt` on mismatch.
A digest that is never re-verified detects nothing.

## 7. Migrations and connection policy

### 7.1 Migrations

```rust
pub struct Migration { pub version: u32, pub name: &'static str, pub sql: &'static str }
impl Migrations {
    pub fn embedded() -> &'static [Migration];   // contiguous from 1, ordered
    pub const LATEST: u32;
    pub fn checksum(sql: &str) -> Digest;
}
```

- **Numbering** is contiguous from 1, never reused, never renumbered. P2 ships
  exactly one: `0001_initial`. P3's is `0002`.
- **Each migration runs inside its own `BEGIN IMMEDIATE … COMMIT`**, together with
  the `schema_migrations` row. SQLite's DDL is transactional, so a failed migration
  rolls back completely — the version row and the schema change land or neither
  does. This is the answer to "can a migration failure leave a partially upgraded DB
  accepted as valid": no, because the version marker and the DDL share a
  transaction.
- **Checksums.** Each applied migration's SQL is re-hashed at open and compared.
  A mismatch is `MigrationChecksumMismatch` and the store does not open. This
  catches a binary whose migration text differs from the one that was applied.
- **Newer schema refused.** `MAX(version) > Migrations::LATEST` is
  `SchemaTooNew`. There is no auto-downgrade and no "best effort" open.
- **Adopting a foreign file is refused.** Zero-length file ⇒ fresh, migrate. A
  non-empty file with no `schema_migrations` table but with other tables ⇒
  `NotSereaStore`. The store never adopts an unknown file and never deletes one.
- **Corruption.** `PRAGMA quick_check` at open, because it is cheap and catches
  page-level damage. A full `PRAGMA integrity_check` is behind an explicit
  `verify_integrity()` for an admin path, not on every open. ADR-0005's open item
  about verifying durability settings against the platform is not closed by
  either, and is not claimed to be.

### 7.2 Connection policy

| Setting | Value | Why |
| --- | --- | --- |
| `journal_mode` | `WAL`, asserted at open | ADR-0005. Concurrent readers with a single writer |
| `foreign_keys` | `ON`, asserted at open | The blob reference integrity and the cascade delete depend on it. Asserted because a silently-off pragma turns every `FOREIGN KEY` in the schema into a comment. It is **not** set inside a migration: `PRAGMA foreign_keys` is a no-op inside a transaction, and every migration runs inside `BEGIN IMMEDIATE` |
| `synchronous` | `FULL` | Task Protocol §5 rule 1 — a step's success and its receipt are committed before the task advances — is the whole point of this phase, and in WAL mode `NORMAL` can lose the last commits on **power** loss (not process crash). An fsync per commit is milliseconds on an SSD |
| `busy_timeout` | 5000 ms | Single writer, low contention, and a bounded wait rather than an immediate `SQLITE_BUSY` |
| `wal_autocheckpoint` | SQLite default | Do not tune what was not measured |
| `wal_checkpoint` | `TRUNCATE` on clean close | Bounds WAL growth across restarts |
| `temp_store` | **default**, deliberately not `MEMORY` | A temp table spills to a file that is *not* at-rest protected. ADR-0022's protection covers `blobs.content`, not SQLite's scratch space. Any future change here must re-open that question |
| `application_id` / `user_version` | **not set** | `schema_migrations` is the single authority |
| Connection count | **1**, behind a `Mutex` | SQLite is single-writer. The mutex guards the *connection*, never lease semantics and never a transition |

### 7.3 Test database policy

- **`Store::open_in_memory`** for pure unit and property tests. Note it **cannot be
  reopened** and has **no** crash durability, so it is never used for a
  reopen-equality or crash test.
- **A file-backed store under a RAII `TempStore`** for every reopen, cascade,
  migration-reopen and crash test. Its path comes from `std::env::temp_dir()`
  joined with a **counter-derived** unique name — never a wall clock and never an
  RNG, so `.clippy.toml`'s ban on `SystemTime::now` / `Instant::now` is satisfied
  and parallel tests cannot collide.
- Every crash test runs in a **child process** re-invoking the test binary with
  `current_exe()`, which is the only way to test durability rather than the
  in-process rollback path.

### 7.4 `rusqlite`, and the alternatives

**Recommendation: `rusqlite`, with the `bundled` feature, synchronous, one
connection.**

Requirements the dependency must satisfy — stated as capabilities rather than
feature strings, because no dependency may be resolved in this run:

| Requirement | Needed for |
| --- | --- |
| SQLite ≥ 3.37.0 | `STRICT` tables; the generated class labels |
| JSON1 present | `json_valid` / `json_type` in `tasks.extensions` and `error_details` |
| `pragma_update` / per-connection `foreign_keys` | §7.2, which is per-connection state |
| `INSERT … ON CONFLICT … DO UPDATE … WHERE` | ADR-0024's atomic `acquire_lease` |
| Statement-level `rows_affected` | The explicit zero-row fence check |

**`bundled` versus system SQLite.** `bundled` compiles SQLite from source, so the
version is whatever the crate pins and every developer and CI machine gets the
same one. A system `libsqlite3` on macOS can be years behind — and a
`STRICT`-table migration that works on a laptop and fails on a CI runner is the
worst possible failure mode. The cost is a `build.rs`, a C toolchain in CI, and
slower builds. `ubuntu-latest` has a toolchain. The trade is worth it.

**Fallback, stated so it is not a surprise.** If the resolved bundled SQLite is
below 3.37.0, `STRICT` and `GENERATED … STORED` are dropped and each column gains
`CHECK (typeof(col) = 'text')` or the integer equivalent. The generated class
columns become ordinary columns with a `CHECK` that rank and label agree, which is
weaker because the agreement is then checked rather than structurally guaranteed.
This fallback is a **decision for P2C at dependency-add time**, not a silent
downgrade.

**Why not `sqlx`.** It requires an async runtime, and P1 established that the
workspace has none and that a `Clock` port would be "a fourth port with no P1
consumer". Adding Tokio to persist a handful of rows in a single-writer embedded
database buys nothing: P2's transactions are milliseconds long, they never span an
`.await`, and `sqlx` also brings a connection pool and either build-time database
verification (impossible for an embedded host) or runtime compilation
(`SQLX_OFFLINE`). It is also a materially larger dependency surface in a workspace
whose whole discipline so far has been minimalism.

**Why not `libsqlite3-sys` directly.** Hand-written FFI, no statement builder, no
error taxonomy. Every safety-adjacent line would be ours.

**Why not the `rusqlite` async or `bundled-full` variants.** They add a runtime.

## 8. Clock and time representation

Durable time is `INTEGER` epoch **milliseconds**, column-named `*_at_ms`.
Authoritative comparisons — lease expiry, the task deadline, any elapsed-time
arithmetic — are integer comparisons, never `TEXT`.

The wire `Timestamp` is produced only at the storage boundary, in both directions,
by functions in `serea-protocol` next to the type that owns the grammar. The
existing `TimestampMs` is a validated 48-bit millisecond value, and `2^48-1`
milliseconds is year 10889, so one type covers the entire wire range.

`Timestamp` derives `Ord`, and a lexicographic comparison is **wrong** across the
two permitted wire forms: `…T09:14:22Z` sorts *after* `…T09:14:22.100Z` because
`Z` (0x5A) beats `.` (0x2E). P2 never compares wire forms.

### 8.1 `Clock` enters `serea-protocol` in P2

[Crate Map §3](../architecture/03-crate-map.md#3-crate-inventory) already freezes
`Clock` as a `serea-protocol` item. P1 recorded that a `Clock` trait there "would
be a fourth port with no P1 consumer". **P2 is the first consumer**, so this fills
a declared slot and needs no ADR.

```rust
pub trait Clock {
    /// The current instant as validated epoch milliseconds.
    fn now_ms(&self) -> Result<TimestampMs, ProtocolError>;
}
```

Returning the validated `TimestampMs` rather than a bare `u64` reuses an existing
type invariant instead of creating a parallel one, and bounds every clock reading
to the same 48-bit range the identifier minting already uses.

`serea-testkit::TestClock` implements it. That requires **one structural change**
to `TestClock`: its authoritative state becomes a single `now_ms: u64`, and the
six calendar fields are derived on demand for `format()`. Today the authoritative
state is **seven** fields — six calendar integers plus an `elapsed_ms` counter —
so a `now_ms()` accessor would have to invert calendar arithmetic that could drift
from `Timestamp::new`'s validation. One authority removes the inversion, and
`elapsed_ms` becomes `now_ms - start_ms`.

`TestClock::at` and `advance` keep returning typed errors rather than panicking,
as P1 established.

### 8.2 No ambient clock

`.clippy.toml` bans `SystemTime::now` and `Instant::now` workspace-wide, so a
production wall clock in `serea-storage` is a clippy error under
`cargo clippy --all-targets` rather than a review comment. That ban does **not**
fire in a doctest, a `build.rs`, or a plain `cargo test` run, which is why tests E7
and O3 add a source-level assertion rather than relying on the lint alone.
`serea-storage` takes `&dyn Clock` at construction and reads it nowhere else.
`Store::open` validates that the supplied clock's `now_ms` is in range once, so a
misconfigured clock is refused at open rather than at first commit.

## 9. Recovery

`TaskEngine::recover()` produces durable classification and committed repairs. It
invokes nothing and re-effects nothing.

### 9.1 The classification table

One exhaustive table, the recovery analogue of the transition table in §10.2. A
`(state, step_status, lease, receipt)` combination not in the table is
`CorruptOrInvariantViolation`, never a default arm.

| # | Condition | Decision | P2 mutation |
| --- | --- | --- | --- |
| 1 | `state.is_terminal()` | `TerminalNoop` | None. **No journal row** |
| 2 | A row violates a `CHECK`, a foreign key`, or `json_valid`, and the damage is attributable to one task | `CorruptOrInvariantViolation`, then `BlockedTask` | Move the task `BLOCKED` with `blocked_reason: UNRECOGNISED_STATE` and continue the pass |
| 3 | An unrecognised `status` or `state` string | `CorruptOrInvariantViolation`, then `BlockedTask` | As #2 — Protocol Index §4.2 rule 5 |
| 3b | Corruption not attributable to one task: a corrupt `schema_migrations` row, or a `foreign_key_check` failure spanning tables | `RefusedPass` | **No mutation at all.** The pass returns `Err`, because there is no task to attribute the damage to and blocking every task would be a worse lie |
| 4 | A held lease with `expires_at_ms <= now_ms` | `ExpiredLease` | `release_lease` with the stored generation, plus a journal row. Then classify the step under #5 or #6 |
| 5 | The step was `EXECUTING`, its lease is gone, and a receipt row exists | `ReceiptAlreadyCommitted` | Commit the task transition from durable facts. **No re-effect** (Task Protocol §6, `T4`) |
| 6 | The step was `EXECUTING`, its lease is gone, and no receipt exists | `NeedsReconciliation` | Journal only. **No re-execution, ever, in P2** |
| 7 | A `WAITING` step of kind `WAIT_APPROVAL` | `AwaitApproval` | Journal only. P2 cannot re-render against a device roster (P6/P12) |
| 8 | A `WAITING` step of kind `WAIT_USER` or `WAIT_SCHEDULE` | `AwaitUser` | Journal only. P2 cannot deliver input |
| 9 | `EXECUTING`, no lease, current step `LEASED` or `PLANNED` | `ResumeNormally` | Journal only. No state change |
| 10 | `EXECUTING`, the current step is `SUCCEEDED`, all steps terminal | `ResumeNormally` | Advance the task per the transition table |
| 11 | Any other non-terminal combination | `ResumeNormally` or `CorruptOrInvariantViolation` | Journal only |

**`ExpiredLease` never becomes blind re-execution.** Task Protocol §6 permits
re-issuing an expired-lease step with the same key when `replay_safety` is
`IDEMPOTENT` and no receipt exists. P2 cannot read `replay_safety` — it lives in a
descriptor and there is no registry — so P2 records the decision durably and P5
acts on it. The decision is durable, so deferring the action loses nothing, and
this is the difference between deferring and guessing.

### 9.2 Idempotency of the pass (`T5`)

Every classification is a read; every mutation is a single conditional statement
whose `WHERE` carries **every precondition the read observed** — task state, lease
generation, lease owner, step status, and receipt presence. A second pass over
unchanged durable state therefore matches nothing and writes nothing.

No "recovery already ran" marker exists, and none is added: a marker would be a
second source of truth for a property that is structurally true, and ADR-0021's
`pending_event_transitions` count gives the operator the visibility a marker was
wanted for.

### 9.3 What recovery establishes, and what it does not claim

| Established | Not claimed |
| --- | --- |
| Restart persistence (`T1`, `T2`, `T3`) | Any provider execution |
| Idempotent recovery analysis (`T5`) | Read-back reconciliation (P5) |
| Stale-lease detection | Approval re-render against a device roster (P6/P12) |
| The structural absence of a duplicated external effect | Any model turn (P4) |
| `ReceiptAlreadyCommitted` repair without re-effect (`T4`) | `E3` or `E4` (P3) |
| The `C4` half that is expressible without a registry | The `side_effect_class != NONE` half |

```rust
pub struct RecoveryReport {
    pub tasks_examined: u64,
    pub tasks_resumed: u64,
    pub repairs_committed: u64,
    pub invariant_violations: u64,
    pub pending_event_transitions: u64,   // ADR-0021
    pub decisions: Vec<RecoveryDecision>,
}

/// One row of the §9.1 table, with the mutation decided for it. `blocked_task` and
/// `refused_pass` are distinct outcomes rather than one "or refuse the pass"
/// branch, so the report says which happened instead of leaving a caller to guess.
pub enum RecoveryDecision {
    ResumeNormally      { task_id: TaskId, next_step_id: Option<StepId> },
    AwaitUser           { task_id: TaskId },
    AwaitApproval       { task_id: TaskId, step_id: StepId },
    ExpiredLease        { task_id: TaskId, step_id: StepId, owner: LeaseOwner },
    NeedsReconciliation { task_id: TaskId, step_id: StepId, receipt_present: bool },
    ReceiptAlreadyCommitted { task_id: TaskId, step_id: StepId, receipt_id: ReceiptId },
    TerminalNoop        { task_id: TaskId, state: TaskState },
    CorruptOrInvariantViolation { task_id: Option<TaskId>, reason: ReasonCode },
    /// The task was moved to `BLOCKED` and the pass continued.
    BlockedTask         { task_id: TaskId, reason: BlockedReason },
    /// The whole pass was refused, because the corruption was not attributable to
    /// one task — a corrupt `schema_migrations` row, or a `foreign_key_check`
    /// failure spanning tables.
    RefusedPass         { reason: ReasonCode },
}
```

## 10. `serea-task-engine`

### 10.1 Public surface

```rust
pub struct TaskEngine { store: Store, journal: TaskJournal }

impl TaskEngine {
    pub fn create_task(&mut self, spec: NewTask) -> Result<TaskRecord, EngineError>;
    pub fn persist_plan(&mut self, task_id: TaskId, plan: Plan) -> Result<PlanRevision, EngineError>;
    pub fn acquire(&mut self, task_id: TaskId, step_id: StepId,
                   owner: LeaseOwner, ttl_ms: u64) -> Result<LeaseGuard, EngineError>;
    pub fn begin_attempt(&mut self, guard: LeaseGuard) -> Result<StepRecord, EngineError>;
    pub fn commit_step(&mut self, guard: LeaseGuard,
                       outcome: StepOutcome) -> Result<StepRecord, EngineError>;
    pub fn close_reconciled_absent(&mut self, guard: LeaseGuard,
                                   reason: ReasonCode) -> Result<StepRecord, EngineError>;
    pub fn release(&mut self, guard: LeaseGuard) -> Result<(), EngineError>;
    pub fn block(&mut self, task_id: TaskId, reason: BlockedReason) -> Result<TaskRecord, EngineError>;
    pub fn cancel(&mut self, task_id: TaskId, by: TaskOriginKind) -> Result<CancellationOutcome, EngineError>;
    pub fn delete_task(&mut self, task_id: TaskId) -> Result<DeletionOutcome, EngineError>;
    pub fn load(&self, task_id: TaskId) -> Result<TaskRecord, EngineError>;
    pub fn recover(&mut self) -> Result<RecoveryReport, EngineError>;
}
```

Every Crate Map §3 name for `serea-task-engine` — `TaskEngine`, `TaskRecord`,
`StepRecord`, `Plan`, `PlanRevision`, `RecoveryReport`, `CancellationOutcome` — is
kept. `StepOutcome` and `DeletionOutcome` are added, and both are justified rather
than named for symmetry:

- `StepOutcome` is the **seam that keeps P2 from growing**. `commit_step` takes an
  outcome something else produced; the engine has no path to a provider, so there
  is no code in `serea-task-engine` that *could* grow one.
- `DeletionOutcome` carries the counts of what was deleted, which Task Protocol
  §8 requires a deletion to report and which ADR-0021's journal hook records. **Not**
  Data Classification §8.2 step 4: that step is `DELETION_CASCADE_COMPLETED`, an
  `EventKind` owned by `serea-event-bus` in P3, and §8.2 is the *memory* cascade,
  which is `serea-memory`'s in P7.

**Not defined in P2, and why:** `ExecutionHandle`, `StepRunner`, `AttemptPlan`,
`RetryPolicy`, `StepExecutor`. Every one of those implies something that *executes*,
and P2 does not execute.

### 10.2 The transition table is one exhaustive function

```rust
fn legal_task_transition(from: TaskState, to: TaskState) -> bool {
    use TaskState::*;
    matches!(
        (from, to),
        (Received, Planning) | (Received, Cancelled) | (Received, Failed)
        | (Planning, Ready) | (Planning, WaitingUser) | (Planning, WaitingApproval)
        | (Planning, Blocked) | (Planning, Cancelled) | (Planning, Failed)
        | (Ready, Executing) | (Ready, Planning) | (Ready, WaitingApproval)
        | (Ready, Cancelled) | (Ready, Failed)
        | (Executing, Verifying) | (Executing, WaitingApproval) | (Executing, WaitingUser)
        | (Executing, Blocked) | (Executing, Ready) | (Executing, Cancelled)
        | (Executing, Failed)
        | (WaitingApproval, Ready) | (WaitingApproval, Cancelled) | (WaitingApproval, Failed)
        | (WaitingUser, Ready) | (WaitingUser, Planning) | (WaitingUser, Cancelled)
        | (WaitingUser, Failed)
        | (Verifying, Completed) | (Verifying, Executing) | (Verifying, Blocked)
        | (Verifying, Cancelled) | (Verifying, Failed)
        | (Blocked, Ready) | (Blocked, Planning) | (Blocked, Cancelled) | (Blocked, Failed)
    )
}
```

Two properties, both deliberate:

1. **`COMPLETED`, `FAILED` and `CANCELLED` have no arm.** `T8` is therefore a
   property of the *absence* of an arm rather than of a runtime check. Adding a new
   `TaskState` variant is a compile error here, because `use TaskState::*` inside a
   `matches!` over a tuple of two enums forces exhaustiveness only if every variant
   appears — which is why the test enumerates all 121 pairs and compares them to a
   literal transcription of Task Protocol §4.2.
2. **The reason a transition is legal is not scattered string literals.** A second
   function, `task_transition_reason(from, to) -> TransitionReason`, returns which
   frozen rule authorises it, so `failure_reason`, `blocked_reason` and the
   journal's `reason_code` all come from one place. Bounds §9 requires a typed
   refusal naming the bound, and a scattered `&str` is how that gets lost.

An illegal transition is `EngineError::IllegalTaskTransition`, and per Task
Protocol §4.2 the host then fails the task to `FAILED` with an
invariant-violation reason — **without persisting the illegal state**, which is
the whole point of checking before the write.

### 10.3 `policy_class` immutability

- No engine method writes `policy_class`. It is a field of `NewTask` and nothing
  else.
- The `tasks_policy_class_immutable` trigger refuses any `UPDATE` regardless of the
  writer. A Rust check can be bypassed by a future writer; a trigger cannot.
- The plan path checks `risk_class ≤ policy_class` only when a descriptor is
  available. **In P2 it never is**, so P2 records the obligation and P5 performs
  the comparison. Stated as a non-claim, not as a partial enforcement.

### 10.4 Attempt accounting

`attempt` is incremented in **exactly one place**: `acquire_lease`. `begin_attempt`
moves `LEASED → EXECUTING` and stamps `started_at_ms`, and touches nothing else.

That is a correction. An earlier draft incremented at *both* points, so a
`max_attempts_per_step` of 3 bought a single attempt — and Task Protocol §3.1 says
the field exists precisely to "distinguish the crash-recovered attempt from a
deliberate retry", which double-charging makes indistinguishable.

The ceiling is checked at acquisition, in two statements inside one transaction, so
each has exactly one possible cause:

1. The fenced `UPDATE … WHERE lease_generation = :expected`. Zero rows ⇒
   `LeaseFenced` — someone else holds it, or it moved on.
2. A ceiling check reading `max_attempts_per_step` from `tasks` **in the same
   transaction**. At the ceiling ⇒ `AttemptCeilingReached`, with the step left
   `PLANNED` and the lease released, so a deliberate retry is possible.

Reading the bound from durable state at the check is
[Bounds Protocol §2.1](../protocols/10-bounds-protocol.md#21-where-these-live-in-durable-state)
and is why the two are separate statements: a single combined `WHERE` would make
"fenced" and "at the ceiling" indistinguishable, and a caller that cannot tell them
apart cannot report them.

### 10.5 Plan revision handling

`persist_plan` with `revision > tasks.plan_revision`:

- Every step's sequence must be **new** or **unchanged**. `UNIQUE (task_id,
  sequence)` makes a rename a constraint violation rather than a silent renumber.
- A revision may **append** at higher sequences and may **delete** steps that are
  still `PLANNED`. ADR-0018 §5 explains why renumbering and mid-plan insertion are
  refused: each changes the meaning of Task Protocol §3.2's prerequisite rule for a
  step that may have run.
- A revision that would drop a step ever leased or executed is
  `EngineError::PlanRevisionWouldDropExecutedStep`.
- The superseded `PLANNED` steps' rows are deleted; their `ARGUMENTS` references
  cascade, and their now-unreferenced blobs are swept in the same transaction. The
  prior plan's content survives as the `plan_revisions` blob, which is what makes
  the deletion auditable.

### 10.6 Cancellation

```rust
pub struct CancellationOutcome {
    pub task_id: TaskId,
    /// The state after the call. Unchanged when `changed` is false.
    pub state: TaskState,
    /// `None` when `changed` is false — a terminal task was not cancelled, so
    /// there is no cancellation instant to report. `tasks.cancelled_at_ms` is
    /// nullable for the same reason.
    pub cancelled_at: Option<Timestamp>,
    pub changed: bool,
    pub already_terminal: bool,
}
```

Cancelling a terminal task returns `changed: false, already_terminal: true` and
commits **nothing** — no state change, no journal row. That is an idempotent
no-op with an explicit outcome, not an error, because a retrying device would
otherwise see a hard failure for a task that already succeeded, and
[Protocol Index §4.2 rule 5](../protocols/00-protocol-index.md#42-compatibility-rules)
is explicit that an unrecognised situation must not crash or silently skip.

Cancellation touches no step row (`T9`) and does not implicitly undo anything.

### 10.7 Parent binding and replay semantics

- Every task-scoped statement carries `task_id` in its `WHERE` clause, and
  `task_steps.task_id` is a `FOREIGN KEY`. A `StepId` from another task cannot be
  mutated by accident, and `LeaseGuard` carries `task_id` as well, so the fence
  predicate is `(step_id, task_id, generation, owner)`.
- `UNIQUE (task_id, idempotency_key)` means a second capability step in one task
  with the same key is refused. With ADR-0019's framing this is correct behaviour
  rather than a coincidence.
- Replay semantics are exactly the frozen ones: a crash replay re-issues the **same**
  `StepId` and the same key; a deliberate second execution requires a new `StepId`
  and therefore a new key, which ADR-0018 §4's matrix makes a fresh step row.

## 11. Security posture, by question

The twelve questions the security review must answer, with the design's answer
attached. The review's independent verdict is in §13.

| # | Question | Design answer |
| --- | --- | --- |
| 1 | Can stale worker A commit after B owns a reclaimed lease? | No. `lease_generation` increments on every acquisition and every commit carries it in its `WHERE`; zero rows is `LeaseFenced`. ADR-0024 |
| 2 | Can a corrupt row widen authority? | Class ranks are integers with generated labels, so rank and label cannot disagree; `policy_class` has an `UPDATE` trigger; `state`, `kind` and every code-shaped field have `CHECK`s. A corrupt row is *refused*, and recovery's row #2 detects it |
| 3 | Can a `PRIVATE` blob hit disk unencrypted? | No, by any writer that leaves constraint checking enabled. `CHECK ((data_class_rank = 2) = (protection = 'AT_REST'))` makes the row unconstructible, and the write path refuses before the insert when no backend is configured. ADR-0022, [schema §7](P2-sqlite-schema.md#7-verified-behaviour) |
| 4 | Can `SECRET` or `CREDENTIAL` enter ordinary SQLite? | No, by any writer that leaves constraint checking enabled: `data_class_rank BETWEEN 0 AND 2` on all seven classified tables makes those rows unconstructible, verified on each. **The boundary, stated once:** `PRAGMA ignore_check_constraints = ON` disables every `CHECK` in the schema for a local file writer, and P2 does not mitigate that. Every trigger and foreign key still holds under it — which is where this design spends its structural budget. ADR-0022, [schema §7](P2-sqlite-schema.md#7-verified-behaviour), tests O14/O15 |
| 5 | Can recovery turn ambiguity into a second effect? | No. P2 recovery never executes. `NeedsReconciliation` records the decision durably for P5. Task Protocol §6.2 |
| 6 | Can a state transition occur without the `E3` seam? | Every transition writes a `task_journal` row through the same `Tx`. `E3` itself is **not claimed** — ADR-0021 |
| 7 | Can a migration failure leave a partial upgrade accepted? | No. The DDL and the `schema_migrations` row share one transaction, and checksums are re-verified at every open. §7.1 |
| 8 | Can one task mutate another's step by ID confusion? | No. `task_id` is in every task-scoped predicate, is a `FOREIGN KEY`, and is carried on `LeaseGuard` |
| 9 | Can an old process write after a new one superseded it? | No, for any write that requires a lease. Of the three that do not: `persist_plan` (`AND state = 'PLANNING'`) and `cancel` (`state NOT IN (terminal)`) carry a full expected-state predicate, so a stale writer wins a race it was always allowed to win or affects zero rows. `insert_task` and `delete_task` carry none, and that is sound rather than sloppy: a `TaskId` is never reused ([Protocol Index §2](../protocols/00-protocol-index.md#2-identifier-grammar) rule 3), so a duplicate `INSERT` is a retry of the same creation and a `DELETE` is idempotent. What a stale process **cannot** do is write step or receipt state |
| 10 | Can extension JSON override a normalised column? | No. `extensions` is a single forward-compatible column with a `json_valid` object `CHECK`, and no code path reads it for a decision. ADR-0021 records that it is retained, not interpreted |
| 11 | Can inconsistent rows fabricate terminal success? | No. `state` has an 11-value `CHECK`; `COMPLETED` is only reachable through the transition table; `SUCCEEDED` requires `result_digest`; a receipt is reachable only through `commit_step_succeeded`; and recovery's invariant scan refuses a receipt on a non-`SUCCEEDED` step |
| 12 | Does any error or log path quote `PRIVATE`/`SECRET` bytes? | No. `StoreError` renders machine-readable causes only, and carries no lease identity beyond the step's own id and generation counter — neither is replayable. ADR-0019's canonicalization returns no payload on error, and `errors.rs` needs no change. The residual, disclosed: the prose fields (`ErrorMessage`, `DescriptorDescription`) are `PRIVATE` for log egress under ADR-0023, and the four classified `TEXT` columns are enforced by the write chokepoint rather than by the schema |

## 12. Resource bounds — still open

**P2 enforces none of these, and tomorrow's P2 must not claim this gap is closed.**

P0's open gap — payload bytes, attachment size, object counts — is unchanged by
this phase. [P1 closure](P1-closure.md) records that the corrective pass "removed
a string-length ceiling; it did not close the broader resource-bound gap and does
not claim to".

A separate decision must settle the following. It is **not** raised as an ADR
here, because an ADR is the artefact that records a *ratified* decision, and
nothing here is ratified: inventing a number with no measurement behind it is the
`MAX_VALUE_LENGTH` mistake P1 already retracted.

| Bound a future decision must ratify | Question it must answer | Evidence needed |
| --- | --- | --- |
| `max_serialized_payload_bytes` | Per document, per boundary or per blob? | Measured p99 of real provider outputs and model structured outputs |
| `max_content_blob_bytes` | Per blob, and does a total-per-task limit also apply? | Measured p99 and max of real argument and result documents |
| `max_attachment_bytes` | Per attachment, per message, per task? | P9 Gmail; no data exists yet |
| `max_object_properties` / `max_array_items` | The **operational** ceiling, distinct from a schema's own `maxItems` | Where a schema would declare a limit above the operational one |
| `max_task_plan_steps` | Distinct from `assistant-task.schema.json`'s structural `maxItems: 1024` | Measured distribution of real plan lengths |
| Decompression / expansion limits | Only if compression is introduced; P2 compresses nothing | A compression decision |
| `max_db_file_bytes`, WAL growth | Retention is already in §2 as `max_retained_tasks` | A measurement after P2 has run in anger |

**Distinguishing this from what already exists.** The following are *structural*
and stay where they are, under ADR-0020:

| Existing constraint | Owner | Status |
| --- | --- | --- |
| `assistant-task.schema.json` `steps` `maxItems: 1024` | Task Protocol's schema | Structural. Unchanged |
| `actionError.details` `maxProperties: 64` | Capability Protocol's schema | Structural. Unchanged |
| `MAX_INSTANCE_DEPTH = 64` | `serea-protocol` | Structural, named as such by ADR-0020. Unchanged |
| `digest` `maxLength: 71`, `task_id` `maxLength: 30` | Identifier grammar | Structural. Unchanged |
| `blobs.size_bytes = length(content)` | This schema | A **consistency** invariant, not a bound. It rejects a torn length without inventing a ceiling |

## 13. Review outcome and dispositions

Three read-only passes, findings frozen before any document changed. **The DDL was
executed between passes**, which is where the substantive findings came from.

| Pass | Verdict | Findings | Accepted | Rejected as wrong |
| --- | --- | --- | --- | --- |
| **A — code review** | Changes requested, then clean after disposition | 55 | 45 | 10 |
| **B — security review** | Not ratifiable, then clean after disposition | 17 | 16 | 1 |
| **C — bounded final re-review** | Clean | 3 | 3 | 0 |

### 13.1 What the two review passes found, and where it landed

| Finding | Class | Landed in |
| --- | --- | --- |
| **The whole migration did not build** — SQLite prohibits subqueries in `CHECK` | A1, blocker | Replaced with `side_effect_receipts_key_matches_step` and `side_effect_receipts_step_must_succeed` triggers |
| **`PLANNED` unconstructible** — two biconditionals contradicted | A2, blocker | One-directional implications; [schema §4.4](P2-sqlite-schema.md#44-task_steps) |
| **ADR-0024's commit statement referenced a column that does not exist** and violated the lease biconditional | A4, blocker | Statement corrected: no `receipt_id`, lease columns cleared |
| **ADR-0018's "a `CHECK` for every matrix row" was false** — `lease_generation` unenforced above `PLANNED` | A5, blocker | Added `CHECK (status = 'PLANNED' OR lease_generation >= 1)` and `task_steps_idempotency_key_immutable` |
| **`SECRET`/`CREDENTIAL` accepted on `tasks`** | A6, blocker | Rank capped at 0–2 on **every** classified table |
| **"A P2 deployment holds no `PRIVATE` durable data" rested on a false premise** — `AssistantTask.data_class` is host-assigned | A7, major | Restated on the dispatch, not on "nothing can produce `PRIVATE`"; classified `TEXT` columns recorded as the disclosed weaker guarantee |
| **`FAILED` required `error_details`**, which the frozen schema makes optional | A8, blocker | Constraint removed; a wire-valid error without `details` is accepted |
| **P2 makes four workspace members, not three** | A9, blocker | Corrected in the design and test matrix |
| **`CANCELLED` was unconstructible** | B1, blocker | Second `CHECK` replaced with `state = 'CANCELLED' OR (both null)` |
| **No lease could be acquired, in either order** | B2, blocker | `leases` upsert then a *derived* step update; the consistency trigger removed |
| **`SECRET`/`CREDENTIAL` reachable through `task_journal`** | B3, blocker | Capped at 0–2, like every other classified table |
| **`PRAGMA ignore_check_constraints` defeats every `CHECK`** | B4, major | **Claim narrowed, not defended.** ADR-0022 now names the pragma; tests O14/O15 pin the boundary and the controls that survive |
| **A `BLOCKED` task could not resume or be cancelled** | B5, major | `blocked_reason` check made one-way |
| **A step that began an attempt could never be leased again** | B6, major | Acquire accepts `LEASED` and `EXECUTING` |
| **`attempt` was double-charged**, so `max_attempts_per_step = 3` bought one | B8, major | Incremented at acquisition only |
| **`side_effect_receipts.task_id` could disagree with its step** | B9, minor | `side_effect_receipts_task_matches_step` trigger |
| **`task_journal.step_id` was bound to nothing** | B10, minor | `task_journal_step_task_matches` trigger |
| **`TASK_DELETED` cannot survive its own cascade** | B11, minor | Removed from the `journal_kind` vocabulary; deletion is reported by `DeletionOutcome`'s counts |
| **Nothing at open verified the schema's shape** | B12, minor | Test F19 |
| **`lease.token` survived as a dead reference in six places, including an unfailable test** | A17/B14, minor | Token removed everywhere; H16 rewritten so it can fail |
| **`attempt` was missing from `task_journal`** | B15, minor | Column added |
| **`PRAGMA foreign_keys` inside the migration is a no-op** | B16, minor | Line removed; the connection policy owns it |
| **ADR-0023 described a `freeText` split that does not exist** | A15, major | Corrected against the actual three files, including the inline patterns |
| **A receipt constraint encoded a rule Capability Protocol §5.1 does not state** | A16, major | Constraint deleted; the real condition deferred to P5 |
| **`max_concurrent_steps_per_task` was called "structural" with nothing behind it** | A18, major | Moved to "not enforced" |
| **`task_journal` DDL was absent from the document claiming to be complete** | A14, major | §4.9 added |
| **ADR-0021's back-fill claim was false** — the column list lacked every payload field | A13/B, major | `payload_json` added; the claim corrected |
| **The category-O schema pattern was missing** | A15, major | Pattern supplied, with a generated-prefix fallback stated |
| **A digest `GLOB` rejected every valid digest** — `sha256:` is not hex | A26, minor | `substr(…, 8) NOT GLOB`; the same form on all four digest columns |
| **`tasks_policy_class_immutable` aborted on a no-op update** | A28, minor | `WHEN NEW.policy_class_rank IS NOT OLD.policy_class_rank` added |

### 13.2 The one finding rejected as wrong

A security-review finding claimed **ADR-0019's seven IDK-1 vectors do not reproduce
from the documented preimage**, and that the accompanying "independently
recomputed … and match" sentence was therefore false.

Recomputation says otherwise. All seven match byte for byte, verified field by
field:

| Component | Bytes |
| --- | --- |
| Domain `serea.idempotency.v1\0` | 21 |
| Field count `u8(5)` | 1 |
| `lp("task_id") ‖ lp("tsk_01JQ8Z9K3M7QWXR4V2T6YH0BNA")` | 53 |
| `lp("step_id") ‖ lp("stp_01JQ8Z9M3R2CVN8H5FWK7PQDSF")` | 53 |
| `lp("capability_id") ‖ lp("calendar.events.list")` | 49 |
| `lp("capability_version") ‖ lp("1.2.0")` | 39 |
| `lp("arguments_canonical") ‖ lp('{"limit":25,"range":"tomorrow"}')` | 66 |
| **Total** | **282** |

`sha256` of those 282 bytes is
`idk_f8d17a2f6fb40db5a3421e035cde37a3234e628381cbb56791c14195f456b1cb`, which is the
published vector 1.

The finding is rejected, but the **methodological** half of it is adopted, because
it identified a real fragility: the preimage was specified in prose, and two
attempts to reconstruct it from that prose — mine and the reviewer's — produced
different answers. A specification that two careful readers implement differently
is under-specified even when one of them gets lucky.

So ADR-0019 now publishes the byte layout above, not just the framing rule, and test
D9 reconstructs the preimage from the layout rather than from prose. The lesson is
recorded rather than the accusation: **on this package, a constant that is asserted
is not a constant that is verified.**

### 13.2b What the third pass found, and what it says about §13.1

The third pass found **2 blockers and 6 majors**, and six of them shared one cause:
**a disposition was recorded as landed, and the edit reached one or two of the five
documents that mention it.**

| Disposition claimed in §13.1 | Reality found by pass C |
| --- | --- |
| `TASK_DELETED` removed | Still in the DDL. Prose was fixed, the `CHECK` was not |
| The lease `token` removed "everywhere" | Survives in nine places across four documents, including a test whose asserted subject no longer exists |
| A7's `PRIVATE` premise "restated" | Restated in ADR-0022; the original wording survived verbatim in the gap analysis, which is the document read first |
| A13's back-fill claim "corrected" | Corrected in ADR-0021 and the schema; the false form survived in the gap analysis |
| A5's `lease_generation` "verified fixed" | The `0` case was fixed; the sentence also claimed `99` was, and it is not |
| B5's `blocked_reason` fixed | The `CHECK` was fixed; the design's own `cancel` statement then aborted on a `BLOCKED` task |

That is not six coincidences. **§13.1 was an inventory of intent, not of state**, and
a reader trusting it would believe six things that were false. The record now names
this explicitly, and the fourth execution round exists because of it.

The two blockers were both the §13.3 blind spot a third time:

| Blocker | What was tested | What was not |
| --- | --- | --- |
| The `WAITING` biconditional made the three wait kinds unplannable | `WAITING` on a `CAPABILITY` step (refused — correct) | `PLANNED` on a `WAIT_APPROVAL` step. Never run. So a third of the lifecycle was unreachable for three of eight kinds, and `persist_plan` aborted on any plan containing a wait step |
| `TASK_DELETED` could not survive its own cascade | the prose argument | the DDL `CHECK` |

**The pattern is now named in one place and enforced in the harness:** a
biconditional is only justified where both directions are *independently* required by
the protocol. `status = 'WAITING'` requires a wait kind; a wait kind does **not**
require `WAITING`. Three biconditionals in this schema are genuine two-way rules
(lease presence, the step-kind matrix, the `FAILED` error set); every other
constraint is one-directional. Group P's P15 pins the pair that was wrong.

### 13.3 The process finding, which is the most useful thing either pass produced

Every round of defects had the same shape: **negative cases were tested and positive
cases were not.**

| Symptom | What was never run |
| --- | --- |
| `CANCELLED` unconstructible | The positive case; only "cancelled without a timestamp" was tried |
| No lease could be acquired | The prescribed two-statement order; the upsert was tested against a hand-prepared row |
| `SECRET` reachable through `task_journal` | A count of all classified tables; four were enumerated |
| `PLANNED` unconstructible | Any lifecycle status at all |

Rounds 3 and 4 of the harness exist because of it, and they add what was missing:
**construct all 37 legal transitions**, **construct every step-kind × status cell**,
**execute every prescribed statement sequence in the order §4 prescribes**, and
**confirm the accepting side of every constraint**.
[Schema §7](P2-sqlite-schema.md#7-verified-behaviour) now states, per row, what was
run — so "verified" is a reference rather than an adjective.

**Four rounds, 106 assertions, and the shape of what they found is the transferable
lesson: every round found only missing positives.** Nothing was ever found by testing
a refusal that a passing test would not have caught; every round found a *cell nobody
had constructed*. Round 4 is the sharpest case — running the full **8 step kinds × 7
statuses** grid, rather than the two cells either side of the previous bug, found that
a single biconditional over four columns admits a partially populated tuple: `NOTIFY`
with `capability_id` set and the other three null evaluates `0 = 0` and is accepted.
Four per-field biconditionals replace it, and they are strictly stronger.

An implementation reviewer running this package's harness should assume the same and
construct the cell before believing the constraint. Three of the four rounds' findings
were in the *first* two rows of a matrix that was otherwise complete.

### 13.4 What did not change

Three proposals were rejected on the merits, and none is a defect:

| Proposal | Why rejected |
| --- | --- |
| Create `serea-event-bus` in P2 to satisfy `E3` | Crate Map §4.1's reason for that crate is that it is *separate*. The honest deferral is recorded instead |
| Bound `RecoveryReport.decisions` with a cap | `max_concurrent_tasks` bounds it operationally. A second bound here would be an unratified one under `B3` |
| Recover with an explicit "blob-store garbage collector runs nightly" comment | Orphan prevention is structural — P2 writes blob and reference in one transaction — so a sweeper is a repair tool, not a scheduled job. No claim is made either way |

## 14. Implementation phasing

Nine subphases. The dependency order is `protocol → primitives → storage →
classification → leases → engine → recovery → faults → review`, because each stage
is testable before the next one exists. Two slices are moved from the prompt's
suggestion and the reason is given.

| Subphase | Delivers | Depends on |
| --- | --- | --- |
| **P2A** | The seven protocol corrections and their atomic code changes | — |
| **P2B** | Canonical JSON, digests, idempotency, `Clock` | P2A |
| **P2C** | Migrations, `Store`, `Tx`, connection policy | P2B |
| **P2D** | Blobs, classification, `delete_task` | P2C |
| **P2E** | Leases and fencing | P2C |
| **P2F** | `TaskEngine`, transition table, plan, cancellation | P2D, P2E |
| **P2G** | Recovery and restart | P2F |
| **P2H** | Fault injection and crash windows | P2C |
| **P2I** | Independent review and closure | all |

**Two deviations from the prompt's suggested order, and why.**

- **Leases (P2E) are parallel to blobs (P2D), not after the engine.** A lease is a
  single table and a single atomic statement, and its tests need only `Store`. Making
  it a dependency of the engine means the engine cannot be tested at all until the
  fence is complete, which would hide engine bugs behind fence bugs.
- **Fault injection (P2H) starts at P2C, not after recovery.** Crash windows are a
  property of `transact` and `commit`, so the injection harness and the first crash
  test are written with `Store`. Deferring all of it to P2H would mean discovering
  a durability bug while writing recovery tests, when the mistake is least
  diagnosable.

### 15.1 P2A — protocol corrections and primitives

| | |
| --- | --- |
| **Files** | `crates/serea-protocol/src/types.rs`, `errors.rs`; `crates/serea-protocol/schemas/{assistant-task,action-result,event}.schema.json`; `docs/protocols/{00,01,02,06,09,10}-*.md`; `docs/decisions/README.md` |
| **Tests first** | Every matrix cell of ADR-0018 §3 and §4, in both the Rust and the schema direction; every category case of ADR-0023, on a shared adversarial corpus; the whitespace-only divergence |
| **Surface** | `StepPresence`, the two matrices, `validate_opaque_token` / `validate_single_line_label` / `validate_prose` |
| **Exit criteria** | 121-pair task transition table pinned to a literal transcription of Task Protocol §4.2 — no, that is P2F; here: every `TaskStep` matrix cell, every step-kind cell, every text-category cell, both surfaces agreeing |
| **Forbidden** | Any digest computation, any storage, any new dependency |
| **Blocked on** | Owner ratification of the seven ADRs, and the open question 4 (minor or major) in the gap analysis |

### 15.2 P2B — canonical JSON, digest, idempotency, `Clock`

| | |
| --- | --- |
| **Files** | `Cargo.toml` (one `sha2` dependency); `crates/serea-protocol/src/canonical.rs`, `clock.rs`, `lib.rs`; `crates/serea-testkit/src/clock.rs` |
| **Tests first** | The ten SCJ-1 vectors and the seven IDK-1 vectors as literals; the **A**/**B** collision pair; idempotence; member-ordering invariance; injectivity over a generated corpus; the naive-collision canary |
| **Surface** | `canonicalize`, `digest_of`, `derive_idempotency_key`, `CanonicalJsonError`, `Clock`, `Timestamp::from_epoch_millis` / `to_epoch_millis`, `TestClock`'s epoch-millisecond authority |
| **Exit criteria** | Every vector passes; the collision pair derives different keys; `canonicalize(parse(canonicalize(x))) == canonicalize(x)` for the whole corpus; no wall-clock call anywhere (`.clippy.toml` unchanged) |
| **Forbidden** | Storage. Any storage code cannot be written against a digest function that might still change |
| **Note** | This is where the workspace's first hashing dependency lands. It is named, not added, by this run |

### 15.3 P2C — migrations and `Store`

| | |
| --- | --- |
| **Files** | `crates/serea-storage/Cargo.toml`; `crates/serea-storage/src/{lib,store,migrate,tx,error,queries}.rs`; `crates/serea-storage/migrations/0001_initial.sql`; root `Cargo.toml` (`members`); `tests/workspace_smoke.py` |
| **Tests first** | Fresh migration; re-open no-op; newer-schema refusal; `NotSereaStore`; checksum mismatch; `WAL`, `foreign_keys`, `synchronous`, `busy_timeout` asserted at open; JSON1 presence |
| **Surface** | `Store::open` / `open_in_memory` / `schema_version` / `verify_integrity` / `view` / `transact`, `Migrations`, `StoreError`, `TaskQueries` |
| **Exit criteria** | All the above pass against a **file-backed** store; `PRAGMA foreign_keys` returns 1; a rollback inside `transact` leaves no row; `workspace_smoke.py` still passes, and `cargo metadata` lists **exactly four** members — `serea-protocol`, `serea-storage`, `serea-task-engine`, `serea-testkit` |
| **Forbidden** | Blob logic, lease logic, and any engine type |
| **Open question** | [Schema §8](P2-sqlite-schema.md#8-open-at-implementation-time): the SQLite version and JSON1 verification, at add time. The DDL itself is already executed and verified — see [schema §7](P2-sqlite-schema.md#7-verified-behaviour) |

### 15.4 P2D — blobs and classification

| | |
| --- | --- |
| **Files** | `crates/serea-storage/src/{blob,classify}.rs`; `crates/serea-testkit/src/at_rest.rs` |
| **Tests first** | Content addressing; dedupe within a class; wrong digest rejected; corrupt blob detected on read; rollback leaves no orphan; `PRIVATE` with no backend refused; the two `CHECK`s against a hand-written `INSERT`; `SECRET` and `CREDENTIAL` refused on every path; cross-class laundering refused |
| **Surface** | `put_blob`, `get_blob`, `put_step_blob`, `put_task_blob`, `BlobRef`, `AtRestProtection` |
| **Exit criteria** | All pass; the `PRIVATE` refusal writes **no** row; a same-digest two-class write stores two rows |
| **Forbidden** | Task or step mutation. A blob test must not need a task |

### 15.5 P2E — leases and fencing

| | |
| --- | --- |
| **Files** | `crates/serea-storage/src/lease.rs`; the `leases` table and `lease_generation` in migration `0001_initial.sql` |
| **Tests first** | Acquire; competing acquire refused; expiry; reclaim bumps the generation; renewal; renewal refused after expiry; release; **stale generation cannot commit**; the fenced write is the first statement, so a fenced commit inserts no receipt |
| **Surface** | `LeaseGuard`, `acquire_lease`, `renew_lease`, `release_lease`, `LeaseHeld` / `LeaseFenced` / `LeaseExpired` |
| **Exit criteria** | The stale-worker test passes with two **separate** `Store` instances on the same file, proving no process-local mutex participates |
| **Forbidden** | Any `serea-task-engine` type |

### 15.6 P2F — `TaskEngine`

| | |
| --- | --- |
| **Files** | `crates/serea-task-engine/Cargo.toml`; `src/{lib,engine,transition,plan,outcome,error}.rs` |
| **Tests first** | All 121 legal and illegal transitions; terminal states never transition; `policy_class` immutability by trigger; plan persistence before execution; unstarted/in-flight/terminal step representation; `UNIQUE (task_id, sequence)`; step parent binding; attempt ceiling; cancellation and its no-op; receipt-before-advance; the ADR-0018 presence matrix against the database |
| **Surface** | `TaskEngine`, `TaskRecord`, `StepRecord`, `Plan`, `PlanRevision`, `StepOutcome`, `CancellationOutcome`, `DeletionOutcome`, `task_transition_reason` |
| **Exit criteria** | Every test above; `Store` has no mutating method outside `transact`, asserted by a compile-level check that the test suite exercises no other route |
| **Forbidden** | Any execution path. There is no `ExecutionHandle` in P2, and its absence is the scope boundary |

### 15.7 P2G — recovery

| | |
| --- | --- |
| **Files** | `crates/serea-task-engine/src/{recovery,journal}.rs` |
| **Tests first** | Every row of the recovery table; pass once changes state; **pass twice is a no-op**; expired in-flight lease → `NeedsReconciliation` and **no** re-execution; receipt present with an incomplete transition → repaired without re-effect; a corrupt row → `CorruptOrInvariantViolation`; `pending_event_transitions > 0` after a task creation |
| **Surface** | `recover`, `RecoveryReport`, `RecoveryDecision`, `TaskJournal` |
| **Exit criteria** | All rows; byte-identical durable state after the second pass; no network, no clock other than `TestClock`, no model or provider symbol reachable from the test binary |
| **Forbidden** | Provider calls, model calls, approval delivery |

### 15.8 P2H — fault injection

| | |
| --- | --- |
| **Files** | `crates/serea-storage/tests/crash.rs`, `tests/support/child.rs`; `crates/serea-testkit/src/faults.rs` |
| **Tests first** | Every crash window in [the test matrix §3](P2-test-matrix.md#3-crash-and-fault-injection) |
| **Surface** | A `TxHook` injection point used **only** by tests; child-process helpers |
| **Exit criteria** | Each window reopened in a fresh process and asserted against durable expectations; in particular "crash after commit before the caller observes success" shows the row present **and** recovery reporting `ReceiptAlreadyCommitted` rather than re-effecting |
| **Forbidden** | Adding an injection point to a production code path that is not a no-op when unused. A fault hook that is present but inert in release builds is a hazard |
| **Explicit** | No test may simulate a crash by returning an `Err` before commit and calling it a crash |

### 15.9 P2I — independent review and closure

| | |
| --- | --- |
| **Files** | `docs/plans/P2-closure.md` |
| **Tests first** | None. This is review, not implementation |
| **Exit criteria** | Two independent passes green; the closure record reproduces P1's structure: workspace, files, verification evidence with real command output, the §9.3 non-claim table, and the still-open items |
| **Forbidden** | Claiming `E3`, `E4`, `C4`'s second half, `T6`'s comparison, any §2 bound other than `max_attempts_per_step`, any `PRIVATE` at-rest support, or the resource-bound gap |

## 15. The exact first step for tomorrow

P2A, and nothing else. Specifically: write the failing test for **one cell** of the
ADR-0018 §3 presence matrix — a `PLANNED` step that cannot be constructed today,
because `started_at` is not `Option` — and watch it fail on the shape rather than
on an assertion. Then the rest of P2A in the order the ADRs list their code changes.

P2B through P2I are unreachable until P2A lands, because every one of them
canonicalises, digests, or persists something whose presence and classification
rules P2A fixes. Starting at P2C because it "feels like the real P2" would mean
building a durable store on a wire contract that cannot represent a planned step.

## 16. Cross-references

- Contract gaps and their classes: [P2 contract gap analysis](P2-contract-gap-analysis.md)
- The DDL: [P2 SQLite schema](P2-sqlite-schema.md)
- The tests: [P2 test matrix](P2-test-matrix.md)
- The decisions: [ADR-0018](../decisions/ADR-0018-taskstep-lifecycle-and-field-presence.md),
  [ADR-0019](../decisions/ADR-0019-canonical-json-and-idempotency-preimage.md),
  [ADR-0020](../decisions/ADR-0020-bounds-b3-scope-clarification.md),
  [ADR-0021](../decisions/ADR-0021-p2-p3-event-atomicity-seam.md),
  [ADR-0022](../decisions/ADR-0022-durable-private-data-at-rest.md),
  [ADR-0023](../decisions/ADR-0023-text-field-validation-categories.md),
  [ADR-0024](../decisions/ADR-0024-lease-fencing-and-commit-under-lease.md)
