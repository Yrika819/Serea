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

## H4-F1F addendum — media finalization and Approval B execution gate (2026-11-21 JST)

This dated addendum preserves the F1E observations above as historical. It records a new read-only macOS snapshot and a future-owner approval package only. F1F performed no disk writes, image creation/mount, key operation, backup, APK/device operation, or Cloud action.

### A. New read-only external SSD snapshot

The external physical-disk list showed a 1.0 TB USB SSD reporting model `EXCERIA PLUS` and a mounted volume named `KIOXIA`. The metadata was read-only; no volume contents were examined. Current observed values:

| Property | 2026-11-21 observation | Interpretation |
|---|---|---|
| Physical device | External, USB, solid-state; model string `EXCERIA PLUS`; approximately 1.0 TB | Consistent with the Owner-described candidate, but not independent proof of manufacturer/ownership. |
| Mounted volume | Label `KIOXIA`; mounted and not read-only | A label alone does not identify the physical product. |
| Filesystem | `File System Personality: ExFAT`; partition-type metadata says `Windows_NTFS` | ExFAT is the mounted filesystem; the partition-type label is not evidence the volume is NTFS. |
| Used / free | Approximately 536.3 GB used / 463.9 GB free | Current snapshot only; recheck in the Owner's UI immediately before any separately approved write. |
| Encryption | ExFAT does not provide native volume encryption; hardware encryption is not established by the inspected metadata | Keep the whole-volume state `UNVERIFIED`; do not store key material directly on it. |

The F1E snapshot (approximately 740.4 GB used / 259.7 GB free) remains a historical observation and is not overwritten by this newer reading. macOS did not expose a manufacturer attribute sufficient to authenticate the physical product. Gate 1 therefore remains `OWNER_PHYSICAL_CONFIRMATION_REQUIRED`: Owner must compare the physical product/label and connector to the intended device and confirm the on-screen model/capacity before selecting it in Disk Utility. No serial, UUID, transient device node, mount path, or other external disk's metadata was recorded. The Owner did not designate a second medium; `BACKUP_2_PENDING_OWNER` remains.

### B. Encrypted-image design and ExFAT assessment

An unencrypted ExFAT volume is intended to support an **encrypted disk-image container** as a new file/package without changing its filesystem; this combination remains an untested candidate on this particular SSD until separately approved Gate 2 succeeds. When the image is closed/ejected, its contents remain encrypted; when mounted and unlocked, the image contents are accessible as plaintext to the logged-in system. AES-256 image encryption therefore protects a lost/offline container, not a mounted session, compromised Mac, weak/reused password, or password stored alongside/in Keychain. Use a strong unique image password, enter it only in the UI prompt, do not save it to Keychain, and keep it separately in Owner-controlled password custody. No password or actual storage location belongs in this repository.

| Disk Utility image format | Storage behavior | F1F assessment |
|---|---|---|
| Sparse Bundle Image (`UDSB`) | Read/write; grows with stored data in multiple band files | **Preferred candidate, not yet tested.** Sparse Bundle is listed in Apple's general blank-image format choices and local `hdiutil` help. Apple describes it as a read/write image that grows across multiple files; this does not mean Apple specifically certifies the encrypted sparse-bundle/APFS/ExFAT combination or prescribes it in the secure-image steps. It avoids allocating the entire virtual size immediately. Its many-file bundle needs reliable ExFAT directory/file operations and safe ejects. |
| Apple Sparse Image (`ASIF`) | Modern sparse read/write image; allocated space follows stored data | Apple documents it, but this workflow does not select it because F1E's local `hdiutil` help did not list it and it has not been exercised here. Do not silently substitute it. |
| Sparse Image (`UDSP`) | Sparse read/write image represented as an image file | Could be more compact than fixed-size read/write, but unlike the bundle uses a single image file; no evidence here establishes it as safer on this exact ExFAT device. Not the selected format. |
| Read/write (`UDRW`) | Read/write image with its virtual capacity allocated as a regular image | Simpler single-file structure but consumes the full chosen size and is not the preferred growing format. Do not fall back without fresh Owner review. |

