# Serea Android Q4 A〜G Integration Audit Report

Date: 2026-10-20

Repository: `Yrika819/Serea`

Integration branch: `codex/android-q4-abcdefg-integration-2026-10-20`

Requested PR base: `codex/android-m0-08-abcd-integration-2026-10-20` (`239aaa181cdcfea756cf6c9ebb5eca9194caf7cf`)

## 1. Result

**Partially complete pending the new Draft PR's exact-head GitHub Actions checks.** All seven pinned lanes are merged without conflicts into the new branch, cross-lane compatibility fixes are committed, local Android unit/build/lint/dependency checks pass, and emulator test counts are now fail-closed at 71 for both API 26 and API 35. The new PR's CI, API 26/35 instrumentation, Process Death checks, and dependency vulnerability scan were not available before publishing this report and must not be inferred from earlier lane runs.

Code and workflow validation was run at `e43cb2240a9f15755e93c5adad0edb4d706ab0ab` (tree `b56d3930db7480f6014cc3d29da876f61a0fce87`). The subsequent report commit changes documentation only. GitHub checks on the final Draft PR head remain the release gate.

## 2. Source PR preflight and provenance

At the start of the campaign, PR #51, #58, and #59–#65 were confirmed `OPEN` and `Draft`. They were rechecked after local integration: the fixed source heads and #58 head still matched, and #51 remained separate. Each A–G fixed commit has merge-base `239aaa181cdcfea756cf6c9ebb5eca9194caf7cf`; the seven source commits and merge commits below retain their original ancestry and identity. Source commits and newly authored commits use `Codex <noreply@github.com>`.

The branch was created from the exact requested base commit. Lanes were merged with `git merge --no-ff` in G → A → B → C → D → E → F order. Every merge completed cleanly with `ort`; no conflict was reported or resolved. No source PR, PR #51, PR #58, or `main` was changed.

