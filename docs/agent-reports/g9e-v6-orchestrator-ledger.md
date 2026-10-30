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