Apple's guidance recommends APFS inside a disk image used with macOS 10.13 or later and identifies sparse bundles as growing, multi-file read/write images. The Owner-managed Mac use case calls for APFS inside the image. ExFAT's lack of APFS semantics is not itself a reason to rule out a container file holding an APFS image, but this particular combination is **not established** by Apple documentation and must be treated as a candidate pending Gate 2. Gate 2's empty-image create/eject/reopen test checks basic compatibility; it is not a long-term reliability test or backup-integrity proof. The current 463.9 GB free snapshot is ample for a 1 GB *virtual* image as displayed by Disk Utility; allocated space should grow with content, but actual units, free space, and allocation are time-varying and must be checked in the UI.

Do not enable whole-SSD encryption, erase/reformat, repartition, convert filesystems, or use “Image from Device.” The container is an additional file/package; it is not a reason to modify existing data. Unclean removal or filesystem/media failure can still corrupt new container files or other data. Owner must confirm important existing SSD data is backed up and the disk is healthy before Gate 2; if errors, disconnects, or repair prompts occur, cancel and stop.

### C. Owner approval package — Gate 1 through Gate 4

These gates are sequential and separately authorized. Passing one does not approve the next. F1F provides no write authorization.

#### Gate 1 — confirm the physical medium

Owner compares the physical product/label and connector with the intended 1 TB KIOXIA SSD, then confirms the current Disk Utility device model/capacity and mounted ExFAT volume. Check available capacity and confirm existing data is protected by an independent backup. Do not browse existing files, change settings, test-write, or treat the volume label alone as identity. If any attribute mismatches, the disk is unhealthy, important data is not backed up, or identity is ambiguous, stop. Gate 1 completion is an Owner confirmation, not an agent claim.

#### Gate 2 — separately approve and create an empty encrypted image only

**Required limited Owner authorization:** explicitly approve creation, automatic initial mount, metadata/password-prompt verification, eject/reopen verification, and final eject of one **empty** image on the visually confirmed KIOXIA ExFAT volume. The approval should specify a virtual size of **1 GB as displayed by Disk Utility** (do not silently substitute GiB), APFS (non-case-sensitive), single GUID partition, AES-256 encryption, Sparse Bundle Image (`UDSB`), and that no key or other file is to be copied into it. This is a media-preparation authorization, **not Approval B**.

**Owner-run Disk Utility sequence after that separate authorization:**

1. Reconfirm the physical SSD in Gate 1 and select the existing mounted ExFAT volume as the save destination. Choose a new Owner-selected image filename that does not already exist; never overwrite an existing item. Do not include personal paths or device IDs in any report.
2. Open Disk Utility and choose **File > New Image > Blank Image**. Select the verified ExFAT volume as the destination. Enter a unique image filename and an innocuous internal volume name.
3. Set **Size: 1 GB** using Disk Utility's displayed unit (virtual capacity; do not substitute GiB), **Format: APFS**, **Partitions: Single partition - GUID Partition Map**, **Encryption: AES-256**, and **Image Format: Sparse Bundle Image**. Use the local password prompt and do not save the password in Keychain. If AES-256 or Sparse Bundle is unavailable, or Disk Utility proposes erasing/reformatting/changing the physical volume, cancel without writing and stop for Owner review; do not choose a substitute format.
4. Save the blank image. Disk Utility normally mounts it after creation. Confirm in Disk Utility that the mounted inner volume is APFS and has the expected bounded capacity. Do not copy any file into it. Keep any displayed paths/output private.
5. Eject the image. Use the read-only `hdiutil isencrypted -verbose <Owner-selected-image>` and `hdiutil imageinfo -verbose <Owner-selected-image>` locally to inspect encryption and image-format metadata. Confirm `isencrypted` reports the image encrypted and `imageinfo` identifies its sparse-bundle format. Local `hdiutil` documentation says `isencrypted` prints additional details for encrypted images, but does not guarantee that every output version identifies the cipher by name: if AES-256 is not explicitly reported, record the UI's AES-256 selection but mark the cipher `NOT_INDEPENDENTLY_VERIFIED`, stop, and do not use the image for key data. Keep output private. Reopen the image from Finder without Keychain assistance; confirm this reopen prompts for the image password and the Owner can unlock and mount it. That prompt shows it did not silently unlock in this check; it does not prove the password is absent from every Keychain or establish content integrity/recoverability.
6. Finally eject the image, confirm it is no longer mounted, then safely eject the external SSD. Keep the container offline; do not add it to Login Items, scripts, or an automounter. The password-prompt observation from step 5 is the only Keychain-related check; do not claim it proves the credential is absent from Keychain.

