# P7-D01 owner authentication technical contract (PROPOSED)

**Status:** Proposed for owner selection; not ratified. **Implementation:** Not authorized.
**Baseline:** ADR-0042 accepts one local V1 Owner, host-verified OS authentication, stable opaque `OwnerId`, immutable Task admission binding, no device/session inference, and quarantine of unprovable legacy Tasks. This document fills in a recommendation only.

## Recommended flow

1. A host-owned `OwnerIdentityService` provisions exactly one Owner record after the host authentication adapter returns a fresh-authentication result. The service—not a device, client, model, Task request, or storage caller—creates the ID and persists it in a single SQLite `BEGIN IMMEDIATE` transaction. Concurrent first-run attempts serialize; the second either observes the same Owner or fails closed if identity evidence differs.
2. `HostAuthenticator` is an explicit platform port. Its input is a purpose string (`owner-bootstrap`, `task-admission`, `forget`, `renew-memory`, or `owner-recovery`) and a freshness policy. Its output is a sealed internal `AuthEvidence` with `principal_handle`, `method`, `verified_at`, `fresh_until`, `assurance`, `challenge_nonce`, and `adapter_version`; no password, biometric template, OS token, or credential material is retained. The host adapter validates evidence; application code cannot construct the successful variant.
3. A logged-in desktop session means the OS session is active. It is not fresh authentication. Operations marked fresh require host confirmation for that operation, with a proposed maximum age of 5 minutes, a single-use nonce, purpose binding, and no silent reuse after restart. Exact platform behavior must be verified per adapter.
4. A successful authentication yields an ephemeral `AuthenticatedActor { owner_id, auth_context_id, evidence_expiry }`. The request cannot supply or override `owner_id`. Task admission receives that actor through the trusted Core/TaskEngine call chain and writes the Task and its immutable owner binding in the same Storage transaction as Task insertion. Missing actor or owner mismatch rejects admission; there is no fallback to origin DeviceId or SessionId.
5. Device sessions are separate rows keyed by DeviceId/SessionId and may record a relationship to the Owner only after explicit host-controlled pairing. A session is never authentication evidence, and a registered device key proves possession of that key only, not human presence. Remote session requests carry no Owner authority by themselves.

## OwnerId lifecycle

**Proposed exact grammar:** ASCII `own_` followed by exactly 32 lowercase hexadecimal characters encoding 128 CSPRNG bits (UUIDv4-quality randomness; version/variant bits are not required). SQL check: `^own_[0-9a-f]{32}$`. Unique in `owners.owner_id`. No timestamps, usernames, device identifiers, or personal data are encoded. Generation retries on a unique collision. IDs are never reused: retain a permanent minimal retired-ID registry/tombstone even when an Owner is replaced.

| State | Entry and allowed transition | Failure behavior |
| --- | --- | --- |
| `UNINITIALIZED` | No owner row; fresh host auth -> generate and atomically commit ID + identity epoch | No authenticated operations; concurrent bootstrap serialized |
| `ACTIVE(owner_id, epoch)` | Normal operation; OS authentication freshly verified for sensitive actions | Session login alone does not satisfy fresh-auth requirement |
| `AUTH_UNAVAILABLE` | Adapter missing, unsupported, or OS verification unavailable | Read-only non-Memory host behavior; deny admission that would create Owner-bound Task, Forget, renewal, and Memory access |
| `RECOVERY_REQUIRED` | Primary DB restored, Owner row missing/conflicting, key/device lost, or identity epoch mismatch | Global Memory read/search/extraction/sync barrier; preserve files for recovery |
| `QUARANTINED` | Legacy Task or unprovable Owner mapping | No Memory extraction from it; no inferred owner; retain ordinary Task policy separately |
| `RETIRED` | Explicit owner replacement approved and prior data disposition completed | Old ID permanently non-reusable; prior Memory remains inaccessible until explicit recovery/Forget disposition |

Recovery does not silently mint a replacement identity. Owner recovery requires fresh host authentication and an explicit owner-selected restore path. Device loss alone does not transfer authority. Credential reset, account change, or host reinstall may make prior identity unverifiable; absent a supported platform proof, quarantine rather than guessing. A backup restore may restore the same ID only if the independent recovery record and identity epoch reconcile; missing or older evidence keeps Memory inaccessible. A new Owner ID must never automatically inherit old Tasks or Memory.

## Additive schema sketch (not migration DDL)

