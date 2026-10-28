# G2 Google OAuth and Workspace Data Policy Readiness

Review date: 2026-10-28 (UTC)

## Checkpoint

```text
AGENT_ID: G2
ACTIVE_PHASE: official Google OAuth / Gmail / Calendar policy readiness review
OVERALL_STATUS: PARTIAL — backend gates and Fake/Mock verification exist; public real-service readiness is not established
BASE_SHA: 373b8be15f407d951157d2a80a632e86d979acdf
BASE_TREE_SHA: 52b6b0f7b9f0449a9a78553bab235e62cdd97068
REVIEW_BRANCH_HEAD_BEFORE_REPORT: 1405783e52fc709ec3833fcd0f0b81ce5eb1d44b
REVIEW_BRANCH_TREE_BEFORE_REPORT: f75bc0ffcab8c7b5e71643b4c0e92217c185685b
WORKTREE: /workspace/Serea-g2-google-readiness-hardening
BRANCH: codex/g2-google-readiness-hardening
DRAFT_PR: none for this review branch
ACCESS_DATE: 2026-10-28 UTC
```

This report summarizes current primary Google documentation and compares it with the G2 backend and current application wiring. It is not legal advice, an OAuth verification decision, a CASA assessment, or proof that Google will approve Serea's app type. No Cloud Console configuration, real account, OAuth consent, credential, or live API request was used.

## Current Serea state inspected

| Area | Observed state at PR #132 source `373b8be…` | Readiness result |
|---|---|---|
| Normal Gmail / Calendar provider selection | `AppProviderComposition.fakeDefault()` builds the synthetic Gmail and Calendar sessions; the ordinary composition root selects this factory. | Fake is the only wired default; Real is not wired. |
| Google demo panel | `MainActivity.kt` mounts `GoogleAuthorizationDemoPanel` in settings. Its UI says it uses a fictional account, synthetic scopes, no real OAuth, network, mail or calendar data. The demo ViewModel uses the Fake gateway. | Fake demonstration only; it is not an account/consent flow. |
| Real authorization boundary | `PlayServicesGoogleAuthorizationGateway` defaults `realOAuthEnabled=false` and `selectedAccount=null`, returning `NOT_CONFIGURED` / `ACCOUNT_REQUIRED` unless explicitly configured. | Real OAuth remains disabled. |
| Gmail / Calendar HTTP | Provider session factories default `googleHttpEnabledByOwner=false`, and additionally require a Play Services gateway before opening the Google read-only session. | Live transport remains disabled. |
| Scopes | Gmail `gmail.readonly`; Calendar `calendar.events.readonly`; Calendar discovery can request `calendar.calendarlist.readonly`. | No write scopes were found in the reviewed authorization constants. Gmail's scope is Restricted; Calendar event-reading is a Sensitive-data use and its final classification must be checked in Cloud Console. |
| Android package | Gradle namespace is `app.serea.android`. The Beta build type suffix yields `applicationId=app.serea.android.beta`; the instrumentation target is isolated under `.isolated`. | The eventual Android OAuth client must match the actual shipped variant's application ID and signing certificate, not the Kotlin namespace by assumption. The final signing SHA-1 is not known here. |
| Disconnect / revocation backend | G2 has Fake-tested lifecycle/revoke contracts, invalidates local session generations synchronously, cancels in-flight provider work, and rejects late results. The application UI is not yet wired to a real account disconnect/revoke action. | Backend contract exists; live Play Services behavior and user-facing disconnect are NOT_RUN. |

Primary code references: `android/app/build.gradle.kts`, `android/app/src/main/java/app/serea/android/AppProviderComposition.kt`, `MainActivity.kt`, `googleauth/GoogleAuthorizationDemoPanel.kt`, `googleauth/GoogleAuthorization.kt`, `gmail/GmailReadPort.kt`, and `calendar/CalendarPermissionPolicy.kt`.

## Official source versions reviewed

Pages were opened directly from `developers.google.com` on 2026-10-28 UTC. The dates below are the page's visible last-modified/updated dates, not the date of a Google policy determination for Serea.

