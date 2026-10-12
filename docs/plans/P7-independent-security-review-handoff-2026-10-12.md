# P7 Gate C independent security review handoff — 2026-10-12

**Scope:** pre-review synthetic engineering evidence on Draft PR #15. This is the code author's review record, not an independent security review and not production authorization. ADR-0042/0043, frozen contracts, migrations 0001–0008, production schema v8, PRIVATE/SECRET/CREDENTIAL exclusion, disabled production Memory, disabled backup/restore/sync, and unauthorized P8 remain in force.

## Ref and CI baseline

Pre-change live refs were verified on 2026-10-12: `main=f70f34c1f683df1b175b612ba100ce6bb6979811`; PR #12 `865dcadad1750f6ba8f4928eb44a29cfa97f2f4c`; #13 `56c008e7a0c582c1b099ef9a809834884a1b245d`; #14 `cb4df7178a4dc74b124006c4bf2c020cd626aa5e`; #15 `49ffe5653c0b54dc2cc723b78110bad5d9288464`, based on `p7/implementation-synthetic-prototype`. All four PRs were OPEN/DRAFT. PR #14 remains the independent canonical P5 fix.

The campaign implementation was validated at PR #15 HEAD `82bbcc8d8ee2d6a3f6a3cbcf0b2b4e6eb2975822` (OPEN/DRAFT, base `p7/implementation-synthetic-prototype`). At that exact SHA, Fast CI run `38262743495`, Full CI run `38262743484`, and Cross-architecture SQLite portability run `38262743463` completed successfully. Full CI jobs: Linux stable `114857699289`, macOS Intel `114857699582`, macOS arm64 `114857699626`, Linux Rust 1.85 `114857699688`, Windows Gate C `114857699849`. SQLite portability jobs `114857149985`, `114857150496`, `114858134901`, and `114859034230` all passed, including both cross-architecture reopen directions. Fast CI covered workspace smoke, identity, docs, formatting, metadata, and workspace check.

The first Windows run on `5bcbb7046555c2633e15e02cddc208b54a08b445` passed Gate C/process tests and correctly rejected the optimized feature, but its expected nonzero compiler status leaked through the PowerShell action and marked the job failed. The workflow now explicitly exits successfully after verifying that rejection. The corrected Windows job passed at `82bbcc8…`. This was a CI harness defect; it did not change product code. The final evidence-package documentation commit receives fresh CI runs, which are reported with the final PR #15 HEAD.

## Threat model and host boundary

The fixture models accidental corruption, stale concurrent work, process termination, malformed or oversized local evidence, and cooperating independent processes using SQLite and OS file locks. It assumes the tested process, SQLite library, OS, and parent directory are trusted. The sidecar lock is cooperative. A hostile same-user process can bypass it or modify/replace files; direct SQLite tampering, compromised OS, controller-cache loss, whole-host rollback, and physical power loss are outside proven protection.

The test checkpoint is a local synthetic authority with deterministic generated keys. Restoring the checkpoint directory, ledger, database, and key together to one valid older state cannot be detected. No platform authentication adapter, production key custody, external rollback-surviving authority, physical deletion reserve, backup reconciliation, or secure erase is claimed.

## Security invariant evidence matrix

`PASS` means the named synthetic test passed on the listed environment. It does not qualify production behavior. `NOT TESTED` identifies evidence absent from this campaign. Production blockers apply to every row unless a later independently reviewed authority contract changes them.

