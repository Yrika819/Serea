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

### PR #147 fixed-candidate review

Lane A's successor candidate is `4cbe5beadf56488012a0d9fd18fc2db619a64385`, Tree `23d63b77eef7ea4578a7bd37c16c42f8ab4e28b9`, PR #147 based on G7 `0560e1e7bcd13f0194eab8caca89d70ea5a2f142`. The source now contains exception boundaries for every JNI export, scoped Java string conversion, and `nativeLoad` rollback with a test-only throw-after-model seam. The E2E stages the verified fixture in app `cacheDir`, seeds the isolated instrumentation namespace with a run token, checks one disposable API35 x86_64 emulator, refuses caller-provided evidence paths, and creates evidence in a fresh canonical `RUNNER_TEMP` child. Lane C's static `test-jni-contract.py` check reports `jni_exception_contract=PASS exports=8 test_failure_seam=present`.

Independent decision on this exact candidate: **CHANGES_REQUESTED**, issue **NATIVE-PREFLIGHT-RET-003 (high)**. `preflight.sh` invokes `pm list packages "$package"` without `-u`. That can omit uninstalled package records whose data was retained with `adb uninstall -k`; a subsequent install could reuse the old namespace despite the fresh-target claim. Require a strict fail-closed `pm list packages -u` absence query for both app and test IDs. In cleanup, keep an installed-only query for deciding whether to call `adb uninstall`, then verify absence with `-u` so retained package data is included. Add regression coverage for retained-only records and failed/malformed queries at preflight/cleanup. Exact-head run [40811065474](https://github.com/Yrika819/Serea/actions/runs/40811065474), attempt 1, source `4cbe5be…`, was queued with no runner at 03:15 UTC. No new native inference or JUnit evidence exists for this SHA. Prior JNI exception and destructive evidence-path findings are addressed in source, but PR #147 remains unaccepted pending this fix and review of the successor.

## PR #150 Lane B handoff review

PR #150 is OPEN / Draft, based on PR #146. Its current report/evidence head is `50a56acd22fb363cf5354a3d39e67b01d980491a`, Tree `ba32d9b5eb3fdecb42c926e218b16effb096d1c7`. The reviewed Gmail code/test commit is `4e4b8f9d0aad80f330d1c93364b3cbfda5369bcd`, Tree `b19a6c52a8e8d4ff1c8365d435c047d0aceb300b`.

Independent Lane C decision: **ACCEPTED** for the scoped Gmail handoff minimization code and its tests. Gmail Chat content now takes nonblank plain text then the parser-produced inert HTML text; subject, sender, date, API snippet and Gmail/handoff metadata are excluded. The handoff remains Fake-only, short-lived, account/session/generation bound, source/revision/content bound, revalidated at preview/confirm/send, single-use, and explicitly local-only. GMAIL lineage is retained; cloud egress denial despite local handoff consent and non-dispatch before confirmation are covered by the existing security tests. New tests cover metadata exclusion and parser-produced HTML fallback with script, event-handler and remote-image sentinels. Lane B reported the exact-code focused JVM test at 22/22 and isolated AndroidTest source compilation as successful. No API 26/35 Gmail UI execution or exact-current-head formal CI is evidenced; those remain NOT_RUN, so this review does not accept the broader G9 UI milestone or real Gmail OAuth.

Independent Lane C decision: **ACCEPTED** for the read-only Chat Model Mode report at exact report head `a657bc8a7510c19451a5420e2a40e3bab921c4ed` / Tree `dfb837fef158131f0f2e55d5394a55187fa482ed`; it persists unchanged at current #150 head `50a56acd…`. It accurately distinguishes the only ordinary Chat route (`DEMO`, deterministic `MockModelChatPort`) from isolated local runtime adapters and disabled Cloud UI, and confirms the Google Fake/local handoff grant is not Cloud consent. Cloud Egress requires a bounded explicit send-intent contract and denies Google/Gmail/Calendar lineage. It also correctly identifies persisted model ID `demo-local` as inaccurate versus active Demo ID `serea-demo/local-chat` and requires reconciliation/migration before mode switching or cross-model history carry. This documentation does not make LOCAL_MODEL or CLOUD a product mode and does not accept unrun UI or Native runtime gates.

Lane C reviewed the preserved local JUnit evidence addition at current PR #150 head `50a56acd22fb363cf5354a3d39e67b01d980491a`, Tree `ba32d9b5eb3fdecb42c926e218b16effb096d1c7`: **ACCEPTED**. The local XML and runtime metadata bind to source/test Tree `b19a6c52a8e8d4ff1c8365d435c047d0aceb300b`, with 22 tests, 0 failures, 0 errors and 0 skips. XML SHA-256 is `214269dd60cd0119e31c199b403096f939522ccb2629cf877e169a3d726e6305`; `SHA256SUMS` verifies both evidence files. This is not a GitHub artifact or Android UI result; documentation-only commits do not rebind the tested code.

At 03:17 UTC, exact PR #150 report/evidence HEAD formal runs were pending/queued: Git Hooks `40811129766`, Android `40811130270`, Native `40811130126`, RustSec `40811129776`, SQLite `40811130124`, Fast `40811129781`, and Full CI `40811129884`. Native stable macOS ARM64 and Linux ARM64 jobs were in progress on runners `1000023380` and `1000023379`; the other ten Native jobs were queued without runners. Android was pending before job creation; the remaining workflow jobs were queued without runners. No API 26/35 UI run or artifact exists at this snapshot. Push-triggered Beta workflow `40811124717` fails the main-only guard and is not PR validation.

## PR #149 Google/Gmail UI diagnostic expansion review

The parent committed the reviewed diagnostic expansion as PR #149 head `4500179bb32ea9696160e7bab115e19e45016da7`, Tree `f4dc2f738a1e84ba2683bc1b87891748f01f2a55` (base branch `codex/serea-g8-integration-candidate-20261029-r1`). Lane C independently rechecked the exact committed diff and **ACCEPTED** it for the scoped diagnostic change. The stage keeps the prior nine non-required tests, adds seven UI/handoff/lifecycle identities already in the non-required manifest, checks they do not overlap Required or Additional tests, requires exact 16/16 JUnit, runs on API 26 and 35 after Required JUnit, actual process death, and data isolation pass, retains package cleanup and Fake-only local Chat tests, and requires `google_demo_ui_result == PASS` before an API campaign can be marked PASS. The campaign rejects API levels other than 26/35 at entry. This is a source review only; no Android UI execution was performed by Lane C.

At 2026-10-29 03:06 UTC, exact-head PR runs were queued without a started runner: Android `40810167891`, Full CI `40810167726`, Native `40810167708`, Fast `40810167710`, RustSec `40810167734`, SQLite `40810171127`, and Git Hooks `40810167802`. Their run and job records remained queued with no runner assignment. This is pending infrastructure scheduling, not a product failure. The separate push-triggered Personal Beta workflow `40810162939` fails its main-only guard and is not PR validation. No run was rerun in this review.

### API 26 skipped-stage classification correction and final review

Follow-up inspection found that the API 26 fallback still labeled the Google/Gmail stage `NOT_REQUIRED` when any precondition failed, even though it is now part of the API 26 success predicate. The overall API result still failed closed, but the stage evidence label was inaccurate. The parent removed that branch, so gate failures leave both UI and cleanup statuses as `NOT_RUN`; the final API predicate continues to require `google_demo_ui_result == PASS`. The targeted static regression now asserts that neither status can be assigned `NOT_REQUIRED`.

Lane C independently re-reviewed the correction and **ACCEPTED** exact PR #149 head `3a0f0c1897f2115549e7682d6fcc1783c44604f3`, Tree `6a1f4d382907a7deb7d6dbf8147e6490aa054e81`. The parent reports the focused `android_emulator_workflow_tests.py` suite passed 34/34 with zero failures, errors, or skips. This does not constitute API 26/35 UI execution. At 03:12 UTC, exact-head Actions runs Android `40810824093`, Git Hooks `40810824150`, Fast `40810824083`, RustSec `40810824040`, SQLite `40810824022`, Native `40810824148`, and Full CI `40810824061` were all queued with runner ID 0 and no completed jobs. The push-triggered Personal Beta workflow `40810819146` again fails the `main`-only guard and is not PR validation. No Actions run was rerun.

At 03:17 UTC, PR #149 Android Dependency Audit and Security Tests had passed (`1000023346`, `1000023373`); Change Scope was also PASS (`1000023333`). Build/Lint/Instrumentation Compile (`1000023339`) and Integration (`1000023375`) were in progress; Unit and Documentation were queued. Fast and RustSec had completed successfully (`1000023357`, `1000023338`). Full CI Windows P7 Gate C and macOS Intel were in progress (`1000023358`, `1000023352`); three jobs were queued. Native stable Windows x86_64 was in progress (`1000023360`) with eleven jobs queued. SQLite had both jobs queued. Git Hooks Ubuntu passed (`1000023366`); Windows and macOS jobs were queued. No failures were observed. Required Android API26/35 campaign, Google/Gmail UI, Identity, Process Death, Data Isolation, Sentinel, and artifacts remain NOT_RUN.

PR #146 remains on immutable source `d444e7cc…`. At 03:17 Full CI macOS Intel remained in progress on runner `1000023330`. Native Rust 1.85 macOS ARM64 passed on `1000023312`, Rust 1.85 macOS x86_64 was in progress on `1000023371`, and the other ten Native jobs passed. SQLite's final Intel-database-on-ARM64 consumer remained queued after three passes. No completion changes the original PR #146 Android Required Gate failure or NOT_RUN API26/35 instrumentation.

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
COMPLETED_WORK: Exact PR #146 CI/API evidence snapshot plus runner refreshes through 03:17 UTC; CI artifact hashing and JUnit counts; PR #144 current and historical native-build failure classification; PR #147 fixed-candidate independent review with one retained-package preflight finding; PR #150 Gmail handoff and Model Mode report reviews plus local JUnit evidence validation; PR #149 package-query patch and final committed Google/Gmail UI expansion review; Owner decision and release readiness packet.
CHANGED_FILES: docs/agent-reports/g8n-ci-security-release.md
TEST_COMMANDS: `python3 tools/g8-local-model-native/test-jni-contract.py` (read-only static source guard; PASS; no runtime claim)
JUNIT_RESULT: PR #146 Android unit 405/405, integration 22/22, security 26/26; PR #150 local GoogleTrustedHandoffTest 22/22, zero failures/errors/skips on code/test Tree `b19a6c52a8e8d4ff1c8365d435c047d0aceb300b`; API26/35 UI remains NOT_RUN. PR #146 instrumentation 0/105 each (preflight FAIL); Process Death/Data Isolation/Sentinel PASS on both
CI_RUN_IDS: 40804694025, 40804693907, 40804693934, 40804693978, 40804693889, 40804693992, 40804693919, 40804722382, 40804722157; native PR #144 40803623345 and historical 40800987367; PR #147 focused attempts 40808797590 and 40811065474; PR #150 successor runs 40811129766, 40811130270, 40811130126, 40811129776, 40811130124, 40811129781, 40811129884; PR #149 successor runs 40810824093, 40810824150, 40810824083, 40810824040, 40810824022, 40810824148, 40810824061.
ARTIFACT_IDS: 12869510041, 12869310588, 12868263908, 12869480087, 12868174398, 12869265751, 12869541631, 12867589033, 12867015936, 12870076223, 12869074919; local Gmail JUnit evidence SHA `214269dd60cd0119e31c199b403096f939522ccb2629cf877e169a3d726e6305` (no GitHub artifact).
SECURITY_IMPACT: Documentation only; preserves Google-origin egress prohibition, debug-only release boundary, and no production signing.
REVIEW_STATUS: PR #149 package-query patch ACCEPTED; final UI expansion ACCEPTED at `3a0f0c1897f2115549e7682d6fcc1783c44604f3` / `6a1f4d382907a7deb7d6dbf8147e6490aa054e81`; PR #150 Gmail handoff, Model Mode report, and local JUnit evidence ACCEPTED at their exact bound trees; PR #147 `4cbe5be…` CHANGES_REQUESTED on retained-package detection; PR #144 remains BLOCKED on current-head failure diagnostics.
KNOWN_ISSUES: PR #146 API26/35 instrumentation did not start because the harness preflight package-state query failed; Process Death/Data Isolation/Sentinel passed separately. Identity was skipped. At 03:17 PR149 and PR150 workflows were in progress/queued as detailed above; #146 Full CI Intel in progress, Native Rust/macOS x86 in progress, and one SQLite consumer queued. PR #147 run 40808797590 started 0 native tests and crashed at runner fresh-namespace/token guard; corrected exact head 4cbe5be remains queued and has no inference evidence. PR #144 current-head failure remains without uploaded diagnostic artifact and with root cause unknown.
CURRENT_BLOCKERS: PR #149 exact-head Required API26/35 and remaining CI; PR #150 exact-head formal CI and API26/35 UI; PR #146 remaining macOS runner jobs; Lane A retained-package preflight/cleanup verification fix and runtime evidence; Owner decisions on package/signing/OAuth/providers/age-region eligibility.
NEXT_ACTION: Re-review Agent A's exact fixed PR #147 SHA/tree and run evidence; capture later #146/#149/#150 exact-head CI outcomes.
OWNER_APPROVAL_REQUIRED: Package ID/coexistence, certificate/channel, OAuth scopes, provider/audience/region, model choice and device acceptance.
```

## Late evidence refresh — 2026-10-29 08:30 UTC

### Current PR identities

GitHub PR metadata and commit tree APIs were refreshed before this update. These four candidates remain distinct and no existing PR was changed by Lane C.

| PR | Exact source / tree | Base | Current state |
|---|---|---|---|
| #146 | `d444e7cc04c8f1c4c6d12b1412cfe3a76258af66` / `25e2bb481a0b25bfed89d6ee25f1812316703b8e` | `codex/serea-g7-calendar-readiness` | OPEN; frozen evidence reference |
| #147 | `1945eafd0fa3be1068ac67972026dcc8b320b339` / `5229b8ea59fc537ba607bde517991328b615e016` | `codex/serea-g7-calendar-readiness` | OPEN Draft; source/harness review accepted, model-license report change requested |
| #149 | `3a0f0c1897f2115549e7682d6fcc1783c44604f3` / `6a1f4d382907a7deb7d6dbf8147e6490aa054e81` | `codex/serea-g8-integration-candidate-20261029-r1` | OPEN Draft; static campaign patch review accepted; Android formal gate failed |
| #150 | `50a56acd22fb363cf5354a3d39e67b01d980491a` / `ba32d9b5eb3fdecb42c926e218b16effb096d1c7` | `codex/serea-g8-integration-candidate-20261029-r1` | OPEN Draft; Gmail handoff code/test and docs/evidence review accepted; Android formal gate failed |

### PR #146 completed formal CI evidence

The nine requested runs all identify exact PR #146 source `d444e7cc04c8f1c4c6d12b1412cfe3a76258af66`, attempt 1. GitHub job records now show every requested workflow terminal. Seven workflow families pass; the Android workflow is the sole Required failure. No run was retried.

| Run | Result | Job result / runner summary |
|---|---|---|
| Android `40804694025` | **FAIL** | Change Scope, Build/Lint/Instrumentation Compile, Security Tests, Integration, Unit, Documentation, Dependency Audit, and Pixel 7a Reacceptance Bundle PASS. Storage and Emulator and Emulator Required Gate FAIL. Identity Isolation SKIPPED because its dependency gate failed. API 26/35 each start 0/105 Required tests; process-death/data-isolation/sentinel harnesses pass independently. |
| Full CI `40804693907` | PASS | All five jobs pass: Windows P7 Gate C, Linux stable, Linux MSRV, macOS ARM64, macOS x86_64. |
| Native `40804693934` | PASS | All twelve platform/compiler jobs pass, including stable/Rust 1.85 across Windows, Linux, and macOS. This is a Rust platform matrix, not Android JNI evidence. |
| SQLite `40804693978` | PASS | Both database producers and both cross-architecture consumers pass. |
| RustSec `40804693889` | PASS | Dependency and vendor audit pass. |
| Fast CI `40804693992` | PASS | Linux fast checks pass. |
| Git Hooks `40804693919` | PASS | Windows, Ubuntu, and macOS jobs pass. |
| Release Security `40804722382` | PASS | Mock release contract and artifact-bound SBOM pass; it uses mock/debug signing contract only. |
| Mock Beta Contract `40804722157` | PASS | Mock Personal Beta signing/Firebase contract tests pass; no Beta was signed or distributed. |

Android run job identities are `123226282050` (Change Scope, runner `1000023202`), `123227924078` (Build/Lint/Instrumentation Compile, `1000023215`), `123227924099` (Security Tests, `1000023207`), `123227924157` (Integration, `1000023225`), `123227924170` (Unit, `1000023233`), `123227924184` (Documentation, `1000023224`), `123227924251` (Dependency Audit, `1000023227`), `123230615644` (Storage and Emulator, `1000023241`), `123232823166` (Pixel 7a Reacceptance Bundle, `1000023246`), `123232824352` (Identity Isolation, skipped/no runner), and `123232881578` (Emulator Required Gate, `1000023247`). All are attempt 1. The non-Android jobs also ran on assigned GitHub runners and completed successfully; none remain queued.

The Android workflow artifacts remain available in GitHub with these IDs and API-reported SHA-256 digests:

| Artifact | ID | SHA-256 |
|---|---:|---|
| Emulator evidence | `12869770024` | `fa8426a3e202ad445d4b3a5b00c021e7c2c3888fe74b8d28e05dbbc4d2049fad` |
| Unit JUnit | `12869510041` | `c727e3421dab84a7b82a26920b2a0502f1938d560777482852e78afabbd35487` |
| Integration JUnit | `12869310588` | `62d700d4c25ac1ef0c9ddc3645586028a2347465c42370e16a3153ab108a0355` |
| API 26 instrumentation evidence | `12869216935` | `d08309fde30db9fb67957bb1631ed5aba7c68f0f8d929f332da240a862c37afd` |
| API 35 instrumentation evidence | `12868798855` | `06ed97683b5d292c2f87c0d47f1eebacddc175f7e4944530f751abe8d10134c3` |
| Security JUnit | `12868263908` | `6c757e8b54d382fc0e9734cd4fe6be6e892cd8d8408a96fac0dd9f949fe9ef12` |
| Debug APK candidate | `12869480087` | `d3654dbde4ed96cbcb53c74983d7d8aafa649a9aca3ba02212da1c44b3d8aa17` |
| Pixel UI reliability bundle | `12868893543` | `95e8ec9c7d5fc46e7e16fb33f8b8703afe8daa5e8d2d2d1a2cc6342002621155` |
| Dependency audit | `12868532793` | `32e9947f6dc9497a516ae626b14f2d33b8fee341c9e06d8a00e05ca04162ec01` |

Other reported artifacts are SQLite ARM64 producer `12869265751` (`sha256:a24f408ba84461818e3f4bb915977ad699900c54ac5dc18416009e3d305be039`), SQLite Intel producer `12869541631` (`sha256:f11760407eca7777e4685c0b72dc19fc14783c4251e2d11d92458a840ae14f7c`), RustSec `12867589033` (`sha256:0f360ef81a5ab03efc14fecf99197c758df0be54cd48cd058935caa11078bee7`), and Release Security `12868174398` (`sha256:17ad843799fdde28e42a9ea099d1c9d571e8f6d093b958a1ee17a464037c40a7`). Full CI, Fast CI, Git Hooks, and Mock Beta Contract uploaded no artifacts. The #146 instrumentation artifact ZIP download was attempted once at 08:29 UTC from this lane and returned HTTP 403 from the GitHub artifact blob endpoint; artifact IDs/digests are recorded from the Actions artifact API, and the earlier evidence review remains the source for JUnit counts and sanitized logs.

### Current PR #149 CI and UI evidence

All seven formal workflow runs below are attempt 1 and bind to the exact #149 source/tree above. Full CI, Native platform, SQLite, RustSec, Fast CI, and Git Hooks complete successfully; Android `40810824093` fails. Its Pixel 7a-named bundle is CI-generated and is not physical-device acceptance.

| Run | Result | Artifact / evidence |
|---|---|---|
| Android `40810824093` | **FAIL** | Required ChatHistoryUxTest on API 26: 105/105 PASS. Subsequent non-required Fake Google/Gmail diagnostic stage ran 16 cases on API 26 and failed 5; observed failure classes include ambiguous Compose selectors, repeated `setContent`, Activity-recreation timeout, and account-switch handoff stale/wrong behavior. API 35 failed earlier in the Required cursor-position stage on `awaitTestImeSession` timeout; therefore API 35 Google/Gmail diagnostics are NOT_RUN. Process Death and Data Isolation pass on both APIs. |
| Full CI `40810824061` | PASS | All five platform jobs pass. |
| Native `40810824148` | PASS | All twelve Rust platform/compiler jobs pass; not Native Android inference. |
| SQLite `40810824022` | PASS | Both database producers and both cross-architecture consumers pass. |
| RustSec `40810824040` | PASS | Vendor and dependency audit pass. |
| Fast CI `40810824083` | PASS | Linux fast checks pass. |
| Git Hooks `40810824150` | PASS | Windows, Ubuntu, macOS pass. |

The Android artifacts are API 26 instrumentation `12872143095` (`sha256:ebdd9e7a9f5343e9dfacba91abed7ba75b4bf56a338fa35cbe3582160ae4f34e`), API 35 instrumentation `12872631908` (`sha256:bd32cb1a21e26a257c429bc74bd0158f41c819072af835204f953232dc4f8978`), aggregate emulator evidence `12872123755` (`sha256:bca7f51ff98bac3fdeaca337ea39244741e10ca66ceff3834ff737d637dfd98d`), unit JUnit `12871522166` (`sha256:845afe4c85c44988d5aac367c45efe01a0fd23f9c8a06934672f3477e686ac1e`), integration JUnit `12870038258` (`sha256:6106c85674cebced09b8e0c20c16cc1c876811615bfe1e93e8c5bd8a5af6cdaf`), security JUnit `12869939481` (`sha256:8669b235060a6af85cd968937287f8ab97862729005d7526f2c8c7ab4432edd8`), dependency audit `12869948133` (`sha256:d27187c8ce4cc9626df94afc7f17ae3c5d4431655c82c8444dc86d5f1f696324`), debug APK candidate `12870217950` (`sha256:c995926cf0fa3301cdba817908da593517478a3350e6dd47b2ff079578c158fd`), and the CI-only Pixel UI reliability bundle `12872890966` (`sha256:bf4939475a65c07ebb55de22cefe4afa6916fb4cc49bd63d0a0b6c276e872d7f`). Native and Full CI runs have no artifacts; SQLite artifacts are `12870847715` (`sha256:6031fd63139d6e0884b276725e94ef2eee88ca288fe69af8c3e02113a1c32e6b`) and `12870854559` (`sha256:de4a54e60bc9fe039ac674ea2b8a4ad51b5ae7b29bdd3df63ae18c4d7d4ceccd`); RustSec artifact is `12870267717` (`sha256:e8fc9236753795159ab41198da5124cd2ed07978d677c86abc7ae6ff6014a735`). Fast and Git Hooks have no artifacts.

This result is **not accepted as Android UI evidence**. The 5/16 API 26 diagnostic failures are executed test failures; selector and duplicate-content errors may be test-harness faults, while the account-switch stale/wrong handoff is a security-relevant product observation requiring Lane B triage against its exact failure trace. API 35 is a required-stage timeout before those diagnostics. The overall Android gate remains failed, not skipped or infrastructure-pending; Lane B owns the fix and exact-head rerun.

The separate `android-personal-beta.yml` push runs `40810819146` and `40811124717` fail the workflow's `main`-only guard. They are not PR CI evidence and do not change the above result.

### PR #147 RET-003 re-review and current runner state

For exact source `1945eafd0fa3be1068ac67972026dcc8b320b339` / tree `5229b8ea59fc537ba607bde517991328b615e016`, Lane C now **ACCEPTS the source/harness fix** for `NATIVE-PREFLIGHT-RET-003`. The preflight query uses `pm list packages -u` for both fixed target and test IDs, accepts only empty output, and rejects the exact package line, malformed nonempty output, or a failed query. Cleanup uses the installed-only result only to decide whether to call `adb uninstall`; it then requires the `-u` query to show no installed or retained record. The tests cover retained-only preflight data, retained records after uninstall, query errors, and malformed output. `bash -n` for the scripts, `bash tools/g8-local-model-native/test-preflight.sh`, `bash tools/g8-local-model-native/test-cleanup.sh`, and `python3 tools/g8-local-model-native/test-jni-contract.py` all pass locally. The static JNI contract reports eight exception-contained exports. No device or model was exercised by these local checks.

The exact-head focused run `40842179056` / attempt 1 is **CANCELLED**, job `123340927331` on runner `1000023549`; the owner agent confirms cancellation occurred before source checkout, fixture download, or inference. This run yields no test/build/runtime evidence. At this snapshot, exact-head Android `40842179113` has Change Scope, Dependency Audit, Security Tests, Unit, Documentation, and Integration PASS; Build/Lint/Instrumentation Compile remains in progress on `1000023556`. Full CI `40842179017` has five jobs in progress. Native `40842179073` has eleven jobs in progress and one queued without a runner. SQLite `40842179031` has two producers PASS and two consumers queued without a runner. Fast `40842179152`, RustSec `40842179048`, Git Hooks `40842179038`, and mock Beta contract `40842179047` PASS. All these exact-source runs are attempt 1. No retry was initiated. The Beta push workflow `40842172786` fails its `main`-only guard and is not PR validation.

### Public model metadata/license review

Lane C independently queried the pinned Hugging Face repository metadata and exact-revision file listings for all five added candidates; no weight files were fetched, no gated terms were accepted, and no runtime or quality claim is promoted from vendor data to a Serea result.

| Model | Metadata cross-check | Independent disposition |
|---|---|---|
| LFM3-1.7B | Exact GGUF revision lists Q4_K_M at 1,099,627,808 bytes; card identifies `lfm1.0`; Japanese language tag and llama.cpp instructions are present. | NOT_RUN; Serea compatibility is not established. The cited Liquid AI URL `https://www.liquid.ai/license` returns 404. Current official page is [LFM License](https://www.liquid.ai/lfm-license). It defines a $10,000,000-or-more annual revenue threshold and conditions commercial-use rights on not exceeding it. Lane A report's phrase “above US$10M” misses the exact boundary; docs correction requested. |
| Bonsai 2 4B | Exact revision lists PTQ1_0 949,549,920 bytes, PQ2_0 1,146,465,120 bytes, Q2_0 1,212,105,520 bytes, Apache-2.0, LICENSE and NOTICE. | Metadata matches Lane A. Keep Serea fixed llama.cpp format constraints and Q2_0 NOT_RUN distinction; no weights/runtime. |
| Qwen3.5-2B | Exact revision lists 4,548,221,488 bytes safetensors, Apache-2.0, model card claims 201 languages and tool use; no official GGUF in that pinned repo. | Metadata matches; current JNI GGUF route incompatible with this artifact until a separately reviewed conversion/backend exists. |
| Gemma 4 E2B | Exact official revisions list 2,671,733,574 bytes mobile QAT safetensors and 3,349,516,256 bytes official Q4_0 GGUF, plus the optional 986,833,664-byte projector; repository metadata and model card identify Apache-2.0. | Metadata matches; no execution. The GGUF artifact alone does not prove support in Serea's pinned llama.cpp. |
| FunctionGemma 270M | Exact gated revision lists 536,223,056 bytes safetensors and 288,440,320 bytes task-specific `tiny_garden.litertlm`; metadata is `license: gemma`, gated manually. Card positions the base model for task-specific function fine-tuning and says it is not a direct-dialogue model. | Metadata matches; no terms accepted and no runtime. Keep only a future isolated router candidate after terms and schema/approval/dispatch tests. |

The source cards and official metadata were checked on 2026-10-29 at the revisions pinned in `docs/agent-reports/g8n-native-bonsai-evaluation.md`. Direct API cross-checks: [LFM3 GGUF revision](https://huggingface.co/api/models/LiquidAI/LFM3-1.7B-GGUF/revision/bad5729bba61d076fff20d2548e62526665af215), [Bonsai 2 4B revision](https://huggingface.co/api/models/prism-ml/Ternary-Bonsai-2-4B-gguf/revision/8d6557168168f91ea6697a4a49ed2ef622b53adb), [Qwen3.5-2B revision](https://huggingface.co/api/models/Qwen/Qwen3.5-2B/revision/15852e8c16360a2fea060d615a32b45270f8a8fc), [Gemma 4 E2B Q4_0 GGUF revision](https://huggingface.co/api/models/google/gemma-4-E2B-it-qat-q4_0-gguf/revision/675cff42a74c774d6cb76f76d8eacb49b48c9b93), and [FunctionGemma revision](https://huggingface.co/api/models/google/functiongemma-270m-it/revision/39eccb091651513a5dfb56892d3714c1b5b8276c). Official [Gemma 4 license page](https://ai.google.dev/gemma/docs/gemma_4_license) and [LFM license text](https://www.liquid.ai/lfm-license) are cited in the model shortlist. These checks do not decide redistribution clearance. The standard model remains UNDECIDED, the Default Chat model remains unchanged, and Pixel 7a model acceptance is still required.

```text
LANE_C_REVIEW_UPDATE:
PR147_SOURCE_HARNESS=ACCEPTED at 1945eafd0fa3be1068ac67972026dcc8b320b339 / 5229b8ea59fc537ba607bde517991328b615e016
PR147_MODEL_LICENSE_REPORT=CHANGES_REQUESTED pending exact LFM threshold/canonical URL correction
PR146_FORMAL_CI=PARTIAL (all requested CI other than Android pass; Android Required gate fails; Identity skipped)
PR149_FORMAL_CI=PARTIAL (Android gate fails; six CI workflows pass; UI diagnostics fail/API35 diagnostics NOT_RUN)
PR149_ANDROID_UI=CHANGES_REQUESTED / NOT_ACCEPTED
MODEL_DEFAULT=UNDECIDED
PIXEL7A_ACCEPTANCE=NOT_RUN
```

## Review refresh — 2026-10-29 08:35 UTC

### PR #147 documentation re-review

Lane A's docs-only correction commit `36700f6c99d15db428c6d434c657ac76593df3f9` / tree `107824f5338ed9ec46d41f6836f5df9c6ad84cf7`, parent `1945eafd0fa3be1068ac67972026dcc8b320b339`, is **ACCEPTED** for report accuracy. The updated source cites Liquid AI's current official [LFM license page](https://www.liquid.ai/lfm-license), accurately states the US$10M-or-more threshold, and classifies the 40811065474 and 40842179056 outcomes without claiming inference. It does not modify code/tests. At the time of this refresh it is a local, accepted successor awaiting Lane A's push; the remote PR #147 head remains the accepted code/harness SHA `1945eafd…` until that push. One bounded inference attempt is still outstanding and no previous run is reused as inference evidence.

### PR #151 combined G9 candidate review and exact-head CI

PR #151 is OPEN Draft, head `d4a36fb5d3613bca2d60151de0991a1baac72fc9` / tree `b78a84b60ef5a2d344eabdda85b1154e62be46d7`, based on PR #149's exact `3a0f0c1897f2115549e7682d6fcc1783c44604f3` / tree `6a1f4d382907a7deb7d6dbf8147e6490aa054e81`. Lane C **ACCEPTS** the scoped Gmail handoff source/test integration and its source-bound local evidence. Product/test code at `GoogleTrustedHandoff.kt` and `GoogleTrustedHandoffTest.kt` is byte-identical to the previously accepted `4e4b8f9d0aad80f330d1c93364b3cbfda5369bcd`; PR #151 introduces no shared Composition Root or `MainActivity` changes. The body-only path excludes message/thread IDs, sender/recipient, subject, timestamps, snippet, HTML raw source, and handoff security metadata from `localChatText()`. Plain body is preferred; parser-produced inert HTML text is the fallback. Existing account/session/generation/content identity/revocation/consent/single-use checks and Google-origin Cloud egress denial remain in force.

Both source-bound evidence manifests verify with `sha256sum -c`. The exact candidate XML at `e2010d64445b136616127448386dd2e9789c227e` / tree `4526fb690b1dc17c41bc156ac7ac24f250042276` contains 22 tests, 0 failures, 0 errors, and 0 skipped; XML SHA-256 is `17d8316ef8b26427cb0b551410f2fc65440f1cc6b50f351c4689801ff2324197`. The test files are unchanged between the accepted product/test source `4e4b8f9…` and the tested evidence commit `e2010d6…`. This is local JVM-only evidence, not GitHub CI or connected Android UI evidence.

At 08:35 UTC, all formal #151 runs identify the exact head above, attempt 1. Fast CI `40842824206` PASS (runner `1000023564`). Git Hooks `40842824356`: Windows and Ubuntu PASS (`1000023568`, `1000023565`); macOS remains QUEUED without runner. Android `40842824215`: Change Scope PASS (`1000023569`); Build/Lint/Instrumentation Compile IN_PROGRESS (`1000023572`); Documentation, Dependency Audit, Unit, Integration, and Security Tests QUEUED without runner. Full CI `40842824328`: Linux stable, Linux MSRV, and Windows synthetic Gate C IN_PROGRESS (`1000023571`, `1000023566`, `1000023574`); both macOS jobs QUEUED. Native `40842824208`: stable Windows x86_64, stable Linux x86_64, stable Windows ARM64, and Rust 1.85 Windows ARM64 IN_PROGRESS (`1000023575`, `1000023567`, `1000023570`, `1000023573`); eight matrix jobs QUEUED. SQLite `40842824221` has both producers QUEUED, RustSec `40842824232` is QUEUED. These are runner waits/in-progress states, not failures or passes. No reruns were triggered. The separate main-only Personal Beta guard `40842785134` fails outside PR validation.

```text
PR147_REVIEW=ACCEPTED for source/harness 1945eafd0fa3be1068ac67972026dcc8b320b339 / 5229b8ea59fc537ba607bde517991328b615e016
PR147_REPORT_CORRECTION=ACCEPTED for 36700f6c99d15db428c6d434c657ac76593df3f9 / 107824f5338ed9ec46d41f6836f5df9c6ad84cf7; pending push at snapshot
PR147_NATIVE_INFERENCE=NOT_RUN
PR151_SCOPED_GMAIL_SOURCE_TEST_EVIDENCE=ACCEPTED at d4a36fb5d3613bca2d60151de0991a1baac72fc9 / b78a84b60ef5a2d344eabdda85b1154e62be46d7
PR151_FORMAL_CI=PARTIAL / IN_PROGRESS / QUEUED at exact head; no Android UI result
PR149_BASE_API26_UI=16 diagnostic cases, 5 failures; PR149_BASE_API35_UI=NOT_RUN after required-stage timeout
MODEL_DEFAULT=UNDECIDED; PIXEL7A_ACCEPTANCE=NOT_RUN
```

## Runner follow-up — 2026-10-29 08:36 UTC

Agent A pushed the accepted PR #147 report correction. The exact remote head is now `36700f6c99d15db428c6d434c657ac76593df3f9` / tree `107824f5338ed9ec46d41f6836f5df9c6ad84cf7`; it has the already-reviewed code/harness parent `1945eafd0fa3be1068ac67972026dcc8b320b339` / tree `5229b8ea59fc537ba607bde517991328b615e016` and changes only the report.

The single authorized bounded exact-head focused Native run is `40843204437`, attempt 1, job `123344314662` on assigned runner `1000023588`, currently IN_PROGRESS. No inference result is available yet. On the same source, Personal Beta Contract `40843204470` passes; Fast `40843204610` and Git Hooks `40843204431` are queued; Android `40843204488` is pending; RustSec `40843204440` is in progress; Full CI `40843204630` is queued; Native platform `40843204674` is queued; SQLite `40843204485` has both macOS producers in progress on runners `1000023581` and `1000023589`. The separate main-only Beta guard `40843196451` fails outside PR validation.

PR #151 remains on exact head `d4a36fb5d3613bca2d60151de0991a1baac72fc9`. Since the prior snapshot, Full CI `40842824328` and RustSec `40842824232` have started; both remain in progress. Android `40842824215` has only Change Scope complete, Build/Lint/Instrumentation Compile in progress; other Android jobs remain queued. Native `40842824208` has four assigned jobs in progress and eight queued. SQLite `40842824221` has both producers queued. Fast `40842824206` passed; Windows and Ubuntu Git Hooks jobs passed while macOS remains queued. No failed required PR #151 job is present yet; queued jobs have no runner assignment and remain NOT_RUN.

```text
PR147_CURRENT_HEAD=36700f6c99d15db428c6d434c657ac76593df3f9 / 107824f5338ed9ec46d41f6836f5df9c6ad84cf7
PR147_EXACT_NATIVE_RUN=40843204437 attempt 1 IN_PROGRESS runner 1000023588
PR147_NATIVE_INFERENCE=NOT_RUN_PENDING_RUN
PR151_FORMAL_CI=IN_PROGRESS_AND_QUEUED; no exact-head Android UI evidence
```

## Lane B test-only successor review — 2026-10-29 08:43 UTC

Lane C **ACCEPTS** the unpushed PR #151 successor's test changes at `b553d68325b7c18dfbecaea2a9e92ebd15adad27` / tree `c494e20d34a961d16d67c4c882bc2eeb7b3b70f0`, base exact #151 head `d4a36fb5d3613bca2d60151de0991a1baac72fc9` / tree `b78a84b60ef5a2d344eabdda85b1154e62be46d7`. Its report-only commit is `5df5b9b48a94e52cfafafd4c34dfbba06811b2f2` / tree `08a7d7341ad5208564f6dcec128fc0a74b0a68a1`. Lane C independently verified the commit parents, changed file list and `git diff --check`.

The patch touches only four AndroidTest files: it replaces ambiguous consent/account text selectors with stable tags, uses `createComposeRule()` in the test that owns `setContent`, changes an account-label assertion to substring matching, and queries the fixture event's local date by converting its fixed instant through the device `ZoneId`. The app-shell integration test asserts the expected `demo-primary` calendar is selected, and timeout messages include the device zone/date. No app source, Composition Root, MainActivity, required test identity/count, or gate predicate changes. The Loopback UI remains synthetic and bound to MockWebServer; no real OAuth or provider data is used.

The recorded exact-source instrumentation compile passes on code SHA `b553d68…` / tree `c494e20…`. API26/35 UI JUnit remains **NOT_RUN** for this successor because no local device is available. The report correctly says the PR #149 API26 date timeout's root cause remains uncertain; the timezone-based query is a bounded test hypothesis, not a confirmed product fix. API35 remains blocked by the separate IME diagnostic failure from base evidence. The exact successor needs new emulator CI before the affected UI tests can be called passing; no PR #149 artifact or outcome transfers to it. The successor is unpushed as of this checkpoint; remote PR #151 remains at `d4a36fb…`.

```text
LANE_B_GMAIL_SOURCE_TEST=ACCEPTED at 4e4b8f9d0aad80f330d1c93364b3cbfda5369bcd / b19a6c52a8e8d4ff1c8365d435c047d0aceb300b
LANE_B_G9_UI_TEST_PATCH=ACCEPTED at b553d68325b7c18dfbecaea2a9e92ebd15adad27 / c494e20d34a961d16d67c4c882bc2eeb7b3b70f0
LANE_B_G9_UI_JUNIT=NOT_RUN
PR149_API26_DIAGNOSTIC=5/16 FAIL; date-timeout cause UNCONFIRMED
PR149_API35_GOOGLE_UI=NOT_RUN after required IME diagnostic failure
```

## Native exact-run closure and PR #153 IME test review — 2026-10-29 08:50 UTC

### PR #147 exact Native run result

GitHub still reports PR #147 OPEN/Draft at exact head `36700f6c99d15db428c6d434c657ac76593df3f9`, tree `107824f5338ed9ec46d41f6836f5df9c6ad84cf7`, based on G7 `0560e1e7bcd13f0194eab8caca89d70ea5a2f142`. The single bounded exact-head run `40843204437`, attempt 1, completed **FAILURE** at 08:46:32 UTC. Job `123344314662` ran on assigned runner `1000023588`. The focused JVM contracts, both ABI APK builds, license metadata check (without model weights), pinned fixture size/hash check, and disposable API 35 x86_64 Emulator setup all passed. The product adapter/inference step started one Android test and failed before reaching model inference with `initializationError`: `RuntimeException: Failed to instantiate test runner class androidx.test.internal.runner.junit4.AndroidJUnit4ClassRunner`. The available log does not include a cause, so dependency/classloader, harness arguments, and emulator image remain unconfirmed possibilities; no specific build or JNI root cause is established. No inference, generation, lifecycle, cancellation, or runtime evidence was produced.

The exact run exposed cleanup behavior: the target package was absent while an app-private model file was still classified as owned (`NOT_VERIFIED_OWNER_PACKAGE_ABSENT`), so app-private final state remained NOT_VERIFIED and cleanup failed closed. The host model and temporary fixture were verified absent; target/test packages were verified absent with retained-package checks. Evidence upload was skipped after the failed test. Diagnostic artifact `12883594935` is 5,881 bytes, digest `sha256:e4ed10a1ec5d14a38bbf2e861b30b196d7e3b8e0e325dc1869df3c3c63760cb1`; its download endpoint returned HTTP 403 in the attempted retrieval. This is a pre-inference test-runner initialization failure with root cause unconfirmed, not a Native Runtime pass. No retry was launched.

### PR #153 independent review and CI snapshot

Lane C **ACCEPTS** the test-only PR #153 change at exact source `6f4c8c5b4caf8f48619234fee5a9c864cb25766`, tree `8a17da2ba3003d8663a3c6734d444682609abe05`, parent/base exact PR #151 `d4a36fb5d3613bca2d60151de0991a1baac72fc9` / tree `b78a84b60ef5a2d344eabdda85b1154e62be46d7`. PR #153 is OPEN/Draft and its remote head matches that SHA. The only file change is AndroidTest `SereaAppShellTest.kt` (+23 lines). After the existing Activity-recreation/draft-restoration wait, the helper waits up to 10 seconds for the Activity to be RESUMED and its decor to be attached, shown, and window-focused. It fails closed on timeout and logs the observed lifecycle/window state. The existing not-focused and selection-preservation assertions remain, followed by explicit RequestFocus, focused assertion, a fresh real Test IME session, commitText, and insertion/content/selection assertions. The helper reads lifecycle and window state on the ActivityScenario UI thread and does not mutate app state. No Required test identity, assertions, gate, production code, or owner boundary is weakened. `git diff --check` passes. Local compile/UI execution is **NOT_RUN** because the available Java installation lacks `javac`; this source review is not Android runtime evidence.

At 08:50 UTC, exact PR #153 Android run `40844961565` attempt 1 is queued with all its jobs queued and no runner assigned: Change Scope, Documentation, Dependency Audit, Build/Lint/Instrumentation Compile, Unit, Integration, Security, and Storage/Emulator. Full CI `40844961537`, Native `40844961372`, SQLite `40844961545`, RustSec `40844961783`, Fast `40844961637`, and Git Hooks `40844961944` are also queued without runner assignment; no artifact IDs exist yet. These are runner waits, not test failures or passes. No retry was initiated. The separate PR #151 predecessor CI remains bound to exact `d4a36fb…`: Android run `40842824215` has Change Scope, Docs, Dependency Audit, Build/Lint/Instrumentation Compile PASS, while Unit, Integration, Security, and Storage/Emulator are queued; Native `40842824208` has Linux x86_64, Linux ARM64, and Rust Linux x86_64/ARM64 PASS, four Windows/macOS jobs in progress and two macOS jobs queued; SQLite `40842824221` has the Intel producer PASS and three jobs queued; Full CI `40842824328`, RustSec `40842824232`, and Fast `40842824206` PASS; Git Hooks `40842824356` has Windows and Ubuntu PASS with macOS queued. The Android UI/Required runtime evidence has not run on either exact PR #151 or #153 candidate. The separate beta main-only guard is not PR validation.

```text
PR147_EXACT_NATIVE_RUN=40843204437 attempt 1 FAILURE before inference; root cause UNCONFIRMED; no retry
PR147_NATIVE_RUNTIME=NOT_RUN
PR153_TEST_SOURCE_REVIEW=ACCEPTED at 6f4c8c5b4caf8f48619234fee5a9c864cb25766 / 8a17da2ba3003d8663a3c6734d444682609abe05
PR153_ANDROID_CI=QUEUED without runner at snapshot; all exact-head Android tests NOT_RUN
PR151_ANDROID_UI=NOT_RUN; PR151_FORMAL_ANDROID=PARTIAL / runner-queued
MODEL_DEFAULT=UNDECIDED; PIXEL7A_ACCEPTANCE=NOT_RUN
```

### PR #153 runner refresh — 2026-10-29 08:50:55 UTC

The PR #153 snapshot had Android run `40844961565` queued with its Change Scope job `123350084172` assigned no runner (`runner_id=0`); every Android build/test/UI check was NOT_RUN. Full CI `40844961537` had macOS Intel job `123350084681` in progress on runner `1000023630`; the other four jobs were queued. Native `40844961372` had Rust Windows x86_64 job `123350083671` and Rust Windows ARM64 job `123350083666` in progress on runners `1000023634` and `1000023635`; the other ten jobs were queued without assignment. SQLite `40844961545`, RustSec `40844961783`, Fast `40844961637`, and Git Hooks `40844961944` jobs were queued. No job had failed or produced artifacts at this snapshot. The runner inventory endpoint `GET /repos/Yrika819/Serea/actions/runners` returned HTTP 403 `Resource not accessible by integration`, so Lane C cannot assess the available hosted or self-hosted pool beyond per-job assignment data. Do not infer overall capacity or rerun; this was queue/in-progress, not product failure.

## PR #153/#154 workflow monitoring — 2026-10-29 08:58:26 UTC

### PR #153 current exact-head run state

PR #153 remains OPEN/Draft at the already-reviewed test-only source `6f4c8c5b4caf8f48619234fee5a9c864cb25766` / tree `8a17da2ba3003d8663a3c6734d444682609abe05`, based on PR #151 exact `d4a36fb5d3613bca2d60151de0991a1baac72fc9`. All seven runs remain attempt 1 and bind to that source. Android `40844961565` has Change Scope `123350084172` queued with `runner_id=0`; no Android UI or JUnit evidence exists. Full CI `40844961537` has macOS Intel job `123350084681` in progress and four queued. Native `40844961372` has stable Windows x86_64 `123350083841`, Rust Windows x86_64 `123350083671`, and Rust Windows ARM64 `123350083666` in progress; the other nine matrix jobs are queued. SQLite `40844961545` has Intel producer `123350084513` PASS (completed 08:53:42 UTC) and artifact `12884169231`, `portable-db-intel`, 36,954 bytes, SHA-256 `d09e3f099123fa405dc4a999bfcfc1673828fe32bfcc6fcb0df7640c52b40e74`; ARM producer and the cross-architecture consumer remain queued. RustSec `40844961783`, Fast `40844961637`, and Git Hooks `40844961944` remain queued, with no artifacts. These are runner waits/in-progress states, not failures. No reruns were initiated; Native inference was not retried.

### PR #154 exact-head docs checks

PR #154 is OPEN/Draft at head `90913b4d4e3d834ae96d4f7e842bddfbd2b61ff6` / tree `3f88b0800971de9474a63c0286959aa94074613a`, based exactly on PR #151 `d4a36fb5d3613bca2d60151de0991a1baac72fc9`. The candidate is documentation-only. All seven formal checks are attempt 1, bound to that exact head, queued without runner assignment and have no artifacts: Android `40845836065`, SQLite `40845836238`, Fast `40845836195`, Full CI `40845836135`, Git Hooks `40845836204`, Native platform `40845836032`, and RustSec `40845836070`. No exact-head docs check has completed; keep the PR Draft. The runner inventory remains inaccessible with HTTP 403, so no pool-capacity inference is supported.

### PR #151 CI debug APK candidate artifact

The exact #151 Android run `40842824215` exposes CI artifact `12883404140` named `serea-android-pixel7a-debug-apk-candidate`, size 33,163,400 bytes, archive SHA-256 `fccaf9a97a606ea3cb79b5805e04c026ec146b1fc0fdd0242e7d9239ae66966a`. Its producing job is Build/Lint/Instrumentation Compile `123343498034`, on the exact #151 head `d4a36fb5d3613bca2d60151de0991a1baac72fc9`. The workflow records `source_commit`, checkout commit, run/attempt, package ID, APK SHA-256, and `apksigner` certificate output in `provenance.txt`; it labels this a debug APK candidate. Artifact download via `gh run download` and the job-log API both returned HTTP 403 from GitHub blob storage. The archive digest is recorded, but the APK's inner SHA-256 and signer could not be verified from the artifact. This is a CI build artifact only; it is not Pixel 7a acceptance, signing approval, or Beta evidence.

## PR #153/#154 and release artifact refresh — 2026-10-29 08:59:54 UTC

PR #153 remains OPEN/Draft at exact head `6f4c8c5b4caf8f48619234fee5a9c864cb25766` / tree `8a17da2ba3003d8663a3c6734d444682609abe05`, based on PR #151 `d4a36fb5d3613bca2d60151de0991a1baac72fc9`. All seven formal runs are attempt 1 and exact-head-bound. Android `40844961565` has only Change Scope job `123350084172` queued without a runner; all Android build, instrumentation, API26/35, and JUnit evidence remains NOT_RUN. Full CI `40844961537` has macOS Intel job `123350084681` in progress and four queued jobs. Native `40844961372` has Stable Windows x86_64 `123350083841`, Rust Windows x86_64 `123350083671`, and Rust Windows ARM64 `123350083666` in progress; nine jobs remain queued. SQLite `40844961545` has Intel producer `123350084513` PASS and artifact `12884169231` (`portable-db-intel`, 36,954 bytes, SHA-256 `d09e3f099123fa405dc4a999bfcfc1673828fe32bfcc6fcb0df7640c52b40e74`); ARM producer and consumer are queued. RustSec `40844961783`, Fast `40844961637`, and Git Hooks `40844961944` remain queued. No failure or retry occurred. The SQLite producer pass is not Android or Native inference evidence.

PR #154 remains OPEN/Draft at exact head `90913b4d4e3d834ae96d4f7e842bddfbd2b61ff6` / tree `3f88b0800971de9474a63c0286959aa94074613a`, based exactly on PR #151 `d4a36fb5d3613bca2d60151de0991a1baac72fc9`; its change is documentation-only. All seven attempt-1 formal runs are bound to this exact source and queued without runner assignment or artifacts: Android `40845836065`, SQLite `40845836238`, Fast `40845836195`, Full CI `40845836135`, Git Hooks `40845836204`, Native platform `40845836032`, and RustSec `40845836070`. No exact-head documentation check has completed; keep it Draft. The runner inventory endpoint remains unavailable with HTTP 403; no runner-capacity inference is made.
