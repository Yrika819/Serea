# P2 Contract Gap Analysis

- **Project:** Serea
- **Branch:** `p2/design-preparation`
- **Base commit:** `c3737039e3e38dbba554dc0b9075025f87948358`
  (`p1/workspace-protocol-skeleton`, P1 closed)
- **Architecture version in force:** `serea-arch/0.2.0`
- **Scope:** design preparation only. No production Rust, no SQLite code, no
  `serea-storage` or `serea-task-engine` source, no dependency change. This
  document records what P2 must decide before it can be written, and it decides
  it.

## 1. How to read this document

Every finding is classified into exactly one bucket. The bucket is a statement
about *when* the finding must be dealt with, not about how hard it is.

| Class | Meaning | Consequence for P2 |
| --- | --- | --- |
| `MUST_FIX_BEFORE_P2_IMPLEMENTATION` | P2 cannot be written correctly until this is resolved, because the code would otherwise have to fabricate a value or break a frozen promise. | The protocol change and the code change must land **atomically** in one commit. Recorded as a PROPOSED ADR. |
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
| 5.4 | The `‖` idempotency preimage **collides on legal input today** | `MUST_FIX_BEFORE_P2_IMPLEMENTATION` | [ADR-0019](../decisions/ADR-0019-canonical-json-and-idempotency-preimage.md) |
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
`result`, and — for every kind but `CAPABILITY` — no `idempotency_key`. The
current shape has no representation for it except fabricating timestamps and
digests, which would corrupt `result_digest`'s stated purpose ("Detects result
corruption or partial writes on recovery") and make `started_at` a lie.

**Resolution: one flat wire object, five fields become nullable, and presence
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

**Resolution (c): the compatibility question, stated honestly.** Converting five
fields from required to optional is not literally the "new optional field" case
that Protocol Index §4.1 names as minor, because it *weakens* validation for a
consumer. Two mitigations make the classification defensible, and the owner must
still ratify it:

- `serea.task/1` has exactly one producer and one consumer in V1 — the host
  itself. No cross-host consumer of a task document exists, so no deployed
  consumer can start accepting something it previously rejected.
- The relaxation is not uniform. For the three step kinds that can reach an
  external effect, the schema *gains* `if`/`then` clauses that make
  `capability_id`, `capability_version`, `provider_id` and `idempotency_key`
  conditionally **required**. Fail-closedness increases where it matters and
  decreases only where nothing external happens.

Classification: `MUST_FIX_BEFORE_P2_IMPLEMENTATION`. Disposition: PROPOSED
[ADR-0018](../decisions/ADR-0018-taskstep-lifecycle-and-field-presence.md), which
carries the exact amendment, the exact code change list, and the compatibility
note. Architecture-version treatment is flagged there as an owner decision
between a minor step and a major one; this run does not decide it.

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
formula inputs do not exist for most of those kinds, yet both the Rust type and
the schema make all of them unconditionally present or required.

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

Classification: `MUST_FIX_BEFORE_P2_IMPLEMENTATION`. Disposition: PROPOSED
ADR-0018 §3.

## 5.3 Canonical JSON is specified but not implemented

**Observed.** Protocol Index §5 states the rule in one sentence: keys sorted
lexicographically by UTF-8 code point, no insignificant whitespace, no trailing
newline, UTF-8, numbers in shortest round-trip form. No function implements it.
P1 closure also records that `serde_json` is used without `arbitrary_precision`,
so "two distinct integers above `u64::MAX` collapse to one `Value`".

**Why "sort the keys" is not a specification.** Four of the five clauses are
underdetermined and each one produces a different digest:

- *Shortest round-trip form* is implementation-defined at the edges. An `f64`
  has no single portable shortest decimal spelling; two conforming
  implementations can differ, so the digest is not portable. Worse, a
  `f64`-derived digest of a value that is *mathematically* an integer is a
  classic duplicate-suppression hole.
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
   `put_blob_value`. This closes the differential at the type level instead of by
   convention.
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

Classification: `MUST_FIX_BEFORE_P2_IMPLEMENTATION`. Disposition: PROPOSED
ADR-0019 §1. Note that P2 introduces `sha256` to the workspace, so `sha2` (or an
equivalent) is a **new dependency**; it is named here and added in P2B, not
today.

