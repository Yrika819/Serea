# Serea Android Phase 2 Owner decision packet

Status: Decisions requested before ordinary cloud chat or Beta distribution.
ADR-0044 and ADR-0045 remain Proposed.

## P0 — User data egress and service model

**Recommendation:** keep ordinary chat local until a written rule distinguishes
user-authored cloud intent from device-derived or secret data. Approve a
provider and either direct mobile access or a backend, then define an explicit
per-send destination and content review. The current prototype only permits a
fixed public probe and refuses all user history/text.

**Alternative:** authenticated backend. It centralizes keys and account-level
limits but introduces service operation, account, and data-processing costs.
Direct access is simpler to operate, but provider keys are distributed to
devices and local controls cannot guarantee billed spend.

**Risk:** a user typing text or checking a confirmation box cannot prove that
the content has no personal or secret information. Arbitrary user text remains
blocked until a class and preparation contract is approved.

## P0 — Credential and history policy

**Recommendation:** retain Keystore AES-GCM and backup exclusion. The Cloud
credential path now refuses software-backed Keystore and requires the observed
StrongBox/TEE profile. Decide whether to keep that requirement or approve a
software-backed fallback. Decide recovery behavior on key loss, retention
period, delete-all semantics, and whether local database metadata is acceptable
in plaintext. Current code encrypts title/body, but conversation IDs, model,
classification, status, and timestamps are metadata in Room.

**Risk:** Keystore does not protect against a live compromised OS, screenshots,
or accessibility capture. Deleting local records does not prove physical
flash erasure. Lost key data cannot be recovered by this candidate.

## P0 — Provider account, pricing, and release

**Recommendation:** before real use, verify the owner's account terms, model
availability, API-key revocation path, provider retention and training terms,
processing region, and provider-side spending cap. Choose personal APK,
managed private distribution, or public release and approve matching privacy
disclosures.

As checked on 2026-10-18, Ollama's public pricing page listed a Free tier with
starter credits/models (not unlimited) and `gemma4:31b` at $0.14 per million
input tokens / $0.40 per million output tokens. Provider terms/prices may
change. The local prototype's timeout/output cap is not a billing cap. The app
cannot guarantee a total bill limit without provider-side account controls.

## Accepted engineering scope for this candidate

- Ollama Cloud HTTPS contract probe only, using a fixed PUBLIC test sentence.
- No live key or provider call in CI, and no ordinary chat or history egress.
- API key stored encrypted by Android Keystore; masked settings entry and
  local deletion action. Account-level revocation remains in the provider UI.
- Local encrypted title/body and Room history with delete controls.
- No change to frozen `serea.device/2`, Rust Core, or P7/P8.

## Owner action

Record decisions by amending and accepting the appropriate ADRs. Until then,
keep the PR Draft and do not authorize ordinary external data transmission,
merge, or public distribution.
