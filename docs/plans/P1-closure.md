# P1 — Workspace and Protocol Skeleton Closure

- **Project:** Serea
- **Architecture version:** `serea-arch/0.1.0` (unchanged)
- **Branch:** `p1/workspace-protocol-skeleton`
- **Base commit:** `78ad2550f1862aa3644d2b36f06616f4512707eb` (`p0/architecture-freeze`, P0 CLOSED)
- **Scope boundary:** protocol contracts only. No task engine, no durable state,
  no scheduler, no external service, no Android, no credential bytes, no
  GoalLatch provider or fake, no MCP connection, no Codex route.

## What P1 is

A compiling Rust workspace and the first contract slice: identifier newtypes,
the frozen wire types and enums, checked-in JSON Schema 2020-12 documents, typed
protocol errors, three empty provider ports, deterministic synthetic test
doubles, and CI. It establishes compile-time and serialization boundaries. It
does **not** make Serea perform anything.

`docs/plans/P1-workspace-and-protocol-skeleton.md` is the authority for what was
built. This document records what was built, how it was verified, what the three
review passes found, and what remains open.

## Workspace

| Crate | Layer | Depends on | Public surface |
| --- | --- | --- | --- |
| `serea-protocol` | `L0` contracts | nothing internal | identifiers, frozen types and enums, five checked-in schemas, typed errors, `CapabilityProvider` / `ModelProvider` / `HostGoalProvider` ports |
| `serea-testkit` | `TD` dev-only | `serea-protocol` | `TestClock`, `DeterministicUlidSource`, `MockModelProvider`, `MockCapabilityProvider`, synthetic descriptors and the frozen roster subset |

Direct dependencies, and why `std` was not enough:

| Crate | Dependency | Why `std` is insufficient |
| --- | --- | --- |
| `serea-protocol` | `serde` (derive) | Protocol Index §5 freezes JSON as the wire format and field names as `snake_case`. `std` has no serialization. |
| `serea-protocol` | `serde_json` | Same, for the value model. Also the only place a `Value` can exist, which is what the schema validator consumes. |
| `serea-protocol` | `jsonschema` (`default-features = false`) | Protocol Index §5 and Capability Protocol §3.1 require JSON Schema 2020-12 with fail-closed validation. A hand-written validator would be a second, drift-prone definition of the same constraints — the exact failure the plan forbids. `default-features = false` compiles out `resolve-http`, `resolve-file`, `resolve-async` and both TLS backends, so a `$ref` can only resolve inside its own document. |
| `serea-protocol` | `async-trait` | The frozen ports are `async` and must stay `dyn`-composable, because `serea-core` registers providers as trait objects (Crate Map §6.1 rule 1). A proc macro only; no async runtime. |
| `serea-testkit` | `serde_json` | The doubles return and accept protocol types. |

**Absent, deliberately:** no async runtime (`tokio`, `async-std`, `smol`,
`futures-executor`), no HTTP client, no database binding, no hashing crate, no
date library, no GoalLatch / Codex / OpenAI / `local_mcp` dependency. Verified
against the resolved lockfile and asserted by a test.

Two deliberate engineering decisions worth naming:

- **No `thiserror`.** The protocol errors are hand-written rather than derived.
  A derive macro puts a `Display` impl next to the rejected values and makes it
  easy to add a field that formats untrusted input; the hand-written impls are
  what let the crate guarantee that no rejected value reaches an error string.
- **No date library.** `Timestamp` validates the frozen wire form directly,
  including real calendar days, so `serea-protocol` carries no calendar
  dependency. `TestClock` does pure calendar arithmetic for the same reason.

## Files

Created, exactly the P1 file map and nothing else:

```text
Cargo.toml            Cargo.lock           rust-toolchain.toml   .clippy.toml
.github/workflows/ci.yml                    tests/workspace_smoke.py
crates/serea-protocol/Cargo.toml            crates/serea-protocol/src/{lib,ids,types,errors,provider,schema}.rs
crates/serea-protocol/schemas/{envelope,action-request,action-result,assistant-task,event}.schema.json
crates/serea-protocol/tests/{ids,protocol_types,schema_contracts}.rs
crates/serea-testkit/Cargo.toml             crates/serea-testkit/src/{lib,clock,models,providers}.rs
crates/serea-testkit/tests/fakes_are_deterministic.rs
docs/plans/P1-closure.md                    (this record)
```

