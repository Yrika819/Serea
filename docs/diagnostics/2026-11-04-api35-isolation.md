# API 35 single-test isolation

This non-required workflow preserves the Android tree at main baseline `334da4bf8855e1b7fefc019a3b397aff506c9913` and runs only `SereaAppShellTest#chatDraftSurvivesGmailAndCalendarDestinationChanges` in a fresh GitHub-hosted emulator. The emulator has no real accounts or real user data. The existing Android Required Gate is unchanged. A pass here is not formal acceptance.

Evidence includes bounded main/system/crash/events logcat, UTP/JUnit and Gradle output, concurrent device and host snapshots, instrumentation state, package exit-info, process IDs, logcat-reader and ADB state, memory/cgroup counters and time boundaries. Raw emulator logs are retained for 14 days; do not use the workflow with real user data. Package exit-info can be unavailable after UTP uninstalls the app; absence is not proof of normal exit. The monitor's sampling interval and any failed commands limit what it can establish. Captures stop at 32 MiB and truncation or a dead reader fails the diagnostic.

First baseline dispatch: [run 41863115737](https://github.com/Yrika819/Serea/actions/runs/41863115737), head `96af977a6956acc7e0441703eef095fe2755c444`, uses identical Android sources and a temporary dispatch workflow. It was queued at this checkpoint. Earlier run 41862998038 was cancelled while queued because review found its package-absence check would reject normal Android 15 behavior; it is not a test result. Do not treat queued runs as evidence of success or reproduction.

After baseline, compare up to three individually run trials and only then test one hypothesis at a time. Preserve test assertions and the IME/InputConnection boundary. A root-cause fix requires a separate PR and formal API 26/35, isolation and required-check validation.
