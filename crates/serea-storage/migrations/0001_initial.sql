
-- Foreign-key enforcement is set per connection by Store::open, not here:
-- PRAGMA foreign_keys is a no-op inside a transaction, and every migration runs
-- inside BEGIN IMMEDIATE. See P2-storage-task-engine.md 7.2.

CREATE TABLE schema_migrations (
  version       INTEGER PRIMARY KEY CHECK (version >= 1),
  name          TEXT    NOT NULL UNIQUE,
  checksum      TEXT    NOT NULL CHECK (length(checksum) = 71
                                       AND substr(checksum, 1, 7) = 'sha256:'
                                       AND substr(checksum, 8) NOT GLOB '*[^0-9a-f]*'
                                                                              AND instr(checksum, char(0)) = 0),
  applied_at_ms INTEGER NOT NULL CHECK (applied_at_ms BETWEEN -62167219200000 AND 253402300799999)
) STRICT;

CREATE TABLE blobs (
  digest          TEXT    NOT NULL CHECK (length(digest) = 71
                                          AND substr(digest, 1, 7) = 'sha256:'
                                          AND substr(digest, 8) NOT GLOB '*[^0-9a-f]*'),
  data_class_rank INTEGER NOT NULL CHECK (data_class_rank BETWEEN 0 AND 2),
  protection      TEXT    NOT NULL CHECK (protection IN ('NONE', 'AT_REST')),
  size_bytes      INTEGER NOT NULL CHECK (size_bytes >= 0),
  content         BLOB    NOT NULL,
  data_class      TEXT GENERATED ALWAYS AS (
                      CASE data_class_rank
                        WHEN 0 THEN 'PUBLIC' WHEN 1 THEN 'PERSONAL'
                        ELSE 'PRIVATE' END) STORED,
  PRIMARY KEY (digest, data_class_rank),
  CHECK (size_bytes = length(content)),
  CHECK ((data_class_rank = 2) = (protection = 'AT_REST'))
) STRICT;