No P0 file was modified. `tools/validate_docs.py` was not modified.

## Verification evidence

Every command is the plan's exact verification command, run from the repository
root on the pinned stable toolchain (`rustc 1.98.1`, `cargo 1.98.1`).

| Command | Result |
| --- | --- |
| `python3 tests/workspace_smoke.py` | OK — workspace shape satisfies the crate-map layering rule |
| `cargo fmt --all -- --check` | clean |
| `cargo metadata --no-deps --format-version 1` | exactly `serea-protocol`, `serea-testkit` |
| `cargo check --workspace --all-targets --offline` | clean |
| `cargo test --workspace --all-targets --offline` | **187 passed, 0 failed** (27 + 84 + 47 + 29) |
| `cargo clippy --workspace --all-targets --all-features --offline -- -D warnings` | clean |
| `python3 -m py_compile tools/validate_docs.py` | passed |
| `python3 tools/validate_docs.py docs` | passed — 38 Markdown files scanned |

Test breakdown: `tests/ids.rs` 27, `tests/protocol_types.rs` 84,
`tests/schema_contracts.rs` 47, `tests/fakes_are_deterministic.rs` 29. No
workspace member has a unit-test target; the behaviour is all exercised through
the public API, which is what a consumer sees.

Tests are offline and deterministic. No test reads a wall clock — `.clippy.toml`
bans `SystemTime::now` and `Instant::now` workspace-wide, so that is a compile
error rather than a review comment. No test opens a socket, reads the
filesystem, or depends on another test having run.

### TDD red/green record

Representative, not exhaustive. Full transcripts were captured during the run.

| Item | Frozen requirement | RED | GREEN |
| --- | --- | --- | --- |
| Workspace bootstrap | the plan's Phase 1 bootstrap gate | `python3 tests/workspace_smoke.py` → `FAIL: workspace members [] do not include 'crates/serea-protocol'` (exit 1) | same command after adding the minimal member → `OK` (exit 0) |
| Identifier grammar | Protocol Index §2/§3 | `tests/ids.rs` with the type structure present and the validators absent → `9 passed; 17 failed`, every failure an assertion on a rejected value | after implementing the grammar → `26 passed; 0 failed`, now `27` after the `TimestampMs` regression case |
| Frozen type and schema behaviour | Protocol Index §4/§5; Capability §4/§5; Event §2; Data Class §2 | `tests/protocol_types.rs` and `tests/schema_contracts.rs` written before the types → assertion failures on composition (`compose_all` returned `CREDENTIAL` for every input, because the fold was seeded with the identity), on `SemVer` pre-release plus build metadata, on an event-kind count, on `WireSurface("serea.action/0")` being accepted, on `EnvelopeVersion("0")` being accepted, and on nullable fields vanishing from a round trip | after implementation → `68 passed`, now `84` |
| Testkit determinism | Model Protocol §10; GoalLatch Adapter §6 | `TestClock::advance` treated elapsed milliseconds as seconds-since-epoch → `21 passed; 3 failed`, all three showing `1970-01-01` where `2026-10-01` was expected | after fixing the arithmetic and normalising the calendar → `24 passed`, now `29` |
| Regression review findings | the defects named below | each fix shipped with a test that fails on the pre-fix code | each now passes; see the review table |

No RED state was manufactured by inserting a syntax error. The identifier and
type items were made to compile with the validators deliberately absent, so the
failure was an assertion on behaviour; the review items were reproduced with
probe programs and by mutating the fix out of a scratch copy.

## Contract compliance

Every frozen set is pinned by a test that compares it against a literal list
transcribed from the owning protocol document, in both directions between the
Rust type and the checked-in schema.