| Invariant | Authority/design source | Implementation | Test and environment | Threat / observed result | Residual risk and production blocker |
|---|---|---|---|---|---|
| Owner isolation | ADR-0042 P7-D01/D03; ADR-0043 | `experimental_memory.rs`, owner-scoped SQL predicates and composite constraints | `fts_japanese_isolation_class_expiry_and_literal_bounds`; Linux x86_64, Rust 1.85, bundled SQLite | Cross-owner lookup/search isolation PASS | Synthetic Owner constructor only; host authentication and production identity binding are NOT IMPLEMENTED |
| Immutable Task binding | ADR-0042 P7-D01; ADR-0043 | `p7_integrated_recovery_tests.rs` synthetic admission fixture | `synthetic_task_owner_admission_binds_in_one_transaction_and_quarantines_legacy`; Linux | immutable binding and legacy quarantine PASS | No production TaskEngine admission contract; actual integration NOT IMPLEMENTED |
| Memory classification | ADR-0042 P7-D04; ADR-0043 | `experimental_memory.rs`, experimental v9 fixture | `provenance_class_inheritance_rejects_downgrade_and_accepts_safe_raise`; Linux | PUBLIC/PERSONAL boundary and class inheritance PASS | Real classification authority and model context are NOT IMPLEMENTED; PRIVATE/SECRET/CREDENTIAL prohibited |
| Derived Forget closure | ADR-0042 P7-D02/D07; ADR-0043 | `p7_integrated_recovery_tests.rs`, lineage and tombstone transaction | `file_backed_memory_forget_reconciles_full_lineage_and_blocks_stale_ledger`; Linux | derived closure, unrelated-row preservation, FTS removal PASS | Fixture only; physical secure erase NOT TESTED and not claimed |
| Authenticated ledger | ADR-0043 HMAC-SHA-256/key-version decision | `p7_gate_c.rs::{encode,decode,verify_stream,LedgerFileGuard}` | `ledger_detects_truncation_tampering_order_and_unknown_key`, `ledger_rotation_requires_old_and_new_verification_keys`; Linux | tamper/order/truncation/unknown-key denials PASS | Synthetic keyring only; production key lifecycle/custody NOT IMPLEMENTED |
| Ledger thread concurrency | Gate C cooperative process lock plus local operation serialization | `LedgerFileGuard::{append_sync_inner,read_bytes}` | `concurrent_append_calls_on_one_guard_never_duplicate_a_sequence`; Linux x86_64, Rust 1.85 | RED reproduced two concurrent appends both returning success for one sequence; mutex repair changed regression to PASS | Does not protect hostile same-user lock bypass or direct inode writes |
| Checkpoint freshness | ADR-0042 P7-D07; ADR-0043 | `PersistentCheckpointStore`, `verify_stream_at_checkpoint`, `verify_stream_recovery_extension` | `independently_checkpointed_stream_rejects_valid_prefixes_and_generation_mismatch`; Linux | missing/stale checkpoint, valid old prefix, generation/key mismatch denial PASS | Local all-artifact rollback protection NOT IMPLEMENTED; no independent authority survives full rollback |
| Checkpoint bounds and failure | ADR-0042 P7-D05; review resource bound | `PersistentCheckpointStore::{open_existing,refresh,advance}` | `checkpoint_reopen_rejects_oversized_revision_before_allocation`, `checkpoint_revision_count_and_advance_ceiling_fail_closed`; Linux | 256-byte record and 1,024-revision hard ceilings PASS | No pruning/compaction; lifetime capacity remains unapproved beyond test bound |
| Crash recovery | ADR-0043 fail-closed recovery direction | `p7_integrated_recovery_tests.rs` | `child_termination_after_durable_intent_keeps_reopened_memory_denied`, `recovery_itself_is_idempotent_across_process_termination`; Linux child processes | real process exit and restart recovery/denial PASS | No hardware power-loss or directory-cache-loss evidence; Windows/macOS process matrix pending CI |
| Stale extraction fencing | ADR-0042 P7-D01/D05; ADR-0043 quota/generation direction | `p7_extraction_fence_tests.rs` (test-only SQLite fixture) | `extraction_started_before_forget_cannot_reinsert_stale_data`, `extraction_generation_check_is_inside_final_sqlite_commit`; Linux | generation compare and final insert in one `BEGIN IMMEDIATE` transaction; stale insert refused PASS. `process_crash_before_final_commit_leaves_no_extracted_row` exits a real child process after INSERT but before COMMIT and confirms no row after reopen | No TaskEngine/model integration; the interface below requires separate review |
| Quota accounting and leases | ADR-0043: 20 starts/day, one concurrent extraction | `p7_extraction_fence_tests.rs` | `extraction_daily_quota_stops_at_twenty_without_counter_drift`, `independent_processes_admit_one_lease_and_crash_release_is_expiry_based`; Linux | exact 20-start ceiling, independent process admission, expiry after process death PASS | UTC is a persisted nondecreasing test clock; production monotonic/UTC authority and lease duration NOT APPROVED |
| FTS leakage and query bounds | ADR-0042 P7-D06; ADR-0043 | `experimental_memory.rs` FTS query and authoritative-row join | `fts_japanese_isolation_class_expiry_and_literal_bounds`, `fts_content_mismatch_fails_closed_instead_of_returning_partial_search`; Linux | owner/class filters, literal limits, mismatch denial PASS | `unicode61` Japanese quality and production data confidentiality NOT independently assessed |
| Key lifecycle | ADR-0043 explicit versions and fail-closed unknown/missing keys | `SyntheticKeyring`, checkpoint/ledger verification | `ledger_rotation_requires_old_and_new_verification_keys`; Linux | old key required to verify retained history; missing key denied PASS | Real OS secrets/Keychain/Keystore deliberately NOT IMPLEMENTED |
| Filesystem/process behavior | ADR-0042 local filesystem residual-risk boundary | `LedgerFileGuard`, checkpoint file adapter | `ledger_guard_rejects_symlinks_and_keeps_the_opened_inode` (Unix); `ledger_lock_is_exclusive_across_processes_and_released_on_termination` (Linux/macOS/Windows CI) | Unix symlink/path substitution and cooperating process lock tested | Hostile parent-directory replacement out of scope; Windows symlink privilege case NOT TESTED |
| Production feature exclusion | ADR-0043 disabled Gate C authorization | `crates/serea-storage/src/lib.rs` compile guards; feature declarations | optimized default build and optimized `p7-gate-c` rejection checks | ordinary build excludes Gate C; optimized opt-in is intended to fail | Final local/GitHub checks must verify after campaign commit; feature enablement remains prohibited |

