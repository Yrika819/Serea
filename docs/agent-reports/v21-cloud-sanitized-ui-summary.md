# V21 Cloud-Safe Summary — V20 Pixel 7a UI Checkpoint

Date: 2026-10-31. This sanitized summary omits the ADB serial, local absolute paths, usernames, raw device logs and binary screenshots. The full local record remains `v20-evidence/pixel7a-ui-acceptance.md`; do not transfer that raw source without reviewing it for device identifiers.

## Provenance and outcome

- Target: `app.serea.android.isolated`, versionCode 1, versionName `0.1.0-demo`, Android Debug signer SHA-256 `caff85c2b3d11e95cdcaafe45806f4f836e79217ff3212c05757ada1ac450d10`.
- Read-only installed APK SHA-256: `7101030d92b5c7d52d17f4f5839a939006756fd9cf5217251189259b3f9bcfa0`. It was not rebuilt or reinstalled; this is not proof of current PR source bytes.
- **PASS (manual, focused):** cold launch, Chat display, one synthetic local Demo message/send, generic IME display and Back dismissal, Chat→Settings→Back, background/foreground, isolated process force-stop/cold relaunch restoring the synthetic conversation, edge-to-edge screenshot spot check.
- **NOT_RUN:** Japanese IME conversion/commit, Activity recreation, formal OS low-memory process death, physical JUnit. Current input method was not verified as Japanese-capable; shell Unicode injection failed at the Android input tooling layer. No system IME setting was changed.
- **JUnit:** NOT_RUN, 0 tests executed. Existing formal Isolated instrumentation Runner remains emulator-only and was not weakened or bypassed.
- No existing Demo, notifications, other apps, personal data, Room/Keystore data, model files or credentials were used. The only app-data mutation was a synthetic conversation in the isolated package, which remains there.

## Interpretation

The force-stop/relaunch result proves only that this focused synthetic conversation was restored after a controlled isolated-app restart. It does not prove ActivityScenario recreation or Android low-memory process-death behavior. Manual UI PASS is not JUnit PASS and does not establish current-source behavior because the installed APK was an existing artifact.