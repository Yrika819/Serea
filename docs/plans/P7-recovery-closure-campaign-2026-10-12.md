# P7 Gate C recovery closure campaign — 2026-10-12

**Branch:** `p7/gate-c-disabled-runtime` (Draft PR #15, stacked on PR #13)
**Scope:** development/test-only synthetic implementation. No personal data, actual host identity, credential store, production key, backup, provider, model Memory context, or external effect was used.
**Result:** coordinator-only synthetic Memory access and restart-safe recovery of complete authenticated Forget intents are implemented and tested on Linux. Production Gate C remains disabled and is not review-ready.

## Capability boundary

- `P7ExperimentalMemory` is no longer re-exported from `serea-storage`. Its module is private, and a `compile_fail` doctest asserts that external code cannot import the raw Memory handle.
- `p7_integrated_recovery_tests.rs` is nested under the private experimental module. `VerifiedMemory` has private fields, no `Clone`, owns the cross-process `LedgerFileGuard`, binds Owner and checkpoint generation, and is consumed once. Every operation rechecks SQLite READY plus exact generation. It cannot outlive the coordinator borrow or survive process restart.
- Coordinator reads/writes mint the permit only after reopening the ledger inode under the OS sidecar lock, authenticating the exact stream against the latest persistent checkpoint, and checking primary Owner/generation. Missing ledger/checkpoint, stale stream or generation fails closed. The SQLite READY bit alone grants no capability.
- The crate-private test fixture can directly reach its private Store for setup and negative tests. This does not defend against arbitrary SQLite file edits or malicious code in the trusted process. Extraction final-commit APIs and a production TaskEngine/Core integration are not present.

## Persistent synthetic checkpoint

`PersistentCheckpointStore` in `crates/serea-storage/src/p7_gate_c.rs` stores immutable `checkpoint-%020d.bin` revisions. The bounded canonical record binds format version, revision, OwnerId, identity epoch, HMAC key version, final sequence, reconciled generation, stream digest, and HMAC-SHA-256. Reopen validates every revision, requires contiguous revision numbers and retained old keys, authenticates each record in constant time, and refuses a missing, malformed, revoked-key, or corrupt highest revision. It never falls back to an older checkpoint when the latest record is corrupt.

Bootstrap is explicit and only accepts generation-zero/sequence-zero state in an absent or empty authority directory. Ordinary reopen does not create a missing directory or genesis checkpoint. Advance uses `create_new` on the next immutable revision, writes and `sync_all`s the file, syncs the parent directory on Unix, reopens/authenticates, then updates in-memory state. A crash leaving a partial highest revision makes the authority unavailable and Memory remains denied. Windows directory-entry durability is not asserted by this fixture. Process termination tests are not physical power-loss tests.

The checkpoint fixture is separate from the primary DB and ledger paths but remains on the same host and uses a deterministic synthetic key. Restoring the entire local checkpoint directory, ledger, primary database and key to one prior state can evade freshness detection. This is not a production rollback-surviving trust anchor or production key custody.

## Recovery protocol

Normal Memory access requires an exact checkpoint match and all operations terminally reconciled. Startup recovery uses a separate path:

1. Acquire the stable ledger sidecar lock and open the ledger file once; reads/writes use that file handle to avoid path re-open between verification and mutation.
2. Authenticate the ledger and prove that the checkpoint sequence identifies the exact byte prefix by whole-prefix digest, Owner, epoch, key version, sequence and generation. Every operation in the anchored prefix must be reconciled.
3. Require a contiguous HMAC-authenticated extension containing one pending operation whose owner/epoch, intent generation, requested-root digest, complete ordered closure chunks, count and closure digest validate. Torn/reordered/partial chunks, unknown keys, mismatched pending operation, or an unanchored prefix are denied.
4. Inspect SQLite generation and every closure ID. Either all rows remain and no tombstones exist at the checkpoint generation, or all rows are absent with matching content-free tombstones at the intent generation. Mixed/conflicting states are denied.
5. Apply the complete closure in one SQLite transaction when absent: tombstones, lineage removal, Blob-reference removal, Memory row deletion (FTS triggers), quota triggers, and quarantined generation update. Replay accepts an already-completed matching transaction only after checking no closure rows, Blob refs, or lineage edges remain and owner counters are recomputed consistently.
6. Append missing `PRIMARY_APPLIED`, `FINALIZED`, and `RECONCILED` records idempotently; set primary generation to the reconciled generation while still quarantined; check FTS and resource accounting; persist and reopen the next checkpoint revision; verify exact checkpoint/ledger/primary convergence; only then clear pending operation fields and set READY.

The sequence is `READY → QUARANTINED + pending operation → INTENT/CHUNKS → primary transaction → PRIMARY_APPLIED → FINALIZED → RECONCILED → primary generation → persistent checkpoint → READY`. A failed transition remains quarantined. A pending operation status is observable as `PENDING`; first successful Forget returns `COMPLETED`; an exact duplicate returns `ALREADY_COMPLETED`; reusing the operation ID with a different target digest is rejected. No completion response precedes checkpoint and primary convergence.

## RED-first defect found

`concurrent_independent_owner_forgets_serialize_checkpoint_advancement` initially failed: each coordinator loaded the persistent checkpoint at startup, and the second serialized writer kept using that stale in-memory snapshot after acquiring the file lock. `PersistentCheckpointStore::refresh` was added and is now called under the ledger lock before every normal verification, status query and recovery. The regression now proves two separate coordinators can Forget separate rows and advance through generations 2 and 4 without a false stale denial.

The previous direct raw-handle bypass was also removed: the Gate C feature no longer re-exports `P7ExperimentalMemory`, the integration tests route reads/writes through single-use `VerifiedMemory`, and the compile-fail doctest verifies the external API surface.

## Test evidence

| Test | Evidence |
|---|---|
| `persistent_checkpoint_bootstrap_reopen_tamper_and_missing_authority` | Actual temporary checkpoint directory; explicit bootstrap, revision advance/reopen, MAC corruption denial, missing authority denial, and demonstrates local authority rollback limitation. |
| `ledger_guard_rejects_symlinks_and_keeps_the_opened_inode` | Linux temporary files; rejects ledger and lock symlinks, keeps the opened ledger inode if its pathname is replaced. |
| `simulated_short_write_write_zero_and_enospc_never_complete_a_frame` | Injected short write, WriteZero, and ENOSPC behavior. ENOSPC is simulated, not physical. |
| `capability_is_single_use_generation_bound_and_lost_on_restart` | Permit reuse and generation change are rejected; new permit also fails after primary/checkpoint mismatch. |
| `missing_checkpoint_or_ledger_does_not_bootstrap_ready_primary` | Existing READY primary does not cause missing checkpoint or ledger recreation. |
| `child_termination_after_durable_intent_keeps_reopened_memory_denied` | Real child-process termination and actual temporary SQLite/ledger/checkpoint files across before intent, partial intent, complete chunks, transaction in-flight/committed, ledger transitions, checkpoint creation/write/sync, and before READY. Recoverable cases converge; incomplete/corrupt authority cases fail closed. |
| `recovery_itself_is_idempotent_across_process_termination` | First child dies after chunks; second child dies during replay after primary commit; a third process completes reconciliation exactly once. |
| `concurrent_independent_owner_forgets_serialize_checkpoint_advancement` | Two independent Store/coordinator instances contend in threads over the OS sidecar lock, delete separate Owner rows, and converge at generation 4. |
| `file_backed_memory_forget_reconciles_full_lineage_and_blocks_stale_ledger` | Actual temporary DB and ledger; transitive closure, unrelated rows, supersession distinction, FTS, idempotency, wrong-target operation reuse, and stale ledger denial. |
| `incomplete_authenticated_intent_is_not_replayed_or_reopened` | Valid HMAC intent with missing closure chunk remains pending and cannot reopen. |

Rust 1.85 focused Gate C tests passed (11 ledger/checkpoint tests and 10 integrated tests). The complete all-feature workspace test/doctest run passed, including 574 storage unit tests, the integrated child-process scenarios and existing P5/P6 regressions. The ordinary workspace test/doctest run also passed. Stable all-target/all-feature Clippy with `-D warnings` and Rustfmt passed. Clippy printed one pre-existing vendored `serde_json` deprecation warning; no workspace lint failed. Default optimized workspace check passed; optimized `p7-gate-c` was rejected by its intended compile-time guard. Bundled SQLite is 3.53.2. The docs validator scanned 131 Markdown files, Python workspace smoke tests passed (76), commit identity guard passed, `git diff --check` passed, and raw SHA-256 comparisons against starting HEAD confirmed migrations 0001–0008 are byte-identical.

GitHub initially found `simulated_short_write_write_zero_and_enospc_never_complete_a_frame` returning success after a partial injected write. The injector was corrected to persist only the partial bytes and return `LedgerError::Io`; the focused regression and all-target storage Clippy then passed. At code head `749785d1691f4953c99af481a2bc70b8584f1ff8`, all applicable GitHub workflows concluded success: Fast CI run `38252381884`, Full CI run `38252381915`, and cross-architecture SQLite portability run `38252381883`. Full CI covered Linux stable, Rust 1.85.0, macOS arm64 and macOS Intel x86_64; SQLite portability passed both architecture directions. No physical power-loss or ENOSPC claim follows from CI.

## Crash matrix evidence boundary

The child-process test terminates actual processes at named program checkpoints and reopens actual files on Linux. It exercises process-crash ordering, SQLite transaction rollback/replay, HMAC frame validation, persistent revision recovery, quarantine and idempotent replay. It does not simulate hardware power loss, actual ENOSPC, directory metadata loss, disk controller caches, macOS/Windows filesystem behavior, hostile same-user lock bypass, production extraction races, or arbitrary system-call failures. Short write/WriteZero/ENOSPC are injected unit-test paths. Incomplete ledger/checkpoint bytes intentionally leave the namespace unavailable; automatic byte repair/truncation is not attempted.

## Remaining blockers

- The checkpoint is local synthetic authority with synthetic deterministic key. Independent rollback authority and production key custody are unapproved.
- macOS and Windows Gate C filesystem/lock adapters are not validated. Windows directory durability is explicitly not claimed.
- The filesystem path and sidecar lock assume a trusted parent directory. Same-user hostile replacement or direct DB modification is outside this fixture's protection.
- Production Owner authentication, actual TaskEngine immutable Owner admission, extraction final commit fencing, production Memory handle construction, external security review and confidentiality verification remain unimplemented.
- Actual ENOSPC, physical power loss, complete local rollback, backup/restore, physical secure erase, deletion reserve and production feature enablement remain unproven or unauthorized.

No production Memory was enabled. No personal data or account was accessed. PR #15 remains Draft and unmerged.
