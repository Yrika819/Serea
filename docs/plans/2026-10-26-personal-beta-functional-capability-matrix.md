# Serea Personal Beta Functional Capability Matrix

Status date: 2026-10-26

This is the F1 inventory for the functional integration branch. The source of truth for prior Android Required closure remains PR #117 at `151ea9d66d32de01b195f4262beb67516823c281`; this branch starts from that exact head. PR #117 and `main` are frozen.

## P0 capability matrix

| P0 area | Current implementation at the F1 base | Missing integration / acceptance | Reuse | Validation / Owner action |
|---|---|---|---|---|
| Android standalone | `MainActivity` owns encrypted chat history, usage and Tasks Room stores; navigation hosts Chat, Gmail, Calendar and Tasks. INTERNET is permissioned, cleartext is disabled, backups are disabled. | A release-grade, owner-signed Beta APK is not available. | PR #117 and the R7 evidence. | Emulator build, API 26/35 UI and Required. Owner approval is required for production signing and distribution. |
| AI chat | The ordinary ChatViewModel is created with `MockModelChatPort`; this remains offline and labels replies as demos. A cloud adapter, adaptive router, Egress gate and Usage-controlled wrapper exist, but MainActivity does not compose them into normal chat. | Normal chat lacks an end-to-end routed fake-provider connection, and its ViewModel currently dispatches the port directly without a per-send Egress/Usage wrapper. Real AI is not enabled. | PR #96's fake activation/cost test intent; existing `CloudProviderComposition`, `RoutedModelChatPort`, `EgressCheckedModelChatPort`, and `UsageControlledModelChatPort`. | Add UI-level fake E2E and assert Egress denial precedes reservation/dispatch, cancellation settles `OUTCOME_UNKNOWN`, history is encrypted. Provider, age/region eligibility, terms, cost and credentials require Owner decision before activation. |
| Google Calendar | Read-only model/parser, HTTP adapter, `CalendarProviderSessions`, authorization adapter and a full Fake Calendar UI are present. Shell currently constructs `FakeCalendarProvider` directly; `CalendarAgendaScreen` accepts that concrete type. | No app-level interface-based composition root selects the provider/session. Google HTTP is unconnected to UI. No real OAuth has been run. | PR #93's default-Fake, owner-gated session and tests; PR #102's account-bound session/revocation changes after semantic review. | API 26/35 Fake UI E2E for today/tomorrow/range/detail, paging, errors, cancellation, session replacement and account isolation. Real OAuth/Calendar acceptance stays NOT_RUN until Owner configuration and account approval. |
| Gmail | Read-only models, MIME parser, Fake UI/VM, HTTP adapter and Google authorization adapter exist. Shell constructs `FakeGmailProvider` directly. | No provider-session wrapper in the F1 base; UI composition is tied to the Fake at the shell. No Real OAuth/HTTP E2E. | PR #94's bounded read-only HTTP adapter and MIME tests; PR #102's Gmail session/account-binding changes after semantic review. | API 26/35 Fake UI E2E for inbox/search/message/thread/paging, timeout, cancellation, stale result rejection and account switching. Real Gmail acceptance stays NOT_RUN; `gmail.readonly` restricted-scope review and OAuth configuration require Owner action. Send/write/delete/read-state APIs remain out of scope. |
| Local Tasks | `TaskScreen` supports create/edit/complete/reopen/delete with confirmation. `EncryptedTaskRepository` uses Keystore encryption, revision CAS and Room. Date/time validation rejects DST gaps and overlaps. TaskProposal durable apply/receipts are present but not integrated as ordinary Tasks UI. | Need to audit the existing UI, small-screen/accessibility, DST, Room reopen/process-death coverage on this branch and add missing cross-screen acceptance only. | PR #95's transactional approved-proposal apply is semantically present at the F1 base; do not copy it blindly. | Unit + instrumentation for CRUD, DST, persistence after process recreation, duplicate action and error states. No existing user data is used. Proposal UI remains deferred unless it does not delay P0. |
| Safe Beta signing/update | Isolated/demo package and temporary-key identity CI exist. | Production application ID, Owner signing identity and real upgrade chain are not authorized/available. | PR #117 formal Identity Required evidence and CI test-key update/rejection contract. | Keep temporary keys in CI; production signature and distribution require Owner approval. |
| Privacy & Security | Encrypted chat/task stores; manifest disables backup and cleartext; read-only Gmail/Calendar ports; cloud defaults disabled; exact-destination/Egress gate; Usage ledger and safe uncertain-outcome handling exist. | Validate ordinary Chat uses those controls; ensure Gmail/Calendar data cannot reach Cloud through context, logs or stale UI. Confirm Beta manifest excludes debug/test components and no secrets enter fixtures. | PR #96 fake usage/Egress test intent; PR #102 account-bound sessions and Google-origin egress tests, subject to review. | Security regression suite: prompt injection as untrusted fixture, scope and account isolation, token lease revocation, stale response rejection, zero Google-to-cloud dispatch, reservation ordering, encrypted Room and log redaction. No security threshold changes. |
| Required CI / device acceptance | PR #117 passed API 26/35 Required 105/105, Formal Identity, Process Death, Data Isolation, Sentinel and cross-platform checks; exact-SHA clean-runner Android/Identity rerun passed. | Functional code changes require fresh final-HEAD verification. Physical Pixel 7a acceptance remains NOT_RUN and is explicitly outside this emulator-only phase. | R7 workflows and evidence packet. | Focused → class/integration → final Required at frozen integration HEAD. Owner action is required only for a future physical device, production signing, OAuth/AI activation, distribution or merge. |

