# G8 Closure → Native AI → G9 → G10 Campaign Ledger

**Checkpoint:** initial snapshot 2026-10-29 UTC 02:42; latest addendum 03:00 UTC
**Roadmap:** `Serea_Roadmap_v4.md`
**Repository:** `Yrika819/Serea`
**Campaign status:** G8 Evidence Closure **PARTIAL**; Native Runtime **PARTIAL**; Bonsai 2 Evaluation **PARTIAL**; G9 Functional Candidate **PARTIAL**; G10 Preflight **PARTIAL**; Personal Beta **NO-GO**.

This checkpoint records exact-source evidence only. Existing work is preserved; PR #144 and #146 have not been modified. No OAuth, real Google data, Cloud AI credentials/billing, Owner signing key, physical Pixel 7a, protected-main merge, or distribution was used.

## GitHub authority and source identity

| Ref | GitHub state at campaign start |
|---|---|
| Protected `main` | `e0acb0254820c381dd97828824bef86a1db5176c`, tree `4955555be75c5742796740a3283d1a521e0d4806` |
| PR #134 | Known G5 HEAD `b84feb337cd57918346ab31cb3ea4af4ba880ffb` |
| PR #138 | Known G6 HEAD `23b884cf2815a2f3287652d5ffd2c3913d09df76` |
| PR #139 | Branch `codex/serea-g7-calendar-readiness`, current HEAD `0560e1e7bcd13f0194eab8caca89d70ea5a2f142` |
| PR #144 | OPEN/DRAFT, `codex/serea-g8-native-local-runtime`, HEAD `07ea5c6b460cea2eb1ccccc49ccd4f74dc65bb59`, tree `ff721fc7ae7642cbaf226376c23df6541d7c8248` |
| PR #146 | OPEN/DRAFT, `codex/serea-g8-integration-candidate-20261029-r1`, base branch `codex/serea-g7-calendar-readiness`, HEAD `d444e7cc04c8f1c4c6d12b1412cfe3a76258af66`, tree `25e2bb481a0b25bfed89d6ee25f1812316703b8e` |
| Local G8 implementation reference | User-provided tree `eba68ebff6f1be05a1cc16c5043edc1c086f62cc`; not a GitHub commit object (tree ID, not commit SHA) |

Protected `main` matches the supplied authority. Existing campaign branch names were checked on the remote; the new `codex/serea-g8n-*` names were absent. The local base clone began clean on `main` at the protected SHA. The parent candidate is a new branch based on exact PR #146 HEAD; this is a stacked Draft candidate, not a rewrite of #146.

## Agent and worktree assignments

| Lane | Assignment | Worktree / branch | Base SHA | Ownership boundary |
|---|---|---|---|---|
| Parent | Ledger, integration authority, final review/CI | `/workspace/serea-integration` — `codex/serea-g8n-integration-candidate` | `d444e7cc04c8f1c4c6d12b1412cfe3a76258af66` | Shared composition, integration, release configuration, and this ledger |
| A — Native AI / Bonsai 2 | PR #144 failure analysis, isolated runtime, model and compatibility evidence | `/workspace/serea-agent-a` — `codex/serea-g8n-native-runtime` | `07ea5c6b460cea2eb1ccccc49ccd4f74dc65bb59` | Native/adapter-only; no default-model switch or shared composition edits |
| B — Android Product / Google / Gmail | Fake-only UI, Gmail safety, lifecycle and data minimization | `/workspace/Serea-gmail` — `codex/serea-g8n-google-gmail-ui` | `d444e7cc04c8f1c4c6d12b1412cfe3a76258af66` | Gmail/Google feature and test files; no shared composition edits |
| C — CI / Security / Release | Formal CI evidence recovery, policy/release packet, independent reviews | `/workspace/Serea-C` — `codex/serea-g8n-security-release`, PR #148 Draft | `0560e1e7bcd13f0194eab8caca89d70ea5a2f142` | Read-only evidence/reports; no product code |

These are three actual child agents. No result is accepted solely from its author's report; source, tests, provenance, licensing and integration safety remain subject to independent review.

## Phase 0 — G8 evidence recovery

All supplied PR #146 runs are attempt 1 and resolve to exact HEAD `d444e7cc04c8f1c4c6d12b1412cfe3a76258af66`.

