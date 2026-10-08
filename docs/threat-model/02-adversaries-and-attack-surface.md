# Serea Threat Model: Adversaries and Attack Surface

Part of `serea-tm/0.1.0` · Threat model version `serea-tm/0.1.0` · Written against `serea-arch/0.1.0` (protocols frozen 2026-10-01)

Index: [Threat model index](README.md) · Previous:
[01 — Assets and trust boundaries](01-assets-and-trust-boundaries.md) · Next:
[03 — Abuse cases and mitigations](03-abuse-cases-and-mitigations.md)

---

## 0. Core principle

> Name the adversary before designing the defence. "Prompt injection" is not an
> adversary; "a stranger who can send the user an email and knows they use a
> calendar assistant" is.

Every entry below is stated as a *capability* and a *goal*, because a threat
model written in terms of techniques goes stale the moment the technique
changes, while one written in terms of reach does not.

---

## 1. Adversary catalogue

| ID | Description | Capability | Motivation | Goal |
| --- | --- | --- | --- | --- |
| ADV-1 | Malicious or compromised cloud model provider, including compromised or substituted model weights | Full control of `ModelResponse.content` and `.structured` for any model in the roster; knows the prompt format, the schemas, and which fields the host treats as host-resolved. Cannot see anything Serea refuses to send ([DC1](../protocols/09-data-classification-protocol.md#9-invariants-summary)). Cannot cause an effect directly | Profit, espionage, ideological, or pure misconfiguration of the vendor's serving stack | Cause an action Serea would not have taken, or exfiltrate content Serea refused to send |
| ADV-2 | Malicious content author (email sender, calendar inviter, page operator, app posting a notification) | Places arbitrary attacker-chosen text into content Serea ingests on TB-11; notification-listener content remains on-device unless the user explicitly selects and submits it | Phishing, fraud, data theft, or spreading instructions to a bot | Steer the assistant into proposing an action, or into disclosing content to the user, or into storing a false durable fact |
| ADV-3 | Attacker with physical possession of an unlocked, unverified Android device | A live paired session, the ability to read the screen, to tap, and to observe prompts; no device private key extraction, no biometric material | Theft, resale, or casual snooping | Approve an action the user never saw, or read `PRIVATE` cached content, or leave a persistent grant |
| ADV-4 | Malicious app on the Android device | Arbitrary intents, accessibility services, notification-listener access, and whatever the Serea manifest happens to export | Data theft, ad fraud, or establishing a foothold on a personal device | Reach Serea's components directly, redirect an intent into a privileged Serea handler, or read Serea's notifications and cache |
| ADV-5 | Network attacker between device and Mac | Sees and modifies traffic on the path; cannot break TLS, cannot forge the pinned host fingerprint, cannot forge a per-frame device signature | Opportunistic interception, hostile Wi-Fi, ISP curiosity | Read content in flight, delay or reorder frames, or use a stolen session token — noting the session itself is *not* a bearer credential here |
| ADV-6 | Compromised local user account on the Mac | Reads Serea's data directory, processes, event log, and database; writes to files the user can write | Curiosity, espionage, or staging | Read task, memory, and policy state; edit the policy ruleset or the approval ledger; capture secret material out of memory |
| ADV-7 | Supply-chain or dependency attacker | Controls what is built or fetched: crates, build scripts, model files, Android dependencies, or a transitive package | Monetisation, state access, or destructive intent | Introduce code that runs inside the host process, exfiltrate through a provider adapter, or weaken a validation path |
| ADV-8 | Insider with read access to the Mac but not the credential store | Reads events, digests, redacted projections, task state, and the policy ruleset; cannot read Keychain items | Legitimate curiosity or covert collection | Learn the user's schedule, contacts, and habits from the audit trail and task store without touching a credential |
| ADV-9 | GoalLatch misbehaving or compromised (future) | Once a real adapter exists: control over goal execution on the host machine, over the `GoalSummary`, artifacts, and the truth of `GOAL_RESULT` evidence | Local compromise, a rogue build, or a future GoalLatch defect | Report success for work that did not happen, or influence Serea's planning by the contents of a goal summary |
| ADV-10 | Replayed or stale provider data | Supplies content that was true earlier: a revoked event, an already-deleted thread, a cached page, a token that was rotated | Not malicious — this is an ordinary systems failure that an attacker can also induce deliberately | Cause Serea to act on facts that are no longer true, in particular to create or re-create something the user already removed |
| ADV-11 | Compromised provider-side account | Full API access to the user's Gmail or Calendar as the user, from the provider's side | Account takeover, prior credential theft | Change the very data Serea reads, so that every downstream decision is made on attacker-authored input |

### 1.1 Adversaries that are *not* in the catalogue

| Not modelled | Why |
| --- | --- |
| The honest user doing something careless | Deliberately excluded. Careless approval is a UX problem, not an adversary; [Approval Protocol §7](../protocols/05-approval-protocol.md#7-approval-fatigue) owns that trade-off, and this model must not be used to justify making approval impossible |
| A second human sharing the Mac | Serea is a single-user system. Household and multi-user threat models are out of scope at P0 |
| Serea's own developers acting maliciously | A code-review and separation-of-duties problem. Supply-chain compromise (ADV-7) is the closest modelled analogue |
| A hostile Ollama Cloud **vendor employee** with infrastructure access | Folded into ADV-1. The distinction does not change any control, because the only defence against provider compromise is not trusting the provider |

### 1.2 Capability ordering

Ordered by how much each adversary can reach, which is the order that matters
for prioritising mitigations:

```
ADV-1  ADV-2  -> reach Serea's decision surface
ADV-4  ADV-11 -> reach the platform or a provider account
ADV-3        -> reach an authenticated session
ADV-6  ADV-8 -> reach the host's durable state
ADV-5        -> reach the transport only
ADV-10       -> reach the truth of the inputs
ADV-9        -> reach a future, gated seam
ADV-7        -> reach the build
```

The uncomfortable conclusion this ordering forces: **ADV-2 has almost no
capability and produces most of the realistic incidents.** Defending against it
cannot mean defending the model; it can only mean denying authority everywhere
else, which is exactly what the pipeline does.

---

## 2. Attack surface enumeration

Eleven surfaces. Each states the entry point, what the attacker controls, what
the host must guarantee, and which protocol invariant provides that guarantee.
Invariant prefixes map to documents as given in
[03 — Abuse cases §2](03-abuse-cases-and-mitigations.md#2-invariant-prefixes).

### AS-1 — Model output ingestion

| Field | Content |
| --- | --- |
| **Entry point** | `ModelResponse.structured` arriving from `ModelProvider::generate`, and the text rendered into chat as `content` |
| **Attacker controls** | Every byte of the response, if ADV-1. The shape, the extra fields, the claimed capability ids, any fabricated receipt or completion claim, any urgency or risk annotation |
| **Host must guarantee** | Output is a *proposal*, not an instruction. Validation is host code against a host schema, fail-closed, bounded, isolated, and ending in explicit failure. No prose is parsed for authority. Repair never receives the conversation or tool definitions. Nothing degrades to a more privileged model |
| **Protocol + invariants** | [Model Protocol §4.1](../protocols/03-model-protocol.md#41-the-trust-boundary-stated-precisely), [§7.2](../protocols/03-model-protocol.md#72-hard-constraints-on-repair) — `M1`, `M3`, `M4`, `M9`; [Capability Protocol §4.2](../protocols/01-capability-protocol.md#42-host-resolved-fields) — `C1`, `C3`; [Policy Protocol §1](../protocols/04-policy-protocol.md#1-core-principle) — `P2` |

### AS-2 — Capability invocation

| Field | Content |
| --- | --- |
| **Entry point** | `CapabilityProvider::invoke` receiving a validated `ActionRequest` |
| **Attacker controls** | Argument values, if they flow from model output or retrieved content. Capability choice, within what the task's `policy_class` permits |
| **Host must guarantee** | Registry is a closed world; descriptors are immutable for a task's lifetime; `risk_class` is host-owned; a provider receives a `CredentialHandle` and no ambient authority; a provider that cannot honour its descriptor degrades to `UNAVAILABLE` rather than returning something laxer; a provider cannot reach another provider's credentials |
| **Protocol + invariants** | [Capability Protocol §9](../protocols/01-capability-protocol.md#9-provider-interface), [§10](../protocols/01-capability-protocol.md#10-capability-registry-and-p5p6p8-boundary), [§3.2](../protocols/01-capability-protocol.md#32-invariants) — `C1`, `C3`, `C7`, `C8`, `C10`; [Policy Protocol §4.2](../protocols/04-policy-protocol.md#42-evaluation-order) — `P3` |

### AS-3 — Approval prompt rendering and response

| Field | Content |
| --- | --- |
| **Entry point** | `ApprovalRequest` rendered on the device; `APPROVAL_RESPONSE` arriving back over TB-1 |
| **Attacker controls** | The *argument values* that appear in `plain_summary` and `arguments_preview`, because those values often came from retrieved content (ADV-2). Also the physical tap, if ADV-3 |
| **Host must guarantee** | `plain_summary` is written by host code from validated arguments and is specific enough to consent to alone; `max_uses` and `expires_at` in a device response are clamped to the request's own bounds; elevated-device approval requires on-device biometric confirmation with no fallback path; a response for a non-`PENDING` request is dropped; no notification action can approve anything |
| **Protocol + invariants** | [Approval Protocol §2.1](../protocols/05-approval-protocol.md#21-the-prompt-must-be-specific-enough-to-consent-to), [§5](../protocols/05-approval-protocol.md#5-device-bound-approval) — `A1`, `A2`, `A3`, `A6`, `A7`; [Device Protocol §5.2](../protocols/07-device-protocol.md#52-approval_response), [§7](../protocols/07-device-protocol.md#7-notifications) — `D10`, `D12` |

### AS-4 — Device link: pairing, session, reconnection

| Field | Content |
| --- | --- |
| **Entry point** | QR scan and `pairing/exchange`; the mutual-authentication handshake; session resumption; `RECONNECT` |
| **Attacker controls** | Timing and observation of the pairing payload (90-second life), the `pairing_nonce` if photographed in time, connection cadence, frame replay, and every byte of payload after decryption if the transport is broken |
| **Host must guarantee** | The device dials out and the host never dials a device; the host fingerprint is pinned and a change is a hard failure; pairing is single-use and every device after the first requires host-side human confirmation; the nonce is destroyed on both sides; every frame is signed and verified; replayed `message_id`s are dropped; a replaced device key fails with `PAIRING_KEY_MISMATCH`; revocation is host-side and a burned `DeviceId` is never reissued |
| **Protocol + invariants** | [Device Protocol §2.1](../protocols/07-device-protocol.md#21-dial-out-is-mandatory), [§3](../protocols/07-device-protocol.md#3-pairing), [§4](../protocols/07-device-protocol.md#4-sessions-and-authentication), [§2.5](../protocols/07-device-protocol.md#25-delivery-semantics) — `D1`, `D3`, `D4`, `D5`, `D7`, `D8` |

### AS-5 — Event stream

| Field | Content |
| --- | --- |
| **Entry point** | `TIMELINE_PAGE_REQUEST` and the rendered page; the host-side append-only log |
| **Attacker controls** | Paging cursor values, filter parameters, and — if ADV-6 or ADV-7 — the log contents themselves |
| **Host must guarantee** | `seq` is gapless and monotonic, so a gap is detectable and surfaced rather than smoothed over; events are append-only; unknown kinds are skipped, never fatal; `data_class` is enforced *before* the event crosses the link; a receipt event's absence means the effect is unverified |
| **Protocol + invariants** | [Event Protocol §5](../protocols/06-event-protocol.md#5-ordering-and-delivery), [§6](../protocols/06-event-protocol.md#6-the-activity-timeline), [Device Protocol §8](../protocols/07-device-protocol.md#8-activity-timeline-feed) — `E1`, `E2`, `E3`, `E4`, `E7`, `E8`, `E9`, `D11` |

### AS-6 — Provider adapters: Gmail, Calendar, GitHub, web, Android

| Field | Content |
| --- | --- |
| **Entry point** | OAuth-scoped API responses; fetched web page bodies; repository status payloads; Android notification-listener observations (device-local by default), user-selected notification text explicitly submitted to Core, and Serea-posted host-event notifications (outbound) |
| **Attacker controls** | Content of everything returned, if ADV-2 or ADV-11, including third-party notification text. A listener observation does not reach Core unless the user explicitly selects and submits it; that submission remains untrusted and `PRIVATE`. Response size, redirect targets, rate limits, and availability |
| **Host must guarantee** | Returned content is classified `PRIVATE` by origin and stays in the provider cache; promotion to memory requires an explicit `EXTRACTION` step with provenance; Android notification-listener text is local-only by default, and selected text enters Core only through an explicit user-mediated submission as untrusted `PRIVATE` input; Serea-posted notification text derived from host events obeys the data-egress/redaction rules; a history-id failure degrades to `FULL_RESYNC` rather than fabricating data; a capability absent from a device's report is unavailable, not "probably supported"; the report grants nothing |
| **Protocol + invariants** | [Data Classification Protocol §7](../protocols/09-data-classification-protocol.md#7-classification-of-provider-data) — `DC11`, `DC3`; [Device Protocol §6](../protocols/07-device-protocol.md#6-device-capability-reporting) — `D9`; [Capability Protocol §9](../protocols/01-capability-protocol.md#9-provider-interface) — `C7` |

### AS-7 — Scheduler and wake sources

| Field | Content |
| --- | --- |
| **Entry point** | Durable schedule firing; proactive watcher candidate generation; recovery and lease reclamation on startup |
| **Attacker controls** | Cadence and volume of external events that generate candidates, if ADV-2 or ADV-11; the *content* of a candidate, indirectly |
| **Host must guarantee** | Automation context permits only `OBSERVE` and `LOCAL_STATE`; anything else denies with `AUTOMATED_ACTION_FORBIDDEN`; the watcher raises no approvals at all; proposal volume is bounded per calendar day and pausing is announced |
| **Protocol + invariants** | [Policy Protocol §4.3](../protocols/04-policy-protocol.md#43-additional-standing-rules), [§4.2](../protocols/04-policy-protocol.md#42-evaluation-order) rule 4 — `P5`; [Approval Protocol §7](../protocols/05-approval-protocol.md#7-approval-fatigue) — `A10`; [Bounds Protocol §2](../protocols/10-bounds-protocol.md#2-the-bound-set) — `B16` |

### AS-8 — Memory ingestion and retrieval

| Field | Content |
| --- | --- |
| **Entry point** | `EXTRACTION` model call producing a memory item; later retrieval of items into a prompt |
| **Attacker controls** | Source content, if ADV-2 or ADV-11 — a hostile email body is the cheapest memory-poisoning primitive available, because the extraction model is doing exactly what it was asked |
| **Host must guarantee** | Only an `EXTRACTION`-purpose call can create an item; the extraction sees the redacted projection, never the raw body; stored class is inherited from the source; provenance is mandatory, immutable, and user-visible; deletion cascades and writes a tombstone; external content does not become ambient model context by default |
| **Protocol + invariants** | [Data Classification Protocol §7.1](../protocols/09-data-classification-protocol.md#71-the-extraction-step), [§8](../protocols/09-data-classification-protocol.md#8-right-to-delete-and-provenance) — `DC11`, `DC3`, `DC12`; [Task Protocol §8](../protocols/02-task-protocol.md#8-task-retention-and-privacy) — `T10` |

### AS-9 — Root provider

| Field | Content |
| --- | --- |
| **Entry point** | A capability whose descriptor carries `root_requirement: OPTIONAL_ROOT` or `REQUIRES_ROOT` |
| **Attacker controls** | Nothing, directly: root capabilities are host-defined and host-registered, and there is no `su` shell at any risk class. An attacker influences only whether a root operation is *requested*, via model output or content |
| **Host must guarantee** | Root operations always require approval and no rule can auto-grant one; the root and rootless variants share one `CapabilityId` and differ only by `implementation_id`; root absence yields `CAPABILITY_UNAVAILABLE`; root must not bypass biometric confirmation, secure screens, device policy, or platform safety controls; Serea stays substantially functional without root |
| **Protocol + invariants** | [Policy Protocol §4.3](../protocols/04-policy-protocol.md#43-additional-standing-rules) — `P7`; [Capability Protocol §3.1](../protocols/01-capability-protocol.md#31-field-semantics) — `C2`, `C9`; [Approval Protocol §5](../protocols/05-approval-protocol.md#5-device-bound-approval) — `A7` |

### AS-10 — GoalLatch adapter (future)

| Field | Content |
| --- | --- |
| **Entry point** | `host.goal.start`, `.run`, `.status`, `.cancel`, `.result` — the only five capabilities, through the registry shim |
| **Attacker controls** | At P0, no GoalLatch provider is available. P15 plans an offline fake with no network, filesystem, or subprocess access. A future real adapter, only after readiness checks and separate authorization, will face an ADV-9 actor controlling goal execution and every field the adapter reports |
| **Host must guarantee** | The delegation crosses the full normal path with no bypass; no `local_mcp::*` path or GoalLatch type appears in Serea's signatures; `GoalHandle` is opaque and never parsed; `GOAL_RESULT` evidence is host-observed and model-originated evidence is invalid; `COMPLETED` requires a schema-valid `host.goal.result`, never a model claim; exactly one implementation is registered at a time |
| **Protocol + invariants** | [GoalLatch Adapter Protocol §2](../protocols/08-goallatch-adapter-protocol.md#2-forbidden-couplings), [§5](../protocols/08-goallatch-adapter-protocol.md#5-delegation-model), [§7](../protocols/08-goallatch-adapter-protocol.md#7-result-and-evidence-contract) — `G1`, `G2`, `G3`, `G5`, `G7`, `G8`, `G9`, `G10`, `G13` |

### AS-11 — Durable storage

| Field | Content |
| --- | --- |
| **Entry point** | SQLite, the argument blob store, the sealed store, and the event log on the Mac's filesystem |
| **Attacker controls** | Contents, if ADV-6, ADV-7, or an attacker with local file write. Also indirectly, via any capability that produces attacker-shaped values which are then persisted |
| **Host must guarantee** | A step's success and receipt persist before the task advances; event and state change commit in one transaction; the event log is append-only; the sealed store is the only place `SECRET` may live; credentials live nowhere in Serea's storage; the registry persists so an in-flight step survives restart |
| **Protocol + invariants** | [Task Protocol §5](../protocols/02-task-protocol.md#5-execution-rules), [§6](../protocols/02-task-protocol.md#6-recovery) — `T4`, `T5`; [Event Protocol §5](../protocols/06-event-protocol.md#5-ordering-and-delivery) — `E2`, `E3`; [Data Classification Protocol §5](../protocols/09-data-classification-protocol.md#5-egress-rules) — `DC5`, `DC7` |

---

## 3. Surfaces with no protocol contract behind them

Three surfaces are enumerated above because they exist, but the frozen protocol
set does not specify a control for them. These are the highest-value findings in
the package and they are repeated in
[03 — Abuse cases §5](03-abuse-cases-and-mitigations.md#5-summary-of-gaps).

| Surface | What is missing | Consequence |
| --- | --- | --- |
| The Android manifest and exported components | No protocol document freezes `exported`, permission attributes, intent filters, or the rule that a Serea component must not re-dispatch an intent's extras | A malicious app on the device has no contractual obstacle to reaching Serea's components |
| Byte-size and object-count bounds on ingested content | `B3` declares the bound table complete, and it contains no payload-byte or attachment-size bound | A single oversized attachment or page has no frozen limit on memory, disk, or parse time |
| Integrity sealing of host authority state | `E2` makes events append-only *by convention of the writer*; no MAC, hash chain, or signature covers the policy ruleset or the approval ledger | An attacker with local write access can change what is permitted and what was approved |

---

## 4. Explicitly out of scope for P0

Each entry states what is excluded and why excluding it is defensible rather
than merely convenient.

| Excluded | Why |
| --- | --- |
| **Real GoalLatch integration internals** | No real adapter exists and none may be written until all six readiness verifications pass ([GoalLatch Adapter Protocol §9](../protocols/08-goallatch-adapter-protocol.md#9-real-adapter-readiness-gate)); P15 covers the fake adapter journey only; any real adapter would require a separate future-phase authorization after P16. This package analyses the *seam*, which is being built now, and the threat that a future adapter reports untruthfully (`AB-21`) |
| **The separate local-MCP repository's own internals** | A separate project with its own threat model. Serea must not read it, depend on it, or share a database with it ([G3](../protocols/08-goallatch-adapter-protocol.md#11-invariants-summary)); a threat model that needed to inspect it would describe a coupling the protocols forbid |
| **Compromise of the Android OS, TEE, or hardware** | Out of any application's reach. The relevant assumption is narrower and stated: the Keystore gate holds, the sandbox holds, and `BiometricPrompt` is honest |
| **Physical attack of the Mac** — cold boot, DMA, memory-scraping of a running process | Requires a different threat class and different hardware assumptions. The in-scope consequence is narrower: secret bytes must not linger in freed memory, which `Secret<T>` and `ZeroizeOnDrop` address ([DC7](../protocols/09-data-classification-protocol.md#9-invariants-summary)) |
| **Denial of service against the model vendor by an unrelated third party** | Not caused by Serea and not mitigable by it. Serea's obligation is to fail *predictably* when the provider is degraded, which `ModelCapabilities.health` routing and the explicit budget-failure rule already cover ([M6](../protocols/03-model-protocol.md#11-invariants-summary)) |
| **Attacks on the cryptographic primitives** | Ed25519, SHA-256, TLS, HKDF, and JSON Schema 2020-12 are treated as sound. Their *misuse* — pinning, key rotation, canonicalisation, fail-closed parsing — is in scope and is covered at AS-4, AS-5, and AS-11 |
| **A global certificate authority compromise** | Would break the transport authentication model entirely. Assumed not to occur; the consequence, if it did, is a full re-pairing rather than a subtle bypass |
| **Multi-user and household threat models** | Serea is a single-user system at P0. A second Mac user, a family member's phone, and shared-device usage are not modelled |
| **Metadata and traffic analysis** | Serea's egress patterns are not an adversary's target at P0. Content confidentiality is in scope; traffic-shape confidentiality is not |
| **Social engineering the user into approving something harmful** | Not excluded from the abuse cases — `AB-29` and `AB-12` both sit here — but excluded as a *primary* framing. The design accepts that a user may approve a bad action; it does not accept that the user may be shown a false summary of what they are approving |

---

## 5. How to use this document

When adding a component, answer three questions in order:

1. **Which surface does it join?** Every new component joins one of AS-1 to
   AS-11, or it is a new surface and this document needs a new entry.
2. **Which adversary now reaches it?** Use the reach order in §1.2. If the
   answer is "a new one", the component needs a boundary in
   [01 — Trust boundaries §2](01-assets-and-trust-boundaries.md#2-trust-boundaries).
3. **Which invariant now carries more weight?** The load-bearing invariant is
   almost always the one closest to the new surface — `C1` at the registry, `A1`
   at approval, `DC1` at egress, `B8` at the loop. Adding to one surface
   generally *increases* the importance of that invariant without changing it.

Index: [Threat model index](README.md) · Previous:
[01 — Assets and trust boundaries](01-assets-and-trust-boundaries.md) · Next:
[03 — Abuse cases and mitigations](03-abuse-cases-and-mitigations.md)
