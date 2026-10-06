# Serea

Serea is an offline-first Rust project for a tightly scoped, durable task-runtime foundation. Its current implementation validates protocol data and executes durable task-engine transitions through SQLite; it does not yet connect to external providers or devices.

## Current status

- **P2 durable runtime: complete.** The protocol, SQLite storage, task engine, and deterministic testkit are implemented and P2 closure evidence is recorded in [`docs/plans/P2-closure.md`](docs/plans/P2-closure.md).
- **P3: not started.** No production P3 implementation or migration exists.
- The workspace contains four crates: `serea-protocol`, `serea-storage`, `serea-task-engine`, and dev-only `serea-testkit`.

Not implemented: Gmail runtime, Calendar runtime, Android runtime, Event Bus, Scheduler, a real GoalLatch integration, a real Codex path, or real credentials/providers. Architecture and protocol documents may describe later-phase plans; those plans are not shipped functionality.

## Architecture

The runtime dependency direction is `serea-task-engine` → `serea-storage` → `serea-protocol`; the task engine also depends directly on the protocol crate. `serea-testkit` is for deterministic test doubles and is not a runtime dependency. Storage uses bundled SQLite. P2 provides durable task transitions, lease fencing, recovery, audit seams, and bounded process-crash/fault-injection evidence; it does not establish power-loss or production durability certification.

## Build and test

Requirements: Rust stable (MSRV 1.85.0) with `rustfmt` and `clippy`, Python 3, and a platform C toolchain required by bundled SQLite.

```sh
cargo fetch --locked
cargo fmt --all -- --check
cargo check --workspace --all-targets --all-features --offline
cargo test --workspace --all-targets --offline
cargo test --workspace --all-features --offline
cargo clippy --workspace --all-targets --all-features --offline -- -D warnings
python3 tests/workspace_smoke.py
python3 -m unittest discover -s tests -p workspace_smoke_tests.py
python3 tools/validate_docs.py docs
```

The offline Cargo commands assume dependencies were fetched first. To verify the declared MSRV, install Rust 1.85.0 and repeat the Cargo checks/tests/Clippy commands with `cargo +1.85.0`.

## Evidence and limitations

The P2 closure record documents the exact local validation results, crash harness scope, and nonclaims. CI results are evidence for their named hosted environments only; they do not prove behavior on every device, guarantee power-loss durability, or certify production readiness.
