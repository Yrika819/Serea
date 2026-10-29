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

- Functional validation currently uses synthetic Gmail, Calendar, Task and Chat fixtures. Hosted API 26 Focused passed 26/26 on source `bfddbc284b3198708b7399d93df4ae81cc40f988`. API 35 on that source had zero tests because its emulator fingerprint was absent from the test-runner allowlist; the test-only correction is commit `613b338ad113cc08ee2ecd656f72a074708264b5`, under serialized API 26/35 validation in run `40292160073`.
- Lane B PR #120's focused release/SBOM run passed, but its lockfile inventory is explicitly partial and does not attest a Beta APK.
- Lane C PR #121's adversarial test changes remain independently reviewable; selected JVM tests are present on Lane A. Neither lane PR has been merged.
- PR #117 Required evidence applies only to its frozen SHA. The functional integration candidate still needs its own formal Required and cross-platform runs after the Draft PR is opened.
- Personal Beta remains NO-GO. Real service use, production signing/distribution, physical device acceptance, main merge and final GO stay Owner-controlled.

## G9R campaign update — 2026-10-29 11:40 UTC

This update supersedes the dated technical snapshot above for current progress. Owner decisions O-01 through O-06 remain outstanding or blocked as stated; no approval is requested or implied by this update.

- Current parent G9R candidate: PR #155, source `683854ba94cb3a1e1972b73d0f1f60392f2271d6`, tree `e6228614a6acf7d3ac440c66b7038bc8728f05ee`. API26 Required is 105/105 (103 Required + 2 Additional), 0 failures/errors/skips. Separate API26 Google/Gmail UI is 16 tests with 3 failures. API35's bounded Chat/IME diagnostic failed with `windowFocus=false`; API35 Required and Google/Gmail UI are NOT_RUN. The exact-source Required Gate is failed because the emulator result was not accepted. No gate was altered.
- Process Death and Data Isolation have independent passing evidence on that #155 run. No Pixel 7a device was used; the CI reacceptance bundle is not physical-device acceptance.
- Native PR #158, source `80e9990625b000e9d56a78743a7460ac120d4979` / tree `a07d653dd7a6a03323f81fa2332ae14a91eaceaf`, has local focused JVM/compile evidence but no successful real model inference. Its current hosted run was in progress at 11:40 UTC. Independent review requested generation-bound cancellation evidence; this remains open pending a successor source and review.
- The standard local model is UNDECIDED. LFM3, Bonsai 2, Qwen3.5, Gemma 4 and FunctionGemma remain candidates at static metadata/license/format screening only. No Serea model comparison or Pixel 7a measurement has been completed.
- Google access remains Fake-only, Cloud AI remains disabled, and no credentials, account data, production signing key, device, or distribution were used. Personal Beta remains NO-GO.
