# ADR-0019: Canonical JSON (SCJ-1) and the Idempotency Preimage (IDK-1)

- Status: **Accepted** — full SCJ-1/digest/duplicate-aware parsing and IDK-1 implemented in P2A
- Architecture version: `serea-arch/1.0.0` (current frozen contract set)
- Decision date: 2026-10-03 — owner direction
- Recorded by: P2 design preparation, from `c3737039e3e38dbba554dc0b9075025f87948358`
- Feeds: [P2 contract gap analysis](../plans/P2-contract-gap-analysis.md) §5.3,
  §5.4

> Accepted on owner ratification after three corrected documentation gate reviews
> GREEN. SCJ-1, digest, duplicate-aware parsing and named IDK-1 with sha2 0.11
> without defaults belong to the same atomic action/2 P2A delivery, not P2B.
> Actual final validation, test counts, independent review and integration evidence
> are recorded in the [closure record](../plans/P2A-review-and-closure.md).
> Precision/raw transport is patched to preserve ordinary literal JSON objects;
> canonical numeric lexical admission remains independently guarded.

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

Historical raw-string framing experiment (not legal ActionRequests): treating
`‖` as raw concatenation gives this mathematical collision:

| Tuple | `capability_id` | `capability_version` | `arguments` | Naive preimage |
| --- | --- | --- | --- | --- |
| **A** | `p.r.list` | `0.0.0` | `-12` | `p.r.list0.0.0-12` |
| **B** | `p.r.list` | `0.0.0-1` | `2` | `p.r.list0.0.0-12` |

`p.r.list` is **invalid**: provider/resource require two-character minimum
segments. The versions and scalar roots are legal for generic SCJ-1/framing, but
ActionRequest requires object-root arguments. Typed public derivation validates its identifier arguments and accepts any SCJ-1
root, independently of ActionRequest's object-only rule. It rejects `p.r.list`; these vectors belong only in low-level private raw-string framing tests. Repeating the
experiment with `pp.rr.list` gives a legal capability ID but still generic scalar
inputs, not ActionRequests. This is not a demonstrated collision of legal actions.

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

The raw tuples differ in version and scalar digest, but are not executable
actions. Framing avoids boundary ambiguity without relying on input grammars.
The earlier approval-transfer claim was false: Approval Protocol §4.1 checks
argument digest, capability and version independently. Equal keys do not authorize
B. Nor does this fixed-StepId pair prove a two-step SQL uniqueness collision;
distinct StepIds participate in the preimage.

## Decision

### SCJ-1 — canonical JSON

1. **The entry point is text, not a `Value`.** `canonicalize(&str)` returns a
   fallible canonical byte result. Reject duplicates before constructing a Value.
   Already discarded duplicate keys cannot be reconstructed: callers must preserve
   original text at the raw-input boundary. A string API cannot prove absolute
   raw provenance (a caller can serialize an already-lossy Value). Future `put_blob`
   takes bytes, not `put_blob_value`; this does not eliminate that caller obligation.
2. **Duplicate object keys are rejected** with a typed error, detected by a
   `serde_json` visitor that tracks seen keys. `serde_json` silently keeps the
   last occurrence, so without this rule a document that another implementation
   reads as the *first* occurrence would digest as the last — on a value that
   decides whether an external effect is suppressed. Diagnostic precedence is
   intentionally preflight-first: a forbidden numeric spelling anywhere reached
   by lexical preflight can return `NonInteger` (or `InvalidJson` for malformed
   numeric syntax) before the visitor reports a duplicate or another syntax error.
   Multiply-invalid documents always refuse without canonical bytes, digest or key;
   `DuplicateKey` is not promised as the first error for every such document.
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
   decimal/exponent spelling is a hard error, even if integer-valued. The
   implemented `canonicalize` runs lexical raw-numeric preflight before the
   duplicate-aware serde visitor: spelling, negative zero and range are checked
   before normalization or synthetic number-map dispatch can occur. Admission
   stays stable under consumer-unified `serde_json/arbitrary_precision`, not
   merely the workspace's default feature set. `u64::MAX` canonicalises;
   `u64::MAX + 1` is refused rather than silently collapsing to `f64` or an object.
   Two distinct out-of-domain integers and a float cannot reach a digest.
   Task wire generation's positive-u32 decoder separately accepts `1.0`/`1e0`;
   that schema-integer parity does not widen SCJ-1's canonical-number domain.
7. **Array order is preserved.** Order is semantic in JSON arrays.
8. **The root may be any value**, including an integer or a string.
9. **Nesting deeper than `MAX_INSTANCE_DEPTH = 64` is refused** before
   canonicalisation, reusing the existing constant and its rationale: a stack
   overflow aborts the process, which is not a graceful refusal.

