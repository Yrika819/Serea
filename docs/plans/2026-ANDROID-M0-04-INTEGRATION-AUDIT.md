# Android AND-M0-04 integration audit

Date: 2026-10-20  
Candidate source: PR #49 head `083edcffcbaebbbada3828a0304a7a431b648f0d`  
Base observed: `main` at `e0acb0254820c381dd97828824bef86a1db5176c`

## Git state and candidate shape

The remote was refreshed before inspection. `main` and the seven listed open
Android PR heads matched the supplied SHAs. PR #49 is 88 commits ahead of
`main`, and its merge-base is exactly the observed `main` SHA. PR #40, #41,
#42, #44, #45, and #47 heads are all ancestors of #49. A candidate based on
PR #49 therefore retains their full commit history without rewriting it.

PR #41, #42, #44, #45, #47, and #49 are open Draft PRs. PR #43, #46, and #48
are closed, unmerged predecessor attempts. Their exact head commits are not
ancestors of #49; the later active PRs contain replacement work on their
respective parent lines. This candidate preserves those existing PRs and does
not retarget, close, or merge any of them.

## PR responsibilities and dependency graph

| PR | Responsibility | Relationship to candidate |
| --- | --- | --- |
| #40 | Event-scoped commit Author/Committer checks, fail-closed scope validation, and matching pre-push identity validation. | Directly based on main; its head is an ancestor of #49. |
| #41 | Standalone Android Demo chat, Android Gradle baseline, tests, and ADR-0044 proposal. | Stacked on #40; its head is an ancestor of #49. |
| #42 | Encrypted Room history, Android Keystore storage, constrained provider probe, Android CI gates, and ADR-0045 proposal. | Stacked on #41; its head is an ancestor of #49. |
| #43 (closed) | First lifecycle/multi-turn readiness attempt. | Not an ancestor; superseded by #44. |
| #44 | Lifecycle-safe multi-turn mock/provider path and integration evidence. | Rebased in scope on #42; its head is an ancestor of #49. |
| #45 | Provider adapter and accounting foundation with mock-only contract tests. | Stacked on #44; its head is an ancestor of #49. |
| #46 (closed) | First adaptive routing/device-readiness attempt. | Not an ancestor; superseded by #47. |
| #47 | Local adaptive model routing, persisted selection/budget policy, and device handoff evidence. | Stacked on #45; its head is an ancestor of #49. |
| #48 (closed) | First Pixel 7a action/deletion reliability attempt. | Not an ancestor; superseded by #49. |
| #49 | Safe overflow actions and deletion confirmation, Soft Limit validation, emulator/device handoff evidence, and APK provenance. | Final listed source head and integration candidate starting point. |

Required history is the complete main-to-#49 ancestry (88 commits), not a
cherry-pick list. No large rebase or history reconstruction is indicated.

## Changed-file and non-Android audit

The cumulative diff from main contains Android app/build/test/schema files,
the Android workflow, owner packets/ADRs, instrumentation verification and
process-death harnesses, and the following repository-wide files:

| Path | Audit |
| --- | --- |
| `.githooks/pre-push` | Reads Git's ref input once and passes the same bounded input to identity and preflight checks. |
| `.github/workflows/ci.yml`, `.github/workflows/full-ci.yml` | Check the event's explicit base/head range and check out the event head. Existing CI jobs remain enabled. |
| `tools/check_commit_identity.py`, `tests/commit_identity_tests.py` | Scope identity checks to introduced commits; test push, dispatch, stacked PR, unrelated refs, and fail-closed cases. |
| `tools/run_android_process_death_harness.sh` | Android-only emulator harness; invoked by the required emulator job. |
| `tools/verify_android_instrumentation.py`, `tests/android_instrumentation_results_tests.py` | Validate JUnit result files and required test classes; the CI threshold remains at least 34 tests on each API. |
| `crates/serea-model-router/src/lib.rs` | The only Rust diff is inside a test-only fixture helper. It reports why fixture insertion failed instead of silently returning `false`; production routing behavior is unchanged. |

The Rust change is diagnostic-only under `#[cfg(test)]`. PR #49's Full CI,
Native platform validation, and cross-architecture SQLite portability runs
all succeeded, covering the changed fixture in the relevant Rust test builds.
No Android authority or provider behavior is delegated to the PC Model Router.

## Android implementation and security boundaries

