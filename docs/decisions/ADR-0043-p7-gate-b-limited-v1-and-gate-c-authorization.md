# ADR-0043: P7 Gate B limited V1 policy and disabled Gate C engineering

Status: **Accepted within the exact scope below** · Date: 2026-10-12 · Architecture: `serea-arch/2.7.0` unchanged

Surfaces affected: P7 implementation policy and development-only implementation authorization. No frozen protocol or ordinary storage behavior changes are authorized.

## Context

ADR-0042 records accepted architectural principles but predates the Owner's explicit limited V1 Gate B choices and Gate C implementation authorization. This ADR records that later decision without amending ADR-0042 or converting unresolved engineering recommendations into accepted production guarantees.

## Owner decision — accepted

The Serea Owner explicitly approved the following on **2026-10-12**:

### V1 resource and retention policy

- Hard per-Owner admission ceilings: 1,000 active items; 4,096 UTF-8 bytes per body; 4 MiB active body bytes; 8 MiB logical FTS source bytes.
- At most 20 extraction starts per UTC day and one concurrent extraction per Owner.
- Retention from creation: WORKING 7 days, EPISODIC 90 days, SEMANTIC 365 days, PREFERENCE 365 days.
- Retrieval never renews expiry. Unexpired active Memory is not implicitly evicted. Admission limits are transactional. Renewal requires explicit Owner authentication.
- Existing PUBLIC/PERSONAL-only Memory direction is unchanged. This approval does not relax the accepted classification protocol or allow SECRET/CREDENTIAL Memory.

### Owner identity and authentication direction

- Local V1 uses one stable opaque OwnerId issued by the host.
- Sensitive operations require fresh, purpose-bound OS authentication. Device possession is not human authentication; an active OS session alone is not proof of freshness.
- Request/model input cannot mint Owner authority. Task admission immutably binds the authenticated Owner. Legacy Tasks without verified ownership remain quarantined.
- macOS, Windows, and Linux adapters, including assurance and freshness behavior, remain to be independently implemented and validated. No platform is declared supported by this decision.

### Forget, ledger, and keys

- Forget uses an authenticated authoritative deletion ledger, primary SQLite tombstone/deletion transactions, per-Owner generation and reconciliation, and startup/read/extraction barriers.
- Uncertainty fails closed. Pending Forget is not completion; no false completion acknowledgement and no physical secure-erase claim.
- A separate ledger file is not by itself a separate rollback domain. Complete rollback of all locally trusted evidence remains undetectable without independently surviving authority.
- Ledger records use HMAC-SHA-256, explicit key versions, checked sequence and generation, and explicit rotation semantics. Missing, unknown, or lost keys fail closed.
- Production key custody, platform protection, recovery, and trust anchor are not approved or verified. No plaintext credentials or key bytes may be stored in ordinary SQLite Memory.

### Backup and restore

- Production Memory backup restore and cross-device Memory synchronization are deferred and disabled for V1. Existing snapshot restoration experiments remain test-only.
- The previously proposed 30-day backup horizon and 37-day tombstone period are not ratified as production guarantees. Unsupported or unverifiable restored Memory remains quarantined.
- Any future Memory backup, restore, or sync feature requires a new Owner decision and independent authority design.

### Production gates and implementation authority

- Production requires independent security review, target-OS authentication validation, real filesystem crash/recovery tests, demonstrated quarantine behavior, verified confidentiality requirements, and explicit feature enablement permission.
- Gate C development-branch engineering is authorized for disabled-by-default Rust storage/recovery, additive schema and migration infrastructure, host identity/authentication ports, Memory APIs, resource accounting, FTS, Forget state machines, synthetic-key ledger adapters, reconciliation, restore-denial barriers, fault injection, tests, docs, and CI.
- This is not Gate D production authorization. No real personal data, account access, root capability, production key, production Memory, model context injection, provider effect, P8 effect, production backup/restore/sync, or existing personal database migration is authorized. PR merge and production enablement are not authorized.

## Recommended values that remain provisional

The Owner's limited approval does **not** resolve every value in the Gate B recommendation. Keep these proposed engineering parameters clearly labeled provisional and disabled unless independently bounded by a test-only implementation: at most two renewals; cumulative retention at most twice base life / 730 days; GC batch 100; query 256 UTF-8 bytes; page size 20; candidate cap 1,000; provenance metadata 4 KiB; tombstone frame 1 KiB; ledger logical cap 64 MiB / 10,000 frames; VM instruction ceiling; physical disk-space or reserve guarantees; any backup/tombstone horizon. The 64 MiB value is not a verified physical capacity guarantee. SQLite VM and filesystem capacity behavior require technical validation.

## Schema, protocol, and migration impact

No wire shape, frozen contract, architecture version, or migration 0001–0008 changes. Experimental migration `0009_p7_synthetic_memory.sql` remains test-only, unratified, and excluded from normal builds. It must not be silently applied to ordinary stores. When a canonical P7 schema is approved for a later rollout, reserve canonical migration version 0009 for its reviewed production schema and remove the experimental migration from the canonical ordered catalog; the experimental DDL remains a standalone disposable test fixture or is retired. Do not ship a production catalog with competing meanings for version 0009. Until that transition is reviewed, normal builds stay at schema v8 and Gate C schema prototypes use isolated synthetic databases.

No protocol or wire-version bump is indicated by these internal implementation contracts. Revisit architecture/protocol change control if later work changes any frozen interface or invariant.

## Integration order

Keep PR #12 and PR #13 history unchanged. PR #14 is the canonical independent P5 concurrent-manifest fix. For eventual integration, land the independently reviewed main-based PR #14 before integrating PR #13; after base updates, use a normal merge from the updated base into PR #13 and resolve the duplicate P5 hunk to PR #14's canonical patch. Do not rewrite public history or merge automatically. Gate C remains stacked on PR #13 until separately reviewed.

## Consequences and nonclaims

Gate C code is engineering work only and must remain unreachable from ordinary operation. Rust tests/CI do not establish production readiness. No production trust anchor, key custody, platform authenticator, power-loss guarantee, physical secure erase, backup support, complete rollback detection, or P8 authority is established here.

See [P7 owner-ratification record](../plans/P7-owner-ratification-record.md) and [Gate B package](../plans/P7-Gate-B-owner-decision-package.md) for source context and implementation evidence.
