# Overnight Android recovery checkpoint — 2026-11-04

Status: **BLOCKED — cause of API 35 instrumentation termination unproven**. Personal Beta remains NO-GO. This is an investigation checkpoint, not a fix or acceptance.

## Baseline and evidence

- Starting main: `334da4bf8855e1b7fefc019a3b397aff506c9913`; PRs #210 and #211 merged.
- [Android run 41842357891](https://github.com/Yrika819/Serea/actions/runs/41842357891), same SHA, attempts 1 and 2 failed; API 26 job succeeded on both; [artifact 13332430219](https://github.com/Yrika819/Serea/actions/runs/41842357891/artifacts/13332430219) records attempt 1 producer provenance, formal 105/105 PASS, Process Death, Data Isolation and Test IME cleanup PASS. Attempt 2 reused that successful job. Three Linux required checks succeeded on this SHA. Android Emulator Required Gate failed.
- Attempt 1 [API 35 job](https://github.com/Yrika819/Serea/actions/runs/41842357891/job/126620480819), [artifact 13331130065](https://github.com/Yrika819/Serea/actions/runs/41842357891/artifacts/13331130065): focused stage `:app:checkIsolatedAndroidTestAarMetadata` failed dependency resolution. Maven Central returned HTTP 429 on a Kotlin Gradle plugin HEAD request, disabling MavenRepo; formal suite NOT_RUN. This is distinct from attempt 2.
- Attempt 2 [API 35 job](https://github.com/Yrika819/Serea/actions/runs/41842357891/job/126643650542), [artifact 13334278025](https://github.com/Yrika819/Serea/actions/runs/41842357891/artifacts/13334278025): focused `SereaAppShellTest#chatDraftSurvivesGmailAndCalendarDestinationChanges` started at 14:48:46.717 UTC; no finish or failure lifecycle event. JUnit reports one failure without detail; UTP reports `Expected 1 tests, received 0`; Gradle exited 1. Formal 105 NOT_RUN. Process Death, Data Isolation and Test IME cleanup report PASS. Device state after UTP cleanup shows no target/test PID or active instrumentation; this does not establish why it ended.
- Before the last recorded output, MainActivity was RESUMED with window focus and Test IME `onCreate` occurred. No Test IME `onStartInput`, active Composer InputConnection, Gmail/Calendar transition or completed test event is recorded; Google IME `onStartInput` is present earlier. The AndroidX Tracing `NoSuchMethodError` appeared earlier and execution continued; its ultimate impact is unproven. `androidx.tracing:tracing:1.0.0` is locked; do not upgrade solely on this evidence.
- The test's `UiAutomation.executeShellCommand` calls include shell pipelines for focus diagnostics; the artifact shows unfiltered `dumpsys` output in `FOREGROUND_*` logs. Large diagnostic output coincides with the last captured records, but neither causation nor an OOM, signal, crash, or kill is established. The live logcat is tag-filtered and cannot exclude system events. Initial host memory was available, not an execution-time OOM measurement.
- GitHub API redirects to Actions blob storage were forbidden from this workspace; the GitHub connector successfully retrieved the exact artifacts instead. No same-HEAD rerun was initiated: attempt 2 is not established as transient infrastructure failure. Local phase A was not possible: no emulator, system image, KVM, or attached device.

## Next diagnostic steps

1. Compare the two successful focused diagnostics below against the failed formal campaign. Repair the unsupported instrumentation-state observation and verify the same test through the formal campaign preparation and execution path; preserve bounded diagnostics and avoid collecting real user data.
2. Investigate shell pipeline execution and the large focus dump independently; validate any test-only correction with unchanged assertions and IME/InputConnection conditions. Do not infer that it fixes the termination without reproduction.
3. After an evidence-backed fix, repeat the exact test three times when practical, then run formal API 26 and 35 (105 each), all integration and required checks, and independent review. Do not relax the gate.
4. Audit Gradle cache hits and 429 mitigation without masking dependency failures; check same-name artifacts across attempts and producer/run/SHA linkage. Current aggregation checks job outcomes, exact suite inventory and evidence provenance and fails closed; past-attempt reuse is a separate design question.

## PR audit ledger

| PR | Current handling |
| --- | --- |
| #210, #211 | Merged; do not re-port. |
| #208 | Draft focused-workflow parallelization; separate from this formal gate failure. |
| #168 | Draft, head `272f68ecf7f8072b812b13e46064a108bab5ef2b`; its two changed files are Calendar tests. Main already has the back-navigation fix and its consent assertion; remaining event/zone diagnostics need selective comparison. Do not apply wholesale. |
| #189 | Activity qualification and launch checks are already in main; the old patch could undo later process-table checks. Do not re-port wholesale. |
| #191–#204 | Drafts; candidate sequence #193 → #195 → #196 → #198 → #199 → #202/#203 → #204; #191 is an earlier alternative, and #192/#194/#197/#200/#201 are documentation. #204 validation is pending. Inspect consent and routing boundaries before any selection; no bulk rebase, close or merge. |

Owner approval is required for main merge, removing an existing PR's Draft status, changing branch protections or a Beta GO decision. No such action is authorized by this checkpoint.

## Commit identity correction

This clean draft supersedes the document changes in [PR #212](https://github.com/Yrika819/Serea/pull/212); that PR and its three commits remain intact for audit. Its Fast/Full identity guard rejected the author and committer email on commits `1bfc000fc6e98e01886012a572598fb782a92b50`, `f0625cb25818ccc14e18210d0ea115bc516585e6`, and `f2c66bc8b576d740882a979a02409fbccf074651`: they used the workspace's configured email, outside the guard's GitHub noreply allowlist. This branch retains the document content and uses the repository owner's established GitHub noreply identity for the replacement commit. The identity policy is unchanged.

## API 35 focused reproduction checkpoint

The original termination remains **unexplained and not reproduced** in two independent single-test diagnostic runs; neither run is a formal campaign result or a root-cause fix. Both used Android tree `7f0e2ad58212619ea0df5a0d42a300758c67c406`, identical to main `334da4bf8855e1b7fefc019a3b397aff506c9913`, but different diagnostic workflow HEADs.

- [Baseline run 41863115737, attempt 1](https://github.com/Yrika819/Serea/actions/runs/41863115737), HEAD `96af977a6956acc7e0441703eef095fe2755c444`, [artifact 13340992524](https://github.com/Yrika819/Serea/actions/runs/41863115737/artifacts/13340992524): focused JUnit 1/1 PASS, zero errors/failures/skips, Gradle exit 0.
- [Diagnostic run 41863410639, attempt 1](https://github.com/Yrika819/Serea/actions/runs/41863410639), HEAD `760f6b3c7a3f44df549c95acea5196057a8eaec1`, event merge-result `aac79d3977cd4835ae9fb860a0fd2e8f54fa680d`, [artifact 13341787468](https://github.com/Yrika819/Serea/actions/runs/41863410639/artifacts/13341787468): focused JUnit 1/1 PASS, zero errors/failures/skips, Gradle exit 0.
- Both successful logcats contain the same `AndroidXTracer` `NoSuchMethodError` seen in the failed run. This is evidence against the exception alone being sufficient to cause the termination; it does not establish that tracing can never contribute. The diagnostic reports a health observation gap: `dumpsys activity instrumentation` is unsupported on this Android image. Instrumentation monitoring must be repaired and independently reviewed.
- [PR #213](https://github.com/Yrika819/Serea/pull/213) at prior HEAD `534a1a56fe8a2fc1681575eb8ff52237606e1883` had required checks 4/4 SUCCESS. Its Android gate was documentation-only and does not establish Android instrumentation health. [PR #214](https://github.com/Yrika819/Serea/pull/214) at `760f6b3c7a3f44df549c95acea5196057a8eaec1` had three Linux required checks PASS and [Android Required Gate FAIL](https://github.com/Yrika819/Serea/actions/runs/41863410541/job/126709278266); its API 35 formal job failed. Do not infer a formal pass from the separate single-test runs.
