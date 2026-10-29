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
| PR #166 Native integration candidate | OPEN/DRAFT; current head `7c8bfb05db14e8fbab17db75f4027ae13fc3b87e`, tree `eb3559263d679facf5afaf8bcc16a8b7d6244520`; base G7. It includes the previously listed #165 limits/diagnostics and deferred Chat proposal, and activates the Native focused workflow while leaving #160 and #165 unchanged. |
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

## V4 continuation checkpoint — 2026-10-29 16:57 UTC

### New candidate heads and review state

| PR | Exact source / tree | Base / review | Current CI state |
|---|---|---|---|
| #170 Identity validator | `fc9b355fb247bc6cd1eb35cd313379f6ed01a60f` / `01d27c481655c7b13c3ebda27190805b8aae22c3` | Based on #169 exact `87af6aab9c9db8f65e18ca18456256e03797ffb3`; Agent A ACCEPTED the strict set/membership validator; local focused Python tests 13/13 pass | Fast `40914655659` and RustSec `40914655648` PASS. Android `40914655856` remains pending/queued after Integration and Dependency Audit pass; no exact-head Android Unit result yet. |
| #171 ARM64 host dependency | `8d4d3cdb44f44d31dc06299579f53427886c35c8` / `c88db8907fa7774bf1027a326bf9ddf7f6a09a36` | Based on G7. Agent C ACCEPTED static review of `libpulse0` install plus QEMU `ldd`/package-owned-library preflight; no alias/symlink workaround | Focused run `40915105551`: x86_64 and ARM64-guest jobs `123593126707` / `123593126899` still QUEUED, no steps or artifacts; ARM64 runtime NOT_RUN. |
| #172 package recovery / timeout-test follow-up | `28f52c2999795ab6d1c83aa456d5186205c8307a` / `e2c735be01ef14e22cd4736a04798c21a1d413f1` | Based on #168. Agent A CHANGES_REQUESTED for absent-package reinstall provenance and fail-closed hash/SHA command regression coverage | No conclusion to carry forward to a later head. |
| #173 package recovery review fix | `540fd754e81c3788a76cc9452f754211e95d8378` / `f52c4104297edae8d84db72680de34136752f4a6` | Draft, stacked on #172 (which is based on #168). Agent C's review of its prior head requested a regression for `adb install` failure; B added it as a normal commit, with no history rewrite. Independent review of this new exact head is pending. | Exact-head PR runs: Android `40916731468` PENDING; Fast `40916731448`, Git Hooks `40916731516`, Native `40916731604`, Full `40916731515`, RustSec `40916731566`, and SQLite `40916731473` QUEUED. Local focused package-state test PASS, `android_emulator_workflow_tests.py` 34/34 PASS, `bash -n` and `git diff --check` PASS; no hosted emulator evidence. Push-triggered run `40916718371` failed without applicable product jobs; not test evidence. |

### Refreshed run evidence and unresolved blockers

- PR #168 Android run `40902585691`, exact source `272f68ecf7f8072b812b13e46064a108bab5ef2b`, remains IN_PROGRESS. API26 Required 105/105 and Google/Gmail UI 16/16 passed on its exact artifact, but the separate Fake Google process-death package-path harness failed after uninstalling the disposable target. Android Unit remains 1/408 failure at `GoogleCalendarHttpReadPortTest#timeoutMapsToTimeout` (expected TIMEOUT, observed PERMISSION_DENIED). The API35 campaign job `123577163290` is still IN_PROGRESS at API35 instrumentation; its artifact/result is NOT_RUN until the job terminates. PR #168 cannot be accepted yet.
- PR #169 Android run `40907496673`, exact source `87af6aab9c9db8f65e18ca18456256e03797ffb3`, API26 Required ran 105 tests with four `ChatHttpE2ETest` failures: `MockWebServer.start` hit `SocketException: EACCES`; this is a test/runtime permission issue, not a failure of the other 101 tests. API35 job `123582802971` is IN_PROGRESS; no result yet. The Native isolation permission boundary remains offline and must not be weakened.
- PR #169 Native run `40907496496`: x86_64 job `123566146405` is SUCCESS at the exact #169 source, including verified fixture and two-turn API35 x86_64 inference through local Egress/Usage, cancellation/release and cleanup. ARM64 guest `123566148073` failed before boot at QEMU startup due to missing `libpulse.so.0`; it did not run Native inference. This is host dependency failure, not ARM64 model incompatibility. Separate fix #171 is queued and has no new runtime evidence.
- Agent C is implementing a restricted `isolatedHttp` build/test variant in worktree `/workspace/serea-api26-http-split`, currently on base #169 source with uncommitted edits to `android/app/build.gradle.kts` and two variant manifests. No committed head, build, tests, PR, or review exists yet. Required follow-up: prove the 4 loopback HTTP test identities run only in the network-enabled isolated test variant, the other 101 Required identities remain in the offline isolated Native-compatible variant, and the unchanged strict manifest still aggregates exactly 105 unique Required results per API. Agent A is assigned independent review after a concrete committed source exists.
- Model evidence remains exact-source only: Native x86_64 inference PASS is limited to #169 source and the previously recorded #160/#169 sources; ARM64 Runtime NOT_RUN; physical ARM64/Pixel 7a NOT_RUN. Standard model undecided and ordinary Chat remains disconnected/default-off.

### Current dispositions

- G8 Evidence Closure: PARTIAL.
- Android/G9 UI acceptance: PARTIAL; #168 API26 UI JUnit passed, but harness/package state and unit timeout test remain unresolved; API35 is in progress.
- Required on #162: 105/105 passed on each API on #162 only. #169 API26 has 4 failures; #168 new-head API35 is still in progress. Do not transfer older results.
- Native x86_64: SUCCESS only on exact #169 source. ARM64 Native Runtime: NOT_RUN.
- G9 Integration Candidate / G10 Preflight: PARTIAL. No new integrated frozen HEAD or final formal CI yet.
- Personal Beta: NO-GO. Owner decisions O-01..O-10 and all security/consent boundaries remain unchanged.

Next resume: obtain the final independent review of #173 head `540fd754…`; inspect exact #168/#169 API35 artifacts after their active jobs terminate; finish and review the isolated HTTP test split before opening/accepting its PR; then select only ACCEPTED changes into a new integration candidate and run formal CI on that exact frozen head. Do not treat queued jobs, skipped jobs, software-emulated ARM64, or old-source evidence as success for a new head.

