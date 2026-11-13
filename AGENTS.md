# Serea — Repository Agent Guide

This file is the repository entry point for coding assistants and review agents. It is a **navigation and safety guide**, not an authorization to execute a phase or change a protected branch.

## Read first

1. **[Current Personal Beta Roadmap (2026-11-12)](docs/roadmaps/2026-11-12-personal-beta-roadmap.md)** — current phase map, delivery gates, Owner decision register and deadlines.
2. [Existing G10–G15 Execution Plan](docs/g9/g10-execution-plan.md) — original phase boundaries and release-critical path.
3. [Personal Beta Scope](docs/g9/beta-scope.md) — P0 / P1 / P2 and Demo Beta distinction.
4. [Owner Decision Ledger](docs/plans/2026-10-26-personal-beta-owner-decision-ledger.md) — historical approval categories; **confirm more recent explicit Owner decisions before acting**.
5. Relevant `SECURITY.md`, `docs/decisions/` ADRs and touched subsystem tests.

At session start, **fetch main and target PR status from GitHub**. SHA values and dated check results in documents are snapshots, not evergreen facts.

## Current handoff snapshot (2026-11-13 JST)

- Canonical main `eaa8f7cefb608d846c336a972f33b0812f269f57`; Tree `4cb69b2c6bec8ed60dd90bc3e03d5415d71226b0`. PR [#234](https://github.com/Yrika819/Serea/pull/234) merged H4-A on this SHA.
- PR [#232](https://github.com/Yrika819/Serea/pull/232) remains **OPEN, not merged, and ready for review (not Draft)**. Its Primary-only implementation is on main through #234. The open PR is retained at the latest observed HEAD; do not treat its state as the state of the integrated implementation.
- PR [#231](https://github.com/Yrika819/Serea/pull/231) already merged. Synthetic in-flight Consent Process Death E2E passed on Android API 26 and 35; this does **not** establish success of real Google OAuth.
- H4-B authorization/session/error hardening is the next implementation phase. Only the specifically instructed independent Draft PR scope is authorized; real Google OAuth, Cloud configuration, and later H4-C work remain separately gated.
- This is a dated handoff snapshot. Fetch GitHub before acting and distinguish current state from historical snapshots in the roadmap.

## Hard boundaries

- **Never** infer approval from a roadmap milestone, issue, previous PR, or passing CI. Require a fresh, explicit Owner instruction for Draft→Ready, Merge, real OAuth/Google Cloud changes, real AI activation, Google or external-network data access, production signing, APK distribution or Pixel/device operations. This file itself grants no such approval.
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
