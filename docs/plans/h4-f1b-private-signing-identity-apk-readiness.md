# H4-F1B — Private Beta Signing Identity & APK Readiness

**State:** `F1B_PARTIAL` — documentation/audit preparation only. Based on `origin/main` HEAD `846d6693248a09656beb2d73ff693c31e474ef29`, Tree `51a8c10c4e8ff9a4bad17ded714491c9f51fb9b2`. No product code, Gradle setting, signing gate, OAuth gate, or HTTP gate was changed. No private file, key, keystore, APK, device, or Google Cloud Console was inspected or used. Formal signing identity adoption, versionCode, and all key/artifact/device actions remain Owner decisions.

## 1. Decision summary

- **Preferred proposal:** Option A, one fixed Owner-managed Private Beta-only signing identity for `app.serea.android.privatebeta`. This remains a proposal; formal adoption is **`PENDING_OWNER`**.
- **Existing installed Private APK:** **`NOT_VERIFIED`**. No Owner-supplied APK or installation evidence was available or accessed. Historical repo evidence contains disposable/test APK generation only, not proof of a formally signed APK installed on an Owner device. Do not generate a new certificate or install a replacement until the Owner establishes whether an earlier same-package APK is installed and, if so, independently verifies its signer and version under separately authorized artifact/device access.
- **First formal versionCode:** **`PENDING_OWNER`**. Source default `2` and the Gradle guard's accepted range are not release approval. Historical disposable values do not establish an installed version.
- **Unsigned build path:** Source declares `assemblePrivateBeta`, `packagePrivateBeta`, and `bundlePrivateBeta` as guarded Private packaging tasks. Both tested `--dry-run` attempts (`assemblePrivateBeta` and `packagePrivateBeta`) were blocked before task graph output: first by default Oracle JDK `26.0.2.1`, then with the documented Temurin JDK 17 by unavailable SDK configuration. Neither packaging task executed; no APK was generated.
- **APK output path / installed SDK `zipalign`, `apksigner`, `apkanalyzer`:** `NOT_VERIFIED` in this worktree. The SDK tools were not on PATH; the isolated worktree has no `android/local.properties`, and no user SDK configuration was searched or copied.
- **Signing/CI boundary:** Keep Gradle Beta/Private Beta signing disabled. Keep Private packaging blocked in CI. Existing repository signing utilities and historical disposable signing evidence are not this Owner-managed formal signing identity and are not authorized for this operation.
- **Google Cloud / OAuth:** Not accessed, not configured, and not authorized by this phase. No `OWNER_ACTION_REQUIRED — GOOGLE_CLOUD` handoff is due yet because the exact verified signed APK certificate SHA-1/SHA-256 do not exist.

## 2. Historical artifact, version and update evidence

### What repository records establish

| Question | Non-secret repository evidence | Finding |
|---|---|---|
| Was a formal Private Beta APK created? | F0/F1A readiness records explicitly say no formal signed APK was inspected; F1A did not execute APK packaging. H4-D2 reports describe fixture/disposable APKs and local validation, not a formally adopted Owner identity. | No formal APK creation established; existence outside the repository is unknown. `NOT_VERIFIED`. |
| Were APKs generated in development/testing? | `docs/plans/2026-11-17-h4-d2-source-integration-closure-report.md` records an unsigned Private APK produced for explicit versionCode `42`, plus disposable/test signing and variant verification. `docs/plans/2026-11-16-h4-d2-final-dynamic-acceptance-report.md` records unsigned current build outputs and disposable signing. | Yes, historical test/build artifacts existed. They are not evidence of an Owner-installed APK or official signer. Do not reuse their test signers as Option A. |
| Which versionCodes were used? | Current Private source fallback is `2`. Historical test reports mention explicit `42`. The repo does not provide a complete, authoritative release ledger for Owner-installed Private APKs. | `2` and `42` are source/test evidence only; neither is a proven installed version or an approved first formal code. Full installed history `NOT_VERIFIED`. |
| Were Private APK install/update checks performed? | Historical D2 evidence covers disposable/test APK signer and API checks, and other Android update-compatibility harnesses use isolated/test identities. F0/F1A explicitly report Owner device/install/update status unknown. | No evidence establishes install/update of a formally signed same-ID Private Beta on an Owner device. `NOT_VERIFIED`; do not conflate test fixture/emulator coverage with user-data-preserving update compatibility. |
| Could a same-package APK already be installed? | Source fixes `app.serea.android.privatebeta`; repository does not inventory external devices. | Possible, not confirmed. A different signer normally blocks in-place update; uninstall/reinstall is not an acceptable workaround because it may delete app-private data. Stop before key generation, signing, or install until Owner decides how the prior installation question will be resolved. |

