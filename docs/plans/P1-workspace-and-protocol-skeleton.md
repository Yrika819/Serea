# P1 — Workspace and Protocol Skeleton

> **Superpowers implementation plan**
> **Spec:** [`docs/architecture/03-crate-map.md`](../architecture/03-crate-map.md)
> **Normative contracts:** [`docs/protocols/00-protocol-index.md`](../protocols/00-protocol-index.md) and all eleven linked protocol documents
> **Architecture version:** `serea-arch/0.1.0`
> **Status:** Planned; no implementation results are claimed here.

## Goal and scope

Create a buildable Rust workspace foundation and the first protocol-contract slice: shared ID newtypes, frozen enums and common types, error types, checked-in JSON Schema 2020-12 contracts for the initial shared messages, empty provider ports, contract tests, and CI checks. P1 establishes compile-time and serialization boundaries; it does not deliver orchestration or any functioning external integration.

The P1 boundary is the bottom of the dependency graph. It does not implement SQLite storage, SQLite WAL configuration, migrations, event persistence, the task engine, policy evaluation, approval ledger, model routing, memory persistence/retrieval, scheduler, device link, OS credential store, Android app, or production provider clients. Storage and migrations are P2 and later. The schema and shared types must not invent protocol variants or alter any frozen shape.

## Constraints

- Keep architecture version `serea-arch/0.1.0`; do not revise protocols or add identifiers, enum variants, event kinds, or capability verbs.
- Serea Core remains the only orchestrator. Define ports at the protocol layer and provide no route around the capability boundary.
- `ActionRequest` is structured. Host-resolved fields remain host-owned; models have no authority.
- Use only synthetic fixtures, including reserved `.test` domains and scripted provider outputs. No real external services, network credentials, test account data, or API keys.
- `codex_allowed=false` in every fixture and configuration default. No test, mock, fallback, or CI job invokes Codex.
- Never connect a real GoalLatch or Local MCP runtime. Do not add `local_mcp` dependencies, inspect GoalLatch storage, or create a GoalLatch provider/fake in P1. The fake is explicitly deferred to P15; no Core path or runtime integration is in scope here.
- No credential bytes in types, fixtures, snapshots, errors, logs, or schemas. Credentials are represented only as `CredentialHandle`; `Secret<T>` is not serializable, cloneable, or formattable.
- Tests must be offline, deterministic, and repeatable. CI has no secret-dependent jobs.
- The protocol documents are normative. Generated/code schemas must be checked against them; do not copy prose protocol definitions into implementation comments as a competing contract.

## Exact P1 file map

Create only the following implementation files in this phase. The list is the complete P1 code/CI file map; later-layer files are out of scope. The root `Cargo.toml` is created first as a minimal virtual workspace with no members; the standalone bootstrap smoke test is included below in this file map so it can run before any crate exists.

```text
Cargo.toml
Cargo.lock
tests/workspace_smoke.py
rust-toolchain.toml
.clippy.toml
.github/workflows/ci.yml
crates/serea-protocol/Cargo.toml
crates/serea-protocol/src/lib.rs
crates/serea-protocol/src/ids.rs
crates/serea-protocol/src/types.rs
crates/serea-protocol/src/errors.rs
crates/serea-protocol/src/provider.rs
crates/serea-protocol/src/schema.rs
crates/serea-protocol/schemas/envelope.schema.json
crates/serea-protocol/schemas/action-request.schema.json
crates/serea-protocol/schemas/action-result.schema.json
crates/serea-protocol/schemas/assistant-task.schema.json
crates/serea-protocol/schemas/event.schema.json
crates/serea-protocol/tests/ids.rs
crates/serea-protocol/tests/protocol_types.rs
crates/serea-protocol/tests/schema_contracts.rs
crates/serea-testkit/Cargo.toml
crates/serea-testkit/src/lib.rs
crates/serea-testkit/src/clock.rs
crates/serea-testkit/src/models.rs
crates/serea-testkit/src/providers.rs
crates/serea-testkit/tests/fakes_are_deterministic.rs
```

