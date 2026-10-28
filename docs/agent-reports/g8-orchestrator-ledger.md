# G8 Orchestrator Ledger — G7 Closure, OAuth UI, Native Runtime, G9 Preflight

**Checkpoint:** 2026-10-29 JST (2026-10-28 UTC)
**Roadmap:** Owner-provided `Serea_Roadmap_v4.md`; campaign 기준일 2026-10-29 JST.
**Overall:** IN_PROGRESS — G7 is PARTIAL; G8 work is delegated; Personal Beta remains NO-GO.

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

**G7: PARTIAL.** The reviewed mock/readiness source is complete, but one cross-architecture SQLite direction remains NOT_RUN because its macOS ARM64 runner was not assigned. Do not convert G6 SQLite evidence into G7 evidence.

| Exact G7 HEAD `00384eb…` evidence | Result |
|---|---|
| Calendar synthetic authorization → loopback MockWebServer focused test | PASS; Run `40749231295`, Artifact `12843121520`, SHA-256 `a198b4746313c27986ab3728f8c7dafc6e060c0a6dd88ad912ef593b689b062e` |
| Android workflow | Run `40749231249` SUCCESS; Build/Lint, Unit, Integration, Security, Dependency Audit, Docs; API 26 and API 35 Required 105/105, Identity, Process Death, Data Isolation, Sentinel. API 26 Artifact `12845730516` (`40090efe7678d2cea8a8e64902fa50298190fd009363ec3c9c276f029cca0ee`); API 35 Artifact `12847400793` (`c0f40a2e03f9f895e9d5be01731c511cfc661e5757a965bf3df1bf06bc473555`); emulator `12846945714`; temporary-key Identity `12846168567` |
| Full / Fast / Native / RustSec / Git Hooks | PASS at this SHA: `40749231298`, `40749231228`, `40749231215`, `40749231235`, `40749231318` |
| SQLite portability | Run `40749231272` incomplete. Intel→Intel, ARM64→ARM64, ARM64→Intel PASS. Intel→ARM64 (`Open Intel DB on macOS arm64`, job `123042883914`) CANCELLED with no runner assignment; G7 ledger/PR records two attempts with runner ID 0. This direction is NOT_RUN, not a product/code failure. No repeated retry is planned absent runner availability. |
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

## Status and resume contract

- **G7:** PARTIAL — SQLite Intel→ARM64 open direction NOT_RUN due runner assignment; IME history retained separately from final-head PASS.
- **G8:** IN_PROGRESS — Agent C documentation accepted; Agents A/B are implementing. No G8 product implementation is yet accepted.
- **G9 Preflight:** PARTIAL — Agent C wrote an accepted preparation checklist; no G9 product test or feature is yet recorded.
- **Personal Beta:** NO-GO.
- **Parent branch / Draft PR:** `codex/serea-g8-integration-candidate-20261029` / [PR #141](https://github.com/Yrika819/Serea/pull/141), OPEN/DRAFT.
- **Current integration HEAD at checkpoint start:** `b06f20e0dd1870a6eb0db684634603b9da3bb353` / tree `16f18279889c4e2aefea3aa1fb57118048b923fd`; the report and ledger update are being added on top. No G8 code is integrated.
- **Report branch:** Agent C PR #142 OPEN/DRAFT, report independently accepted and cherry-picked; A/B remain in progress.
- **Superseded PR:** #140 OPEN/DRAFT, title `[SUPERSEDED / DO NOT MERGE]`; no source rewrite or close was performed.
- **Next:** collect each agent's reviewable commit and exact evidence; independently review/revise; wire only accepted OAuth UI in parent Composition Root; make the Native product-integration decision from actual runtime/resource/license evidence; integrate accepted changes and revalidate one frozen HEAD; continue safe G9 preparation.

No asynchronous future resumption is assumed. This file and branch are the resume record.
