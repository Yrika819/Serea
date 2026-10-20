# Android Chat History UX Report

## Scope and Git

- Task: D — Chat History UX / Empty Conversation Hardening.
- Required base branch: `codex/android-m0-08-abcd-integration-2026-10-20`.
- Verified base SHA: `239aaa181cdcfea756cf6c9ebb5eca9194caf7cf` (matches PR #58 head).
- Work branch: `codex/android-q4-chat-history-ux-2026-10-20`, created from the verified SHA.
- Draft PR: https://github.com/Yrika819/Serea/pull/63 (Open, Draft).
- This follow-up audits PR #63 at the requested starting HEAD `9328c363d4d79cd1b9de8c7adc49f6d068f5538a`, with the same verified base SHA above.

## Implementation

- New conversations remain in memory until the first user message is sent. Startup with no saved conversations, opening another empty conversation, and starting fresh after deletion no longer write empty rows.
- The first user message and its encrypted title are written together in one Room transaction. A failed first request retains the user turn and transitions its status through the existing failure/unknown handling.
- Titles are generated locally from the first message, whitespace and control characters are normalized, and text is capped at 48 Unicode code points. The existing encrypted title column and Android Keystore encryption are reused. The app UI displays the title; no title text is added to logs, notifications, or test diagnostics.
- Legacy `Conversation <8 hex>` titles remain readable and are shown as-is when the stored conversation has no user message. When a user message exists, the history UI can show its derived title without rewriting the stored row.
- The current conversation name is visible in chat and in the current-conversation deletion confirmation. Conversation selection, history display, startup restore, deletion completion, and history refresh use generation checks to prevent stale asynchronous results from replacing a later selection. `openConversation()` checks again after pending-message status updates, which can suspend.
- Title length is defined as at most 48 Unicode code points, including the ellipsis. A truncated title uses 47 source code points followed by `…`; an untruncated title can use all 48.
- History schema remains version 4. No migration, Manifest, Gradle, Workflow, task, calendar, Gmail, Google authorization, provider, usage, or signing files were changed.

## Verification

- `git diff --check`: PASS.
- `./gradlew --no-daemon testDebugUnitTest --tests '*ChatViewModelTest' assembleDebug lintDebug`: PASS. `ChatViewModelTest`: 17 tests, 0 failures, 0 errors, 0 skipped. Debug APK build and `lintDebug` passed.
- `./gradlew --no-daemon assembleDebugAndroidTest`: PASS; Android instrumentation sources compiled. Runtime instrumentation was NOT_RUN.
- `ChatHistoryUxTest` now uses only an in-memory Room database and a unique Android Keystore alias. It does not open or clear the production `serea-history.db`; it checks the named app database file metadata is unchanged before and after each test. It covers the delayed A-then-B conversation selection race and transaction rollback when insertion of the first message fails.
- Safe non-CI invocation condition: only the isolated `ChatHistoryUxTest` class may be run on a device with real app data. The broader `ChatScreenTest` suite still uses app history and must only run on a disposable CI emulator or an otherwise empty test installation. Never run instrumentation on Pixel 7a or a device holding user history.
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