- Build configuration sets `applicationId=app.serea.android`, `minSdk=26`,
  `targetSdk=35`, and debug suffix `.demo`. PR validation builds debug and
  release variants; a runner-generated debug certificate is not a persistent
  distribution identity.
- Room schema exports versions 1–3 and registers additive migrations 1→2 and
  2→3. No destructive migration fallback is configured.
- History uses AES-GCM with record identity as associated data. Credential
  writes require reported hardware-backed Keystore protection; backups exclude
  app data. Keystore loss, rooted/live-compromised devices, and secure physical
  deletion remain outside the guarantees.
- Android egress denies UNKNOWN, PRIVATE, SECRET, CREDENTIAL, and PERSONAL
  content. Cloud mode defaults disabled; the UI's Cloud action remains disabled.
  Contract tests use loopback mock HTTP. No provider credential or live request
  was used by this audit.
- Android model routing is device-local and does not itself authorize egress.
  No production model is selected by this candidate.
- The debug-only process-death receiver is in the debug source set and is not
  exposed in the release build.
- `MainActivity` contains the PR #49 overflow reachability and deletion
  confirmation protections. Invalid Soft Limits fail validation before save.

## CI evidence at the PR #49 source SHA

GitHub Actions for the exact PR #49 SHA `083edcff…` reported success:

| Workflow | Run | Result and relevant coverage |
| --- | ---: | --- |
| Android | 39277831474 | Success: Build/Lint, Unit, Integration, Security, Dependency Audit, API 26 and API 35 instrumentation (34 each), Process Death on both APIs, and Android Emulator Required Gate. |
| Fast CI | 39277831642 | Success: workspace smoke, commit identity, preflight, hook, docs, format, metadata, and workspace checks. |
| Full CI | 39277832032 | Success: stable and Rust 1.85 validation; Linux, macOS Intel/arm64, and Windows P7 synthetic jobs. |
| Native platform validation | 39277831773 | Success: Linux, Windows x86_64/arm64, macOS Intel/arm64 at stable and/or MSRV as configured. |
| SQLite portability | 39277831976 | Success: databases produced and reopened across macOS Intel and arm64. |
| Git hooks Windows compatibility | 39277831638 | Run conclusion success; job-detail API did not return a response during this audit. |

These results validate PR #49's source SHA, not a future documentation commit
on the candidate branch. Candidate and signing PR heads must be checked again
after their final commits. Required checks or instrumentation thresholds must
not be reduced.

## Integration risks and Owner decisions

1. PR #49's Pixel 7a APK is `app.serea.android.demo` and was signed by that
   Actions run's debug key. It is not compatible by assumption with the PR #47
   APK or with a future owner-managed key. The #49 provenance packet correctly
   requires a fingerprint comparison and forbids uninstall/data clear when it
   differs.
2. The stable internal application ID must be chosen before distribution.
   Preserving the currently installed `.demo` app requires matching that ID
   as well as its signer; changing either blocks in-place update.
3. ADR-0044 and ADR-0045 remain **PROPOSED / NOT RATIFIED**. Their open
   provider, egress, retention, credential recovery, billing, and distribution
   decisions remain Owner gates. They must not be promoted to Accepted here.
4. The old PR #49 signing key is not present in this repository or environment.
   Until the Owner locates that key or chooses a migration, data-preserving
   upgrade from the installed Pixel 7a app cannot be claimed.
5. GitHub Environment reviewers, deployment branch restrictions, owner-held
   signing secrets, and the expected certificate fingerprint are not visible
   from this checkout. The protected signing path remains unavailable until
   the Owner configures and verifies those controls.

## Recommended integration order

Review and merge the single main-targeted integration candidate only after its
final-head CI succeeds and the Owner accepts the Android ADR/security decisions.
Do not merge or close the stacked source PRs as a substitute; keep them intact
until the Owner decides their disposition. Review the separate signing PR on
top of the integration candidate. Merge it only after the Environment,
branch restrictions, signing key, fingerprint, and Pixel 7a migration plan are
approved. No merge is performed by this audit.

## Follow-on roadmap

- AND-M0-05: implement the approved real-provider adapter boundary only after
  provider eligibility, model roster, egress, credential, and spending
  decisions are settled.
- AND-M0-06: Android task management.
- AND-M0-07: read-only Google Calendar after scopes and data-egress policy are
  approved. PC pairing, full GoalLatch integration, and production Gmail
  reading remain outside this milestone.
- Mid-December: feature freeze and integration test campaign.
- Late December: focused UI finishing and Beta readiness for the
  2026-12-31 target.
