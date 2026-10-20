# AND-M0-05A — Provider and Model Routing Integration Report

Date: 2026-10-20  
Status: implementation candidate; real Cloud disabled; real provider requests NOT_RUN; emulator and final CI NOT_RUN.

## Baseline and scope

- PR #50 is open with head `94492d281fe0ee583744c932259c3525d5c34f86` on `codex/android-m0-04-integration-candidate-2026-10-20`. This branch starts at that exact commit and is intended to be a delta PR against that branch.
- Android remains standalone. The `MainActivity` composition continues to use `MockModelChatPort`; this change does not connect ordinary UI input to Cloud.
- No Rust Core, task, Calendar, Gmail, PC pairing, GoalLatch, Room schema, encrypted history, or Usage repository/DAO/entity changes are included.
- ADR-0044 and ADR-0045 remain `PROPOSED / NOT RATIFIED`.

## Findings and architecture changes

- The Router already selected deterministically and propagated a model ID, but the Ollama Adapter still used a one-ID transport allowlist. The direct composition did not include the Router, and wrappers could be arranged so Usage reservation preceded routing or egress denial.
- The integrated candidate now composes `RoutedModelChatPort -> EgressCheckedModelChatPort -> UsageControlledModelChatPort -> OllamaModelChatPort -> OllamaCloudAdapter`. The Router writes the selected ID to the existing `ModelChatRequest.selectedModelId`; downstream stages do not select a replacement. Manual selection failure has no fallback.
- Egress remains independent from selection. The Egress contract must match conversation, send, text, context, origin, classification, fixed provider destination, explicit send intent, and expiry. After routing, the gate binds its approved payload to the selected model ID. The Adapter uses the gate-produced payload and checks that its binding still matches the request.
- Egress denial occurs before Usage reservation and credential reads. The Adapter rechecks Catalog eligibility immediately before request creation, so removal or revocation after routing denies the send.
- Provider destination remains fixed to `https://ollama.com/api/chat`. Model IDs are validated as bounded identifiers and never form a URL. The injectable test endpoint is internal and accepts only fixed Ollama or loopback MockWebServer URLs.
- Existing timeout/cancellation behavior makes no implicit retry. Timeouts report an unknown remote outcome. Streaming remains bounded NDJSON, emits deltas in order, rejects malformed/truncated/incomplete responses, and returns a completed response only after `done: true` with nonempty content.

## Model Catalog contract

- Production defaults to an empty `LocalModelCatalog`. No live catalog synchronization is implemented.
- Existing `ModelDescriptor` now includes explicit `verificationStatus` (default `UNVERIFIED`) and optional `expiresAt`. `LocalModelCatalog` applies a seven-day default freshness window; a source expiry can shorten it. Invalid, future, expired, revoked, or stale evidence is ineligible.
- Routing and the Adapter require a Catalog record, fresh verification, `AVAILABLE`, positive authorization, required capabilities and input format, context and output limits, source evidence, and an explicitly allowed Provider ID. Unknown remains a denial for required eligibility. Authorization false is treated as unconfirmed/denied.
- Test Catalog rows use a separate `TEST_FIXTURE` verification state and `FakeModelCatalog`. Their capacity, capabilities, prices, and authorization do not describe real Provider offers. The Ollama Adapter accepts that state only when its endpoint is loopback; a test fixture can never authorize the fixed real-provider endpoint.
- Balanced uses deterministic ordering when prices cannot be compared across currencies. Cost Priority excludes unknown prices and enforces the configured currency and estimate ceiling. Quality Priority has no measured benchmark and reports that ID ordering is only a deterministic tie-break, not a quality ranking.

## Routing and adapter behavior

