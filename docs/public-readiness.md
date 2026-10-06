# Public-readiness audit

**Status: PUBLIC_AND_CI_VALIDATED**

**Audit scope:** all locally reachable Git refs and the current working tree. The owner explicitly authorized a pre-public history rewrite to remove machine-specific home paths; that rewrite has completed locally and all phase refs were rewritten consistently.

**P3:** NOT STARTED.

This record is an audit log, not a license grant or legal opinion. Scanner scratch and metadata are ephemeral under ignored `target/public-readiness/` and `tmp/public-readiness/` paths; they are not publication artifacts.

## Git history secret audit

- Reachable history inventory at initial scan: 18 commits, approximately 480 unique blobs across reachable refs. After private staging, `main` preserves the complete history and adds no rewritten or squashed commits.
- Specialized scanner: Gitleaks 8.30.1, full Git history with `--log-opts="--all"`; 18 commits and approximately 4.89 MB scanned. It reported six `generic-api-key` hits, all on `idempotency_key` example/test values in `docs/architecture/README.md`, `docs/architecture/04-execution-pipeline.md`, `docs/protocols/08-goallatch-adapter-protocol.md`, `crates/serea-protocol/tests/schema_contracts.rs`, and `crates/serea-protocol/tests/json_value_preservation.rs`. These are fixed synthetic protocol values, not credentials: **DOCUMENTATION_EXAMPLE** or **TEST_FIXTURE**.
- Independent scan: a custom Git-object scan examined every reachable blob for high-confidence GitHub/OpenAI/AWS/Bearer/PEM/OAuth/password/environment credential patterns. One `sk-`-shaped value is the deliberate `sk-proj-generation-private-marker` in `crates/serea-protocol/tests/p2a_types.rs`, used to prove error redaction: **TEST_FIXTURE**. A broad environment-assignment match in the vendored `serde_json/Cargo.toml.orig` is ordinary Cargo feature/configuration syntax: **FALSE_POSITIVE**.
- Classification: no `REAL_SECRET` identified. SHA-256 vectors, IDKs, ULIDs, protocol IDs, and synthetic test markers are not credentials.
- Gitleaks' separate current-directory scan timed out while traversing generated build output and was partial; it is not treated as a clean result. A tracked-tree-only scan was completed from ignored scratch against 292 files after readiness changes; scanner output remains ephemeral.

## Current-tree secret audit

Gitleaks 8.30.1 scanned the current repository tree copied to ignored scratch (292 files, approximately 4.62 MB) and reported six generic-key matches. They are the same fixed synthetic idempotency values classified above as documentation examples/test fixtures; the custom scanner additionally identifies the explicit OpenAI-shaped redaction marker as a test fixture. No real credential was identified. Scanner output remains ephemeral.

## Entropy / false-negative check

A separate heuristic sweep of tracked text surfaced 221 strings meeting a deliberately broad length/entropy threshold. Manual category review found protocol ULID/opaque-ID fixtures, SHA-256/Cargo checksums, the pinned GitHub Action commit, test names/URLs, and two public-protocol signature examples. The example pairing nonce in `docs/protocols/07-device-protocol.md` decodes to the literal synthetic marker `ordLnonceForThis DefinedDevice...`; it is a documentation example, not a live pairing credential. No unexplained high-entropy token was found in workflow configuration or source constants. SHA-256 vectors, ULIDs, test IDs, and canonical hash vectors were not classified as secrets based on entropy alone.

## Privacy audit

- The owner explicitly authorized removal of the historical machine-specific paths before publication.
- The complete reachable local history was rewritten with `git-filter-repo` 2.47.0 using exact literal replacements for the former Serea checkout path and separate local-MCP path.
- The current tree, every reachable commit, and commit messages were rescanned afterward; no former machine-specific home, Desktop checkout, or local-MCP path remains.
- A complete pre-rewrite Git bundle was kept only as a local recovery artifact outside the repository. It is not tracked and must never be pushed or published.
- The rewrite changed commit object IDs. The generated commit map was used to update tracked current-document references from old private-history SHAs to the rewritten public-history SHAs; no old full SHA remains in the tracked current tree.
- Current and rewritten historical source/documentation were searched for personal names, private emails, phone/address patterns, hostnames, LAN IPs, machine identifiers, account IDs, OAuth IDs, and repository-local absolute paths. `alice@example.test`, `bob@example.test`, and `mei.tanaka@example.com` are **SAFE_EXAMPLE** values. Upstream maintainer names/emails retained in vendored manifests and upstream contributing guidance are **REQUIRED_PUBLIC_TECHNICAL** provenance/contact information.
- The sole project commit identity remains the public-safe GitHub noreply identity documented below.
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

