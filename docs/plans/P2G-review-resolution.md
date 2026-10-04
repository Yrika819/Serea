# P2G Review Resolution — Verified Remediation

- Report type: `receiving-code-review`
- Review chain ID: `rc-20261005-p2grecovery`
- Resolution ID: `rr-20261005-p2grecovery`
- Source report ID: `cr-20261005-p2grecovery0`
- Source report: [frozen generation 0](P2G-review-generation-0.md)
- Status: **Accepted remediation verified — terminal review PASS and final release gates PASS**

Generation 0 is immutable and validator-approved: six Major findings, one Minor finding and four Minor standalone test gaps. Three independent post-GREEN specialists completed their assigned scopes. The coordinator independently re-read the candidate paths and governing contracts; overlapping chronology candidates were merged into F3. No accepted item is deferred.

| Item | Decision | Repair boundary | Verification |
| --- | --- | --- | --- |
| F1 | Accepted | Restricted privately constructed RecoveryPass exposes only five recovery methods, no unrestricted Tx/acquisition/begin/outcome/Deref. Existing normal transaction guard provenance is unchanged. | Old acquisition-rollback/generation-reuse guard escape reproduced in Rust before restriction; retained RED. Eight compile-fail capability checks and ordinary rolled-back authority regression now GREEN. |
| F2 | Accepted | Every later real task-state/terminal transition supersedes old outcome evidence, including recovery-owned BLOCKED edges. Fingerprint exclusion is separate from authority supersession. | Recovery-block supersession fixture reproduced RED; repair now refused/classified with no receipt/result rewrite. |
| F3 | Accepted | Current completion equals authoritative release/outcome batch; receipt observation cannot postdate that batch. | Contradictory completion and observation fixtures RED before fix, GREEN after. |
| F4 | Accepted | Empty/unplanned EXECUTING or VERIFYING aggregates are attributable corruption, not eligible work or vacuous ordinary success. | Four engine F4/F5 fixtures initially failed; both F4 variants now invariant classifications with no false resumed count. |
| F5 | Accepted | Quarantine writability accounts for independent blocked-reason CHECK damage. Unwriteable known rows receive invariant-only audit, retain their raw state/reason, and do not abort unrelated task recovery. | Both READY/RECEIVED residual reason variants initially ConstraintViolation; now GREEN with raw task bytes preserved and other task processed. |
| F6 | Accepted | Storage enforces exact exhausted blocking disposition and rejects ResumeNormally when uncertain/waiting/failed/absent or exhausted leased work exists anywhere. | Contradictory public action regressions RED before predicate repair; now typed refusal/no partial mutation. |
| F7 | Accepted | Current P2 acquisition attempt/generation arithmetic and acquisition-copy expiry bounds are semantic invariants; strict renewed authoritative expiry remains valid. | Counter/expiry mismatch regressions now corruption without guessed normalization; renewal regression stays GREEN. |
| T1 | Accepted | Source/dependency guards scan engine and storage runtime closure, excluding tests/doctests and allowing only existing Store initialization Clock calls. | Extended no-provider/model/network/subprocess/scheduler/GoalLatch and no-ambient-clock guards GREEN. |
| T2 | Accepted | Forced race ordering requires its selected winner, while barrier races permit either complete serialized outcome. | Deterministic outcome-first now requires committed outcome; recovery-first requires LeaseFenced. All nine file-backed race cases GREEN. |
| T3 | Accepted | Public engine verifier-receipt stale aggregate repair to COMPLETED pins repair/resume/invariant counters and second-pass TerminalNoop. | Complete real verifier outcome fixture and full logical-state comparison GREEN. |
| T4 | Accepted | Verifier held/released/exact-expired exhausted authority has explicit conservative classifications. | Four additional verifier cases pin runtime preservation, no fabricated failure/result/receipt, legal blocking and second-pass identity. |

Focused post-remediation command completed:

`cargo test -p serea-task-engine --test recovery --offline`

**56 passed, 0 failed, 0 ignored**. Retained log: `tmp/p2g-remediation-engine-green.log`.

Storage remediation retained **41 recovery unit tests** and **9 new recovery capability doctests** passing on stable/Rust 1.85.0 debug/release focused runs. Stable full storage passed 360 unit + 2 integration + 37 doctests. Logs: `tmp/p2g-remediation-storage-red.log`, `tmp/p2g-remediation-storage-green.log`; engine RED: `tmp/p2g-remediation-engine-red.log`.

An agent's combined full MSRV storage invocation exceeded its 300-second bound after emitting suite results; that command is NOT claimed PASS. Exact final commands were subsequently completed independently for closure on true stable 1.98.1 and exact Rust 1.85.0; the timed-out command is not used as PASS evidence. No new feature, normal outcome journal change, schema/version/vendor change, external package/version, provider/model runtime or P2H implementation was added during remediation. The existing `rusqlite` package is reused only through a dev dependency edge.

Terminal review was bounded to F1–F7/T1–T4 remediation and their affected execution chains; generation 0 was not reopened or rewritten. This resolution is the authoritative parent input to [generation 1](P2G-review-generation-1.md), which records **R1/R2/R3 PASS**, zero remaining findings/gaps/questions and a successful parent-linked report validator.

Final true stable1.98.1 and exact Rust1.85.0 workspace gates pass **801 regular +45 doctests =846**, with zero failures/ignored. Each toolchain debug/release storage passes **362+37=399** and engine passes **103+3=106**; all nine file-backed races pass in all four modes. Fmt/check/Clippy and smoke/docs/metadata/diff gates pass. Baseline704 regular identities and all comparable doctest obligations are preserved; one obsolete recovery-absence compile-fail block becomes the positive API-presence block. Evidence and intentional scope/nonclaims are recorded in the [final closure ledger](P2G-review-and-closure.md).

Only final documentation hygiene and the already-authorized single closure commit remain; no production remediation, new feature or P2H/P3 implementation follows this terminal report automatically.
