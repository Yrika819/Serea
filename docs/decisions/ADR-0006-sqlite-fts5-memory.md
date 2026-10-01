# ADR-0006: SQLite FTS5 Memory, No Vector Database

- Status: **Accepted**
- Architecture version: `serea-arch/0.1.0`
- Decision date: 2026-10-01

## Context

Memory is derived user data whose provenance, classification, retention, and deletion must remain explicit. The P0 architecture names SQLite and a content-addressed blob store and defers a vector database; semantic recall is described as ranking durable rows. FTS5 is selected as the initial SQLite-native text index, not as a change to the memory protocol.

## Decision

Store memory records and provenance in SQLite, keep any large payloads in the content-addressed blob store, and use SQLite FTS5 for initial lexical retrieval. Do not add an embedding model or vector database in this phase. Memory creation remains an explicit `EXTRACTION` operation and every record inherits source classification and provenance.

## Consequences

- P1 can establish common memory-related identifiers/types and schema compatibility without implementing memory persistence or retrieval.
- Initial retrieval is local and inspectable; it does not introduce embedding egress, index lifecycle, or a second storage system.
- FTS5 ranking is not claimed to provide semantic similarity. A future retrieval change requires measured need and a separate decision that preserves classification and deletion semantics.
- SQLite build configuration must verify FTS5 availability on the supported host target before the memory phase.

## Rejected alternatives

- Vector database at the workspace skeleton stage: adds operations and data-egress complexity before the retrieval contract needs it.
- Automatic memory from every provider read: turns untrusted external content into durable ambient context without explicit extraction.
- Keep all memory only in model context: context is discardable and is not durable state.

## Frozen source docs

[System Overview §6](../architecture/01-system-overview.md#6-non-goals-at-this-phase); [Crate Map §3](../architecture/03-crate-map.md#3-crate-inventory); [Data Classification §§7–8](../protocols/09-data-classification-protocol.md#7-classification-of-provider-data); [Task Protocol §§1, 8](../protocols/02-task-protocol.md#8-task-retention-and-privacy); [Threat Model Assets AST-13](../threat-model/01-assets-and-trust-boundaries.md#1-assets).
