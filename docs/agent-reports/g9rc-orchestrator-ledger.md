# G9R Orchestrator Ledger

Checkpoint: 2026-10-29 UTC. This ledger records the current campaign state; GitHub run/job state can advance independently. Reconfirm each exact source before accepting later evidence.

## Authority and source map

| Item | Status / source |
|---|---|
| Protected main | `e0acb0254820c381dd97828824bef86a1db5176c` — unchanged; no merge performed |
| G7 baseline | `0560e1e7bcd13f0194eab8caca89d70ea5a2f142` |
| PR #160 Native baseline | OPEN/DRAFT; head `025aff71effb03e0fe3bd540079530369e9beaf7`, tree `73ff9019802aba4a7ac9437e52ff407842fbfcdc`; base G7 |
| PR #162 Android recovery | OPEN/DRAFT; head `6b7a741362b58b6bbc7728e69de90d929391dcaf`, tree `e61db67fcdf0ce7c5c5ab7e93efaef3bac4c1b5d`; base PR #155 `c6d9b3971c19d75c6824de616e703b105b6be71f` |
| PR #165 Native limits follow-up | OPEN/DRAFT; head `8dde33024b76efc3d32812e7d580f8a33db94c9f`, tree `4fca4c7fea82bd8c3bf59481fc8341d8165f9c54`; base PR #160 head |
| PR #166 Native integration candidate | OPEN/DRAFT; same head/tree as #165; base G7. Created to activate the Native focused workflow while leaving #160 and #165 unchanged. |
| PR #159 Release/Owner packet | OPEN/DRAFT; head `982c8271698c5ea323e7a4d19f4dd662087db2c5`, tree `11e3c0ca61577bb435e0f69b1fcd24313857ff1f` |
| PR #163 Static model matrix | OPEN/DRAFT; head `2c109276bcbdd4e14fb2b6e9558962516f2df32c`, tree `b04164da95bf66f43fd08e5537e4d0e242d6a566` |

PR #160's earlier CI is not transferred to later heads. Its run `40884851425`, attempt 1, is a PASS for the pinned SmolLM2 x86_64/API35 Native scenario only. ARM64 APK packaging in that run does not prove ARM64 runtime.

## PR #162 — Android recovery

Android run `40889965195`, attempt 1, is bound to `6b7a741362b58b6bbc7728e69de90d929391dcaf` / tree `e61db67fcdf0ce7c5c5ab7e93efaef3bac4c1b5d`.

- Android Change Scope, Security Tests, Build/Lint/Instrumentation Compile, Integration, Documentation, Dependency Audit, and Android Unit jobs: SUCCESS.
- Security JUnit artifact `12905248797`, 1,826 bytes, digest `sha256:bbb04506625dfd531dd5f26e35055467ee7a0a5b499cf8c9bddc4788870d3709`. Its verifier did not compare result identities to source because `--source-root` was omitted.
- Android Unit artifact `12907406561`, 29,786 bytes, digest `sha256:4e26754de49c05b9016d4f9ba37c2252e03fae4a47863138bb2a49aa1604726e`.
- Android Integration artifact `12907331313`, 1,214 bytes, digest `sha256:2762381fbdfe6cdd82bc58dc83fdc36c6f3df094f34c5d671dce8d21cbe2a838`.
- APK candidate artifact `12906433639`, 33,174,181 bytes, digest `sha256:80edf95c7452ec2116084c31c75527fd95c9c5490b976186906d81a55020e13f`. APK internal hash/signer have not been read from provenance.
- Storage/Emulator job `123526283372`: API26 instrumentation step failed; artifact `12907539920`, 769,248 bytes, digest `sha256:d08313d38c2f211a9cfc092153c7d912cec4faa8832c287956fbd15eb013b2c8`.
- API26 failing JUnit identities/counts are not yet available: artifact download returned HTTP 403 and the job-log API returned HTTP 503 while the overall job remained active. Do not guess the cause.
- API35 instrumentation step is currently IN_PROGRESS on the same job; its result is not yet known. API26 failure does not transfer to API35.
- Required definitions remain 103 Required + 2 Additional = 105; #162 did not change their identities.
- Fast `40889965202` and RustSec `40889965176` passed exact source. Full `40889964943`, SQLite `40889964716`, Git Hooks `40889964753`, and Native platform `40889965376` were queued/partial at last inspection.

