# Serea Night Lane 5 — TaskProposal Approval UX Hardening

Date: 2026-10-20

## Branch and base

- Branch: `codex/android-q4-night-taskproposal-ux-2026-10-21`
- Source: PR #66, confirmed `OPEN` / `Draft`
- Pinned source HEAD: `072aa2b823523f7a92196299dd86cfb3509a78e3`
- Draft PR base: `codex/android-q4-abcdefg-integration-2026-10-20`
- Final HEAD, tree SHA, and PR URL are recorded in the completion message.

The source SHA is the head of PR #66 and the requested PR base branch. Work is isolated on the new branch; PR #66, `main`, and the integration branch were not edited.

## Changes

### Review screen

`TaskProposalReviewScreen` now presents CREATE, UPDATE, COMPLETE, and DELETE with Japanese operation names and a preview that mirrors the values passed to `TaskProposalConfirmer`:

- CREATE shows title, description, priority, due date/time/timezone, initial status/revision, and which ID/timestamps are generated at approval.
- UPDATE shows old and proposed values for every editable task field.
- COMPLETE shows the target title and the status transition.
- DELETE shows the title, target ID, expected revision, and a separate explicit acknowledgement.

Approval, rejection, close, and Back are separate actions. Buttons are disabled while processing; a synchronous UI action guard and the session mutex prevent duplicate submissions. Status messages are Japanese and exposed as a polite live region. Expired and conflicting reviews explain that the task must be fetched and validated again and cannot be retried from the same review.

### Isolated demo API

```kotlin
TaskProposalDemoScreen(
    onClose = { /* Navigation owner closes this demo route */ },
)
```

The demo owns a private in-memory `TaskRepository` implementation and fixed fictional task data. It displays `DEMO / 架空のタスク`, a write counter, and the fake task state. It does not construct or access `TasksDatabase`, `EncryptedTaskRepository`, the real task list, a provider, or chat. Each operation or explicit “new review” action prepares a new proposal/session. The demo is not wired into `MainActivity` or `TasksActivity`.

### Review-session boundary

`prepare` remains read-only. It now rejects a repository response whose task ID differs from the requested proposal target. A missing task during execution is presented as Conflict. Cancellation marks a session Rejected inside `NonCancellable`, so a call that may have crossed the write boundary cannot be retried with that session.

The `TaskRepository`, DAO, schema, and production CAS contract were not changed. The proposal and review session remain non-persistable; approval is never restored through Parcelable or SavedState.

## Adversarial tests

`TaskProposalTest` now has 20 JVM methods using only a synchronized synthetic `FakeTaskRepository` with a write counter. Coverage includes:

1. CREATE, UPDATE, COMPLETE, and DELETE all have zero writes before explicit approval.
2. DELETE without its extra acknowledgement has zero writes.
3. Decline, non-approval, and close leave the write count at zero.
4. Expiration at the exact deadline and clock rollback cannot revive a session.
5. A wrong selected ID or a mismatched task returned from `get` is rejected.
6. Stale revisions fail both before review and at the repository CAS boundary.
7. Double approval, concurrent approval/decline, and proposal-ID receipt checks allow at most one write.
8. Selection changes, an old proposal ID in a new session, and process recreation do not carry approval state forward.
9. Cancellation before a write and cancellation after a possible write both retire the session; retry cannot add another write.
10. A synthetic error after a fake write leaves the session unusable.

The existing `TaskProposalReviewScreenTest` still has exactly two AndroidTest methods. Their assertions now cover every operation preview, DELETE acknowledgement reset, close without a write, expiration guidance, and a burst of approval taps with one callback/write. The repository-wide AndroidTest method count remains 71.

## Independent audit passes

The final implementation was reviewed in four separate passes against bypass attempts, not only the expected flow.

- **Security:** Verified that `prepare` only validates and reads; repository mutation remains behind `approve` and `TaskProposalConfirmer`'s explicit approval check. Found and fixed the unchecked task ID returned by `get`. Verified delete acknowledgement, proposal/preview alignment, no saved approval state, and no provider/repository bridge.
- **Concurrency:** Verified the READY → EXECUTING single-use transition, mutex behavior, expiration boundary, close/decline races, duplicate taps, and late/cancelled results. Strengthened the synchronous UI guard and cancellation-state update. Fake tests cover cancellation both before and after a possible write.
- **Repository/CAS:** Re-read the production repository and DAO contracts. UPDATE/COMPLETE use expected revision through CAS; DELETE checks expected revision atomically; CREATE proposal receipts remain atomic. No production repository/CAS code changed. Fake failure tests show no stale-session retry.
- **UX/accessibility:** Found English status copy, incomplete CREATE/COMPLETE preview details, and no explicit close action. Replaced the copy with Japanese guidance, completed the previews, added close/Back behavior, a live-region result, a heading semantic, a labeled DELETE acknowledgement, and full-width action buttons.

No unresolved approval bypass was found in the reviewed scope.

## Verification

- `:app:testDebugUnitTest`: PASS; 172 tests across 15 suites, 0 failures/errors. The TaskProposal suite passed all 20 methods.
- `:app:assembleDebug`: PASS.
- `:app:assembleRelease`: PASS.
- `:app:assembleDebugAndroidTest`: PASS (APK compile only).
- `:app:lintDebug`: PASS.
- `git diff --check`: PASS.
- Instrumentation execution on a device/emulator: `NOT_RUN`; `adb devices -l` found no attached device. The two screen tests use only a Fake Repository, and the AndroidTest APK compiled successfully.
- CI: checked after the Draft PR is created; any pending status is recorded in the completion message.

Git identity is configured and authenticated GitHub CLI access is available. `core.hooksPath` and signing configuration were not changed; the repository `.githooks` directory was left untouched.

The executor initially had a JRE without `javac`. A temporary Adoptium JDK under `/tmp` and the system CA trust store were used for local verification; no Gradle or environment configuration was committed.

## Navigation handoff

The Navigation owner can expose the demo by adding a route that calls `TaskProposalDemoScreen(onClose = { navController.popBackStack() })`. For a real review route, the caller must prepare a new `TaskProposalReviewSession` from a validated proposal and repository for each review, then pass the session, current selected task ID, result callback, and close callback to `TaskProposalReviewScreen`. Do not save or parcel the session. No real chat/provider proposal generation path is part of this lane.
