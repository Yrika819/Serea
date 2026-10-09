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

## P5F implementation checkpoint

P5F adds no new runtime behaviour. It closes the phase: concurrency, crash and recovery coverage across the P5 surfaces, the typed P6 handoff boundary, and the whole-phase review.

### Concurrency

- Four independent Store connections installing the same manifest concurrently produce exactly one activation; the rest observe the already-active generation. The activation is serialized by SQLite, not by an in-memory mutex.
- Concurrent installations racing after an activation leave the active pointer on an activated generation whose digests match the validated manifest. The pointer never moves to a prepared generation.
- Availability snapshots over identical facts resolve to the same descriptor digest, so provider health sampling timing is not an authority input.

### Crash and recovery

- An injected deterministic failure at `BeforeCommit` during a second installation leaves the previously activated generation authoritative after reopen, with its manifest digest intact.
- An injected failure at `BeforeBegin`, before any preparation, writes nothing: no generation row exists afterwards.
- A caller that loses its response after a successful activation reopens to the activated generation with its members, manifest digest and catalog digest intact.
- Reopening with no provider registered at all never rebuilds authority from an advertisement: resolution fails closed while the durable generation stays readable.
- These are deterministic SQLite transaction failures at the storage fault seam. No hardware power-loss claim is made, and no `-shm` transfer or live-WAL portability claim is made.

### P6 handoff boundary

- `PreparedActionV1` is the P5 endpoint delivered to a future P6. Preparing the same inputs twice yields equal values, so a consumer cannot observe authority drift across the boundary.
- The type exposes accessors for exactly the frozen pinned facts and none for a RequestId, an approval state, a PolicyDecision, an execution permission or a credential. A test renders every accessor and asserts none of those concepts appear.
- P6 receives an immutable value and has no mutator for the descriptor revision, arguments, digest, IDK, provider, implementation, classification, requester, deadline, risk or authorization.
- P5 implements no policy, approval or grant logic at all. That is P6's work.

## Whole-P5 TDD evidence

RED evidence captured before the phase integration existed:

- `cargo test -p serea-capability --test p5f_integration` failed to compile before the P5F suite existed; the first GREEN run then surfaced and fixed the test-harness errors listed below.
- The P5F fault suite was written against the existing storage fault seam and passed on its first run, so it records no RED for the seam itself.

Defects found by the first GREEN runs and fixed:

- The concurrency harness initially shared one in-memory Store, which cannot express independent connections; the fixtures now use file-backed databases so each thread opens its own connection.
- The cross-architecture SQLite fixture re-pinned a Task generation that P5E now pins automatically, which failed on both architectures; the fixture now asserts the automatic pin, making it a stronger portability check.

Focused GREEN evidence:

- `cargo test -p serea-capability --test p5f_integration` — 8 passed.
- `cargo test -p serea-capability --test p5f_faults` — 4 passed.
- `cargo test -p serea-capability` — 148 passed, covering P5B, P5C, P5D and P5F.
- `cargo test -p serea-task-engine` — 147 passed, covering P5E and every pre-existing suite.
- `cargo test -p serea-storage` — 472 passed. `cargo test -p serea-scheduler` — 45 passed. `cargo test -p serea-event-bus` — 13 passed. `cargo test -p serea-protocol` — 197 passed across its suites. `cargo test -p serea-model-router` and `cargo test -p serea-testkit` pass with no failures.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`, the MSRV 1.85 workspace clippy with `--locked --offline`, `cargo fmt --all -- --check`, docs validation, workspace smoke and its 76 unit tests, the commit identity guard and `git diff --check` all pass.

## Whole-P5 source search gates

- `.invoke(` in P5 production source: no occurrences in `serea-capability`, `serea-task-engine`, `serea-storage` or `serea-event-bus`. Every P5 provider double panics if invoked.
- `reqwest`, `Command::` and `std::process::Command` in `serea-capability` production source: none.
- `host.goal` in `serea-capability` production source: none.
- Test fixtures and negative test strings may still contain such text; no runtime path was introduced.

## Whole-P5 security review

1. **Model authority injection.** Refused. `ToolCallProposalV1` accepts exactly three member names; twenty-four host-resolved field names each have an explicit negative test and refuse the whole proposal. Duplicate member names refuse at any depth.
2. **Provider authority widening.** Refused. Advertisements are revalidated against the frozen manifest; an unmanifested or altered descriptor fails the operation, and a provider cannot add a revision, choose a version, priority or implementation, replace a schema, or change risk, authorization, data-class ceiling or replay safety.
3. **Arbitrary shell or `host.*` registration.** None. Ordinary registration refuses ProviderId `host` and any CapabilityId beginning `host.`; the manifest is the allowlist and is Rust-constructed with no deserializer.
4. **Schema SSRF and filesystem access.** No path. Resolution is in-memory only under `https://serea.local/schemas/`, and `jsonschema` is built with `default-features = false`, so no HTTP, file or async resolver exists in the dependency.
5. **Malformed or hostile JSON Schema.** Refused. Duplicate member names, non-2020-12 drafts, open objects, `patternProperties`, unbounded strings and arrays, cyclic refs, unprovable `oneOf` overlap and every limit overflow are typed refusals with no truncation.
6. **Classification lowering.** Refused. The class is a trusted constructor argument, CREDENTIAL is refused at the boundary, a class above the descriptor ceiling is refused, and the prepared action records the exact trusted class rather than the ceiling.
7. **CREDENTIAL reachability.** Refused. `DataClass::Credential` cannot cross the classification boundary or the preparation boundary.
8. **Hot-update and restart authority switching.** Refused. A Task keeps its pinned generation for life; a new generation affects only Tasks created after it; a same-manifest restart reuses the active generation without a second activation event.
9. **Root implementation switching.** None. Host eligibility is a trusted caller-supplied map keyed by descriptor digest, a missing entry is ineligible, and nothing infers eligibility from an implementation name.
10. **Nondeterministic ordering.** Refused. Every user-visible ordering is an explicit sort or a B-tree key; permutation tests cover catalog, descriptor, manifest, provider registry, candidates, tool definitions and candidates-by-priority.
11. **Sensitive diagnostics and events.** Refused. Rejections carry stable codes, member names and counts; the event bus re-bounds those names; model activity events refuse any class above PERSONAL; no prompt, argument or model content is persisted.
12. **Accidental provider invocation.** None. Verified by source search and by provider doubles that panic on invocation.
13. **Crate and dependency inversion.** One new direction, `task-engine -> capability`, frozen in both the workspace smoke test and the M20 closure test; the reverse edge is forbidden by both.
14. **Legacy Task accidental rebinding.** Refused. A NULL-pinned Task fails closed on a capability Step, and storage independently refuses the bind.

## P5 final nonclaims

P5 does not implement policy, approvals or grants, duplicate suppression, repeated-action accounting, tool-call dispatch accounting, provider invocation, ActionResult processing, receipt or evidence processing, reconciliation, credentials, Gmail, Calendar, an Android provider, Android standalone mode, Android-only memory, GoalLatch, or any real external effect. P6 has not started. P8 has not started. No P5 code path invokes a provider, and no hardware power-loss claim, `-shm` transfer claim or live-WAL portability claim is made.

P5 registers no `host.goal.*` capability, adds no execution domain or origin-node concept, and introduces no assumption that a Mac is the only possible execution node.

## P5 final CI and closure status

Behaviour commits in phase order: P5C `c98cb62` (with the MSRV clippy fix `0cbbf05`), P5D `99e74b4`, P5E `e18fd00` (with the portability fixture fix `ac0dc00`), P5F `9c2c48a`.

Final validation on exact behaviour commit `9c2c48a`:

- Fast CI run 37917523509: GREEN, including the commit identity guard.
- Full CI run 37917523691: GREEN — Linux stable full validation GREEN, MSRV 1.85.0 GREEN, macOS Intel x86_64 GREEN, macOS arm64 GREEN, every step green including the P2H crash/fault suite, the release fault-seam exclusion proof and the identity guard.
- Cross-architecture SQLite portability run 37917523536: GREEN in both directions, Intel producer to arm64 consumer and arm64 producer to Intel consumer.
- Intermediate slices were also green on their own exact commits: P5C runs 37864342530 / 37864342745 / 37864342695, P5D runs 37908936160 / 37908936109 / 37908936165, P5E runs 37915553099 / 37915553131 / 37915553104.

Migrations: schema version 4. 0001 `d9068dccbc26ececb71be79c475080633166ba0163c62b2d98b9733512baefea`, 0002 `4924e69150bbff9c39e2e6b7e2bdd61045202e504900fe0f510d513fbf815e67`, 0003 `530a6d6cb5ec9c757311d48e10a62ef456d9d01c09512f321cffe42fe3307f80`, 0004 `b60000371f3c10d64adc7bb54b5e5144fd6ac6ec34246c072aaf68d77861d93a`. No migration 0005 exists.

P5 status: **CLOSED_ON_BRANCH**. PR #5 stays open on `p5/capability-registry` and is NOT merged; `main` is unchanged pending a separate integration gate.

## P5 independent-audit repair (supersedes the closure above)

The `4079e16` `CLOSED_ON_BRANCH` conclusion above was superseded after an independent source audit reopened P5. PR #5 remains open and unmerged. The record is retained as historical evidence; it is not the current closure conclusion.

### Findings and RED evidence

- **Generation identity was semantic-only.** TaskEngine accepted an externally supplied availability snapshot without proving it represented the Task's durable generation. RED coverage now proves refusal for a Task pinned to generation A with a generation B snapshot, including same-descriptor generations. Additional tests cover historical defaults, candidate priority, identical manifest digests with distinct generation IDs, missing historical material, and PreparedAction generation identity.
- **Argument provenance and requester provenance were lost.** The old generic path reconstructed DataClass from the descriptor ceiling and hard-coded `RequestedBy::Model`. RED cases cover trusted PUBLIC/PERSONAL/PRIVATE/CREDENTIAL classifications, descriptor and Task containment, Model/Scheduler provenance, and canonical persisted arguments.
- **New Task creation could leave a NULL generation.** Ordinary and scheduled creation now require an active generation and pin it in the same transaction. Failure injection after Task insert proves Task, journal, and event rollback; scheduled failure proves the Task and occurrence map roll back. Initial fixture setup seeds registry state explicitly; engine reopen/recovery constructors are pure.
- **Capability plan persistence was a second partial writer.** The old append path did not preserve complete plan membership, current-plan reference replacement, or the full ordinary revision history path. A multi-capability RED/reopen regression now covers the shared writer, argument bytes, both bindings, current PLAN count, revision history, and live-Task bound-Step removal refusal. A journal trigger proves plan, Steps, and bindings roll back together.
- **Migration 0004 contradicted Task deletion semantics.** The old unconditional binding `BEFORE DELETE` trigger rejected Task-owned cascade deletion despite `ON DELETE CASCADE`. A lifecycle RED test confirmed normal deletion failed under the old SQL.
- **Availability health sampling could spin forever.** The synchronous noop-waker polling loop was removed. Snapshot building is async and awaits each provider health future once; a Pending/wakeup regression proves completion and snapshot immutability.
- **Diagnostics could expose argument content through Debug.** Argument-bearing values now use redacted Debug output. Sentinel tests cover proposal, classified arguments, prepared actions, preparation/proposal errors, `MODEL_SCHEMA_VIOLATION`, and event payloads. Missing-field metadata names the actual absent field(s).

### Migration 0004 amendment

The old development checksum `b60000371f3c10d64adc7bb54b5e5144fd6ac6ec34246c072aaf68d77861d93a` is historical pre-repair evidence. Migration 0004 had not been merged to `main` or released, so the owner authorized an in-place amendment; no upgrade path for a deployed v4 database is required.

The binding delete trigger now rejects deletion only while its owning Task row exists. Tests prove direct binding deletion and bound Step deletion while the Task lives are refused, while normal Task deletion cascades all owned bindings and Steps. The same regression checks zero dangling bindings, zero `foreign_key_check` rows, and `integrity_check = ok`.

New 0004 SHA-256: `06b22fa682564290b71a26825a777f7a295220bd02f4c6c614d234b73001685f`. Schema version remains 4; 0001–0003 are unchanged; 0005 is absent.

### Repair implementation and verification

- PASS A binds `CapabilityAvailabilitySnapshotV1` to an exact caller-supplied durable generation and verifies durable manifest, catalog, member, priority, and default facts. Task generation must equal snapshot generation. PreparedAction carries both the semantic manifest digest and exact durable generation ID.
- PASS B accepts one trusted `ClassifiedArgumentsV1` and trusted `RequestedBy` per capability Step. It refuses CREDENTIAL and any class above either the descriptor ceiling or Task container class. Canonical JSON is derived from the prepared trusted arguments and is the plan/blob persistence source.
- PASS C makes ordinary and scheduled runtime creation strict. Both pin the active generation atomically; scheduled creation also commits its occurrence mapping, journal and event atomically. NULL is preserved only for explicit historical fixtures/migrated Tasks.
- PASS D routes ordinary and capability plans through one full-plan writer. It shares validation, membership, immutable-Step checks, historical StepId and sequence high-water, current PLAN replacement, PLAN_REVISION retention, state/revision update, journal evidence, and blob cleanup. Every new capability Step binding is written before the same transaction commits.
- PASS E removed the production executor and noop-waker loop. `build_for_generation` awaits `provider.health()` exactly once per provider for each snapshot.
- PASS F redacts argument-bearing Debug implementations and keeps violation errors/events to stable codes and bounded safe names/counts.
- Production `.invoke(` search in `serea-capability` and `serea-task-engine`: zero. The model-router `insert_task` occurrences are under `#[cfg(test)]`; production runtime creation goes through TaskEngine or the scheduler's strict scheduled-task path.

### Reclosure evidence status

The repair is not closed by the historical `CLOSED_ON_BRANCH` line above. Exact repaired behavior HEAD, final local validation, fresh whole-P5 review, behavior commits, CI run IDs, and final evidence HEAD will be recorded here only after those gates complete. PR #5 must remain **OPEN / READY FOR REVIEW / UNMERGED**. P6 and P8 remain **NOT STARTED**.

### Local repair verification update (2026-10-09)

The repaired worktree remains based on `4079e16ca4e6eb4884aaab1bf579c28a3deec9e6`; no repair commit has yet been created at the time of this validation update.

- PASS A/B/C/D/E/F focused evidence is recorded above and in the named regression targets. Latest full runs: capability 154 integration tests; TaskEngine 153 integration tests plus 3 doctests; storage 434 unit tests, 2 integration tests, and 37 doctests; scheduler 45 integration tests. The multi-capability reopen, journal-failure rollback, Task deletion cascade, Pending/wakeup health, and privacy sentinel tests passed.
- Static/local checks passed: `cargo fmt --all -- --check`; four-crate `cargo check`; four-crate all-target/all-feature Clippy with `-D warnings`; docs validator (100 Markdown files); workspace smoke; 76 Python workspace tests; Cargo metadata; commit identity; and `git diff --check`.
- Fresh whole-P5 review: migration/FK deletion semantics — no blocker or major; schema/catalog security — no blocker or major; manifest/provider authority — no blocker or major; exact generation/pinning — no blocker or major; classification/provenance — no blocker or major; Task creation paths — no runtime bypass found; plan/history/binding invariants — no blocker or major; crash/recovery/concurrency — no blocker or major; privacy/debug/events — no blocker or major; provider invocation/P6 boundary — no blocker or major, production invocation count zero.
- Review note: private Task storage remains subject to the existing P2 at-rest protection contract; no private Task acceptance is claimed where that contract refuses storage.
- Branch source audit: runtime Task creation is through `TaskEngine::create_task` and `TaskEngine::create_scheduled_task` (scheduler delegates to the latter). The model-router direct `insert_task` occurrences are under test-only code. Storage direct inserts are test/setup/recovery fixtures.
- Final local migration hashes: 0001 `d9068dccbc26ececb71be79c475080633166ba0163c62b2d98b9733512baefea`; 0002 `4924e69150bbff9c39e2e6b7e2bdd61045202e504900fe0f510d513fbf815e67`; 0003 `530a6d6cb5ec9c757311d48e10a62ef456d9d01c09512f321cffe42fe3307f80`; amended 0004 `06b22fa682564290b71a26825a777f7a295220bd02f4c6c614d234b73001685f`; schema version 4; 0005 absent.
- External GitHub state and required Fast/Full/cross-architecture CI are pending. The GitHub CLI query for PR #5 could not connect to `api.github.com`; no CI IDs are available yet. P5 remains **NOT CLOSED** until the repaired behavior and final evidence heads have the required exact-head CI results.

### Cross-architecture fixture follow-up

The first repair commit `f786daac3b1e0b32c6e8699723b552aead799046` was pushed normally. Exact-head Fast CI run `37948001927` and Full CI run `37948001935` passed. Cross-architecture run `37948001955` failed while producing its representative database on both architectures: the fixture created a new capability Step through ordinary plan persistence and then inserted its binding separately, which the repaired shared plan writer correctly rejects. The arm64 log showed the refusal at `sqlite_portability.rs` line 417.

The fixture is now being repaired to persist the complete capability plan, immutable binding, TaskJournal, and EventBus evidence through `put_capability_plan_revision` in one transaction. Local producer and consumer modes pass against the same single closed SQLite file; no `-shm` or `-wal` sidecar was present. The complete scheduler suite passes with the repaired fixture. The follow-up commit and new exact-head Fast, Full, and cross-architecture results remain pending; P5 remains **NOT CLOSED**.