CREATE TABLE tasks (
  task_id              TEXT    PRIMARY KEY CHECK (length(task_id) = 30
                                                   AND substr(task_id, 1, 4) = 'tsk_'
                                                   AND substr(task_id, 5, 1) <= '7'),
  kind                 TEXT    NOT NULL CHECK (kind IN ('USER_REQUEST','SCHEDULED','PROACTIVE','DELEGATED_HOST_GOAL','MAINTENANCE')),
  title                TEXT    NOT NULL,
  state                TEXT    NOT NULL CHECK (state IN ('RECEIVED','PLANNING','READY','EXECUTING','WAITING_APPROVAL','WAITING_USER','VERIFYING','COMPLETED','FAILED','BLOCKED','CANCELLED')),
  origin_kind          TEXT    NOT NULL,
  origin_device_id     TEXT,
  origin_message_id    TEXT,
  origin_extensions    TEXT    NOT NULL DEFAULT '{}'
                               CHECK (json_valid(origin_extensions)
                                      AND json_type(origin_extensions) = 'object'),
  data_class_rank      INTEGER NOT NULL CHECK (data_class_rank BETWEEN 0 AND 2),
  policy_class_rank    INTEGER NOT NULL CHECK (policy_class_rank BETWEEN 0 AND 7),
  created_at_ms        INTEGER NOT NULL CHECK (created_at_ms BETWEEN -62167219200000 AND 253402300799999),
  updated_at_ms        INTEGER NOT NULL CHECK (updated_at_ms BETWEEN -62167219200000 AND 253402300799999),
  deadline_at_ms       INTEGER CHECK (deadline_at_ms BETWEEN -62167219200000 AND 253402300799999),
  blocked_reason       TEXT,
  result_summary       TEXT,
  cancelled_at_ms      INTEGER CHECK (cancelled_at_ms BETWEEN -62167219200000 AND 253402300799999),
  cancelled_by         TEXT,
  failure_reason       TEXT,
  max_model_calls      INTEGER NOT NULL CHECK (max_model_calls >= 0),
  max_tool_calls       INTEGER NOT NULL CHECK (max_tool_calls >= 0),
  max_attempts_per_step INTEGER NOT NULL CHECK (max_attempts_per_step >= 0),
  budget_extensions    TEXT    NOT NULL DEFAULT '{}'
                               CHECK (json_valid(budget_extensions)
                                      AND json_type(budget_extensions) = 'object'),
  plan_revision        INTEGER NOT NULL DEFAULT 0 CHECK (plan_revision >= 0),
  extensions           TEXT    NOT NULL DEFAULT '{}'
                               CHECK (json_valid(extensions)
                                      AND json_type(extensions) = 'object'),
  data_class           TEXT GENERATED ALWAYS AS (
                         CASE data_class_rank
                           WHEN 0 THEN 'PUBLIC' WHEN 1 THEN 'PERSONAL'
                           WHEN 2 THEN 'PRIVATE' ELSE 'SECRET' END) STORED,
  policy_class         TEXT GENERATED ALWAYS AS (
                         CASE policy_class_rank
                           WHEN 0 THEN 'OBSERVE' WHEN 1 THEN 'LOCAL_STATE'
                           WHEN 2 THEN 'REVERSIBLE_WRITE' WHEN 3 THEN 'EXTERNAL_WRITE'
                           WHEN 4 THEN 'COMMUNICATION' WHEN 5 THEN 'ELEVATED_DEVICE'
                           WHEN 6 THEN 'DESTRUCTIVE' ELSE 'CREDENTIAL' END) STORED,
  CHECK (updated_at_ms >= created_at_ms),
  CHECK (deadline_at_ms IS NULL OR deadline_at_ms >= created_at_ms),
  CHECK (state = 'BLOCKED' OR blocked_reason IS NULL),
  CHECK ((state = 'CANCELLED') = (cancelled_at_ms IS NOT NULL AND cancelled_by IS NOT NULL)),
  CHECK (state = 'CANCELLED' OR (cancelled_at_ms IS NULL AND cancelled_by IS NULL)),
  CHECK (state <> 'FAILED' OR failure_reason IS NOT NULL),
  CHECK (state = 'FAILED' OR failure_reason IS NULL),
  CHECK (origin_kind NOT GLOB '*[^A-Z0-9_]*' AND substr(origin_kind,1,1) BETWEEN 'A' AND 'Z'),
  CHECK (origin_device_id IS NULL OR (length(origin_device_id)=30 AND substr(origin_device_id,1,4)='dev_')),
  CHECK (origin_message_id IS NULL OR (length(origin_message_id)=30 AND substr(origin_message_id,1,4)='evt_')),
  CHECK (blocked_reason IS NULL OR (blocked_reason NOT GLOB '*[^A-Z0-9_]*' AND substr(blocked_reason,1,1) BETWEEN 'A' AND 'Z')),
  CHECK (failure_reason IS NULL OR (failure_reason NOT GLOB '*[^A-Z0-9_]*' AND substr(failure_reason,1,1) BETWEEN 'A' AND 'Z')),
  CHECK (cancelled_by IS NULL OR (cancelled_by NOT GLOB '*[^A-Z0-9_]*' AND substr(cancelled_by,1,1) BETWEEN 'A' AND 'Z'))
) STRICT;

CREATE TRIGGER tasks_policy_class_immutable
BEFORE UPDATE OF policy_class_rank ON tasks
WHEN NEW.policy_class_rank IS NOT OLD.policy_class_rank
BEGIN SELECT RAISE(ABORT, 'policy_class is immutable for a task'); END;

CREATE TRIGGER tasks_data_class_monotonic
BEFORE UPDATE OF data_class_rank ON tasks
WHEN NEW.data_class_rank < OLD.data_class_rank
BEGIN SELECT RAISE(ABORT, 'data_class may not be lowered'); END;

