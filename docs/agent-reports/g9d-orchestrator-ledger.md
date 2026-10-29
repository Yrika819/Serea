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

- At the previous checkpoint, PR #174 head `db3ec5dd04f0756eb6806efaea664e033645e6de` / tree `340ed1f325bd7d7103b5a08a8dcef0ff115cd720` had a matching description; Agent B accepted that exact revision after the source-map fix. This ledger update advances #174 to head `67db71b160352b7b4590f93cb3ec311b34b0821c` / tree `d761269eaea6ea5ea806be2f50f60e46774039e1`, based on #167 `ca49dc4…`; its PR description was refreshed to this source. Independent review of the updated 67db head is PENDING.
- PR #175 documentation proposal was clarified and independently re-reviewed. Current head `b7e9a7a8a9c10905f0de2f2aaf1c1f41994bd985` / tree `09e6ce6731c085b1f9ea63cf67325f04472669aa`, base #166 `7c8bfb05db14e8fbab17db75f4027ae13fc3b87e`; Agent B **ACCEPTED**. It explicitly requires Usage reservation release for pre-dispatch hash/ABI/model-limit/load failure, or `OUTCOME_UNKNOWN` if durable release cannot be confirmed. No runtime source wiring is present.
- Agent C's offline/HTTP partition work remains uncommitted in `/workspace/serea-api26-http-split`. Full local harness suites are being rerun after the final exact-identity Gradle guard and installed-test-APK SHA/path verifier. The design now limits INTERNET-enabled connected runs to the exact four `ChatHttpE2ETest` identities, keeps the offline/native APK without INTERNET and JNI, and validates exact installed package paths/digests. No new GitHub SHA, independent review, or Emulator runtime evidence exists yet.

## G9C implementation checkpoint — 2026-10-29 17:44 UTC

- PR #176 **OPEN/DRAFT**, based on PR #170 exact head `fc9b355fb247bc6cd1eb35cd313379f6ed01a60f` / tree `01d27c481655c7b13c3ebda27190805b8aae22c3`. Source `1523cddd7616873a465875c632bd11012689b50e`, tree `296c15faa83db059b6d6365eee7c7ba0b2163ef5`. Agent C authored; Agent B independently **ACCEPTED** this exact head.
- The split keeps the isolated offline target/test APK INTERNET-denied and JNI-free for 101 formal identities. A separately guarded `isolatedHttp` target/test build can execute only the exact four declared `ChatHttpE2ETest` identities; the HTTP APKs have INTERNET but no local llama JNI. Build flags cannot combine HTTP and Native modes. The formal verifier binds source SHA/tree, API level and build provenance, checks exact identities per partition, zero failures/errors/skips, and union=105. Package install and target/test APK provenance fail closed on malformed/multiple paths or missing/mismatched hashes; disposable packages are removed before the process-death stage.
- Exact clean-head local post-commit validation: `android_instrumentation_results_tests.py` 16/16 PASS; `android_emulator_workflow_tests.py` 32/32 PASS; shell syntax and `git diff --check` PASS. Offline and isolatedHttp target/test APK assembly and actual aapt permission/JNI verification PASS. APK SHA-256 values: offline target `ba404ca04f3254efb15771e04a7de5eda3a235dbaff25ec39dabc1ff3417f49f`; offline test `054197a505a564192487ea216cb12f6b9d8ebe3af0b1b274daa5ac14d72b9688`; HTTP target `f56265ce8661ef6f55b627cfe49dc001f7d07ac0d310f83e1c53634c043741da`; HTTP test `7046480b29960cb5f654de0fdd468b5c3d369e0ed314c75c26f7e03b483eeca4`.
- Hosted workflows are source-bound to #176 commit `1523cddd…`: Android `40922393033`, Fast `40922393018`, Full `40922393299`, SQLite `40922392970`, RustSec `40922393302`, Git Hooks `40922393053`, Native platform `40922393055`, and Mock Beta Contract `40922393277`. All are QUEUED at last check; Android Change Scope job `123618823187` is QUEUED. API26/API35 emulator execution is therefore NOT_RUN on #176.
- PR #174 current head `c4846b6ff9f28dafc54a1d68784f9e57cfe71d8a` / tree `8847d39be13f676c4f9091edf45dd968db09f713`, base #167 `ca49dc4…`; its description matches. Agent B **ACCEPTED** this exact ledger PR revision after the source-map timeline correction.
- #170 Android run `40914655856` remains IN_PROGRESS at API26 instrumentation job `123606101694`; API26/35 runtime results are not terminal. #171 Native run `40915105551` remains QUEUED for x86_64 job `123593126707` and ARM64 software guest job `123593126899`. #173 Android run `40916731468` remains QUEUED at Change Scope job `123603984767`. Do not transfer any earlier head's tests or inference results to these candidates.

## Native x86_64 evidence recheck — 2026-10-29 17:48 UTC

Agent A rechecked PR #169 Native run `40907496496`, attempt 1, exact source `87af6aab9c9db8f65e18ca18456256e03797ffb3` / tree `0fceca72877574acdd3cdbfa67b363dcabfb4dda`. x86_64 job `123566146405` SUCCESS; the overall workflow fails only because ARM64 job `123566148073` failed before Emulator startup in host package preflight. Downloaded runtime JUnit artifact `12912718167` (3,134 bytes), SHA-256 `bc2b783a33584af709725d97d06f917c0d38a783a9f3a9017b9f48a762e9b827`, matches Actions metadata. Its two exact identities pass: no-op AndroidJUnit4 runner initialization and `LocalModelNativeRuntimeE2ETest#pinnedModelRunsThroughLocalModelEgressUsageAndNativeBackend`; failures/errors/skips 0. Model revision `1076b85d135017971845b19b8d07df1479155988`, size 101,016,128 bytes, SHA-256 `e3131339bf4e8065265593d4fd8f7bb7ff2d3abff1edb5618aa1197b89cad9f5`; llama.cpp SHA `276378515ceebee5d14bcc8536603dd5c87491a2`.

The source-bound test exercises two generated turns through local Egress and Usage adapters, local authorization, denied INTERNET, Usage settlement/token attribution with unknown monetary cost, cancellation/release, reload/recovery, repeated close, and model/temp/package cleanup. Observed PSS (not an enforced cap): load 105460→218184 KiB, turns 218184→210861 and 211576→214098 KiB, normal release 103742 KiB, cancellation release 110958 KiB, cancellation-to-release 18 ms. Build artifact `12911929532` confirms ABI packaging, not ARM64 runtime. ARM startup artifact `12913138579` and skipped inference steps keep ARM64 Runtime **NOT_RUN**.

The artifact's `instrumentation_code=-1` is **not a failure**: Android SDK platform-35 defines `Activity.RESULT_OK=-1` and `RESULT_CANCELED=0` (verified with `javap`). This agrees with `adb_instrumentation_exit=0`, terminal per-test codes 0, and sanitized JUnit 2/2. This interpretation applies only to the listed #169 x86_64 run, not #176 or ARM64.
## Campaign V4 authority refresh — 2026-10-29 17:48 UTC

Protected `main` remains unchanged at `e0acb0254820c381dd97828824bef86a1db5176c` / tree `4955555be75c5742796740a3283d1a521e0d4806`.

### PR #168 exact-head Android result

PR #168 remains OPEN/DRAFT at `272f68ecf7f8072b812b13e46064a108bab5ef2b` / tree `010eca037f28f5d5dd5818ed99a2601852a719e9`. Android run `40902585691`, attempt 1, is terminal failure. Artifacts were downloaded and their ZIP hashes matched GitHub artifact digests:

- API26 artifact `12914558157`, 841,465 bytes, SHA-256 `a90dff71bef97f2675f5d3d0c1843b0f693c0474a1abf9a433d6b94bb8b23697`.
- API35 artifact `12916742164`, 3,849,637 bytes, SHA-256 `e4e77ba4b5327b2bed8d2ad033146f54f7573a66d9517bc1b47435891d3302b7`.

