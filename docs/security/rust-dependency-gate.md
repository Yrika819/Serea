# Rust dependency security gate

The `Rust Dependency Security` workflow uses RustSec `cargo-audit` against the
root workspace lockfile and each checked-in vendored or standalone lockfile:

- `Cargo.lock`
- `vendor/serde_json/Cargo.lock`
- `vendor/jsonschema-value/Cargo.lock`
- `vendor/jsonschema-value/regression-tests/Cargo.lock`

The scanner is pinned to cargo-audit 0.22.2 and runs under the current stable
Rust toolchain. The product build remains on its declared MSRV, Rust 1.85.0;
the scanner does not compile or alter the product. CI has `contents: read`, no
secrets, and checkout credentials are disabled.

Each scan writes `security-results/rust-security.json` with the exact commit
and tree, scanner version, RustSec database commit and timestamp, lockfile
results, severity counts, errors, and source digests for every vendored Rust
file. The workflow uploads this file even after a scan failure. Missing or
unparseable results, inability to identify the advisory database revision,
scanner execution errors, and unexcepted advisory findings have distinct
statuses and fail the check. Any unexcepted advisory is a release blocker;
CVSS 7.0+ findings are P0/P1-equivalent and block release without exception.

## Vendored code scope

`cargo-audit` matches advisory records to package versions and cannot establish
that locally modified Rust code is safe. In addition to scanning the lockfiles,
the evidence artifact inventories every `.rs` file under each vendored `src/`
tree and checks the pinned upstream VCS revision and package version against
the vendor provenance notes. The accepted local Rust changes are constrained
to:

- `serde_json` 1.0.151: `src/value/de.rs`, `src/number.rs`, `src/raw.rs`.
- `jsonschema-value` 0.58.3: `src/numeric.rs`, `src/types.rs`.

The provenance notes in the vendor directories describe why these changes
exist and their limitations. A change to a different vendored Rust file causes
the scanner to fail until this scope and its review are explicitly updated.
This source inventory is integrity evidence, not a claim that cargo-audit
performs semantic source analysis.

## Exceptions and release policy

Exceptions live in `security/rust-audit-exceptions.json`. Each item must name
the RustSec advisory ID and affected package, give a specific rationale, set
an ISO date expiry, and include verifiable Owner approval (`approved_by`,
`approved_at`, `approval_ref`). The gate rejects malformed, expired, or
unapproved exceptions. Exceptions only apply to that exact advisory/package
pair; they cannot waive missing evidence or scanner failures. There are no
standing or permanent allowances. Reassess and remove the entry before its
expiry; an extension requires a new explicit Owner approval and expiry.

Normal pull requests run the lockfile and vendor checks. Main pushes, version
and release-candidate tags, and manual dispatches run the same complete scan
against the exact checked-out revision. The artifact is retained for 90 days.
