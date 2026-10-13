# Six Platform Core Foundation — Engineering Evidence

Status: **VERIFIED on implementation revision `e5aca230d5f2ba87db48785c1c430720ea50c868`; final PR checks run on the report-only tip.**

Branch: `platform/six-target-core-foundation`

Implementation revision: `e5aca230d5f2ba87db48785c1c430720ea50c868`

PR: [#16 — Six-platform foundation and synthetic host](https://github.com/Yrika819/Serea/pull/16) (Draft)

## 1. Live repository preflight

- `Yrika819/Serea` is public; live `main` was `f70f34c1f683df1b175b612ba100ce6bb6979811` before the branch was created.
- PR #6 and #10 are merged. PR #12–#15 remained open drafts.
- PR #15 HEAD was `bcb49472a8d96020226b9d2e8625e57095abb15c`; its independent verdict was `P7_INDEPENDENT_RECHECK_PASSED` for disabled synthetic Gate C only.
- This branch was created from verified main. PR #12–#15 were not modified.

## 2. Implementation by area

- **CI/platform:** added six native stable jobs and six native Rust 1.85.0 jobs. Each verifies OS, OS architecture, compiler host triple, and compiled host binary architecture. Stable jobs run locked metadata, default/all-target tests, all-feature tests, release builds, and Core smoke. MSRV jobs run checks and tests with defaults/all features plus release builds.
- **Core:** added `serea-core` as a synchronous composition root for SQLite Store, Event Bus, Task Engine, and Scheduler recovery. Startup takes an injected clock and Event Bus. Normal startup installs no provider or capability generation. A development-only method can install the side-effect-free synthetic descriptor.
- **Host:** added `serea-host` with development `doctor`, `status`, and opt-in `smoke --synthetic` commands. Testkit is optional behind the `synthetic` feature. The smoke creates one Task, persists `RECEIVED → PLANNING`, verifies audit/event ordering, closes/checkpoints SQLite, reconstructs Core, runs recovery, verifies schema v8 and durable task state, and ensures one `TaskCreated` event.
- **Cross-platform source semantics:** added LF checkout attributes so immutable migration bytes and embedded Rust source are stable on Windows. The TaskEngine source guard canonicalizes path separators before excluding test-only paths.
- **Persistent schema:** no migration SQL, accepted ADR, P7 Memory default, or canonical schema version changed; default schema remains v8.

### Changes by commit

| Commit | Change |
| --- | --- |
| `aa4203a` | Added native six-platform stable and MSRV CI matrices. |
| `6c36c70` | Added Core composition and opt-in synthetic executable lifecycle. |
| `9ff9c35` | Recorded isolated cap-std, keyring-core, and Lindera SQLite assessments. |
| `e988eb1` | Corrected macOS OS identity detection in native jobs. |
| `906ff85` | Fixed identical-manifest concurrent registry activation; cherry-picked the canonical PR #14 fix without modifying PR #14. |
| `6af9e49`, `93976b4` | Closed lingering TaskEngine/Store SQLite handles before deleting test databases on Windows. |
| `d0426d0` | Gave the distinct-generation activation race its own manifest digest. |
| `94fad67` | Fixed LF checkout semantics for canonical migration/source bytes on Windows. |
| `e5aca23` | Normalized path separators in the TaskEngine production-source guard. |
| `1590b5a` | Recorded final implementation and native validation evidence. |

## 3. Native matrix and regression verification

Verified on GitHub Actions run [38392787006](https://github.com/Yrika819/Serea/actions/runs/38392787006), for implementation revision `e5aca230d5f2ba87db48785c1c430720ea50c868`:

| Native runner | Target triple | Stable Rust 1.99.0 | Rust 1.85.0 |
| --- | --- | --- | --- |
| `windows-2025` | `x86_64-pc-windows-msvc` | **PASS** | **PASS** |
| `windows-11-arm` | `aarch64-pc-windows-msvc` | **PASS** | **PASS** |
| `macos-15-intel` | `x86_64-apple-darwin` | **PASS** | **PASS** |
| `macos-15` | `aarch64-apple-darwin` | **PASS** | **PASS** |
| `ubuntu-24.04` | `x86_64-unknown-linux-gnu` | **PASS** | **PASS** |
| `ubuntu-24.04-arm` | `aarch64-unknown-linux-gnu` | **PASS** | **PASS** |

- Each stable job verified the compiler host and output executable architecture; synthetic Core smoke passed on all six targets.
- Stable compiler: `rustc 1.99.0 (b940084d7 2026-09-28)`, `cargo 1.99.0 (5f94df478 2026-08-27)`.
- MSRV compiler: `rustc 1.85.0 (4d91de4e4 2025-02-17)`, `cargo 1.85.0 (d73d2caf9 2024-12-31)`.
- Full CI [run 38392786894](https://github.com/Yrika819/Serea/actions/runs/38392786894): **PASS**, including stable Clippy, rustfmt, documentation, Python guards, stable/MSRV Linux tests, macOS Intel/ARM tests, and release fault-seam exclusion.
- Cross-architecture SQLite portability [run 38392786851](https://github.com/Yrika819/Serea/actions/runs/38392786851): **PASS**, cleanly closed synthetic DBs reopened on the other macOS architecture in both directions.
- Fast CI [run 38392786904](https://github.com/Yrika819/Serea/actions/runs/38392786904): **PASS**.
- P2H child-process termination and recovery tests are part of the workspace suites and passed on all six native jobs. The Core host's own smoke uses orderly shutdown followed by reconstruction/recovery; it does not inject a kill into the host process itself.

Regression defects found and corrected during native runs:

1. Windows temp SQLite files could not be removed while TaskEngine/Store handles remained open. Tests now drop those handles before cleanup.
2. The concurrent activation regression fixture used identical manifest digests for a distinct-generation race. The distinct-generation test now uses a distinct digest while same-manifest deduplication has its own coverage.
3. Windows checkout line endings changed raw migration hashes and source probes. LF checkout attributes retain canonical migration bytes; migration SHA-256 values remain identical.
4. A TaskEngine source guard searched only for `/tests/`, so Windows `\tests\` paths were incorrectly treated as runtime sources. Paths are normalized to `/` before the guard.

## 4. Additional verification evidence

| Check | Result |
| --- | --- |
| Stable workspace default/all-target tests | **PASS** on all six native targets |
| Stable workspace all-features tests | **PASS** on all six native targets |
| Rust 1.85.0 default/all-feature tests | **PASS** on all six native targets |
| Stable release workspace build | **PASS** on all six native targets |
| Rust 1.85.0 release workspace build | **PASS** on all six native targets |
| Synthetic Core lifecycle and restart recovery | **PASS** on all six native targets |
| Workspace Clippy `-D warnings`, rustfmt, docs validator | **PASS** in Full CI |
| Python smoke/architecture guard and identity guard | **PASS** in Full/Fast CI |
| Migration SHA-256 versus verified main, 0001–0008 | **PASS**, all eight unchanged |
| Release fault-seam exclusion | **PASS** in Full CI |
| Git whitespace validation | **PASS** (`git diff --check`) |

The latest PR tip after this report-only update has its own Full CI and six-platform native matrix in progress; the live PR checks are authoritative for that exact tip.

## 5. External component assessment

Detailed upstream commits, licenses, MSRV, dependency/security review, test evidence, and recommendation are in [External Component Assessment](external-component-assessment.md). No assessed component was added to Serea manifests or lockfile.

| Component | Recommendation | Evidence state |
| --- | --- | --- |
| cap-std 4.0.3 | **CONDITIONAL** | Linux x86_64 MSRV check passed; filesystem race/security behavior and other targets not tested. |
| keyring-core 1.0.1 | **DEFER** | Linux x86_64 MSRV check and 20 fake-store tests passed; native credential backends not tested. |
| Lindera SQLite 2.0.0 | **DEFER** | Rust 1.85 check blocked by resolved Lindera syntax and missing `stdarg.h`; Japanese search and performance not tested. |

## 6. Security boundaries and limitations

- The synthetic host has no external provider, network client, real account, credential, or capability invocation edge. The descriptor is `SideEffectClass::None`. Model Router invocation and P8 effects remain outside this host.
- Deterministic test IDs/time and disposable local SQLite are development fixtures, not production identity or clock implementations.
- No OS authentication, secure key custody, app-data directory selection, process lock, IPC, or lifecycle signal adapter is implemented. Security-sensitive facilities remain unavailable; no plaintext or mock-auth fallback exists.
- Structured tracing was not added. The smoke prints only content-free status facts and does not log task payloads.
- Only explicit clean-shutdown database reopening was portability-tested. Copying a live WAL database remains unsupported by this evidence.
- External component PoCs are scoped as documented; security properties not tested there remain unverified.

## 7. Recommended next milestone

1. Design typed OS filesystem/process/key-custody adapters around the actual Ledger/Checkpoint threat model; validate them independently on each desktop OS.
2. Add host-level process-kill/relaunch E2E if the host becomes a long-running process. Current committed TaskEngine crash tests cover process termination at durable transition boundaries.
3. Prototype the exact future file operations and races before adopting cap-std.
4. Revisit keyring-core only with compatible native backends and per-OS missing-key/refusal tests.
5. Build a disposable Japanese tokenizer harness with a pinned Rust-1.85-compatible Lindera version and benchmark all requested search strategies plus delete consistency.

This milestone does not authorize production Memory, real Owner authentication, key custody, external accounts, P8 effects, PR merges, or deployment.
