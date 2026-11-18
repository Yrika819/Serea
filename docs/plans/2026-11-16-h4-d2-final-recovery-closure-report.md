# Serea H4-D2 — Final Recovery Closure

**Overall: `PARTIAL` — signing feature freeze is not yet ready.**

Disposable recovery code and fixture-backed dynamic coverage are complete for the tested paths. The missing encrypted-Vault recovery run and unresolved conditional review finding are not treated as PASS. Production signing remains disabled and unauthorized.

## Scope and integrity

- Expected baseline remains HEAD `317c601760ab7148afd979ee9ef092838d76a072`, Tree `2017e9b15c09724459afe3f59c4740bb4a6c04fe`. The worktree was already dirty/untracked; those artifacts were preserved. Only the recovery CLI/tests and Owner runbook/report were edited for this closure.
- No network/GitHub fetch, download, formal asset, Keychain, external medium, device/ADB, OAuth, Google Cloud, AI, APK distribution, OS security change, commit, push, PR, or merge was performed. Astra was not used.
- Source SHA-256: `tools/serea_signing.py` — `bcae40d4ccbffb7b6754e86dbaec069424be4aa96701ff28726ae9195490e14c`; `tests/test_serea_sign_apk_cli_e2e.py` — `4a5ba8fdb86a836a66c30f3117835e8a8de184076ee47e9456b5647f8e12b0bc`; `tools/serea_release_ledger.py` (unchanged) — `33ab0d32427f6cce1628594dcfc6ae645669c8cad16f5aec8ef177f26045c0cf`.
- These hashes identify local dirty/untracked files only. They are not an approved source snapshot, signed manifest, or same-SHA CI result.

## Acceptance criteria

| AC | Result | Evidence / limitation |
|---|---|---|
| AC-01 Receipt-backed safe recovery | **PASS (fixture-scoped)** | Added `ledger-recover-publish`, Disposable-only, exact confirmation required. It validates latest attempt and state stores, never calls the signer, reconciles missing Vault artifact history, and uses no-replace sidecar-then-APK publication. |
| AC-02 Representative interruption recovery | **PARTIAL** | Dynamic disposable APK tests recovered VERIFIED-before-BUILT and BUILT-after-sidecar-before-APK. Tampered sidecar was refused; idempotent retry passed. Recovery itself was not SIGKILLed at every write boundary and this flow was not run through a newly created encrypted disk-image Vault. |
| AC-03 Recovery then new version signing | **PASS (fixture-scoped)** | The VERIFIED recovery E2E test publishes code 42, then independently signs code 43; API 26/35 signer verification is asserted. The test uses fresh disposable PKCS12 identities and temporary directory-backed mount fixtures, not an encrypted mounted Vault. |
| AC-04 No code reuse / invalid publication | **PASS (tested paths)** | Same consumed code is blocked while recovery is pending; tampered managed sidecar is rejected; existing-output conflicts are refused by the no-replace path. No wrong artifact was observed published. |
| AC-05 Concurrency / competition | **PARTIAL** | Recovery runs under the existing root lock; same-version retry is tested sequentially. No true competing-process signer-vs-Recovery race or concurrent same-version signer run was performed in this closure. |
| AC-06 Related regressions | **PASS** | Recovery CLI E2E: **9/9**; Ledger/Registry/Witness/CLI/signer unit suites: **71/71**; Android build-provenance: **5/5**. `py_compile`, docs validator (**241 Markdown files**), and `git diff --check` passed. Android/Gradle was not rerun; the prior phase’s Android results are historical, not this run. |
| AC-07 Independent review | **PARTIAL** | Terra performed one focused read-only review. No confirmed Critical/High issue was reported; the review did report a **Major, conditional** same-user filesystem TOCTOU concern and a Minor conditional receipt/provenance-binding limitation. These are retained at their reported severities; the Major concern needs disposition before declaring freeze readiness. |
| AC-08 Owner/source preparation | **PARTIAL** | Runbook now documents the exact Disposable recovery operation, its confirmation string, and stop conditions. Candidate allowlist, draft PR text and CI contract are below. Complete secret/personal-environment audit, Owner source selection, and exact-SHA CI remain outstanding. |
| AC-09 Preserve artifacts and boundaries | **PASS (observed scope)** | No existing artifact was deleted/reset and no prohibited authority was exercised. Existing unrelated dirty and untracked work remains present. |

## Dynamic recovery evidence

