# P2 Durable Task Runtime — Closure Evidence (CLOSED)

- **Repository:** Serea
- **Status:** **CLOSED — all P2I review and validation gates are GREEN.**
- **Current branch:** `p2/p2i-final-closure`
- **P2H commit:** `8417b1fff325e311120050f6c733f185111a90bc`
- **P2H parent:** `f50fd8f0aa01ae8847c92506a61015fa586a69ec`
- **P2I commit:** `0826c16dd77bf50a724b84377e2e2d88ab8d1bcf`
- **Pushed:** no
- **P3 production implementation:** not started

The generation-0 P2A–P2H review reports below remain preserved as historical evidence, including their coverage limits. The terminal whole-P2 reviews are recorded separately and passed; later evidence closed the named coverage cells without rewriting the earlier review history.

## Phase history

| Phase | Commit | Scope/status |
|---|---|---|
| P2A | `824c6ce1931cc5b4f9f77e72bdbedc600a2e31af` | Protocol and canonical corrections |
| P2B | `5d63c5f661670aa8ac2dea35f49a9e2762161bc8` | Clock and time |
| P2C | `03d71cd2e8875ef3c5cfd75dade4ad48952384f7` | SQLite storage foundation and migration 0001 |
| P2D | `e6c57a03211d8493ac82cdd35ab402379ccf24da` | Blob/classification boundary |
| P2E | `d3413af80a0f2b82926c643607a0ffad1f55fcb5` | Lease authority |
| P2F-a | `e1b71040366a0bad8e1b70739d37281c3b895720` | Atomic outcome fencing |
| P2F-b | `7625d206e1fe6794fd21c63ef9b97a8546289e5a` | Task-engine integration |
| P2G | `f50fd8f0aa01ae8847c92506a61015fa586a69ec` | Recovery |
| P2H | `8417b1fff325e311120050f6c733f185111a90bc` | Crash/fault injection |
| P2I | `0826c16dd77bf50a724b84377e2e2d88ab8d1bcf` | Group O, whole-P2 review reconciliation and this closure record; CLOSED |

The P2H commit has exactly the specified P2G parent. The P2I branch was created only after the P2H commit was verified clean. There has been no amend or push.

## Workspace and dependency architecture

Exactly four workspace members:

```text
serea-protocol
    ↑
serea-storage
    ↑
serea-task-engine

serea-testkit (dev-only)
```

The runtime dependency direction is `serea-task-engine → serea-storage → serea-protocol`; the engine also depends directly on protocol. Storage does not depend on engine. `rusqlite` is storage runtime and engine dev-only. `serea-testkit` is not reachable from either runtime crate. The current smoke guard verifies workspace membership and manifest dependency scopes; the offline normal-edge Cargo tree was also inspected.

No event bus, scheduler, provider/model runtime, capability registry, policy engine, approval renderer, memory store, or new production dependency was added by P2A–P2H. P2I added no dependency or migration.

## Persistence and protocol identity

- Migration set: exactly migration `0001_initial.sql`; no migration 0002.
- Migration 0001 SHA-256: `d9068dccbc26ececb71be79c475080633166ba0163c62b2d98b9733512baefea`.
- P2A protocol surfaces: `serea-arch/1.0.0`, `serea.task/2`, `serea.action/2`; `serea.event/1` remains a frozen protocol surface, not an event runtime.
- The migration checksum and absence of migration/vendor/lockfile changes were verified at P2H. P2I source/test/documentation changes do not modify schema or dependency resolution.

## Current P2I changes and evidence

P2I's Group O guards are in `tests/workspace_smoke_tests.py` as O1–O15. They complement, rather than replace, existing Rust tests and the workspace smoke guard. O6 mechanically rejects a task-state writer on `Store` outside the transaction seam; O10 scans migration 0001; O14 inspects all seven class-cap CHECKs and uses Python SQLite to show that `ignore_check_constraints` disables the blob class cap; O15 pins the authority triggers/FKs. O13's N7 assertions remain deliberately variable and stress-only.

Two findings from independent review were verified and fixed with tests:

1. **Task schema step count:** `assistant-task.schema.json` already structurally caps `steps` at 1024. Storage now rejects plans larger than that existing structural constraint while accepting exactly 1024; the regression verifies refusal is atomic and that the boundary projection loads with 1024 steps.
2. **Failure detail shape:** the existing task schema caps `error.details` at 64 properties. Storage now refuses larger canonical objects before the fenced failure write; the regression verifies 65 properties refuse and leave the step in `EXECUTING` with no error outcome.

No new operational resource bound was selected. Payload bytes, blob bytes, arbitrary object counts and remaining numeric limits are still open. ADR-0020 classifies the two existing schema caps as structural value predicates.

For a valid `PLANNED` step with `max_attempts_per_step = 0`, recovery conservatively refuses the whole atomic pass with `InvalidRecoveryAction` rather than inventing work or a new outcome. Regressions assert both unchanged logical state for a single unsupported task and rollback of an earlier eligible repair when a later zero-budget task refuses the mixed pass. It does not claim a successful classification/resume report for disabled work.

## Crash, fault and recovery evidence

P2H owns N1–N8 and F25/F26. The harness is coordinator → writer/crash child → distinct fresh verifier child, all using the exact inherited fixture identity with bounded waits and explicit acknowledgements.

- **N1–N5:** child process death at named pre-begin, post-begin, partial-write and pre-commit windows; fresh-process durable assertions prove absence/rollback and integrity.
- **N6/N6a:** after `COMMIT` returns but before application-success publication, the child is killed; a separate fresh verifier proves complete committed P2F state. Caller success is not observed.
- **N6b:** terminal recovery is a strict logical no-op; nonterminal recovery may add the existing P2G classification audit but does not reconstruct an outcome, duplicate receipt/outcome journal, or execute work.
- **N7:** repeated commit/SIGKILL stress only. It does not prove that any kill landed inside SQLite `COMMIT`, requires neither durable category, and pins no exact count. No power-loss, faulty-device, or fsync/VFS failure guarantee follows.
- **N8:** deterministic fault-injection rollback after a fenced write and before inspection; this is not called a process crash.
- **F25/F26:** temp paths include binary identity/process/counter; crash child reopens the parent-created exact absolute fixture directory from a distinct working directory.

The fault module is feature-gated and absent in normal/default production configurations. The workspace requests `p2h-fault-injection` only through the engine dev-dependency edge. An explicitly feature-enabled artifact is seam-bearing; universal exclusion under arbitrary downstream feature combinations is **not** claimed. The P2H release proof previously completed A/B/C/D successfully on the local macOS host and is recorded in `P2H-review-and-closure.md`; one independent P2I reviewer later timed out while attempting a fresh proof, which is not counted as a second proof or as a pass. No Apple Silicon or cross-platform empirical claim is made.

## Final validation evidence at the P2I tree

After both terminal reviews were recorded, final validation was rerun:

- Stable: `cargo fmt --all -- --check`; workspace all-target/all-feature offline check; workspace all-target and all-feature offline tests; and warning-denied Clippy all exited 0.
- MSRV: the same workspace check/test/Clippy commands with `cargo +1.85.0` all exited 0.
- Stable and MSRV storage and task-engine all-feature debug/release test modes all exited 0.
- Workspace all-feature test output: **826 regular tests + 45 doctests = 871 executions**, zero failures and zero ignored. The stable all-feature suite with `--test-threads=1` ran twice; logs matched after normalizing duration-only presentation.
- P2H crash suite: **21/21** passed on stable and Rust 1.85.0. N7 remains stress-only; no in-COMMIT kill or power-loss guarantee is inferred.
- Group O: `python3 -m unittest discover -s tests -p workspace_smoke_tests.py` — **74 passed**, O1–O15 GREEN. `python3 tests/workspace_smoke.py` passed.
- `python3 tools/validate_docs.py docs` passed; **80 Markdown files** scanned.
- `tools/prove_release_fault_exclusion.sh target/p2i-release-proof` completed exit **0**: default storage/workspace artifacts seam-absent; explicit feature positive control seam-present; crash test target seam-present. Scope excludes arbitrary all-features production artifacts.
- `cargo tree --offline --workspace --edges normal`, `cargo metadata --no-deps --format-version 1 --offline`, and `git diff --check` completed successfully. The normal runtime graph remains protocol → storage → task-engine with testkit not on runtime edges.

