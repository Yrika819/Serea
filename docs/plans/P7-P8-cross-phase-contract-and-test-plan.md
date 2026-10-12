# P7–P8 cross-phase contract and test plan

**Status:** Proposed implementation roadmap; no owner decision is ratified.
**Scope:** P7 durable Memory and P8 synthetic provider execution remain independently useful and may not introduce a cyclic dependency.

## Accepted layering and phase boundary

The verified P6 integration baseline is recorded in [P6 post-merge main verification](P6-post-merge-main-verification.md); P6A–P6F are merged and P7/P8 remain preimplementation.

The [Crate Map](../architecture/03-crate-map.md#1-the-layering-rule) fixes `serea-memory` at L3 alongside (below) `serea-task-engine`; it depends downward on Protocol, Storage, Event Bus and Model Router. Task Engine may call Memory; Memory never calls Task Engine, Capability or Policy. The provider implements the protocol port and never owns Policy/Approval. P8 orchestration stays in Task Engine/Core composition; Storage owns durable atomicity; `serea-capability` remains preparation/registry only and cannot depend on `serea-policy`.

Recommended one-way flow:

```text
P5 PreparedAction ──> P6 Policy/Approval ──> P8 TaskEngine dispatch gate
                                              │
                                              ├─ transaction → durable intent/use/budgets/events
                                              ├─ after commit → fake or later real provider
                                              └─ validate result + receipt → durable result/event
                                                                    │ optional, explicit
                                                                    ▼
                                                         P7 EXTRACTION request
                                                         memory → Store / FTS
```

There is no provider→Memory or Memory→provider dependency edge. P7 can store eligible user-authored/local facts without P8. P8 can execute fake-provider scenarios and commit receipts without Memory. Provider outcomes become Memory sources only through a separately requested, host-authored `EXTRACTION` step after P8 has validated the exact result/receipt.

## Provider result as an optional Memory source

1. P8 provider output is untrusted external data. It becomes a candidate source only after exact `RequestId`, intent, Task/Step, pinned descriptor output schema, canonical digest, provider reference and receipt validation succeed.
2. The source identity is immutable: use an `EventId` under the currently specified EventId convention for the P8 result/receipt event, plus original TaskId/StepId, RequestId, capability/provider/account binding and validated result digest in provenance. The EventId is a routing identity, not authority. Do not use provider-controlled IDs alone as source identity.
3. Extraction is a new host-created model request with `purpose=EXTRACTION`, source ID fixed by host and eligible projection fixed by host. It cannot be an implicit side effect of provider success or a model-selected action. The TaskEngine binds the extraction to its own Step and records source provenance as an immutable snapshot.
4. Classification is monotone: derived Memory gets the highest class of request, result, receipt metadata, source provenance and extracted content. Redaction changes only transit projection; it does not lower stored class. Under proposed P7-D04, PRIVATE results and PRIVATE provenance fail closed; SECRET/CREDENTIAL never enter extraction or ordinary Memory. Credentials are never present in ActionResult or Memory.
5. External content is data, not host authority. Any instruction-like text returned by a provider is quoted/untrusted content in the extraction input; it cannot set owner, tool, policy, grant, retention, classification or follow-up action. Extraction output is schema-validated and cannot contain executable host instructions as a privileged field.
6. Source Task deletion under normal retention does not delete retained Memory. Explicit forget resolves the retained provenance snapshot and derived supersession closure. Source EventId may itself age out under event retention; retained provenance must still distinguish “source event expired” from “never had source.” No live FK from retained Memory to an expiring event/task/step.

## Event, Blob and retention ordering

- P8 success transaction commits validated receipt/result references, Step/Task progress and result event atomically. Only after commit can a separate explicit EXTRACTION Step consume that result event. A result event cannot precede an uncommitted receipt/success.
- P7 memory item, immutable provenance, owner binding checks, FTS row, supersession edge, event and any Blob reference commit in one SQLite transaction. Task deletion and Memory expiration are separate GC domains.
- Blob identity remains exactly `(digest,data_class_rank)`. Shared content is deleted only after no Memory, Task, result, event, scheduler, receipt or other durable row refers to that exact pair. Cross-class same digest is a distinct row; GC never collapses class. PRIVATE Blob references still need protection, while ordinary-row metadata is separately protected.
- FTS contains only class-approved search projection and is rebuilt from authoritative Memory rows. Search applies authenticated OwnerId and class/transit eligibility from base rows in the same query; stale FTS cannot authorize or disclose a row.
- Forget must atomically remove selected Memory/provenance/FTS/event state and create a content-free tombstone. It cannot claim to remove old SQLite pages, WAL content, exported snapshots or backups. Backup restore needs a durable deletion/tombstone reconciliation source before content is made searchable again.
- Retention GC is bounded and resumable. It never runs a long SQLite write transaction over provider IO, FTS rebuild, or a whole database. P7 memory expiry may delete a Blob only after cross-domain reference accounting. P8 dispatch intent/request/result/receipt retention requires an explicit P8 owner decision and cannot be inferred from Task retention alone.

## Migration sequence and portability

Migrations 0001–0008 are frozen. Preferred order:

1. Owner ratifies P7-D01..D07. No migration is drafted before owner identity, provenance-after-Task-delete, class ceiling, bounds and tombstone lifetime are resolved.
2. P7B authors one additive, reviewable schema migration plan: owner binding and backfill/quarantine; Memory and immutable provenance; owner/class unique keys; supersession; tombstone; FTS and indexes. Parent keys for every composite FK must be declared unique. Validate `foreign_key_check`, query plans and transaction rollback in isolated temporary DBs.
3. P7C implements deterministic extraction, retrieval and forget under the approved schema and fake model/source fixtures. P7D validates privacy, FTS/Japanese and recovery across platform SQLite builds. P7E closure documents retained limits.
4. P8A contract package is independent and can be reviewed at any point. P8 schema/migration follows the agreed P7 schema sequence so both phases share owner identity, Blob reference accounting and transaction/migration evidence. P8 introduces only its own intent/request/result state; no P7 table owns P8 authorization.
5. P8 fake-provider integration follows its approved atomic dispatch/receipt contracts. The first P8 implementation must have no network provider and no credential backend dependency.

Current proposal `P7A → P7B → P7C → P7D → P7E`, with P8A contract preparation independent and P8 Storage migration after the agreed P7 migration sequence, has **no critical dependency requiring reordering**. The caveat is P7-D01: if owner binding/backfill is not decided early, both P7 migration and any owner-scoped P8 ledger are blocked. P8A research can proceed. P15 GoalLatch integration is excluded from P8 implementation; no GoalLatch crate, real Local MCP or device bridge is needed to validate P8 fake providers.

Cross-platform constraints: run bundled SQLite feature probes and migration tests on Linux, macOS Intel, macOS arm64, then the Android SQLite/runtime packaging path before shipping. Avoid platform-specific tokenizer assumptions. Keychain/Keystore production PRIVATE and credential backends are outside this docs-only task and outside synthetic provider tests.

## Cross-phase test cases

These are planned regression names; this document does not create tests.

| Proposed test | Invariant |
| --- | --- |
| `p7_owner_binding_backfill_quarantine` | No `origin_device_id`→OwnerId inference; ambiguous legacy Task cannot produce Memory. |
| `p7_memory_survives_task_retention` | Deleting Task/Step retains permitted independently retained Memory and provenance snapshot. |
| `p7_explicit_forget_after_source_deleted` | Forget still locates item after original Task and Step are gone; removes derived supersession closure. |
| `p7_blob_shared_reference_gc` | Shared `(digest,class)` Blob survives one Memory deletion and is removed only after last reference. |
| `p7_p8_result_extraction_requires_receipt` | Unvalidated, ambiguous, refused or receipt-less P8 result cannot enter EXTRACTION. |
| `p7_provider_content_never_sets_authority` | Prompt-injection strings in fake-provider result cannot set owner/class/policy/action/approval/retention. |
| `p7_class_inheritance_from_provider_result` | Maximum of source/result/receipt/provenance classes is preserved; PRIVATE fails closed. |
| `p7_expired_event_retained_provenance` | Event retention does not erase Memory provenance or falsely imply source verification remains possible. |
| `p7_tombstone_blocks_restored_snapshot` | A backup restored before forget cannot expose searchable Memory until tombstones are reconciled. |
| `p7_fts_delete_and_class_filter_atomic` | Forget/class eligibility updates FTS and authoritative rows atomically; cross-owner or stale index cannot disclose. |
| `p8_no_memory_required_for_provider_success` | P8 fake-provider result/receipt commits when extraction is disabled/unavailable. |
| `p7_extraction_no_provider_dependency` | Memory test harness uses source protocol types/fakes and introduces no runtime dependency on provider crates. |

For all cross-phase tests, assert final SQLite rows, event ordering, foreign-key integrity and reopened-process behavior, not only returned Rust outcomes. Fault injection must cover precommit rollback and postcommit restart. Any test using a provider uses an in-process deterministic fake; zero network and zero external accounts.

## Proposed next authorized slice (not authorized here)

After owner review, the first implementation authorization should be **P7B schema design and isolated SQLite constraint proof**, not implementation of Memory UI or provider effects. Required inputs: ratified P7-D01..D07; exact single-user legacy backfill/quarantine rule; owner ID grammar and host identity port; chosen supersession graph semantics; retention/quota/tombstone values; FTS tokenizer decision or explicitly lexical-only default; approved additive migration sequence. Deliverable should be a migration design review with unique parent-key proof, foreign-key probes, retention/forget transaction diagrams, and benchmark plan. Do not bundle P8 provider invocation or P15 GoalLatch work into that slice.
