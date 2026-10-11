# Policy Protocol

Protocol ID: `PROTO-POLICY` · Surface: `serea.policy/1` · Status: **FROZEN for P0**

Policy is **deterministic, host-owned, and independent of model output**. The
policy engine is a pure function of durable state, the capability descriptor,
the task, and the approval ledger. It has no model in the loop, and no
network call in the loop.

If the policy engine's answer would change if the model had phrased its request
differently, the engine is wrong.

---

## 1. Core principle

> The model may request an operation. It may not select its own policy
> classification, and it has no vote in the outcome.

The `risk_class` on a `CapabilityDescriptor` is host-authored and
host-reviewed. `ToolCallProposalV1` contains no risk field. Any undeclared or
authority-bearing model field rejects the entire proposal and is logged as a
sanitized `MODEL_SCHEMA_VIOLATION`.

## 2. Risk class

Frozen ordered set. Order matters: a capability has exactly one class, and the
class is a property of the capability, not of the call.

| # | Class | Meaning |
| --- | --- | --- |
| 0 | `OBSERVE` | Reads state. No external effect. |
| 1 | `LOCAL_STATE` | Mutates Serea's own durable state (memory, preferences, task records). |
| 2 | `REVERSIBLE_WRITE` | External effect with a defined inverse. |
| 3 | `EXTERNAL_WRITE` | External effect without a reliable inverse (creating a calendar event, filing a label). |
| 4 | `COMMUNICATION` | Content leaves the system to a third party. |
| 5 | `ELEVATED_DEVICE` | Requires privileges above the app's normal grant (root, or a privileged user gesture). |
| 6 | `DESTRUCTIVE` | Irreversible removal of data or capability. |
| 7 | `CREDENTIAL` | Touches secret material. |

A capability may only ever be in one class. If a capability spans two classes —
for example "create a calendar event" which is both `EXTERNAL_WRITE` and
`COMMUNICATION` — it takes the **highest** class. Splitting it into two
capabilities is preferred when the halves want different policy treatment.

## 3. `PolicyDecision`

```rust
pub enum PolicyDecision {
    Allow,
    RequireApproval(ApprovalRequest),
    RequireHandoff(HandoffRequest),
    Deny(DenyReason),
}
```

| Decision | Meaning |
| --- | --- |
| `Allow` | Proceed. No user involvement. |
| `RequireApproval` | Suspend the task in `WAITING_APPROVAL` and raise a prompt. |
| `RequireHandoff` | Suspend and route the request to a human channel that can handle credentials. |
| `Deny` | Refuse permanently for this request. The step fails. |

There is no "allow with a warning", no "allow but log", and no "ask the model
if it is sure".

### 3.1 `DenyReason`

A stable, machine-readable reason code. Never a free-text string. Reasons
drive user-facing messages and test assertions.

Examples: `CAPABILITY_DISABLED`, `TASK_POLICY_CEILING_EXCEEDED`,
`AUTOMATED_ACTION_FORBIDDEN`, `DATA_CLASS_NOT_PERMITTED_FOR_MODEL`,
`SCOPE_NOT_GRANTED`, `RETIRED_CAPABILITY_VERSION`.

### 3.2 `HandoffRequest`

Emitted only for `CREDENTIAL`-class capabilities. Specifies the human channel
and the reason. Serea never handles raw credentials in-band; it points a human
at the OS credential store or the device's own UI.

### 3.3 `PolicyDecision` is an internal type, not a wire type

`PolicyDecision`, `DenyReason`, `HandoffRequest`, `PolicyRule`, `PolicyInputV1` and
`AuthorizationEvidenceV1` are internal Rust types owned by the `serea-policy` crate
(Crate Map §3.1). They never cross a wire and carry no `serde` surface. `serea-protocol`
remains the frozen contract crate and holds the enums policy actually consumes —
`RiskClass`, `DataClass`, `SideEffectClass`, `Authorization`, `RequestedBy` — and the
`Clock` injection port. Adding a wire string for any of these types would be a wire-major
change and is not proposed.

