# G8 integration source matrix (2026-11-03)

## Baseline and candidate

- Repository: `Yrika819/Serea`
- Base: `main` at `5bb1df8dd09bd906e6bf8efa61d1bf77a7d6fc69`
- Local integration branch: `codex/serea-g8-main-candidate-20261103`
- Candidate starts with merge commit `34df47cd5d9ed5d4215e1c03720d42482c81fbe7`, whose first parent is the baseline main and second parent is PR #146 HEAD `d444e7cc04c8f1c4c6d12b1412cfe3a76258af66`.
- Local candidate currently has three patch-equivalent commits for the Gmail body-only implementation/tests from #150, plus focused harness/test fixes under review. A new merge-base-based PR will be opened to main after these changes are reviewed.

## Source selection

| Source | Current source HEAD | Relationship / selected work | Excluded work and evidence |
|---|---|---|---|
| #146 Fake Google / Calendar UI | `d444e7cc04c8f1c4c6d12b1412cfe3a76258af66` | Integrated by a regular merge from current main. Includes synthetic Fake Google consent/account/session UI, read-only Calendar loopback, revocation and activity recreation paths, isolated process-death harness, plus JVM and instrumentation coverage. | Historical evidence bundles and stale checkpoint material are not carried into the candidate. Source PR’s previous `Android Storage and Emulator` and `Android Emulator Required Gate` checks failed, so those results do not establish candidate correctness. |
| #149 Emulator harness recovery | `3a0f0c1897f2115549e7682d6fcc1783c44604f3` | Selected the fail-closed Android package absence verification, G8 focused test inventory expansion to both API levels, and diagnostic instrumentation fixes. Candidate tests retain existing harness regression coverage and separately verify the new behavior. | G8N native-runtime and unrelated checkpoint/evidence files are excluded. The old #149 CI is not reused as evidence. |
| #150 Gmail body-only handoff | `50a56acd22fb363cf5354a3d39e67b01d980491a` | Three implementation/test commits are included: `Minimize Gmail chat handoff content`, metadata-locality assertion, and metadata/HTML fallback tests. They are patch-equivalent to #151 commits `2472daaa39e33012979d576b3633c7e1966225cf`, `f6694ef31058c6a10d286beed0187d69436904d0`, and `b345c0f9f7850e7ca050066e70b6f09e11cba086`, respectively. Candidate has its own shell test asserting Gmail subject does not enter Chat composer. | #150 evidence snapshots and lane reports are not treated as current-candidate evidence. Prior source CI had Android harness failures; only new candidate runs can qualify. |
| #151 G9 functional candidate | `d4a36fb5d3613bca2d60151de0991a1baac72fc9` | Used only to compare duplicate Gmail body-only implementation and tests. | G9-only scope, #149 ancestry, and G9 evidence/checkpoint commits are not merged. #151 is not an integration source. |

## Dependency and duplication findings

- #149 and #150 are siblings based on #146; neither is wholesale merged. Their selected code/tests are layered on top of the one #146 merge.
- #151 contains #149 ancestry and patch-equivalent Gmail changes. It is used as a comparison point only; no G9 feature or evidence is brought into this G8 candidate.
- LocalModel Runtime remains excluded. Existing main default-off / offline boundaries are untouched.
- No main-side G7 CI, identity, Rust Security, OAuth, Cloud Egress, or Required Gate workflow is intentionally changed. The candidate-specific diff is confined to the G8 implementation/tests plus selected emulator harness repair and this report.

## Validation state

- Candidate-specific host harness tests: 40 passed on the current working tree after restoring all pre-existing harness tests; includes the expanded 16-test G8 diagnostic manifest and fail-closed package-probe coverage.
- Shell syntax and `git diff --check`: passed before the last test assertion-only edit; rerun before commit.
- Android Gradle unit tests and isolated Android-test Kotlin compilation: in progress; result must be recorded before candidate commit.
- API26/API35 emulator Formal Required, G8 UI diagnostics, Android Security, Native, SQLite, Rust Security, Fast/Full, and GitHub Required checks: NOT_RUN for the new candidate SHA until the new Draft PR has been created and Actions have completed. Older source PR results are not substitutes.
- Independent review: pending review of the committed candidate.

## Review and environment findings

- Independent read-only security/code review: **ACCEPTED**, no actionable blocker and no #151/G9 product scope found. The reviewer traced synthetic consent isolation, read-only Calendar loopback, one-use/account/session-bound handoff, body-only Gmail data, inert HTML parsing, Cloud Egress lineage checks, and fail-closed harness guards. Reviewer caveat: `run_android_fake_google_process_death.sh` accepts any nonempty successful `pm path` output at its initial package probe; subsequent launch/PID checks still fail closed, so this was not considered a blocker.
- Local Android Gradle tasks (`:app:testDebugUnitTest :app:compileIsolatedAndroidTestKotlin`) did not start because this Cloud image has Java 21 runtime without `javac`; Gradle reported missing `JAVA_COMPILER`. This is an environment limitation, not a passing compile or test result.
- Rust Security scan on committed pre-fix candidate HEAD `6aab6f00972e26e733e4f5c5f0aff142eae28e0e` passed with cargo-audit 0.22.2, zero findings. The scanner refuses dirty tracked trees; it must be rerun against the final commit SHA or rely on formal Actions for that exact SHA.
