# SEC-A2 Lane A — Complete Dependency Graph

## Scope and baseline

This lane starts at PR #86 HEAD `e4dc8baea5f94a4314511dfb2afb70a7baa84285` in branch `phase2/lane-a`. The configured Android build requests Gradle 8.14.6, AGP 8.12.3, Kotlin 2.4.21, and KSP 2.3.12. The app lockfile contains 324 unique coordinates. The existing combined inventory contains 502 unique coordinates: 492 in the inactive Gradle-generated candidate and 10 locked coordinates without hashes in that generation.

The ten missing coordinates all map to `_internal-unified-test-platform-android-test-plugin-host-retention` in `android/app/gradle.lockfile`:

- `com.android.tools.emulator:proto:31.9.2`
- `com.android.tools.utp:android-test-plugin-host-retention-proto:31.9.2`
- `com.android.tools.utp:android-test-plugin-host-retention:31.9.2`
- `com.android.tools.utp:utp-common:31.9.2`
- `com.android.tools:annotations:31.9.2`
- `com.android.tools:common:31.9.2`
- `org.jetbrains.kotlin:kotlin-stdlib-common:2.1.0`
- `org.jetbrains.kotlin:kotlin-stdlib-jdk7:2.1.0`
- `org.jetbrains.kotlin:kotlin-stdlib-jdk8:2.1.0`
- `org.jetbrains.kotlin:kotlin-stdlib:2.1.0`

## Cause and resolution status

The existing candidate was generated with `--write-verification-metadata sha256 help`. That task resolves plugin and ordinary project configuration but does not instantiate the UTP host-retention configuration recorded in the lockfile. A direct `:app:dependencies --configuration _internal-unified-test-platform-android-test-plugin-host-retention` attempt confirms the configuration is absent from the ordinary configuration-time graph. The ten entries are therefore task-specific; the `help` inventory did not cover the task path that creates them.

Gradle 8.14.6 successfully ran `help` and produced an updated transient candidate. I separately fetched the locked coordinates' available artifacts and metadata from the configured official repository endpoints, recording the exact SHA-256 and HTTP result in [gradle-lane-a-utp-observed-hashes.csv](evidence/gradle-lane-a-utp-observed-hashes.csv). The retrieval found 9 JARs, 10 POMs, and 3 Gradle Module Metadata files (22 files total); Kotlin `kotlin-stdlib-common-2.1.0.jar` and seven `.module` URLs returned 404. A 404 is recorded as unavailable, not as an absent dependency. The fetched hashes are observations only: publisher authenticity review is pending, and they are not represented as Gradle-generated or trusted hashes.

The inactive [expanded candidate](evidence/gradle-8.14.6-lane-a-expanded-candidate.xml) carries these 22 observations with an explicit “authenticity review pending” origin. It combines the PR #86 candidate with the missing coordinates' retrieved file hashes, reaching 502 coordinate entries and 880 listed hashes. The original PR #86 candidate remains unchanged. A separate [help-only candidate](evidence/gradle-8.14.6-lane-a-help-candidate.xml) preserves the exact output of the successful task run. No `android/gradle/verification-metadata.xml` activation file remains in this worktree.

## Task coverage

The detailed command matrix is [gradle-lane-a-task-coverage.csv](evidence/gradle-lane-a-task-coverage.csv).

| Area | Result |
| --- | --- |
| Gradle version / plugin resolution via `help` | PASS, Gradle 8.14.6 |
| Task discovery (`tasks --all`) | FAILED before listing: toolchain lacks `JAVA_COMPILER` |
| Direct UTP configuration report | FAILED: configuration is absent from normal project configuration |
| Build, Unit, Lint, instrumentation compile | BLOCKED: only JRE 21 is installed; no compiler is available for the configured Java 17 toolchain |
| Instrumentation execution | BLOCKED: `adb devices -l` returned no attached device/emulator; execution not attempted |
| UTP host-retention task path | BLOCKED pending a usable JDK and execution of the Android instrumentation task graph |

Exact compiler failure: `Toolchain installation '/usr/lib/jvm/java-21-openjdk-amd64' does not provide the required capabilities: [JAVA_COMPILER]`.

## Remaining Lane A work

The 10 locked coordinates now have fetched file hash evidence, but Gradle has not resolved them in a task execution on this environment. Re-run the instrumented Android task path on an isolated machine with a complete JDK 17 toolchain and no device execution unless a disposable emulator is explicitly available. Then compare the task-generated file set with the observed-hash supplemental CSV and reconcile any artifact/metadata differences. Keep the candidate inactive until Lane B has independently verified publisher evidence and the review gates are met.