**Stated cost of rule 6, and the real reason for it.** A capability whose
`input_schema` legitimately admits a fractional number cannot have a stable digest
under SCJ-1. A future decision must define fractional encoding/range or keep the input
non-canonicalisable; `arbitrary_precision` alone is not a canonical-number rule. That is a P5 obligation, recorded
here rather than inherited silently.

An earlier draft of this ADR rejected a shortest-round-trip `f64` rule with the
reason *"there is no single portable spelling, so two conforming implementations
could derive different digests for the same value."* **That reason is false, and
the P2 autonomous audit corrected it.** RFC 8785 §3.2.2.3 requires numbers to be
serialized per ECMAScript §7.1.12.1 `Number::toString` including the "Note 2"
enhancement, and names Ryu as a reference implementation. ECMAScript's
`Number::toString` is fully specified, is the shortest round-tripping form, and is
exact-integer based, so no target-dependent floating-point behaviour is involved
and `x86_64` and `aarch64` produce identical bytes. **A single portable spelling
does exist.**

Rule 6 is therefore kept for two *different* reasons, and only the second is
load-bearing:

1. **SCJ-1 deliberately has a limited integer-only domain.** ModelRequest already
   carries `temperature: f64` (wire example 0.2). SCJ-1 does not change model wire
   validation and does not automatically cover every blob or MODEL_TURN input.
   Future runtime digest/storage paths must reject noncanonical model documents.
   Fractional support requires a future canonical-number/range decision; never
   truncate or coerce temperature to manufacture a digest.
2. **Full JCS adoption is unavailable anyway, for an unrelated reason.** RFC 8785
   §3.2.3 sorts object properties by **UTF-16 code units**, and warns explicitly
   that "sorting data encoded in UTF-8 or UTF-32 would also work, but the outcome
   for JSON data like above would differ and thus be incompatible with this
   specification." Frozen Protocol Index §5 says **"keys sorted lexicographically
   by UTF-8 code point"**. The two orderings genuinely disagree: in UTF-16 an
   astral character is a surrogate pair beginning `D800`, which sorts *before*
   `U+E000–U+FFFF`, whereas in UTF-8 it is a four-byte sequence beginning `F0`,
   which sorts *after*. Adopting JCS wholesale would contradict frozen text.

So the choice is not "custom versus standard". It is: keep the frozen §5 ordering,
and defer the number rule until a capability needs it.

**If and when rule 6 changes, the dependency is `ryu-js`, not `ryu` and not
`std`.** Verified against RFC 8785 Appendix B: Rust's `f64` `Display` mismatches
five of the twelve reference values — `-0.0` (`-0` vs `0`), `1e30` and `1e-27`
and `1.7976931348623157e308` and `5e-324` (all printed as full expansions rather
than exponent forms), and `1424953923781206.25`, where the round-to-even case
yields `1424953923781206.3` in Rust against `1424953923781206.2` in ECMAScript.
`ryu` produces a shortest round-trip form that is not the ECMAScript form;
`ryu-js` implements the ECMAScript `Number::toString` algorithm and is the crate
that would satisfy RFC 8785.

**Named trigger for revisiting rule 6:** a capability whose `input_schema`
admits a fractional number. At that point the rule becomes "integers as
SCJ-1 rule 6, fractions per ECMAScript §7.1.12.1 with `Note 2`", the range must
be ratified, and `ryu-js` enters P5 — not P2A.

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
needs a domain tag and an injective encoding. The SHA-256 document-preimage definition is unchanged; admissible numeric domain
is explicitly narrowed and requires the ratified major treatment.

## Fixed test vectors

Computed from this specification, not illustrative. P2A pins them as literal
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
| 8 | `{"k": "q\"b\\s\nt\tu\u0001v\u007f/é"}` | `{"k":"q\"b\\s\nt\tu\u0001v\u007f/é"}` | `sha256:e1e4c6bf233f76ae93bbd29dcd61c9d7704064e2d8ab627ba2310118de3d7a16` |
| 9 | `{ "a" : [ 1 , 2 ] , "b" : { } }` | `{"a":[1,2],"b":{}}` | `sha256:8c547cce7ccb1b89359479c0b71a0a4b62acfc54a2b2780fd34aaeb75f9e44b7` |
| 10 | `{"range":"tomorrow","limit":25,"opts":{"tz":"Asia/Tokyo","flags":["a","b"],"n":null}}` | `{"limit":25,"opts":{"flags":["a","b"],"n":null,"tz":"Asia/Tokyo"},"range":"tomorrow"}` | `sha256:12820828e332666cbc4a22dbaed9e5c192bdfbd7ce8a61ff2eb444a9e8538351` |

