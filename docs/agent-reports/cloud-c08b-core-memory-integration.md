# C-08B Core Memory Integration

## Branch and accepted inputs

- Base: `cloud/serea-v13-c08-integration-readiness` at `c102ed64ce72ca30dbc188829ab35cf28d6105a6`.
- Candidate: `cloud/serea-v13-c08b-core-memory-integration`.
- C-08A's synthetic `P7DevConnection` is reused without changing the production Memory boundary.
- PR #270's API 26 and API 35 Formal evidence remains accepted at 106/106 tests each, with the same-run build/artifact aggregate passing. Its Android Required Gate remains incomplete because the Unit and Integration jobs previously failed on Maven Central HTTP 429 before test execution.

## PR #272 CI review and repair

At review time, PR #272 was open as Draft on `c102ed64ce72ca30dbc188829ab35cf28d6105a6`.

- Fast CI, Rust Dependency Security, Git hooks, and Android Personal Beta Contract passed.
- Full CI and Native platform validation failed on Linux, macOS, and Windows all-feature test jobs; the Windows P7 job failed on the same cases. Ten P7 crash/recovery tests all launched the crash-child harness using a stale test filter (`experimental_memory::p7_integrated_recovery_tests::...`). The registered test module is `experimental_memory::p7_integrated_recovery::...`, so child processes ran zero tests and returned success instead of reaching the requested crash checkpoint. This was a CI test-harness regression in C-08A; the recovery assertions themselves did not run in the child.
- The subprocess filter has been corrected in `p7_integrated_recovery_tests.rs`. All ten previously failing cases passed locally with `--all-features`; CI will re-run on the C-08B HEAD.
- The SQLite workflow was queued. Android Build/Lint/Instrumentation Compile, Android Integration, Security, Unit, Documentation, Dependency Audit, and Host Probe had passed; API 26, API 35, and temporary-key Identity tests were still running. Therefore this run does not establish Android Formal completion or Required Gate completion.

The first C-08B Fast CI run failed only in documentation validation: a literal fully qualified Rust module path in this report matched the validator's ULID detector. The wording is now descriptive rather than a code path, and the same documentation validator passes locally. A new CI run will evaluate the corrected report.

## Core adapter

Added an independent Core development-memory module behind the explicit `p7-memory-integration-dev` feature. It accepts the existing synthetic `P7DevCompletedConversation` type and calls the existing `P7DevConnection`; it does not define a second shared Conversation or Chat DTO. Core currently has no common Chat/Conversation DTO. Its existing TaskEngine, host supplied Clock, task authority, and initialization contracts remain untouched.

The adapter hashes the length-delimited synthetic conversation ID, turn IDs, roles, classes, and text into a stable request ID. `on_chat_result` invokes extraction only for `Ok` Chat results and returns the successful result unchanged alongside the P7 extraction result, including extraction errors. Failed and cancelled results pass through unchanged without extraction; an unfinished result has no `Ok` completion to submit. The input remains the existing synthetic fixture, never a real Core/Android conversation. PUBLIC/PERSONAL class, user/model role origin, and generated source lineage continue through the P7 path without coercion; the integration test verifies PERSONAL retrieval/provenance. P7's fixed synthetic Owner and authentication remain fixture-only.

The focused Core test runs successful completion → stable-ID extraction/save → idempotent replay → extraction validation failure while preserving Chat result → adapter reopen → provenance-bearing retrieval → Forget → reopen/no retrieval → post-Forget re-save refusal. Ordinary Core open, TaskEngine ownership, storage initialization, and WAL behavior are not modified.

## Verification

- Focused Core/P7 integration test with `p7-memory-integration-dev`: PASS, including failed/cancelled/in-progress result pass-through, extraction-error Chat preservation, and PERSONAL provenance.
- The ten previously failing P7 crash/recovery subprocess tests with all features: PASS (10/10).
- Standard `cargo check -p serea-core`: PASS.
- Feature `cargo check -p serea-core --features p7-memory-integration-dev`: PASS.
- `cargo clippy -p serea-core --features p7-memory-integration-dev --all-targets -- -D warnings`: PASS.
- `cargo fmt --all -- --check` and `git diff --check`: PASS.
- Optimized build with `p7-memory-integration-dev`: rejected by the existing storage P7 development-only compile guards, as required.

The only warning observed is the existing deprecated `usize::max_value` call in vendored serde_json. No Android Bridge, Android UI, ChatSessionComposition, Gradle, signing, AVD, campaign runner, main, or existing PR branch changes were made.

ADR-0043's development-only boundary is retained. ADR-0044 remains PROPOSED / NOT RATIFIED. No real AI communication, personal data, OAuth, production Memory, ordinary migration, or model Context injection is enabled.
