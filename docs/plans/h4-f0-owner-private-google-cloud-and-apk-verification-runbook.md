# H4-F0 — Owner-only Google Cloud and Private APK verification runbook

**Owner-run only.** This document is preparation, not permission to operate Google Cloud, authenticate, enable an API, sign/install/distribute an APK, or send a Calendar request. Do not perform any step until the Owner explicitly approves the exact Cloud operations and any Android artifact/device operations. An agent must not log in to Google Cloud or perform these actions.

## Current approved design inputs (not Cloud state)

- Audience: Google Auth Platform **Personal**, limited to the single Google Account that creates the application.
- Use: Serea Personal Beta, ordinary user-driven Android OAuth client (not AI-powered-agent client).
- Intended Android package: `app.serea.android.privatebeta` (source-level Gradle identity; not verified from an APK).
- API: Google Calendar API only.
- Scope: exactly `https://www.googleapis.com/auth/calendar.events.readonly`.
- Calendar resource: `primary` only.
- Prohibited: Gmail, Drive, CalendarList access/scope, Calendar writes, Cloud AI egress, multi-user access, and AI-powered-agent OAuth client setting.
- Current build flags: real OAuth is opt-in for Private only; Private HTTP remains `false`. This runbook does not direct enabling either gate.
- Cloud project/client/API/quota/billing state: **UNCONFIRMED**.

For future multi-user use, stop and create a separately reviewed project/design. Do not expand this Personal application. Personal is one-time; an existing External/Internal app cannot be converted to Personal, and Personal cannot be changed to External/Internal.

## Required handoff format

When a future approved task reaches a Cloud action, stop before Console interaction and send the Owner this explicit handoff:

> **OWNER_ACTION_REQUIRED — GOOGLE_CLOUD**
>
> **Purpose** — [specific project/audience/client/API action and why it is needed]
>
> **Console screen** — [Google Cloud Console > exact page/menu] ([official URL])
>
> **Preconditions** — [approved Personal project decision; verified app package; approved signed APK provenance; verified signer SHA-1/SHA-256; exact approvals]
>
> **Owner inputs** — [operation; client type Android; Android package; verified SHA-1; scope/API if shown; never include a secret in the report]
>
> **Do not select** — External/Testing/Internal; Web client; broader scopes or unrelated APIs; AI-powered-agent client; Calendar write/list access; billing/quota changes not separately approved.
>
> **Expected confirmation** — [audience Personal; Android type; package/SHA-1 match; Calendar API only; quota/billing observations; secret-free evidence]
>
> **Stop if** — [unexpected screen/state, audience mismatch, package/SHA-1 mismatch, unapproved project creation or bundled actions, additional permission/billing/quota prompt, or unclear UI]
>
> **After Owner action** — [request only non-secret confirmation/evidence; do not infer success from silence; verify redacted facts before next gate]

If Personal onboarding combines project configuration/audience and client creation, do not ask the Owner to submit it until **both project/audience and OAuth-client creation are explicitly approved**. API enablement, billing/quota changes and later code-gate activation each require their own approval. Never represent Cloud completion without Owner confirmation.

## Phase 0 — before any Cloud screen

1. Obtain explicit Owner approval naming whether a new Personal project is to be created or an existing candidate inspected. No existing project is known or verified by F0.
2. Confirm the actual application creator account privately. Personal mode permits only that account; a project Owner/Editor role does not make a different account eligible. Never record its email in public PRs.
3. Decide the exact operation(s) approved: project/audience onboarding, Android OAuth client creation, Calendar API enablement. If combined onboarding submits project setup and client creation together, obtain explicit approval for both before proceeding.
4. Do not proceed if the candidate is External/Internal, audience cannot be confirmed, or console offers migration/conversion language. Stop; do not convert. Prefer a separately approved new Personal project.
5. Do not access existing secrets, keystores, Cloud credentials, or accounts as part of this preparation.

## Phase 1 — Owner verifies the exact Private APK certificate

This phase requires separate Owner approval for artifact handling. Use only the exact, provenance-approved, signed, installable APK intended for the operation. Do not use source defaults, `signingReport`, AAB upload certificate, Debug/CI/ephemeral signer, unsigned APK, or a similarly named output as the OAuth identity.

Run these non-mutating checks against the exact approved APK (do not substitute a Gradle default or signing report):

```sh
apksigner verify --verbose --print-certs <approved-apk>
apkanalyzer manifest application-id <approved-apk>
apkanalyzer manifest version-code <approved-apk>
apkanalyzer manifest version-name <approved-apk>
shasum -a 256 <approved-apk>
```

The `apkanalyzer` commands inspect package/version metadata from the APK. Independently verify artifact provenance before trusting those values. `shasum` records the APK file hash; it is not a signer fingerprint.

Privately capture:

1. APK origin/provenance and authorized build/revision reference.
2. File SHA-256 hash (artifact integrity hash; distinct from signer fingerprint).
3. Package/applicationId and versionCode/versionName read from the APK itself.
4. Command exit status and signature-verification result.
5. Number of signers and each signer SHA-1 and SHA-256 fingerprint.
6. Whether signer multiplicity is expected and supported; stop on unexplained multiple signers.
7. Update compatibility against the prior approved Private APK: same package, accepted signer/signing lineage, and a strictly greater `versionCode` than the prior installed APK. Stop on a package/signer mismatch or a non-increasing version; do not uninstall/install to bypass an incompatible update unless separately authorized. Do not install for this check unless separately authorized.

**OAuth input:** Android OAuth registration uses the exact APK signer **SHA-1**. Keep SHA-256 as a separate audit comparison. Do not place real fingerprints, APK hash, Owner identity, project ID or client ID in a public PR. If artifact provenance, verification, package, signer, or update compatibility is unknown, mark `PENDING_OWNER` / `NOT_VERIFIED` and stop.

