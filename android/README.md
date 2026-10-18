# Serea Android Demo

This is a standalone Kotlin + Jetpack Compose demo. The chat screen is labeled
`DEMO · Mock chat · No cloud AI`; ordinary chat uses deterministic local replies
and encrypted on-device history. A separately labeled Cloud Candidate can send
only a fixed public contract probe through its egress gate. No Calendar access,
PC pairing, or capability execution is included.

## Build and checks

Requirements: JDK 17, Android SDK Platform 35, and Android Build Tools 35.0.0.
From this directory:

```sh
./gradlew assembleDebug
./gradlew assembleRelease
./gradlew testDebugUnitTest
./gradlew lintDebug
./gradlew assembleDebugAndroidTest
```

The Gradle wrapper pins Gradle 8.11.1 and its published distribution SHA-256.
The debug APK is written to `app/build/outputs/apk/debug/`; the release
variant is an unsigned demo APK, not a production release.

The egress and credential ports restrict Cloud requests to the fixed public
probe. Software-backed Keystore keys cannot save Cloud credentials. ADR-0044
and ADR-0045 remain proposed and await Owner review.

## Required pull request check

The `Android Emulator Required Gate` workflow job is emitted for every pull
request. Android implementation, build, test, or security configuration changes
require Android build, unit, mock HTTP, security, dependency audit, and API 35
emulator jobs to succeed. The emulator job must run
`connectedDebugAndroidTest` and produce JUnit XML for at least thirteen tests with
zero failures, errors, or skipped tests. Missing emulator access or missing
JUnit output fails the gate. Android documentation-only changes run
documentation validation without requiring an emulator.

To require this check in GitHub, the Owner must open **Settings → Branches** (or
the applicable ruleset), edit the protection rule for the target branch, enable
**Require status checks to pass before merging**, select the exact check
**Android Emulator Required Gate**, and save. This workflow does not change
branch protection or rulesets. If GitHub does not list the check yet, let the
workflow complete once on a pull request, then add it to the rule.
