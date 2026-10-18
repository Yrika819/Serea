# P8B post-PR #36 stabilization campaign report

**Campaign source:** PR #36 HEAD `bfe47522d808388f82b55c67793184726273600d`.
**Final campaign HEAD:** recorded in the final commit/PR metadata; this document
is updated before publication. **Native result:**
`NATIVE_INSTRUMENTATION_READY_UNVERIFIED` and
`ROOT_CAUSE_UNCONFIRMED`. **PR audit:**
`STACK_INTEGRATION_DECISION_READY`. **Candidate C:**
`C_AUTHORITY_CONTRACT_DECISION_READY`. No production authority was enabled.

## Git and scope

- Repository `Yrika819/Serea`; isolated clone `/workspace/Serea-P8B-2026-10-18`;
  campaign worktree `/workspace/Serea-P8B-worktree`.
- New branch begins at exact PR #36 HEAD `bfe47522d808388f82b55c67793184726273600d`.
- PR #36 is Draft/Open, head branch `codex/p8b-overnight-2026-10-18`, base
  branch `codex/p8b-l1b-boundary-repair-2026-10-17`, base head
  `b9a0ef070bbec396f107ccb2b5a43925b0e22ed0`.
- `main` verified at `e0acb0254820c381dd97828824bef86a1db5176c` before and after
  the local scratch rehearsal. The original `/workspace/Serea` clone remained
  on branch `work`; no main, existing PR, remote branch, review, or protection
  setting was changed. The rehearsal worktree was local-only and uncommitted.
- Changed content is documentation, a bounded Windows diagnostic script, and
  the PR manifest. No Rust production code, public authorization API, schema,
  migration, ADR status, architecture version, or CI workflow changed.

## Track N — Windows ARM64

| Evidence | Result |
|---|---|
| PR #34 Native run `39036558000`, Stable ARM64 job `117407503289`, head `f3804a643e6334bc4f1dddfea1a0c096c1fd7a13` | All-feature tests failed in `model_accounting_is_bound_to_a_task_created_by_the_scheduler_task_engine`; child `0xC0000142`, exit decimal `3221225794` / signed `-1073741502`, 0 stdout and 0 stderr; failure at loop index 0. |
| PR #36 Native run `39054682137`, Stable ARM64 job `117460408311`, head `bfe47522d808388f82b55c67793184726273600d` | Same test, status and empty streams; failure at loop index 9. |
| Controls | PR #33 Stable ARM64 job `117389941104` passed all 12 Host lifecycle tests; PR #35 Stable ARM64 job `117431513197` passed. PR #34/#36 Rust 1.85.0 ARM64 jobs `117407503223` / `117460408219` passed; #36 Stable x64 job `117460408021` passed. |
| Runner evidence | GitHub-hosted `windows-11-arm` was ephemeral. No live shell, WER/Application Error module, ETW image-load, loader event, or resource sample was available. |

`Command::output()` returned a child exit status, so the process launch handle
was created; the child failed before normal synthetic Host output. The provider
is in-process. Failure movement from child 0 to child 9 fits several possible
transient startup/resource conditions but proves none. Stable ARM64 parallel
all-features is the only failing observed configuration. The dedicated
`tools/windows_arm64_model_smoke_diagnostic.ps1` gives a bounded synthetic
reproduction path, but it was not executed or syntax-checked on Windows.
Therefore root cause is unconfirmed; no retry, skip, delay, test exclusion, or
Native job weakening was used.

## Track I — Draft stack

The live Open PR census contained all 19 target PRs: #14 and #19–#36. Exact
head/base names and SHAs, merge-base, commit range, changed paths, visible
checks and classification are in [P8B-STACK-MANIFEST.json](P8B-STACK-MANIFEST.json).
Summary:

- #14 is independent P5 work, but local no-commit merge onto current main
  produced an identical tree; treat as redundant pending Owner verification.
- #19 is review research; #20 is direction-only ADR/docs candidate; #21
  requires schema redesign/approval; #22–27 are research/Synthetic prototypes.
- #28 is a candidate for a clean extraction of ADR-0045 directional text and
  test evidence. #29 is blocked on Owner decision; ADR-0046 stays Proposed.
- #30 is fully contained in #31's history and overlapping paths; do not apply
  it separately. #31–36 are research/test evidence; #34 and #36 retain Native
  ARM64 failures.
