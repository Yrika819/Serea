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
