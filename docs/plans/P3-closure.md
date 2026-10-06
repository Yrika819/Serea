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

P3F–P3G evidence will be appended after each gated phase.

## P3D — Bounded replay and retention (closed)

| Evidence | Result |
|---|---|
| Behavior commit | 7ed1445cba008e8eda132d3c2379f44d0e6608cb (`feat: add bounded event replay and retention`) |
| Scope | Typed high-water replay pages; exact intentional-expiry ranges; compacted-prefix boundary; corruption detection; bounded whole-content retention; Task lifecycle event 30-day deadline |
| RED proof | New replay suite initially failed to compile because `ReplayItem`, replay/expiry APIs, and typed invalid-page errors were absent. |
| Focused GREEN | `cargo test -p serea-event-bus --test replay --offline` — 6 tests PASS; `cargo test -p serea-task-engine --test workflow --offline` — 32 tests PASS. |
| Covered behavior | Ordered pagination pinned to one high-water; appends after snapshot wait for next pass; exact expiry ranges; separate `HISTORY_EXPIRED_PREFIX`; content+ledger/range deletion rollback; consumer backlog does not delay expiry; unexplained absence is typed corruption; wire replay discriminators; replay/retention batch boundaries. |
| Integrity and bounds | `Store::verify_integrity` checks accounting, contiguous coverage, canonical retained event objects, disjoint/merged expiry ranges, and deterministic bytes; metadata carries no event/task/actor/device fingerprint or content. Replay max 256, expiry delete batch max 512, payload/event limits unchanged. |
| Workspace validation | docs validator PASS; workspace smoke PASS; 75 smoke regression tests PASS; metadata, fmt, diff check, all-target/all-feature workspace tests, and Clippy `-D warnings` PASS. |
| Fast CI | PASS — run `37508701133`, Linux fast job `112423895712` |
| Full CI | PASS — run `37508706509` |
| Linux stable / MSRV 1.85 | PASS — jobs `112423921030` / `112423921458` |
| GitHub-hosted macOS Intel x86_64 / arm64 | PASS — jobs `112423921066` / `112423920620` |
| Release fault-seam proof | PASS — Linux stable job `112423921030` |
| Sequential review | PASS; event content remains append-only and `serea.event/1` unchanged; retention is not cursor-gated; range and prefix results advance only through declared sequences; unexplained gaps stop at the first unverified sequence. |
| Nonclaims | No Scheduler runtime or event consumption yet; no cross-architecture database artifact exchanged; no power-loss durability claim. |

Paired portability evidence: GitHub-hosted macOS Intel x86_64 CI passed.
GitHub-hosted macOS arm64 CI passed. Both ran the fresh 0001→0002,
close/reopen, and integrity validation tests. No cross-architecture database
artifact was exchanged.

## P3E — Durable Scheduler storage and claims (closed)

| Evidence | Result |
|---|---|
| Initial behavior commit | 94084303e6cee055c5381db3a84d75ac499ba636 (`feat: add durable scheduler claims`) |
| Follow-up commit | 44ef12b13e385a5fff44fdc53bb66c7c31d1e647 (`fix: complete scheduler state command fences`) |
| Scope | Typed schedule creation and state commands with authenticated retry receipts; bounded occurrence rows; source EventId uniqueness; fenced occurrence claims and mappings; singleton durable Scheduler replay cursor |
| RED proof | New Scheduler storage suite first failed to compile because schedule/occurrence storage and claim APIs did not exist. Follow-up tests required pause/resume lifecycle behavior and duplicate source-event rejection. |
| Focused GREEN | `cargo test -p serea-storage scheduler_tests --offline` — 10 tests PASS; storage Clippy `-D warnings` PASS. |
| Covered behavior | Active schedule and pending occurrence bounds; competing Store connections; lease expiry/reclamation and stale-fence refusal; cancellation-versus-claim ordering; pause/resume revision handling; command retry idempotency; source EventId dedupe; event insertion rollback; pinned high-water cursor and stale consumer generation. |
| Workspace validation | docs validator, workspace smoke, 75 smoke regression tests, metadata, fmt, all-target/all-feature workspace tests, Clippy `-D warnings`, and diff check — PASS. |
| Fast CI | PASS — run 37511789290, Linux fast job 112434529945. |
| Full CI | PASS — run 37511795677. |
| Linux stable / MSRV 1.85 | PASS — jobs 112434550249 / 112434550240. |
| GitHub-hosted macOS Intel x86_64 / arm64 | PASS — jobs 112434549877 / 112434550097. |
| Release fault-seam proof | PASS — Linux stable job 112434550249. |
| Sequential review | PASS; lifecycle mutation, event, and receipt share one transaction; occurrence mapping is fenced and independent of schedule cancellation after claim; storage has no Task Engine or Event Bus dependency. |
| Nonclaims | No recurring occurrence expansion, Scheduler runtime, Task creation+mapping transaction, approval/device resumption, or integrated crash-matrix closure. |

Paired portability evidence: GitHub-hosted macOS Intel x86_64 CI passed.
GitHub-hosted macOS arm64 CI passed. Both ran all-feature tests and the task
engine crash suite. No cross-architecture database artifact was exchanged.

## Project nonclaims

P3 does not include Model Router, Capability Registry, Policy Engine, Approval
runtime, Memory runtime, Gmail, Calendar, Android provider execution, GoalLatch,
production credentials, or arbitrary exactly-once external effects. No claim of
universal crash/power-loss durability is made.
