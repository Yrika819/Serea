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
| P3 implementation branch | Pending P3A gate |
| Draft PR | Pending P3A gate |

## P3A — Contract and transaction gate

| Evidence | Result |
|---|---|
| Commit | Pending |
| Scope | ADR-0026 Option A; Event/Device replay contract; protocol registry and current architecture references; audit reconciliation |
| RED tests | Not applicable to docs/version-registry closure; existing protocol registry checks run below |
| GREEN tests | `python3 tests/workspace_smoke.py`; 74-test `workspace_smoke_tests.py`; `cargo test -p serea-protocol --offline` |
| Docs validation | `python3 tools/validate_docs.py docs` — PASS |
| Cargo metadata | `cargo metadata --no-deps --format-version 1 --offline` — PASS |
| Formatting | `cargo fmt --all -- --check` — PASS |
| Diff check | `git diff --check` — PASS |
| Fast CI | Pending |
| Full CI | Pending |
| Linux stable / MSRV 1.85 | Pending |
| GitHub-hosted macOS Intel x86_64 / arm64 | Pending / Pending |
| Release fault-seam proof | Pending |
| Sequential review | Pending |
| Nonclaims | No migration 0002, Event Bus runtime, Scheduler runtime, P3 task transitions, or runtime retention/replay behavior yet |

## P3B–P3G

Not started. Add a phase subsection with commit, RED/GREEN evidence, Cloud
validation, Fast/Full Actions run and job IDs, paired Mac results, review
findings, and nonclaims when each phase closes.

## Project nonclaims

P3 does not include Model Router, Capability Registry, Policy Engine, Approval
runtime, Memory runtime, Gmail, Calendar, Android provider execution, GoalLatch,
production credentials, or arbitrary exactly-once external effects. No claim of
universal crash/power-loss durability is made.
