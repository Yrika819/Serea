# P5 Closure Record

## P5A integration

- PR #4 (`p5/preimplementation-audit`) was integrated with a merge commit.
- Merge commit: `86ef2f47993bd59921470dbabe8a42d8702aec6a`.
- Parent 1: `528806b0117c6ff385a79a7baaed6ab508527fe6`.
- Parent 2: `dc39bbb92ee22b0c0504f5bf82a4f2990fafce56`.
- The exact merge commit became `main` and passed Fast CI, Full CI (Linux stable, MSRV 1.85, Intel, arm64, release fault proof), identity guard, and cross-architecture SQLite portability.
- P5B branch `p5/capability-registry` was based on that merge commit. Draft PR #5, “P5: Capability Registry and Tool Router,” is open against `main`.

## P5B implementation checkpoint

- Implementation commits: `e8fb8f78e6feec8a76b1e4a238c48aa74eb575a7` and `cb0b298f4d5f486df1bee4318924925f87376e64`.
- Exact behavior commit `cb0b298f4d5f486df1bee4318924925f87376e64` passed the required GitHub Actions matrix; PR #5 remains draft/open.
- Migration: `0004_capability_registry.sql`; schema version 4.
- Migration checksums: 0001 `d9068dccbc26ececb71be79c475080633166ba0163c62b2d98b9733512baefea`; 0002 `4924e69150bbff9c39e2e6b7e2bdd61045202e504900fe0f510d513fbf815e67`; 0003 `530a6d6cb5ec9c757311d48e10a62ef456d9d01c09512f321cffe42fe3307f80`; 0004 `b60000371f3c10d64adc7bb54b5e5144fd6ac6ec34246c072aaf68d77861d93a`.
- `0001`, `0002`, and `0003` were not modified. Existing Tasks keep a NULL registry generation after upgrade.
- `serea-capability` runtime dependencies are limited to protocol, storage, and event-bus. `serea-testkit` is dev-only. Storage has no Event Bus dependency.
- Generation IDs use SQLite `INTEGER PRIMARY KEY AUTOINCREMENT` under the Store write transaction. Active authority is a singleton pointer, not `MAX(generation_id)`.
- Descriptor revisions use the host semantic digest, preserve all descriptor facts and schema URI/digest references, and are immutable. Generation membership records explicit integer candidate priority and one host-selected default version for each CapabilityId.
- Missing overlay rows mean `ENABLED`, experimental opt-in false, revision 0. Overlay changes use expected revisions. New bindings are blocked for disabled/removed capabilities and for experimental descriptors without opt-in.
- Task generation pins remain nullable and are only set through an explicit storage operation. Step bindings preserve TaskId, StepId, generation, revision, capability, provider, and implementation and are immutable.
- Registry activation and overlay state changes append metadata-only `CAPABILITY_REGISTRY_CHANGED` events inside the same SQLite transaction as the state change.
- The cross-architecture fixture carries active and prepared generations, descriptor revision, membership/default, overlay, pinned Task, binding, and representative existing scheduler/event/model state. Local producer-to-consumer round trip passed on cloud Linux. GitHub Actions passed Intel producer to arm64 consumer and arm64 producer to Intel consumer. No `-shm` transfer or live-WAL portability claim is made.
- P5 does not implement registry-history garbage collection. Registry generations and revisions are retained; no TTL is claimed.

## TDD evidence

RED evidence captured before production support:

- Migration tests: `cargo test -p serea-storage migration_0004_tests` first failed because schema version/latest migration were 3 and the catalog lacked 0004; v3 upgrade stayed at v3; interrupted 0004 and strict registry table tests failed while the migration was absent.
- Event protocol test: `cargo test -p serea-protocol --test p5b_registry_event` failed because `CAPABILITY_REGISTRY_CHANGED` was absent from EventKind.
- Registry API tests: `cargo test -p serea-storage capability_registry_tests` failed to compile because generation, descriptor revision, membership/default, and overlay types and operations did not exist.
- Step identity regression: `cargo test -p serea-storage binding_refuses_step_capability_version_and_provider_mismatches` first returned a successful binding for a Step whose capability_id disagreed with the descriptor. The migration trigger now checks capability_id, version, and provider_id against the stored Step and preserves those identity fields after binding.

Focused GREEN evidence after implementation:

- `cargo test -p serea-storage migration_0004_tests` — 7 passed.
- `cargo test -p serea-storage capability_registry_tests` — 10 passed.
- `cargo test -p serea-capability --test registry_transactions` — 10 passed, including independent Store connections, rollback injection, overlay conflict, and restart recovery.
- `cargo test -p serea-protocol --test p5b_registry_event` — 1 passed.
- `cargo test -p serea-protocol pro_event_3_event_kind_includes_the_sixty_one_frozen_values` — 1 passed.
- Query-plan checks cover active generation, revision lookup, membership/candidate order, default version, overlay, Task pin, and Step binding; only the membership priority index is additional to primary-key indexes.
- Closed-file producer and consumer runs of `sqlite_portability` both passed locally. No `-shm` transfer or live-WAL portability claim is made.
- Fault tests cover deterministic SQLite transaction failure and caller loss/reopen. No hardware power-loss claim is made.

## Sequential review record

1. **Migration/schema:** one new STRICT migration, v3-to-v4 is nullable/no-backfill, interrupted migration rolls back, integrity and FK checks pass, earlier SQL checksums are fixed. No P5-only host namespace restriction is encoded in the schema.
2. **Registry identity/immutability:** digest is the revision key; activated generation, revisions, membership/defaults, and bindings cannot be rewritten. Active pointer advances explicitly.
3. **FK/pinning:** Task pin is RESTRICT and activated-only; binding validates the Task/Step pair, pin, generation member, Step capability/version/provider, and descriptor facts. Bound Step identity cannot change. No implicit generation fill is present.
4. **Transaction/event atomicity:** the capability facade composes Store and EventBus using the fixed transaction participant pattern. Injected append failure rolls back activation/overlay changes.
5. **Concurrency/recovery:** independent connections serialize generation allocation/activation; overlay expected-revision conflicts do not overwrite state. Prepared and committed state are distinguished after reopen.
6. **Crate graph/authority:** `serea-capability` has only protocol/storage/event-bus runtime edges; it does not depend on policy, task-engine, model-router, core, or testkit at runtime.
7. **Privacy/content:** registry events contain only stable IDs/digests/revisions/change kinds. No task arguments, prompts, credentials, schema bytes, or provider output are persisted by P5B.
8. **Tests/docs/nonclaims:** migration/API/fault/concurrency tests, docs validator, workspace smoke, and its Python regressions pass. Nonclaims remain listed below.

## Validation and CI

- Local validation completed on cloud Linux: `cargo fmt --all -- --check`; `cargo check --workspace --all-targets --all-features`; `cargo test --workspace --all-targets`; `cargo test --workspace --all-features`; `cargo clippy --workspace --all-targets --all-features -- -D warnings`; docs validation; workspace smoke; workspace smoke unit tests; Cargo metadata; commit identity guard; and `git diff --check`.
- GitHub Actions for exact behavior commit `cb0b298f4d5f486df1bee4318924925f87376e64`: Fast CI GREEN (run 127, including identity guard); Full CI GREEN (run 116); Linux stable GREEN; MSRV 1.85 GREEN; macOS Intel x86_64 GREEN; macOS arm64 GREEN; release fault proof GREEN; cross-architecture SQLite GREEN in both directions (run 94).

## Nonclaims

P5B does not implement manifest matching, schema compilation or URI resolution, model tool projection, ToolCallProposal parsing, PreparedAction construction, policy, approval, duplicate suppression execution, tool-call execution accounting, provider invocation, ActionResult processing, receipts/evidence processing, reconciliation, a real external provider, or Android standalone mode. P5 is not closed by this record. P5C, P6, and P8 have not started.

## P5C implementation checkpoint

- Starting P5B HEAD: `2067cd367e8dd79dd9f75455c1fc36d13ab51245`.
- Migration: unchanged. Schema version remains 4. Migration checksums are unchanged: 0001 `d9068dccbc26ececb71be79c475080633166ba0163c62b2d98b9733512baefea`; 0002 `4924e69150bbff9c39e2e6b7e2bdd61045202e504900fe0f510d513fbf815e67`; 0003 `530a6d6cb5ec9c757311d48e10a62ef456d9d01c09512f321cffe42fe3307f80`; 0004 `b60000371f3c10d64adc7bb54b5e5144fd6ac6ec34246c072aaf68d77861d93a`. No migration 0005 exists.
- Architecture: `serea-arch/2.6.0`; ADR-0034, ADR-0035 and ADR-0036 unchanged.
- New P5C source: `schema_catalog.rs`, `digests.rs`, `manifest.rs`, `provider.rs`, `availability.rs`, `install.rs`.
- `serea-capability` runtime dependencies are protocol, storage, event-bus, serde, serde_json, sha2 and jsonschema (`0.58.3`, `default-features = false`, so no HTTP, file or async resolver). `serea-testkit` and `async-trait` remain dev-only. No runtime async runtime is added: provider health is sampled with a noop-waker poll helper that adds no reactor and performs no provider IO.

