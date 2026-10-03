# ADR-0022: Durable `PRIVATE` Data at Rest

- Status: **Proposed** — acceptance requires the deferred green P2D dispatch/migration/test gate
- Architecture version: `serea-arch/0.2.0` at the time of writing
- Decision date: not yet ratified
- Recorded by: P2 design preparation, from `c3737039e3e38dbba554dc0b9075025f87948358`
- Feeds: [P2 contract gap analysis](../plans/P2-contract-gap-analysis.md) §5.8

> P2A records the proposed fail-closed direction only. At-rest dispatch, schema
> migration and tests are deferred to P2D; no backend or runtime enforcement is
> implemented or accepted by this docs run.

## Context

`PRIVATE` is the only class with a storage condition attached. Data
Classification §2 permits it in "Durable storage, **encrypted at rest**". §5's
egress matrix repeats it. [Trust Boundaries
§4.1](../architecture/02-trust-boundaries.md#41-permitted-crossings-by-destination)
repeats it for `TB-7`. Crate Map's `serea-storage` row says the crate owns
"retention and **redaction-at-rest**".

There is no owner. The only crate permitted secret custody is
`serea-credential-store`, and Crate Map §4.2 makes it a *separate* crate for six
enumerated reasons, the first of which is that it "is the only crate whose
interior is permitted to hold secret bytes". Its dependency edge points one way:
`serea-credential-store` depends on `serea-protocol`; `serea-storage` does **not**
depend on it. Reaching the Keychain from `serea-storage` would invert the layering
graph, which is the thing §4.2 exists to prevent.

So `serea-storage` must be able to refuse a `PRIVATE` write it cannot protect,
and the refusal must be total: no plaintext fallback, no "encrypt later", no
warning-and-continue.

## Decision

### Enforcement is by class, in one place, structurally

| Class | P2 behaviour |
| --- | --- |
| `PUBLIC` | Ordinary content store |
| `PERSONAL` | Ordinary content store |
| `PRIVATE` | **Refused** with `StoreError::AtRestProtectionUnavailable` unless an injected `AtRestProtection` is configured whose `classes_protected()` contains `PRIVATE`. With one configured, the value is stored under `protection = 'AT_REST'` |
| `SECRET` | **Always refused** by the ordinary store: `StoreError::ClassRefused`. Routed only to a future sealed store |
| `CREDENTIAL` | **Always refused**: `StoreError::ClassRefused`. Its only permitted destination is the OS credential store, which P2 is not |

```rust
pub trait AtRestProtection: Send + Sync {
    fn classes_protected(&self) -> &'static [DataClass];
    fn protect(&self, plaintext: &[u8], class: DataClass) -> Result<Vec<u8>, StoreError>;
    fn unprotect(&self, ciphertext: &[u8], class: DataClass) -> Result<Vec<u8>, StoreError>;
}
```

"Fail closed" here means precisely: with no backend configured, a `PRIVATE`
write returns an error and **nothing reaches disk**.

### The class is enforced in SQL as well as in Rust

```sql
data_class_rank INTEGER NOT NULL CHECK (data_class_rank BETWEEN 0 AND 2)
CHECK ((data_class_rank = 2) = (protection = 'AT_REST'))   -- blobs only
```

The cap has to be on **every** table that stores classified content, not just on
`blobs`. An earlier draft of this ADR claimed "`SECRET` and `CREDENTIAL` cannot
reach ordinary SQLite by any route" while `tasks.data_class_rank` allowed ranks
0-4 — so a `tasks` row classified `CREDENTIAL` was accepted, and the claim was true
of the blob store and false of everything else. Verified fixed: rank 3 and rank 4
are refused on `tasks`, `blobs`, `side_effect_receipts` and `plan_revisions`.

A `SECRET` or `CREDENTIAL` row cannot be constructed by any writer that does not
disable constraint checking — a hand-written `INSERT`, a future code path that
bypasses the Rust check, or a `sqlite3` script. A `PRIVATE` row without a protection
tag cannot either.

**There are two qualifiers, and this ADR previously named only one.**
`PRAGMA ignore_check_constraints = ON` disables every `CHECK` in the schema, so a
local file writer can set `tasks.data_class` to `SECRET` through it. Confirmed by
execution. What that writer **cannot** do is fire a trigger or violate a foreign
key, and every authority-bearing control here is one of those:
`tasks_policy_class_immutable`, `tasks_data_class_monotonic`, the three
`side_effect_receipts_*` triggers, `task_steps_idempotency_key_immutable` and
`task_journal_step_task_matches`. All were verified to hold with the pragma set —
**and that claim is true**, which the P2 autonomous audit established by isolating
the pragma rather than assuming it.

**The second qualifier is `PRAGMA foreign_keys`, and it is the more direct of the
two.** It is an ordinary per-connection setting that **defaults to `OFF`** in
SQLite, and one statement disables it:

```sql
PRAGMA foreign_keys = OFF;   -- outside a transaction: takes effect
BEGIN IMMEDIATE;
INSERT INTO task_steps (… task_id …) VALUES (…, 'tsk_…BNB', …);  -- ACCEPTED
COMMIT;
```

No trickery, and no privilege beyond write access. This matters here more than the
first qualifier, because the composite-key anti-laundering property in
[P2 SQLite schema §5.3](../plans/P2-sqlite-schema.md#53-classification-and-laundering)
— a `PUBLIC` reference can never resolve a `PRIVATE` blob, which is `DC3`'s
enforcement — rests on `FOREIGN KEY` clauses and therefore on that pragma.

So the complete statement of what this ADR claims is:

> The rank cap and the protection-tag biconditional are **structural** guarantees
> against any writer that leaves `foreign_keys = ON` and
> `ignore_check_constraints = OFF`. They are **pragma-dependent** against a local
> file writer, who disables either with one line. Every trigger, and referential
> integrity itself, holds only while `foreign_keys` is on.

This is the correct place to have spent the structural budget rather than trying to
defend the `CHECK` layer, because
[Trust Boundaries §2 `TB-7`](../architecture/02-trust-boundaries.md#tb-7-core-to-durable-store)
already puts filesystem permissions at "defence in depth, not the mechanism" and
[Security Invariants §6](../threat-model/04-security-invariants.md) records
tamper-evidence against a local file writer as "Not specified". Inventing a defence
against an attacker the threat model has already excluded would re-open the question
ADR-0020 just closed. What was missing was never a control — it was the second
sentence.

The class is stored as an integer `rank` with the label as a
`GENERATED … STORED` column, so the value the `CHECK` validates and the value a
reader sees cannot disagree.

### P2 ships no real backend, deliberately

A real backend needs a cipher and a key source. The key source that exists is the
macOS Keychain, which is `serea-credential-store`'s. Putting a real backend in P2
would either invert Crate Map §4.2's edge or invent a second one.

So:

- P2 provides the **trait** and the fail-closed plumbing.
- P2 provides a **test double in `serea-testkit`**, which is dev-only and
  mechanically unreachable from any runtime crate by
  `tests/workspace_smoke.py`. Its documentation says in its own first line that
  it is **not encryption**: it exists to prove the wiring and the refusal path,
  and nothing else.
- A real backend is a separate decision with its own crate-boundary question,
  recorded as open question 1 in the P2 gap analysis.

### File-level encryption is rejected for P2

SQLCipher or an equivalent would be a new dependency with a licensing and audit
burden, is not named in Crate Map, and does not compose with per-value class
enforcement — which is what the `CHECK` buys, and what `SECRET`-in-sealed-store-only
actually requires. A single file-wide key would also mean the whole database is
encrypted to protect one column, which is a different threat model from the one
Data Classification §2 states.

## What P2 can and cannot test

Stated plainly so tomorrow's closure record cannot overstate it.

| P2 **can** test | P2 **cannot** test |
| --- | --- |
| `PRIVATE` write with no backend returns `AtRestProtectionUnavailable` and writes no row | That a real backend is cryptographically sound |
| The rank cap rejects a hand-inserted `SECRET` or `CREDENTIAL` row on **all five** classified tables | That a real key is protected at rest |
| **Not a "can test" item.** A local writer with `PRAGMA ignore_check_constraints = ON` is **not stopped** from writing a `SECRET` rank, and neither is one with `PRAGMA foreign_keys = OFF` from breaking referential integrity. P2 does not attempt to stop either. What the first cannot do is fire a trigger or violate a foreign key, and each of those was verified to still hold |
| The `CHECK` rejects a `PRIVATE` row with `protection = 'NONE'` | Key rotation, key derivation, nonce handling, tag verification |
| With the test double, a `PRIVATE` row's stored bytes are not the plaintext | Anything about the sealed store for `SECRET`, which does not exist |
| `SECRET` and `CREDENTIAL` are refused on every write path, including the classified-text path | Anything about macOS Keychain custody |
| A rollback after a refused write leaves no row | Anything about at-rest behaviour on a machine P2 does not run on |

## The consequence P2 must state at closure

**A P2 deployment holds no `PRIVATE` durable data**, because a `PRIVATE` write is
refused with no backend configured. The enforcement is the dispatch — **not** the
claim that nothing could produce a `PRIVATE` value. That claim was in an earlier
draft and it is false: `AssistantTask.data_class` is host-assigned at task creation
(Task Protocol §2), and Data Classification §2 names exactly this kind of content —
calendar event titles — as `PRIVATE`. A host that creates a `PRIVATE` task gets a
typed refusal, which is the correct behaviour.

Recorded so that a later phase cannot read "P2 supports `PRIVATE`" into a claim P2
did not earn.

### What is *not* structurally enforced, stated plainly

`tasks.title`, `tasks.result_summary`, `task_steps.error_message` and
`side_effect_receipts.effect_summary` are `TEXT` columns that may hold
`PRIVATE`-classified prose. A SQLite `CHECK` cannot record a per-column class, so
these are enforced by the **classified-write dispatch** — a single
`Tx::put_classified_text` chokepoint that no other write path uses — and not by the
schema.

That is a weaker guarantee than the blob store's, and it is claimed as weaker. The
"put the rule where it cannot be forgotten" move reaches `blobs`; for text columns
it reaches "one function, plus a test asserting no second write path exists". An
earlier draft claimed the schema enforced these columns too. It did not.

## Proposed amendment

The following data-classification annotation remains proposed for the P2D
implementation gate, not a P2A delivery requirement. Dispatch source, migration,
refusal/rollback tests and this annotation must land together and pass green
before acceptance. A real encryption backend and key custody remain separate
open decisions; a test double cannot close them.

A sentence added under §5's matrix, plus a changelog section:

> A durable store that has no configured at-rest protection backend **refuses** a
> `PRIVATE` write with a typed error. It does not store the value in plaintext, it
> does not defer the encryption, and it does not emit a warning and continue. The
> refusal is the enforcement of this row for a host that has not yet configured a
> backend. See ADR-0022.

The row's own wording is **unchanged**.

## Deferred runtime implementation gate (P2D, not P2A)

| File | Change |
| --- | --- |
| `crates/serea-storage/src/classify.rs` | `AtRestProtection` trait, `StoreError::AtRestProtectionUnavailable`, `StoreError::ClassRefused` |
| `crates/serea-storage/src/blob.rs` | `put_blob` classification dispatch; no `put_blob_value`, per ADR-0019 |
| `crates/serea-storage/migrations/0001_initial.sql` | The two `CHECK`s on `blobs`; `data_class_rank` plus a generated label |
| `crates/serea-testkit/src/at_rest.rs` | `TestAtRestProtection`, labelled as not encryption |
| `crates/serea-storage/tests/` | The refusal, the `CHECK`, and the round-trip cases in the test matrix |

## Consequences

- `SECRET` and `CREDENTIAL` cannot reach ordinary SQLite by any route that leaves
  constraint checking enabled. `task_journal` was the one table an earlier draft
  capped at 0-4 with a free-form `payload_json`, so a `CREDENTIAL`-ranked journal row
  carrying a refresh token was accepted with no pragma at all; it is now capped at
  0-2 like every other classified table, and verified across all five.
- `PRIVATE` is unavailable until a backend exists, which is a real functional
  limitation with no workaround in P2. It is stated rather than hidden.
- The class is readable and comparable as an integer, so a later `CLASS`-indexed
  retention or redaction pass does not need a string comparison.
- Every classified *text* column — task titles, error messages, effect summaries —
  goes through a classified-write path with the same dispatch, so `PRIVATE` prose
  cannot bypass the blob rule by landing in a `TEXT` column.

## Open questions this ADR does not answer

| # | Question | Why not here |
| --- | --- | --- |
| 1 | Which real backend, in which crate, under what key custody | Answering it changes the Crate Map layering graph, which is outside P2's declared scope. The two viable shapes are a new L1 crate beside `serea-storage`, or an injected implementation composed in `serea-core` |
| 2 | Where the `SECRET` sealed store lives | No owning crate is named anywhere in the repository, and `serea-credential-store` is scoped to `CREDENTIAL` only |

## Rejected alternatives

| Alternative | Why rejected |
| --- | --- |
| Store `PRIVATE` in plaintext and encrypt in a later phase | Data reaches disk unencrypted. That is the failure `DC` exists to prevent, and "later" is not a mitigation |
| Silent degradation with a warning | A warning is not an enforcement; the caller would proceed and the value would be on disk |
| Require a backend at `Store::open` | Correct once a real backend exists, but it would make `PRIVATE` unimplementable in P2 in a way that also blocks the refusal tests. The dispatch is per-write, so the refusal path stays reachable |
| Put the key in an environment variable or a config file | A key next to the ciphertext is not protection. Data Classification §3's custody rules apply to an at-rest key as much as to a provider secret |
| Make `serea-storage` depend on `serea-credential-store` | Inverts Crate Map §4.2's deliberate edge and gives the durable-state crate access to the only interior permitted to hold secret bytes |
| Encrypt the whole file with SQLCipher | New dependency with licensing and audit burden; not named in Crate Map; does not compose with per-value class enforcement |
| Store `SECRET` in a `BLOB` column with an access flag | Data Classification §5 is explicit: "never in the general database, never in the events table". A flag is not a sealed store |
| Add a `may_store` boolean column to `blobs` | A boolean is a comment. A `CHECK` on the class is an enforcement |