## 5.4 The idempotency preimage collides on legal input today

**This is not a theoretical ambiguity. Two different actions derive the same
key under the frozen notation.**

Capability Protocol §8.2 writes the preimage with a `‖` that the documents never
define. Treating it as raw concatenation:

| Tuple | `capability_id` | `capability_version` | `arguments` | Naive preimage |
| --- | --- | --- | --- | --- |
| **A** | `p.r.list` | `0.0.0` | `-12` | `p.r.list0.0.0-12` |
| **B** | `p.r.list` | `0.0.0-1` | `2` | `p.r.list0.0.0-12` |

Every component of both tuples is legal under the frozen grammars:

- `p.r.list` is three segments matching `[a-z][a-z0-9_]{1,31}` with `list` drawn
  from the frozen verb set (Capability Protocol §2).
- `0.0.0` is a valid SemVer, and so is `0.0.0-1` — `1` is a valid numeric
  pre-release identifier.
- `-12` and `2` are both legal canonical JSON integers under SCJ-1 rule 6.

An exhaustive search over the frozen grammars (14 verbs × 405 SemVer forms × 20
argument forms = 113 400 triples) finds this family and no other: the collision
is always between a **SemVer pre-release identifier** and a **negative argument
integer**. Searching for a collision across the `capability_id` /
`capability_version` boundary finds none in 39 424 triples, because every
identifier segment must start with `[a-z]` and every SemVer numeric identifier
must start with a digit.

The consequences are not cosmetic. These two tuples are *different actions*:
different pinned descriptor versions, different arguments, different
`arguments_digest` (`sha256:7ed00270e394c3e190d18a977f5d0e1ed889bdc03a637e8fe35f00946841b0bf`
versus `sha256:d4735e3a265e16eee03f59718b9b5d03019c07d8b6c51f90da3a666eec13ab35`).
Under one key they would:

- let `C6` and `B11` suppress a *different* action as a duplicate;
- let a crash replay of B be recognised as A;
- let an `APPROVAL`-protocol grant bound to A's exact `arguments_digest`
  authorise B, because the key that identifies the action is the same;
- let two steps in one task carry the same key, which P2's
  `UNIQUE (task_id, idempotency_key)` must then reject — a *correct* index
  turning a *wrong* key derivation into a spurious "duplicate key" failure.

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

**Change control.** Capability Protocol §8.2's `‖` notation is prose that never
defined an encoding. Making the encoding explicit is a specification-precision
change to a *structured* derivation, on a value Serea has never minted, with no
cross-version key obligation. ADR-0019 records it as an architecture-minor
clarification and states that a different classification is the owner's call. It
is **not** recorded as a silent change: if the owner judges the frozen formula
normative rather than illustrative, §8.2 must be amended in the same commit that
implements IDK-1.

Classification: `MUST_FIX_BEFORE_P2_IMPLEMENTATION`. Disposition: PROPOSED
ADR-0019 §2–§3.

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
ADR; and `serea-protocol/src/schema.rs`, unchanged.

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

| Category | Fields | Rules |
| --- | --- | --- |
| **O — opaque token / reference** | `ActorId`, `LeaseOwner`, `ProviderReference` | Non-empty. No C0 control, no DEL. No leading or trailing whitespace. **Refused if the value parses as a prefixed or fixed-shape frozen identifier domain** — the eleven ULID prefixes, `idk_`+64 hex, `sha256:`+64 hex, or a `CapabilityId` — so a `ProviderReference` cannot impersonate a `StepId` in an audit row or a `LeaseOwner` cannot be mistaken for a `ProviderId`. **`ProviderId`, `ModelId` and `ImplementationId` are deliberately excluded**: measured, refusing them produced 8 false positives out of 14 legitimate opaque tokens including `calendar` and `worker`, because those grammars subsume ordinary words. Length bounded by the owning schema's `maxLength` |
| **L — single-line label** | `TaskTitle`, `DescriptorTitle`, `EffectSummary`, `PlainSummary` | Non-empty. No C0 control including `\n` and `\t`. No leading or trailing whitespace. Length bounded by the owning schema's `maxLength` |
| **P — prose** | `ErrorMessage`, `DescriptorDescription` | Non-empty. `\n` and `\t` permitted. Every other C0 control and DEL refused, including `\r`, so CR/LF normalisation cannot smuggle a line break past the renderer. No leading or trailing whitespace. Length bounded by the owning schema's `maxLength` |

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

