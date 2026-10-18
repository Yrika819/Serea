# Serea Android Demo

This is an Android-only Compose prototype. The chat screen is labeled `DEMO ·
Mock chat · No cloud AI`. It uses deterministic local replies, has no network
permission, does not persist the transcript, and includes no provider API key,
OAuth flow, Calendar access, PC pairing, or capability execution.

## Build and checks

Requirements: JDK 17, Android SDK Platform 35, and Android Build Tools 35.0.0.
From this directory:

```sh
./gradlew assembleDebug
./gradlew assembleRelease
./gradlew testDebugUnitTest
./gradlew lintDebug
```

The Gradle wrapper pins Gradle 8.11.1 and its published distribution SHA-256.
The debug APK is written to `app/build/outputs/apk/debug/`; the release
variant is an unsigned demo APK, not a production release.

The egress and credential ports are future-facing contracts only. No network
provider is included. ADR-0044 is proposed and awaits Owner review.