## V4 review update — 2026-10-29 17:00 UTC

- Agent C independently reviewed the cumulative #168 → #173 change, not only the last commit. Decision: **ACCEPTED** for exact #173 source `540fd754e81c3788a76cc9452f754211e95d8378` / tree `f52c4104297edae8d84db72680de34136752f4a`, based on exact #168 `272f68ecf7f8072b812b13e46064a108bab5ef2b` through #172. Reviewed scope is timeout classification test, Fake Google process-death harness, and fail-closed fake-ADB regression tests. Timeout is deterministically injected as `SocketTimeoutException`; the test checks TIMEOUT mapping, one credential lookup, and zero server requests. Harness validates target APK presence/hash, reinstalls only the campaign APK if absent, re-queries package path, and fails on install/hash/provenance errors. Tests cover install success, install failure, mismatch, local and device checksum failures. No OAuth/provider data or production composition changes. Focused review test 1/1, harness suite 34/34, shell syntax, and diff check pass locally.
- PR #173 remains Draft. Exact-head Actions: Android run `40916731468` PENDING; Fast `40916731448`, Full `40916731515`, SQLite `40916731473`, RustSec `40916731566`, Git Hooks `40916731516`, and Native `40916731604` QUEUED. The older Android run `40916342264` on predecessor `4b8a8195…` was cancelled when the new commit arrived; it is not evidence for current head. Push-triggered run `40916718371` ended failure without applicable product jobs and is not a test result. API26/API35 Emulator and UI on #173 remain NOT_RUN.
- The #168 Android campaign job `123577163290` is still IN_PROGRESS at API35 instrumentation after the API26 campaign step failed on the isolated process-death harness. The #169 Android job `123582802971` is also still IN_PROGRESS at API35 instrumentation after API26 reported four loopback `ChatHttpE2ETest` `EACCES` failures. No API35 JUnit artifact/result is available yet.
- PR #169 Native x86_64 remains a source-bound SUCCESS; its ARM64 guest failure is pre-boot missing `libpulse.so.0`. PR #171 has Agent C static ACCEPTED review, but focused run `40915105551` jobs remain QUEUED, so no rerun/runtime evidence exists.
- C is still implementing the isolated HTTP test variant split on base #169 in `/workspace/serea-api26-http-split`; at this checkpoint its worktree is uncommitted, no build has been verified, and no PR/review exists. Agent A will review only a concrete committed exact head.

Next actions unchanged: finish the test-only HTTP variant design with the offline Native target intact; wait for current API35 jobs to reach terminal without retry loops; verify #173 exact-head CI before taking it into a new integration candidate; preserve Personal Beta NO-GO.

## API35 artifact closure update — 2026-10-29 17:08 UTC

The previously active API35 emulator jobs have now terminated. Their evidence is bound to their own source SHAs only:

- PR #168 source `272f68ecf7f8072b812b13e46064a108bab5ef2b` / tree `010eca037f28f5d5dd5818ed99a2601852a719e9`, Android run `40902585691`, API35 instrumentation artifact `12916742164`, size 3,849,637 bytes, SHA-256 `e4e77ba4b5327b2bed8d2ad033146f54f7573a66d9517bc1b47435891d3302b7`. JUnit summary: Required 105/105 PASS (0 failures/errors/skips); Calendar 19/19, Chat screen 16/16, cursor 1/1, Google/Gmail UI 16/16 PASS. Main OS Process Death log and data-isolation status PASS. The overall API35 campaign result is FAIL because `google-demo-ui-status.txt` is FAIL: after instrumentation, UTP uninstalled the isolated target package and the host-side Fake Google process-death harness could not verify it. Cleanup completed, but this separate harness did not pass. The bundled process-death log's other harness is PASS; do not conflate them. Companion emulator evidence artifact `12915274982`, 16,581,691 bytes, digest `sha256:b7b3226b501ad5b7febd837ba92de73ec5c9fba2916c83177596c896b1ebe355`, confirms the API26 and API35 harness distinction. API26 on #168 likewise has Required 105/105 and Google/Gmail UI 16/16 PASS, but its aggregate remains FAIL on the same package-path harness. Neither API result is a #173 result.
- PR #169 source `87af6aab9c9db8f65e18ca18456256e03797ffb3` / tree `0fceca72877574acdd3cdbfa67b363dcabfb4dda`, Android run `40907496673`, API35 artifact `12915723538`, size 2,627,292 bytes, SHA-256 `b21bd251ad68b1d6d3877f569c1bdbc185666b73d175c24c015712b526751bfa`. Required JUnit ran 105 tests, with 4 failures (0 errors/skips), all `app.serea.android.ChatHttpE2ETest` methods: `twoTurnsIncludePersistedHistoryAndUsageThroughLoopbackHttp`, `manualRouterSelectionIsPassedToMockHttpRequestAndNeverFallsBack`, `directCompositionRoutesCatalogModelAndDenialPrecedesUsageReservation`, `innerEgressExpiryReleasesReservationBeforeAnyHttpOrCredentialAccess`. Each fails during `MockWebServer.start()` with `java.net.SocketException: socket failed: EPERM (Operation not permitted)`. The Calendar 19, Chat screen 16, and cursor 1 tests pass, but Android stops at Required failure, so Google/Gmail UI on this API/source is NOT_RUN. API26 on same #169 source has the same four identities failing with `EACCES`; both are test-process socket permission failures because isolated instrumentation removes INTERNET. Do not move or skip these identities or enable INTERNET on the Native isolated variant.
- PR #168 Android Unit artifact `12914261201` remains 408 tests, one failure: `GoogleCalendarHttpReadPortTest#timeoutMapsToTimeout` expected TIMEOUT but got PERMISSION_DENIED. In #172/#173 the test was changed to deterministic asynchronous `SocketTimeoutException` injection without increasing runtime timeout. Parent reran this exact focused JUnit on #173 source `540fd754e81c3788a76cc9452f754211e95d8378` / tree `f52c4104297edae8d84db72680de34136752f4a` using isolated JDK 17 with `--rerun-tasks`: **1/1 PASS**, 0 failures/errors/skips, JUnit time 3.042s. An earlier no-rerun Gradle attempt reported UP-TO-DATE and is excluded as execution evidence.
- PR #173 Android run `40916731468` remains queued on exact head `540fd754…`; its latest Attempt has Change Scope job `123603984767` queued. Its prior attempt's queued work was cancelled by the newer attempt, not a product failure. No #173 emulator artifact/result yet.
- PR #168 and #169 Android workflows remain queued at their non-product Pixel 7a reacceptance evidence-bundle jobs after emulator job failure; no physical device was used and no Pixel acceptance is claimed.