## PR and dependency audit

| PR | Base / head at audit | Reusable work | Overlap / disposition |
|---|---|---|---|
| #93 | `e4dc8ba` / `1cbfad4` | Calendar session opens Fake by default, only selects Google HTTP when explicitly owner-enabled and Play Services-backed; session close/replacement cancellation tests. | `CalendarProviderSessions.kt` already exists in #117; compare semantics and selectively adopt account-bound/cancellation improvements from #102. Do not copy a duplicate session implementation. |
| #94 | `e4dc8ba` / `109aacb` | Read-only Gmail HTTP adapter, scoped authorization, parser, bounds and MockWebServer tests. | Adapter/auth adapter already exist at #117's base. Reuse as-is, add the missing provider-session and composition layer; no Gmail write methods. |
| #95 | `e4dc8ba` / `7373141` | Durable explicitly-approved TaskProposal transaction, revision CAS and receipts. | The production-domain changes appear semantically present in #117's base. Do not cherry-pick duplicate DAO/repository changes. Preserve the rule that suggestions alone never write. |
| #96 | `e4dc8ba` / `227dc15` | Fake AI activation and usage/Egress validation intent. | PR changes a Room test and removes two dispatch/accounting assertions. Those removals are not adopted; inspect the current Required test and retain strong assertions. No provider implementation is unique to this PR. |
| #102 | `00e0453` / `dd1e17b` | Gmail ProviderSessions, authorization session IDs, selected-account binding, invalidation/cancellation and Google-origin Egress regression coverage. | Targets a different stack; it also changes Tasks-adjacent/R3 UI and harness files. Port only provider/session/auth/tests needed for F2 after exact diff review. Do not merge or cherry-pick the PR. |
| #117 | `ad868af` / `151ea9d` | Verified standalone Android foundation, real IME UI tests, Calendar reliability, Required Identity and gates. | Frozen green base. No writes to PR #117. |

All six PRs were open Drafts at the audit. Current remote heads and bases were re-fetched on 2026-10-26. PR #93–#96 and #102 are not ancestors of #117. No PR branch or protected branch was changed.

## Provider composition design

```text
MainActivity (composition root)
  ├─ Fake-default ModelChatPort
  │    └─ RoutedModelChatPort → EgressCheckedModelChatPort → UsageControlledModelChatPort
  │         └─ test-only fake dispatch / owner-gated real adapter
  ├─ GmailProviderSessions.openDefault() → GmailReadPort + GmailAuthorizationPort
  │    └─ optional GoogleReadOnly session requires owner flag + Play Services + pinned account session
  ├─ CalendarProviderSessions.openDefault() → CalendarReadPort + CalendarAuthorizationPort
  │    └─ optional GoogleReadOnly session requires owner flag + Play Services + pinned account session
  └─ EncryptedTaskRepository → TaskViewModel → TaskScreen
```