| Frozen set | Count | Owning protocol |
| --- | --- | --- |
| ULID-prefixed identifier domains | 11 | Protocol Index §2 |
| `CapabilityId` verbs | 14 | Capability Protocol §2 |
| `DataClass` | 5 | Data Classification §2 |
| `RiskClass` | 8 | Policy Protocol §2 |
| `SideEffectClass` | 6 | Capability Protocol §3.1 |
| `Authorization` | 4 | Capability Protocol §3.1 |
| `RootRequirement` | 3 | Capability Protocol §3.1 |
| `IdempotencySupport` | 3 | Capability Protocol §3.1 |
| `CostClass` | 3 | Capability Protocol §3.1 |
| `ReplaySafety` | 3 | Capability Protocol §3.1 |
| `RequestedBy` | 5 | Capability Protocol §4.1 |
| `ActionStatus` | 6 | Capability Protocol §5 |
| `ActionErrorKind` | 13 | Capability Protocol §6.1 |
| `EvidenceKind` | 6 | Capability Protocol §7 |
| `TaskKind` | 5 | Task Protocol §2 |
| `TaskState` | 11 | Task Protocol §4.1 |
| `StepKind` | 8 | Task Protocol §3 |
| `ActorKind` | 6 | Event Protocol §2.1 |
| `EventKind` | 59 | Event Protocol §3.1–§3.10 |
| `ModelPurpose` | 6 | Model Protocol §3 |
| `FinishReason` | 5 | Model Protocol §4 |
| `JsonSchemaMode` | 3 | Model Protocol §5 |
| `ProviderHealth` | 2 | Capability Protocol §9 |

Specific properties P1 proves:

- **Authority fields are absent, not merely ignored.** `ActionRequest` has no
  `risk_class`, `side_effect_class`, `required_authorization` or `provider_id`
  member at all, so a model or any other caller cannot supply one. Those are
  host-resolved from the descriptor (Capability Protocol §4.2).
- **A closed surface where the protocol demands one, an open one where it
  demands compatibility.** `additionalProperties: false` appears only on the two
  action surfaces and on the capability-owned receipt, evidence and error
  objects nested inside them. The envelope, task, step, origin, budget, trace and
  actor objects retain unknown members in an opaque extension set and round-trip
  them unchanged.
- **Unknown enum variants fail closed everywhere**, including on the
  forward-compatible surfaces. Event Protocol §6 rule 2's "skip unknown kinds"
  is a *recipient rendering* rule; the host parse fails closed per Protocol Index
  §4.2 rule 3. Both halves of that reading are documented in the code.
- **`codex` is unreachable.** There is no routing in P1, no `codex_allowed`
  member on any protocol type, and the default is stated once as
  `CODEX_ALLOWED = false`. A test walks every protocol value the testkit
  produces and asserts none of them names `codex`.
- **`CapabilityDescriptor` cannot be deserialised past its own invariants.**
  Every construction path — `new`, `TryFrom<Draft>`, and therefore
  `Deserialize` — goes through the same two registration-time checks.
- **`goallatch` is a `ProviderId` but never a capability namespace**
  (GoalLatch Adapter §3.2, `G13`), enforced identically in the Rust validator and
  in all three schemas that can carry a `CapabilityId`.
- **No rejected value reaches an error string.** A credential-shaped value is
  refused by the identifier path, by the schema path (including the
  `anyOf` / `additionalProperties` kinds that nest a sub-error), and is asserted
  absent from both the `Display` and the `Debug` rendering.
- **The layering rule is enforced mechanically.**
  `tests/workspace_smoke.py` proves `serea-protocol` has no internal dependency of
  any kind and that no crate reaches `serea-testkit` outside
  `[dev-dependencies]`, across target-specific, `build-` and renamed-`package`
  spellings. Verified against five synthetic evasions.

## Deliberate departures from frozen prose

Three, all in the safe direction, all recorded here because P0's phase record is
where a departure belongs.

1. **Registration failure is a typed error, not a panic.** Capability Protocol
   §3.1 says a `provider_id`/identifier mismatch is a "registration-time panic,
   not a runtime warning"; Crate Map §6.1 rule 4 says the same. P1 returns
   `ProtocolError::ContractViolation`. The guarantee — never silently accepted —
   is unchanged; `serea-core` owns the startup behaviour in P5, and this crate's
   no-panic posture is the reason.
2. **The `CREDENTIAL`-class registration rule is applied in its general form.**
   Data Classification §2.3 names the rejected shape as a `CREDENTIAL`-class
   capability at `OBSERVE` risk. P1 requires `risk_class: CREDENTIAL` for any
   `CREDENTIAL` data class, because a `CREDENTIAL` capability at a lower risk
   class is the same contradiction with a different number. Relaxing it is a
   contract question under Protocol Index §7, not an implementation detail.
3. **One host acceptance ceiling exists where P0 specifies none.** See below.

## Open items — not closed by P1

P0's closure record retains these as unresolved. **P1 does not close any of
them**, and nothing in this phase should be read as enforcement.

