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

## P4E Progress

P4E remains in progress. The first behavior slice adds the host-side structured
JSON validation boundary; it does not yet implement response acceptance,
repair, fallback, dispatch gates, or P4E budgets.

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
- This is not a P4E closure and has no behavior commit or authoritative Actions
  result yet. Migrations 0001, 0002 and 0003 are unchanged.
