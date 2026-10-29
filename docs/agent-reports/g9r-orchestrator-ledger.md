# Serea G9R Orchestrator Ledger

**Checkpoint:** 2026-10-29 09:44 UTC
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
| 155 | Open, Draft | `a82ecc8edc19d3bed0b95a4c89a111c68729d434` | `71222ef5b9e7aa31eb8ca12a6b6c8eda1721f9f2` | PR #151 `d4a36fb5d3613bca2d60151de0991a1baac72fc9` | Remote checkpoint is docs-only. Local parent worktree has selected PR #157 patch at `8e14755c4783beb6ee56a6b2e4071b5cb6bf76be` / tree `ccd0e86efaa192e8b67ee6a05f13aaf75aee78ad`, not yet pushed. |
| 156 | Open, Draft | `f7f40629048788f440edea39145afd9e823c4b47` | `5181e7aafbd2919af97b68e8774ed56f0419d655` | PR #151 | CI/security/release report checkpoint |
| 157 | Open, Draft | `e19a592611112ac2d9ea52bf337c4a4507400e59` | `c32444191c43e7f40af941cee1000f4e5ab711f5` | PR #151 | Google/Gmail and IME AndroidTest fixes |
| 158 | Open, Draft | `07f0671e801236a1e110295186c5dcc01888fbae` | `2b826a2e881b6f50da241557d5f813ff9db9d46f` | G7; based on PR #147 `36700f6c99d15db428c6d434c657ac76593df3f9` | Native runner diagnostics; independent review requested a fix |

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
| #151 `d4a36fb…` | Android | `40842824215` | IN_PROGRESS | Change Scope, Build/Lint/Instrumentation Compile, Unit, Integration, Security, Docs and Dependency Audit PASS; Storage/Emulator job `123346917078` is in progress on runner `1000023744`. No API JUnit artifact yet. |
| #151 | Full / Fast / RustSec / Git Hooks / SQLite | `40842824328` / `40842824206` / `40842824232` / `40842824356` / `40842824221` | PASS | Exact-head auxiliary CI. Native platform matrix `40842824208` remains in progress; it is not JNI runtime evidence. |
| #152 `a9da3b7…` | Android | `40844297910` | QUEUED | Change Scope, Security and Unit PASS; Dependency Audit, Docs, Build/Lint/Instrumentation Compile and Integration remain queued. No emulator/UI JUnit evidence. |
| #153 `6f4c8c5…` | Android | `40844961565` | QUEUED | Change Scope PASS; Docs skipped. Required/UI/IME execution NOT_RUN. |
| #154 `90913b4…` | Android | `40845836065` | COMPLETED SUCCESS, PRODUCT TESTS SKIPPED | Change Scope, Docs and downstream Required Gate job succeeded. Unit, Integration, Dependency Audit, Security, Build/Lint/Instrumentation Compile, Identity, Storage/Emulator and Pixel bundle were skipped under docs-only scope. No Android product JUnit evidence. |
| #157 `e19a592…` | Android / Full / Native / SQLite / RustSec / Fast / Git Hooks | `40850921385` / `40850921503` / `40850921304` / `40850921299` / `40850921203` / `40850921367` / `40850921057` | QUEUED | Exact-head jobs had no assigned runner in the 09:44 query; Android Change Scope remained queued. No JUnit/artifact. |
| #158 `07f0671…` | G8 Local Native Runtime Focused | `40851453193` | QUEUED | One API35 x86_64 emulator job, no runner assignment/artifact. Native inference NOT_RUN. |

The main-only Personal Beta workflow runs `40845818357` and earlier related runs failed with `jobs=[]`; these are not product validation. No runner inventory was inferred from a 403 response.

PR #149 JUnit artifact binding: API26 artifact `12872143095`, 867,807 bytes, SHA-256 `ebdd9e7a9f5343e9dfacba91abed7ba75b4bf56a338fa35cbe3582160ae4f34e`; API35 diagnostic artifact `12872631908`, 255,398 bytes, SHA-256 `bd32cb1a21e26a257c429bc74bd0158f41c819072af835204f953232dc4f8978`. Both artifact hashes were verified against the Actions digest. API26 failing UI identities: `GoogleAuthorizationDemoUiTest#accountSwitchExpiryAndSharedRevocationInvalidateDemoAuthorization` (ambiguous account text selector); `GoogleAuthorizationDemoUiTest#expiredCalendarReconnectShowsFreshPreviewAndWaitsForConfirm` (ambiguous reconnect text); `GoogleCalendarDemoLoopbackUiTest#consentUiOpensLoopbackAgendaAndAccountSwitchDisconnectsIt` (existing MainActivity Compose content); `GoogleAuthorizationAppIntegrationTest#settingsConsentOpensCalendarSessionAndActivityRecreationReturnsToConsentPanel` (5-second Design review timeout, cause unconfirmed); `GoogleCalendarHandoffRevocationUiTest#confirmedCalendarHandoffCannotSendAfterSyntheticAccountSwitch` (exact `B` assertion against full account label). API35's diagnostic failure is at `SereaAppShellTest#chatDraftSurvivesGmailAndCalendarDestinationChanges`; instrumentation status reported NOT_RUN for its broader suites. Android Storage/Emulator job was `123243058373`.

Lane C and B verified those API26/35 counts against the exact JUnit artifacts; API26 Required and UI are separate stages. PR #152/#153 fresh-branch patch selection is underway. Lane B found `/workspace/.tools/temurin17/bin/javac` and is using that JDK for local instrumentation-source compilation; the earlier PATH-only `javac` check was incomplete.

Parent PR #155's last pushed docs-only head `a82ecc8…` has workflows on previous checkpoint head `f7cde2eec5b7ad6de38ab6d328905ee05a8e33ee`: Android `40850068164` pending, Fast `40850068140` in progress, Full `40850068231` queued, Native `40850068147` queued, SQLite `40850068199` pending, RustSec `40850068210` pending, Git Hooks `40850068257` queued. These do not validate later local code patch `8e14755…`. Main-only Beta run `40850061261` failed and is not PR/product evidence.

## Work assignments and boundaries

