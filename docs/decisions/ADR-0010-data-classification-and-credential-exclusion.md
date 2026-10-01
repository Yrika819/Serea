# ADR-0010: Host-Enforced Data Classification and Credential Exclusion

- Status: **Accepted**
- Architecture version: `serea-arch/0.1.0`
- Decision date: 2026-10-01

## Context

Prompts, retrieved data, providers, devices, and logs cross trust boundaries. Prompt instructions cannot protect secrets, and a generic serialized request can accidentally expose credential material or data whose classification is unknown.

## Decision

Use the frozen ordered `DataClass` set and enforce classification, composition, redaction, and egress in host code. Unknown or unclassified data fails closed at the highest class. `CREDENTIAL` material is excluded from Serea-owned storage and model inputs and remains in OS credential stores; providers receive only opaque `CredentialHandle` references and resolve secret bytes inside the credential-store boundary for a single call.

## Consequences

- Schemas are closed-world allowlists; undeclared properties and credential-shaped input fail validation.
- `SECRET` and `CREDENTIAL` never reach local or cloud models; `PRIVATE` cloud egress remains host-disabled by default and redacted when permitted.
- Derived memory inherits the source class, requires explicit extraction and provenance, and supports deletion cascades.
- Types and serialization must make accidental credential flow difficult, while tests verify structural exclusions rather than prompt wording.

## Rejected alternatives

- Rely on prompts asking a model not to reveal secrets: behavioral requests are not enforcement.
- Use only a credential-name denylist: novel names, values, and opaque blobs bypass it.
- Store credentials beside ordinary task data: broadens the number of components and logs that can expose them.
- Treat redaction as permission to persist the original in general logs/events: confuses transit projection with stored classification.

## Frozen source docs

[Data Classification §§1–6](../protocols/09-data-classification-protocol.md#1-why-classification-exists); [Capability Protocol §§3–4](../protocols/01-capability-protocol.md#3-capabilitydescriptor); [Trust Boundaries TB-2 and TB-4](../architecture/02-trust-boundaries.md#tb-2-model-to-core); [Threat Model Assets AST-3, AST-7, AST-14](../threat-model/01-assets-and-trust-boundaries.md#1-assets).