**Gate 2 success evidence:** Owner privately confirms the image was newly created on the physically checked SSD, requested settings were selected, metadata reports encryption and sparse-bundle format, the image's configured AES-256 setting is recorded from Disk Utility, the inner APFS volume mounted at the expected displayed 1 GB capacity, reopening required the password and succeeded, and both image and physical volume were ejected. If `isencrypted` does not identify AES-256, report `NOT_INDEPENDENTLY_VERIFIED`; do not claim that the cipher was independently confirmed or use the image for key data pending Owner-approved resolution. Public report contains only pass/fail/not-run, with no path, password, serial, UUID or device identifier. No existing file is intentionally modified; only the new uniquely named image bundle is created. This does not establish media reliability under failure or recovery of future key data.

**Stop/failed-create behavior:** cancel on identity/space mismatch, warning to erase/reformat, UI option mismatch, unexpected auto-unlock, metadata inconsistency, mount/password failure, filesystem errors, disconnect, or any existing-name collision. Do not retry with changed layout/format, repair, delete partial files, overwrite, or move existing data. Leave any partial new item untouched, eject/unmount only if the UI safely allows it, and ask Owner to decide next steps. A cleanup/removal operation requires separate authorization.

#### Gate 3 — choose a second independent backup medium

`BACKUP_2_PENDING_OWNER`. Recommended: a separate, quality external SSD or dependable removable USB medium sized for small encrypted files, with a separate AES-256 encrypted container or a dedicated encrypted macOS-compatible volume, chosen by the Owner. For long-term storage, prefer a reputable SSD over an inexpensive generic thumb drive; flash retention, controller failure and loss remain risks. Keep it physically separate from the KIOXIA device (ideally a separate storage location), offline/ejected, with the recovery password managed separately. A second partition on KIOXIA, Mac internal disk folder, or same physical device is not independent. No unspecified personal medium was examined. Gate 3 completion requires Owner designation and a separate read-only metadata check only if Owner identifies the device; no formatting or encryption change without its own scope.

#### Gate 4 — Approval B is still a separate future decision

Approval B remains `NOT_AUTHORIZED`. It is not granted by Gate 2, Gate 3, or this document. Before considering B, Owner must:

- Resolve whether any other device, Android user/work profile, prior formal same-package APK, signer, versionCode, or app data requires update continuity. Pixel 7a remains `OWNER_REPORTED_NOT_INSTALLED`; agent/ADB confirmation remains `NOT_RUN`. If prior APK/data continuity may matter, stop until resolved through a separately approved method.
- Designate the second physical backup medium and accept custody, password-separation, offline-storage, and recovery-read limitations.
- Confirm the final key parameters: Option A; PKCS#12; RSA 3072-bit candidate; `SHA256withRSA`; at least 25 years, with exact validity-days and minimal Subject selected privately; alias `serea-private-beta`; Owner-controlled Mac and Temurin JDK 17. API 26+ real APK-signature compatibility remains **unverified**. No key is generated by this gate.
- Explicitly state that Approval B authorizes Owner-only key generation at the approved encrypted/restricted primary location and copy of the resulting keystore into the two approved encrypted independent backups, plus exactly which non-signing `keytool -list` checks may be done. Keytool read/certificate comparison does not prove signing usability or end-to-end recovery.
- Keep `VERSION_CODE_PROPOSED_43 / NOT_APPROVED`; APK build/signing/version adoption remain separate; Google Cloud/OAuth remains `BLOCKED_BY_SIGNING_IDENTITY`; certificate fingerprints and signed APK remain `NOT_VERIFIED`.

