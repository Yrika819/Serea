# G5 Agent C — On-device AI Validation Checkpoint

Report date: 2026-10-28 UTC

## Agent checkpoint

```text
AGENT_ID: C — On-device AI Validation
PHASE: G5 C1–C6
STATUS: SUCCESS for exact-head API 35 Emulator PoC validation; product integration NOT_RUN
BASE_SHA: PR #127 base / PR #126 head 8ca55a32096bc4449717c2fd3332b0602381d7d5
PR126_BASE_SHA: frozen PR #123 3924b36080ffa43331f065a15db80c604c243355
VERIFIED_SOURCE_HEAD: a2bafeea77160e8e2746bdfdd0d309348ac04aa7
VERIFIED_SOURCE_TREE: 25874a7f3fc53028ae72aa7ddf4cc42a5e823f70
REPORT_BRANCH: codex/serea-g5-ondevice-ai-report-20261028
REPORT_WORKTREE: /workspace/Serea-g5-ondevice-ai-report
DRAFT_PR: none; PR #126/#127 unchanged
CHANGED_FILES: docs/agent-reports/g5-ondevice-ai.md (checkpoint only)
OWNER_ACTION_REQUIRED: Pixel 7a measurement, product Chat integration approval, real AI eligibility/credentials/billing, production signing/distribution, main merge and Beta GO
```

The tested source branch was `codex/serea-g3-diagnostic-recovery`, PR #127 Open/Draft. Its remote and local HEAD were verified at `a2bafeea...`; this report branch is a separate docs-only child. No source, workflow, PR #126, or PR #127 changes were made.

## Exact-head API 35 run

- Workflow: `.github/workflows/g3-ondevice-poc.yml`, non-Required `workflow_dispatch`, API level `35`.
- Dispatch command, from `/workspace/Serea-g3-diagnostic-recovery`:

  ```text
  gh workflow run .github/workflows/g3-ondevice-poc.yml --ref codex/serea-g3-diagnostic-recovery -f api_level=35
  ```

- Run `40657508541`, exact source HEAD `a2bafeea...`, completed `SUCCESS`; job `122712028734` passed.
- Artifact `12802681672`, `serea-g3-ondevice-poc-api35-a2bafeea77160e8e2746bdfdd0d309348ac04aa7`; GitHub digest and downloaded ZIP SHA-256: `df06078f5c8b4164bd24e255d1df0e2df9069570e08c08eea1295f4f391161f8`.
- Extracted evidence: `/tmp/serea-g3-api35-40657508541/unpacked`.
- JUnit: 1 test, 0 failures, 0 errors, 0 skips; duration 19.179 s.
- Test ID: `app.serea.g3poc.OnDeviceInferenceTest#pinnedModelLoadsGeneratesCancelsReleasesAndRejectsCorruptFile`.

The test passed assertions for model/hash validation, Unicode JNI handling, single- and multi-turn nonblank output, one-engine ownership and foreign cancel/free isolation, cancellation before JNI entry, cancellation observed inside llama.cpp's abort callback, post-cancel recovery, close/reopen generation, close-vs-load race rejection, closed-port rejection, and corrupt GGUF rejection. Log markers include `cancellation_abort_callback_gate_reached=true callback_entries=1 starts=4`.

Build command evidenced by `gradle-build.log`:

```text
./gradlew --no-daemon --console=plain -Pg3LlamaCppSource=<pinned-llama-source> :app:assembleDebug :app:assembleDebugAndroidTest
```

Instrumentation command executed by the workflow script (model path is an app-private runtime path):

```text
./gradlew --no-daemon --console=plain -Pg3LlamaCppSource=<pinned-llama-source> -Pandroid.testInstrumentationRunnerArguments.modelPath=<app-private-model-path> -Pandroid.testInstrumentationRunnerArguments.modelSha256=e3131339bf4e8065265593d4fd8f7bb7ff2d3abff1edb5618aa1197b89cad9f5 -Pandroid.testInstrumentationRunnerArguments.class=app.serea.g3poc.OnDeviceInferenceTest#pinnedModelLoadsGeneratesCancelsReleasesAndRejectsCorruptFile :app:connectedDebugAndroidTest
```

The API 35 build log records CMake Release configure/build tasks for both `arm64-v8a` and `x86_64`. This proves ARM64 compilation on this exact source; runtime was only x86_64.

## Model and APK provenance

