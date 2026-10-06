# Public-readiness audit

**Status: READY_FOR_PRIVATE_STAGING**

**Audit scope:** all locally reachable Git refs and the current working tree, including the post-P2 closure documentation commit. No history rewrite has been authorized.

**P3:** NOT STARTED.

This record is an audit log, not a license grant or legal opinion. Scanner scratch and metadata are ephemeral under ignored `target/public-readiness/` and `tmp/public-readiness/` paths; they are not publication artifacts.

## Git history secret audit

- Reachable history inventory: 17 commits, 651 reachable Git objects, 471 unique blobs across all local refs. `git remote -v` was empty at audit start.
- Specialized scanner: Gitleaks 8.30.1, full Git history with `--log-opts="--all"`; 17 commits and approximately 4.86 MB scanned. It reported six `generic-api-key` hits, all on `idempotency_key` example/test values in `docs/architecture/README.md`, `docs/architecture/04-execution-pipeline.md`, `docs/protocols/08-goallatch-adapter-protocol.md`, `crates/serea-protocol/tests/schema_contracts.rs`, and `crates/serea-protocol/tests/json_value_preservation.rs`. These are fixed synthetic protocol values, not credentials: **DOCUMENTATION_EXAMPLE** or **TEST_FIXTURE**.
- Independent scan: a custom Git-object scan examined every reachable blob for high-confidence GitHub/OpenAI/AWS/Bearer/PEM/OAuth/password/environment credential patterns. One `sk-`-shaped value is the deliberate `sk-proj-generation-private-marker` in `crates/serea-protocol/tests/p2a_types.rs`, used to prove error redaction: **TEST_FIXTURE**. A broad environment-assignment match in the vendored `serde_json/Cargo.toml.orig` is ordinary Cargo feature/configuration syntax: **FALSE_POSITIVE**.
- Classification: no `REAL_SECRET` identified. SHA-256 vectors, IDKs, ULIDs, protocol IDs, and synthetic test markers are not credentials.
- Gitleaks' separate current-directory scan timed out while traversing generated build output and was partial; it is not treated as a clean result. The independent reachable-blob scan includes all current committed source/docs; rerun a tracked-tree-only secret scan after the final readiness commit.

## Current-tree secret audit

Gitleaks 8.30.1 scanned the current repository tree copied to ignored scratch (292 files, approximately 4.62 MB) and reported six generic-key matches. They are the same fixed synthetic idempotency values classified above as documentation examples/test fixtures; the custom scanner additionally identifies the explicit OpenAI-shaped redaction marker as a test fixture. No real credential was identified. Scanner output remains ephemeral.

## Entropy / false-negative check

A separate heuristic sweep of tracked text surfaced 221 strings meeting a deliberately broad length/entropy threshold. Manual category review found protocol ULID/opaque-ID fixtures, SHA-256/Cargo checksums, the pinned GitHub Action commit, test names/URLs, and two public-protocol signature examples. The example pairing nonce in `docs/protocols/07-device-protocol.md` decodes to the literal synthetic marker `ordLnonceForThis DefinedDevice...`; it is a documentation example, not a live pairing credential. No unexplained high-entropy token was found in workflow configuration or source constants. SHA-256 vectors, ULIDs, test IDs, and canonical hash vectors were not classified as secrets based on entropy alone.

## Privacy audit

