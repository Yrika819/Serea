# P7 Gate B Owner Decision Package (PROPOSED)

**Status:** All technical choices below are PROPOSED. No choice is accepted by this document. **Production Memory:** unauthorized and disabled. **Scope:** one consolidated decision review for P7 technical details after ADR-0042's seven architectural directions.

The current P7 implementation is a synthetic test-only prototype behind `test + p7-experimental`. Migration 0009 is unratified. Its measurements do not establish production storage, authentication, deletion durability, or platform support. The Owner may accept, modify, or defer each category as one package; selecting one category does not authorize production enablement.

For each category, select exactly one: **[ ] Accept recommendation  [ ] Modify (describe)  [ ] Defer**. Acceptance means only that Gate B technical choices are selected. Gate C implementation and Gate D production enablement still need separate authorization.

## A — Numeric limits and retention

### Recommendation A (PROPOSED)

Adopt the provisional values in the table below as hard admission and operation bounds for a later V1 implementation. Keep them disabled until Gate C. Do not evict active or unexpired Memory to free quota. Retrieval never extends expiry. Require fresh Owner authentication for each explicit renewal.

| Parameter | Proposed V1 value | Unit / enforcement meaning |
|---|---:|---|
| Active item count | 1,000 | Per Owner; transactionally enforced |
| Maximum body | 4,096 | UTF-8 bytes per item |
| Active body budget | 4 MiB | Sum of stored body bytes per Owner |
| Logical FTS source budget | 8 MiB | Count normalized indexed-source bytes; does not equal SQLite page/WAL/FTS segment storage |
| Extraction starts | 20/day | Durable UTC day bucket per Owner |
| Concurrent extraction | 1 | Durable Owner-scoped admission fence |
| WORKING retention | 7 days | From creation or explicit renewal |
| EPISODIC retention | 90 days | From creation or explicit renewal |
| SEMANTIC retention | 365 days | From creation or explicit renewal |
| PREFERENCE retention | 365 days | From creation or explicit renewal |
| Explicit renewal | At most 2 | Fresh-authenticated Owner action per item |
| Cumulative age | At most 2x base life, capped at 730 days | From creation; checked transactionally |
| GC batch | 100 rows | Per committed deletion transaction |
| Query input | 256 UTF-8 bytes | Reject before FTS parsing |
| Search page | 20 rows | Stable owner-bound cursor; recheck authoritative rows |
| FTS candidates | 1,000 | Output candidate cap plus separate VM cancellation budget; not a strict internal CPU bound |
| Provenance metadata | 4 KiB | Encoded bytes per item |
| Tombstone frame | 1 KiB | Encoded content-free metadata per target |
| Ledger capacity | 64 MiB / 10,000 frames | PROPOSED logical limit; count every chunk and state transition |
| Backup support horizon | 30 days | Older snapshots are unsupported and quarantined |
| Tombstone minimum | 37 days | 30-day maximum supported backup age plus 7-day reconciliation allowance |

**Alternatives:** (1) smaller fixed limits and shorter retention; (2) per-device configurable limits; (3) no retention for selected kinds; (4) larger or soft quotas. Soft admission quotas are discouraged because they make concurrent and offline behavior hard to bound. Any soft warning may accompany, but cannot replace, the hard admission limit.

**Security consequences:** hard transactional checks prevent concurrent quota overflow; no silent eviction protects unexpired content; finite expiry and renewal reduce indefinite retention. The FTS logical source budget does not cap physical SQLite growth. A 64 MiB deletion reserve is not a promise of 64 MiB free disk: unrelated writes, filesystem metadata, WAL growth, snapshots, and allocation failure can consume or prevent that space. A later implementation must maintain a separately accounted logical reserve, refuse extraction before reserve admission becomes unsafe, and keep all Memory reads closed when Forget cannot durably record intent. Physical free-space checks are advisory only.

**Expiry/accounting:** expired rows become immediately ineligible for list/search; asynchronous deletion releases active accounting only according to the ratified state matrix. Superseded rows remain retained until their permitted deletion transition; deleting an inactive row must not decrement active bytes twice. Recompute authoritative counters during migration/recovery rather than silently resetting counters.

**Evidence:** P7 synthetic SQLite tests cover concurrent admission, count/body boundaries, downgrade attempts, expiry filtering, supersession, inactive deletion accounting, and per-transition counter recomputation. They do not prove physical disk reserves or a useful end-user retention policy.