| Owner | Worktree / branch | Base | Files and mission |
|---|---|---|---|
| Parent | `/workspace/serea-g9r-integration-candidate` / `codex/serea-g9r-integration-candidate` | PR #151 `d4a36fb5d3613bca2d60151de0991a1baac72fc9` | Ledger, review arbitration, selective integration, shared roots, frozen candidate and final CI. |
| Agent A — Native | `/workspace/serea-g9r-native-recovery` / `codex/serea-g9r-native-recovery` | PR #147 `36700f6c99d15db428c6d434c657ac76593df3f9` | Native test runner/root-cause and runtime only; no shared composition or model default switch. |
| Agent B — Android/G9 | `/workspace/serea-g9r-android-ui-recovery` / `codex/serea-g9r-android-ui-recovery` | PR #151 `d4a36fb5d3613bca2d60151de0991a1baac72fc9` | Instrumentation/device-query harness and Android UI test files only; preserve all Required identities and assertions. |
| Agent C — CI/Security | `/workspace/serea-g9r-security-evidence` / `codex/serea-g9r-security-evidence` | PR #151 `d4a36fb5d3613bca2d60151de0991a1baac72fc9` | Read-only review and separate `docs/agent-reports/g9r-ci-security-release.md`; independent evidence and release review. |

Three subagents were launched in independent worktrees. Agent B has completed its report; Agents A and C continue their review/fix work. Each lane reports exact base/head/tree, tests and counts, failures/skips, run/job/artifact IDs and hashes, review status, blockers, next action, and any required Owner decision. The owner decision source includes `docs/plans/2026-10-26-personal-beta-owner-decision-ledger.md` and the prior PR #148 security/release report.

## Environment and scope limits

The managed workspace reports current enforced unrestricted HTTP networking and ready Android SDK/JDK selectors. Local SDK inventory contains platform-tools and command-line tools but no emulator binary or system image; `/dev/kvm` is absent and `adb devices -l` returns no device. Java 21 is on PATH; a Temurin 17 JDK with `javac` is present at `/workspace/.tools/temurin17` and Agent B is using it for compile checks. Local connected API26/API35 or Native Emulator runtime is NOT_RUN; GitHub Actions is the emulator execution route. No model weights beyond the existing bounded SmolLM2 fixture were downloaded. No credentials, personal data, or production keys are used.

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

**Lane progress at 09:44 UTC:**

- **Agent A — Native:** PR #158 exact source `07f0671e801236a1e110295186c5dcc01888fbae`, tree `2b826a2e881b6f50da241557d5f813ff9db9d46f`; focused run `40851453193` remains queued with no runner. Source checks passed on this head: parser 6/6, preflight/cleanup/JNI contract checks, and identity scanner 103 Required + 2 Additional + 17 non-required = 122. Independent review is **CHANGES_REQUESTED** (`NATIVE-RUNNER-RESULT-001`): parser must reject a non-success `INSTRUMENTATION_CODE` even if both expected tests report PASS, and validate `numtests` if emitted. Agent A is fixing this on a new commit. No JNI inference ran.
- **Agent B — Android/G9:** PR #157 is OPEN/Draft at exact source `e19a592611112ac2d9ea52bf337c4a4507400e59`, tree `c32444191c43e7f40af941cee1000f4e5ab711f5`. Independent source review **ACCEPTED**; scope is five AndroidTest files only, with no Required manifest or assertions removed. On the exact tree, Temurin 17 `:app:compileIsolatedAndroidTestKotlin` BUILD SUCCESSFUL (26 tasks), and `PYTHONPATH=. python3 tests/android_emulator_workflow_tests.py` passed 34/34 (log SHA-256 `6701848a3a17941aed8496f8ad448bbcf5401eaae550183ac9765cb60f45ad04`). Hosted UI/runtime JUnit is NOT_RUN; seven #157 formal runs are queued.
- **Agent C — CI/Security/Release:** PR #156 is OPEN/Draft at `f7f40629048788f440edea39145afd9e823c4b47`, tree `5181e7aafbd2919af97b68e8774ed56f0419d655`. Its exact-source report binds #149 artifacts, accepts #157 source, requests the #158 terminal-result fix, and records current runner states. No product code changes.
- **Parent:** locally cherry-picked only reviewed PR #157 commit `e19a592…` into PR #155's worktree. Exact local source `8e14755c4783beb6ee56a6b2e4071b5cb6bf76be`, tree `ccd0e86efaa192e8b67ee6a05f13aaf75aee78ad`; `:app:compileIsolatedAndroidTestKotlin` BUILD SUCCESSFUL, 26 actionable tasks, after 4m15s. This is compile-only evidence for this exact local head. The candidate is not yet pushed and has no emulator result.

**Resume point:** refresh GitHub runs for the exact SHAs above, receive the three lane reports, review failures and runner assignment, and update this ledger with a new timestamp. No evidence from a different HEAD may be reused.


## Checkpoint refresh — 2026-10-29 10:08 UTC

This section supersedes earlier status rows above for PRs #151 and #155–#158. Prior run results remain bound to their original SHAs and are not reused on later heads. Protected main remains `e0acb0254820c381dd97828824bef86a1db5176c` / tree `4955555be75c5742796740a3283d1a521e0d4806`.

### Current heads and review state

| Item | Source / tree | State at checkpoint |
|---|---|---|
| PR #146 G8 baseline | `d444e7cc04c8f1c4c6d12b1412cfe3a76258af66` / `25e2bb481a0b25bfed89d6ee25f1812316703b8e` | Frozen; Android failure remains, API26/35 Required each 0/105 started. |
| PR #151 G9 base | `d4a36fb5d3613bca2d60151de0991a1baac72fc9` / `b78a84b60ef5a2d344eabdda85b1154e62be46d7` | Android run 40842824215 is in progress on API35 after API26 artifact completion. |
| PR #155 remote | `b53651a6a5ea2ac7990ecdf2a508676e13b4bc21` / `7cfca07f1bc9e306ebaf5cb379db73f255f46c2b` | Prior parent candidate; its CI is for this SHA only. |
| Parent integration worktree | `98950583152d8d522da56c2e49712d0fd2727390` / `4759e544c7e69c05f1dc729b2b2a9ce0b50ec4c1` | Local, not yet pushed at checkpoint; contains only independently ACCEPTED PR #157 AndroidTest changes plus this Ledger. |
| PR #156 report | `83f8e502d9adccdc5d98d46c938da9fa910764d4` / `3a047045accbfe078ff6413cd4ec31adeddba4ff` | C report; G10/model audit refresh in progress. |
| PR #157 Android recovery | `4075ce2de11df3cee05b1b64d2aa6b8899376064` / `6fd63236729cf2ac1ba1e48b3a4cd97681f9577c` | **ACCEPTED for source** by independent Lane C. Runtime/UI JUnit NOT_RUN. |
| PR #158 Native diagnostics | `26645df4cc115ab0872c7b49349575dca761828c` / `4a313464747c897c7b134406a69cc40ee179dacb` | **ACCEPTED for code source** by independent Lane C; focused runner job queued, inference NOT_RUN. A is correcting model-report inaccuracies in a docs-only follow-up; that new source will need a separate review and CI binding. |

