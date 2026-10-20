# E: Local Tasks Date/Time Picker / Japanese Validation

## Status

- Implementation branch: `codex/android-q4-task-date-time-2026-10-20`
- Pinned start: `239aaa181cdcfea756cf6c9ebb5eca9194caf7cf`
- PR #58 was verified OPEN and draft with head `239aaa181cdcfea756cf6c9ebb5eca9194caf7cf`; its head branch is the requested PR base.
- The local base tracking ref matched that same SHA at implementation start.
- Draft PR: [#61](https://github.com/Yrika819/Serea/pull/61).
- PR base SHA: `239aaa181cdcfea756cf6c9ebb5eca9194caf7cf`.
- PR #61 remains OPEN and Draft, based on `codex/android-m0-08-abcd-integration-2026-10-20` at `239aaa181cdcfea756cf6c9ebb5eca9194caf7cf`.
- Current audit adds constrained editor scrolling and additional instrumentation coverage; its final HEAD and final-head CI status are recorded in the completion response after push.
- Pixel 7a manual acceptance: `NOT_RUN`.

## Implementation

- Added Material 3 date and time picker controls to the Local Tasks editor. Date selections are converted from UTC picker milliseconds to `LocalDate` in UTC, preserving the selected calendar day.
- Kept manual ISO date and strict 24-hour `HH:MM` entry. A date with no time remains an all-day deadline; time without a date is rejected.
- The IANA timezone field uses the device `ZoneId.systemDefault()` as its initial value and is shown with an example. Validation gives separate Japanese title, date, time, timezone, DST-gap, and DST-overlap feedback with polite live-region semantics.
- Existing `rememberSaveable` draft fields remain in place. Picker cancellation does not commit a selection.
- The editor body has a bounded height, vertical scrolling, and IME inset padding so its fields remain reachable in a compact dialog while the keyboard is shown.
- `TaskValidation`, Task database/repository, CAS, encryption, and ViewModel were not edited.

## Files

- `android/app/src/main/java/app/serea/android/tasks/TaskScreen.kt`
- `android/app/src/androidTest/java/app/serea/android/tasks/TaskDateTimePickerTest.kt`
- `docs/plans/2026-ANDROID-TASK-UX-REPORT.md`

No files outside this lane's Task UI, matching instrumentation test, and lane report were edited. The existing `TaskScreenTest` remains unchanged to preserve the Android workflow's exact four-test class-count check.

## Verification

- `git diff --check`: PASS for the audit changes.
- Android Gradle verification: `NOT_RUN`. Attempted from `android/` with `./gradlew :app:testDebugUnitTest :app:assembleDebug :app:assembleDebugAndroidTest :app:lintDebug`; the wrapper could not download Gradle 8.11.1 because the connection to `services.gradle.org` was refused. No installed `gradle` executable was available.
- Six instrumentation cases cover a fixed DatePicker selection (`2026-07-15`) and date-only save, a TimePicker dial selection (`09:35`) and exact saved-time equality, existing-task update with exact ID/revision/draft assertions, invalid IANA zone rejection, DST gap/overlap rejection, and time-without-date rejection. The recording Fake Repository verifies create/update calls and captured drafts. The date picker case also exercises scrolling to the lower editor fields.
- API 26 and API 35 instrumentation execution status for the final HEAD is recorded in the completion response from the Android workflow artifacts.
- The Android workflow selects all Android paths. The new tests remain in `TaskDateTimePickerTest`, preserving the existing exact four-test `TaskScreenTest` verifier rule.
- Live Google OAuth/API, AI provider, emulator/device, and Pixel 7a: `NOT_RUN`.

## Security and integration

- No live OAuth, Gmail/Calendar, AI provider, credential, token, or real user data was used.
- No Manifest, Gradle dependency, Workflow, signing, database/migration, crypto, repository, or CAS changes were made.
- The task editor remains UI-only and calls the existing `parseTaskDraft` and ViewModel save path after the field-specific checks. No storage contract or DST validation was weakened.
- The task completion response records the final report commit SHA/tree and its CI status after publication.
