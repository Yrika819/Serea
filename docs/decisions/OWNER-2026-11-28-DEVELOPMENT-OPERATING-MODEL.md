# Owner Decision: Serea Development Operating Model

**Status: APPROVED / EFFECTIVE IMMEDIATELY**

**Decision date:** 2026-11-28 JST
**Authority:** Owner message in the Serea development session. This decision supersedes conflicting earlier Owner instructions, repository guidance and stop conditions within the scope below. It does not erase historical test outcomes or extend product/data permissions that the Owner explicitly retained.

## 1. Objective

Optimize for both safety and delivery speed. Reversible, scoped development work is delegated to the agent. Security machinery must support delivery rather than become a goal in itself. The priority is developing Serea into a usable AI secretary while preserving data boundaries and existing evidence.

## 2. Standing development authorization

Without requesting another design or implementation approval, the agent may:

- change product code, tests, documentation, build logic and development tools;
- investigate and repair compiler, test, runtime, environment and design failures;
- build, lint and run unit, integration and synthetic tests;
- use designated development emulators with synthetic data, generate/install diagnostic APKs, and perform narrowly scoped cleanup of campaign-owned packages;
- add appropriately scoped development campaigns;
- make reasonable implementation/design decisions and continue into executable work rather than stopping at a report or proposal;
- commit and push to the assigned development branch and create/update Draft PRs, separating unrelated concerns into scoped commits/branches/PRs.

Preserve pre-existing uncommitted changes, generated evidence, APKs, keys and unrelated outputs. Do not overwrite or discard them merely to obtain a clean workspace.

## 3. Stop-condition policy

A fixable compile/test failure, transient network or environment failure, campaign-capacity issue, minor design question, incomplete non-dependent validation, or temporary CI failure is not by itself a stop condition. Diagnose the cause, apply a bounded repair or safe alternative, and continue independent work. Do not repeat an unchanged failing operation indefinitely. Separate a proven pre-instrumentation abort from an instrumentation attempt whose outcome is unknown.

Seek fresh Owner judgment only where the task would newly involve real personal-data/credential transmission, real API keys or provider activation outside the approved scope, unapproved billing, production signing, physical-device changes, irreversible data loss, main merge/public release, a major security-boundary change, or a genuinely unavoidable technical/resource limit after reasonable alternatives.

## 4. Campaign requirements

Remove arbitrary fixed campaign-count caps. Campaign creation is an engineering decision based on available capacity, the designated target and a synthetic test purpose. Keep unique campaign IDs and namespaces, APK identity/signature/manifest verification, target-AVD attestation, existing-data protection, campaign evidence and cleanup verification. Never reuse an ID or erase evidence to make a quota appear available. A failure proven to occur before instrumentation starts must be recorded as not executed; an instrumentation timeout, lost transport or otherwise ambiguous start remains outcome-unknown and is not silently retried as a pass.

## 5. Completion and dependency policy

Judge each bounded phase against its own acceptance conditions. Do not label a completed phase PARTIAL solely because a separate phase has open reliability work. Classify residual issues by severity and actual dependency; resolve issues that block the current integration, retain other reliability work as explicit backlog, and advance independent work.

The S1-02 bounded synthetic Chat/Embedded Core runtime gate is **COMPLETE** based on the recorded 1/1 passing campaign `s1bc3a5e21dd6`, with `RUNTIME_PASS` and `CLEANUP_PASS`. This does not claim real provider access, durable personal Chat history, process-death recovery, or completion of S1-01B.

S1-01B remains separately assessed. Its known 16-KB lifecycle failure, native-internal cancellation gap, process-death/WAL gap and variant-isolation evidence must retain their truthful status. They are not automatically integration blockers; block only when a concrete dependency is demonstrated.

## 6. Cloud / Embedded Core scope

Preserve Cloud and Embedded Core changes and begin their integration work through implementation and verification. Keep the existing Egress-before-Usage-before-Dispatch ordering, fail-closed behavior, credential redaction and public/synthetic data policy. This decision does not itself authorize real API credentials, live Cloud AI activation, Google/OAuth, personal-data egress, billing or production release.

## 7. Retained boundaries

This decision authorizes development work on assigned branches and Draft PRs; it does not authorize main merge, making a Draft PR Ready, production signing/public distribution, physical-device use, real personal-data or credential transmission, unapproved billing, or irreversible cleanup. Existing project and storage protections remain active unless specifically contradicted above. All test results, including historical failures, must be represented accurately.

## 8. Session application record

- S1-02 bounded runtime: COMPLETE (synthetic Chat/Core UI test 1/1 PASS; exact evidence is recorded in the S1-02 Owner decision and campaign ledger).
- S1-01B overall: not yet complete; residual issues are being classified against actual Cloud/Core integration dependencies.
- Current implementation/validation plan: [`docs/plans/2026-11-28-cloud-embedded-core-integration-execution-plan.md`](../plans/2026-11-28-cloud-embedded-core-integration-execution-plan.md).
