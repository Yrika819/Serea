-- P6D adds grant expiry and authenticated grant revocation as approval lifecycle outcomes.
-- Preserve existing wake rows while extending only the outcome enum; previous
-- migration bytes remain immutable.
ALTER TABLE approval_lifecycle_wakes RENAME TO approval_lifecycle_wakes_v7;

CREATE TABLE approval_lifecycle_wakes (
  source_event_id TEXT PRIMARY KEY CHECK (length(source_event_id)=30
                                           AND substr(source_event_id,1,4)='evt_'),
  source_seq INTEGER NOT NULL UNIQUE CHECK (source_seq BETWEEN 1 AND 9223372036854775807),
  approval_id TEXT NOT NULL CHECK (length(approval_id)=30
                                   AND substr(approval_id,1,4)='apr_'),
  task_id TEXT NOT NULL REFERENCES tasks(task_id) ON DELETE CASCADE
                  CHECK (length(task_id)=30 AND substr(task_id,1,4)='tsk_'),
  step_id TEXT NOT NULL CHECK (length(step_id)=30 AND substr(step_id,1,4)='stp_'),
  outcome_kind TEXT NOT NULL CHECK (outcome_kind IN ('GRANTED','DENIED','EXPIRED','EXPIRED_UNUSED','REVOKED')),
  created_at_ms INTEGER NOT NULL CHECK (created_at_ms BETWEEN -62167219200000 AND 253402300799999)
) STRICT;

INSERT INTO approval_lifecycle_wakes(
  source_event_id,source_seq,approval_id,task_id,step_id,outcome_kind,created_at_ms
)
SELECT source_event_id,source_seq,approval_id,task_id,step_id,outcome_kind,created_at_ms
FROM approval_lifecycle_wakes_v7;

DROP TABLE approval_lifecycle_wakes_v7;

CREATE INDEX approval_lifecycle_wakes_pending
  ON approval_lifecycle_wakes(created_at_ms, source_seq, source_event_id);
CREATE INDEX approval_lifecycle_wakes_task
  ON approval_lifecycle_wakes(task_id);
