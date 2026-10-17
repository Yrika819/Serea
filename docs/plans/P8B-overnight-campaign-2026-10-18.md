# P8B overnight trust-boundary campaign — 2026-10-18

## Baseline and execution controls

- Repository: `Yrika819/Serea`; PR #35 HEAD `b9a0ef070bbec396f107ccb2b5a43925b0e22ed0`.
- PR #35 is an open Draft stacked on PR #34; PR #34 HEAD is an ancestor. `main` is `e0acb0254820c381dd97828824bef86a1db5176c`. No PR #36 exists at preflight.
- Worktree: `/workspace/Serea-overnight`, branch `codex/p8b-overnight-2026-10-18`, based directly on PR #35. Original `/workspace/Serea` remains clean and on its initial checkout.
- Cargo target/cache are kept under `/workspace/.serea-build` for isolation. Rust toolchains are installed under `/workspace/.serea-build/rustup` and `/workspace/.serea-build/cargo` only; these paths are outside both Git worktrees.
- ADR-0046 stays PROPOSED / NOT RATIFIED. This plan authorizes only tests, synthetic models, and evidence documentation. No production authority, dispatch, provider call, migration, credential, memory, receipt, retry, or external effect is in scope.

## Tracks

| Track | Owner / output | Test or evidence | Time bound and stop condition | Proof level |
|---|---|---|---|---|
| G — full Grant integrity | Storage test module; direct SQLite contention witness and bounded extra hostile fixtures if safe | targeted Grant probe, rollback/reopen/integrity, exact zero-timeout `BEGIN IMMEDIATE` Busy/Locked witness; test-only predicate mutation if practical | First priority. Stop on uncertain fixture semantics or changes that require runtime/public API edits | Test-only only; no authorization |
| W — Worker/Lease | Storage/TaskEngine audit and decision evidence | existing Lease APIs, source inspection, external Cargo probe | Stop at public API bypass; no API deletion or compatibility break | Partial; arbitrary caller can acquire raw lease |
| T — Clock | Storage transaction and expiry source audit | existing file-backed transaction clock test, API audit | Stop at caller clock source-trust boundary | Partial; caller-supplied time remains |
| O — Owner authentication | Policy/Storage API audit and external consumer probes | positive arbitrary-verifier probe plus candidate privacy compile-fail where available | Stop before OS/device authentication or production issuer | Blocked on Owner-selected issuer contract |
| P — Provider instance | Capability/Host code and existing synthetic tests | same-ID different-Arc test, default health audit | Stop before Host lifecycle or invoke changes | Partial; no exact Host-owned instance proof |
| Native | Actions logs and historical PR #34 failure context | exact-HEAD run/job and PR #34 run logs | Stop if host telemetry/reproduction requires Windows access | Root cause unconfirmed unless direct evidence exists |
| Integration/report | campaign report and at most one stacked Draft PR | Stable/MSRV workspace suites, lint, docs, probes, CI for exact final SHA | Stop after one bounded correction cycle per reproducible failure | Explicit pass/fail/pending per command and SHA |

## Initial evidence

- The PR #35 exact HEAD has completed Fast CI, Full CI, Native platform validation, Cross-architecture SQLite portability, and Git hooks Windows compatibility successfully (run IDs `39044878214`, `39044878234`, `39044878224`, `39044878258`, `39044878218`). These are baseline evidence only, not evidence for this branch.
- The existing Grant probe recomputes the full Request digest and the approved Grant subset digest independently, checks all member rows, positions, selected Step, capability/task bindings, seal cardinality, use rows and use counters. Its fixture now compares every ordinary SQLite table row before and after the read-only probe. It is private to tests and is not wired to runtime authorization.
- The existing writer contention probe signals before the competing writer attempts SQLite. The campaign therefore adds a separate zero-timeout connection that attempts `BEGIN IMMEDIATE` while the first Store transaction is held and classifies only SQLite `DatabaseBusy`/`DatabaseLocked` as the direct lock witness.
- The local execution image initially had no Rust toolchain. Stable 1.99.1 and Rust 1.85.0 were installed under `/workspace/.serea-build` after confirming network access; no repo/global Rust configuration was changed.
- The first all-workspace Stable test attempt exhausted the dedicated 4.9 GiB `/tmp` mount while Cargo linked several test binaries concurrently. This produced linker `SIGBUS` and `No space left on device`, not a compiler/test assertion. The complete isolated Rust/Cargo/target trees were moved to the workspace overlay (25 GiB free at relocation) and full-suite retry uses `CARGO_BUILD_JOBS=2`.

## Final record

Final evidence: Stable and Rust 1.85.0 all-target/all-feature workspace tests pass (647 Storage unit tests each; two manual resource benchmarks ignored). Stable and MSRV workspace check and Clippy with `-D warnings`, Stable release Host no-default-features check, formatter, metadata, docs validator, external Cargo authority probes, identity/preflight/hook Python suites, and diff check pass. The first whole-suite attempt with the new table snapshot found `no such column: rowid` on `p7_memory_fts_config`; the helper now orders by declared columns, the isolated RED mutation fails on the `digest` fixture, and restored GREEN targeted tests pass. The first published SHA actions were Fast `39053418119` and Git hooks `39053418118` success; Full `39053418085`, Native `39053418122`, and SQLite `39053418124` were queued at audit. They predate the final snapshot fix. Exact final-HEAD Actions IDs and conclusions are in the Japanese handoff report. ADR-0046 remains PROPOSED; unresolved Owner choices and hard gates below remain in force.