Static review of #162 accepts the process-death harness, account/session isolation, and Gmail body-only handoff tests. Runtime proof depends on the API26/35 artifact results. No real Google OAuth/data or Cloud egress was used.

## PR #165/#166 — Native limits, diagnostics, and proposal

Agent C independently **ACCEPTED static review** of #165 exact HEAD `8dde33024b76efc3d32812e7d580f8a33db94c9f` / tree `4fca4c7fea82bd8c3bf59481fc8341d8165f9c54`.

Focused local evidence reported by Agent A (not hosted exact-source CI):

- `:app:testDebugUnitTest --tests app.serea.android.LocalModelChatAdapterTest --tests app.serea.android.LocalModelNativeRuntimeTest`: 24 tests, 0 failures/errors/skips.
- `:app:compileIsolatedAndroidTestKotlin`: PASS.
- Native variant isolation verifier: PASS; only `isolated` remains enabled.
- Native opt-in `:app:assembleBeta`: rejected by the expected fail-closed guard.
- Metrics parser 4/4, instrumentation stream parser 8/8, upload allowlist, JNI contract, preflight, cleanup, shell syntax, YAML parse, and `git diff --check`: PASS.

Changes: Native context default now 512, matching backend support; `maxModelFileBytes` is an input-file limit, not a process-memory cap. PSS samples are observational and a complete-sample indicator requires load, two inference turns, and both release samples. Raw instrumentation/logcat is removed before successful evidence publication; artifacts are allowlisted and JUnit failure messages/stacks are suppressed. Diagnostic JNI remains off by default and enabled only for the isolated variant. `LOCAL_MODEL_CHAT_ENABLED` is a separate future Product Chat flag, default off; ordinary Chat remains DEMO/disconnected until ARM64 runtime and independent review. No production composition changes.

ARM64 route: a single API35 `arm64-v8a` guest experiment on x86_64 Linux uses software CPU/GPU emulation, a pinned image revision, a single SHA-pinned 101,016,128-byte fixture, 30-minute job bound, explicit `ARM64_GUEST_SOFTWARE_EMULATED` labeling, and arm64 JNI APK assertion. It cannot prove ARM hardware/NEON speed, thermal behavior, Pixel acceptance, or physical-device runtime.

PR #166 exists to run the same exact source against the original G7 base, which matches the focused workflow's base-branch filter. Focused run `40899129986`, attempt 1, source `8dde33024b76efc3d32812e7d580f8a33db94c9f`, tree `4fca4c7fea82bd8c3bf59481fc8341d8165f9c54`: x86_64/API35 Native job `123536283262` and ARM64 software-emulated guest job `123536283414` were QUEUED, with no artifacts at last check. Thus this new head has no Native runtime inference evidence yet. PR #165's regular CI at that source was also queued/pending; PR #165's prior source attempt was cancelled by docs-only HEAD updates and remains history, not current evidence.

## Model selection and G10

PR #163's static model metadata/license screen is independently accepted; it makes no runtime or quality claims and does not choose a default. No weights were downloaded. LFM3, Bonsai 2, Qwen3.5, Gemma 4, FunctionGemma, and SmolLM2 remain candidates with varying license/backend unknowns.

Current `maxModelFileBytes` default is 768 MiB and max is 2 GiB. LFM3 Q4_K_M and Bonsai 2 4B Q2_0 listed files exceed the default; Gemma 4 E2B GGUF exceeds the maximum; Qwen3.5's listed safetensors format is incompatible with the current GGUF adapter. Bonsai PTQ1_0/PQ2_0 are incompatible with the pinned backend; Q2_0 is NOT_RUN. File size is not RAM/PSS evidence. Standard model remains UNDECIDED; Japanese/tool quality, TTFT, tokens/s, memory, thermals, and Pixel data are NOT_RUN.

