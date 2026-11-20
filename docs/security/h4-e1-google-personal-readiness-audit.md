# H4-E1 — Google Personal readiness and implementation audit

**Audit basis:** repository `origin/main` at `487ca82a95e460baa4b7e451bac96cc1032a2212`, tree `29100f9861524b220f92801e13eec54058555e3c` (2026-11-20). Static/documentary audit only. No Cloud, OAuth, signing, APK, or device operations were performed.

## Finding summary

The code provides a useful Fake-first base and separates OAuth from Google HTTP authorization. It does not yet provide an approved product Live HTTP route: `PRIVATE_GOOGLE_CALENDAR_HTTP_ENABLED` is hard-coded `false` for defaults and Beta/Private Beta variants, and the isolated MockWebServer path is test-only. H4-E2 must design and test a dedicated Private-only compile-time gate without enabling it in that phase.

## Identity and build variants

`android/app/build.gradle.kts` declares namespace `app.serea.android`, Public Beta ID `app.serea.android.beta`, and Private Beta ID **`app.serea.android.privatebeta`**. Gradle variant checks and `PrivateBuildOAuthGateTest` assert the Private ID; Beta tests/contracts assert the public ID. Debug/Demo and isolated variants have distinct suffixes (`.demo` and `.isolated` in the relevant variant setup). Thus `app.serea.android.privatebeta` is the verified Android OAuth package-name candidate—not evidence that a Google client exists or that a signed artifact matches it.

## OAuth and Calendar boundary

- `PrivateGoogleAuthorizationViewModel` separates account selection and explicit Calendar consent, binds results to request/session generations, and closes/cancels on account switch. OAuth input is controlled independently from HTTP.
- `PrivateGoogleCalendarComposition` and `CalendarProviderSessions` default to Fake. HTTP is selected only when its explicit gate and an eligible Play Services gateway are present; the isolated test composition accepts only loopback test infrastructure. A selected Live error is not meant to silently switch to Fake.
- `GoogleCalendarAuthorizationAdapter` checks the enabled feature, exact permitted scope and session-bound grant/token access. Mock-only authorization cannot authorize HTTP.
- `GoogleCalendarHttpReadPort` is GET-only, constrains production transport to Google Calendar HTTPS, disallows redirects, bounds retries, and rechecks authorization/session state. In primary-only mode it rejects non-`primary` IDs and CalendarList before authorization/token/network use. The approved first scope is exactly `https://www.googleapis.com/auth/calendar.events.readonly`.
- Existing tests cover independent gates, Fake defaults, isolated loopback tests, scope/gateway rejection, primary-only behavior, transport statuses, paging and event parsing. Add Personal audience wrong-account behavior as a **mock-only** rejection test; do not try another real account.

These observations are static-source evidence and do not constitute a live security certification. H4-E2/F must recheck the current source and test names on their own base.

## Signing identity: not yet ready

An Android OAuth client must match the package name and the SHA-1 of the signing certificate on the APK actually installed. SHA-256 should also be recorded and independently compared as an identity audit. The registered value must never be inferred from an unrelated build or copied from a fixture.

Keep these identities distinct:

1. CI ephemeral signing certificate: temporary validation only; not a Private identity.
2. Android Debug signing certificate: local development only; not a Private identity.
3. Owner-controlled Private test certificate: the intended limited-test identity, but its exact certificate and ownership have not been verified/approved here.
4. Future Production signing certificate: separate release identity and approval; not interchangeable with test signing.

No key was generated, retrieved, used, or disclosed in this audit. Existing Disposable/fixture fingerprints are not candidates.

After separate Owner authorization, compare the *exact signed, installable APK intended for installation* against the proposed client registration: verify its provenance and variant; read `applicationId` and `versionCode` from artifact/build metadata; and run `apksigner verify --verbose --print-certs <apk>` on that exact APK. Confirm the reported signer SHA-1 matches the Android OAuth client registration and the approved certificate record. Confirm the signer SHA-256 matches the independently maintained approved certificate record; SHA-256 is not a field in the Android OAuth client registration. Fail closed on verification failure, ambiguous signer output, multiple unexpected signers, or either fingerprint mismatch. `keytool -printcert -jarfile <approved-apk>` may be a supplementary cross-check only; it is not the required APK-aware verification. Confirm update compatibility with the already-installed app and release lineage before distributing anything. A Play upload certificate is not necessarily the Play app-signing certificate; use the certificate that signs the artifact users install. This required `apksigner` verification has **not** been performed in this audit: the audit performed no APK inspection, signing, install, or distribution for H4-E1. See [apksigner](https://developer.android.com/tools/apksigner).

## Google Personal fit and residual risks

The Owner selected Personal for one account. Google’s official audience guidance says only the account that created the app can authorize; another account gets `client_user_mismatch`, regardless of project IAM role. Google says Personal cannot be changed to External or Internal, and existing External/Internal projects cannot be changed to Personal. Prefer a new Personal project; whether any existing candidate supports Personal is unverified and must be checked in the authorized console before use. Personal’s exemption from verification and External/Testing seven-day test-user expiry does not eliminate token expiry/revocation.

Cloud project ID, Android OAuth Client ID, Owner email, certificate fingerprints and console screenshots are environment-specific and must remain in Owner-controlled private records, never the public PR.

## Residual risks / blockers

- Exact Personal project and audience not established; no project ID is recorded.
- No Android OAuth client exists/was verified in this audit.
- Owner-controlled Private test certificate, artifact provenance, installed package and update lineage are unverified.
- Product Live HTTP route does not exist; compile-time Private gate remains a proposal.
- Google’s console labels/flow may change. Only documented constraints are treated as confirmed; current UI details must be confirmed by the Owner when authorized.
- Real Google authorization and Calendar reads remain untested and unauthorized.

See [decision](../plans/2026-11-20-h4-e1-personal-mode-decision.md), [runbook](../plans/h4-e1-personal-cloud-owner-runbook.md), and [H4-E2 design](../superpowers/specs/h4-e2-private-calendar-live-gate-design.md).
