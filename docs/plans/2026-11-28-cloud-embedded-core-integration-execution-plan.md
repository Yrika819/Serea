# Serea Cloud / Embedded Core Integration Execution Plan

**Owner authority:** Formal Serea development operating-model decision, effective 2026-11-28 JST. This plan is execution guidance, not a request for another design approval.

## Current verified baseline

- Worktree: `serea-v13-s1-01b-core-reliability`, HEAD `4269c79c7b5847edc31e27f61cec26dbc6b9a560`.
- PR #262 is OPEN / Draft, based on `s1-01a-embedded-core-preserve-20261123223142`; remote checks shown for the previously pushed HEAD do not validate the current dirty tree.
- Existing working-tree changes combine S1-01B reliability/campaign changes and bounded S1-02 Chat/Core composition changes. Preserve every pre-existing modification and evidence artifact; separate commits/branches by concern.
- S1-02's authorized synthetic Chat/Core runtime gate is accepted COMPLETE after campaign `s1bc3a5e21dd6`: exact UI method 1/1 PASS, `RUNTIME_PASS`, `CLEANUP_PASS`.
- Cloud's existing Ollama path remains gated by Egress → Usage → Dispatch. Real credentials, live provider activation, and personal-data transmission remain outside this development authorization.

## S1-01B residual classification

| Item | Integration impact | Action |
|---|---|---|
| Diagnostic/native-race tests share an owner/database and cleanup may be bypassed on setup failure | Blocks safe expansion of Android race verification; not a product API blocker | Fix isolation, ownership and failure-preserving cleanup before running these tests |
| Default/isolated variant artifact separation | Direct safety dependency for any composition change | Verify normal variants remain Mock/Fake-only and diagnostic Core wiring is opt-in; reject any leak |
| 16-KB `syntheticLifecycle` failure | Does not block the already-passing synthetic API35/4-KB Chat/Core path; remains a platform reliability/release gate | Preserve the historical FAIL. Diagnose with current evidence and one controlled retry only after test cleanup; do not relabel as PASS without a passing test result |
| Cancellation while native code is executing | Does not block the current Chat path, which performs bounded readiness calls and uses a cancellable provider seam; important reliability follow-up | Keep explicitly NOT_VERIFIED until a native-internal cancellation test exists |
| Android process death / SQLite integrity / Core-originated WAL recovery | Does not block the current in-memory diagnostic session; required before claiming durable Core-backed Chat history | Keep out of the current integration scope; implement before durable-history enablement |
| High pre-run side effects and package/manifest isolation | Blocks unsafe campaign installation, not host-only composition work | Preserve fresh private package names, APK identity/signature checks, emulator-only runner and final package cleanup; verify manifests before new runtime use |
| Campaign count caps and pre-instrumentation consumption | Development-tool friction; not a product acceptance condition | Remove fixed per-profile quotas. Keep unique IDs, identity/attestation, target AVD, data isolation and evidence. Record proven pre-instrumentation abort separately from started/unknown runtime; never recycle an ID or erase evidence |

## Execution sequence

1. Record the effective Owner operating policy in repository guidance and the local Serea storage addendum. Preserve storage-link, volume-identity, lock, data-isolation and destructive-operation protections that do not conflict with the decision.
2. Rationalize campaign tools: remove fixed count caps; make preparation, install, instrumentation-started/unknown, proven-not-started, pass/fail, and cleanup outcomes distinguishable. Add unit tests before implementation changes. Preserve all existing campaign ledgers and APK evidence.
3. Repair S1-01B Android test cleanup ownership and isolation with regression tests. Do not change the historical 16-KB FAIL record.
4. Integrate the Cloud `ModelChatPort` seam with the Embedded Core Chat composition without activating a live provider. Tests must use synthetic PUBLIC data and a local mock transport; continue enforcing Egress-before-Usage-before-Dispatch, fail-closed behavior, credential redaction and no default Cloud dispatch.
5. Verify the isolated Chat/Core runtime path and normal build variants separately. Use only the target configured worktree, shared Android lock, designated emulators, fresh campaign namespaces and retained evidence.
6. Reassess residual S1-01B items against actual dependencies. Move only non-blocking reliability work to follow-up; leave release/durable-history acceptance explicit.
7. Separate policy, S1-01B and S1-02/Cloud-Core changes into scoped commits and Draft PRs. Do not merge, change Draft→Ready, activate real AI/OAuth, use personal data, or touch physical devices.

## Completion evidence

