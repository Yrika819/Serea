# Serea Personal Beta Functional Capability Matrix

Status date: 2026-10-26

This is the F1 inventory for the functional integration branch. The source of truth for prior Android Required closure remains PR #117 at `151ea9d66d32de01b195f4262beb67516823c281`; this branch starts from that exact head. PR #117 and `main` are frozen.

## P0 capability matrix

| P0 area | Current implementation at the F1 base | Missing integration / acceptance | Reuse | Validation / Owner action |
|---|---|---|---|---|
| Android standalone | `MainActivity` owns encrypted chat history, usage and Tasks Room stores; navigation hosts Chat, Gmail, Calendar and Tasks. INTERNET is permissioned, cleartext is disabled, backups are disabled. | A release-grade, owner-signed Beta APK is not available. | PR #117 and the R7 evidence. | Emulator build, API 26/35 UI and Required. Owner approval is required for production signing and distribution. |
| AI chat | Ordinary Chat now composes `RoutedModelChatPort → EgressCheckedModelChatPort → UsageControlledModelChatPort → LocalDemoModelChatPort`. `MainActivity` uses a fictional `TEST_FIXTURE` catalog and deterministic `MockModelChatPort`; Cloud is unreachable from this composition. | The composition and new LOCAL_DEMO target are committed but focused tests are NOT_RUN. Need prove normal UI path, multi-turn encrypted history, deny-before-reservation, cancellation/unknown outcome and no Cloud dispatch. Real AI is not enabled. | PR #96's fake activation/cost test intent; existing Router, Egress, Usage and Cloud adapter contracts. | Run focused Chat/Egress/Usage unit tests and API 26/35 app-shell checks. Provider eligibility, age/region, terms, cost and credentials require Owner review before any real activation. |
| Google Calendar | Read-only model/parser, HTTP adapter, account-bound `CalendarProviderSessions`, authorization adapter and Fake Calendar UI are composed through `AppProviderComposition`; the shell consumes port interfaces and source-matched results. | New app-shell/session changes are not yet verified by a successful focused run. Google HTTP is selectable only behind a disabled Owner gate; no real OAuth has run. | PR #93's default-Fake, owner-gated session and tests; selected PR #102 account-bound session/revocation changes. | API 26/35 Fake UI E2E for today/tomorrow/range/detail, paging, errors, cancellation, replacement and account isolation. Real OAuth/Calendar remains NOT_RUN until Owner configuration and account approval. |
| Gmail | Read-only models/parser/HTTP adapter and a new account-bound `GmailProviderSessions` are composed through `AppProviderComposition`; UI uses read/authorization interfaces and rejects source mismatch. | The initial focused run exposed a stale test expectation for AUTHORIZED plus Fake-source data; the expectation was corrected, but rerun is pending. Google HTTP remains behind a disabled Owner gate and OAuth has not run. | PR #94's bounded read-only HTTP adapter and MIME tests; selected PR #102 account-session/revocation changes. | API 26/35 Fake UI E2E for inbox/search/message/thread/paging, timeout, cancellation, stale result rejection and switching. Real Gmail remains NOT_RUN; restricted `gmail.readonly` review and OAuth configuration require Owner action. No write APIs. |
| Local Tasks | `TaskScreen` supports create/edit/complete/reopen/delete with confirmation. `EncryptedTaskRepository` uses Keystore encryption, revision CAS and Room. Date/time validation rejects DST gaps and overlaps. TaskProposal durable apply/receipts are present but not integrated as ordinary Tasks UI. | F4 audit found the P0 acceptance paths already present: CRUD/error states, accessible action labels, editor scrolling, DST validation, encrypted close/reopen persistence, CAS races and activity recreation. No Tasks product-code change is currently justified. Run the existing focused acceptance on this functional head before closing F4. | PR #95's transactional approved-proposal apply is semantically present at the F1 base; do not copy it blindly. | The non-Required focused workflow now selects the existing Tasks UI, date/time, Room and activity-recreation tests on API 26/35. No existing user data is used. Proposal UI remains deferred unless it does not delay P0. |
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
- Chat request → typed target-specific Egress decision → Usage reservation → selected adapter. `LOCAL_DEMO` is exact-bound to the bundled deterministic adapter with no network capability; it allows USER_AUTHORED `UNKNOWN`/`PERSONAL` but rejects `PRIVATE`, `SECRET`, `CREDENTIAL` and all Google origins before reservation. External `CLOUD` remains exact-destination, explicit-intent, PUBLIC-only; its adapter rejects local-target payloads.
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
| B — Security / Release | Agent `/root/security_release`; independent worktree/branch; PR #120 is the policy-correct replacement for unchanged PR #118. | Supply chain, SBOM, dependency verification, mock signing/update, release-security evidence. No Android UI/Composition Root edits. | Focused non-Required run `40278395856` passed. PR #120 Android run `40278399056` remains pending; no production keys, secrets or distribution. |
| C — Adversarial / Reliability | Agent `/root/adversarial_reliability`; independent worktree/branch; PR #119 is preserved as-is; replacement PR #121 is based on Lane A head `7d1ddfd`. | OAuth/account confusion/revocation, hostile provider data, Egress rejection, replay/cancel/process recovery. New test files only; no Android UI/Composition Root edits. | PR #121 tests Google-origin Cloud-egress rejection and `DemoChatComposition` Egress/Usage local Fake routing. Local Gradle test is NOT_RUN before compilation because artifacts were not cached offline; hosted checks are queued. |

