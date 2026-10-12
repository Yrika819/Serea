# P7-D07 Forget, deletion ledger, and recovery (PROPOSED)

**Status:** Proposed for owner selection; no durability claim is accepted. **Implementation:** Not authorized.
**Accepted baseline:** ADR-0042 requires authenticated explicit Forget, transitive derived-Memory deletion, content-free tombstones, shared-Blob-safe GC, independent deletion/reconciliation records, restored-snapshot quarantine, and truthful incomplete reporting. Migrations 0001–0008 and Blob identity `(digest,data_class_rank)` remain unchanged.

## Decision comparison

| Option | Strength | Failure/rollback limitation | Disposition |
| --- | --- | --- | --- |
| A. Primary tombstone, separately reconciled ledger | Primary transaction makes local reads hide Forget atomically with FTS/link updates; simple when DB survives | Crash after DB commit before ledger persist leaves an unreported/incomplete operation and restore protection gap. A restored older DB has no tombstone. | Necessary local transaction, insufficient authority by itself |
| B. Independent authoritative ledger + startup/read barrier | Ledger-first Forget survives primary DB rollback if ledger itself survives; startup can replay into DB before reads | Missing, corrupted, locked, or rolled-back ledger means no proof; deny Memory reads/extraction/sync. A ledger restored old with DB old cannot reveal rollback. | Required authority/barrier, subject to trust boundary |
| C. Generation/epoch anchor | Detects partial restore when a monotonic anchor outside backup set records a newer generation | No general trusted monotonic hardware is verified for macOS/Windows/Linux. A file/keychain or local hash-chain restored together can roll back. | Not assumed available; optional future platform evidence only |

**Recommendation:** combine A+B. Treat the independently retained deletion ledger as authoritative for all completed Forget operations; append a durable intent before modifying the primary DB; keep all Memory inaccessible until ledger and DB converge. Use a protected ledger directory excluded from ordinary DB snapshots, plus a separate backup manifest that binds OwnerId, identity epoch, ledger generation, and primary snapshot generation. This is rollback detection only when the ledger/manifest survives independently. A hash chain detects alteration but not complete rollback and must not be described as a monotonic anchor.

## Record and schema sketch

Primary DB additive sketch (illustrative, not migration DDL):

```sql
memory_owner(owner_id TEXT PRIMARY KEY, identity_epoch INTEGER NOT NULL,
             db_applied_generation INTEGER NOT NULL, read_state TEXT NOT NULL);
memory_items(memory_id TEXT PRIMARY KEY, owner_id TEXT NOT NULL,
             source_tag BLOB NOT NULL, class_rank INTEGER NOT NULL,
             state TEXT NOT NULL, body_blob_digest TEXT, body_class_rank INTEGER,
             created_at_ms INTEGER NOT NULL, expires_at_ms INTEGER NOT NULL,
             UNIQUE(memory_id, owner_id),
             FOREIGN KEY(body_blob_digest, body_class_rank)
               REFERENCES blobs(digest, data_class_rank));
memory_provenance(memory_id TEXT PRIMARY KEY, owner_id TEXT NOT NULL,
                  source_task_id TEXT, source_tag BLOB NOT NULL,
                  FOREIGN KEY(memory_id, owner_id)
                    REFERENCES memory_items(memory_id, owner_id));
memory_tombstones(owner_id TEXT NOT NULL, memory_id TEXT NOT NULL,
                  forget_op_id TEXT NOT NULL, ledger_generation INTEGER NOT NULL,
                  forgotten_at_ms INTEGER NOT NULL, state TEXT NOT NULL,
                  PRIMARY KEY(owner_id, memory_id));
```

The durable tombstone contains identifiers, class/epoch/generation and status only. No text, plaintext excerpt, or unkeyed content digest. Provenance is immutable, minimized, and has no live Task foreign key. Keep a content-free ledger tombstone after deleting Memory/provenance only as necessary for restore/re-extraction policy.

Independent append-only conceptual ledger record (canonical bounded binary/JSON envelope, maximum 1 KiB each):

`format_version, owner_id, identity_epoch, generation, forget_op_id, request_nonce, target_memory_ids[], derived_memory_ids[], source_tags[], created_utc, previous_record_mac, record_mac, status(INTENT|COMMIT|RECONCILED)`.

