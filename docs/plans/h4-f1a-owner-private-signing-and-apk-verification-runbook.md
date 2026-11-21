# H4-F1A — Owner-only Private Beta signing and APK verification runbook

**Current decision:** Option A was formally adopted by Owner decision `APPROVED — 2026-11-21`; see the [H4-F1D decision and Approval B readiness record](2026-11-21-h4-f1d-option-a-owner-decision-and-approval-b-readiness.md). This runbook remains Owner-run preparation only. Approval A does not authorize Approval B or any key generation/read/copy/backup/restore, APK build/signing, installation/distribution, device, Google Cloud, OAuth or Calendar operation. Each operation requires its own exact Owner approval. The agent must not handle key material/passwords.

## 1. Adopted identity model and pending key parameters

Owner adopted Option A: one fixed Owner-managed certificate dedicated to `app.serea.android.privatebeta`, separate from Public Beta and any future Production key. This supports a stable Android OAuth package/SHA-1 identity and an update chain without exposing a Production key. It also creates a critical recovery dependency: loss of the only usable private key may prevent compatible updates; compromise can require a new package or a supported signing lineage and may disrupt user-data continuity. The specific key-generation parameters remain subject to Owner's Approval B.

Do not adopt a public test certificate, Debug signer, CI/ephemeral signer or a key discovered in an existing workspace as the official Private identity. Existing key material has not been inspected. If an earlier Private APK exists, pause: the Owner must separately authorize artifact access and determine its signer before selecting/generating a replacement. A new certificate will not make an old differently signed package update-compatible.

### Proposed format and cryptographic choices

Preferred candidates are a PKCS#12 keystore (`.p12`/`.pfx`), RSA 3072-bit key, `SHA256withRSA`, and validity of at least 25 years, subject to current Android guidance and Owner policy. These are not final key-parameter approvals: the Owner must confirm exact format, key length, signature algorithm, validity, alias, minimal Subject, host and tools in Approval B. If API 26+ compatibility or policy is uncertain, pause and present RSA 2048-bit as an alternative; do not switch without Owner approval. Do not use MD5/SHA-1 signatures or reuse a personal TLS, Production, or unrelated signing key. The certificate validity date is not a substitute for backup or key continuity.

## 2. Owner decision and preparation checklist

Before key generation, Owner privately decides and records:

- [ ] Confirm the recorded Approval A decision: fixed Owner-managed Private Beta-only identity for `app.serea.android.privatebeta`; do not reinterpret Approval A as Approval B.
- [ ] Package identity remains `app.serea.android.privatebeta`.
- [ ] Whether an older approved Private APK exists, and separate authorization/procedure to inspect it without accessing any key.
- [ ] Keystore format, algorithm, validity, alias naming policy, custodian and access policy.
- [ ] Two independently protected recovery copies and how restoration will be tested without exposing key/passwords.
- [ ] Password manager/recovery arrangements; no password in shell history, environment, source, CI, chat, ticket, screenshot or build log.
- [ ] Exact source commit, toolchain, APK output, destination and artifact-handling scope approved for a later signing task.
- [ ] VersionCode: greater than the prior installed Private artifact and explicitly approved. Current source default `2` is not an approved release version.
- [ ] Install/device/update checks are separately and tightly authorized, including what data may be touched and how to stop/restore.

If any item is undecided, do not generate a key or APK.

## 3. Owner-only key generation and custody (future, after approval)

The following illustrates an interactive generation command. Run only on an Owner-controlled, trusted machine after separate approval. Use a new, dedicated absolute keystore path outside all repository/worktree/backup-sync folders; ensure its parent directory has restrictive permissions first. `keytool` should prompt for passwords; do not add password values as command-line arguments or environment variables. Do not include real identity details in public output.

```sh
keytool -genkeypair \
  -keystore <owner-controlled-private-location>/serea-private-beta.p12 \
  -storetype PKCS12 \
  -alias serea-private-beta \
  -keyalg RSA \
  -keysize 3072 \
  -sigalg SHA256withRSA \
  -validity <owner-approved-days>
```

The placeholder is instructional, not a value to submit literally. Choose the validity duration only after checking current official guidance and Owner policy. Answer identity prompts with only the approved minimal certificate subject. Never send the keystore, private key, alias if sensitive, passwords or unredacted certificate details to an agent/public PR. Record the certificate fingerprint only in the Owner's private record.

### Storage, backup and recovery

