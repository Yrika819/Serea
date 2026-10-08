-- P4B durable model-call accounting. This migration stores trusted attempt facts,
-- bounded accounting metadata and accepted-response blob references only.

-- Preserve the per-task dispatch budget after 30-day terminal attempt pruning.
-- Existing pre-P4 tasks start at zero because no P4 calls have been dispatched.
ALTER TABLE tasks
  ADD COLUMN model_call_count INTEGER NOT NULL DEFAULT 0 CHECK (model_call_count BETWEEN 0 AND 12);

-- Preserve top-level model-turn consumption after terminal attempt pruning.
-- Fallback and repair attempts consume calls but do not add another turn.
ALTER TABLE tasks
  ADD COLUMN model_turn_count INTEGER NOT NULL DEFAULT 0 CHECK (model_turn_count BETWEEN 0 AND 12);

-- Preserve trustworthy task token totals after usage-row retention removes
-- the detailed provider accounting records.
ALTER TABLE tasks
  ADD COLUMN model_token_count INTEGER NOT NULL DEFAULT 0 CHECK (model_token_count >= 0);

CREATE TABLE model_call_attempts (
  request_id                         TEXT    PRIMARY KEY CHECK (length(request_id) = 30
                                                               AND substr(request_id, 1, 4) = 'req_'
                                                               AND substr(request_id, 5, 1) <= '7'),
  task_id                            TEXT    REFERENCES tasks(task_id) ON DELETE SET NULL,
  purpose                            TEXT    NOT NULL CHECK (purpose IN ('CHAT','PLANNING','EXTRACTION','ANALYSIS','PROACTIVE','STRUCTURED_REPAIR')),
  model_id                           TEXT    NOT NULL CHECK (length(model_id) BETWEEN 1 AND 128),
  provider_id                        TEXT    NOT NULL CHECK (length(provider_id) BETWEEN 1 AND 128),
  deployment_class                   TEXT    NOT NULL CHECK (deployment_class IN ('CLOUD','LOCAL')),
  data_class_rank                    INTEGER NOT NULL CHECK (data_class_rank BETWEEN 0 AND 1),
  state                              TEXT    NOT NULL CHECK (state IN ('DISPATCH_INTENT','COMPLETED','FAILED','AMBIGUOUS')),
  relation_kind                      TEXT    NOT NULL CHECK (relation_kind IN ('NONE','FALLBACK','REPAIR')),
  parent_request_id                  TEXT,
  fallback_from_model_id             TEXT,
  accounting_day_utc                 INTEGER NOT NULL,
  cost_class                         TEXT    NOT NULL CHECK (cost_class IN ('FREE','LOW','PAID')),
  price_revision                     TEXT    NOT NULL CHECK (length(price_revision) BETWEEN 1 AND 128),
  input_rate_microusd_per_million     INTEGER NOT NULL CHECK (input_rate_microusd_per_million >= 0),
  output_rate_microusd_per_million    INTEGER NOT NULL CHECK (output_rate_microusd_per_million >= 0),
  max_context_tokens                  INTEGER NOT NULL CHECK (max_context_tokens BETWEEN 1 AND 9223372036854775807),
  effective_max_output_tokens         INTEGER NOT NULL CHECK (effective_max_output_tokens BETWEEN 1 AND max_context_tokens),
  reserved_cost_usd_micros            INTEGER NOT NULL CHECK (reserved_cost_usd_micros >= 0),
  actual_cost_usd_micros               INTEGER CHECK (actual_cost_usd_micros IS NULL OR actual_cost_usd_micros >= 0),
  finish_reason                       TEXT CHECK (finish_reason IS NULL OR finish_reason IN ('STOP','LENGTH','CONTENT_FILTER','ERROR','STRUCTURE_INVALID')),
  error_kind                          TEXT CHECK (error_kind IS NULL OR (length(error_kind) BETWEEN 1 AND 128
                                                                          AND error_kind NOT GLOB '*[^A-Z0-9_]*')),
  response_blob_digest                TEXT,
  response_data_class_rank            INTEGER CHECK (response_data_class_rank IS NULL OR response_data_class_rank BETWEEN 0 AND 1),
  response_storage_allowed            INTEGER NOT NULL DEFAULT 1 CHECK (response_storage_allowed IN (0,1)),
  dispatch_intent_at_ms               INTEGER NOT NULL CHECK (dispatch_intent_at_ms BETWEEN -62167219200000 AND 253402300799999),
  terminal_at_ms                      INTEGER CHECK (terminal_at_ms BETWEEN -62167219200000 AND 253402300799999),
  CHECK ((relation_kind = 'NONE' AND parent_request_id IS NULL AND fallback_from_model_id IS NULL)
      OR (relation_kind = 'FALLBACK' AND parent_request_id IS NOT NULL AND fallback_from_model_id IS NOT NULL)
      OR (relation_kind = 'REPAIR' AND parent_request_id IS NOT NULL AND fallback_from_model_id IS NULL)),
  CHECK (parent_request_id IS NULL OR parent_request_id <> request_id),
  CHECK (cost_class <> 'FREE' OR (input_rate_microusd_per_million = 0 AND output_rate_microusd_per_million = 0)),
  CHECK ((response_blob_digest IS NULL) = (response_data_class_rank IS NULL)),
  CHECK (response_data_class_rank IS NULL OR response_data_class_rank >= data_class_rank),
  CHECK (state = 'DISPATCH_INTENT' OR terminal_at_ms IS NOT NULL),
  CHECK ((state = 'DISPATCH_INTENT') = (terminal_at_ms IS NULL)),
  CHECK ((state IN ('FAILED','AMBIGUOUS')) = (error_kind IS NOT NULL)),
  CHECK (state <> 'COMPLETED' OR (actual_cost_usd_micros IS NOT NULL AND finish_reason IS NOT NULL)),
  CHECK (state = 'COMPLETED' OR response_blob_digest IS NULL),
  CHECK (response_storage_allowed = 0 OR state <> 'COMPLETED' OR response_blob_digest IS NOT NULL),
  CHECK (response_storage_allowed = 1 OR response_blob_digest IS NULL),
  FOREIGN KEY (response_blob_digest, response_data_class_rank)
    REFERENCES blobs(digest, data_class_rank) ON DELETE RESTRICT
) STRICT;

