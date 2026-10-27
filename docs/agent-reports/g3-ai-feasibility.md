# G3 — Real AI / On-device Inference Audit

**AGENT_ID:** G3 — Real AI / On-device Inference Feasibility
**ACTIVE_PHASE:** G1–G4 read-only audit of frozen product candidate and isolated inference PoC
**OVERALL_STATUS:** PARTIAL — ordinary Chat remains an explicit local demo. The isolated API 35 PoC loaded its pinned model, but generation did not reach the native gate; no generated answer is evidenced.
**BASE_SHA:** PR #123 `3924b36080ffa43331f065a15db80c604c243355`; PR #126 base is that SHA; PR #127 base is PR #126 at `8ca55a32096bc4449717c2fd3332b0602381d7d5`.
**AUDIT_TARGET_HEADS:** PR #123 `3924b36080ffa43331f065a15db80c604c243355`; PR #126 `8ca55a32096bc4449717c2fd3332b0602381d7d5`; PR #127 `f96e77a798575c971313599ded8b267a353d5b46`.
**WORKTREE:** independent docs-only worktree, based on PR #127
**BRANCH:** `codex/serea-g3-ai-evidence`
**DRAFT_PR:** none; this docs-only branch is pushed to `origin` and has no PR.

## Continuation — exact-SHA recovery rerun queue blocker (2026-10-27 17:11 UTC)

The G4 run `40546007995` reached terminal status before this continuation was recorded. Its Android Storage and Emulator job and temporary-key Identity job were both **CANCELLED**; no Emulator job remains active. The separate Pixel 7a evidence-bundle job also reached a terminal state. This was not a G3 test result.

One rerun request was made for the existing cancelled G3 diagnostic run `40533524841`. It retains the exact PR #127 source SHA `f96e77a798575c971313599ded8b267a353d5b46`, branch/ref `codex/serea-g3-diagnostic-recovery`, push event, and existing workflow `G3 On-device Inference PoC (Non-Required)`. No new source commit or branch update was made.

- At `2026-10-27T17:11:14Z`, GitHub's top-level run API reported `status=queued`, `run_attempt=1`, no conclusion, and `updated_at=2026-10-27T16:55:12Z`.
- The Jobs API returned no jobs. `GET /actions/runs/40533524841/attempts/2` returned HTTP 404. The original attempt 1 is the earlier cancelled run; the requested retry has not materialized as an attempt.
- The normal cancellation call `gh run cancel 40533524841` failed with “Cannot cancel a workflow run that is completed.” The REST force-cancel call returned HTTP 409: “Cannot cancel a workflow re-run that has not yet queued.” No other run was cancelled, and no second dispatch was made.
- The pending retry has no job, Emulator, artifact, JUnit, APK build, model fetch/stage, or cleanup result. These are **NOT_RUN / NOT_CREATED**, not PASS or FAIL. The prior attempt's artifact `12748226296` is the cancelled-run artifact described above and is not evidence for this pending request.
- No model, test, or Emulator activity was initiated during this queue investigation. Source SHA/ref remains unchanged.

**Classification:** `ENVIRONMENT / CI_QUEUE`. GitHub exposes an unmaterialized rerun request that cannot yet be cancelled or observed as attempt 2. Do not start another G3 run while this request's status is unresolved, and do not treat the queued state as a test result.

**Next recovery condition:** after the G4 dependency/security repair is stable at its final head and no Required Emulator job is active, the Orchestrator will coordinate one exact-SHA G3 recovery attempt. First resolve the pending/unmaterialized request; if GitHub materializes it, use that exact recovery attempt. If it remains stuck, document the queue limitation and use only an Orchestrator-approved way to execute the existing diagnostic at the same `f96e77a…` source SHA. Do not substitute a newer G3 head or run alongside G4 Emulator work.

## Independent evidence recheck after Orchestrator resume (2026-10-27 17:56 UTC)

