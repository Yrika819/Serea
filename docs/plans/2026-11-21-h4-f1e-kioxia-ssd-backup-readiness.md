# H4-F1E — Approval B Preflight & KIOXIA SSD Backup Readiness

**Outcome:** `F1E_PARTIAL` — Approval B preparation documented, but the external SSD is **not accepted as a ready backup destination** yet. The metadata-only check found an approximately 1 TB external SSD with a mounted, writable ExFAT volume and available space, but macOS did not report a manufacturer identity or encryption state. Stop before using it for key material or creating an encrypted image until the Owner confirms identity and the exact encrypted-image operation is separately authorized.

**Approval state:** A `APPROVED — 2026-11-21`; B, C, D, E `NOT_AUTHORIZED`.
**Base:** `origin/main` HEAD `e3fdfd8a74147d9db212ea592a2a8101349442f9`, Tree `1baf906c24d62a39c09c6d2d18985aef7ccaa681`.

## 1. Read-only SSD observation

This is a one-time macOS metadata snapshot taken after an external physical disk was reported connected. Commands used were `diskutil list external physical` and `diskutil info` on the candidate whole disk and its mounted data volume. No files or directory contents were listed or opened. No `df` follow-up was needed because `diskutil info` reported volume usage.

| Requested property | Observation | Assessment |
|---|---|---|
| External physical drive present | Yes; a USB-connected external physical SSD was reported. | Connection observed for this check only. |
| KIOXIA manufacturer | The volume label is `KIOXIA`; the device/media model string reported by macOS is `EXCERIA PLUS`. `diskutil` output did not expose a manufacturer field, and the volume label alone is not proof of make. | Consistent with the Owner's description, but manufacturer identity is **not independently verified**. No serial number or persistent device ID was read into this record. |
| Physical capacity | About 1.0 TB (macOS-reported physical size approximately 1,000,204,886,016 bytes). | Meets the stated nominal capacity. |
| Disk/volume mapping | macOS associated the external physical device with one mounted data volume named `KIOXIA`. | Mapping observed; transient device node, UUID and mount path intentionally omitted. |
| Filesystem | Mounted filesystem personality: ExFAT. The partition-type label is a generic Microsoft data type; this is not evidence that the mounted filesystem is NTFS. | ExFAT detected. Do not reformat or alter it. |
| Used / available | Approximately 740.4 GB used and 259.7 GB free of approximately 1.0 TB. | Capacity appears sufficient for a small encrypted image, subject to Owner confirmation and free-space check at execution time. |
| Read/write state | Whole disk and mounted volume both reported not read-only. | Writable state observed; **no write test was performed**. Writable does not mean safe or approved to write. |
| Encryption | macOS metadata did not report an encryption state. ExFAT itself does not provide APFS/FileVault-style volume encryption; no transparent hardware encryption status was established. | `ENCRYPTION_STATE_UNVERIFIED`; do not put key material directly on this volume. Stop until the Owner resolves the state/identity question. |
| Backup suitability now | No encryption has been confirmed and no encrypted image exists. | `NOT_READY_FOR_KEY_BACKUP`. The SSD remains a proposed destination only. |

The user-supplied model and volume name are not enough to override the encryption/identity stop condition. No attempt was made to read serials, inspect personal files, scan the volume, mount/unmount it, repair it, test-write it, or change settings. Existing data remains untouched.

## 2. Backup design and destination decision

### Primary keystore

The future original remains on an Owner-managed Mac, in an Owner-selected location outside the repository, all worktrees, build outputs, GitHub/CI, shared folders and general-purpose sync. The Owner limits access to themself and uses an interactive PKCS#12 keystore password. Actual paths and credentials remain private and are not collected in this record.

### Backup 1 — external SSD candidate

**Proposed method:** after Owner confirms the physical device and explicitly approves the exact future operation, create a **separately encrypted, read/write sparse-bundle disk image** as an ordinary file/package on the existing ExFAT volume. Do not encrypt, erase, repartition or reformat the whole SSD. Format the filesystem *inside* the image as APFS (non-case-sensitive unless the Owner has a concrete need otherwise); protect the image with **AES-256**; choose a bounded virtual capacity (a small value such as 1 GB is ample for a Keystore plus certificates and metadata, but the Owner selects it). The sparse bundle grows as its bands are used, subject to its virtual size and the SSD's free space. Eject the mounted image when finished, then safely eject the external SSD and keep it offline.