| Workflow | Run | State at 02:40 UTC | Job / artifact evidence |
|---|---:|---|---|
| Android | `40804694025` | IN_PROGRESS | Change Scope, Security, Integration, Docs, Dependency Audit passed. Build/Lint/Instrumentation Compile and Android Unit were running on assigned `ubuntu-24.04` runners. Integration JUnit artifact `12869310588`, SHA-256 `62d700d4c25ac1ef0c9ddc3645586028a2347465c42370e16a3153ab108a0355`; Security JUnit `12868263908`, SHA-256 `6c757e8b54d382fc0e9734cd4fe6be6e892cd8d8408a96fac0dd9f949fe9ef12`; dependency audit `12868532793`, SHA-256 `32e9947f6dc9497a516ae626b14f2d33b8fee341c9e06d8a00e05ca04162ec01`. API 26/35 Required, Identity, Process Death, Data Isolation, Sentinel and their JUnit evidence were not yet confirmed. |
| Full CI | `40804693907` | IN_PROGRESS | Windows P7 Gate C, MSRV Linux and stable Linux were assigned/running. macOS ARM64 and Intel jobs were QUEUED with no runner assignment. No conclusion yet. |
| Native platform validation | `40804693934` | IN_PROGRESS | Linux ARM64/x86_64, stable macOS ARM64 and stable Windows x86_64 passed. Windows ARM64, stable macOS x86_64, MSRV Linux x86_64 and Windows x86_64 were running. MSRV macOS ARM64 was QUEUED without a runner. This workflow validates the Rust native matrix; it is not evidence for Android JNI inference. |
| SQLite portability | `40804693978` | IN_PROGRESS | macOS ARM64 DB production passed; artifact `12869265751`, SHA-256 `a24f408ba84461818e3f4bb915977ad699900c54ac5dc18416009e3d305be039`. macOS Intel DB production and ARM64→Intel consumption were QUEUED without runner assignment. |
| RustSec | `40804693889` | SUCCESS | Artifact `12867589033`, SHA-256 `0f360ef81a5ab03efc14fecf99197c758df0be54cd48cd058935caa11078bee7`. |
| Fast CI | `40804693992` | SUCCESS | Linux fast checks passed; no artifact. |
| Git Hooks | `40804693919` | SUCCESS | Windows, Ubuntu and macOS jobs all passed; no artifact. |
| Release Security | `40804722382` | SUCCESS | Mock release contract + APK-bound lockfile SBOM artifact `12868174398`, SHA-256 `17ad843799fdde28e42a9ea099d1c9d571e8f6d093b958a1ee17a464037c40a7`. CI-only debug APK and mock signing only. |
| Mock Beta Contract | `40804722157` | SUCCESS | One mock signing/Firebase contract job passed; no artifact. |

Pending jobs above had no assigned runner where stated; they are infrastructure queue delays, not product failures. No workflow was rerun at this checkpoint. There is no same-HEAD Android API 26/35 Required result, Identity result, Process Death/Data Isolation/Sentinel result or instrumented Gmail/Calendar UI result yet.

**Android update at 02:41 UTC:** `Android Build, Lint, and Instrumentation Compile`, `Android Unit`, Integration and Security all completed SUCCESS. Integration artifact `12869310588` contains `OllamaCloudAdapterTest` 22/22 (0 failures, errors, skips). Security artifact `12868263908` contains `SecurityPortsTest` 10/10 and `GmailSecurityPortsTest` 16/16 (0 failures, errors, skips). `Android Storage and Emulator` is now running on assigned runner `GitHub Actions 1000023241`; API 26/35 Required, Identity, Process Death, Data Isolation and Sentinel evidence are pending. The Full/Native/SQLite workflow runs remain uncompleted; queued jobs without `runner_name` have not been classified as source failures.

## PR #144 native failure analysis

Latest exact-head PR checks include Native focused run `40803623345`, attempt 1, on `07ea5c6b460cea2eb1ccccc49ccd4f74dc65bb59`. The job failed at `Build opt-in product JNI APKs for both supported ABIs` while invoking `:app:assembleIsolated :app:assembleIsolatedAndroidTest`; the build step returned exit 1. It expected pinned llama.cpp `276378515ceebee5d14bcc8536603dd5c87491a2`, model revision `1076b85d135017971845b19b8d07df1479155988`, SHA-256 `e3131339bf4e8065265593d4fd8f7bb7ff2d3abff1edb5618aa1197b89cad9f5`, size 101,016,128 bytes.

