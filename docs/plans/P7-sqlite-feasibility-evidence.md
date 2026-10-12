# P7 isolated SQLite feasibility evidence

**Purpose:** synthetic probes of prospective schema/transaction semantics only. No Serea migration, Rust source, or existing Serea database was changed. All created databases were disposable files under `/tmp/serea-p7-sqlite-*` and contained synthetic Owner/Task/Memory identifiers and strings.

## Environment and portability boundary

- Platform: Linux x86_64, kernel string `Linux-6.18.44-x86_64-with-glibc2.41`.
- Python: 3.12.14 `sqlite3` module linked to SQLite **3.53.1**.
- Repository Rust configuration: `rusqlite = 0.40.2`, `libsqlite3-sys = 0.38.2`, feature `bundled`; checked crate header declares bundled SQLite **3.53.2**. The probes below did not run through Rust/rusqlite. One patch-level version difference remains, and no claim is made that this proves Rust bundled behavior.
- SQLite settings in the probe: `PRAGMA foreign_keys=ON`, autocommit connections, `BEGIN IMMEDIATE` where indicated, FTS5 `unicode61`.
- No power interruption, filesystem fault, fsync verification, process kill during SQLite write, Android target, or production migration was tested. A Python close/reopen is only a process-level restart simulation.

## Synthetic schema assumptions

The temporary schema defined `owners(owner_id PRIMARY KEY)`, `tasks(task_id PRIMARY KEY, owner_id NOT NULL, UNIQUE(task_id,owner_id), FK owner_id)`, `memory(memory_id PRIMARY KEY, owner_id NOT NULL, UNIQUE(memory_id,owner_id), FK owner_id)`, `prov(memory_id PRIMARY KEY, owner_id, task_id nullable, FK(memory_id,owner_id) -> memory)`, `blobs(digest,class_rank,content,PRIMARY KEY(digest,class_rank))`, `refs(memory_id,digest,class_rank,FK(digest,class_rank)->blobs)`, `edges(owner_id,old_id,new_id,PRIMARY KEY(owner_id,old_id),UNIQUE(owner_id,new_id),CHECK(old_id<>new_id))`, an immutability trigger on `tasks.owner_id`, a recursive CTE cycle trigger on `edges`, an FTS5 table, and integer quota CHECKs (`active_items 0..1000`, `body_bytes 0..4194304`). This is a probe schema, not proposed migration SQL.

## Executed experiments and results

| Probe / SQL operation | Expected | Observed | Enforcement |
| --- | --- | --- | --- |
| Insert child provenance with `(memory_id,owner_id)` matching parent `UNIQUE(memory_id,owner_id)` | Accept | PASS, inserted | SQLite FK + UNIQUE |
| Define child FK to parent columns without PK/UNIQUE, then insert (`INSERT INTO bad_child VALUES('a','b')`) | Reject as invalid parent key | PASS, `OperationalError: foreign key mismatch` | SQLite schema validation on use |
| `UPDATE tasks SET owner_id=other_owner WHERE task_id='t1'` | Reject | PASS, `IntegrityError: owner immutable` | Trigger |
| Insert provenance for memory under a different Owner than its parent | Reject | PASS, FK constraint failed | Composite FK |
| Delete Task while provenance has only an opaque nullable `task_id` and no FK | Provenance survives | PASS, provenance count 1 | Deliberately no Task FK; app chooses retention |
| Insert Blob ref to exact `(digest,class_rank)` | Accept | PASS | Composite FK |
| Insert Blob ref with same digest but different class rank | Reject | PASS, FK constraint failed | Composite FK |
| Insert one edge then a second edge with same `(owner,old_id)` | Reject second | PASS, unique constraint failed | UNIQUE |
| Insert `a->b`, `b->c`, then `c->a` | Reject cycle | PASS, `IntegrityError: cycle` | Recursive CTE trigger |
| Two connections race contradictory edges for same `(owner,old_id)`; first holds `BEGIN IMMEDIATE`, second waits then inserts | One commit only | PASS, second failed unique constraint | SQLite write serialization + UNIQUE; no parallel simultaneous write |
| Start Forget transaction, insert tombstone, delete FTS/ref rows, `ROLLBACK` | All changes absent/restored | PASS: tombstones 0, FTS row 1, ref row 1 | Single SQLite transaction |
| Delete one reference while second reference still exists | Blob remains referenced | PASS, remaining refs 2 including another synthetic row | Reference counting query is application logic; FK does not auto-GC |
| FTS insert then `ROLLBACK`; query `MATCH 'rollback'` | No match | PASS, 0 | FTS5 participates in transaction |
| Commit FTS insert; query `MATCH 'committed'` | Match | PASS, 1 | FTS5 transaction |
| Delete FTS row then `ROLLBACK`; query again | Match restored | PASS, 1 | FTS5 transaction |
| Close/reopen DB while synthetic ledger is generation 5 `INTENT`, primary applied generation 4 | Detect incomplete sequence and require barrier | PASS; observed `(4, (5, 'INTENT'), 'barrier required')` | Barrier is application logic, not SQLite |
| Copy old primary snapshot at generation 4; compare with newer synthetic tombstone in current DB/ledger | Detect mismatch and quarantine/replay | PASS, old generation 4 vs current tombstone count 1 | Restore policy is application logic |
| Update quota 999/4,194,303 to exact 1,000/4,194,304 boundary | Accept | PASS | CHECK |
| Increment count from 1,000 to 1,001 | Reject | PASS, CHECK constraint failed | CHECK; body overflow similarly configured |
| Query `PRAGMA foreign_keys` | Must be on for connection | PASS, returns 1 | Connection initialization responsibility |

