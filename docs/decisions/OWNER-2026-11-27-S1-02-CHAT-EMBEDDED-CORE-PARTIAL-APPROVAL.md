# Owner decision: S1-02 Android Chat / Embedded Core limited implementation approval

**Date:** 2026-11-27

**Status:** Approved for the bounded implementation and synthetic verification below; this is not ADR ratification.
**Authority:** Owner instruction in the Serea v1.3 development session.

## Approved scope

- Android-only Chat / Embedded Core connection using the existing Chat UI, `ChatViewModel`, `ModelChatPort`, and synthetic Embedded Core adapter.
- Core initialization, state/schema readiness checks, explicit cleanup, and fail-closed error reporting.
- Deterministic Mock AI and synthetic conversation/messages only, including delayed-result cancellation behavior.
- Verification of synthetic conversation/message creation, completion, cancellation/error status and cleanup.
- Internal Android/Core responsibility boundary: Android owns Chat presentation, cancellation and turn status; the embedded Core adapter owns native Core lifecycle/state. No automatic Cloud/provider dispatch is introduced.

## Explicit exclusions

- Do not change the PC-facing `device/2` protocol, Core authority/permission boundaries, or imply that this Android path implements `device/2`.
- No real Cloud/network transport, API keys, OAuth, external transmission of personal data, formal durable Chat-history or encryption-policy decision, production signing, or public distribution.
- ADR-0044 remains **PROPOSED / NOT RATIFIED**. This decision does not ratify ADR-0044 as a whole.
- S1-01B completion is not a prerequisite for this bounded S1-02 work. S1-01B remains subject to separate Owner completion judgment.

## Change ownership in the shared worktree

The following existing changes remain owned by S1-01B and are not part of S1-02: Android diagnostic build/signing restrictions and campaign install runner/tests; native bridge ownership/diagnostic-gate changes and their tests; and the synthetic Core WAL test. A Gradle DSL compile error discovered by the newly approved JDK 17 `:app:help` run is also an S1-01B correction in `android/app/build.gradle.kts`.

S1-02 changes are limited to this decision record, `android/app/src/embeddedCore/java/app/serea/android/EmbeddedCoreModelChatPort.kt`, and `android/app/src/embeddedCoreUnitTest/java/app/serea/android/EmbeddedCoreModelChatPortTest.kt`. The tests use only the Fake Core native seam, the existing Mock AI, and an ephemeral in-memory `ConversationHistoryPort` fixture; they do not open the app's durable Room DB. Do not fold these changes into S1-01B or touch `device/2` protocol code under this decision.

## Verification record (2026-11-27)

- JDK 17 was applied only to the target Gradle processes. The final unsigned `:app:help` execution passed (`BUILD SUCCESSFUL in 53s`); command fingerprint `d3809ab5389b98cb865a1da6a0097ab8d4b2787210803188e72820d77d369eca`, `app/build.gradle.kts` SHA-256 `672516dea970cf93b3dfa3bc78559ea6ef85c3e36a1b6dd941d48d5bb33f30ec`, init guard SHA-256 `33bdfcad68764034cfcdd54e4fea5517f23c6156577ca56599145f48c8e83c8e`, recipe SHA-256 `b46eb633d405f6b78fd7dd4f128e6845a8d28c8be2321c635f4b86bfc6aea448`, and JDK executable SHA-256 `a59ee18937d6bf8eb0d1523f0f1e245bb9608e38eead7f6bec284bb0c9d65aeb`. Fixes were limited to using `TaskExecutionGraphListener` in the Kotlin DSL and replacing Groovy-incompatible `rows.single()` with an explicit `size()==1`/`rows[0]` check in the S1B-only help init script. The original init script was preserved as a non-loaded backup. All earlier consumed help activation pairs were retained under unique versioned names.
- `:app:assembleIsolated -PsereaEmbeddedCore=true` reached `BUILD SUCCESSFUL`; unsigned artifact path: `android/app/build/outputs/apk/isolated/app-isolated-unsigned.apk`. Its wrapper receipt is incomplete (`OBSERVATION_ERROR`/sampling timeout despite Gradle exit 0), so this is a build result, not a complete resource-observation record. No signing pin, install, AVD or runtime operation occurred.
- `:app:testIsolatedUnitTest` first ran 507 tests with 12 failures. A failure-filtered run completed 30 cases with 29 passing and one Chat success case failing; subsequent isolated Chat-class run had 2 passes and 3 failures, exposing asynchronous test-scheduler/ViewModel cleanup assumptions and the expected post-dispatch `OUTCOME_UNKNOWN` status. The test source was then changed to pump worker callbacks, clear its `ViewModelStore`, and assert the post-dispatch status. These latest test-source edits have **not** been verified by a rerun.
- The helper now permits only exact offline, isolated, Mock/Core unit-test recipes. Resume preflight: `android-common check` passed; no emulator/ADB process or lock holder was present. However an unrelated Gradle 9.4.1/JDK 21 daemon and Kotlin compiler daemon were still running idle; the existing `build_observer.active_build()` treats any Java process as active. They were not stopped or changed. Do not bypass that gate; retry the exact affected Chat test class only when those processes exit or an Owner-approved safe execution path is provided.

