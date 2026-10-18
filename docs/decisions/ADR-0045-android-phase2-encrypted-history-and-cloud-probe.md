# ADR-0045: Android Phase 2 encrypted history and bounded cloud probe

Status: **PROPOSED / NOT RATIFIED** · Date: 2026-10-18 · Scope: Android Phase 2 candidate

## Context

ADR-0044 leaves standalone Android provider access, credential handling, and
history policy unresolved. AND-04/05/06 need a testable transport and local
conversation foundation, but implementation work does not grant authority to
send user conversations or select a production service.

## Proposed implementation boundary

- Demo chat is local and has no network path.
- A separate Cloud Candidate setting can call only Ollama Cloud's fixed HTTPS
  `/api/chat` host and only with a fixed, non-personal contract probe. Ordinary
  user text, saved history, Calendar, and Gmail are not sent.
- The adapter rejects disabled mode, unsupported model IDs, absent/invalid
  credentials, oversized responses, redirects, and invalid streams. It does
  not log request content or provider error bodies and does not retry.
- The Android egress gate denies UNKNOWN, PRIVATE, SECRET, CREDENTIAL, and all
  PERSONAL input. A caller cannot construct an approved payload from arbitrary
  strings. Provider composition does not expose an unguarded general chat API.
- Credential ciphertext is app-private and protected with AES-GCM under a
  non-exportable Android Keystore key. History titles and message bodies use
  authenticated AES-GCM with record identity as associated data. Backup is
  disabled. If a required key is unavailable or data fails authentication,
  the app reports unavailable data and does not reset it silently.
- Room owns Android-only history. Database schema version 1 is exported;
  destructive migration fallback is not configured. Deletion removes rows and
  stops repository reads; cryptographic physical erasure is not claimed.

## Provider contract evidence

The adapter targets `POST https://ollama.com/api/chat`, bearer authentication,
NDJSON streaming, `message.content` deltas, and prompt/evaluation usage fields.
The candidate model ID is `gemma4:31b`, subject to current model availability.
Ollama's public pricing page showed $0.14 input / $0.40 output per million
tokens for that model on 2026-10-18; service tiers, prices, free credits,
regional processing, and terms may change. Local UI limits do not cap the
provider account bill. No authenticated live request was made for this
implementation.

## Consequences and limitations

This candidate makes transport, encryption, persistence, and error paths
testable without enabling production chat egress. Keystore software-backed
security is reported as software-backed; hardware backing is not assumed.
Android process memory, a compromised/rooted OS, screenshots, and accessibility
services remain outside the protection guarantee. App deletion, retention
period, secure erasure, account revocation, billing limits, and production
disclosures need explicit owner policy. A single supported model allowlist may
need updating as provider models change.

## Owner decisions required before enabling ordinary cloud chat or release

1. Approve a provider and deployment model: direct mobile connection or an
   authenticated backend.
2. Define which user-authored text can be sent, the trusted preparation and
   data-classification contract, confirmation UX, and refusal behavior.
3. Approve retention, deletion, encryption-key loss/recovery, backup, and
   supported software-backed Keystore policy.
4. Set provider-side spending limits and app-side soft limits, retry and
   cancellation disclosures, and credential revocation instructions.
5. Approve provider account/terms, model roster, region and retention facts,
   privacy disclosure, and personal APK versus public distribution.

## References

- [ADR-0044](ADR-0044-android-independent-chat-mode.md)
- [Ollama Cloud](https://docs.ollama.com/cloud)
- [Chat API](https://docs.ollama.com/api/chat)
- [Ollama pricing](https://ollama.com/pricing)
