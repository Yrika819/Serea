# Serea G1–G4 Orchestrator Ledger

Status updated: 2026-10-27 UTC. This file is owned only by the Orchestrator.

## Authority

- Repository: `Yrika819/Serea`.
- Owner roadmap: G1–G4 Real Service Readiness / On-device AI / Main Integration instruction dated 2026-10-27. This instruction is authoritative for this campaign.
- Frozen Functional Candidate: PR #123, head `3924b36080ffa43331f065a15db80c604c243355`, tree `41cfce316c485abe31f86c827c6ca81a4d5c107b`, Draft/Open.
- Frozen Android Required base: PR #117, `151ea9d66d32de01b195f4262beb67516823c281`.
- Current `main`: `e0acb0254820c381dd97828824bef86a1db5176c`.
- PR #122: `b64a517ec33dd672c17cd604a912045116ad963a`, base `codex/serea-personal-beta-integration-clean`; PR #123 is a divergent head, not assumed equivalent.
- PR #98 `00e0453f83970a2b7d2bd26ce01462e421f75f46`, #100 `ad868af753672c546c52411ce151904360633acc`, #117, #120 `772b1cd37f43b3f92276271abe8c67aabb74f7d0`, #121 `8853c2afe25497e8abdd55282e85331efee3f009`, #122 and #123 are all currently Open/Draft.
- The older `docs/verification/personal-beta-autonomous-closure.md` and 2026-10-26 plan ledger contain stale phase state. They are historical evidence only and do not supersede this campaign checkpoint or current GitHub state.

## G4 Worktree / Branch

