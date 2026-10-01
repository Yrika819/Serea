# ADR-0003: Ollama Cloud Model Roles and Codex Exclusion

- Status: **Accepted**
- Architecture version: `serea-arch/0.1.0`
- Decision date: 2026-10-01

## Context

The system needs normal assistant, strict structured-output, vision, and structured-repair capability without binding orchestration to a particular model. An implicit fallback to a code-specialist model would create a privileged escalation path.

## Decision

Use the frozen Ollama Cloud roster and host-configured role routing: `nemotron-3-nano-30b` is the default/planning/analysis/extraction model; `gpt-oss-20b` is strict structured fallback and repair; `gemma-4-31b` is selected for vision inputs only. `codex` is known but disabled and is excluded from every normal routing chain. `codex_allowed=false` by default; only an explicit, durable host policy can alter it for delegated host-goal work after the real adapter gate.

## Consequences

- Model selection is deterministic from purpose, capabilities, data class, constraints, health, and configured preference.
- Planning and extraction require strict structured output; repair is bounded and isolated.
- Fallbacks are configured, bounded, and audited; failures never route to Codex.
- No real model service is needed for P1 or ordinary tests; tests use scripted synthetic providers.

## Rejected alternatives

- Use Codex as an ordinary fallback: failure content or provider outage could silently increase privilege.
- Select models by scattered provider-specific branches: makes routing inconsistent and hard to test.
- Let model output choose its successor: gives untrusted output control of routing.

## Frozen source docs

[Model Protocol §§1, 5.1, 6, 8, 10](../protocols/03-model-protocol.md#51-initial-model-roster); [System Overview §5](../architecture/01-system-overview.md#5-model-roster-in-this-architecture); [Bounds Protocol §§7–8](../protocols/10-bounds-protocol.md#7-cost-bounds).
