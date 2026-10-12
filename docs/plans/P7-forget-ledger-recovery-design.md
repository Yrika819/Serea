# P7-D07 Forget, deletion ledger, and recovery (PROPOSED)

**Status:** Proposed for owner selection; no durability claim is accepted. **Implementation:** Not authorized.
**Accepted baseline:** ADR-0042 requires authenticated explicit Forget, transitive derived-Memory deletion, content-free tombstones, shared-Blob-safe GC, an independent deletion/reconciliation record, restored-snapshot quarantine, and truthful incomplete reporting. Migrations 0001–0008 and Blob identity `(digest,data_class_rank)` remain unchanged.

## Decision comparison

| Option | Strength | Failure/rollback limitation | Disposition |
| --- | --- | --- | --- |
| A. Primary tombstone plus separately reconciled ledger | DB transaction hides/deletes rows and updates FTS atomically while DB survives | Primary commit alone does not survive an older DB restore; a separate ledger write is not atomic with it | Required local step, insufficient authority alone |
| B. Independent ledger plus startup/read barrier | Current ledger can repair a restored older primary DB before reads | Missing, corrupt, or rolled-back ledger means no proof; Memory must stay unavailable | Recommended authority/barrier, subject to owner-selected survival domain |
| C. External generation/epoch anchor | Detects rollback if a current anchor survives independently | No trusted monotonic hardware has been verified for the target hosts; local hash chains/files can roll back together | Not assumed; separate future platform evidence only |

**Recommendation:** combine A and B. A cross-process Memory operation lock serializes readers/writers around a Forget sequence; an independent ledger records all closure chunks before primary mutation. A protected ledger location/manifest outside ordinary DB snapshots is a proposal only. It detects rollback only if that evidence actually survives independently. A hash chain is tamper evidence when its current head survives; it is not monotonic rollback protection.

## Identity fields and schema sketch

Keep five concepts distinct:

1. `memory_id`: random opaque identity for one Memory row; never derived from body/source.
2. `source_id`: random opaque identifier for an admitted source occurrence/provenance record. Required for provenance/lineage if the Owner approves source tracking; it contains no content-derived value.
3. `source_equivalence_tag`: optional keyed tag used only if the Owner separately approves re-extraction suppression. It is not `source_id` and is absent by default.
4. HMAC tag: one possible encoding for the optional equivalence tag; it requires separately approved key custody, rotation, restore, and loss behavior. It is not authorized by this proposal.
5. `forget_op_id` / ledger sequence: random operation identity and monotone-within-ledger record sequence; these identify a deletion operation, not Memory content.

Primary DB sketch (illustrative, not migration DDL):

```sql
memory_owner(owner_id TEXT PRIMARY KEY, identity_epoch INTEGER NOT NULL,
             db_applied_generation INTEGER NOT NULL, read_state TEXT NOT NULL);
memory_items(memory_id TEXT PRIMARY KEY, owner_id TEXT NOT NULL,
             source_id TEXT NOT NULL, class_rank INTEGER NOT NULL,
             state TEXT NOT NULL, body_blob_digest TEXT, body_class_rank INTEGER,
             created_at_ms INTEGER NOT NULL, expires_at_ms INTEGER NOT NULL,
             UNIQUE(memory_id, owner_id),
             FOREIGN KEY(body_blob_digest, body_class_rank)
               REFERENCES blobs(digest, data_class_rank));
memory_provenance(memory_id TEXT PRIMARY KEY, owner_id TEXT NOT NULL,
                  source_id TEXT NOT NULL, source_task_id TEXT,
                  FOREIGN KEY(memory_id, owner_id)
                    REFERENCES memory_items(memory_id, owner_id));
-- Optional only after owner approval of suppression mode and key lifecycle:
memory_source_suppression(owner_id TEXT NOT NULL, key_version INTEGER NOT NULL,
                          equivalence_tag BLOB NOT NULL,
                          PRIMARY KEY(owner_id,key_version,equivalence_tag));
memory_tombstones(owner_id TEXT NOT NULL, memory_id TEXT NOT NULL,
                  forget_op_id TEXT NOT NULL, ledger_generation INTEGER NOT NULL,
                  forgotten_at_ms INTEGER NOT NULL, state TEXT NOT NULL,
                  PRIMARY KEY(owner_id, memory_id));
```