The evidence search was limited to repository documentation/history and tracked APK/AAB filenames. It did not search personal folders, keystores, SDK caches, device storage, or external backups.

## 3. Option A design proposal (not yet adopted)

### Format and cryptography

Use a dedicated **PKCS#12** keystore (`.p12`/`.pfx`) with a single RSA private key of **2048 bits minimum**, using **SHA256withRSA** for the certificate signature. Prefer 3072-bit RSA if the Owner's current tooling/policy supports it without compatibility risk; do not choose an undocumented alternative by default. Do not use SHA-1/MD5 signing, Debug/CI/ephemeral/Disposable credentials, a personal TLS key, a future Production key, or a shared Public Beta key.

Android's current signing guidance recommends a validity of **at least 25 years** for a key expected to sign updates. A long certificate expiry does not recover a lost key. Owner must approve the actual validity and recheck current Android/SDK policy before generation. The exact API-level APK signature schemes are selected/verified with the installed current `apksigner` and the app's minimum API level (`minSdk=26`); do not force a scheme based on this report. `apksigner` is the APK-aware signing/verification tool; use the exact SDK Build Tools version recorded in the private operation record.

### Custody and recovery (Owner-controlled)

1. Generate only on an Owner-controlled, patched machine and user account using a currently supported `keytool`/Android Build Tools toolchain; select the Owner's OS before issuing the exact action approval. Do not generate in this worktree, repository, CI, a shared machine, or an agent session.
2. Store the primary encrypted keystore outside source control, build directories, shared folders, and general cloud-sync locations. Restrict access to the Owner; no automated CI access.
3. Keep keystore and key passwords in an Owner-controlled password manager/recovery plan. Never put secrets in command arguments, shell history, environment variables, Gradle properties, source, screenshots, logs, tickets, CI, or chat. Prefer interactive tool prompts.
4. Maintain at least **two separately encrypted backups in independent failure domains** (for example, two separately protected offline media copies stored separately). Protect the backup encryption key/password separately from the backup. Do not publish exact paths or custody details.
5. Keep a private non-secret inventory of custody, backup count/location references, custodian, last integrity check, and recovery procedure. A restore rehearsal is a separate Owner action; do not produce a test APK unless separately approved.
6. **Loss/unrecoverable corruption:** stop new releases and OAuth identity work. Do not generate a replacement and claim compatibility. Owner decides whether recovery is possible or a new package/reinstall/data migration is acceptable.
7. **Suspected compromise:** stop signing and distribution; preserve evidence; Owner-led incident response identifies scope and decides replacement/communication. Do not send key material for diagnosis or revoke unrelated Google grants automatically.
8. **Rotation:** assume a different signer prevents ordinary in-place updates. Android supports signing-certificate lineage under specific platform/tool/channel constraints, but no such lineage has been verified for this Private Beta distribution. Before rotation, require a separately reviewed API/device/channel matrix, Owner-approved lineage, and migration/recovery plan. If compatibility cannot be proven, treat it as a new app identity with potential data loss and a fresh OAuth identity decision.

## 4. Gradle unsigned APK and CI audit

Source review of `android/app/build.gradle.kts` on the stated main SHA:

- `applicationId` is guarded as exactly `app.serea.android.privatebeta`; this proposal does not change it.
- The Private Beta `signingConfig` is explicitly cleared/inherited unsigned. Public Beta's signing configuration is separately cleared; do not alter either.
- Private source default `privateBetaVersionCode` falls back to `2`, but packaging requires the explicit `-PsereaPrivateBetaVersionCode=<integer>` property and accepts only `2..2100000000` in `validatePrivateBetaUnsigned`.
- `assemblePrivateBeta`, `packagePrivateBeta`, and `bundlePrivateBeta` depend on `validatePrivateBetaUnsigned`. `assemblePrivateBeta` / `packagePrivateBeta` are only source-declared graph evidence here; no resolved task graph was captured because Gradle could not resolve task dependencies without SDK configuration.
- The intended artifact is unsigned. Do not describe Gradle `assemblePrivateBeta` as a signed, installable, OAuth-ready or Owner-approved APK. The exact actual output filename/path remains `NOT_VERIFIED` until a separately approved unsigned build produces it.
- CI (`CI=true` or `GITHUB_ACTIONS=true`) rejects Private Beta tasks except the narrowly allowed `testPrivateBetaUnitTest`; do not weaken or bypass this rule.
- Existing repository signing-domain code/records separate development packaging from signing and document disposable-only/test lanes, while formal production signing remains disabled. This does **not** establish a ready/approved Owner Private Beta signing command, custody integration, or permission to use those utilities for Option A. No key is to be passed to Gradle or GitHub Actions.
- Base, Beta, and Private Beta `PRIVATE_GOOGLE_CALENDAR_HTTP_ENABLED` remain false. No OAuth/live gate was changed.

