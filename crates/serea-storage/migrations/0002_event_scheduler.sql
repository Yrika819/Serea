-- P3 durable Event Bus and Scheduler foundation.
-- Applied inside the migration runner's single BEGIN IMMEDIATE transaction.
-- Migration 0001 is immutable. No native-endian or host-dependent encoding is
-- used in durable fields.

-- P3 task-state fencing is owned by TaskEngine/Storage. The trigger increments
-- this revision for every durable state edge without changing Task wire shape.
ALTER TABLE tasks ADD COLUMN state_revision INTEGER NOT NULL DEFAULT 1
  CHECK (state_revision BETWEEN 1 AND 9223372036854775807);

CREATE TRIGGER tasks_state_revision_advance
AFTER UPDATE OF state,blocked_reason ON tasks
WHEN NEW.state IS NOT OLD.state OR NEW.blocked_reason IS NOT OLD.blocked_reason
BEGIN
  UPDATE tasks SET state_revision=OLD.state_revision+1
  WHERE task_id=NEW.task_id;
END;

CREATE TABLE event_store_state (
  singleton             INTEGER PRIMARY KEY CHECK (singleton = 1),
  last_allocated_seq    INTEGER NOT NULL DEFAULT 0
                        CHECK (last_allocated_seq BETWEEN 0 AND 9223372036854775807),
  expired_prefix_through INTEGER NOT NULL DEFAULT 0
                        CHECK (expired_prefix_through BETWEEN 0 AND last_allocated_seq),
  retained_count        INTEGER NOT NULL DEFAULT 0
                        CHECK (retained_count BETWEEN 0 AND 1000000),
  event_store_bytes     INTEGER NOT NULL DEFAULT 0
                        CHECK (event_store_bytes BETWEEN 0 AND 536870912)
) STRICT;

INSERT INTO event_store_state(singleton) VALUES (1);

-- One row per sequence whose complete content is currently retained. An
-- intentionally expired sequence is represented by event_expired_ranges;
-- old contiguous expired ranges are folded into expired_prefix_through.
-- Therefore this ledger cannot grow forever one row per allocated sequence.
-- It contains the sequence only: no event, task, actor, device, schedule,
-- digest, payload, classification, or other identifying information.
CREATE TABLE event_sequence_ledger (
  seq INTEGER PRIMARY KEY CHECK (seq BETWEEN 1 AND 9223372036854775807)
) STRICT;

CREATE TABLE event_content (
  seq             INTEGER PRIMARY KEY REFERENCES event_sequence_ledger(seq) ON DELETE RESTRICT,
  message_id      TEXT    NOT NULL UNIQUE CHECK (length(message_id) = 30
                                                   AND substr(message_id,1,4) = 'evt_'
                                                   AND substr(message_id,5,1) <= '7'),
  kind            TEXT    NOT NULL CHECK (length(kind) BETWEEN 1 AND 64
                                          AND kind NOT GLOB '*[^A-Z0-9_]*'
                                          AND substr(kind,1,1) BETWEEN 'A' AND 'Z'),
  occurred_at_ms  INTEGER NOT NULL CHECK (occurred_at_ms BETWEEN -62167219200000 AND 253402300799999),
  data_class_rank INTEGER NOT NULL CHECK (data_class_rank BETWEEN 0 AND 4),
  event_json      TEXT    NOT NULL CHECK (json_valid(event_json)
                                          AND json_type(event_json) = 'object'),
  payload_bytes   INTEGER NOT NULL CHECK (payload_bytes BETWEEN 0 AND 32768),
  content_bytes   INTEGER NOT NULL CHECK (content_bytes = length(CAST(event_json AS BLOB))),
  retention_at_ms INTEGER CHECK (retention_at_ms BETWEEN -62167219200000 AND 253402300799999)
) STRICT;

CREATE TRIGGER event_content_no_update
BEFORE UPDATE ON event_content
BEGIN
  SELECT RAISE(ABORT, 'retained event content is immutable');
END;

