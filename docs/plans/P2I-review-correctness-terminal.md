# P2I Terminal Correctness / Durability Review

- **Review identity:** `p2i-correctness-terminal-14017e3-20261006`
- **Review mode:** coordinator review, read-only source/contract/test-assertion inspection; terminal synthesis over bounded coverage ledgers plus independent high-risk source spot-checks.
- **Scope:** P2A `10622803fc52e7ed8e91de5ce41f472b09793766` through P2H `14017e3981cf23ac2b4a076aad435f16eb59b535`, plus current P2I diff.
- **Prior lineage:** `P2I-review-correctness.md` is retained as generation-0 / incomplete evidence and is not replaced or represented as PASS.
- **Coverage inputs:** `P2I-review-coverage.md`, `P2I-recovery-coverage.md`, `P2I-vendor-provenance.md`, phase closure records, current Git diff and source/test checks described below.
- **P3:** excluded; no production implementation started.

## Independent spot-checks for terminal synthesis

This pass independently re-opened and checked the highest-risk implementation surfaces, not just the ledgers:

1. **Canonical/protocol:** `crates/serea-protocol/src/canonical.rs` lexically rejects fractions/exponents/negative zero/out-of-domain integers before serde normalization; parser end-of-input is enforced; visitor checks decoded keys before parsing duplicate values; depth is bounded during parse; serializer explicitly byte-sorts keys. The exact IDK-1 named fields/domain prefix/length-prefix code is in `idempotency_preimage`, with 282-byte literal preimage and hash vectors in `canonical_vectors.rs` and module tests. `canonical_properties.rs` covers escaped/surrogate duplicate names, both integer endpoints, invalid range, full escape set, nesting and UTF-8 ordering. Current protocol registry and schemas inspected; no post-P2A enum or version drift found.
2. **Persistence/authority:** `store.rs` preflights nonempty databases read-only, revalidates on the writable connection, enables WAL/FULL/FK/timeout, applies catalog under migration authority, and keeps checkpoint outcome explicit. `migrate.rs` is the catalog/checksum authority; migration 0001 checksum independently matches `d9068dccbc26ececb71be79c475080633166ba0163c62b2d98b9733512baefea`. `blob.rs` dispatches data class before canonicalization/dedupe, requires PRIVATE backend before reuse, and performs exact class identity plus canonical digest checks. Lease acquisition uses expected generation, SQL conditional ownership, and one attempt charge; renewal/release predicate on authoritative generation/status. `outcome.rs` embeds task/step/lease owner/generation/unreleased predicates in the fenced update and keeps result/ref/receipt/aggregate/release/audit inside the savepoint.
3. **Task engine/recovery:** `transition.rs` has exhaustive state matching and `tests/core.rs` literal 11×11 oracle asserts all 121 pairs. Engine task writes route through typed lifecycle APIs and TaskJournal; candidate plan-step admission checks `steps.len() > 1024` before mutations. Recovery is a single explicit-time audited transaction and restricted `RecoveryPass`; classification and storage `validate_action` agree on lease, attempt, state and receipt evidence; snapshot fingerprints are rechecked immediately before mutation. Receipt repair is allowed only after `prove_receipt_repair`; no recovery path invokes an executor/provider or fabricates outcomes. The two current zero-attempt regressions assert atomic refusal and whole-pass rollback; both passed focused runs in this session.
4. **Crash scope:** P2H harness coordinates separate writer and verifier processes; a returned `Err` is not treated as death, Unix path checks SIGKILL, waits are bounded and timeout kills/reaps. N6 verifier runs in a distinct fresh process and proves complete durable state before recovery. N7 assertions remain integrity/FK stress only. The release proof script was inspected and its completed A/B/C/D run exited 0; it establishes default storage/workspace artifact absence, explicit feature positive control, and seam in the crash-test target only.

## Bounded history coverage

- **A1-1 PASS:** protocol/schema/canonical/vendor cell covered in `P2I-review-coverage.md`; current contracts, Rust implementation, literal vectors and representative regression assertions were reconciled.
- **A1-2 PASS:** migration/profile, blob/class, leases and P2F-a outcome cell covered; implementation chains and representative assertions reconciled. No database-identity isolation guarantee or blanket fsync/power-loss guarantee is inferred.
- **A1-3 PASS:** transition/task/journal/recovery cell covered; 121-pair oracle, provenance, deletion/cancellation surfaces and restricted recovery are mapped. Recovery row-by-row evidence is in `P2I-recovery-coverage.md`.
- **A1-4 PASS:** crash/fault/CI/claims cell covered; exact release boundary and N6/N7 distinctions are retained.
- **A8 PASS:** every required final P2G classification is `COVERED`, with production path, storage predicate, test, mutation/audit/repeat and race disposition where relevant; no `GAP` remains.
- **A11 PASS:** all nine phase closures are reconciled to their commits, direct parents, migration/dependency/protocol claims, present status and historical timeout/nonclaim language. One stale present-tense preimplementation sentence in P2G was corrected; no historical RED/GREEN evidence was rewritten.
- **A12 PASS:** official crates.io archive checksum matched the recorded checksum and local archive SHA-256; extracted source diff contained only the documented numeric patch/test target and provenance/license/harness files. No unexpected production-source delta.
- **A9 PASS:** bounded default-production release-seam property completed exit 0; no arbitrary all-features seam-free claim.

## P2I candidate fixes

- **Task projection cap:** schema already contains `steps.maxItems = 1024`. Runtime now rejects 1025 with no task/revision mutation and accepts 1024; exact 1024 projection passes the AssistantTask schema. Focused regression passed.
- **Failure detail cap:** frozen schema already contains `details.maxProperties = 64`. Runtime requires an object and rejects >64 before the fenced outcome update; 65 leaves step EXECUTING with no error; 64 persists and validates. Focused regression passed.
- **Zero-attempt PLANNED:** existing zero budget is not silently turned into work. Recovery returns payload-free typed `InvalidRecoveryAction` without effects; a mixed-pass test proves an earlier candidate repair rolls back. Both focused regressions passed.
- **Fault-feature wording:** Cargo, module docs and crate-root cfg describe the same boundary: absent from normal/default workspace production; explicit opt-in is seam-bearing.
- **ADR-0021/index:** retained audit-history claim is limited to supported audited task-engine operations; it expressly excludes low-level lease-only changes and deletion-after-cascade.

## Findings and test gaps

No unresolved Blocker or Major was found within the complete frozen P2A–P2H correctness/durability scope. No release-relevant test gap remains in the required matrix. The A8 table includes justified test coverage for reachable decisions and explicit database/API invariants for refusal cases; it does not claim coverage of every physically corrupt SQLite page or every arbitrary local-file writer.

## Residual nonclaims

This PASS does not claim: exhaustive formal proof over every possible SQLite byte state; custom VFS/fsync/power-loss guarantees; Apple Silicon or other-platform empirical runs; complete upstream vendor test-suite execution; complete vulnerability certification of all transitive dependencies; arbitrary feature-combination fault-seam exclusion; external provider acknowledgement/exactly-once side effects; or any deferred P2 nonclaim listed in `P2-closure.md`.

## Verdict

PASS
