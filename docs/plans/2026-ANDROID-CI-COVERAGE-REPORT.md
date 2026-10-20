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

- Draft PR: pending
- Head branch: `codex/android-q4-ci-coverage-2026-10-20`
- Final head SHA / tree: pending
- Base: `codex/android-m0-08-abcd-integration-2026-10-20` at pinned SHA `239aaa181cdcfea756cf6c9ebb5eca9194caf7cf` before publication
- Final-head CI run IDs and results: pending; report `NOT_RUN` until observed