## Owner follow-on instruction (2026-11-27)

This follow-on supersedes only the earlier S1-02 file-ownership limit above. It does not relax the explicit exclusions (no durable personal Chat history, real AI/network, OAuth, signing/distribution, or `device/2` protocol changes).

- Authorized an isolated-only Chat UI composition using the existing `ChatScreen`, `ChatViewModel`, `EmbeddedCoreModelChatPort`, and Mock AI. The opt-in is detected only in the `isolated` build when the provider compiled under `-PsereaEmbeddedCore` is present; normal variants and isolated builds without that source continue through `DemoChatComposition`.
- The diagnostic session uses only a mutex-protected in-memory history port, no Usage repository, Room history, credentials, task screen, or model settings. Provider construction/linkage failure stays on the diagnostic screen and returns a fail-closed local error; it must not fall back to Demo or any external provider.
- A `ViewModelStore`-scoped session owner reuses the Core adapter across Activity/Chat ViewModel recreation and requests explicit cleanup on final owner clear. Android lifecycle runtime verification remains separate from host-side tests and compile.
- The earlier idle-daemon restriction is superseded by Owner approval for normal shutdown of verified idle Gradle/Kotlin daemons, without a per-process reapproval; an active build or unrelated process must still not be stopped.
- Authorized S1-02 source/host-test changes include `ChatSessionComposition.kt`, `MainActivity.kt`, `EmbeddedCoreDiagnosticChatSession.kt`, and the isolated host/UI test sources. Cloud C-01 files remain out of scope.

## Verification update (2026-11-27)

- Final `EmbeddedCoreModelChatPortTest`: 5/5 PASS after the Activity/ViewModel owner and cancel-on-finish changes.
- `EmbeddedCoreDiagnosticChatSessionTest`: 5/5 host tests PASS across targeted executions (memory-only turn lifecycle, owner reuse, normal-path selection, fail-closed provider construction, and opt-in provider class lookup).
- `:app:compileIsolatedAndroidTestKotlin`: BUILD SUCCESSFUL; UI instrumentation source compiled, and the isolated native Core artifacts built for x86_64 and arm64-v8a. No APK install or instrumentation execution occurred.
- `:app:compileDebugKotlin`: BUILD SUCCESSFUL with the Core opt-in unset; the final run had a complete resource-observer receipt. This confirms the default `debug` build compiles without adding a Core opt-in BuildConfig switch.
- S1-01B offline host safety suites: 45 tests PASS; tools manifest/signing/runner suite: 11 PASS, 2 merged-manifest checks SKIPPED because no merged manifests were supplied. Synthetic APK signatures used disposable test-only keys; no approved pin or production signing key was used.
- `EmbeddedCoreDiagnosticChatUiTest` was compile-checked, not run. Android runtime remains NOT_RUN under the local AVD method whitelist; S1-02 host work is complete, while runtime completion remains separate.

## Owner follow-on and bounded runtime completion (2026-11-27)

The Owner's formal development-operating-model decision supersedes the former local campaign whitelist/cap and accepts the authorized S1-02 runtime gate as a separate bounded completion criterion. It does not authorize real AI/network, personal-data egress, credentials, OAuth, durable personal Chat history, `device/2`, production signing/distribution, physical devices or main merge.

- Campaign `s1bc3a5e21dd6`, API35/4KB `serea_s1_01a3` / `emulator-5554`, executed exactly `app.serea.android.EmbeddedCoreDiagnosticChatUiTest#diagnosticChatUsesCoreMockAcrossRecreationAndCleansUp`.
- Instrumentation: **1 test, PASS**, `OK (1 test)`; result SHA-256 `b22f13fe0b2caadfcf6522eaed52bd44edfe1b01c51508303b417463addefab9`.
- Ledger: `RUNTIME_PASS`, cleanup: `CLEANUP_PASS`. All 72 recorded ADB stages exited 0; final package inventory excludes both campaign packages. Evidence and package/manifest/APK attestations are retained in `~/Documents/SereaDevelopment/Architecture/v1.3/avd-campaigns/s1bc3a5e21dd6/`.
- **S1-02 bounded synthetic Chat/Core runtime gate: COMPLETE.** This is not a claim that Cloud/live inference or broader Android release criteria are complete. S1-01B remains independently assessed and may retain reliability backlog without downgrading this completed S1-02 gate.