The workflow redirected Gradle output to `evidence/g8-local-native/gradle-native-build.log`, but no artifact was uploaded for the failed run. Therefore the observable category is a **native APK assembly/build failure**; JNI compiler, CMake, NDK/ABI, llama.cpp API, link, packaging and test-harness subcause is **UNCONFIRMED**. The output recovery path is a harness/evidence gap. Do not call this a proven JNI or CMake defect. Lane A is adding recoverable failure diagnostics and will use a new source SHA for any subsequent CI evidence.

Historical failure `40800987367`, attempt 1, is separate: source `082d223d9975492b5dab3c70349f0d6e9fc504de`, tree `c25df6797e0b00946afb6ee5c8893521bacc0ca6`, artifact `12867015936` with SHA-256 `d8d4ff9c94c387ad8ca9bcd9d4cc179bb95d1999ef109a16a0820780a55683a3`. Its recovered Gradle log shows CMake Release configuration/build completed and the isolated target APK assembled for arm64-v8a and x86_64, then `:app:compileIsolatedAndroidTestKotlin` failed at `LocalModelNativeRuntimeE2ETest.kt:126:80`: unresolved `awaitReleased` on `LocalModelInferenceBackend`. This is a **Test Harness compile failure at the older source**, not evidence of the cause at current PR #144 HEAD. The artifact contains a separate JVM test `LocalModelNativeRuntimeTest#defaultProductBuildFailsClosedBeforeLoadingNativeLibrary` (1/1 PASS); this does not establish JNI inference.

Run `40803623345` is the G8 Native Runtime Focused non-required workflow, separate from Rust `40804693934`. No x86_64 inference evidence can be inferred from either run's name or build failure.

## Current implementation progress and gates

| Milestone | Status | Evidence / remaining gates |
|---|---|---|
| G8 Evidence Closure | PARTIAL | PR #146 exact source verified; 5 auxiliary runs passed. Android build, unit, integration and security jobs passed; `Android Storage and Emulator` has started, but API 26/35 Required and lifecycle evidence is pending. Full/Native/SQLite still have pending runner work. |
| Native Runtime | PARTIAL | PR #144 adapter/build candidate exists but latest native APK build failed; log artifact is absent; new JNI inference, two-turn generation, lifecycle, cancellation, memory release and x86_64/ARM64 execution remain unproven. |
| Bonsai 2 Evaluation | PARTIAL | Prior source reports cover SmolLM2 metadata/license baseline only. Bonsai 2 2B/4B/9B formats, quantization, compatible backend, Android runtime, performance and Japanese/tool quality still require verified public-source and measured evidence. |
| G9 Functional Candidate | PARTIAL | Existing PR #146 Fake Calendar and consent foundation is under CI. Lane B has identified and is repairing the Gmail handoff metadata-minimization path; Android UI tests and lifecycle/process-death evidence remain incomplete. |
| G10 Preflight | PARTIAL | CI-only mock signing/SBOM and mock Firebase/signing contract passed on #146; native inventory, complete APK/SBOM provenance, migration and rollback plan, owner runbooks and Pixel acceptance are pending. |
| Personal Beta | NO-GO | Requires all P0 same-HEAD Required gates, Owner decisions, production credential/key/device approval and Pixel 7a acceptance. |

## Lane updates

### Lane A — Native AI / Bonsai 2

- Current branch starts at the exact existing PR #144 HEAD. Existing PR is unchanged.
- Latest source-bound native build run failed during isolated APK assembly; no recoverable Gradle log artifact exists. Root cause is unconfirmed.
- Local execution environment currently has Java 21 without `javac`, no `adb` device, and no `/dev/kvm`; native emulator inference is not available locally unless the runner/toolchain is improved.
- Native library/model download, compile, emulator inference, ARM64 runtime and Pixel testing remain NOT_RUN until direct evidence is produced.
- Next: repair failure artifact capture; inspect current JNI/CMake/build config and latest native logs; perform bounded source/metadata investigation of Bonsai 2 without committing weights or claiming unmeasured compatibility.

### Lane B — Android Product / Google / Gmail

- Branch starts at PR #146 exact source. Fake-only and no-OAuth boundary preserved.
- Found that selected Gmail handoff content included subject/from/date and snippet; patching toward body-only Chat handoff plus an exhaustive synthetic metadata-exclusion regression. Source revision binding is being updated to bind the full selected record without disclosing excluded metadata.
- Focused JVM execution has not started: available Java runtime has no compiler. A workspace-local JDK availability check is in progress.
- Next: complete focused source test, produce exact command and JUnit, and request independent security review. If no JDK/emulator becomes available, compilation/UI execution remains NOT_RUN.

### Lane C — CI / Security / Release