### Schema catalog

- `CapabilitySchemaCatalogV1` is host-owned and immutable after successful construction. It accepts only exact document URIs under `https://serea.local/schemas/`; query strings, fragments, userinfo, alternate schemes or hosts, backslashes, percent-encoded forms and parent traversal are refused. Nothing is fetched to decide whether a URI is valid.
- `$ref` accepts a same-document JSON Pointer, an exact catalog URI, or an exact catalog URI plus a JSON Pointer fragment. `http://`, foreign hosts, `file://`, filesystem paths, relative paths and unknown catalog documents are refused as external or unknown references.
- Documents are parsed once with a duplicate-name-rejecting deserializer, so a repeated object member cannot silently change which value is digested or compiled.
- Canonical bytes: parse once, recursively sort object keys by UTF-8 byte order, serialize compact UTF-8. Identical bytes for any input whitespace or key order, and JSON number syntax is preserved rather than restricted to the SCJ-1 integer domain. Those bytes are the input to the byte limit and to the schema digest.
- Structural limits (ADR-0035): canonical bytes per document `<= 65_536`; nesting depth `<= 64`; schema nodes per document `<= 4_096`; properties per object `<= 256`. Overflow is a typed refusal with no truncation.
- Object schemas must carry `additionalProperties: false`; `patternProperties` is refused anywhere; string-capable schemas require a finite `maxLength`; array-capable schemas require a finite `maxItems`. Checks apply through `properties`, `items`, `$defs`, `definitions`, `dependentSchemas`, `prefixItems`, `contains`, `not`, `if`/`then`/`else` and the combinators. Ambiguity fails closed: a node is treated as object-, string- or array-capable from its `type` when present, otherwise from the keywords that imply that shape.
- Reference graphs are explicit. Same-document cycles are detected over ref-site containment, cross-document cycles over the document graph, and both are refused at catalog construction. Acyclic repeated references are accepted.
- `oneOf` is accepted only when every branch pair is mechanically provably disjoint: disjoint JSON types, or a common required discriminator property with pairwise distinct `const` values, after resolving `$ref`. Unprovable overlap is refused, and a branch reachable only through `$ref` is still checked.
- Every document is compiled with `jsonschema` Draft 2020-12 against an in-memory registry of catalog bytes only. A document can pass JSON Schema syntax and still be refused by the stricter Serea's structural compiler.
- `validator(uri)` exposes a narrow compiled-catalog handle for P5D argument validation. It is not a ToolCallProposal parser.

### Digests

- Schema digest: `sha256:` + 64 lowercase hex over the canonical schema bytes, host-computed.
- Catalog digest: domain-separated projection `{"kind":"serea.capability-schema-catalog/1","documents":[{"uri":…,"digest":…}]}` with documents sorted by URI UTF-8 byte order, hashed the same way. Independent of insertion order and of map iteration.
- Descriptor semantic digest: domain-separated projection `serea.capability-descriptor/1` over every authority-bearing fact — CapabilityId, SemVer, ProviderId, optional ImplementationId, title, description, input schema URI and computed input schema digest, output schema URI and computed output schema digest, SideEffectClass, RiskClass, Authorization, ReplaySafety, DataClass, RootRequirement, IdempotencySupport, `max_duration_ms`, CostClass and experimental. Candidate priority is generation metadata and is deliberately excluded. Debug output, memory layout and unordered maps are never hashed, and a provider-supplied digest is never trusted.
- Manifest digest: `serea.capability-manifest/1` over the catalog digest, entries sorted by CapabilityId, SemVer exact text, implementation presence then value, ProviderId and candidate priority, and defaults sorted by CapabilityId. Entry permutation, provider order and descriptor order do not affect it.
- Golden vector: the `calendar.events.read` descriptor digest is pinned in `tests/p5c_digests.rs` as `sha256:00feb5c8aaf595afa8115dfeda9b7501b6191687c1001899351e20c84bfa66d5`.

### Manifest

- `CapabilityManifestV1` is built in Rust only; no deserializer or wire constructor exists, so neither model output nor provider advertisement can construct one.
- Construction is atomic and fails closed for duplicate logical identity, duplicate candidate priority in one capability/version, `ImplementationId = None` mixed with a named implementation, more than one `None` implementation, a default version not represented by an entry, duplicate default, provider namespace mismatch, ProviderId `host`, a CapabilityId beginning `host.`, a descriptor semantic digest mismatch, a schema URI absent from the catalog, an input or output schema digest mismatch, and any uncompiled schema.
- The manifest records the schema catalog digest so a generation can prove which schemas it was built against.

