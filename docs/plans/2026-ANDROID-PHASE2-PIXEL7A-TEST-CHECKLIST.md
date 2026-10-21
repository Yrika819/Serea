# Pixel 7a Android beta acceptance checklist

State: **NOT_RUN** in Codex Cloud. Target: a dedicated disposable Pixel 7a
test profile on Android 17 Beta. Never run this checklist on a personal Pixel
7a or a profile containing personal data. Use synthetic content only. Do not
add a Provider account or key; live Provider access remains disabled.

Before continuing, prove that this is the disposable test profile and that
`app.serea.android.demo`, `app.serea.android.isolated`, and
`app.serea.android.isolated.test` are absent. If any package is installed or
retained, the profile may contain personal data, or the profile cannot be
verified, stop without installing or changing anything. Do not use
`adb install -r`, uninstall an app, or clear app/device data to make the check
pass. Use a newly provisioned disposable profile instead.

Run this read-only package check on the verified device; it must print no
matches:

```sh
adb shell pm list packages -u --user all | grep -E 'package:app\.serea\.android\.(demo|isolated|isolated\.test)$'
```

If `adb`, the profile identity, or package inventory cannot be verified, stop.

## Build identity and install

1. Download `app-debug.apk` from the Draft PR's `serea-android-phase2-build`
   Actions artifact for the exact reviewed commit SHA. Confirm its SHA-256 with
   `sha256sum app-debug.apk`; record both checksums and the commit SHA.
2. The APK is the Android Gradle debug build signed with the standard
   development debug key. Record `apksigner verify --print-certs app-debug.apk`
   and `aapt dump badging app-debug.apk`. The release APK is not the install
   candidate for this checklist.
3. Connect only the verified disposable profile with USB debugging already
   enabled. Run `adb devices -l`, record the Android build fingerprint and API
   level, then install the APK with `adb install app-debug.apk` after confirming
   the package checks above. Do not clear data during restore checks.

## UI, history, selection, and local limits

- Confirm `DEMO · Mock chat · No cloud AI` and the Cloud-disabled notice. Use
  synthetic conversation text only; confirm Demo replies and encrypted history.
- In Settings, switch Auto/Manual and Balanced/Cost Priority/Quality Priority.
  Confirm values survive closing/reopening Settings, rotation, and an
  `adb shell am force-stop app.serea.android.demo` followed by relaunch.
- Confirm Catalog is empty, no model is preselected, Manual has no selectable
  model, and an unavailable/deleted model does not silently fall back.
- Confirm task explanation, no verified price, and no claim that the local
  estimate is Provider usage or a billing cap.
- Exercise history switching, read, delete-one, delete-all, and reopen after a
  force-stop. Confirm pending requests are never replayed automatically.
- Exercise daily/monthly UTC Soft Limits with synthetic values. Confirm the UI
  distinguishes reservation estimates, Provider measured tokens, and unresolved
  requests. Do not interpret this as account-side spend protection.
- Rotate during a long synthetic response and while Settings is open. Test a
  small window, large font scale, TalkBack labels/focus, keyboard IME resize,
  and dark theme. Verify controls remain reachable and values remain legible.
- Confirm `FLAG_SECURE` is active while credential controls/settings are open
  and is cleared when Settings close; no real credential should be entered.

## Process Death, logs, and device security observations

```sh
adb shell am force-stop app.serea.android.demo
adb shell monkey -p app.serea.android.demo 1
adb logcat -c
adb logcat -d -s SereaProcessDeath:I SereaProcessDeath:E
adb logcat -d | grep -E 'Authorization:|api/chat|conversation body|api key'
```

Use `adb shell am force-stop` to demonstrate OS process death; Activity
recreation alone does not count. Verify settings, history, usage reservations,
and deletion after relaunch. Record only the pass/fail outcome and safe error
category, not conversation text.

Record the reported Android Keystore security level from Settings. Rooting
changes the device threat model: a reported TEE or StrongBox backed key does
not establish that the rooted OS, app process, or device is trustworthy.

## Result

| Check | Result | Evidence / notes |
| --- | --- | --- |
| APK SHA-256 and commit match | NOT_RUN | |
| Android build and install | NOT_RUN | |
| Auto/Manual and policy restore | NOT_RUN | |
| Empty Catalog and no fallback | NOT_RUN | |
| History, delete, and process death | NOT_RUN | |
| Soft Limit and unresolved usage | NOT_RUN | |
| TalkBack, font scale, IME, rotation, dark theme | NOT_RUN | |
| FLAG_SECURE and safe Logcat | NOT_RUN | |
| Keystore level observed (not a device trust verdict) | NOT_RUN | |

Physical-device acceptance: **NOT_RUN**. No live Provider request, real
credential, subscription, or billing operation is part of this test.