## Extraction fixture and production interface proposal

`p7_extraction_fence_tests.rs` is compiled only for tests with both explicit development features. It is not exported as a production API and does not call TaskEngine, a model, or provider. It creates durable owner-scoped admission rows with operation identity, captured generation, UTC-day count, and lease state. `BEGIN IMMEDIATE` serializes admission/Forget/final commit. Final generation comparison, lease check, fixture row insert, and lease completion share one SQLite transaction. Forget increments the owner generation and deletes fixture output atomically. Crashes after durable admission leave the slot occupied until expiry; crashes after fixture insert but before SQLite commit leave no row. Backward wall-clock movement is refused; UTC-day keys use Euclidean division and do not roll back.

Proposed future port, pending TaskEngine/Core and Storage review:

```rust
trait ExtractionAdmission {
    fn admit(&self, owner: OwnerId, task: TaskId, operation: ExtractionOperationId,
             observed_utc: UtcInstant) -> Result<ExtractionLease, Refusal>;
    fn commit<T>(&self, lease: ExtractionLease,
                 write: impl FnOnce(&mut StorageTransaction) -> Result<T, StorageError>)
                 -> Result<T, Refusal>;
}
```

The opaque lease must bind Owner, immutable Task Owner, durable operation ID, generation, UTC quota day, and expiry. Storage must validate the lease generation in the *same transaction* that inserts all derived Memory rows and FTS/accounting effects. Forget must increment generation and remove its approved derived closure in that same serialization domain. No external model call is made while holding a SQLite write transaction. The production design must separately decide idempotent operation replay, clock source/rollback handling, lease expiry semantics across reboot, cleanup authority, and how concurrent limit releases interact with the daily-start quota.

## Resource measurement and bounds

Linux x86_64, Rust 1.85, debug test binary, bundled SQLite 3.53.2; temporary synthetic files, single run, wall-clock `Instant`, no peak RSS instrumentation. Commands:

```sh
cargo +1.85.0 test -p serea-storage --features p7-experimental,p7-gate-c --test p7_resource_bench measure_ledger_append_replay_and_history_growth -- --ignored --nocapture
cargo +1.85.0 test -p serea-storage --features p7-experimental,p7-gate-c --test p7_resource_bench measure_checkpoint_advance_and_reopen_history_growth -- --ignored --nocapture
```

