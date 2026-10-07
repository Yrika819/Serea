# ADR-0028: Scheduler Event Predicate and Task Template V1

- Status: **Accepted**
- Date: 2026-10-07
- Architecture version: `serea-arch/2.2.0`
- Event surface: `serea.event/1` (unchanged)
- Scheduler surface: `serea.scheduler/1` (unchanged)
- Decision owner: Serea

## Context

The Scheduler Protocol defined event predicates and task templates without
closed value shapes. P3 needs values that cannot express arbitrary event
queries or capability authority. `serea-protocol` owns their closed persisted
types and validators; `serea-scheduler` owns causal-root evaluation and
durable orchestration.

## EventPredicateV1

The canonical SCJ-1 object contains exactly `version` and `event_kind`.
`version` is `"1"`; `event_kind` is one registered `EventKind`. Duplicate
keys, unknown fields/kinds, and unsupported versions are rejected. Runtime
decodes the type and never compares raw JSON. The sole positive match is exact
EventKind equality; payload, task/step, actor, correlation/causation,
classification, timestamps, and arbitrary JSON do not participate.

The HOST_EVENT deny set is exhaustive for V1: `DEVICE_CONNECTED`;
`APPROVAL_GRANTED`, `APPROVAL_DENIED`, `APPROVAL_EXPIRED`; and all
Scheduler-owned lifecycle kinds `SCHEDULE_CREATED`, `SCHEDULE_UPDATED`,
`SCHEDULE_PAUSED`, `SCHEDULE_RESUMED`, `SCHEDULE_CANCELLED`,
`SCHEDULE_OCCURRENCE_MISSED`, `SCHEDULE_TASK_CREATED`, and
`SCHEDULE_CATCH_UP_DEFERRED`. Dedicated wake kinds remain owned by their
dedicated paths. An event causally rooted in any Scheduler occurrence is also
ineligible across every schedule. Scheduler derives the root from durable
occurrence/task and event causal identity, never caller-controlled predicate
data. V1 has no recursion override.

## ScheduledTaskTemplateV1

The canonical SCJ-1 object contains exactly `version`, `title`, and `intent`.
`version` is `"1"`; `title` uses existing `TaskTitle` validation; `intent`
uses existing prose validation. Intent is planning context only, never trusted
instruction, authority, policy, or executable code. V1 has no optional fields,
extension map, or action arguments.

The template cannot carry policy, approval, capability/provider,
ActionRequest, TaskStep, Plan, idempotency key, receipt, credential handle,
GoalLatch goal, model route, bound override, admin authorization, or arbitrary
arguments. The Schedule remains authoritative for ownership, trigger, policy
ceiling, and approval policy. Normal future Task planning and capability
validation remain authoritative for actions.

The host classifies title and intent independently; stored class is their
maximum and cannot be supplied by the template. CREDENTIAL is forbidden.
SECRET is refused because this blob path has no accepted sealed-storage
contract. PRIVATE follows the existing fail-closed protected-blob capability.
A digest is content identity/integrity metadata, not encryption or authority.

Templates use the existing content-addressed blob path. The new structural
bound `max_schedule_template_bytes = 32768` limits raw UTF-8 input and
canonical bytes. Each durable occurrence captures the template digest and
class current when it is resolved, preserving historical planning context
through schedule edits and restart. The occurrence mapping is the durable path
from mapped TaskId to Schedule, occurrence, source EventId if any, and template
version. No new global identifier or authority-bearing task extension is
introduced. References remain while a schedule, unresolved occurrence, or
existing mapped task needs the template; they are released only by durable
lifecycle cleanup, never filesystem lifetime.

A scheduled task uses `kind: SCHEDULED`, template title, Schedule policy
ceiling, inherited data class, an injected timestamp, host-generated Scheduler
origin, and ordinary host-owned attempt-budget resolution. Task Engine remains
the sole task lifecycle authority.

## Compatibility and migration

These backward-compatible contract additions and one structural bound advance
`serea-arch/2.1.0` to `serea-arch/2.2.0`. Event and Scheduler wire surfaces
remain `serea.event/1` and `serea.scheduler/1`. Under the pre-release migration
policy, migration 0002 may gain the minimum occurrence template reference
columns; migration 0001 remains immutable. No released data is reinterpreted.

## Consequences

- HOST_EVENT has deterministic exact-kind matching and no Scheduler feedback.
- Templates express intent without action arguments or authority.
- Occurrences pin immutable template versions for later planning.
- P3 does not implement model planning or external effects.
