# Code Review Report — H4-F1B Signing Preparation

## Report Contract

- Report type: `code-review`
- Report ID: `cr-20261121-f1b-7d4a`
- Review chain ID: `rc-20261121-f1b-19c2`
- Review generation: `0`
- Review trigger: `initial`
- Parent review report ID: `None`
- Parent review report path: `None`
- Parent resolution ID: `None`
- Parent resolution path: `None`
- Generated at: `2026-11-21`
- Report path: `docs/plans/2026-11-21-code-review-h4-f1b-signing-preparation.md`
- Source skill: `code-review`
- Status: `Review complete`
- Git mutation during review: `None`
- Scope fingerprint: `sha256:687058d9029671e0fb914dbc267fbe12a7ac45dfc288bfc8fd2e43586268a3ca`

## Scope

- Review date: `2026-11-21`
- Scope kind: `file set`
- Scope description: Initial independent security and accuracy review of the F1B readiness report, with supporting source/docs and official Android guidance.
- Scope mode: `full frozen scope`
- Baseline: `origin/main`, `846d6693248a09656beb2d73ff693c31e474ef29`
- Target: `untracked report at working tree; SHA-256 above`
- Changed paths: `1`
- Diff size: `192 added lines / 0 deleted lines`
- Completion: `Complete within reviewed scope`
- Requirements consulted: Owner H4-F1B instruction; F1A report/runbook; F0 report; H4-E1→F gate matrix; Private Calendar checklist; current Gradle source; Android signing, apksigner, and zipalign official documentation; historical D2 report.
- Prior resolution consulted: `None`
- Assumptions: Review is static/read-only. No APK, key, device, SDK-local configuration, or Cloud state was accessed.
- Excluded as unrelated: Product code correctness and execution of unauthorized signing/build/install/Cloud actions.

## Review Orchestration

- Assessment subagent: `Coordinator assessment - one cohesive preparation report and one sequencing invariant`
- Orchestration decision: `Single reviewer`
- Decision confidence: `high`
- Decision rationale: One report with a dominant approval/operation sequence; one independent security reviewer adds useful scrutiny without duplicative subsystem partitions.
- Coordinator override: `None`
- Context or tool limits: Reviewer could not verify dated command/CI evidence, private artifacts, SDK configuration, or external device state.

### Risk Dimensions

- Approval sequencing: an unsigned APK hash cannot be known until after its specifically approved build.
- Signing identity/update continuity: a signer mismatch can prevent same-package updates and risk app-private data.
- Evidence accuracy: distinguish source and historical test artifacts from Owner-installed signed APK and current CI.

### Reviewer Assignments

| Reviewer | Angle | Owned surfaces | Mandatory cross-checks | Status |
| --- | --- | --- | --- | --- |
| `R1` Terra | Security and operational sequencing | Entire F1B report; supporting Gradle, F1A/F0, D2 and gate docs | Owner A–E approvals; exact APK boundary; Cloud/device separation; Android signing guidance | Complete |

### Synthesis Statement

The coordinator independently checked the sole approval-affecting candidate against the report’s approval table and future APK procedure. It is a real sequencing defect. The separate device and Cloud gates and the report’s current stop posture remain intact. Private artifact/device state and dynamic Gradle output remain blind spots by design and were not represented as verified.

## Review Snapshot

- Recommendation: `Changes requested`
- Completion: `Complete within reviewed scope`
- Why now: Approval D requires an exact unsigned APK hash, but the procedure requires D before approval C creates that APK.
- Must-review now: `F1` — resolve unsigned build/signing approval order.
- Findings count: `Blocker 0 | Major 1 | Minor 0 | Question 0`
- Standalone test gaps: `Blocker 0 | Major 0 | Minor 0`
- Coverage confidence: `high`
- Biggest blind spot: Actual same-package APK/install/version state is Owner-controlled and unverified.

## Complete Findings Index

| ID | Severity | Surface | Review risk | Confidence | Origin | Verification | Issue key | Issue fingerprint | Expected basis |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `F1` | Major | Approval C/D and APK procedure | D is non-actionable before C has produced the exact unsigned artifact and hash | High | R1-C1 | Independently checked procedure and approval definitions | `behavior; entry=Owner approval of private APK signing; contract=Approval D applies to the exact Owner-approved unsigned artifact after C creates and hashes it; effect=signing is blocked by a circular or non-specific approval prerequisite` | `ifp-sha256:56b29e3a0ebc5dca2258afb1d43748f80e6c1d1c3f72007453f285609464bbb2` | `kind:requirement; strength:authoritative; evidence:Owner H4-F1B instruction, approvals C and D are independent and refer to specified APK operations` |

