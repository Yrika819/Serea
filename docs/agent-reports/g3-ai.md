# G3 — Real AI / On-device Inference Feasibility

Retrieved official provider/runtime pages: **2026-10-27 UTC**. This is a readiness review, not a legal opinion or a provider activation. No live account, API key, paid request, Google account, or model weight was used.

## Agent report contract

**AGENT_ID:** G3 — Real AI / On-device Inference Feasibility<br>
**ACTIVE_PHASE:** G3 research / feasibility<br>
**OVERALL_STATUS:** PARTIAL — official-source review and integration assessment completed; a valid inference PoC is blocked by missing native build tooling and an intentionally undownloaded 639 MB model artifact.<br>
**BASE_SHA:** `3924b36080ffa43331f065a15db80c604c243355`<br>
**CURRENT_HEAD:** `3924b36080ffa43331f065a15db80c604c243355` (source revision inspected; report-only commit follows)<br>
**TREE_SHA:** `41cfce316c485abe31f86c827c6ca81a4d5c107b` (source tree inspected)<br>
**WORKTREE:** `/workspace/Serea-g3-ai`<br>
**BRANCH:** `codex/serea-g3-real-ai-feasibility`<br>
**DRAFT_PR:** None<br>

### COMPLETED_WORK

- Inspected existing Chat contracts and composition on PR #123. `ModelChatPort.send(ModelChatRequest)` is suspend-based and returns a complete `ModelChatResponse`; `MockModelChatPort` is explicitly deterministic demo behavior.
- Existing normal Chat composition is local demo only: `RoutedModelChatPort → EgressCheckedModelChatPort (LOCAL_DEMO) → UsageControlledModelChatPort → LocalDemoModelChatPort → MockModelChatPort`.
- Existing cloud route has a fixed Ollama endpoint and requires a fresh, verified model catalog entry, exact per-send egress approval, a user-authored PUBLIC payload, credentials, and enabled settings. `CloudSettings.enabled` defaults to `false`. No provider call was made.
- Inspected the existing accepted ADR and AI activation plan. They already say that no real provider is enabled, and list Owner decisions required before activation.
- Compared the existing Ollama Cloud candidate, OpenAI API, Gemini API, ML Kit GenAI/AICore, and an isolated `llama.cpp` + official Qwen GGUF route against current official terms, age/region conditions, data handling, pricing model, hardware support, and code integration constraints.
- Determined why a real local inference PoC could not be validated in this worktree: `/workspace/android-sdk` has no Android NDK installation; `cmake` and `ninja` are not installed/in PATH; no inference model is present. The official Qwen Q8 GGUF model file is **639,446,688 bytes**. It was not downloaded because this host lacks the native toolchain and a download alone cannot prove Android inference, cancellation, memory use, or Pixel 7a performance.
- No source code, dependency, build configuration, provider setting, or production composition was changed.

### CHANGED_FILES

- `docs/agent-reports/g3-ai.md` (this report only; no runtime or product changes).

### TEST_COMMANDS / TEST_RESULTS

- Source inspection: `rg -n 'ModelChatPort|CloudSettings|cloud.*enabled|RoutedModelChatPort|DemoChat' ...` and targeted `sed` reads — completed; findings above.
- Toolchain inventory: `command -v cmake`, `command -v ninja`, and `find /workspace/android-sdk ... -name 'ndk*'` — no CMake, Ninja, or NDK found.
- Device inventory: `adb devices -l` — daemon started; no devices attached.
- Official model metadata: queried Hugging Face model metadata API for `Qwen/Qwen3-0.6B-GGUF` — `license=apache-2.0`; sole `.gguf` entry observed `Qwen3-0.6B-Q8_0.gguf`, size `639446688` bytes.
- No build or tests were run: no PoC implementation was added. There is no inference result, emulator result, ARM64 execution result, or Pixel 7a benchmark to report.

**CI_RUN_IDS:** None<br>
**ARTIFACT_IDS:** None

### SECURITY_IMPACT

- No credential, permission, manifest, network allowlist, egress policy, usage accounting, provider activation, model artifact, or app runtime behavior changed.
- A future cloud adapter must remain behind `RoutedModelChatPort → EgressCheckedModelChatPort → UsageControlledModelChatPort → provider`; preserve egress evaluation before reservation/dispatch. Google-origin email/calendar content must stay blocked from cloud egress absent a separately approved, explicit path.
- A future on-device model must have a distinct typed local target/model identity. The current security contract only defines `EgressTarget.LOCAL_DEMO` and binds it to `serea-demo/local-chat`; do not reuse that demo identity to disguise a real model. Add a separately reviewed local-model destination and provenance gate before integration.
- Native runtime and model weights require independent supply-chain review: pin an immutable upstream revision, audit bundled/transitive licenses, generate SBOM, verify downloaded model SHA-256 at load, reject untrusted model files, and keep weights out of ordinary app/diagnostic logs.

