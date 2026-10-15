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

Platform signal/console handling remains outstanding. A Rust 1.85.0 Linux x64
Release Host was measured against disposable databases: idle RSS 11,708 KiB;
idle CPU 1.00% of one core over five seconds; ready in 44.67 ms; one due
calendar Task recovered in 30.43 ms from process start; one fake model command
completed in 30.23 ms; shutdown took 115.70 ms after issuing the stop command.
The native measurement sampled `/proc/<pid>/status` and `/proc/<pid>/stat` at
100 clock ticks/second. These single-run timings are not stable benchmarks;
the model and task observations are one-call/one-task latencies, not throughput
tests. No estimates are made for other architectures. The earlier Rust 1.85.0
debug observation was 12,972 KiB RSS, 5.2% CPU, 193.68 ms startup, and 265.95
ms shutdown under its separate one-off conditions.

## Publication and CI

PR #17 remains an open Draft targeting
`platform/six-target-core-foundation`; no other PR is changed or merged. The
implementation and measurements at HEAD
`1eb46831687ee78b0a26ff4223893ae271ad7da8` passed these GitHub Actions runs:

- Fast CI: [38558661854](https://github.com/Yrika819/Serea/actions/runs/38558661854)
- Full CI: [38558662048](https://github.com/Yrika819/Serea/actions/runs/38558662048)
- Cross-architecture SQLite portability:
  [38558661903](https://github.com/Yrika819/Serea/actions/runs/38558661903)
- Native Stable and Rust 1.85.0 matrix, 12/12 jobs:
  [38558661856](https://github.com/Yrika819/Serea/actions/runs/38558661856)

The subsequent report-only commit carries no implementation changes. The
Draft PR description records the exact branch HEAD and final-head workflow run
IDs so the verification target remains explicit.

No live provider, credential, real account, external network path, production
Memory, Capability invocation, or P8 effect is enabled. Process-kill tests do
not establish exactly-once external effects or physical power-loss durability.

## Durable recovery follow-up

This follow-up starts from PR #17 HEAD
`4868489a943201837b389658821084a27911892a`. The synthetic Host startup now
keeps the existing order `Core::recover` (Scheduler and Task repair), then
`recover_unresolved_model_calls`, and only then emits `host_ready`. The latter
uses the existing Model Router recovery function with the canonical Store,
the Host EventBus, and injected Clock. It does not construct a Router or
register/call a provider. A recovery error propagates to the existing generic
`host_error` response and startup exits before readiness; there is no warning
and continue path. Core/Scheduler recovery and Model Router recovery use
separate bounded synchronous Store transactions, with no async wait while a
SQLite transaction is open.

The new compiled-executable child test seeds one valid PUBLIC `DISPATCH_INTENT`
in a disposable v8 database, starts the real Host, and independently reads the
database after startup. It verifies AMBIGUOUS state, original RequestId,
preserved spend reservation, null actual cost, no model_usage row, and exactly
one ModelFailed audit event. A second actual Host process startup leaves the
state and audit count unchanged. The fixture seeds the durable intent directly
through Store, so this proves Host startup recovery and repeat idempotence; it
does not prove a kill at each provider execution boundary or the full
Router-created `MODEL_CALLED` to `MODEL_FAILED` sequence in this Host test.
Existing Model Router tests separately exercise actual process-death intent,
ambiguous recovery, repeated/concurrent recovery, completion recovery,
failure, cancellation and accounting behavior, but they are not the requested
Host child-process barrier matrix.

The prior security guard in
`crates/serea-task-engine/tests/crash.rs`,
`model_execution_is_confined_to_explicit_synthetic_host_feature` at line 1535.
At reviewed base SHA `c74228c`, line 1535 defined
`synthetic_core_has_no_model_or_capability_invocation_edge`. That original
guard read Core and Host manifests/source and rejected any occurrence of
`serea-model-router`, `ModelProvider::generate`, `CapabilityProvider::invoke`,
`reqwest`, or `std::net::`; its purpose was to keep synthetic execution from
reaching model, capability, or network edges. The guard was located and
preserved with narrower feature-aware semantics; it was not deleted. The
revised test checks default dependency trees, non-default synthetic activation,
ordinary Core source, and the separate fake adapter. It permits only
`synthetic_model::recover_unresolved` inside `run_synthetic_host` and rejects
`execute_once` and `ModelProvider::generate` references there. It continues to
exclude Router/testkit from default builds and network/credential/capability
references from the synthetic model adapter. Its focused test passed locally
and the full crash test ran in Full CI. The isolated-copy mutation suite is
recorded in the current follow-up below. The guard remains source-pattern and
Cargo-tree based; it is not a proof against every possible indirect call graph
mutation.

Current local evidence for this follow-up: Rust 1.85.0 workspace check,
workspace tests, all-feature tests, release build and all-feature Clippy
completed successfully; formatting, workspace smoke, identity and Core API
boundary checks passed. Stable Host lifecycle tests passed 4/4. Default Host
release and explicit synthetic Host release builds both completed. Migration
regression tests were included in the passing workspace suite, and no
migration source was changed. On implementation commit
`68d99d9f8557506de1dcfbd7f35caa4a2c269754`, GitHub Actions Fast CI
`38568006823`, Full CI `38568006964`, SQLite portability `38568006855`, and
Native platform run `38568006922` succeeded. The native matrix completed
12/12 Stable/MSRV jobs, including compiled Host child-process lifecycle on
Windows x64 and ARM64. These runs are on the implementation commit; a later
report-only commit requires its own exact-HEAD workflow verification.

At the end of the preceding recovery campaign, the remaining items included
Scheduler and Router process barriers, Host-path cancellation and accounting
coverage, mutation tests for the security guard, and a durable identifier
strategy review. The prior follow-up addressed identifier restart collision
resistance and the Scheduler post-commit kill boundary. The current follow-up
adds Model Router and Scheduler child-process barriers and security guard
mutation tests; the overall verdict remains partial.

### Identifier and Scheduler crash follow-up

The long-running Host and explicit model-smoke command now construct a
Host-owned ULID source from OS-provided entropy behind the non-default
`synthetic` feature. This replaces the prior deterministic testkit sequence
seeded from wall-clock milliseconds and PID. One EventBus source supplies both
event and model RequestIds during the exclusive Host lifetime. The 80-bit
entropy value increments per identifier, and exhaustion fails loudly. Each
process lifetime receives an independent OS-random seed; collision resistance
is probabilistic (for identifiers from two lifetimes at the same timestamp,
the seed collision probability is 2^-80 before accounting for call volume),
not a mathematical uniqueness guarantee or production identity system. OS
entropy failure aborts startup before readiness.

The compiled Host Scheduler E2E now kills and reaps the child immediately
after `host_ready`. Startup has already committed the due occurrence's Task
and audit through `Core::recover`; the test independently inspects SQLite,
restarts the Host, and verifies exactly one TaskCreated and one
ScheduleTaskCreated event with matching occurrence identity, Task origin,
registry generation pin and audit order. This covers a process kill after the
atomic Task/audit transaction and before subsequent Host diagnostics. The
existing transaction does not expose safe separate kill points inside
admission, claim and Task/audit commit; splitting it for tests would weaken the
durability envelope.

The follow-up Rust 1.85.0 synthetic Host suite passed 4/4, including the
same-database process-restart ID check and Scheduler post-commit kill/relaunch
scenario. The more detailed barrier coverage and mutation suite are recorded
below. Process-kill tests do not prove physical power-loss durability or
exactly-once external effects.

### Release resource sample

On the available native Linux x86_64 runner, the Stable optimized synthetic
Host was measured in a fresh disposable directory/database. `host_ready` took
55.42 ms from process launch, resident memory was 11,932 KiB after readiness,
and CPU use over a 5-second idle sample was 1.60% of one core. The stop-command
to process-exit interval was 115.66 ms, including creation/invocation of the
separate stop process. These are one-run observations, not targets or
cross-platform estimates. Scheduler latency/throughput and repeated fake-model
throughput were not measured in this sample; the child E2E is correctness
evidence only.

### Deterministic crash-boundary follow-up

At HEAD `6fce8ea3c1b827e46bc85d4d72b0956f11f5f935`, Fast CI 38597809280,
Full CI 38597809324, SQLite portability 38597809277 and Native platform
validation 38597809378 succeeded; the native matrix completed 12/12
Stable/MSRV jobs. The changes below follow that HEAD and require new final-HEAD
workflow runs before acceptance.

The Host adds the non-default `synthetic-test-hooks` feature, which forwards
`serea-scheduler/test-hooks`. It is not included by `default = []`; normal
default and ordinary synthetic Host builds do not compile the barrier or clock
override. The native lifecycle test command explicitly enables this feature.

The actual Host executable's model-smoke dispatch can pause immediately before
Router execution (no durable attempt or `ModelCalled`), in the fake provider's
`generate` entry after the Router has committed its durable intent and
`ModelCalled` event, after the fake response is obtained but before terminal
persistence, or after terminal persistence and before command
acknowledgement. The lifecycle harness opens the same SQLite database at each
barrier, kills and reaps the child, restarts a fresh Host, and checks the
original RequestId, state, usage and audit cardinality. At provider entry, the
attempt is `DISPATCH_INTENT`; startup marks it `AMBIGUOUS`, leaves usage absent,
and does not append a second `ModelCalled`. After terminal commit, restart
preserves completion, usage and the singular terminal event. This does not
claim exactly-once execution by an external provider.

The Scheduler test feature pauses around existing transaction boundaries:
before/after occurrence admission, after durable claim, immediately before
TaskEngine's atomic Task/audit transaction, and immediately after that
transaction. The compiled Host E2E independently inspects SQLite at each
barrier, kills/reaps the process, restarts against the same database, and
checks for one TaskCreated and one ScheduleTaskCreated. Claim recovery uses a
test-only injected Host clock advanced beyond the existing lease expiry; no
production clock or lease semantics changed. The Task and audit remain in one
existing atomic TaskEngine transaction, with no manufactured interior crash
point. Rust 1.85.0 passed both model and Scheduler barrier suites locally.

`tools/check_synthetic_host_boundary.py` adds source checks for default
dependency edges, synthetic-only Router use, trusted Host request preparation,
network/credential/Capability exclusions and no normal-loop dispatch.
`SyntheticHostSecurityMutationTests` mutates five disposable copies: default
Host Router dependency, default Core Router dependency, synthetic default
activation, dispatch from the normal Host loop, and an external network
adapter reference. All five mutations are rejected; the unmodified source
passes. Existing Rust security tests still inspect Cargo dependency trees.
These checks are defense-in-depth, not a semantic proof of every possible
indirect Rust call path.

Remaining gaps are Host-level timeout/cancellation, provider response identity
mismatch and budget/spend accounting integration tests; OS signal/console
handlers; and Scheduler/model throughput measurements. The process-kill tests
prove neither physical power-loss durability nor exactly-once external
effects.


### Host policy, accounting, termination and bounded workload follow-up

At the follow-up from `15746d684a06124efc18f95f1332f40a552d9580`, the explicit
synthetic `model-smoke` Host path gained bounded timeout handling and scripted
policy/accounting scenarios. The Host waits using a monotonic elapsed-time
boundary and drops a pending Router future at expiry. It then calls the existing
`recover_unresolved_model_calls` reconciliation. The scripted timeout test holds the provider pending for one second against a
150 ms Host timeout, leaving a wide scheduling margin on slower native runners.
A committed intent therefore
remains conservatively `AMBIGUOUS`; the Host does not retry it, refund its
reservation, invent usage, or record a fabricated cost. SQLite transactions
remain synchronous and no write transaction crosses an async wait. This is
limited to the explicitly requested synthetic probe and is not connected to
Scheduler-created Tasks or ordinary Host startup dispatch.

The compiled Host integration suite exercises pre-dispatch cancellation and
expired deadlines (no attempt/provider call), cancellation after a durable
intent (one intent and terminal refusal, zero provider calls), provider
identity mismatch, invalid returned usage bounds, definite provider failure,
paid success, daily spend refusal, and a provider pending past its effective
deadline. Tests reopen the same SQLite database and inspect durable RequestIds,
model-attempt state, reservation, settled cost, usage rows and audit event
cardinality/order. The paid success reserves 131,080 micro-USD and settles the
scripted 2 micro-USD for its two reported tokens. The timed-out attempt retains
its 131,080 micro-USD reservation, records neither actual cost nor usage, and
remains ambiguous after ordinary Host restart with no replacement RequestId or
redispatch. The spend-bound scenario refuses before provider invocation. These
are synthetic in-process provider outcomes; they do not establish behavior for
external providers or a Task-correlated production budget.

The test-only `after-result` barrier is reached after the scripted provider has
constructed its response but before `generate` returns that response to the
Router. It tests an obtained-but-unreturned result and correctly leaves the
intent unresolved on process kill. The separate `terminal` barrier is reached
after `router.execute` returns, which means Router terminal persistence and
accounting have completed, but before the Host command acknowledges success.
Restart at the former boundary yields ambiguity without invented usage;
restart at the latter preserves the existing terminal result and accounting
without duplicate audit. The barriers are only compiled by the distinct
`synthetic-test-hooks` test feature and are absent from default and ordinary
synthetic command builds.

The synthetic Host registers a termination handler only after startup recovery
and before publishing `host_ready`. On Unix the child-process test sends
SIGTERM, observes bounded cooperative shutdown and `host_stopped`, then reopens
the schema-v8 database. The optional `ctrlc` dependency is activated only by the
non-default `synthetic` feature. Its documented Windows console support covers
Ctrl-C/Ctrl-Break; interactive Windows console-control delivery was not
available for native execution in this environment, and close/logoff/service
shutdown events are not claimed. POSIX SIGTERM and Windows console events have
different delivery semantics, so only the Unix signal path has direct runtime
E2E evidence here.

A reproducible bounded Release sample was run on native Linux x86_64 with Rust
1.85.0. The Scheduler workload admitted 32 one-shot synthetic schedules and
verified 32 durable `TaskCreated` events; start-to-ready elapsed time was
74.04 ms, or 432.19 Tasks/s for this bounded startup workload. The fake-model
sample ran 16 separate compiled `model-smoke` processes, then verified 16
unique durable RequestIds, 16 completions and no unfinished attempts; the
process-inclusive rate was 103.48 calls/s. These figures include process and
database setup, are single-run observations rather than targets, and are not
estimates for other platforms or pure in-process Router throughput. They do not
measure idle CPU/RSS or provider network performance.

Security checks retain `default = []`, no Router edge from Core/default Host,
and no real provider/network or credential path. `ctrlc` and Router-related
code remain behind explicit non-default synthetic features. The workspace smoke
clock guard permits only the two monotonic `Instant::now` reads in the
synthetic timeout helper; the injected protocol Clock remains authoritative for
wall-clock policy and persisted event times. The ordinary `run --synthetic`
loop still creates Scheduler Tasks without sending them to the model adapter.
Schema v8 and migrations 0001–0008 are unchanged.

Local verification at runtime source HEAD `6b740c09e880f7a94836968b1fb2077fa2f00a6a` includes Rust 1.85 and Stable workspace tests, all-feature tests, Clippy with denied warnings, default and synthetic Release builds, and ten Host lifecycle tests including policy/accounting scenarios, Scheduler/model crash barriers, throughput samples, and Unix SIGTERM. The local Host suite passed again twice for the policy/accounting scenario after increasing the fake-provider wait margin.

Remaining limitations: the task-correlated token/model-call budget path is not
independently exercised through a real TaskEngine-created Task; cancellation
while a fake provider is already pending is represented by timeout/process
termination rather than a separate live cancellation control; Windows console
termination is compile/dependency evidence only; and this run does not provide
idle resource, physical power-loss, external side-effect, or exactly-once
measurements. The security guards are source/dependency checks plus behavioral
tests, not a formal proof over arbitrary transitive execution paths. Independent
review remains required.


#### Cross-platform timeout regression correction

The first final-HEAD native run exposed a timing-sensitive failure in the timeout
integration scenario on Rust 1.85.0 macOS ARM64: a 20 ms timeout against a 100
ms fake delay returned an outcome before the expected timeout classification
under parallel workspace load. The behavior was confined to the synthetic test
provider timing margin; no Router/storage contract failed. The fixture now waits
1,000 ms and the Host timeout is 150 ms. On the Linux runner, the full Host
lifecycle suite and two additional repetitions of the policy/accounting test all
passed after this adjustment. The original failure was corrected before final validation. On runtime source HEAD `6b740c09e880f7a94836968b1fb2077fa2f00a6a`, Fast CI run 38656957158, Full CI run 38656957280 (4/4), SQLite portability run 38656957279 (4/4), and Native platform run 38656957376 (12/12) all succeeded. The exact Actions evidence is linked from the PR #17 description. The initial failed run was 38653559344; it is retained as diagnostic history rather than counted as a pass.