A source equivalence tag is optional and requires HMAC, not plain hash: HMAC-SHA-256 over domain string `serea:p7:forget-source:v1\0` + canonical source locator; secret key held by the host OS secret store, never in the SQLite DB or backup manifest. Include key version; rotation retains old verification/suppression keys or re-tags under an explicit migration before retiring one. If key is lost, tags cannot be verified: stop extraction and do not promise re-extraction suppression; keep existing memory barrier until owner-directed recovery. If a supported restore cannot restore the key, require fresh owner decision and quarantine. HMAC tags can leak equality and are a privacy tradeoff requiring owner acceptance. Ledger encryption/confidentiality and key recovery are separate from integrity.

## Forget transaction and durability sequence

1. Authenticate a fresh host actor for purpose `forget`; do not accept a logged-in session or device key alone. Validate exact target Owner and idempotency nonce. If auth or target validation fails, do not write and do not claim deletion.
2. Acquire process-wide Memory operation barrier and SQLite `BEGIN IMMEDIATE`. Resolve the requested set plus complete transitive derived closure using owner-scoped rows, cycle detection, and bounded traversal. If graph exceeds the operation limit, persist/return `INCOMPLETE_LIMIT` and keep affected owner Memory inaccessible until resumed. Never truncate closure silently.
3. Compute content-free intent record and reserve deletion space. Commit ledger `INTENT(g+1)` durably first. Ledger append uses exclusive lock, sequence/generation compare, fsync of file then parent directory as platform capability allows; a successful library write alone is not proof of stable media. Exact API/durability requirements await target verification.
4. In one primary `BEGIN IMMEDIATE` transaction, set Owner `read_state=QUARANTINED`/advance `db_applied_generation`; insert content-free tombstones; mark closure pending-deletion; remove FTS rows; remove provenance; delete Memory-to-Blob references; preserve shared Blob rows while referenced elsewhere; record an idempotent pending GC set. Recheck Owner and ledger generation before commit. Search and model retrieval remain blocked for that Owner while this commit is incomplete.
5. Append durable ledger `COMMIT(g+1)` and then `RECONCILED(g+1)` only after primary commit proof. If ledger append fails, operation remains incomplete and Owner stays quarantined; retry is idempotent. Do not report complete.
6. In a primary transaction, set applied generation and `read_state=ACTIVE` only if current ledger confirms the same committed generation and no pending intents, then release barrier. Report Forget complete only after both DB transaction and independent ledger record are durable and FTS deletion is verified by an exact query/integrity check. Otherwise return truthful `PENDING_RECOVERY` with stable operation ID.
7. Physical Blob GC runs later in bounded batches. Delete only when exact `(digest,data_class_rank)` has no references across all approved owners/domains. Crash repeats safely. Logical Forget completion need not wait for shared-safe garbage collection, but user-facing status says physical cleanup pending; no secure erase claim.

To avoid claiming durable quarantine from RAM, the owner row/read barrier is committed before the Forget intent is exposed as pending; if even that cannot be written, the process stops all Memory access immediately and on restart detects ledger/store integrity mismatch if either state survives. If neither file can record a denial and all external evidence is unavailable, a prior complete operation may be indistinguishable from no operation; this is the explicit residual rollback limit below.

## Startup, reads, and extraction barrier

At startup, before opening any Memory-capable API or syncing data: verify ledger file presence, format, MAC/sequence continuity, OwnerId+epoch, and highest generation; verify primary DB's applied generation and restore manifest. Any missing/corrupt/truncated ledger, unexplained generation gap, unknown backup provenance, Owner mismatch, or key unavailability sets durable/global Memory unavailable and stops search, extraction, model context assembly, backup export, and synchronization. Recover ledger intents by replaying owner-scoped tombstones/closure, FTS deletes, Blob ref removals, and pending GC idempotently; commit DB generation; then append reconciliation marker. Only after exact generation equality and no unresolved intents may Memory reads resume.

