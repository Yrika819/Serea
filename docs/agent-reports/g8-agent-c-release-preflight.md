# G8 Agent C — G7 Closure and Release Security Preflight

**Checkpoint:** 2026-10-29 JST
**AGENT_ID:** C — G7 closure / Release Security
**ACTIVE_PHASE:** G7 evidence reconciliation; G8 and G9 release preflight
**GOAL_STATUS:** PARTIAL — the G7 SQLite matrix remains incomplete; release and OAuth preparation is documentary only.
**BASE_SHA:** `00384eb6468214362435140c9a2d848fa2e0f12e`
**BASE_TREE_SHA:** `10d07a8ec57d7f67fc94663c489347938cccbd58`
**WORKTREE:** `/workspace/Serea-g8-release-preflight`
**BRANCH:** `codex/serea-g8-release-preflight`
**DRAFT_PR:** [#142](https://github.com/Yrika819/Serea/pull/142), OPEN / DRAFT, based on G7 #139.
**JUNIT_RESULT:** NOT_RUN — documentation-only work.
**OWNER_APPROVAL_REQUIRED:** Real OAuth/project activation, real Google account/data, Owner package/signing decisions, production signing/distribution, Pixel 7a operation, protected-main merge, and Personal Beta GO.

## Authority and scope

GitHub was checked on 2026-10-29 JST. `main` remains `e0acb0254820c381dd97828824bef86a1db5176c`. PRs #132, #134, #138, and #139 are OPEN/DRAFT and form the current dependency chain:

| PR | Head SHA | Base | State and exact-head CI note |
|---|---|---|---|
| [#132](https://github.com/Yrika819/Serea/pull/132) | `373b8be15f407d951157d2a80a632e86d979acdf` | `main` at `e0acb02…` | OPEN / DRAFT; Android run `40599571275` failed Storage/Emulator and Required; Identity was SKIPPED. Not green for merge. |
| [#134](https://github.com/Yrika819/Serea/pull/134) | `b84feb337cd57918346ab31cb3ea4af4ba880ffb` | #132 head | OPEN / DRAFT; frozen G5 candidate, Android run `40664188621` PASS on its exact head. |
| [#138](https://github.com/Yrika819/Serea/pull/138) | `23b884cf2815a2f3287652d5ffd2c3913d09df76` | #134 head | OPEN / DRAFT; G6 candidate, Android run `40710445577` PASS on its exact head (attempt 2). |
| [#139](https://github.com/Yrika819/Serea/pull/139) | `00384eb6468214362435140c9a2d848fa2e0f12e` | #138 head | OPEN / DRAFT; G7 candidate, Android run `40749231249` PASS; SQLite run `40749231272` has one direction NOT_RUN. |

No existing PR or protected ref was modified. This lane adds only this report; no live OAuth, Google data, signing key, release, or physical-device work was done.

## G7 SQLite portability closure

Run [40749231272](https://github.com/Yrika819/Serea/actions/runs/40749231272) is attached to exact PR #139 source SHA `00384eb6468214362435140c9a2d848fa2e0f12e` (run attempt 2; run number 780). The two attempts agree on the material result:

| Direction / job | Attempt 1 | Attempt 2 | Assessment |
|---|---|---|---|
| Intel x86_64 produces DB | PASS | PASS | Complete |
| macOS ARM64 produces DB | PASS | PASS | Complete |
| ARM64-produced DB opens on Intel x86_64 | PASS | PASS | Complete |
| Intel-produced DB opens on macOS ARM64 | CANCELLED, `runner_id=0` | CANCELLED, `runner_id=0` | **NOT_RUN**; no runner was assigned |

GitHub's latest-attempt job records show the missing direction requested label `macos-15` with `runner_id=0` in both attempts. This is runner unavailability, not an SQLite assertion or application-code failure. Repeating the workflow without a runner-capacity change is not useful. No new rerun was started.

Attempt 2 uploaded the Intel producer artifact `portable-db-intel` (artifact `12843411146`, digest `sha256:a949aec7af20dcceb46e83f702b36fed2ac6194e43f8bb066b6d250ece73600b`) and the ARM64 producer artifact `portable-db-arm64` (artifact `12843930249`, digest `sha256:d37cdb542c0543cacf197d84711f5a03f7dff33bb0ce17108c347f2be90b91f0`). They are source-bound to `00384eb…`; their GitHub artifact expiry is 2026-10-29 18:54 UTC. These producer artifacts do not prove the missing Intel→ARM64 open direction. No JUnit result exists for that cancelled job.

**G7 SQLite status: PARTIAL.** Preserve the three successful directions and the missing fourth direction separately. G6 SQLite success is not substituted for G7 evidence.

## G7 exact-head Android evidence and historical IME risk

The G7 implementation/test source was `acde94c3c0f81ac2e76d0b0e51e48fd84589ebeb` (tree `f2fa81955f4c8834b83073ef4d239c831ff4f773`). Android run [40740591530](https://github.com/Yrika819/Serea/actions/runs/40740591530) on that exact SHA had two API 35 attempts failing the existing `app.serea.android.SereaAppShellTest#chatDraftSurvivesGmailAndCalendarDestinationChanges` case. The first failure was a Compose condition timeout; the same-SHA rerun failed `Composer IME state did not settle at R4-B-committed` in `awaitComposerImeState`. API 26 passed, but API 35 did not produce a passing 105-test result, so the dependent Identity Isolation job was skipped on that run. These failures remain historical evidence and a release reliability risk; G7 did not change the Chat/IME source or loosen the gate.

The final G7 HEAD `00384eb…` adds the evidence ledger checkpoint. Its exact-head Android run [40749231249](https://github.com/Yrika819/Serea/actions/runs/40749231249) completed successfully, including API 26 and API 35 Required at 105/105 each, temporary-key Identity Isolation, Process Death, Data Isolation, and Sentinel. API 26 artifact: `12845730516`; API 35 artifact: `12847400793`; emulator bundle: `12846945714`; Identity evidence: `12846168567`. This final-head PASS does not erase the prior reproducibility risk.

Other verified final-head signals are:

| Scope | Run | Result |
|---|---:|---|
| Calendar OAuth synthetic loopback focused test | `40749231295` | PASS, exact one JUnit test; artifact `12843121520`, SHA-256 `a198b4746313c27986ab3728f8c7dafc6e060c0a6dd88ad912ef593b689b062e` |
| Full CI | `40749231298` | PASS, 5/5 jobs |
| Fast CI | `40749231228` | PASS |
| Native platform validation | `40749231215` | PASS, 12/12 jobs |
| Rust Dependency Security | `40749231235` | PASS |
| Git Hooks Windows compatibility | `40749231318` | PASS |
| Cross-architecture SQLite | `40749231272` | PARTIAL; one direction NOT_RUN as above |

All these runs are tied to `00384eb…`; no result from `acde94c…` is used as proof of final-head success. The CI-produced Pixel 7a bundle is a report/fixture only, not physical-device acceptance. The main-only Beta workflow remains protected and was not forced on the feature branch.

## Calendar-first OAuth owner checklist

### Google requirements reviewed on 2026-10-29 JST

Official sources were fetched and checked on 2026-10-29 JST. The pages state their own update dates below where shown. This is implementation preparation, not legal advice, OAuth approval, or a Google verification determination.

| Official source | Current fact relevant to Serea |
|---|---|
| [Google API Services User Data Policy](https://developers.google.com/terms/api-services-user-data-policy) — page says last updated 2024-02-15 | Disclosures must accurately describe data, purpose, storage and sharing. Limited Use restricts data to providing/improving prominent user-facing features; transfer is constrained and user consent is required for transfers that provide such features. Sensitive/Restricted scopes may have extra access and security-assessment requirements depending on API and grants/users. The policy applies even where OAuth verification is not required. |
| [Calendar API scopes](https://developers.google.com/workspace/calendar/api/auth) — last updated 2026-09-03 UTC | Scope descriptions distinguish `calendar.events.readonly` (“View events on all calendars”) from `calendar.events.owned.readonly` (events on calendars the user owns). `calendar.calendarlist.readonly` reveals the list of subscribed calendars and is an additional scope, not required merely to fetch events from a known calendar ID. Request the narrowest scope that satisfies the reviewed product requirement. |
| [Gmail API scopes](https://developers.google.com/workspace/gmail/api/auth/scopes) — last updated 2026-09-10 UTC | `https://www.googleapis.com/auth/gmail.readonly` is listed under **Restricted scopes** and permits viewing messages and settings. It is not needed for Calendar acceptance and must remain a separate Owner decision. |
| [When is verification not needed?](https://support.google.com/cloud/answer/13464323?hl=en) | The page describes a Personal Use exception for fewer than 100 users and permits users to proceed through the unverified-app warning; it also describes separate Development/Testing/Staging projects. It says all Google API integrations must still comply with the API Services User Data Policy. The exception is conditional, not an automatic blanket approval for Serea. |
| [Android OAuth client setup](https://support.google.com/cloud/answer/6158849?hl=en) | Android client configuration requires the app package name and the SHA-1 fingerprint of its signing certificate. The OAuth SHA-1 must come from the same Owner-approved release certificate whose SHA-256 is recorded in release evidence. |
| [AuthorizationClient reference](https://developers.google.com/android/reference/com/google/android/gms/auth/api/identity/AuthorizationClient) | The API separates `authorize`, result retrieval, `clearToken`, and `revokeAccess`; local app disconnect and revocation of Google's grant are distinct user-facing actions and require distinct lifecycle handling. |

### Owner decisions and activation sequence — not executed

1. **Package identity (O-01, target 2026-11-03):** Owner ratifies whether `app.serea.android.beta` is the stable Beta package and whether `.demo` and `.beta` coexist. Recheck all application IDs, install/update tests, OAuth client records and app-private storage assumptions if the decision changes.
2. **Project and client (O-02, target 2026-11-10):** Owner selects a dedicated development/test Google Cloud project, project administrator and API owner. Initially enable Calendar API only. Create an Android OAuth client only after the package ID and release certificate are ratified; record the package, SHA-1, certificate SHA-256, project ID, client ID owner and consent-screen state in an Owner-controlled record. No credentials belong in Git or public CI.
3. **Consent, purpose and scope:** Explain account selection, the exact requested scope, what event fields are read, why the data is used, local storage/retention/deletion, and that Calendar connection does not authorize Chat handoff or Cloud processing. Start with `calendar.events.readonly` only if access across all accessible calendars is required; use the narrower owned-events scope if that is sufficient. Request `calendar.calendarlist.readonly` only for an explicit calendar-discovery feature.
4. **Verification and publication:** Owner checks current project audience (External/Internal), Testing/Production status, test-user cap, warning behavior, verification requirements and any security assessment for the selected scope and data flow. A Personal Use or Testing exception does not waive the API Services User Data Policy. Do not claim eligibility before Google Console states and policy conditions are confirmed.
5. **User-driven lifecycle:** Implement a visible foreground-only Connect action; select one account; show scope and purpose before the system consent UI; handle cancel, partial grant, denial, Activity recreation and process death fail-closed. Account switch increments session generation and invalidates old callbacks and reads before rendering. Disconnect synchronously cancels local sessions and clears app-held Google results; Revoke separately calls Google's revocation path and leaves the app locally disconnected even if remote revocation fails. Reconnect must create a new lease/session.
6. **Calendar acceptance after separate Owner approval:** Use only an Owner-controlled test account. Verify event list/detail/time range/time-zone/all-day cases, 401/403/429, expiry, account switch, stale result, revocation, reconnect and no write endpoint. Keep public CI synthetic and loopback-only; save no real token, account ID, event body or screenshot in GitHub.
7. **Gmail later gate (O-02 follow-up):** Treat `gmail.readonly` as separate Restricted-scope approval. Before any Gmail enablement, complete its own data-flow, retention, deletion, verification/security-assessment and user-disclosure review. G8/G9 fake MIME and hostile HTML tests do not authorize real Gmail access.

Google→Chat handoff consent and Cloud-send consent remain separate from Google connection consent. Local processing permission is not cloud-transfer permission. Google-origin Cloud egress remains disabled.

## Release security readiness and remaining evidence

| Area | Current evidence / decision required |
|---|---|
| Package IDs | Current build variants expect Beta `app.serea.android.beta`, Demo/debug `.demo`, and isolated test IDs. Owner ratification and coexistence acceptance remain pending. |
| Versioning | Demo defaults to `versionCode=1`; Beta build validation requires an explicit `-PsereaBetaVersionCode` greater than 1. Owner must retain a private monotonic version ledger and never reuse a published code. |
| Signing | No Owner production signing key was created, read, or used. CI temporary-key Identity proves test-only update behavior, not Owner certificate identity. Owner must choose custody/recovery and provide matching SHA-1/SHA-256 only inside a protected release workflow. |
| Non-destructive update | Preserve Demo data and Beta encrypted Room/Keystore state. Verify release-equivalent same-key update with higher versionCode and representative Room migration. A different key or downgrade must fail closed; recovery is a correctly signed forward update, not uninstall/data clear. Physical-device confirmation is NOT_RUN. |
| APK provenance | Current release-security workflow evidence is a CI debug APK and mock signing/distribution contract. It does not prove an Owner-signed Beta APK, publisher authenticity, or exact packaged dependency membership. No APK was distributed. |
| SBOM | The current evidence is a lockfile-derived CycloneDX inventory, with a debug-APK digest binding in mock release evidence. It does not fully capture packaged membership, Gradle plugins/buildscript, wrapper/SDK/toolchain, or attest source-to-signed-APK provenance. A final Beta needs APK inspection plus source SHA, lockfile, build provenance, APK SHA-256, package/version, signer SHA-256, native objects and model assets bound together. |
| Native/model licensing | Prior candidate records identify SmolLM2-135M fixture as Apache-2.0 and llama.cpp as MIT. Complete linked-native dependency notices/SBOM and redistribution review remain open; model redistribution conditions and final packaged license inventory are not closed. Keep GGUF outside Git. |
| Pixel 7a | CI reacceptance bundle is not physical acceptance. Pixel 7a Android 17, non-destructive installation/update, actual encrypted data continuity, temperature/performance, and final APK identity remain NOT_RUN pending Owner device authorization. |
| Required/branch protection | Required manifests and checks were not weakened. Main-only Beta workflow protection was not changed or bypassed. Future G8 formal CI must run on the frozen G8 integration SHA; G7 evidence does not transfer to it. |

### Owner decision source

The canonical decision IDs, dated 2026-11-03 and 2026-11-10 checkpoints, later Gmail Restricted-scope gate, and current approval states live in [`docs/plans/2026-10-26-personal-beta-owner-decision-ledger.md`](../plans/2026-10-26-personal-beta-owner-decision-ledger.md). This historical release-preflight report does not define a parallel O-ID matrix. All real OAuth, key, account, device, merge, and distribution actions remain Owner-gated.

## G9 preflight checkpoint

Safe preparatory work may proceed independently of live OAuth or G8 product integration. The candidate checklist is:

- Gmail Fake/Mock only: MIME type/charset parsing, malformed and nested parts, size caps, HTML sanitization, dangerous links, remote image suppression, page boundaries, and 401/403/429/cancel/revoke races.
- Gmail→Chat Fake data minimization: explicitly selected text only; preserve account/session/generation and source lineage; adversarial descriptions/bodies treated as untrusted content; stale/replay/mismatch rejects before Usage reservation, credential read or dispatch.
- Local model preflight: compare candidate licenses and native notices; test cancellation, repeated open/close, memory ceilings and process death; keep native runtime isolated unless independent review accepts the evidence.
- Release packet: improve lockfile/native SBOM coverage, artifact-to-source provenance design, version/signing-owner worksheet, and Pixel 7a non-destructive checklist. Do not create or load a signing key.
- Usage/UI mock checks: distinguish Fake, runnable local model and Cloud provider identities; show unknown local cost as unknown; do not enable Cloud egress.

This is a G9 preparation list, not a completed G9 E2E. Real Gmail, OAuth, Cloud AI, physical-device work and production signing remain prohibited in this phase.

## Agent C checkpoint contract

**CURRENT_HEAD:** `00384eb6468214362435140c9a2d848fa2e0f12e` before this report commit
**TREE_SHA:** `10d07a8ec57d7f67fc94663c489347938cccbd58` before this report commit
**CHANGED_FILES:** `docs/agent-reports/g8-agent-c-release-preflight.md` only
**TEST_COMMANDS:** `git diff --cached --check`; Python documentation sanity assertions for final newline, balanced code fences, authority SHA, runner-unassigned evidence, and NO-GO status. JUnit NOT_RUN because this is documentation-only.
**CI_RUN_IDS:** G7 exact final head: `40749231249`, `40749231295`, `40749231298`, `40749231228`, `40749231215`, `40749231235`, `40749231318`; SQLite `40749231272`; historical G7 code/test Android `40740591530` at `acde94c…`.
**ARTIFACT_IDS:** SQLite producer DBs `12843411146`, `12843930249` (expire 2026-10-29 18:54 UTC); focused test `12843121520`; API26 `12845730516`; API35 `12847400793`; Identity `12846168567`.
**SECURITY_IMPACT:** Documentation only; records scope breadth, consent separation, restricted Gmail implications, release provenance gaps and owner blockers.
**KNOWN_ISSUES:** G7 SQLite Intel→ARM64 Open NOT_RUN due no runner; historical API35 IME flake risk; Owner OAuth, package, signing, physical-device and AI decisions remain unresolved.
**DEPENDENCIES:** Parent independent review; G8 OAuth UI and native runtime reports for consolidated release checklist.
**INTEGRATION_READY:** Documentation candidate ready for independent review; no product code integration.
**REVIEW_REQUEST:** Verify G7 source/attempt/artifact bindings, Google policy interpretation and the distinction between mock/debug/temporary-key and Owner release evidence.
**NEXT_ACTION:** Parent review; integrate this checkpoint only if accepted, then refresh final CI/evidence against the integrated G8 HEAD.
**OWNER_APPROVAL_REQUIRED:** As listed above; no such action has been taken.