- Current work: recover PR #146 complete run/job/JUnit/artifact evidence; PR #144 failure classification; Owner-ready release/provider/model decisions; independent reviews of A/B.
- Early check confirms queued macOS jobs have empty `runner_name`; assigned runners for other workflows are actively executing. No retry is authorized by this evidence alone.
- Next: record every exact job/artifact result after the running workflows settle, review reports and security boundaries, then return any concrete issues to their lane owners.

## Owner decision ledger

| Decision / approval | State | Gate |
|---|---|---|
| Android Beta Package ID and Demo/Beta coexistence | OWNER DECISION REQUIRED | Before Android OAuth client identity or Beta installation/update contract |
| Signing certificate / production key custody | OWNER DECISION REQUIRED | Before real OAuth SHA binding or production signing |
| Google Cloud project, Calendar scope and OAuth disclosures | OWNER DECISION REQUIRED | Before any real OAuth activation |
| Gmail `gmail.readonly`, Restricted Scope and verification eligibility | OWNER DECISION REQUIRED | Before any live Gmail activation |
| Real Google account/data usage | OWNER APPROVAL REQUIRED | Not used; no access performed |
| Cloud provider age/region/terms/eligibility, credentials and billing | OWNER DECISION REQUIRED | Cloud remains disabled; no credentials used |
| Model distribution/license and product default model | OWNER DECISION REQUIRED | No model weights committed; no default switch |
| Pixel 7a physical acceptance | OWNER APPROVAL REQUIRED | No device operation performed |
| Protected `main` merge, production key/signing, Beta distribution | OWNER APPROVAL REQUIRED | Not performed |
| Personal Beta GO | OWNER DECISION REQUIRED | Remains NO-GO until every P0 gate and Owner decision is complete |

## Source-bound evidence rules

For every new result retain source SHA + Tree SHA, exact command/config, run + attempt + job, JUnit test IDs and totals, failures/errors/skips, runner allocation, artifact IDs/digests, APK hash, native dependency identity, model revision/hash, reviewer decision and source/report branch. Required manifests and thresholds cannot be relaxed. Compile success does not equal execution; queue does not equal pass; a failed build without its Gradle log does not identify its root cause. Evidence from another SHA remains historical.

## Campaign update — 2026-10-29 UTC 03:00

This update supersedes the earlier Phase 0 job-state snapshot above. PR #144 and #146 remain unchanged. All run IDs below are attempt 1 and tied to the listed exact source.

### PR #146 formal evidence closure

Source remains `d444e7cc04c8f1c4c6d12b1412cfe3a76258af66`, tree `25e2bb481a0b25bfed89d6ee25f1812316703b8e`.