CREATE TABLE event_expired_ranges (
  first_seq INTEGER PRIMARY KEY CHECK (first_seq BETWEEN 1 AND 9223372036854775807),
  last_seq  INTEGER NOT NULL CHECK (last_seq BETWEEN first_seq AND 9223372036854775807)
) STRICT;

CREATE TRIGGER event_expired_ranges_no_overlap
BEFORE INSERT ON event_expired_ranges
WHEN EXISTS (
  SELECT 1 FROM event_expired_ranges
  WHERE NEW.first_seq <= last_seq AND first_seq <= NEW.last_seq
)
BEGIN
  SELECT RAISE(ABORT, 'intentional event expiry ranges overlap');
END;

CREATE INDEX event_content_kind_seq ON event_content(kind, seq);
CREATE INDEX event_content_retention ON event_content(retention_at_ms, seq)
  WHERE retention_at_ms IS NOT NULL;
CREATE INDEX event_expired_ranges_last ON event_expired_ranges(last_seq);

CREATE TABLE schedules (
  schedule_id              TEXT    PRIMARY KEY CHECK (length(schedule_id) = 30
                                                        AND substr(schedule_id,1,4) = 'sch_'
                                                        AND substr(schedule_id,5,1) <= '7'),
  owner_kind               TEXT    NOT NULL CHECK (owner_kind IN ('DEVICE','HOST')),
  owner_id                 TEXT    NOT NULL CHECK (length(owner_id) BETWEEN 1 AND 128),
  state                    TEXT    NOT NULL CHECK (state IN ('ACTIVE','PAUSED','CANCELLED')),
  revision                 INTEGER NOT NULL CHECK (revision BETWEEN 1 AND 4294967295),
  trigger_kind             TEXT    NOT NULL CHECK (trigger_kind IN
                             ('CALENDAR','HOST_EVENT','DEVICE_SESSION_ESTABLISHED','APPROVAL_EVENT')),
  recurrence_json          TEXT    CHECK (recurrence_json IS NULL OR
                             (json_valid(recurrence_json) AND json_type(recurrence_json) = 'object')),
  event_predicate_json     TEXT    CHECK (event_predicate_json IS NULL OR
                             (json_valid(event_predicate_json) AND json_type(event_predicate_json) = 'object')),
  event_predicate_after_seq INTEGER CHECK (event_predicate_after_seq BETWEEN 0 AND 9223372036854775807),
  template_digest          TEXT,
  template_data_class_rank INTEGER,
  policy_class_rank        INTEGER NOT NULL CHECK (policy_class_rank BETWEEN 0 AND 7),
  approval_policy_json     TEXT    NOT NULL CHECK (json_valid(approval_policy_json)
                                                   AND json_type(approval_policy_json) = 'object'),
  timezone                 TEXT,
  recurrence_evaluator     TEXT,
  tzdb_version             TEXT,
  next_due_at_ms           INTEGER CHECK (next_due_at_ms BETWEEN -62167219200000 AND 253402300799999),
  next_local_label         TEXT,
  missed_policy            TEXT    NOT NULL CHECK (missed_policy IN ('SKIP','RUN_ONCE','RUN_EACH')),
  last_processed_occurrence TEXT,
  created_at_ms            INTEGER NOT NULL CHECK (created_at_ms BETWEEN -62167219200000 AND 253402300799999),
  updated_at_ms            INTEGER NOT NULL CHECK (updated_at_ms BETWEEN -62167219200000 AND 253402300799999),
  cancelled_at_ms          INTEGER CHECK (cancelled_at_ms BETWEEN -62167219200000 AND 253402300799999),
  FOREIGN KEY (template_digest, template_data_class_rank)
    REFERENCES blobs(digest, data_class_rank) ON DELETE RESTRICT,
  CHECK (updated_at_ms >= created_at_ms),
  CHECK ((trigger_kind = 'CALENDAR') = (recurrence_json IS NOT NULL)),
  CHECK ((trigger_kind = 'HOST_EVENT') = (event_predicate_json IS NOT NULL)),
  CHECK ((trigger_kind = 'HOST_EVENT') = (event_predicate_after_seq IS NOT NULL)),
  CHECK ((template_digest IS NULL) = (template_data_class_rank IS NULL)),
  CHECK (template_data_class_rank IS NULL OR template_data_class_rank BETWEEN 0 AND 2),
  CHECK ((next_due_at_ms IS NULL) = (next_local_label IS NULL)),
  CHECK (trigger_kind = 'CALENDAR' OR
         (timezone IS NULL AND recurrence_evaluator IS NULL AND tzdb_version IS NULL
          AND next_due_at_ms IS NULL AND next_local_label IS NULL)),
  CHECK (trigger_kind <> 'CALENDAR' OR timezone IS NOT NULL),
  CHECK ((state = 'CANCELLED') = (cancelled_at_ms IS NOT NULL))
) STRICT;

