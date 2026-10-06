# Contributing

## Development setup

Use Rust stable and MSRV 1.85.0. Install `rustfmt` and `clippy`; Python 3 is used by repository checks. Fetch locked dependencies before running Cargo offline:

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

## Change expectations

- Use test-driven development for behavior-bearing changes: add or update a regression test before changing behavior.
- Preserve the frozen phase and authority boundaries. In particular, do not start P3 work, add provider/device integrations, or introduce credentials without explicit phase authorization.
- Never commit credentials, private data, local machine paths, or runtime state.
- Keep tests deterministic and offline after dependency fetch. Do not make tests contact external services.
- Keep changes focused and document evidence and limitations without overstating guarantees.
