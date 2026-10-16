# P8 Owner-authorized directions — 2026-10-16

**Authority:** Owner mandate in
`Serea_Codex_Cloud_P8_Long_Horizon_Mandate_2026-10-16.md`, delivered
2026-10-16.
**Scope:** synthetic P8 development and contract reconciliation. This record is
not production authorization and does not resolve details absent from the
mandate.

## Adopted direction

| ID | Owner-authorized direction | Runtime consequence |
| --- | --- | --- |
| D01 | A single host-owned Dispatch Transaction revalidates current Task/Step, exact Lease, Policy, Approval, deadline, cancellation and Registry state. Commit of its Dispatch Intent is the authority point. | No point-in-time evaluation or model claim authorizes invocation. Exact state transition and commit-to-invoke handling remain fail-closed where current contracts do not decide them. |
| D02 | Preserve Accepted ADR-0039 Option B: Grant use, Budget, IDK reservation, RequestId and Dispatch Intent commit atomically in SQLite. Provider IO begins only after commit. | Any failed transaction has no dispatch authority. Provider IO is outside the SQLite write transaction. |
| D03 | A consumed Grant records the same logical operation and never grants a new effect. V1 has no automatic same-Step effect retry. A new operation requires a new Step and current authorization. | Historical grant use cannot authorize another invoke. |
| D04 | AMBIGUOUS is reconcile-only in V1. NATIVE idempotency does not permit blind or automatic effect replay. Resolve conflicting Capability Protocol, Task Protocol and ADR-0036 language. | No effecting replay after AMBIGUOUS. Until exact amendments are landed, the more restrictive rule controls synthetic dispatch and the affected runtime path is blocked. |
| D05 | Target a separate typed late-outcome reconciliation/history route (Option A). Keep Option C fail-closed until that route is fully specified. | Never revive a CANCELLED Task. Do not create a normal success or canonical Receipt from an unresolved/late response. |
| D06 | Defer production CredentialStore, OS authentication and real providers. Fake Provider uses no real Credentials. | No real secret, account, network or provider access in this work. |
| D07 | Require a deterministic Fake Provider with fault injection, process restart and concurrency tests. | Synthetic evidence must inspect independent SQLite state and fake effect ledger; no exactly-once claim. |

Migration direction is **Option A**: immutable migrations 0001–0008; canonical
P7 schema candidate 0009; P8 additive candidates 0010+; the P7 experimental v9
fixture remains separately named and never serves as canonical schema merely
because it is usable in a fixture.

## Contract reconciliation ledger

| Surface | Current text / concern | Disposition |
| --- | --- | --- |
| ADR-0039 | Option B is already Accepted. | Preserve unchanged; implementation must join consumption to the dispatch transaction. |
| ADR-0036 | Describes a same-Step retry with the same IDK and a new RequestId. | Amend only the retry authority to the D03 V1 restriction. Any later retry capability requires a separate decision. The exact retry budget/accounting semantics remain `PROPOSED`. |
| Capability Protocol §6.2 | Says AMBIGUOUS is never blindly retried and requires reconciliation. | Consistent direction. Amend text to state V1 is reconcile-only, including `NATIVE`, and no automatic effect replay. Exact read-only reconciliation sources and authority remain `PROPOSED`. |
| Capability Protocol §8.1 | Current table says IDEMPOTENT retry on AMBIGUOUS is allowed; CONDITIONAL has same-Step retry language. | Contradicts D03/D04. Amend V1 column to forbid AMBIGUOUS replay and automatic same-Step effect retry. A new logical operation needs a new Step and fresh authorization. Provider retention horizon and reconciliation completion remain `PROPOSED`. |
| Task Protocol §6 | Expired-lease recovery says an IDEMPOTENT Step is re-issued and describes other replay rules. | Amend P8-specific recovery to inspect the exact durable intent. An unresolved intent is reconcile-only; absence of an intent is not itself an authorization and must pass a fresh dispatch gate. No new Step/Task transition is introduced here. |
| Task / Capability Protocol late outcome | A CANCELLED Task is terminal; existing normal outcome and Receipt path does not admit a late result. | Keep Option C fail-closed. Option A data model, Step eligibility, Receipt authority, retention and Forget holds remain `PROPOSED`, so no canonical late Receipt is permitted. |

## Explicitly unresolved (PROPOSED; not Accepted)

- Exact legal Task/Step transition at intent commit, including post-commit
  cancellation and deadline races.
- Whether an intent committed but not yet invoked can be closed as definitely
  uninvoked after a crash. No such conclusion may be inferred from a marker
  whose durability/ordering is not proven.
- Exact authorization revalidation serialization for concurrent Policy,
  Grant, registry, overlay, availability and cancellation changes.
- Any same-Step retry command, manual retry, budget interpretation, or fresh
  approval requirements. Default V1 has no automatic effecting same-Step retry.
- Read-only reconciliation authority and what evidence establishes exact
  provider absence or occurrence.
- Late result Step status, canonical Receipt eligibility, retention hold,
  Forget interaction, and permanent-history rules.
- P7 OwnerId issuer/grammar details not already ratified; exact v9 backfill,
  quarantine release, provenance and FTS/Blob/tombstone details where existing
  accepted contracts do not decide them.

Until those details have a reviewed contract, the affected operation returns a
typed refusal or remains unresolved. This document does not authorize a
provider effect, canonical Receipt, production Memory, or migration of an
existing personal database.
