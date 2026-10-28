# G3 — Real AI / On-device Inference Feasibility

## Latest G3-owned checkpoint — model-metadata evidence wording corrected (2026-10-28)

An independent review found that an older README sentence overstated what the metadata-only request established. This follow-up separates repository/license/base-model metadata from model-file fixture expectations and the later byte-level weight-fetch check.

**AGENT_ID:** G3 — PR #127 owner<br>
**ACTIVE_PHASE:** correct metadata-evidence wording and prevent regression<br>
**OVERALL_STATUS:** PARTIAL — documentation boundary is corrected and five offline host regression suites pass; no Android rebuild or runtime inference was performed.<br>
**BASE_SHA:** `eecfabe9e6fd1b7a016658000de793083a6a8ada`<br>
**CURRENT_HEAD:** `3323ba943074ef2b794a3f1ca700dd46c93f107d`<br>
**CURRENT_TREE:** `1cb47c8b6b16e733ae3c9b2632f1f9f73b827747`<br>
**WORKTREE:** `/workspace/Serea-g3-diagnostic-recovery` — clean after the source/documentation commit<br>
**BRANCH:** `codex/serea-g3-diagnostic-recovery` — G3-owned<br>
**DRAFT_PR:** [#127](https://github.com/Yrika819/Serea/pull/127), Open/Draft; no merge.<br>
**CHANGED_FILES:** `tools/g3-ondevice-poc/README.md`; `tools/g3-ondevice-poc/scripts/test-poc-metadata-contracts.py`.<br>
**CORRECTION:** README now says the immutable repository metadata response confirms the `apache-2.0` declaration and base-model metadata only. Filename/byte-count/SHA in the model fixture are expected values, not confirmed by that response; the selected weight-fetch step compares actual downloaded bytes against fixture size/SHA. The metadata capture downloads no model weights.<br>
**REGRESSION:** the metadata-contract host test requires exactly one README metadata-only claim and checks for the repository-metadata-only, expected-fixture-values, and separate weight-fetch boundaries.<br>
**FOCUSED_TESTS:** all five passed with `python3 -B`: `test-api-selection.py`; `test-gradle-project-dir.py`; `test-native-source-contract.py`; `test-poc-metadata-contracts.py` (including the new README regression); `test-runner-failure-paths.py`. `git diff --check` PASS.<br>
**FILE_SHA256:** `README.md` `d1561507d88bcf604d032a8387eb61fefe2a6af7d2cf7df2e28aca97e2ab0b94`; `test-poc-metadata-contracts.py` `4f70326e857a5fa98c49b4a3df0f3c0f39a24ee25eefad2fd83668b99ceba165`.<br>
**NATIVE_BUILD / ALLOCATION-FAULT-INJECTION / API26 / API35:** NOT_RUN. The no-network/no-Emulator restriction remains; no model weights were fetched.<br>
**NEXT_ACTION:** Orchestrator re-review and incorporate only after confirming the documentation/test diff and exact-head host evidence. Keep native compile/runtime and allocation-failure behavior explicitly unverified.<br>

## Latest G3-owned checkpoint — native PoC review findings closed (2026-10-28)

This checkpoint records the focused G3-only repair after the independent source review. It does not claim an Android APK rebuild, AndroidTest compilation, model inference, or Emulator result. No model weights, network services, real credentials, or device were used.

**AGENT_ID:** G3 — PR #127 owner<br>
**ACTIVE_PHASE:** close native allocation, JNI exception-safety, and license-evidence review findings<br>
**OVERALL_STATUS:** PARTIAL — three review findings are addressed and five offline host regression suites pass. Android native rebuild and runtime verification remain NOT_RUN.<br>
**BASE_SHA:** `844b45bcabb2871fa4f46d0fa9ef5aefcc11d539`<br>
**SOURCE_HEAD:** `3b32050d346b2b7812dea4fb4fb8870b5826b6b1`<br>
**SOURCE_TREE:** `c0c9aca132dca5b717a8dbebaee7707bca1c7ba0`<br>
**WORKTREE:** `/workspace/Serea-g3-diagnostic-recovery` — clean after source commit<br>
**BRANCH:** `codex/serea-g3-diagnostic-recovery` — pushed fast-forward from remote `8d4c5cf77b4a0207ee15d36dad8ae9027722845d`<br>
**DRAFT_PR:** [#127](https://github.com/Yrika819/Serea/pull/127), Open/Draft, base `codex/serea-g3-real-ai-feasibility`. Existing PR retained; no merge or unrelated branch change.<br>
**COMMIT:** `3b32050d346b2b7812dea4fb4fb8870b5826b6b1` (`fix(g3): close native PoC review findings`)<br>
**CHANGED_FILES:** `tools/g3-ondevice-poc/app/src/main/cpp/llama_jni.cpp`; `tools/g3-ondevice-poc/scripts/capture-model-license.py`; `tools/g3-ondevice-poc/scripts/test-native-source-contract.py`; `tools/g3-ondevice-poc/scripts/test-poc-metadata-contracts.py`.<br>
**REVIEW_FINDINGS:** (1) Batch allocation now frees exactly the successfully allocated per-token `seq_id` buffers and all parent buffers on partial failure without relying on `n_tokens` being populated. (2) JNI UTF-16 chars have scoped release; `nativeGenerate` and `nativeUnicodeRoundTrip` catch `std::bad_alloc` and report `OutOfMemoryError`. (3) License evidence labels model filename/size/SHA as fixture-expected values not observed in the license metadata response; the response SHA and pinned repo metadata remain separately captured.<br>
**FOCUSED_TESTS:** `python3 scripts/test-api-selection.py` PASS; `python3 scripts/test-gradle-project-dir.py` PASS; `python3 scripts/test-native-source-contract.py` PASS; `python3 scripts/test-poc-metadata-contracts.py` PASS; `python3 scripts/test-runner-failure-paths.py` PASS; `git diff --check` PASS. The native source-contract test initially failed because it still required the old `make_batch` signature and `llama_batch_free` marker; the test was updated to cover the new cleanup helper and JNI RAII/OOM behavior, then the full host suite passed. No tests were skipped in these host scripts.<br>
**FILE_SHA256:** `llama_jni.cpp` `54d44fa265bbd071d1c956f2b38425f04d30cb2d9de40f1af2eba81a0b50be88`; `capture-model-license.py` `b3998378aa5806a40574cda7504ea1ab29e2a91ba867a19e0bd1723a323dc497`; `test-native-source-contract.py` `ca98631d24f63515fbf5828d86bc04031444bc1be48a08625ad2797591ed274b`; `test-poc-metadata-contracts.py` `ce5c1aecdc4b7f5196eb3a66777f59c4b656577eb1fed27c973cf8ef886cdabe`.<br>
**ANDROID_BUILD / APK_SHA256:** NOT_RUN. The pinned llama.cpp source checkout is not available in this workspace (`G3_LLAMA_CPP_SOURCE` unset; no `llama.h` checkout); no network access was used to restore it. Prior APK/JNI hashes belong to earlier source and do not validate this commit.<br>
**API26_RESULT / API35_RESULT:** NOT_RUN at `3b32050`; no model or Emulator used. The API 35 abort-callback cancellation, partial allocator failure at runtime, and JNI Unicode runtime behavior remain NOT_VERIFIED.<br>
**CI_RUN_IDS / ARTIFACT_IDS:** none for this repair. The non-required model workflow was not dispatched. Existing historical runs/artifacts are not treated as this HEAD's evidence.<br>
**SECURITY_IMPACT:** no product/provider integration, app permissions, cloud egress, or Serea runtime path changed. Model metadata capture remains metadata-only; no weights were fetched.<br>
**CURRENT_BLOCKER:** native compilation and instrumentation/runtime evidence require the exact pinned llama.cpp checkout and AndroidX test dependencies; these were not restored under the no-network/no-Emulator restriction.<br>
**NEXT_ACTION:** Orchestrator to independently review this exact source commit and coordinate any later Android build/Emulator work after the API 26/35 real-IME campaign has released the serialized Emulator slot. Keep real AI, device performance, and product integration unclaimed.<br>
**OWNER_APPROVAL_REQUIRED:** none for these local isolated fixes. Real provider/model activation, live credentials, Pixel 7a, production signing, distribution, merge, and Beta GO remain Owner-gated.<br>

## Latest G3-owned checkpoint — G1 review follow-up (2026-10-28)

This checkpoint addresses G1's low-severity source review findings in the G3-only isolated PoC. No Serea product integration, API workflow dispatch, Emulator, model-weight download, live provider access, or push was performed. The API 26/35 runtime matrix and API35 in-compute cancellation callback are still **NOT_RUN / NOT_VERIFIED** at this source revision.

**AGENT_ID:** G3 — PR #127 owner<br>
**ACTIVE_PHASE:** close review findings and retain exact host/build evidence<br>
**OVERALL_STATUS:** PARTIAL — SDK/model metadata capture, exact APK permission allowlists, defensive JNI batch allocation, and Unicode conversion are implemented. Host/JVM/native-build checks pass; AndroidTest manifest resolution and Emulator runtime remain unavailable/not run.<br>
**BASE_SHA:** `2e03ec96e5b3b1f78c0e7f9ece04923612603dce`<br>
**IMPLEMENTATION_HEAD:** `e5e8783fc7017ca5bb2fc6198521742242a67471`<br>
**IMPLEMENTATION_TREE:** `84118388f41131ffe6a69cc0d93f99fa7c8ba0e9`<br>
**WORKTREE:** `/workspace/Serea-g3-diagnostic-recovery`<br>
**BRANCH:** `codex/serea-g3-diagnostic-recovery` — G3-owned only<br>
**DRAFT_PR:** [#127](https://github.com/Yrika819/Serea/pull/127) remains Open/Draft. No PR metadata, source branch outside this owned branch, or other Agent branch was changed.<br>
**CHANGED_FILES:** `.github/workflows/g3-ondevice-poc.yml`; `tools/g3-ondevice-poc/README.md`; `app/src/main/cpp/llama_jni.cpp`; `app/src/main/java/app/serea/g3poc/GgufModelChatPort.kt`; `app/src/test/java/app/serea/g3poc/ModelChatPortContractTest.kt`; `app/src/androidTest/java/app/serea/g3poc/OnDeviceInferenceTest.kt`; runner and new metadata/permission/native host-check scripts under `tools/g3-ondevice-poc/scripts/`.<br>
**REVIEW_FIXES:** the runner captures the full installed SDK package list and explicit package revisions (platform, build-tools, platform-tools, NDK, CMake, Emulator, selected image). A metadata-only Hugging Face API request verifies the exact pinned repository revision, `apache-2.0` declaration, base model, model filename, size, and SHA; it downloads no weights. Generated target APK permissions must be exactly empty; AndroidTest must request exactly `android.permission.REORDER_TASKS`. JNI batch buffers and every sequence-id allocation are checked before decode, with partial allocations freed and an `OutOfMemoryError` returned. JNI prompt/output conversion now uses strict UTF-16/UTF-8 conversion and `NewString`, avoiding modified UTF-8 for llama.cpp input/output; an instrumentation fixture covers Japanese, accented BMP text, emoji, and malformed lone surrogates. These are isolated PoC changes only.<br>
**CLEANUP_FINDING:** historical API35 run `40515091197`, artifact `12740455266`, recorded `app_private_model_removed=false` after the test runner had uninstalled the target APK. That value is preserved as the observed failure and is not recast as proof of deletion. When direct `adb run-as` removal cannot be verified, the new runner records `app_private_model_removed=not_individually_verified` and the disposable fresh AVD / ephemeral GitHub-hosted runner teardown as the final cleanup boundary. Fake-ADB regression models PackageManager removal of a synthetic private file. Direct app-private deletion remains not individually verified in the already-uninstalled-package path.<br>
**FOCUSED_TESTS:** exact implementation HEAD `e5e8783...`: `test-poc-metadata-contracts.py` PASS; `test-native-source-contract.py` PASS; `test-gradle-project-dir.py` PASS; `test-runner-failure-paths.py` PASS; `test-api-selection.py` PASS; shell syntax PASS; workflow YAML parse PASS; `git diff --check` PASS. Local `:app:testDebugUnitTest --rerun-tasks` PASS, 4/4 tests, 0 failures/errors/skips. Local `:app:externalNativeBuildDebug` and `:app:assembleDebug` PASS for arm64-v8a and x86_64 using pinned llama.cpp commit `276378515ceebee5d14bcc8536603dd5c87491a2`.<br>
**BUILD_EVIDENCE:** target APK SHA-256 `a6e09bfc23c3ee3637338a415b537dc4f1d784afc2e80a232d082b34643ddb6d`; actual `aapt dump permissions` output is only `package: app.serea.g3poc`. JNI library hashes: arm64-v8a `dbd5db39197e79a5ae062e97bf63013c476757494f1e9e318e1caebb199da48a`; x86_64 `b3750462fddeb0ffbc82f5f14f1335d4d4e1de35eaac305bda869992e09b7349`. AndroidTest APK and merged-manifest permissions for this HEAD are **NOT_RUN**. `:app:processDebugAndroidTestManifest` failed before merge because Google Maven was unreachable for `androidx.annotation:1.7.0-beta01`, `androidx.tracing:1.1.0`, and related dependencies. Do not treat the historical API26 artifact `12774231044` as current-head manifest evidence.<br>
**LOCAL_SDK_METADATA:** this workspace's API35 package revisions were platform 2, build-tools 35.0.0, platform-tools 37.0.1, NDK 28.2.13676358, CMake 3.31.6, Emulator 37.2.12, Google APIs x86_64 image 9. This is local SDK evidence, not a CI runner measurement. Workflow artifacts will capture the hosted runner's resolved versions on a future selected diagnostic run.<br>
**MODEL_LICENSE_EVIDENCE:** immutable API endpoint for `ggml-org/SmolLM2-135M-GGUF@1076b85d135017971845b19b8d07df1479155988` returned `apache-2.0`, base `HuggingFaceTB/SmolLM2-135M`, and matches file SHA `e3131339bf4e8065265593d4fd8f7bb7ff2d3abff1edb5618aa1197b89cad9f5` / 101,016,128 bytes. Metadata response SHA-256 `0706cb4f7d4f54abf49b620da9863532cdacbba289ba403a37533c8e43eebb1f`. No model weights were fetched.<br>
**ARTIFACTS:** local evidence files under `artifacts/g3-ondevice-poc/review-e5e8783-*` include the host/JVM JUnit XML, build and focused-check logs, APK/JNI SHA records, target permission dump, and captured model/SDK metadata. No GitHub Actions Run ID or Artifact ID was created for this local review; historical run `40515091197` / artifact `12740455266` remains the failure evidence above.<br>
**API26_RESULT:** no test run at this HEAD — **NOT_RUN**. Prior API26 B/C inference evidence is a historical different revision and does not validate this PoC update.<br>
**API35_RESULT:** no test run at this HEAD — **NOT_RUN**. In-compute abort callback, JNI Unicode runtime fixture, batch allocation failure injection, and current AndroidTest APK permissions remain **NOT_VERIFIED**.<br>
**CURRENT_BLOCKER:** Google Maven dependency network failure prevents current AndroidTest manifest/source compilation. Parent also directed no Emulator or model workflow until R3 IME B/C completes.<br>
**NEXT_ACTION:** G1 re-review this local owned-branch diff. After R3 B/C has completed and Orchestrator clears the serialized Emulator slot, build current AndroidTest APK and run focused API35 then API26 on exact committed source, preserving results. Do not push or run the Emulator before that review/coordination.<br>
**OWNER_APPROVAL_REQUIRED:** none for this local isolated repair. Real provider, live credentials, production signing, Pixel device, main merge, or Beta release were not used.

## Latest G3-owned checkpoint — API 35 generation-start gate failure (2026-10-28)

This follow-up analyzes the earlier API 35 run whose native-start latch assertion failed. It is separate from the API marker repairs below. No product integration, Emulator run, model download, workflow dispatch, push, or live provider access occurred in this follow-up.

**AGENT_ID:** G3 diagnostic recovery — PR #127 owner<br>
**ACTIVE_PHASE:** root-cause analysis for API 35 run `40515091197` and source-contract regression guard<br>
**OVERALL_STATUS:** PARTIAL — high-confidence TEST_HARNESS cause identified; existing dispatcher correction is now guarded by a host source-contract test. The fix has not been rerun on an Emulator.<br>
**BASE_SHA:** `156e3da83cb9c8406fdc8fc6f696cb2a8cf137ae`<br>
**BASE_TREE_SHA:** `ac9d6c40e76797dde93a849f4d93a4f44c382e24`<br>
**TESTED_SOURCE_SHA:** `f90b229dd4b0c625b2a41f409ab8036248a2ab55`<br>
**TESTED_SOURCE_TREE:** `b43239428cc30b756068a28c26353d4ff13dca7c`<br>
**CURRENT_HEAD:** this checkpoint/evidence-only commit follows the tested source commit; reread the owned branch ref when resuming.<br>
**WORKTREE:** `/workspace/Serea-g3-diagnostic-recovery`<br>
**BRANCH:** `codex/serea-g3-diagnostic-recovery`<br>
**DRAFT_PR:** [#127](https://github.com/Yrika819/Serea/pull/127) — Open/Draft; isolated on-device PoC only<br>
**FAILED_RUN:** `40515091197`, artifact `12740455266`, source HEAD `8ca55a32096bc4449717c2fd3332b0602381d7d5`<br>
**FAILURE_RESULT:** API 35 `google_apis` x86_64/ranchu (`google/sdk_gphone64_x86_64/emu64xa:15/AE3A.240806.043/12960925`, boot complete); one test, one failure, zero errors/skips; test duration 17.599 seconds, suite 18.336 seconds. Assertion at `OnDeviceInferenceTest.kt:97`: “the first engine must enter its native generation gate,” after `awaitStartGateForTest(15, SECONDS)`.<br>
**ROOT_CAUSE:** TEST_HARNESS, high confidence. At failed HEAD `8ca55a3`, `val firstRun = async { port.send(...) }` inherited the `runBlocking` event loop. The test then called a blocking `CountDownLatch.await()` on that same event-loop thread. The child could not execute `port.send`, submit native work, or invoke `onNativeGenerationStart` while its dispatcher was blocked; the latch therefore timed out and the assertion failed. This explains the 15-second wait and failure without increasing a timeout. The failed test process loaded `libserea-g3-llama.so` successfully before the assertion. Targeted logcat has no fatal native exception and no `G3_TEST_PHASE` tags (phase logging was added later).<br>
**FIX_AND_GUARD:** the actual harness correction already exists in branch ancestor `f96e77a798575c971313599ded8b267a353d5b46`: latch-bound `firstRun` and other blocking async children use `Dispatchers.IO`. This follow-up adds a host source-contract regression in `test-gradle-project-dir.py` requiring `firstRun` to use `Dispatchers.IO` before the blocking native-start wait and requiring all instrumentation `async` children to specify that dispatcher.<br>
**PRESERVED_FAILURE_EVIDENCE:** `artifacts/g3-ondevice-poc/failure-40515091197-junit.xml` (SHA-256 `0abfb62bb0a8f3fe512b1d8e438631fdb6c2f62f027f652a19a343041e89534d`); `failure-40515091197-logcat-excerpt.txt` (SHA-256 `d628c66ad22c31b028813af27fd8e08bb925af03b12ca82e0206228e37d58aee`); `failure-40515091197-root-cause.txt` (SHA-256 `feabc1d1e5aa159b773c22e7cf94a5a08365ffef0d26616d9208549bf2febccf`). Original complete logcat remains in Artifact `12740455266`.<br>
**APK_AND_FIXTURE_EVIDENCE:** target APK built/installed SHA `13741ec99951497b1ed4c92c57716ddb241a0c620338277fb8f12d060eb1c52a`; AndroidTest APK built/installed SHA `6dd71875482e73ab2aa4f0427c21d445bc0ee22ada1a8426a0434ccca0cff8d0`; installed hashes matched. The synthetic pinned model revision, size (101,016,128 bytes), and SHA-256 matched before the test. A separate cleanup result recorded `app_private_model_removed=false` after target uninstall; it is not the JUnit failure cause and remains a cleanup evidence issue.<br>
**FOCUSED_TESTS_ON_TESTED_SOURCE_SHA:** source-contract regression PASS; fake-ADB failure paths PASS; API marker host regression PASS; shell syntax PASS; workflow YAML parse PASS; `git diff --check` PASS; `:app:testDebugUnitTest --rerun-tasks` PASS, 3 tests / 0 failures / 0 errors / 0 skipped. Preserved JVM artifacts: `artifacts/g3-ondevice-poc/gradle-unit-after-f90b229.log`, `host-junit-after-f90b229.xml`, `host-junit-after-f90b229-summary.txt`.<br>
**NOT_RUN:** no rerun of API 35 or API 26 instrumentation, no new Emulator, no model download, no AndroidTest compile, no APK assemble (pinned llama.cpp checkout is absent in this workspace). The host JVM task compiled `:app:compileDebugKotlin`; it does not compile AndroidTest JNI or prove runtime behavior. Earlier Google Maven resolution attempts failed before AndroidTest compilation because dependencies were unreachable.<br>
**NEXT_ACTION:** review the failure classification and dispatcher source-contract guard; only after review and explicit Emulator availability may exact-head API validation be scheduled. No push has been made.

## Previous G3-owned branch repair checkpoint — API markers (2026-10-28)

This top section is the current G3-owned PR #127 status and supersedes older cancellation and workflow-marker descriptions below where they differ. No product integration is included. No Emulator, model download, API workflow dispatch, or live provider access has been performed for this repair.

**AGENT_ID:** G3 diagnostic recovery — PR #127 owner<br>
**ACTIVE_PHASE:** deterministic native abort-callback cancellation proof and API-marker exclusivity<br>
**OVERALL_STATUS:** PARTIAL — malformed/unsupported API markers, including an unclosed marker before LF or CRLF followed by a valid marker, are rejected. Host checks and JVM 3/3 passed on committed source HEAD `40a6f1e4d0c0d440299f87d91142c0fba4b611f8`, tree `3d200b814ce44bd036695a0c26cef58f14bf6698`. G1 re-review is pending. AndroidTest compile and in-compute callback runtime remain NOT_VERIFIED.<br>
**BASE_SHA:** `8d4c5cf77b4a0207ee15d36dad8ae9027722845d`<br>
**BASE_TREE_SHA:** `3f0baf105f7cace0156aebce8046eaec73661f36`<br>
**TESTED_SOURCE_SHA:** `40a6f1e4d0c0d440299f87d91142c0fba4b611f8`<br>
**TESTED_SOURCE_TREE:** `3d200b814ce44bd036695a0c26cef58f14bf6698`<br>
**CURRENT_HEAD:** this checkpoint/evidence commit follows the tested source commit above and changes only report/evidence files. Re-read the branch ref when resuming.<br>
**WORKTREE:** `/workspace/Serea-g3-diagnostic-recovery`<br>
**BRANCH:** `codex/serea-g3-diagnostic-recovery`<br>
**DRAFT_PR:** [#127](https://github.com/Yrika819/Serea/pull/127) — Open/Draft; isolated on-device PoC only, not product integration or formal Required validation<br>
**API26_RESULT:** prior exact-base run `40589226410`, artifact `12774231044`: exact one-test JUnit 1/1 PASS, 0 failures/errors/skips. This run predates the callback-gate change; its cancellation assertion used the pre-decode generation-start gate. It is not evidence for the new callback test.<br>
**API35_RESULT:** current/post-repair source NOT_RUN. Prior API35 run `40584263964`, artifact `12772451027`, passed 1/1 at older source `f0a9d92878c804aae3a2d0b5192b957186e06807`; it tested pre-decode cancellation only. No result is carried forward to this branch HEAD.<br>
**PERMISSION_EVIDENCE:** API26 exact-base artifact `12774231044` (run `40589226410`, source HEAD `8d4c5cf77b4a0207ee15d36dad8ae9027722845d`) contains `aapt dump badging` for both APKs. Target `app.serea.g3poc` lists no permissions; AndroidTest `app.serea.g3poc.test` lists `android.permission.REORDER_TASKS` only and no INTERNET. AndroidTest APK SHA-256: `ce682271a3a4d3dcbb04a4c199d7b9c0ea62bb26d2c116ada44af248b0aa5113`. The current branch adds runner-time `aapt dump permissions` evidence for both generated APKs and fails if either requests INTERNET; no required permission is removed. Post-repair local AndroidTest manifest regeneration and Kotlin compile are NOT_RUN: the offline Gradle manifest task had uncached AndroidX dependencies, and one normal compile attempt failed at `:app:checkDebugAndroidTestAarMetadata` because `dl.google.com` was unreachable. No source test code uses networking APIs; the host runner downloads the pinned model and stages it via ADB. The archived APK is exact-base evidence, not a post-repair build result.<br>
**CHANGES:** test-only native callback gate and assertion in `llama_jni.cpp`, `GgufModelChatPort.kt`, and `OnDeviceInferenceTest.kt`; mutually exclusive API selection helper/host regression and early workflow validation; both APK INTERNET guard; README/checkpoint evidence corrections.<br>
**FOCUSED_TESTS:** on tested source SHA `40a6f1e`, `python3 -B tools/g3-ondevice-poc/scripts/test-api-selection.py` PASS (valid API 26/35; both markers, supported+unsupported, unsupported-only, malformed, duplicate, missing, and unclosed-before-LF/CRLF-followed-by-valid-marker inputs rejected; dispatch 26/35 accepted); `python3 -B tools/g3-ondevice-poc/scripts/test-gradle-project-dir.py` PASS; `python3 -B tools/g3-ondevice-poc/scripts/test-runner-failure-paths.py` PASS; `bash -n tools/g3-ondevice-poc/scripts/run-diagnostic.sh` PASS; workflow YAML parse PASS; `git diff --check` PASS. Exact-source `:app:testDebugUnitTest --rerun-tasks` PASS, 3 tests / 0 failures / 0 errors / 0 skipped. Preserved evidence: `artifacts/g3-ondevice-poc/gradle-unit-after-40a6f1e.log`, `host-junit-after-40a6f1e.xml`, `host-junit-after-40a6f1e-summary.txt`. The older `host-junit.xml` and `gradle-unit-final-review.log` timestamped 2026-10-27 15:31Z are pre-repair and NOT evidence for this source. AndroidTest compile and post-repair manifest remain NOT_RUN because Google Maven dependencies were unreachable. No model or Emulator activity.<br>
**CURRENT_BLOCKER:** R3 run `40603105338` blocks a new Emulator run. In-compute abort-callback behavior is NOT_VERIFIED until an exact-head Emulator run can execute it.<br>
**NEXT_ACTION:** commit the latest host/JVM evidence checkpoint on this owned branch and request G1 re-review. Do not push until the Orchestrator accepts the repair. Run API Emulator validation only after the parent confirms R3 Emulator work is clear.
**OWNER_APPROVAL_REQUIRED:** none for this isolated repair; no real provider, credential, Pixel device, signing, or product integration used.

## Orchestrator recovery checkpoint — API 35 first runtime result (2026-10-27)

This is a separate recovery branch from the G3-owned PR #126 branch. The G3 implementation agent's turn ended at its execution usage limit; to preserve the no-overwrite boundary, Orchestrator is preparing a stacked recovery PR rather than editing that branch. No Serea product Chat or composition code is changed.

**AGENT_ID:** G3 diagnostic recovery — Orchestrator-owned branch<br>
**ACTIVE_PHASE:** repair deterministic API 35 instrumentation scheduling and bounded model cleanup<br>
**OVERALL_STATUS:** PARTIAL — run `40515091197` booted an accelerated API 35 Emulator, built and installed both APKs with matching hashes, and fetched the pinned model. Its exact one-test JUnit executed 1 test and failed at `OnDeviceInferenceTest.kt:97` because the native generation gate was not observed. No generation result is PASS. The source review identified that the test creates `async {}` children on the `runBlocking` event loop and then blocks that same thread in a CountDownLatch wait; those children cannot run until the wait returns. The recovery changes dispatch blocking-test children to `Dispatchers.IO`, preserve the assertion thresholds, and add phase timestamps. Runner cleanup now applies a 10-second bound to each cleanup ADB call, records each result, continues cleanup, checks UTP package removal, and has fake-ADB hang/cleanup tests.
**BASE_SHA:** `8ca55a32096bc4449717c2fd3332b0602381d7d5` (G3 PR #126 current head at recovery branch creation)<br>
**CURRENT_HEAD:** pending; working changes are based on `8ca55a32096bc4449717c2fd3332b0602381d7d5`<br>
**TREE_SHA:** pending<br>
**WORKTREE:** `/workspace/Serea-g3-diagnostic-recovery`<br>
**BRANCH:** `codex/serea-g3-diagnostic-recovery`<br>
**DRAFT_PR:** pending review/commit; proposed base `codex/serea-g3-real-ai-feasibility`<br>
**FAILED_RUN:** `40515091197`, source `8ca55a32096bc4449717c2fd3332b0602381d7d5`; artifact `12740455266`; artifact ZIP SHA-256 `d0a22bc992cd609d50ea1e65a2d5ef6b226c813528fd24a8aca2197a6c88b33e`.<br>
**FAILURE_EVIDENCE:** API 35 x86_64 `google/sdk_gphone64_x86_64/emu64xa:15/AE3A.240806.043/12960925`, ranchu, KVM acceleration available, boot complete. Build/AndroidTest compile PASS. Built and installed target/test SHA-256 matched (`13741ec99951497b1ed4c92c57716ddb241a0c620338277fb8f12d060eb1c52a`, `6dd71875482e73ab2aa4f0427c21d445bc0ee22ada1a8426a0434ccca0cff8d0`). Model revision `1076b85d135017971845b19b8d07df1479155988`, size 101,016,128, SHA-256 `e3131339bf4e8065265593d4fd8f7bb7ff2d3abff1edb5618aa1197b89cad9f5`, staged hash matched. JUnit exactly 1 test / 1 failure / 0 errors / 0 skips; assertion `awaitStartGateForTest(15, SECONDS)` false. Cleanup: emulator temp removed; target app was already uninstalled by the test runner, `run-as` then reported unknown package and uninstall returned `DELETE_FAILED_INTERNAL_ERROR`; private model deletion was not proven by that artifact. The AVD was disposable and terminated after the run.<br>
**TESTS:** recovery host tests now PASS: `python3 -B tools/g3-ondevice-poc/scripts/test-runner-failure-paths.py` exercises optional PSI denial, bounded hanging `adb logcat`, later cleanup continuation, primary exit preservation, host/app-private/temp model cleanup, UTP-uninstalled-package handling and cleanup-state reset. `test-gradle-project-dir.py`, `bash -n`, and `git diff --check` PASS. AndroidTest build/runtime on the recovery branch remains NOT_RUN pending hosted verification.<br>
**NEXT_ACTION:** commit/push only the recovery branch, create a Draft PR stacked on PR #126, review source and checks, and run one more marker-gated disposable API 35 test if the host regression checks and hosted APK build pass. Do not integrate this diagnostic harness into product Chat. No cloud AI, real account, API key, Pixel performance or production-route result is claimed.

### Independent recovery review update

- An independent read-only reviewer found no blocking Kotlin scheduling or cleanup correctness/security issue. It confirmed that `async(Dispatchers.IO)` allows requests to progress while the instrumentation test waits on its blocking native gate, and that per-command ADB timeouts still allow later cleanup and host-side model removal.
- The reviewer noted that the UTP-uninstall fake path had only asserted the cleanup record, not simulated removal of the target package's private model. The host regression now stages a synthetic app-private model, simulates PackageManager removing it together with the target package, and asserts the file is absent after cleanup. This closes that test-coverage note without claiming emulator verification.
- Rechecked after this change: `test-runner-failure-paths.py` PASS (including intentional hanging `adb logcat`, UTP uninstall with private-file deletion, subsequent cleanup, and original failure status preservation); `test-gradle-project-dir.py` PASS; `bash -n` PASS; workflow YAML parse PASS; `git diff --check` PASS. AndroidTest compile and runtime remain pending hosted verification.

---

## Continuation update — isolated Android JNI PoC (2026-10-27)

The initial readiness report below predates toolchain provisioning and the isolated PoC. Its no-toolchain/Qwen-candidate statements are superseded by this section; provider-terms findings remain applicable. No product source or composition changed.

**AGENT_ID:** G3 — Real AI / On-device Inference Feasibility
**ACTIVE_PHASE:** isolated llama.cpp Android feasibility PoC
**OVERALL_STATUS:** PARTIAL — isolated adapter/JNI source, ARM64/x86_64 APK build, 3/3 host contract tests, and synthetic bounded-download check pass. Run `40512775686` compiled both APKs and booted API35 Emulator, then failed before model acquisition because guest CPU pressure was inaccessible and the cleanup trap referenced function-local state. The current dirty worktree repairs both issues and adds deterministic fake-adb regression checks; no model or inference test ran.
**BASE_SHA:** `3924b36080ffa43331f065a15db80c604c243355` (PR #123 head/base ref for this stacked G3 Draft PR)
**LAST_GOOD_SOURCE_HEAD:** `4444647e4dca2538203b393cbba2261949188b2c` (last pushed, reviewed source/workflow; current worktree contains only the authorized runner-failure repair, regression check, and checkpoint update)
**LAST_GOOD_SOURCE_TREE:** `6606edc7be9bc51c38807bb43ab90f2add486c31`
**WORKTREE:** owned isolated G3 worktree (repo-relative paths in this report)
**BRANCH:** `codex/serea-g3-real-ai-feasibility`
**DRAFT_PR:** [#126](https://github.com/Yrika819/Serea/pull/126) — Open/Draft; base ref `codex/serea-personal-beta-functional-integration-identity` at `3924b36080ffa43331f065a15db80c604c243355`.
**CI_RUN_IDS:** refreshed 2026-10-27 13:38 UTC; prior PR checks on implementation HEAD `4e5ecddcb7a333d1732580fe546ee634b274138c`: Fast `40503535403` PASS; Android umbrella `40503535345` SUCCESS from scope/documentation checks only (**Android Required NOT_RUN**, not PASS); RustSec `40503535332` PASS; Git Hooks Windows `40503535196` PASS; SQLite `40503535264` PASS; Full `40503535352` PASS; Native `40503535228` PASS. G3 run `40510213740` FAILED before project configuration; `40511062977` FAILED at AndroidTest Kotlin compilation (source corrected later); `40512775686` built both APKs and booted API35 Emulator, then failed before model download due TEST_HARNESS metadata/cleanup errors. Earlier marker-free G3 push `40503475635` was SKIPPED; main-only Beta workflow `40503473453` had zero jobs on the non-main ref.
**ARTIFACT_IDS:** SQLite `12733618204` (`portable-db-arm64`) and `12734452180` (`portable-db-intel`); RustSec `12733927790`; G3 root-failure `12736014521` (6,609 bytes, SHA-256 `d5d05198156a34c6df9901dba6117f3a721910946c8dc26f190e4d882e8f7ea9`); compile-failure `12737981076` (7,469 bytes, SHA-256 `c5273bf91b361a4861c2908ab59632c3a4a2869d2dbae01228a6e3cc60b9331c`); Emulator pre-download failure `12738407819` (8,866 bytes, SHA-256 `95d79315c8ea099984639646f85b3f6c682280c0e6a0db1a3be93123b0abb9c4`).

### G3 diagnostic run 40510213740 — failure classification

- **Classification:** WORKFLOW/HARNESS. The pinned `gradlew` executable was called by absolute path while the process remained at the repository root. That directory has no Gradle settings file; the standalone project root is `tools/g3-ondevice-poc/`.
- **Evidence:** artifact `12736014521` contains `gradle-build.log`, which reports that `/home/runner/work/Serea/Serea` does not contain a Gradle build; `model-status.txt` records exactly `model_downloaded=false`. KVM/Emulator acceleration and the synthetic bounded-transfer preflight passed. APK build did not begin, the Emulator step was skipped, and there is no JUnit report or instrumentation result.
- **Correction and follow-up result:** both build and instrumentation Gradle calls now go through `run_poc_gradle`, which changes directory to the standalone project before invoking `./gradlew`. In run `40511062977`, the target app APK built and `:app:compileDebugAndroidTestKotlin` failed on unresolved `InstrumentationRegistry.getTargetContext` (line 44), downstream file/type inference errors (lines 46–50), Long-to-Int assignments (lines 75, 215, 218), and unresolved `absolutePath`/`delete` (lines 284/294) in `OnDeviceInferenceTest.kt`. Artifact `12737981076` records `model_downloaded=false`; the Emulator step was skipped. Commit `e7469248b5e96606485f70dde86ceb0c604335c7` replaces the obsolete accessor with `InstrumentationRegistry.getInstrumentation().targetContext` and declares PSS counters as `Long`. The local compile attempt was blocked before Kotlin source compilation because Google Maven was unreachable; the corrected test source was subsequently compiled on the hosted runner in run `40512775686`. No instrumentation/JUnit test ran in that follow-up.

### G3 diagnostic run 40512775686 — pre-download harness failure

- **Classification:** TEST_HARNESS. The app and AndroidTest APKs compiled successfully. The disposable API35 `google_apis/x86_64` Emulator booted (`ranchu`, fingerprint `google/sdk_gphone64_x86_64/emu64xa:15/AE3A.240806.043/12960925`, `boot_completed=1`) and the built APK hashes were verified.
- **Failure:** reading `adb shell cat /proc/pressure/cpu` returned `Permission denied` and, under `set -e`, exited before model acquisition. The EXIT cleanup then failed with `results_dir: unbound variable` because it referenced function-local state after unwinding.
- **Evidence:** artifact `12738407819` reports `apk_build_and_androidtest_compile=PASS` and `model_downloaded=false`. There is no downloaded model, JUnit report, instrumentation result, or inference output. The Emulator runner was disposable and terminated after the job.
- **Current repair:** guest CPU pressure is now optional and records denial details. Cleanup runs from a top-level handler with initialized global state; it preserves the original exit status, removes host and app/device model copies, and records state reset. Fake-adb host tests cover both permission-denied pressure capture and EXIT cleanup after early failure.

### INDEPENDENT_REVIEW

- G1 review status: **ACCEPTED** for the isolated G3 source, test, workflow, and evidence scope.
- Accepted contract: the JNI resources use a documented process-wide single-engine lease; second-owner load is rejected; cancellation identities include engine and request IDs; only the owner can free/cancel; close serializes with queued work; tests cover cross-engine isolation, reuse after close, and queued-load-versus-close.
- Review limitation: AndroidTest did not compile in this environment, so those assertions are source-reviewed only. The queued-load test does not separately exercise a load that has already entered native model loading before close.
- Preservation action: branch and diagnostic Draft PR #126 are committed/pushed. No model download, Emulator execution, real provider, or product integration was part of this preservation step.

### COMPLETED_WORK

- Installed/used isolated toolchains: Temurin 17.0.21+9 (archive SHA-256 `8b9b611d917e3dfd8419d3566b5949226f69b052fed7e109c1368add1dfe8711`), NDK 28.2.13676358, CMake 3.31.6, Ninja 1.12.1, Gradle wrapper 8.14.6, AGP 8.12.3, Kotlin 2.4.21.
- Built llama.cpp source pinned at tag `b11927`, commit `276378515ceebee5d14bcc8536603dd5c87491a2`, using `ANDROID_PLATFORM=android-26`, `ANDROID_ABI=arm64-v8a`, Release, static libraries, no OpenMP/dynamic backends/KleidiAI/native CPU tuning. `libllama.a` SHA-256: `19f2c604e11333ca61a07655595a560c67c5be45fae0bce217b6087f4de2be8f`.
- Added standalone project `tools/g3-ondevice-poc/`, outside the normal Android Gradle settings. It contains a test-local suspend `ModelChatPort`-shaped adapter, JNI wrapper, native CPU generation path, a target manifest without permissions, host contract/hash tests, and instrumentation tests. The generated AndroidTest APK inspected in artifact `12774231044` requests `REORDER_TASKS` only; neither APK requests INTERNET. It does not touch `MainActivity`, `SereaAppShell`, product composition, normal Chat routing, or provider settings.
- Final corrected Kotlin/JNI source builds the standalone debug app APK for `arm64-v8a` and `x86_64`; the exact APK SHA-256 and native library hashes are recorded below.
- Selected a smaller fixed model candidate from official `ggml-org/SmolLM2-135M-GGUF` metadata: repo revision `1076b85d135017971845b19b8d07df1479155988`, Q4_K_M artifact size `101016128` bytes, Apache-2.0 metadata, SHA-256 `e3131339bf4e8065265593d4fd8f7bb7ff2d3abff1edb5618aa1197b89cad9f5`. The weight file was not downloaded. `tools/g3-ondevice-poc/model-fixture.json` records the immutable source/revision/hash.
- Host contract and model-hash tests verify multi-turn prompt serialization and that a mismatched digest is rejected before native loading. They use a fake engine and synthetic bytes, so they do not claim inference.
- The final native resource contract is an explicit process-wide singleton lease: one `NativeLlamaEngine` owns the process-global llama.cpp model/context/sampler; a second active owner is rejected, engine IDs namespace request IDs, and only the owner can cancel or free native state. Per-engine executors serialize load/generate/close, and queued loads recheck closure before native entry. Instrumentation deterministically covers second-engine rejection, same-numbered cross-engine cancel/free isolation, successful new-engine use after close, and a blocked queued load versus close. Historical Emulator cancellation coverage was queued-before-JNI and pre-decode at `onNativeGenerationStart`, not abort callback during computation. The new callback-gated assertion is source-only until a later exact-head Emulator run.
- Added a standalone **non-required** workflow and host runner at `.github/workflows/g3-ondevice-poc.yml` and `tools/g3-ondevice-poc/scripts/run-diagnostic.sh`. It is restricted to pushes on this owned branch whose head commit message contains `[g3-poc-run]`; marker-free `40503475635` was skipped. The workflow uses read-only permission, verifies KVM/acceleration, provisions disposable API35 `google_apis/x86_64`, runs synthetic transfer and runner regression checks, builds both APKs before model acquisition, validates the fixed model size/SHA before app-private staging, runs one exact non-skipped JUnit method, captures sanitized evidence, and cleans staged files. It does not change Required gates or upload model weights/APKs. Run `40510213740` exposed a repository-root Gradle invocation; `40511062977` exposed AndroidTest compile errors; `40512775686` compiled both APKs but exposed the optional guest PSI/cleanup failures described above.
- Static checks after the latest harness repair: `bash -n tools/g3-ondevice-poc/scripts/run-diagnostic.sh`, `test-gradle-project-dir.py`, `test-runner-failure-paths.py`, workflow YAML/order review, model/hash/JUnit/cleanup guard inspection, and `git diff --check` pass. Emulator/system-image package revisions remain resolved by SDK package name rather than fully pinned; actual versions are recorded, so bit-for-bit environment reproducibility is not claimed.

### CHANGED_FILES

- `tools/g3-ondevice-poc/` — standalone Gradle test app, JNI, instrumentation/unit tests, pinned fixture manifest, build notes.
- `.github/workflows/g3-ondevice-poc.yml` — non-required, commit-marker-gated diagnostic workflow proposal; NOT_RUN.
- `tools/g3-ondevice-poc/scripts/run-diagnostic.sh` — metadata/build/run/evidence/cleanup and bounded-transfer-test script; API35 booted on run `40512775686`, but model acquisition did not occur.
- `tools/g3-ondevice-poc/scripts/test-gradle-project-dir.py` — deterministic regression check for both Gradle call sites and AndroidTest source API/types.
- `tools/g3-ondevice-poc/scripts/test-runner-failure-paths.py` — fake-adb tests for optional guest CPU pressure and EXIT cleanup after an early failure.
- `artifacts/g3-ondevice-poc/` — bounded Gradle build, AndroidX resolution failure, and host-test logs (no model weights or secrets).
- `docs/agent-reports/g3-ai.md` — this continuation checkpoint.

### TEST_COMMANDS / TEST_RESULTS

- **PASS (native cross-compile only):** with `LLAMA_CPP_SOURCE` pointing to the pinned llama.cpp checkout and `G3_BUILD_DIR` to an isolated build directory, run from the repository root:
  `cmake -S "$LLAMA_CPP_SOURCE" -B "$G3_BUILD_DIR" -DCMAKE_TOOLCHAIN_FILE="$ANDROID_SDK_ROOT/ndk/28.2.13676358/build/cmake/android.toolchain.cmake" -DANDROID_ABI=arm64-v8a -DANDROID_PLATFORM=android-26 -DCMAKE_BUILD_TYPE=Release -DBUILD_SHARED_LIBS=OFF -DLLAMA_BUILD_TESTS=OFF -DLLAMA_BUILD_EXAMPLES=OFF -DLLAMA_BUILD_SERVER=OFF -DLLAMA_BUILD_COMMON=OFF -DGGML_NATIVE=OFF -DGGML_OPENMP=OFF -DGGML_LLAMAFILE=OFF -DGGML_VULKAN=OFF -DGGML_BACKEND_DL=OFF` then `cmake --build "$G3_BUILD_DIR" --target llama -j2`.
- **PASS (final host JVM contract rerun):** from `tools/g3-ondevice-poc`, with Temurin JDK 17.0.21+9 on `PATH` and `G3_LLAMA_CPP_SOURCE` pointing to the pinned source checkout, run `./gradlew --offline --no-daemon --console=plain -Pg3LlamaCppSource="$G3_LLAMA_CPP_SOURCE" --rerun-tasks :app:testDebugUnitTest`; JUnit XML reports **3 tests, 0 failures, 0 errors, 0 skipped**. Log: `artifacts/g3-ondevice-poc/gradle-unit-final-review.log`, SHA-256 `a76a67c75ea176d5d51dc17664f250dff0e317779485f31f77f3ba736fdd8195`. JUnit XML: `artifacts/g3-ondevice-poc/host-junit.xml`, SHA-256 `7097139bbe1cd210436ddc209e16a64ce6e4da5800c1ddc7c4dfb1b60f3183f7`.
- **PASS (final corrected Android JNI/app build):** from `tools/g3-ondevice-poc`, with Temurin JDK 17.0.21+9 on `PATH` and the same `G3_LLAMA_CPP_SOURCE`, run `./gradlew --offline --no-daemon --console=plain -Pg3LlamaCppSource="$G3_LLAMA_CPP_SOURCE" :app:assembleDebug`; both ARM64 and x86_64 JNI builds and APK assembly passed. `app-debug.apk` is 14,557,726 bytes, SHA-256 `4115f121e329bc765ebb3b92242ac6789d9f0647e1e673efba65fb3a9506f4c3`. Final log: `artifacts/g3-ondevice-poc/gradle-assemble-final-review.log`, SHA-256 `587c9122f0039e497ab61c503625ba2676992e6c7f7df044de17adc347d384e9`.
- **PASS (bounded-download synthetic test, no external fetch):** from the repository root, with `G3_EVIDENCE_DIR` pointing to `artifacts/g3-ondevice-poc` and `RUNNER_TEMP` set to a disposable temporary directory, run `bash tools/g3-ondevice-poc/scripts/run-diagnostic.sh test-download-bound`; the local 33-byte payload was rejected by `curl --max-filesize 32` with expected status 63, with at most 32 partial bytes and no model content. Final review log `artifacts/g3-ondevice-poc/download-bound-final-review.log`, SHA-256 `a09990283dfdbda853613ef3d030583179a9d37cb44de96d4605d01a0f2b4aa1`.
- **BLOCKED (local AndroidTest APK compile attempt):** from `tools/g3-ondevice-poc`, with Temurin JDK 17.0.21+9 and `G3_LLAMA_CPP_SOURCE` configured, `./gradlew --no-daemon --console=plain -Pg3LlamaCppSource="$G3_LLAMA_CPP_SOURCE" :app:assembleDebugAndroidTest` failed at `:app:checkDebugAndroidTestAarMetadata` before AndroidTest Kotlin compilation. Google Maven returned `Network is unreachable`; `androidx.annotation:annotation:1.7.0-beta01` and other AndroidX test artifacts could not resolve. This local attempt did not compile the edited AndroidTest source. Log: `artifacts/g3-ondevice-poc/gradle-androidtest-final-review.log`, SHA-256 `462971e670cdbcb185faf4e793715d1e22887820cab6f455347c3bc31e24e677`; earlier attempts are preserved in `gradle-build-failure.log` and `gradle-androidtest-network.log`. Hosted run `40512775686` later compiled the test APK; that is the compile evidence for the current source revision.
- **PASS (static checks):** `bash -n tools/g3-ondevice-poc/scripts/run-diagnostic.sh`, PyYAML workflow parse, `git diff --check`, and source whitespace scan; `actionlint` unavailable.
- **PASS (runner and AndroidTest source regression):** `python3 tools/g3-ondevice-poc/scripts/test-gradle-project-dir.py` verifies both Gradle call sites use `POC_DIR`, the current `InstrumentationRegistry` target-context API is used, the obsolete accessor is absent, and all PSS counters are `Long`.
- **PASS (fake-adb failure-path host tests):** `python3 tools/g3-ondevice-poc/scripts/test-runner-failure-paths.py` verifies denied optional PSI is recorded without aborting and EXIT cleanup preserves primary status `23`, removes synthetic host/app-private/emulator-temp model files, and resets cleanup state.
- **FAIL (corrected hosted AndroidTest compile):** run `40511062977` successfully configured the standalone Gradle project and assembled the app APK, then failed at `:app:compileDebugAndroidTestKotlin` with the source errors above. AndroidTest APK, Emulator test, and JUnit are NOT_RUN. `model-status.txt` is `model_downloaded=false`; the Emulator/model step was skipped.
- **PASS (host contract tests after AndroidTest source edits):** from `tools/g3-ondevice-poc`, run `./gradlew --offline --no-daemon --console=plain -Pg3LlamaCppSource="$G3_LLAMA_CPP_SOURCE" --rerun-tasks :app:testDebugUnitTest`; JUnit reports **3 tests, 0 failures, 0 errors, 0 skipped**.
- **BLOCKED (local AndroidTest APK compile after source edits):** from `tools/g3-ondevice-poc`, `./gradlew --no-daemon --console=plain -Pg3LlamaCppSource="$G3_LLAMA_CPP_SOURCE" :app:testDebugUnitTest :app:assembleDebugAndroidTest` stopped at `:app:checkDebugAndroidTestAarMetadata` because Google Maven was unreachable while resolving the Android Gradle plugin and AndroidX test artifacts. This did not compile the edited AndroidTest source. No bypass or dependency substitution was used.
- **PASS (hosted app and AndroidTest APK build):** run `40512775686` on source HEAD `4444647e4dca2538203b393cbba2261949188b2c` completed both `:app:assembleDebug` and `:app:assembleDebugAndroidTest`. Target APK SHA-256 `2ae5c7f2b519a75235c47b79a05f85d448a95961abfcf7ee62dd4a9cfd259a9b`; AndroidTest APK SHA-256 `89427482f3263a7359e4fad71857759b5b0a75e653024693711c331a37f41fd2`. Artifact `12738407819` records the build PASS.
- **FAIL (hosted Emulator runner before model download):** run `40512775686` booted API35 x86_64 then failed because guest `/proc/pressure/cpu` access returned `Permission denied`, followed by cleanup unbound-local error. Artifact `12738407819` (8,866 bytes, SHA-256 `95d79315c8ea099984639646f85b3f6c682280c0e6a0db1a3be93123b0abb9c4`) records `model_downloaded=false`; no JUnit or inference ran. Run host metadata recorded 4 vCPU, approximately 15 GiB memory, and host CPU PSI `some avg10=4.49` after the Emulator. The target APK requested no permissions, including no INTERNET.
- **PASS (post-failure fake-adb regression checks, current dirty worktree):** `python3 tools/g3-ondevice-poc/scripts/test-runner-failure-paths.py` verifies a denied optional guest PSI read is recorded without aborting, and an injected early EXIT failure preserves status 23 while deleting synthetic host, app-private, and Emulator-temp model fixtures and resetting cleanup globals. No model or Emulator is used by this host test.
- **GitHub validation status (snapshot above):** Android umbrella success is scope-classifier/documentation validation only. Required product build, instrumentation compile, unit/integration/security, emulator/storage, and identity jobs were skipped, so Android Required is **NOT_RUN**, not Required PASS. Fast, Full, Native, RustSec, SQLite, and Git Hooks checks completed PASS on the implementation HEAD listed above.
- G3 marker-gated run `40503475635` was skipped with no steps or artifacts; no G3 diagnostic workflow job executed. The non-main personal-beta run `40503473453` had zero jobs, as expected for the main-only workflow; it is not a signing/distribution result.
- `aapt dump permissions` on the final APK reports only `package: app.serea.g3poc` and no permissions; target app manifest requests no INTERNET permission.
- Local Emulator availability: `/dev/kvm` absent, `emulator -accel-check` reports that CPU lacks VMX/SVM; no local Emulator is available. A disposable hosted API35 Emulator booted in run `40512775686` and was terminated after the pre-download failure.
- **NOT_RUN:** model download/hash of model bytes, native model load, generation, Android instrumentation, cancellation/recovery, corrupt GGUF parse on-device, offline socket observation, emulator inference, Pixel 7a memory/latency/thermal/battery. A metadata hash is not a measurement of a downloaded weight artifact.

### Native output evidence

| ABI | Output | Size | SHA-256 | Evidence |
|---|---|---:|---|---|
| arm64-v8a | final unstripped `libserea-g3-llama.so` | 59,005,016 bytes | `beaac45f37ba3e7f9cb475879ece2cfe64ab6631418ee760bb00366cc0215f3d` | Android NDK 28.2.13676358; API 26 target; final engine-lease JNI |
| arm64-v8a | final packaged/stripped `libserea-g3-llama.so` | 6,308,144 bytes | `6e0a6fa041f69dcede352853446fc10b0d91891c1ee6a95223c6ed68e66f6309` | APK build output |
| x86_64 | final unstripped `libserea-g3-llama.so` | 57,520,360 bytes | `1a53ce1b918d0e726f61e4d5880ae579e4f928964eb2b61f13d5974ebb1688b7` | Android x86_64 Emulator ABI build |
| x86_64 | final packaged/stripped `libserea-g3-llama.so` | 6,787,224 bytes | `353a969f431af16a9d1414d587b3e4a393838074ad1c8c438c274f4cb811a6f7` | APK build output |

### SECURITY_IMPACT / COMPATIBILITY_IMPACT

- Product/provider behavior and normal Chat remain unchanged. The standalone target manifest has no permissions or exported components; archived generated APK evidence shows the AndroidTest package has `REORDER_TASKS` only, with no INTERNET. The local test source uses no networking APIs. The host runner obtains the pinned fixture and stages it locally before AndroidTest execution.
- The local adapter is structurally compatible with the `suspend send(request): response` shape, not linked to Serea's production interface. No local-model egress target or product catalog was added. It must not be integrated as `LOCAL_DEMO`.
- ARM64 API26 native compilation does not prove model load/runtime compatibility; the ABI APK can include either architecture, and x86_64 Emulator inference would not prove ARM64/Pixel 7a performance.

### CURRENT_BLOCKERS / NEXT_ACTION

- Local AndroidTest APK compilation remains **NOT_COMPILED locally** because Google Maven was unreachable before source compilation. Hosted run `40512775686` compiled the AndroidTest APK (SHA-256 `89427482f3263a7359e4fad71857759b5b0a75e653024693711c331a37f41fd2`), but its test method is **NOT_RUN** because runner setup failed before model acquisition.
- The Google Maven failure is observed only in this local environment. Hosted reproducibility is **UNKNOWN / NOT_RUN** because Android run `40503535345` classified this tool-only PR and skipped the build/instrumentation-compile job; hosted dependency resolution never ran. No dependency bypass or test skip was added.
- Model weights remain undownloaded. A disposable API35 Emulator booted in run `40512775686`, but native model load, single/multi-turn generation, cancellation, release/recovery, corrupt GGUF parsing, offline socket verification, API 26 runtime, and Pixel 7a memory/latency/thermal/battery are **NOT_RUN**.
- G1 accepted the source/workflow scope; its review notes the queued-load-vs-close test does not separately exercise a load already inside native model loading. The current runner repair is limited to optional guest PSI capture, top-level cleanup trap state, and deterministic fake-adb regression checks. A single follow-up marker run is authorized after these focused checks; model download remains gated behind app and AndroidTest APK compilation.
- Real cloud AI/provider integration remains disabled. Provider-specific terms are documented below; Owner account, audience age, launch region, budget, credential custody, and real provider authorization remain unverified/Owner-blocked.
- Next: after static/fake-adb checks pass and current G4 Emulator activity is clear, commit the reviewed runner repair with the `[g3-poc-run]` marker and run exactly one disposable API35 workflow. Keep the compile-before-model boundary. If the next execution advances, verify the fixed model hash, exact one-test JUnit, sanitized metrics, and cleanup. No live API/account or Pixel 7a performance claim is involved.

Retrieved official provider/runtime pages: **2026-10-27 UTC**. This is a readiness review, not a legal opinion or a provider activation. No live account, API key, paid request, Google account, or model weight was used.

## Initial readiness inventory (historical; superseded where the continuation above differs)

The remaining sections preserve the initial read-only policy and integration research at source base `3924b36080ffa43331f065a15db80c604c243355`. Their toolchain inventory, Qwen candidate, unbuilt-project and no-test statements describe that earlier point in time. They are not the current G3 implementation or verification status. Current evidence is in the continuation above: NDK/CMake/Ninja, JNI/APK build, and 3/3 host tests now exist; the SmolLM2 fixture is pinned but not downloaded; AndroidTest, native inference, Emulator and Pixel 7a remain NOT_RUN. Provider policy observations remain useful, with Owner-specific eligibility still unverified.

### Historical agent-report snapshot

**AGENT_ID:** G3 — Real AI / On-device Inference Feasibility<br>
**ACTIVE_PHASE:** G3 research / feasibility<br>
**OVERALL_STATUS:** PARTIAL — initial official-source review and integration assessment completed; at that historical snapshot no native build tooling or standalone inference harness existed, and a 639 MB Qwen fixture was intentionally not downloaded. This status is superseded by the continuation at the top of this report.<br>
**HISTORICAL_BASE_SHA:** `3924b36080ffa43331f065a15db80c604c243355`<br>
**HISTORICAL_HEAD:** `3924b36080ffa43331f065a15db80c604c243355` (source revision inspected at initial research snapshot)<br>
**HISTORICAL_TREE_SHA:** `41cfce316c485abe31f86c827c6ca81a4d5c107b` (source tree inspected at initial research snapshot)<br>
**WORKTREE:** owned isolated G3 worktree (historical snapshot; host path omitted)<br>
**BRANCH:** `codex/serea-g3-real-ai-feasibility`<br>
**HISTORICAL_DRAFT_PR:** None at initial research snapshot<br>

#### Historical COMPLETED_WORK

- Inspected existing Chat contracts and composition on PR #123. `ModelChatPort.send(ModelChatRequest)` is suspend-based and returns a complete `ModelChatResponse`; `MockModelChatPort` is explicitly deterministic demo behavior.
- Existing normal Chat composition is local demo only: `RoutedModelChatPort → EgressCheckedModelChatPort (LOCAL_DEMO) → UsageControlledModelChatPort → LocalDemoModelChatPort → MockModelChatPort`.
- Existing cloud route has a fixed Ollama endpoint and requires a fresh, verified model catalog entry, exact per-send egress approval, a user-authored PUBLIC payload, credentials, and enabled settings. `CloudSettings.enabled` defaults to `false`. No provider call was made.
- Inspected the existing accepted ADR and AI activation plan. They already say that no real provider is enabled, and list Owner decisions required before activation.
- Compared the existing Ollama Cloud candidate, OpenAI API, Gemini API, ML Kit GenAI/AICore, and an isolated `llama.cpp` + official Qwen GGUF route against current official terms, age/region conditions, data handling, pricing model, hardware support, and code integration constraints.
- Determined why a real local inference PoC could not be validated in this worktree: the then-configured Android SDK had no NDK package; `cmake` and `ninja` were not installed/in PATH; no inference model was present. The official Qwen Q8 GGUF model file is **639,446,688 bytes**. It was not downloaded because this host lacked the native toolchain and a download alone cannot prove Android inference, cancellation, memory use, or Pixel 7a performance.
- No source code, dependency, build configuration, provider setting, or production composition was changed.

#### Historical CHANGED_FILES

- `docs/agent-reports/g3-ai.md` (this report only; no runtime or product changes).

#### Historical TEST_COMMANDS / TEST_RESULTS

- Source inspection: `rg -n 'ModelChatPort|CloudSettings|cloud.*enabled|RoutedModelChatPort|DemoChat' ...` and targeted `sed` reads — completed; findings above.
- Toolchain inventory: `command -v cmake`, `command -v ninja`, and inspection of the then-configured Android SDK's NDK packages — at that historical snapshot no CMake, Ninja, or NDK was available.
- Device inventory: `adb devices -l` — daemon started; no devices attached.
- Official model metadata: queried Hugging Face model metadata API for `Qwen/Qwen3-0.6B-GGUF` — `license=apache-2.0`; sole `.gguf` entry observed `Qwen3-0.6B-Q8_0.gguf`, size `639446688` bytes.
- No build or tests were run: no PoC implementation was added. There is no inference result, emulator result, ARM64 execution result, or Pixel 7a benchmark to report.

**HISTORICAL_CI_RUN_IDS:** None at initial research snapshot<br>
**HISTORICAL_ARTIFACT_IDS:** None at initial research snapshot

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

#### Historical AI route comparison (local Qwen candidate superseded by SmolLM2 fixture above)

| Route | Eligibility / terms | Execution / device and memory | License / model provenance | Integration / security | Pixel 7a unknowns | Decision |
|---|---|---|---|---|---|---|
| **Ollama Cloud** (existing fixed adapter) | Ollama Terms require a user of the Service to be 18+. Owner has not confirmed audience age, account eligibility, or release/user region; do not infer adults-only from “Personal Beta.” | Cloud inference; model memory is provider-side. Official privacy/pricing says hosting is primarily US and may route to Europe. Pay-per-token model pricing and plans can change. App/device inference latency is not measured. | Hosted model weights are not shipped by Serea. Exact model ID and the current hosted model provenance/licensing still require activation-time review; the frozen ADR roster may be stale relative to current catalog/prices. | Adapter exists with default disabled and exact egress gate. Cloud prompt/response processing is described as transient and not trained on; cost is token-based. Cross-border egress and provider account custody remain explicit risks. No network request occurred. | No device-local performance/memory claim. Pixel 7a network quality, end-to-end latency, battery, or data costs are untested. | **Conditional candidate only.** Do not enable pending audience/age and region determination, current model/pricing review, data-flow approval, account ownership, and budget. |
| **OpenAI API** (direct) | Services Agreement disallows minors using service without parent/guardian consent. Under-18 API guidance requires reasonable age-appropriate safety for organizations serving minors and says not to process personal data of children below 13 (or applicable digital-consent age) without approved Zero Data Retention. Access countries are maintained in an official list; Owner account and user/deployment countries are unverified. | Cloud inference; model memory is provider-side. Token-priced per model; rate limits/pricing change and must be rechecked before approval. No API call made, so no measured latency. | Hosted model weights are proprietary/provider-hosted; no weight redistribution. Select an exact currently supported model only after official source, data, and price review. | No OpenAI adapter exists in PR #123. API data is not used to train by default, but abuse logs may include content and default retention is up to 30 days; ZDR/MAM require prior approval and have limitations. Never embed a direct API key in the APK; key custody/backend requires a separate decision. Preserve Serea egress/usage ordering. | Network latency, availability, battery, carrier cost, and any API region/data-residency behavior from Pixel 7a are untested. | **Potential candidate only** after audience, region, safety, retention, model, billing, and credential-custody decisions. Not connected/authorized. |
| **Gemini API / AI Studio** | Additional Terms (effective 2026-03-23): developer must be 18+, API clients must not be directed to or likely accessed by under-18s, and service is for professional/business development, not consumer use. Owner account country and audience remain unverified. | Cloud inference; model memory is provider-side. Only listed regions allowed; EEA/UK/Switzerland clients must use paid services. On unpaid service, prompts/responses may be used to improve products and human reviewed; do not send sensitive/confidential/personal data. Paid access costs and requires explicit approval. | Hosted weights cannot be extracted or redistributed under the service terms. Exact model is not selected. | No adapter wired. Current no-unapproved-egress rule blocks usage. Google free-tier data use is incompatible with sending personal assistant content; paid terms do not itself authorize Serea's egress. | Pixel 7a bandwidth, latency, battery and supported-country service access not tested. | **Not suitable for this consumer Personal Beta under current terms.** No workaround/activation. |
| **ML Kit GenAI / Gemini Nano (AICore)** | ML Kit GenAI Additional Terms require developer age 18+ and prohibit apps directed to or likely accessed by under-18s. Prompt API is marked Beta; terms prohibit Preview/Experimental use in production. | On-device inference, with AICore managing model distribution and execution; memory/limits are model/device managed and no Pixel 7a budget is published here. Some feature-specific APIs only perform narrow tasks; Prompt API is Beta. | Google-managed model weights; no Serea model artifact or weight distribution. AICore/ML Kit terms and attribution/data disclosure obligations apply. | SDK reports input/output are processed on-device, but it may send API performance/utilization metrics to Google; user disclosure may be required. Uses AICore and has per-app quotas / foreground-use restrictions in official docs. | Pixel 7a is **not** on the current official Prompt or feature-specific supported-device lists. No runtime `checkStatus`, AICore availability, memory, latency, or API 26/35 test occurred. | **Not a valid Pixel 7a target-device route today.** Support, production status, and age conditions remain disqualifying/unresolved. |
| **Historical `llama.cpp` + Qwen3-0.6B GGUF candidate (superseded)** | The earlier review found no service-account age/region gate for local inference; model terms still apply. The runtime is MIT and Qwen's model card reports Apache-2.0. | Earlier candidate was the 639,446,688-byte Q8 file. It was not downloaded or measured. | Historical metadata only; no file hash/runtime audit. Replaced as the PoC fixture by the smaller pinned SmolLM2 candidate described above. | Product integration remains absent. A future local target needs a distinct reviewed egress/model identity, provenance, integrity verification, and cancellation/memory review. | Pixel 7a performance remains unmeasured. | **Superseded candidate; not the current PoC fixture.** |

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

### Historical KNOWN_ISSUES (initial inventory only; see current status above)

- No approved/connected real AI provider exists in the inspected app composition. The ordinary Chat screen remains a deterministic local demo, not real AI.
- Cloud routes remain Owner-blocked for eligibility, age/audience, location, price/budget, credential custody, and data-transfer decisions. Provider-level terms have been documented separately from Owner facts.
- Gemini Nano/AICore does not list Pixel 7a as supported in the current official device list. The owner’s physical Pixel 7a is unavailable; no device query or performance test occurred.
- At the time of this initial inventory there was no native build/harness. That statement is superseded: an isolated SmolLM2/llama.cpp harness and ARM64/x86_64 app APK build now exist. Actual model load/generation, cancellation, recovery, corrupt-model parsing, offline behavior, Emulator inference, and Pixel 7a performance remain **NOT_RUN** as reported above.
- The current local egress identity is explicitly demo-only. A real local inference route needs a new separately reviewed local target/catalog/provenance contract and must not be made to appear as cloud AI or demo output.
- The PoC JNI model/context/sampler remain process-global behind the documented single-owner lease; support for parallel model instances is intentionally not provided. Instrumentation has not yet compiled, so cross-engine and load/close tests are source-level expectations, not verified passing results. PSS before/load/after-generation/after-close is logged as an Emulator diagnostic metric only; it is not asserted as proof of memory reclamation.
- Pinned library revision, transitive license inventory, SBOM, model SHA verification, model download/update plan, resource constraints, and safety evaluation remain open.

### Historical CURRENT_BLOCKERS (initial toolchain/model inventory; superseded for build/harness status)

- At the time of the initial inventory, NDK/CMake/Ninja were absent, and the Qwen Q8 model candidate was 639,446,688 bytes. This toolchain/candidate state is historical; the current pinned NDK build and SmolLM2 fixture are documented above. Model bytes remain intentionally undownloaded.
- No Android ARM64 device was attached in the initial inventory; the Pixel 7a remains unavailable for this campaign.
- Owner facts: intended user age/audience, provider account eligibility, launch regions, usage budget, model allowlist, and acceptance of each provider’s data terms are unknown. These do not block isolated Fake/offline development but block real service enablement.

### INTEGRATION_DEPENDENCIES

- G4 must not replace ordinary Chat with any real route on the basis of this research.
- A cloud integration requires Owner approval of a specific provider/model, intended user age and supported countries, data terms/retention, cost ceiling, and operator credential custody. It must retain Egress → Usage reservation → dispatch ordering and keep Google-origin data blocked by default.
- A local model integration requires a dedicated `LOCAL_ON_DEVICE` (or equally explicit) contract separate from `LOCAL_DEMO`, immutable runtime/model provenance, SHA verification, asset/update policy, Android API 26 compatibility evidence, and a model-sized device benchmark. The new project is test-only source; it is not an inference result or product integration.
- No modifications are requested to `MainActivity.kt`, `SereaAppShell.kt`, or composition root.

### Historical REVIEW_REQUEST (initial policy research review)

Please review the provider eligibility distinctions, especially the split between provider-level age/region rules and Owner-unverified account/audience facts. Confirm that the local route remains explicitly feasibility-only and that G4 does not treat this report as a PoC or real AI E2E.

### Historical recommended next engineering path (superseded by continuation above)

1. Continue product-independent Fake/offline Chat and test work; keep `CloudSettings.enabled=false` and do not bind a real API key/provider.
2. The initial recommendation to provision NDK/CMake/Ninja and a local PoC has been completed for the isolated SmolLM2 fixture harness; model acquisition and runtime are still blocked pending AndroidTest compilation and a coordinated KVM Emulator run.
3. The Qwen-specific fixture proposal in this initial plan is obsolete. The current fixture is the pinned 101,016,128-byte Apache-2.0 SmolLM2 Q4_K_M candidate above; its model bytes remain undownloaded.
4. The test-only adapter/JNI and host contract have been implemented. Instrumentation compile/runtime, real model load/generation, cancellation, native release, corrupt-model parsing, and offline behavior remain NOT_COMPILED/NOT_RUN as stated in the current checkpoint.
5. Collect separate ARM64-emulator and Pixel 7a measurements for peak RAM, startup, tokens/sec, thermal behavior, storage, cancellation latency, and process-death recovery. Only Pixel 7a data can answer Pixel 7a performance.
6. Independently audit the pinned runtime/native dependency licenses/SBOM and add a distinct reviewed `LOCAL_ON_DEVICE` egress/catalog identity before any product-composition work.
7. Revisit Cloud candidates only after Owner records exact target audience age, launch regions, account qualification, model allowlist, data terms/retention, spend cap, credential custody, and any guardian/safety requirements. Provider-term eligibility does not establish the Owner's account or users' eligibility.

### Historical no-test / NOT_RUN ledger (initial snapshot only; current status is above)

These are research observations, not passing tests:

| Check | State |
|---|---|
| Source/contract inspection | DONE (read-only) |
| Official policy/runtime/model metadata review | DONE (read-only, sources above) |
| Project build / compile | NOT_RUN at the initial `3924b360...` inventory; superseded by current PASS for the standalone Android app/JNI APK and host JVM contract tests |
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

### Historical NEXT_ACTION (replaced by current blockers and next action above)

The initial Qwen/no-toolchain action plan is superseded by the current continuation. Current action is to restore AndroidX dependency access, compile the instrumentation APK, and only then coordinate one disposable API35 KVM Emulator diagnostic; keep model weights, real providers, and Pixel 7a usage blocked until those conditions and Owner approvals are met.
