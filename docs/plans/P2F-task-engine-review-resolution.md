# P2F-b Review Resolution — Verified Remediation

- Report type: `receiving-code-review`
- Review chain ID: `rc-20261004-p2fbcore`
- Resolution ID: `rr-20261004-p2fbcore`
- Source report ID: `cr-20261004-p2fbcore0`
- Source report: [frozen generation 0](P2F-task-engine-review-generation-0.md)
- Chain: `rc-20261004-p2fbcore`
- Status: **Accepted remediation verified — terminal review PASS and full release gates PASS**

Generation 0 is frozen and validated: four Major findings, one Minor
fault-conditioned fail-closed defect, six Minor standalone test gaps.
All three independent read-only reviews completed. Coordinator re-read every
candidate's primary/supporting path. Overlapping mapper/reason gaps were merged.
No accepted item is deferred; no scope expansion or recovery work is authorized.

| Item | Decision | Repair boundary | Verification |
| --- | --- | --- | --- |
| F1 | Accepted | Refuse retained membership that cannot legally publish READY; preserve runtime and outcome predicates | RED reproduced; three InvalidPlan refusal regressions now GREEN |
| F2 | Accepted | Checked receipt capability/key comparisons during load | RED reproduced; both receipt fields and both read paths now refuse CorruptRow |
| F3 | Accepted | Validate individual stored JSON, not deeper assembled task projection | RED reproduced; depth64 known failure load/reopen now GREEN; P2F-a admission unchanged |
| F4 | Accepted | Release journal generation comes from the authority actually released | RED generation2 vs1; expired/unexpired authoritative release facts now GREEN |
| F5 | Accepted | Required PLAN delete count zero initial/one replacement | RED committed success; required delete count now refuses and restores exact rows |
| T1 | Accepted | Real mapper receipt/failure/terminal literal batch oracle | Three real mapper literal record/metadata tests GREEN |
| T2 | Accepted | Independent exact typed reason/code oracle | Independent 121-pair exact typed rule/code oracle GREEN |
| T3 | Accepted | Succeeded capability runtime/provenance through append and reopen | Succeeded prefix receipt/result/times/generation/extensions/original revision equality GREEN |
| T4 | Accepted | Complete row/ref/blob replacement-failure restoration | Audit/reference/ignored-delete failures restore full prior rows, refs and blobs; unrelated blob commits |
| T5 | Accepted | Planner negative runtime/status/root/key admission | Four focused malformed membership/key/root refusal cases GREEN |
| T6 | Accepted | Production-schema same non-null key across distinct tasks | Same-task refusal plus cross-task non-null key acceptance GREEN |

The original eight test-first engine test names/assertions remain unchanged (formatting only). Production fixes are
limited to accepted findings. Coordinator combined focused verification:
`cargo test -p serea-storage -p serea-task-engine --offline` completed with
**319 storage unit + 2 integration + 28 storage doctests + 47 engine integration
+ 3 engine compile-fail doctests = 399 executions**, no failures/ignored.
`cargo clippy --workspace --all-targets --all-features --offline -- -D warnings`
also passed. Logs: `tmp/p2fb-remediation-storage-red.log` and
`tmp/p2fb-remediation-green.log`.

The independent bounded [terminal review](P2F-task-engine-review-generation-1.md)
is PASS: every F1–F5/T1–T6 item is resolved, with zero new findings/gaps/questions.
Final stable/Rust1.85 all-target tests pass704 regular; all-feature tests pass
704 regular +36 doctests =740 executions. All624 baseline identities are retained,
116 new, zero failed/ignored. Storage passes349 and engine passes50 in every
stable/MSRV debug/release mode. Full check/Clippy/fmt and docs/smoke/metadata/diff
gates also pass. Timeout/continuation details and exact commands are recorded in
[final closure §6](P2F-task-engine-review-and-closure.md#6-final-p2f-b-runtime-closure).
Only documentation closure and the already-authorized single final commit remain;
no additional production remediation, new feature or P2G work is authorized.
