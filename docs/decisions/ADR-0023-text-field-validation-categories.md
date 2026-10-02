# ADR-0023: Text Field Validation Categories

- Status: **Proposed** — pending implementation and owner ratification
- Architecture version: `serea-arch/0.2.0` at the time of writing
- Decision date: not yet ratified
- Recorded by: P2 design preparation, from `c3737039e3e38dbba554dc0b9075025f87948358`
- Feeds: [P2 contract gap analysis](../plans/P2-contract-gap-analysis.md) §5.6

> This ADR changes no frozen protocol text and no code. The amendments below are
> **drafted, not applied**.

## Context

`validate_label` in `crates/serea-protocol/src/types.rs` rejects empty,
whitespace-only, and any string containing a `char::is_control()` character. One
validator serves nine fields with materially different semantics:

| Field | What it actually is | Where it is rendered |
| --- | --- | --- |
| `ActorId` | An event actor identity | A machine column |
| `LeaseOwner` | The worker holding a lease | Equality-compared in SQL, audited |
| `ProviderReference` | An external system's own handle, never parsed | Equality-compared, echoed in reconciliation |
| `ErrorMessage` | A provider diagnostic | A terminal or a log line |
| `TaskTitle` | A task heading | One timeline row |
| `DescriptorTitle` | A capability heading | One picker row |
| `DescriptorDescription` | Capability documentation | Prose |
| `PlainSummary` | The sentence a human consents to | **An approval prompt** |
| `EffectSummary` | What changed | One timeline row |

Two demands in that single validator are wrong for at least some of those fields.

**Newlines in `PlainSummary` are a consent-spoofing vector.** It is rendered into
an approval prompt, so a newline lets a provider-shaped summary present a second
line that the user reads as part of the host's own instruction. Refusing `\n` and
`\t` there is correct and must not be relaxed.

**Newlines in `ErrorMessage` cost something and buy nothing.** A provider
diagnostic frequently contains a stack-shaped message with newlines. Refining them
forces a provider author to mangle a real diagnostic, and no human is misled,
because nothing renders an error message as an instruction.

Symmetrically, the *token* fields need a rule the label fields do not need: they
are stored in equality-compared columns and printed in audit records, so two
different identifier grammars sharing one column is a genuine confusion vector.

### Second, separate defect: the schema and Rust disagree

The schema pattern for free text is `^[^\u0000-\u001f\u007f]+$` with
`minLength: 1`. It does not exclude whitespace-only input, so `"   "` passes the
schema and fails the Rust validator. P1 closure already records this as a
pre-existing divergence, verified against the `997f747` schemas, and deliberately
left alone because tightening the schema is a contract change.

**P2 cannot leave it alone.** P2 reads a row, converts it through the Rust type,
and writes it back. A row the schema admits and Rust refuses is a round-trip
failure at the storage boundary, and the failure mode is an `Err` on a reopen, not
a visible validation error. This is promoted from a recorded limitation to a P2
blocker.

## Decision

### Three categories, one validator each

| Category | Fields | Rules |
| --- | --- | --- |
| **O** — opaque token / reference | `ActorId`, `LeaseOwner`, `ProviderReference` | Non-empty. No C0 control, no DEL. No leading or trailing whitespace. **Refused if the value parses as any other frozen identifier domain.** Length bounded by the owning schema's `maxLength` |
| **L** — single-line label | `TaskTitle`, `DescriptorTitle`, `EffectSummary`, `PlainSummary` | Non-empty. No C0 control, including `\n` and `\t`. No leading or trailing whitespace. Length bounded by the owning schema's `maxLength` |
| **P** — prose | `ErrorMessage`, `DescriptorDescription` | Non-empty. `\n` and `\t` permitted. Every other C0 control and DEL refused, **including `\r`**. No leading or trailing whitespace. Length bounded by the owning schema's `maxLength` |

Three judgements worth their reasoning:

- **The impersonation rule in category O is the only non-whitespace rule here,
  and it earns its place.** `ProviderReference` "is opaque here; Serea stores what
  it is given and never parses it", so it may legitimately hold anything. But a
  `ProviderReference` that happens to parse as a `stp_` ULID could be rendered in
  an audit row as though it identified a step. Refusing that costs nothing,
  because a real provider handle is not a Serea ULID.
