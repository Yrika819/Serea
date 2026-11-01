# REL-B — Dedicated Personal Beta application identity

## Identity and build behavior

- The existing `debug` type remains `app.serea.android.demo`; `isolated` remains
  `app.serea.android.isolated`; `release` remains `app.serea.android`.
- The new `beta` build type pins `app.serea.android.beta` and displays
  **Serea Personal Beta**. A Gradle variant assertion fails if its resolved
  application ID changes.
- `assembleBeta` requires a PKCS12 private-key entry, alias, passwords, expected
  certificate SHA-256, and `-PsereaBetaVersionCode` greater than 1. Gradle
  checks the keystore certificate before packaging and removes a stale Beta APK
  if preflight fails. The APK verifier checks the final application ID, label,
  version code, APK signature, and signer fingerprint.
- The Owner-signed workflow is manual, runs only from the protected `main`
  workflow source, and reads signing inputs only from the
  `serea-personal-beta` GitHub Environment. That Environment must require
  reviewers and restrict deployments to protected branches. Repository `main`
  must require the configured checks, at least one pull request approval, and
  enforce protection for administrators. The workflow fails closed if any of
  these protections are absent. It expects these Environment secrets:

  - `SEREA_ANDROID_PERSONAL_BETA_KEYSTORE_BASE64`
  - `SEREA_ANDROID_PERSONAL_BETA_KEYSTORE_PASSWORD`
  - `SEREA_ANDROID_PERSONAL_BETA_KEY_ALIAS`
  - `SEREA_ANDROID_PERSONAL_BETA_KEY_PASSWORD`

  and these Environment variables:

  - `SEREA_ANDROID_PERSONAL_BETA_CERT_SHA256`
  - `SEREA_PERSONAL_BETA_VERSION_CODE_BASELINE`, the highest version code
    already distributed for this package
  - `SEREA_REQUIRED_CHECKS_JSON`, exact GitHub Actions check names required on
    both protected `main` and the exact signed source SHA

  The certificate variable is the SHA-256 digest of the configured alias's
  certificate. The workflow version code is the Owner baseline plus the
  monotonic workflow run number; reruns are blocked and require a new dispatch.
  When Firebase distribution is enabled, the workflow sends the verified Beta
  APK directly to the Owner-approved Firebase group through WIF. It does not
  upload a signed APK to GitHub Actions Artifacts. The Owner keystore,
  fingerprint, Environment, branch protection, and cloud settings were not
  created or supplied for this change.

The test-only `sereaPersonalBetaValidationHarness` Gradle property includes the
existing process-death receiver in that build alone. The protected signing
workflow does not set it, so distributed Beta APKs contain no ADB test receiver.

## Isolation audit

- The main manifest declares no content/file provider and no `authorities`.
- Room uses names including `serea-history.db` and `serea-tasks.db`. Android
  stores them under the application-private data directory, so Demo and Beta
  have separate databases because their application IDs and UIDs differ.
- The encrypted Room values use Android Keystore aliases including
  `serea-history-v1` and `serea-tasks-v1`; credentials use
  `serea-cloud-credential-v1`. Android Keystore access is scoped to the calling
  application UID, so identical alias strings do not share keys between Demo
  and Beta.
- Real OAuth is disabled in `GoogleAuthorization` and no OAuth client ID,
  redirect URI, or package-bound OAuth resource is configured in this tree.
  The application ID change therefore creates no OAuth identity collision.

## Disposable emulator check

Use a fresh API 35 QEMU AVD with one connected device, then run:

```sh
SEREA_PERSONAL_BETA_DISPOSABLE_EMULATOR=1 bash tools/test_personal_beta_identity.sh
```

The script generates and deletes two temporary PKCS12 keys. It refuses an AVD
that already has the Beta package, never runs `adb uninstall`, and does not
launch or write through Demo. It verifies Demo's installed APK signer, package
version, and app-private files before and after; installs Demo only if it is
absent; then checks co-installation, encrypted Room/Keystore data after reboot,
same-key version 100 to 101 upgrade, and rejection of a higher-version APK
signed by a different temporary key. The emulator must support `adb root` so
the Demo data comparison can read the app-private files without executing the
Demo app.

The normal Required Gate and its 103 instrumentation tests are unchanged. The
Android build job adds only a compile check for the Beta variant; it does not
sign or upload a Beta APK.
