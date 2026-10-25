# Serea Personal Beta Autonomous Closure Checkpoint

## Current State

- ACTIVE_PHASE: R4/R5 final API 35 campaign; R6 integrated candidate is prepared locally.
- GOAL_STATUS: PARTIAL.
- AUTHORITY_SHA: `e0acb0254820c381dd97828824bef86a1db5176c` (`origin/main` at audit); Candidate D PR #111 head `dc98547b256c3a2c1551814cdcf4a07ebc2373c8`; verified R3 branch head `93e2da60ed1b1ea05e6fdce04076800214d8a833`.
- INTEGRATION_BASE: PR #100 head `ad868af753672c546c52411ce151904360633acc` (`codex/pr99-ci-recovery`).
- WORKTREE: `/workspace/Serea-r6-clean`.
- OWNED_BRANCH: `codex/serea-personal-beta-integration-clean`.
- CURRENT_COMMIT: `1480b34c4c85033e812a96f3830cbb72a4755017`, tree `c8028f9468be85ac7ce5b2a734b96261f5abc199`; single approved-identity commit on PR #100 head. This is an unpushed local integration candidate pending R4/R5 API 35 completion and evidence checkpoint update.
- LAST_IMPLEMENTATION_COMMIT: `fd8ea17d2474d173e8216b8ab5cd3595a1549f5f`, tree `32a53ac02c4a9d4f8f74ef503f45aa83ada28daa`. The R4/R5 implementation tree is unchanged in the integrated candidate; only the Identity readiness script and this checkpoint differ.
- LATEST_CI_RUN: Android Run `40210852450`, source SHA `d449f22c5d05597d7d02529e19ea9a92f608567d`, still running API 35 at the last check. API 26 artifact ID `12606272335` is preserved in GitHub and downloaded locally to `/workspace/evidence/r4r5-40210852450-api26/`.
- API26_R4_R5: Cursor focused PASS 1/1; Calendar class PASS 19/19; ChatScreen class PASS 16/16; Full JUnit PASS 105 tests, 0 failures, 0 errors, 0 skips. Process Death, Data Isolation, and Test IME cleanup PASS. API 26 target APK SHA-256 `acea9b8d55ab404d536160d51498cee0f333bf4f0122ffb78fa2a1f6a5c35c02`; instrumentation APK SHA-256 `244279ac48ba695ad3ec3a0d98049773d99dd5949d0192c789892d894902cf45`; Test IME APK SHA-256 `aefaeb0e088be42e166d48dabf68040452f90613b9089c92766eab175fcb6599`.
- API26_IME: Android-managed Test IME callback and non-null bound InputConnection were confirmed. Composition changed `null -> 0..5 -> 0..25 -> null`; ViewModel and Semantics selection matched. Gmail, Calendar, Activity recreation, and insertion-position checks passed. Default IME was restored and diagnostic IME disabled/removed.
- API35_R4_R5: NOT YET REPORTED; the serialized emulator step in Run `40210852450` remains `in_progress`. Do not infer PASS from API 26.
- CALENDAR: API 26 Calendar class and full suite passed with UiAutomator swipes and the new post-query UI idle barriers. No SnapshotStateObserver exception occurred. Previous failure is classified TEST_HARNESS strongly supported; API 35 evidence is pending.
- IDENTITY: Formal Android workflow integration is present but not yet verified on the integrated candidate SHA. PR #113 remains diagnostic only. PR #112's older ADB root/startup implementation was not copied over the stricter R3 implementation; its transient Package Manager readiness retry was selectively adapted into `tools/test_personal_beta_identity.sh`.
- PR100_PROVENANCE: PR #100 overlaps the Candidate D editor state. Its stale local `rememberSaveable` Composer value was excluded because Candidate D/R3 owns `TextFieldValue` and generations in `ChatViewModel`. This preserved the tested D implementation and avoided a second editor state source.
- R4_R5_RUN_HISTORY: Run `40210852450` is on v2 commit `d449f22c5d05597d7d02529e19ea9a92f608567d`; its Android job is serialized API 26 then API 35. Fast CI Run `40210852435` failed the commit identity guard on the v2 history. Do not treat that as a source/test failure. Full CI `40210852428`, Native `40210852427`, and SQLite `40210852471` were cancelled to control duplicate work and must be rerun on the final integrated SHA.
- CURRENT_BLOCKER: API 35 R4/R5 instrumentation in Run `40210852450` has not completed.
- NEXT_ACTION: Wait for API 35 artifact and inspect focused, class, 105-test, Process Death, Data Isolation, and IME cleanup evidence. Update this checkpoint with exact outcome. Then verify PR #100 remote head is unchanged, push this clean-history integration branch, create a Draft PR based on PR #100 head, and run all Required/CI gates on that exact integrated SHA. Keep PR #116 open until its Android run completes; supersede it only after the replacement Draft PR exists.
- REQUIRED_GATES: Android API 26 and API 35 each 103 Original + 2 Additional = 105 tests, 0 failures/errors/skips; Process Death; Data Isolation; Sentinel; formal Identity Required; Android Build/Unit/Integration/Security/Dependency Audit; Pixel 7a CI reacceptance bundle; Native; Fast; Full; SQLite; Git Hooks; RustSec. Required manifests and thresholds remain unchanged.
- OWNER_APPROVAL_REQUIRED: main merge, final Personal Beta GO, physical-device destructive work, production signing credentials, real Gmail/Calendar accounts, or acceptance criteria changes.

