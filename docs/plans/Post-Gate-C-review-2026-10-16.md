# Post-Gate-C review record — 2026-10-16

**Purpose:** review evidence and prepare one Owner review across P8A, migration ordering, and P7 Gate C.
**Status:** engineering review only. No new ADR/Protocol/contract is ratified here. P8 D01–D07 recommendations remain **PROPOSED / NOT RATIFIED**. This is not an independent security review or production approval.

## Live baseline

- Repository baseline: `e0acb0254820c381dd97828824bef86a1db5176c`, fetched `origin/main` and used to create `codex/post-gate-c-review`.
- PR #14: GitHub PR endpoint reports OPEN, DRAFT, UNMERGED at head `cb4df7178a4dc74b124006c4bf2c020cd626aa5e`. Its checks on that head report Linux stable, Linux MSRV 1.85.0 and SQLite cross-architecture checks passing. The dedicated P5 activation fix remains unmerged.
- PR #15: MERGED to the baseline above. Its description records the synthetic Gate C work and remaining production blockers.
- Main's latest workflows at inspection: Fast `38887469390`, Full `38887469433`, Native platform `38887469455`, SQLite portability `38887469452`; all successful on `e0acb025...`. Native validation ran Windows/Linux x86_64 and ARM64 plus macOS x86_64/ARM64; portability ran Intel↔ARM64 DB production/open in both directions. These CI results are GitHub evidence, not tests from this branch.
- This review is published as Draft PR #19, head `7472f8cc323297a49c2b52550721cb28b4420bfb`. Its Fast/Full/Native/SQLite/Git Hooks Windows workflows are pending; the baseline results above must not be mistaken for PR #19 checks.
- No `AGENTS.md` exists in the repository. `docs/git-safety.md` requires repository-local Codex identity, preflight before commit/push, hooks, explicit branch targets, and forbids implicit/force pushes.

## P8A Owner review summary

The detailed per-decision contract, alternatives, risks, dependencies, and proposed test names are in [P8A owner decision package](P8A-owner-decision-package.md). Its recommendations remain **PROPOSED / NOT RATIFIED**. Code inspection confirms the package's main implementation boundary:

- `TaskEngine::evaluate_prepared_action` is a point-in-time evaluation, not dispatch authority. No P8 provider dispatch intent API, RequestId minting, or production provider invocation path was found in the reviewed TaskEngine/Capability/Host code.
- P6 persists approval/grant facts and joinable grant-use consumption; Storage has durable lease fencing and transaction participants. P8's atomic composition is not implemented.
- Existing `side_effect_receipts` has unique Step binding, Task/Step identity constraints and the succeeded-Step trigger. Normal outcome commit is lease-fenced and does not accept a late result against a cancelled Task.
- No production CredentialStore provider boundary is ready; no real credentials/accounts were accessed.

Owner decision checklist, all **PROPOSED / NOT RATIFIED**:

1. **D01 dispatch eligibility:** ratify exact Task states and transaction linearization for deadline, cancellation and lease. Recommendation is no dispatch from PLANNED/WAITING/terminal states; only an atomic, current-state admission into EXECUTING may commit the first intent. A grant/evaluation is not a permit.
2. **D02 commit boundary:** retain Accepted ADR-0039 Option B: grant use, budget, IDK reservation, RequestId and intent commit atomically; no provider IO inside the transaction. Select/freeze the only typed API owner. Pre-commit failure must leave no use/intent/budget; uncertain commit is resolved by exact intent lookup.
3. **D03 same-Step retry:** decide whether retry can cause provider IO, its fresh policy/approval rules, and how the historical one-per-Step grant use relates to retry. Same logical Step keeps IDK-1; new action after confirmed absence uses a new StepId/key and fresh authority.
4. **D04 AMBIGUOUS conflict:** ADR-0036 says never blindly retry an AMBIGUOUS effect, while Capability Protocol §8.1 says IDEMPOTENT retry is allowed and Task Protocol has same-Step retry language. Ratify the meaning of `allowed`, provider-native idempotency horizon, and no-effect proof. Any frozen Protocol amendment requires separate approval before invocation.
5. **D05 result, Receipt, cancellation:** choose among a dedicated late-outcome reconciliation record (proposed Option A), fail-closed unresolved evidence (Option C), or another explicitly reviewed lifecycle. Keep Task cancelled, never infer a Receipt, preserve unique-Step canonical receipt authority, and define retention/Forget behavior for unresolved effects.
6. **D06 credential custody:** approve no real-provider work until immutable host provider/account binding, scoped opaque credential handle, OS auth/elevation, revocation and rotation rules are specified and independently validated. Alternative is to defer all real integrations.
7. **D07 fake-provider gate:** specify required deterministic no-network fake effect ledger, manual clock, SQLite/process restart and failure hooks. The package's listed fake-provider tests are future tests and do not exist as P8 execution evidence.