The Mac's read-only `hdiutil create -help` output lists `SPARSEBUNDLE`/`UDSB` as a sparse-bundle image type and `AES-256` as 256-bit AES encryption, recommended by the local utility help. Apple's Disk Utility guide describes creating a blank read/write image, selecting filesystem, size, image format and encryption, and notes that a sparse-bundle grows with stored data across multiple files. These sources support the **design option**, not the claim that an image has been created or that this exact ExFAT SSD has been tested with one.

#### Why not encrypt the whole SSD?

Whole-device encryption changes the protection and access requirements for every existing file, can affect cross-platform access, and Apple Disk Utility's whole-device encryption/format workflow may require erasing/reformatting. That risks existing data and is not appropriate here. Do not select “Erase,” change partition maps, convert filesystems, enable encryption on the existing volume, or accept a workflow that proposes erasure.

An encrypted image adds a separate encrypted container without intentionally changing the surrounding ExFAT filesystem or existing files. Its password is required to mount/read it. However, image creation still writes new data and can fail or consume space; it is **not authorized now**. A sparse bundle is a directory-like package made of multiple band files, so it requires successful file creation/rename operations and enough free space on the backing filesystem. ExFAT supports large files but does not provide APFS metadata, access-control or native encryption semantics. If Disk Utility cannot create/mount/verify the selected image on the confirmed volume without proposing a format change, stop; do not switch to a destructive whole-disk method. The future Owner action should first review the destination in Finder/Disk Utility and the existing data's backup status, then obtain specific written approval for the image creation and copy operation.

**Current destination gate:** `KIOXIA_SSD_CANDIDATE / NOT_READY`. Manufacturer identity is not independently verified, encryption state is unverified, and the sparse-bundle has not been created or tested. The known free space is an observation only, not permission to write.

### Backup 2 — independent physical failure domain

`BACKUP_2_PENDING_OWNER`. Owner selects a second separately encrypted removable medium with independent device and storage failure domains (for example, another encrypted external SSD or USB storage). It must not be another partition on the KIOXIA device, another folder on the Mac's internal SSD, or a cloud-synced copy. Confirm its make/model, filesystem, encryption, access and offline storage privately before any operation. Keep the recovery secret separate from each medium. No second medium is selected or inspected in F1E.

## 3. Key parameter proposal for Approval B

Approval A selected the Option A identity model only. The following candidate values are to be confirmed by the Owner immediately before the separately authorized Approval B operation:

| Item | Candidate | Status / condition |
|---|---|---|
| Target | Serea Private Beta only | Follows Approval A. |
| Application ID | `app.serea.android.privatebeta` | Fixed target. |
| Keystore | PKCS#12 (`.p12`/`.pfx`) | Candidate; Owner confirms. |
| Key algorithm / size | RSA / 3072-bit | Preferred candidate; not final. RSA 2048-bit may be considered only by Owner if compatibility/policy concern arises. |
| Certificate signature | `SHA256withRSA` | Candidate; Owner confirms JDK 17 `keytool` value. |
| Validity | 25 years or more | Candidate; exact number of days is Owner-confirmed after rechecking official Android guidance and policy. Do not infer exact day count by calculation in advance. |
| Alias | `serea-private-beta` | Candidate, dedicated and unique. |
| Subject | Minimum non-identifying attributes | Owner chooses locally; no account/address details in chat or repo. |
| Host / Java | Owner-managed Mac; Temurin JDK 17 `keytool` | Explicit executable selection and version check required. |
| Custodian | Owner only | No agent access to key, passwords, storage locations or recovery secrets. |

The JDK 17 `keytool` manual documents `-genkeypair`, RSA `-keysize`, `-sigalg`, `-validity`, `-storetype`, prompts for omitted passwords and the PKCS12 store type. Choose a validity value in days only after Owner confirms the desired end date and rechecks current official guidance. RSA 3072 is **not yet verified for a real signed APK on API 26**: no APK signature/API 26 verification has run. Generating a key alone cannot establish APK signature-scheme compatibility or update compatibility.

## 4. Existing installs, update chain and release version

