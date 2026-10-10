# ADR-0037: Deterministic policy semantics and revision authority

Status: **Proposed** · Date: 2026-10-10 · Architecture: `serea-arch/2.6.0` → `2.7.0` if accepted

Surfaces affected: `serea.policy/1` semantics (prose only), `serea.event/1` unchanged,
`serea.task/2` unchanged. No wire-major change.

## Context

[Policy Protocol §4.1](../protocols/04-policy-protocol.md) defines a class-default table
(OBSERVE and LOCAL_STATE allow, writes generally require approval, CREDENTIAL hands off).
[Policy Protocol §4.2](../protocols/04-policy-protocol.md) then states that unmatched rules
fall back to `RequireApproval` and that "there is no permissive default". Read literally
together, these make the §4.1 table unreachable, because every `RiskClass` is in it, so the
fallback can never fire.

§4.2 also says rules are evaluated "most specific first", while
[§5](../protocols/04-policy-protocol.md) says the ordering is `priority` descending then
`rule_id` ascending. Those are two different mechanisms and neither document names the
other.

Neither §4.2 nor §5 says what happens when two matching rules disagree, one `DENY` and one
`ALLOW`, other than that a grant cannot override a deny.

The [P6 preimplementation audit](../plans/P6-preimplementation-audit.md) recorded these as
D1, D2 and D3. This ADR is the proposed resolution and is **not** owner-ratified.

## Options considered

**A — class defaults are a fixed code constant; explicit rules are durable data; `DENY`
overrides `ALLOW`.** Fail-closed by construction, because a missing rule is never an allow.
Predictable, because the default table is versioned with the code. Low authoring complexity.
Accidental privilege is structurally hard: there is no way to write a rule that promotes
`CREDENTIAL` above an explicit deny.

**B — explicit rules only, unmatched becomes `RequireApproval`.** Rejected. It makes §4.1
dead text and makes [Approval Protocol §7](../protocols/05-approval-protocol.md)'s success
measure — zero prompts for read-only work — unattainable, since every `gmail.messages.list`
call would prompt. It also contradicts §4.2's own step 7.

**C — class defaults as synthetic rules with priority ordering.** Rejected. It makes the
default table mutable data, so a priority error could promote `CREDENTIAL` above an explicit
deny, and an admin revision could change the meaning of every capability at once.

## Decision

**Proposed: Option A.**

1. The §4.1 class-default table is a fixed, code-level, versioned constant. It is not rule
   data and cannot be edited by an admin revision.
2. Explicit rules are the only durable rule data. Each has `rule_id`, `priority`, `enabled`,
   a closed match, a decision, and a `reason_code`.
3. Rule evaluation order is exactly: task policy ceiling, automation-context rules,
   data-class rules, scope rules, explicit rules, class default, then the fallback.
4. Among matching explicit rules, the winner is `priority` descending, then `rule_id`
   ascending. That is the complete precedence rule; there is no specificity score.
5. Any matching explicit `DENY` overrides every `ALLOW`, regardless of priority. This
   applies between rules, between a rule and a built-in default, and between defaults. It
   does not change the task ceiling, which is already a deny and sits outside the ordering,
   and it does not touch the capability overlay, which is not a policy rule and cannot be
   re-enabled by one.
6. Match dimensions are closed: `capability_id`, `risk_class`, `side_effect_class`,
   `authorization`, `automation_context`, `requested_by`, `enabled`. No `data_class`
   dimension, because the egress question is already a hard matrix. No glob, no regex, no
   negation, no expression language.
7. `RequestedBy` maps to automation context one-to-one and never self-upgrades: `USER` and
   `MODEL` to `INTERACTIVE`, `SCHEDULER` to `SCHEDULED`, `PROACTIVE_WATCHER` to `PROACTIVE`,
   `SYSTEM` to `SYSTEM`. `PROACTIVE` permits only `OBSERVE` and `LOCAL_STATE`. `SYSTEM`
   receives a **narrower** posture than `INTERACTIVE` — it may not auto-allow a `DESTRUCTIVE`
   or `CREDENTIAL` action and may not raise an approval for an effecting capability. No
   context grants authority; only `PROACTIVE` restricts.
8. Revision identity is monotonic `revision_id` plus `rules_digest`. The activation pointer
   is a singleton row that only advances. Activation and its `POLICY_CHANGED` append are one
   transaction. SQLite is the sole runtime authority; host configuration is a trusted import
   path and is never read at evaluation time.

## Consequences

- §4.1 and §4.2 become consistent without either being weakened.
- The P6 evaluator is a pure function of `PolicyInputV1` and the immutable revision, which
  makes determinism testable across restarts.
- "Deny everything except X" is not expressible; writing `X` as the allow and inheriting the
  default deny is the only shape. This is the acceptable direction to be inexpressive in.
- Four new bounds follow and are proposed in ADR-0040 rather than here.

## Verification obligations (future P6C)

- Permutation tests over rule sets that differ only in `priority` and `rule_id`.
- Deny-override tests at every priority combination.
- Provenance tests proving `MODEL` and `USER` produce identical decisions.
- Restart determinism over a 1000-rule revision.
- Pointer-advances-only tests, including a direct SQL edit attempt.

## Status

**Proposed. Not accepted. P6 runtime is not started and this ADR authorizes no
implementation.**
