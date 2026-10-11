-- P6B durable policy and approval foundation.
--
-- SQLite is the sole runtime authority for policy (Policy Protocol §7.1): host
-- configuration is a trusted import path and is never read at evaluation time.
-- Approval rows are task-scoped derived state and cascade with the Task; policy
-- revisions, rules and the activation pointer never cascade, matching Event
-- Protocol §8, which gives policy events one year and states that policy history
-- outliving its task is intentional.
--
-- Owner decision R2 (2026-10-10) adds the enumerated multi-action grant: one
-- grant binds 1 to 8 individually approved actions, each with its own step_id and
-- its own exact arguments_digest. `approval_request_actions` and
-- `approval_grant_members` are the durable authority binding of that exact Step
-- set for the request side and the grant side respectively. Both are immutable
-- after their parent row exists, and a grant member is always an action the human
-- actually saw, because it references `approval_request_actions`.
--
-- Deliberately absent, because they belong to P8: dispatch intents, RequestId
-- minting, provider attempts, duplicate suppression, repeated-action counters,
-- ActionResult, receipts and reconciliation state. Also absent: a
-- `policy_evaluations` table, which would duplicate what the immutable revision
-- and the request row already prove. No credential material is stored in any
-- column here, and `approval_requests.data_class` is mechanically restricted to
-- the ratified PUBLIC/PERSONAL ceiling.

-- ---------------------------------------------------------------------------
-- Policy revisions and rules
-- ---------------------------------------------------------------------------

CREATE TABLE policy_revisions (
  revision_id     INTEGER PRIMARY KEY AUTOINCREMENT CHECK (revision_id > 0),
  rules_digest    TEXT    NOT NULL CHECK (length(rules_digest) = 71
                                           AND substr(rules_digest,1,7) = 'sha256:'
                                           AND substr(rules_digest,8) NOT GLOB '*[^0-9a-f]*'),
  rule_count      INTEGER NOT NULL CHECK (rule_count BETWEEN 0 AND 512),
  created_at_ms   INTEGER NOT NULL CHECK (created_at_ms BETWEEN -62167219200000 AND 253402300799999),
  activated_at_ms INTEGER CHECK (activated_at_ms IS NULL OR activated_at_ms BETWEEN -62167219200000 AND 253402300799999),
  actor_class     TEXT    NOT NULL CHECK (actor_class IN ('HOST','USER')),
  actor_id        TEXT    NOT NULL CHECK (length(actor_id) BETWEEN 1 AND 128
                                           AND actor_id NOT GLOB '*[^A-Za-z0-9_.:@-]*'),
  reason_code     TEXT    NOT NULL CHECK (reason_code NOT GLOB '*[^A-Z0-9_]*'
                                           AND substr(reason_code,1,1) BETWEEN 'A' AND 'Z'),
  CHECK (activated_at_ms IS NULL OR activated_at_ms >= created_at_ms)
) STRICT;

CREATE INDEX policy_revisions_digest ON policy_revisions(rules_digest);

-- Activation is write-once: an activated revision is frozen in time and its
-- facts and rules can never change afterwards. Policy Protocol §7.2.
CREATE TRIGGER policy_revision_activation_once
BEFORE UPDATE OF activated_at_ms ON policy_revisions
WHEN OLD.activated_at_ms IS NOT NULL OR NEW.activated_at_ms IS NULL
BEGIN SELECT RAISE(ABORT, 'policy revision activation is immutable'); END;

CREATE TRIGGER policy_revision_facts_immutable_after_activation
BEFORE UPDATE OF rules_digest,rule_count,actor_class,actor_id,reason_code ON policy_revisions
WHEN OLD.activated_at_ms IS NOT NULL
BEGIN SELECT RAISE(ABORT, 'activated policy revision is immutable'); END;

CREATE TRIGGER policy_revision_activated_no_delete
BEFORE DELETE ON policy_revisions
WHEN OLD.activated_at_ms IS NOT NULL
BEGIN SELECT RAISE(ABORT, 'activated policy revision is immutable'); END;

