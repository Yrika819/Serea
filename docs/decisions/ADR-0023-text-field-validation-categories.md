# ADR-0023: Text Field Validation Categories

- Status: **Accepted** — complete O/L/P validation implemented in P2A
- Architecture version: `serea-arch/1.0.0` (current frozen contract set)
- Decision date: 2026-10-03 — owner direction
- Recorded by: P2 design preparation, from `c3737039e3e38dbba554dc0b9075025f87948358`
- Feeds: [P2 contract gap analysis](../plans/P2-contract-gap-analysis.md) §5.6

> Accepted on owner ratification after three corrected documentation gate reviews
> GREEN. P2A implements Rust validators (`types.rs`), affected schema occurrences
> and parity tests, including every receipt `provider_reference` and event actor.
> The coordinator owns final workspace/MSRV validation, bounded regression review
> and integration closure; current results and counts belong in the
> [closure record](../plans/P2A-review-and-closure.md), not an earlier-run summary.

## Context

At the P1 baseline, `validate_label` in `crates/serea-protocol/src/types.rs` rejected empty,
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
| **O** — opaque token / reference | `ActorId`, `LeaseOwner`, `ProviderReference` | Non-empty, no leading/trailing pinned whitespace. Refuse C0, DEL, C1 and U+2028/U+2029; refuse exact prefixed/fixed-shape domains below. |
| **L** — single-line label | `TaskTitle`, `DescriptorTitle`, `EffectSummary`, `PlainSummary` | Non-empty, no leading/trailing pinned whitespace. Refuse C0 (including LF/TAB), DEL, C1 and U+2028/U+2029. |
| **P** — prose | `ErrorMessage`, `DescriptorDescription` | Non-empty, no leading/trailing pinned whitespace. Permit interior LF/TAB/U+2028/U+2029. Refuse CR, other C0, DEL and C1. |

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

The affected checked-in schemas do **not** share one definition to be split, and an
earlier draft of this ADR described them as if they did. The real shape:

| File | How free text is written today | Fields affected |
| --- | --- | --- |
| `action-result.schema.json` | A `$defs/freeText` **definition**, `$ref`'d three times | `actor.id` (line 147, category **O**), `effect_summary` (176, **L**), `message` (193, **P**) |
| `assistant-task.schema.json` | **No** definition. The pattern is written inline at lines 39, 152, 353, 423, 428, 460 | `title` (**L**), `result_summary` (**L**), `lease_owner` (**O**), `effect_summary` (**L**), `message` (**P**) |
| `event.schema.json` | **No** definition. Inline pattern at line 93 | `actor.id` (**O**) |
| `action-result.schema.json`, `assistant-task.schema.json` | Nested receipt `provider_reference`, separate from freeText | **O**; inventory and parity-test each occurrence, including repeated receipt definitions |

So the work is: replace the single `freeText` definition in
`action-result.schema.json` with three, **keeping `actor.id` in scope** — omitting
it leaves that field on the old pattern and test A2 fails — and give the inline
occurrences in the other two files a `$ref` to the definition each one needs.

The whitespace and category rules are specified using negative lookaheads, which ECMA-262
supports and which this repository already uses in three capability-identifier
patterns:

Pin Unicode **White_Space** identically in Rust and schema: U+0009–000D,
U+0020, U+0085, U+00A0, U+1680, U+2000–200A, U+2028, U+2029, U+202F,
U+205F, U+3000. Do not use JavaScript `\s` (includes FEFF but omits U+0085),
locale rules, or an unpinned Unicode library predicate. U+FEFF and U+200B are not
members of this set. C1 remains refused independently of whitespace membership.
This conservative policy does **not widen event/1 category O**: no C1 character
previously rejected by Rust is newly admitted, and line separators are refused.

For generation, `W` below is a fragment, not a literal pattern character:

```text
W = [\u0009-\u000d\u0020\u0085\u00a0\u1680\u2000-\u200a\u2028\u2029\u202f\u205f\u3000]
B = ^(?!W)(?![\s\S]*W(?![\s\S]))
E = (?![\s\S])
L = B[^\u0000-\u001f\u007f-\u009f\u2028\u2029]+E
P = B[^\u0000-\u0008\u000b-\u001f\u007f-\u009f]+E
```

Substitute W into B and expand B/E when generating checked-in patterns. E is a
strict end-of-input assertion, avoiding `$`'s before-final-newline behavior.
P's character class deliberately leaves U+0009/U+000A allowed. These fragments
are specified for generation and must be exercised as expanded ECMA-262 patterns.

**Why the expansion implements the rule.** At start-of-input, B's first
lookahead refuses a first character in W; its second refuses exactly a last
character in W because `[\s\S]*` spans all characters and E asserts strict EOF.
The nonempty `+` class separately enforces the category's forbidden code points;
P excludes U+0000–0008 and U+000B–001F, leaving only TAB/LF from C0. Each O
negative lookahead refuses one complete grammar ending at E, not a prefix or
substring. ULID's first digit is bounded to 0–7, idk_/sha256: use distinct exact
separators, and the inner goallatch lookahead subtracts only that exact provider.
[The docs probe](../plans/p2a-doc-probes.py) expands these published fragments
and compares them with independent expected code-point/identifier predicates.
It does not replace the required production parity gate.

