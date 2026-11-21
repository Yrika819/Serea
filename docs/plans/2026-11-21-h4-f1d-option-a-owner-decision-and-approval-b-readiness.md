# H4-F1D — Option A Owner Decision Record & Approval B Readiness

**Decision status:** `APPROVED` — Approval A only. **Approval B–E remain `NOT_AUTHORIZED`.**

**Decision date:** 2026-11-21 (JST)
**Decision authority:** Owner
**Target:** Serea Private Beta
**Application ID:** `app.serea.android.privatebeta`
**Current repository base reviewed:** `origin/main` HEAD `510559074b865f1deebee4ca6e2d7aa31ef314cc`, Tree `5a091147e3d35c07cf17513ab9baa2bac486a571`.

## 1. Formal Owner decision — Approval A

The Owner formally adopts **Option A: a fixed, Owner-managed signing certificate dedicated to Serea Private Beta** and `app.serea.android.privatebeta`.

- The Owner is the signing-identity custodian and is responsible for key generation, access, storage, backup and recovery.
- The identity is exclusive to Private Beta. Sharing its key or certificate with Public Beta or Production is prohibited. Public Beta and Production signing identities must remain separate.
- Gradle's unsigned build and APK signing remain separate operations. This decision does not change Gradle signing configuration, CI guards or signing boundaries.
- This decision is not Approval B and does not authorize key/keystore generation, inspection, copying, backup or restoration; APK/AAB generation or signing; installation or distribution; Google Cloud/OAuth; or any device operation.
- No signing certificate has been generated. Certificate SHA-1 and SHA-256 fingerprints do not yet exist and are `NOT_VERIFIED`. This record is not evidence of key generation.

**Decision provenance:** the Owner's explicit approval in the H4-F1D conversation on 2026-11-21. No separate signed approval document is represented as existing.

### Design candidates — not final key-parameter approval

The Owner-approved choice is the **Option A identity model**, not every implementation parameter. The following are the current preferred design candidates for the Owner's separate Approval B confirmation:

| Parameter | Preferred candidate | Approval B confirmation required |
|---|---|---|
| Keystore | PKCS#12 (`.p12`/`.pfx`) | Confirm exact format and tested JDK/tool support. |
| Key | RSA 3072-bit | Confirm final key length. If API 26+ compatibility is not established, pause and present RSA 2048-bit as an alternative; do not change it without Owner decision. |
| Certificate signature | `SHA256withRSA` | Confirm actual `keytool` value before generation. |
| Validity | At least 25 years | Confirm exact duration after checking current official Android guidance and Owner policy. |
| Alias | Dedicated Private Beta alias; runbook candidate `serea-private-beta` | Confirm final naming policy; do not reuse another identity's alias/key. |
| Certificate Subject | Minimum non-identifying subject attributes needed by the tool/policy | Owner selects exact attributes locally; do not collect or publish identity details. |
| Host/tool | Owner-managed Mac; explicit JDK 17 `keytool`; selected Android SDK Build Tools `apksigner` | Confirm exact host, JDK distribution/version, SDK Build Tools version, and API 26+ verification plan before execution. |
| Custody | Primary encrypted keystore plus two independently encrypted backups in separate failure domains | Confirm Owner-controlled locations and recovery arrangement privately; do not disclose paths or passwords. |

PKCS#12 and RSA/SHA256withRSA are candidate configuration, not a compatibility certification. Existing F1C reporting confirms tools were present but did not perform APK creation/signature checks on API 26. No API 26 signed-APK compatibility result is claimed. Current `minSdk` is 26 per the F1A/F1B records; use current `apksigner` Build Tools and validate the chosen signature scheme against API 26+ when a later separately authorized artifact stage occurs. Do not assume RSA 3072 is proven by the dry-run evidence.

## 2. Rationale and accepted risks

The Owner selects Option A to:

1. keep the Private Beta Application ID stable;
2. preserve a viable future APK update path;
3. keep the signing-certificate identity used for Android OAuth registration stable;
4. preserve risk separation from Production keys; and
5. establish Owner-controlled backup and recovery responsibility.

The decision accepts that the design has material residual risks: loss or corruption of the only usable key can prevent compatible updates; certificate/key mismatch can block updates and OAuth registration; an incompatible replacement can interrupt app-data continuity or require a new package/migration; and key disclosure can enable unauthorized signing and require incident response and potentially a new identity. Backups reduce but do not eliminate these risks. A certificate rotation or uninstall/reinstall is not assumed to preserve updates or app data.

## 3. Approval state and separate gates

