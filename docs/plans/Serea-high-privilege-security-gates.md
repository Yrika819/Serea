# Serea High-Privilege Security Gates

**状態:** proposed gate map; no capability or owner decision is authorized here.
**Scope:** P7 Memory, P8 fake/real dispatch, Android, optional root, GoalLatch and later host integrations.

## Evidence labels

- **Accepted + implemented:** accepted contract has an identifiable implementation path and current tests/evidence. This is not production authorization by itself.
- **Accepted, unimplemented:** frozen requirement, but no production implementation/evidence yet.
- **Proposed:** recommendation only; owner has not ratified it.
- **Unverified:** code or documentation suggests a control, but a required target/fault/integration proof is absent.
- **Deferred:** explicitly out of current implementation scope.
- **Known limitation:** residual weakness or guarantee intentionally not claimed.

No row may move to “implemented” from prose alone. Every production gate requires an evidence artifact linked to exact code, migration, tests/build, threat review, and target environment. Host Policy/Approval remains the sole authority system. These gates do not add a second policy service.

## System-wide effect gate

Before a future privileged effect, Core must establish, in one fresh and auditable authorization path:

1. authenticated Owner principal and immutable Task binding;
2. exact capability, descriptor/version, account binding, target, and argument digest;
3. current registry and host Policy decision, including current deny/disabled overlay;
4. explicit approval where Policy requires it, bound to exact action identity, target, scope, material consequences, expiry, and Task/Step;
5. current Task/Step/lease/deadline/session/device/revocation state;
6. durable intent, atomic grant-use and bounded accounting before provider/process IO;
7. exact provider-produced result/receipt validation and durable commit;
8. cancellation, ambiguous result, revocation, retention, Forget, and recovery semantics;
9. content-free audit and truthful user summary.

Model output, external content, retrieval, a signed device frame, capability availability, historical approval, Task label, GoalLatch output, and silence are never permission. If any check is missing or stale, deny before effect and preserve unresolved-state evidence where an effect may already have happened.

## Registered trust boundaries TB-1–TB-15

Boundary names and meanings are those of [Architecture Trust Boundaries](../architecture/02-trust-boundaries.md) and [Threat Model Assets/Boundaries](../threat-model/01-assets-and-trust-boundaries.md). This table preserves their numbering.