| Official source | Page version shown during review | Relevance |
|---|---|---|
| [Create access credentials — Android client setup](https://developers.google.com/workspace/guides/create-credentials) | Last updated 2026-10-12 UTC | Android client type; package name from `AndroidManifest.xml`; SHA-1 certificate fingerprint. |
| [OAuth 2.0 Policies](https://developers.google.com/identity/protocols/oauth2/policies) | Last modified 2026-08-05 | Correct platform client, project tiering, least privilege, token security/revocation, production verification, contextual consent, token invalidation. |
| [Manage App Audience — Google Cloud Console Help](https://support.google.com/cloud/answer/15549945) | Accessed 2026-10-28 UTC; no last-updated date displayed | Testing audience allowlist, tester warning, authorization/refresh-token lifetime, and separate OAuth new-user cap. |
| [Using OAuth 2.0 to Access Google APIs](https://developers.google.com/identity/protocols/oauth2) | Last updated 2026-10-16 UTC | External + Testing refresh-token expiry and other refresh-token invalidation conditions. |
| [Restricted scope verification](https://developers.google.com/identity/protocols/oauth2/production-readiness/restricted-scope-verification) | Last updated 2026-08-19 UTC | Restricted-scope verification exceptions, permitted app types, security-assessment triggers and cadence. |
| [Submit for brand verification / exceptions](https://developers.google.com/identity/protocols/oauth2/production-readiness/brand-verification) | Last updated 2026-08-19 UTC | Personal-use and test/staging exceptions; external testing conditions. |
| [Gmail API scopes](https://developers.google.com/workspace/gmail/api/auth/scopes) | Last updated 2026-09-10 UTC | `gmail.readonly` is Restricted; narrower-scope and server assessment implications. |
| [Google Calendar API scopes](https://developers.google.com/workspace/calendar/api/auth) | Last updated 2026-09-03 UTC | Event and calendar-list read-only scope meanings; public-app verification statement. |
| [Calendar sharing](https://developers.google.com/workspace/calendar/api/concepts/sharing) | Last updated 2026-09-03 UTC | Shared-calendar reader ACL semantics and the distinction between calendar owner and users with access. |
| [Google Workspace user data and developer policy](https://developers.google.com/workspace/workspace-api-user-data-developer-policy) | Last updated 2026-09-03 UTC | Gmail approved application types, disclosure and consent, Limited Use, data deletion/security, Restricted-scope controls. |
| [Google API Services User Data Policy](https://developers.google.com/terms/api-services-user-data-policy) | Last updated 2024-02-15 | General Google API user-data disclosure, privacy-policy, least-permission, Limited Use and security rules. |
| [AuthorizationClient reference](https://developers.google.com/android/reference/com/google/android/gms/auth/api/identity/AuthorizationClient) | Last updated 2025-07-23 UTC | Android `revokeAccess`, disconnect capability, and account deletion cleanup requirement. |
| [Sensitive scope verification](https://developers.google.com/identity/protocols/oauth2/production-readiness/sensitive-scope-verification) | Opened 2026-10-28; no footer date captured | Google explicitly uses reading Calendar events as an example of sensitive-scope data. Check the Cloud Console scope category before final submission. |

## Policy requirements and their Serea implications

### Android OAuth client and package/certificate binding

**Google requirements:** Register an Android-type OAuth client for the Android app. The Cloud Console Android client form asks for the package name from the app manifest and the SHA-1 certificate fingerprint. OAuth policy requires a separate OAuth client for each platform and says not to use a web client for native Android. Each production deployment tier must use a separate Cloud project under the production-app policy definition. Keep any confidential client credentials out of public repositories; a native Android client must not be treated as a place to embed a web-client secret. [Credential setup](https://developers.google.com/workspace/guides/create-credentials), [OAuth 2.0 Policies](https://developers.google.com/identity/protocols/oauth2/policies)

**Serea consequence:** Gradle currently defines the Beta application ID as `app.serea.android.beta` (and isolated test as `app.serea.android.isolated`). The source does not provide the final Owner-controlled Beta signing certificate SHA-1. Thus the final OAuth client tuple cannot be registered or verified from this work. Do not configure the isolated instrumentation package as the Personal Beta client. If build variants use different package IDs or signers, configure/test the exact tuple for each OAuth-enabled variant. The OAuth project audience/tier classification also remains an Owner decision.

OAuth policy defines production as not personal use, not development/testing/staging, and not internal-only. It defines personal use as not shared with anyone else or used by fewer than 100 people, all personally known to the developer. Therefore, the phrase “Personal Beta” does not by itself prove the personal-use exception; the actual audience and release model must be assessed. For a production app, OAuth policy says projects must be separate for deployment tiers. Google separately recommends test and production projects for test/staging use. [OAuth 2.0 Policies](https://developers.google.com/identity/protocols/oauth2/policies), [brand verification exceptions](https://developers.google.com/identity/protocols/oauth2/production-readiness/brand-verification)

**External Testing status is not general Beta approval:** an External project in `Testing` mode is authorizable only by Google Accounts explicitly listed as test users, up to 100 listed users. Google shows a tester warning. Test authorizations expire seven days after consent; if an offline flow receives a refresh token, that token also expires after seven days. The exception to the seven-day limit is only for requests limited to `openid`, `userinfo.email`, and `userinfo.profile` (or equivalents), so it does not cover Serea's Gmail or Calendar scopes. Separately, an unverified app warning for unapproved sensitive/restricted scopes is subject to a 100-new-user lifetime cap; this is not a substitute for the Testing allowlist, nor does either limit establish that an externally distributed Beta qualifies for a policy exception or permitted application type. [Manage App Audience](https://support.google.com/cloud/answer/15549945), [Using OAuth 2.0 to Access Google APIs](https://developers.google.com/identity/protocols/oauth2)

### Scope classification and verification

| Scope currently used | Google-documented capability/classification | Serea fit and verification consequence |
|---|---|---|
| `https://www.googleapis.com/auth/gmail.readonly` | Gmail API lists this as **Restricted** (“view your email messages and settings”). | Correctly avoids write access but still requests a high-sensitivity scope. For public production use, verification is required unless a specific verification exception applies. If restricted Gmail data is stored on or transmitted through a server, Google requires a security assessment; any applicability to a changed server path must be reassessed. |
| `https://www.googleapis.com/auth/calendar.events.readonly` | Calendar API says it views events on all calendars accessible to the user. The narrower `calendar.events.owned.readonly` views events only on calendars the user owns. Google’s sensitive-scope verification page expressly gives reading Calendar events as an example of sensitive data. | The current scope is justified if Serea is intended to read events from calendars shared with the user (for example, family/team calendars) where the user has event-read ACL access. The owned-only scope is a narrower candidate if the product promises to read only user-owned calendars; it will not cover non-owned shared calendars. Calendar sharing distinguishes an owner from a `reader` grantee, who can read events. Final selection depends on the intended feature and must be confirmed in Cloud Console; this review does not approve either scope. |
| `https://www.googleapis.com/auth/calendar.calendarlist.readonly` | Calendar API says it views the list of subscribed calendars. | Keep optional and request only for calendar discovery/listing; event listing itself uses an event scope. This list scope does not replace authorization to read events. Verify its classification in Cloud Console and do not bundle it into a broader initial request without a feature need. |

The Restricted-scope guide says verification applies unless the app qualifies for an exception; an app must also demonstrate an allowed product-specific application type. If the app accesses Restricted data from or through a third-party server, a Google-empanelled security assessment is required. For verified Restricted scopes the guide describes reverification and security-assessment renewal at least every 12 months after Letter of Assessment approval, and potential reassessment after adding new restricted scopes. Gmail's scope page independently says storing restricted data on servers (or transmitting it) entails a security assessment. These are not waived by read-only scope names. [Restricted scope verification](https://developers.google.com/identity/protocols/oauth2/production-readiness/restricted-scope-verification), [Gmail API scopes](https://developers.google.com/workspace/gmail/api/auth/scopes), [Calendar scopes](https://developers.google.com/workspace/calendar/api/auth), [Sensitive scope verification](https://developers.google.com/identity/protocols/oauth2/production-readiness/sensitive-scope-verification)

**Gmail application-type uncertainty:** Google Workspace policy permits Gmail scopes for built-in/web email clients, email productivity enhancements, and user-benefiting email reporting/monitoring. Its built-in/web email-client example describes compose/send/read/process, while Serea's intended Google surface is currently a read-only inbox/search/message experience. That does not establish which permitted type Google would apply to Serea. Serea may have a productivity use case, but Google review must determine if the concrete UI and intended use qualify. Do not claim approval based only on this policy list or on a personal-use exception. [Google Workspace user data and developer policy](https://developers.google.com/workspace/workspace-api-user-data-developer-policy)

### Google user-data rules that apply even to read-only use

The Workspace policy applies its Limited Use rules to both Sensitive and Restricted scope data and data derived from it. In practical terms, Google requires:

- request data only for an approved user-benefiting use and the smallest required scope; use it only for the prominent user-facing feature disclosed to the user;
- provide accurate in-app disclosure in normal app use, immediately before consent; require affirmative action; do not treat navigation/dismissal as consent; provide help explaining how to manage and delete data;
- publish a privacy policy describing access/use/storage/sharing, link it in OAuth configuration when public, and update it plus re-obtain consent before materially new data use;
- allow data transfer only for the user-facing purpose with user consent or the listed security/legal/asset-transfer cases; do not sell/use data for ads; do not retain permanent copies or cache beyond permitted API cache headers;
- do not transfer/use Google data to train or improve general AI/ML models. The policy's narrow exception is the specific user's personalized model for the appropriate user-facing feature;
- protect data in transit and at rest; for Restricted scopes follow CASA. Required measures include accepted encryption for data on portable media, outside Google/developer systems, across externally managed networks, and at rest; modern secure protocols; encrypted-at-rest tokens; appropriate key management; prompt-injection protection; and prompt security-incident notice to Google.

Google describes ISO/IEC 27001-style ISMS and OWASP Top 10 robustness as **recommended** security practices; they are distinct from the enumerated required measures. Google may require periodic third-party security assessment depending on API and user/grant counts. [Workspace data policy](https://developers.google.com/workspace/workspace-api-user-data-developer-policy), [Google API Services User Data Policy](https://developers.google.com/terms/api-services-user-data-policy)

### Token revocation, disconnect, and deletion

**Google requirements / documented obligations:**

- OAuth tokens must never be transmitted in plaintext; if stored, store encrypted at rest. Revoke tokens when account access or a previously granted permission is no longer needed, then permanently delete them from the application/system. Keep the Cloud consent scope list aligned with scopes requested by the application and revoke tokens for removed scopes at the earliest opportunity. [OAuth 2.0 Policies](https://developers.google.com/identity/protocols/oauth2/policies)
- `AuthorizationClient.revokeAccess` removes this application's access and future authorization requires consent again. Its reference says apps **are required** to let signed-in users disconnect their Google account. If the user deletes their account, the app **must delete** information obtained from Google APIs. Clearing the local access-token cache alone is not equivalent to revocation. [AuthorizationClient](https://developers.google.com/android/reference/com/google/android/gms/auth/api/identity/AuthorizationClient)
- Refresh tokens can become invalid at any time. If the app needs a revocation notification to provide its UX, OAuth policy says integrate Cross-Account Protection. [OAuth 2.0 Policies](https://developers.google.com/identity/protocols/oauth2/policies)

**Engineering recommendations for Lane A (not additional Google policy text):** expose clear shared-account disconnect/revoke and feature-only disable separately; before awaiting remote revoke, synchronously make local sessions unusable, cancel in-flight Gmail/Calendar calls, clear result/cache state, and reject late responses by generation/account binding; then invoke Play Services revocation and present partial failure/retry state without re-enabling the account. Delete Google-derived local information on an account-deletion request and honor in-app deletion requests. Avoid storing refresh tokens; if future architecture requires persistence, use platform-backed encryption/key management and independently review migration/deletion. The existing G2 Fake contract models local invalidation/cancellation, but actual Android account consent/revocation has not been exercised.

## Policy obligations versus engineering recommendations

| Topic | Google policy requirement / rule | Engineering recommendation for Serea |
|---|---|---|
| OAuth client | Correct Android client type, exact package ID and certificate SHA-1; separate platform client; production-tier project separation where production definition applies. | Register only after Owner selects final Beta package/signing and audience; keep debug/isolated OAuth configurations separate from Beta. |
| Scopes | Minimum necessary scopes; production sensitive/restricted verification unless an exception applies; scope list must match app behavior. | Ask Gmail read scope only on explicit Gmail entry; Calendar event scope for events, optional calendar-list scope only when discovery is used; do not request unused/future features. |
| Account consent | User must understand data access/use; in-app disclosure preceding consent, affirmative action, contextual/incremental authorization where possible; unsupported/denied scope must disable dependent feature. | Build explicit states for disconnected/authorization-required/authorizing/authorized/expired/revoked/error and per-account session replacement. |
| Revocation | Provide disconnect capability; revoke unneeded scope/account access and delete revoked tokens; delete Google data if user's account is deleted. | Atomically shut local session off and cancel calls before network/Play Services revoke; clear in-memory data and ensure old account results cannot render. |
| Data use | Limited Use; data only for prominent, disclosed, appropriate feature; no unauthorized sharing, ads, permanent copies, or general model training. | Keep Google content local and out of cloud AI by default. Any future transfer needs a separately user-visible and consented feature plus provider/legal review. |
| Restricted data security | Required encryption and secure transport, key management, prompt-injection protection, CASA; possible periodic assessment and incident reporting. | Threat-model email content as untrusted; test injection boundaries; preserve redaction and data isolation; evaluate CASA applicability with actual data path before launch. |
| Development projects | Separate tiers are mandatory for production apps under the OAuth policy definition; test/prod project separation is also recommended for testing. | Use a disposable/test OAuth project only after Owner setup; never place test credentials in product source or a release APK. |

## G2 verification provenance

The six focused Google lifecycle/session/HTTP JVM classes are listed in `docs/agent-reports/g2-p0-backend-acceptance-review.md`. The prior local run recorded there passed 92 tests, 0 failures, 0 errors, 0 skipped, `BUILD SUCCESSFUL`, at source commit `eaaeb24af4c6dac2d7bc41cbba8fab32fdb79f4f`. The prior PR #132 Android Unit CI evidence is run `40599571275`, artifact `12778060790` (`sha256:a5ed666eaeea203e30cdeea648a271c19a6639ea572fc82806a52ff5b2d48430`); its 335-case JUnit packet includes these six Google classes with 92/92 passing. These are source/backend tests only, not live OAuth acceptance.

A fresh local focused rerun was attempted from this worktree with the same six-class command and `--rerun-tasks`. Gradle failed before test compilation/discovery because `/usr/lib/jvm/java-21-openjdk-amd64` is a JRE without `JAVA_COMPILER`; no `javac` executable is installed. Therefore this attempt is **ENVIRONMENT / NOT_RUN**, not a test failure or pass. The implementation source is unchanged from the earlier tested revision; this report commit is documentation-only. `git diff --check` is the verification available for this change.

No Google Emulators, real accounts, OAuth, tokens, Gmail/Calendar endpoints, production credentials, or Cloud Console scopes were exercised. Live consent, scope grant/partial-grant behavior, actual token expiry/revocation, account-deletion cleanup, and end-to-end UI disconnect remain **NOT_RUN**.

## Owner decisions and activation checklist

1. Decide intended audience and whether this exact beta fits Google's personal-use or testing exception. The personal-use criterion requires only the developer or a few users personally known to the developer; a wider Beta cohort cannot be assumed to qualify. If using External/Testing for development, enumerate the intended test users and accept the warning, 100-user allowlist ceiling, and seven-day grant/refresh-token expiry. Testing mode alone does not qualify a public external Beta or settle restricted-scope eligibility.
2. Select the final Android Beta package ID and signing identity, then provide/record the corresponding SHA-1 for the Android OAuth client. Do not use `app.serea.android.isolated` as the Beta client identity.
3. Select Cloud project ownership, audience/publishing tier, test-user strategy, and separate tier projects required by the production classification. Review the app's exact Gmail permitted application type with Google's verification process.
4. Supply a public privacy policy and required Limited Use statement, contextual in-app disclosure, support/deletion instructions, and the exact data retention and transfer behavior.
5. Decide whether reading non-owned shared calendars is a P0 requirement. If yes, justify `calendar.events.readonly`; if no, assess `calendar.events.owned.readonly` as the narrower candidate. Confirm final scope categorization and allowed scope selection in Cloud Console Data Access.
6. Determine the final sensitive/restricted classifications in Cloud Console Data Access; submit required verification and security assessment(s) before public launch. Confirm whether direct-device-only Gmail access has any assessment obligations under Google's interpretation; any server proxy, server storage, or cloud processing of restricted data changes the analysis.
7. Explicitly approve real OAuth activation and user account testing. Until then, leave Fake defaults and the `realOAuthEnabled` / `googleHttpEnabledByOwner` gates closed.

## Verification and source status

```text
OFFICIAL_SOURCE_ACCESS: AVAILABLE; primary Google developer pages opened successfully on 2026-10-28 UTC
REPORT_CHANGE_SCOPE: documentation only
REAL_OAUTH: NOT_RUN / disabled
LIVE_GOOGLE_ACCOUNT: NOT_USED
OWNER_APPROVAL_REQUIRED: final package+signing identity, audience/project tier, OAuth activation, real-account test, verification submission
```
