-- P6B forward-only correction: a row-count ceiling is not a complete-set gate.
-- Migration 0005 is immutable. These triggers make a request approval depend on
-- its declared action count and make a grant unusable until its exact granted
-- subset has been sealed. The host writer validates action_set_digest before
-- sealing; SQL enforces membership, ownership, size and immutability.

-- A request can only start pending. Its immutable prompt facts and enumerated
-- actions are frozen once inserted; the response changes only status/reason.
CREATE TRIGGER approval_request_starts_pending
BEFORE INSERT ON approval_requests
WHEN NEW.status IS NOT 'PENDING'
BEGIN SELECT RAISE(ABORT, 'approval request must start pending'); END;

CREATE TRIGGER approval_request_transition_requires_complete_set
BEFORE UPDATE OF status ON approval_requests
WHEN (NEW.status IS NOT OLD.status AND NOT
       (OLD.status='PENDING' AND NEW.status IN ('APPROVED','DENIED','EXPIRED')))
   OR (OLD.status='PENDING' AND NEW.status='APPROVED'
       AND (SELECT count(*) FROM approval_request_actions
            WHERE approval_id=OLD.approval_id) <> OLD.action_count)
BEGIN SELECT RAISE(ABORT, 'approval response requires a complete pending action set'); END;

CREATE TRIGGER approval_request_authority_facts_immutable
BEFORE UPDATE OF task_id,plan_revision,capability_id,capability_version,generation_id,
                 descriptor_digest,risk_class,side_effect_class,authorization,data_class,
                 requested_by,automation_context,action_set_digest,action_count,max_uses,
                 summary_kind,summary,raised_at_ms,expires_at_ms
ON approval_requests
BEGIN SELECT RAISE(ABORT, 'approval request facts are immutable'); END;

CREATE TRIGGER approval_request_no_delete_while_task_exists
BEFORE DELETE ON approval_requests
WHEN EXISTS (SELECT 1 FROM tasks WHERE task_id=OLD.task_id)
BEGIN SELECT RAISE(ABORT, 'approval request history is immutable while its task exists'); END;

-- R2 permits an approved subset. The grant commits that subset's independent
-- digest, so it must not be forced equal to the full request-set digest here.
-- The trusted policy writer recomputes the subset digest and enforces exact
-- member count before it seals the grant.
DROP TRIGGER approval_grant_matches_request;
CREATE TRIGGER approval_grant_matches_request
BEFORE INSERT ON approval_grants
WHEN (SELECT task_id FROM approval_requests WHERE approval_id=NEW.approval_id) IS NOT NEW.task_id
   OR (SELECT plan_revision FROM approval_requests WHERE approval_id=NEW.approval_id) IS NOT NEW.plan_revision
   OR (SELECT capability_id FROM approval_requests WHERE approval_id=NEW.approval_id) IS NOT NEW.capability_id
   OR (SELECT capability_version FROM approval_requests WHERE approval_id=NEW.approval_id) IS NOT NEW.capability_version
   OR (SELECT generation_id FROM approval_requests WHERE approval_id=NEW.approval_id) IS NOT NEW.generation_id
   OR (SELECT descriptor_digest FROM approval_requests WHERE approval_id=NEW.approval_id) IS NOT NEW.descriptor_digest
   OR NEW.action_count > (SELECT action_count FROM approval_requests WHERE approval_id=NEW.approval_id)
   OR (SELECT status FROM approval_requests WHERE approval_id=NEW.approval_id) IS NOT 'APPROVED'
BEGIN SELECT RAISE(ABORT, 'grant facts do not match its approved request'); END;

CREATE TRIGGER approval_grant_authority_facts_immutable
BEFORE UPDATE OF approval_id,task_id,plan_revision,capability_id,capability_version,
                 generation_id,descriptor_digest,action_set_digest,action_count,max_uses,
                 granted_at_ms,expires_at_ms,granted_by,actor_device_id,actor_session_id,
                 auth_strength,elevated_ref
ON approval_grants
BEGIN SELECT RAISE(ABORT, 'approval grant authority facts are immutable'); END;