| Boundary | Reviewed control and current status | Required future gate / denial behavior | Main threat references |
| --- | --- | --- | --- |
| TB-1 Device → Core | **Accepted + partly implemented:** Device Protocol separates authenticated device/session from intent; P6 approval persistence records device/session. **Known limitation:** no host-authenticated OwnerId contract/production client pairing evidence here; paired device is not a human identity. | Owner identity bootstrap; host-pinned key; fresh session and replay tests; signed approval bound to exact summary/action. Device loss/revoke denies new commands and approvals; stale session never grants authority. | AST-4/5/8/10/13; AS-2/3; AB-10/11/12/27; INV-SEC-15/16 |
| TB-2 Model → Core | **Accepted + implemented at interface level:** model responses are untrusted proposal data; schema/purpose routing exists in model router. **Unverified:** hostile external content and future extraction-to-Memory end-to-end. | Closed schema, host-owned purpose/class/owner; malformed/unknown fields reject; output cannot forge approval, policy, account, retention, OwnerId or tool. | AST-14; AS-1/4; AB-01–06/18; INV-SEC-01/03/09/10 |
| TB-3 Core ↔ external provider | **Accepted, unimplemented:** provider boundary, scoped credential handle and receipt contract exist; no real Gmail/Calendar invocation in P6. | Separate per-provider/account capability, current Policy, exact target approval, durable intent, credential/account/scope binding, result/receipt reconciliation. Any missing provider proof means unavailable. | AST-1/3/7/12; AS-5/6; AB-08/09/13/17/26; INV-SEC-04/08/12 |
| TB-4 Host → credential store | **Accepted, unimplemented:** opaque `CredentialHandle` and OS store design; production CredentialStore custody absent. | OS Keychain/Keystore adapter, handle audience/provider/account/scope/owner/version checks, one-call resolution and revocation/rotation; bytes excluded from model, DB, logs, diagnostics and crash reports. Fail closed on stale handle. | AST-3/7/22; AS-6; AB-13; INV-SEC-08 |
| TB-5 Host → GoalLatch | **Accepted, unimplemented/deferred:** ADR-0011 fake-only until integration gate; adapter output never authority. | No connection in P7/P8. Later only after protocol §9 live validations + separate phase authorization. Host-side confinement/Policy must wrap every delegated operation; fake tests prove no network/filesystem/process. Stop/cancel and secondary-action behavior must be observed. | AST-18; AS-10; AB-21; INV-SEC-02/12 |
| TB-6 Host → local OS resources | **Accepted design, unimplemented:** finite capabilities, no arbitrary shell; rootless-first. Filesystem/process contract and implementation absent. | Resource handles, path-root confinement, symlink/TOCTOU defense, read/write/delete separation, process argv allowlist, network isolation, artifact provenance, destructive preview, OS sandbox. No generic shell; deny outside scope. | AST-16/19; AS-7/9; AB-07/14/24; INV-SEC-03/07 |
| TB-7 Core → durable store | **Accepted + implemented for P6 task/policy/approval/event transactions:** SQLite constraints, transactional participants, lease fencing and tests exist. **Unverified for Memory:** there is no Memory schema/FTS/Forget. **Known limitation:** local database tampering has no general integrity seal. | P7 owner/class composite keys, WAL/recovery/backup/forget tests, required UNIQUE parent keys and fail-closed restore reconciliation. Database corruption or schema/index mismatch must disable read/write rather than return unscoped data. | AST-9/10/12/13/16/17; AS-8/11; AB-15/16/18/22/28; INV-SEC-06/10/11/14 |
| TB-8 Provider → Provider | **Accepted architectural separation;** current concrete provider graph is limited. | Build-time crate/dependency checks and CredentialStore provider-audience tests; no callback or cross-provider handle. If an adapter asks another provider directly, reject architecture/registration. | AST-3/7/11/22; AS-5/6; AB-13; INV-SEC-08 |
| TB-9 Android app → Android OS | **Accepted design, deferred implementation:** device keys, biometric gate, notification privacy. No Android app permission/manifest/runtime proof in this task. | Manifest/export review, least permissions, Keystore key loss/revocation, biometric evidence freshness, notification cache/backup behavior. Fail closed if OS proof absent; selected notification stays untrusted PRIVATE input. | AST-2/4/20; AS-2/8; AB-12/14/24; INV-SEC-15/16 |
| TB-10 Host → Policy engine | **Accepted + implemented for P6 Policy/Approval evaluation:** deterministic rules, revision authority, bounded Task grants and fail-closed mismatch. **Known phase gap:** point-in-time evaluation alone does not dispatch a provider. | P8 dedicated dispatch path revalidates current Policy/Approval/registry/Task/lease immediately before atomic intent/use commit. A stale policy snapshot yields no IO. | AST-9/10/11/21; AS-3; AB-06/10/15/16/22; INV-SEC-01/03/05/06 |
| TB-11 Retrieved content → Host ingestion | **Accepted, unimplemented end-to-end:** origins are untrusted; retrieved content defaults PRIVATE; explicit EXTRACTION required. | Ingestion assigns class from host-known origin, never model; injection tests for Gmail/calendar/web/notifications/provider/GoalLatch output; extraction provenance host-bound; PRIVATE extraction refused under proposed D04. | AST-1/2/13/14; AS-1/4; AB-01–04/18/23; INV-SEC-09/10 |
| TB-12 Host → cloud model | **Accepted + partly implemented in model routing/classification:** SECRET/CREDENTIAL denied, PRIVATE cloud egress disabled by default. **Unverified:** production provider disclosure surface and future Memory metadata/FTS projection. | Destination and purpose allowlist, class inherited from all content and metadata, prompt logging disabled/minimized, credential scan, provider/account pinning. Deny if provenance/class missing. | AST-1/7/13/14/22; AS-4/6; AB-03/13/18/25; INV-SEC-08/09 |
| TB-13 Host → device notifications | **Accepted, unimplemented device UI path:** notification rendering must be redacted and cannot authorize. | Escape untrusted text, class-aware projection, opt-in preview; notification action opens ordinary authenticated flow and cannot approve/execute. | AST-15/20; AS-2/8; AB-12/23/29; INV-SEC-09/14/15 |
| TB-14 Scheduler/watcher → Task engine | **Accepted + partly implemented scheduler;** ADR-0016 says PROACTIVE may OBSERVE/LOCAL_STATE only, proposal-only. **Unverified** end-to-end watcher authority partition. | Host-fixed automation context; no writable provider Task, no approval generation, bounded proposal rate; proposal requires user-started new Task. Silence/retry cannot authorize. | AST-13/21; AS-1/3/10; AB-19/30; INV-SEC-01/03/13 |
| TB-15 Local admin → Host controls | **Accepted + partly implemented:** host-admin Policy/registry/bounds authority and audit events exist. **Known limitation:** local DB file writer can bypass semantic APIs; no general tamper seal. | Separate OS admin authentication, auditable diff, no device/model edits, sensitive change reauthentication, immutable audit export if required. Local DB integrity threat boundary must be owner-approved; mismatch disables dispatch. | AST-8/9/10/11/16/21; AS-8/11; AB-15/16/28; INV-SEC-05/14 |