Every Memory read/search/model-retrieval and extraction commit also checks a process barrier plus authoritative generation under a serialized Storage operation. A stale connection cannot bypass it. Extraction captures source tags and admission generation at start; its final transaction rechecks that no newer Forget covers the source and its tags are not suppressed. Forget takes a write fence that cancels or invalidates in-flight extraction; the extraction result is discarded if its generation is stale.

## Recovery states

`READY(g)` -> `FORGET_INTENT(g+1)` -> `PRIMARY_APPLIED(g+1)` -> `LEDGER_COMMITTED(g+1)` -> `READY(g+1)`.
Any uncertainty transitions to `QUARANTINED(reason, observed_generation)`; only replay plus independently verified ledger convergence returns to READY. No startup path infers success from a primary tombstone without ledger confirmation or from a ledger record without applying it. `INCOMPLETE_LIMIT` remains quarantined and resumable; idempotent retry with same operation ID continues. Storage exhaustion, key loss, or corruption never falls back to ordinary reads.

## Failure matrix

| Point | Primary DB | Independent ledger | Visibility / report / extraction | Safe recovery and evidence |
| --- | --- | --- | --- | --- |
| 1 Before Forget validation | Unchanged | Unchanged | Existing state; no success; extraction may continue | Re-authenticate; request evidence |
| 2 After auth, before transaction | Unchanged | Unchanged | No success; auth nonce consumed; extraction may continue unless operation lock taken | Fresh retry; auth evidence/nonce audit |
| 3 During tombstone insertion | Transaction rollback or partial uncommitted pages | INTENT if step 3 completed | Owner barrier; no search/model; pending only; extraction blocked | SQLite recovery, replay INTENT; transaction/journal evidence |
| 4 After primary commit | Tombstones/FTS deletion committed, DB generation advanced | INTENT only | Owner quarantined; no success; extraction/search stop | Append COMMIT then RECONCILED; inspect rows, FTS, generation |
| 5 Before ledger persistence | Prior primary state or no commit | No INTENT | If no durable intent, no success and no Forget action; if DB may have started, global barrier until consistency inspection | Reopen/check DB; retry using operation ID; no assertion from RAM |
| 6 During ledger persistence | Unchanged or transaction state | Torn/uncertain final record | Global Memory barrier, no success, extraction/search stop | Verify authenticated complete record; truncate only incomplete tail under locked recovery, retain evidence |
| 7 Ledger durable, before success report | Applied DB or INTENT/COMMIT state | Durable record | No user success yet; reads blocked until READY | Retry returns same operation status; durable generation/MAC evidence |
| 8 During FTS update/rollback | SQLite transaction either all committed or rolled back; if uncertain DB integrity check | INTENT persists | Owner blocked; no stale FTS use | Replay FTS deletion/rebuild under barrier; query + FTS integrity evidence |
| 9 During shared Blob GC | Tombstones and ref deletion durable; some unreferenced blobs may remain | COMMIT/RECONCILED | No content retrievable; physical cleanup pending, not a Forget failure | Recompute exact class-bound refcount; delete only zero refs; repeat batch |
| 10 During backup creation | Snapshot is consistent only if SQLite backup API/manifest protocol succeeds | Ledger has generation g | Backup not exportable until manifest binds DB generation g and ledger checkpoint g; no service success affected | Discard incomplete backup; checksum manifest and generation evidence |
| 11 During restore | Restored DB may be older/newer; ledger independently current | Current ledger or unknown | Startup barrier blocks every Memory surface | Compare epoch/generations, replay newer tombstones; missing evidence => permanent quarantine until owner recovery |
| 12 During startup reconciliation | Partial replay transaction rolls back or batch cursor advances durably | Authoritative sequence intact | No reads/extraction/sync | Idempotent replay; exact generation and unresolved-intent count evidence |
| 13 Concurrent extraction of forgotten source | Write fence invalidates admission/final commit | INTENT generation advances | Search/model blocked; extraction cannot commit old generation | Retry source only after READY and suppression check; compare captured/current generation |
| 14 During graph traversal | No deletion commit until bounded closure validated; incomplete traversal may have read-only snapshot | No intent until closure resolved (or a preliminary quarantine intent) | Do not report complete; quarantine affected Owner once traversal bound exceeded | Resume from persisted cursor only with closure generation fence; prove closure count and no cross-owner edge |
| 15 Storage full | Atomic DB transaction may fail; some prior states intact | Append may fail or be uncertain | No success; stop all Memory reads if durable quarantine cannot be proven; stop extraction | Free/repair reserve, verify ledger tail, replay; preserve operation ID and report pending |

