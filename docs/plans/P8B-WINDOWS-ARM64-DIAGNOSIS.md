# P8B Windows ARM64 `0xC0000142` diagnosis

**Status: `NATIVE_INSTRUMENTATION_READY_UNVERIFIED` / `ROOT_CAUSE_UNCONFIRMED`.**
This report compares exact PR heads and existing Actions evidence. It contains no
new Windows execution. Source baseline: PR #36 HEAD
`bfe47522d808388f82b55c67793184726273600d`.

## Observed executions

| PR / run / job | Exact head | Stable Windows ARM64 result | Failure position | Controls |
|---|---|---|---:|---|
| #34 / `39036558000` / `117407503289` | `f3804a643e6334bc4f1dddfea1a0c096c1fd7a13` | All-feature test stage failed in `model_accounting_is_bound_to_a_task_created_by_the_scheduler_task_engine`; child exit decimal `3221225794` / signed `-1073741502` / `0xC0000142`; stdout/stderr each 0 bytes | 9 | Rust 1.85.0 ARM64 job `117407503223` passed; Windows x64 jobs passed |
| #36 / `39054682137` / `117460408311` | `bfe47522d808388f82b55c67793184726273600d` | Same test and same exit/status/empty streams | 0 | Rust 1.85.0 ARM64 job `117460408219` and Stable Windows x64 job `117460408021` passed |
| #33 / `39030546016` / `117389941104` | `a8210bca8177b9fbc523f7c8561287b59ccd522b` | Stable ARM64 all-feature suite passed, including all 12 Host lifecycle tests | — | Successful same-workflow control |
| #35 / `39044878224` / `117431513197` | `b9a0ef070bbec396f107ccb2b5a43925b0e22ed0` | Stable ARM64 passed | — | Successful same-workflow control |

The matching numeric status is `STATUS_DLL_INIT_FAILED`. The `Command::output()`
call returned a child status; this is not a failure to create the process handle.
The zero streams and location in `model-smoke` startup are consistent with the
child failing before it reaches normal Rust command handling. They do not name
the failing DLL or the failing Windows API.

## Code and runner context

`.github/workflows/native-platforms.yml` uses hosted `windows-11-arm`. In the
Stable ARM64 job it runs the default workspace tests, then the all-features
workspace tests, and only after those succeed runs release and dedicated Host
lifecycle checks. Both failures happened in the all-features stage; later
steps were skipped. The failing test at
`crates/serea-host/tests/host_lifecycle.rs:647-674` launches as many as twelve
`serea-host model-smoke --synthetic` children sequentially against the same
disposable task database. The all-feature test harness also runs independent
tests in parallel. In `crates/serea-host/src/main.rs:105-149`, `model-smoke`
uses an in-process synthetic provider; no real model/provider subprocess is
started.

The failure moves from child index 9 on PR #34 to index 0 on PR #36. That is compatible with transient
startup/resource state during parallel process churn, but it does not establish
resource pressure. Stable ARM64 is the only failing configuration in the
observed #34/#36 matrices; MSRV ARM64 and Stable x64 passed. Runner IDs differ,
and hosted runner images are ephemeral. No resource samples, module list,
Application Error/WER fault module, ETW image-load trace, or loader diagnostics
were present in the retrieved logs. Runner availability is limited to GitHub
Actions; there is no live ARM64 shell/session in this environment.

## Hypotheses still open

- Transient loader initialization failure during concurrent tests/process starts.
- Runner-specific image, loader, or system-resource state.
- A Windows ARM64 Stable toolchain/runtime or dependency interaction.
- External runner interference or another unobserved process condition.

These are hypotheses only. The evidence does not identify a DLL, loader call,
memory pressure, process limit, or Rust defect. The intermittent successes do
not resolve the failure.

## Bounded reproduction instructions

The accompanying `tools/windows_arm64_model_smoke_diagnostic.ps1` runs a
synthetic `serea-host model-smoke` child repeatedly on native Windows ARM64,
with a caller-provided disposable data directory (seed it with the normal
synthetic Scheduler command first). It limits iterations to 1–12 and each
child to 1–60 seconds (12 minutes maximum), records UTC, OS/process architecture, result, status,
stream byte counts and aggregate memory/process/handle/thread samples, and does
not log stream contents or environment variables. Example:

```powershell
.	ools\windows_arm64_model_smoke_diagnostic.ps1 `
  -HostExe .\target\debug\serea-host.exe `
  -DataDir $env:TEMP\serea-arm64-diagnostic `
  -TaskId <synthetic-task-id> -Iterations 12
```

The script was added on PR #36's Linux-side worktree and has **not** been run
or syntax-checked by PowerShell. It does not capture Windows event logs, WER,
or ETW. Follow-up execution should first run the exact test alone with
`--test-threads=1`, then the existing all-feature parallel suite. If a failure
recurs, preserve only bounded, time-scoped Application Error/WER fields
(faulting image/module and exception code) and a short process/image-load ETW
trace. Do not upload full environment variables, profiles, unrestricted event
messages, or runner directories. A passing attempt remains a non-reproduction,
not a root-cause fix.

No retries, sleeps, skipped tests, parallelism weakening, or Native job changes
were made. Next native status remains `ROOT_CAUSE_UNCONFIRMED` until the status
is reproduced and a loader/module mechanism is directly evidenced.
