# ADR-0022: Durable `PRIVATE` Data at Rest

- Status: **Proposed** — P2D blob GREEN alone cannot accept this ADR; complete ordinary-row PRIVATE protection remains unresolved
- Architecture version: `serea-arch/0.2.0` at the time of writing
- Decision date: not yet ratified
- Recorded by: P2 design preparation, from `007f038af19ae7855ad00b7e58389ce04d0fe727`
- Feeds: [P2 contract gap analysis](../plans/P2-contract-gap-analysis.md) §5.8

> P2A recorded the proposed fail-closed direction only. The
> [frozen P2D gate](../plans/P2D-review-and-closure.md) now narrows the design to
> JSON blob put/get and the owned PRIVATE-only seam; P2C's migration is unchanged.
> Complete ordinary-row PRIVATE protection is deferred/fail-closed even with a
> blob backend. This reconciliation implements no runtime, records no runtime
> test PASS and does not accept the ADR.

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

### P2D blob dispatch is class-first and fail-closed

| Class | Frozen P2D blob behaviour |
| --- | --- |
| `PUBLIC` | SCJ-1 canonical plaintext, `protection = 'NONE'` |
| `PERSONAL` | SCJ-1 canonical plaintext, `protection = 'NONE'` |
| `PRIVATE` | **Refused** with `StoreError::AtRestProtectionUnavailable` without a backend, including read/dedupe of an existing row. With a backend, protect canonical plaintext and store backend bytes under `AT_REST`; backend refusal/failure is `AtRestProtectionFailed` |
| `SECRET` | **Always refused** by the ordinary store: `StoreError::ClassRefused`. Routed only to a future sealed store |
| `CREDENTIAL` | **Always refused**: `StoreError::ClassRefused`. Its only permitted destination is the OS credential store, which P2 is not |

```rust
pub struct AtRestProtectionError; // payload-free unit error
pub trait AtRestProtection: Send + Sync {
    fn protect(&self, plaintext: &[u8]) -> Result<Vec<u8>, AtRestProtectionError>;
    fn unprotect(&self, protected: &[u8]) -> Result<Vec<u8>, AtRestProtectionError>;
}
```

The object-safe trait is **PRIVATE-only**: presence means PRIVATE capability,
with no class parameter or capability list. `Store` owns
`Option<Arc<dyn AtRestProtection>>`, not a borrow; its normal constructors have
no backend. `open_with_protection` and `open_in_memory_with_protection` delegate
to the unchanged P2C open path and introduce no Store lifetime. `Tx` accesses
protection only within its existing transaction lifetime. `AtRestProtectionError`
carries no payload, dynamic diagnostics or source chain; storage maps it to
`AtRestProtectionFailed` with payload-safe error rendering.

Dispatch occurs **before any dedupe success**: SECRET/CREDENTIAL refuse on
put/get, and PRIVATE without a backend refuses even if the exact row exists.
A refused write stores no blob; it never falls back to plaintext, encrypt-later
or warning-and-continue. The default Store may still hold PUBLIC/PERSONAL data.

`Tx::put_blob` accepts original UTF-8 JSON bytes, not arbitrary binary or a
parsed `Value`. SCJ-1 refusals apply to PLAN/PLAN_REVISION/ARGUMENTS/INSTRUCTION/
RESULT documents. Fractional model temperature remains noncanonicalizable; no
coercion or promise that all wire-valid JSON is admissible. SHA-256 of canonical
**plaintext** is the identity; PRIVATE content is the backend's opaque envelope,
which may be nondeterministic. `size_bytes` is stored-content length including
envelope/expansion, not logical length or a bound.

Reads and same-class dedupe require exact `(digest, rank)` lookup, protection
marker and stored-size verification, unprotect when PRIVATE, SCJ-1
canonicalization and plaintext-digest verification. Reads return canonical
plaintext bytes. Corrupt metadata/content is `BlobCorrupt`; backend failure is
`AtRestProtectionFailed`; a missing exact row is `BlobMissing`. Conflict-success
or `INSERT OR IGNORE` cannot substitute for verification. No caller expected
digest or `DigestMismatch` write variant exists.

