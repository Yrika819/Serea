# H4-F0 — Private Identity / Signing / Google Cloud Readiness Audit

**State:** Documentation-only preflight, based on `origin/main` HEAD `e908b561d1b878cb4b9afd82eb9092ff9dda4abf` (Tree `cfb11e345eab173968804cc2a0da9f78302a982b`). Source audit is not a build, manifest merge, signed-artifact, OAuth, or Cloud-configuration result. No product code was changed.

## Executive result

- The configured Private Beta identity is `app.serea.android.privatebeta`; no Application ID change is recommended. Actual merged manifest output was **NOT_VERIFIED** because the targeted Gradle task failed before producing it.
- Build configuration deliberately leaves `beta` and `privateBeta` unsigned. The committed source does not establish the signer or certificate of any existing APK. Current versionCode default for Private Beta is `2` (Gradle property override permitted); `versionName` inherits `0.1.0-demo`.
- Recommend **A: a fixed, Owner-managed Private Beta-only signing certificate**, subject to Owner decision and approved recovery/backup plan. No key was generated, searched for, read, copied, or used.
- OAuth and Calendar HTTP are independent gates. The source-level OAuth opt-in is Private-only and default-off. The product HTTP gate is literal false; Private composition opens a Primary-only Fake provider. No real OAuth, token acquisition, Calendar traffic, or Live path was exercised.
- Primary-only/read-only defenses are present at route and transport boundaries. Account changes initiated through the Private flow invalidate the prior session. Runtime behavior with real Play Services accounts and externally revoked/removed Android accounts remains **NOT_RUN**.
- Google Cloud project, Personal audience, Android OAuth client, Calendar API state, quota and billing are **UNCONFIRMED**. The Owner-only procedure is in [the H4-F0 Cloud and APK runbook](../plans/h4-f0-owner-private-google-cloud-and-apk-verification-runbook.md).

## Scope and evidence

Read-only source review covered `android/app/build.gradle.kts`, the main/variant manifest sources, `GoogleAuthorization.kt`, `PlayServicesGoogleAuthorizationGateway` implementation (in `GoogleAuthorization.kt`), `PrivateGoogleAuthorizationViewModel.kt`, `PrivateGoogleCalendarComposition.kt`, `PrivateCalendarProviderRoutePolicy.kt`, `CalendarProviderSessions.kt`, `GoogleCalendarHttpReadPort.kt`, `SereaAppShell.kt`, and relevant tests. Design references reviewed: [H4-E1→H4-F gate matrix](../plans/h4-e1-to-h4-f-gate-matrix.md), [E1 Personal Cloud Owner runbook](../plans/h4-e1-personal-cloud-owner-runbook.md), [Private Calendar Live activation checklist](private-google-calendar-live-activation-checklist.md), [H4-E2 implementation record](h4-e2-private-calendar-live-gate-implementation.md), and [2026-11-20 Personal mode decision](../plans/2026-11-20-h4-e1-personal-mode-decision.md).

The request called out `PlayServicesGoogleAuthorizationGateway` as a separate path; the implementation is defined in `GoogleAuthorization.kt`, rather than a file with that name. `PrivateCalendarProviderRoutePolicy` exists and was reviewed. The targeted `:app:processPrivateBetaManifest` task was attempted once and failed before producing a merged manifest (`26.0.2.1`, `BUILD FAILED`). It is therefore **NOT_VERIFIED**, not a passing check. No APK/AAB was assembled.

## Private Beta application identity audit