**PASS — owner selected MIT.** Serea is licensed under the root `LICENSE` file with `Copyright (c) 2026 Yrika819`. Workspace package metadata declares `license = "MIT"` and all four workspace crates inherit it through `license.workspace = true`. README links to the MIT license. Applicable third-party and vendored licenses remain in force and are not relicensed by the Serea project license.

## Project-name collision

The owner reviewed the known exact-name and near-name collisions and explicitly accepts them for this OSS publication. Serea remains the project name. The repository identity `Yrika819/Serea` is unambiguous; no legal trademark opinion or claim of exclusive naming rights is made. **Disposition: OWNER_ACCEPTED.**

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
- No private-repository Actions run was started. Actions were enabled only after the repository was verified Public; all initial hosted CI below ran on the Public repository.

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
- No local full Rust/MSRV/portability matrix was rerun for these docs/workflow-only readiness changes. The remote Public CI bootstrap described below now provides the Linux/MSRV/macOS hosted evidence.

## Known nonclaims

- No Gmail, Calendar, Android, GoalLatch, Local MCP, real provider, model, or credential integration is shipped.
- P3 remains NOT STARTED.
- Local and planned CI do not prove power-loss durability, universal device behavior, production certification, or all Apple Silicon devices.
- The release fault proof's scope is its named A/B/C/D builds; it does not claim that an arbitrary all-features release artifact is seam-free.
- No CODE_OF_CONDUCT is added; it is OPTIONAL / NOT ADDED.

## Private staging record

**SANITIZED_PRIVATE_STAGING_VERIFIED before publication.** The pre-public history was replaced using a guarded `force-with-lease` update whose expected remote SHA matched exactly. At that checkpoint the repository remained Private, remote `main` contained the sanitized rewritten history and MIT license, GitHub Actions were disabled, and the workflow-runs endpoint reported zero runs. No private hosted CI was executed.

## Final publication gate

All mandatory categories P1–P25 are **PASS** or **OWNER_ACCEPTED**:

- full-history and current-tree secret audits: PASS; Gitleaks' six findings are fixed synthetic idempotency documentation/test values, not credentials;
- historical credential paths, large/binary objects and commit-message secret review: PASS;
- current and historical privacy: PASS after authorized history rewrite; no former machine-specific path remains in reachable history;
- author identity: PASS — GitHub noreply only;
- dependency and vendored-source licensing/provenance: PASS within the recorded audit scope;
- Serea project license: PASS — MIT;
- project-name collision: OWNER_ACCEPTED;
- README / SECURITY / CONTRIBUTING claim accuracy: PASS;
- GitHub Actions threat model: PASS — `contents: read`, no `pull_request_target`, no self-hosted runner, no workflow secrets, immutable full-SHA Action pinning, finite timeouts and no build cache;
- runner-label/static workflow validation: PASS;
- P2 closure consistency and rewritten Git history preservation: PASS;
- sanitized private remote verification: PASS — Private, `main` correct, Actions disabled, zero runs.

**PUBLICATION_GATE = PASS.** The existing repository was subsequently changed from Private to Public without deletion or recreation; `main` and the rewritten history were preserved.


## Public release and CI validation

- Repository: <https://github.com/Yrika819/Serea>
- Visibility: **PUBLIC**; default branch `main`.
- GitHub Actions were enabled only after Public visibility was verified.
- Fast CI manual bootstrap: run `37477567529` — **SUCCESS** on head `084b5048176afe94d63127e30ad07a8c41ca7e0f`.
- Full CI manual bootstrap: run `37477783465` — **SUCCESS** on the same head.
  - Linux stable full validation: **SUCCESS**, including all-target tests, all-feature tests, warning-denied Clippy, Group O/docs/smoke and release fault-seam exclusion.
  - Linux MSRV 1.85.0: **SUCCESS**, with the exact compiler gate plus check/tests/Clippy.
  - macOS Intel x86_64: **SUCCESS**, including all-feature tests and P2H crash/fault suite.
  - macOS arm64: **SUCCESS**, including all-feature tests and P2H crash/fault suite.
- Apple Silicon empirical claim is deliberately narrow: **GitHub-hosted macOS arm64 CI passed.** It is not universal device or power-loss durability certification.
- Release fault-seam proof completed successfully in the Linux stable job; its existing A/B/C/D scope remains unchanged.

## Public repository security settings

- Private Vulnerability Reporting: enabled.
- Dependabot vulnerability alerts: enabled.
- Dependabot security updates: enabled.
- Secret scanning: enabled.
- Secret scanning push protection: enabled.
- GitHub Actions workflow permissions remain least-privilege at `contents: read`; no workflow secrets or self-hosted runners are used.
- `main` branch protection blocks force pushes and branch deletion and requires the `Linux fast checks` status in normal protected-flow updates. Administrator enforcement is disabled so the single-owner repository retains an emergency maintenance path; mandatory external review is not configured.
