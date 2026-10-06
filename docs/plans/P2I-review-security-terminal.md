# P2I Terminal Security / Authority Review

- **Review identity:** `p2i-security-terminal-14017e3-20261006`
- **Review mode:** second, separate main-agent review pass; fresh security checklist and direct source/diff inspection. No subagent or delegated reviewer was used.
- **Scope:** P2A `10622803fc52e7ed8e91de5ce41f472b09793766` through P2H `14017e3981cf23ac2b4a076aad435f16eb59b535`, plus the current dirty P2I delta.
- **Prior lineage:** `P2I-review-security.md` remains unchanged as generation-0 / incomplete evidence. This terminal report does not upgrade or overwrite it.
- **Supporting bounded evidence:** `P2I-review-coverage.md`, `P2I-recovery-coverage.md`, and `P2I-vendor-provenance.md`; current Git diff and normal-edge Cargo dependency tree were inspected directly.
- **Excluded:** P3 implementation, exhaustive third-party vulnerability certification, arbitrary feature combinations, and guarantees outside the frozen P2 contract.

## Fresh security checklist and direct inspection

This pass began from the security-specific checklist rather than adopting the correctness verdict. Highest-risk source was independently re-opened, including migration 0001, `recovery.rs`, `tx.rs`, `audit.rs`, `blob.rs`, storage/engine error formatting, `fault.rs`, crate feature gates, both relevant Cargo manifests, engine journal/recovery routing, and the current P2I source/test diff.

### Authority constraints and lease fencing

- Migration 0001 binds task steps to tasks by FK, leases to steps by FK, receipts to both task and step, blob references to exact `(digest, data_class_rank)`, and plan revisions to their task and exact plan blob. Receipt insert triggers bind receipt task and idempotency key to its step and require a succeeded step; a journal trigger binds an optional journal step to its task. Step constraints bind lease owner/expiry presence to in-flight statuses, require positive attempts/generations outside PLANNED, and constrain terminal/error fields.
- Storage is the transaction and SQL authority. `Tx` keeps its transaction private to the crate, exposes no raw SQL escape, and its compile-fail examples prohibit extracting the underlying transaction or calling internal outcome/journal methods. `LeaseGuard` is not described as SQLite authority; mutating predicates still check durable owner/generation/status/expiry facts. No cross-database identity guarantee is claimed.
- Blob dispatch checks class before canonicalization and dedupe. PRIVATE requires an injected protection backend even on reuse; SECRET/CREDENTIAL refuse. Lookup is exact class plus digest and validates marker, size, canonical content, and digest before returning bytes. This is not represented as a real PRIVATE backend or ciphertext authentication guarantee.

### Task/step binding, classification, and receipt authority

- Storage APIs operate on typed task/step inputs and transaction-scoped methods; SQLite FKs/triggers/checks provide a second binding layer. Receipt-bearing outcomes are written within the fenced outcome operation, after the step is accepted as succeeded, with task aggregate, references, receipt, lease release, and audit in the same operation envelope. Receipt authority is not inferred from caller prose.
- Recovery only repairs a stale task aggregate after `prove_receipt_repair` establishes corroborating durable receipt/result/step facts. Recovery action validation binds requested action to the observed state, step, lease, attempt budget, receipt, and projection; the current snapshot is re-inspected and fingerprint-compared before writes. Conditional lease revocation repeats durable owner/generation/timestamps/status/task predicates in SQL.
- Unknown state/status and attributable semantic corruption are handled fail-closed by quarantine/refusal paths. Global catalog/FK preflight occurs before task-local writes. No recovery path invokes execution or fabricates an external result. Zero-attempt PLANNED work now refuses the complete savepoint with payload-free `InvalidRecoveryAction`; the mixed-pass regression confirms an earlier repair rolls back.

### Recovery capability and audit/journal authority

- `RecoveryPass` is an opaque borrowed capability with only recovery-specific preflight, list/count, inspect, and apply methods. Compile-fail examples prohibit lease acquisition, attempt start, outcome commit, arbitrary SQL, extracting its transaction, or constructing it from `Tx`. `recovery_pass` wraps the complete body in an operation savepoint; a returned error rolls back even if the caller catches it.
- The only production audit participant is the task-engine `TaskJournal`. It receives immutable storage-constructed `DurableTransition` facts, not SQL/connection/`Tx`; its record drafts are validated as a complete batch, canonicalized, rebound to actual facts, and persisted by storage in the same savepoint. Recovery audit payloads use static decision names and fingerprints; raw malformed state/status values are not included in journal prose. ADR-0021 and closure claims explicitly limit retained history to supported audited operations, excluding low-level lease-only operations and deletion-after-cascade.
- Storage and engine error formatters render fixed categories only. `StoreError` has no source chain that exposes SQLite diagnostics; conversion maps raw SQLite failures to typed categories. Group O12 checks this boundary. No raw SQL upward escape was found in the production crate APIs.