I re-read the live GitHub run and downloaded the original API 35 artifact through the GitHub artifact connector, independently checking its bytes and JUnit instead of relying only on this report's earlier transcription.

- Run `40515091197` is **FAILURE** on source `8ca55a32096bc4449717c2fd3332b0602381d7d5`; its only job is `Pinned llama.cpp model lifecycle on disposable API 35 Emulator` and it failed at that job's one instrumentation test. The artifact endpoint reports artifact `12740455266`, ZIP digest `sha256:d0a22bc992cd609d50ea1e65a2d5ef6b226c813528fd24a8aca2197a6c88b33e`. Downloaded artifact ZIP produced the same SHA-256.
- The XML JUnit has exactly `tests=1`, `failures=1`, `errors=0`, `skipped=0`; the sole method is `pinnedModelLoadsGeneratesCancelsReleasesAndRejectsCorruptFile`. Gradle build and APK verification succeeded; model file download, hash validation, and app-private staging succeeded. The failure is the assertion that the test must enter its first native generation gate, at `OnDeviceInferenceTest.kt:97` in that source. No generated answer is present.
- The Emulator metadata confirms API 35, `x86_64`, `ranchu`, `google_apis` fingerprint `google/sdk_gphone64_x86_64/emu64xa:15/AE3A.240806.043/12960925:userdebug/dev-keys`, `boot_completed=1`, and KVM available. Artifact app/test APK hashes match the installed package hashes. This makes an Emulator boot/installation failure an unsupported explanation for this particular test failure.
- The exact failing source used `async { port.send(...) }` inside `runBlocking`, then synchronously waited on a `CountDownLatch` from that same coroutine event-loop thread. The child was scheduled on the blocked event loop and could not submit its native request during the wait. The recovery source at `f96e77a798575c971313599ded8b267a353d5b46` changes that call to `async(Dispatchers.IO)`, consistent with the observed gate timeout being a **TEST_HARNESS scheduling defect**. The change's runtime effect is not yet verified.
- `model-cleanup.txt` confirms the temporary Emulator copy was removed, but `app_private_model_removed=false` and records `DELETE_FAILED_INTERNAL_ERROR`; post-test `app-meminfo-after.txt` says no process was found. Because the runner/AVD is disposable, the device is terminated after the run, but explicit in-container deletion of the app-private model was not proven. Do not claim cleanup PASS for the app-private copy.
- The current recovery request `40533524841` remains `queued`, `run_attempt=1`, source `f96e77a798575c971313599ded8b267a353d5b46`; GitHub reports zero jobs and zero artifacts. This is **NOT_RUN**, not an inference test result. Artifact `12748226296`, previously mentioned as a cancelled-run artifact, currently returns HTTP 404 from GitHub and cannot be independently retrieved; do not use it as current evidence.

### Current official eligibility and device-support check

Official Google pages were checked on 2026-10-27. Gemini API Additional Terms effective 2026-03-23 require the developer to be at least 18, prohibit serving an app directed to or likely accessed by under-18 users, and describe Gemini API/AI Studio for professional or business development rather than consumer use. That makes Gemini API unsuitable for this Serea Personal Beta use case as documented; no workaround is appropriate. ML Kit GenAI terms likewise require an 18+ developer and disallow API clients directed to or likely accessed by under-18 users; preview/experimental services cannot be used in production. Current ML Kit device-support docs list Pixel 9/10 generations and selected other devices, not Pixel 7a. These terms/device facts are not a substitute for Owner-specific age, audience, region, eligibility, or device confirmation.