The tests create fresh APKs and Public/Private disposable PKCS12 identities in temporary directories; the context manager removes those test fixtures after assertions. No production or persistent artifact is involved.

1. **VERIFIED, Ledger still ATTEMPTED, Vault artifact append missing:** an injected failure after the VERIFIED receipt but before `_ledger_add` reproduced a first-retry failure. Recovery was corrected to reload and validate the Vault ledger after adding the matching artifact. The same end-to-end test then proved publication without re-signing, API 26/35 certificate verification, and a successful next-version signing. A repeated signing request for the consumed code remained blocked before recovery.
2. **BUILT, sidecar already published, APK absent:** injected APK-link failure left the receipt/sidecar in place. Recovery verified the managed artifact, refused a tampered sidecar without publishing the APK, then completed publication after the fixture was restored. A second recovery invocation passed idempotently; exactly one BUILT event remained.
3. API 26/35 verification, certificate/lane identity, receipt state, Ledger progression, and next `versionCode` are asserted in the E2E tests. Temporary test outputs are not retained, so there is no standalone durable APK/receipt bundle or retained per-attempt hash set from these runs.

**Not dynamically demonstrated:** recovery through a fresh AES-256 encrypted disk image with actual mount/unmount; recovery process interruption at multiple recovery boundaries; competing processes for signing and Recovery; encrypted backup/restore of the recovery-in-progress state. Earlier phase encrypted Vault creation, dual-lane signing, API 26/35 checks, backup/isolated restore, and stale-history rejection remain earlier evidence only and do not fill these recovery-specific gaps.

## Independent review findings

Terra’s single focused review confirmed no recovery call to the signer and no intentional destination overwrite. It reported:

- **Major, conditional TOCTOU:** verification reads the managed APK/sidecar by path, while later publication hard-links by path. A concurrent same-UID filesystem writer could replace a managed path between verification and linking. The review notes this is outside the ordinary protected-directory/cooperating-CLI model, but it is not resolved by the current CLI lock against arbitrary same-user writers. Do not describe this as fixed or silently lower its severity.
- **Minor, conditional receipt binding:** recovery takes the source-input hash and intended output path from the owner-private receipt; those values are shape-checked but have no separate authenticated attempt record. A receipt alteration that preserves schema cannot be independently disproved from the signed APK alone. The receipt is trusted local state in this implementation; stronger independent binding is deferred, not claimed.
- The review’s prior first-retry issue—stale in-memory Vault history after recovery appended a missing artifact—was reproduced and fixed. The regression test passed after the fix.

No other review was repeated. This was not an external audit, GitHub approval, or production authorization.

## Regression commands

- `python3 -m unittest tests.test_serea_sign_apk_cli_e2e` — 9 passed.
- `python3 -m unittest tests.test_serea_release_ledger tests.test_serea_identity_registry tests.test_serea_signing_witness tests.test_serea_signing_cli tests.test_serea_apk_signer` — 71 passed. The suite emitted expected argparse negative-case usage text, a duplicate-ZIP fixture warning, and a warning about a pre-existing attached image at an unverified mountpoint; that image was left untouched.
- `python3 -m unittest tests.test_android_build_provenance` — 5 passed.
- `python3 -m py_compile tools/serea_signing.py tests/test_serea_sign_apk_cli_e2e.py` — passed.
- Editor diagnostics still show unrelated/pre-existing type-analysis errors in the broader untracked signer module (`_signed_record`, recovery-plan reporting, `cmd_sign_apk`/retired `cmd_build`) and test typing; these were not broadened into this recovery-only change.
- `python3 tools/validate_docs.py docs` — passed; 242 Markdown files scanned.
- `git diff --check` — passed.

## Owner recovery procedure (Disposable only)

1. Run `ledger-recovery-plan`; preserve the attempt directory and do not edit JSON or remove evidence.
2. Independently match the exact attempt ID, lane, code, VERIFIED/BUILT receipt, signed APK hash, sidecar hash (if present), APK identity and certificate, Ledger latest event, Vault artifact history/counter, Registry/Witness high-watermarks, and intended output path. Any mismatch or unknown state is STOP.
3. Only after explicit Owner review, run the exact command shape below in Disposable mode; the confirmation must exactly repeat its lane, UUID and code. This command is not available for Production authorization:

   ```sh
   python3 tools/serea_signing.py ledger-recover-publish \\
     --disposable --root <disposable-root> \\
     --release-ledger <ledger.json> --identity-registry <registry.json> \\
     --witness <witness.json> --lane <public-or-private> \\
     --attempt-id <attempt-uuid> --version-code <code> \\
     --confirm-publish 'PUBLISH <lane> <attempt-uuid> <code>'
   ```