- Pixel 7a status remains `OWNER_REPORTED_NOT_INSTALLED` for package `app.serea.android.privatebeta`; agent/ADB check `NOT_RUN`.
- Other devices/users/work profiles, past formal APKs/signers, release/version allocations and app-data continuity remain unknown. No device connection, APK/file search or user-data inspection was performed.
- If Owner may have installed a formal same-package APK, stop key generation until the prior signer, version and data continuity requirement are resolved through a separately authorized method. Never use uninstall or clear-data as a compatibility test.
- Initial versionCode proposal remains `VERSION_CODE_PROPOSED_43 / NOT_APPROVED`. Owner must approve a number and source/artifact scope separately before Approval C. Approval B does not approve versionCode, APK generation or signing.

## 5. Owner-only Approval B execution plan (future, not authorized now)

This preparation does **not** authorize any listed operation. Approval B remains `NOT_AUTHORIZED`; no keystore, image, backup or restore check may be performed until the Owner explicitly approves the exact operation scope. No APK/test signing is part of B.

### Before the approval

1. Owner resolves the prior Private Beta installation/APK/version/data-continuity question. If a prior official APK may exist and its signer/update chain is unresolved, stop.
2. Owner confirms final parameters in §3 and confirms the exact PKCS#12 alias, minimal Subject and validity-days value privately.
3. Owner confirms the Mac is trusted and controlled; identifies the explicit non-secret path to the approved Temurin JDK 17 installation (for example, an Owner-defined `JDK17_HOME`) and verifies that installation's `bin/java -version` reports Temurin 17.0.21+9 (or an Owner-approved JDK 17 patch) and its `bin/keytool -help` is from the same installation. These path references are not passwords; do not rely on default PATH Java.
4. Before generating anything, Owner selects and confirms the primary keystore location is encrypted at rest, outside repository/worktrees/build/sync/shared locations, and has restrictive Owner-only parent-directory access. If that protection cannot be confirmed, stop before running `keytool`. Owner also rechecks current Apple Disk Utility/`hdiutil`, Oracle JDK 17 `keytool`, and Android signing documentation. Confirm SSD make/model and encryption status without recording serial/UUID; confirm filesystem and sufficient free space in the UI. Since F1E could not establish hardware/volume encryption, do not proceed with direct SSD storage. The planned image itself must be explicitly AES-256 encrypted.
5. Owner selects an independent second physical medium and its encryption/custody plan. If unavailable, record `BACKUP_2_PENDING_OWNER`; do not represent a single SSD as two backups.
6. Owner provides a new explicit Approval B scope naming the operations permitted: key generation, exact primary-storage scope, creation of the AES-256 encrypted sparse-bundle on the confirmed external SSD, second-media backup creation, and which non-signing restore/read checks are allowed. No approval for the encrypted image or backup should be inferred from the SSD candidate designation.

### Owner actions after that exact Approval B

