# G9E V6 Orchestrator Ledger

Checkpoint: 2026-10-30 UTC. Reconfirm the exact Source SHA and Tree before accepting any later run or artifact. This ledger is append-only for campaign evidence; a historical result is never transferred to another HEAD.

## Authority and current status

| Lane | Authority / exact source | Evidence at this checkpoint | Status |
|---|---|---|---|
| Protected `main` | `e0acb0254820c381dd97828824bef86a1db5176c` | Unchanged; no merge | Protected |
| G9 integration | PR #187, head `68dd17e687b80bdad0134099077a886fca4de836`, tree `c262ea25c1241e6c1b9ecd885383143886236fb6`; based on PR #173 `540fd754e81c3788a76cc9452f754211e95d8378` | Android run `40970247739` terminal FAILURE. Required/UI/process/data results below are exact to this source. | PARTIAL |
| ARM64 Native | PR #182, head `3ed787d0e4bf678d92a21117e57e81c9b3d0de31`, tree `75fc296f788c51198a3ad02ae2c06028724ced98`; focused run `40967790559` | x86_64 adapter/inference SUCCESS; ARM64 software guest failed at bounded boot/inference step with ADB unavailable; no guest/JNI/model evidence | BLOCKED / NOT_RUN |
| Native x86_64 baseline | PR #179 exact source `a7f5b407d400b611df2c5f9260eede9b62f41b59`, tree `9bb6f18000d841fc436b3caa6b20c92a329ed5ae`; run `40967241879` | x86 adapter and real SmolLM2 inference job SUCCESS; source-bound, not ARM64 | SUCCESS for x86_64 only |
| Windows Native Platform | Run `40967593381`; diagnostic PR #181 head `a2cd5bf863670aa22cff7b82f4eb0ad9e468150f`, tree `c86de2ed7454dbef7eeb86fa7bba8e11280145f8` | Current Windows rerun ended SUCCESS across jobs; original empty-output timeout cause remains undetermined | CI recovered; root cause unresolved |
| Native Chat | PR #175 proposal + PR #185 consent clarification, latest #185 `4db504fc718d1c062c97a4418b005693aa670ad9`, tree `e00f5a8d042138e55cd00b4a620e03dd845591a0` | Reviewed design only; product adapter/route/flag wiring is not accepted or enabled | NOT_RUN |

G8 Evidence Closure: PARTIAL. G9 Android Formal Closure: PARTIAL. Native x86_64: SUCCESS for the cited source only. ARM64 Runtime: NOT_RUN. Product Chat Native integration: NOT_RUN. Model selection: UNDECIDED. G10: PARTIAL. Personal Beta: NO-GO.

## PR #187 exact Android run and artifacts

