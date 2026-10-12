# P7 Owner ratification record — Gate B limited V1 decision

**Decision date:** 2026-10-12. **Record:** ADR-0043. **Scope:** explicit limited V1 Gate B acceptance and disabled Gate C engineering authorization. This entry supplements, and does not rewrite, ADR-0042 or the earlier record below.

## Accepted by the Owner

The Owner accepted these hard per-Owner limits: 1,000 active items; 4,096 UTF-8 body bytes/item; 4 MiB active body budget; 8 MiB logical FTS source budget; 20 extraction starts per UTC day; one concurrent extraction. Accepted base retention is WORKING 7d, EPISODIC 90d, SEMANTIC 365d, PREFERENCE 365d. Retrieval does not renew expiry; active unexpired data is not implicitly evicted; admission is transactional; renewal is explicit and Owner-authenticated.

Also accepted: opaque host-issued local OwnerId; fresh purpose-bound host authentication for sensitive actions; immutable Task Owner binding; legacy Tasks without proof quarantined; authenticated Forget ledger plus SQLite tombstones, generation/reconciliation barriers, and fail-closed uncertainty; HMAC-SHA-256 with explicit key versions/rotation and fail-closed unknown or missing keys. The current PUBLIC/PERSONAL-only boundary and all accepted classification invariants remain in force.

## Not accepted or verified

- Two renewals, 2x/730d cumulative age, GC 100, query 256 bytes, page 20, candidate 1,000, provenance 4 KiB, tombstone 1 KiB, 64 MiB/10,000-frame ledger capacity, and VM instruction ceilings remain recommended/provisional unless individually proven as implementation bounds. The 64 MiB deletion reserve is not a physical capacity guarantee.
- macOS, Windows, and Linux authentication adapters and their freshness/assurance behavior are not implemented or validated. An OS session alone is not fresh authentication.
- Production ledger trust domain, independent rollback-surviving authority, key custody, recovery, platform protection, and trust anchor are not selected or verified. A separate local ledger file is not a separate rollback domain.
- Backup restore and cross-device Memory sync are deferred and disabled for V1. 30d backup age and 37d tombstone retention are not production guarantees. Existing restore work remains synthetic/test-only.
- Independent security review, target-OS validation, real filesystem crash/recovery, confidentiality verification, and explicit production enablement remain gates. Gate D production authorization is not granted.

## Gate C authorization

Owner authorized disabled development-branch implementation of the storage/authentication ports, schema/migration infrastructure, Memory/accounting/search foundations, Forget/ledger/reconciliation state machines, synthetic-key adapters, restore-denial barriers, tests, docs and CI. No actual personal data, account/provider connection, root capability, production secrets, personal database migration, model Memory context, external effect, backup/sync, PR merge, or production activation is authorized.

## Migration transition

Experimental migration 0009 remains test-only and cannot enter normal migration catalogs. Normal storage remains v8. Canonical P7 migration version 0009 is reserved for a later reviewed schema; the experimental DDL must be isolated as fixture material or retired before a canonical migration can use that number. ADR-0043 leaves migrations 0001–0008 untouched.

---

# P7 owner-ratification record

**Date:** 2026-10-12
**Authority:** Explicit Serea owner approval
**Process state:** `P7_OWNER_DIRECTIONS_RATIFIED_DETAILS_PENDING`
**Implementation authorization:** None. P7B implementation is explicitly not authorized.
**Baseline reviewed:** `main` `f70f34c1f683df1b175b612ba100ce6bb6979811`; Draft PR #11 head `5a19dade4ece21223014e6ca2a186956d09906d0`.

This record preserves the owner's approval of exactly the seven architectural directions below. Approval is limited to these principles. It does not complete all P7 technical contracts, approve unresolved values, authorize schema work, alter an Accepted ADR or frozen protocol, approve P8, or relax another security gate.

## Approved directions

| Decision | Owner-approved scope |
| --- | --- |
| **P7-D01 — Owner identity** | V1 uses one local Owner backed by host-verified OS authentication. Generate a stable opaque `OwnerId`, bind it immutably when a Task is admitted, and never derive it from `DeviceId`, `SessionId`, model output, or an untrusted request. Quarantine legacy Tasks without provable ownership. |
| **P7-D02 — Independent provenance** | Memory retention is independent of ordinary Task retention. Preserve immutable, content-minimized provenance without live Task foreign keys that obstruct deletion. Explicit authenticated Forget removes selected Memory and its approved derived closure. |
| **P7-D03 — SQLite ownership and supersession** | Use additive SQLite schema, owner-scoped composite constraints, verified UNIQUE parent keys, immutable provenance, class-bound Blob references, and a V1 single-successor supersession graph. Reject self-links, cycles, and cross-owner edges. |
| **P7-D04 — Data classification** | Initial Memory may persist only `PUBLIC` and `PERSONAL` under the host-controlled classification contract. `PRIVATE` fails closed until separately approved production protection exists. `SECRET` and `CREDENTIAL` are excluded from ordinary Memory. |
| **P7-D05 — Strict bounds and retention** | Bound item count, item bytes, aggregate bytes, index size, extraction frequency, retention, renewal, tombstone storage, and garbage collection. Retrieval cannot silently extend retention. Exhaustion cannot trigger unsafe eviction or bypass Forget. This approves the boundedness principle only; no numeric parameter is approved. |
| **P7-D06 — Lexical retrieval** | Use SQLite FTS5 as the initial lexical retrieval mechanism. `unicode61` is provisional and makes no claim of adequate Japanese search quality. Require owner/class filtering against authoritative rows, bounded literal query handling, stable result semantics, and actual target SQLite tests. |
| **P7-D07 — Forget and restoration protection** | Use authenticated explicit Forget, transitive derived-Memory deletion, content-free tombstones, shared-Blob-safe GC, and an independently retained deletion/reconciliation record. Restored snapshots remain inaccessible to search, extraction, model input, and synchronization until authoritative deletion reconciliation completes. Do not claim physical secure erase. |