- ACTIVE_PHASE: G1 integration/release audit complete; G2 lifecycle repair integrated; G4-A final-head validation and independent security review in progress; G3 isolated on-device AI feasibility PoC in progress.
- GOAL_STATUS: PARTIAL.
- BASE_SHA / LAST_GOOD_HEAD: PR #123 functional candidate `3924b36080ffa43331f065a15db80c604c243355`.
- CURRENT_HEAD: `2b12dfd6ed77ed830d8fd15260f6beb7ed60f2e2`; tree `5ca58c32302e931db0e18ebc09e9635a029d4855` (local; checkpoint update and push pending).
- OWNED_BRANCH: `codex/serea-g4-personal-beta-readiness`.
- WORKTREE: `/workspace/Serea-g4-personal-beta-readiness`.
- Checkpoint branch first pushed and GitHub-verified at `06d182db41f957fdbb9f29b48ae50cd318e465f9`; subsequent checkpoint heads are recorded in branch history. No #123 Required result transfers to G4.
- DRAFT_PR: [#125](https://github.com/Yrika819/Serea/pull/125), Open / Draft; base is PR #123's actual head branch `codex/serea-personal-beta-functional-integration-identity` at `3924b36080ffa43331f065a15db80c604c243355`. GitHub last reported CLEAN; current G4 diff is seven paths after G2 checkpoint integration.
- main, PR #117, #120, #121, #122 and #123 are protected from modification/merge.

## Existing exact-head evidence

All evidence below applies only to candidate SHA `3924b36080ffa43331f065a15db80c604c243355` and tree above:

- Formal Android run `40427448635`: API 26 `105/105`, API 35 `105/105`, no failures/errors/skips; Formal Identity, Process Death, Data Isolation and Required verifier PASS.
- API 26 artifact `12701024855`; API 35 artifact `12702662029`; Identity artifact `12702479957`; Emulator evidence artifact `12703250122`; Pixel 7a CI reacceptance artifact `12702701890` (not physical-device acceptance).
- Functional Focused run `40425595932`: JVM `199/199`, API 26 `33/33`, API 35 `33/33`.
- Exact-head CI: Fast `40427448605`; Full `40427448681`; Native `40427448836`; SQLite `40427448709`; RustSec `40427448616`; Git Hooks `40427448704`; Android Contract `40427448688` — all PASS.
- Release/SBOM Focused run `40425866784` PASS. Real Google OAuth/account, real AI credentials/provider, production signing/distribution and physical Pixel 7a acceptance remain NOT_RUN.
- This is the frozen pre-G4 baseline only. Any G4 code or Required workflow change requires validation on its own exact HEAD.

## Agent status

| Agent | Goal | Status | PR | Head | Review status |
|---|---|---|---|---|---|
| G1 | Main integration and release architecture | SUCCESS for provenance/rehearsal; independent read-only G4 security review underway | [#124](https://github.com/Yrika819/Serea/pull/124) Draft | `e9dc99f5b204af38437e5dc3cb1b5595c21fb2dc` | G2 repair accepted; G4 review is read-only |
| G2 | Google OAuth/Gmail/Calendar readiness | PARTIAL — Fake lifecycle repair complete; real Google path remains NOT WIRED | None | source `b87b39d9c86d94bc6121c0605aaccd9497e9694c`; latest report `2a69fb2c9dcc6733fb0a4774b43628e13b757426` | G1 ACCEPTED; G4 rerun 75/75 JVM PASS; isolated test-source compile PASS; no emulator |
| G3 | Real AI / on-device inference feasibility | PARTIAL; fixed llama.cpp ARM64 compile passed; isolated harness ongoing | None | report `6d6f362da2380b56ea0fb7bc9372dbcceae67e90`; PoC source HEAD pending | No model downloaded, no runtime/model inference or Pixel 7a performance claimed |
| G4 | Orchestration, integration, independent security review and Required validation | Active | [#125](https://github.com/Yrika819/Serea/pull/125) Draft | local `2b12dfd6ed77ed830d8fd15260f6beb7ed60f2e2` | Exact-head local JVM 75/75 and isolated Android test-source compile PASS; hosted Functional Focused/Required pending |

Each agent owns a separate worktree, branch, and checkpoint. No agent may modify another lane's files or branch. Only this Orchestrator edits this ledger.

## P0 / service state

- Local/Fake Chat, Gmail, Calendar, Local Tasks, Egress/session tests and exact-head Required are verified on the frozen candidate.
- Real Gmail/Calendar OAuth, real AI selection/activation, production signer/distribution, and Pixel 7a acceptance are Owner-gated or device-unavailable and remain NOT_RUN.
- Lane B PR #120 and Lane C PR #121 remain separate Drafts; selected tests/release checks already present in candidate are not automatically duplicated or merged.
- No Required acceptance identities, counts, thresholds, skip conditions, security gates, or workflow requirements may be relaxed.

## Current risks / decisions

- PR #122 and #123 diverged from a shared base. G1's exact-ref and semantic review found no #122-only paths; Required manifests/verifier blobs match, and the disposable merge rehearsal reproduced #123's tree. Main was not changed.
- PR #123 has a large cumulative diff against main. G1's provenance matrix and disposable merge rehearsal are evidence for a staged integration path, not authorization to merge.
- Google policy and scope research is not permission to use an account or enable OAuth.
- Host/Emulator AI inference evidence is not Pixel 7a performance evidence. No real API key or model service call is authorized.
- Main merge, production signing, Firebase distribution, real accounts/provider credentials, Pixel 7a operation, and final GO require Owner decision.
- G2's synthetic authorization walkthrough had races and a disconnect/shared-revoke state mismatch. G2 repaired the single-flight and stale-grant races, then preserved app-local feature disconnect across shared Fake grant revocation. G1 accepted source commit `b87b39d9c86d94bc6121c0605aaccd9497e9694c`; G4 cherry-picked it as `0ad81a8`, plus both report checkpoints. On G4 exact HEAD `2b12dfd6ed77ed830d8fd15260f6beb7ed60f2e2`, six local JVM classes passed **75/75**, 0 failures/errors/skips, and `:app:compileIsolatedAndroidTestKotlin` passed. No emulator, real account or live OAuth was used.
- G2 cancellation invalidates authorization generations and closes stale leases but does not cancel coroutine Jobs. The current gateway is Fake-only and performs no live I/O; add Job cancellation before reusing the ViewModel with a real asynchronous gateway. Ordinary Gmail/Calendar UI remains NOT WIRED; OAuth/API and emulator E2E remain NOT_RUN.
- G4 PR #125 was initially opened against PR #122's head branch. G1 identified the mismatch; it was corrected through GitHub API to PR #123's actual head branch. The current local diff from #123 is seven paths: Google demo UI/ViewModel/Fake gateway/tests, G2 checkpoint, and this G4 ledger. No protected or agent-owned branch was changed.
- Earlier local verification on `4c5f7c5d8a47f9325c3aca93fb0ad232acf84b6e` (74/74) is superseded for source coverage by the exact-head rerun above.
- Functional Focused run `40491693974` completed SUCCESS on pre-repair SHA `68569d093cd9f5f631ca22e0343a78cd366c535b`: provider unit, API 26, and API 35 passed the workflow's exact 33-test verifier. It is not final-head API evidence because it predates the latest Fake disconnect/revoke source repair. Artifacts: provider `12727943809` (zip digest `afdf05ef3b36ab7cce96623900a56b2b7c1e61104a5b6473d0e475848b13ea14`); API 26 `12729515163` (zip digest `3ba7f70248f7060110c19286ad5445fdcfc537194ed77c2ad107316fc6cc798f`); API 35 `12730231075` (zip digest `4fbdf8b84f9b369ee43c981be81ddddb66659cdb04b87d2c31c4bed155b47b`). `gh run download` returned HTTP 403 for artifact payloads. Logs show accelerated KVM checks and sequential API26/API35 passes; unavailable inner APK digests and device fingerprint are not claimed.
- Release Security Focused run `40492139978` completed SUCCESS on the same pre-repair SHA. Artifact `12728709100`, upload zip digest `b12db56eb064fcf01fe6b760ef7d8c7518d092a78238f49182e2b5b8ab698400`. Logs record CycloneDX 1.5, 325 lockfile components, and `publisher-authenticity=not-established-by-this-inventory`. Artifact payload access returned HTTP 403. Local deterministic regeneration at that old source revision produced SBOM SHA-256 `c57a9ac5a57a3d04813ffc21e40fac4945d170598f61e1d7daffdde2d5a3d5fc` with 325 components; bytewise artifact comparison was unavailable.
- G1's independent read-only review of exact G4 local tip `2b12dfd6…` is in progress. The branch checkpoint has not yet been pushed. The main-only Personal Beta workflow had zero jobs and is not product validation.
- G3 report branch `codex/serea-g3-real-ai-feasibility` is pushed and verified at `6d6f362da2380b56ea0fb7bc9372dbcceae67e90`, tree `f4fe1cf534b855b58ea19683621bf3dc7f7a3cd6`; only `docs/agent-reports/g3-ai.md` changed. Orchestrator reviewed the exact diff, clean worktree, scope, explicit NOT_RUN inventory, model provenance/licensing caveats, and policy/runtime claims against current official Gemini, ML Kit GenAI, Ollama and llama.cpp sources. No correction requested. This report is feasibility evidence only; no model, cloud provider, or runtime is integrated or approved.
- PR #124 is an audit-only Draft based on main. Its Fast CI run `40488015151` failed before tests because main's existing `tools/check_commit_identity.py` scans `git log --all` in a full-depth checkout and finds unrelated commits from fetched refs. The newer #123 stack uses event-bounded commit-range validation. This is a stale-main-base integration issue, not grounds to relax the guard. G1-canceled broad runs `40488015109`, `40488015078`, `40488015079`, `40488015071` are not validation evidence.

## NEXT_ACTIONS

1. Push the checkpoint to the owned branch, verify PR #125's base/head and changed paths, then dispatch final-head Functional Focused and Release/SBOM Focused.
2. Receive G1's read-only review of exact G4 tip `2b12dfd…`; request a bounded correction if any issue is found.
3. If source remains stable and Functional Focused passes, run Formal Required and relevant Fast/Full/Native/SQLite/RustSec/Git Hooks/Contract CI on the same HEAD. Do not inherit #123 results.
4. Review G3's isolated Android AI PoC when the build/harness and evidence are ready; coordinate one KVM-backed non-required runtime attempt after G4 Emulator jobs finish.
5. Prepare release/OAuth/AI/Pixel checklists without production credentials, services, signing keys, or devices.

## OWNER_APPROVAL_REQUIRED

Real Google OAuth/account and scopes; real AI provider eligibility/terms/cost/credentials; production Beta signing and distribution; physical Pixel 7a acceptance; merge to main; security or Required-contract changes; final Personal Beta GO.