CREATE TRIGGER approval_grant_no_delete_while_task_exists
BEFORE DELETE ON approval_grants
WHEN EXISTS (SELECT 1 FROM tasks WHERE task_id=OLD.task_id)
BEGIN SELECT RAISE(ABORT, 'approval grant history is immutable while its task exists'); END;

CREATE TRIGGER approval_task_step_facts_immutable_while_approval_exists
BEFORE UPDATE OF task_id,plan_revision,capability_id,capability_version,input_digest ON task_steps
WHEN EXISTS (SELECT 1 FROM approval_request_actions WHERE step_id=OLD.step_id)
  OR EXISTS (SELECT 1 FROM approval_grant_members WHERE step_id=OLD.step_id)
  OR EXISTS (SELECT 1 FROM approval_grant_uses WHERE step_id=OLD.step_id)
BEGIN SELECT RAISE(ABORT, 'approved Step authority facts are immutable'); END;

CREATE TRIGGER approval_task_step_no_delete_while_approval_exists
BEFORE DELETE ON task_steps
WHEN EXISTS (SELECT 1 FROM tasks WHERE task_id=OLD.task_id)
 AND (EXISTS (SELECT 1 FROM approval_request_actions WHERE step_id=OLD.step_id)
   OR EXISTS (SELECT 1 FROM approval_grant_members WHERE step_id=OLD.step_id)
   OR EXISTS (SELECT 1 FROM approval_grant_uses WHERE step_id=OLD.step_id))
BEGIN SELECT RAISE(ABORT, 'approved Step cannot be deleted while its task exists'); END;

CREATE TABLE approval_grant_seals (
  grant_id      TEXT PRIMARY KEY REFERENCES approval_grants(grant_id) ON DELETE CASCADE,
  sealed_at_ms  INTEGER NOT NULL CHECK (sealed_at_ms BETWEEN -62167219200000 AND 253402300799999)
) STRICT;

CREATE TRIGGER approval_grant_seal_requires_complete_members
BEFORE INSERT ON approval_grant_seals
WHEN (SELECT status FROM approval_grants WHERE grant_id=NEW.grant_id) IS NOT 'ACTIVE'
   OR (SELECT count(*) FROM approval_grant_members WHERE grant_id=NEW.grant_id)
      <> (SELECT action_count FROM approval_grants WHERE grant_id=NEW.grant_id)
   OR (SELECT max(position) FROM approval_grant_members WHERE grant_id=NEW.grant_id)
      <> (SELECT action_count - 1 FROM approval_grants WHERE grant_id=NEW.grant_id)
BEGIN SELECT RAISE(ABORT, 'grant seal requires the complete approved member set'); END;

CREATE TRIGGER approval_grant_seal_no_update
BEFORE UPDATE ON approval_grant_seals
BEGIN SELECT RAISE(ABORT, 'grant seal is immutable'); END;

CREATE TRIGGER approval_grant_seal_no_delete
BEFORE DELETE ON approval_grant_seals
WHEN EXISTS (SELECT 1 FROM approval_grants WHERE grant_id=OLD.grant_id)
 AND EXISTS (
   SELECT 1 FROM approval_grants AS g
   JOIN tasks AS t ON t.task_id=g.task_id
   WHERE g.grant_id=OLD.grant_id
 )
BEGIN SELECT RAISE(ABORT, 'grant seal is immutable'); END;

CREATE TRIGGER approval_grant_member_no_insert_after_seal
BEFORE INSERT ON approval_grant_members
WHEN EXISTS (SELECT 1 FROM approval_grant_seals WHERE grant_id=NEW.grant_id)
BEGIN SELECT RAISE(ABORT, 'sealed grant membership is immutable'); END;

CREATE TRIGGER approval_grant_use_requires_sealed_complete_set
BEFORE INSERT ON approval_grant_uses
WHEN NOT EXISTS (SELECT 1 FROM approval_grant_seals AS s
                  WHERE s.grant_id=NEW.grant_id)
   OR (SELECT count(*) FROM approval_grant_members WHERE grant_id=NEW.grant_id)
      <> (SELECT action_count FROM approval_grants WHERE grant_id=NEW.grant_id)
BEGIN SELECT RAISE(ABORT, 'grant use requires a sealed complete member set'); END;

