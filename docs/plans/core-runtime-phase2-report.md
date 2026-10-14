# Core Runtime Phase 2 implementation status

This branch continues PR #17 on `core/runtime-phase2`, stacked on
`platform/six-target-core-foundation`. It remains synthetic-only and uses the
canonical schema v8. Migrations 0001–0008 are unchanged. This report records a
partial implementation; it is not an independent-review readiness claim.

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

At this partial HEAD, `cargo +1.85.0 check -p serea-host --features synthetic
--all-targets --locked` passed. The existing Scheduler `calendar_runtime`
integration suite passed 6/6, covering bounded catch-up, mapped occurrence
reuse, expired-claim recovery, and RunOnce/Skip policy behavior. The exact
changed code has not yet completed the requested full workspace verification.

The Core `scheduler_wake` regression test also runs two consecutive empty
wake cycles on one disposable database to verify that the consumer fence is
released for the next pass. It does not seed a due schedule and does not prove
scheduled Task/audit attribution through the Host process.

The current child-process test still covers only start, single-instance
refusal, cooperative shutdown, process termination after readiness, relaunch,
and database reopening. It does not establish deterministic barriers at the six
requested durable boundaries or assert occurrence, Task and audit rows after
each relaunch. Process termination is not a physical power-loss test.

The Host has no trusted synthetic ModelProvider configuration or active
ModelRouter request execution interface. Provider selection, successful and
failed fake results, timeout/cancellation, budget/policy integration and
ambiguous-dispatch process E2E remain unimplemented. The existing security
regression test `synthetic_core_has_no_model_or_capability_invocation_edge`
rejects any `serea-model-router` reference in either the Core or Host manifest
or source. Integrating the requested router while preserving this guard needs
an Owner-reviewed boundary decision and a separately designed trusted
interface; this campaign did not weaken or bypass the guard. Existing Model
Router unit/crash tests do not prove Host integration.

Platform signal/console handling and resource benchmarks (idle CPU/RSS,
scheduler latency/throughput, shutdown and recovery duration) remain
unmeasured. The one-second wake cadence is a configured behavior, not a
measured latency result. No measurements are inferred for other architectures.

## Publication and CI

PR #17 remains an open Draft targeting
`platform/six-target-core-foundation`; no other PR is changed or merged. The
prior PR #17 HEAD `00e23630f0dc3637cdca11266413c179c67c13d1` passed the 12-job
Stable/Rust 1.85.0 native matrix and Fast CI, Full CI, and SQLite portability.
Those results do not apply to this changed HEAD. Final workflow run IDs and
final HEAD will be recorded only after the new commits and workflows finish.

No live provider, credential, real account, external network path, production
Memory, Capability invocation, or P8 effect is enabled. Process-kill tests do
not establish exactly-once external effects or physical power-loss durability.
