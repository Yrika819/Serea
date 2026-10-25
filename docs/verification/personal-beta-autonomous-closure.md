# Serea Personal Beta Autonomous Closure Checkpoint

## Current State

- ACTIVE_PHASE: R5 Calendar reliability; R4 formal IME conversion has passed its API 26 focused/Class and Full-105 method checks on the previous candidate.
- GOAL_STATUS: IN_PROGRESS
- AUTHORITY_SHA: `93e2da60ed1b1ea05e6fdce04076800214d8a833` (R3 real-IME baseline; descendant of Candidate D `dc98547b256c3a2c1551814cdcf4a07ebc2373c8`). GitHub main is still `e0acb0254820c381dd97828824bef86a1db5176c`; R3 remote head still matches the authority.
- WORKTREE: `/workspace/Serea-r4-r5-v2`
- OWNED_BRANCH: `codex/serea-r4-r5-required-harness-v2`
- LAST_IMPLEMENTATION_COMMIT: `8759b1d179a94a2d586fbbe596af2870ada9a934`, tree `3830c438668c39db173dfd3eb0f8f931eb5a7792` (local commit; not yet pushed). A following doc-only checkpoint records this state. PR #116 still points to parent `9c8cf37a73e82453dbb5aaa98dc7094828e2114d`.
- CURRENT_UNCOMMITTED_WORK: none; owned worktree is clean. Commit `8759b1d` adds the explicit non-required test identity manifest/runner/verifier integration and Calendar keyboard dismissal barrier.
- LATEST_CI_RUN: `40207352360` on parent commit `9c8cf37a73e82453dbb5aaa98dc7094828e2114d`. API 26 artifact ID `12605137672` is saved locally at `/workspace/attachments/be620694-9368-4cbe-8915-6c8dfefa1a97/serea-android-instrumentation-api26-40207352360.zip`. API 35 is still in progress. R3 successful real-IME baseline remains Run `40203437416`.
- API26_EVIDENCE: real-IME Chat focused 1/1 PASS; Calendar class 19/19 PASS; ChatScreen class 16/16 PASS; full Required JUnit ran 105 with 0 skips and failed only `calendarDetailAndSelectedCalendarSurviveAnotherDestination`; Data Isolation, Process Death, Test IME cleanup passed. The full verifier separately rejected 5 R3 exploratory source identities because they were not in either formal manifest.
- CURRENT_BLOCKER: API 26 full-set Calendar first swipe did not advance the LazyList during the full 105 run, although Calendar class passed. The UI log places this about 9 ms after the date-query/IME stage. Product cause remains UNKNOWN; suspected synchronization race at the test harness boundary. API 35 results for this same committed source are pending.
- NEXT_ACTION: Finish the API 35 baseline run, push commit `8759b1d` to PR #116, and run the updated campaign on API 26/35. Advance only after Focused, Shell Class, and full-set Calendar behavior are stable. Keep 105 Required identities and thresholds unchanged.
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

## Local Verification

- `:app:compileIsolatedAndroidTestKotlin :r25-ime-test:assembleDebug`: PASS.
- `:app:compileIsolatedAndroidTestKotlin :app:packageIsolatedAndroidTest`: PASS; APK contains the explicit 5-identity diagnostic asset (local Test APK SHA-256 `be430440dd28149079f7dd78cd25d085216fbd6b7e6663f07c0570053549f21a`).
- `PYTHONPATH=. python3 tests/android_emulator_workflow_tests.py`: PASS, 26 tests; `PYTHONPATH=. python3 tests/android_instrumentation_results_tests.py`: PASS, 13 tests.
- `bash -n tools/run_android_instrumentation_campaign.sh` and `git diff --check`: PASS.
- Re-running the verifier on the preserved API 26 105-test JUnit artifact now accepts the full source inventory (105 formal + 5 explicitly listed non-required diagnostics = 110); it correctly still FAILs on the single Calendar swipe assertion, with 0 skips.
- `:app:lintIsolated`: BLOCKED by dependency resolution TLS certificate validation while fetching uncached `androidx.arch.core:core-testing:2.2.0` from Google Maven. This is an environment failure; the hosted Android build/lint job on the committed HEAD passed.
- Required test identity manifests remain 103 + 2; no manifest, test-count, gate-threshold, or skip-condition file was changed.
- Local emulator acceleration is unavailable (`emulator -accel-check`: CPU has no VMX/SVM); hosted KVM CI is required for API Focused execution.

## Failure Classification / Hypotheses

| Issue | Classification | Evidence | Next observation |
|---|---|---|---|
| Formal Chat composition failures | TEST_HARNESS strongly supported; product UNKNOWN | Directly constructed connection was not Android's bound IME; R3 B/C pass on both APIs | Run formal method through system-bound Test IME on both APIs |
| API 26 Calendar SnapshotStateObserver | TEST_HARNESS strongly supported; prior exception absent | Stack originated from semantics tree diagnostic traversal called by `performTouchInput`; updated UiAutomator Calendar class passed 19/19 with no recurrence | Confirm in updated full-set execution |
| API 26 Calendar date-change first swipe | TEST_HARNESS hypothesis, not confirmed | Calendar class passed; full-set run failed because first UiAutomator swipe did not advance the list immediately after IME/date update; old code requested `hideSoftInputFromWindow` asynchronously | Use Espresso keyboard close plus Compose idle barrier, then compare full-set swipe progression |
| Source inventory mismatch | TEST_HARNESS confirmed | Formal source inventory contained 5 known R3 diagnostics, while the verifier only allowed 103+2 formal identities; JUnit itself had exactly 105 tests | Add an explicit diagnostic identity asset; runner and verifier read the same list; JUnit identities remain exactly 105 |
| Identity integration / Required | UNKNOWN | PR #113 is diagnostic only; formal Identity gate not yet run on a unified candidate | Audit dependencies and validate formal Required gate during R6/R7 |

## Handoff

Do not infer R4/R5 PASS from compile or R3. Preserve this file at each milestone with exact branch HEAD, Tree SHA, PR/Run/Artifact IDs, and the next failed event. Proceed to R6 only after R4 and R5 API 26/35 Focused + Shell Class succeed; proceed to R7 only on a provenance-audited unified candidate.
