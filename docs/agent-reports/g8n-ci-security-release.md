# G8N CI, Security, and Release Evidence

**Agent:** C — CI / Security / Release
**Captured:** 2026-10-29 02:49 UTC
**Status:** G8 Formal CI is **PARTIAL**. Android Required is **FAILURE** on the emulator harness; Full CI, Native, and SQLite still have queued macOS jobs. Release/Owner decisions are prepared, not approved.
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

### API 26/35 instrumentation and lifecycle evidence

Both emulator campaigns recorded their device API level and completed the isolated Process Death/data-isolation harness, but the bounded `cursor-position-focused` diagnostic stage stopped before any instrumentation test started:

| API | Required instrumentation | Diagnostic result | Data Isolation / Sentinel | Actual OS Process Death | Artifact |
|---|---|---|---|---|---|
| 26 | `NOT_RUN`; 0/105 started and finished; no JUnit files; zero recorded test failures because no tests ran. | FAIL at `run_diagnostic_stage`: `could not query isolated package state; refusing to install into an unverified emulator.` Gradle exit/status `NOT_RUN`. | PASS; demo history and `model_selection` sentinel seeded and verified without demo history mutation. | PASS; harness logs explicitly say force-stopped the isolated app without clearing data, recovered encrypted history/Usage Ledger state, completed Delete All, and preserved Demo sentinel. Host process-death exit 0. | `serea-android-instrumentation-api26`, artifact `12869216935`, 9,044 bytes, digest `sha256:d08309fde30db9fb67957bb1631ed5aba7c68f0f8d929f332da240a862c37afd`. |
| 35 | `NOT_RUN`; 0/105 started and finished; no JUnit files; zero recorded test failures because no tests ran. | FAIL at the same package-state query before Gradle. `api-result=FAIL`; Gradle exit/status `NOT_RUN`. | PASS; Demo sentinel seed/verify passed. | PASS; force-stop/recovery, encrypted history/Usage Ledger, deletion, and sentinel preservation passed. Host process-death exit 0. | `serea-android-instrumentation-api35`, artifact `12868798855`, 9,090 bytes, digest `sha256:06ed97683b5d292c2f87c0d47f1eebacddc175f7e4944530f751abe8d10134c3`. |

The aggregate `serea-android-emulator-evidence` artifact is `12869770024` (11,853,573 bytes; digest `sha256:fa8426a3e202ad445d4b3a5b00c021e7c2c3888fe74b8d28e05dbbc4d2049fad`). The CI-only Pixel 7a reacceptance bundle artifact is `12868893543` (45,057,882 bytes; digest `sha256:95e8ec9c7d5fc46e7e16fb33f8b8703afe8daa5e8d2d2d1a2cc6342002621155`); its workflow job passes, but physical Pixel 7a acceptance remains NOT_RUN.

The script's `android_package_path()` treats any nonzero `adb shell pm path` result as a query failure, while stderr is suppressed. The failure happened before instrumentation and before the Gradle instrumentation diagnostic command, for the not-yet-installed isolated target/test packages. The artifact does not preserve the original package-manager stderr/exit details, so the exact `pm`/ADB subcause is unconfirmed; do not claim that 105 product tests failed or that artifact upload omission caused this. Evidence classifies the Required failure as an emulator test-harness preflight failure. A fix and regression coverage are being handled separately from PR #146 by the parent on a successor branch.

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
DRAFT_PR: report branch's Draft PR (recorded by GitHub)
COMPLETED_WORK: Exact PR #146 CI/API evidence snapshot; CI artifact hashing and JUnit counts; PR #144 current and historical native-build failure classification; Owner decision and release readiness packet.
CHANGED_FILES: docs/agent-reports/g8n-ci-security-release.md
TEST_COMMANDS: none (evidence/report lane; no code tests were requested or needed)
JUNIT_RESULT: PR #146 Android unit 405/405, integration 22/22, security 26/26; API 26/35 instrumentation NOT_RUN (preflight FAIL, 0/105 each); Process Death/Data Isolation/Sentinel PASS on both
CI_RUN_IDS: 40804694025, 40804693907, 40804693934, 40804693978, 40804693889, 40804693992, 40804693919, 40804722382, 40804722157; native PR #144 40803623345 and historical 40800987367
ARTIFACT_IDS: 12869510041, 12869310588, 12868263908, 12869480087, 12868174398, 12869265751, 12867589033, 12867015936
SECURITY_IMPACT: Documentation only; preserves Google-origin egress prohibition, debug-only release boundary, and no production signing.
REVIEW_STATUS: Pending parent review
KNOWN_ISSUES: PR #146 API26/35 instrumentation did not start because the harness preflight package-state query failed; Process Death/Data Isolation/Sentinel passed separately. Identity was skipped. Full CI/Native/SQLite macOS jobs still queued. Current PR #144 native build failure lacks an uploaded diagnostic artifact and root cause is unknown.
CURRENT_BLOCKERS: macOS runner availability; current-head PR #144 Native failure log; parent-owned Android package-query harness fix; Owner decisions on package/signing/OAuth/providers/age-region eligibility.
NEXT_ACTION: Commit/push report checkpoint; monitor finite current Actions runs; independently review A/B implementation once available.
OWNER_APPROVAL_REQUIRED: Package ID/coexistence, certificate/channel, OAuth scopes, provider/audience/region, model choice and device acceptance.
```