CREATE INDEX schedules_active_due ON schedules(next_due_at_ms, schedule_id)
  WHERE state = 'ACTIVE' AND trigger_kind = 'CALENDAR';
CREATE INDEX schedules_active_trigger ON schedules(trigger_kind, schedule_id)
  WHERE state = 'ACTIVE';

CREATE TABLE schedule_occurrences (
  schedule_id          TEXT    NOT NULL REFERENCES schedules(schedule_id) ON DELETE RESTRICT,
  occurrence_key       TEXT    NOT NULL CHECK (length(occurrence_key) BETWEEN 1 AND 512),
  schedule_revision    INTEGER NOT NULL CHECK (schedule_revision BETWEEN 1 AND 4294967295),
  trigger_kind         TEXT    NOT NULL CHECK (trigger_kind IN
                         ('CALENDAR','HOST_EVENT','DEVICE_SESSION_ESTABLISHED','APPROVAL_EVENT')),
  source_event_id      TEXT    CHECK (source_event_id IS NULL OR
                         (length(source_event_id) = 30 AND substr(source_event_id,1,4) = 'evt_')),
  source_event_data_class_rank INTEGER CHECK (source_event_data_class_rank BETWEEN 0 AND 1),
  intended_local_label TEXT,
  timezone             TEXT,
  recurrence_evaluator TEXT,
  tzdb_version         TEXT,
  due_at_ms            INTEGER CHECK (due_at_ms BETWEEN -62167219200000 AND 253402300799999),
  not_before_ms        INTEGER CHECK (not_before_ms BETWEEN -62167219200000 AND 253402300799999),
  state                TEXT    NOT NULL CHECK (state IN ('PENDING','CLAIMED','MAPPED','SKIPPED','PROCESSED')),
  lease_owner          TEXT,
  lease_generation     INTEGER NOT NULL DEFAULT 0 CHECK (lease_generation BETWEEN 0 AND 4294967295),
  lease_expires_at_ms  INTEGER CHECK (lease_expires_at_ms BETWEEN -62167219200000 AND 253402300799999),
  mapped_task_id       TEXT    CHECK (mapped_task_id IS NULL OR
                         (length(mapped_task_id) = 30 AND substr(mapped_task_id,1,4) = 'tsk_')),
  template_digest      TEXT    NOT NULL CHECK (length(template_digest) = 71
                                                AND substr(template_digest,1,7) = 'sha256:'
                                                AND substr(template_digest,8) NOT GLOB '*[^0-9a-f]*'),
  template_data_class_rank INTEGER NOT NULL CHECK (template_data_class_rank BETWEEN 0 AND 2),
  outcome_code         TEXT,
  created_at_ms        INTEGER NOT NULL CHECK (created_at_ms BETWEEN -62167219200000 AND 253402300799999),
  updated_at_ms        INTEGER NOT NULL CHECK (updated_at_ms BETWEEN -62167219200000 AND 253402300799999),
  processed_at_ms      INTEGER CHECK (processed_at_ms BETWEEN -62167219200000 AND 253402300799999),
  PRIMARY KEY (schedule_id, occurrence_key),
  CHECK (updated_at_ms >= created_at_ms),
  CHECK ((state = 'CLAIMED') = (lease_owner IS NOT NULL)),
  CHECK ((state = 'CLAIMED') = (lease_expires_at_ms IS NOT NULL)),
  CHECK ((state = 'MAPPED') = (mapped_task_id IS NOT NULL)),
  CHECK (state NOT IN ('MAPPED','SKIPPED','PROCESSED') OR processed_at_ms IS NOT NULL),
  CHECK (state IN ('MAPPED','SKIPPED','PROCESSED') OR processed_at_ms IS NULL),
  CHECK ((trigger_kind = 'CALENDAR') = (source_event_id IS NULL)),
  CHECK ((source_event_id IS NULL) = (source_event_data_class_rank IS NULL)),
  CHECK (trigger_kind = 'CALENDAR' OR
         (intended_local_label IS NULL AND timezone IS NULL
          AND recurrence_evaluator IS NULL AND tzdb_version IS NULL)),
  FOREIGN KEY (template_digest, template_data_class_rank)
    REFERENCES blobs(digest, data_class_rank) ON DELETE RESTRICT
) STRICT;