### 状態の解釈

「Accepted + implemented」は指定範囲のみ。たとえば SQLite portability workflow はDB portabilityを検査してもFTS5 tokenizer、権限、電源断、WAL/backup confidentialityを証明しない。P6 CI成功は実provider・Android・GoalLatchの成功を意味しない。ADR-0022はProposedであり、PRIVATE通常行の完全保護をAcceptedとして扱わない。

## Capability denial prerequisites by phase

| Stage | Safe work | Required exit evidence | Deny/stop condition |
| --- | --- | --- | --- |
| 1. Research | Source/document review, synthetic threat models, protocol/DDL sketches | Evidence sources, existing ADR consistency, limitations and exact owner questions | No network/account/device access needed; no assumptions promoted to contract |
| 2. Fake-only implementation | No-network deterministic providers and synthetic fixtures, after relevant contract approval | Frozen fake interfaces; no credential/network/process effects; durable-state and fault matrix; code review | No fake-to-real route, no external effect, no model-controlled authority |
| 3. Isolated environment test | Ephemeral DB/VM/emulator, synthetic accounts/data, no production credentials | Reproducible build, policy/approval/revoke/cancel/restore tests, logs inspected for secret sentinels, environment isolation proof | Isolation or synthetic fixtures uncertain; any live account/host access; bypass means stop |
| 4. Request real-user authorization | A specific capability/action becomes reviewable; user sees exact target/account, payload/data class, effect/reversibility, timing, retention, consequences | Signed/verified session + fresh human confirmation; current Policy; one action identity, bounded expiry/use; explicit user understands external effect | Generic “automation” or provider-wide consent, ambiguity, stale target/session, hidden secondary action |
| 5. Enable one production capability | Only exact reviewed capability/version/target/account/scope and bounded rollout | Independent security review; migration and backup proof; provider contract/fault evidence; revoke/recovery; monitoring; owner authorization separate from tests | Any unresolved HIGH affecting authority/confidentiality, receipt ambiguity, policy bypass, credentials exposed, or failed deletion recovery |

Passing tests is evidence for a specific implementation claim, never user consent or production authorization. A single action approval cannot authorize another provider action, target, account, argument, retry, secondary action, or later Task.

## P8 and integrations: mandatory gates

### P8 dedicated dispatch and durable authority