## Blocker

None.

## Major

### F1 Major — Build and signing approvals have a circular prerequisite

Impact: Owner cannot provide Approval D for the exact unsigned input/hash before Approval C has generated it.

Review reason: This can block the specified workflow or encourage premature, non-specific signing approval, contrary to independent approval boundaries.

Surface: F1B report, APK operation procedure and Approval C/D handoff.

Issue key: `behavior; entry=Owner approval of private APK signing; contract=Approval D applies to the exact Owner-approved unsigned artifact after C creates and hashes it; effect=signing is blocked by a circular or non-specific approval prerequisite`

Issue fingerprint: `ifp-sha256:56b29e3a0ebc5dca2258afb1d43748f80e6c1d1c3f72007453f285609464bbb2`

Expected basis: `kind:requirement; strength:authoritative; evidence:Owner H4-F1B instruction, approvals C and D are independent and refer to specified APK operations`

Confidence: High

Origin: `R1-C1`

Coordinator verification: Confirmed the procedure requires both C and D before the build while D requires the exact unsigned artifact/hash. This evidence is sufficient to establish the ordering contradiction without building an APK.

Look here first:
- [`H4-F1B report — Future Owner-only APK procedure`](h4-f1b-private-signing-identity-apk-readiness.md#L85)
- [`H4-F1B report — Approval gates`](h4-f1b-private-signing-identity-apk-readiness.md#L137)

Failure mode:
- Expected: C authorizes production of the specified unsigned artifact; after the Owner records its hash/provenance, D separately authorizes signing that exact artifact.
- Current: Both C and D are required before build; D depends on an unsigned artifact hash unavailable until after C’s build.

Evidence:
- The report’s Section 6 opening requires approvals C and D before the build and later says D applies to the exact unsigned input/hash.
- The Owner instruction explicitly separates Approval C (unsigned APK generation) from Approval D (signing and certificate verification).

Assumptions and limits:
- Assumes D must identify the exact unsigned APK, as stated by the Owner’s signing operation boundary. No signing/build action was taken.

Reviewer action: `request fix`

## Minor

None.

## Questions

None.

## Test Gaps

None; this documentation-only sequencing issue is verified by direct contract comparison, not a missing product test.

## Review Coverage Ledger

| Area ID | Area / path | Touched files or entry points | Owner | Depth | Status | Result | Evidence / next step |
| --- | --- | --- | --- | --- | --- | --- | --- |
| `A1` | Existing APK/install and version history | F1B report Sections 1–2, 5, 7 | R1 | Contract trace | Reviewed - no issue found | Correctly `NOT_VERIFIED`; no private artifact/device search claimed. |
| `A2` | Option A crypto and custody | F1B report Section 3 | R1 | Contract trace | Reviewed - no issue found | RSA/SHA-256 proposal and 25-year guidance align with current Android docs; Owner approval remains pending. |
| `A3` | Gradle unsigned/CI boundary and historical artifacts | F1B report Section 4 | R1 | Dependency trace | Reviewed - no issue found | Source guard and CI restriction accurately described; dynamic task graph appropriately marked unavailable. |
| `A4` | APK alignment/sign/verify procedure | F1B report Section 6 | R1 | Contract trace | Finding F1 | Sequence must be C → artifact/hash → D → sign; no alignment/signing should precede D. |
| `A5` | Cloud and device gates | F1B report Sections 7–9 | R1 | Contract trace | Reviewed - no issue found | Cloud remains unperformed; device approval E remains separate. |
| `A6` | Historical evidence and claims | F1B report Sections 2 and 10 | R1 | Evidence trace | Reviewed - no issue found | Test-generated version 42 distinguished from approved/installed version. |

## Subagent Candidate Adjudication

| Candidate ID | Proposed by | Decision | Final ID | Coordinator evidence | Reason |
| --- | --- | --- | --- | --- | --- |
| `R1-C1` | R1 | Accepted | F1 | Re-read APK procedure and C/D definitions | Exact hash prerequisite follows the build, but text puts D before build. |
| `R1-C2` | R1 | Dismissed | None | Target report explicitly marks versionCode 2 pending and Cloud/device unauthorized | No evidence the report adopted source default or authorized Cloud/device operations. |
| `R1-C3` | R1 | Dismissed | None | Target records SDK absence and no task graph | It does not claim dynamic graph/output-path verification. |

## Evidence Appendix

### Diff Inventory

| File or area | Classification | Semantic review area considered |
| --- | --- | --- |
| `docs/plans/h4-f1b-private-signing-identity-apk-readiness.md` | docs-only | Signing authority, APK provenance/version, update compatibility, Cloud/device approval and operational sequence |

### Verification Commands

- Read-only comparison of F1B report with `android/app/build.gradle.kts` and F0/F1A/H4-E1 documents -> static contract review; no APK task executed.
- Official Android app signing, `apksigner`, and `zipalign` references -> guidance consistency checked.
- `python3 tools/validate_docs.py` -> 241 Markdown documents valid at review time; this is cross-reference hygiene, not signing proof.

### Supporting Code Links

| ID | Role | Link | Why it matters |
| --- | --- | --- | --- |
| `F1` | entry | [`F1B report Section 6`](h4-f1b-private-signing-identity-apk-readiness.md#L85) | Approval preconditions for build/sign. |
| `F1` | risk | [`F1B report Approval table`](h4-f1b-private-signing-identity-apk-readiness.md#L137) | D requires the exact unsigned artifact/hash. |

### Dismissed Coordinator Candidates

| Candidate | Decision | Evidence |
| --- | --- | --- |
| Source default version 2 is proposed for formal use | Dismissed | Report marks first official version `PENDING_OWNER` and says dry-run literal 2 is not a recommendation. |
| H4-F1B authorizes device installation or Google Cloud | Dismissed | Report explicitly leaves E and all Cloud actions unapproved. |
| Gradle package task graph was verified | Dismissed | Report says SDK location prevented task graph resolution. |

### Blind Spots

| Area ID | Blind spot | Decision risk | What would resolve it |
| --- | --- | --- | --- |
| `A1` | Existing signed APK/install state is not known | Could make a newly generated signer incompatible and risk data continuity | Owner supplies non-secret prior-artifact/install determination; any artifact/device inspection requires separate approval. |
| `A3` | Dynamic Gradle task graph/output path unavailable | Exact output selection cannot be promised | Separately approved build setup and unsigned APK generation, with task output/provenance recorded. |

## Prior Resolution Reconciliation

None - initial review generation.

## Receiving Handoff

- Handoff status: `Ready for receiving-code-review`
- Automatic receiving permitted: `Yes` — documentation change is authorized by the Owner; no key/APK/device/Cloud action is authorized.
- Source report ID: `cr-20261121-f1b-7d4a`
- Scope fingerprint to recheck: `sha256:687058d9029671e0fb914dbc267fbe12a7ac45dfc288bfc8fd2e43586268a3ca`
- Actionable finding IDs: `F1`
- Deferred finding IDs: `None`
- Actionable test-gap IDs: `None`
- Deferred test-gap IDs: `None`
- Open question IDs: `None`
- Open coverage area IDs: `None`
- Highest-risk verification to repeat: Review the revised C → artifact/hash → D order and confirm E/Cloud boundaries remain independent.
- Suggested implementation boundaries: `docs/plans/h4-f1b-private-signing-identity-apk-readiness.md` only.
- Re-review note: `Treat every finding as a claim to verify. Challenges require a counterclaim, argument, evidence, limits, and settlement criterion.`
- Chain rule: `Generation 1 is terminal. Do not automatically invoke receiving-code-review; return remaining findings to the user or product owner.`

## Report Self-Check

- Yes — actual assessment mode and rationale recorded.
- Yes — all review-relevant areas appear in the coverage ledger.
- Yes — F1 appears in index and matching card.
- Yes — no test gap is asserted for this documentation sequence issue.
- Yes — issue fingerprint and authoritative expected basis recorded.
- Yes — generation 0 and handoff are consistent.
- Yes — meaningful subagent candidates adjudicated.
- Yes — blind spots include resolution steps.
- Yes — recommendation maps Major to Changes requested.
- No — repository does not contain `scripts/validate_review_report.py`; report was checked against the required template manually.
- Yes — no git mutation occurred during review.