Observed on the campaign run: ledger append through 64/256/512 frames took 75/1,391/6,570 ms total and produced 10,332/41,436/83,024 bytes; replay took 2/15/16 ms. Each append revalidates the prefix plus its new frame, so cumulative append validation processed exactly 2,080/32,896/131,328 frames (N×(N+1)/2); replay processed another 64/256/512 frames. Checkpoint histories of 16/64/128 revisions used 2,256/9,024/18,048 file bytes; advancing each history took 5/190/157 ms total, and reopening at those sizes took under 1/1/2 ms. The per-advance reopen authenticated a cumulative 135/2,079/8,255 checkpoint revisions (N×(N+1)/2−1); bootstrap's initial reopen authenticated one more, and the separately timed reopen authenticated another 16/64/128. Timings are noisy, machine-specific observations, not service latency promises. Peak memory was NOT MEASURED. Append and checkpoint advancement still authenticate accumulated history repeatedly; cumulative work is quadratic. The hard fixture ceilings are 1,024 ledger frames/1 MiB and 1,024 checkpoint revisions with 256-byte checkpoint records. At the ceiling, verification allocation/work is finite; history is refused rather than pruned or compacted. These are defensive fixture bounds, not ADR-0043 product quotas or a production deletion reserve.

## Platform matrix and unsupported guarantees

| Platform | Evidence expected on final campaign HEAD | Status |
|---|---|---|
| Linux x86_64 | Rust 1.85 focused/all workspace tests, child process termination, Unix symlink/path tests, CI Fast/Full | PASS locally and in Full CI `38262743484`; stable Clippy `-D warnings` PASS |
| macOS arm64 | Full CI all-feature and P2H crash/fault suites; SQLite cross-architecture reopen | PASS in Full CI job `114857699626`; portability run `38262743463` |
| macOS Intel x86_64 | Full CI all-feature and P2H crash/fault suites; SQLite cross-architecture reopen | PASS in Full CI job `114857699582`; portability run `38262743463` |
| Windows | Rust 1.85 Gate C unit suite, SQLite WAL/reopen, process lock/release after termination, optimized feature isolation and exclusion | PASS in Full CI job `114857699849` |

Unix parent-directory sync is not asserted on Windows. Windows checkpoint create semantics use `create_new` immutable revisions; directory-entry durability is unsupported. Symlink tests that require Unix APIs are cfg-gated; CI privilege denial is NOT classified as a code failure. No platform proves physical power-loss durability.

## Code-author review, independent questions, and reproduction

This package records an adversarial review by the implementation author and automated tests. It is not ADR-0043's required independent security review. An independent reviewer should receive the exact final commit SHA, CI job IDs, this document, ADR-0042/0043, the fixture/schema, and repository source; they should work from a clean checkout and report findings without relying on this author's conclusions.

Questions for the independent reviewer:

1. Can any public/default/optimized feature path construct or use a Memory-capable handle, admit PRIVATE/SECRET/CREDENTIAL data, or bypass the authenticated Owner boundary?
2. Does the Forget closure include every approved derived row, lineage edge, Blob reference, FTS entry, quota counter, and restore/reconciliation barrier under partial writes and retries?
3. Can a valid stale ledger/checkpoint prefix or coordinated rollback of all local artifacts restore Memory access? Which separate authority must survive to prevent it?
4. Are HMAC framing, key-version rotation, canonical parsing, duplicate IDs, sequence/generation transitions, checkpoint count/size limits, and append cumulative work sound under hostile inputs?
5. Does extraction admission/final commit serialize correctly with Forget across independent processes, owner changes, duplicate operations, UTC boundaries, wall-clock rollback, crash, and SQLite busy/rollback?
6. Which Windows/macOS filesystem guarantees are actually supported, especially lock release, file create/rename, SQLite WAL reopen, and directory-entry durability?
7. Which production controls are missing before any real Owner, personal data, model context, backup, synchronization, or external effect can be enabled?

Reproduction on a Rust 1.85 host:

```sh
cargo +1.85.0 test -p serea-storage --features p7-experimental,p7-gate-c --lib extraction_ --locked
cargo +1.85.0 test -p serea-storage --features p7-experimental,p7-gate-c --lib p7_integrated_recovery_tests --locked
cargo +1.85.0 test -p serea-storage --features p7-experimental,p7-gate-c --test p7_gate_c_lock --locked
cargo +1.85.0 test --workspace --all-features --locked
cargo +1.85.0 test --workspace --locked
cargo +1.85.0 test --workspace --all-features --locked
cargo +stable clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo +stable fmt --all -- --check
python3 tools/validate_docs.py docs
python3 tests/workspace_smoke.py
python3 -m unittest discover -s tests -p workspace_smoke_tests.py
python3 tools/check_commit_identity.py
git diff --check
sha256sum crates/serea-storage/migrations/000{1,2,3,4,5,6,7,8}_*.sql
```

