# ADR-0031: Model roster, routing requirements, and egress V1

- Status: **Accepted**
- Date: 2026-10-07
- Architecture version: `serea-arch/2.5.0`
- Affected surfaces: `serea.model/1` and `serea.bounds/1` unchanged
- Owners: Core/host preparation, Model Router, Protocols

## Context

The P4 preimplementation audit found that existing prose named routing inputs
without defining their types, trusted source, allowed combinations, or ordering.
Provider discovery and provider identity were also insufficient authority for
classification-sensitive routing. This decision closes those contracts; it
does not authorize router runtime implementation.

## Routing input and purpose matrix

The host constructs the closed `ModelRoutingRequirementsV1` value with exactly
these semantic fields: `vision_required: bool`, `tools_required: bool`,
`min_context_tokens: u32`, `min_output_tokens: u32`, and
`structured_requirement: ANY | STRICT`. It contains no arbitrary JSON,
constraint map, provider-specific field, or model-authored value. The model
cannot change it. The routing input for P4 V1 is purpose, this requirements
value, data class, an immutable trusted egress-policy snapshot, an immutable
configured roster snapshot, one logical health snapshot, and host-owned
dispatch/budget/deadline gates. Budgets and deadlines can refuse a call; they
never reorder a preference chain.

The purpose/response-format matrix is closed:

| Purpose | Response format | Structured requirement |
| --- | --- | --- |
| `CHAT` | `TEXT` only | `ANY` |
| `PLANNING` | `JSON_SCHEMA` only | `STRICT` |
| `EXTRACTION` | `JSON_SCHEMA` only | `STRICT` |
| `ANALYSIS` | `JSON_SCHEMA` only | `ANY` |
| `PROACTIVE` | `JSON_SCHEMA` only | `STRICT` |
| `STRUCTURED_REPAIR` | `JSON_SCHEMA` only | `STRICT` |

An illegal purpose/format combination is rejected before provider dispatch.
Every JSON_SCHEMA response is parsed and host-validated, including STRICT
providers. Prose is never parsed as a substitute for a required structured
response.

## Prepared call, roster, and candidate selection

P4 routing accepts only an internal host `PreparedModelCallV1` (or equivalent)
whose prompt has already passed classification, required redaction, purpose
assignment, response-format assignment, and routing-requirement construction.
Raw user text is not a trusted router input. This preparation does not
implement a general redaction engine. A public caller-set `redaction_verified`
boolean is not proof; production construction is through a trusted host seam,
with only an explicit test-only builder in contract tests.

`ModelRequest` is the post-routing provider envelope because it already has a
`model_id`. The host constructs it after selection with the selected model and
a new RequestId; no placeholder ModelId is used. `serea-model-router` must not
depend on `serea-task-engine`; immutable context is supplied by the caller,
with TaskId represented only through `serea-protocol` and durable references
through `serea-storage`.

`ModelRosterV1` is immutable trusted host configuration loaded and validated at
Core/process startup. P4 V1 has no hot reload. A restart may load different
configuration, which applies only to new call attempts; a durable attempt keeps
its own selected routing and accounting facts. Each entry contains trusted
`model_id`, `provider_id`, `deployment_class`, `enabled`, allowed capability
ceiling, and `cost_class`. Provider discovery can confirm an entry, narrow its
capabilities, or make it unavailable. Effective capabilities are the
intersection of the host ceiling and provider advertisement. Discovery cannot
add entries, add Codex, widen capabilities, change order, deployment class, or
cost class. Duplicate ModelId rejects startup; one ModelId identifies exactly
one configured provider/model entry. No first-wins or map-overwrite behavior is
permitted.

`ModelDeploymentClass` is closed to `CLOUD` and `LOCAL` and is host-configured.
It is never inferred from provider/model IDs, hostnames, or model output.
Preference is explicit, not roster iteration order. No general-purpose durable
roster version is required; each attempt snapshots facts needed for recovery.

Initial normal TEXT/structured preference chains are:

| Purpose | Ordered chain |
| --- | --- |
| `CHAT` | `nemotron-3-nano-30b`, `gpt-oss-20b` |
| `PLANNING` | `nemotron-3-nano-30b`, `gpt-oss-20b` |
| `EXTRACTION` | `nemotron-3-nano-30b`, `gpt-oss-20b` |
| `ANALYSIS` | `nemotron-3-nano-30b`, `gpt-oss-20b` |
| `PROACTIVE` | `nemotron-3-nano-30b`, `gpt-oss-20b` |
| `STRUCTURED_REPAIR` | `gpt-oss-20b` |

