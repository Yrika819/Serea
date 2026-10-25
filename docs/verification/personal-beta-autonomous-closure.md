# Serea Personal Beta Autonomous Closure Checkpoint

## Current State

- ACTIVE_PHASE: R5 Calendar reliability; R4 formal IME conversion has passed its API 26 focused/Class and Full-105 method checks on the previous candidate.
- GOAL_STATUS: IN_PROGRESS
- AUTHORITY_SHA: `93e2da60ed1b1ea05e6fdce04076800214d8a833` (R3 real-IME baseline; descendant of Candidate D `dc98547b256c3a2c1551814cdcf4a07ebc2373c8`). GitHub main is still `e0acb0254820c381dd97828824bef86a1db5176c`; R3 remote head still matches the authority.
- WORKTREE: `/workspace/Serea-r4-r5-v2`
- OWNED_BRANCH: `codex/serea-r4-r5-required-harness-v3`
- LAST_IMPLEMENTATION_COMMIT: `fd8ea17d2474d173e8216b8ab5cd3595a1549f5f`, tree `32a53ac02c4a9d4f8f74ef503f45aa83ada28daa` (new clean-history commit with approved `noreply@github.com` identity; local until current API 26/35 campaign finishes). PR #116 has the identical tree on a history rejected by Fast CI; create a replacement Draft PR for this branch without force-pushing.
- CURRENT_UNCOMMITTED_WORK: none; the owned worktree is clean. The replacement branch contains the same R4/R5 implementation and test harness tree as PR #116, squashed into one implementation commit with an accepted identity.
- LATEST_CI_RUN: `40210852450` is in progress on commit `d449f22c5d05597d7d02529e19ea9a92f608567d` (same tree as the v3 implementation commit). Fast CI Run `40210852435` failed solely because the seven v2 commits used an unapproved personal email. API26/35 artifact results for the new idle barrier are pending. Previous baseline artifacts: API 26 ID `12605137672` at `/workspace/attachments/be620694-9368-4cbe-8915-6c8dfefa1a97/serea-android-instrumentation-api26-40207352360.zip`; API 35 ID `12605199696` at `/workspace/attachments/fcabc84c-668c-4306-8c7e-69a68f66671a/serea-android-instrumentation-api35.zip`. R3 successful real-IME baseline remains Run `40203437416`.
- API26_EVIDENCE: real-IME Chat focused 1/1 PASS; Calendar class 19/19 PASS; ChatScreen class 16/16 PASS; Full Required JUnit 105 tests, 0 skips, 1 Calendar first-swipe failure; Data Isolation, Process Death, Test IME cleanup PASS.
- API35_EVIDENCE: real-IME Chat focused 1/1 PASS; Calendar class 19/19 PASS; ChatScreen class 16/16 PASS; Full Required JUnit 105/105, 0 failures/errors/skips; Data Isolation, Process Death, Test IME cleanup PASS. API 35's CI verifier failed only because it did not yet accept the five explicit non-required source identities. Re-running that exact JUnit with the updated verifier passes 103/103 + 2/2 and API 35 ProfileVerifier evidence.
- API35_IME_TRACE: Test IME `onStartInput`/`onStartInputView` targeted `app.serea.android.isolated` with `inputType=163841`; B/C logged composing ranges `0..5`, replacement `0..25`, and finish with composition `null`; Gmail/Calendar return and recreation insertion reached the asserted text/selection. Default Latin IME was restored and Test IME package removed.
- SAME_REVISION_COMPARISON: both APIs used source SHA `9c8cf37a73e82453dbb5aaa98dc7094828e2114d`, target APK SHA-256 `c3d4e6f9dcecd8776b1345ab9c452c5cd7a2b214210dd9bdb7ca1b91a4c61a22`, instrumentation APK SHA-256 `ed6511339554a9ddfcf15b79f58c6556960765602b36c95b4d2d810029008d49`, Test IME APK SHA-256 `f2b8645396d2f6c7d6d4db7d7b4ba7dc4d3569e29f1feda1f8b2bb9cb130529b`.
- CURRENT_BLOCKER: API 26 Full Required had one Calendar first-swipe non-advance; API 35 passed the same class and Full suite on identical APKs. The API 26 UI trace shows the first gesture started about 9 ms after the date-query stage and did not move the LazyList. This supports an API 26-specific or intermittent test synchronization issue, not a confirmed product bug. The new Compose/UI idle barriers are compiled but await focused emulator validation.
- NEXT_ACTION: Finish the in-progress API 26/35 campaign on the same tree, push the approved-history branch as a replacement Draft PR, then rerun required checks against that final SHA. Advance only after Focused, Shell Class, and full-set Calendar behavior are stable. Keep 105 Required identities and thresholds unchanged.
- REQUIRED_GATES: 103 Original Required + 2 Additional (105 total), API 26 and API 35, 0 failures/errors/skips; Process Death; Data Isolation; Sentinel; Identity Required; Android Build/Unit/Integration/Security/Dependency Audit; Pixel 7a CI reacceptance bundle; Native/Fast/Full/SQLite/Git Hooks/RustSec.
- OWNER_APPROVAL_REQUIRED: Main merge, final Personal Beta GO, real-device destructive actions, production signing credentials, real Gmail/Calendar accounts, or changes to Required acceptance criteria.

