# External Component Assessment

Status: source reviewed; isolated Linux x86_64 PoCs completed where stated. No component was added to Serea manifests or lockfile.

All three upstream repositories were cloned under `/tmp/serea-component-poc`, outside the Serea workspace.

## E1 — cap-std

- Upstream: `sunfishcode/cap-std` (redirects to `bytecodealliance/cap-std`); version `4.0.3`, commit `b7acf8e8807fe3fab991884d2208b7e03d35a409`.
- License: `Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT`; workspace MSRV: 1.70.
- **TESTED:** isolated `cargo +1.85.0 check -p cap-std` on Linux x86_64 passed. The checkout's lockfile needed an update, so this was a fresh compatible dependency resolution, not a locked reproducibility test. It emitted an upstream unused-import warning.
- **SOURCE REVIEWED:** scoped `Dir` handles and relative operations exist. Internal symlink/path-resolution code is substantial and uses platform branches. Dependencies include `cap-primitives`, `rustix` on non-Windows, `io-lifetimes`, and Windows system bindings.
- Symlink substitution, relative escape, replacement races, file identity, process coordination, directory sync behavior, performance, and the other five targets: **NOT TESTED**.
- Security boundary: a useful filesystem adapter building block; it does not establish portable file identity, inter-process locking, or directory durability semantics by itself.
- Recommendation: **CONDITIONAL** — prototype the exact future Ledger/Checkpoint operations and race cases on all target OSes before production adoption. This review does not alter or re-review PR #15's Gate C design.

## E2 — keyring-core

- Upstream: `open-source-cooperative/keyring-core`; version `1.0.1`, commit `50fe5c44fa3dc46cdf792acfe5116e1e8ebc2e0b`.
- License: `MIT OR Apache-2.0`; declared MSRV: 1.85.
- **TESTED:** isolated `cargo +1.85.0 check` passed on Linux x86_64. Its 20 library tests, including its mock credential store, passed. These tests use only fake/test data.
- **SOURCE REVIEWED:** this crate defines credential/store APIs and a mock, not macOS Keychain, Windows Credential Manager, or a Linux secure-store backend. Entry APIs expose secret values as `String`/`Vec<u8>`; the host must own zeroization, key versioning, missing-key refusal, and lifecycle policy.
- Native backend availability, concurrent OS-store behavior, unsupported/headless Linux behavior, missing-key refusal policy, and the `windows-native-keyring-store` compatibility path: **NOT TESTED** here. The identified Windows native backend requires Rust 1.88, above Serea's MSRV.
- It cannot establish fresh Owner authentication or complete rollback protection.
- Recommendation: **DEFER** — the core API alone is not key custody. Revisit only with MSRV-compatible backends and independent per-OS missing-key/refusal tests; never add plaintext fallback.

## E3 — Lindera SQLite

- Upstream: `lindera/lindera-sqlite`; version `2.0.0`, commit `01d5c6d3d5d0f999f6094c0a17f996753a7a67bd`.
- License: `MIT`. No `rust-version` is declared. Cargo manifest uses edition 2024 and a non-exact `lindera = "2.0.1"` requirement.
- **TESTED:** isolated Rust 1.85 Linux x86_64 check attempted. Cargo selected `lindera-dictionary 2.3.4`, whose let-chain syntax is rejected by rustc 1.85. The build also stopped in bindgen because this environment lacked a C `stdarg.h` include. This component is **BLOCKED/NOT TESTED** as a usable MSRV-compatible build; no Japanese search quality or performance result is claimed.
- **SOURCE REVIEWED:** it is a loadable FTS5 C extension, has a substantial unsafe FFI boundary, and defaults to the extension feature without embedding dictionaries. Dictionary feature size, runtime behavior, redistribution obligations, and all six packaging targets remain unmeasured.
- Search relevance versus `unicode61`, FTS5 trigram, and Rust-side preprocessing; false matches; latency; startup; peak memory; binary/index size; ARM64; and forget/delete consistency: **NOT TESTED**.
- Recommendation: **DEFER** — current dependency resolution fails the 1.85 compiler check and uses a C/SQLite extension boundary. Revisit only with a pinned MSRV-compatible Lindera version and a disposable schema/tokenizer/delete PoC.
