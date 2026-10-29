# G8 Orchestrator Ledger — G7 Closure, OAuth UI, Native Runtime, G9 Preflight

**Checkpoint:** 2026-10-29 JST (2026-10-28 UTC)
**Roadmap:** Owner-provided `Serea_Roadmap_v4.md`; campaign 기준일 2026-10-29 JST.
**Overall (pre-integration checkpoint):** G7 **SUCCESS** — evidence closure; G8 **PARTIAL**; G9 Preflight **PARTIAL**; Personal Beta **NO-GO**. Active integration candidate: Draft PR #146. Its last pre-integration checkpoint was HEAD `55fc9ff9f0675154ab5242f52e1ee84d97c0ee41`, tree `db4b495521824705dac2e33b0a084695522a163a`. The accepted OAuth consent fix from PR #145 is integrated later in this ledger; use the PR metadata and the source checkpoint below for the resulting current HEAD. Historical startup statuses are retained as dated history and are superseded by later correction sections.

## Authority and candidate baseline

| Ref | Observed state |
|---|---|
| Repository | `Yrika819/Serea`, public; GitHub metadata reports push permission for the connected identity |
| Protected `main` | `e0acb0254820c381dd97828824bef86a1db5176c` (confirmed with `git ls-remote`; unchanged) |
| Frozen G5 PR #134 | OPEN/DRAFT, HEAD `b84feb337cd57918346ab31cb3ea4af4ba880ffb`, tree `9133d4a3ba2ab870d126e56cdd22336b856c94e1` |
| G6 PR #138 | OPEN/DRAFT, HEAD `23b884cf2815a2f3287652d5ffd2c3913d09df76`, tree `f77fea2f7800dbf8daa1f5f25175c52f951f31a4`; SUCCESS — Functional Readiness Candidate |
| G7 PR #139 | OPEN/DRAFT, branch `codex/serea-g7-calendar-readiness`, HEAD `00384eb6468214362435140c9a2d848fa2e0f12e`, tree `10d07a8ec57d7f67fc94663c489347938cccbd58` |
| G7 code/test source | `acde94c3c0f81ac2e76d0b0e51e48fd84589ebeb`, tree `f2fa81955f4c8834b83073ef4d239c831ff4f773` |
| G8 parent PR #141 | OPEN/DRAFT, branch `codex/serea-g8-integration-candidate-20261029`, based on G7 #139; current source before this checkpoint update `b06f20e0dd1870a6eb0db684634603b9da3bb353`, tree `16f18279889c4e2aefea3aa1fb57118048b923fd` |
| G7 parent | G6 candidate `23b884cf2815a2f3287652d5ffd2c3913d09df76`; G6 depends on frozen G5 PR #134. No candidate PR is merged to `main`. |

PR metadata was refreshed through GitHub on 2026-10-28 UTC. The local G7 candidate ref was fetched from `refs/pull/139/head` and its commit/tree/parent match the GitHub PR metadata. The initial workspace checkout was `main` at the protected SHA and clean; no existing work was overwritten.

**PR #140 identity incident:** the first parent checkpoint used the workspace's default personal author identity and failed the existing Fast CI commit identity guard (Run `40789250204`, failed commit `fca58011119f0374092351401620b16f2565b19d`). No CI rule was changed and no force-push was made. PR #140 was marked **SUPERSEDED / DO NOT MERGE** and remains open; its branch is not used. Corrected parent PR #141 uses GitHub noreply identity; the exact introduced-commit range passes locally and Fast CI Run `40789416642` passes on source `b06f20e…`. Parent commits now use the approved noreply identity.

## G7 closure status

**G7: SUCCESS — Evidence Closure.** Exact final source SHA `00384eb6468214362435140c9a2d848fa2e0f12e` passed all four SQLite portability directions on run `40749231272`, attempt 3. The earlier attempt cancellations remain historical; they are superseded by the successful Intel→ARM64 run recorded below. No G6 SQLite evidence is reused.

| Exact G7 HEAD `00384eb…` evidence | Result |
|---|---|
| Calendar synthetic authorization → loopback MockWebServer focused test | PASS; Run `40749231295`, Artifact `12843121520`, SHA-256 `a198b4746313c27986ab3728f8c7dafc6e060c0a6dd88ad912ef593b689b062e` |
| Android workflow | Run `40749231249` SUCCESS; Build/Lint, Unit, Integration, Security, Dependency Audit, Docs; API 26 and API 35 Required 105/105, Identity, Process Death, Data Isolation, Sentinel. API 26 Artifact `12845730516` (`40090efe7678d2cea8a8e64902fa50298190fd009363ec3c9c276f029cca0ee`); API 35 Artifact `12847400793` (`c0f40a2e03f9f895e9d5be01731c511cfc661e5757a965bf3df1bf06bc473555`); emulator `12846945714`; temporary-key Identity `12846168567` |
| Full / Fast / Native / RustSec / Git Hooks | PASS at this SHA: `40749231298`, `40749231228`, `40749231215`, `40749231235`, `40749231318` |
| SQLite portability, attempt 2 (historical) | Run `40749231272`, attempt 2: Intel→Intel, ARM64→ARM64, ARM64→Intel PASS; Intel→ARM64 (`Open Intel DB on macOS arm64`, job `123042883914`) CANCELLED with `runner_id=0`. This earlier attempt was NOT_RUN for that direction; it was superseded by attempt 3. |
| SQLite portability, attempt 3 (latest) | Run `40749231272`, attempt 3 SUCCESS. All four directions passed; Intel→ARM64 job `123182687879` ran on macOS ARM64 runner `1000022879`. Artifact, JUnit and database integrity evidence is in `docs/agent-reports/evidence/g7-sqlite-arm64-20261029.txt`. |
| Historical Android API 35 Chat/IME failure | On code/test SHA `acde94c…`, `SereaAppShellTest#chatDraftSurvivesGmailAndCalendarDestinationChanges` failed twice (initial + same-SHA retry), with a Compose timeout and then `Composer IME state did not settle at R4-B-committed`. Preserve as a reproducibility/reliability risk. |
| Final G7 HEAD result | On exact final SHA `00384eb…`, later Run `40749231249` passed API 26 and API 35 Required 105/105 and Identity. This does not erase the earlier source-SHA failures. |