Expected raw migration SHA-256 values at the campaign base:

| Migration | SHA-256 |
|---|---|
| `0001_initial.sql` | `d9068dccbc26ececb71be79c475080633166ba0163c62b2d98b9733512baefea` |
| `0002_event_scheduler.sql` | `4924e69150bbff9c39e2e6b7e2bdd61045202e504900fe0f510d513fbf815e67` |
| `0003_model_accounting.sql` | `530a6d6cb5ec9c757311d48e10a62ef456d9d01c09512f321cffe42fe3307f80` |
| `0004_capability_registry.sql` | `06b22fa682564290b71a26825a777f7a295220bd02f4c6c614d234b73001685f` |
| `0005_policy_approval.sql` | `cf6686807e3ceb10bd21c6653fbdd61c487e6b8938a2d78104d160920831e71a` |
| `0006_policy_authority_sealing.sql` | `0af16a30cd2b29f046c14d7079a356dc25ce4352216edd9ad6821b168e6eb6ca` |
| `0007_policy_revision_sealing.sql` | `48f2f1805d59f4eec215963ac5109007294a0dfe015b9a4b4e7e69c14f9197eb` |
| `0008_approval_lifecycle_routing.sql` | `6e1520bf22ced832ed480a1cf26476e056611f22b1f517f2324b32a8d4fbf925` |

The migration hash output must match these values; no migration 0001–0008 edits are permitted. At implementation ref `82bbcc8d8ee2d6a3f6a3cbcf0b2b4e6eb2975822`, Fast CI, Full CI, and SQLite portability are PASS. The evidence-package documentation commit is validated again on its own final PR #15 ref; see the final campaign report for those exact run IDs and outcomes.

Campaign exit state: **P7_GATE_C_PRE_INDEPENDENT_SECURITY_REVIEW_READY** once the documentation-only evidence commit's Fast CI, Full CI, and SQLite portability are all PASS. This state means the synthetic pre-independent-review evidence package is ready for an external reviewer. It is not an independent review, production authorization, merge authorization, or feature enablement.

## Independent findings remediation — 2026-10-12

This section supersedes earlier PR #15 ref/CI summaries above. The original findings were reproduced against implementation source at pre-fix PR #15 HEAD `aef575282b36687222d87c9f28e0256eb4e7c3a0`, with no production data or keys. RED was executed with Rust 1.85.0 and the bundled SQLite fixture. The P7 findings remain **pending independent reviewer verification**; the implementation author does not close them on the reviewer's behalf.

