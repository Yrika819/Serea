# P8B Contract Closure Package — 2026-10-16

Status: **Engineering recommendation; no new Owner decision recorded.** This
package separates existing accepted rules from the four missing contracts
needed before a runtime dispatch API can be implemented.

## Evidence and current boundary

- `ADR-0036` accepts one `max_tool_calls_per_task` unit per durably committed
  provider dispatch intent. Pre-dispatch refusal and duplicate suppression do
  not consume a unit; a committed intent is not refunded. Future retries and
  read-only reconciliation accounting remain proposed.
- `ADR-0039` requires applicable Grant use to join the Dispatch Intent,
  RequestId, IDK reservation, Budget and required Event in one SQLite write
  transaction. The existing P6 `Tx::consume_grant_use` is transaction-joinable,
  enforces exact R2 membership and unique per-Step consumption, and rechecks
  current grant status and expiry. It does not authorize provider dispatch.
- Task `max_tool_calls` is persisted in migration 0001 and the typed
  `AttemptBudget`; there is no Task tool-call usage counter in migrations
  0001–0008. Migration 0003's `model_call_count` is specifically P4 model-call
  accounting and cannot be reused for tools. `max_attempts_per_step` bounds
  lease/attempt lifecycle, not committed provider dispatches.
- `CapabilityAvailabilitySnapshotV1` reads a durable registry generation and
  overlay, then snapshots provider advertisement and asynchronous
  `CapabilityProvider::health()`. That health method defaults to `Ready`.
  Readiness is in-memory, is not versioned in SQLite, and is not part of the
  P8 transaction. The snapshot is useful for resolution, not durable proof of
  current readiness at invoke time.
- `TaskEngine::evaluate_prepared_action` explicitly returns point-in-time
  evidence. There is no TaskEngine `dispatch` or provider `invoke` path. The
  existing recovery protocol says expired leases do not authorize reinvoke;
  unresolved committed Intent is reconciliation-only and no automatic
  same-Step retry is allowed.
- Task Protocol §8 states 30-day default retention, Task data deletion, and
  Task/Step cascades. ADR-0044 says unresolved Intent may not be deleted and
  then treated as dispatch authority, but sets no Intent retention horizon or
  Forget disposition. Candidate 0010's RESTRICT FKs stop Task, Step and
  Grant-use deletion while the Intent exists. Its new development-only
  `dispatch_intent_no_delete` trigger also blocks direct deletion; this does
  not define a production retention or Forget policy.
- Event Protocol §3.3 already assigns `CAPABILITY_REQUESTED` the meaning
  "Future P8 dispatch intent was durably committed." ADR-0044 separately
  states that Intent commit is authority for the dispatch attempt, not proof
  that provider IO or an external effect happened. The current Storage probe
  uses the accepted kind in its atomic transaction, but its minimal test
  payload does not establish a complete canonical P8 correlation payload.

## A. Budget accounting

### Accepted

One unit is consumed at the same commit as each provider dispatch Intent. A
refused or duplicate-suppressed operation with no committed Intent consumes
zero. A committed Intent remains charged after a later timeout, crash, known
pre-invoke stop, provider failure or AMBIGUOUS result. The Intent/IDK/RequestId
uniqueness boundary and initial no-same-Step-effect-retry rule mean a second
effect cannot be authorized by reusing that unit.

### Proposed clarification

Interpret the unqualified phrase “provider dispatch intent” to include both
OBSERVE and EFFECT provider invocations. An OBSERVE call that does not cross a
provider boundary consumes zero only if it is resolved locally without a
committed provider Intent. A future read-only reconciliation call has no
accepted budget treatment and stays blocked. Duplicate suppression has no
provider dispatch and consumes zero. `max_attempts_per_step` remains a separate
guard; it is not a substitute counter.

### Recommended minimal durable shape

Use a Task-scoped durable `tool_call_count` (or a precisely equivalent
countable Intent set) and conditionally increment it inside the dispatch
transaction only while it is below `tasks.max_tool_calls`. In the same
transaction, insert the applicable P6 use, unique IDK reservation, minted
RequestId, immutable Intent, and the approved Intent event. Do not refund on
any post-commit outcome. The counter has no independent write API. The current
candidate probe counts immutable Intent rows under its serialized SQLite write
transaction; its no-delete trigger is necessary for that test-only strategy.
For canonical schema, a counter on Task is easier to retain after Intent
terminalization, but its lifecycle must be specified together with Task
deletion.

