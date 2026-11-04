# Overnight Android recovery checkpoint — 2026-11-04

Status: **BLOCKED — cause of API 35 instrumentation termination unproven**. Personal Beta remains NO-GO. This is an investigation checkpoint, not a fix or acceptance.

## Baseline and evidence

- Starting main: `334da4bf8855e1b7fefc019a3b397aff506c9913`; PRs #210 and #211 merged.
- [Android run 41842357891](https://github.com/Yrika819/Serea/actions/runs/41842357891), same SHA, attempts 1 and 2 failed; API 26 job succeeded on both. Three Linux required checks succeeded on this SHA. Android Emulator Required Gate failed.
- Attempt 1 [API 35 job](https://github.com/Yrika819/Serea/actions/runs/41842357891/job/126620480819), [artifact 13331130065](https://github.com/Yrika819/Serea/actions/runs/41842357891/artifacts/13331130065): focused stage `:app:checkIsolatedAndroidTestAarMetadata` failed dependency resolution. Maven Central returned HTTP 429 on a Kotlin Gradle plugin HEAD request, disabling MavenRepo; formal suite NOT_RUN. This is distinct from attempt 2.
- Attempt 2 [API 35 job](https://github.com/Yrika819/Serea/actions/runs/41842357891/job/126643650542), [artifact 13334278025](https://github.com/Yrika819/Serea/actions/runs/41842357891/artifacts/13334278025): focused `SereaAppShellTest#chatDraftSurvivesGmailAndCalendarDestinationChanges` started at 14:48:46.717 UTC; no finish or failure lifecycle event. JUnit reports one failure without detail; UTP reports `Expected 1 tests, received 0`; Gradle exited 1. Formal 105 NOT_RUN. Process Death, Data Isolation and Test IME cleanup report PASS. Device state after UTP cleanup shows no target/test PID or active instrumentation; this does not establish why it ended.
- Before the last recorded output, MainActivity was RESUMED with window focus and Test IME `onCreate` occurred. No `onStartInput`, InputConnection, Gmail/Calendar transition or completed test event is recorded. The AndroidX Tracing `NoSuchMethodError` appeared earlier and execution continued; its ultimate impact is unproven. `androidx.tracing:tracing:1.0.0` is locked; do not upgrade solely on this evidence.
- The test's `UiAutomation.executeShellCommand` calls include shell pipelines for focus diagnostics; the artifact shows unfiltered `dumpsys` output in `FOREGROUND_*` logs. Large diagnostic output coincides with the last captured records, but neither causation nor an OOM, signal, crash, or kill is established. The live logcat is tag-filtered and cannot exclude system events. Initial host memory was available, not an execution-time OOM measurement.
- GitHub API redirects to Actions blob storage were forbidden from this workspace; the GitHub connector successfully retrieved the exact artifacts instead. No same-HEAD rerun was initiated: attempt 2 is not established as transient infrastructure failure. Local phase A was not possible: no emulator, system image, KVM, or attached device.

## Next diagnostic steps

1. Run the exact focused test alone on a disposable API 35 emulator at the same source SHA, recording exit status, JUnit, UTP, test lifecycle, system/crash/event log buffers and process-exit evidence. Preserve bounded diagnostics and avoid collecting real user data. Compare with the existing failure before changing behavior.
2. Investigate shell pipeline execution and the large focus dump independently; validate any test-only correction with unchanged assertions and IME/InputConnection conditions. Do not infer that it fixes the termination without reproduction.
3. After an evidence-backed fix, repeat the exact test three times when practical, then run formal API 26 and 35 (105 each), all integration and required checks, and independent review. Do not relax the gate.
4. Audit Gradle cache hits and 429 mitigation without masking dependency failures; check same-name artifacts across attempts and producer/run/SHA linkage. Current aggregation checks job outcomes, exact suite inventory and evidence provenance and fails closed; past-attempt reuse is a separate design question.

## PR audit ledger

| PR | Current handling |
| --- | --- |
| #210, #211 | Merged; do not re-port. |
| #208 | Draft focused-workflow parallelization; separate from this formal gate failure. |
| #168 | Draft, based on `codex/serea-g9rc-ui-accepted`, head `272f68ecf7f8072b812b13e46064a108bab5ef2b`; compare Consent/authorization changes against current main before selecting any residual. |
| #191–#204, #189 | Drafts remain candidates for dependency and duplicate analysis after this blocker; no bulk rebase, close or merge. |

Owner approval is required for main merge, removing an existing PR's Draft status, changing branch protections or a Beta GO decision. No such action is authorized by this checkpoint.
