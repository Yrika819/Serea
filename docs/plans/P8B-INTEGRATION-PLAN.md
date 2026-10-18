# P8B integration plan and scratch rehearsal

**Scope: recommendation only.** The rehearsal started at `main`
`e0acb0254820c381dd97828824bef86a1db5176c` in local branch
`codex/local-not-for-merge-main-rehearsal` and worktree
`/workspace/Serea-main-rehearsal`. It merged local `origin/p5/concurrent-manifest-install-idempotency`
with `--no-commit --no-ff`; no push or commit occurred. The merge resolved
without conflicts and the resulting tree had no delta from `main`. PR #14's
content is therefore already represented in the current main tree even though
its commit is not an ancestor. The rehearsal branch/worktree is scratch only
and is not a merge candidate.

## Recommended review and integration sequence

| Order | Candidate | Action | Required gate |
|---:|---|---|---|
| 0 | PR #14 | Confirm with Owner that the exact P5 diff is already present on main; then close as redundant if verified. Do not merge a tree no-op. | Compare PR branch patch and current main file/test behavior. This audit did not change PR state. |
| 1 | PR #19 | Review as an independent docs/research package. | Ensure no obsolete migration/runtime implications. |
| 2 | PR #20 | Review/merge only after #19 context reconciliation; extract the direction-only ADR/protocol documentation. | Keep only explicitly approved scope and preserve PROPOSED statuses elsewhere. |
| 3 | PR #28 | Prepare a clean, standalone extraction of ADR-0045 and narrowly scoped test evidence. | Remove inherited candidate schema/prototype content; do not imply runtime readiness. |
| 4 | PR #21–27 | Keep as research; do not integrate as a cumulative product stack. | Any future schema/Intent/Permit work needs Owner contracts, migration plan and separate approval. |
| 5 | PR #29 | Keep proposal and external-surface evidence for decision review. | Owner must explicitly select/revise/reject Candidate C before implementation adoption. |
| 6 | PR #31–36 | Retain one grouped evidence series or archive redundant layers after review; #31 subsumes #30. | Resolve ARM64 diagnosis and reduce overlapping evidence/docs before proposing any clean-up PR. |

The full cumulative ancestry order is #19 → #20 → #21 → #22 → #23 → #24 →
#25 → #26 → #27 → #28 → #29 → #31 (includes #30) → #32 → #33 → #34 → #35 →
#36. This is ancestry, **not** a recommendation to merge that stack. It carries
unratified schema candidates and synthetic authority prototypes. #14 is
independent of that stack.

## Technical application vs normative approval

| Bundle | Technical result | Normative state |
|---|---|---|
| #14 onto current main | No resulting file change in scratch merge; branch tree is already represented. | Owner review/close decision still needed; no action taken. |
| #19 docs | Expected conflict-free if applied after current main; not applied in this rehearsal. | Research only. |
| #20 direction docs | Git ancestry is available; no independent scratch merge was performed. | Limit to explicitly recorded direction clauses. |
| #28 ADR-0045 evidence | Must be extracted from stacked branch to avoid importing earlier prototypes. | D1/D2/D3/D5 are directions; D4 remains interim. |
| #21–27 cumulative branch | Git ancestry can apply in sequence but brings candidate 0009/0010 and Synthetic prototypes. | `NOT_FOR_MERGE` without separate Owner decisions. |
| #29–36 evidence chain | Descendant history applies as a chain; #31 contains #30; #34/#36 have known ARM64 failures. | Evidence does not ratify ADR-0046 or authorize dispatch. |

No broad scratch merge of #19–36 was run because its known contents include
unratified migrations and authority prototypes. No scratch run is represented
as main CI or as merge permission. No migration was added to the canonical
catalog; no ADR status or architecture version was changed.

## Proposed organization work

After the Owner decisions in [P8B-OWNER-DECISION-PACKET.md](P8B-OWNER-DECISION-PACKET.md),
the smallest useful cleanup is one documentation/research consolidation PR:

1. preserve a single provenance trail for #30/#31–36 evidence and label tests
   as test-only;
2. move no candidate SQL into the canonical migration catalog;
3. present ADR-0045 accepted directions separately from ADR-0046 proposal;
4. add a Native diagnosis result without retry/sleep/skip behavior;
5. keep any future Candidate C code proof test-only until its trust sources are
   explicitly selected.

Potential close/archive candidates after Owner review: #14 if the tree
equivalence is confirmed; #30 because #31 includes it. Other research PRs may
be retained for provenance or closed after their evidence is consolidated.
This report does not close, retarget, merge or edit any PR.