### Non-generating verification attempted

| Attempt | Result |
|---|---|
| `./gradlew :app:assemblePrivateBeta --dry-run -PsereaPrivateBetaVersionCode=2 --no-daemon --max-workers=2` using default JDK 26 | Failed during Gradle startup/configuration with `What went wrong: 26.0.2.1`; no task graph. |
| Same `--dry-run` with documented Temurin JDK 17 | Failed resolving task dependencies: Android SDK location unavailable; no `android/local.properties`. No task graph. |
| `./gradlew :app:packagePrivateBeta --dry-run ...` with default JDK 26 | Same `26.0.2.1` failure. |
| Same package dry-run with JDK 17 | Same SDK location failure. |
| SDK tool availability | `zipalign`, `apksigner`, `apkanalyzer` not found on PATH. No SDK paths or private locations were searched. |

The literal `2` above was supplied **only to a `--dry-run` command** to pass configuration/task dependency resolution; it is not a release recommendation or Owner version decision. No APK-producing Gradle task was executed.

## 5. versionCode proposal

**Status: `PENDING_OWNER`.** Do not infer the first official number. First establish, through Owner-held non-secret records and separately authorized read-only artifact/device checks if needed, the highest versionCode that could be installed for package `app.serea.android.privatebeta`. Then Owner explicitly selects a first formal value strictly greater than that installed value and any prior formally allocated/released code. The source default `2` and historical disposable `42` are not sufficient evidence; the actual installation state remains unknown.

Proposed continuing rule after approval:

- Each Private Beta release gets a unique, strictly increasing positive integer; never reuse a code, even for a failed/withdrawn build once allocated to an artifact/release record.
- Stay within the Gradle guard/Android maximum `2..2100000000`; stop before exhaustion and obtain a new migration/versioning design.
- `versionCode` is the machine ordering/upgrade value. `versionName` is display metadata (`0.1.0-demo` currently inherited); it does not affect upgrade ordering and does not authorize a build.
- Keep a private, access-controlled append-only release record: Owner approval reference, source commit/tree, toolchain, allocated versionCode, versionName, unsigned APK provenance/hash, signing result, final signed APK SHA-256, signer SHA-1 and independently checked SHA-256, distribution/installation state where approved, and superseding/withdrawn disposition. Do not publish private fingerprints or artifact hashes in a public PR.
- Before each future build, compare against that private ledger and any known installed version. Missing/conflicting ledger or unknown installed state is a stop condition, not permission to choose the source fallback.

## 6. Future Owner-only APK procedure (not authorized now)

Only after the prior-APK compatibility question is resolved and each approval is obtained in order. Approvals C and D are separate and apply to different, precisely bounded operations:

