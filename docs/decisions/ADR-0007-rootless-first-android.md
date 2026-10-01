# ADR-0007: Rootless-First Android with Isolated Optional Root

- Status: **Accepted**
- Architecture version: `serea-arch/0.1.0`
- Decision date: 2026-10-01

## Context

Root access materially expands the effect surface and is unavailable on many devices. Core functionality must remain useful without it, while optional root-dependent capabilities need explicit, host-controlled availability and authorization.

## Decision

Design Android capabilities rootless-first. The small enumerated optional-root surface uses the same `CapabilityId` for rootless and root-backed implementations and differs only by `implementation_id`. Root is never a requirement for general operation. Root operations always require approval; absent root produces structured `CAPABILITY_UNAVAILABLE`, not startup failure or fallback to a laxer implementation.

## Consequences

- Root capability availability is data, not authority; device capability reports grant nothing.
- The root provider is isolated behind registered descriptors and cannot expose arbitrary shell execution.
- No implementation may bypass biometric confirmation, platform safety controls, policy, or approval.
- P1 defines provider ports and metadata only; it does not implement an Android/root provider.

## Rejected alternatives

- Root-first support: excludes ordinary devices and makes privileged access foundational.
- Treat root availability as permission: confuses capability with authorization.
- Generic root shell capability: allows arbitrary commands outside the frozen enumerated surface.
- Start a root operation automatically when root is present: violates the approval requirement.

## Frozen source docs

[System Overview §6](../architecture/01-system-overview.md#6-non-goals-at-this-phase); [Capability Protocol §3.1](../protocols/01-capability-protocol.md#31-field-semantics); [Policy Protocol §4.3](../protocols/04-policy-protocol.md#43-additional-standing-rules); [Device Protocol §6](../protocols/07-device-protocol.md#6-device-capability-reporting); [Threat Model AS-9](../threat-model/02-adversaries-and-attack-surface.md#as-9-root-provider).
