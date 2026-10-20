# Android Google Authorization Foundation Report

## 1. Scope and source revisions

- Lane: C — Google Android Authorization Foundation.
- Branch: `codex/android-q4-google-authorization-2026-10-20`.
- Draft PR: [#65](https://github.com/Yrika819/Serea/pull/65), base `codex/android-m0-08-abcd-integration-2026-10-20`.
- Pinned PR #58 head and actual PR base SHA: `239aaa181cdcfea756cf6c9ebb5eca9194caf7cf`.
- Implementation commit before this report: `4498d1dd2515a29345ba59155b5f066a5acbdac0`; tree `1f70a72cc7b4837126fd8f598bea25ff482e8f53`.
- The report-only commit follows the implementation commit. The exact current branch HEAD and tree are in the PR and are recorded in the handoff response; the report itself cannot contain its own commit SHA.
- At initial preflight PR #58 was OPEN/DRAFT and its head matched the pinned SHA. Immediately before creating PR #65, PR #58 still had that same head.

## 2. Changed files and implementation

- `android/app/src/main/java/app/serea/android/googleauth/GoogleAuthorization.kt`
- `android/app/src/main/java/app/serea/android/googleauth/FakeGoogleAuthorizationGateway.kt`
- `android/app/src/test/java/app/serea/android/googleauth/GoogleAuthorizationTest.kt`
- `android/app/build.gradle.kts`
- `android/app/gradle.lockfile`
- `docs/plans/2026-ANDROID-GOOGLE-AUTH-REPORT.md`

The production adapter uses Play services `AuthorizationClient` 22.0.0. Official Android guidance says `authorize()` returns access when prior grants cover the request, or a `PendingIntent` resolution when consent or account selection is needed. The adapter exposes a one-shot consent intent for a foreground caller and consumes the Activity Result through `getAuthorizationResultFromIntent`. Real OAuth is disabled by default and no startup path calls it. Current work does not wire it to `MainActivity`, a ViewModel, or Gmail/Calendar transports.

Scopes are allowlisted as Gmail `gmail.readonly`, Calendar `calendar.events.readonly`, and optional `calendar.calendarlist.readonly`. Callers must request only the scopes needed for an operation. The Calendar scope choice matches PR #57's plan to add calendar-list scope only if calendar discovery is actually used. Google describes the Gmail scope as restricted; Calendar's listed scopes are read-only but consent and verification classification must be confirmed for the selected project in Google Auth Platform.

Typed outcomes distinguish consent needed, denied, account changed, expired, network failure, cancellation, unconfigured OAuth, missing/outdated Play services, and unavailable authorization. `AccessTokenLease` retains the access token in a private in-memory field, redacts it from `toString()`, caps the lease at five minutes, and drops the reference on close. The Gmail/Calendar-facing boundary is a `GoogleAccessTokenSupplier`; it has no persistence method. The fake uses only synthetic fixture values and makes no Play services or network calls.

`revokeGoogleGrant()` calls the app-wide Google `revokeAccess`; it is deliberately separate from the `DisableInApp` action and the fake's feature toggle. A per-service UI disable must not be presented as Google grant revocation. Google documents that `revokeAccess` removes this app's grant and future requests require consent again.

## 3. Implemented, not implemented, and Owner action

**Implemented:** authorization seam, default-disabled Play services adapter, consent resolution handoff, scope status, fake grant/reject/revoke/account-switch/service-failure states, transient token lease, and revocation distinction.

**Not implemented:** Gmail or Calendar API transport, account-selection UI, persisted account identity, automatic token refresh, cache deletion, UI settings, MainActivity/ViewModel wiring, and verified runtime detection of a real Google account change. The state model and fake cover account change; the SDK adapter does not persist or compare an account identifier. A later UI integration must decide how to report an account switch without logging or persisting account PII.

**Owner action required:** create/configure the personal Cloud project and consent screen; enable Gmail and Calendar APIs; decide audience and publish state (External/Testing/In production), test-user list, disclosure/privacy text, verification path, and personal-use exception applicability; register the chosen Android package name plus the actual selected signing certificate SHA-1. No OAuth client ID, secret, token, project, consent screen, or SHA-1 registration was created here. Gmail `gmail.readonly` is a restricted scope and can require restricted-scope verification; Google documents seven-day authorization/refresh-token expiry for External apps in Testing in the applicable cases. Those are deployment decisions, not enabled code behavior.

The OAuth adapter does not request offline/server access or a refresh token. Google OAuth client secrets must not be placed in the APK. No OAuth consent screen or real authorization was launched: **real OAuth/API and device checks are `NOT_RUN`; Cloud project/SHA-1 setup is `OWNER_ACTION_REQUIRED`.**

## 4. Verification

Executed locally against the implementation source on Java 17, Gradle 8.11.1, Android SDK 35:

| Check | Result | Evidence |
|---|---|---|
| `:app:testDebugUnitTest --tests 'app.serea.android.googleauth.*'` | PASS | `GoogleAuthorizationTest`: 14 tests; fake consent/added scope/denial/re-authorization/missing scope/expiry/account switch/service unavailable and outdated/cancel/network/token redaction and expiry/revoke isolation/default disabled. |
| `:app:testDebugUnitTest` full regression | PASS | 107 test methods across 12 test classes, 0 failures/errors/skips. |
| `:app:assembleDebug` | PASS | Debug APK assembled; not installed. |
| `:app:assembleRelease` | PASS | Release APK assembled; no signing configuration changed. |
| `:app:assembleDebugAndroidTest` | PASS | Instrumentation APK compilation only; no emulator/device test was run. |
| `:app:lintDebug` | PASS | Android Lint completed with abort-on-error enabled. |
| `:app:dependencies` and `:app:generateLicenseReport` | PASS | Lock state regenerated for debug and release; Play services Auth 22.0.0 appears in the license report with Google's Android SDK license. |
| `git diff --check` | PASS | No whitespace errors. |
| `tools/git_preflight.py --mode commit` | PASS | Repository-local Codex identity and staged scope accepted. |
| `tools/check_commit_identity.py --pre-push --remote origin` | PASS | 138 new commits checked after refreshing `origin/main`; all approved identities. |
| `tools/git_preflight.py --mode push` | PASS | Explicit target branch and PR #58 base verified; push hook passed. |
| OSV scanner | NOT_RUN locally | `osv-scanner` is not installed in this workspace; the existing Android Dependency Audit workflow owns this check. |
| GitHub CI for the final report commit | PENDING/NOT_RUN at report authoring | The draft PR's Android workflow dispatches after each pushed head; its exact final-head run must be read from PR #65 checks. Earlier-head green results must not be reused as final-head evidence. |
| Real OAuth, Google API, and physical device | NOT_RUN | No real account, API request, emulator authorization, or device install was used. |

The Android-specific `Android Calendar Read-only Tests` workflow filters both by the older PR #57 base branch and by `calendar/**`; this PR targets PR #58's head branch and adds `googleauth/**`. It is therefore not the validation path for this lane. The required `Android` workflow selects Android changes broadly and runs the full unit task; no workflow filter or security policy was edited.

## 5. CI status and run IDs

PR #65 dispatched the repository's existing Fast CI, Full CI, cross-platform, hook, and Android workflows on implementation head `4498d1dd2515a29345ba59155b5f066a5acbdac0`. At report authoring, checks were pending. Those run IDs are not evidence for the later report commit. Final-head workflow names, run IDs, head SHA, and statuses are reported in the handoff after polling PR #65; unavailable or unfinished checks remain `PENDING`/`NOT_RUN`, never PASS.

## 6. Security and data invariants

- Egress: no Google result is connected to `ChatContext`, provider routing, or external model calls; existing `SecurityPorts.kt` is untouched.
- OAuth: real authorization is disabled by default; scopes are read-only and allowlisted; no credentials, account identity, or consent result is persisted or logged.
- Credentials: access tokens are transient in a private lease and are absent from SavedState, Room, diagnostics, or test artifacts. Only a synthetic fake token is used in tests.
- Keystore/DB/Task CAS: no Keystore, Room schema, History DB, Tasks DB, Task CAS, migration, Usage Ledger, or repository file was changed.
- Git identity: repository-local `Codex <noreply@github.com>`, `serea.identityProfile=codex`, `user.useConfigOnly=true`, and `push.default=nothing`; repository hooks stayed enabled. The first push preflight found stale local `origin/main`; it was refreshed read-only, then the pinned start and PR #58 head were rechecked unchanged before push.
- Device/secret handling: no APK was installed; no signing configuration or key material was inspected or changed; no OAuth secret or token was requested or emitted.

## 7. Integration dependencies and PR #51

PR #57's `CalendarPermissionPolicy` specifies `calendar.events.readonly` and adds `calendar.calendarlist.readonly` only for calendar discovery; this lane supplies those exact scope values without editing Calendar files. A/B and Calendar ports can later receive `GoogleAccessTokenSupplier` through a separate integration change.

PR #51 remains OPEN/DRAFT at the recorded head `79de1b30d6f552538368e8c38dea71ce0b026664`. Its diff changes signing/version sections of `android/app/build.gradle.kts`; this lane changes the dependency declaration in that same file and the dependency lockfile. There is textual overlap at the file level, so integration must preserve PR #51's signing/version edits and this lane's auth dependency and lock entries. No PR #51 code or signing block was imported or changed here. The dependency resolved to Play services Auth 22.0.0 plus its transitive libraries; Gradle lock and license output were regenerated and checked. The dependency-audit workflow still needs to complete on the final head.

## 8. Known limitations and next steps

- Owner registers the Android OAuth client for the package/signing certificate pair selected for Beta, configures the consent screen/test users and verification, then explicitly authorizes a separately reviewed real-flow implementation.
- Add Activity Result UI wiring, account-change UX, and clear/retry behavior in the integration lane; test on a non-production test account only after Owner configuration.
- Validate user deletion/revoke behavior and ensure any future Google-derived cache is invalidated.
- Poll required CI on the exact report-commit HEAD before calling remote validation complete. Until then CI is pending, not passed.

### Source references

- [Android Authorization guide](https://developer.android.com/identity/authorization)
- [AuthorizationClient reference](https://developers.google.com/android/reference/com/google/android/gms/auth/api/identity/AuthorizationClient)
- [Gmail API scopes](https://developers.google.com/workspace/gmail/api/auth/scopes)
- [Calendar API scopes](https://developers.google.com/workspace/calendar/api/auth)
- [OAuth Testing status and token lifetime](https://developers.google.com/identity/protocols/oauth2/)
- PR #57 Calendar plan: `docs/plans/2026-ANDROID-M0-07A-CALENDAR-READONLY-REPORT.md`