Fake is the default. Production Real selection must fail closed unless an explicit owner-controlled activation flag, real Play Services authorization, correct read-only scopes and the account-bound session all agree. Replacing/revoking an authorization session cancels in-flight requests and prevents stale results from entering the new account's UI. Google content is PERSONAL/PRIVATE and is never automatically passed into AI context. No current PR provides permission to activate real OAuth or a real AI endpoint.

## Trust boundaries and data flow

- UI input → ViewModel → domain port. UI does not construct HTTP clients, OAuth adapters, or credentials.
- Google authorization returns a short-lived, scope-bound token lease to a read-only transport. The transport has bounded responses/pages, uses HTTPS only and exposes no mutation API.
- Provider session is bound to an authorization/account generation. Revocation, account switch or session replacement cancels reads and rejects late responses.
- Google-origin message/event data stays in its provider screen and encrypted on-device only if persistence is later explicitly designed. It does not flow into Chat or Cloud AI by default.
- Chat request → explicit local classification/allowlist decision → Usage reservation → fake provider. A future real adapter is unreachable unless owner policy and exact Egress approval are present. `PRIVATE`, `SECRET`, `CREDENTIAL`, `UNKNOWN`, Gmail and Calendar origins fail before reservation and network dispatch.
- Tasks UI mutations → ViewModel → encrypted Room repository. Proposal data remains untrusted and cannot mutate without explicit confirmation, target/revision validation and durable receipt/CAS.

## Provider eligibility precheck (2026-10-26)

This is a terms-level precheck, not activation approval or a final provider decision.

| Candidate | Eligibility/terms signal | Data/cost notes | Status |
|---|---|---|---|
| Google Gemini API | Current Additional Terms say the API client must not be directed to, or likely accessed by, under-18 users. This may conflict with an unspecified Personal Beta audience. | Google distinguishes free/unpaid and paid data-use terms by region/account configuration; current pricing and region must be rechecked before use. | Exclude until target audience, region and Google terms are confirmed. No API call made. |
| OpenAI API | Official docs provide an under-18 developer safety guide; service-specific eligibility, regional availability and required safeguards still need Owner confirmation. | API data controls describe non-training by default but include abuse-monitoring/application-state retention; endpoint and current pricing/model/limits must be selected deliberately. | Candidate only; no credential or request. |
| Anthropic API | Commercial/API access is governed by Commercial Terms; API retention is described as up to 30 days by default with exceptions; contract and user eligibility need confirmation. | No provider cost/model choice made; retention and regional availability require Owner/security review. | Candidate only; no credential or request. |
| Ollama Cloud (existing adapter candidate) | Existing product UI says 18+; current official terms/pricing/privacy should be reconciled with the repository's older adapter assumptions before use. | Official pricing is dynamic and provider/model dependent; local estimated-cost controls do not cap the provider invoice. | Do not activate; requires age, region, terms, model/pricing and data-retention review. |