Run `40970247739` was fetched from GitHub on 2026-10-30. Checked out source SHA `68dd17e687b80bdad0134099077a886fca4de836` (the PR's recorded tree is `c262ea25c1241e6c1b9ecd885383143886236fb6`). Android Change Scope, Android Unit, Integration, Security, Build/Lint/Instrumentation Compile, and Dependency Audit jobs passed. The required emulator job failed; the aggregate Required Gate failed. The API-specific artifacts were downloaded and SHA-256 matched their Actions metadata:

| API | Artifact ID | Size | SHA-256 | Result |
|---|---:|---:|---|---|
| 26 instrumentation | `12944519580` | 825,691 B | `faf644a79f1c94dc4def01c992f60ee57d3a5c18fe6b3c2ad0c0bafde4ada6a0` | Required 105/105 PASS; Google/Gmail UI 16/16 PASS; OS Process Death PASS; Data Isolation PASS; Fake Google Process Death FAIL |
| 35 instrumentation | `12945743981` | 3,219,455 B | `be1b6b359ce3812339e1a3d4e2cedea7ee99a0fd3d4ea88618a2c28f16966e6f` | Required 105/105 PASS; Google/Gmail UI 16/16 PASS; OS Process Death PASS; Data Isolation PASS; Fake Google Process Death FAIL |
| Combined emulator evidence | `12945609231` | 15,936,774 B | `9fdfdff78e2a51d8442627e1b720b2a952d5839dccfe715e65386c8b40fe4481` | Contains per-API harness logs and lifecycle evidence |

JUnit reports for both APIs show the required and Google/Gmail UI sets with zero failures, errors, or skips. These PASS results do not close G9 because the separate Fake Google Process Death gate failed on both APIs.

### Fake Google Process Death failure classification

Exact per-API `seed-am-start.txt` says `Error: Activity class {app.serea.android.isolated/app.serea.android.isolated.FakeGoogleProcessDeathHarnessActivity} does not exist.` The command did execute `am start -W`; the host harness then timed out waiting for the `seed` marker, and seed logcat contained no application marker. The failure occurred before `onCreate`, ViewModel setup, Consent Preview, or Fake Calendar session generation. This is an isolated target APK/manifest/activity packaging or installation mismatch, not evidence of a slow seed coroutine or marker polling delay. The exact build/manifest cause is under investigation; no timeout increase or assertion removal is acceptable.

Relevant files: `tools/run_android_fake_google_process_death.sh`, `android/app/src/isolated/AndroidManifest.xml`, `android/app/src/isolated/java/app/serea/android/FakeGoogleProcessDeathHarnessActivity.kt`. Next diagnosis must inspect the APK installed after UTP, its package/activity manifest, manifest merge source, variant/flavor output, and exact APK provenance. Add a fail-closed prelaunch component check and preserve one-use seed/provenance gates.

Other exact #187 artifacts recorded by GitHub include APK candidate `12943424642` and JUnit/evidence artifacts. The debug APK artifact digest is `sha256:288c5f80d8778dd2727afb80aba95ffd0f71c9ce2a0207514668d0061a75e421`; emulator evidence and JUnit result ZIP digests are in the Actions metadata and are listed above where directly inspected. No physical Pixel evidence exists.

## ARM64 and Native evidence

PR #182 run `40967790559` ended FAILURE. x86_64 product adapter job `123776591543` passed. ARM64 guest job `123776591261` did not produce an ADB-visible guest or runtime inference result; its small sanitized artifact `12944267140` (863 B; Actions digest `41f57ee980e204e5b1c655b9c5014cf33326bd6672c8b18447bd874e7e9bb1dd`) reports x86_64 host, API35 arm64-v8a image revision 9, software emulation, stable Emulator 37.2.13.0/build 16740729 verified, but `adb_state=NOT_AVAILABLE` and `emulator_log_state=NOT_AVAILABLE`. No ARM64 guest boot, APK install, JNI load, model hash/load, or inference was demonstrated. Do not label this as ARM64 model incompatibility; environment/guest startup is unresolved.

The verified x86_64 runtime on PR #179 remains source-bound. It includes exact JUnit 2/2 and direct artifact/digest checks for the SmolLM2 fixture, two generations through local Router/Egress/Usage/Adapter/JNI, cancellation, release, recovery and cleanup. It is not evidence of ARM64 or Pixel 7a operation.

## Windows timeout

The previously failed test was `actual_host_model_policy_and_accounting_scenarios_survive_database_reopen`, which reported empty stdout/stderr without sufficient child termination evidence. The bounded diagnostic-only successor PR #181 added exit status/code and bounded output byte counts, without changing the 150 ms scenario timeout or policy assertions. Run `40967593381` now completes successfully on Windows, but no reproduced failure or diagnostic child status was captured. Classify: **root cause unresolved**; do not call the original issue a proven flake or product fix.

## Native Chat proposal

PR #175/#185 is documentation-only and reviewed as a design proposal. Required future contract remains: `LOCAL_MODEL_CHAT_ENABLED=false` in ordinary Demo/Beta/Release; test-only explicit composition; a fresh single-use, content-bound consent for each send; no proof replay; cancellation/process death after dispatch yields `OUTCOME_UNKNOWN`; no automatic retry and no Cloud fallback; context <=512, output <=256, threads <=2; model integrity, usage reservation/settlement, lifecycle/unload, cancellation and memory-pressure eviction. The proposal is not product wiring and cannot be described as accepted implementation. ARM64 and separate Security/Android integration gates remain open.

## Owner decisions and G10

Carry forward the authoritative O-01..O-10 from `docs/plans/2026-10-26-personal-beta-owner-decision-ledger.md` and PR #167; this checkpoint makes no Owner decision:

- O-01 Beta package/coexistence: pending.
- O-02 OAuth project, audience, Calendar read-only scope: pending; no activation.
- O-03 Cloud provider eligibility: pending.
- O-04 Google-origin Cloud egress: NOT_APPROVED.
- O-05 production signing identity/custody: pending; no production key.
- O-06 Pixel 7a: BLOCKED/NOT_RUN; no device operation.
- O-07 main merge: pending; main unchanged.
- O-08 Personal Beta: NO-GO.
- O-09 Gmail Restricted scope: pending later gate; no mailbox/data.
- O-10 standard local model: UNDECIDED; no default or redistribution change.

G10 work may continue on package/update policy, signing plan, OAuth and Gmail runbooks, SBOM/NOTICE, migration and Pixel acceptance checklist. It must not imply owner authorization or completion of runtime gates.

## Subagent lanes and checkpoint status

Three independent subagents were launched for this V6 turn:

- Agent A: ARM64 runtime assessment and isolated Native Chat adapter/proposal review.
- Agent B: Fake Google Process Death packaging/root cause and minimal fix.
- Agent C: exact-source CI/security audit, independent review, and G10 decision materials.

Their final Source SHA, Tree SHA, worktree, changed files, test evidence and review outcomes are to be appended after lane completion. Until independent review is recorded for a new code change, it is `PENDING_REVIEW` and is not integrated.

## Prohibitions maintained

No protected-main merge; real Google OAuth/account/data; Cloud API key or billing; production signing or Beta distribution; Pixel 7a operation; personal-data deletion; Required gate weakening; existing PR rewrite; Native Chat production enablement; unsupported ARM64 SUCCESS claim; or Personal Beta GO declaration.

## V6 Fake Google recovery checkpoint — 2026-10-30

Agent B confirmed root cause against `:app:processIsolatedManifest` and the built isolated APK. Manifest and DEX both contain `app.serea.android.FakeGoogleProcessDeathHarnessActivity`; the APK’s package ID is `app.serea.android.isolated`. The old host component shorthand expanded under the application ID and targeted nonexistent `app.serea.android.isolated.FakeGoogleProcessDeathHarnessActivity`. The fix explicitly invokes `app.serea.android.isolated/app.serea.android.FakeGoogleProcessDeathHarnessActivity`; it checks `am start -W` exit/status and exact Activity before waiting for the phase marker. This addresses the `Error type 3` hidden by a zero shell exit status and does not extend timeouts.

- PR #189: OPEN/DRAFT, base exact PR #187 head `68dd17e687b80bdad0134099077a886fca4de836`.
- PR #189 head: `df790b15e4d48da948564f12ceee7320d3423344`; tree `895ebeb92286828475fbefac2837f36e3f124004`.
- Changed files: `tools/run_android_fake_google_process_death.sh`, `tests/android_emulator_workflow_tests.py`.
- Agent C independent review: **ACCEPTED** at exact head. No Required IDs/gates, consent/session logic, network, or production composition changed.
- Focused Fake Google regressions: 2/2 PASS. Independent Android workflow + Identity suite: 47/47 PASS. Bash syntax and diff check PASS.
- Local isolated variant `:app:processIsolatedManifest :app:compileIsolatedKotlin :app:assembleIsolated`: BUILD SUCCESSFUL (39 tasks). Built APK SHA-256 `e276db3b694e6f036be0b15c52fba54fae581e3c49977886b9a4b629b5c5587c`; `aapt dump xmltree` and DEX inspection verify declared class and class bytecode. This is not emulator execution.
- Exact-source hosted runs: Android `41027562072`, Full CI `41027562035`, Native platform `41027562017`, Fast `41027562092`, RustSec `41027562207`, Git Hooks `41027562103`, and SQLite `41027562107` were QUEUED at last inspection. API26/35 Fake Google rerun and full Android artifacts remain pending; G9 stays PARTIAL.

## V6 G10 refresh and hosted-run checkpoint — 2026-10-30

PR #190 is an append-only G10 decision-packet successor to PR #167. It is OPEN/DRAFT, based on exact PR #167 source `ca49dc4ab4731b5820732d381a24a225ff5ae0c7`; head `ce87c71a266d6771caa6228f6843634d1cb4a542`, tree `a48dd38ecd43e82d769bf2ff547e5094dad420ff`. Agent B independently **ACCEPTED** the exact docs diff. `python3 tools/validate_docs.py docs` passed (205 Markdown files), and `git diff --check` passed. O-01..O-10 are byte-identical across base/head and no approval was granted.

The PR #189 Android run `41027562072` has assigned jobs. Android Change Scope `123969187042` passed, while Android Build/Lint/Instrumentation Compile, Unit, Security, Integration, and Dependency Audit were queued at last check; API26/35 emulator jobs had not run. Fast run `41027562092` passed. Full CI `41027562035` was in progress on several jobs; Native platform `41027562017` and RustSec `41027562207` were in progress; Git hooks `41027562103` passed; SQLite `41027562107` partially passed with remaining architecture jobs queued. These status checks do not establish G9 Android acceptance.

Agent A’s separate isolated Native Chat code candidate is being developed on PR #187 exact base. It is not connected to ordinary Chat. No integration or review conclusion is recorded yet; focused tests were rerun after fixing a test helper type mismatch. Record final source/tree and exact test output only after that run completes. Keep review `PENDING_REVIEW` until Agent C examines the committed head.

## Native Chat isolated candidate and G10 review — 2026-10-30

PR #191 is a separate default-off Native Chat route candidate; it is OPEN/DRAFT on exact PR #187 base `68dd17e687b80bdad0134099077a886fca4de836`. Source `ca8a2bbdc068b30bc7e20021194ccd649ea029c0`, tree `509146dd00202e2b1582f8176eea9d2eabf00c18`. It adds a standalone `LocalModelChatFeatureRouter`, `LOCAL_MODEL_CHAT_ENABLED=false` for all Android variants, adapter limit corrections (context 512, output 256, threads 2), and focused tests. No composition root, ordinary Chat UI, or release wiring changed. Exact local Gradle focused suite: `:app:testDebugUnitTest --tests app.serea.android.LocalModelChatFeatureRouterTest --tests app.serea.android.LocalModelChatAdapterTest` — 32/32 PASS, 0 failures/errors/skips. Agent C independently **ACCEPTED** this isolated patch. Its acceptance excludes ARM64 runtime, normal Chat integration, Android UI, and explicit memory-pressure/process-death product lifecycle. Hosted #191 Android `41028413798`, Fast `41028413595`, Full `41028413602`, Native platform `41028413657`, and related security/hooks/SQLite runs were queued at last inspection. Do not connect it to normal Chat or enable the flag.

PR #190 G10 decision-packet docs successor is independently **ACCEPTED** at exact source `ce87c71a266d6771caa6228f6843634d1cb4a542` / tree `a48dd38ecd43e82d769bf2ff547e5094dad420ff`, based on PR #167 `ca49dc4ab4731b5820732d381a24a225ff5ae0c7`. It appends current evidence only; Owner decision table O-01..O-10 is byte-identical to base. Docs validation: 205 files PASS. No Owner decision has been made.

## V6 ARM64 strategy and latest bounded CI poll

Agent A compared the supported Android Emulator host packages from the official stable SDK metadata: the listed host binaries are Linux x86_64, macOS x86_64, and Windows x86_64; no supported Linux ARM64/macOS ARM64 host package was available to run the arm64-v8a guest. The current local host is x86_64 with no Emulator binary and no `/dev/kvm`. PR #182's bounded guest run already failed before ADB/guest confirmation. A public ARM64 GitHub host therefore does not resolve the missing supported Android ARM64 host emulator. Pixel/device-farm access would cross Owner hardware/account/billing boundaries. **No retry was made; ARM64 remains BLOCKED/NOT_RUN.**

Latest bounded poll: PR #189 Android `41027562072` has Change Scope SUCCESS; Build/Lint/Instrumentation Compile and Android Unit are IN_PROGRESS; Security, Integration, and Dependency Audit are QUEUED. Required API26/35 Emulator jobs remain unassigned/not run. Fast `41027562092`, RustSec `41027562207`, Git hooks `41027562103` passed. Full `41027562035` has multiple jobs still IN_PROGRESS/QUEUED; Native platform `41027562017` has jobs IN_PROGRESS/QUEUED; SQLite `41027562107` has two producer jobs passed, with consumer architecture jobs queued. PR #191 hosted Android `41028413798`, Full `41028413602`, Native platform `41028413657` remain queued. No hosted run in this poll supplied ARM64 Android inference or #189 API26/35 Fake Process Death evidence.

## Final checkpoint for this execution window — 2026-10-30

At the final bounded GitHub poll, PR #189 Android run `41027562072` Change Scope was SUCCESS; Build/Lint/Instrumentation Compile, Android Unit, and Android Integration were IN_PROGRESS; Security and Dependency Audit were QUEUED. Emulator Required/API26/API35 jobs were not yet assigned, so the accepted Fake Google fix has not received hosted process-death reacceptance. Fast `41027562092`, RustSec `41027562207`, and Git hooks `41027562103` were SUCCESS; Full CI `41027562035`, Native platform `41027562017`, and SQLite `41027562107` had outstanding IN_PROGRESS/QUEUED jobs. PR #191 hosted Android `41028413798`, Full `41028413602`, and Native platform `41028413657` were QUEUED/IN_PROGRESS without terminal evidence. No further same-source retries were started.

Resume point: poll only these existing exact-source runs; retrieve final API26/35 artifacts for PR #189 if assigned; verify JUnit identities/counts, Fake Google seed/verify markers, APK provenance and artifact digests; then decide whether to create a G9 integration candidate from #187 plus the reviewed #189 patch. Keep Native Chat PR #191 separate/default-off and require its hosted CI and future ARM64/lifecycle gates before product wiring. ARM64 remains blocked/not run; Windows timeout root cause remains unresolved; G9/G10 PARTIAL; Personal Beta NO-GO.