Vector 8 pins the escape table: `\u0001` and `\u007f` are escaped to their
lowercase four-digit forms, `/` and `é` are emitted raw, and `\n`/`\t` use their
two-character forms. Vector 9 pins whitespace removal. Vector 1 pins member
ordering. Vector 2 pins nested ordering and that array order is preserved.

**Vector 8's input was corrected by the P2 autonomous audit; the hash was not.**
The published input omitted the `\u007f` escape, so the claimed canonical bytes
contained a `U+007F` the input never had — which cannot happen, because
canonicalization is a function. Recomputed independently:

| | |
| --- | --- |
| `sha256` of the **old** input canonicalised | `52f38c8cf283fe4c27906193c127759a3dc55a2c09d3193e52aa4795fc859a3c` |
| `sha256` of the **old** claimed canonical bytes | `e1e4c6bf233f76ae93bbd29dcd61c9d7704064e2d8ab627ba2310118de3d7a16` |
| Published | `e1e4c6bf233f76ae93bbd29dcd61c9d7704064e2d8ab627ba2310118de3d7a16` |

The hash matched the canonical bytes, so the **input** was the defective element.
Two repairs were available: add `\u007f` to the input, or drop it from the
canonical bytes. The input was corrected, for two reasons. The constant does not
move. And the vector's own purpose — pinning that `\u007f` escapes to its
lowercase four-digit form — is only served by an input that contains one; the
other repair would have left a vector named "pins the escape table" that never
exercises `U+007F`. This is the audit's own lesson restated: **an asserted
constant is not a verified constant, and neither is an unexercised vector.**

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
Vectors **A** and **B** retain historical hashes solely as mathematical raw-string
framing vectors. Their common naive suffix `p.r.list0.0.0-12` is **15 bytes**, not
16. Typed public derivation rejects the invalid ID. Independently pin the legal-ID
`pp.rr.list` scalar framing regression below, plus typed public derivation of both valid-ID scalars and legal object arguments. Hash agreement is not input-domain validity.

### Legal-ID regression vectors (independently derived in reconciliation)

With the same task/step IDs above, use `pp.rr.list`:

| Domain | Version | SCJ-1 bytes | IDK-1 SHA-256 key |
| --- | --- | --- | --- |
| Generic scalar framing | `0.0.0` | `-12` | `idk_1796e5d503926eb7f41f6dd59234e4b8f6de7526650c8911f9d0e68c4f87abd1` |
| Generic scalar framing | `0.0.0-1` | `2` | `idk_c2df4132f97df1dddabce31306238753086962b0e3aea7366dfdb7fe3fdec06a` |
| Legal ActionRequest object arguments | `0.0.0` | `{"n":-12}` | `idk_1a0326cb75273a11e56017804e9c1e187f60b3ab65eff3c87e8777bd6009f9f4` |
| Legal ActionRequest object arguments | `0.0.0-1` | `{"n":2}` | `idk_8d00b0d917a9e90716a255d02af048fdd6589960b6dd336e188f99630b68ff35` |

The scalar naive suffix is `pp.rr.list0.0.0-12` (17 bytes), identical for both
scalar rows. Named-framing preimages are 244 bytes each and differ; object rows
are 250 bytes each and have different naive suffixes as well. These are independent
Python SHA-256/struct reconstructions, not evidence of Rust tests passing.

The historical 113400/39424 corpus search used invalid short segments and scalar
roots. Its counts remain historical evidence only, not a legal-action completeness
claim. P2A reconstructs the corpus with legal IDs, distinguishes private raw
framing, typed generic derivation and object-root ActionRequest domains, and tests
injectivity of encoded preimages rather than claiming SHA-256 mathematically
injective.

## Proposed amendment

Protocol amendment, source, schemas, manifest, versions, tests and migration
notes belong to the same P2A integration gate. This run edits documentation only.

1. Protocol Index §5 gains a subsection defining SCJ-1 by reference to this ADR,
   with the entry-point-is-text rule and the integer-only number rule stated
   inline, because those two are the ones a reader will otherwise get wrong.
2. Capability Protocol §8.2 replaces the `‖` expression with a reference to IDK-1
   and its byte layout.
3. Capability Protocol §8.2 gains the sentence: *"The key is absent on a step
   whose `kind` is not `CAPABILITY`, `DELEGATE` or `VERIFY`; those steps carry no
   external effect and derive no key."* — which cross-references ADR-0018 §4.
4. A changelog section is created in each of the two protocol documents.

## P2A code change, same atomic delivery

