# Core Runtime Phase 2 implementation status

This branch continues PR #17 on `core/runtime-phase2`, stacked on
`platform/six-target-core-foundation`. It remains synthetic-only and uses the
canonical schema v8. Migrations 0001–0008 are unchanged. This report records a
partial implementation; it is not an independent-review readiness claim. The
Owner-approved trust boundary permits only the non-default Host `synthetic`
feature to use the existing Model Router with an in-process testkit fake.
Ordinary Core/default Host remain model-execution-free; production providers,
network access, credentials, and Capability invocation remain prohibited.

## Host and Scheduler path

The opt-in `run --synthetic --data-dir PATH` Host retains its explicit data
directory, cross-platform exclusive lock, JSON lifecycle diagnostics,
cooperative stop command, synchronous SQLite work, startup recovery and parked
wait. A stale stop marker is removed only after acquiring the instance lock.
The idle wait is one second and wakes the scheduler without spinning.

Startup calls `Core::recover`, which uses the existing Scheduler recovery path.
Each subsequent Host wake calls `Core::process_scheduler_wake`: it claims the
fenced Scheduler Event Bus consumer, replays one bounded page, dispatches up to
64 materialized Host Event occurrences, processes bounded due calendar work,
and releases the consumer lease after successful completion. Scheduler
occurrence claims, TaskEngine task construction, TaskCreated and
ScheduleTaskCreated audit persistence remain in the existing Scheduler and
TaskEngine transaction contracts. The Host does not insert Task rows directly
or invoke capabilities. Startup JSON reports aggregate recovered task counts;
per-wake JSON reports created-or-reused counts without task content.

## Verification and remaining gaps

On this working change, Rust 1.85.0 workspace all-target and all-feature tests,
all-target/all-feature check, release workspace build, all-feature Clippy with
`-D warnings`, formatting, workspace smoke, API boundary, identity and docs
guards passed. The Host lifecycle suite passed 3/3 with both Rust 1.85.0 and
Stable. The revised security guard and Scheduler `calendar_runtime` suite
passed (6/6), covering bounded catch-up, mapped occurrence reuse, expired-claim
recovery, and RunOnce/Skip policy behavior. These local results do not replace
the final-head GitHub Actions matrix.

The Host harness invokes an explicit synthetic `scheduler-seed` command that
uses a development-only Registry fixture and the existing
`Scheduler::create_schedule` API to create a PUBLIC RunOnce calendar schedule.
It starts the compiled Host child against that database, waits for the durable
recovery/wake diagnostic, stops it, and independently inspects SQLite. The
test checks exactly one `TaskCreated` followed by one `ScheduleTaskCreated`,
matching Task correlation, Schedule ID and occurrence key, Scheduled Task kind
and origin, and the Task's pinned generation against the active fixture
generation. A fresh Host restart leaves TaskCreated count at one. This proves
one due-occurrence and duplicate recovery path through the actual executable;
it does not cover every crash barrier, timezone edge, or missed policy.

The Core `scheduler_wake` regression test also runs two consecutive empty
wake cycles on one disposable database to verify that the consumer fence is
released for the next pass.

The existing lifecycle child test still covers start, single-instance
refusal, cooperative shutdown, termination after readiness, relaunch, and
database reopening. The added Scheduler child test proves one full durable
occurrence-to-Task path and duplicate recovery; the model child test proves
success/failure terminal audit state. Neither establishes deterministic
barriers at the requested Scheduler/Model crash boundaries. Process
termination is not a physical power-loss test.

The Owner-approved guard correction is in
`crates/serea-task-engine/tests/crash.rs`, test
`model_execution_is_confined_to_explicit_synthetic_host_feature`. It replaces
the former blanket test `synthetic_core_has_no_model_or_capability_invocation_edge`,
at `crates/serea-task-engine/tests/crash.rs` (previously around line 1535),
which searched Core/Host manifests and entry-point source for the forbidden
strings `serea-model-router`, `ModelProvider::generate`,
`CapabilityProvider::invoke`, `reqwest`, and `std::net::`. Its purpose was to
keep Core/Host synthetic execution away from models, capabilities, and network
transports. The revised test
keeps Core free of Router references, requires default Host features to be
empty, requires optional Router/testkit dependencies behind `synthetic`, keeps
`main.rs` free of Router calls, and checks the dedicated adapter for the fake
provider, Router execution, network adapters, credentials, and Capability
invocation. The test runs Cargo dependency-tree checks for default Core/default
Host and the opt-in Host feature. It confirms default Host omits
`serea-model-router` and `serea-testkit`; `--features synthetic` includes them.
`--all-features` intentionally enables that test surface and is not the
default/release exclusion proof. The test runs in Full CI's `serea-task-engine`
crash integration suite.

The Host now has an explicit `model-smoke --synthetic --data-dir PATH`
development command. It builds a PUBLIC-only request through
`PreparedModelCallV1::from_host`, uses a zero-cost immutable local roster, a
zero spend cap, an offline dispatch gate, and `MockModelProvider` with a
deterministic echo script. It invokes the existing
`ModelRouterProcessV1::execute` while SQLite remains synchronous. A child
process test reopens the same v8 database and verifies durable `MODEL_CALLED`
and `MODEL_COMPLETED` audit events and no unresolved attempt. An explicit
`--scenario failure` path returns a typed definite provider failure, records
`MODEL_FAILED`, verifies exactly one fake call and no unresolved attempt, and
uses a separate disposable database. These commands are opt-in, have no Task
correlation, and normal Host startup never calls them or submits
Scheduler-created Tasks to a model.

Host-boundary tests for provider identity mismatch, timeout, cancellation,
budget/token limits, crash after intent, unresolved-call recovery, conservative
ambiguity, and no automatic redispatch remain unimplemented. Existing Router
crate tests do not substitute for these Host scenarios.

Platform signal/console handling and Scheduler throughput/recovery measurements
remain outstanding. A Rust 1.85.0 Linux x64 Release Host was measured against a
disposable empty database: idle RSS 11,668 KiB; idle CPU 1.00% of one core over
five seconds; ready in 48.85 ms; stop-command-inclusive shutdown in 65.82 ms.
The native measurement sampled `/proc/<pid>/status` and `/proc/<pid>/stat` at
100 clock ticks/second, then used the existing stop command. These are single
run observations, not stable benchmarks. Scheduler wake latency and throughput
were not measured; the one-second wake cadence is configuration, not a
measured latency result. No estimates are made for other architectures. The
earlier Rust 1.85.0 debug observation was 12,972 KiB RSS, 5.2% CPU, 193.68 ms
startup, and 265.95 ms shutdown under its separate one-off conditions.

## Publication and CI

PR #17 remains an open Draft targeting
`platform/six-target-core-foundation`; no other PR is changed or merged. The
prior PR #17 HEAD `c6eebe449e5c0ab60db3ac970348a94926a5c396` passed the 12-job
Stable/Rust 1.85.0 native matrix and Fast CI, Full CI, and SQLite portability.
Those results do not apply to this changed HEAD. Final workflow run IDs and
final HEAD will be recorded only after the new commits and workflows finish.

No live provider, credential, real account, external network path, production
Memory, Capability invocation, or P8 effect is enabled. Process-kill tests do
not establish exactly-once external effects or physical power-loss durability.