### COMPATIBILITY_IMPACT

- Product compatibility is unchanged; only a report was added.
- Existing `ModelChatPort` is a plausible adapter seam, but its return contract is whole-response (not token streaming). The adapter can still suspend and propagate cancellation; true streaming requires a separately reviewed contract change and UI work.
- `llama.cpp` documents an Android NDK `arm64-v8a` build using `ANDROID_PLATFORM=android-28`; Serea supports `minSdk=26`. API 26 runtime compatibility has not been established and requires an actual API 26 build/run test.
- ML Kit Prompt API documentation says Android API 26+, but current supported-device lists exclude Pixel 7a.

### AI route comparison

| Route | Eligibility / terms | Execution / device and memory | License / model provenance | Integration / security | Pixel 7a unknowns | Decision |
|---|---|---|---|---|---|---|
| **Ollama Cloud** (existing fixed adapter) | Ollama Terms require a user of the Service to be 18+. Owner has not confirmed audience age, account eligibility, or release/user region; do not infer adults-only from “Personal Beta.” | Cloud inference; model memory is provider-side. Official privacy/pricing says hosting is primarily US and may route to Europe. Pay-per-token model pricing and plans can change. App/device inference latency is not measured. | Hosted model weights are not shipped by Serea. Exact model ID and the current hosted model provenance/licensing still require activation-time review; the frozen ADR roster may be stale relative to current catalog/prices. | Adapter exists with default disabled and exact egress gate. Cloud prompt/response processing is described as transient and not trained on; cost is token-based. Cross-border egress and provider account custody remain explicit risks. No network request occurred. | No device-local performance/memory claim. Pixel 7a network quality, end-to-end latency, battery, or data costs are untested. | **Conditional candidate only.** Do not enable pending audience/age and region determination, current model/pricing review, data-flow approval, account ownership, and budget. |
| **OpenAI API** (direct) | Services Agreement disallows minors using service without parent/guardian consent. Under-18 API guidance requires reasonable age-appropriate safety for organizations serving minors and says not to process personal data of children below 13 (or applicable digital-consent age) without approved Zero Data Retention. Access countries are maintained in an official list; Owner account and user/deployment countries are unverified. | Cloud inference; model memory is provider-side. Token-priced per model; rate limits/pricing change and must be rechecked before approval. No API call made, so no measured latency. | Hosted model weights are proprietary/provider-hosted; no weight redistribution. Select an exact currently supported model only after official source, data, and price review. | No OpenAI adapter exists in PR #123. API data is not used to train by default, but abuse logs may include content and default retention is up to 30 days; ZDR/MAM require prior approval and have limitations. Never embed a direct API key in the APK; key custody/backend requires a separate decision. Preserve Serea egress/usage ordering. | Network latency, availability, battery, carrier cost, and any API region/data-residency behavior from Pixel 7a are untested. | **Potential candidate only** after audience, region, safety, retention, model, billing, and credential-custody decisions. Not connected/authorized. |
| **Gemini API / AI Studio** | Additional Terms (effective 2026-03-23): developer must be 18+, API clients must not be directed to or likely accessed by under-18s, and service is for professional/business development, not consumer use. Owner account country and audience remain unverified. | Cloud inference; model memory is provider-side. Only listed regions allowed; EEA/UK/Switzerland clients must use paid services. On unpaid service, prompts/responses may be used to improve products and human reviewed; do not send sensitive/confidential/personal data. Paid access costs and requires explicit approval. | Hosted weights cannot be extracted or redistributed under the service terms. Exact model is not selected. | No adapter wired. Current no-unapproved-egress rule blocks usage. Google free-tier data use is incompatible with sending personal assistant content; paid terms do not itself authorize Serea's egress. | Pixel 7a bandwidth, latency, battery and supported-country service access not tested. | **Not suitable for this consumer Personal Beta under current terms.** No workaround/activation. |
| **ML Kit GenAI / Gemini Nano (AICore)** | ML Kit GenAI Additional Terms require developer age 18+ and prohibit apps directed to or likely accessed by under-18s. Prompt API is marked Beta; terms prohibit Preview/Experimental use in production. | On-device inference, with AICore managing model distribution and execution; memory/limits are model/device managed and no Pixel 7a budget is published here. Some feature-specific APIs only perform narrow tasks; Prompt API is Beta. | Google-managed model weights; no Serea model artifact or weight distribution. AICore/ML Kit terms and attribution/data disclosure obligations apply. | SDK reports input/output are processed on-device, but it may send API performance/utilization metrics to Google; user disclosure may be required. Uses AICore and has per-app quotas / foreground-use restrictions in official docs. | Pixel 7a is **not** on the current official Prompt or feature-specific supported-device lists. No runtime `checkStatus`, AICore availability, memory, latency, or API 26/35 test occurred. | **Not a valid Pixel 7a target-device route today.** Support, production status, and age conditions remain disqualifying/unresolved. |
| **`llama.cpp` + official Qwen3-0.6B GGUF** | Technically no service-account age or region requirement for local inference; applicable local model use terms still apply. `llama.cpp` code is MIT; official Qwen model card reports Apache-2.0. | Inference can be fully local/offline. Official Q8 GGUF file is **639,446,688 bytes**. Model resident memory, KV/cache, peak/free RAM, thermals, token rate, and storage/update impact are **unknown**; weight bytes are not a memory benchmark. Upstream documents Android `arm64-v8a` NDK build, with its example targeting API 28. | Official Qwen repo/model card is clear; exact Q8 file was identified through official model metadata, but it was **not downloaded**, SHA-256 not recorded, and redistribution/notices/transitive runtime licenses are not yet audited. | No app integration. A real local adapter must get a distinct egress target/model identity from `LOCAL_DEMO`, keep requests offline, verify artifact hash at load, reject corrupt files, honor coroutine cancellation, free native context, and preserve Usage behavior. NDK/JNI and SBOM review required. | No ARM64 model-load/generation run, no API 26 compatibility proof, and no Pixel 7a RAM/latency/battery evidence. Host/x86 or emulator throughput cannot substitute for Pixel 7a. | **Feasibility only.** Technically promising but PoC is blocked in this environment; no PASS/performance claim. |

