# G10–G15 Execution Plan (targets, not guarantees)

## G10 (Nov 11–20): inference route and integration prerequisites
1. Map Android ChatViewModel → adapter → Rust router/FFI, using source/tests; no provider credentials. Resolve #191/#193 before reuse; compare exact patch and tests. If usable, adapt in a focused PR; otherwise implement a default-off adapter with fake runtime tests.
2. Define inference contract, cancellation, load/unload, memory pressure, background resume, ARM64 ABI and model license/size constraints. Do not bundle a model until license and Pixel 7a memory/thermal measurements pass on a dedicated device.
3. Keep Calendar fake. Document OAuth client, scopes, data policy and account choice; await owner decision before real consent/API. Then separate PRs: OAuth authorization, read-only transport/pagination, UI/error/revoke, device acceptance.
4. PR #208 is a CI optimization candidate only; require comparison of workflow outputs/required checks before any reuse.

## G11 (Nov 21–30)
Tasks persistence/recovery and reminders; formal and dedicated-device tests. Calendar read-only only if owner authorization landed. Gmail stays disabled unless restricted-scope decision and review precede implementation.

## G12 (Dec 1–10)
Feature freeze; API 26/35 formal tests and dedicated clean-device acceptance; process death, IME, rotation, offline, battery/crash, permission revoke, deletion.

## G13 (Dec 11–17)
Independent security review: Keystore, OAuth, egress, logs, dependencies/SBOM, Actions supply chain, data deletion.

## G14 (Dec 18–24)
RC build reproducibility/provenance, package/version checks, rollback rehearsal. Signing and Firebase distribution NOT_RUN pending owner approval.

## G15 (Dec 25–31)
Owner GO/NO-GO using acceptance evidence; no automatic GO implied.

Critical path: owner model/provider/data-policy choice → adapter contract → (approved runtime) Android connection → device acceptance → security review → RC. Calendar path is independent: owner OAuth/scopes/policy → OAuth client → read-only API → account/revoke/device verification → review. Release path: protected environment and owner key → provenance/package validation → authorized distribution.

For every task, retain focused tests plus formal 26/35 JUnit, native/SQLite checks as applicable; completion requires tests to execute and evidence to name the tested SHA. Key risks are model licensing/memory, provider approval, emulator-only confidence, and release credentials.

## G9-R3 decision gate update

Do not start G10 inference implementation by rebuilding the adapter. First perform a narrowly scoped integration spike PR: compose existing `LocalModelChatAdapter` behind current Egress/Usage wrappers and the explicit local consent contract, with a fake `LocalModelInferenceBackendFactory`; keep it default-off and never alter `MainActivity`'s default Demo route. This must establish route selection, cancellation, history restoration behavior, and fail-closed egress. Add focused JVM tests for explicit selection/consent and Android ViewModel integration tests for send/cancel/process recreation; then run API 26/35 Formal Required Gate. No real model or provider required.

Before that spike, resolve candidate PRs #165/#166/#169 (native runtime limits/ARM64 capability), #178/#179/#182 (ARM64 emulator harness), #195/#198 (memory trim/lifecycle), and #185 (consent after eviction) from the existing 175 snapshot. They may constrain or duplicate the spike. Candidate extraction across the 175-record snapshot finds 116 keyword/path candidates; machine-readable result is `g10-pr-candidates.json`. The listed native/lifecycle candidates are still REVIEW and affect safe G10 scope, so READY_FOR_G10 is blocked until at least these are checked against main and each other.

G10-A details are specified in `g10-a-spike-spec.md`. Native runtime constraints and ARM64 guest tooling from #165/#166/#169/#178/#179/#182 are subsequent gates for real inference, not prerequisites for a Fake Backend spike. Fresh single-use consent after eviction (#185) is a pre-spike acceptance invariant; optional trim-memory callback #195 and its Android test #198 can follow, while the spike must still test adapter cancellation/close/eviction serialization directly.
