# Cloud C-08 Integration Readiness

## Accepted inputs and Git boundary

- C-06R: PR #269, head `c0f3d46de455ff3862b93a48d809989dc41f0760`.
- C-07: PR #271, head `35afb9abb03e21acf4cc72fea7e72b5ecc579dcc`.
- Android Formal inventory recovery: PR #270, head `30b6e6e92d389398f0b41e172e9f9ec5470a769a`.
- Candidate branch: `cloud/serea-v13-c08-integration-readiness`, based on the exact C-07 head above. PR #270's single inventory correction commit is merged into this candidate by a normal Git merge; its nine-file diff applied cleanly.

No existing PR branch, `main`, or existing PR state was changed. ADR-0043 remains the development-only boundary and ADR-0044 remains **PROPOSED / NOT RATIFIED**.

## Existing CI evidence reviewed

- PR #269 C-06R: Full CI run `45992802494` and Fast CI `45992802484` completed successfully on the exact C-06R head. Rust Dependency Security `45992802406` and Git hooks `45992802435` also passed. Native validation `45992802561` and SQLite portability `45992802438` were still in progress at the last observation.
- PR #271 C-07: Fast CI run `45992921276` passed. Full CI run `45992921323` ran its test suites successfully, then failed at Clippy: `experimental_memory.rs::P7ExperimentalMemory::retrieve_context` exceeded the lint argument limit. SQLite portability `45992921287`, Rust Dependency Security `45992921571`, and Git hooks `45992921260` passed; Native validation `45992921425` remained queued. The Android workflow `45992921293` was a successful branch-scope run with product jobs skipped; it provides no Android Formal evidence.
- PR #270 Android run `45989860032`: Build/lint/instrumentation compile and APK provenance passed. API 26 executed 106/106 tests, 0 failures/errors/skips; Original Required 103/103 and Additional 3/3. API 35 executed the same 106/106 with zero failures/errors/skips and the same manifest totals. Android Storage and Emulator independently validated both API artifacts and the same-run Build evidence. Android Unit and Android Integration failed before tests when Maven Central returned HTTP 429 for Kotlin/KSP dependencies; their test suites were not run. The overall Android Emulator Required Gate failed because those required jobs failed. This is an external dependency-resolution failure, not a product assertion failure. Android Formal runtime evidence is PASS for both APIs on this run; full Android Required is not PASS.

The C-07 Japanese comparison remains unchanged: bundled SQLite 3.54.1; current `unicode61` measured 200/1,100 exact queries, 500/1,600 relevant rows, zero candidate false positives, and 23 µs mean query time. It misses Japanese substrings inside continuous text. The alternative tokenizer comparison was not adopted. No new tokenizer, embeddings, or vector database were added.

## Development-only connection surface

The new opt-in `p7-integration-dev` feature composes the existing completed-conversation sanitizer, mock extractor, extraction Admission/lease, persistence, FTS retrieval/provenance, synthetic ledger, recovery barrier, and Forget coordinator. Its public DTO accepts synthetic turn IDs/text with PUBLIC or PERSONAL classification only; the owner is fixed to one synthetic fixture. Callers can open or reopen only through the fixture connection, submit completed synthetic conversations, retrieve bounded lexical results, and exercise synthetic Forget. The API does not accept an extractor, provider, model, or caller-selected Owner authority.

The feature is not default-enabled and inherits the existing optimized-build compile guards through `p7-experimental` and `p7-gate-c`. Existing test-only ledger capacity reservation is enabled for this development feature only. The database path is created as a fresh synthetic fixture by `open`; `reopen` requires existing state and runs the existing recovery barrier. Only that fixture-specific path uses migration 0009. Ordinary `Store::open` remains at schema v8 in non-test builds because its embedded catalog is unchanged by `p7-integration-dev`; no ordinary database migration is added.

## Verification on C-08 working tree

- `cargo check -p serea-storage --features p7-integration-dev`: PASS.
- Focused synthetic completion → extraction/save → DB reopen → provenance-bearing retrieval → Forget → reopen/no retrieval → post-Forget save refusal: PASS (1/1).
- `cargo clippy -p serea-storage --features p7-integration-dev --all-targets -- -D warnings`: PASS. The C-07 lint was resolved with a narrow documented allow for the retrieval method's independent bounded arguments; query and ranking behavior is unchanged.
- Default `cargo check -p serea-storage`: PASS. Optimized `cargo check --release -p serea-storage --features p7-integration-dev` was rejected by both existing P7 optimized-build guards, as required.
- The existing Japanese benchmark was not rerun; C-07 evidence is retained as recorded above.

No runtime or code change was made to Android Bridge, `serea-core`, `ChatSessionComposition`, or Android UI.

## Minimum Android integration contract for a later phase

1. Android Bridge (or its host adapter) supplies a typed completed-conversation DTO with opaque conversation/turn IDs, role, classification, origin, and source lineage. Completion must not be inferred from arbitrary model/request text.
2. `serea-core` owns an optional Memory adapter boundary: submit that DTO with a stable extraction request ID after a successful conversation result, preserve the chat result if extraction fails, and expose bounded retrieval/Forget operations under host-supplied identity and authorization. The current C-08 API is a synthetic fixture and cannot serve as that production adapter.
3. `ChatSessionComposition` wires the core adapter only after separate authorization and storage design. Memory results must not be injected into a model request in this phase.
4. Android UI remains outside this connection contract; no preference, toggle, consent screen, or Memory-on affordance should be added until those decisions are ratified.

Before any real integration: replace the fixed synthetic Owner and synthetic key/checkpoint/ledger fixture with separately reviewed host authority ports; define the bridge/core DTO compatibility; keep ordinary storage migrations and Memory activation disabled; preserve best-effort completion semantics; then run targeted Android Bridge/core composition tests. No real AI communication, personal data, OAuth, ordinary DB migration, production Memory, or model-context injection is authorized by C-08.