| Finding | Initial condition and executable RED | Root cause and correction | Regression and crash/concurrency evidence | Residual risk / re-review status |
|---|---|---|---|---|
| P7-IR-01 HIGH | At `aef5752…`, `p7_memory_items` allowed a tombstoned `(owner_id,memory_id)` to be inserted again. `p7_ir01_forgotten_memory_id_cannot_be_reinserted` first failed at `unwrap_err`: API returned `Ok(())` for the same ID after Forget. | `experimental_memory.rs` now checks the tombstone in the same immediate SQLite transaction as insertion. The v9 fixture's `p7_memory_admission` trigger independently rejects direct SQL inserts against the tombstone. Tombstones remain immutable. `p7_memory_items` and its FTS join use Owner-scoped identity `(owner_id,memory_id)`; equal IDs are valid for distinct Owners and isolated in search. | `p7_ir01_forgotten_memory_id_cannot_be_reinserted` PASS: API and direct SQL reject reuse; repeated completed operation is idempotent; a new operation cannot create intent for the retired ID; resource counters and FTS remain consistent; crash/reopen still rejects reuse. `memory_identity_is_owner_scoped_and_fts_joins_owner_identity` PASS. Existing independent-connection admission test covers same-owner duplicate racing. | Linux synthetic SQLite/OS process evidence only. Owner authentication and malicious same-user database edits are not covered. Independent reviewer: PENDING. |
| P7-IR-02 HIGH | At `aef5752…`, the coordinator wrote `read_state='QUARANTINED'` and pending operation before appending intent. Rust 1.85 RED at the first capacity refusal showed `before=(READY,None,408)` and `after=(QUARANTINED,Some("capacity-op-204"),409)`; the error was `LedgerError::TooLarge`. Reproduction: `p7_ir02_capacity_refusal_does_not_quarantine_before_intent` on a temporary worktree at the pre-fix SHA. | `LedgerFileGuard::ensure_append_capacity` authenticates current bytes under the namespace lock and sums actual `encode()` output lengths and frame count with checked arithmetic. The coordinator prebuilds Intent, all chunks, PrimaryApplied, Finalized, Reconciled, checks the next authenticated checkpoint revision and exact encoded checkpoint record, then starts the transition. It persists pending/quarantine only after complete Intent+chunks. Pre-intent crash leaves READY; post-complete-intent crash replays from authenticated authority. No history is removed. | `p7_ir02_capacity_refusal_does_not_quarantine_before_intent` PASS at ledger exhaustion; `p7_ir02_checkpoint_exhaustion_refuses_before_quarantine_or_intent` PASS at revision 1,024; `crash_before_intent_leaves_ready_generation_and_memory_unchanged` PASS; complete-intent, primary-commit and checkpoint crash recovery tests PASS; repeated recovery and concurrent independent Forget tests PASS. Existing frame/byte/checkpoint hard-bound and simulated short/write-zero/no-space tests remain in `p7_gate_c` unit tests. | Finite append-only evidence cannot support unlimited Forget. No compaction/retirement mechanism is authorized or implemented. See [finite-ledger rollover proposal](P7-finite-ledger-rollover-proposal-2026-10-12.md). Production uninterrupted Forget remains unresolved. Independent reviewer: PENDING. |
| P7-IR-03 MEDIUM | At `aef5752…`, Forget removed `p7_memory_blob_refs` but did not delete an exclusive orphan Blob. Rust 1.85 RED `p7_ir03_forget_collects_an_exclusive_memory_blob` observed `left: 1, right: 0` for the post-Forget Blob row count. | Forget now captures and bounds exact `(digest,data_class_rank)` candidates before writing intent, then in the same SQLite transaction deletes the Memory refs/rows and calls the shared candidate sweeper. The sweep checks every existing P5/P6 relation and conditionally checks P7 only when `p7_memory_blob_refs` exists in `sqlite_schema`; ordinary v8 does not query a missing table. A still-referenced Blob survives. | PASS: `p7_ir03_forget_collects_an_exclusive_memory_blob`, `p7_ir03_shared_memory_blob_survives_until_last_reference_is_forgotten`, `p7_ir03_task_reference_protects_blob_until_task_reference_is_removed`, `p7_ir03_schedule_and_occurrence_references_protect_blob`, `p7_ir03_model_call_response_reference_protects_blob`, `p7_ir03_gc_candidate_bound_refuses_before_forget_intent`, `p7_ir03_same_digest_different_class_ranks_are_gc_candidates_separately`, `p7_ir03_crash_during_memory_blob_gc_replays_cleanup_after_reopen`, and `model_call_tests::task_blob_sweep_is_v8_safe_and_preserves_a_p7_memory_reference`. Shared Memory, Task, Schedule, occurrence, and model-call refs survive until last reference removal; over-bound candidates refuse before intent; v8 table absence and P7 table presence are both exercised. | SQLite transaction gives all-or-nothing logical reference removal and orphan collection in this fixture. It does not establish physical secure erase. P7 is still test-only. Independent reviewer: PENDING. |

### Exact focused commands

Run from a clean checkout of the final PR #15 code using Rust 1.85.0 (and stable for Clippy/format checks):

