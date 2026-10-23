# Serea Personal Beta Signing and Supply Chain Readiness

Status recorded for the isolated security campaign on 2026-10-23. This document describes the release design and the Owner evidence required before real credentials or Firebase distribution can be used.

## Threat matrix

| Threat | Control in this change | Evidence or remaining limit |
| --- | --- | --- |
| Gradle, plugins, or build scripts read the long-lived Beta signing key | `beta` has no Gradle signing config or Owner-secret reads. The Build job receives no signing, transport, Firebase, or WIF values. | Confirmed by source and workflow contract tests. Build artifact is unsigned. |
| A different or stale build artifact is signed | Signing runs after the exact-main-only Environment gate on a fresh hosted runner. It checks the API-reported main/tree SHA, exact-SHA Required Checks, artifact run ID, source/tree/version metadata, APK digest, Application ID, unsigned signature state, and GitHub provenance before accessing the key. | Negative metadata, digest, package, and workflow-boundary tests use synthetic inputs. The GitHub attestation chain itself needs a real Owner-approved main workflow run. |
| A malicious APK is signed merely because Build and Sign are separate jobs | Sign does not checkout the repository, execute Gradle, or execute any repository helper. It accepts the one unsigned artifact produced in this workflow run only after the exact source, checks, metadata, signature state, and Build provenance pass. Signing and Distribution Environments each require the single configured Owner reviewer. | Owner review of the exact source SHA, artifact SHA, required-check evidence, and release summary remains necessary. A reviewer can still approve malicious source or a malicious dependency that passes the configured checks. |
| Android signing key is reachable during build or distribution | The signing key exists only in the Signing Environment and is materialized for the single apksigner step. Distribution receives a separate encrypted transport private key; it does not receive the Android signing key. | No Owner key was used in this campaign. Runtime separation is defined by job permissions and fresh hosted runners. |
| Signed Beta APK is exposed through a public Actions artifact | Build uploads only `unsigned.apk` and metadata. Signing encrypts the final APK to the pinned transfer certificate and uploads only `.apk.cms`. The clear signed APK and Owner key are deleted before artifact upload. | Contract tests inspect the upload paths. Build provenance covers the unsigned APK; signed output is bound to the same workflow run by Sign job outputs and checked again after decryption. |
| Ciphertext is replaced or decrypted under a different transport identity | Both Environments pin the transfer certificate fingerprint. Distribution checks the ciphertext digest, key/certificate match, CMS decryption, signed APK digest, package/version, APK signature, and Owner certificate before WIF. | Local end-to-end test uses disposable RSA keys; real Owner transfer key is NOT_RUN. |
| Firebase or GCP credentials enter Build or Sign | Only Distribution has `id-token:write`, WIF configuration, transport decryption credentials, and Firebase variables. Build has `id-token:write` only for GitHub build provenance; its OIDC subject has no Distribution Environment claim. | Owner must restrict the Google WIF provider and service-account binding to the exact Distribution Environment subject, repository, workflow, main ref, and audience. The live provider condition was not available to inspect and is NOT_RUN. |
| OSV scanner tag changes behind a Git commit-pinned Action | The Android Dependency Audit runs the scanner from the immutable image digest `sha256:fc633b975f339ddfde55c747f5bf4dde71605ccda1073fd535760ba38e4e7065`. It rejects an image override, runs the scanner as UID 65532 with no capabilities and a read-only Android source mount, and fails if execution or JSON output fails. | Digest immutability and fetched manifest digest were checked. The image labels point to `google/osv-scanner` revision `b97d1de7d8c3c7de8c11308b3d9cb5bbf3f7a0e9`, whose GitHub commit is verified. No independent OCI build attestation was available, so image publisher/build provenance is **NOT_PROVEN**. |
| Tool download is substituted | Signing and distribution install Android Build Tools 35.0.0 and Android 35 from fixed Google URLs and verify hard-coded archive SHA-256 values plus apksigner.jar, aapt, and android.jar hashes. GitHub CLI 2.103.0 is checksum-pinned before provenance/API checks. | Integrity mismatch is fatal. Google SDK ZIP authenticity is not independently signed. GitHub CLI source tag is signed, but release-binary provenance is not independently attested. Firebase CLI dependencies are locked with npm integrity hashes and installed with lifecycle scripts disabled; publisher provenance is still NOT_PROVEN. |
| Gradle dependency metadata promotes compromised bytes to trust roots | No active `android/gradle/verification-metadata.xml` is added and Gradle dependency verification is not enabled. Existing SEC-A2 candidate metadata and publisher review remain evidence only. | Existing evidence covers 502 component coordinates and 880 candidate hashes, but does not independently authenticate every Plugin, POM, Gradle Module Metadata, UTP artifact, and ordinary artifact. Existing review records 273 coordinates without complete independent publisher evidence and 10 task-resolved entries missing from the help-only graph. Enforcement remains **NOT_READY**. |

## Release stages