### Provider matching

- `ProviderRegistry` owns `Arc<dyn CapabilityProvider>` values, sorted by ProviderId, with unique ProviderId and no ordinary `host` provider.
- `validate_advertisements` requires each advertised descriptor's ProviderId to match the advertising provider, its identity to exist in the manifest, and its full descriptor facts to equal the manifest entry. An unmanifested advertisement is refused, a same-identity fact change is refused, an entry a provider does not advertise only makes that candidate unavailable, and an entirely absent provider is allowed.
- The manifest wins every semantic comparison. A provider cannot add a revision, select a version, priority or implementation, replace a schema, change risk, authorization, data-class ceiling or replay safety, or enable experimental.

### Availability and resolution

- `CapabilityAvailabilitySnapshotV1` is one immutable per-operation freeze: exactly one `health()` read and at most one `capabilities()` read per registered provider, the live durable overlay for every manifest capability, and the caller-supplied `HostEligibility`.
- Advertisements are revalidated against the frozen manifest while the snapshot is built; a mismatch or unmanifested advertisement fails the operation with a typed provider-contract error and freezes nothing. Durable descriptor semantics are never updated from an advertisement.
- `HostEligibility` is a trusted caller-supplied map keyed by descriptor digest. A missing entry is not eligible. P5C does not probe root, device capability or platform, and never infers eligibility from an implementation name.
- Resolution uses the exact persisted manifest default version, never max SemVer, latest, provider order or a model choice. Candidates are ordered by candidate priority with a deterministic descriptor tie-break as corruption defence. The first candidate satisfying live overlay enabled, experimental opt-in where required, provider `READY`, exact current advertisement and host eligibility wins. Otherwise the outcome is typed: `Unknown` for a CapabilityId absent from the generation, `Unavailable` for a known capability with no usable implementation, and `ContractFailure` for corrupt persisted facts or a violated invariant. No fallback to another version exists.
- Overlay `DISABLED` and `REMOVED`, and experimental descriptors without opt-in, all yield unavailable. The overlay is read once per snapshot and never mutated during resolution; a later overlay change does not affect a frozen snapshot, and the next snapshot observes it.
- `install` validates the catalog and manifest first, then provider advertisements, and only then prepares and activates a new generation through the existing P5B transaction and `CAPABILITY_REGISTRY_CHANGED` event path. A validation failure writes nothing. A same-manifest restart reuses the active generation and emits no second activation event. A changed manifest creates a new generation while the old one stays readable. Prepared-but-inactive generations are never authoritative: the active pointer is the only authority, never `MAX(generation_id)`. If the active generation claims the manifest digests but its member facts disagree, install fails closed with a typed corruption error instead of rebuilding or repairing authority.

### Zero-invoke proof

No production path in `serea-capability` calls `CapabilityProvider::invoke`; a search of the crate's production source for `invoke` returns nothing. `tests/p5c_availability.rs` asserts the scripted provider's invocation counter is zero across manifest validation, generation installation, advertisement matching, health sampling and availability resolution, and the scripted provider panics if invoked. P8 remains the first phase permitted to invoke a provider.

## P5C TDD evidence

RED evidence captured before each subsystem existed:

- `cargo test -p serea-capability --test p5c_schema_catalog` first failed to compile because `CapabilitySchemaCatalogV1` and `CatalogError` did not exist.
- `cargo test -p serea-capability --test p5c_oneof` failed to compile with the same unresolved imports before the oneOf proof existed.
- `cargo test -p serea-capability --test p5c_digests` failed to compile because the descriptor and manifest digest functions did not exist.
- `cargo test -p serea-capability --test p5c_manifest` failed to compile because `CapabilityManifestV1` and `ManifestError` did not exist.
- `cargo test -p serea-capability --test p5c_provider` failed to compile because `ProviderRegistry` and `ProviderError` did not exist.
- `cargo test -p serea-capability --test p5c_install` failed to compile because the install entry point did not exist.
- Defects found by the first GREEN run and fixed: cross-document `$ref` cycles were not detected until a document-level graph was added; a same-document cycle was invisible because containment edges must relate a ref site to the ref sites inside its target; the ref-site containment pass was quadratic in document size and slowed the catalog suite to 46 s, and now runs over ref sites only at 0.04 s; the `oneOf` root node was refused as an open object until object-, string- and array-capability were derived from implying keywords when `type` is absent; and a `serde_json` parse would have kept the last of two identical member names, so a duplicate-name-rejecting deserializer was added.

