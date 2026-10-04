# Data Classification Protocol

Protocol ID: `PROTO-DATA` · Surface: `serea.data/1` · Status: **FROZEN for P0**

This protocol defines the `DataClass` set, where each class may travel, and how
credentials are kept structurally out of the model's reach. It is read by the
policy engine ([Policy Protocol §4.2](04-policy-protocol.md#42-evaluation-order)
rule 5), by the model router
([Model Protocol §6](03-model-protocol.md#6-model-routing) step 3), by the
capability registry, and by every provider adapter.

---

## 1. Why classification exists

> A model that is told "these are secrets" is not a control. Classification is
> enforced by host code, or it is not enforced at all.

The prompt is the one part of this system that is fully under adversarial
influence: it is assembled from data, sent to a third party, and returned
output is itself an input to the next turn. Anything whose protection depends on
the model complying with an instruction is not protected.

So the design rule is:

- **No prompt instruction is a security control.** "Never reveal passwords" is a
  behavioural request to a component that has no authority over any other
  component.
- **Every classification decision is made by host code**, in the same process
  that decides whether an action may happen at all.
- **The default at every boundary is deny.** A payload with no declared class,
  or a class the host cannot verify, is treated as the highest class and
  refused.
- **A consumer that computes a higher class than the producer declared must
  treat the payload as the higher class**
  ([Protocol Index §6](00-protocol-index.md#6-envelope)). Producers under-declare
  at their peril; the consumer's own computation wins.

What classification buys: a single, auditable answer to "where may this go?"
that both the policy engine and the router can consult mechanically, without
either asking a model.

---

## 2. The `DataClass` set

Frozen at `serea-arch/0.1.0`
([Protocol Index §4.3](00-protocol-index.md#43-frozen-for-p0)). Exactly five
values. Adding one is an architecture-major change.

| Class | Definition | Examples | May be stored in | May transit to a cloud model |
| --- | --- | --- | --- | --- |
| `PUBLIC` | Published by its owner; no disclosure harm. | Serea's own documentation; a public holiday calendar; the model roster; provider capability descriptors | Durable storage; logs | **Yes** |
| `PERSONAL` | Identifies the user to the user. Harm falls on them alone. | The user's name, timezone, locale; calendar *metadata* (times, busy flags); message *metadata* (sender, subject, date); task titles; notification text | Durable storage; device cache | **Yes**, after redaction (§6) |
| `PRIVATE` | Identifies, or is sensitive about, someone other than the user, or is sensitive user content whose disclosure would harm them. | Message bodies; calendar event titles, locations, and attendee lists; notification bodies; contact records; home and work address | Durable storage, encrypted at rest; device cache | **Yes**, but only when the `cloud_model_private_egress` host setting is enabled, and always after redaction |
| `SECRET` | Disclosing this would harm the user or a third party materially, and it is not an authentication credential. | Full message threads; health, financial, or legal details; unreleased plans; internal project names; security-adjacent configuration | Sealed store only; device cache | **No** |
| `CREDENTIAL` | Authentication and authorization material. Disclosing this means impersonation. | OAuth refresh tokens; passwords; device private keys; pairing nonces; authentication challenges; biometric material; API secrets | **Only** macOS Keychain or Android Keystore | **No** |

### 2.1 Class ordering

```
PUBLIC  <  PERSONAL  <  PRIVATE  <  SECRET  <  CREDENTIAL
```

The set is **totally ordered by harm**, and the ordering is what makes
composition and inheritance well defined. It is not a lattice with incomparable
elements, so `max(a, b)` is always well defined and there is no ambiguity about
what to do with a payload of mixed classes.

> Classification is inherited. A value takes the highest class of anything it
> contains or is derived from, and a derived value never drops below its inputs.

Three corollaries:

| Rule | Statement |
| --- | --- |
| **Containment** | A list of `PERSONAL` items that includes one `PRIVATE` item is `PRIVATE`. A schema whose `items` include a `SECRET` is a `SECRET` schema. |
| **Derivation** | A memory item extracted from a `PRIVATE` email is `PRIVATE`. A summary of a `PRIVATE` document is `PRIVATE`. A model paraphrase is `PRIVATE`. Classification follows information, not formatting. |
| **Absence** | An unclassified value is treated as `CREDENTIAL`. There is no "unknown class, probably fine". |

### 2.2 Composition

When a value is assembled from several sources, the host takes the maximum:

```
compose([PERSONAL, PERSONAL, PRIVATE])        -> PRIVATE
compose([PUBLIC, SECRET])                     -> SECRET
compose([PUBLIC, PUBLIC])                     -> PUBLIC
compose([PERSONAL, unclassified])             -> CREDENTIAL
```

The `data_class` field on every envelope is the composed class of the whole
`payload`, not of its most interesting field
([Protocol Index §6](00-protocol-index.md#6-envelope)).

### 2.3 `DataClass` is not `RiskClass`

`RiskClass` in
[Policy Protocol §2](04-policy-protocol.md#2-risk-class) measures *what an
operation does*. `DataClass` measures *what the data is*. The two axes are
independent, they have different frozen sets, and both contain a member named
`CREDENTIAL` — a collision of names, not of meaning. A `CREDENTIAL`-classified
capability at `OBSERVE` risk is an oxymoron the registry rejects at
registration; the `CapabilityDescriptor` carries both fields
([Capability Protocol §3](01-capability-protocol.md#3-capabilitydescriptor))
because both constraints apply and neither substitutes for the other.

---

## 3. Credential exclusion

> Credentials are not data Serea handles. They are data Serea references.

`CREDENTIAL` is the one class that is **never LLM context**. Not redacted
context, not summarised context, not hashable context. It is absent from every
model prompt, under every configuration, permanently.

| Item | Held in | Reaches a model |
| --- | --- | --- |
| OAuth refresh tokens | macOS Keychain | Never |
| Provider passwords | macOS Keychain | Never |
| Device Ed25519 private keys | Android Keystore | Never, and never leaves the device |
| Pairing nonces | Host memory, single-use | Never |
| Authentication challenges (host, device, OAuth) | Ephemeral, per-handshake | Never |
| Biometric templates and matching material | OS / TEE only | Never; the host receives only a boolean attestation result |
| Provider API secrets | macOS Keychain | Never |

### 3.1 `CredentialHandle`

Host code never holds a secret; it holds an **opaque reference**.

```rust
pub struct CredentialHandle(String);   // Digest form: "sha256:" + 64 lowercase hex
```

The handle reuses the frozen `Digest` wire form from
[Protocol Index §2](00-protocol-index.md#2-identifier-grammar) rather than
introducing a new identifier shape. It is derived from
`(provider_id, scope_digest, key_generation)` so that:

- the same secret always yields the same handle, making handles stable and
  cacheable;
- rotating a secret changes the handle, so a stale handle fails loudly rather
  than silently reading the old value;
- the handle is **not** an address. It does not reveal a keychain service name,
  an account, a path, or a slot index.

`ProviderContext` carries a `CredentialHandle` and never the secret itself
([Capability Protocol §9](01-capability-protocol.md#9-provider-interface)). A
provider resolves the handle inside the credential-store process boundary, uses
the bytes for exactly one call, and discards them.

### 3.2 The `Secret<T>` pattern

Secret bytes are wrapped in a type that cannot be logged, formatted, cloned,
compared, or dropped without being overwritten. Type signatures only; the
implementation is `zeroize`'s on the byte buffer and `subtle`'s on the
comparison.

```rust
pub struct Secret<T: Zeroize + ZeroizeOnDrop> {
    inner: T,
    class: DataClass,
}

impl<T: Zeroize + ZeroizeOnDrop> Secret<T> {
    pub fn new(inner: T, class: DataClass) -> Self;
    pub fn class(&self) -> DataClass;
    pub fn expose(&self) -> &T;            // named call site; audited
    pub fn expose_mut(&mut self) -> &mut T;
    pub fn with<R>(&self, f: impl FnOnce(&T) -> R) -> R;
}

impl<T: Zeroize + ZeroizeOnDrop> Secret<T> {
    // No Debug, no Display, no Clone, no PartialEq, no Serialize.
}

impl From<Secret<Vec<u8>>> for CredentialHandle;
```

The contract each of those omissions buys:

| Property | Mechanism | What it prevents |
| --- | --- | --- |
| Not in logs | No `Debug`/`Display` impl; a logger formatting a `Secret` is a compile error | `log::info!("using {}", secret)` |
| Not in errors | Error types accept only `CredentialHandle` and digests | An error message that quotes a token, which ends up in a crash report |
| Not on the stack longer than needed | `expose()` is a named call site, greppable, and the only way to read the bytes | Incidental copies from ordinary field access |
| Not in a model prompt | `Secret<T>` does not implement `Serialize`, and the prompt builder accepts only `PromptFragment` values of class `PERSONAL` or lower | A serialisation sweep quietly promoting a secret into context |
| Not in a `.clone()` | No `Clone` | Two live copies existing after a retry, one of them in a buffer nobody will overwrite |
| Not in freed memory | `ZeroizeOnDrop` | Secrets persisting in a freed allocation and readable from a core dump |
| Not comparable by accident | `PartialEq` is `subtle`-based, returning a `Choice` | Timing side channels and `assert_eq!` in a test log |

---

## 4. Credential exclusion

> A credential cannot be expressed as input, so it cannot be supplied as input.
> This is structural, not editorial.

A capability's `input_schema` is JSON Schema 2020-12
([Capability Protocol §3](01-capability-protocol.md#3-capabilitydescriptor)).
The host compiles it against these constraints at registration time, and
registration fails if the schema violates any of them:

1. **`additionalProperties: false` on every object.** An undeclared property
   does not pass through unexamined; it is rejected. This is the structural
   exclusion — an undeclared credential field is a `VALIDATION` error at the
   schema gate, before the policy engine, before the provider.
2. **Every property is on an allowlist, declared explicitly.** There is no
   pattern-matched or inherited acceptance.
3. **A forbidden-property denylist is applied in addition**, as defence in depth
   against a schema author who allowlists a credential-shaped field by mistake.
4. **No `patternProperties`, no overlapping `oneOf`, no unbounded recursion**
   ([Capability Protocol §3.1](01-capability-protocol.md#31-field-semantics)).

```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "$id": "https://serea.local/schemas/gmail.messages.list.input.1.2.0.json",
  "type": "object",
  "additionalProperties": false,
  "required": [ "query" ],
  "properties": {
    "query": {
      "type": "string",
      "maxLength": 512,
      "minLength": 1
    },
    "folder": {
      "type": "string",
      "maxLength": 128,
      "default": "INBOX"
    },
    "limit": {
      "type": "integer",
      "minimum": 1,
      "maximum": 50,
      "default": 25
    }
  },
  "propertyNames": {
    "not": {
      "pattern": "^([Pp]assword|[Pp]asswd|[Pp]wd|[Ss]ecret|[Tt]oken|[Aa][Pp][Ii][_-]?[Kk]ey|[Aa]ccess[_-]?[Kk]ey|[Rr]efresh[_-]?[Tt]oken|[Bb]earer|[Aa]uthorization|[Cc]redential|[Pp]rivate[_-]?[Kk]ey|[Ss]ession[_-]?[Kk]ey|[Oo]tp|[Tt]otp|[Ss]eed[_-]?[Pp]hrase|[Mm]nemonic|[Ss]ignature|[Nn]once|[Cc]hallenge)$"
    }
  }
}
```

The denylist pattern is written out letter-by-letter because JSON Schema
`pattern` is an ECMA-262 regular expression and has no inline case-insensitive
flag. That verbosity is itself an argument for §4.1: a defence that costs this
much to write correctly, by hand, for nineteen names today, is not a defence
that will still be correct when the sixty-first name appears next quarter.

### 4.1 Why an allowlist is required, not a denylist

The denylist above is **not** the control. It is a tripwire that catches a
mistake by a competent, well-intentioned schema author. It could not carry the
guarantee on its own, and pretending otherwise is the failure mode this section
exists to prevent:

| Attack the denylist alone would miss | Example |
| --- | --- |
| A synonym the pattern does not list | `pw`, `auth`, `material`, `bearer_value`, `x_cred` |
| A credential in a *value*, not a *name* | `{ "query": "my password is hunter2" }` |
| A credential-shaped string in an opaque blob | `attachments[].data` holding a PEM key |
| Unicode confusables | `pаssword` with a Cyrillic `а` |
| Naming drift | The schema says `api_key`; the provider's API renamed it to `apiKey` |
| A legitimate field that later becomes a credential | `folder` starts storing an OAuth-derived folder token |

An allowlist closes all six, because it never asks the question "is this name
allowed?" — it asks only "is this exact property present?", and `false` for
everything else. Denylists fail **open** on novelty; allowlists fail **closed**,
which is the only acceptable direction at this boundary
([Policy Protocol §4.2](04-policy-protocol.md#42-evaluation-order)).

### 4.2 Output direction

The same constraint applies in reverse with a different rule. An `output_schema`
may not *declare* a `CREDENTIAL` field either, but a provider that needs to
return secret material must do so out of band: it returns a
`CredentialHandle`, never bytes. And because output validation is fail-closed
on both directions ([Capability Protocol §3](01-capability-protocol.md#3-capabilitydescriptor)),
a provider that leaks a token into a declared `PUBLIC` string field produces
invalid output — the step fails rather than smuggling the leak onward.

---

## 5. Egress rules

This matrix is **authoritative**. Every subsystem that moves data consults it,
and no subsystem may exceed it. `Permitted` means exactly permitted;
anything not listed is denied.

| `DataClass` | Cloud model | Local model | Durable storage | Event stream to device | Application log |
| --- | --- | --- | --- | --- | --- |
| `PUBLIC` | Permitted | Permitted | Permitted | Permitted | Permitted |
| `PERSONAL` | Permitted, after redaction (§6) | Permitted | Permitted | Permitted, after redaction (§6) | Permitted, after redaction |
| `PRIVATE` | Permitted **only** when the `cloud_model_private_egress` host setting is enabled; redacted either way | Permitted | Permitted, encrypted at rest | Permitted, after redaction (§6) | Digests, shapes, and counts only — **never payload bytes** |
| `SECRET` | **Denied** | **Denied** | Permitted in the sealed store only; never in the general database, never in the events table | **Denied** | **Denied** — digest only |
| `CREDENTIAL` | **Denied** | **Denied** | **Denied in all Serea-owned storage** | **Denied** | **Denied** — handle only, never bytes |

Three properties of this matrix are worth stating explicitly:

1. **`CREDENTIAL` reaches nothing.** Not a model, not a log, not a database, not
   the event stream, not an error message. Its only permitted destination is the
   OS credential store itself, where Serea holds a `CredentialHandle`. There is
   no row in this matrix where "permitted" appears for `CREDENTIAL` in any
   column other than the store.
2. **`SECRET` may be stored but not transmitted.** Storage and egress are
   different questions. A secret Serea legitimately holds (a configuration
   value, a sealed store item) is stored encrypted and never sent anywhere,
   including a local model.
3. **Deny is the default.** A value with no declared class, a destination with
   no rule, or a mismatch between declared and computed class is a denial.

**P2D implementation annotation (no wire/version or matrix change).** The
[frozen P2D gate](../plans/P2D-review-and-closure.md) specifies a PRIVATE-only
blob protection seam, not production PRIVATE task support. Ordinary storage
refuses `SECRET`/`CREDENTIAL`; PRIVATE blob put/get and existing-row dedupe
refuse without a configured backend before any success. A configured backend
must protect canonical JSON plaintext and reads/dedupe must unprotect and verify
its plaintext digest. The `AT_REST` SQL marker alone is not encryption proof.
No real backend ships. Complete ordinary task/step/receipt/journal PRIVATE
protection, including JSON extensions, is deferred: future PRIVATE-bearing row
writers must fail closed before SQLite **even with a blob backend** until that
design exists. [ADR-0022](../decisions/ADR-0022-durable-private-data-at-rest.md)
remains **Proposed**; this annotation claims no runtime-test PASS.

### 5.1 `cloud_model_private_egress`

The default is `false`. It is a durable host setting, changed only from the
host's local admin surface, and it emits `POLICY_CHANGED` with a before/after
diff ([Policy Protocol §7](04-policy-protocol.md#7-policy-changes-are-audited)).
It is **not** settable from the Android client's ordinary settings screen and
**not** settable by model output. It is never permanent: turning it off is
immediate and always permitted
([Bounds Protocol §3](10-bounds-protocol.md#3-who-may-set-a-bound)).

---

## 6. Redaction

Redaction happens **before** data is placed into a model prompt or into the
event stream. It is host code, it runs on the classified value, and the
unredacted form never enters a prompt buffer, a notification body, a log line,
or an `arguments_preview`
([Approval Protocol §2.1](05-approval-protocol.md#21-the-prompt-must-be-specific-enough-to-consent-to)).

> Redaction is **removal**, not derivation. A redacted projection contains
> strictly *less* information than its input, so it is not a derived value and
> does not trip the inheritance rule in §2. What redaction produces is a
> **transit class** declaration for that projection only; the stored class of the
> original is unchanged.

### 6.1 The default floor

Any prompt bound for a cloud provider is treated as at most `PERSONAL` unless
the `cloud_model_private_egress` setting permits `PRIVATE`. There is no path by
which a `SECRET` or `CREDENTIAL` value reaches any model, local or cloud,
regardless of configuration.

### 6.2 Pattern classes

| Class | Detection | Replacement | Example in → out |
| --- | --- | --- | --- |
| Email address | RFC-shaped local@domain | `<EMAIL_n>` | `mei.tanaka@example.com` → `<EMAIL_1>` |
| Phone number | E.164 and national formats, 7–15 digits with separators | `<PHONE_n>` | `+81 90-1234-5678` → `<PHONE_1>` |
| Postal address | Street-line shapes with a postal code pattern | `<ADDRESS_n>` | `1-2-3 Marunouchi, Chiyoda-ku, Tokyo 100-0005` → `<ADDRESS_1>` |
| Credential-shaped token | `eyJ` JWT header, `sk-`, `ghp_`, `xox[baprs]-`, `AKIA`, PEM `-----BEGIN` headers | `<TOKEN_n>`, and the containing field is dropped | `sk-proj-4a7f9c2b1d` → `<TOKEN_1>` |
| Account or card number | 13–19 digit runs passing a Luhn check | `<ACCOUNT_n>` | `4539 8821 0044 2193` → `<ACCOUNT_1>` |
| Person name | Host-maintained contact and account-name list | `<PERSON_n>` | `Mei Tanaka` → `<PERSON_1>` |
| Free text above the transit class | Any `PRIVATE` string with no pattern match | The **whole field is dropped** | `notes: "Do not circulate the launch plan"` → field removed |

`n` is assigned deterministically within a single prompt: the first distinct
value seen gets `_1`, the next distinct value `_2`. A repeated value keeps its
placeholder. This lets a model reason about relationships — "these two messages
are from the same sender" — without learning the identity.

### 6.3 Conservative behaviour

- **Pattern failure is a drop, not a pass-through.** An unmatched `PRIVATE`
  string is removed rather than forwarded on the theory that it probably does not
  match anything.
- **Redaction is idempotent.** Running it twice produces the same output; a
  placeholder is never re-detected as a pattern.
- **Redaction is recorded.** Each projection records `source_class`,
  `transit_class`, the pattern classes applied, and a `payload_digest` over the
  unredacted canonical JSON — never the unredacted bytes. That is what the
  timeline shows a user who asks where a fact came from.
- **Redaction is not a substitute for exclusion.** `SECRET` and `CREDENTIAL`
  are denied from model and device egress by the matrix in §5; they are excluded,
  not redacted for transit. `CREDENTIAL` is also excluded at the schema and
  credential-store boundary (§3, §4). Redaction applies to `PERSONAL` and
  permitted `PRIVATE` data as specified by the destination-specific matrix in
  §5; redaction never authorizes a destination the matrix denies.

---

## 7. Classification of provider data

Content arriving from outside the user's own requests is classified by origin,
not by content inspection, and it stays in the provider cache.

| Item | Arrives as | Retention |
| --- | --- | --- |
| Gmail message body and attachments | `PRIVATE` | Provider cache, encrypted at rest |
| Message metadata — sender, subject, date, labels | `PERSONAL` | Provider cache and durable task state |
| Calendar event title, location, description, attendee list | `PRIVATE` | Provider cache |
| Calendar event times and busy flags | `PERSONAL` | Durable task state |
| On-device notification title and body | `PRIVATE` | Device cache only; never mirrored to the host |
| Provider account identifiers and profile names | `PERSONAL` | Durable storage |
| Provider capability descriptors and health | `PUBLIC` | Durable storage |

> External content does not become permanent memory by accident. Promotion from
> cache to memory is an **explicit extraction step**, never a side effect of
> reading.

### 7.1 The extraction step

1. The memory item is created only by a model call whose host-assigned `purpose`
   is `EXTRACTION` ([Model Protocol §3](03-model-protocol.md#3-modelrequest)).
   A `CHAT` or `ANALYSIS` turn cannot create a memory item, whatever its text
   says.
2. The extraction receives the already-redacted projection (§6). It never sees
   the raw body.
3. The resulting item records its **stored class** — inherited from its source
   per §2.1, so an item extracted from a `PRIVATE` email is stored `PRIVATE` —
   and separately a **transit class** for the context it may later be rendered
   into. An item whose content cannot be projected below the required transit
   class keeps its stored class and is simply not sent to a cloud model; it is
   used by a local model or not used by a model at all.
4. Nothing derived from external content becomes readable by a cloud model
   unless the projection in step 2 allowed it. The architecture recorded in the
   memory design ADR holds that email and calendar content is user data the user
   *received*, not content the user *authored*, and therefore does not become
   ambient model context by default.
5. Every item carries provenance (§8), so this decision is auditable and
   reversible.

---

## 8. Right-to-delete and provenance

Every stored item that can influence future behaviour records where it came
from, so that deletion cascades correctly and a user can be told the truth about
a remembered fact.

```json
{
  "memory_item_id": "evt_01JQ90K7R2WQ4T9MY6DB3XHPNF",
  "statement": "Prefers design reviews before 15:00 JST",
  "stored_class": "PRIVATE",
  "transit_class": "PERSONAL",
  "provenance": {
    "task_id": "tsk_01JQ8Z9K3M7QWXR4V2T6YH0BNA",
    "step_id": "stp_01JQ8Z9M5T9WXK2H4BNPQ7RDSF",
    "capability_id": "calendar.events.list",
    "source_event_id": "evt_01JQ8ZB7H2XKM9P4QW7NRT5YCD",
    "extraction_model_id": "nemotron-3-nano-30b",
    "extracted_at": "2026-10-01T09:20:14.882Z",
    "source_class": "PRIVATE",
    "redaction": { "applied": [ "EMAIL_1", "PERSON_2" ], "payload_digest": "sha256:3b7e0d4f8a2c6b9e1d5f3a7c0e4b8d2f6a9c1e5b7d3f0a8c2e6b4d9f1a7c3e5b" }
  },
  "retention_expires_at": "2026-12-01T09:20:14.882Z"
}
```

`memory_item_id` is an `EventId` (`evt_` + ULID). At `serea-arch/0.1.0` the
identifier registry has no separate memory-item prefix
([Protocol Index §2](00-protocol-index.md#2-identifier-grammar)), and this
protocol does not invent one: a memory item is a durable record keyed by
`EventId` until an ADR adds a dedicated prefix. The `evt_` prefix is a wire
format guard so that a memory-item reference cannot be silently accepted by a
component expecting a different record type.

### 8.1 Provenance obligations

| Obligation | Rule |
| --- | --- |
| Every memory item names a task | Without a `task_id`, deletion cannot cascade and the item is not writable. |
| Every memory item names the capability that supplied the source | "Where did this come from" must be answerable in one query. |
| Every memory item names the extraction that produced it | An item the model volunteered with no extraction provenance is not stored. |
| Provenance is itself classified | The provenance record is at least the stored class; a pointer to a `SECRET` source never exposes the secret. |
| Provenance is immutable | Correcting a memory item creates a new record with a supersedes link; it does not rewrite history. |
| Provenance is user-visible | The client can render "this was learned while handling <task> from <capability> on <date>", with the `payload_digest` rather than the payload. |

### 8.2 Deletion cascades

A "forget this" request is **one transaction**, not a sequence of best-effort
deletes:

1. Resolve every item whose `provenance.task_id` is the named task, plus the
   named item itself, plus any item whose `supersedes` chain reaches them.
2. Delete the items, the provenance rows, and the content-addressed blobs those
   rows referenced — in that order, so no step leaves an orphan pointing at a
   missing row.
3. Write a **tombstone** recording the deleted item's digest, so a later
   extraction of the same source cannot silently resurrect the fact. A tombstone
   carries no content; it carries the digest and the deletion time.
4. Record `DELETION_CASCADE_COMPLETED` with the counts, so a partial failure is
   visible rather than silent.
5. Conversation history is deleted or is not; it is never left behind as a
   shadow copy of something the user asked to forget.

This extends the task-deletion rule in
[Task Protocol §8](02-task-protocol.md#8-task-retention-and-privacy): deleting a
task cascades to memory items explicitly extracted from it, while an item with
provenance from a *different* task is retained, with its own retention and its
own provenance, because deleting it would silently degrade a capability the user
still wants.

---

## 9. Invariants summary

| # | Invariant |
| --- | --- |
| DC1 | **Credentials never enter a model prompt**, under any configuration, to any model, cloud or local. |
| DC2 | **Classification is enforced by host code, not by prompt instruction.** No control in this system depends on the model complying. |
| DC3 | Classification is inherited: a value takes the highest class of anything it contains or derives from, and a derived value never drops below its inputs. |
| DC4 | An unclassified value is treated as `CREDENTIAL`; there is no unknown-and-probably-fine. |
| DC5 | `CREDENTIAL` reaches no destination but the OS credential store; the egress matrix has no other permitted row for it. |
| DC6 | Providers receive credentials only as opaque `CredentialHandle`s; secret bytes never leave the credential-store process boundary. |
| DC7 | `Secret<T>` cannot be `Debug`-formatted, cloned, serialised, or dropped without being zeroized; secret bytes reach no log, error message, or prompt. |
| DC8 | No capability `input_schema` can express a credential field: `additionalProperties: false` on every object plus an explicit property allowlist, with a forbidden-property denylist as defence in depth. |
| DC9 | A consumer that computes a class higher than the producer declared treats the payload as the higher class. |
| DC10 | Redaction is removal, not derivation: it lowers a projection's transit class, never the stored class of the original. |
| DC11 | External email, calendar, and notification content arrives `PRIVATE`, stays in the provider cache, and becomes permanent memory only through an explicit `EXTRACTION` step with recorded provenance. |
| DC12 | "Forget this" deletes the item, its provenance, and its blobs in one transaction, and writes a tombstone so the fact cannot be silently resurrected. |
| DC13 | `cloud_model_private_egress` is host-only, durable, audited, defaults to `false`, and is unreachable from model output or the device's ordinary settings. |