- **A host acceptance ceiling on free text.** `MAX_VALUE_LENGTH = 4096` applies
  to code-like and diagnostic fields that P0 does not bound. Bounds Protocol §2
  declares its table authoritative and a bound enforced elsewhere a bug (`B3`),
  and the P0 closure records payload-byte bounds as an open gap. This ceiling
  must be ratified into Bounds §2 by ADR, or removed, in the phase that owns
  bound configuration (`serea-core`, P2). It deliberately does **not** apply to
  model input or output text, because P0 *does* bound those with
  `max_output_tokens_per_call`; the asymmetry is asserted by a test.
- **Payload-byte, attachment-size and object-count bounds** remain unspecified.
  P1 imposes a schema *nesting* depth bound of 64 and refuses an instance deeper
  than that before the validator runs, because a deep `Value` read from durable
  state has no recursion guard and would abort the process. Size and count
  bounds are still P0's open gap.
- **No durable-state integrity seal** against a local writer. Nothing in P1
  touches durable state.
- **No frozen Android exported-component or Intent contract.** No Android code
  exists.
- **No universal provider-freshness guarantee.** No provider exists.
- **No dedicated external model-provider quota bound.** P1 models
  `ModelUsage` and `CostClass` as data and enforces nothing.
- **Ordinary approval does not require biometrics.** No approval type exists in
  P1; `ApprovalId` and `GrantId` are identifiers only.

### Representation-only items, deferred by design

- `Secret<T>` is a frozen public-surface item of `serea-protocol` (Crate Map
  §3.1; Data Classification §3.2). P1 defines only `CredentialHandle`. Defining
  `Secret<T>` needs `zeroize` and `subtle` and a `ZeroizeOnDrop` bound decision
  the P1 plan does not make. **No other crate may define it in its place.**
- `GoalHandle`, `GoalObservedState`, `GoalEvidenceRef`, `GoalArtifactRef`,
  `GoalSummary` (GoalLatch Adapter §3.1) travel *inside* `arguments` and
  `output` as schema-validated JSON per §3. P1 declares the `HostGoalProvider`
  port and no adapter, so there is nothing for them to travel in yet.
- `PolicyDecision` / `DenyReason` (`serea-policy`, P2), `ApprovalRequest` /
  `ApprovalGrant` (`serea-capability`, P6), `DeviceLinkPort` (`serea-core`, P12).
- Canonical JSON and digest **computation**. `Digest` is a validated newtype;
  nothing computes one. `jsonschema` and `serde_json` are used without the
  `arbitrary_precision` feature, so two distinct integers above `u64::MAX`
  collapse to one `Value`. This matters only for a capability whose
  `input_schema` admits a bare number, and `A11` binds a grant to an exact
  digest — the P5 schema-compilation obligation is therefore "reject a
  non-integer where an integer is meant".
- `ProviderId` validates the frozen `[a-z][a-z0-9_]{1,31}` grammar, not the
  frozen *namespace set*, which Protocol Index §4.3 freezes but no document
  enumerates. Namespace membership is the registry's job in P5.
- `codex_allowed` has no protocol member. P0 assigns it to a task-level durable
  host setting, and the frozen `AssistantTask` shape does not carry it. P1 states
  the default once, as `CODEX_ALLOWED = false`.
- `Envelope<T>` does not bind `T` to a surface, and
  `Envelope::require_supported_major` / `require_supported_surface` are opt-in
  because Protocol Index §4.2 rule 1 places the duty on the consumer. The device
  link in `serea-core` (P12) is where they must be called.
