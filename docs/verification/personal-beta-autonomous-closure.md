# Serea Personal Beta Autonomous Closure Checkpoint

## Current State

- ACTIVE_PHASE: R4 / R5 focused verification
- GOAL_STATUS: IN_PROGRESS
- AUTHORITY_SHA: `93e2da60ed1b1ea05e6fdce04076800214d8a833` (R3 real-IME baseline; descendant of Candidate D `dc98547b256c3a2c1551814cdcf4a07ebc2373c8`)
- WORKTREE: `/workspace/Serea-r4-r5`
- OWNED_BRANCH: `codex/serea-r4-r5-required-harness`
- LAST_GOOD_SHA: `93e2da60ed1b1ea05e6fdce04076800214d8a833`
- LATEST_CI_RUN: `40203437416` (R3 non-Required real-IME success; no CI run yet for this checkpoint)
- CURRENT_BLOCKER: Focused API 26/35 and Shell class execution are pending.
- NEXT_ACTION: Build and run the focused formal Chat/Calendar instrumentation on disposable API 26 and API 35 emulators; analyze first failures, then run the formal Required workflow only after focused/class are green.
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

- `:app:assembleIsolatedAndroidTest :r25-ime-test:assembleDebug`: PASS on the current uncommitted implementation after correcting a duplicate helper name.
- `PYTHONPATH=. python3 tests/android_emulator_workflow_tests.py`: PASS, 26 tests.
- `bash -n tools/run_android_instrumentation_campaign.sh` and `git diff --check`: PASS.
- Required test identity manifests remain 103 + 2; no manifest, test-count, gate-threshold, or skip-condition file was changed.
- Local emulator acceleration is unavailable (`emulator -accel-check`: CPU has no VMX/SVM); hosted KVM CI is required for API Focused execution.

## Failure Classification / Hypotheses

| Issue | Classification | Evidence | Next observation |
|---|---|---|---|
| Formal Chat composition failures | TEST_HARNESS strongly supported; product UNKNOWN | Directly constructed connection was not Android's bound IME; R3 B/C pass on both APIs | Run formal method through system-bound Test IME on both APIs |
| API 26 Calendar SnapshotStateObserver | TEST_HARNESS strongly supported | Stack originates from semantics tree diagnostic traversal called by `performTouchInput`; API 35 class passes; scrolls advanced before exception | Run Calendar class with UiAutomator touch dispatch and check exception absence |
| Identity integration / Required | UNKNOWN | PR #113 is diagnostic only; formal Identity gate not yet run on a unified candidate | Audit dependencies and validate formal Required gate during R6/R7 |

## Handoff

Do not infer R4/R5 PASS from compile or R3. Preserve this file at each milestone with exact branch HEAD, Tree SHA, PR/Run/Artifact IDs, and the next failed event. Proceed to R6 only after R4 and R5 API 26/35 Focused + Shell Class succeed; proceed to R7 only on a provenance-audited unified candidate.