The package already supplies proposed alternatives and security risks per item. No D01–D07 decision can be inferred from code or synthetic tests. Synthetic dispatch in Host is an in-process fake and remains separate from CapabilityProvider invocation.

## P7/P8 migration collision

### Facts

- Ordinary catalog is schema v8 (migrations 0001–0008).
- `p7_experimental_memory_v9.sql` is a standalone test fixture appended as version 9 only for test builds with `p7-experimental`; it is not an ordinary migration.
- `migrate::inspect` validates every applied row as a contiguous prefix including exact migration name and raw SQL checksum; ordinary builds reject versions newer than 8. Editing/reusing a committed version would fail catalog/checksum validation.
- ADR-0043 explicitly reserves canonical migration 0009 for reviewed P7 schema and says the synthetic v9 stays fixture-only. Version 0009 does not have two meanings in a production catalog.
- Disposable SQLite 3.53.1 DB: executed exact 0001–0008 SQL, then the synthetic P7 fixture as a candidate v9 with `PRAGMA foreign_keys=ON`; `quick_check` returned `ok`, `foreign_key_check` returned no rows. This validates only this SQL sequence on the Python SQLite build, not bundled SQLite portability or any nonexistent P8 DDL. Existing Rust migration tests use bundled SQLite 3.53.2.
- P7 owner identity is an immutable owner-scoped primary key; `p7_task_owner_bindings` binds TaskId to OwnerId; Memory tables use composite owner/memory keys and exact parent keys. The Rust P7 migration test checks composite parent-key indexes and foreign-key integrity.

### Options for Owner

| | A — P7 canonical 0009, P8 0010+ | B — amend ADR, P8 canonical 0009, P7 later |
|---|---|---|
| Authority | Consistent with Accepted ADR-0043. | Blocked until Owner explicitly amends ADR-0043; no P8 migration exists to review. |
| Upgrade from v8 | P7 binds/quarantines legacy Tasks before later P8 schema. | P8 schema must work before P7 OwnerId binding exists or define a reviewed dependency/compatibility strategy. |
| OwnerId/FK | P7's `OwnerId` is available for later P8 tables; composite FKs need exact UNIQUE/PK parent keys. | P8 cannot silently assume P7 tables/OwnerId. Adding owner binding later creates schema and data/backfill questions. |
| Checksum/history | P7 owns v9 once approved; P8 begins at v10. Existing v8 stores have a straightforward forward path. | P8 owns v9 and its exact checksum permanently; P7 becomes v10. Test fixture/catalog names and tests must change. No production v9 is deployed today, but the ADR reservation must change before implementation. |
| Tests / rollout | Update catalog and fixtures for P7 v9 then P8 v10; validate old v8 upgrades and every FK. | Update fixture/version assertions and migration chain; validate v8→P8 v9→P7 v10 plus old Tasks and Owner binding. |

**Assessment:** A is the only presently authorized order and matches the explicit ADR-0043 reservation. B is a possible future design only after an Owner ADR amendment and P8 schema/dependency proof. No canonical migration was added or applied; fixture SQL was applied only in a deleted disposable file. No P8 DDL exists, so B's actual SQL, checksum, FK structure and rollout compatibility are **NOT EXECUTED / NOT PROVABLE YET**. Do not merge experimental v9 into an ordinary store.

## P7 Gate C security review

P7-IR-01–04 tests were rerun locally on stable Rust 1.99.0, `serea-storage`, features `p7-experimental,p7-gate-c`, isolated file-backed synthetic fixtures. Results: IR-01–03 selected tests 11/11 passed; IR-04 process crash/restart tests 6/6 passed. The tests exercise retired Memory ID reuse denial, ledger/checkpoint preflight capacity refusal, shared/exclusive Blob GC and restart recovery after the last closure chunk. Historical RED observations and remediation explanations are retained in the P7 campaign ledger and independent-review handoff. GREEN means regression behavior is currently reproduced in Synthetic fixtures; it is not production safety.

