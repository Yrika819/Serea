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

## P3B — Durable event foundation (closed)

| Evidence | Result |
|---|---|
| Commit | df738660ab5196539b771234745e981b57baaba1 |
| Scope | Migration 0002, event metadata/content schema, transactional typed event append, scheduler durable schema foundation, serea-event-bus crate edge |
| RED proof | cargo test -p serea-storage migration_0002_tests:: first failed because the migration catalog lacked version 2; the typed append test then failed to compile because Tx::append_event did not exist |
| Focused GREEN | cargo test -p serea-storage migration_0002_tests:: --offline — 11 tests PASS; cargo test -p serea-event-bus --offline — PASS |
| Workspace checks | docs validator PASS; workspace smoke PASS; regression suite PASS (75 tests; one extra case guards the new Storage/Event Bus direction); fmt, metadata, diff check, full workspace all-target/all-feature tests, and clippy -D warnings PASS |
| Covered behavior | Migration 0001 checksum unchanged; 0001→0002 apply/reopen; production 0002 DDL rollback; seq allocation + event row transaction atomicity; rollback creates no committed gap; two Store connections serialize; content immutability; privacy-minimal sequence/range columns; payload, transaction-count, and logical byte capacity refusals |
| Cloud validation | PASS on P3B commit |
| Fast CI | PASS — PR run 37503004570, job 112404502008; explicit dispatch run 37503026250 also PASS |
| Full CI | PASS — run 37503034192 |
| Linux stable / MSRV 1.85 | PASS — jobs 112404608220 / 112404608779 |
| GitHub-hosted macOS Intel x86_64 / arm64 | PASS — jobs 112404608720 / 112404608694 |
| Release fault-seam proof | PASS — Linux stable job 112404608220 |
| Sequential review | PASS; no generic SQL API or reverse Storage→Event Bus dependency; byte accounting is deterministic logical bytes and excludes SQLite page/WAL representation |
| Nonclaims | No retention execution, range-aware replay runtime, Scheduler runtime, Task Engine event participation, or crash-matrix closure yet |

Paired portability evidence: GitHub-hosted macOS Intel x86_64 CI passed.
GitHub-hosted macOS arm64 CI passed. Each ran fresh migration 0001→0002,
close/reopen, and Store integrity validation through the storage migration test.
No cross-architecture database artifact was exchanged.

## P3C — Atomic Task Engine event participation

| Evidence | Result |
|---|---|
| Behavior commit | fb647f7bbea802855a5a16bd552747e8fdbce899 (`feat: make task transitions event-atomic`) |
| Documentation follow-up | e593a057498a7dff77b1b3cbc28bed4e2a494db0 (`docs: clarify Task Engine event role`) |
| Scope | Fixed Storage audit+event participants; Task Engine event mapper; recovery event participation; operation inventory in [P3C task event inventory](P3C-task-event-inventory.md) |
| RED proof | Before implementation, `p3c_task_creation_commits_task_journal_event_and_sequence_together` observed task+journal counts `(1,1)` with event+seq `(0,0)`; expected all four to commit. |
| Focused GREEN | `cargo test -p serea-task-engine --all-targets --offline` — PASS (all five task-engine test binaries); Storage event-participant serialization rollback test PASS; focused workflow/recovery suites PASS. |
| Failure-path coverage | Event mapper serialization failure; journal insert trigger failure; duplicate EventId insert failure; illegal transition refusal; terminal-specific/generic event selection; step lease writes produce no task lifecycle event; committed create retry produces no duplicate event. |
| Workspace checks | Docs validator, workspace smoke, 75 smoke regression tests, metadata, fmt, diff check, full workspace all-target/all-feature test, and Clippy `-D warnings` — PASS. |
| Cloud validation | PASS locally in the Codex Cloud workspace; exact commit evidence pending. |
| Fast CI | PASS — run `37506453647`, job `112416544924`, at docs-only descendant `e593a05`; earlier P3C run `37506345232` was canceled by that follow-up push before workspace check completed. |
| Full CI | PASS — run `37506354685`, on behavior commit `fb647f7`. |
| Linux stable / MSRV 1.85 | PASS — jobs `112415916865` / `112415917357`. |
| GitHub-hosted macOS Intel x86_64 / arm64 | PASS — jobs `112415917261` / `112415917325`. |
| Release fault-seam proof | PASS — Linux stable job `112415916865`. |
| Sequential review | PASS. Fixed TaskAuditParticipant + EventParticipant composition only; Task Engine accepts the concrete Event Bus mapper; no generic registry/list/callback or Store re-entry; Event Bus depends only on Protocol/Storage; Storage has no Event Bus edge. |
| Nonclaims | No range-aware replay/retention runtime, Scheduler runtime, scheduler-produced events, or P3 crash-matrix closure. |

P3D–P3G evidence will be appended after each gated phase.

## Project nonclaims

P3 does not include Model Router, Capability Registry, Policy Engine, Approval
runtime, Memory runtime, Gmail, Calendar, Android provider execution, GoalLatch,
production credentials, or arbitrary exactly-once external effects. No claim of
universal crash/power-loss durability is made.
