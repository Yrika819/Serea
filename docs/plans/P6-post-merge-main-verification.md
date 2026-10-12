# P6 post-merge main verification

**Evidence date:** 2026-10-12
**State:** `P6A_P6F_MERGED_MAIN_VERIFIED`
**Scope:** historical integration and post-merge CI evidence; this record does not close P7 or P8.

This record supersedes the branch-state observations in [P6-closure.md](P6-closure.md) and [P6F-closure.md](P6F-closure.md). Those documents accurately reported that their reviewed heads were unmerged when written. Their historical wording is preserved; the later merge and main CI evidence below is the authoritative integration record.

## Merge evidence

| PR | Merge SHA | First parent | Second parent | Merge tree | Result |
| --- | --- | --- | --- | --- | --- |
| [#6](https://github.com/Yrika819/Serea/pull/6) | `9d2e0c442e419eccb1381bb27879eb5c704fa3fb` | `e17fbfc7f3e75f0bc95a66d693fe11b57a6ced50` | `1c04451469a810a484e92fa7f0e5eb249451a54e` | `888d64cd0efa4c70f87aa09acbecc52d4af0fd05` | MERGED |
| [#10](https://github.com/Yrika819/Serea/pull/10) | `f70f34c1f683df1b175b612ba100ce6bb6979811` | `4c801275398c568cce82377fdbe4640a11ff6d0e` | `9d2e0c442e419eccb1381bb27879eb5c704fa3fb` | `2a3b0d5150d7800877befc1d6003c3c38fceec65` | MERGED |

PR #10's merge tree equals the recorded `main` HEAD tree. The second parent of #10 is the merge commit for #6, preserving the dependency order #6 → #10. GitHub reports both pull requests closed and merged. `origin/main` and the checked-out baseline both resolve to `f70f34c1f683df1b175b612ba100ce6bb6979811`.

## Post-merge main checks

All three workflow runs tested exact head SHA `f70f34c1f683df1b175b612ba100ce6bb6979811`; every listed job concluded `success`.

| Workflow | Run ID | Job ID | Job | Conclusion |
| --- | ---: | ---: | --- | --- |
| Fast CI | `38159594830` | `114529536389` | Linux fast checks | success |
| Full CI | `38159594837` | `114529537286` | macOS Intel x86_64 | success |
| Full CI | `38159594837` | `114529537389` | Linux stable full validation | success |
| Full CI | `38159594837` | `114529537459` | Linux MSRV 1.85.0 | success |
| Full CI | `38159594837` | `114529537465` | macOS arm64 | success |
| SQLite portability | `38159594774` | `114529536919` | Produce closed DB on macOS arm64 | success |
| SQLite portability | `38159594774` | `114529536982` | Produce closed DB on macOS Intel x86_64 | success |
| SQLite portability | `38159594774` | `114529737179` | Open arm64 DB on macOS Intel x86_64 | success |
| SQLite portability | `38159594774` | `114530149416` | Open Intel DB on macOS arm64 | success |

The workflow definitions at `.github/workflows/ci.yml`, `full-ci.yml` and `sqlite-portability.yml` identify the corresponding validation scopes. This is evidence of the recorded CI jobs at that SHA, not a claim about physical power-loss durability or production external-provider behavior.

## P6 closure and migration identity

P6A–P6F are integrated on main. The accepted contract decisions and implementation closure recorded by PR #6 and #10 remain in force. Migrations `0001`–`0008` are present and immutable at this checkpoint. Current SHA-256 identities:

| Migration | SHA-256 |
| --- | --- |
| `0001_initial.sql` | `d9068dccbc26ececb71be79c475080633166ba0163c62b2d98b9733512baefea` |
| `0002_event_scheduler.sql` | `4924e69150bbff9c39e2e6b7e2bdd61045202e504900fe0f510d513fbf815e67` |
| `0003_model_accounting.sql` | `530a6d6cb5ec9c757311d48e10a62ef456d9d01c09512f321cffe42fe3307f80` |
| `0004_capability_registry.sql` | `06b22fa682564290b71a26825a777f7a295220bd02f4c6c614d234b73001685f` |
| `0005_policy_approval.sql` | `cf6686807e3ceb10bd21c6653fbdd61c487e6b8938a2d78104d160920831e71a` |
| `0006_policy_authority_sealing.sql` | `0af16a30cd2b29f046c14d7079a356dc25ce4352216edd9ad6821b168e6eb6ca` |
| `0007_policy_revision_sealing.sql` | `48f2f1805d59f4eec215963ac5109007294a0dfe015b9a4b4e7e69c14f9197eb` |
| `0008_approval_lifecycle_routing.sql` | `6e1520bf22ced832ed480a1cf26476e056611f22b1f517f2324b32a8d4fbf925` |

This record does not authorize migration edits; future schema work requires additive migrations and its own owner-approved contract.

## Scope and residual limits

P6 provides durable Policy and Approval state and task orchestration. P6 contains no provider invocation, no `CapabilityProvider::invoke`, no P8 dispatch intent, and no P8 `RequestId` minting. The existing `TaskEngine::evaluate_prepared_action` returns a point-in-time policy evaluation; that result alone is not dispatch authority. The residual boundary is recorded as a NOTE in the P7/P8 audit: P8 must perform fresh Task/Step/lease/plan/registry/policy/approval/availability/bounds checks and atomically commit authorization consumption with durable dispatch intent before provider IO.

The host remains responsible for authenticated user/device/session identity, elevated authentication where required, credential custody, provider/account binding, egress decisions, and truthful result/receipt validation. The CI evidence does not establish complete production PRIVATE ordinary-row protection. ADR-0022 remains Proposed; PRIVATE-bearing ordinary rows fail closed. P6 does not guarantee exactly-once external effects, secure physical erasure, or protection against a compromised host, operating system, provider, or backup operator.

No P7/P8 implementation or completion is claimed. Current phase state remains `PREIMPLEMENTATION_RESEARCH_ONLY`.