1. On the Owner-managed Mac in a trusted local terminal, use only the approved JDK 17 `keytool` binary. Before invoking it, confirm the selected absolute primary location is encrypted at rest, outside repository/worktrees/build/sync locations, and its parent directory is already Owner-restricted. Create the new PKCS#12 key interactively at that Owner-chosen path. The command template in [F1A §3](h4-f1a-owner-private-signing-and-apk-verification-runbook.md#3-owner-only-key-generation-and-custody-future-after-approval) contains candidate RSA/alias/signature settings; replace the validity placeholder and alias only with the Owner-approved values. Do not pass passwords as arguments, shell variables, environment variables, logs or files. Do not paste real paths/Subject/passwords into chat.
2. Restrict the primary keystore file and parent directory to Owner access using Finder/Get Info or an Owner-verified macOS permission method. Keep it off source, CI, shared folders and cloud sync.
3. In Disk Utility, use **File > New Image > Blank Image**. Save a **Sparse Bundle Image** on the already confirmed ExFAT volume, choose a bounded Owner-selected size (for example, 1 GB virtual), set the image's internal filesystem to APFS, choose **AES-256** in Encryption, select a read/write image, and use an Owner-managed password prompt. Do not select “Image from Device,” “Erase,” “Restore,” partition/volume changes or whole-drive encryption. If the image UI lacks AES-256 or proposes formatting/changing the existing SSD, stop without writing.
4. Unlock/mount the new image, copy the keystore into it as backup 1, then eject the image. Do not leave the image mounted when the SSD is offline. Keep the image password separate from the keystore password and do not save it in the image, on the SSD, in Keychain without a separate policy decision, or in the repository.
5. Create backup 2 on the separately selected, independently encrypted physical medium. Verify its device/failure-domain separation and encryption before copying. Do not count a second partition/copy on either device as independent.
6. For each primary/backup copy, use the explicit JDK 17 `keytool -list -v -storetype PKCS12 -keystore <Owner-private-keystore-path> -alias <Owner-approved-alias>` with interactive password prompts only. Owner supplies the approved alias and private path locally. Confirm locally that the entry is a `PrivateKeyEntry` and that the displayed public certificate Subject, RSA bit length, validity and SHA-256 fingerprint match the Owner's actual primary certificate and selected parameters. Compare the public certificate identity across the primary and each backup locally; a displayed certificate fingerprint is not a pre-approved value. Keep command output private. Actual fingerprints, file hashes, paths, subject details and password values stay in Owner custody and must not be sent or committed.
7. If separately authorized, perform a recovery-read check by copying a backup to a temporary Owner-controlled protected location, then use `keytool -list -v` on that copy and compare its public certificate with the primary. Mounting/listing the image in place is an image/keystore-read check, not a restoration test. Neither successful listing nor a matching public certificate proves the private key can perform a signing operation or establishes end-to-end recovery; do not claim operational recovery verified under Approval B. Any cryptographic private-key use/signing test is outside B and needs separate explicit approval. Clean up temporary copies under Owner policy, then close/eject the image and disconnect the drive.
8. If any identity, encryption, path, capacity, mount, keytool, read, certificate-match or second-medium condition is unexpected, stop without retrying by changing disk configuration. Preserve existing files; do not erase/reformat/repair. Owner decides next action.
9. Report only a non-secret completion summary: approved parameters matched/not matched; primary created/not created; backup 1 and backup 2 integrity/read checks passed/failed/not run; recovery-read check result; any blocker. Do not include secret material, paths, serial/UUID/device IDs, certificate fingerprints or APK data.

## 6. Approval gates and prohibited actions

| Gate | State after F1E |
|---|---|
| Approval A | `APPROVED — 2026-11-21` |
| Approval B | `NOT_AUTHORIZED` |
| Approval C | `NOT_AUTHORIZED` |
| Approval D | `NOT_AUTHORIZED` |
| Approval E | `NOT_AUTHORIZED` |
| KIOXIA SSD image/key backup | Candidate only; `NOT_READY` pending identity/encryption confirmation and explicit B scope |
| Backup 2 | `BACKUP_2_PENDING_OWNER` |
| versionCode 43 | `VERSION_CODE_PROPOSED_43 / NOT_APPROVED` |
| Google Cloud/OAuth | `BLOCKED_BY_SIGNING_IDENTITY` |
| Real signed APK / certificate SHA-1 / SHA-256 | `NOT_VERIFIED` |

No key generation, disk image creation, SSD write, backup/copy, recovery-read check, APK/AAB, test signing, device/ADB, Google Cloud/OAuth/API, or build/CI gate operation was performed or authorized by F1E. A future keytool read/certificate comparison can check keystore readability and public identity but cannot establish signing usability or end-to-end recovery. This is a paper-only readiness record; do not proceed until Approval B and unresolved media/continuity questions are addressed.

## 7. Official references

Recheck current UI and documentation immediately before future Owner execution:

- Apple, [Create a disk image using Disk Utility on Mac](https://support.apple.com/guide/disk-utility/create-a-disk-image-dskutl11888/mac) — blank secure image, filesystem choice, sparse-bundle growth, AES encryption menu, and password loss warning.
- Apple, [Disk Utility User Guide](https://support.apple.com/guide/disk-utility/welcome/mac) — device encryption and disk-image topics. Whole-drive erase/reformat instructions are not authorization to apply them to this existing SSD.
- Oracle, [JDK 17 `keytool`](https://docs.oracle.com/en/java/javase/17/docs/specs/man/keytool.html).
- Android, [Sign your app](https://developer.android.com/studio/publish/app-signing) and [`apksigner`](https://developer.android.com/tools/apksigner).

Local `hdiutil create -help` (read-only help output only; no `create` operation was run) listed `AES-256` as recommended and listed sparsebundle creation types. This confirms available tool options on this Mac, not that the KIOXIA candidate accepts the image or that an image has been created.