**Schema side.** The single `freeText` definition becomes three definitions, and
each free-text field cites its category. The whitespace-only divergence is fixed
with a negative lookahead, which ECMA-262 supports and which this repository
already uses in three capability-identifier patterns:

```text
^(?![ \t\n\r\f\v]*$)[^\u0000-\u001f\u007f]+$          category L
^(?![ \t\n\r\f\v]*$)[^\r\u0000-\u001f\u007f]+$        category P (no \r)
```

Classification: `MUST_FIX_BEFORE_P2_IMPLEMENTATION` for both halves. Disposition:
PROPOSED [ADR-0023](../decisions/ADR-0023-text-field-validation-categories.md).
The Rust validator change is atomic with the schema change; neither alone is
correct.

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
| A transaction primitive in P2 that P3 fills without rewriting P2 | **Accepted.** Described below |

**The seam.** `serea-storage` owns the *transaction*. A transaction participant is
a trait object whose `participate` runs **inside** the caller's
`BEGIN IMMEDIATE … COMMIT`, so anything it writes commits or rolls back with the
state change:

```rust
/// Immutable description of the transition being committed. Built once, by the
/// `Tx` method performing the state write, and passed to every participant, so
/// no participant can record a different transition from any other.
pub struct DurableTransition<'a> { /* occurred_at_ms, actor, causation_id,
                                      data_class, payload_digest, payload_json */ }

pub trait TransactionParticipant {
    fn participate(&mut self, tx: &mut Tx, t: &DurableTransition<'_>)
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
`pending_event_transitions: u64`, counting journal rows whose `event_seq IS
NULL`, and a P2 test asserts that number is greater than zero after a task
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

**Resolution: a monotonic `lease_generation`, checked inside the statement.**

- `leases(step_id PRIMARY KEY, owner, generation, acquired_at_ms, expires_at_ms,
  released_at_ms)`.
- `generation` starts at 1 and increments on **every** acquisition, including an
  expiry reclaim. It never resets and never decreases.
- **No `token` column.** An earlier draft of this resolution specified one, and
  ADR-0024 removed it as overengineering: `generation` alone already discriminates
  two acquisitions by the same owner after a reclaim, and a second value that must be
  kept consistent with the first, and that no statement needed, is a liability. The
  ADR is the authority; this summary was corrected to match rather than left
  forwarding a schema the ADR deleted.
- `acquire_lease` is **two statements in one `BEGIN IMMEDIATE`**: the `leases`
  upsert, whose `DO UPDATE` `WHERE` permits the update only when the lease is
  released or expired, and then the step update, which **reads** its generation back
  out of `leases` rather than guessing it. A competing acquisition affects zero rows
  on the first statement and is refused. There is no read-then-write window, and the
  two generation copies cannot diverge observably.
- The acquire predicate accepts `PLANNED`, `LEASED` **and** `EXECUTING`. A
  `PLANNED`-only predicate made retries, expiry reclaim, and Task Protocol §6.3
  structurally unreachable, because a step whose lease was released is still
  `LEASED`/`EXECUTING`.
- **Every** outcome write carries the fence predicate
  `WHERE step_id = ? AND task_id = ? AND generation = ? AND owner = ?`
  and treats zero affected rows as `LeaseFenced`.
- The fenced write is the **first** statement in the transaction. SQLite does not
  roll back on a zero-row `UPDATE`, so the `rows_affected == 0` check must be
  explicit and must happen before any receipt or journal row is written.

The five required semantics are specified in full in
[ADR-0024](../decisions/ADR-0024-lease-fencing-and-commit-under-lease.md):
acquire, renew, release, commit-under-lease, expired reclaim. `renew` refuses to
extend an already-expired lease, which is what stops a stalled worker from
resurrecting its own fence.

"Do not rely solely on process-local mutexes" is honoured literally: the only
mutex in `serea-storage` guards the single SQLite connection. No mutex guards
lease semantics, and no lease predicate is evaluated in process memory.

Adding `lease_generation` to the wire `TaskStep` is a **new optional field**, so
it is an architecture-minor addition and is folded into ADR-0018's field matrix
rather than needing its own amendment.

Classification: `MUST_FIX_BEFORE_P2_IMPLEMENTATION`. Disposition: PROPOSED
ADR-0024.

## 5.11 Time representation

**Observed.** Wire `Timestamp` allows exactly two forms, `…THH:MM:SSZ` and
`…THH:MM:SS.mmmZ`. `Timestamp` derives `Ord`, so a lexicographic comparison works
*only* when both sides have the same fractional-digit form — `…:22Z` sorts
*after* `…:22.100Z` because `Z` (0x5A) beats `.` (0x2E). A lease expiry stored in
the second form and compared against `now` in the third form is therefore wrong
in a way that only shows up on some rows. The two forms must never be compared as
text.

**Resolution.** Durable time is `INTEGER` epoch **milliseconds**, column-named
`*_at_ms`, `NOT NULL` where the event certainly happened. The wire form is
produced only at the storage boundary, in both directions, by functions that
live in `serea-protocol` next to the `Timestamp` type that owns the grammar.
`TimestampMs` already exists there as a validated 48-bit millisecond value, and
`2^48-1` milliseconds is year 10889, so it covers the whole wire range and one
type serves both.

**P2 is the phase that introduces `Clock`.** [Crate Map §3](../architecture/03-crate-map.md#3-crate-inventory)
already freezes `Clock` as a `serea-protocol` public item; P1 recorded that a
`Clock` trait there "would be a fourth port with no P1 consumer". P2 is the first
consumer, so this fills a declared slot and needs no ADR.
`serea-testkit::TestClock` implements it, which is where Crate Map §5.1 already
says `TestClock` belongs.

`TestClock` needs one structural change: today its authoritative state is six
calendar integers plus an `elapsed_ms` counter, so a `now_ms()` accessor would
have to invert calendar arithmetic that could drift. P2 makes `now_ms: u64` the
single source of truth and derives the calendar fields on demand for `format()`.
One authority, no inversion, and `elapsed_ms` becomes derivable.

**No ambient wall clock.** `.clippy.toml` already bans `SystemTime::now` and
`Instant::now` workspace-wide, so a production wall clock in `serea-storage`
would be a clippy error rather than a review comment. `serea-storage` takes
`&dyn Clock` at construction and reads it nowhere else.

`deadline_at_ms` and `lease_expires_at_ms` are compared as integers. Lease
expiry, the task deadline, and `max_task_wall_clock_ms` arithmetic all become
integer comparisons.

Classification: `P2_DESIGN_DECISION`, recorded in
[P2 design §8](../plans/P2-storage-task-engine.md#8-clock-and-time-representation).
It adds no field to any frozen wire surface.

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

## 7. What P2 must change atomically, in one commit

The prompt requires the package to state exactly which protocol and code changes
must land together. Tomorrow's P2A is precisely this list, and nothing in P2B
through P2I may start before it lands.

| # | Protocol side | Code side, same commit |
| --- | --- | --- |
| 1 | Task Protocol §3.1 gains a step-lifecycle table and the field-presence matrix; §3 gains `lease_generation` as an optional member | `TaskStep`'s five fields become `Option`; the lifecycle and step-kind matrices are added as checked constructors; `assistant-task.schema.json` `$defs/step.required` is reduced and `if`/`then` clauses added |
| 2 | Capability Protocol §8.2's `‖` notation is replaced by a reference to IDK-1 and its exact byte layout | `canonicalize`, `digest_of`, `derive_idempotency_key` land with the ten pinned SCJ-1 vectors and the collision-regression vector |
| 3 | Bounds Protocol gains a "what a bound is not" clause; `B3` is scoped | **No code change at all.** A documentation-only clarification, with a negative test asserting no behaviour changed |
| 4 | Capability Protocol §3.1's `maxItems`/`maxLength` bullets are annotated as structural | None |
| 5 | Data Classification §5 gains a note that `PRIVATE` durable storage without a configured backend is refused, not degraded | `AtRestProtection` trait, `StoreError::AtRestProtectionUnavailable`, `StoreError::ClassRefused`, and the rank cap on every classified table |
| 6 | Capability Protocol §3's field-semantics section gains the three text categories | `validate_opaque_token`, `validate_single_line_label`, `validate_prose` replace the single `validate_label`; `action-result.schema.json`'s `freeText` becomes three definitions, and the six inline patterns in `assistant-task.schema.json` plus the one in `event.schema.json` become `$ref`s |
| 7 | Event Protocol gains a note that `E3`/`E4` become enforceable **forward only** once `serea-event-bus` supplies a transaction participant, with no change to `E3` itself and no reconstruction of pre-P3 history | `DurableTransition`, `TransactionParticipant`, `TaskJournal`, `task_journal`, `RecoveryReport.pending_event_transitions` |

The seven rows above are **seven separate atomic commits**, not one. Each pairs
one protocol amendment with the code that satisfies it, and splitting them further
would let one land without its partner. Grouping them differently — protocol text
first, code second — would leave the repository claiming a contract the code does
not yet implement, which is the specific failure
[Protocol Index §7](../protocols/00-protocol-index.md#7-change-control) exists to
prevent.

Architecture-version treatment is an owner decision recorded in each ADR rather
than assumed here. This run applies none of it.

---

## 8. Open questions this design does not answer

These are recorded so tomorrow's implementation agent does not discover them
mid-code and improvise.

| # | Question | Why it is not answered here | Owner |
| --- | --- | --- | --- |
| 1 | Which real `AtRestProtection` backend, in which crate, and under what key-custody rule | Crate Map §4.2's whole argument is that credential custody is a separate crate; answering this would change the layering graph, which is out of P2's scope. ADR-0022 records the question and the two viable shapes | P2 owner, with an ADR |
| 2 | Where does the `SECRET` sealed store live | No owning crate is named anywhere, and `serea-credential-store` is scoped to `CREDENTIAL` only | P2/P3 owner, with an ADR |
| 3 | Does `NOTIFY` become capability-shaped | P2 records the obligation and P2's design treats it as host-internal; the answer depends on P5/P6's `device.*` surface | P5 owner |
| 4 | ~~Is relaxing five `TaskStep` fields minor or major~~ — **RESOLVED by the P2 autonomous audit: major.** §4.1's minor case is a *new* optional field; this is a relaxation, and §5 sets the precedent that a weakening of a required field is breaking. The plan is `serea-arch/0.2.0 → 1.0.0`, `serea.task/1 → 2`, `serea.action/1 → 2`. **What the owner now supplies is the migration-note text**, which §7 item 4 requires and which is short because `serea-core` and the Android client are P12 | Architecture owner, ratification only |
| 5 | The numeric values for every resource bound | No evidence exists. Inventing a number would be the `MAX_VALUE_LENGTH` mistake P1 already retracted | Bounds owner, with its own ADR. See [P2 design §12](../plans/P2-storage-task-engine.md#12-resource-bounds-still-open) |
| 6 | Whether `insert_at` plan revisions are ever needed | P2 V1 is append-only. The cost is that a mid-plan insertion requires a new task | P2/P4 owner |
| 7 | ~~The minimum `rusqlite` feature set~~ — **RESOLVED by the P2 autonomous audit.** `rusqlite` **0.40.2** with `default-features = false, features = ["bundled"]`, bundling SQLite **3.53.4**, so the `STRICT`/`GENERATED` fallback is verified unnecessary. `default-features = false` is **required**: rusqlite's defaults pull `hashlink` and `sqlite-wasm-rs`. `libsqlite3-sys`'s defaults select **system SQLite** via pkg-config/vcpkg, which `bundled` overrides. `bundled-full` rejected. See [P2 design §7.4](../plans/P2-storage-task-engine.md#74-rusqlite-and-the-alternatives) | Closed — but it raises owner decision #1 |

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