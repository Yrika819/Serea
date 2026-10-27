# G1 — Git Integration & Release Architecture

**Updated:** 2026-10-27 UTC
**Status:** SUCCESS for the scoped G1 audit and disposable merge rehearsal. No source integration or main merge was performed.

## Agent report contract

| Field | Value |
|---|---|
| AGENT_ID | G1 — Git Integration & Release Architecture |
| ACTIVE_PHASE | G1: dependency/provenance audit and main integration rehearsal |
| OVERALL_STATUS | SUCCESS (G1 deliverables complete; integration approval remains separate) |
| BASE_SHA | `e0acb0254820c381dd97828824bef86a1db5176c` |
| CURRENT_HEAD | See Draft PR #124 head; initial report commit was `4b01fc5822b724c0205b90f615c300b40ca95716` (parent `e0acb0254820c381dd97828824bef86a1db5176c`, tree `0229f8098806230751ed21332c038c7a86692616`) |
| TREE_SHA | The G1 report commit above has tree `0229f8098806230751ed21332c038c7a86692616`; latest follow-up is reported with the final agent response and PR metadata |
| WORKTREE | `/workspace/Serea-g1-main-integration-readiness` |
| BRANCH | `codex/serea-g1-main-integration-readiness` |
| DRAFT_PR | [#124 — G1 integration provenance and merge rehearsal](https://github.com/Yrika819/Serea/pull/124), Draft; audit only |
| CI_RUN_IDS | Existing exact-#123 evidence: `40427448635`, `40425595932`, `40425866784`, `40427448605`, `40427448681`, `40427448836`, `40427448709`, `40427448616`, `40427448704`, `40427448688` |
| ARTIFACT_IDS | Required run `40427448635`: `12703250122`, `12702701890`, `12702662029`, `12702479957`, `12701024855`, `12700521566`, `12700507955`, `12700382715`, `12699894031`, `12699669873`. Focused run `40425595932`: `12700535333`, `12700187700`, `12699214853`. Release-security run `40425866784`: `12699746258`. |

## Authority snapshot

| Ref | Base | Head | State |
|---|---|---|---|
| `main` | — | `e0acb0254820c381dd97828824bef86a1db5176c` | protected; unchanged |
| PR #98 | `main` at `e0acb0254820c381dd97828824bef86a1db5176c` | `00e0453f83970a2b7d2bd26ce01462e421f75f46` | open Draft |
| PR #99 | PR #98 head `00e0453f83970a2b7d2bd26ce01462e421f75f46` | `8140bd759e7a01041364ebc443ba452f546d325a` | open Draft |
| PR #100 | PR #99 head `8140bd759e7a01041364ebc443ba452f546d325a` | `ad868af753672c546c52411ce151904360633acc` | open Draft |
| PR #117 | PR #100 head `ad868af753672c546c52411ce151904360633acc` | `151ea9d66d32de01b195f4262beb67516823c281` | open Draft; Frozen R7 baseline |
| PR #122 | PR #117 head `151ea9d66d32de01b195f4262beb67516823c281` | `b64a517ec33dd672c17cd604a912045116ad963a` | open Draft |
| PR #123 | PR #117 head `151ea9d66d32de01b195f4262beb67516823c281` | `3924b36080ffa43331f065a15db80c604c243355` | open Draft; Frozen Functional Candidate |

The ancestry graph is `main → #98 → #99 → #100 → #117 → (#122, #123)`. The `main` commit is an ancestor of both #117 and #123. `main..#117` contains 339 commits; `main..#123` contains 409 commits and changes 241 paths (61,892 insertions and 110 deletions). The stack is reviewable as an ordered sequence; it is not necessary to rewrite or force-push any branch. A direct main merge of #123 would include the whole chain and bypass the intended per-PR review sequence, so the recommended route is to land approved stack PRs in order, then choose #123 as the final functional child.

## PR #122 / #123 semantic comparison

The two heads have common merge base `151ea9d66d32de01b195f4262beb67516823c281`; neither head is an ancestor of the other. Tree comparison against that base found:

- #122 changes 45 paths; #123 changes 61.
- No #122-changed path is absent from #123's changed-path set.
- Of 45 shared changed paths, 39 have identical blobs at both heads; six have later content in #123.
- #123 adds 16 paths.

The six evolved shared paths are:

| Path | Change in #123 relative to #122 |
|---|---|
| `.github/workflows/android-functional-focused.yml` | Adds a focused-candidate branch trigger; it remains explicitly non-required. |
| `android/app/src/androidTest/java/app/serea/android/ChatHttpE2ETest.kt` | Uses a fixed catalog clock and relative freshness timestamps, removing time-dependent fixture behavior. |
| `android/app/src/androidTest/java/app/serea/android/SereaAppShellTest.kt` | Adds foreground/window-focus recovery, validates the live IME connection's editor package/session/type, captures a replacement connection after IME rebinding, and strengthens the Tasks-return path. |
| `android/app/src/androidTest/java/app/serea/android/SereaIsolatedAndroidJUnitRunner.kt` | Updates diagnostic identity documentation; runner filtering behavior is unchanged. |
| `android/app/src/main/java/app/serea/android/MainActivity.kt` | Adds compact-screen Chat actions and a local fake-only Google consent walkthrough in Settings. |
| `tools/run_android_functional_focused.sh` | Expands the non-required Focused list from 26 to 33; installs/verifies/cleans up the isolated Test IME and checks exact test identities/count. |

The 16 #123-only additions are:

1. `.github/workflows/native-platforms.yml`: serializes the synthetic host lifecycle test (`--test-threads=1`).
2. `android/app/src/androidTest/assets/serea-non-required-test-identities.txt`: excludes the three new Google demo UI tests from the formal Required selection.
3. `android/app/src/androidTest/java/app/serea/android/ChatScreenTest.kt`: routes “new conversation” through compact-screen overflow and updates the corresponding label assertion.
4. `android/app/src/androidTest/java/app/serea/android/GoogleAuthorizationDemoUiTest.kt`: covers fake consent grant/deny/cancel, expiry/revocation/account-switch, and local disconnect.
5. `android/app/src/androidTest/java/app/serea/android/tasks/TaskScreenTest.kt`: scrolls the saving indicator into view before asserting it.
6–7. `GoogleAuthorizationDemoPanel.kt` and `GoogleAuthorizationDemoViewModel.kt`: implement a clearly labeled synthetic consent UI with no real account, network, Play Services, or retained token lease.
8. `android/r25-ime-test/.../RealImeDiagnosticsProvider.java`: resolves and validates the Android-bound editor connection/session/package on the IME service thread before operating it.
9–11. `crates/serea-host/src/main.rs`, `src/test_readiness.rs`, and `tests/host_lifecycle.rs`: add explicit synthetic readiness evidence and its host lifecycle coverage.
12. `tests/android_emulator_workflow_tests.py`: tests the bounded, non-mutating IME readiness helper and stable reboot sequencing.
13. `tests/android_instrumentation_results_tests.py`: updates the non-required diagnostic identity count from 5 to 8 while checking disjointness from Required identities.
14. `tools/run_android_instrumentation_campaign.sh`: waits for a default IME and records readiness diagnostics; the required manifest/count checks remain unchanged.
15. `tools/test_personal_beta_identity.sh`: makes reboot/root recovery serial-specific, bounded, and checks emulator stability.
16. `tools/wait_for_android_default_ime.sh`: adds bounded polling without changing IME selection or enablement.

No #122-only code or required test path was found. #123 is the more complete sibling snapshot; do not merge #122 in addition to #123. Keep #122 open until the repository owner decides whether to mark it superseded.

## Lane B/C provenance

PRs #120 and #121 were checked as related inputs, not merged.

- PR #120's head delta from its recorded base `d74998f60c9062343ce9c62a326343e9a4d18e45` is four paths. The release posture document, SBOM tests, and SBOM generator are byte-identical in #123. Its release-security workflow is also present in #123, with additional integration-branch and path filters. No unique #120 implementation needs cherry-picking into #123.
- PR #121's head delta from its recorded base `7d1ddfdc18425901af1889f3d4730e8e5624a664` is two adversarial unit-test paths; both are byte-identical in #123. No unique #121 code needs cherry-picking into #123.
- These PRs remain independent Drafts. Their recorded base SHAs differ from the #117 base of #123, so preserve their own CI/provenance and do not infer that their branch histories are interchangeable.

## Required CI contract audit

Compared PR #117, #122, and #123 source trees:

- `tools/android-instrumentation-required-tests.txt`: 103 unique identities; identical blob SHA `e49fe3fb80c39da56ec5f8f7d9f7137cf52c5e47` at all three heads.
- `tools/android-instrumentation-additional-tests.txt`: 2 unique identities; identical blob SHA `2572f80bfbc2ecbe667b39d2edc1aeb01a0e5920` at all three heads.
- `tools/verify_android_instrumentation.py`: identical blob SHA `dd027d419f28902a79a57f348fd83f80ef626913` at all three heads.
- The 103, 2, and 10 diagnostic identities are pairwise disjoint. Required test count, selection, and verifier were not relaxed.
- Formal campaign still asks the verifier for minimum and exact count 105, required count 103, additional count 2, the identity manifests, and per-class required minima. Process Death, Data Isolation/Sentinel, Formal Identity, and zero-failure/error/skip checks remain in the workflow.
- The only `android.yml` additions after #117 are installation of the `libpulse0` host runtime package for the API 35 emulator jobs. The instrumentation campaign adds bounded default-IME readiness diagnostics, but does not alter formal counts or thresholds.
- Focused workflow is named “Non-Required” and its 33/33 result must not substitute for formal 105/105 Required evidence.

GitHub reports run `40427448635` (Required), `40425595932` (Functional Focused), and `40425866784` (Release Security Focused) as successful at exact head `3924b36080ffa43331f065a15db80c604c243355`. Other exact-head successful runs: Fast `40427448605`, Full `40427448681`, Native `40427448836`, SQLite `40427448709`, RustSec `40427448616`, Git Hooks `40427448704`, Android Contract `40427448688`.

Artifact metadata confirms Required-run artifacts `12703250122`, `12702701890`, `12702662029`, `12702479957`, `12701024855`, `12700521566`, `12700507955`, `12700382715`, `12699894031`, and `12699669873`; Focused artifacts `12700535333`, `12700187700`, and `12699214853`; Release Security artifact `12699746258`. Attempts to download JUnit artifacts from this environment returned blob-storage HTTP 403, so G1 did not independently parse the XML. Run status and artifact metadata are confirmed; artifact payload contents should be re-opened by the integration reviewer if exact testcase-by-testcase inspection is required.

## Disposable main integration rehearsal

Created a second, local-only worktree and branch from the exact main SHA:

- Worktree: `/workspace/Serea-g1-main-merge-rehearsal`
- Branch: `codex/serea-g1-main-merge-rehearsal`
- Initial/base SHA: `e0acb0254820c381dd97828824bef86a1db5176c`
- Rehearsal merge command: `git merge --no-ff --no-edit 3924b36080ffa43331f065a15db80c604c243355`
- Result: merge completed with no conflicts; no remote ref was changed.
- Merge commit: `898bf799d8eba2aec5b43253fc8c29b4ef750527`
- Merge parents: `e0acb0254820c381dd97828824bef86a1db5176c` and `3924b36080ffa43331f065a15db80c604c243355`.
- Result tree: `41cfce316c485abe31f86c827c6ca81a4d5c107b`, exactly the #123 tree; `git diff --exit-code 3924b36080ffa43331f065a15db80c604c243355 HEAD` was clean.

This establishes mechanical mergeability from current main without modifying main or any PR ref. It does not replace the ordered PR review, branch-protection checks, or post-merge CI. The rehearsal branch/worktree is preserved for inspection and was not pushed.

## Integration sequence, rollback, and residual risks

Recommended route, only after review/approval of each PR:

1. Merge #98 into `main`.
2. Rebase/retarget only through normal GitHub PR base advancement by landing #99 after #98, then #100 after #99, then #117 after #100. Do not rewrite branch history.
3. For the functional child, select #123. Its source tree retains all #122-changed paths, evolves six shared paths, and adds the 16 listed paths. Do not separately merge #122, #120, or #121 when their exact reviewed code is already represented in #123; preserve their PRs and run evidence as provenance.
4. Re-run formal Required and release checks on the resulting final main candidate SHA. Prior passes attach to #123 SHA, not a later merge commit.

Rollback plan: record the pre-merge main SHA and merge commit for every step; if a landed step must be undone, submit a new protected revert PR using the appropriate mainline parent (`git revert -m 1 <merge-commit>` for a merge commit), then run the same Required and release checks. Do not reset, force-push, or rewrite protected history. Verify migration/update compatibility before rollback if persistent schemas have advanced.

Residual risks / blockers:

- No PR was merged and no production release was attempted. Owner approval is still required for main integration.
- #123 passes its reported CI at its own head, but those results do not transfer to a later merge SHA.
- PR #123 changes 61 paths on its immediate #117 base and spans 409 commits / 241 paths from current main. Review its stacked provenance rather than treating “ancestor of main” as equivalent to already reviewed.
- Real Google OAuth/accounts, real AI credentials/eligibility, production signing/distribution, and Pixel 7a acceptance remain outside this G1 scope and must remain disabled or NOT_RUN.
- `git diff --check` finds one trailing whitespace in the #123 owner-decision ledger (`docs/plans/2026-10-26-personal-beta-owner-decision-ledger.md:3`); this is documentation-only and does not affect the rehearsal tree.

## Verification record

Commands/evidence used:

- `gh pr list` and `gh api repos/Yrika819/Serea/pulls/{98,99,100,117,120,121,122,123}` — current PR heads, bases, state, and dependencies.
- `git merge-base`, `git rev-list --count`, and `git diff --shortstat` — ancestry and main-relative scope.
- Blob SHA/path-set comparison between #117, #122, and #123; `git diff` inspection of all six evolved shared files and the 16 additions.
- SHA/count/disjointness comparison of Required, Additional, and non-required identity manifests; workflow and verifier diff inspection.
- `gh run view` for the exact-head run IDs above; GitHub reports all listed runs completed successfully at #123 HEAD.
- `gh api .../actions/runs/{id}/artifacts` — artifact IDs and sizes. `gh run download` was attempted for formal API 26/API 35 and focused API 26/API 35 payloads; each returned HTTP 403 from GitHub's artifact blob host. No JUnit result was fabricated from missing payloads.
- Disposable merge rehearsal described above; main and all existing PR branches were not modified.

### Review request / next action

Review the dependency sequence and the six evolved shared files, then retain #123 as the selected functional candidate if no additional source-level issue is found. The G4 orchestrator should record this provenance and carry forward the exact-head run/artifact IDs. Owner action is required only for protected-main merge and eventual real-service/signing/device steps; those are not part of this G1 completion.

## Independent security review of G4 Fake authorization integration

**Review date:** 2026-10-27 UTC
**Review result:** `CHANGES_REQUESTED` for live Google OAuth readiness; the reviewed G4 Fake-only lifecycle changes are accepted as isolated demo/test code.
**Inspected G4 source:** `ebe7048452fea3a8642266c989436927033020ff` (tree `4af5c88ec6976c9e5ed24d25910ca06ae5277b37`), based on PR #123 head `3924b36080ffa43331f065a15db80c604c243355`.
**G4 PR:** #125, Draft, base PR #123's head branch, seven changed paths.
**Review mode:** read-only; no G4/G1 source was changed for this review.

### Scope and reviewed behavior

The G4 diff from #123 is limited to the Google authorization demo UI/ViewModel and Fake gateway, the demo instrumentation/unit tests, and G2/G4 reports. The five changed source/test files are byte-identical to G2's reviewed source branch. G4 adds no real OAuth, network, or provider activation path:

- `GoogleAuthorizationDemoViewModel` takes a concrete `FakeGoogleAuthorizationGateway`; the demo text says it uses no real Google account, OAuth UI, network, mail, or calendar data.
- `AppProviderComposition.fakeDefault()` remains the runtime composition. `PlayServicesGoogleAuthorizationGateway.realOAuthEnabled` still defaults to `false`; Gmail and Calendar provider sessions also retain their Owner gate and Fake default.
- Manifest, permissions, backup rules, scopes, real provider adapters/transports, Composition Root, Cloud/Egress code, and workflows are unchanged from #123. The existing manifest has only the Internet permission, `allowBackup="false"`, and `usesCleartextTraffic="false"`.
- Existing Google allowlists remain read-only (`gmail.readonly`, `calendar.events.readonly`, optional `calendar.calendarlist.readonly`). No logging statements or credential data were added. Access token leases keep their redacted `toString()` behavior.
- Existing Cloud egress remains disabled by default. The cloud gate accepts only `USER_AUTHORED`/`MODEL_OUTPUT` origin and PUBLIC classification; Google-origin Gmail/Calendar content is rejected. No new Google-to-Cloud route was added.

The changed Fake lifecycle code synchronously reserves one consent operation, rejects a competing request, invalidates queued Fake grant operations on revoke/disconnect, closes stale leases, and blocks direct consent while the app-local feature is disabled. `sharedRevocationPreservesAppDisconnectUntilExplicitReconnect` covers disconnect → Fake shared revoke → blocked request → explicit reconnect → fresh consent-required state. This does not establish real OAuth behavior.

### Findings — real provider path, unchanged by G4

**G1-SEC-GOOGLE-REVOKE-01 — MEDIUM: app-local disconnect can be erased by a concurrent real grant revoke.**

- **Evidence:** `GoogleAuthorization.kt:301–307` adds/removes `disabledFeatures`. `revokeGoogleGrant()` marks authorization in flight, suspends while awaiting Play Services at `:337–357`, then unconditionally clears `disabledFeatures` at `:358–361`. The G4/G2 Fake now deliberately preserves the independent app-local disable (`FakeGoogleAuthorizationGateway.kt:113–119`), so its behavior differs from the real adapter.
- **Reproduction:** with an enabled real gateway, start `revokeGoogleGrant()` and hold its Play Services completion. Call `disableInApp(GMAIL)` while the revoke is suspended, then complete revoke successfully. The success block clears the just-added disable flag; `isEnabledInApp(GMAIL)` becomes true without explicit reconnect.
- **Required correction:** before live OAuth, preserve or explicitly merge the app-local feature state when completing a shared grant revoke. Add a controlled concurrent-revoke test asserting that a feature disabled during revocation stays disabled until explicit re-enable, and that fresh consent is still required after the shared grant is revoked. The G2 checkpoint records this parity gap and says not to wire real OAuth until it is reconciled.
- **Current impact:** latent. G4's demo owns only the Fake gateway; the ordinary app UI does not compose the real gateway.

**G1-SEC-GOOGLE-REVOKE-02 — MEDIUM: an in-flight Gmail/Calendar result can be delivered after real feature disable or grant revocation.**

- **Evidence:** `disableInApp()` and successful `revokeGoogleGrant()` change feature versions, invalidating a lease's callback, but they do not close/invalidate provider sessions or cancel their active read jobs (`GoogleAuthorization.kt:301–307, 356–364`). `GoogleGmailAuthorizationAdapter.accessToken()` and `GoogleCalendarAuthorizationAdapter.accessToken()` check the session/feature before returning a lease; HTTP transports use `lease.useToken()` to construct the Authorization header but return parsed response data without a post-response lease/generation check (`GmailHttpReadPort.kt:162–188`, `GoogleCalendarHttpReadPort.kt:132–160`). Provider-session delivery checks whether the provider session is closed/current (`GmailProviderSessions.kt:115–143`, `CalendarProviderSessions.kt:118–144`); gateway disable/revoke does not close that session. Existing tests cover token invalidation before dispatch and account/session replacement before delivery, not revoke/disable after dispatch.
- **Reproduction:** use a delayed MockWebServer response. Start a Gmail or Calendar read and wait until the request has been received. Then call `disableInApp(feature)` or complete `revokeGoogleGrant()`, while leaving the provider session open. Release the response. The current session remains current/open and the transport can return the buffered data to its caller after the authorization action.
- **Required correction:** bind read delivery to a feature/shared authorization generation that changes on disable/revoke, recheck it before publishing response data, and cancel active OkHttp calls where possible. Add delayed-response tests for Gmail and Calendar: hold a response after dispatch, disable the feature or revoke the shared grant, release the response, and assert the result is rejected and no parsed data is delivered. Also assert Gmail-only disable does not cancel an unrelated Calendar read, while shared revoke invalidates both.
- **Current impact:** latent and blocks real Google provider activation. G4 does not create a new path to this issue: real providers are not selected by the app composition and real OAuth remains disabled/not wired.

### Verification and provenance limits

- G2's exact Fake source commit `b87b39d9c86d94bc6121c0605aaccd9497e9694c` (tree `63b0593992ddfdfd3aca5b3da3ffab3bc3b83c80`) has six local JUnit XML files totaling **75 tests, 0 failures, 0 errors, 0 skips**; the files were independently parsed. The G2 report records `:app:compileIsolatedAndroidTestKotlin` passing at that source commit. G4's cherry-picked source files match those G2 source files.
- No real OAuth, real Google account, or live Google endpoint was used. No emulator result is inferred from the local JVM suite.
- Functional Focused run `40491693974` and Release Security Focused run `40492139978` completed successfully at older SHA `68569d093cd9f5f631ca22e0343a78cd366c535b`; they are not current-HEAD evidence. The current G4 head `ebe7048…` has no corresponding hosted API 26/35 or formal Required PASS at review time.
- GitHub PR #125 now points to `ebe7048452fea3a8642266c989436927033020ff` with seven paths and base `3924b360…`. The G4 ledger content still names `2b12dfd…` as CURRENT_HEAD; this is one documentation-only commit behind the PR tip and should be refreshed in its next checkpoint.

**Integration decision:** accept the current G4 changes only as Fake-only lifecycle/demo integration. Do not treat them as evidence of real Google readiness. Resolve both real-provider findings, add the proposed race tests, and rerun provider/security verification on the resulting exact source HEAD before enabling real OAuth.

## G1 Draft PR automation

Creating Draft PR #124 automatically queued broad PR workflows on this documentation-only branch. To limit unnecessary work, G1 submitted cancellation requests for all runs it created. GitHub reports these outcomes at `4b01fc5822b724c0205b90f615c300b40ca95716`:

- Cancelled: SQLite `40487833072`, Fast `40487833076`, and Git Hooks `40487833000`.
- Cancellation requested; status was still in progress at report time: Full `40487833125` and Native `40487833119`.

These runs are not G1 validation evidence. No change to workflow definitions or gate behavior was made.