- Auto and Manual continue to use the existing request/policy contract. Manual requires the one named model. Auto filters by verified Catalog, Provider allowlist, capability, entitlement, availability, context, output, and applicable cost constraints.
- The normal Ollama request path no longer has an implicit fixed model. It requires the Router-selected Catalog model ID and validates it against the `ollama` Provider allowlist. The historical `gemma4:31b` identifier remains only in the separate fixed public contract probe path; it is not a Catalog entry, ordinary-chat default, or model recommendation.
- Adapter errors distinguish authentication, forbidden, model unavailable, rate limit, server, timeout, network, invalid response, and local size limit. Redirect following is disabled; TLS uses platform validation; one send creates one HTTP call.
- `ProviderUsage(inputTokens, outputTokens)` remains unchanged. The existing `sendId` remains the reservation/history key and is not regenerated by routing or transport.

## Validation added and results

- Router JVM cases were extended for stale/unverified catalog rejection and deterministic policy behavior with fictional multi-currency fixtures. Existing cases cover empty Catalog, Manual no-fallback, provider allowlist, capabilities, context/output limits, prices, and URL-shaped IDs.
- One local HTTPS MockWebServer instrumentation case exercises the full direct composition with two selected model IDs, checks each HTTP body and same-conversation context, then proves a PRIVATE egress denial adds neither a request nor a usage reservation. The API 26/35 verifier was raised from 34 to 35 total cases and `ChatHttpE2ETest` from 2 to 3 based on this added test; existing class minima remain unchanged.
- Existing provider MockWebServer unit coverage exercises streaming fragmentation and UTF-8, status errors (401/403/404/429/5xx), redirects, timeout, cancellation, disconnect, TLS rejection, malformed/incomplete/oversized/empty output, no automatic retry, and no credential read on disabled/denied paths.
- Executed locally: `git diff --check` passed.
- NOT_RUN: Gradle unit tests, Android integration/security suites, API 26 and API 35 instrumentation, Process Death, Android Emulator Required Gate, and repository workflows. Gradle Wrapper download succeeded after explicitly selecting the installed JDK 21 and proxy; Gradle then stopped because this environment has no Android SDK (`/workspace/android-sdk` is absent). No app test result is claimed here.
- No real provider endpoint, real API key, account, or billing operation was used.

## Usage Ledger integration contract for AND-M0-05B

- The existing `ModelChatRequest.selectedModelId` is the final Router choice, and `sendId` remains identical across routing, reservation, provider dispatch, and history. `ProviderUsage` input/output token fields are unchanged.
- This branch does not change `UsageAccounting.kt`. Current global usage reservations retain their existing `sendId` API and occur only after Egress approval.
- To persist per-model usage, B will need a backward-compatible reservation/settlement extension that accepts the already selected model ID and associates provider-reported input/output tokens with that same ID and `sendId`. B must not re-run model selection or create another send/reservation ID. Reconcile the wrapper boundary before combining changes to avoid both branches editing `UsageAccounting.kt` concurrently.

## C boundary and likely merge conflicts

- No C-owned task domain or task UI work is included. `TOOL_REQUIRED` classification stays descriptive and has no Tool execution path.
- Likely shared-file conflicts are `AdaptiveModelRouter.kt`, `CloudProvider.kt`, `SecurityPorts.kt`, Android tests, and `.github/workflows/android.yml`. This change intentionally leaves `UsageAccounting.kt`, Room and history files untouched. Integrate C's task work separately and retain the no-tool-execution boundary.

## Owner decisions and year-end Beta work

- Keep Cloud disabled until the Owner separately approves provider terms/eligibility, exact Catalog sources and refresh/freshness policy, account entitlement evidence, data classification and consent, credential policy, retention/region facts, spending controls, distribution channel, and release disclosures.
- Do not promote the fictional test Catalog fixtures into production. Do not change ADR-0044/0045 status as part of code integration.
- Before v0.1.0 Beta: complete all final-head required workflows and API 26/35 emulators; run the physical-device checklist; define production Catalog ingestion and revocation; finish the B model ledger integration; review Android credential storage and account setup; verify provider terms/privacy/retention and owner-approved egress; and make a separate activation decision.
- Final CI Run IDs/results and final commit SHA must be appended after the Draft PR branch is pushed and Actions finish.
