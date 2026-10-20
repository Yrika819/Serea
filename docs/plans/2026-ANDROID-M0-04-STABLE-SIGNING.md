# Android AND-M0-04 protected signing and update test

Date: 2026-10-20

Scope: stacked Draft PR based on Android integration candidate PR #50

## APK identities

### Pull request validation

The ordinary Android workflow continues to build a Debug APK and run API 26
and API 35 tests without signing secrets. Its artifact is named
`serea-android-pr-validation-only`; its provenance labels it
`distribution_class=PR-validation-only` and `stable_update_identity=no`.
Android Gradle's Debug key is runner-local and may vary between runs. Treat the
APK as CI evidence. Before any manual install, compare its package and
certificate with the installed app. Never uninstall or clear an actual device
to work around a mismatch.

### Protected internal Beta

`.github/workflows/android-internal-beta.yml` has only a manual
`workflow_dispatch` trigger. Its signing job runs only for a dispatch whose
selected ref is `refs/heads/main`, checks out the exact dispatch SHA, and uses
the GitHub Environment `android-internal-beta`. It does not run on `pull_request`
or `pull_request_target`, and it does not build a PR head or accept a caller
supplied source SHA.

The workflow requires these values in the Environment before it can pass:

| Setting | GitHub setting | Use |
| --- | --- | --- |
| `SEREA_ANDROID_KEYSTORE_BASE64` | Environment secret | Owner-managed PKCS#12 keystore bytes, base64 encoded. |
| `SEREA_ANDROID_KEYSTORE_PASSWORD` | Environment secret | Keystore password. |
| `SEREA_ANDROID_KEY_ALIAS` | Environment secret | Signing key alias. |
| `SEREA_ANDROID_KEY_PASSWORD` | Environment secret | Key password. |
| `SEREA_ANDROID_SIGNING_CERT_SHA256` | Environment variable | Owner-recorded expected certificate SHA-256 fingerprint, colon separated or plain hex. |

Gradle reads the keystore path and credentials from process environment. The
workflow materializes the keystore under a mode-700 directory in runner temp,
uses the existing Release variant only after the signing preflight passes,
disables Gradle build/configuration cache, and deletes the temporary key files
in an `always()` cleanup step. The Release application ID is kept at
`app.serea.android.demo` to match PR #49's installed Debug package. It does not
upload the keystore, passwords, Gradle home, or build cache. The uploaded
artifact contains only the verified APK and a 14-day provenance file with APK
SHA-256, expected/observed certificate fingerprint, application ID, version
code, source/build commit SHA, and workflow run identifiers.

The signature check uses `apksigner verify` and compares the APK signer
fingerprint to the Environment's separately recorded expected fingerprint.
The workflow fails if any required setting is missing, if the signer does not
match, if package/version checks fail, or if the exact source SHA differs from
the build checkout. Version code is the workflow run number plus one, so the
first run is above the existing development baseline of 1 and each later run
increases it.

## Owner configuration gate

No GitHub Environment, secret, variable, required reviewer, or branch rule was
created or verified by this change. Until those settings exist, the internal
signing route is **NOT_RUN**. A dispatch with missing values fails its
preflight; it must not be reported as a successful internal Beta build.

Before using the route, the Owner should:

1. Create the GitHub Environment `android-internal-beta`.
2. Restrict deployments to the protected `main` branch and require explicit
   approval by an Owner. Enable prevention of self-review where available.
3. Add the four secret values and the expected certificate fingerprint to
   that Environment only. Do not use repository-wide secrets.
4. Confirm `main` itself is protected under the existing repository policy;
   this change does not alter Branch Protection or required checks.
5. Confirm the repository's artifact access and retention policy are suitable
   for internal APKs, then manually dispatch and approve one build.
6. Verify the run summary and downloaded artifact provenance independently.

The workflow YAML cannot prove that GitHub's Environment reviewer and branch
restrictions are configured; that setting remains an Owner-controlled gate.
An Environment created without protection would not meet this policy.

## Same-signer update compatibility test

The Android Emulator Required Gate now runs
`tools/test_android_update_compatibility.sh` on API 35 after the existing API 35
instrumentation/process-death checks. The script creates a fresh, short-lived
PKCS#12 test key under runner temp, builds two Debug APKs with version codes
100 and 101, verifies both package IDs and signatures, installs the first,
seeds encrypted Room history/settings/usage/Keystore state through the
debug-only process-death harness, and installs the second with `adb install
-r`. It force-stops and relaunches the upgraded app and rechecks the saved
records and recovery state. The CI emulator may remove its pre-existing test
installation to start from a clean test fixture; this does not operate on a
physical Owner device.

The compatibility result reports both APK SHA-256 values, the ephemeral test
certificate fingerprint, signature verification, package/version codes,
successful replacement, preserved encrypted history/settings/Keystore
reference, and process-death recovery. It reports only test evidence. The
ephemeral key and APKs are not uploaded or reused as distribution credentials.
This test does not establish that Owner signing is configured or that the
Pixel 7a APK can be updated.

## Pixel 7a and old signing key

PR #47's Debug certificate was
`d38d3eadd2ee74fe33b6c6fa803f9570100339b5db4229a173ec56377a4ab651`; PR #49's
was `ca82217d50c840b5183d3027ce56a5d1c3f862d32f0ae3979658014e3b7b912c`.
The PR #49 APK already installed on the Owner's Pixel 7a has package
`app.serea.android.demo`. Its signing private key is not present in this
repository or the current environment. The protected workflow's Release APK
keeps the `.demo` application ID suffix to match that package, but the new
owner-managed certificate still will not match automatically.

An in-place update requires both an identical application ID and a certificate
accepted by Android for that package. Before attempting an install, the Owner
must determine whether the PR #49 key can be recovered and authorized, or
select a deliberate migration path. If neither is possible, preserve the
current installation and its data while deciding how to retain/export it; do
not uninstall, clear data, or claim that a new key preserves it. Future
internal APKs signed by the same Owner key and with a higher version code are
the update target for fresh installs after the migration/identity decision.

## Workflow security review

- Token permissions are limited to `contents: read`; checkout does not persist
  credentials.
- Only manual dispatch is enabled. A job condition requires `main`; the
  Environment must independently restrict deployable branches to `main` and
  require Owner approval.
- No PR workflow receives signing secrets. No untrusted PR code is combined
  with a signing secret. There is no `pull_request_target` use.
- The signing job does not restore or save caches. It uses a fresh hosted
  runner and disables Gradle build/configuration caches.
- Secret values are passed only to the preflight, materialization, or Gradle
  steps that need them. The workflow does not echo them or add them to
  provenance. The keystore is outside the workspace and artifact path.
- The uploaded APK comes from the exact checked-out main commit, passes
  signature and identity checks, and has provenance recorded alongside it.
- GitHub Environment protections and artifact reader access cannot be inspected
  or set from this repository change and remain unverified until Owner setup.

## Next steps

Keep this Draft PR separate from PR #50. Run its Android and repository CI at
the exact final head. Owner review is still required for the Environment,
certificate, Pixel 7a migration, ADR-0044/0045, provider policy, and later Beta
distribution. AND-M0-05 may start on mock-only Provider boundaries once its
Owner inputs are recorded; live traffic and credentials remain disabled until
that milestone's authorization.
