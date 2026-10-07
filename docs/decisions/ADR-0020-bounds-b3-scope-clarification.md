# ADR-0020: Bounds `B3` Scope — Operational Bounds versus Structural Constraints

- Status: **Accepted** — semantic B3 operational/structural clarification
- Architecture version: `serea-arch/1.0.0` at acceptance (superseded as current by ADR-0026 / `serea-arch/2.0.0`)
- Decision date: 2026-10-03 — owner direction
- Recorded by: P2 design preparation, from `007f038af19ae7855ad00b7e58389ce04d0fe727`
- Feeds: [P2 contract gap analysis](../plans/P2-contract-gap-analysis.md) §5.5, §12

> Accepted on owner ratification after three corrected documentation gate reviews
> GREEN. Contract annotations belong to P2A; this is semantic architecture-minor
> in isolation, not editorial patch. No operational/resource-bound values or code
> limits are added; this acceptance is not full workspace/MSRV verification.

## Context

[Bounds Protocol §2](../protocols/10-bounds-protocol.md#2-the-bound-set) declares
its table "The authoritative list. Every bound the host enforces appears here; a
bound enforced anywhere else is a bug", and invariant `B3` repeats it.

Read literally, `B3` makes the following a bug:

| What | Where | Why it reads as a bound |
| --- | --- | --- |
| "Every array has `maxItems`; every string has `maxLength`" | [Capability Protocol §3.1](../protocols/01-capability-protocol.md#31-field-semantics) | A limit on quantity |
| `maxLength`, `maxItems`, `maxProperties`, `minLength`, `minimum`, `maximum` | every checked-in schema | Limits |
| `MAX_INSTANCE_DEPTH = 64` | `crates/serea-protocol/src/schema.rs` | A nesting limit |

None of these appears in the §2 table. So either the frozen protocols contradict
each other, or `B3` does not mean what a literal reading says.

The precedent shows which reading is right. P1 added `MAX_VALUE_LENGTH = 4096` to
`validate_label`, then removed it by **owner decision** on the grounds that it
was "an unratified competing bound", with the reasoning that
"`B3` makes a bound enforced anywhere else a bug". That reasoning is correct for
the constant that was removed — it was a *length ceiling on free text*, which is
a quantity limit, and it had no default, no scope, and no exhaustion behaviour in
the §2 sense.

But the same reasoning cannot be applied to `maxLength: 71` on the `digest`
pattern. That value has an explicit P0 basis: Capability Protocol §3.1 *requires*
every schema string to carry a `maxLength`, and `serea-protocol` documents the
reason — "a schema reference is host-authored and never caller-supplied", so a
length bound there has a stated grounding for *having* a bound and is not the
unratified competing bound that `MAX_VALUE_LENGTH` was.

The distinction exists in the documents; it is simply never written down. That
gap is what this ADR closes, because without it every P2 review pass will re-litigate
the same question and one of them will eventually "fix" a `maxLength` in the wrong
direction.

## Decision

Formalise two distinct kinds of limit.

| | Operational / resource bound | Structural / schema validation constraint |
| --- | --- | --- |
| Question answered | How much *work* may happen? | Is this *value* well formed? |
| Has a default | Yes, in the §2 table | No |
| Has a scope | Yes: per-task, per-step, per-call, per-device, global | None; it is a predicate |
| Has an exhaustion behaviour | Yes: task `FAILED`, or `BLOCKED` where genuinely resumable, plus a `BOUND_EXCEEDED` event carrying `bound_name` | None: the value is refused |
| Runs relative to scheduling | After a decision to do work | Before any work is scheduled |
| Governs | Call counts, token budgets, active time, retries, concurrency, retention | Identifier grammar, string and array shape, object closure, integer ranges, nesting depth |
| Under `B3` | **Must** appear in §2 | **Must not** appear in §2, and its absence there is **not** a bug |

Two rules make the distinction operational rather than rhetorical:

1. **A structural constraint may not enforce an operational bound.** No
   truncation, no clamping to fit, no silent coercion. A value that exceeds a
   structural limit is refused, never shortened. This is the same relationship
   Bounds Protocol §8 draws between bounds and policy: bounds stop work,
   validation refuses values, and neither substitutes for the other.
2. **Anything that counts work is a bound** and must be listed in §2.

### Named classifications

`MAX_INSTANCE_DEPTH = 64` is a **structural** predicate. It refuses a *value*, it
has no default and no exhaustion behaviour, and a `BOUND_EXCEEDED` event has no
meaning for a schema rejection. Its purpose is process safety: a stack overflow
aborts rather than unwinding, which is not a graceful refusal. Naming it here
retires the literal-`B3` reading explicitly, instead of leaving it for a future
reviewer to rediscover.

The `maxLength`/`maxItems`/`maxProperties` keywords in the checked-in schemas are
**structural**. Capability Protocol §3.1's requirement that every array have
`maxItems` and every string have `maxLength` is a **schema-compilation
obligation on the capability schema author**, grounded in closed-world schema
discipline — an unbounded input is an unbounded resource — and it is a validation
predicate on a value, not a counter on work.

`ActionError.details`' `maxProperties: 64` is **structural** and is the existing
worked example of the rule inside this repository: it bounds one object's shape
and has no exhaustion behaviour, so it belongs in the schema and not in §2.

## Proposed amendment

P2A applies the clarification to Bounds and Capability protocols with changelog
entries. This is architecture-minor in isolation, not editorial/patch; it rides
in the ratified architecture-major P2A package. No storage implementation is required.

### 1. A new subsection in Bounds Protocol, after §2.3

> ### 2.4 What a bound is not
>
> This table governs **operational bounds**: host-set limits on how much work may
> happen. A bound has a default, a scope, and an exhaustion behaviour, and it is
> read from durable state at the moment of the check
> ([§2.1](#21-where-these-live-in-durable-state)).
>
> A **structural constraint** is a different thing and is not a bound. It is a
> predicate on a value's shape: identifier grammar, string and array length and
> count, object closure, integer ranges, nesting depth. A structural constraint
> has no default, no scope, and no exhaustion behaviour, and it refuses a value
> before any work is scheduled.
>
> Consequently:
>
> 1. `B3` scopes to operational bounds. A structural constraint need not appear
>    in this table, and its absence here is not a bug.
> 2. A structural constraint may not be used to enforce an operational bound. A
>    value that exceeds one is refused, never truncated or clamped. Refusing is
>    the only behaviour; a bound that silently shrinks its input has stopped
>    bounding.
> 3. Anything that counts work is an operational bound and belongs in this
>    table.
>
> This restates `B3`; it does not weaken it. A quantity limit that refuses or
> truncates untrusted input in order to bound work is an operational bound and
> must be ratified here.

### 2. An annotation in Capability Protocol §3.1

The bullet list gains a trailing parenthetical: *"(These are structural
constraints on a value, not operational bounds, and are therefore out of the
scope of [Bounds Protocol §2](../protocols/10-bounds-protocol.md#2-the-bound-set) invariant
`B3`. A schema that wants to bound *work* rather than shape must declare that
bound in Bounds Protocol §2 instead.)"*

### 3. A changelog section in Bounds Protocol

Protocol Index §7 item 3 requires an entry in the affected protocol's changelog.

## Affected contracts, identified

Protocol Index §7 item 3 requires every affected contract to be named.

| Contract | Change |
| --- | --- |
| Bounds Protocol §2, `B3` | Scoped, not weakened. New §2.4 |
| Capability Protocol §3.1 | One parenthetical; the bullet list itself is unchanged |
| `crates/serea-protocol/src/types.rs`, `validate_label` doc comment | **Unchanged.** It already cites `B3` for *not* adding a length ceiling, which this ADR confirms is correct |
| `crates/serea-protocol/src/schema.rs`, `MAX_INSTANCE_DEPTH` | Depth constant and behavior unchanged by this ADR; named as structural so the literal-`B3` reading is retired. In the complete P2A inventory, schema.rs has version documentation only; embedding/validation unchanged |
| The five checked-in schemas | **Unchanged by this ADR.** Complete P2A changes four schemas for other decisions; envelope remains unchanged |
| Every §2 bound | **Unchanged.** No bound is added, removed, or re-defaulted by this ADR |

No code change accompanies this ADR. The test obligation is a negative one: a
test asserts that the reclassification changed no behaviour, so the ADR cannot be
smuggled in as a licence to add a limit.

## Compatibility

None. No wire surface, enum, field, or bound value changes. This is a
clarification of the scope of an existing invariant, which is why it is
architecture-minor with no migration note.

## Consequences

- Every future review of a schema keyword or a Rust validator has one answer
  instead of two defensible ones.
- The `B3` argument that retracted `MAX_VALUE_LENGTH` stands unchanged and is now
  stated with its limit: it was a bound because it limited quantity, and the
  removed constant is not reinstated by this ADR.
- **This ADR does not close the open resource-bound gap.** Payload bytes, blob
  bytes, attachment bytes, object counts and decompression limits remain
  unratified. §12 of the P2 design records what a future decision must settle and
  why P2 cannot supply the numbers.

## Rejected alternatives

| Alternative | Why rejected |
| --- | --- |
| Add every schema `maxLength`/`maxItems` to the §2 table | A §2 row implies a default, a scope and an exhaustion behaviour. A schema keyword has none, so the row would be a fiction and `B17` would demand a `BOUND_EXCEEDED` event for a schema rejection |
| Weaken `B3` to "the complete set of *runtime* bounds" without defining the word | Leaves the ambiguity in place; a reviewer can still read `maxItems` as a bound |
| Delete the schema keywords | Removes real closed-world protection that Capability Protocol §3.1 and Data Classification §4 both rely on |
| Accept a free-text length ceiling again, now that §2 has a table | The evidence problem is unchanged: no measurement exists, and a guessed number is a bound nobody chose |
| Put resource bounds in this ADR | No evidence exists for any value. Inventing one is the `MAX_VALUE_LENGTH` mistake. §12 of the design records the separate decision that must settle them |