Updated Android disposition: PR #168's Google/Gmail UI and 105 Required test suites individually pass on both APIs at exact #168 source, but the API campaign is not accepted until the package-verification harness and unit failure are fixed and repeated on a new source. PR #173 has cumulative ACCEPTED review and the timeout unit locally passes, but its formal API26/API35 run is pending. PR #169 Required is FAIL on both APIs due the four offline loopback tests; its HTTP split remains in progress. G9 Android closure stays PARTIAL.

### PR #173 local JVM rerun — 2026-10-29 17:11 UTC

Parent reran the complete Android Gradle Unit task on exact PR #173 HEAD `540fd754e81c3788a76cc9452f754211e95d8378` / tree `f52c4104297edae8d84db72680de34136752f4a` using the isolated JDK 17 at `/workspace/.tools/temurin17-download/jdk-17.0.21+9`:

```text
JAVA_HOME=/workspace/.tools/temurin17-download/jdk-17.0.21+9 ./gradlew --no-daemon testDebugUnitTest
```

JUnit XML aggregate: **408 tests, 0 failures, 0 errors, 0 skips**. The focused `GoogleCalendarHttpReadPortTest#timeoutMapsToTimeout` JUnit test is included (21 tests in the class, all pass). The build completed successfully in 1m07s with `:app:testDebugUnitTest` executed. Earlier first attempt without JDK17 failed because system JDK21 lacked `javac`; an earlier no-rerun focused attempt was UP-TO-DATE and excluded. This is local JVM evidence only, not API26/API35 emulator evidence or hosted CI.

### ARM64 runner feasibility audit — 2026-10-29 17:12 UTC

Agent A reviewed exact #171 source `8d4d3cdb44f44d31dc06299579f53427886c35c8` / tree `c88db8907fa7774bf1027a326bf9ddf7f6a09a36`, with no edits. `.github/workflows/g8-local-native-runtime-focused.yml` configures both jobs on `ubuntu-24.04`. The primary job runs API35 x86_64. The second is explicitly a software-emulated ARM64 guest (`arch: arm64-v8a`, `disable-linux-hw-accel: true`, `swiftshader_indirect`), not ARM64 host/hardware execution. Repo CI uses the standard `ubuntu-24.04-arm` label for a Rust Linux ARM64 matrix; there is no source evidence that this provides a supported Android SDK/Emulator host for the present guest experiment. No configured ARM64-hosted Android/Emulator job was found. The Actions runner inventory API returned 403 `Resource not accessible by integration`; no permission workaround or escalation was attempted. Therefore self-hosted/other ARM64 runner availability is UNKNOWN, not absent.

On exact #169 `87af6aab…`, ordinary product Chat is still Demo. `LOCAL_MODEL_CHAT_ENABLED` is a future proposal only; diagnostic `LOCAL_NATIVE_RUNTIME_ENABLED` defaults false and is confined to the isolated build type. There is no Native product composition wiring. Native x86_64 inference is source-bound PASS; software-emulated ARM64 is NOT_RUN/queued at #171 run `40915105551` (jobs `123593126707`, `123593126899`, no artifacts); ARM64 hardware/runtime and Pixel 7a are NOT_RUN. A future software guest PASS cannot establish ARM hardware/NEON throughput, thermal limits, or physical Pixel acceptance. No Feature Flag integration proposal has been connected to ordinary Chat.

### PR #170 Identity validator CI — 2026-10-29 17:14 UTC

PR #170 exact HEAD `fc9b355fb247bc6cd1eb35cd313379f6ed01a60f` / tree `01d27c481655c7b13c3ebda27190805b8aae22c3`, based on #169 exact `87af6aab9c9db8f65e18ca18456256e03797ffb3`, has a successful Android Unit job `123591635633` in Android run `40914655856`. Artifact `12917461874`, size 29,045 bytes, digest `sha256:c08ed0c9fd808cc947cf6eda25f352fbe92705f77bdf3b63debf0cbe5d8a3f6c`, downloaded and hash-verified. Its 34 JUnit XML files aggregate to **399 tests, 0 failures, 0 errors, 0 skips**. Instrumentation JUnit verifier regressions, Android emulator campaign script regressions, Android unit verifier regressions, and full Android Unit JUnit verification all passed in the same job. This closes the stale 15-vs-17 validator mismatch for this exact source; manifests remain at 103 Required + 2 Additional + 17 non-required, with fixed identity set/source matching. Android run Change Scope, Build/Lint/Instrumentation Compile, Integration, Dependency Audit also passed. Fast `40914655659` and RustSec `40914655648` passed. Android Security Tests had been queued at this observation; Android Storage/Emulator job `123606101694` and other formal CI are still queued/incomplete. Thus validator fix CI evidence is SUCCESS, but PR #170 Android campaign and API26/API35 Required/UI acceptance remain PARTIAL/NOT_RUN. The #169 API26/35 loopback Required failures are not fixed by this identity-only PR.

At 17:16 UTC the #170 Android Security Tests job `123591635794` also completed SUCCESS (Egress allow/deny JUnit verified). Thus on #170 exact source the Android Change Scope, Build/Lint/Instrumentation Compile, Unit (399/399), Integration, Security, and Dependency Audit jobs all pass. Android Storage/Emulator job `123606101694` remains QUEUED; run `40914655856` and other workflows remain incomplete. No API26/API35 acceptance follows from the completed static/hosted jobs.

### G10 Owner packet refresh — 2026-10-29 17:19 UTC

PR #167 remains OPEN/DRAFT, existing branch `codex/serea-g10-preflight-refresh`, now at head `ca49dc4ab4731b5820732d381a24a225ff5ae0c7` / tree `bf41428b2653dceae0a452cb65ad45e96c2e6c0a`; base is the existing PR #159 Owner/Release packet. The normal commit updates two Markdown files only. O-01..O-10 decisions are unchanged; it explicitly preserves Google-origin Cloud egress NOT_APPROVED, Gmail Restricted scope as a separate later gate, Pixel acceptance NOT_RUN/BLOCKED, model default UNDECIDED, and Personal Beta NO-GO. It refreshes CI evidence through #173 and #170, documents that `versionName` remains `0.1.0-demo` unless Beta overrides it (v4 target is `0.1.0`), and clarifies that Nov 10 may ratify OAuth project/scope plan but actual OAuth certificate binding must wait for the Owner-approved signing identity targeted for Nov 21. Debug/CI certificate fingerprints are not accepted as Beta OAuth identity. No actual OAuth/project, owner certificate, device, distribution, or external provider was touched.

