# P8B Owner-ratified execution plan (2026-10-17)

## Objective

Record the Owner's D1/D2/D3/D5 approved directions and D4 interim-only decision, prove the test-only D3 by-value Permit behavior, and compare authority boundaries A/B without creating a callable runtime dispatch surface. Preserve Proposed and Blocked details as unresolved gates.

## Intake baseline

- Source documents: `ADR-0045 (DRAFT)` and `Serea_Codex_P8B_Owner_Ratified_Long_Horizon_2026-10-17.md`, supplied with this task.
- `main`: `e0acb0254820c381dd97828824bef86a1db5176c` (fetched from origin, 2026-10-17).
- PR #27: `c08615aa08b91abcd67f7ca4a3ac7a0512d7cb95`, based on PR #26 `72be5ac56d01765b8c9dea79ee3e4f9d1fc5bbc4`.
- Local original checkout `/workspace/Serea`: branch `work`, clean, HEAD equals current main. Work is isolated at `/workspace/Serea-p8b-owner`, branch `p8b/owner-ratified-boundary-2026-10-17`, starting at PR #27 HEAD.
- PRs #19–#27 remain open Drafts. Latest observed PR #27 runs were Fast `38985315264`, Full `38985315198`, Native `38985315222`, SQLite portability `38985315217`, Git Hooks `38985315215`, all successful for the starting SHA. These do not validate this branch.
- Rust 1.85.0 is present but `rustc +1.85.0` fails with “missing manifest in toolchain”; stable resolved to Rust 1.99.1. Offline Cargo initially lacked `async-trait`; online fetch is being attempted. Do not claim local tests until they execute.

## Decision boundaries

| Direction | Ratified scope | Still Proposed / Blocked |
|---|---|---|
| D1 | A committed Provider Dispatch Intent charges one Task tool-call for OBSERVE and EFFECT; no-Intent local observation/denial/duplicate suppression costs zero; no refund | Canonical monotonic counter/schema, overflow, restore/Forget, reconciliation accounting |
| D2 | Host owns exact live Provider instance; fresh readiness before commit and immediately before invoke; pin request/provider/descriptor/registry/overlay | Freshness duration, Host authority type, lifecycle locking, postcommit outcome |
| D3 | Opaque non-Clone, non-serializable Permit consumed by value on every attempt including mismatch; only after committed/revalidated Intent; never rebuild on restart | Authority crate/API, exact cancellation/readiness/outcome/Receipt state machine |
| D4 | Interim refusal to erase unresolved Intent or essential correlation; missing evidence never permits replay; quarantine unverified restore | No finite/infinite production retention policy, tombstone, override, Gate D, migration, or deletion guarantee approved |
| D5 | Existing `CAPABILITY_REQUESTED` means committed Intent only; minimal correlation and IDK digest, no raw IDK/secrets/args/response | Exact payload type, fields/limits/grammar, digest domain, privacy and retention validation |

Non-authorizations: production Memory/provider/credentials/effects/Receipt/retry, canonical migrations 0009/0010, main push, merge/close/retarget/force push, branch-protection bypass.

## Work waves and status

1. **Intake** — complete. GitHub baseline, clean state, PRs, workflows, toolchain and isolated worktree captured above.
2. **ADR incorporation** — complete in this branch. ADR-0045 number was unused on the fetched starting head. The file and decision index describe direction-only ratification and exclusions. Docs validator passes.
3. **A/B authority boundary** — initial code/manifest review complete; architecture gate remains open. Evidence and matrix below. No public candidate API exists to externally forge, so no false compile-fail evidence is claimed.
4. **D3 Permit slice** — implementation and focused tests complete; final reviewer/regression pass pending. `invoke_once(self, ...)` consumes before binding checks; no consumed flag and no `Clone`/`Copy` derive. The synthetic binding covers RequestId+IDK, TaskId/StepId, Owner, Lease generation, ProviderId+instance, Registry generation, descriptor digest and Overlay revision. The old mismatch-then-reuse test is replaced by `mismatched_attempt_burns_by_value_permit_without_effect`.
5. **Synthetic regression / independent audit** — pending. Run allowed local validation only; inspect feature gates, migrations, and replay/forget/event claims. Do not fill missing contract details.
6. **GitHub** — pending. At most one new Draft PR after cohesive verified work. Never alter existing PRs or main. Exact-head Fast/Full/Native/SQLite/Git Hooks required; current SHA results count only after final push.
7. **Handoff** — pending. Update this file with commands, outcomes, risks and exact next slice. Report in Japanese.

## A/B review rubric

- A: ownership consolidated behind a crate-private seam; inspect whether this can own authoritative Storage transaction inputs without moving all Storage or cycling `serea-policy -> serea-storage` dependencies.
- B: public Storage operation must be safe against arbitrary callers and independently revalidate policy, Grant, Task, Lease, Registry, Overlay, time, and Host readiness. Caller booleans/evidence are not proof; generic `Store::transact` is an existing broad mutation surface to account for.
- For both: attempt external construction of proofs/permits and arbitrary Intent writes; analyze races, rollback, compatibility, dependency cycles, feature leakage, reentrancy and testability. If completeness cannot be proven, mark blocked with the exact missing authority source; do not select by preference.

## Initial A/B evidence (code inspection)