| Gate | Current state | Boundary |
|---|---|---|
| Approval A — adopt Option A identity model | `APPROVED — 2026-11-21` | Decision recorded here; no key was generated. |
| Approval B — Owner generates, stores and backs up key | `NOT_AUTHORIZED` | Requires Owner's future explicit approval of final parameters and exact operation. Owner performs it; agent never receives secrets. |
| Approval C — generate exact unsigned APK | `NOT_AUTHORIZED` | Separate approval of source SHA, versionCode, build task, host and artifact scope. |
| Approval D — sign and verify exact APK | `NOT_AUTHORIZED` | Separate approval naming exact approved input and signing scope. |
| Approval E — device installation/update | `NOT_AUTHORIZED` | Separate device-, data- and action-specific approval. |
| Google Cloud/OAuth | `BLOCKED_BY_SIGNING_IDENTITY` | No Console/client/auth/network action; a verified signed APK and later separate approval are prerequisites. |
| Signed APK | `NOT_VERIFIED` | None generated or inspected in this phase. |
| Real APK certificate SHA-1 / SHA-256 | `NOT_VERIFIED` | No certificate exists yet. |
| First formal versionCode | `VERSION_CODE_PROPOSED_43 / NOT_APPROVED` | Separate Owner decision; not part of Approval B. |

Approval B does not approve C, D, E, versionCode 43, an APK, Cloud/OAuth, a release, or distribution. Approval C must not start until the Owner approves the formal versionCode, exact source SHA and artifact operation scope. Approval D requires the exact output of C. Approval E is separate.

## 4. H4-F1C evidence handoff — report-derived, not repository-verified

H4-F1C did not create a new PR. Its report is not represented here as a GitHub-managed formal record; the following are carried forward as facts reported for that F1C check, not re-executed or independently verified in F1D.

### Owner-reported installation status

- Package: `app.serea.android.privatebeta`.
- Owner reports that this package is not installed on the target Pixel 7a: `OWNER_REPORTED_NOT_INSTALLED`.
- Agent independent physical-device/ADB check: `NOT_RUN`.
- This report applies only to the Owner's statement about that Pixel 7a. It does not establish the state of other devices, users, work profiles, historical formal APKs, backups, or other copies of the app. Those remain unknown. No ADB connection, APK search, installation or data inspection was performed.

### Technical host/tool report

F1C reported the following on its own check date:

- Temurin JDK `17.0.21+9` and JDK 17 `keytool`;
- Android SDK on macOS, with Build Tools `35.0.0`, `36.0.0`, `36.1.0`, and `37.0.0`;
- `zipalign`, `apksigner`, and `apkanalyzer`;
- Gradle Wrapper `8.14.6`;
- Private Beta Gradle dry-run success using explicit JDK 17, explicit SDK and an independent cache.

These are historical report results, not F1D host verification or execution authorization. No real APK was generated or signed; no signature was verified on API 26. F1D does not repeat these checks.

## 5. Approval B Owner-only preflight checklist

Use, do not duplicate, the existing [F1A Owner signing and APK verification runbook](h4-f1a-owner-private-signing-and-apk-verification-runbook.md) and [F1B readiness report](h4-f1b-private-signing-identity-apk-readiness.md). This checklist prepares a future Owner action only; every unchecked item below is for the Owner to resolve privately before requesting Approval B.

### 5.1 Existing installations, artifacts, version and data continuity

- [ ] Owner privately lists every device/account/profile that may have run `app.serea.android.privatebeta`, including personal devices, other users, work profiles and backups. Do not infer these are empty from the Pixel 7a report.
- [ ] Determine from Owner-held records whether a same-package formal installation or historical formal APK exists; keep the actual APK private.
- [ ] If a prior APK exists, obtain a separately scoped authorization and approved method to inspect only its package, versionCode and signer identity. Do not search devices or files as part of this decision record.
- [ ] Reconcile any formal versionCode allocations/releases and any candidate currently installed. Historical test code `42` and source fallback `2` are not official version records.
- [ ] Decide whether existing private app data must be preserved and identify which installation/update path must remain viable.
- [ ] If a prior APK has another/different signer or uncertainty remains, **stop** before key generation or installation. Do not uninstall, clear data, generate a replacement to test, or claim update compatibility. Resolve continuity in a separate Owner-approved action.
- [ ] Keep first formal `versionCode = 43` as `VERSION_CODE_PROPOSED_43 / NOT_APPROVED`. Owner must decide the formal number and reconcile the complete known allocation/install history before Approval C.

### 5.2 Final key parameters and compatibility

- [ ] Confirm PKCS#12 format.
- [ ] Confirm RSA key length. Preferred candidate is 3072-bit. If compatibility or policy is uncertain, pause and review RSA 2048-bit as a proposal; agent does not select or change it.
- [ ] Confirm `SHA256withRSA` certificate signature.
- [ ] Confirm exact validity period (candidate: at least 25 years) against then-current official guidance and Owner policy.
- [ ] Confirm unique alias naming policy, without reusing a Public Beta, Production, personal or other key identity.
- [ ] Select only the minimum required Subject attributes locally; do not put personal identity data in public records.
- [ ] Confirm API 26+ compatibility plan and selected `apksigner` scheme/tool version. F1C dry-run does not establish signed APK/API 26 verification.
- [ ] Confirm Owner-managed Mac, explicit JDK 17 binary/distribution/version, JDK 17 `keytool`, Android SDK path and chosen Build Tools version. Do not rely on PATH defaults.
- [ ] Recheck official Android and JDK tool documentation immediately before execution. If the selected key format, key size, subject or tool behavior differs from this plan, stop and get Owner approval before generating.

### 5.3 Custody, backups and recovery