Focused GREEN evidence:

- `cargo test -p serea-capability --test p5c_schema_catalog` — 22 passed, including the 65,536-byte boundary, the depth boundary, the 4,096-node boundary, the 256-property boundary, cycle refusal, ref-form refusals, closed-object, bounded-string and bounded-array refusals, and duplicate member name refusal.
- `cargo test -p serea-capability --test p5c_oneof` — 6 passed, including the branch hidden only behind `$ref`.
- `cargo test -p serea-capability --test p5c_digests` — 10 passed, including the golden vector and one changed field per authority fact.
- `cargo test -p serea-capability --test p5c_manifest` — 14 passed.
- `cargo test -p serea-capability --test p5c_provider` — 12 passed.
- `cargo test -p serea-capability --test p5c_availability` — 14 passed, including one health read per provider per snapshot, health change between snapshots, degraded unavailability, rootless/rooted candidate ordering driven by host eligibility, overlay disabled/removed, experimental opt-in, snapshot immutability, and the zero-invoke counter.
- `cargo test -p serea-capability --test p5c_install` — 7 passed, including fresh install, reopen, same-manifest restart with no second event, changed manifest with a new generation, a prepared generation that never becomes authoritative, a validation failure that writes nothing, and a corruption refusal.
- `cargo test -p serea-capability` — 95 passed in total, including the 10 pre-existing P5B registry transaction tests.
- `cargo fmt --all -- --check`, `cargo check -p serea-capability`, and `cargo clippy -p serea-capability --all-targets --all-features -- -D warnings` pass.
- `python3 tools/validate_docs.py docs`, `python3 tests/workspace_smoke.py`, `python3 tools/check_commit_identity.py` and `git diff --check` pass.

## P5C sequential review record

1. **Catalog/ref security:** only trusted-namespace exact URIs resolve; every forbidden ref form is refused by tests; no HTTP, file, redirect, DNS or arbitrary URL resolution exists and no resolver feature of `jsonschema` is enabled; duplicate member names cannot change a digest identity.
2. **Structural compiler:** limits are enforced over the whole document, closed objects, bounded strings and arrays are required wherever the shape can produce them, cyclic references are refused before instance validation, and `oneOf` fails closed on unprovable overlap.
3. **Digest determinism:** schema, catalog, descriptor and manifest digests are domain-separated, order-independent permutations are tested, the golden descriptor vector is pinned, and priority is shown not to affect the descriptor digest while it does affect the manifest digest.
4. **Manifest authority/matching:** the manifest is Rust-constructed only, refuses every listed inconsistency, and provider advertisement can only confirm or withdraw.
5. **Provider health/advertisement snapshots:** one health read and at most one advertisement read per provider per snapshot, advertisements revalidated against the frozen manifest, health able to affect availability only.
6. **Version/implementation selection:** exact persisted default version, priority-ordered candidates, eligibility supplied by the trusted caller, and no provider-order or model influence.
7. **Restart/corruption/failure:** same-manifest restart is idempotent with no second event, changed manifests create a new generation, prepared generations are never authoritative, and disagreeing persisted member facts fail closed.
8. **Crate graph/privacy/nonclaims/tests:** runtime edges are protocol, storage, event-bus, serde, serde_json, sha2 and jsonschema only; the crate depends on no policy, task-engine, model-router, core or testkit at runtime; nothing P5C persists contains arguments, prompts, credentials or schema bytes; no provider is invoked.

## P5C GitHub Actions validation

- P5C behaviour commit: `c98cb62`. MSRV clippy fix commit: `0cbbf05`.
- Fast CI run 37864342530: GREEN on `0cbbf05`, including the commit identity guard.
- Full CI run 37864342745: GREEN on `0cbbf05` — Linux stable full validation GREEN, MSRV 1.85.0 GREEN, macOS Intel x86_64 GREEN, macOS arm64 GREEN, P2H crash/fault suite GREEN on both macOS jobs, release fault-seam exclusion proof GREEN, commit identity guard and guard tests GREEN.
- Cross-architecture SQLite portability run 37864342695: GREEN on `0cbbf05`, Intel producer to arm64 consumer and arm64 producer to Intel consumer. No `-shm` transfer and no live-WAL portability claim is made.
- An earlier Full CI run on `c98cb62` failed only on MSRV clippy, which flagged an elidable lifetime in `schema_catalog.rs` under the 1.85 lint set. `0cbbf05` fixes it; no behaviour changed.
- PR #5 remains OPEN, DRAFT and MERGEABLE, and is NOT merged.

