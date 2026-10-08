# ADR-0034: Capability manifest, registry generations, and pinning

Status: **Accepted** · Date: 2026-10-08 · Architecture: `serea-arch/2.6.0`

## Decision

A trusted host-owned `CapabilityManifestV1` is authoritative for capabilities
that may exist. Entries identify CapabilityId, SemVer, ProviderId, optional
ImplementationId, host-computed descriptor semantic digest, input/output
trusted schema catalog identities/digests, and whether the implementation is
an allowed candidate. ProviderId must match the CapabilityId namespace.
Ordinary registration rejects ProviderId `host` and IDs beginning `host.`.
Only a future explicit privileged `HostBuiltin` origin may register reviewed
`host.goal.*`; P5 registers none. `goallatch.*` remains invalid by grammar.

An advertised descriptor must exactly match a manifest entry. An unmanifested
descriptor, version, implementation, provider namespace, schema, or authority
change fails provider registration and never creates registry authority. The
provider cannot widen descriptors, replace schemas, lower risk, or change
authorization or replay safety. This allowlist, not name heuristics, is the
structural no-arbitrary-shell guarantee.

Logical descriptor identity is `(CapabilityId, SemVer, optional
ImplementationId)`. `ImplementationId = None` is legal only when exactly one
implementation exists for a CapabilityId and version in the manifest
generation. If multiple exist, each has a distinct `Some(ImplementationId)`.
Duplicate exact identity rejects generation activation. Provider vector, map,
and hash iteration order never establishes identity or priority.

Each activation creates immutable `DescriptorRevision` records with a
host-computed semantic digest over all authority-bearing descriptor facts and
referenced schema digests. A semantic change creates a new revision and
generation; history is never rewritten. A monotonically increasing
`CapabilityRegistryGeneration` snapshots revisions, explicit per-CapabilityId
default versions, ordered implementation priorities/configuration, and schema
catalog revision.

Each post-P5 Task pins exactly one generation. New capability Steps resolve
from it. A pre-P5 Task may retain NULL generation but cannot add a capability
Step; it fails closed with a typed unavailable/internal host outcome and is
never implicitly rebound. Updates affect Tasks created afterwards. Existing
bindings retain exact revision and implementation through retry, recovery,
replan, and removal.

A separate live overlay keyed by CapabilityId applies across versions and
implementations, with enabled/disabled and removed states. New bindings pass
the current overlay, including in old Tasks. Already-bound Steps are not
abandoned and may retry/recover against their pinned revision. Experimental
capabilities are hidden and unbindable by default. Durable local-admin opt-in
keyed by CapabilityId is required; model and ordinary device settings cannot
enable them. Opt-out blocks new bindings. Experimental status is not policy,
approval, or authorization.

The manifest explicitly selects one default version per CapabilityId. No
provider order, runtime `latest`, latency, or model choice selects a version.
Prereleases are eligible only when that exact version is marked default. The
model never supplies `capability_version`.

The manifest defines ordered implementation candidates. One immutable host
availability snapshot (including root/device eligibility) and one provider
health snapshot per logical registry operation are sampled. Select the first
manifest-authorized eligible READY implementation. Health affects availability
only. A bound implementation never changes automatically; if unavailable,
return `CAPABILITY_UNAVAILABLE`. A replacement Step may bind afresh within its
Task's pinned generation and current overlay. Descriptor authority is sampled
only at generation construction.

`CAPABILITY_REGISTRY_CHANGED` records generation activation and durable
overlay, experimental, removal, and reactivation mutations. Mutation and
metadata-only event commit in one SQLite transaction through fixed upper-layer
composition; storage has no Event Bus dependency. `POLICY_CHANGED` remains for
policy rules only.

## Consequences

P5 migration 0004 is limited to registry generations, descriptor revisions,
generation membership/default/priority and trusted schema references, admin
overlays, nullable Task generation for legacy rows, and immutable Step bindings.
It has no policy, approval, dispatch, duplicate, repeat, action-result, receipt,
reconciliation, or tool-call count tables. Provider invocation is prohibited
through P5 and P6; P8 is first permitted to invoke providers. This is an
architecture-minor change only; `serea.action/2` remains unchanged and internal
types create no new public API.
