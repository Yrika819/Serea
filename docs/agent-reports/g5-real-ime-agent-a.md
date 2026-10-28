# G5 Agent A — Real IME B/C Closure Evidence

## Report contract

AGENT_ID: A — Real IME / Formal Recovery  
PHASE: G5 Agent A  
STATUS: SUCCESS (independent B/C diagnostic matrix only; non-Required)  
BASE_SHA: `b84feb337cd57918346ab31cb3ea4af4ba880ffb`  
CURRENT_HEAD: report-only checkpoint commit on the branch listed below; exact object SHA is reported in the Orchestrator handoff.  
SOURCE_TREE_SHA: `9133d4a3ba2ab870d126e56cdd22336b856c94e1`  
WORKTREE: `/workspace/Serea-g5-ime-report`  
BRANCH: `codex/serea-g5-agent-a-ime-report-20261028`  
DRAFT_PR: none; this is a report-only local checkpoint branch  

OBJECTIVE: Independently verify the exact-head API 26/35 Real IME B/C diagnostics and classify the earlier seed-preflight failure.  
CHANGED_FILES: `docs/agent-reports/g5-real-ime-agent-a.md` only; no product or test source changed.  
TESTS: No tests or emulators were launched by this reviewer. Read-only review of Run `40664188678` and Run `40650170070` artifacts, JUnit, logs, workflow jobs, and exact-source test code.  
CI_RUN_IDS: `40664188678` (current exact-head matrix); `40650170070` (old preflight failure); `40657482497` (distinct earlier run; see distinction below).  

## Current exact-head matrix

GitHub identifies Run `40664188678` as completed SUCCESS at PR #134 head `b84feb337cd57918346ab31cb3ea4af4ba880ffb`. The run is non-Required. All five jobs succeeded: one APK build/provenance job and four serial fresh-AVD cases. Each case artifact digest below was independently checked against the downloaded ZIP SHA-256. Each artifact also records `expected_source_sha`, checked-out SHA, and APK provenance source commit as the same `b84feb337cd57918346ab31cb3ea4af4ba880ffb`; source tree is `9133d4a3ba2ab870d126e56cdd22336b856c94e1`.

All four cases ran the exact named JUnit method once: `tests=1`, `failures=0`, `errors=0`, `skipped=0`. They used separate fresh AVD jobs and the same APK set built once. API 26/35 results here do not replace the formal Required Gate.

| Case | Artifact ID / SHA-256 of artifact ZIP | JUnit identity / result | InputConnection evidence | Composition and selection evidence | Cleanup |
|---|---|---|---|---|---|
| API 26 B — continuous | `12806470650` / `a3eea7a42aab4f7155456bc0c8f4bd18eba8dce38a14712dd98c2d568c319637` | `IndependentImeCompositionTest#B_realImeCompositionWithoutSynchronizationBoundaries`; 1/0/0/0; PASS | IME generation 3; non-null `InputConnectionWrapper@93897949`; editor package `app.serea.android.isolated`; inputType 163841; composing/replacement/finish calls accepted with matching service/session/connection/package. `onStartInput` observed. | Initial TextFieldValue/semantics empty, composition null, selection 0. Final TextFieldValue is `draft survives navigation`, composition null, selection 25; Semantics selection 25. Window focus and `acceptingText` true. B intentionally does not observe/assert intermediate Compose state between calls; only final reflected state is awaited. | PASS; LatinIME restored and verified, Test IME disabled/uninstalled and package absent. |
| API 26 C — synchronized | `12805802286` / `03d5896e2828fec286269ed55306664679ad15b991b27caa9d3d86528c81497e` | `IndependentImeCompositionTest#C_realImeCompositionWithSynchronizationBoundaries`; 1/0/0/0; PASS | IME generation 3; non-null `InputConnectionWrapper@41667454`; editor package `app.serea.android.isolated`; inputType 163841; `onStartInput`, `onStartInputView`, and matching accepted operations observed. | Initial empty/null/selection 0. After draft: `draft`, composition 0..5, selection 5; Semantics selection 5. After replacement: `draft survives navigation`, composition 0..25, selection 25; Semantics selection 25. After finish: same text, composition null, selection/Semantics selection 25. Window focus and `acceptingText` true. | PASS; LatinIME restored and verified, Test IME disabled/uninstalled and package absent. |
| API 35 B — continuous | `12806207252` / `84fa00f32b3ceda4639fc380d02f496b3af7dd2a8b031d9de8e670781533c219` | `IndependentImeCompositionTest#B_realImeCompositionWithoutSynchronizationBoundaries`; 1/0/0/0; PASS | IME generation 4; non-null `RemoteInputConnection@18400711`; editor package `app.serea.android.isolated`; inputType 163841; composing/replacement/finish calls accepted with matching service/session/connection/package. `onStartInput` and `onStartInputView` observed. | Initial TextFieldValue/semantics empty, composition null, selection 0. Final TextFieldValue is `draft survives navigation`, composition null, selection 25; Semantics selection 25. Window focus and `acceptingText` true. B does not observe/assert intermediate Compose state between calls. | PASS; LatinIME restored and verified, Test IME disabled/uninstalled and package absent. |
| API 35 C — synchronized | `12806182929` / `6519c898dae4d8f73909686dfdfbd569201b2c28e57a7696653b77dbda99a42b` | `IndependentImeCompositionTest#C_realImeCompositionWithSynchronizationBoundaries`; 1/0/0/0; PASS | IME generation 4; non-null `RemoteInputConnection@217746153`; editor package `app.serea.android.isolated`; inputType 163841; `onStartInput`, `onStartInputView`, and matching accepted operations observed. | Initial empty/null/selection 0. After draft: `draft`, composition 0..5, selection 5; Semantics selection 5. After replacement: `draft survives navigation`, composition 0..25, selection 25; Semantics selection 25. After finish: same text, composition null, selection/Semantics selection 25. Window focus and `acceptingText` true. | PASS; LatinIME restored and verified, Test IME disabled/uninstalled and package absent. |