Power loss and partial durable writes require filesystem, SQLite journal/WAL, `synchronous` mode, fsync, and hardware testing; a process-restart simulation proves none of these. SQLite transaction atomicity covers only one DB under its configured durability assumptions, not an independent ledger file.

## Restore scenarios and limits

| Restore | Required response |
| --- | --- |
| Old primary DB + current independent ledger | Detect lower DB generation; quarantine; replay every newer Forget before reads; preserve suppression tags |
| Old primary DB + old ledger together | If no surviving external checkpoint/manifest, complete rollback is undetectable. Treat backup as untrusted/expired; require independent owner confirmation and quarantine. No resurrection-prevention guarantee |
| Primary DB without ledger | Missing authority; global Memory barrier; do not synthesize an empty ledger |
| Ledger key/integrity anchor lost | Cannot authenticate authoritative records; no read/extraction/sync; recover key from separately verified backup or owner-approved reset/quarantine |
| Partial backup set | Generation/hash mismatch; reject as a usable Memory restore |
| Offline restore | Ledger and signed manifest must be locally available and current; otherwise remain quarantined offline. Network is not assumed |
| DB contains previously forgotten Memory | Replay tombstones and delete derived closure/FTS/Blob refs before opening readers |
| Restore after tombstone expiry | Supported backups are max 30d; tombstone horizon is >=37d. Older/unknown backup is rejected/quarantined, not admitted as clean |
| OwnerId recovery/replacement | Match exact OwnerId+epoch. New ID never inherits data automatically. Old ledger/key missing means old data quarantined; owner explicitly chooses rebind, export-free reset, or retain inaccessible |

A system cannot distinguish a complete rollback of every local DB, ledger, key, and manifest from a genuine older state without an independently surviving authority. No verified monotonic hardware, network authority, or universal host service is assumed. A local hash chain only detects partial alteration when a current chain head survives.

## Reserve, exhaustion, and idempotence

Proposed 64 MiB deletion reserve and 10,000 live operation records, with 25% warning and stop new extraction at 10% remaining. Reserve must be separately accounted/preallocated and inaccessible to ordinary Memory writes. Forget uses bounded 100-item replay/GC batches. Repeat Forget with same `forget_op_id` returns the same terminal/pending state and cannot create duplicate work. If ledger corruption or reserve exhaustion prevents durable denial/quarantine, stop all Memory reads, retrieval, extraction, synchronization, and backup export; this stop must be recoverable from an external surviving record or remain until manual recovery. No in-memory-only quarantine claim.

## Alternatives, limitations, gates

**Security justification:** ledger-first intent prevents the normal primary-commit/ledger-later window from being reported as complete; startup/read barriers prevent stale primary snapshots from being served when current ledger evidence survives.
**Known limitations:** complete rollback of all local evidence cannot be detected; ledger key custody/rotation and supported backup product are unresolved; filesystem fsync semantics vary; SQLite local file tampering remains in threat model; logical deletion is not physical secure erase.
**Validation evidence:** primary schema in migrations 0001–0008 contains no Memory/Forget tables; Blob APIs and references are class-bound and existing GC helpers are DB-scoped. Synthetic transaction/restore probes are described in the evidence record and do not prove power-loss behavior.
**Exact owner questions:** Which ledger survival domain is acceptable (second protected local volume, owner-managed backup, or future remote authority)? Accept fail-closed global Memory outage on ledger uncertainty? Accept HMAC equivalence-tag equality leakage and key custody? Accept 64 MiB reserve/10,000 records/30-day backup horizon? Is Forget “complete” once logical deletion is converged while physical GC remains pending? What explicit recovery may replace an OwnerId?
**Implementation gate:** Gate A only. Gate B requires selected trust/rollback model, reserve and key/backup lifecycle. Gate C separately authorizes additive schema. Gate D requires Rust fault injection; Gate E independent review and target restore evidence.
