# Android REL-C: Firebase App Distribution

Date: 2026-10-22

Status: Draft implementation; Owner configuration and live Firebase distribution
are **NOT_RUN**.

## Scope and Lane B contract

`.github/workflows/android-firebase-distribution.yml` is a separate, manual
release route for an Owner-signed Personal Beta. It uses the existing Lane B
`android-internal-beta` GitHub Environment and does not change Android Gradle
files.

The accepted Lane B contract is:

- Application ID: `app.serea.android.demo`.
- Owner PKCS#12 inputs: `SEREA_ANDROID_KEYSTORE_BASE64`,
  `SEREA_ANDROID_KEYSTORE_PASSWORD`, `SEREA_ANDROID_KEY_ALIAS`, and
  `SEREA_ANDROID_KEY_PASSWORD` Environment secrets.
- Certificate pin: `SEREA_ANDROID_SIGNING_CERT_SHA256` Environment variable.
- Release task: `:app:verifyInternalBetaSigning :app:assembleRelease`.
- Version code: `GITHUB_RUN_NUMBER + 1`, matching Lane B's existing rule.
- Release APK path: `android/app/build/outputs/apk/release/app-release.apk`.

The route checks the dispatch SHA out exactly, confirms it is reachable from
`main`, and verifies every Owner-configured required check against that exact
SHA before it builds. It rejects missing configuration, an unsigned or
wrong-package APK, version mismatch, certificate mismatch, and checksum change.
Only the verified Release APK is passed to Firebase App Distribution. It is not
uploaded to GitHub Actions Artifacts or another public artifact store. The APK
and signing files are removed from the hosted runner after the run.

## GitHub settings required from the Owner

No GitHub Environment, reviewer, secret, variable, Firebase app, tester group,
or Google Cloud identity was created or changed by this PR. Configure and
review the following before dispatching a signed build:

1. Use the existing `android-internal-beta` Environment. Restrict deployments
   to protected `main`, require explicit Owner approval, and prevent
   self-review where GitHub supports it.
2. Keep the four Lane B signing secrets and certificate pin Environment-only.
   Never copy them to repository-level secrets.
3. Add Environment variable `SEREA_REQUIRED_CHECKS_JSON` as a JSON array of the
   exact required check-run names for protected `main`, for example
   `["Linux fast checks","Linux stable full validation"]`. The listed
   names must be kept synchronized with the required checks configured in
   GitHub branch protection/rulesets. The workflow fails closed if the list is
   absent, empty, duplicated, or any latest result on the exact source SHA is
   missing, pending, or not successful. GitHub does not expose branch-protection
   administration data to this workflow's read-only `GITHUB_TOKEN`, so this
   variable is an Owner-maintained mirror of that setting.
4. Create a Google Cloud Workload Identity Pool and GitHub OIDC provider. Map
   and constrain repository, ref, environment, and workflow claims to:
   `Yrika819/Serea`, `refs/heads/main`, `android-internal-beta`, and
   `.github/workflows/android-firebase-distribution.yml`. Do not issue or store
   a long-lived service-account key. Bind only this identity to the dedicated
   distribution service account and grant only the Firebase App Distribution
   upload permission it needs.
5. Add Environment variables `SEREA_GCP_WIF_PROVIDER` (full provider resource
   name), `SEREA_GCP_SERVICE_ACCOUNT`, `SEREA_FIREBASE_APP_ID` (Android app ID),
   and `SEREA_FIREBASE_APPROVED_TESTER_GROUP` (one Firebase group alias).
   Maintain that Firebase group with only Owner-approved Personal Beta
   testers. The workflow accepts one group alias, not tester emails or a list
   of arbitrary recipients.
6. After checking the WIF trust policy, service-account permission, Firebase
   app, and group membership, set `SEREA_FIREBASE_DISTRIBUTION_ENABLED` to the
   exact value `true`. Until then, the workflow cannot authenticate or
   distribute. Missing WIF configuration stops the run; there is no
   long-lived-key fallback.

The workflow dispatch input `distribute` defaults to `false`. A false run can
build, verify, and attest the APK but records Firebase distribution as
`NOT_RUN`. A live distribution requires an explicit `true` dispatch, all
protected Environment configuration, successful required checks, and Owner
approval. The current repository state has none of the new Firebase/WIF
settings, so live distribution remains **NOT_RUN**.

The Firebase CLI is pinned to `15.34.1` with an npm lockfile covering its
transitive dependencies. Installation uses `npm ci --ignore-scripts` before
Google authentication.

## Provenance and artifact attestation

The run summary records source SHA and tree SHA, numeric workflow ID and path,
run ID and attempt, Application ID, version code, APK SHA-256, signing
certificate SHA-256, Firebase App ID, and distribution result. It does not
record secrets, the tester group alias, Firebase CLI output, or authentication
responses. CLI output is captured privately so tester details and auth data do
not appear in logs.

After APK verification and before any Firebase authentication, the workflow
publishes a GitHub Artifact Attestation for the APK digest. It does not upload
the APK itself. The attestation contains build provenance only; it receives no
signing credentials, Firebase credentials, or tester information.

After downloading the APK from Firebase, a third party can verify its GitHub
provenance with:

```sh
gh attestation verify serea-personal-beta.apk \
  --repo Yrika819/Serea \
  --signer-workflow Yrika819/Serea/.github/workflows/android-firebase-distribution.yml
```

The verifier should also compare the verified APK digest with the successful
REL-C run summary. To inspect local package and signer facts independently,
use Android Build Tools `aapt dump badging` and `apksigner verify --verbose
--print-certs` on the downloaded APK.

## Regression coverage

The separate `android-firebase-distribution-tests.yml` workflow runs only on
pull requests touching this pipeline and receives no signing or Firebase
secrets. Its Fake Firebase CLI tests cover successful distribution, ordinary
CLI failure, authorization denial, checksum mismatch blocking before CLI
invocation, exact-SHA required-check enforcement, and APK package/version/
certificate verification. These tests never contact Firebase.

The Owner-signed distribution workflow itself has only `workflow_dispatch`.
It has no `pull_request` or `pull_request_target` trigger and grants no signing
secrets to PR workflows. It does not use `upload-artifact`.

## Remaining Owner verification

- Set and verify Environment deployment restrictions and required reviewers.
- Confirm the protected `main` required check list and mirror it in
  `SEREA_REQUIRED_CHECKS_JSON`.
- Establish the constrained Google WIF provider and least-privilege service
  account; verify Firebase App ID and approved group membership.
- After review, run the workflow first with `distribute=false`; confirm
  signature, provenance summary, and public attestation. A separate
  Owner-approved dispatch with `distribute=true` is required for any real
  Firebase delivery.