1. **Build unsigned** validates exact `workflow_dispatch` source, current `main`, branch protection, and configured checks before Gradle runs. Gradle builds `app.serea.android.beta` without signing values. The job records source SHA, tree SHA, workflow run, versionCode, package ID, and APK SHA-256, uploads only the unsigned APK and metadata for one day, and creates GitHub build provenance.
2. **Sign** starts on a new hosted runner only when `sign=true`. The `serea-personal-beta-signing` Environment is configured for exactly `main` and the single Owner reviewer. Sign has no repository checkout and executes only the reviewed workflow steps, pinned Android signing tools, and pinned GitHub CLI. It rechecks source/tree, branch policy, checks, version, run-bound artifact metadata, unsigned state, digest, package, and GitHub provenance before materializing the Owner key. It verifies the signed APK against the Owner certificate, encrypts the result, deletes the plaintext APK and key, and uploads only the ciphertext.
3. **Distribute** starts on another runner only when both `sign=true` and `distribute=true`. The `serea-personal-beta-distribution` Environment is also restricted to `main` and the single Owner reviewer. Distribution has no repository checkout; it downloads only this run's ciphertext, rechecks its digest, decrypts with the separate transfer key, and validates the signed APK and certificate. It fetches only the locked Firebase CLI manifests from the exact source SHA, installs with lifecycle scripts disabled, rechecks source and Required Checks, and only then exchanges a GitHub OIDC token through WIF and calls Firebase App Distribution.

Every job fails closed when its required inputs, checks, identities, tools, artifacts, or API responses are missing or malformed. Both Environment gates use a selected-branch policy containing exactly `main`, one configured Owner user/team ID, and GitHub's `prevent_self_review` rule. Workflow Dispatch by itself never requests signing or distribution; both inputs default to false. `distribute=true` with `sign=false` fails in Build before Gradle runs. Build needs an OIDC token for GitHub provenance, so the Google WIF condition must reject the Build job's non-Environment subject.

## Owner setup checklist

### Repository settings

- Create repository variables `SEREA_PERSONAL_BETA_VERSION_CODE_BASELINE` (last known installed Beta versionCode) and `SEREA_REQUIRED_CHECKS_JSON` (a nonempty JSON array of exact Required Check names). Keep these at repository or organization scope so the credential-free Build job can read them.
- Set `SEREA_BETA_OWNER_REVIEWER_ID` and `SEREA_BETA_OWNER_REVIEWER_TYPE` (`User` or `Team`). The selected user/team must be the release Owner approver.
- Protect `main` with at least one pull request approval, strict required checks, administrator enforcement, and a policy that keeps the release workflow and signer scripts under review.
- Keep workflow dispatch permission limited to trusted maintainers. Environment review remains required even for a trusted dispatcher.

### GitHub Environments

Create `serea-personal-beta-signing` and `serea-personal-beta-distribution` separately.

- Set each deployment branch policy to selected branches and add exactly `main`; do not add patterns or tags.
- Configure exactly the selected Owner user/team in the required reviewers rule and enable **Prevent self-review**. The workflow checks the API reviewer ID/type, exact `main` branch policy, and self-review setting after the gate.
- Disable administrator bypass in Environment settings. The current Environment API response does not expose that setting consistently, so record a screenshot or settings export as Owner evidence.
- Configure these public variables in both Environments and keep values identical:
  - `SEREA_BETA_TRANSPORT_CERT_PEM`
  - `SEREA_BETA_TRANSPORT_CERT_SHA256`
- Configure `SEREA_ANDROID_PERSONAL_BETA_CERT_SHA256` in Signing and Distribution. The certificate pin is public; the private key is not.

Signing Environment secrets only:

- `SEREA_ANDROID_PERSONAL_BETA_KEYSTORE_BASE64`
- `SEREA_ANDROID_PERSONAL_BETA_KEYSTORE_PASSWORD`
- `SEREA_ANDROID_PERSONAL_BETA_KEY_ALIAS`
- `SEREA_ANDROID_PERSONAL_BETA_KEY_PASSWORD`

Distribution Environment secrets only:

- `SEREA_BETA_TRANSPORT_PRIVATE_KEY_BASE64`
- `SEREA_BETA_TRANSPORT_KEY_PASSWORD`
- Firebase/WIF settings are variables, never long-lived service-account key JSON.

### Firebase and WIF

- Bind `SEREA_FIREBASE_PROJECT_ID`, `SEREA_FIREBASE_APP_ID`, `SEREA_FIREBASE_APPROVED_TESTER_GROUP`, and `SEREA_FIREBASE_DISTRIBUTION_ENABLED=true` in the Distribution Environment. Verify the Firebase Android App ID is registered for `app.serea.android.beta` and the tester group is Owner-controlled.
- Create a dedicated Google service account with only the Firebase App Distribution role required by the project. Grant GitHub WIF impersonation to that service account, not a downloaded JSON key.
- Constrain both the WIF provider and service-account IAM binding to repository `Yrika819/Serea`, `refs/heads/main`, this workflow path, `workflow_dispatch`, and `repo:Yrika819/Serea:environment:serea-personal-beta-distribution` as the exact subject. Restrict the audience to this provider. This Environment subject prevents Build's GitHub-attestation OIDC token from impersonating the Firebase service account. Save the provider condition and service-account IAM policy as Owner evidence; they cannot be verified without GCP access.
- Set an emergency disable runbook: turn `SEREA_FIREBASE_DISTRIBUTION_ENABLED` off, remove WIF trust or disable the service account, disable the Environment gate, and rotate exposed transport credentials. If the Android signing key is compromised, stop updates under that identity and communicate the required reinstall/migration plan; Android does not permit an app update under an unrelated signing key.