## P5C nonclaims

P5C does not implement ToolDefinitionV1 projection, ToolCallProposalV1 parsing, the MODEL_SCHEMA_VIOLATION runtime path, ClassifiedArgumentsV1, PreparedActionV1, TaskEngine generation pinning, Step creation integration, policy, approval, duplicate suppression, repeated-action accounting, tool-call accounting, provider invocation, ActionResult processing, receipt or evidence processing, reconciliation, Gmail, Calendar, an Android provider, Android standalone mode, or GoalLatch.

## P5D implementation checkpoint

- New P5D source: `strict_json.rs`, `proposal.rs`, `tool_definition.rs`, `preparation.rs`, `violation.rs`. `serea-event-bus` gains one content-free event draft, `draft_model_schema_violation`.
- No new migration. Schema version remains 4 with the same four checksums. No architecture change beyond ADR-0034/0035/0036.
- `serea-capability` runtime dependencies are unchanged: protocol, storage, event-bus, serde, serde_json, sha2, jsonschema. `serea-testkit`, `async-trait` and `serde_json` (test ergonomics) are dev-only.

### Tool definition projection

- `ToolDefinitionV1` carries exactly `version`, `capability_id`, `title`, `description` and `input_schema`. Provider, implementation, capability version, risk, side effect, authorization, replay safety, root, idempotency, credential and policy facts are absent by construction, and a test asserts the rendered projection never contains them.
- Definitions sort by CapabilityId UTF-8 byte order.
- Visibility requires a resolvable candidate in the frozen snapshot, which already applies the current live overlay, experimental opt-in, provider health, exact advertisement and host eligibility. Policy and approval are deliberately not consulted: visibility grants no authority. A capability with no usable candidate is simply not projected rather than being an error.

### Tool call proposal

- `ToolCallProposalV1` parses exactly `version: "1"`, `capability_id` and an object-root `arguments`. The parser is a duplicate-name-rejecting deserializer, so a repeated member name at any depth refuses the whole proposal.
- Any member outside the three declared names refuses the whole proposal. Nothing is stripped and parsing never continues with a partial result. Every host-resolved field named by Capability Protocol §4.2 and ADR-0035 has an explicit negative test: request_id, task_id, step_id, capability_version, provider_id, implementation_id, risk_class, side_effect_class, required_authorization, replay_safety, arguments_digest, idempotency_key, data_class, requested_by, deadline_ms, approval, policy, credential_handle, descriptor_digest, descriptor, generation_id, risk and authorization.
- `ActionRequest` is never deserialized from model JSON. The proposal type exposes only the three declared facts.
- Rejections are whole-proposal refusals carrying a stable code, the offending member NAMES and a count. Values, arguments, the raw proposal and prompts never appear in a rejection or an event.

### MODEL_SCHEMA_VIOLATION

- The frozen `EventKind::ModelSchemaViolation` now has a content-free draft. The payload carries the common model attempt metadata, the optional StepId, the stable violation code, the offending member names and their count, and nothing else.
- Names are bounded and filtered twice: once when the rejection is built, and again in the event bus, which is the last point where model-derived text could reach a durable record. A hostile 500-byte member name is dropped rather than echoed.
- Model activity events still refuse any class above PERSONAL, so a higher-class proposal cannot be recorded as an activity event at all. If the append fails, the proposal still produces no prepared action.

### Classification and preparation

- `ClassifiedArgumentsV1` wraps arguments with the trusted class of their source. The only constructor takes that class explicitly; there is no model-derived path and no field-name heuristic that could prove safety. Unknown provenance is expressed as `CREDENTIAL`, the fail-closed default.
- `prepare_action` resolves the capability through the frozen snapshot, refuses `CREDENTIAL` arguments, refuses an argument class above the descriptor's reviewed ceiling, validates arguments against the descriptor's trusted input schema, canonicalizes with SCJ-1, derives the arguments digest and the exact IDK-1, computes the effective deadline, and pins every authority fact.
- The effective deadline is the minimum of the descriptor's `max_duration_ms`, any stricter caller deadline and the remaining Task budget. A zero result is refused; nothing overflows and no model input participates.
- Arguments that cannot be represented canonically fail closed. A fractional number inside the schema's accepted domain is refused as `ARGUMENTS_NOT_CANONICAL` rather than rounded or reformatted.
- `PreparedActionV1` carries TaskId, StepId, generation digest, descriptor revision digest, capability, version, provider, optional implementation, validated arguments, arguments digest, IDK-1, the exact trusted argument class, RequestedBy, the effective deadline, and the pinned side-effect, risk, authorization, replay, root, idempotency and cost facts. It has no RequestId, no approval state, no PolicyDecision and no execution permission: the type offers no accessor for any of them.
- The argument class recorded is the exact trusted class, never the ceiling by coincidence and never lowered. `RequestedBy` is provenance and does not change the logical idempotency meaning: the same task, step and arguments derive the same IDK-1 regardless of requester.

