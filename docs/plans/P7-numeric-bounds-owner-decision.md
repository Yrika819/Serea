# P7-D05 numeric bounds and retention (PROPOSED)

**Status:** Proposed; no value accepted. **Implementation:** Not authorized.
**Accepted baseline:** ADR-0042 requires hard boundedness, no silent retention extension, no unsafe eviction, and Forget must remain available at ordinary quota exhaustion. Candidate values below were supplied for evaluation and remain unapproved.

## Recommended initial package

| Parameter | Proposed hard bound | Unit and accounting domain | Enforcement and failure |
| --- | --- | --- | --- |
| Active Memory count | 1,000 | rows per Owner, states active/retained | `BEGIN IMMEDIATE` insert admission; concurrent writer serializes; reject extraction as `MEMORY_QUOTA_COUNT` |
| Item body | 4 KiB | 4,096 UTF-8 bytes after canonical stored representation per item | validate before Blob write and recheck transactionally; reject item, no truncation |
| Active bodies | 4 MiB | sum of UTF-8 body bytes per Owner across active retained rows | transactional counter/aggregate; concurrent writers serialize; reject candidate; no eviction |
| FTS logical index | 8 MiB | sum UTF-8 bytes of the deterministic normalized/token source text per Owner, excluding SQLite pages/segments | transactionally account source bytes; rebuild recomputes; reject extraction/search-index update if exceeded; FTS physical DB allocation is not this quota |
| Extraction starts | 20 | durable starts per Owner per UTC day | transactional counter; restart-safe; one UTC-day bucket; rollback of wall clock cannot reopen prior bucket (persist high-water day; backwards time freezes new starts until caught up) |
| Concurrent extraction | 1 | in-flight admitted extraction per Owner | durable lease/state under immediate transaction; lease has bounded deadline; recovery marks ambiguous work failed/quarantined, not silently double-admitted |
| Retention | WORKING 7d; EPISODIC 90d; SEMANTIC 365d; PREFERENCE 365d | elapsed UTC seconds from creation or explicit owner renewal, fixed by class | expiry computed once; retrieval does not extend; expired items excluded immediately, purge async |
| Renewal count | 2 | successful renewals per item | atomic; each requires fresh auth; exhausted renewal denied |
| Cumulative retention | 2x base retention, capped at 730d from creation | elapsed UTC days | checked in transaction; no repeated renewal beyond cap |
| Expired purge batch | 100 | rows per committed GC transaction | keyset cursor `(owner_id, memory_id)` persisted transactionally; crash repeats safely; no unbounded delete transaction |
| Query | 256 bytes | UTF-8 input after normalization | reject oversize before FTS parse; only literal tokenized query, never raw FTS syntax |
| Search page | 20 | results per page | owner/class/expiry rechecked on every page; stable `(rank, memory_id)` cursor; page token binds owner and index generation |
| Search work | max 1,000 candidates returned from FTS candidate stage; max 20 final rows | logical candidate rows, plus 256-byte query | hard output-work cap; cannot strictly cap SQLite's internal FTS CPU from SQL alone, so query deadline/progress handler is required and must cancel fail-closed |
| Provenance | 4 KiB | serialized metadata per Memory excluding body, enforced on encoded bytes | transactional; omit optional field or reject; no Task FK |
| Ledger frame | 1 KiB maximum | each BEGIN, CHUNK, completion, apply, finalization, reconciliation, abort, and recovery frame | bounded immutable frame; no content or unkeyed/guessable content digest |
| Deletion ledger | 64 MiB capacity candidate; 10,000 logical frames | exact encoded bytes/frame and total frames, including every chunk and state record; physical allocation is separately measured | before BEGIN, reserve capacity for every frame of the full operation; reject partial intent; stop ordinary growth before protected recovery allowance is consumed |
| Ledger low warning | 25% logical frame/byte budget remaining | remaining ledger budget | warn; no ordinary growth may consume deletion allowance |
| Extraction stop | 10% logical frame/byte budget remaining or any ledger uncertainty | remaining ledger budget/state | stop extraction/renewal; preserve at least one maximum-closure operation allowance when possible; if a complete Forget cannot fit, no success and all Memory reads fail closed |
| Backup age | 30 days maximum | age since ledger-compatible backup manifest | restore older backup requires reinitialization or external reconciliation; stale snapshots quarantined |
| Tombstone retention | at least max supported backup age + 7d = 37d (PROPOSED) | elapsed time since Forget; per target | no expiry while any accepted backup can contain row; age alone does not detect full rollback; optional equivalence tags have separate approval/lifecycle |

The supplied candidate values are retained as proposed inputs: count 1,000; body 4 KiB; total bodies 4 MiB; FTS logical budget 8 MiB; 20 starts/day; one concurrent extraction; lifetimes 7/90/365/365 days. No approximate database file length is used as proof of logical quotas.