All 21 listed assertions passed in the corrected run. An initial harness run had two fixture mistakes (reused a provenance primary key and created a SQLite connection in a different Python thread); those were corrected before the reported run. No finding was suppressed.

## Queries and experimental patterns

The exercised representative statements included:

```sql
INSERT INTO prov VALUES('m_valid','own_0123456789abcdef0123456789abcdef','missing');
UPDATE tasks SET owner_id='own_aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa' WHERE task_id='t1';
INSERT INTO refs VALUES('m3','sha256:test',1);
INSERT INTO edges VALUES('o','a','b');
INSERT INTO edges VALUES('o','b','c');
INSERT INTO edges VALUES('o','c','a');
BEGIN IMMEDIATE;
INSERT INTO tombstones VALUES('m1',NULL);
DELETE FROM mem_fts WHERE memory_id='m1';
DELETE FROM refs WHERE memory_id='m1';
ROLLBACK;
SELECT count(*) FROM mem_fts WHERE mem_fts MATCH 'committed';
UPDATE quota SET active_items=1000,body_bytes=4194304 WHERE owner_id='q';
UPDATE quota SET active_items=active_items+1 WHERE owner_id='q';
```

The cycle check was a recursive CTE from `NEW.new_id` through existing successors and rejected insertion when the walk reached `NEW.old_id`. The concurrency test used two Python SQLite connections, one holding a writer reservation while the other waited; it validates serialized constraint behavior under this harness, not all OS/runtime scheduling conditions.

## Findings and unresolved SQL constraints

1. Composite FKs require the referenced exact column list to be a matching PRIMARY KEY/UNIQUE key, including collation compatibility. A mismatched declaration may be accepted at `CREATE TABLE` and fail later with `foreign key mismatch`; migration validation must execute representative DML and run `foreign_key_check`.
2. Owner immutability needs trigger and application-level enforcement. SQLite FK alone does not prohibit changing a valid Owner to another valid Owner. `INSERT OR REPLACE` must be forbidden on immutable tables because it can delete/reinsert rows and interact with cascades.
3. A Task deletion does not remove content-minimized provenance only when provenance intentionally lacks a Task FK. `task_id` should be opaque audit context and not a live FK; Forget must still delete the selected Memory/provenance explicitly.
4. Exact class-bound Blob references are expressible with existing Blob key `(digest,data_class_rank)`. Zero-reference Blob GC is a separate serialized query and must count every reference table before deleting.
5. A unique `(owner_id,old_id)` and `(owner_id,new_id)` enforce at most one successor and predecessor under the chosen edge direction. Acyclicity needs a trigger/transaction algorithm; a trigger can check cycles for each insert, while application transactions must handle bulk graph changes and bound recursive work.
6. FTS5 writes roll back with SQLite transactions in this build. Stale-index safety still requires search barriers and authoritative row/owner/class/expiry checks; this test did not verify external-content trigger configurations or `integrity-check` behavior on corruption.
7. Ledger sequence, restart barrier, restore quarantine, query-work budgets, auth freshness, quota counters, HMAC/key lifecycle, deletion reserve, and truthful Forget completion are application/system contracts. SQLite cannot enforce the independent ledger atomically.
8. The actual bundled Rust SQLite is 3.53.2 by vendored header inspection, versus Python 3.53.1. Run equivalent rusqlite integration tests against the actual project feature set before treating any result as a production constraint proof.

**Security justification:** the probes test that proposed relational invariants can be expressed and expose which checks remain application responsibilities.
**Known limitations:** synthetic schema only; no Rust binding execution, fault injection, power-loss, filesystem guarantee, target OS authentication, or production migration.
**Validation evidence:** exact environment and 21 probe outcomes above. No experiment mutates repository migrations 0001–0008.
**Exact owner question:** Accept this proof boundary and require rerun in Rust/rusqlite under Gate D, or require a narrower SQL design before ratifying the contract?
**Implementation gate:** Gate A evidence only. P7B and Rust runtime tests remain separately gated.
