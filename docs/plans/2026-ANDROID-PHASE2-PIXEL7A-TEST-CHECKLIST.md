# Pixel 7a Android Phase 2 acceptance checklist

State: `NOT_RUN` in Codex Cloud. The checklist is for a device owner with a
Pixel 7a and Android SDK platform-tools. It does not require root access,
changing OS security settings, or exporting personal data.

## Install and local Demo

1. Download `app-debug.apk` from the Draft PR's `serea-android-phase2-build`
   artifact. This is a debug-signed, installable APK; the release variant from
   this branch is unsigned.
2. Connect the Pixel 7a with USB debugging already enabled and confirm the
   device using `adb devices -l`. Install with `adb install -r app-debug.apk`.
3. Open Serea and verify the `DEMO · Mock chat · No cloud AI` label and
   Japanese local-only notice. Send two messages and confirm Demo replies.
4. Open History, select the conversation, then open Settings and return. The
   conversation should remain available. Force-stop and reopen the app; verify
   the same messages return.
5. Rotate the device while viewing a long response. Confirm the current
   conversation and text remain readable and controls are still reachable with
   the keyboard open.
6. Open a new conversation and verify it has no messages from the previous
   conversation. Delete the current conversation and confirm it no longer
   appears. Repeat with Delete all history and confirm the empty-history state.
7. In Android App info, verify backup is unavailable/disabled for this app and
   confirm it launches in airplane mode for local history browsing.

## Credential and cloud probe

Cloud access remains a candidate and sends only the fixed public contract
probe. Do not enter a real key or enable the probe until the Owner has approved
the provider/account terms, model, region, billing controls, and probe request.
If approval is recorded and a test key is intentionally provided by the owner:

1. Enter the key in Settings and save it. Verify it remains masked, the UI
   reports encrypted storage and the actual Keystore security profile, and the
   key never appears in chat history.
2. Close and reopen Settings; verify the key is reported as configured without
   being revealed. Delete it and verify the UI reports no configured key.
3. With the candidate disabled or no key configured, verify the probe button
   is unavailable. With approved test conditions, enable the candidate and
   run one fixed PUBLIC probe. Verify the model/usage result or a safe mapped
   error. Do not send a user message through this probe.
4. While the probe streams, cancel it. Verify the UI says provider-side
   completion may still occur and does not automatically retry.
5. Turn on airplane mode and repeat once. Verify a bounded network error is
   shown, with no error body or credential displayed. Restore normal network.

## Evidence to record

Record Android OS build, app version, PR commit SHA, installation result,
Demo/history/delete results, API-key storage profile, and any safe error
category. Do not include the API key, conversation text, screenshots exposing
the key, or unrelated personal data in the report. Mark each item PASS, FAIL,
or NOT_RUN; emulator results do not replace this device acceptance.
