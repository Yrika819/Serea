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

### 4.2 Evaluation order

Rules are evaluated in a fixed, documented order, **most specific first**:

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

### 4.3 Additional standing rules

- **Proactive watcher is read-only.** In automation context, only `OBSERVE`
  and `LOCAL_STATE` may reach `Allow`. Everything else denies. See
  [ADR-0016](../decisions/ADR-0016-proactive-watcher-is-read-only.md).
- **`DESTRUCTIVE` defaults to disabled.** A destructive capability must be
  explicitly enabled by the user *and* still requires approval. Both, not
  either.
- **Root requires approval, always.** There is no rule that can grant a root
  operation automatically.
- **No "allow everything" rule.** A policy rule cannot express unbounded
  blanket authority. Broad authority is expressible only as a bounded
  [approval grant](05-approval-protocol.md).

## 5. Policy rules as data

```json
{
  "rule_id": "pol_0007",
  "priority": 40,
  "match": {
    "capability_id": "gmail.messages.*",
    "risk_class": ["OBSERVE"],
    "automation_context": ["INTERACTIVE", "SCHEDULED"]
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

`match` fields are all optional; an absent field matches everything. There is
no expression language, no `if/then` scripting, and no way to write a rule that
consults the model, the clock's sub-second value, or a network call.

## 6. What policy may and may not depend on

**May depend on:** durable policy rules; the capability descriptor; task state
and `policy_class`; the approval ledger; the automation context; declared data
classes; device registry state (paired, trusted, online).

**May not depend on:** model output of any kind; wall-clock time except a
coarse expiry check on grants; any network call; any randomness.

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
| Root always approval | No rule set yields `Allow` for a root capability. |

## 9. Invariants summary

| # | Invariant |
| --- | --- |
| P1 | Policy is a pure, deterministic function of durable state — never of model output. |
| P2 | The model cannot select, influence, or annotate a risk class. |
| P3 | There is no permissive fallback; unmatched rules require approval. |
| P4 | The task's `policy_class` is a hard ceiling. |
| P5 | Proactive/automated context permits only `OBSERVE` and `LOCAL_STATE`. |
| P6 | `DESTRUCTIVE` requires explicit enablement *and* approval. |
| P7 | Root operations always require approval; no rule can auto-grant them. |
| P8 | Registry enabled/removal overlay is checked for new binding and rechecked before P8 dispatch; policy rules cannot re-enable it. |
| P9 | Policy changes are audited and unreachable from model or device input. |