CREATE TABLE policy_rules (
  revision_id        INTEGER NOT NULL REFERENCES policy_revisions(revision_id) ON DELETE RESTRICT,
  rule_id            TEXT    NOT NULL CHECK (length(rule_id) BETWEEN 1 AND 64
                                              AND rule_id NOT GLOB '*[^A-Za-z0-9_]*'
                                              AND substr(rule_id,1,1) NOT BETWEEN '0' AND '9'),
  priority           INTEGER NOT NULL CHECK (priority BETWEEN -2147483648 AND 2147483647),
  capability_id      TEXT    CHECK (capability_id IS NULL OR length(capability_id) BETWEEN 1 AND 99),
  risk_class         TEXT    CHECK (risk_class IS NULL OR risk_class IN
                                       ('OBSERVE','LOCAL_STATE','REVERSIBLE_WRITE','EXTERNAL_WRITE',
                                        'COMMUNICATION','ELEVATED_DEVICE','DESTRUCTIVE','CREDENTIAL')),
  side_effect_class  TEXT    CHECK (side_effect_class IS NULL OR side_effect_class IN
                                       ('NONE','LOCAL_STATE','DEVICE_STATE','EXTERNAL_WRITE',
                                        'COMMUNICATION','ELEVATED_DEVICE')),
  authorization      TEXT    CHECK (authorization IS NULL OR authorization IN
                                       ('NONE','DEVICE_USER','SCOPED_GRANT','CREDENTIAL_HANDOFF')),
  automation_context TEXT    CHECK (automation_context IS NULL OR automation_context IN
                                       ('INTERACTIVE','SCHEDULED','PROACTIVE','SYSTEM')),
  requested_by       TEXT    CHECK (requested_by IS NULL OR requested_by IN
                                       ('MODEL','USER','SCHEDULER','PROACTIVE_WATCHER','SYSTEM')),
  enabled            INTEGER NOT NULL CHECK (enabled IN (0,1)),
  decision           TEXT    NOT NULL CHECK (decision IN
                                       ('ALLOW','DENY','REQUIRE_APPROVAL','REQUIRE_HANDOFF')),
  reason_code        TEXT    NOT NULL CHECK (reason_code NOT GLOB '*[^A-Z0-9_]*'
                                              AND substr(reason_code,1,1) BETWEEN 'A' AND 'Z'),
  PRIMARY KEY (revision_id,rule_id),
  UNIQUE (revision_id,priority,rule_id)
) STRICT;

CREATE INDEX policy_rules_evaluation_order
  ON policy_rules(revision_id,priority DESC,rule_id ASC);

CREATE TRIGGER policy_rule_no_update
BEFORE UPDATE ON policy_rules
BEGIN SELECT RAISE(ABORT, 'policy rules of a revision are immutable'); END;

CREATE TRIGGER policy_rule_no_delete
BEFORE DELETE ON policy_rules
WHEN (SELECT activated_at_ms FROM policy_revisions WHERE revision_id=OLD.revision_id) IS NOT NULL
BEGIN SELECT RAISE(ABORT, 'policy rules of an activated revision are immutable'); END;

-- Singleton activation pointer. It only advances; it cannot be deleted; and it
-- cannot name a revision that has never been activated. These three triggers are
-- the "pointer only advances" guarantee, not the foreign key: without them a
-- direct SQL edit of active_revision_id succeeds.
CREATE TABLE policy_state (
  singleton          INTEGER PRIMARY KEY CHECK (singleton = 1),
  active_revision_id INTEGER REFERENCES policy_revisions(revision_id) ON DELETE RESTRICT
) STRICT;
INSERT INTO policy_state(singleton,active_revision_id) VALUES (1,NULL);

CREATE TRIGGER policy_state_no_delete
BEFORE DELETE ON policy_state
BEGIN SELECT RAISE(ABORT, 'policy singleton state cannot be deleted'); END;

CREATE TRIGGER policy_state_requires_activated_revision
BEFORE UPDATE OF active_revision_id ON policy_state
WHEN NEW.active_revision_id IS NOT NULL
  AND (SELECT activated_at_ms FROM policy_revisions WHERE revision_id=NEW.active_revision_id) IS NULL
BEGIN SELECT RAISE(ABORT, 'active policy revision must be activated'); END;

CREATE TRIGGER policy_state_advances_only
BEFORE UPDATE OF active_revision_id ON policy_state
WHEN (OLD.active_revision_id IS NOT NULL AND
      (NEW.active_revision_id IS NULL OR NEW.active_revision_id <= OLD.active_revision_id))
