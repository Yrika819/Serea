# P0 — Architecture Freeze Closure

- **Project:** Serea
- **Architecture version:** `serea-arch/0.1.0`
- **Branch:** `p0/architecture-freeze`
- **Status:** **Closed as a documentation/contract phase only**, subject to the evidence and limitations below.
- **Scope boundary:** No P1 implementation, Rust workspace, Android project, real provider, real credential, or GoalLatch/Local MCP connection was created.

## Frozen package

P0 freezes the architecture and protocol contract surface across `docs/architecture/`, `docs/protocols/`, `docs/threat-model/`, `docs/decisions/`, and `docs/plans/P1-workspace-and-protocol-skeleton.md`. The document validator at `tools/validate_docs.py` checks Markdown links/anchors, JSON examples, identifiers, and placeholders.

The material boundaries are:

- Serea Core owns task state, scheduling, memory, routing, policy, approvals, events, and device sessions.
- Models propose structured requests only. Host schema validation, registry resolution, deterministic policy, approval, provider execution, evidence, and durable state remain the authority chain.
- A task's host-assigned policy ceiling is immutable. Broader authority requires a new task.
- Retry/recovery retains a step's idempotency key; conditional/ambiguous effects require reconciliation before a new step/key.
- The proactive watcher is read-only and is assigned to P11.
- Android is rootless-first; optional root is isolated and finite. P0 does not implement Android components.
- P0 defines only the GoalLatch adapter contract. No provider is implemented or registered. The deterministic fake is planned for P15; P16 is readiness closure for a possible separately authorized future real-adapter phase.
- `codex_allowed = false`; normal failures cannot route to Codex.

## Review findings and disposition

Independent read-only contract reviews were performed against the architecture/protocol/threat-model/P1-plan surface. Findings were resolved in-scope before closure:

1. Execution-pipeline example contradicted immutable task ceilings and placed duplicate/repeat checks before policy/approval. Corrected to creation-time host ceiling and protocol order.
2. Conditional retry described a new key per attempt; corrected to same durable step/key for eligible transport retries, with a new step/key only after reconciled absence.
3. Unregistered evidence/event names appeared in examples and failure mappings. Replaced with registered capability evidence/event vocabulary and `BOUND_EXCEEDED` plus `bound_name`.
4. P15/P16 GoalLatch ordering and P11 watcher references were inconsistent. Aligned phase mappings and explicitly made real adapter work unscheduled and separately authorized.
5. Grant IDs were absent from the identifier registry/validator. Added `GrantId = grt_ + ULID` and field validation.
6. Scheduler protocol ownership was omitted from architecture maps. Added `PROTO-SCHED`/`serea-scheduler` ownership.
7. P1 planned a GoalLatch fake too early and several P0 passages described a fake as already implemented or resolvable. P1 now has exactly the protocol/testkit workspace members and no GoalLatch implementation; affected architecture and threat-model passages now consistently say contract-only at P0, fake planned at P15.
8. Documentation validation did not cover ULID range or complete JSON-field values. Validator now checks ULID length/alphabet/range and structured identifier/idempotency fields, including wrong prefix, malformed type, and multiline value cases.

The final bounded re-review confirmed the last threat-model GoalLatch correction and found no remaining claim that the fake is available or implemented at P0. The scoped reviewers also found no remaining contradiction in policy ordering, immutable task ceiling, retries, event/evidence vocabulary, phase mapping, or P1 boundaries.

## Verification evidence

- `python3 -m py_compile tools/validate_docs.py` — passed.
- `python3 tools/validate_docs.py docs` — passed; **38 Markdown files scanned**, identifiers, JSON, placeholders, and cross-references valid.
- Temporary adversarial validator examples were added and removed after confirming rejection of: wrong `grant_id` prefix, 26-character ULID outside the 128-bit range, multiline idempotency value with a `.extra` suffix, null `grant_id`, and numeric `idempotency_key`.
- `git` branch check — `p0/architecture-freeze`.
- No Rust or Android build/test was run: neither implementation project exists in P0.
- No runtime acceptance was claimed; no credentials or external services were used.

## Known limitations retained explicitly

P0 is a design/contract freeze, not implementation evidence. The threat model records the following gaps without asserting mitigations that do not exist:

- No integrity seal/tamper evidence for policy, approval, registry-overlay, or event state against a local file writer.
- Payload-byte, attachment-size, and object-count bounds are not specified, despite the current bounds catalogue claiming completeness.
- Android exported-component/Intent policy is not yet frozen.
- No universal provider freshness guarantee; freshness must be provider-specific.
- No dedicated external model-provider quota bound beyond local accounting estimates.
- Ordinary approval does not require biometrics; the frozen requirement applies to elevated-device approval.
- No Rust workspace, durable task engine, scheduler, model router, provider implementations, Android application, or Pixel 7a/API 37 evidence exists yet.

These gaps require explicit design/ADR/protocol work in the relevant later phase before any claim of enforcement or acceptance. They do not authorize weakening the frozen authority boundaries.

## P0 exit decision

**P0 CLOSED.** The architecture package is coherent within the documentation scope and the available documentation validation passed. This does not mark any runtime/product phase green and does not authorize P1 automatically; P1 requires a separate implementation task. No real GoalLatch integration is authorized.

## Proposed exact P1 instruction

> Implement only P1 — Workspace and Protocol Skeleton, following `docs/plans/P1-workspace-and-protocol-skeleton.md` exactly. Before editing, inspect/use the applicable installed planning and TDD skills. Create the Rust workspace with exactly `serea-protocol` and `serea-testkit` members, protocol types/schemas/empty ports, deterministic synthetic test doubles, focused tests, and secret-free CI. Write behavior-bearing tests first. Do not create task engine, persistence, scheduler, Android project, external-service clients, a GoalLatch provider/fake, or any GoalLatch/Local MCP connection. Do not access or modify `$HOME/local-mcp` or any project outside `$HOME/Desktop/Serea`. Run the plan's exact offline verification commands, conduct independent code and regression reviews, resolve in-scope findings, document limitations, and stop at P1 closure without beginning P2.