PR #157’s latest local and remote candidate changes only `androidTest` files. The integration patch was selected by cherry-picking its body-only assertion commit onto the parent branch; Git conflict resolution selected the full reviewed PR #157 test file. No Production, composition-root, Required manifest, workflow, or gate file changed.

### G8/G9 Android evidence recovered from PR #151

Android run `40842824215`, attempt 1, binds to PR #151 `d4a36fb5d3613bca2d60151de0991a1baac72fc9` / tree `b78a84b60ef5a2d344eabdda85b1154e62be46d7`. Job `123346917078` ran on GitHub Actions runner `1000023744`; Build/Lint/Instrumentation Compile, Unit, Integration, Security, Dependency Audit and Documentation jobs passed.

- API26 Required JUnit: **105 tests (103 Required + 2 Additional), 0 failures, 0 errors, 0 skips**. Separate Calendar 19/19, Chat 16/16, cursor 1/1, Process Death and Data Isolation passed.
- API26 Google/Gmail UI: **16 tests, 6 failures, 0 errors, 0 skips**. Artifact `12888066976`, 906,550 bytes; downloaded ZIP SHA-256 `a45b8121cac63209438120e540b720e7463806eec94749b53c0bd71006c86dae`, matching GitHub's artifact digest.
- Five UI failures are the selector/setContent/timeout failures already addressed in the reviewed PR #157 test patch. The sixth is `GoogleTrustedHandoffShellTest#gmailHandoffReachesChatComposerAndUsesLocalDemoOnlyAfterSend`: the composer includes the message body and the expected safety wrapper, while the old test incorrectly expected the Gmail subject. `GoogleTrustedHandoff.kt` hands only plain/HTML-derived body text to the wrapper; sender, recipients, subject, date, snippet and IDs stay local. PR #157 now asserts the body occurs exactly once in both draft and submitted USER text and rejects every exposed metadata value.
- API35 instrumentation on #151 was still at “Run API 35 Pixel emulator and all instrumentation tests” in progress at this checkpoint; no API35 result or artifact existed yet. This is #151 evidence only.

The #151 debug APK artifact is `12883404140` (33,163,400 bytes), ZIP SHA-256 `fccaf9a97a606ea3cb79b5805e04c026ec146b1fc0fdd0242e7d9239ae66966a`, verified against GitHub. Its debug APK SHA-256 is `36c60a05d57342c7897b3f267d42dddbe93ba2d0b50ec62922be96fbf60d278b`; package `app.serea.android.demo`, versionCode `1`, versionName `0.1.0-demo`, minSdk 26, targetSdk 35. `apksigner verify --verbose --print-certs` passed: one Android Debug signer, cert SHA-256 `4ba419f5487f36bbd6fa34cb6994cb810bc029603d4debfa410476044161698d`. The `app-release-unsigned.apk` in the same archive is unsigned. This does not establish a Beta package decision, production signature, device acceptance or provenance for a later SHA.

### Required Gate identity audit and accepted test change

Agent B’s read-only audit compared #151, #155 `b53651a…`, and #157 `4075ce2…` with the project identity verifier. It found 27 Kotlin test files and 126 test identities partitioned exactly into **103 Required + 2 Additional + 21 non-required**, with no overlap, missing, or unexpected identities. SHA-256 values: Required manifest `283f1aed9c1e1af93a3de24c9f4bf22a5636595dbbb9c1cbada9b70ffc5a0c10`; Additional `81b138db871c74e2dc8e45d2114f53748865082762035a109261b79bc7964c28`; non-required identity file `2467534bccd3e74622fff36f6a428e071915891f22d615db1fd571486cf13ff0`. The API26/API35 harness remains 105 = 103 + 2. Test manifests, runner, verifier, skip/failure rules and gate were unchanged.

On PR #157 exact SHA/tree `4075ce2…` / `6fd632…`, Temurin 17 `:app:compileIsolatedAndroidTestKotlin` passed (26 tasks; 2 executed, 24 up-to-date). `PYTHONPATH=. python3 tests/android_emulator_workflow_tests.py` passed **34/34**; log SHA-256 `995b5c32d219f2d15b37ab937e650410bd9154e724640f9f759dc3b36d8000c5`. Lane C independently ACCEPTED this source. Android run `40854847034` was pending with `jobs=[]`; Full `40854846881`, Native platform `40854846982`, SQLite `40854847149`, RustSec `40854847020`, Fast `40854847203` and Git Hooks `40854846876` were queued/pending. No device JUnit exists for that source.

Parent cherry-picked the accepted test change into local parent HEAD `9895058…` / tree `4759e5…`. On this exact local HEAD, `:app:compileIsolatedAndroidTestKotlin` passed (26 tasks, 2 executed, 24 up-to-date; Gradle log SHA-256 `ddd2538cb8dc7396edf3b4690f995f481e83ae96603f4ee4fbd600b59b650b01`) and the emulator workflow harness passed **34/34** (log SHA-256 `18bd7d076f6d5ab74c7d7c979c5c5476d7019c55bf3d46723514b095794b1585`). These local checks do not replace exact-head hosted API26/35 execution.

### Native runner and model evidence

PR #147 exact run `40843204437` built x86_64 and ARM64 JNI APKs, validated the pinned fixture hash and prepared the API35 x86_64 emulator, then failed before inference during AndroidJUnit4 runner initialization. The existing logs still lack the complete cause chain. No-op/runtime diagnostics are isolated in PR #158; run `40852384302` remained queued without a runner, while `40852384382` remained pending with `jobs=[]`. Root cause is UNCONFIRMED; JNI inference, 2-turn generation, Usage/Egress, cancellation, release and cleanup remain NOT_RUN.