Protected `main`, G5/G6 branches, PR #139, and its CI records were not modified. PR #139 remains OPEN/DRAFT. G7 ledger source is `docs/agent-reports/g7-orchestrator-ledger.md` on the candidate.

## New campaign worktrees and delegated lanes

All four branches start from exact G7 final HEAD `00384eb6468214362435140c9a2d848fa2e0f12e`; no same-name local or remote branch existed at setup.

| Owner | Branch / worktree | Scope and ownership |
|---|---|---|
| Parent | `codex/serea-g8-integration-candidate-20261029` — `/workspace/Serea-g8-integration-candidate-20261029`, PR #141 | Central ledger, independent reviews, Composition Root, integration, frozen-head verification and owner decisions. |
| Agent A | `codex/serea-g8-oauth-ui-mock` — `/workspace/Serea-g8-oauth-ui-mock` | Fake Google connection/consent lifecycle and Calendar UI/E2E in dedicated UI/domain/test files. No live OAuth or endpoint. `MainActivity`, `SereaAppShell`, `AppProviderComposition`, and this central ledger are excluded. |
| Agent B | `codex/serea-g8-native-local-runtime` — `/workspace/Serea-g8-native-local-runtime` | Audit G3/G6, native runtime/adapter candidate, JVM/runtime evidence, model integrity and license proposal. No model weights in Git; no normal Chat wiring. Composition Root and central ledger excluded. |
| Agent C | `codex/serea-g8-release-preflight` — `/workspace/Serea-g8-release-preflight`, PR #142 OPEN/DRAFT; HEAD `bb9ee1fd8bc92323c068b2edba3be3f94ba76169`, tree `e16911da53b5ef0c831d175e8b3d15b9bb606a82` | G7 SQLite/IME evidence, official OAuth policy checklist, release/SBOM/license gaps, dependency matrix and G9 preflight. Product files and central ledger excluded. |

Agents are running as actual subagents in this chat. Their commits/branches are not accepted merely because they exist. Parent reviews Source, Architecture, Tests, Security, Integration, Provenance and License/Distribution before integrating any change.

### Parent review disposition — Agent C

**ACCEPTED (documentation only).** Agent C's report changes only `docs/agent-reports/g8-agent-c-release-preflight.md`; it claims JUnit NOT_RUN and contains no product code. Parent independently verified its exact base/head provenance, G7 run/artifact IDs, `runner_id=0` for the missing SQLite direction, and that the historical `acde94c…` API 35 failures are separate from the final `00384eb…` Required/Identity pass. Parent fetched the cited Google API Services User Data Policy, Calendar scopes, Gmail scopes, OAuth verification exception and Android OAuth setup pages directly; current displayed dates and scope descriptions match the report's 2026-10-29 JST check. Local source diff is documentation-only and `git diff --check` passes. The report preserves unresolved Owner and redistribution decisions; no owner-only action is suggested.

Integrated documentation came from Agent C branch head `bb9ee1fd8bc92323c068b2edba3be3f94ba76169` after review. Parent cherry-picked the report commits without rewriting Agent C's branch. Report PR #142 remains OPEN/DRAFT and is a report-only checkpoint, not an accepted product implementation.

## Parent integration plan

1. Keep the default provider Fake and keep Play Services authorization dormant. The UI should consume the existing authorization gateway, consent registry, leases, provider sessions and Calendar authorization adapter.
2. Keep Google connection consent, Google-to-Chat local handoff consent, and Cloud Egress consent as independent operations. A successful connection must not imply handoff or Cloud consent.
3. Calendar HTTP in tests must be loopback-only. Each rendered/read result remains bound to synthetic account, provider session and generation. Stale callbacks and responses must be suppressed after switch, revoke, expiry, cancellation or recreation.
4. Native code remains a separately identified `LOCAL_MODEL` candidate with integrity, offline, resource and lifecycle gates. The product Chat default remains unchanged until independent review and adapter/runtime gates pass.
5. The parent owns `MainActivity`, `SereaAppShell`, `AppProviderComposition`, and any final product-mode selector. Agent work that needs those files is submitted as an integration patch proposal, not an uncoordinated edit.
6. Integrate only reviewed commits. Run focused tests on each lane, then freeze one exact integration HEAD before any formal workflow. CI evidence from a different SHA is not inherited.

## CI plan

Required scope and checks are not weakened. At a stable integration SHA, plan Android Build/Lint, JVM/Integration/Security, API 26 and API 35 Required, Identity, Process Death, Data Isolation, Sentinel, Full, Fast, Native, SQLite, RustSec, Git Hooks, Android Contract, and Release Security. Run the new focused OAuth/Calendar tests and native adapter/runtime tests separately and identify them as non-Required where appropriate. Do not launch heavy Emulator jobs speculatively or retry the G7 runner-unassigned SQLite direction without an assigned runner.

