# Cloud C-09A — Real AI Readiness

## Base and branch

- Base: PR #273, `3eb1a9eb1561b23e5e78ad542996b950472e22a4` (`cloud/serea-v13-c08b-core-memory-integration`).
- Candidate: `cloud/serea-v13-c09a-real-ai-readiness`, created independently at that exact HEAD.
- C-08B is accepted as the development-only Core/P7 adapter. This branch adds an external-crate feature-boundary test; it does not alter PR #273 or its branch.

## C-08 closure: actual completion and Memory boundary

The synthetic Core adapter accepts only the existing P7 development conversation fixture and only calls extraction after an `Ok` result. A production call site must not synthesize this fixture from arbitrary provider text or use provider response success as the completion authority.

The current Android Chat lifecycle already records a pending user turn, invokes the model, confirms the coroutine is still active, verifies that the same message and conversation still own the request, persists the assistant turn with `completeTurn`, and then marks the user turn `COMPLETED`. Cancellation and timeout persist `OUTCOME_UNKNOWN`; definite model failure becomes `FAILED`. A canceled coroutine must first pass its active/captured-request checks before its reply can enter the completed path. A late reply after cancellation or conversation switch remains unknown and is not eligible for extraction. The C-09A branch does not change this locally owned lifecycle code.

For a later Android connection, expose a narrow post-finalization event carrying stable conversation/message IDs and the already-established role, status, class, origin, and source lineage. Only a persisted `COMPLETED` turn may be submitted. Enqueue the development/production extraction work after completion so its latency or failure cannot delay, replace, or roll back the Chat response. Do not put retrieved Memory into the model context as part of this connection.

The current Core adapter's P7 Owner and authentication are synthetic. Real Owner authorization must come from a reviewed host-authentication boundary; key custody must come from a separately reviewed protected credential/key port; provenance and classification must remain intact from the completed turn through the extractor and stored Memory. No class, origin, or lineage may be rewritten to PUBLIC. A real integration needs a production-only storage handle isolated from the fixture's path and schema; it must not open the development fixture or ordinary Core database as a side effect. ADR-0043 production gates remain in force.

### External crate verification

Added an external integration target under Core tests. It imports the public Core adapter as a dependency while the development feature is explicitly enabled. It exercises failed/cancelled result exclusion, stable extraction request identity, mock extraction/save, Chat-result preservation on extraction validation failure, database reopen, PERSONAL provenance retrieval, Forget, and post-Forget save refusal. Its input and Owner stay synthetic; the test is not a production adapter contract.

## C-09A readiness review

The existing Android Cloud route already has a fixed Ollama destination, default-off settings, trusted/fresh model catalog validation, an exact model/provider registry, class/origin and payload matching at Egress, output/input bounds, durable local Usage reservation before dispatch, typed usage/price attribution, hardware-backed Android Keystore credential storage, no automatic retry or redirect, and typed cancellation/timeout/network/auth/rate-limit outcomes. Existing focused synthetic registry, Ollama MockWebServer, Usage, routing, context, and Egress attack-matrix tests cover these boundaries. The latest PR #273 Android workflow is also green. This review found no new provider-code regression or narrow implementation defect that can be corrected without selecting policy for the Owner; no existing Cloud transport or Egress rules were widened.

The following are readiness blockers, rather than defects to paper over in this branch:

1. **Provider and model:** only the Ollama destination and selected catalog models are wired. O-02 has not selected an eligible provider/model roster or authorized an account. Any additional provider would require a provider implementation and an exact, separately approved Egress destination.
2. **Credentials:** the existing credential boundary reads the key only inside the adapter and stores it encrypted with a hardware-backed Android Keystore key. Owner approval is still required for the actual account/key operation, device eligibility, key recovery/replacement/revocation behavior and provider account terms. No key was supplied or read.
3. **Classification and Egress:** current cloud admission is deliberately limited to PUBLIC, USER_AUTHORED material and the approved Ollama endpoint. `ChatContextBuilder` retains source lineage; Google-origin and other inherited data do not become eligible from a user click or by changing metadata. Any additional class, preparation/redaction rule, context source or endpoint requires an explicit policy decision and tests.
4. **Usage and billing:** the existing local daily/monthly request and token limits are soft client-side controls. Price evidence is optional and may be unknown; provider billing and account-side limits remain authoritative. Owner must select explicit request/token/spend ceilings, price source and account-level limit strategy before live usage.
5. **Failure and cancellation:** requests do not retry automatically. Cancellation or timeout after dispatch may leave a remote outcome unknown; the user must decide the explicit retry UX and acceptable duplicate/billing risk. Provider-side cancellation is not guaranteed.
6. **Activation and disclosures:** Cloud remains disabled by default. Owner must approve age/region eligibility, current provider terms, retention/training policy, privacy disclosures, account/model selection and an explicit activation point. ADR-0044 remains PROPOSED / NOT RATIFIED.

No real provider was added because doing so would encode unresolved provider, endpoint, account, data and budget choices. Existing Ollama MockWebServer and synthetic registry tests already exercise the current provider seam; duplicating those tests without a behavior change would not close an identified gap. Real API keys, live requests, billing, OAuth, and production Memory remain disabled.

## Owner decisions required before real AI activation

The Owner should resolve O-02 as one explicit decision packet:

- Confirm user age/eligibility, region, provider terms, data retention/training settings, provider and exact model roster.
- Select the account and credential flow, approve device/Keystore eligibility, and set replacement, revocation, recovery and lost-device behavior.
- Approve endpoint(s), exactly eligible message/context classes and source origins, preparation/redaction requirements, and whether any Google-origin or Memory-derived data can ever be sent. Current Cloud path remains PUBLIC/USER_AUTHORED only.
- Set per-request output limits and daily/monthly requests, tokens and monetary ceilings; identify authoritative price evidence, provider account limits and who bears charges.
- Confirm no automatic retries; define user-visible behavior for cancellation, timeout, unknown remote outcome, and explicit resubmission.
- Approve local history/usage retention and deletion, provider retention/training disclosures, crash/logging exclusions, activation UX, and rollback/deactivation expectations.
- Explicitly authorize live requests and billing only after those decisions are recorded and the selected destination, credential store, class policy, budgets and target-device security are verified. This report does not request or assume any of those approvals.

## Verification and CI

- The focused Core external integration target with `p7-memory-integration-dev`: **PASS (1/1)**.
- `cargo check -p serea-core`: **PASS**.
- `cargo check -p serea-core --features p7-memory-integration-dev`: **PASS**.
- `cargo check --release -p serea-core --features p7-memory-integration-dev`: **rejected as required** by the existing P7 optimized-build compile guards.
- The first local Cargo invocation used the environment's non-writable default rustup home and failed before compilation. Re-running with the existing workspace toolchain/cache paths succeeded; this was environment configuration, not a product failure.
- PR #273 HEAD `3eb1a9e`: Fast CI `46001462208`, Full CI `46001462085`, Rust Dependency Security `46001462113`, and Git hooks `46001462140` passed. Android `46001462275` completed successfully as a cloud-scope run: Android product, Formal API 26/35, Unit, Integration and Identity jobs were skipped; the successful workflow gate is not Android Required or Formal evidence. Native `46001462091` is still in progress and SQLite portability `46001462138` is queued. PR #273 remains open, Draft and unmerged.

## Scope and safety

Only the independent Core test target and this C-09A report are changed. No Android Bridge, Core initialization/ownership, Chat session composition, Android UI, Provider, Egress, Usage, Context, Gradle, signing, campaign runner, or existing PR branch is modified. ADR-0043 remains development-only; ADR-0044 remains PROPOSED / NOT RATIFIED.
