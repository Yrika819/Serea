# ADR-0009: Approval Grants Are Bounded and Task-Bound

- Status: **Accepted**
- Architecture version: `serea-arch/0.1.0`
- Decision date: 2026-10-01

## Context

An approval is delegated authority, not a general account permission. Broad, permanent, reusable consent makes a single prompt a standing authority channel and allows a later task or changed request to act without informed consent.

## Decision

Every `ApprovalGrant` is restricted to one capability and version, an exact capability-defined scope, an expiry, a maximum-use count, and the requesting `TaskId`. Consumption is atomic and idempotent per step. Grants can satisfy `RequireApproval`, but cannot override a policy denial; the model cannot mint or widen grants.

## Consequences

- Requests and grants remain tied to validated arguments and auditable task/step state.
- A new task, broader scope, extra use, or expired grant requires a new approval.
- Offline or absent-device responses do not become pre-grants; silence and timeout mean no grant.
- Approval handling is tested as a durable authority boundary, not a UI-only interaction.

## Rejected alternatives

- Permanent “allow all” grant: unbounded authority and no reliable consent to a specific action.
- Wildcard scope or capability family grant: can authorize materially different objects or operations.
- Approval that survives task binding: turns one consent into transferable standing permission.
- Let grants override `Deny`: makes an absolute policy refusal advisory.

## Frozen source docs

[Approval Protocol §§1–4](../protocols/05-approval-protocol.md#3-approvalgrant); [Policy Protocol §§4, 6](../protocols/04-policy-protocol.md#4-rule-evaluation); [Task Protocol §4](../protocols/02-task-protocol.md#4-task-state-machine); [Threat Model AS-3](../threat-model/02-adversaries-and-attack-surface.md#as-3-approval-prompt-rendering-and-response).
