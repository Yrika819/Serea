# G5 Orchestrator Ledger

## Checkpoint — 2026-10-28

| Field | State |
|---|---|
| ACTIVE_PHASE | G5 evidence closure; exact-head reruns pending |
| GOAL_STATUS | PARTIAL — IME provider permission regression isolated and patched locally; Cloud lineage fix focused-pass locally; API 35 isolated inference PoC validated on its own PR |
| AUTHORITY_SHA | PR #132 `373b8be15f407d951157d2a80a632e86d979acdf`; PR #133 `b06c1acd5f3ddb4f59f0cad6ae1a1d291b086671` (base PR #132); main `e0acb0254820c381dd97828824bef86a1db5176c` |
| LAST_GOOD_SHA | Formal run `40650170110` SUCCESS on `029c5aa6267fe3b4c44ac6fd5b082ed1c763b140` only. G5 focused egress tests pass locally on `3ba450ec1d6c4adf0b29daf42909e28fa242f087` only. Neither result is inherited by later commits. |
| WORKTREE | `/workspace/Serea-g5-evidence-closure` |
| BRANCH | `codex/serea-g5-evidence-closure-20261028` |
| CURRENT_HEAD | `dd236abf75c26b8ba8f4c5db7c696b7b380a1548`; tree `2ef2610a320cfdab8e3f3e74a6170430848b27a6`; local checkpoint before preserving the new exact-head evidence |
| DRAFT_PR | None yet; intended base `codex/serea-g4-final-integration-candidate-20261027` (PR #132 branch) |
| CI_RUN_IDS | Old-head Formal `40650170110` SUCCESS; PR133 IME matrix `40657482497` FAIL; PR133 Formal `40657482550` FAIL; Agent C API35 inference `40657508541` SUCCESS; PR133 Full `40657482463`, Fast `40657482477`, RustSec `40657482449`, Git Hooks `40657482501`, Native `40657482527`, SQLite `40657482541` SUCCESS on `b06c1ac` only |
| ARTIFACT_IDS | IME build `12801519343`, build evidence `12802301577`; API26 B `12803116378`, API26 C `12802653113`, API35 B `12802623709`, API35 C `12803148298`; Formal API26 `12803068069`, API35 `12802953852`; on-device AI `12802681672`; Cloud local exact-head evidence under `docs/agent-reports/evidence/g5-cloud-egress-20261028/after-lineage-fix-3ba450e/` |
| OWNER_APPROVAL_REQUIRED | Real Google OAuth/account/data, real Cloud AI credentials/billing, production signing/distribution, Pixel 7a, main merge, final Beta GO |

## Lane A — independent real IME

### Prior seed failure

The earlier matrix Run `40650170070` failed its fresh-data seed guard. Its four requested JUnit methods were **NOT_RUN**. It does not describe IME behavior.

### PR #133 exact-head matrix

Run `40657482497` used PR #133 source HEAD `b06c1acd5f3ddb4f59f0cad6ae1a1d291b086671`. The APK build/provenance job passed, and all four case jobs passed source/APK identity, fresh AVD/API, seed/campaign, target-package, installed-hash, and IME selection guards. Each requested one JUnit method and recorded 1 failure, 0 errors, 0 skips:

| Case | Artifact | Observed first failure | IME input assertions |
|---|---:|---|---|
| API 26 B | `12803116378` | Signature-protected Test IME diagnostics provider denied target access | NOT_REACHED |
| API 26 C | `12802653113` | Same signature permission denial | NOT_REACHED |
| API 35 B | `12802623709` | Diagnostics provider connection unavailable after `onStartInput` | NOT_REACHED |
| API 35 C | `12803148298` | Same provider connection failure after `onStartInput` | NOT_REACHED |

Root Cause category: **TEST_HARNESS**. The trace and manifests show the isolated target did not request the provider's signature permission. The Android-managed Test IME did receive `onStartInput`, but the harness could not read diagnostics. These results neither pass nor fail the actual composing operations and do not establish a product defect. Each case reported cleanup PASS: restore Latin IME, disable/uninstall the Test IME, and verify the package absent.

### Root-owned harness correction

Commit `abdc320a09ebc6d69bc32a0cacd42609f925ce26` adds `src/isolated/AndroidManifest.xml`, keeping the signature permission request within `app.serea.android.isolated`. The Test IME provider remains signature-protected; the isolated target requests the permission; debug and Beta do not. `ProcessDeathHarnessReceiver` remains protected by DUMP. The workflow compares the signer of target, instrumentation, and Test IME APKs. The host script records that runtime `pm grant` is not applicable for a signature permission, and the instrumentation verifies `targetContext.checkSelfPermission()` before provider IPC.

Preliminary local checks passed: debug/isolated/beta merged manifest scope, APK assemblies, AndroidTest Kotlin compilation, AVD guard script, YAML parsing, manifest-scope verifier, and `git diff --check`. The local signer certificate SHA-256 was `36c5b853ac2bf2a5fde88f6d93fb6a2aeb6a8888e725c06bcd45dba65924d8d8`. The earlier local APK hashes recorded during this check were from a pre-lineage-fix build and are **not** CI evidence or evidence for the current HEAD. Required next: rerun B/C on the final pushed G5 HEAD, verify all four exact JUnit IDs, connection/editor identity, composition, selection, APK hashes, and cleanup. No post-fix IME case has run yet.

## Lane B — Cloud Egress lineage

Agent B's branch `codex/serea-g5-cloud-egress-20261028` is pushed at HEAD `e62d16e34a4227d0ff99d2a6af6da9894e960baa`, tree `087147e32f712e978888c936b175842b88432441`. Its matrix exposed that the current provider-origin `userMessage.sourceLineage` was omitted from the generated assistant lineage. On the unchanged product candidate `fc1c0527eea8ea92509935dc8721d24b27fd7f9d`, the clarified attack matrix ran 9 tests with 4 expected regression failures, 0 errors, 0 skips; prior focused egress tests were 72/72. The matrix showed the current assistant remained UNKNOWN and the Cloud gate denied before reservation, credential read, and HTTP dispatch. The bounded-context PUBLIC assertion was a hypothetical regression test, not an observed disclosure.

Agent A independently reviewed the final root patch and accepted it. Root commit `9b9e7fa53b744c991fa575e73ba739b2d0c66553` adds current `userMessage.sourceLineage` to the generated model-output lineage while retaining `MODEL_OUTPUT`, `USER_AUTHORED`, prior context lineage, and UNKNOWN classification. A first focused run observed the stronger `SOURCE_LINEAGE_DENIED` result instead of `CLASSIFICATION_DENIED`, evidence that Google source was retained and denied. The assertion was updated to the stronger denial reason; commit `3ba450ec1d6c4adf0b29daf42909e28fa242f087` adds an explicit Gmail-origin assertion across bounded context.

Exact local focused suite at `3ba450e` passed: 81 tests, 0 failures, 0 errors, 0 skips. Its Gradle log and JUnit XMLs are preserved under `docs/agent-reports/evidence/g5-cloud-egress-20261028/after-lineage-fix-3ba450e/`. Agent A's independent source review was ACCEPT at the same HEAD. Then at exact HEAD `dd236abf75c26b8ba8f4c5db7c696b7b380a1548` / tree `2ef2610a320cfdab8e3f3e74a6170430848b27a6`, the same focused suite passed again: 81 tests, 0 failures, 0 errors, 0 skips. The same run built isolated target, instrumentation, and Test IME APKs; compiled AndroidTest Kotlin; passed debug/isolated/beta manifest generation, manifest-scope verification, and AVD guard tests. All three local APKs had the same signer certificate SHA-256 `36c5b853ac2bf2a5fde88f6d93fb6a2aeb6a8888e725c06bcd45dba65924d8d8`; local APK digests and the Gradle/JUnit evidence are recorded under `docs/agent-reports/evidence/g5-cloud-egress-20261028/after-lineage-fix-dd236ab/`. This remains local evidence, not CI, and is not transferred to a later commit.

Security residuals: `prefillEditorFromProvider` currently has no production call site, so a future Gmail/Calendar-to-Chat UI handoff must pass the correct explicit origin. Arbitrary clipboard or pasted text provenance is not inferred. Account/session generation binding across a real Google handoff is not covered. No disclosure was observed. Real Provider and Cloud AI remain **DISABLED** until production handoff lineage, authorization binding, and same-head security evidence are complete.

## Lane C — On-device AI

PR #127 HEAD `a2bafeea77160e8e2746bdfdd0d309348ac04aa7`, tree `25874a7f3fc53028ae72aa7ddf4cc42a5e823f70`, Run `40657508541` SUCCESS, artifact `12802681672` (SHA-256 `df06078f5c8b4164bd24e255d1df0e2df9069570e08c08eea1295f4f391161f8`). The API 35 x86_64 Emulator test passed 1/1 with 0 failures/errors/skips for model load, single/multi-turn generation, native cancellation, recovery, close/reopen, and corrupt-model rejection. The run built ARM64 and x86_64 native targets. API26 Run `40589226410` passed on older source `8d4c5cf...`, not current PR #127 HEAD. ARM64 runtime, Pixel 7a performance, and product Chat integration are **NOT_RUN**. The PoC remains isolated and is not integrated into Serea Router/Egress/Usage.

## Formal Required and Identity

Old-head Run `40650170110` completed **SUCCESS** on exact SHA `029c5aa6267fe3b4c44ac6fd5b082ed1c763b140`. All jobs succeeded, including Formal Identity, Android Emulator Required Gate, storage/emulator, and Pixel 7a CI bundle. This success does not transfer to later heads.

Run `40657482550` on PR #133 HEAD `b06c1ac` completed **FAILURE**. Focused Chat failed while opening the same diagnostics provider. Full Required 105-test suites did **NOT_RUN** on API26 or API35; Identity was **SKIPPED**. Storage/Emulator failed. Build/Lint, Unit, Integration, Security, Dependency Audit, Documentation, and Pixel 7a CI bundle succeeded on `b06c1ac`. Other cross-cutting runs on `b06c1ac` (Full, Fast, RustSec, Git Hooks, Native, SQLite) succeeded but do not transfer to G5.

## Agent review status

| Lane | Branch / HEAD | Review | State |
|---|---|---|---|
| A — Real IME | no independent source branch; root harness fix at `abdc320`, current combined HEAD `3ba450e` | Provider permission issue classified TEST_HARNESS; source fix awaits post-fix B/C. Cloud patch independently ACCEPTED by A at `3ba450e`. | Post-fix B/C NOT_RUN |
| B — Cloud Egress | `codex/serea-g5-cloud-egress-20261028` / `e62d16e` | Test-only regression patch; independently checked; root fix is separate and narrowly scoped. | Integrated in root branch; exact local tests PASS at `3ba450e` |
| C — On-device AI | PR #127 / `a2bafee` | Evidence reviewed; no product integration. | API35 x86_64 PoC PASS; product use NOT_RUN |

## Next actions

1. Preserve this checkpoint and local exact-head Cloud evidence in a root-owned commit.
2. Push the root-owned branch and create a new Draft PR based on PR #132's branch; do not modify or merge PR #132/#133/#126/#127.
3. On the pushed exact HEAD, run independent IME B/C and inspect each JUnit, Android-owned connection, target editor identity, composition/selection, APK digests, and cleanup. If still failing, classify from the new trace before changing code.
4. Run Formal Required and cross-cutting CI only on the stable exact candidate HEAD. No old SHA result transfers.
5. Keep real Google, Cloud AI, production signing/distribution, Pixel 7a, main merge, and Personal Beta final GO as Owner-controlled/not run.

Personal Beta remains **NO-GO**.
