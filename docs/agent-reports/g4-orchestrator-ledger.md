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

- ACTIVE_PHASE: G1/G2/G3 independent work in progress; G4-A integration baseline and provenance setup.
- GOAL_STATUS: PARTIAL.
- BASE_SHA / LAST_GOOD_HEAD: PR #123 functional candidate `3924b36080ffa43331f065a15db80c604c243355`.
- CURRENT_HEAD: Resolve from branch `codex/serea-g4-personal-beta-readiness`; the initial commit is a checkpoint-only descendant of BASE_SHA. Candidate test evidence belongs to BASE_SHA only; checkpoint-only commits do not inherit exact-head Required status.
- OWNED_BRANCH: `codex/serea-g4-personal-beta-readiness`.
- WORKTREE: `/workspace/Serea-g4-personal-beta-readiness`.
- Checkpoint branch pushed and GitHub-verified at `06d182db41f957fdbb9f29b48ae50cd318e465f9` on 2026-10-27; this is documentation only and has no inherited test status.
- DRAFT_PR: not created. Create after G4 has reviewable integrated changes and a deliberate base decision; do not create a 409-commit main PR without G1's reviewed rehearsal.
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
| G1 | Main integration and release architecture | Working; interim graph/rehearsal evidence received, final audit pending | TBD | TBD | NOT_REVIEWED |
| G2 | Google OAuth/Gmail/Calendar readiness | Working; interim scope/test report received, exact head/result pending | TBD | base `3924b360…` | NOT_REVIEWED |
| G3 | Real AI / on-device inference feasibility | PARTIAL; report complete, PoC NOT_RUN | None | `6d6f362da2380b56ea0fb7bc9372dbcceae67e90` | REVIEWED — documentation-only; no correction requested; no runtime integration approved |
| G4 | Orchestration, integration, independent security review and Required validation | Active | TBD | G4 branch above | BASELINE_VERIFIED; new changes pending |

Each agent owns a separate worktree, branch, and checkpoint. No agent may modify another lane's files or branch. Only this Orchestrator edits this ledger.

## P0 / service state

- Local/Fake Chat, Gmail, Calendar, Local Tasks, Egress/session tests and exact-head Required are verified on the frozen candidate.
- Real Gmail/Calendar OAuth, real AI selection/activation, production signer/distribution, and Pixel 7a acceptance are Owner-gated or device-unavailable and remain NOT_RUN.
- Lane B PR #120 and Lane C PR #121 remain separate Drafts; selected tests/release checks already present in candidate are not automatically duplicated or merged.
- No Required acceptance identities, counts, thresholds, skip conditions, security gates, or workflow requirements may be relaxed.

## Current risks / decisions

- PR #122 and #123 diverged from a shared base. G1 must review semantic file/contract differences, not infer equivalence from ancestry or head names.
- PR #123 has a large cumulative diff against main; staged provenance and reviewed integration sequence are required before a main-based candidate is considered.
- Google policy and scope research is not permission to use an account or enable OAuth.
- Host/Emulator AI inference evidence is not Pixel 7a performance evidence. No real API key or model service call is authorized.
- Main merge, production signing, Firebase distribution, real accounts/provider credentials, Pixel 7a operation, and final GO require Owner decision.
- G3 preliminary note says documented Gemini/ML Kit routes have age/consumer-use restrictions and Pixel 7a is absent from the currently listed supported device set; this is not yet accepted as a final eligibility finding. Account-specific age/region eligibility and primary-source citations remain unreviewed. G3 reports no local Android NDK/CMake and is not downloading a large model without a justified, reproducible PoC plan.
- G1 interim reports #98 → #99 → #100 → #117 and sibling #122/#123 from #117; #122/#123 diverge at #117. It reports 45 versus 61 changed paths, no #122-only paths, 39 shared paths byte-identical, 6 evolved in #123, 16 additions in #123, and a conflict-free disposable main-based merge whose tree equals #123. This remains interim and NOT_REVIEWED until exact refs, semantic path list, parent SHAs, and required workflow audit are provided.
- G2 interim reports existing `googleauth`, provider-session, Fake gateway, and MockWebServer tests already cover core authorization/session behavior. It is changing the fake Settings walkthrough and a JVM state test for explicit auth states. First JVM attempt exited 127 because the configured JDK lacked `javac`; a rerun with a temporary JDK 21.0.13 reports 70 tests, 0 failures/errors/skips (new lifecycle test 2; auth/provider tests 68). Exact branch HEAD, source/artifact and JDK provenance, Android UI compile result, and worktree state remain pending Orchestrator review. Product UI reachability of real Google providers is specifically under review; no real account/OAuth or emulator is used.
- G3 report branch `codex/serea-g3-real-ai-feasibility` is pushed and verified at `6d6f362da2380b56ea0fb7bc9372dbcceae67e90`, tree `f4fe1cf534b855b58ea19683621bf3dc7f7a3cd6`; only `docs/agent-reports/g3-ai.md` changed. Orchestrator reviewed the exact diff, clean worktree, scope, explicit NOT_RUN inventory, model provenance/licensing caveats, and policy/runtime claims against current official Gemini, ML Kit GenAI, Ollama and llama.cpp sources. No correction requested. This report is feasibility evidence only; no model, cloud provider, or runtime is integrated or approved.
- G1 interim provenance reports `main` is an ancestor of #117/#123, the ordered stack is #98→#99→#100→#117→#123, and #122 is a sibling of #123. A disposable merge rehearsal reportedly had no conflicts and produced #123's tree exactly; #120/#121 changes named in that report are already present in #123 so no duplicate cherry-pick is indicated. G1 reports Required manifests/verifier are unchanged, and its artifact download attempt returned 403, so JUnit XML was not independently parsed. Exact rehearsal parents/commit, semantic path inventory, and workflow audit remain pending; not yet approved.

## NEXT_ACTIONS

1. G1/G2/G3 create independent worktrees, inspect existing implementations, and return exact HEAD/test evidence in their dedicated checkpoint docs.
2. Orchestrator fetches each branch/diff and independently reviews scope, tests, security, base provenance, and Required impact; send formal CHANGES_REQUESTED records where needed.
3. G1 completes a disposable main integration rehearsal; G2/G3 changes remain separate until reviewed.
4. After G2/G3 Focused evidence, integrate only non-overlapping approved changes into this G4 branch, then run integration and security review.
5. Run Required and cross-workflow CI only after the integrated source head stabilizes. Do not mix baseline PASS evidence with new source heads.
6. Prepare release/OAuth/AI/Pixel checklists without using production credentials, services, or devices.

## OWNER_APPROVAL_REQUIRED

Real Google OAuth/account and scopes; real AI provider eligibility/terms/cost/credentials; production Beta signing and distribution; physical Pixel 7a acceptance; merge to main; security or Required-contract changes; final Personal Beta GO.
