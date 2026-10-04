# ADR-0024: Lease Fencing and Commit-Under-Lease

- Status: **Proposed** — P2A wire generation and P2E acquisition/reclaim/renew/release/attempt accounting implemented; begin/outcome atomic fencing remains P2F
- Architecture version: `serea-arch/0.2.0` at the time of writing
- Decision date: not yet ratified
- Recorded by: P2 design preparation, from `c3737039e3e38dbba554dc0b9075025f87948358`
- Feeds: [P2 contract gap analysis](../plans/P2-contract-gap-analysis.md) §5.10

> P2A implements only the wire `lease_generation` member and validation;
> this does not implement lease acquisition, revocation or outcome fencing.
> Field-local `serde_json/raw_value` decoding checks positive-u32 membership
> exactly without f64 rounding, retaining integral decimal/exponent spellings.
> Precision-enabled schema validation uses the narrow patched dependency described
> in [launch §3](../plans/P2-6.1-sol-launch.md#3-dependency-lines-current-p2a-integration-and-p2c-candidate).
> [P2E's frozen gate E1–E11](../plans/P2E-review-and-closure.md#1-preflight-and-frozen-pre-implementation-gate)
> selects acquisition/renewal/release, ceilings, overflow and caught-error atomicity
> only. Its closure record contains actual implementation, independent review,
> remediation and stable/MSRV/debug/release validation evidence. Lease-authority
> closure is not outcome proof.
> P2F owns `begin_attempt` and embedded outcome UPDATE fences. This ADR remains
> Proposed even after lease-only GREEN. Historical SQL/docs probes below are
> constructibility evidence, not current production runtime proof. P2A validation
> remains recorded separately in its [closure record](../plans/P2A-review-and-closure.md).

## Context

Task Protocol §3.1 requires `lease_owner` and `lease_expires_at` and says they
"Prevent two workers executing one step concurrently". That is a
**mutual-exclusion** claim. It is not a **fencing** claim, and the difference is
exactly the failure:

1. Worker A acquires the lease, generation *n*.
2. A stalls past `lease_expires_at` — a scheduler pause, a machine suspend, a
   long GC, a stopped debugger.
3. Worker B sees the lease expired, reclaims it, and starts the step.
4. A wakes up and commits its success.

With only `lease_owner` and `lease_expires_at`, step 4 either succeeds — one
external effect, two writers, and B's result silently lost — or fails A's write
with no way to distinguish it from a genuine constraint violation. Neither is
acceptable, and the first is the dangerous one.

The frozen architecture already has the right mechanism, for the *scheduler* lease.
[Scheduler Protocol §4](../protocols/11-scheduler-protocol.md#4-lease-and-concurrency):
"The lease has a host-configured bounded duration and a **monotonically changing
lease owner token** stored with the occurrence record. **Lease expiry permits
reclamation, not assumption that work did not happen.**"

P2 applies that same mechanism to task steps. This is applying a frozen precedent,
not inventing one — which is the strongest available answer to "is this
necessary?", because the answer is already written down elsewhere in the
architecture.

## Decision

### Durable shape

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

- `generation` starts at 1 and increments on **every** acquisition, including an
  expiry reclaim. It never resets and never decreases.
- **The `token` column was removed.** An earlier draft carried a 16-byte unguessable
  `token` alongside `generation`, and simultaneously conceded in the same paragraph
  that `generation` alone already discriminates two acquisitions by the same owner
  across a reclaim. The two justifications offered for it — distinguishing a same-owner
  reacquisition, and preventing a caller from reconstructing a guard from fields it
  read — are both answered by `generation` and `owner`, which the caller would have
  to read anyway. A second value that must be kept consistent with the first and that
  no statement needed is overengineering, and it would have left a `DC7`-shaped
  "never render this" rule defending a value with nothing to defend.

### `LeaseGuard` is the only way to reach a commit

```rust
pub struct LeaseGuard {
    task_id: TaskId,
    step_id: StepId,
    owner: LeaseOwner,
    generation: NonZeroU32,
    origin: Arc<AtomicBool>,
}
```

`LeaseGuard` has private fields, no public constructor, and no `Clone`, `Copy`
or Serde support. It is produced only on successful acquisition. There is no
owner formatter and **no Drop release or other Drop side effect**. SQLite, not
possession of this value, authorizes each write. An outer transaction rollback
invalidates the purported authority of a guard returned inside that transaction.
A private per-transaction commit marker is published only after successful outer
commit. A pending guard is usable only in its origin Tx; after rollback, panic or
commit error it stays unpublished permanently, even if a later same-owner acquire
reuses that uncommitted generation. This marker only rejects invalid capabilities;
it never grants authority or caches a lease decision. SQL remains authoritative,
including when a committed guard is used through a different Store. It introduces
no durable token, lease map, global mutex or schema change.

P2E renewal borrows `&LeaseGuard`; release consumes it **on every result**, even
an infrastructure error. P2F `begin_attempt` borrows it and outcome methods consume
it. Nonclone shape prevents accidental duplication but does not replace SQL fences.

### Implemented P2E API (lease authority only)

```rust,ignore
impl Tx<'_> {
    pub fn acquire_lease(
        &mut self,
        task_id: TaskId,
        step_id: StepId,
        owner: LeaseOwner,
        expected_generation: Option<u32>,
        now: EpochMillis,
        expires_at: EpochMillis,
    ) -> Result<LeaseGuard, StoreError>;
    pub fn renew_lease(&mut self, guard: &LeaseGuard, now: EpochMillis,
                       new_expiry: EpochMillis) -> Result<(), StoreError>;
    pub fn release_lease(&mut self, guard: LeaseGuard, now: EpochMillis)
        -> Result<(), StoreError>;
}
```

These are explicit **absolute `EpochMillis` instants**, not TTLs; storage retains
no Clock. Core owns `max_lease_seconds` and supplies policy-bounded instants.
`expected_generation` is the caller's observation: `None` binds SQL 0 (never
leased), `Some(n)` requires exact positive generation equality, and `Some(0)`
refuses as `LeaseFenced`. Never replace that expectation with a fresh SQL read.

### The five semantics

**acquire (P2E).** Inside the outer `BEGIN IMMEDIATE`, an **internal SQLite
savepoint encloses the complete acquisition**: classification reads, the ordered
write pair, durable ceiling check and all later failures. Require `expires_at >
now`, otherwise `InvalidLeaseInterval`. Read task/step/lease authority and
`tasks.max_attempts_per_step` in this same savepoint, with no inter-writer window.

For valid intervals and generation inputs (`None` or positive `Some`), wrong
task binding, a missing step, or a status outside `PLANNED`/`LEASED`/`EXECUTING`
is `LeaseFenced`. On an otherwise bound eligible step, classify in order: active
unreleased authority => `LeaseHeld`; stale caller expectation => `LeaseFenced`;
exhausted durable budget =>
`AttemptCeilingReached`; otherwise max-u32 authoritative generation => payload-free
`LeaseGenerationOverflow`. Classify overflow **before any mutation**, not by a
CHECK exception or SQLite message/string matching. Preserve the existing schema.

The two writes remain in this order. Historical probes exercised an earlier pair;
they did not prove the selected savepoint/refusal contract.

```sql
-- 1. the leases row is the authority on generation
INSERT INTO leases (step_id, owner, generation, acquired_at_ms, expires_at_ms)
VALUES (:step_id, :owner, 1, :now_ms, :expires_at_ms)
ON CONFLICT(step_id) DO UPDATE SET
    owner          = excluded.owner,
    generation     = leases.generation + 1,
    acquired_at_ms = excluded.acquired_at_ms,
    expires_at_ms  = excluded.expires_at_ms,
    released_at_ms = NULL
WHERE (leases.released_at_ms IS NOT NULL
       OR leases.expires_at_ms <= :now_ms)
  AND leases.generation < 4294967295
```

The bounded increment predicate is a SQL defense in addition to pre-mutation
overflow classification. Do not equate all zero-row refusals with `LeaseHeld`:
classify under the same locked snapshot using the precedence above. Never wrap,
clamp, parse SQLite strings or weaken the bounded-u32 CHECKs; migration 0001 is
unchanged and no 0002 is authorized.

```sql
-- 2. the step copy is DERIVED from the leases row, never guessed
UPDATE task_steps
   SET status            = 'LEASED',
       lease_owner       = :owner,
       lease_expires_at_ms = :expires_at_ms,
       lease_generation  = (SELECT generation FROM leases WHERE step_id = :step_id),
       attempt           = attempt + 1,
       started_at_ms     = NULL,
       result_digest     = NULL,
       completed_at_ms   = NULL
 WHERE step_id    = :step_id
   AND task_id    = :task_id
   AND status     IN ('PLANNED', 'LEASED', 'EXECUTING')
   AND lease_generation = :expected_generation_sql
```

`:expected_generation_sql` is exactly the public Option mapping above, not an
internal newly observed value. A zero-row step UPDATE is `LeaseFenced`. Any
refusal or later SQL failure explicitly rolls back and releases the savepoint,
restoring pre-call state even if the caller catches `Err` and returns `Ok` from
`Store::transact`. This must not depend on propagating `?`. If rollback/release
cleanup fails, mark the outer Tx **rollback-only** and prohibit its commit, even
when its body returns `Ok`.

Three consequences worth stating, each of which was a defect first:

- **`attempt` increments here and only here.** `begin_attempt` moves
  `LEASED → EXECUTING` and stamps `started_at_ms`; it does not touch `attempt`. An
  earlier draft incremented in both, so `max_attempts_per_step = 3` bought one
  attempt. This is what Task Protocol §3.1 means by "distinguishes the
  crash-recovered attempt from a deliberate retry".
- **`status IN ('PLANNED','LEASED','EXECUTING')`, not `'PLANNED'` only.** A step
  whose lease was released or expired is still `LEASED`/`EXECUTING`, so a
  `PLANNED`-only predicate made retries, expiry reclaim, and
  [Task Protocol §6](../protocols/02-task-protocol.md#6-recovery)
  structurally unreachable.
- **The two generation copies are consistent by construction, not by a trigger.**
  `lease_generation` is read *from* the `leases` row in the same transaction, so
  they cannot diverge observably. The `leases_generation_matches_step` trigger an
  earlier draft specified was removed: it would have to model the upsert's
  insert-or-update branch, and it fired only on the insert branch, so it enforced
  the invariant in one of the two cases and silently skipped the other.

In SQL, `PLANNED` keeps `attempt = 0` and `lease_generation = 0`; wire generation
is None (missing/null accepted and serialization omitted), never wire 0. Positive
SQL generation must fit u32; acquisition at u32::MAX refuses and rolls back, never
wraps or truncates. This is what makes PLANNED mean "never leased". A step that has begun an attempt is never `PLANNED` again.

**renew (P2E).** SQLite must authorize the guard's task/step binding, owner,
generation and unreleased authority. Missing/stale/released authority returns
`LeaseFenced` first; a matching unreleased row with expiry `<= now` returns
`LeaseExpired`. Only then require `new_expiry > authoritative old expires_at_ms`:
equal/shorter expiry returns `InvalidLeaseInterval` without mutation. Merely being
later than `now` is insufficient.

```sql
UPDATE leases SET expires_at_ms = :new_expiry
WHERE step_id = :step_id AND owner = :owner AND generation = :generation
  AND released_at_ms IS NULL AND expires_at_ms > :now_ms
  AND :new_expiry > expires_at_ms
  AND EXISTS (SELECT 1 FROM task_steps AS s
              WHERE s.step_id = leases.step_id AND s.task_id = :task_id)
```

Classify zero rows under the same `BEGIN IMMEDIATE`, not uniformly as
`LeaseExpired`. An expired holder cannot resurrect its own fence; it must
reacquire at a newer generation. Renewal changes **only authoritative
`leases.expires_at_ms`**. All step lease columns remain unchanged: the step expiry
is an acquisition snapshot, not current authority.

**release (P2E).** Changes **only `leases.released_at_ms`**, revoking rather
than transitioning the step. It permits matching expired leases. Stale/missing/
released authority is `LeaseFenced`; for matching authority, `now < acquired_at_ms`
returns `InvalidLeaseInterval` without mutation, not a generic CHECK failure.

```sql
UPDATE leases SET released_at_ms = :now_ms
WHERE step_id = :step_id AND owner = :owner AND generation = :generation
  AND released_at_ms IS NULL AND acquired_at_ms <= :now_ms
  AND EXISTS (SELECT 1 FROM task_steps AS s
              WHERE s.step_id = leases.step_id AND s.task_id = :task_id)
```

The step copy deliberately remains unchanged (including status, owner, generation
and acquisition-snapshot expiry). Successful release permanently revokes that
generation; every later begin/renew/outcome must consult authority rather than
trusting the copy. Reacquisition obtains a strictly newer generation.

The guard is consumed **on every result**, including validation, fencing and
infrastructure errors. `Err` is **not evidence of durable release**; likewise an
operation's `Ok` inside a transaction is not durable until the outer commit.
Safety takes priority over retryability: after a consuming error, eventual
expiry/recovery is required, not retry with that guard. Dropping a guard performs
no release.

**Expiry policy.** Expiry permits reclaim, not proof that work never occurred.
An expired but unreclaimed and unreleased lease may commit a known result under
its current fence. It may not renew or begin a new attempt (begin requires
`expires_at_ms > :now_ms`). Reclaim changes authoritative generation and refuses
the old outcome. Commit deliberately does not require unexpired TTL; this policy
must be identical in implementation and tests. No racy application precheck.

**commit-under-lease (P2F, not P2E).** Every outcome UPDATE embeds the fence in
its `WHERE` clause; no P2E fake commit API or precheck stands in for this gate:

```sql
UPDATE task_steps
   SET status = 'SUCCEEDED', result_digest = :digest, completed_at_ms = :now_ms,
       lease_owner = NULL, lease_expires_at_ms = NULL
 WHERE step_id = :step_id AND task_id = :task_id AND status = 'EXECUTING'
   AND lease_generation = :generation AND lease_owner = :owner
   AND EXISTS (SELECT 1 FROM leases AS l
               WHERE l.step_id = task_steps.step_id
                 AND l.generation = :generation AND l.owner = :owner
                 AND l.released_at_ms IS NULL)
```

Every placeholder is **named**. An earlier revision of this statement mixed
`:digest`, `:now_ms`, `:generation` and `:owner` with four positional `?`, which
`rusqlite` cannot bind in one call. Historically only the
placeholders were normalized; the P2A reconciliation additionally strengthens the
predicate with the authoritative unreleased-lease EXISTS check, and they are named rather than positional
deliberately so the next reader does not "simplify" the statement back.

Three corrections went into this statement, and each was a real defect caught by
historical schema probes rather than reasoning about it (not current runtime proof):

1. **No `receipt_id` column.** An earlier draft set one. `task_steps` deliberately
   has no such column — the receipt is reached by `side_effect_receipts.step_id`,
   `UNIQUE`, which avoids a circular foreign key — so the statement could not run at
   all. The receipt insert is a separate statement in the same transaction.
2. **The lease columns are cleared.** A terminal write leaves `lease_owner`
   populated, which the schema's biconditional `(status IN
   ('LEASED','EXECUTING')) = (lease_owner IS NOT NULL)` refuses. Clearing them is
   also the correct semantics: the lease ends with the attempt.
3. **`status` is literal, not a parameter.** Only `commit_step_succeeded` has this
   shape; the failed and reconciled-absent commits set their own literals. A
   `:status` parameter would let a caller write a terminal status without clearing
   the lease.

Zero affected rows is `StoreError::LeaseFenced`. Worker A at generation 3 cannot
write while B holds generation 4 or after generation 3 is released: the predicate
cannot match, **atomically in the database**, not by a racy prior read. Historical
probes covered stale-generation refusal, not the added authoritative-release
predicate. A historical docs probe checked that predicate separately; **P2F**
runtime integration and H9–H13 rollback/receipt/journal/task assertions remain
required. P2E authority-only GREEN cannot establish them.

**expired reclaim.** Covered by `acquire`'s `expires_at_ms <= :now_ms` branch,
with `generation + 1`. Reclaiming is therefore always a strictly newer
generation, which is what makes the old holder's writes unrepresentable rather
than merely discouraged.

### An expiry reclaim spends an attempt, and that arithmetic is stated here

`attempt` increments on **every successful** acquisition, and its resulting
value must not exceed durable `max_attempts_per_step`. Before increment, an
exhausted budget (`attempt >= max_attempts_per_step`) is `AttemptCeilingReached`;
a refused acquisition spends nothing. P2E owns this rule; P2F owns the proof that
begin does not increment again. The historical autonomous audit illustrated the
arithmetic, not current runtime implementation:

> **`max_attempts_per_step` bounds acquisitions, not executions.** A worker that
> acquires a lease and then dies before `begin_attempt` has still spent one
> attempt. A host that crashes *N* times before beginning an attempt has spent *N*
> of its budget, so the effective execution budget is
> `max_attempts_per_step − crashes`.

Historical SQL measurement against `max_attempts_per_step = 2`, with each
acquisition standing in for a worker that acquired and then died:

```
acquisitions before the ceiling refused .... 2
of which NONE ever called begin_attempt ..... 2
final step row ............................. ('LEASED', 2, 2)
```

This is *correct* and is not changed here. Task Protocol §3.1 says `attempt`
exists to "distinguish the crash-recovered attempt from a deliberate retry", and
counting a crash is the only way that distinction is visible at all. It was
recorded here because it is the kind of operational consequence an implementer
otherwise discovers in production, having reasonably read §"acquire" and
concluded that `max_attempts_per_step = 3` buys three executions.

**The recovery consequence is named too.** When the ceiling is reached by
crashes rather than by failures, the correct outcome is **not** `FAILED`: nothing
was proven to have failed. Such a step is `NeedsReconciliation` with `attempt` at
the ceiling, and the task moves `BLOCKED` with an invariant-violation reason.

### What the ceiling refusal actually leaves behind

The selected P2E gate reads `max_attempts_per_step` from `tasks` inside the
complete acquisition savepoint, not from cached caller state. Exhaustion refuses
before mutation; any later failure explicitly rolls back/releases the savepoint.
Cleanup failure makes the outer Tx rollback-only. Thus a caught error cannot
commit a partial lease upsert or attempt increment. The historical post-increment
third-statement sketch is not the selected classification algorithm.

Refusal restores the *pre-call state* (the prior committed state in these
historical fixtures), which is not always `PLANNED`:

| Case | Result after rollback |
| --- | --- |
| First acquisition against `max_attempts_per_step = 0` | Step left `PLANNED`, `attempt = 0`, `lease_generation = 0`, and **no `leases` row exists**. No lease was acquired or durably released |
| Third acquisition against a ceiling of 2 | Step reverts to its prior committed state — `LEASED`, `attempt = 2` — and exactly one `leases` row remains, at `generation = 2`. The third acquisition left no trace |

Both were historical SQL probe results, not current P2E caught-error/savepoint
proof. Production must also cover callers that catch the error and commit
unrelated outer writes, plus cleanup failure that must prevent outer commit.

### Ordering inside a commit transaction

This is a **P2F** whole-outcome transaction requirement. The fenced write is the
**first mutation**, and the `rows_affected == 0` check is
**explicit**. SQLite does not roll back on a zero-row `UPDATE`, so a fenced commit
would otherwise proceed to insert a receipt and a journal row before anything
noticed. The engine therefore:

1. runs the fenced `UPDATE task_steps` and returns `LeaseFenced` on zero rows;
2. inserts the blob and the receipt row;
3. updates `tasks` with its own expected-state predicate;
4. returns immutable successful transition(s) to shared-receiver participants,
   which append the journal in this same transaction (ADR-0021);
5. commits.

### No process-local mutex participates

The connection mutex is not a lease-authority registry. Authority comes from
durable SQLite rows and atomic SQL predicates; classification reads stay under
`BEGIN IMMEDIATE`/savepoint so another writer cannot intervene. In-process
classification of those SQL facts is not an independent source of authority.
The required lock is SQLite's writer serialization, not a process-local lease
mutex. Separate file-backed Stores prove shared authority in P2E's production tests;
historical SQL probes are not current runtime proof. These tests do not claim
child-process crash behavior or outcome fencing.

### What P2 does not implement

- **Renewal scheduling.** `renew` is a method; nothing calls it on a timer in P2.
  There is no runtime in P2.
- **Lease-driven task admission.** `max_concurrent_tasks` and
  `max_concurrent_steps_per_task` are §2 bounds; Crate Map §3.1 assigns bound
  configuration and global counters to `serea-core`. P2 reads
  `max_attempts_per_step` from durable state because it owns the step attempt;
  it enforces no other bound.
- **`max_lease_seconds`.** Core's `BoundConfig` owns duration policy; the caller
  supplies absolute `EpochMillis` now/expiry to storage. P2 enforces interval
  validity and the lease mechanism, not a numeric duration bound.

## Proposed amendment

P2A implements only the optional wire member and its validation. The lease table
is P2C foundation; acquisition/renewal/release/ceiling/overflow is implemented in
P2E; `begin_attempt` and embedded outcome fencing are P2F with runtime tests, not
P2A. P2E lease-only closure cannot accept this full ADR; it remains Proposed.

The implemented Rust wire field remains `Option<u32>`. Its decoder accepts
JSON integer-valued numeric spellings such as `1`, `1.0` and `1e0` within the
positive u32 domain; missing/null yield None, while zero, fractions and overflow
refuse. This schema-integer wire domain is separate from SCJ-1, which refuses
decimal/exponent spellings even if integer-valued. Checked `TaskStep` uses manual
draft deserialization with sanitized errors before `StepPresence::new`, not a
`serde(try_from = ...)` deserialization attribute.

1. §3.1's table gains a `lease_generation` row: *"A monotonically increasing
   per-step counter, incremented on every lease acquisition including an expiry
   reclaim. A commit carrying a generation lower than the stored one is refused.
   Absent on a step that has never been leased."*
2. §5 rule 3 gains: *"Lease expiry permits reclamation, never assumption that work
   did not happen. Every commit under a lease is conditional on the lease
   generation, so a holder whose lease was reclaimed cannot write."* — mirroring
   Scheduler Protocol §4's existing sentence for the scheduler lease.
3. A changelog section.

`Task Protocol` §3.1's existing `lease_owner` / `lease_expires_at` rows are
unchanged.

## Phase-specific implementation gates

| Phase gate | File | Change |
| --- | --- | --- |
| P2A wire only | `crates/serea-protocol/src/types.rs`, task schema and wire tests | `TaskStep.lease_generation: Option<u32>`, None omission, positive-u32 validation; not full ADR acceptance |
| P2B | `crates/serea-testkit/src/clock.rs` | `TestClock` gains `Clock` and an epoch-millisecond authority (§8 of the P2 design) |
| P2C foundation, unchanged in P2E | `crates/serea-storage/migrations/0001_initial.sql` | Existing bounded-u32 generation/interval CHECKs; no schema change, trigger addition or 0002 |
| P2E implemented authority | Storage lease/Tx/error/root wiring | Private nonclone/nonserializable guard, no constructor/owner formatter/Drop release; absolute EpochMillis API and exact expected_generation Option; acquire/renew/release only |
| P2E atomicity/refusals | Storage lease/Tx/error tests | Complete acquisition savepoint, explicit cleanup and outer rollback-only on cleanup failure; durable ceiling; payload-free LeaseHeld/Fenced/Expired, InvalidLeaseInterval, AttemptCeilingReached and LeaseGenerationOverflow; overflow classified before mutation with bounded SQL defense, no string matching |
| P2E authority closure | Storage evidence in closure record | H1–H8, H14/H14b, acquisition half of H15/H18/H22, H16, H19–H21 and authority regressions; separate file-backed Stores, not a process-local registry. Actual stable/MSRV/debug/release and independent-review evidence is recorded there; no outcome proof |
| P2F begin/outcomes/deletion | Storage/engine whole-transition APIs and tests | Borrowed begin with authoritative unexpired lease and no second attempt charge; consuming outcomes with embedded unreleased-authority predicates; H9–H13 receipts/journal/task assertions, outcome H15/H22, begin H18, deletion H17 |
| Full ADR runtime gate | P2E authority plus P2F lifecycle/outcome evidence | ADR remains Proposed during lease-only closure; historical probes are not runtime proof. Child-process crash evidence is separately P2H |

## Consequences

- A stale worker cannot commit. Not because the engine checks, but because the
  database predicate cannot match.
- The fence is durable, so it survives a process restart: a worker that stalls
  across a restart cannot commit against a lease reclaimed in the meantime.
- Two acquisitions by the same owner string after a reclaim are distinguishable,
  so a self-confusing worker cannot write with an old guard.
- `renew` refusing an expired lease is what stops a stalled worker from
  resurrecting its own fence. It is the clause most easily forgotten.
- The cost is one extra table and one extra indexed lookup per attempt. That is
  the price of not needing a mutex that would not survive a second process.

## Rejected alternatives

| Alternative | Why rejected |
| --- | --- |
| A process-local mutex per step | Does not survive a second process, which is exactly the stale-worker scenario. The prompt's own prohibition |
| `lease_owner` alone, refreshed on reclaim | Both workers can legitimately hold the same owner string after a reclaim, so the value does not discriminate |
| `lease_expires_at` alone | A holder whose lease expired but who has not noticed is still holding the step |
| Comparing `now_ms` at commit time | Racy: time can pass between the check and the write |
| An optimistic `version` column on `task_steps` | Equivalent in shape to `generation`, but `generation` is required to be **monotonic across reclaims**, which a version bumped per row write does not guarantee once rows are deleted |
| Holding the transaction open for a provider call | SQLite is single-writer, so an open write transaction would serialise every task in the host. The lease exists precisely so the transaction is short |
| Letting `renew` extend an expired lease | A stalled worker resurrects its own fence and the guarantee is void |
| Storing the lease as JSON on the step row | The fence predicate must be SQL-addressable; a JSON document would turn it into an application-side read-modify-write and destroy the atomicity |

**Two rows this ADR previously carried were removed by the P2 autonomous audit**,
and their removal is itself part of the record. They read *"Comparing `owner` but
not `token` | Two acquisitions by the same owner are indistinguishable without the
token"* and *"Rendering the token in `Debug` or in an error | `DC7` … The token is
not a credential, but the same discipline applies"*. The first **argues for the
token this ADR removed**, and is refuted by this ADR's own `acquire` section:
`generation` increments on every acquisition including an expiry reclaim, so two
acquisitions by one owner *are* distinguishable. The second defends a value that no
longer exists. A rejected-alternatives table that argues for the rejected thing is
worse than no table, and the defect class is exactly the one this package's §13.3
names — a disposition applied to the decision and not to the argument. The rationale
for the token's removal lives in §"Durable shape", where it belongs.