Event Protocol §3.3 already assigns `CAPABILITY_REQUESTED` to a durably
committed P8 Dispatch Intent. That event proves Intent commit only; it does not
prove provider invocation, effect success, billing, or Receipt. Reuse this
accepted EventKind by default. Its exact bounded P8 payload is still a
proposed Owner decision, and transaction tests must not claim payload
completeness before that decision.

### Acceptance tests

1. `max_tool_calls=0`: any otherwise-valid dispatch is refused; after reopen,
   Grant uses, IDK reservations, RequestIds, Intents and events are all absent.
2. `max_tool_calls=1`: one Intent commits and count becomes one; a distinct
   eligible Step is refused at the limit with no partial rows.
3. Competing connections at limit−1 yield exactly one commit.
4. Pre-commit failure rolls back count and all authority writes. A committed
   Intent followed by a simulated crash retains the charge after reopen.
5. No-intent duplicate suppression and policy/approval/lease refusal consume
   zero. Reconciliation accounting remains NOT RUN / blocked until separately
   accepted.
6. `max_tool_calls=2`: two distinct approved Steps commit exactly two Intent,
   Grant-use and Event rows; the reopened Task limit and durable count agree.

The test-only PR #25 follow-up implements zero-limit rollback and a limit-one
race against the actual Task row and Storage transaction. The
2026-10-17 continuation adds two distinct concurrent eligible Steps that
commit exactly the two allowed units and asserts the reopened rows. These
remain test-only candidate mechanics; no production counter or API is added.

The reused P8C simulator also now has test-only separate Intent and Effect
SQLite ledgers. Added cases reopen both after ACK loss and verify exact
RequestId/IDK correlation without replay, and detect a same-RequestId / wrong-
IDK effect record as non-matching evidence. These tests do not wire the
simulator to TaskEngine or make a canonical Receipt. A third case simulates
worker loss after synthetic Intent commit and before fake invoke; independent
reopen preserves the Intent and confirms the effect ledger remains empty
without replay. It does not terminate a real OS process.

The Storage composition probe now takes a private test-only readiness enum.
`Unknown` and `Unavailable` refuse before the write transaction and leave P6
Grant-use/Intent/Event state empty. Its `SyntheticReadyForCompositionOnly`
variant exists only to test transaction composition; it is not produced by the
Capability resolver and is not a production readiness source.

## B. Provider availability

### Accepted

Trusted manifest and activated Registry generation pin descriptor identity.
The current live overlay can disable or remove a capability. Provider
advertisement and health are considered by the existing capability resolver.
An absent/non-Ready provider cannot resolve as available in a fresh snapshot.

### Blocked

No durable, revisioned Provider readiness authority exists. The async health
snapshot and overlay read are not atomic with the P8 transaction and can become
stale before provider IO. A provider's default `Ready` value is an interface
default, not evidence that a backing service/account is usable. SQLite cannot
prove that an external service stays available after commit.

### Recommendation

Do not persist an old health boolean as authority. A future host-owned,
one-shot Dispatch Permit should bind the exact in-process provider instance,
provider ID, descriptor digest, registry generation, overlay revision and
Intent RequestId. The host should require a fresh `Ready` health observation
immediately before the commit and again immediately before `invoke`; a missing,
unavailable or mismatched provider fails closed. The post-commit result of a
failed readiness check must consume the committed Budget and remain unresolved
until an accepted no-invoke disposition exists. It must never silently mint a
second Intent. This permit and post-commit failure disposition remain
PROPOSED; no Fake readiness may be treated as production authority.

### Acceptance tests

1. Missing provider, `Unavailable`, `Degraded`, advertisement mismatch, stale
   registry generation or disabled/removed overlay produces no Intent or
   Grant use when detected before commit.
2. A readiness observation from a different provider instance, descriptor,
   registry generation, overlay revision or RequestId cannot authorize invoke.