PR #167 documentation validation: `validate_docs.py` passed 205 Markdown files; `git diff --check` passed. Independent review was PENDING at the initial checkpoint and is now ACCEPTED as recorded below. Exact-head CI runs: Android `40919807990`, Fast `40919808153`, Full `40919807655`, Native `40919807653`, SQLite `40919807740`, RustSec `40919807776`, Git Hooks `40919808032` were QUEUED at observation. Their skipped/queued gates are not product evidence.

Agent A independently reviewed the exact #167 diff and **ACCEPTED** at `ca49dc4…` / `bf41428…`, base #159 `982c8271698c5ea323e7a4d19f4dd662087db2c5`. Two Markdown files only, append-only, O-01..O-10 unchanged. Review confirms versionName discrepancy and OAuth/certificate sequencing are accurate, source-bound CI claims match canonical ledger `e514a96…` at capture, and runner inventory 403 is represented as unknown. Review found no boundary mismatch. New PR #167 CI remains queued as above.

## PR #169 review and runner update — 2026-10-29 16:03 UTC

Agent C completed the full exact-HEAD review: **ACCEPTED** at `87af6aab9c9db8f65e18ca18456256e03797ffb3` / tree `0fceca72877574acdd3cdbfa67b363dcabfb4dda`. Review confirms the diff from #166 is only Kotlin capability-bound validation, a boundary test, and report text; limits align with existing JNI checks, no silent clamp or lifecycle/security change; Native artifact allowlist stays sanitized; ordinary Chat remains disconnected.

Run `40907496496`, attempt 1, advanced: x86_64 job `123566146405` is IN_PROGRESS on runner `1000024660` at “Install pinned Android build and Emulator packages”; ARM64 software-guest job `123566148073` remains QUEUED. No APK, JUnit, model hash, JNI load, inference or runtime artifact exists yet. Android run `40907496673` Change Scope job `123566146896` remains QUEUED; other exact-head full/security/hooks/SQLite/contract jobs remain queued. PR #169 stays PARTIAL with x86_64 and ARM64 inference NOT_RUN.

## PR #169 active-job checkpoint — 2026-10-29 16:05 UTC

The exact-source Native x86_64 job `123566146405` on PR #169 has passed Android SDK/Emulator installation (step 5) and source checkout/CMake dirty-tree checks (steps 6–7), and remains IN_PROGRESS at focused JVM contracts (step 8). API35 emulator boot and inference steps have not run. ARM64 guest job `123566148073` and Android Change Scope job `123566146896` remain QUEUED. The active job-log endpoint returned HTTP 404 while the job was running; this is unavailable live log evidence, not a workflow failure. No retry or runner workaround was attempted.

## PR #169 focused JVM completion — 2026-10-29 16:06 UTC

On exact PR #169 source `87af6aab9c9db8f65e18ca18456256e03797ffb3` / tree `0fceca72877574acdd3cdbfa67b363dcabfb4dda`, Native x86_64 job `123566146405` completed the source-identity and focused JVM-contract step successfully. It is now running “Build opt-in product JNI APKs for both supported ABIs” (step 9). This is not a runtime result. ARM64 guest `123566148073` and Android Change Scope `123566146896` remain queued at this observation.

## Queue/build wait checkpoint — 2026-10-29 16:09 UTC

PR #169 native x86_64 job `123566146405` remains IN_PROGRESS at APK build step 9; the job-log download endpoint continues returning 404 while active. The workflow has not emitted a failure or artifact. ARM64 guest `123566148073` and Android Change Scope `123566146896` remain QUEUED. This is recorded as in-progress/queued infrastructure state, not as code failure or success. No retry was triggered.

# G9 Closure & ARM64 Native Runtime Campaign V4 — live checkpoint

Checkpoint observed 2026-10-29 16:47 UTC. Historical sections above preserve the G9R record; this section supersedes their status snapshots with current V4 evidence. Protected main `e0acb0254820c381dd97828824bef86a1db5176c` remains unchanged.

## Current PR and run authority

| PR | Exact source / tree | Base | State / current evidence |
|---|---|---|---|
| #162 G9 baseline | `6b7a741362b58b6bbc7728e69de90d929391dcaf` / `e61db67fcdf0ce7c5c5ab7e93efaef3bac4c1b5d` | #155 integration base | OPEN/DRAFT. API26/35 Required 105/105 PASS; Google/Gmail UI 16 tests with 2 failures per API on these historical bytes. |
| #168 Google/Gmail UI fix | `272f68ecf7f8072b812b13e46064a108bab5ef2b` / `010eca037f28f5d5dd5818ed99a2601852a719e9` | #162 exact head | OPEN/DRAFT. Run `40902585691` in progress at API35. API26 Required 105/105, Calendar 19/19, Chat 16/16, cursor 1/1, Google/Gmail UI 16/16 PASS, main Process Death and Data Isolation PASS. Separate Fake Google process-death harness fails after instrumentation because it cannot verify isolated target package path. Android Unit fails one test; see below. |
| #169 Native integration candidate | `87af6aab9c9db8f65e18ca18456256e03797ffb3` / `0fceca72877574acdd3cdbfa67b363dcabfb4dda` | G7 `0560e1e7bcd13f0194eab8caca89d70ea5a2f142` | OPEN/DRAFT. Native focused `40907496496` overall FAILURE due ARM host dependency; x86_64 runtime PASS. Android `40907496673` in progress at API35 after API26 run; Android Unit fails stale validator count. |
| #170 Identity validator fix | `fc9b355fb247bc6cd1eb35cd313379f6ed01a60f` / `01d27c481655c7b13c3ebda27190805b8aae22c3` | #169 exact head | OPEN/DRAFT. Agent A independently ACCEPTED test-only patch; Python validator tests 13/13 PASS. Android CI `40914655856` queued; Fast `40914655659` and RustSec `40914655648` in progress at observation. |
| ARM64 dependency follow-up branch | `8d4d3cdb44f44d31dc06299579f53427886c35c8` / `c88db8907fa7774bf1027a326bf9ddf7f6a09a36` | #169 exact head | Branch `codex/serea-arm64-libpulse-preflight`, pushed, no PR yet. Agent A focused checks PASS; independent Agent C review pending. |

