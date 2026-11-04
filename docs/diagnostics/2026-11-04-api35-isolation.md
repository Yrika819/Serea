# API 35 single-test isolation

Status: **NOT REPRODUCED, cause undetermined**. The required gate remains failed; no root fix is claimed. The baseline Android tree is `334da4bf8855e1b7fefc019a3b397aff506c9913`.

## Evidence and comparison

| Run / artifact | Result | Scope and limitations |
| --- | --- | --- |
| [41842357891 attempt 2](https://github.com/Yrika819/Serea/actions/runs/41842357891), [13334278025](https://github.com/Yrika819/Serea/actions/runs/41842357891/artifacts/13334278025) | focused test began; no finish; UTP expected 1 received 0 | Formal campaign initial preparation; 105 NOT_RUN. Live filtered log ends in focus output. No cause established. |
| [41863115737](https://github.com/Yrika819/Serea/actions/runs/41863115737), [13340992524](https://github.com/Yrika819/Serea/actions/runs/41863115737/artifacts/13340992524) | single test 1/1 PASS, Gradle 0, UTP complete | Android tree matches baseline; no demo sentinel. Original instrumentation snapshot returned Unknown command; cgroup OOM evidence missing. |
| [41863410639](https://github.com/Yrika819/Serea/actions/runs/41863410639), [13341787468](https://github.com/Yrika819/Serea/actions/runs/41863410639/artifacts/13341787468) | single test 1/1 PASS, Gradle 0, UTP complete | Same Android tree; no demo sentinel. Same instrumentation and cgroup evidence gaps. |

Both successes contain AndroidX Tracing `NoSuchMethodError` before normal FINISH and RUN_FINISH. This refutes a claim that the exception alone always terminates the test; indirect effects remain unproven. All three use the same test runner, class, listener, UTP timeout, Pixel 7, API 35, google_apis x86_64 and emulator 37.2.13.0. The failed run and one successful run use the same runner image (20261101.360.1); image version alone does not explain the failure. All had the same Gradle cache key hit. Successes use fingerprint `google/sdk_gphone64_x86_64/emu64xa:15/AE3A.240806.043/12960925`.

The failed run stopped at roughly 58 KB of focus output. Both successful runs reached comparable initial output and then produced 553–560 KB of focus output, making deterministic failure from output volume alone unlikely; timing, stream or load effects remain possible. Both reached Test IME input sessions; the failed evidence ends before the Test IME onStartInput. Successful host monitors show minimum available memory of roughly 10.4 GiB; failure lacks runtime host memory and cgroup evidence, so OOM cannot be excluded. Successful end-of-test ActivityManager process termination followed RUN_FINISH and is consistent with normal cleanup, not proof about the failed run.

Formal preparation installs the demo APK and seeds a sentinel before isolated target installation and token seeding; the two standalone runs do not. APK hashes also differ across builds, so matching source trees do not imply byte-identical APKs. A separate focused experiment through the original Formal preparation path is required.

PR #214's [Android run 41863410541](https://github.com/Yrika819/Serea/actions/runs/41863410541) API 35 job failed to start its emulator: userdata needed 7372.80 MB and 7360.96 MB was available. The subsequent device connection errors are distinct from the original in-test termination.

## Diagnostic design and limitations

The non-required workflow uses a disposable no-real-account emulator; it does not change the Required Gate. It records bounded main/system/crash/events logcat, UTP/JUnit and Gradle output, concurrent device and host snapshots, process IDs, exit-info, ADB and reader state, memory/cgroup counters and time boundaries. Raw emulator logs are retained for 14 days. Each snapshot records command exit and output size; the first 16,000 bytes of selected output are retained. Truncation, sampling intervals, unsupported commands, post-UTP uninstall and missing cgroup data constrain interpretation. Test result and diagnostic evidence completeness are separate; incomplete required evidence fails the diagnostic workflow.

Android 15 does not implement `dumpsys activity instrumentation` as an activity subcommand and can return exit 0 with an error message. The replacement queries `dumpsys activity processes`: Android 15 AOSP's [ProcessList.dumpProcessesLSP](https://android.googlesource.com/platform/frameworks/base/+/refs/heads/android15-release/services/core/java/com/android/server/am/ProcessList.java) calls `dumpActiveInstruments`; [ActivityManagerService](https://android.googlesource.com/platform/frameworks/base/+/refs/heads/android15-release/services/core/java/com/android/server/am/ActivityManagerService.java) emits `Active instrumentation:` only when active. A missing active snapshot is incomplete, not proof of absence. The replacement itself requires an emulator run before its effectiveness is established.

Earlier dispatch 41862998038 was cancelled while queued because review found its package-absence check would reject normal Android 15 behavior; it is not a test result. After baseline, change one hypothesis at a time and preserve assertions and IME/InputConnection checks. Formal API 26/35 and all required checks remain necessary for any evidence-backed fix.
