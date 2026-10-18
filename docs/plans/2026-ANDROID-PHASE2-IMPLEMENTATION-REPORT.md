# Serea Android Phase 2 implementation report

Date: 2026-10-18 · State: Draft implementation evidence

## Scope and status

This report covers AND-04/05/06 on the stacked Android branch. `IMPLEMENTED`,
`TESTED_LOCAL`, `CI_PASS`, `DEVICE_TESTED`, `OWNER_PENDING`, `BLOCKED`, and
`NOT_RUN` are reported separately in the final PR description and run record.
The branch does not change Rust Core, the P7/P8 research series, formal
migrations, or frozen protocols.

## Candidate behavior

- Demo mode stores and answers ordinary chat locally. It makes no HTTP calls.
- Cloud Candidate is a separate settings action that sends a fixed public
  contract probe to Ollama's pinned HTTPS API host. Ordinary chat text and
  history remain ineligible for cloud submission pending the Owner's
  classification decision.
- The provider supports bounded NDJSON streaming, usage counts, cancellation,
  timeout/error mapping, disabled redirects, model allowlisting, and local
  response limits. It never retries automatically.
- Android Keystore AES-GCM protects the credential and encrypted history title
  and body. Room stores Android-only metadata and ciphertext. Backup is
  disabled and no destructive migration fallback is used.
- UI supports Demo conversations, history navigation/deletion, masked key
  configuration, provider probe, and pending/failure/cancellation states.

## Important limits

No live provider request or real API key was used. Provider model availability,
current terms, account conditions, exact retention, regional processing, and
revocation must be rechecked by the Owner before external data use. Local
soft-limits do not guarantee a provider bill ceiling. The current candidate
does not send ordinary user messages to a model, so production multi-turn
cloud chat, context truncation, and provider usage budgets are not enabled.
Room schema currently exports version 1 only; migration tests for a future
version do not yet exist. Software-backed Android Keystore is reported
accurately but acceptance policy is unresolved. Physical secure erasure is not
claimed.

## Validation evidence

Local verification on 2026-10-18 used JDK 17, Gradle Wrapper 8.11.1, SDK 35.
The combined Gradle command passed `assembleDebug`, `assembleRelease`,
`testDebugUnitTest`, `assembleDebugAndroidTest`, `lintDebug`, and the license
report task before the final CI submission. The latest local JVM JUnit XML has
22 tests: ViewModel 5/5, provider Mock HTTP 12/12, and egress security 5/5.
Storage, Keystore, Compose instrumentation (8 tests), and emulator execution
are `NOT_RUN` locally because the workspace has no emulator binary or system
image and `adb devices -l` reports no attached device. SDK platform-tools adb
is installed. The instrumentation APK compiled successfully.

Artifacts: `android/app/build/outputs/apk/debug/app-debug.apk` (installable
debug build), `android/app/build/outputs/apk/androidTest/debug/app-debug-androidTest.apk`,
and `android/app/build/outputs/apk/release/app-release-unsigned.apk` (release
variant compile output; unsigned). Room schema version 1 is exported. Docs
validation passed. Branch coverage is not measured; Android framework code and
new domain logic have JUnit failure-path evidence but no numeric coverage
claim. Emulator/Keystore and latest-HEAD Actions evidence must be added after
the GitHub run. Pixel 7a remains `NOT_RUN` until a physical device is attached
and exercised.

Manual install, Demo/history/delete, rotation, backup, offline, and
Owner-approved fixed-probe checks are in the
[Pixel 7a checklist](2026-ANDROID-PHASE2-PIXEL7A-TEST-CHECKLIST.md).

## Delivery

This is a stacked Draft PR based on the current PR #41 head. Do not merge,
retarget, accept ADR-0045, or enable ordinary cloud egress until the Owner
decisions in the companion packet are resolved.
