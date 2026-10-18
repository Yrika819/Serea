# P8B Draft PR stack audit

**Result: `STACK_INTEGRATION_DECISION_READY`; this is not merge authorization.**
Repository `Yrika819/Serea`; audit source PR #36 HEAD
`bfe47522d808388f82b55c67793184726273600d`; `main` verified at
`e0acb0254820c381dd97828824bef86a1db5176c`. GitHub open PR census returned
exactly 19 Draft PRs: #14 and #19–#36. Per-PR machine-readable metadata,
changed file paths, commits, merge bases, base SHAs, status checks and
recommendations are in [P8B-STACK-MANIFEST.json](P8B-STACK-MANIFEST.json).

## Inventory and disposition

| PR | Head | Base | Commits ahead of base | Classification | Reason / next handling |
|---:|---|---|---:|---|---|
| 14 | `cb4df717` | `main` | 1 | `SUPERSEDED_OR_REDUNDANT` after scratch rehearsal | Independent P5 activation correctness fix. Local merge rehearsal against current `main` produced no tree delta: current main already has equivalent content. Verify with Owner before closing; do not merge a no-op. |
| 19 | `4032f1e2` | `main` | 4 | `REVIEW_ONLY_RESEARCH` | P8A / Gate-C and migration review package. Docs context only; can review separately. |
| 20 | `5f78762e` | #19 branch | 1 | `MERGE_CANDIDATE` | ADR and protocol direction docs; reconcile against #19 and preserve exact approved direction-only scope. |
| 21 | `a56c7e20` | #20 branch | 1 | `REQUIRES_REDESIGN` | Isolated migration 0009 candidate and disposable tests; never canonical without new contract/migration approval. |
| 22 | `58944dd0` | #21 branch | 1 | `REVIEW_ONLY_RESEARCH` | Isolated 0010 Intent schema candidate, outside catalog; research only. |
| 23 | `b6667d19` | #22 branch | 3 | `REVIEW_ONLY_RESEARCH` | Synthetic authority transaction/recovery prototype. |
| 24 | `fab5ee91` | #23 branch | 3 | `REVIEW_ONLY_RESEARCH` | Real Storage transaction boundary tests; no production writer. |
| 25 | `73e38fdd` | #24 branch | 3 | `REVIEW_ONLY_RESEARCH` | Budget/Intent test gaps and decision package. |
| 26 | `72be5ac5` | #25 branch | 3 | `REVIEW_ONLY_RESEARCH` | Synthetic budget race, Fake crash and one-shot Permit evidence. |
| 27 | `c08615aa` | #26 branch | 5 | `REVIEW_ONLY_RESEARCH` | Synthetic Permit registry/overlay pinning. |
| 28 | `067c5e4e` | #27 branch | 2 | `MERGE_CANDIDATE` | ADR-0045 direction record plus test-only evidence; extract to a clean review PR without prototype ancestry. |
| 29 | `bb4c4d2b` | #28 branch | 3 | `BLOCKED_OWNER_DECISION` | ADR-0046 candidate C remains proposed. Keep probe/docs available as research; do not adopt topology. |
| 30 | `ba16eaa7` | #29 head branch | 4 | `SUPERSEDED_OR_REDUNDANT` | Transaction-scoped P6 snapshot proof; all four commits are included in #31. Do not integrate independently. |
| 31 | `1adb8e26` | #29 head branch | 6 | `REVIEW_ONLY_RESEARCH` | Contains #30's history and extends it with L1A observer/facts. Prefer #31 as the evidence record if retaining either. |
| 32 | `17d8d5e5` | #31 branch | 3 | `REVIEW_ONLY_RESEARCH` | Test-only observer-barrier evidence. |
| 33 | `a8210bca` | #32 branch | 1 | `REVIEW_ONLY_RESEARCH` | Grant revocation race test; synthetic accepting verifier is not production Owner auth. |
| 34 | `f3804a64` | #33 branch | 1 | `REVIEW_ONLY_RESEARCH` | External trust-root probe and Stable ARM64 failure evidence; diagnose failure before any merge proposal. |
| 35 | `b9a0ef07` | #34 branch | 2 | `REVIEW_ONLY_RESEARCH` | Full Grant set test evidence only. Green CI does not establish authorization. |
| 36 | `bfe47522` | #35 branch | 2 | `REVIEW_ONLY_RESEARCH` | Additional lock/snapshot test evidence; Stable ARM64 failure persists. |

Every PR in the census was Open and Draft at audit time. `#14`'s local
scratch merge resolved without a file conflict and resulted in no tree change
relative to `main`. Most stacked PRs have a clean automatic commit ancestry;
this is a Git property, not a review of contract or security scope. The
machine manifest records exact check rollups and file lists rather than
inferring them from titles.

## Dependency, duplicate and policy findings

The stack's Git ancestry is:

```text
#19 → #20 → #21 → #22 → #23 → #24 → #25 → #26 → #27 → #28 → #29
                                                       #30 ─┐
                                                            └→ #31 → #32 → #33 → #34 → #35 → #36
```

The diagram's #30/#31 relation is strict: #31's six-commit range includes all
four #30 commits and adds two commits. #30 and #31 also overlap in policy
snapshot/storage test and external Cargo probe paths. Treat #30 as subsumed;
do not apply its patch independently before #31.

P7 already has merged synthetic work (#13 and #15). The #21 migration 0009 and
#22 migration 0010 candidates are in isolated candidate paths and are not
catalog entries. They cannot be called the approved/current P7 schema. The P8
chain #22–27 contains Synthetic prototype/test evidence, not a production
Intent writer, Permit issuer, or dispatch route. ADR-0045 in #28 is a record of
ratified *directions* (D1/D2/D3/D5; D4 interim scope), not approval of every
later prototype. ADR-0046 in #29 remains PROPOSED / NOT RATIFIED. CI green and
GitHub's mergeable state do not change these gates.

## Exact-head CI and review limits

All 19 PR heads and their visible check rollups are recorded in the JSON
manifest. PR #34 and #36 have a Stable Windows ARM64 Native failure; #35 is
green and the Rust 1.85 Windows ARM64 controls for #34/#36 pass. Other displayed
checks are green for the audited heads. This status is not main validation and
does not resolve the ARM64 failure. Branch protection rules were not separately
queried, so required-review/check policy is not inferred.

For merge order and the local-only rehearsal, see
[P8B-INTEGRATION-PLAN.md](P8B-INTEGRATION-PLAN.md). No remote branch, PR state,
base, or main was changed by this audit.