CREATE UNIQUE INDEX schedule_occurrences_source_event
  ON schedule_occurrences(schedule_id, source_event_id)
  WHERE source_event_id IS NOT NULL;
CREATE INDEX schedule_occurrences_claim
  ON schedule_occurrences(schedule_id, state, due_at_ms, not_before_ms, occurrence_key);
CREATE INDEX schedule_occurrences_expired_lease
  ON schedule_occurrences(lease_expires_at_ms, schedule_id)
  WHERE state = 'CLAIMED';
CREATE UNIQUE INDEX schedule_occurrences_mapped_task
  ON schedule_occurrences(mapped_task_id)
  WHERE mapped_task_id IS NOT NULL;

CREATE TABLE schedule_command_receipts (
  message_id       TEXT    PRIMARY KEY CHECK (length(message_id) = 30
                                               AND substr(message_id,1,4) = 'evt_'),
  request_digest   TEXT    NOT NULL CHECK (length(request_digest) = 71
                                            AND substr(request_digest,1,7) = 'sha256:'
                                            AND substr(request_digest,8) NOT GLOB '*[^0-9a-f]*'),
  command_kind     TEXT    NOT NULL CHECK (command_kind IN
                         ('CREATE','UPDATE','PAUSE','RESUME','CANCEL')),
  schedule_id      TEXT    NOT NULL CHECK (length(schedule_id) = 30
                                            AND substr(schedule_id,1,4) = 'sch_'),
  result_revision  INTEGER NOT NULL CHECK (result_revision BETWEEN 1 AND 4294967295),
  result_state     TEXT    NOT NULL CHECK (result_state IN ('ACTIVE','PAUSED','CANCELLED')),
  committed_at_ms  INTEGER NOT NULL CHECK (committed_at_ms BETWEEN -62167219200000 AND 253402300799999)
) STRICT;

CREATE INDEX schedule_command_receipts_schedule
  ON schedule_command_receipts(schedule_id, committed_at_ms);

-- A task's sole authority to resume after a device connection. Event sequence
-- high-water makes registration order deterministic across clock skew/replay.
CREATE TABLE device_resume_waits (
  task_id TEXT PRIMARY KEY REFERENCES tasks(task_id) ON DELETE CASCADE,
  device_id TEXT NOT NULL CHECK (length(device_id)=30
                                  AND substr(device_id,1,4)='dev_'),
  blocked_task_revision INTEGER NOT NULL CHECK (blocked_task_revision BETWEEN 1 AND 9223372036854775807),
  registration_event_high_water_seq INTEGER NOT NULL CHECK (registration_event_high_water_seq BETWEEN 0 AND 9223372036854775807),
  created_at_ms INTEGER NOT NULL CHECK (created_at_ms BETWEEN -62167219200000 AND 253402300799999)
) STRICT;

CREATE INDEX device_resume_waits_by_device_sequence
  ON device_resume_waits(device_id, registration_event_high_water_seq, task_id);

-- Any ordinary Task state edge other than entering the explicitly registered
-- DEVICE_OFFLINE wait invalidates that wait in the same Task transaction.
CREATE TRIGGER tasks_device_wait_invalidate
AFTER UPDATE OF state,blocked_reason ON tasks
WHEN (NEW.state IS NOT OLD.state OR NEW.blocked_reason IS NOT OLD.blocked_reason)
  AND (NEW.state <> 'BLOCKED' OR NEW.blocked_reason <> 'DEVICE_OFFLINE')