CREATE TABLE task_steps (
  step_id            TEXT    PRIMARY KEY CHECK (length(step_id) = 30
                                                 AND substr(step_id, 1, 4) = 'stp_'
                                                 AND substr(step_id, 5, 1) <= '7'),
  task_id            TEXT    NOT NULL REFERENCES tasks(task_id) ON DELETE CASCADE,
  sequence           INTEGER NOT NULL CHECK (sequence >= 0),
  kind               TEXT    NOT NULL CHECK (kind IN ('CAPABILITY','MODEL_TURN','WAIT_APPROVAL','WAIT_USER','WAIT_SCHEDULE','VERIFY','NOTIFY','DELEGATE')),
  status             TEXT    NOT NULL CHECK (status IN ('PLANNED','LEASED','EXECUTING','WAITING','SUCCEEDED','FAILED','RECONCILED_ABSENT')),
  attempt            INTEGER NOT NULL DEFAULT 0 CHECK (attempt >= 0),
  plan_revision      INTEGER NOT NULL DEFAULT 0 CHECK (plan_revision >= 0),
  provider_id        TEXT,
  capability_id      TEXT,
  capability_version TEXT,
  idempotency_key    TEXT,
  input_digest       TEXT    NOT NULL CHECK (length(input_digest) = 71
                                            AND substr(input_digest,1,7) = 'sha256:'
                                            AND substr(input_digest,8) NOT GLOB '*[^0-9a-f]*'),
  result_digest      TEXT    CHECK (result_digest IS NULL OR (length(result_digest) = 71
                                            AND substr(result_digest,1,7) = 'sha256:'
                                            AND substr(result_digest,8) NOT GLOB '*[^0-9a-f]*')),
  started_at_ms      INTEGER CHECK (started_at_ms BETWEEN -62167219200000 AND 253402300799999),
  completed_at_ms    INTEGER CHECK (completed_at_ms BETWEEN -62167219200000 AND 253402300799999),
  lease_owner        TEXT,
  lease_expires_at_ms INTEGER CHECK (lease_expires_at_ms BETWEEN -62167219200000 AND 253402300799999),
  lease_generation   INTEGER NOT NULL DEFAULT 0 CHECK (lease_generation BETWEEN 0 AND 4294967295),
  error_kind         TEXT,
  error_code         TEXT,
  error_message      TEXT,
  error_retryable    INTEGER CHECK (error_retryable IS NULL OR error_retryable IN (0,1)),
  error_host_action  TEXT,
  error_details      TEXT    CHECK (error_details IS NULL
                                     OR (json_valid(error_details)
                                         AND json_type(error_details) = 'object')),
  UNIQUE (task_id, sequence),
  UNIQUE (task_id, idempotency_key),
  -- ADR-0018 3: PLANNED
  CHECK (status <> 'PLANNED' OR (attempt = 0
      AND result_digest IS NULL AND started_at_ms IS NULL AND completed_at_ms IS NULL
      AND lease_owner IS NULL AND lease_expires_at_ms IS NULL AND lease_generation = 0)),
  CHECK (status = 'PLANNED' OR attempt >= 1),
  CHECK (status = 'PLANNED' OR lease_generation >= 1),
  CHECK (attempt <= 4294967295),
  -- ADR-0018 3: LEASED  (attempt begun, attempt not yet started)
  CHECK (status <> 'LEASED' OR (attempt >= 1 AND started_at_ms IS NULL
      AND completed_at_ms IS NULL AND result_digest IS NULL
      AND lease_owner IS NOT NULL AND lease_expires_at_ms IS NOT NULL)),
  -- lease present exactly while leased or executing
  CHECK ((status IN ('LEASED','EXECUTING')) = (lease_owner IS NOT NULL)),
  -- ... and so is its expiry. Enforced as a biconditional rather than left
  -- one-directional: the pair must not disagree, or a terminal step can carry a
  -- dangling expiry with no owner, which is the same hole the lease_owner
  -- biconditional above was added to close.
  CHECK ((status IN ('LEASED','EXECUTING')) = (lease_expires_at_ms IS NOT NULL)),
  -- started_at required from EXECUTING onward
  CHECK (status NOT IN ('EXECUTING','WAITING','SUCCEEDED','FAILED','RECONCILED_ABSENT')
         OR started_at_ms IS NOT NULL),
  -- completed_at required for every terminal step status
  CHECK (status NOT IN ('SUCCEEDED','FAILED','RECONCILED_ABSENT') OR completed_at_ms IS NOT NULL),
  -- completed_at and result_digest absent for every non-terminal status. ADR-0018
  -- §3 marks both `N` here; without these two clauses a step that is still in
  -- flight could also read as finished.
  CHECK (status NOT IN ('PLANNED','LEASED','EXECUTING','WAITING') OR completed_at_ms IS NULL),
  CHECK (status NOT IN ('PLANNED','LEASED','EXECUTING','WAITING') OR result_digest IS NULL),
  CHECK (status <> 'SUCCEEDED' OR result_digest IS NOT NULL),
  -- WAITING only for the wait kinds
  CHECK (status <> 'WAITING' OR kind IN ('WAIT_APPROVAL','WAIT_USER','WAIT_SCHEDULE')),
  -- FAILED requires all mandatory error members; details remains optional.
  CHECK (status <> 'FAILED' OR (error_kind IS NOT NULL AND error_code IS NOT NULL
      AND error_message IS NOT NULL AND error_host_action IS NOT NULL
      AND error_retryable IS NOT NULL)),
  -- Outside FAILED, every member is absent, including optional details.
  CHECK (status = 'FAILED' OR (error_kind IS NULL AND error_code IS NULL
      AND error_message IS NULL AND error_host_action IS NULL
      AND error_retryable IS NULL AND error_details IS NULL)),
  -- ADR-0018 4: step-kind matrix
  -- ADR-0018 4, one clause per field. A single biconditional over all four
  -- admits a PARTIALLY populated tuple: kind=NOTIFY with capability_id set and
  -- the other three null evaluates 0 = 0 and is accepted.
  CHECK ((kind IN ('CAPABILITY','DELEGATE','VERIFY')) = (capability_id IS NOT NULL)),
  CHECK ((kind IN ('CAPABILITY','DELEGATE','VERIFY')) = (capability_version IS NOT NULL)),
  CHECK ((kind IN ('CAPABILITY','DELEGATE','VERIFY')) = (provider_id IS NOT NULL)),
  CHECK ((kind IN ('CAPABILITY','DELEGATE','VERIFY')) = (idempotency_key IS NOT NULL)),
  CHECK (capability_id IS NULL OR (length(capability_id) <= 99
                                   AND capability_id NOT GLOB 'goallatch.*')),
  CHECK (idempotency_key IS NULL OR (length(idempotency_key) = 68
                                     AND substr(idempotency_key,1,4) = 'idk_'
                                     AND substr(idempotency_key,5) NOT GLOB '*[^0-9a-f]*')),
  CHECK (error_code IS NULL OR (error_code NOT GLOB '*[^A-Z0-9_]*' AND substr(error_code,1,1) BETWEEN 'A' AND 'Z')),
  CHECK (error_host_action IS NULL OR (error_host_action NOT GLOB '*[^A-Z0-9_]*' AND substr(error_host_action,1,1) BETWEEN 'A' AND 'Z'))
) STRICT;