## Whole-P2 review gate — CLOSED

The generation-0 reports [correctness/durability](P2I-review-correctness.md) and [security/authority/claims](P2I-review-security.md) remain unchanged and explicitly incomplete; they are preserved as lineage, not presented as terminal approvals. Finite supporting evidence closes the named coverage cells in [P2I-review-coverage.md](P2I-review-coverage.md), [P2I-recovery-coverage.md](P2I-recovery-coverage.md), and [P2I-vendor-provenance.md](P2I-vendor-provenance.md).

- **A1:** all four bounded history passes PASS; all 242 unique P2A–P2H changed paths assigned and accounted for.
- **A8:** final P2G classification matrix PASS; no GAP remains, including the zero-attempt PLANNED refusal disposition.
- **A9:** scoped release fault-seam proof PASS, exit 0.
- **A11:** all nine phase closure records reconciled; no material current-truth contradiction remains.
- **A12:** exact `jsonschema-value` 0.58.3 archive/checksum and local patch disposition PASS; complete transitive vulnerability certification remains a nonclaim.
- **Terminal correctness/durability:** [P2I-review-correctness-terminal.md](P2I-review-correctness-terminal.md) — PASS.
- **Terminal security/authority:** [P2I-review-security-terminal.md](P2I-review-security-terminal.md) — PASS.

No unresolved Blocker, Major, or release-relevant test gap remains within the frozen P2 scope. Explicit P2 nonclaims and later-phase ADR obligations remain preserved below. Terminal correctness and security reviews both passed.

## ADR status and P2 nonclaims

| ADR | Current P2 disposition |
|---|---|
| ADR-0018 | Accepted; P2A wire and scoped lifecycle/recovery implementation are present. |
| ADR-0019 | Accepted; P2A SCJ-1/digest/IDK-1 implementation is present. |
| ADR-0020 | Accepted; structural-schema versus operational-bound distinction remains. |
| ADR-0021 | **Proposed**; P2 journal/audit seam is implemented; P3 event gate and E3/E4 remain outstanding. |
| ADR-0022 | **Proposed**; fail-closed PRIVATE blob dispatch exists; real backend/key custody and ordinary-row PRIVATE representation remain open. |
| ADR-0023 | Accepted; P2A O/L/P validation is implemented. |
| ADR-0024 | **Proposed**; scoped runtime fencing/recovery is implemented; full architecture ratification remains outstanding. |

P2 does **not** provide:

- E3 or E4;
- an event bus or event backfill;
- Capability Registry C4's `side_effect_class != NONE` second half;
- T6 descriptor/risk comparison;
- a real PRIVATE encryption backend;
- complete ordinary-row PRIVATE support;
- a SECRET sealed store or CREDENTIAL store;
- arbitrary resource-bound values;
- provider execution;
- approval rendering;
- scheduler behavior;
- a real GoalLatch;
- empirical Apple Silicon validation.

P2 journals supported audited task-engine transition operations; it does not claim a complete retained audit trail for low-level lease-only operations or deletion after cascade.

## Open items carried beyond P2

- **ADR-0021:** P2 runtime gate complete; P3 event gate outstanding.
- **ADR-0022:** real backend/key custody, SECRET store and ordinary-row PRIVATE representation.
- **ADR-0024:** runtime fencing implemented; architecture ratification remains honestly Proposed.
- **Resource bounds:** payload bytes, blob bytes, object counts and remaining numeric limits unselected; existing structural schema caps are not operational budgets.
- **P5:** descriptor-dependent receipt policy, C4 second half and T6.
- **P6/P12:** approval/device roster behavior.
- **P3:** event bus and scheduler design/implementation boundary; no retroactive P2 event fabrication.

## Next phase boundary

**P2 status is CLOSED. P2I is CLOSED.** P2I commit: `0826c16dd77bf50a724b84377e2e2d88ab8d1bcf`; parent: `8417b1fff325e311120050f6c733f185111a90bc`. Terminal correctness review: PASS. Terminal security review: PASS. The worktree after the P2I commit was clean. No P3 production code or migration has begun.
