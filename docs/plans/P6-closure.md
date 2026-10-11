# P6 final closure and integration review

**Status: `P6_VERIFIED_UNMERGED_READY_FOR_OWNER_REVIEW`**  
This records verification of two unmerged branches only. It does not claim main integration, release, or P8 completion.

## Verified baseline

| Ref | Expected and verified SHA | Current state |
|---|---|---|
| `main` | `e17fbfc7f3e75f0bc95a66d693fe11b57a6ced50` | unchanged during audit; ancestor of both P6 branches |
| `p6/preimplementation-audit` (PR #6) | `1c04451469a810a484e92fa7f0e5eb249451a54e` | OPEN / DRAFT / UNMERGED; base `main` |
| `p6/policy-approval-runtime-privacy` (PR #10) | `09e2cc3d272bd445e13613310e238497657e1ff9` | OPEN / DRAFT / UNMERGED; base P6A branch |

The worktree was clean before this closure edit. Remote branches were fetched; no newer branch work was present. P6A is an ancestor of runtime HEAD; `main` is an ancestor of both. The merge base of P6A and runtime is exactly P6A HEAD. PR #6's head equals its expected SHA and PR #10's head equals its expected SHA. Neither PR is merged. Their status and bases were not changed during review.

The commit history uses approved noreply identities for the runtime verification commits. P6A commits are attributed to `yrika <143304522+Yrika819@users.noreply.github.com>`; runtime commits are attributed to `Codex <noreply@github.com>`. The current repository guard checks approved identity formats. The earlier P6B checkpoint `3de5098e3b51c5a6bebf01b40550a6b94f4fd7e3` is an explicitly retained WIP checkpoint, followed by repair/verification commits; it is not represented as a verified milestone. There is no evidence of rewritten branch history.

## Milestones and accepted owner decisions

| Milestone | Closure / verification identity | Scope and status |
|---|---|---|
| P6A | `1c04451469a810a484e92fa7f0e5eb249451a54e` | Documentation-only preimplementation audit and owner-ratified contract package. Runtime was absent from PR #6. |
| P6B | `a5feb8373007aedf7b3418717f763c0d3c6475e3`; repair `d90641974785faacc434821ef3336c0c7c31ec22` | Durable schema, migration integrity, policy revision storage, approval authority sealing; later exact-head checks are recorded in the runtime history. |
| P6C | `da2b391021270a67838930f0945b91bb09449948` | Deterministic evaluator, code-owned defaults, trusted inputs and evidence. |
| P6D | `ef064dfdce37ac765bc47c14000fc3ec547510b4`; privacy follow-up `a54325b48af8c336b6d252fdfa2dc4c9821167db` | Authenticated response/revocation, R2 membership, expiry, atomic lifecycle, privacy corrections. |
| P6E | `798c483c98ab0adaeb48e66030d9b1673087aae7` | Option A mixed Plan orchestration, durable WAIT resolution, public Scheduler wake and fresh Policy reevaluation. Included unchanged in later exact-head verification. |
| P6F | `c211f8483b77bd4b34dd83d766b4e905ca5cf3d5`, `cbe67cac2b82e260627144e966cea13189ce17ba`, final source `09e2cc3d272bd445e13613310e238497657e1ff9` | Adversarial concurrent Policy/approval/expiry/cancellation/revocation tests and final P6E closure record. Final exact-head CI passed. |

Owner decisions A–F are ratified in the accepted package and ADRs. The retained decision points are: **A** deterministic class defaults and precedence; **B** SQLite as authority and monotonic policy revision identity; **C** exact approval/action identity; **D** authenticated response and separate authenticated grant revocation; **E** privacy, retention and bounded persisted authority; **F** crate ownership and P6/P8 seam. **DC-1–DC-4** settle grant-use identity and consumption, authenticated actor/revocation authority, privacy/retention constraints, and runtime/crate ownership as recorded in ADR-0038/0039/0040. **R2** resolves the feasibility conflict with a bounded, immutable grant over 1–8 explicitly enumerated action members, each with its own Step ID, canonical argument digest and scope. **Option A** in ADR-0041 permits an initial mixed Plan; a fully ALLOW gate completes as audited `NOT_REQUIRED`, while approval-required actions transition through a durable request/WAIT lifecycle. These decisions do not confer external execution authority.

## ADR status

- ADR-0037 — Accepted: deterministic policy semantics and revision authority.
- ADR-0038 — Accepted: approval identity, lifecycle, authenticated response and R2 subset semantics.
- ADR-0039 — Accepted: current-policy revalidation and grant-consumption boundary; P8 execution obligations remain deferred.
- ADR-0040 — Accepted: storage, retention, bounds and crate ownership.
- ADR-0041 — Accepted: Option A initial mixed approval gates and `NOT_REQUIRED` result.

Historical pre-ratification or WIP paragraphs remain in the handoff records for auditability. This closure record supersedes their stale “proposed”, “blocked”, or “not started” status where applicable; it does not rewrite those historical records.

## Architecture and crate graph

P6 adds `serea-policy` as an L2 control crate depending on protocol, storage and event-bus. `serea-task-engine` composes immutable P5 `PreparedActionV1` bindings with Policy and Storage. Scheduler routes durable lifecycle events and wakes TaskEngine. Storage owns SQLite authority and migrations. There is deliberately no `serea-capability -> serea-policy` edge, and no capability provider dispatch in P6. The runtime implementation is in PR #10; PR #6 remains the contract/documentation prerequisite and contains no runtime implementation itself.

## Migration inventory and immutability

| Version | File | Purpose | SHA-256 |
|---|---|---|---|
| 0005 | `crates/serea-storage/migrations/0005_policy_approval.sql` | Policy revisions/rules/active pointer; requests, enumerated actions, grants, members, use constraints and triggers | `cf6686807e3ceb10bd21c6653fbdd61c487e6b8938a2d78104d160920831e71a` |
| 0006 | `crates/serea-storage/migrations/0006_policy_authority_sealing.sql` | Request/grant/member/use sealing, immutable authority facts, monotonic grant state and consumption guards | `0af16a30cd2b29f046c14d7079a356dc25ce4352216edd9ad6821b168e6eb6ca` |
| 0007 | `crates/serea-storage/migrations/0007_policy_revision_sealing.sql` | Complete revision activation and no late rule insertion | `48f2f1805d59f4eec215963ac5109007294a0dfe015b9a4b4e7e69c14f9197eb` |
| 0008 | `crates/serea-storage/migrations/0008_approval_lifecycle_routing.sql` | Durable routing-only approval lifecycle wake rows | `6e1520bf22ced832ed480a1cf26476e056611f22b1f517f2324b32a8d4fbf925` |

These are current file SHA-256 digests; Storage's migration runner separately computes and verifies its canonical `Digest` checksum. Migrations 0001–0008 were not edited by this closure task. Migration 0005's digest matches the earlier P6B record and the previous runtime PR description.

## Implementation versus contract evidence

| Property | Mechanism / source | Test evidence | Evidence boundary |
|---|---|---|---|
| Policy revision authority and monotonic active pointer | Migrations 0005/0007, `serea-storage/src/policy.rs` | Policy storage and migration tests; activation/snapshot concurrency tests in `migration_0005_tests.rs` | Mechanically constrained and tested; no proof against direct database-file tampering. |
| Atomic activation and event/audit recording | Transaction-scoped Storage and EventBus APIs | fault-injection and rollback tests | Tested software transaction behavior; not physical power-loss proof. |
| Request/action sealing, Task/Step binding, R2 set | Migrations 0005/0006, `serea-storage/src/approval.rs` | `an_action_step_plan_capability_generation_and_descriptor_must_all_match`, bounded/incomplete/member tests | Structural constraints plus case tests. |
| One-use consumption and concurrent authority | `UNIQUE(step_id)`, sealed use rows, typed use transaction | duplicate-use, independent-connection use, distinct R2 member and revocation race tests | SQLite serialization for tested interleavings. Exactly-once effects are not established. |
| Deterministic policy and provenance | `serea-policy/src/lib.rs` | deny precedence, order permutation, task ceiling, root, PROACTIVE, credential handoff, trusted context tests | Unit-tested deterministic behavior; host-supplied trusted facts remain integration responsibility. |
| Authenticated response/revocation and expiry | `serea-policy/src/approval.rs`, typed Storage transaction | authenticated port/session/elevation, replay, inclusive expiry, cancellation and response race tests | Verifier port is tested with fakes; production authenticator remains host integration. |
| Mixed Plan, WAIT, ALLOW / REQUIRE_APPROVAL and wake | TaskEngine public builder, Storage complete-plan and wake transactions, Scheduler public path | `task_engine_approval_e2e.rs`, TaskEngine approval and Storage wake/restart cases | Focused public integration path plus fault/restart tests. |
| Classification, redaction and credential separation | approval request constructors and redacted Debug implementations | protected-class refusal, forged provenance, scope ceiling, debug redaction and ordinary-approval/credential separation tests | API-level tests; trusted classification source correctness is a host responsibility. |
| No external execution in P6 | no production invoke/dispatch/RequestId surface in P6 crates; evidence-only policy result | public E2E counting provider confirms zero calls; Full CI release fault seam exclusion | Strong source/test boundary for current code. P8 remains unimplemented. |

Security review found no unresolved P6 authority blocker. The important residuals are the host authenticator and classification-source integration, hostile database-file access/corruption, platform durability assumptions, and all P8 dispatch obligations. Detailed attack matrix and test names are in [P6F closure](P6F-closure.md).

## Hosted exact-head CI evidence

Before this documentation change, all three required workflows passed at exact runtime source `09e2cc3d272bd445e13613310e238497657e1ff9`:

| Workflow | Run ID | Job conclusions |
|---|---:|---|
| Fast CI | `38142851922` | Linux fast checks `114480086510`: success |
| Full CI | `38142851916` | Linux stable `114480086683`, Rust 1.85 MSRV `114480086476`, macOS Intel `114480086616`, macOS arm64 `114480086658`: all success |
| SQLite cross-architecture | `38142851926` | arm64 producer `114480086378`, Intel producer `114480086576`, arm64 DB on Intel `114480291443`, Intel DB on arm64 `114480596561`: all success |

Because this closure creates a new documentation commit, those historical runs do not constitute exact-HEAD verification of the new commit. New required exact-head workflows must pass after push before this status can be treated as final. If the post-edit workflows are not available, replace the status above with the precise pending state; do not substitute the earlier IDs.

## P8 boundary and deferred work

Production P6 does not call `CapabilityProvider::invoke`, create dispatch intent, mint `RequestId`, create an unauthorized Grant, accept model-origin authority, expose credentials, or bypass fresh Policy revalidation. P6 produces durable authority evidence and workflow coordination only.

P8 must implement dispatch intent and provider-call lifecycle, atomic Grant-use/dispatch-intent coordination, `RequestId` semantics, provider idempotency and ambiguous-outcome reconciliation, fresh Policy/overlay/Task/Plan/registry/descriptor/arguments/expiry/cancellation/revocation checks immediately before dispatch, result persistence, and credential custody/exposure rules. Exactly-once external effects are not claimed. Software fault testing does not establish physical power-loss durability.

## Merge readiness review

### PR #6 → main

- Actual head/base: `p6/preimplementation-audit` → `main`; head `1c04451469a810a484e92fa7f0e5eb249451a54e`; merge base is current main `e17fbfc7f3e75f0bc95a66d693fe11b57a6ced50`.
- State: open, draft, unmerged. It is documentation-only and has no runtime implementation. Its complete diff is the P6A audit/decision/ADR/protocol package.
- Dependency: PR #10 explicitly targets the P6A branch and depends on its accepted contract. Merge #6 first. No conflict is reported by GitHub's current PR metadata; no merge has been attempted.
- Checks: P6A exact-head CI was successful at its source head (Fast `38060643714`, Full `38060643636`, SQLite `38060643647`), but after P6A is merged, verify the resulting main commit and required checks. Do not infer future main checks solely from branch checks.

### PR #10 → updated main

- Actual head/base: `p6/policy-approval-runtime-privacy` → `p6/preimplementation-audit`; head `09e2cc3d272bd445e13613310e238497657e1ff9`; merge base equals P6A HEAD.
- State: open, draft, unmerged. GitHub currently reports mergeable; this is not a guarantee after #6 lands.
- Dependency: #10 relies on P6A ADR/protocol contracts and should be retargeted to `main` only after #6 is merged, if GitHub does not update the base automatically. Retargeting is not performed here.
- Checks: its exact current source head has successful Fast/Full/SQLite runs listed above. These runs do not prove the future post-merge main result. After #6 merges and #10 is retargeted, fetch the updated base, inspect the new complete diff and conflicts, then run required CI on the exact resulting #10 head and any merge-test object before considering integration.

No PR was merged, marked ready, retargeted, or otherwise changed in this audit. No branch was deleted or force-pushed.

## Historical handoff supersession

The dated addendum below preserves earlier handoff states while making the final closure record discoverable.
