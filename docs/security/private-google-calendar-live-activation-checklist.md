# Private Google Calendar Live Activation Checklist

**Status:** Preparation only. This change performs no Google Cloud configuration, real OAuth, Google Calendar API request, account operation, signing, or distribution. `PRIVATE_GOOGLE_CALENDAR_HTTP_ENABLED` remains a literal `false` for all product variants. `PRODUCTION_SIGNING_AUTHORIZED=false`.

This checklist is not an authorization. Each Owner decision below is separate; passing tests, a roadmap date, or a merged preparation PR does not grant permission to activate Live access.

## Current safety contract

- Public, Debug/Demo, Beta, and Private Beta product construction remains Fake-first. OAuth consent alone never selects Google HTTP.
- The future Private Live path must satisfy all of these together: the Owner-approved Private variant, an explicitly Owner-approved Live gate, the Play Services Google authorization gateway, the exact Primary-only scope contract, and a current authorization session bound to the selected account.
- The future Live path is GET-only and is restricted to `calendarId=primary` and `calendar.events.readonly`. It does not call CalendarList and has no write operation.
- Access tokens remain short-lived and in memory. Do not log tokens, account names, event titles/descriptions/locations, or authorization codes; do not retain them in CI artifacts.
- Google-origin Calendar data is never sent automatically to Cloud AI. Preserve the Egress-before-Usage-before-Dispatch boundary and require any future handoff to follow its separate, explicit local-consent flow.
- The MockWebServer URL and synthetic Gateway are test dependencies only. Explicit Live test composition overrides are accepted only in the `isolated` test variant and rejected by every product variant. They must never be supplied by a product ViewModel factory or exposed as a user, build-property, environment-variable, or intent switch.
- Only the `isolated` manifest permits cleartext to `localhost`, `127.0.0.1`, and `::1` for MockWebServer; its base policy and every product manifest deny cleartext traffic.
- No Gmail scope, CalendarList scope, Calendar write scope, production signing, APK distribution, Pixel operation, or account reset is authorized by this checklist.

## Owner-only prerequisites before any future activation

Do not begin these steps until the Owner separately approves the specific phase and target environment.

### 1. Confirm package and signing identity

- [ ] Select the final Private Beta `applicationId` from the agreed Private identity contract; do not infer it from a temporary/debug package or an old PR.
- [ ] Obtain the SHA-1 (and record SHA-256 for independent identity verification) of the Owner-controlled signing certificate that will sign that exact application ID.
- [ ] Verify the package name and SHA-1 pair against the intended Android OAuth client and the exact future APK/build provenance.
- [ ] Decide whether identity migration, update lineage, and test installation requirements are satisfied before creating a Cloud client.
- [ ] Do not generate, import, inspect, or use an Owner signing key as part of this preparation PR. Production signing remains unauthorized (`PRODUCTION_SIGNING_AUTHORIZED=false`).

### 2. Approve Google Cloud and consent configuration

- [ ] Owner selects and records the Google Cloud project and confirms who controls it.
- [ ] Owner verifies the current Calendar API enablement and quota/billing posture in that project; no API has been enabled by this change.
- [ ] Owner decides the OAuth consent audience and test-user list. For a personal account, confirm whether External/Testing is required; verify current Google restrictions and re-consent/expiry requirements from official documentation at setup time. Do not assume Internal audience supports an arbitrary personal account.
- [ ] Owner creates or approves an Android OAuth client only after package ID and certificate fingerprint are final and verified.
- [ ] Owner independently checks redirect/client configuration, authorized test users, API access policy, and any applicable sensitive-scope verification or user-warning requirements.
- [ ] Keep OAuth client identifiers, client configuration, access/refresh tokens, authorization codes, and account information out of source control, logs, public CI artifacts, and PR descriptions.

### 3. Approve the least-privilege Calendar contract

- [ ] Permit exactly `https://www.googleapis.com/auth/calendar.events.readonly` for the Primary event read.
- [ ] Prohibit `https://www.googleapis.com/auth/calendar.calendarlist.readonly`, every Calendar write scope, and every Gmail scope (including `gmail.readonly`).
- [ ] Keep `calendarId=primary` as the only accepted ID. Reject non-Primary IDs, CalendarList endpoints, and unapproved scopes before authorization status lookup, token acquisition, or HTTP.
- [ ] Confirm the API contract is read-only: event list/pagination and event details only; no create, update, delete, watch, sync-token expansion, or other resource access without a separate reviewed authorization.
- [ ] Confirm 401 invalidates only the rejected authorization generation; 403/429 retry behavior remains bounded; cancellation, account switch, grant revoke, and process death fail closed and require fresh authorization when appropriate.

### 4. Review privacy, retention, egress, and recovery