CREATE UNIQUE INDEX model_call_one_active_per_task
  ON model_call_attempts(task_id)
  WHERE task_id IS NOT NULL AND state = 'DISPATCH_INTENT';
CREATE INDEX model_call_attempts_task_state
  ON model_call_attempts(task_id, state);
CREATE INDEX model_call_attempts_spend_day_state
  ON model_call_attempts(accounting_day_utc, state);
CREATE INDEX model_call_attempts_recovery
  ON model_call_attempts(state, dispatch_intent_at_ms, request_id)
  WHERE state = 'DISPATCH_INTENT';
CREATE INDEX model_call_attempts_terminal_retention
  ON model_call_attempts(terminal_at_ms, request_id)
  WHERE terminal_at_ms IS NOT NULL;
CREATE INDEX model_call_attempts_response_blob
  ON model_call_attempts(response_blob_digest, response_data_class_rank)
  WHERE response_blob_digest IS NOT NULL;

CREATE TABLE model_usage (
  usage_id                         INTEGER PRIMARY KEY,
  request_id                       TEXT UNIQUE REFERENCES model_call_attempts(request_id) ON DELETE SET NULL,
  task_id                          TEXT REFERENCES tasks(task_id) ON DELETE SET NULL,
  model_id                         TEXT    NOT NULL CHECK (length(model_id) BETWEEN 1 AND 128),
  purpose                          TEXT    NOT NULL CHECK (purpose IN ('CHAT','PLANNING','EXTRACTION','ANALYSIS','PROACTIVE','STRUCTURED_REPAIR')),
  input_tokens                     INTEGER NOT NULL CHECK (input_tokens >= 0),
  output_tokens                    INTEGER NOT NULL CHECK (output_tokens >= 0),
  cost_usd_micros                  INTEGER NOT NULL CHECK (cost_usd_micros >= 0),
  cost_class                       TEXT    NOT NULL CHECK (cost_class IN ('FREE','LOW','PAID')),
  latency_ms                       INTEGER NOT NULL CHECK (latency_ms >= 0),
  repair_attempts                  INTEGER NOT NULL CHECK (repair_attempts BETWEEN 0 AND 2),
  fallback_from_model_id           TEXT,
  recorded_at_ms                   INTEGER NOT NULL CHECK (recorded_at_ms BETWEEN -62167219200000 AND 253402300799999)
) STRICT;

CREATE INDEX model_usage_task_id ON model_usage(task_id) WHERE task_id IS NOT NULL;
CREATE INDEX model_usage_recorded_at ON model_usage(recorded_at_ms, usage_id);

-- Task deletion is also a privacy boundary for recoverable response content.
-- Keep non-identifying accounting facts, but clear the content reference and
-- remember that a late response for this now-deleted task must not be retained.
CREATE TRIGGER tasks_model_response_privacy_delete
BEFORE DELETE ON tasks
BEGIN
  UPDATE model_call_attempts
     SET response_blob_digest=NULL,
         response_data_class_rank=NULL,
         response_storage_allowed=0
   WHERE task_id=OLD.task_id;
END;