| Lane | Source PR | Pinned source HEAD | Integration merge commit |
|---|---:|---|---|
| G — CI Coverage | [#62](https://github.com/Yrika819/Serea/pull/62) | `87999492e6318ccd3580f7cbd581e987a826a316` | `848abb42e9d4571c176e54c3046096f83cd61256` |
| A — Gmail Domain | [#64](https://github.com/Yrika819/Serea/pull/64) | `4249d3e0bb861efee3a4c6470c13f902912be875` | `42a1246fcb22b081b0dfa765712f97b2a9ae9cdb` |
| B — Calendar HTTP | [#59](https://github.com/Yrika819/Serea/pull/59) | `9f9d41954c4112b7abdb38684ce543027b0450ac` | `a835782bf0a3aa8460d83a012362b143be5e49aa` |
| C — Google OAuth | [#65](https://github.com/Yrika819/Serea/pull/65) | `13068668c8602e5f7fbe1c1c50281a210faa3b98` | `16e682858782f425c5d8c3242d9daec757a9f5db` |
| D — Chat History UX | [#63](https://github.com/Yrika819/Serea/pull/63) | `997d174695726a9f627e8fcf0719c43b01b88703` | `46fcec808a54254bd2512ab306d5f36a4f0561a7` |
| E — Task Date/Time UX | [#61](https://github.com/Yrika819/Serea/pull/61) | `2a2590f40e59e9062b3b633d00205246cb32b52f` | `1243b7a737552c34ee3efb12c9b5ab6a257a997a` |
| F — TaskProposal Review | [#60](https://github.com/Yrika819/Serea/pull/60) | `490e3be4cebe54f85e8e42f670236d29e74f812a` | `ea1b3a1f1696ce940c555e31b5f5f7d662bca21f` |

### PR #51 coexistence review

PR #51 was confirmed OPEN/Draft at head `79de1b30d6f552538368e8c38dea71ce0b026664`, based on `94492d281fe0ee583744c932259c3525d5c34f86`. Its `android/app/build.gradle.kts` edits concern app versioning, optional internal Beta signing, and the protected update-test configuration. Lane C adds `play-services-auth:22.0.0`; its corresponding lockfile change is in `android/app/gradle.lockfile`. Those concerns can coexist when PR #51 is later integrated. No PR #51 signing block, key, or version configuration was copied into this branch, and PR #51 was not edited.

## 3. Integrated contract fixes

The additional code and gate fixes are in `e43cb2240a9f15755e93c5adad0edb4d706ab0ab`.

### Gmail and OAuth

- Gmail's only allowlisted scope is `gmail.readonly`. The OAuth gate and shared authorization code do not start authorization at app startup. Real OAuth remains disabled by default.
- `MOCK_ONLY` cannot authorize real Google HTTP. The Gmail domain has no real REST transport in this integration, so no Gmail HTTP request, inbox sync, or automatic cloud egress was enabled.
- A Gmail text part with `body.attachmentId` but no inline `body.data` is rejected as incomplete. The parser does not report an attachment-only body as a complete message. It still does not download attachments. Existing multipart/alternative, multipart/mixed, unsupported MIME, Base64URL, charset, HTML text extraction, and payload size limits remain in place.
- Gmail messages retain `DataOrigin.GMAIL` and non-public classification. The message model rejects `PUBLIC`; the read-domain collector checks origin and classification. No Gmail/Calendar data is connected to the chat model input.

The future Gmail REST adapter boundary is `GmailReadPort` → a transport adapter that checks `GmailPermissionPolicy` and the Gmail feature gate before calling the shared OAuth supplier for exactly `gmail.readonly`. It may implement the existing inbox/search/message/thread read verbs using Gmail `messages.list` and `messages.get(format=full)`. It must not add modify/send/watch/history/attachment-download behavior without a separate review. It must keep the current paging and parser byte limits, use a short-lived token lease, redact API errors, and never log account IDs, message IDs, sender, subject, snippet, body, or bearer token. Gmail-derived content remains PRIVATE and `DataOrigin.GMAIL`; it is not persisted or attached to an AI request by default. No such REST adapter or live HTTP request is part of this integration.

### Calendar and OAuth

- Event reads request only `calendar.events.readonly`. Calendar list calls use the separately allowlisted `calendar.calendarlist.readonly`; the bridge passes the exact endpoint scope into token acquisition and never starts consent itself. `CalendarAgendaService` discovery needs the list scope and fails closed if it is absent.
- `GoogleCalendarAuthorizationAdapter` joins the shared OAuth gateway to Calendar authorization/token ports. It rejects `MOCK_ONLY`, reports a locally disabled Calendar feature as `FEATURE_DISABLED`, and holds the short-lived token lease until the HTTP operation ends. Disabling Gmail leaves the Calendar gate available; disabling either feature invalidates its existing leases without revoking the shared Google grant.
- For all-day events with no timezone, the parser no longer assumes UTC. It uses an explicit event/page IANA timezone, or fetches the calendar timezone only when CalendarList scope is already granted. If that timezone cannot be obtained without new consent, it returns `TIME_ZONE_UNKNOWN` after the event request rather than inventing a time. Calendar list responses without `timeZone` also fail as incomplete.
- The transport still checks authorization before each request, does not follow redirects, does not retry automatically, bounds response bytes at 512 KiB, and retains the existing date/time parsing behavior. Unit tests cover known and unknown timezone cases, scope refusal, no retry, redirect behavior, and bounded reads.

| Feature | Scope | Required for |
|---|---|---|
| Gmail read | `https://www.googleapis.com/auth/gmail.readonly` | Gmail inbox/search/message/thread read adapter |
| Calendar event read | `https://www.googleapis.com/auth/calendar.events.readonly` | `events.get` and `events.list` |
| Calendar List read | `https://www.googleapis.com/auth/calendar.calendarlist.readonly` | Calendar discovery and `getCalendar`; not requested for event-only reads |

### OAuth lifecycle

- A Google result that presents a re-consent resolution replaces the cached permission snapshot for the requested scopes, including replacing stale `GRANTED` with `NEEDS_CONSENT`. `SIGN_IN_REQUIRED` maps to `CONSENT_REQUIRED`, which also updates known permission state. Network, cancellation, service, and feature-gate failures do not falsely rewrite grant state.
- The one-slot consent registry binds request ID, scopes, `PendingIntent`, launch, completion, cancellation, and stale Activity Result handling. The request and feature gate are rechecked immediately before authorization starts. Parallel consent/revoke attempts are rejected.
- Tokens remain inside redacted, closeable, time-limited `AccessTokenLease` objects. The Calendar bridge does not copy a token into a longer-lived string before the transport uses it.
- The `PlayServicesGoogleAuthorizationGateway` round trip was not run. Unit tests exercise scope mapping, the mock boundary, local gate behavior, lease invalidation, consent registry behavior, and cached-state helpers only.

### Chat history and Tasks

- The A→B history race test blocks A's pending-status update, selects B, releases and awaits A's completion, then asserts that B's conversation and message still own the screen state. It uses an in-memory Room database and a unique temporary Keystore alias, and verifies that the named app history file is unchanged.
- Existing lazy conversation creation, first-message/title transaction, Unicode code-point title limit, encrypted title/body storage, old row reading, PENDING/OUTCOME_UNKNOWN recovery, and History DB v4 remain intact. The integration does not alter the History schema or production Keystore alias.
- DatePicker/TimePicker UI coverage remains six methods. It checks date-only tasks, a 24-hour time and IANA zone, exact existing task ID/revision/Draft updates, invalid zones, daylight-saving gaps/overlaps, and time-without-date rejection. The new source compiles into the instrumentation APK; it was not run on an emulator here.
- TaskProposal still requires explicit approval; delete requires a second confirmation; expiry, stale revisions, target mismatch, and duplicate approval are rejected. No AI/provider adapter calls `TaskRepository` directly. The `TaskProposalReviewScreen` is not connected to the live chat path; no model integration was fabricated.
- The Room-generated `TasksDatabase` schema export at `app.serea.android.tasks.TasksDatabase/1.json` is byte-identical to the existing v1 schema export (identity hash `403b2c91e324942e0474527fab87210b`). No table, schema version, migration, task ID, revision, or CAS rule changed.

### Composition root and data egress

`MainActivity` still creates `ChatViewModel` with `MockModelChatPort()` and displays `ChatScreen`. It does not instantiate OAuth, Gmail, Calendar, Tasks, or TaskProposal navigation. Gmail and Calendar are domain/transport seams rather than user-facing integrated features. `ChatEgressGate` only allows PUBLIC USER_AUTHORED/MODEL_OUTPUT context to a model adapter; `DataOrigin.GMAIL` and `DataOrigin.CALENDAR` are excluded. Usage Ledger, provider routing, Egress Gate, encrypted history, Task CAS, and database persistence logic were not weakened.

## 4. CI coverage changes

The Android workflow still runs the complete `:app:testDebugUnitTest`. Its unit JUnit verifier now fails on absent, empty, malformed, failed, errored, or skipped XML; required artifacts fail if no XML is uploaded. The Android artifact-download steps no longer use `continue-on-error`.

Both API 26 and API 35 instrumentation commands now require ten exact classes and a minimum of 71 tests. The verifier compares all of the following before passing: each class's Kotlin `@Test` source count, the total source count, the JUnit XML total, and every required class's JUnit count. Existing seven class requirements are unchanged; the three additional classes are `ChatHistoryUxTest=4`, `TaskDateTimePickerTest=6`, and `TaskProposalReviewScreenTest=2`. It rejects failures, errors, skips, absent XML, and source/JUnit count mismatches. Process Death checks remain required on both API levels.

| Required class | Exact methods |
|---|---:|
| `EncryptedHistoryTest` | 16 |
| `ChatScreenTest` | 16 |
| `ChatHttpE2ETest` | 4 |
| `UsageLedgerDatabaseTest` | 12 |
| `TasksDatabaseTest` | 6 |
| `TaskScreenTest` | 4 |
| `ChatTasksDatabaseIntegrationTest` | 1 |
| `ChatHistoryUxTest` | 4 |
| `TaskDateTimePickerTest` | 6 |
| `TaskProposalReviewScreenTest` | 2 |
| **Total** | **71** |

## 5. Local validation evidence

Environment: Temurin Java 17, Gradle 8.11.1, Android SDK platform 35. Gradle used the configured HTTPS proxy and system CA trust store; TLS verification was not disabled.

| Check | Result | Evidence |
|---|---|---|
| `:app:testDebugUnitTest` | **PASS** | 166 tests, 15 JUnit suites, 0 failures, 0 errors, 0 skipped. JUnit XML checked by `tools/verify_android_unit_junit.py`. |
| `assembleDebug assembleRelease assembleDebugAndroidTest lintDebug` | **PASS** | All four tasks succeeded on the final code source. Debug and Release APKs and the AndroidTest APK were built; Lint completed. This is compilation/build evidence, not an emulator run. |
| `:app:dependencies :app:generateLicenseReport` | **PASS** | Resolved and reported the locked graph. `play-services-auth:22.0.0` appears with the Android Software Development Kit License; `kotlinx-coroutines-play-services:1.10.1` and auth transitive entries appear in the lockfile. |
| Instrumentation source scan | **PASS** | All ten required source classes total exactly 71 `@Test` methods. |
| Instrumentation verifier regression tests | **PASS** | `android_instrumentation_results_tests.py`: 6 tests. |
| Unit JUnit verifier regression tests | **PASS** | `android_unit_junit_results_tests.py`: 3 tests. |
| Git preflight regression tests | **PASS** | `git_preflight_tests.py`: 28 tests. |
| Git hook regression tests | **PASS** | `git_hook_python_tests.py`: 4 tests. |
| Commit identity regression tests | **PASS** | `commit_identity_tests.py`: 17 tests. |
| Git commit preflight and pre-commit hook | **PASS** | Repository-local Codex identity, staged scope, explicit commit branch, and enabled `.githooks` accepted the code commit. |
| `git diff --check` | **PASS** | No whitespace errors. |
| API 26/35 instrumentation and Process Death | **NOT_RUN locally** | No emulator executable or connected ADB device; `/dev/kvm` is absent. No physical device was used. The API 26/35 Required Emulator Gate must run on GitHub Actions. |
| OSV vulnerability scan | **NOT_RUN locally** | `osv-scanner` is not installed in this workspace. The Android Dependency Audit workflow remains required. |
| Real OAuth, Gmail, Calendar, or AI provider calls | **NOT_RUN** | No Google account, network API call, provider credential, or personal data was used. |

An initial local attempt exposed and fixed two integration defects before the passing evidence above: the TaskProposal suspend fake used an invalid `@Synchronized` annotation, and the Task date/time instrumentation source omitted its Compose `assert` import. Calendar token fixtures were also changed to provide a fresh one-shot lease per request, and encoded path assertions now inspect decoded path segments.

## 6. Source-PR CI snapshot (not final integration evidence)

These are workflow-run snapshots for the pinned source commits, captured during the campaign. They are **not** proof for the new integration head. At that snapshot, all seven Android runs were queued. JUnit counts, required jobs, and artifacts were therefore `NOT_AVAILABLE` for those runs.

| Lane / source PR | Workflow | Run ID | Head SHA | Status / conclusion | JUnit / artifact |
|---|---|---:|---|---|---|
| G / #62 | Android | `39431078445` | `87999492e6318ccd3580f7cbd581e987a826a316` | queued / none | not available |
| A / #64 | Android | `39439058865` | `4249d3e0bb861efee3a4c6470c13f902912be875` | queued / none | not available |
| B / #59 | Android | `39436061002` | `9f9d41954c4112b7abdb38684ce543027b0450ac` | queued / none | not available |
| C / #65 | Android | `39440601386` | `13068668c8602e5f7fbe1c1c50281a210faa3b98` | queued / none | not available |
| D / #63 | Android | `39437859416` | `997d174695726a9f627e8fcf0719c43b01b88703` | queued / none | not available |
| E / #61 | Android | `39436371953` | `2a2590f40e59e9062b3b633d00205246cb32b52f` | queued / none | not available |
| F / #60 | Android | `39434830496` | `490e3be4cebe54f85e8e42f670236d29e74f812a` | queued / none | not available |

Fast CI succeeded on each pinned lane head: G `39431078637`, A `39439060962`, B `39436061012`, C `39440601958`, D `39437859661`, E `39436371781`, and F `39434830342`. The other source-commit workflows had a mix of completed success, queued, pending, and in-progress states. None of those older-head results are used as acceptance evidence for the Q4 integration.

The new PR's exact-head workflow run IDs, job conclusions, JUnit totals, and artifact availability must be recorded from the PR checks and are included in the campaign handoff when observed. A queued, pending, skipped, or missing run remains non-PASS.

## 7. Security audit outcome and remaining risk

**Verified in code/tests:** read-only allowlisted scopes; no mock-to-real HTTP authorization; no token after local feature disable; old leases invalidated; Gmail and Calendar data origin/classification preserved; no data is automatically attached to AI input; no Authorization header forwarded on redirect; no automatic retry; response bound retained; TaskProposal approval/CAS protections retained; encrypted History/Keystore path retained; test databases/aliases isolated.

**Not verified here:** real Play Services grant/re-consent/revoke round trips; real Google API behavior; API 26/35 runtime behavior; OSV vulnerability results; physical-device reacceptance. These remain explicit Beta gates, not assumed successes.

## 8. Owner actions and Beta blockers

1. **Required before merge:** inspect the new Draft PR's latest-head Android, API 26/35 Emulator Required Gate, Process Death, and Dependency Audit results. Resolve any product/test/workflow failure; do not accept missing or queued checks.
2. **Required before connecting Google:** Owner decides OAuth project, consent screen, Android package/signing certificate registration, test users, disclosure, and restricted Gmail scope verification. Keep real OAuth disabled until that review is complete. No project, OAuth client, signing identity, or token was created or changed here.
3. **Next app integration phase:** connect TaskScreen/TaskProposal review to explicit app navigation and define a user-approved proposal source. Do not connect model output directly to `TaskRepository`.
4. **Next Google phase:** define and review the Gmail REST adapter boundary and account/settings UX; persist feature toggles only through the approved settings store; add explicit Calendar/Gmail UI entry points. Gmail full sync, attachment fetching, and AI context injection remain out of scope.
5. **Separate signing work:** review PR #51 with the Owner's signing configuration in its own lane. This integration branch contains no signing key or production release identity.
6. **Device reacceptance:** run the approved disposable emulator suites first. Any Pixel 7a reacceptance remains a separate Owner/device workflow; no physical device was accessed here.

### Preserved invariants

No `main` push/merge, existing PR mutation, history rewrite, force push, CI gate weakening, hook bypass, real OAuth/API, production signing, personal-device test, Gmail/Calendar cloud egress, or unapproved task mutation was performed. History DB v4, Tasks DB v1, Keystore use, Usage Ledger, Egress Gate, Task CAS, and PENDING / OUTCOME_UNKNOWN contracts remain in force.
