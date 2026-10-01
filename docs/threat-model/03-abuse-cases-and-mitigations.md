# Serea Threat Model: Abuse Cases and Mitigations

Part of `serea-tm/0.1.0` · Written against `serea-arch/0.1.0` and the P0-frozen protocols.

Index: [Threat model index](README.md) · Previous: [02 — Adversaries and attack surface](02-adversaries-and-attack-surface.md) · Next: [04 — Security invariants](04-security-invariants.md)

This document describes attack paths and mitigations. Protocol invariants cited below are frozen design contracts; they are not claims that implementation or testing already exists. P0 freezes documentation. Enforcement belongs to the named implementation phase and is not proven until its named check passes. For the interpretation of phase bands, see [the architecture phase plan](../architecture/README.md#6-phase-plan-pointer).

## 1. Card format and status language

Each card names a STRIDE-like threat (spoofing, tampering, repudiation, information disclosure, denial of service, or elevation of privilege), the adversary and path, the affected asset, severity, host-side mitigation, exact protocol invariant IDs, residual risk, and verification obligation. **Frozen design** means the contract is specified; **not yet enforced** means implementation and evidence remain outstanding. Phase bands identify the architecture's owning implementation stages; plan documents own exact phase test IDs.

## 2. Invariant prefixes

| Prefix | Protocol source |
| --- | --- |
| `C` | [Capability Protocol §11](../protocols/01-capability-protocol.md#11-invariants-summary) |
| `T` | [Task Protocol §9](../protocols/02-task-protocol.md#9-invariants-summary) |
| `M` | [Model Protocol §11](../protocols/03-model-protocol.md#11-invariants-summary) |
| `P` | [Policy Protocol §9](../protocols/04-policy-protocol.md#9-invariants-summary) |
| `A` | [Approval Protocol §9](../protocols/05-approval-protocol.md#9-invariants-summary) |
| `E` | [Event Protocol §9](../protocols/06-event-protocol.md#9-invariants-summary) |
| `D` | [Device Protocol §10](../protocols/07-device-protocol.md#10-invariants-summary) |
| `G` | [GoalLatch Adapter Protocol §11](../protocols/08-goallatch-adapter-protocol.md#11-invariants-summary) |
| `DC` | [Data Classification Protocol §9](../protocols/09-data-classification-protocol.md#9-invariants-summary) |
| `B` | [Bounds Protocol §10](../protocols/10-bounds-protocol.md#10-invariants-summary) |

## 3. Abuse-case cards

### AB-01 — Prompt injection via an email body

- **Threat / adversary:** Elevation of privilege; `ADV-2` malicious sender or `ADV-1` model provider.
- **Attack path:** A crafted email body reaches retrieved-content ingestion (`TB-11`), steers the model to propose sending mail or changing data, and attempts to skip validation or policy.
- **Asset:** `AST-1`, `AST-14`; severity **Critical** because an unauthenticated sender can induce consequential proposals.
- **Host-side mitigation:** Classify the body `PRIVATE`, redact before prompt construction, accept model output only as a schema-validated proposal, then perform registry, task-ceiling, policy, and approval checks. Do not rely on prompt instructions to neutralize injection. **Frozen design:** `DC11`, `M1`, `C1`; enforcement **not yet present** until P4–P8 and provider integration P9–P12.
- **Protocol + invariants:** `PROTO-DATA DC2, DC3, DC11`; `PROTO-MODEL M1, M9`; `PROTO-CAP C1`; `PROTO-POLICY P1, P2`.
- **Residual risk:** Injection can produce misleading summaries or socially engineer a human approver; Serea cannot ensure a user interprets consent safely.
- **Verification:** `SEC-CHK-01`; P4–P8 fake-model injection test and P9–P12 provider-ingestion/redaction integration evidence.

### AB-02 — Prompt injection via a calendar invitation

- **Threat / adversary:** Elevation of privilege / information disclosure; `ADV-2` malicious inviter or `ADV-11` compromised provider account.
- **Attack path:** Instructions embedded in invite description or title are returned by Calendar and become model context for a scheduling task.
- **Asset:** `AST-1`, `AST-14`; **High** because external text can influence proposals and recipient/content selection.
- **Host-side mitigation:** Treat event title, location, description, and attendees as `PRIVATE`; apply redaction and require normal capability policy and scoped approval for writes. **Frozen design:** `DC3`, `DC11`, `M1`; enforcement **not yet present** until P4–P8 and P9–P12.
- **Protocol + invariants:** `PROTO-DATA DC2, DC3, DC11`; `PROTO-MODEL M1`; `PROTO-CAP C1`; `PROTO-POLICY P1, P2`.
- **Residual risk:** A persuasive, correctly rendered approval can still be accepted by a human who fails to notice the target or side effect.
- **Verification:** `SEC-CHK-01`; P4–P8 scripted hostile-content proposal test; P9–P12 provider integration verifies classification and approval path.

### AB-03 — Prompt injection via web content

- **Threat / adversary:** Elevation of privilege / disclosure; `ADV-2` page operator or `ADV-7` compromised web/provider dependency.
- **Attack path:** A page places instructions and exfiltration requests in content fetched for a user task; model output attempts to send private context to a third party.
- **Asset:** `AST-1`, `AST-14`, `AST-3`; **High** because web content is unauthenticated and may request credential-bearing actions.
- **Host-side mitigation:** Classify fetched page content by origin, redact before any permitted model egress, exclude credentials structurally, and independently authorize every communication capability. Web capabilities stay enumerated; no arbitrary URL/command authority is inferred from prose. **Frozen design:** `DC1`, `DC8`, `M1`, `C1`; host code enforcement **not yet present** until P4–P8; web-provider implementation phase is not assigned.
- **Protocol + invariants:** `PROTO-DATA DC1, DC2, DC3, DC8`; `PROTO-MODEL M1`; `PROTO-CAP C1, C2`; `PROTO-POLICY P1`.
- **Residual risk:** A page can manipulate what the model recommends; explicitly approved disclosure remains a human decision.
- **Verification:** `SEC-CHK-01`; P4–P8 mock-provider adversarial-output test and, in a future explicitly assigned web-provider phase, isolation/egress evidence.

### AB-04 — Prompt injection via Android notification text

- **Threat / adversary:** Elevation of privilege / disclosure; `ADV-2` author of another app's notification text or `ADV-4` malicious app with notification-listener access.
- **Attack path:** A listener-observed notification is mistakenly treated as a Core-ingestion source, or text the user explicitly selects and submits is treated as trusted instructions from its original author; either could steer the model toward an action.
- **Asset:** `AST-2`, `AST-14`, `AST-20`; **High** because notification-listener text is `PRIVATE` and is controlled by other apps/authors.
- **Host-side mitigation:** Android notification-listener observations are local-only by default and do not sync to Core. The explicit user-mediated path is selection/submission of chosen notification text as a user request; submitted content remains untrusted and `PRIVATE`, classified by origin rather than by the submitting gesture, and follows the normal model proposal and authorization pipeline. Separately, notification content Serea posts from host events follows data-egress/redaction rules, and no notification action authorizes an effect. **Frozen design:** `D2`, `D10`, `DC11`, `M1`; implementation enforcement **not yet present** until P4–P8/P9–P12.
- **Protocol + invariants:** `PROTO-DEVICE D2, D10`; `PROTO-DATA DC2, DC11`; `PROTO-MODEL M1`; `PROTO-CAP C1`.
- **Residual risk:** A user can deliberately submit hostile notification text; Serea cannot make the user's submitted request benign or authenticate the notification's original author.
- **Verification:** `SEC-CHK-02`; P4–P8 untrusted-input tests and P9–P12 Android boundary tests for local-only observation, explicit user-mediated submission, and redaction of Serea-posted notifications.

### AB-05 — Model fabricates completion of an action

- **Threat / adversary:** Spoofing / repudiation; `ADV-1` malicious model or an ordinary incorrect response.
- **Attack path:** Model says an email was sent or an event created despite no provider invocation or receipt; UI presents the claim as fact.
- **Asset:** `AST-12`, `AST-17`, `AST-15`; **Critical** because user decisions may depend on a false completion claim.
- **Host-side mitigation:** Derive completion from durable task state and provider-produced `SideEffectReceipt`, commit receipt before advancing, and label unverified narrative as narrative. Model self-report is never evidence. **Frozen design:** `M3`, `C4`, `T4`, `E9`; implementation **not yet present** until P4–P8, storage P1–P3, and end-to-end P9–P12.
- **Protocol + invariants:** `PROTO-MODEL M3`; `PROTO-CAP C4`; `PROTO-TASK T4`; `PROTO-EVENT E9, E10`.
- **Residual risk:** No protocol rule currently prevents free-form chat text itself from making an unsupported completion claim; user-facing claim gating needs implementation discipline and a test.
- **Verification:** `SEC-CHK-03`; P4–P8 fake-provider test rejects model-authored receipt/success and P9–P12 timeline/UI end-to-end evidence.

### AB-06 — Escalation beyond task scope or policy ceiling

- **Threat / adversary:** Elevation of privilege; `ADV-1`, `ADV-2`, or `ADV-9` (future delegated summary).
- **Attack path:** A low-risk task is replanned to add communication, external write, destructive, or delegated-host work beyond its assigned `policy_class`.
- **Asset:** `AST-9`, `AST-11`, `AST-12`; **Critical** because a successful bypass directly widens authority.
- **Host-side mitigation:** Host assigns the task ceiling; resolve risk from the registered immutable descriptor; deny above-ceiling actions before provider invocation; plan persistence confers no authority. **Frozen design:** `T6`, `P4`, `P2`, `C3`; enforcement **not yet present** until P4–P8.
- **Protocol + invariants:** `PROTO-TASK T6`; `PROTO-POLICY P1, P2, P4`; `PROTO-CAP C1, C3`.
- **Residual risk:** Human/admin misconfiguration can assign an unnecessarily broad ceiling; the protocols do not validate the user's intent.
- **Verification:** `SEC-CHK-04`; P4–P8 ceiling/property tests, including `COMMUNICATION` proposed in `OBSERVE` task.

### AB-07 — Arbitrary root operation or scope widening

- **Threat / adversary:** Elevation of privilege; `ADV-1`, `ADV-2`, `ADV-4`, or a compromised provider.
- **Attack path:** Attacker proposes shell/path access, claims root availability, widens a descriptor scope, or induces root provider to perform an unregistered operation.
- **Asset:** `AST-19`, `AST-11`, `AST-10`; **Critical** because root or arbitrary scope can escape Serea's intended finite surface.
- **Host-side mitigation:** Closed-world capability registry; no arbitrary shell/path capability; root operations individually registered and always require approval; rootless implementation is preferred and unavailable root must return structured `CAPABILITY_UNAVAILABLE`. **Frozen design:** `C1`, `C2`, `C9`, `P7`, `D9`; enforcement **not yet present** until P4–P8/P9–P12. A concrete scope-widening integrity seal has no protocol basis.
- **Protocol + invariants:** `PROTO-CAP C1, C2, C9`; `PROTO-POLICY P7`; `PROTO-DEVICE D9`; `PROTO-APPROVAL A1, A2`.
- **Residual risk:** Root-provider implementation defects and authority-state tampering are not solved by semantic checks; state-integrity gap is tracked at [AB-15](#ab-15-tampering-with-the-policy-ruleset).
- **Verification:** `SEC-CHK-05`; P4–P8 registry/root-policy test and P9–P12 rootless device integration proving absence is unavailable, not a bypass.

### AB-08 — Duplicate retry creates a second effect

- **Threat / adversary:** Denial of service / tampering; `ADV-1`, `ADV-5`, or ordinary crash/retry behavior.
- **Attack path:** Lost provider response or replayed request causes the host to issue an equivalent write again, creating duplicate messages/events.
- **Asset:** `AST-12`, provider state, `AST-17`; **High** because repeated communication or writes can be irreversible.
- **Host-side mitigation:** Derive stable per-step idempotency key; host checks equivalent completed `(capability_id, arguments_digest)` inside duplicate window; preserve original result/receipt; bound attempts. **Frozen design:** `C6`, `B10`, `T5`; implementation **not yet present** until P4–P8 and P9–P12 adapter testing.
- **Protocol + invariants:** `PROTO-CAP C6`; `PROTO-BOUNDS B7, B10`; `PROTO-TASK T5`; `PROTO-EVENT E3`.
- **Residual risk:** Provider-side idempotency support and the declared duplicate window may not cover a provider's semantics or repeated intent with intentionally changed arguments.
- **Verification:** `SEC-CHK-06`; P4–P8 crash/recovery duplicate test and P9–P12 provider idempotency integration including same key across restart.

### AB-09 — Ambiguous effect after a dropped connection

- **Threat / adversary:** Repudiation / tampering; `ADV-5` network attacker or provider timeout/failure.
- **Attack path:** Provider may have completed a non-idempotent send/write, but response is lost; retry creates a second effect.
- **Asset:** `AST-12`, external provider state, `AST-17`; **Critical** because a blind retry converts uncertainty into multiple effects.
- **Host-side mitigation:** Reconcile with read-back by provider reference/natural key; retry only after confirmed absence and when descriptor replay policy permits; unresolved result becomes `BLOCKED/AMBIGUOUS_EFFECT`. **Frozen design:** `C5`, `B12`, `T5`; enforcement **not yet present** until P4–P8/P9–P12.
- **Protocol + invariants:** `PROTO-CAP C5`; `PROTO-BOUNDS B12`; `PROTO-TASK T3, T5`; `PROTO-EVENT E9`.
- **Residual risk:** Some providers offer no reliable read-back; correct outcome remains blocked and requires human resolution.
- **Verification:** `SEC-CHK-07`; P4–P8 fake provider ambiguity matrix and P9–P12 integration evidence for each effecting provider's reconciliation behavior.

### AB-10 — Approval spoof or confused deputy

- **Threat / adversary:** Spoofing / elevation of privilege; `ADV-3`, `ADV-4`, `ADV-5`, or a malicious model.
- **Attack path:** Forged approval response, session treated as consent, or provider/model tricks host into applying a genuine grant to a different target/task.
- **Asset:** `AST-8`, `AST-10`, `AST-12`; **Critical** because grant authority is the only authorization for many writes.
- **Host-side mitigation:** Verify signed device frame, pending approval identity, exact capability/version/scope/task binding, use/expiry bounds; render host-written summary; do not infer consent from session presence. **Frozen design:** `A1–A4`, `A8`, `D2`, `D4`, `D6`; enforcement **not yet present** until P4–P8/P9–P12.
- **Protocol + invariants:** `PROTO-APPROVAL A1, A2, A3, A4, A8`; `PROTO-DEVICE D2, D4, D6`; `PROTO-CAP C1`.
- **Residual risk:** A real user may approve an accurately rendered but harmful request; the protocol does not eliminate coercion or inattentive consent.
- **Verification:** `SEC-CHK-08`; P4–P8 grant matching/property tests and P9–P12 signed-device approval integration tests.

### AB-11 — Replay of an approval, device frame, or pairing challenge

- **Threat / adversary:** Spoofing / elevation; `ADV-5` network attacker or `ADV-3` session holder.
- **Attack path:** Re-send captured `APPROVAL_RESPONSE`, reuse message ID, stale session frame, or consumed pairing nonce to mint/consume authority again.
- **Asset:** `AST-6`, `AST-8`, `AST-10`; **Critical** where replay produces a new grant or effect.
- **Host-side mitigation:** Verify signature and time skew, dedupe message IDs per session, consume pairing nonce once, reject non-pending approval, and consume grant atomically per step. **Frozen design:** `D4`, `D5`, `D7`, `A5`, `A6`; enforcement **not yet present** until P1–P3 and P9–P12.
- **Protocol + invariants:** `PROTO-DEVICE D4, D5, D7`; `PROTO-APPROVAL A5, A6`; `PROTO-EVENT E3`.
- **Residual risk:** Cryptographic primitive compromise is out of P0 scope; replay beyond retention semantics must fail closed.
- **Verification:** `SEC-CHK-09`; P1–P3 nonce/grant atomicity tests and P9–P12 device replay/reconnect integration tests.

### AB-12 — Stolen or unattended device session authorizes an action

- **Threat / adversary:** Spoofing / elevation; `ADV-3` physical holder of an unlocked, unverified phone.
- **Attack path:** Holder sees a pending approval and taps grant without the user understanding the request; attempts to use session longevity as authority.
- **Asset:** `AST-8`, `AST-10`, user/provider state; **High** because standard unlocked sessions can approve non-elevated actions.
- **Host-side mitigation:** Session authenticates device, not intent; each grant stays narrow, task-bound, expiring, limited-use; biometric is required only for `ELEVATED_DEVICE`, with no weaker fallback for that class. **Frozen design:** `D6`, `A1`, `A4`, `A7`; enforcement **not yet present** until P4–P8/P9–P12.
- **Protocol + invariants:** `PROTO-DEVICE D6, D8`; `PROTO-APPROVAL A1, A4, A7`.
- **Residual risk:** Biometrics are not required for ordinary `EXTERNAL_WRITE` or `COMMUNICATION`; unattended unlocked phone can authorize those if the person taps grant. This is an accepted design risk, not a biometric guarantee.
- **Verification:** `SEC-CHK-10`; P9–P12 device approval tests prove session alone is insufficient for `ELEVATED_DEVICE`; specifically record that ordinary approval has no biometric requirement.

### AB-13 — Credential egress through prompt, argument, or log

- **Threat / adversary:** Information disclosure; `ADV-1`, `ADV-7`, or compromised provider.
- **Attack path:** Secret bytes are serialized into model prompt, tool arguments, error/log, event, or provider different from credential's owner.
- **Asset:** `AST-3`, `AST-4`, `AST-7`, `AST-22`; **Critical** because disclosure enables account/device impersonation.
- **Host-side mitigation:** `Secret<T>` cannot serialize or format; inputs are allowlisted; unclassified values default to `CREDENTIAL`; only opaque provider-scoped `CredentialHandle` crosses provider call boundary; enforce egress matrix and redact before placement. **Frozen design:** `DC1`, `DC5–DC8`; implementation **not yet present** until P1–P3/P4–P8.
- **Protocol + invariants:** `PROTO-DATA DC1, DC4, DC5, DC6, DC7, DC8, DC9`; `PROTO-CAP C8`; `PROTO-EVENT E8`.
- **Residual risk:** Compromised code executing inside the host process can access secrets at named exposure points; malicious supply chain remains in scope.
- **Verification:** `SEC-CHK-11`; P1–P3 serialization/zeroization tests and P4–P8 prompt/egress negative tests; evidence must show no secret bytes in prompts, logs, events, or other-provider contexts.

### AB-14 — Malicious device app abusing an exported device component

- **Threat / adversary:** Spoofing / elevation / information disclosure; `ADV-4` malicious Android app.
- **Attack path:** External app invokes an exported activity/receiver/service, supplies hostile extras, redirects an intent to a privileged handler, or reads Serea cache.
- **Asset:** `AST-4`, `AST-20`, `AST-10`; **Critical** because the manifest surface currently has no protocol contract.
- **Host-side mitigation:** **No frozen protocol mitigation exists.** Proposed implementation control (not a guarantee): minimize exported components, explicit permissions, validate every intent at entry, never re-dispatch untrusted extras, and keep authority operations on authenticated host pipeline. Add manifest/component tests before shipping Android client. Does not exist until Android phase P9–P12 and requires a design/ADR if it changes contract.
- **Protocol + invariants:** No protocol invariant directly covers manifest/exported intent surface. `D2` limits device authority in design but does not define Android manifest policy.
- **Residual risk:** **No protocol basis at P0.** A malicious app may reach a component unless implementation security review and tests close the gap; OS/TEE compromise remains out of scope.
- **Verification:** `SEC-CHK-12`; P9–P12 manifest audit and instrumentation test must enumerate exported components and prove hostile intents cannot invoke approval/effect paths.

### AB-15 — Tampering with the policy ruleset

- **Threat / adversary:** Tampering / elevation; `ADV-6` compromised local account or `ADV-7` supply-chain attacker.
- **Attack path:** Modify durable policy/disabled overlay or registry state outside the admin plane so previously denied capabilities become eligible.
- **Asset:** `AST-9`, `AST-11`, `AST-16`; **Critical** because policy state decides the authority surface and currently has no integrity seal.
- **Host-side mitigation:** Semantic controls: policy is deterministic, disabled overlay wins, mutations use local admin plane and emit `POLICY_CHANGED` before/after diff. **No frozen MAC/hash-chain/signature protects stored policy bytes.** Integrity sealing is deferred for an ADR and implementation; semantic checks cannot stop a local writer.
- **Protocol + invariants:** `PROTO-POLICY P1, P8, P9`; `PROTO-EVENT E2, E3, E5`; these audit intended writer behavior but do not cryptographically seal state.
- **Residual risk:** An attacker with host database write access can alter policy without detection guaranteed by P0 protocol.
- **Verification:** `SEC-CHK-13`; P4–P8 policy determinism/admin audit tests; storage-phase tamper-detection test **cannot be claimed until a protocol/ADR defines a seal**.

### AB-16 — Forging or replaying an approval grant

- **Threat / adversary:** Spoofing / tampering; `ADV-6`, `ADV-7`, or captured device response.
- **Attack path:** Insert or modify durable `ApprovalGrant`, consumption record, task binding, expiry, scope, or digest; replay a previously valid response.
- **Asset:** `AST-10`, `AST-16`; **Critical** because forged ledger state directly manufactures consent.
- **Host-side mitigation:** Semantically validate all six grant bounds, including the exact approved `arguments_digest`, verify authenticated response, task-bind grant, atomically consume per step, append audit lifecycle events. Durable-ledger integrity sealing is **not specified** and is not implied by `A5`/`E2`.
- **Protocol + invariants:** `PROTO-APPROVAL A1, A2, A4, A5, A8`; `PROTO-DEVICE D4, D5`; `PROTO-EVENT E2, E3, E5`.
- **Residual risk:** A local database writer can forge or change durable grant state; no P0 protocol integrity guarantee closes it.
- **Verification:** `SEC-CHK-14`; P1–P3 atomic-consumption and P4–P8 grant semantics tests; tamper-evidence test deferred until an ADR defines durable-state protection.

### AB-17 — Stale provider data triggers a wrong write

- **Threat / adversary:** Tampering / repudiation; `ADV-10` stale data or `ADV-11` compromised provider account.
- **Attack path:** Cached/deleted/revoked event or message is treated as current; Serea creates/recreates or sends based on outdated facts.
- **Asset:** `AST-1`, `AST-12`, external provider state; **High** because stale state can cause an external effect.
- **Host-side mitigation:** Provider data is classified by origin; sync detects invalid history and falls back to full resync; before a consequential write, re-read/reconcile current state where the provider supports it. The frozen Approval Protocol binds a grant to the exact canonical `arguments_digest` shown to the user and revalidates it against durable step input immediately before grant consumption; any changed arguments, step order, descriptor, task ceiling, or denied policy invalidates the grant. **Frozen design:** `A11`, `DC11`, `D12`. Provider freshness/read-back remains provider-specific and must be implemented/tested, not assumed universally.
- **Protocol + invariants:** `PROTO-DATA DC3, DC11`; `PROTO-DEVICE D12`; `PROTO-CAP C5, C7`; `PROTO-APPROVAL A1, A4`.
- **Residual risk:** External provider may be eventually consistent or lack reliable freshness/read-back; protocol does not give all content a universal freshness bound.
- **Verification:** `SEC-CHK-15`; P9–P12 provider integration tests for stale/deleted item and history expiry/full resync, plus a negative test proving digest mismatch invalidates a grant before invocation; document per-provider freshness/read-back limitations.

### AB-18 — Memory poisoning by attacker-authored content

- **Threat / adversary:** Tampering / elevation; `ADV-2` malicious author or `ADV-11` compromised account.
- **Attack path:** Hostile email/calendar content is extracted as durable preference/fact and later steers a different task without the original context.
- **Asset:** `AST-13`, `AST-14`; **High** because poisoned memory persists beyond the source task.
- **Host-side mitigation:** Only host-assigned `EXTRACTION` call creates memory; use redacted projection; inherit source class; record immutable provenance; explicit deletion cascades with tombstone; external content is not ambient context by default. **Frozen design:** `DC3`, `DC11`, `DC12`, `T10`; enforcement **not yet present** until P4–P8.
- **Protocol + invariants:** `PROTO-DATA DC3, DC10, DC11, DC12`; `PROTO-TASK T10`; `PROTO-MODEL M1`.
- **Residual risk:** Correctly attributed but false or misleading extracted claims can still influence later work; provenance aids review, not truth verification.
- **Verification:** `SEC-CHK-16`; P4–P8 extraction/provenance/deletion cascade tests, including chat/analysis memory-write rejection and attacker-authored false fact.

### AB-19 — Runaway orchestration or repeated action loop

- **Threat / adversary:** Denial of service / elevation; `ADV-1` model or a bugged provider/scheduler.
- **Attack path:** Model repeatedly revises plan, retries tools, loops equivalent actions, or schedules steps indefinitely to burn resources or create effects.
- **Asset:** `AST-12`, `AST-17`, provider state; **High** due to cumulative cost and side-effect blast radius.
- **Host-side mitigation:** Host owns termination and all bounds; durable per-task repeat window, attempt/tool/model/token/wall-clock caps; fail explicitly; no model-settable or model-visible bound. **Frozen design:** `B1–B9`, `B15`, `B16`, `M6`; implementation **not yet present** until P4–P8.
- **Protocol + invariants:** `PROTO-BOUNDS B1, B2, B7, B8, B9, B15, B16`; `PROTO-MODEL M6`; `PROTO-TASK T7`.
- **Residual risk:** Bounds limit quantity but cannot prevent an authorized effect within the permitted budget; overly generous admin limits increase exposure.
- **Verification:** `SEC-CHK-17`; P4–P8 deterministic loop tests across restart, explicit outcome/event assertions for each exhausted bound.

### AB-20 — Denial of service by a huge attachment or notification flood

- **Threat / adversary:** Denial of service; `ADV-2`, `ADV-4`, `ADV-11` or compromised provider.
- **Attack path:** Supply oversized attachment/page/notification stream or high-object-count payload to exhaust memory, disk, parse time, or notification handling.
- **Asset:** `AST-1`, `AST-2`, `AST-20`, availability; **High** because no frozen byte/object limit exists.
- **Host-side mitigation:** Apply existing call-rate and task-work bounds where applicable (`B3`, `B16`) and isolate parsing; however `B3` declares the set complete while specifying no payload-byte, attachment-size, or object-count cap. New finite byte/object limits and error semantics require a protocol update/ADR, not an invented guarantee here. **Deferred: No protocol basis at P0.**
- **Protocol + invariants:** `PROTO-BOUNDS B3, B16` do not provide payload size bounds; `PROTO-DEVICE D5` handles duplicate delivery, not volume size.
- **Residual risk:** One large payload may exhaust resources despite current call limits. Bound set incompleteness remains an open security finding.
- **Verification:** `SEC-CHK-18`; proposed P9–P12 adversarial size/flood test is blocked as a security pass until an ADR adds byte/object bounds and implementation rejects over-limit input before allocation.

### AB-21 — Fake or future GoalLatch misreports completion

- **Threat / adversary:** Spoofing / repudiation; `ADV-9` future compromised/misbehaving GoalLatch; future fake misconfiguration in P15.
- **Attack path:** A goal summary, model turn, or adapter response claims success without host-observed terminal result/evidence; future adapter also attempts bypass or reads extra workspace state.
- **Asset:** `AST-18`, `AST-12`, `AST-17`; **Critical** because Serea may falsely claim host code/filesystem work completed.
- **Host-side mitigation:** At P0 the GoalLatch boundary is contract-only; no provider is implemented or registered. P15 is planned to implement an offline fake and exercise delegation through the normal capability/policy/approval path; `COMPLETED` requires schema-valid `host.goal.result` and `GOAL_RESULT` evidence must be host-observed. A future real adapter is barred until six live readiness checks and requires separate explicit phase authorization. **Frozen design:** `G1, G2, G4, G8, G9`; P16 closes pre-integration readiness. No integration is permitted now.
- **Protocol + invariants:** `PROTO-GOALLATCH G1, G2, G4, G5, G7, G8, G9, G13`; `PROTO-CAP C4`; `PROTO-MODEL M3`.
- **Residual risk:** Fake proves path shape only, not real execution truth. Even after gate, adapter's observations/evidence can be dishonest or incomplete; host must never treat it as authority.
- **Verification:** `SEC-CHK-19`; P15 fake scenario/misreport tests; six live artifacts in GoalLatch §9 required for P16 readiness closure; any later real adapter requires separate authorization and adversarial result/evidence tests.

### AB-22 — Cross-task isolation failure or approval reused across tasks

- **Threat / adversary:** Elevation / information disclosure; `ADV-1`, `ADV-6`, or buggy host worker.
- **Attack path:** Step/lease/arguments or approval from task A are attached to task B; model context or one task's grant is reused across task boundary.
- **Asset:** `AST-10`, `AST-12`, `AST-14`; **Critical** because data can leak and authority can cross task boundaries.
- **Host-side mitigation:** Task/step IDs bind durable state; approval grants require exact `task_binding`; leases are per step; separate task contexts; task class ceiling applies; no conversation history is authoritative task state. **Frozen design:** `A4`, `T1`, `T6`, `T7`; enforcement **not yet present** until P1–P3/P4–P8.
- **Protocol + invariants:** `PROTO-APPROVAL A1, A4, A5`; `PROTO-TASK T1, T4, T6, T7`; `PROTO-CAP C1`.
- **Residual risk:** Local durable-store tampering can defeat semantic task binding; see [AB-16](#ab-16-forging-or-replaying-an-approval-grant).
- **Verification:** `SEC-CHK-20`; P1–P3 schema/transaction isolation tests and P4–P8 two-task cross-binding/property tests across restart.

### AB-23 — Notification becomes a covert channel

- **Threat / adversary:** Information disclosure; `ADV-4` notification listener or observer of a locked screen.
- **Attack path:** Sensitive content is copied into notification body, approval preview, or chat delta and exposed beyond Serea to OS surfaces/other apps.
- **Asset:** `AST-2`, `AST-15`, `AST-20`; **High** because notification content can be read by screen viewers or notification access apps.
- **Host-side mitigation:** Redact before notification rendering; `PRIVATE`-or-higher content is removed/redacted; fixed channels and no inline actions; notifications are convenience mirrors, timeline is authoritative. **Frozen design:** `D10`, `E8`, `DC3`, `DC10`; enforcement **not yet present** until P9–P12.
- **Protocol + invariants:** `PROTO-DEVICE D10`; `PROTO-EVENT E8`; `PROTO-DATA DC3, DC9, DC10`.
- **Residual risk:** Redaction patterns can miss sensitive free text; external apps with notification-listener access are outside Serea's control.
- **Verification:** `SEC-CHK-21`; P9–P12 redaction-before-render tests with notification listener fixture and locked-screen previews.

### AB-24 — Root bypasses biometrics or security screens

- **Threat / adversary:** Elevation of privilege; `ADV-1`, `ADV-4`, or defective root provider.
- **Attack path:** Root implementation performs UI action, device control, or sensitive operation without full approval/secure-screen checks, or falls back around unavailable biometrics.
- **Asset:** `AST-19`, `AST-10`, device security controls; **Critical** due to privileged execution and user-intent bypass.
- **Host-side mitigation:** Root capabilities are enumerated and individually approved; root cannot bypass `ELEVATED_DEVICE` biometric rule or full in-app approval screen; absent root is unavailable; rootless path remains preferred. **Frozen design:** `P7`, `A7`, `D6`, `D10`, `C9`; implementation **not yet present** until P9–P12.
- **Protocol + invariants:** `PROTO-POLICY P7`; `PROTO-APPROVAL A7`; `PROTO-DEVICE D6, D10`; `PROTO-CAP C9`.
- **Residual risk:** The device OS/root implementation itself could lie or bypass platform controls; protocol says Serea must not bypass them but does not defeat compromised OS/TEE.
- **Verification:** `SEC-CHK-22`; P9–P12 rootless/root-provider matrix tests; prove elevated action cannot pass without biometric and every approval opens the full foreground screen.

### AB-25 — Cost exhaustion and provider quota burn

- **Threat / adversary:** Denial of service / cost abuse; `ADV-1`, runaway orchestration, or repeated user-originated workload.
- **Attack path:** Excess model calls, repair/fallback attempts, large token totals, or paid model use burns per-task/global budget or provider quota.
- **Asset:** `AST-7`, `AST-21`, service availability; **High** because unexpected spend and quota exhaustion deny future service.
- **Host-side mitigation:** Host-enforced call/output/token/spend budgets; account every call; hard-fail at exhaustion, never silently switch model or shorten/claim complete. Provider quota has no distinct frozen bound beyond local spend/token controls. **Frozen design:** `M6`, `M8`, `B2`, `B14`, `B16`; enforcement **not yet present** until P1–P3/P4–P8.
- **Protocol + invariants:** `PROTO-MODEL M6, M8`; `PROTO-BOUNDS B2, B14, B16`.
- **Residual risk:** Provider-side quota/pricing changes and uncapped provider API charges beyond Serea's accounting remain possible; no dedicated provider-quota bound is frozen.
- **Verification:** `SEC-CHK-23`; P1–P3 atomic usage accounting and P4–P8 budget-exhaustion tests covering repair/fallback and no silent model downgrade.

### AB-26 — Forged or malformed provider result accepted as success

- **Threat / adversary:** Spoofing / tampering; `ADV-7` malicious dependency/provider or `ADV-11` compromised provider account.
- **Attack path:** Provider returns malformed output, wrong request result, or `SUCCEEDED` effecting result with missing/fabricated receipt; host forwards it to task state/UI.
- **Asset:** `AST-12`, `AST-17`, external state; **Critical** when false receipt supports user-visible completion.
- **Host-side mitigation:** Validate output schema fail-closed; provider faults degrade to unavailable; effecting success requires provider-produced receipt and receipt is persisted before advancing. **Frozen design:** `C4`, `C7`, `T4`, `E9`; enforcement **not yet present** until P4–P8/P9–P12.
- **Protocol + invariants:** `PROTO-CAP C4, C7`; `PROTO-TASK T4`; `PROTO-EVENT E9`.
- **Residual risk:** A malicious provider running in-process can fabricate internally consistent output; P0 does not cryptographically attest provider truth.
- **Verification:** `SEC-CHK-24`; P4–P8 adversarial provider contract tests and P9–P12 adapter output/receipt validation tests.

### AB-27 — Pairing or host-identity substitution

- **Threat / adversary:** Spoofing / elevation; `ADV-5` network attacker or `ADV-4` rogue app observing pairing material.
- **Attack path:** Replay/steal pairing nonce, substitute host fingerprint, enroll rogue device, or present changed host identity as a warning users can dismiss.
- **Asset:** `AST-5`, `AST-6`, `AST-8`; **Critical** because rogue paired devices can receive private activity and submit approvals.
- **Host-side mitigation:** Host-signed pairing payload with short-lived single-use nonce; device pins fingerprint; later devices require host-side confirmation; device key is non-exportable; host revocation burns device identity and fingerprint changes hard-fail. **Frozen design:** `D3`, `D4`, `D7`, `D8`; enforcement **not yet present** until P9–P12.
- **Protocol + invariants:** `PROTO-DEVICE D3, D4, D7, D8`; `PROTO-DATA DC5, DC7`.
- **Residual risk:** Human scanning/confirmation can be deceived; cryptographic primitive and OS/TEE compromise are out of scope.
- **Verification:** `SEC-CHK-25`; P9–P12 pairing integration tests for nonce replay/expiry, host-key mismatch, later-device confirmation, revocation.

### AB-28 — Event history or client timeline misrepresents what happened

- **Threat / adversary:** Tampering / repudiation; `ADV-6`, `ADV-7`, or stale/replayed event delivery.
- **Attack path:** Event/state commit diverges, timeline drops a sequence gap, unknown event kind crashes a client, or durable event store is altered after effect.
- **Asset:** `AST-17`, `AST-12`, `AST-15`; **High** because incident reconstruction and user claims become unreliable.
- **Host-side mitigation:** Append-only events; same transaction as state; monotonic gapless sequence; actor/causation; clients resume/dedupe and surface gaps; classify before crossing device link. **Frozen design:** `E1–E5`, `E7–E9`, `D5`, `D11`; implementation **not yet present** until P1–P3/P9–P12. Append-only writer semantics are not a cryptographic seal against local file tampering.
- **Protocol + invariants:** `PROTO-EVENT E1, E2, E3, E4, E5, E7, E8, E9`; `PROTO-DEVICE D5, D11`.
- **Residual risk:** Local storage writer can rewrite data absent integrity sealing; this can conceal prior activity despite event semantics.
- **Verification:** `SEC-CHK-26`; P1–P3 transactional/sequence tests and P9–P12 reconnect, gap, duplicate, unknown-kind and redaction tests.

### AB-29 — Attacker-controlled text inside the approval preview

- **Threat / adversary:** Spoofing / repudiation; `ADV-2` content author attempts to speak directly to approver.
- **Attack path:** Attacker-controlled strings become title, recipient, location, summary, or preview text; UI hides or truncates the actual target/effect, causing mistaken consent.
- **Asset:** `AST-10`, `AST-15`, external provider state; **High** because approval is the final consent boundary and preview includes untrusted values.
- **Host-side mitigation:** Generate `plain_summary` in host code from validated arguments; show capability, exact object, expected effect; redact preview; escape/control formatting, preserve target visibility, and require full screen. The protocol specifies summary source/content but not every UI text-rendering defense; implementation and UI tests remain necessary.
- **Protocol + invariants:** `PROTO-APPROVAL A1, A2, A6`; `PROTO-DEVICE D10`; `PROTO-DATA DC10`.
- **Residual risk:** User can still approve a harmful action after accurate rendering; malformed/unusual Unicode and truncation behavior need implementation testing.
- **Verification:** `SEC-CHK-27`; P4–P8 host summary unit tests and P9–P12 UI tests for markup/control characters, long values, recipient/target visibility, redaction.

### AB-30 — Proactive watcher turns into an unattended writer

- **Threat / adversary:** Elevation of privilege / denial of service; `ADV-2`, `ADV-11`, or scheduler defect.
- **Attack path:** Attacker-shaped candidate wakes watcher; automation context raises approval or executes external write/communication instead of emitting suggestion only.
- **Asset:** `AST-9`, `AST-12`, `AST-15`; **Critical** if unattended external effect occurs.
- **Host-side mitigation:** Policy automation context permits only `OBSERVE` and `LOCAL_STATE`; watcher raises no approvals; bounded proposal count, explicit pause event; candidates remain suggestions for user action. **Frozen design:** `P5`, `A10`, `B16`; enforcement **not yet present** until P4–P8 and P11.
- **Protocol + invariants:** `PROTO-POLICY P5`; `PROTO-APPROVAL A10`; `PROTO-BOUNDS B16`; `PROTO-EVENT E5`.
- **Residual risk:** Read-only proposals can still mislead or disclose data through a poorly classified/redacted preview; output volume and candidate processing require bounded integration.
- **Verification:** `SEC-CHK-28`; P4–P8 exhaustive policy context tests and P11 watcher integration proving no approval or effecting invocation.

## 4. Verification references

Named `SEC-CHK-nn` obligations and phase evidence are catalogued in [04 — Security invariants §5](04-security-invariants.md#5-verification-catalogue). The catalogue distinguishes protocol contract tests from implementation evidence and identifies items that cannot pass until a missing control is specified.

## 5. Summary of gaps

| Gap | Cards | Status and disposition |
| --- | --- | --- |
| Android exported-component/intent contract absent | [AB-14](#ab-14-malicious-device-app-abusing-an-exported-device-component) | No protocol basis at P0. Require manifest policy and implementation tests before Android release; an architectural contract change needs normal change control. |
| Policy and approval durable-state integrity seal absent | [AB-15](#ab-15-tampering-with-the-policy-ruleset), [AB-16](#ab-16-forging-or-replaying-an-approval-grant), [AB-28](#ab-28-event-history-or-client-timeline-misrepresents-what-happened) | No protocol basis at P0. Existing semantic checks/audit do not resist local database writes. Decide threat assumption or add ADR and integrity design before claiming tamper detection. |
| Payload-byte, attachment-size, object-count bounds absent | [AB-20](#ab-20-denial-of-service-by-a-huge-attachment-or-notification-flood) | `B3` declares a complete bound set but no such limits exist. Requires ADR and protocol bound update before enforcement can be claimed. |
| Provider-specific freshness guarantees incomplete | [AB-17](#ab-17-stale-provider-data-triggers-a-wrong-write) | Implement read-back/freshness where providers support it; do not assert a universal frozen freshness bound. |
| Provider quota bound not specified | [AB-25](#ab-25-cost-exhaustion-and-provider-quota-burn) | Local model-call/token/spend ceilings are frozen design; external quota/pricing may exceed estimates. A new quota bound requires protocol change. |
| Real GoalLatch behavior unverified | [AB-21](#ab-21-fake-or-future-goallatch-misreports-completion) | Keep fake only. P15 is fake-only and P16 closes readiness; any real integration requires a separate future-phase authorization after the gate. |
| Ordinary approval does not require biometrics | [AB-12](#ab-12-stolen-or-unattended-device-session-authorizes-an-action) | Frozen design requires biometric only for `ELEVATED_DEVICE`. Changing this is an approval-protocol decision, not an implementation assumption. |
