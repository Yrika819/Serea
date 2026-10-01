# ADR-0011: FakeGoalLatch Only Until the Integration Gate

- Status: **Accepted**
- Architecture version: `serea-arch/0.1.0`
- Decision date: 2026-10-01

## Context

A real GoalLatch runtime contract cannot be inferred safely from source documentation or an assumed schema. Serea needs to exercise the adapter seam without granting the real system access before its behavior, state mapping, evidence, worktree, and session properties have been observed.

## Decision

P0 freezes the GoalLatch adapter contract only; no provider is implemented or registered. P15 is planned to implement the deterministic offline `FakeGoalLatchProvider` (no network, filesystem, subprocess, or credentials). The fake remains the only provider through this pre-integration plan. Real integration is unscheduled and requires separate explicit phase authorization after every live verification in GoalLatch Adapter Protocol §9 passes; this ADR does not authorize it.

## Consequences

- Before P15, `host.goal.*` is unavailable; in P15 it is backed only by the fake and cannot reach real host work.
- Fake scenarios exercise the same public protocol types, result validation, and receipt path without sharing private adapter types.
- No `local_mcp::*` dependency, GoalLatch database access, or direct adapter bypass is permitted.
- `codex_allowed` remains false; delegation does not imply a Codex route.

## Rejected alternatives

- Build a real adapter from README, checked-in schema, or upstream `main`: none proves the running binary's contract.
- Keep fake and real implementations active simultaneously: makes provider attribution and receipts ambiguous.
- Use fake semantics that omit receipts or actual result shapes: fails to test the protocol path.
- Enable Codex as part of the initial seam: changes the trust boundary before evidence exists.

## Frozen source docs

[GoalLatch Adapter §§1–2, 6, 9–10](../protocols/08-goallatch-adapter-protocol.md#9-real-adapter-readiness-gate); [Model Protocol §8](../protocols/03-model-protocol.md#8-codex-exclusion); [System Overview §6](../architecture/01-system-overview.md#6-non-goals-at-this-phase); [Threat Model AS-10](../threat-model/02-adversaries-and-attack-surface.md#as-10-goallatch-adapter-future).
