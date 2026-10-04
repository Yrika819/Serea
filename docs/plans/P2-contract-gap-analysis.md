# P2 Contract Gap Analysis

- **Project:** Serea
- **Branch:** `p2/design-preparation`
- **Base commit:** `c3737039e3e38dbba554dc0b9075025f87948358`
  (`p1/workspace-protocol-skeleton`, P1 closed)
- **Historical P1 baseline:** `serea-arch/0.2.0`; current frozen P2A contract `serea-arch/1.0.0`
- **Scope:** design preparation only. No production Rust, no SQLite code, no
  `serea-storage` or `serea-task-engine` source, no dependency change. This
  document records what P2 must decide before it can be written, and it decides
  it.

**Current disposition (2026-10-03):** P2A protocol/canonical slices are
implemented, including exact generation decoding without f64 rounding and the
narrow precision dependency patch described in [launch §3](P2-6.1-sol-launch.md#3-dependency-lines-current-p2a-integration-and-p2c-candidate).
Three independent subagent corrected docs reviews under the coordinator were
GREEN; contingent owner design ratification accepts ADR-0018/19/20/23 within
their stated scopes. The coordinator records actual final workspace/MSRV
validation, test counts, review and integration status in the closure record;
runtime remains deferred. Dated baseline gap
observations below are preserved, not rewritten as present-day failures.
See [frozen gate](P2A-review-and-closure.md).

## 1. How to read this document

Every finding is classified into exactly one bucket. The bucket is a statement
about *when* the finding must be dealt with, not about how hard it is.

| Class | Meaning | Consequence for P2 |
| --- | --- | --- |
| `MUST_FIX_BEFORE_P2_IMPLEMENTATION` | P2 cannot be written correctly until this is resolved, because the code would otherwise have to fabricate a value or break a frozen promise. | The protocol change and code change must land **atomically within their owning phase gate**. ADR-0018/19/20/23 now Accepted within their stated P2A scopes; ADR-0018 runtime deferred, ADR-0021/22/24 runtime Proposed. P2A annotations do not promise runtime implementation or full workspace/MSRV closure. |
| `P2_DESIGN_DECISION` | A choice P2 owns. No frozen contract forbids any answer; P2 must simply pick one and write it down. | Decided here. No ADR needed unless it changes a frozen protocol. |
| `SAFE_TO_DEFER` | Real, but P2 can be correct without it. | Recorded with the phase that owns it. |
| `PRE_EXISTING_LIMITATION` | Already known and already recorded; re-stating so it is not mistaken for a P2 regression. | No action. |

Two rules govern the whole document:

1. **A finding is never closed by inventing a value.** If the contract requires
   a `Digest`, the answer is never `sha256:` of the empty string; if a step has
   no `idempotency_key` to derive, the answer is never a reserved pseudo
   capability. Where P1's shapes force one of those, the shape is what changes.
2. **Nothing here edits a frozen protocol.** Every proposed protocol edit is
   written as an exact amendment inside a PROPOSED ADR and left unapplied.
   [Protocol Index §7](../protocols/00-protocol-index.md#7-change-control) makes
   a contract change without an ADR, an architecture-version bump, and a
   changelog entry a review rejection, and this run cannot satisfy all three
   without also changing code, which is forbidden here.

## 2. Summary table

| # | Finding | Class | Owner |
| --- | --- | --- | --- |
| 5.1 | `TaskStep` cannot express a planned-but-unstarted step | `MUST_FIX_BEFORE_P2_IMPLEMENTATION` | [ADR-0018](../decisions/ADR-0018-taskstep-lifecycle-and-field-presence.md) |
| 5.1b | Step `status` is an open code with no closed lifecycle for an engine | `P2_DESIGN_DECISION` | [ADR-0018](../decisions/ADR-0018-taskstep-lifecycle-and-field-presence.md) |
| 5.2 | `idempotency_key` is required on every step kind | `MUST_FIX_BEFORE_P2_IMPLEMENTATION` | [ADR-0018](../decisions/ADR-0018-taskstep-lifecycle-and-field-presence.md) |
| 5.3 | Canonical JSON is specified but not implemented; large integers collapse | `MUST_FIX_BEFORE_P2_IMPLEMENTATION` | [ADR-0019](../decisions/ADR-0019-canonical-json-and-idempotency-preimage.md) |
| 5.4 | The `‖` idempotency preimage has raw-string boundary ambiguity (not a proven legal-action collision) | `MUST_FIX_BEFORE_P2_IMPLEMENTATION` | [ADR-0019](../decisions/ADR-0019-canonical-json-and-idempotency-preimage.md) |
| 5.5 | `B3` reads as forbidding structural schema constraints | `MUST_FIX_BEFORE_P2_IMPLEMENTATION` (clarification only) | [ADR-0020](../decisions/ADR-0020-bounds-b3-scope-clarification.md) |
| 5.6 | One `validate_label` serves three incompatible text categories | `MUST_FIX_BEFORE_P2_IMPLEMENTATION` | [ADR-0023](../decisions/ADR-0023-text-field-validation-categories.md) |
| 5.6b | Schema accepts whitespace-only free text that Rust refuses | `MUST_FIX_BEFORE_P2_IMPLEMENTATION` | [ADR-0023](../decisions/ADR-0023-text-field-validation-categories.md) |
| 5.7 | P2 must mutate state; `E3` requires an event it cannot build | `MUST_FIX_BEFORE_P2_IMPLEMENTATION` | [ADR-0021](../decisions/ADR-0021-p2-p3-event-atomicity-seam.md) |
| 5.8 | `PRIVATE` durable storage has no at-rest owner in P2 | `MUST_FIX_BEFORE_P2_IMPLEMENTATION` | [ADR-0022](../decisions/ADR-0022-durable-private-data-at-rest.md) |
| 5.9 | Blob store shape is undecided between rows and files | `P2_DESIGN_DECISION` | [P2 design §6](../plans/P2-storage-task-engine.md#6-blob-store) |
| 5.10 | `lease_owner` + `lease_expires_at` cannot fence a stale worker | `MUST_FIX_BEFORE_P2_IMPLEMENTATION` | [ADR-0024](../decisions/ADR-0024-lease-fencing-and-commit-under-lease.md) |
| 5.11 | No durable time type; lexicographic `TEXT` comparison is wrong for deadlines | `P2_DESIGN_DECISION` | [P2 design §8](../plans/P2-storage-task-engine.md#8-clock-and-time-representation) |
| 5.12 | Recovery scope: P2 must classify, not execute | `P2_DESIGN_DECISION` | [P2 design §9](../plans/P2-storage-task-engine.md#9-recovery) |
| 6 | One-`AssistantTask`-JSON-blob storage is explicitly refused | `P2_DESIGN_DECISION` | [P2 SQLite schema](../plans/P2-sqlite-schema.md) |
| 12 | Payload-byte / blob-byte / object-count bounds are still P0's open gap | `SAFE_TO_DEFER` (still open) | [P2 design §12](../plans/P2-storage-task-engine.md#12-resource-bounds-still-open) |
| — | `ActionResult` cannot express `C4` without the descriptor | `PRE_EXISTING_LIMITATION` | recorded in P1 closure |
| — | Schema `freeText` whitespace divergence predates the corrective pass | `PRE_EXISTING_LIMITATION` (now promoted to 5.6b) | recorded in P1 closure |

---

## 3. What P1 actually implemented, by inspection

Written from the code, not from the prose. This is the ground the gap analysis
stands on.

| Capability | Where | Present? |
| --- | --- | --- |
| Identifier newtypes for all 11 ULID domains plus `IdempotencyKey`, `Digest`, `ModelId`, `CapabilityId`, `ProviderId`, `ImplementationId` | `crates/serea-protocol/src/ids.rs` | Yes, with a fail-closed `Deserialize` per type |
| `IdMinter<S: UlidSource>` and the validated `TimestampMs` / `UlidValue` pair | `ids.rs` | Yes |
| Frozen wire types and enums | `types.rs` | Yes, including `AssistantTask`, `TaskStep`, `ActionRequest`, `ActionResult`, `SideEffectReceipt`, `ActionError`, `SereaEvent` |
| `TaskState` with `is_terminal()`, 11 variants | `types.rs` | Yes |
| `StepKind`, 8 variants | `types.rs` | Yes |
| Checked-in JSON Schema 2020-12, embedded with `include_str!` | `schemas/*.json`, `schema.rs` | Yes, 5 documents |
| `MAX_INSTANCE_DEPTH = 64` instance-side stack guard | `schema.rs` | Yes |
| Provider ports `CapabilityProvider`, `ModelProvider`, `HostGoalProvider` | `provider.rs` | Declared, zero implementations |
| `CancellationToken`, `ProviderContext`, `ModelCallContext` | `provider.rs` | Data shapes only |
| `TestClock`, `DeterministicUlidSource` | `crates/serea-testkit/src/clock.rs` | Yes, in the dev-only crate |
| A production `Clock` trait | — | **No.** P1 recorded that a `Clock` here "would be a fourth port with no P1 consumer" |
| Canonical JSON | — | **No.** Not one function |
| Any digest computation | — | **No.** `Digest` is a validated newtype; nothing produces one |
| Any persistence | — | **No** |
| Any task state machine | — | **No** |
| Any lease | — | **No** |
| Any event append | — | **No.** `Seq` carries a number |

Consequences for P2 that follow directly from that table:

- P2 is the **first consumer** of a `Clock`, so P2 is the phase that introduces
  it. [Crate Map §3](../architecture/03-crate-map.md#3-crate-inventory) already
  freezes `Clock` as a `serea-protocol` item, so this is filling a declared slot,
  not a new contract.
- P2 is the **first author** of any `Digest`, so SCJ-1 and IDK-1 have no
  backward-compatibility obligation to any previously minted value. That is the
  cheapest possible moment to fix them, and the worst possible moment to get them
  wrong.
- P2 has no Capability Registry, so anything requiring a descriptor — the `C4`
  receipt invariant, `side_effect_class`, `replay_safety`, `T6` ceiling checking
  — cannot be closed by P2. Each such item is listed as deferred below rather
  than silently claimed.

---

## 4. Findings carried over unchanged

These are already recorded and are restated so no reader mistakes them for P2
discoveries.

| Item | Where recorded | P2's relationship |
| --- | --- | --- |
| `ActionResult` cannot express `C4` (receipt non-null exactly when `SUCCEEDED` and `side_effect_class != NONE`) because the result carries no `side_effect_class` | P1 closure, "Representation-only items" | Still true. A step carries no `receipt_id` column; the receipt is reached by `side_effect_receipts.step_id`, `UNIQUE`, and a trigger enforces the `receipt ⇒ SUCCEEDED` direction. The `side_effect_class != NONE` half needs the registry (P5). |
| `ProviderId` validates the grammar, not the frozen namespace set | P1 closure | Unchanged. Namespace membership is the registry's job (P5). |
| No `Secret<T>` | P1 closure | Unchanged. `serea-storage` must never hold secret bytes, which is consistent with `CREDENTIAL` being refused (5.8). |
| No capability `input_schema` documents exist, so `DC8` is enforced only at the envelope level | P1 closure | Unchanged. P2's `plan` path stores arguments as blobs without schema validation; schema validation of arguments is P5. |
| Payload-byte / attachment-size / object-count bounds unspecified | P1 closure, "Open items" | **Still open.** See §12. |

---

## 5.1 `TaskStep` cannot represent a step that has not started

**Observed.** `TaskStep` in `types.rs` declares `idempotency_key`, `input_digest`,
`result_digest`, `started_at` and `completed_at` as non-`Option`, non-`default`.
`assistant-task.schema.json` lists the same five in `$defs/step.required`.

**Contradiction with a frozen clause.** Task Protocol §4.3 requires a plan to be
persisted *before* execution, and §3.1 requires `attempt` to distinguish a
crash-recovered attempt from a deliberate retry. A step that exists in a
persisted plan but has not run has no `started_at`, no `completed_at`, no
`result`, and — for the five non-capability-shaped kinds — no `idempotency_key`. The
current shape has no representation for it except fabricating timestamps and
digests, which would corrupt `result_digest`'s stated purpose ("Detects result
corruption or partial writes on recovery") and make `started_at` a lie.

**Resolution: one flat wire object, four fields become optional, and presence
becomes a checked invariant.** Task Protocol §3 gives an exhaustive field list
but explicitly does not require a closed schema, and
[Protocol Index §4.2 rule 3](../protocols/00-protocol-index.md#42-compatibility-rules)
makes a new *optional* field an architecture-minor change. Making an existing
required field optional is a **relaxation**, which is a different thing and is
discussed honestly in §5.1c.

**Why not split `TaskStep` into a plan object plus an execution object.** The
alternative — a plan-level `TaskStep` and a separate `StepExecution` — was
evaluated and rejected for three concrete reasons:

1. `SideEffectReceipt` requires `capability_id` and `idempotency_key`, and
   `ActionError` is nested inside the step. Splitting puts the receipt's
   required fields in one object and the receipt in another, so the frozen
   "non-null exactly when an effect occurred" rule becomes a cross-object
   invariant instead of a field-presence rule.
2. `AssistantTask.steps` is `Vec<TaskStep>`. Changing the element type changes
   the wire shape of a frozen surface and forces a second schema document,
   which is a new named concept in the Protocol Index §1 registry.
3. Task Protocol §3 says a step carries "enough information to be re-executed or
   verified after a hard restart". One object keeps that property legible; two
   objects invite a caller to persist the second without the first.

**Resolution (a): the seven-status step lifecycle.** The step status set is
currently an open `StepStatus` code validated only as `^[A-Z][A-Z0-9_]*$`. An
engine needs a closed set to make transitions exhaustive. P2 therefore defines a
**closed engine-internal** lifecycle and keeps the wire code open:

| Status | Meaning | Terminal for the step? |
| --- | --- | --- |
| `PLANNED` | Persisted from the plan. Never leased, never attempted. | No |
| `LEASED` | A lease is held. The attempt has not begun. | No |
| `EXECUTING` | An attempt is in flight; `started_at` is set. | No |
| `WAITING` | Suspended on a human, a grant, or a schedule. Reachable **only** for `WAIT_APPROVAL`, `WAIT_USER`, `WAIT_SCHEDULE`. | No |
| `SUCCEEDED` | Completed successfully. The frozen name. | Yes |
| `FAILED` | Completed unsuccessfully. | Yes |
| `RECONCILED_ABSENT` | Closed as confirmed-absent by read-back. The frozen name from Bounds Protocol §4.4. | Yes |

A `StepStatus` the engine does not recognise is **not** persisted and **not**
executed. The task moves `BLOCKED` with `blocked_reason: UNRECOGNISED_STATE`,
reusing a code Event Protocol §4 already names, per
[Protocol Index §4.2 rule 5](../protocols/00-protocol-index.md#42-compatibility-rules).
That is the rule for unrecognised task states; applying it to an unrecognised
step status is the same rule at the same boundary and invents nothing.

**No `SUPERSEDED` status is introduced.** A plan revision that drops an
unstarted step deletes the step row instead, because the prior plan's full
content is retained as a content-addressed blob referenced by the
`plan_revisions` row. A step that was ever leased or executed is never deleted;
it is closed under its real status. This keeps the status set at seven without
losing auditability.

**Resolution (b): the presence matrix.** `input_digest` is required in every
status because a step's input is known at plan time: for `CAPABILITY` it is the
arguments blob, and for every other kind it is the host-written instruction
document. `result_digest` is required exactly when a result document was
persisted, and every `SUCCEEDED` step persists a real, host-written result
document — for `WAIT_USER` that is `{"answered": true}`, for `VERIFY` the
verification outcome, for `MODEL_TURN` the structured output. That is how
`SUCCEEDED ⇒ result_digest non-null` holds **without any fabricated digest**.
`lease_owner` and `lease_expires_at` are present exactly while the status is
`LEASED` or `EXECUTING`, because Task Protocol §5 rule 3 and the Execution Pipeline
require the lease to be released before a wait. A capability step is `PLANNED`
while its sibling `WAIT_APPROVAL` step is `WAITING`.

Full matrix, with the exact SQL `CHECK` expressions, is in
[ADR-0018](../decisions/ADR-0018-taskstep-lifecycle-and-field-presence.md).

**P2A wire disposition.** Convert only idempotency_key/result_digest/started_at/
completed_at to Option; input_digest remains required, provider/capability/version
already Option. Seven unconditional fields, missing/null accepts None, serializer
omits None. SQL generation 0 maps to None and positive u32 maps to Some, with
overflow refused. Known-status presence validation applies only to known statuses;
kind invariants apply always, so unknown well-formed status still parses. Runtime
unknown execution blocking remains P2F. Major/version/migration direction is
ratified by owner instruction, not an open minor/major choice. ADR-0018 is Accepted
as wire/lifecycle architecture with implemented checked wire validation, not runtime
enforcement. TaskStepDraft → private validated TaskStep/StepPresence has no public
mutation bypass; reserved extension keys are refused. Non-capability receipts
are absent on ALL statuses including unknown; corrected Rust/schema parity is P2A.

## 5.1b Step `status` needs a closed lifecycle, and a plan revision needs an ordering rule

**Observed.** `StepStatus` is an open code. `PlanRevision` is a Crate Map §3
public type with no definition anywhere. Task Protocol §4.3 rule 5 says a
revision "is recorded as evidence" and "Revising a plan never re-executes a
completed step automatically", but says nothing about ordering, and Task
Protocol §3.2 makes `sequence` a total order with a prerequisite rule
("Steps with `sequence < n` that are required for step `n` must be `SUCCEEDED`").

**Resolution.** P2 V1 restricts a plan revision to **append-only at higher
sequences, plus deletion of `PLANNED` steps**. A revision may not renumber,
reorder, or insert between existing sequences, because every one of those would
change the meaning of the §3.2 prerequisite rule for a step that has already
run. The named relaxation, when the need is demonstrated, is an `insert_at`
that renumbers only a wholly-`PLANNED` suffix.

Classification: `P2_DESIGN_DECISION`, recorded in ADR-0018 alongside the status
set because they are the same "how does the engine model a step" question. The
§2 summary table row for 5.1b said `MUST_FIX_BEFORE_P2_IMPLEMENTATION`; it is not.
The wire change is the MUST_FIX, and it is carried by 5.1.

## 5.2 Idempotency and capability fields on non-capability steps

**Observed.** Capability Protocol §8.2 derives the key from `task_id`, `step_id`,
`capability_id`, `capability_version` and `canonical_json(arguments)`.
`StepKind` also contains `MODEL_TURN`, `WAIT_APPROVAL`, `WAIT_USER`,
`WAIT_SCHEDULE`, `VERIFY`, `NOTIFY` and `DELEGATE`. Three of the five
formula inputs do not exist for most of those kinds, yet idempotency_key is unconditionally required. Provider/capability/version
already are Option; their kind-dependent invariant still needs validation.

**Resolution: a step-kind × required-field matrix, and `idempotency_key` becomes
capability-scoped.**

| `StepKind` | `provider_id` | `capability_id` | `capability_version` | `idempotency_key` | `input_digest` | Rationale |
| --- | --- | --- | --- | --- | --- | --- |
| `CAPABILITY` | required | required | required | **required** | required | Invokes a provider; §8.2 applies verbatim |
| `DELEGATE` | required | required | required | **required** | required | Reaches `host.goal.*` through the same `ActionRequest` path (GoalLatch Adapter §4, §6), so the same external-effect and same-key rules apply |
| `VERIFY` | required | required | required | **required** | required | A verify step reads back through a read-only capability, so it is an invocation with an effect class of `NONE`, not a host-only inspection (Bounds Protocol §6.3) |
| `MODEL_TURN` | absent | absent | absent | **absent** | required | A model call is not idempotent and re-issuing one is a billing event. Deduping a model call is the usage ledger's decision in P4, not a step-key decision in P2 |
| `WAIT_APPROVAL` | absent | absent | absent | **absent** | required | No effect |
| `WAIT_USER` | absent | absent | absent | **absent** | required | No effect |
| `WAIT_SCHEDULE` | absent | absent | absent | **absent** | required | No effect |
| `NOTIFY` | absent | absent | absent | **absent** | required | In P2 a notify step records that a notification is owed; `serea-core` renders it in P12. P5/P6 may route `NOTIFY` through `device.*` capabilities, at which point it becomes capability-shaped and this row changes by ADR |

**Why the key is not derived for a non-capability step at all.** The three
rejected alternatives were each worse than absence:

1. A reserved pseudo-capability such as `serea.host.model` would be a *new
   capability namespace*, and a `CapabilityId` namespace is owned by a
   registered `ProviderId` (Protocol Index §3). Minting one in P2 would
   pre-empt P5's registry with a namespace no provider owns.
2. Deriving the key from `task_id ‖ step_id` alone would reuse the one wire field
   whose *meaning* is "this is the same action", for actions that are not
   capability actions. A key that means "the same" for two unrelated purposes is
   exactly what Capability Protocol §8.2 forbids — it exists so a post-crash
   re-issue is recognised as the same *action*.
3. A second, separately-prefixed key would need a second identifier prefix, and
   the identifier registry is frozen (Protocol Index §2).

What replaces attempt-safety for those kinds is `(step_id, attempt)` plus
`max_attempts_per_step` read from durable state, which is what Task Protocol
§3.1 already says `attempt` is for.

**`TASK_POLICY_CEILING_EXCEEDED` is not P2's to enforce either.** A step whose
capability risk class exceeds the task's `policy_class` is refused — but the risk
class lives in a descriptor, and P2 has no registry. P2 enforces the ceiling
*mechanism* (`policy_class` is immutable, and the ceiling column is
CHECK-constrained) and records the obligation; P5 supplies the comparison.

Classification: `MUST_FIX_BEFORE_P2_IMPLEMENTATION`. Disposition: Proposed ADR-0018 architectural/wire decision; runtime deferred.

## 5.3 Canonical JSON is specified but not implemented

**Observed.** Protocol Index §5 states the rule in one sentence: keys sorted
lexicographically by UTF-8 code point, no insignificant whitespace, no trailing
newline, UTF-8, numbers in shortest round-trip form. No function implements it.
P1 closure also records that `serde_json` is used without `arbitrary_precision`,
so "two distinct integers above `u64::MAX` collapse to one `Value`".

**Why "sort the keys" is not a specification.** Four of the five clauses are
underdetermined and each one produces a different digest:

- *Shortest round-trip form* needs a specified algorithm and domain. A portable
  ECMAScript spelling exists; the earlier claim that no such spelling exists was
  false. SCJ-1 deliberately limits its domain to integers. ModelRequest.temperature
  is f64 and remains valid on model/1; future digest paths reject fractional model
  documents until a new fractional canonicalization decision, never truncate them.
- *Escape policy* is unspecified. `"\u0041"` and `"A"` are the same string but
  different bytes; so are a raw U+2028 and its escaped form.
- *Number domain* is unspecified. Does `1.0` canonicalise to `1`, to `1.0`, or is
  it rejected? Does `-0` survive? What about an exponent?
- *Duplicate object keys* are unspecified. `serde_json` keeps the **last**
  occurrence silently, so `{"arguments": …, "arguments": …}` would digest as the
  last value — and a provider written in another language may read the first.
  That is a parser differential on a value that decides whether an effect is
  suppressed.

**Resolution: SCJ-1, with the number domain restricted and duplicates refused.**

1. **Entry point is text, never a `Value`.** `canonicalize(&str) -> Vec<u8>` is
   the only entry point. P2 never digests a pre-built `serde_json::Value`, because
   a `Value` has already lost duplicate keys and cannot be checked for them.
   `put_blob` therefore takes `&[u8]` and canonicalises; there is deliberately no
   `put_blob_value`. Original raw text must reach this parser before Value discards duplicates.
   The string API cannot prove provenance or reconstruct already-lost duplicates.
2. **Duplicate object keys are rejected** with a typed error, detected by a
   `serde_json` visitor that tracks seen keys. This is the single most important
   rule in SCJ-1.
3. **Member ordering** is by the UTF-8 byte sequence of the key. For UTF-8 this
   is identical to ordering by Unicode scalar value, so the wording "by UTF-8
   code point" is satisfied exactly, and no locale or collation is involved.
4. **No insignificant whitespace** anywhere; `:` and `,` carry no space.
5. **Escaping is a closed list.** `\"`, `\\`, `\b`, `\f`, `\n`, `\r`, `\t`; every
   other C0 control and U+007F as lowercase `\u00xx`; everything else as raw
   UTF-8, including non-ASCII, U+2028, U+2029 and `/`. Ill-formed UTF-8 is
   refused at the parse boundary, so a lone surrogate cannot occur.
6. **Numbers are integers only, in `-2^63 ..= 2^64-1`, written as a decimal with
   no exponent, no leading `+`, no leading zero and no `-0`.** Any `f64` — and
   therefore any integer above `u64::MAX`, which `serde_json` parses as `f64` when
   `arbitrary_precision` is off — is a hard error. This is what turns P1's
   silent collapse into a **detectable** condition: `u64::MAX` canonicalises,
   `u64::MAX + 1` is refused. Neither two-distinct-large-integers nor a float can
   reach a digest. Stated cost: a capability whose `input_schema` legitimately
   admits a fractional number cannot have a stable digest under SCJ-1, and P5
   must decide per capability whether to admit `arbitrary_precision` or to declare
   the field non-canonicalisable. That is a P5 obligation, recorded, not inherited
   silently.
7. **Arrays keep order.** Order is semantic in JSON arrays; reordering them would
   change the value.
8. **The root may be any value**, including an integer or a string. There is no
   requirement that a digest preimage be an object.
9. **Output is UTF-8, no BOM, no trailing newline.**
10. **Nesting deeper than `MAX_INSTANCE_DEPTH = 64` is refused** before
    canonicalisation, reusing the constant and the rationale already in
    `schema.rs` (a stack overflow aborts, which is not a graceful refusal).

**Content digests versus structured digests.** This is the distinction that keeps
the change-control surface small, and it is the most important structural point in
this document:

| Digest | Frozen definition | Proposed encoding |
| --- | --- | --- |
| `arguments_digest` | "sha256 over the canonical JSON of `arguments`" (Capability Protocol §4.3) | `sha256(SCJ-1(arguments))` — **unchanged**, no domain tag |
| `input_digest`, `result_digest` | "sha256 over the canonical JSON of …" (Task Protocol §3.1) | `sha256(SCJ-1(x))` — **unchanged** |
| Evidence `payload_digest` | "sha256 over the canonical JSON of the payload" (Capability Protocol §7) | `sha256(SCJ-1(x))` — **unchanged** |
| `idempotency_key` | A hash over a **tuple of named fields** (Capability Protocol §8.2) | **Domain-separated and framed** — see 5.4 |

A content digest hashes one document, so its preimage is the document and there is
nothing to frame. The idempotency key is a *derived* value over several named
fields, so it is the only one that needs a domain tag and an injective encoding.
The frozen text for every content digest therefore stays exactly as written.

**Fixed vectors.** Ten input → canonical-bytes → digest vectors are computed and
pinned in [ADR-0019](../decisions/ADR-0019-canonical-json-and-idempotency-preimage.md),
including whitespace normalisation, member reordering, the escape table, array
order preservation, and `u64::MAX`. They are real values, computed from the
specification, not illustrative placeholders.

Classification at design time: `MUST_FIX_BEFORE_P2_IMPLEMENTATION`. Current
disposition: Accepted ADR-0019 full primitive decision, implemented in P2A with
sha2 0.11 without defaults. Current final workspace/MSRV validation, review and
integration status is recorded by the coordinator in the
[closure record](P2A-review-and-closure.md), not inferred from earlier counts.
No canonical work moves to P2B.

## 5.4 Idempotency framing ambiguity and the input-domain correction

Historical A/B hashes using `p.r.list` are mathematical raw-string framing vectors
only: the ID is invalid (provider/resource require 2–32 characters), and scalar
arguments are not ActionRequest object roots. SCJ-1 generically permits scalars.
`pp.rr.list` with `0.0.0`/`-12` versus `0.0.0-1`/`2` gives a generic scalar naive
collision, not a demonstrated collision between legal ActionRequests. Independently
pin that pair and legal object inputs per ADR-0019. Typed public derivation accepts any SCJ-1 root with valid typed identifiers;
ActionRequest separately requires object-root arguments. Historical raw vectors
belong only in low-level private framing tests. Public derivation rejects
`p.r.list`. Historical corpus counts remain evidence of that raw-string experiment,
not coverage of frozen legal action grammars.

No grant transfers from A to B: approval independently checks digest, capability
and pinned version. Distinct StepIds also participate, so the fixed-StepId scalar
pair does not prove a two-step uniqueness collision. IDK-1 supplies unambiguous
encoding rather than relying on accidental grammar separation.

**Resolution: IDK-1, a domain-separated, length-prefixed framing.**

```text
preimage :=
    b"serea.idempotency.v1\x00"          21 bytes, domain separation
  || u8(5)                              field count, pinned by version
  || lp("task_id")              || lp(task_id_bytes)
  || lp("step_id")             || lp(step_id_bytes)
  || lp("capability_id")       || lp(capability_id_bytes)
  || lp("capability_version")  || lp(capability_version_bytes)
  || lp("arguments_canonical") || lp(scj1(arguments)_bytes)

lp(x) := u64 big-endian byte length of x, followed by x
idempotency_key := "idk_" + lowercase_hex(sha256(preimage))
```

This is injective **by construction**, not by an accident of three
independently-maintained grammars, which is the entire point. The field count
byte makes truncation detectable and makes a future preimage with a different
field set impossible to confuse with this one.

The discriminating vectors are pinned in ADR-0019 §3:

| Tuple | `idempotency_key` under IDK-1 |
| --- | --- |
| A | `idk_b9e8299d5b628af7d40e253035ce7af0f653a4523a20448ea061bea316f0adaf` |
| B | `idk_141fa78316b05b374a0a11a2fe7093880daa19598dddc9b22675ffadedf232ef` |

**Fixed-separator alternatives were evaluated and rejected** on the specific
ground that ambiguity must be *formally impossible*, and a separator is not:

- `task_id ‖ "|" ‖ step_id ‖ …` fails the moment any field may contain `|`.
  `ProviderReference` is opaque and Serea never parses it (GoalLatch Adapter §2),
  and capability arguments are arbitrary validated JSON, so a `|` can occur in a
  preimage.
- Newline framing fails for the same reason, and additionally collides with a
  literal newline in a JSON string argument.
- Percent- or C0-escaping every field is equivalent to length prefixing with more
  moving parts and no correctness gain.

**Change control.** Full canonical/digest/IDK primitives and sha2 0.11 without
defaults belong to one P2A gate with action/2; Clock remains P2B. The owner ratified
current architecture/1, task/2, action/2, event/1, envelope1 and MSRV 1.85.
ADR-0019 is Accepted in full as an implemented primitive decision. The coordinator
records current validation, test counts, bounded regression review and atomic
integration status in the [closure record](P2A-review-and-closure.md).

## 5.5 Bounds `B3` versus structural schema constraints

**Observed.** Bounds Protocol §2 declares its table "the authoritative list"
and `B3` says "This table is the complete bound set; a bound enforced anywhere
else is a bug." Capability Protocol §3.1 separately requires that every array
have `maxItems` and every string have `maxLength`. The checked-in schemas carry
`maxLength`, `maxItems` and `maxProperties` keywords, and `schema.rs` enforces a
nesting depth of 64. Under a literal reading of `B3`, all of those are bugs.

**Resolution: formalise the distinction rather than pick a winner.**

| | Operational / resource bound | Structural / schema validation constraint |
| --- | --- | --- |
| Question answered | How much *work* may happen? | Is this *value* well formed? |
| Has a default | Yes, in the §2 table | No |
| Has a scope | Yes, per-task, per-step, global | None; it is a predicate |
| Has an exhaustion behaviour | Yes: task `FAILED`, or `BLOCKED` where resumable, plus a `BOUND_EXCEEDED` event | None: the value is refused |
| Runs before or after scheduling | After a decision to do work | Before any work is scheduled |
| Governs | Call counts, token budgets, time, retries, concurrency, retention | Identifier grammar, string and array shape, object closure, integer ranges, nesting depth |
| Under `B3` | Must appear in §2 | Must **not** appear in §2, and its absence there is not a bug |

Two rules make the distinction operational rather than rhetorical:

1. A structural constraint **may not** be used to enforce an operational bound.
   No truncation, no clamping to fit, no silent coercion. A value that is too
   long is refused, never shortened. This is the same shape as
   [Bounds Protocol §8](../protocols/10-bounds-protocol.md#8-bounds-are-not-security-policy):
   bounds stop work, validation refuses values, and neither substitutes for the
   other.
2. Anything that **counts** work is a §2 bound and must be listed there.

`MAX_INSTANCE_DEPTH = 64` is the live case. It is a structural predicate: it
refuses a *value*, it has no default and no exhaustion behaviour, and a bound's
`BOUND_EXCEEDED` event has no meaning for a schema rejection. ADR-0020 names it
explicitly so the literal-`B3` reading is retired rather than left to a future
reviewer to rediscover.

**Affected contracts, identified as §7 item 3 requires:** Bounds Protocol §2 and
`B3`; Capability Protocol §3.1 (its `maxItems`/`maxLength` bullet list stays, and
gains an explicit classification); the `serea-protocol` `validate_label`
documentation, which already cites `B3` for *not* adding a length ceiling and is
therefore already correct; the checked-in schemas, which are unchanged by this
ADR; and `serea-protocol/src/schema.rs`, whose depth constant and executable
behavior are unchanged by this ADR. In the complete current P2A inventory,
`schema.rs` has version documentation only; embedding/validation unchanged.

**Why an ADR and not a plan note.** `B3` is in the Protocol Index §4.3 frozen
list's neighbourhood and is quoted by production code comments. Retiring a
misreading of a frozen invariant by editing the protocol without recording it is
exactly the "silent change" the change-control section forbids. ADR-0020 carries
the exact proposed insertion and leaves the document unedited.

Classification: `MUST_FIX_BEFORE_P2_IMPLEMENTATION` for the *clarification*, which
is a documentation defect in a frozen invariant's meaning and will otherwise be
re-litigated during P2 code review. Disposition: PROPOSED ADR-0020.

## 5.6 Free text, whitespace and control characters

**Observed.** `validate_label` in `types.rs` rejects empty, whitespace-only, and
any string containing a `char::is_control()` character. One validator serves nine
fields with genuinely different semantics:

| Field | What it is | Rendered as |
| --- | --- | --- |
| `ActorId` | An event actor identity | A machine column |
| `LeaseOwner` | The worker holding a lease | Compared for equality in SQL, audited |
| `ProviderReference` | An external system's own handle, never parsed | Compared for equality, echoed in reconciliation |
| `ErrorMessage` | A provider diagnostic | Terminal or log line |
| `TaskTitle` | A task heading | One timeline row |
| `DescriptorTitle` | A capability heading | One picker row |
| `DescriptorDescription` | Capability documentation | Prose |
| `PlainSummary` | The sentence a user consents to | **An approval prompt** |
| `EffectSummary` | What changed | One timeline row |

Two of these demands are wrong for at least some of those fields.
`PlainSummary` is rendered into an approval prompt: a newline there is a
consent-spoofing vector, so refusing newlines is correct and must not be
relaxed. `ErrorMessage` is a provider diagnostic that frequently contains a
stack-shaped message with newlines: refusing them forces a provider author to
mangle a real diagnostic, and it makes no security difference, because nothing
renders it to a human who could be misled by a second line.

**Second observed defect.** The schema pattern for free text is
`^[^\u0000-\u001f\u007f]+$` with `minLength: 1`. It does **not** exclude
whitespace-only input, so `"   "` passes the schema and fails the Rust
validator. P1 closure already records this as a pre-existing divergence and
deliberately left it alone because tightening the schema is a contract change.
P2 cannot leave it alone: P2 reads a row, converts it through the Rust type, and
writes it back, so a row that the schema admits and Rust refuses is a
round-trip failure at the storage boundary. It is promoted here.

**Resolution: three categories, one validator each.**

The complete current rules and generated fragments are normative in
[ADR-0023](../decisions/ADR-0023-text-field-validation-categories.md): O/L refuse
C0, DEL, C1, U+2028/U+2029; P allows interior LF/TAB/U+2028/U+2029 but rejects
CR, other C0, DEL and C1. All reject leading/trailing pinned Unicode White_Space.
The identical Rust/schema list includes U+0085; do not use JavaScript `\s`.
Exact identifier subtraction uses idk_, sha256:, ULID [0-7] then25 and exact
provider goallatch only. No event/1 O widening; no new free-text length bounds.

Notes on the choices, because each is a judgement:

- The **impersonation rule** in category O is the only rule here that is not
  about whitespace. It earns its place because these three values are stored in
  equality-compared columns and rendered in audit records; two different
  identifier grammars sharing one column is a genuine confusion vector, and
  refusing it costs nothing (a real provider reference is not a `stp_` ULID).
- **No length ceiling in Rust.** P1 removed `MAX_VALUE_LENGTH` by owner decision
  because it was an unratified competing bound under `B3`. That removal stands
  and this design does not reintroduce the constant. The per-field
  `maxLength` belongs in the schema, which is where Capability Protocol §3.1
  already grounds it for schema strings and where a *structural* constraint
  belongs under ADR-0020. The free-text fields currently carry no `maxLength`
  and this design adds none.
- **Prose fields are `PRIVATE` for log and event egress.** Data Classification §5
  permits `PRIVATE` in an application log only as "Digests, shapes, and counts
  only — never payload bytes". `ErrorMessage` is already named as the `AB-13`
  carrier in P1 closure. So a prose field may be *stored* (subject to §5.8) but
  may not be *logged* or placed in an event payload without redaction. This is a
  caller obligation recorded in the plan, not a new type.

**Schema inventory.** All affected inline and referenced fields are recategorized,
including event actor.id and each nested receipt provider_reference in both task
and action-result schemas. The validators belong to types.rs, not schema.rs.
Patterns and accepting/refusing corpus agree within the historical scoped GREEN
evidence. ADR-0023 is Accepted as the full implemented validation decision;
current whole-workspace/MSRV and review/integration evidence is coordinator-owned
in the [closure record](P2A-review-and-closure.md).

## 5.7 P2 versus P3 event atomicity

**The question.** Event Protocol `E3` says an event and the state change it
describes commit in one transaction, and `E4` says `seq` is gapless and assigned
at commit. P2 must implement a durable task lifecycle, which means mutating
state. `serea-event-bus` is P3 and Crate Map §3.1 gives it sole ownership of
`SereaEvent` construction, gapless `seq` assignment, the append-only log, and
retention classes. Crate Map §4.1 states the event-bus crate exists precisely so
that "`seq` could [not] be assigned outside the commit transaction".

Three options, and why two of them are unacceptable:

| Option | Verdict |
| --- | --- |
| Create `serea-event-bus` in P2 | **Rejected.** It inverts the phase plan, pre-empts P3's design of the fan-out queue and retention classes, and puts an L1 crate into a slice whose declared scope is L1 `serea-storage` plus L3 `serea-task-engine`. The prompt's own instruction not to "prematurely implement P3's event bus" is also the architecture's instruction |
| Mutate state with no durable trace at all | **Rejected.** `E3` is violated, and worse, a task's history becomes unreconstructable. It also breaks Task Protocol §6 recovery, which needs to know what was already decided |
| A transaction primitive in P2 that P3 fills without rewriting P2 | **Selected proposal; deferred runtime gate.** Described below |

**The seam.** `serea-storage` owns the *transaction*. A transaction participant is
a shared-receiver participant whose `record` runs **inside** the caller's
`BEGIN IMMEDIATE … COMMIT`, so anything it writes commits or rolls back with the
state change:

```rust
/// Immutable description of the transition being committed. Built once, by the
/// `Tx` method performing the state write, and passed to every participant, so
/// no participant can record a different transition from any other.
pub struct DurableTransition<'a> { /* occurred_at_ms, actor, causation_id,
                                      data_class, payload_digest, payload_json */ }

pub trait TransactionParticipant {
    fn record(&self, tx: &Transaction, t: &DurableTransition<'_>)
                   -> Result<(), StoreError>;
}
```

P2 registers exactly one participant, `TaskJournal`, owned by `serea-task-engine`,
which writes append-only rows to `task_journal`. P3 adds a second,
`serea-event-bus`, which writes `serea_events` and allocates `seq` from
`store_meta.next_seq` inside the *same* transaction, from the *same*
`DurableTransition` the journal received. Because both run inside one
`BEGIN IMMEDIATE`, `E3` holds for every transition from that point on and **not one
P2 state-transition function changes**.

**Corrected by the P2 autonomous audit, in two places.** The earlier text said
"`E3` becomes true at the moment P3 exists" and made P3's last obligation
back-filling `task_journal.event_seq`. Both are wrong:

- `E3` is **not retroactively satisfiable**. A reconstructed event was written in a
  different transaction, later, from a different process; for P2-era transitions
  the transaction `E3` describes does not exist. `E3` holds **forward only**.
- The back-fill is the `pending_event` outbox this same section had already
  **rejected** on the grounds that "the gap is permanent rather than
  transitional". Adopting it under another name was a self-contradiction.

So `event_seq` is dropped rather than back-filled, and P3's upgrade path *reads*
`task_journal` for pre-P3 history without synthesising events. The historical
material is already durable, which is why `task_journal` carries `actor_kind`,
`actor_id`, `actor_version`, `causation_id`, `data_class_rank`, `payload_digest`,
`payload_ref_digest` and `payload_json` — so the history is **complete**, not so
that events can be fabricated from it.

A separate earlier error, also corrected: an earlier draft claimed the envelope
columns were "precisely the fields an `EventKind`-specific payload needs".
`CAPABILITY_COMPLETED` needs `duration_ms` and `output_digest`,
`BOUND_EXCEEDED` needs `bound_name`, the limit and the observed value, and
`MODEL_CALLED` needs `model_id`, `purpose` and a token estimate. None was in the
column list, which is why `payload_json` was added.

**What P2 can honestly claim at closure.** This is the part that must not be
fudged:

| Claim | P2 status |
| --- | --- |
| `T1` task state is durable and authoritative | **Claimed.** `tasks.state` is a CHECK-constrained column and the only authority |
| `T4` a step's success and receipt persist before the task advances | **Claimed.** Both are in the same `Tx` |
| `T5` recovery is idempotent | **Claimed.** Conditional writes with full preconditions |
| `TB-7` cross-record atomicity for task, step, receipt and journal rows | **Claimed** |
| `E3` an event and its state change commit together | **NOT claimed.** Deferred to P3 by construction |
| `E4` `seq` gapless and assigned at commit | **NOT claimed.** No `seq` exists in P2 |

To make the deferral *visible* rather than silent, `RecoveryReport` carries
`pending_event_transitions: u64`, counting all P2 journal rows (no event_seq
column or backfill), and a P2 test asserts that number is greater than zero after a task
creation. An operator can see the `E3` debt from inside the product.

**One more consequence, stated because it is easy to miss.** P2 must not create
the `serea_events` table, and it must not create a `store_meta.next_seq` counter.
Both belong to P3. That is why the P2 schema in
[P2-sqlite-schema.md](../plans/P2-sqlite-schema.md) has **no** event table and
**no** store-metadata table, and why P3's first migration is `0002`.

Classification: `MUST_FIX_BEFORE_P2_IMPLEMENTATION` — P2 cannot be written
without knowing which surface it owns, and getting this wrong in either direction
is expensive. Disposition: PROPOSED
[ADR-0021](../decisions/ADR-0021-p2-p3-event-atomicity-seam.md).

## 5.8 Durable `PRIVATE` data and the at-rest boundary

**Observed.** Data Classification §2 requires `PRIVATE` to be stored "encrypted
at rest", §5's egress matrix repeats it, and [Trust Boundaries
§4.1](../architecture/02-trust-boundaries.md#41-permitted-crossings-by-destination)
repeats it for `TB-7`. `serea-storage`'s Crate Map row says it owns "retention
and redaction-at-rest". There is **no** at-rest protection owner anywhere: the
only crate permitted secret custody is `serea-credential-store`, which Crate Map
§4.2 makes a *separate* crate precisely so it is not entangled with the database,
and whose dependency edge points one way — `serea-credential-store` depends on
`serea-protocol`, and `serea-storage` does **not** depend on it.

**Resolution: enforcement is structural, and `PRIVATE` fails closed with no
backend.**

| Class | P2 behaviour |
| --- | --- |
| `PUBLIC` | Ordinary content store |
| `PERSONAL` | Ordinary content store |
| `PRIVATE` | **Refused** with `AtRestProtectionUnavailable` unless an injected `AtRestProtection` is configured whose `classes_protected()` contains `PRIVATE`. With one configured, the blob is stored under `protection = 'AT_REST'` |
| `SECRET` | **Always refused** by the ordinary store: `ClassRefused`. Routed only to a future sealed store |
| `CREDENTIAL` | **Always refused**: `ClassRefused`. Its only permitted destination is the OS credential store, which P2 is not |

"Fail closed" here means: with no backend configured, a `PRIVATE` write returns an
error and **nothing reaches disk**. There is no plaintext fallback, no
"encrypt later", no warning-and-continue.

**Enforcement is in the schema as well as in Rust**, so that a hand-written
`INSERT` or a future code path cannot bypass it:

```sql
CHECK (data_class IN ('PUBLIC','PERSONAL','PRIVATE'))
CHECK ((data_class = 'PRIVATE') = (protection = 'AT_REST'))
```

A `SECRET` or `CREDENTIAL` row cannot be constructed at all, and a `PRIVATE` row
without a protection tag cannot either.

**P2 ships no real backend, and that is a deliberate layering decision.** A real
backend needs a cipher and a key source. The key source that exists is the macOS
Keychain, which is `serea-credential-store`'s, and Crate Map §4.2's entire
argument is that credential custody must *not* be reachable from the durable-state
crate. Folding a real backend into P2 would either invert that edge or invent a
second one. So:

- P2 provides the `AtRestProtection` **trait** and the fail-closed plumbing.
- P2 provides a **test double in `serea-testkit`**, which is dev-only and
  mechanically unreachable from any runtime crate by `tests/workspace_smoke.py`.
  It is labelled in its own documentation as *not encryption*; it exists to prove
  the wiring and the refusal path, nothing else.
- A real backend is a separate, later decision with its own crate-boundary
  question, recorded as such in ADR-0022.

**What P2 can and cannot test.** Stated plainly so tomorrow's closure record is
honest.

| P2 **can** test | P2 **cannot** test |
| --- | --- |
| `PRIVATE` write with no backend returns `AtRestProtectionUnavailable` and writes no row | That a real backend is cryptographically sound |
| The `CHECK` rejects a hand-inserted `SECRET` or `CREDENTIAL` row | That a real key is protected at rest |
| The `CHECK` rejects a `PRIVATE` row with `protection = 'NONE'` | That key rotation, key derivation, or nonce handling are correct |
| With the test double, a `PRIVATE` row's stored bytes are not the plaintext | Anything about the sealed store for `SECRET`, which does not exist |
| `SECRET` and `CREDENTIAL` writes are refused on every path | Anything about macOS Keychain custody |

**Consequence for P2's closure claim: a P2 deployment holds no `PRIVATE` durable
data**, because a `PRIVATE` write is refused with no backend configured.

The enforcement is the dispatch, **not** an assumption that nothing can produce a
`PRIVATE` value. An earlier draft of this document made that assumption — "nothing in
P2 can *produce* a `PRIVATE` value — provider data classification is P9/P10" — and it
is **false**: `AssistantTask.data_class` is host-assigned at task creation
(Task Protocol §2), and Data Classification §2 names exactly this kind of content,
calendar event titles, as `PRIVATE`. A host that creates a `PRIVATE` task gets a
typed refusal, which is the correct behaviour. Recorded so that a later phase cannot
read "P2 supports `PRIVATE`" into a claim it did not earn.

**The boundary this does not cross.** `PRAGMA ignore_check_constraints = ON`
disables every `CHECK` in the schema for a local file writer, so the *structural* half
of the enforcement is defeatable by that one pragma. Triggers and foreign keys are
not `CHECK`s and every authority-bearing control here is one of those; each was
verified to hold with the pragma set. See
[ADR-0022](../decisions/ADR-0022-durable-private-data-at-rest.md) and
[P2 SQLite schema §7](P2-sqlite-schema.md#7-verified-behaviour).

**File-level encryption is explicitly rejected for P2.** SQLCipher or an
equivalent would be a new dependency with a licensing and audit burden, is not
named in Crate Map, and does not compose with per-value class enforcement, which
is what the `CHECK` above buys. ADR-0022 records the rejection.

Classification: `MUST_FIX_BEFORE_P2_IMPLEMENTATION`. Disposition: PROPOSED
[ADR-0022](../decisions/ADR-0022-durable-private-data-at-rest.md).

## 5.9 Content-addressed blob storage: rows or files

**Observed.** Task Protocol §3 says `arguments` "are retained in a separate
content-addressed blob store, referenced by `input_digest`". Neither the shape
nor the medium is decided. Data Classification §8.2 step 2 requires deleting
items, provenance rows and referenced blobs **in one transaction, in that order**.

**Resolution: SQLite content-addressed `BLOB` rows.** The word "blob" does not
imply a file, and every property the frozen text actually asks for is a
transactional property that files cannot provide:

| Required property | SQLite rows | Filesystem |
| --- | --- | --- |
| Atomicity with the referencing row | One `COMMIT` | Impossible: two stores, two journals, a torn-write window in both directions |
| Data Classification §8.2 ordered cascade in one transaction | Yes | Needs a two-phase tombstone plus a sweeper |
| Orphan prevention | A foreign key makes a dangling reference impossible; P2 writes blob and reference in one `Tx`, so a crash rolls back both | Crash between write and reference leaves an orphan needing a GC pass |
| Crash behaviour | WAL: row and payload commit or neither | Partial file, or file without row |
| Deduplication | `PRIMARY KEY` | Directory-level name collision handling |

**Classification and reference ownership.** The blob primary key is
`(digest, data_class)`, not `digest` alone. This is the safety-relevant choice:
keying by `digest` alone would let a reference classified `PERSONAL` read back a
blob stored `PRIVATE`, laundering a private payload downward. With a composite
key, a reference at class `X` can only ever resolve to bytes stored at class `X`.
Deduplication still works for the case that matters — the same arguments written
twice at the same class.

**Two reference tables, not one polymorphic one.** `blob_refs(owner_kind,
owner_id, …)` would lose referential integrity, because a polymorphic owner
cannot carry a foreign key, and right-to-delete would then depend on remembering
to delete the refs by hand. So `task_blob_refs` and `step_blob_refs`, each with a
real `FOREIGN KEY … ON DELETE CASCADE` to its owner and `ON DELETE RESTRICT` to
`blobs`. Deleting a task therefore cascades to its references automatically, and
deleting a still-referenced blob is refused by the database.

The full DDL, digest-verification policy, and the crash and orphan arguments are
in [P2-sqlite-schema.md §5](../plans/P2-sqlite-schema.md#5-content-addressed-blobs)
and [P2 design §6](../plans/P2-storage-task-engine.md#6-blob-store).

Classification: `P2_DESIGN_DECISION`, recorded in the design documents. It
changes no frozen text — "a separate content-addressed blob store" is satisfied
by a table that is separate from `tasks` and `task_steps`.

## 5.10 Lease fencing

**Observed.** Task Protocol §3.1 requires `lease_owner` and `lease_expires_at`
and says they prevent two workers executing one step concurrently. That is a
*mutual-exclusion* claim. It is not a *fencing* claim, and the difference is
exactly the attack:

1. Worker A acquires the lease at generation *n*.
2. A stalls past `lease_expires_at`.
3. Worker B sees the lease expired, reclaims it, and starts the step.
4. A wakes up and commits its success.

With only `lease_owner` and `lease_expires_at`, step 4 either succeeds — one
external effect, two writers, and B's result is lost — or fails A's write with no
distinction from a genuine constraint failure. Neither is acceptable.
[Scheduler Protocol §4](../protocols/11-scheduler-protocol.md#4-lease-and-concurrency)
already froze the correct shape for the *scheduler* lease: "a monotonically
changing lease owner token stored with the occurrence record". P2 applies the
same mechanism to task steps, so this is applying a frozen precedent rather than
inventing one.

**Resolution: monotonic generation and SQLite authority, with separate phase gates.**
[P2E E1–E11](P2E-review-and-closure.md#1-preflight-and-frozen-pre-implementation-gate)
is the frozen **lease-authority gate**, implemented and closed with actual
implementation, independent review/remediation and stable/MSRV/debug/release
validation in that record. Historical SQL/docs probes retain their original
constructibility scope. ADR-0024 stays **Proposed**: lease closure is not outcome proof.

**P2E owns acquisition/renewal/release/ceilings/overflow/caught-error atomicity:**

- Existing `leases(step_id PRIMARY KEY, owner, generation, acquired_at_ms,
  expires_at_ms, released_at_ms)` is authority; no token or generation consistency
  trigger, no schema change, no 0002. Positive u32 generation starts at 1 and
  increments on every successful acquisition/reclaim. SQL step generation 0 means
  never leased and maps to wire None; positive SQL generation maps to Some.
- Selected `Tx::acquire_lease(task_id, step_id, owner, expected_generation:
  Option<u32>, now: EpochMillis, expires_at: EpochMillis)` uses **absolute instants**
  and retains no Clock. None binds SQL0; Some(n) is exact observed positive
  generation; Some(0) is LeaseFenced. Never replace caller expectation with a
  fresh read. Core owns `max_lease_seconds`. Expiry <= now is InvalidLeaseInterval.
- For valid interval/generation inputs, missing/wrong-parent/noneligible step is
  LeaseFenced; accept
  PLANNED/LEASED/EXECUTING only. On a bound eligible step, active unreleased authority
  gives LeaseHeld before stale expectation gives LeaseFenced, then durable budget
  exhaustion gives AttemptCeilingReached, then eligible max-u32 gives payload-free
  LeaseGenerationOverflow **before mutation**. SQL also bounds the increment with
  `generation < 4294967295`. No wrap/clamp/SQLite text matching/CHECK-error inference.
- **Complete internal savepoint under BEGIN IMMEDIATE** includes authority/budget
  reads, the leases upsert then derived step UPDATE, and any ceiling/later failure.
  Keep exact expected-generation equality in the step UPDATE; derive its new
  generation from leases. Read tasks.max_attempts_per_step durably here and charge
  attempt once per successful acquisition/reclaim; refusal spends nothing.
- On any acquisition error, explicitly roll back and release the savepoint even
  if a caller catches Err and returns Ok from the outer body. Cleanup failure marks
  outer Tx rollback-only and prevents commit; do not depend on ?. Earlier caller
  writes may commit after successful cleanup, but no partial acquisition may.
- `renew_lease(&LeaseGuard, now: EpochMillis, new_expiry: EpochMillis)` checks
  authoritative task/step/owner/generation/unreleased binding. Stale/missing/released
  => LeaseFenced first; matching expired-at/before-now => LeaseExpired; then equal/
  shorter than **authoritative old expiry** => InvalidLeaseInterval unchanged.
  Strict extension changes only leases expiry, never the step acquisition snapshot.
- `release_lease(LeaseGuard, now: EpochMillis)` permits matching expired authority;
  stale/missing/released => LeaseFenced and matching now < acquired_at =>
  InvalidLeaseInterval. Only released_at changes, all step copies remain unchanged.
  Guard is consumed on every result, including infrastructure failure; Err is **not
  proof of durable release**. Inner Ok is not durable until outer commit. Safety
  over retryability requires eventual expiry/recovery after a consuming error.
- Guard fields are private, with no constructor/Clone/Copy/Serde/owner formatter or
  Drop release. SQLite alone authorizes writes. A returned guard's purported
  authority is invalidated by outer rollback. A private origin commit marker
  stays unpublished after rollback/panic/commit failure and prevents same-owner
  generation reuse from reviving an escaped capability. Pending guards work only
  inside their origin Tx. This rejection gate never replaces authoritative SQL
  and introduces no durable token, local lease map or schema change.

**P2F owns begin and embedded outcome fencing, not P2E:**

- `begin_attempt` borrows `&LeaseGuard`, requires authoritative matching unreleased/
  unexpired authority and never increments attempt again. H18 acquisition charge
  is P2E; its no-second-charge begin proof is P2F.
- Every outcome UPDATE matches step/task/owner/generation **and EXISTS** matching
  authoritative unreleased lease. Released authority refuses even with an unchanged
  step copy; same-owner reclaim fences the old outcome. Known outcome after expiry
  is allowed only while unreclaimed/unreleased; outcomes consume the guard.
- The fenced UPDATE is the first mutation. Explicit zero-row => LeaseFenced and
  rollback precede result references, receipt, task advancement and participant/
  journal append. H9–H13 must assert current/stale outcomes and no receipt/journal/
  task mutation on refusal; lease-only GREEN proves none of these.
- H15 acquisition-copy agreement and H22 cross-connection authority are P2E;
  their outcome halves are P2F. H17 physical deletion cascade is P2F, not a P2E
  release API. Use separate file-backed Stores, not a process-local lease registry;
  child-process crash evidence belongs to P2H.

See [ADR-0024](../decisions/ADR-0024-lease-fencing-and-commit-under-lease.md) for the
selected API and named SQL predicates. The P2A optional wire generation member
is already implemented; it does not establish authority or full runtime fencing.
Classification: `MUST_FIX_BEFORE_P2_IMPLEMENTATION` design resolved by the frozen
phase decisions; disposition: **Proposed ADR-0024, P2E authority implemented and
closed, P2F outcome proof pending**.

## 5.11 Time representation

**Observed baseline.** Wire `Timestamp` allows exactly two forms,
`YYYY-MM-DDTHH:MM:SSZ` and `YYYY-MM-DDTHH:MM:SS.mmmZ`, with legal year `0000`.
The baseline derived `Ord` compares spelling, not instants: the explicit string
`"2026-01-01T09:14:22Z"` sorts *after*
`"2026-01-01T09:14:22.100Z"` because `Z` (0x5A) beats `.` (0x2E), while the
first instant is earlier. Seconds and `.000Z` spellings also sort differently
while representing the same instant. Authoritative comparisons must not use text.

**Frozen accepted P2B resolution.** Introduce distinct `EpochMillis` with a
**private `i64` field**, checked constructor, `get()` and numeric `Ord`.
Inclusive bounds are `MIN = -62_167_219_200_000`
(`0000-01-01T00:00:00.000Z`) and `MAX = 253_402_300_799_999`
(`9999-12-31T23:59:59.999Z`). Durable time remains signed SQLite `INTEGER`
epoch **milliseconds**, column-named `*_at_ms`, `NOT NULL` where the event
certainly happened. `TimestampMs` remains the existing **unsigned 48-bit ULID**
timestamp, unchanged. Reaching year 10889 at its upper bound does not cover the
negative epochs in the full wire domain; it cannot serve as Clock/durable time.

The Timestamp grammar/calendar validation is unchanged. Each existing Timestamp
retains exact validated wire spelling, with spelling-based serialization, `Eq`
and `Hash`. Conversion to EpochMillis preserves the instant; conversion back
always emits canonical `.mmmZ`, including `.000Z`. Instant round-trips are exact;
original seconds-form spelling is **not recovered**. Protocol conversion lives
next to Timestamp and is reused by testkit and later storage, not restricted to
a storage-only helper. P2B removes Timestamp `PartialOrd`/`Ord`; there are no
repository consumers to migrate. Numeric ordering belongs to EpochMillis.

**P2 is the phase that introduces `Clock`.** [Crate Map §3](../architecture/03-crate-map.md#3-crate-inventory)
already freezes `Clock` as a `serea-protocol` public item; P1 recorded that a
`Clock` trait there "would be a fourth port with no P1 consumer". P2 is the first
consumer, so this fills a declared slot and needs no ADR.
The accepted signature is `Clock: Send + Sync` with synchronous, object-safe
`fn now_ms(&self) -> Result<EpochMillis, ProtocolError>`. Injection only: no
async runtime or ambient/system Clock implementation.
`serea-testkit::TestClock` is to implement it, in the existing crate identified
by Crate Map §5.1.

TestClock stores `start_ms: EpochMillis` and `now_ms: EpochMillis`; the latter is
the only current-time authority, with elapsed milliseconds derived from their
difference. `at` accepts seconds and `.mmmZ` through Timestamp validation and
conversion, including year0000 and negative epochs; `format` uses canonical
protocol conversion. No private calendar arithmetic/parser is retained.
`advance` checks the duration magnitude before narrowing, addition and domain
bounds before mutation. Zero/exact-MAX advances succeed; overflow, beyond-MAX
and `Duration::MAX` return typed errors without changing current or elapsed time.

**No ambient wall clock.** `.clippy.toml` bans `SystemTime::now` and
`Instant::now` under Clippy. Group E7 additionally checks the **current P2B
protocol/testkit sources and tests**, including embedded examples and any build
scripts present; it does not wait for new storage/engine crates. Storage's injected
`&dyn Clock` and open-time propagation of Clock errors are **P2C**, not P2B.
Successful Clock readings already meet the signed bounds by construction;
out-of-range construction refusal belongs in E5, not Store-open tests.

`deadline_at_ms` and `lease_expires_at_ms` are compared as integers. Lease
expiry, the task deadline, and `max_task_wall_clock_ms` arithmetic all become
integer comparisons.

Classification: `P2_DESIGN_DECISION`, recorded in
[P2 design §8](../plans/P2-storage-task-engine.md#8-clock-and-time-representation)
and [Group E](P2-test-matrix.md#7-group-e-clock-and-time). All P2B BLOCKER/MAJOR
design findings are **accepted and resolved by this corrected design before
production**; E1–E7 and API/ULID regressions still require implementation evidence.
No tests passed or P2B completion is claimed here. No frozen wire field/grammar,
ADR or protocol version changes; no storage implementation. Historical P2A
evidence remains unchanged.

## 5.12 Recovery scope

**Observed.** Task Protocol §6 specifies recovery that re-issues steps,
re-renders approvals against a live device roster, and reconciles in-flight
effects by read-back. None of that is available in P2: there is no Capability
Registry, no provider, no model router, and no device link.

**Resolution: P2 recovery is durable classification and decision production, not
execution.** `recover()` reads durable state, classifies every non-terminal task
against **one** exhaustive table, writes at most one conditional mutation per
task inside one `Tx`, appends a `RECOVERY_DECISION` journal row, and returns a
`RecoveryReport`. It never invokes anything and never re-effects anything.

| `RecoveryDecision` | Condition | P2 action |
| --- | --- | --- |
| `ResumeNormally` | Non-terminal, no lease held, no in-flight step | Journal only. No state change |
| `AwaitUser` | A `WAITING` step of kind `WAIT_USER` or `WAIT_SCHEDULE` exists | Journal only. P2 cannot deliver input |
| `AwaitApproval` | A `WAITING` step of kind `WAIT_APPROVAL` exists | Journal only. P2 cannot re-render against a device roster; that is P6/P12 |
| `ExpiredLease` | A held lease with `expires_at_ms <= now_ms` | Release the lease and journal. **Then classify the step separately** |
| `NeedsReconciliation` | A released-or-expired lease on a step that was `EXECUTING` | Journal. **No re-execution, ever, in P2** |
| `ReceiptAlreadyCommitted` | A `side_effect_receipts` row exists for the step and the task has not advanced | Commit the state transition from durable facts. No re-effect (Task Protocol §6, `T4`) |
| `TerminalNoop` | `state.is_terminal()` | Nothing. No journal row |
| `CorruptOrInvariantViolation` | A row violates a `CHECK`, a foreign key, or a transition predicate | Move the task `BLOCKED` with `blocked_reason: UNRECOGNISED_STATE`, or refuse the whole pass. Never silently skip |

**`ExpiredLease` -> `NeedsReconciliation`, never a blind re-execution.** Task
Protocol §6 says an expired lease on a step with no receipt and
`replay_safety: IDEMPOTENT` may be re-issued with the same key. P2 cannot read
`replay_safety` — it lives in a descriptor, and there is no registry — so P2
records the decision and P5 acts on it after restart. The decision is durable, so
nothing is lost by deferring the action.

**Idempotency of the pass (`T5`).** Every classification is a read, and every
mutation is a single conditional statement whose `WHERE` clause contains the full
precondition the read observed — state, lease generation, receipt presence and
step status. A second pass over unchanged durable state therefore matches nothing
and writes nothing. No "recovery already ran" marker is needed or added, because
a marker would be a second source of truth for a property that is structurally
true.

**What P2 must establish, and what it must not pretend.** Established: restart
persistence, idempotent recovery analysis, stale-lease detection, and the
structural absence of any duplicated external effect. Not established and not
claimed: any provider execution, any read-back reconciliation, any approval
re-render, any model turn, and `E3`.

Classification: `P2_DESIGN_DECISION`, recorded in
[P2 design §9](../plans/P2-storage-task-engine.md#9-recovery). It implements
Task Protocol §6 as far as §6 permits without a registry, and names the residue
explicitly.

---

## 6. Storage shape

**Decision: a relational schema, explicitly not one JSON blob per task.**

The prompt's prohibition is also the correct design. `TaskStep` has five fields
that must be *SQL-addressable* for the design to work at all: `status` (the
recovery predicate), `lease_generation` (the fence), `attempt` (the attempt
ceiling), `side_effect_receipts.step_id` (the `ReceiptAlreadyCommitted` predicate), and
`task_id` (parent binding). If any of those lives inside a JSON document, every
one of those predicates becomes an application-side read-modify-write, and the
atomicity that `TB-7` buys is gone.

What *is* stored as JSON, and why, is decided per column in
[P2-sqlite-schema.md §2](../plans/P2-sqlite-schema.md#2-normalised-versus-json),
with one rule: a field is JSON only if nothing ever needs to compare, order, or
predicate on it. Today that is the forward-compatible `extensions` set on
`tasks`, `TaskOrigin.extensions` and `AttemptBudget.extensions`, and
`ActionError.details`. Everything else is a column.

Class ranks are stored as integers with the label as a `GENERATED … STORED`
column, so the class a `CHECK` validates and the class a reader sees cannot
disagree — which removes an entire class of corrupt-database authority widening
before it can be written.

---

## 7. Phase-specific atomic changes

One atomic **P2A** integration gate pairs task/action contract corrections with
four Option conversions, StepPresence/kind validation, all text categories,
SCJ-1/digest/duplicate parsing/IDK-1, sha2 0.11 without defaults, four changed
schemas (task, action-request, action-result, event; envelope unchanged),
protocol manifest/per-surface registry, canonical tests, versions/changelogs and
both consumer migration notes. The corrected signed EpochMillis/Clock,
Timestamp conversion/ordering and TestClock scope in §5.11 is P2B. No seven
separate P2A commits and no action/2 without implemented primitives.

ADR-0018/19/20/23 are Accepted within their stated P2A scopes after corrected
docs GREEN and owner ratification; 0018 runtime plan/lifecycle enforcement is deferred. ADR-0021/22/24 remain Proposed for runtime P2C–P2G:
participant/journal, at-rest dispatch and full fencing do not land in P2A. Only
0024's wire generation member/validation is P2A. Its P2E lease-authority slice is
implemented and closed; begin and embedded outcome fences remain P2F (§5.10).
B3 is semantic minor, not patch,
and introduces no resource bounds. See [frozen gate](P2A-review-and-closure.md).

## 8. Open questions this design does not answer

These are recorded so tomorrow's implementation agent does not discover them
mid-code and improvise.

| # | Question | Why it is not answered here | Owner |
| --- | --- | --- | --- |
| 1 | Which real `AtRestProtection` backend, in which crate, and under what key-custody rule | Crate Map §4.2's whole argument is that credential custody is a separate crate; answering this would change the layering graph, which is out of P2's scope. ADR-0022 records the question and the two viable shapes | P2 owner, with an ADR |
| 2 | Where does the `SECRET` sealed store live | No owning crate is named anywhere, and `serea-credential-store` is scoped to `CREDENTIAL` only | P2/P3 owner, with an ADR |
| 3 | Does `NOTIFY` become capability-shaped | P2 records the obligation and P2's design treats it as host-internal; the answer depends on P5/P6's `device.*` surface | P5 owner |
| 4 | ~~Is relaxing four `TaskStep` conversions minor or major~~ — **RESOLVED by the P2 autonomous audit: major.** §4.1's minor case is a *new* optional field; this is a relaxation, and §5 sets the precedent that a weakening of a required field is breaking. The plan is `serea-arch/0.2.0 → 1.0.0`, `serea.task/1 → 2`, `serea.action/1 → 2`. Owner direction records that target; both migration-note drafts are in launch §4. Current validation/review/integration evidence is coordinator-owned in the [closure record](P2A-review-and-closure.md), not inferred from historical measurements | Architecture owner / P2A integration gate |
| 5 | The numeric values for every resource bound | No evidence exists. Inventing a number would be the `MAX_VALUE_LENGTH` mistake P1 already retracted | Bounds owner, with its own ADR. See [P2 design §12](../plans/P2-storage-task-engine.md#12-resource-bounds-still-open) |
| 6 | Whether `insert_at` plan revisions are ever needed | P2 V1 is append-only. The cost is that a mid-plan insertion requires a new task | P2/P4 owner |
| 7 | ~~The minimum `rusqlite` feature set~~ — **RESOLVED by the P2 autonomous audit.** `rusqlite` **0.40.2** with `default-features = false, features = ["bundled"]`, bundling SQLite **3.53.2**, so the `STRICT`/`GENERATED` fallback is verified unnecessary. `default-features = false` is **required**: rusqlite's defaults pull `hashlink` and `sqlite-wasm-rs`. `libsqlite3-sys`'s defaults (measured: `min_sqlite_version_3_34_1` → pkg-config/vcpkg) select **system SQLite**; `bundled` overrides the discovery path, though `pkg-config`/`vcpkg` are still compiled because `rusqlite` does not pass `default-features = false` to it. `bundled-full` rejected (81 packages compiled vs 20). **Correction from the final closure run:** the earlier claim that this row "raises owner decision #1" (an MSRV rise) was based on a misreading of `libsqlite3-sys`'s manifest — it declares no `rust-version` and is `edition = "2021"`, and `cargo +1.85.0` builds and runs this exact configuration. **No owner decision arises.** See [P2 design §7.4](../plans/P2-storage-task-engine.md#74-rusqlite-and-the-alternatives) | Closed — no owner decision |

## 9. Cross-references

- The decisions these findings feed:
  [ADR-0018](../decisions/ADR-0018-taskstep-lifecycle-and-field-presence.md),
  [ADR-0019](../decisions/ADR-0019-canonical-json-and-idempotency-preimage.md),
  [ADR-0020](../decisions/ADR-0020-bounds-b3-scope-clarification.md),
  [ADR-0021](../decisions/ADR-0021-p2-p3-event-atomicity-seam.md),
  [ADR-0022](../decisions/ADR-0022-durable-private-data-at-rest.md),
  [ADR-0023](../decisions/ADR-0023-text-field-validation-categories.md),
  [ADR-0024](../decisions/ADR-0024-lease-fencing-and-commit-under-lease.md)
- The design those decisions produce:
  [P2 storage and task engine](P2-storage-task-engine.md),
  [P2 SQLite schema](P2-sqlite-schema.md),
  [P2 test matrix](P2-test-matrix.md)
- What P1 closed, and what it deliberately left open:
  [P1 closure](P1-closure.md)