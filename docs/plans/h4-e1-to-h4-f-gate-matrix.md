# H4-E1 → H4-F gate matrix

This matrix records dependencies, evidence and explicit approval boundaries for the Personal Beta Calendar lane. Dates are targets, not permission to weaken safety or Required CI.

## Delivery targets

| Gate | Target | Completion evidence |
|---|---:|---|
| H4-E1 — Personal decision/readiness | Nov 24, 2026 | Owner Personal decision recorded; source/signing audit, private Cloud runbook, H4-E2 design and test matrix reviewed. No Cloud/API/OAuth activity. |
| H4-E2 — Private Live Gate | Nov 28, 2026 | Reviewed implementation candidate; separate compile-time Private HTTP gate default-off; variant/security tests; Required CI and independent review. No real Google connection unless separately authorized. |
| H4-F — limited real connection | Dec 4, 2026 | Only after explicit Owner approvals; designated Personal project/client/artifact/certificate/account; narrow Calendar read-only and primary-only demonstration; redacted evidence and teardown/revoke plan. |
| Feature Freeze | Dec 10, 2026 | Scope locked; unresolved security/Required CI failures block. |
| Security Review | Dec 17, 2026 | Independent security review and issue dispositions. |
| RC | Dec 24, 2026 | Release-candidate evidence; signing/distribution approvals still separate. |
| Personal Beta GO/NO-GO | Dec 31, 2026 | Owner decision based on evidence. If real Google is late, retain a clearly Fake-labeled Demo Beta; do not imply Live access. |

Gmail, Cloud AI and Tasks lanes are not changed by this plan.

## Gate and approval matrix

| Gate | Required evidence | Owner approval / stop condition |
|---|---|---|
| E1 Personal choice | Google official audience guidance; creator account and one-account limitation; recorded rule that Personal cannot be changed to External/Internal and existing External/Internal cannot be changed to Personal | Decision recorded, but project creation/change remains separately unauthorized. |
| Project readiness | New project preferred; audience visibly Personal; project/account privately recorded | Owner approves exact project action. If existing project is External/Internal or audience unclear, stop; no conversion attempt. |
| Android identity | Exact intended installable APK provenance, Private package `app.serea.android.privatebeta`, versionCode, required `apksigner verify --verbose --print-certs <apk>` result, signer SHA-1 and SHA-256, and update compatibility | Owner approves certificate/artifact. Fail closed on verification failure, ambiguous/multiple unexpected signers, or fingerprint mismatch. CI, Debug, Disposable and future Production identities are not substitutes. No key generation/retrieval/use under E1. |
| OAuth client / API | Android client identity matches package + APK signer SHA-1 (with SHA-256 independently compared); Calendar API only; exact events-readonly scope; quota/billing reviewed | Separate Owner approval for client creation and API enablement. If Personal onboarding combines project/audience configuration with client creation, require explicit combined approval before submission. Private project/client IDs and account data stay in private record. |
| E2 implementation | Private-only compile-time HTTP gate, default false; OAuth and HTTP independent; non-Private variants incapable of product Google HTTP; HTTPS fixed endpoint; no MockWebServer product path; gate OFF selects default Fake, gate ON fails visibly before provider creation unless valid OAuth/session and eligible gateway select Live, and Live has no silent Fake fallback | Dedicated code approval and Required CI. H4-E1 does not authorize implementation or enabling the Live gate; do not set gate true during E1. |
| H4-F real connection | Limited approved artifact and owner-created account; current session/scope/primary validated; minimal read-only result; redacted logs/evidence; disconnect/revoke procedure tested | Fresh explicit approval naming project/client, build, signing identity, account, operation and time window. No testing with another real account. |
| Release | Security review, formal variant/Required CI and release identity evidence | Separate approvals for production signing, APK installation/distribution and device operations. |

## H4-F GO / NO-GO conditions

**GO only if all are true:**

- Personal audience is confirmed in the approved project, with the creator account being the sole test account.
- Android OAuth package and SHA-1 match the exact artifact signing certificate; SHA-256 independently matches the approved identity record; version/update compatibility is established.
- H4-E2 independent OAuth and HTTP gates are reviewed, default-off and isolated to Private product; Public/Beta/Debug cannot make Google product HTTP requests.
- Scope is exactly `https://www.googleapis.com/auth/calendar.events.readonly`; request uses only `primary`; CalendarList, other IDs and writes are rejected before auth/token/network.
- HTTPS/Google endpoint, session/account validation, secret/data log redaction, no Cloud AI egress, no silent Fake fallback, and 401/403/429/revocation/disconnect/account-change/process-death fail-closed behavior pass tests.
- Required CI and independent review are green/accepted; narrow real-connection approval is current and explicit.

**NO-GO** on any identity mismatch, ambiguous audience, unauthorized account, scope broadening, nonprimary/CalendarList/write path, unexpected network destination, secret/event logging, implicit Fake fallback, stale session acceptance, Required CI failure, or missing Owner approval. Preserve Fake-first Demo Beta with clear labeling; no deadline overrides these conditions.

## Owner approvals still required

1. Whether to create a new dedicated Personal Cloud project, and the actual project/account to use.
2. Cloud project creation, Android OAuth client creation, Calendar API enablement, quota/billing action, and any consent configuration.
3. Selection/verification and any use of an Owner-controlled Private test signing certificate and the exact artifact to install; no production signing approval is implied.
4. H4-E2 implementation review and any dedicated Private compile-time HTTP gate configuration.
5. H4-F real OAuth and Calendar read: explicit account, artifact, scope, operation, and test window; separately approve credential revocation/cleanup actions as needed.
6. Any APK signing, install, distribution, physical-device operation, production signing, Draft readiness or merge—none is authorized by H4-E1.