### Official source register (retrieved 2026-10-27 UTC)

Provider policies are facts about provider conditions, not proof that the Owner’s own age, account, billing profile, or deployment/user region qualifies. Those Owner-specific facts remain **unverified**.

- Ollama Terms of Service (updated May 2026; age 18+): <https://ollama.com/terms>
- Ollama Privacy Policy (updated March 2026; local vs cloud handling): <https://ollama.com/privacy>
- Ollama pricing/model rates and hosting location: <https://ollama.com/pricing>
- OpenAI Services Agreement (minor users require parent/guardian consent): <https://openai.com/policies/services-agreement/>
- OpenAI API under-18 guidance: <https://developers.openai.com/api/docs/guides/safety-checks/under-18-api-guidance>
- OpenAI API data controls (training, abuse retention, ZDR eligibility): <https://developers.openai.com/api/docs/guides/your-data>
- OpenAI API supported countries: <https://help.openai.com/en/articles/5347006-openai-api-supported-countries-and-territories>
- OpenAI model pricing (dynamic model-specific rates): <https://developers.openai.com/api/docs/pricing>
- Gemini API Additional Terms (effective March 23, 2026; age, audience, business/consumer, region, free/paid data terms): <https://ai.google.dev/gemini-api/terms>
- Gemini API available regions: <https://ai.google.dev/gemini-api/docs/available-regions>
- Android Gemini Nano / AICore overview: <https://developer.android.com/ai/gemini-nano>
- ML Kit GenAI API overview/device support; docs last updated October 23, 2026: <https://developers.google.com/ml-kit/genai>
- ML Kit GenAI Additional Terms (age, preview/production, metrics disclosure): <https://developers.google.com/ml-kit/genai-terms>
- ML Kit Prompt API setup and API 26+ requirement: <https://developers.google.com/ml-kit/genai/prompt/android/get-started>
- `llama.cpp` Android arm64 NDK build documentation: <https://github.com/ggml-org/llama.cpp/blob/master/docs/build.md#arm-kleidiaitm>
- `llama.cpp` license: <https://github.com/ggml-org/llama.cpp/blob/master/LICENSE>
- Official Qwen3-0.6B GGUF model card/license: <https://huggingface.co/Qwen/Qwen3-0.6B-GGUF>

### KNOWN_ISSUES

- No approved/connected real AI provider exists in the inspected app composition. The ordinary Chat screen remains a deterministic local demo, not real AI.
- Cloud routes remain Owner-blocked for eligibility, age/audience, location, price/budget, credential custody, and data-transfer decisions. Provider-level terms have been documented separately from Owner facts.
- Gemini Nano/AICore does not list Pixel 7a as supported in the current official device list. The owner’s physical Pixel 7a is unavailable; no device query or performance test occurred.
- There is no valid local inference PoC. Model load/generation, multi-turn, cancellation, release, recovery, corrupt-file rejection, offline behavior, memory, latency, API 26 compatibility, emulator/ARM64 execution, and Pixel 7a performance are all **NOT_RUN**.
- The current local egress identity is explicitly demo-only. A real local inference route needs a new separately reviewed local target/catalog/provenance contract and must not be made to appear as cloud AI or demo output.
- Pinned library revision, transitive license inventory, SBOM, model SHA verification, model download/update plan, resource constraints, and safety evaluation remain open.

