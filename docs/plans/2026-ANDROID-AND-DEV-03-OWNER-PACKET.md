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

## Validation ledger

GitHub evidence below is from Android run
[39182801325](https://github.com/Yrika819/Serea/actions/runs/39182801325)
on code SHA `473a8d8707f30da1046105266db84e837e9ff196`. The final documentation
commit will trigger the complete PR check set again; do not treat a queued or
skipped job as a pass.

| Area | Result |
| --- | --- |
| Android JVM tests | PASS, 28 test methods across ViewModel, Provider Adapter and Egress contract suites (`Android Unit`). |
| Mock HTTP/provider tests | PASS, 13 test methods, including fragmented fixed-seed streaming, redirect, TLS, timeout and error mapping (`Android Integration`). |
| Egress/security tests | PASS, 7 contract tests (`Android Security Tests`). |
| API 35 instrumentation | PASS, 15 tests; JUnit XML: failures 0, errors 0, skipped 0. Required class counts: EncryptedHistory 12, ChatScreen 3. |
| API 26 instrumentation | PASS, 15 tests; JUnit XML: failures 0, errors 0, skipped 0. Required class counts: EncryptedHistory 12, ChatScreen 3. |
| Conversation/lifecycle stress | PASS in the instrumentation suite: 100 conversation create/read/delete operations, 100-message ordered restore, concurrent write isolation, late insert versus delete, and Activity recreation. |
| Streaming stress | PASS: fixed-seed 256 short deltas across fragmented Mock HTTP chunks. |
| OS Process Death | NOT_RUN. Database reopen and Activity recreation do not prove recovery after actual process death. |
| Branch coverage | NOT_MEASURED. |
| Local verifier/docs checks | PASS: 5 verifier regression tests and 145 Markdown files validated locally. |
| Live Ollama Cloud request | NOT_RUN; no API key or user data was supplied. |

Failure-and-repair history: run `39180509846` exposed two API 26 screen-test
assumptions; run `39181121873` exposed history-dependent UI assertions; run
`39182084749` showed that a combined SDK image installation could block both
API test levels; and run `39182369691` caught an ambiguous history row
assertion. Tests and workflow were corrected. The passing API evidence is from
run `39182801325`; the required gate remains strict and keeps the API 35 job
name unchanged.

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
  behavior have Mock HTTP evidence. API 35 and API 26 passed this run. Actual
  process-death recovery and longer repeated stress runs remain open.
- **AND-06:** the emulator gate validates API 35 and API 26 JUnit XML,
  per-class expected test counts, and zero failure/error/skip results. Full CI,
  native platform validation and SQLite portability are independent checks and
  must also finish successfully on the final documentation commit.
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