## Phase 2 — Owner Cloud Console sequence (only after explicit approval)

Official entry points (labels may change; stop on ambiguity):

- Console: [Google Cloud Console](https://console.cloud.google.com/)
- Audience guidance: [Manage app audience](https://support.google.com/cloud/answer/15549945?hl=en)
- Client guidance: [Manage OAuth clients](https://support.google.com/cloud/answer/15549257?hl=en)
- Android package/SHA-1 guidance: [Android client authentication](https://developers.google.com/android/guides/client-auth)
- Calendar scope reference: [Calendar API authorization scopes](https://developers.google.com/workspace/calendar/api/auth)

1. Owner opens the approved project using the creator account. Confirm Google Auth Platform **Audience** is Personal. Do not assume this is true based on project name, old documents or client presence.
2. If using Personal onboarding, inspect all bundled outcomes before submitting. Proceed only if the exact project/audience action and Android client creation are both approved and exact inputs are final. Otherwise stop without submitting.
3. Navigate to Google Auth Platform > **Clients** (or the current documented Android client creation page). Select **Android** client type. Do not choose Web or AI-powered-agent.
4. Enter the exact package from the approved APK and its verified signer SHA-1. Re-read the summary and compare it to private artifact evidence before saving. Do not use SHA-256 in the SHA-1 field.
5. Enable **Google Calendar API only**, if and only if that API enablement is explicitly approved. Do not enable Gmail, Drive, Workspace/MCP or unrelated APIs.
6. Confirm scope planning remains exactly `https://www.googleapis.com/auth/calendar.events.readonly`. Do not grant CalendarList, writes, Gmail, Drive or broad Calendar scopes. The application must still enforce `calendarId=primary`.
7. Inspect quota and billing state. Record only the observed state privately. Do not enable billing, increase quota, accept paid terms, or change quota without separate explicit Owner approval. A prompt or required-cost notice is a stop condition.
8. Do not authenticate or test consent in this Cloud setup phase. Personal allows only the creator account, and no real OAuth is authorized by this runbook.

## Stop conditions

Stop without submitting/saving or continuing if any of these occur:

- Audience is not confirmed Personal, or existing project is External/Internal/ambiguous.
- Client type differs from Android; package or SHA-1 does not match the approved APK; SHA-256 comparison is inconsistent; verification failed; signer count is unexplained.
- The APK is unsigned, Debug/CI/ephemeral, unprovenanced, or not the Owner-approved identity.
- Onboarding bundles unapproved project creation/configuration and client creation, or any required operation is not explicitly approved.
- UI requests a broad/unexpected OAuth scope, Gmail/Drive/Calendar write/list permission, Web client, or AI-powered-agent configuration.
- A non-Calendar API, billing, quota increase, paid service, organization/admin permission, or unrelated data access is requested.
- Any account mismatch, `client_user_mismatch`, unexplained authorization error, missing confirmation, or secret disclosure is observed.

Do not troubleshoot by broadening scopes, changing audience, adding users, switching accounts, enabling APIs, creating another client, or changing build gates. Capture only redacted, non-secret evidence and return to the Owner for a new decision.

## Owner completion confirmation (private record)

The Owner may report non-secret confirmation sufficient for the next review: audience is Personal; client type is Android; package matches; certificate SHA-1 matches the approved APK; SHA-256 independently matches; Calendar API state is known; scope policy reviewed; quota/billing observation recorded; no unexpected API/client/billing option was accepted. Do not send account email, project/client IDs, fingerprints, APK hash, tokens, screenshots containing private identifiers, or calendar content in public channels. If any item cannot be confirmed, leave it `UNCONFIRMED` and stop.

Suggested private state fields:

| Evidence | State |
|---|---|
| Owner approval for project/audience action | `PENDING_OWNER` |
| Personal audience confirmed by Owner | `UNCONFIRMED` |
| Android client creation separately approved | `PENDING_OWNER` |
| Approved APK provenance/hash/package/version | `NOT_VERIFIED` |
| APK signer count / verify result | `NOT_VERIFIED` |
| APK signer SHA-1 / SHA-256 match | `PENDING_OWNER` |
| Calendar API enabled state | `UNCONFIRMED` |
| Quota/billing status | `UNCONFIRMED` |
| Real OAuth, token, Calendar HTTP | `NOT_AUTHORIZED` |

## Key handling, update continuity and recovery

F0 recommends (but does not select on behalf of the Owner) a fixed Private Beta-only certificate held by the Owner. Keep its private key outside the repository and ordinary CI; use an Owner-controlled encrypted offline backup with a separately protected recovery path. Never send key material/passwords to an agent or public ticket. A lost sole signing key can prevent compatible updates; changing signer generally prevents an in-place update unless a supported signing lineage is deliberately established. Uninstall/reinstall can erase app-private data. A suspected leak requires halting use/distribution and Owner-led incident response; do not improvise signer rotation or revoke a grant without approval. Debug/CI credentials are not an official OAuth identity. Production-key sharing is not recommended for Personal Beta.

## Owner and agent boundary

This runbook authorizes no Cloud Console operation, project/client/API change, billing/quota change, authentication, token request, real Calendar request, Live HTTP gate change, key operation, APK signing/build/install/distribution, or device access. Cloud work remains Owner-only. Any later agent task must first present the handoff above and wait for explicit approval/results. Historical External/Testing text in older roadmap/checklist sections is superseded for this Personal lane by [the 2026-11-20 decision](2026-11-20-h4-e1-personal-mode-decision.md); it remains history, not the setup policy. Future multi-user support is a distinct design.
