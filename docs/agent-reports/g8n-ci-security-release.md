# G8N CI, Security, and Release Evidence

**Agent:** C — CI / Security / Release
**Captured:** Initial capture 2026-10-29 02:49 UTC; Actions refresh 02:57 UTC
**Status:** G8 Formal CI is **PARTIAL**. Android Required is **FAILURE** on the emulator harness; Full CI, Native, and SQLite have assigned in-progress jobs and remaining unassigned macOS jobs. Release/Owner decisions are prepared, not approved.
**Worktree:** `/workspace/Serea-C`
**Branch:** `codex/serea-g8n-security-release`
**Base SHA / Tree:** `0560e1e7bcd13f0194eab8caca89d70ea5a2f142` / `ea12eec70afb172e610fbae4c8cea95c5061aaa7`

This is a timestamped evidence snapshot. It does not treat queued or in-progress jobs as passing. PR #146 and PR #144 were inspected read-only and were not changed.

## Authority and exact source identity

| Reference | State at capture | Source SHA | Tree SHA |
|---|---|---|---|
| Protected `main` | unchanged | `e0acb0254820c381dd97828824bef86a1db5176c` | `4955555be75c5742796740a3283d1a521e0d4806` |
| PR #146 integration candidate | OPEN, Draft | `d444e7cc04c8f1c4c6d12b1412cfe3a76258af66` | `25e2bb481a0b25bfed89d6ee25f1812316703b8e` |
| PR #144 native candidate | OPEN, Draft | `07ea5c6b460cea2eb1ccccc49ccd4f74dc65bb59` | `ff721fc7ae7642cbaf226376c23df6541d7c8248` |
| PR #146 base / G7 | current base of #146 | `0560e1e7bcd13f0194eab8caca89d70ea5a2f142` | `ea12eec70afb172e610fbae4c8cea95c5061aaa7` |

The observed branch head for PR #146 was `codex/serea-g8-integration-candidate-20261029-r1`, based on `codex/serea-g7-calendar-readiness`. The PR #144 head was `codex/serea-g8-native-local-runtime`. Run attempts below are all `1` unless called out otherwise.

## PR #146 formal CI snapshot

All listed runs point to `d444e7cc04c8f1c4c6d12b1412cfe3a76258af66`. Runner names and jobs are from the Actions API. Statuses reflect the snapshot at 02:44 UTC; do not reuse this evidence for another SHA.

