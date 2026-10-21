# Serea Android Demo

This is a standalone Kotlin + Jetpack Compose demo. The chat screen is labeled
`DEMO · Mock chat · No cloud AI`; ordinary chat uses deterministic local replies
and encrypted on-device history. Cloud controls are disabled until provider
eligibility, data-egress conditions, and Owner activation are resolved. Mock
HTTP tests use loopback only. No Calendar access, PC pairing, or capability
execution is included.

## Build and checks

Requirements: JDK 17, Android SDK Platform 35, and Android Build Tools 35.0.0.
From this directory:

```sh
./gradlew assembleDebug
./gradlew assembleRelease
./gradlew testDebugUnitTest
./gradlew lintDebug
./gradlew assembleIsolatedAndroidTest
```

Instrumented tests target the `isolated` build type, whose application ID is
separate from the installed demo app. The Gradle wrapper pins Gradle 8.11.1 and
its published distribution SHA-256.
The debug APK is written to `app/build/outputs/apk/debug/`; the release
variant is an unsigned demo APK, not a production release.

The egress and credential ports restrict Cloud requests to the fixed public
probe. Software-backed Keystore keys cannot save Cloud credentials. ADR-0044
and ADR-0045 remain proposed and await Owner review.

## Required pull request check

The `Android Emulator Required Gate` runs the isolated instrumentation suite on
API 35 and API 26, in addition to Android build, JVM, lint, security, and
dependency checks. It compiles the Debug and Release APKs, plus the isolated
instrumentation APK, and requires exactly 102 source/JUnit test cases across 15
classes with zero failures, errors, or skips on each emulator. The process-death
harness verifies encrypted history, the Usage Ledger `OUTCOME_UNKNOWN` state,
Delete All, and a separate `.demo` history/preferences sentinel on both APIs.
Missing emulator access or missing JUnit output fails the gate.

Instrumentation uses the `isolated` application ID (`app.serea.android.isolated`).
Its runner refuses to start tests on a physical or unrecognized device, an
unsupported API, the normal `.demo` application ID, or a target with existing
app data. `connectedIsolatedAndroidTest` is the only generated instrumented
variant. The CI campaign additionally refuses any emulator
with an existing Serea package and never clears or reinstalls over existing
application data. Android documentation-only changes run documentation
validation without requiring an emulator.

To require this check in GitHub, the Owner must open **Settings → Branches** (or
the applicable ruleset), edit the protection rule for the target branch, enable
**Require status checks to pass before merging**, select the exact check
**Android Emulator Required Gate**, and save. This workflow does not change
branch protection or rulesets. If GitHub does not list the check yet, let the
workflow complete once on a pull request, then add it to the rule.
