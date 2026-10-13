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

The implementation revision above passed the linked Full CI and six-platform matrix. The report-only PR tip and its exact check conclusions are recorded on PR #16.

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

## 8. PR #16 synthetic Core API security remediation

Independent review finding: **MEDIUM — `Core::install_synthetic_registry_generation` was a public, ungated release API in `crates/serea-core/src/lib.rs`.** At reviewed baseline `66bf5861292dfd59f575ef67480b5175e215be40`, this allowed an ordinary Core consumer to install and activate the synthetic registry fixture.

### RED reproduction

An independent temporary consumer crate depended on `serea-core` by path and called the method from a function accepting `&Core`. `cargo +stable check --manifest-path /tmp/serea-core-api-probe/Cargo.toml` completed successfully at the baseline, proving the method resolved through the public dependency API. This probe did not use source-text inspection.

After the correction, the same consumer call fails with Rust diagnostic `E0599: no method named install_synthetic_registry_generation found for reference &Core`. The checked-in `tools/check_core_api_boundary.py` reproduces this consumer boundary and fails unless that specific missing-method diagnostic is present. Full CI runs that guard.

### Remediation architecture

- Removed fixture construction and all fixture-only imports/error variants from the ordinary `serea-core` API. Core no longer has a regular dependency on `serea-capability`.
- Moved the fixed, `SideEffectClass::None` synthetic fixture setup into the existing `serea-host` binary under its opt-in `synthetic` feature. Only that feature enables Host's optional `serea-capability`, `serea-storage`, and `serea-testkit` dependencies.
- The explicit `smoke --synthetic` command creates a disposable SQLite database, installs one fixture through the existing trusted registry and Store functions, closes the setup Store, and then opens Core with the same EventBus. It rejects an already active generation and verifies the Task's durable generation pin.
- Default Host commands remain `doctor` and `status`; they do not open a Core database or bootstrap a registry. Synthetic code is not compiled into the default Host build.
- Added a Core startup regression test which opens/shuts down Core and inspects the durable database to verify that no active registry was installed.

### Local remediation evidence

At the remediation worktree HEAD before publication:

- Rust 1.85.0 all-target/all-feature workspace check: **PASS**.
- Rust 1.85.0 default all-target workspace tests: **PASS**.
- Rust 1.85.0 all-feature workspace tests: **PASS**.
- Rust 1.85.0 workspace release build and standalone Core/Host release builds: **PASS**.
- Rust 1.85.0 explicit synthetic Host smoke: **PASS**; schema v8, Task generation pin, active-generation refusal, ordered audit, shutdown/checkpoint, reopen and recovery verified.
- Stable explicit synthetic Host smoke: **PASS** with the same durable-state checks.
- External consumer negative API probe: **PASS**, expected `E0599` observed.

The native workflow now runs the explicit synthetic smoke in both its Stable and Rust 1.85.0 matrices on every native runner, in addition to each matrix's default/release build checks.

Default/release consumers receive only portable Core composition and lifecycle operations. Synthetic registry installation exists only in the Host binary's explicit opt-in development harness. Neither boundary adds provider/network access, real credentials, or capability invocation.

### GitHub Actions and publication record

Source commit `f879adf4cf2e55856279a2e7a59349101f842a7f` passed:

