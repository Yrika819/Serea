# ADR-0046: P8B authority ownership and Host invocation boundary

**Status:** PROPOSED / NOT RATIFIED · **Date:** 2026-10-17 · **Architecture:** `serea-arch/2.8.0` unchanged

## Context

The P8B Cargo and SQLite proof in
[P8B authority architecture proof](../plans/P8B-authority-architecture-proof-2026-10-17.md)
compared a TaskEngine-owned authority seam, a self-validating public Storage
operation, and a Host-owned composition seam. The present workspace has no
production Dispatch Intent writer, cross-crate Permit factory, or Capability
Provider invocation path. The candidate P8 SQL is outside migrations 0001–0008.

ADR-0045's Owner-ratified D1/D2/D3/D5 directions and D4's interim unresolved
evidence preservation are unchanged. They do not select an authority crate,
API, schema, Provider freshness contract, Event payload, Receipt, retry, or
retention lifecycle.

## Proposed architecture for Owner selection

**C — Host binary-owned dispatch coordinator, with TaskEngine-owned task and
Policy revalidation and Storage-owned atomic commit mechanics.** This is the
recommended topology for a bounded next proof, not an accepted architecture.

- Keep the invocation coordinator and the opaque, non-Clone, non-serializable
  one-shot Permit inside the `serea-host` executable's private composition
  module. The current Host package is binary-only, so this avoids creating a
  reusable public Provider invocation surface.
- Keep Task/Step/Plan/Lease and P6 Policy/Approval decisions in their existing
  owners. The TaskEngine must re-read transaction-dependent facts at the
  serialized commit point; point-in-time `PolicyEvaluation` is not authority.
- Keep SQLite serialization, exact Grant member use, Task Budget, IDK,
  RequestId, immutable Intent, and the required `CAPABILITY_REQUESTED` append
  in one Storage transaction. Storage must not trust a caller boolean,
  synthetic Owner, arbitrary `PolicyEvaluation`, or unverified readiness
  snapshot as authorization.
- Host owns the exact live Provider instance and performs fresh readiness
  checks before commit and immediately before invocation. Provider I/O remains
  outside the SQLite write transaction. A stale or mismatched binding refuses;
  a post-commit failed attempt consumes the committed authority and is not
  silently reissued.
- The Permit is created only in the Host composition after the transaction
  reports a committed, fully revalidated Intent. Every call consumes it by
  value, including mismatches. It is never reconstructed from SQLite.

One plausible acyclic dependency shape is:

```text
serea-host (binary) -> serea-task-engine, serea-policy, serea-storage, serea-capability, serea-protocol
serea-task-engine -> serea-policy, serea-storage, serea-event-bus, serea-capability, serea-protocol
serea-policy -> serea-storage, serea-event-bus, serea-protocol
serea-storage -> serea-protocol
```

The existing workspace has no normal dependency cycle, but this proposal still
needs an executable API and private-module proof. It may require a narrow typed
Storage/TaskEngine revalidation seam. Any proposed public Storage commit method
must be safe for arbitrary consumers of the compiled crate; crate convention
or an unchecked callback is not that proof. Any Policy extraction or crate
reorganization is blocked until Owner approval.

## Alternatives and evidence

- **A — TaskEngine alone:** unproven. Current `evaluate_prepared_action` reads
  current state before returning point-in-time evidence. The public Store
  transaction surface remains separately callable, and TaskEngine does not own
  the live Provider instance. A complete TaskEngine transaction seam was not
  implemented or externally compiled in this spike.
- **B — public self-validating Storage operation:** refuted under the current
  graph as a complete authority owner. `serea-policy` already depends on
  `serea-storage`, so Storage importing Policy creates a cycle. Storage also
  cannot establish which live Host Provider instance supplied readiness. Its
  `Tx` prevents raw SQL escape, but existing public typed grant-use and Event
  operations remain reachable through `Store::transact`; a new safe operation
  would need an explicit bypass audit.
- **C — Host composition with typed lower-layer commit:** plausible but
  unproven. The dependency direction is acyclic and Host owns the live
  Provider instance, but exact TaskEngine/Storage API ownership, arbitrary
  consumer safety, freshness synchronization, and commit-to-Permit issuance
  have not been proved.

## Security assumptions and limits

This boundary is intended to protect against untrusted model, user, and
Provider inputs reaching the trusted compiled Serea Host. It does not
authenticate the physical caller of a public Rust method. Arbitrary code
loaded into the same process, a fully compromised Host process, or an actor
able to modify/restore the SQLite files is outside Rust privacy's guarantee.
No exactly-once external effect, rollback resistance, physical power-loss
durability, production retention, or Provider availability guarantee is
implied.

## Owner decision requested

Approve C for a narrowly scoped implementation proof, approve a revised A/B/C
topology, or decline until a new contract is supplied. An approval must name
the crate that owns the atomic authority decision, the private Permit boundary,
the Host readiness/instance binding contract, and the transaction inputs that
must be re-read. This proposal does not authorize a migration, public dispatch
API, Provider invocation, architecture version change, Receipt, retry,
Production Memory, or production retention policy.

## Existing ratifications retained

- D1, D2, D3, and D5 remain Owner-approved directions within ADR-0045's exact
  scope; D4 remains interim unresolved-Intent evidence deletion refusal only.
- ADR-0036, ADR-0039 Option B, and ADR-0044 remain unchanged.
- Topology C, all new APIs, data shape, exact Event payload, freshness timing,
  lifecycle disposition, Receipt/reconciliation details, and retention remain
  Proposed or Blocked.