The final root `Cargo.toml` is a virtual workspace manifest with exactly the two members listed above. Bootstrap begins with a minimal root manifest using `members = []`; the workspace smoke test must fail at that point, then pass after adding the minimal `serea-protocol` member. `serea-protocol` is dependency-free on other workspace crates. `serea-testkit` is used only as a dev-dependency and cannot be included in a production dependency graph. No GoalLatch provider or fake is created in P1; the fake is deferred to P15. No production Core crate is introduced in P1.

No source files are to be added under `serea-storage`, `serea-task-engine`, `serea-core`, or any network-backed provider in this phase.

## Contract slice

Implement only contract definitions already named in P0 documents:

- **IDs:** opaque newtypes and validation for `TaskId`, `StepId`, `ApprovalId`, `GrantId`, `EventId`, `DeviceId`, `ScheduleId`, `ProposalId`, `ReceiptId`, `SessionId`, `IdempotencyKey`, `CapabilityId`, `ProviderId`, `ImplementationId`, `ModelId`, and `Digest`. ULID minting may use an injected deterministic source in tests; consumers must not parse identifiers for meaning.
- **Frozen types:** `DataClass`, `RiskClass`, `SideEffectClass`, `ReplaySafety`, `Authorization`, `ActionRequest`, `ActionResult`, `ActionError`, `ActionErrorKind`, `CapabilityDescriptor`, `AssistantTask`, `TaskState`, `TaskStep`, `SereaEvent`, model request/response/capability types, `CredentialHandle`, and `Envelope<T>`. Implement only the fields required by their normative protocol shapes; where a complete implementation would require runtime behavior, define the data shape only.
- **Errors:** use the frozen `ActionErrorKind` set and typed protocol parsing/validation errors. Error display strings are diagnostic only and never control flow. Error values and tests may not contain secret bytes.
- **Schema files:** author JSON Schema 2020-12 for the five listed files, using explicit object properties, frozen identifier forms, bounded strings/arrays where specified, and closed enum sets. Match each schema to its owning protocol. Unknown fields are ignored for compatibility and preserved and round-tripped on shared envelope/wire surfaces; do not strip them during parsing or serialization. Set `additionalProperties: false` only on capability, model, or action schemas whose owning protocols explicitly require a closed schema. Do not apply closed-object validation globally to shared messages such as envelopes, tasks, or events. Unknown enum variants still fail closed as required by the Protocol Index.
- **Provider ports:** declare empty async trait interfaces at the lowest protocol layer for `CapabilityProvider`, `ModelProvider`, and the protocol-only `HostGoalProvider` contract; do not implement a GoalLatch provider/fake or include HTTP, MCP, policy, task-engine, database, or credential-byte behavior. Ports take protocol request/context types and return the corresponding protocol result/error types.
- **Test doubles:** add scripted, in-memory model and capability providers to `serea-testkit`. They return fixtures, never call real services. `FakeGoalLatchProvider` and its scenario tests are explicitly deferred to P15.

P1 does not claim the full P0 architecture is implemented. The initial schemas/types are a compiling contract skeleton; completing protocol coverage remains subject to later phase scope and protocol change control.

## TDD workflow and ordered tasks

For every behavior-bearing item, write the failing test first, observe the expected failure, implement the smallest conforming change, then rerun the focused test before advancing. Do not replace protocol tests with snapshots alone.

### Phase 1 — Workspace bootstrap