### CURRENT_BLOCKERS

- Tooling: Android NDK/CMake/Ninja are absent from this workspace; there is no Android ARM64 device available. This prevents an Android JNI build and on-device runtime validation here.
- Exact tool/device evidence: `command -v cmake` and `command -v ninja` emitted no executable path; `/workspace/android-sdk` contained no `ndk*` directory; `adb devices -l` listed no devices.
- Artifact: the smallest verified GGUF file found in the selected official repository is Q8 at 639,446,688 bytes. It was intentionally not downloaded because without native toolchain/device the large artifact would add no valid inference evidence.
- Owner facts: intended user age/audience, provider account eligibility, launch regions, usage budget, model allowlist, and acceptance of each provider’s data terms are unknown. These do not block isolated Fake/offline development but block real service enablement.

### INTEGRATION_DEPENDENCIES

- G4 must not replace ordinary Chat with any real route on the basis of this research.
- A cloud integration requires Owner approval of a specific provider/model, intended user age and supported countries, data terms/retention, cost ceiling, and operator credential custody. It must retain Egress → Usage reservation → dispatch ordering and keep Google-origin data blocked by default.
- A local model integration requires a dedicated `LOCAL_ON_DEVICE` (or equally explicit) contract separate from `LOCAL_DEMO`, immutable runtime/model provenance, SHA verification, asset/update policy, Android API 26 compatibility evidence, and a model-sized device benchmark.
- No modifications are requested to `MainActivity.kt`, `SereaAppShell.kt`, or composition root.

### REVIEW_REQUEST

Please review the provider eligibility distinctions, especially the split between provider-level age/region rules and Owner-unverified account/audience facts. Confirm that the local route remains explicitly feasibility-only and that G4 does not treat this report as a PoC or real AI E2E.

### Recommended next engineering path

1. Continue product-independent Fake/offline Chat and test work; keep `CloudSettings.enabled=false` and do not bind a real API key/provider.
2. For a local PoC, provision a pinned NDK/CMake/Ninja toolchain and an isolated ARM64 emulator/device environment first. Confirm build output/API compatibility against Serea's API 26 minimum before obtaining weights.
3. If that setup is available, fetch the official Qwen artifact only under a recorded source revision, license copy, exact size, and SHA-256. Keep it as an external test fixture, not inside product APK or normal test artifacts.
4. Implement a test-only `ModelChatPort` adapter and test load/single-turn/multi-turn, cancellation, native release, corrupt/hash-mismatch rejection, error recovery, and zero-network behavior. Treat every test as NOT_RUN until it actually runs with the real weights.
5. Collect separate ARM64-emulator and Pixel 7a measurements for peak RAM, startup, tokens/sec, thermal behavior, storage, cancellation latency, and process-death recovery. Only Pixel 7a data can answer Pixel 7a performance.
6. Independently audit the pinned runtime/native dependency licenses/SBOM and add a distinct reviewed `LOCAL_ON_DEVICE` egress/catalog identity before any product-composition work.
7. Revisit Cloud candidates only after Owner records exact target audience age, launch regions, account qualification, model allowlist, data terms/retention, spend cap, credential custody, and any guardian/safety requirements. Provider-term eligibility does not establish the Owner's account or users' eligibility.

### Explicit no-test / NOT_RUN ledger

These are research observations, not passing tests:

| Check | State |
|---|---|
| Source/contract inspection | DONE (read-only) |
| Official policy/runtime/model metadata review | DONE (read-only, sources above) |
| Project build / compile | NOT_RUN |
| Model download / hash check | NOT_RUN (intentionally not downloaded) |
| Runtime/model load | NOT_RUN |
| Single-turn generation | NOT_RUN |
| Multi-turn generation | NOT_RUN |
| Cancellation / native context release | NOT_RUN |
| Corrupt-model rejection / recovery | NOT_RUN |
| Offline/no-socket verification | NOT_RUN |
| Android Emulator inference (any API/ABI) | NOT_RUN |
| Android ARM64 inference | NOT_RUN |
| Pixel 7a memory/latency/battery | NOT_RUN |
| Cloud provider connectivity / eligibility | NOT_RUN; provider eligibility conditions are documented, Owner account/audience/region are unverified |

### NEXT_ACTION

Proceed with Fake/offline development. For a future isolated local PoC, first provision a reproducible native toolchain (pinned NDK/CMake/Ninja) and a controlled ARM64 emulator/device run. Then acquire the official Qwen model only under an explicit artifact-size/hash/license record; implement test-only `ModelChatPort` adapter and lifecycle/corrupt/offline tests. Before product integration, prove API 26 compatibility and obtain Pixel 7a memory/latency evidence. Keep real Cloud use disabled until Owner eligibility and data/budget decisions are recorded.
