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

- ACTIVE_PHASE: G1/G2/G3 report review complete; G2 Fake authorization changes integrated; G4-A focused revalidation and G4-C preparation.
- GOAL_STATUS: PARTIAL.
- BASE_SHA / LAST_GOOD_HEAD: PR #123 functional candidate `3924b36080ffa43331f065a15db80c604c243355`.
- CURRENT_HEAD: `4c5f7c5d8a47f9325c3aca93fb0ad232acf84b6e` (G2 source/report commits integrated; focused validation pending).
- OWNED_BRANCH: `codex/serea-g4-personal-beta-readiness`.
- WORKTREE: `/workspace/Serea-g4-personal-beta-readiness`.
- Checkpoint branch first pushed and GitHub-verified at `06d182db41f957fdbb9f29b48ae50cd318e465f9`; subsequent checkpoint heads are recorded in the Git branch history. The integrated G4 source head has no inherited Required status.
- DRAFT_PR: not yet created. After focused validation, base G4 Draft PR on PR #123's head branch `codex/serea-personal-beta-functional-integration` to show only G4 changes.
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
| G1 | Main integration and release architecture | SUCCESS for scoped provenance/rehearsal audit; independent review of G2 repair completed | [#124](https://github.com/Yrika819/Serea/pull/124) Draft | `e9dc99f5b204af38437e5dc3cb1b5595c21fb2dc` | REVIEWED — provenance, rehearsal and G2 race repair accepted; baseline artifact XML access limitation recorded |
| G2 | Google OAuth/Gmail/Calendar readiness | PARTIAL — fake lifecycle state repair integrated; real Google path remains NOT WIRED | None | source `efa24f660d28ec92cc3095cc868f27b1d0e576b9`; final report `5931c7b5c4effeecd16b2fb5da011c9289ed207a` | REVIEWED/ACCEPTED by G1; 74 JVM PASS; isolated test-source compile PASS; no emulator |
| G3 | Real AI / on-device inference feasibility | PARTIAL; report complete, PoC NOT_RUN | None | `6d6f362da2380b56ea0fb7bc9372dbcceae67e90` | REVIEWED — documentation-only; no correction requested; no runtime integration approved |
| G4 | Orchestration, integration, independent security review and Required validation | Active | TBD | `4c5f7c5d8a47f9325c3aca93fb0ad232acf84b6e` | G2 source integrated; exact-head JVM/compile and UI instrumentation pending |

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
- G2's initial implementation exposed two Fake-only authorization races. G1 requested changes; G2 repaired them in `efa24f660d28ec92cc3095cc868f27b1d0e576b9`. The repair reserves the active feature synchronously, enforces single-flight consent, invalidates queued grants on revoke/disconnect, and adds three regression cases. G1 accepted the repair. Orchestrator parsed six exact-source JUnit XML files: 74 tests, 0 failures/errors/skips; isolated Android test-source compile passed. G4 integrated the agent commit chain; exact G4-head rerun is pending. No emulator, real account or live OAuth was used.
- G2 cancellation invalidates authorization generations and closes stale leases but does not cancel coroutine Jobs. The current gateway is Fake-only and performs no live I/O; add Job cancellation before reusing the ViewModel with a real asynchronous gateway. Ordinary Gmail/Calendar UI remains NOT WIRED; OAuth/API and emulator E2E remain NOT_RUN.
- G3 report branch `codex/serea-g3-real-ai-feasibility` is pushed and verified at `6d6f362da2380b56ea0fb7bc9372dbcceae67e90`, tree `f4fe1cf534b855b58ea19683621bf3dc7f7a3cd6`; only `docs/agent-reports/g3-ai.md` changed. Orchestrator reviewed the exact diff, clean worktree, scope, explicit NOT_RUN inventory, model provenance/licensing caveats, and policy/runtime claims against current official Gemini, ML Kit GenAI, Ollama and llama.cpp sources. No correction requested. This report is feasibility evidence only; no model, cloud provider, or runtime is integrated or approved.
- PR #124 is an audit-only Draft based on main. Its Fast CI run `40488015151` failed before tests because main's existing `tools/check_commit_identity.py` scans `git log --all` in a full-depth checkout and finds unrelated commits from fetched refs. The newer #123 stack uses event-bounded commit-range validation. This is a stale-main-base integration issue, not grounds to relax the guard. G1-canceled broad runs `40488015109`, `40488015078`, `40488015079`, `40488015071` are not validation evidence.

## NEXT_ACTIONS

1. Rerun the 74-test JVM suite and isolated Android test-source compile on G4's integrated HEAD using the validated temporary JDK; inspect the resulting JUnit XML.
2. Run the bounded Fake Google UI instrumentation test through the disposable API 26/35 focused emulator workflow if the test identity is included; verify it executes and does not become a skip.
3. Update the G4 ledger with exact-head results, push the owned branch, and open a Draft PR against #123's head branch with the G1/G2/G3 evidence and Owner decisions.
4. Once the integrated source head is stable, run formal Required and relevant cross-workflow CI on that exact HEAD; do not inherit #123 PASS results.
5. Prepare release/OAuth/AI/Pixel checklists without using production credentials, services, or devices.

## OWNER_APPROVAL_REQUIRED

Real Google OAuth/account and scopes; real AI provider eligibility/terms/cost/credentials; production Beta signing and distribution; physical Pixel 7a acceptance; merge to main; security or Required-contract changes; final Personal Beta GO.
