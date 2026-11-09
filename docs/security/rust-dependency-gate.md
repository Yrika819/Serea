# Rust dependency security gate

The `Rust Dependency Security` workflow uses RustSec `cargo-audit` against the
root workspace lockfile and each checked-in vendored or standalone lockfile:

- `Cargo.lock`
- `vendor/serde_json/Cargo.lock`
- `vendor/jsonschema-value/Cargo.lock`
- `vendor/jsonschema-value/regression-tests/Cargo.lock`

The scanner is pinned to cargo-audit 0.22.2 and runs under the current stable
Rust toolchain. Installation errors fail the workflow, the workflow checks the
installed version, and the scanner independently verifies the executable
version before scanning. The product build remains on its declared MSRV, Rust 1.89.0;
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

## MSRV required-check name migration

The Linux required check is renamed with the compiler from `Linux MSRV 1.85.0`
to `Linux MSRV 1.89.0`. Branch Protection and rulesets are intentionally not
changed by this repository change. Before merging the remediation, an Owner
should:

1. Confirm the Draft PR's `Linux MSRV 1.89.0` check passed on its current head.
2. In the active branch protection rule or ruleset, replace the required check
   `Linux MSRV 1.85.0` with `Linux MSRV 1.89.0`, preserving every other required
   check and any existing review or deployment requirement.
3. Save the rule and confirm GitHub reports the new `Linux MSRV 1.89.0` check
   as required and successful on the PR head before considering merge.

Do not merge while the old required name is still configured: the renamed
workflow no longer emits that status, so GitHub can keep the PR blocked. If the
new status is absent or not successful, leave the rule and PR unmerged and
investigate the workflow run. This procedure changes only the required-check
name; it does not relax branch protection.

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

`security/rust-audit-exceptions.json` is empty and exceptions are disabled. The
current `owner_approval` fields are plain repository data; non-empty names,
timestamps, or references do not prove that an Owner approved them. The scanner
therefore fails closed on any non-empty exception list until an external,
verifiable approval mechanism is implemented and independently reviewed. No
exception can waive missing evidence, scanner failures, or vulnerability
findings in the current gate.

Before scanning, the scanner verifies that `GITHUB_SHA` equals the checked-out
commit; checkout explicitly uses that SHA. Normal pull requests run the root
lockfile and any changed vendored/standalone lockfiles, plus vendor source
integrity checks. Main pushes, version and release-candidate tags, and manual
dispatches run the same complete scan
against the exact checked-out revision. The artifact is retained for 90 days.