At PR #158 exact code SHA/tree `26645df…` / `4a3134…`, parser tests 8/8 and shell/preflight/cleanup/JNI contract checks passed. Local exact-tree JUnit XMLs report `LocalModelChatAdapterTest` 21/21 and `LocalModelNativeRuntimeTest` 1/1, zero failures/errors/skips, with XML SHA-256s `4e1a4e066edd6ae32ea83aba5f3cdd715a0146e02f2eab95bdd275c8e9738f95` and `0f2f22f02bd97292db25b96896fcbbe5d72df467b2a613818f0ff10379e3dd5c`. The original Gradle log (`072052c…`) records BUILD SUCCESSFUL and AndroidTest compile completion, though the outer unified session reported exit 1; a direct rerun on the same SHA/tree returned exit 0, BUILD SUCCESSFUL in 28s, 60 tasks UP-TO-DATE (direct log SHA-256 `a807f7c79927add41718306cf77d8048e0917a858664349eb3296ce12e3f7b39`). This resolves Gradle task status but does not create Android runtime evidence.

Agent A’s pinned-source model audit confirms all candidates’ Serea runtime, Android runtime, Japanese/tool quality, TTFT, tokens/s, RAM, heat and cancellation performance remain NOT_RUN. The standard model remains UNDECIDED; no weights were downloaded. Key static findings: Serea’s llama.cpp pin lacks LFM3 support; it lacks Bonsai PTQ1_0/PQ2_0 private types (those packings statically incompatible; Q2_0 NOT_RUN); Gemma 4 loader architecture source exists but exact GGUF NOT_RUN; Qwen3.5-2B card snapshot is safetensors-only and current GGUF adapter is format-incompatible; FunctionGemma is gated under Google Gemma terms, has no identified GGUF and its LiteRT example is for task-specific fine-tuning. The old evaluation file’s “Gemma 4 35+ languages” is wrong (official card says >140); LFM exact US$10M license boundary is inconsistent between official page and pinned license. A docs-only correction on #158 is in progress and will be reviewed independently.

### Current formal CI and next actions

The old PR #155 CI belongs to SHA `b53651a…`: Android `40852318331` pending with Change Scope job `123376387617` queued and no runner; RustSec `40852318286` PASS; Full `40852318252`, SQLite `40852318241`, Fast `40852318237`, Git Hooks `40852318290` queued. Native platform run `40852318251` is Rust matrix only: at last query 5/12 jobs in progress, 1 passed, 6 queued. RustSec artifact `12888565174` (3,186 bytes) reports digest `fafcf34d52542d59607e1059f463b3ff364d7d07932b063724738ac4d3234efc`; artifact content was not available to this client and is not inspected. These runs do not validate parent local HEAD `9895058…`.

Next: finish Agent A’s documentation correction and Lane C’s independent review; finish the in-progress #151 API35 run if it completes; push parent candidate `9895058…`, update PR #155 description/Ledger to that exact SHA/tree, and monitor its new formal CI without retry loops. Keep #158 Native diagnostics isolated from product Chat. No main merge, OAuth/data access, Cloud credentials, production key, Pixel 7a operation, Beta distribution or GO decision occurred.


## Checkpoint update — 2026-10-29 10:13 UTC

This update supersedes the 10:08 PR #157 and #158 head/review rows above.

- **PR #157**: latest source `4075ce2de11df3cee05b1b64d2aa6b8899376064`, tree `6fd63236729cf2ac1ba1e48b3a4cd97681f9577c`, base #151 `d4a36fb5d3613bca2d60151de0991a1baac72fc9`. Lane C **ACCEPTED** this exact source. It modifies AndroidTest only; it checks the safety-wrapped Gmail body occurs exactly once in composer and submitted USER text, and rejects each exposed Gmail metadata value. Local compile 26 tasks / workflow harness 34/34 are recorded above. Current Android `40854847034` is pending with `jobs=[]`; Full `40854846881`, Native platform `40854846982`, SQLite `40854847149`, Git Hooks `40854846876` queued; RustSec `40854847020` and Fast `40854847203` pending. API26/35 runtime JUnit is NOT_RUN.
- **Parent integration candidate**: cherry-picked only the accepted #157 handoff test change onto the existing accepted #157 selector/IME patch. Exact local source `98950583152d8d522da56c2e49712d0fd2727390`, tree `4759e544c7e69c05f1dc729b2b2a9ce0b50ec4c1`; currently local and not pushed. Exact-head local AndroidTest compile succeeded in 39 seconds (26 tasks, 2 executed / 24 up-to-date), log SHA-256 `ddd2538cb8dc7396edf3b4690f995f481e83ae96603f4ee4fbd600b59b650b01`. Workflow harness passed 34/34, log SHA-256 `18bd7d076f6d5ab74c7d7c979c5c5476d7019c55bf3d46723514b095794b1585`. Lane C has been asked for an independent integration-tree review before Parent pushes; no CI run exists yet for this local head.
- **PR #158**: code review remains ACCEPTED at `26645df4cc115ab0872c7b49349575dca761828c` / tree `4a313464747c897c7b134406a69cc40ee179dacb`, with no emulator inference. Agent A pushed the model-document correction as a docs-only successor at `943fb4e0b409da3d7f342f9d18a60188d40cd262` / tree `2d2d2e60e2cf0ee12d2d1cb75ac796acdf4204b2`; changed file `docs/agent-reports/g8n-native-bonsai-evaluation.md`, SHA-256 `aa56693d53c0741a59d674449df909ddd5f030fc57308c45839369c5f019d095`. The correction includes a complete 2B/4B/9B matrix and official pinned revisions, labels pinned LFM3 and Bonsai PTQ1_0/PQ2_0 static incompatibilities without claiming inference, corrects Gemma 4 to >140 languages and its LiteRT-LM mobile guidance, distinguishes exact artifact support from loader source, and clarifies the LFM $10M license ambiguity. All untested Serea runtime/performance and model-quality rows remain NOT_RUN; the standard model remains UNDECIDED. Lane C is reviewing this exact doc head.
- PR #158 new-head workflows: Android `40855571117` pending, focused Native `40855571172` queued, Full `40855570956`, Native platform `40855570971`, SQLite `40855571214`, RustSec `40855570966`, Fast `40855570942`, Git Hooks `40855571207`, and Android Personal Beta Contract Tests `40855570943` queued at last query. The focused job at `40855571172` has no runtime result; no new inference evidence exists.
- PR #151 Android `40842824215` remains in progress on API35 instrumentation after the API26 artifact and six UI failures documented above. No API35 result had appeared at this update. This run remains bound to #151 and is not transferred to #155/#157.

