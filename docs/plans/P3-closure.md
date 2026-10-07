# P3 Event Bus and Scheduler Closure

This record tracks accepted architecture, phase commits, validation evidence,
and the limits of each claim. P3 runtime work does not begin until its phase gates
are green on the exact commit. All CI run identifiers in the P3A–P3E historical
sections below are **PRE_AUTHOR-SANITIZATION CI EVIDENCE**; those runs were not
executed on the rewritten SHAs.

## Baseline

| Item | Value |
|---|---|
| Starting branch | `p3/preimplementation-audit` |
| Starting commit | `02c0f0d0c813a332a7c86feb8960364cc21a23a7` |
| Owner semantic choice | ADR-0026 Option A, accepted |
| Architecture version | `serea-arch/2.0.0` |
| Replay response surface | `serea.device/2` |
| Event object surface | `serea.event/1` unchanged |
| P3 implementation branch | `p3/event-bus-scheduler`, created from P3A closure commit |
| Draft PR | [#1](https://github.com/Yrika819/Serea/pull/1), open against `main`, not merged |

## Privacy repair and sanitized baseline

The owner-authorized metadata-only rewrite covered both P3 refs after the
unchanged `main` baseline. It changed author and committer identities to the
approved GitHub noreply identity. Commit order, parent topology, author and
committer timestamps, messages, and every commit tree were verified unchanged.
The audit ref remains the rewritten equivalent of its P3A checkpoint and is an
ancestor of the implementation ref. No other branch or tag was rewritten.

| Ref | Before | After |
|---|---|---|
| `main` | `cb580be8dd0057202c6b0f9c783391460bfc1875` | unchanged |
| `p3/preimplementation-audit` | `687bee58205b1e71e63425fda16172ab4ab45ad1` | `3e58d174b07bbc7a44bd3857cf403a09c86c7689` |
| `p3/event-bus-scheduler` | `eabda5f1fc3ec2b9f656952f4adbe87acd745285` | `6a278d33d2f411d27c20cbb17cc8b06e6a99f668` |

The old-to-new mapping for each P3 commit follows. The mapping is retained to
reconcile source-history references; historical run IDs above remain evidence
for the old SHAs only.

| Old commit | Rewritten commit |
|---|---|
| `da117b5f9c4572bb989f5bf7f3d61b9cb2886164` | `e066561d7d3fd70e4f6300a519bda955b4583e65` |
| `d280754b4175daa55300a1ac4f83031ca708cda0` | `02c0f0d0c813a332a7c86feb8960364cc21a23a7` |
| `687bee58205b1e71e63425fda16172ab4ab45ad1` | `3e58d174b07bbc7a44bd3857cf403a09c86c7689` |
| `e6d05b65414570fd927e647e4dc375302272c2c6` | `c09dec3c48c32fbdceba9e7533e1e0d0318af722` |
| `df738660ab5196539b771234745e981b57baaba1` | `09fc6e653799e78fb4085b24cd51ff17502e14f9` |
| `c37178252399361109f05e54abc7828a4204a721` | `e0ab3be8c706337a6f6f8ac59ab168dce093b7fb` |
| `fb647f7bbea802855a5a16bd552747e8fdbce899` | `7f418e1254e053f8799d79bc5df46bf705542575` |
| `e593a057498a7dff77b1b3cbc28bed4e2a494db0` | `d9e89314121b8672f0a482981837ac0ae5e3c2a4` |
| `e11fa261c24c02fb5b1c51fb3b39fcc705d1faba` | `51f223c62db0f337f20d54d874c0022283d89e92` |
| `7ed1445cba008e8eda132d3c2379f44d0e6608cb` | `6156bd34c13994657bca110c2efd48f5b1cd236b` |
| `b3f7be3da6663bd0c61d3736f2a0869a055c3860` | `5eac2a4eb798da9e6f0db83ab40274542f6a823a` |
| `94084303e6cee055c5381db3a84d75ac499ba636` | `a16e00f6ed73f8b8d785e24908ebf401c66525d9` |
| `44ef12b13e385a5fff44fdc53bb66c7c31d1e647` | `346a773c622b0b9c16d0582704437a832b406180` |
| `4da482e94cd3ddd4d29b5ef5f4ab826c242b74b0` | `2a972c73f22053c78c4cff9ebfa258750e5f0b3f` |
| `eabda5f1fc3ec2b9f656952f4adbe87acd745285` | `6a278d33d2f411d27c20cbb17cc8b06e6a99f668` |

### Sanitized baseline validation

| Gate | Exact-SHA result |
|---|---|
| Commit | `8ab2eab8b1b399945644acb70fae3c62e8e24bdf` |
| Local docs, workspace smoke, 75 smoke unit tests, metadata, fmt, diff check, identity guard and its 3 tests | PASS in Cloud |
| Fast CI | PASS — PR run `37541026358`, Linux job `112533782791` |
| Full CI | PASS — PR run `37541026436` |
| Linux stable | PASS — job `112533783719` |
| Linux MSRV 1.85.0 | PASS — job `112533783718` |
| macOS Intel x86_64 | PASS — job `112533783397` |
| macOS Apple Silicon arm64 | PASS — job `112533783753` |
| Release fault-seam proof | PASS — job `112533783719` |

These runs validate the rewritten history plus identity guard and SHA document
reconciliation at the exact sanitized commit above. Duplicate push-triggered
runs `37541022537` (Fast CI) and `37541022549` (Full CI) also passed at the same
SHA.

## P3A — Contract and transaction gate

| Evidence | Result |
|---|---|
| Closure commit | `3e58d174b07bbc7a44bd3857cf403a09c86c7689` |
| Evidence record commit | c09dec3c48c32fbdceba9e7533e1e0d0318af722 |
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
| Commit | 09fc6e653799e78fb4085b24cd51ff17502e14f9 |
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
| Behavior commit | 7f418e1254e053f8799d79bc5df46bf705542575 (`feat: make task transitions event-atomic`) |
| Documentation follow-up | d9e89314121b8672f0a482981837ac0ae5e3c2a4 (`docs: clarify Task Engine event role`) |
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
| Behavior commit | 6156bd34c13994657bca110c2efd48f5b1cd236b (`feat: add bounded event replay and retention`) |
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
| Initial behavior commit | a16e00f6ed73f8b8d785e24908ebf401c66525d9 (`feat: add durable scheduler claims`) |
| Follow-up commit | 346a773c622b0b9c16d0582704437a832b406180 (`fix: complete scheduler state command fences`) |
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

## P3F contract — CalendarRecurrenceV1 (accepted; contract CI green)

The owner selected the structured JSON grammar in ADR-0027. Its only kinds are
ONCE, DAILY, and WEEKLY. V1 has exact kind-specific fields, a local civil minute
anchor, strict closed-object and duplicate-key validation, canonical SCJ-1
storage, ISO weekday ordering, and no end rule or additional frequency. The
Schedule timezone remains authoritative. Calendar occurrence identity is the
intended local label plus IANA timezone under ScheduleId; TZDB/evaluator
versions and resolved instants are separate durable facts. DST gaps resolve to
the first valid instant after the gap; folds select the earlier UTC instant.

Architecture changes from `serea-arch/2.0.0` to `serea-arch/2.1.0`, a minor
contract addition. `serea.scheduler/1` remains unchanged. ADR-0027, Scheduler
Protocol, Protocol Index, current architecture documents, decision index, crate
map, and this closure record carry the contract. No runtime code or Jiff
manifest dependency is included in this contract commit.

The docs-only contract closure passed exact-SHA Fast CI run `37542441358` and
Full CI run `37542441329`. Linux stable/release proof, MSRV 1.85, macOS Intel,
and macOS arm64 all passed on that contract commit. P3F runtime and P3G remain
open; P3 remains partially complete.

## P3F contracts — EventPredicateV1 and ScheduledTaskTemplateV1

ADR-0028 accepts exact registered-EventKind matching, dedicated-wake and
Scheduler-lifecycle exclusions, and cross-schedule Scheduler-causal-root
suppression. It also accepts a closed title/intent-only scheduled template,
host classification inheritance, `max_schedule_template_bytes = 32768`, and
occurrence-pinned template digests for edit/restart stability. Architecture
advances from `serea-arch/2.1.0` to `serea-arch/2.2.0`; event and Scheduler
surfaces remain `/1`. No runtime or migration change is claimed by this contract
record. ADR-0028 was included
in the follow-up contract commit `35e93a14f3e9ee9670f8563c738cd0d1e02a7e6e`;
Fast CI run `37606284419` and Full CI run `37606291329` passed on that exact
commit, including Linux stable/release proof, MSRV 1.85, Intel, and arm64.

## P3F contract — device-session task resume waits (ADR-0029 accepted)

`DEVICE_CONNECTED` now has the closed `DeviceConnectedPayloadV1` payload with
exactly `device_id`. Only an explicit `DeviceResumeWaitV1` keyed by TaskId
authorizes resumption; it records DeviceId, blocked task revision, Event Bus
registration high-water, and creation time. A matching connection event is
eligible only when its sequence is greater than that high-water. Scheduler
materializes uniquely keyed internal wakes in bounded pages before advancing
the source cursor. Task origin, task kind/title, `WAITING_USER`, generic
`BLOCKED`, and event correlation never imply eligibility. Generic
`DEVICE_OFFLINE` block is refused in favor of Task Engine's atomic
`block_for_device` operation.

ADR-0029 advances architecture from `serea-arch/2.2.0` to `serea-arch/2.3.0`;
`serea.event/1`, `serea.scheduler/1`, and `serea.task/2` remain unchanged.
Contract validation is green on exact commit
`35e93a14f3e9ee9670f8563c738cd0d1e02a7e6e`: Fast CI run `37606284419` and
Full CI run `37606291329`; Full CI jobs Linux stable/release proof
`112742997443`, MSRV 1.85 `112742997733`, macOS arm64 `112742997778`, and
macOS Intel `112742997819`. The later runtime implementation is recorded below;
these runs remain contract-only evidence and are not presented as runtime
validation.

## P3F contract — approval lifecycle wake handoff (ADR-0030 accepted)

Approval lifecycle events carry only typed routing identity (`approval_id`,
`task_id`, `step_id`) with matching task correlation and required matching
task/step trace. P3 is authorized to materialize and expose a durable,
deduplicated Scheduler wake and to preserve it until explicit P6 acknowledgement.
It does not interpret approval authority, apply outcomes, or transition tasks.
Architecture advances from `serea-arch/2.3.0` to `serea-arch/2.4.0`; Event,
Scheduler, Approval, and Task surfaces remain unchanged. This contract commit's
CI evidence: Fast CI run `37613362390` and Full CI run `37613362482`; Full CI
Linux stable/release proof job `112765733101`, MSRV 1.85 job `112765732716`,
macOS Intel job `112765733121`, and macOS arm64 job `112765733257`. Approval
runtime remains a P3 routing-only handoff; no P6 behavior is claimed.

## P3F — Scheduler runtime (behavior commit green)

Behavior commit `e34e4b178c982054c721dd912859c57addeaa60c` implements
`serea-scheduler`, deterministic ONCE/DAILY/WEEKLY recurrence using Jiff
`0.2.38` with bundled `jiff-tzdb 0.1.9` / TZDB `2026e`, exact EventPredicateV1
matching with Scheduler-causal suppression, ScheduledTaskTemplateV1 snapshot
storage, and atomic scheduled Task + journal + lifecycle event + occurrence
mapping. Calendar wakes, bounded SKIP/RUN_ONCE/RUN_EACH catch-up, RETRY_DUE,
HOST_EVENT replay, explicit device resume waits, and routing-only approval
wakes are included. Scheduler recovery preserves P6 approval wakes and resumes
only explicitly registered device waits. Approval events do not mutate Tasks
or apply approval authority.

Migration 0001 remains byte-identical (SHA-256
`d9068dccbc26ececb71be79c475080633166ba0163c62b2d98b9733512baefea`). The
migration 0002 checksum is
`4924e69150bbff9c39e2e6b7e2bdd61045202e504900fe0f510d513fbf815e67`.

Exact behavior-commit Actions evidence:

| Gate | Run/job | Result |
|---|---:|---|
| Fast CI | run `37625296024`, Linux job `112805394146` | PASS |
| Full CI | run `37625295706` | PASS |
| Linux stable and release fault-seam proof | job `112805394360` | PASS |
| Linux MSRV 1.85.0 | job `112805393968` | PASS |
| macOS Intel x86_64 | job `112805394383` | PASS |
| macOS Apple Silicon arm64 | job `112805394485` | PASS |
| Cross-architecture SQLite | run `37625295910` | PASS |
| Intel producer / arm64 producer | jobs `112805393495` / `112805393689` | PASS |
| arm64 artifact consumed on Intel / Intel artifact consumed on arm64 | jobs `112805788922` / `112806213407` | PASS |

The portability workflow transfers only cleanly closed main-database artifacts;
it excludes `-shm` and `-wal`. The supported claim is limited to a closed Serea
SQLite fixture produced on GitHub-hosted macOS Intel x86_64 being opened and
semantically validated on GitHub-hosted macOS arm64, and vice versa. This does
not claim live-WAL portability, universal hardware compatibility, or
power-loss durability.

## P3G — integrated crash and recovery closure

The P3G test suite integrates Event Bus retention and replay, calendar and
HOST_EVENT occurrences, explicit device resume, approval wake handoff, Task
Engine mapping, restart, and repeated recovery. It also runs two independent
Scheduler workers against one replayed source EventId and injects a schedule
cursor-update failure after occurrence admission to prove both writes roll back
together. Device-wait registration's event-participant failure also
proves Task state, journal, wait row, and Event sequence remain unchanged.

P3G closure commit is `89d726ab1af0b94b9fd2e865f8e6ba663e00998a`. Cloud
validation passed on that exact commit: workspace check, all-target tests,
all-feature tests, Clippy `-D warnings`, fmt, docs validation, workspace
smoke, 75 smoke unit tests, metadata, identity guard and its tests, diff check,
focused Scheduler tests, and the release fault-seam exclusion test. Migration
0001/0002 checksums and `Store::verify_integrity` were checked. The integrated
restart test verifies retention/range replay, HOST_EVENT materialization,
device wake recovery, approval wake preservation, scheduled mappings, and a
second recovery pass without duplicate Task events. Separate tests cover
two-worker HOST_EVENT replay/dispatch and calendar enqueue rollback.

Exact P3G commit Actions evidence:

| Gate | Run/job | Result |
|---|---:|---|
| Fast CI | run `37627364065`, Linux fast job `112812473100` | PASS |
| Full CI | run `37627364242` | PASS |
| Linux stable and release fault-seam proof | job `112812475054` | PASS |
| Linux MSRV 1.85.0 | job `112812474646` | PASS |
| macOS Intel x86_64 | job `112812475071` | PASS |
| macOS Apple Silicon arm64 | job `112812475322` | PASS |
| Cross-architecture SQLite | run `37627364099` | PASS |
| Intel producer / arm64 producer | jobs `112812472436` / `112812472942` | PASS |
| arm64 artifact consumed on Intel / Intel artifact consumed on arm64 | jobs `112812915059` / `112813274415` | PASS |

P3 is closed for the accepted Event Bus, Task Engine, and Scheduler contracts.
Approval lifecycle wakes are durable, typed routing handoffs only; P6 owns
approval authority and result application. Cross-architecture portability is
limited to cleanly closed main-database fixtures. No `-shm` or live-WAL
portability claim is made.

## Project nonclaims

P3 does not include Model Router, Capability Registry, Policy Engine, Approval
runtime, Memory runtime, Gmail, Calendar, Android provider execution, GoalLatch,
production credentials, or arbitrary exactly-once external effects. No claim of
universal crash/power-loss durability is made.


## Post-merge main validation

PR #1 was merged with GitHub's merge-commit method after exact candidate
revalidation. Main is merge commit
`1d27a91c0d037b9b2cfe42456611f13556895793`, with first parent
`cb580be8dd0057202c6b0f9c783391460bfc1875` and second parent the reviewed
P3 head `e6d57c5fe50d25148cd7b16d7cd830bf5df9255b`. The P3 branch remains
retained.

All results below are for the exact merge commit above.

| Gate | Run/job | Result |
|---|---:|---|
| Fast CI | run `37634050711`, Linux fast job `112835471115` | PASS |
| Full CI | run `37634050501` | PASS |
| Linux stable; identity guard; migration/workspace checks; release fault-seam proof | job `112835491548` | PASS |
| Linux MSRV 1.85.0 | job `112835491326` | PASS |
| macOS Intel x86_64 | job `112835491289` | PASS |
| macOS Apple Silicon arm64 | job `112835491288` | PASS |
| Release fault-seam exclusion | Linux stable job `112835491548` | PASS |
| Cross-architecture SQLite | run `37634050459` | PASS |
| Intel producer / arm64 producer | jobs `112835469344` / `112835469033` | PASS |
| arm64 artifact consumed on Intel / Intel artifact consumed on arm64 | jobs `112835985826` / `112836626412` | PASS |

Migration validation remains unchanged: migration 0001 checksum is
`d9068dccbc26ececb71be79c475080633166ba0163c62b2d98b9733512baefea`;
migration 0002 checksum is
`4924e69150bbff9c39e2e6b7e2bdd61045202e504900fe0f510d513fbf815e67`;
migration 0003 is absent. The merged migration tests assert both checksums,
fresh database migration 0001→0002, close/reopen, schema version 2, and Store
integrity validation. Full CI passed those all-target/all-feature tests.
Cross-architecture producers require a single closed main database file and
consumers run semantic validation. No `-shm` or live-WAL portability claim is
made.

The passing Fast and Full CI runs include the workspace smoke and identity
guards; Full CI also passed documentation, formatting, metadata, all-target
and all-feature checks, and the release fault-seam proof. The P3 branch remains
available for audit/reference.