### D. Current gate state after F1F

| Gate / decision | State |
|---|---|
| Gate 1 physical media confirmation | `OWNER_PHYSICAL_CONFIRMATION_REQUIRED` |
| Gate 2 empty AES-256 image preparation | `NOT_AUTHORIZED — OWNER_APPROVAL_REQUIRED` |
| Gate 3 second independent medium | `BACKUP_2_PENDING_OWNER` |
| Approval A | `APPROVED — 2026-11-21` |
| Approval B | `NOT_AUTHORIZED` |
| Approval C / D / E | `NOT_AUTHORIZED` |
| Pixel 7a | `OWNER_REPORTED_NOT_INSTALLED`; independent check `NOT_RUN` |
| Other installs, old APK/signer, data continuity | `UNCONFIRMED` |
| versionCode | `VERSION_CODE_PROPOSED_43 / NOT_APPROVED` |
| Google Cloud/OAuth | `BLOCKED_BY_SIGNING_IDENTITY` |
| Real APK and certificate SHA-1/SHA-256 | `NOT_VERIFIED` |

### E. Official references checked for F1F

- Apple, [Create a disk image using Disk Utility on Mac](https://support.apple.com/guide/disk-utility/create-a-disk-image-dskutl11888/mac): distinguishes the general blank-image format choices (including sparse bundle) from the secure-image procedure (blank read/write image with encryption). The selected sparse-bundle/APFS/AES-256 combination is this runbook's untested candidate, not an Apple-certified ExFAT recipe; the guide recommends APFS for modern macOS use and notes new images are mounted.
- Apple, [File system formats available in Disk Utility](https://support.apple.com/guide/disk-utility/file-system-formats-dsku19ed921c/mac): APFS is the macOS 10.13+ filesystem and ExFAT is a Windows-compatible filesystem; this does not establish container-on-ExFAT compatibility.
- Apple, [Disk Utility User Guide](https://support.apple.com/guide/disk-utility/welcome/mac): device/file-system, image and eject topics. Apple's erase/reformat procedure is explicitly not part of these gates.
- Oracle, [JDK 17 `keytool` manual](https://docs.oracle.com/en/java/javase/17/docs/specs/man/keytool.html): `-genkeypair`, `-keyalg`, `-keysize`, `-sigalg`, `-validity`, `-storetype`, `-list -v`; omitted passwords prompt interactively; RSA 3072 defaults to `SHA256withRSA` but explicit values remain subject to Owner approval.
- Android, [Sign your app](https://developer.android.com/studio/publish/app-signing): at least 25-year validity guidance and same-certificate update continuity. It does not verify this key's compatibility on API 26.

The local macOS `hdiutil create -help`, `hdiutil imageinfo -help`, and `hdiutil isencrypted -help` were read only. Help lists `SPARSEBUNDLE`/`UDSB`, `SPARSE`/`UDSP`, `UDRW`, and `AES-256` (recommended), and documents read-only metadata/encryption inspection. No create/attach/convert command was run. Apple documentation and help establish available formats, not successful operation on this particular ExFAT SSD. Gate 2 remains a separately approved empty-image compatibility check.

## H4-F1F-R1 addendum — Owner physical confirmation and Gate 1 status (2026-11-21 JST)

This addendum records the Owner's explicit physical-model confirmation in the current conversation. It supplements the F1F observations and does not alter or delete the F1E/F1F historical snapshots.

### Owner confirmation

- Target: Owner-described KIOXIA 1 TB external SSD.
- Owner reports that the physical SSD enclosure bears the model marking `EXCERIA PLUS`.
- The F1F macOS snapshot reported model `EXCERIA PLUS`, volume label `KIOXIA`, approximately 1 TB capacity, and ExFAT filesystem.
- Owner's physical model observation matches the macOS model display: `OWNER_PHYSICAL_MODEL_MATCH_CONFIRMED`.

This is Owner confirmation of the model marking/display match only. It is not manufacturer authenticity certification, serial-number verification, a device-health test, or independent agent inspection of the product. No serial number or persistent device identifier is requested or recorded.

### Gate 1 result and remaining preconditions

**Gate 1 model/volume identification: `OWNER_CONFIRMED`.** This resolves the earlier `OWNER_PHYSICAL_CONFIRMATION_REQUIRED` status for the physical model/display match only. The F1E and F1F snapshots remain historical records; the approximate 463.9 GB free value is from the F1F snapshot and is not a current free-space guarantee.

Still unconfirmed before any Gate 2 write:

- Whether existing SSD data has an independent backup and is protected against loss.
- Current SSD health; no health test was performed, and SMART was not supported by the F1F metadata query.
- Free space at the time Gate 2 might later be separately approved.
- Whether an encrypted sparse-bundle can be created, mounted, verified and safely ejected on this specific ExFAT volume.
- Whether the new empty encrypted container will operate normally; no image exists and no compatibility test has been performed.

**Do not execute Gate 2 until the Owner confirms that important existing SSD data is independently backed up and protected, and the Owner separately approves the exact Gate 2 scope.** If the medium reports errors, disconnects, or a repair/format prompt, stop without retrying or changing it.

### Gate 2 limited approval package (still not authorized)

The proposed scope remains one empty disk image on the Owner-confirmed KIOXIA SSD, preserving the existing ExFAT filesystem:

- Disk Utility displayed virtual size: `1 GB`.
- Image format: Sparse Bundle Image (`UDSB`).
- Inner filesystem: APFS (non-case-sensitive).
- Partition map: Single partition - GUID Partition Map.
- Encryption selection: AES-256; Owner enters the password locally and does not save it in Keychain.
- After separate authorization: create and initial auto-mount, inspect image/encryption metadata, eject, re-open with Owner password entry, verify, and final eject of both image and SSD.
- No keystore, private key, other file, APK, or password is placed in or copied to the image.

The image is only usable for future key data if encryption is independently confirmed as AES-256, the basic mount/reopen checks succeed, and the Owner separately decides to proceed. If local metadata does not identify the cipher, record it `NOT_INDEPENDENTLY_VERIFIED`, do not place key material in the image, and stop for Owner decision. Do not erase/reformat/repartition the SSD, inspect existing files, remove a partial image, or retry using another format.

### Gate and approval state after F1F-R1

| Gate / decision | State |
|---|---|
| Approval A — Option A formal adoption | `APPROVED` |
| Gate 1 — physical model/display match | `OWNER_CONFIRMED` (`OWNER_PHYSICAL_MODEL_MATCH_CONFIRMED`) |
| Gate 2 — empty image creation / mount / verification | `NOT_AUTHORIZED` |
| Gate 3 — second independent medium | `PENDING_OWNER` (`BACKUP_2_PENDING_OWNER`) |
| Approval B — private key generation and backups | `NOT_AUTHORIZED` |
| Approval C — unsigned APK | `NOT_AUTHORIZED` |
| Approval D — APK signing | `NOT_AUTHORIZED` |
| Approval E — installation | `NOT_AUTHORIZED` |
| versionCode 43 | `VERSION_CODE_PROPOSED_43 / NOT_APPROVED` |
| Pixel 7a installation | `OWNER_REPORTED_NOT_INSTALLED`; independent device check `NOT_RUN` |
| Other devices/profiles, previous formal APK/signer, data continuity | `UNCONFIRMED` |
| Google Cloud/OAuth | `BLOCKED_BY_SIGNING_IDENTITY` |
| Real APK and certificate SHA-1/SHA-256 | `NOT_VERIFIED` |

Gate 2 is independent of Approval B and may be considered as a media-only preparation after the existing-data protection question is answered. Gate 2 approval does not authorize Approval B. Approval B remains blocked from GO until Gate 2 is completed, a physically independent second medium is selected and prepared, and prior-installation/data-continuity questions are resolved.
