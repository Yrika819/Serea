# G6-C — Integration, OAuth and Release Readiness

**Checkpoint date:** 2026-10-28 UTC  
**Authority:** frozen G5 PR #134 HEAD `b84feb337cd57918346ab31cb3ea4af4ba880ffb`, tree `9133d4a3ba2ab870d126e56cdd22336b856c94e1`  
**Status:** PARTIAL — documentation, PR graph audit and clean merge rehearsal complete; real OAuth, Owner key and physical-device acceptance remain intentionally NOT_RUN.

## Agent checkpoint

```text
AGENT_ID: C — Integration / OAuth / Release Security
ACTIVE_PHASE: G6-C
GOAL_STATUS: PARTIAL — Owner-ready preparation is complete; live service and production release acceptance are not authorized
BASE_SHA: b84feb337cd57918346ab31cb3ea4af4ba880ffb
CURRENT_HEAD: 9579675d4466c3db2a9b32eeab0db88f029da999 (reviewed submission head; this remote-status checkpoint is documentation-only)
TREE_SHA: c886b51c9a2107e8d9fd3548d937ce922a5635df (reviewed submission tree)
WORKTREE: /workspace/Serea-g6-agent-c
BRANCH: codex/serea-g6-release-readiness
DRAFT_PR: #135 OPEN / DRAFT, base codex/serea-g5-evidence-closure-20261028 at b84feb337cd57918346ab31cb3ea4af4ba880ffb; https://github.com/Yrika819/Serea/pull/135
COMPLETED_WORK: live PR graph/status audit; PR dependency review; no-merge main integration rehearsal; Google OAuth setup/operation checklist; package/signing/update matrix; SBOM gap inventory; Pixel 7a acceptance checklist; gateway/UI composition gap review
CHANGED_FILES: docs/agent-reports/g6-c-release-readiness.md
TEST_COMMANDS: python3 tests/android_lockfile_sbom_tests.py; python3 tests/android_artifact_sbom_tests.py; python3 tests/android_firebase_distribution_tests.py; git merge-tree --write-tree --messages origin/main refs/remotes/origin/pr-134; git diff --check
JUNIT_RESULT: NOT_RUN — documentation-only lane; host release/SBOM Python checks PASS (36 tests total)
CI_RUN_IDS: none started by Agent C
ARTIFACT_IDS: none
SECURITY_IMPACT: none to product behavior; no credentials, OAuth activation, real accounts, signed Beta APK or device were used
DEPENDENCIES: parent G6 composition/review; Owner decisions O-01/O-02/O-05/O-06/O-07/O-08 in the v4 plan
KNOWN_ISSUES: normal app UI selects Fake providers only; OAuth Android certificate SHA-1 and final Owner package remain unknown; current SBOM is a lockfile inventory with limited debug APK binding, not a complete release SBOM
CURRENT_BLOCKERS: Owner approval for real OAuth/account/data, production signing/distribution, Pixel 7a operation and main merge
INTEGRATION_READY: documentation/checkpoint is ready for review; no source integration proposed
REVIEW_REQUEST: independent review of the PR graph conclusion, OAuth scope/verification advice, release evidence gaps and no-merge rehearsal
NEXT_ACTION: parent review; recheck official Google policy and Owner configuration immediately before any future live-service activation
OWNER_APPROVAL_REQUIRED: real Google Cloud project/OAuth/account/data; production signing key and distribution; physical Pixel 7a; merge to main; Personal Beta GO
```

## Live PR and dependency ledger

Live GitHub metadata was fetched on 2026-10-28. The tested/frozen main SHA is `e0acb0254820c381dd97828824bef86a1db5176c`; this lane did not update any remote branch or PR.