1. **Before C:** Owner records the exact proposed source HEAD/Tree, toolchain, `applicationId`, Owner-selected versionCode/versionName, one packaging task, private destination and artifact-handling scope. Verify source/provenance without adding keys or secrets to the repo/CI. Unknown/conflicting prior installation/version remains a stop condition.
2. **Approval C — unsigned build only:** On the approved build machine, run only the explicit Private Beta unsigned packaging task named in C, e.g. `:app:assemblePrivateBeta -PsereaPrivateBetaVersionCode=<OWNER_APPROVED_INTEGER>`. Do not run a broader build or Public Beta task. C does not authorize signing. Preserve the unsigned output as immutable input; record its provenance, metadata and file SHA-256 privately.
3. **After C, before D:** Check the exact unsigned APK's provenance and metadata. Do not trust a filename alone. Verify package, versionCode/versionName and expected source/build record. Locate output from Gradle's actual successful task output; no fixed path is asserted by this report. Present the exact input identity/hash and operation details privately to the Owner for a separate Approval D. Do not align or sign until D is granted.
4. **Approval D — exact artifact only:** D names the just-built unsigned APK by private path/reference and SHA-256, approved source/version, signing certificate identity reference, aligned/final output destinations and verification scope. It authorizes only signing that exact input and verifying the resulting artifact. If the APK/hash changes or is rebuilt, stop and obtain a new D approval for the replacement.
5. Use the approved current Android SDK Build Tools `zipalign` before `apksigner`. For an APK containing uncompressed `.so` files, current Android guidance is `zipalign -P 16 -f -v 4 <unsigned.apk> <aligned.apk>`; then verify with `zipalign -c -P 16 -v 4 <aligned.apk>`. For APKs without `.so`, verify ordinary 4-byte alignment using the current tool. Record tool version and result. Never align or otherwise modify an APK after signing.
6. In the Owner-controlled signing environment, sign the aligned copy with the Owner-held PKCS#12 key using `apksigner` and an explicit output path. Tool prompts should request passwords interactively; do not put passwords in command lines, env, properties, CI, logs, or repo. Use `--ks-type PKCS12` if required by the installed tool to unambiguously select the keystore type. Do not sign the source unsigned file in place; retain clear input/output lineage in private custody.
7. After signing, do not modify, realign, repackage, zip, or re-sign the final APK. Run all verification only against the exact final signed APK:

```sh
apksigner verify --verbose --print-certs <final-signed.apk>
apkanalyzer manifest application-id <final-signed.apk>
apkanalyzer manifest version-code <final-signed.apk>
apkanalyzer manifest version-name <final-signed.apk>
shasum -a 256 <final-signed.apk>
```

   Also record `zipalign -c` result for the exact final file; it is read-only. Record apksigner tool/version, successful exit statuses, signature schemes/API support, expected signer count (normally one), every signer certificate SHA-1 and SHA-256, and compare both fingerprints to the Owner's independently held certificate record. The OAuth SHA-1 must come from this exact verified final signed APK, never source, `signingReport`, Debug, CI, disposable or unsigned APK. SHA-256 is a separate integrity/certificate cross-check, not the OAuth SHA-1 field.
8. Retain final APK, hashes, unsigned input and private provenance/evidence only in Owner-approved private storage. Public PRs contain no private fingerprint, APK hash, personal account, project/client identifier or private storage location.

**Fail closed:** stop on unknown prior APK/install state; wrong package/version; version downgrade/reuse; missing source provenance; unexpected/multiple signer; verification/alignment failure; SHA-1 or SHA-256 mismatch; unsupported format/tool; password exposure; modified artifact after signing; unclear output path; SDK/toolchain ambiguity; any CI packaging attempt; or any request that exceeds the exact approval.

## 7. Separate update compatibility and data-preservation gate

A source/manifest check is not proof of install/update compatibility. Before any device action, compare the exact new signed candidate and the exact prior installed APK (only if Owner separately authorizes access): same application ID; same signer or verified Android-supported signing lineage for the target OS/channel; candidate versionCode strictly higher; artifact provenance and signature valid. If any field is unknown, `NOT_VERIFIED` and no installation.

Installation/update is **Approval E**, separate from signing. If approved, do not uninstall, clear app data, reset accounts, or use personal device data outside the explicitly named test. Verify the update is in-place and app data is preserved only under that narrow approval; stop immediately on package/signer/version mismatch or unexpected data loss. No device or user-data access was performed for this report.

## 8. Google Cloud handoff boundary

No Cloud action is authorized or performed. Android OAuth client registration stays **`BLOCKED_BY_SIGNING_IDENTITY`** until there is an Owner-approved, provenance-verified final signed APK with confirmed package `app.serea.android.privatebeta`, a single expected signer, verified APK signer SHA-1, and independently compared SHA-256. When a later stage actually requires Cloud interaction, report `OWNER_ACTION_REQUIRED — GOOGLE_CLOUD` and provide screen, input, prohibited choices, confirmation and stop conditions; Owner alone operates the Console.

For that later Owner-only stage, the inputs/checklist from the current Personal Calendar decision are:

- Google Auth Platform audience: **Personal**, creator account only (Owner verifies in Console; status remains unconfirmed here).
- OAuth client type: Android.
- Application ID: `app.serea.android.privatebeta`.
- SHA-1: certificate fingerprint from the exact verified installable signed APK (not available yet).
- SHA-256: independently compared audit fingerprint from that APK/certificate (not available yet).
- API: Google Calendar API only.
- Scope: `https://www.googleapis.com/auth/calendar.events.readonly` only.
- Calendar: `primary` only.
- Do not select Gmail, Drive, CalendarList, Calendar write, extra APIs, AI-powered-agent client, broad audience, extra accounts, billing/quota changes, or any unapproved OAuth/HTTP gate.

Owner reports only non-secret confirmation; project/client IDs, account details, fingerprints, APK hashes, credentials and screenshots with private identifiers remain private. No Cloud completion is inferred.

## 9. Approval gates and Owner handoff

Each approval below is independent. Approval of one never authorizes the next. Current state for every action is **not approved/not performed** unless stated otherwise.

| Approval | Separate explicit Owner decision needed | Current state |
|---|---|---|
| **A — Adopt Option A** | Formally adopt a fixed Owner-managed Private Beta-only certificate for the unchanged package; acknowledge loss/compromise/rotation and update-compatibility risks. This report only recommends A. | `PENDING_OWNER` |
| **B — Generate and secure key** | After prior APK/install uncertainty is resolved sufficiently to decide continuity, approve Owner's OS/host/tool versions, exact key parameters, interactive generation, storage and two encrypted backup domains. The Owner performs all key operations. | `NOT_AUTHORIZED` |
| **C — Build unsigned APK** | Approve exact source HEAD/Tree, Owner versionCode/versionName, host/toolchain, one Gradle packaging task and private output/provenance handling. C authorizes no signing. After the build, record the exact unsigned artifact's metadata and SHA-256 privately. | `NOT_AUTHORIZED` |
| **D — Sign and verify** | Only after C completes: separately approve the exact unsigned APK by private reference and SHA-256, source/version, Owner-managed key operation, aligned/final output and certificate verification/evidence handling. If the input changes, obtain a new D approval. Owner retains/uses the private key. | `NOT_AUTHORIZED` |
| **E — Install/update** | If necessary, name exact device, test action, package/version, data scope, preservation expectations and stop/recovery rules. No uninstall/data-clear workaround without separate authorization. | `NOT_AUTHORIZED` |

### `OWNER_ACTION_REQUIRED — PRIVATE_SIGNING`

- **Target:** Serea Private Beta.
- **Proposed method:** A — fixed Owner-managed Private Beta-only certificate; not formally adopted yet.
- **Application ID:** `app.serea.android.privatebeta` (verified from source guard, not an APK).
- **Existing Private APK:** existence and installed state unknown (`NOT_VERIFIED`). Historical unsigned/disposable test APKs exist in repository records; no official Owner-installed artifact is proven. Do not proceed with new key generation until Owner resolves the possibility of an existing installation and decides a safe, separately authorized way to verify its signer/version if needed.
- **versionCode:** Owner must decide after comparing known installed/formal release records. Current source fallback `2` and test `42` are not recommendations; current formal code is `PENDING_OWNER`.
- **Proposed keystore/signature:** PKCS#12; RSA 2048-bit minimum (3072-bit if policy/tooling-compatible), SHA256withRSA; validity at least 25 years subject to Owner policy/current official guidance. These are a proposal pending Approval A/B.
- **Owner OS/tool:** `PENDING_OWNER`. Current report environment is macOS with JDK 17 available, but SDK Build Tools/SDK path unavailable in this worktree; do not treat this as the selected Owner signing host.
- **Key generation/custody:** after A and B, Owner uses interactive `keytool` on Owner-controlled host, saves outside repo/CI, secures credentials in password manager and maintains two independently encrypted backups in separate failure domains. No agent handles the key or password.
- **APK operation approvals:** Obtain C first for one unsigned packaging task/source/version. After that task completes and its exact APK provenance/hash is recorded privately, request D for that exact input before any alignment/signing. E separately approves any device install/update. None is implied by another; changed inputs require fresh D.
- **Verification:** provenance and unsigned identity; zipalign before apksigner (16 KiB `.so` alignment when present); no changes after signing; `apksigner verify --verbose --print-certs`; `apkanalyzer` application-id/version-code/version-name; APK SHA-256; signer count; independently compare signer SHA-1 and SHA-256; prior APK signer/version compatibility.
- **Stop conditions:** any unknown/conflicting previous APK/install/version; identity or lineage mismatch; version not approved/increasing; invalid provenance, alignment or signature; multiple/unexpected signer; fingerprint mismatch; credential exposure; unapproved source/task/path/device; CI packaging; disabled gate changed; or request to access personal file/device/Cloud.
- **Non-secret confirmation requested from Owner only:** decision A yes/no; whether an earlier same-ID APK may be installed and its verified package/version/signer match result (do not send fingerprint publicly); Owner-selected OS/toolchain; approved versionCode; completion of separate backup/recovery arrangements; which *next single* approval is requested. Do not send key, password, private path, APK, fingerprints, account data, device data, or screenshots to the agent/public PR.

