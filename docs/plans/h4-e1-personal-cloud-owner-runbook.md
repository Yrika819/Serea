# H4-E1 — Owner runbook: Google Auth Platform Personal project

**Not an authorization to operate Cloud.** This is a future Owner-run procedure. Do not perform any step until the Owner separately approves the specific Cloud project/client/API operation. Console labels and page placement may change; stop if the documented state is ambiguous.

## Before opening the console

- Confirm the intended Google Account is the Owner account that will create the Personal application. Personal permits only that account; project Owner/Editor roles do not make other accounts eligible.
- Prefer a **new** Google Cloud project dedicated to this Personal app. Google says Personal cannot be changed to External or Internal, and existing External/Internal projects cannot be changed to Personal. Do not assume an existing External/Internal project can be converted. If checking an existing candidate, inspect its audience first and stop if it is not clearly Personal.
- Keep project ID, OAuth Client ID, account email, screenshots and certificate fingerprints in an Owner-controlled private operational record. Do not add them to a public PR, issue, or ordinary log.
- Scope the intended product to Calendar API, Android OAuth, events read-only, and `primary` only. No Gmail, Drive, MCP, Cloud AI or write access.

## Console procedure (only after separate approval)

1. **Open Google Cloud Console using the Owner account.** Select the approved project or create the approved new Personal project. Record privately which project is selected. Do not create/delete/modify a project based only on this runbook.
2. **Inspect onboarding and audience without submitting.** Follow Google Auth Platform’s Personal onboarding path. Confirm the displayed audience is Personal and the creating account is the intended Owner account. Google says Personal onboarding can configure the project and create a client in one step. Inspect the final confirmation screen and do **not** submit unless the Owner has separately approved every resulting action and the exact Android client identity (client type, package, and signer SHA-1) that will be created. If the console cannot separate audience configuration from client creation, stop and obtain explicit combined approval for both actions. If the console shows External/Internal or offers unclear migration language, stop. Never attempt audience conversion.
3. **Verify artifact identity before any onboarding submission or client fields.** For the exact signed, installable APK intended for installation—not an AAB, unsigned APK, build output selected by name, or source/Gradle value alone—use approved artifact provenance/build metadata to confirm `applicationId=app.serea.android.privatebeta` and its `versionCode`. Run `apksigner verify --verbose --print-certs <apk>` on that exact APK. Compare the reported signer SHA-1 and SHA-256 with the exact certificate approved for this Private test. Fail closed: do not submit onboarding or create/select a client if verification fails, the signer output is ambiguous, multiple signers appear unexpectedly, or either fingerprint mismatches. `keytool -printcert -jarfile <approved-apk>` may be used only as a supplementary cross-check, never as the required APK-aware verification. Verify that the approved certificate is not CI-ephemeral, Android Debug, Disposable fixture, or future Production signing identity. Confirm update compatibility and any Play signing distinction. Do not generate, retrieve, use, or disclose keys as part of this runbook.
4. **Choose or confirm Android OAuth client.** Only after the preceding approval and exact-artifact verification, create/select an Android-type OAuth client. Do not create a Web client as a substitute. Leave “Use this client for an AI-powered agent” unchecked: Google describes it for agentic clients; normal Android Play Services authorization plus Calendar REST is not the agent/MCP workflow in scope. If UI wording or effect differs from official documentation, stop and verify before proceeding.
5. **Enter Android client identity.** Enter the exact package name and signing certificate SHA-1 verified from the intended installable APK. Re-read the final client details and compare package/SHA-1 to the artifact; do not rely on a copied value without independent comparison. Keep client ID private.
6. **Enable Calendar API only.** Confirm the Google Calendar API is enabled for this project. Do not enable unrelated APIs.
7. **Confirm least-privilege authorization.** The planned application request is exactly `https://www.googleapis.com/auth/calendar.events.readonly`. Confirm no CalendarList, write, Gmail, Drive or broader scope is requested. Application policy permits only calendar ID `primary`; non-primary IDs and CalendarList must be rejected before authorization/token/network access.
8. **Review quota and billing.** Inspect current API quota and project billing status/requirements in the console. Record the observed state and any warnings privately. Do not enable billing or increase quota without a separate explicit Owner decision. No assumptions about billing requirements are made here.
9. **Confirm eligible account.** The only permitted authorization account is the same Google Account that created this Personal application. Do not attempt another account, including an account that is a project Owner/Editor. A `client_user_mismatch` is expected for a different account.
10. **Capture completion evidence privately.** Record audience, project, Android client type, package, `versionCode`, exact APK provenance, `apksigner` verification result, SHA-1/SHA-256, API enabled state, scope reviewed, quota/billing observations, and date/operator. Redact credentials, tokens, authorization codes and event data. Do not attach private evidence to a public PR.

## Verification and rollback

Cloud configuration alone does not authorize an app HTTP request. After H4-E2 code and CI gates are reviewed and the Owner separately approves H4-F, verify first with non-network build/config checks and mock tests. Any real connection requires a fresh, narrow approval, designated artifact/certificate, account, scope and test window. Validate authorization and primary-only behavior, then a minimal read-only Calendar operation; do not broaden scope or enumerate CalendarList.

If audience, package, SHA-1, client, API, account or quota state is unexpected: stop, do not authorize or send a request, and preserve only redacted private evidence. Rollback means disable/remove the unapproved client or API configuration only with a separate explicit Owner decision and console review. Revoking the Owner grant through the official account security controls and clearing device/local session or token state through the app’s supported disconnect path each require the applicable fresh, scoped Owner approval before execution. Without that approval, stop and preserve only redacted private evidence. Do not assume deleting a local session restores a revoked or unrevoked server grant, or that audience can be changed back. Never delete a project as an improvised rollback. See the [H4-E1 → H4-F gate matrix](h4-e1-to-h4-f-gate-matrix.md) for the approval boundary.

## Google references

- [Manage app audience](https://support.google.com/cloud/answer/15549945?hl=en)
- [Manage OAuth clients](https://support.google.com/cloud/answer/15549257?hl=en)
- [Android client authentication](https://developers.google.com/android/guides/client-auth)
- [apksigner](https://developer.android.com/tools/apksigner)
- [Calendar API authorization scopes](https://developers.google.com/workspace/calendar/api/auth)
- [OAuth 2.0 best practices](https://developers.google.com/identity/protocols/oauth2/resources/best-practices)

Project-specific IDs, account email, certificate data and console evidence belong only in private Owner records.
