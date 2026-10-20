# Android Chat History UX Report

## Scope and Git

- Task: D — Chat History UX / Empty Conversation Hardening.
- Required base branch: `codex/android-m0-08-abcd-integration-2026-10-20`.
- Verified base SHA: `239aaa181cdcfea756cf6c9ebb5eca9194caf7cf` (matches PR #58 head).
- Work branch: `codex/android-q4-chat-history-ux-2026-10-20`, created from the verified SHA.
- Draft PR URL, final HEAD, tree SHA, and GitHub Actions run IDs: recorded after publishing the branch and opening the draft PR.

## Implementation

- New conversations remain in memory until the first user message is sent. Startup with no saved conversations, opening another empty conversation, and starting fresh after deletion no longer write empty rows.
- The first user message and its encrypted title are written together in one Room transaction. A failed first request retains the user turn and transitions its status through the existing failure/unknown handling.
- Titles are generated locally from the first message, whitespace and control characters are normalized, and text is capped at 48 Unicode code points. The existing encrypted title column and Android Keystore encryption are reused. The app UI displays the title; no title text is added to logs, notifications, or test diagnostics.
- Legacy `Conversation <8 hex>` titles remain readable and are shown as-is when the stored conversation has no user message. When a user message exists, the history UI can show its derived title without rewriting the stored row.
- The current conversation name is visible in chat and in the current-conversation deletion confirmation. Conversation reads use a generation check so an older asynchronous selection cannot replace a newer one.
- History schema remains version 4. No migration, Manifest, Gradle, Workflow, task, calendar, Gmail, Google authorization, provider, usage, or signing files were changed.

## Verification

- `git diff --check`: PASS.
- `./gradlew --no-daemon testDebugUnitTest --tests '*ChatViewModelTest' assembleDebug lintDebug`: PASS. `ChatViewModelTest`: 17 tests, 0 failures, 0 errors, 0 skipped. Debug APK build and `lintDebug` passed.
- `./gradlew --no-daemon assembleDebugAndroidTest`: PASS; Android instrumentation sources compiled, including the dedicated `ChatHistoryUxTest` cases.
- Local API 26/35 instrumentation and Process Death: NOT_RUN. This workspace has no Android Emulator package or AVD. No physical device or ADB device was contacted.
- Pixel 7a acceptance: NOT_RUN.
- Live provider, API key, Google OAuth, Gmail, and Calendar traffic: NOT_RUN.
- GitHub Actions: pending branch publication; statuses will be checked against the submitted HEAD.

## Security and integration notes

- No real provider, OAuth, Gmail, or Calendar connection was enabled or invoked. The app remains wired to `MockModelChatPort()`.
- Titles and messages continue to use the existing AES-GCM Android Keystore encryption and authenticated data. The History DB v4 schema, Task DB v1, usage ledger, `PENDING`/`OUTCOME_UNKNOWN` flow, cancellation, timeout, recovery, and existing deletion transactions were not changed.
- The added instrumentation regressions live in a dedicated class, so exact class-count checks for existing `EncryptedHistoryTest` and `ChatScreenTest` selectors remain unchanged. The Android workflow summary currently prints the old aggregate count; the additional cases run under `connectedDebugAndroidTest` but the summary count should be updated by the integration owner when Workflow changes are permitted.
- This branch edits `MainActivity.kt`, which D owns. Other lanes should integrate their UI changes with this file; the downstream UI integration may need a conflict resolution. No dependency change is required.

## Remaining

- Record GitHub Actions run IDs/statuses for the final submitted HEAD after publication.
- API 26/35 instrumentation, Process Death, and Pixel 7a acceptance remain outstanding in an authorized environment.
