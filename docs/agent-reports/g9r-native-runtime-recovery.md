# G9R Native Runtime Recovery — Agent A

**Checkpoint:** 2026-10-29 UTC
**Status:** PARTIAL; runner root cause and native inference remain NOT_RUN pending a new hosted diagnostic run.
**Base:** PR #147 HEAD `36700f6c99d15db428c6d434c657ac76593df3f9`, tree `107824f5338ed9ec46d41f6836f5df9c6ad84cf7`.
**Worktree:** `/workspace/serea-g9r-native-recovery`.
**Branch:** `codex/serea-g9r-native-recovery`.

## Baseline evidence

PR #147 focused run `40843204437`, attempt 1, is bound to the base HEAD/tree above. It passed the x86_64 and ARM64 native APK builds, model-fixture size/hash checks, and disposable API35 x86_64 KVM setup. The instrumentation job started one test identity and failed before the test body with:

```text
app.serea.android.LocalModelNativeRuntimeE2ETest > initializationError
java.lang.RuntimeException: Failed to instantiate test runner class androidx.test.internal.runner.junit4.AndroidJUnit4ClassRunner
```

The Actions log contains no `Caused by` chain or logcat. Diagnostic artifact `12883594935` is 5,881 bytes with SHA-256 `e4ed10a1ec5d14a38bbf2e861b30b196d7e3b8e0e325dc1869df3c3c63760cb1`; artifact retrieval returned HTTP 403. The root cause is **UNCONFIRMED**. The test source uses `@RunWith(AndroidJUnit4::class)`; the isolated manifest/Gradle configuration selects `SereaIsolatedAndroidJUnitRunner`; locked AndroidX test versions are ext-junit 1.2.1, runner/core 1.6.1. No version or manifest defect has been established from these facts.

The failed connected-test run ended with both app packages absent while the fixture had been staged under the target app's private cache. Existing cleanup correctly refused to claim app-private deletion without a package. The new diagnostic invokes the installed instrumentation component directly so the script remains responsible for installed APK ownership, fixture removal, and evidence retention.

## Changes in this candidate

- Added `SereaNativeRunnerNoOpTest`, using the same `AndroidJUnit4` runner and asserting the target/test package identities. Both it and the existing native E2E identity are listed only in `serea-non-required-test-identities.txt`.
- Kept the 103 Required and 2 Additional manifests unchanged. Static identity validation now reports Required 103, Additional 2, non-required diagnostics 17, source total 122, with no overlap or unlisted source identity.
- Changed the focused native script to run the no-op and native E2E test in one explicit `am instrument` request, capture the complete runner status output and bounded logcat diagnostics, and preserve a JUnit XML derived from the raw runner status stream.
- Bound the instrumentation command to 30 minutes. On timeout, it force-stops only the two isolated packages installed by the current run before verified cleanup.
- Added focused parser cases for exact pass identities, initialization-error cause retention, skip/failure accounting, no testcase status, and unexpected identities.

The new invocation is a non-Required diagnostic, not Android API26/35 Required evidence. The report generated from instrumentation status is labeled as raw instrumentation evidence and is not presented as Gradle's JUnit artifact.

## Focused checks

| Command | Result |
|---|---|
| `python3 tools/g8-local-model-native/test-instrumentation-stream.py` | PASS, 6 tests |
| `bash tools/g8-local-model-native/test-preflight.sh` | PASS; rejects physical/multiple devices, reused namespaces, malformed package query, and unsafe evidence override |
| `bash tools/g8-local-model-native/test-cleanup.sh` | PASS; cleanup remains fail-closed and preserves primary failures |
| `python3 tools/g8-local-model-native/test-jni-contract.py` | PASS; 8 exported JNI boundaries and failure seam checked |
| `bash -n tools/g8-local-model-native/run-e2e.sh` | PASS |
| `git diff --check` | PASS |
| AndroidTest compile / Emulator inference | NOT_RUN; local environment has no `javac`, emulator binary, ADB device, or `/dev/kvm` |

## Gates and next action

- `X86_64_BUILD`: PASS on baseline run `40843204437`; no new-source build evidence yet.
- `ARM64_BUILD`: PASS on baseline run `40843204437`; no ARM64 runtime evidence.
- `X86_64_RUNTIME`: NOT_RUN; baseline stopped before the native test body.
- `ARM64_RUNTIME`: NOT_RUN.
- `PIXEL7A_PHYSICAL`: NOT_RUN and outside this campaign's authorization.
- Model fixture identity remains SmolLM2 GGUF revision `1076b85d135017971845b19b8d07df1479155988`, 101,016,128 bytes, SHA-256 `e3131339bf4e8065265593d4fd8f7bb7ff2d3abff1edb5618aa1197b89cad9f5`; llama.cpp remains pinned to `276378515ceebee5d14bcc8536603dd5c87491a2`. The model is removed from host/device after the run and never uploaded.
- No model default is selected. LFM3-1.7B, Bonsai 2, Qwen3.5 2B, Gemma 4 E2B, and FunctionGemma 270M remain metadata/license/compatibility research only; no additional weights were downloaded.

**Next action:** obtain independent Agent C review of this patch; push as a Draft PR based on the exact G7/PR147 base; inspect the next source-bound run for no-op outcome, exact initialization exception chain, JUnit identity/counts, cleanup proof, APK/runtime identity, and two-turn inference. Do not call Native Runtime accepted unless the same candidate run reaches inference, usage/egress, resource release, and verified cleanup.