## R3 Evidence Reconciled

- R3 source: `93e2da60ed1b1ea05e6fdce04076800214d8a833`, tree `39b5d60ec8c832847501d368c9050c9e35f2ff98`.
- Run `40203437416` passed the non-Required Android-managed IME B/C/D campaign on API 26 and API 35 using identical APKs.
- Existing formal test used `View.onCreateInputConnection(EditorInfo())` directly for its composition probes; this was not the Android-bound IME session.
- Formal Android run `40170431448` recorded the API 26 Calendar cross-thread SnapshotStateObserver exception during `SemanticsNodeInteraction.performTouchInput` failure reporting, after many successful scroll events. The API 35 Shell class recorded the same target test as passing.
- The API 26 formal Chat focused run recorded successful direct-connection composition initially, then `A-consecutive-composition-committed` timed out; API 35 timed out at `B-navigation-draft-settled`.

## Current R4 / R5 Changes

- Replace the formal Chat composition input with the standalone Android-managed R3 Test IME. The required method checks the IME package, active `InputConnection`, session generation, EditorInfo target, composition set/replace/finish, TextFieldValue, Compose selection, Gmail and Calendar return insertion, Activity recreation selection, and cleanup restoring the original IME.
- Install the isolated Test IME only in the campaign's disposable emulator and remove it on campaign exit. Capture its APK hash, lifecycle/output, and cleanup state.
- Keep the two exploratory R3 diagnostic classes outside default formal runs; explicit diagnostic class filters still run them. The 103+2 test manifests and acceptance thresholds remain unchanged.
- Replace Calendar `performTouchInput` swipes with UiAutomator touchscreen gestures using copied window bounds; continue asserting LazyList progress and intended event visibility.
- Update the Android workflow mock ADB fixture for the new read-only default-IME query.

## R6 Provenance Audit (Read Only)

- Current R3/R4/R5 history descends from Candidate D PR #111 and includes the complete PR #114 real-IME diagnostic implementation; both source ancestry checks pass. No PR #111/#112/#113/#114 branch was modified.
- PR #100 is based on `codex/pr98-overnight-gate-recovery` at `8140bd759e7a01041364ebc443ba452f546d325a`, head `ad868af753672c546c52411ce151904360633acc`; its broad diff spans Android product code, tests, workflow and Rust host/storage. Its older Composer selection/save-state changes replace the Candidate D ViewModel contract. Do not merge or cherry-pick it wholesale.
- PR #112 is based on the same `8140bd...` ancestry and has one distinct commit `3d80b68881dae236f7f192ca1d6252aab464abf3` touching only `tools/test_personal_beta_identity.sh` (109 additions, 10 deletions). Evaluate this isolated hardening patch for the R6 integration branch after R4/R5 closure.
- PR #113 is based on PR #112 and adds a non-required API 35 diagnostic workflow/script. Its independent validation passed in Run `40167074114`; the formal identity job was skipped and the Android Required Gate failed in Run `40167818895` because the emulator gate did not pass. This is diagnostic evidence only.
- PR #114 is open as a separate Draft, but its head `4d4a6bbb1b9350edfe523a4a00920b8c3c5dd828` is an ancestor of R3; no extra cherry-pick is needed.

