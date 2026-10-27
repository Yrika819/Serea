# Android CI Coverage Report — Lane G

Date: 2026-10-20

## Scope and implementation

Source of truth was PR #58, open and draft, with head `239aaa181cdcfea756cf6c9ebb5eca9194caf7cf`. The new branch was created from that exact commit. PR #58's head branch is `codex/android-m0-08-abcd-integration-2026-10-20`, and the remote base branch resolved to the same SHA before work began.

Changed files:

- `.github/workflows/android.yml`: Android Unit now runs the complete `testDebugUnitTest` task without `--tests` class selectors. Gradle therefore discovers every test class in the source set, including Gmail, GoogleAuth, Calendar transport, or other domain tests added later. No required new class count is asserted.
- `.github/workflows/android-calendar-readonly.yml`: retained the existing base filter and added `codex/android-m0-08-abcd-integration-2026-10-20`, allowing Calendar-path changes in PRs based on PR #58's head branch to trigger this workflow.

No Android source, Gradle configuration, manifest, identity gate, or security workflow was changed. No new test class was added in this lane.

## Verification

| Check | Result | Evidence |
|---|---|---|
| Initial PR #58 state and pinned head | PASS | PR #58 was open/draft; live head exactly matched `239aaa181cdcfea756cf6c9ebb5eca9194caf7cf`. |
| Integration base branch | PASS | `git ls-remote` resolved `codex/android-m0-08-abcd-integration-2026-10-20` to `239aaa181cdcfea756cf6c9ebb5eca9194caf7cf`. |
| Initial checkout and identity | PASS | Clean `work` checkout at the supplied main handoff SHA; switched to the new lane branch at the pinned SHA. Repository-local Codex identity, `user.useConfigOnly=true`, `push.default=nothing`, and versioned hooks were configured. Commit preflight passed. Identity-scope checker passed for the bounded zero-commit initial range. |
| Workflow YAML parse | PASS | PyYAML 6.0.3 parsed both workflow files (Android: 10 jobs; Calendar: 1 job). |
| Unit discovery scope | PASS | Parsed workflow command is `./gradlew --no-daemon testDebugUnitTest`, with no class filter. Existing and future unit tests in the Android unit source set are discovered. |
| Calendar PR trigger | PASS | Parsed `pull_request.branches` includes the PR #58 head branch, and Calendar source/test path filters remain present. |
| Required instrumentation gates | PASS | Compared against the pinned workflow: API 26 and API 35 emulator definitions, minimum 59 tests, all seven exact class counts, Process Death summary, and Android Emulator Required Gate are unchanged. |
| `git diff --check` | PASS | No whitespace errors. |
| Local Gradle unit task | NOT_RUN | `./gradlew --no-daemon testDebugUnitTest` could not download Gradle 8.11.1 because the environment refused the connection; no Gradle test result was produced. |
| Android CI on this lane's final HEAD | NOT_RUN | Will be recorded after the branch is published and GitHub Actions starts. |
| Device, real Google OAuth/API, production AI provider | NOT_RUN | Not used or enabled by this CI-only lane. |

No test count is claimed for newly added Gmail/Auth/transport classes, since none existed in this lane. PR #58's description records the prior source-head results: Android workflow run `39404054887` PASS (59 API 26 and 59 API 35 instrumentation cases), and Calendar workflow run `39404054851` PASS (22 cases). These are baseline results for PR #58's old head, not evidence for this lane's final head.

## Security and integration

The change only broadens which fake/local Android unit tests run. It does not enable provider, Google OAuth, Gmail, or Calendar network access and does not touch credentials, Keystore, databases, Task CAS, or device data. Existing build, lint, dependency audit, instrumentation, Process Death, and required emulator gates remain enabled and unchanged.

Other lanes' Android source and Gradle changes will be picked up automatically by the complete unit task when integrated. The Calendar-specific workflow remains path-filtered and now recognizes PR #58's base branch. Its tests will also be included in the complete Android unit run for Android-changing PRs; the dedicated Calendar run remains available as a focused gate.

## Publication

- Draft PR: https://github.com/Yrika819/Serea/pull/62
- Head branch: `codex/android-q4-ci-coverage-2026-10-20`
- Initial implementation commit: `09cfa214b34a7718cb7a8f1b95793e6db277962b`; tree `c86890041bd344821a821af7d956a9756cfb1536`.
- Base: `codex/android-m0-08-abcd-integration-2026-10-20`, SHA `239aaa181cdcfea756cf6c9ebb5eca9194caf7cf` at PR creation.
- Runs associated with the initial implementation commit were queued when checked: Fast CI `39430976314`, Native platform validation `39430976081`, Android Calendar Read-only Tests `39430976095`, Git hooks Windows compatibility `39430976186`, Android `39430976113`, Cross-architecture SQLite portability `39430975712`, Full CI `39430975736`.
- These run IDs belong to `09cfa214...`, not any later report-update commit. Treat them as pending until their conclusions are observed, and do not treat PR #58's earlier runs as results for this branch. The PR's latest head SHA and associated checks are authoritative after report updates.