3. Readiness loss after Intent commit yields no second effect and no automatic
   retry; reopen preserves an unresolved record for reconciliation.
4. A Fake provider's readiness values are type-separated from Host runtime
   readiness and never enter production authorization APIs.

### Additional test-only evidence — 2026-10-17

The Storage `cfg(test)` Permit model now pins registry generation,
descriptor digest and overlay revision in addition to RequestId/IDK,
Task/Step/Owner, Lease generation and provider instance. A deliberate mutation
that removed those three comparisons caused the focused test to fail on a
registry-generation mismatch after the Fake ledger recorded an effect (RED);
restoring the comparisons passed the Storage P8 selector (GREEN). A separate
test commits the synthetic Intent, then makes the Fake Provider unavailable
before the effect write: the Intent remains, the effect ledger stays at zero,
and the consumed Permit rejects a second call. This does not establish live
Host readiness or production invocation authority.

The prototype still does not model durable availability. A persisted Ready
Boolean would not fix the time-of-check/time-of-use gap; a concrete in-process
provider instance with fresh checks and these revision pins is the recommended
synthetic boundary, subject to Owner ratification.

## C. Commit-to-invoke and recovery

### Accepted

The Intent commit is the P8 authority point; provider IO is after commit, with
no SQLite write transaction spanning IO. A consumed Grant is historical
evidence and not repeat-effect authority. Recovery inspects the exact Intent;
unresolved Intent is reconcile-only, even under `NATIVE` idempotency. CANCELLED
is terminal and a late, unverified response is not a canonical Receipt.

### Recommended closure clause

Treat the committed Intent as a linearization point for exactly one
post-commit provider invocation by the in-process worker holding the bound
Lease generation and one-shot Permit. A cancellation, Policy change, Grant
revocation, deadline expiry, Lease expiry or overlay change committed before
the Intent transaction makes it refuse. Changes committed after Intent commit
stop future Steps/Intents, but cannot mint or revoke a different Intent. The
already-authorized call may still race with cancellation; if the Task is
cancelled, do not revive it or commit a canonical late Receipt under Option C.
If the process stops after commit and before a durable fact proves invocation
was absent, recovery treats the outcome as AMBIGUOUS and never invokes again.
This avoids an unsafe “revalidate and replay” gap but accepts the availability
cost that a committed Intent may consume budget without producing an effect.

This clause narrows to the accepted “commit is authority point” and
“reconciliation only” directions, but its exact in-process Permit and
post-commit cancellation ordering still require Owner acceptance before
runtime invoke is connected.

### Acceptance tests

1. Crash before Intent commit: no Intent/Grant use/Budget/IDK/RequestId/Event;
   no provider call.
2. Crash immediately after commit and before invoke: reopened Intent is
   unresolved; no provider call occurs during recovery and no retry path is
   returned.
3. Cancellation, deadline, Policy, Grant or Lease change serialized before
   commit refuses with no partial writes. Same changes after commit cannot
   create a second invoke; Task remains CANCELLED when cancellation won later.
4. Invoke ledger has one effect but ACK is lost: AMBIGUOUS remains
   reconciliation-only after repeated restarts.
5. Invoke result arrives after cancellation: no Task revival, no canonical
   Receipt, exact RequestId mismatch rejected.

### Unresolved Permit mismatch consumption detail

The current private test simulator checks the presented bindings before
marking its mutable Permit consumed. A wrong IDK attempt produces no effect,
and the same Permit can then be used once with the exact RequestId/IDK and
other bindings. The test `mismatched_attempt_does_not_consume_permit_before_a_later_exact_binding`
records that behavior. The accepted contracts do not specify whether a
mismatched call burns the Permit. For a future Rust API, the recommendation is
to pass a non-Clone Permit by value to `invoke_once`, so every attempted use,
including a mismatched one, consumes that in-process token; Owner should
approve or reject this detail before runtime integration. It remains a
test-only implementation question and creates no current invoke route.

## D. Intent retention and Forget

### Accepted