## Local Verification

- `:app:compileIsolatedAndroidTestKotlin :r25-ime-test:assembleDebug`: PASS.
- `:app:compileIsolatedAndroidTestKotlin :app:packageIsolatedAndroidTest`: PASS; APK contains the explicit 5-identity diagnostic asset (Test APK SHA-256 after the idle barrier `6a25f759f2af7680a5b2b3d09830fade289f3193d4827e780081540a371dc0ff`).
- `PYTHONPATH=. python3 tests/android_emulator_workflow_tests.py`: PASS, 26 tests; `PYTHONPATH=. python3 tests/android_instrumentation_results_tests.py`: PASS, 13 tests.
- `bash -n tools/run_android_instrumentation_campaign.sh` and `git diff --check`: PASS.
- Re-running the updated verifier on the preserved API 26 JUnit accepts the source inventory (105 formal + 5 explicitly listed non-required diagnostics = 110) and correctly reports the one Calendar failure. Re-running it on API 35 accepts the source inventory and passes 105/105, including ProfileVerifier evidence.
- `:app:lintIsolated`: BLOCKED by dependency resolution TLS certificate validation while fetching uncached `androidx.arch.core:core-testing:2.2.0` from Google Maven. This is an environment failure; the hosted Android build/lint job on the committed HEAD passed.
- Required test identity manifests remain 103 + 2; no manifest, test-count, gate-threshold, or skip-condition file was changed.
- Local emulator acceleration is unavailable (`emulator -accel-check`: CPU has no VMX/SVM); hosted KVM CI is required for API Focused execution.

## Failure Classification / Hypotheses

| Issue | Classification | Evidence | Next observation |
|---|---|---|---|
| Formal Chat composition failures | TEST_HARNESS strongly supported; product UNKNOWN | Directly constructed connection was not Android's bound IME; R3 B/C pass on both APIs | Run formal method through system-bound Test IME on both APIs |
| API 26 Calendar SnapshotStateObserver | TEST_HARNESS strongly supported; prior exception absent | Stack originated from semantics tree diagnostic traversal called by `performTouchInput`; updated UiAutomator Calendar class passed 19/19 with no recurrence | Confirm in updated full-set execution |
| API 26 Calendar date-change first swipe | TEST_HARNESS hypothesis, not confirmed | Same source/APKs passed Calendar class and all 105 on API 35; API 26 class passed but one Full-set swipe did not advance immediately after date-query/IME; no IME window state was recorded | Updated candidate adds Espresso close plus Compose and platform UI/accessibility idle barriers; compare API 26 full-set progression |
| Source inventory mismatch | TEST_HARNESS confirmed | Both old API artifacts contained exactly 105 formal JUnit identities; source inventory also had 5 known R3 diagnostics, and the old verifier allowed only formal source | Explicit diagnostic identity asset accepted by the updated verifier; API 35 old JUnit now passes locally with it; ensure hosted gate uses updated source |
| Identity integration / Required | UNKNOWN | PR #113 is diagnostic only; formal Identity gate not yet run on a unified candidate | Audit dependencies and validate formal Required gate during R6/R7 |

## Handoff

Do not infer R4/R5 PASS from compile or R3. Preserve this file at each milestone with exact branch HEAD, Tree SHA, PR/Run/Artifact IDs, and the next failed event. Proceed to R6 only after R4 and R5 API 26/35 Focused + Shell Class succeed; proceed to R7 only on a provenance-audited unified candidate.
