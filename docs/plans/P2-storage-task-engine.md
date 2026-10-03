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
| `append_journal` | One journal row, called by the above. Invoked as a `TransactionParticipant` with the `DurableTransition` the calling method just performed — see ADR-0021 |

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

### 4.1 The durable-transition participant seam — paper compile

No Rust is written by this run. This is a **paper compile**: pseudo-signatures
sufficient to prove the shape type-checks against the properties ADR-0021 and
ADR-0024 require, and to expose anything that would not.

**What it replaces.** The earlier draft's `CommitHook`:

```rust
// REJECTED — see below
fn append(&mut self, tx: &Transaction<'_>) -> Result<()>;
```

Three defects, all structural:

1. The transition identity is **not a parameter**. It would have to live in the
   hook's own mutable state, so `tx.append(&mut journal)` describes *some*
   transition the hook last remembered, not the one this transaction performed.
2. `&mut self` forces the `Store`'s `transact(&self)` to reach the hook through
   interior mutability — a `Mutex` or a `RefCell` — which contradicts ADR-0024's
   single-mutex claim and puts a lock inside the commit path.
3. An early `return Err(…)` from the `transact` body drops the hook without
   calling it, leaving the hook's remembered transition describing a write that
   was rolled back. The journal would then be **one transition behind reality**,
   silently.

**The replacement.**

```rust
// Immutable identity of the transition being committed. Constructed once, at the
// point the engine decides the transition, and passed by shared reference into
// every participant. Not stored anywhere mutable.
pub struct DurableTransition {
    pub task_id:    TaskId,
    pub step_id:    Option<StepId>,
    pub kind:       TransitionKind,     // TASK_STATE_CHANGED | STEP_COMMITTED | …
    pub from_state: Option<TaskState>,
    pub to_state:   Option<TaskState>,
    pub attempt:    Option<u32>,
    pub reason:     TransitionReason,   // which frozen rule authorises it
    pub occurred_at_ms: TimestampMs,    // from the injected Clock, never ambient
}

// One participant, one transaction, one transition. `&self`, not `&mut self`:
// a participant that needs to sequence a counter does it in SQL, not in memory.
pub trait TransactionParticipant {
    fn record(
        &self,
        tx: &Transaction<'_>,
        transition: &DurableTransition,
    ) -> Result<(), StoreError>;
}

// The seam itself. The transition is a parameter, so it cannot be stale.
pub fn transact<P: TransactionParticipant>(
    store: &Store,
    p: &P,
    t: &DurableTransition,
    body: impl FnOnce(&Transaction<'_>) -> Result<(), StoreError>,
) -> Result<(), StoreError> {
    let tx = store.begin_immediate()?;
    let r = body(&tx);            // any early return propagates; nothing is recorded
    tx.run_participants(p, t)?;   // SAME transition the body just performed
    match r {
        Ok(())  => tx.commit(),
        Err(e)  => { tx.rollback(); Err(e) }
    }
}
```

**Each property, and the line that proves it.**

| Required property | Proven by | Why the rejected shape could not |
| --- | --- | --- |
| Immutable transition identity passed **explicitly** | `transition: &DurableTransition` is a parameter of `record`; the struct has no interior mutability and no `Deref` to anything mutable | `CommitHook::append` had no identity parameter at all |
| Participant receives the **same** transition as the state writer | one `t: &DurableTransition` is threaded to `body` and to `run_participants`; there is no second copy to diverge | the hook's remembered transition could differ from the write |
| Participant has the **same `Tx`** | `record(&self, tx: &Transaction<'_>, …)` receives the `tx` `body` mutated | a hook that opened its own connection would break atomicity outright |
| No participant keeps pending transition in mutable object state | `&self`, and `DurableTransition` is borrowed not owned | `&mut self` + a stored field is exactly the stale-state defect |
| Early return cannot leave stale transition state | `body`'s `Result` is bound **before** `run_participants`, and any `Err` rolls back without recording | an early `return` dropped the hook mid-sequence |
| `TaskJournal` can implement it | one `INSERT INTO task_journal` per `journal_kind`, plus `journal_seq = (SELECT COALESCE(MAX(journal_seq),0)+1 …)` — all SQL, no in-memory counter | needs no interior mutability either |
| A future `EventBus` can implement it | same signature; P3 adds a second `p2` and constructs it with the *same* `t` | this is the whole P3 mechanism, so it must fit the shape now |
| No runtime plugin registry required | `transact` takes one `&P`; composition is explicit at the call site | a `Vec<Box<dyn TransactionParticipant>>` would need registration, ordering and interior mutability for no gain |

**Why explicit participants and not a registry.** A registry would need to answer
"in what order?", "may a participant refuse the commit?", and "who owns the
participant?" — and P2 has exactly one participant and a known one for P3. Explicit
composition makes the participant set visible in the type at every call site, gives
the borrow checker the lifetime relationship for free, and keeps ordering a
syntactic fact. **If a third participant ever appears whose order is not
syntactic, revisit this; do not pre-build for it.** The `run_participants` helper is
where a second participant is added, and nothing else changes.

**Two properties this does *not* claim.** `E3` is not claimed — `TaskJournal` is not
an event participant, and adding one is P3. And a participant **cannot** observe
post-commit state, because it runs inside the transaction; anything that needs
post-commit visibility is a different mechanism with a different name.

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
- **Corruption.** Four verification tiers, defined because each pragma verifies
  something different and the P2 autonomous audit established exactly what.
  `PRAGMA quick_check` and `PRAGMA integrity_check` are **page-level** checks:
  against a database holding one deliberately orphaned `task_steps` row, **both
  return `ok`**, and only `PRAGMA foreign_key_check` reports it. So no claim about
  referential integrity may rest on the first two.

  | Tier | Check | Cost | When |
  | --- | --- | --- | --- |
  | Normal open | `quick_check` | cheap | every open |
  | Post-migration | `quick_check` **+ `foreign_key_check`** | cheap on a fresh schema | after each migration, inside that migration's gate |
  | Explicit admin | `integrity_check` **+ `foreign_key_check`** | O(database) | `verify_integrity()`, on demand |
  | Recovery precondition | `foreign_key_check` | cheap | before classifying, so §9.1 row 3b's "spanning tables" case is decidable |

  The post-migration tier is the one that earns its place: a migration that
  produced dangling references has failed in a way `quick_check` cannot see, and
  this schema leans on foreign keys for both the cascade delete and the
  cross-class anti-laundering property.

  **Wording frozen, because these are the exact claims.** Each tier is stated as
  the set of properties it establishes and nothing wider. Re-verified by execution
  during the final closure run:

  | Tier | Establishes | Does **not** establish |
  | --- | --- | --- |
  | Normal open | Page-level structural integrity: b-tree ordering, page linkage, cell and record well-formedness, freelist consistency | Referential integrity. On an FK-orphaned database `quick_check` returns `ok` |
  | Post-migration | The above, **plus** that no row references a missing parent, checked by `foreign_key_check` | That the migration produced the *intended* rows, only that it produced a referentially whole schema |
  | Admin full verify | Everything `integrity_check` reports — the same page-level class as `quick_check`, exhaustively and with index-versus-table cross-checks — **plus** `foreign_key_check` | Application invariants. Neither pragma knows what a `TaskStep` is |
  | Recovery precondition | That referential damage is absent, so §9.1's classification is decidable | That the database is otherwise sound; recovery's own classification reads the rows |

  Three consequences the implementation must honour:

  1. **`quick_check` and `integrity_check` must never be described as detecting
     foreign key violations.** They do not. `PRAGMA foreign_key_check` is the only
     one of the three that does, and it is named explicitly in three tiers above.
  2. **Both page-level pragmas return *multiple rows* on damage**, not one error
     string, so the check is "no row differs from `ok`" and never "the first row
     equals `ok`". `PRAGMA integrity_check(N)` bounds the error count and is used to
     keep a diagnostic bounded.
  3. **A successful read is not an integrity signal.** Measured on a database with
     two deliberately overwritten pages: `SELECT count(*)` returned the correct
     `500` while `quick_check` reported `*** in database main ***`. Reads that touch
     only intact pages succeed on a corrupt file.