## P5D TDD evidence

RED evidence captured before each subsystem existed:

- `cargo test -p serea-capability --test p5d_tool_definitions` failed to compile because `ToolDefinitionV1` and `provider_tools` did not exist.
- `cargo test -p serea-capability --test p5d_proposal` failed to compile because `ToolCallProposalV1`, `ProposalRejection` and `parse_tool_call_proposal` did not exist.
- `cargo test -p serea-capability --test p5d_preparation` failed to compile because `ClassifiedArgumentsV1`, `PreparedActionV1`, `PreparationError` and `prepare_action` did not exist.
- `cargo test -p serea-capability --test p5d_violation` failed to compile because `ModelSchemaViolation` and `record_schema_violation` did not exist.

Defects found by the first GREEN runs and fixed:

- The tool projection originally failed the whole projection when one capability had no candidate; a hidden or unavailable capability must simply not be projected.
- The shared strict parser was first written privately inside the catalog module; the proposal path needs the same duplicate-name rejection, so it became `strict_json.rs` and both paths now use one implementation.
- The first preparation signature tried to recover the capability from the arguments wrapper, which is not where a proposal's capability lives; the host-resolved CapabilityId is now an explicit argument.
- `SCJ-1` refusal was originally mapped to a generic error; it is now its own typed `ArgumentsNotCanonical` outcome so a non-representable argument set can never be confused with a schema violation.

Focused GREEN evidence:

- `cargo test -p serea-capability --test p5d_tool_definitions` — 7 passed.
- `cargo test -p serea-capability --test p5d_proposal` — 13 passed, including 24 authority-field injections and the sanitization of rejection diagnostics.
- `cargo test -p serea-capability --test p5d_preparation` — 14 passed.
- `cargo test -p serea-capability --test p5d_violation` — 7 passed.
- `cargo test -p serea-capability` — 126 passed in total, covering P5B, P5C and P5D.
- `cargo test -p serea-event-bus` — 13 passed.
- `cargo clippy -p serea-capability -p serea-event-bus --all-targets --all-features -- -D warnings`, the MSRV 1.85 workspace clippy with `--locked --offline`, `cargo fmt --all -- --check`, docs validation, workspace smoke, the commit identity guard and `git diff --check` all pass.
- Zero-invoke is preserved: the scripted providers in every P5D test panic if invoked, and no production path in `serea-capability` calls `CapabilityProvider::invoke`.

## P5D GitHub Actions validation

- P5D behaviour commit: `99e74b4`.
- Fast CI run 37908936160: GREEN, including the commit identity guard.
- Full CI run 37908936109: GREEN — Linux stable full validation GREEN, MSRV 1.85.0 GREEN, macOS Intel x86_64 GREEN, macOS arm64 GREEN, with the P2H crash/fault suite, release fault-seam exclusion proof and identity guard all GREEN.
- Cross-architecture SQLite portability run 37908936165: GREEN in both directions.
- PR #5 remains OPEN, DRAFT and MERGEABLE, and is NOT merged.

## P5D nonclaims

P5D does not implement TaskEngine generation pinning, Step creation integration, policy, approval or grants, duplicate suppression, repeated-action accounting, tool-call accounting, provider invocation, ActionResult processing, receipt or evidence processing, reconciliation, Gmail, Calendar, an Android provider, Android standalone mode, or GoalLatch. Preparation is a P6 input, not an approval: no PolicyDecision, no approval state and no execution permission exists in P5.

## P5E implementation checkpoint

- New P5E source: `serea-task-engine` gains `create_task_with_capability_pinning` and `create_capability_step`. `serea-storage` gains one narrow typed operation, `Tx::append_capability_step`, plus a `has_registry_generations` read.
- No new migration. Schema version remains 4 with the same four checksums.
- Crate direction: P5E freezes the single new edge `serea-task-engine -> serea-capability`. The reverse edge is now forbidden by both the workspace smoke test and the M20 closure test, which were updated from the pre-P5 rule that forbade any such edge.

