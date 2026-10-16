# ADR-0044: P8 durable dispatch V1 authority directions

Status: **Accepted within the exact Owner-approved scope below** · Date: 2026-10-16 · Architecture: `serea-arch/2.8.0`

Surfaces affected: Capability Protocol, Task Protocol, ADR-0036 P8 dispatch
retry semantics, and development-only schema/runtime candidates. No provider,
credential, production Memory, or production migration enablement is authorized.

## Owner decision

The Owner explicitly approved these P8 directions on 2026-10-16:

1. **Dispatch authority:** one host-owned typed Dispatch Transaction re-reads
   and revalidates the current Task/Step, exact Lease, Policy, Approval,
   deadline, cancellation and Registry state. Commit of its Dispatch Intent is
   the authority point. Point-in-time evaluation and model claims are not
   authority.
2. **Atomic consumption:** preserve Accepted ADR-0039 Option B. Applicable
   Grant use, Budget, IDK reservation, RequestId and Dispatch Intent commit
   together in one SQLite transaction. Provider IO starts only after commit
   and no write transaction spans provider IO.
3. **Consumed Grant:** the use row is historical evidence for the committed
   logical action. It is not permission for another effect. Initial V1 has no
   automatic same-Step effect retry; a new logical action uses a new Step and
   current authorization.
4. **AMBIGUOUS:** initial V1 is reconciliation-only, even with `NATIVE`
   idempotency. No blind or automatic effect replay is authorized. Conflicting
   Capability Protocol, Task Protocol and ADR-0036 text is narrowed accordingly.
5. **Late outcome:** Option A, a separate typed reconciliation/history path,
   is the target. Option C remains the fail-closed interim. A cancelled Task is
   never revived and an unresolved late response is not a canonical Receipt.
6. **Credentials/providers:** production CredentialStore, OS authentication,
   live accounts and real providers are deferred. Fake Provider tests use no
   real Credentials.
7. **Synthetic proof:** deterministic no-network Fake Provider, fault
   injection, restart and concurrency testing are required. No exactly-once
   external effect guarantee is claimed.

Migration Option A is adopted for development: immutable 0001–0008, a reviewed
canonical P7 schema candidate at 0009 and P8 additive candidates at 0010+.
The synthetic P7 v9 fixture remains a distinct disposable fixture and is not
the canonical schema.

## Normative boundaries and unresolved details

This ADR accepts the directions above, not every proposed state machine or
storage detail in the P8A package. In particular, it does **not** define:

- every legal Task/Step transition at dispatch commit or recovery;
- whether a committed but not yet invoked intent can ever be classified as
  definitely uninvoked after process failure;
- authorization freshness/serialization under every concurrent revocation;
- a same-Step manual retry or future automatic retry path;
- reconciliation sources, provider authority, or a proof of no effect;
- late Step status, Receipt eligibility, retention/Forget behavior, or result
  staging requirements;
- an intent retention horizon or production migration/backfill procedure.

Those items remain `PROPOSED`. Until a clause is fully specified and reviewed,
the affected effecting/reconciliation path stays fail-closed. An unresolved
intent may not be deleted and then treated as permission to dispatch again.

## Consequences

- ADR-0039 Option B is unchanged.
- Capability Protocol §8.1 and Task Protocol §6 apply the initial V1
  reconciliation-only/no-automatic-effect-retry rule.
- An intent commit is necessary authority for provider IO, but this ADR does
  not make an intent alone proof that IO happened or did not happen.
- Development-only P7/P8 schema candidates and tests may run only on isolated
  branches and disposable databases. Their existence does not enable
  production Memory or provider execution.
- Existing personal databases are not migrated. No Gate D or production
  feature is enabled.

## Verification obligations

Before any real provider is considered, P8 must prove atomic dispatch
accounting, current-state fencing, provider-IO-after-commit, no effect replay
after AMBIGUOUS, deterministic fake effect-ledger consistency, cancellation and
late-result fail-closed behavior, crash/restart recovery, and exact Receipt
cardinality. Platform security review and separate Owner authorization remain
required.