```sh
cargo +1.85.0 test -p serea-storage --features p7-experimental,p7-gate-c --lib p7_ir01_forgotten_memory_id_cannot_be_reinserted
cargo +1.85.0 test -p serea-storage --features p7-experimental,p7-gate-c --lib p7_ir02_capacity_refusal_does_not_quarantine_before_intent
cargo +1.85.0 test -p serea-storage --features p7-experimental,p7-gate-c --lib p7_ir02_checkpoint_exhaustion_refuses_before_quarantine_or_intent
cargo +1.85.0 test -p serea-storage --features p7-experimental,p7-gate-c --lib crash_before_intent_leaves_ready_generation_and_memory_unchanged
cargo +1.85.0 test -p serea-storage --features p7-experimental,p7-gate-c --lib p7_ir03_forget_collects_an_exclusive_memory_blob
cargo +1.85.0 test -p serea-storage --features p7-experimental,p7-gate-c --lib p7_ir03_shared_memory_blob_survives_until_last_reference_is_forgotten
cargo +1.85.0 test -p serea-storage --features p7-experimental,p7-gate-c --lib p7_ir03_task_reference_protects_blob_until_task_reference_is_removed
cargo +1.85.0 test -p serea-storage --features p7-experimental,p7-gate-c --lib p7_ir03_schedule_and_occurrence_references_protect_blob
cargo +1.85.0 test -p serea-storage --features p7-experimental,p7-gate-c --lib p7_ir03_model_call_response_reference_protects_blob
cargo +1.85.0 test -p serea-storage --features p7-experimental,p7-gate-c --lib p7_ir03_gc_candidate_bound_refuses_before_forget_intent
cargo +1.85.0 test -p serea-storage --features p7-experimental,p7-gate-c --lib p7_ir03_same_digest_different_class_ranks_are_gc_candidates_separately
cargo +1.85.0 test -p serea-storage --features p7-experimental,p7-gate-c --lib p7_ir03_crash_during_memory_blob_gc_replays_cleanup_after_reopen
cargo +1.85.0 test -p serea-storage --features p7-experimental,p7-gate-c --lib ir04_
cargo +1.85.0 test -p serea-storage --all-features --lib model_call_tests::task_blob_sweep_is_v8_safe_and_preserves_a_p7_memory_reference
cargo +1.85.0 test -p serea-storage --features p7-experimental,p7-gate-c --lib experimental_memory::p7_integrated_recovery_tests
cargo +1.85.0 test -p serea-storage --all-features --locked
cargo +1.85.0 test --workspace --locked
cargo +stable clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo +stable fmt --all -- --check
```

Also run `cargo +1.85.0 test -p serea-storage --features p7-experimental --locked` for the P7 FTS/migration suite, `cargo +1.85.0 test -p serea-storage --locked` for ordinary v8 Storage, the repository CI workflows for Linux/macOS/Windows and SQLite portability, `python3 -m unittest discover -s tests -p workspace_smoke_tests.py`, `python3 tools/validate_docs.py docs`, `python3 tools/check_commit_identity.py`, `git diff --check`, and the migration hash table above.

### Scope and non-claims

ADR-0042/0043 and migrations 0001–0008 remain unchanged. Default production schema remains v8; v9 is an explicitly selected synthetic fixture. Gate C remains disabled in ordinary operation and optimized activation remains rejected. No Owner authentication adapter, production key custody, independent rollback authority, production backup/restore/sync, real personal data, or secure erase is implemented. No P8 or provider connection is authorized. The finite ledger/checkpoint capacity is a production design blocker, not a resolved implementation detail.

Implementation commit `50806bb746d25f893bb9392c27feefbc9bfc1c13` is included in PR code head `70be4516b669862423d7e6d813811af0379167b3`. Rust 1.85 workspace default and all-feature tests/doctests PASS, stable all-target/all-feature Clippy with `-D warnings` PASS, Rustfmt PASS, release default check PASS, optimized `p7-gate-c` rejection PASS, docs validator PASS (133 Markdown files), Python workspace smoke PASS (76 tests), commit identity guard PASS, and migration 0001–0008 SHA-256 unchanged. On the exact code head `70be4516b669862423d7e6d813811af0379167b3`, GitHub Fast CI `38273915849`, Full CI `38273915855`, and Cross-architecture SQLite portability `38273915854` all concluded success. Full CI jobs passed on Linux stable, Linux Rust 1.85, macOS Intel x86_64, macOS arm64, and Windows Gate C synthetic validation. SQLite portability passed both cross-architecture directions. The code and handoff commits use author and committer `Codex <noreply@github.com>`.

