# Android Phase 1 implementation report — 2026-10-18

## Scope and ancestry

Implemented on `codex/android-first-phase1-2026-10-18`, based directly on PR
#40 head `4dede3eb033cd176e666c96407d7836066ff9a93` (parent:
`e0acb0254820c381dd97828824bef86a1db5176c`). PR #40 remained OPEN/Draft at
the start of work. PR #39 was also rechecked as OPEN/Draft at
`dbdf0c7ee7016e4a729117337a9839e0bb99f80f`.

## Audit findings

- `serea.device/2` is a frozen PC-paired Android client protocol. This app is a
  standalone demo and does not implement or claim `device/2` compatibility.
- Current PC model dispatch is host-owned. Android does not reuse or expose
  `PreparedModelCallV1::from_host` or `ModelRouterProcessV1::execute`.
- Rust workspace has 11 crates and MSRV 1.85. The Android build is isolated
  under `android/`; no Cargo manifests, canonical migrations 0001–0008, accepted
  ADRs, or frozen protocol files are changed.
- ADR-0044 is a proposal only. It does not change `serea.device/2` or ratify
  provider, identity, history persistence, or credential choices.

## Implementation

The single-module Kotlin/Compose/Material 3 app has a chat screen, send/input,
user and assistant messages, loading/error states, system light/dark theme,
keyboard send handling, live accessibility announcements, and ViewModel state
that survives configuration changes. The visible `DEMO · Mock chat · No cloud
AI` label appears above the conversation. The deterministic Mock adapter has
no HTTP client, no `INTERNET` permission, and explicitly says no cloud AI was
contacted. The UI warns users not to enter private information. Demo history is
in-memory and is not yet durable.

`ModelChatPort` defines a timeout-bearing future chat seam. Its current
implementation is only `MockModelChatPort`. `CredentialPort` exposes an opaque
handle contract; no credential adapter exists. `ChatEgressGate` allows PUBLIC,
requires a preparation marker for PERSONAL, rejects PRIVATE/SECRET/CREDENTIAL,
and rejects UNKNOWN. Unit tests exercise the deny cases. The preparation marker
is only a Phase 1 contract placeholder; there is no PERSONAL-source preparation
pipeline and no cloud provider capable of dispatching it. The egress wrapper
replaces PERSONAL raw text with the prepared/redacted payload before delegation.

## Verification

| Check | Result |
| --- | --- |
| `./gradlew assembleDebug` | PASS; `app-debug.apk` produced |
| `./gradlew assembleRelease` | PASS; unsigned `app-release-unsigned.apk` produced |
| `./gradlew testDebugUnitTest` | PASS; 8 tests, 0 failures/errors |
| `./gradlew lintDebug` | PASS; 5 advisory newer-dependency warnings remain |
| `./gradlew assembleDebugAndroidTest` | PASS; Compose smoke-test APK compiled |
| Compose instrumentation execution | NOT_RUN; no emulator binary or `/dev/kvm` available |
| Pixel 7a physical device | DEVICE_NOT_AVAILABLE / NOT_RUN; `adb devices -l` returned no devices |
| Android CI at final PR head | pending Draft PR creation and CI completion |
| Rust validation | not run; no Rust source/workspace changes |

Commands ran through the Gradle 8.11.1 wrapper with a verified distribution
SHA-256. JDK 17, Android SDK 35, and Build Tools 35.0.0 were installed in
temporary tooling directories after official package checksums were verified.
No build result is evidence of physical-device behavior. The implementation
does not contact any external service or contain API keys/OAuth tokens.

## Remaining work and gates

- History repository, durable storage/encryption, retention and deletion UI are
  not implemented. Rotation retains in-memory ViewModel state; process death
  clears the demo transcript.
- No real AI, provider selection, OAuth, Calendar/Gmail integration, reminders,
  or local task system is implemented.
- AND-04 needs the Owner choices in
  [the decision list](2026-ANDROID-PHASE2-OWNER-DECISIONS.md), followed by an
  independently reviewed provider/credential/egress implementation.
- Target remains 2026-10-25 for the bounded Mock prototype. The code slice is
  in place and local build/test/lint gates pass. Owner review and new PR CI are
  pending; this does not claim Beta or production-AI completion.
- 2026-11-08 real-cloud-AI goal is not yet achieved. It depends on provider or
  backend selection, data disclosure/retention terms, auth and credential
  custody, egress preparation rules, spending limits, and a verified AND-04
  implementation.