- llama.cpp source: `276378515ceebee5d14bcc8536603dd5c87491a2`.
- Model repository/revision: `ggml-org/SmolLM2-135M-GGUF` at `1076b85d135017971845b19b8d07df1479155988`.
- Pinned metadata declares base `HuggingFaceTB/SmolLM2-135M` and license `apache-2.0`. `model-license-evidence.json` distinguishes metadata claims from fixture expectations; metadata response SHA-256 is `0706cb4f7d4f54abf49b620da9863532cdacbba289ba403a37533c8e43eebb1f`.
- Actual downloaded/staged model: 101,016,128 bytes, SHA-256 `e3131339bf4e8065265593d4fd8f7bb7ff2d3abff1edb5618aa1197b89cad9f5`; app-private staged hash matched.
- Target APK SHA-256 `60613ff71c82cab737137fdd9d201628c34bd701f356e841656595f989eb5061`; AndroidTest APK SHA-256 `82e6327146499b18ad74d2e55f40411b9d339ed47149161b1ba19c15e01b6930`. Installed target and test APK hashes exactly matched.
- Target APK has no requested permissions; AndroidTest requests only `android.permission.REORDER_TASKS`. Neither package requests INTERNET. The host fetches and verifies the model before staging it with `adb`.
- Model weights are absent from the repository and artifact. The artifact is 2.31 MB and contains sanitized logs/metadata only.

## API 35 measurements and environment

Emulator: Google APIs x86_64, API 35 / Android 15, `ranchu`, boot completed, fingerprint `google/sdk_gphone64_x86_64/emu64xa:15/AE3A.240806.043/12960925`. Runner: Ubuntu 24.04.5, x86_64, 4 vCPU, 15 GiB host memory, AMD EPYC 7763 under Microsoft hypervisor. Emulator 37.2.12; NDK 28.2.13676358; CMake 3.31.6; build-tools 35.0.0; API 35 platform revision 2; Google APIs x86_64 image revision 9. After KVM permissions were applied, `/dev/kvm` was accessible and `emulator -accel-check` reported KVM v12 usable.

Observed test metrics from `G3_ONDEVICE_METRICS`:

| Metric | API 35 x86_64 Emulator |
|---|---:|
| Model load | 928 ms |
| First turn | 3,242 ms |
| Multi-turn | 2,166 ms |
| Cancel to native return | 1 ms |
| Recovery generation | 1,175 ms |
| PSS before / after load / after generation / after close | 35,405 / 162,309 / 166,110 / 54,166 KiB |

PSS is observational, not proof of memory returned to the OS; after-close PSS remained above baseline. Guest CPU PSI was unavailable (`/proc/pressure/cpu` missing); host `some avg10` after the emulator was 22.82. Neither condition caused a test failure. Do not compare these x86_64 Emulator timings to Pixel performance.

Cleanup evidence: AVD was force-created with wipe-data and not reused; temporary `/data/local/tmp` model was removed and verified. The instrumentation runner had uninstalled the target, so app-private removal was explicitly `not_individually_verified`; the disposable AVD and ephemeral runner VM were then discarded. No model weights were uploaded.

## API 26 historical result — different source

- Run `40589226410`, artifact `12774231044`, artifact ZIP SHA-256 `10e4c4768e8c7542a8bcde7a6975ec10459e4ef2dfd58da4fc79ab0fe22555f7`.
- Source was `8d4c5cf77b4a0207ee15d36dad8ae9027722845d`, **not** the current `a2bafee...` HEAD.
- Same test ID passed 1/1, failures/errors/skips 0/0/0 on API 26 x86_64 Google APIs Emulator with KVM.
- Metrics: load 233 ms; first turn 337 ms; multi-turn 593 ms; cancel 1 ms; recovery 372 ms; PSS 21,265 / 147,914 / 151,325 / 54,281 KiB before/load/generation/close.
- This older revision predates the current in-decode abort-callback cancellation test. API 26 exact-current-source execution remains NOT_RUN.

## Limits and disposition

- The PoC lives in `tools/g3-ondevice-poc` and uses its own `GgufModelChatPort`. It does not import the production Chat port or enter Serea's Router, Egress, or Usage chain. No product Chat, Google data, Cloud AI, real API key, or Google account was used.
- Output quality was not evaluated; the test asserts nonblank generation. Long-session quality/stability and conversation persistence were not measured.
- Pixel 7a runtime throughput, memory, thermal load, battery, and storage impact are NOT_RUN. ARM64 runtime is also NOT_RUN despite the successful ARM64 build.
- The exact-head Android repository workflow `40655516273` was change-scoped to documentation: Android Unit, Build/Lint, Storage/Emulator, Security, Integration, Identity and Pixel bundle jobs were skipped. Do not treat that run as host/JVM evidence. Host contract tests on the exact latest HEAD are NOT_RUN.
- No product defect is established. Keep the PoC independent; product AI remains NOT_RUN and is not integration-ready. Any future integration requires a reviewed production adapter preserving Router/Egress/Usage, model selection, cancellation, memory lifecycle, and data-origin contracts.
- Owner actions remain Pixel 7a access for device performance, real provider eligibility/credentials/billing, product integration approval, production signing/distribution, main merge, and final Beta GO.

NEXT_ACTION: retain this as a standalone feasibility PoC; parent Orchestrator records Run `40657508541` and artifact `12802681672` in the G5 integration ledger. Do not dispatch another Emulator run in this lane without coordination.