Both artifacts bind to the exact #168 source/tree above. On each API, the 105 Required+Additional identities started and finished (103 Required + 2 Additional); Required JUnit reports 0 failures/errors/skips. The separate Google/Gmail UI JUnit stage ran 16 tests with 0 failures/errors/skips on both APIs, including the two previously failing calendar identities. However, the API aggregate remains FAIL because the separate Fake Google process-death harness could not verify the isolated target package path (`Could not verify the isolated target package path`; `device-state-status=NOT_RUN`). Do not collapse this into a UI JUnit failure or claim aggregate Android acceptance. The process-death harness successor is PR #173 at `540fd754e81c3788a76cc9452f754211e95d8378` / tree `f52c4104297edae8d84db72680de34136752f4a`; its Android run `40916731468` has only Change Scope complete and executable Android jobs queued.

The same #168 run's Android Unit job `123567679297` failed. The separately identified `GoogleCalendarHttpReadPortTest#timeoutMapsToTimeout` remains a test/product classification issue under review; this does not invalidate the emulator JUnit counts. `api-result.txt` is FAIL on both APIs for the process-death aggregate described above.

### Identity, loopback isolation, and current Android candidate

PR #170 exact source `fc9b355fb247bc6cd1eb35cd313379f6ed01a60f` / tree `01d27c481655c7b13c3ebda27190805b8aae22c3` fixes the 15-vs-17 verifier expectation by validating the exact identity sets: 103 Required, 2 Additional, 17 non-required, 122 source tests, with no overlap. Android Unit job `123591635633` in run `40914655856` passed (399/399, 0 failures/errors/skips); the four separate loopback Chat HTTP identities failed when run under the intentionally offline APK (API26 artifact `12919376899`, digest `59656f37709dde9ea2a643b4deaf2280191283bbf4c3f01a37e3abc462d01483`). API35 in that run was still in progress at this checkpoint. Do not infer API35 or full 105 acceptance from the Android Unit result.

PR #176 (`isolatedHttp` variant split) is OPEN/DRAFT at `1523cddd7616873a465875c632bd11012689b50e` / tree `296c15faa83db059b6d6365eee7c7ba0b2163ef5`, based on #170. Agent B accepted static security review; Agent C review is pending. It keeps 101 identities offline and permits only the four canonical loopback identities on a distinct HTTP test variant, without Native JNI. Exact-head local contract/APK permission checks passed as previously recorded. Hosted Android run `40922393033` and Fast `40922393018`, Full `40922393299`, SQLite `40922392970`, RustSec `40922393302`, Git Hooks `40922393053`, Native `40922393055`, and Mock Beta Contract `40922393277` are queued; API26/35 runtime is NOT_RUN.

### ARM64 diagnostics and current Native evidence

PR #171's prior ARM64 guest attempt `40915105551`, ARM job `123593126899`, failed before guest startup at QEMU dependency preflight on Ubuntu 24.04 after `libpulse0` installation. The old checker emitted only a generic unresolved dependency failure, so missing package identity remains unconfirmed. No ARM64 inference ran.

PR #177 was independently reviewed and received CHANGES_REQUESTED at `84586731d29f36e86b5dd05098e382a734fb4b76` / tree `1605d1dc9074d2ad9c65e5a633605bc5af1d000c`: malformed unresolved `ldd` records could be silently ignored, and non-library tokens could be surfaced. Agent A fixed this normally on the same PR. Current PR #177 head is `e604f8fd1bd619f7996f329e5f2135630dc541aa` / tree `48ee16590ce997ac49b717bd0a485332b39899a7`. It accepts only validated `lib*.so[.N...]` basenames, fails closed on malformed unresolved lines with a generic sanitized message, and has regression fixtures proving no false pass or raw leakage. Focused shell/contract/allowlist/diff checks pass. Independent re-review is pending. Exact-head Native run `40924654435`, attempt 1: ARM guest job `123626806488` and x86_64 job `123626806729` are QUEUED; no new dependency names, emulator boot, ARM JNI, or ARM inference evidence is available.

The older PR #169 exact head `87af6aab9c9db8f65e18ca18456256e03797ffb3` has a verified x86_64 SmolLM2 two-turn result only on that source. Its ARM job failed before inference due the host QEMU dependency issue. Do not transfer x86_64 evidence to new source heads or ARM64.

### CI queue and disposition

Run `40915105551` x86_64 job `123593126707` is still in progress at Native APK build while its ARM64 guest job is terminal preflight failure. PR #170 run `40914655856` API35 emulator job `123606101694` is in progress; its current step is the API35 campaign, so no terminal API35 outcome is recorded here. Run `40916731468` Android jobs and #176 formal jobs remain queued. PR #177 exact-head general workflows `40924032792`, `40924033018`, `40924033024`, `40924032872`, `40924032724`, `40924033185`, and `40924032874` were queued on the preceding head; do not attribute them to the updated #177 source. The updated #177 Native run `40924654435` is the relevant source-bound run and remains queued.