- [ ] Generate only on an Owner-managed, trusted Mac; select and verify an absolute destination outside repository/worktrees, build directories, shared folders and general cloud-sync folders.
- [ ] Confirm the keystore's own password protection and use interactive prompts. The Owner manages passwords using a separately protected method.
- [ ] Confirm storage media encryption and access controls.
- [ ] Prepare two separately encrypted backup copies in independent failure domains; do not keep both copies or their recovery credentials together.
- [ ] Confirm a private recovery inventory and a recovery process. Do not place actual paths, passwords, aliases/fingerprints or personal subject details in this repository or chat.
- [ ] Decide the separately approved restore-integrity/recovery test. Do not create backups or run a restore test until explicitly included in Approval B's operation scope.
- [ ] Confirm stop/incident actions for loss, corruption, suspected leak or mismatch: cease signing and downstream OAuth/release work; preserve evidence; Owner decides recovery/incident response. Never send key material to the agent.

### 5.4 Owner execution sequence after separate Approval B

This is a future, Owner-only sequence, not authorized for execution in F1D:

1. Owner selects the operation Mac and opens a trusted local terminal. Explicitly select the approved JDK 17 `keytool` executable; verify its version and verify it is JDK 17 before proceeding. Do not use the agent, repository environment, CI or a shared machine.
2. Review the prior-installation/artifact continuity checklist, final parameters, source-independent storage path requirements, encryption, two backup domains, password plan and exact Approval B scope. Any unknown/conflicting result is a stop condition.
3. Recheck current official `keytool` and Android signing guidance. Owner confirms final alias, minimum Subject, key size, signature algorithm and exact validity duration. Do not paste actual Subject values or private path into chat.
4. Following separate explicit Approval B, Owner runs the interactive `keytool -genkeypair` procedure in F1A's §3 with the approved absolute keystore path outside the repository. Its current preferred candidate command uses `-storetype PKCS12`, `-keyalg RSA`, `-keysize 3072`, `-sigalg SHA256withRSA`, and the Owner-approved validity value; F1A documents interactive prompting and must be updated to the Owner-finalized values before execution if Approval B chooses otherwise. Passwords must not appear as arguments, environment variables, logs, source, CI, screenshots, chat or GitHub content.
5. Owner completes the two encrypted backups in separate failure domains and verifies each copy's integrity without exposing its contents or credentials. Actual backup creation and restore verification happen only inside the explicitly approved B scope.
6. Owner checks the public certificate from the newly generated identity locally, confirms its Subject/validity/public-key parameters against the approved values, and compares its SHA-256/SHA-1 fingerprints between the primary and backup copies. An Approval-B-scoped restoration check may restore a backup copy in Owner custody and compare the restored public certificate/integrity with the primary only if that exact check is included in B's approved scope; it must not sign or create any APK/test artifact. Any test signing or artifact generation requires its own explicit approval and is not authorized by B here. Do not export or transmit the private key. Keep fingerprint values private; report only a non-secret match/pass result.
7. Owner sends only a non-secret completion statement: operation completed/not completed, approved parameter confirmation, backup/recovery checks completed/not completed, and any stop condition. Do not send passwords, private paths, keystore, certificate fingerprints, screenshots or personal Subject values to the agent.
8. If a prompt, tool version, path, key format, Subject, algorithm, validity, backup integrity or certificate comparison is unexpected; a secret is exposed; or any prior-installation conflict appears, stop without retrying or improvising. Preserve the private state for Owner assessment. Do not create/sign a test APK, continue to C/D/E, Cloud or device operations.

## 6. Related document state and scope

- F1A and F1B retain their dated audit/CI snapshots as historical evidence; this record is the current Owner decision for Approval A and supersedes their then-pending state only for that decision.
- No F1C report or validation claim is upgraded to a formal PR/CI record by being summarized here.
- The Gradle unsigned-build/signing boundary and Public/Production key separation remain unchanged.
- No production code, build configuration, CI protection, OAuth/HTTP gate or key material is changed or accessed by this documentation decision.

## 7. Authority and record integrity

This decision record was prepared to capture the Owner's explicit conversation approval dated 2026-11-21 JST. It records that approval only; it is not a signed approval artifact, proof of possession of a private key, proof of APK signing, or authorization for Approval B–E. The next required Owner judgment is Approval B: confirm final parameters, resolve any continuity blocker sufficiently to make an informed decision, and explicitly authorize the exact Owner-performed generation/custody/backup scope. VersionCode and APK operations remain separate decisions.

## Official references for future Owner execution

Recheck the current versions before any later operation; these references do not authorize execution:

- Oracle JDK 17 [`keytool` command](https://docs.oracle.com/en/java/javase/17/docs/specs/man/keytool.html) — `-genkeypair`, `-keysize`, `-sigalg`, `-validity`, `-keystore`, `-storetype`, prompting, and password handling.
- Android [Sign your app](https://developer.android.com/studio/publish/app-signing) — app update identity, key custody, and recommended minimum 25-year certificate validity.
- Android [`apksigner`](https://developer.android.com/tools/apksigner) — APK signature and verification tooling; choose and validate the actual scheme for the supported API range during a separately approved artifact operation.