The first corrected parent source `b06f20e…` contains only the initial Orchestrator Ledger. On this exact SHA, Fast CI Run `40789416642` and RustSec Run `40789416463` PASS. The Android workflow `40789416724`, SQLite `40789416511`, Native `40789416519`, Git Hooks `40789416551`, and Full CI `40789416711` were still queued at last observation. Android change-scope classified the source as documentation-only; the API 26/35, Identity, Unit and emulator implementation tests were skipped, so the gate summary is not G8 Android test evidence. The earlier superseded PR #140 failure is a commit-identity failure, not source behavior. All G7 run IDs above remain exact-G7 evidence only. Formal G8 CI has not run on an integrated product-code HEAD.

Focused G8 lane tests are not yet available. Agent A and Agent B independently report the installed Java 21 runtime lacks `javac`; the host is x86_64 with no `/dev/kvm`, Android emulator binary, or attached device. Their focused test results must be recorded NOT_RUN if no usable compiler/runner is found; exact-source GitHub CI remains the next viable path. ARM64 runtime and Pixel 7a remain NOT_RUN.

## Owner decision ledger

| Decision | Status | Required before |
|---|---|---|
| Stable Android package ID and app/demo coexistence | OWNER DECISION REQUIRED | Google Android OAuth client, release manifest and Beta identity |
| Production certificate SHA-1/SHA-256 and signing/temporary-key transition | OWNER DECISION REQUIRED | Real OAuth client and production Beta signing; no Owner key used here |
| Google Cloud project, Calendar API, consent disclosures and tester/verification settings | OWNER DECISION REQUIRED | Any real OAuth activation; not performed |
| Real Google account/Calendar/Gmail access | OWNER APPROVAL REQUIRED | Any real data acquisition; NOT_RUN |
| Restricted Gmail scope / Google verification assessment | OWNER DECISION REQUIRED | Any live Gmail activation; no Gmail OAuth enabled |
| Cloud model provider eligibility, terms, credentials, cost and data policy | OWNER DECISION REQUIRED | Any Cloud provider/API or billing; Cloud remains disabled |
| Pixel 7a physical use | OWNER APPROVAL REQUIRED | Device acceptance; NOT_RUN |
| Production signing, release distribution, merge to protected `main` | OWNER APPROVAL REQUIRED | Release/merge; none performed |
| Personal Beta GO | OWNER DECISION REQUIRED | Final 2026-12-31 release judgment; current status NO-GO |

## G9 preflight independent work

If G8 is incomplete, safe independent preparation continues: Gmail synthetic MIME/HTML and remote-image/size tests, data-minimization contract, model license/native SBOM improvements, token/usage mock UI, and a non-destructive Pixel 7a checklist. No real Gmail, OAuth, Cloud AI, device, production key, or user data is allowed. Agent C's accepted report contains the release/policy and checklist checkpoint; parent will add only work that is independent of G8 acceptance.

## Initial status and resume record — historical, superseded below

