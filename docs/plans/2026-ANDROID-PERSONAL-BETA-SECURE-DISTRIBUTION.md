# Personal Beta signing and restricted distribution

Date: 2026-10-22

Status: Candidate implementation. Owner signing, WIF, and Firebase delivery are
**NOT_RUN**; the required GitHub Environment and cloud settings remain Owner
managed.

## Distribution identity and flow

`app.serea.android.beta` is the sole Personal Beta distribution package. The
Gradle `beta` variant pins this ID and refuses packaging unless the Owner
PKCS#12 alias, passwords, certificate SHA-256, and version code validate. The
final APK verifier checks its package ID, version code, APK signature, pinned
certificate, and SHA-256 digest. Before distribution, Firebase's Android app
inventory must map the configured Firebase App ID to the same package ID.

`.github/workflows/android-personal-beta.yml` is manual and runs only for the
exact `main` workflow source in `Yrika819/Serea`. It checks out `github.sha`,
verifies the checkout and workflow ref, confirms the source SHA is reachable
from `main`, then requires every Owner-configured check to have a successful
GitHub Actions check-run on that exact SHA. The job uses the dedicated
`serea-personal-beta` Environment, which must require Owner review and restrict
deployment branches to protected `main`.
Before accessing signing inputs, the workflow queries GitHub's `main` branch
protection API and requires the configured checks, at least one pull request approval,
and rule enforcement for administrators. Missing or unprotected `main` stops the
workflow.

The workflow builds and verifies the Beta APK on the hosted runner. It creates
a GitHub build provenance attestation for the APK digest and sends the verified
APK directly from that runner to one Owner-approved Firebase App Distribution
group only when the manual `distribute` input is `true`. The signed APK is not
uploaded to GitHub Actions Artifacts. The ordinary CI debug APK and this
Owner-signed Beta APK have separate build variants, paths, signing inputs, and
delivery routes. The attestation is public provenance metadata; it does not
contain the APK.

Google authentication uses GitHub OIDC and Workload Identity Federation only.
There is no service-account JSON key input or automatic long-lived-key
fallback. The distribution workflow rejects a missing WIF provider, service
account, Firebase project/app, approved group, or explicit distribution flag.
The signed APK and generated WIF credentials are removed from the runner after
the job.

Version code is the Owner-configured highest previously distributed Beta
version code plus `GITHUB_RUN_NUMBER`. The baseline accounts for installs made
before this workflow was introduced. New workflow runs advance it monotonically;
a rerun attempt is rejected because GitHub reuses the run number. A retry
requires a new manual dispatch, which advances the version code.

## Owner configuration required

Before any signed build, configure `serea-personal-beta` with required reviewers
and a protected branches restriction. Protect repository `main` with the same
required checks listed in `SEREA_REQUIRED_CHECKS_JSON`, at least one pull request approval,
and rules enforced for administrators. Keep signing secrets
and release variables Environment-scoped; do not place them at repository scope.

Required signing secrets:

- `SEREA_ANDROID_PERSONAL_BETA_KEYSTORE_BASE64`
- `SEREA_ANDROID_PERSONAL_BETA_KEYSTORE_PASSWORD`
- `SEREA_ANDROID_PERSONAL_BETA_KEY_ALIAS`
- `SEREA_ANDROID_PERSONAL_BETA_KEY_PASSWORD`

Required signing and repository variables:

- `SEREA_ANDROID_PERSONAL_BETA_CERT_SHA256`
- `SEREA_PERSONAL_BETA_VERSION_CODE_BASELINE`, the highest version code already
  distributed for this package when enabling the workflow
- `SEREA_REQUIRED_CHECKS_JSON`, a JSON array of exact check-run names required
  by protected `main`

For Firebase delivery, additionally configure:

- `SEREA_FIREBASE_DISTRIBUTION_ENABLED=true`
- `SEREA_GCP_WIF_PROVIDER`
- `SEREA_GCP_SERVICE_ACCOUNT`
- `SEREA_FIREBASE_PROJECT_ID`
- `SEREA_FIREBASE_APP_ID`, whose registered package must be
  `app.serea.android.beta`
- `SEREA_FIREBASE_APPROVED_TESTER_GROUP`, a single group alias

Constrain the WIF trust policy to this repository, protected `main`, the
`serea-personal-beta` Environment, and this workflow path. Grant the bound
service account only the Firebase App Distribution permission required to
upload. Keep only Owner-approved people in the tester group. Until these
settings and approval rules are independently reviewed, real signing and
Firebase distribution remain **NOT_RUN**.

## Disposable emulator update guard

PR #92's stricter API 35 ADB guard and mock tests are included. The update test
targets only `app.serea.android.updatecompat`, uses a throwaway PKCS#12 key, and
pins every ADB command to the one verified API 35 QEMU emulator. Negative Mock
ADB cases cover physical, multiple, unauthorized, fake, wrong-API, mismatched
serial, and missing-designation targets. It has no `app.serea.android.demo`
uninstall path. The Beta identity test independently refuses a pre-existing
Beta package and never uninstalls Demo.

The Personal Beta identity emulator job runs after the API 26/API 35
instrumentation job, and its result is part of the Android Required Gate. This
keeps the two full emulator activities sequential in the workflow run.

## Local mock validation

Run the contract checks with:

```sh
python3 -m unittest discover -s tests -p android_firebase_distribution_tests.py
bash tools/test_android_update_compatibility_guard.sh
```

These checks use fake APK tools, a Mock Firebase CLI, and Mock ADB. They do not
use an Owner key, Firebase, Google credentials, or a connected device. They
cover distribution success/failure/authorization denial, WIF failure, exact
source and check-run gates, package/certificate/version/checksum verification,
Firebase package binding, signed APK artifact separation, version-code reruns,
and the protected Environment workflow contract.
