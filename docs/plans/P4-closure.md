# P4 Closure Record

## P4B Evidence

P4B implements the durable model-accounting storage foundation. It does not
close P4 as a whole.

- Base: P4A merge commit `187fd57ea2c713facc47400ed64269c2d0388be2` on `main`.
- Branch: `p4/model-router`.
- P4B implementation commit: `71f5070064b64cd82ca087ea8d6a6834647527e4`
  (`feat: add durable model call accounting`).
- Draft PR: [#3](https://github.com/Yrika819/Serea/pull/3), open against `main`.
- Schema version: 3.
- Migration 0001 SHA-256 unchanged:
  `d9068dccbc26ececb71be79c475080633166ba0163c62b2d98b9733512baefea`.
- Migration 0002 SHA-256 unchanged:
  `4924e69150bbff9c39e2e6b7e2bdd61045202e504900fe0f510d513fbf815e67`.
- Migration 0003 (`0003_model_accounting.sql`) SHA-256:
  `8f4c5c4e047a8829201ce834ff192d740ab6ca199ecc3d12dfe8e357cf82c2ec`.

### Test-first evidence

Before migration 0003 existed, the migration tests failed because fresh and
v2-upgraded stores still reported schema version 2 instead of 3. Before the
storage APIs existed, the model-call tests failed to compile because the
attempt, usage and accounting API types and methods were absent. The
implementation was then added until those tests passed.

Focused coverage includes migration catalog and v2 upgrade, interrupted
migration rollback, STRICT/FK/integrity checks, closed enum and relation
constraints, cost rounding and overflow, reservation caps, task call and token
accounting, reopen recovery, response persistence, privacy refusal, retention,
two-connection spend/task races, terminal transition races, task deletion, and
transaction fault windows after intent insert, response blob staging, usage
insert and terminal update.

### Durable storage behavior

- `model_call_attempts` snapshots one unique RequestId per dispatch, host model
  and provider identity, deployment/data class, relation, UTC accounting day,
  price and token bounds, reservation, terminal facts and an optional response
  blob reference. The database enforces the closed attempt states and
  relationship domains, plus one active `DISPATCH_INTENT` per non-null TaskId.
- `model_usage` stores integer token counts and micro-USD cost only. It contains
  no prompt, output, content digest, device ID, conversation ID or task title.
- `tasks.model_call_count` preserves the 12-call task ceiling after 30-day
  attempt detail pruning. Every committed dispatch intent increments it in the
  same transaction as the attempt and reservation.
- Money uses non-negative SQLite-compatible integer micro-USD. Input and
  output components round up independently using checked integer arithmetic.
  FREE prices require both rates to be zero.
- UTC accounting day is the signed integer day count from
  `1970-01-01T00:00:00Z`, derived with floor division from explicit epoch
  milliseconds. No local timezone is consulted.
- Daily spend occupancy counts actual cost after settlement and the full
  reservation for unresolved, ambiguous or unknown-usage failures. Settlement
  never refunds a reservation without trustworthy usage facts.
- Accepted response content uses the existing classified blob store and a
  durable foreign-key reference. Only PUBLIC and PERSONAL response persistence
  is allowed. Task deletion clears response references and sweeps unreferenced
  content while preserving non-identifying accounting rows.
- Attempt/result detail retention is 30 days; usage detail retention is 365
  days. `DISPATCH_INTENT` rows are not pruned, and ambiguous attempts linked to
  active tasks remain available for recovery.
- The closed-database portability fixture includes ambiguous and completed
  attempts, one usage row and a response blob reference. It transfers only the
  closed database file; `-shm` is not transferred.

### Local cloud validation

All commands below passed on the cloud runner:

- `cargo check --workspace --all-targets --all-features`
- `cargo test --workspace --all-targets`
- `cargo test --workspace --all-features`
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`
- `cargo fmt --all -- --check`
- `python3 tools/validate_docs.py docs`
- `python3 tests/workspace_smoke.py`
- `python3 -m unittest discover -s tests -p workspace_smoke_tests.py`
- `cargo metadata --no-deps --format-version 1 --offline`
- `python3 tools/check_commit_identity.py`
- `git diff --check`
- Cross-architecture fixture producer and consumer modes, both successful.

### GitHub Actions on P4B implementation commit

All runs below used exact head `71f5070064b64cd82ca087ea8d6a6834647527e4`.

- [Fast CI](https://github.com/Yrika819/Serea/actions/runs/37649932169): GREEN.
- [Full CI](https://github.com/Yrika819/Serea/actions/runs/37649932213): GREEN.
  Linux stable, Linux MSRV 1.85, macOS Intel x86_64, macOS arm64 and release
  fault-seam exclusion all passed.
- [Cross-architecture SQLite portability](https://github.com/Yrika819/Serea/actions/runs/37649932326): GREEN.
  Intel-produced DB opened on arm64 and arm64-produced DB opened on Intel.

### P4B nonclaims

P4B does not implement actual routing, provider health, provider dispatch,
`ModelProvider.generate`, JSON Schema validation, duplicate-key rejection,
repair, fallback, runtime `MODEL_CALLED` event production, Task Engine
integration, a real Ollama provider, PRIVATE model calls, P4C roster/routing,
or P5. There were no model calls and no provider network code. Migration 0003
is the first P4 runtime storage change; P4 is not complete.

## Remaining P4 Slices

The accepted sequence remains P4C immutable roster/discovery/health and
deterministic selection; P4D durable dispatch, response binding and ambiguity;
P4E validation, repair, fallback and budget integration; then P4F integrated
Model Router closure. P5 remains outside P4.

## P4C Evidence (closed)

- Base: exact P4B closure HEAD `e436e73937f8e0bf62f4a8fbe712e368f766fdc6`.
- RED proof: before `src/lib.rs` existed, `cargo test -p serea-model-router
  --test routing --offline` failed to compile because the router crate API was
  absent (`unresolved import serea_model_router`).
- Focused tests: `cargo test -p serea-model-router --offline` passes 12 tests
  covering frozen chains/order, duplicate model/provider registration,
  discovery omission and identity mismatch, capability narrowing, STRICT/ANY,
  tools/context/output filters, PUBLIC/PERSONAL, PRIVATE/SECRET/CREDENTIAL
  refusal, degraded health, one health read per provider, no dispatch, price
  non-reordering, Codex exclusion, purpose/format, non-finite temperatures,
  and vision input refusal.
- Focused Clippy: `cargo clippy -p serea-model-router --all-targets
  --all-features --offline -- -D warnings` passes.
- Sequential review passes: (1) contract compliance; (2) crate graph is
  protocol plus `serde_json` only at runtime; (3) P4C has no storage/event
  transaction surface; (4) health/discovery snapshots are immutable and
  health is read once per registered provider; (5) prepared content is not
  logged or serialized and prompt-bearing prepared types do not implement
  `Debug`; (6) capability, prompt, schema and output bounds are enforced; (7)
  deterministic tests have no sleeps, wall clock, ignored cases, or platform
  skips; (8) docs and claims were reconciled. A stale P3F smoke invariant was
  updated, with a new router-layer edge guard.
- The migration checksums remain exactly the P4B values: 0001
  `d9068dccbc26ececb71be79c475080633166ba0163c62b2d98b9733512baefea`, 0002
  `4924e69150bbff9c39e2e6b7e2bdd61045202e504900fe0f510d513fbf815e67`, and
  0003 `8f4c5c4e047a8829201ce834ff192d740ab6ca199ecc3d12dfe8e357cf82c2ec`.
- Required local cloud validation passed on the P4C candidate: fmt, workspace
  check, all-target tests, all-features tests, workspace Clippy, docs
  validation, workspace smoke, all 76 smoke unit tests, Cargo metadata,
  identity guard, and `git diff --check`. Cargo emitted only the existing
  vendored `serde_json` deprecation warning for `usize::max_value`.
- Runtime dependencies: `serea-protocol`, `serde_json`; no Storage, Event Bus,
  Task Engine, Policy, Capability, or testkit runtime edge. Testkit is not
  needed by P4C routing tests.
- No `ModelProvider::generate` call exists in the routing slice. Provider
  health is read once per registered ProviderId in a deterministic snapshot;
  the current protocol method returns `ProviderHealth` directly, so adapters
  must map a failed underlying health read to `DEGRADED`.
- Prepared calls are a trusted in-process host boundary. Construction does not
  prove redaction, and the type is not suitable for an untrusted device/API
  surface. No caller-set redaction proof is present.
- Exact behavior commit CI: [Fast CI run 37655583453](https://github.com/Yrika819/Serea/actions/runs/37655583453)
  GREEN; [Full CI run 37655583531](https://github.com/Yrika819/Serea/actions/runs/37655583531)
  GREEN; [cross-architecture SQLite run 37655583318](https://github.com/Yrika819/Serea/actions/runs/37655583318)
  GREEN. Full CI passed Linux stable, Linux MSRV 1.85, macOS Intel x86_64,
  macOS arm64, and release fault-seam exclusion. Both SQLite directions passed.
- P4C closed on exact behavior commit `3b509bde05547736d4355bd71e525fe62c760be6`.
  No P4D work began before these authoritative results were GREEN.
- Nonclaims: no provider dispatch, real model service, image transport,
  structured validation, repair, fallback, or P5.