CREATE TABLE side_effect_receipts (
  receipt_id         TEXT    PRIMARY KEY CHECK (length(receipt_id) = 30
                                                 AND substr(receipt_id,1,4) = 'rcp_'),
  task_id            TEXT    NOT NULL REFERENCES tasks(task_id) ON DELETE CASCADE,
  step_id            TEXT    NOT NULL REFERENCES task_steps(step_id) ON DELETE CASCADE,
  capability_id      TEXT    NOT NULL,
  idempotency_key    TEXT    NOT NULL CHECK (length(idempotency_key) = 68
                                             AND substr(idempotency_key,1,4) = 'idk_'
                                             AND substr(idempotency_key,5) NOT GLOB '*[^0-9a-f]*'),
  provider_reference TEXT,
  effect_summary     TEXT    NOT NULL,
  observed_at_ms     INTEGER NOT NULL CHECK (observed_at_ms BETWEEN -62167219200000 AND 253402300799999),
  replay_safe        INTEGER NOT NULL CHECK (replay_safe IN (0,1)),
  data_class_rank    INTEGER NOT NULL CHECK (data_class_rank BETWEEN 0 AND 2),
  UNIQUE (step_id)
) STRICT;

CREATE TRIGGER side_effect_receipts_key_matches_step
BEFORE INSERT ON side_effect_receipts
WHEN (SELECT idempotency_key FROM task_steps WHERE step_id = NEW.step_id) IS NOT NEW.idempotency_key
BEGIN SELECT RAISE(ABORT, 'receipt idempotency_key must equal its step key'); END;

