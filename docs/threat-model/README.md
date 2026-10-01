# Serea Threat Model

Status: **P0 threat model** · Threat model version `serea-tm/0.1.0` · Written against architecture version `serea-arch/0.1.0` (protocols frozen 2026-10-01)

This package answers one question: **what could break the guarantees Serea's
protocol documents make, and what stops it?** It is written against the frozen
protocol set and is itself subordinate to it. Where the two disagree, the
protocol wins and this document is the thing that is wrong.

---

## 1. Core principle

> Serea's threat model is not "a model might be wrong." Serea's threat model is
> "every component is assumed to be adversarial except the host's own decisions,
> and the host must reach an effect without asking any of them for permission."

Everything below follows from the same asymmetry. The model, the retrieved
content, the phone, the network, the providers, the GoalLatch adapter, and the
user's own software may all be wrong, coerced, or hostile simultaneously. The
only component trusted with authority is the host's own deterministic pipeline,
and only for the stages
[Capability Protocol §1](../protocols/01-capability-protocol.md#1-core-principle)
pins it to.

---

## 2. Purpose

The protocol documents state what Serea guarantees. They are written from the
inside out, as contracts. They are not, and cannot be, an argument about *why*
a guarantee is needed or about what an adversary actually tries.

This package supplies the missing half:

| Question | Answered by |
| --- | --- |
| What must not be damaged, disclosed, or forged? | [Assets and trust boundaries](01-assets-and-trust-boundaries.md) |
| Who wants it damaged, disclosed, or forged, and what can they reach? | [Adversaries and attack surface](02-adversaries-and-attack-surface.md) |
| What specifically goes wrong, and what stops it? | [Abuse cases and mitigations](03-abuse-cases-and-mitigations.md) |
| Which guarantees are load-bearing, and how is each one proven? | [Security invariants](04-security-invariants.md) |

---

## 3. Scope

### 3.1 In scope for P0

- The full host pipeline from model output to durable state: schema validation,
  registry, policy, approval, provider, receipt, persistence.
- Untrusted **retrieved content** — email bodies, calendar invites and
  descriptions, web pages, notification text — as an injection carrier.
- The Mac↔Android link: pairing, sessions, approval transport, the event stream.
- The Android app's own attack surface: exported components, intent handling,
  keystore and biometric use, notification rendering.
- Host-owned state that an attacker would want to edit rather than use:
  the policy ruleset, the approval ledger, the task store, the memory store, the
  pairing roster, durable storage generally.
- The optional root provider and the explicit rootless-first requirement.
- Bounds as a security control (blast radius), not merely as a cost control.
- The GoalLatch delegation seam as a contract; P0 does not implement a provider.
  The deterministic offline fake is planned for P15 and will inherit this seam.

### 3.2 Out of scope for P0

Summarised here, argued in
[§4 of the adversary document](02-adversaries-and-attack-surface.md#4-explicitly-out-of-scope-for-p0):
real GoalLatch integration internals (not scheduled; any future phase requires P16 readiness closure and separate authorization), Android
OS/TPM/Keystore compromise, physical attack of the Mac, denial of service
against the model vendor by an unrelated third party, and the cryptographic
primitives themselves. Section references use this package's own headings.

---

## 4. How to read this package

Four documents, intended to be read in order, though a reader with a specific
concern can jump:

1. **[01 — Assets and trust boundaries](01-assets-and-trust-boundaries.md)** —
   the noun list and the boundary list. Read this if you need to know what
   "the credential store" or "the approval ledger" means, or where a trust
   boundary sits.
2. **[02 — Adversaries and attack surface](02-adversaries-and-attack-surface.md)** —
   the threat actors and the eleven entry points. Read this if you are adding a
   capability, a provider, or a transport and want to know which adversary you
   have just exposed yourself to.
3. **[03 — Abuse cases and mitigations](03-abuse-cases-and-mitigations.md)** —
   the core document. Thirty concrete abuse cases in a consistent card format,
   each naming the mechanism that stops it and the residual risk that remains.
4. **[04 — Security invariants](04-security-invariants.md)** — the normative
   cross-reference. Read this before changing any protocol invariant, and read
   it when you need to know which test proves a guarantee.

### 4.1 Deferral vocabulary

This package is honest about gaps. Three deferral classes are used, and only
these three:

| Class | Meaning |
| --- | --- |
| **Deferred to P0** | The invariant is already frozen in a protocol document; only the implementation is outstanding. This is ordinary build work, not a design gap. |
| **Deferred beyond P16** | The mitigation cannot exist until the real GoalLatch adapter does, and the adapter may not be written until the readiness gate passes ([GoalLatch Adapter Protocol §9](../protocols/08-goallatch-adapter-protocol.md#9-real-adapter-readiness-gate)). |
| **No protocol basis at P0** | The frozen protocol set does not contain the control the abuse case needs. Closing it requires an ADR under [Protocol Index §7](../protocols/00-protocol-index.md#7-change-control) — and, where the fix is a new bound, an update to the bound set, which
  [B3](../protocols/10-bounds-protocol.md#10-invariants-summary) says must be complete. These are the findings that matter most. |

No other phase numbers are asserted here. Phase numbering for subsystems beyond
P0, P15, and P16 is owned by the phase plan and is deliberately not
second-guessed from this package.

### 4.2 Test naming

Protocol documents refer to "named tests in the phase plans". This package
assigns its own obligation identifiers, `SEC-CHK-nn`, defined in
[04 — Security invariants §5](04-security-invariants.md#5-verification-catalogue).
Those are threat-model-local names; the plan documents own the phase test IDs
they map onto. Where a check has no such mapping yet, that is recorded rather
than assumed.

---

## 5. Asset categories at a glance

The full table, with per-asset confidentiality and integrity needs, is in
[01 — Assets §1](01-assets-and-trust-boundaries.md#1-assets).

| Category | Representative assets | Worst credible outcome | Primary control stack |
| --- | --- | --- | --- |
| User personal data | Email bodies, calendar bodies, notification text | Disclosure to a third party or a cloud model | `DataClass` egress matrix, redaction, `DC1`–`DC13` |
| Authentication material | OAuth refresh tokens, device private keys, pairing nonces, model provider API keys | Impersonation of the user, of the phone, or of the Serea host | `CREDENTIAL` reaches only the OS store; `C8`, `DC5`, `DC6`, `DC7` |
| Host authority state | Policy ruleset, approval ledger, capability registry, disabled overlay | Silent permission widening; forged or replayed authority | `P1`–`P9`, `A1`–`A10`, `C10`, admin plane + audit |
| Durable work state | Task store, step records, receipts, leases | Lost, duplicated, or fabricated effects | `T1`–`T10`, `C4`–`C6`, `B10`, `E3` |
| Derived knowledge | Memory store and its provenance | A durable attacker-authored "fact" that steers later actions | `DC11`–`DC13`, explicit `EXTRACTION`, provenance |
| Derived volatile state | Model prompts and completions, notification bodies, chat deltas | Covert exfiltration channel; user-visible deception | `E8`, `D10`, redaction before egress, receipts for claims |
| Delegated results | `host.goal.*` outputs, artifacts, evidence | Serea reports local code work that never happened | `G1`–`G13`, host-observed evidence, read-only completion path |
| Audit record | Event log, `model_usage`, `POLICY_CHANGED` history | Loss of accountability after an incident | `E1`–`E10`, `B16`, append-only, retention classes |

---

## 6. Top risks

Ranked by the combination of consequence and the weakness of the current
mitigation set, not by likelihood alone. Each links to its abuse card.

| # | Risk | Why it ranks here | Card |
| --- | --- | --- | --- |
| 1 | **Prompt injection from retrieved content** (email, calendar, web, notification) | The attacker does not need any Serea weakness at all — ordinary content delivery suffices. Serea cannot prevent the model from being steered; it can only prevent the steered model from obtaining authority. The residual is social-engineering the *approver*, not the pipeline. | [AB-01](03-abuse-cases-and-mitigations.md#ab-01-prompt-injection-via-an-email-body) – [AB-04](03-abuse-cases-and-mitigations.md#ab-04-prompt-injection-via-android-notification-text) |
| 2 | **Model-reported completion of work that did not happen** | Every downstream guarantee is receipt-based, but nothing stops the model from *telling the user* it is done. The state is safe; the user's belief is not. No protocol clause closes this. | [AB-05](03-abuse-cases-and-mitigations.md#ab-05-model-fabricates-completion-of-an-action) |
| 3 | **Ambiguous external effect after a network drop** | The single highest-severity anti-pattern named in [Capability Protocol §6.2](../protocols/01-capability-protocol.md#62-the-ambiguous-rule). For several real capabilities there is no safe read-back, so the only correct outcome — `BLOCKED` — is also the one users will find most annoying, making it the likeliest thing to be "fixed" later. | [AB-09](03-abuse-cases-and-mitigations.md#ab-09-ambiguous-effect-after-a-dropped-connection) |
| 4 | **Unattended unlocked phone approving non-elevated actions** | Biometric confirmation is required only for `ELEVATED_DEVICE`. Calendar creation and mail sending are approvable on a bare unlocked session. | [AB-12](03-abuse-cases-and-mitigations.md#ab-12-stolen-or-unattended-device-session-authorizes-an-action) |
| 5 | **Tampering with host authority state** | The policy ruleset and the approval ledger have strong *semantic* controls and **no specified integrity seal**. An attacker with write access to the host database can change what is permitted. | [AB-15](03-abuse-cases-and-mitigations.md#ab-15-tampering-with-the-policy-ruleset), [AB-16](03-abuse-cases-and-mitigations.md#ab-16-forging-or-replaying-an-approval-grant) |
| 6 | **Exported Android components and intent redirection** | No protocol document freezes the manifest surface. This is the one major attack surface in the system with *no* contract behind it at all. | [AB-14](03-abuse-cases-and-mitigations.md#ab-14-malicious-device-app-abusing-an-exported-device-component) |
| 7 | **Bound set incompleteness for payload size and egress rate** | `B3` states the bound table is complete, yet it contains no payload-byte bound, no attachment-size bound, and no provider-quota bound. Three distinct denial-of-service and cost paths therefore have no frozen limit. | [AB-20](03-abuse-cases-and-mitigations.md#ab-20-denial-of-service-by-a-huge-attachment-or-notification-flood), [AB-25](03-abuse-cases-and-mitigations.md#ab-25-cost-exhaustion-and-provider-quota-burn) |
| 8 | **Attacker-authored text inside the approval preview** | `plain_summary` is host-*written* but renders host-*uncontrolled* strings from argument values. The consent UI is the last place an attacker can speak to the user directly. | [AB-29](03-abuse-cases-and-mitigations.md#ab-29-attacker-controlled-text-inside-the-approval-preview) |

---

## 7. Relationship to `docs/protocols/`

The two sets have a strict division of labour and neither may restate the other.

- **Protocols state what the system guarantees.** They are normative,
  machine-checkable where possible, versioned, and frozen. Changing one requires
  an ADR ([Protocol Index §7](../protocols/00-protocol-index.md#7-change-control)).
- **The threat model states what could break those guarantees, who would try,
  and what mitigates them.** It is descriptive and analytic. It may not introduce
  a type, an enum, a bound, or an invariant that the protocols do not already
  name — where a control is missing, this package says so and marks the gap
  rather than inventing the control here.

Consequences worth being explicit about:

1. Every mitigation in this package cites a protocol invariant by its own ID
   (`C1`, `T4`, `M5`, `P7`, `A3`, `E3`, `D6`, `G8`, `DC1`, `B16` and so on).
   The prefix-to-document mapping is given in
   [03 — Abuse cases §2](03-abuse-cases-and-mitigations.md#2-invariant-prefixes)
   and repeated normatively in
   [04 — Security invariants §2](04-security-invariants.md#2-normative-cross-reference).
2. An abuse case that **cannot** cite an invariant is not a weak card — it is a
   finding. The cases that land in the "No protocol basis at P0" class are
   collected in [03 — Abuse cases §5](03-abuse-cases-and-mitigations.md#5-summary-of-gaps)
3. If a protocol invariant is later relaxed, the abuse cases that depended on it
   do not become invalid; they become findings, and they are already written.

## 8. Relationship to `docs/architecture/`, `docs/decisions/`, `docs/plans/`

| Directory | Relationship |
| --- | --- |
| [`docs/architecture/`](../architecture/) | Describes how the components are built and arranged. This package assumes that structure and analyses it. Where the architecture and this package disagree about a boundary, the architecture document wins and this package is corrected. |
| [`docs/decisions/`](../decisions/) | Records *why* a control exists. Every "No protocol basis at P0" finding in [03](../threat-model/03-abuse-cases-and-mitigations.md) is a candidate ADR, and several existing ADRs — notably the read-only proactive watcher rule cited by [P5](../protocols/04-policy-protocol.md#9-invariants-summary) — are already load-bearing mitigations here. |
| [`docs/plans/`](../plans/) | Owns phase ordering and the real test identifiers. This package uses the deferral classes in §4.1 and the `SEC-CHK-nn` names in §4.2 and does not assert a phase plan of its own. |

## 9. Change control

This package is not frozen, because a threat model that cannot record a newly
found gap is worse than none. It changes by:

1. Adding or amending an abuse card, a deferral class, or a `SEC-CHK-nn` check —
   editorial, no ADR needed.
2. Promoting a deferral into an enforced mitigation — requires the ADR that the
   corresponding protocol change requires, plus a changelog entry in that
   protocol.
3. Changing the architecture version this package is written against — a full
   re-read of the protocols, because the entire premise is that these documents
   are consistent.

A threat-model revision that adds no new finding is suspicious; it usually means
the protocols changed and this package was not re-read.