- `ActionResult` cannot express `C4` ("receipt non-null exactly when
  `SUCCEEDED` and `side_effect_class != NONE`"), because the result does not
  carry `side_effect_class`. A consumer must join against the descriptor. That
  join is where `E9` / `INV-SEC-04` is enforced.
- Capability `input_schema` documents do not exist yet, so the `DC8` structural
  credential exclusion is enforced only at the envelope level. P5 must add a test
  that every registered `input_schema` compiles under
  `default-features = false` and that none can express a `$ref` off-document.

### P0 internal inconsistency noted, not resolved

`DELETION_CASCADE_COMPLETED` is named in Data Classification §8.2 step 4 but is
absent from the frozen `EventKind` table in Event Protocol §3. The frozen table
wins: the name is refused by both the Rust enum and `event.schema.json`, and a
test says why. Resolving the discrepancy is a documentation change under
Protocol Index §7 and belongs to whoever owns that contract.

## Review passes

Three independent read-only passes, findings frozen before any code changed.
Full dispositions are in `tmp/reviews/` (git-ignored scratch) and summarised
here.

### Pass A — code review: 20 findings, all dispositioned

Verdict REQUEST CHANGES. Scope compliance PASS on all eight questions (file map
exact, no P2 scope, no forbidden dependency, no credentials, no GoalLatch
provider or fake, workspace shape, `codex_allowed`, no runtime edge to the
testkit).

Two were real defects with a reachable consequence:

- **`Timestamp` validation read past the end of a 20-byte value** whose final byte
  was not `Z`, panicking from every wire deserialisation. Fixed with a
  length-first match; four regression cases added.
- **`CapabilityDescriptor`'s derived `Deserialize` wrote its private fields
  directly**, bypassing the two registration invariants the type's own
  documentation claims to enforce. Fixed with `serde(try_from = Draft)`; two
  regression tests added.

The rest: an `EnvelopeVersion`/`WireSurface` inconsistency; closed-object
validation applied to task sub-objects the plan excludes; schema enum sets
validated only against Rust constants (so an *added* schema value would ship
green); a workspace guard that was both over-broad and evadable; two declared
dependencies never used; a CI step that printed the member list instead of
asserting it; a `SchemaName::index()` that fell back silently; an invented
`serea.envelope/1` surface name in a schema title; a misattributed `Secret<T>`
ownership citation; an `AMBIGUOUS` test asserting a different error's flag; four
assertions that could not fail; a module comment that contradicted the code; a
testkit clock whose `starting_at`/`reset` contradicted their documentation.

Rejected: pinning an exact toolchain version. The plan mandates "the current
stable channel used by CI"; the floating channel is recorded as an accepted
risk here.

### Pass B — security review: 13 findings, all dispositioned

Verdict APPROVE WITH FINDINGS. The reviewer could not find any path by which a
model, caller, or deserialized payload creates, widens, or attaches authority,
and confirmed the flattened-extension design is not a shadowing hole.

- **Three reachable panics on arithmetic paths.** `UlidValue::new` asserted on a
  caller-supplied `u64`; `TestClock::at`/`advance` hit `unreachable!` on ordinary
  input (the year-9999 boundary, a calendar-impossible epoch); the deterministic
  identifier source overflowed in debug and wrapped in release. Fixed: a
  validated `TimestampMs` type makes `UlidValue` total, `at`/`advance` return
  typed errors and `advance` validates before committing, and the source holds at
  the range end with counter-derived entropy so every mint stays distinct.
- **The schema and the Rust type disagreed about `G13`.** Three schema documents
  carried the three-segment capability regex without the `goallatch`
  prohibition. Fixed, with a test.
- **`Trace` was forward-compatible in Rust and closed in both schemas.** Fixed
  on both sides.
- **`schema::validate` stack-overflowed on a deep `Value`.** `serde_json`'s
  128-level guard only applies when parsing text. Fixed with a depth bound.
- **`SchemaError`'s `Display` echoed the rejected instance.** Fixed; the reason
  is now derived from the validator's error *kind*, never the instance.
- **`lib.rs` claimed "no type can hold a secret"**, which was true of intent but
  not capability. Weakened to the claim that is true, and `ActionError.message`
  and `details` named as the `AB-13` carrier a P2 provider author must be
  reviewed for.

Rejected: a CI credential-path guard finding, which the reviewer re-tested and
found sound.

### Pass C — scoped re-review: 12 findings, all fixed

Verdict REQUEST CHANGES. Scoped to the Pass A/B fixes and the paths they touched.

Three blocking, and the most useful result of the three passes: the reviewer
**mutated each fix out and re-ran the suite**, which proved several Pass A
"fixes" were not actually pinned by any test.

- **The new depth guard was recursive in disguise**, so it aborted the process
  at ~40 000 depth and was quadratic on a chain — the very failure it existed to
  prevent. Its doc comment claimed the opposite. Fixed with a single-pass stack
  walk; the comment now matches the code.
- **The credential-echo fix still echoed**, through the four validator error
  kinds that nest a sub-error (`AnyOf`, `OneOf`, `PropertyNames`,
  `AdditionalProperties`). Fixed with an explicit kind walk and a wildcard arm,
  so a variant added by a future `jsonschema` cannot reintroduce the leak. The
  regression test now exercises five rejection shapes instead of one.
- **`Trace` was open in Rust and closed in both schemas**, and the doc comment
  claimed they agreed. Fixed on both sides.
- **Nine fixes were unpinned**: `origin` and `attempt_budget` forward
  compatibility, the schema enum pinning (a stub module — the merge that was
  supposed to add it had silently *destroyed* nine existing tests), the
  `TimestampMs` guard, `is_valid`'s new semantics, the `Envelope` major checks,
  and the workspace guard's two idiomatic Cargo spellings. All now have tests that
  fail on the pre-fix code.
- A duplicate JSON key introduced by a Pass A edit, and a 3-digit schema cap that
  outlived the Rust change it was meant to match.

### Regression review: 16 findings, all dispositioned

Because P1 has no runtime and no UI, this reviewed P0 contract drift,
documentation contradiction, identifier/event/schema regression, accidental scope
expansion, test-fixture hygiene, and future compatibility hazards.

- **No scope expansion.** No task engine, SQLite, event-bus runtime, scheduler,
  policy engine, model-router runtime, memory, device link, Android, credential
  bytes, OAuth, network client, GoalLatch provider or fake, MCP connection, Codex
  routing, daemon, device pairing, HTTP/WebSocket server, canonical-JSON digest
  computation, or approval ledger. Confirmed absent.
- **Every frozen set is mechanically exact**, script-extracted in both directions:
  11 ULID prefixes, `EventKind` 59/59 in document order, `ActionErrorKind` 13/13
  with no bound-exhaustion kind added, `EvidenceKind` 6/6 disjoint from
  `EventKind`, `DataClass` 5, `RiskClass` 8 with the §2 numbering, 14 verbs
  order-identical. `DELETION_CASCADE_COMPLETED` correctly refused.
- **A test name asserted the opposite of its body.** The task-ceiling test was
  named `an_observe_task_cannot_widen_its_own_ceiling_by_deserialisation` while
  asserting the field *does* widen and deferring enforcement to P6. The body was
  right; the name was a false claim printed as `ok`. Renamed.
- **A `ProviderContext` of public fields enforced nothing** — reproduced a
  `delete@9.9.9` request alongside a `list@1.0.0` context. Now assembled through
  one constructor, with a doc note stating what it cannot check and why.
- **`side_effect_receipt` and `error` on a step were bare `object | null`** in the
  schema, so a step could carry any object including a credential-shaped one,
  while the Rust type refused it. Both are now closed and shape-checked.
- **The Rust code validator and the schema code pattern disagreed**: `1FOO` and
  `_NONE` were valid Rust and invalid on the wire. Unified on
  `^[A-Z][A-Z0-9_]*$`.
- Four documentation accuracy fixes: a roster description that implied three
  frozen models where Model Protocol §5.1 has four; a citation to Event Protocol
  §8 (Retention) for determinism; an invented `fast: true` on a model §5.1 does
  not mark fast; and an incomplete list of the Crate Map §3.1 surface items P1
  does not define.
- **The free-text ceiling was non-uniform**: model text was unbounded while
  diagnostic text was capped. Now deliberate and documented, with the reason
  (P0 *does* bound model text) and a test.

## Explicit non-claims

P1 does **not** claim, and nothing here may be read as claiming, any of the
following:

- that a model proposal has been validated, registered, policy-checked, or
  approved. There is no registry, no policy engine and no approval ledger.
- that a capability effect has been proven. A `SideEffectReceipt` is a type; no
  code produces or requires one.
- that any bound is enforced. `AttemptBudget` is three integers.
- that events are ordered, durable, or gapless. `Seq` carries a number; `seq` is
  assigned at commit in P3.
- that a task state transition was validated. `TaskState` is a closed enum with
  `is_terminal()`; the transition table is `serea-task-engine`'s in P2.
- that a duplicate action was suppressed, or that an `AMBIGUOUS` result was
  reconciled.
- that a capability input schema is closed, or that a credential cannot appear
  in `arguments`.
- that `codex` exclusion is *enforced* anywhere. It is unreachable by
  construction in P1 because there is no router.
- that any runtime exists. There is no runtime, no daemon, and no network.
