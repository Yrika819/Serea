# ADR-0019: Canonical JSON (SCJ-1) and the Idempotency Preimage (IDK-1)

- Status: **Proposed** — pending implementation and owner ratification
- Architecture version: `serea-arch/0.2.0` at the time of writing
- Decision date: not yet ratified
- Recorded by: P2 design preparation, from `c3737039e3e38dbba554dc0b9075025f87948358`
- Feeds: [P2 contract gap analysis](../plans/P2-contract-gap-analysis.md) §5.3,
  §5.4

> This ADR changes no frozen protocol text and no code. The amendments below are
> **drafted, not applied**.

## Context

[Protocol Index §5](../protocols/00-protocol-index.md#5-serialization) states, in
one sentence: *"Canonical JSON for digesting: keys sorted lexicographically by
UTF-8 code point, no insignificant whitespace, no trailing newline, UTF-8, numbers
in shortest round-trip form."* No function in the repository implements it, and
`Digest` is a validated newtype that nothing produces.

[Capability Protocol §8.2](../protocols/01-capability-protocol.md#82-idempotency-key-derivation)
defines the per-step idempotency key as

```text
idempotency_key = idk_ + hex(sha256(
    task_id ‖ step_id ‖ capability_id ‖ capability_version ‖ canonical_json(arguments)
))
```

No document defines `‖`. P1 closure already records the related `serde_json`
limitation: without `arbitrary_precision`, "two distinct integers above
`u64::MAX` collapse to one `Value`".

Two independent defects therefore exist. The first is that the canonicalization
rule is underdetermined. The second is worse and is not hypothetical.

## The defect that decides this ADR

Treating `‖` as raw concatenation produces a **collision on input that is legal
under every frozen grammar**:

| Tuple | `capability_id` | `capability_version` | `arguments` | Naive preimage |
| --- | --- | --- | --- | --- |
| **A** | `p.r.list` | `0.0.0` | `-12` | `p.r.list0.0.0-12` |
| **B** | `p.r.list` | `0.0.0-1` | `2` | `p.r.list0.0.0-12` |

Every component is legal: `p.r.list` is three segments with `list` drawn from the
frozen verb set; `0.0.0` and `0.0.0-1` are both valid SemVer (`1` is a valid
numeric pre-release identifier); `-12` and `2` are both valid canonical JSON
integers.

A generated-corpus search over the frozen grammars — 14 verbs from Capability
Protocol §2, 405 SemVer forms built from a 3 × 3 × 3 grid of numeric identifiers
with optional pre-release and build metadata, and 20 argument forms — examines
113 400 triples and finds this family and no other. The collision is always between
a **SemVer pre-release identifier** and a **negative argument integer**. A second
search of 39 424 triples finds no collision across the
`capability_id`/`capability_version` boundary, because every identifier segment must
start `[a-z]` and every SemVer numeric identifier must start with a digit. That is
an accident of three independently-maintained grammars, not a stated invariant,
and nothing in the repository tests for it — which is why test D13 is a canary that
fails loudly if a future grammar change makes the naive form collide.

The two tuples are *different actions*: different pinned descriptor versions,
different arguments, and different `arguments_digest`
(`sha256:7ed00270e394c3e190d18a977f5d0e1ed889bdc03a637e8fe35f00946841b0bf` versus
`sha256:d4735e3a265e16eee03f59718b9b5d03019c07d8b6c51f90da3a666eec13ab35`).
Sharing one key would let:

- `C6` and `B11` suppress a *different* action as a duplicate;
- a crash replay of B be recognised as A;
- an Approval Protocol grant bound to A's exact `arguments_digest` authorise B,
  because the value identifying the action is the same;
- two steps in one task collide on `UNIQUE (task_id, idempotency_key)`, turning a
  *wrong* key derivation into a spurious "duplicate key" failure.

## Decision

### SCJ-1 — canonical JSON

1. **The entry point is text, never a `Value`.** `canonicalize(&str) -> Vec<u8>`
   is the only entry point. A `serde_json::Value` has already lost duplicate
   object keys and cannot be checked for them, so P2 never digests one:
   `put_blob` takes `&[u8]` and canonicalises, and there is deliberately **no**
   `put_blob_value`. This closes the parser-differential hole at the type level
   rather than by convention.
2. **Duplicate object keys are rejected** with a typed error, detected by a
   `serde_json` visitor that tracks seen keys. `serde_json` silently keeps the
   last occurrence, so without this rule a document that another implementation
   reads as the *first* occurrence would digest as the last — on a value that
   decides whether an external effect is suppressed.
3. **Member ordering** is by the UTF-8 byte sequence of the key. For UTF-8 this
   equals ordering by Unicode scalar value, so "by UTF-8 code point" is satisfied
   exactly, with no locale or collation involved.
4. **No insignificant whitespace** anywhere; `:` and `,` carry no space. No
   trailing newline. UTF-8, no BOM.
5. **Escaping is a closed list.** `\"`, `\\`, `\b`, `\f`, `\n`, `\r`, `\t`. Every
   other C0 control and U+007F as lowercase `\u00xx`. Everything else as raw
   UTF-8, including non-ASCII, U+2028, U+2029 and `/`. Ill-formed UTF-8 is
   refused at the parse boundary, so a lone surrogate cannot occur.
6. **Numbers are integers only**, in `-2^63 ..= 2^64-1`, written as a decimal
   integer with no exponent, no leading `+`, no leading zero and no `-0`. Any
   `f64` is a hard error. Because `serde_json` parses an integer above `u64::MAX`
   as `f64`, this turns P1's silent collapse into a detectable condition:
   `u64::MAX` canonicalises, `u64::MAX + 1` is refused. Two distinct large
   integers and a float can neither reach nor collide in a digest.
7. **Array order is preserved.** Order is semantic in JSON arrays.
8. **The root may be any value**, including an integer or a string.
9. **Nesting deeper than `MAX_INSTANCE_DEPTH = 64` is refused** before
   canonicalisation, reusing the existing constant and its rationale: a stack
   overflow aborts the process, which is not a graceful refusal.

**Stated cost of rule 6.** A capability whose `input_schema` legitimately admits a
fractional number cannot have a stable digest under SCJ-1. P5 must decide per
capability whether to admit `arbitrary_precision` or to declare the field
non-canonicalisable. That is a P5 obligation, recorded here rather than inherited
silently. The alternative — defining a portable shortest-round-trip form for
`f64` — is rejected because there is no single portable spelling, so two
conforming implementations could derive different digests for the same value.

### IDK-1 — the idempotency preimage

```text
preimage :=
    b"serea.idempotency.v1\x00"          21 bytes, domain separation
  || u8(5)                              field count, pinned by version
  || lp("task_id")              || lp(task_id_bytes)
  || lp("step_id")             || lp(step_id_bytes)
  || lp("capability_id")       || lp(capability_id_bytes)
  || lp("capability_version")  || lp(capability_version_bytes)
  || lp("arguments_canonical") || lp(scj1(arguments)_bytes)

lp(x) := u64 big-endian byte length of x, followed by x
idempotency_key := "idk_" + lowercase_hex(sha256(preimage))
```

Injective **by construction**, rather than by an accident of three grammars. The
field-count byte makes truncation detectable and makes a future preimage with a
different field set impossible to confuse with this one.

### The distinction that keeps change control small

| Digest | Frozen definition | Encoding |
| --- | --- | --- |
| `arguments_digest` | "sha256 over the canonical JSON of `arguments`" (Capability Protocol §4.3) | `sha256(SCJ-1(x))` — **unchanged** |
| `input_digest`, `result_digest` | "sha256 over the canonical JSON of …" (Task Protocol §3.1) | `sha256(SCJ-1(x))` — **unchanged** |
| Evidence `payload_digest` | "sha256 over the canonical JSON of the payload" (Capability Protocol §7) | `sha256(SCJ-1(x))` — **unchanged** |
| `idempotency_key` | a hash over a **tuple of named fields** (Capability Protocol §8.2) | domain-separated and framed |

A content digest hashes one document, so its preimage is the document and there
is nothing to frame. Only a *structured* derivation over several named fields
needs a domain tag and an injective encoding. Every frozen sentence about a
content digest therefore stays exactly as written.

## Fixed test vectors

Computed from this specification, not illustrative. P2B pins them as literal
constants in `crates/serea-protocol/tests/canonical_vectors.rs`.

### SCJ-1

| # | Input text | Canonical bytes | `sha256:` |
| --- | --- | --- | --- |
| 1 | `{"b":1,"a":2}` | `{"a":2,"b":1}` | `sha256:d3626ac30a87e6f7a6428233b3c68299976865fa5508e4267c5415c76af7a772` |
| 2 | `{"a":{"z":[3,1,2],"y":null},"b":true}` | `{"a":{"y":null,"z":[3,1,2]},"b":true}` | `sha256:754ee7a1aee4ccd0efc11f0a8de464fc62b47a2c784547ccf0fa37e3f03fdf2e` |
| 3 | `{}` | `{}` | `sha256:44136fa355b3678a1146ad16f7e8649e94fb4fc21fe77e8310c060f61caaff8a` |
| 4 | `[]` | `[]` | `sha256:4f53cda18c2baa0c0354bb5f9a3ecbe5ed12ab4d8e11ba873c2f11161202b945` |
| 5 | `42` | `42` | `sha256:73475cb40a568e8da8a045ced110137e159f890ac4da883b6b17dc651b3a8049` |
| 6 | `-7` | `-7` | `sha256:a770d3270c9dcdedf12ed9fd70444f7c8a95c26cae3cae9bd867499090a2f14b` |
| 7 | `18446744073709551615` | `18446744073709551615` | `sha256:2cdb26265b4dc65e3b44d694f121fd6de99b9e4b8ae7f08d84bfa9537635ae43` |
| 8 | `{"k": "q\"b\\s\nt\tu\u0001v/é"}` | `{"k":"q\"b\\s\nt\tu\u0001v\u007f/é"}` | `sha256:e1e4c6bf233f76ae93bbd29dcd61c9d7704064e2d8ab627ba2310118de3d7a16` |
| 9 | `{ "a" : [ 1 , 2 ] , "b" : { } }` | `{"a":[1,2],"b":{}}` | `sha256:8c547cce7ccb1b89359479c0b71a0a4b62acfc54a2b2780fd34aaeb75f9e44b7` |
| 10 | `{"range":"tomorrow","limit":25,"opts":{"tz":"Asia/Tokyo","flags":["a","b"],"n":null}}` | `{"limit":25,"opts":{"flags":["a","b"],"n":null,"tz":"Asia/Tokyo"},"range":"tomorrow"}` | `sha256:12820828e332666cbc4a22dbaed9e5c192bdfbd7ce8a61ff2eb444a9e8538351` |

Vector 8 pins the escape table: `\u0001` and `\u007f` are escaped to their
lowercase four-digit forms, `/` and `é` are emitted raw, and `\n`/`\t` use their
two-character forms. Vector 9 pins whitespace removal. Vector 1 pins member
ordering. Vector 2 pins nested ordering and that array order is preserved.

### IDK-1, all with `task_id = tsk_01JQ8Z9K3M7QWXR4V2T6YH0BNA` and
`step_id = stp_01JQ8Z9M3R2CVN8H5FWK7PQDSF`

| # | `capability_id` | `capability_version` | `arguments` | `idempotency_key` |
| --- | --- | --- | --- | --- |
| 1 | `calendar.events.list` | `1.2.0` | `{"range":"tomorrow","limit":25}` | `idk_f8d17a2f6fb40db5a3421e035cde37a3234e628381cbb56791c14195f456b1cb` |
| 2 | `calendar.events.list` | `1.2.0` | `{"limit":25,"range":"tomorrow"}` | `idk_f8d17a2f6fb40db5a3421e035cde37a3234e628381cbb56791c14195f456b1cb` |
| 3 | `calendar.event.create` | `1.0.0` | `{}` | `idk_820ecdf813cba9cb22628af8fe0133965e201764d390c968ce23732e5c90ad0d` |
| 4 | `calendar.events.list` | `1.2.1` | `{"range":"tomorrow","limit":25}` | `idk_fe8bc5a29cd8df7c9092d894d9a29b16daa1df6f4f84ea98d2b44d47512c01e8` |
| 5 | `calendar.events.list` | `1.2.0` | `{"range":"tomorrow","limit":25}`, `task_id` = `tsk_01JQ8Z9K3M7QWXR4V2T6YH0BNB` | `idk_8303c302796fda66c38aec019cbb5f6b86309bbdcdb41001a08873cbc83c4fbd` |
| **A** | `p.r.list` | `0.0.0` | `-12` | `idk_b9e8299d5b628af7d40e253035ce7af0f653a4523a20448ea061bea316f0adaf` |
| **B** | `p.r.list` | `0.0.0-1` | `2` | `idk_141fa78316b05b374a0a11a2fe7093880daa19598dddc9b22675ffadedf232ef` |

Vectors 1 and 2 are equal and pin the member-ordering invariance.
Vectors 1, 3, 4 and 5 are pairwise distinct and pin that each of
`capability_id`, `capability_version` and `task_id` actually participates.
Vectors **A** and **B** are the collision pair from the Context section: under the
naive `‖` reading their concatenated suffix is the same 16 bytes —
`p.r.list0.0.0-12` — and therefore the same key; under IDK-1 they differ. That pair
is the regression test for this ADR. Both derived values were independently
recomputed with `sha256` during design preparation and match.

## Proposed amendment

Applied to `docs/protocols/00-protocol-index.md` and
`01-capability-protocol.md` only in the same commit that implements it. Nothing
here is applied by this run.

1. Protocol Index §5 gains a subsection defining SCJ-1 by reference to this ADR,
   with the entry-point-is-text rule and the integer-only number rule stated
   inline, because those two are the ones a reader will otherwise get wrong.
2. Capability Protocol §8.2 replaces the `‖` expression with a reference to IDK-1
   and its byte layout.
3. Capability Protocol §8.2 gains the sentence: *"The key is absent on a step
   whose `kind` is not `CAPABILITY`, `DELEGATE` or `VERIFY`; those steps carry no
   external effect and derive no key."* — which cross-references ADR-0018 §4.
4. A changelog section is created in each of the two protocol documents.

## Code change, same commit

| File | Change |
| --- | --- |
| `Cargo.toml` | One new workspace dependency providing `sha256`. Named, not added, by this run |
| `crates/serea-protocol/src/canonical.rs` | `CanonicalJsonError`, `canonicalize`, `digest_of`, `derive_idempotency_key`, the duplicate-key visitor |
| `crates/serea-protocol/src/lib.rs` | `pub mod canonical;` plus re-exports |
| `crates/serea-protocol/tests/canonical_vectors.rs` | The ten SCJ-1 vectors and the seven IDK-1 vectors as literals |
| `crates/serea-protocol/tests/canonical_properties.rs` | Idempotence, member-ordering invariance, injectivity over a generated corpus, and a naive-collision canary over the frozen grammars |

## Change control and compatibility

Capability Protocol §8.2's `‖` notation is prose that never defined an encoding,
so making the encoding explicit is a specification-precision change to a
*structured* derivation — with no cross-version obligation, because P1 computed no
digest and Serea has minted no key. This ADR records it as an architecture-minor
clarification.

If the owner instead judges the frozen formula **normative** — that is, if `‖`
was meant to mean something other than raw concatenation — then §8.2 must be
amended in the same commit that implements IDK-1, and the collision above is the
evidence that the amendment is required rather than optional.

## Consequences

- Every `Digest` in Serea becomes computable, which unblocks argument content
  addressing, `input_digest`, `result_digest`, future evidence payload digests,
  and duplicate suppression.
- The canonicalizer becomes the single choke point for untrusted JSON in the
  digest path, which is where `TB-11` retrieved content is supposed to be
  validated.
- SCJ-1 rejects floats, so `arguments` containing a fractional number is refused
  rather than digested ambiguously. That is a visible behaviour change and a P5
  obligation.
- Duplicate-key rejection means an `arguments` document that a provider emitted
  with a repeated key is refused. Providers should be reviewed for it.

## Rejected alternatives

| Alternative | Why rejected |
| --- | --- |
| "Just sort the keys" | Four of the five clauses of Protocol Index §5 are underdetermined; four different implementations would produce four different digests |
| Shortest round-trip `f64` form | No portable spelling; two conforming implementations can differ, so the digest is not portable |
| Percent- or C0-escaping every preimage field | Equivalent to length prefixing with more moving parts and no correctness gain |
| `\|`-separated framing | `arguments` is arbitrary validated JSON and `ProviderReference` is opaque and never parsed, so a separator character can occur in a preimage |
| Newline framing | Same, plus a literal newline can occur in a JSON string argument |
| Fixed-width fields only | `capability_version` and the arguments document are variable-length |
| Last-wins duplicate keys | Parser differential on a value that decides whether an external effect is suppressed |
| Accepting a `Value` for convenience | It has already lost duplicate keys and cannot be checked for them |