# ADR-0042: P7 owner-ratified architectural directions

Status: **Accepted in the explicitly stated principles; technical details pending** · Date: 2026-10-12 (explicit owner approval) · Architecture: `serea-arch/2.7.0` (unchanged)

Surfaces affected: future P7 Memory design only. No protocol, wire, schema, migration, or production code changes are made or authorized by this ADR.

## Context

P7 owner review had a prepared A/B/C decision package, but no owner-selected numeric limits or completed deletion-ledger design. On 2026-10-12 the Serea owner explicitly approved the seven architectural directions recorded below. This ADR records only those principles. Its `Accepted` status must not be read as acceptance of any unresolved detail or authorization to implement P7B.

## Decision — accepted principles

1. **P7-D01 Owner identity.** V1 has one local Owner backed by host-verified OS authentication. The host generates a stable opaque `OwnerId` and binds it immutably at Task admission. It is never derived from `DeviceId`, `SessionId`, model output, or an untrusted request. Legacy Tasks without provable ownership are quarantined.
2. **P7-D02 Independent provenance.** Memory retention is independent of ordinary Task retention. Immutable provenance is content-minimized and has no live Task foreign key that obstructs deletion. Explicit authenticated Forget removes selected Memory and its approved derived closure.
3. **P7-D03 SQLite ownership and supersession.** Use additive SQLite schema, owner-scoped composite constraints, verified UNIQUE parent keys, immutable provenance, class-bound Blob references, and a V1 single-successor supersession graph. Self-links, cycles, and cross-owner edges are rejected.
4. **P7-D04 Classification.** Initial Memory may persist only `PUBLIC` and `PERSONAL` under the host-controlled classification contract. `PRIVATE` fails closed until separately approved production protection exists. `SECRET` and `CREDENTIAL` are excluded from ordinary Memory.
5. **P7-D05 Bounds and retention principle.** All listed resource and retention domains must be bounded. Retrieval cannot silently extend retention. Exhaustion cannot cause unsafe eviction or bypass Forget. This principle approves no numeric value.
6. **P7-D06 Lexical retrieval.** SQLite FTS5 is the initial lexical retrieval mechanism. `unicode61` is provisional and is not evidence of adequate Japanese quality. Search must filter owner and class against authoritative rows, bound literal queries, provide stable result semantics, and receive actual target SQLite tests.
7. **P7-D07 Forget and restoration protection.** Forget is authenticated and explicit, deletes transitive derived Memory, uses content-free tombstones and shared-Blob-safe GC, and retains an independent deletion/reconciliation record. Restored snapshots stay inaccessible to search, extraction, model input, and synchronization until authoritative deletion reconciliation completes. Physical secure erase is not claimed.

## Decision — unresolved, not accepted

Candidate quotas (1,000 items/Owner, 4 KiB/item, 4 MiB active body/Owner, 8 MiB FTS index/Owner, 20 extraction starts/day, one concurrent extraction) and candidate retention (WORKING 7 days, EPISODIC 90 days, SEMANTIC 365 days, PREFERENCE 365 days) remain **PROPOSED**. No A/B/C matrix choice is implied. Renewal, quota accounting, tombstone limits/lifetime, GC bounds, deletion reserve, exhaustion handling, durable quarantine, ledger lifecycle and restore protocol, provenance field list, Owner bootstrap/recovery, SQL proof details, FTS query/ranking/pagination semantics, target-build test evidence, and Japanese quality criteria remain unresolved.

In particular, Forget under storage exhaustion must not silently leave Memory retrievable or usable in model egress. Reserve deletion capacity independently of ordinary growth and stop new extraction/storage before exhaustion. If durable deletion proof cannot be committed, affected Memory must be quarantined from retrieval and model egress, the incomplete operation reported truthfully, and recoverable evidence preserved. An in-memory flag does not satisfy restart safety. The durable quarantine mechanism and its backup/restore behavior require separate design and proof.

## Security boundaries

SQLite constraints and audit records do not provide tamper resistance against arbitrary local file modification. P7B research must document OS filesystem protection assumptions and residual same-user process risk. Before production high-impact provider effects, integrity and rollback protection for Policy, Approval, Grant, and Dispatch authority requires separate review (or a comparably defensible trusted-authority design); integrity failure fails closed for external effects. This ADR makes no security claim against a fully compromised OS.

`PRIVATE` remains disabled until separately approved production protection exists. Existing Accepted contracts, frozen protocols, migrations 0001–0008 and the GoalLatch integration gate are unchanged. No physical secure erase is claimed.

## Consequences and implementation status

The owner direction is ratified, while P7 technical contracts remain incomplete. P7B implementation is **not authorized** and remains blocked by unresolved parameters and proofs. No migration, Rust code, Memory, FTS, or Forget behavior is authorized here. A separate owner authorization is required before P7B work. Architecture remains `serea-arch/2.7.0`; any future frozen-contract version impact must be reviewed under Protocol Index change control rather than inferred from this ADR.

See the [P7 owner-ratification record](../plans/P7-owner-ratification-record.md) for the auditable scope, candidate values, blockers, and next decisions.