- **Referential integrity is pragma-dependent, and that is stated rather than
  implied.** `PRAGMA foreign_keys` **defaults to `ON` under `bundled`** — not
  `OFF` — because `libsqlite3-sys` compiles the amalgamation with
  `-DSQLITE_DEFAULT_FOREIGN_KEYS=1`. Upstream SQLite's own default is `OFF`. Two
  consequences, and the second is the reason the store still sets it explicitly:

  1. **Do not rely on the default.** It differs between a `bundled` build and a
     system-SQLite build, and P2 refuses the system build, but the store asserts
     `foreign_keys = ON` at open regardless so that the assertion is *observed*
     rather than inherited from a build flag.
  2. **The claim is unchanged either way.** A writer sets `foreign_keys = OFF` with
     one line and then inserts an orphan. Every `CHECK`, trigger and `FOREIGN KEY`
     here holds against a writer who leaves `foreign_keys = ON` and
     `ignore_check_constraints = OFF`, and the threat model already excludes a local
     file writer from tamper-evidence. What the `OFF` default buys is a *foot-gun*,
     not a guarantee, and `bundled` removes the foot-gun without adding a guarantee.
     See [schema §7](P2-sqlite-schema.md#the-pragma-boundary-in-full).

  `PRAGMA foreign_keys` is a **no-op inside a transaction**, and the setting is
  **discarded** rather than deferred: measured, setting it `ON` inside an open
  transaction and then committing leaves it `OFF`. That is why §7.2 sets it at open
  and not inside a migration.

  ADR-0005's open item about verifying durability settings against the platform is
  not closed by any of this, and is not claimed to be.

### 7.2 Connection policy

**Two profiles, not one.** The P2 autonomous audit established by execution that
`Store::open_in_memory` **cannot** satisfy ADR-0005's WAL requirement, so a single
table asserting WAL at open is unimplementable for one of the two constructors:

| Property | `:memory:` | file-backed |
| --- | --- | --- |
| `PRAGMA journal_mode` | **`memory`** | `wal` |
| `PRAGMA synchronous` (**read**) | returns a row, value `2` | `0`/`1`/`2`/`3` honoured |
| `PRAGMA synchronous = FULL` (**set**) | returns **no row** | returns **no row** — see below |
| `PRAGMA foreign_keys` | `1` — `bundled` default, see §7.1 | `1` — `bundled` default, see §7.1 |
| `PRAGMA journal_mode = WAL` | returns `memory`; the request is **silently ignored** | returns `wal` |
| `PRAGMA wal_checkpoint(TRUNCATE)` | one row, value `0` | one row, value `0` |
| `PRAGMA database_list` | one row: `(0, "main", "")` | one row: `(0, "main", "<path>")` |

Three of those cells were mis-recorded by the earlier draft and are corrected here.
All are measured against `rusqlite` 0.40.2 + `bundled` (SQLite 3.53.2):

- **Reading `synchronous` on `:memory:` returns `2`, not `1`.** The value is
  `SQLITE_DEFAULT_SYNCHRONOUS=2` from the bundled `compile_options`, reported
  truthfully, and it means nothing because there is no file to fsync.
- **`PRAGMA wal_checkpoint(TRUNCATE)` on `:memory:` returns a single row `0`, not
  `(0, -1, -1)`.** There is no WAL, so the checkpoint succeeds vacuously. The
  correct statement is "not applicable", not a specific triple — a caller that
  destructures three columns would have been reading past the end of the result.
- **"Setting `synchronous` returns no row" is not an in-memory quirk.** Measured on
  the **file-backed** profile too: `PRAGMA synchronous = FULL` returns no row there
  as well. This is how SQLite's assignment pragmas behave generally, not a symptom
  of memory backing. The genuinely in-memory-specific fact is that the setting is
  **unenforceable**, because `journal_mode = memory` means nothing can be fsynced.

So the durability gap is real but it is narrower than "the pragma misbehaves":

| Setting | `ProductionProfile` | `TestMemoryProfile` | Why |
| --- | --- | --- | --- |
| `journal_mode` | `WAL`, **asserted** | `memory`, **asserted as `memory`** | ADR-0005. The in-memory profile asserts what it actually is, so a test cannot "pass" by skipping the check. Requesting `WAL` in memory returns `memory` rather than erroring, which is precisely why the assertion has to be a read-back |
| `foreign_keys` | `ON`, **asserted** | `ON`, **asserted** | The blob reference integrity and the cascade delete depend on it. A silently-off pragma turns every `FOREIGN KEY` in the schema into a comment. **Not** set inside a migration: `PRAGMA foreign_keys` is a no-op inside a transaction, and every migration runs inside `BEGIN IMMEDIATE` |
| `synchronous` | `FULL`, **asserted** | **not asserted**; documented as unenforceable | Task Protocol §5 rule 1 — a step's success and its receipt are committed before the task advances — is the point of this phase, and in WAL mode `NORMAL` can lose the last commits on **power** loss (not process crash). An fsync per commit is milliseconds on an SSD |
| `busy_timeout` | 5000 ms, asserted | asserted | Single writer, low contention, and a bounded wait rather than an immediate `SQLITE_BUSY`. Measured: two writers serialise correctly; a second `BEGIN IMMEDIATE` waits out the timeout and then reports `SQLITE_BUSY`. **The 5000 ms default is `rusqlite`'s, not SQLite's** — SQLite's own default is `0`, meaning immediate `SQLITE_BUSY`. Asserting it is asserting a deliberate choice rather than inheriting one |
| `wal_autocheckpoint` | SQLite default (`1000` pages) | SQLite default | Do not tune what was not measured |
| `wal_checkpoint` | `TRUNCATE` on clean close | **not performed** — vacuous | Bounds WAL growth across restarts. Measured: with a concurrent reader holding a snapshot, `TRUNCATE` returns `busy = 1` having checkpointed 3 of 4 frames, so `busy` must be checked rather than discarded; `PASSIVE` returns `busy = 0` and does what it can |
| `temp_store` | **default**, deliberately not `MEMORY` | default | A temp table spills to a file that is *not* at-rest protected. ADR-0022's protection covers `blobs.content`, not SQLite's scratch space. Any future change here must re-open that question |
| `application_id` / `user_version` | **not set** | not set | `schema_migrations` is the single authority |
| Connection count | **1**, behind a `Mutex` | 1 | SQLite is single-writer. The mutex guards the *connection*, never lease semantics and never a transition |

**Every durability-bound test uses `ProductionProfile` on a file.** Not by
convention — by rule, stated as a positive list in §7.3, because the alternative
is a suite that passes only because the in-memory profile bypassed a production
requirement.

**The in-memory profile is never described as satisfying ADR-0005.** It cannot: its
`journal_mode` is `memory`, not `wal`, it has no `-wal` and no `-shm`, it cannot be
reopened, and closing it discards the schema entirely — measured, reopening an
`:memory:` database after close fails with `no such table`. Any documentation,
doc-comment or test name that calls the in-memory profile durable, persistent or
WAL-backed is wrong and is a review finding.

### 7.3 Test database policy

- **`Store::open_in_memory` / `TestMemoryProfile`** for pure unit and property
  tests: schema construction, constraint refusals, canonicalization, enum and
  matrix logic. It **cannot be reopened** and has **no** crash durability, so it is
  never used for a durability-bound test.
- **A file-backed store under a RAII `TempStore`** — and the rule is a positive
  list, so "which tests may use memory" is answerable by reading one sentence:

  | Must be file-backed | Why |
  | --- | --- |
  | Reopen and reopen-equality | `open_in_memory` cannot be reopened at all |
  | Crash and fault injection | Needs a real file, a real WAL and a real `fsync` |
  | Migration, migration-reopen, `SchemaTooNew`, checksum mismatch | Needs the file and its sidecars |
  | Cascade delete and the §5.4 blob sweep | Needs durability across statements |
  | Lease reclaim against expiry | Needs two independent connections |
  | Any assertion about `journal_mode`, `synchronous`, or the close checkpoint | Those pragmas do not mean what they claim in memory |
  | Any two-process or two-`Store`-instance test | Needs a shared file |

- **`TempStore` identity.** Constructed from the three things that are jointly
  unique with no wall clock and no RNG, so `.clippy.toml`'s ban on `SystemTime::now`
  / `Instant::now` holds and two processes cannot collide:

  ```text
  <binary-identity>-<pid>-<atomic-counter>
  ```

  The counter is a process-wide `AtomicU64`. The binary identity is the test's own
  label, so **two integration-test binaries cannot collide** — a counter alone
  does not achieve this, and the P2 autonomous audit verified that two binaries each
  counting from 0 produce the same three names. A pid alone is also insufficient,
  because `cargo test` runs tests as threads of one process and the crash harness
  spawns children; hence the counter as a third component. `TempStore::new(label)`
  builds one; `TempStore::child_inherited(dir)` takes the parent's directory, which
  is also how a crash child reopens the file it must assert against.

- Every crash test runs in a **child process** re-invoking the test binary with
  `current_exe()`, which is the only way to test durability rather than the
  in-process rollback path.

### 7.4 `rusqlite`, and the alternatives

**Recommendation: `rusqlite` with `default-features = false, features =
["bundled"]`, synchronous, one connection.**

Requirements the dependency must satisfy — stated as capabilities rather than
feature strings, because no dependency is resolved by the design-preparation run:

| Requirement | Needed for | Satisfied by |
| --- | --- | --- |
| SQLite ≥ 3.37.0 | `STRICT` tables; the generated class labels | bundled **3.53.2** |
| JSON1 present | `json_valid` / `json_type` in `tasks.extensions` and `error_details` | built into SQLite core by default since 3.38; `json_valid` and `json_extract` both verified against the bundled build |
| per-connection pragmas | §7.2 | `Connection::pragma_update`, no feature needed |
| statement-level `rows_affected` | the explicit zero-row fence check | always available |
| `INSERT … ON CONFLICT … DO UPDATE … WHERE` | ADR-0024's atomic `acquire_lease` | always available |
| `STRICT` + `GENERATED … STORED` | the schema, unencoded | both verified constructible on the bundled build, so the §8 fallback is dead |

**Resolved at audit time, so tomorrow's implementation does not spend reasoning
effort discovering basic crate facts.** No dependency is added by the design or
the audit; this is the record P2C reads. **Every value below was re-verified from
crate source and the crates.io API during the final closure run, not copied from the
previous audit.**

| Item | Value | How verified |
| --- | --- | --- |
| Candidate | **`rusqlite` 0.40.2**, published 2026-08-08 | crates.io API |
| License | **MIT** | crate `Cargo.toml` |
| MSRV declared on `rusqlite` | **none** — no `rust-version` field at all | crate `Cargo.toml` |
| Transitive crate | **`libsqlite3-sys` 0.38.2**, `edition = "2021"`, **no `rust-version` field**, license **MIT** | crate `Cargo.toml` |
| Bundled SQLite | **3.53.2**, `SQLITE_SOURCE_ID` `2026-06-03 19:12:13 d6e03d8c…` | `libsqlite3-sys/sqlite3/sqlite3.h` line 149 and `sqlite3.c` line 470, **and** `SELECT sqlite_version()` / `sqlite_source_id()` on a live connection |
| Native build | C toolchain via `cc`; `bundled` implies `modern_sqlite` implies `bundled_bindings`, so `build.rs` copies `sqlite3/bindgen_bundled_version.rs` and **no local `bindgen`/`libclang` is required** | `rusqlite` + `libsqlite3-sys` `Cargo.toml` feature graph and `build.rs` |
| System SQLite | **not linked.** `otool -L` on the built binary lists no `libsqlite3`, and the bundled source id string is present in the binary — the amalgamation is statically linked | link inspection |
| Apple Silicon, Intel macOS, Linux | identical — `bundled` compiles the same 3.53.2 from the same source on all three, so no system SQLite and no ABI question | by construction |

**Three defaults must be overridden, and each is a trap. Two of the three earlier
statements about them were factually wrong and are corrected here.**

1. **`rusqlite`'s own defaults are `["cache", "ffi-sqlite-wasm-rs"]`.** `cache`
   pulls `hashlink` (+ `hashbrown`, `foldhash`), and `ffi-sqlite-wasm-rs` pulls
   **`sqlite-wasm-rs`** (+ `rsqlite-vfs`, `wasm-bindgen`, `js-sys` on wasm targets).
   Both are unwanted, so `default-features = false` is **required**, not tidiness.
   Measured cost on `aarch64-apple-darwin`: **11** packages compiled, versus **20**
   for the chosen configuration.
2. **`libsqlite3-sys`'s default feature is `["min_sqlite_version_3_34_1"]`, which
   expands to `["pkg-config", "vcpkg"]`** — that is, **system SQLite**, the exact
   failure mode this section exists to avoid. The earlier draft named the feature as
   `min_sqlite_version_3_45_3`; that is **wrong**, the feature is
   `min_sqlite_version_3_34_1`.

   One precision that matters, because "overridden" is doing too much work in the
   earlier phrasing: `rusqlite` declares its non-wasm `libsqlite3-sys` dependency
   **without** `default-features = false`, so `libsqlite3-sys`'s own defaults **are
   still enabled** even under `default-features = false` on `rusqlite`.
   `pkg-config` and `vcpkg` are therefore still *compiled* as build dependencies.
   What `bundled` overrides is the **discovery path**, not the compilation:
   `build.rs` takes the bundled branch, never consults `pkg-config`, and the
   resulting binary statically links the amalgamation. Verified, not assumed — see
   the `otool -L` row above.
3. **`bundled-full` is rejected.** Measured at **81** packages compiled on
   `aarch64-apple-darwin`, against 20 for the chosen configuration. It expands to
   `chrono`, `jiff`, `time`, `serde_json`, `url` (and transitively the whole `icu_*`
   / `idna` tree), `uuid`, `csv`, `series`, `vtab`, `window`, `load_extension`,
   `unlock_notify`, `column_metadata`, `trace`, `hooks`, `backup`, `collation` and
   `limits`. Nothing in §7.2's table needs any of it.

```toml
rusqlite = { version = "0.40.2", default-features = false, features = ["bundled"] }
```

**`bundled` versus system SQLite — and a concrete reason it is not merely
preferable here.** The general argument is unchanged and sound: `bundled` makes
every developer and CI machine compile the same version, and a `STRICT`-table
migration that works on a laptop and fails on a CI runner is the worst possible
failure mode. On this host the argument is sharper than that. The system SQLite
available here is **3.43.2**, and SQLite's own WAL documentation records the
**WAL-reset bug** as present in "all versions of SQLite from 3.7.0 (2010-07-21)
through 3.51.2 (2026-01-09)", fixed in **3.51.3 (2026-03-13)** and later. That bug
can corrupt a WAL-mode database when two connections write and checkpoint
concurrently — precisely Serea's shape, with a second connection used for lease
tests and for recovery. The bundled **3.53.2** is past the fix; the system
**3.43.2** is not, and the published backports (`3.44.6`, `3.50.7`) do not cover
it. Choosing system SQLite here would select a version with a known
data-corruption bug in exactly the concurrency pattern this design uses. The cost of
`bundled` is a `build.rs`, a C toolchain in CI, and slower builds; `ubuntu-latest`
has a toolchain and `macos-latest` has Xcode CLT.

**`bundled` also silently changes one pragma default, which is why §7.1 sets
`foreign_keys` explicitly.** The bundled amalgamation is compiled with
`-DSQLITE_DEFAULT_FOREIGN_KEYS=1` (plus `ENABLE_API_ARMOR`, `ENABLE_COLUMN_METADATA`,
`ENABLE_DBSTAT_VTAB`, `ENABLE_FTS3`, `ENABLE_FTS5`, `ENABLE_JSON1`,
`ENABLE_LOAD_EXTENSION`, `ENABLE_RTREE`, `ENABLE_STAT4`, `THREADSAFE=1`). One of
those is load-bearing for §7.1's wording and another is a trap:

- `SQLITE_DEFAULT_FOREIGN_KEYS=1` makes `PRAGMA foreign_keys` default to `ON`,
  unlike upstream SQLite's `OFF`.
- `ENABLE_LOAD_EXTENSION=1` means the **C** capability to `sqlite3_load_extension`
  is compiled in **whether or not** the `rusqlite` `load_extension` feature is
  enabled. Per §7.4's last rule, Serea enables no Rust feature for it and exposes
  no API for it. A future claim that "extension loading is not compiled in" would be
  false; the accurate claim is that no Rust binding for it is linked.

**MSRV: there is no conflict, and no rise is required.** The earlier draft recorded
that `libsqlite3-sys` 0.38.x "declares `rust-version = "1.88.0"` and `edition =
"2024"`" and made raising the workspace MSRV the single genuine owner decision in
P2. **Both halves of that premise are false**, and the conclusion falls with them:

| Claim in the earlier draft | Verified reality | Source |
| --- | --- | --- |
| `libsqlite3-sys` 0.38.x sets `rust-version = "1.88.0"` | **It has no `rust-version` field at all** | `libsqlite3-sys-0.38.2/Cargo.toml` |
| `libsqlite3-sys` 0.38.x is `edition = "2024"` | **`edition = "2021"`** | same |
| `rusqlite` 0.40.2's `bundled` path raises the effective MSRV above 1.85 | **`cargo +1.85.0 check` and `cargo +1.85.0 run` both succeed** against `rusqlite 0.40.2` with `default-features = false, features = ["bundled"]`, compiling the SQLite amalgamation and returning `sqlite_version() = 3.53.2` from the resulting binary** | direct execution with the 1.85.0 toolchain |

Both crates publish the same MSRV policy instead of a number: *"Latest stable Rust
version at the time of release. It might compile with older versions."* For
`rusqlite` 0.40.2, published 2026-08-08, the then-current stable was **1.97.1**
(1.98.0 shipped 2026-08-20), so **1.97** is the version those crates were *tested
against* — a floor on their *own* CI, not on this workspace. It is not a
requirement, and it is **not** what the workspace needs, because the whole chain
builds on **1.85.0** today.

**Decision: keep `rust-version = "1.85"` and `.clippy.toml` `msrv = "1.85"`. Unchanged.
No `Cargo.toml` edit is needed, proposed or recorded for P2C.**

This is not "manufacturing a need to keep 1.85" — it is the option that costs
nothing. The alternative, raising the MSRV to the stack's tested-against version,
would buy nothing measurable: the same SQLite 3.53.2, the same STRICT support, the
same generated columns, the same Apple Silicon and Intel macOS and Linux behaviour,
the same 20-package dependency surface, and the same `cc`/`pkg-config`/`vcpkg` build
dependencies. It would cost a higher minimum toolchain for every contributor and CI
runner for no capability gained, on the strength of a version number that appears in
no `Cargo.toml` in the chain. Both of the stated working preferences — prefer the
current maintained SQLite stack, avoid an MSRV rise justified only by a developer
machine being on 1.98 — are satisfied **simultaneously**, because they were never in
conflict.

The full comparison the owner asked for, both alternatives measured:

| Measure | **Chosen: `rusqlite` 0.40.2 + `bundled`, MSRV 1.85** | Alternative A: raise MSRV to 1.97, same crates | Alternative B: keep 1.85, *older* `rusqlite` |
| --- | --- | --- | --- |
| SQLite version | **3.53.2** | 3.53.2 — identical | older; would have to be re-verified |
| `STRICT` support | yes (needs ≥ 3.37.0) | yes | version-dependent |
| Generated columns | yes (`GENERATED … STORED`) | yes | version-dependent |
| WAL-reset bug (fixed 3.51.3) | **not affected** | not affected | **likely affected** — a pre-3.51.3 `rusqlite` selects a vulnerable SQLite, which is the whole reason to stay current |
| Maintenance age | current release | current release | stale, and stale *unsafely* |
| Apple Silicon | `bundled`, no system SQLite | identical | system or older bundled |
| Intel macOS | identical | identical | identical |
| Linux CI | `cc` available | identical | identical |
| Build dependencies | `cc`, `pkg-config`, `vcpkg` | identical | identical |
| Transitive surface | **20** packages compiled | **20** — identical | ≥ 20 |
| Feature differences | none | none | none |
| Known implementation cost | **zero** | one-line `Cargo.toml` + `.clippy.toml` edit, and a raised floor for everyone | re-verifying every fact in this table against a different crate, and shipping a SQLite with a known corruption bug |
| Verdict | **adopt** | **rejected** — buys nothing | **rejected** — costs correctness |

**The §8 fallback is not needed for this candidate.** Bundled 3.53.2 is far above
3.37.0, so `STRICT` and `GENERATED … STORED` are both available and the
`CHECK (typeof(col) = …)` degradation is **verified unnecessary**. It is retained
only as a branch for a future `rusqlite` whose bundled SQLite might predate 3.37.0,
which is a hypothetical several years out at the current release cadence.

**SHA-256.** `sha2` **0.11.0**, published 2026-03-25. `rust-version = "1.85"`,
`edition = "2024"` — so its MSRV is **exactly** the workspace MSRV, with no
conflict. License **MIT OR Apache-2.0**. Default features `["alloc", "oid"]`;
`default-features = false` is sufficient, because Serea computes a SHA-256 digest
and neither `alloc` (nothing here allocates) nor `oid` (no ASN.1 object identifiers
are used) is needed for it. Pure Rust, no clock, no network, no randomness, standard
FIPS 180-4. `0.10.9` is the last `0.10.x` fallback.

```toml
sha2 = { version = "0.11.0", default-features = false }
```

Three implementation facts about `sha2` 0.11 that P2B must not rediscover:

1. **`Digest::finalize()` returns `Array<u8, …>`, which does not implement
   `LowerHex`.** `format!("{:x}", h.finalize())` does not compile. Measured: the
   compiler reports `the trait LowerHex is not implemented for Array<u8, …>`.
   `0.10`'s `Output<Sha256>` did implement it, so this is an API break across the
   version bump that will silently cost an afternoon. Hex-encode explicitly —
   `out.as_slice().iter().map(|b| format!("{:02x}", b)).collect::<String>()` — and
   pin the result in the SCJ-1 vectors.
2. **Architecture behaviour is transparent, which is the point.** `sha2` 0.11 takes
   an optional `cpufeatures` dependency on `aarch64`/`x86`/`x86_64` and selects an
   `aarch64-sha2` or `x86-sha` hardware backend at runtime. The accelerated backends
   compute the same SHA-256, so the digest is byte-identical across architectures;
   the acceleration is an implementation detail with no wire effect. This is a
   *stronger* portability statement than "no platform-specific behaviour", and it
   belongs in the portability invariant.
3. **The crate's `rust-version = "1.85"` is a declared fact**, unlike the SQLite
   crates, so this is the one MSRV in the P2 dependency set that is actually pinned.

Recorded, **not added**, by this run.

**A canonical-number dependency is not required by SCJ-1.** Rule 6 refuses every
`f64`, so P2B needs no float formatter at all. If P5 later admits fractions, the
crate is **`ryu-js`** 1.0.3 (MSRV 1.71) — which implements the ECMAScript
`Number::toString` algorithm that RFC 8785 requires — and **not** `ryu`, whose
shortest-round-trip output is not the ECMAScript form, and **not** `std`, whose
`f64` `Display` mismatches five of RFC 8785 Appendix B's twelve reference values.
See [ADR-0019 SCJ-1 rule 6](../decisions/ADR-0019-canonical-json-and-idempotency-preimage.md).

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

**Why not the `rusqlite` async or `bundled-full` variants.** They add a runtime or
a large feature surface, neither of which §7.2 needs.

### 7.5 Moving a Serea database to another machine

**The earlier draft's rule was wrong and is replaced.** It said: *"Copy
`serea.sqlite` **and** its `-wal` **and** its `-shm` together, or copy neither. A
`-wal` without its `-shm` (or an `-shm` from another machine) is discarded by SQLite
on open…"*, and made the three-file copy the supported procedure. Two of its claims
are contradicted by upstream SQLite's own documentation and by execution.

**What upstream actually says, and it is unambiguous.** The WAL file format is
documented as *"precisely defined and is cross-platform"*, and the WAL is *"part of
the persistent state of the database and should be kept with the database if the
database is copied or moved."* The wal-index is categorised differently, in the
file-format document:

> "Because the wal-index is **transient**, it can use an **architecture-specific
> format; it does not have to be cross-platform**. Hence, unlike the database and
> WAL file formats which store all values as **big endian**, the wal-index stores
> multi-byte values in the **native byte order of the host computer**."

That is the distinction the earlier draft missed. It is also confirmed on disk here:
the main file's header bytes 18/19 are `2`/`2` (WAL mode recorded in the database
header) and the `-wal` header reads `0x377f0682`, format `3007000`, page size
`4096` — all **big-endian**. The `-shm` file's first word reads `3007000` as a
**little-endian** `u32` on this `x86_64` host and as a meaningless `417475840`
read as big-endian, which is exactly the "native byte order" the file-format
document describes.

**And `-shm` is not merely rebuildable in principle — it is deleted.** SQLite's WAL
documentation: *"When the last connection to a database closes, that connection
does one last checkpoint and then deletes the WAL and its associated shared-memory
file."* Measured, on a file-backed store after a clean close: **both `-wal` and
`-shm` are absent.** The wal-index is never synced either, so there is nothing in
it worth carrying.

The stronger proof is behavioural. Experiment: commit rows into a WAL database,
simulate process death without closing (so `-wal` and `-shm` both survive), then
**delete the `-shm`** and reopen. SQLite rebuilds the wal-index from the `-wal`
alone, recovers every committed row (`value = "committed-in-wal"`), and
`quick_check` returns `ok`. The `-shm` is reconstructible from durable state, which
is the definition of an artifact rather than an asset.

#### Classification — the words are frozen

| File | Status | Format | Carry it when moving machines? |
| --- | --- | --- | --- |
| `serea.sqlite` | **durable migration asset** | big-endian, cross-platform | **Yes — always** |
| `serea.sqlite-wal` | **durable migration asset, conditionally** | big-endian, cross-platform | **Yes, if it exists** — it holds committed transactions not yet checkpointed into the main file |
| `serea.sqlite-shm` | **transient, rebuildable artifact** | **native byte order, architecture-specific by upstream's own statement** | **No.** Never copy it |

#### The normal supported path

This is the procedure, and it is the one to put in user-facing documentation:

1. **Stop Serea.** Not "close the window" — stop the process, so the last
   connection closes cleanly.
2. **Confirm no other reader or writer holds the file.** A second connection from
   another Serea instance, a stray `sqlite3` shell, or a backup agent all count.
3. **Checkpoint the WAL.** `PRAGMA wal_checkpoint(TRUNCATE)`. Check the `busy`
   column: measured, `TRUNCATE` returns `busy = 1` and completes only partially
   while another connection holds a read snapshot, so a `busy = 1` here means *stop
   and find the other connection*, not "carry on".
4. **Close all SQLite connections.** This deletes `-wal` and `-shm` as a side
   effect, which is the desired outcome and the reason the single-file case is the
   normal one.
5. **Copy `serea.sqlite` alone.**
6. **Open it on the destination**, which applies the normal open checks (§7.1's
   normal-open tier).
7. **Run the migration, integrity and recovery checks** on the destination before
   treating the data as live.

Steps 1–4 are what "stop Serea" means operationally, and steps 5–7 are what P2's
open path already does. **No architecture-specific handling appears anywhere in
this procedure**, which is the whole point.

#### The abnormal case: committed frames remain in the `-wal`

If Serea did not stop cleanly — a crash, a `SIGKILL`, a dead power supply — the
`-wal` may hold committed transactions that are not yet in the main file, and
**copying only `serea.sqlite` would silently lose them.** Upstream is explicit:
*"If a database file is separated from its WAL file, then transactions that were
previously committed to the database might be lost, or the database file might
become corrupted."*

So in the abnormal case:

1. Confirm no process holds the file on the source. Never copy a `-wal` from a
   **live** writer: the copy is not atomic with respect to SQLite's appends, and a
   torn tail is exactly the "database file might become corrupted" outcome.
2. Copy `serea.sqlite` **and** `serea.sqlite-wal`. **Do not copy `-shm`.**
3. Open on the destination. SQLite rebuilds the wal-index from the `-wal`, recovers
   the committed frames, and the normal-open tier then verifies the result.
4. Run the recovery classification (§9) before trusting the state, because an
   unclean stop is precisely the condition recovery exists for.

`-wal` **without** `-shm` is therefore the *correct* abnormal-case artifact set, and
it is safe precisely because the wal-index is reconstructible. The earlier draft had
this backwards.

#### Two consequences that must not be softened

- **Never instruct a user to copy `-shm` across `x86_64` → `arm64`.** Upstream does
  not guarantee it is safe; it explicitly permits the format to be
  architecture-specific. The fact that both Intel and Apple Silicon are
  little-endian, so a byte-swap is not the failure mode, does not upgrade "permitted
  to be architecture-specific" into "guaranteed portable", and the wal-index also
  carries native-width values and a native hash-table layout that are not merely a
  byte-order question.
- **Do not claim a stale `-shm` is "discarded by SQLite on open" as a safety
  property.** Measured, copying one database's `-shm` onto a different database
  caused no observable damage — SQLite validates the wal-index header and rebuilds
  when it does not match. That is a *reassurance about the observed case*, not a
  guarantee, and it is a second reason not to depend on carrying the file.

#### Read-only media, which the migration procedure runs into

Upstream records that before SQLite 3.22.0 a WAL-mode database could not be opened
read-only at all, and since then only under three conditions: the `-shm` and `-wal`
already exist and are readable; the containing directory is writable; or the
connection uses `immutable=1`. Measured on this build: a WAL-mode database opened
read-only with a read-only directory and **no** sidecars fails every read with
`attempt to write a readonly database` (`SQLITE_READONLY_DBMOVED`, extended code
1544); with `-wal` and `-shm` both present it opens and read-only reads **see the
committed WAL rows**. If P2 ever supports a read-only store, it must either ship the
sidecars, require directory write permission, or open `immutable=1` — and it must say
which.

### 7.6 The portability invariant, and the fixture that proves it

**The invariant, stated once so it can be ratified rather than re-derived.**

> Serea core and durable state MUST support migration between
> `x86_64-apple-darwin` and `aarch64-apple-darwin` without architecture redesign.

The previous audit reached "no current portability blocker" from a source audit on
an `x86_64` host. A negative found by reading is weaker than a positive that is
written down, so the conclusion is converted into a requirement. **This text is
placed here, in the P2 implementation plan, and not added to the frozen architecture
documents**, because this run is documentation-only and the frozen texts are under
change control. Ratifying it is one of tomorrow's items.

**What the implementation must not persist.** Each is a way a Rust or SQLite value
becomes architecture-dependent once written to disk:

| Must not persist | Why it breaks a move |
| --- | --- |
| `usize` / `isize` | Width is target-dependent: 4 bytes on a 32-bit target, 8 on 64-bit. Both Apple Silicon and Intel macOS are 64-bit, so this bites on a future target rather than on this move — which is exactly why it is forbidden now rather than discovered later |
| Native-endian integers | Byte order differs by target. The durable format is **big-endian** for the main file and the WAL, per SQLite's file-format documentation |
| Raw Rust structs (`#[repr(C)]` or not) | Layout is unspecified and may gain padding, reorder, or change between compiler versions |
| Pointer values | Meaningless in another process, on another machine, after any restart |
| Platform ABI structs | Layout is platform-defined, not Serea-defined |
| Architecture-dependent float serialization | The durable representation is a decimal string or a scaled integer, never a binary `f64` |
| Architecture-specific SQLite sidecar state **as durable application state** | The wal-index is native byte order (§7.5). It is a transient artifact and is never application state |

The last row is the one that is easy to get wrong by accident: the `-shm` is not
Serea's, and the moment anything reads it as if it were, portability is gone.

**What the implementation must not hard-code.** `/usr/local` (Intel Homebrew),
`/opt/homebrew` (Apple Silicon Homebrew), and any architecture-qualified executable
path. A path that differs between the two Macs is a hard failure at exactly the
moment the user moves their database. **Platform-specific behaviour stays behind
platform adapters**, per Crate Map's existing rule; the core is written once.

**Two supporting facts that make the invariant cheap here.** `bundled` compiles the
same SQLite 3.53.2 from the same source on all three targets, so no system SQLite
and no ABI question arises — and `otool -L` on the binary shows no `libsqlite3` at
all. And `sha2` 0.11 selects an `aarch64-sha2` or `x86-sha` hardware backend at
runtime via `cpufeatures`; the accelerated backends compute the same SHA-256, so a
digest computed on an Intel Mac equals one computed on Apple Silicon **byte for
byte**. That is a stronger statement than "no platform-specific behaviour", and it
is why `digest` and `idempotency_key` need no cross-architecture caveat.

#### The cross-architecture fixture

**Not implemented by this run.** The deliverable is the design, so the CI job is
written once and the job is the deliverable rather than an assumption.

**Producer** — run once, on any one platform, committed as a small binary artifact:

1. Create a deterministic P2 fixture on a file-backed `ProductionProfile` store.
2. Insert deterministic data across every table the portability claim touches:
   `tasks` in several states; `task_steps` across **all 51 constructible**
   `kind × status` cells; a `blobs` row with `PRIVATE` and `PUBLIC` ranks so the
   generated `data_class` column is exercised; `leases` rows at two generations so
   `lease_generation` is non-trivial; `plan_revisions`; both blob-ref tables; and
   `task_journal` rows in order.
3. Every timestamp, id and digest is a **literal constant**, never derived from a
   clock or an RNG, so the fixture is byte-reproducible.
4. `PRAGMA wal_checkpoint(TRUNCATE)`, then **close every connection**.
5. Copy **only the main database file** — §7.5's normal path. Assert that `-wal`
   and `-shm` do not exist at artifact-creation time; if they do, the producer is
   wrong and the fixture is not the ordinary case.

**Consumers** — three jobs, `ubuntu-latest`, `macos-13` (Intel) and an Apple
Silicon runner. Each opens the artifact **read-write under `ProductionProfile`**
and asserts:

| # | Assertion | Pins |
| --- | --- | --- |
| 1 | `PRAGMA schema_version`; `SELECT MAX(version) FROM schema_migrations`; the applied checksum string | schema migration/version |
| 2 | `sqlite_master` inventory is **10 tables / 7 triggers / 6 explicit indexes**, and each table's `sql` text matches | no object is architecture-dependent |
| 3 | `PRAGMA quick_check` has no row differing from `ok`; `PRAGMA foreign_key_check` returns zero rows | integrity on a foreign machine (§7.1's tiers) |
| 4 | Every stored digest re-computes from its own canonical bytes | digest stability |
| 5 | Every `idempotency_key` re-derives from IDK-1 and matches | IDK-1 is architecture-independent |
| 6 | Every `TaskStep` survives the full Rust round trip and compares **semantically**, field by field | `TaskStep` round trip |
| 7 | `lease_generation` values are exactly those written, at both generations | lease fencing state |
| 8 | The recovery classification for a seeded matrix is identical | recovery classification |
| 9 | `task_journal` rows read back in `journal_seq` order with identical content | journal ordering |
| 10 | `SELECT sqlite_version()` is `3.53.2` on **all three** | `bundled` really is uniform |

**Semantic equality, not byte equality.** Assertion 10 above is the one place a
literal is right, because there the point *is* that it is identical. Everywhere
else the comparison is semantic: row sets compared as ordered tuples of typed
values, not as file bytes. A byte comparison would be wrong — SQLite is free to
choose page layout, and the `sqlite_sequence`/freelist details are not part of
the contract. Asserting byte equality across platforms would produce a test that
fails for reasons that are not defects.

**The second fixture, kept separate.** A second, smaller artifact covers the
abnormal WAL-recovery case and is **not** the architecture-migration fixture,
because it tests a different thing:

1. Producer inserts and commits rows, then is killed without closing, so committed
   frames remain in the `-wal`.
2. The artifact is main database **plus `-wal`**, and **never** `-shm` (§7.5).
3. Each consumer asserts the committed frames are recovered on open, the row set
   equals the producer's, `quick_check` is `ok`, and the classification matches.

Keeping it separate matters: a fixture that carried a `-wal` would be testing WAL
recovery, and one that did not would be testing migration. Mixing them means a
failure does not say which property broke.

**Why this is deferred to CI and not claimed here.** The host is `x86_64`. The
fixture's producer has not been run, and the Apple Silicon consumer cannot be run
from this machine at all. The claim this section makes is narrow and honest: the
design contains nothing architecture-dependent, the *procedure* for moving a
database is architecture-neutral (§7.5), and the **job that would prove it is
specified**. Ledger 7.10 keeps this as `SAFE_DEFER` to CI for the same reason.

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
| 3b | Corruption not attributable to one task: a corrupt `schema_migrations` row, or a `foreign_key_check` failure spanning tables | `RefusedPass` | **No mutation at all.** The pass returns `Err`, because there is no task to attribute the damage to and blocking every task would be a worse lie. §7.1's recovery tier runs `foreign_key_check` **first**, so this row is decidable before any classification begins — and it is the only integrity pragma that can see a referential violation at all |
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
wanted for. With `event_seq` removed from the schema that count is simply the
journal row count: in P2 every transition predates an event participant, so the
number is the size of the window during which `E3` did not hold. It is a fact to
display, not a backlog to drain.

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

### 9.4 Migration and crash edge cases — classified

Each case below is classified into exactly one bucket, and the bucket names **who
owns the guarantee**. The buckets are not interchangeable: "SQLite guarantees it" is
a weaker claim than "P2 tests it", and a test that asserts a SQLite guarantee is
testing SQLite.

| Bucket | Meaning |
| --- | --- |
| **DIRECTLY TESTED** | P2 has a named test in the matrix that asserts this |
| **TYPED FAILURE** | Enforced by the code returning a named `StoreError`; no test needed to prove enforcement, but one exists to prove the mapping |
| **SQLITE GUARANTEE** | Provided by SQLite; P2 relies on it and does not re-test it. Recorded so the reliance is visible |
| **STRESS-ONLY** | Real but timing-dependent; a stress test with weak assertions, never an exact row count |
| **EXPLICIT DEFER** | Named phase; safe because the omission cannot produce a wrong claim |

| # | Case | Class | Basis |
| --- | --- | --- | --- |
| 1 | **Crash during the migration transaction** | **SQLITE GUARANTEE**, verified | SQLite's DDL is transactional. Measured: `BEGIN; CREATE TABLE c1(…); INSERT INTO schema_migrations …; ROLLBACK;` leaves **no** table and **no** row. §7.1's "the version marker and the DDL share a transaction" depends on this and it is true |
| 2 | **`PRAGMA user_version` participates in the migration transaction** | **SQLITE GUARANTEE**, verified | Measured: `user_version=1; BEGIN; PRAGMA user_version=9; ROLLBACK;` leaves `user_version = 1`. The pragma is **rolled back**, not deferred — so it is a safe second marker inside the same transaction |
| 3 | **Migration row committed but schema incomplete** | **IMPOSSIBLE by construction** | Case 1 and case 2 together: the DDL, the `schema_migrations` row and the version marker are one transaction. There is no interleaving in which one lands and another does not, because SQLite gives the whole `BEGIN IMMEDIATE … COMMIT` atomicity. Recorded as a proof, not a test |
| 4 | **Crash after `COMMIT`, before the caller observes `Ok`** | **DIRECTLY TESTED** | N6, via `SIGKILL` in a child process. Measured by the earlier audit: 0 rows before commit, 1 row after, `quick_check` ok, and SQLite recovers the stale `-wal` on the next open. This is the genuinely dangerous window and the reason recovery exists |
| 5 | **Checksum changed after a migration was applied** | **TYPED FAILURE** | `MigrationChecksumMismatch`; the store does not open. Note the limit honestly: the `checksum` column is **mutable by any writer**, so SQLite provides no protection here — the check is Serea's own comparison at open, and it detects a *different binary*, not a tampered file |
| 6 | **Duplicate migration ID** | **SQLITE GUARANTEE** + **TYPED FAILURE** | `version INTEGER PRIMARY KEY` gives a `UNIQUE constraint failed: schema_migrations.version`. Measured. The store additionally never renumbers or reuses a version, so the constraint is defence rather than the primary mechanism |
| 7 | **Migration downgrade / open-newer refusal** | **TYPED FAILURE** | `MAX(version) > Migrations::LATEST` ⇒ `SchemaTooNew`. No auto-downgrade, no best-effort open. Measured: `user_version = 99` is readable and produces the refusal signal |
| 8 | **Migration SQL that fails mid-statement** | **SQLITE GUARANTEE** | Case 1. A failed statement aborts the transaction; `execute_batch` surfaces the error and the store does not open |
| 9 | **Read-only database file** | **EXPLICIT DEFER**, with a measured constraint | Measured: a WAL-mode database with **no** sidecars, opened read-only with a read-only directory, fails every read with `attempt to write a readonly database` (`SQLITE_READONLY_DBMOVED`, 1544). With `-wal` and `-shm` both present it opens and **sees the committed WAL rows**. Upstream's three conditions are: sidecars present, directory writable, or `immutable=1`. **P2 does not claim read-only store support.** If it is ever wanted, the design must name which of the three it relies on |
| 10 | **Read-only directory** | **EXPLICIT DEFER** | Same measurement as case 9. The directory must be writable for SQLite to create `-shm`/`-wal`; that is a deployment requirement, not a code path |
| 11 | **Disk full** | **EXPLICIT DEFER** at the failure-semantics level | The correct behaviour is that SQLite returns `SQLITE_FULL`, the transaction rolls back, and nothing partial commits — which follows from case 1. What P2 does **not** specify is retry, backoff, or a user-facing message; those belong to whichever phase owns resource bounds. No claim either way |
| 12 | **`wal_checkpoint` returning `SQLITE_BUSY`** | **DIRECTLY TESTED** | Measured: `TRUNCATE` with a concurrent reader returns `busy = 1` having checkpointed 3 of 4 frames; `PASSIVE` returns `busy = 0` and does what it can. So `busy` must be **read and checked**, not discarded, and `TRUNCATE` must not be assumed to complete. §7.2's close path depends on this |
| 13 | **`Store::close` while another connection exists** | **SQLITE GUARANTEE** + **DIRECTLY TESTED** | Upstream: the last connection takes a brief exclusive lock while it cleans up the WAL and shared-memory files, so a concurrent opener may get `SQLITE_BUSY`; and a connection recovering after a crash holds an exclusive lock, so a third may get `SQLITE_BUSY`. `busy_timeout = 5000` absorbs the ordinary case. Measured: two independent connections on one file both succeed and all writes land |
| 14 | **Two OS processes writing one file** | **DIRECTLY TESTED** (N-group) | Measured by the earlier audit: 40 of 40 writes landed, `quick_check` ok, `busy_timeout` serialising correctly. SQLite permits exactly one writer at a time and the pragma is the mechanism |
| 15 | **Deterministic mid-`COMMIT` abort** | **IMPOSSEIBLE through `rusqlite`** — already recorded | Ledger 6.1. Every injection point was worked through; it needs a custom SQLite build or a fault VFS. Not deferred, **not attempted** |
| 16 | **A stale `-shm` from another machine** | **SQLITE behaviour, not relied upon** | Measured: a mismatched `-shm` caused no observable damage. But §7.5's rule stands — never carry it, because upstream permits the wal-index to be architecture-specific. Not tested, because the rule is "do not copy it", which needs no test |
| 17 | **Recovery runs on a store whose `foreign_key_check` fails** | **DIRECTLY TESTED** | `RefusedPass`. §9.1 row 3b is undecidable without it, and it is the only pragma that sees a referential violation (§7.1) |

**Two rows are honest non-claims rather than gaps.** Case 11 (disk full) has a
correct *data-integrity* behaviour by case 1 and an unspecified *operational*
behaviour; §12's resource-bound row already records that bounds are not P2's.
Case 5's checksum check detects a mismatched binary, **not** a tampered file — a
local file writer can alter the checksum, and the threat model already excludes
that writer from tamper-evidence.

**Nothing in this table changed the design.** It is recorded so that no case is
first discovered during P2C or P2H.

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

The ceiling is checked at acquisition, in three statements inside one transaction, so
each has exactly one possible cause:

1. The `leases` upsert. Zero rows ⇒ `LeaseHeld`.
2. The fenced `UPDATE … WHERE lease_generation = :expected`. Zero rows ⇒
   `LeaseFenced` — someone else holds it, or it moved on.
3. A ceiling check reading `max_attempts_per_step` from `tasks` **in the same
   transaction**. Over the ceiling ⇒ `AttemptCeilingReached`, and the transaction
   **rolls back**.

Reading the bound from durable state at the check is
[Bounds Protocol §2.1](../protocols/10-bounds-protocol.md#21-where-these-live-in-durable-state)
and is why the fence and the ceiling are separate statements: a single combined
`WHERE` would make "fenced" and "at the ceiling" indistinguishable, and a caller
that cannot tell them apart cannot report them.

**What the rollback leaves behind is not always a `PLANNED` step**, and the
flattering version was the one written first. Verified by execution:

| Case | After rollback |
| --- | --- |
| First acquisition against `max_attempts_per_step = 0` | Step left `PLANNED`, `attempt = 0`, `lease_generation = 0`, and **no `leases` row** — which is what "the step is left `PLANNED` and the lease released" describes |
| Third acquisition against a ceiling of 2 | Step reverts to its **prior committed** state: `LEASED`, `attempt = 2`, with exactly one `leases` row at `generation = 2`. The refused acquisition leaves no trace |

**An expiry reclaim spends an attempt, so the bound is on acquisitions, not
executions.** ADR-0024 increments `attempt` on every acquisition including a
reclaim, and the ceiling is `attempt > max_attempts_per_step`. The two compose into
a consequence ADR-0024 now states and this audit measured: a worker that acquires
and then dies before `begin_attempt` has still spent one attempt, so a host that
crashes *N* times has an effective execution budget of
`max_attempts_per_step − crashes`. Measured against a ceiling of 2, with every
acquisition standing in for a crash: **2 acquisitions refused, 0 executions**, and
the step ends at `('LEASED', 2, 2)`.

This is correct — counting a crash is the only way `attempt` can "distinguish the
crash-recovered attempt from a deliberate retry", which is Task Protocol §3.1's
stated purpose. It is recorded because an implementer reading this section would
otherwise conclude that `max_attempts_per_step = 3` buys three executions.

**The recovery consequence is named.** When the ceiling is reached by crashes
rather than by failures, the outcome is **not** `FAILED`, because nothing was
proven to have failed. Such a step is `NeedsReconciliation` with `attempt` at the
ceiling, and its task moves `BLOCKED` with an invariant-violation reason.

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
| 4 | Can `SECRET` or `CREDENTIAL` enter ordinary SQLite? | No, by any writer that leaves constraint checking enabled: `data_class_rank BETWEEN 0 AND 2` on all seven classified tables makes those rows unconstructible, verified on each. **The boundary, stated once:** `PRAGMA ignore_check_constraints = ON` disables every `CHECK` in the schema for a local file writer, and P2 does not mitigate that. Every trigger and foreign key still holds under it — which is where this design spends its structural budget, and **the P2 autonomous audit verified that claim rather than assuming it**. **The second boundary is `PRAGMA foreign_keys = OFF`**, which the earlier revision of this answer did not name: it defaults to `OFF` in SQLite, one line disables it, and a `task_steps` row referencing a non-existent task is then accepted. So the composite-key anti-laundering guarantee in [schema §5.3](P2-sqlite-schema.md#53-classification-and-laundering) is *structural* against a writer who leaves enforcement on and *pragma-dependent* against a local file writer — which `TB-7` already excludes from tamper-evidence. ADR-0022, [schema §7](P2-sqlite-schema.md#the-pragma-boundary-in-full), tests O14/O15 |
| 5 | Can recovery turn ambiguity into a second effect? | No. P2 recovery never executes. `NeedsReconciliation` records the decision durably for P5. Task Protocol §6.2 |
| 6 | Can a state transition occur without the audit seam? | Every transition writes a `task_journal` row through the same `Tx`, as a `TransactionParticipant` receiving the same `DurableTransition` every other participant receives — so no participant can record a different transition from any other. `E3` itself is **not claimed**, and the P2 autonomous audit established it is **not retroactively claimable**: it holds forward from P3's first migration and never held for P2-era transitions. No event is reconstructed. ADR-0021 |
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
| **§4.6 still published the removed `leases_generation_matches_step` trigger**, which makes the *first* lease acquisition abort | audit, **blocker** | Trigger and its "verified" claim deleted; §4.6 now explains why publishing one reintroduced the round-2 defect |
| **8 of ADR-0018 §3's 32 `N`/`0` presence cells were accepted by SQL** | audit, major | Three additive constraints; all 32 cells now refused, all 51 constructible cells still construct |
| **ADR-0023's category-O pattern was inert** — it required a colon before the prefix, so every frozen identifier was accepted | audit, **blocker** | Rule decided by measurement (C: 16/16 caught, 0/14 false positives); pattern regenerated from the frozen prefix and verb lists |
| **`open_in_memory` cannot be WAL**, so §7.2's single table was unimplementable | audit, major | Two named profiles; a positive list of which tests must be file-backed |
| **`foreign_key_check` was in no tier**, and `quick_check`/`integrity_check` both report `ok` on an FK orphan | audit, major | Four verification tiers, `foreign_key_check` added to post-migration and recovery |
| **The composite-key anti-laundering guarantee is pragma-dependent** and §5.3 did not say so | audit, major | Stated the way ADR-0022 states the `ignore_check_constraints` boundary |
| **ADR-0021 adopted the `pending_event` outbox it had rejected**, and promised `E3` retroactively | audit, major | `E3` is forward-only; no event reconstruction; `event_seq` dropped |
| **`CommitHook::append(&mut self, tx)` needs hidden state and cannot be driven from `transact(&self)`** | audit, major | `DurableTransition` parameter + `TransactionParticipant`; registry rejected |
| **An expiry reclaim spends an attempt**, so crashes exhaust the budget with zero executions | audit, major | Arithmetic stated in ADR-0024 and §10.4; the recovery outcome named |
| **Three ADRs took three positions on the version treatment**, and ADR-0019's was wrong | audit, major | One plan: `serea-arch/1.0.0`, `serea.task/2`, `serea.action/2` |
| **ADR-0019 rejected shortest-round-trip floats for a false reason** | audit, major | RFC 8785 mandates ECMAScript `Number::toString`; the real obstacle is UTF-16 vs UTF-8 key ordering |
| **`rusqlite`'s bundled path requires Rust 1.88**; the workspace pins 1.85; two crate defaults are wrong | audit, minor | **The premise was wrong and is corrected by the final closure run.** No `libsqlite3-sys` MSRV exists and `cargo +1.85.0` builds and runs the chosen configuration. Minimal feature set kept; **there is no MSRV decision** — see §7.4 |
| **A counter-derived `TempStore` name collides across test binaries** | audit, minor | `<binary>-<pid>-<atomic-counter>`, plus an inherited-directory variant for crash children |
| **ADR-0024's rejected alternatives still argued for the removed `token`** | audit, minor | Both rows removed; the rationale lives where the decision is made |
| **ADR-0024's commit statement mixed `:named` and `?` placeholders** | audit, minor | All placeholders named; `rusqlite` binds one style per call |
| **`task_steps.plan_revision` had no `DEFAULT`** while every sibling did | audit, minor | `DEFAULT 0` added |
| **SCJ-1 vector 8's input omitted the `\u007f` it claimed to pin** | audit, minor | **Input corrected; hash unchanged.** 9 of 10 SCJ-1 and 7 of 7 IDK-1 vectors recomputed and reproduce |
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

### 13.5 Positive-constructibility verification record

The earlier audit found that **testing only refusals is not enough**: a schema can
refuse everything and pass. So the positive direction is asserted as its own
matrix, and re-verified by execution during the final closure run.

**Method.** The migration DDL was extracted **verbatim** from
[schema §4.0](P2-sqlite-schema.md#40-the-whole-migration) and built against real
SQLite 3.53.2 — not retyped, and not asserted from the document. The only thing
written by hand was the *data* each positive case needs. **69 of 69 checks passed.**

| Group | What was constructed | Result |
| --- | --- | --- |
| Inventory | `sqlite_master` after migration | **10 tables, 7 triggers, 6 explicit indexes** — exactly as asserted |
| Task transitions | All **37** legal `TaskState` pairs from §10.2's frozen table | **37/37 construct.** The 4 leaving `BLOCKED` construct only when `blocked_reason` is cleared in the same statement |
| Step cells | All 8 `kind` × 7 `status` combinations | **51 constructible, 5 correctly refused** (`WAITING` on each of the 5 non-wait kinds), 0 wrongly accepted, 0 intended cells refused |
| Leases | acquire; renew inside expiry; stale-generation refusal; expiry reclaim (`generation` 1→2, `attempt` 1→2); release; `attempt` against `max_attempts_per_step` | **all construct**, and the two *refusal* behaviours hold: a stale generation touches 0 rows, and the refused commit inserts no receipt |
| Migrations | fresh; `0001_initial` applied; `user_version` transactional; rollback leaves no row; duplicate ID refused; malformed checksum refused; newer-schema signal; `NotSereaStore` detectable | **all construct**, all three refusals hold |
| Recovery | `ExpiredLease`; `ReconciledAbsent`; `LeaseFenced` (and no receipt written); `ResumeNormally`; ordered journal | **every designed output reachable** |
| Open profiles | file-backed store under `synchronous=FULL` **and** `synchronous=NORMAL` | `journal_mode=wal`, `foreign_keys=1`, `quick_check=ok`, **0 `foreign_key_check` violations**, `busy_timeout=5000`, `wal_autocheckpoint=1000`; reopen preserves WAL mode and all 10 tables |

**Three design facts this confirmed by execution, none of which was obvious from
reading the DDL.** They are the reason the matrix is worth running:

1. **A `WAITING` step carries no lease columns at all.** The biconditional
   `(status IN ('LEASED','EXECUTING')) = (lease_owner IS NOT NULL)` ties the lease
   to the executing pair, so a step waiting on a user or an approval holds no
   lease. Reading only ADR-0018's presence matrix does not make this plain.
2. **The step-commit statement must clear `lease_owner` and `lease_expires_at_ms`
   together.** Setting `status='SUCCEEDED'` while leaving either behind is refused
   by the biconditional — which is exactly ledger 5.4's claim, now verified rather
   than asserted.
3. **Leaving `BLOCKED`, `FAILED` or `CANCELLED` requires clearing the previous
   state's reason columns in the same `UPDATE`.** §4's `cancel task` row already
   says this for `blocked_reason`; the measurement shows it is true for all three
   columns, and that a row-level `UPDATE tasks SET state=…` that forgets them
   aborts instead of silently leaving inconsistent state.

**What this does not establish.** It verifies the *schema* admits and refuses the
right things. It does not verify the Rust API, which does not exist, and it does not
substitute for the named tests in [the test matrix](P2-test-matrix.md) — those assert
behaviour, this asserts constructibility.

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

### 15.0 Ordering *inside* the single atomic P2A commit

P2A is one commit carrying the whole contract change: the Rust types, the JSON
Schema, the tests, the version numbers, the changelog entries and both migration
notes. It cannot be split, because a `serea.task/2` schema with `serea.task/1`
Rust types — or a Rust type that deserialises a document its own schema rejects — is
a state no other phase should ever observe, and the intermediate commits would not
build.

So the question is not *whether* to interleave but **how to interleave so the RED
failures stay meaningful**.

| Order | Sequence | Verdict |
| --- | --- | --- |
| **A** | Rust `StepPresence` → schema → docs/version | **Rejected.** The Rust types compile and their tests pass while the published schema still says `started_at` is required. For the whole of that window the *authoritative* wire artifact is wrong and nothing fails. Worse, `serde(try_from = Draft)` makes the Rust side permissive, so a `PLANNED` step round-trips in Rust and is rejected by the frozen schema — a green build over a broken contract |
| **B** | schema → Rust → docs/version | **Rejected, and worse than A.** The schema lands first and immediately starts rejecting documents the Rust type still emits. The RED failure is now a *runtime* schema-validation failure in P1's existing tests rather than a compile error, so the signal is real but the diagnosis is misleading — it looks like a data bug, not a type-contract change |
| **C** | tests for **both** surfaces first → Rust + schema together → protocol/version/changelog/migration notes | **Adopted** |

**Why C is the only safe order.**

1. **Both REDs are written before either GREEN.** One test against the Rust
   `StepPresence` matrix, and one against the JSON Schema's `if`/`then` clauses,
   over the *same* cell list. Both fail, and both fail for the right reason —
   `started_at` is not `Option` in Rust, and the schema has no conditional
   requirement. Two independent REDs for one contract change is the only way to
   know the change is genuinely two-sided.
2. **Then Rust and schema land in the same commit, driven by those tests.** Because
   both were RED first, both are now GREEN for a reason that was observed failing.
   Neither surface is ever briefly green while the other is wrong — they become
   correct in the same commit, so the window does not exist.
3. **Then the protocol text, version numbers, changelog entries and both migration
   notes.** These are last because they are *descriptions* of the change rather
   than the change. Writing them first would document an interface that does not
   exist yet, and the earlier audit's own lesson applies: a constant that is
   asserted is not a constant that is verified.

**The exact suggested sequence, as commits-worth-of-work inside the one commit:**

| Step | Action | Gate before moving on |
| --- | --- | --- |
| 1 | Write the **Rust** matrix test for one `PLANNED` cell | **RED**, failing on the type shape |
| 2 | Write the **schema** test for the same cell | **RED**, failing on the missing `if`/`then` |
| 3 | `StepPresence` + the five `Option` fields + `serde(try_from = Draft)` | step 1 GREEN |
| 4 | The schema's per-kind `required` / `if`–`then` clauses, incl. the category-O pattern and the `goallatch` subtraction | step 2 GREEN |
| 5 | The `lease_generation` optional field (ADR-0024), both surfaces | both GREEN |
| 6 | ADR-0023's three text categories + validators, both surfaces | both GREEN |
| 7 | **Parity check**: every cell in the Rust `StepPresence` matrix has a schema counterpart and vice versa | the parity test passes |
| 8 | `serea-arch/0.2.0 → 1.0.0`; `serea.task/1 → 2`; `serea.action/1 → 2` | — |
| 9 | Changelog entry in each affected protocol document | Protocol Index §7 items 2 and 3 |
| 10 | Both migration notes (`serea.task/2`, `serea.action/2`) | Protocol Index §7 item 4 |
| 11 | Whole workspace green; **one** commit | §14's P2A exit criteria |

Step 7 is the one that is easy to skip and the one that matters most, because it is
the only step that would catch one surface being updated and the other missed. It
is the test that makes the "atomic" requirement enforceable rather than aspirational.

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