BEGIN SELECT RAISE(ABORT, 'active policy revision only advances'); END;

-- ---------------------------------------------------------------------------
-- Approval requests and the enumerated action set
-- ---------------------------------------------------------------------------

CREATE TABLE approval_requests (
  approval_id        TEXT    PRIMARY KEY CHECK (length(approval_id) = 30
                                                 AND substr(approval_id,1,4) = 'apr_'
                                                 AND substr(approval_id,5,1) <= '7'),
  task_id            TEXT    NOT NULL REFERENCES tasks(task_id) ON DELETE CASCADE,
  plan_revision      INTEGER NOT NULL CHECK (plan_revision >= 0),
  capability_id      TEXT    NOT NULL CHECK (length(capability_id) BETWEEN 1 AND 99),
  capability_version TEXT    NOT NULL CHECK (length(capability_version) BETWEEN 5 AND 128),
  generation_id      INTEGER NOT NULL REFERENCES capability_registry_generations(generation_id) ON DELETE RESTRICT,
  descriptor_digest  TEXT    NOT NULL REFERENCES capability_descriptor_revisions(descriptor_digest) ON DELETE RESTRICT,
  risk_class         TEXT    NOT NULL CHECK (risk_class IN
                                        ('OBSERVE','LOCAL_STATE','REVERSIBLE_WRITE','EXTERNAL_WRITE',
                                         'COMMUNICATION','ELEVATED_DEVICE','DESTRUCTIVE','CREDENTIAL')),
  side_effect_class  TEXT    NOT NULL CHECK (side_effect_class IN
                                        ('NONE','LOCAL_STATE','DEVICE_STATE','EXTERNAL_WRITE',
                                         'COMMUNICATION','ELEVATED_DEVICE')),
  authorization      TEXT    NOT NULL CHECK (authorization IN
                                        ('NONE','DEVICE_USER','SCOPED_GRANT','CREDENTIAL_HANDOFF')),
  -- The ratified persisted ceiling. PUBLIC/PERSONAL only; PRIVATE, SECRET and
  -- CREDENTIAL are refused mechanically, not by a convention.
  data_class         TEXT    NOT NULL CHECK (data_class IN ('PUBLIC','PERSONAL')),
  requested_by       TEXT    NOT NULL CHECK (requested_by IN
                                        ('MODEL','USER','SCHEDULER','PROACTIVE_WATCHER','SYSTEM')),
  automation_context TEXT    NOT NULL CHECK (automation_context IN
                                        ('INTERACTIVE','SCHEDULED','PROACTIVE','SYSTEM')),
  action_set_digest  TEXT    NOT NULL CHECK (length(action_set_digest) = 71
                                              AND substr(action_set_digest,1,7) = 'sha256:'
                                              AND substr(action_set_digest,8) NOT GLOB '*[^0-9a-f]*'),
  action_count       INTEGER NOT NULL CHECK (action_count BETWEEN 1 AND 8),
  max_uses          INTEGER NOT NULL CHECK (max_uses BETWEEN 1 AND 8),
  summary_kind       TEXT    NOT NULL CHECK (length(summary_kind) BETWEEN 1 AND 64
                                              AND summary_kind NOT GLOB '*[^A-Za-z0-9_.]*'),
  -- Host-authored summary. No length ceiling is published here on purpose: the
  -- ratified decision is that a summary has no byte bound, and a summary that
  -- exceeds max_event_payload_bytes fails only at event append.
  summary            TEXT    NOT NULL,
  raised_at_ms       INTEGER NOT NULL CHECK (raised_at_ms BETWEEN -62167219200000 AND 253402300799999),
  expires_at_ms      INTEGER NOT NULL CHECK (expires_at_ms BETWEEN -62167219200000 AND 253402300799999),
  status             TEXT    NOT NULL CHECK (status IN ('PENDING','APPROVED','DENIED','EXPIRED')),
  terminal_reason    TEXT    CHECK (terminal_reason IS NULL OR (terminal_reason NOT GLOB '*[^A-Z0-9_]*'
                                            AND substr(terminal_reason,1,1) BETWEEN 'A' AND 'Z')),
  CHECK (expires_at_ms > raised_at_ms),
  -- Owner decision R2 invariant 7: the request's use ceiling is the number of
  -- individually enumerated actions, so no unit can carry authority for more
  -- Steps than a human individually approved.
  CHECK (max_uses = action_count),
  CHECK (terminal_reason IS NULL OR status <> 'PENDING'),
  CHECK ((status IN ('DENIED','EXPIRED')) = (terminal_reason IS NOT NULL))
) STRICT;