## PR #168 API26 and Android Unit evidence

API26 instrumentation artifact `12914558157` (841,465 bytes), downloaded and ZIP digest verified as `sha256:a90dff71bef97f2675f5d3d0c1843b0f693c0474a1abf9a433d6b94bb8b23697`, binds to exact #168 source above. Required 105/105 passed with 0 failures/errors/skips. Google/Gmail UI passed 16/16 with 0 failures/errors/skips. Calendar 19/19, Chat 16/16, cursor 1/1, Data Isolation and the main Process Death status passed. API result is nevertheless FAIL because the separate Fake Google process-death host harness reports `Could not verify the isolated target package path.` Artifact `utp.0.log` confirms `uninstall_after_test: true`; the harness then expects `app.serea.android.isolated` to remain installed. This is a harness/package-state failure after the UI JUnit suite; no fake process-death success is claimed. Agent B is preparing a fail-closed exact-package verification/restoration fix. API35 remains in progress, not yet reported.

Android Unit artifact `12914261201` (30,930 bytes), SHA-256 `bd910af5d988780a386ec767e22caedf06e1bef99ee71a17951f252dfa49ad36`, has 408 tests, 1 failure, 0 errors/skips. `GoogleCalendarHttpReadPortTest#timeoutMapsToTimeout` expected TIMEOUT but received PERMISSION_DENIED at line 347. Agent B ran the exact focused test once locally and it passed; this does not establish the CI issue as environmental/flaky. Root cause remains under investigation. Timeout increase alone is not an accepted fix.

## PR #169 Android identity and API26 evidence

The Android Unit job `123571936579` on #169 ran Gradle tests successfully (artifact `12912983973`, 29,011 bytes, digest `sha256:6a9641e8896baa30605e2cf5ab2719cdfb1f580ff8e2f61ae3d8dffaaef8b943`; 399 tests, no JUnit failures/errors/skips), then the Python instrumentation verifier failed its stale assertion `15 != 17`. The non-required manifest contains 17 identities. Agent C's correction is on #170; Agent A independently reviewed and accepted exact #170 source. It verifies the fixed 103 Required + 2 Additional = 105, pins 17 Non-Required entries, rejects duplicate/malformed manifest identities, checks the three partitions do not overlap, and matches all 122 source `@Test` identities one-to-one. All three manifest files remain unchanged. Focused Python validator tests PASS 13/13. #169's failed Attempt remains historical evidence; only #170 can establish whether the correction is CI-successful.

PR #169 API26 artifact `12916130202`, size 723,089 bytes, digest `sha256:37bd4ede3df170742a39305ac1b0f2d19b7bc65606149b64fa34d97b2b2784e1`, confirms Required ran 105 tests but 4 failed during `MockWebServer.start` with `java.net.SocketException: socket failed: EACCES (Permission denied)` in four `ChatHttpE2ETest` identities. Calendar 19/19, Chat screen 16/16, cursor 1/1, Data Isolation and Process Death pass. API result FAIL. This is a separate Android loopback/variant/harness issue under Agent B's classification; do not blend it with the #168 UI result. API35 is in progress.

## PR #169 Native x86_64 and ARM64 evidence

Run `40907496496`, attempt 1, exact source `87af6aa…` / tree `0fceca7…`:

- x86_64 job `123566146405`: SUCCESS including both ABI APK packaging, Beta fail-closed packaging guards, exact test-runner/JUnit verification, pinned model hash, API35 x86_64 KVM emulator, two-turn Native E2E through local Egress/Usage, and cleanup.
- ARM64 software guest job `123566148073`: FAILURE before emulator boot/inference at pinned guest package verification. Runner is Ubuntu 24.04.5 `ubuntu-24.04` image `20261025.353.1`; Android API35 ARM64 guest packages installed, but `qemu-system-x86_64` could not load `libpulse.so.0` (exit 127). The emulator-version command did not complete, so ARM job emulator version is NOT_VERIFIED (workflow expected 37.3.6). ARM build packaging passed in the x86 job, but ARM64 runtime/inference is NOT_RUN.

Artifact digests match GitHub Actions metadata:

- Native build diagnostics `12911929532`, 6,308 bytes, `sha256:239256c3e44726c53d1b63e35ff4a5d5c12d0c75adeb61bc1039247bf97b62ca`.
- Sanitized runtime JUnit/metrics `12912718167`, 3,134 bytes, `sha256:bc2b783a33584af709725d97d06f917c0d38a783a9f3a9017b9f48a762e9b827`.
- ARM startup summary `12913138579`, 337 bytes, `sha256:8156244782389a8dd03781c5c66341e66b65c5f0f55b171cc4e9d26b9e6f4e09`; it records adb/log unavailable and raw diagnostics not retained.

The sanitized Native JUnit has No-op AndroidJUnit4 runner PASS and `LocalModelNativeRuntimeE2ETest#pinnedModelRunsThroughLocalModelEgressUsageAndNativeBackend` PASS: 2/2, 0 failures/errors/skips, instrumentation exit 0. Model revision `1076b85d135017971845b19b8d07df1479155988`, SHA-256 `e3131339bf4e8065265593d4fd8f7bb7ff2d3abff1edb5618aa1197b89cad9f5`, 101,016,128 bytes; llama.cpp SHA `276378515ceebee5d14bcc8536603dd5c87491a2`. API35 x86_64 emulator. Target APK SHA-256 `75c78ad5ae5467355fe10dc50f7e75f027432f4c858fc6e50aa96d771dc8ad3a`; AndroidTest APK SHA-256 `8953085feb6e7cf05a5b190727b44c1df2da7bfbdc8f1d488c10f6474b2319c5`. Signer not independently read from the APK.

Measured x86_64 PSS (observations, not a cap): load before 105,460 KiB, load after 218,184 KiB; inference samples 218,184/210,861 KiB and 211,576/214,098 KiB; main release 103,742 KiB; cancellation release 110,958 KiB; cancellation-to-release 18 ms. `pss_measurement_status=PASS` requires samples only. Model cleanup, app-private model deletion, package uninstall and raw-log removal passed; model weights were not uploaded. These are emulator measurements, not Pixel/ARM64 performance.