### Shared APK provenance

The one-time build artifact is ID `12805232327`, ZIP digest `123f927c8d0b991cf42b7f498a60023cc88201e48bb093b8f82a880850af291e`. Its SHA list matches each case's installed APK SHA records:

- Target `app.serea.android.isolated`: `8d191c826563548f737d2aed32ac5cddeed335df51e32351f74acc15f5a0deed`
- Instrumentation `app.serea.android.isolated.test`: `6c94fa4c0d75f8fe0d122e0ee0557560804276526c8ac2816592166abc76d221`
- Test IME `app.serea.android.r25testime`: `0abdec6c2bdf1da952531c6ccf94dc15eb44163c8cf17e59cc9e5ae3506dd30d`

All case artifacts report installed hashes equal to expected hashes. The build job passed the diagnostic source-scope, guard, isolated Test IME permission/component, signer-equality, and source/APK provenance checks. The IME diagnostic permission is install-time signature permission; test log reports the target package permission check as granted, with no runtime permission grant attempt.

## Earlier Run 40650170070: preflight failure, not IME failure

Run `40650170070` concluded FAILURE. GitHub run metadata lists head SHA `029c5aa6267fe3b4c44ac6fd5b082ed1c763b140`; the per-case artifact's checked-out `source_commit` is synthetic PR merge commit `3892d0a2c1f735686583c7041c336629939a8739`, tree `e2659452f5d150449d3e99a25664355132f3b574`. Git commit metadata confirms the synthetic merge parents are base `373b8be15f407d951157d2a80a632e86d979acdf` and PR commit `029c5aa6267fe3b4c44ac6fd5b082ed1c763b140`. The merge tree equals the PR commit tree.

For all four old case artifacts, `result.txt` says `stage_at_exit=fresh-data-seed`, status 1, and `junit-status.txt` says `NOT_RUN`; there is no instrumentation JUnit XML. This means JUnit did not run (it is not a zero-test run), and no IME behavior was tested. The seed logs themselves contain `SEED_OK` and `PASS: fresh isolated namespace received a one-use host seed without starting the app.` The old runner script instead required the different literal `PASS: empty isolated app storage authorized`, then emitted `FAIL [fresh-data-seed]: Campaign seed helper did not report its explicit PASS marker.` This is a TEST_HARNESS marker-contract mismatch after a successful seed helper, not an Emulator or product IME failure.

Old artifact IDs and verified ZIP SHA-256:

- API 26 B: `12799308439` — `cbc0b66ee53053ddec3934833dc4b745d90dcf5d04525f597caf1e488e5d77b4`
- API 26 C: `12799403754` — `f6f114b4b580a078e4d21a2ba5b2114ebd7d195d1602fbd2940c736b49015398`
- API 35 B: `12799413213` — `e5407283b2a07e3d54703a56662d703fca19eaf242c45830e9c56215ca6e4fa1`
- API 35 C: `12800520284` — `b63bbfa42944971e8e1ae58f17489845df16ca9c7e602cd681f78cdc06e52b28`

Run `40657482497` is a separate PR #133 run at source head `b06c1acd5f3ddb4f59f0cad6ae1a1d291b086671`. The Orchestrator reported that it crossed the provider IPC boundary. Its behavior/result is not inferred from the older seed-preflight artifacts above and is not part of the exact-b84 four-case matrix.