Next: receive Lane C’s #155 integration-tree and #158 document review, update PR #155 description and Ledger, push this frozen parent tree as a new source, then monitor formal CI for that exact SHA. No main merge, live Google/Cloud access, production signing, Pixel operation, distribution or Personal Beta GO occurred.

## Campaign resume checkpoint — 2026-10-29 11:40 UTC

This section supersedes earlier PR-head and run-state rows above where they conflict. Historical artifacts remain bound to their original source; no result is transferred between SHAs. GitHub PRs #146–#158 were re-queried after resuming. All remain open Drafts. Protected `main` is still `e0acb0254820c381dd97828824bef86a1db5176c`; it was not changed.

### Current exact candidate sources

| PR | Base SHA | Source SHA | Tree SHA | Current disposition |
|---:|---|---|---|---|
| #146 | `0560e1e7bcd13f0194eab8caca89d70ea5a2f142` | `d444e7cc04c8f1c4c6d12b1412cfe3a76258af66` | `25e2bb481a0b25bfed89d6ee25f1812316703b8e` | Frozen G8 baseline; PARTIAL. |
| #147 | `0560e1e7bcd13f0194eab8caca89d70ea5a2f142` | `36700f6c99d15db428c6d434c657ac76593df3f9` | `107824f5338ed9ec46d41f6836f5df9c6ad84cf7` | Original Native candidate; runner initialization failure remains historical. |
| #149 | `d444e7cc04c8f1c4c6d12b1412cfe3a76258af66` | `3a0f0c1897f2115549e7682d6fcc1783c44604f3` | `6a1f4d382907a7deb7d6dbf8147e6490aa054e81` | Emulator harness recovery ancestor. |
| #151 | `3a0f0c1897f2115549e7682d6fcc1783c44604f3` | `d4a36fb5d3613bca2d60151de0991a1baac72fc9` | `b78a84b60ef5a2d344eabdda85b1154e62be46d7` | G9 Gmail body-only handoff base. |
| #155 | `d4a36fb5d3613bca2d60151de0991a1baac72fc9` | `683854ba94cb3a1e1972b73d0f1f60392f2271d6` | `e6228614a6acf7d3ac440c66b7038bc8728f05ee` | Parent integration candidate; exact Android run ended failure below. |
| #156 | `d4a36fb5d3613bca2d60151de0991a1baac72fc9` | `bd281b1781eb1ab822c4266bfd6c54162b1538d2` | `a5da857b2c47f03cb88225c8c9d6b07a3b3ee723` | CI/Security/Release docs; exact Fast and docs checks pass; product tests skipped by docs-only scope. |
| #157 | `d4a36fb5d3613bca2d60151de0991a1baac72fc9` | `4075ce2de11df3cee05b1b64d2aa6b8899376064` | `6fd63236729cf2ac1ba1e48b3a4cd97681f9577c` | AndroidTest patch source ACCEPTED; exact Android run still in progress, no UI JUnit yet. |
| #158 | `0560e1e7bcd13f0194eab8caca89d70ea5a2f142` | `80e9990625b000e9d56a78743a7460ac120d4979` | `a07d653dd7a6a03323f81fa2332ae14a91eaceaf` | Native exact-head run in progress; independent review CHANGES_REQUESTED for cancellation observation. |

PR #152 (`a9da3b71fff786fc93d3365249ea8087477b31b2` / `cbf688e4438794437b02bc7bd0fcc9bba975264f`), #153 (`6f4c8c5b4caf8f48619234fee5a9c864cb25766b` / `8a17da2ba3003d8663a3c6734d444682609abe05`), and #154 (`90913b4d4e3d834ae96d4f7e842bddfbd2b61ff6` / `3f88b0800971de9474a63c0286959aa94074613a`) remain open Drafts based on #151. Existing branches were not rewritten.

### PR #155 exact Android result

Android workflow `40856235701`, attempt 1, is bound to source `683854ba94cb3a1e1972b73d0f1f60392f2271d6` / tree `e6228614a6acf7d3ac440c66b7038bc8728f05ee`. Storage and Emulator job `123412988359` failed; Required Gate job `123422742095` failed from the emulator result. The API 26/API 35 Required manifest remains 103 Required + 2 Additional = 105; no identity, assertion, threshold, or skip rule changed.

- API26 artifact `12892919797`, 796,432 bytes, verified ZIP SHA-256 `60e0c71a02c64016b29f7191052a2a71798191ec7454a323fd698cdfc0b2c08f`.
- API26 Required JUnit: 105/105, 0 failures, 0 errors, 0 skips (103 Required + 2 Additional); XML SHA-256 `3396bfe35cffb24b47bb4af51e21d26b69f20a3f37deceed0cd872ff611bc946`.
- API26 Process Death and Data Isolation independently PASS. Google/Gmail UI is a separate suite: 16 tests, 3 failures, 0 errors, 0 skips; XML SHA-256 `bf6355f69e14f1fb2b9d2714c7ba06001d0cd0f8452e2c9fa81ffe36080167ab`. The three failures are `GoogleCalendarDemoLoopbackUiTest#consentUiOpensLoopbackAgendaAndAccountSwitchDisconnectsIt`, `GoogleAuthorizationAppIntegrationTest#settingsConsentOpensCalendarSessionAndActivityRecreationReturnsToConsentPanel`, and `GoogleCalendarHandoffRevocationUiTest#confirmedCalendarHandoffCannotSendAfterSyntheticAccountSwitch`. Compared with #151's six API26 UI failures, three were fixed and no new failure identity appeared.
- API35 artifact `12893128722`, 278,210 bytes, verified ZIP SHA-256 `7be37491d953f8725e689aeb65ea2caccdbd5d3eec4a7a1843cd5f7de8559109`. The bounded `SereaAppShellTest#chatDraftSurvivesGmailAndCalendarDestinationChanges` diagnostic failed after recreation with Activity RESUMED, decor attached/visible, `windowFocus=false`. Its diagnostic result is 1 failure, 0 errors/skips. API35 Required and Google/Gmail UI suites are **NOT_RUN**, not 105 failed tests. Process Death and Data Isolation harness checks passed independently.
- The same Android run's Build/Lint/Instrumentation Compile, Unit, Integration, Security, Dependency Audit, Documentation, and Change Scope jobs passed. Pixel 7a Reacceptance Bundle was an evidence-bundle CI job only; no physical device operation occurred. Personal Beta Identity Isolation was SKIPPED. Required Gate failure is not a gate contract change.
- Exact-source Full `40856235656`, Fast `40856235640`, RustSec `40856235844`, Git Hooks `40856235773`, and Native platform validation `40856235700` succeeded. Cross-architecture SQLite `40856235774` has three producer/open jobs passed and its final Intel-open job `123405030473` remains queued. Native platform validation is not Android JNI inference.
- Debug candidate artifact `12892208109`; APK package `app.serea.android.demo`, APK SHA-256 `9d0aa158c9d8d56b9574dbaca9f73b326d35359e8868b5a38998f058dfb1e199`; CI debug signer certificate SHA-256 `43f6d84b9241df72cb2434588a64077ea259a26520efb81a09b514f224876b2a`. These are CI debug artifacts, not production signing or device acceptance.