BEGIN
  DELETE FROM device_resume_waits WHERE task_id=NEW.task_id;
END;

-- Materialized wakes intentionally do not reference event_content: retention
-- may delete source payload after the Scheduler cursor has advanced.
CREATE TABLE device_session_resume_wakes (
  source_event_id TEXT NOT NULL CHECK (length(source_event_id)=30
                                       AND substr(source_event_id,1,4)='evt_'),
  source_seq INTEGER NOT NULL CHECK (source_seq BETWEEN 1 AND 9223372036854775807),
  task_id TEXT NOT NULL REFERENCES tasks(task_id) ON DELETE CASCADE,
  device_id TEXT NOT NULL CHECK (length(device_id)=30
                                 AND substr(device_id,1,4)='dev_'),
  blocked_task_revision INTEGER NOT NULL CHECK (blocked_task_revision BETWEEN 1 AND 9223372036854775807),
  created_at_ms INTEGER NOT NULL CHECK (created_at_ms BETWEEN -62167219200000 AND 253402300799999),
  PRIMARY KEY (source_event_id, task_id)
) STRICT;

CREATE INDEX device_session_resume_wakes_pending
  ON device_session_resume_wakes(created_at_ms, source_event_id, task_id);
CREATE INDEX device_session_resume_wakes_task
  ON device_session_resume_wakes(task_id);

-- Approval lifecycle events are routed to future P6 only. These durable rows
-- contain identity and outcome kind, never grant or policy authority. Reads do
-- not consume a row; explicit acknowledgement removes it. Task deletion cascades
-- to avoid retaining a wake that targets a deleted Task.
CREATE TABLE approval_lifecycle_wakes (
  source_event_id TEXT PRIMARY KEY CHECK (length(source_event_id)=30
                                           AND substr(source_event_id,1,4)='evt_'),
  source_seq INTEGER NOT NULL UNIQUE CHECK (source_seq BETWEEN 1 AND 9223372036854775807),
  approval_id TEXT NOT NULL CHECK (length(approval_id)=30
                                   AND substr(approval_id,1,4)='apr_'),
  task_id TEXT NOT NULL REFERENCES tasks(task_id) ON DELETE CASCADE
                  CHECK (length(task_id)=30 AND substr(task_id,1,4)='tsk_'),
  step_id TEXT NOT NULL CHECK (length(step_id)=30 AND substr(step_id,1,4)='stp_'),
  outcome_kind TEXT NOT NULL CHECK (outcome_kind IN ('GRANTED','DENIED','EXPIRED')),
  created_at_ms INTEGER NOT NULL CHECK (created_at_ms BETWEEN -62167219200000 AND 253402300799999)
) STRICT;

CREATE INDEX approval_lifecycle_wakes_pending
  ON approval_lifecycle_wakes(created_at_ms, source_seq, source_event_id);
CREATE INDEX approval_lifecycle_wakes_task
  ON approval_lifecycle_wakes(task_id);

CREATE TABLE scheduler_consumer_state (
  singleton                INTEGER PRIMARY KEY CHECK (singleton = 1),
  last_processed_seq       INTEGER NOT NULL DEFAULT 0 CHECK (last_processed_seq BETWEEN 0 AND 9223372036854775807),
  replay_high_water_seq    INTEGER CHECK (replay_high_water_seq BETWEEN 0 AND 9223372036854775807),
  lease_owner              TEXT,
  lease_generation         INTEGER NOT NULL DEFAULT 0 CHECK (lease_generation BETWEEN 0 AND 4294967295),
  lease_expires_at_ms      INTEGER CHECK (lease_expires_at_ms BETWEEN -62167219200000 AND 253402300799999),
  CHECK ((lease_owner IS NULL) = (lease_expires_at_ms IS NULL)),
  CHECK (replay_high_water_seq IS NULL OR replay_high_water_seq >= last_processed_seq)
) STRICT;

INSERT INTO scheduler_consumer_state(singleton) VALUES (1);