`BlobRef` privately holds validated `Digest`/`DataClass`, with public constructor,
read accessors and safe `Debug`, but no `Serialize`. It is not host authorization
or inferred classification; a lower-class ref never resolves a higher-class-only
row. Reference attachment/role APIs, deletion and blob+reference atomicity remain
P2F; blob rollback alone is neither that proof nor a crash test.

### The class is enforced in SQL as well as in Rust

```sql
data_class_rank INTEGER NOT NULL CHECK (data_class_rank BETWEEN 0 AND 2)
CHECK ((data_class_rank = 2) = (protection = 'AT_REST'))   -- blobs only
```

The cap has to be on **every** table that stores classified content, not just on
`blobs`. An earlier draft of this ADR claimed "`SECRET` and `CREDENTIAL` cannot
reach ordinary SQLite by any route" while `tasks.data_class_rank` allowed ranks
0-4 — so a `tasks` row classified `CREDENTIAL` was accepted, and the claim was true
of the blob store and false of everything else. Historical SQL probes verified
rank 3/4 refusal on the five named tables once `task_journal` was added to the
inventory; that evidence is not P2D runtime validation. Current production 0001
also caps both reference tables, for seven rank-capped tables in total.

With CHECK enforcement enabled, a rank-3/4 row cannot be constructed by a
hand-written `INSERT`, a future path bypassing Rust or a `sqlite3` script. Only
`blobs` has the PRIVATE/protection-tag biconditional: ordinary PRIVATE rows are
SQL-accepted without a protected representation. Even on blobs, a correctly
labelled plaintext `AT_REST` row is SQL-accepted; the marker proves no encryption
or backend use, and rank caps cannot infer the class of locally relabelled bytes.

**There are two qualifiers, and this ADR previously named only one.**
`PRAGMA ignore_check_constraints = ON` disables every `CHECK` in the schema, so a
local file writer can set `tasks.data_class` to `SECRET` through it. Confirmed by
historical execution. That pragma does **not** disable triggers or foreign keys
(while `foreign_keys = ON`), so their enforced conditions still refuse violations:
`tasks_policy_class_immutable`, `tasks_data_class_monotonic`, the three
`side_effect_receipts_*` triggers, `task_steps_idempotency_key_immutable` and
`task_journal_step_task_matches`. All were verified to hold with the pragma set —
**and that claim is true**, which the P2 autonomous audit established by isolating
the pragma rather than assuming it.

**The second qualifier is `PRAGMA foreign_keys`, and it is the more direct of the
two.** Upstream SQLite **defaults to `OFF`**; the selected bundled build defaults
to ON, but Store must set/assert ON explicitly. One statement disables it:

```sql
PRAGMA foreign_keys = OFF;   -- outside a transaction: takes effect
BEGIN IMMEDIATE;
INSERT INTO task_steps (… task_id …) VALUES (…, 'tsk_…BNB', …);  -- ACCEPTED
COMMIT;
```

