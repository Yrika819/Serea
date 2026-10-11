-- Migration 0005 is immutable. Its `enabled` column records whether the rule
-- participates in evaluation, but Policy Protocol §5.1 separately ratifies
-- `enabled` as an exact match dimension. Keep those two facts distinct.
ALTER TABLE policy_rules
  ADD COLUMN match_enabled INTEGER
    CHECK (match_enabled IS NULL OR match_enabled IN (0,1));

-- A revision is a set whose declared count and digest identify its complete
-- canonical snapshot. Refuse activation of a partial set, including direct SQL
-- activation attempts through the otherwise monotonic pointer.
CREATE TRIGGER policy_revision_activation_requires_complete_rules
BEFORE UPDATE OF activated_at_ms ON policy_revisions
WHEN OLD.activated_at_ms IS NULL
 AND NEW.activated_at_ms IS NOT NULL
 AND (SELECT count(*) FROM policy_rules WHERE revision_id=OLD.revision_id) <> OLD.rule_count
BEGIN SELECT RAISE(ABORT, 'policy revision rule set is incomplete'); END;

-- Migration 0005 prevented updates and post-activation deletes, but did not
-- prevent a rule being appended after activation. That would change policy
-- behaviour without changing the stored revision digest.
CREATE TRIGGER policy_rule_no_late_insert
BEFORE INSERT ON policy_rules
WHEN (SELECT activated_at_ms FROM policy_revisions WHERE revision_id=NEW.revision_id) IS NOT NULL
BEGIN SELECT RAISE(ABORT, 'activated policy revision is immutable'); END;