- G8 Evidence Closure: PARTIAL.
- G9 Android/Product Acceptance: PARTIAL. #168 UI tests themselves pass 16/16 on both APIs, but process-death aggregate and successor formal acceptance are incomplete.
- Identity verifier: PR #170 Android Unit PASS at its exact source; its API26/35 product campaign is not closed.
- Native x86_64 inference: SUCCESS only on exact prior sources (#160/#169 as separately recorded); current integrated candidate remains unaccepted.
- Native ARM64 build/runtime: NOT_RUN; guest startup has not passed. Pixel 7a physical: NOT_RUN and prohibited without Owner authorization.
- Native Product Chat integration: NOT_RUN; approved documentation proposal only, feature flag default OFF, no normal Chat connection.
- Model selection: UNDECIDED. G10 Preflight: PARTIAL. Personal Beta: NO-GO.

Next checkpoint: obtain independent re-review and exact-head diagnostics for #177; finish inspecting #170 API35; wait for existing queued Android and Native runs without retry loops; review #173 and #176 terminal artifacts; then integrate only accepted source-bound changes into a new draft candidate and run its full formal CI. Preserve all Required gates and Owner boundaries.

## Campaign V4 review follow-up — 2026-10-29 17:50 UTC

- Agent B independently **ACCEPTED** PR #177 diagnostics at exact head `e604f8fd1bd619f7996f329e5f2135630dc541aa` / tree `48ee16590ce997ac49b717bd0a485332b39899a7`. It confirmed all `not found` lines are parsed, malformed records fail closed with generic withheld details, valid `lib*.so[.numeric-version]` names are sorted, and raw paths/addresses/output are not surfaced. Regression fixtures cover multiple missing libraries, malformed fields, and a non-`.so` token alongside a valid pulse dependency. Focused shell/contract/diff checks pass. Run `40924654435` still has both x86_64 job `123626806729` and ARM64 guest job `123626806488` QUEUED; no new runtime evidence.
- #168's exact API26/35 artifacts confirm the Google UI stage 16/16 JUnit pass on both APIs. The Android aggregate fails because the separate fake process-death helper cannot query the isolated target APK after UTP uninstalls it; the helper reports `Could not verify the isolated target package path`, and its device-state result is NOT_RUN. Other Process Death and Data Isolation statuses in those artifacts pass; they do not substitute for the failing fake-Google helper.
- #168 Android Unit artifact `12914261201` (SHA-256 `bd910af5d988780a386ec767e22caedf06e1bef99ee71a17951f252dfa49ad36`) records 408 tests, 1 failure: `GoogleCalendarHttpReadPortTest#timeoutMapsToTimeout` expected TIMEOUT but received PERMISSION_DENIED. On #170, that method passed with the original timer-based test; it is not validation of the deterministic change. PR #172 introduces an injected `SocketTimeoutException` and asserts the token request occurs while no server request occurs. PR #173 includes the follow-up harness fixes and is accepted statically by Agents B/C, but Android run `40916731468` has only Change Scope complete; Android Unit and emulator work remain QUEUED.
- #170 Android run `40914655856`, API26 artifact `12919376899` / SHA-256 `59656f37709dde9ea2a643b4deaf2280191283bbf4c3f01a37e3abc462d01483`, is a required-gate FAIL with 4 failures, 0 errors/skips. Exact identities are `ChatHttpE2ETest#twoTurnsIncludePersistedHistoryAndUsageThroughLoopbackHttp`, `#manualRouterSelectionIsPassedToMockHttpRequestAndNeverFallsBack`, `#directCompositionRoutesCatalogModelAndDenialPrecedesUsageReservation`, and `#innerEgressExpiryReleasesReservationBeforeAnyHttpOrCredentialAccess`; each fails at `MockWebServer.start()` with `SocketException: socket failed: EACCES`. This identifies the offline variant's loopback-bind incompatibility, not an identity inventory regression. #170 API35 job `123606101694` remains IN_PROGRESS at the full API35 instrumentation campaign; it has no terminal result at this checkpoint.
- Independent review of PR #176 at source `1523cddd7616873a465875c632bd11012689b50e` / tree `296c15faa83db059b6d6365eee7c7ba0b2163ef5` returned **CHANGES_REQUESTED** for timeout budget. Its three diagnostic stages and two formal partitions run serially, permitting 125 minutes per API (250 minutes total) before setup/cleanup, while API26/API35 campaign steps cap at 90/80 minutes and the Android job at 180 minutes. Timers do not overlap, so valid slow executions are truncated before evidence aggregation. Agent C is revising stage-specific budgets and adding a regression check; its new head/re-review are pending. The formal run on #176 remains NOT_RUN.

PR #168 UI acceptance remains source-bound with JUnit PASS but aggregate FAIL; API26 and API35 are never substituted for one another. Identity fix #170 is ACCEPTED and its Android Unit gate passes, while its emulator gate is still open. G9 remains PARTIAL and Personal Beta remains NO-GO.

## Native x86_64 recovery result — 2026-10-29 17:56 UTC

PR #171 exact source `8d4d3cdb44f44d31dc06299579f53427886c35c8` / tree `c88db8907fa7774bf1027a326bf9ddf7f6a09a36`; Native run `40915105551`, attempt 1:

- x86_64 Native job `123593126707`: **SUCCESS**. Runtime mode `X86_64_GUEST_KVM_ACCELERATED`, Android API 35, ABI `x86_64`. Directly downloaded JUnit/evidence artifact `12920216940`, 3,135 bytes, SHA-256 `3b3032fd574b2428754801847db992c12d38cf53e79f82b5f56c5d9ddf18dd44`, matches GitHub artifact digest. Two identities pass, 0 failures/errors/skips: `SereaNativeRunnerNoOpTest#androidJUnit4RunnerInstantiatesAndTargetsIsolatedApp` and `LocalModelNativeRuntimeE2ETest#pinnedModelRunsThroughLocalModelEgressUsageAndNativeBackend`. `adb_instrumentation_exit=0`, terminal statuses 0, initialization errors absent. Android's raw instrumentation `-1` is `Activity.RESULT_OK`, consistent with successful exit and JUnit.
- Model revision `1076b85d135017971845b19b8d07df1479155988`, size 101,016,128 bytes, SHA-256 `e3131339bf4e8065265593d4fd8f7bb7ff2d3abff1edb5618aa1197b89cad9f5`; llama.cpp revision `276378515ceebee5d14bcc8536603dd5c87491a2`. The artifact reports the same model hash in the app-private location. It exercises two turns, local Egress/Usage accounting, cancellation/release, reload/recovery, repeated close, and cleanup. Weights were not uploaded.
- Observed PSS (diagnostic only, not a hard cap): load 107,459→219,192 KiB; turn samples 218,269→218,274 and 218,006→214,427 KiB; normal release 104,487 KiB; cancellation release 111,108 KiB; cancellation-to-release 39 ms. Instrumentation exit 0, timeout not expired, cleanup PASS; model and temp files absent after cleanup.
- ARM64 guest job `123593126899`: **FAIL before emulator startup**. `libpulse0` installed, but QEMU dependency closure remained unresolved. ARM64 source build, guest boot, JNI load, and inference steps were skipped. Diagnostic artifact `12918702852`, 337 bytes, SHA-256 `8507918e7864dd865f7e6a33b86ef6e7aa556b21d805891dad6937ab0659eddf`, reported ADB/emulator logs unavailable and raw diagnostics not retained. This is a host/preflight failure, not an ARM64 inference failure. PR #177 addresses safe missing-library diagnosis; its new exact-source Native run remains queued.

This newly validates the x86_64 runtime only at PR #171's exact source. It does not establish ARM64 runtime, Pixel 7a acceptance, or native Chat integration. x86_64 Runtime: SUCCESS at this source; ARM64 Runtime: NOT_RUN; Product Chat remains disconnected/default-off.

The #176 Android runtime budget remains CHANGES_REQUESTED until a shared worst-case budget safely accounts for two serial API campaigns, emulator startup, and job/step limits. Agent C's patch and rerun are in progress.

## PR #176 bounded campaign successor — 2026-10-29 18:05 UTC

Agent C updated PR #176 normally. Current source `0810b877c772b51b32a41de27c4034af22d9158f` / tree `01bd708b795c26b784b42a29bfd25d25b113b9dc`, still based on #170 exact `fc9b355fb247bc6cd1eb35cd313379f6ed01a60f`. The offline/HTTP variant security split is retained. API26 has 47 minutes of inner command budget; API35 has 53 minutes including Update Compatibility. Two 15-minute emulator boot allowances and a 20-minute setup reserve fit inside a shared 150-minute deadline; each API step caps at 75 minutes and the job at 180 minutes, reserving 30 minutes after the shared deadline for artifact/cleanup work. All stage timeouts remain bounded; insufficient budget writes NOT_RUN and the aggregate gate fails closed.

Exact local validation reported by Agent C: Bash syntax for both API scripts and workflow YAML parse PASS; `git diff --check` PASS; `python3 -m unittest tests.android_emulator_workflow_tests tests.android_instrumentation_results_tests` 49/49 PASS. The previously identified stale process-death assertion was corrected. Independent Agent B re-review is PENDING. New source-bound runs: Android `40926772754`, Fast `40926772716`, Full `40926772550`, SQLite `40926772756`, RustSec `40926772697`, Git Hooks `40926772499`, Native `40926772773`, and Mock Beta Contract `40926773346`; all were QUEUED/PENDING at observation. API26/API35 runtime remains NOT_RUN. No model or APK artifact from this hosted revision is available.

The #176 formal `isolatedHttp` partition is still a candidate only; its added variant has not been exercised on an Android emulator. Keep G9 Android and Personal Beta gates PARTIAL/NO-GO until exact-source hosted artifacts and independent review are complete.

## Additional CI review findings — 2026-10-29 18:12 UTC

PR #170 API35 is now terminal. Run `40914655856`, artifact `12920536431` (2,217,520 bytes; digest/SHA-256 `0b48f177e8ebed583f5d672b64f829c1fc5f62d57eb44c89a1d9c36bb7aa5cd6`), binds to #170 source `fc9b355fb247bc6cd1eb35cd313379f6ed01a60f`. The artifact reports Required `66/103` started/finished and Additional `0/2`; the JUnit report recorded 62 of 105 expected identities, 5 failures, 0 errors/skips, one started-without-finish, and 7 Required identities not started. Four exact `ChatHttpE2ETest` loopback fixtures failed at `MockWebServer.start()` with `SocketException: EPERM`; the fifth is `SereaAppShellTest#chatDraftSurvivesGmailAndCalendarDestinationChanges`, whose JUnit failure element has no cause text. The test log says `Expected 105 tests, received 61`; timeout status is `NOT_TIMED_OUT`, device state shows no active instrumentation or target/test processes. Do not infer the fifth failure's root cause without new diagnostics. Process Death and Data Isolation statuses pass, but Required fails and the Google UI stage was not run. API26 artifact remains a separate failure with four EACCES bind errors. The #176 isolated HTTP variant is intended to address the four loopback permission failures; it does not yet explain or clear the fifth API35 incomplete test.

Independent Agent B review of PR #176 source `0810b877c772b51b32a41de27c4034af22d9158f` / tree `01bd708b795c26b784b42a29bfd25d25b113b9dc` returned **CHANGES_REQUESTED**: the deadline was exported from the `android-changes` job but consumed by the separate `android-storage-emulator` job, so `$GITHUB_ENV` cannot propagate it. Also the 150-minute budget omitted the wrappers' 180-second reserve. Agent C is moving the deadline to the emulator job and revising the total arithmetic; new source and re-review are pending.

Agent C independently reviewed PR #177 head `e604f8fd1bd619f7996f329e5f2135630dc541aa` / tree `48ee16590ce997ac49b717bd0a485332b39899a7` and returned **CHANGES_REQUESTED** for the upload-artifact contract parser: it can miss a valid YAML step that starts with `- id:` and later declares `uses: actions/upload-artifact`, allowing an unvalidated `with.path`. Agent A is updating the parser to inspect every list item and adding an id-first raw/glob negative fixture. PR #177's earlier Agent B acceptance of the diagnostics parser does not override this newer independent artifact-boundary finding; full PR review remains CHANGES_REQUESTED. No ARM64 inference occurred on either source.

## ARM64 diagnostic allowlist successor — 2026-10-29 18:15 UTC

Agent A patched the latest PR #177 artifact-boundary finding without force-push. Current source `08335a0a21f08a3f5061e84a1fb85cea435d18ae` / tree `6d31abef3a1b9dc559d054e4be163217b8a0b904`, based on the same G7/PR #171 lineage. The validator now parses every `steps:` sequence item regardless of its initial YAML key, including `id:`-first actions, and applies the upload-artifact path allowlist to nested `uses:` declarations. A regression fixture appends an id-first upload with raw log and wildcard paths and verifies rejection. Agent A reports shell syntax, host-library contract, allowlist contract, and diff checks PASS. Agent B exact-head independent review is PENDING.

Native focused run `40927164825`, attempt 1, is bound to this new source. x86 job `123635629580` and ARM guest job `123635629906` are QUEUED. No new missing-library names, ARM emulator boot, ARM build, JNI, or inference evidence is available. Previous #177 CI on `e604f8f…` is not transferred.

## V4 campaign continuation checkpoint — 2026-10-29

### Current candidate heads and CI

- PR #176 remains OPEN/DRAFT. Current head `d4232c9bfc64e0afd2927e9e66b3402ad55cfef7`, tree `05f589d749dba763b6f034a3ef97eb12109f2847`, based on #170 `fc9b355fb247bc6cd1eb35cd313379f6ed01a60f`. Exact-source Android run `40927824047` is PENDING and currently has no jobs assigned; therefore API26/API35 JUnit, UI, process-death, and identity execution are NOT_RUN on this head. Native platform `40927824153` is PENDING with no jobs assigned. SQLite `40927824092` and Fast `40927824379` are QUEUED; no outcomes are credited. Agent C's local checks on this exact source: workflow/instrumentation contracts 49/49 PASS, Bash syntax and workflow YAML parsing PASS, `git diff --check` PASS. Independent Agent B review is PENDING.
- PR #176 deadline correction is job-scoped in `android-storage-emulator`, after exact checkout and before SDK/emulator setup. Shared deadline is 170 minutes within the 180-minute job cap. Worst-case declared schedule is API26 51m + API35 58m + emulator boots 30m + setup 20m + wrapper grace 3m = 162m, leaving 8m under the deadline and 10m before job cap. Each API action caps at 75m. This is a bounded harness design assertion, not a hosted emulator result.
- PR #177 current head `08335a0a21f08a3f5061e84a1fb85cea435d18ae`, tree `6d31abef3a1b9dc559d054e4be163217b8a0b904`. Agent B independently ACCEPTED the static missing-library diagnostic and artifact-upload allowlist patch; Agent C independently ACCEPTED the `id:`-first upload parser correction. These are static CI guard reviews only. Exact-source Native run `40927164825`: x86_64 job `123635629580` is IN_PROGRESS during pinned Android package installation; ARM64 software guest job `123635629906` is QUEUED. No ARM guest boot, ARM64 library load, or inference result exists. Exact-source Android run `40927164875` has only queued Change Scope job `123638155770` at the latest check. There are no new artifacts.
- PR #170 identity correction remains independently ACCEPTED at exact source `fc9b355fb247bc6cd1eb35cd313379f6ed01a60f` / tree `01d27c481655c7b13c3ebda27190805b8aae22c3`; Android Unit run `40914655856`, job `123591635633`, passed 399/399 with zero failures/errors/skips (artifact `12917461874`, SHA-256 `c08ed0c9fd808cc947cf6eda25f352fbe92705f77bdf3b63debf0cbe5d8a3f6c`). This corrects the strict 17 nonrequired identity set and does not close emulator acceptance: API26 had four loopback bind failures; API35 had those same four plus one unfinished `SereaAppShellTest#chatDraftSurvivesGmailAndCalendarDestinationChanges`. The latter root cause remains unresolved.
- PR #168 exact-source UI evidence remains 16/16 on API26 and 16/16 on API35, with Required 105/105 on each. Its aggregate run `40902585691` still fails the separate fake-Google process-death helper after UTP removes the target package. Android Unit remains 407/408 due `GoogleCalendarHttpReadPortTest#timeoutMapsToTimeout` (TIMEOUT expected, PERMISSION_DENIED observed). Do not transfer these results to #176 or a later integration head.

### Current acceptance and integration boundary

- The #176 architecture retains offline/JNI-free instrumentation for 101 identities and a separately guarded INTERNET-enabled `isolatedHttp` variant for exactly four synthetic loopback identities, then enforces the unchanged 103 Required + 2 Additional = 105 exact identity gate. Until hosted API26/API35 JUnit and UI results exist, the remediation is a candidate only.
- Native SmolLM2 x86_64 two-turn inference is SUCCESS only on its recorded exact sources (#169/#171). ARM64 Runtime and Pixel 7a remain NOT_RUN. #177 has not yet booted an ARM64 guest.
- Native Product Chat wiring remains absent; documentation proposal stays default-OFF, no Cloud fallback. Standard model remains UNDECIDED.
- PR #176 needs Agent B's independent re-review, then terminal exact-source Android run/artifacts. PR #177 requires its currently running exact-source jobs to finish before any new inference statement. No older CI run or artifact is transferred to these heads.
- G8 Evidence Closure: PARTIAL. Android/G9 closure: PARTIAL. Native x86_64: SUCCESS only at exact sources; ARM64: NOT_RUN. G10: PARTIAL. Personal Beta: NO-GO.
- Owner boundaries and O-01 through O-10 decisions are unchanged. No main merge, OAuth activation, real Google data, Cloud credentials/billing, production signing, physical Pixel operation, Beta distribution, or Required-gate relaxation occurred.

## Independent deadline review and live run update — 2026-10-29

- Agent B independently **ACCEPTED** PR #176 exact head `d4232c9bfc64e0afd2927e9e66b3402ad55cfef7` / tree `05f589d749dba763b6f034a3ef97eb12109f2847`. Review confirmed the deadline is set inside `android-storage-emulator` after checkout verification and before Java/SDK/KVM/image/emulator setup; it is not written in the separate `android-changes` job. The tested budget is 51m API26 + 58m API35 + 30m boots + 20m setup + 3m wrapper grace = 162m under the 170m shared deadline and 180m job cap. API26 plus 15m step setup fits 66/75m; API35 plus 15m fits 73/75m. The 101 offline + 4 isolated HTTP identity split remains exact, source/build/API-bound, and strict 105 union; timeout partitions become NOT_RUN and cannot pass. The local 49-test, YAML, shell, and diff checks were confirmed.
- PR #176 hosted Android run `40927824047` still has no jobs assigned at the latest API query; API26/API35 execution remains NOT_RUN despite review acceptance.
- PR #177 run `40927164825`: x86_64 job `123635629580` remains IN_PROGRESS and has advanced to building opt-in product JNI APKs for both supported ABIs. ARM64 guest job `123635629906` failed in host preflight at pinned guest package installation/verification before AVD boot. After installing `libpulse0`, the QEMU dependency check still reported 23 unresolved emulator-bundled libraries (`libQt6*AndroidEmu`, `libandroid-emu-*`, `libc++.so`, `libprotobuf.so.28.4.0`, and related libraries); guest build and inference steps were skipped. Sanitized startup artifact `12921901164`, 337 bytes, is bound to this run and exact #177 head; it was directly downloaded and its SHA-256 `996cdce3eeb07d63d15d70c7e244d04fef6ca572c102152772da04a893386934` matches GitHub metadata. It contains only `adb_state=NOT_AVAILABLE`, `emulator_log_state=NOT_AVAILABLE`, and `raw_diagnostics_retained=false`. ARM64 inference remains NOT_RUN; no device boot or artifact containing weights is evidenced. This status supersedes prior queued/setup-java snapshots.

## Android queue and Native x86 progress — 2026-10-29

- PR #173 exact head `540fd754e81c3788a76cc9452f754211e95d8378` / tree `f52c4104297edae8d84db72680de34136752f4a`: Android Unit, Build/Lint/Instrumentation Compile, Dependency Audit, Integration, and Security jobs passed on run `40916731468`; Storage/Emulator job `123639690086` is QUEUED. API26/API35 UI/runtime evidence remains NOT_RUN on #173.
- PR #176 exact head `d4232c9bfc64e0afd2927e9e66b3402ad55cfef7` / tree `05f589d749dba763b6f034a3ef97eb12109f2847`: Android run `40927824047` has Change Scope job `123640791127` QUEUED. Emulator/API26/API35 steps have not started; no test results transfer from earlier sources.
- PR #177 Native run `40927164825`: x86_64 job `123635629580` is IN_PROGRESS at the pinned x86_64+ARM64 Native APK build step. ARM64 guest job `123635629906` is terminal FAILURE at host preflight before AVD boot, as detailed above. No x86 inference result yet.

## ARM64 SDK search-path successor — 2026-10-29

- PR #178 OPEN/DRAFT, now targeted at PR #177 branch `codex/serea-arm64-libpulse-diagnostics`; exact base `08335a0a21f08a3f5061e84a1fb85cea435d18ae`. Head `957d651a654accd4c445d9c6e2c69ec3fdc1633e`, tree `5465d6f196b7ee554310f8accd9c01e72f84efe9` (two shell files). It changes only the bounded host-library preflight and its contract test. The checker recreates the documented Android SDK emulator search context using canonical SDK-contained `emulator/lib64`, `lib64/qt/lib`, and QEMU binary directories for `ldd`; it keeps `libpulse0` package ownership validation and strict unresolved-name parsing, with no extra apt package and no symlink aliasing. Fake `ldd` fails unless the exact path is supplied.
- Focused local checks reported on this source: `bash -n`, host-library contract, native artifact allowlist, and `git diff --check` PASS. Agent B independently ACCEPTED the path containment, exact loader path, and fail-closed parser at this head. Agent C returned CHANGES_REQUESTED: the containment comparison treats the SDK root as a glob pattern, so metacharacters in the path could admit an outside symlink. Agent A is fixing this with literal path-boundary validation and a metacharacter/escape regression test. Exact-head Native run `40928932296`, attempt 1, has x86_64 and ARM64 guest jobs queued; no new emulator or inference result.
- This addresses the observed #177 `ldd` context failure as a candidate; it does not establish that emulator dependencies resolve, that an ARM64 AVD boots, or that ARM64 inference works. If the bounded SDK-path check cannot clear preflight, an official `ubuntu-24.04-arm` runner remains a possible follow-up only after its Android Emulator/KVM/NDK capabilities are verified.

## ARM64 literal-path fix and inference run progress — 2026-10-29

- PR #178 now has head `07da2596ba84c3e4caa23419696dee80418af744`, tree `ae22893beef1504c3bec2224e88912b7e9b130f8`, based on PR #177 exact source `08335a0a21f08a3f5061e84a1fb85cea435d18ae`. The symlink fixture now uses an `[unsafe]` SDK root and an outside target under `sdk/u/emulator/lib64`; it proves the old unquoted glob matches that target while the literal path-boundary check rejects it. Agent B independently ACCEPTED this exact source. Agent C re-review is pending. Local host-library fixture, Bash syntax, artifact allowlist, and diff checks PASS.
- Exact #178 run set is pending: Native `40929315005` QUEUED, Android `40929315004` PENDING, Full `40929314861` QUEUED, Fast `40929315073` QUEUED, SQLite `40929314870` QUEUED, RustSec `40929314846` PENDING, Git Hooks `40929314871` QUEUED. There is no ARM64 emulator result on this source yet.
- PR #177 run `40927164825`: x86_64 job `123635629580` has advanced to the exact product adapter / real x86_64 inference step (IN_PROGRESS). ARM64 job `123635629906` remains terminal host-preflight FAILURE before guest boot, with artifact `12921901164` already verified. No x86 inference result is credited until JUnit/artifact and job terminal evidence are available.

### PR #178 independent review correction

Agent C's review of head `07da2596ba84c3e4caa23419696dee80418af744` / tree `ae22893beef1504c3bec2224e88912b7e9b130f8` remains **CHANGES_REQUESTED**. Although the `[unsafe]` target itself matches the former glob pattern, the test's QEMU directory includes literal `[unsafe]` and fails the vulnerable check earlier, so it does not demonstrate that vulnerable code would admit the escape. C reproduced a bypass using an SDK root such as `/tmp/sdk/*/emulator` and sibling target `/tmp/sdk/u/emulator/lib64`, where both the qemu path and outside target match the old glob. Agent A is updating the fixture to that wildcard case; no #178 integration or ACCEPTED security review yet.

### PR #178 review closure and source precision — 2026-10-29

- PR #178 exact head `a7f5b407d400b611df2c5f9260eede9b62f41b59` / tree `9bb6f18000d841fc436b3caa6b20c92a329ed5ae`, based on #177 `08335a0a21f08a3f5061e84a1fb85cea435d18ae`, is independently **ACCEPTED by both Agents B and C** for the host-library preflight correction. The final fixture uses a literal `*` path component; it proves an unquoted glob prefix would admit both QEMU directory and sibling symlink target, then confirms literal-boundary validation rejects the resolved escape with sanitized output. Exact SDK library path, package-owned `libpulse0`, unresolved dependency handling, and artifact allowlist remain covered. Focused fixture, Bash syntax, and diff checks PASS.
- Precision: PR #177's earlier expression quoted the SDK-root variable; it did not itself have the demonstrated glob bypass. The #178 regression exercises the unsafe unquoted-pattern class and verifies the new literal path boundary rejects an escape. The change also mirrors the Android launcher’s documented library search context; hosted preflight remains necessary to confirm actual resolution.
- Exact #178 current-head CI remains queued/pending: Native `40929532184` QUEUED, Android `40929532167` PENDING, Fast `40929532339` QUEUED, Full `40929532259` QUEUED, SQLite `40929532201` QUEUED, RustSec `40929532142` QUEUED, Git Hooks `40929532140` QUEUED. No Android ARM guest execution evidence yet.
- PR #177 x86_64 job `123635629580` on run `40927164825` is IN_PROGRESS at the exact product adapter and real x86_64 inference step. ARM64 job `123635629906` remains a preflight failure, no AVD boot/inference. Do not transfer prior x86 inference evidence to this run until its exact JUnit/artifacts and terminal job result are verified.

### PR #177 x86_64 Native runtime completed — exact source only

Run `40927164825`, attempt 1, is terminal overall **FAILURE** because ARM64 guest job `123635629906` failed at preflight. The independent x86_64 API35 KVM guest job `123635629580` is **SUCCESS**. It built the supported JNI APKs, passed the exact product adapter inference step, uploaded evidence, and completed cleanup. This source-specific result does not change ARM64 `NOT_RUN`.

- Exact source/tree: `08335a0a21f08a3f5061e84a1fb85cea435d18ae` / `6d31abef3a1b9dc559d054e4be163217b8a0b904`.
- Downloaded JUnit/runtime artifact `12922127154`, 3,134 bytes, SHA-256 `413a20846deed81dc48c7d73e3990215c3446f22c453ae9bb006f7f8e720ad19`, exactly matches GitHub artifact metadata and is bound to this run/source. JUnit identities: `SereaNativeRunnerNoOpTest#androidJUnit4RunnerInstantiatesAndTargetsIsolatedApp` and `LocalModelNativeRuntimeE2ETest#pinnedModelRunsThroughLocalModelEgressUsageAndNativeBackend`; 2 tests, 0 failures/errors/skips, instrumentation exit 0, timeout not expired. The native test exercises the local Router/Egress/Usage path and two turns, cancellation/release/reload/recovery and double close (per run evidence); full raw runtime logs are not retained.
- Model revision `1076b85d135017971845b19b8d07df1479155988`, 101,016,128 bytes, SHA-256 `e3131339bf4e8065265593d4fd8f7bb7ff2d3abff1edb5618aa1197b89cad9f5`; llama.cpp `276378515ceebee5d14bcc8536603dd5c87491a2`. App-private model hash matches. No model weights were uploaded.
- Observed PSS (diagnostic metric, not a hard cap): load 106,421→217,977 KiB; inference samples 216,966→214,093 and 214,173→214,944 KiB; main release 104,740 KiB; cancellation-to-release 18 ms, cancelled release 110,601 KiB. Model/temp files and disposable packages were removed; cleanup PASS.
- Build diagnostic artifact `12922720184` (6,310 bytes; SHA-256 `e30500ca862d3fd0e1f4cc1759c5abe66058f8161ecf6f3112089829e690c5f9`) and the ARM startup artifact `12921901164` are separately source-bound to the same run.

### Native E2E artifact validation detail

Agent A completed exact-source verification for #177 run `40927164825`, attempt 1. The x86_64 job is SUCCESS; the run overall is FAILURE solely because its separate ARM64 guest job failed before boot. Direct artifact ZIP downloads were hash-verified against Actions metadata:

- Runtime artifact `12922127154`: 3,134 bytes, SHA-256 `413a20846deed81dc48c7d73e3990215c3446f22c453ae9bb006f7f8e720ad19`. JUnit is exactly 2/2 (no-op runner + `LocalModelNativeRuntimeE2ETest#pinnedModelRunsThroughLocalModelEgressUsageAndNativeBackend`), zero failures/errors/skips. Context 512, output bound 32, 2 worker threads. Two nonblank sequential turns use LOCAL_MODEL Egress → Usage → ModelChatPort/native backend; both reservations SETTLED, two usage rows, local-device attribution, first-turn token counts present, cost UNKNOWN. Target and test APKs deny INTERNET. Cancellation identity/sequence match, release within 15s (observed 18ms), double close, failed-load cleanup, backend regeneration, and post-dispatch cancellation OUTCOME_UNKNOWN are asserted.
- Model identity: `ggml-org/SmolLM2-135M-GGUF`, revision `1076b85d135017971845b19b8d07df1479155988`, size 101,016,128 bytes, SHA-256 `e3131339bf4e8065265593d4fd8f7bb7ff2d3abff1edb5618aa1197b89cad9f5`; llama.cpp `276378515ceebee5d14bcc8536603dd5c87491a2`.
- Build/JVM artifact `12922720184`: 6,310 bytes, SHA-256 `e30500ca862d3fd0e1f4cc1759c5abe66058f8161ecf6f3112089829e690c5f9`, direct download hash matches metadata. It confirms Android build PASS, ARM64 APK compile/package PASS (runtime NOT_RUN), Pixel NOT_RUN, LocalModelChatAdapterTest 24/24 and LocalModelNativeRuntimeTest 1/1. No APK file is uploaded.
- Runtime artifact observed PSS: load 106,421→217,977 KiB; inference samples 216,966→214,093 and 214,173→214,944 KiB; normal release 104,740 KiB; cancellation release 110,601 KiB; cancel-to-release 18ms. These are measurements, not enforced OS memory caps. Host, app-private, and device model files were absent after cleanup; both test and target packages were uninstalled; no model weights were uploaded.
- ARM guest artifact `12921901164` (337 bytes, SHA-256 `996cdce3eeb07d63d15d70c7e244d04fef6ca572c102152772da04a893386934`, directly downloaded and matched) contains only sanitized adb/emulator NOT_AVAILABLE flags. Host preflight found 23 unresolved Android SDK Qt/emulator libraries despite successful `libpulse0` installation; guest build/model/emulator/inference steps were skipped.

PR #178 at `a7f5b407d400b611df2c5f9260eede9b62f41b59` / tree `9bb6f18000d841fc436b3caa6b20c92a329ed5ae` has final independent ACCEPTED from Agents B and C. Its exact-source run set is queued/pending as listed above; no hosted ARM64 preflight result yet.

- Exact #177 runtime evidence reports target APK SHA-256 `7b7bd92b2807d3384e2f62bef9dc4df0c84a03ec260fc709c09b4f0b5015fed3` and instrumentation APK SHA-256 `5f1a4c5eb508af0e2529652cf465fe6d8e2dcefc69b4012eb1e99886e2e1c6cd`. APK files/signing certificate identity are not included in the sanitized evidence artifact; signer verification remains NOT_REPORTED.

### Independent Native security review

Agent C independently **ACCEPTED** PR #177's isolated Native diagnostic and exact x86 E2E contract at `08335a0a21f08a3f5061e84a1fb85cea435d18ae` / tree `6d31abef3a1b9dc559d054e4be163217b8a0b904`, with no findings requiring changes. Review covered serialized process lease/generation, JNI exception containment and RAII, cancellation identity and deferred release, pinned model/llama identity plus size/hash checks, context/output/thread limits, distinction between file-size bounds and observed PSS, Egress/Usage composition and local accounting, offline APK permissions, cleanup and artifact sanitization. The finding is scoped to this isolated diagnostic and x86 evidence; ARM64 remains NOT_RUN and is not accepted by this review.

# G9D Formal Closure & ARM64 Execution — V5 Checkpoint (2026-10-29)

## Authority recheck

- Protected `main` still points to `e0acb0254820c381dd97828824bef86a1db5176c`; local tree `4955555be75c5742796740a3283d1a521e0d4806`. No changes made to main.
- PR #173 head `540fd754e81c3788a76cc9452f754211e95d8378`, tree `f52c4104297edae8d84db72680de34136752f4a`; Android run `40916731468`, attempt 1, terminal FAILURE.
- PR #176 head `d4232c9bfc64e0afd2927e9e66b3402ad55cfef7`, tree `05f589d749dba763b6f034a3ef97eb12109f2847`; Android run `40927824047`, terminal FAILURE.
- PR #177 head `08335a0a21f08a3f5061e84a1fb85cea435d18ae`, tree `6d31abef3a1b9dc559d054e4be163217b8a0b904`; Native run `40927164825`, terminal overall FAILURE: x86_64 job PASS, ARM guest host preflight FAIL before boot.
- PR #178 head `a7f5b407d400b611df2c5f9260eede9b62f41b59`, tree `9bb6f18000d841fc436b3caa6b20c92a329ed5ae`; Android run `40929532167` has no substantive Android test jobs (only scope/gate bookkeeping; product Android jobs skipped). Native platform run `40929532184`, terminal FAILURE in Windows x86_64. This Rust validation is not Android ARM64 runtime evidence.
- PR #174 checkpoint base `479a6653ffa102bc49aafa6d567dc06ba181e52d`, tree `9c850d9948b64249f629118555f41e4e8b0fb3ea`.

## PR #173 Android artifacts: exact result, correcting the supplied baseline

Artifacts are from exact #173 source `540fd754e81c3788a76cc9452f754211e95d8378` / tree `f52c4104297edae8d84db72680de34136752f4a`; ZIP hashes match Actions metadata.

- API26 artifact `12923566730`, 841,228 bytes, SHA-256 `a2eff7aae0cd7ea1539cc55be2fdf61fe0dfc8cd17681b8466f6820c97e92dfd`. Required 105/105 PASS; Google Authorization Demo UI 16/16 PASS; Process Death PASS; Data Isolation PASS. Aggregate API26 FAIL because separate Fake Google process-death helper failed: `/system/bin/sh: serea-pid-query: not found`; it could not prove the isolated instrumentation process absent. Sentinel/helper remains a distinct gate; do not treat its failure as a JUnit UI failure.
- API35 artifact `12925950038`, 3,447,484 bytes, SHA-256 `16f76cda71054289a4ddcec0d9be3bbe5fd89a82fbe80c79b3cf0cb8bca519e6`. Required suite ran all 105 identities; 104 pass, 1 fails: `ChatScreenTest#emptySoftLimitFieldShowsValidationAndDisablesSave` at `ChatScreenTest.kt:647`, `assertIsDisplayed()` reports component not displayed. Process Death PASS; Data Isolation PASS. `google-demo-ui-status=NOT_RUN`; API35 Google/Gmail UI 16/16 is not evidenced by this artifact. A separate API35 ChatScreen stage has 16/16 PASS, which is not the Google/Gmail UI suite. No API35 IME gate result is inferred.
- API26/35 artifact identities and exact package digests are bound to this source. Artifact contents have been directly downloaded and inspected; API26/35 results are not transferred to later heads.
- Other jobs on #173 run: Unit, Build/Lint/Instrumentation Compile, Dependency Audit, Integration, Android Security all PASS; Android Documentation SKIPPED. No G9 Android acceptance because API26 aggregate and API35 Required fail.

## PR #176 shell failure

Run `40927824047` is terminal FAILURE. API26 and API35 instrumentation artifacts are only 673 bytes each and contain no executed test report. Both emulator steps failed before tests with `/usr/bin/sh: 1: set: Illegal option -o pipefail`: the emulator action invoked the multiline `script:` through `/bin/sh`, while the script uses Bash-only `pipefail`/arithmetic. Product instrumentation on PR #176 is NOT_RUN. Preserve the 101 offline + 4 HTTP partition, 103 Required + 2 Additional = 105, deadline, evidence and provenance checks. Agent B owns a bounded shell-entrypoint fix and regressions for shell, environment propagation, deadline exhaustion, exit propagation, identity partition and permissions.

## PR #178 Windows failure

Native platform run `40929532184`, attempt 1, had 11/12 jobs PASS and Stable Windows x86_64 job `123643969841` FAIL. In `crates/serea-host/tests/host_lifecycle.rs:750`, `actual_host_model_policy_and_accounting_scenarios_survive_database_reopen` failed with `Host timeout scenario failed: stdout= stderr=`; 13/14 host lifecycle tests passed. Classification remains UNDER_INVESTIGATION until Agent C inspects child process creation, pipes, timeout state, and exit status. Do not label this a flake or raise timeouts without evidence.

## Native status carried forward

- #177 API35 x86_64 Native: SUCCESS on its exact source only; 2/2 JUnit identities; model, Runtime/Build artifact hashes and metrics are in this ledger's preceding G9C sections. ARM64 Runtime remains NOT_RUN.
- #178 SDK `LD_LIBRARY_PATH` correction is independently ACCEPTED by Agents B/C, but exact ARM guest workflow execution is unproven. Its Native Platform run is a Rust cross-platform matrix, not Android emulator inference.
- Product Chat stays disconnected/default OFF. Model selection remains UNDECIDED. Pixel 7a physical acceptance is NOT_RUN and remains owner-prohibited.

## Agent lanes and next actions

- Agent A: inspect/implement a legitimate ARM64 focused-workflow trigger path without weakening workflow permissions/required gates; execute the exact #178 SDK-path preflight, guest boot, then ARM64 inference only if all preceding gates pass.
- Agent B: use the exact API35 artifact to diagnose the hidden Soft Limit validation component and preserve the assertion; repair the PR #176 Bash `/bin/sh` mismatch in a new branch and preserve every identity/boundary. Then rerun API26/35 on one exact integrated SHA.
- Agent C: diagnose the Windows child-process timeout from run `40929532184`, classify with evidence, implement the minimum fix, and independently review A/B changes. Keep O-01..O-10 unchanged.
- Parent: select only independently accepted changes for a new G9D integration candidate; do not integrate until API26/API35 Required, UI, and identity-bound process-death evidence are available on the same HEAD. Continue G10 docs-only work where independent; no Owner approval boundaries changed.

Current gates: G8 PARTIAL; G9 Android PARTIAL; x86_64 Native SUCCESS only at PR #177 source; ARM64 Native NOT_RUN; Product Chat Native NOT_RUN; Model Selection UNDECIDED; G10 PARTIAL; Personal Beta NO-GO.

## V5 live execution update — 2026-10-29

### New ARM64 focused trigger PR

Agent A found `.github/workflows/g8-local-native-runtime-focused.yml` only listens to PR base `codex/serea-g7-calendar-readiness` plus Native paths. PR #178 was based on #177, so it never triggered that focused workflow. To preserve #177/#178 and the workflow's existing trigger/security conditions, Agent A opened Draft PR #179 with the same accepted #178 head `a7f5b407d400b611df2c5f9260eede9b62f41b59` / tree `9bb6f18000d841fc436b3caa6b20c92a329ed5ae`, base G7 `0560e1e7bcd13f0194eab8caca89d70ea5a2f142`. Exact focused run `40967241879` is now IN_PROGRESS; ARM guest job `123774725655` and x86 job `123774726124` are installing packages. No host preflight, emulator boot, ARM runtime, or inference result yet. PR #179 is a trigger-path candidate, not acceptance evidence. It is based on a previously reviewed code head; no Required workflow conditions were weakened.

### Windows run detail

Agent C has confirmed Run `40929532184` failed in Stable Windows x86_64 job `123643969841`, command `cargo +stable test --workspace --all-features --locked`. In `host_lifecycle.rs`, 13/14 tests passed and the timeout scenario child returned non-success with empty stdout/stderr; the assertion at line 750 drops exit-status details, so root cause remains unknown. Classification: Windows-specific TEST_FAILURE (not yet root-caused), not infrastructure. Agent C is adding bounded child status diagnostics and running a focused Linux repro without extending the timeout.

### Agent execution

- Agent A: ARM64 workflow trigger discovery; PR #179 opened. Continue bounded focused run `40967241879`; analyze SDK loader paths and execute ARM64 only if preflight/boot succeed.
- Agent B: Android G9 lane; directly audited #173 artifacts and is fixing API35 Soft Limit displayed-state failure plus #176 `/bin/sh` vs Bash script invocation in an isolated branch. Keep Required and 101+4 partition unchanged. #173 Android artifact facts above supersede the supplied claim that API35 Google/Gmail UI 16/16 passed: that suite is `NOT_RUN` in the exact artifact; only the focused ChatScreen stage is 16/16.
- Agent C: Windows host timeout test and independent security/release lane. Windows cause is under investigation; no timeout increase or accounting contract change is allowed.

Latest aggregate: G9 Android remains PARTIAL; #173 API26 Required/UI pass but its fake Google process-death host helper fails; API35 has one Required failure and Google UI NOT_RUN. #176 partition workflow fails in the `/bin/sh` launch path, with instrumentation NOT_RUN. Native x86_64 remains SUCCESS only on #177 exact source; ARM64 remains NOT_RUN while the accepted #178 loader-path fix is tested in PR #179. Windows host validation is PARTIAL due one unresolved timeout test. G10 PARTIAL; Product Chat disconnected/default OFF; model UNDECIDED; Personal Beta NO-GO.

## Focused ARM trigger execution

- PR #179 is a new OPEN/DRAFT trigger candidate with the already accepted #178 head `a7f5b407d400b611df2c5f9260eede9b62f41b59` / tree `9bb6f18000d841fc436b3caa6b20c92a329ed5ae`, based on configured G7 branch head `0560e1e7bcd13f0194eab8caca89d70ea5a2f142`. It exists because #178's base #177 did not satisfy the G8 Native workflow's pull-request base filter. No workflow trigger condition or permission was loosened.
- Exact G8 focused run `40967241879` is IN_PROGRESS. ARM guest job `123774725655` is installing pinned API35 ARM64 guest packages; x86 job `123774726124` is at focused JVM contracts. No ARM host preflight/AVD boot/inference result is available at this snapshot. Keep the run source-bound to #178 SHA/tree; PR #179 is not a product integration candidate.
- Agent C's Windows diagnosis on #178 run `40929532184` remains under investigation. Planned diagnostic adds child process exit status and bounded output byte counts without changing the timeout or policy behavior. The #173 API26/API35 JUnit artifacts and #176 `/usr/bin/sh` launch failure remain as recorded above.

### #179 ARM64 preflight result — library search path fixed, version guard mismatch

- Focused run `40967241879` attempt 1 is currently overall pending/in progress. ARM guest job `123774725655` reached and **passed** `emulator_host_preflight`: `unresolved_libraries=0`, `libpulse=package_owned`. This validates PR #178's SDK library search path at the exact tested source, with the prior 23 SDK-bundled libraries resolved.
- The same ARM job then failed before AVD boot because its binary-version assertion expected `37.3.6`, while the installed emulator reports `37.2.13.0 (build_id 16740729)`. This is a focused workflow pin/verification mismatch, not an ARM inference failure. No emulator boot, ADB connection, ARM APK install, JNI load, model load, or inference occurred.
- Startup artifact `12939225225`, 337 bytes, exact head `a7f5b407d400b611df2c5f9260eede9b62f41b59`, digest `sha256:16ac75a9cd396bdae0a4a60d7bd824160fbe18ee7cf36a389182c59888a50e83`; directly downloaded and verified. It contains only `adb_state=NOT_AVAILABLE`, `emulator_log_state=NOT_AVAILABLE`, `raw_diagnostics_retained=false`.
- x86 job `123774726124` in the same focused run remains IN_PROGRESS at source/JVM contracts. Do not call the run terminal until it completes.
- Agent A is checking how `sdkmanager` resolved the emulator package and will pin/verify its actual version from package provenance; no blind version bump or retry of unchanged source.

## Windows diagnostic checkpoint

- Agent C opened Draft PR #181 from the exact #178 source. Current head `a2cd5bf863670aa22cff7b82f4eb0ad9e468150f`, tree `c86de2ed7454dbef7eeb86fa7bba8e11280145f8`, base #178 `a7f5b407d400b611df2c5f9260eede9b62f41b59`. It adds child ExitStatus/code and bounded stdout/stderr byte counts to the host timeout assertion; timeout duration and Host Policy/Usage semantics are unchanged. Linux focused compound test passed 1/1; Windows root cause remains UNKNOWN until a new diagnostic Windows run.
- Exact #181 run set is queued: Native `40967593381`, Android `40967593082`, Full `40967593089`, Fast `40967593152`, SQLite `40967593097`, RustSec `40967593083`, Git Hooks `40967593404`. Agent B has been asked for independent review; result pending.

### PR #181 review outcome

Agent B independently **ACCEPTED** PR #181's diagnostic-only patch at `a2cd5bf863670aa22cff7b82f4eb0ad9e468150f` / tree `c86de2ed7454dbef7eeb86fa7bba8e11280145f8`. It only expands the failed-child assertion to include ExitStatus, numeric exit code, and bounded stdout/stderr byte counts. Timeout constants, process launch, Host Policy/Usage, scenarios, and assertions are unchanged. This makes a future Windows failure diagnosable but does not establish root cause. Windows rerun remains required; exact-head workflows are queued.

## Focused-fix successors opened

- PR #182: stable Android Emulator package verification for the ARM64 focused run. Head `3ed787d0e4bf678d92a21117e57e81c9b3d0de31`, tree `75fc296f788c51198a3ad02ae2c06028724ced98`, base G7 `0560e1e7bcd13f0194eab8caca89d70ea5a2f142`. Agent A pinned stable SDK channel 0 and verifies package `Pkg.Revision=37.2.13`, emulator `37.2.13.0`, Linux build id `16740729`, and archive SHA1 `f41f9e31eb5ca64926603c03265c50bbc0ff41792` using a bounded tested helper and sanitized evidence. Focused local shell/version/library/YAML/allowlist checks pass. Focused G8 run `40967790559` is QUEUED (ARM job `123776591261`, x86 `123776591543`); no emulator result. Independent Agent C review pending.
- PR #183: API35 Soft Limit validation visibility test fix only. Head `681f28e2c434c8571a7cab9354cafd6f92e417aa`, tree `e6011c2887576fd8ae974032061e633b407ebe36`, base #173 `540fd754e81c3788a76cc9452f754211e95d8378`. Retains visible validation and disabled Save assertions, dismisses IME and waits boundedly for hidden insets. Local compile/UI test NOT_RUN (JDK 21 lacks javac; no local emulator/KVM). Exact Android run `40967821758` and other checks are QUEUED; no hosted API35 result. Independent Agent A review pending.
- PR #181 diagnostic-only patch is independently ACCEPTED by Agent B; Windows root cause remains unknown until the Windows job reruns. Native run `40967593381` Windows x86_64 job `123775922557` QUEUED at last check; Stable Windows ARM64 was IN_PROGRESS. Do not call diagnostic text a root cause.

### PR #183 API35 Soft Limit test review

Agent A independently **ACCEPTED** PR #183 at exact head `681f28e2c434c8571a7cab9354cafd6f92e417aa` / tree `e6011c2887576fd8ae974032061e633b407ebe36`, based on #173 exact source. The test clears input, explicitly closes numeric IME, uses a bounded 5-second wait for IME insets hidden, then retains `performScrollTo().assertIsDisplayed()` on the validation message and the Save-disabled assertion. No sleeps, assertion removal, identity/gate changes, product behavior changes, or dependency changes. Hosted compile/UI test is queued; review is limited to source/test semantics, not acceptance.

Agent B is separately diagnosing the #173 API26 Fake Google process-death helper failure (`sh -c` missing command and `serea-pid-query` not found) and the #176 `/bin/sh` Bash entrypoint failure; these are distinct from API26 product Process Death PASS and Data Isolation PASS. Required/UI/identity evidence remains source-bound.

### PR #182 independent review

Agent C independently **ACCEPTED** PR #182 at exact head `3ed787d0e4bf678d92a21117e57e81c9b3d0de31` / tree `75fc296f788c51198a3ad02ae2c06028724ced98`, based on G7 trigger branch. Review confirmed the only diff from #178 is the focused workflow, package-version verifier, and its test. The helper rejects symlink/missing/duplicate/altered package metadata, requires `Pkg.Path`/`Pkg.Revision` exact match, checks emulator version and build ID, and emits sanitized fields. Artifact allowlist is exact and model weights/raw emulator logs are not uploaded. Local version contract, Bash syntax, YAML, upload allowlist, and diff checks PASS. Independent review confirms official `repository2-1.xml` stable mapping: revision `37.2.13` → `emulator-linux_x64-16740729.zip`, archive size 349,658,551 bytes, SHA1 `f41f9e31eb5ca64926603c03265c50bbc0ff41792`. The workflow comment carries this archive identity; sdkmanager verifies package acquisition. Exact focused CI `40967790559` is still QUEUED, so no hosted ARM boot or inference is accepted.
