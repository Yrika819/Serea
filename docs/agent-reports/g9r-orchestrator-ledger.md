# Serea G9R Orchestrator Ledger

**Checkpoint:** 2026-10-29 09:27 UTC
**Campaign:** Android Instrumentation Recovery → Native Runtime Inference → G9 Product Acceptance → G10 Preflight
**Roadmap:** Serea 2026 Personal Beta Roadmap v4
**Repository:** `Yrika819/Serea`
**Current disposition:** G8 PARTIAL; Native Runtime PARTIAL; G9 PARTIAL; G10 PARTIAL; Personal Beta NO-GO.

This ledger is tied to the sources and workflow runs listed below. It records a checkpoint, not a claim that queued jobs have completed. No protected-main merge, OAuth activation, real Google data, Cloud AI credentials/billing, production signing, Pixel 7a operation, or beta distribution occurred.

## Authority and verified source identity

GitHub was queried at campaign resume. Protected `main` is still `e0acb0254820c381dd97828824bef86a1db5176c`, tree `4955555be75c5742796740a3283d1a521e0d4806`.

| PR | State | Head SHA | Tree SHA | Base SHA | Scope |
|---:|---|---|---|---|---|
| 146 | Open, Draft | `d444e7cc04c8f1c4c6d12b1412cfe3a76258af66` | `25e2bb481a0b25bfed89d6ee25f1812316703b8e` | G7 `0560e1e7bcd13f0194eab8caca89d70ea5a2f142` | G8 integration evidence baseline |
| 147 | Open, Draft | `36700f6c99d15db428c6d434c657ac76593df3f9` | `107824f5338ed9ec46d41f6836f5df9c6ad84cf7` | G7 `0560e1e7bcd13f0194eab8caca89d70ea5a2f142` | Native runtime candidate |
| 148 | Open, Draft | `a00a4a224a7b36854b70d883acfd35253cf200d6` | `e8fab02b3abcd62211355f73f3c7d829ff230f75` | G7 | Security/release report |
| 149 | Open, Draft | `3a0f0c1897f2115549e7682d6fcc1783c44604f3` | `6a1f4d382907a7deb7d6dbf8147e6490aa054e81` | PR #146 `d444e7cc04c8f1c4c6d12b1412cfe3a76258af66` | Emulator harness recovery |
| 151 | Open, Draft | `d4a36fb5d3613bca2d60151de0991a1baac72fc9` | `b78a84b60ef5a2d344eabdda85b1154e62be46d7` | PR #149 `3a0f0c1897f2115549e7682d6fcc1783c44604f3` | G9 Gmail body-only handoff candidate |
| 152 | Open, Draft | `a9da3b71fff786fc93d3365249ea8087477b31b2` | `cbf688e4438794437b02bc7bd0fcc9bba975264f` | PR #151 `d4a36fb5d3613bca2d60151de0991a1baac72fc9` | Google/Gmail Android UI test fixes |
| 153 | Open, Draft | `6f4c8c5b4caf8f48619234fee5a9c864cb25766b` | `8a17da2ba3003d8663a3c6734d444682609abe05` | PR #151 `d4a36fb5d3613bca2d60151de0991a1baac72fc9` | API 35 IME Activity recreation test fix |
| 154 | Open, Draft | `90913b4d4e3d834ae96d4f7e842bddfbd2b61ff6` | `3f88b0800971de9474a63c0286959aa94074613a` | PR #151 `d4a36fb5d3613bca2d60151de0991a1baac72fc9` | Prior checkpoint/report |

Dependency graph: `#146 → #149 → #151 → (#152, #153, #154)`. PRs #147 and #148 are independent G7-based lanes. Existing PRs remain unmodified. The new integration branch starts from PR #151; only independently accepted patches may be selected into it.

## Exact-head CI snapshot

All run IDs below were queried by exact `head_sha`; attempts shown are 1 unless stated otherwise. Job states are point-in-time and may change after this checkpoint.

### G8 PR #146

All nine requested workflows are terminal and bind to `d444e7cc04c8f1c4c6d12b1412cfe3a76258af66` / tree `25e2bb481a0b25bfed89d6ee25f1812316703b8e`. Android `40804694025` is the sole failure. Full CI `40804693907`, Native platform validation `40804693934`, SQLite `40804693978`, RustSec `40804693889`, Fast `40804693992`, Git Hooks `40804693919`, Release Security `40804722382`, and Mock Beta Contract `40804722157` succeeded. Android Unit is 405/405 PASS. Storage/Emulator and Required Gate failed before instrumentation: API 26 started 0/105 tests and API 35 started 0/105 tests. These are **not** 105 failed tests; no Required test body ran. Identity was skipped behind the failed emulator gate. Process Death, Data Isolation, and Sentinel passed as separate evidence. See PR #148 for artifact/job digests and harness details.

### Native PR #147

