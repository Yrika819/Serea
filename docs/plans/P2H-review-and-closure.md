# P2H Crash/Fault Injection — Review and Closure Ledger

## Status

**CLOSED at P2H commit `8417b1fff325e311120050f6c733f185111a90bc`.** This ledger was initially assembled immediately before that commit; this P2I update records the now-verified committed state.

P2H scope is restricted to real child-process crash windows, narrowly scoped test-only fault seams, fresh-process durable verification, N1–N8 and F25/F26. It adds no production runtime feature behavior. P2H is committed; P2I final closure is in progress on `p2/p2i-final-closure`. P3 is not started.

Starting branch: `p2/p2g-recovery`
Starting/parent HEAD: `f50fd8f0aa01ae8847c92506a61015fa586a69ec`
Implementation branch: `p2/p2h-crash-fault-injection`
P2H commit: `8417b1fff325e311120050f6c733f185111a90bc`; parent: `f50fd8f0aa01ae8847c92506a61015fa586a69ec`. No push or amend occurred.

## Baseline and scope

P2G baseline independently passed on stable and exact Rust 1.85.0:

- **801 regular + 45 doctests = 846**, zero failures, zero ignored.

P2H adds 21 crash/fault harness tests. Current final suite is:

- **822 regular + 45 doctests = 867**, zero failures, zero ignored, on stable and Rust 1.85.0.

Scope does not include an event bus, scheduler, model/provider call, capability registry, policy, approval, memory, P3 or migration/schema change. Migration 0001 checksum remains `d9068dccbc26ececb71be79c475080633166ba0163c62b2d98b9733512baefea`; vendor and Cargo.lock are unchanged.

## Harness architecture

```text
parent coordinator test process
  -> writer/crash child (`current_exe`, explicit role/mode and inherited DB dir)
  -> fresh verifier child (`current_exe`, separate role, same DB dir)
```

The parent creates the fixture and establishes preconditions. The crash child runs the selected storage/engine operation and reaches one named seam. It writes a bounded acknowledgement and blocks; the parent observes that acknowledgement and sends SIGKILL. The parent does not treat `Err` as a crash and the verifier is a distinct process that reopens the exact inherited database. Verifier expectations execute only in that fresh process.

The child parser and subprocess machinery exist only in `crates/serea-task-engine/tests/crash.rs`; production runtime crates have no subprocess/env access path. No shell, network, external service or credential path is used. All acknowledgement, verifier and probe waits are bounded. A timeout kills/reaps the child before failing. N7 also reaps its writer if progress fails. Poll sleeps only bound polling; acknowledgements/gates, not sleeps, are authority.

Fixture identity is label + sanitized binary identity + PID + per-process atomic counter. F25 exercises two distinct executable identities/processes and separately isolates the binary-name component with equal pid/counter inputs. F26 canonicalizes the temp root, sends the exact parent-created directory to the child, starts it in a unique distinct cwd, verifies the exact database path opened, and proves no second SQLite file was created.

## Test-only seam and release exclusion

`crates/serea-storage/src/fault.rs` is included behind `#[cfg(feature = "p2h-fault-injection")]`; all reach sites are similarly feature-gated. `Store` has no hook field/callback. There is no environment-controlled activation. The workspace requests the feature only from test-target dev-dependency edges in task-engine and model-router; their runtime dependency edges are unchanged. Storage also has `#![forbid(unsafe_code)]`.

Narrow seam stages are: BeforeBegin, AfterBegin, AfterTaskInsert, BeforeFenceInspection, AfterFencedStepWrite, BeforeJournalInsert, BeforeSavepointRelease, BeforeCommit and AfterCommit. Stages map to explicit N/F contracts; no arbitrary callback surface is installed.

Mechanical proof: `bash tools/prove_release_fault_exclusion.sh /tmp/p2h-relproof-final` completed exit 0. It built and inspected:

- A: default storage release — no seam source/symbol.
- B: explicit feature positive control — seam present and artifact differs.
- C: engine all-targets release test build — Cargo JSON identifies the exact `crash` integration-test executable and the executable contains the seam.
- D: default workspace release — Cargo JSON binds inspection to its storage and engine library artifacts; all four reported rmeta/rlib artifacts are seam-free.

**Scope caveat:** the proof is for the default production configurations built in A and D. B explicitly enables the optional test/development feature and intentionally contains the seam. No claim is made that an arbitrary downstream command explicitly enabling `p2h-fault-injection` produces a seam-free artifact. The feature is named test-only and enabled through the dev-dependency edge for test targets; default production configurations do not enable it. This exact boundary is recorded in Review B and is not disguised as universal configuration coverage.

The first clean isolated release proof exceeded its 10-minute command bound during dependency compilation. That run is not PASS evidence. The cache-preserving rerun of the completed target tree and the final artifact-specific proof both exited 0; there is no claimed hidden timeout success.

## N1–N8 and F25/F26 dispositions