## Root-cause and disposition

ROOT_CAUSE: The failure in Run `40650170070` is CONFIRMED as a harness marker mismatch. Product IME root cause is UNKNOWN / not reproduced in this matrix.  
SECURITY_FINDINGS: Exact target package binding and current-session identity are logged; wrong-package, unapproved fixture, missing package, and wrong input-type probes are rejected before operation. Test IME remains isolated to diagnostics APK/AVD.  
REMAINING_RISKS: B checks accepted consecutive IME operations and final committed state but deliberately lacks intermediate TextFieldValue/semantics synchronization assertions; C supplies these stage-by-stage checks on both APIs. All results are diagnostic/non-Required and do not certify formal Required Gate, navigation/activity recreation, real service integration, or Personal Beta readiness.  
INTEGRATION_READY: YES for preserving this diagnostic evidence; no product code change is supported by these results.  
NEXT_ACTION: Orchestrator may close Agent A's B/C diagnostic task, retain the artifact IDs/digests, and treat any later source changes as requiring new exact-head evidence.  
OWNER_ACTION_REQUIRED: None for this diagnostic review. Personal Beta remains NO-GO pending the separate required, security, real-service, release, and Owner approval gates.

## Formal API 26 Required artifact follow-up

This follow-up records the separate Formal API 26 artifact from Run `40664188621`; it does not change or extend the non-Required matrix result above. The workflow run was still `IN_PROGRESS` when inspected, with the Android Storage and Emulator job active, so this artifact alone is not a final whole-run conclusion.

Artifact ID: `12807200481` (`serea-android-instrumentation-api26`)

Verified ZIP SHA-256: `1ca4464caf0197ce04f5312e0f163f56728739c1719cdd7fc1616d6773cc8993`

Exact source: run head, source checkout SHA, and candidate SHA all `b84feb337cd57918346ab31cb3ea4af4ba880ffb`; API level 26.

The formal JUnit XML contains 105 test cases, failures 0, errors 0, skips 0. I compared the exact XML identity set to the manifests at the same b84 source: the Original Required manifest contains 103 identities, the Additional manifest contains 2, the manifests are disjoint, and XML equals their union exactly (missing Required 0, extra 0, missing Additional 0). The verifier log independently records `Original Required Set: 103/103 PASS; Additional Tests: 2/2 PASS; total=105`.

The two Additional identities are:

- `app.serea.android.UsageLedgerDatabaseTest#fakeProviderBridgeRoutesApprovedDataAndKeepsCostAndUnknownOutcomesDurable`
- `app.serea.android.tasks.TasksDatabaseTest#approvedProposalMutationsAndSingleUseReceiptsSurviveDatabaseReopen`

The Formal IME acceptance identity `app.serea.android.SereaAppShellTest#chatDraftSurvivesGmailAndCalendarDestinationChanges` is present in the 105-case XML and passed. Its test log confirms the Android Test IME session bound to `app.serea.android.isolated`, non-null InputConnection, inputType 163841, and accepted composition calls. In the same test, the draft transitions through `draft` composition 0..5, `draft survives navigation` composition 0..25, then committed text with composition null and selection 25; Semantics selection matches. The test subsequently records insertion after Gmail and Calendar returns and after Activity recreation, with the restored cursor position checked by the passing test. Test IME cleanup records restoring LatinIME and disabling the diagnostic service.

The separate Process Death artifact markers are all positive: `process-death-status.txt=PASS`, readiness status PASS, host lifecycle exit code 0, and `process-death.log` ends with a PASS for Android OS Process Death recovery, encrypted history/Usage Ledger recovery, no replay, Delete All, and demo sentinel preservation. The readiness log has a `NOT_READY` line followed by `PASS: freshly installed isolated target accepted the Process Death seed`; the status file and subsequent full process-death result are PASS.

Data isolation status is PASS. Its log records the debug/demo package `app.serea.android.demo`, isolated target `app.serea.android.isolated`, instrumentation package `app.serea.android.isolated.test`, demo history and `model_selection` sentinel seed and verification, and sentinel token `serea-api26-40664188621-1`. It explicitly states no demo history mutation was requested. The diagnostics cleanup status is also PASS.

`junit-verification.log` separately records ProfileVerifier as unsupported on API 26 (`resultCode=262144`, embedded=false, marker absent); the verifier still reports the instrumentation JUnit threshold PASS. This does not change the JUnit counts above. Formal results for the other API and other workflow jobs must be judged from their own artifacts and the final Run conclusion.