-- A request REVOKED state does not exist: revocation is grant-only
-- (Approval Protocol §2.2.1, §3.4).

CREATE TABLE approval_request_actions (
  approval_id      TEXT    NOT NULL REFERENCES approval_requests(approval_id) ON DELETE CASCADE,
  position         INTEGER NOT NULL CHECK (position BETWEEN 0 AND 7),
  step_id          TEXT    NOT NULL REFERENCES task_steps(step_id) ON DELETE CASCADE,
  arguments_digest TEXT    NOT NULL CHECK (length(arguments_digest) = 71
                                            AND substr(arguments_digest,1,7) = 'sha256:'
                                            AND substr(arguments_digest,8) NOT GLOB '*[^0-9a-f]*'),
  scope            TEXT    NOT NULL CHECK (json_valid(scope) AND length(scope) BETWEEN 2 AND 4096),
  scope_digest     TEXT    NOT NULL CHECK (length(scope_digest) = 71
                                            AND substr(scope_digest,1,7) = 'sha256:'
                                            AND substr(scope_digest,8) NOT GLOB '*[^0-9a-f]*'),
  PRIMARY KEY (approval_id,position),
  UNIQUE (approval_id,step_id),
  -- The step must be a capability Step of the request's own task, pinned to the
  -- request's plan revision, capability, version, generation and descriptor
  -- revision, and its canonical arguments digest must match the durable Step.
  -- This is the composite task/step integrity the earlier conceptual DDL lacked.
  -- No wildcard authority, not even inside a scope value (DC-4).
  CHECK (scope NOT GLOB '*[*]*')
) STRICT;

CREATE INDEX approval_request_actions_step ON approval_request_actions(step_id);

CREATE TRIGGER approval_request_action_matches_requested_step
BEFORE INSERT ON approval_request_actions
WHEN (SELECT task_id FROM task_steps WHERE step_id=NEW.step_id) IS NOT
       (SELECT task_id FROM approval_requests WHERE approval_id=NEW.approval_id)
   OR (SELECT kind FROM task_steps WHERE step_id=NEW.step_id) IS NOT 'CAPABILITY'
   OR (SELECT plan_revision FROM task_steps WHERE step_id=NEW.step_id) IS NOT
       (SELECT plan_revision FROM approval_requests WHERE approval_id=NEW.approval_id)
   OR (SELECT capability_id FROM task_steps WHERE step_id=NEW.step_id) IS NOT
       (SELECT capability_id FROM approval_requests WHERE approval_id=NEW.approval_id)
   OR (SELECT capability_version FROM task_steps WHERE step_id=NEW.step_id) IS NOT
       (SELECT capability_version FROM approval_requests WHERE approval_id=NEW.approval_id)
   OR (SELECT input_digest FROM task_steps WHERE step_id=NEW.step_id) IS NOT NEW.arguments_digest
   OR NOT EXISTS (SELECT 1 FROM step_capability_bindings AS b
                   WHERE b.task_id=(SELECT task_id FROM approval_requests WHERE approval_id=NEW.approval_id)
                     AND b.step_id=NEW.step_id
                     AND b.generation_id=(SELECT generation_id FROM approval_requests WHERE approval_id=NEW.approval_id)
                     AND b.descriptor_digest=(SELECT descriptor_digest FROM approval_requests WHERE approval_id=NEW.approval_id)
                     AND b.capability_id=(SELECT capability_id FROM approval_requests WHERE approval_id=NEW.approval_id)
                     AND b.capability_version=(SELECT capability_version FROM approval_requests WHERE approval_id=NEW.approval_id))
BEGIN SELECT RAISE(ABORT, 'approval action facts do not match the requested task step'); END;

CREATE TRIGGER approval_request_action_within_count
BEFORE INSERT ON approval_request_actions
WHEN (SELECT count(*) FROM approval_request_actions WHERE approval_id=NEW.approval_id)
     >= (SELECT action_count FROM approval_requests WHERE approval_id=NEW.approval_id)
