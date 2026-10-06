# P2I Whole-P2 Review Summary — Security / Authority / Regression / Claims

- **Independent reviewer ID:** `cr-20261006-p2i-security-14017e3` (continued recheck on P2I working tree)
- **Scope:** P2A `10622803fc52e7ed8e91de5ce41f472b09793766` inclusive through P2H `14017e3981cf23ac2b4a076aad435f16eb59b535`; additional P2I delta on branch `p2/p2i-final-closure`.
- **Review mode:** independent read-only review; initial inventory included nine implementation commits, 242 changed paths and 76,736 additions / 2,257 deletions.
- **Raw report:** reviewer results were returned inline to the coordinator, not persisted as a canonical code-review report. This file is an attributed coordinator summary, not a substitute for the full response or a review-validator report.
- **Final recommendation:** **Discuss — incomplete whole-P2 coverage. Not terminal PASS.**
- **Verified security/authority Blocker/Major:** none in the inspected paths. This is not a certification of uncovered surfaces.

## Findings and dispositions

| Candidate | Final disposition | Evidence / limitation |
|---|---|---|
| Fault-feature exclusion wording | Fixed to distinguish normal/default production builds from explicitly feature-enabled seam-bearing builds. | `Cargo.toml`, storage crate root and `fault.rs` now state the same configuration boundary. No universal arbitrary-feature exclusion is claimed. |
| Stale P2H precommit status | Fixed. | P2H closure ledger records commit `14017e3981cf23ac2b4a076aad435f16eb59b535` and P2I in progress. |
| Complete/replayable audit-history overclaim | Fixed by narrowing ADR-0021 wording. | It now scopes retained journal history to supported audited task-engine operations and excludes low-level lease-only mutation and deletion-after-cascade. |
| Default-feature production exclusion architecture question | Not retained as a current blocker. | User-authorized test/dev-oriented feature is only requested by the engine dev-dependency in this workspace; explicit feature opt-in remains seam-bearing and disclosed. This is not architecture ratification. |
| LeaseGuard database-domain question | Not established as a P2 defect. | No frozen multi-database isolation contract or attacker-accessible bypass was proved. No database-identity isolation guarantee is claimed. |
| ADR-0021 unchanged-P2 P3 promise | Future design obligation, not current P2 blocker. | ADR-0021 remains Proposed; P3 event sequence/participant coverage and E3/E4 remain outstanding. |
| Missing P2-closure link | Fixed in current tree. | `docs/plans/P2-closure.md` exists, is explicitly OPEN, and docs validation passes. |

## Security and nonclaim assessment

The reviewed claims preserve these distinctions:

- Default production configuration excludes the fault seam; an explicitly feature-enabled artifact contains it.
- P2H N6 proves process death after successful SQLite COMMIT but before caller success publication, not provider acknowledgement or exactly-once external effects.
- N7 proves only stress integrity/FK invariants; no sample is represented as a deterministic in-COMMIT kill.
- PRIVATE blob protection wiring is not a real encryption backend, key-custody proof, or complete ordinary-row PRIVATE representation.
- No SECRET sealed store, CREDENTIAL store, event runtime, E3/E4, event backfill, scheduler/provider execution, or ADR-0021/22/24 ratification is claimed.
- P2 journal coverage is not a complete retained audit trail for every low-level authority operation or deletion after cascade.

## Coverage and remaining limits

The reviewer inspected the P2I delta, current indexes/ADR-0021/P2H closure status, fault cfg boundary, and relevant N6/N7/nonclaim wording. It still explicitly left these surfaces incomplete:

- **A1:** exhaustive line-by-line inspection of all P2A–P2H history and intermediate revisions.
- **A8:** not every recovery test/state combination and authority interleaving was independently inspected.
- **A11:** not every historical phase record, raw review transcript, and evidence lineage was audited.
- **A12:** `jsonschema-value` upstream archive checksum and complete file-delta provenance remain independently unverified; no complete dependency vulnerability review was performed.
- **A9:** this reviewer did not complete a full release-exclusion artifact proof; previous bounded attempts timed out during compilation.
- **Platform/durability:** no fresh cross-platform, Apple Silicon, power-loss, custom-VFS or fsync-failure validation.

The two scoped P2H reviews and recorded validation are useful evidence but do not discharge those whole-P2 coverage gaps.

## Validation actually run by this reviewer

At final recheck: docs validator (73 Markdown files), Python smoke tests (74), workspace smoke, `git diff --check`, and commit/parent check passed. Rust suites, crash tests, vendor suites and release artifact proof were not rerun at this final recheck. Earlier timeouts are recorded as timeouts, not passes. No repository edits, staging or commits were performed by the reviewer.