Agent B is correcting only the remaining UI/IME issues on a separate successor branch. API26 loopback, lifecycle, handoff, account/session and authorization checks must remain intact; API35 focus/InputConnection assertions remain required. No changes will be pushed to the tested #155 source while its formal results are being checkpointed.

### PR #158 Native runtime result and review

Focused Native run `40866167732`, attempt 1, is bound to #158 source `80e9990625b000e9d56a78743a7460ac120d4979` / tree `a07d653dd7a6a03323f81fa2332ae14a91eaceaf`, job `123421381275`, and completed FAILURE. The x86_64/ARM64 APK builds, pinned model license/fixture checks (101,016,128 bytes; SHA-256 `e3131339bf4e8065265593d4fd8f7bb7ff2d3abff1edb5618aa1197b89cad9f5`), KVM requirement, and API35 emulator boot completed. The step `run-e2e.sh` then exited 1. Cleanup failed closed because the app-package query returned exit 1 / `QUERY_FAILED`, device-temp model removal failed, and package/model absence could not be verified; host fixture removal was verified. No JUnit or runtime artifact was uploaded, so the test identity, model load/inference stage, and cancellation outcome are **NOT_VERIFIED**. The only artifact is build diagnostic `12893797798`, 6,063 bytes, SHA-256 `24290d738c99e5e9f150716af73adf5b61eee5a667367ff22847dc126dce10b7`. Public workflow logs do not retain the primary failure/JUnit output because the cleanup gate suppressed runtime evidence; no root cause is inferred from the ADB query error alone. Companion Fast CI `40866167926`, beta contract `40866167760`, and RustSec `40866167855` passed; Android `40866167864`, SQLite `40866167796`, Full `40866167965`, Git Hooks `40866172038`, and Native platform `40866167791` remain queued/pending at the latest query.

Local exact code commit `0f9926b9995b06c0651eb2dadc698064676d74bc` / tree `e1a9d625da149c3b3db4649dc26d0014067f8950` passed the isolated Android manifest/AndroidTest compile and focused JVM suite: 22 tests, 0 failures/errors/skips. Gradle log SHA-256 `b646fa2944af7d8061c9385d46c4193ae99e3da4360d75b3ea20e06a06659adf`; `LocalModelChatAdapterTest` XML SHA-256 `2ac66f3c1702ca169e3ceecaf6c5ef624bb913d3f24d591fa9ac06dc35b6044d`; `LocalModelNativeRuntimeTest` XML SHA-256 `3d171942af5980ac065a2f9e02f873531004f155ecfb889401e46c802ce0c4c6`. Static JNI/preflight/cleanup/parser/gate checks passed. These local JVM/compile checks are not Native inference evidence.

The previous exact-source run on `848fe67e4d8dd2de21ffb05793c4945127edcb06` / tree `57ab7cd83ef100cb5122dd39463186234cafc0d0`, run `40860331937`, ended FAILURE after the Native test body started: `TimeoutCancellationException` at the 15-second send bound; no successful model output or Usage settlement. Its bounded runtime artifact was `12891353567`, 24,280 bytes, digest `e4b829dc964697164ffb1a7844018a030b487819e9f745e0ece1d1fb8805acde`; the separate build artifact was `12891333258`, digest `708ab18a0cac841b73785cf8dbe4aa543a2efd62d1c90766ee9f27872317d2d8`. Fixture revision `1076b85d135017971845b19b8d07df1479155988`, size 101,016,128, SHA-256 `e3131339bf4e8065265593d4fd8f7bb7ff2d3abff1edb5618aa1197b89cad9f5`; model weights were not uploaded.

Independent Lane C review of #158 `80e9990…` returned **CHANGES_REQUESTED** (`NATIVE-CANCEL-OBSERVATION-001`): the test counter proves a JNI cancel call occurred during nonterminal progress but does not prove the same active generation observed cancellation and aborted. Agent A is implementing a generation-bound cancellation-observed/terminal signal and bounded join before release, preserving existing timeout, release, double-close/reload, Usage unknown, offline, and failure-seam checks. The tested #158 source is not modified in place. Native source remains not accepted until the successor is re-reviewed and exact-head inference is evidenced.

### Remaining gates and next action

API26 Required on #155 is PASS; API35 Required remains NOT_RUN. Google/Gmail UI acceptance is PARTIAL, Native inference is unverified, and the standard local model remains UNDECIDED. Bonsai/LFM3/Qwen3.5/Gemma/FunctionGemma evidence is static specification/license/compatibility only; Serea runtime quality/performance, heat and Pixel 7a measurements remain NOT_RUN. G10 preflight can continue with release documentation and CI-only rehearsal; Personal Beta remains NO-GO. Next: review exact successor heads from Agents A/B, run focused checks, request fixes if needed, then selectively integrate accepted AndroidTest changes plus this Ledger into a new frozen Draft candidate and run CI against that exact source/tree. Do not reuse #155 or #158 results for successor heads.

## Independent Android artifact review — 2026-10-29 11:46 UTC

