# G10–G15 Execution Plan (targets, not guarantees)

## G10 (Nov 11–20): inference route and integration prerequisites
1. G10-A is the first implementation unit: connect the existing `LocalModelChatAdapter` behind the test-only default-off selector, with a deterministic injected Fake Backend. Keep `MainActivity` on Demo. See `g10-a-implementation-evidence.md`; no historical adapter stack is cherry-picked.
2. G10-B is gated on an Owner-selected model/runtime/license and a dedicated disposable ARM64 device. Only then add a lifecycle/eviction bridge, fresh consent after eviction, and memory/thermal/ABI validation. Do not bundle a model before those decisions and checks.
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

## G9-R4 and G10-A implementation decision

The G9-R4 audit closed the pre-spike question: main already has the reusable `LocalModelChatAdapter`, typed single-use consent registry, Egress/Usage wrappers, and extensive adapter lifecycle tests. Candidate PRs #165/#166/#169/#178/#179/#182 concern real-runtime limits and ARM64 validation; #185 concerns consent after actual eviction; #195/#198 concern memory-trim integration. These do not block a Fake Backend-only composition spike and are not imported wholesale.

G10-A is now the bounded default-off composition described in `g10-a-spike-spec.md`. It adds a Debug/isolated-only selector and Android test composition; normal `MainActivity` remains Demo. It must prove focused JVM and API 26/35 test behavior, preserve the existing 105 formal identities, and include a real force-stop boundary for consent invalidation. It does not prove model inference, ABI compatibility on Pixel hardware, memory eviction/reload, or Personal Beta readiness.

The 175-snapshot PR candidate extract remains a historical search aid, not an implementation queue. Revisit its Local Model candidates only when scoping G10-B or real inference. G10-A completion is not approval to enable a model or provider.
