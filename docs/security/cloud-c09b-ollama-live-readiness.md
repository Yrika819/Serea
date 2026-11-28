# C-09B — Ollama Cloud live readiness

Date: 2026-11-27
Branch: `cloud/serea-v13-c09b-ollama-live-readiness`
Base: C-09A PR #274 HEAD `64e65673f9f95948628b71d457e42795079497f2`

## Owner decision and official-source review

Ollama Cloud / Gemma 4 is the first selected provider for the Owner's personal Serea. The application API uses model ID `gemma4` at `https://ollama.com/api/chat`; the CLI example uses tag `gemma4:cloud`. These names are not interchangeable.

The official model page lists a 256K context window. The pricing page, reviewed 2026-11-27, lists USD 0.14 per million uncached input tokens, USD 0.05 per million cached input tokens and USD 0.40 per million output tokens. Its PAYG description says cloud models are charged per token. The application therefore records the published input/output prices for Usage estimates, but does not claim that its local caps ensure zero account spend. The current provider response parser reports aggregate prompt/evaluation token counts, not cached-token counts, so the ledger applies the uncached input rate to all provider-reported input tokens. This can overestimate cached-input cost; it is not a billing guarantee.

The API introduction and Cloud documentation describe account/API-key based application requests. The Terms require eligibility and authorized access, and restrict unauthorized automated access. They do not state a special blanket permission for embedding in a personal Android app. The implementation uses only the documented endpoint and bearer-key API; the Owner must accept the current Terms. This is an implementation record, not legal advice. The privacy policy says prompt/response data is processed to fulfill requests and is not used for model training; account, usage and device/network metadata may be processed. No real account or key was inspected here.

Official sources reviewed:

- <https://ollama.com/library/gemma4:cloud>
- <https://ollama.com/pricing>
- <https://ollama.com/terms>
- <https://ollama.com/privacy>
- <https://docs.ollama.com/cloud>
- <https://docs.ollama.com/api/introduction>

## Implementation

- Added a dated, expiring Gemma 4 catalog and USD price snapshot. The selected API model ID, HTTPS destination and request output ceiling are fixed. The catalog's 1024 output limit is Serea's Owner-approved cap, not a provider-published model maximum.
- Added a one-shot Cloud settings probe composed through the existing adaptive router, Egress gate, durable Usage reservation/settlement and Ollama provider registry. It sends only a fixed synthetic PUBLIC / USER_AUTHORED message, with no chat input or history. Ordinary chat remains on Demo and Cloud defaults remain disabled.
- The settings screen requires an ephemeral Owner confirmation that the account shows free Gemma 4 availability and no paid subscription, purchased credit or automatic charge. It also checks configured credentials and hardware-backed Android Keystore. The key remains entered and stored on the device; the probe sends no key in its body and does not print provider response details or credentials.
- Enforced the selected request ceilings in Usage accounting: 20 requests/day, 200/month, 4096 estimated input tokens/request, 1024 output tokens/request and one unresolved dispatch at a time. Existing lower user-configured caps remain in force; higher stored caps are bounded by these limits. These are local application limits only and cannot control account billing or enforce provider-side quotas.
- Input token count is conservatively estimated as four tokens per UTF-16 character across sent message content. No Gemma tokenizer is present, so 4096 is an estimate, not an exact tokenizer guarantee. The provider transport independently rejects requests above the estimate before credential access/network dispatch.
- HTTP 401, 402, 403/404, 429, network and timeout outcomes are distinguished without automatic retry/fallback. A post-dispatch cancellation or timeout remains `OUTCOME_UNKNOWN`; cancellation is propagated and the reservation is retained for review.
- Kept Android Chat composition and the ordinary Demo path unchanged. No live request, real API key, account access, charge, Memory enablement, OAuth, personal data, migration, or ADR-0044 ratification occurred.

## Verification

Added a MockWebServer integration test that exercises the synthetic probe through routing, Egress, Usage, and Ollama transport, including model ID and secret non-exposure. Added Usage cap/concurrency coverage. Existing adapter tests cover response parsing, failure mapping, no redirects/retries, cancellation and dispatch evidence.

The focused Gradle tests could not run in this environment. The wrapper attempted to download Gradle 8.14.6, but the configured external connection refused the request before Gradle started. This is an environment dependency-resolution failure; no product test result was produced. GitHub CI at the resulting C-09B HEAD is the next executable verification. No claim of test pass is made here.

## Owner actions before a real request

1. On the Ollama account, privately confirm that `gemma4` is available under a free allowance with remaining quota and that no paid plan, purchased credit or automatic charge will be used. If this is not clear, do not use the probe. Serea cannot verify the billing source.
2. On the exact target Android device, confirm hardware-backed Keystore support. Enter/register the API key directly in Cloud Settings; never paste it into chat, GitHub, logs or diagnostics.
3. Review the fixed endpoint, model, synthetic prompt, current Terms and Privacy policy. Select the in-screen free-use confirmation and run one synthetic probe only when ready. Do not put a real conversation into this route.
4. If the provider reports unavailable/free quota exhausted, stop. There is no paid fallback, model switch, redirect or automatic retry. Inspect Usage for `OUTCOME_UNKNOWN` after timeout/cancellation before considering a separate explicit retry.