**Effort:** medium for ratified SQL/accounting; high for reserve admission across primary DB, FTS, WAL, temp files, backups, and platform filesystem behavior.

**Known limits / reset rule:** if the ledger cannot append, close retrieval/extraction/export/synchronization and preserve pending status. Do not auto-delete unexpired Memory or retry with a weaker ledger. If an Owner-approved recovery cannot restore verifiable current-generation evidence, remain quarantined indefinitely or require an explicit reset that destroys the entire affected Memory namespace and its future restoration authority.

**Owner decision A:** [ ] Accept recommendation  [ ] Modify: ______  [ ] Defer

## B — Owner authentication

### Recommendation B (PROPOSED)

Use one opaque random `OwnerId` per local installation, created by a host-owned bootstrap service only after a supported host adapter proves the required local principal. Do not derive OwnerId from username, device/session ID, model output, or a caller-supplied field. Bind it immutably to a Task in the Task admission transaction. Quarantine legacy Tasks whose Owner cannot be proved. Require fresh, purpose-bound authentication for bootstrap, Forget, renewal, Owner recovery, device enrollment, and destructive restore decisions; an active desktop session alone is not fresh authentication.

Proposed adapter boundary (interface contract only; **NOT IMPLEMENTED**):

```text
HostAuthenticator.authenticate(purpose, freshness_policy) -> AuthEvidence | Denied
OwnerIdentityService.bootstrap(AuthEvidence) -> AuthenticatedOwner
AuthenticatedOwner.owner_id is non-constructible by request/model/device input
TaskAdmission.create(task, AuthenticatedOwner) commits Task + immutable Owner binding
```

`AuthEvidence` may contain only opaque principal handle, assurance/method label, verified time, freshness deadline, one-use challenge nonce, purpose, and adapter version. It must never retain password, biometric template, reusable OS token, or credential bytes. A successful result proves only what the adapter's tested contract says it proves.