## Rationale and workload

A 4 MiB total body quota bounds plaintext content while allowing up to 1,000 short preferences/facts; the per-item cap prevents one large item consuming the pool. A 4 MiB body + 8 MiB logical FTS source budget gives a 12 MiB logical per-Owner payload/index ceiling before provenance, SQLite page overhead, WAL, backups, and temporary rebuild files. Rebuild staging must reserve capacity for a second logical 8 MiB index plus measured platform page/WAL headroom; actual disk-space checks are advisory only and must fail closed. 20 starts/day and one in flight constrain repeated model/extraction work while permitting a modest daily batch. 7/90/365-day classes distinguish transient from durable preferences without retrieval renewal. The 64 MiB reserve and 10,000-frame ledger cap are starting proposals, not proven allocation. At most 1,000 active Memory items implies a proposed maximum closure of 1,000 targets; with up to 500 two-entry chunks plus operation-state frames, each frame capped at 1 KiB, a maximum operation consumes roughly 0.5 MiB of logical frames. Frame count includes every state transition, not operations. Physical file allocation, directory/inode overhead, WAL, temporary rebuilds, and filesystem behavior may exceed logical bytes; the reserve is not enforceable until a target-specific preallocation or dedicated quota design is verified.

## Accounting and concurrency contract

All admission counters, ownership, retention, provenance, Blob references, and FTS row changes are committed in one `BEGIN IMMEDIATE` transaction. Every counter uses checked integer arithmetic; overflow is a refusal/corruption condition, never wraparound. A crash before commit rolls back the entire admission; after commit recovery reads durable counters and FTS external-content state. FTS5 logical source bytes are application-accounted bytes, not `dbstat`, page allocation, or file size. Physical rebuild headroom and temporary files/WAL require a separate reserve and target-platform capacity exercise. FTS integrity/rebuild is performed under exclusive maintenance barrier; readers remain blocked if stale index state cannot be proven repaired.

The daily counter uses UTC day number and a persisted monotonic maximum day. Backward clock movement cannot reset the count; forward jumps can deny starts until real time catches up and therefore require operator recovery, not counter deletion. If reliable wall time is unavailable, no new extraction starts. Quota exhaustion refuses the new extraction and never evicts retained active Memory. It cannot block an explicit authenticated Forget merely because ordinary Memory quotas are full: deletion frames are accounted separately, and the full bounded closure must fit before a ledger BEGIN is written. If the deletion allowance is exhausted or durable writes fail, no successful Forget acknowledgement is allowed; all Memory reads/model retrieval are disabled where durable evidence permits, otherwise the process refuses to expose Memory on restart until recovery. An in-memory flag is inadequate. A finite local reserve cannot promise unlimited future deletions without safe ledger compaction and independent backup/checkpoint evidence.

Expired items become invisible at the expiry comparison even if physical purge is pending. GC uses an indexed keyset cursor, a limit of 100, and one transaction per batch; crash repeats a batch idempotently. Search rechecks status, Owner, class and expiry against authoritative rows, then joins FTS only as candidate generation. Renewal is explicit, fresh-authenticated, audited with no content, and cannot restore a forgotten item.

## Alternatives

A/B/C quota matrix choices remain unselected. Smaller packages reduce storage/recovery burden but refuse more extractions; larger values increase rebuild, backup, and deletion reserve needs. Physical-file byte quotas are rejected as a substitute for exact logical accounting because SQLite page reuse, WAL, FTS segments, and rebuild temporaries make them unsuitable as an exact per-Owner measure. Soft quotas are rejected for primary privacy bounds; warning thresholds may be soft, but admission limits are hard.

**Security justification:** limits are enforced before content admission and transactionally rechecked; Forget has separate reserved capacity; no expiry is extended by retrieval and no active item is silently evicted.
**Known limitations:** SQLite cannot impose a strict CPU bound on FTS internal work using a result limit alone; physical free space is platform/filesystem dependent; the reserve size is not benchmark-derived; clock integrity is not guaranteed.
**Validation evidence:** no benchmark claim. Candidate workload arithmetic above is a capacity model only. Python SQLite probes exercise integer overflow and boundary predicates separately; see [isolated evidence](P7-sqlite-feasibility-evidence.md).
**Exact owner questions:** Accept/modify/defer each row? Is 64 MiB/10,000 ledger records acceptable? Is 30-day backup support a product constraint? Accept 2 renewals and 2x/730-day cap? Require fresh auth for each renewal?
**Implementation gate:** Gate A recommendation only; Gate B must explicitly ratify numbers and accounting; Gate C separately authorizes schema.