The prior sketch's mandatory `source_tag BLOB NOT NULL` was ambiguous and is withdrawn. This proposal uses an opaque `source_id` for provenance, while equivalence suppression is a separate nullable/optional relation. If Owner approval does not include suppression, no equivalence tag or tag key exists; repeated extraction is not claimed to be suppressed by content equivalence. No plaintext Memory text or unkeyed/guessable content digest may enter a tombstone. Existing Blob identity `(digest,data_class_rank)` is unchanged.

If separately approved, a proposed HMAC input would use a versioned domain such as `serea:p7:forget-source-equivalence:v1\0` plus canonical source locator. The key must be distinct from the ledger-integrity MAC key. Both key custody designs remain undecided. Rotation must preserve comparison for every supported backup/tag version or explicitly disable suppression and quarantine on loss; no key store or recovery protocol is approved here. HMAC tags leak equality and may still reveal low-entropy source locators under key compromise.

## Bounded, complete ledger representation

A ledger operation uses immutable, individually framed records. Each canonical encoded record is at most **1,024 bytes**; a Chunk record carries at most **512 bytes** of canonical entry payload. No list is truncated to fit.

- `BEGIN`: owner ID/epoch, generation, operation ID, request nonce, snapshot generation, exact target count, exact chunk count, format/key versions, and closure commitment. The candidate maximum closure is 1,000 Memory IDs, matching the proposed per-Owner active-item cap; it remains PROPOSED. A larger/unbounded store would require changing both bounds and reserve calculations.
- `CHUNK`: operation ID, zero-based chunk index, declared total chunks, canonical entries, previous-record MAC, and this-record MAC. Entries are sorted bytewise by `memory_id`; duplicates are forbidden. Each entry identifies direct versus derived target and its immediate predecessor/source Memory ID where applicable. Optional equivalence tags appear only in an approved suppression mode.
- `COMPLETE_INTENT`: emitted only after every declared chunk exists and verifies; records exact total entries, chunk count, closure generation/commitment, and the authenticated chain tip. The primary DB must not be mutated before this marker is durable.
- `PRIMARY_APPLIED`, `LEDGER_COMMITTED`, `RECONCILED`, or `ABORT`: bounded state records refer to the same operation ID/generation. An operation is reportable complete only after all declared chunks, COMPLETE_INTENT, primary application, and ledger finalization verify.

The closure is computed from an owner-scoped consistent SQLite read snapshot while Memory writers are excluded. The canonical entries are chunked without dropping any member; chunk indices must equal exactly `[0, chunk_count)`, entries must total `target_count`, IDs must be unique, and graph edges must be owner-local and acyclic. The chain MAC authenticates each index and payload; the COMPLETE_INTENT binds the expected count and final chain tip. A missing, duplicated, reordered, altered, or truncated chunk cannot satisfy the completion check. A separately keyed ledger integrity MAC is a design dependency, not an accepted key-custody decision.

If a crash leaves a BEGIN or a proper subset of chunks before COMPLETE_INTENT, no primary deletion has started and no success may be acknowledged. Startup detects the incomplete operation before Memory APIs open and quarantines that Owner (global quarantine if the Owner cannot be authenticated from the valid record prefix). Recovery reopens the DB under the same operation lock, verifies the saved snapshot generation and recomputes the full closure. If it exactly matches existing chunks, it appends the missing chunks and completion marker. If not, it appends an authenticated ABORT and requires a fresh authenticated request; it never fills from a changed/guessed closure. A torn frame, missing sequence, bad MAC, or orphan temp file is ledger uncertainty: do not truncate silently; retain evidence and keep Memory unavailable pending recovery.