For `vision_required = true`, `gemma-4-31b` is the only initial eligible model,
and only for a future request surface with typed image input. The present
text-only request cannot represent image bytes and must refuse a request that
requires actual image input. No image transport is implied. Text-only calls
never select Gemma solely for its vision capability. Codex is known but
disabled and excluded from every normal, fallback, repair, and vision chain;
no health, error, or model output can make it routable. P4 adds no
`codex_allowed` field.

There is no dynamic tie-break. Filter the configured ordered chain by enabled
state, capability requirements, health, data class, and policy, then select the
first survivor. Latency, price, usage, randomness, provider response, and model
recommendation do not reorder candidates. Exactly one logical health snapshot
is taken before selection and reused for initial and normal fallback
eligibility; a health read failure is DEGRADED. Repair takes one health
snapshot at the start of its ladder and reuses it for both attempts. A degraded
repair model causes validation failure without dispatch or alternate repair
selection.

## Egress and change semantics

Each dispatch attempt carries an immutable host-resolved
`ModelEgressPolicySnapshotV1` with at least `private_cloud_egress_allowed`.
It is host-owned, never model/provider-settable, and is not persisted as policy
authority. Every fallback and repair is a new dispatch intent and must receive
a fresh resolved snapshot before outbound dispatch. Revocation prevents future
dispatches; it cannot undo an already sent request.

P4 V1 data-class routing is:

| Data class | CLOUD | LOCAL |
| --- | --- | --- |
| `PUBLIC` | permitted | permitted |
| `PERSONAL` | only a trusted prepared/redacted call | permitted |
| `PRIVATE` | policy eligibility may be evaluated; dispatch refused in P4 V1 | dispatch refused in P4 V1 |
| `SECRET` | refused | refused |
| `CREDENTIAL` | refused | refused |

The PRIVATE refusal applies even if `cloud_model_private_egress` is true:
production PRIVATE ordinary-row durable result protection is incomplete, and
P4 requires recoverable durable results. This is an implementation limitation,
not a change to the global Data Classification matrix. No PRIVATE plaintext or
digest-only substitute is stored.

## SCJ and temperature

P4 does not digest raw `ModelRequest`, treat it as an SCJ-1 canonical object,
persist a canonical request blob, or truncate, round, or stringify temperature
to manufacture a digest. `temperature` remains a finite host-selected `f64`
under the existing wire semantics. P4 rejects NaN and either infinity before
dispatch. No new temperature range is introduced where none is frozen. No
wire bump follows from this decision.

## Caller and router ownership

Core/Task orchestration supplies TaskId, ModelPurpose, prompt construction,
classification, redaction, routing requirements, configured bounds, task
cancellation state, effective deadline, and current resolved egress policy.
The Model Router owns candidate filtering, deterministic selection, call
reservation/accounting, dispatch attempt state, provider-response binding,
host schema validation, repair, fallback, and model-call events. It does not
own task lifecycle, redaction, policy setting persistence, or provider side
effects. It must not depend on `serea-task-engine`; TaskId comes through
`serea-protocol`, and durable task/accounting references through
`serea-storage`.

## Consequences and nonclaims

- The complete routing contract is host-owned and deterministic.
- P4 V1 does not hot reload rosters or prices and does not transport images or
  stream model output. Streaming support in capability descriptors remains
  descriptive; `generate()` returns one completed response.
- This ADR authorizes only contract closure. P4 runtime, migration 0003, and a
  real provider remain unstarted.

## Future P4 validation gate

Every behavior-bearing P4 commit uses paired GitHub Actions gates for Linux
stable, Linux MSRV 1.85, macOS Intel x86_64, and macOS Apple Silicon arm64.
Migration 0003 requires the full four-platform gate. GitHub Actions is
authoritative; no local platform validation is required. Release fault proof
is part of the required full gate. Cross-architecture SQLite remains optional
for docs/protocol-only P4A because this closure makes no storage-schema change.

## References

- [Model Protocol](../protocols/03-model-protocol.md)
- [Data Classification Protocol](../protocols/09-data-classification-protocol.md)
- [P4 preimplementation audit](../plans/P4-preimplementation-audit.md)