| File | Change |
| --- | --- |
| `Cargo.toml`, `Cargo.lock`, `crates/serea-protocol/Cargo.toml`, `vendor/` | sha2 0.11 without defaults; exact JSON/schema numeric admission and literal-object-preserving patches described in launch §3, no storage/runtime |
| `crates/serea-protocol/src/canonical.rs` | `CanonicalJsonError`, `canonicalize`, `digest_of`, `derive_idempotency_key`, the duplicate-key visitor |
| `crates/serea-protocol/src/lib.rs` | Implemented `pub mod canonical;` and root re-exports of `CanonicalJsonError`, `canonicalize`, `digest_of`, `derive_idempotency_key` |
| `crates/serea-protocol/tests/canonical_vectors.rs` | Ten SCJ-1 vectors, five historical typed-object IDK-1 vectors, two historical A/B vectors only via private raw framing, and four valid-ID scalar/object vectors via typed public derivation |
| `crates/serea-protocol/tests/canonical_properties.rs` | Idempotence, member-ordering invariance, injectivity over a generated corpus, and a naive-collision canary over the frozen grammars |

### Implemented public API

The public module is `serea_protocol::canonical`; `lib.rs` also re-exports:

```rust
canonicalize(text: &str) -> Result<Vec<u8>, CanonicalJsonError>
digest_of(text: &str) -> Result<Digest, CanonicalJsonError>
derive_idempotency_key(
    task: &TaskId,
    step: &StepId,
    capability: &CapabilityId,
    version: &SemVer,
    arguments: &str,
) -> Result<IdempotencyKey, CanonicalJsonError>
```

Typed generic derivation accepts any SCJ-1 root, including scalars; it does not
change `ActionRequest.arguments`' object-only contract. Invalid `p.r.list` cannot
enter the typed API; the old A/B vectors remain private raw-framing regressions.
The API accepts original JSON text, not `Value`; preserving original text before
any lossy parse remains the caller's obligation.

## Change control and compatibility

Capability Protocol §8.2's `‖` notation is prose that never defined an encoding,
so making the encoding explicit is a specification-precision change to a
*structured* derivation — with no cross-version obligation, because P1 computed no
digest and Serea has minted no key.

**The accepted architecture/action-major treatment supersedes the earlier draft's
self-classification as "an architecture-minor clarification", withdrawn by the
P2 autonomous audit as wrong in a specific way.** SCJ-1 rule 6 is
not only a precision change to `‖`. It **narrows** the frozen Protocol Index §5
sentence "numbers in shortest round-trip form": a document containing a
fractional number is now non-canonicalisable, where before it was merely
underdetermined. A narrowing of a frozen rule is not a clarification. The
coherent plan — recorded once, in [the audit's M6](../plans/P2-autonomous-audit.md)
and [the decision ledger](../plans/P2-tomorrow-decision-ledger.md) — classifies
this ADR as **major on `serea.action/1`**, driving
`serea-arch/0.2.0 → 1.0.0` together with ADR-0018. This ADR defers to that plan
rather than asserting a different answer.

If the owner instead judges the frozen formula **normative** — that is, if `‖`
was meant to mean something other than raw concatenation — then §8.2 must be
amended in the same commit that implements IDK-1, and the collision above is the
evidence that the amendment is required rather than optional.

## Consequences

- Digests become computable for the explicit SCJ-1 domain. Noncanonical model
  documents and fractional capability arguments are not automatically covered.
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
| **Full JCS (RFC 8785) adoption** | **Conflicts with frozen text.** §3.2.3 sorts keys by UTF-16 code units; frozen Protocol Index §5 says UTF-8 code point. The orders differ for astral-plane keys, so full adoption would require amending §5 — and would make Serea digests incompatible with every other JCS implementation, which is a cost with no benefit when Serea has no external verifier |
| ECMAScript `Number::toString` for fractions, ahead of need | A fully specified and portable spelling exists, so this is not a correctness question — it is that SCJ-1 deliberately excludes fractions although model temperature is f64, and adopting it now adds a formatting dependency and ratifies a numeric range for nothing P2 can use. Deferred to P5 behind a named trigger, not rejected |
| Percent- or C0-escaping every preimage field | Equivalent to length prefixing with more moving parts and no correctness gain |
| `\|`-separated framing | `arguments` is arbitrary validated JSON and `ProviderReference` is opaque and never parsed, so a separator character can occur in a preimage |
| Newline framing | Same, plus a literal newline can occur in a JSON string argument |
| Fixed-width fields only | `capability_version` and the arguments document are variable-length |
| Last-wins duplicate keys | Parser differential on a value that decides whether an external effect is suppressed |
| Accepting a `Value` for convenience | It has already lost duplicate keys and cannot be checked for them |