**Proposed accounting:** the 10,000-record cap counts every BEGIN, CHUNK, completion, apply, finalization, reconciliation, abort, and recovery frame, not 10,000 operations. Each frame counts against the logical ledger byte and record budgets. A maximum 1,000-item closure can require up to 500 chunk frames at two worst-case entries/frame, plus state frames; reserve admission must calculate the complete operation before persisting BEGIN. If capacity for every required frame is unavailable, do not start a Forget that can only partially persist and do not acknowledge it. Keep a separately protected recovery allowance for one maximum closure as a candidate; finite reserve cannot promise unbounded future Forget if records cannot be safely compacted. Compaction is allowed only when accepted backups can no longer contain the deleted rows and an independent checkpoint proves that fact. These figures are PROPOSED and need reconciliation with the numeric package and owner approval.

## Lock and transaction order

There is no nested SQLite transaction and no SQLite write transaction is held across ledger filesystem I/O.

1. **Memory visibility barrier:** acquire the process-local Memory write gate, then an exclusive cross-process operation lock. Proposed cross-process primitive is a host-verified OS lock on a dedicated lock object; supported APIs, crash release, network filesystems, and macOS/Windows/Linux behavior remain unverified. All Memory readers acquire a shared lock; every Memory mutation, extraction final commit, restore, Forget, and GC acquires exclusive. Lock order is fixed: process gate → cross-process operation lock → ledger lock (only for a record-set operation) → short SQLite transaction. Never acquire these in reverse order.
2. **Closure snapshot:** while holding exclusive operation lock, read the DB generation and full owner-scoped graph in a read transaction; verify no cycle/cross-owner edge and build the exact bounded closure. End the read transaction before filesystem writes. All Serea Memory writers must honor the operation lock, so the closure cannot change while the lock is held.
3. **Ledger intent:** with no SQLite transaction open, acquire the ledger's own exclusive lock, write BEGIN/chunks/COMPLETE_INTENT as immutable framed records, flush record contents and directory metadata using the platform durability primitive, verify the full operation, then release the ledger lock. Keep the cross-process Memory operation lock. Exact durable semantics need target-platform testing; a successful write call alone is not durability proof.
4. **Primary apply:** start one `BEGIN IMMEDIATE`; recheck the DB generation and operation ID; set durable quarantine and applied generation, insert tombstones, delete the complete closure's FTS/provenance/Memory references, and enqueue shared-safe Blob GC. Commit. No filesystem I/O or ledger lock is held inside this SQLite transaction.
5. **Ledger finalize:** with no SQLite transaction open, append and flush PRIMARY_APPLIED/LEDGER_COMMITTED/RECONCILED frames under ledger lock. If this fails, DB remains durably quarantined; no success response and no readers reopen.
6. **Reopen:** start a short primary transaction; verify the complete ledger chain, matching OwnerId/epoch/generation and no unresolved operations; set `READY(g+1)` and commit. Then release cross-process lock and process gate. A successful Forget acknowledgement occurs only after this commit and verified FTS exclusion. Physical Blob GC may remain pending and is reported as such.

The shared read lock is held until search results/context are consumed at the boundary. To guarantee no pre-Forget context is newly sent to a model after Forget completion, model dispatch must retain a shared read lease through provider-call completion or use an equivalent serialized egress fence. This may make Forget wait for a bounded in-flight call; timeout fails closed and reports pending. A Forget cannot recall content already delivered before its linearization point. This boundary and timeout require owner acceptance. If host lock support is unavailable or cannot be verified, disable Memory rather than silently rely on in-process locking.

## Recovery and visibility state

`READY(g)` → process/interprocess exclusive gate → complete ledger intent durable → primary transaction sets `QUARANTINED(g+1)` and removes the complete target set → ledger finalization durable → primary transaction verifies and sets `READY(g+1)` → release locks and acknowledge.

On startup, before any Memory API, inspect ledger frames, OwnerId/epoch, DB generation/read_state, and restore manifest. Incomplete or corrupt records, missing ledger/keys/manifest, generation mismatch, or uncertain lock state sets a durable Owner/global barrier where storage permits; otherwise the process refuses to open Memory APIs. Replay is idempotent by `(owner_id, forget_op_id, generation, memory_id)`. Never infer success from an in-memory flag. Readers acquire shared cross-process lock and recheck readiness/generation before returning content. Extraction start and final commit require the lock/generation check; Forget's exclusive lock fences both.