| Workflow | Current result | Evidence |
|---|---|---|
| Android `40804694025` | **FAILURE** | Build/Lint/Instrumentation Compile, Android Unit, Integration, Security, Docs and Dependency Audit passed. Android Unit artifact `12869510041` (`sha256:c727e3421dab84a7b82a26920b2a0502f1938d560777482852e78afabbd35487`) has 34 XMLs, 405/405 PASS, 0 failure/error/skip. Integration artifact `12869310588` (`sha256:62d700d4c25ac1ef0c9ddc3645586028a2347465c42370e16a3153ab108a0355`) has `OllamaCloudAdapterTest` 22/22 PASS. Security artifact `12868263908` (`sha256:6c757e8b54d382fc0e9734cd4fe6be6e892cd8d840a96fac0dd9f949fe9ef12`) has Security 10/10 and Gmail security 16/16 PASS. These suites overlap; do not add as unique cases. Android Storage job `123230615644` ran on runner `1000023241`; Required Gate job `123232881578` ran on `1000023247`. |
| Android API 26/35 Required | **FAIL / instrumentation NOT_RUN** | Artifacts API26 `12869216935` (`sha256:d08309fde30db9fb67957bb1631ed5aba7c68f0f8d929f332da240a862c37afd`) and API35 `12868798855` (`sha256:06ed97683b5d292c2f87c0d47f1eebacddc175f7e4944530f751abe8d10134c3`) fail before install/instrumentation at `cursor-position-focused`. Each reports 0/105 JUnit tests started and no UI test result. `android_package_path()` treated `pm path`'s empty output/exit 1 for an absent isolated package as a query error. Exact response was suppressed by the workflow. AOSP Android 8.0 and Android 15 source confirms this is a normal absent-package response; the test harness, not product UI, caused the early stop. |
| Identity | **SKIPPED** | Identity job `123232824352` was gated on successful emulator completion. This is not a pass. |
| Process Death / Data Isolation / Sentinel | **PASS** | Both API artifacts show data isolation and sentinel PASS. Actual OS Process Death used host force-stop, recorded PID/relaunch and exit code 0, then verified fresh state. This does not imply UI instrumentation ran. |
| Pixel 7a Reacceptance Bundle | **CI bundle only** | Job `123232823166`; artifact `12869770024`, SHA-256 `fa8426a3e202ad445d4b3a5b00c021e7c2c3888fe74b8d28e05dbbc4d2049fad`; API instrumentation NOT_RUN, zero JUnit. Not physical-device acceptance. |
| Full CI `40804693907` | **PARTIAL / still running** | Windows P7 Gate C, stable Linux and Linux MSRV passed on assigned runners. macOS ARM64 is in progress on runner `1000023279`; macOS Intel x86_64 remains queued with `runner_id=0`. |
| Native Rust matrix `40804693934` | **PARTIAL / still running** | Stable Linux x86_64/ARM64, stable Windows x86_64/ARM64, stable macOS ARM64, Linux MSRV x86_64 and Windows MSRV x86_64/ARM64 passed. Stable macOS x86_64 is in progress; MSRV macOS x86_64 and ARM64 remain queued with no runner. This Rust matrix is not Android JNI inference evidence. |
| SQLite `40804693978` | **PARTIAL / cross-open NOT_RUN** | macOS ARM64 and Intel DB producers passed; ARM64 artifact `12869265751` (`sha256:a24f408ba84461818e3f4bb915977ad699900c54ac5dc18416009e3d305be039`) and Intel artifact `12869541631` (`sha256:f11760407eca7777e4685c0b72dc19fc14783c4251e2d11d92458a840ae14f7c`), each 36,954 bytes. Both cross-architecture open jobs remain queued with `runner_id=0`. |
| RustSec `40804693889`, Fast `40804693992`, Git Hooks `40804693919` | **SUCCESS** | All supplied checks passed at exact source SHA. RustSec artifact `12867589033`, SHA-256 `0f360ef81a5ab03efc14fecf99197c758df0be54cd48cd058935caa11078bee7`. |
| Release Security `40804722382` / Mock Beta `40804722157` | **SUCCESS** | CI-only APK/lockfile SBOM and mock signing contract passed; no production key, distribution or Beta package acceptance. Release artifact `12868174398`, SHA-256 `17ad843799fdde28e42a9ea099d1c9d571e8f6d093b958a1ee17a464037c40a7`. |

Retained JUnit XMLs and their SHA-256 inventory are in `docs/agent-reports/evidence/g8n-pr146-d444e7c/`. The full individual XML test IDs and archive digests remain source-bound in that folder. No full APK or model weights were copied there.

### Accepted Android package-absence harness correction

The new parent candidate is branch `codex/serea-g8n-integration-candidate`, PR #149, stacked on PR #146. Its current source is `dd8c5b661c1f4be6c4a171c68246cbd1fb343d50`, tree `c049cfd8b2849e738932e4c4844219c815034740`. It changes only `tools/android_package_path.sh` and its Python workflow regression. A missing package now counts as absent only when `pm path` is empty/supported absent output and a second successful `pm list packages -u <package>` query returns exactly empty output; list errors, a listed package, malformed output, transport failures and unrelated PM errors fail closed.

Focused command `PYTHONPATH=. python3 tests/android_emulator_workflow_tests.py` passed **34/34 tests**, 0 failures/errors/skips; `bash -n tools/android_package_path.sh` and `git diff --check` passed. Agent C independently reviewed exact SHA/tree and returned **ACCEPTED**. Formal CI has been triggered for this SHA: Android `40809303535` (pending, no jobs at last poll), Full `40809303532`, Native `40809303531`, Fast `40809303507`, Git Hooks `40809303450`, RustSec `40809303401`, SQLite `40809303281`; most required jobs were queued with no runner at last poll. No same-source formal CI pass is yet available. A separate branch-gated Personal Beta workflow run `40809296854` had zero jobs/logs and no artifact; it provides no Beta evidence.

### Lane A — Native AI and model candidate update

