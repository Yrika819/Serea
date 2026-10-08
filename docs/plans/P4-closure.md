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
- Migration 0003 (`0003_model_accounting.sql`) SHA-256 at P4 task start:
  `8f4c5c4e047a8829201ce834ff192d740ab6ca199ecc3d12dfe8e357cf82c2ec`.
- Migration 0003 was amended on the still-unmerged P4 branch to add the
  durable task token total required by Bounds Protocol. Its current SHA-256 is
  `530a6d6cb5ec9c757311d48e10a62ef456d9d01c09512f321cffe42fe3307f80`.
  Migrations 0001 and 0002 remain unchanged. P4B evidence is reconciled here
  because the required token bound must survive usage-detail retention; no
  released migration or schema version changed.

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
- `tasks.model_token_count` accumulates only trustworthy settled usage in the
  same transaction as usage persistence. The durable total survives 365-day
  usage-detail retention, so cleanup cannot reset the frozen 128,000-token
  task bound. Integrity checks require the durable total to cover all retained
  usage detail.
- `tasks.model_call_count` preserves the 12-call task ceiling after 30-day
  attempt detail pruning. Every committed dispatch intent increments it in the
  same transaction as the attempt and reservation. P4E adds
  `tasks.model_turn_count`, which increments only for primary (`NONE`) attempts
  and survives the same attempt-detail pruning.
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
- At P4C closure, all three migration checksums still matched P4B: 0001
  `d9068dccbc26ececb71be79c475080633166ba0163c62b2d98b9733512baefea`, 0002
  `4924e69150bbff9c39e2e6b7e2bdd61045202e504900fe0f510d513fbf815e67`, and
  0003 `8f4c5c4e047a8829201ce834ff192d740ab6ca199ecc3d12dfe8e357cf82c2ec`.
  P4E later amended only unreleased migration 0003 as recorded below.
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

## P4D Evidence (closed on branch)

- First response-binding behavior slice RED proof: after adding the test for
  exact request/model/provider identity and clearing provider-controlled
  `structured`/`repair_attempts`, `cargo test -p serea-model-router --test
  routing --offline` failed to compile because `bind_provider_response` was
  not implemented.
- The narrow binding helper is now implemented and the focused routing test
  passes, including request ID, model ID and provider ID spoof rejection.
  This helper does not validate or persist content and does not dispatch.
- Sequential review: the helper enforces exact host-selected identities,
  strips the provider's `structured` proposal and repair count, adds no crate
  edges or storage effects, and does not inspect response content. Tests cover
  spoofed request/model/provider IDs and host-owned field sanitization. The
  helper is explicitly documented as identity binding only, not validation.
- Local validation passes: focused routing test; workspace check; all-target
  workspace tests; all-feature workspace tests; workspace Clippy with denied
  warnings; docs validation; workspace smoke and 76 smoke unit tests; Cargo
  metadata; identity guard; and `git diff --check`.
- This is an internal P4D behavior sub-slice. P4D remains open; durable intent,
  `MODEL_CALLED`, provider dispatch, completion, failure and recovery are not
  implemented yet.
