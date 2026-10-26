# Personal Beta Owner Decision Ledger

Updated: 2026-10-26  
Authority: functional integration candidate branch `codex/serea-personal-beta-functional-integration`; frozen green base PR #117 at `151ea9d66d32de01b195f4262beb67516823c281`.

This ledger collects approvals that must come from the Owner. Until approved, affected integrations remain disabled or NOT_RUN. These items do not block Fake/offline development. No approval is requested by this checkpoint.

| ID | Decision needed | State | Required evidence / Owner input | Agent boundary before approval |
|---|---|---|---|---|
| O-01 | Enable Google OAuth and connect a real Google account for Gmail/Calendar read-only acceptance | NOT_REQUESTED / NOT_RUN | Confirm account, target package, OAuth client/configuration, allowed read-only scopes and Google consent/review conditions | Keep Google sessions Fake by default; do not start OAuth or access a real account |
| O-02 | Select and activate a real AI Provider | NOT_REQUESTED / NOT_RUN | Confirm user eligibility/age and region, current terms and data retention/learning settings, endpoint/model, pricing/rate limits and explicit budget/egress policy | Use only the fictional local Demo route; no API key, external request or Google-origin data egress |
| O-03 | Use production Beta signing identity and distribution | NOT_REQUESTED / NOT_RUN | Owner-provided signing configuration, certificate fingerprint, package identity, update lineage and distribution target | Continue temporary-key/mock contract verification; do not generate or imply a production key |
| O-04 | Install and accept on the physical Pixel 7a | BLOCKED BY DEVICE AVAILABILITY / NOT_RUN | Pixel 7a must be made available and Owner must authorize installation/acceptance scope | Use disposable API 26/35 Emulator and CI evidence; do not request device data deletion or reset |
| O-05 | Merge integration candidate into `main` | NOT_REQUESTED | Owner review of final same-HEAD Required, security, release and evidence packet | Keep all PRs Draft/open; never merge automatically |
| O-06 | Final Personal Beta GO / release authorization | NOT_REQUESTED | Explicit Owner decision after Required CI, service acceptance, production identity, device acceptance and residual-risk review | Report GO-candidate or NO-GO only; do not distribute or claim release readiness |

## Current technical boundaries

- Functional validation currently uses synthetic Gmail, Calendar, Task and Chat fixtures. Hosted API 26 Focused passed on source `bfddbc284b3198708b7399d93df4ae81cc40f988`; API 35 run `40288581135` is still in progress.
- Lane B PR #120's focused release/SBOM run passed, but its lockfile inventory is explicitly partial and does not attest a Beta APK.
- Lane C PR #121's adversarial test changes remain independently reviewable; selected JVM tests are present on Lane A. Neither lane PR has been merged.
- PR #117 Required evidence applies only to its frozen SHA. The functional integration candidate still needs its own formal Required and cross-platform runs after the Draft PR is opened.
- Personal Beta remains NO-GO. Real service use, production signing/distribution, physical device acceptance, main merge and final GO stay Owner-controlled.
