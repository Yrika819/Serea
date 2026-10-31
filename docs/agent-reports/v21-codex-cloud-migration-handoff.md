# Serea V21 — Codex Cloud Migration Handoff

Date: 2026-10-31  
Status: **PREPARATION COMPLETE / OWNER APPROVAL REQUIRED FOR TRANSFER PUSH**. No commit, push, merge, PR mutation, source rewrite, or heavy local build was performed. Personal Beta remains **NO-GO**.

## 1. Baseline and preservation

### Main Android worktree

- Worktree: repository root `android-pr204-local` (local path intentionally omitted from this Cloud handoff).
- Branch: `local/android-pr204`
- HEAD: `8017e7e9e8987dc66361eb9297cf87535d8ef7b1` (expected and current; exact PR #204 head at last GitHub read).
- Tracked diff from HEAD: SHA-256 `8fce3598485a31c0831d7024a6e859e9854fa2d6ca18e998071095cb6698e873`; 5 modified paths, 105 insertions / 12 deletions at review time.
- Modified paths: `android/app/build.gradle.kts`, `android/app/src/androidTest/java/app/serea/android/ChatScreenTest.kt`, `android/app/src/androidTest/java/app/serea/android/LegacyHistoryV4MigrationDiagnosticTest.kt`, `android/app/src/androidTest/java/app/serea/android/LocalModelChatViewModelIntegrationAndroidTest.kt`, `android/app/src/main/java/app/serea/android/MainActivity.kt`.
- Untracked source/docs/evidence are listed below. Main worktree untracked entries consist of `AGENTS.md`; the PhysicalQA sources; V14, V15, V16, V17, V17.1, V18 and V19 report files; V16/V17/V17.1/V19 evidence directories; V20 CI recovery report/evidence directory; V20 physical acceptance report/evidence directory; and `android/.kotlin/` cache/error logs. The four newly created V21 Markdown files are also untracked. `android/.kotlin/` and its error logs are generated local state, not source.

### Native isolation clone

- Worktree: `tmp/v17-native-isolation` relative to the main repository root (local absolute path intentionally omitted).
- Branch: `local/v17-native-variant-isolation`
- HEAD: `36700f6c99d15db428c6d434c657ac76593df3f9` (expected and current).
- Tracked diff SHA-256 `a27a640d75e0161f6d41b075fa4b40661a8930670c413c2815ae31ae04d796b0`; two modified paths: `android/app/build.gradle.kts`, `android/app/src/main/cpp/CMakeLists.txt`.
- Untracked sources: `AGENTS.md`, `android/app/src/androidTestNativeqa/AndroidManifest.xml`, `android/app/src/androidTestNativeqa/java/app/serea/android/NativeQaSmokeTest.kt`, `android/app/src/androidTestNativeqa/java/app/serea/android/SereaPhysicalNativeQaRunner.kt`, `android/app/src/nativeqa/AndroidManifest.xml`, `tools/v17-native-variant-isolation-check.sh`. `android/app/.cxx/` is a generated NDK cache and excluded.

Both worktrees passed `git diff --check` at this audit. No existing worktree content was removed or rewritten. The diff hashes exclude untracked paths; those paths must be reviewed/selectively staged individually before transfer.

## 2. V20 physical UI evidence — preserve status distinctions

Source evidence: [`v20-evidence/pixel7a-ui-acceptance.md`](v20-evidence/pixel7a-ui-acceptance.md) and [`v20-evidence/isolated-after-process-restart.png`](v20-evidence/isolated-after-process-restart.png).

- PASS: Pixel 7a identity; isolated Cold Start; Chat display; one synthetic `V20 test` local Demo send; generic IME display and Back dismissal; Chat→Settings→Back; Home/background→foreground; isolated force-stop/cold relaunch restored the synthetic conversation; screenshot edge-to-edge visual spot check.
- NOT_RUN: Japanese IME conversion/commit, ActivityScenario recreation, formal OS low-memory process death, physical JUnit. The installed IME was `com.akshaykadam.pixelboard`; Android shell Unicode text injection failed. No IME settings were changed.
- JUnit: NOT_RUN, 0 executed / 0 failures / 0 skips. Existing `SereaIsolatedAndroidJUnitRunner` is emulator-only (API 26/35 plus disposable campaign marker). It was not changed or bypassed.
- APK: existing `app.serea.android.isolated`, versionCode 1, versionName `0.1.0-demo`, SHA-256 `7101030d92b5c7d52d17f4f5839a939006756fd9cf5217251189259b3f9bcfa0`; Debug signer SHA-256 `caff85c2b3d11e95cdcaafe45806f4f836e79217ff3212c05757ada1ac450d10`. APK pull is evidence only and must not be cloud-transferred.
- The only chat data written were the authorized synthetic message and local Demo response in the isolated package. Existing Demo, other apps, notification contents, Room/Keystore real data were not touched. No APK install/update occurred.

Do not upgrade process force-stop/relaunch into a claim of Activity recreation or OS process-death acceptance. Keep all statuses above unchanged unless new exact evidence is produced.

## 3. File-by-file migration classification

### A. Android CI repair (cloud-eligible source; selective hunks only)

- `android/app/src/androidTest/java/app/serea/android/LocalModelChatViewModelIntegrationAndroidTest.kt`: remove invalid explicit `androidx.compose.ui.test.assertExists` import; preserve valid member call. This file also contains other ViewModel test/Compose host modifications, so do not blindly stage the whole file as a CI-only commit.
- `android/app/src/androidTest/java/app/serea/android/LegacyHistoryV4MigrationDiagnosticTest.kt`: explicit `arrayOf<Any?>` on mixed SQLite bind arrays to avoid Kotlin intersection/reified inference errors. The file's synthetic-only migration fixture remains non-required; do not change required test identities/counts.
- Evidence: `v20-ci-recovery-evidence/v20-ci-recovery.md` records GitHub Run `41054235106` failure and local focused compile/unit/device results. Formal Android Required post-fix is still NOT_RUN. Transfer the sanitized Markdown report, not APKs or raw local logs.

### B. PhysicalQA (cloud-eligible after Linux build compatibility review)

- `android/app/build.gradle.kts`: PhysicalQA variant, build task boundary, AndroidTest enablement, and NDK probe task. This path currently hardcodes `darwin-x86_64` as the NDK host; a Linux Codex Cloud build will fail until a minimal host-tag selection supports the actual Cloud runner. Do not claim cloud build readiness or alter the gate silently.
- `android/app/src/androidTest/java/app/serea/android/PhysicalQaNativeSmokeAndroidTest.kt`
- `android/app/src/androidTest/java/app/serea/android/SereaPhysicalSmokeAndroidJUnitRunner.kt`
- `android/app/src/physicalqa/AndroidManifest.xml`
- `android/app/src/physicalqa/cpp/serea_physicalqa_smoke.cpp`
- `android/app/src/physicalqa/java/app/serea/android/PhysicalQaNativeSmoke.kt`
- `android/app/src/physicalqa/java/app/serea/android/ProfileVerifierCacheMarker.kt`
- Preserve exact package isolation, Pixel/API/ABI guard, per-test allowlist/token and no-INTERNET invariant. The physical runner must never replace or weaken formal API26/35 gates.

### C. NativeQA isolation clone (separate Cloud lane)

- `tmp/v17-native-isolation/android/app/build.gradle.kts`: opt-in is restricted to a single exact task; adds a separate `nativeqa` build type with `app.serea.android.nativeqa`, arm64-only packaging, runtime flag off unless NativeQA opt-in, and runner identity selection. Keep its fail-closed checks intact.
- `tmp/v17-native-isolation/android/app/src/main/cpp/CMakeLists.txt`: adds static-library symbol hiding (`--exclude-libs,ALL`) so only expected JNI exports remain visible.
- `tmp/v17-native-isolation/android/app/src/nativeqa/AndroidManifest.xml`, `tmp/v17-native-isolation/android/app/src/androidTestNativeqa/AndroidManifest.xml`, `NativeQaSmokeTest.kt`, and `SereaPhysicalNativeQaRunner.kt`: isolated no-INTERNET native app and exact single-method Pixel/API/ABI/token runner. The one added V19 test exercises model-free close/post-close rejection only; it is not real-model lifecycle proof.
- `tmp/v17-native-isolation/tools/v17-native-variant-isolation-check.sh`: variant/task guard checks. Preserve this lane as a separate candidate; do not merge it into PR #204 or let llama/ggml enter normal Debug/Release.

### D. Compose / Chat / Room test behavior

- `android/app/src/androidTest/java/app/serea/android/ChatScreenTest.kt` and `android/app/src/main/java/app/serea/android/MainActivity.kt`: paired semantic locator change (`contentDescription="送信"`, `testTag="chat-send-button"`) plus focused test lookup. Stage together if accepted as one Compose semantics fix.
- `LocalModelChatViewModelIntegrationAndroidTest.kt`: Compose rule scope and precise expected error assertion are separate from the CI import removal. Review/commit by hunks or as a separately named test-only unit; preserve Fake-only behavior, non-required manifest status and local consent/usage boundaries.
- `LegacyHistoryV4MigrationDiagnosticTest.kt`: the synthetic migration test additions already exist in the PR chain; the local uncommitted edit is only explicit nullable bind-array typing. Keep its temp database/unique alias behavior and no real Room/Keystore access.

### E. Ledgers, evidence, and AGENTS

- `AGENTS.md` and Native clone's own `AGENTS.md` must travel with their matching source scope. Do not combine or overwrite the two files.
- Untracked text reports include `v14-pixel7a-local-device-ledger.md`, `v15-pixel7a-physical-native-ledger.md`, `v16-mac-native-integration-ledger.md`, `v16-mac-only-agent-brief.md`, `v16-native-pr147-compatibility.md`, `v17-mac-native-variant-isolation-brief.md`, `v17-mac-native-variant-isolation-ledger.md`, `v17-1-mac-native-build-ledger.md`, `v18-pixel7a-native-acceptance-ledger.md`, `v19-android17-native-safety-ledger.md`, `v19-evidence/v19-nativeqa-recovery.md`, `v20-ci-recovery-evidence/v20-ci-recovery.md`, `v20-local-physical-acceptance-handoff.md`, and the V20 UI evidence Markdown. Existing V16/V17/V17.1/V19/V20 evidence directories include additional logs and binaries; do not transfer those directories wholesale.
- Existing V14–V20 reports/evidence are source references only. In particular, do **not** transfer raw `v20-ci-recovery-evidence/v20-ci-recovery.md`, raw `v20-evidence/pixel7a-ui-acceptance.md`, or older raw ledgers by default; they include local environment details and/or device identifiers. The Cloud-safe text packet is this V21 handoff plus `v21-cloud-sanitized-ci-summary.md`, `v21-cloud-sanitized-ui-summary.md`, and `v21-codex-cloud-first-task.md`.
- Transfer only reviewed, sanitized text needed to establish HEAD/status, security boundaries, test results and source provenance. Review for absolute paths, local usernames, serials/device identifiers, environment-specific details, and incidental device or personal data before adding. Do not move raw logs just because they are under `docs/`.

### F. Local-only / transfer prohibited by default

- `android/.kotlin/**`, Native clone `android/app/.cxx/**`, Gradle/NDK build outputs and caches.
- Binary artifacts and device pulls: APK/AAB/`.so`/`.png` and installed package copies, including `v19-evidence/artifacts/libserea_local_llama.so`, `v20-ci-recovery-evidence/*.apk`, and `v20-evidence/*.apk`.
- Raw command logs, environment dumps, SDK/NDK caches, generated CMake/Ninja metadata, device-local state, model weights, credentials/tokens, signing materials, personal/notification data, and machine-specific absolute-path configuration.
- The fixed llama.cpp checkout is outside Git and not a Cloud transfer payload. Cloud must obtain/verify the approved source revision in its own workspace; revision `276378515ceebee5d14bcc8536603dd5c87491a2` and clean-tree rule are the invariants.

## 4. GitHub PR dependency map (read-only snapshot)

GitHub PR metadata queried read-only on 2026-10-31. Recheck current refs/status before acting; no PR state was changed.

| PR | State / head | Base and relationship | Migration implication |
|---|---|---|---|
| #160 | OPEN, Draft; `025aff71effb03e0fe3bd540079530369e9beaf7` | Base `0560e1e7bcd13f0194eab8caca89d70ea5a2f142`; separate G7 Native chain | Run `40884851425` exact-source API35 x86_64 diagnostic inference PASS; ABI build PASS for arm64 and x86_64, but ARM64 runtime/Pixel NOT_RUN. Do not merge/cherry-pick into #204. |
| #163 | OPEN, Draft; `2c109276bcbdd4e14fb2b6e9558962516f2df32c` | Directly based on #160 head `025aff7…` | Static model metadata matrix only; no weights/selection. Review in Native chain; do not merge into #204. |
| #199 | OPEN, Draft; `018d19f025e5f3232abcf2a112b8dbf954bf04e8` | Base branch `codex/serea-v10-agent-a-lifecycle-test`, base OID `eb07aee0408e562f17cb472b474a79e1dca8f30f`; its earlier source parent is `c1d163b…` | Parent ViewModel fake-only test candidate. #204 uses current #199 head as its base. |
| #202 | OPEN, Draft; `042114fcbd52a3e6274c398fe37e1b03626c16b1` | Base `codex/serea-v11-agent-a-vm-candidate` at old OID `c1d163b4091fbab29a2e4d2c3c42596b355e8226` | Compose-host diagnostic branch forked from earlier #199; its commits are included in #204 integration candidate. Avoid duplicate cherry-picks. |
| #203 | OPEN, Draft; `b83dfeb6f94a0b2fdb9c954e3d50863c48c3d4d2` | Same older #199 base `c1d163b…` | V4→V5 synthetic Room migration diagnostic; its commits are included in #204 integration candidate. Avoid duplicate cherry-picks. |
| #204 | OPEN, Draft; `8017e7e9e8987dc66361eb9297cf87535d8ef7b1` | Base branch `codex/serea-v11-agent-a-vm-candidate` at current #199 head `018d19f…`; integration body records commits from #202 and #203 | Current integration candidate. Its source branch does not include this worktree's uncommitted diff or untracked files. Exact-head Formal Android Required is NOT_RUN after local CI repairs. |

### Proposed integration order (not executed)

1. Review/accept #199 parent source and required exact-head CI before any merge decision.
2. Treat #204 as the combined Compose/Room candidate on current #199, because it already includes #202/#203 source commits. Verify commit ancestry and manifests on current GitHub before deciding whether #202/#203 become redundant; do not merge all three blindly.
3. Reconcile V20 local CI fixes and PhysicalQA changes as a new, path-selective source revision based on the approved #204 chain. Then run exact-head Formal Android Required. Do not push until Owner authorizes exact branch/commit contents.
4. Keep #160/#163 isolated from #204; any future Native integration is a separate proposal after source/review, license, SBOM, model approval and ARM64 acceptance gates.

## 5. Cloud test design for remaining Pixel-only journeys

### A. Japanese text and Japanese IME are separate acceptance cases

Current read-only V20 discovery found default IME `com.akshaykadam.pixelboard/com.android.inputmethod.latin.LatinIME`; Japanese capability/subtype is **NOT_VERIFIED**. ADB `input text` Unicode failure was a shell/keymap limitation, not an Serea failure. Do not change the device IME or install a keyboard without Owner approval.

1. **Unicode editor/send case:** one test injects synthetic Japanese literal `こんにちはV21` into the Compose editor using a Unicode-capable test API, asserts exact text in editor/ViewModel, then sends via the local Demo route and verifies exact user text plus Demo response. This proves Unicode/editor/storage path, not Japanese IME conversion.
2. **Japanese IME conversion/commit case:** only after Owner approves an already-installed or newly installed Japanese-capable IME/subtype, use actual IME key events/candidate selection to compose a Japanese phrase, explicitly commit it, assert the committed value and cleared composition state, then send locally. Record IME package/subtype and test method only; never capture keyboard suggestions containing unrelated personal text.
3. Proposed exact identities: `V21PhysicalImeTest#unicodeTextRoundTripsThroughComposer` token `SEREA_V21_PHYSICAL_JP_UNICODE_ONLY`; `V21PhysicalImeTest#japaneseImeCompositionCommitsToComposer` token `SEREA_V21_PHYSICAL_JP_IME_COMMIT_ONLY`. Final class/method/token must be reviewed and frozen before allowlisting.
4. Target/test identity: target `app.serea.android.isolated`, instrumentation `app.serea.android.isolated.test`. Require package/version/signing certificate + APK SHA comparison against the already-installed app before any in-place update. Do not uninstall or clear app data. Confirm Cloud Disabled/local Demo route and synthetic-only test content.

### B. Activity recreation versus process death

1. **Activity recreation:** exact JUnit identity `V21PhysicalLifecycleTest#activityRecreationRestoresSyntheticConversation`; launch Chat, create a unique synthetic conversation, send through local Demo, call `ActivityScenario.recreate()`, assert a new Activity instance was created, conversation ID/message/response restored exactly once, composer/input usable, and no duplicate send. This tests Activity recreation only.
2. **Process death:** separate exact identity/orchestrated case `V21PhysicalLifecycleTest#processDeathRestoresSyntheticConversation`. Prove the target PID changes; persist one uniquely named synthetic conversation; terminate only `app.serea.android.isolated` through a controlled external device harness; relaunch; assert restore once and no duplicate messages/usage. Do not call `force-stop` alone a test of Android low-memory process recreation. Test-driver survival and process kill mechanics must be proven before accepting results; no Room/Keystore cleanup.
3. Proposed opt-in token: `SEREA_V21_PHYSICAL_LIFECYCLE_RESTORE_ONLY` for the Activity case and `SEREA_V21_PHYSICAL_PROCESS_DEATH_RESTORE_ONLY` for the separately orchestrated process case. Keep them separate to isolate failure and prevent broad selectors.
4. A new physical Runner must be a distinct class/Gradle entry point, scoped to the expected Pixel 7a / `lynx` / API 37 / arm64-v8a and exact one-method selector/token, validate target package `app.serea.android.isolated`, refuse `package`, regex, class-wide, `notClass`, and multi-test selectors, and write a safe bounded summary. The existing `SereaIsolatedAndroidJUnitRunner` and formal API26/35 campaign must remain untouched. Instrumentation APK `app.serea.android.isolated.test` identity and both APK signer/package/hash must be checked before any install. A runner-internal serial check may not be available to an app process; bind serial at the host orchestrator and validate device build identity inside the runner.
5. Cloud implementation scope: design/build/host tests and source review in Cloud; no local Pixel operations are needed until owner-authorized APK provenance and a focused run plan are approved. Cloud must not claim Pixel results.

### C. Physical test acceptance values

Before ARM64 real-model execution, Owner must approve candidate/revision/SHA/use and redistribution terms plus quantitative limits for peak RSS/PSS, load latency, first-token latency, generation rate, cancel bound, post-close memory reclamation window, temperature/thermal status, and battery/power. Candidate diagnostics must use exact model hash and isolated no-INTERNET package, default-off Native backend, consent/usage/egress checks, no real account/cloud, and bounded test duration. Unset thresholds are a blocker, not an assumed pass.

## 6. Native model readiness (no acquisition authorized)

- PR #160 `40884851425`: exact source `025aff71…`, API35 x86_64 emulator inference workflow PASS; two diagnostic runner identities; fixture bytes/hash verified and cleaned in workflow. `arm64-v8a` build/package PASS does **not** equal ARM64 runtime. Runtime artifact download was HTTP 403 locally; APK/JUnit binaries, APK signer/hash, runtime timing and memory details are unavailable here.
- Fixture candidate: `ggml-org/SmolLM2-135M-GGUF`, revision `1076b85d135017971845b19b8d07df1479155988`, file `SmolLM2-135M-Q4_K_M.gguf`, metadata 101,016,128 bytes, LFS SHA-256 `e3131339bf4e8065265593d4fd8f7bb7ff2d3abff1edb5618aa1197b89cad9f5`. CI reported byte/hash verification on disposable API35 x86_64; no file was downloaded to this Mac/Pixel.
- License: GGUF model card declares Apache-2.0 but no separate repository LICENSE/COPYING/NOTICE was found; exact converted base revision and dataset provenance are unresolved. Owner/legal distribution clearance and notices are still required. Pinned llama.cpp `276378515ceebee5d14bcc8536603dd5c87491a2` is MIT, but complete packaged native dependency inventory/SBOM/NOTICE is open.
- RAM: 101 MB on-disk file size is not a RAM estimate or enforced cap. PR #160's reviewed `maxResidentMemoryBytes` was not an OS RSS/PSS limit. Pixel RAM peak, post-close reclamation, thermal/power and latency are NOT_RUN. Japanese suitability is not established for this English-focused candidate. A Japanese candidate needs its own pinned exact file hash and license/compatibility review.
- Required next step is source/metadata/legal review in Cloud and Owner approval. Do not download, stage, execute or redistribute weights in this migration task.

## 7. Proposed small commit units — approval required

All units are proposals only. Current branch contains uncommitted work; no `git add`, commit, push, or PR update was performed. Selective hunks are necessary because `build.gradle.kts` and `LocalModelChatViewModelIntegrationAndroidTest.kt` combine concerns.

1. **CI compiler fixes:** only the invalid import-removal hunk in `LocalModelChatViewModelIntegrationAndroidTest.kt` plus explicit nullable bind-array typing in `LegacyHistoryV4MigrationDiagnosticTest.kt`. Preserve all required/non-required identity counts. Parent: exact #204 head `8017e7e…`. Verify local evidence and rerun exact-head Formal Android Required in GitHub after approved push.
2. **Compose send semantics:** `MainActivity.kt` semantics/testTag and paired `ChatScreenTest.kt` selector change. Parent: same #204 head, unless cloud inspection proves already part of a later authorized source. Focused AndroidTest compile and UI test review; physical UI separate.
3. **PhysicalQA variant:** relevant hunks of `android/app/build.gradle.kts` plus the six `src/physicalqa`/runner files listed above. Before commit, make the NDK host selection Cloud-compatible (Linux) in a separately reviewed patch and add host-side fail-closed/task-graph tests. Preserve current Intel path, package isolation, exact tokens and no-INTERNET. No broad physical connected tests.
4. **Remaining ViewModel diagnostic refactor:** remaining specific hunks in `LocalModelChatViewModelIntegrationAndroidTest.kt` (custom Compose TestRule and explicit expected error). Separate test-only commit, exact non-required scope; reviewer should assess rule behavior and test count/manifest impact.
5. **Documentation / migration packet:** `AGENTS.md` (and Native clone AGENT only with Native source packet), `v21-cloud-sanitized-ci-summary.md`, `v21-cloud-sanitized-ui-summary.md`, this V21 handoff, and `v21-codex-cloud-first-task.md`. Do not transfer raw V14–V20 reports/ledgers without an explicit sanitization pass; exclude APK/screenshot binaries and raw logs.
6. **Native isolation candidate:** only if separately authorized, create an independent branch/PR based on the exact native parent chain. Include Native clone Gradle/CMake, `src/nativeqa`, `src/androidTestNativeqa`, guard script and its matching `AGENTS.md`. Never copy `.cxx`; do not merge whole native PR into #204.

## 8. Codex Cloud reproducibility and Formal CI

1. Start by checking out the approved PR #204 branch `codex/serea-v13-integration-candidate` at the approved exact source SHA (baseline currently `8017e7e…`), then inspect `git status`, base/head ancestry and PR status. Do not assume uncommitted local files exist in Cloud.
2. Read `AGENTS.md` before editing; compare it with the Native clone AGENT if work crosses Native boundaries. Keep Cloud Disabled, Default Deny, Consent/Usage/Egress/source-lineage and Native variant isolation unchanged.
3. Use JDK 17 and pinned Gradle Wrapper 8.14.6/Android SDK from the CI image. Run one Android/NDK build at a time in Cloud. For native work, fetch/verify clean llama.cpp commit `276378515ceebee5d14bcc8536603dd5c87491a2` using a Cloud-local path; do not transfer Mac caches.
4. After Owner-authorized exact source is pushed, run the GitHub Formal Android Required workflow on that exact SHA. Verify Build/Lint, Unit, Integration, API26 and API35 instrumentation and exact required-count/process-death/data-isolation/sentinel gates from workflow artifacts. Run ID and SHA must match the candidate. Do not use prior run PASS, local test results, or physical API37 results to waive gates.
5. Do not run wide connected tests on Pixel. Keep host-emulator required tests in Codex CI, and only the new exact physical API37 IDs on the local Pixel after separate readiness/operation approval.

## 9. Independent review

- Separate read-only Agent review: **ACCEPTED** for this V21 migration packet and first-task instruction, including PR ancestry interpretation, Cloud-transfer exclusions, Owner commit/push gate and Japanese IME/Activity recreation/process-death distinctions.
- Review scope was local docs/status and V20 evidence. It did not query live GitHub status, execute tests, or access the device. Recheck PR state/SHAs before any authorized remote operation.

## 10. Approval needed before Cloud can receive local changes

Please approve or reject these precise operations:

- Whether to create the six small commits above (or fewer) from the listed path/hunk sets in the existing V15 worktree, preserving all unrelated changes.
- Whether to push the resulting approved commit(s) to the existing PR #204 head branch `codex/serea-v13-integration-candidate`, or instead to a new Owner-named branch for review. This will change a remote branch and update its PR if the existing branch is selected.
- Whether the four V21 Cloud-safe Markdown files (`v21-codex-cloud-migration-handoff.md`, `v21-codex-cloud-first-task.md`, `v21-cloud-sanitized-ci-summary.md`, `v21-cloud-sanitized-ui-summary.md`) and the two `AGENTS.md` files may be committed/pushed with their corresponding source. Raw prior reports/ledgers require sanitization first. No APKs, `.so`, `.cxx`/`.kotlin` cache, weights, credentials, local device pulls, raw logs, serials, private information, or absolute machine paths are included in the proposal.
- Whether a Japanese IME package/subtype may be installed or enabled if the current Pixel configuration lacks one, and whether a separately scoped physical runner may later be installed/updated in `app.serea.android.isolated.test` after package/signer/hash review.
- Model-specific approval is separate: exact artifact/revision/SHA, license/distribution authorization, and resource thresholds are needed before model acquisition or Pixel ARM64 runtime.

Until explicit approval, all source/evidence remains only in the existing local worktrees; no Git operation is authorized by this document.