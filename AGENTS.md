# Serea — Repository Agent Guide

This file is the repository entry point for coding assistants and review agents. It is a **navigation and safety guide**. The current authorization and development operating model are in [Owner Decision: Serea Development Operating Model (2026-11-28)](docs/decisions/OWNER-2026-11-28-DEVELOPMENT-OPERATING-MODEL.md); follow that decision over conflicting historical guidance.

## Read first

1. **[Owner Decision: Serea Development Operating Model (2026-11-28)](docs/decisions/OWNER-2026-11-28-DEVELOPMENT-OPERATING-MODEL.md)** — effective development authorization, rationalized stops, campaign policy, completion policy and retained boundaries.
2. [Current Personal Beta Roadmap (2026-11-12)](docs/roadmaps/2026-11-12-personal-beta-roadmap.md) — historical phase map and delivery context; check current Owner decisions before applying old deadlines or stop conditions.
3. [Existing G10–G15 Execution Plan](docs/g9/g10-execution-plan.md) — original phase boundaries and release-critical path.
4. [Personal Beta Scope](docs/g9/beta-scope.md) — P0 / P1 / P2 and Demo Beta distinction.
5. [Owner Decision Ledger](docs/plans/2026-10-26-personal-beta-owner-decision-ledger.md) — historical approval categories; newer explicit decisions take precedence.
6. Relevant `SECURITY.md`, `docs/decisions/` ADRs and touched subsystem tests.

At session start, **fetch main and target PR status from GitHub**. SHA values and dated check results in documents are snapshots, not evergreen facts.

## Current handoff snapshot (2026-11-13 UTC)

- Canonical main `8c137c538135d66fb2dab16bd74c10cc56146da4`; Tree `792287a6b359c547f7a2026e7c3d2054be541176`. This includes merged PR #237 (Cargo 1.100 release artifact support and rustfmt-only formatting in `types.rs`).
- H4-A is complete through merged PR #234 (`eaa8f7cefb608d846c336a972f33b0812f269f57` was the prior main HEAD). PR #232 remains **OPEN, not merged, and ready for review (not Draft)**; its Primary-only implementation is on main through #234. Do not change PR #232.
- PR #231 already merged. Synthetic in-flight Consent Process Death E2E passed on Android API 26 and 35; this does **not** establish success of real Google OAuth.
- H4-B implementation is in PR #236, still Draft and now synchronized with main. Its current validation is running on HEAD `43698d47caa134b2b0fc0ffef43daffed2d669f3` (a dated snapshot); Private Google HTTP remains disabled and no real Google connection is authorized.
- PR #233 is the documentation-only roadmap handoff. Its recorded pre-sync HEAD is a historical snapshot; current branch state and checks must be fetched from GitHub.
- This is a dated handoff snapshot. Fetch GitHub before acting and distinguish current state from historical snapshots in the roadmap.

## Hard boundaries

- Within the effective Owner decision, do not add a second design/implementation approval gate for reversible assigned-branch development. That decision does not authorize Draft→Ready, main merge, real OAuth/Google Cloud access, real AI activation, personal-data/credential transmission, unapproved billing, production signing/distribution, physical-device operations or irreversible deletion.
- **Never** infer approval for those retained boundaries from a roadmap milestone, issue, previous PR, or passing CI. Require a fresh, explicit Owner instruction. This file itself grants no such approval.
- Do not bypass GitHub branch protection, disable Required checks, force-push or overwrite the Owner's working tree. Favor isolated branches and **Draft PRs**.
- **Public remains safe and Fake-first**. Google OAuth is allowed only in the Owner-controlled Private build after approval. Private Preview currently uses Fake Calendar even with a real-OAuth build flag. Do not treat a compiled code path as activated.
- Primary Calendar real reads (once separately authorized) must use `calendarId=primary`, events-read-only scope only, and reject nonprimary/CalendarList access **before** authorization, token acquisition and HTTP. Keep normal Demo multi-calendar fixtures working.
- Google-origin and other nonpublic data must never be sent automatically to Cloud AI. Maintain Egress-before-Usage-before-Dispatch, durable accounting, token/credential redaction and fail-closed authorization.
- Gmail real `gmail.readonly` remains disabled until separate Restricted Scope / privacy / Owner review.
- An unapproved AI provider remains Demo/Fake; label it honestly.
- Do not access, delete or reset personal device data without an explicit, tightly scoped Owner authorization.

## Testing and reporting

For changes affecting Android / OAuth or release gates: retain API 26/35 Formal, Required 4 checks, Private/Public variant isolation, Unit/Integration/Security, RustSec, Native Platform, SQLite portability and Git identity (as applicable). Keep test counts and failure assertions intact; distinguish `NOT_RUN`, `NOT_DELIVERED`, `DELIVERED_REJECTED`, `HOST_INFRA`, `TEST_FAILURE`.

Before claiming DONE, report exact PR, HEAD, Tree, same-SHA check runs, independent review (separate from GitHub APPROVED), any non-Required failures, and outstanding Owner decisions. End with `DONE`, `READY_FOR_OWNER_REVIEW`, `PARTIAL`, or `BLOCKED` based on actual evidence.