- **`\r` is refused in category P while `\n` is permitted.** CR/LF normalisation
  differs between consumers, so a message containing a bare CR can be made to
  produce a line break in one renderer and not another. Refusing the ambiguous
  character while permitting the unambiguous one is the whole point.
- **No length ceiling in Rust.** P1 removed `MAX_VALUE_LENGTH = 4096` by owner
  decision because it was an unratified competing bound under `B3`, and that
  removal stands. This ADR does not reinstate the constant. Per-field `maxLength`
  belongs in the schema, which is where Capability Protocol §3.1 already grounds
  it for schema strings and where a structural constraint belongs under ADR-0020.
  The free-text fields currently carry no `maxLength` and **this ADR adds none**.

### Schema side — verified against the actual documents

The three checked-in schemas do **not** share one definition to be split, and an
earlier draft of this ADR described them as if they did. The real shape:

| File | How free text is written today | Fields affected |
| --- | --- | --- |
| `action-result.schema.json` | A `$defs/freeText` **definition**, `$ref`'d three times | `actor.id` (line 147, category **O**), `effect_summary` (176, **L**), `message` (193, **P**) |
| `assistant-task.schema.json` | **No** definition. The pattern is written inline at lines 39, 152, 353, 423, 428, 460 | `title` (**L**), `result_summary` (**L**), `lease_owner` (**O**), `effect_summary` (**L**), `message` (**P**) |
| `event.schema.json` | **No** definition. Inline pattern at line 93 | `actor.id` (**O**) |

So the work is: replace the single `freeText` definition in
`action-result.schema.json` with three, **keeping `actor.id` in scope** — omitting
it leaves that field on the old pattern and test A2 fails — and give the inline
occurrences in the other two files a `$ref` to the definition each one needs.

The whitespace-only divergence is fixed with a negative lookahead, which ECMA-262
supports and which this repository already uses in three capability-identifier
patterns:

```text
^(?![ \t\n\r\f\v]*$)[^\r\u0000-\u001f\u007f]+$      category P
^(?![ \t\n\r\f\v]*$)[^\u0000-\u001f\u007f]+$        category L
^(?![ \t\n\r\f\v]*$)(?!.*:(tsk|stp|apr|grt|req|evt|dev|sch|prop|rcp|ses)_)
 [^\u0000-\u001f\u007f]+$                              category O
```

The category-O pattern is the awkward one, and the awkwardness is the point: eleven
identifier prefixes cannot be expressed as one readable negated class. Two
consequences are recorded rather than glossed over:

- The pattern is long and must be **generated** from the prefix list rather than
  hand-written, with a test asserting the two agree. A hand-written copy drifts, and
  a drifted copy means the schema accepts an `ActorId` the Rust validator refuses.
- If generating it proves impractical, the honest fallback is to state the
  impersonation rule as **Rust-only**, and to narrow test A2 to the categories whose
  rejection *is* expressible in both. That fallback must be an explicit decision,
  not a quiet drift from "identical on both sides".

Every field's rejection must be identical on both sides, and a test enumerates
every field against a shared corpus of adversarial strings — empty,
whitespace-only, leading and trailing whitespace, `\n`, `\r`, `\t`, NUL, DEL, a
`stp_`-prefixed ULID in an O field, a 5000-character string, and a non-ASCII
string — asserting the same verdict from the Rust validator and the schema. That
test is the mechanism that prevents this divergence from recurring.

### Prose fields are `PRIVATE` for log and event egress