- Current-tree search initially found machine-specific home paths in `docs/plans/P0-closure.md`, `docs/plans/P2-autonomous-audit.md`, and `docs/threat-model/02-adversaries-and-attack-surface.md`. The working copies have been generalized to `$HOME` or repository-relative wording; no content from the separate local-MCP project was accessed.
- Historical reachable blobs still contain the former user-home root in those three documentation paths, including an explicit path to a separate local-MCP repository. These are historical privacy disclosures, not secrets. **OWNER_REVIEW_REQUIRED before Public**; no history rewrite has been performed or authorized.
- Current and historical source/documentation were searched for personal names, private emails, phone/address patterns, hostnames, LAN IPs, machine identifiers, account IDs, OAuth IDs, and repository-local absolute paths. The current-tree `$HOME` paths were generalized (**REMOVE_OR_GENERALIZE**, completed). `alice@example.test`, `bob@example.test`, and `mei.tanaka@example.com` are **SAFE_EXAMPLE** values. Upstream maintainer names/emails retained in vendored `jsonschema-value`/`serde_json` manifests and upstream contributing guidance are **REQUIRED_PUBLIC_TECHNICAL** provenance/contact information, not project-owner details. The sole project commit identity is the public-safe GitHub noreply identity below. No additional verified owner-private material was identified beyond the historical paths above.
- Do not expose credentials or private user data in issues or pull requests.

## Commit identity audit

`git log --all --format='%H %an <%ae>'` and the corresponding committer inventory show one identity across the 17 commits: author/committer `yrika <143304522+Yrika819@users.noreply.github.com>`. This is a GitHub noreply address and is classified **GitHub noreply / public-safe**. No private email identity or real full name was found.

## Commit-message privacy audit

Commit subjects and bodies are technical phase/change records. No real secret, personal correspondence, or personal email was identified. Historical audit documentation includes the machine-specific and separate-repository paths described in the privacy section; the path exposure is an owner-review item even when present in commit-message context.

## Binary/blob audit

- No reachable blob exceeds 5 MiB; a byte-level scan found no NUL-containing/binary blobs. No archive, DMG, APK/AAB, database, or accidental dump was identified in reachable history.
- No suspicious historical credential-file path (`.env`, private key/certificate/keystore, database, package, or credential-export path) was identified.
- Git object database reports approximately 16 MiB of loose objects and no packed objects. No Git LFS pointer or submodule is used by the current tree.

## Dependency license audit

`cargo metadata --format-version 1 --offline --locked` resolved the workspace lockfile and reported package/license metadata for the direct and transitive Rust graph. Direct third-party runtime packages include `async-trait 0.1.92` (MIT OR Apache-2.0), `jsonschema 0.58.3` (MIT), `rusqlite 0.40.2` (MIT), `serde 1.0.229` (MIT OR Apache-2.0), `serde_json 1.0.151` (MIT OR Apache-2.0), and `sha2 0.11.0` (MIT OR Apache-2.0). `libsqlite3-sys 0.38.2` (MIT) is transitive and builds bundled SQLite.

The full locked graph reports SPDX license expressions, predominantly MIT/Apache-2.0, with permissive BSD-2-Clause, Zlib, Unlicense, MIT-0, and Apache-2.0-only entries as applicable. Conditional alternatives include `r-efi 5.3.0` with LGPL-2.1-or-later OR Apache-2.0 OR MIT; the permissive alternatives are available. `unicode-ident 1.0.26` reports `(MIT OR Apache-2.0) AND Unicode-3.0`, so retain its Unicode notice when redistributing. No unknown third-party crate license or unconditional strong-copyleft-only dependency was found in Cargo metadata.

Cargo registry dependencies are fetched rather than vendored by this repository; Cargo.lock pins their versions. The `serde_json` and `jsonschema-value` copied source notices and license files are reviewed separately below. This is a metadata/provenance audit, not legal advice; license obligations for third-party dependencies remain applicable.

### Complete locked-package inventory

The table includes all 108 packages resolved by `Cargo.lock` at audit time, including workspace path packages (shown as `UNKNOWN` because the project license is undecided) and the two patched vendored path packages. License expressions are copied from `cargo metadata`; they are not a substitute for reviewing applicable license texts and notices.