- Tests run against the exact committed HEAD are reported with commands, counts, exit status and evidence paths.
- Normal variant and isolated variant manifests/artifacts are checked independently.
- Every runtime campaign records profile, AVD/serial, page size, unique package identities, APK hashes/signature, exact instrumentation result and cleanup inventory.
- S1-02 remains COMPLETE for its bounded synthetic runtime acceptance even while S1-01B has separate reliability backlog. S1-01B completion is decided only against its own acceptance criteria and explicit blockers.

## Execution results (2026-11-28 JST)

- **Owner-policy documentation:** added the effective Owner decision, linked it from `AGENTS.md`, updated the S1-02 decision with the later runtime PASS, and added the local storage-policy supersession. The host-local storage document remains uncommitted.
- **Campaign rationalization:** removed per-profile fixed issue-count gates from `~/.local/bin/serea-avd-campaign`; capacity uses a shared 8-GiB minimum plus measured/estimated peak reserve rather than a 33-GiB stop. Added `PREPARATION_ABORTED_NOT_EXECUTED` state recording and kept timeout/start ambiguity consumed. Temporary policy tests exercised 9→10 campaigns and 12-GiB capacity: 2/2 PASS. Detailed latest Python test counts appear below.
- **Embedded Core cleanup ownership:** added strict deletion of only UUID-named synthetic SQLite files/sidecars inside the private test cache; cleanup occurs only after owner release and successful close. Tests serialize the process-wide native owner. Full isolated unit suite includes 20 cleanup tests and passed.
- **Cloud/Core seam:** direct Cloud composition is represented by `GatedCloudModelChatPort`; the explicit Embedded Core composition accepts only that type. Default app composition remains Fake/Mock. Loopback synthetic integration verifies Core readiness → usage reservation → synthetic credential read → localhost HTTP dispatch, and verifies PERSONAL data is denied before usage/credential/dispatch. No external provider/API was used.
- **Verification:** `:app:testIsolatedUnitTest` 516/516 PASS (latest task UP-TO-DATE against that run); `:app:lintIsolated` PASS after declaring the isolated baseline-profile copy dependency for lint; isolated Android-test Kotlin compile PASS; Rust `diagnostic_native_gate` 2/2 PASS; host `synthetic_campaign_wal` 1/1 PASS; `cargo fmt --all -- --check` PASS; S1B campaign host tests 55/55 PASS; storage monitor/campaign helper suite 118/118 PASS; `tools/tests` 37 run, 35 PASS, 2 skipped. Debug/release manifest processing is UP-TO-DATE; isolated manifest processing was included in the unit/compile graph. Campaign-manifest host tests passed. CI at code head `45d7ad3216307cd1bcfce4d2b3f23e1cb2ab2e4f` passed 42/42 displayed contexts; API 26 and API 35 each passed the 103-test Required Set plus 2 additional tests (zero failures/errors/skips). S1B PR #262 at `3042d268fe59a596cbd1a9700f0ad84346fabffa` passed 43/43 displayed contexts after retrying a transient Maven Central 429. This execution record's later docs-only clarification does not modify the tested source or runtime behavior.
- A broad unrelated `tests/` discovery ran 181 tests but ended with 3 errors: two missing `PyYAML` imports and one pre-existing `_mounted_vault` mock returning a `Path` instead of a context manager in the release-signing CLI test. The changed S1B test subset passes; no unrelated release-signing code was modified.
- The Cloud/Core integration was exercised in GitHub CI on disposable API 26 and API 35 emulators through the required `ChatHttpE2ETest` loopback/MockWebServer cases; the complete 103-test Required Set and 2 additional tests passed on each API. This does not enable a real provider or establish the separate S1-02 synthetic-runtime acceptance beyond its own bounded evidence.
- A first Rust diagnostic run exposed shared-gate interference between parallel tests; a test-only mutex fixed the race and the rerun passed. No historical product/runtime result was rewritten.
- The generated generic `isolated` manifest with no campaign ID still contains INTERNET and unrelated harness components; it is **not** a campaign APK and is not safe to install. Campaign APK identity/manifest attestation remains mandatory. CI built standard debug/release/instrumentation artifacts and verified producer provenance; no dedicated campaign APK or Owner/production-signed APK was produced, and no persistent local AVD was used for the new Cloud/Core code.
- **Remaining non-blocking reliability work:** the historical API35/16-KB `syntheticLifecycle` remains FAIL/NOT_RETESTED; the Android end-to-end native-cancellation case remains NOT_VERIFIED (the Rust host gate proves close waits for a native status call, not caller cancellation); Android process-death/WAL recovery remains NOT_RUN (host synthetic WAL reopen passed); full regular/campaign artifact isolation for a newly signed integration APK remains NOT_RUN. These do not block the current Fake-first Cloud/Core seam, but remain before durable-history or release claims.