ADR-0043's development-only boundary remains in force. ADR-0044 remains `PROPOSED / NOT RATIFIED`; this O-02 decision selects only the initial Owner-personal provider lane.

## C-09BR — Android Formal recovery and first-connection review

Date: 2026-11-27. Target reviewed: PR #275 HEAD `20be4f8e506a5bca2c39ef27af29668ec86286ef`, Tree `d8113c755616e5ce477323accf82783c62af4380`.

### Formal failure and test-only correction

Android workflow run `46011161590`, attempt 3, compiled and built successfully. API 26 and API 35 both launched the Pixel emulator and ran instrumentation. Each reported the same three `SereaAppShellTest` failures (19 tests, 3 failures, 0 errors, 0 skips):

- `fontScaleTwoKeepsTaskProposalReviewAndFooterReachable`
- `activityRecreationRestoresRouteAndBackStackButNoProposalSession`
- `chatSettingsBackKeepsCredentialScreenshotProtectionScoped`

All three looked for the removed text `Provider候補: Ollama Cloud`. The current settings route exposes its heading as `destination_title`. C-09BR changes only those test assertions to use that stable tag and the current Settings heading. The screenshot `FLAG_SECURE` checks, route/back-stack assertions, accessibility/reachability assertions, required-test identities, 103-test Original Required Set, three additional-test identities, and provenance checks remain intact. No product defect was identified from these failures. The Android Emulator Required Gate correctly remained failed because the formal evidence did not pass; API 26/35 and aggregate success were not inferred from the build or partial artifacts.

### Limited first-connection control review

This is a source-and-existing-test review only. No credentials were registered, no real request was sent, no account entitlement was checked, and no device was operated.

| Control | Current code and existing evidence | Finding |
|---|---|---|
| Free-use confirmation | Settings keeps `freeAllowanceConfirmed` ephemeral, resets it when leaving Settings and after a probe; the send button and handler both require it. | No clear regression found. Account eligibility and remaining quota still require the Owner's own confirmation. |
| Hardware-backed Keystore | Settings requires a configured credential and `isHardwareBacked()` before enabling and dispatching. `CredentialRepository` rejects unsupported storage. `EncryptedHistoryTest` covers unsupported hardware and secret-at-rest behavior. | Fail closed in the app path; not exercised on a physical target device. |
| Multiple dispatches | The button enters `cloudProbeRunning` before network work and disables during the operation. Usage admission has a one-in-flight limit; `ollamaUsageCapsClampHigherSavedSettingsAndLimitInFlightDispatchesToOne` checks a second reservation is refused. | No clear duplicate-dispatch regression found in the reviewed path. |
| Data sent | `runFixedPublicContractProbe()` constructs one hard-coded synthetic PUBLIC / USER_AUTHORED message with empty context; normal Chat is not passed into it. `syntheticOllamaProbeRunsThroughEgressUsageRouterAndMockTransport` checks model/prompt, reservation, reported usage and that its fake key is absent from the body. | The reviewed UI route is limited to the synthetic probe. No real conversation is connected to this route. |
| Key exposure | Credential handling uses the Keystore reader and an Authorization header; provider failures contain generic status text, and the provider path has no key logging. Mock tests use only fake key strings. | No key-log path found in the reviewed Cloud implementation; this is not a guarantee about unrelated future logging changes. |
| Timeout, cancellation, unknown outcome | Dispatch observation is tracked, calls are cancelled with the coroutine, redirects/connection retries are disabled, and Usage preserves `OUTCOME_UNKNOWN` after ambiguous post-dispatch failures. `cancellationBeforeDispatchReleasesButAfterDispatchKeepsOutcomeUnknown` and `providerFailureAfterDispatchKeepsUsageOutcomeUnknownWithoutRetry` cover the ledger behavior. | Existing tests preserve the required no-auto-retry behavior. |
| Demo isolation | Ordinary Chat remains Demo/Fake, Cloud defaults disabled, and the Cloud candidate is opened only from Settings. `demoLabelAndConversationTitleAreVisible` and the Mock contract tests cover the ordinary path and synthetic candidate separately. | No unintended Demo-path connection found. |
| Model/price freshness | The Gemma 4 model entry and price evidence both expire on 2026-12-04. The router rejects an expired catalog entry before dispatch; the attribution resolver rejects expired model evidence and treats expired price evidence as unknown. `staleMismatchedOrNotYetEffectivePriceEvidenceStaysUnknown` covers the generic stale-price rule. | No stale-evidence bypass found. The current dated catalog must be reviewed again after expiry. |