1. Create only the minimal root virtual `Cargo.toml` first, with `members = []` and `resolver = "2"`. Do not run Cargo commands before this file exists. Add no package dependencies or extra workspace members yet.
2. Write `tests/workspace_smoke.py`, a standalone Python standard-library assertion that the root workspace lists `crates/serea-protocol` and that its `Cargo.toml` exists. Run `python3 tests/workspace_smoke.py` while the root manifest still has no members; confirm the expected assertion failure because the protocol member is absent.
3. Add the minimal `serea-protocol` workspace member (`crates/serea-protocol/Cargo.toml` and `crates/serea-protocol/src/lib.rs`) and list it in the root manifest. Rerun `python3 tests/workspace_smoke.py` and confirm it passes. This is the bootstrap green gate; the smoke assertion checks for the protocol member, not that it is the only final member.
4. Pin the Rust toolchain to the current stable channel used by CI; add workspace package metadata and shared dependency versions. Add the `serea-testkit` manifest and crate root, then set final workspace membership to exactly the two P1 members. Keep runtime dependency arrows downward: protocol has no internal dependency; testkit depends on protocol; production crates never depend on testkit.
5. Add the CI workflow with offline-safe formatting, checking, tests, Clippy, a static dependency-graph check, the workspace smoke test, and documentation validation. Do not configure credentials, external-service jobs, or deployment actions.

**Entry criteria:** P0 protocol index and architecture crate map are frozen at `serea-arch/0.1.0`; the two target crates are empty of workspace code.
**Exit criteria:** `cargo metadata --no-deps --format-version 1` lists exactly the two P1 packages; `serea-protocol` has no internal dependency; `serea-testkit` appears only as a dev dependency; no real-service, credential, or GoalLatch provider/fake configuration is present; CI runs the exact commands below.

### Phase 2 — Shared IDs, types, and errors

1. In `tests/ids.rs`, specify accepted and rejected values against Protocol Index §§2–3: correct prefixes and bodies, wrong prefixes, malformed ULIDs, forbidden Crockford characters, capability IDs with other than three segments, malformed idempotency keys, model IDs, and digests.
2. Implement the ID newtypes in `src/ids.rs`, with constructors that validate and opaque accessors. Add injected ID generation only where a protocol type requires minting; deterministic tests use a fixed source and never rely on wall-clock state.
3. In `tests/protocol_types.rs`, test exact enum membership and serialization names, unknown enum rejection, snake_case fields, and representative round trips for `ActionRequest`, `AssistantTask`, and `SereaEvent`. Fixtures contain no real personal data and use `codex_allowed=false` where that setting is represented.
4. Implement the shared data types in `src/types.rs`. Keep task state and GoalLatch observed state distinct. Do not create a GoalLatch-internal type or parse the opaque goal handle.
5. Add typed parse/validation errors in `src/errors.rs`; test that each frozen `ActionErrorKind` round-trips and unknown kinds fail closed. Ensure error formatting cannot include credential bytes because no credential-bearing input type exists.

**Entry criteria:** workspace bootstrap meets Phase 1 exit criteria.
**Exit criteria:** ID grammar tests, type round-trip tests, enum-closure tests, and error tests pass; public protocol definitions compile without importing storage, engine, provider SDK, or GoalLatch dependencies.

### Phase 3 — Schemas and provider ports

1. Write failing schema-contract tests for required fields, wrong identifier prefixes, protocol-required closed-schema additional properties, unknown fields on forward-compatible shared/wire surfaces, unrecognized enum values, missing bounds, and model attempts to include host-resolved authority fields.
2. Add the five schemas under `schemas/` using the frozen protocol fields. Apply `additionalProperties: false` only where the owning capability/model/action protocol explicitly requires closure; keep shared envelope/task/event objects forward-compatible and preserve unknown fields for round-trip. Encode actual protocol constraints, and do not add `risk_class` or `provider_id` as model-populated `ActionRequest` input.
3. Implement `src/schema.rs` to expose embedded schema documents and validate synthetic values against JSON Schema 2020-12. Pin schema content to its owning protocol in test names and test data; do not create a second Rust-only schema definition that can drift silently.
4. In `tests/schema_contracts.rs`, test valid examples, invalid/unknown fields, authority-field injection, credential-shaped field rejection, and strict output validation. Verify schema documents parse as JSON.
5. Define the empty `CapabilityProvider`, `ModelProvider`, and `HostGoalProvider` ports in `src/provider.rs`. Add compile-time smoke tests using the in-memory model/capability test doubles; keep provider API details out of orchestration layers that do not yet exist.

