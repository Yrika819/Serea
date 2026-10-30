# G9F V7 Orchestrator Ledger

Checkpoint date: 2026-10-30 UTC. This is an append-only, source-bound checkpoint. Recheck each PR HEAD, tree, run, artifact ID, and digest before accepting evidence. Evidence never transfers to a different source SHA.

## Required evidence index

CAMPAIGN: Serea Autonomous Campaign V7 — P0 G9 Android Formal Closure; P1 Native Chat lifecycle and ARM64 readiness; P2 G10 preflight.

AGENT: Parent Orchestrator with Agent A (Native Chat / ARM64), Agent B (G9 #189), and Agent C (independent security/release review). Agent work used the shared execution filesystem; code and ledger edits are isolated in separate Git worktrees.

STATUS: IN_PROGRESS. G9 formal closure is not established. Native Chat remains isolated and disabled. G10 is PARTIAL. Personal Beta is NO-GO.

SOURCE_SHA: G9 PR #189 `df790b15e4d48da948564f12ceee7320d3423344`; Native Chat PR #191 current head `31bbab289c52802ca0f2be142c8dcebd97767737`; V6 ledger base PR #188 `1eadc83583567d3e79cdf695b961c1f02b388508`; protected `main` `e0acb0254820c381dd97828824bef86a1db5176c`.

TREE_SHA: G9 #189 `895ebeb92286828475fbefac2837f36e3f124004`; Native Chat #191 `0c58ebdb5772c752b5153f9316a5c9ceb6b46b51`; V6 ledger base #188 `9725d03af1c46de47c57a404ad4e9bb1f0df622f`; `main` `4955555be75c5742796740a3283d1a521e0d4806`.

BASE_SHA: G9 #189 is based on exact PR #187 HEAD `68dd17e687b80bdad0134099077a886fca4de836`; Native Chat #191 is based on the same exact #187 HEAD. Ledger checkpoint is based on PR #188 HEAD `1eadc83583567d3e79cdf695b961c1f02b388508`, itself a docs-only successor.

DRAFT_PR: V7 ledger is saved in open Draft PR #192, head `e4557671a4f7331690eca1f2cc7f3a1ab7a8609c`, tree `bff992f7c6d9e0226dcfb9bba00144a8f5e251eb`, based on PR #188 exact head `1eadc83583567d3e79cdf695b961c1f02b388508`. Native Chat changes remain in existing Draft PR #191. No code from #189 or #191 is combined into #192.

CHANGED_FILES: V7 checkpoint PR: `docs/agent-reports/g9f-v7-orchestrator-ledger.md`. Native Chat #191 update from `ca8a2bbdc068b30bc7e20021194ccd649ea029c0` to `31bbab289c52802ca0f2be142c8dcebd97767737`: `android/app/src/main/java/app/serea/android/LocalModelChatAdapter.kt`, `android/app/src/test/java/app/serea/android/LocalModelChatAdapterTest.kt`. G9 #189 candidate changes remain its two reviewed files: `tools/run_android_fake_google_process_death.sh`, `tests/android_emulator_workflow_tests.py`.

TEST_RESULTS: #189 exact source: Change Scope, Build/Lint/Instrumentation Compile, Android Unit, Security, Integration, and Dependency Audit SUCCESS. API26/35 formal emulator tests were in progress at this checkpoint. #191 exact updated head: independent Agent C review ACCEPTED for fake-provider lifecycle changes only; local `git diff --check` PASS. Focused Gradle suite could not start because Android Gradle Plugin `com.android.application:8.12.3` could not be resolved under current network restrictions; this is NOT a test pass. Existing #191 head before lifecycle update had 32/32 focused adapter/router tests recorded in PR history. No ARM64 Android inference or Pixel operation was performed.

CI_RUNS: #189 Android `41027562072` (API26 currently running; API35 pending); Fast `41027562092` SUCCESS; Full `41027562035` SUCCESS; Native platform `41027562017` pending/partial; RustSec `41027562207` SUCCESS; Git Hooks `41027562103` SUCCESS; SQLite `41027562107` pending/partial. #191 updated HEAD `31bbab2`: Android `41033109776`, Fast `41033109843`, Full `41033109849`, Native platform `41033109860`, SQLite `41033109777`, RustSec `41033109880`, Personal Beta Contract `41033109774`, Git Hooks `41033109823` were newly queued/pending when captured. The earlier #191 Android `41028413798`, Full `41028413602`, Native `41028413657`, and SQLite `41028413633` belong only to old head `ca8a2bbdc068b30bc7e20021194ccd649ea029c0`; they do not validate the updated head.

ARTIFACT_IDS: #189 non-emulator same-source artifacts available: Android Security JUnit `12966397825`, Android Unit JUnit `12965375448`, debug APK candidate `12965056693`, Integration JUnit `12964483006`, Dependency Audit `12967170064`. No #189 API26/API35 instrumentation artifact was available yet. #191 updated-head artifacts are pending.

ARTIFACT_SHA256: #189 security JUnit `18cf46fa31a2ef8fe4ef7f63dcc6997565efd84a1d23d0f4ec4083c15a0e09c9`; unit JUnit `b907e2ca28e02601c2c498a7f4d78b065faa944b48c4a8ff3a958f648eeea955`; debug APK candidate `7cce3c52989b055f34dc0f28353198976751f10e9529f6f4fb9e5402f851d975`; integration JUnit `f75751ffec6696637f8fa1b36ee93078027b488cb239f93462656c35ff44b0f2`; Dependency Audit `bd22a7c4c67a97175c5d1621f6b591abc0415817ea84c4374c596c6f76e5c3ad`. These are GitHub Actions metadata digests; emulator-specific artifact checksums and JUnit contents remain pending.

ANDROID_API26: #189 Required 105/105, Google/Gmail UI 16/16, OS Process Death, Fake Google Process Death, Data Isolation, Sentinel, Identity, IME, and APK provenance are NOT_RUN/NOT_VERIFIED on this HEAD as of this checkpoint. The API26 Pixel emulator workflow step was active.

ANDROID_API35: Same-source formal matrix NOT_RUN/NOT_VERIFIED on #189. API35 emulator campaign had not started at checkpoint time.

FAKE_GOOGLE_PROCESS_DEATH: #187 run `40970247739` failed both APIs because the shortened component resolved inside suffixed package `app.serea.android.isolated`. #189 corrects the launch to explicit component `app.serea.android.isolated/app.serea.android.FakeGoogleProcessDeathHarnessActivity` and fails closed on `am start -W` status/component mismatch. Prior independent review ACCEPTED the isolated correction. Exact #189 hosted seed/verify markers, stop/new process, process identity, session/consent state, failure handling, and API artifacts remain PENDING until run `41027562072` finishes.

NATIVE_X86_64: SUCCESS is source-bound only to PR #179 `a7f5b407d400b611df2c5f9260eede9b62f41b59` / tree `9bb6f18000d841fc436b3caa6b20c92a329ed5ae`, run `40967241879`: real SmolLM2 inference through local Router/Egress/Usage/Adapter/JNI, cancellation/recovery/cleanup. It is not evidence for updated #191 runtime or ARM64.

NATIVE_ARM64: BLOCKED / NOT_RUN. PR #182 run `40967790559` did not obtain an ADB-visible Android guest on x86_64 software emulation; no Android ARM64 JNI load, model load, inference, or hardware acceptance. This environment is x86_64 without Android Emulator or `/dev/kvm`. Existing Rust ARM64 CI jobs validate Rust targets only. Do not repeat the same bounded software-emulation attempt. Reconsider only a supported Android ARM64 host/runner with cost, permission, API image, acceleration, and toolchain confirmed; Pixel 7a remains Owner-gated.

NATIVE_CHAT: #191 remains Draft and `LOCAL_MODEL_CHAT_ENABLED=false` for all variants, without normal Chat or production composition wiring. Updated head `31bbab289c52802ca0f2be142c8dcebd97767737` adds internal model eviction under the adapter mutex, fresh verified reload after eviction, in-flight eviction/close-race checks, and deterministic serialized-request tests. Agent C ACCEPTED this fake-provider test scope. Hosted CI for this updated SHA is pending. Process death, native runtime, UI/composition, Android security, ARM64 inference, and memory-pressure OS integration remain separate open gates. No Cloud or Demo fallback is introduced.

SECURITY_REVIEW: #189 isolated component correction ACCEPTED per V6 ledger, limited to launch validation and workflow tests; exact #189 emulator acceptance remains BLOCKED/PENDING. #191 original isolated route/limits patch ACCEPTED per V6 ledger; lifecycle update ACCEPTED by Agent C at the exact reviewed uncommitted diff, now committed as `31bbab2` with the same tree content. Acceptance covers fake-provider eviction, close race and request serialization only. GitHub review-submission endpoints returned no review nodes for these PRs; review findings are versioned in the ledger/PR record. Security boundaries preserved: no real OAuth/data, Cloud credentials/billing, silent fallback, consent replay, account cross-isolation, or unauthorized egress; all-variant feature flag remains OFF; required identities/gates unchanged.

OWNER_DECISIONS: Canonical PR #190 head `ce87c71a266d6771caa6228f6843634d1cb4a542`, tree `a48dd38ecd43e82d769bf2ff547e5094dad420ff`. O-01 PENDING_OWNER_DECISION; O-02 PENDING_OWNER_DECISION / NOT_RUN; O-03 PENDING_OWNER_DECISION / NOT_RUN; O-04 NOT_APPROVED / NOT_RUN; O-05 PENDING_OWNER_DECISION / NOT_RUN; O-06 BLOCKED_BY_DEVICE_AVAILABILITY / NOT_RUN; O-07 PENDING_OWNER_DECISION; O-08 current NO-GO, decision pending 2026-12-31; O-09 PENDING_LATER_GATE / NOT_RUN; O-10 UNDECIDED / NOT_RUN. No Owner decision or authorization is implied.

BLOCKERS: #189 API26/API35 hosted emulator matrix and source-bound JUnit/evidence/APK provenance; #191 updated-head Hosted CI and focused tests under a dependency-resolving environment; Android ARM64 runner/runtime; explicit runtime lifecycle, process death, security/UI/composition gates; O-01/O-02/O-03/O-05/O-07/O-09 decisions, O-04 denial, O-06 device availability/authorization, O-10 model selection; G10 package identity/signing custody, complete packaged SBOM/NOTICE/model redistribution license, encrypted migration/update/rollback, and Pixel 7a acceptance.

NEXT_ACTION: Continue polling only existing exact-source #189 run `41027562072` and updated #191 runs. When #189 completes, download and inspect API26/API35 JUnit and lifecycle evidence, verify artifact digests, markers, APK provenance, and every required identity; decide G9 closure only from that exact HEAD. Record terminal results in a follow-up ledger commit. For #191, accept only hosted checks on `31bbab2` and record Android runtime/security and product-integration gates separately. Keep ARM64 BLOCKED/NOT_RUN and Personal Beta NO-GO.

## Dependency and worktree record

Protected `main` was confirmed at `e0acb0254820c381dd97828824bef86a1db5176c`; remote `git ls-remote` and local main worktree agree. Local main worktree was clean. Current isolated worktrees: main baseline `/workspace/Serea`; Native Chat `/workspace/Serea-native-lifecycle` (branch `codex/serea-v7-native-lifecycle`, updated commit `31bbab2` pushed fast-forward to existing PR #191 branch); ledger `/workspace/Serea-v7-ledger` (branch based on PR #188). No main changes, merge, force push, or rebase occurred. PR #187 -> #189 G9 correction and #191 Native Chat are independent Drafts; PR #190 G10 is a separate Owner packet. No ARM64 retry was started. Personal Beta remains NO-GO.

## PR #192 initial saved checkpoint

PR #192 was opened as a Draft against exact PR #188 base. Its initial checkpoint source is `e4557671a4f7331690eca1f2cc7f3a1ab7a8609c` / tree `bff992f7c6d9e0226dcfb9bba00144a8f5e251eb`. It changes only this ledger file. This append-only follow-up records the PR metadata; its own commit/tree will be recorded by GitHub as a new exact-source checkpoint.