- Preserve ADR-0036, ADR-0039 Option B and P6 Policy/Approval as the sole authorization chain. Do not add a competing authority ledger.
- A single TaskEngine dispatch entry point must revalidate current owner, pinned descriptor/version, Task and exact Step, lease generation, deadline, availability, current Policy revision/deny overlay, approval grant/action digest/scope/uses, and bounds.
- In one SQLite transaction, commit Grant use, per-step/member accounting, idempotency key and unique dispatch intent before any provider IO. No transaction spans IO. No public write API bypasses this entry point.
- Same-Step retry, fresh-Step retry, AMBIGUOUS, no-effect evidence, provider idempotency horizon and already-consumed Grant behavior need explicit owner choice and any required ADR/protocol amendment before replay.
- Cancellation before intent commit denies dispatch. Cancellation after intent does not erase intent/use; late result is not success or receipt authority until the exact reconciliation contract is implemented. Never revive a CANCELLED Task.
- Trusted Receipt must bind exact RequestId, intent, Task/Step, provider/account, target, action digest, validated result digest and registered capability/version. Provider output alone cannot mint authorization.

### Credentials and real Gmail/Calendar

No real provider until CredentialStore production custody is independently reviewed. Handle resolution must bind owner, provider, account, scopes, version, expiry and revocation; token bytes stay inside the adapter call path and out of model context, sqlite, ordinary results, events, memory, logs, telemetry and core dumps. No broad account selection by model. Real provider authorization is per exact action, not per provider family. PRIVATE data remains blocked from cloud egress by default and cannot be persisted to ordinary Memory under proposed P7-D04.

### Android sensitive permissions and optional root

Review Android manifest/export surface and permission rationale independently. Notification listener data remains local PRIVATE unless user explicitly submits selected text; submission does not make that content trusted or PUBLIC. Biometric assertion must be fresh, action-bound, verified by host protocol, and have no weaker fallback for elevated actions. Device capability report says available, not permitted.

ADR-0007 is binding: root is optional/non-default; finite allowlisted operations only; no generic root shell; every root action needs explicit approval and fresh host Policy; rootless features remain functional; no weaker fallback is selected; Credential privilege and OS privilege remain separate; enforce again at the actual effect boundary. Root availability never triggers escalation. Any helper process must have its own narrow OS identity/sandbox and operation protocol.

### GoalLatch / local MCP

ADR-0011 and GoalLatch Adapter Protocol remain binding. P7/P8 do not connect it. Fake-only P15 work must demonstrate no network, filesystem, subprocess or credentials and must use the ordinary Serea Policy/Approval/Task path. GoalLatch cannot select workspace/project by untrusted text without host pinning; it cannot return approval, OwnerId, policy or success authority. For a future real adapter, validate workspace/session identity, privilege inheritance, artifact paths, secondary actions, cancellation and late effects at the host boundary. A delegate continuing after cancellation stays unresolved and never grants follow-up authority.

## P7 interactions that raise later risk

- Owner binding plus Task deletion: detached provenance must keep an immutable owner/source snapshot; no live FK cascade that deletes Memory; Owner mismatch still blocks access.
- FTS plus class filters: index text can leak even if result rows are filtered later; index only eligible projection and verify owner/class on base row in the same query.
- Forget plus backup restore: tombstones must outlive recoverable snapshots or restore remains quarantined until an independent ledger is current.
- Model extraction plus untrusted content: source text can poison durable facts; extraction purpose, schema, provenance, class, user control and later action reauthorization all remain host-enforced.
- Scheduler plus persistent grants: watcher context must not create grant/approval or carry an approval into a different exact target.
- Grant use plus retry: a consumed use cannot transfer to another Step or action; unresolved effect cannot be replayed by convenience.
- Cancellation plus delayed provider response: cancelled Task cannot be revived; late result needs separately approved reconciliation.
- GoalLatch plus host privileges: delegate cannot inherit ambient user privileges or perform unapproved follow-on operations.

## Owner exception format

No security exception is self-approved. Any future exception must record: exact capability ID/version; single target/account/scope; exact restriction and expiry; residual impact; compensating technical control; stop/revocation trigger; owner identity and approval timestamp; independent reviewer; tests/evidence. “Risk accepted” alone is not a valid record.

## Production access remains denied

At this review state, Memory writes, PRIVATE Memory, real provider invocation, sensitive Android permissions, root operations, real GoalLatch/Local MCP and generic shell are unavailable. A proposed document, passing unit test, valid device signature, installed root binary, cached token or live user session does not change this state. Enablement requires the stage-specific gates above and a separate explicit owner authorization.