Focused Native run `40843204437` failed on PR #147 exact HEAD, attempt 1. x86_64 and ARM64 JNI APK builds, model fixture identity/hash validation, and API 35 x86_64 KVM/emulator setup passed. Instrumentation failed before tests with `RuntimeException: Failed to instantiate test runner class androidx.test.internal.runner.junit4.AndroidJUnit4ClassRunner`; the cause chain is not in the currently recovered log. Native library load, real inference, two-turn generation, usage/egress, cancellation, release, recovery, and runtime cleanup are NOT_RUN. The root cause remains unknown until full diagnostics or a same-runner no-op discriminator exists. Android run `40843204488` and several auxiliary workflows were queued at last query; those states do not establish a product failure or pass.

### G8N PR #149 and G9 PRs #151–154

| Source | Workflow | Run | Last observed state | Evidence |
|---|---|---:|---|---|
| #149 `3a0f0c1…` | Android | `40810824093` | FAILURE | API26 Required: 105 tests, 0 fail/error/skip (103 Required + 2 Additional). Process Death passed. Google/Gmail Additional UI: 16 tests, 5 failures. API35 ran only a 1-test diagnostic (`chatDraftSurvivesGmailAndCalendarDestinationChanges`), which failed; API35 Required and Google/Gmail UI were NOT_RUN. API35 Data Isolation and actual Process Death passed separately. This is distinct from PR #146's 0/105 API26 and 0/105 API35. |
| #151 `d4a36fb…` | Android | `40842824215` | QUEUED | Change Scope, Build/Lint/Instrumentation Compile, Unit, Integration, Security, Docs and Dependency Audit PASS; Storage/Emulator remained queued. No API JUnit artifact yet. |
| #151 | Full / Fast / RustSec / Git Hooks | `40842824328` / `40842824206` / `40842824232` / `40842824356` | PASS | Exact-head auxiliary CI. Native and SQLite were still incomplete/queued. |
| #152 `a9da3b7…` | Android | `40844297910` | QUEUED | Only Change Scope PASS; remaining Android jobs queued. No emulator or UI JUnit evidence. |
| #153 `6f4c8c5…` | Android | `40844961565` | QUEUED | Change Scope PASS; Docs skipped. Required/UI/IME execution NOT_RUN. |
| #154 `90913b4…` | Android | `40845836065` | QUEUED | Docs-only patch: product Android jobs were skipped. Not product-test evidence. Several non-Android jobs were queued. |

The main-only Personal Beta workflow runs `40845818357` and earlier related runs failed with `jobs=[]`; these are not product validation. No runner inventory was inferred from a 403 response.

PR #149 JUnit artifact binding: API26 artifact `12872143095`, 867,807 bytes, SHA-256 `ebdd9e7a9f5343e9dfacba91abed7ba75b4bf56a338fa35cbe3582160ae4f34e`; API35 diagnostic artifact `12872631908`, 255,398 bytes, SHA-256 `bd32cb1a21e26a257c429bc74bd0158f41c819072af835204f953232dc4f8978`. Both artifact hashes were verified against the Actions digest. API26 failing UI identities: `GoogleAuthorizationDemoUiTest#accountSwitchExpiryAndSharedRevocationInvalidateDemoAuthorization` (ambiguous account text selector); `GoogleAuthorizationDemoUiTest#expiredCalendarReconnectShowsFreshPreviewAndWaitsForConfirm` (ambiguous reconnect text); `GoogleCalendarDemoLoopbackUiTest#consentUiOpensLoopbackAgendaAndAccountSwitchDisconnectsIt` (existing MainActivity Compose content); `GoogleAuthorizationAppIntegrationTest#settingsConsentOpensCalendarSessionAndActivityRecreationReturnsToConsentPanel` (5-second Design review timeout, cause unconfirmed); `GoogleCalendarHandoffRevocationUiTest#confirmedCalendarHandoffCannotSendAfterSyntheticAccountSwitch` (exact `B` assertion against full account label). API35's diagnostic failure is at `SereaAppShellTest#chatDraftSurvivesGmailAndCalendarDestinationChanges`; instrumentation status reported NOT_RUN for its broader suites. Android Storage/Emulator job was `123243058373`.

## Work assignments and boundaries

| Owner | Worktree / branch | Base | Files and mission |
|---|---|---|---|
| Parent | `/workspace/serea-g9r-integration-candidate` / `codex/serea-g9r-integration-candidate` | PR #151 `d4a36fb5d3613bca2d60151de0991a1baac72fc9` | Ledger, review arbitration, selective integration, shared roots, frozen candidate and final CI. |
| Agent A — Native | `/workspace/serea-g9r-native-recovery` / `codex/serea-g9r-native-recovery` | PR #147 `36700f6c99d15db428c6d434c657ac76593df3f9` | Native test runner/root-cause and runtime only; no shared composition or model default switch. |
| Agent B — Android/G9 | `/workspace/serea-g9r-android-ui-recovery` / `codex/serea-g9r-android-ui-recovery` | PR #151 `d4a36fb5d3613bca2d60151de0991a1baac72fc9` | Instrumentation/device-query harness and Android UI test files only; preserve all Required identities and assertions. |
| Agent C — CI/Security | `/workspace/serea-g9r-security-evidence` / `codex/serea-g9r-security-evidence` | PR #151 `d4a36fb5d3613bca2d60151de0991a1baac72fc9` | Read-only review and separate `docs/agent-reports/g9r-ci-security-release.md`; independent evidence and release review. |