- [ ] Owner approves how Calendar-origin data is displayed, retained in memory, cleared on disconnect/revoke, and excluded from logs and build/CI artifacts.
- [ ] Confirm no automatic Calendar-to-Chat or Calendar-to-Cloud-AI dispatch. Any future user-requested handoff must pass the existing explicit local confirmation and egress/usage/dispatch checks.
- [ ] Verify no token persistence, account-name persistence, event-content analytics, crash-report payload, or CI artifact capture is introduced.
- [ ] Review app-local disconnect versus Google Grant revoke as distinct operations; require explicit confirmation before provider-wide revoke.
- [ ] Review session generation binding, stale-result rejection, process-death recovery, and reauthorization UX with tests on the exact candidate SHA.

### 5. Require a separate code and access approval

- [ ] Owner explicitly authorizes a new, scoped code change for the Private Live HTTP gate. This checklist and this preparation PR do not authorize that change.
- [ ] Keep Public, Debug/Demo, and Beta gates permanently false. Do not add an environment/property/UI override. Any future Private gate must fail closed unless all approved variant, gateway, scope, and current-session conditions hold.
- [ ] Owner separately authorizes real OAuth consent with the selected test account.
- [ ] Owner separately authorizes real external Google Calendar HTTP access and defines the exact account, time window, test purpose, and stop conditions.
- [ ] Before access, verify same-SHA Required CI, API 26/35 Formal evidence, Public/Private variant isolation, secret/artifact audit, and an independent scoped review with zero unresolved Critical/High findings.
- [ ] After the bounded test, revoke or disconnect only as explicitly approved; verify local session/token invalidation and retain only non-sensitive evidence.

## Owner execution record (future use)

Record decisions in a separately approved, access-controlled operational record. Do not fill this section with credentials or account/event data.

| Decision | Owner-approved value / reference | State |
|---|---|---|
| Final Private application ID | Not selected here | NOT_APPROVED |
| Owner signing-certificate SHA-1 verification | Not performed here | NOT_RUN |
| Google Cloud project / audience / tester choice | Not selected here | NOT_APPROVED |
| Android OAuth client creation | Not performed here | NOT_RUN |
| Calendar API enablement | Not performed here | NOT_RUN |
| Scope and privacy review | Future Owner review required | NOT_APPROVED |
| Private Live gate code change | Not authorized here | NOT_APPROVED |
| Real OAuth consent | Not authorized here | NOT_RUN |
| Real Google Calendar HTTP access | Not authorized here | NOT_RUN |
| Production signing / APK distribution | Not authorized here | NOT_RUN |

## Official references to re-check at setup time

Google requirements may change. The Owner must re-check official guidance before any Cloud operation:

- [OAuth client IDs on Android](https://developers.google.com/android/guides/client-auth)
- [Google OAuth 2.0 security best practices](https://developers.google.com/identity/protocols/oauth2/resources/best-practices)
- [Google Calendar API authorization scopes](https://developers.google.com/workspace/calendar/api/auth)
- [Google Calendar API events list](https://developers.google.com/workspace/calendar/api/v3/reference/events/list)


## 2026-11-20 Personal-mode decision addendum (current for future activation)

The Owner selected Google Auth Platform **Personal** for the single creating Google Account. This addendum supersedes older External/Testing assumptions in this checklist for the initial Personal Calendar lane; prior entries are retained as historical records. Google says Personal cannot be changed to External or Internal, and existing External/Internal projects cannot be changed to Personal. Prefer a new Personal project. Another account, including a project Owner/Editor, is not eligible; `client_user_mismatch` is expected. Personal’s exemption from External/Testing seven-day test-user token expiry does not make tokens permanent.

Before any future activation, verify the exact signed, installable APK package (`app.serea.android.privatebeta`) and signing certificate with `apksigner verify --verbose --print-certs <apk>`; compare signer SHA-1 against the Android OAuth client and record/compare SHA-256 for independent identity verification. Fail closed for verification failure, ambiguous/multiple unexpected signers, or fingerprint mismatch; `keytool -printcert -jarfile` is supplementary only. Do not substitute CI ephemeral, Debug, Disposable fixture or future Production signing identities. The approved first grant is only `https://www.googleapis.com/auth/calendar.events.readonly`, and application access is only `primary`; reject CalendarList, non-primary IDs and writes before token/network use. Keep OAuth and HTTP gates independent; Public/Beta/Debug remain unable to use Google product HTTP, and Live failure must not silently fall back to Fake.

No Cloud project/client/API change, OAuth, real API request, signing, APK distribution, or Live Gate activation is authorized by this addendum. Follow the [Personal Owner runbook](../plans/h4-e1-personal-cloud-owner-runbook.md), [H4-E2 gate design](../superpowers/specs/h4-e2-private-calendar-live-gate-design.md), and [approval matrix](../plans/h4-e1-to-h4-f-gate-matrix.md) after obtaining fresh, scoped Owner approvals.
