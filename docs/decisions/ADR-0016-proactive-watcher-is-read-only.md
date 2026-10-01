# ADR-0016: Proactive Watcher Is Read-Only

- Status: **Accepted**
- Architecture version: `serea-arch/0.1.0`
- Decision date: 2026-10-01

## Context

The proactive watcher is awakened by schedules and externally shaped candidates. If it can write or raise approval, attacker-controlled content can cause unattended effects. Calling the policy restriction an implementation convention would not provide an enforceable boundary.

## Decision

In `PROACTIVE` automation context, policy permits only `OBSERVE` and `LOCAL_STATE`. Every other risk class is denied; the watcher raises no approvals and emits proposals for a user to act on. Its work and proposal count remain subject to host-owned durable bounds. This ADR records the rationale for the frozen P0 rule and does not add policy variants or change protocol behavior.

## Consequences

- The policy engine must receive a host-assigned automation context and test it independently from model output.
- Scheduler wakeups cannot acquire authority or bypass the normal host policy path.
- A proposal is not an action or approval; the user must initiate any later effect through an ordinary task.
- P1 defines the shared protocol boundary only; watcher and scheduler implementation remain deferred.

## Rejected alternatives

- Permit writes after a watcher-generated approval: allows unattended content to start an effect path and defeats the proposal-only design.
- Rely on prompt instructions to keep the watcher read-only: model output is untrusted.
- Treat a low call budget as authorization: bounds limit quantity and do not decide whether an action is allowed.

## Frozen source docs

[Policy Protocol §§4.2–4.3](../protocols/04-policy-protocol.md#43-additional-standing-rules); [Approval Protocol §7](../protocols/05-approval-protocol.md#7-approval-fatigue); [Bounds Protocol §§2, 8](../protocols/10-bounds-protocol.md#8-bounds-are-not-security-policy); [Execution Pipeline §7.2](../architecture/04-execution-pipeline.md#72-the-proactive-watcher); [Threat Model AB-30](../threat-model/03-abuse-cases-and-mitigations.md#ab-30-proactive-watcher-turns-into-an-unattended-writer).