## Unresolved parameters and proof obligations

The following candidate initial quotas remain **PROPOSED, not approved**: 1,000 active items per Owner; 4 KiB per item; 4 MiB total active body; 8 MiB FTS index; 20 extraction starts/day; one concurrent extraction. Candidate retention also remains **PROPOSED, not approved**: WORKING 7 days, EPISODIC 90 days, SEMANTIC 365 days, PREFERENCE 365 days. These values do not select any row or column of the existing A/B/C matrix. All other numeric limits and operational details also remain open.

Separate technical decisions and evidence are still needed for exact quota accounting and atomic admission; extraction concurrency/frequency windows; renewal semantics; index-size accounting and rebuild reserve; item/tombstone/ledger retention and byte limits; GC batch/cursor semantics; deletion-reserve size and exhaustion behavior; durable quarantine across restart and backup restore; ledger placement, replication, reconciliation ordering, key custody/loss if applicable, and truthful incomplete-operation reporting; source/provenance fields and Forget target semantics; Owner bootstrap/recovery and OS-authentication port; SQL DDL, exact unique parent keys, trigger/transaction strategy and executable FK/cycle proof; FTS5 build support, query byte/work caps, rank and tie-breaking, pagination, actual Rust target SQLite tests, and representative Japanese quality evaluation.

## Mandatory security decisions

### Forget under storage exhaustion

A full tombstone ledger must never cause Forget to silently leave target Memory available to normal search or model use. Reserve deletion capacity independently from ordinary Memory growth and stop new extraction/storage before the reserve is exhausted. If durable deletion proof cannot be committed, quarantine affected Memory from retrieval and model egress, report the operation as incomplete, and preserve recoverable evidence. An in-memory flag alone is not crash-safe. The durable mechanism that enforces quarantine across restart and backup restoration, exact reserve, exhaustion protocol, and storage bounds require a separate technical decision and proof.

### Local SQLite tampering and production authority

Do not claim that SQLite constraints or audit records resist arbitrary local file modification. P7B research must document the OS filesystem protection assumption and residual same-user process risk. Before production high-impact provider effects, require a separately reviewed mechanism that protects the integrity and rollback of Policy, Approval, Grant, and Dispatch authority, or a comparably defensible trusted-authority design. Integrity failure fails closed for external effects. No claim is made against a fully compromised OS.

## Current implementation blockers and separate production gates

P7B remains blocked on approved numeric and deletion-ledger parameters, a concrete durable exhaustion/quarantine design, Owner authentication/bootstrap/recovery boundary, exact provenance and Forget scope details, and executable additive-schema/UNIQUE-parent/cycle proof. No Memory, FTS, Forget, migration, or Rust implementation exists as a result of this ratification. Target SQLite/FTS tests and the later runtime, restart, backup/restore, GC, adversarial, portability, and Japanese retrieval evidence remain outstanding.

`PRIVATE` persistence remains separately blocked on production protection for ordinary rows, metadata, FTS, WAL, temporary files, backups, key recovery and relevant logs, followed by independent review and explicit approval. High-impact production provider effects remain blocked on trusted-authority integrity and rollback protection. Real providers, GoalLatch integration, and any change to the GoalLatch gate remain outside this decision. No physical secure erase, arbitrary local tamper resistance, or fully compromised OS resistance is claimed.

## Existing contracts and version scope

ADR-0006's FTS5 lexical-only direction, ADR-0010 classification/credential exclusion, ADR-0022's unresolved PRIVATE protection, Task Protocol §8 independent retention/explicit forget, existing Blob identity `(digest,data_class_rank)`, frozen migrations 0001–0008, and the GoalLatch integration gate remain in force. This record does not amend an Accepted ADR or frozen protocol, and does not change architecture/wire version numbers. The architecture registry remains `serea-arch/2.7.0` pending a separately reviewed version-impact decision if implementation changes a frozen contract.

## Technical proposals added after ratification

The following are Gate A engineering proposals only. They are not added to the Accepted principles above and do not complete technical contract ratification:

- [Owner authentication technical contract](P7-owner-auth-technical-contract.md)
- [Numeric bounds and retention proposal](P7-numeric-bounds-owner-decision.md)
- [Forget ledger and recovery design](P7-forget-ledger-recovery-design.md)
- [Isolated SQLite feasibility evidence](P7-sqlite-feasibility-evidence.md) (synthetic probes; scope limits apply)

These materials do not authorize P7B or alter ADR-0042.

The documentation-only Git identity recovery and PR relationship are recorded in [P7 clean-history reconstruction](P7-clean-history-reconstruction.md). The original PR #11 remains audit evidence; replacement history and branch retirement do not ratify technical parameters.

## Next owner decisions

1. Resolve each initial quota/retention value or explicitly defer it with a safe disabled/fail-closed scope; do not choose A/B/C by implication.
2. Resolve deletion-reserve size, capacity accounting, durable quarantine mechanism, exhaustion behavior, and deletion-ledger lifecycle/retention/restore reconciliation.
3. Resolve remaining Owner authentication/recovery, immutable provenance fields, Forget selection details, and SQLite/FTS operational semantics.
4. Review the separate production security gates before any private-data persistence or high-impact provider effect.
5. Give a separate, explicit authorization before any P7B implementation. This ratification is not that authorization.