| ID | Contract and final evidence |
| --- | --- |
| F25 | Distinct integration executable identities derive distinct deterministic temp paths; binary identity, PID and counter all participate; no wall-clock/RNG. |
| F26 | Child reopens the exact parent DB directory, via inherited path and different cwd; parent content survives, child mutation is visible in fresh verifier, and no independent temp DB appears. |
| N1 | SIGKILL before BEGIN; fresh verifier asserts schema version, no task/journal rows and valid integrity. |
| N2 | SIGKILL after BEGIN IMMEDIATE before first write; fresh verifier finds no new rows, quick_check `ok`, foreign_key_check empty. |
| N3 | SIGKILL after task insert before journal insert; fresh verifier proves transaction rollback removes both. |
| N4 | SIGKILL after fenced step UPDATE and before later outcome writes; verifier proves conservative pre-outcome state: no succeeded step, result, reference, blob, receipt, terminal outcome journal or task advance; lease authority remains. |
| N5 | SIGKILL after all target transaction writes before COMMIT; verifier proves the target transaction row sets—including step/result/ref/blob/receipt/task/journal/lease release—are absent, while prior fixture commits remain. |
| N6 | COMMIT returns; harness-only acknowledgement is recorded before caller success can be published; child is killed. Fresh verifier proves the complete state. Nonterminal fixture pins task `VERIFYING`, one succeeded step and exact result digest/ref/blob/class association. Terminal fixture pins `COMPLETED`, exactly two succeeded steps and both digest/ref/blob/class associations, two receipts and lease releases. |
| N6a | Child has no application-success marker although the committed complete state is durable. |
| N6b | Terminal recovery is strict logical no-op over full logical dump. Nonterminal recovery does not reconstruct outcome, repeat receipt/outcome journal, re-effect or report corruption; first classification audit may follow existing P2G semantics. |
| N7 | Stress-only repeated commits and sibling/controller SIGKILL. Signal is checked; every sample opens in fresh verifier, quick_check `ok`, foreign_key_check empty, per-task exactly-one insertion audit holds. Counts are reported but not pinned; neither durable category is required. It does not prove a particular kill landed inside COMMIT. |
| N8 | Fault-injection rollback test (not crash): error after fenced write and before rows_affected/result inspection; Tx rollback leaves no partial state/receipt. |
| Audit/savepoint seams | Deterministic typed failure tests verify rollback and are labeled fault-injection tests, not process-crash evidence. |

## Reviews

- [Generation 0](P2H-review-generation-0.md): initial post-GREEN findings frozen; 5 findings and 4 test gaps, no blockers.
- [Resolution](P2H-review-resolution.md): all accepted findings fixed; includes provenance caveat for the unretained initial R1/R2 raw transcript snapshots and first release-proof timeout.
- [Generation 1](P2H-review-generation-1.md): three scoped independent perspectives completed. Durability review's N6 candidate and terminal cardinality follow-up were fixed; security review PASS with the explicit-feature caveat; concurrency review PASS. Terminal recommendation PASS with caveat; no unresolved findings or test gaps in the default-build contract.

## Final validation evidence

Commands completed separately; full-suite counts are stable and exact Rust 1.85.0:

- `cargo fmt --all -- --check`: PASS.
- `cargo check --workspace --all-targets --all-features --offline`: PASS.
- `cargo test --workspace --all-targets --offline`: PASS, 822 regular tests.
- `cargo test --workspace --all-features --offline`: PASS, 822 regular + 45 doctests = 867.
- `cargo clippy --workspace --all-targets --all-features --offline -- -D warnings`: PASS.
- Exact same check/test/clippy commands with `cargo +1.85.0`: PASS.
- Storage and engine debug/release all-feature suites, stable and Rust 1.85.0: PASS.
- Crash suite stable: 21/21, repeated after final N6 changes; Rust 1.85.0: 21/21 after final changes.
- Release feature-exclusion proof: exit 0; details above.
- `python3 tests/workspace_smoke.py`: PASS.
- `python3 -m unittest discover -s tests -p workspace_smoke_tests.py`: 59 passed.
- `python3 tools/validate_docs.py docs`: PASS after final documentation edits.
- `git diff --check`, `cargo metadata --no-deps --format-version 1 --offline`, and `bash -n tools/prove_release_fault_exclusion.sh`: PASS after final edits.
- Generation-0 and generation-1 review-report validators: PASS; generation 1 reports 0 findings, 0 test gaps, 11 coverage areas, recommendation=Pass.

The x86_64-only run scope and the absence of empirical Apple Silicon validation are not broadened. This is macOS-host evidence only.

## P2H closure proof

P2H was committed once as `feat: implement P2H crash fault injection` at
`8417b1fff325e311120050f6c733f185111a90bc`, directly on
`f50fd8f0aa01ae8847c92506a61015fa586a69ec`. The P2H commit was verified with a
clean worktree before creating `p2/p2i-final-closure`. No push or amend occurred.

P2I is the current phase and remains open until Group O, whole-P2 reviews and
final closure evidence are complete. P3 implementation has not started.