| PR | Live head / base | State | Dependency and semantic disposition |
|---|---|---|---|
| [#132](https://github.com/Yrika819/Serea/pull/132) | `373b8be15f407d951157d2a80a632e86d979acdf` → `main` | OPEN / DRAFT; merge state BLOCKED; no review decision | Main-targeted G4 candidate, ancestor of #134. Its exact-head checks on run [40599571275](https://github.com/Yrika819/Serea/actions/runs/40599571275) include **Android Emulator Required Gate = FAIL** (job `122533773745`), **Android Storage and Emulator = FAIL** (job `122527381737`), and **Personal Beta Identity Isolation (temporary signing keys) = SKIPPED** (job `122533674693`). Other checks in that run passed, but #132 itself is not green. It already contains the older functional foundation (#123) and clean G4 lines #129/#130/#131. Its broad cumulative diff (100 commits reported by API) still needs source/evidence review and a green exact-head run before Owner merge review.
| [#134](https://github.com/Yrika819/Serea/pull/134) | `b84feb337cd57918346ab31cb3ea4af4ba880ffb` → #132 branch | OPEN / DRAFT; merge state CLEAN; no review decision | Stacked G5 evidence closure. `git merge-base --is-ancestor pr-132 pr-134` succeeds. Later exact-head run [40664188621](https://github.com/Yrika819/Serea/actions/runs/40664188621) is **SUCCESS, 11/11 jobs**, with API 26/35 Required each 105/105 and Identity PASS; independent IME run [40664188678](https://github.com/Yrika819/Serea/actions/runs/40664188678) passed all four API 26/35 B/C cases. These are evidence on b84feb3, not a change to #132's `373b8be` check conclusions. Preserve G5 evidence and do not substitute another source SHA.
| [#123](https://github.com/Yrika819/Serea/pull/123) | `3924b36080ffa43331f065a15db80c604c243355` → functional integration branch | OPEN / DRAFT; merge state CLEAN | Functional integration ancestor of both #132 and #134. Treat as incorporated through the candidate chain; do not cherry-pick it again.
| [#125](https://github.com/Yrika819/Serea/pull/125) | `2ecec61d8f2c0ec0b00d047b7d226d62307dc742` → #123 branch | OPEN / DRAFT; merge state UNSTABLE; no review decision | Legacy G4 Fake Google lifecycle branch. Its own body says it is superseded by clean-history PR #129. It is not an ancestor of #132/#134. The clean PR #129 and related #130/#131 are ancestors of #132; audit/verify those integrated commits there instead of adding #125 as a second patch stack.
| [#126](https://github.com/Yrika819/Serea/pull/126) | `8ca55a32096bc4449717c2fd3332b0602381d7d5` → #123 branch | OPEN / DRAFT; merge state UNSTABLE | Isolated G3 native AI diagnostic, not an ancestor of #132/#134. Keep isolated until local-model adapter review and product gates are independently satisfied.
| [#127](https://github.com/Yrika819/Serea/pull/127) | `a2bafeea77160e8e2746bdfdd0d309348ac04aa7` → #126 | OPEN / DRAFT; merge state CLEAN | Repair/extension stacked on #126. The pair is a diagnostic lineage; do not import it into product Chat as a side effect of main integration.
| [#129](https://github.com/Yrika819/Serea/pull/129) / [#130](https://github.com/Yrika819/Serea/pull/130) / [#131](https://github.com/Yrika819/Serea/pull/131) | Current G4 readiness, main integration and APK/SBOM evidence branches | OPEN / DRAFT in the PR list | Current heads of #129 and #130 have identical tree `2d8fb30a…`; #131 tree is `941170db…`. All three PR heads are ancestors of #132. Review the implementation/evidence once at #132/#134 rather than re-integrating them independently.

All reviewed PRs are Draft and `reviewDecision` is empty. A `CLEAN` merge state only reports GitHub's mergeability calculation; it is not a human security review or a GO decision. PR #132 has two failed Android jobs and a skipped Identity job on its own head; the newer #134 success is on a distinct descendant SHA and does not make #132 green. Do not close or force-push any branch as part of this ledger.

### No-merge main integration rehearsal

Fetched the live `origin/main` and PR #134 heads, then ran:

```text
git merge-tree --write-tree --messages origin/main refs/remotes/origin/pr-134
=> tree 9133d4a3ba2ab870d126e56cdd22336b856c94e1, exit 0, no conflict messages
```

The result tree matches PR #134's frozen G5 tree. This is a content merge rehearsal because PR #134 descends from PR #132 and both have `origin/main` as their merge base. It created no branch, commit or GitHub merge and did not move `main`. It does not substitute for required CI on any future integration HEAD.

## OAuth readiness and Owner checklist

### Existing implementation and UI gap

- `PlayServicesGoogleAuthorizationGateway` wraps Play Services `AuthorizationClient`, but `realOAuthEnabled` defaults to `false`, requires an explicitly selected Android `Account`, and does not start authorization during app startup. Its access-token lease is in-memory and redacted.
- `GmailProviderSessions` and `CalendarProviderSessions` default `googleHttpEnabledByOwner` to `false`; `openDefault()` selects Fake. The allowlisted read scopes are `gmail.readonly`, `calendar.events.readonly`, and optional `calendar.calendarlist.readonly`.
- `AppProviderComposition.fakeDefault()` creates `FakeGmailProvider` and `FakeCalendarProvider`; `GoogleAuthorizationDemoPanel` is an explicit synthetic walkthrough. Neither the ordinary application connection UI nor the composition root selects the Play Services gateway or wires Activity Result consent/resolution into Gmail/Calendar sessions.
- The real adapter and account/session/lease tests are useful preparation, but they are Mock/Fake tests. Live OAuth and service reads remain NOT_RUN. A future UI must separately explain connection scopes, selected account, provider status, disconnect/revoke behavior, and deletion/retention implications; this Demo panel is not live-service consent UX.

### Before enabling any Google connection (Owner review required)

1. **Choose and document the project.** Prefer a dedicated development/test Google Cloud project separate from any eventual production project. Name the project and billing/API owners; enable only Calendar API initially. Add Gmail API only after its separate approval and data policy review.
2. **Ratify Android identity.** The current Beta variant is `app.serea.android.beta`; Demo/debug is `app.serea.android.demo`; instrumentation uses isolated IDs. Owner must ratify the stable Beta Package ID by the v4 O-01 date (2026-11-03). An Android OAuth client binds the exact package ID and signing certificate SHA-1. The Owner Beta certificate SHA-1 is not currently supplied or known. Do not substitute the CI debug, temporary Identity-test or local upload certificate. Record the OAuth SHA-1 and the release verifier's SHA-256 for the same certificate and verify them from the final APK.
3. **Configure consent screen and project.** Set app name, support/developer contact, audience (personal external use vs Workspace/internal), homepage/privacy policy and authorized domains as applicable; accurately disclose Google data access, purpose, local processing, retention, deletion and sharing. Add only Owner-controlled test users during development. Confirm API enablement, OAuth client type Android, package/SHA-1 pair, and any branding/verification status. No real OAuth client or project was created by this lane.
4. **Use incremental, read-only scopes.** Calendar-first should request only `calendar.events.readonly` for event listing/detail. Request `calendar.calendarlist.readonly` only when calendar discovery is needed. Avoid broader `calendar.readonly` unless a separately reviewed product requirement proves it necessary. Do not ask for Gmail on first Calendar connection.
5. **Treat Gmail as a separate higher-review step.** `https://www.googleapis.com/auth/gmail.readonly` is a Restricted scope. Before adding it, complete a data-flow and retention review. Google says restricted-scope data stored or transmitted on a server can trigger a security assessment; the Fake/local handoff does not establish eligibility to send Google data to any cloud model or server. Keep Chat on an explicitly local path unless a future recipient/content/purpose/expiry/provider-terms consent contract has its own review.
6. **Verify personal-use status narrowly.** Google documents a potential verification exception for some apps used by fewer than 100 users, but this is conditional and does not waive the Google API Services User Data Policy. Confirm the current audience, scopes, app status, user count and verification/security-assessment conditions in current Google documentation immediately before activation; do not infer an exception from Serea being personal or from a testing project.
7. **Calendar-first acceptance.** After Owner approval only: create the Android client against the ratified Beta package and certificate; use an explicitly selected test account; wire the Activity consent result to the existing one-shot registry; exercise read-only list/today/detail/period/timezone paths, account switch, stale callback, expiry, denial, disconnect and revoke; assert no mutation endpoint and no Cloud egress; record package, cert fingerprints, scopes, API versions, source SHA and sanitized evidence. Keep the test account out of public CI.
8. **Enable/disable and revoke separately.** Enabling requires an explicit Owner-controlled build/runtime switch, a visible connection action and a test account. App-local Disconnect should synchronously close provider sessions, cancel in-flight reads, clear app-local cached Google content and invalidate session generations while preserving other feature grants where intended. “Revoke Google access” is a distinct user action that invalidates all local leases immediately and calls the Google revoke path; handle remote failure/cancellation as locally disconnected. On reconnect, require fresh authorization. Provide a final data-delete route for information Google policy requires deleting after account disconnection.
9. **Account isolation and lifecycle.** Bind each provider session and any cached records/conversation handoff to an opaque selected-account identity plus generation. Account change must replace both Gmail/Calendar sessions and invalidate delayed results before UI delivery. Do not write account names, tokens, message bodies, calendar descriptions or OAuth callbacks to logs/analytics. Activity recreation must retain or safely rebuild a single owner of the consent request; process death should cancel pending consent and require a fresh session.
10. **Keep real data out of public CI.** CI uses Fake gateways, synthetic account identifiers and MockWebServer only. Store no refresh/access token, OAuth credential, personal message/event fixture or account identifier in repository, workflow artifacts, issue/PR text, screenshots, crash logs or model prompts. Public workflow secrets must not be used for personal OAuth. Any future live acceptance runs locally with an Owner-controlled test account and sanitized evidence; rotate/revoke the test grant after use.

Official references to recheck before activation (reviewed in the supplied v4 roadmap, 2026-10-28): [Gmail API scopes](https://developers.google.com/workspace/gmail/api/auth/scopes), [OAuth verification and personal-use exceptions](https://support.google.com/cloud/answer/13464323?hl=en), [AuthorizationClient](https://developers.google.com/android/reference/com/google/android/gms/auth/api/identity/AuthorizationClient), and [Google API Services User Data Policy](https://developers.google.com/terms/api-services-user-data-policy). Policy review here is preparation, not a legal or Google verification determination.

## Package, signing and update decision matrix

| Item | Current implementation/evidence | Owner / release acceptance still needed |
|---|---|---|
| Namespace and IDs | Namespace `app.serea.android`; Beta ID fixed in variant check to `app.serea.android.beta`; Demo/debug suffix `.demo`; isolated tests use `.isolated` | Owner ratifies Beta ID and Demo/Beta coexistence by 2026-11-03. If identity changes, repeat OAuth client/package, install, Room/Keystore and update checks.
| Versioning | Demo defaults `versionCode=1`; Beta task requires explicit `-PsereaBetaVersionCode` > 1; artifact verifier rejects mismatch | Maintain a private monotonic release ledger; never reuse a version code for a new artifact; record versionCode/versionName/source SHA/run ID per Owner build. Define recovery versioning before first real install.
| Owner key | Beta signing validation expects external PKCS12 path/password/alias/key password, expected certificate SHA-256 and explicit versionCode; task fails closed without them | Decide offline/HSM/managed secure signing custody, named access/recovery process and encrypted backup. Never create, upload, commit or use Owner production key in this lane. Current Gradle entry expects local key material, so a later secure signing workflow needs a separately reviewed implementation decision.
| Certificate | Build validates configured cert SHA-256; APK verifier checks `app.serea.android.beta`, versionCode, label and signing certificate SHA-256 | Independently derive certificate SHA-1 for Android OAuth client. Bind source SHA → build run → APK SHA-256 → package/version → signer SHA-256 (+ OAuth SHA-1) in signed/reviewable release record.
| Same-key update | CI identity tests previously used temporary keys to test same-key update, alternate-key rejection, Demo/Beta separation, encrypted Room persistence and Demo-data preservation | Repeat with disposable data and release-equivalent APK before Owner release; same package and same signer, strictly higher versionCode. The test is not evidence about the Owner certificate or Pixel device.
| Different-key update | Android rejects update from a different signing certificate; verifier/security tests cover mismatch | Preserve old key access; do not uninstall or clear app data to force installation. Treat different signer as an incompatible package and fail closed. Recovery needs correctly signed forward update, not a silent key swap.
| Room / Keystore | Beta is a separate app ID, giving it an app-private storage and Android Keystore namespace. Previous emulator identity test records encrypted Room survival across reboot/update | Confirm migrations on release APK with disposable data. Do not promise Keystore secrets can be copied/restored after uninstall or device reset. Keep upgrade non-destructive and document app-data backup/export boundaries.
| Demo + Beta | Distinct `.demo` and `.beta` application IDs support coexistence | Verify both remain installed, Demo package data/signer do not change when Beta updates, and no shared file/content provider crosses the namespace.
| Rollback / recovery | Versioned APK verifier and isolated update test hooks exist | Android package managers reject downgrade and mismatched signer. Prefer same-key forward-fix build with a higher versionCode; database migrations must be forward-compatible or provide a tested recovery path. Do not uninstall/clear data as a rollback. Owner tests recovery on disposable state before use.
| Distribution | Existing GitHub Beta workflow is main-only and protected by environment/reviewer/Owner enablement gates; tests ensure branch protection, required exact-SHA checks, WIF, package identity and checksum checks | No distribution or workflow policy change in G6. Owner must separately approve destination/tester list, data handling, exact approved source SHA and release artifact. No signed APK was distributed.

### Release evidence and SBOM status

- Existing `generate_android_lockfile_sbom.py` creates a deterministic CycloneDX 1.5 inventory from `android/app/gradle.lockfile` (325 locked coordinates in the checked-in G4 evidence). It explicitly marks dependency edges, artifact hashes, publisher authenticity, Gradle plugin/wrapper and Android SDK as out of scope.
- Existing `bind_android_apk_sbom.py` can bind an inspected APK SHA-256 plus package/version metadata to the lockfile SBOM. Its contract explicitly says the locked libraries' packaged membership is not established and APK-to-source provenance is not independently attested. Existing checked-in G4 evidence binds a CI **debug APK**, not an Owner-signed Beta release.
- Before RC, generate the BOM from the same source/lockfile and inspect the exact Beta APK; bind lockfile SHA-256, source SHA, workflow/build provenance, APK SHA-256, package ID, versionCode/versionName, signer SHA-256, and artifact storage identity. Include Gradle plugins, wrapper distribution digest, Android SDK/build-tools, native libraries and model assets if any. Resolve component licenses and attribution/notice requirements; lockfile coordinate inventory alone is not license clearance.
- Compare the release APK's actual libraries/native objects against the SBOM and produce a signed or tamper-evident evidence packet. Preserve exact test results, formal Required run, Identity/update test, vulnerability/license review, privacy/security review, and installation instructions. Until this chain exists, SBOM status is **PARTIAL**, not complete release provenance.

The exact G5 lockfile is not the input that produced the checked-in G4 inventory: current lockfile SHA-256 is `acd5c277ee1286165649985ffba952dd578eb2de497e2b7da9c9f689292f8795`, while the G4 BOM records `7ba42b06d78db5641e37ed3d9e61b441f87fc54918498058959b05fce5cbde23` and source commit `71ccdd1767c4597b19f584e19901f7a28aa479ea`. Running the generator tests on G5 parsed 317 components, compared with 325 in that prior G4 artifact. This is evidence drift from a newer lockfile, not a claim of a dependency defect; it means the checked-in 325-component report cannot certify the G5 lockfile. Regenerate and bind evidence at the exact G6/RC source SHA.

### Pixel 7a Owner acceptance checklist (NOT_RUN)

1. Confirm Owner permission and use a non-critical test profile/device state; record model, Android build/security patch, app package/version/signer and free storage. Back up only using Owner-approved method; do not wipe or uninstall personal apps/data.
2. Verify signed Beta APK SHA-256, signer certificate SHA-256, package `app.serea.android.beta`, versionCode and release evidence packet before install. Install Beta beside Demo and confirm identities/data stores remain separate.
3. Exercise cold start, relaunch, process kill/restart, screen rotation/activity recreation, offline local Chat, cancellation, navigation and encrypted history recovery. Confirm local tasks and UI errors. Capture no real personal content in logs/screenshots.
4. If/when separately approved, test Calendar connection first with the approved account and read-only scope. Validate explicit account selection, list/event/timezone, expiry, disconnect, account switch and revoke. Keep Gmail disabled until its separate Restricted-scope review. Confirm no Google mutation and no Cloud egress.
5. Test release update from the previous signed build using the same Owner key, higher versionCode, Room migration and Keystore continuity; verify the previous data remains available. Test recovery by installing a correctly signed higher-version forward-fix. Do not simulate downgrade by uninstalling or clearing data.
6. Check battery/thermal behavior, memory pressure, network state, crash recovery, accessibility and notification/privacy surfaces under normal personal use. For local model acceptance, measure first token/turn latency, throughput, memory peak, cancellation, repeated multi-turn heat/power and offline behavior separately; model-quality suitability is a distinct Owner decision.
7. Retain only sanitized pass/fail notes, package/version/certificate fingerprints and hashes. Physical Pixel 7a acceptance remains NOT_RUN until the Owner authorizes device operation.

## Owner decision matrix

| Decision | Roadmap date | Preparation | G6 state |
|---|---:|---|---|
| O-01 Beta package ID / Demo coexistence and update identity | 2026-11-03 | Identity matrix and test evidence requirements written | Owner decision outstanding; current candidate code pins `app.serea.android.beta` |
| O-02 Google project, Android OAuth client, package/certificate SHA-1, account and scopes | 2026-11-10 | Calendar-first checklist written; Gmail scope separation and verification conditions documented | No project/client/account enabled or used |
| O-03 AI provider/model eligibility and cost/retention terms | 2026-11-10 | Keep G3 isolated; require adapter, license, performance and product review | No cloud key/billing use; no product AI selection by this lane |
| O-05 Owner signing key, access/recovery and distribution | 2026-11-21 target in plan | Key custody, cert, version, APK/SBOM evidence procedure documented | No Owner key created/loaded/used |
| O-06 Pixel 7a access and non-destructive acceptance scope | During November | Checklist prepared | No physical device used |
| O-07 main integration method and merge | RC review | No-merge rehearsal clean against current main/G5 | No merge or protected-branch mutation |
| O-08 final GO / Conditional Preview / NO-GO | 2026-12-31 | Gate conditions described in v4 | Not determined by Agent C; G6 preparation does not imply Personal Beta GO |

## Evidence / result

- G5 base SHA/tree verified before work. GitHub PR head/base/status fields fetched for #123, #125, #126, #127, #129, #130, #131, #132 and #134.
- PR topology checked directly with Git ancestry: #123, #129, #130 and #131 are ancestors of #132/#134; #132 is ancestor of #134. #125/#126/#127 are not ancestors of the current candidate. #126 and #127 remain a separate PoC stack.
- Main merge rehearsal: `git merge-tree --write-tree --messages origin/main refs/remotes/origin/pr-134` returned exit 0, no conflicts and G5 tree `9133d4a3...`; no branch/ref/PR/main was moved.
- Focused host checks: `python3 tests/android_lockfile_sbom_tests.py` — 7 PASS; `python3 tests/android_artifact_sbom_tests.py` — 10 PASS; `python3 tests/android_firebase_distribution_tests.py` — 19 PASS. Total 36 PASS, 0 failures. These exercise existing generator/binding/distribution guard contracts; they do not test real signing or distribution.
- `git diff --check` — PASS. Python bytecode caches from the checks were removed before commit.
- Remote result: corrected branch pushed to `origin/codex/serea-g6-release-readiness`; Draft PR #135 created through GitHub integration, OPEN and unmerged, targeting the frozen G5 branch. The standard `gh pr create` attempt returned HTTP 401; the connected GitHub integration successfully created #135. No merge was attempted.
- No live Google project, OAuth client, account, real API/data, cloud model, production key, physical Pixel or distribution path was activated.
- G5 formal CI values remain evidence for G5 only; this documentation change has not yet run CI and must not be represented as a G6 tested product integration.