Data Classification §5 permits `PRIVATE` in an application log only as "Digests,
shapes, and counts only — **never payload bytes**". `ErrorMessage` is already
named as the `AB-13` carrier in P1 closure, and
[Trust Boundaries §2 `TB-4`](../architecture/02-trust-boundaries.md#tb-4-host-to-credential-store)
forbids secret bytes reaching "any log, error message, event payload,
`arguments_preview`, notification body, or model prompt".

So a prose field may be **stored** (subject to ADR-0022) but may not be **logged**
or placed in an event payload without redaction. This is a caller obligation
recorded in the P2 design, not a new type, and no new field is added.

## Proposed amendment

Applied to `docs/protocols/01-capability-protocol.md` and
`02-task-protocol.md` only in the same commit that implements it. Nothing here is
applied by this run.

1. Capability Protocol §3.1 gains a paragraph defining the three text categories
   and assigning every free-text field to one, with the impersonation rule stated
   for category O.
2. Task Protocol §3.1's table gains the category for `lease_owner`.
3. A changelog section in each.

## Code change, same commit

| File | Change |
| --- | --- |
| `crates/serea-protocol/src/types.rs` | `validate_opaque_token`, `validate_single_line_label`, `validate_prose` replace the single `validate_label`; each `declare_value!` invocation names its category |
| `crates/serea-protocol/schemas/assistant-task.schema.json` | Six inline patterns replaced by `$ref`s to the definition their field's category needs: `title`, `result_summary` (L); `lease_owner` (O); `effect_summary` (L); `message` (P) |
| `crates/serea-protocol/schemas/action-result.schema.json` | `freeText` replaced by three definitions; `actor.id` (O), `effect_summary` (L), `message` (P) recategorised |
| `crates/serea-protocol/schemas/event.schema.json` | The inline pattern at line 93 replaced by a `$ref` to the opaque-token definition |
| A test asserting the generated category-O pattern matches the frozen prefix list | So the eleven prefixes cannot drift |
| `crates/serea-protocol/tests/protocol_types.rs` | Per-category accept and reject cases |
| `crates/serea-protocol/tests/schema_contracts.rs` | The shared adversarial corpus, asserted equal on both sides |

The Rust change and the schema change are **one commit**. Neither alone is
correct: the Rust change alone leaves the schema accepting whitespace-only values,
and the schema change alone leaves the Rust type refusing values the schema
permits.

## Compatibility

Making the schema *stricter* is backward-compatible under Protocol Index §4.2 for
a forward-compatible surface only if no producer depends on the looser behaviour.
`TaskStep` and `Actor` are forward-compatible surfaces whose producers are the
host itself, and a whitespace-only `title` or actor id has no legitimate producer.
Making the Rust `ErrorMessage` validator *accept* newlines is a relaxation, in the
safe direction, and §5.1 of the gap analysis records that the same reasoning
already applies to the five `TaskStep` fields in ADR-0018.

The owner chooses the architecture-version treatment; this ADR does not.

## Consequences

- Nine fields stop sharing one validator, so a future author picks a category
  instead of inheriting a default.
- The divergence that P1 recorded and deliberately left is closed, and the
  closing test is the mechanism that stops it reopening.
- No length bound is introduced, so ADR-0020's "no resource bounds tonight"
  position holds.
- `ErrorMessage` and `DescriptorDescription` may carry real multi-line text
  again, and the redaction obligation on them becomes explicit rather than
  accidental.

## Rejected alternatives

| Alternative | Why rejected |
| --- | --- |
| One validator for all nine fields | Two of its demands are wrong for at least one field each: forbidding newlines mangles provider diagnostics, and allowing them enables consent spoofing in `PlainSummary` |
| Allow newlines everywhere for simplicity | `PlainSummary` is an approval prompt. A second line the user reads as the host's instruction is a real attack, not a hypothetical |
| Forbid newlines everywhere | Forbids a provider author from reporting a real diagnostic, and buys nothing |
| Length-ceiling every free-text field to fix the divergence | The divergence is about whitespace, not length. Adding `maxLength` would also reinstate the exact unratified bound P1 retracted |
| Add a Rust `is_whitespace` check only | Leaves the schema admitting values Rust refuses, which is the storage-boundary round-trip failure described above |
| Tighten the schema with `minLength: 2` or similar | A length threshold is not a whitespace rule; `"a "` still passes it |
| Reject a `ProviderReference` that parses as any identifier | Rejected for `ProviderReference` only if measurement later shows real provider handles colliding with a Serea grammar. Recorded as a named future relaxation rather than assumed unnecessary |
| Prohibit any `stp_` prefix in an O field | Narrower than the grammar check and would miss a `tsk_`, `rcp_`, or `evt_` value |
| Add a separate "display string" type | Nine fields do not justify a second parallel family of validated scalars; a category parameter on the existing macro is sufficient |