| Package | Version | Cargo license expression |
|---|---:|---|
| `ahash` | `0.8.12` | `MIT OR Apache-2.0` |
| `aho-corasick` | `1.1.5` | `Unlicense OR MIT` |
| `allocator-api2` | `0.2.21` | `MIT OR Apache-2.0` |
| `async-trait` | `0.1.92` | `MIT OR Apache-2.0` |
| `autocfg` | `1.5.1` | `Apache-2.0 OR MIT` |
| `bit-set` | `0.8.0` | `Apache-2.0 OR MIT` |
| `bit-vec` | `0.8.0` | `Apache-2.0 OR MIT` |
| `bitflags` | `2.13.1` | `MIT OR Apache-2.0` |
| `block-buffer` | `0.12.1` | `MIT OR Apache-2.0` |
| `borrow-or-share` | `0.2.4` | `MIT-0` |
| `bumpalo` | `3.20.3` | `MIT OR Apache-2.0` |
| `bytecount` | `0.6.9` | `Apache-2.0/MIT` |
| `cc` | `1.6.0` | `MIT OR Apache-2.0` |
| `cfg-if` | `1.0.5` | `MIT OR Apache-2.0` |
| `cpufeatures` | `0.3.1` | `MIT OR Apache-2.0` |
| `crypto-common` | `0.2.2` | `MIT OR Apache-2.0` |
| `data-encoding` | `2.11.1` | `MIT` |
| `digest` | `0.11.3` | `MIT OR Apache-2.0` |
| `email_address` | `0.2.9` | `MIT` |
| `equivalent` | `1.0.2` | `Apache-2.0 OR MIT` |
| `fallible-iterator` | `0.3.0` | `MIT/Apache-2.0` |
| `fallible-streaming-iterator` | `0.1.9` | `MIT/Apache-2.0` |
| `fancy-regex` | `0.19.2` | `MIT` |
| `find-msvc-tools` | `0.1.14` | `MIT OR Apache-2.0` |
| `fluent-uri` | `0.4.1` | `MIT` |
| `foldhash` | `0.2.0` | `Zlib` |
| `fraction` | `0.17.0` | `MIT OR Apache-2.0` |
| `getrandom` | `0.3.4` | `MIT OR Apache-2.0` |
| `hashbrown` | `0.17.1` | `MIT OR Apache-2.0` |
| `heck` | `0.5.0` | `MIT OR Apache-2.0` |
| `hybrid-array` | `0.4.15` | `MIT OR Apache-2.0` |
| `indexmap` | `2.14.2` | `Apache-2.0 OR MIT` |
| `itoa` | `1.0.18` | `MIT OR Apache-2.0` |
| `js-sys` | `0.3.103` | `MIT OR Apache-2.0` |
| `jsonschema` | `0.58.3` | `MIT` |
| `jsonschema-macros` | `0.58.3` | `MIT` |
| `jsonschema-macros-core` | `0.58.3` | `MIT` |
| `jsonschema-regex` | `0.58.3` | `MIT` |
| `jsonschema-value` | `0.58.3` | `MIT` |
| `libc` | `0.2.189` | `MIT OR Apache-2.0` |
| `libsqlite3-sys` | `0.38.2` | `MIT` |
| `lock_api` | `0.4.14` | `MIT OR Apache-2.0` |
| `memchr` | `2.8.3` | `Unlicense OR MIT` |
| `micromap` | `0.3.0` | `MIT` |
| `num` | `0.4.3` | `MIT OR Apache-2.0` |
| `num-bigint` | `0.4.8` | `MIT OR Apache-2.0` |
| `num-cmp` | `0.1.0` | `MIT/Apache-2.0` |
| `num-complex` | `0.4.6` | `MIT OR Apache-2.0` |
| `num-integer` | `0.1.47` | `MIT OR Apache-2.0` |
| `num-iter` | `0.1.46` | `MIT OR Apache-2.0` |
| `num-rational` | `0.4.2` | `MIT OR Apache-2.0` |
| `num-traits` | `0.2.19` | `MIT OR Apache-2.0` |
| `once_cell` | `1.21.4` | `MIT OR Apache-2.0` |
| `outref` | `0.5.2` | `MIT` |
| `parking_lot` | `0.12.5` | `MIT OR Apache-2.0` |
| `parking_lot_core` | `0.9.12` | `MIT OR Apache-2.0` |
| `percent-encoding` | `2.3.2` | `MIT OR Apache-2.0` |
| `pkg-config` | `0.3.34` | `MIT OR Apache-2.0` |
| `proc-macro-crate` | `3.5.0` | `MIT OR Apache-2.0` |
| `proc-macro2` | `1.0.107` | `MIT OR Apache-2.0` |
| `quote` | `1.0.47` | `MIT OR Apache-2.0` |
| `r-efi` | `5.3.0` | `MIT OR Apache-2.0 OR LGPL-2.1-or-later` |
| `redox_syscall` | `0.5.18` | `MIT` |
| `ref-cast` | `1.0.27` | `MIT OR Apache-2.0` |
| `ref-cast-impl` | `1.0.27` | `MIT OR Apache-2.0` |
| `referencing` | `0.58.3` | `MIT` |
| `regex` | `1.13.1` | `MIT OR Apache-2.0` |
| `regex-automata` | `0.4.18` | `MIT OR Apache-2.0` |
| `regex-syntax` | `0.8.11` | `MIT OR Apache-2.0` |
| `rusqlite` | `0.40.2` | `MIT` |
| `rustversion` | `1.0.23` | `MIT OR Apache-2.0` |
| `scopeguard` | `1.2.0` | `MIT OR Apache-2.0` |
| `serde` | `1.0.229` | `MIT OR Apache-2.0` |
| `serde_core` | `1.0.229` | `MIT OR Apache-2.0` |
| `serde_derive` | `1.0.229` | `MIT OR Apache-2.0` |
| `serde_json` | `1.0.151` | `MIT OR Apache-2.0` |
| `serea-protocol` | `0.1.0` | `UNKNOWN` |
| `serea-storage` | `0.1.0` | `UNKNOWN` |
| `serea-task-engine` | `0.1.0` | `UNKNOWN` |
| `serea-testkit` | `0.1.0` | `UNKNOWN` |
| `sha2` | `0.11.0` | `MIT OR Apache-2.0` |
| `shlex` | `2.0.1` | `MIT OR Apache-2.0` |
| `smallvec` | `1.16.2` | `MIT OR Apache-2.0` |
| `strum` | `0.28.0` | `MIT` |
| `strum_macros` | `0.28.0` | `MIT` |
| `syn` | `2.0.119` | `MIT OR Apache-2.0` |
| `syn` | `3.0.6` | `MIT OR Apache-2.0` |
| `toml_datetime` | `1.1.1+spec-1.1.0` | `MIT OR Apache-2.0` |
| `toml_edit` | `0.25.15+spec-1.1.0` | `MIT OR Apache-2.0` |
| `toml_parser` | `1.1.3+spec-1.1.0` | `MIT OR Apache-2.0` |
| `typenum` | `1.20.1` | `MIT OR Apache-2.0` |
| `unicode-general-category` | `1.1.0` | `Apache-2.0` |
| `unicode-ident` | `1.0.26` | `(MIT OR Apache-2.0) AND Unicode-3.0` |
| `uuid-simd` | `0.8.0` | `MIT` |
| `vcpkg` | `0.2.15` | `MIT/Apache-2.0` |
| `version_check` | `0.9.5` | `MIT/Apache-2.0` |
| `vsimd` | `0.8.0` | `MIT` |
| `wasip2` | `1.0.4+wasi-0.2.12` | `Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT` |
| `wasm-bindgen` | `0.2.126` | `MIT OR Apache-2.0` |
| `wasm-bindgen-macro` | `0.2.126` | `MIT OR Apache-2.0` |
| `wasm-bindgen-macro-support` | `0.2.126` | `MIT OR Apache-2.0` |
| `wasm-bindgen-shared` | `0.2.126` | `MIT OR Apache-2.0` |
| `windows-link` | `0.2.1` | `MIT OR Apache-2.0` |
| `winnow` | `1.0.4` | `MIT` |
| `wit-bindgen` | `0.57.1` | `Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT` |
| `zerocopy` | `0.8.59` | `BSD-2-Clause OR Apache-2.0 OR MIT` |
| `zerocopy-derive` | `0.8.59` | `BSD-2-Clause OR Apache-2.0 OR MIT` |
| `zmij` | `1.0.23` | `MIT` |