Official precheck sources: [Gemini API Additional Terms](https://ai.google.dev/gemini-api/terms), [OpenAI API under-18 guidance](https://developers.openai.com/api/docs/guides/safety-checks/under-18-api-guidance), [OpenAI API data controls](https://developers.openai.com/api/docs/guides/your-data), [Anthropic commercial/API retention guidance](https://privacy.anthropic.com/en/articles/7996866-how-long-do-you-store-my-organization-s-data), [Ollama pricing](https://ollama.com/pricing). Recheck before any future provider activation.

## Ownership and validation plan

| Lane | Owner in this campaign | Files / boundaries | Validation |
|---|---|---|---|
| A — Integration / Android | Current integration agent; source of truth | MainActivity/SereaAppShell composition root, provider interfaces, navigation and Android UI tests. Owns the functional integration branch. | Unit first, then Fake UI E2E on API 26/35; preserve Required 105 identities and thresholds. |
| B — Security / Release | Same agent sequentially (no independent subagent available) | OAuth/account session boundaries, Cloud Egress/Usage, Gradle/manifest, signing/update contract and evidence. | Security and provider-session focused suites; no real keys/accounts; run exact-head CI only after stable. |
| C — Functional | Same agent sequentially | Gmail, Calendar, ordinary Chat, Tasks state/error/retry UX and fixtures. Provider UI depends on Lane A interfaces. | Fake provider E2E, accessibility/compact layouts, process persistence and session cancellation tests. |

## Checkpoint

- ACTIVE_PHASE: F2 Google Calendar/Gmail Fake-first UI integration; F1 inventory is complete.
- GOAL_STATUS: PARTIAL; first integration implementation is ready for focused CI, but no test result is PASS yet.
- BASE_SHA: `151ea9d66d32de01b195f4262beb67516823c281` (PR #117 verified candidate).
- BASE_TREE: `ad5d67d5b30e869d86af95c863a5c804cb335e1f`.
- CURRENT_HEAD: update after the F2 provider-session implementation commit; working tree `/workspace/Serea-functional`.
- LAST_GOOD_HEAD: `151ea9d66d32de01b195f4262beb67516823c281`.
- OWNED_BRANCHES: `codex/serea-personal-beta-functional-integration`.
- WORKTREES: `/workspace/Serea-functional` (owned); PR #117 worktree remains untouched at `/workspace/Serea-r6-clean`.
- MERGE_DEPENDENCIES: none merged; PR #117 base only. #93/#94/#95/#96/#102 remain independent Drafts.
- ACTIVE_WORK: reuse the account-bound session changes from PR #102 selectively; add Fake-default Activity composition; generalize Calendar/Gmail screens to session ports and require authorization/source agreement; run Fake app-shell E2E on API 26/35.
- IMPLEMENTED: `AppProviderComposition` opens fake Gmail/Calendar sessions by default; `MainActivity` owns and closes them; `CalendarAgendaScreen` accepts read and authorization interfaces; Gmail/Calendar controllers accept only provider data whose source agrees with the active authorization mode; PR #102 account-session, stale-result, and cancellation changes are selected without its Calendar UI/test-semantics removals. Existing app-shell test methods now assert that the injected sessions remain Fake. Added a focused, non-Required workflow that runs selected session/UI unit tests and API 26 then API 35 Gmail/Calendar shell tests with evidence artifacts.
- P0_COMPLETION_MATRIX: Android foundation PASS; Calendar/Gmail Fake app-shell routes are wired through injected sessions (Focused API 26/35 NOT_RUN); Real adapters are available behind a disabled Owner gate and no app path selects them; Chat still uses demo Mock only; Tasks encrypted CRUD exists / integration coverage audit pending; production signing NOT_RUN; Required CI green only on base SHA; real services NOT_RUN.
- CURRENT_BLOCKERS: the local focused Gradle invocation cannot reach `dl.google.com` for locked AndroidX artifacts; the new non-Required GitHub workflow is the next execution path. Real Google/AI activation needs Owner configuration/eligibility approval; the physical device is unavailable by explicit constraint and remains NOT_RUN.
- SECURITY_GATES: no real OAuth/account/API key, cloud dispatch, production signing or real data; Fake default; HTTPS/read-only contracts; Required criteria frozen.
- CI_RUN_IDS: R7 base Android `40224223408`; Full `40224223407`; Native `40224223397`; Fast `40224223448`; SQLite `40224223405`; RustSec `40224223396`; Git Hooks `40224223428`; Android Contract `40224223430`.
- ARTIFACT_IDS: API26 `12611994574`; API35 `12613473302`; emulator evidence `12613383376`; identity `12614365409`.
- FOCUSED_TEST_STATUS: local `:app:testDebugUnitTest` first failed before compilation because `/usr/lib/jvm/java-21-openjdk-amd64` lacks `javac`; retry with `/workspace/jdk21` reached dependency resolution but could not access Google Maven (`Network is unreachable`). No test result is PASS. Added Gmail UI source-binding and Calendar controller source-binding unit tests; unexecuted.
- FOCUSED_WORKFLOW: `.github/workflows/android-functional-focused.yml`, explicitly Non-Required; ordered provider unit tests → API 26 shell Fake Gmail/Calendar → API 35 shell Fake Gmail/Calendar. It records JUnit, logcat, IME state and environment per API; no real Test IME or OAuth is selected.
- NEXT_ACTIONS: commit and push F2 implementation so the focused workflow executes; repair any observed build/test failure; then complete Fake authorization/session-replacement E2E coverage. Once F2 Fake behavior is verified, move to F3 ordinary Chat Routed/Egress/Usage/Fake integration; then F4 Tasks audit and F5 final-head CI/security evidence.
- OWNER_ACTION_REQUIRED: eventual real OAuth/account; real AI provider/age-region/pricing/terms; production signing/distribution; physical Pixel 7a; main merge and final GO. None blocks Fake/offline development.