## 4. Rule evaluation

### 4.1 Default rules

| Capability class | Default decision |
| --- | --- |
| `OBSERVE` | `Allow` |
| `LOCAL_STATE` | `Allow` |
| `REVERSIBLE_WRITE` | Configurable; default `Allow` for local-only, `RequireApproval` for external |
| `EXTERNAL_WRITE` | `RequireApproval` |
| `COMMUNICATION` | `RequireApproval` |
| `ELEVATED_DEVICE` | `RequireApproval` |
| `DESTRUCTIVE` | `RequireApproval` (and default-disabled; see §4.3) |
| `CREDENTIAL` | `RequireHandoff` |

Concrete defaults the user specified:

| Capability | Default |
| --- | --- |
| Gmail read | `Allow` |
| Calendar read | `Allow` |
| Memory write | `Allow` |
| Draft creation | Configurable |
| Calendar create | `RequireApproval` |
| Email send | `RequireApproval` |
| Root operation | `RequireApproval` |
| Credential operation | `RequireHandoff` |

#### 4.1.1 The default table is a fixed code constant

This table is a **code-level, versioned constant**, not rule data. It is imported with the
host binary and cannot be edited by an admin revision, by a device, or by a model. Making it
rule data would let one admin revision change the meaning of every capability at once and
would let a rule-priority error promote `CREDENTIAL` above an explicit deny. Explicit rules
are the only durable rule data.

### 4.2 Evaluation order

Rules are evaluated in a fixed, documented order:

1. **Task policy ceiling** — if `capability.risk_class > task.policy_class`,
   `Deny(TASK_POLICY_CEILING_EXCEEDED)`. The task cannot exceed its ceiling by
   planning harder.
2. **Context rules** — automation-context rules. The proactive watcher, for
   example, is restricted to `OBSERVE` and `LOCAL_STATE`; anything else is
   `Deny(AUTOMATED_ACTION_FORBIDDEN)`. This rule family is what makes the
   read-only proactive invariant enforceable rather than aspirational.