### Offline key and release acceptance

- Generate the long-lived Android Beta signing key offline; keep encrypted backups under Owner control. Import only its PKCS#12 bytes and passwords into the Signing Environment. Record its certificate SHA-256 from an independent offline inspection and compare it with both Environment pins.
- Generate a separate RSA 3072 transport certificate and encrypted private key. Put the certificate and SHA-256 pin in both Environments. Keep the encrypted private key and passphrase only in the Distribution Environment. Rotate this pair independently of the Android app signing key.
- Before any real run, use disposable keys to validate unsigned Build, signer rejection cases, same-key update acceptance, different-key update rejection, CMS tamper/wrong-key rejection, and APK package/version/certificate checks. Never place an Owner key in a local or CI test.
- For a release candidate, retain the workflow run ID, exact source/tree SHA, exact-SHA Required Check list, unsigned and signed APK SHA-256, package ID, versionCode, Owner certificate SHA-256, Build attestation verification, signer Environment approval, distributor Environment approval, Firebase project/app/group and Firebase result. Mark any absent evidence `NOT_RUN`; do not infer PASS from a skipped job.

## Supply-chain disposition

- Gradle wrapper distribution URL and `distributionSha256Sum`, wrapper JAR checksum, dependency lockfiles, RustSec and existing OSV checks remain in force. No security check is disabled.
- Gradle dependency verification metadata remains inactive until every resolved Plugin, POM, Gradle Module Metadata file, UTP artifact, and ordinary dependency has independent publisher/integrity evidence and a reviewed strict metadata file. No generated hash is promoted to trusted metadata by this change.
- All `uses:` references in the Personal Beta workflow and scanner workflow are full commit SHAs. The OSV scanner itself is invoked as a separate digest-pinned container because the Action's Docker metadata selects the mutable `v2.3.3` tag.

## Campaign evidence

- PR #101 remains open and unchanged at `0c342409155aaa04c732a637827d3391376f6c67`; PR #98 still points to the fixed base `00e0453f83970a2b7d2bd26ce01462e421f75f46`. Fast CI and Full CI remain failed on the original PR because of Commit Identity Guard; Android also remains failed there. This branch reconstructs the PR #101 patch after the fixed base with approved noreply identity.
- Focused contract tests passed locally: 23 Firebase/release tests, 7 isolated signing/scanner tests, and 24 Commit Identity tests. YAML, embedded shell/Python syntax, `git diff --check`, APK mutation negatives, and mock ADB guard cases passed.
- Disposable test-key signing, certificate verification across versionCode 101 to 102, wrong-key rejection, package mismatch rejection, and encrypted handoff checks passed. This checks signature compatibility data, not Android PackageManager installation or app data preservation on a running emulator.
- The digest-pinned OSV scanner completed against the Android Gradle lockfile, scanned 324 packages, and emitted valid JSON with zero reported findings. Its immutable digest is fixed, but independent OCI publisher/build provenance remains **NOT_PROVEN**.
- OSV scanner source-copy symlink rejection is covered by a negative test and passed locally.
- Local `:app:assembleBeta` is **NOT_RUN**: the Gradle wrapper distribution download failed with a connection refusal before Gradle could execute. No assembled candidate APK is available from this run.
- Android same-key update installation, different-key PackageManager rejection, and app-data preservation are **NOT_RUN**: no emulator is available (`adb devices` listed none). Mock ADB guard cases passed; they do not prove PackageManager behavior.
- Main is reported protected and its four required check names are visible through the branch endpoint. Detailed review-count, strictness, and administrator-enforcement fields could not be read because the GitHub App received HTTP 403 for the branch-protection endpoint.
- The `serea-personal-beta-signing` and `serea-personal-beta-distribution` Environments are **NOT_CONFIGURED** (GitHub returned 404). Their Owner reviewer rules, selected-main policies, and secrets therefore remain Owner setup work; the workflow fails closed before signing or WIF when these Environments are absent.
- Owner Android signing key: **NOT_RUN**.
- Owner transport private key: **NOT_RUN**.
- GitHub Environment reviewer configuration: **NOT_CONFIGURED / Owner action required**.
- Real WIF authentication: **NOT_RUN**.
- Firebase app/tester group validation and distribution: **NOT_RUN**.
- No candidate PR was created because the local Gradle Beta build and Android PackageManager update acceptance remain unverified. No Actions CI exists for this local candidate SHA.
