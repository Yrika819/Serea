# C-09C — Fixed synthetic Ollama Cloud probe candidate

## Owner authorization and boundary

Based on accepted integration HEAD `9c14f02a54be8a448eb731a74cb856b552116a5b` and Draft PR #277. Implement only a one-shot, explicitly confirmed, public synthetic probe to Ollama Cloud `gemma4`. Do not register/read a real API key, contact Ollama, bill an account, send conversation/history/Memory/Calendar/Gmail/personal data, use a physical device, production-sign, mark Ready, or merge.

General external Cloud chat remains denied; ordinary Chat stays Demo-first. The probe is the only exception and must match fixed model, destination, content, provenance, empty context, and approved limits at the composition gate and transport boundary. Test transports use MockWebServer loopback only.

## Implementation sequence

1. Add failing tests for: ordinary external Cloud Chat denied before Usage/credential/dispatch; exact fixed probe accepted while altered content/context is denied; the live test path stops at a fake credential barrier before any socket; loopback wire request has `Authorization: Bearer <synthetic-key>` while the key is absent from JSON; catalog/price freshness uses an injected clock; receipt shows settled token/cost usage; Settings displays the exact plan and requires hardware-backed key, Owner free-allowance confirmation, fresh model/price, and a final explicit confirm.
2. Implement a typed fixed-probe intent carried from `CloudCandidatePort` through Egress-approved payload. Keep general external dispatch disabled. Enforce exact shape before Usage in the outer gate and again before credential/socket in registry/adapter. Preserve one-request/no-retry/no-fallback, provider limits, cap accounting, and key redaction.
3. Return a probe receipt with send ID, bounded reply, and settled Usage ledger data; render the planned payload, `Authorization`-header distinction, model/price evidence/expiry, result, tokens, and estimated cost in Settings. Use the same injected clock for model freshness, price attribution, and deterministic tests.
4. Add one exact-method Android instrumentation test using the real embedded Core JNI adapter, in-memory Usage storage, synthetic credentials, and loopback MockWebServer. Preserve the runner's exact-method guard and test only that new method; do not use real API credentials or endpoint.
5. Run focused tests red→green, the full isolated unit suite, Core+Cloud method on a fresh approved emulator campaign if host preflight/runner attestation succeed, regular API 26/35 Formal and variant checks, Rust checks as affected, docs validation, and build an unsigned-for-production Debug candidate APK (Gradle debug signing only). Do not install the candidate or execute its external probe.
6. Review the diff and check evidence, commit/push only the assigned candidate branch, update PR #277 in Draft, and report APK path/hash, exact HEAD/Tree, same-SHA Required checks, retained non-Required failures, and remaining Owner gates. No Ready or merge.

## Acceptance

- The only external-capable request is the one exact `gemma4` fixed public probe after Owner confirmation and a one-shot Settings action.
- Any ordinary chat, changed prompt/context, stale model/price, missing hardware-backed credential, missing free-allowance confirmation, quota exhaustion, or invalid provenance fails closed before Usage/credential/network as applicable.
- Tests prove request ordering, no fallback/retry, no header/body secret confusion, settled Usage receipt, and Core cleanup. All HTTP tests are loopback; no actual Ollama request is issued.
- Candidate APK is built and hashed, not installed or used to submit a probe.

## Prior CI flake check

On the accepted HEAD, the first API 26/35 Formal attempt had two UI assertion failures; a same-SHA rerun passed, and API 26 focused E2E later passed after a pre-test Maven HTTP 429. Preserve those attempts. Only make an additional UI synchronization change if the failure reproduces or code inspection proves a race; do not mask assertions.

## Implementation and validation status (2026-11-28)

- Implemented a typed `FIXED_SYNTHETIC_PROBE` purpose, exact prompt/model/destination/provenance/empty-context checks before Usage and again at the provider boundary. General external Cloud chat remains disabled. The probe receipt includes response, send ID, and the Usage ledger row; model/price checks use one injected clock.
- Added Settings UI with visible fixed text, verified price and expiry, Ollama free-allowance confirmation, hardware-backed credential and embedded-Core wrapper gates, and a separate final one-shot confirmation. The probe response/estimated ledger cost stay in Settings, not Chat history. Diagnostic Cloud wrapper shares the existing Embedded Core Chat session owner.
- TDD evidence: initial fixed-probe test failed before the dispatch gate (`reservationAttempts=0`); after implementation, the focused cloud tests pass. Full isolated unit suite: 545 tests, 0 failures/errors/skips. `compileIsolatedAndroidTestKotlin` succeeds, including the exact-method native Core + loopback MockWebServer integration test.
- The native instrumentation method is deliberately **NOT_RUN**. The existing host installer `tools/s1b_campaign_install.py` and local campaign planner have no registration for this new exact method, and the current active diagnostic signing review does not authorize it. No AVD campaign was reserved; no APK was installed; no ADB instrumentation was started. Do not bypass those gates. Required next step for runtime evidence: Owner-approved signer/runtime-plan entry and corresponding exact-method registration in the host campaign tooling.
- The isolated target APK and Android-test APK were built only (not installed): `android/app/build/outputs/apk/isolated/app-isolated-unsigned.apk` SHA-256 `92aa9cfd259ca7267952b46679021122bebdae5422c3d04fae023ebe145e3e9b`; `android/app/build/outputs/apk/androidTest/isolated/app-isolated-androidTest.apk` SHA-256 `fa9ffbeb50f6b6bf92024e231b6fa8fd4b5d156c264e0d6c2f7200cd1f49bb22`. `apksigner verify` confirms both are unsigned. Previous APK/metadata outputs were preserved before Gradle replaced the default output set at `/Volumes/Tahoe - Data/Users/yuta/DevelopmentStorage/SereaEvidence/c09c-pre-assemble-20261128/` (mode 0700; files mode 0600).
- No real credential was read or stored; no Ollama/external request, account billing, physical-device operation, or production signing/distribution occurred. All executed transport tests used MockWebServer loopback.

## Owner follow-up authorization — continue (2026-11-28)

Owner has accepted the implementation and explicitly authorized the exact Core/loopback test below, use of the existing `S1B_DIAGNOSTIC_2026_11` diagnostic signing generation, a new isolated API35/4KB campaign, install/run on `serea_s1_01a3`, and campaign-scoped cleanup. New synthetic methods no longer require per-method Owner approval; exact method filters and all campaign/signing/data-isolation guards remain required. No external provider, real key, billing, personal data, physical device, main merge, or Ready transition is authorized.

Execution checklist:

1. Register `app.serea.android.EmbeddedCoreCloudChatIntegrationTest#fixedCloudProbeCrossesRealCoreBeforeLoopbackRequestAndSettlesUsage` in the existing exact allowlists and development runtime plan; keep other methods exact and unchanged.
2. Refresh source/build/APK pins and the active campaign review without changing signer generation or certificate; preserve prior candidate/approval/APK evidence.
3. Prepare one `api35-4k` campaign via the existing helper; verify serial, API/4KB AVD, unique uninstalled target/test namespaces, package/signer identity, and fresh private state before installation.
4. Build the campaign pair with the existing diagnostic signer; record the host install-attempt state before ADB install; run only the exact method with loopback MockWebServer and the runner guard.
5. Preserve evidence, perform only campaign-package cleanup through the approved helper, and verify unrelated packages/data remain untouched.
6. Run the registered local/CI verification, then commit and push the authorized changes to PR #277 while keeping it Draft. Do not register or run any real-network test.