CREATE TRIGGER side_effect_receipts_task_matches_step
BEFORE INSERT ON side_effect_receipts
WHEN (SELECT task_id FROM task_steps WHERE step_id = NEW.step_id) IS NOT NEW.task_id
BEGIN SELECT RAISE(ABORT, 'receipt task_id must equal its step task_id'); END;

CREATE TRIGGER side_effect_receipts_step_must_succeed
BEFORE INSERT ON side_effect_receipts
WHEN (SELECT status FROM task_steps WHERE step_id = NEW.step_id) <> 'SUCCEEDED'
BEGIN SELECT RAISE(ABORT, 'a receipt may only be recorded for a SUCCEEDED step'); END;

CREATE TABLE leases (
  step_id        TEXT    PRIMARY KEY REFERENCES task_steps(step_id) ON DELETE CASCADE,
  owner          TEXT    NOT NULL,
  generation     INTEGER NOT NULL CHECK (generation BETWEEN 1 AND 4294967295),
  acquired_at_ms INTEGER NOT NULL CHECK (acquired_at_ms BETWEEN -62167219200000 AND 253402300799999),
  expires_at_ms  INTEGER NOT NULL CHECK (expires_at_ms BETWEEN -62167219200000 AND 253402300799999),
  released_at_ms INTEGER CHECK (released_at_ms BETWEEN -62167219200000 AND 253402300799999),
  CHECK (expires_at_ms > acquired_at_ms),
  CHECK (released_at_ms IS NULL OR released_at_ms >= acquired_at_ms)
) STRICT;

CREATE TRIGGER task_steps_idempotency_key_immutable
BEFORE UPDATE OF idempotency_key ON task_steps
WHEN NEW.idempotency_key IS NOT OLD.idempotency_key
BEGIN SELECT RAISE(ABORT, 'idempotency_key is derived at plan time and never changes'); END;


CREATE TABLE plan_revisions (
  task_id       TEXT    NOT NULL REFERENCES tasks(task_id) ON DELETE CASCADE,
  plan_revision INTEGER NOT NULL CHECK (plan_revision >= 0),
  created_at_ms INTEGER NOT NULL CHECK (created_at_ms BETWEEN -62167219200000 AND 253402300799999),
  plan_digest   TEXT    NOT NULL,
  data_class_rank INTEGER NOT NULL CHECK (data_class_rank BETWEEN 0 AND 2),
  step_count    INTEGER NOT NULL CHECK (step_count >= 0),
  PRIMARY KEY (task_id, plan_revision),
  FOREIGN KEY (plan_digest, data_class_rank)
    REFERENCES blobs(digest, data_class_rank) ON DELETE RESTRICT
) STRICT;