```sql
owners(owner_id TEXT PRIMARY KEY, epoch INTEGER NOT NULL UNIQUE,
       state TEXT NOT NULL, created_at_ms INTEGER NOT NULL,
       retired_at_ms INTEGER);
tasks.owner_id TEXT NULL; -- NULL only for explicitly quarantined legacy rows
UNIQUE(tasks.task_id, tasks.owner_id); -- verified parent key where used
owner_task_bindings(task_id TEXT PRIMARY KEY, owner_id TEXT NOT NULL,
                    admitted_at_ms INTEGER NOT NULL,
                    FOREIGN KEY(task_id) REFERENCES tasks(task_id),
                    FOREIGN KEY(owner_id) REFERENCES owners(owner_id));
legacy_task_quarantine(task_id TEXT PRIMARY KEY, reason_code TEXT NOT NULL,
                       quarantined_at_ms INTEGER NOT NULL);
device_owner_links(device_id TEXT NOT NULL, owner_id TEXT NOT NULL,
                   paired_at_ms INTEGER NOT NULL, revoked_at_ms INTEGER,
                   PRIMARY KEY(device_id, owner_id),
                   FOREIGN KEY(owner_id) REFERENCES owners(owner_id));
```

The binding is insert-once and update/delete protected by triggers plus Storage API checks. A null Owner never means “current Owner”. Existing migrations 0001–0008 are immutable. Whether binding should be duplicated on `tasks` for query efficiency or only represented by a dedicated table remains an implementation choice subject to the composite-FK proof.

## Failure and revocation matrix

| Condition | Admission / Forget / renewal | Existing Memory | Recovery |
| --- | --- | --- | --- |
| Fresh OS auth unavailable | Deny | Read barrier for sensitive Memory actions | Retry only through verified host adapter |
| Logged-in session, no fresh challenge | Deny fresh-required action | No implicit grant | Prompt host authentication |
| Device key revoked/lost | Device session revoked; Owner unaffected absent explicit policy | No authority from key | Re-pair with fresh host auth |
| OS credential reset/account changed | Do not infer same human principal | Quarantine if adapter cannot prove continuity | Owner-reviewed recovery or new isolated Owner |
| Primary backup restored | Admission/search stopped pending ledger/epoch reconcile | Quarantined | Reconcile exact OwnerId+epoch; else remain blocked |
| Owner record absent or duplicate/corrupt | Deny | Global Memory barrier | Preserve and report recovery required |
| Legacy task owner unknown | No binding | No extraction or provenance attribution | Quarantine; bind only with independent, auditable proof |

## Platform review

- **macOS:** An adapter must use a host-verified user-presence/authentication facility with operation/purpose freshness. No API choice or support claim is made here; login state and keychain/device key possession are insufficient evidence by themselves.
- **Windows:** A future adapter must establish which local authentication UX and assurance/freshness result are available on supported editions and remote sessions. No API or compatibility claim is verified.
- **Linux development:** PAM/desktop-agent availability, headless/SSH sessions, and differing distributions prevent one assumed mechanism. Development fake credentials are test-only and must be impossible to enable in production builds.
- **Android future client:** A phone/device client is not the Owner authority in this V1 design. Android biometrics/device keys, root, and GoalLatch privileged actions are outside scope; pairing cannot infer or replace host Owner authentication.

## Negative tests required before implementation acceptance

Reject absent, expired, wrong-purpose, replayed-nonce, wrong-owner, malformed evidence; caller-supplied OwnerId; model-produced identity; Task admission without actor; task-owner update; owner delete/reuse; device/session-only authentication; host auth unavailable; fake credential in production configuration; concurrent bootstrap creating two owners; DB/ledger Owner epoch mismatch; replacement identity inheriting old rows; legacy backfill without proof; and a Task+binding partial commit. Verify that fresh auth is distinct from session login and a registered device key cannot satisfy human presence.

## Alternatives and rationale

- Derive OwnerId from OS username/account: rejected; usernames are mutable, platform-specific, and can expose identity.
- Treat DeviceId/SessionId or registered device key as Owner: rejected by ADR-0042 and does not prove human presence.
- Accept caller-provided OwnerId: rejected because it creates a confused-deputy path.
- One opaque random host-issued ID plus host authentication: recommended for stable identity separation and minimized disclosure.

**Security justification:** authenticated identity is established at the host boundary, propagated as non-forgeable trusted context, and bound with Task admission atomically.
**Known limitations:** local SQLite tampering and a fully compromised OS can falsify local state; no platform adapter or API support is verified; account continuity across reinstall is unresolved.
**Validation evidence:** repository inspection found no Owner schema or host-auth adapter; existing Task rows contain origin DeviceId but no Owner proof. Synthetic auth tests remain future work; this document is design only.
**Exact owner questions:** Accept this OwnerId grammar and issuer? Require fresh OS authentication for Forget and renewal, and with what freshness window? Which host OS versions and authentication assurance are V1 targets? What recovery authority may reconnect a restored Owner? May legacy rows be bound only by independent proof, with all others quarantined?
**Implementation gate:** Gate A recommendation only. Gate B requires explicit answers; Gate C requires separate P7B authorization.