A process crash releases OS locks, but that fact does not indicate transaction success. SQLite `BEGIN IMMEDIATE` rollback/commit is judged from reopened DB and configured SQLite durability. Ledger record durability is judged independently from verified frames and the platform flush guarantee. Primary DB and independent ledger are never described as one atomic transaction.

## Failure matrix

| Point | Primary DB durable state | Ledger durable state | Visibility / success | Safe recovery evidence |
| --- | --- | --- | --- | --- |
| Before validation/auth | Unchanged | Unchanged | Existing visibility; no Forget | Fresh authentication and target check |
| After auth, before locks | Unchanged | Unchanged | No success | New operation ID/nonce on retry |
| Locks held, closure snapshot/read transaction | Unchanged | Unchanged | Readers/writers blocked by cross-process gate; no success | DB generation and exact closure snapshot |
| During BEGIN/chunk frame writes | Unchanged | Valid prefix, incomplete operation or torn frame | No success; startup barrier before APIs | Frame sequence/MAC; recompute exact closure, resume or authenticated ABORT; torn data remains quarantined |
| Complete intent durable, before primary transaction | Unchanged/READY at old generation | Complete INTENT with full chunk set | API remains blocked under lock; crash restart sees intent and blocks before reads | Verify all chunks and snapshot generation; replay primary apply |
| During primary transaction | Atomic rollback or atomic commit under SQLite assumptions | Complete INTENT | Lock blocks live readers; after crash startup sees ledger intent before Memory opens | Reopen DB, generation/read_state, transaction integrity; apply or finish idempotently |
| After primary commit, before ledger finalization | Quarantined; closure and FTS deleted | Complete INTENT only | No reads, no success | Verify DB commit then append final frames |
| During ledger finalization | Quarantined | Final frame absent/partial/uncertain | No reads, no success | Validate segment durability; resume finalization or remain quarantined |
| Ledger finalized, before READY transaction | Quarantined | Complete committed operation | No reads, no success | Verify full chain and DB state; set READY in one transaction |
| After READY commit, before response | READY at new generation | Complete committed operation | Safe; retry returns same complete result | Readiness/generation plus operation ID |
| FTS update/rollback | Same atomic outcome as primary transaction | Complete INTENT | No search until final readiness and FTS verification | Query exclusion and supported FTS integrity check |
| Shared Blob GC | Memory/tombstones remain deleted; some bytes may remain | Complete record | Logical Forget complete; physical cleanup pending | Recount all class-bound references, bounded idempotent GC |
| Backup during operation | SQLite snapshot may be old or quarantined depending snapshot point | Ledger may be newer | Backup not declared usable absent matching manifest generation | Discard/mismatch quarantine; bind snapshot and ledger checkpoint |
| Restore or startup reconciliation | Restored DB may be older; replay transactions may advance it | Current/old/missing/invalid | All Memory APIs remain blocked | Independent trusted generation evidence; exact replay; manifest and integrity checks |
| Concurrent extraction/supersession traversal | Writers excluded while closure snapshot/operation runs | No success until complete closure recorded | Extraction commit cannot pass exclusive gate; bounded graph traversal | Generation, owner-local full closure, no cycle, chunk counts |
| Storage full / system-wide ENOSPC | SQLite apply may rollback or complete; readiness remains quarantined if committed | Ledger frame may fail/uncertain | No success; stop all Memory if durable denial cannot be proven | Recover capacity, inspect both durability domains, replay; no RAM-only recovery claim |

Power-loss behavior still requires target filesystem, SQLite journal/WAL mode, `synchronous`, flush ordering, directory metadata, and device testing. Python process-restart probes do not establish this behavior.

## Restore scenarios and rollback limits

