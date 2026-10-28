# G5 Orchestrator Ledger

## Checkpoint — 2026-10-28

| Field | State |
|---|---|
| ACTIVE_PHASE | G5 evidence closure |
| GOAL_STATUS | PARTIAL — harness regression identified; lineage regression reproduced; API 35 on-device PoC validated |
| AUTHORITY_SHA | G4 IME branch `b06c1acd5f3ddb4f59f0cad6ae1a1d291b086671`; base candidate #132 `373b8be15f407d951157d2a80a632e86d979acdf` |
| LAST_GOOD_SHA | `029c5aa6267fe3b4c44ac6fd5b082ed1c763b140` for Formal Run `40650170110` only; not inherited by `b06c1ac` |
| WORKTREE | `/workspace/Serea-g5-evidence-closure` |
| BRANCH | `codex/serea-g5-evidence-closure-20261028` |
| CURRENT_HEAD | `b06c1acd5f3ddb4f59f0cad6ae1a1d291b086671` before local harness fix commit |
| DRAFT_PR | None |
| CI_RUN_IDS | `40657482497` IME B/C matrix; `40657482550` Formal Android/Identity; `40657482463` Full; `40657482477` Fast; `40657482449` RustSec; `40657482501` Git Hooks; `40657482527` Native; `40657482541` SQLite |
| OWNER_APPROVAL_REQUIRED | Real Google/OAuth/account/data, real AI credentials or billing, production signing/distribution, Pixel 7a, main merge, final Beta GO |

## Lane A — independent real IME

The previous `40650170070` matrix failed before instrumentation because of a stale seed success marker; its JUnit methods were **NOT_RUN**. The current `40657482497` run passed seed, fresh-AVD, package, and APK-hash guards for each case and ran one JUnit method in every completed case.

- API 26 B artifact `12802301577` (APK build evidence) / `12803116378` (case evidence): 1 test, 1 failure, 0 errors, 0 skips. The Android-managed IME emitted `onStartInput` for `app.serea.android.isolated`. Opening `RealImeDiagnosticsProvider` then failed with a signature-permission `SecurityException`; composition operations and Compose state assertions were not reached.
- API 26 C artifact `12802653113`: 1 test, 1 failure, 0 errors, 0 skips. Same provider permission denial before B/C assertions. ZIP SHA-256 `4ed42d9df4d297db9b5a8cbc7b21009fcc4fbf2a90ac7558e3cc6892531b0a4b`.
- API 35 B artifact `12802623709`: 1 test, 1 failure, 0 errors, 0 skips. The provider wrapper reports `Could not connect to the test IME diagnostics provider`; it fails at the first `imeEvents()` call after `onStartInput`, before composition assertions. ZIP SHA-256 `796649eb7a0bb6be2200cdbfa8b0c8a12ea2b6ea899c3b7e329e8dead4e1a683`.
- API 35 C artifact `12803148298`: 1 test, 1 failure, 0 errors, 0 skips. The same provider connection exception occurs after `onStartInput` and before composition assertions. ZIP SHA-256 `a6825418b3dc7c29cfa16d9dc7835ae2cf331b93d2e6e8b3f062d3e58481eea6`.
- Completed cases recorded cleanup PASS: original Latin IME restored, Test IME disabled and uninstalled, package absent. The cases do **not** prove composition failure or product defect.

The isolated target variant had been reusing `src/debug/AndroidManifest.xml`, while the IME diagnostic signature permission was removed from that debug manifest. Historical source `029c5aa` had the permission request in that manifest. The fix under review moves the permission request into an isolated-only manifest while retaining the Process Death harness receiver. It does not add the permission or Test IME service to debug or beta manifests. The B/C provider helper now asserts `targetContext.checkSelfPermission()` before IPC and logs the result; the host script no longer tries to runtime-grant a signature-only permission.

Local verification on the root-owned worktree:

- `:app:processDebugMainManifest`, `:app:processIsolatedMainManifest`, `:app:processBetaMainManifest`: PASS.
- Merged manifest check: `DIAGNOSTICS` permission appears only in `isolated`; no Test IME component appears in debug, isolated target, or beta.
- Added `tools/verify_serea_real_ime_manifest_scope.py` as a build-workflow guard for the merged target/Test IME manifests; local invocation PASS. The diagnostic Workflow source guard now compares G5 changes against frozen #133 HEAD `b06c1ac` and allowlists the exact new G5 paths.
- `:app:assembleIsolated :app:assembleIsolatedAndroidTest :r25-ime-test:assembleDebug`: PASS.
- AndroidTest Kotlin compile, `bash tools/test_serea_real_ime_avd_guards.sh`, workflow YAML parse, manifest-scope guard, and `git diff --check`: PASS.
- Target, AndroidTest, and Test IME APKs share signer certificate SHA-256 `36c5b853ac2bf2a5fde88f6d93fb6a2aeb6a8888e725c06bcd45dba65924d8d8` in the local build. Local APK SHA-256: target `a21fd835beffc9254c55eea8f46b139c98363cbc300ca6f101982a69d303df46`; AndroidTest `dc1cd73dd2e9a3e4a10f809bd5e625b8f47e2766902f77276b210e0e9354363e`; Test IME `c65604f39498bbf144d5e4ead812e0fad5071a124a489d51f573c8d5c62cccac`. These local APK hashes are not CI-run artifacts and must not be confused with the old matrix APKs.

## Lane B — Cloud Egress

Agent B branch `codex/serea-g5-cloud-egress-20261028` HEAD `e62d16e34a4227d0ff99d2a6af6da9894e960baa`, tree `087147e32f712e978888c936b175842b88432441`, is pushed and clean. Its clarified regression matrix on unchanged product source `fc1c0527eea8ea92509935dc8721d24b27fd7f9d` ran 9 tests: 4 expected regression failures, 0 errors, 0 skips; the prior focused egress suite ran 72/72 PASS. Independent review confirmed current-turn `userMessage.sourceLineage` is omitted from generated assistant lineage. Current assistant classification remains `UNKNOWN`, and the test confirms 0 usage reservations, 0 credential reads, and 0 HTTP dispatches. The `PUBLIC` bounded-context case is hypothetical and is not an observed disclosure. The renamed exact-head JUnit is `attack-matrix-renamed-exact-head.junit.xml`, SHA-256 `a99f330ed982c36734613a3696ad921ba8708cdf1e4c7272db29040d5c98a021`; the original pre-fix JUnit remains unchanged.

Review requested clearer names for the whole-`TextFieldValue` replacement and retained-`ViewModelStore` tests. Agent B is updating its own branch. No Agent B changes are integrated yet. The minimal product fix is root-owned: union `userMessage.sourceLineage` into assistant lineage; keep Cloud disabled until same-HEAD focused tests pass.

## Lane C — On-device AI

PR #127 HEAD `a2bafeea77160e8e2746bdfdd0d309348ac04aa7` Run `40657508541` SUCCESS, artifact `12802681672` (SHA-256 `df06078f5c8b4164bd24e255d1df0e2df9069570e08c08eea1295f4f391161f8`). API 35 x86_64 Emulator inference passed 1/1, 0 failures/errors/skips, including load, single/multi-turn generation, native cancellation, recovery, close/reopen, and corrupt-model rejection. ARM64 native build succeeded; ARM64 runtime, Pixel 7a performance, and product Chat integration are **NOT_RUN**. PoC remains unintegrated and does not pass through production Router/Egress/Usage.

## Formal result / next actions

Run `40650170110` completed SUCCESS on the old exact SHA `029c5aa...`, including Formal Identity. It is not evidence for later heads. Run `40657482550` on `b06c1ac` was running its API 35 emulator suite at checkpoint time; no final conclusion may be inferred.

Next actions:

1. Commit the isolated-only permission fix in this G5 worktree; wait until current emulator workflows finish before dispatching the next B/C matrix.
2. Accept Agent B's clarified test-only commit after rechecking its source, exact JUnit, hashes, and remote HEAD; apply the minimal assistant-lineage fix in this worktree and run focused security tests.
4. Open a new Draft PR based on #132 only after the candidate is coherent. Do not edit or merge #132/#133/#126/#127.
5. Re-run the required and cross-cutting gates on the final exact G5 HEAD. No old-head PASS transfers.

Personal Beta remains **NO-GO**. Real Google services, real Cloud AI, Pixel 7a, production signing, distribution, and main merge remain unapproved/not run.