| Workflow | Run | Result |
| --- | ---: | --- |
| Native platform validation | [38429458077](https://github.com/Yrika819/Serea/actions/runs/38429458077) | **12/12 native jobs PASS**: Stable and Rust 1.85.0 on Windows x86_64/ARM64, macOS x86_64/ARM64, and Linux x86_64/ARM64. Every job ran native workspace checks/tests, all-feature tests, release build, synthetic smoke, and executable architecture verification. |
| SQLite cross-architecture portability | [38429457961](https://github.com/Yrika819/Serea/actions/runs/38429457961) | **PASS**, both producer/consumer directions. |
| Fast CI | [38429458299](https://github.com/Yrika819/Serea/actions/runs/38429458299) | **FAIL at commit identity guard** before later checks. Workspace smoke guard and all 76 workspace smoke regression tests passed. |
| Full CI | [38429458242](https://github.com/Yrika819/Serea/actions/runs/38429458242) | **FAIL at Linux stable commit identity guard**. Linux MSRV 1.85.0, macOS x86_64, and macOS ARM64 jobs passed. |

The identity guard rejects the author/committer identity on commits `ed3a118407fa4d27a8f40a1bdd2d912c1168f64e` and `f879adf4cf2e55856279a2e7a59349101f842a7f`. Both commits are already published. Their history is not rewritten, and the guard is not weakened. This is the outstanding campaign blocker: Fast CI and Full CI are not green, so PR #16 is not ready for the independent recheck despite the successful API remediation and native/SQLite workflows.

The reviewed baseline was `66bf5861292dfd59f575ef67480b5175e215be40`; at preflight PR #16 was Open/Draft/Unmerged with base `main` at `f70f34c1f683df1b175b612ba100ce6bb6979811`. Repository visibility was public, push permission was confirmed, and PRs #12–#15 were left unchanged. The PR remains Open/Draft/Unmerged. Migration files `0001`–`0008` and schema v8 are unchanged.

## 9. PR #16 commit identity repair and final-head validation

The PR branch identity repair was published on 2026-10-13 after verifying the live PR was Open/Draft/Unmerged, based on `main`, at the expected head, and that the target branch had not moved. The repository is public and the authenticated owner has push permission. A local bundle at `/workspace/serea-pr16-original.bundle` preserves the original branch objects without adding a ref that the identity guard can reach.

### Identity-only rewrite

The branch from base `f70f34c1f683df1b175b612ba100ce6bb6979811` to original HEAD `d71a459a52a2e29910a107994bebaa972873e607` was a 16-commit linear sequence with no merge or signed commits. Exactly two commits had invalid author and committer emails. Their human author name, timestamps, message, and tree were retained; the email was changed to the verified owner noreply identity. The unaffected preceding commits retain their original object IDs. Descendants were recreated only to point to the new parents.

| Original commit | Repaired commit | Message |
| --- | --- | --- |
| `ed3a118407fa4d27a8f40a1bdd2d912c1168f64e` | `a951d453fd9af32cc00cc57e4232f60ad9433e03` | `security(core): isolate synthetic registry fixture` |
| `f879adf4cf2e55856279a2e7a59349101f842a7f` | `6ce87a396c3ea87bd08c6e767097a00af7bdf868` | `ci: run synthetic smoke on MSRV native matrix` |
| `d71a459a52a2e29910a107994bebaa972873e607` | `a0270b1cb84b49ac4c47002dfea1eb3899067361` | `docs: record synthetic core remediation evidence` |

All 16 old/new commit pairs have identical tree SHA values, messages, author/committer names and timestamps; parent order and topology are equivalent. Only the two specified identities and descendant parent references changed. The original and repaired final tree SHA is `b7a2f8c6f9fc85179ce6ebe01e0379433d7feb1e`; `git diff d71a459a52a2e29910a107994bebaa972873e607 a0270b1cb84b49ac4c47002dfea1eb3899067361` is empty. No implementation, test, documentation or migration bytes were changed by the rewrite itself.

### Publication and validation

The branch was updated using `--force-with-lease` scoped to `refs/heads/platform/six-target-core-foundation` with expected old SHA `d71a459a52a2e29910a107994bebaa972873e607`. A fresh clone fetched exactly `a0270b1cb84b49ac4c47002dfea1eb3899067361`; its complete reachable history passes `python3 tools/check_commit_identity.py`. `python3 tools/validate_docs.py docs`, `python3 tests/workspace_smoke.py`, `git diff --check`, and the external consumer negative compilation probe passed. The probe failed with the expected E0599 for the removed `Core::install_synthetic_registry_generation` API.

The explicit synthetic Host path remains feature-gated; the default Host does not install synthetic registry authority. No external Provider or Capability invocation edge was introduced. Schema remains v8; migrations 0001–0008 remain unchanged. Rust 1.85.0 is still the declared and validated MSRV. The post-rewrite local Rust 1.85.0 workspace check, tests, release build, and synthetic Host smoke were run from the fresh clone; their result and the final GitHub Actions run IDs are recorded in the PR description so the final source HEAD remains stable.

The final Fast CI, Full CI, 12-job native platform validation, and SQLite portability run IDs and conclusions are maintained in the PR description. Independent review has not yet issued a PASS verdict. Remaining limitations are the same as those in §6: no production Memory, OS identity/key custody, real account/credentials, external provider invocation, P8 effects, or live-WAL copying.
