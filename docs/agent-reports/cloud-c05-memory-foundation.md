# Cloud C-05 — Conversation-derived Memory foundation

## Scope and base

Branch `cloud/serea-v13-c05-memory-foundation` starts from C-04R HEAD `8fef15004e8089ab2a7d99bbfbfdc837fb8b5da3`. This is disabled P7 Gate C engineering evidence only. ADR-0044 remains PROPOSED / NOT RATIFIED.

## Implementation

- Added a typed synthetic `CompletedConversation` input with owner and conversation IDs, turn state/role, classification, origin, and source lineage. Validation binds it to the existing P7 owner handle and rejects failed/in-progress conversations or unknown conversation ownership. Failed/incomplete, other-conversation, and null-conversation turns are omitted before they reach the extractor.
- The deterministic `MockMemoryExtractor` derives an episodic candidate from completed user turns. It is test-only and has no model or provider adapter. Candidate validation checks bounded content, exact source references, and inherited classification. PUBLIC/PERSONAL may be stored; PRIVATE, SECRET, CREDENTIAL, and UNKNOWN are refused or excluded. Packed source lineage, per-source class, and origin are retained in the existing provenance source field.
- Candidate IDs are deterministic for owner, conversation, and candidate key. The new transactional `insert_once` uses a request-plus-candidate idempotency key, refuses conflicting candidate content, and checks P7 tombstones before insertion. It uses the existing owner-scoped Memory tables, FTS triggers, quotas, expiry, and recovery readiness barrier. No second database, ordinary migration, or new schema was added.
- Candidate body size is bounded by the ADR-0043 accepted 4,096 UTF-8-byte ceiling. Extraction uses the accepted 20 starts/UTC-day and one concurrent extraction/Owner limits through P7's existing resource-accounting row. Existing `EXPERIMENTAL_PROVISIONAL` remains explicitly `TestOnly`; no other recommended value is promoted. Existing accepted retention periods are exercised through the existing MemoryKind expiry behavior.
- `preserve_chat_result` keeps an already-produced chat result when extraction or storage fails. The new module is nested under `experimental_memory` and compiled only for tests when both `p7-experimental` and `p7-gate-c` are explicitly enabled. It is absent from ordinary builds and does not inject Memory into model context.

## Verification

- Synthetic extraction tests: **8/8 PASS**. Covers candidate generation and FTS search, owner and conversation boundaries, turn filtering, classification inheritance and unknown/restricted inputs, request retries, multiple candidates per request, accepted daily/concurrency bounds, retention/body bounds, tombstone re-import refusal, and preserving the original chat result on extractor failure.
- Full P7-enabled storage suite: **618 unit tests PASS**, 2 foundation integration tests PASS, 2 Gate C lock integration tests PASS, 35 doc tests PASS. Two manual resource measurement tests remain ignored by their existing annotations.
- Default-feature storage suite: **511 unit tests PASS**, 2 foundation integration tests PASS, 35 doc tests PASS; P7 Gate C and experimental schema tests remain disabled.
- No real conversation, personal data, model/API call, OAuth credential, provider traffic, production DB, or context injection was used.

## Integration notes

The synthetic path intentionally stops at a private development module. Its extraction counter is a test-only use of P7's accepted accounting bounds, not a crash-recoverable production lease. Before any production conversation-memory integration, the host must supply reviewed owner authentication, crash-safe extraction admission/fencing, and the approved conversation persistence adapter. ADR-0043's production gates remain in force; this work does not enable Gate D or ordinary P7 migration 0009.
