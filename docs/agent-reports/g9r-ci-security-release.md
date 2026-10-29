# G9R CI, Security, and Release Evidence

**Captured:** 2026-10-29 09:50 UTC
**Agent:** C — CI / Security / Release  
**ACTIVE_PHASE:** G8/G9 evidence recovery; independent security and release review  
**GOAL_STATUS:** PARTIAL — source and exact-run evidence recovered; API26 Required passes on PR #149, while Google/Gmail UI and API35 IME diagnostic failures remain; current successor UI runs are queued. Native inference remains unproven.  
**BASE_SHA / TREE:** `d4a36fb5d3613bca2d60151de0991a1baac72fc9` / `b78a84b60ef5a2d344eabdda85b1154e62be46d7` (PR #151)  
**WORKTREE / BRANCH:** `/workspace/serea-g9r-security-evidence` / `codex/serea-g9r-security-evidence`  
**CHANGED_FILES:** this report only. Product code, required manifests, gates, existing PR branches, and protected `main` were not modified.

This snapshot separates a test failure from a test that did not start. It does not reuse evidence across source SHAs. All test data was synthetic/Fake. No live OAuth, Google account, Cloud provider, production key, physical Pixel 7a, main merge, or Beta distribution was used.

**Local environment:** `/workspace/.tools/temurin17/bin/javac` and Android `adb` are available. No emulator/system image, connected device, or `/dev/kvm` is available in this workspace. Agent B's local AndroidTest Kotlin compilation and workflow-harness tests are recorded below at exact source/tree; this lane has no emulator runtime.

## Authority and dependency graph

GitHub was queried at campaign resume. The protected main source remains unchanged at `e0acb0254820c381dd97828824bef86a1db5176c` (tree `4955555be75c5742796740a3283d1a521e0d4806`). PR #146 and #147–#154 remain open Draft candidates; none was rebased, force-pushed, closed, or merged.

| PR | Head SHA / tree SHA | Base | Dependency / current meaning |
|---|---|---|---|
| #146 | `d444e7cc04c8f1c4c6d12b1412cfe3a76258af66` / `25e2bb481a0b25bfed89d6ee25f1812316703b8e` | G7 `0560e1e7bcd13f0194eab8caca89d70ea5a2f142` | Frozen G8 integration comparison candidate. |
| #147 | `36700f6c99d15db428c6d434c657ac76593df3f9` / `107824f5338ed9ec46d41f6836f5df9c6ad84cf7` | G7 | Isolated Native/JNI runtime candidate; independent of #146. |
| #148 | `a00a4a224a7b36854b70d883acfd35253cf200d6` / `e8fab02b3abcd62211355f73f3c7d829ff230f75` | G7 | Prior C evidence/report candidate; this report is a new checkpoint. |
| #149 | `3a0f0c1897f2115549e7682d6fcc1783c44604f3` / `6a1f4d382907a7deb7d6dbf8147e6490aa054e81` | #146 | G8 emulator package-query recovery candidate. |
| #151 | `d4a36fb5d3613bca2d60151de0991a1baac72fc9` / `b78a84b60ef5a2d344eabdda85b1154e62be46d7` | #149 | Current G9 Gmail body-only handoff candidate. |
| #152 | `a9da3b71fff786fc93d3365249ea8087477b31b2` / `cbf688e4438794437b02bc7bd0fcc9bba975264f` | #151 | Google/Gmail AndroidTest selector and date fixes; no app production code changes. |
| #153 | `6f4c8c5b4caf8f48619234fee5a9c864cb25766b` / `8a17da2ba3003d8663a3c6734d444682609abe05` | #151 | Test-only bounded Activity/IME readiness diagnostic. |
| #154 | `90913b4d4e3d834ae96d4f7e842bddfbd2b61ff6` / `3f88b0800971de9474a63c0286959aa94074613a` | #151 | Documentation-only central ledger checkpoint. |
| #155 | `b53651a6a5ea2ac7990ecdf2a508676e13b4bc21` / `7cfca07f1bc9e306ebaf5cb379db73f255f46c2b` | #151 | Parent-owned G9 integration candidate; exact-head formal CI pending. |
| #157 | `e19a592611112ac2d9ea52bf337c4a4507400e59` / `c32444191c43e7f40af941cee1000f4e5ab711f5` | #151 | AndroidTest-only UI selector/date and bounded IME recovery. |
| #158 | `26645df4cc115ab0872c7b49349575dca761828c` / `4a313464747c897c7b134406a69cc40ee179dacb` | G7 branch; parent is exact #147 SHA `36700f6c99d15db428c6d434c657ac76593df3f9` | Native instrumentation diagnostics/no-op recovery; runtime still queued. |

Dependency chain: #146 → #149 → #151 → (#152, #153, #154, #155, #157). #147 → #158 remains an isolated Native lane pending runtime evidence. PR #152 and #153 are sibling changes; neither their UI tests nor PR #157's Android tests are represented as hosted runtime success yet.

## Frozen G8 formal result — PR #146

The nine requested workflows are terminal, attempt 1, and bound to exact source `d444e7cc04c8f1c4c6d12b1412cfe3a76258af66` / tree `25e2bb481a0b25bfed89d6ee25f1812316703b8e`.

| Run | Workflow | Result |
|---:|---|---|
| `40804694025` | Android | **FAIL**. Android Unit is 405/405 PASS. Storage/Emulator and Required Gate fail before Required instrumentation. API26 Required is 0/105 started; API35 Required is 0/105 started. No Required JUnit identity exists for either API. Identity Isolation is SKIPPED. Process Death, Data Isolation, and Sentinel passed in their separate harness evidence. |
| `40804693907` | Full CI | PASS |
| `40804693934` | Native platform validation | PASS for Rust platform matrix only; not Android JNI inference. |
| `40804693978` | SQLite portability | PASS |
| `40804693889` | RustSec | PASS |
| `40804693992` | Fast CI | PASS |
| `40804693919` | Git Hooks | PASS |
| `40804722382` | Release Security | PASS; CI-only debug APK/mock signing and lockfile SBOM. |
| `40804722157` | Mock Beta Contract | PASS |

Thus G8 formal closure remains **PARTIAL**. “0/105” means the 105 tests were not started, not that 105 tests failed. No evidence from #149 or later is applied retroactively to #146.

## G8N Android baseline — PR #149 exact failure decomposition

Android run `40810824093`, attempt 1, is bound to PR #149 exact SHA/tree above. Workflow job `123243058373` ran on GitHub Actions runner `1000023470` and ended FAILURE. Logs and both API artifacts were retrieved through the GitHub artifact connector; downloaded ZIP SHA-256 values match the Actions artifact digest fields.

Supporting workflows on this same exact #149 head passed: Full CI `40810824061`, RustSec `40810824040`, Fast CI `40810824083`, SQLite `40810824022`, Git Hooks `40810824150`, and Rust Native platform matrix `40810824148`. The Android failure is not erased by these passes; the Rust Native matrix is not Android JNI runtime evidence.

### API 26

- The Required manifest was **103 Original Required + 2 Additional = 105**. The Required JUnit XML reports **105 tests, 0 failures, 0 errors, 0 skips**. Original Required is 103/103 PASS; Additional is 2/2 PASS.
- Separate, non-Required focused stages passed: Calendar class 19/19, Chat screen class 16/16, cursor diagnostic 1/1.
- Separate Fake Google/Gmail UI stage reports **16 tests, 5 failures, 0 errors, 0 skips**. The exact failed JUnit identities and observed behavior are:

| JUnit identity | Observed failure |
|---|---|
| `app.serea.android.GoogleAuthorizationDemoUiTest#accountSwitchExpiryAndSharedRevocationInvalidateDemoAuthorization` | Text selector for `架空アカウント B` matched three nodes, so `performScrollTo()` did not identify a unique account button. |
| `app.serea.android.GoogleAuthorizationDemoUiTest#expiredCalendarReconnectShowsFreshPreviewAndWaitsForConfirm` | Reconnect copy matched two controls (Gmail and Calendar); selector was ambiguous. |
| `app.serea.android.GoogleCalendarDemoLoopbackUiTest#consentUiOpensLoopbackAgendaAndAccountSwitchDisconnectsIt` | `MainActivity` had already set Compose content before the test called `setContent` again. |
| `app.serea.android.GoogleAuthorizationAppIntegrationTest#settingsConsentOpensCalendarSessionAndActivityRecreationReturnsToConsentPanel` | Timed out waiting 5 seconds for `Design review` before Activity recreation; cause is not confirmed. The device-local query date versus UTC fixture instant is a plausible hypothesis only. |
| `app.serea.android.GoogleCalendarHandoffRevocationUiTest#confirmedCalendarHandoffCannotSendAfterSyntheticAccountSwitch` | Assertion searched for exact text `B`, while the semantic label included `選択中: 架空アカウント B`. |

API26 artifact: ID `12872143095`, 867,807 bytes, SHA-256 `ebdd9e7a9f5343e9dfacba91abed7ba75b4bf56a338fa35cbe3582160ae4f34e`. It contains the 105-test Required XML and 16-test Google/Gmail stage XML. The Google/Gmail UI failures are additional product UI acceptance failures; they are not counted as Required manifest failures.

### API 35

- The bounded IME diagnostic ran first and reports exactly **1 test, 1 failure, 0 errors, 0 skips**: `app.serea.android.SereaAppShellTest#chatDraftSurvivesGmailAndCalendarDestinationChanges`.
- The campaign then recorded `instrumentation-status=NOT_RUN`; the API35 full Required suite is **0/105 started**, with no Required JUnit test identity/XML, and Google/Gmail UI stage is **NOT_RUN**, not failed. API35 process-death recovery and Data Isolation passed separately.
- This is the baseline IME diagnostic addressed by PR #153; it does not prove the PR #153 fix.

API35 artifact: ID `12872631908`, 255,398 bytes, SHA-256 `bd32cb1a21e26a257c429bc74bd0158f41c819072af835204f953232dc4f8978`. It contains the single failed diagnostic JUnit XML; no full Required JUnit exists in the archive.

## Native runtime review baseline — PR #147

Focused Native run `40843204437`, attempt 1, exact source `36700f6c99d15db428c6d434c657ac76593df3f9` / tree `107824f5338ed9ec46d41f6836f5df9c6ad84cf7`, failed in job `123344314662` on assigned runner `1000023588`. Both x86_64 and ARM64 native APK builds, fixture size/SHA validation, license metadata checks, and API35 x86_64 emulator/KVM setup passed. `LocalModelNativeRuntimeE2ETest` failed during Android instrumentation runner initialization with `RuntimeException: Failed to instantiate test runner class androidx.test.internal.runner.junit4.AndroidJUnit4ClassRunner`; the accessible run log does not include its cause chain. No inference ran. No source-level cause is confirmed; this remains a runner/classpath/manifest or test-harness issue to isolate with a same-runner No-op test and complete cause diagnostics.

Diagnostic artifact ID `12883594935`, 5,881 bytes, SHA-256 `e4ed10a1ec5d14a38bbf2e861b30b196d7e3b8e0e325dc1869df3c3c63760cb1`, was retrieved through GitHub and its ZIP digest verified. It contains the exact source/llama.cpp pin, successful x86_64 CMake/APK assembly log, and `LocalModelNativeRuntimeTest#defaultProductBuildFailsClosedBeforeLoadingNativeLibrary` 1/1 PASS. It does not contain the Android runner initialization cause or an inference JUnit report. The runtime artifact was skipped. Cleanup verified host and `/data/local/tmp` fixture absence and package absence; app-private model deletion could not be verified after the app package was absent and failed closed. SmolLM2 inference, two-turn generation, Usage/Egress, cancellation/release/recovery, ARM64 runtime, APK hash/signer and Pixel7a remain NOT_RUN.

The separate PR #147 Android run `40843204488` has Change Scope and Documentation PASS; Android Unit `123351782654` is now IN_PROGRESS on runner `1000023742`; Dependency Audit, Build/Lint/Instrumentation Compile, Security, and Integration remain queued. Native platform `40843204674` and Full CI `40843204630` remain queued; they are not Android JNI runtime evidence.

## G9 current candidates and exact Actions state

Every run below is attempt 1 and is bound to its cited head. A queued job with an empty `runner_name` is an unassigned runner wait. It is neither a source failure nor a pass. Skipped Android product jobs on the documentation-only PR #154 are not product evidence.

### PR #151 — source `d4a36fb…`

Android run `40842824215` is IN_PROGRESS with the same passing jobs: Change Scope `123343071547`, Documentation `123343497932`, Dependency Audit `123343497977`, Build/Lint/Instrumentation Compile `123343498034`, Unit `123343498047`, Integration `123343498065`, Security `123343498159`. Storage and Emulator job `123346917078` is running on runner `1000023744`; no API26/35 result exists yet. Android Unit artifact `12884802699` (29,537 bytes, digest `sha256:3f713d66034cd7786cf585de3fb6729bc3a15b290872cc34e3689b6f211e55e1`); Integration `12886263140` (1,218 bytes, `sha256:b62b8cf7ba951c85d0c6a80be20ccfebc154f3236d6938c2339cce1f6ffcfdaa`); Security `12885645768` (1,829 bytes, `sha256:567583e25fb159a11795072f28b559f7223469b498500325f5b88a7698b2e4d8`); Dependency Audit `12884620339` (99,201 bytes, `sha256:29adc213c06bf67c3670f1332a0fa2dd399acfad509d671f5efa7ffba92c3299`). Debug APK candidate artifact `12883404140` is 33,163,400 bytes, artifact digest `sha256:fccaf9a97a606ea3cb79b5805e04c026ec146b1fc0fdd0242e7d9239ae66966a`; the archive is larger than the artifact-download limit available here, so inner APK SHA-256 and signer were not verified. Do not call it Pixel evidence.

Other exact-head runs: Full CI `40842824328` PASS; RustSec `40842824232` PASS; Fast `40842824206` PASS; Git Hooks `40842824356` PASS; SQLite `40842824221` PASS across both producers and both cross-architecture consumers (artifacts `12883124117`, `12885587512`). Native platform `40842824208` remains in progress; stable macOS x86_64 and Rust 1.85 macOS x86_64 are still active in this query. This Rust matrix does not establish JNI inference.

### PR #152 — source `a9da3b7…`

Android run `40844297910`: Change Scope job `123348731256`, Security `123350450350` on runner `1000023736`, and Unit `123350450370` on runner `1000023768` PASS. Dependency Audit `123350450336`, Documentation `123350450382`, Build/Lint/Instrumentation Compile `123350450402`, and Integration `123350450465` remain queued without runners. Therefore no API26/35 UI result exists on #152. Fast CI `40844297989` PASS. Full CI `40844298004` is in progress (four jobs complete, macOS ARM64 `123347958495` active). Native `40844297921` and SQLite `40844297913` are partial/in progress with other jobs queued. RustSec `40844298116` and Git Hooks `40844297907` are queued.

Independent source review of the four AndroidTest-only changes is **ACCEPTED** at the exact tested code precursor `b553d68325b7c18dfbecaea2a9e92ebd15adad27` / tree `c494e20d34a961d16d67c4c882bc2eeb7b3b70f0`: unique semantics tags for ambiguous labels, test-owned Compose content via `createComposeRule`, substring account assertion, and timezone-aware query date derived from the fixed fixture instant. Required test identities and assertions remain. The current PR head adds the report file; UI/JUnit CI remains pending. Local AndroidTest compilation at the precursor passed; that compile is not a runtime result and is not promoted to exact-head CI evidence.

### PR #153 — source `6f4c8c5…`

Android run `40844961565`: Change Scope `123350084172` PASS; Unit `123357717576`, Dependency Audit `123357717636`, Integration `123357717676`, Security `123357717680`, and Build/Lint/Instrumentation Compile `123357717711` remain queued. Documentation job `123357719925` was skipped. No API26/35 UI JUnit is present. Full CI `40844961537` has four jobs PASS and macOS ARM64 `123350084499` queued; Native `40844961372` has completed passes, in-progress, and queued jobs; SQLite `40844961545` has both producer jobs PASS, one consumer in progress and one queued. RustSec `40844961783`, Fast `40844961637`, Git Hooks `40844961944` PASS.

The test-only source patch is **ACCEPTED** by independent review: after Activity recreation, a bounded 10-second wait checks `RESUMED`, decor attached/shown/window-focused before the existing focus, selection, real Test IME, `commitText`, content and cleanup assertions. On timeout it records lifecycle/window state. It adds no unconditional sleep and removes no assertion. JDK17 compilation is locally possible, but exact-head local compile and emulator execution were NOT_RUN in this lane. The PR's CI instrumentation jobs remain queued. Acceptance is limited to source safety/intent, not to runtime behavior.

### PR #154 — source `90913b4…`

This is a docs-only ledger change. Android run `40845836065` completed **SUCCESS**: Change Scope `123352969306`, Documentation `123361709252`, and downstream Required Gate `123365974470` succeeded. Unit `123361712312`, Integration `123361711236`, Dependency Audit `123361711456`, Security `123361711551`, Build/Lint/Instrumentation Compile `123361712393`, Identity Isolation `123361712877`, Storage/Emulator `123361712961`, and Pixel bundle `123361713571` were **SKIPPED** under the docs-only change scope; none ran product tests. Fast CI `40845836195` and Git Hooks `40845836204` PASS. Full `40845836135`, Native `40845836032`, SQLite `40845836238`, and RustSec `40845836070` remain partial/queued; run completion is pending.

### PR #157 — source `e19a592…`, tree `c324441…`

This Draft is based on exact #151 SHA `d4a36fb5d3613bca2d60151de0991a1baac72fc9`. The five-file patch is limited to AndroidTest selectors/date setup and bounded Activity/IME readiness; it does not change production code, the 103 Required or 2 Additional identities, assertions, or gates. Independent source review is **ACCEPTED** at this exact HEAD/tree, with runtime behavior still NOT_RUN.

Agent B's local compile was bound to the same staged contents, with no changes before commit: `:app:compileIsolatedAndroidTestKotlin` completed successfully (26 actionable tasks; 5m10s). Command:

```text
cd /workspace/serea-g9r-android-ui-recovery/android && JAVA_HOME=/workspace/.tools/temurin17 PATH=/workspace/.tools/temurin17/bin:$PATH ./gradlew --no-daemon --console=plain -Dhttp.proxyHost=proxy -Dhttp.proxyPort=8080 -Dhttps.proxyHost=proxy -Dhttps.proxyPort=8080 -Djavax.net.ssl.trustStore=/etc/ssl/certs/java/cacerts -Djavax.net.ssl.trustStorePassword=changeit :app:compileIsolatedAndroidTestKotlin
```

The local harness test `PYTHONPATH=. python3 tests/android_emulator_workflow_tests.py` passed 34/34 on this exact HEAD/tree. Output file SHA-256 is `6701848a3a17941aed8496f8ad448bbcf5401eaae550183ac9765cb60f45ad04`; it is a local log, not a GitHub artifact. The harness uses mocked ADB and does not run an emulator. On Android run `40850921385`, Change Scope `123369868355` PASS, Documentation `123374675864` SKIPPED, and Security `123374673160`, Dependency Audit `123374673268`, Integration `123374673271`, Unit `123374673389`, and Build/Lint/Instrumentation Compile `123374673498` are queued. Thus no API26/35 JUnit exists yet. Full `40850921503`, Native `40850921304`, SQLite `40850921299`, RustSec `40850921203`, Fast `40850921367`, and Git Hooks `40850921057` remain queued or in progress; no artifact is available.

### PR #155 — Parent integration candidate, source `b53651a…`, tree `7cfca07…`

PR #155 is an open Draft based on PR #151. Exact-head CI is pending/queued at capture: Android `40852318331` (pending with `jobs=[]`), Full CI `40852318252`, Native `40852318251`, SQLite `40852318241`, RustSec `40852318286`, Fast `40852318237`, and Git Hooks `40852318290`. Native run `40852318251` has Rust 1.85 Windows ARM64 job `123374640569`, Stable Windows ARM64 job `123374640755`, Stable Linux ARM64 job `123374640810`, and Stable Windows x86_64 job `123374640825` in progress; other matrix jobs remain queued. Android `jobs=[]` is no job evidence and cannot be treated as skipped product success. The Beta workflow `40852309768` failed with `jobs=[]`; its cause is not established here and it provides no Android product evidence. No artifact/JUnit is available for this exact integration SHA at capture.

### PR #158 — Native recovery, source `26645df…`, tree `4a31346…`

This Draft targets the G7 branch and is a two-commit extension whose first parent is the PR #147 exact SHA `36700f6c99d15db428c6d434c657ac76593df3f9` / tree `107824f5338ed9ec46d41f6836f5df9c6ad84cf7`. Patch review covered the direct `am instrument` command and parser, no-op and native identities, timeout/force-stop boundary, package/model ownership cleanup, artifact gating, and non-Required identity isolation. The Required and Additional manifests are unchanged; the two new instrumentation identities are explicitly non-Required. Agent A reports source inventory as 103 Required + 2 Additional + 17 non-Required = 122, with no overlap.

The first patch head `07f0671e801236a1e110295186c5dcc01888fbae` / tree `2b826a2e881b6f50da241557d5f813ff9db9d46f` received **CHANGES_REQUESTED** because it did not require successful `INSTRUMENTATION_CODE: -1`. Its focused run `40851453193`, attempt 1, exact old SHA, remains queued at job `123371667057`. Agent A added a terminal-code requirement, validates reported `numtests` against the two expected identities, and added regression cases. Independent source review is now **ACCEPTED** at the current exact SHA/tree. Rerun on this source: `python3 tools/g8-local-model-native/test-instrumentation-stream.py` passed 8/8; `bash -n tools/g8-local-model-native/run-e2e.sh` and `git diff --check 36700f6..HEAD` passed. These do not establish Android execution.

New exact-head hosted runs are still pending: Android `40852384382` is `pending` with `jobs=[]` (no job result); focused Native `40852384302` is queued at job `123374814558`; Full CI `40852384297`, SQLite `40852384311`, RustSec `40852384399`, Fast `40852384315`, Git Hooks `40852384323`, and Native platform `40852384322` are queued or pending. Old-SHA run evidence does not transfer to this new SHA. The prior runner root cause remains unconfirmed; no-op execution, inference, Usage/Egress and cleanup are NOT_RUN.

Agent A reports a bounded AndroidTest/JVM compile is now running on this exact source/tree; its result is pending. The prior baseline `40843204437` established native APK builds and fixture identity only, not runtime.

## Independent review decisions

| Candidate | Decision | Scope / limitation |
|---|---|---|
| PR #149 `tools/android_package_path.sh` package-absence query fix | ACCEPTED (prior independent source review) | Correctly uses a successful empty `pm list packages -u` query before treating an absent package as absent; errors and unexpected output remain fail-closed. Its follow-on Android run still found the separate API26 UI and API35 diagnostic failures above. |
| PR #152 | ACCEPTED (test source only; verified code precursor) | No production code or Required manifest change; final PR UI evidence pending. |
| PR #153 | ACCEPTED (test source only) | Bounded lifecycle/window readiness with diagnostic snapshot; exact-head runtime evidence pending. |
| PR #157 Android UI/IME recovery | ACCEPTED (source only) | Exact source/tree reviewed; local compile and harness suite pass, but hosted API26/35 Android runtime is queued. |
| PR #158 Native runner recovery | ACCEPTED (source only, exact SHA `26645df…`) | Terminal instrumentation result and reported test count fail closed; hosted runner remains queued and inference NOT_RUN. |
| PR #147 Native recovery baseline | BLOCKED | Prior source run lacks a complete runner exception cause and inference; #158 is the successor candidate. |
| PR #151 G9 candidate | BLOCKED for Android acceptance | Required/API26/35, Google/Gmail UI and account/process lifecycle remain unexecuted on exact #151. |

No acceptance here authorizes merging a PR, moving the G9 integration candidate, enabling a real backend, or switching Chat to Native AI.

## Security and G10 release preflight

- **Google:** all runs are Fake/synthetic. Calendar is intended read-only. The Gmail `gmail.readonly` scope is Restricted and requires a separate Owner decision and data-flow/retention/verification review. Google connection consent, Google→Chat handoff consent and Cloud-egress consent remain independent. Google-origin Cloud egress stays denied; Cloud is disabled.
- **Account and handoff:** retain account/session/generation binding, revocation and single-use handoff, source lineage/content identity, stale-callback rejection, and metadata exclusion. No email/calendar writes or other external actions are permitted.
- **Package and signing:** current expected variants are Beta `app.serea.android.beta`, Demo `.demo`, plus isolated test IDs. Package ID/coexistence is not Owner-ratified. No production key was made or used; CI temporary keys do not establish production identity.
- **Version/update:** Demo default is `versionCode=1`; Beta validation requires an explicit greater code. Owner must retain a monotonic private version ledger. Migration/update acceptance must preserve encrypted data under the same approved key; rollback should use a correctly signed forward update, not uninstall/data clear.
- **SBOM/APK provenance:** lockfile-derived CycloneDX is useful but incomplete for native linked objects, packaged membership, Gradle plugins/buildscript, wrapper, SDK/toolchain, and artifact-to-source attestation. A release packet must bind frozen source/tree, toolchain and lockfiles, packaged APK hash, package/version, signer certificate fingerprint, native ELF inventory/license notices/SBOM, and model revision/hash if distributed. Current #151 APK archive digest is available, but inner APK hash/signer could not be examined here.
- **Models:** existing SmolLM2 fixture is Apache-2.0 and llama.cpp MIT per prior pinned-candidate review; distribution is not cleared by metadata alone. Do not add model weights to Git. LFM3, Bonsai 2, Qwen3.5, Gemma 4 and FunctionGemma require official revision/license/format/backend review before download or distribution. Vendor benchmarks must be separate from Serea measurements; standard model remains undecided until independent review and Pixel 7a evidence.
- **Hardware/release:** Android Emulator/API evidence is not Pixel 7a acceptance. No owner device, production signing, main merge, OAuth activation or Beta distribution was used. Personal Beta remains NO-GO.

### Owner decision ledger discrepancy

The current source file `docs/plans/2026-10-26-personal-beta-owner-decision-ledger.md` assigns O-01 OAuth, O-02 AI provider, O-03 production signing, O-04 Pixel 7a, O-05 main merge, O-06 GO. The newer `docs/agent-reports/g8-agent-c-release-preflight.md` assigns O-01 package/coexistence, O-02 Google project/scopes, O-03 provider/model, O-05 signing, O-06 Pixel, O-07 main merge, O-08 GO. These are inconsistent identifiers, not changed permissions. All listed actions remain unapproved; preserve both source records until the Parent publishes one reconciled current matrix. No live service, key, device, merge, or GO action was performed.

## Next evidence and disposition

1. Keep #146 frozen as the G8 failure baseline; do not relabel 0/105 as a test failure count.
2. Do not accept API26/35 product UI until #152/#153 or the new Android recovery candidate produces exact-head artifacts with all manifest identities, counts, failures/errors/skips, and required lifecycle/security evidence.
3. For #147, require a No-op runner isolation result and full initialization cause before claiming the root cause; then test Native load/inference, two turns, Usage/Egress, cancellation, release and cleanup on a disposable x86_64 emulator.
4. Keep `X86_64_BUILD`, `X86_64_RUNTIME`, `ARM64_BUILD`, `ARM64_RUNTIME`, and `PIXEL7A_PHYSICAL` as separate evidence rows. The physical Pixel test remains Owner-gated.
5. Keep #154 docs-only skips out of product acceptance. Reconcile Owner decision IDs in the Parent ledger.
6. Run formal CI only after the Parent freezes a new integration SHA; bind each workflow and artifact to that exact source/tree. Preserve failed attempts.

**TEST_COMMANDS:** Read-only GitHub Actions API queries; exact run/job logs; exact PR diffs; GitHub connector artifact retrieval for #149 API26/#35 ZIPs; `sha256sum` ZIP digest verification; XML test-count/identity extraction; Agent B exact-tree `:app:compileIsolatedAndroidTestKotlin` (26 tasks, PASS) and `PYTHONPATH=. python3 tests/android_emulator_workflow_tests.py` (34/34 PASS); independent Agent A exact-tree `python3 tools/g8-local-model-native/test-instrumentation-stream.py` (8/8 PASS), `bash -n tools/g8-local-model-native/run-e2e.sh` (PASS), and `git diff --check 36700f6..HEAD` (PASS). No emulator/JUnit was run in this lane.
**JUNIT_RESULT:** PR #149 source-bound JUnit: API26 Required 105 total, 0 failures/errors/skips; API26 separate Google/Gmail UI 16 total, 5 failures, 0 errors/skips; API35 bounded diagnostic 1 total, 1 failure, 0 errors/skips; API35 full Required 0/105 started and UI NOT_RUN. PR #157 compile + local mocked harness pass, but API26/35 JUnit NOT_RUN. PR #158 parser regression 8/8 pass, but Android instrumentation/inference NOT_RUN.
**CI_RUN_IDS:** #146 `40804694025`, `40804693907`, `40804693934`, `40804693978`, `40804693889`, `40804693992`, `40804693919`, `40804722382`, `40804722157`; #147 `40843204437`, `40843204488`, `40843204630`, `40843204674`; #149 Android `40810824093`; #151 Android `40842824215`, Full `40842824328`, Native `40842824208`, SQLite `40842824221`, RustSec `40842824232`, Fast `40842824206`, Git Hooks `40842824356`; #152 Android `40844297910`, Full `40844298004`, Native `40844297921`, SQLite `40844297913`, RustSec `40844298116`, Fast `40844297989`, Git Hooks `40844297907`; #153 Android `40844961565`, Full `40844961537`, Native `40844961372`, SQLite `40844961545`, RustSec `40844961783`, Fast `40844961637`, Git Hooks `40844961944`; #154 Android `40845836065`, Full `40845836135`, Native `40845836032`, SQLite `40845836238`, RustSec `40845836070`, Fast `40845836195`, Git Hooks `40845836204`; #155 Android `40852318331`, Full `40852318252`, Native `40852318251`, SQLite `40852318241`, RustSec `40852318286`, Fast `40852318237`, Git Hooks `40852318290`; #157 Android `40850921385`, Full `40850921503`, Native `40850921304`, SQLite `40850921299`, RustSec `40850921203`, Fast `40850921367`, Git Hooks `40850921057`; #158 old-SHA focused Native `40851453193`; #158 current-SHA Android `40852384382`, focused Native `40852384302`, Full `40852384297`, SQLite `40852384311`, RustSec `40852384399`, Fast `40852384315`, Git Hooks `40852384323`, Native platform `40852384322`.
**ARTIFACT_IDS:** PR #149 API26 `12872143095` (`sha256:ebdd9e7a9f5343e9dfacba91abed7ba75b4bf56a338fa35cbe3582160ae4f34e`); API35 `12872631908` (`sha256:bd32cb1a21e26a257c429bc74bd0158f41c819072af835204f953232dc4f8978`); PR #147 native diagnostic `12883594935` (`sha256:e4ed10a1ec5d14a38bbf2e861b30b196d7e3b8e0e325dc1869df3c3c63760cb1`). PR #151 Android artifact IDs/digests are listed above. No #157/#158 hosted artifact exists at capture; #157 local test output hash `6701848a3a17941aed8496f8ad448bbcf5401eaae550183ac9765cb60f45ad04` is a local log, not an artifact.
**SECURITY_IMPACT:** Documentation-only; records fail-closed review boundaries, current product evidence, artifact/provenance limitations and Owner approval boundary.
**REVIEW_STATUS:** PR #157 ACCEPTED for source only, hosted API26/35 tests queued; PR #158 ACCEPTED for source at exact SHA `26645df…`, hosted focused run queued. Native runtime remains BLOCKED; Android/G9 product acceptance remains PARTIAL.
**KNOWN_ISSUES:** #151 API26/35 Storage and Emulator is running without a result; #152 Unit/Security/Change Scope passed but remaining Android jobs are queued; #153 Android UI jobs are queued; #154 Android workflow overall succeeded while all product test jobs were skipped; #155 Android run is pending with `jobs=[]` and product evidence absent; #157 Android Change Scope is queued with no assigned runner; #158 current Android run is pending with `jobs=[]` and focused Native run is queued; #149 API26 UI and API35 IME diagnostic failed; native runner exception cause is unresolved; APK inner hash/signer is unverified; Owner decision ID sources conflict.
**NEXT_ACTION:** Continue tracking #155 integration and #157/#158 exact-head workflows; append artifacts/JUnit when available; no Native runtime acceptance until the exact current candidate demonstrates inference, Usage/Egress and verified cleanup; do not merge.
**OWNER_APPROVAL_REQUIRED:** Beta package and coexistence; OAuth/project/scopes; Gmail Restricted scope; cloud provider/eligibility/cost; model redistribution/default; production signing and distribution; Pixel 7a access; main merge; Personal Beta GO.
