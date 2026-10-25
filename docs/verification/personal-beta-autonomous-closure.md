# Serea Personal Beta Autonomous Closure Checkpoint

## Current State

- ACTIVE_PHASE: R6 Identity Required failure isolation; R4/R5 are complete on API 26 and 35.
- GOAL_STATUS: PARTIAL.
- AUTHORITY_SHA: `e0acb0254820c381dd97828824bef86a1db5176c` (`origin/main` at audit); Candidate D PR #111 head `dc98547b256c3a2c1551814cdcf4a07ebc2373c8`; verified R3 branch head `93e2da60ed1b1ea05e6fdce04076800214d8a833`.
- INTEGRATION_BASE: PR #100 head `ad868af753672c546c52411ce151904360633acc` (`codex/pr99-ci-recovery`).
- WORKTREE: `/workspace/Serea-r6-clean`.
- OWNED_BRANCH: `codex/serea-personal-beta-integration-clean`.
- CURRENT_COMMIT: local integration candidate on PR #100 head. It includes the R4/R5 implementation/evidence, R6 provenance, and safe Identity failure-stage instrumentation. Exact-SHA CI is pending; see the current branch HEAD for the authoritative commit and tree.
- LAST_IMPLEMENTATION_COMMIT: `fd8ea17d2474d173e8216b8ab5cd3595a1549f5f`, tree `32a53ac02c4a9d4f8f74ef503f45aa83ada28daa`. The R4/R5 implementation tree is unchanged in the integrated candidate; only the Identity readiness script and this checkpoint differ.
- LATEST_CI_RUN: Android Run `40210852450`, source SHA `d449f22c5d05597d7d02529e19ea9a92f608567d`. API 26/35 emulator campaigns and Pixel 7a CI reacceptance bundle completed PASS. Formal Personal Beta Identity job failed during `campaign-seed-and-claim`; Android Required Gate consequently failed. API 26 artifact ID `12606272335` and API 35 artifact ID `12606374414` are preserved in GitHub and downloaded locally to `/workspace/evidence/r4r5-40210852450-api26/` and `/workspace/evidence/r4r5-40210852450-api35/` respectively. Identity artifact ID `12606828807` contains `NOT_RUN`.
- API26_R4_R5: Cursor focused PASS 1/1; Calendar class PASS 19/19; ChatScreen class PASS 16/16; Full JUnit PASS 105 tests, 0 failures, 0 errors, 0 skips. Process Death, Data Isolation, and Test IME cleanup PASS.
- API26_IME: Android-managed Test IME callback and non-null bound InputConnection were confirmed. Composition changed `null -> 0..5 -> 0..25 -> null`; ViewModel and Semantics selection matched. Gmail, Calendar, Activity recreation, and insertion-position checks passed. Default IME was restored and diagnostic IME disabled/removed.
- API35_R4_R5: Cursor focused PASS 1/1; Calendar class PASS 19/19; ChatScreen class PASS 16/16; Full JUnit PASS 105 tests, 0 failures, 0 errors, 0 skips. Process Death, Data Isolation, and Test IME cleanup PASS. API 35 target APK SHA-256 `acea9b8d55ab404d536160d51498cee0f333bf4f0122ffb78fa2a1f6a5c35c02`; instrumentation APK SHA-256 `244279ac48ba695ad3ec3a0d98049773d99dd5949d0192c789892d894902cf45`; Test IME APK SHA-256 `aefaeb0e088be42e166d48dabf68040452f90613b9089c92766eab175fcb6599`, identical to API 26.
- API35_IME: Android-bound `RemoteInputConnection` sessions targeted `app.serea.android.isolated`, input type `163841`; composing ranges were `null -> 0..5 -> 0..25 -> null`. In one Full-run B observation, `finishComposingText()` was followed by a transient old range at `R4-B-after-finish-call`, then `null` at the state-reflected `R4-B-committed` boundary. Synchronized C and all assertions passed; no product failure was reproduced. Gmail, Calendar, recreation, and insertion position passed. Default IME was restored and Test IME package removed.
- CALENDAR: API 26 Calendar class and full suite passed with UiAutomator swipes and the new post-query UI idle barriers. No SnapshotStateObserver exception occurred. Previous failure is classified TEST_HARNESS; API 26 and API 35 focused/class/full reruns pass without recurrence.
- IDENTITY: Formal Android workflow integration ran on prior v2 SHA `d449f22...` and failed after the v100 APK installed, when `campaign-seed-and-claim` returned `IllegalStateException`; the emitter only logged the exception class, so the exact guard is UNKNOWN. A test-only stage label (`seed campaign marker` vs `claim campaign marker`) was added to the integrated candidate receiver without logging exception messages, stack contents, or fixture values. The same focused host run failed to compile because this container's `/usr/lib/jvm/java-21-openjdk-amd64` lacks `javac`; hosted Build/Lint had succeeded on the prior source. Python regression suites pass: Android emulator workflow 26/26, instrumentation result verifier 13/13. Re-run formal Identity on the integrated candidate to identify the failing marker stage and verify the fix. PR #113 remains diagnostic only. PR #112's older ADB root/startup implementation was not copied over the stricter R3 implementation; its transient Package Manager readiness retry was selectively adapted into `tools/test_personal_beta_identity.sh`.
- PR100_PROVENANCE: PR #100 overlaps the Candidate D editor state. Its stale local `rememberSaveable` Composer value was excluded because Candidate D/R3 owns `TextFieldValue` and generations in `ChatViewModel`. This preserved the tested D implementation and avoided a second editor state source.
- R4_R5_RUN_HISTORY: Run `40210852450` is on v2 commit `d449f22c5d05597d7d02529e19ea9a92f608567d`; its Android job is serialized API 26 then API 35. Fast CI Run `40210852435` failed the commit identity guard on the v2 history. Do not treat that as a source/test failure. Full CI `40210852428`, Native `40210852427`, and SQLite `40210852471` were cancelled to control duplicate work and must be rerun on the final integrated SHA.
- CURRENT_BLOCKER: Formal Identity seed/claim precondition fails on prior v2 source SHA. The precise failed stage is UNKNOWN; a test-only operation label is now present in the candidate. Local Kotlin compile cannot run because `/usr/lib/jvm/java-21-openjdk-amd64` has no `javac`. The Android workflow run on old SHA `d449f22c5d05597d7d02529e19ea9a92f608567d` is failed overall: API26/35 campaigns and Pixel7a CI bundle pass; formal Identity and Required Gate fail.
- NEXT_ACTION: PR #100 head remains `ad868af753672c546c52411ce151904360633acc`; R3 head remains `93e2da60ed1b1ea05e6fdce04076800214d8a833`. Push the clean-history integration branch and create a Draft PR based on PR #100. Run hosted Build/Lint and formal Identity on this exact candidate, identify the marker failure stage, and make a minimal evidence-led correction. Then run the full Required/CI set on the settled integrated SHA. Keep PR #116 open until its Android run is complete; supersede it only after the replacement Draft PR exists.
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
| Composer composition failures in the old formal test | TEST_HARNESS strongly supported; product root cause UNKNOWN | Directly created InputConnection was not the Android-bound IME session. R3 and current API 26 R4 evidence show real IME composition passes. API 26 and 35 R4/R5 evidence now confirms both APIs pass; no product issue reproduced. |
| Calendar SnapshotStateObserver exception | TEST_HARNESS strongly supported | Earlier exception came from Compose semantics/touch failure diagnostics crossing SnapshotStateObserver thread ownership. UiAutomator gesture path and API 26 focused/class/full pass without recurrence. API 26 and 35 Calendar focused/class/full reruns pass without recurrence. |
| API 26 first date-change swipe did not advance | TEST_HARNESS supported | Previously occurred 9 ms after date query on API 26 while API 35 passed. Post-query Compose and UI idle boundaries now yield API 26 full 105/105. API 26 and 35 Calendar focused/class/full reruns pass without recurrence. |
| Formal Identity Required | UNKNOWN / not yet run on integrated candidate | Run the existing Android Required workflow on the final integration SHA. |

The prior Identity failure is classified `TEST_HARNESS / UNKNOWN` pending precise stage evidence. The emulator was online (API 35, QEMU, boot complete), Beta v100 installed successfully, and the failure came from the debug-only process-death authorization marker workflow. The artifact status was `NOT_RUN`; it did not claim a pass. The diagnostic change logs only one of fixed stage labels and the exception type, never the exception message, stack trace, or fixture data.

## Owner Handoff State

- The unpushed clean-history integration branch is local and reproducible. All four commits introduced over PR #100 head pass `tools/check_commit_identity.py` and use `Codex <noreply@github.com>` identity.
- No Required gate has been weakened and no full formal gate has been claimed from the prior R3 diagnostics.
- Personal Beta remains NO-GO until every exact-SHA gate passes, physical reacceptance is reviewed as applicable, and the Owner approves GO.
