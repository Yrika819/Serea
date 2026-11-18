# Serea H4-D2-D2 — Final Dynamic Acceptance

**総合判定: `PARTIAL`**  
**Signing feature freeze: NOT READY**  
**Production signing: 未実施・未承認 (`PRODUCTION_SIGNING_AUTHORIZED=false`)**

## Baseline and authority boundary

- Repository: `Yrika819/Serea`; expected HEAD `317c601760ab7148afd979ee9ef092838d76a072`, Tree `2017e9b15c09724459afe3f59c4740bb4a6c04fe`.
- Final HEAD/Tree remained those baseline values. Existing dirty and untracked work was present at start and preserved; this run made only the requested minimal test/source and runbook/report edits. No commit, push, PR, merge, network fetch/download, formal signing asset, Keychain, external medium, device, ADB, OAuth service, Google Cloud, AI service, distribution, or OS-security change was used. Astra was not used.
- Gradle ran offline with Temurin 17.0.21, Gradle 8.14.6, AGP 8.12.3, Build Tools 35.0.0 and Android 35, using a separate `/private/tmp/serea-h4d2-gradle-home` that linked only the pre-existing Gradle caches and wrapper distribution. No wrapper or dependency download occurred. Disk space was sufficient; no competing Gradle process was found at preflight.
- The local worktree is not a clean, Owner-approved source snapshot. APK evidence below proves local disposable build/signing only, not formal source approval or same-SHA CI.

## Acceptance criteria

| AC | 判定 | Evidence / remaining limitation |
|---|---|---|
| AC-01 Real encrypted Disposable Vault; both signing lanes | **PASS** | Fresh AES-256 encrypted Vault, distinct Public/Private PKCS12 identities, external Ledger/Registry/Witness binding, and independent CLI signing. Actual Serea APKs signed in separate lanes; single expected signer and API 26/35 verification passed. No formal key or APK involved. |
| AC-02 Encrypted backup, isolated restore, stale-state rejection | **PASS (scoped)** | Encrypted backup and isolated restore succeeded. Both lane identities and restored internal high-watermarks `43`/`72` were checked against external state. A fresh encrypted-image `verify-backup` reproduction rejected same-counter stale history after the guard below. The Vault image does not contain its separately stored Ledger/Registry/Witness; backups of all four stores and independent custody are still an Owner gate. |
| AC-03 Essential recovery and continued signing | **PARTIAL** | Existing receipt/recovery diagnostics and one SIGKILL boundary (after APK signing, before `BUILT`) are evidenced; consumed version remains consumed and receipt/artifact is discoverable. Separate-process lock contention is covered. Recovery interruption, sidecar/APK publish boundaries, signer-vs-Recovery race, and manual recovery followed by a fresh signed version are not all dynamically demonstrated. No direct ledger edit, consumed-code rollback, or evidence deletion was used. |
| AC-04 Serea Public/Private unsigned APK build | **PASS (local inputs only)** | Both APKs built offline, with OAuth false and no Beta signing credentials. IDs, code/name and unsigned state verified below. Build inputs came from this dirty worktree, not an Owner-approved immutable source snapshot. |
| AC-05 Actual Serea APK independent Disposable signing | **PASS (Disposable only)** | Both actual Serea APK inputs were independently signed by the CLI using separate lane identities. Receipt/Ledger evidence, output hashes and certificate fingerprints are below; `apksigner` verified API 26 and 35. These signed artifacts were test outputs, not released or distributed. |
| AC-06 Python and Android regression | **PASS (focused scope)** | Python **95/95 PASS** including build-provenance tests and the new Ledger/Vault artifact-history regression. Android Debug focused **77/77 PASS**; Private OAuth-enabled Fake JVM **10/10 PASS**; Private OAuth-disabled gate **1/1 PASS**. No OAuth-enabled APK packaging or service communication. Docs validation and final `git diff --check` are recorded under Final verification. |
| AC-07 Independent review | **PASS (review scope)** | Terra’s final read-only follow-up confirmed the same-counter stale-artifact-history High was resolved and found no remaining Critical/High in its inspected scope. It still identified receipt-backed interruption recovery as a Beta-operational blocker; this is reflected under AC-03. Internal review is not GitHub approval or an external audit. |
| AC-08 Owner runbook and source preparation | **PARTIAL** | Production launch runbook synchronized; historical CLI runbook prominently superseded and non-executable. Candidate source allowlist and draft PR/CI contract are listed below. Comprehensive publication secret audit, Owner source selection, same-SHA CI, and formal host/key preparation remain undone. |
| AC-09 Existing artifact and authority-boundary preservation | **PASS (observed scope)** | HEAD/Tree unchanged; no pre-existing APK outputs or unrelated mounts were removed. One Python test reported an already attached image at an unverified mountpoint and left it untouched. Only this run’s newly created, confirmed-detached Disposable scratch images were removed. No prohibited operation was performed. |

