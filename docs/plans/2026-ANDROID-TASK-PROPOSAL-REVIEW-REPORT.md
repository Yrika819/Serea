# Android TaskProposal Review Report

Date: 2026-10-20
Lane: F — TaskProposal Review / Explicit Approval Component

## Git and delivery

- Source of truth checked: PR #58 is OPEN and Draft at head `239aaa181cdcfea756cf6c9ebb5eca9194caf7cf` on `codex/android-m0-08-abcd-integration-2026-10-20`.
- New lane branch: `codex/android-q4-taskproposal-review-2026-10-20`, created from the exact source SHA above.
- Draft PR: https://github.com/Yrika819/Serea/pull/60 (OPEN, Draft).
- PR base: `codex/android-m0-08-abcd-integration-2026-10-20` at `239aaa181cdcfea756cf6c9ebb5eca9194caf7cf`. Final follow-up HEAD and tree SHA are reported in the completion handoff because a report commit cannot contain its own commit SHA.
- No existing branch or PR was changed.

## Changes

- Added an ephemeral review session that validates proposals, captures the target task and revision, expires after five minutes, and only calls `TaskProposalConfirmer` after explicit approval. The approval API requires the caller to pass the current selected Task ID, including explicit `null` for create.
- Added a Compose review screen that shows operation, task ID, expected revision, and old-to-new values. Delete requires an additional acknowledgement keyed to the exact session object. An expiry timer updates the observable UI state and tells the user to prepare and approve a fresh review. UI results contain operation/status only; the component does not log task text or exception details.
- Added 14 unit-test methods using an in-memory fake repository and two Compose instrumentation tests. Coverage includes no-write-before-approval, decline/approval races, duplicate approval on one session, expiry and fresh approval, target/revision conflicts, update/complete/delete/create, create idempotency, session recreation, delete acknowledgement reset, and invalid proposal shapes.
- Corrected the delete-approval test ordering: it now checks the task remains after an unacknowledged approval attempt, then deletes it only after a separate review session receives explicit acknowledgement. The deleted task is no longer prepared or asserted present afterward.
- Changed files are limited to the new `android/app/src/main/java/app/serea/android/assistant/taskproposal/` package, its unit and instrumentation test packages, and this report. No task domain, repository, Tasks screen, database, Activity, provider, Gradle, manifest, workflow, or security-contract files were edited.

## Status

- Implemented: independent review UI/session, one in-flight approval guard, explicit selected Task ID contract, separate per-session delete acknowledgement, five-minute expiration with UI state refresh, target ID/revision binding, generic result states, fake repository and Compose tests.
- Not implemented: generation from real Chat/AI output, MainActivity or TaskScreen integration, provider access to TaskRepository, autonomous execution. These remain `NOT_IMPLEMENTED`.
- Owner decision: future integration must decide how fresh AI proposals enter the review flow and how the UI is routed. No dependency changes are requested.

## Verification

- `git diff --check`: PASS.
- New unit and instrumentation compile command: `./gradlew --no-daemon :app:testDebugUnitTest --tests 'app.serea.android.assistant.taskproposal.TaskProposalTest' :app:assembleDebugAndroidTest` — `NOT_RUN`; Gradle 8.11.1 wrapper download failed with `java.net.ConnectException: Connection refused` before project configuration or test execution.
- Android build, instrumentation execution, and lint: `NOT_RUN` for the same unavailable Gradle wrapper.
- New test class is selected by the existing Android workflow filter `--tests '*TaskProposalTest'`. No workflow changes were made.
- Existing regression tests: `NOT_RUN`; no test result is claimed as PASS.
- Identity preflight: repository-local Codex identity, `user.useConfigOnly=true`, `push.default=nothing`, and versioned hooks are configured. `tools/git_preflight.py --mode commit` passed at the source SHA before implementation. `tools/check_commit_identity.py` could not establish a bounded scope in the local environment because no supported `SEREA_EVENT_NAME` is supplied; this is an environment limitation, not a passing result.
- Real AI, Google OAuth/API, credentials, device, APK install, and production provider behavior: `NOT_RUN` by design.

## Security and integration notes

- No provider receives a repository reference. Proposal preparation reads only the exact target task needed for review. Writes occur only after explicit approval.
- Update/complete/delete rely on the existing repository expected-revision/CAS contract. Creates use `createFromApprovedProposal` and its durable proposal receipt to prevent replay. Each review session is ephemeral and is not parcelable or persisted; restored UI must prepare a fresh review and receive fresh approval.
- Existing encrypted task storage, database schema/migrations, Android Keystore, manifest restrictions, egress/OAuth policies, and workflow contracts were not edited.
- Future integration can depend on the new `assistant.taskproposal` package. It must provide a fresh proposal, current selected target ID, and an explicit approval callback. No Gradle or CI dependency change is required.
- Build and runtime behavior remain unverified until the Gradle wrapper can be obtained and the affected tests/build/lint run.
