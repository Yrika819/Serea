# P5 Closure Record

## P5A integration

- PR #4 (`p5/preimplementation-audit`) was integrated with a merge commit.
- Merge commit: `86ef2f47993bd59921470dbabe8a42d8702aec6a`.
- Parent 1: `528806b0117c6ff385a79a7baaed6ab508527fe6`.
- Parent 2: `dc39bbb92ee22b0c0504f5bf82a4f2990fafce56`.
- The exact merge commit became `main` and passed Fast CI, Full CI (Linux stable, MSRV 1.85, Intel, arm64, release fault proof), identity guard, and cross-architecture SQLite portability.
- P5B branch `p5/capability-registry` was based on that merge commit. Its draft PR is opened after the first implementation commit.

## P5B implementation checkpoint

- Migration: `0004_capability_registry.sql`; schema version 4.
- Migration checksums: 0001 `d9068dccbc26ececb71be79c475080633166ba0163c62b2d98b9733512baefea`; 0002 `4924e69150bbff9c39e2e6b7e2bdd61045202e504900fe0f510d513fbf815e67`; 0003 `530a6d6cb5ec9c757311d48e10a62ef456d9d01c09512f321cffe42fe3307f80`; 0004 `140d25ba62c90b7406e59dcf5d4f146be9b62b9ff8b4bb9310eaefacf4d2932d`.
- `0001`, `0002`, and `0003` were not modified. Existing Tasks keep a NULL registry generation after upgrade.
- `serea-capability` runtime dependencies are limited to protocol, storage, and event-bus. `serea-testkit` is dev-only. Storage has no Event Bus dependency.
- Generation IDs use SQLite `INTEGER PRIMARY KEY AUTOINCREMENT` under the Store write transaction. Active authority is a singleton pointer, not `MAX(generation_id)`.
- Descriptor revisions use the host semantic digest, preserve all descriptor facts and schema URI/digest references, and are immutable. Generation membership records explicit integer candidate priority and one host-selected default version for each CapabilityId.
- Missing overlay rows mean `ENABLED`, experimental opt-in false, revision 0. Overlay changes use expected revisions. New bindings are blocked for disabled/removed capabilities and for experimental descriptors without opt-in.
- Task generation pins remain nullable and are only set through an explicit storage operation. Step bindings preserve TaskId, StepId, generation, revision, capability, provider, and implementation and are immutable.
- Registry activation and overlay state changes append metadata-only `CAPABILITY_REGISTRY_CHANGED` events inside the same SQLite transaction as the state change.
- The cross-architecture fixture now carries active and prepared generations, descriptor revision, membership/default, overlay, pinned Task, binding, and representative existing scheduler/event/model state. Local producer-to-consumer round trip passed on this cloud Linux executor. Intel-to-arm64 and arm64-to-Intel jobs remain GitHub Actions validation.

## TDD evidence

RED evidence captured before production support:

- Migration tests: `cargo test -p serea-storage migration_0004_tests` first failed because schema version/latest migration were 3 and the catalog lacked 0004; v3 upgrade stayed at v3; interrupted 0004 and strict registry table tests failed while the migration was absent.
- Event protocol test: `cargo test -p serea-protocol --test p5b_registry_event` failed because `CAPABILITY_REGISTRY_CHANGED` was absent from EventKind.
- Registry API tests: `cargo test -p serea-storage capability_registry_tests` failed to compile because generation, descriptor revision, membership/default, and overlay types and operations did not exist.

Focused GREEN evidence after implementation:

- `cargo test -p serea-storage migration_0004_tests` — 7 passed.
- `cargo test -p serea-storage capability_registry_tests` — 9 passed.
- `cargo test -p serea-capability --test registry_transactions` — 10 passed, including independent Store connections, rollback injection, overlay conflict, and restart recovery.
- `cargo test -p serea-protocol --test p5b_registry_event` — 1 passed.
- `cargo test -p serea-protocol pro_event_3_event_kind_includes_the_sixty_one_frozen_values` — 1 passed.
- Query-plan checks cover active generation, revision lookup, membership/candidate order, default version, overlay, Task pin, and Step binding; only the membership priority index is additional to primary-key indexes.
- Closed-file producer and consumer runs of `sqlite_portability` both passed locally. No `-shm` transfer or live-WAL portability claim is made.

## Sequential review record

1. **Migration/schema:** one new STRICT migration, v3-to-v4 is nullable/no-backfill, interrupted migration rolls back, integrity and FK checks pass, earlier SQL checksums are fixed. No P5-only host namespace restriction is encoded in the schema.
2. **Registry identity/immutability:** digest is the revision key; activated generation, revisions, membership/defaults, and bindings cannot be rewritten. Active pointer advances explicitly.
3. **FK/pinning:** Task pin is RESTRICT and activated-only; binding validates the Task/Step pair, pin, generation member, and descriptor facts. Bound Step identity cannot change. No implicit generation fill is present.
4. **Transaction/event atomicity:** the capability facade composes Store and EventBus using the fixed transaction participant pattern. Injected append failure rolls back activation/overlay changes.
5. **Concurrency/recovery:** independent connections serialize generation allocation/activation; overlay expected-revision conflicts do not overwrite state. Prepared and committed state are distinguished after reopen.
6. **Crate graph/authority:** `serea-capability` has only protocol/storage/event-bus runtime edges; it does not depend on policy, task-engine, model-router, core, or testkit at runtime.
7. **Privacy/content:** registry events contain only stable IDs/digests/revisions/change kinds. No task arguments, prompts, credentials, schema bytes, or provider output are persisted by P5B.
8. **Tests/docs/nonclaims:** migration/API/fault/concurrency tests, docs validator, workspace smoke, and its Python regressions pass. Nonclaims remain listed below.

## Validation and CI

- Local validation completed on cloud Linux: `cargo fmt --all -- --check`; `cargo check --workspace --all-targets --all-features`; `cargo test --workspace --all-targets`; `cargo test --workspace --all-features`; `cargo clippy --workspace --all-targets --all-features -- -D warnings`; docs validation; workspace smoke; workspace smoke unit tests; Cargo metadata; commit identity guard; and `git diff --check`.
- GitHub Actions for the P5B exact implementation commit: pending.
- No P5B commit SHA is recorded until validation and commit complete.

## Nonclaims

P5B does not implement manifest matching, schema compilation or URI resolution, model tool projection, ToolCallProposal parsing, PreparedAction construction, policy, approval, duplicate suppression execution, tool-call execution accounting, provider invocation, ActionResult processing, receipts/evidence processing, reconciliation, a real external provider, or Android standalone mode. P5 is not closed by this record. P5C, P6, and P8 have not started.