- [Gemini API Additional Terms](https://ai.google.dev/gemini-api/terms)
- [ML Kit GenAI Additional Terms](https://developers.google.com/ml-kit/genai-terms)
- [ML Kit GenAI device support](https://developers.google.com/ml-kit/genai)
- The Hugging Face repository metadata independently reports Apache-2.0 for both the pinned GGUF repository and its SmolLM2 base model; llama.cpp at pinned revision `276378515ceebee5d14bcc8536603dd5c87491a2` is MIT. This is repository metadata, not an independent audit of training-data provenance or every redistribution obligation.

**Disposition remains unchanged:** an offline local model is the only currently plausible P0 route, but its generation/runtime lifecycle has no passing Emulator evidence. Cloud eligibility is not established and Gemini API is inappropriate for consumer Personal Beta. Do not wire this PoC into ordinary Chat or label any synthetic provider response as real AI.

## Host-only diagnostic checks on exact recovery source (2026-10-27 18:30 UTC)

These three checks were run from a clean checkout of exact recovery source `f96e77a798575c971313599ded8b267a353d5b46`, tree `b737967dcf760990f6b2323afebddc45b7f29ea0`, in `/workspace/Serea-g3-diagnostic-recovery`. No app/test source was modified. They do not download model weights or start an Emulator.

| Command | Exit | Result |
|---|---:|---|
| `python3 tools/g3-ondevice-poc/scripts/test-gradle-project-dir.py` | 0 | `runner_and_androidtest_source_contract=PASS gradle_callers=prepare_build,run_model_test` |
| `python3 tools/g3-ondevice-poc/scripts/test-runner-failure-paths.py` | 0 | `runner_failure_paths=PASS optional_psi_denied=recorded adb_hang=bounded later_cleanup=continued utp_uninstall=verified primary_exit=preserved models=removed state=reset` |
| `G3_EVIDENCE_DIR=/tmp/serea-g3-hostcheck-f96-evidence RUNNER_TEMP=/tmp/serea-g3-hostcheck-f96-runner bash tools/g3-ondevice-poc/scripts/run-diagnostic.sh test-download-bound` | 0 | `bounded_download_test=PASS fixture_bytes=33 maximum_bytes=32 curl_status=63 partial_bytes=32` |

The `test-download-bound` command uses only a locally created 33-byte synthetic file through `file://`, validates a 32-byte cap, and removes its temporary directory. Its `curl: (63) Exceeded the maximum allowed file size` line is the expected rejection. The cleanup regression uses a fake `adb` and synthetic paths/files only. No Google/model download, real device, Android SDK invocation, Gradle build, Emulator, or GitHub workflow was started. The queued run `40533524841` was left untouched.

**Disposition:** these host-only guard checks PASS for the exact recovery source, but they do not verify native generation, Android runtime behavior, model cleanup on a real Emulator, ARM64 runtime, or Pixel 7a performance. Those remain NOT_RUN / NOT_CONFIRMED.

## Findings and disposition

### Product Chat / cloud egress

No blocking product vulnerability was found in the reviewed source. This is a source review, not a new test run.

- `android/app/src/main/java/app/serea/android/MainActivity.kt:115` constructs the ordinary ChatViewModel with `DemoChatComposition.create(usage)`. `MainActivity.kt:216–224` implements a deterministic mock and explicitly says no cloud AI was contacted. The UI labels the route DEMO and Cloud disabled (`MainActivity.kt:902–915`). The Cloud test action is disabled (`MainActivity.kt:1095–1097`).
- `DemoChatComposition.kt:5–50,71–85` names a fictional local demo model/provider, limits routing to `serea-demo`, and validates the exact local contract. It has no credential or HTTP dispatch path.
- `CloudProvider.kt:24–26` defaults `CloudSettings.enabled` to false. `CloudProvider.kt:292–315` builds `RoutedModelChatPort(EgressCheckedModelChatPort(UsageControlledModelChatPort(adapter)))`; the egress wrapper evaluates before usage reservation and adapter dispatch. The adapter reads credentials and constructs an HTTP request only downstream of the typed approval (`CloudProvider.kt:111–154`).
- `SecurityPorts.kt:56–118,121–162` uses a fixed cloud destination, short-lived explicit intent, bounded payload, and PUBLIC-only cloud classification. Cloud payload origins are restricted to user-authored/model output; context origins are checked too. Google-origin email/calendar context is denied by these checks. The product's local demo accepts a broader local classification, but it is bound to the deterministic mock and exact `LOCAL_DEMO` target.
- PR #123 includes `GoogleOriginEgressAdversarialTest` and `LocalFakeRoutingAdversarialTest`; its body records JVM Provider/Chat/Egress **199/199 PASS** on the candidate SHA. This audit did not rerun those tests.

**Integration requirement:** the current product contract defines `LOCAL_DEMO`, not a real on-device inference identity. Do not make the G3 JNI adapter appear under `serea-demo/local-chat`; add a separately reviewed target, catalog/provenance record, and safety contract before any product integration.

### G3 PoC runtime evidence

Run **40515091197** executed on PR #126 source SHA `8ca55a32096bc4449717c2fd3332b0602381d7d5`. It used an API 35 `google_apis` x86_64 Emulator, `ranchu`, fingerprint `google/sdk_gphone64_x86_64/emu64xa:15/AE3A.240806.043/12960925:userdebug/dev-keys`, with `boot_completed=1` and KVM acceleration available.

- Artifact **12740455266**, ZIP SHA-256 `d0a22bc992cd609d50ea1e65a2d5ef6b226c813528fd24a8aca2197a6c88b33e`: <https://github.com/Yrika819/Serea/actions/runs/40515091197>
- App APK SHA-256: `13741ec99951497b1ed4c92c57716ddb241a0c620338277fb8f12d060eb1c52a`.
- AndroidTest APK SHA-256: `6dd71875482e73ab2aa4f0427c21d445bc0ee22ada1a8426a0434ccca0cff8d0`. Artifact verification records matching installed APK hashes.
- The runner downloaded and staged the exact fixture: repository `ggml-org/SmolLM2-135M-GGUF`, revision `1076b85d135017971845b19b8d07df1479155988`, file `SmolLM2-135M-Q4_K_M.gguf`, size `101016128`, SHA-256 `e3131339bf4e8065265593d4fd8f7bb7ff2d3abff1edb5618aa1197b89cad9f5`. `model-status.txt`, `model-stage-metadata.txt`, and `adb-model-stage.log` record the verified/staged file. The file is not in the repository or uploaded artifact.
- JUnit `androidTest-results/debug/TEST-emulator-5554 - 15-_app-.xml` reports exactly `app.serea.g3poc.OnDeviceInferenceTest.pinnedModelLoadsGeneratesCancelsReleasesAndRejectsCorruptFile`: **1 test, 1 failure, 0 errors, 0 skips**. The assertion at `OnDeviceInferenceTest.kt:97` failed because the native generation gate was not reached in 15 seconds.
- Before that assertion, `port.load()` returned and the first engine's native model-state assertion passed. Therefore **native model load PASS on API 35 x86_64** is supported. The test's generation path did not reach the native callback/gate and emitted no generated response. Single-turn generation is not a PASS; multi-turn, cancellation, release/recovery, and corrupt-file checks after the gate were not reached.
- The old test source creates `async { ... }` on the `runBlocking` event loop and then synchronously blocks that same thread in `CountDownLatch.await()` (`OnDeviceInferenceTest.kt` at source SHA `8ca55a…`, lines 91–100). The deferred child cannot execute while its event-loop thread is blocked. This confirms a **TEST_HARNESS scheduling defect for the observed gate timeout**, not a product model or product Chat failure. PR #127 dispatches these requests to `Dispatchers.IO`; that correction has not been confirmed by a completed Emulator test.
- The failed run's cleanup record says `emulator_temp_model_removed=true`, `app_private_model_removed=false`, and records `DELETE_FAILED_INTERNAL_ERROR`. UTP had already uninstalled the target package, so a direct `run-as` file check was unavailable. The workflow used a disposable hosted runner/AVD, which terminated after the job. The artifact does not explicitly record a host-side model-file-removed assertion.

PR #127 diagnostic run **40533524841** was cancelled to avoid overlap with G4's Required Emulator campaign. Artifact **12748226296**, ZIP SHA-256 `c9bf8c9caf77c60ab26746d04cc6dce41f9bfac2f442fdd8a6a3965d152f12ad`, contains no JUnit result or completed cleanup record. Recovery-source changes compile and host regression checks are reported as passing, but runtime inference and cleanup are **NOT_RUN / NOT_CONFIRMED** on this recovery head.

### PoC provenance and limits

- The standalone project is under `tools/g3-ondevice-poc/`, outside the ordinary Serea Gradle settings. It has package ID `app.serea.g3poc`, no Internet permission, and a test-local structural `ModelChatPort` contract; it does not import or replace Serea's product port.
- The model repository metadata declares Apache-2.0; model revision, file size, and SHA are fixed above. llama.cpp is pinned to commit `276378515ceebee5d14bcc8536603dd5c87491a2` (MIT). No model weights are committed or uploaded. This is not a transitive-license/SBOM audit, and model training-data provenance has not been independently audited.
- Android NDK/API 26 `arm64-v8a` native cross-build and a dual `arm64-v8a`/`x86_64` app build were recorded in G3's build artifacts. That proves build output only. The observed runtime was x86_64 API 35 model load only; ARM64 runtime and API 26 inference are **NOT_RUN**.
- The JNI resources are process-global behind an explicit singleton engine lease. Request IDs are namespaced by engine ID; a second active engine is rejected. Source defines `n_ctx=512`, batch/ubatch 128, two CPU threads, and a greedy sampler. The adapter returns a complete response rather than streamed tokens. The model file is about 101 MB; this is not the runtime RAM requirement. A useful PSS/timing record was not emitted by the failed test. Pixel 7a memory, speed, thermals, battery, and storage impact are all **NOT_RUN**.
- PR #127 adds source-level tests for second-engine rejection, cancellation isolation, queued/in-flight cancellation, close/reopen, load/close, and corrupt-model rejection. These assertions were not reached by the failed run and have not passed on an Emulator.

### Provider eligibility and official sources

Provider/service account facts are separate from Owner-specific facts. As of the source review retrieved **2026-10-27 UTC**, the Owner's age, account country, user audience, billing profile, and deployment region remain **OWNER-UNVERIFIED**. No live provider or credential was used.

| Route | Eligibility / terms | Integration and decision |
|---|---|---|
| Ollama Cloud | Its Terms require a service user to be at least 18. Account/audience/region facts for Serea are unknown. Pricing is token/plan based and may change. | Existing product adapter remains disabled by default. Conditional only after eligibility, model, pricing, data-flow, and Owner approval. No request was made. |
| OpenAI API | Official under-18 guidance requires suitable protections for products serving minors; Services Agreement and supported-country conditions also apply. Owner and deployment eligibility are unknown. | No adapter in PR #123. Treat as conditional pending user-age/audience, region, data-retention, model/pricing, billing, and credential-custody decisions. No API request/key was used. |
| Gemini API | Current Additional Terms restrict developer/audience eligibility and frame service for professional/business rather than consumer development; regional and free/paid conditions apply. Owner account eligibility remains unknown. | Not suitable for this consumer Personal Beta under the reviewed terms; no workaround or activation. |
| ML Kit GenAI / Gemini Nano / AICore | ML Kit GenAI terms have age and production-use conditions; the Prompt API is documented as Beta. | Current official device support does not list Pixel 7a. No `checkStatus`, API call, or device test was performed. |
| llama.cpp + SmolLM2 | Local inference has no cloud account eligibility, but software/model licensing still applies. | Feasibility only. Apache-2.0 model metadata and MIT runtime are recorded; API 35 x86_64 load is observed, generation and Pixel-device suitability are not. |

Official primary sources:

- Ollama: [Terms](https://ollama.com/terms), [Privacy](https://ollama.com/privacy), [Pricing](https://ollama.com/pricing)
- OpenAI: [Services Agreement](https://openai.com/policies/services-agreement/), [Under-18 API guidance](https://developers.openai.com/api/docs/guides/safety-checks/under-18-api-guidance), [API data controls](https://developers.openai.com/api/docs/guides/your-data), [supported countries](https://help.openai.com/en/articles/5347006-openai-api-supported-countries-and-territories), [pricing](https://developers.openai.com/api/docs/pricing)
- Google: [Gemini API Additional Terms](https://ai.google.dev/gemini-api/terms), [Gemini API regions](https://ai.google.dev/gemini-api/docs/available-regions), [ML Kit GenAI device support](https://developers.google.com/ml-kit/genai), [ML Kit GenAI Additional Terms](https://developers.google.com/ml-kit/genai-terms), [AICore overview](https://developer.android.com/ai/gemini-nano)
- Local inference: [SmolLM2 GGUF repository](https://huggingface.co/ggml-org/SmolLM2-135M-GGUF), [llama.cpp Android build docs](https://github.com/ggml-org/llama.cpp/blob/master/docs/build.md#arm-kleidiaitm), [llama.cpp pinned license](https://github.com/ggml-org/llama.cpp/blob/276378515ceebee5d14bcc8536603dd5c87491a2/LICENSE)

## Verification and CI

**TEST_COMMANDS:** None for this read-only audit. No model download, provider access, test, or Emulator was run.

**PRODUCT REQUIRED EVIDENCE:** PR #123 documents Android Formal run `40427448635`, API 26 and API 35 each 105/105 on the exact candidate SHA. This is evidence for PR #123 only; it is not inherited by #126 or #127.

**G3 RUNS:**

- `40515091197`: AndroidTest compile and one test execution; 1 failure at native gate, no inference pass. Artifact `12740455266`.
- `40533524841`: cancelled before JUnit completion; inference and cleanup **NOT_RUN / NOT_CONFIRMED**. Artifact `12748226296`.
- PR #127's other platform checks passing does not turn its cancelled diagnostic into an inference pass. Android Required jobs classified/skipped for tool-only scope are **NOT_RUN**, not Required PASS.

**CURRENT_BLOCKER (historical audit snapshot):** At the initial audit check, G4 run `40533346554` remained in its API 35 Emulator stage; its Android Dependency Audit had failed. That run is not the current G4 attempt; see the continuation above.

## Common report contract

**CHANGED_FILES:** In the audit itself, none. This durable branch changes only `docs/agent-reports/g3-ai-feasibility.md` and the stale evidence wording in `tools/g3-ondevice-poc/README.md`.
**SECURITY_IMPACT:** No source, permission, egress, provider, model, or runtime changes. No high/critical product security defect found in the inspected boundaries. The README discrepancy is a low-severity evidence-integrity issue.
**COMPATIBILITY_IMPACT:** None. No provider or PoC integration.
**KNOWN_ISSUES:** Generation gate did not run; PR #127 remedy lacks Emulator verification; prior artifact did not directly prove host model-file removal; README/PR #126 body had stale “not downloaded/not compiled” wording.
**CURRENT_BLOCKERS:** The exact-SHA G3 recovery rerun is an unmaterialized GitHub Actions queue request, with no jobs or attempt 2, and cannot yet be cancelled. G4 run `40546007995` is terminal with its Emulator and Identity jobs cancelled. Provider-specific Owner eligibility/approval is separate and does not block offline source/evidence work.
**INTEGRATION_DEPENDENCIES:** None for normal Chat. Any real local model needs a separate target/catalog identity, provenance and license review, memory/cancellation/process lifecycle evidence, product tests, and integration review.
**REVIEW_REQUEST:** Orchestrator read-only review of this docs-only checkpoint.
**NEXT_ACTION:** After the G4 repair/head is stable, have the Orchestrator resolve and coordinate the one exact-SHA recovery run as described above. Do not present model-load-only evidence as inference success.
**OWNER_APPROVAL_REQUIRED:** Real OAuth/provider accounts, credentials, paid AI requests, Pixel 7a use, production signing/distribution, main merge, and final GO remain owner-controlled.