## Actual Serea APK evidence

| Lane | Application ID | versionCode / versionName | Unsigned input SHA-256 | Signed output SHA-256 | Certificate SHA-256 | Verify |
|---|---|---:|---|---|---|---|
| Public | `app.serea.android.beta` | `42` / `0.1.0-demo` | `71bd31de17b34cd46f343d415ea99e62d5aedd4f1a2f0c3af0a897d78df840ef` | `bc37a36a443ee197593439ccc6753c40de2cfc25665aa17d44f01db317302e94` | `282cd6343abe2de987b134b4aea5738971204c02e41c8a2e4dceb5cd7583886c` | API 26, 35 PASS |
| Private | `app.serea.android.privatebeta` | `71` / `0.1.0-demo` | `c9767f0f8fe1e456deda9e16720a55933a273100a6e66ec0eec3c7180dd57d1f` | `32306ffe89ff088fa83565bbace6779e0d9f8436043f78e283456fafb4452c12` | `1e4a5736385a24c453f3d0e945255730efcfdadc6e8967d103a393f333fb81b8` | API 26, 35 PASS |

Both current build-output APKs remain unsigned: `aapt dump badging` confirmed the IDs, version codes and version name above; `apksigner verify` returned `DOES NOT VERIFY` / `Missing META-INF/MANIFEST.MF`, as expected for unsigned inputs. Their current on-disk SHA-256 values match the input hashes above. BuildConfig evidence had `OWNER_PRIVATE_BETA_GOOGLE_OAUTH_ENABLED=false` for both variants. Private auth UI being compiled does not mean OAuth is active. No APK install or distribution occurred.

Disposable signing attempt IDs: Public `fb01a0df-71de-4b1e-9294-fa0227f73f0d`; Private `f6eab248-fb88-467b-9596-20270e3c20b8`. The signed output and certificate hashes matched their publication receipts and Ledger `BUILT` records. Signed Disposable output files were not retained as distributable artifacts.

## Stale-Vault defect and fix

`ReleaseLedger.assert_matches_vault()` previously checked the binding, signer identities, counters, and unfinished attempts, but not whether the Vault’s completed artifact records matched external `BUILT` events. The new per-lane check requires the Vault artifact history to match external `BUILT` records by ordered versionCode, versionName and APK SHA-256. It fails closed on missing, extra, or mismatched records.

- TDD RED: `test_vault_must_include_every_artifact_recorded_as_built_in_release_ledger` failed because a same-counter Vault with zero artifacts was accepted against one external `BUILT` event.
- GREEN: the focused test passed after the validation guard was added; the full Python regression passed afterward.
- Dynamic reproduction: created a fresh AES-256 encrypted Disposable Vault and separately bound Ledger/Registry/Witness. The test consumed Public versionCode 2, persisted internal `next_version_code=3` with no artifact record, snapshotted that encrypted image, then advanced the external stores to `BUILT` for version 2. Internal and external next counters both equaled 3; the stale image had 0 artifacts versus 1 external `BUILT`. The actual `verify-backup` CLI rejected it with the expected recovery-reconciliation refusal. Only newly created, detached test images were cleaned up.
- An earlier stale-image run had unexpectedly passed. Its original exact state was not retained, so this report does not assert that the new reproduction reconstructs every detail of that earlier run. The identified same-counter/stale-history acceptance path is now covered and rejected.

## Recovery evidence and operator boundary

- Existing receipt diagnostics remain read-only; they report unknown/incomplete evidence and do not automatically republish.
- The existing SIGKILL child-process test covers signing complete but Ledger `BUILT` not recorded. It verifies permanent code consumption, no final APK publication, and discoverable receipt/artifact evidence. Separate lock-contention coverage exists.
- **Not proven:** forced exit after `BUILT`, after sidecar publication, before APK publication, Recovery interruption, same-code competing signer, signer-vs-Recovery race, nor a full manual recovery followed by an actual new signed version. A receipt-backed uncertain attempt remains `STOP`: retain attempt artifacts and reconcile exact hashes, Ledger, Vault, lane, code and certificate with the Owner. Do not rerun the same code, rewrite the Ledger, remove evidence, or publish an uncertain artifact.
- `ledger-abandon` is limited to an exact eligible reservation with no managed receipt or candidate APK; it permanently consumes the version. It is not a repair/republication command for receipt-backed attempts.

## Regression and final verification