PR #159 G10 packet is useful but PARTIAL and stale relative to later #162/#165/#166 evidence. O-01 package/coexistence, O-02 OAuth/Calendar, O-03 provider eligibility, O-05 signing, O-09 Gmail Restricted scope, O-07 main merge, and O-10 model default are pending; O-04 Google-origin Cloud egress is NOT_APPROVED; O-06 Pixel is BLOCKED/NOT_RUN; O-08 is NO-GO. No Owner certificate, actual Beta APK, release migration rehearsal, complete SBOM/native NOTICE/provenance, Google Console eligibility check, or physical Pixel acceptance exists. The next decision packet update must cite PR #163 exact source/tree and refresh the CI snapshot.

## Decisions and invariants

- Personal Beta: **NO-GO**.
- No main merge, real OAuth/account data, Cloud key/billing, production signing, physical Pixel 7a use, or distribution occurred.
- Cloud remains disabled; Google-origin Cloud egress is forbidden; no automatic external actions; no silent fallback.
- Native model remains disconnected from ordinary Chat pending ARM64 runtime and independent security acceptance.
- No Required gate, assertion, or test identity was weakened.

## Next actions

1. Let PR #162 API35 complete; retrieve exact API26/API35 JUnit identities and counts once job logs are available. Classify failures before proposing fixes; no retry loop.
2. Let PR #166 exact-source x86_64 Native and ARM64 software-emulated jobs run when hosted runners are assigned; capture run/job/attempt/artifact digests and distinguish build, guest runtime, and hardware acceptance.
3. If either test lane fails, record exact JUnit identity and cause, fix only the owning files, then test a new HEAD and repeat independent review.
4. Refresh the owner/release ledger with #163 model matrix and the current CI facts after active jobs reach terminal.
5. Keep G8 Evidence Closure, Android Recovery, Native Runtime, G9, and G10 PARTIAL until their specific evidence gates pass. Keep Personal Beta NO-GO pending all P0 evidence and Owner decision.

## Subsequent checkpoint — 2026-10-29 15:24 UTC

- Central ledger commit `4e36b8225be7f5f45a2104188750d4226af01df9` / tree `a4adb8e192827cf109e3c34a161fc35fac6e018a` is on `codex/serea-g9rc-orchestrator-ledger`.
- PR #167 G10 documentation refresh is OPEN/DRAFT at head `110ceb583573acef54cb9ce21254da49eeb49f13`; tree `27a663e4931cdc35a678bf7744eca0ac5f511c3b`; based on PR #159 head. Agent C independently ACCEPTED the exact documentation diff. Documentation-only checks are pending; it is not Android product evidence.
- PR #162 run `40889965195` remains in progress. API26 failed; API35 step remains active. No JUnit identity/count detail or API35 artifact is available yet.
- PR #166 focused run `40899129986` remains queued; x86_64 Native job `123536283262` and ARM64 software guest `123536283414` have not started.
- O-01 through O-10 are unchanged; Personal Beta remains NO-GO.

## API26 artifact inspection — 2026-10-29 15:28 UTC

The GitHub artifact connector successfully retrieved API26 artifact `12907539920`; downloaded ZIP SHA-256 matches the Actions digest `d08313d38c2f211a9cfc092153c7d912cec4faa8832c287956fbd15eb013b2c8`.

On PR #162 exact source `6b7a741…` / tree `e61db67…`:

- Required manifest: **105/105 PASS**, 0 failures, 0 errors, 0 skips.
- Calendar class: 19/19 PASS; Chat screen: 16/16 PASS; cursor diagnostic: 1/1 PASS.
- Actual process-death lifecycle: PASS; artifact records 10 PASS events including seed, process-death, verify, and cleanup phases. Data Isolation: PASS.
- Separate Google/Gmail UI: 16 tests, 2 failures, 0 errors, 0 skips. Failed identities:
  - `app.serea.android.GoogleCalendarDemoLoopbackUiTest#consentUiOpensLoopbackAgendaAndAccountSwitchDisconnectsIt` — `performScrollTo()` could not find test tag `google_auth_demo_account_account_a` after returning from Calendar view. Classification (product vs test navigation state) not yet confirmed.
  - `app.serea.android.GoogleCalendarHandoffRevocationUiTest#confirmedCalendarHandoffCannotSendAfterSyntheticAccountSwitch` — timed out at line 66 waiting for `Design review` after entering the local-zone query date derived from fixture instant `2026-10-20T00:00:00Z`. The contextual catch did not catch `ComposeTimeoutException`, so the artifact omits the zone/query-date diagnostic; root cause not yet confirmed.

The API26 aggregate campaign is FAIL because the separate Google/Gmail UI stage failed; this does not change Required's 105/105 pass. Agent B is analyzing both exact failures and preparing a focused successor based on #162 without modifying #162. API35 remains a separate active job.

## API35 artifact inspection — 2026-10-29 15:29 UTC

GitHub artifact `12909203169` is 3,479,526 bytes; the downloaded ZIP SHA-256 matches Actions digest `a265911ad3c8e6aa5d6f231d7fc2d88591e46fa4163f77f9b73cd79059bc2c77`.

On the same PR #162 exact source `6b7a741…` / tree `e61db67…`:

- Required manifest: **105/105 PASS**, 0 failures, 0 errors, 0 skips.
- Calendar class: 19/19 PASS; Chat screen: 16/16 PASS; cursor diagnostic: 1/1 PASS.
- Process Death: PASS with 10 observed PASS lifecycle events; Data Isolation: PASS.
- Separate Google/Gmail UI: 16 tests, 2 failures, 0 errors, 0 skips. The same two JUnit identities fail as API26:
  - `app.serea.android.GoogleCalendarDemoLoopbackUiTest#consentUiOpensLoopbackAgendaAndAccountSwitchDisconnectsIt` — account-A test tag absent after return from Calendar view.
  - `app.serea.android.GoogleCalendarHandoffRevocationUiTest#confirmedCalendarHandoffCannotSendAfterSyntheticAccountSwitch` — `Design review` fixture wait timed out at line 66; contextual diagnostics were bypassed by `ComposeTimeoutException`.

Both APIs pass all 105 Required tests but fail the separate 16-test Google/Gmail UI stage on these same two identities. Root cause is under active Agent B analysis; do not call Android/G9 product acceptance SUCCESS. API35 package identity isolation is a separate temporary-signing job and was SKIPPED; do not infer it from these emulator results. The API Storage/Emulator job `123526283372` concluded FAILURE due the UI stage; the enclosing Android workflow remains queued behind a Pixel evidence bundle job, which is not Pixel physical acceptance.

## UI failure classification and refreshed G10 packet — 2026-10-29 15:31 UTC

Agent B inspected both API artifacts and exact tests. The loopback failure is a test navigation defect: first Back from the Calendar event detail returns to the agenda; the test searched for a Settings-only account chip before returning from the agenda to Settings. The Handoff test's failing evidence is a missing diagnostic: `ComposeTimeoutException` bypasses its `AssertionError` catch while waiting for `Design review`. Agent B is correcting test navigation to use the intended back-stack transitions, replacing the text-only wait with bounded fixture-event identity/date/session diagnostics, and scrolling to the resolved event index. Security assertions remain unchanged. The fix is on a separate successor branch based on exact #162; local AndroidTest Kotlin compilation passed; emulator execution on the fix head is NOT_RUN. The branch is awaiting final commit and independent review.