BEGIN SELECT RAISE(ABORT, 'approval request action set is bounded by action_count'); END;

CREATE TRIGGER approval_request_action_no_late_insert
BEFORE INSERT ON approval_request_actions
WHEN (SELECT status FROM approval_requests WHERE approval_id=NEW.approval_id) IS NOT 'PENDING'
BEGIN SELECT RAISE(ABORT, 'approval request action set is fixed before the response'); END;

CREATE TRIGGER approval_request_action_no_update
BEFORE UPDATE ON approval_request_actions
BEGIN SELECT RAISE(ABORT, 'approval request actions are immutable'); END;

-- Immutable while the parent request exists. Removing an already-deleted
-- request's actions is the cascade, not a mutation, so the trigger is scoped.
CREATE TRIGGER approval_request_action_no_delete
BEFORE DELETE ON approval_request_actions
WHEN EXISTS (SELECT 1 FROM approval_requests WHERE approval_id=OLD.approval_id)
BEGIN SELECT RAISE(ABORT, 'approval request actions are immutable'); END;

-- ---------------------------------------------------------------------------
-- Approval grants, the granted action subset, and uses
-- ---------------------------------------------------------------------------

CREATE TABLE approval_grants (
  grant_id          TEXT    PRIMARY KEY CHECK (length(grant_id) = 30
                                                 AND substr(grant_id,1,4) = 'grt_'
                                                 AND substr(grant_id,5,1) <= '7'),
  approval_id       TEXT    NOT NULL REFERENCES approval_requests(approval_id) ON DELETE CASCADE,
  task_id           TEXT    NOT NULL REFERENCES tasks(task_id) ON DELETE CASCADE,
  plan_revision     INTEGER NOT NULL CHECK (plan_revision >= 0),
  capability_id     TEXT    NOT NULL CHECK (length(capability_id) BETWEEN 1 AND 99),
  capability_version TEXT   NOT NULL CHECK (length(capability_version) BETWEEN 5 AND 128),
  generation_id     INTEGER NOT NULL REFERENCES capability_registry_generations(generation_id) ON DELETE RESTRICT,
  descriptor_digest TEXT    NOT NULL REFERENCES capability_descriptor_revisions(descriptor_digest) ON DELETE RESTRICT,
  action_set_digest TEXT    NOT NULL CHECK (length(action_set_digest) = 71
                                             AND substr(action_set_digest,1,7) = 'sha256:'
                                             AND substr(action_set_digest,8) NOT GLOB '*[^0-9a-f]*'),
  action_count      INTEGER NOT NULL CHECK (action_count BETWEEN 1 AND 8),
  max_uses          INTEGER NOT NULL CHECK (max_uses BETWEEN 1 AND 8),
  uses_remaining    INTEGER NOT NULL CHECK (uses_remaining BETWEEN 0 AND 8),
  granted_at_ms     INTEGER NOT NULL CHECK (granted_at_ms BETWEEN -62167219200000 AND 253402300799999),
  expires_at_ms     INTEGER NOT NULL CHECK (expires_at_ms BETWEEN -62167219200000 AND 253402300799999),
  granted_by        TEXT    NOT NULL CHECK (granted_by IN ('USER','ADMIN')),
  actor_device_id   TEXT    CHECK (actor_device_id IS NULL OR (length(actor_device_id) = 30
                              AND substr(actor_device_id,1,4) = 'dev_')),
  actor_session_id  TEXT    CHECK (actor_session_id IS NULL OR (length(actor_session_id) = 30
                               AND substr(actor_session_id,1,4) = 'ses_')),
  auth_strength     TEXT    NOT NULL CHECK (auth_strength IN ('SESSION','ELEVATED_CONFIRMED')),
  -- Digest handle only. No biometric material, ever.
  elevated_ref      TEXT    CHECK (elevated_ref IS NULL OR (length(elevated_ref) = 71
                              AND substr(elevated_ref,1,7) = 'sha256:'
                              AND substr(elevated_ref,8) NOT GLOB '*[^0-9a-f]*')),
  status            TEXT    NOT NULL CHECK (status IN ('ACTIVE','EXHAUSTED','EXPIRED','REVOKED')),
  UNIQUE (approval_id,task_id),
  -- A grant is one capability, one version, one generation, one descriptor
  -- revision, one plan revision, one task and one expiry. Ambiguous authority
  -- merging is refused by the identity trigger below, not by a convention.
  CHECK (uses_remaining <= max_uses),
  -- EXHAUSTED means exactly "every granted step has consumed". Writing it by
  -- hand while a use is still available would forge a state the spend trigger
  -- produces.
  CHECK (status <> 'EXHAUSTED' OR uses_remaining = 0),
  -- Owner decision R2 invariant 7: max_uses is at most the number of actually
  -- approved individual Steps.
  CHECK (max_uses = action_count),
  CHECK (expires_at_ms > granted_at_ms)
) STRICT;

