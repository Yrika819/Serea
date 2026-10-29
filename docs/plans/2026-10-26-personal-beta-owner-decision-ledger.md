# Personal Beta Owner Decision Ledger

Updated: 2026-10-29
Authority: this file is the canonical source for Owner decision IDs and states. Historical reports may link here but must not assign a second meaning to an O-ID. Implementation and CI evidence is recorded separately against its exact source SHA/tree.

This ledger collects approvals that must come from the Owner. Until approved, affected integrations remain disabled or NOT_RUN. These items do not block Fake/offline development. The 2026-11-03 and 2026-11-10 rows are decision-preparation checkpoints, not permission to perform the listed real-world actions. No approval is requested by this checkpoint.

## Canonical Owner decision matrix

| ID | Decision / target checkpoint | State | Required evidence / Owner input | Boundary before approval |
|---|---|---|---|---|
| O-01 | Beta package identity, Demo/Beta coexistence, and signing-custody policy — **2026-11-03** | PENDING_OWNER_DECISION | Ratify the stable Beta package ID (current candidate `app.serea.android.beta`), whether it coexists with Demo/debug `app.serea.android.demo`, side-by-side storage/update expectations, and the signing custody/recovery policy. Specify how the release certificate’s SHA-1 (OAuth binding) and SHA-256 (release evidence) will be protected and verified. | This is policy selection only. Do not create, read, or use a production key, configure OAuth, install on a personal device, or distribute an APK. CI/debug certificates are not the Owner certificate. |
| O-02 | Google Cloud project, Android OAuth client, account, and Calendar read-only plan — **2026-11-10** | PENDING_OWNER_DECISION / NOT_RUN | Select the dedicated development/test project and accountable project/API owners; review audience, consent screen, test-user eligibility, and verification requirements. Plan Calendar API only first, using the narrowest justified read-only scope; record package and matching certificate SHA-1/SHA-256 in an Owner-controlled record after O-01/O-05 prerequisites are satisfied. | No project/client activation, OAuth flow, real account, token, or Google data until separate explicit authorization. Connection, Google→Chat handoff, and Cloud-egress consents remain distinct. |
| O-03 | Cloud AI provider eligibility, terms, budget, and data handling — **2026-11-10** | PENDING_OWNER_DECISION / NOT_RUN | For any proposed provider, confirm user age and regional eligibility, current terms, data retention/training and deletion settings, endpoint, pricing/rate limits, explicit budget, and data-egress policy. Compare with a local-only option. | No provider credentials, API requests, billing, or Google-origin Cloud egress. Cloud remains disabled by default. |
| O-04 | Google-origin data transfer to Cloud — before any such transfer | NOT_APPROVED / NOT_RUN | A separate explicit decision would require a documented purpose, specific fields, recipient, user disclosure/consent, retention/deletion, provider eligibility, and egress/security review. | Default deny. Calendar or Gmail access and Google→Chat handoff do not grant Cloud-transfer consent. No Google-origin Cloud egress is authorized by this ledger. |
| O-05 | Production Beta signing identity, custody/recovery, and distribution — roadmap target **2026-11-21** | PENDING_OWNER_DECISION / NOT_RUN | After O-01 policy, choose Owner-controlled key custody and recovery; validate package, update lineage, certificate fingerprints, protected signing workflow, and any distribution destination/tester group. | No production key creation/loading/use, signed Beta build, or distribution. Continue only CI temporary-key/mock contract checks. |
| O-06 | Pixel 7a acceptance scope — criteria review **2026-11-10**; physical acceptance at November/final G13 gate | BLOCKED_BY_DEVICE_AVAILABILITY / NOT_RUN | Define non-destructive install/update, package/certificate identity, encrypted-data continuity, offline local-model operation, cancellation/recovery, thermal/performance, and forward-update rollback criteria. Owner must separately authorize device access and installation. | No physical device operation, reset, or personal-data deletion. Emulator results are not Pixel acceptance. |
| O-07 | Integration candidate merge into `main` — RC review | PENDING_OWNER_DECISION | Review final same-HEAD Required/security/release evidence and the integration decision packet. | Keep PRs Draft/open; do not merge or alter protected `main` automatically. |
| O-08 | Personal Beta GO / Conditional Preview / NO-GO — **2026-12-31** | PENDING_OWNER_DECISION; current status **NO-GO** | Decide only after all P0 gates, service acceptance, production identity, update/migration evidence, authorized Pixel acceptance, licensing/provenance, and residual risks are reviewed. | No Beta distribution or GO declaration before the Owner’s explicit decision. |
| O-09 | Gmail Restricted scope (`gmail.readonly`) — separate later Owner gate | PENDING_LATER_GATE / NOT_RUN | Keep Gmail disabled for initial Calendar-only evaluation. Before any Gmail decision, complete separate data-flow/purpose, retention/deletion, verification/security-assessment, eligibility, and user-disclosure review. | Calendar approval does not include Gmail. No restricted scope activation, real mailbox, or real email data. |
| O-10 | Standard local-model selection and redistribution — review at **2026-11-10**, final selection deferred | UNDECIDED / NOT_RUN | Review pinned model revisions, licenses, formats, redistribution notices, and compatibility evidence. Keep the default model undecided until Pixel 7a measurements and independent quality/security/license review support a choice. No multi-model residency or FunctionGemma router without demonstrated benefit. | No model default change or weight redistribution. Do not download/commit weights as part of the Owner decision packet; all model execution/performance evidence remains source-bound and separately reported. |

