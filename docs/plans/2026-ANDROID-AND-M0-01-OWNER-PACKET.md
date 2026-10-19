# AND-M0-01 Android direct-provider architecture candidate

Date: 2026-10-19 · Status: implementation candidate; production cloud disabled

This records the Owner's architecture selection for the Serea Android Beta. It
does not mark ADR-0044 or ADR-0045 Accepted and does not approve credentials,
external transmission, a paid plan, or distribution.

## Owner decision

- **Selected architecture:** Android connects directly to a cloud model
  provider through the existing Ollama Cloud Adapter.
- **Dedicated Serea Backend:** not a required component.
- Android owns conversation identity, send/cancel behavior, credential
  references, encrypted local history, usage tracking, and user deletion.
- UI calls `ModelChatPort`; it does not call a provider adapter directly.
- A gate-issued payload is required by the Ollama adapter. Ordinary chat
  credentials and provider transport remain unavailable from the shipping UI.
- Mock HTTP is a loopback-only test implementation. It is not evidence of a
  successful live provider request.

The official [Ollama Terms of Service](https://ollama.com/terms), last updated
May 2026, requires users to be at least 18 years old. Provider eligibility has
not been established for this project. The app's Cloud connection action stays
disabled. No account, key, authenticated connection, user-data transmission,
provider billing, or subscription operation was performed for this candidate.

The official [Ollama chat API](https://docs.ollama.com/api/chat) documents
`POST /api/chat`, a `messages` history array, streaming by default, and
`prompt_eval_count` / `eval_count` usage fields. The official
[Gemma 4 cloud listing](https://ollama.com/library/gemma4:31b-cloud) shows
`gemma4:31b-cloud` as the cloud entry and uses `gemma4:31b` in its API example.
The candidate model ID remains provisional. These sources establish an API
shape and candidate listing, not account entitlement or permission to transmit
Serea user data.

## Unapproved decisions

- Provider account eligibility and all provider terms applicable to the
  eventual deployment.
- Which ordinary user-authored data may be classified PUBLIC, preparation and
  minimization rules, confirmation and expiry requirements, and any PERSONAL
  data preparation process.
- Provider retention, processing region, data handling, and model roster.
- Local history retention and deletion disclosure.
- Provider-side budget / spend limit and billing owner. Local limits are
  best-effort application soft limits and cannot guarantee a bill ceiling.
- Credential enrollment, supported device policy, distribution channel, and
  privacy disclosures.
- Production Cloud mode activation. ADR-0044/0045 remain PROPOSED.

## Candidate implementation scope

- A typed egress contract carries conversation and message IDs, origin,
  classification, destination, exact planned body, completed context, data
  preparation state, explicit send-intent timestamp, and expiry. User-authored
  text remains UNKNOWN by default; no string scan or confirmation text claims
  to prove that it contains no secrets.
- Context retains source IDs, origins, classifications, status and conversation
  identity. Only COMPLETED records from the active conversation are eligible.
  The current budget uses a deliberately conservative character ceiling; it is
  not exact model tokenization.
- Provider requests reserve daily and UTC-calendar-month request/token budgets
  transactionally before dispatch. Missing usage, timeout, cancellation, or
  process interruption keeps the reservation charged and marks the outcome
  uncertain. Exact provider usage is shown separately from local estimates.
  Device time can be advanced by a user or compromised OS; this soft limit is
  neither tamper-resistant nor provider billing enforcement.
- Ordinary chat and the fixed Cloud probe are disabled from the UI. The UI can
  display a local content preview with source and classification and states
  that a preview does not prove the content is safe to transmit.
- A debug-only ADB harness stages persistent encrypted history, lets the host
  force-stop the installed app, launches it again, and checks pending recovery
  and deletion. It is not an instrumentation test runner and its final result
  must be reported separately from JUnit XML.

## Validation ledger

Validation snapshot: code head `0cce771fc5927efb6b0fdf19739a6125e46d9827`
(PR #45, before this validation-ledger-only documentation commit). Final PR
head checks are rerun after this update; use those checks as the final evidence.
On each API, JUnit XML must record at least 21 tests with zero failures,
errors, or skips and exact class counts: EncryptedHistory 16, ChatScreen 4,
ChatHttpE2E 1.

| Area | Result |
| --- | --- |
| JVM / Android Unit | PASS — 9 tests, 0 failures/errors/skips (Android run 39227204386) |
| Mock HTTP / Android Integration | PASS — 18 tests, 0 failures/errors/skips (Android run 39227204386) |
| Egress / Android Security | PASS — 8 tests, 0 failures/errors/skips (Android run 39227204386) |
| API 35 instrumentation | PASS — 21 tests: EncryptedHistory 16, ChatScreen 4, ChatHttpE2E 1; 0 failures/errors/skips (Android run 39227204386) |
| API 26 instrumentation | PASS — 21 tests: EncryptedHistory 16, ChatScreen 4, ChatHttpE2E 1; 0 failures/errors/skips (Android run 39227204386) |
| Real OS Process Death | PASS — API 26 and 35 each survived two host-issued force-stop/restart cycles; completed history restored, pending marked outcome-unknown without replay, and deletions persisted (Android run 39227204386) |
| 100 conversation / 100 message history stress | PASS — both deterministic Room stress cases ran in EncryptedHistoryTest on API 26 and 35 (Android run 39227204386) |
| Streaming fragmentation stress | PASS — Mock HTTP integration suite, including fragmented streaming response coverage (Android run 39227204386) |
| Pixel 7a physical-device acceptance | NOT_RUN until a physical device is available |
| Live Provider E2E | NOT_RUN / BLOCKED pending eligibility and Owner activation |

## Pixel 7a acceptance checklist

- Verify install, first launch, and restore on Android API 35.
- Send a multi-turn local Demo conversation and confirm each turn is saved once.
- Rotate the device and verify completed history and draft behavior.
- Kill and relaunch the app from device settings/ADB without clearing app data;
  confirm completed history, pending outcome state, and no replay.
- Confirm Delete This Conversation and Delete All remain deleted after restart.
- Confirm Cloud controls remain disabled, the label says Demo/Mock, and ordinary
  text is never sent to a provider.
- Confirm settings screenshots are suppressed while credential controls are
  visible and check Window/IME inset behavior.
- Verify TalkBack labels and text scaling on the message list and send control.
- Verify no credentials, request bodies, or provider raw error bodies appear in
  logs or backup.
- Do not test authenticated provider access as part of this acceptance list.

Physical-device result: **NOT_RUN** in the current Cloud Linux environment.

## Roadmap

- **2026-11-08 M0 AI connection:** Mock transport and direct-adapter boundaries
  can be validated without live egress. Real connection remains blocked by
  eligibility, user-data classification, terms, and separate Owner activation.
- **2026-11-22 history/recovery:** Room history, recovery states, usage
  reservations, API 26/35 instrumentation and ADB Process Death evidence are
  the priority; real-device confirmation is still needed.
- **2026-12-31 Beta:** remains conditional on the unapproved provider, data,
  retention, billing, device, and distribution decisions and final CI evidence.

Next priority: use final-head CI evidence to close the candidate validation,
then resolve provider eligibility, data classification/minimization, retention,
provider processing and budget, distribution, and production activation as
separate Owner decisions. Keep live egress disabled while those decisions are
open.