## Parallel coordination addendum (2026-10-26)

Lane A remains the only source of truth for Android product UI and Composition Root. Lane B and Lane C use separate worktrees and branches. Neither agent edits Lane A's `MainActivity.kt`, `SereaAppShell.kt`, or provider composition files; Lane A does not edit their new tests or verification scripts. Agent PRs are reviewed by file ownership, contract compatibility and evidence before any selective transfer. No PR is merged automatically.

The first focused run on the F2 head failed only in `GmailViewModelTest.authorizationBoundariesCoverDeniedMissingScopeExpiredRevokedAndSourceMismatch`: the old test expected no provider read for an AUTHORIZED state even though the new implementation correctly performed a read and rejected its Fake-source payload as `INVALID_DATA`. That test was corrected without removing its assertion; a separate source-mismatch test now asserts that the authorized request was attempted and that mismatched data was rejected. API 26/35 shell jobs had been skipped behind the failed unit job.

Agent B's PR #118 is preserved unchanged; its Fast/Full checks failed at the repository commit-identity guard. The policy-correct independent PR #120 uses noreply commit identities. Its focused release-security checks passed in `40278395856`; its Android run `40278399056` is pending. Agent C's PR #119 remains unchanged after its commit-identity check failed. Replacement PR #121 (`8853c2a`) contains only independently owned adversarial tests, uses the required noreply identity, and is based on Lane A `7d1ddfd`; focused dependency resolution was NOT_RUN offline, and its hosted checks are queued.

### F4 Tasks audit (2026-10-26)

The existing Tasks implementation already covers CRUD, completion/reopen, delete confirmation, validation and accessible labels. Timed due dates require an IANA zone and reject DST gaps and overlaps. Room tests verify ciphertext-only storage, Keystore key separation, tamper/swap rejection, concurrent revision CAS, no stale resurrection, and persistence after database close/reopen. The Activity shell test preserves an unsaved synthetic draft across Activity recreation. These findings do not support speculative Tasks product changes. The focused diagnostic workflow was expanded to run the existing Tasks, Chat local Demo, Gmail and Calendar tests serially on API 26 and 35. This remains non-Required and does not alter the formal 105-test set.

## Checkpoint