**Entry criteria:** Phase 2 shared types and frozen ID types pass all focused tests.
**Exit criteria:** all checked-in schemas parse and validate the test vectors; the protocol crate exports the three empty provider ports; model authority-injection and credential-exclusion tests pass; no provider performs network or filesystem I/O.

### Phase 4 — Synthetic testkit

1. In `serea-testkit/tests/fakes_are_deterministic.rs`, add failing tests for scripted model responses, scripted capability results, fixed clock behavior, and identical output across repeated calls.
2. Implement only in-memory deterministic `TestClock`, `MockModelProvider`, and `MockCapabilityProvider`. Script success and typed failures, including malformed structured model output. The mock models are not routed to a real provider.
3. Add a test assertion that no model route or fixture enables Codex; keep `codex_allowed=false` explicit in defaults and any delegated-task-shaped protocol fixture. Do not implement `FakeGoalLatchProvider` or any GoalLatch runtime behavior; the fake is P15 work.

**Entry criteria:** protocol ports and schema tests pass.
**Exit criteria:** repeated mock calls under identical fixture and clock inputs return identical results; all test doubles are offline; P1 contains no GoalLatch implementation; tests demonstrate the absence of Codex and credentials from model/test inputs.

### Phase 5 — CI and P1 release gate

1. Run every exact verification command below from the repository root on the pinned stable toolchain.
2. Correct only P1-caused failures; do not alter frozen protocol behavior to make tests pass.
3. Confirm CI uses synthetic fixtures only and does not define secret-dependent or external-network tests.
4. Review dependency edges and the final file map against the crate map and user constraints.

**Entry criteria:** Phases 1–4 exit criteria are met.
**Exit criteria:** all listed commands pass in local offline mode and in the CI environment; workspace membership and the allowed file map are exact; no P1 module can execute real external or delegated host work. These are future acceptance criteria, not results claimed by this plan.

## Exact verification commands

Run the bootstrap smoke command at the indicated intermediate stages: only after the minimal root `Cargo.toml` exists, first expecting failure before the protocol member and then expecting success after that member is added. Run the remaining commands from the repository root after the full P1 file map exists. These are executable future commands, not commands run for this documentation change.

```sh
python3 tests/workspace_smoke.py
cargo fmt --all -- --check
cargo metadata --no-deps --format-version 1
cargo check --workspace --all-targets --offline
cargo test --workspace --all-targets --offline
cargo clippy --workspace --all-targets --offline -- -D warnings
python3 tools/validate_docs.py docs
```

The CI skeleton must run the same command lines on pull requests and the default branch, including `python3 tests/workspace_smoke.py`. `cargo metadata` is the workspace-shape check; the Rust commands must not require credentials or network access after dependencies are cached. If a dependency cannot be resolved offline in a clean CI cache, cache acquisition occurs as the normal dependency setup step and no test is permitted to contact a real service or GoalLatch. P1 includes only protocol data shapes and deterministic offline test doubles; task-engine behavior and storage remain P2 or later.

## Explicit deferrals

- **P2 and later:** SQLite schema/storage implementation, WAL operation, ordered migrations, transactional event persistence, blob storage, and storage recovery.
- **P2 and later:** task state machine, leases, task recovery, policy/approval implementations, memory database and FTS5, event bus, scheduler, Core composition, and device link.
- **Later provider phases:** Ollama Cloud HTTP provider, Gmail, Calendar, GitHub, Web, Android, root implementation, and real GoalLatch adapter.
- **Real GoalLatch adapter:** remains prohibited until the six live verifications in [GoalLatch Adapter Protocol §9](../protocols/08-goallatch-adapter-protocol.md#9-real-adapter-readiness-gate) are complete and an explicit integration phase authorizes it.

P1 is complete when the contracts compile, tests prove their shapes and safety exclusions, and CI enforces those checks. It does not make Serea perform tasks.