Draft PR #147 carries the isolated runtime candidate on top of the exact G7 base `0560e1e7bcd13f0194eab8caca89d70ea5a2f142`. Current head is `49e14da2b5dc06579ff88d0adbc10d41f5040dd9`, tree `bcc5ce1f837d2904f608c60db26569469340e695`. Native focused run `40808797590` passed workflow setup, clean-tree CMake checks, the source identity/JVM contract step, both opt-in JNI APK ABI builds, bounded diagnostics upload, official model license metadata verification, fixture digest verification and KVM availability. Its real Android x86_64 inference step is currently IN_PROGRESS on API 35; JUnit counts and inference result are not yet collected. No Native Runtime success is accepted until its final JUnit/artifact is source-bound and inspected. The older PR #144 latest build failure remains unclassified because its Gradle log was not retained. Historical run `40800987367` at source `082d223...` failed AndroidTest Kotlin compilation at `awaitReleased`; that does not establish the later PR #144 failure cause.

The model-selection addendum keeps the default model **undecided** and does not suspend SmolLM2 Native work. Candidate metadata research now includes LFM3-1.7B, Bonsai 2 4B, Qwen3.5 2B, Gemma 4 E2B and future FunctionGemma 270M, alongside SmolLM2. Report vendor benchmarks separately from Serea measurements. ModelChatPort and Model Router remain the exchange boundary for llama.cpp, future LiteRT-LM or another backend. No model weights are committed or downloaded in this parent lane; Pixel 7a measurement and independent review remain required before a standard model decision.

### Lane B — Gmail product safety

Lane B PR #150 is OPEN/DRAFT, stacked on PR #146. Current HEAD `31efa59eed6a7da7f058b7d17be31c7944190ab5`, tree `0168cd5d100383a564f5a2609fe369a423ff9c94`; the latest commit changes only report links from the already-reviewed product/test source at `4e4b8f9d0aad80f330d1c93364b3cbfda5369bcd`, tree `b19a6c52a8e8d4ff1c8365d435c047d0aceb300b`. Focused JVM command `:app:testDebugUnitTest --tests app.serea.android.handoff.GoogleTrustedHandoffTest` on Temurin 17 produced **22/22 PASS**, 0 failure/error/skip at the exact product/test tree. `:app:compileIsolatedAndroidTestKotlin` on source `4e4b8f9...` succeeded (26 tasks UP-TO-DATE); this is compilation, not connected UI execution. Agent A independently accepted the source/test scope after requesting and receiving negative metadata sentinels and inert HTML-only fallback assertions. Gmail UI/API26/35 JUnit and full G9 E2E remain NOT_RUN. PR #150 checks on its latest head `31efa59...` are queued/pending: Android `40809490077`, Full `40809490143`, Native `40809490124`, SQLite `40809490101`, Fast `40809490137`, Git Hooks `40809490130`, RustSec `40809490200`. A newer push to C's docs branch `893d2b3...` has also created queued reports-only workflows; these do not change product acceptance.

### Lane C — CI / Security / Release

PR #148 remains OPEN/DRAFT; latest checkpoint head `893d2b303f8557d1f6bc6b6a4aeeb5c0f6e7b276`, tree `257343ebd177c996cf619e173701000daa23f351`, based on exact G7 SHA `0560e1e7bcd13f0194eab8caca89d70ea5a2f142`. Its report refresh records the same #146 source-bound Android failure, current Full/Native/SQLite runner assignments, and C's **ACCEPTED** independent review of the parent PR #149 package-absence helper. It is documentation only; PR #148's CI result does not close product gates. C is reviewing exact PR #147 Native and PR #150 Gmail sources independently.

### Current gates and next actions

- **G8 Evidence Closure: PARTIAL.** The same-HEAD Android emulator gate failed in its precheck; formal successor CI and remaining queued cross-platform jobs are still required.
- **Native Runtime: PARTIAL.** SmolLM2 product JNI build/inference is still running through the focused workflow; no inference/lifecycle/performance result is accepted yet.
- **Bonsai/model comparison: PARTIAL.** Public metadata research only; all model runtime, performance, Japanese/tool-use and Pixel 7a measurements are NOT_RUN; standard model is undecided.
- **G9 Functional Candidate: PARTIAL.** Fake Gmail handoff body-minimization is source-reviewed and locally tested; API 26/35 UI and lifecycle/process-death Google E2E remain unproven.
- **G10 Preflight: PARTIAL.** CI-only debug package/SBOM evidence exists; native complete SBOM, signed update/migration, rollback, OAuth/Gmail scope activation runbooks and Pixel acceptance remain open.
- **Personal Beta: NO-GO.** Owner package/certificate, Google OAuth/scopes, model/provider, production signing and real Pixel decisions remain open.

Next: monitor existing CI runs by job and runner assignment without replay loops; collect exact PR #147 evidence and independent review; capture PR #150 Android source-compile/run status; commit this checkpoint and PR #146 JUnit evidence to #149; update with same-source CI results and final Owner decision matrix.

