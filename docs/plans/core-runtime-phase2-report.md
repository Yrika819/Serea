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

On this working change, `cargo +1.85.0 check -p serea-host --features synthetic
--all-targets --locked` passed. The targeted Host lifecycle suite passed 2/2
with Rust 1.85.0, including the actual child-process model dispatch and
database reopen. The revised security guard passed its targeted test. The
existing Scheduler `calendar_runtime`
integration suite passed 6/6, covering bounded catch-up, mapped occurrence
reuse, expired-claim recovery, and RunOnce/Skip policy behavior. The exact
changed code has not yet completed the requested full workspace verification.

The Core `scheduler_wake` regression test also runs two consecutive empty
wake cycles on one disposable database to verify that the consumer fence is
released for the next pass. It does not seed a due schedule and does not prove
scheduled Task/audit attribution through the Host process.

The Scheduler child-process test still covers only start, single-instance
refusal, cooperative shutdown, process termination after readiness, relaunch,
and database reopening. The new model success child process checks terminal
audit state, but neither test establishes deterministic barriers at the
requested Scheduler/Model durable boundaries. Scheduler occurrence-to-Task
attribution is not yet proven through the compiled Host executable. Process
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
invocation. Cargo tree inspection confirms default Host omits
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
and `MODEL_COMPLETED` audit events and no unresolved attempt. This is success
path evidence only: the command is opt-in, has no Task correlation, and normal
Host startup never calls it or submits Scheduler-created Tasks to a model.

Host-boundary tests for typed fake failure, provider identity mismatch,
timeout, cancellation, budget/token limits, crash after intent, unresolved
call recovery, conservative ambiguity, and no automatic redispatch remain
unimplemented. Existing Router crate tests do not substitute for these Host
scenarios.

Platform signal/console handling and resource benchmarks (idle CPU/RSS,
scheduler latency/throughput, shutdown and recovery duration) remain
partially measured. On Linux x64, one Rust 1.85.0 debug Host over a disposable
empty database reported 12,972 KiB RSS and 5.2% of one core over a five-second
idle sample. Start-to-ready took 193.68 ms. Shutdown took 265.95 ms including
the separate stop command. These are single native observations, not stable
benchmarks. Scheduler wake latency and bounded-work throughput have not been
measured; the one-second wake cadence is configuration, not a measured
latency result. No measurements are inferred for other architectures.

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
