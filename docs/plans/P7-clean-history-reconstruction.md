# P7 documentation history reconstruction record

**Status:** Clean-history replacement prepared; remote-ref retirement pending owner authorization.  
**Scope:** Documentation only. No production code, migration, ADR body, or frozen protocol changed.

## Recorded Git state

| Object | SHA |
| --- | --- |
| Verified `main` base | `f70f34c1f683df1b175b612ba100ce6bb6979811` |
| Last good commit on original PR #11 | `5a19dade4ece21223014e6ca2a186956d09906d0` |
| Original PR #11 problematic head | `9a627b097c88f2f30618e88627427601979596d6` |
| Original PR #11 tree | `fae8f71920a29d034598d13a07825bc1eb727381` |
| Clean reconstruction commit before later doc corrections | `e65be65c90c683d928f43c6b8c06c4fb72e47a4f` |
| Reconstructed tree before later doc corrections | `fae8f71920a29d034598d13a07825bc1eb727381` |
| Current clean reconstruction branch | `p7-p8/owner-decision-preparation-clean` |

The full PR diff was captured from verified `main` to original PR head, saved outside the repository as `/tmp/serea-pr11-full.diff`, and had SHA-256 `1ef3d736b74b130f8578f90bc4b10e48218ee953cd53c1876faaab56acd92255`. It was applied as one staged patch in a separate worktree based directly on the verified main commit. `git write-tree` returned `fae8f71920a29d034598d13a07825bc1eb727381`, exactly equal to the original PR head tree, before any P7 technical correction. The first replacement commit has only the verified main commit as parent and was made with both author and committer set to `Codex <noreply@github.com>`.

After that equivalence proof, focused Forget/numeric corrections and this record are added in a separate commit. The current branch tree therefore intentionally differs from the original PR tree only by those subsequent documented corrections; compare the second commit's diff to review them.

## CI identity ref scope

Fast CI and Linux stable Full CI set `actions/checkout` `fetch-depth: 0`, then run `python3 tools/check_commit_identity.py`. That checker evaluates `git log --all`. With full history, checkout fetches remote branch refs (`refs/heads/*` into `refs/remotes/origin/*`) and the current PR merge ref. The original remote branch `refs/heads/p7-p8/owner-decision-preparation` still points to the problematic commit, so its remote-tracking ref makes that commit reachable to `git log --all` in the replacement PR checkout. A clean branch alone cannot make CI pass while that ref remains fetched.

The checker passed in an isolated verification repository whose only branch refs were verified `main` and the clean replacement branch. In the original checkout, the checker continues to find the bad commit through `refs/remotes/origin/p7-p8/owner-decision-preparation`. This is expected and is not a reason to weaken the guard.

## Audit relationship and authorization boundary

The replacement branch is intended as a clean-history review successor to [original PR #11](https://github.com/Yrika819/Serea/pull/11). PR #11 and its branch remain available as audit evidence. The bad commit remains reachable by its public SHA and is not represented as erased.

To obtain green identity-guard CI for a new PR, the minimum likely retirement is deletion of the old remote branch ref after rechecking its exact SHA, while leaving PR #11 open and retaining its review/audit record. This branch deletion is an owner-controlled, externally visible action and has **not** been performed. It does not erase the commit object or guarantee the SHA becomes unreachable elsewhere. Owner authorization is required before deleting the old branch. If authorized, re-fetch refs immediately before deletion, verify the exact branch still points to `9a627b097c88f2f30618e88627427601979596d6`, delete only that branch, keep PR #11 open, then verify the replacement PR's fetched refs and actual CI results. Do not merge or mark either PR ready for review.