Engineering status: **P7_GATE_C_SECURITY_REMEDIATION_READY_FOR_INDEPENDENT_RECHECK**. This means the implementation and evidence package are ready for external recheck; it is not an independent security approval and does not authorize PR merge or production enablement. Findings remain PENDING the independent reviewer. PR #15 remains OPEN/DRAFT. The PR has no reviewer request assigned; route this handoff and the Rust 1.85 reproduction commands to an independent Rust-enabled reviewer. The actual external re-review request remains unsent because no GitHub reviewer handle was supplied.

## Explicit production blockers

- Independent external security review has not happened; automated/internal review does not satisfy ADR-0043.
- Production Owner authentication, immutable Task admission, and reviewed production Storage/TaskEngine extraction contract are NOT IMPLEMENTED.
- Production key custody, rotation/recovery authority, and independently surviving rollback authority are NOT IMPLEMENTED.
- A fully local rollback remains undetectable; backup/restore/sync are disabled.
- macOS/Windows production adapter semantics, whole-host rollback protection, and physical durability are NOT TESTED. Synthetic Windows Gate C CI behavior is PASS; Unix parent-directory sync is unsupported on Windows.
- Real personal data, model context, provider/account, OS secrets, physical deletion reserve, and physical secure erase are outside this campaign and unauthorized.
- P8 is unauthorized. PR #15 must remain Draft and unmerged; no production feature activation is authorized by this review exit.

## P7-IR-04 crash-recovery remediation — 2026-10-12

| Finding | Exact RED reproduction and old state | Correction and GREEN evidence | Limits / disposition |
|---|---|---|---|
| P7-IR-04 HIGH | Added `after-last-chunk-before-pending` to the real child-process crash seam. The child exited with code 73 after the final authenticated chunk was synced. The disposable ledger held exactly two authenticated records (Intent and complete Chunk); the separate trusted checkpoint remained sequence 0 / generation 0; SQLite remained `(generation=0, READY, pending=NULL, pending_generation=NULL)`. Calling the original `recover_startup` returned exact `Err(Stale)` because no SQLite pending marker matched. The RED assertion requiring recovery failed against the original implementation with that error. | Under the existing cross-process ledger lock, startup refreshes and verifies the persistent checkpoint prefix and authenticated extension. It accepts exactly one complete Intent+ordered-Chunks extension, exact Owner/key version/generation/operation/root-digest shape, bounded canonical unique decoded target IDs, matching closure digest, a READY primary at the trusted generation, all target rows still present, no target tombstone and no prior completed operation. A conditional SQLite update atomically adopts the operation as QUARANTINED with the reconciled pending generation; then the existing idempotent replay validates/applies closure, reconciles ledger records, persists checkpoint, checks FTS/accounting, and releases READY. Terminal ledger records must be a strict prefix in protocol order. Single-chunk and 60-target multi-chunk recovery pass. A second child-process crash inside the pending SQLite transaction recovers from rollback; a crash after pending commit resumes the existing pending path. The 607-test serea-storage all-feature suite passes, including IR-01/02/03, Gate C, checkpoint/ledger, P5/P6 and doctests. | The test-only MAC authenticates synthetic coordinator output; it is not Owner authentication. The synthetic model treats a valid Intent from its test coordinator as already admitted under its test adapter assumptions. Production recovery must independently establish admission and authorization. No reviewer disposition is made here; IR-04 remains PENDING independent recheck. |

The RED command was `cargo +1.85.0 test --locked -p serea-storage --features 'p7-experimental p7-gate-c' ir04_red_last_chunk_before_pending_is_durable_but_startup_refuses -- --nocapture` before the corrective transition. It failed with `original exact error: Err(Stale)`. The corrected focused commands are `cargo +1.85.0 test --locked -p serea-storage --features 'p7-experimental p7-gate-c' ir04_ -- --nocapture` and `cargo +1.85.0 test --locked -p serea-storage --features 'p7-experimental p7-gate-c' child_termination_after_durable_intent_keeps_reopened_memory_denied -- --nocapture`.

At the time of this addendum, full workspace validation and post-push CI are still pending; consult the final campaign entry below before treating this package as ready. Migrations 0001–0008 and ADR-0042/0043 remain unchanged. The finite-ledger rollover proposal remains unapproved. PR #15 remains OPEN/DRAFT and unmerged; no independent finding is closed by this author.
