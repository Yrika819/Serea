# ExecPlan: Post-Gate-C review — 2026-10-16

## Goal and boundaries

Prepare the P8A D01–D07 contract for Owner review, validate the P7/P8 migration reservation without applying a canonical v9, re-review P7 Gate C remediation evidence, fix only defects against already Accepted invariants, and perform a hostile final audit. Do not modify Accepted ADRs or protocols, add canonical P8 migrations/provider invocation, enable production Memory/effects, or access real credentials, accounts, or personal data.

## Baseline and workspace

- Repository: `Yrika819/Serea`; requested baseline `e0acb0254820c381dd97828824bef86a1db5176c`.
- Initial local `work`: clean, HEAD `210f8aba897858ff959f046b44a18c0c765677e0`, remote `origin/main` initially stale at `cb580be`.
- Fetched GitHub. Current `origin/main` is the requested baseline exactly. Created `codex/post-gate-c-review` from it; no pre-existing changes were carried or rewritten.
- No repository `AGENTS.md` found. `docs/git-safety.md` and CI workflows inspected.
- GitHub API: PR #14 is OPEN, DRAFT, UNMERGED (P5 idempotent activation fix); PR #15 is MERGED at `e0acb025...` (disabled synthetic Gate C). CLI listing disagreed with the #14 API response; use the fetched PR endpoint as current evidence and recheck before final report.
- Latest main CI reported by `gh run list`: Fast `38887469390`, Full `38887469433`, Native platform validation `38887469455`, SQLite portability `38887469452`, all completed success on merge commit `e0acb025...`. Combined commit-status API returned no legacy statuses; workflow run checks are the evidence. Full job details confirmed Windows Gate C, Linux MSRV 1.85, Linux stable full and macOS Intel/ARM64 all succeeded; Native and SQLite jobs also succeeded.

## Workstreams and completion conditions

1. **P8A contract review.** Inspect P8A D01–D07 against accepted ADRs/protocols and live code. Produce traceable contract review material with implementation facts, accepted requirements, proposals explicitly labeled `PROPOSED / NOT RATIFIED`, alternatives, risks, Owner decisions, and executable test gates. Stop before production P8 implementation or protocol/ADR edits.
2. **Migration collision.** Confirm ordinary schema v8, test-only P7 synthetic v9 and ADR-0043 canonical P7 0009 reservation. Compare A (P7 0009 then P8 0010+) and B (Owner ADR amendment, P8 0009 then P7 later). Use only disposable synthetic DB/fixture SQL. Record that no real canonical v9 was applied.
3. **P7 Gate C review.** Re-run available Gate C IR-01–04 tests and inspect code; distinguish historical RED, current GREEN and residual risk. Inspect rollback freshness, key custody, OS auth, ledger rollover, real filesystem fault limits, confidentiality. Do not claim independent review.
4. **Approved-invariant remediation.** Only if a concrete defect is found against accepted contracts, reproduce first, minimize fix, rerun focused, adjacent and workspace checks, then conduct hostile review. Otherwise keep code unchanged.
5. **Synthesis and persistence.** Update this plan with actual commands/results, completed work, unresolved risks, Owner decisions and next autonomous steps. Commit on this dedicated branch; create a Draft PR only if available and safe.

## Evidence log

### Initial read-only inspection