CREATE INDEX approval_grants_task ON approval_grants(task_id,status);

CREATE TRIGGER approval_grant_matches_request
BEFORE INSERT ON approval_grants
WHEN (SELECT task_id FROM approval_requests WHERE approval_id=NEW.approval_id) IS NOT NEW.task_id
   OR (SELECT plan_revision FROM approval_requests WHERE approval_id=NEW.approval_id) IS NOT NEW.plan_revision
   OR (SELECT capability_id FROM approval_requests WHERE approval_id=NEW.approval_id) IS NOT NEW.capability_id
   OR (SELECT capability_version FROM approval_requests WHERE approval_id=NEW.approval_id) IS NOT NEW.capability_version
   OR (SELECT generation_id FROM approval_requests WHERE approval_id=NEW.approval_id) IS NOT NEW.generation_id
   OR (SELECT descriptor_digest FROM approval_requests WHERE approval_id=NEW.approval_id) IS NOT NEW.descriptor_digest
   OR (SELECT action_set_digest FROM approval_requests WHERE approval_id=NEW.approval_id) IS NOT NEW.action_set_digest
   OR (SELECT status FROM approval_requests WHERE approval_id=NEW.approval_id) IS NOT 'APPROVED'
BEGIN SELECT RAISE(ABORT, 'grant facts do not match its approved request'); END;

CREATE TABLE approval_grant_members (
  grant_id         TEXT    NOT NULL REFERENCES approval_grants(grant_id) ON DELETE CASCADE,
  position         INTEGER NOT NULL CHECK (position BETWEEN 0 AND 7),
  approval_id      TEXT    NOT NULL,
  step_id          TEXT    NOT NULL REFERENCES task_steps(step_id) ON DELETE CASCADE,
  arguments_digest TEXT    NOT NULL CHECK (length(arguments_digest) = 71
                                            AND substr(arguments_digest,1,7) = 'sha256:'
                                            AND substr(arguments_digest,8) NOT GLOB '*[^0-9a-f]*'),
  scope_digest     TEXT    NOT NULL CHECK (length(scope_digest) = 71
                                            AND substr(scope_digest,1,7) = 'sha256:'
                                            AND substr(scope_digest,8) NOT GLOB '*[^0-9a-f]*'),
  PRIMARY KEY (grant_id,position),
  UNIQUE (grant_id,step_id),
  -- A granted member is always an action the human actually saw: it must be a
  -- row of the originating request's action set.
  FOREIGN KEY (approval_id,step_id)
    REFERENCES approval_request_actions(approval_id,step_id) ON DELETE CASCADE
) STRICT;

CREATE INDEX approval_grant_members_step ON approval_grant_members(step_id);

CREATE TRIGGER approval_grant_member_is_an_approved_action
BEFORE INSERT ON approval_grant_members
WHEN (SELECT approval_id FROM approval_grants WHERE grant_id=NEW.grant_id) IS NOT NEW.approval_id
   OR NOT EXISTS (SELECT 1 FROM approval_request_actions AS a
                   WHERE a.approval_id=NEW.approval_id AND a.step_id=NEW.step_id)
   OR (SELECT arguments_digest FROM approval_request_actions
        WHERE approval_id=NEW.approval_id AND step_id=NEW.step_id) IS NOT NEW.arguments_digest
   OR (SELECT scope_digest FROM approval_request_actions
        WHERE approval_id=NEW.approval_id AND step_id=NEW.step_id) IS NOT NEW.scope_digest
BEGIN SELECT RAISE(ABORT, 'granted step must be an action the human actually approved'); END;