- Python command: `python3 -m unittest tests.test_serea_sign_apk_cli_e2e tests.test_serea_release_ledger tests.test_serea_identity_registry tests.test_serea_signing_witness tests.test_serea_enrollment_authority tests.test_serea_signing_cli tests.test_serea_apk_signer tests.test_serea_apk_signer_e2e tests.test_android_build_provenance -q` — **95 tests PASS**. CLI negative-case usage output and a duplicate-ZIP fixture warning are expected. A test encountered a pre-existing unrelated attached image at an unverified mountpoint; it was left untouched.
- Android Debug focused — **77/77 PASS**, task `:app:testDebugUnitTest` for the seven named focused classes; offline, isolated Gradle home, `--rerun-tasks`.
- Private OAuth enabled Fake JVM — **10/10 PASS**, only `:app:testPrivateBetaUnitTest` with `PrivateGoogleConsentResultTest` and `PrivateGoogleGatewayOperationsTest`, OAuth flag enabled for this JVM task only.
- Private OAuth disabled gate — **1/1 PASS**, only `PrivateBuildOAuthGateTest.oauthGateRequiresBothPrivateVariantAndCallsiteOptIn`, OAuth flag false.
- `python3 tools/validate_docs.py docs`: **PASS**, 241 Markdown files scanned; identifiers, JSON, placeholders and cross-references valid.
- `git diff --check`: **PASS**.

Android compiler emitted existing Kotlin/Gradle deprecation and opt-in warnings; tests succeeded. `android.enableResourceOptimizations=false` is currently deprecated by AGP and is a compatibility workaround for case-insensitive APK resource-path collisions rejected by the signer. It is not treated as a permanent configuration choice.

## Source hashes and source-preparation packet

Final local file SHA-256 values:

```text
317c601760ab7148afd979ee9ef092838d76a072  git HEAD
2017e9b15c09724459afe3f59c4740bb4a6c04fe  git Tree (baseline; local changes remain dirty/untracked)
bcc9475f85c5148e0042163ebd416e65e139a6024106a912c246351f8e37102a  tools/serea_signing.py
33ab0d32427f6cce1628594dcfc6ae645669c8cad16f5aec8ef177f26045c0cf  tools/serea_release_ledger.py
b0a5dd7576bf5cbcbd281982c058e391f4fa10bee60c9fe625d48b972649ddc4  tests/test_serea_sign_apk_cli_e2e.py
e912d172117523f272ee8443816dcdf517d3327eb36c29e9de0a1197b019485e  tests/test_serea_signing_cli.py
260eaec937520554623fd8055fefc6c7ea85514b0d61e356ff98a3fc02482654  tests/test_serea_release_ledger.py
d2806925421c45181e536a8d70cd74995521cca3b97ba01fe20dcf76b5d78a40  android/app/build.gradle.kts
06a55a1fcba0484a760fe08178a21b8de21f441476a7f535119dad0850ae6d79  android/gradle.properties
```

Candidate public-source allowlist for Owner review (not yet approved): `tools/serea_signing.py`, `tools/serea_release_ledger.py`, the referenced signing modules under `tools/`, focused tests under `tests/`, `android/app/build.gradle.kts`, `android/gradle.properties`, and the two current Owner runbooks plus this acceptance report. Exclude `.local/`, `.superpowers/`, build outputs, machine-local Gradle/SDK state, passphrases, private keys, PKCS12 files, Vault images, formal or Disposable state, and personal environment/configuration files. This is a proposed path allowlist, not a completed secret scan or source selection.

**Draft PR body (not created):** “H4-D2 final signing acceptance: preserve unsigned Public/Private Gradle variants and use the independent signer; reconcile encrypted Vault artifact history against external Release Ledger `BUILT` events. Disposable validation only; no Production authorization. Link the final dynamic acceptance report; require independent review and all Required checks on the exact source SHA before any Owner source selection.”

**CI contract:** do not enable secrets or formal signing. Require all four protected-main checks—`Linux fast checks`, `Linux stable full validation`, `Android Emulator Required Gate`, `Linux MSRV 1.89.0`—on the exact reviewed source SHA, plus applicable Android API 26/35, variant isolation, Python/signing, security, provenance and docs checks. No GitHub fetch/check run was performed, so same-SHA CI is `NOT_RUN`.

## Owner gates still outstanding

- Select the official source snapshot/allowlist and review a complete secret/personal-environment scan; obtain same-SHA CI and independent approval before source freeze.
- Choose and harden a supported formal signing host. Resolve FileVault, SIP/OCLP and host-trust concerns in the current runbook; no OS setting was changed here.
- Decide custody and backup/restore procedure for the encrypted Vault **and** its separately located Release Ledger, Identity Registry and Witness. Test encrypted backups on Owner-controlled external media; none was accessed here.
- Complete only the essential remaining manual recovery, publication interruption and signer/Recovery race evidence. If operational recovery cannot safely complete a receipt-backed attempt, STOP; do not relax fail-closed behavior.
- Formal identity generation, formal key custody, formal APK signing, distribution, external media, and same-SHA CI remain separate Owner approvals/gates. No formal signing approval is requested or implied by this report.

**Decision:** `PARTIAL` — do not declare `SIGNING_FEATURE_FREEZE_READY` yet. Keep feature work limited to safety/usability blockers; resolve the recovery and Owner/source gates, then make a fresh freeze decision. No feature-freeze milestone itself authorizes Production signing.