| Property | Source finding | Status |
|---|---|---|
| Namespace/default ID | `app.serea.android` | Verified in Gradle source |
| Private Beta build type | `privateBeta`, suffix `.privatebeta` | Verified in Gradle source |
| Private Beta application ID | `app.serea.android.privatebeta` | Verified from Gradle composition; no artifact inspected |
| Debug identity | suffix `.demo`; `.updatecompat` only for isolated update-compatibility test mode | Verified in Gradle source |
| Isolated identity | suffix `.isolated` | Verified in Gradle source |
| Public Beta identity | suffix `.beta` | Verified in Gradle source |
| Private Beta versionCode | defaults to `2`; `sereaPrivateBetaVersionCode` can override, valid range `2..2,100,000,000` | Verified in Gradle source; packaged APK not checked |
| versionName | inherits default `0.1.0-demo`; no Private Beta override found | Verified in Gradle source; packaged APK not checked |
| Manifest merge | Main manifest has INTERNET permission, `allowBackup=false`, `usesCleartextTraffic=false`, exported launcher `MainActivity`; Private Beta source manifest changes app label to “Serea Personal Beta”. | Merge **NOT_VERIFIED** |
| OAuth BuildConfig | Base false; Beta false; Private Beta UI true, OAuth true only when `SEREA_OWNER_PRIVATE_BETA_GOOGLE_OAUTH_ENABLED=true`; gateway additionally checks the generated gate | Source reviewed; real authorization not tested |
| HTTP BuildConfig | false in default, Beta, and Private Beta configuration; Private product composition also uses compile-time ID/build/UI checks | Source reviewed; no activation authorized |
| Signing | Beta and Private Beta clear inherited signing config; a conditional ephemeral update-test config is only for a test variant. No Private signer is configured. | Source reviewed; actual APK signer **NOT_VERIFIED** |
| CI packaging | CI restricts Private Beta tasks to an explicit small unit-test task and rejects packaging | Source reviewed; this F0 did not rerun CI |
| Public/Private storage boundary | Distinct application IDs create separate Android app data sandboxes under normal Android package identity; no package migration or shared UID was found in reviewed variant setup. | Source-level finding only; no device/data test |

**Recommendation:** Keep `app.serea.android.privatebeta` stable. Changing it would create a distinct Android app identity, prevent normal in-place updates and leave the old app’s private data under the previous package. If later evidence requires a change, stop for a separate Owner migration decision; document reinstallation, data export/import (if supported), parallel-install and OAuth-client implications first. Do not infer update compatibility from Gradle settings alone: Android requires a compatible signer and an accepted version transition. A signer change without an accepted signing lineage generally prevents an in-place update; uninstall/reinstall can lose app-private data.

## Private Beta signing options

| Option | Google Android OAuth compatibility | APK updates and data | Loss, rotation, leakage | F0 assessment |
|---|---|---|---|---|
| **A. Fixed Private Beta-only certificate** | Works when the Android OAuth client uses the exact package and SHA-1 of the certificate signing the installed APK. SHA-256 remains independent audit evidence. | Stable signer supports same-package updates when versioning and device policy permit. App-private data remains available through compatible update. | Loss of the only usable key can block updates; prepare protected backups and recovery. Rotation requires a deliberately supported signing lineage / migration plan and verified client identity. Leak requires stopping releases, assessing compromise, and planning a safe replacement; changing signer may force reinstall/data-loss risk. | **Recommended for Personal Beta**, Owner-managed, dedicated to Private Beta, with offline encrypted backup and recovery procedure. Owner has not yet selected or provisioned it. |
| **B. Shared with future production certificate** | OAuth can technically match it, but exposes the production identity to beta handling. | Easiest shared update chain if the same app identity and signer are intended. | Widens blast radius: beta handling/CI/device exposure now threatens production signing; backup, rotation, and leak response become production-critical. Future beta/production separation is harder. | Not recommended; unnecessary production-key exposure for a personal beta. Do not use without a separate explicit Owner decision. |
| **C. Temporary Debug/CI certificate** | Can be registered technically, but its fingerprint identifies that temporary signer—not the intended Private artifact. Must not be used for formal OAuth registration. | Different ephemeral/debug keys usually cannot update one another; package may differ (`.demo`). Data continuity and update tests become unreliable. | Key replacement is expected and rotation/backups are not a stable release policy. CI secret injection broadens access and risks artifact/log leakage. | Unsuitable as the official Private Beta identity. Existing CI/test keys, if any, are not adopted by this audit. |

**Owner decisions required:** select/approve A or another explicit scheme; define custodian and two independent encrypted backup/recovery controls; decide acceptable signer-rotation and suspected-compromise response; decide the exact APK provenance, versionCode and update test before any install. This audit has not inspected any key material or APK and does not identify an actual signer.

## APK certificate verification record (future Owner execution)

Run only against the exact APK the Owner has approved for the proposed private operation:

```sh
apksigner verify --verbose --print-certs <approved-apk>
```

Record privately: artifact provenance/build SHA; APK SHA-256 file hash; package/applicationId; versionCode and versionName; command exit/result; signer count; each signer certificate SHA-1 and SHA-256; whether multiple signers are expected; and compatibility against the prior approved Private APK (same package, signer/update lineage, and increasing versionCode). Confirm APK authenticity/provenance independently before trusting its certificate output. OAuth Android client registration uses the **SHA-1 from the approved APK’s signing certificate only**. Retain SHA-256 as a separate comparison/audit value.

| Evidence | State at F0 |
|---|---|
| Approved signed Private APK and its provenance/hash | `PENDING_OWNER` |
| APK signature verification result / signer count | `NOT_VERIFIED` |
| `applicationId`, versionCode/versionName as packaged | `NOT_VERIFIED` |
| Signer SHA-1 for Android OAuth input | `PENDING_OWNER` |
| Signer SHA-256 for independent comparison | `PENDING_OWNER` |
| Prior Private APK update compatibility | `NOT_VERIFIED` |

Never fill these fields from a Gradle signing report, debug/CI key, unsigned APK, source default or example fingerprint. Do not publish real fingerprints or APK hashes in public PR text. Any unexpected signer multiplicity, failed verification, provenance gap, package mismatch, fingerprint mismatch or version downgrade is a stop condition. No signing, key access, installation or distribution is authorized by F0.

## OAuth and Calendar implementation audit

| Question | Source-level result |
|---|---|
| Can only Private product composition reach real OAuth? | Private ViewModel/factory uses `BuildConfig.PRIVATE_GOOGLE_AUTH_UI_ENABLED` and `OWNER_PRIVATE_BETA_GOOGLE_OAUTH_ENABLED`; base and Public Beta flags are false. `PlayServicesGoogleAuthorizationGateway` checks the Private OAuth build gate and real opt-in. Synthetic gateway/composition injection asserts isolated build only. **No runtime OAuth test was run.** |
| Are OAuth and HTTP gates independent? | Yes. OAuth requires a local build input for Private only. HTTP is a separate literal-false build config; `CalendarProviderSessions` defaults `googleHttpEnabledByOwner=false`; Private product composition selects Fake while HTTP is off. No F0 operation changes either gate. |
| Safe defaults? | Both default false for real use. Private UI may be present while OAuth is Mock-only. The UI text distinguishes mock consent and Fake calendar from real access. |
| Account/session binding and switching? | Real account choice uses an explicit Android AccountPicker result with request ID/generation/token checks. Re-selection closes the old Calendar session and disables the prior Calendar grant before accepting a new account. Consent result is bound to generation and authorization session ID. Provider construction checks the selected session before and after opening. In-app disconnect, revoke, ViewModel clear, and provider session replacement invalidate/close local sessions. Real Play Services account removal/revocation and account switching were not exercised. |
| Primary-only and read-only? | `PrivatePrimaryCalendarScopeContract` accepts exactly `calendar.events.readonly`. Primary-only route/adapter/transport reject CalendarList and non-`primary` IDs before authorization/token/HTTP. The HTTP transport exposes GET reads only, fixed Google Calendar HTTPS origin in product composition, and disables redirects. Generic Demo fixtures remain separate. |
| Token/event leakage? | Reviewed Calendar code uses short-lived token leases, redacts token-bearing `toString`, maps errors to generic messages and contains no direct logging call found in the inspected auth/transport files. Calendar data is marked Calendar-origin; egress policy blocks automatic Cloud egress and requires a separately authorized local handoff. This is source inspection, not a whole-app dynamic leak test. |
| Does E2 implementation record match code? | Yes on reviewed core contract: HTTP literal false, Private route is Fake, no product injection of HTTP client/URL/gate, isolated synthetic route, Primary/read-only guards. The E2 record reports its own historical local suite; it is not fresh F0 evidence. |

### Findings by severity (readiness gaps; not claims of an exploitable defect)