Task Protocol has ordinary Task retention and Forget deletion. P7 Forget
principles require authenticated explicit deletion and fail-closed behavior
on uncertainty, but Gate D, production Owner authentication, key custody,
rollback-resistant authority, and production Memory are not enabled. ADR-0044
prohibits deleting an unresolved Intent and using its absence as permission to
dispatch again. No accepted P8 Intent horizon or completed-outcome retention
rule exists.

### Recommended interim behavior

Keep the candidate's refusal to delete or mutate an Intent, its referenced
Task/Step and Grant-use while unresolved; also refuse direct Intent deletion.
This is a temporary development guard only. Do not claim it implements Task
Forget: under a lost provider outcome it can block Task/Forget indefinitely.
Do not add a retention clock or tombstone cleanup without an Owner decision.

### Owner choices required before canonical schema

1. **Unresolved Intent:** retain until provider-backed reconciliation,
   indefinite quarantine, or an authenticated Forget override with permanent
   no-replay tombstone. Recommend retain/quarantine until explicit
   reconciliation; this minimizes evidence loss but delays full Forget.
2. **Resolved Intent/Receipt:** choose whether Task expiry deletes payload but
   preserves a minimized dispatch tombstone, and choose its horizon. No
   numeric duration is recommended without policy input.
3. **Forget override:** choose which identifiers/digests/event references may
   survive deletion, how replay/duplicate suppression remains safe, and how
   primary SQLite deletion is reconciled after restore. Do not call a local
   tombstone rollback-resistant.
4. **Events:** candidate Intent stores an EventId without FK so normal event
   pruning does not cascade to Intent. Decide whether the event body is
   allowed to expire independently and what durable audit evidence remains.

### Acceptance tests

1. Task, Step, Grant-use and direct Intent delete fail while unresolved; all
   rows remain after reopen and foreign-key/integrity checks.
2. Forget racing with recovery is serialized; either Forget is refused while
   unresolved or a separately approved tombstone path preserves no-replay
   evidence. No test may equate missing rows with “effect absent.”
3. Event pruning does not delete or authorize deletion of the Intent; the
   correlation event may be absent after its independently approved horizon.
4. Once an Owner-approved terminal disposition exists, test exact allowed
   payload deletion, retained evidence fields, Owner isolation and restore
   quarantine. This test is blocked until decisions 1–3 are accepted.

## E. P8 Intent event semantics

### Accepted facts

- The Dispatch Transaction must append its required Event in the same SQLite
  write transaction as the applicable Grant-use, budget reservation, RequestId,
  IDK and Intent. An Event write or commit failure leaves none of those writes
  durable.
- Event Protocol §3.3 defines `CAPABILITY_REQUESTED` as the durable-Intent
  commit event for future P8. ADR-0044 limits what that event proves: it does
  not prove that provider IO started, an effect occurred, or a Receipt exists.
- The current Storage probe writes this accepted EventKind in the same test
  transaction. Its payload carries RequestId and StepId; TaskId is in the
  correlation/trace fields. It does not yet prove a full agreed payload with
  IDK digest, OwnerId, ProviderId, and pinned registry identity.
- Event Protocol payloads are bounded, canonicalized, and cannot contain
  SECRET/CREDENTIAL content. Event content has an independently enforced
  retention deadline in Storage.

### Proposed selected clause

Use the already accepted `CAPABILITY_REQUESTED` kind to record the transaction
that committed the dispatch Intent and its budget/Grant-use participants. The
event must not imply Provider invocation, effect success, or Receipt authority.
Propose a minimal correlation payload containing TaskId, StepId, RequestId, an
IDK digest, OwnerId, ProviderId, pinned registry generation/descriptor digest,
and the Intent's event identifier. It must omit raw arguments, credentials,
provider responses, and personal content. Exact payload field names and
retention interaction remain proposed until their protocol wording is
reviewed.

### Alternatives

1. **Recommended:** retain `CAPABILITY_REQUESTED` and specify its bounded
   correlation payload. This preserves Event Protocol §3.3 and separates
   committed authority from subsequent provider/effect events.
2. Add `DISPATCH_INTENT_COMMITTED` only through an explicit Owner-approved
   Protocol change. A new kind may improve naming but changes the frozen
   EventKind enum and consumers.