## Vendored source audit

- `vendor/jsonschema-value`: package `jsonschema-value 0.58.3`, upstream repository `Stranger6667/jsonschema`, VCS commit `a171a01f52f20142bc4f1cfb07c6e56e195e8da6`, crates.io archive SHA-256 `43519639b8e32075ef90a9b6b19136e516a3236b7943c3fe6b1622dafdced8ea`. `README.vendor.md` records the source, exact modified files and patch rationale. The vendored `LICENSE` is MIT and retained; upstream archive omitted the license file and the documented license text provenance is the cached sibling release. No unrelated private files or credentials were identified.
- `vendor/serde_json`: package `serde_json 1.0.151`, upstream VCS commit `de8500740cdcabffb9734f503e4889def823cf10`, crates.io archive SHA-256 `c841b55ecdae098c80dcae9cf767f6f8a0c2cdb3416bbef72181df4d0fe73f14`. `README.vendor.md` records provenance and the three-file patch scope. Original MIT and Apache-2.0 license texts are retained. No unrelated private files or credentials were identified.
- No vendor contents were downloaded or replaced for this audit.

## Project license

**BLOCKER — OWNER LICENSE DECISION REQUIRED.** No root `LICENSE`, `LICENSE-MIT`, `LICENSE-APACHE`, or `COPYING` exists. No license has been selected or added. Owner options compatible with the locked dependency expressions include `MIT OR Apache-2.0` (dual license, giving downstream recipients a choice) or MIT (simple permissive terms); retain applicable third-party notices, including the Unicode-3.0 notice. The owner must choose. Do not publish until the exact project license is approved and added.