- 2026-10-16: fetched remote main and PR metadata; see baseline above.
- `docs/plans/P8A-owner-decision-package.md` says PREIMPLEMENTATION/PROPOSED, no D01–D07 ratified. It documents current TaskEngine/P6/Storage boundaries, Option B from ADR-0039, AMBIGUOUS conflict between Capability Protocol §8.1 and ADR-0036, cancelled late-result incompatibility with current P2 path, credential-store not production-ready, and fake-provider tests as future P8 work.
- `ADR-0043` reserves canonical version 0009 for reviewed P7 production schema; test-only synthetic v9 stays outside the normal catalog. Current migration catalog has 0001–0008 normally and conditionally appends fixture v9 only for `p7-experimental` tests.
- `ADR-0042/0043` leave OS authentication adapters, production key custody, rollback trust anchor, real filesystem crash/power-loss, confidentiality, independent security review and Gate D activation unapproved.
- Stable toolchain available: `rustc 1.99.0`; exact `rustc +1.85.0` is also installed. Initial PATH omitted `/workspace/.cargo/bin`, so commands explicitly set `PATH`, `CARGO_HOME` and `RUSTUP_HOME`.
- PR #14's current checks from `gh pr checks 14`: Linux MSRV 1.85.0, Linux stable full, Linux Fast and macOS Intel↔ARM64 SQLite production/open checks all PASS on PR #14's head. Main Full CI `38887469433` PASS on exact baseline with Windows Gate C, Linux MSRV 1.85, Linux stable full and macOS Intel/ARM64 jobs. Main Native `38887469455` and SQLite portability `38887469452` PASS; latest Fast `38887469390` PASS.
- P7 feature-focused tests on local stable 1.99: `cargo test -p serea-storage --features p7-experimental,p7-gate-c p7_ir -- --test-threads=1` — 11 passed; `... ir04_ ...` — 6 passed. All synthetic.
- Disposable Python SQLite 3.53.1 experiment applied exact SQL 0001–0008 followed by fixture `p7_experimental_memory_v9.sql`; `PRAGMA foreign_keys=ON`, `quick_check=[('ok',)]`, `foreign_key_check=[]`. Temp DB and WAL/SHM files removed. This is candidate SQL feasibility only, not canonical migration approval.
- `cargo fmt --all -- --check` PASS; `python3 tools/validate_docs.py docs` PASS (139 Markdown files); Core API boundary, Synthetic Host boundary, workspace smoke and 82 `workspace_smoke_tests.py` tests PASS.
- First `cargo test --workspace --all-targets --locked` attempt failed only in `model_execution_is_confined_to_explicit_synthetic_host_feature`: the test's nested `cargo tree --offline` could not find uncached optional dependency `cfg_aliases v0.2.2`. `cargo fetch --locked` fetched locked dependencies; isolated rerun passed, then a full offline all-target rerun completed with all observed targets/tests passing.
- Workspace all-features rerun completed PASS. No production or canonical migration/test fixture change. The only doc change to the existing P8 package adds an explicit **PROPOSED / NOT RATIFIED** umbrella; the review record contains detailed RED/GREEN/residual evidence and attack review.

## Current status

Workstreams 1–5 are complete. Only normal commit and dedicated Draft PR publication remain. No source code or Accepted ADR/Protocol was changed; only review documentation was added/clarified.

### Validation and adversarial closeout

- `cargo test --workspace --all-targets --locked --offline`: PASS after a first attempt exposed an uncached optional dependency in one nested `cargo tree --offline` boundary test. `cargo fetch --locked` populated the cache; the affected test passed alone and the full Workspace rerun passed without test/code changes.
- `cargo test --workspace --all-features --locked --offline`: PASS, including P7 Gate C synthetic ledger, capacity, GC, recovery, SQLite migration/foreign-key and doctest suites. This is synthetic development evidence only.
- Focused P7 IR selected tests: 11/11 for `p7_ir` and 6/6 for `ir04_`, separately from the full feature suite.
- `cargo clippy --workspace --all-targets --all-features --locked --offline -- -D warnings`: PASS. Compiler emitted one deprecated `usize::max_value` warning from vendored `serde_json`; Clippy exited successfully and the vendored file was not changed.
- `cargo fmt --all -- --check`, `git diff --check`, `python3 tools/validate_docs.py docs` (139 Markdown files), Core API boundary, Synthetic Host boundary, workspace smoke and 82 workspace smoke regression tests: PASS.
- Stable `cargo check --workspace --all-targets --all-features --locked --offline`: PASS. Exact Rust 1.85.0 `cargo check --workspace --all-targets --all-features --locked --offline`: PASS. Local MSRV all-feature tests/Clippy are **NOT EXECUTED**; GitHub Full CI on the exact baseline ran Linux Rust 1.85 workspace check/tests/Clippy successfully.
- Native platform and cross-architecture SQLite portability: **NOT EXECUTED locally**. GitHub Native (`38887469455`) and SQLite portability (`38887469452`) succeeded on the exact baseline `e0acb025...`.
- Hostile review recorded in `Post-Gate-C-review-2026-10-16.md`: no stale-authority/provider path exists to approve yet; P8 retry/cancellation/Receipt invariants remain Owner blockers; no independent P7 security review is claimed; no code defect against an Accepted invariant was found, so no source fix/regression test was introduced.

### Publication state

- Dedicated branch `codex/post-gate-c-review` was pushed after a fresh explicit-target preflight; it contains normal commit `7472f8cc323297a49c2b52550721cb28b4420bfb`. No force push or main update was used.
- Draft PR #19 is OPEN/DRAFT, base `main` at `e0acb0254820c381dd97828824bef86a1db5176c`, head `7472f8cc323297a49c2b52550721cb28b4420bfb`: https://github.com/Yrika819/Serea/pull/19. It is blocked as expected while Draft.
- PR checks started at 2026-10-16 11:51 UTC: Fast `38896599970`, Full `38896599868`, Native `38896599790`, SQLite portability `38896599963`, Git hooks Windows `38896599679`. At last read, Fast and Git hooks were in progress; Full, Native and SQLite runs were queued. Recheck before closing this ExecPlan.
- Next autonomous work: monitor these exact PR runs, record any failure and diagnose it without relaxing gates; keep PR #19 Draft and do not merge. Owner review is still required for P8 D01–D07 and any proposed migration/protocol change.
