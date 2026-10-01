# ADR-0012: Capability/Provider Protocol Boundary

- Status: **Accepted**
- Architecture version: `serea-arch/0.1.0`
- Decision date: 2026-10-01

## Context

Providers need to implement effects and model calls, but must not evaluate their own authority, inspect task policy, access another provider's credentials, or be invoked through ad hoc paths. Shared request types and trait ports need one owner so crates cannot drift in their interpretation.

## Decision

`serea-protocol` owns the frozen shared types, identifiers, schemas/serialization contract, errors, and provider port traits. Implementations live in leaf provider crates and are injected/registered by `serea-core`. Capability providers receive validated `ActionRequest` and constrained `ProviderContext`; host code owns registry, policy, approval, output validation, and durable receipt handling. The normal capability path is the only route to an effect.

## Consequences

- Protocol crate has no internal runtime dependency; higher layers depend downward and the workspace graph remains acyclic.
- Provider swaps do not change callers or task-engine decisions.
- Interface-only implementations are testable with synthetic fixtures and scripted test doubles.
- P1 creates ports and compile-time contracts only; no provider connects to an external service.

## Rejected alternatives

- Duplicate request/error types in each provider: contract drift and inconsistent validation.
- Provider-to-provider calls: leaks credentials and creates hidden authority paths.
- Put port traits in Core: forces leaves to depend upward and creates dependency cycles.
- Runtime plugin discovery or model-driven registration: makes the capability registry an open, model-influenced world.

## Frozen source docs

[Protocol Index §§0, 4–7](../protocols/00-protocol-index.md#7-change-control); [Capability Protocol §§1, 9–10](../protocols/01-capability-protocol.md#9-provider-interface); [Model Protocol §2](../protocols/03-model-protocol.md#2-modelprovider); [Crate Map §§1–3, 6](../architecture/03-crate-map.md#1-the-layering-rule); [Trust Boundaries TB-3 and TB-8](../architecture/02-trust-boundaries.md#tb-3-core-to-external-service).
