# Serea's narrow serde_json patch

## Provenance

This directory retains the published `serde_json` **1.0.151** crate from
crates.io, obtained from the local Cargo registry cache. It is licensed under
MIT OR Apache-2.0; both original license files are retained.

- Published archive SHA-256:
  `c841b55ecdae098c80dcae9cf767f6f8a0c2cdb3416bbef72181df4d0fe73f14`.
- Upstream Git revision recorded in the original `.cargo_vcs_info.json`:
  `de8500740cdcabffb9734f503e4889def823cf10`.
- The normalized `Cargo.toml`, `Cargo.toml.orig`, upstream `Cargo.lock`, build
  script, original README, licenses, and every published test/fixture are
  retained unchanged. Cargo's local `.cargo-ok` extraction marker is omitted;
  it is not part of the published crate.
- Only `src/value/de.rs`, `src/number.rs`, and `src/raw.rs` differ from published
  upstream source. This note is the only additional file.

## Reason and exact changes

Frozen P2A N3 (Major, review `47586732`, closure §4.6) concerns ordinary JSON
objects whose first key is `$serde_json::private::Number` or
`$serde_json::private::RawValue`. With `arbitrary_precision`/`raw_value` enabled,
upstream's `Value` visitor interprets those literal keys as synthetic Serde
transport, replacing the object or rejecting otherwise valid JSON. This
violates opaque payload/extension retention.

The patch preserves both required features and public `Value`/`Map` APIs:

1. `value/de.rs`: the key classifier requests `deserialize_any`; ordinary
   string keys always stay literal. Only a newtype-wrapped marker key selects
   internal Number/RawValue transport.
2. `number.rs` and `raw.rs`: the internal synthetic-key deserializers emit a
   newtype-wrapped marker for `deserialize_any`, while retaining explicit
   string/identifier decoding. Marker-key consumers also accept the newtype
   after buffering. Serde derive's `ContentVisitor` retains it as
   `Content::Newtype`, and `ContentDeserializer` replays it, so flattened
   extension fields do not erase the distinction.
3. `number.rs`: arbitrary-precision `Number::deserialize_any` replays native
   i64/u64 values directly and every other number through its exact synthetic
   map. Eager u128/i128 events are unsupported by Serde's flatten buffer, and
   an f64 fallback can respell a precise number. Explicit typed numeric
   deserialization methods are unchanged.

Literal JSON and explicit `Value::Object` keys cannot manufacture that Serde
newtype signal. No keys are reserved or banned, and no numeric precision
feature is disabled. This is a transport discriminator, not an authorization
mechanism for arbitrary custom deserializers.

## Scope and compatibility limits

This is not a general Serde compatibility, numeric arithmetic, schema, or
resource-budget rewrite. The raw numeric parser, serializer, recursion limits,
and protocol types/canonicalization are unchanged. Serea's separate
`jsonschema-value` patch and exact generation decoder remain required.

The patch deliberately changes the private synthetic-key representation and
`Number::deserialize_any` replay events. Third-party deserializers/visitors
that depend on upstream's old private marker maps or eager u128/i128/f64
`deserialize_any` events may need adaptation; no compatibility guarantee for
such consumers or other serialization formats is made. Typed u128/i128/f64
requests retain their original decoding paths.

Regression coverage lives in
`crates/serea-protocol/tests/json_value_preservation.rs`: literal marker maps,
escaped keys, invalid marker values, nesting, raw string/slice/reader inputs,
owned/borrowed Value replay, actual action arguments and Trace extensions,
two derive-flatten layers, genuine precise numbers/RawValue transport,
explicit numeric targets, and exact schema classification. Original upstream
tests remain intact; retaining them is not a claim they have all been run.

The workspace pins `serde_json = "=1.0.151"` and patches crates.io to this
path. The standalone jsonschema-value regression workspace has its own path
patch because it does not inherit the root workspace's patches.

## Upgrade and removal

Do not remove this patch merely because a dependency version changes or an
unrelated test suite passes. First verify that an upstream release preserves
literal marker objects on raw and owned/borrowed Value paths, retains the
synthetic discriminator through derive flatten, and preserves exact numeric
replay, generation/schema parity, and canonical/error tests on stable and the
workspace MSRV.

Once an upstream release provides those guarantees, update the exact pin,
remove the root and standalone-harness serde_json path patches and the vendor
workspace exclusion, update their lockfiles offline where possible, then
remove this directory. Retain the protocol regression tests. If upgrading
before an upstream fix, rebase only these three source changes against the
published crate, preserve its manifest/licenses/tests, and refresh provenance
and this note.
