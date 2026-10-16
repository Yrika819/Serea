-- Candidate 0010 for P8B development. NOT ACCEPTED and intentionally excluded
-- from the ordinary migration catalog. It depends on the isolated P7 0009
-- schema candidate and disposable databases only. No provider invocation path
-- or production migration is authorized by this file.
--
-- This is a durable data-shape probe, not a complete authority transaction.
-- Exact Budget semantics, commit-to-invoke eligibility, dynamic revalidation,
-- provider availability evidence, state transitions, retry authority, and
-- result/Receipt authority remain PROPOSED. Runtime must remain fail-closed.
CREATE TABLE dispatch_intents (
  request_id TEXT PRIMARY KEY CHECK (length(request_id)=30
                                      AND substr(request_id,1,4)='req_'
                                      AND substr(request_id,5,1)<='7'),
  idempotency_key TEXT NOT NULL UNIQUE CHECK (length(idempotency_key)=68
                                               AND substr(idempotency_key,1,4)='idk_'
                                               AND substr(idempotency_key,5) NOT GLOB '*[^0-9a-f]*'),
  task_id TEXT NOT NULL REFERENCES tasks(task_id) ON DELETE RESTRICT,
  step_id TEXT NOT NULL UNIQUE REFERENCES task_steps(step_id) ON DELETE RESTRICT,
  owner_id TEXT NOT NULL,
  grant_id TEXT NOT NULL,
  plan_revision INTEGER NOT NULL CHECK (plan_revision >= 0),
  task_state_revision INTEGER NOT NULL CHECK (task_state_revision >= 1),
  lease_generation INTEGER NOT NULL CHECK (lease_generation BETWEEN 1 AND 4294967295),
  policy_revision_id INTEGER NOT NULL REFERENCES policy_revisions(revision_id) ON DELETE RESTRICT,
  policy_rules_digest TEXT NOT NULL CHECK (length(policy_rules_digest)=71
                                            AND substr(policy_rules_digest,1,7)='sha256:'
                                            AND substr(policy_rules_digest,8) NOT GLOB '*[^0-9a-f]*'),
  generation_id INTEGER NOT NULL,
  descriptor_digest TEXT NOT NULL,
  capability_id TEXT NOT NULL CHECK (length(capability_id) BETWEEN 1 AND 99),
  capability_version TEXT NOT NULL CHECK (length(capability_version) BETWEEN 5 AND 128),
  provider_id TEXT NOT NULL CHECK (length(provider_id) BETWEEN 1 AND 64),
  overlay_revision INTEGER NOT NULL CHECK (overlay_revision >= 1),
  arguments_digest TEXT NOT NULL CHECK (length(arguments_digest)=71
                                         AND substr(arguments_digest,1,7)='sha256:'
                                         AND substr(arguments_digest,8) NOT GLOB '*[^0-9a-f]*'),
  request_digest TEXT NOT NULL CHECK (length(request_digest)=71
                                       AND substr(request_digest,1,7)='sha256:'
                                       AND substr(request_digest,8) NOT GLOB '*[^0-9a-f]*'),
  -- Event retention/Forget interaction is unsettled, so this is only a
  -- correlation identifier; it deliberately imposes no retention hold.
  event_id TEXT NOT NULL UNIQUE CHECK (length(event_id)=30
                                         AND substr(event_id,1,4)='evt_'
                                         AND substr(event_id,5,1)<='7'),
  committed_at_ms INTEGER NOT NULL CHECK (committed_at_ms BETWEEN -62167219200000 AND 253402300799999),
  -- Retention/Forget authority remains PROPOSED. This development candidate
  -- fails closed by refusing deletion while an unresolved intent is present.
  FOREIGN KEY(task_id, owner_id) REFERENCES task_owner_bindings(task_id, owner_id) ON DELETE RESTRICT,
  FOREIGN KEY(grant_id, step_id) REFERENCES approval_grant_uses(grant_id, step_id) ON DELETE RESTRICT,
  FOREIGN KEY(generation_id, descriptor_digest)
    REFERENCES capability_generation_members(generation_id, descriptor_digest) ON DELETE RESTRICT
) STRICT;

-- The intent is a frozen historical record. It has no lifecycle/status column:
-- state transitions have not been approved. ON DELETE RESTRICT above means a
-- Task with an intent cannot currently be deleted. This candidate-only
-- fail-closed behavior is not an accepted retention or Forget policy.
CREATE TRIGGER dispatch_intent_no_update
BEFORE UPDATE ON dispatch_intents
BEGIN SELECT RAISE(ABORT, 'P8 dispatch intent is immutable'); END;

-- A committed intent is the only candidate-local durable count of a P8
-- provider dispatch. Until Forget/retention is accepted, direct deletion
-- would erase both recovery evidence and that count, so this development
-- candidate refuses it. This is not a production retention guarantee.
CREATE TRIGGER dispatch_intent_no_delete
BEFORE DELETE ON dispatch_intents
BEGIN SELECT RAISE(ABORT, 'P8 dispatch intent deletion is not defined'); END;