| Scenario | Supported response |
| --- | --- |
| Old primary DB, current independently surviving ledger | Detect lower generation; quarantine and replay every complete newer Forget before reads |
| Old primary and old ledger, but a newer independently surviving authenticated manifest/checkpoint | Detect mismatch and quarantine/reconcile. This depends on the checkpoint being genuinely outside the rollback domain and is not yet implemented or approved |
| Primary DB with no ledger | Fail closed; never synthesize an empty ledger |
| Ledger key/anchor missing or corrupt | Fail closed; restore only from separately verified evidence or remain quarantined |
| Partial backup set | Reject as usable; no Memory reads, extraction, model context, or sync |
| Offline restore | Works only if current ledger and independently trusted manifest are available offline; otherwise quarantine |
| DB still contains a forgotten row | Apply complete ledger operation before any reader opens |
| Restore older than 30 days / after 37-day tombstone horizon | Reject/quarantine unless a separately surviving authority proves it safe; age values alone are not proof |
| Owner recovery/replacement | Require exact OwnerId+epoch match; new Owner does not inherit old data automatically |
| Primary, ledger, keys, and manifests all rolled back together | **Unsupported and potentially undetectable** without an independently surviving authority; do not claim resurrection prevention |

A 30-day maximum backup age and 37-day tombstone retention are proposed operational limits only. They do not detect complete rollback of all local evidence. A local hash chain, file generation, or keychain value also does not supply independent monotonic authority if restored together with its state.

## Deletion reserve and exhaustion

The **64 MiB** value remains a capacity candidate, not proof of enforceable allocation. Distinguish these mechanisms:

| Mechanism | What it provides | What it does not guarantee |
| --- | --- | --- |
| Logical reservation/counters | Serea refuses ordinary growth before its logical budget | Other processes, WAL, temp files, filesystem metadata, or host disk usage can consume physical space |
| Preallocated reserve file | May provide already allocated blocks that can be repurposed after ordinary space runs out, if allocation is real and platform semantics verified | Sparse allocation, metadata exhaustion, filesystem errors, WAL/SQLite write failure, or system-wide failure may still prevent durable denial |
| Dedicated filesystem/quota | Stronger accounting isolation if target OS/filesystem enforces it and reserve is inaccessible to ordinary allocations | Not universal across macOS/Windows/Linux; administrator/volume failure, metadata, ledger rotation and SQLite WAL/temp still need capacity planning |
| System-wide ENOSPC/device failure | No general Serea guarantee | Both primary DB and ledger may be unable to commit; an in-memory quarantine does not survive restart |

Reserve sizing must include ledger frames and filesystem allocation overhead, primary SQLite rollback/WAL space for FTS and metadata transactions, temp/rebuild files, checkpoint behavior, and directory/inode metadata. A reserve on the ledger volume does not ensure primary DB commit; a primary DB reserve does not ensure ledger finalization. The proposed 25% warning and 10% extraction-stop thresholds are logical admission thresholds until actual quota/preallocation is verified. If any required write cannot be durably established, acknowledge no Forget success; disable all Memory APIs and require recovery. Finite local reserve cannot guarantee unbounded future Forget if deletion evidence cannot be safely compacted; compaction depends on independent proof that all accepted backups predate neither the Forget nor the checkpoint.

## Alternatives, evidence, gates, and owner decisions

**Security justification:** no primary mutation starts before the complete bounded deletion intent is durable; cross-process locks serialize closure/read/write boundaries; no completion response is sent until primary and ledger converge.
**Known limitations:** no cross-platform lock/flush implementation was tested; complete rollback of all local evidence remains undetectable; reserve enforceability is unproven; already delivered model context cannot be recalled; logical deletion is not secure erase.
**Validation evidence:** synthetic SQLite findings are in [P7 SQLite feasibility evidence](P7-sqlite-feasibility-evidence.md); capacity assumptions and proposed thresholds are in [P7 numeric bounds](P7-numeric-bounds-owner-decision.md). No Rust, filesystem, power-loss, or platform lock test proves this proposal.
**Exact owner questions:** Accept the lock/linearization boundary and bounded model-call wait? Approve source identity separate from optional equivalence suppression? If HMAC suppression is desired, approve its separate key custody/rotation/recovery design? Accept chunk and per-operation/ledger record limits? Which independent recovery authority, if any, is supported? Accept fail-closed behavior when ledger/reserve/evidence is uncertain? Which reserve mechanism and platform is a V1 requirement?
**Implementation gate:** Gate A only. Gate B requires explicit choices for locks, closure/chunks, identity fields, rollback boundary, and reserve. Gate C separately authorizes additive schema. Gate D requires rusqlite, cross-process, crash/fault, restore, and filesystem evidence. Gate E requires independent security and target-platform review.