- Exact pushed commit `d9479de37c997c11e0e7dca09341df59c83b3e2f` Actions:
  [Fast CI 37657974845](https://github.com/Yrika819/Serea/actions/runs/37657974845)
  GREEN; [Full CI 37657974779](https://github.com/Yrika819/Serea/actions/runs/37657974779)
  GREEN including Linux stable, MSRV 1.85, Intel, arm64 and release fault proof;
  [cross-architecture SQLite 37657974803](https://github.com/Yrika819/Serea/actions/runs/37657974803)
  GREEN in both directions.
- Sixth P4D RED proof: the scripted dispatch test failed to compile because
  `recover_completed_chat_text_response` was absent. The recovery reader now
  rebuilds a CHAT/TEXT `ModelResponse` from the completed attempt's trusted
  identity, host price/cost class, validated usage and accepted stored text.
  The file-backed test closes and reopens the Store, recovers the same result,
  verifies one usage and one durable completion event, and confirms provider
  invocation count did not increase.
- Sequential review limits this reader to completed CHAT operations with a
  retained accepted response and matching host accounting facts. It does not
  make a provider call, accept provider `structured` metadata, or add content
  to events. Structured response reconstruction remains for P4E host
  validation.
- Local validation passes: fmt; workspace check; all-target and all-feature
  tests; Clippy with denied warnings; docs validation; workspace smoke and its
  76 tests; Cargo metadata; identity guard; and `git diff --check`. Exact
  caller-loss behavior commit `5a664dc196bc0b39057f6be347d328e11240fa6b`
  Actions: [Fast CI 37668400647](https://github.com/Yrika819/Serea/actions/runs/37668400647)
  GREEN; [Full CI 37668400602](https://github.com/Yrika819/Serea/actions/runs/37668400602)
  GREEN including Linux stable, MSRV 1.85, Intel, arm64 and release fault proof;
  [cross-architecture SQLite 37668400564](https://github.com/Yrika819/Serea/actions/runs/37668400564)
  GREEN in both directions.
- Seventh P4D RED proof: adding the uncertain-outcome assertion initially
  failed to compile because live ambiguity recording was not implemented. A
  typed `AMBIGUOUS_DISPATCH` provider error now records AMBIGUOUS and a
  content-free `MODEL_FAILED` with `retryable=false` in one transaction, keeps
  the full reservation, and writes no usage. The scripted dispatch test also
  checks response model-ID spoofing and provider cost-class mismatch both fail
  without accepted response, usage, or raw event content.
- Sequential review confirms this typed ambiguous result cannot be surfaced as
  a definite retryable failure for P4E fallback. Identity and cost are compared
  to the host attempt before acceptance/accounting; the persisted CHAT/TEXT
  caller-loss result remains recoverable without another provider call. Local
  validation passes: fmt; workspace check; all-target and all-feature tests;
  Clippy with denied warnings; docs validation; workspace smoke and its 76
  tests; Cargo metadata; identity guard; and `git diff --check`. Exact behavior
  commit `b2e25f561f5a546a0dde05deb9b99ece9a3d7cb0` Actions:
  [Fast CI 37670535054](https://github.com/Yrika819/Serea/actions/runs/37670535054)
  GREEN; [Full CI 37670534918](https://github.com/Yrika819/Serea/actions/runs/37670534918)
  GREEN including Linux stable, MSRV 1.85, Intel, arm64 and release fault proof;
  [cross-architecture SQLite 37670534938](https://github.com/Yrika819/Serea/actions/runs/37670534938)
  GREEN in both directions.
- Eighth P4D RED proof: the testkit scripted-model test failed to compile
  because request capture and scripted health methods were absent. The
  existing `MockModelProvider` now captures each exact `ModelRequest` and
  scripts provider health readings in order, repeating its last reading like
  model outcomes. Existing exact `ModelResponse` and `ModelError` scripts
  already cover finish reasons, usage, latency and identity mismatches through
  the real `ModelProvider` trait.
- The fake remains in `serea-testkit`, with no router runtime or testkit
  production dependency edge and no network access. Its focused test checks
  captured request equality, DEGRADED then READY health, deterministic final
  health repetition, and read count. Local validation passes: fmt; workspace
  check; all-target and all-feature tests; Clippy with denied warnings; docs
  validation; workspace smoke and its 76 tests; Cargo metadata; identity guard;
  and `git diff --check`. The exact preceding docs head
  `377a41089556b4a7fd6558e6820fd8465e744d39` passed Fast CI
  [37671204419](https://github.com/Yrika819/Serea/actions/runs/37671204419),
  Full CI
  [37671203777](https://github.com/Yrika819/Serea/actions/runs/37671203777)
  including Linux stable, MSRV 1.85, Intel, arm64 and release fault proof, and
  cross-architecture SQLite
  [37671203755](https://github.com/Yrika819/Serea/actions/runs/37671203755)
  in both directions.
- Exact testkit behavior commit `cb6a25ed66074c902c5088009eb76d486cda193f`
  Actions: [Fast CI 37672272876](https://github.com/Yrika819/Serea/actions/runs/37672272876)
  GREEN; [Full CI 37672273121](https://github.com/Yrika819/Serea/actions/runs/37672273121)
  GREEN including Linux stable, MSRV 1.85, Intel, arm64 and release fault proof;
  [cross-architecture SQLite 37672272956](https://github.com/Yrika819/Serea/actions/runs/37672272956)
  GREEN in both directions.
- The exact closure-doc head `1a3e6bd00e1d4108ac90e45847cb3be3f50e5132` is
  GREEN on [Fast CI 37672949637](https://github.com/Yrika819/Serea/actions/runs/37672949637),
  [Full CI 37672949610](https://github.com/Yrika819/Serea/actions/runs/37672949610)
  including Linux stable, MSRV 1.85, Intel, arm64 and release fault proof, and
  [cross-architecture SQLite 37672949712](https://github.com/Yrika819/Serea/actions/runs/37672949712)
  in both directions.
- Second P4D RED proof: the post-routing request test initially failed to
  compile because `ModelRouterV1::build_request` did not exist. The constructor
  now binds to the same `RoutingSessionV1`, rechecks eligibility, and copies
  only host-prepared request fields. Its focused test covers semantic-field
  preservation and refusal to substitute a different model. This remains
  request construction only; provider dispatch and durable intent are pending.
- Sequential authority review found that a public mutable route decision
  could let an in-process caller replace the selected chain member. The
  decision and health snapshot are now private with read-only accessors, and
  request construction is bound to that immutable session. Focused routing
  tests pass after the change.
- Local validation for this request-construction slice passes: fmt, workspace
  check, all-target tests, all-feature tests, Clippy with denied warnings, docs
  validation, workspace smoke and its 76 tests, Cargo metadata, identity guard,
  and `git diff --check`.
- Exact pushed commit `351ccd0c2109b6a769823e6cab5cf2a56ec58aff` Actions:
  [Fast CI 37659651139](https://github.com/Yrika819/Serea/actions/runs/37659651139)
  GREEN; [Full CI 37659650977](https://github.com/Yrika819/Serea/actions/runs/37659650977)
  GREEN including Linux stable, MSRV 1.85, Intel, arm64 and release fault proof;
  [cross-architecture SQLite 37659650845](https://github.com/Yrika819/Serea/actions/runs/37659650845)
  GREEN in both directions.
- Third P4D RED proof: the Event Bus model-event test initially failed because
  typed model metadata and the `MODEL_CALLED` event builder were absent. The
  Event Bus now drafts `MODEL_CALLED`, `MODEL_COMPLETED`, and `MODEL_FAILED`
  with bounded host metadata only, typed relation/error facts, and the frozen
  model activity retention. Tests assert exact payload keys and no diagnostic
  content. These drafts still must be committed atomically with model accounting
  by the upcoming router dispatch path.
- Local validation passes for the event-drafting slice: focused model-event
  tests; fmt; workspace check; all-target and all-feature workspace tests;
  Clippy; docs validation; workspace smoke and all 76 smoke tests; Cargo
  metadata; commit identity guard; and `git diff --check`. Sequential review
  confirms Storage remains independent of Event Bus and the new API has only
  typed metadata inputs; dispatch integration and event/state atomicity remain
  open.
- Exact pushed commit `0b3e8f5259e38e0feb0b8d9a6b44dfff7ff187ef` Actions:
  [Fast CI 37661630288](https://github.com/Yrika819/Serea/actions/runs/37661630288)
  GREEN; [Full CI 37661630216](https://github.com/Yrika819/Serea/actions/runs/37661630216)
  GREEN including Linux stable, MSRV 1.85, Intel, arm64 and release fault proof;
  [cross-architecture SQLite 37661630298](https://github.com/Yrika819/Serea/actions/runs/37661630298)
  GREEN in both directions.
- Fourth P4D RED proof: the scripted dispatch test initially failed to compile
  because `dispatch_chat_text` was absent. The new crate-private CHAT/TEXT path
  uses the `ModelProvider` trait and atomically commits the model-call
  reservation/intent with `MODEL_CALLED` before `generate()`. The scripted
  provider verifies both records are visible at invocation. Success commits
  attempt, bounded usage/cost, response blob and `MODEL_COMPLETED` together;
  definite `ModelError` commits FAILED with content-free `MODEL_FAILED` facts.
  The test also proves a failed reservation does not call the provider or add
  an event, and that response `structured`/repair fields are host-sanitized.
- Sequential review findings/fixes: Clippy found oversized argument lists and
  test `unwrap()` calls; a narrow context/facts struct and panic-free test
  assertions resolved them. Review also removed an output-cap filter beyond
  the frozen `min_output_tokens` rule, and tightened persisted/accepted output
  usage to the effective selected-model ceiling. Router dependencies are now
  limited to protocol, storage and Event Bus; Storage remains independent of
  Event Bus. No prompt or provider diagnostic is stored in events.
- Local validation for this dispatch sub-slice passes: fmt; workspace check;
  all-target and all-feature workspace tests; workspace Clippy with denied
  warnings; docs validation; workspace smoke and its 76 tests; Cargo metadata;
  identity guard; and `git diff --check`. P4D remains open: recovery to
  AMBIGUOUS, caller-loss lookup, crash windows, and broader failure tests remain
  to be implemented.
- Exact behavior commit `02e177e5b721de337b28b002f951e16a102606df` Actions:
  [Fast CI 37664012626](https://github.com/Yrika819/Serea/actions/runs/37664012626)
  GREEN; [Full CI 37664012623](https://github.com/Yrika819/Serea/actions/runs/37664012623)
  GREEN including Linux stable, MSRV 1.85, Intel, arm64 and release fault proof;
  [cross-architecture SQLite 37664012627](https://github.com/Yrika819/Serea/actions/runs/37664012627)
  GREEN in both directions.
- Fifth P4D RED proof: startup recovery test compilation failed because
  `recover_unresolved_model_calls` was absent. The host startup seam now lists
  prior unresolved intents and atomically marks each AMBIGUOUS with its stable,
  content-free MODEL_FAILED event. It preserves the full reservation and writes
  no usage. The focused file-backed test closes and reopens SQLite, verifies
  AMBIGUOUS and unchanged reservation, and proves a second recovery pass adds
  no duplicate event.
- Sequential review confirms the state/event pair shares one Storage
  transaction per attempt. If another terminal transition wins after the
  recovery snapshot, the stale recovery transaction adds no event. The router
  still has no task lifecycle ownership; recovery never redispatches. This
  change adds no dependency edges or migration changes. Broader crash and
  concurrent-recovery coverage remains open.
- Local validation passes for the recovery slice: fmt; workspace check;
  all-target and all-feature workspace tests; Clippy with denied warnings;
  docs validation; workspace smoke and its 76 tests; Cargo metadata; identity
  guard; and `git diff --check`. The exact preceding docs head
  `69b562c634f0e37047129016d4b30d42b4e02104` is green on Fast CI
  [37664528638](https://github.com/Yrika819/Serea/actions/runs/37664528638),
  Full CI
  [37664528729](https://github.com/Yrika819/Serea/actions/runs/37664528729)
  and cross-architecture SQLite
  [37664528650](https://github.com/Yrika819/Serea/actions/runs/37664528650).
- CI review found an Intel-only failure in the existing
  `two_store_connections_serialize_global_sequence_allocation` storage test:
  both workers raced to bootstrap the same fresh database before reaching the
  test's allocation barrier. The test now initializes its fixture before
  opening the two concurrent stores, so the barrier isolates the intended
  sequence-allocation race. The focused test passed twice locally. No
  production Storage behavior or migration changed. Exact correction commit
  `973ded29a1ef43160227c662b084fec813268a1f` Actions:
  [Fast CI 37666894893](https://github.com/Yrika819/Serea/actions/runs/37666894893)
  GREEN; [Full CI 37666894903](https://github.com/Yrika819/Serea/actions/runs/37666894903)
  GREEN including Linux stable, MSRV 1.85, Intel, arm64 and release fault proof;
  [cross-architecture SQLite 37666894967](https://github.com/Yrika819/Serea/actions/runs/37666894967)
  GREEN in both directions.

- Exact closure-document head `00be832ef0757e27010f8913e64627ae9732d193`
  passed Fast CI `37673625809`, Full CI `37673625822` (Linux stable,
  MSRV 1.85, Intel, arm64, and release fault proof), and cross-architecture
  SQLite `37673625791` in both directions.
- Finish-reason coverage sub-slice: expanded the file-backed scripted dispatch
  test to exercise `CONTENT_FILTER`, `LENGTH`, `ERROR`, and
  `STRUCTURE_INVALID`. Every response is terminal, persists no accepted
  response blob, and records only validated usage and host-priced cost. No
  provider output enters event payloads. The first focused run caught an
  incorrect aggregate event-count expectation in the new test; correcting the
  test oracle made the focused test pass. This slice adds coverage for the
  existing P4D finish handling and does not change runtime behavior.
- Sequential review: contract and authority checks confirmed finish reasons
  do not trigger fallback or become a host schema verdict; transaction review
  confirmed terminal failure and usage remain committed together; recovery
  review confirmed no accepted truncated response; privacy review confirmed
  event payloads remain content-free; accounting review confirmed input,
  output, latency, and cost are retained only after provider metadata checks;
  test review confirmed the reopened file-backed assertions; docs review keeps
  P4D explicitly in progress. No migration or dependency change.
- Local validation for this coverage slice passes: focused dispatch test;
  fmt; workspace check; all-target and all-feature workspace tests; Clippy
  with denied warnings; docs validation; workspace smoke and all 76 smoke
  tests; Cargo metadata; identity guard; and `git diff --check`.
- Exact behavior commit `2dfb912` (`test: cover model finish reason failures`)
  passed Fast CI
  [37675104356](https://github.com/Yrika819/Serea/actions/runs/37675104356),
  Full CI
  [37675104275](https://github.com/Yrika819/Serea/actions/runs/37675104275)
  including Linux stable, MSRV 1.85, Intel, arm64, and release fault proof,
  and cross-architecture SQLite
  [37675104231](https://github.com/Yrika819/Serea/actions/runs/37675104231)
  in both directions.
- Provider-response identity coverage sub-slice: the scripted dispatch fake
  can independently return a mismatched RequestId or ProviderId. Both cases
  are rejected as terminal `PROVIDER_PROTOCOL_FAILURE`; the durable attempt
  fails with no accepted response blob and no usage, and the event contains the
  stable non-retryable error. The reopened success remains unchanged. This
  exercises the existing exact three-identity binding and changes no runtime
  behavior. The first focused runs caught two test-fixture issues: the spoofed
  provider token did not follow the protocol grammar, and the reopened replay
  page limit hid newly added events. Using a valid alternate provider token
  and a sufficient replay limit made the focused assertions pass.
- Sequential review: contract checks confirmed all three response identities
  are bound; authority checks confirmed spoofed response facts do not replace
  the selected model/provider or durable RequestId; transaction checks
  confirmed protocol failures terminalize the original attempt; recovery and
  privacy checks confirmed no response content or usage is accepted; accounting
  checks confirmed no untrusted usage is charged; test-quality review confirms
  file reopen and stable event assertions; docs continue to mark P4D open.
- Local validation for this identity-binding coverage passes: focused dispatch
  test; fmt; workspace check; all-target and all-feature tests; Clippy with
  denied warnings; docs validation; workspace smoke and all 76 smoke tests;
  Cargo metadata; identity guard; and `git diff --check`.
- Exact behavior commit `bab0004aab2e42229aed7237b6936109119046ef`
  passed Fast CI
  [37677167715](https://github.com/Yrika819/Serea/actions/runs/37677167715),
  Full CI
  [37677167677](https://github.com/Yrika819/Serea/actions/runs/37677167677)
  including Linux stable, MSRV 1.85, Intel, arm64, and release fault proof,
  and cross-architecture SQLite
  [37677167689](https://github.com/Yrika819/Serea/actions/runs/37677167689)
  in both directions.
- Exact closure-document head `05c59734fc2c57e804aa15290b6444c54eac9512`
  passed Fast CI
  [37677814303](https://github.com/Yrika819/Serea/actions/runs/37677814303),
  Full CI
  [37677814306](https://github.com/Yrika819/Serea/actions/runs/37677814306)
  including Linux stable, MSRV 1.85, Intel, arm64, and release fault proof,
  and cross-architecture SQLite
  [37677814329](https://github.com/Yrika819/Serea/actions/runs/37677814329)
  in both directions.
- Exact closure-document head `da038f7490eef1bcbb219707779fcee9ac201d52`
  passed Fast CI
  [37678476729](https://github.com/Yrika819/Serea/actions/runs/37678476729),
  Full CI
  [37678476699](https://github.com/Yrika819/Serea/actions/runs/37678476699)
  including Linux stable, MSRV 1.85, Intel, arm64, and release fault proof,
  and cross-architecture SQLite
  [37678476768](https://github.com/Yrika819/Serea/actions/runs/37678476768)
  in both directions.
- Exact closure-document head `6926f85fdc98b335da48d25c0a0009ffea7f1258`
  passed Fast CI
  [37679116021](https://github.com/Yrika819/Serea/actions/runs/37679116021),
  Full CI
  [37679115704](https://github.com/Yrika819/Serea/actions/runs/37679115704)
  including Linux stable, MSRV 1.85, Intel, arm64, and release fault proof,
  and cross-architecture SQLite
  [37679115750](https://github.com/Yrika819/Serea/actions/runs/37679115750)
  in both directions.
- Exact closure-document head `e9145edc58b09f8d2e2ccf7d8c4eba4d55c4e2c1`
  passed Fast CI
  [37679790537](https://github.com/Yrika819/Serea/actions/runs/37679790537),
  Full CI
  [37679790571](https://github.com/Yrika819/Serea/actions/runs/37679790571)
  including Linux stable, MSRV 1.85, Intel, arm64, and release fault proof,
  and cross-architecture SQLite
  [37679790526](https://github.com/Yrika819/Serea/actions/runs/37679790526)
  in both directions.
- Exact closure-document head `8981550e12c6cbb1b61800e2bddf6b9eef431f4f`
  passed Fast CI
  [37680369748](https://github.com/Yrika819/Serea/actions/runs/37680369748),
  Full CI
  [37680369751](https://github.com/Yrika819/Serea/actions/runs/37680369751)
  including Linux stable, MSRV 1.85, Intel, arm64, and release fault proof,
  and cross-architecture SQLite
  [37680369792](https://github.com/Yrika819/Serea/actions/runs/37680369792)
  in both directions.
- Exact closure-document head `f5ee0da08a27e35fb9d2a1f711103a89cd366660`
  passed Fast CI
  [37681151729](https://github.com/Yrika819/Serea/actions/runs/37681151729),
  Full CI
  [37681151730](https://github.com/Yrika819/Serea/actions/runs/37681151730)
  including Linux stable, MSRV 1.85, Intel, arm64, and release fault proof,
  and cross-architecture SQLite
  [37681151791](https://github.com/Yrika819/Serea/actions/runs/37681151791)
  in both directions.
- P4D process-crash evidence slice: added a test-only child-process harness
  that kills dispatch at four acknowledged boundaries: after durable intent
  commit before provider entry, after the scripted provider receives the
  request, after a valid response is staged before terminal commit, and after
  completion commit before the caller receives the result. Fresh Store opens
  prove unresolved intents recover once to AMBIGUOUS with reservation
  preserved, no usage, and only MODEL_CALLED/MODEL_FAILED; the committed
  response remains recoverable after caller loss. A separate injected
  pre-commit storage failure proves attempt and event rollback together and
  provider invocation count remains zero. These tests use the existing
  feature-gated Storage crash seam only through a model-router dev-dependency;
  runtime dependency edges and production dispatch code are unchanged.
- RED evidence: the first harness compile failed because the router test target
  could not access Storage's feature-gated crash seam and attempted to call a
  private helper in another test module. Adding the feature only to the
  router's dev-dependency and a local immediate-future test helper resolved the
  compile failures; no production seam was added.
- Focused tests pass for all three crash-harness tests, including four actual
  child-process kill windows, the intent rollback assertion, recovery twice,
  reservation preservation, content-free events, and caller-loss response
  reconstruction. The existing P4D finish-reason and response-identity tests
  remain in the same workspace suite.
- Sequential review: (1) contract review maps the process windows to P4D crash
  cases A-E and response identity remains covered separately; (2) crate graph
  review confirms only a dev feature edge was added, with no runtime
  TaskEngine/Policy/Testkit edge; (3) transaction review verifies the injected
  failed intent transaction leaves no attempt/event and the completion blob,
  usage, terminal state, and event roll back together; (4) recovery review
  verifies child death, fresh Store reopen, one ambiguity transition, and
  second-pass no-op; (5) privacy review confirms only test marker/request ID
  files are used and events remain content-free; (6) accounting review checks
  the full reservation survives ambiguity and usage remains absent; (7) test
  review uses explicit acknowledgement files, SIGKILL proof, bounded waiting,
  and reopened SQLite assertions with no timing-based success; (8) docs review
  records the dev-only seam change, exact predecessor CI evidence, unchanged
  migrations, and P4D-in-progress status.
- Local validation passes: focused crash tests; fmt; workspace check; all
  targets and all features; Clippy with denied warnings; documentation
  validation; workspace smoke; all 76 workspace smoke tests; Cargo metadata;
  identity guard; and `git diff --check`. Existing vendored serde_json emits
  its prior `usize::max_value` deprecation warning; validation succeeds.
- Exact behavior commit `f36bd59b66d9bc67063e692b05aad7d0d5be38c5`
  passed Fast CI
  [37682848836](https://github.com/Yrika819/Serea/actions/runs/37682848836),
  Full CI
  [37682848895](https://github.com/Yrika819/Serea/actions/runs/37682848895)
  including Linux stable, MSRV 1.85, Intel, arm64, and release fault proof,
  and cross-architecture SQLite
  [37682848942](https://github.com/Yrika819/Serea/actions/runs/37682848942)
  in both directions.
- P4D closed on branch at behavior commit
  `f36bd59b66d9bc67063e692b05aad7d0d5be38c5`. The accepted CHAT/TEXT path
  atomically reserves and journals dispatch intent before the provider call,
  binds provider response identity to the host-selected request/model/provider,
  accepts only bounded STOP text, commits usage/cost/response/event together,
  records definite failure and ambiguity without raw diagnostics, and recovers
  unresolved prior-process attempts conservatively without redispatch. The
  crash matrix covers reservation/event rollback, process loss before provider
  entry, provider receipt, response-before-terminal-commit, durable completion
  with caller loss, and response identity mismatch. P4E owns all retry,
  structured validation, repair, fallback and budget orchestration; none is
  exposed as an unsafe retry path from this slice.
- P4D review passes all eight required sequential passes: frozen semantics;
  permitted crate direction and no TaskEngine/Policy/Codex authority; atomic
  Storage/Event Bus composition; real process-death recovery and repeated
  recovery no-op; content-free events and no prompt persistence; validated
  usage with host price and preserved ambiguity reservation; deterministic
  crash/reopen tests without ignored cases or timing-based success; and
  documentation/claims reconciled. No migration changed; 0001, 0002 and 0003
  checksums remain the P4B values. No real provider network or credential is
  present.
- Exact P4D closure-document head
  `59ca466ba2a217d82d16f24506a37a8e7933946f` passed Fast CI
  [37683581338](https://github.com/Yrika819/Serea/actions/runs/37683581338),
  Full CI
  [37683581420](https://github.com/Yrika819/Serea/actions/runs/37683581420)
  including Linux stable, MSRV 1.85, Intel, arm64, and release fault proof,
  and cross-architecture SQLite
  [37683581408](https://github.com/Yrika819/Serea/actions/runs/37683581408)
  in both directions.
- Exact P4D closure-document head
  `47e5dce4119f6b577dbd24ca0b947dded711d576` passed Fast CI
  [37684405955](https://github.com/Yrika819/Serea/actions/runs/37684405955),
  Full CI
  [37684405753](https://github.com/Yrika819/Serea/actions/runs/37684405753)
  including Linux stable, MSRV 1.85, Intel, arm64, and release fault proof,
  and cross-architecture SQLite
  [37684405424](https://github.com/Yrika819/Serea/actions/runs/37684405424)
  in both directions.

## P4E Evidence (closed on branch)

The following records the first P4E behavior slice and its then-current
boundary; later sections record the remaining P4E slices. P4E closed on branch
at behavior commit `6315f424bf18a967fdddeac8d5437af9e5842d38`, with closure
evidence head `558223aa8bef753fbb78a4ba7a84391974663634` passing Fast CI
`37710992048`, Full CI `37710992053`, and cross-architecture SQLite
`37710992056`. The complete P4E behavior and crash evidence is recorded below.

- RED evidence: the first focused structured-validation test compile failed
  because the validator module, response/depth/error bounds, and validation
  entry point did not exist. After adding the minimal implementation, focused
  tests exercise valid/invalid schema output, malformed JSON, top-level,
  nested, and escaped duplicate keys, schema-local references, refused remote
  references, response/schema depth and byte bounds, bounded diagnostic count
  and bytes, and arbitrary-precision JSON numbers.
- The host constructor now compiles JSON Schema before routing or any provider
  health/generate operation. An integration spy test proves an invalid schema
  returns `InvalidJsonSchema` with zero health and generation calls.
- JSON parsing rejects duplicate keys while the parser still has the key
  stream, before constructing `serde_json::Value`. Validation uses Draft
  2020-12. Diagnostics contain a bounded instance path, keyword, and static
  description only; the rejected instance value is never formatted.
- Dependency review: `jsonschema` is pinned to 0.58.3 (MIT, MSRV 1.85) with
  workspace `default-features = false`. Its HTTP, file, async resolver, and TLS
  features are absent from the dependency feature tree. In-process `$defs`
  references validate; remote `$ref` fails closed. No model/network service is
  called.
- Focused validation passes: 5 structured tests and 13 routing integration
  tests; router check and Clippy pass. Workspace validation passes fmt,
  check, all-target tests, all-feature tests, Clippy with denied warnings,
  docs validation, workspace smoke, all 76 workspace smoke unit tests, Cargo
  metadata, identity guard, and diff check. The existing vendored serde_json
  deprecation warning remains non-fatal.
- Sequential review: (1) contract review confirms only host JSON parsing,
  Draft 2020-12 validation and frozen structural bounds are implemented;
  (2) crate graph review confirms protocol/storage/event-bus dependencies are
  unchanged and no Task Engine, Policy or Testkit runtime edge is added;
  (3) transaction review confirms invalid schemas are rejected before route
  selection/provider health and before dispatch; (4) recovery review confirms
  this slice adds no state transition or retry path; (5) privacy review
  confirms diagnostics never include rejected values, prompts or provider
  fragments; (6) bounds review confirms schema/response bytes, JSON depth,
  diagnostic count and encoded diagnostic bytes are bounded; (7) test review
  confirms parser-level nested/escaped duplicate checks and malformed input
  checks are deterministic; (8) docs review records the validator's exact
  dependency configuration and limits the P4E claims to this slice.
- P4E structured-validation behavior commit `be429e6414d8605ec1ac03139cb4d8af008ede76`
  passed exact-head Fast CI
  [37687688591](https://github.com/Yrika819/Serea/actions/runs/37687688591),
  Full CI
  [37687688473](https://github.com/Yrika819/Serea/actions/runs/37687688473)
  with Linux stable, MSRV 1.85, Intel x86_64, arm64, and release fault proof,
  and cross-architecture SQLite
  [37687688431](https://github.com/Yrika819/Serea/actions/runs/37687688431)
  in both directions. The Linux, MSRV, Intel, and arm64 jobs are all GREEN.
- This closes only the first P4E behavior slice. At that point migrations
  0001, 0002 and 0003 were unchanged; later P4E amended unreleased migration
  0003 for durable model-turn accounting and trustworthy task token totals.

### P4E slice: typed structured-output-invalid event

- RED evidence: the new Event Bus integration test failed to compile because
  `ModelOutputInvalidEventV1` and `EventBus::draft_model_output_invalid` were
  absent. The missing typed event surface was then added.
- The event payload carries the common host-selected model metadata and a
  diagnostic count bounded to 32. It has no field for prompt, response,
  validator text, schema, or invalid JSON. Counts above the bound are refused.
- Focused tests pass: five `model_events` integration tests, including exact
  event kind/shape, content-free payload, and over-bound count refusal.
- Sequential review: (1) Event Protocol kind and metadata semantics match
  `MODEL_OUTPUT_INVALID`; (2) the event builder adds no dependency edge or
  authority; (3) it produces only an event draft for the caller's existing
  transaction composition; (4) it creates no dispatch/recovery state; (5) its
  type has no content-bearing fields; (6) count is capped at 32; (7) tests
  verify exact keys and reject 33 diagnostics without timing or platform
  assumptions; (8) this record separates the event payload slice from the
  still-incomplete validation/repair orchestration.
- Local validation passes: fmt, workspace check, all-target tests,
  all-feature tests, denied-warning Clippy, docs validation, workspace smoke,
  all 76 workspace smoke unit tests, Cargo metadata, identity guard, and diff
  check. No migration changed. Exact-head Actions remain pending.
- Exact typed-event behavior commit `e6a15f765e8e21a000449f28b482e5fd3e0103a2`
  passed Fast CI
  [37689403697](https://github.com/Yrika819/Serea/actions/runs/37689403697),
  Full CI
  [37689403699](https://github.com/Yrika819/Serea/actions/runs/37689403699)
  with Linux stable, MSRV 1.85, Intel x86_64, arm64, and release fault proof,
  and cross-architecture SQLite
  [37689403666](https://github.com/Yrika819/Serea/actions/runs/37689403666)
  in both directions.

### P4E slice: host-validated structured dispatch

- RED evidence: the new structured dispatch test first failed to compile
  because `dispatch_structured` did not exist. After adding accepted-output
  recovery coverage, that test again failed to compile because
  `recover_completed_structured_response` did not exist. Both entry points
  were then implemented.
- Structured dispatch remains crate-private. It accepts only non-CHAT
  JSON-Schema prepared calls; routing has already enforced the purpose/format
  matrix and host schema compilation. The provider's `content` is the only raw
  validation source. Provider `structured` and provider repair counts remain
  discarded by identity binding.
- On host-valid STOP output, the router builds `structured` from the uniquely
  parsed, schema-valid JSON value, persists `{content, structured}` with usage,
  cost, completion state, and `MODEL_COMPLETED` in one transaction, and returns
  a host-built response with zero repairs. Reopen recovery reconstructs the
  accepted structured response from committed content and trusted attempt and
  usage facts without a provider call.
- On invalid structured STOP output, the router records trustworthy provider
  usage/cost, marks the attempt FAILED, and appends `MODEL_COMPLETED` followed
  by the typed content-free `MODEL_OUTPUT_INVALID` in one transaction. It
  persists no invalid response. The private typed failure carries bounded
  diagnostics and a transient raw response only in process memory for the
  forthcoming repair ladder; it has no debug formatter and is not logged,
  serialized, placed in events, or stored.
- Focused tests pass for host structured value acceptance despite provider
  spoofed `structured`, persistence/reopen reconstruction, invalid-output
  failure state, usage/cost settlement, absence of a response blob, event
  order, and absence of an invalid-content marker in the event payloads.
- Sequential review: (1) structured purpose/format eligibility is checked at
  the host-prepared routing boundary; (2) router dependencies remain within the
  permitted protocol/storage/event-bus direction; (3) intent/event precede
  provider entry and accepted completion or invalid-output accounting/event
  are each atomic; (4) completion recovery reopens Store and does not redispatch;
  (5) invalid content is absent from durable response, events, and logs while
  diagnostics omit values; (6) provider identity, cost class, token maxima,
  response size, schema/JSON depth, and diagnostic bounds remain enforced;
  (7) scripted tests are deterministic, use reopened SQLite for recovery, and
  assert persisted state; (8) docs limit this slice to structured dispatch and
  explicitly leave repair/fallback and P4E closure open.
- Local validation passes: fmt, workspace check, all-target tests,
  all-feature tests, denied-warning Clippy, docs validation, workspace smoke,
  all 76 workspace smoke unit tests, Cargo metadata, identity guard, and diff
  check. Migrations 0001, 0002 and 0003 are unchanged. Exact-head Actions remain
  pending.
- Exact structured-dispatch behavior commit
  `49dc33ad5ed8a169104e10f18ad2aa51f093171b` passed Fast CI
  [37691539096](https://github.com/Yrika819/Serea/actions/runs/37691539096),
  Full CI
  [37691539042](https://github.com/Yrika819/Serea/actions/runs/37691539042)
  with Linux stable, MSRV 1.85, Intel x86_64, arm64, and release fault proof,
  and cross-architecture SQLite
  [37691539045](https://github.com/Yrika819/Serea/actions/runs/37691539045)
  in both directions.

### P4E slice: durable model-turn accounting

- RED evidence: the primary/fallback/repair test first failed to compile because
  `Store::task_model_turn_count` did not exist. Migration v2 upgrade coverage
  then pins that prior Tasks start at zero and rejects values outside 0..12.
- Amendment to unreleased migration 0003: added durable
  `tasks.model_turn_count INTEGER NOT NULL DEFAULT 0 CHECK (0..12)`. Schema
  version remains 3; migrations 0001 and 0002 are unchanged. The 0003 checksum at task start was
  `8f4c5c4e047a8829201ce834ff192d740ab6ca199ecc3d12dfe8e357cf82c2ec`; the
  amended current checksum is
  `bd85c804c832e58a520c070ab6b1cf0d7ad15515f44d1f8c3a161366c3d9d078`.
- Every committed dispatch intent increments durable model-call count. Only
  relation `NONE` increments durable model-turn count; `FALLBACK` and `REPAIR`
  consume call units without consuming additional turns. Both counters persist
  independently of terminal attempt pruning. The task-owned counter is removed
  only with the Task itself.
- Focused tests pass for primary/fallback/repair accounting, terminal attempt
  pruning with durable counters retained, a second primary turn, v2-to-v3
  default-zero upgrade, and SQL range constraints. The full P4B migration test
  module passes with exact 0001/0002 and amended 0003 checksums.
- P4B evidence is amended for an already-frozen P4 Bounds requirement, not a
  new semantic decision. 0003 start/end checksums are recorded above; 0001 and
  0002 remain unchanged. This amendment adds no fourth migration and leaves the
  schema version at 3.
- Sequential review: (1) the 12 turn bound and primary-only definition match
  the supplied Bounds contract; (2) the field belongs to Storage's Task row
  and adds no upward crate edge; (3) counter increment shares the dispatch
  intent/call reservation transaction; (4) the counter survives close/reopen
  and detail retention; (5) it stores no prompt or model content; (6) SQL
  bounds, call/turn relation, and existing 12-call limit are checked; (7)
  tests use deterministic terminal attempts and SQLite retention, not sleeps;
  (8) P4B/P4E evidence and checksums are reconciled while schema remains v3.
- Required local validation passes: fmt, workspace check, all-target tests,
  all-feature tests, denied-warning Clippy, docs validation, workspace smoke,
  all 76 smoke unit tests, Cargo metadata, identity guard, and diff check. The
  local portability fixture produces and reopens v3 on this host with durable
  call/turn counters. Exact-head Actions are green on behavior commit
  `f43b420d267d524b0b4fe7c2e0929f647dd37387`: Fast CI
  [37693502078](https://github.com/Yrika819/Serea/actions/runs/37693502078), Full CI
  [37693502077](https://github.com/Yrika819/Serea/actions/runs/37693502077), and cross-architecture SQLite
  [37693502118](https://github.com/Yrika819/Serea/actions/runs/37693502118). Full CI passed Linux stable,
  MSRV 1.85, Intel x86_64, arm64, and release fault proof; cross-architecture
  passed Intel-to-arm64 and arm64-to-Intel. The evidence-only closure head still
  requires its own Fast, Full, and cross-architecture Actions gates.

### P4E slice: fresh dispatch gate

- RED evidence: the new dispatch-gate test first failed to compile because the
  snapshot type, resolver, and typed refusal outcomes did not exist. After the
  seam was wired, the Personal cloud test exposed that the old deployment
  filter ignored the prepared egress fact; it now rejects Personal cloud when
  either initial or refreshed host egress permission is false.
- Added a narrow `ModelDispatchGateSource` that receives only optional `TaskId`
  and data class, and returns typed host-resolved cancellation, egress, and
  deadline facts. Router does not evaluate policy or query Task Engine. It
  checks cancellation and remaining deadline before allocating a request ID or
  writing dispatch intent, intersects fresh egress with the prepared fact, and
  only narrows the prepared deadline before passing it to the provider. It
  rechecks the gate after intent COMMIT immediately before provider invocation;
  if facts changed, one transaction marks the known-unsent attempt FAILED,
  settles spend to zero without synthetic usage, and appends MODEL_FAILED.
- Focused evidence: dispatch-gate cancellation/expiry/egress refusal test,
  cancellation-before-intent provider/event assertion, cancellation-after-intent
  failure/zero-settlement/provider assertion, Personal cloud egress routing
  regression, full model-router unit/integration/doc tests, and existing dispatch
  crash suite all pass. The call unit remains consumed; no provider invocation
  occurs on either cancellation path.
- Sequential review: (1) pre-intent refusal writes no intent; post-intent
  refusal closes the known-unsent attempt before provider generate;
  (2) the seam has no Policy, Task Engine, Storage callback, or Event Bus edge;
  (3) intent and MODEL_CALLED retain their atomic composition, then the
  post-intent failure/state/event and zero spend settlement share one
  transaction; (4) no retry path exists yet and crash tests still pass;
  (5) the seam receives no prompt or raw content and never widens egress;
  (6) a refreshed deadline is min-bounded by the prepared deadline; (7) tests
  are deterministic and assert no called event/provider operation on cancel;
  (8) this section records the discovered egress filter correction and keeps
  fallback/repair and complete budget claims open.
- Local validation passed: fmt, workspace check, all-target tests, all-feature
  tests, denied-warning Clippy, docs validation, workspace smoke, all 76 Python
  smoke tests, Cargo metadata, identity guard, and diff check. The repository's
  CI smoke entry point is `tests/workspace_smoke.py`; the task-named
  `tools/workspace_smoke.py` does not exist in this checkout. Behavior commit
  `b1b70b97add046ee65af3e26985f4ce62f9ccceb` passed Fast CI
  [37695570012](https://github.com/Yrika819/Serea/actions/runs/37695570012), Full CI
  [37695569990](https://github.com/Yrika819/Serea/actions/runs/37695569990), and cross-architecture SQLite
  [37695570019](https://github.com/Yrika819/Serea/actions/runs/37695570019), including Linux stable,
  MSRV 1.85, Intel x86_64, arm64, release fault proof, and both SQLite transfer
  directions. The closure-document head `27743c9588b144316542ad98c73969eceaa2e518`
  passed Fast CI
  [37696372204](https://github.com/Yrika819/Serea/actions/runs/37696372204), Full CI
  [37696372072](https://github.com/Yrika819/Serea/actions/runs/37696372072), and cross-architecture SQLite
  [37696372051](https://github.com/Yrika819/Serea/actions/runs/37696372051), with the same required job set and
  both transfer directions. The evidence-only head
  `4cceb88568ca3e81aba68c4bf8667b7d94bd6312` passed Fast CI
  [37696983501](https://github.com/Yrika819/Serea/actions/runs/37696983501), Full CI
  [37696983526](https://github.com/Yrika819/Serea/actions/runs/37696983526), and cross-architecture SQLite
  [37696983716](https://github.com/Yrika819/Serea/actions/runs/37696983716), including Linux stable, MSRV 1.85,
  Intel x86_64, arm64, release fault proof, and both transfer directions.
  The evidence-only head `97e7b3ca7ba08bbd34f6417c366df4b5f952d54f` passed Fast CI
  [37697801311](https://github.com/Yrika819/Serea/actions/runs/37697801311), Full CI
  [37697801437](https://github.com/Yrika819/Serea/actions/runs/37697801437), and cross-architecture SQLite
  [37697801320](https://github.com/Yrika819/Serea/actions/runs/37697801320), including Linux stable, MSRV 1.85,
  Intel x86_64, arm64, release fault proof, and both transfer directions.
  The evidence-only head `87f7ef2eed18b082806f6143cc0c61eb1b505ced` passed Fast CI
  [37698390427](https://github.com/Yrika819/Serea/actions/runs/37698390427), Full CI
  [37698390122](https://github.com/Yrika819/Serea/actions/runs/37698390122), and cross-architecture SQLite
  [37698390313](https://github.com/Yrika819/Serea/actions/runs/37698390313), including Linux stable, MSRV 1.85,
  Intel x86_64, arm64, release fault proof, and both transfer directions.
  The evidence-only head `f8b9e17be89fd6e2de66fa94cccea7ce0edbb167` passed Fast CI
  [37698906914](https://github.com/Yrika819/Serea/actions/runs/37698906914), Full CI
  [37698906899](https://github.com/Yrika819/Serea/actions/runs/37698906899), and cross-architecture SQLite
  [37698906898](https://github.com/Yrika819/Serea/actions/runs/37698906898), including Linux stable, MSRV 1.85,
  Intel x86_64, arm64, release fault proof, and both transfer directions.
  The evidence-only head `acc83d3264e2ca006f65e0aa814bfe4ce8e1c9b0` passed Fast CI
  [37699472545](https://github.com/Yrika819/Serea/actions/runs/37699472545), Full CI
  [37699472495](https://github.com/Yrika819/Serea/actions/runs/37699472495), and cross-architecture SQLite
  [37699472496](https://github.com/Yrika819/Serea/actions/runs/37699472496), including Linux stable, MSRV 1.85,
  Intel x86_64, arm64, release fault proof, and both transfer directions.
  The closure-document head `2efa263f2ed14c0fdb11851bc5ff0a2b21d80041` passed Fast CI
  [37700100583](https://github.com/Yrika819/Serea/actions/runs/37700100583), Full CI
  [37700100740](https://github.com/Yrika819/Serea/actions/runs/37700100740), and cross-architecture SQLite
  [37700100744](https://github.com/Yrika819/Serea/actions/runs/37700100744), including Linux stable, MSRV 1.85,
  Intel x86_64, arm64, release fault proof, and both transfer directions.
  The current evidence head `0ba1d8509b8839addfa0fd462ea3902aa56fc8b7` passed
  Fast CI [37700566080](https://github.com/Yrika819/Serea/actions/runs/37700566080),
  Full CI [37700566010](https://github.com/Yrika819/Serea/actions/runs/37700566010),
  and cross-architecture SQLite
  [37700565986](https://github.com/Yrika819/Serea/actions/runs/37700565986).
  Full CI passed Linux stable, MSRV 1.85, Intel x86_64, arm64, and release
  fault-seam exclusion; cross-architecture passed Intel-to-arm64 and
  arm64-to-Intel. This closes validation for the fresh-dispatch-gate evidence
  head. It does not close P4E repair, fallback, or remaining budget work.

### P4E slice: bounded structured repair

- RED evidence: the deterministic router test failed to compile because
  `dispatch_structured_with_repair` did not exist. The failure isolated the
  missing orchestration behavior; no production implementation was present.
- Added a crate-private structured repair ladder. It uses only the configured
  `gpt-oss-20b` candidate after one fresh provider-health read, and makes at
  most two actual repair dispatches. Each dispatch gets a new request ID,
  `REPAIR` relation and parent RequestId; every attempt passes fresh cancellation,
  deadline and egress gates and uses the process price snapshot and normal call
  and spend reservation path.
- The repair request contains only the original validated schema, one bounded
  invalid raw response, and bounded value-free validator diagnostics. It has
  no conversation, system prompt, tools, or task history. The original schema
  is re-applied to every repair result by the host. Provider `structured` and
  repair-count fields remain untrusted.
- Invalid and definite failed repair attempts consume a repair dispatch. A
  definite failure may use the second attempt; ambiguity, cancellation,
  deadline/egress refusal, content filter, LENGTH, budget failure, and
  oversized invalid output stop the ladder. Oversized content is never
  truncated to create a repair payload. Two invalid repair outputs stop after
  the second attempt.
- Provider `STRUCTURE_INVALID` is not treated as a schema verdict. Bounded
  response content still goes through host parsing and validation before the
  host decides whether to accept it or enter repair.
- Added typed, content-free `MODEL_REPAIRED`. It is appended atomically with
  accepted response completion. The event carries host metadata and the
  host-derived repair count only. Attempt usage and the returned
  `ModelResponse.repair_attempts` use the host ordinal; recovery reconstructs
  the accepted repaired response from durable completion facts.
- Focused evidence: the scripted provider test covers repaired success,
  minimal repair context, provider diagnostic/content exclusion from events,
  primary-to-repair lineage, definite first repair failure followed by second
  repair success, two definite repair failures mapping to hard validation
  failure, ambiguous repair stopping immediately, two invalid repair results
  stopping at the maximum, one repair health read per ladder, degraded repair
  health, cancellation before repair intent, oversized output refusal, and
  reopen recovery of the accepted repaired result. Router and Event Bus test
  suites pass.
- Sequential review: (1) fixed model and strict schema capability check;
  Codex remains unreachable; (2) no crate direction changed; (3) initial
  invalid accounting and events stay atomic, and successful repair completion
  plus `MODEL_COMPLETED`/`MODEL_REPAIRED` share one transaction; (4) retry
  ordinals, parent links, fresh gates, and health snapshot are deterministic;
  (5) invalid output appears only in the transient trusted repair request and
  never in durable response, usage, or events; (6) every repair reserves and
  settles through the existing accounting path, with a hard two-dispatch cap;
  (7) tests use scripted trait providers, independent SQLite reopen, and no
  wall-clock sleeps; (8) migration 0003 and its checksum are unchanged.
- Local validation passed: fmt; workspace check; all-target and all-feature
  tests; denied-warning Clippy; docs validation; workspace smoke; all 76 Python
  smoke tests; Cargo metadata; identity guard; and diff check. Behavior commit
  `cfa507ce20d6f022f5a2b335e3c7775599debea8` passed Fast CI
  [37702723229](https://github.com/Yrika819/Serea/actions/runs/37702723229), Full CI
  [37702723064](https://github.com/Yrika819/Serea/actions/runs/37702723064), and
  cross-architecture SQLite
  [37702723224](https://github.com/Yrika819/Serea/actions/runs/37702723224).
  Full CI passed Linux stable, MSRV 1.85, Intel, arm64, and release-fault
  exclusion; cross-architecture passed Intel-to-arm64 and arm64-to-Intel.
  This closes the bounded structured-repair slice. Normal fallback, all budget
  integration, repair crash injection, and P4E as a whole remain open.

## P4E slice: one-step normal fallback

- RED proof: the scripted test first failed to compile because
  `dispatch_chat_text_with_fallback` was absent. It exercises a retryable
  primary `ModelError`, successful configured fallback, non-retryable primary
  failure, ambiguous primary failure, a fallback provider failure, persisted
  lineage, event order, privacy, and one health snapshot per logical operation.
- Implemented a CHAT/TEXT fallback ladder using only the accepted frozen chain.
  It advances to the first remaining candidate that passes the same prepared
  call filters and the initial immutable health snapshot. Codex and all other
  models outside the chain remain unreachable.
- For eligible retryable primary failures, the Router constructs the fallback
  request and events, then commits primary FAILED plus `MODEL_FAILED`,
  `MODEL_FALLBACK`, the fallback call reservation, and `MODEL_CALLED` in one
  Storage transaction. The fallback provider runs only after that commit.
  The child attempt carries `FALLBACK`, the primary RequestId, and the primary
  model ID. If fresh gate resolution or reservation prevents the fallback
  transaction, no child survives and the primary failure is terminalized.
- The fresh dispatch gate is read for the fallback before creating its intent
  and once again before the provider call. No provider health read occurs after
  the initial route snapshot. A fallback failure emits the content-free
  `MODEL_FALLBACK_EXHAUSTED`; there is no third normal dispatch.
- Focused tests prove successful fallback, distinct request IDs, stored
  parent/source model, primary/fallback event sequence, one health read reused
  despite scripted health changing on a subsequent read, exhausted fallback,
  no fallback after a non-retryable error or ambiguity, and diagnostic privacy.
- Sequential review: (1) only definite retryable adapter errors enter the
  fallback path; (2) no dependency direction changed; (3) parent failure,
  fallback decision, child reservation, and dispatch event share one
  transaction; (4) the child is intent-committed before provider invocation
  and the normal snapshot is reused; (5) event payloads contain no prompt,
  provider diagnostic, or response content; (6) the child uses normal call
  reservation and spend accounting and does not consume another turn; (7) the
  test uses scripted outcomes and fixed time with SQLite persistence; (8)
  no schema or migration changed. Structured fallback, fallback/repair
  interaction, crash injection, and remaining P4E budget integration remain
  open.
- Initial denied-warning Clippy identified an iterator-style lint and an
  oversized dispatch-helper signature. The scan now uses slice iteration and
  per-attempt relation facts are grouped in a private `DispatchLineage`.
  Final local validation passed fmt, workspace check, all-target tests,
  all-feature tests, denied-warning Clippy, docs validation, workspace smoke,
  76 Python smoke tests, Cargo metadata, identity guard, and diff check.
- Behavior commit: `c6296901a9dcdfe4abc00e8cfee6529ede447964`
  (`feat: add deterministic model fallback`). Fast CI
  [37704755840](https://github.com/Yrika819/Serea/actions/runs/37704755840)
  passed; Full CI
  [37704755890](https://github.com/Yrika819/Serea/actions/runs/37704755890)
  passed Linux stable, MSRV 1.85, Intel, arm64, and release-fault exclusion;
  cross-architecture SQLite
  [37704755834](https://github.com/Yrika819/Serea/actions/runs/37704755834)
  passed Intel-to-arm64 and arm64-to-Intel. No migration changed. This closes
  the CHAT/TEXT one-step fallback behavior slice; structured fallback,
  fallback/repair interaction, crash injection, and remaining P4E budgets stay
  open.

## P4E slice: structured fallback into repair

- RED proof: a scripted test initially failed to compile because
  `dispatch_structured_with_fallback_and_repair` did not exist. The test scripts
  a retryable primary error, invalid structured fallback content, then a
  schema-valid repair response.
- Structured normal calls now enter the same one-step fallback state machine
  as CHAT/TEXT calls. If the fallback returns host-invalid structured content,
  that attempt is usage-accounted and emits `MODEL_OUTPUT_INVALID`; the repair
  ladder then starts with the fallback RequestId as parent. No second normal
  fallback occurs during repair.
- The initial normal health snapshot is reused for fallback. The repair ladder
  obtains its own one-time health snapshot as defined by ADR-0033. The repair
  request carries only the schema, bounded invalid fallback output, and
  sanitized validator diagnostics; conversation, system text, and tools are
  excluded. Provider content remains transient and does not enter response
  storage or events.
- Focused test evidence: scripted primary retryable failure, successful child
  fallback intent, invalid fallback schema output with usage accounting and no
  response blob, repair parent bound to the fallback RequestId, minimal repair
  context, one normal snapshot plus one repair snapshot, content-free event
  sequence including fallback exhaustion, and reopened recovery of the
  accepted repaired response. The existing primary-invalid repair test remains
  green and still dispatches repair without normal fallback.
- Sequential review: (1) only the primary definite retryable adapter error
  creates fallback; structured invalidity starts repair only; (2) no crate
  direction or dependency changed; (3) the primary failure and fallback child
  intent remain atomic, and response/usage/event terminal facts use existing
  transactions; (4) normal and repair health snapshots are distinct and each
  reused only for its defined ladder; (5) invalid fallback bytes are transient
  repair input and absent from events/storage; (6) fallback and repair each
  consume normal call/spend accounting while repair does not add a model turn;
  (7) tests use scripted provider outcomes and reopen SQLite without sleeps;
  (8) no schema or migration changed. The behavior slice closes only with the
  exact-head Fast, Full, and cross-architecture results recorded below.
- Local validation passed on this change: fmt, workspace check, all-target
  tests, all-feature tests, denied-warning Clippy, docs validation, workspace
  smoke, 76 Python smoke tests, Cargo metadata, identity guard, and diff check.
- Behavior commit `0ca253814921728cfc65af66af339af52b64bde1` passed Fast CI
  [37705935655](https://github.com/Yrika819/Serea/actions/runs/37705935655),
  Full CI
  [37705935661](https://github.com/Yrika819/Serea/actions/runs/37705935661),
  and cross-architecture SQLite
  [37705935651](https://github.com/Yrika819/Serea/actions/runs/37705935651).
  Full CI passed Linux stable, MSRV 1.85, Intel, arm64, and release-fault
  exclusion; both cross-architecture transfer directions passed. This closes
  structured fallback/repair composition; crash injection and remaining P4E
  budget integration remain open.

## P4E slice: durable task token bound

- RED proof: before adding the task-owned token total, the storage retention
  test observed `TokenCount(0)` after the only `model_usage` row was pruned,
  despite ten trustworthy tokens having been settled. The router test also
  initially returned success after a completion crossed the 128,000-token
  bound.
- Migration 0003 adds `tasks.model_token_count`; storage increments it
  atomically with trustworthy completed or definite-failure usage. The
  `task_model_token_usage` API now reads the durable counter, and integrity
  checking rejects a counter below retained usage detail. On a threshold
  crossing, the router persists the provider result and accounting first,
  emits content-free `BOUND_EXCEEDED`, then returns a typed token-bound
  outcome. A later dispatch is refused before intent creation. The same check
  applies after a provider finish reason with trustworthy usage.
- Focused evidence: migration v2-to-v3 defaults the new counter to zero and
  rejects negative values; storage proves the total remains ten after usage
  pruning; router proves a response crossing the bound remains durably
  completed with 128,000 accounted tokens and returns a bound outcome;
  Event Bus verifies frozen bound event naming and content-free payload.
- Sequential review: (1) threshold is the frozen 128,000 trustworthy total;
  (2) the router still depends only on Storage, Event Bus and Protocol, with
  no Task Engine or Policy dependency; (3) token count, usage and terminal
  attempt commit atomically; the bound event follows that committed result;
  (4) the counter survives retention and uses SQLite checked arithmetic;
  (5) only numeric accounting metadata enters the event; (6) no provider
  response is used as token authority outside validated usage; (7) tests
  reopen and retain SQLite state without timing dependence; (8) migration
  checksum and P4B evidence are reconciled above.
- Local validation passed: fmt, workspace check, all-target tests, all-feature
  tests, denied-warning Clippy, docs validation, workspace smoke, 76 Python
  smoke tests, Cargo metadata, identity guard and `git diff --check`.
- Behavior commit `8066062a2985909b6ca33a2e0c37a9957b41fa02` passed exact-head
  Fast CI
  [37707970712](https://github.com/Yrika819/Serea/actions/runs/37707970712),
  Full CI
  [37707970732](https://github.com/Yrika819/Serea/actions/runs/37707970732),
  and cross-architecture SQLite
  [37707970710](https://github.com/Yrika819/Serea/actions/runs/37707970710).
  Full CI passed Linux stable, MSRV 1.85, Intel x86_64, arm64 and release
  fault-seam exclusion. Cross-architecture database validation passed
  Intel-to-arm64 and arm64-to-Intel. The v3 portability fixture transferred
  only the closed database file; no `-shm` was transferred and no live-WAL
  portability claim is made.
- Nonclaims: this slice does not close model-call/turn/daily-spend bound event
  integration, P4E crash matrix, or P4E as a whole.

## P4E slice: dispatch budget refusal events

- RED proof: the daily-spend dispatch test first failed to compile because the
  router had no typed generic `BoundExceeded` outcome. The call-budget test
  was also run with storage-budget translation disabled and failed because
  the router returned raw `StoreError::ModelCallBudgetExceeded` rather than a
  typed host outcome and `MODEL_BUDGET_EXHAUSTED` event.
- Daily reservation refusal now emits content-free `BOUND_EXCEEDED` with the
  configured `max_daily_spend_usd` and the attempted total of current UTC-day
  occupancy plus this dispatch reservation. It returns a typed generic bound
  outcome and creates no dispatch intent or provider call. Per-task call
  exhaustion emits `MODEL_BUDGET_EXHAUSTED` with the effective task call limit
  and durable observed call count, then returns a typed budget outcome. The
  task call limit lookup applies the frozen twelve-call ceiling to the
  Task-owned configured limit.
- Focused evidence: daily spend refusal leaves provider invocation count
  unchanged and appends the bounded event; a task seeded with twelve failed
  calls returns a typed call-budget outcome, emits exactly one
  `MODEL_BUDGET_EXHAUSTED`, and never calls the scripted provider; Event Bus
  checks frozen event kinds, payload bounds, and a configured zero daily-spend
  limit; existing storage tests cover atomic call-count limits and races.
- Sequential review: (1) call counts include every committed dispatch intent;
  daily cost checks use the same host price snapshot and UTC-day occupancy as
  reservation; (2) Store remains independent of Event Bus and the router adds
  no Policy/Task Engine edge; (3) a failed intent transaction rolls back
  before the bounded event is appended, leaving no half attempt; (4) the
  monotone call count and spend occupancy support concurrent refusals; (5)
  events carry only task/data-class and integer bound metadata; (6) call and
  spend failures do not consume another provider dispatch; (7) tests use
  deterministic provider doubles and no timing; (8) migration 0003 is
  unchanged in this slice. Denied-warning Clippy flagged an overlong helper
  parameter list; the immutable accounting inputs were grouped into a private
  dispatch facts struct, with no behavior change.
- Local validation passed: fmt, workspace check, all-target tests, all-feature
  tests, denied-warning Clippy, docs validation, workspace smoke, 76 Python
  smoke tests, Cargo metadata, identity guard and `git diff --check`.
  Exact-head CI follows.
- Behavior commit `63cd6d5bd084863c0905554b6d6effbfa630d3ae` passed Fast CI
  [37709066254](https://github.com/Yrika819/Serea/actions/runs/37709066254),
  Full CI
  [37709066294](https://github.com/Yrika819/Serea/actions/runs/37709066294),
  and cross-architecture SQLite
  [37709066262](https://github.com/Yrika819/Serea/actions/runs/37709066262).
  Full CI passed Linux stable, MSRV 1.85, Intel x86_64, arm64 and release
  fault-seam exclusion. Both database transfer directions passed. No migration
  changed in this slice.
- Nonclaims: the integrated P4E crash matrix and P4E closure remain open.

## P4E slice: fallback and repair crash windows

- RED proof: the process-death parent test was added first for the fallback,
  repair, and second-repair intent windows. It failed with
  `child missed the crash acknowledgement` because the scripted child did not
  yet expose these provider outcomes or transaction boundaries. The child
  provider now scripts only deterministic protocol outcomes through the real
  `ModelProvider` trait and parks at named storage COMMIT boundaries.
- Process-death coverage now includes: (A) kill during the atomic primary
  failure/fallback decision before COMMIT, then reopen and prove there is no
  fallback child and recovery marks the primary ambiguous; (B) kill after the
  atomic fallback child intent COMMIT but before its provider call, then prove
  the fallback child becomes ambiguous; (C) invalid primary structured output,
  repair intent COMMIT, and process loss before repair provider dispatch; and
  (D) invalid first repair output, second repair intent COMMIT, and process loss
  before that second provider dispatch. The call count records show no child
  provider call after intent COMMIT and no third repair call. All cases reopen
  SQLite, recover once, then recover again as a semantic no-op.
- Existing completion recovery coverage now includes both structured repair
  and successful CHAT/TEXT fallback results after dropping the caller result,
  closing the Store, and reopening by the accepted child RequestId. Event
  sequences are checked for each crash stage; prompts and raw invalid
  structured output are absent.
- Sequential review: (1) the tested stages match the frozen fallback and
  repair contract; (2) no crate dependency or production behavior changed;
  (3) the fallback decision is one transaction and child intents recover
  conservatively; repair intent is durable before provider operation; (4)
  tests kill real child processes at file-acknowledged transaction stages and
  do not use sleeps as outcome authority; (5) event payload checks reject
  prompt/output leakage; (6) no synthetic usage is created on recovery; (7)
  reopened state and second recovery are asserted; (8) this is test/evidence
  work only. Provider call-count files are asserted so a child provider call
  after its intent COMMIT cannot be hidden. Local fmt, workspace check, both
  workspace test commands, Clippy, docs validation, workspace smoke, Python
  workspace tests, metadata, identity guard, and diff check passed. Exact-head
  GitHub Actions passed for behavior commit
  `6315f424bf18a967fdddeac8d5437af9e5842d38`: Fast run `37710481752` (Linux
  fast checks), Full run `37710481746` (Linux stable, MSRV 1.85.0, Intel,
  arm64, and release fault-seam exclusion), and cross-architecture run
  `37710481790` (Intel-produced DB opened on arm64 and arm64-produced DB opened
  on Intel). No migration changed in this slice.
- Nonclaims: P4F caller/TaskEngine integration, the full concurrency matrix,
  and whole-branch review remain open.

## P4F CLOSED ON BRANCH: trusted host boundary and integrated concurrency

- RED proof: the host-boundary integration test was added before the public
  process router and host context existed; it failed to compile on the missing
  `ModelRouterHostContextV1` and `ModelRouterProcessV1::execute` API. The
  initial event rollback test was also run with an injected pre-COMMIT failure
  and failed until the deterministic Store fault seam was connected to the
  dispatch-intent transaction. No test calls a live provider.
- The production entry point now binds one immutable process router, provider
  registry, per-model price snapshots, and daily spend ceiling. Its trusted
  in-process host context supplies Store, Event Bus, Clock, and resolved
  dispatch-gate facts. Router owns dispatch sequencing and typed outcomes;
  TaskEngine/Core retains task lifecycle transitions. No Task Engine edge was
  added. Process reconstruction is required to change the roster or prices;
  tests show in-flight and later calls retain their respective process price
  revisions.
- Integrated evidence uses real persisted Task records and independent Store
  connections. It covers TaskId accounting and one durable top-level turn,
  cancellation before and after intent, deletion before dispatch, late
  response accounting after cancellation, cancellation blocking fallback and
  repair, same-Task active-call exclusion, distinct-Task concurrency, daily
  spend reservation races, the final call/turn slot race, two-worker recovery,
  and per-model fallback prices with one reused health snapshot. Existing
  Storage race tests pin completion-versus-ambiguity linearization. Fault
  tests prove event/intent rollback before provider invocation and terminal
  response blob/usage rollback followed by conservative ambiguity recovery.
- Sequential reviews: (1) contract: caller-visible failure types omit raw
  provider output, and call, turn, Task lifecycle, cancellation, deadline,
  price, and health behavior follow the frozen P4 contracts; (2) crate graph:
  Router has no TaskEngine or Policy dependency, and Storage still has no Event
  Bus edge; the workspace smoke check confirms the graph; (3) atomicity:
  existing fixed Storage/Event Bus transaction composition is retained, and
  fault tests inspect reopened durable state; (4) concurrency/recovery: all
  newly added races use independent SQLite connections and channel/barrier
  coordination, with no sleeps; Clippy's misleading non-`Drop` context drops
  were removed; (5) privacy: event and error surfaces carry bounded metadata,
  provider structured fields remain non-authoritative, and prompt content is
  not persisted; (6) bounds/accounting: process prices are validated for every
  roster entry, fallback resolves its own model price, committed calls and
  turns are monotone, and daily spend remains reserved before dispatch; (7)
  test quality: the new crash tests assert provider counts, attempts, events,
  usage, response references, and recovery state; no ignored tests, sleeps,
  wall-clock dependencies, or platform skips were introduced; (8) docs: this
  section records the current P4F boundary and validation status. The only
  review fixes were removing test-only explicit drops flagged by denied-warning
  Clippy and a needless borrow, and replacing the crash-test sleep poll with an
  explicit durable file acknowledgement loop that checks child exit and yields
  without a time threshold. No contract or migration changes were needed.
- Local validation passed after these changes: `cargo fmt --all -- --check`,
  offline workspace check, all-target workspace tests, all-feature workspace
  tests, denied-warning Clippy, docs validation, workspace smoke, 76 Python
  workspace-smoke tests, Cargo metadata, commit identity guard, and
  `git diff --check`. The vendored serde_json deprecation warning remains
  upstream; it does not fail these commands. Exact-head authoritative Actions
  results for the P4F behavior commit follow.
- Exact behavior commit `cb3d9e64721c6650e42f6f0ddb5168e01a87b0e3` passed Fast
  CI [37713666086](https://github.com/Yrika819/Serea/actions/runs/37713666086),
  Full CI
  [37713666097](https://github.com/Yrika819/Serea/actions/runs/37713666097),
  and cross-architecture SQLite
  [37713666106](https://github.com/Yrika819/Serea/actions/runs/37713666106).
  Full CI passed Linux stable, MSRV 1.85, Intel x86_64, arm64, and release
  fault-seam exclusion. Cross-architecture passed both Intel-to-arm64 and
  arm64-to-Intel; the workflow transferred closed database fixtures, without
  `-shm` or any live-WAL portability claim.
- Whole-branch review of `origin/main...p4/model-router`: **BLOCKER 0, MAJOR
  0, MINOR 0, NOTE 1**. The note is the pre-existing vendored serde_json
  deprecation warning; denied-warning Clippy remains green. The audit found
  only migration 0003 changed among migration files; its final checksum is
  `530a6d6cb5ec9c757311d48e10a62ef456d9d01c09512f321cffe42fe3307f80`.
  Migrations 0001 and 0002 are unchanged. Workspace graph, release fault-seam
  exclusion, all P4C–P4E slice evidence, and final source/docs claims were
  reviewed together.
- Final security review found no route to Codex, PRIVATE, SECRET, or CREDENTIAL
  dispatch; no provider capability, identity, or cost widening; no model
  self-selection or model-controlled fallback; no recursive repair or third
  normal candidate; no remote schema resolution; no prompt persistence or raw
  provider output in events/usage; and no external model dependency, credential,
  or network call. The host call context remains trusted in-process; Router
  does not depend on Policy or TaskEngine and does not change Task lifecycle.
- P4F is closed on this branch. The exact closure evidence head
  `30fec93e5ef1077ff6c7075d96326fb677c5b13d` passed Fast CI `37714321233`,
  Full CI `37714321119`, and cross-architecture SQLite `37714321114`. P4 is
  `CLOSED_ON_BRANCH` at that validated head. This closure note is
  documentation-only; PR Ready status is applied only after its exact head
  also passes the required Actions. No local Mac or Local MCP was used; no
  merge or P5 work occurred.
