# Serea — Repository Agent Guide

This file is the repository entry point for coding assistants and review agents. It is a **navigation and safety guide**, not an authorization to execute a phase or change a protected branch.

## Read first

1. **[Current Personal Beta Roadmap (2026-11-12)](docs/roadmaps/2026-11-12-personal-beta-roadmap.md)** — current phase map, delivery gates, Owner decision register and deadlines.
2. [Existing G10–G15 Execution Plan](docs/g9/g10-execution-plan.md) — original phase boundaries and release-critical path.
3. [Personal Beta Scope](docs/g9/beta-scope.md) — P0 / P1 / P2 and Demo Beta distinction.
4. [Owner Decision Ledger](docs/plans/2026-10-26-personal-beta-owner-decision-ledger.md) — historical approval categories; **confirm more recent explicit Owner decisions before acting**.
5. Relevant `SECURITY.md`, `docs/decisions/` ADRs and touched subsystem tests.

At session start, **fetch main and target PR status from GitHub**. SHA values and dated check results in documents are snapshots, not evergreen facts.

## Current handoff snapshot (2026-11-12 JST)

- Canonical main `27f56e972a6f8f0b9406b6054c1637fdfb3c587f`.
- PR [#232](https://github.com/Yrika819/Serea/pull/232) is **Draft / OPEN**, with Primary-only Calendar security hardening and passing Required CI at recorded HEAD `71caa1b9ab6434f0fabfc287b4316add8317af03`. **Not merged.**
- PR [#231](https://github.com/Yrika819/Serea/pull/231) already merged. Synthetic in-flight Consent Process Death E2E passed on Android API 26 and 35; this does **not** establish success of real Google OAuth.
- Next work, if expressly authorized: Owner review of #232, then H4-B authorization/session/error hardening. Separately plan application ID / certificate / OAuth configuration. See roadmap for gates.

## Hard boundaries

- **Never** infer approval from a roadmap milestone, issue, previous PR, or passing CI. Require a fresh, explicit Owner instruction for Draft→Ready, Merge, real OAuth/Google Cloud changes, real AI activation, Google or external-network data access, production signing, APK distribution or Pixel/device operations.
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
