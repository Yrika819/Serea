# SEC-A2 Lane C — Enforcement Preparation

## Decision

**Do not activate dependency verification.** The PR #86 baseline still has no
active `android/gradle/verification-metadata.xml`. The existing 8.14.6 generated
candidate was copied into a disposable Android project for the strict-mode
trial; the repository copy was not changed. The candidate rejects unregistered
and changed bytes only for coordinates it already records. It is not an
approved publisher trust inventory, and its signature verification is disabled.

## Negative enforcement tests

`tools/test_gradle_dependency_verification.sh` runs Gradle 8.14.6 against
temporary local Maven repositories and temporary verification files. The
fixtures do not add trust or dependencies to the Android project.

| Case | Expected | Result |
| --- | --- | --- |
| Correct registered SHA-256 | Resolve | PASS |
| Unregistered dependency | Reject | PASS |
| Wrong registered SHA-256 | Reject | PASS |
| Artifact changed after its approved digest was recorded | Reject | PASS |
| POM metadata changed after its approved digest was recorded | Reject | PASS |
| Artifact signed by a generated fixture key when metadata expects a different publisher key | Reject for signature/key evidence | PASS |

The unexpected-key test creates a fresh local RSA key and signature in a
temporary GPG home. Verification metadata pins a different fingerprint. The
assertion requires a signature/key-related rejection; a checksum-only failure
does not satisfy the test. The key is synthetic test data, not a Serea trust
root.

## Isolated candidate trial and task coverage

The trial used a temporary copy of `android/` with the generated candidate at
that copy's `gradle/verification-metadata.xml` path. No active metadata was
created in the worktree. The machine has Android SDK platform 35 and build
tools 35.0.0, but its configured OpenJDK 21 installation lacks `javac`, while
the Android Java compile task requires a Java compiler toolchain.

| Path | Command / evidence | Result |
| --- | --- | --- |
| Plugin resolution | `./gradlew --no-daemon --console=plain help` | PASS with the inactive generated candidate temporarily active |
| Runtime dependency graph | `:app:dependencies --configuration debugRuntimeClasspath` | PASS for graph reporting only; this task does not download/verify every artifact |
| Unit-test dependency graph | `:app:dependencies --configuration debugUnitTestRuntimeClasspath` | PASS for graph reporting only; this task does not download/verify every artifact |
| Instrumentation compile dependency graph | `:app:dependencies --configuration isolatedAndroidTestCompileClasspath` | PASS for graph reporting only; this task does not download/verify every artifact |
| Build | `assembleDebug assembleRelease assembleIsolated` | BLOCKED before compilation: `/usr/lib/jvm/java-21-openjdk-amd64` has no `JAVA_COMPILER` |
| Lint | `lintDebug lintIsolated` | BLOCKED by the same toolchain failure before Lint tasks execute |
| Unit | `testDebugUnitTest` | BLOCKED by the same toolchain failure before test tasks execute |
| Instrumentation compile | `assembleIsolatedAndroidTest` | BLOCKED by the same toolchain failure before Android test compilation |
| Instrumentation execution / Emulator | Not run | BLOCKED / not required for compilation; no emulator was started and no physical device was used |

The strict trial therefore does not establish successful Build, Lint, Unit, or
Instrumentation Compile. Re-run each task on a supported JDK 17 CI runner after
the evidence inventory and candidate metadata are complete. Run Android
instrumentation only on the designated disposable CI emulator gate.

## CI acceptance design

Before enforcement can be activated, a dedicated PR must satisfy every item:

1. Pin the wrapper distribution and record the Gradle, JDK, Android SDK, AGP,
   Kotlin, KSP and plugin versions used to refresh the candidate.
2. Resolve plugin/buildscript, runtime, unit-test, instrumentation compile and
   runtime, Android Test Platform detached/task-specific configurations, lint,
   and license-report paths. Fail CI if the task coverage report omits a
   required path or any coordinate remains unregistered.
3. Require independent publisher evidence for each artifact and each POM or
   `.module`. Record source URL, digest/signature result, full signing-key
   fingerprint, and the independent publication that establishes the trust
   root. A same-repository sidecar and a Gradle-generated digest remain
   candidate evidence only.
4. Run negative tests on every verification-metadata change. Require rejection
   of unregistered artifacts, altered bytes, metadata edits, bad checksums and
   unexpected signing keys; require a registered known-good control to pass.
5. Run Build, Lint, Unit, Instrumentation Compile, plugin resolution, and all
   configured dependency-resolution tasks with a fresh Gradle user home and
   the reviewed active metadata. Do not use `--write-verification-metadata`
   during the acceptance build. Do not suppress, ignore, or soften verification
   errors.
6. Require two approvals: one Android/Gradle reviewer and one reviewer who
   independently recomputes publisher evidence from the original sources.
   The second reviewer checks component coordinates, file names, artifact and
   metadata hashes, key fingerprints, task coverage, and metadata diff.
7. Retain the exact candidate and accepted metadata, inventory CSV, evidence
   URLs and retrieved evidence, detached signature/key data where licensing
   permits, recomputation transcript, task logs, test logs, wrapper/JDK/SDK
   versions, source/base SHAs, and CI run IDs as immutable CI artifacts.
   Link the retention artifact from the PR and keep its digest in the review
   record.
8. Make the metadata activation change a separate, reviewable commit after the
   evidence gate has passed. Keep all unknown coordinates fatal; no broad
   trusted-artifacts, ignored keys, or lenient mode.

## Dependency update procedure

For every dependency or plugin update, use a dedicated PR. Resolve the full
task matrix using a clean cache, collect updated artifact and metadata evidence,
and produce a candidate diff. The second reviewer independently fetches or
checks the original publisher evidence and recomputes each changed digest or
signature. Run the complete CI acceptance matrix and negative tests before
approval. A missing or inconclusive evidence row blocks the change from the
active verification file.

## Rollback

If an accepted update breaks verification, stop the affected release/build,
retain the failing logs and metadata diff, and revert the single activation or
dependency-update commit to the last reviewed metadata state. Re-run the
negative tests and CI acceptance against that exact rollback SHA before
resuming. Do not edit out a failing component, add a temporary trust wildcard,
or enable lenient verification. If the failure indicates a publisher or
repository compromise, preserve the evidence and keep releases blocked pending
security review.

## Readiness

**NOT READY.** Negative behavior is validated in local fixtures and plugin
resolution passed the isolated candidate trial. Four compile/test paths remain
unrun because the environment has no Java compiler toolchain. Publisher
evidence and complete task-specific coverage remain outstanding in Lanes A and
B. Do not treat this report, the candidate file, or Android Beta readiness as
authorization to activate dependency verification.
