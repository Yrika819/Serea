# Device Protocol

Protocol ID: `PROTO-DEVICE` · Surface: `serea.device/1` · Status: **FROZEN for P0**

This protocol defines the link between Serea Core (the Mac) and the Android
client (Pixel 7a, Android 17, API 37). It covers transport, pairing, sessions,
the device message set, capability reporting, notifications, the activity
timeline feed, and degraded operation.

---

## 1. Role of the device

> The device is a client of Serea, not an instrument of it. It renders, it
> collects, and it confirms. It never decides and it never effects.

The Android app is a first-class Serea client. A paired device is a complete
surface for six things:

| Surface | What the device does | Protocol source |
| --- | --- | --- |
| Chat | Sends `CHAT_MESSAGE`, renders streamed answers | [Model Protocol](03-model-protocol.md#3-modelrequest) `purpose: CHAT` |
| Activity timeline | Renders the event stream for the user | §8 |
| Approvals | Renders `ApprovalRequest`, returns `APPROVAL_RESPONSE` | [Approval Protocol §5](05-approval-protocol.md#5-device-bound-approval) |
| Notifications | Surfaces host events as Android notifications | §7 |
| Pairing | Establishes the device's identity and credential | §3 |
| Settings | Notification channels, timeline filters, sign-out | §7, §9 |

Two things the device is **not**:

- **It is not a remote-control bot.** There is no verb by which the device
  accepts "run this" and acts without the host's full pipeline. Every device
  effect is a host-caused capability invocation that passed schema validation,
  registry lookup, policy, and approval
  ([Capability Protocol §1](01-capability-protocol.md#1-core-principle)). The
  device client is a transport and a human interface, not an executor.
- **It is not an authority.** A device cannot widen a scope, extend a grant,
  lower a risk class, enable a capability, or move a bound. It holds a
  credential that *identifies* it to the host; authority over the world is lent
  separately, per action, per task, per session
  ([Approval Protocol §3.1](05-approval-protocol.md#31-the-six-bounds)).

Trust direction is one-way: the host trusts a device only to the extent that the
device's signed statements are attributable and to the extent that the device
presents a live session. A compromised device can lie about *itself*; it cannot
manufacture authority it was never granted.

---

## 2. Transport

> The device dials the host. The host never dials the device.

The link is authenticated HTTPS for pairing and bulk exchange, and an
authenticated WebSocket for the live channel. Both carry the envelope defined in
[Protocol Index §6](00-protocol-index.md#6-envelope).

### 2.1 Dial-out is mandatory

The **Android device initiates every connection** to Serea Core. The host
accepts inbound connections from paired devices and never opens an outbound
connection to a device's address.

| Requirement | Reason |
| --- | --- |
| No inbound port forwarding on the host | A public listener is an attack surface with no upside for a desktop app. |
| The phone stays reachable from arbitrary networks | Cellular, hotel wifi, and corporate VPNs change address constantly. |
| No device-side listening socket | A phone must never hold an open port that a remote party can connect to. |
| Cloudflare Tunnel or equivalent egress-only relay | Preserves the dial-out property when the host is behind NAT. |

Host discovery is by a single configured endpoint plus a host key fingerprint.
The device pins that fingerprint; a changed fingerprint is a hard failure and
requires re-pairing. There is no discovery protocol and no "accept any host".

### 2.2 Reconnection

Exponential backoff with **full jitter**: the delay before attempt *n* is drawn
uniformly from `[0, min(60000, 500 · 2ⁿ))` milliseconds — 0–500 ms at attempt
0, doubling each time, and capped at 60 s from attempt 6 on. Two additional
rules keep a flapping phone from hammering the host:

- A **reconnect floor**: no more than one connection attempt per 30 s, so a
  long-lived offline device costs the host about two handshakes a minute rather
  than one per jitter draw.
- A **circuit breaker**: after 20 consecutive failed attempts the client waits
  15 minutes before trying again, then resumes the ladder.

The host applies the mirror-image rule: a device presenting an invalid
credential is refused three times and then rate-limited to
`max_events_per_minute_per_device` for 15 minutes
([Bounds Protocol §2](10-bounds-protocol.md#2-the-bound-set)).

### 2.3 Session resumption versus full re-auth

| Condition | Behaviour |
| --- | --- |
| Transport dropped; device key valid; long-lived credential valid; prior session younger than 24 h | **Resume.** A new `ses_` is minted for the same `DeviceId`; the device re-presents its key and credential and the host re-verifies both. |
| Prior session older than 24 h, or host restarted and the session table was durably reloaded | **Full re-auth.** Same mechanism, but the host additionally re-verifies the device's registered key fingerprint and re-checks the revocation list. |
| Device key replaced (device reset, restored backup) | **Full re-auth**, which fails with `PAIRING_KEY_MISMATCH`. The device must be re-paired from the host admin surface. |
| Credential rotated by the host | **Full re-auth** against the new credential; the old one is revoked atomically. |
| Device revoked or unpaired | Connection refused with `PAIRING_REVOKED`. The device clears its local session material and returns to the pairing screen. |
| Host key fingerprint changed | Connection refused with `HOST_KEY_MISMATCH`. Never a warning, never a prompt. |

Resumption is cheap and frequent. Full re-auth is rare. Neither ever widens
authority: a resumed session is subject to exactly the same policy decisions as
the session it replaced.

### 2.4 Liveness

| Parameter | Value |
| --- | --- |
| Heartbeat interval (either side) | 20 s when idle, or immediately after any outbound frame |
| Liveness window | 90 s of silence |
| Client action after 3 unanswered heartbeats | Close the socket and reconnect |
| Host action after 90 s of silence | Mark session `DEGRADED`; tasks requiring that device move to `BLOCKED` with `blocked_reason: DEVICE_OFFLINE` ([Task Protocol §4.1](02-task-protocol.md#41-states)) |

`BLOCKED` rather than `FAILED` is deliberate: a task waiting on a phone that is
in a lift must resume when the phone reappears, not fail.

### 2.5 Delivery semantics

The wire is **ordered and at-least-once**. Ordering is guaranteed per
connection by a monotonically increasing `client_seq` on device-originated
messages and `server_seq` on host-originated ones. Delivery is at-least-once
because transport-level exactly-once does not exist across a reconnect boundary.

At-least-once is made safe by **idempotent handling keyed on `message_id`**:

| Side | Retained `message_id` set | Behaviour on a repeat |
| --- | --- | --- |
| Device | Last 2048 inbound ids | Drop the frame; do not re-render, do not re-emit an approval |
| Host | Last 4096 inbound ids per session | Drop the frame; emit no event; return no new result |

`message_id` is an `EventId` (`evt_` + ULID,
[Protocol Index §2](00-protocol-index.md#2-identifier-grammar)) and is never
reused. A frame whose `message_id` is already present is acknowledged with a
`PONG` carrying `duplicate: true` so the sender can retire it promptly rather
than waiting out a backoff window.

---

## 3. Pairing

> A device proves who it is once, to a host that a human is standing at. After
> that, the device's key is its name, and revocation is a host decision.

The **first** device is paired from the host admin surface alone. Every
subsequent device requires **explicit host-side confirmation** — the host's own
screen, on the Mac, with a human present. A second phone cannot approve the
arrival of a third phone; that is the rule that makes the pairing secret
meaningful.

### 3.1 Device key material

The device generates an **Ed25519 keypair in Android Keystore** using
`KeyGenParameterSpec`, with export disabled. Consequences:

- The private key **never leaves the device**. It is not serialisable, not
  extractable by root without the hardware-backed gate, and not present in any
  Serea process on the host.
- The host **never receives the device private key**. It stores the device public
  key and its fingerprint only.
- The key is the device's stable identity. Clearing Serea's app data destroys
  it; the device must be re-paired.
- The key is used for *authentication* of the device. It grants no capability,
  no scope, and no approval.

### 3.2 Pairing payload

The host renders a pairing payload as a QR code (and, as a fallback, as an
8-character Crockford Base32 short code with the same 90-second life).

```json
{
  "pairing_version": "1",
  "host_endpoint": "wss://serea-core.example.ts.net/device",
  "host_key_fingerprint": "sha256:3b7e0d4f8a2c6b9e1d5f3a7c0e4b8d2f6a9c1e5b7d3f0a8c2e6b4d9f1a7c3e5b",
  "device_name": "Pixel 7a",
  "pairing_nonce": "b3JkTG5vbmNlRm9yVGhpcyBEZWZpbmVkRGV2aWNlLi4u",
  "issued_at": "2026-10-01T09:12:04.210Z",
  "expires_at": "2026-10-01T09:13:34.210Z",
  "host_attestation": {
      "alg": "Ed25519",
      "payload_digest": "sha256:9f2c1a7e4b6d0f8a3c5e9b1d7f2a4c6e8b0d3f5a7c9e1b4d6f8a0c2e4b6d8f9a",
      "signature": "nJm4kQ8vT2xYcRz7bLwP0aHdF5sG1uXeN9iO3rVbKt6AyM4qCzW8dJhS0lGtE2nPuXuR7yE0iZtWq5nVb9xGc3lKd6"
    }
}
```

- `host_key_fingerprint` is a `Digest`
  ([Protocol Index §2](00-protocol-index.md#2-identifier-grammar)); the device
  pins it and refuses any host presenting a different one.
- `pairing_nonce` is a one-time 256-bit secret, **not an identifier** and not
  prefixed. It is classified `SECRET` and is discarded immediately after a
  single use.
- `expires_at` is 90 seconds after `issued_at`. A payload presented after
  expiry is refused with `PAIRING_NONCE_EXPIRED`, not silently refreshed.

### 3.3 Pairing sequence

1. The user opens the Serea host admin surface on the Mac and chooses *Pair a
   device*. The host mints a fresh 256-bit `pairing_nonce` and a host-signed
   pairing payload, and renders it as a QR code valid for 90 seconds.
2. The device app, on first launch, scans the QR code with the camera. No
   network traffic occurs yet: the phone has no credential and cannot
   authenticate.
3. The device generates its Ed25519 keypair in Android Keystore and computes
   the public key and its `sha256:` fingerprint.
4. The device posts `{ host_key_fingerprint, device_public_key, pairing_nonce }`
   to `host_endpoint/pairing/exchange` over TLS, signed with that new key so
   the host can bind the key it receives to the key the device holds.
5. The host verifies the nonce is unexpired and unused and verifies the device
   signature. For the **first** device it proceeds; for any later device it
   presents a confirmation prompt on the Mac naming the model, the fingerprint,
   and the fact that a new device will become able to approve requests.
   Absence of a host-side confirmation is a refusal, never an implicit one.
6. The host mints a `DeviceId` (`dev_` + ULID) and a per-device long-lived
   credential, both bound to the device's Ed25519 key. The host stores its copy
   in its own credential store; the device stores its copy in Android Keystore,
   bound to the device key so it cannot be copied to another handset.
7. The host records `DEVICE_PAIRED` evidence with the device id, fingerprint,
   model, `android_sdk_int`, and the actor who confirmed.
8. The device presents `{ device_id, credential, device_signature }`. The host
   verifies the signature against the registered public key and the credential
   against its store, then mints a session (§4).
9. The pairing nonce is destroyed on both sides. It is single-use; a replay
   attempt returns `PAIRING_NONCE_CONSUMED` and is recorded as evidence.

### 3.4 Revocation and unpairing

| Operation | Trigger | Effect |
| --- | --- | --- |
| Revoke | Host admin surface | Credential destroyed, sessions refused, `DEVICE_REVOKED` emitted. Pending approvals routed to that device are re-routed to another trusted device or the task moves to `BLOCKED` ([Task Protocol §6](02-task-protocol.md#6-recovery)). |
| Unpair | Either side | Revocation plus local credential deletion on the device. The `DeviceId` is burned and never reissued ([Protocol Index §2](00-protocol-index.md#2-identifier-grammar), rule 3). |
| Suspend | Host admin surface | Credential retained, sessions refused, device excluded from approval routing but still visible in the timeline. |
| Replace lost device | Host admin surface | Old device revoked, new device paired with a new key and a new `DeviceId`. |

---

## 4. Sessions and authentication

> A session authenticates the *device*. It never authenticates the *user's
> intent*.

A session is a mutual proof of identity plus a short-lived context. It is not a
capability, and holding one grants nothing on its own.

| Field | Value |
| --- | --- |
| `SessionId` | `ses_` + ULID, e.g. `ses_01JQ8ZG9V5MXK3N7QW2RTF8YHB` |
| Established by | Mutual authentication, not by possession of a token alone |
| Maximum absolute lifetime | 24 h |
| Idle lifetime | 15 min without a heartbeat or a frame |
| Binding | One `DeviceId`, one `SessionId`, one transport |
| Recorded in | The audit trail with device id, session id, and client version |

### 4.1 Mutual authentication

1. The device sends its `DeviceId`, its public key, and a client nonce.
2. The host replies with its own public key, its fingerprint, and a **host
   challenge** — a fresh 256-bit value, also classified `SECRET` and never
   logged.
3. The device signs the host challenge with its Ed25519 key and presents the
   long-lived credential it received at pairing.
4. The host verifies the signature against the registered public key, verifies
   the credential against its store, and confirms the fingerprint matches the
   one pinned at pairing.
5. Only then does the host mint the `SessionId` and switch the connection to the
   authenticated channel.

Every device-originated frame after handshake carries an Ed25519 signature over
the canonical JSON of the envelope
([Protocol Index §5](00-protocol-index.md#5-serialization)), verified per
frame. The `CAUGHT` condition to guard against — a replayed frame from a
captured session — is met by the per-frame `issued_at` skew check (±120 s) and
the `message_id` dedupe set (§2.5).

### 4.2 A session is not intent

An `ELEVATED_DEVICE` capability (per
[Policy Protocol §2](04-policy-protocol.md#2-risk-class)) requires approval,
and that approval must be confirmed with a **local biometric or
device-credential check on the device itself**
([Approval Protocol §5](05-approval-protocol.md#5-device-bound-approval)).

Therefore:

- A stolen or unattended session can render prompts, but cannot approve a
  highest-risk action without a `BiometricPrompt` confirmation on the handset.
- The host verifies the confirmation result over the device link and records
  `biometric.performed`, `biometric.method`, and the session id. It never
  receives or stores biometric material.
- If no device with a confirmed secure context is reachable, the approval
  **cannot be granted** and the task stays `WAITING_APPROVAL` or moves to
  `BLOCKED`. There is no fallback to a weaker path.
- Session lifetime does not extend a grant. A grant's `expires_at` runs from
  `granted_at` regardless of session renewal.

---

## 5. Message flow

Every device frame is the common envelope
([Protocol Index §6](00-protocol-index.md#6-envelope)) with a `payload` whose
`kind` is one of the frozen device message set below.

```json
{
  "envelope_version": "1",
  "surface": "serea.device/1",
  "message_id": "evt_01JQ8ZK5H4NQW9T2XR7BV3M8DF",
  "correlation_id": "tsk_01JQ8Z9K3M7QWXR4V2T6YH0BNA",
  "causation_id": null,
  "issued_at": "2026-10-01T09:14:20.118Z",
  "data_class": "PERSONAL",
  "trace": { "task_id": null, "step_id": null },
  "payload": {
    "kind": "CHAT_MESSAGE",
    "device_id": "dev_01JQ8ZC5N8TVG3K6MRQ2XW9JHF",
    "session_id": "ses_01JQ8ZG9V5MXK3N7QW2RTF8YHB",
    "client_seq": 4182,
    "content": "Move tomorrow's design review to 14:00.",
    "attachments": [],
    "signature": {
      "alg": "ed25519",
      "key_id": "dev_01JQ8ZC5N8TVG3K6MRQ2XW9JHF",
      "value": "nJm4kQ8vT2xYcRz7bLwP0aHdF5sG1uXeN9iO3rVbKt6AyM4qCzW8dJhS0lGtE2nPuXuR7yE0iZtWq5nVb9xGc3lKd6"
    }
  }
}
```

`causation_id` is null because a `CHAT_MESSAGE` is user-originated, per
[Protocol Index §6](00-protocol-index.md#6-envelope). The host binds
`correlation_id` to the `tsk_01JQ8Z9K3M7QWXR4V2T6YH0BNA` it mints on receipt; a
device that pre-populates it has produced an invalid request and the field is
dropped.

### 5.1 Device message kinds

| Kind | Direction | Purpose |
| --- | --- | --- |
| `CHAT_MESSAGE` | device → host | User-originated instruction. Creates a `USER_REQUEST` task with `kind: USER_REQUEST` ([Task Protocol §2](02-task-protocol.md#2-assistanttask)). |
| `APPROVAL_RESPONSE` | device → host | The user's answer to an `ApprovalRequest`, or a confirmation of one already granted. |
| `NOTIFICATION_ACK` | device → host | The user opened or dismissed a notification. Used for engagement accounting, never for authority. |
| `TIMELINE_PAGE_REQUEST` | device → host | Fetch the next page of the activity timeline from a `seq` cursor. |
| `DEVICE_CAPABILITY_REPORT` | device → host | This device's current capability surface (§6). |
| `DEVICE_EVENT` | device → host | A device-side observation: connectivity change, battery, root state, app lifecycle, `ELEVATED_DEVICE` completion attestation. |
| `RECONNECT` | device → host | Resume request carrying the last consumed `seq` so the timeline continues without a gap. |
| `APPROVAL_REQUEST` | host → device | A rendered approval, carrying `plain_summary` and the arguments preview. |
| `TIMELINE_PAGE` | host → device | A page of timeline items plus the next cursor. |
| `CHAT_DELTA` | host → device | An incremental assistant answer or tool-progress note for an active chat turn. |
| `TASK_STATE_CHANGED` | host → device | Task moved to a new state; drives the timeline badge and pending-approval UI. |
| `SESSION_ENDED` | host → device | The host is closing the session, with a reason code. |

### 5.2 `APPROVAL_RESPONSE`

```json
{
  "envelope_version": "1",
  "surface": "serea.device/1",
  "message_id": "evt_01JQ8ZM7P6WXR2K5TY9BN4MQ8V",
  "correlation_id": "tsk_01JQ8Z9K3M7QWXR4V2T6YH0BNA",
  "causation_id": "evt_01JQ8ZK9S3QMW7T4XZ6CD8NRP2",
  "issued_at": "2026-10-01T09:15:02.774Z",
  "data_class": "PERSONAL",
  "trace": {
    "task_id": "tsk_01JQ8Z9K3M7QWXR4V2T6YH0BNA",
    "step_id": "stp_01JQ8Z9M5T9WXK2H4BNPQ7RDSF"
  },
  "payload": {
    "kind": "APPROVAL_RESPONSE",
    "device_id": "dev_01JQ8ZC5N8TVG3K6MRQ2XW9JHF",
    "session_id": "ses_01JQ8ZG9V5MXK3N7QW2RTF8YHB",
    "approval_id": "apr_01JQ8ZA1D4NFG8K2M6RTV9XCWB",
    "decision": "GRANT",
    "grant": {
      "max_uses": 1,
      "expires_at": "2026-10-01T09:45:02.000Z"
    },
    "biometric": {
      "performed": true,
      "method": "BIOMETRIC_STRONG",
      "attestation_digest": "sha256:3b7e0d4f8a2c6b9e1d5f3a7c0e4b8d2f6a9c1e5b7d3f0a8c2e6b4d9f1a7c3e5b",
      "auth_time": "2026-10-01T09:15:02.701Z"
    },
    "denial_reason": null,
    "client_seq": 4183,
    "signature": {
      "alg": "ed25519",
      "key_id": "dev_01JQ8ZC5N8TVG3K6MRQ2XW9JHF",
      "value": "Rt5pWq8nZ2mKd6vXc9lGb4hS0jYfE3uIaN1zQtW7kBz6Mg2xCeV0pLr8dYs5HfA3iQ7wSb2Ne5Ru0ZxKc8Vp1Mt6"
    }
  }
}
```

Rules that the host enforces on receipt, without exception:

- `decision` ∈ `GRANT`, `DENY`, `DEFER`. `DEFER` extends neither `expires_at`
  nor `max_uses`; it is a request to re-render, and re-rendering is still
  bounded by the request's own `expires_at`
  ([Approval Protocol §2.2](05-approval-protocol.md#22-expiry-of-the-request)).
- `grant.max_uses` and `grant.expires_at` are **ceiling proposals**. The host
  clamps them to the request's own bounds and rejects a response that tries to
  exceed either. A device cannot widen an approval by editing the JSON.
- For an `ELEVATED_DEVICE` capability, `biometric.performed` must be `true` or
  the grant is discarded with `APPROVAL_BIOMETRIC_REQUIRED`. There is no
  host-side waiver.
- A response whose `approval_id` is not `PENDING` is dropped: a duplicate
  response is idempotent, a late one is refused, and a second `GRANT` never
  consumes two uses
  ([Approval Protocol §4.2](05-approval-protocol.md#42-consumption-is-atomic)).

---

## 6. Device capability reporting

The device reports what it can currently do. The report is **data, not
authority**: it informs the capability registry, it never grants anything.

```json
{
  "kind": "DEVICE_CAPABILITY_REPORT",
  "device_id": "dev_01JQ8ZC5N8TVG3K6MRQ2XW9JHF",
  "session_id": "ses_01JQ8ZG9V5MXK3N7QW2RTF8YHB",
  "reported_at": "2026-10-01T09:14:22.010Z",
  "device": { "manufacturer": "Google", "model": "Pixel 7a", "android_sdk_int": 37, "android_release": "17", "root_available": false, "secure_context": true },
  "non_root_capabilities": [ "notification.post", "device.media.control", "device.screenshot.capture", "device.battery.read", "device.calendar.open" ],
  "root_capabilities": [],
  "data_class": "PUBLIC"
}
```

### 6.1 Reporting rules

| Rule | Reason |
| --- | --- |
| Non-root and root capabilities are reported in **separate arrays**, never merged | The host must be able to answer "can this do it right now?" and "can this do it with root?" without inference. |
| The report is bound to the `SessionId` and to `reported_at` | A report older than 10 minutes or issued under a previous session is discarded. |
| The report is re-sent on every session open and on every root-state change | Devices change their answers; cached reports rot. |
| `root_available: false` makes every root capability resolve to `CAPABILITY_UNAVAILABLE` | Matches [Capability Protocol §3.1](01-capability-protocol.md#31-field-semantics) and invariant C9. |
| Root and rootless variants of an `OPTIONAL_ROOT` capability share one `CapabilityId` and differ only by `implementation_id` | Registration, not device negotiation, decides which one is bound. |
| A capability absent from both arrays is treated as unavailable for that device | Absence is a denial, never a "probably supported". |

### 6.2 The report grants nothing

A device that claims `notification.post` does not thereby acquire the ability to
post notifications outside the host's pipeline, and a device that claims a root
capability does not thereby acquire root authority. The registry consults the
report for *availability* only; `risk_class`, `required_authorization`, and
`root_requirement` come from the `CapabilityDescriptor`
([Capability Protocol §3](01-capability-protocol.md#3-capabilitydescriptor)),
which the device cannot influence.

---

## 7. Notifications

> A notification tells the user that something happened. It never becomes the
> thing that happened.

Host events become Android notifications through a fixed channel set, keyed to
the `RiskClass` of whatever is being surfaced.

| Channel | `id` | Posted for | Importance | Inline actions |
| --- | --- | --- | --- | --- |
| Approvals | `serea.approvals` | `APPROVAL_REQUIRED` (`EXTERNAL_WRITE`, `COMMUNICATION`, `ELEVATED_DEVICE`) | `HIGH` | **None.** Tapping opens the in-app approval screen. |
| Security | `serea.security` | Pairing requests, device revocation, credential rotation, `POLICY_CHANGED` | `HIGH` | **None.** Tapping opens the security surface. |
| Chat | `serea.chat` | `CHAT_DELTA` for a turn the user is waiting on | `DEFAULT`, raised to `HIGH` while the task is in `WAITING_USER` or `WAITING_APPROVAL` | **None.** Tapping opens the conversation. |
| Watcher | `serea.watcher` | Proactive proposals (`PROPOSAL_CREATED`) | `LOW` | **None.** Tapping opens the proposal. |
| Activity | `serea.activity` | Task started, step completed, task terminal | `LOW` | **None.** |
| Diagnostics | `serea.diagnostics` | Bound exhaustion, degraded provider, reclaimed lease | `MIN` | **None.** |

**Absolute rule: no notification action causes or authorizes an effect.** Every
actionable notification navigates to a foreground screen where the normal
pipeline applies. In particular:

- A high-risk approval is **never** grantable from the notification shade.
  Granting requires rendering the full approval screen with `plain_summary` and
  the arguments preview
  ([Approval Protocol §2.1](05-approval-protocol.md#21-the-prompt-must-be-specific-enough-to-consent-to)),
  and for `ELEVATED_DEVICE` a `BiometricPrompt` confirmation. A notification
  that lets the user approve without seeing the screen has removed the
  consent.
- Notification bodies are rendered from the same redacted projection as any
  other outbound surface. A `PRIVATE`-or-higher payload is redacted before it
  reaches the notification
  ([Data Classification §6](09-data-classification-protocol.md#6-redaction));
  a notification never carries a secret.
- A notification can be dismissed, and dismissal is `NOTIFICATION_ACK`, which
  is engagement accounting only. It never cancels a task, revokes an approval,
  or acknowledges an effect.

Notifications are a convenience mirror of the timeline. If the device shows
nothing, nothing was lost: the activity timeline (§8) is the authoritative
record.

---

## 8. Activity timeline feed

The timeline is the device's window onto the event stream. It is paginated by a
monotonic `seq` cursor and is resumable across reconnects.

```json
{
  "kind": "TIMELINE_PAGE_REQUEST",
  "device_id": "dev_01JQ8ZC5N8TVG3K6MRQ2XW9JHF",
  "session_id": "ses_01JQ8ZG9V5MXK3N7QW2RTF8YHB",
  "after_seq": "18422",
  "limit": 50,
  "filter": { "task_id": null, "min_risk_class": "OBSERVE" }
}
```

| Field | Contract |
| --- | --- |
| `after_seq` | Cursor, exclusive. `null` means "from the beginning". Serialized as a decimal **string** because `seq` may exceed JavaScript's exact integer range ([Protocol Index §5](00-protocol-index.md#5-serialization)). |
| `limit` | `1..200`, default `50`. A page shorter than `limit` means the stream was caught up as of the page's `seq`. |
| Response | A page of items plus `next_seq`, which is `null` at the live edge. It also includes `history_status: AVAILABLE`, `EXPIRED`, or `INTEGRITY_ERROR` and the relevant oldest retained sequence or missing sequence information. |

### 8.1 Resumability

The device persists `last_rendered_seq` in local durable storage, updated after
each successfully rendered page. On `RECONNECT` the device sends that cursor and
the host replays from it. Re-delivery is permitted and deduplicated by event id;
a cursor never silently skips a committed, retained event.

When the host returns `history_status: EXPIRED`, it emits
`EVENT_HISTORY_EXPIRED` and supplies the oldest retained `seq`. The device shows
an explicit history-expired marker, sets its next exclusive cursor to the value
immediately before that oldest retained sequence, and fetches again. The marker
means older history was removed under normal retention; it does not claim a
commit was lost.

When the host returns `history_status: INTEGRITY_ERROR`, it emits
`EVENT_SEQUENCE_CORRUPTION` and identifies the missing sequence/range. The device
keeps its last verified cursor, shows an integrity warning, and stops advancing
or replaying beyond the gap until host repair is complete. It must not relabel
this as history expiry or acknowledge unseen later events as contiguous.

### 8.2 Unknown event kinds

An event kind the device does not recognise is **skipped, never fatal**:

- The device advances its cursor past the item, increments a local
  `skipped_kinds` counter keyed by kind name, and continues the page.
- It does not raise, does not show an error state, and does not stop the feed.
- This is required by
  [Protocol Index §4.2](00-protocol-index.md#42-compatibility-rules) rule 4: a
  new event kind is a minor architecture change and older clients skip unknown
  kinds rather than failing the stream.
- Unknown *fields* within a known kind are ignored, by the same rule and the
  same reason.

The device renders the subset of kinds it knows. It is not required to
understand the whole event vocabulary to be a correct client.

---

## 9. Offline and degraded operation

The device is designed to be useful with no connection and is designed to be
**powerless** with no connection.

| Capability with no connection | Behaviour |
| --- | --- |
| Read the cached timeline | Available. The last pages remain rendered. |
| Read cached provider data | Available, from the device-side cache only. |
| Type a chat message | Buffered locally, up to 50 messages, rendered with a visible *queued, not yet submitted* marker. Nothing is executed and nothing is claimed to have happened. |
| Grant an approval | Unavailable, always (§9.1). |
| Answer an approval prompt already on screen | Refused. The device discards it and re-requests on reconnect, so no stale-timeout grant is possible. |
| Control media, capture a screenshot, drive any `ELEVATED_DEVICE` action | Unavailable. These are host-caused, not device-initiated. |

### 9.1 Approvals cannot be pre-granted offline

There is no "remember my answer" for approvals, and no queue of approvals
waiting for a connection. This is not a UX limitation; it is the property that
makes the approval protocol safe:

- Pre-granting would create an `ApprovalGrant` with a use ceiling and an expiry
  that is consumed without a human present — precisely the standing authority
  [Approval Protocol §3.1](05-approval-protocol.md#31-the-six-bounds) exists to
  forbid.
- An approval granted on a disconnected device cannot be bound to a verified
  secure context, which `ELEVATED_DEVICE` requires
  ([Approval Protocol §5](05-approval-protocol.md#5-device-bound-approval)).
- A queued approval silently answers a question whose facts may have changed:
  the calendar may now be locked, the draft may no longer exist, the user may
  have changed their mind.

### 9.2 Pending approvals survive a disconnect

An approval request already delivered to the device is durable host state. On
reconnect the host re-renders every pending approval against the live device
roster and the live argument digest. The device re-renders it as pending. It is
**never** auto-granted and **never** auto-denied by the reconnect
([Task Protocol §6](02-task-protocol.md#6-recovery)).

If the underlying arguments changed while the device was away, the host
invalidates the old request and raises a new one, so a user never approves a
request whose preview is stale. Requests that outlive their own `expires_at`
become `EXPIRED` and the step fails with the reason recorded; an expired
approval is not a denial and not a retry.

---

## 10. Invariants summary

| # | Invariant |
| --- | --- |
| D1 | The device initiates every connection; the host never dials a device and no inbound device port is ever opened. |
| D2 | The device is a client and an authority in no respect; no device message causes or widens an effect. |
| D3 | The device private key is generated in Android Keystore, is non-exportable, and never reaches the host or any Serea process. |
| D4 | Every device frame is signed with the device key and verified per frame; the host key fingerprint is pinned at pairing. |
| D5 | Delivery is ordered and at-least-once, and every receiver deduplicates by `message_id`, which is never reused. |
| D6 | A session authenticates the device only; `ELEVATED_DEVICE` approval additionally requires an on-device biometric confirmation with no fallback path. |
| D7 | Pairing is one-time and single-use; the first device pairs from the host surface and every later device requires explicit host-side confirmation. |
| D8 | Revocation is a host decision; a revoked or unpaired device is refused, and a burned `DeviceId` is never reissued. |
| D9 | `DEVICE_CAPABILITY_REPORT` informs availability and grants nothing; absence of a capability means unavailable, not probably supported. |
| D10 | No notification action causes or authorizes an effect; high-risk approval requires the full in-app screen. |
| D11 | Unknown timeline event kinds are skipped and never fatal, and the timeline cursor resumes across reconnects without gap or duplication. |
| D12 | Approvals are never pre-granted, queued, or auto-resolved offline; pending approvals re-render on reconnect and are never auto-granted or auto-denied. |