The 2026-11-10 checkpoint may decide to defer or keep a capability disabled; it does not activate Google, Gmail, Cloud AI, or a local-model default. O-02, O-03, O-09, and O-10 are independent decisions. Later activation still requires the corresponding explicit Owner approval and technical gates.

## Current technical boundaries

- Functional validation currently uses synthetic Gmail, Calendar, Task and Chat fixtures. Hosted API 26 Focused passed 26/26 on source `bfddbc284b3198708b7399d93df4ae81cc40f988`. API 35 on that source had zero tests because its emulator fingerprint was absent from the test-runner allowlist; the test-only correction is commit `613b338ad113cc08ee2ecd656f72a074708264b5`, under serialized API 26/35 validation in run `40292160073`.
- Lane B PR #120's focused release/SBOM run passed, but its lockfile inventory is explicitly partial and does not attest a Beta APK.
- Lane C PR #121's adversarial test changes remain independently reviewable; selected JVM tests are present on Lane A. Neither lane PR has been merged.
- PR #117 Required evidence applies only to its frozen SHA. The functional integration candidate still needs its own formal Required and cross-platform runs after the Draft PR is opened.
- Personal Beta remains NO-GO. Real service use, production signing/distribution, physical device acceptance, main merge and final GO stay Owner-controlled.

## Recovery campaign checkpoint — 2026-10-29 15:19 UTC

This status refresh does not grant any approval and does not change O-01 through O-10.

- G8 Evidence Closure, Android Recovery, Native Runtime, G9, and G10 remain **PARTIAL**; Personal Beta remains **NO-GO**.
- PR #162 is `6b7a741362b58b6bbc7728e69de90d929391dcaf` / tree `e61db67fcdf0ce7c5c5ab7e93efaef3bac4c1b5d`. Android build, lint, unit, integration, and security jobs passed exact source. API26 instrumentation step failed; artifact `12907539920`, SHA-256 `d08313d38c2f211a9cfc092153c7d912cec4faa8832c287956fbd15eb013b2c8`. Failing identities remain unknown because artifact download returned 403 and job-log API 503 while the job remained active. API35 instrumentation is still running; no API26 evidence is reused for API35.
- PR #163 model matrix source/tree: `2c109276bcbdd4e14fb2b6e9558962516f2df32c` / `b04164da95bf66f43fd08e5537e4d0e242d6a566`; static review only. Standard model remains UNDECIDED; runtime quality/performance and Pixel results remain NOT_RUN.
- PR #165/#166 Native source/tree: `8dde33024b76efc3d32812e7d580f8a33db94c9f` / `4fca4c7fea82bd8c3bf59481fc8341d8165f9c54`. Static Native security review ACCEPTED; focused x86_64 and ARM64 software-emulated Android jobs are queued, with no new inference artifact. Ordinary Chat remains disconnected.
- No real OAuth/account data, Cloud credentials/billing, production signing, Pixel device use, main merge, or Beta distribution occurred. Google-origin Cloud egress remains unauthorized.

Refresh the decision packet with terminal exact-source CI and PR #163 model evidence before the 2026-11-03/10 Owner checkpoints. Decisions remain Owner-controlled.

### Android evidence update — 2026-10-29 15:29 UTC

PR #162 `6b7a741362b58b6bbc7728e69de90d929391dcaf` / tree `e61db67fcdf0ce7c5c5ab7e93efaef3bac4c1b5d` now has inspectable API26/API35 artifacts. Each API independently passed Required **105/105**, Calendar 19/19, Chat 16/16, cursor 1/1, Process Death and Data Isolation. Each API's separate 16-test Google/Gmail UI stage failed the same two tests:

- `GoogleCalendarDemoLoopbackUiTest#consentUiOpensLoopbackAgendaAndAccountSwitchDisconnectsIt` — the account-A semantics tag was absent after returning from Calendar.
- `GoogleCalendarHandoffRevocationUiTest#confirmedCalendarHandoffCannotSendAfterSyntheticAccountSwitch` — timed out waiting for the `Design review` event; diagnostic handler did not catch `ComposeTimeoutException`.

API26 artifact `12907539920`, digest `sha256:d08313d38c2f211a9cfc092153c7d912cec4faa8832c287956fbd15eb013b2c8`; API35 artifact `12909203169`, digest `sha256:a265911ad3c8e6aa5d6f231d7fc2d88591e46fa4163f77f9b73cd79059bc2c77`. The UI failures remain unresolved; no gate was weakened. Personal Beta stays NO-GO. No Owner approvals changed.