Both #162 API artifacts are now fully inspected and confirm 105/105 Required PASS on each API, same two UI failures, 10 process-death PASS events, and Data Isolation PASS. Android Storage/Emulator job `123526283372` concluded FAILURE due the separate UI stages. The enclosing Android workflow remains QUEUED on Pixel evidence bundle job `123546278019`; temporary-signing Identity Isolation job `123546280944` is SKIPPED. This does not establish identity isolation or physical Pixel acceptance.

PR #167 documentation refresh is OPEN/DRAFT at head `f2ce5af26bcb5d1525a6a0099937b568c8933fa2`, tree `d817f9eea74783d788cd5be1bd02cf5b538c3aa9`, based on #159's owner-packet branch. Agent C independently ACCEPTED its exact docs diff after verifying both artifact ZIP digests, counts, identities, process/data isolation statuses, and unchanged O-01..O-10. PR #167's docs-only CI is pending.

PR #166 Native focused run `40899129986` remains QUEUED at head `8dde330…` / tree `4fca4c7…`; x86_64 job `123536283262` and software-emulated ARM64 guest job `123536283414` have not started. Native inference on the follow-up source remains NOT_RUN.

## Native/Android review follow-up — 2026-10-29 15:33 UTC

- PR #168 Android test-only fix candidate is OPEN/DRAFT at head `272f68ecf7f8072b812b13e46064a108bab5ef2b`, tree `010eca037f28f5d5dd5818ed99a2601852a719e9`, based on PR #162 exact head. Agent C **ACCEPTED static review**: the first Back returns detail→agenda; the added second Back returns agenda→Settings before account selection. The fixture event lookup is bounded by event identity/date/session diagnostics and uses scroll-to-index, retaining the title assertion and all security checks. Required identities are unchanged; diff only touches two Android UI test files. Local Kotlin AndroidTest compile PASS; hosted Android run `40902585691` is QUEUED, so the fix has no API26/35 runtime evidence yet.
- For PR #165/#166 exact source `8dde330…` / tree `4fca4c7…`, Agent A reports regular CI passes: Fast `40899130000`, RustSec `40899129919` (artifact `12909132822`, 3,184 bytes, `sha256:11d720b5c2b08e2402a5f1e4fe99082716ebb254b318af89f3dd7b4a5a6fe79f`), Mock Beta Contract `40899130224`; Full CI `40899129925` Linux MSRV, Windows P7 synthetic, and Linux stable jobs passed while macOS jobs queued; Android `40899129978` Change Scope/Security/Integration passed, Unit and Build/Lint queued; Native platform `40899129918` is mixed/in progress/queued; SQLite `40899130017` Intel producer passed, arm consumer queued; Git Hooks `40899130057` Ubuntu/Windows passed, macOS queued. None establishes inference.
- Agent A's additional source review found an artifact-boundary gap in the ARM64 experiment: its upload selected all of `evidence/native-arm64-guest/`, including raw `emulator.log` and `adb-devices.txt`; sanitized JUnit XML is also omitted. This new finding overrides the earlier static acceptance for that workflow artifact boundary only. Agent A is preparing a narrowly scoped patch against #166 in an independent worktree; PR #165/#166 remain unchanged pending C review and parent integration. Runtime jobs remain queued, no artifacts or inference.

## UI remediation candidate checkpoint — 2026-10-29 15:36 UTC

PR #168 is OPEN/DRAFT, stacked directly on #162's exact head `6b7a741362b58b6bbc7728e69de90d929391dcaf`. New source/tree: `272f68ecf7f8072b812b13e46064a108bab5ef2b` / `010eca037f28f5d5dd5818ed99a2601852a719e9`. It changes only `GoogleCalendarDemoLoopbackUiTest.kt` and `GoogleCalendarHandoffRevocationUiTest.kt`; Required identities/manifests and production/security files are unchanged.