## Resume point

1. Poll existing #146 runs without restarting them; update this ledger from their exact job, JUnit, identity, artifact and hash evidence.
2. Lane A repairs diagnostics and evaluates the exact PR #144 build failure on a new campaign SHA; independently review any change before using its CI.
3. Lane B finishes body-only Gmail handoff and explicit excluded-metadata tests; review session/account generation binding and stale selection behavior.
4. Lane C completes release/security decision packet and independent review of each lane.
5. Integrate only ACCEPTED parts into this successor branch, freeze one HEAD/tree, run focused tests and formal CI against that frozen source.
6. Proceed to G10 preflight only where dependencies are satisfied; keep runtime, real OAuth, physical-device and release-only gates NOT_RUN/OWNER-BLOCKED as applicable.

**This checkpoint does not promise unattended/background continuation.** New work must resume from this ledger, actual branch/PR HEADs and exact same-HEAD evidence.

## Campaign update — 2026-10-29 UTC 03:10

This update records the new PR #149 candidate and distinguishes its static validation from the still-pending formal workflows. It also records the latest exact-source runner observations. Existing PR #144 and #146 remain untouched.

### PR #149 current integration candidate

- PR #149 is OPEN/DRAFT, branch `codex/serea-g8n-integration-candidate`, stacked on PR #146 HEAD `d444e7cc04c8f1c4c6d12b1412cfe3a76258af66` / tree `25e2bb481a0b25bfed89d6ee25f1812316703b8e`.
- Current HEAD `4500179bb32ea9696160e7bab115e19e45016da7`, tree `f4dc2f738a1e84ba2683bc1b87891748f01f2a55` adds seven Fake Google/Gmail/handoff and chat-shell UI identities to the existing non-required diagnostic stage, bringing it to exactly 16 tests on both API 26 and API 35. It remains gated behind Required JUnit, Process Death, and Data Isolation PASS. The final per-API success predicate now requires this UI stage PASS. Required and Additional manifests were not modified.
- Independent Lane B and Lane C reviews of the committed change both returned **ACCEPTED**. At 03:06 UTC, all 16 identities were confirmed as non-required manifest entries and absent from both Required and Additional test lists.
- The first focused static-suite invocation (`PYTHONPATH=. python3 tests/android_emulator_workflow_tests.py`) ran 34 cases and reported 31 PASS, 0 assertion failures, and 3 `FileNotFoundError` errors for the generated `personal-beta-identity.txt` in existing identity harness cases. Each affected case was rerun individually with captured harness output and passed. A subsequent serial rerun using `python3 -m unittest tests/android_emulator_workflow_tests.py` ran 34/34 with 0 failures/errors/skips. That rerun also includes the API26 `NOT_RUN` status assertion added in response to Lane C's independent review; its immutable source/tree identity will be added after checkpoint commit. `bash -n tools/run_android_instrumentation_campaign.sh` and `git diff --check` passed on the same code state.
- PR #149 exact-head formal run IDs: Android `40810167891`; Full CI `40810167726`; Native Rust `40810167708`; SQLite `40810171127`; Fast `40810167710`; Git Hooks `40810167802`; RustSec `40810167734`. As of 03:08 UTC, all Android, Native, Fast, RustSec, SQLite and Git Hooks jobs were queued with runner ID 0; Full CI's Windows P7 Gate C job had started on runner `1000023317` while its Linux/macOS jobs were queued. No PR #149 API 26/35, Identity, UI, Process Death, Data Isolation, Sentinel or artifact result is yet established. Branch-gated Personal Beta run `40810162939` has a main-only guard failure with no executed job; it is not product validation.

### PR #146 late formal runner update

All evidence remains for exact source `d444e7cc04c8f1c4c6d12b1412cfe3a76258af66`, attempt 1. By 03:08 UTC, Full CI had four PASS jobs (Windows synthetic, Linux stable, Linux MSRV, macOS ARM64); macOS Intel remained queued. Native Rust had 10/12 jobs complete PASS; MSRV macOS ARM64 was in progress on runner `1000023312`, and MSRV macOS x86_64 queued. SQLite had 3/4 jobs PASS, with Intel DB on macOS ARM64 still queued. These runner waits are infrastructure status and do not resolve the failed Android required gate or Identity skip on #146. PR #146 remains frozen as comparison evidence.

### Native runtime exact-run evidence and independent review