### Which identifier domains category O refuses — and why not all of them

The prose rule is *"refused if the value parses as any other frozen identifier
domain"*, and the question of which domains that means is **decided by
execution, not by preference**. Three candidate rules were implemented and run
against one corpus of sixteen impersonations (all eleven ULID prefixes,
`idk_`+64 hex, `sha256:`+64 hex, two `CapabilityId`s) and fourteen legitimate
opaque tokens (`calendar`, `worker`, `worker-1`, `host-a3f9`,
`session-42.worker`, `x`, `w`, a long reference, `provider:handle/1234`, and five
near-miss identifier shapes that must be *accepted*):

| Rule | Impersonations caught | False positives on legal opaque tokens |
| --- | --- | --- |
| **A** — the Serea ULID family only | 11 / 16 | 0 / 14 |
| **B** — every registered identifier domain | 15 / 16 | **8 / 14** |
| **C** — the prefixed and fixed-shape domains | **16 / 16** | **0 / 14** |

**B is untenable, and the evidence is specific rather than stylistic.**
`ProviderId` is `[a-z][a-z0-9_]{1,31}` and `ModelId` is
`^[a-z0-9]+(-[a-z0-9]+)*$`. Both subsume ordinary words. Under B the refused set
includes `calendar`, `worker`, `worker-1`, `host-a3f9`, `x` and `w` — that is,
**every plausible `LeaseOwner`** and most plausible `ProviderReference`. A rule
that refuses its own intended input is not stricter, it is dead.

So the rule is **C**, and the exclusion is stated rather than left implicit:

> Category O refuses a value that parses as a **prefixed or fixed-shape** frozen
> identifier: the eleven ULID prefixes `tsk_ stp_ apr_ grt_ req_ evt_ dev_ sch_
> prop_ rcp_ ses_`; `idk_` + 64 lowercase hex; `sha256:` + 64 lowercase hex; or a
> `CapabilityId`. It does **not** refuse `ProviderId`, `ModelId` or
> `ImplementationId`, because those grammars subsume ordinary words and the rule
> would then refuse every legitimate value.

The pattern, and it **is** expressible in ECMA-262, so Rust and JSON Schema can
produce identical verdicts:

Using B/E above, concatenate these fragments without presentation whitespace:

```text
B
 (?!(tsk|stp|apr|grt|req|evt|dev|sch|prop|rcp|ses)_[0-7][0-9A-HJKMNP-TV-Z]{25}E)
 (?!idk_[0-9a-f]{64}E)
 (?!sha256:[0-9a-f]{64}E)
 (?!(?!goallatch\.)[a-z][a-z0-9_]{1,31}\.[a-z][a-z0-9_]{1,31}
      \.(list|read|search|open|control|write|create|send|delete|start|status|run|cancel|result)E)
 [^\u0000-\u001f\u007f-\u009f\u2028\u2029]+E
```

`idk:`/`idk` and `sha256_`/`sha256` are near misses, not identifiers. A 26-character
ULID body starting above 7 is out of range and not an identifier. Subtract only
exact provider `goallatch`, not `goallatch_foo` or `goallatch1`.

Three properties of this pattern are load-bearing and each is a trap:

- **Each banned grammar is its own anchored negative lookahead**, `(?!…E)`, not a
  spliced alternation inside the negated character class. The earlier draft
  spliced, and the resulting pattern was not merely wrong but *inert*.
- **The `goallatch` exclusion.** `serea-protocol`'s `CapabilityId` validator
  refuses the `goallatch` provider namespace, so `goallatch.goal.run` is **not** a
  `CapabilityId`. A banned set that ignored this would refuse a value Rust
  accepts. `(?!goallatch\.)` makes the banned set exactly equal to
  `CapabilityId`'s accept set. A pattern that is merely *stricter* here would
  still be a parity bug.
- **The production pattern must be generated, never hand-written.** Both the eleven-prefix
  list and the fourteen-verb list come from `ids.rs`
  (`ULID`-family prefixes and `CAPABILITY_VERBS`), and a test asserts the
  generated pattern matches the frozen lists exactly. A hand-written copy drifts,
  and a drifted copy means the schema accepts an `ActorId` the Rust validator
  refuses — which is the failure mode this whole rule exists to prevent.

**The defect the audit found, for the record.** The pattern this ADR previously
published was

```text
^(?![ \t\n\r\f\v]*$)(?!.*:(tsk|stp|apr|grt|req|evt|dev|sch|prop|rcp|ses)_)
 [^\u0000-\u001f\u007f]+$
```

