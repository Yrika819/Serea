# P7 integrated synthetic recovery campaign — 2026-10-12

**Branch:** `p7/gate-c-disabled-runtime` (Draft PR #15, stacked on PR #13)
**Scope:** development/test-only integration using generated identities, keys, SQLite files and ledgers. No real account, personal data, production key, backup, provider or model path is used.
**Status:** integrated synthetic path demonstrated; production review exit criteria are **not** met.

## Repository and accepted contract

Campaign started at PR #15 head `c14d1ad627e5475cfb9bc37be9b1fda4511648a3`. Current owner-approved policy remains exactly ADR-0043: 1,000 active items/Owner, 4,096 UTF-8 bytes/body, 4 MiB active body, 8 MiB logical FTS source, 20 extraction starts/UTC day, one concurrent extraction, and 7/90/365/365 day base retention. Retrieval does not renew expiry; there is no implicit eviction; admission is transactional; renewal needs explicit Owner authentication. The ADR's secondary numeric values remain provisional. Backup, restore and cross-device sync remain disabled/deferred.

## Code and evidence added

- `crates/serea-storage/src/p7_gate_c.rs` now distinguishes frame integrity from history freshness. `verify_stream_at_checkpoint` requires an externally supplied Owner+epoch+key-version+terminal sequence+generation+whole-stream digest checkpoint, a terminal `Reconciled` frame (except the empty generation-zero bootstrap), and exact primary generation agreement. No checkpoint, torn/corrupt bytes, a valid older complete prefix, omitted reconciliation, wrong Owner/epoch, missing key version, or primary generation mismatch denies access. HMAC-valid bytes alone are not treated as latest state.
- `crates/serea-storage/src/p7_integrated_recovery_tests.rs` adds a test-only synthetic coordinator over actual temporary SQLite primary and ledger files. It holds the cooperative sidecar lock across barrier checks and Memory operations, verifies host-synthetic purpose-bound Forget evidence and nonce use, computes explicit Owner-scoped lineage closure, persists complete chunked intent, applies tombstones/lineage removal/Blob-ref removal/row deletion and FTS triggers in one SQLite transaction, writes primary-applied/finalized/reconciled records, compares checkpoint and DB generation, and returns completion only after reconciliation. Duplicate completed operation IDs are idempotent.
- The integration fixture covers Memory insert/get/list/search, a three-row derived lineage closure, unrelated-row preservation, proof that supersession is not derivation, FTS integrity, stale ledger replacement denial, primary generation mismatch, wrong authentication purpose, legacy Task quarantine, Task+Owner binding rollback on a pre-bind failure, binding immutability, nonce replay denial, post-commit reopen and two concurrent admissions of one Task with one winner. Forget operation IDs bind a digest of the canonical requested root set; reuse for another target fails.
- A child process calls `process::exit(73)` after syncing an intent and committing quarantine. Reopen with the prior external checkpoint denies `get` and `search`. Existing `p7_gate_c_lock` tests separately check process-wide sidecar exclusion and OS lock release after termination.
- Experimental SQL moved from the migration directory to `crates/serea-storage/tests/fixtures/p7_experimental_memory_v9.sql` and its catalog name is explicitly `test-fixture-p7-experimental-memory-v9`. Ordinary migration catalog is v8. A separate `p7_memory_lineage` relation records derivation independently of provenance and supersession, prevents cycles, and is deleted only after tombstones exist. Migrations 0001–0008 are unchanged.

## Specific freshness cases

`independently_checkpointed_stream_rejects_valid_prefixes_and_generation_mismatch` verifies middle-frame truncation, valid 1/2/3-frame complete prefixes of a four-frame stream, missing terminal reconciliation, no-checkpoint denial, Owner/epoch mismatch, primary older and newer generation, and key revocation/missing version. It also demonstrates the fundamental limit: if the ledger, key availability, primary DB and trusted checkpoint all return to the same earlier state, local verification cannot identify that complete rollback. The test checkpoint lives in process memory only and is not a production trust anchor.

## Remaining failures and limits

The existing `P7ExperimentalMemory` test surface still checks the SQLite `read_state` row internally. The integrated coordinator checks the independent checkpoint first, but an internal unit-test caller with direct access to the old prototype object can bypass the coordinator while the DB row remains READY. A test demonstrates this isolation gap. That object is compiled only for `cfg(test) + p7-experimental`, is not exported from ordinary builds, and optimized feature builds are refused; nevertheless the synthetic composition is not a hardened production API boundary. The production-facing redesign must make Memory handles constructible only by a verified coordinator capability and perform each read/write under the same cross-process fence.

The synthetic checkpoint is process-memory state. Reopening succeeds only when the test supplies the held checkpoint again; this models an independent authority contract, not durable production custody. If the process dies after intent, the old checkpoint does not authorize automatic checkpoint advancement, so Memory remains denied. Automated idempotent replay from trusted intent, process-independent checkpoint storage, real Owner TaskEngine admission, extraction-final-commit races, complete multi-process Forget contention, symlink/path replacement defense, real ENOSPC, physical power-loss behavior, directory durability, and full crash-window matrix remain incomplete or untested.

No production OS adapter is supported. `LedgerFileGuard` is cooperative and assumes a trusted directory and stable sidecar; a hostile same-user process can bypass the lock or replace paths. HMAC and SQLite transactions do not defend against a compromised host. No physical secure erase, physical deletion reserve, independent rollback domain, confidentiality qualification, backup recovery or production key custody is established.

## Validation status

The following local checks passed on Linux x86_64, Rust 1.85, bundled SQLite 3.53.2, unless indicated otherwise:

- `cargo +1.85.0 test --workspace --all-features --locked --offline` — passed, including the new six-test integration module, all Rust targets and doctests.
- `cargo +1.85.0 test --workspace --locked --offline` — passed with the ordinary feature set.
- Stable `cargo clippy --workspace --all-targets --all-features --locked --offline -- -D warnings` — passed after fixing new test-helper lints. Vendored `serde_json` emits its existing deprecated `usize::max_value` warning; no new warning remains.
- Rust 1.85 default optimized storage build — passed. Optimized `p7-gate-c` build — rejected by its intended compile-time guard.
- Rustfmt, `git diff --check`, docs validator (130 Markdown files), Python workspace smoke (76 tests), commit identity checker, and raw SHA-256 comparison for migrations 0001–0008 — passed.

GitHub Fast CI, Full CI and SQLite portability for the resulting head are pending push/dispatch. macOS/Windows Gate C runtime behavior and physical power-loss tests are **NOT TESTED**. Injected/process faults are not hardware failure tests.

The highest proven state is an integrated **synthetic file-backed** admission/Memory/Forget/checkpoint-denial path. It is not `P7_GATE_C_INTEGRATED_SYNTHETIC_RECOVERY_REVIEW_READY` while the external checkpoint lacks durable independent custody, pending-intent replay is not implemented, and crash/concurrency and bypass coverage is incomplete.