PR #147 remains OPEN/DRAFT, based on G7 `0560e1e7bcd13f0194eab8caca89d70ea5a2f142`. Attempt 1 of focused run `40808797590` is tied to source `49e14da2b5dc06579ff88d0adbc10d41f5040dd9`, tree `bcc5ce1f837d2904f608c60db26569469340e695`, runner `1000023273`, job `123233966301`. Its two ABI JNI APK builds, bounded diagnostic upload, fixture/license hash verification, KVM and API 35 x86_64 emulator setup passed. Android instrumentation started 0 tests: the verified 101,016,128-byte fixture was placed in target `filesDir` before the one-use instrumentation namespace token was seeded, so the isolated runner rejected startup (`filesEmpty=false`, token/marker absent). This is a test-harness order error before inference; no Android JUnit or real generation occurred. Focused JVM JUnit was 21/21 `LocalModelChatAdapterTest` + 1/1 `LocalModelNativeRuntimeTest`, 0 failures/errors/skips. Exact fixture SHA-256 was `e3131339bf4e8065265593d4fd8f7bb7ff2d3abff1edb5618aa1197b89cad9f5`; llama.cpp commit `276378515ceebee5d14bcc8536603dd5c87491a2`.
- Sanitized runtime artifact `12870076223` is 15,855 bytes, SHA-256 `9f6c7a7fe20657522aa8e4b86b9d58b2e55102cb96a75dd16d9d2f06013748cc`; build diagnostics `12869074919` are 5,958 bytes, SHA-256 `658e850e779214c1e8b2189e6d7778f148cc128b7624ede3e5833c3b49fbe040`. Retained source/JUnit/cleanup evidence is in `docs/agent-reports/evidence/g8n-native-49e14da/`; model weights and APK binaries were not retained. The artifact records Android and ARM64 ABI build PASS (build/package only), ARM64 runtime and physical Pixel 7a NOT_RUN, and successful fixture cleanup after exit 1.
- Lane C's independent review of the exact PR #147 source returned **CHANGES_REQUESTED**: (1) contain all C++ exceptions at every exported JNI boundary, with exception-safe partial load/resource cleanup; (2) hard-require a disposable API35 x86_64 emulator before mutation, refuse to uninstall packages not installed/owned by this run, and constrain evidence cleanup to a dedicated safe path. Lane A is implementing these fixes on a new source. The corrected harness must also seed/pass the required one-use namespace token and stage the model outside `filesDir` before connected instrumentation. No accepted Native Runtime candidate exists yet.

### Model-selection policy addendum

Standard local model remains **UNDECIDED**. SmolLM2 Native/JNI remains the active baseline and is not blocked on comparison research. ModelChatPort and Model Router remain model/backend boundaries so llama.cpp, a future LiteRT-LM backend, or another native backend can be evaluated independently. The public-spec/license comparison now includes LFM3-1.7B, Bonsai 2 4B, Qwen3.5 2B, Gemma 4 E2B, and FunctionGemma 270M (future lightweight operation-router candidate, not presumed dialogue model). Initial comparison is metadata-only; vendor benchmarks must be labeled separately from Serea measurements. Runtime, Japanese/tool quality, RAM, TTFT, tokens/s, thermals, storage, implementation difficulty, and Android compatibility remain NOT_RUN until measured. A default model requires Pixel 7a evidence and independent review. No multiple-resident model or FunctionGemma integration without demonstrated benefit.

### Current gates and next action

- G8 Evidence Closure: **PARTIAL** — #146 API26/35 instrumentation did not start; successor #149 formal jobs are waiting for runners.
- Native Runtime: **PARTIAL** — SmolLM2 ABI builds and JVM contract pass on a candidate SHA; inference, lifecycle, cancellation/release, crash recovery, ARM64 runtime and Pixel remain unproven. Review changes are in progress.
- Candidate/model evaluation: **PARTIAL** — official public-spec/license research only; standard model undecided.
- G9: **PARTIAL** — Gmail body-only local handoff is independently reviewed and 22/22 JVM tests passed on its exact source; Google/Gmail UI expansion is accepted in source but no API-level run has started.
- G10: **PARTIAL** — CI-only release/owner packet exists; full native inventory/SBOM, update/migration/rollback exercises and hardware acceptance remain open.
- Personal Beta: **NO-GO**.

Next: finish the affected static-suite rerun; monitor exact-head #149 jobs without unnecessary replay; re-review Lane A's new source and exact inference artifact; continue model comparison research independent of Native build progress; record Lane B's exact-head review and preserve the Owner approval boundary.
