# G1 — Git Integration & Release Architecture Audit

Audit date: 2026-10-27 (UTC)

## Agent report contract

- `AGENT_ID`: G1 — Git Integration & Release Architecture
- `ACTIVE_PHASE`: G1 main integration readiness audit
- `OVERALL_STATUS`: REVIEW_COMPLETE (scoped audit; no integration approval or merge)
- `BASE_SHA`: protected `main` `e0acb0254820c381dd97828824bef86a1db5176c`; functional baseline PR #117 `151ea9d66d32de01b195f4262beb67516823c281`
- `CURRENT_HEAD`: audited PR #123 candidate `3924b36080ffa43331f065a15db80c604c243355`
- `TREE_SHA`: `41cfce316c485abe31f86c827c6ca81a4d5c107b`
- `WORKTREE`: source audit in `/workspace/Serea`; no source edits. Disposable merge rehearsal was cleaned up. This checkpoint is on its own docs-only branch based on the audited PR #123 head.
- `BRANCH`: `codex/serea-g1-integration-audit-20261027`
- `DRAFT_PR`: checkpoint PR is stacked on PR #123; exact link/head is recorded in the orchestration response and live GitHub metadata.

## Live refs observed

| PR | State | Base ref / SHA | Head ref / SHA | Scope in the stack |
|---|---|---|---|---|
| #98 | Open / Draft | `main` / `e0acb0254820c381dd97828824bef86a1db5176c` | `codex/serea-final-candidate-00e0453-20261022` / `00e0453f83970a2b7d2bd26ce01462e421f75f46` | Initial secure Personal Beta candidate |
| #99 | Open / Draft | #98 head / `00e0453f83970a2b7d2bd26ce01462e421f75f46` | `codex/pr98-overnight-gate-recovery` / `8140bd759e7a01041364ebc443ba452f546d325a` | Recovery between #98 and #100 |
| #100 | Open / Draft | #99 head / `8140bd759e7a01041364ebc443ba452f546d325a` | `codex/pr99-ci-recovery` / `ad868af753672c546c52411ce151904360633acc` | CI recovery |
| #117 | Open / Draft | #100 head / `ad868af753672c546c52411ce151904360633acc` | `codex/serea-personal-beta-integration-clean` / `151ea9d66d32de01b195f4262beb67516823c281` | R4–R7 Required Closure baseline |
| #122 | Open / Draft | #117 head / `151ea9d66d32de01b195f4262beb67516823c281` | `codex/serea-personal-beta-functional-integration` / `b64a517ec33dd672c17cd604a912045116ad963a` | P0 functional integration sibling |
| #123 | Open / Draft | #117 head / `151ea9d66d32de01b195f4262beb67516823c281` | `codex/serea-personal-beta-functional-integration-identity` / `3924b36080ffa43331f065a15db80c604c243355` | Frozen functional integration candidate |
| #125 | Open / Draft | #123 head / `3924b36080ffa43331f065a15db80c604c243355` | `codex/serea-g4-personal-beta-readiness` / `c9ce1c2674e57dbab787d97be5f24c9822f4fe22` | G4 readiness work; separate from the frozen #123 baseline |
| #120 | Open / Draft | Functional branch snapshot / `d74998f60c9062343ce9c62a326343e9a4d18e45` | `codex/serea-functional-security-release-verified` / `772b1cd37f43b3f92276271abe8c67aabb74f7d0` | Security / release lane |
| #121 | Open / Draft | Functional branch snapshot / `7d1ddfdc18425901af1889f3d4730e8e5624a664` | `codex/serea-adversarial-reliability-v2` / `8853c2afe25497e8abdd55282e85331efee3f009` | Adversarial / reliability lane |

Live `main` remained `e0acb0254820c381dd97828824bef86a1db5176c` at the time of audit.

## Dependency graph and provenance

```text
main
└── #98
    └── #99
        └── #100
            └── #117
                ├── #122 (functional sibling)
                └── #123 (frozen functional candidate)
                    └── #125 (G4 readiness)
```

The ancestry checks confirmed the #98 → #99 → #100 → #117 → #123 chain. PRs #122 and #123 are siblings with common base #117; neither head is an ancestor of the other.

PR #123 is 409 commits and 241 files ahead of current `main`. This is cumulative stack size, not evidence that every change lacks prior verification. The final #123 tree is `41cfce3…`; its merge ref `fdbfedd2167173f15c6cdaac5cd03764da5131cd` has the same tree.

### PR #122 versus #123

The common merge base is `151ea9d66d32de01b195f4262beb67516823c281`. `git range-diff` paired all 42 commits unique to #122 with equivalent patches in #123. The first pair differs in author metadata; its stable patch ID is identical (`58565525…`). The remaining mapped pairs are patch-identical. PR #123 adds 28 later commits. The resulting trees differ in 22 files (1,051 insertions and 146 deletions), primarily from later authorization demo UI, IME/UI reliability, and test-harness changes. No #122-only patch was found missing from #123.

The relevant #120/#121 lane work is also represented in #123 by patch-equivalent commits:

- #120 SBOM/release-security implementation `90a1add…` ↔ #123 `6478081…`, patch ID `bc3b2cfc…`.
- #120 release-check integration `772b1cd…` ↔ #123 `1464c59…`, patch ID `8571a228…`.
- #121 adversarial demo/Egress tests `8853c2a…` ↔ #123 `6990a09…`, patch ID `eb9564e5…`.

