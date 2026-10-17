# ADR-0045 (DRAFT): P8B Owner-ratified dispatch contract boundaries D1–D5

**Status:** Draft repository transcription of Owner-ratified directions; no merge or runtime activation is implied. **Decision date:** 2026-10-17. **Architecture baseline:** `serea-arch/2.8.0` (no version change approved). **Source:** Owner supplied and explicitly approved the exact directional clauses in the attached ADR-0045 draft during this task. See [execution plan](../plans/P8B-owner-ratified-execution-plan.md).

## Decision

The Owner approved D1, D2, D3 and D5 as design directions, and approved only the interim unresolved-Intent deletion refusal under D4. These approvals authorize bounded Synthetic engineering and documentation. They do not approve unselected numeric parameters, schemas, authority APIs, production behavior, or merge.

### D1 — Task tool-call Budget

**OWNER-APPROVED direction:** Each durably committed Provider Dispatch Intent consumes exactly one Task `max_tool_calls` unit for both OBSERVE and EFFECT. Purely local OBSERVE with no committed Provider Intent, precommit refusal and duplicate suppression without an Intent consume zero. A committed Intent is charged permanently, without refund after failure, cancellation, timeout, crash or ambiguity. `max_attempts_per_step` and model call count do not substitute.

**Existing accepted constraints:** ADR-0036 committed-Intent accounting and ADR-0039 joint transaction for applicable Grant-use, Budget, IDK, RequestId, Intent and required Event; one committed logical Step cannot authorize a second same-Step effect.

**Proposed:** Durable counter vs Intent-derived representation, exact schema, overflow, integrity/correlation, reconciliation accounting, Task expiry/deletion, backup/restore and legacy migration. Minimum Synthetic acceptance includes budgets 0/1/2, independent competing Stores, rollback, restart/lost acknowledgement, no replenishment after corruption/deletion, and fail-closed overflow/exhaustion.

### D2 — Provider identity and readiness

**OWNER-APPROVED direction:** Host owns the exact live in-process Provider instance. It observes fresh readiness before dispatch commit and again immediately before invocation. Authorization binds RequestId, ProviderId and exact instance, descriptor digest, Registry generation and Overlay revision. Missing, mismatched, unready, disabled or unproven Provider refuses. Persisted `ready=true`, model claims, default `health() = Ready`, and stale snapshots are not independent authorization evidence. Readiness does not guarantee future availability.

**Proposed:** Freshness interval, trusted Host instance identity, lifecycle/registry locking, asynchronous health representation, atomic revision revalidation, postcommit disposition/telemetry, and same-OS-user compromise protection. No exactly-once or guaranteed readiness claim is permitted.

### D3 — one-shot Permit

**OWNER-APPROVED direction:** Permit is opaque, authority-owned, non-Clone, non-serializable, in-process, consumed by value on every invocation attempt including a mismatched binding. Only a successfully committed and fully revalidated Intent may cause issuance. Wrong RequestId/IDK, Step, Lease generation, Owner, Provider instance, descriptor, Registry generation or Overlay revision cannot invoke and does not return a usable Permit. Never reconstruct it after restart.

Before atomic commit, cancellation, expiry, invalid/revoked Grant, Policy/Registry/Overlay mismatch or stale Lease refuses without partial writes. Postcommit cancellation does not grant a second call; CANCELLED stays cancelled. Unresolved committed Intent after crash/lost acknowledgement is reconciliation-only; no automatic same-Step effect replay, including for NATIVE idempotency. Intent commit authorizes an attempt, not proof of Provider I/O.

**Existing accepted constraints:** ADR-0044 D01–D07, ADR-0039 Option B, TaskEngine lifecycle ownership, no Provider I/O in SQL write transaction, and interim Option C no canonical late Receipt.

**Proposed:** Crate/API ownership and proof interface, Permit internals/lifetime, commit-to-invoke scheduling, cancellation/readiness ordering, typed outcome/event mapping, Receipt cardinality and late-result eligibility. No guessing. Prior mutable synthetic Permit behavior allowing mismatch then reuse is historical evidence superseded by this direction.

### D4 — interim unresolved Intent preservation

**OWNER-APPROVED, INTERIM ONLY:** Refuse direct deletion/mutation of unresolved Intent and Task/Step/Grant-use deletion if it erases unresolved Intent or essential correlation evidence. Missing evidence is not proof of absent effect and does not authorize redispatch. Unreliable/incomplete restore/import remains quarantined from effect execution until separately verified. Keep fail-closed Forget behavior truthful.

**Not approved:** Indefinite production retention, any retention duration, resolved Intent/payload period, tombstone schema/content/lifetime, authenticated Forget override, event-pruning coupling, privacy exception, P7 Gate D, backup/restore enforcement, canonical migrations 0009/0010, or rollback-proof claims. No blanket permission to block deletion forever. Complete Forget/terminalization stays blocked pending contract.

### D5 — committed Intent Event

**OWNER-APPROVED direction:** Reuse existing `CAPABILITY_REQUESTED` for atomic Provider Dispatch Intent commit. It records committed Intent only—not Provider I/O start, effect success, accepted result, billing or Receipt. Correlation categories: TaskId, StepId, RequestId, IDK digest (never raw IDK), OwnerId, ProviderId, pinned Registry generation, descriptor digest, and applicable EventId correlation. No raw arguments, credentials, provider response, secret, private text or unnecessary personal content.

**Existing accepted constraints:** Event append is atomic with Intent/Grant/Budget/IDK/RequestId. Event content may expire independently; expiry is not evidence of absent Intent/effect.

**Proposed:** Exact closed payload type/field names, grammar, limits, IDK digest domain separation, Event envelope/correlation mechanics, retention coordination, compatibility and validation. No exact payload validator is approved here.

## Architecture and implementation gate

Neither candidate A (single authority-owning crate/module) nor B (self-verifying public Storage operation) is selected. `pub(crate)` does not cross crate boundaries; public unchecked proof/boolean is not authorization. Conduct bounded external-crate A/B spikes and independently verify arbitrary dependent crates cannot forge proof/Permit, mutate Intent or invoke Provider. Candidate B must be safe for every public caller. An architecture-major refactor requires a separate gate.

Permitted next engineering: docs incorporation, isolated Synthetic tests, compile-fail probes, bounded A/B spikes, and necessary test-only corrections. Candidate migrations remain outside ordinary catalog. Production Provider invoke, Memory/Credentials, Receipt/retry, real effect, and production Host dispatch remain absent and unreachable.

## Further decisions required before normative runtime activation

1. Authority-owning crate/API and typed P6 Policy proof for untrusted cross-crate callers.
2. Durable budget schema/accounting, no-Grant OBSERVE representation, recovery and Restore/Forget interaction.
3. Provider instance/readiness evidence, freshness and postcommit failure/cancellation matrix.
4. Resolved Intent, Receipt, late outcome and reconciliation authority.
5. Finite retention or reviewed retention rationale, Forget/tombstone/restore/privacy policy.
6. Exact bounded versioned `CAPABILITY_REQUESTED` payload and retention compatibility.
7. P7 canonical schema/Owner issuer authentication, independent security review and separate production authorization.

## Explicit non-authorizations

No main push, PR merge/close/retarget, force push, branch-protection bypass, candidate migration registration, P7 Gate D, OS Owner issuer authentication, CredentialStore, real account/Provider call, native/root operation, network effect, canonical Receipt, automatic retry, unresolved Intent replay, live migration, production retention guarantee, tamper/rollback resistance or physical power-loss guarantee is authorized.
