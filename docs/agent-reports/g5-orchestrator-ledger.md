# G5 Orchestrator Ledger

## Current checkpoint — 2026-10-28

| Field | State |
|---|---|
| ACTIVE_PHASE | G5 evidence closure completed |
| GOAL_STATUS | SUCCESS — Evidence Closure Candidate; Personal Beta remains NO-GO |
| AUTHORITY_SHA | Candidate b84, based on PR #132 at 373b8be15f407d951157d2a80a632e86d979acdf; main e0acb0254820c381dd97828824bef86a1db5176c unchanged |
| LAST_GOOD_SHA | b84feb337cd57918346ab31cb3ea4af4ba880ffb; tree 9133d4a3ba2ab870d126e56cdd22336b856c94e1 |
| WORKTREE | Candidate: /workspace/Serea-g5-evidence-closure (clean). This report-only ledger branch is based on b84 and does not change candidate evidence. |
| OWNED_BRANCH | codex/serea-g5-evidence-closure-20261028 |
| DRAFT_PR | #134, OPEN/DRAFT, based on PR #132; https://github.com/Yrika819/Serea/pull/134 |
| ACTIVE_WORK | None in the G5 validation runs |
| CURRENT_BLOCKER | Owner-controlled real Google/AI, Pixel 7a, production signing/distribution, main merge, final GO |
| OWNER_APPROVAL_REQUIRED | Real Google OAuth/account data; Cloud AI eligibility/credentials/billing; Pixel 7a; production signing/distribution; main merge; final Personal Beta GO |

## Formal Required and cross-cutting CI

Formal Android Run `40664188621` completed **SUCCESS** at exact candidate b84, all 11 jobs successful.

- API 26: artifact `12807200481`, SHA-256 `1ca4464caf0197ce04f5312e0f163f56728739c1719cdd7fc1616d6773cc8993`. Exact manifest union: 103 Required + 2 Additional = 105; failures/errors/skips 0.
- API 35: artifact `12807019854`, SHA-256 `c8282d542a8e4e24d0d5e93e77f7cd55fb5d9eebd9812c039955e440eb94d8f0`. Exact manifest union: 105; failures/errors/skips 0.
- Identity job `122755666193` passed. Artifact `12808134869`, SHA-256 `237c6c66770fb2b95f22866488506a959ce8a3ddc8541f06b487816197875d4b`: temporary-key v100→101 update accepted; different-key v102 rejected; Beta encrypted Room data survived emulator reboot; Demo data and signer unchanged; Owner key unused.
- Process Death, Data Isolation, Sentinel, Formal IME composition/navigation/recreation, and cleanup passed on both API levels.
- Shared emulator evidence artifact `12807019864` (SHA-256 `ea8cf07f983f549ef6d2ae16805e8239cfd7cd2ea3e107fa5e01a9df787242c4`); Pixel 7a CI bundle `12806769689` is not physical-device acceptance.
- At b84, Full `40664188715`, Fast `40664188732`, Native `40664188790`, SQLite `40664188697`, RustSec `40664188734`, Git Hooks `40664188657`, and mock Beta contract `40664188689` all passed.
- Protected main-only production Beta workflow run `40664180644` had zero jobs on this non-main branch. It was not bypassed; it is not a product or security failure.
- Required manifests and `.github/workflows/android.yml` are unchanged from PR #132. The only added workflow is the non-Required isolated Real IME diagnostic.

## Lane A — Real IME

- Exact-b84 diagnostic Run `40664188678`: API26 B/C and API35 B/C all PASS, one exact JUnit test each, failures/errors/skips 0. Separate fresh AVDs; Android-managed Test IME and non-null package-bound InputConnection; expected composition, selection, APK provenance and cleanup verified.
- Artifacts: API26 B `12806470650`, C `12805802286`; API35 B `12806207252`, C `12806182929`; shared APK build `12805232327`.
- Formal IME test `SereaAppShellTest#chatDraftSurvivesGmailAndCalendarDestinationChanges` passed on both APIs, including Gmail/Calendar return and Activity recreation cursor insertion.
- Historical Run `40650170070` stopped before JUnit: seed helper passed but obsolete marker comparison failed. Confirmed TEST_HARNESS mismatch; no IME operation ran. No product IME failure is established.
- Agent A report-only branch: `codex/serea-g5-agent-a-ime-report-20261028`, HEAD `5479544823dd718813bfbc6e6d22c56b2179d7c7`, tree `66ec6c8be0a80d1ce873f31dc7eb480cf87095b2`.

## Lane B — Cloud Egress lineage

- Confirmed defect: assistant/model-output lineage omitted current `userMessage.sourceLineage`. Root fix carries Gmail/Calendar ancestry into assistant output; bounded-context and first-turn regressions pass.
- Exact-b84 Android Unit artifact `12805840704`: 350/350; Security artifact `12806135450`: 26/26; Cloud lineage matrix 9/9. Rejections occur before usage reservation, credential reads and HTTP dispatch.
- No disclosure was observed on the old path because UNKNOWN classification denied current Cloud dispatch. The bounded-context PUBLIC case was a future-path regression hypothesis, not an observed leak.
- Residuals: no production Gmail/Calendar-to-Chat call site, source-session/account-generation binding or live explicit-consent creation. Arbitrary clipboard provenance is not inferred. **Cloud remains DISABLED**.
- Agent B report-only branch: `codex/serea-g5-cloud-egress-20261028`, HEAD `5ac80e4af28ed165b4574ad9f4b0d1205767fc36`, tree `e9558d0c1e7612c6437d8b4d275bd6afb122e1e8`.

## Lane C — On-device AI

- Standalone PR #127 API 35 x86_64 Emulator Run `40657508541`, artifact `12802681672`, passed 1/1 for model load, generation, multi-turn, cancel, close/reopen and corrupt-model rejection. ARM64 native build passed; ARM64 runtime, current-head API26 inference, Pixel 7a performance and product Chat integration are NOT_RUN.
- PoC remains isolated from Serea Router/Egress/Usage and is not integrated.
- Agent C report-only branch: `codex/serea-g5-ondevice-ai-report-20261028`, HEAD `9e7039b1ebda51e4ab01fc18f2ea0d18ed88e2bd`, tree `3364e138c5a84ff544816155f6da15cad94ab863`.

## Integration / review

- Candidate branch/remote HEAD and worktree are clean at b84. PR #134 remains Draft. PRs #132/#133/#126/#127 and main were not rewritten or merged.
- Code review confirmed the Test IME permission remains isolated to the isolated test target and the Test IME remains a separate test APK.
- Cloud source-lineage fix and regression suite were independently reviewed; no real credentials/account/network dispatch was used.
- No Required test, test identity, skip rule, threshold, or Required workflow was reduced.
- Each lane checkpoint is on its own report-only branch; PR #134 body carries the shared orchestration checkpoint.

## Next actions

1. Keep Cloud and real service paths disabled until production provider-to-Chat source/session/account binding and explicit consent are implemented and reviewed.
2. Keep the on-device model PoC separate until ARM64/device performance, memory lifecycle, model provenance, and Router/Egress/Usage integration are validated.
3. Owner-led real Google OAuth, Pixel 7a non-destructive acceptance, production signing/distribution, main merge, and final Personal Beta review remain pending.