CREATE TRIGGER approval_grant_use_matches_current_step_authority
BEFORE INSERT ON approval_grant_uses
WHEN (SELECT task_id FROM task_steps WHERE step_id=NEW.step_id) IS NOT NEW.task_id
   OR (SELECT task_id FROM approval_grants WHERE grant_id=NEW.grant_id) IS NOT NEW.task_id
   OR (SELECT plan_revision FROM task_steps WHERE step_id=NEW.step_id) IS NOT
      (SELECT plan_revision FROM approval_grants WHERE grant_id=NEW.grant_id)
   OR (SELECT capability_id FROM task_steps WHERE step_id=NEW.step_id) IS NOT
      (SELECT capability_id FROM approval_grants WHERE grant_id=NEW.grant_id)
   OR (SELECT capability_version FROM task_steps WHERE step_id=NEW.step_id) IS NOT
      (SELECT capability_version FROM approval_grants WHERE grant_id=NEW.grant_id)
   OR (SELECT input_digest FROM task_steps WHERE step_id=NEW.step_id) IS NOT
      (SELECT arguments_digest FROM approval_grant_members
        WHERE grant_id=NEW.grant_id AND step_id=NEW.step_id)
   OR NOT EXISTS (
      SELECT 1 FROM step_capability_bindings AS b
      JOIN approval_grants AS g ON g.grant_id=NEW.grant_id
      WHERE b.task_id=NEW.task_id AND b.step_id=NEW.step_id
        AND b.capability_id=g.capability_id
        AND b.capability_version=g.capability_version
        AND b.generation_id=g.generation_id
        AND b.descriptor_digest=g.descriptor_digest
   )
BEGIN SELECT RAISE(ABORT, 'grant no longer matches the current Step authority'); END;

-- The global UNIQUE(step_id) is durable only if use rows cannot be edited or
-- removed while their grant exists. Parent/task cascades remain permitted.
CREATE TRIGGER approval_grant_use_no_update
BEFORE UPDATE ON approval_grant_uses
BEGIN SELECT RAISE(ABORT, 'grant use accounting is immutable'); END;

CREATE TRIGGER approval_grant_use_no_delete
BEFORE DELETE ON approval_grant_uses
WHEN EXISTS (SELECT 1 FROM approval_grants WHERE grant_id=OLD.grant_id)
 AND EXISTS (
   SELECT 1 FROM tasks AS t WHERE t.task_id=OLD.task_id
 )
BEGIN SELECT RAISE(ABORT, 'grant use accounting is immutable'); END;

-- A use insert is the only operation that may decrement the counter. The count
-- includes the new use row in the spend trigger's AFTER INSERT statement.
CREATE TRIGGER approval_grant_counter_only_follows_use
BEFORE UPDATE OF uses_remaining ON approval_grants
WHEN NEW.uses_remaining IS NOT OLD.uses_remaining
 AND (NOT EXISTS (SELECT 1 FROM approval_grant_seals WHERE grant_id=OLD.grant_id)
      OR NEW.uses_remaining <> OLD.uses_remaining - 1
      OR NEW.uses_remaining <> OLD.max_uses -
           (SELECT count(*) FROM approval_grant_uses WHERE grant_id=OLD.grant_id)
      OR NEW.status IS NOT CASE WHEN NEW.uses_remaining=0 THEN 'EXHAUSTED' ELSE 'ACTIVE' END)
BEGIN SELECT RAISE(ABORT, 'grant use counter must match durable consumption'); END;

CREATE TRIGGER approval_grant_status_is_monotonic
BEFORE UPDATE OF status ON approval_grants
WHEN NEW.status IS NOT OLD.status
 AND NOT (OLD.status='ACTIVE' AND NEW.status IN ('EXPIRED','REVOKED'))
 AND NOT (OLD.status='ACTIVE' AND NEW.status='EXHAUSTED'
          AND NEW.uses_remaining=0
          AND (SELECT count(*) FROM approval_grant_uses WHERE grant_id=OLD.grant_id)=OLD.max_uses)
BEGIN SELECT RAISE(ABORT, 'grant terminal state cannot be forged or reversed'); END;
