# Serea's narrow jsonschema-value patch

## Provenance and license

This directory preserves all 27 files from the published `jsonschema-value`
0.58.3 crate, including its original README, manifests, lockfile, tests and
fixtures. Only the normalized Cargo.toml, src/numeric.rs and src/types.rs are
modified; Serea adds this note, LICENSE, tests/serea_numeric.rs and the
regression-tests harness.

- Registry archive: `jsonschema-value-0.58.3.crate` from crates.io, copied from
  the local Cargo cache without a network request.
- Archive SHA-256:
  `43519639b8e32075ef90a9b6b19136e516a3236b7943c3fe6b1622dafdced8ea`.
- Upstream repository: https://github.com/Stranger6667/jsonschema
- Published VCS commit: `a171a01f52f20142bc4f1cfb07c6e56e195e8da6`;
  repository path: `crates/jsonschema-value` (see .cargo_vcs_info.json).
- License: MIT. The value crate archive omits the license text; LICENSE is
  copied unchanged from the cached sibling `jsonschema` 0.58.3 release,
  copyright 2020-2026 Dmitry Dygalo.
- Original unpacked payload: 388,442 bytes; archive: 92,062 bytes.

## Exact patch rationale

P2A N1's direct schema validation reaches unchecked decimal normalization:
`1.0e-9223372036854775807` makes the decimal shift i64::MIN, whose negation
panics. `1.00e-9223372036854775807` underflows the preceding subtraction.
`1e-9223372036854775808` instead exceeds the existing exponent parser's
positive i64 magnitude and is declined by that conversion parser.

1. DecimalComponents::decimal_shift returns Option<i64>, with checked length
   conversion and subtraction. Both conversion helpers propagate refusal.
2. Negative shifts use unsigned_abs and checked usize conversion. BigInt's
   zero branch precedes the negative magnitude conversion; positive append
   counts are also checked.
3. BigFraction checks the actual denominator power against the existing
   1,000,000-place cap, not just the raw exponent, before constructing a power.
4. Under arbitrary-precision, integer type classification never falls back to
   rounded f64. For a nonzero coefficient, membership is exactly
   `exponent + trailing_zero_count >= fractional_digit_count`. Zero, including
   signed zero, is integral for any valid exponent. Only the exponent is
   parsed into the already-enabled BigInt; no power or expanded coefficient is
   materialized. The input is a validated serde_json::Number token.

The root workspace excludes this directory and pins jsonschema to =0.58.3.
This numeric patch changes only jsonschema-value; the companion serde_json
transport patch (see ../serde_json/README.vendor.md) preserves literal objects
under the required precision/raw features. Both local crates are excluded from
workspace membership. Network/file resolution remains disabled. No dependency or MSRV increase is required:
this crate declares Rust 1.85.0, and the added APIs are available there.

## Limited guarantees

This patch addresses exact integer classification and conversion overflow /
denominator allocation protection needed by Serea's existing bounded wire
integers. It preserves direct jsonschema::Validator use and the published tests.
It does not implement unrestricted exact schema arithmetic: equality, bounds
and multipleOf still have upstream f64 fallbacks and conversion caps. It does
not change Draft 4's lexical integer semantics, non-arbitrary-precision behavior,
or add a general numeric resource budget. Do not infer correctness for arbitrary
fractional schemas, const/enum/uniqueItems or large numeric bounds from this patch.

## Offline regression checks

The original upstream test dependencies (hegeltest and test-case) were not
cached when this patch was made. The small independent regression-tests
manifest runs the new helper tests using only jsonschema-value and the same
pinned, patched serde_json as the root workspace;
it does not replace, delete or disable the published test suites. The same
new test target is also registered in the vendored crate's normalized manifest
for normal upstream-style testing when those dev dependencies are available.

Run from the Serea root, first without --locked when establishing the two
lockfiles, then with --locked. Each invocation should have a 300-second limit:

```sh
cargo +stable test --offline --locked -p serea-protocol --test p2a_parity
cargo +stable test --offline --locked --release -p serea-protocol --test p2a_parity
cargo +1.85.0 test --offline --locked -p serea-protocol --test p2a_parity
cargo +1.85.0 test --offline --locked --release -p serea-protocol --test p2a_parity
cargo +stable test --offline --locked --manifest-path vendor/jsonschema-value/regression-tests/Cargo.toml --target-dir target/vendor-regressions
cargo +stable test --offline --locked --release --manifest-path vendor/jsonschema-value/regression-tests/Cargo.toml --target-dir target/vendor-regressions
cargo +1.85.0 test --offline --locked --manifest-path vendor/jsonschema-value/regression-tests/Cargo.toml --target-dir target/vendor-regressions
cargo +1.85.0 test --offline --locked --release --manifest-path vendor/jsonschema-value/regression-tests/Cargo.toml --target-dir target/vendor-regressions
```

The published Cargo.lock remains unchanged; the harness has its own lockfile.
Offline builds require the already-resolved dependency cache: only this one
upstream crate is vendored, not the entire graph.

## Removal conditions

Remove the path patch, exact version pin and vendor directory only after an
upstream release passes both the unchanged P2A parity tests and the added direct
integer/helper regressions on stable and Rust 1.85 in debug and release. Check
subtraction overflow, minimum signed shifts, arbitrary signed exponent lengths,
signed zero and the actual denominator cap, not merely absence of a panic.
Confirm direct validator / error iteration behavior, bounded wire membership,
feature settings and offline availability before updating the root lockfile.
Preserve regression coverage when retiring the local helper test harness.