No trickery, and no privilege beyond write access. This matters here more than the
first qualifier, because the composite-key anti-laundering property in
[P2 SQLite schema §5.3](../plans/P2-sqlite-schema.md#53-classification-and-laundering)
— durable references must match a blob's digest **and rank** — rests on
`FOREIGN KEY` clauses and therefore on that pragma. Disabling it permits dangling
lower-class references, but does not make P2D's exact lookup resolve a PRIVATE-only
row; no digest-only fallback is permitted.

So the complete statement of what this ADR claims is:

> Rank caps and the blob protection-tag pairing require
> `ignore_check_constraints = OFF`; referential integrity requires
> `foreign_keys = ON`. Ordinary triggers are independent of both settings.
> These are structural schema guarantees within that boundary, not encryption
> proof or tamper-evidence against a local file writer who can disable enforcement.

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

- P2D specifies the **PRIVATE-only blob trait** and fail-closed plumbing.
- Its synthetic reversible double is **local `cfg(test)` storage code**, not
  a reusable `serea-testkit` API or a production backend: **NOT ENCRYPTION, NOT
  SECURITY, NEVER PRODUCTION**. It can test wiring/refusal, not cryptography.
  [Crate Map §5.4](../architecture/03-crate-map.md#54-p2d-storage-local-at-rest-double-exception)
  records the narrow exception: a testkit implementation would require a new
  testkit → storage edge. Storage currently has no dev-dependency section or
  testkit edge; a cycle would be a risk only if a storage → testkit dev edge were
  also added. No manifest, dependency or smoke-rule changes are needed.
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

Required test coverage, **not executed P2D runtime evidence**, stated plainly so
closure cannot overstate it. Historical SQL probes above retain their own scope.

| P2 **can** test | P2 **cannot** test |
| --- | --- |
| No-backend PRIVATE put/get/dedupe returns `AtRestProtectionUnavailable`, including existing-row reuse; a refused put writes no blob | That a real backend is cryptographically sound |
| Rank caps reject rank 3/4 on all seven current classified tables, using private SQL fixtures | That a real key is protected at rest |
| CHECK/FK violations are accepted when the corresponding pragma is disabled; ordinary triggers remain independent | Defense against a local file writer or relabelled/copied content |
| Blob CHECK rejects PRIVATE/NONE; correctly labelled plaintext AT_REST is an accepted SQL control | Encryption from the marker, key rotation, derivation, nonce or tag soundness |
| Local test-double PRIVATE stored bytes differ from plaintext; storage unprotects, canonicalizes and verifies the plaintext digest on read/dedupe | Production encryption or a SECRET sealed store |
| SECRET/CREDENTIAL refuse on both blob put/get, including constructed refs; there is no classified-text API | macOS Keychain custody or complete PRIVATE ordinary-row support |
| Corrupt rows refuse on read and dedupe; backend errors have no payload/source chain; stored length includes envelope/expansion | Resource bounds or cryptographic authentication |
| Blob rollback leaves no inserted blob | Blob+reference atomicity/deletion (P2F), crash durability (P2H), or unexecuted portability tests |

## The consequence P2 must state at closure

**P2 ships no real at-rest backend or production PRIVATE task support.** Default
blob constructors refuse PRIVATE put/get/dedupe even for existing rows; the
synthetic test-only backend can exercise PRIVATE blobs but is not deployment
security. Future ordinary-row PRIVATE-bearing writers must refuse before SQLite
**even when a blob backend is configured**, until the complete row design exists.

The historical "nothing can produce PRIVATE" premise remains rejected:
`AssistantTask.data_class` is host-assigned (Task Protocol §2), and calendar event
titles are PRIVATE under Data Classification §2. Such values require refusal,
not plaintext persistence. The old unqualified "a P2 deployment holds no PRIVATE
durable data" cannot describe raw SQL, a supplied backend or all row surfaces;
the enforceable claim is the scoped fail-closed API obligation above. Blob seam
GREEN alone must not be read as acceptance of this ADR.

### What is *not* structurally enforced, stated plainly

P2D D5 supersedes the historical four-TEXT-field chokepoint: there is **no
`Tx::put_classified_text` API**. `tasks.title`, `tasks.result_summary`,
`task_steps.error_message` and `side_effect_receipts.effect_summary` were only
examples, not the complete PRIVATE surface. Error details, journal JSON,
provider references and all task/step/receipt/journal extensions, including task
origin/budget extensions, can also carry PRIVATE bytes.

Neither a class cap, `json_valid` nor the blob backend supplies a reversible
protected representation for these columns. A complete ordinary-row write/read
and representation design is deferred. Every future ordinary-row PRIVATE-bearing
writer must fail closed before SQLite even with blob protection configured;
SECRET/CREDENTIAL remain refused. Do not silently encrypt text into an undefined
format or infer row support from PRIVATE blob GREEN. This unresolved surface is
why the ADR remains **Proposed**.

## Proposed amendment

[Data Classification §5](../protocols/09-data-classification-protocol.md#5-egress-rules)
now carries a useful P2D implementation annotation, not a wire/version or matrix
change. The historical proposed sentence below retains the fail-closed direction;
the annotation additionally names no-backend read/dedupe refusal and the deferred
complete ordinary-row surface. It does **not** accept the ADR. P2D must use the
unchanged P2C migration and obtain actual runtime evidence; blob GREEN still
cannot resolve ordinary-row protection, a real backend or key custody.

> A durable store that has no configured at-rest protection backend **refuses** a
> `PRIVATE` write with a typed error. It does not store the value in plaintext, it
> does not defer the encryption, and it does not emit a warning and continue. The
> refusal is the enforcement of this row for a host that has not yet configured a
> backend. See ADR-0022.

The row's own wording is **unchanged**.

## Deferred runtime implementation gate (P2D, not P2A)

| Surface | Frozen requirement |
| --- | --- |
| Storage classification/error/root wiring | PRIVATE-only `AtRestProtection`, unit `AtRestProtectionError`; payload-free `ClassRefused`, `AtRestProtectionUnavailable`, `AtRestProtectionFailed` |
| Storage Store/Tx wiring | Owned `Option<Arc<dyn AtRestProtection>>`, two protection constructors delegating to unchanged P2C open; no new Store lifetime |
| Storage blob implementation | Only Tx put/get and BlobRef; class dispatch before dedupe, exact lookup and read/dedupe verification; no text/reference/role/deletion API |
| Production `0001_initial.sql` and catalog | **Unchanged**, no 0002; checksum `sha256:d9068dccbc26ececb71be79c475080633166ba0163c62b2d98b9733512baefea` |
| Storage-local `cfg(test)` tests/double | Group G applicable P2D cases; private SQL FK/cap/corruption fixtures; no testkit API or dependency change |
| RED/GREEN and review evidence | Intended missing-API RED, never a permissive helper; actual runtime results and independent reviews belong in the P2D closure record and remain pending here |

## Consequences

- Rank-3/4 rows are refused while CHECKs are enabled; this cannot infer the class
  of bytes a writer mislabels. Historically `task_journal` allowed 0–4 with
  free-form `payload_json`, so a CREDENTIAL-ranked row was accepted without a
  pragma; capping it at 0–2 completed the five-table historical probe. Current
  production also caps both reference tables, for seven total.
- PRIVATE blob access is unavailable without a backend, including dedupe/read.
  Backend injection is a seam, not proof of security. Ordinary-row PRIVATE
  writes remain refused even with a blob backend until the full design exists.
- The class is readable and comparable as an integer, so a later `CLASS`-indexed
  retention or redaction pass does not need a string comparison.
- P2D implements no ordinary-row text/JSON writer; future writers must uphold
  the complete-surface refusal obligation, not a four-column text chokepoint.
  ADR acceptance remains deferred beyond PRIVATE blob tests.

## Open questions this ADR does not answer

| # | Question | Why not here |
| --- | --- | --- |
| 1 | Which real backend, in which crate, under what key custody | Answering it changes the Crate Map layering graph, which is outside P2's declared scope. The two viable shapes are a new L1 crate beside `serea-storage`, or an injected implementation composed in `serea-core` |
| 2 | Where the `SECRET` sealed store lives | No owning crate is named anywhere in the repository, and `serea-credential-store` is scoped to `CREDENTIAL` only |
| 3 | Complete ordinary-row PRIVATE write/read representation, including prose, JSON and all extensions | P2D chooses fail-closed disposition A, not a partial text API. A blob backend cannot protect these columns; this design must be resolved before ADR acceptance or production PRIVATE task support |

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