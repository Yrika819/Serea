# ADR-0002: Structured Action Requests, Zero Model Authority

- Status: **Accepted**
- Architecture version: `serea-arch/0.1.0`
- Decision date: 2026-10-01

## Context

Model output is untrusted and may be influenced by malicious or stale content. A direct route from generated prose to an effect would make validation, host policy, and human approval optional.

## Decision

Models return structured proposals. Host code validates the schema and constructs `ActionRequest`, resolving every authority-bearing or routing field from the registry and durable task state. Every invocation passes schema validation, registry lookup, deterministic policy, required approval, provider invocation, evidence/receipt handling, and durable commit. Models have no authority and their claims are not evidence.

## Consequences

- Unknown or invalid fields fail closed; model-supplied risk, provider, authorization, or deadline values are not accepted.
- Providers are the only place effects occur and must return schema-valid results and receipts for effects.
- Invalid model structure is an ordinary typed failure, not best-effort execution.
- Callers share one request contract instead of defining authority-bearing ad hoc tool payloads.

## Rejected alternatives

- Parse action requests from free prose: ambiguous, unbounded, and impossible to validate reliably.
- Let a model choose provider, risk class, scope, or approval: delegates host authority to an adversarial input.
- Accept partial or “best effort” plans: hides which parts were understood and can produce unintended effects.

## Frozen source docs

[Capability Protocol §§1, 4, 6](../protocols/01-capability-protocol.md#1-core-principle); [Model Protocol §§1, 4.1, 7](../protocols/03-model-protocol.md#41-the-trust-boundary-stated-precisely); [Trust Boundaries §3](../architecture/02-trust-boundaries.md#3-the-authority-model); [Execution Pipeline §1](../architecture/04-execution-pipeline.md#1-the-ten-stages); [Adversaries AS-1–AS-2](../threat-model/02-adversaries-and-attack-surface.md#2-attack-surface-enumeration).