Agent B's local `:app:compileIsolatedAndroidTestKotlin` and diff check pass. Agent C **ACCEPTED static review** on this exact tree: test back-stack actions reflect detail→agenda→Settings, and event identity/date/calendar/session diagnostics are bounded while title and security assertions remain. The underlying Handoff `Design review` fixture timeout is still **unconfirmed**; the new test will diagnose it rather than suppress it. PR #168 Android run `40902585691` is queued without a runner; focused API26/35 tests on this new head are NOT_RUN. Fast CI `40902585494` passed exact PR #168 source.

New PR #168 results are not credited with PR #162 API26/35 passes; those older source results remain 105/105 Required per API and UI 16/2 failures per API.

## Native artifact guard review — 2026-10-29 15:38 UTC

Agent A's separate ARM artifact fix is on branch `codex/serea-native-artifact-boundary`, source `5f9cc22c4140d061253537e908533e6b141d901d`, tree `b43e79c9f634fcdfacbad1d534a74bc4df9dae5b`, based on PR #166 exact source `8dde330…`. It changes only the Native workflow and allowlist contract test; the ARM upload paths are currently explicit and raw emulator/ADB outputs are removed before upload; sanitized JUnit is included.

Agent C independently reviewed the patch and returned **CHANGES_REQUESTED**: the contract test rejects bare evidence-directory paths but still accepts wildcard variants such as `evidence/native-arm64-guest/*` and `${{ steps.arm64_native_e2e.outputs.evidence_dir }}/**`. Required exact-file validation/wildcard-negative fixtures are being added. This patch is not yet integrated into #166; Native runtime jobs remain queued/NOT_RUN.

### Native artifact guard review round 2

At exact patch head `6122751a3e0349d8008237a553495fa402fb0e37` / tree `5e82a922125a864811c1f2e35f61df22110c0cbf`, C confirmed the current workflow uploads explicit safe paths and covers multiline glob negative fixtures, but requested another fix: the validator only parses block-style `path: |`; a second `upload-artifact` step using scalar `path: evidence/native-arm64-guest/*` bypasses the guard. Agent A is extending the contract to cover scalar/folded/all upload-artifact `with.path` styles and adding the scalar regression fixture. No runtime experiment has started.

### Native artifact guard review round 3

Parent independently injected an inline YAML flow-style upload action into patch head `9e0be352ec73aebb2a754188cde9a54592da3f7e` / tree `55b25959353d1bdb1ef12f50ca622f74d81482b8`:

`with: {name: injected, path: evidence/native-arm64-guest/*}`

The current validator returned no errors, bypassing path declarations. Agent C confirmed this finding and superseded its prior acceptance with **CHANGES_REQUESTED**. Agent A is extending flow-style `with.path` detection/rejection and adding a negative fixture. The current actual workflow still uses explicit allowlisted block paths; no artifact was uploaded and no Native run has started.

### Native artifact guard parent fix — 2026-10-29 15:42 UTC

Parent addressed the flow-mapping escape in isolated branch `codex/serea-native-artifact-inline-guard`, based on the reviewed A branch head `9e0be352ec73aebb2a754188cde9a54592da3f7e`. New exact head/tree: `24bf1ca463f12fe521de9d64d495ce6c8c4d7ae9` / `3956f4f444ed51014f7ab03c4ebcb65eb69ed604`. The one-file diff rejects flow-style `with: {}` on every `upload-artifact` step and adds the inline wildcard regression fixture. Artifact allowlist contract and `git diff --check` pass. C independent review is pending. No PR #166 change or Native run yet.

## Native Artifact gate integrated — 2026-10-29 15:43 UTC

Agent C **ACCEPTED** the final artifact-guard fix at `7c8bfb05db14e8fbab17db75f4027ae13fc3b87e` / tree `eb3559263d679facf5afaf8bcc16a8b7d6244520`. The guard validates every upload-artifact `with.path` declaration, requires safe block lists, exact allowed file paths and rejects globs/flow mappings, including `with : {…}`. Raw emulator/ADB diagnostics are ephemeral and removed; the sanitized Native JUnit XML is uploaded. Contract, YAML, shell, Python compile and diff checks pass. This accepts artifact-boundary code statically only.