- ACTIVE_PHASE: F2/F3 focused verification and F4 Tasks closure.
- GOAL_STATUS: PARTIAL; F2/F3 wiring and the F4 audit are committed; hosted functional verification is pending. No functional-branch focused PASS has been recorded yet.
- BASE_SHA: `151ea9d66d32de01b195f4262beb67516823c281` (PR #117 verified candidate).
- BASE_TREE: `ad5d67d5b30e869d86af95c863a5c804cb335e1f`.
- AUTHORITY_SHA: `151ea9d66d32de01b195f4262beb67516823c281` (frozen R7 green baseline; latest GitHub branch tip must still be re-fetched on resume).
- CURRENT_HEAD: `c6343bfde7380b2d8b5d9b53e53d846f5514063b` at the latest verified push (this checkpoint edit is metadata-only and may be a child; re-fetch the exact tip from GitHub on resume).
- CURRENT_TREE: `fe1eefb0d00faad428616ac58db8b0e17524a134` at the latest verified push.
- REMOTE_BRANCH_HEAD: `c6343bfde7380b2d8b5d9b53e53d846f5514063b` verified with GitHub API; focused run `40279685870` is queued on this SHA.
- LAST_GOOD_HEAD: `151ea9d66d32de01b195f4262beb67516823c281`.
- LAST_GOOD_SHA: `151ea9d66d32de01b195f4262beb67516823c281`.
- WORKTREES: `/workspace/Serea-functional` (Lane A); `/workspace/Serea-security-release-verified` (Lane B); `/workspace/Serea-functional-adversarial` (Lane C); frozen PR #117 worktree untouched.
- WORKTREE: `/workspace/Serea-functional`.
- OWNED_BRANCHES: Lane A `codex/serea-personal-beta-functional-integration`; Lane B `codex/serea-functional-security-release-verified`; Lane C `codex/serea-adversarial-reliability-v2` (PR #121); original B/C PR branches remain unchanged.
- OWNED_BRANCH: `codex/serea-personal-beta-functional-integration` (Lane A; only this branch is writable by Lane A).
- MERGE_DEPENDENCIES: none integrated from #118/#119/#120/#121. PR #117 remains the only base. #93/#94/#95/#96/#102 remain independently audited Drafts.
- ACTIVE_WORK: run focused provider/Chat/Egress/Usage unit tests, then serial API 26/35 Fake Gmail/Calendar + Chat local Demo + Tasks acceptance; inspect B/C CI evidence; resolve observed failures; proceed to F5 integration and final-head Required/security evidence.
- IMPLEMENTED: F2 commits `d74998f`/`ef75624` add Fake-default Gmail/Calendar sessions, UI source binding and the source-mismatch expectation correction. F3 commit `2f46145` routes ordinary Chat through exact local-only target, Egress, Usage and deterministic Fake; Cloud adapters reject LOCAL_DEMO. F4 audit found existing CRUD, accessible UI, DST and encrypted Room coverage; no speculative product changes. The focused workflow is expanded to include those existing Chat and Tasks tests.
- P0_COMPLETION_MATRIX: Android foundation and Required gate PASS on PR #117 base only; F2 Gmail/Calendar Fake composition implemented, focused rerun pending; F3 local Demo composition implemented, Egress/Usage adversarial tests are isolated in PR #121 pending CI; real AI/OAuth/Gmail/Calendar NOT_RUN; Tasks implementation/audit complete but API 26/35 focused confirmation pending; production signing/distribution NOT_RUN; Pixel 7a NOT_RUN.
- CURRENT_BLOCKERS: local Gradle offline attempt stopped before compilation because locked Compose 1.7.8 artifacts are absent from cache; earlier online attempts could not reach Google Maven. Hosted GitHub Actions focused workflow is the viable verification path. B/C hosted runs are pending/queued. No Owner-only operation blocks Fake/offline work.
- SECURITY_GATES: Cloud default disabled; normal Chat catalog is fictional `TEST_FIXTURE` only; `LOCAL_DEMO` allows no HTTP/credentials and is isolated from external Cloud target; Google-origin, PRIVATE, SECRET and CREDENTIAL data are rejected before Usage reservation; no real OAuth/account/API key, production signing or real data; Required criteria frozen.
- CI_RUN_IDS: R7 base Android `40224223408`; Full `40224223407`; Native `40224223397`; Fast `40224223448`; SQLite `40224223405`; RustSec `40224223396`; Git Hooks `40224223428`; Android Contract `40224223430`; F2 focused `40277812755` failed one stale Gmail assertion (superseded by correction); Lane A superseded focused `40279158982` cancelled after scoped run pushed; Lane A expanded focused `40279685870` queued on `c6343bf`; push-only Android dispatcher `40279684585` has no jobs and is not a formal execution result. Agent B #120 focused `40278395856` PASS (artifact `12631821182`) and Android `40278399056` pending; Agent C #121 Android `40279276746`, Full `40279276751`, Fast `40279276749`, Native `40279276791`, SQLite `40279276790`, RustSec `40279276657`, Git Hooks `40279276741` queued/in progress; old #119 Fast `40278276687` failed identity guard; old push-only dispatch `40279262064` has no jobs.
- LATEST_CI_RUN: `40279685870` — queued against `c6343bf`, covering provider/Chat unit tests and serial 25-case API 26→35 Fake-provider, local Chat and Tasks acceptance.
- ARTIFACT_IDS: base API26 `12611994574`; API35 `12613473302`; emulator evidence `12613383376`; identity `12614365409`; F2 failure unit artifact `12630469470` (SHA-256 `24e308b91403f8c2e7139b5844cb42ef0ef07902b79fdc5696133deba2ebaa0f`); Agent B release-focused `12631821182`.
- FOCUSED_TEST_STATUS: prior hosted unit run `40277812755` compiled production/tests, reported 115 tests/1 stale source-mismatch assertion; fixed in `ef75624`. A new local offline run with `/workspace/jdk21` stopped before compilation because locked Compose artifacts were not cached; no local test result. API 26/35 focused rerun pending hosted runner. Lane B focused `40278395856` PASS. Lane C #121 local Gradle NOT_RUN before compilation due dependency availability.
- FOCUSED_WORKFLOW: `.github/workflows/android-functional-focused.yml`, explicitly Non-Required; provider/Chat/Egress/Usage JVM tests → API 26 app-shell Fake Gmail/Calendar, real MainActivity local Demo Chat and Tasks (25 selected instrumentation tests) → same API 35 set. Serial; artifacts capture JUnit, logcat, IME state and environment. No real Test IME, OAuth or external AI is selected.
- NEXT_ACTIONS: inspect run `40279685870` unit result, then serial API 26/35 E2E; repair only observed failures. Review PR #120 Android and PR #121 independent CI/test evidence; selectively import no PR until file/contract/test review. Close F4 only after both API jobs pass; proceed to F5 integration/final-head Required and security checks. Keep all Owner-only operations NOT_RUN.
- REQUIRED_GATES: PR #117's 105/105 API 26/35 Required and formal Identity evidence applies only to `151ea9d`; formal Required must be rerun on the final F5 SHA without changing its manifests, count, thresholds or skip rules.
- OWNER_APPROVAL_REQUIRED: real Google OAuth/account, real AI provider eligibility/contract/cost/credentials, production signing/distribution, physical Pixel 7a, main merge, final Personal Beta GO. All remain NOT_RUN; no independent Fake/offline work is blocked.
- OWNER_ACTION_REQUIRED: eventual real OAuth/account; real AI provider/age-region/pricing/terms; production signing/distribution; physical Pixel 7a; main merge and final GO. None blocks Fake/offline development.
