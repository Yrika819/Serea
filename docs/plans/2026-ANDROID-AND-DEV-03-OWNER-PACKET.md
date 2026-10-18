# AND-DEV-03 Android chat readiness review

Date: 2026-10-19 · Status: implementation candidate, Owner decisions pending

This packet records the AND-DEV-03 implementation boundary and decisions that
remain necessary before production cloud chat or Beta release. Implementation
and Mock Provider evidence do not ratify ADR-0044/0045, select a production
provider, or grant ordinary user-message egress.

## Implemented candidate scope

- Android-only conversation IDs, encrypted Room history, role and status
  metadata, and bounded chronological context for the deterministic Mock
  Provider. Context entries retain their original classification and source
  message ID. Only completed turns enter subsequent context; recent turns are
  retained within the local approximate token budget.
- A single bounded request per user send, explicit cancellation, timeout and
  uncertain-outcome reporting. The fixed Ollama Cloud contract probe remains
  the only Cloud Candidate payload. The ordinary chat path remains Mock-only.
- Egress checks apply to current input and every context entry. UNKNOWN,
  PRIVATE, SECRET, CREDENTIAL and PERSONAL are rejected. No UI confirmation or
  mock success changes that contract.
- Assistant ciphertext persistence and the associated user-message
  `COMPLETED` transition occur in one Room transaction. A pending turn can be
  completed once, and recovery changes orphaned `PENDING` to
  `OUTCOME_UNKNOWN` without automatic resubmission.
- API 35 remains the required emulator gate. The candidate workflow also runs
  the same instrumentation suite on API 26 and validates JUnit XML and the
  expected history and Compose test-class counts separately for each API.

## Validation ledger for this branch

Update these values from the final commit's local and GitHub evidence before
review. Never infer execution from compilation or a successful emulator boot.

| Area | Result |
| --- | --- |
| JVM/ViewModel tests | NOT_RUN in the current Codex environment: Gradle distribution download failed; SDK is absent. |
| Mock HTTP/provider tests | NOT_RUN in the current Codex environment. |
| Egress/security tests | NOT_RUN in the current Codex environment. |
| API 35 instrumentation | NOT_RUN locally; required GitHub Actions evidence pending for this branch. |
| API 26 instrumentation | NOT_RUN locally; candidate GitHub Actions run pending. |
| Process death | NOT_RUN; DB reopen and Activity recreation do not prove OS process-death recovery. |
| Branch coverage | NOT_MEASURED. |
| Live Ollama Cloud request | NOT_RUN; no API key or user data was supplied. |

## Owner decisions before production cloud chat

| Decision | Recommendation | Alternative | Main risk | Function enabled after decision |
| --- | --- | --- | --- | --- |
| Provider deployment | Prefer an authenticated Serea backend for centrally managed credentials and spend controls; evaluate direct Ollama only after mobile credential and abuse review. | Direct Android-to-Ollama Cloud. | Backend adds service operations and data-processing obligations; direct mobile increases credential distribution and abuse risk. | Provider adapter for ordinary messages. |
| Data eligibility | Keep all ordinary user-authored text and all stored history denied until an explicit trusted classification and minimization contract exists. | Permit a narrowly defined, user-confirmed PUBLIC subset after threat-model review. | User confirmation alone cannot prove that content is public or non-sensitive. | Typed preparation path for selected user content. |
| Retention and deletion | Set an explicit local retention period and provider retention/training terms; preserve backup exclusion and authenticated local encryption. | User-managed indefinite local history, with provider-specific terms accepted separately. | Key loss makes local encrypted history unreadable; deletion does not guarantee physical flash erasure or remote deletion. | Release-grade history policy and disclosure. |
| Credential custody | Keep hardware-backed Android Keystore required for direct-provider credentials; use short-lived backend tokens if the backend option is selected. | Approve a reviewed software-backed fallback for specified devices. | Hardware-backed availability varies; software-backed secrets have weaker extraction resistance. | Credential enrollment and refresh for the approved deployment. |
| Cost and reliability | Configure provider-side billing caps and alerts; keep local daily/monthly controls labeled as soft limits; no implicit retries. | Permit a user-selected retry with explicit uncertain-outcome messaging. | Local estimates do not cap provider charges; timeout may follow provider-side completion. | Controlled model use and explicit retry. |
| Model and terms | Recheck Ollama's current official model list, pricing, terms, data region, retention and training conditions at approval time. | Select another provider only after equivalent contract review. | Availability, price and terms change after this packet date. | Enable only the approved model IDs. |
| Distribution | Complete security and privacy review before Play or external Beta; choose personal APK versus closed testing explicitly. | Internal APK testing with named participants and a short retention policy. | Store disclosures, OAuth review and regional obligations depend on distribution. | Beta distribution. |

## Roadmap impact

- **AND-04:** local conversation, storage, context construction, Mock request
  path and fail-closed egress are implemented in this candidate. Owner provider,
  classification, retention, credential, cost and distribution decisions
  remain open.
- **AND-05:** streaming, transport error mapping, cancellation and fixed-probe
  behavior exist from Phase 2; process-death, API 26 and repeated stress
  evidence must pass before their reliability claims can be closed.
- **AND-06:** emulator and regression gates are extended to validate API 35 and
  API 26 JUnit evidence. Coverage measurement and CI evidence for this commit
  remain pending.
- **2026-11-08 AI connection target:** no ordinary cloud chat can be enabled
  until Owner decisions, live credential custody and a separate approved
  connection check are complete. Mock work alone does not make this target
  production-ready.
- **2026-12-31 Beta target:** plausible only after the remaining Owner
  decisions, API 26/API 35 evidence, real process-death recovery, security and
  privacy review, and distribution-specific disclosures are completed.

## Next engineering priority

First, obtain passing API 35 and API 26 instrumentation evidence for the final
head. Then add a reproducible actual process-death recovery test and repeatable
conversation/streaming stress cases. Keep production user-message egress
disabled until the Owner's decisions above are recorded separately from this
proposal.