Three subagents are active. Each lane reports exact base/head/tree, tests and counts, failures/skips, run/job/artifact IDs and hashes, review status, blockers, next action, and any required Owner decision. The owner decision source includes `docs/plans/2026-10-26-personal-beta-owner-decision-ledger.md` and the prior PR #148 security/release report.

## Environment and scope limits

The managed workspace reports current enforced unrestricted HTTP networking and ready Android SDK/JDK selectors. Local SDK inventory contains platform-tools and command-line tools but no emulator binary or system image; `adb devices -l` returns no device. Java is OpenJDK 21, but `javac` is unavailable. Thus local connected API26/API35 or Native Emulator runtime is NOT_RUN unless the lane provisions a valid toolchain without bypassing policy. GitHub Actions jobs remain the primary emulator route. No model weights beyond the existing bounded SmolLM2 fixture were downloaded. No credentials, personal data, or production keys are used.

## Model selection policy

SmolLM2 JNI is the active baseline task; model comparison must not block it or G8/G9 work. The standard local model remains **UNDECIDED**. Candidate screening covers LFM3-1.7B, Bonsai 2 2B/4B (and 9B only if useful), Qwen3.5 2B, Gemma 4 E2B, and FunctionGemma 270M. Verify official model/revision, format, license and redistribution conditions, runtime/quantization compatibility, storage and memory requirements before considering bounded execution. Keep `ModelChatPort`/Model Router runtime-neutral. Separate vendor benchmark claims from Serea measurements. Do not resident-load multiple models or add FunctionGemma without demonstrated benefit. Final model choice requires Pixel 7a measurements and independent review; no Pixel 7a use is authorized in this campaign.

## Owner boundary and gates

No protected-main merge, PR rebase/force-push/merge/close, real OAuth/account/data, Cloud AI credential or billing use, production key/signing, beta distribution, Pixel 7a physical operation, personal-data deletion, Required Gate reduction, or Personal Beta GO declaration. Cloud AI and Google-origin Cloud egress remain disabled. Read-only scopes, explicit consent, account/session/generation binding, revocation, single-use handoff, content identity, native integrity/offline enforcement, usage/egress checks, and fail-closed cleanup remain required.

Current decision matrix (pending; IDs here are local to this campaign and do not replace IDs in older planning documents):

| Decision | Owner input needed |
|---|---|
| Package / coexistence | Beta package ID and Demo/Beta coexistence/update identity. |
| Signing | Certificate/channel policy and production key custody. |
| Google | Cloud project, OAuth activation, Calendar read-only and Gmail restricted scope eligibility. |
| AI provider | Provider age/region/terms eligibility and cloud-provider choice, if any. |
| Local model | Final model only after Pixel 7a measurements and independent review. |
| Device | Pixel 7a physical acceptance session. |
| Integration | Protected-main merge approval. |
| Release | Personal Beta GO after all P0 acceptance gates. |

| Milestone | Status at this checkpoint | Exit evidence still needed |
|---|---|---|
| Android Instrumentation Recovery | PARTIAL | API26/35 Required 105/105; precise identities; UI and IME evidence; lifecycle/process-death and account isolation. |
| G8 Evidence Closure | PARTIAL | Frozen #146 evidence is complete; Required API tests/Identity not passed. |
| Native Runtime Recovery | PARTIAL | Runner root cause; no-op discriminator; new source-bound runtime JUnit; real two-turn SmolLM2 inference and lifecycle/usage/egress/cleanup. |
| Real Model Inference | NOT_RUN | No Native inference reached. |
| G9 Functional Candidate | PARTIAL | Android Google/Gmail UI acceptance and process/identity safety on a fixed integration HEAD. |
| Model Selection Preflight | PARTIAL | Metadata/licensing/compatibility comparison; performance and Japanese/tool quality remain NOT_RUN. |
| G10 Preflight | PARTIAL | CI-only signing/SBOM, migration/update/rollback, OAuth runbook, Pixel acceptance checklist and final same-HEAD provenance. |
| Personal Beta | BLOCKED / NO-GO | All P0 gates, Owner decisions and physical acceptance are outstanding. |

## Next actions

1. Agent A: recover the complete Native runner exception; add a same-runner no-op test if the cause cannot be proven from current logs; fix only in its fresh branch; run bounded focused checks; stop without claiming inference if emulator is unavailable.
2. Agent B: inspect and reuse PR #149 package-query recovery; classify its exact harness failure; test #152/#153 patches on a fresh candidate; preserve all Required identities and obtain API26/35 JUnit artifacts when runners execute.
3. Agent C: update exact queued/completed statuses without retry loops; independently review each new code head; audit cleanup/ownership/egress/usage, test identity, provenance, license and release packet.
4. Parent: accept only reviewed patches; create a frozen new integration SHA/tree; run formal CI on that same source; record any failure and preserve the failed attempt before a source fix.

**Resume point:** refresh GitHub runs for the exact SHAs above, receive the three lane reports, review failures and runner assignment, and update this ledger with a new timestamp. No evidence from a different HEAD may be reused.