CREATE TABLE task_blob_refs (
  task_id        TEXT    NOT NULL REFERENCES tasks(task_id) ON DELETE CASCADE,
  role           TEXT    NOT NULL CHECK (role IN ('PLAN','PLAN_REVISION')),
  digest         TEXT    NOT NULL,
  data_class_rank INTEGER NOT NULL CHECK (data_class_rank BETWEEN 0 AND 2),
  PRIMARY KEY (task_id, role, digest),
  FOREIGN KEY (digest, data_class_rank)
    REFERENCES blobs(digest, data_class_rank) ON DELETE RESTRICT
) STRICT;

CREATE TABLE step_blob_refs (
  step_id        TEXT    NOT NULL REFERENCES task_steps(step_id) ON DELETE CASCADE,
  role           TEXT    NOT NULL CHECK (role IN ('ARGUMENTS','INSTRUCTION','RESULT')),
  digest         TEXT    NOT NULL,
  data_class_rank INTEGER NOT NULL CHECK (data_class_rank BETWEEN 0 AND 2),
  PRIMARY KEY (step_id, role, digest),
  FOREIGN KEY (digest, data_class_rank)
    REFERENCES blobs(digest, data_class_rank) ON DELETE RESTRICT
) STRICT;

CREATE TABLE task_journal (
  journal_id       TEXT    PRIMARY KEY,
  task_id          TEXT    NOT NULL REFERENCES tasks(task_id) ON DELETE CASCADE,
  step_id          TEXT,
  journal_seq      INTEGER NOT NULL CHECK (journal_seq >= 1),
  journal_kind     TEXT    NOT NULL CHECK (journal_kind IN (
                     'TASK_INSERTED','PLAN_PERSISTED','TASK_STATE_CHANGED',
                     'STEP_LEASE_ACQUIRED','STEP_LEASE_RELEASED','STEP_ATTEMPT_STARTED',
                     'STEP_COMMITTED','STEP_FAILED','STEP_RECONCILED_ABSENT',
                     'RECEIPT_RECORDED','RECOVERY_DECISION','TASK_CANCEL_REQUESTED',
                     'TASK_TERMINAL')),
  state_from       TEXT,
  state_to         TEXT,
  attempt          INTEGER,
  reason_code      TEXT,
  actor_kind       TEXT    NOT NULL,
  actor_id         TEXT    NOT NULL,
  actor_version    TEXT    NOT NULL,
  causation_id     TEXT,
  data_class_rank  INTEGER NOT NULL CHECK (data_class_rank BETWEEN 0 AND 2),
  occurred_at_ms   INTEGER NOT NULL CHECK (occurred_at_ms BETWEEN -62167219200000 AND 253402300799999),
  payload_digest   TEXT    CHECK (payload_digest IS NULL OR (length(payload_digest) = 71
                                    AND substr(payload_digest,1,7) = 'sha256:'
                                    AND substr(payload_digest,8) NOT GLOB '*[^0-9a-f]*')),
  payload_json     TEXT    CHECK (payload_json IS NULL OR json_valid(payload_json)),
  payload_ref_digest TEXT,
  UNIQUE (task_id, journal_seq)
) STRICT;

CREATE TRIGGER task_journal_step_task_matches
BEFORE INSERT ON task_journal
WHEN NEW.step_id IS NOT NULL
 AND (SELECT task_id FROM task_steps WHERE step_id = NEW.step_id) IS NOT NEW.task_id
BEGIN SELECT RAISE(ABORT, 'journal step_id must belong to the journal task'); END;

CREATE INDEX task_steps_status_lease ON task_steps(status, lease_expires_at_ms);
CREATE INDEX task_steps_task_status ON task_steps(task_id, status);
CREATE INDEX tasks_state ON tasks(state);
CREATE INDEX step_blob_refs_digest ON step_blob_refs(digest);
CREATE INDEX task_blob_refs_digest ON task_blob_refs(digest);
CREATE INDEX plan_revisions_digest ON plan_revisions(plan_digest);