1. Keep the primary keystore encrypted at rest in an access-controlled Owner location outside the repository, developer shared folders and general cloud-sync storage. Restrict read access to the Owner and avoid unattended CI access.
2. Create at least two encrypted backups on independent media/storage failure domains. Protect the backup encryption key separately from the backup; do not place the password alongside its keystore. The [H4-F1E KIOXIA SSD readiness record](2026-11-21-h4-f1e-kioxia-ssd-backup-readiness.md) describes a proposed encrypted sparse-bundle image and independent second-medium plan. The SSD is not yet confirmed as a ready destination; its encryption state and hardware identity require Owner resolution, and image/backup creation remains unauthorized until explicitly approved.
3. Document the non-secret recovery inventory privately. Do not disclose exact storage paths, serials, device identifiers, actual certificate fingerprints or passwords publicly.
4. Perform a backup read/certificate-identity check only when the exact operation is included in a separately approved Owner scope. A successful `keytool -list` and matching public certificate can establish that the keystore can be read and its public identity matches; it does not prove that the private key can sign or establish end-to-end recovery. Do not describe this as a recovery test. Any cryptographic private-key use or test signing requires its own explicit approval; do not generate an APK/test artifact as part of Approval B. Do not install/distribute test artifacts or use them for OAuth registration.
5. Close/eject and secure any encrypted backup container after the approved read check, securely remove temporary copies under Owner policy, and document only the non-secret result. Do not describe a read/certificate comparison as proof of private-key usability or end-to-end recovery.
6. Do not use GitHub Actions/CI secrets or a committed/local project keystore as the long-term custody plan. If automated signing is ever proposed, it needs a distinct threat model, approval, access controls, audit, and recovery review.

## 4. Future Owner-only APK build/signing boundary

The current Gradle configuration deliberately clears signing configuration for both `beta` and `privateBeta`; Beta APK packaging is intentionally unsigned and the separate signing-domain CLI performs APK signing. This runbook does not change that boundary and does not require wiring Owner credentials into Gradle. If and when the Owner separately approves artifact creation, the existing Private Beta packaging guard requires an explicit `-PsereaPrivateBetaVersionCode=<approved-integer>` in the range `2..2100000000`; the source default `2` is not sufficient for packaging and is not an Owner version decision. The resulting artifact is unsigned and must not be represented as installable, signed, OAuth-ready, or approved. Never enable the HTTP Live gate as part of signing preparation.

After separate Owner approval of the exact source revision, versionCode and artifact operation, the Owner can invoke the approved Private Beta unsigned packaging task, then sign its verified output with `apksigner` using the Owner-held keystore via the approved signing-domain process. No signing-integration code/configuration change is implied or authorized by F1A. Do not assume `assemblePrivateBeta` yields a signed or approved APK. Never place passwords in command arguments, shell history, environment variables, CI logs or build properties. A schematic signing command, to be adapted only by the Owner to the verified SDK/build-tools path and approved APK, is:

```sh
apksigner sign \
  --ks <owner-controlled-private-location>/serea-private-beta.p12 \
  --ks-key-alias serea-private-beta \
  --out <owner-controlled-output>/serea-private-beta-signed.apk \
  <approved-unsigned-private-beta.apk>
```

Run interactively so tooling can request secrets securely. Do not run the schematic command from the agent or use guessed/unverified paths. Keep both unsigned and signed artifacts in approved private storage. Hash the final signed artifact immediately and transfer only through an Owner-approved private channel.

## 5. APK identity, integrity and certificate checks

Against the exact Owner-approved candidate APK, run these non-mutating checks using installed Android SDK Build Tools (`apksigner`, `apkanalyzer`) and a platform-provided SHA-256 utility:

```sh
apksigner verify --verbose --print-certs <approved-private-beta.apk>
apkanalyzer manifest application-id <approved-private-beta.apk>
apkanalyzer manifest version-code <approved-private-beta.apk>
apkanalyzer manifest version-name <approved-private-beta.apk>
shasum -a 256 <approved-private-beta.apk>
```

On systems without `shasum`, use an equivalent platform SHA-256 utility and record its name/version. Check every command's exit status. Do not treat `apksigner` output as provenance proof by itself; independently tie the APK to the approved source commit/build process and preserve a private chain-of-custody record.

Private evidence record (never publish real values in public PRs/issues):

| Field | Required evidence |
|---|---|
| Source | Approved source commit SHA and build/toolchain provenance |
| APK origin | Owner approval, build origin, artifact filename/location in private record |
| Artifact integrity | APK file SHA-256 hash |
| Package | `apkanalyzer manifest application-id` equals `app.serea.android.privatebeta` |
| Version | APK-derived versionCode/versionName; versionCode approved and suitable for update |
| Signature | `apksigner verify --verbose --print-certs` exit success |
| Signer count | Expected count is one unless a separately reviewed signing scheme documents otherwise |
| OAuth identity | Exact certificate SHA-1 from this APK, retained privately; this is the Android OAuth client input |
| Independent cross-check | Certificate SHA-256 separately recorded and compared with the approved identity; do not put SHA-256 in the OAuth SHA-1 field |
| Update comparison | Prior approved APK provenance, package, signer/signing lineage and versionCode comparison |
| Disposition | `PASS`, `NOT_VERIFIED`, or `BLOCKED`; evidence reviewer and date |