### Generation pinning

- `create_task` pins the active generation in the same transaction as the Task row, and distinguishes three durable cases: a post-P5 database with an active generation pins it; a post-P5 database whose active generation is missing fails closed with `NoActiveCapabilityGeneration`; a genuinely pre-P5 database that has never held a generation keeps the accepted legacy NULL pin.
- `create_task_with_capability_pinning` is the strict constructor: it always requires an active generation and can assert the caller's expected one.
- The pin and the Task row become durable together or not at all, so a Task is never transiently usable for capability planning without a generation and is never silently attached to a later "current" value.
- A legacy NULL-pinned Task may continue non-capability work but fails closed with a typed outcome when a capability Step is attempted; storage independently refuses the bind with `RegistryTaskUnpinned`.

### Capability Step creation and binding

- `create_capability_step` resolves the capability through the frozen availability snapshot, prepares the arguments through the P5D trusted path, and writes the Step row and its immutable binding in one storage transaction. A half-created capability Step is therefore not observable.
- The narrow storage operation re-checks the pinned generation, the descriptor revision, the live overlay and the experimental opt-in before writing, refuses a Step that already exists or is already bound, writes the input blob and plan-revision document through the existing blob path, and then binds. The P5B trigger still independently verifies that the binding facts match the Step row, the Task pin and the generation membership.
- A duplicate Step is refused and never rebound; the existing binding is unchanged.
- Disable or remove after binding leaves the binding intact: an already-bound Step keeps its exact revision through every later change. Disable, remove, degraded provider and ineligible host each refuse only NEW bindings.
- Hot update: a Task created before a new generation activates stays on its own generation, and a Task created after uses the new one.

### Storage boundary judgment

Making the Step row and the binding atomic needed one new storage method. `put_plan_revision` is the only pre-existing step-write path and requires the caller to supply the entire plan, while `StepSnapshot` does not carry `input_json`, so an append could not be expressed through it without either losing existing inputs or rewriting the plan. The added operation is narrow and typed — one Step plus one already-resolved descriptor digest, reusing the existing plan input validation, blob and revision machinery — and is not a generic SQL seam, a mutable descriptor update or a registry delete. No schema change was involved.

## P5E TDD evidence

RED evidence captured before the integration existed:

- `cargo test -p serea-task-engine --test p5e_capability_binding` failed to compile because neither engine operation, the new engine errors nor the store handle for an independent connection existed.

Defects found by the first GREEN runs and fixed:

- The first binding attempt inserted the binding before the Step row, which the P5B trigger correctly refused; the operation now writes the Step row first and binds after, while still validating every gate before any write.
- The first append advanced `plan_revision` without a `plan_revisions` row, which `load_history` correctly detects as corruption; the operation now writes the plan-revision document and blob references through the ordinary path.
- `EngineError` could no longer derive `Copy` once it carried a `CapabilityId`, so one existing recovery test that used `.copied()` was updated to `.cloned()`.
- The M20 closure test and the workspace smoke test both encoded the pre-P5 rule that the Task Engine may not depend on the capability crate; both were updated to the frozen P5E direction while keeping the reverse edge forbidden.

Focused GREEN evidence:

- `cargo test -p serea-task-engine --test p5e_capability_binding` — 11 passed, covering pinning at creation, fail-closed creation without an active generation, restart persistence, a legacy NULL-pinned Task, exact binding to the pinned generation descriptor, disable-before-binding, remove-before-binding, provider degraded before binding, an existing binding surviving a later disable, hot update across two generations, duplicate-step refusal, and the zero-invoke proof.
- `cargo test -p serea-task-engine` — 147 passed across all suites.
- `cargo test -p serea-storage` — 472 passed.
- `cargo test -p serea-scheduler` — 45 passed, so scheduler Task creation is unaffected.
- Clippy for capability, task-engine and storage with `--all-targets --all-features -D warnings`, the MSRV 1.85 workspace clippy with `--locked --offline`, `cargo fmt --all -- --check`, docs validation, workspace smoke and its 76 unit tests, the commit identity guard and `git diff --check` all pass.
- Zero-invoke holds: every P5E provider panics if invoked, and no P5 production path calls `CapabilityProvider::invoke`.

## P5E nonclaims

P5E does not implement policy, approval or grants, duplicate suppression, repeated-action accounting, tool-call accounting, provider invocation, ActionResult processing, receipt or evidence processing, reconciliation, Gmail, Calendar, an Android provider, Android standalone mode, or GoalLatch. Binding a Step is not approval and not execution authority.
