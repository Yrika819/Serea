# ADR-0005: SQLite WAL and Ordered Migrations

- Status: **Accepted**
- Architecture version: `serea-arch/0.1.0`
- Decision date: 2026-10-01

## Context

Serea is a single-user, single-host application, but durable task state, receipts, grants, usage, and ordered events must survive restart and commit atomically. The P0 architecture selects SQLite and migrations. WAL mode is an implementation-level storage choice for the host workload, not a new wire or protocol contract.

## Decision

Use SQLite as the host durable relational store, enable Write-Ahead Logging for the database connection, and apply versioned, ordered migrations before Core starts accepting work. Changes that must be atomic—including an event and the state change it describes—use one database transaction. Secret and credential storage remain outside the ordinary SQLite database per the data-classification protocol.

## Consequences

- Storage is local and transactional; no database service is introduced.
- Migration application is a startup gate and schema changes are explicit and ordered.
- WAL supports concurrent readers while preserving SQLite's single-writer transaction model; checkpointing, durability settings, and backup behavior must be verified against the supported platform before implementation release.
- SQLite and WAL do not provide protocol-level integrity sealing; the threat-model gap about host storage integrity remains unresolved by this choice.

## Rejected alternatives

- In-memory state: loses tasks, approvals, and event continuity on restart.
- A remote database: adds an unnecessary service and trust boundary for the frozen single-host architecture.
- Per-file JSON persistence: cannot reliably provide the required cross-record atomic commits.
- Treat WAL as a substitute for migration control, transaction boundaries, or storage integrity protection: it provides none of those guarantees.

## Frozen source docs

[System Overview §1](../architecture/01-system-overview.md#1-purpose-and-scope); [Crate Map §3](../architecture/03-crate-map.md#3-crate-inventory); [Task Protocol §§1, 5, 6](../protocols/02-task-protocol.md#5-execution-rules); [Event Protocol §5](../protocols/06-event-protocol.md#5-ordering-and-delivery); [Data Classification §5](../protocols/09-data-classification-protocol.md#5-egress-rules); [Threat Model Assets AST-10 and AST-16](../threat-model/01-assets-and-trust-boundaries.md#1-assets).
