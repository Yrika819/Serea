# Six Platform Core Foundation — Engineering Evidence

Status: **PARTIAL — Draft PR requires six native runner results**

Branch: `platform/six-target-core-foundation`  
Base: `main` at `f70f34c1f683df1b175b612ba100ce6bb6979811`  
Final branch SHA and Draft PR URL: authoritative values are recorded on the linked GitHub PR; this report is part of that PR's source.

## 1. Live repository preflight

- `Yrika819/Serea` is public; live `main` equals `f70f34c1f683df1b175b612ba100ce6bb6979811`.
- PR #6 and #10 are merged. PR #12–#15 are open drafts.
- PR #15 HEAD equals `bcb49472a8d96020226b9d2e8625e57095abb15c`; its recorded independent verdict covers disabled synthetic Gate C only.
- No existing PR branch was modified. This branch starts at the verified `main` commit.

## 2. Implementation by area

- **CI/platform:** added a six-native-runner stable matrix plus an exact Rust 1.85.0 matrix. Jobs verify OS, OS architecture, compiler host triple, and compiled host executable architecture. Jobs run locked metadata, workspace checks/tests, all-feature tests, release builds, and the synthetic Core lifecycle. Existing `full-ci.yml` continues to enforce stable Clippy, rustfmt, docs, Python guards, and Linux release-fault exclusion.
- **Core:** added `serea-core` as a synchronous composition root for SQLite Store, Event Bus, Task Engine, and Scheduler recovery. Startup requires injected time and an Event Bus. Normal startup does not install a provider or capability generation. An explicitly named development method can install only an experimental, side-effect-free synthetic descriptor.
- **Host:** added `serea-host` with development `doctor`, `status`, and opt-in `smoke --synthetic` commands. Testkit is behind the optional `synthetic` feature. The synthetic flow creates a Task, persists `RECEIVED → PLANNING`, checkpoints, reconstructs Core, recovers Scheduler/Task durable state, verifies schema v8, audit ordering, and exactly one TaskCreated event, then closes and removes its disposable DB.
- **Architecture guard:** workspace dependency checks now permit the narrow Core wiring edges and keep testkit optional at the synthetic host boundary. Reusable libraries remain free of ambient process/environment access.
- **Persistent schema:** no migration, accepted ADR, P7 Memory, or default schema version change.

## 3. Local verification evidence

| Check | Result |
| --- | --- |
| Stable workspace default all-target tests | **PASS** |
| Stable workspace all-features tests and doctests | **PASS** |
| Rust 1.85.0 workspace all-target/all-feature check | **PASS** |
| Rust 1.85.0 workspace default all-target tests | **PASS** |
| Rust 1.85.0 workspace all-features tests and doctests | **PASS** |
| Stable Clippy, all targets/features, `-D warnings` | **PASS** |
| Rust 1.85.0 Clippy, all targets/features, `-D warnings` | **PASS** |
| Stable release workspace build | **PASS** |
| Release fault-seam exclusion proof | **PASS** |
| Linux x86_64 release binary format/ISA check | **PASS** (`ELF x86_64`) |
| Synthetic Core lifecycle, restart/recovery, event ordering | **PASS** (local Linux x86_64) |
| Workspace architecture guard and its 76 Python regressions | **PASS** |
| Commit identity guard and 3 regressions | **PASS** |
| Documentation validator (113 Markdown files) | **PASS** |
| Rustfmt and `git diff --check` | **PASS** |
| Migration SHA-256 versus `main`, 0001–0008 | **PASS**, all eight exact matches |
| Windows x86_64/ARM64 and macOS Intel/ARM64/Linux ARM64 native execution | **NOT TESTED locally; GitHub Actions required** |

Stable local compiler was `rustc 1.99.0`; MSRV was `rustc 1.85.0 (4d91de4e4 2025-02-17)` with `cargo 1.85.0`. Full platform results must be read from the final source HEAD's GitHub Actions checks; no matrix result is asserted here before those jobs finish.

## 4. External component assessment

Detailed versions, commits, licenses, MSRV, dependency and security boundaries, tested results, blocked areas, and recommendations are in [External Component Assessment](external-component-assessment.md). No evaluated dependency was added to production manifests.

| Component | Decision | Evidence state |
| --- | --- | --- |
| cap-std 4.0.3 | **CONDITIONAL** | Linux x86_64 MSRV check passed; filesystem race/security and other targets not tested.
| keyring-core 1.0.1 | **DEFER** | Linux x86_64 MSRV check and 20 mock tests passed; native credential backends not tested.
| Lindera SQLite 2.0.0 | **DEFER** | MSRV check blocked by resolved Lindera syntax and missing `stdarg.h`; search and performance not tested.

## 5. Security boundaries and limitations

- The host contains no external model provider, network client, real account, credential, or capability provider. The experimental descriptor has `SideEffectClass::None`; Core exposes no capability invocation path. Model Router invocation and P8 effects remain outside this host.
- The synthetic feature uses deterministic test IDs/time and disposable local SQLite only. It is not a production identity or clock implementation.
- No OS authentication, secure key custody, app-data directory selection, process lock, IPC, or lifecycle signal adapter is implemented. Security-sensitive operations remain unavailable; no plaintext/mock-auth fallback exists.
- Structured tracing was not added. No user content is logged by the host smoke; the CLI prints only content-free status facts.
- Recovery is exercised by orderly Core reconstruction in one process. Existing TaskEngine crash tests cover child termination at committed and uncommitted transition points, but a host-process kill/relaunch test is **NOT TESTED**.
- Local verification is Linux x86_64 only. The final six-target result is pending the native GitHub runner matrix on the pushed PR HEAD.

## 6. Remaining work and next milestone

1. Run the final stable and Rust 1.85.0 matrices on the final PR HEAD; resolve reproducible native failures and rerun affected jobs.
2. Review the Core composition contracts and the synthetic registry bootstrap with the architecture owners.
3. Prototype portable file operations for the actual checkpoint/ledger threat model before adopting cap-std.
4. Define a typed OS key-custody adapter and evaluate compatible native backends before integrating keyring-core.
5. For Japanese search, build a disposable schema/tokenizer harness with a pinned Rust-1.85-compatible Lindera version; compare all four requested strategies and test delete consistency.

This milestone does not authorize production Memory, real Owner authentication, key custody, external accounts, P8 effects, PR merges, or deployment.
