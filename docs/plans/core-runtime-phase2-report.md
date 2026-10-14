# Core Runtime Phase 2 implementation status

This branch adds an opt-in, synthetic-only long-running Host entry point while
preserving the existing default `doctor`, `status`, and `smoke --synthetic`
commands. The run mode requires an explicit data directory, opens schema v8,
runs the existing Core recovery path at startup, emits content-free JSON
lifecycle records, and waits without busy polling. `stop --synthetic` requests
bounded cooperative shutdown through a stop file. An OS-released exclusive file
lock prevents concurrent Host instances and is recoverable after process
termination. SQLite work stays synchronous.

The child-process lifecycle test starts the compiled Host, checks single
instance protection, requests graceful shutdown, terminates a second Host
process, and starts a fresh process against the same disposable database. The
test verifies startup recovery succeeds, the final Host closes, and the
database remains available. This demonstrates process-kill/relaunch after
startup recovery; it does not simulate physical power loss.

## Remaining acceptance gaps

This branch is not the complete Runtime Phase 2 milestone and must not be
reported as ready for independent review. In particular:

- Host wake processing does not yet drive due Scheduler occurrences through a
  long-running controlled execution loop. Existing Scheduler recovery is used
  only at startup.
- The child-process test does not persist and assert Task/audit attribution at
  multiple crash boundaries, and immediate recovery from a crash while a
  Scheduler occurrence lease is pending is not demonstrated.
- Host-owned Fake Model Router dispatch, timeout/cancellation and ambiguous
  result recovery are not integrated.
- Host termination is cooperative through the stop command. Platform signal
  handlers are not installed.
- Resource measurements, full Clippy/release/all-feature verification and the
  six-platform Stable plus Rust 1.85.0 GitHub Actions matrix remain to be
  collected for the final commit.

No P8 capability invocation or live model provider is enabled. Schema v8 and
migrations 0001–0008 are unchanged. No physical power-loss durability claim is
made.