No real artifact is available/authorized in F1A. At this point APK provenance/hash, package/version from APK, signature result, signer count, SHA-1, SHA-256 and update compatibility are all **`NOT_VERIFIED` / `PENDING_OWNER`**. Do not fill in dummy fingerprints or infer values from Gradle, source manifests, `signingReport`, a CI key or an unsigned APK. The unsigned package output itself is not acceptable for OAuth registration.

## 6. Update compatibility and app data

Do not call an update compatible until the exact candidate and previous approved Private APK have been inspected and all are true:

1. Both have exactly the same package/application ID.
2. Candidate signature is compatible with the previous installed signer, or a supported, deliberately approved signing lineage has been verified.
3. Candidate versionCode is strictly greater than the installed versionCode and is accepted by the target Android version/policy.
4. The candidate artifact is provenance-approved and passes signature verification.
5. Any device/update test is separately approved and is performed without clearing app data or uninstalling as a workaround.

A compatible in-place update is the expected route to preserve app-private data, but only an explicitly authorized device test can establish observed behavior. A changed package installs as another app and does not migrate the old private data. A signer mismatch generally blocks in-place update; uninstall/reinstall may delete app-private data. Never claim existing APK update compatibility from source configuration or a merged manifest.

## 7. Loss, suspected compromise and rotation

- **Lost key / failed recovery:** halt artifact releases and OAuth identity work; do not generate an ad hoc replacement. Owner assesses recovery copies and decides whether a new app identity/reinstallation/data migration is acceptable.
- **Suspected leak:** stop signing and distribution; do not delete evidence or keep using the key. Owner initiates incident response, limits access, identifies exposed artifacts/systems, and decides on replacement/communication. Never upload material for diagnosis.
- **Rotation:** do not assume changing the certificate preserves updates. First verify platform-supported proof-of-rotation/signing-lineage requirements for the exact distribution/install channel and Android versions; obtain Owner approval, create a migration/recovery test plan, and assess OAuth registration implications. If continuity cannot be proven, treat rotation as a new app identity/reinstall migration with potential data loss.
- **Wrong/lost OAuth fingerprint:** do not register a Debug/CI SHA-1 as a workaround. Stop until the exact approved signed APK is verified and the Owner authorizes Cloud action.

## 8. Google Cloud is a separate Owner gate

F1A does not request Cloud action. Android OAuth registration remains `BLOCKED_BY_SIGNING_IDENTITY` until a real approved APK supplies a verified signer SHA-1 and independently checked SHA-256. The required future design inputs are:

- Owner-approved Private signing method and signed APK;
- verified package `app.serea.android.privatebeta`, provenance, version and signature;
- exact APK signer SHA-1 as Android OAuth input and separate SHA-256 audit comparison;
- Owner-confirmed Google Auth Platform Personal mode (creator account only);
- Android OAuth client, Google Calendar API only, `https://www.googleapis.com/auth/calendar.events.readonly`, `calendarId=primary`;
- no Gmail, Drive, Calendar writes/list, extra API, AI-powered-agent client, or multi-user access.

If Cloud work is later explicitly approved, stop before Console interaction and send a complete **`OWNER_ACTION_REQUIRED — GOOGLE_CLOUD`** handoff stating purpose, exact Console screen and official URL, preconditions, values the Owner enters, prohibited settings, expected confirmation, stop conditions and next-stage gate. Do not ask the Owner to proceed until project/audience onboarding and Android client creation are each explicitly approved; combined onboarding requires approval for both. Wait for the Owner's non-secret outcome and verify it. Project, audience, client, API, quota and billing are currently **UNCONFIRMED**.

## 9. Official reference pages (recheck before execution)

- [Android app signing](https://developer.android.com/studio/publish/app-signing)
- [Android `apksigner` documentation](https://developer.android.com/tools/apksigner)
- [Android app versioning](https://developer.android.com/studio/publish/versioning)
- [Gradle 8.14.6 Java compatibility](https://docs.gradle.org/8.14.6/userguide/compatibility.html)
- [Android build JDK guidance](https://developer.android.com/build/jdks)
- [Google Android client authentication](https://developers.google.com/android/guides/client-auth)
- [Google Auth Platform: manage app audience](https://support.google.com/cloud/answer/15549945?hl=en)
- [Google Auth Platform: manage OAuth clients](https://support.google.com/cloud/answer/15549257?hl=en)
- [Google Calendar authorization scopes](https://developers.google.com/workspace/calendar/api/auth)

This document gives preparation guidance only. Confirm the current official requirements, exact Console UI, tool version, artifact process and Owner approval immediately before any future execution.