4. Re-run the read-only plan; verify receipt `PUBLISHED`, exact output and sidecar hashes, API 26/35 signer, and unchanged consumed high-watermarks. Continue only with the next assigned code. If recovery stops, retain evidence and escalate; do not re-sign, overwrite, directly edit stores, roll back a code, or delete evidence.

## Source-preparation packet

**Candidate changed-file allowlist for a future review (not approved):**

- `tools/serea_signing.py`
- `tests/test_serea_sign_apk_cli_e2e.py`
- `docs/plans/2026-11-15-owner-production-signing-launch-runbook.md`
- `docs/plans/2026-11-16-h4-d2-final-recovery-closure-report.md`

The closure did not alter Android/Gradle settings. This narrow list is not the complete public source allowlist: existing related signing modules/tests and prior approved build changes must be reviewed as a cohesive snapshot. Exclude all private config, personal environment files, keys, passphrases, PKCS12/Vault images, state stores, `.local/`, `.superpowers/`, caches and build outputs. **A complete secret/personal-environment scan has not been performed.**

**Draft PR title (not created):** `H4-D2: close Disposable receipt-backed signing recovery`

**Draft body:** “Add explicit, Disposable-only receipt-backed publication recovery. Recovery verifies the exact latest attempt, reconciles existing Vault/Ledger artifact state, does not re-sign, and publishes without replacing existing outputs. Include recovery interruption/retry E2E tests. Production remains disabled and unauthorized. Require focused independent review and all Required CI on the exact Owner-selected source SHA; encrypted-Vault recovery and remaining race/host gates are explicitly tracked.”

**CI contract:** Required checks on exact approved source SHA; Python signing/Ledger/Recovery and docs validation; Build Provenance; Android Debug and applicable API 26/35 variant isolation. Private OAuth Enabled remains Fake JVM only, with no service communication or OAuth-enabled APK packaging. No same-SHA CI run was requested or performed.

## Individual readiness decisions

| Dimension | Decision |
|---|---|
| `SIGNING_CORE` | **PASS (Disposable scope)** |
| `RECOVERY_READINESS` | **PARTIAL** — no encrypted mounted-Vault recovery proof; Major conditional review finding remains |
| `DYNAMIC_ACCEPTANCE` | **PARTIAL** — fixture E2E passes, encrypted Vault and true process-race coverage absent |
| `SECURITY_REVIEW` | **PARTIAL** — Major conditional TOCTOU finding requires explicit disposition |
| `SOURCE_PREPARATION` | **PARTIAL** — runbook/allowlist/CI packet ready; secret scan and source selection outstanding |
| `SOURCE_INTEGRITY` | **PARTIAL** — local hashes recorded; tree is dirty and not Owner-approved |
| `HOST_TRUST` | **NOT READY** — formal host/FileVault/SIP/OCLP review is an Owner gate |
| `BACKUP_READINESS` | **PARTIAL** — earlier encrypted backup/restore succeeded; external media and independent custody of Vault plus Ledger/Registry/Witness remain Owner gates |
| `PRODUCTION_SIGNING_AUTHORIZED` | **false** — separate explicit Owner approval is required |

## Owner gates and decision

- Review/disposition of the Major conditional same-user TOCTOU finding; retain the Minor receipt-binding limitation accurately.
- Run recovery through a fresh AES-256 encrypted Disposable Vault, with no formal assets, then validate encrypted backup/isolated restore as applicable.
- Add or run bounded true competing-process/recovery-interruption acceptance if needed to close the Beta operation risk.
- Select and freeze an Owner-approved clean source snapshot, complete secret/personal-environment audit, and obtain Required CI on that exact SHA.
- Formal signing host trust, FileVault/SIP/OCLP, external backup media and independent custody, and formal key generation remain separate Owner decisions. None authorizes formal signing here.

**Final decision: `PARTIAL`.** Recovery implementation is usable in tested Disposable fixtures, but `SIGNING_FEATURE_FREEZE_READY` is not asserted because mandatory encrypted-Vault acceptance and review disposition remain open. Formal signing approval was not part of this work and is not implied.