Agent A's follow-up branch installs `libpulse0` on the disposable runner and checks the emulator QEMU dependency closure with `ldd`, readable package-owned `libpulse.so.0`, regular SDK binary, and no symlink alias before emulator version/AVD commands. Focused contract/YAML/shell/diff checks pass. Agent C independent review is PENDING; no fix rerun exists yet.

## Issue ledger and current gates

- `G9C-UI-168-UNIT-TIMEOUT`: expected TIMEOUT, observed PERMISSION_DENIED on one 408-test Android Unit run; root cause unconfirmed; local focused run passed once.
- `G9C-UI-168-FAKE-PROCESS-DEATH`: separate host harness cannot find isolated package after UTP uninstall; repair pending.
- `G9C-IDENTITY-169`: stale assertion 15 vs manifest 17; #170 fix accepted independently, hosted validation pending.
- `G9C-ANDROID-169-LOOPBACK`: four Required `ChatHttpE2ETest` setup failures due `EACCES` binding MockWebServer on API26; classification/variant diagnosis pending.
- `G9C-ARM64-HOST-LIBS`: missing `libpulse.so.0` before emulator boot; fix branch pushed, independent review and CI pending.
- G8 Evidence Closure: PARTIAL.
- G9 Functional Candidate: PARTIAL.
- Native x86_64 Runtime: SUCCESS only for exact PR #169 source and its listed artifact evidence.
- Native ARM64 Runtime: NOT_RUN.
- Native Product Chat Integration: NOT_RUN; feature flag remains default OFF and no Chat composition is connected.
- Model Selection: UNDECIDED.
- G10 Preflight: PARTIAL.
- Personal Beta: NO-GO.

Owner decisions O-01..O-10 and all prohibitions remain unchanged. No real OAuth/account/data, Cloud credentials or billing, owner production key, physical Pixel, Beta distribution, main merge, Required gate weakening, or GO declaration.

## Resume actions

1. Agent C independently review and verdict Agent A branch `8d4d3c…`; only then open a Draft PR based on the exact #169 source/G7 workflow trigger and run one bounded Native retry.
2. Agent B finish API35 of #168 and #169, inspect exact artifact digests/JUnit, repair the process-death package-state harness and diagnose `timeoutMapsToTimeout` without padding timeouts. Classify the separate #169 `EACCES` failure.
3. Agent A review of #170 is ACCEPTED; wait for #170 exact CI and retain #169 failed history.
4. Integrate only independently accepted, exact-source-tested diffs into a new G9 candidate; freeze HEAD and run required formal CI.
5. After ARM64 runtime succeeds, prepare the ModelChatPort/Router feature-flag proposal; keep ordinary Chat disconnected until separate security and integration acceptance.

## Campaign V4 follow-up — 2026-10-29 16:49 UTC

A separate Draft PR #171 now contains Agent A's accepted ARM64 host-dependency fix, preserving #169 unchanged:

- PR #171 HEAD `8d4d3cdb44f44d31dc06299579f53427886c35c8`, tree `c88db8907fa7774bf1027a326bf9ddf7f6a09a36`, base G7 `0560e1e7bcd13f0194eab8caca89d70ea5a2f142`.
- Agent C independent review: ACCEPTED. Focused dependency-contract, upload allowlist, shell/YAML/diff checks PASS.
- New Native run `40915105551` and other hosted workflows are QUEUED; no fix-head rerun or inference evidence exists yet.

Latest exact CI state:

- PR #168 Android run `40902585691`: Storage/Emulator job `123577163290` remains in API35. API26 artifact `12914558157` was downloaded and its ZIP digest `a90dff71bef97f2675f5d3d0c1843b0f693c0474a1abf9a433d6b94bb8b23697` matched Actions. API26 Required 105/105, Google/Gmail UI 16/16, Calendar 19/19, Chat 16/16, cursor 1/1, Data Isolation and main Process Death pass. The separate Fake Google process-death host fixture fails after UTP uninstall; exact `utp.0.log` confirms `uninstall_after_test: true` and the script then cannot verify the isolated package. API35 still pending. Android Unit artifact `12914261201`, digest `bd910af5d988780a386ec767e22caedf06e1bef99ee71a17951f252dfa49ad36`: 408 tests, 1 failure, 0 errors/skips; `timeoutMapsToTimeout` observed PERMISSION_DENIED instead of TIMEOUT. Agent B's single local focused run passed but cause remains unconfirmed.
- PR #169 Android run `40907496673`: Android Unit artifact `12912983973` digest `6a9641e8896baa30605e2cf5ab2719cdfb1f580ff8e2f61ae3d8dffaaef8b943`, 399 tests, 0 JUnit failure/error/skip; downstream strict Python identity verifier failed 15 !=17. Agent C fix is PR #170, independently ACCEPTED by Agent A; exact CI is not complete. API26 artifact `12916130202`, digest `37bd4ede3df170742a39305ac1b0f2d19b7bc65606149b64fa34d97b2b2784e1`: Required 105 tests, 4 failures (`ChatHttpE2ETest` MockWebServer bind EACCES); Calendar, Chat screen, cursor, process death, data isolation pass. API35 still in progress.
- PR #170 run set for source `fc9b355…`: Fast `40914655659` PASS, RustSec `40914655648` PASS; Android `40914655856` and other jobs queued at observation.
- PR #171 run set for source `8d4d3cd…`: Native focused `40915105551`, Android `40915105462`, Fast `40915105468`, Full `40915105531`, RustSec `40915105436`, SQLite `40915105516`, Git Hooks `40915105479`, Native platform `40915105530`, and Mock Beta Contract `40915105451` are QUEUED.

PR #169 x86_64 evidence remains exact-source SUCCESS with artifacts and metrics already detailed above; ARM64 remains NOT_RUN until PR #171's bounded retry completes. PR #171 branch patch is statically accepted but hosted checks are pending.

No new Owner decision or external-service authorization. Personal Beta remains NO-GO.

## PR #172 UI/harness follow-up checkpoint — 2026-10-29 16:52 UTC