- **At initial startup, G7:** PARTIAL — the Intel→ARM64 direction had not yet run. This was later closed by run `40749231272`, attempt 3; see current G7 status above and the evidence closure section below. IME history remains separate from final-head PASS.
- **At initial startup, G8:** IN_PROGRESS — Agent C documentation accepted; Agents A/B were implementing. The later integration checkpoint supersedes this state.
- **G9 Preflight:** PARTIAL — Agent C wrote an accepted preparation checklist; no G9 product test or feature is yet recorded.
- **Personal Beta:** NO-GO.
- **Initial parent branch / Draft PR:** `codex/serea-g8-integration-candidate-20261029` / [PR #141](https://github.com/Yrika819/Serea/pull/141), OPEN/DRAFT; later marked SUPERSEDED / DO NOT MERGE. Active replacement is PR #146.
- **Current integration HEAD at checkpoint start:** `b06f20e0dd1870a6eb0db684634603b9da3bb353` / tree `16f18279889c4e2aefea3aa1fb57118048b923fd`; the report and ledger update are being added on top. No G8 code is integrated.
- **Report branch:** Agent C PR #142 OPEN/DRAFT, report independently accepted and cherry-picked; A/B remain in progress.
- **Superseded PR:** #140 OPEN/DRAFT, title `[SUPERSEDED / DO NOT MERGE]`; no source rewrite or close was performed.
- **Next:** collect each agent's reviewable commit and exact evidence; independently review/revise; wire only accepted OAuth UI in parent Composition Root; make the Native product-integration decision from actual runtime/resource/license evidence; integrate accepted changes and revalidate one frozen HEAD; continue safe G9 preparation.

No asynchronous future resumption is assumed. This file and branch are the resume record.

## Current campaign checkpoint — 2026-10-29

This addendum supersedes the earlier “work is delegated / no G8 code is integrated” status above. Historical G7 failures and exact-head results remain unchanged.

### Current parent candidate

| Item | Current evidence |
|---|---|
| Parent branch/worktree | `codex/serea-g8-integration-candidate-20261029` — `/workspace/Serea-g8-integration-candidate-20261029` |
| Reviewed product/test/tooling source HEAD before this ledger update | `d45d7eb` (`ac4b7c0618f416aa195ca02aa13c7278b02248df` tree); based on G7 final `00384eb…`. This exact source is ready for a normal push to PR #141. |
| PR #141 remote state before this push | OPEN/DRAFT; remote head `3b20faac13907146858f7aa8ce5c2832f47175c4`. The local accepted source commits have not yet been pushed at checkpoint creation. |
| Changed scope | Fake OAuth/Calendar UI composition, Activity recreation/account-switch UI test, G9 adversarial Gmail UI test, API35 exact focused UI stage and fail-closed package cleanup helper. No production OAuth activation or Native Chat wiring. |
| Required manifests | Required 103 and Additional 2 remain untouched (105 formal identities total). Non-required 19; source identities 124; overlap 0, missing 0, unclassified 0. |

### Lane dispositions

| Lane | Disposition and evidence |
|---|---|
| Agent A — Fake OAuth/Calendar | **ACCEPTED for source integration after security fix.** PR #145 latest `f857b29a9b2c12e61b347eed595a5dfa51d3aba9`, tree `a7b713ec89c39167250017d5fa86f5745d3d0f42`. Parent review found and returned `G8-AUTH-STALE-CONSENT-01`; the immutable preview fix at `63b459d…` plus cross-feature stale-Confirm regression at `f857b29…` now pass parent review. Confirm/Deny/Cancel are bound to one displayed preview, account, feature, scopes and account generation. The parent integrates only the reviewed files; Android UI runtime remains NOT_RUN pending exact-head CI. The app shell composes separate connection, local Handoff and Cloud consents; no live OAuth/provider URL is enabled. |
| Agent A — focused JVM/compile | Parent ran on app/test source HEAD `e7dff90947631d15429d299f28da8dc86b3a42e9`, tree `36df3fa991ce3b06c04f7c450b696cb8aadbd8eb`: JVM 13/13 PASS (9 lifecycle, 1 Calendar mock E2E, 3 loopback integration), plus `:app:compileIsolatedAndroidTestKotlin` PASS. JUnit XML and SHA-256 are saved under `docs/agent-reports/evidence/g8-parent-focused-e7dff90/`. No Android emulator execution is included in this result. |
| Agent A — UI/lifecycle limits | The parent API35 stage contains 6 Fake Google/Calendar UI methods, the app-shell recreation/account-switch integration test, and the Fake Gmail inert-HTML test (7 exact identities total). It has not run yet. Activity recreation is covered by source test; OS process-death for the OAuth ViewModel remains NOT_RUN. |
| Agent B — Native Runtime | **NOT ACCEPTED for product integration; quarantined candidate only.** PR #144 OPEN/DRAFT, report tip `3e8644338599bee5c0b0554a7b3de8a9234c8095`, tree `48e43dddfe3ce64b36f0a6735b2587106af88fe5`; implementation source `ee5c25c9ca7139db9d4fb387c87e5d3922b0c4d8`, tree `5b4f19c66bbb6f922f4e19f3d184e2bfe87aff83`. It remains default-off and disconnected from normal Chat. Focused JVM PASS 22/22. Product JNI x86_64 inference is NOT_RUN; focused run `40792192393`, job `123179857937`, was QUEUED with no artifact. ARM64_BUILD=NOT_RUN, ARM64_RUNTIME=NOT_RUN, PIXEL7A_PHYSICAL=NOT_RUN. The CMake dirty-checkout fix is pushed; its independent re-review is pending. Complete native NOTICE/SBOM/attribution and redistribution review, actual memory/latency/power evidence, and runtime remain open. |
| Agent C — Release/Security | Report-only PR #142 remains OPEN/DRAFT and its report was independently accepted. OAuth policy checklist is documentation only; no Cloud project/client/service was configured. C reviewed the parent Composition Root and API35 UI-stage design, requested fail-closed ADB package-query handling, then **ACCEPTED** the fix after re-review. |
| Agent C / G9 Fake Gmail | PR #143 OPEN/DRAFT, tip `d93674154ee0a5297a3a55e9ad25edabcfdf914b`; test source `b07f91612b28a1f9338f04dd081ef3a94180c84f`. Parent cherry-picked the synthetic adversarial HTML UI test and report (`3641eaa`, `e7dff90`). Parent-run isolated Android test-source compilation passed on G9 source `6afd718e8e2e3882f7f07ebc3e8d3b97ed67032a`, tree `923aeaf2868ce886b433fcf8e1094722f746f398`; no UI JUnit was produced. This test remains non-required and will run only in the focused API35 stage. |

### Parent integration and verification

- Parent source commit `348da9c` wires the current Fake authorization ViewModel through Settings and the app shell, opens the loopback agenda from the current consent session, and carries the existing local-only Handoff flow. `GoogleAuthorizationAppIntegrationTest` exercises consent, agenda opening, Activity recreation, and account switch. Independent review by Agent C found the session/account and consent boundaries consistent with the existing egress checks.
- Parent commit `d45d7eb` adds the API35 post-Required focused stage with these seven exact non-required test identities: four `GoogleAuthorizationDemoUiTest` methods, `GoogleCalendarDemoLoopbackUiTest#consentUiOpensLoopbackAgendaAndAccountSwitchDisconnectsIt`, `GoogleAuthorizationAppIntegrationTest#settingsConsentOpensCalendarSessionAndActivityRecreationReturnsToConsentPanel`, and `GmailHtmlInertUiSecurityTest#fakeGmailRendersAdversarialHtmlAsInertTextWithoutRemoteRequests`.
- The focused stage is only eligible after API35 formal Required JUnit, Process Death, and Data Isolation all PASS. It uses exact test identity verification and runs after the Required set; API26 records NOT_REQUIRED. Required IDs, assertions, thresholds, skip conditions, and Additional manifest are unchanged.
- Independent review found that failed `adb shell pm path` could previously appear to mean “absent.” `tools/android_package_path.sh` now distinguishes an ADB error from a successful empty result. The diagnostic precheck and every focused-stage pre/post query fail closed; any uninstall or query error prevents cleanup/API35 PASS. Agent C accepted the fix. Python regression tests exercise transport failure (exit status 2), a successful absence, and an installed path.
- On the final committed test-stage source `d45d7eb`, `python3 -m unittest tests.android_emulator_workflow_tests` passed 32/32; Bash syntax and `git diff --check` passed; source/manifest identity accounting is 124/103/2/19 with zero mismatch. These are host/static tests, not Android UI execution.
- The seven focused UI methods, Android API26/35 formal gates, Identity, Process Death, Sentinel, Full, Fast, Native, SQLite, RustSec, Git Hooks, Android Contract and Release Security have no run IDs yet for `d45d7eb`; they are awaiting the parent branch push. Runs at the older parent source `3b20faac…` are stale and cannot be used as evidence for this candidate.

### G7, G9, Owner gates, and status

- G7 is **SUCCESS — Evidence Closure**. The statement in the earlier checkpoint that attempt 3 was queued with `runner_id=0` was accurate only before its later completion and is superseded: job `123182687879` passed on runner `1000022879`; all four directions passed on exact G7 source SHA `00384eb6468214362435140c9a2d848fa2e0f12e`. See the SQLite evidence closure correction below.
- G9 Preflight is **PARTIAL**: the Fake Gmail adversarial HTML UI source and release checklist are saved; isolated Android source compilation passed; focused JUnit/UI execution is NOT_RUN. Gmail parsing, size boundaries and data minimization not yet covered by this current Gmail UI test remain future G9 work.
- G8 is **PARTIAL** pending exact-HEAD Android/UI CI, Native JNI inference and ARM64/resource/license evidence. Personal Beta remains **NO-GO**.
- Existing Owner decisions remain required for live OAuth/Google data, Cloud provider/billing/credentials, package/signing, production distribution, physical Pixel 7a acceptance, `main` merge and Beta GO. None was performed.
- No future asynchronous resume is assumed. The next actions are: push the reviewed parent branch to PR #141; collect exact-source workflows and artifacts; record any runner failures as NOT_RUN/cancelled rather than product failures; independently re-review Agent B’s provenance fix when its re-review arrives; continue independent G9 mock/preflight work while Native runtime is queued or unavailable.


## Identity-guard remediation and exact d0 verification checkpoint — 2026-10-29

The source snapshot above was pushed to PR #141 as exact head `d0f32fcf845208f065e8edbdb498ec00448a1b65` / tree `f5a5f8d977638ab08ec3b3c1719e43d737fb425c`. Fast CI run `40794667396` failed only at its commit identity guard (job `123188095512`). The guard passed checkout, Rust setup/fetch, workspace smoke guard, and smoke regression, then rejected committers on `aa878eae952e3e5ce668d0f192f47d48d611d6df`, `ed0bf401b859cfa2aac43b1695a17af80d9d9c9c`, `348da9c18722e526ebab3ee580e4db97c7f882b5`, `3641eaace85a03fbc98be8bf53470fc11e21af7c`, and `e7dff90947631d15429d299f28da8dc86b3a42e9`. The first two and `3641eaa`/`e7dff90` kept an accepted author but inherited the local personal committer; `348da9c` had the personal author and committer. No source tests ran after this guard failure. No identity policy was changed.

To preserve history, no commit was amended or rebased and no branch was force-pushed. Existing PR #141 remains open and will be marked **SUPERSEDED / DO NOT MERGE**. The identical reviewed source tree is being snapshotted from G7 final onto the separate branch `codex/serea-g8-integration-candidate-20261029-r1`; that branch will contain one ordinary G8 integration commit with approved GitHub noreply author and committer. This is an identity-safe replacement candidate, not an overwrite of PR #141. The local identity guard must pass before it is pushed/opened as a new Draft PR.

### Local exact-source verification at d0f32fc

- `:app:assembleDebug :app:lintDebug`: **PASS**, BUILD SUCCESSFUL in 3m19s (44 tasks; 24 executed, 20 up-to-date).
- Gmail Fake/MIME/session/read-port/ViewModel JVM group: **70/70 PASS** (Gmail parser and security ports 16, provider sessions 13, HTTP read port 11, Gmail ViewModel 30).
- Google-origin egress, body lineage, usage, Calendar security/session, OAuth, and trusted-handoff JVM group: **88/88 PASS**.
- Mock release/security tests: lockfile SBOM **7/7 PASS**, artifact-bound SBOM **10/10 PASS**, Firebase signing/distribution contract **19/19 PASS**. Release/emulator guard shell syntax passed.
- Local CycloneDX 1.5 inventory contains 317 locked components and is bound to the CI-only debug APK hash. APK SHA-256: `b9189b8331bc94f58b703924eb1104b555dd71e49f97c513c9f4c8a35aaf192d`; bound SBOM SHA-256: `5de2529e71b6c1e33b41e74b7834ec7d1bb142eede3a84e8df16f5fe47a76a6c`. Evidence is saved under `docs/agent-reports/evidence/g8-release-security-d0f32fc/`. The APK itself is not committed. This lockfile inventory does not represent dependency graph edges, independently establish packaged membership, publisher authenticity or APK-to-source attestation.
- Focused JUnit XMLs for the 158 Gmail/egress/consent/usage tests are saved with SHA-256 under `docs/agent-reports/evidence/g8-parent-focused-d0f32fc/`. Earlier G8 OAuth/Calendar tests and instrumentation source compile are saved under `g8-parent-focused-e7dff90`.

### Exact d0 GitHub run states at the latest check

| Run | State/result |
|---|---|
| Android | `40794667213` PENDING at check-suite level; no job/runner assigned yet. |
| Full CI | `40794667223` QUEUED. |
| Native platform validation | `40794667224` QUEUED. |
| Cross-architecture SQLite portability | `40794667256` QUEUED. |
| Git hooks Windows compatibility | `40794667202` QUEUED. |
| Rust Dependency Security | `40794667240` QUEUED. |
| Fast CI | `40794667396` FAIL at the identity guard described above; code tests were skipped. |
| Release Security Focused | `40795288341` QUEUED on exact d0 via workflow_dispatch. |
| Mock Personal Beta Contract Tests | `40795297504` QUEUED on exact d0 via workflow_dispatch input `head_sha=d0f32fc…`. |
| Feature-branch Android Personal Beta workflow | Run `40794660333` showed event `push`, conclusion `failure`, and zero jobs/logs/artifacts. The feature branch's workflow file has only `workflow_dispatch` and its job is guarded to `refs/heads/main`; no manual dispatch was issued. No Beta APK build, Owner signing, or distribution occurred. This zero-job workflow event is not product or release test evidence. |

All of these run IDs are tied to exact SHA `d0f32fc…`; do not attribute any to the replacement branch SHA. The exact-G7 SQLite job remains queued/unassigned as recorded above. Agent C has now independently **ACCEPTED** Agent B's CMake provenance fix at source `ee5c25c9ca7139db9d4fb387c87e5d3922b0c4d8`; the native candidate is accepted only as a quarantined, default-off candidate. B's focused JNI workflow remains queued without runtime evidence.

G8 and G9 remain **PARTIAL**; Personal Beta remains **NO-GO**. The next candidate push must pass the exact-source identity guard first, then run formal CI on its own SHA. No successful test evidence from d0f32fc may be reused as a different replacement SHA's result without documenting unchanged source scope and obtaining that SHA's required CI.


## Replacement branch exact-source checkpoint — 2026-10-29

The identity-safe candidate now exists on `codex/serea-g8-integration-candidate-20261029-r1`, based directly on G7 final HEAD `00384eb6468214362435140c9a2d848fa2e0f12e`. Its snapshot commit is `fb47d5027cb1f27988c3b40c56688540ae7d969f`, tree `55a17e762bf21d98a629fd182ce1d6259bb5916a`. The commit author and committer are both `Yrika819 <143304522+Yrika819@users.noreply.github.com>`. Local `tools/check_commit_identity.py` passed for the exact `pull_request` range (one introduced commit). The reviewed Android/app/test/tooling blobs are the same as the rejected `d0f32fc…` candidate; the replacement commit is a one-commit snapshot from the same G7 base, not a rebase, amend, or force-push of #141. `git diff --check`, Bash syntax, manifest accounting (124 source / 103 Required / 2 Additional / 19 non-required; no mismatches) and SHA verification of saved evidence all pass.

On exact snapshot `fb47d50…`, parent reran 14 focused JVM test classes plus Android test-source compilation: **171 JVM tests PASS**, zero failures/errors/skips; `:app:compileIsolatedAndroidTestKotlin` PASS; Gradle BUILD SUCCESSFUL in 1m41s (55 tasks executed). The 14 JUnit XML files and hashes are saved under `docs/agent-reports/evidence/g8-parent-focused-fb47d50/`. No Android emulator UI or physical device test is represented by this run. The app source tree is unchanged from the previously locally linted `d0f32fc…` candidate; the replacement adds the same tested app/test/tooling files on the exact G7 base.

This branch is the active parent candidate. It will be pushed normally and opened as a Draft PR after the exact commit-identity guard passes locally; PR #141 remains open and must not be merged. Runs `40794667213` through `40795297504` remain evidence only for the separate old SHA `d0f32fc…`; no result will be carried onto `fb47d50…`.


## Active replacement Draft PR and CI handoff — 2026-10-29

The identity-safe replacement branch is now pushed and opened as **Draft PR #146**: `https://github.com/Yrika819/Serea/pull/146`, base `codex/serea-g7-calendar-readiness`. At this checkpoint its exact head is `a1f22f2550afdfc55acea1a7af9bc520bd28ea09`, tree `3b68ab749827f860faa2167d4b55d2f66c3127a8`. Its two introduced commits (`fb47d50…` source snapshot and `a1f22f2…` evidence/checkpoint update) both use approved GitHub noreply author/committer identities; the local exact-range guard passed for two commits. The second commit only records checksummed JUnit evidence and this ledger status; no product/test/tooling file changed after the `fb47d50…` run.

PR #141 was not closed, rebased, or force-pushed. Its title/body now mark it **SUPERSEDED / DO NOT MERGE** and point to #146. PR #141 remains OPEN/DRAFT at failed-identity head `d0f32fc…` for audit. G7 PR #139, G5 #134, G6 #138, and lane PRs #142–#145 remain open; none was closed or merged.

### Exact checks for the preceding active replacement head (historical)

For the preceding exact PR #146 head `a1f22f2…`, GitHub created Android `40796324758`, Native `40796324946`, Full `40796325050`, Git Hooks `40796324553`, RustSec `40796324505`, Fast `40796324508`, and SQLite `40796324477`. At last check all were QUEUED; no run had yet produced a job result. These are distinct from #141’s d0 runs and will be superseded by the latest exact-head workflows once this ledger update is pushed.

The feature-branch-only Android Personal Beta workflow again appeared as push run `40796293362` on `a1f22f2…`, conclusion FAILURE with zero jobs/logs/artifacts. It was not manually dispatched. Its branch file remains `workflow_dispatch`-only with `if: github.ref == 'refs/heads/main'`; no Beta build/sign/distribution occurred. This is not release evidence.

Release Security Focused `40795288341` and mock Personal Beta Contract Tests `40795297504` remain QUEUED on old exact SHA `d0f32fc…`; do not reuse them as evidence for #146. The SQLite queued status in the older checkpoint was superseded by G7 run `40749231272`, attempt 3, which passed job `123182687879` on ARM64 runner `1000022879`.

The #146 worktree and branch are the active continuation point. Its exact source snapshot `fb47d50…` has 171 JVM PASS plus isolated Android test-source compilation PASS, with JUnit XML under `g8-parent-focused-fb47d50/`. Local `assembleDebug`/`lintDebug`, 70 Gmail + 88 egress/consent/usage JVM tests, and mock release-security/SBOM work are recorded against the earlier identical app/test source tree `d0f32fc…`. Android UI execution, formal API26/35, process-death on the authorization session, Native runtime and ARM64 remain NOT_RUN until exact-source evidence arrives.

No Owner-only action was performed. G8 and G9 remain PARTIAL; Personal Beta is NO-GO.


## G7 SQLite evidence closure correction — 2026-10-29

The exact-G7 Intel→macOS ARM64 SQLite gap has now **PASSED**. G7 is **SUCCESS — Evidence Closure**, not PARTIAL. Exact source/test SHA is still `00384eb6468214362435140c9a2d848fa2e0f12e`, tree `10d07a8ec57d7f67fc94663c489347938cccbd58`; the successful CI run was run `40749231272`, attempt 3, overall SUCCESS. Its previously unassigned job `Open Intel DB on macOS arm64` completed SUCCESS as job `123182687879`, runner `1000022879`.

All four exact-source directions passed in that run: Intel produce job `123182727042`; ARM64 produce `123182743251`; ARM64→Intel open `123182690038`; Intel→ARM64 open `123182687879`. The latter executed `closed_serea_database_can_be_produced_or_consumed_cross_architecture`: 1 passed, zero failures/ignored/measured/filtered. Input Intel artifact `12843411146`, output ARM64 validation artifact `12843930249`. Both databases contain 626,688 bytes and the same SQLite SHA-256 `1a248d00016ed601c3c718e48389d3aec64804e0a2e48ab82cb7b33315fe5383`; read-only `PRAGMA integrity_check` returned `ok` for each. ZIP hashes and timestamps are saved in `docs/agent-reports/evidence/g7-sqlite-arm64-20261029.txt`, now present on the updated G7 base branch.

Earlier exact-G7 runner-unassigned cancellations remain intact as historical evidence. The successful attempt 3 is a later exact-source execution and closes that one residual; it does not borrow G6 SQLite results. PR #139 remains OPEN/DRAFT at report-only head `0560e1e7bcd13f0194eab8caca89d70ea5a2f142`, tree `ea12eec70afb172e610fbae4c8cea95c5061aaa7`, based on G6; its code/test source remains `00384eb…`. The G8 replacement branch is now merging this evidence-only G7 checkpoint into its base without rebasing or changing app/test code.

Exact G7 Android API26/35 Required 105/105 and Identity passes remain as recorded. Historical API35 failures on `acde94c…` remain an IME reliability note, not a current G7 gate failure. No Required gate was relaxed. Personal Beta remains NO-GO.

### G8 status after G7 closure

G8 remains **PARTIAL**: Fake OAuth/Calendar source is accepted after the stale-consent fix; parent independently ran 36 focused lifecycle/loopback/HTTP JVM tests on PR #145 head `f857b29…` with zero failures, and the Android test-source compile passed on the preceding code head `63b459d…` (no Android source changed in `f857b29`). The integration candidate's formal API26/35, focused UI and other exact-head checks remain pending. Native is separately reviewed and default-off; x86_64 inference, ARM64 build/runtime, memory/performance and complete license/SBOM evidence remain NOT_RUN. G9 preflight remains PARTIAL; Fake Gmail parser/security JVM coverage and test-source compile exist, but focused Android UI execution remains NOT_RUN. Personal Beta remains NO-GO.


### Exact checks at the pre-correction #146 head `4cd887a`

The base-evidence merge head was `4cd887af09f81d3a2d9401986ae27d2bd2ea3210` / tree `8082586e899df9d9900ea00f50baabc16ae4589c`. GitHub reported 26 check runs tied to that exact SHA: 25 queued/pending and one Native-platform job in progress; none had completed PASS or a test failure when reviewed at 2026-10-29 00:55 UTC. Runs: Android `40796897532` (pending, no job record yet); Native platform `40796897670` (job `123195523147` in progress, 11 queued); Full `40796897674` (5 queued); Fast `40796897677` (queued); SQLite `40796897607` (2 queued); Git Hooks `40796897563` (3 queued); RustSec `40796897555` (queued); Release Security `40796900490` (queued); mock Personal Beta Contract `40796905335` (queued). A correction-only ledger commit follows this recorded head, so these runs are historical for `4cd887a`; do not attribute them to the later HEAD.

The push-triggered Personal Beta workflow run `40796891332` on `4cd887a` concluded FAILURE with zero jobs or artifacts. The workflow is main-only and its feature-branch event did not execute a build, signing or distribution step. This is not a product test failure or release evidence. No Beta workflow was manually dispatched.

## Independent OAuth stale-consent review and correction — 2026-10-29

At parent review of the pre-fix integration source, issue `G8-AUTH-STALE-CONSENT-01` was **HIGH / CHANGES_REQUESTED**: the consent panel's no-argument Confirm callback read mutable `pendingFeature` at callback time, so a delayed callback from one preview could authorize a later feature/account preview. No actual Google credential or endpoint was involved; this still violated the explicit-consent/fail-closed contract. The structured finding and accepted-source boundary are recorded in PR #146's body.

Agent A fixed the issue on PR #145 without touching Composition Root files. `GoogleDemoConsentPreview` now captures a unique preview identity, feature, requested/displayed scopes, synthetic account and account generation. Confirm, Deny and Cancel callbacks carry that exact preview object; stale, replaced or replayed callbacks are rejected by referential-current-preview and account/scope/generation checks. Confirm consumes the preview synchronously before queuing authorization work. The stale path returns before Fake grant, authorization, Calendar session creation or any HTTP dispatch.

Parent reviewed PR #145 source HEAD `63b459d1ac1b2a177ad752aa57a19a8bc9a8decd` and follow-up test HEAD `f857b29a9b2c12e61b347eed595a5dfa51d3aba9`, tree `a7b713ec89c39167250017d5fa86f5745d3d0f42`. The initial 35/35 run did not include the required cross-feature case, so parent requested `G8-AUTH-STALE-CONSENT-TEST-02`; Agent A added `staleCalendarConfirmCannotSubstituteForLaterGmailPreview`. Parent then independently ran all selected tests in a detached worktree on exact `f857b29…`: **36/36 PASS**, zero failures/errors/skips (Lifecycle 13, Calendar loopback integration 3, Calendar HTTP read port 20). The additional regression preserves the Gmail preview and both `NEEDS_CONSENT` states, and observes no Calendar session creation or MockWebServer request. JUnit files and SHA-256 are saved in `docs/agent-reports/evidence/g8-parent-oauth-stale-consent-f857b29/summary.txt` and adjacent XML files.

`G8-AUTH-STALE-CONSENT-01` and `G8-AUTH-STALE-CONSENT-TEST-02` are now **ACCEPTED** for candidate integration. `:app:compileIsolatedAndroidTestKotlin --rerun-tasks` passed on preceding source HEAD `63b459d…`; the only `f857b29…` change is JVM test code, so the Android and instrumented-test sources are unchanged. Android UI runtime remains NOT_RUN and must be resolved by exact integration-head CI. The Agent A PR stays OPEN/DRAFT for audit; no PR was merged to `main`.

## Native workflow pin and release-report correction

Agent B's Native product candidate remains quarantined/default-off and is **NOT ACCEPTED for product Chat integration**. Parent independently accepted only the focused-workflow pin fix on PR #144 head `ee5467f791c66912e95fdc86ccdf435f48d5aa33`, tree `dfcb723f01dae1be21bc8ac4c12dc4a447759b09`. Its single-line workflow diff changes a 39-character emulator-runner revision to full commit `a421e43855164a8197daf9d8d40fe71c6996bb0d`, verified against the upstream `v2.38.0` tag. Focused run `40797306274`, job `123196792146`, is still QUEUED with no steps or artifacts; prior failures were action-resolution failures before setup. No JNI inference ran. `ARM64_BUILD=NOT_RUN`, `ARM64_RUNTIME=NOT_RUN`, `PIXEL7A_PHYSICAL=NOT_RUN`; the local host has no emulator, NDK, CMake or KVM.

Agent C's report-only PR #142 remains OPEN/DRAFT at `fa28794f1958f901a169573ac19ca954d5eb76fa`, tree `0333327f28b08e070bdf1e0b8eb108164455cc08`. Parent reviewed its docs-only correction: it reports G7 SUCCESS on exact source `00384eb…`, preserves attempts 1/2 cancellations as history, and records attempt 3 plus the artifact/JUnit evidence. The independent Google policy/release checklist remains documentary; Owner gates remain unresolved.

## Exact integrated G8 source checkpoint — 2026-10-29

Parent integrated only the reviewed OAuth preview binding and its tests from PR #145 into the separate G8 candidate branch. Code-bearing checkpoint: `00d554b56c93a27704748fa2923b0c0a180588ed`, tree `fbd019ed91b7dc9f5e776f0873ff8e895b76ab9f`; branch `codex/serea-g8-integration-candidate-20261029-r1`, Draft PR #146. This candidate remains based on G7 final source and the G7 evidence-only update. The subsequently added evidence/ledger commit changes documentation only and is described in the PR metadata; no source files changed after the verification below.

At exact code-bearing HEAD `00d554b…`, the full Android JVM task passed **404/404** tests (34 suites, zero failures/errors/skips). `tools/verify_android_unit_junit.py` passed source/JUnit discovery and complete-result checks. XML, `SHA256SUMS`, and a test summary are saved under `docs/agent-reports/evidence/g8-parent-integrated-00d554b/`.

At the same exact code-bearing HEAD, `:app:assembleDebug :app:lintDebug :app:compileIsolatedAndroidTestKotlin` passed (70 tasks; BUILD SUCCESSFUL in 2m01s). The debug/demo APK is 12,368,206 bytes, package `app.serea.android.demo`, version `0.1.0-demo` / code 1, min SDK 26 / target 35, SHA-256 `e0c67ece2cad3833e5d84d88c31b0d918a853091d76230438480696d5392d14d`. It is not an Owner-signed Beta artifact. A CycloneDX 1.5 inventory lists 317 locked components; an APK-bound BOM and explicit scope limitations are saved under `docs/agent-reports/evidence/g8-release-security-00d554b/`. The inventory does not include dependency graph edges, attest packaged membership/publishers, cover build plugins/wrapper/SDK, or prove APK-to-source reproducibility.

Release/SBOM contract tests passed **36/36** (`android_artifact_sbom_tests`, `android_firebase_distribution_tests`, `android_lockfile_sbom_tests`). These mock contract checks do not authorize signing or distribution. Android emulator UI, API26/35 formal Required, Identity, Process Death, Data Isolation, Sentinel and all PR workflows remain dependent on the exact pushed integration HEAD; no UI/runtime PASS is claimed here.