## Project-name collision

- GitHub repository search for `Serea` returned multiple exact-name repositories, largely small or unrelated projects, and the established `Sereal/Sereal` serialization project; the Rust-language GitHub query returned no direct Serea software match.
- crates.io API search for `serea` returned no crates.
- General web search through Google was blocked by its interstitial, so general-web coverage is incomplete.
- Assessment: **REVIEW_REQUIRED**, not a formal trademark opinion. `Serea` and the established software name `Sereal` are visually/phonologically close; assess before Public. No automatic rename.

## README

`README.md` now describes Serea as an offline-first Rust task-runtime foundation, identifies the four current crates and architecture, says P2 durable runtime is complete and P3 not started, lists build/test commands, and explicitly says planned Gmail, Calendar, Android, Event Bus, Scheduler, GoalLatch, Codex, credentials, and providers are not implemented.

## SECURITY

`SECURITY.md` describes the supported development status, warns against production use, advises against credentials in public issues, and directs private reports to GitHub Private Vulnerability Reporting if enabled. It does not invent a contact email; no dedicated security email exists.

## CONTRIBUTING

`CONTRIBUTING.md` covers Rust stable/MSRV 1.85.0, fmt/check/tests/Clippy/docs/smoke, dependency fetch before offline validation, no credentials, deterministic offline tests, TDD for behavior changes, and frozen phase boundaries.

## GitHub Actions threat model