## 10. Security review and validation status

### Read-only security assessment

- No Critical/High defect identified in this bounded audit. This is not a full product or release security certification.
- **High-impact unresolved risk:** an undiscovered old same-ID installation with another signer/version could block updates or lead to app-data loss if handled with uninstall/reinstall. Treat this uncertainty as a release stop until Owner resolution.
- **Medium:** no formal private signing identity, prior installed APK inventory, Owner-approved version, signed-artifact provenance/fingerprint, output path or update compatibility is established. OAuth registration is blocked.
- **Medium:** Gradle dry-run could not resolve the actual task graph because SDK location is unavailable. Source statically declares the tasks/guards; runtime task graph and output path remain unverified.
- **Low/operational:** this machine's default JDK 26 fails with the known `26.0.2.1` startup/configuration issue; JDK 17 gets past it, but the SDK configuration is absent in this worktree. No configuration was borrowed from the dirty checkout.
- Source review confirmed Application ID, unsigned signing configuration, version guard, CI packaging restriction, unchanged Public Beta signing configuration and disabled HTTP gate. No product-code fix was attempted.

### Validation matrix

| Check | Result |
|---|---|
| Owner-provided main SHA / Tree | `846d6693248a09656beb2d73ff693c31e474ef29` / `51a8c10c4e8ff9a4bad17ded714491c9f51fb9b2` |
| GitHub main check snapshot (2026-11-21; historical for main SHA only) | Full CI, Fast CI, Rust Dependency Security, Cross-architecture SQLite: success; Android workflow: failure; Native platform validation: in progress. These are main checks, not F1B PR results. |
| Gradle source/guard/CI review | Read-only review performed; dynamic task graph unavailable as detailed above. |
| APK generation/signing/verify/install/update | `NOT_RUN` (not authorized); no APK output exists from this run. |
| Official Android signing docs | Reviewed 2026-11-21: app-signing validity/continuity guidance, `apksigner` verification and “zipalign before apksigner / do not modify after signing”, `zipalign -P 16` for APKs containing uncompressed `.so`, and versionCode monotonic ordering. |
| F1B document link/identifier validation | `python3 tools/validate_docs.py`: 241 Markdown documents passed. |
| Secret scan of F1B report and review records | Gitleaks scanned each changed document; no leaks found. |
| `git diff --check` | Passed for the staged F1B document-only changes. |
| Independent read-only security review | Generation 0: F1 Major found and resolved in documentation. Generation 1: F1 closed; no new finding in the corrected approval sequence. See [review report](2026-11-21-code-review-h4-f1b-signing-preparation.md), [resolution](2026-11-21-receiving-review-h4-f1b-resolution.md), and [generation-1 follow-up](2026-11-21-code-review-followup-h4-f1b.md). |
| Same-SHA PR checks / Required CI / Android Formal | Not yet available before PR creation; check current Draft PR exact HEAD before treating CI as complete. Do not infer from main or F0/F1A results. |
| Google Cloud / OAuth / token / Calendar HTTP | `NOT_AUTHORIZED`, `NOT_RUN`. |

Official sources checked:

- [Android app signing](https://developer.android.com/studio/publish/app-signing)
- [Android `apksigner`](https://developer.android.com/tools/apksigner)
- [Android `zipalign`](https://developer.android.com/tools/zipalign)
- [Android app versioning](https://developer.android.com/studio/publish/versioning)

## 11. Gate decision

**`F1B_PARTIAL` / NO-GO for key generation, APK production/signing, Cloud, and device actions.** The technical proposal and stop conditions are documented, but first formal versionCode is pending; prior installed APK state is unknown; Gradle's live task graph/output path are blocked by absent SDK configuration; and Owner approvals A–E remain separate. A future stage may proceed only after the relevant fresh Owner approval and all prior-artifact/update compatibility questions are resolved. Keep PR Draft; do not enable signing, OAuth or HTTP gates, and do not merge under this report.