CREATE TRIGGER approval_grant_member_belongs_to_grant_task
BEFORE INSERT ON approval_grant_members
WHEN (SELECT task_id FROM approval_grants WHERE grant_id=NEW.grant_id) IS NOT
     (SELECT task_id FROM task_steps WHERE step_id=NEW.step_id)
BEGIN SELECT RAISE(ABORT, 'granted step belongs to another task'); END;

CREATE TRIGGER approval_grant_member_within_count
BEFORE INSERT ON approval_grant_members
WHEN (SELECT count(*) FROM approval_grant_members WHERE grant_id=NEW.grant_id)
     >= (SELECT action_count FROM approval_grants WHERE grant_id=NEW.grant_id)
BEGIN SELECT RAISE(ABORT, 'grant action set is bounded by action_count'); END;

CREATE TRIGGER approval_grant_member_no_update
BEFORE UPDATE ON approval_grant_members
BEGIN SELECT RAISE(ABORT, 'granted steps are immutable'); END;

-- Immutable while the parent grant exists; the task/approval/grant cascade is
-- not a mutation.
CREATE TRIGGER approval_grant_member_no_delete
BEFORE DELETE ON approval_grant_members
WHEN EXISTS (SELECT 1 FROM approval_grants WHERE grant_id=OLD.grant_id)
BEGIN SELECT RAISE(ABORT, 'granted steps are immutable'); END;

CREATE TABLE approval_grant_uses (
  grant_id       TEXT    NOT NULL REFERENCES approval_grants(grant_id) ON DELETE CASCADE,
  step_id        TEXT    NOT NULL REFERENCES task_steps(step_id) ON DELETE CASCADE,
  task_id        TEXT    NOT NULL REFERENCES tasks(task_id) ON DELETE CASCADE,
  consumed_at_ms INTEGER NOT NULL CHECK (consumed_at_ms BETWEEN -62167219200000 AND 253402300799999),
  -- At most one use per (grant, step), and at most one grant per step for the
  -- lifetime of the host. DC-1 keeps both.
  PRIMARY KEY (grant_id,step_id),
  UNIQUE (step_id)
) STRICT;

CREATE INDEX approval_grant_uses_task ON approval_grant_uses(task_id);

-- Owner decision R2: membership, never scope or digest equality, is what
-- authorizes a consuming Step. An unlisted Step is refused even if its arguments
-- equal an approved digest or it sits inside an approved scope.
CREATE TRIGGER approval_grant_use_must_be_a_granted_step
BEFORE INSERT ON approval_grant_uses
WHEN NOT EXISTS (SELECT 1 FROM approval_grant_members AS m
                  WHERE m.grant_id=NEW.grant_id AND m.step_id=NEW.step_id)
BEGIN SELECT RAISE(ABORT, 'step is not a granted step of this grant'); END;

CREATE TRIGGER approval_grant_use_matches_grant
BEFORE INSERT ON approval_grant_uses
WHEN (SELECT task_id FROM approval_grants WHERE grant_id=NEW.grant_id) IS NOT NEW.task_id
   OR (SELECT task_id FROM task_steps WHERE step_id=NEW.step_id) IS NOT NEW.task_id
BEGIN SELECT RAISE(ABORT, 'grant use does not match grant and step task'); END;

CREATE TRIGGER approval_grant_use_requires_consumable_grant
BEFORE INSERT ON approval_grant_uses
WHEN (SELECT status FROM approval_grants WHERE grant_id=NEW.grant_id) IS NOT 'ACTIVE'
   OR (SELECT uses_remaining FROM approval_grants WHERE grant_id=NEW.grant_id) <= 0
BEGIN SELECT RAISE(ABORT, 'grant is not consumable'); END;

-- Spending a use and recording it are one statement pair inside one SQLite
-- statement sequence, so a direct SQL manipulation cannot create a use row
-- without decrementing the grant. EXPIRED and REVOKED are terminal and are set
-- by the Rust layer, never by this trigger.
CREATE TRIGGER approval_grant_use_spends_one_use
AFTER INSERT ON approval_grant_uses
BEGIN
  UPDATE approval_grants
     SET uses_remaining = uses_remaining - 1,
         status = CASE WHEN uses_remaining - 1 = 0 THEN 'EXHAUSTED' ELSE 'ACTIVE' END
   WHERE grant_id = NEW.grant_id;
END;