### Fault feature, subprocess, and release artifact boundary

- The source diff narrows, rather than overstates, the seam claim. `p2h-fault-injection` is not a default feature; the workspace requests it only through `serea-task-engine`'s dev-dependency edge. `fault` module and all reach sites are cfg-gated. Explicit feature-enabled artifacts are acknowledged as seam-bearing and excluded from the default-production proof claim.
- `fault.rs` has no environment-variable arming route and no callback/hook stored in `Store`; its one-shot thread-local actions are reached at named private transaction sites. Crash action acknowledges and blocks until supervisor termination; injected `Fail` is explicitly an error/rollback rather than a crash.
- P2H's harness confines subprocess use to the crash integration-test path, with explicit acknowledgements and bounded child handling. Existing P2H evidence distinguishes fresh-process N6 durable-state verification from N7 integrity/FK stress-only behavior. The release proof completed exit 0 and establishes only the scoped default storage/workspace absence, explicit-feature positive control, and deliberate crash-test presence. The script was inspected; the independent proof evidence is recorded in `P2H-review-and-closure.md` and the bounded coverage ledger. No all-features exclusion claim is made.

### Runtime dependency and execution boundary

- Direct manifest review and `cargo tree --offline --workspace --edges normal` show runtime direction `task-engine -> storage -> protocol`; storage does not depend on engine, and testkit is not reachable on normal runtime edges. Engine `rusqlite` is dev-only. Normal graph contains no network/provider runtime path; protocol/storage/task-engine source and O1/O2/O4/O9/O11 smoke assertions are consistent with no network sockets, provider/model execution, event runtime, or event construction in the P2 production route.
- Group O guard review found the checks scoped to the frozen four-member workspace and required claims, with virtual-manifest regression cases for aliases, inherited dependencies, target/build scopes, malformed syntax and testkit/internal dependency reachability. O1's textual source guard is not treated as a complete semantic proof by itself; it is complemented by manifest parsing, actual Cargo metadata/tree, and direct source review. O2 similarly checks the architectural subprocess boundary and is complemented by direct harness inspection. O6/O8/O9/O11/O12/O14/O15 checks align with the reviewed source/schema; no trivial false positive was identified that invalidates their present positive result.

### Migration, dependency, vendor, and claim disposition

- Migration remains exactly `0001_initial.sql`, checksum `d9068dccbc26ececb71be79c475080633166ba0163c62b2d98b9733512baefea`. The current diff does not alter migration, Cargo.lock resolution, or vendor source. The four workspace members and normal dependency graph remain unchanged.
- The bounded A12 provenance report records exact `jsonschema-value` 0.58.3 identity, official archive checksum agreement, documented local patch-only source differences, retained license/provenance information, and focused behavioral tests. This is sufficient for the P2 dependency disposition; it is not a transitive vulnerability audit.
- Deferred claims remain explicit: E3/E4, event bus/backfill, scheduler, provider/model execution, real PRIVATE encryption/key custody and complete ordinary-row PRIVATE support, SECRET/CREDENTIAL storage, C4/T6, arbitrary operational budgets, complete low-level/deletion audit history, power-loss/fsync/custom-VFS or Apple Silicon empirical guarantees, and full third-party vulnerability certification. These are not P2 closure blockers under the frozen acceptance scope.

## Findings

No unresolved security/authority Blocker or Major was found in the complete bounded P2 scope. No release-relevant security test gap remains in the required coverage cells. The generation-0 report's A1/A8/A11/A12/A9 uncertainty is resolved by the finite ledgers and evidence cited above; no unsupported claim is promoted to a guarantee.

## Validation basis and limits

This terminal security review itself was read-only. It re-ran the offline normal-edge Cargo dependency tree and inspected the current diff/source. The latest previously completed candidate evidence recorded before this report includes 74 Group O Python tests PASS, focused P2I regressions PASS, and release exclusion proof exit 0. The user-required full final stable/MSRV validation matrix remains a separate mandatory gate after both terminal reports; this report does not claim that matrix has been rerun after the report was written.

## Verdict

PASS