## R4/R5 Implementation Summary

- The formal Chat draft test uses the Android-bound standalone Test IME and `getCurrentInputConnection()`, verifying IME package/session/editor target, composition, selection, ViewModel/Semantics state, Gmail/Calendar return, Activity recreation, and cleanup.
- The Calendar test uses UiAutomator touchscreen gestures, asserts actual LazyList progress and event identity, and waits for Compose/UI idle after the date query before scrolling.
- The diagnostic identities remain explicitly separate. The 103 + 2 Required manifests, expected test identities, skip conditions, and thresholds were not changed.

## R6 Provenance

- PR #111 Candidate D (`dc98547...`) and PR #114 Phase R2.5 (`4d4a6bb...`) are ancestors of the verified R3 source branch.
- R3 real-IME branch `93e2da60...` passed Android-managed IME B/C/D on API 26/35 in Run `40203437416`.
- PR #100 head is included as the integration base. The Candidate D ViewModel-backed editor model supersedes its conflicting local Composer state.
- PR #112's formal Identity script update is partly superseded by the later R3 readiness/diagnostic implementation. Only the additional Package Manager transient transport handling was retained. Formal Identity Required remains pending.
- PR #113 diagnostic success is not formal Identity Required evidence. PR #114's changes are already in R3; no extra cherry-pick is needed.

## Failure Classification

| Issue | Classification | Evidence / next action |
|---|---|---|
| Composer composition failures in the old formal test | TEST_HARNESS strongly supported; product root cause UNKNOWN | Directly created InputConnection was not the Android-bound IME session. R3 and current API 26 R4 evidence show real IME composition passes. Confirm API 35 current run. |
| Calendar SnapshotStateObserver exception | TEST_HARNESS strongly supported | Earlier exception came from Compose semantics/touch failure diagnostics crossing SnapshotStateObserver thread ownership. UiAutomator gesture path and API 26 focused/class/full pass without recurrence. Confirm API 35. |
| API 26 first date-change swipe did not advance | TEST_HARNESS supported | Previously occurred 9 ms after date query on API 26 while API 35 passed. Post-query Compose and UI idle boundaries now yield API 26 full 105/105. Confirm API 35. |
| Formal Identity Required | UNKNOWN / not yet run on integrated candidate | Run the existing Android Required workflow on the final integration SHA. |

## Owner Handoff State

- The unpushed clean-history integration commit is local and reproducible. Its tree matches the audited integration worktree and uses `Codex <noreply@github.com>` identity.
- No Required gate has been weakened and no full formal gate has been claimed from the prior R3 diagnostics.
- Personal Beta remains NO-GO until every exact-SHA gate passes, physical reacceptance is reviewed as applicable, and the Owner approves GO.
