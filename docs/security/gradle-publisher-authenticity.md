# SEC-A2 Phase II — Publisher authenticity evidence (Lane B)

## Scope and conclusion

This report is based on PR #86 HEAD `e4dc8baea5f94a4314511dfb2afb70a7baa84285` and the Gradle 8.14.6 generated candidate at that revision. It addresses the publisher-authenticity gap, not Android Beta readiness.

A fresh run of `tools/audit_gradle_dependency_candidates.py --check-official-sidecars --verify-androidx-signatures` completed for the full candidate graph. It found 502 unique dependency coordinates, 492 with candidate metadata and 858 candidate artifact/metadata files. The app lockfile still has 10 coordinates absent from the generated candidate. The audit rechecked 437 repository SHA-256 sidecars successfully, found no mismatches or fetch errors, and verified 35 AndroidX binary signatures against Android's published full fingerprints.

**No coordinate is fully publisher-authenticated for Gradle verification.** The 35 verified signatures authenticate 35 AndroidX JAR/AAR files only. Corresponding POM or Gradle module metadata was not independently authenticated. The repository SHA-256 sidecars are served by the same repository as the bytes and are not independent publisher evidence. Gradle-generated hashes are candidate values only.

## Current counts

| Measure | Coordinates / files | Meaning |
| --- | ---: | --- |
| Total unique coordinates | 502 | Union of app lockfile and Gradle candidate |
| Candidate coordinates / candidate files | 492 / 858 | Captured by `help` task candidate at this baseline |
| All candidate files match repository sidecars | 229 coordinates | Same-repository evidence only, except for the separate binary signatures below |
| Of those, sidecar-only | 194 coordinates | No independently verified publisher signature in this audit |
| AndroidX signed binaries independently verified | 35 files across 35 coordinates | Signature matched one of Google's fingerprints; metadata files remain unauthenticated |
| Partial repository-sidecar match | 29 coordinates | At least one file matched and at least one did not |
| No repository-sidecar match | 234 coordinates | No candidate file matched a published sidecar |
| Missing candidate | 10 coordinates | No candidate artifact or generated hash to verify |
| **Incomplete/unverified** | **273 coordinates** | 29 partial + 234 no match + 10 missing; all remain unverified |
| Sidecar mismatches / fetch errors | 0 / 0 | The sidecar pass did not observe either condition |

The 273 unverified coordinates contain no independently verified signature in this audit. Their per-file candidate hashes, sidecar status and URLs, scopes, and lock configurations are recorded in [`evidence/gradle-publisher-authenticity-matrix.csv`](evidence/gradle-publisher-authenticity-matrix.csv). That matrix has 460 artifact-level rows, including one explicit row for each of the 10 coordinates with no generated candidate.

## Review order and exposed high-risk scope

The matrix prioritizes plugin and build-logic dependencies first. Of the 273 unverified coordinates, 217 touch plugin/buildscript resolution (including six direct plugin marker/implementation coordinates). Another 59 touch the Android Test Platform task-internal resolution path; these overlap the plugin/buildscript scope. Twenty-two are additionally flagged by dependency-name heuristics for crypto, authentication, or network review; this is a triage aid, not a complete security classification. The remaining 56 are other runtime or test dependencies.

The 10 missing candidate coordinates are the `_internal-unified-test-platform-android-test-plugin-host-retention` closure:

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

All 10 are reported as missing candidate evidence rather than accepted from the lockfile or from a presumed publisher. Lane A is separately investigating their task-specific resolution.

## Evidence and trust roots

- The Android Developers [Jetpack signature guidance](https://developer.android.com/jetpack/getting-started#verify-jetpack-dependencies) publishes four full Google signing-key fingerprints. The audit imported keys by fingerprint and accepted a signature only when the `VALIDSIG` fingerprint exactly matched one of those published values. The verified binary signatures establish those binary bytes' publisher provenance. They do not cover POMs or `.module` files.
- `.sha256` values were obtained from the artifact repositories (Google Maven, Maven Central, or Gradle Plugin Portal) and compared with the candidate file hashes. A match establishes consistency with that repository's sidecar, not independent publisher identity.
- The Gradle candidate was generated by Gradle. Its hashes were never treated as authenticity evidence. The audit did not add keys, hashes, trusted-artifact exceptions, or lenient verification to active Gradle configuration.
- No publisher-key fingerprint or independently published checksum from a separate trust channel was established for the 273 unverified coordinates in this lane. A downloadable `.asc` signature without an independently anchored publisher fingerprint would not be sufficient.

## Residual risk and activation status

Dependency verification remains **not ready for activation**. Enabling the strict file with the present candidate would promote unreviewed repository hashes to trust roots, omit 10 task-resolved coordinates, and leave publisher provenance incomplete. The candidate and shared `android/gradle/verification-metadata.xml` were not changed. No Security Gate exception was introduced.

Before activation, each artifact, POM, and `.module` file needs independent publisher evidence, or must remain unverified and block adoption. A second reviewer should independently retrieve the publisher key/checksum evidence and recompute verification results. Continue to distinguish same-repository sidecar agreement from signature or separately published checksum verification.

## Reproduction

From the repository root:

```sh
python3 tools/audit_gradle_dependency_candidates.py \
  --check-official-sidecars \
  --verify-androidx-signatures \
  --output /tmp/gradle-publisher-matrix-current.csv
```

The command used the pre-existing inactive candidate; it did not alter verification enforcement or the shared metadata path. Relevant guidance: [Gradle 8.14.6 dependency verification](https://docs.gradle.org/8.14.6/userguide/dependency_verification.html).