Parent integrated that accepted descendant by normal fast-forward into PR #166's branch `codex/serea-g9rc-native-accepted`; PR #160 and #165 remain unchanged. PR #166 is now OPEN/DRAFT at source `7c8bfb05db14e8fbab17db75f4027ae13fc3b87e`, tree `eb3559263d679facf5afaf8bcc16a8b7d6244520`, base G7. New focused run `40904857394`, attempt 1, source-bound to that exact SHA: x86_64/API35 Native job `123556718172` and ARM64 software-emulated guest job `123556718597` are QUEUED; no artifacts, boot, model hash, JNI load, inference, or JUnit result yet. Earlier #166 run `40899129986` at `8dde330…` is historical and not evidence for this updated head.

## Native context-bound integration checkpoint — 2026-10-29 16:02 UTC

The existing PRs #162, #165, #166, #168, and #167 remain unchanged. A fresh Native candidate was created from the G7 base after Agent C accepted the capability-bound diff on PR #165's source; no inference evidence has been transferred.

| PR | Source / tree | Current exact CI |
|---|---|---|
| #166 Native reference | `7c8bfb05db14e8fbab17db75f4027ae13fc3b87e` / `eb3559263d679facf5afaf8bcc16a8b7d6244520` | Focus run `40904857394` attempt 1; x86_64 job `123556718172` and ARM64 software-guest job `123556718597` QUEUED |
| #168 Android UI fix | `272f68ecf7f8072b812b13e46064a108bab5ef2b` / `010eca037f28f5d5dd5818ed99a2601852a719e9` | Android run `40902585691`, Change Scope `123548599390` QUEUED; Fast `40902585494` PASS |
| #169 Native capability integration | `87af6aab9c9db8f65e18ca18456256e03797ffb3` / `0fceca72877574acdd3cdbfa67b363dcabfb4dda` | Focused Native `40907496496`: x86_64 `123566146405` and ARM64 guest `123566148073` QUEUED. Android `40907496673`, Change Scope `123566146896` QUEUED. Full `40907496594` all 5 jobs QUEUED; Fast, RustSec, SQLite, Git Hooks, Beta Contract also QUEUED. |

PR #169 is OPEN/DRAFT at `codex/serea-g9rc-native-context-integration`, base `codex/serea-g7-calendar-readiness` SHA `0560e1e7bcd13f0194eab8caca89d70ea5a2f142`. Its one follow-up commit `87af6aa` integrates the reviewed context/output/thread bounds into a new candidate; #165 and #166 are preserved. Agent C accepted the three-file diff on source #165 and is reviewing the full new candidate exact head; that review status is PENDING at this checkpoint.

### Context and memory boundary

`LocalModelRuntimeLimits` default context is 512, matching the pinned llama.cpp JNI maximum. The new candidate rejects context >512, output >256, and threads >2 in Kotlin construction as well as retaining the Native pre-allocation checks. The boundary test covers context 513, output 257, and threads 3. Widening limits requires backend capability evidence. The focused local command on the candidate tree content was `./gradlew --no-daemon testDebugUnitTest --tests app.serea.android.LocalModelChatAdapterTest --tests app.serea.android.LocalModelNativeRuntimeTest`: 25 tests, 0 failures/errors/skips. The test ran before the commit was made but against identical file contents; the resulting committed tree is `0fceca7…`.

`maxModelFileBytes` constrains input file length, not resident memory. PSS/RSS observations are diagnostic only; there is no OS-enforced hard RAM cap. Native model ownership is single-lease and context/thread/output/file bounds reduce exposure but do not establish device memory acceptance. x86_64/ARM64 runtime on #169 is NOT_RUN while CI is queued. Ordinary Chat remains disconnected; Product Chat flag stays off.

### Environment and Android status