| Area | Re-review disposition and remaining limit |
|---|---|
| IR-01 retired ID reuse | **RED:** before the fix, Insert→Forget→Insert of the same `memory_id` returned `Ok(())`. **GREEN:** API and fixture trigger now deny reuse after Forget, including a fresh operation/reopen; quota/FTS consistency checked. Owner scope and fixture-only schema. |
| IR-02 capacity | **RED:** old append admission let a capacity error leave SQLite in `QUARANTINED` with a pending operation/generation despite no complete intent. **GREEN:** ledger-frame and checkpoint-revision refusal now occurs before quarantine/intent; complete authenticated intent recovery is tested. At finite append/checkpoint ceilings it fails closed; no approved rollover/compaction/tombstone retirement exists. Availability eventually stops. |
| IR-03 Blob GC | **RED:** after Forget, the exclusive orphan Blob row count remained 1 (expected 0). **GREEN:** exclusive deletion, same digest at different class ranks, shared Memory/Task/Schedule/occurrence/Model refs, candidate bound and crash/reopen are tested. Physical erase, hostile concurrent file replacement and storage-device guarantees are not proven. |
| IR-04 final chunk crash | **RED:** child exit after the complete final Chunk but before SQLite pending adoption left `(READY, generation 0)` with authenticated Intent+Chunk, and old startup recovery returned `Err(Stale)`. **GREEN:** deterministic child exit at that point now recovers with 1-target and multi-chunk tests, atomic pending crash windows, near-capacity reserve and GC. This proves the tested process checkpoints only. |
| Rollback freshness | Partial local freshness checks detect missing/corrupt checkpoints, stale valid prefixes and generation mismatch while newer trusted local evidence remains. Deleting/restoring the latest checkpoint can leave an older valid checkpoint accepted; restoring DB+ledger+checkpoint+key together to a valid older snapshot is not detectable. No independent rollback-surviving authority exists. |
| Key custody | Synthetic deterministic test keyring only. No production OS keychain/keystore, key provisioning, access control, backup, rotation operations or recovery proof. |
| OS authentication | `HostAuthenticator` is a port with unsupported-platform adapter; synthetic evidence does not validate macOS/Windows/Linux user authentication, freshness or assurance. |
| Ledger rollover | Append-only ledger/checkpoint histories have hard 1,024-frame/revision ceilings; safe compaction/rollover is only a proposal and remains unresolved. No pruning or tombstone expiry accepted. |
| Filesystem faults | Simulated short-write/incomplete-frame faults and process termination are covered. `sync_all` is not a portable power-loss/directory-entry guarantee; arbitrary real filesystem/media faults and OS-specific secure durability were not tested. |
| Confidentiality | Synthetic schema limits classes to PUBLIC/PERSONAL, but this does not prove production at-rest confidentiality. No real personal data or production key was used; plaintext exclusion from all host surfaces needs platform and independent validation. |

The historical independent-review handoff describes cooperating locks and CI coverage, and explicitly lists independent reviewer disposition as pending. This task has not obtained or performed a formal independent review. No Gate D or production Memory activation is authorized.

## Hostile cross-phase audit

- **Stale Policy/Grant/Lease and privilege escalation:** P8 currently has no dispatch commit API. `evaluate_prepared_action` is non-authoritative; P6 membership/use constraints are durable, but only the unimplemented P8 transaction can close the live revalidation race. A stale grant/use row cannot be treated as a retry permit under the proposed contract.
- **Duplicate effect / IDK / RequestId:** no P8 provider invoke path is present. ADR-0036 requires new RequestId per committed intent and stable IDK-1 within one logical Step; fake-provider effect-ledger and crash/restart tests remain future gates. Do not infer exactly-once from the Synthetic P7 ledger.
- **AMBIGUOUS:** accepted ADR and Capability Protocol wording remains inconsistent. Safe implementation cannot choose the liberal retry interpretation without an Owner decision or approved Protocol clarification.
- **Cancellation / late result / false Receipt:** normal P2 success is fenced to current Task/Step/lease state; the SQLite trigger requires a succeeded Step and receipt key/task bindings. It refuses a cancelled Task late result. Until a separate approved path exists, unresolved external result must not be presented as canonical success/Receipt.
- **Migration collision / checksum:** normal v8 and exact contiguous name/checksum validation prevent reusing version 0009 with different bytes. Synthetic fixture is excluded from the ordinary catalog; no canonical P8/P7 change was applied.
- **Retention / restart:** P7 tests cover restart reconciliation after specified child-process exits. Append-only ledger/checkpoint capacity has no rollover or approved tombstone retirement; deletion safety evidence may exhaust storage and stop future Forget. Physical erase and production unresolved-effect retention are unresolved.
- **CI false positive:** latest main workflow run metadata was checked against exact head `e0acb025...` and each relevant job conclusion, including Windows Gate C, Linux MSRV, stable full suite, native macOS/Linux/Windows, and SQLite cross-architecture. Those are real CI jobs, but do not establish production properties or independent review.

## Execution record

- Focused commands and results are retained in `ExecPlan-post-gate-c-2026-10-16.md`; stable Workspace all-target and all-feature suites, focused IR tests, Rust 1.85 all-target/all-feature check, Clippy, formatting and docs checks passed. Draft PR #19 is open; its GitHub CI is pending.
- No source-code fix has been made because the scoped findings are either unresolved Owner choices, explicitly unapproved production guarantees, or already covered synthetic regression behavior. Any newly discovered implementation defect must be reproduced and corrected against an Accepted invariant before closing.
- Owner review remains required for P8 D01–D07, the ADR-0043 version reservation if considering option B, and any proposed Protocol/ADR amendment.
