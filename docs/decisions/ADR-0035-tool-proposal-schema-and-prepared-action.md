# ADR-0035: Closed tool proposals, trusted schemas, classification, and preparation

Status: **Accepted** · Date: 2026-10-08 · Architecture: `serea-arch/2.6.0`

## Decision

Model-authored `ToolCallProposalV1` is a closed internal object with exactly
`version: "1"`, `capability_id`, and object-root `arguments`. It has no
RequestId, Task/Step IDs, version/provider/implementation, risk/effect/
authorization/replay facts, digest, IDK, data class, requester, deadline,
approval/policy fields, or credential handle. Any undeclared field, including
host-resolved authority, invalidates the whole proposal. The older instruction
to drop extras and continue is superseded. No ActionRequest or PreparedAction
is produced from an invalid proposal.

The host emits `MODEL_SCHEMA_VIOLATION` with TaskId, optional StepId/model
RequestId, stable violation code, offending field names, and count only. Never
include offending values, proposal/model content, arguments, or prompt.
ActionRequest is not parsed directly from model JSON.

`ToolDefinitionV1` contains exactly version `"1"`, CapabilityId, title,
description, and input schema. It excludes provider, implementation, version,
risk, side effect, authorization, replay safety, root, idempotency, credential,
and policy internals. Definitions sort by CapabilityId UTF-8 byte order.
Provider-specific function-name conversion is adapter work. Visibility requires
the Task-pinned generation, current live enablement, experimental opt-in,
structural validity, and at least one eligible READY implementation. It does
not evaluate policy/approval/grants and grants no authority.

`CapabilitySchemaCatalogV1` is host-owned and immutable. Only exact catalog
URIs under `https://serea.local/schemas/` resolve. No network, redirect, DNS,
arbitrary URL, filesystem path, or parent traversal. Same-document JSON
Pointers and exact catalog-to-catalog references are allowed. Providers may
reference trusted bytes but cannot supply or replace them. Draft 2020-12 is
required. P5 V1 structural limits under ADR-0020: 65,536 canonical UTF-8 bytes
per document, depth 64, 4,096 schema nodes total, 256 properties per object.
Cyclic `$ref` graphs are refused. Every object has `additionalProperties:
false`; `patternProperties` is forbidden; every array has `maxItems`; every
string has `maxLength`; ambiguous `oneOf` is refused when disjointness cannot
be safely proven. Overflow is a typed registration refusal, never truncation.
These are structural limits, not B3 operational counters.

Arguments cross an internal trusted classification boundary equivalent to
`ClassifiedArgumentsV1 { arguments, data_class }`. Classification comes from
host/model-output provenance, not proposal fields or bare JSON names.
Unknown/unclassified is CREDENTIAL and refused. Model proposals inherit the
trusted class of accepted structured output/source context; projection retains
or raises it and never lowers it. USER, SCHEDULER, PROACTIVE_WATCHER, and
SYSTEM callers supply trusted classification; bare JSON is refused as
CREDENTIAL. RequestedBy is independently host-provided.

`CapabilityDescriptor.data_class` is the maximum reviewed transit class. The
actual trusted argument class must be <= it. Future ActionRequest.data_class
is the exact argument class, not the ceiling. Higher arguments are refused as
a contract mismatch before P6. Runtime classification is the primary
credential-exclusion control; closed schemas and reviewed property allowlists
are defense in depth. Field heuristics may raise/refuse, never prove safety or
lower a class.

P5 produces immutable internal `PreparedActionV1`, not a final executable
ActionRequest. It carries TaskId, StepId, pinned generation and descriptor
revision, capability/version/provider/implementation, validated arguments and
canonical digest, IDK-1, trusted data class, host requester, effective
capability deadline, and pinned risk/effect/authorization/replay/idempotency/
cost facts. It has no RequestId or execution authority and means neither policy
allow nor approval. P6 receives it immutably and cannot alter bound facts. No
ActionResult, provider status, or execution ActionError is fabricated for
preparation/P6 outcomes.

At provider integration, a trusted host/provider adapter attaches output
classification independently of arbitrary JSON. Unknown output class is
CREDENTIAL and refused; accepted output must be <= descriptor data_class.
P8 closes these adapter semantics. Capability deadline is host-derived from
pinned max_duration_ms, remaining Task active wall-clock budget, and any
stricter host/caller deadline; there is no model-supplied deadline.

## Consequences

Internal ToolCallProposalV1 does not change supported `serea.action/2` or
create a public API. Provider output acceptance and execution semantics remain
deferred to P8.