The local workspace has JDK 17 and Android SDK build/platform tools, but no Emulator binary/images and no `/dev/kvm`. Android CI also remains queued without runner assignment. The hosted Required gates, API26/35 UI, IME, Identity, Process Death, and Data Isolation on #168/#169 are NOT_RUN. No local substitute was counted as formal acceptance. The SDK manager initially could not validate the injected proxy certificate; using the system CA trust store allowed package-list inspection, but no Emulator package was installed. No credential or OAuth was used.

PR #162 remains bound to its own exact-source artifacts: API26 and API35 each passed Required 105/105 and Process Death/Data Isolation, while their separate 16-test Google/Gmail UI stage had 2 failures. PR #168's proposed test-only repair has no runtime evidence yet. Keep Android/G9 acceptance PARTIAL.

### Current disposition

- Android Instrumentation Recovery: PARTIAL; #168/#169 runner jobs queued, new-head tests NOT_RUN.
- Native Runtime: PARTIAL; #160's x86_64 two-turn pass remains exact-source historical evidence only. #166/#169 x86_64 and ARM64 guest runtime NOT_RUN.
- ARM64 physical/runtime and Pixel 7a: NOT_RUN; software guest result cannot imply hardware acceptance.
- G8/G9 closure: PARTIAL. G10: PARTIAL. Personal Beta: NO-GO.
- Owner decisions O-01..O-10 unchanged; no main merge, OAuth, real Google data, Cloud key/billing, production signing, Pixel use, Beta distribution, or GO declaration.

Next resume: monitor only existing run attempts above; once terminal, retrieve and hash artifacts, inspect API-specific JUnit and Native sanitized evidence, classify failures, and review the exact tested HEAD before any further integration. Do not retry queued jobs without a source-backed infrastructure diagnosis.

## PR #169 review and runner update — 2026-10-29 16:03 UTC

Agent C completed the full exact-HEAD review: **ACCEPTED** at `87af6aab9c9db8f65e18ca18456256e03797ffb3` / tree `0fceca72877574acdd3cdbfa67b363dcabfb4dda`. Review confirms the diff from #166 is only Kotlin capability-bound validation, a boundary test, and report text; limits align with existing JNI checks, no silent clamp or lifecycle/security change; Native artifact allowlist stays sanitized; ordinary Chat remains disconnected.

Run `40907496496`, attempt 1, advanced: x86_64 job `123566146405` is IN_PROGRESS on runner `1000024660` at “Install pinned Android build and Emulator packages”; ARM64 software-guest job `123566148073` remains QUEUED. No APK, JUnit, model hash, JNI load, inference or runtime artifact exists yet. Android run `40907496673` Change Scope job `123566146896` remains QUEUED; other exact-head full/security/hooks/SQLite/contract jobs remain queued. PR #169 stays PARTIAL with x86_64 and ARM64 inference NOT_RUN.

## PR #169 active-job checkpoint — 2026-10-29 16:05 UTC

The exact-source Native x86_64 job `123566146405` on PR #169 has passed Android SDK/Emulator installation (step 5) and source checkout/CMake dirty-tree checks (steps 6–7), and remains IN_PROGRESS at focused JVM contracts (step 8). API35 emulator boot and inference steps have not run. ARM64 guest job `123566148073` and Android Change Scope job `123566146896` remain QUEUED. The active job-log endpoint returned HTTP 404 while the job was running; this is unavailable live log evidence, not a workflow failure. No retry or runner workaround was attempted.

## PR #169 focused JVM completion — 2026-10-29 16:06 UTC

On exact PR #169 source `87af6aab9c9db8f65e18ca18456256e03797ffb3` / tree `0fceca72877574acdd3cdbfa67b363dcabfb4dda`, Native x86_64 job `123566146405` completed the source-identity and focused JVM-contract step successfully. It is now running “Build opt-in product JNI APKs for both supported ABIs” (step 9). This is not a runtime result. ARM64 guest `123566148073` and Android Change Scope `123566146896` remain queued at this observation.
