# ADR-0044: Android independent chat mode

Status: **PROPOSED / NOT RATIFIED** · Date: 2026-10-18 · Scope: Android standalone Beta

## Context

The frozen `serea.device/2` protocol defines an Android client paired to a PC
Serea Core. It does not define a standalone Android conversation client.
Android standalone chat must not be presented as a `device/2` implementation,
and it must not reuse or expose the PC host's `PreparedModelCallV1::from_host`
or `ModelRouterProcessV1::execute` trust boundary. This proposal starts a
bounded Android-only path. It does not ratify a production backend or provider.

## Proposed decision

### Responsibility, ownership, and lifecycle

The Android application owns its standalone conversation identity, local
conversation/message records, user input, model choice and settings, send/cancel
state, history retention, and user initiated deletion. Each record has an
opaque conversation/message ID, creation time, classification, and optional
expiry/deletion metadata. The Android installation is the owner of this data;
there is no PC OwnerId, task lease, worker, event journal, or remote session.
The user can delete a conversation and request deletion of all local history.
After a deletion request, UI and local repositories must stop returning those
records; secure physical erasure is not promised by this ADR.

Offline use may display locally stored history, but cannot perform inference.
Cancellation stops UI waiting and suppresses late responses; the provider
contract must define whether an already transmitted request can be canceled.
Requests have a bounded timeout, never retry implicitly, and one user send
creates at most one provider request. A timeout is an indeterminate remote
outcome and requires an explicit user retry.

### Data classification and cloud boundary

Classification is carried as typed metadata, never inferred solely from
substrings or a UI warning. Unknown classification fails closed. `SECRET` and
`CREDENTIAL` are never model inputs. `PRIVATE` is not sent to cloud models.
`PERSONAL` may be sent only from a trusted preparation path after minimization,
redaction, and an explicit egress decision; unprepared user text is not assumed
safe because the user typed it. `PUBLIC` may be sent after the gate. A data
source permission (for example, reading Calendar) is separate from permission
to include its contents in a model request. Calendar and Gmail access are not
implemented here.

### Credentials, keys, backups, and threats

No API key, OAuth token, account sign-in, or credential UI is in Phase 1. A
future credential port must keep secret bytes outside ViewModels, history,
saved state, logs, crash reports, analytics, and request DTOs, and must expose
only opaque handles to model code. Android Keystore is the required protection
boundary for device-held secrets; non-exportable, hardware-backed keys should
be used when available, with capability reported rather than assumed. Backup
and device transfer must exclude credential ciphertext and key material.
Restored credential references are invalidated and require reauthentication.
Keystore use does not protect against a live compromised/rooted OS, an unlocked
device, screenshots, or a malicious accessibility service. Rooted/lost-device
response and production key recovery need a separate security review.

### Provider choice and timeouts

Direct Android-to-provider HTTP offers a smaller service footprint but exposes
provider credentials to a distributed mobile client and makes cost limits,
revocation, and policy enforcement harder. An authenticated backend centralizes
provider secrets and policy but adds service availability, account, and data
processing obligations. Neither option is selected. AND-04 must choose a
provider/backend, retention and training terms, account/auth model, spending
limits, timeout budget, cancellation semantics, privacy disclosures, and
release channel before enabling network calls. All provider calls must flow
through `ModelChatPort`, a credential port, and a fail-closed egress gate.

### PC compatibility and authority

Standalone Android chats are not `serea.device/2` sessions and do not change
that frozen protocol. No sync or pairing protocol is proposed here. Any later
PC integration requires a separate ADR defining compatibility, identity,
conflict resolution, merge/deletion propagation, and user consent. Android
does not inherit PC OwnerId, Worker, Task Lease, Approval Grant, Permit,
Capability, or Provider invoke rights. Android standalone mode contains no
route to execute PC capabilities or effects.

### Distribution and adjacent features

Calendar OAuth, if later approved, should begin with least-privilege read-only
scopes; access to read data does not grant model egress. Gmail is a conditional
additional goal. Email sending, Calendar writes, autonomous effects, PC
connection, Local MCP/GoalLatch, P7 long-term Memory, and P8 production
Capability execution are out of scope. Public Play release, private testing,
and personal APK distribution have different OAuth verification, disclosure,
and review obligations; the release channel remains undecided.

## Consequences

Phase 1 can ship a visibly labeled local Demo without implying cloud inference.
The interface and data model can support a later provider without selecting
one. History persistence, provider transport, credential storage, OAuth,
backup, and sync remain disabled until their separate decisions and tests are
complete.

## Open decisions for the Owner

1. Direct provider or authenticated backend; provider and model roster.
2. Which user message classes may be sent, and what trusted preparation can
   establish PERSONAL eligibility.
3. History retention, local encryption, backup exclusion, deletion semantics,
   and crash/logging policy.
4. Credential storage and recovery requirements; supported device security
   level and response to device loss/root compromise.
5. Timeout, cancellation, retry, spending, and provider outage behavior.
6. Google OAuth scopes, release channel, privacy disclosures, and review gates.

## References

- [Device Protocol](../protocols/07-device-protocol.md) (`serea.device/2`, frozen)
- [Model Protocol](../protocols/03-model-protocol.md)
- [Data Classification Protocol](../protocols/09-data-classification-protocol.md)
- [ADR-0007](ADR-0007-rootless-first-android.md), [ADR-0010](ADR-0010-data-classification-and-credential-exclusion.md), [ADR-0031](ADR-0031-model-roster-routing-and-egress-v1.md), [ADR-0043](ADR-0043-p7-gate-b-limited-v1-and-gate-c-authorization.md)