| Run | Workflow | Jobs/results at capture | Artifacts |
|---|---|---|---|
| [40804694025](https://github.com/Yrika819/Serea/actions/runs/40804694025) | Android | Change Scope PASS; Build/Lint/Instrumentation Compile PASS; Security PASS; Integration PASS; Unit PASS; Documentation PASS; Dependency Audit PASS; Storage and Emulator FAIL; Pixel 7a Reacceptance Bundle PASS; Personal Beta Identity Isolation SKIPPED because it depends on emulator success; Android Emulator Required Gate FAIL. | Unit, integration, security JUnit, debug APK, API26, API35, emulator-evidence, and CI Pixel bundle artifacts (details below). |
| [40804693907](https://github.com/Yrika819/Serea/actions/runs/40804693907) | Full CI | Windows P7 Gate C synthetic validation PASS (`1000023175`); Linux stable full validation PASS (`1000023194`); Linux MSRV 1.85 PASS (`1000023177`); macOS arm64 and macOS Intel x86_64 QUEUED with no runner assigned. | None at capture. |
| [40804693934](https://github.com/Yrika819/Serea/actions/runs/40804693934) | Native platform validation | Stable Linux ARM64/x86_64, stable macOS ARM64, stable Windows ARM64/x86_64, and Rust 1.85 Linux ARM64/x86_64 and Windows ARM64/x86_64 PASS. Stable macOS x86_64 IN_PROGRESS. Rust 1.85 macOS x86_64 and ARM64 QUEUED with no runner assigned. | None at capture. This workflow validates the Rust native platform matrix; it is not the Android llama.cpp/JNI runtime test. |
| [40804693978](https://github.com/Yrika819/Serea/actions/runs/40804693978) | Cross-architecture SQLite portability | macOS ARM64 producer PASS (`1000023234`); macOS Intel x86_64 producer and cross-architecture consumer QUEUED without runners. | `portable-db-arm64`, artifact `12869265751`, 36,954 bytes, digest `sha256:a24f408ba84461818e3f4bb915977ad699900c54ac5dc18416009e3d305be039`. Producer pass only; portability consumer remains pending. |
| [40804693889](https://github.com/Yrika819/Serea/actions/runs/40804693889) | Rust Dependency Security | RustSec audit PASS, runner `1000023151`. | `rust-security-d444e7cc04c8f1c4c6d12b1412cfe3a76258af66-40804693889`, artifact `12867589033`, 3,189 bytes, digest `sha256:0f360ef81a5ab03efc14fecf99197c758df0be54cd48cd058935caa11078bee7`. |
| [40804693992](https://github.com/Yrika819/Serea/actions/runs/40804693992) | Fast CI | Linux fast checks PASS, runner `1000023138`. | None. |
| [40804693919](https://github.com/Yrika819/Serea/actions/runs/40804693919) | Git hooks Windows compatibility | Windows, Ubuntu, and macOS jobs PASS (`1000023140`, `1000023147`, `1000023145`). | None. |
| [40804722382](https://github.com/Yrika819/Serea/actions/runs/40804722382) | Android Release Security Focused (non-required) | Mock release contract, SBOM, and CI-only debug APK checks PASS, runner `1000023201`. | `serea-android-release-security-focused`, artifact `12868174398`, 11,845,572 bytes, digest `sha256:17ad843799fdde28e42a9ea099d1c9d571e8f6d093b958a1ee17a464037c40a7`. |
| [40804722157](https://github.com/Yrika819/Serea/actions/runs/40804722157) | Android Personal Beta Contract Tests | Mock Firebase/signing contract and emulator guard checks PASS, runner `1000023185`. | None. |

The queued macOS jobs had empty `runner_name` and no conclusion. This is runner wait, not a product failure or a pass. The Android emulator job had an assigned runner and was actively installing an image; API 26/35 tests had not started at this snapshot. No automatic rerun was requested.

### Runner refresh at 03:08 UTC

PR #146 remains pinned to the same source SHA above. Full CI has four passing jobs (Windows P7 Gate C runner `1000023175`, Linux stable runner `1000023194`, Linux MSRV runner `1000023177`, macOS ARM64 runner `1000023279`) and macOS Intel x86_64 queued without a runner. Native has 10/12 jobs completed successfully; Rust 1.85 macOS ARM64 is in progress on runner `1000023312`, and Rust 1.85 macOS x86_64 remains queued. SQLite has three successful producer/consumer jobs; the final Intel-database-on-ARM64 consumer remains queued. These queued jobs are infrastructure waits; no failures are recorded in this refresh.

For successor PR #149 exact source `4500179bb32ea9696160e7bab115e19e45016da7`, Tree `f4dc2f738a1e84ba2683bc1b87891748f01f2a55`, the 03:08 UTC Actions API snapshot shows Android, Native, Fast, RustSec, SQLite, and Git Hooks jobs queued without runner assignment. Full CI has Windows P7 Gate C in progress on runner `1000023317`; its four Linux/macOS jobs remain queued. No PR #149 job had completed and no API 26/35, JUnit, Identity, Process Death, Data Isolation, Sentinel, APK, or artifact evidence exists at this snapshot. Do not rerun; keep the source bound and wait for scheduled jobs.

### Android JUnit and build artifacts

The Android JUnit, debug APK, release security, and historical Native artifacts downloaded through the GitHub Actions artifact endpoint had ZIP SHA-256 values matching the Actions artifact digests.

| Evidence | JUnit result | Artifact identity |
|---|---|---|
| Android unit suite | 405 tests; 0 failures, 0 errors, 0 skips; 34 XML files. | `serea-android-unit-junit`, ID `12869510041`, digest `sha256:c727e3421dab84a7b82a26920b2a0502f1938d560777482852e78afabbd35487`, 29,487 bytes. |
| Android integration suite (`OllamaCloudAdapterTest`) | 22 tests; 0 failures, 0 errors, 0 skips; 1 XML file. | `serea-android-integration-junit`, ID `12869310588`, digest `sha256:62d700d4c25ac1ef0c9ddc3645586028a2347465c42370e16a3153ab108a0355`, 1,227 bytes. |
| Android security suite (`SecurityPortsTest`) | 26 tests; 0 failures, 0 errors, 0 skips; 2 XML files. | `serea-android-security-junit`, ID `12868263908`, digest `sha256:6c757e8b54d382fc0e9734cd4fe6be6e892cd8d8408a96fac0dd9f949fe9ef12`, 1,830 bytes. |
| CI debug APK candidate | Build, lint, instrumentation APK compile PASS; this is not a physical Pixel 7a test or a Beta APK. | `serea-android-pixel7a-debug-apk-candidate`, ID `12869480087`, digest `sha256:d3654dbde4ed96cbcb53c74983d7d8aafa649a9aca3ba02212da1c44b3d8aa17`, 33,163,808 bytes. APK SHA-256 `784b8d86f0af195c682cbcda92a672403f06e438d7d4a841c02db6bfc19431c6`; package `app.serea.android.demo`; version/signature are the CI debug build. |

The separate non-required Release Security run also produced a source-bound debug APK and CycloneDX SBOM. Its evidence says `package_id=app.serea.android.demo`, `version_code=1`, `version_name=0.1.0-demo`, APK SHA-256 `76c28aea15ffe7f0955d1c08e59597f344ee0f53f95a41e4c79928e84f195bb3`, and SBOM SHA-256 `45b32b00ffb8368cdd81ba4b01fb7e3d7f115e40897663bc606b736bbb18c39b`. The SBOM is CycloneDX 1.5 with 317 components. Its own evidence marks dependency edges absent, build plugins/Gradle wrapper/SDK outside scope, publisher authenticity unestablished, and APK-to-source build provenance not independently attested. This is useful CI evidence, not a signed Beta provenance claim.

### Complete job inventory at capture

`runner=—` means no runner was assigned at the time captured.

| Run | Job ID / job | Result | Runner |
|---|---|---|---|
| 40804694025 | 123226282050 Android Change Scope | PASS | 1000023202 |
| 40804694025 | 123227924078 Android Build, Lint, and Instrumentation Compile | PASS | 1000023215 |
| 40804694025 | 123227924099 Android Security Tests | PASS | 1000023207 |
| 40804694025 | 123227924157 Android Integration | PASS | 1000023225 |
| 40804694025 | 123227924170 Android Unit | PASS | 1000023233 |
| 40804694025 | 123227924184 Android Documentation | PASS | 1000023224 |
| 40804694025 | 123227924251 Android Dependency Audit | PASS | 1000023227 |
| 40804694025 | 123230615644 Android Storage and Emulator | FAIL | 1000023241 |
| 40804694025 | 123232823166 Android Pixel 7a Reacceptance Bundle | PASS | 1000023246 |
| 40804694025 | 123232824352 Personal Beta Identity Isolation (temporary signing keys) | SKIPPED | — |
| 40804694025 | 123232881578 Android Emulator Required Gate | FAIL | 1000023247 |
| 40804693907 | 123220796323 Windows P7 Gate C synthetic validation | PASS | 1000023175 |
| 40804693907 | 123220796675 Linux stable full validation | PASS | 1000023194 |
| 40804693907 | 123220796723 Linux MSRV 1.85.0 | PASS | 1000023177 |
| 40804693907 | 123220796736 macOS arm64 | QUEUED | — |
| 40804693907 | 123220796743 macOS Intel x86_64 | QUEUED | — |
| 40804693934 | 123220813534 Stable Linux ARM64 | PASS | 1000023149 |
| 40804693934 | 123220813702 Stable Linux x86_64 | PASS | 1000023203 |
| 40804693934 | 123220813728 Rust 1.85.0 Windows ARM64 | PASS | 1000023159 |
| 40804693934 | 123220813735 Stable macOS ARM64 | PASS | 1000023189 |
| 40804693934 | 123220813816 Stable macOS x86_64 | IN_PROGRESS | 1000023237 |
| 40804693934 | 123220813844 Stable Windows x86_64 | PASS | 1000023143 |
| 40804693934 | 123220813861 Rust 1.85.0 macOS x86_64 | QUEUED | — |
| 40804693934 | 123220813882 Rust 1.85.0 Linux ARM64 | PASS | 1000023164 |
| 40804693934 | 123220813923 Stable Windows ARM64 | PASS | 1000023160 |
| 40804693934 | 123220813939 Rust 1.85.0 Linux x86_64 | PASS | 1000023213 |
| 40804693934 | 123220814007 Rust 1.85.0 Windows x86_64 | PASS | 1000023166 |
| 40804693934 | 123220814009 Rust 1.85.0 macOS ARM64 | QUEUED | — |
| 40804693978 | 123220722159 Produce closed DB on macOS arm64 | PASS | 1000023234 |
| 40804693978 | 123220722413 Produce closed DB on macOS Intel x86_64 | QUEUED | — |
| 40804693978 | 123230198552 Open arm64 DB on macOS Intel x86_64 | QUEUED | — |
| 40804693889 | 123220717824 RustSec dependency and vendor audit | PASS | 1000023151 |
| 40804693992 | 123220718319 Linux fast checks | PASS | 1000023138 |
| 40804693919 | 123220717911 Windows x64 Git hooks with python fallback | PASS | 1000023140 |
| 40804693919 | 123220718168 ubuntu-latest Git hook regression | PASS | 1000023147 |
| 40804693919 | 123220718300 macos-latest Git hook regression | PASS | 1000023145 |
| 40804722382 | 123220819493 Mock Release Contract and Artifact-Bound SBOM | PASS | 1000023201 |
| 40804722157 | 123220818918 Mock Personal Beta Signing and Firebase Contract | PASS | 1000023185 |

### Latest Actions refresh — 2026-10-29 02:57 UTC

The GitHub Actions API was queried again after the initial table. All nine requested runs still identify PR #146 source `d444e7cc04c8f1c4c6d12b1412cfe3a76258af66`, attempt 1. No PR #146 run was manually rerun.

- **Android 40804694025:** remains completed FAILURE; API 26/35 emulator preflight stopped before instrumentation. The job results and artifacts listed above are unchanged.
- **Full CI 40804693907:** Linux, MSRV and Windows synthetic jobs PASS. macOS arm64 is now assigned to runner `1000023279` and IN_PROGRESS; macOS Intel x86_64 remains QUEUED with runner ID 0.
- **Native platform 40804693934:** existing Linux, Windows and macOS arm64 jobs PASS. Stable macOS x86_64 is now assigned to runner `1000023237` and IN_PROGRESS. Rust 1.85 macOS x86_64 and ARM64 remain QUEUED with runner ID 0. This is still the Rust platform matrix, not Android JNI validation.
- **SQLite 40804693978:** both macOS producers now PASS. The Intel producer ran on `1000023274` and uploaded `portable-db-intel`, artifact `12869541631`, 36,954 bytes, digest `sha256:f11760407eca7777e4685c0b72dc19fc14783c4251e2d11d92458a840ae14f7c`. The ARM64 producer artifact remains `12869265751`, digest `sha256:a24f408ba84461818e3f4bb915977ad699900c54ac5dc18416009e3d305be039`. Both cross-architecture open/consumer jobs remain QUEUED with runner ID 0.
- **Terminal runs:** RustSec `40804693889`, Fast `40804693992`, Git Hooks `40804693919`, Release Security `40804722382`, and Mock Beta Contract `40804722157` remain SUCCESS at the same exact source.

Queued jobs have no runner assignment and are infrastructure waits. Assigned in-progress macOS jobs are not failures and are not passes. Do not treat the overall workflow API's `queued` status as a completed result while its job list contains active work.

### Latest Actions refresh — 2026-10-29 03:05 UTC

All observations remain tied to PR #146 source `d444e7cc04c8f1c4c6d12b1412cfe3a76258af66`, attempt 1. Android run `40804694025` remains completed FAILURE with the same API 26/35 preflight result; no rerun was initiated. Progress since 02:57:

- Full CI macOS arm64 `123220796736` is now PASS on runner `1000023279`; macOS Intel x86_64 `123220796743` remains QUEUED with no runner.
- Native stable macOS x86_64 `123220813816` is now PASS on runner `1000023237`; Rust 1.85 macOS x86_64 `123220813861` and ARM64 `123220814009` remain QUEUED with no runner.
- SQLite's arm64-DB-on-Intel consumer `123230198552` is assigned to `1000023305` and IN_PROGRESS. The Intel-DB-on-arm64 consumer `123235254022` remains QUEUED with no runner. Both producer jobs PASS and their artifact identities remain those listed above.
- These runs' API-level workflow status remains `queued` because at least one job is still queued/in progress; no terminal overall PASS exists yet.

### Successor package-query fix review

Parent's successor integration PR [#149](https://github.com/Yrika819/Serea/pull/149) is OPEN Draft, based on PR #146's exact `d444e7cc04c8f1c4c6d12b1412cfe3a76258af66` / tree `25e2bb481a0b25bfed89d6ee25f1812316703b8e`. At review, PR #149 head is `dd8c5b661c1f4be6c4a171c68246cbd1fb343d50`, tree `c049cfd8b2849e738932e4c4844219c815034740`.

Independent static review decision: **ACCEPTED** for the `tools/android_package_path.sh` fix and its regression. AOSP Android 8.0 and Android 15 `pm path` implementations return exit 1 with no output when the package lookup has no path; `pm list packages -u` supports the uninstalled-package flag on both and provides a second Package Manager query. The helper now accepts absence only when that query exits successfully with empty output. A listed package, failed list query, unexpected content, transport error, malformed path, or other `pm path` failure remains closed. AOSP references: [Android 8.0 Pm.java](https://android.googlesource.com/platform/frameworks/base/+/android-8.0.0_r1/cmds/pm/src/com/android/commands/pm/Pm.java) and [Android 15 PackageManagerShellCommand.java](https://android.googlesource.com/platform/frameworks/base/+/android-15.0.0_r1/services/core/java/com/android/server/pm/PackageManagerShellCommand.java).

The legacy `Error: package not found` strings are not emitted by the AOSP `pm path` implementations above (the Android 15 occurrence is in a different PM command). Their residual matching cases are safe because they still require the independent successful-empty query. Parent's focused evidence for PR #149: `PYTHONPATH=. python3 tests/android_emulator_workflow_tests.py` — 34 tests PASS, 0 failures; `bash -n tools/android_package_path.sh` PASS; `git diff --check` PASS. PR #149's formal workflows are separate from the frozen #146 runs and are not evidence for its exact head.

### API 26/35 instrumentation and lifecycle evidence

Both emulator campaigns recorded their device API level and completed the isolated Process Death/data-isolation harness, but the bounded `cursor-position-focused` diagnostic stage stopped before any instrumentation test started:

| API | Required instrumentation | Diagnostic result | Data Isolation / Sentinel | Actual OS Process Death | Artifact |
|---|---|---|---|---|---|
| 26 | `NOT_RUN`; 0/105 started and finished; no JUnit files; zero recorded test failures because no tests ran. | FAIL at `run_diagnostic_stage`: `could not query isolated package state; refusing to install into an unverified emulator.` Gradle exit/status `NOT_RUN`. | PASS; demo history and `model_selection` sentinel seeded and verified without demo history mutation. | PASS; harness logs explicitly say force-stopped the isolated app without clearing data, recovered encrypted history/Usage Ledger state, completed Delete All, and preserved Demo sentinel. Host process-death exit 0. | `serea-android-instrumentation-api26`, artifact `12869216935`, 9,044 bytes, digest `sha256:d08309fde30db9fb67957bb1631ed5aba7c68f0f8d929f332da240a862c37afd`. |
| 35 | `NOT_RUN`; 0/105 started and finished; no JUnit files; zero recorded test failures because no tests ran. | FAIL at the same package-state query before Gradle. `api-result=FAIL`; Gradle exit/status `NOT_RUN`. | PASS; Demo sentinel seed/verify passed. | PASS; force-stop/recovery, encrypted history/Usage Ledger, deletion, and sentinel preservation passed. Host process-death exit 0. | `serea-android-instrumentation-api35`, artifact `12868798855`, 9,090 bytes, digest `sha256:06ed97683b5d292c2f87c0d47f1eebacddc175f7e4944530f751abe8d10134c3`. |

The aggregate `serea-android-emulator-evidence` artifact is `12869770024` (11,853,573 bytes; digest `sha256:fa8426a3e202ad445d4b3a5b00c021e7c2c3888fe74b8d28e05dbbc4d2049fad`). The CI-only Pixel 7a reacceptance bundle artifact is `12868893543` (45,057,882 bytes; digest `sha256:95e8ec9c7d5fc46e7e16fb33f8b8703afe8daa5e8d2d2d1a2cc6342002621155`); its workflow job passes, but physical Pixel 7a acceptance remains NOT_RUN.

At the #146 head, `android_package_path()` treated any nonzero `adb shell pm path` result as a query failure, while stderr was suppressed. The failure happened before instrumentation and before the Gradle instrumentation diagnostic command, for the not-yet-installed isolated target/test packages. The artifact does not preserve the original package-manager stderr/exit details, so the exact original `pm`/ADB subcause is unconfirmed; do not claim that 105 product tests failed or that artifact upload omission caused this. Evidence classifies the Required failure as an emulator test-harness preflight failure. A separately reviewed successor fix exists on PR #149; #146 remains unchanged, and this review does not convert its failed formal run into a pass.

No API26/35 JUnit ID exists because no JUnit XML was produced. The expected manifest is 103 Original Required + 2 Additional = 105 per API, but neither suite ran. Fake Google/Gmail Android UI tests are also NOT_RUN because they are gated on successful required instrumentation. The temporary-key Personal Beta Identity job was SKIPPED on this run because its `if` condition requires the emulator job to succeed; Identity is therefore unproven at this candidate.

## PR #144 native build failure analysis

The current PR #144 candidate is `07ea5c6b460cea2eb1ccccc49ccd4f74dc65bb59` / Tree `ff721fc7ae7642cbaf226376c23df6541d7c8248`.

### Current PR #144 head

Run [40803623345](https://github.com/Yrika819/Serea/actions/runs/40803623345), attempt 1, failed on the assigned GitHub runner `1000023205` in job `123217271235`, **Product adapter on pinned API 35 x86_64 Emulator**. The failing step was **Build opt-in product JNI APKs for both supported ABIs**. The Gradle invocation was:

```text
./gradlew --no-daemon --console=plain \
  -PsereaLocalNativeRuntime=true \
  -PsereaLlamaCppSource="$GITHUB_WORKSPACE/llama.cpp" \
  :app:assembleIsolated :app:assembleIsolatedAndroidTest
```

The build redirected Gradle output to `evidence/g8-local-native/gradle-native-build.log`; the run returned exit 1. The current run has no uploaded artifact, and `--log-failed` shows the command/environment and exit code but not the redirected Gradle failure. Therefore the current-head failure is a confirmed Android Gradle/JNI build-path failure with **subtype and root cause unconfirmed**. Do not infer CMake, JNI source, NDK, ABI link, APK packaging, or a test harness cause until the log is recovered. Artifact recovery for this run is incomplete.

### Earlier PR #144 candidate

Run [40800987367](https://github.com/Yrika819/Serea/actions/runs/40800987367), attempt 1, is source `082d223d9975492b5dab3c70349f0d6e9fc504de`, Tree `c25df6797e0b00946afb6ee5c8893521bacc0ca6`. Its diagnostic artifact `12867015936`, 6,054 bytes, digest `sha256:d8d4ff9c94c387ad8ca9bcd9d4cc179bb95d1999ef109a16a0820780a55683a3`, contains `gradle-native-build.log`, JUnit, and `source.txt`.

That log confirms CMake x86_64 configuration/build and `:app:assembleIsolated` succeeded, then `:app:compileIsolatedAndroidTestKotlin` failed at `LocalModelNativeRuntimeE2ETest.kt:126:80`: unresolved reference `awaitReleased` on `LocalModelInferenceBackend`. This historical failure is an Android instrumentation Kotlin test/API mismatch, not a CMake/JNI compile failure or an APK target packaging failure. The artifact contains 21 passing `LocalModelChatAdapterTest` cases and one passing `LocalModelNativeRuntimeTest` case (`defaultProductBuildFailsClosedBeforeLoadingNativeLibrary`); it contains no successful real inference result. Recorded model identity: llama.cpp commit `276378515ceebee5d14bcc8536603dd5c87491a2`, model revision `1076b85d135017971845b19b8d07df1479155988`, SHA-256 `e3131339bf4e8065265593d4fd8f7bb7ff2d3abff1edb5618aa1197b89cad9f5`, size 101,016,128 bytes. Its status file records Android build, ARM64 build/runtime, and Pixel 7a physical runtime as NOT_RUN.

The earlier failure gives a concrete fixed defect for `082d…`; it does not prove that `07ea…` has the same defect. Lane A owns retrieval/diagnostics and the next native attempt.

## PR #147 Lane A source review and focused runtime attempt

PR #147 is OPEN / Draft. Reviewed candidate identity: Source `49e14da2b5dc06579ff88d0adbc10d41f5040dd9`, Tree `bcc5ce1f837d2904f608c60db26569469340e695`, based on G7 SHA `0560e1e7bcd13f0194eab8caca89d70ea5a2f142`. This contains the isolated JNI/runtime candidate and the updated AndroidTest contract; it is not accepted for native runtime or product integration.

Run [40808797590](https://github.com/Yrika819/Serea/actions/runs/40808797590), attempt 1, exact source `49e14da…`, failed on runner `1000023273` in job `123233966301` (Product adapter on pinned API 35 x86_64 Emulator). Evidence confirms JNI/CMake configuration, both x86_64 and ARM64 APK builds, license metadata check, exact model fixture revision/size/hash, disposable API 35 emulator boot, and temporary model staging completed. `connectedIsolatedAndroidTest` then **started 0 tests**. The target process crashed in `SereaIsolatedAndroidJUnitRunner.onStart` with:

```text
Refusing instrumentation without a fresh isolated namespace ...
filesEmpty=false, filesDirEntries=[g8-smollm2-135m-q4_k_m.gguf],
campaignTokenPresent=false, markerMatches=false
```

The fixture had been staged in the isolated app `filesDir` before instrumentation, which violates the runner's fresh-namespace invariant; the direct Gradle invocation also omitted its required one-use `sereaCampaignToken`. This is a **harness setup failure before test execution**, not a native inference result. No JUnit evidence exists for this run. The workflow's bounded cleanup steps passed and the API lists two artifacts: sanitized runtime artifact `12870076223`, 15,855 bytes, digest `sha256:9f6c7a7fe20657522aa8e4b86b9d58b2e55102cb96a75dd16d9d2f06013748cc`; build diagnostics `12869074919`, 5,958 bytes, digest `sha256:658e850e779214c1e8b2189e6d7778f148cc128b7624ede3e5833c3b49fbe040`. Artifact download returned `Forbidden` from the GitHub artifact endpoint during this review, so the action log is the available failure detail and the zip contents were not independently inspected.

Independent Lane C source review: **CHANGES_REQUESTED** at that exact source/tree.

- **NATIVE-JNI-EXC-001 (high):** `llama_jni.cpp` catches `std::bad_alloc` only in `nativeGenerate`; other `extern "C"` JNI exports have no exception barrier. `nativeLoad` can throw after acquiring JNI string chars or claiming the process-wide owner, leaving partial model/context state unless cleanup is made exception-safe. Required fix: contain all C++ exceptions at every exported JNI boundary, translate to Java failures without overwriting pending exceptions, use RAII for path chars, and free partial state/release ownership on failed loads. Require focused failure-path coverage.
- **NATIVE-CLEANUP-002 (high):** `run-e2e.sh` checks API 35 and x86_64 but does not prove emulator hardware or that the fixed isolated package IDs were absent before install. Its EXIT cleanup uninstalls packages if present; direct use on a matching physical/reused device could remove pre-existing app data. The cleanup failure branch also recursively deletes environment-provided `G8_EVIDENCE_DIR` without constraining its canonical target. Required fix: verify one disposable emulator before mutation, refuse existing target/test packages, constrain evidence cleanup to a dedicated safe workspace, and test the rejected paths.

Lane A is implementing these source changes on a new head. After the run failure, Lane A identified that a corrected attempt must both stage the verified fixture outside `filesDir` (such as cacheDir) and follow the existing fresh-namespace token seed protocol, including passing `-Pandroid.testInstrumentationRunnerArguments.sereaCampaignToken=<token>`. No model inference, 2-turn generation, native cancellation/release, crash recovery, ARM64 runtime, or Pixel 7a result is established yet. The Native Runtime gate remains **PARTIAL**.

## PR #150 Lane B handoff review

PR #150 is OPEN / Draft, based on PR #146. The current PR head is `31efa59eed6a7da7f058b7d17be31c7944190ab5`, Tree `0168cd5d100383a564f5a2609fe369a423ff9c94`; its current tip adds only the report link. The reviewed code/test commit is `4e4b8f9d0aad80f330d1c93364b3cbfda5369bcd`, Tree `b19a6c52a8e8d4ff1c8365d435c047d0aceb300b`.

Independent Lane C decision: **ACCEPTED** for the scoped Gmail handoff minimization code and its tests. Gmail Chat content now takes nonblank plain text then the parser-produced inert HTML text; subject, sender, date, API snippet and Gmail/handoff metadata are excluded. The handoff remains Fake-only, short-lived, account/session/generation bound, source/revision/content bound, revalidated at preview/confirm/send, single-use, and explicitly local-only. GMAIL lineage is retained; cloud egress denial despite local handoff consent and non-dispatch before confirmation are covered by the existing security tests. New tests cover metadata exclusion and parser-produced HTML fallback with script, event-handler and remote-image sentinels. Lane B reported the exact-code focused JVM test at 22/22 and isolated AndroidTest source compilation as successful. No API 26/35 Gmail UI execution or exact-current-head formal CI is evidenced; those remain NOT_RUN, so this review does not accept the broader G9 UI milestone or real Gmail OAuth.

## PR #149 Google/Gmail UI diagnostic expansion review

The parent committed the reviewed diagnostic expansion as PR #149 head `4500179bb32ea9696160e7bab115e19e45016da7`, Tree `f4dc2f738a1e84ba2683bc1b87891748f01f2a55` (base branch `codex/serea-g8-integration-candidate-20261029-r1`). Lane C independently rechecked the exact committed diff and **ACCEPTED** it for the scoped diagnostic change. The stage keeps the prior nine non-required tests, adds seven UI/handoff/lifecycle identities already in the non-required manifest, checks they do not overlap Required or Additional tests, requires exact 16/16 JUnit, runs on API 26 and 35 after Required JUnit, actual process death, and data isolation pass, retains package cleanup and Fake-only local Chat tests, and requires `google_demo_ui_result == PASS` before an API campaign can be marked PASS. The campaign rejects API levels other than 26/35 at entry. This is a source review only; no Android UI execution was performed by Lane C.

At 2026-10-29 03:06 UTC, exact-head PR runs were queued without a started runner: Android `40810167891`, Full CI `40810167726`, Native `40810167708`, Fast `40810167710`, RustSec `40810167734`, SQLite `40810171127`, and Git Hooks `40810167802`. Their run and job records remained queued with no runner assignment. This is pending infrastructure scheduling, not a product failure. The separate push-triggered Personal Beta workflow `40810162939` fails its main-only guard and is not PR validation. No run was rerun in this review.

### API 26 skipped-stage classification correction and final review

Follow-up inspection found that the API 26 fallback still labeled the Google/Gmail stage `NOT_REQUIRED` when any precondition failed, even though it is now part of the API 26 success predicate. The overall API result still failed closed, but the stage evidence label was inaccurate. The parent removed that branch, so gate failures leave both UI and cleanup statuses as `NOT_RUN`; the final API predicate continues to require `google_demo_ui_result == PASS`. The targeted static regression now asserts that neither status can be assigned `NOT_REQUIRED`.

Lane C independently re-reviewed the correction and **ACCEPTED** exact PR #149 head `3a0f0c1897f2115549e7682d6fcc1783c44604f3`, Tree `6a1f4d382907a7deb7d6dbf8147e6490aa054e81`. The parent reports the focused `android_emulator_workflow_tests.py` suite passed 34/34 with zero failures, errors, or skips. This does not constitute API 26/35 UI execution. At 03:12 UTC, exact-head Actions runs Android `40810824093`, Git Hooks `40810824150`, Fast `40810824083`, RustSec `40810824040`, SQLite `40810824022`, Native `40810824148`, and Full CI `40810824061` were all queued with runner ID 0 and no completed jobs. The push-triggered Personal Beta workflow `40810819146` again fails the `main`-only guard and is not PR validation. No Actions run was rerun.

## Release readiness and Owner decision packet

### Decisions required for 3 November

| Decision | Evidence / safe choices to decide |
|---|---|
| Beta package ID | CI currently builds `app.serea.android.demo`. No Beta application ID was selected. Choose whether Beta replaces/upgrades Demo under the same package identity or coexists under a different `applicationId`; a different ID is a separate app/data namespace and does not provide an in-place update path. |
| Demo/Beta coexistence | Select one explicit install/update policy before OAuth registration and release configuration. The existing debug package and artifact are not a Beta identity. |
| Signing certificate | No Owner production key has been created or used. CI artifacts are debug-signed; the inspected Pixel candidate signer certificate SHA-256 was `d1c37ae27479389c38bf7a713f4883c30e6a62e596925a5c6e17c59c32371f20`. Choose owner-managed sideload signing versus Google Play App Signing only if distribution channel changes. Preserve the app-signing identity for every same-package update; keep production private keys outside source and CI. |
| Version/update policy | Debug evidence has versionCode 1 / versionName 0.1.0-demo. Beta should use a monotonic versionCode and a documented same-key update path. Do not claim rollback is tested; test rollback by installing a rebuilt prior source with a higher versionCode on a disposable profile. |

### Decisions required for 10 November

| Decision | Current disposition |
|---|---|
| Google Cloud project and OAuth client | NOT_CONFIGURED. Owner must select the project, consent screen, package ID, and certificate fingerprint. No real OAuth or account was used. |
| Calendar scope | Prefer `calendar.events.readonly` if event listing/detail is sufficient; it grants event reads on accessible calendars. `calendar.readonly` reads/downloads any accessible calendar and should be chosen only if needed. Scope and product operations remain Owner decisions. |
| Gmail scope | `gmail.readonly` is a Restricted scope. It is enough for message read; do not request `gmail.modify`, `gmail.compose`, `gmail.send`, or broad `mail.google.com` for this Beta. Google verification and potentially a security assessment apply. No real Gmail access was run. |
| Google data transfer | Google API Services User Data Policy limits use and transfer; transfers for user-facing features require explicit user consent. Serea's campaign invariant is stricter: Google-origin Cloud AI egress remains DISABLED, independent of provider settings. Google connection consent, local handoff consent, and Cloud consent stay separate. |
| Cloud AI provider | NOT_SELECTED / NOT_ENABLED. Owner's country/region, app audience age, applicable API contract, retention, and pricing were not provided/verified. Gemini API Additional Terms require API users to be 18+ and prohibit API Clients directed to or likely accessed by under-18 users. OpenAI Terms of Use (effective 2026-01-01) set 13 or local consent minimum for individual Services and parent/guardian permission under 18; the Services Agreement places responsibility on the customer not to allow minors without parent/guardian consent. The exact OpenAI API contract/audience conditions must be verified if that provider is selected. No keys, API calls, billing, or Google-origin prompts were used. |
| Provider geography | Owner's residence and deployment/consumer region are unknown. Check the selected provider's current supported country/region list and terms before enabling. Region eligibility is BLOCKED pending that choice. |
| Local model | NOT_SELECTED. Lane A's revision-specific model code, weights license, runtime compatibility, redistributable rights, size, and runtime evidence are required before setting a standard model or shipping weights. No model weight was added to this branch. |

### Release checklist prepared; execution remains gated

- **Encrypted migration:** existing history/tasks DB schemas and migration unit coverage are in CI. A real same-package, same-signature upgrade and recovery on the intended Beta package is still NOT_RUN. Preserve encrypted user data; do not test destructive migrations on a personal profile.
- **Native inventory / SBOM:** current 317-component CycloneDX inventory is lockfile-derived and explicitly partial. Before RC, include Gradle/plugin/SDK and all native code/build inputs, NDK, ABI libraries, licenses/NOTICE, model revision/hash/license, and an APK-bound SBOM/provenance statement.
- **APK provenance:** current CI debug APK is bound to Source SHA `d444e7c…` and an APK hash, but is signed with Android debug key. No production-signed artifact or independent source-build attestation exists.
- **Rollback:** document retaining a prior compatible source/build, bumping versionCode for a same-key rollback package, database forward/backward compatibility, and backup/restore on synthetic data. No downgrade/restore exercise is evidenced.
- **Pixel 7a:** NOT_RUN. Requires explicit Owner-supplied device/approval. Acceptance list: verify package/certificate non-destructively; install/update without uninstall/data clearing; launch the real local model offline and capture model hash, load/TTFT/prefill/decode/memory/cancel/release; verify encrypted history/tasks survive app restart, activity recreation, OS process death and update; confirm Calendar/Gmail consent, isolation, revoke and account-switch behavior only if real OAuth is separately approved; exercise Tasks approval/CAS, IME, orientation, accessibility and crash recovery; bind all results to one frozen RC SHA and APK hash.
- **OAuth runbook:** select app/package and SHA-1/SHA-256 signing fingerprints, create the consent screen, request only approved scopes, complete verification/security assessment, test account/session-generation binding, revoke/expire, stale callbacks, 401/403/429 and recovery, and keep OAuth secrets/data out of CI. Real OAuth activation remains unauthorized.
- **Gmail Restricted-scope checklist:** request `gmail.readonly` only; verify Restricted-scope review and assessment obligations; use inert text only with explicit handoff; exclude attachments and metadata from Chat payloads; test remote fetch/script rejection, body-size/MIME limits, account/session binding and revocation; prohibit send/modify scopes; keep all provider-derived Cloud egress disabled.
- **Personal Beta decision:** NO-GO until all P0 gates (exact final HEAD Required CI, native model runtime and Pixel 7a, identity/update/migration, privacy/security and required owner decisions) pass and Owner makes the final decision.

## Official references checked 2026-10-29

- [Google restricted-scope verification](https://developers.google.com/identity/protocols/oauth2/production-readiness/restricted-scope-verification) — restricted scopes can require extra verification and a security assessment.
- [Gmail API scopes](https://developers.google.com/workspace/gmail/api/auth/scopes) — `gmail.readonly` is classified under Restricted scopes; the broad `mail.google.com` scope can permanently delete mail.
- [Calendar API scopes](https://developers.google.com/workspace/calendar/api/auth) — describes the relative breadth of `calendar.events.readonly` and `calendar.readonly`.
- [Google API Services User Data Policy](https://developers.google.com/terms/api-services-user-data-policy) — Limited Use and user-consent conditions for transfer of user data.
- [Gemini API Additional Terms](https://ai.google.dev/gemini-api/terms) — 18+ API user requirement and restrictions on API Clients aimed at or likely accessed by under-18 users.
- [OpenAI Terms of Use](https://openai.com/policies/terms-of-use/) and [Services Agreement](https://openai.com/policies/services-agreement/) — age/guardian conditions for individual Services and customer responsibility around minors and submitted input; re-check API-specific terms at provider selection.
- [OpenAI supported countries](https://platform.openai.com/docs/supported-countries) and [Gemini API available regions](https://ai.google.dev/gemini-api/docs/available-regions) — check the Owner's actual region at activation time.
- [Android app signing](https://developer.android.com/studio/publish/app-signing) — update signing identity and key custody considerations.
- [Android Keystore](https://developer.android.com/privacy-and-security/keystore) — Android Keystore key material is non-exportable and can be constrained to cryptographic/authentication conditions.
- [Android versioning](https://developer.android.com/studio/publish/versioning) and [application ID](https://developer.android.com/build/configure-app-module#set-application-id) — release versionCode and application identity/update decisions.

## Agent report contract

```text
AGENT_ID: C
ACTIVE_PHASE: G8 Evidence Closure / G10 Preflight
GOAL_STATUS: PARTIAL
BASE_SHA: 0560e1e7bcd13f0194eab8caca89d70ea5a2f142
CURRENT_HEAD: report branch head recorded by GitHub
TREE_SHA: tree of the report branch head recorded by GitHub
WORKTREE: /workspace/Serea-C
BRANCH: codex/serea-g8n-security-release
DRAFT_PR: https://github.com/Yrika819/Serea/pull/148
COMPLETED_WORK: Exact PR #146 CI/API evidence snapshot plus runner refreshes through 03:12 UTC; CI artifact hashing and JUnit counts; PR #144 current and historical native-build failure classification; PR #147 exact-head failure classification and independent source review; PR #150 scoped Gmail handoff security review; PR #149 package-query patch and final committed Google/Gmail UI expansion review; Owner decision and release readiness packet.
CHANGED_FILES: docs/agent-reports/g8n-ci-security-release.md
TEST_COMMANDS: none (evidence/report lane; no code tests were requested or needed)
JUNIT_RESULT: PR #146 Android unit 405/405, integration 22/22, security 26/26; API 26/35 instrumentation NOT_RUN (preflight FAIL, 0/105 each); Process Death/Data Isolation/Sentinel PASS on both
CI_RUN_IDS: 40804694025, 40804693907, 40804693934, 40804693978, 40804693889, 40804693992, 40804693919, 40804722382, 40804722157; native PR #144 40803623345 and historical 40800987367; PR #147 focused native attempt 40808797590
ARTIFACT_IDS: 12869510041, 12869310588, 12868263908, 12869480087, 12868174398, 12869265751, 12869541631, 12867589033, 12867015936, 12870076223, 12869074919
SECURITY_IMPACT: Documentation only; preserves Google-origin egress prohibition, debug-only release boundary, and no production signing.
REVIEW_STATUS: PR #149 package-query patch ACCEPTED; final committed UI expansion ACCEPTED at `3a0f0c1897f2115549e7682d6fcc1783c44604f3` / `6a1f4d382907a7deb7d6dbf8147e6490aa054e81`; PR #150 Gmail handoff code/test ACCEPTED at exact code/test commit; PR #147 CHANGES_REQUESTED on exception safety and disposable cleanup; PR #144 remains BLOCKED on current-head failure diagnostics.
KNOWN_ISSUES: PR #146 API26/35 instrumentation did not start because the harness preflight package-state query failed; Process Death/Data Isolation/Sentinel passed separately. Identity was skipped. At 03:12 PR149 exact-head jobs were queued without runner assignment; the #146 Full CI Intel, Native Rust/macOS MSRV and one SQLite consumer were still queued at the 03:08 snapshot. PR #147 run 40808797590 started 0 native tests and crashed at runner fresh-namespace/token guard; no inference evidence. PR #144 current-head failure remains without uploaded diagnostic artifact and with root cause unknown.
CURRENT_BLOCKERS: PR #149 exact-head formal CI is queued without runner assignment; remaining #146 macOS runner jobs; Lane A JNI exception boundary and disposable cleanup review fixes; Lane A's corrected model-stage/one-use instrumentation marker harness; Owner decisions on package/signing/OAuth/providers/age-region eligibility.
NEXT_ACTION: Re-review Agent A's exact fixed PR #147 SHA/tree and run evidence; capture later #146/#149 exact-head CI outcomes.
OWNER_APPROVAL_REQUIRED: Package ID/coexistence, certificate/channel, OAuth scopes, provider/audience/region, model choice and device acceptance.
```
