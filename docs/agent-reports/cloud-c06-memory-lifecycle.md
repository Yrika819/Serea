# Cloud C-06 — Memory lifecycle and recovery

## Scope and base

Branch `cloud/serea-v13-c06-memory-lifecycle` is based on C-05 HEAD `61c718e2350237a9889d6ca3d10a8d1d747dd870`. This is development-only, synthetic P7 evidence. ADR-0043's development boundary is preserved; ADR-0044 remains PROPOSED / NOT RATIFIED.

## Implementation

- Replaced the extraction path's `Drop`-only counter release with persistent owner/request admission rows in the test-only P7 extraction module. They capture request input digest, P7 ledger generation, admission/expiry, daily UTC slot, and terminal state. Existing P7 resource accounting remains authoritative for the accepted one-active/20-starts-per-UTC-day ceilings. A test-only provisional 60-second lease expiry is explicit; expiry does not refund a daily start.
- Admission and accounting updates serialize in the existing SQLite transaction. Stale generations are fenced. Final generation/readiness validation, the full candidate batch, P7 provenance/FTS trigger writes, and COMMITTED state are in one transaction. Candidate validation happens before writes. A failed storage transaction rolls back the whole batch; abandoned ACTIVE leases are reclaimed only after expiry.
- Request replay is tied to the synthetic input digest and candidate digest. Reusing a request ID with different input or output is refused. Candidate duplication, prior conflicting identity, and tombstoned IDs fail closed. Forget's generation transition and extraction's final commit serialize on the same writer transaction, so an old extraction cannot save after Forget advances the generation.
- The existing P7 Memory tables, triggers, quota, FTS, tombstones, recovery barrier, and Owner boundary are reused. The new admission tables are created only by code inside the test-only `p7_conversation_extraction` module; no production migration or ordinary feature schema is added. No Android/Core/UI integration or automatic model-context injection is introduced.

## Verification

- C-06 extraction tests: **13/13 PASS**. Includes two independent file connections admitting at most one active extraction, daily limit retention after expiry, actual child-process termination during the candidate transaction followed by file reopen/expiry recovery, transaction rollback after the first candidate, replay/conflict/tombstone handling, and Forget generation fencing.
- P7-enabled `serea-storage` suite: **623 unit + 2 foundation integration + 2 Gate C lock integration + 35 doctests PASS**. The two pre-existing manual resource benchmark tests remain ignored by their annotations.
- Default-feature `serea-storage` suite: **511 unit + 2 foundation integration + 35 doctests PASS**. P7 experimental code remains disabled in this configuration.
- `cargo check` default and with `p7-experimental,p7-gate-c`, `cargo fmt --all -- --check`, and `git diff --check`: **PASS**. Only the existing vendored `serde_json` deprecated `usize::max_value` warning appears.
- No actual conversations, personal data, provider traffic, API keys, OAuth, production memory, or model context were used.

## Android Formal inventory finding (proposal only)

Run `45976772516`'s API26/API35 Formal source inventory guard counted 14 methods in `UsageLedgerDatabaseTest`, while the original required manifest contains 12 methods for that class and the additional manifest contains only `fakeProviderBridgeRoutesApprovedDataAndKeepsCostAndUnknownOutcomesDurable`. The missing identity is `providerInputAboveReservationEstimateSettlesAndPricesReportedUsage`, introduced in C-03 commit `7f5e165` (`Fix provider usage and cancellation accounting`); it verifies that provider-reported input usage above the reservation estimate settles and is priced using the provider amount. The recorded class expectation was therefore 12 + 1 = 13 while source has 14.

Proposed separate CI-owner patch (not applied here): add the missing identity to `tools/android-instrumentation-additional-tests.txt`; raise additional count from 2 to 3 and expected campaign/aggregate JUnit total from 105 to 106 in `tools/run_android_instrumentation_campaign.sh`, `tools/verify_android_formal_aggregate.py`, and `.github/workflows/android.yml`; leave the original required set and existing class baseline of 12 intact. This aligns the exact inventory without deleting tests or weakening checks. The CI change remains unapproved and was not committed or pushed.

## Integration notes

The lease duration remains a test-only provisional value, not an approved production parameter. Before production use, the host still needs reviewed extraction orchestration and lifecycle policy; this patch does not enable Memory or ratify ADR-0044. Android Formal remains unverified; the inventory correction requires coordination with the local CI owner.
