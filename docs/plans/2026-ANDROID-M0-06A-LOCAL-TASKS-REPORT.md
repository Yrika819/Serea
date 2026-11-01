# Android AND-M0-06A: standalone local tasks

Date: 2026-10-20
Repository: `Yrika819/Serea`
Base: `codex/android-m0-04-integration-candidate-2026-10-20` at `94492d281fe0ee583744c932259c3525d5c34f86`
Implementation validation SHA: `9e637c3f7ba228f1cf8c70babdb9205be76eb04a`
Working branch: `codex/android-m0-06a-local-tasks-2026-10-20-recovery`
Draft PR: [#56](https://github.com/Yrika819/Serea/pull/56), based on the requested integration-candidate branch.

## Result

The Android-only task feature is implemented under `app.serea.android.tasks`.
Its UI opens in a private `TasksActivity`; `MainActivity` has only a small
launch affordance. Tasks work without AI, a PC Core, cloud, Calendar, Gmail,
credentials, or network access.

The requested branch name was already used by PR #54, whose head contains a
commit rejected by the repository's commit-identity check. To avoid rewriting
or changing that existing PR, the validated implementation and PR were created
on the recovery branch above. PR #54 remains open and unchanged. PR #50 and
PR #51 also remain unchanged.

Local Android compilation was **NOT_RUN**: the execution container's configured
Java home has no `javac`, and its configured Android SDK directory is absent.
The GitHub Actions Android build and emulator checks passed on the validation
SHA below. No CI requirement or test threshold was reduced.

## Existing app audit and parallel boundaries

The base uses `SereaHistoryDatabase` in `serea-history.db`, schema version 3.
Its history AES-GCM uses `KeystoreAesGcm` and the `serea-history-v1` alias.
Compose Material 3, lifecycle ViewModels, Room, and coroutines were already
available. The manifest disables backup and applies both legacy and Android 12
data-extraction exclusions.

This change adds a separate tasks database and does not modify
`SereaHistoryDatabase`, history migrations, `EncryptedStorage.kt`,
`UsageAccounting.kt`, the Usage schema/ledger, AdaptiveModelRouter, provider or
security ports, Rust Core, or PC permissions. The debug process-death harness
was extended only to exercise its separate test task database. The task launch
entry point is the only chat-screen integration.

## Android Tasks architecture

`TaskScreen` → `TaskViewModel` → `TaskRepository` → `TaskDao` → `TasksDatabase`
and `TaskCipher`. Compose has no DAO reference. Domain and proposal contracts
have no Compose, model provider, or cloud dependency.

`SereaTasksDatabase` uses `serea-tasks.db`, Room schema version 1, and a
committed exported schema at
`android/app/schemas/app.serea.android.tasks/1.json`. `tasks` has a UUID primary
key, monotonic revision, and encrypted record. `proposal_receipts` stores an
opaque proposal ID and task ID for one-time approval. No destructive migration
is enabled and no historical TasksDB migration is claimed.

## Domain, validation, and date semantics

`Task` has a stable UUID, title, notes, open/completed state, LOW/NORMAL/HIGH
priority, created/updated instants, optional due date/time/timezone,
completion instant, and revision. Title and notes retain Unicode, emoji, and
newlines; code-point limits are 200 and 10,000. Empty titles, malformed UTF-16,
invalid IDs, invalid priorities/zones/dates, impossible timestamps, and
completion-state mismatches are rejected.

A date without a time is an all-day local deadline (`LocalDate`). A timed
deadline requires a date and IANA timezone and resolves to a UTC `Instant`.
DST gaps and overlaps are rejected. Overdue evaluation accepts an injected
instant and zone. One clock is shared between the Activity, ViewModel, and
repository; all-day deadlines compare local dates and timed deadlines compare
instants.

## Encryption and storage policy

The complete payload is encrypted: title, notes, status, priority, timestamps,
due date, due time, and timezone. SQLite plaintext is limited to random task
UUIDs, revision counters, and opaque proposal receipt/task IDs. Task text and
metadata are not logged or sent off-device.

The task adapter reuses low-level `KeystoreAesGcm` with the dedicated alias
`serea-tasks-v1`; the history alias is not reused. AES-GCM creates nonces and
checks authentication tags. Associated data binds ciphertext to task ID and
record version, so tampering or moving a ciphertext between tasks is rejected.
Missing keys and invalid ciphertext raise a visible storage error; the app
does not create a replacement key, erase rows, or initialize an empty store.
Manifest backup exclusion covers the TasksDB. Keystore use does not guarantee
integrity on a rooted or otherwise compromised OS.

## CRUD, concurrency, and proposals

Repository operations include create/get/list/update/complete/reopen/delete,
incomplete/completed filters, due-today/overdue queries, and in-memory
priority/date sorting. Revision-based SQL compare-and-set protects updates;
deletion is conditional on the same ID and revision. Per-instance mutation
serialization plus the database CAS handles multiple repository instances.
Stale updates, update/delete races, and late updates after deletion cannot
recreate a row. Completion and reopening with the current revision are
idempotent.

The UI freezes the selected ID and revision in the delete confirmation. Saves,
validation, storage failures, and revision conflicts have visible feedback.

`TaskProposal` distinguishes CREATE, UPDATE, COMPLETE, and DELETE. Proposal
creation/receipt alone has no repository side effect. Explicit user approval is
required before the repository call; proposal CREATE uses a transaction and a
unique receipt to prevent duplicate approval from creating a second task.
Formal authorization policy, provider egress, and AI integration remain
future work; no PC permit is reused.

## Compose UI and lifecycle

The minimal Material 3 screen displays tasks, status, due date, priority, and
empty state. The editor supports title, notes, priority, ISO local date, 24-hour
time, and timezone. It supports create/edit/complete/reopen/delete and delete
confirmation. Existing `SereaTheme` and dark mode remain in use.

ViewModel state reloads from Room after Activity recreation. Mutations
invalidate older list refreshes. The debug-only process-death receiver and
harness use a dedicated task database and test alias, force-stop and relaunch
the app, verify persisted/open/completed/deleted task state, and do not clear
app data. This is separate from a rotation-only test.

## Tests and CI results

On implementation validation SHA `9e637c3f7ba228f1cf8c70babdb9205be76eb04a`:

- Android build, lint, dependency audit, integration, security, unit, and
  documentation jobs: **PASS**, Android workflow run `39372623137` (attempt 2).
- API 35 instrumentation: **44/44 PASS**, including 6 task DB and 4 task
  Compose cases; OS force-stop/relaunch Process Death: **PASS**.
- API 26 instrumentation: **44/44 PASS**; OS force-stop/relaunch Process Death:
  **PASS**.
- Android Emulator Required Gate: **PASS**.
- Fast CI and Commit Identity: **PASS**, run `39372623160`.
- Full CI (Linux stable, MSRV 1.85.0, macOS Intel/ARM64, Windows P7 gate):
  **PASS**, run `39372623124`.
- Native platform validation (stable and Rust 1.85.0 Linux/Windows/macOS
  targets): **PASS**, run `39372623211`.
- SQLite portability (Intel/ARM64 database creation and cross-opening):
  **PASS**, run `39372623205`.
- Git Hooks Windows compatibility (Ubuntu/macOS/Windows): **PASS**, run
  `39372623170`.

The first API 35 run had one 30-second timeout in the existing
`ChatScreenTest.currentConversationDeletionRequiresConfirmationAndCancelKeepsMessages`
while waiting for a Demo Chat response. The task tests in that run completed
without failures. The failed jobs were rerun on the same SHA; API 35 then
passed, API 26 passed, and the Android gate passed. The initial flake is
recorded rather than hidden.

Local Android Gradle compile/test: **NOT_RUN** due to the missing local JDK and
SDK described above. GitHub Actions ran the Android build and all required
tests. API 26/35 each retained the prior 34 cases and added 10 task cases; the
instrumentation verifier threshold is 44. No test was disabled.

## A/B compatibility and security review

- A/provider boundary: no router, provider, security-port, cloud, or egress
  changes; no task text leaves the device.
- B/usage boundary: no UsageAccounting, Usage Ledger, history DB, or migration
  changes. History and Usage structures were verified unchanged against the
  base diff.
- `MainActivity`: one small task launch affordance; screen behavior remains in
  `TasksActivity` and `tasks/`.
- No external credentials or real provider requests were used. Workflow
  provider tests use their existing mocks.
- No task is automatically created, edited, completed, or deleted by AI.

## Not implemented and follow-up

- Notifications: **NOT_IMPLEMENTED**.
- Recurrence, sync, Calendar/Gmail, live AI proposal generation, cloud access,
  PC Core integration, and detailed UI polish: not implemented.
- No formal ADR was proposed or adopted.
- Physical-device UI acceptance, owner beta acceptance, and future notification
  design remain follow-up work. No merge, signing-key setup, or user-data
  deletion was performed.

The report-only follow-up commit updates this file with the implementation
validation outcomes; it does not change the tested Android implementation.
