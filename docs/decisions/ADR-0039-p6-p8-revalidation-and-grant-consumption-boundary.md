# ADR-0039: P6/P8 revalidation and grant-consumption boundary

Status: **Proposed** · Date: 2026-10-10 · Architecture: `serea-arch/2.6.0`

Surfaces affected: none directly. This ADR records the boundary between P6 and P8 for a
question the P6 audit recorded as D11.

## Context

[ADR-0036](../decisions/ADR-0036-p5-p6-p8-authorization-and-dispatch.md) already fixes the
P8 order: proposal validation, registry and schema, P6 policy, P6 approval, duplicate
suppression, repeated-action bound, tool-call budget and durable dispatch intent, provider
invoke, result, receipt, evidence, reconciliation. It also states that P8 rechecks task
cancellation, live capability enablement, provider availability, deadline, and approval
validity, and does not re-resolve pinned descriptor facts.

What ADR-0036 does not say is where grant consumption sits relative to the dispatch-intent
commit. [Approval Protocol §4.2](../protocols/05-approval-protocol.md) says consumption is a
single atomic durable operation tied to `step_id`, and that a crash between "grant validated"
and "grant consumed" must not allow a double use. It does not say whether that operation
shares a transaction with the intent.

## Options

**A — P6 consumes before P8 dispatch.** A separate transaction, before the intent commit.
Simple and safe against double use, but it converts "approved" into "consumed" before the
intent exists, so a failure between the two strands a use on a step that never dispatched.

**B — P8 consumes in the same transaction as the dispatch intent.** The intent row, the
budget consumption, the IDK reservation and the grant-use row commit together or not at all.
No stranded use, because a use can only exist if the intent that used it also exists.

**C — reservation in P6, final consumption in P8.** Two-phase. Strictly more machinery than
B, with no additional guarantee, because the intent commit is the only point at which the use
is real.

## Decision

**Proposed: Option B, subject to owner ratification, and reclassified as a P8 contract
decision rather than a P6 blocker.**

P6 must expose a consume operation that a caller can join to an existing transaction. P6 does
not decide when that happens. The invariant P6 must provide, and must test on its own, is:

> For a given `(grant_id, step_id)`, at most one use row exists, and `uses_remaining` can
> never be decremented twice for the same Step.

That is enforced by a primary key on `(grant_id, step_id)` plus a uniqueness constraint on
`step_id`, checked inside the same transaction as the decrement. P6 tests it with independent
Store connections. Whether the surrounding transaction also contains a dispatch intent is
P8's concern.

## Consequences

- P6 is not blocked by a P8 decision. D11 is `DEFER_TO_P8`.
- P6's consume operation is transaction-joinable, which is the only shape P8 needs.
- The crash-after-consume-before-dispatch case disappears, because there is no such window
  under Option B.

## Verification obligations

P6 proves the invariant on its own, with independent connections and with fault injection at
the transaction boundary. P8 proves the composition when it exists.

## Status

**Proposed. Not accepted. P8 is not started.**
