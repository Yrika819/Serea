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

Concrete clause candidates remain **PROPOSED**, not Accepted:

1. **Budget:** count every committed effect Dispatch Intent against the Task's `max_tool_calls`; enforce the limit in the same SQLite write transaction as Grant use and Intent. No same-Step second intent is allowed. Existing schema has no durable capability-call counter, so this needs an additive counter design and agreement on whether non-effect capability calls count.
2. **Commit-to-invoke:** after a successful intent commit, the live process may make one provider invocation for that intent. A crash, lost acknowledgement or restart after commit must never invoke that intent again automatically; until reconciliation authority is approved, unresolved results become `AMBIGUOUS`/Option C closed. The `AMBIGUOUS` transition itself is not yet implemented or accepted for capability effects.
3. **Revalidation:** one `BEGIN IMMEDIATE` transaction must compare Task cancellation/deadline/state revision, Step/plan revision, active Lease owner/generation/expiry, current Policy revision, Grant status/member/revocation/expiry, pinned Registry descriptor, and Overlay revision before consuming authority. Any mismatch rolls back all rows/events. Provider health has no SQLite authority source; unavailable or unprovable provider readiness refuses before commit.
4. **Event:** append the bounded Intent event in that same transaction. Do not store a copied event body or assume a retention hold from an Intent foreign key; define only a correlation ID until the retention interaction is approved.
5. **Result and Receipt:** accept a result only for the exact RequestId and IDK after host validation. A response received after cancellation/deadline or with unresolved commit provenance does not complete the Task and creates no canonical Receipt (Option C). Duplicate or mismatched responses are quarantined/refused. Late reconciliation remains blocked pending Option A evidence and eligibility rules.

Each clause above changes detailed semantics beyond the approved direction and requires Owner acceptance before becoming normative runtime behavior. Development probes may test rollback and uniqueness; they cannot authorize a provider effect.

Do not add this file to the normal migration catalog. Do not invoke a provider or construct a canonical Result/Receipt based on this candidate. The current safe runtime behavior is no capability effect. P8C is blocked until the atomic authorization and invoke boundary can be implemented with all accepted checks; unresolved transitions and late outcome authority remain fail-closed under ADR-0044.

## Validation boundary

The focused P8 candidate test passed. P7 migration selector passed 14/14. Workspace all-target and all-feature tests passed (the candidate is covered by the opt-in feature run). Rust 1.85.0 workspace all-target/all-feature check, Clippy with `-D warnings`, format, documentation validation, and diff checks passed. GitHub Native/SQLite workflows are still pending on dependency PRs. This is synthetic schema validation, not production migration, security review, or physical power-loss evidence.