- Candidate migrations 0009/0010 remain isolated, outside the normal catalog.
  No schema is promoted as canonical P7 and no prototype is called production
  authority.

Integration ordering and the local-only rehearsal are detailed in
[P8B-INTEGRATION-PLAN.md](P8B-INTEGRATION-PLAN.md). No main merge or PR action
was performed.

## Track C — Candidate C

[P8B-CANDIDATE-C-CONTRACT.md](P8B-CANDIDATE-C-CONTRACT.md) maps Host-private
coordinator → TaskEngine revalidation → Storage atomic transaction; it
documents the public lease/time, accept-all verifier, caller-composed Policy,
Grant ID/member, and ProviderId/default-Ready substitution paths. The full-set
Grant probe remains test-only. The Rust API boundary probe was run from this
exact source baseline: 2 external positive compositions compile and 7 negative
probes match expected privacy/absent-API diagnostics. It demonstrates API
reachability only, not authentication or production authorization.

[P8B-OWNER-DECISION-PACKET.md](P8B-OWNER-DECISION-PACKET.md) presents five
explicit decisions: topology, Worker admission/API compatibility, clock and
expiry, Owner identity/challenge, and Grant/Provider/effect rules. No choice was
implicitly accepted. ADR-0046 remains PROPOSED / NOT RATIFIED.

## Verification

Environment installed for this campaign: Rust stable `1.99.1 (58cf8f9c3
2026-10-13)` and exact MSRV `1.85.0 (4d91de4e4 2025-02-17)`. All Cargo commands
used `--locked`; Cargo target cache was isolated at `/workspace/Serea-target`.

| Check | Stable | Rust 1.85.0 |
|---|---|---|
| `fmt --all -- --check` | PASS | PASS |
| workspace `check --all-targets --all-features` | PASS | PASS |
| workspace `clippy --all-targets --all-features -- -D warnings` | PASS | PASS |
| workspace `test --all-targets --all-features` | PASS; Storage unit suite 647 passed; Host lifecycle 12 passed | PASS; Storage unit suite 647 passed; Host lifecycle 12 passed |
| `serea-host --release --no-default-features` | PASS | Not required by campaign command set |
| `tools/p8b_authority_cargo_probe/run.py` | PASS: 2 positive, 7 expected negative probes | PASS: probe compiles were performed by script per its matrix |

`python3 tools/validate_docs.py docs` passed (164 Markdown files scanned at the
time of the run). `git diff --check` passed before final commit. Rust builds
reported one pre-existing deprecation warning in vendored `serde_json` under
Rust 1.99.1; workspace Clippy with `-D warnings` still passed for both
toolchains. No Windows ARM64 test was run locally.

## GitHub Actions and publication

The baseline PR #36 exact-head runs were Fast CI `39054682165` (success), Full
CI `39054682171` (success), Native `39054682137` (failure: 11/12 jobs, Stable
Windows ARM64 only), SQLite portability `39054682309` (success), and Git Hooks
`39054682192` (success). All correspond to `bfe47522d808388f82b55c67793184726273600d`.

The final campaign HEAD and its exact-head workflow/job IDs are filled after
publication. A Draft PR is created only if all committed artifacts remain
substantive; its base is the actual PR #36 head branch
`codex/p8b-overnight-2026-10-18`. No more than one new PR is in scope.

## Next restart point

1. Owner reviews the five Candidate C decisions and explicitly selects the
   trust process, Worker/session model, time source, Owner authentication source
   and Grant/Provider rules.
2. In a new isolated branch, make a bounded, test-only Host composition proof
   for one admitted Worker and one pinned fake Provider. Add RED/GREEN negative
   cases only at private/test seams. Keep effects disabled.
3. Run the isolated Stable/MSRV suite and external Cargo positive/negative
   probes. Split migration work from authority work and keep 0009/0010 outside
   the canonical catalog until separately approved.
4. Continue Native diagnosis with a bounded Windows ARM64 run capturing only
   time-scoped resource and loader evidence. Keep failures fatal.

Still prohibited: ADR-0046 ratification, L2 Intent writer, L3 Permit factory,
Production Provider, Credentials, Memory, Receipt, Retry, canonical migration,
external effect, and production authorization.
