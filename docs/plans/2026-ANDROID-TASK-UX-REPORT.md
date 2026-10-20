# E: Local Tasks Date/Time Picker / Japanese Validation

## Status

- Implementation branch: `codex/android-q4-task-date-time-2026-10-20`
- Pinned start: `239aaa181cdcfea756cf6c9ebb5eca9194caf7cf`
- PR #58 was verified OPEN and draft with head `239aaa181cdcfea756cf6c9ebb5eca9194caf7cf`; its head branch is the requested PR base.
- The local base tracking ref matched that same SHA at implementation start.
- Draft PR and CI run: pending publication; no run ID or result is available yet.
- Pixel 7a manual acceptance: `NOT_RUN`.

## Implementation

- Added Material 3 date and time picker controls to the Local Tasks editor. Date selections are converted from UTC picker milliseconds to `LocalDate` in UTC, preserving the selected calendar day.
- Kept manual ISO date and strict 24-hour `HH:MM` entry. A date with no time remains an all-day deadline; time without a date is rejected.
- The IANA timezone field uses the device `ZoneId.systemDefault()` as its initial value and is shown with an example. Validation gives separate Japanese title, date, time, timezone, DST-gap, and DST-overlap feedback with polite live-region semantics.
- Existing `rememberSaveable` draft fields remain in place. Picker cancellation does not commit a selection.
- `TaskValidation`, Task database/repository, CAS, encryption, and ViewModel were not edited.

## Files

- `android/app/src/main/java/app/serea/android/tasks/TaskScreen.kt`
- `android/app/src/androidTest/java/app/serea/android/tasks/TaskDateTimePickerTest.kt`
- `docs/plans/2026-ANDROID-TASK-UX-REPORT.md`

No files outside this lane's Task UI, matching instrumentation test, and lane report were edited. The existing `TaskScreenTest` remains unchanged to preserve the Android workflow's exact four-test class-count check.

## Verification

- `git diff --check`: PASS before report creation.
- Android Gradle verification: `NOT_RUN`. Attempted from `android/` with `./gradlew :app:testDebugUnitTest :app:assembleDebug :app:lintDebug`; the wrapper could not download Gradle 8.11.1 because the connection to `services.gradle.org` was refused. No installed `gradle` executable was available.
- New instrumentation cases cover picker cancellation, unchanged all-day draft state, and Japanese rejection of time without a date. These cases have not executed locally.
- CI: pending Draft PR creation. The Android workflow selects all Android paths; the new tests use a separate class so the existing `TaskScreenTest=4` verifier rule remains valid.
- Live Google OAuth/API, AI provider, emulator/device, and Pixel 7a: `NOT_RUN`.

## Security and integration

- No live OAuth, Gmail/Calendar, AI provider, credential, token, or real user data was used.
- No Manifest, Gradle dependency, Workflow, signing, database/migration, crypto, repository, or CAS changes were made.
- The task editor remains UI-only and calls the existing `parseTaskDraft` and ViewModel save path after the field-specific checks. No storage contract or DST validation was weakened.
- Final commit SHA, tree SHA, actual PR URL, and final-head CI results are recorded in the task completion response after publication.