| Host | Candidate mechanism to investigate | What it can establish / open questions | Evidence here |
|---|---|---|---|
| macOS | [LocalAuthentication](https://developer.apple.com/documentation/localauthentication) policy evaluation for fresh user presence; Keychain/Secure Enclave access controls may protect host-held keys | Login/session and Keychain access are not by themselves a stable human Owner identity. Determine policy fallback, remote-session behavior, freshness, principal mapping, and OS support matrix. | Public API documentation reviewed; **no platform adapter test. Design only.** |
| Windows | [Windows Hello UserConsentVerifier](https://learn.microsoft.com/en-us/uwp/api/windows.security.credentials.ui.userconsentverifier); DPAPI or platform key protection for local secrets | Hello/user verification, OS account identity, process session, and DPAPI-protected bytes are separate claims. Define edition/version, remote desktop behavior, recovery, and which assurance the API actually reports. | Public API documentation reviewed; **no Windows adapter test. Design only.** |
| Linux | PAM-mediated local authentication or desktop authorization agent, selected per supported distro/session; see [Linux-PAM](https://github.com/linux-pam/linux-pam) and [polkit](https://polkit.pages.freedesktop.org/polkit/) | PAM modules and desktop agents vary; headless/SSH, remote sessions, biometric plugins, and freshness semantics differ. Do not claim one portable mechanism until target matrix is tested. | Public project/API documentation reviewed; **no Linux adapter test. Design only.** |
| Future Android participant | [BiometricPrompt](https://developer.android.com/identity/sign-in/biometric-auth) and Android Keystore/device-bound keys for device-local verification/possession | Biometrics and a registered device key prove neither the local Serea Owner identity nor elevated action approval. Future pairing is distinct from Owner bootstrap. Root and sensitive permissions remain outside scope. | Public API documentation reviewed; **not in repository and not tested.** |

Keep these facts distinct: **OS account possession**, **logged-in session**, **fresh user authentication**, **registered-device possession**, **Task Owner identity**, and **elevated action approval**. None substitutes for another without an explicit ratified rule. Device loss/revocation must not transfer Owner identity. OS authentication unavailable means fail closed for Owner-bound Memory operations; do not silently fall back to session login or a fake backend.

**Alternatives:** Owner per OS account; one owner per device; device-key-only Owner; multi-owner from V1. ADR-0042 favors one local Owner established by host-verified OS authentication; the other models require a separate direction decision.

**Security consequences:** immutable admission binding and non-forgeable context reduce confused-deputy/cross-owner risks. Local administrator compromise can still alter files, memory state, or adapter code. No adapter can make a compromised OS trustworthy.

**Evidence:** typed synthetic Owner context and owner-bound schema/search tests exist. No real OS authenticator, bootstrap, enrollment, revocation, Owner recovery, freshness enforcement, or target OS test exists. The repository does not yet have a production Core composition root to wire an adapter safely.

**Effort:** high; each supported OS needs a separate adapter, threat review, version matrix, UI/freshness behavior, key protection, recovery path, and native integration tests.

**Owner decision B:** supported V1 host OS/version(s): ______; fresh-auth policy: ______; recovery authority: ______; [ ] Accept recommendation  [ ] Modify  [ ] Defer

## C — Forget authority and completion

### Recommendation C (PROPOSED)

Make a separately administered, authenticated append-only ledger the Forget intent authority. A Forget operation uses per-Owner monotonic generation and sequence, a bounded complete closure, content-free chunk frames, and an extraction-generation fence. Persist the authoritative intent before deleting primary Memory. Place all affected Memory behind a read/export/sync barrier until primary tombstones/deletion, ledger `APPLIED`, and ledger finalization agree. A user-visible `PENDING` state is not success; report `COMPLETED` only after reconciled durable state satisfies the selected authority contract.

If any ledger frame is missing, malformed, unauthenticated, out of order, over limit, from an unknown key version, or inconsistent with primary generation, fail closed. Do not expose old Memory to model use, retrieval, export, synchronization, or extraction while quarantined. A complete rollback of all local DB, ledger, key, and manifest evidence cannot be detected without a surviving independent authority; document that limitation rather than assuming hardware monotonic state.

**Alternatives:** same-DB tombstones only; same-host external ledger file; platform secure storage plus authenticated log; remote independent authority. Same-DB and same-rollback-domain approaches cannot prove resurrection prevention. A remote authority changes offline availability and privacy. The exact trust domain remains unselected.

**Security consequences:** selected authority determines whether old snapshots can be distinguished. Incomplete evidence must result in unavailable Memory, possibly indefinitely. No design may claim physical secure erase: SQLite pages, WAL, snapshots, file-system copies, and backups can retain bytes.

**Evidence:** test-only temporary files now exercise HMAC authenticated frames, key versions, sequence parsing, bounded chunks, child-process exit after intent/during SQLite transaction/after commit, restart gate, stale generation, tampering, missing authority, old snapshot, and replay. Ledger and DB files share one local filesystem; this does not prove an independent rollback domain or power-loss guarantees.

**Effort:** very high for production: authority lifecycle, OS/process locking, crash recovery, reserve admission, untrusted restore, device loss, offline semantics, and red-team review.

**Owner decision C:** authority trust domain: ______; completion criterion: ______; quarantine/reset rule: ______; [ ] Accept recommendation  [ ] Modify  [ ] Defer

## D — Ledger MAC key lifecycle

### Recommendation D (PROPOSED)

Use HMAC-SHA-256 with explicit key-versioned frames and constant-time MAC comparison. Bind the exact format version, sequence, OwnerId, generation, operation ID, frame kind, chunk index/count, and target IDs into a canonical length-prefixed frame. Keep old verification keys available for all supported backups until those backups expire or are explicitly invalidated. Rotate by adding a new active key version; never silently reinterpret unknown versions. Missing/lost keys quarantine the affected Owner namespace.

**Alternatives:** platform-provided authenticated storage/secure element; signature-based remote authority; unkeyed hash chain. Unkeyed hashes detect accidental corruption only and do not authenticate a writer. Key storage and rollback authority must be selected independently from the MAC algorithm.

**Security consequences:** MAC detects frame modification only while the verification key is trusted and not rolled back with the log. HMAC does not detect deletion or complete rollback of all evidence. Deterministic test keys are not production key management.

**Evidence:** test-only HMAC implementation passes an RFC 4231 vector; generated synthetic keys exercise versions 1/2, rotation, missing-key refusal, tampering, duplicate/reordered/truncated frames. No production key custody, OS keystore, backup key recovery, key rotation lifecycle, side-channel audit, or real power-loss test exists.

**Effort:** high; platform key adapters and owner recovery must be co-designed with B/C/E.

**Owner decision D:** algorithm: ______; key custody/authority: ______; rotation overlap: ______; key-loss recovery: ______; [ ] Accept recommendation  [ ] Modify  [ ] Defer

## E — Backups and restore

### Recommendation E (PROPOSED)

Support only backups no older than 30 days, and retain Forget evidence for at least 37 days (30 days plus a proposed seven-day restore/reconciliation margin). Bind OwnerId, primary generation, ledger sequence, schema/catalog version, and a digest of the closed primary artifact in a separately authenticated backup manifest. On restore, reconcile that manifest against current independent authority before returning any Memory-capable handle. Older, mismatched, missing-authority, wrong-Owner, wrong-key, or wrong-generation restores stay quarantined. If current external evidence is unavailable, do not downgrade to the backup's local ledger; wait indefinitely or perform an explicit namespace reset that prevents old snapshots from reintroducing the retired Memory.

**Alternatives:** no supported backup restore; longer horizon with proportionally longer tombstones/reserve; remote encrypted backup authority; user-mediated reset and reimport with no old Memory. If backup age is extended, tombstone retention and reserve must be recalculated from maximum restore age, propagation delay, and all replicas.

**Security consequences:** a manifest detects mismatch only if its authority survives independently. A manifest stored beside the backup is not independent rollback evidence. Two local files on the same disk are not separate rollback domains.

**Evidence:** synthetic manifest tests bind closed primary bytes, Owner, generation, and ledger position; stale primary versus newer ledger is refused; restoring both old local artifacts is demonstrably undetectable. No production backup/export/restore path or device-to-device restore is implemented.

**Effort:** high; cross-device backup and recovery require new trusted authority and user-visible restore workflow.

**Owner decision E:** supported age: ______; authority/manifest location: ______; offline behavior: ______; reset semantics: ______; [ ] Accept recommendation  [ ] Modify  [ ] Defer

## F — Production gates

### Recommendation F (PROPOSED)

Require all of the following before Gate D: Gate B selections recorded; separate Gate C implementation authorization; migration 0009 reviewed and ratified or replaced by an approved migration; production Owner adapter on every supported OS; test-only fake auth impossible to enable in ordinary/optimized builds; persistent ledger/key authority and restore protocol independently reviewed; reserve/exhaustion tests on actual supported filesystems; subprocess/power-loss/crash matrix with precise OS guarantees; P7/P6 adversarial review; confidential data-at-rest review; data deletion, export, support, and incident behavior specified; all relevant CI green; independent security review; explicit Owner enablement authorization.

**Alternatives:** ship limited local-only V1 with no backup/sync; defer P7; enable only PUBLIC synthetic/low-risk product data. Any narrower scope still needs an explicit threat model and cannot silently broaden DataClass or bypass ADR-0042.

**Security consequences:** CI and synthetic tests are necessary but do not prove target filesystem, OS authentication, hardware-key survival, or power-loss behavior. Production enablement is a separate decision after review.

**Evidence:** current P7 code is gated to test/development; migration 0009 does not apply in normal builds; optimized experimental build fails closed. No production Memory route exists and no production authorization is granted.

**Effort:** high; multiple native platforms, migration rollout, support lifecycle, recovery operations, and independent review.

**Owner decision F:** target platforms: ______; required independent review: ______; production enablement prerequisites: ______; [ ] Accept recommendation  [ ] Modify  [ ] Defer

## Decision record

Owner: ______  Date: ______  Package version: ______

Accepted categories: ______

Modified categories and exact choices: ______

Deferred categories and blocker: ______

This record does not merge PRs, enable production Memory, approve P8/provider effects, authorize real personal-data ingestion, or replace any separate security review and explicit Gate D authorization.

## Current test evidence versus unsupported claims

| Property | Status |
|---|---|
| PUBLIC/PERSONAL classification downgrade rejected in Rust and direct SQL | **TESTED — synthetic SQLite** |
| Owner/class/cross-owner filtering and immutable binding | **TESTED — synthetic SQLite** |
| Counter transitions, concurrent quota admission, inactive delete, and rollback | **TESTED — synthetic SQLite** |
| Ordinary FTS route read-only and progress-handler cleanup | **TESTED — synthetic SQLite** |
| HMAC parsing, key-version rotation/loss, sequence/chunk integrity | **TESTED — test-only keys** |
| Child process exit around primary/ledger ordering and restart reconciliation | **TESTED — child processes on local temporary filesystem** |
| macOS/Windows/Linux Owner authentication | **NOT IMPLEMENTED / NOT TESTED** |
| OS-backed ledger/key custody | **NOT IMPLEMENTED** |
| Durable reserve against actual disk exhaustion | **NOT IMPLEMENTED** |
| Device-independent rollback detection | **NOT IMPLEMENTED / fundamentally absent without surviving authority** |
| Power-loss survival or secure erase | **NOT TESTED / NOT CLAIMED** |
| Production Memory route, real Owner, provider, or personal account | **NOT IMPLEMENTED / NOT AUTHORIZED** |

## Current experiment measurements (synthetic only)

The reproducible corpus/query set is embedded in `p7_japanese_fts_eval_tests.rs` and contains 12 short everyday/factual Japanese and mixed Japanese-English records, including names, calendar facts, numbers, and API terms. Eleven exact substring queries are each run 100 times against the bundled SQLite build. The host-controlled `host-ja-v1` tokenizer is a proposal that emits overlapping Japanese bigrams and ASCII characters before FTS indexing; it is not integrated into the Memory API. Candidate false positives are counted before exact-body confirmation. These are small-corpus observations, not a Japanese quality score.

| Strategy | Full exact answers | Relevant rows retrieved | Candidate false-positive rows | Mean query time observed | FTS shadow allocated pages | Rebuild observed |
|---|---:|---:|---:|---:|---:|---:|
| `unicode61` | 200/1,100 | 500/1,600 | 0 | 22 µs | 20,480 B | 1,012 µs |
| `trigram` | 500/1,100 | 700/1,600 | 0 | 26 µs | 20,480 B | 1,163 µs |
| `host-ja-v1` | 1,000/1,100 | 1,500/1,600 | 400 | 114 µs | 20,480 B | 1,570 µs |

Environment: bundled SQLite **3.53.2**, in-memory synthetic DB, 12 rows, 100 repetitions per query. Latest local run: latency/rebuild values are single-run observations and vary by machine/load. Shadow page size is page-allocated storage (`dbstat`), too coarse to distinguish the three indexes at this corpus size. No cross-SQLite-version comparison was run. `trigram` does not retrieve all two-character Japanese substrings. The custom tokenizer raises coverage on this corpus but has materially higher candidate false positives and latency; this result is not sufficient to adopt it. **V1 recommendation:** retain `unicode61` as the default lexical V1 for its lower observed cost and zero candidate false positives, while explicitly not promising Japanese continuous-text subword recall. Keep `trigram` and host tokenization as future proposals until a broader, owner-approved evaluation corpus and acceptance criterion exist. This recommendation does not change ADR-0006's lexical-only direction.

## Child-process recovery checkpoints

On the local test filesystem, the parent explicitly kills a child after (1) complete authenticated BEGIN+CHUNK frames have been `sync_all`'d, (2) tombstone/delete/quarantine writes are pending inside an uncommitted SQLite transaction, and (3) the primary Forget transaction commits but before ledger APPLIED/FINALIZED. After each kill, the parent reopens the actual temporary files, inspects the base rows/generation/quarantine, then invokes a test-only startup gate. The gate does not return a synthetic Memory-capable Store until it authenticates and replays the closure, appends APPLIED and FINALIZED, verifies primary generation, and clears the barrier. Repeating recovery appends no extra frames. A separate child competes for the create-new ledger writer lock.

The test calls file `sync_all` and uses SQLite's configured file transactions on the current host. This does **not** establish directory-entry durability, power-loss survival, filesystem/controller cache behavior, inter-file atomicity, key secrecy, or a physically independent rollback domain. The file lock is a prototype: a process killed while holding it leaves a stale lock and causes fail-closed refusal; no production stale-lock reclamation policy is implemented. WriteZero is injected through a test writer, not by filling a real filesystem. The known limit remains: rolling back every local artifact, including database, ledger, key, and manifest, restores matching local evidence and cannot be detected without a surviving independent authority.

## Explicitly unimplemented test scenarios

The campaign has not performed actual filesystem-full testing, power-cut testing, cross-device restore, operating-system Owner authentication, production HMAC key storage, Android participation, or SQLite-version portability for tokenizer behavior. Recovery tests do not yet cover extraction and Forget racing through a shared production admission service, an actual concurrent long closure, backup export interruption, or a crash while the lock file is held followed by a process restart. These are **NOT IMPLEMENTED / NOT TESTED**, not passes. No Memory-capable production handle is wired to the test-only ledger gate.