which requires a **literal colon immediately before** the prefix. No Serea
identifier has one — `tsk_01JQ8Z9K3M7QWXR4V2T6YH0BNA` contains no colon at all.
Executed under ECMA-262, that pattern fires *only* on strings of the form
`a:tsk_`, `x:stp_…`, `sha256:tsk_…`, none of which can occur as an `ActorId`,
`LeaseOwner` or `ProviderReference`. Every real frozen identifier was accepted.
It had seven divergences across a fourteen-case corpus, including the exact case
this ADR's own adversarial corpus names: *"a `stp_`-prefixed ULID in an O
field"*. The rule was inert, and this ADR's claim that *"every field's rejection
must be identical on both sides"* was false in the only direction that matters.

**The fallback this ADR reserved is therefore not needed.** It said that "if
generating it proves impractical, the honest fallback is to state the
impersonation rule as Rust-only, and narrow test A2". Generation proved
practical — eleven prefixes and fourteen verbs are a small table — and the full
rule is expressible in both surfaces, so **A2 stays whole**. The fallback remains
recorded as the correct response if the verb set ever grows enough to make the
generated pattern unwieldy, because a narrowed A2 must be a decision rather than
a drift.

P2A must prove identical rejection on both sides; a required test enumerates every
field against a shared corpus of adversarial strings — empty, whitespace-only,
leading and trailing whitespace, `\n`, `\r`, `\t`, NUL, DEL, **each of the eleven
ULID prefixes in an O field**, `idk_`+hex, `sha256:`+hex, a real `CapabilityId`,
a `CapabilityId` with an unknown verb, `goallatch.goal.run`, a `ProviderId`-shaped
and a `ModelId`-shaped ordinary token, a 5000-character string, and a non-ASCII
string — asserting the same verdict from the Rust validator and the schema. That
required test is the mechanism to prevent recurrence; it would have caught the
inert pattern. Historical corpus success and the docs-only regex probe do not
prove production Rust/schema parity.

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

All validation amendments, source validators, schema occurrences, generated
patterns and tests belong to the coordinator's atomic P2A integration; current
validation/review/closure evidence is recorded in the
[closure record](../plans/P2A-review-and-closure.md).

1. Capability Protocol §3.1 gains a paragraph defining the three text categories
   and assigning every free-text field to one, with the impersonation rule stated
   for category O.
2. Task Protocol §3.1's table gains the category for `lease_owner`.
3. A changelog section in each.

## P2A code change, same atomic delivery

| File | Change |
| --- | --- |
| `crates/serea-protocol/src/types.rs` | `validate_opaque_token`, `validate_single_line_label`, `validate_prose` replace the single `validate_label`; each `declare_value!` invocation names its category |
| `crates/serea-protocol/schemas/assistant-task.schema.json` | Six inline patterns replaced by `$ref`s to the definition their field's category needs: `title`, `result_summary` (L); `lease_owner` (O); `effect_summary` (L); `message` (P) |
| `crates/serea-protocol/schemas/action-result.schema.json` | `freeText` replaced by three definitions; `actor.id` (O), `effect_summary` (L), `message` (P) recategorised |
| `crates/serea-protocol/schemas/event.schema.json` | The inline pattern at line 93 replaced by a `$ref` to the opaque-token definition |
| A test asserting the generated category-O pattern matches the frozen lists | So the eleven prefixes and the fourteen verbs cannot drift. Both lists come from `ids.rs`; a drifted copy means the schema accepts an `ActorId` the Rust validator refuses |
| `crates/serea-protocol/tests/protocol_types.rs` | Per-category accept and reject cases, including the eleven prefixes, `idk_`, `sha256:`, a real and a near-miss `CapabilityId`, and the six legitimate opaque tokens |
| `crates/serea-protocol/tests/schema_contracts.rs` | The shared adversarial corpus, asserted equal on both sides. **This is the test that would have caught the inert pattern** |

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
already applies to the four `TaskStep` conversions in ADR-0018.

**The owner has ratified the P2A version plan in the reconciliation request.** The
narrowing of the schema and the widening of `ErrorMessage` place this ADR at
**minor on `serea.action/1`**, recorded once in
[the audit's M6](../plans/P2-autonomous-audit.md) alongside ADR-0018 and ADR-0019
rather than decided separately here. Two positions were withdrawn by the P2
autonomous audit for taking a third, inconsistent view: ADR-0019 had
self-classified as a minor *clarification* when its integer-only number rule
narrows frozen Protocol Index §5, and the gap analysis had left the whole
question open. See [the decision ledger](../plans/P2-tomorrow-decision-ledger.md).

## Consequences

- Nine fields stop sharing one validator, so a future author picks a category
  instead of inheriting a default.
- The P2A implementation must close the divergence recorded by P1 and prove
  the rule with accepting and rejecting parity cases; docs alone do not close it.
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
| Refuse `ProviderId`, `ModelId` and `ImplementationId` shapes too | Measured, not assumed: doing so produces **8 false positives out of 14** legitimate opaque tokens, including `calendar` and `worker` — every plausible `LeaseOwner`. Their grammars subsume ordinary words, so the rule would refuse its own input |
| Add a separate "display string" type | Nine fields do not justify a second parallel family of validated scalars; a category parameter on the existing macro is sufficient |