Agent B created Draft PR #172 at HEAD `28f52c2999795ab6d1c83aa456d5186205c8307a` / tree `e2c735be01ef14e22cd4736a04798c21a1d413f1`, based on exact #168 head `272f68ecf7f8072b812b13e46064a108bab5ef2b`. It changes Android unit test behavior plus the Fake Google process-death harness/regression test: when UTP removed the isolated APK after instrumentation, the harness revalidates the same APK SHA before reinstallation; timeout classification is exercised by deterministic SocketTimeoutException injection without widening the 100 ms network timeout or removing TIMEOUT assertions. Agent-reported local focused unit and harness regression checks pass, but independent C review is pending and no new-head Android/runtime evidence exists.

PR #172 workflow run set (`40915643093`, `40915642839`, `40915642806`, `40915643044`, `40915642846`, `40915642950`, `40915642857`) is QUEUED at this observation. PR #168 API35 job `123577163290` and PR #169 API35 job `123582802971` remain IN_PROGRESS in the API35 instrumentation step; no API35 JUnit artifact or result is available yet. #168 API26's Google/Gmail UI 16/16 pass remains exact-source evidence; the overall API26 result still fails the separate package-state harness. #169 API26 remains Required 4 EACCES failures as classified above.

PR #170 remains Draft exact head `fc9b355…`; Fast and RustSec PASS, Android and full cross-platform evidence pending. PR #171 remains Draft exact head `8d4d3cd…`; all new hosted jobs are queued. Agent A's Native fix has independent ACCEPTED review; it has not reached an emulator run yet. Agent C is preparing a separate strict solution for #169's Required `ChatHttpE2ETest` variant/permission mismatch; Agent A will independently review that branch.

## Review/checkpoint update — 2026-10-29 16:55 UTC

Agent A independently reviewed PR #172 exact head `28f52c2999795ab6d1c83aa456d5186205c8307a` / tree `e2c735be01ef14e22cd4736a04798c21a1d413f1` and returned **CHANGES_REQUESTED**. The fake ADB tests do not cover the missing-target → exact APK install → re-query/provenance path or install/hash mismatch failures; `set -e` can exit before status changes from NOT_RUN to FAIL on hash command errors. Agent B is fixing these fail-closed paths on a successor branch. PR #172 remains unaccepted and its CI queued.

Agent C's confirmed #169 EACCES root cause is Android variant configuration: the Native isolated target and androidTest manifests remove INTERNET; four Required ChatHttpE2ETest methods start MockWebServer and Android rejects bind with EACCES. #162 source inherited INTERNET and is not equivalent Native-offline evidence. C is developing a test-only variant/harness split preserving the complete 105 identity gate and keeping the Native isolated target without INTERNET; no branch/head yet. Agent A is assigned independent review when C publishes.

As of this checkpoint, #168 run `40902585691` job `123577163290` and #169 run `40907496673` job `123582802971` remain IN_PROGRESS at API35 instrumentation; API35 results are NOT_RUN until artifacts terminate. #171 run `40915105551` both Native jobs are QUEUED; no retry. #170 Android jobs remain queued. No new source change or terminal CI evidence is credited here.

## Campaign V4 continuation — 2026-10-29 17:22 UTC

### Exact-head review and CI update

- PR #173 is `540fd754e81c3788a76cc9452f754211e95d8378` / tree `f52c4104297edae8d84db72680de34136752f4a`, based on #172. Independent Agent C and Agent B reviews are **ACCEPTED**. The code review covers the cumulative #168 UI fixes and process-death harness repair. The reinstaller checks package absence, installs only the exact target APK, re-queries, and requires strict local/device 64-hex SHA-256 equality before recording provenance. Required identities, production composition, permissions, and egress are unchanged.
- Exact #173 Android run `40916731468`, attempt 1, is still QUEUED at Change Scope job `123603984767`; API26/35 acceptance on this head is **NOT_RUN**. Fast `40916731448` and RustSec `40916731566` PASS. Full, SQLite, Git Hooks, and Native remain queued. Local exact-head evidence: Android Unit 408/408, harness 34/34, focused process-death regression 1/1, shell syntax and diff checks PASS.
- PR #170 identity fix exact head `fc9b355fb247bc6cd1eb35cd313379f6ed01a60f` / tree `01d27c481655c7b13c3ebda27190805b8aae22c3`. Android Unit job `123591635633` on run `40914655856` PASS; uploaded JUnit artifact `12917461874`, 29,045 bytes, digest `sha256:c08ed0c9fd808cc947cf6eda25f352fbe92705f77bdf3b63debf0cbe5d8a3f6c`; 399 tests, 0 failures/errors/skips. Android Security Tests, Build/Lint/Instrumentation Compile, Integration, Dependency Audit, Fast `40914655659`, and RustSec `40914655648` PASS. Android Storage/Emulator job `123606101694` remains IN_PROGRESS in API26 instrumentation; API26/35 formal result is not yet terminal.
- PR #171 ARM64 dependency repair exact head `8d4d3cdb44f44d31dc06299579f53427886c35c8` / tree `c88db8907fa7774bf1027a326bf9ddf7f6a09a36`. Independent Agent A review: **ACCEPTED**. Dependency package checks, QEMU `ldd` closure, regular executable/no alias, readable package-owned `libpulse.so.0`, and failure cases are checked before Emulator startup. Native run `40915105551` remains QUEUED for x86_64 job `123593126707` and software-emulated ARM64 job `123593126899`; no ARM64 guest or inference evidence/artifact exists.
- PR #168 Android run `40902585691` is terminal: API26/35 each have Required 105/105 and Google/Gmail UI 16/16 PASS on exact #168; both API results still FAIL due the separate Fake Google process-death host harness failing after UTP uninstalls its target. Android Unit has 408 tests / 1 failure: `GoogleCalendarHttpReadPortTest#timeoutMapsToTimeout` got PERMISSION_DENIED instead of TIMEOUT. #173 contains the deterministic timeout classification test and exact-package harness repair, but new-head emulator evidence remains queued.

### Active Android Required remediation

The #169 API26 artifact established a separate four-test loopback failure: `ChatHttpE2ETest` cannot bind MockWebServer under the Native isolated offline manifest (`EACCES`). The Required list remains 103 + 2 = 105; none of those tests may be removed or skipped, and the offline Native variant must remain without INTERNET. Agent C is implementing an isolated HTTP test-only variant and a two-part execution/aggregation harness in `/workspace/serea-api26-http-split`. The working diff is not yet committed, has no GitHub Source SHA, and has not been independently reviewed or run on an emulator. A local run of `android_instrumentation_results_tests.py` on that worktree passed 16/16; this does not validate the full harness or Android runtime.