Those PR heads are not ancestors of #123. Their relevant patches are present, so merging their entire branches as well would duplicate work; their independent CI evidence remains tied to their own heads.

## Required Gate contract audit

Between #117 and #123, the only Required-adjacent identity file changed is `android/app/src/androidTest/assets/serea-non-required-test-identities.txt`, which adds three Google authorization demo UI tests as explicitly non-required. The formal Required and Additional Test manifest files and verifier are unchanged:

- Original Required manifest: 103 entries.
- Additional manifest: 2 entries.
- Explicit non-required diagnostics: 8 entries.
- The campaign verifier enforces exact count 105 and zero failures, errors, or skips.

Android run **40427448635** completed successfully at source head `3924b360…`; all Android workflow jobs succeeded, including Formal Identity and the Required Emulator Gate. I downloaded and parsed the API-specific artifacts:

| API | Artifact ID | JUnit result | Source SHA |
|---|---:|---|---|
| 26 | `12701024855` | 105 tests, 0 failures, 0 errors, 0 skipped | `3924b360…` |
| 35 | `12702662029` | 105 tests, 0 failures, 0 errors, 0 skipped | `3924b360…` |

The run’s Pixel 7a reacceptance artifact **12700507955** records source SHA `3924b360…`, checkout commit `fdbfedd…`, and the same candidate tree SHA `41cfce3…`. Its APK is the debug/demo package `app.serea.android.demo`, SHA-256 `0d6f81ba766c15a473fe47bb62bb1ddd1fcdcaa9f81e81e5690b27d2b50de058`, signed by the Android Debug certificate. It is not a production Beta APK or an Owner-signing test.

The Beta release workflow in the candidate is `workflow_dispatch` and has a job-level `github.ref == 'refs/heads/main'` guard. It validates protected-main and required-check state, requires the protected `serea-personal-beta` environment, reads signing secrets only inside that job, and defaults distribution to false. Production signing and Firebase distribution remain Owner-controlled.

## Main integration rehearsal

A disposable worktree was created from `main` and merged with PR #123 using `git merge --no-commit --no-ff`. Git reported an automatic merge with no conflicts. The rehearsal tree SHA equalled PR #123’s tree SHA `41cfce316c485abe31f86c827c6ca81a4d5c107b`. The merge was aborted and the scratch worktree/branch removed; `main`, all PR refs, and the audited candidate were not changed.

`git diff --check` found one existing whitespace issue in PR #123:

- `docs/plans/2026-10-26-personal-beta-owner-decision-ledger.md:3` — trailing spaces after the update date.

This is a documentation hygiene issue, not a merge conflict or runtime defect. It should be cleaned up in a reviewed follow-up if the source candidate is edited.

## Proposed integration and rollback

If the Owner chooses to integrate the stack, the topological sequence is #98 → #99 → #100 → #117 → #123. Do not additionally merge #122, #120, or #121 wholesale; their relevant #122/#120/#121 patches are already represented in #123. Keep all existing Draft PRs open until the Owner decides their disposition.

The rehearsal confirms tree compatibility only; it does not approve the 241-file cumulative source diff. Any production integration requires Owner approval. Use ordinary revert commits on a follow-up branch if a merged candidate needs rollback; do not reset or force-push protected history.

## Evidence and commands

No tests, Emulator, Required Gate, or release workflow were launched for this audit. Evidence came from live GitHub refs and the completed PR #123 run.

Commands used included:

```text
gh api repos/Yrika819/Serea/pulls/{98,99,100,117,120,121,122,123,125}
git fetch origin refs/pull/{98,100,117,120,121,122,123,125}/head

git merge-base refs/g1-audit/pr122 refs/g1-audit/pr123
git rev-list --left-right --count refs/g1-audit/pr122...refs/g1-audit/pr123
git range-diff 151ea9d66d32de01b195f4262beb67516823c281..refs/g1-audit/pr122 \
  151ea9d66d32de01b195f4262beb67516823c281..refs/g1-audit/pr123
git patch-id --stable
git diff --check origin/main refs/g1-audit/pr123
gh run view 40427448635 --repo Yrika819/Serea
gh api repos/Yrika819/Serea/actions/runs/40427448635/artifacts
git worktree add -b codex/g1-main-integration-rehearsal ... origin/main
git merge --no-commit --no-ff refs/g1-audit/pr123
```

JUnit artifact XML totals were parsed directly. The downloaded API 26/API 35 JUnit and Pixel reacceptance artifacts were local read-only evidence; no APK was installed on a physical device.

## Security and Owner boundaries

- No production secret, real Google account, real AI provider credential, production signature, or Firebase distribution was used.
- No PR, `main`, or another agent’s branch was modified by the audit.
- `main` merge, production signing/distribution, real-service access, and Personal Beta GO remain Owner decisions.
- No functional or security policy change is proposed by this audit.

## Integration conclusion

The PR dependency chain is understandable and the current #123 tree can be merged into current `main` without content conflicts. PR #123 contains all 42 functional patches represented in sibling PR #122, plus later candidate changes, and its exact head has passing API 26/35 Required evidence. The remaining integration decision is Owner-controlled; this audit is not merge approval or Personal Beta GO.