3. **Data-class rules** — whether the capability's data class may transit to
   the models or destinations involved. See
   [Data Classification §5](09-data-classification-protocol.md#5-egress-rules).
4. **Scope rules** — whether a previously granted scope covers this call.
5. **Class default** — the §4.1 table.
6. **Fallback** — `RequireApproval`. **There is no permissive default.** A
   capability with no matching rule requires approval. Failing open is the
   failure mode this ordering exists to prevent.

"Most specific first" in earlier drafts described this family order — stage 1 is the
narrowest constraint and stage 6 the broadest. It is **not** a specificity score
computed over rules, and no such score exists. Rule-to-rule precedence is defined
entirely by §5.

Two supremacy rules sit across the whole ordering and are absolute:

- **Explicit `DENY` beats every `ALLOW`.** Any matching explicit `DENY` overrides every
  matching `ALLOW`, regardless of `priority`, and overrides a built-in class-default
  `ALLOW` as well. This removes the entire "accidental high-priority allow" class and is
  what makes an emergency stop rule writable.
- **The task ceiling, the capability overlay and mandatory approval categories sit outside
  the rule ordering.** They are already denials or non-delegable requirements: no rule can
  raise a `policy_class`, re-enable a disabled or removed capability, or auto-grant a root
  operation.

### 4.3 Additional standing rules

- **Proactive watcher is read-only.** In automation context, only `OBSERVE`
  and `LOCAL_STATE` may reach `Allow`. Everything else denies. See
  [ADR-0016](../decisions/ADR-0016-proactive-watcher-is-read-only.md).
- **`DESTRUCTIVE` defaults to disabled.** A destructive capability must be
  explicitly enabled by the user *and* still requires approval. Both, not
  either.
- **Root requires approval, always.** There is no rule that can grant a root
  operation automatically. A matching rule whose decision is `ALLOW` does not
  relax this: a root operation reaches `RequireApproval` even when an explicit
  allow rule matches it, and all existing stricter requirements are preserved,
  including trusted-device confirmation where it is required.
- **No "allow everything" rule.** A policy rule cannot express unbounded
  blanket authority. Broad authority is expressible only as a bounded
  [approval grant](05-approval-protocol.md).

### 4.4 Automation context derivation

`AutomationContext` is host-authored and host-supplied. It is derived from **two trusted
inputs** — the durable Task `kind` and the trusted `RequestedBy` provenance — and from
nothing else. It is never derived from model output, from a caller-supplied field, or from
a comparison of enum discriminants. There is no numeric privilege ladder between these
values; each restriction below is stated explicitly.

The derivation is a total function over `(TaskKind, RequestedBy)`, evaluated in order,
first match wins:

```text
1.  TaskKind::Proactive                      -> PROACTIVE
2.  RequestedBy::ProactiveWatcher             -> PROACTIVE
3.  (TaskKind::Maintenance,  RequestedBy::System)   -> SYSTEM
4.  (TaskKind::Maintenance,  _)                    -> SYSTEM
5.  (TaskKind::Scheduled,     _)                   -> SCHEDULED
6.  (TaskKind::UserRequest | TaskKind::DelegatedHostGoal,
        RequestedBy::User | RequestedBy::Model | RequestedBy::Scheduler) -> INTERACTIVE
7.  (TaskKind::UserRequest | TaskKind::DelegatedHostGoal,
        RequestedBy::System)                 -> refused: INVALID_CONTEXT_COMBINATION
8.  (_, RequestedBy::ProactiveWatcher)        -> PROACTIVE        (already rule 2)
```

Rule 4 keeps host maintenance host-internal: a `MAINTENANCE` task never derives
`INTERACTIVE`, whichever provenance it carries. Rule 7 refuses a user-request task that
claims `SYSTEM` provenance rather than promoting it, because provenance is not authority.
Rule 5 is the "a scheduler cannot self-upgrade" rule: a `SCHEDULED` task never derives
`INTERACTIVE`, even when its action was triggered by a user.

The hard restrictions each derived context then applies. These are absolute and are applied
before any explicit rule is consulted (§4.2 stage 2):

| Context | Hard restriction |
| --- | --- |
| `PROACTIVE` | Only `OBSERVE` and `LOCAL_STATE` may reach `Allow`; everything else is `Deny(AUTOMATED_ACTION_FORBIDDEN)`. No approval is raised at all. |
| `SYSTEM` | Never `Allow` for `DESTRUCTIVE` or `CREDENTIAL`. Never raises an approval for a capability whose `side_effect_class` is not `NONE`. This is a **narrower** posture than `INTERACTIVE`, not a broader one: `SYSTEM` is not god mode. |
| `SCHEDULED` | Ordinary class defaults apply unchanged; no automatic allowance is created. A `SCHEDULED` task may not be created with a `policy_class` above `LOCAL_STATE` unless an explicit durable rule allows it — a creation-time check on the task, not an evaluation-time one. |
| `INTERACTIVE` | Ordinary class defaults apply. `USER` and `MODEL` provenance derive identically here; the model has no context of its own. |

Two consequences worth stating because they are the whole point of the exercise. A
restrictive Task kind is never relaxed by a different `RequestedBy`: a `PROACTIVE` task
whose action reports `USER` provenance still derives `PROACTIVE`, and a `SCHEDULED` task
whose action reports `USER` provenance still derives `SCHEDULED`. And an invalid
combination is either refused or frozen into the restrictive semantics; it is never
silently promoted.

## 5. Policy rules as data

```json
{
  "rule_id": "pol_0007",
  "priority": 40,
  "match": {
    "capability_id": "gmail.messages.list",
    "risk_class": "OBSERVE",
    "automation_context": "INTERACTIVE"
  },
  "decision": "ALLOW",
  "reason_code": "READ_ONLY_MAILBOX_OBSERVATION",
  "enabled": true
}
```

Rules are durable data, versioned, and evaluated deterministically:
`priority` descending, then `rule_id` ascending as a tiebreak. Given identical
durable state, the engine always returns the same decision — this is a
property with a dedicated test.

`match` fields are all optional; an absent field matches everything. There
is no expression language, no `if/then` scripting, and no way to write a rule
that consults the model, the clock's sub-second value, or a network call.

### 5.1 Closed match dimensions

Every `match` field is one of exactly seven closed dimensions, and each carries
**one exact value or is absent**:

| Dimension | Value domain |
| --- | --- |
| `capability_id` | one exact `CapabilityId` |
| `risk_class` | one `RiskClass` |
| `side_effect_class` | one `SideEffectClass` |
| `authorization` | one `Authorization` |
| `automation_context` | one `AutomationContext` |
| `requested_by` | one `RequestedBy` |
| `enabled` | boolean |

There is no `data_class` dimension. The egress question is answered by
[Data Classification §5](09-data-classification-protocol.md#5-egress-rules), a hard matrix,
and adding a field the model can influence indirectly would create a second, weaker answer
to it. Earlier examples in this document showed `"capability_id": "gmail.messages.*"` and
array-valued `match` fields. **Those forms are obsolete and invalid.** No glob, no regex, no
wildcard, no negation, no expression language, and no implicitly expanded capability
family. A rule that names a family is refused at write time.

### 5.2 Rule precedence

Among matching explicit rules the complete winner rule is:

```text
priority DESC, then rule_id ASC
```

That is the whole rule. There is no specificity score, no row order, no insertion order, no
hash iteration, and no "most specific first" tiebreak. `rule_id` is unique within one
revision, so the order is total and the decision is a pure function of durable state.
Applying the §4.2 supremacy rules, a matching `DENY` wins over this ordering.

## 6. What policy may and may not depend on

**May depend on:** durable policy rules; the capability descriptor; task state
and `policy_class`; the approval ledger; the automation context; declared data
classes; device registry state (paired, trusted, online).

**May not depend on:** model output of any kind; wall-clock time except a
coarse expiry check on grants; any network call; any randomness.

`PolicyInputV1` is the evaluator's input contract. It carries the P5
`PreparedActionV1` identity, the classified facts, the Task snapshot, the trusted
automation context and the policy revision identity. It carries **no raw
arguments** — a rule language over raw arguments is an interpreter — and **no
`now`** — expiry is the approval layer's job, which is the only reading
consistent with §6 and with Approval §4.

## 7. Policy changes are audited

Any mutation of policy rules emits `POLICY_CHANGED`
with a before/after diff, the actor, and the reason. Policy changes are not
reachable from model output and not reachable from the Android client's
ordinary settings screen — only from the host's local admin configuration.
Capability enabled/disabled/removal and experimental opt-in/out are registry
mutations and emit `CAPABILITY_REGISTRY_CHANGED`, not `POLICY_CHANGED`, as
specified by [ADR-0034](../decisions/ADR-0034-capability-manifest-registry-and-pinning.md).
P5 does not evaluate policy. P6 owns `PolicyDecision` evaluation after receiving
immutable `PreparedActionV1`; model visibility is not policy or approval
filtering.

### 7.1 SQLite is the sole runtime authority

Rules live in SQLite and nowhere else. Host configuration is a **trusted import
path**, never a live rules store, and is not read at evaluation time. The admin
flow is the one migration 0004 already established for the capability registry:

```text
trusted local admin request (host only, never model, never ordinary device settings)
  -> validate structurally (closed dimension set, bounded, no glob or family)
  -> create immutable revision rows plus a rules_digest, unactivated
  -> one transaction: advance the singleton activation pointer and append POLICY_CHANGED
  -> return the new revision identity
```

SQLite becomes the runtime authority the moment that transaction commits, so a
partial edit of configuration while the host runs cannot change a decision. Two
live authorities would make the engine's answer depend on which source a code
path happened to read, which no test can close.

### 7.2 Revision identity and activation

A revision carries a monotonic `revision_id`, which orders revisions and drives
the activation pointer, **and** a `rules_digest`, a SHA-256 over the canonical
JSON of the revision's complete rule array, which lets any reader — including a
P8 recheck — prove that a durable snapshot it holds is byte-identical to the one
an evaluation named. The activation pointer is a singleton row that **only
advances**; it cannot be deleted, stepped backwards, or pointed at an unactivated
revision. Activation and its event append are one transaction, so a failed
`POLICY_CHANGED` leaves the previous revision authoritative.

### 7.3 Evaluation is against one immutable revision

An evaluation runs against exactly one immutable revision and records that
revision's identity in the authorization evidence. P6 never re-evaluates a
decision it already made. P8 must independently re-evaluate against the current
pointer before dispatch, and a mismatch is a refusal rather than an automatic
continuation.

## 8. Testing obligations

| Property | Test |
| --- | --- |
| Determinism | Same durable state + same request ⇒ identical decision, across 1000 iterations and across restart. |
| No model influence | For every model output that contains a policy-like field, the decision is unchanged. |
| Ceiling enforcement | A `COMMUNICATION` step in an `OBSERVE` task denies. |
| Fail-closed | A capability with no matching rule requires approval. |
| Proactive read-only | Every non-`OBSERVE` capability in `PROACTIVE` context denies. |
| Live overlay wins | Registry resolution blocks new bindings when disabled/removed; P8 rechecks before dispatch. The overlay is not a policy-rule mutation. |
| No escalation | A task cannot raise its own `policy_class`. |
| Root always approval | No rule set yields `Allow` for a root capability, including a rule whose decision is `ALLOW`. |
| Deny supremacy | Every allow/deny priority permutation yields `Deny`. |
| Order independence | Permuting a revision's rules across `priority` and `rule_id` never changes a decision. |
| Context derivation | Each `(TaskKind, RequestedBy)` pair derives exactly the §4.4 context; a `PROACTIVE` task with `USER` provenance still denies a write; a `SCHEDULED` task with `USER` provenance does not become `INTERACTIVE`. |
| Pointer direction | A direct SQL edit that points the activation pointer at an older or unactivated revision is refused. |

## 9. Invariants summary

| # | Invariant |
| --- | --- |
| P1 | Policy is a pure, deterministic function of durable state — never of model output. |
| P2 | The model cannot select, influence, or annotate a risk class. |
| P3 | There is no permissive fallback; unmatched rules require approval. |
| P4 | The task's `policy_class` is a hard ceiling. |
| P5 | Proactive/automated context permits only `OBSERVE` and `LOCAL_STATE`. |
| P6 | `DESTRUCTIVE` requires explicit enablement *and* approval. |
| P7 | Root operations always require approval; no rule can auto-grant them, not even an `ALLOW`. |
| P8 | Registry enabled/removal overlay is checked for new binding and rechecked before P8 dispatch; policy rules cannot re-enable it. |
| P9 | Policy changes are audited and unreachable from model or device input. |
| P10 | `AutomationContext` is derived from the Task kind and trusted provenance only, never from a caller or from model output. |
| P11 | An explicit `DENY` overrides every `ALLOW` regardless of priority. |

## 10. Changelog

- 2026-10-10: owner ratified the P6 policy semantics. §3.3 names the owning
  crate for the internal types; §4.1.1 freezes the class-default table as a code
  constant; §4.2 replaces "most specific first" with the family order and states
  the two supremacy rules; §4.4 publishes the finite `AutomationContext`
  derivation function and its hard restrictions; §5.1 replaces the obsolete glob
  and array `match` examples with the closed dimension set; §5.2 states that
  `priority DESC, rule_id ASC` is the complete winner rule; §7.1–§7.3 record
  SQLite as the sole runtime authority, revision identity, activation and the
  evaluation-against-one-revision rule; §8 and §9 add the corresponding
  obligations and invariants. The `serea.policy/1` surface semantics are
  unchanged in kind; no wire-major change.
