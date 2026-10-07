# Serea

Serea is an offline-first Rust project for a tightly scoped, durable task-runtime foundation. Its current implementation validates protocol data and executes durable task-engine transitions through SQLite; it does not yet connect to external providers or devices.

## Current status

- **P2 durable runtime: complete.** The protocol, SQLite storage, task engine, and deterministic testkit are implemented and P2 closure evidence is recorded in [`docs/plans/P2-closure.md`](docs/plans/P2-closure.md).
- **P3 durable Event Bus and Scheduler: closed on `p3/event-bus-scheduler`.** PR #1 is open as a draft and has not been merged to `main`; see [`docs/plans/P3-closure.md`](docs/plans/P3-closure.md) for scope, evidence, and nonclaims.
- The workspace contains six crates: `serea-protocol`, `serea-storage`, `serea-event-bus`, `serea-task-engine`, `serea-scheduler`, and dev-only `serea-testkit`.

Not implemented: Gmail runtime, Calendar provider, Android runtime, a real GoalLatch integration, a real Codex path, production credentials, or external providers. P3 Event Bus and Scheduler code is on the open branch and is not yet part of `main`.

## Architecture

The runtime dependency graph is acyclic: Event Bus depends on Storage and Protocol; Task Engine depends on Event Bus, Storage, and Protocol; Scheduler depends on Task Engine, Event Bus, Storage, and Protocol; Storage depends on Protocol. `serea-testkit` is for deterministic test doubles and is not a runtime dependency. Storage uses bundled SQLite. P2/P3 provide durable task transitions, lease fencing, recovery, event sequencing, and bounded process-crash/fault-injection evidence; they do not establish power-loss or production durability certification.

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

## License

Serea is licensed under the [MIT License](LICENSE). Third-party and vendored components remain under their respective licenses.