Lane C independently inspected both API artifacts from #155 run `40856235701` (attempt 1; source `683854ba94cb3a1e1972b73d0f1f60392f2271d6`, tree `e6228614a6acf7d3ac440c66b7038bc8728f05ee`, Android job `123412988359`, runner `1000024076`). Its counts and hashes match the previous section. Additional details:

- The loopback Calendar case proceeds through consent/session setup, then waits for an agenda event from its local MockWebServer. Request count, session state, and agenda state were not captured at failure; root cause is still unresolved. Cleartext policy/TLS is a candidate to test, not a confirmed cause.
- The Design review/date test's intended zone/query-date diagnostics are inside `catch (AssertionError)`, but the observed Compose timeout exception bypasses that handler. Root cause remains unresolved among date fixture/query/apply behavior.
- The handoff test's `performScrollTo()` failure is confirmed as a Compose action/layout semantics mismatch: the field has no scrollable ancestor. Preserve its stale-account handoff and no-send assertions.
- API35 diagnostic XML `d590f93c101ab34f1c912b0f7cc170d288034db82bb508c02afc7aa1ae1f8c35` confirms the focus-ready failure; logcat reports Activity RESUMED, attached and visible, but `windowFocus=false` through recovery. Product re-acquisition versus ActivityScenario/emulator focus scheduling remains unresolved. API35 Required 105 and Google/Gmail UI remain NOT_RUN.

On the unchanged #157 source `4075ce2de11df3cee05b1b64d2aa6b8899376064` / tree `6fd63236729cf2ac1ba1e48b3a4cd97681f9577c`, Android run `40854847034` remains in progress at API26 Storage/Emulator job `123417035471`; no artifact/JUnit is yet available. Its companion Full `40854846881`, Fast `40854847203`, RustSec `40854847020`, and Git Hooks `40854846876` are PASS; Native platform `40854846982` and SQLite `40854847149` remain queued. No result from #157 is used as #155 evidence.

## Independent model/decision review — 2026-10-29 11:50 UTC

Lane C's independent official-source review **ACCEPTED** the expanded model shortlist for static metadata, format, license, and compatibility claims in Agent A's report at SHA-256 `aa56693d53c0741a59d674449df909ddd5f030fc57308c45839369c5f019d095`. Pinned Hugging Face revision/access/file-tree values, sizes and listed hashes matched; official Bonsai format/Hadamard, Gemma license/mobile guidance, FunctionGemma gate/usage, and LFM license threshold were checked. No model weights were downloaded. Gemma's >140-language statement remains a model-card claim; its details distinguish 35+ out-of-box support from 140+ training languages, not demonstrated uniform quality.

The expanded model section is accepted, but Agent A's full current campaign report is **CHANGES_REQUESTED** as a checkpoint: it still names #147 as its current draft and describes the runner cause as unresolved. The cause established on the subsequent line was a non-void AndroidJUnit4 test method (`InvalidTestClassError`, inferred `Int` from a final `Log.i` expression); the explicit `Unit` fix plus later no-op test resolved that initialization blocker. Agent A is updating the report on its next normal #158 successor commit with exact-head status and the 40866167732 cleanup/ADB failure. This request does not alter the accepted model matrix or trigger model selection.

Lane C also classified the #156 Owner decision packet as **CHANGES_REQUESTED** for decision readiness. The release checklist covers the required topics, but decision IDs conflict between the canonical Owner Decision Ledger and older G8 report; 11/03 omits certificate/signing custody policy, and 11/10 does not put Gmail Restricted scope explicitly in the dated matrix. Lane C is preparing a separate docs-only successor from #156 exact head `bd281b1781eb1ab822c4266bfd6c54162b1538d2` / tree `a5da857b2c47f03cb88225c8c9d6b07a3b3ee723`, with a consolidated canonical O-ID matrix. Requested coverage: 11/03 package/coexistence/certificate policy; 11/10 OAuth/project, Calendar read-only, separate Gmail Restricted gate, provider eligibility/terms/cost/data handling, undecided local-model choice, and Pixel acceptance criteria. Owner-only activation, credentials, signing key, real data, and physical-device steps remain blocked.

## Owner decision packet successor — 2026-10-29

Lane C reconciled the canonical Owner decision IDs and dated checkpoints in PR #159, an open Draft docs-only successor to #156. Source `982c8271698c5ea323e7a4d19f4dd662087db2c5`, tree `11e3c0ca61577bb435e0f69b1fcd24313857ff1f`, base #151 `d4a36fb5d3613bca2d60151de0991a1baac72fc9`. Focused docs validation passed for 205 files; Python suites: workspace smoke 82/82, commit identity 24/24, git preflight 28/28, hook launcher 4/4. Exact PR CI is pending. No owner-only operation occurred.

Agent B's successor remains local/uncommitted at base #155 `683854ba94cb3a1e1972b73d0f1f60392f2271d6`; its JVM tests were in progress when the agent hit the usage limit. It has not been accepted or integrated. Agent A's cancellation/diagnostic changes also remain local/uncommitted at #158 base `80e9990625b000e9d56a78743a7460ac120d4979`; no new Native inference run is authorized until review and focused tests complete.

## Resume checkpoint — 2026-10-29 (latest query)

Parent candidate PR #155 remains open Draft. Its current exact head is `8f6021920d8e7e671a4156298f2619634d64507b`, tree `37122ae04c3ab906ac4c10edd2263f059b3702e1`, base #151 `d4a36fb5d3613bca2d60151de0991a1baac72fc9`. This is a docs-only checkpoint successor to source `683854ba94cb3a1e1972b73d0f1f60392f2271d6`; prior API26/API35 artifact results remain bound only to the prior source and must not be transferred. Exact-head formal CI started as Android run `40881958023` and the latest query showed Change Scope queued, Fast CI in progress, Native platform matrix partly in progress, and Full CI, SQLite, Git Hooks, RustSec and remaining matrix jobs queued. No final result is available yet.

PR #159 remains open Draft at source `982c8271698c5ea323e7a4d19f4dd662087db2c5`, tree `11e3c0ca61577bb435e0f69b1fcd24313857ff1f`. Its docs check passed; the Android Emulator Required Gate was still queued (`40881913899`, job `123475563464`). Build/Lint, Android Unit, Integration, Security, Dependency Audit, Emulator, Pixel bundle and Identity jobs were skipped by docs-only scope; none counts as product acceptance.

