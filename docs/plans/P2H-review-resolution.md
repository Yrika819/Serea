# P2H Review Resolution — Accepted Findings and Verification

- Report type: `receiving-code-review`
- Review chain ID: `rc-20261006-p2hcrash`
- Resolution ID: `rr-20261006-p2hcrash`
- Source report ID: `cr-20261006-p2hcrash0`
- Source report: [frozen generation 0](P2H-review-generation-0.md)
- Status: **Generation-0 findings remediated; terminal generation-1 reviews PASS; final full-workspace validation PASS**

Generation 0 recorded F1–F5 and T1–T4. All accepted items were fixed in the authorized P2H scope. A later independent durability review identified one further N6 assertion gap (nonterminal aggregate and result digest association). That was also fixed before terminal re-review; it is recorded here rather than retroactively added to the frozen generation-0 report.

## Finding and gap dispositions

| Item | Decision | Remediation | Current evidence |
| --- | --- | --- | --- |
| F1 | Accepted / fixed | Per-mode durable assertions run in the fresh verifier process, not only in the supervising process. N4/N5/N6 row checks cover task/step/result/ref/blob/receipt/journal/lease state as applicable. | `crates/serea-task-engine/tests/crash.rs::verify_durable`; latest focused stable and Rust 1.85 runs pass 21/21. |
| F2 | Accepted / fixed | N7 now states clearly it cannot deterministically kill inside COMMIT, confirms child SIGKILL, runs fresh-process integrity checks, records variable counts and pins neither exact counts nor both categories. | N7 comments/assertions in `crash.rs`; repeated focused runs pass. |
| F3 | Accepted / fixed | Added bounded `wait_with_output_bounded`; a missing acknowledgement kills/reaps a running child. N7 progress failure also kills/reaps before panic. Verifier and F25/F26 probe waits are bounded. | Current harness source; stable/MSRV focused tests pass. No sleep is an authority. |
| F4 | Accepted / fixed | Reworked release script to build A default storage, B feature positive-control, C engine all-target test build and D default workspace release in a shared target tree. C consumes Cargo JSON to inspect the exact crash integration-test executable; D consumes Cargo-reported production library artifacts. | `bash tools/prove_release_fault_exclusion.sh /tmp/p2h-relproof-final`: exit 0; exact crash test executable contains seam; D identifies two storage and two engine artifacts, all clean. |
| F5 | Accepted / fixed | Added `#![forbid(unsafe_code)]` to storage crate root. | Stable/MSRV compile and clippy suites pass. |
| T1 | Accepted / fixed | N4/N5 verifier now checks the expected result blob absence; N6 checks blob presence and references. | Latest focused crash test pass. |
| T2 | Accepted / fixed | Capture non-`RECOVERY_DECISION` journal count before recovery and compare after recovery. | `n6b_recovery_after_a_nonterminal_post_commit_crash_re_effects_nothing` passes stable/MSRV. |
| T3 | Accepted / fixed | F26 canonicalizes the temp root, passes the parent's exact absolute fixture directory, and creates a unique child working directory underneath the fixture so parent cwd cannot accidentally equal it. | F26 passes stable/MSRV; child records the exact opened DB path and verifier reopens it. |
| T4 | Accepted / fixed | N7 uses a grouped per-task `HAVING count(...) <> 1` query; missing and duplicate audit rows cannot cancel in a total count. | N7 passes latest stable/MSRV focused runs. |
| G1 (generation-1 candidate) | Accepted / fixed | N6 fresh verifier now pins nonterminal task state `VERIFYING` and joins the succeeded result digest to the RESULT reference and blob on digest and data-class rank. Terminal N6 fixture asserts exactly two steps, both succeeded, and checks both through the same join. | Added after terminal review candidate; latest focused stable/MSRV test pass. Independent reviewer confirmed the nonterminal/terminal joins and exact two-step terminal cardinality statically. |

## Review C findings on current state

- Bounded timeout / kill-and-reap paths: verified; no remaining finding. The kill/wait system calls themselves are not fault-injected.
- N7 mid-COMMIT limitation: remains explicitly stress-only; no deterministic commit-internal claim.
- F26 cwd and path: addressed as above.
- Release proof: the current script proves only the normal default production configurations it explicitly builds. Its positive-control B intentionally demonstrates the opt-in feature build, and C proves the integration-test executable is seam-bearing. The Cargo feature remains explicitly named test-only and is enabled via the engine dev-dependency for tests; the script does not claim that an explicit downstream production command opting into this feature would be seam-free. Normal/default storage and workspace release artifacts are clean. This limitation is not represented as a claim about every arbitrary Cargo feature combination.

## Validation performed after fixes

- `cargo test -p serea-task-engine --test crash --offline`: **21 passed, 0 failed, 0 ignored**, repeated; latest run after N6 additions passed.
- `cargo +1.85.0 test -p serea-task-engine --test crash --offline`: **21 passed, 0 failed, 0 ignored**, latest run after N6 additions passed.
- Final stable and Rust 1.85 workspace `--all-targets` and `--all-features` runs: **822 regular + 45 doctests = 867**, zero failures, zero ignored. Both toolchains' check, all-target tests, all-features tests and Clippy `-D warnings` passed after the final N6 assertions.
- Stable and Rust 1.85 storage/engine all-feature debug and release tests passed after the final N6 assertions.
- Final stable and Rust 1.85 crash suite: 21/21 passed twice per toolchain after all source changes; N7 remains stress-only.
- `bash tools/prove_release_fault_exclusion.sh /tmp/p2h-relproof-final`: completed exit 0 after the script was revised to select exact Cargo JSON artifacts. Default storage and workspace production artifacts contain no seam; C's exact crash test executable contains it. A first clean isolated build exceeded a 10-minute tool bound; it is not counted as a pass. The cache-preserving rerun completed successfully.
- Final `cargo fmt --all -- --check`, `git diff --check`, `python3 tests/workspace_smoke.py`, `python3 -m unittest discover -s tests -p workspace_smoke_tests.py` (59 passed), `python3 tools/validate_docs.py docs`, offline Cargo metadata and `bash -n tools/prove_release_fault_exclusion.sh`: all passed.

## Remaining review lineage

- Initial read-only reviews A/B/C were completed. The exact raw A/B transcript artifacts were not persisted before compaction; their accepted findings are summarized in generation 0. This evidence limitation is explicit.
- Current-state generation-1 bounded security review: PASS, static. It notes that an explicitly enabled optional feature build is intentionally distinct from normal/default production configuration.
- Current-state generation-1 bounded durability review found G1; remediation above is complete and received a follow-up source-level PASS for the nonterminal N6 assertion. The terminal result-association check was also added conservatively. Final terminal review must cover it.
- Generation-1 whole-scope terminal review is PASS with no unresolved findings or test gaps in the default-build contract. Its explicit-feature scope caveat is retained.
