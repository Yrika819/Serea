# P3 Event Bus and Scheduler Closure

This record tracks accepted architecture, phase commits, validation evidence,
and the limits of each claim. P3 runtime work does not begin until P3A's Fast CI,
Full CI, Linux, MSRV, Intel macOS, arm64 macOS, and release fault-seam gates are
green on its exact commit.

## Baseline

| Item | Value |
|---|---|
| Starting branch | `p3/preimplementation-audit` |
| Starting commit | `d280754b4175daa55300a1ac4f83031ca708cda0` |
| Owner semantic choice | ADR-0026 Option A, accepted |
| Architecture version | `serea-arch/2.0.0` |
| Replay response surface | `serea.device/2` |
| Event object surface | `serea.event/1` unchanged |
| P3 implementation branch | `p3/event-bus-scheduler`, created from P3A closure commit |
| Draft PR | [#1](https://github.com/Yrika819/Serea/pull/1), open against `main`, not merged |

## P3A — Contract and transaction gate

| Evidence | Result |
|---|---|
| Closure commit | `687bee58205b1e71e63425fda16172ab4ab45ad1` |
| Evidence record commit | e6d05b65414570fd927e647e4dc375302272c2c6 |
| Scope | ADR-0026 Option A; Event/Device replay contract; protocol registry and current architecture references; audit reconciliation |
| RED tests | Not applicable to docs/version-registry closure; existing protocol registry checks run below |
| GREEN tests | `python3 tests/workspace_smoke.py`; 74-test `workspace_smoke_tests.py`; `cargo test -p serea-protocol --offline` |
| Docs validation | `python3 tools/validate_docs.py docs` — PASS |
| Cargo metadata | `cargo metadata --no-deps --format-version 1 --offline` — PASS |
| Formatting | `cargo fmt --all -- --check` — PASS |
| Diff check | `git diff --check` — PASS |
| Fast CI | PASS — run `37498596845`; Linux fast job `112389455412` |
| Full CI | PASS — run `37498595882` |
| Linux stable / MSRV 1.85 | PASS — jobs `112389449468` / `112389449489` |
| GitHub-hosted macOS Intel x86_64 / arm64 | PASS — jobs `112389449064` / `112389449430` |
| Release fault-seam proof | PASS — included in Linux stable full job `112389449468` |
| Sequential review | PASS — docs/version surfaces reconciled; explicit device/1 and device/3 refusals tested; no P3 runtime behavior entered |
| Nonclaims | No migration 0002, Event Bus runtime, Scheduler runtime, P3 task transitions, or runtime retention/replay behavior yet |

## P3B — Durable event foundation (in progress)

| Evidence | Result |
|---|---|
| Scope | Migration 0002, event metadata/content schema, transactional typed event append, scheduler durable schema foundation, serea-event-bus crate edge |
| RED proof | cargo test -p serea-storage migration_0002_tests:: first failed because the migration catalog lacked version 2; the typed append test then failed to compile because Tx::append_event did not exist |
| Focused GREEN | cargo test -p serea-storage migration_0002_tests:: --offline — 11 tests PASS; cargo test -p serea-event-bus --offline — PASS |
| Workspace checks | docs validator PASS; workspace smoke PASS; regression suite PASS (75 tests; one additional test guards the new Storage/Event Bus direction); fmt, metadata, diff check, and full workspace all-target/all-feature tests PASS |
| Covered behavior | Migration 0001 checksum unchanged; 0001→0002 apply/reopen; production 0002 DDL rollback; seq allocation + event row transaction atomicity; rollback creates no committed gap; two Store connections serialize; content immutability; privacy-minimal sequence/range columns; payload, transaction-count, and logical byte capacity refusals |
| Cloud validation | Workspace full gates in progress |
| Fast CI / Full CI | Pending P3B commit |
| Linux stable / MSRV 1.85 / Intel / arm64 / fault proof | Pending paired GitHub Actions |
| Sequential review | In progress; no generic SQL API or reverse Storage→Event Bus dependency; byte accounting is deterministic logical bytes, independent of SQLite page/WAL representation |
| Nonclaims | No retention execution, range-aware replay runtime, Scheduler runtime, Task Engine event participation, or crash-matrix closure yet |

P3C–P3G have not started. Add each phase's commit, RED/GREEN evidence, Cloud
validation, Fast/Full Actions run and job IDs, paired Mac results, review
findings, and nonclaims when that phase closes.

## Project nonclaims

P3 does not include Model Router, Capability Registry, Policy Engine, Approval
runtime, Memory runtime, Gmail, Calendar, Android provider execution, GoalLatch,
production credentials, or arbitrary exactly-once external effects. No claim of
universal crash/power-loss durability is made.