The exhausted agents' uncommitted work remains isolated and is not integrated:

- Native branch/worktree `codex/serea-g9r-native-recovery`, based on #158 head `80e9990625b000e9d56a78743a7460ac120d4979`: cancellation outcome is now generation-bound, JNI instrumentation reports observed cancellation/terminal outcome, and the E2E harness records primary failure stage plus bounded cleanup diagnostics. `bash tools/g8-local-model-native/test-cleanup.sh` passed on this exact working tree; `bash -n tools/g8-local-model-native/run-e2e.sh` passed. The Android test/JNI changes have no completed focused compile or exact-head device run. A prior #158 run `40866167732` ended with failed ADB package query/device-temp cleanup; no runtime artifact exists, so model load/inference/cancellation remain NOT_VERIFIED. Do not claim real inference.
- UI branch/worktree `codex/serea-g9r-ui-followup`, based on #155 head `683854ba94cb3a1e1972b73d0f1f60392f2271d6`: changes are limited to test source. The tests use a test-process-only HTTPS MockWebServer client constrained to exact Calendar GET origin/path/query rules, disable redirects, and add agenda/focus diagnostics; production URL policy and manifest were not changed. Tests are uncommitted and not independently reviewed/accepted. Local Gradle could not start because configured `JAVA_HOME=/usr/lib/jvm/java-17-openjdk-amd64` is absent. Using available Java 21 JRE also failed toolchain detection because `javac` is unavailable. Thus JVM tests and AndroidTest compilation are BLOCKED/NOT_RUN in this environment.

No PR was rewritten, no main merge occurred, and owner-only actions remain untouched. Protected main was not modified. Next: restore a JDK with compiler in the execution environment or use exact-head GitHub Android jobs; finish independent review and focused tests before committing the Native/UI source changes. Preserve the source-specific failures above and rerun formal CI on any newly frozen source.

## Draft recovery successors — 2026-10-29

Two isolated successors are now pushed as Draft PRs. Their exact-head evidence is pending; neither is accepted or integrated.

| PR | Base SHA | Source SHA | Tree SHA | Scope |
|---:|---|---|---|---|
| #160 | #158 source `80e9990625b000e9d56a78743a7460ac120d4979` (base branch G7 readiness) | `447e151738cb4e98c5d97cddf47b96e0d1035656` | `8787bbe744c2b67fc2950f2e58930297b65c8447` | Native cancellation and cleanup diagnostics; product Chat remains disconnected. |
| #161 | #155 source `683854ba94cb3a1e1972b73d0f1f60392f2271d6` (PR base G9 functional candidate) | `0941043a1e687ec3b4e4778ce387c7f2c4b70ba1` | `e64ddd7bafd7c03de3898f2c2503f6ff1aff8604` | Google/Gmail/Calendar UI test-only recovery and test-local HTTPS fixture. |

#160 workflows are queued at exact source for Android `40882489925` (Change Scope job `123477424648`), Native focused `40882490072` (job `123477423229`), Full `40882489846`, Fast `40882490126`, Git Hooks `40882490004`, SQLite `40882489910`, Native platform `40882490092`, RustSec `40882489931`, and Mock Beta Contract `40882489912`. The focused runner has no assigned runner (`runnerName=null`).

#161 workflows are queued at exact source for Android `40882542674` (Change Scope job `123477601597`), Full `40882542835`, Fast `40882542844`, Git Hooks `40882542975`, SQLite `40882542777`, Native platform `40882542765`, and RustSec `40882542951`. No exact-head Android UI runtime result is available.

The UI diff touches tests and adds test-only helpers/report; it does not modify production files, manifests, Required identities, or the Required Gate. The Native diff touches the JNI implementation only for diagnostic test seams and cancellation witnesses, plus its test/harness; its Android/JNI compile is still NOT_RUN locally. `git diff --check` passed before both commits; the cleanup harness and shell syntax checks pass on #160. JVM tests and UI instrumentation compile remain blocked locally by missing JDK compiler. Both successors require independent review and exact-head CI before integration.

Parent PR #155 current checkpoint source is `35d35542680d4438c7f6970176906b719cf1be79`, tree `227cfea352102e190de6fd4c5583a281628972b8`. Its current non-Android workflows are still partly queued/in progress (Full `40882305754`, Fast `40882305710`, Native platform `40882305768`, SQLite `40882305751`, RustSec `40882305759`, Git Hooks `40882305803`). No PR #155 CI result is transferred to #160/#161. Main remains `e0acb0254820c381dd97828824bef86a1db5176c`; Personal Beta remains NO-GO.

## Queue and docs-gate update — 2026-10-29

At exact PR #159 source `982c8271698c5ea323e7a4d19f4dd662087db2c5` / tree `11e3c0ca61577bb435e0f69b1fcd24313857ff1f`, Android run `40881913899` completed SUCCESS. The Android Documentation job and `Android Emulator Required Gate` contract job (`123475563464`) passed. The gate's required-check execution verified exact source/scope; all product Build/Lint, Unit, Integration, Security, Dependency Audit, Emulator/Storage, Pixel bundle and Identity jobs were SKIPPED as documentation-only. Therefore this is a PASS for docs and scope/gate contract only, with Android product tests NOT_RUN.

Latest runner query: #160 Native focused run `40882490072` remains QUEUED with `runnerName=null`; #161 Android Change Scope run `40882542674` remains QUEUED with `runnerName=null`; PR #155 Fast run `40882735878` remains QUEUED with `runnerName=null`. Other #155/#160/#161 checks also showed queued or in-progress states without an Android product artifact. This is runner allocation pending, not a product failure. No retry was issued. The exact-head CI runs already scheduled for #160 and #161 remain the next evidence source.

PR #155's most recent observed source before this ledger append was `a62fe55bc6daa05b3733d80dd6f55b989fab6d9f`, tree `276cc02968489abfff0c9791d4736ab204e3c945`; its observed runs: Full `40882305754`, Fast `40882305710`, Native platform `40882305768`, SQLite `40882305751`, RustSec `40882305759`, Git Hooks `40882305803`. These jobs were queued/in progress at query time. The new ledger commit will create a successor source; all these run results remain attached to the recorded `a62fe55…` source only.