Current workspace graph includes `serea-task-engine -> {serea-storage, serea-policy}` and `serea-policy -> serea-storage`. `serea-storage::Store::transact` is public and accepts a caller closure with `&mut Tx`; `Tx` exposes typed operations but no SQL handle. `serea-storage` cannot safely recompute P6 policy by importing `serea-policy` without a cycle. The Host/live Provider instance is above both crates and not represented by a Storage value.

| Candidate | What it can own | Main bypass/obstruction | Current disposition |
|---|---|---|---|
| A: TaskEngine authority module | Coordinate Task lifecycle, Policy evaluation and Store transaction; can keep Permit private to TaskEngine | Does not by itself own Host's live Provider/readiness; a public TaskEngine entry must authenticate inputs and atomically couple Store writes; broad crate refactor/seam changes need review | Plausible investigation path, not proved or selected |
| B: public self-verifying Storage API | Storage transaction and typed DB facts | Storage cannot recompute Policy without dependency cycle, cannot prove in-process Host readiness, and public `transact` remains an alternate mutation surface that must be shown incapable of violating new invariants | Blocked as a complete authority boundary under current graph |

No candidate proof/Permit API exists in these crates to compile-fail externally; the current synthetic Permit is private under Storage's `#[cfg(test)]` module and has no cross-crate surface. This is evidence of API absence, not proof of a future authority transaction. A real A/B Cargo spike is still required before the architecture gate can close.

## Verification evidence log

Append exact command, working directory, toolchain, exit code and concise output here. Separate local runs from GitHub Actions. No local PASS is recorded before actual successful execution.

### Observed runs

- `cargo +stable test -p serea-storage --lib p8_synthetic_recovery_tests --locked` — PASS, 12 tests, Rust 1.99.1, online locked dependency resolution; includes child-process/reopen cases.
- `cargo +stable fetch --locked` — PASS; all locked crates fetched, including prior missing `fs2` and `async-trait`.
- `cargo +stable test -p serea-storage --all-targets --all-features --locked --offline --quiet` — PASS on final source; 641 unit tests, 6 integration tests passed, 2 manual resource tests ignored; 89.89s. Includes changed mismatch-burn test and isolated candidate P8 Storage tests. No migration catalog changes were made.
- `cargo +1.85.0 fmt --all -- --check` — PASS after repairing the incomplete preinstalled toolchain with `rustup toolchain install 1.85.0 --profile minimal --component rustfmt --component clippy` (toolchain then reported unchanged, rustc 1.85.0).
- `cargo +1.85.0 test -p serea-storage --lib p8_synthetic_recovery_tests --locked --offline` — PASS, 12 tests.
- `cargo +1.85.0 test -p serea-storage --lib p8_synthetic_recovery_tests --locked --offline` — PASS again after adding TaskId as an explicit binding and mismatch vector; 12 tests.
- `rustc +stable /tmp/p8b_permit_move_probe.rs` — RED attack probe: fails with E0382 when a second call reuses a value after `invoke_once(self)`. The repository GREEN test is `mismatched_attempt_burns_by_value_permit_without_effect`, with zero Fake effects; successful one-shot, ACK-loss and unavailable-after-commit/no-replay regressions also pass in the 12-test module. The compile-fail probe models Rust receiver ownership and is not an external integration test of a production Permit type; no such API exists.
- An isolated Cargo external consumer depending on `serea-storage` attempted `use serea_storage::SyntheticDispatchPermit`; compilation failed with E0432 (`no SyntheticDispatchPermit in the root`). This proves no production-visible Permit symbol exists; it does not test the future A/B authority factory because no such API is implemented.
- `python3 tools/check_core_api_boundary.py` — PASS; independent external consumer rejected the removed synthetic Core registry installer with expected diagnostic.
- `cargo +stable test --workspace --all-targets --locked --offline` — all workspace unit and integration targets completed with no failures in the captured output; use final exact-head GitHub Fast/Full CI as independent status evidence.
- `cargo +stable fmt --all` — PASS (format applied); follow-up `--check` required after remaining edits.
- `python3 tools/validate_docs.py docs` — PASS, 147 Markdown files.
- `python3 tests/workspace_smoke.py` — PASS; workspace graph meets existing constraints.
- `python3 -m unittest discover -s tests -p workspace_smoke_tests.py` — PASS, 82 tests.
- `python3 tools/check_synthetic_host_boundary.py --static-only` — PASS.
- `rustc +stable /tmp/p8b_permit_move_probe.rs` — expected compile failure E0382 on second invocation after `invoke_once(self)`; this is a standalone Rust ownership probe, not an external compile test of a production Permit API (none exists).
- `python3 tools/check_core_api_boundary.py` — started but manually interrupted after it spent over 30 seconds resolving/compiling an external consumer. No result; rerun with `PATH=/workspace/.cargo/bin:$PATH` and a bounded environment if feasible.
- `cargo +stable check --workspace --all-targets --all-features --locked --offline` — PASS, 18.07s.
- `cargo +stable clippy --workspace --all-targets --all-features --locked --offline -- -D warnings` — PASS, 9.45s. A deprecated `usize::max_value` warning appears while compiling vendored serde_json dependency, but Clippy completes successfully.
- `cargo +1.85.0` verification is fmt check + focused P8 synthetic recovery tests only; full 1.85 workspace test is delegated to exact-head Native/Full CI.
