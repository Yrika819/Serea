# AND-M0-02 — Adaptive Model Routing and Android Beta readiness

Date: 2026-10-19. Status: implementation candidate; Cloud disabled; physical
Pixel acceptance NOT_RUN.

## Architecture and current state

- Android owns an independent Kotlin Catalog and deterministic Router. No Rust
  host Router, PC permit, or Serea backend participates.
- `ModelDescriptor`, `ModelCapabilities`, `ModelPricing`, `ModelAvailability`,
  `ModelSelectionMode`, `ModelRoutingPolicy`, `ModelSelectionResult`, and
  `AdaptiveModelRouter` represent the local contract. The production local
  Catalog is intentionally empty. It contains no guessed model, price,
  performance, availability, account entitlement, context limit, or API
  capability.
- Unknown values remain `UNKNOWN`. Availability and user authorization must be
  positively confirmed. Capability requirements, context, output ceiling, and
  explicit estimated budget are hard filters. Cost Priority excludes unknown
  prices; Balanced leaves price-unknown eligible when no budget constraint is
  requested and never calls it free. Quality Priority uses stable Model ID
  ordering because there is no measured quality evidence; it exposes this
  limitation instead of inventing quality scores.
- The device-only rules classify SIMPLE, STANDARD, COMPLEX, STRUCTURED,
  LONG_CONTEXT, and TOOL_REQUIRED. Complex tasks require confirmed reasoning
  capability. This classification sends no prompt to a separate model.
- Manual selection is exact. Unknown, deleted, unauthorized, or unavailable
  IDs fail; the router does not fall back. The fixed Ollama candidate retained
  inside the legacy transport is not exposed as the Catalog default or a
  recommendation and is not activated by these settings.
- Model selection preferences use a separate Android SharedPreferences file,
  synchronously committed before reporting save completion. They do not share
  provider credential storage. No Room schema changed; schema remains v3 and no
  migration or destructive migration is introduced.
- The optional per-request estimate cap and its three-letter currency are
  persisted with the routing policy. The Router calculates an estimate only
  from verified input/output prices and matching currency. Existing global UTC
  soft-limit reservations remain separate; per-model actual usage/cost ledger
  rows are not added in this step, and local estimates do not cap Provider
  billing.
- Routing is not an Egress decision. Existing typed egress contracts and
  `EgressCheckedModelChatPort` continue to govern provider dispatch. Classification
  and history metadata are not rewritten by the selection. UNKNOWN, PRIVATE,
  SECRET, CREDENTIAL and unapproved PERSONAL requests remain denied by that gate.
- Cloud remains disabled. No provider account, credential, live request,
  subscription, or billing operation was used. Official documentation checked
  on 2026-10-19: [Ollama Cloud](https://docs.ollama.com/cloud) says Cloud models
  can be listed at `https://ollama.com/api/tags`, and API requests use a key.
  The separate [List models](https://docs.ollama.com/api/tags) reference
  documents `GET /api/tags` against `http://localhost:11434` for local Ollama;
  the Cloud page is the evidence for the Cloud URL. No catalog request or
  credential was used. Production sync waits for a separately approved source
  and refresh policy. The UI reports an empty catalog instead of offering the
  legacy candidate.

## Validation added

- JVM Router cases: empty catalog, task categories, SIMPLE/COMPLEX selection,
  capability and context exclusions, availability and authorization, explicit
  estimated-cost exclusion, deterministic selection, unknown-price handling,
  URL-shaped ID rejection, source-verification requirements, and no Manual
  fallback. Input token budgeting uses the existing conservative four tokens per
  UTF-16 character estimate; it is not tokenizer output.
- Multi-turn Mock HTTP instrumentation sends two distinct manually chosen
  model IDs in consecutive request bodies while preserving the same conversation
  history and usage accounting. A separate case proves unavailable Manual
  selection creates zero additional requests.
- The existing OS force-stop harness stages Auto/Manual settings, policy, a
  removed model ID, and an unresolved usage reservation, then verifies them
  after actual app process termination/restart. It also checks the removed
  model cannot be reselected from the empty catalog.
- API 26/35 emulator verifier expectations are updated from 21 to 23 tests,
  with existing EncryptedHistory (16) and ChatScreen (4) retained and the
  Mock HTTP class count raised from 1 to 2. The persisted Model Selection
  Compose case raises ChatScreen to 5, for 23 total. `Android Emulator Required Gate`
  stays the same required job.
- Unit suite is run by the Android Unit CI job. Full CI and platform checks
  must be collected on this final head before calling it complete.

Expected targeted JUnit totals on final head: Android Unit 16 (9 existing
ViewModel plus 7 Router); Android Integration 19 (Mock Provider adapter); Android
Security 9; API 26 23; API 35 23. Coverage is not measured by these workflows.

## Device handoff and decisions

Use [the Pixel 7a checklist](2026-ANDROID-PHASE2-PIXEL7A-TEST-CHECKLIST.md).
Physical results are NOT_RUN. The Draft PR Actions artifact supplies the
installable debug-signed APK and the exact head SHA/checksum. Rooted-device
Keystore observations do not establish whole-OS trust. No real API key or
Provider request belongs in device acceptance.

| Milestone | Readiness |
| --- | --- |
| 2026-11-08 live AI connection | Code and Mock path are prepared; live communication is still blocked. On the date, recheck current terms, age eligibility, official API/catalog/pricing, entitlement, spend controls, retention/processing, data classification and egress approval. Require separate Owner activation. No timer or age-based switch enables Cloud. |
| 2026-11-22 M0 history/recovery | Existing encrypted history, Room v3 migrations, UTC soft limits, and API 26/35 process-death harness are retained. This branch adds model settings/usage restoration evidence; new read-in-flight and conversation-switch process-death cases remain separate validation targets. |
| 2026-12-31 v0.1.0 Beta | Feasible only after final CI, Pixel 7a acceptance, explicit model source and pricing policy, data/retention review, credential/distribution decisions, and Owner approval. Live service is not implied by the date. |

Recommended next work: collect final-head CI and device evidence, then review
the Catalog source interface and provider terms/entitlements with the Owner
before implementing any production Catalog refresh or network activation.