### Gate state / next actions

- API26/35 Required: #162 exact baseline passed; #168 exact head passed 105/105; #169 exact head had 4 HTTP test setup failures per API. New candidate acceptance is still pending.
- Google/Gmail UI: #168 exact-head UI suites passed 16/16 per API, but campaign result failed the separate process-death host fixture. #173 code is accepted; its formal Android run has not started.
- Native: x86_64 SmolLM2 two-turn inference remains SUCCESS only on #169's exact evidence. #171 host dependency patch is accepted, but its run is queued; ARM64 runtime/inference remains NOT_RUN.
- Product Chat Native remains disconnected; #165/#166 contain a default-OFF proposal only. Model selection remains UNDECIDED. G8/G9/G10 remain PARTIAL; Personal Beta remains NO-GO.
- Owner decisions and boundaries remain unchanged. No OAuth, real Google account/data, Cloud key/billing, production signing, physical Pixel, distribution, main merge, Required gate change, or GO declaration.
- Resume: complete and checkpoint Agent C's strict two-variant harness; independent Agent A review at its exact committed source/tree; use an exact-head bounded CI run. Continue #170 API26/35 and #173 Android results without repetitive retries, then selectively integrate only accepted/tested changes into a frozen G9 candidate.

## Additional exact-head reviews and artifact verification — 2026-10-29 17:27 UTC

- Agent B independently **ACCEPTED** PR #170's one-file identity-validator regression patch at `fc9b355fb247bc6cd1eb35cd313379f6ed01a60f` / tree `01d27c481655c7b13c3ebda27190805b8aae22c3`. It confirms the test keeps 103 Required + 2 Additional = 105, the 17 non-required entries disjoint, all 122 source test identities mapped exactly, and duplicate/missing/unlisted identities rejected. Android Unit run `40914655856` job `123591635633`: 399/399, 0 failures/errors/skips; artifact `12917461874`, 29,045 bytes, SHA-256 `c08ed0c9fd808cc947cf6eda25f352fbe92705f77bdf3b63debf0cbe5d8a3f6c`. API26 emulator job `123606101694` remains IN_PROGRESS; API26/35 runtime result is not terminal.
- Agent B independently **ACCEPTED** PR #173 at `540fd754e81c3788a76cc9452f754211e95d8378` / tree `f52c4104297edae8d84db72680de34136752f4a`; Agent C also independently accepted. UI/local evidence and run statuses remain as above. Its Android run `40916731468` is queued at Change Scope, so API26/35 are NOT_RUN on this head.
- API35 artifact `12916742164` from #168 run `40902585691` was downloaded directly. ZIP SHA-256 `e4e77ba4b5327b2bed8d2ad033146f54f7573a66d9517bc1b47435891d3302b7` matches GitHub Actions metadata and is bound to exact #168 source `272f68ecf7f8072b812b13e46064a108bab5ef2b`. JUnit instrumentation completed with no JUnit failure/error/skip, API35 Required 105/105 and Google/Gmail UI 16/16 pass; aggregate `api-result.txt=FAIL` solely because separate `google-demo-ui-status.txt=FAIL` after UTP uninstalled the target package.
- API35 artifact `12915723538` from #169 run `40907496673` was downloaded directly. ZIP SHA-256 `b21bd251ad68b1d6d3877f569c1bdbc185666b73d175c24c015712b526751bfa` matches GitHub Actions metadata and is bound to exact #169 source `87af6aab9c9db8f65e18ca18456256e03797ffb3`. JUnit has 141 tests, 4 failures, 0 errors/skips. All four failures are the same Required `ChatHttpE2ETest` loopback `MockWebServer.start()` identities listed under `G9C-ANDROID-169-LOOPBACK`; no UI failure is inferred from this source because the Required gate failed before UI.
- Agent A reviewed PR #166's Product Chat proposal and found planning gaps; no Chat source wiring exists and ordinary Chat remains Demo. Docs-only Draft PR #175 is based on exact #166 source `7c8bfb05db14e8fbab17db75f4027ae13fc3b87e` / tree `eb3559263d679facf5afaf8bcc16a8b7d6244520`, head `9cd9e4e0fcb132bdddcbfc9d0aa3c49e11f6b029`, tree `92f6e3d66d2cbb19f7e1adf028baebbb038312f1`. It now specifies lifecycle owner and unload, `onTrimMemory` synchronization/eviction, Router→Egress→Usage→adapter order, cancellation and outcome-unknown accounting, explicit user reload/error UX, no Cloud fallback, and ARM64/security acceptance gates. `validate_docs.py` (201 Markdown files) and `git diff --check` PASS. Independent review PENDING. It adds no runtime wiring.
- Parent review of #174 found the source map claimed #166 still had #165's old SHA/tree. The map and PR description now carry current source identity; Agent B accepted the corrected exact head as recorded below.

## Review completion — 2026-10-29 17:34 UTC

- PR #174 description now correctly binds its current head `db3ec5dd04f0756eb6806efaea664e033645e6de` / tree `340ed1f325bd7d7103b5a08a8dcef0ff115cd720` (base #167 `ca49dc4…`). Agent B re-reviewed the exact head and returned **ACCEPTED** after the description refresh; the earlier source-map finding is resolved.
- PR #175 documentation proposal was clarified and independently re-reviewed. Current head `b7e9a7a8a9c10905f0de2f2aaf1c1f41994bd985` / tree `09e6ce6731c085b1f9ea63cf67325f04472669aa`, base #166 `7c8bfb05db14e8fbab17db75f4027ae13fc3b87e`; Agent B **ACCEPTED**. It explicitly requires Usage reservation release for pre-dispatch hash/ABI/model-limit/load failure, or `OUTCOME_UNKNOWN` if durable release cannot be confirmed. No runtime source wiring is present.
- Agent C's offline/HTTP partition work remains uncommitted in `/workspace/serea-api26-http-split`. Full local harness suites are being rerun after the final exact-identity Gradle guard and installed-test-APK SHA/path verifier. The design now limits INTERNET-enabled connected runs to the exact four `ChatHttpE2ETest` identities, keeps the offline/native APK without INTERNET and JNI, and validates exact installed package paths/digests. No new GitHub SHA, independent review, or Emulator runtime evidence exists yet.
