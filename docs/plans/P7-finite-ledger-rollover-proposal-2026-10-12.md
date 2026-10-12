# P7 finite-ledger exhaustion and rollover proposal — 2026-10-12

**Status:** unresolved production architecture proposal; not approved or implemented. This document does not amend ADR-0042/0043 or authorize production Memory, ledger compaction, migration, or enablement.

## Constraint

An append-only ledger with finite frame/byte limits and a finite checkpoint history cannot guarantee an unlimited number of Forget operations. HMAC authenticates records under a key; it does not establish that a presented stream is the latest stream. A local checkpoint that can be rolled back with the ledger is not an independent freshness authority. Increasing a cap or checking free disk space does not solve either problem.

The current synthetic fixture therefore remains bounded and fail-closed. Admission refuses before any durable operation transition if the full encoded Forget record set or the next checkpoint revision does not fit. The deployment-level behavior after evidence exhaustion remains a production blocker.

## Candidate future architecture

1. **Historical commitment:** each closed authority epoch commits to the exact prior ledger tip and an authenticated index of retired `(owner_id, memory_id)` identities. The index must support exact membership/non-membership proofs; a probabilistic structure cannot be the sole anti-resurrection evidence.
2. **Authority epoch handoff:** a new epoch record binds the prior epoch, final sequence, generation, key version, historical commitment root, and the new epoch key. The handoff requires verification by both the retiring and successor authority keys, plus the independently trusted freshness authority.
3. **Crash-safe rollover:** write and flush a complete new epoch ledger and checkpoint in temporary names; reopen and verify every byte and commitment; persist a handoff intent; publish an authenticated current-epoch manifest atomically; reopen and verify the manifest; only then may old frame storage be reclaimed. Every crash point must either select the old epoch or prove the new epoch complete. Keep the old epoch until the manifest and successor authority are confirmed.
4. **Anti-resurrection retention:** keep the exact forgotten-ID index or a proof-capable immutable commitment and enough authenticated data to prove membership after rollover. Never discard tombstones solely because they are old or because a primary snapshot is believed expired.
5. **Independent freshness authority:** the current epoch number and commitment root must survive rollback of the application database, ledger files, checkpoint directory, and local key material. A separate OS process, local file, or same-host keychain value is not automatically independent. A verified remote monotonic authority or platform-backed monotonic primitive would need target-specific threat analysis and Owner approval.
6. **Key-version continuity:** retain old verification keys until every retained epoch handoff and proof is authenticated by the successor chain. Unknown or missing keys deny reads and Forget completion. Key retirement itself is an authenticated authority transition.
7. **Exhaustion and retirement:** stop ordinary Memory growth and extraction before the protected deletion budget is exhausted. If the current authority cannot durably accept another Forget, deny all Memory visibility and report an explicit incomplete state; do not delete evidence to regain capacity. Namespace retirement/recreation requires an approved process that preserves old anti-resurrection commitments and binds the successor Owner epoch.

## Required gate before implementation

Specify and independently review the trust anchor, epoch handoff state machine, proof format and bounds, crash-safe manifest publication on every supported filesystem, exact retired-ID lookup, key continuity, recovery after partial rollover, and fail-closed namespace retirement. Demonstrate target-platform fault injection and a surviving freshness authority. Until then there is no production claim of uninterrupted Forget, compaction safety, or anti-resurrection across complete local rollback.
