# Android Personal Beta release and supply-chain posture

Assessment date: 2026-10-26

Assessment base: functional integration commit `d74998f60c9062343ce9c62a326343e9a4d18e45`

Status: security review only; this does not authorize production signing, OAuth, distribution, or Beta GO.

## Release path review

The manual Personal Beta workflow is restricted to a `workflow_dispatch` on protected `main`. It checks out the exact workflow SHA with persisted credentials disabled, validates that source against `main`, requires configured checks on the exact SHA, and uses the protected `serea-personal-beta` Environment. The input `distribute` defaults to false.

Packaging fails closed unless Owner-managed PKCS12 signing inputs, certificate SHA-256, and a version code above the baseline are present. The Gradle task verifies the configured alias, private-key access, and certificate fingerprint before assembling `app.serea.android.beta`. The workflow independently checks APK package ID, version code, certificate, and SHA-256. It creates a build provenance attestation and deliberately does not upload or retain the signed APK as an Actions artifact.

Distribution is optional and uses GitHub OIDC Workload Identity Federation. Before upload, the workflow verifies that the configured Firebase App ID resolves to the Beta package and that the verified APK digest still matches. Firebase CLI output is suppressed to avoid exposing tester identities or request details. Cleanup removes temporary credentials, CLI data, release notes, and APK build output.

The debug app uses `.demo`; the instrumentation target uses `.isolated`; the beta variant uses `.beta`. The process-death receiver is wired only into disposable validation sources when the validation harness property is explicitly enabled. Release and Beta do not inherit the debug manifest by default.

## Dependency inventory and verification

- `android/gradle/wrapper/gradle-wrapper.properties` pins Gradle 8.14.6 and its distribution SHA-256.
- `android/app/gradle.lockfile` contains 317 locked Maven coordinates and Gradle configuration coverage after the Guava security update. The lockfile is committed and `lockAllConfigurations()` is enabled for app configurations.
- The Required Android dependency audit resolves Gradle dependencies, creates the license report, and runs the pinned OSV Scanner Action recursively over `android/`.
- Active Gradle dependency verification remains **not ready**. The published SEC-A2 review found that parts of the available checksum candidate lack independently verified publisher provenance and that plugin/task-specific coordinates require separate coverage. No `android/gradle/verification-metadata.xml` is active. This assessment does not promote repository-generated hashes into trust roots.
- `tools/generate_android_lockfile_sbom.py` produces a deterministic CycloneDX 1.5 component inventory from the checked-in app lockfile. Its `bom-ref` values are Maven PURLs and its metadata records the lockfile hash and optional source SHA. It does not claim artifact hashes, publisher authenticity, dependency edges, Gradle plugin/buildscript dependencies, Gradle distribution, Android SDK, or complete build provenance. A generated lockfile inventory is a useful release input, not a complete artifact SBOM.

## Validation and owner-controlled items

The existing Beta contract tests exercise the distribution helper with fake CLI/APK tooling and assert that raw Firebase output, tester identities, and credentials are not exposed. They also check main-only dispatch, environment protection, WIF, package binding, exact source and APK provenance, and absence of signed APK artifact uploads. The SBOM generator has a separate pure-Python focused test suite. No signing key, Firebase credential, real account, or production distribution was used in this review.

The following remain **NOT_RUN** and require Owner configuration or approval:

- Build and verify the actual Owner-signed Beta APK and its exact update certificate chain.
- Verify the protected Environment reviewer list, branch protection, required checks, version baseline, Firebase app, approved tester group, and WIF provider in the live repository configuration.
- Distribute any APK to testers.
- Produce an artifact-level SBOM from the actual Beta build and attest that SBOM with the APK digest.
- Activate Gradle dependency verification after independent publisher evidence and complete task-specific coordinate coverage are reviewed.

Formal Required thresholds are unchanged. This review does not make the R7 base CI result transferable to a later source SHA and does not establish Personal Beta readiness.
