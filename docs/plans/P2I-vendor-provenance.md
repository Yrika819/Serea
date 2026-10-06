# P2I JSONSchema-Value Provenance Disposition

**Review-only provenance check.** No vendor file, Cargo manifest, lockfile or dependency-resolution state was changed by this check.

## Package identity and archive

- Root `Cargo.lock` contains `jsonschema-value` version `0.58.3` as the package selected through the local path patch; a path package has no registry `source` or lockfile checksum entry.
- `vendor/jsonschema-value/Cargo.toml` and `Cargo.toml.orig` both identify `jsonschema-value` `0.58.3`, MIT, repository `Stranger6667/jsonschema`.
- The vendored `.cargo_vcs_info.json` identifies upstream commit `a171a01f52f20142bc4f1cfb07c6e56e195e8da6` at `crates/jsonschema-value`.
- `README.vendor.md` records crates.io archive SHA-256 `43519639b8e32075ef90a9b6b19136e516a3236b7943c3fe6b1622dafdced8ea` and says the archive was copied from local cache. Local cache was absent in this run.
- The official crates.io API for `jsonschema-value/0.58.3` reported version `0.58.3`, license `MIT`, archive size `92062`, Rust version `1.85.0`, and checksum `43519639b8e32075ef90a9b6b19136e516a3236b7943c3fe6b1622dafdced8ea`.
- Downloaded the official archive solely for this review to `target/p2i-provenance/jsonschema-value-0.58.3.crate`; `shasum -a 256` returned the exact same digest. Extracted under that ignored target directory. Neither Cargo nor vendor resolution was changed.

## Archive-to-vendor comparison

Recursive `diff -ru target/p2i-provenance/jsonschema-value-0.58.3 vendor/jsonschema-value` found only:

| Difference | Classification | Disposition |
|---|---|---|
| `Cargo.toml` adds the `serea_numeric` test target | DOCUMENTED_PATCH | Added regression target, documented in README.vendor.md. |
| `src/numeric.rs` checked decimal-shift arithmetic, unsigned absolute conversion, denominator cap accounting and exact integer classification helper | DOCUMENTED_PATCH | Narrow P2A numeric safety/schema-integer patch, documented in README.vendor.md; not a dependency-name/version substitution. |
| `src/types.rs` delegates arbitrary-precision integer classification to exact decimal helper | DOCUMENTED_PATCH | Same narrow patch. |
| `tests/serea_numeric.rs` | DOCUMENTED_PATCH | Focused helper regressions for extreme shifts, exact values, denominator cap and integer classification. |
| `LICENSE`, `README.vendor.md`, `regression-tests/` (including its own manifest/lockfile) | PROVENANCE_METADATA | Recorded license/provenance and offline helper harness. |

No unexpected production-source difference appeared. The package's normalized manifest and original manifest keep package identity, version, repository and MIT license. No license file was present in the published value-crate archive; the vendored note identifies the sibling jsonschema 0.58.3 MIT license source. The P2-required patch behavior is additionally exercised by protocol schema and numeric tests; this is not a complete upstream test-suite or transitive vulnerability audit.

**A12 disposition: PASS.** Exact release identity and recorded checksum match the official registry checksum; archive SHA-256 matches; patch scope is limited to the documented code/tests/provenance files. Residual supply-chain nonclaim: no complete third-party dependency vulnerability certification is asserted.