3. Store no semantic Event and rely on the Intent row alone. This conflicts
   with the accepted requirement for an Event participant in the atomic
   transaction and weakens event-driven audit visibility.

### Attack scenarios

- A committed database row with the wrong EventKind is incorrectly presented
  as proof of authorization or invocation.
- An Event payload copies arguments, credentials, response data, or other
  sensitive values into an independently retained stream.
- Event insert, validation, or commit fails after Grant-use/Intent insertion
  and leaves partial authority rows.
- Event pruning removes the correlation record and an operator mistakes that
  absence for proof the Intent or external effect never existed.

### Precise API and schema impact

- Define and validate the bounded `CAPABILITY_REQUESTED` P8 payload only after
  exact field names, requiredness, and privacy limits are reviewed. Keep the
  EventKind enum and protocol version bounds unchanged in this slice.
- A dedicated event requires a separate Owner-approved protocol change and
  compatibility review; it is not necessary to satisfy the current accepted
  EventKind semantics.
- Have the future TaskEngine dispatch transaction append this event through
  the existing transaction participant. Do not add an independently callable
  production Intent writer or event-only authority path.
- Keep the candidate Intent/Event correlation identifier non-FK if approved
  event pruning must remain independent. Event retention must not erase or
  mutate the Intent evidence.

### Executable tests

1. Event insertion failure and pre-commit failure roll back Grant-use,
   RequestId/IDK, budget and Intent on a reopened real Store.
2. Successful commit writes exactly one Intent and one event with the exact
   approved correlation fields and no secret/raw argument fields.
3. The event itself does not assert provider invoke, effect success, or
   Receipt creation.
4. Expiring the event content leaves the unresolved Intent and its RequestId/
   IDK evidence unchanged. Current candidate coverage is synthetic and does
   not define a canonical retention policy.

### Residual risks and Owner decision

The accepted event can be pruned independently, and SQLite cannot make it a
rollback-proof external audit anchor. The EventKind semantics already
distinguish committed Intent from provider invocation; its exact P8 payload is
not specified. Owner decision: approve the minimal payload fields above, choose
an alternative, or defer payload closure while keeping runtime integration
blocked. A new EventKind is not proposed as the default.

## Minimal API sketch and implementation gate

The only acceptable production entry is a TaskEngine-owned operation returning
an opaque one-shot post-commit Permit, never a boolean authority flag or raw
Storage Intent writer:

```text
TaskEngine::commit_dispatch(prepared, current_worker, current_time)
    -> Result<CommittedDispatchPermit, DispatchRefusal>
CommittedDispatchPermit::invoke_once(provider_registry, provider_context)
    -> Result<ActionResult, DispatchOutcome>
```

The transaction must consume applicable P6 Grant use, reserve IDK and
RequestId, charge the accepted Task budget, re-read Task/Step/Lease/Policy/
Approval/Registry/Overlay facts, and append the approved event. The permit
must be unforgeable outside TaskEngine, bound to the committed Intent, and
non-serializable across process restart. Storage may expose no public arbitrary
Intent insert. This is an API shape recommendation only: existing crate
boundaries do not yet provide a typed, unforgeable P6 Policy proof that
Storage can validate without creating a dependency cycle. Do not implement
the production signature until that evidence boundary is resolved.

## Current follow-up evidence

The PR #25 branch adds only a test-only budget check over actual Task
`max_tool_calls` plus committed candidate Intents, and a candidate-only
no-delete trigger. The follow-up independent-Store deletion/event-expiry test
is development-only and does not promote a retention contract. Cargo results
and local build storage are recorded in the current ExecPlan so earlier
environment observations are not mistaken for current validation. Provider
dispatch remains absent from TaskEngine and normal Host builds.

The 2026-10-17 continuation adds three revision pins to the private synthetic
Permit and tests pre-invoke provider unavailability after commit. Guard-removal
mutation is RED and restored guard is GREEN; exact commands and test totals are
in the ExecPlan. A further test demonstrates that a mismatched attempt leaves
the current mutable test Permit reusable for one later exact invocation. The
future move-consuming API is recommended but remains an Owner decision. No
Protocol, normal migration, production Storage API, Host/TaskEngine invoke,
Receipt or retry behavior changed.