This review found no clear product regression requiring a provider or security-code change. The C-09BR fix remains limited to the stale UI assertions and this record.

### Cloud/local integration note

The C-09B Cloud changes relative to Base are in:

- `android/app/src/main/java/app/serea/android/MainActivity.kt` — Settings-only Cloud candidate UI, free-use confirmation, credential and screenshot-protection handling, limits and fixed-probe action.
- `android/app/src/main/java/app/serea/android/CloudProvider.kt` — provider transport, Egress/Usage route, fixed synthetic probe, request limits and failure handling.
- `android/app/src/main/java/app/serea/android/AdaptiveModelRouter.kt` — dated Gemma 4 catalog, source and freshness evidence.
- `android/app/src/main/java/app/serea/android/CloudProviders.kt` and `UsageAccounting.kt` — local caps and durable reservations/settlement.
- `android/app/src/test/java/app/serea/android/CloudProviderRegistryTest.kt` and `OllamaCloudAdapterTest.kt` — synthetic route, provider and failure coverage.
- `docs/security/cloud-c09b-ollama-live-readiness.md` — this implementation record.

Local work also edits `MainActivity.kt`. During any future integration, review and combine individual Cloud Settings changes with the local Chat/Core/ownership changes; do not replace the whole file from this branch. C-09BR does not edit `MainActivity.kt`, Chat/Core composition, Gradle, signing or campaign scripts. C-09BR changes only Android instrumentation test expectations in `android/app/src/androidTest/java/app/serea/android/SereaAppShellTest.kt` and `android/app/src/androidTest/java/app/serea/android/ChatScreenTest.kt`.

The corrected Formal suite, Android Required Gate, and APK provenance must be checked on the resulting exact C-09BR HEAD before providing an APK candidate. An emulator APK is not evidence of physical-device compatibility or package/signature coexistence. Any Owner-installed candidate remains subject to exact artifact digest, signing-certificate and package-ID review; do not overwrite an existing app or profile with personal data.

### Formal follow-up on `f2fa66f30491a7d09e475fbd76d250aff29ecc81`

After the first correction, API 26 and API 35 both passed all 19 `SereaAppShellTest` cases, including the three recovered tests. Each then exposed the same three stale assumptions in the 16-test `ChatScreenTest` group:

- `correctedSoftLimitInputCanBeSavedAndRestored` entered 21, but the approved daily-request cap is 20. It now saves and restores 19, a valid value below the cap.
- `softLimitRejectsZeroNegativeNonNumericOverflowAndDailyMonthlyMismatch` used 101 while the current monthly default is 200, so that value is valid. The mismatch case now uses 201.
- `previewShowsUnclassifiedUserTextWhileCloudSendRemainsDisabled` looked for a removed “Cloud connection test” button. It now checks the current “合成probeを1回送信” action remains disabled without credential/free-use confirmation.

The three test identities and all unrelated validation assertions remain. These are test-expectation corrections to the current Cloud limits and Settings UI; no provider behavior changed. The same run's Personal Beta identity job failed before tests because Maven Central returned HTTP 429 while downloading `failureaccess-1.0.3.jar` and `error_prone_annotations-2.50.0.jar`; the failed build means that identity result is not a product test result and requires a failed-job retry on the corrected HEAD.

### Formal follow-up on `d91d7712c3444351e6c13e39c96c31d142316caa`

On API 35, all 19 `SereaAppShellTest` cases passed and the `ChatScreenTest` group reported 16 tests with one failure in `softLimitRejectsZeroNegativeNonNumericOverflowAndDailyMonthlyMismatch`. The test set daily tokens to 50,001 while the existing monthly-token default is 1,024,000, so the expected mismatch validation did not apply. It now sets 1,024,001, an actually invalid daily-over-monthly value. This preserves the mismatch assertion and changes no product behavior. The API 26 job was still running when this correction was prepared; its eventual result remains to be checked on its own evidence.

### Cloud / Embedded Core integration overlay — 2026-11-28

The integration candidate supersedes the earlier Settings action's availability: external provider dispatch is now default-deny in the composition, registry, and adapter layers, and the Cloud Settings action is disabled until a separate explicit Owner authorization. No key is read and no request reaches Usage or a socket through the product Cloud composition while this gate is false. This is not a real-provider readiness claim or an account/billing check.

The loopback-only integration test composes Embedded Core readiness with the existing Cloud path, preserving the official API model ID `gemma4`, the 1024 output cap, Egress-before-Usage ordering, credential access only after reservation, and settled Usage attribution. Core initialization failure and Egress denial stop before Usage, credentials, and HTTP. The ordinary Chat composition remains Demo-first; the Core adapter remains isolated to the diagnostic variant. No real key, account, provider endpoint, personal data, or charge was used. The disabled Settings label assertion was updated to `Cloud probe（Owner承認待ち）`; its Android instrumentation runtime was not repeated as part of this local loopback integration.
