# P8B 0010 dispatch schema candidate audit — 2026-10-16

**Status:** DEVELOPMENT SHAPE PROBE ONLY; not an accepted migration

**Dependency:** Draft PR #21, branch `p7/schema-v9-candidate`

**Candidate:** `crates/serea-storage/migrations/candidates/0010_p8_dispatch_candidate.sql`

This candidate is applied only by a `p7-experimental` unit test to an in-memory disposable database after the separate P7 0009 candidate. It is outside the normal migration catalog, `Store::open`, and production migration path.

## Recorded data shape

The prototype freezes one row per Task Step with unique RequestId, IDK, and Step identities. It correlates Task, Step, Owner binding, Grant use, plan revision, Task state revision, Lease generation, Policy revision/digest, pinned registry descriptor, overlay revision, an event identifier and commit time. Updates are refused. It has no mutable lifecycle status and no Receipt field. The event identifier intentionally does not reference event storage, avoiding an unreviewed retention hold.

The schema runner probe passed and verified the candidate checksum, required columns, no `state`/`receipt_id` columns, clean SQLite integrity/FK checks, and exclusion from `Migrations::embedded()`.

## PROPOSED blockers; no runtime authority

The prototype does not establish a callable dispatch transaction. Exact tool-budget accounting, grant use alongside budget consumption, live Policy and Grant revalidation, Task/Step/Lease admissibility, provider availability evidence, Registry/Overlay revalidation semantics, event append atomicity, commit-to-invoke eligibility, and definite versus ambiguous outcomes remain unresolved at the runtime boundary. No public Storage intent-write API exists. No TaskEngine path calls `CapabilityProvider::invoke`.

Do not add this file to the normal migration catalog. Do not invoke a provider or construct a canonical Result/Receipt based on this candidate. The current safe runtime behavior is no capability effect. P8C is blocked until the atomic authorization and invoke boundary can be implemented with all accepted checks; unresolved transitions and late outcome authority remain fail-closed under ADR-0044.

## Validation boundary

The focused P8 candidate test passed. P7 migration selector passed 14/14. Workspace all-target and all-feature tests passed (the candidate is covered by the opt-in feature run). Rust 1.85.0 workspace all-target/all-feature check, Clippy with `-D warnings`, format, documentation validation, and diff checks passed. GitHub Native/SQLite workflows are still pending on dependency PRs. This is synthetic schema validation, not production migration, security review, or physical power-loss evidence.