- Workflows are `.github/workflows/ci.yml` and `.github/workflows/full-ci.yml`.
- Both use `pull_request` safely (no `pull_request_target`), workflow-level `permissions: contents: read`, no job-level write permissions, no `workflow_run`, no secrets, no untrusted metadata interpolated into shell, and no self-hosted runners.
- Checkout sets `persist-credentials: false`; PR jobs execute no deployment or privileged operation.
- No `actions/cache` or `target/` cache is used. Runners are disposable.
- Every job has a finite timeout. Concurrency cancels superseded runs. Full CI uses `fail-fast: false` nowhere because there is no matrix; jobs are independent and report separately.
- No private-repository Actions run has been started. Actions should remain disabled during private staging.

## Action SHA provenance

Only `actions/checkout` is used. Official `actions/checkout` GitHub API tag reference `v7.0.1` resolves directly to commit `3d3c42e5aac5ba805825da76410c181273ba90b1`; the official latest-release endpoint identifies `v7.0.1`. Workflow refs pin that immutable 40-character commit with `# v7.0.1`. No other external Action is used.

## Runner labels

Verified 2026-10-06 against GitHub's official runner reference: <https://docs.github.com/en/actions/reference/runners/github-hosted-runners>.

- Linux x86_64: `ubuntu-latest` (official table lists x64).
- macOS Intel x86_64: `macos-15-intel` (official table explicitly lists Intel architecture).
- macOS Apple Silicon arm64: `macos-15` (official table lists arm64).

Workflows print and assert architecture in the full jobs. The runner labels describe the hosted images, not all hardware sharing those architectures.

## CI plan

- Fast PR and `main` push: Linux stable, locked dependency fetch, smoke guard/tests, docs validator, fmt, metadata, and all-target/all-feature check. No full portability matrix.
- Full validation: `main` push or manual `workflow_dispatch`, no initial schedule. Linux stable performs check, all-target tests, all-feature tests, Clippy, Group O, docs/smoke/fmt/metadata, and release fault-exclusion proof. Linux MSRV installs exact Rust 1.85.0 and runs check/tests/Clippy. Current official Intel and arm64 macOS jobs print/assert architecture and run workspace check, all-feature tests, and P2H crash suite.
- No local full Rust/MSRV/portability matrix is rerun for these docs/workflow-only readiness changes. Remote CI is intentionally deferred until the repository is Public and Actions are explicitly enabled.

## Known nonclaims

- No Gmail, Calendar, Android, GoalLatch, Local MCP, real provider, model, or credential integration is shipped.
- P3 remains NOT STARTED.
- Local and planned CI do not prove power-loss durability, universal device behavior, production certification, or all Apple Silicon devices.
- The release fault proof's scope is its named A/B/C/D builds; it does not claim that an arbitrary all-features release artifact is seam-free.
- No CODE_OF_CONDUCT is added; it is OPTIONAL / NOT ADDED.

## Pre-remote readiness status

**READY_FOR_PRIVATE_STAGING.** Repository integrity and local Git access are sound; there is no current remote. `gh` 2.101.0 is authenticated; `gh auth status` displays the old login `yutaGOTO819`, while authenticated `gh api user` returns `Yrika819` (user ID `143304522`), matching the numeric ID in the commit noreply identity. The authenticated API identity is treated as the current owner; verify exact repo ownership/collision before creation. No REAL_SECRET was found in reachable history or the current repository tree. Both workflows passed static YAML/shell/security checks, and the required lightweight local checks passed. This status permits private staging only; it does not authorize Public visibility.

## Blockers

1. **Project LICENSE:** owner choice and exact license file required before Public.
2. **Historical privacy:** owner review/explicit acceptance required for user-home and separate local-MCP paths in three historical blobs, or separate explicit authorization for history rewrite. History has not been rewritten.
3. **Name collision:** review required because of exact-name projects and the established `Sereal` software identity; general-web query was blocked.
4. **Private staging not yet verified:** exact `Yrika819/Serea` collision check, Actions disablement before push, private push, and remote verification remain pending.

Do not switch Public while any mandatory category is UNKNOWN or unresolved.
