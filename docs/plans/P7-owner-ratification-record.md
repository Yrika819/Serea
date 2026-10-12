# P7 owner-ratification record

**Date:** 2026-10-12
**Authority:** Explicit Serea owner approval
**Process state:** `P7_OWNER_DIRECTIONS_RATIFIED_DETAILS_PENDING`
**Implementation authorization:** None. P7B implementation is explicitly not authorized.
**Baseline reviewed:** `main` `f70f34c1f683df1b175b612ba100ce6bb6979811`; Draft PR #11 head `d23b2163193aa1856390afb2b6ae4c8a0e09e1e9`.

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

## Next owner decisions

1. Resolve each initial quota/retention value or explicitly defer it with a safe disabled/fail-closed scope; do not choose A/B/C by implication.
2. Resolve deletion-reserve size, capacity accounting, durable quarantine mechanism, exhaustion behavior, and deletion-ledger lifecycle/retention/restore reconciliation.
3. Resolve remaining Owner authentication/recovery, immutable provenance fields, Forget selection details, and SQLite/FTS operational semantics.
4. Review the separate production security gates before any private-data persistence or high-impact provider effect.
5. Give a separate, explicit authorization before any P7B implementation. This ratification is not that authorization.