- **Critical — none identified** in this bounded source review.
- **High — no currently identified source defect.** Real access is deliberately unavailable, so H4-F is blocked until code/Cloud/artifact/Owner gates are separately completed. This is an approval dependency, not a request to enable HTTP.
- **Medium — artifact identity not established.** No approved signed APK, signer fingerprint, version in artifact, update chain, or successful merged manifest is verified. The Gradle manifest task failed before output. This blocks OAuth client registration and F1 real-access readiness.
- **Medium — real-account lifecycle remains unverified.** The code has request/session invalidation paths, but real Play Services account switch/removal/revocation and API behavior have not run. H4-F must require authorized, bounded validation only after the proper approvals.
- **Low — historical roadmap text is stale for this lane.** The 2026-11-12 roadmap’s External/Testing and OD-H4-03 proposal predates the 2026-11-20 Personal decision. The dated Personal decision/addenda and Owner instruction take precedence for this one-account lane; keep old text as history, not current setup guidance. Future multi-user use needs a separate project/design.

## Google Cloud Personal mode decision and status

The Owner’s 2026-11-20 decision selects Google Auth Platform **Personal**, one eligible account (the application creator), Android OAuth client, Google Calendar API, exactly `https://www.googleapis.com/auth/calendar.events.readonly`, and `calendarId=primary`. Gmail, Drive, writes, Cloud AI, and AI-powered-agent OAuth client settings are excluded. This one-account Personal decision supersedes earlier External/Testing assumptions **for this Personal Beta lane only**. It is not a multi-user design; future multi-user support requires a separate project and review. Personal cannot be switched to External/Internal and existing External/Internal projects cannot be converted to Personal; prefer a new project after explicit approval.

Project existence/identity, actual audience, OAuth client, Calendar API enablement, quota, billing and any setup confirmation are **UNCONFIRMED**. No Console was accessed. Official docs were rechecked on 2026-11-21: [Manage app audience](https://support.google.com/cloud/answer/15549945?hl=en), [Manage OAuth clients](https://support.google.com/cloud/answer/15549257?hl=en), [Android client authentication](https://developers.google.com/android/guides/client-auth), [Calendar scopes](https://developers.google.com/workspace/calendar/api/auth), and [OAuth best practices](https://developers.google.com/identity/protocols/oauth2/resources/best-practices). Confirm all live requirements again when Owner chooses to act; documentation may change.

## H4-E2 → H4-F gaps and F1 readiness decision

Before any F1 real-access operation, the Owner and authorized implementers must resolve all of the following with separate approvals and evidence:

1. Owner selects the Private Beta signing identity and recovery plan; then approves a specific installable APK. Record verified artifact/package/version/certificate and update compatibility privately.
2. Owner completes the Personal project onboarding and Android OAuth client/API setup (including combined approval if onboarding submits these together); verify audience, package/SHA-1, API, quota/billing without publishing identifiers.
3. Any proposed change to the compile-time Private HTTP gate receives a distinct code-change approval, review and same-SHA Required CI/API 26+35 Formal evidence. F0 does not authorize it.
4. Bounded real OAuth, account, artifact, operation, test window, stop conditions, and any grant-revocation/cleanup action receive separate fresh Owner approval.
5. Confirm no automatic Google-origin data egress, exact scope/primary boundary, account/session invalidation, redacted evidence, and expected recovery behavior with authorized evidence.

**F1 decision: NO-GO at F0.** Design/readiness work can be reviewed, but this audit does not authorize H4-F1/H4-F real access. H4-F1 may start only after Owner review of this report and runbook and the approvals/evidence above. Keep all gates off and use clearly Fake-labeled behavior until then.

## Validation performed for F0

- Source and required design-document review: performed read-only.
- Google official documentation: re-fetched on 2026-11-21 (links above).
- `:app:processPrivateBetaManifest`: attempted once; **FAILED before merged output** with Gradle error text `26.0.2.1`; merged manifest remains **NOT_VERIFIED**.
- APK build/signing, Android Formal, device tests, real OAuth, real Calendar HTTP, Google Cloud, key access: **NOT_RUN / NOT_AUTHORIZED**.
- H4-E2 local/CI results: historical evidence in the E2 record/PR #248 only; not represented as new F0 validation. PR #248’s historical check set includes non-Required failure and cancellation; consult live GitHub status before relying on any CI claim.
