-- P5B durable capability registry foundation. Provider schema bytes and
-- provider-discovered authority are intentionally absent.

CREATE TABLE capability_registry_generations (
  generation_id           INTEGER PRIMARY KEY AUTOINCREMENT CHECK (generation_id > 0),
  manifest_digest         TEXT    NOT NULL CHECK (length(manifest_digest) = 71
                                                AND substr(manifest_digest,1,7)='sha256:'
                                                AND substr(manifest_digest,8) NOT GLOB '*[^0-9a-f]*'),
  schema_catalog_digest   TEXT    NOT NULL CHECK (length(schema_catalog_digest) = 71
                                                AND substr(schema_catalog_digest,1,7)='sha256:'
                                                AND substr(schema_catalog_digest,8) NOT GLOB '*[^0-9a-f]*'),
  activated_at_ms         INTEGER CHECK (activated_at_ms BETWEEN -62167219200000 AND 253402300799999)
) STRICT;

CREATE TABLE capability_registry_state (
  singleton             INTEGER PRIMARY KEY CHECK (singleton = 1),
  active_generation_id  INTEGER REFERENCES capability_registry_generations(generation_id) ON DELETE RESTRICT
) STRICT;
INSERT INTO capability_registry_state(singleton,active_generation_id) VALUES (1,NULL);

CREATE TRIGGER capability_registry_state_no_delete
BEFORE DELETE ON capability_registry_state
BEGIN SELECT RAISE(ABORT, 'registry singleton state cannot be deleted'); END;

CREATE TRIGGER capability_generation_activation_once
BEFORE UPDATE OF activated_at_ms ON capability_registry_generations
WHEN OLD.activated_at_ms IS NOT NULL OR NEW.activated_at_ms IS NULL
BEGIN SELECT RAISE(ABORT, 'registry generation activation is immutable'); END;

CREATE TRIGGER capability_generation_facts_immutable_after_activation
BEFORE UPDATE OF generation_id,manifest_digest,schema_catalog_digest ON capability_registry_generations
WHEN OLD.activated_at_ms IS NOT NULL
BEGIN SELECT RAISE(ABORT, 'activated registry generation is immutable'); END;

CREATE TRIGGER capability_generation_activated_no_delete
BEFORE DELETE ON capability_registry_generations
WHEN OLD.activated_at_ms IS NOT NULL
BEGIN SELECT RAISE(ABORT, 'activated registry generation is immutable'); END;

CREATE TABLE capability_descriptor_revisions (
  descriptor_digest       TEXT    PRIMARY KEY CHECK (length(descriptor_digest) = 71
                                                AND substr(descriptor_digest,1,7)='sha256:'
                                                AND substr(descriptor_digest,8) NOT GLOB '*[^0-9a-f]*'),
  capability_id           TEXT    NOT NULL CHECK (length(capability_id) BETWEEN 1 AND 99),
  capability_version     TEXT    NOT NULL CHECK (length(capability_version) BETWEEN 5 AND 128),
  provider_id             TEXT    NOT NULL CHECK (length(provider_id) BETWEEN 1 AND 32),
  implementation_id      TEXT    CHECK (implementation_id IS NULL OR length(implementation_id) BETWEEN 1 AND 64),
  title                   TEXT    NOT NULL,
  description             TEXT    NOT NULL,
  input_schema_uri        TEXT    NOT NULL CHECK (length(input_schema_uri) BETWEEN 1 AND 512),
  input_schema_digest     TEXT    NOT NULL CHECK (length(input_schema_digest) = 71
                                                AND substr(input_schema_digest,1,7)='sha256:'
                                                AND substr(input_schema_digest,8) NOT GLOB '*[^0-9a-f]*'),
  output_schema_uri       TEXT    NOT NULL CHECK (length(output_schema_uri) BETWEEN 1 AND 512),
  output_schema_digest    TEXT    NOT NULL CHECK (length(output_schema_digest) = 71
                                                AND substr(output_schema_digest,1,7)='sha256:'
                                                AND substr(output_schema_digest,8) NOT GLOB '*[^0-9a-f]*'),
  side_effect_class       TEXT    NOT NULL CHECK (side_effect_class IN ('NONE','LOCAL_STATE','DEVICE_STATE','EXTERNAL_WRITE','COMMUNICATION','ELEVATED_DEVICE')),
  risk_class              TEXT    NOT NULL CHECK (risk_class IN ('OBSERVE','LOCAL_STATE','REVERSIBLE_WRITE','EXTERNAL_WRITE','COMMUNICATION','ELEVATED_DEVICE','DESTRUCTIVE','CREDENTIAL')),
  required_authorization TEXT    NOT NULL CHECK (required_authorization IN ('NONE','DEVICE_USER','SCOPED_GRANT','CREDENTIAL_HANDOFF')),
  replay_safety           TEXT    NOT NULL CHECK (replay_safety IN ('IDEMPOTENT','CONDITIONAL','NON_REPLAYABLE')),
  data_class              TEXT    NOT NULL CHECK (data_class IN ('PUBLIC','PERSONAL','PRIVATE','SECRET','CREDENTIAL')),
  root_requirement        TEXT    NOT NULL CHECK (root_requirement IN ('NOT_REQUIRED','OPTIONAL_ROOT','REQUIRES_ROOT')),
  idempotency_support     TEXT    NOT NULL CHECK (idempotency_support IN ('NATIVE','EMULATED','NONE')),
  max_duration_ms         INTEGER NOT NULL CHECK (max_duration_ms BETWEEN 0 AND 4294967295),
  cost_class              TEXT    NOT NULL CHECK (cost_class IN ('FREE','LOW','PAID')),
  experimental            INTEGER NOT NULL CHECK (experimental IN (0,1)),
  implementation_key      TEXT GENERATED ALWAYS AS (coalesce(implementation_id,'')) STORED,
  CHECK (provider_id = substr(capability_id,1,instr(capability_id,'.')-1)),
  UNIQUE (descriptor_digest,capability_id,capability_version,provider_id,implementation_key)
) STRICT;

CREATE TRIGGER capability_descriptor_revision_no_update
BEFORE UPDATE ON capability_descriptor_revisions
BEGIN SELECT RAISE(ABORT, 'descriptor revisions are immutable'); END;
CREATE TRIGGER capability_descriptor_revision_no_delete
BEFORE DELETE ON capability_descriptor_revisions
BEGIN SELECT RAISE(ABORT, 'descriptor revisions are immutable'); END;

CREATE TABLE capability_generation_members (
  generation_id      INTEGER NOT NULL REFERENCES capability_registry_generations(generation_id) ON DELETE RESTRICT,
  descriptor_digest  TEXT    NOT NULL REFERENCES capability_descriptor_revisions(descriptor_digest) ON DELETE RESTRICT,
  capability_id      TEXT    NOT NULL,
  capability_version TEXT    NOT NULL,
  provider_id        TEXT    NOT NULL,
  implementation_id TEXT,
  candidate_priority INTEGER NOT NULL CHECK (candidate_priority >= 0),
  implementation_key TEXT GENERATED ALWAYS AS (coalesce(implementation_id,'')) STORED,
  PRIMARY KEY (generation_id,descriptor_digest),
  UNIQUE (generation_id,capability_id,capability_version,implementation_key),
  UNIQUE (generation_id,capability_id,capability_version,candidate_priority),
  FOREIGN KEY(descriptor_digest,capability_id,capability_version,provider_id,implementation_key)
    REFERENCES capability_descriptor_revisions(descriptor_digest,capability_id,capability_version,provider_id,implementation_key)
    ON DELETE RESTRICT
) STRICT;

CREATE INDEX capability_generation_members_priority
  ON capability_generation_members(generation_id,capability_id,capability_version,candidate_priority,descriptor_digest);

CREATE TRIGGER capability_generation_member_is_prepared
BEFORE INSERT ON capability_generation_members
WHEN (SELECT activated_at_ms FROM capability_registry_generations WHERE generation_id=NEW.generation_id) IS NOT NULL
BEGIN SELECT RAISE(ABORT, 'activated generation membership is immutable'); END;
CREATE TRIGGER capability_generation_member_identity_unambiguous
BEFORE INSERT ON capability_generation_members
WHEN (NEW.implementation_id IS NULL AND EXISTS (
        SELECT 1 FROM capability_generation_members
         WHERE generation_id=NEW.generation_id AND capability_id=NEW.capability_id
           AND capability_version=NEW.capability_version))
  OR (NEW.implementation_id IS NOT NULL AND EXISTS (
        SELECT 1 FROM capability_generation_members
         WHERE generation_id=NEW.generation_id AND capability_id=NEW.capability_id
           AND capability_version=NEW.capability_version AND implementation_id IS NULL))
BEGIN SELECT RAISE(ABORT, 'ambiguous descriptor implementation identity'); END;
CREATE TRIGGER capability_generation_member_no_update
BEFORE UPDATE ON capability_generation_members
BEGIN SELECT RAISE(ABORT, 'generation membership is immutable'); END;
CREATE TRIGGER capability_generation_member_no_delete
BEFORE DELETE ON capability_generation_members
BEGIN SELECT RAISE(ABORT, 'generation membership is immutable'); END;

CREATE TABLE capability_generation_defaults (
  generation_id      INTEGER NOT NULL REFERENCES capability_registry_generations(generation_id) ON DELETE RESTRICT,
  capability_id      TEXT    NOT NULL CHECK (length(capability_id) BETWEEN 1 AND 99),
  capability_version TEXT    NOT NULL CHECK (length(capability_version) BETWEEN 5 AND 128),
  PRIMARY KEY (generation_id,capability_id)
) STRICT;

CREATE TRIGGER capability_generation_default_is_prepared
BEFORE INSERT ON capability_generation_defaults
WHEN (SELECT activated_at_ms FROM capability_registry_generations WHERE generation_id=NEW.generation_id) IS NOT NULL
  OR NOT EXISTS (SELECT 1 FROM capability_generation_members
                  WHERE generation_id=NEW.generation_id AND capability_id=NEW.capability_id
                    AND capability_version=NEW.capability_version)
BEGIN SELECT RAISE(ABORT, 'generation default must name a prepared member'); END;
CREATE TRIGGER capability_generation_default_no_update
BEFORE UPDATE ON capability_generation_defaults
BEGIN SELECT RAISE(ABORT, 'generation defaults are immutable'); END;
CREATE TRIGGER capability_generation_default_no_delete
BEFORE DELETE ON capability_generation_defaults
BEGIN SELECT RAISE(ABORT, 'generation defaults are immutable'); END;

CREATE TRIGGER capability_generation_activation_requires_complete_snapshot
BEFORE UPDATE OF activated_at_ms ON capability_registry_generations
WHEN NEW.activated_at_ms IS NOT NULL AND (
  NOT EXISTS (SELECT 1 FROM capability_generation_members WHERE generation_id=NEW.generation_id)
  OR EXISTS (SELECT 1 FROM capability_generation_members AS m
              WHERE m.generation_id=NEW.generation_id
                AND NOT EXISTS (SELECT 1 FROM capability_generation_defaults AS d
                                 WHERE d.generation_id=m.generation_id
                                   AND d.capability_id=m.capability_id)))
BEGIN SELECT RAISE(ABORT, 'generation requires membership and per-capability defaults'); END;

CREATE TABLE capability_overlays (
  capability_id       TEXT    PRIMARY KEY CHECK (length(capability_id) BETWEEN 1 AND 99),
  availability_state TEXT    NOT NULL CHECK (availability_state IN ('ENABLED','DISABLED','REMOVED')),
  experimental_opt_in INTEGER NOT NULL CHECK (experimental_opt_in IN (0,1)),
  revision            INTEGER NOT NULL CHECK (revision >= 1)
) STRICT;

CREATE TABLE step_capability_bindings (
  task_id             TEXT    NOT NULL REFERENCES tasks(task_id) ON DELETE CASCADE,
  step_id             TEXT    NOT NULL REFERENCES task_steps(step_id) ON DELETE CASCADE,
  generation_id       INTEGER NOT NULL REFERENCES capability_registry_generations(generation_id) ON DELETE RESTRICT,
  descriptor_digest   TEXT    NOT NULL,
  capability_id       TEXT    NOT NULL,
  capability_version  TEXT    NOT NULL,
  provider_id         TEXT    NOT NULL,
  implementation_id   TEXT,
  PRIMARY KEY (task_id,step_id),
  FOREIGN KEY(generation_id,descriptor_digest)
    REFERENCES capability_generation_members(generation_id,descriptor_digest) ON DELETE RESTRICT
) STRICT;
CREATE TRIGGER step_capability_binding_matches_task_step
BEFORE INSERT ON step_capability_bindings
WHEN (SELECT task_id FROM task_steps WHERE step_id=NEW.step_id) IS NOT NEW.task_id
  OR (SELECT kind FROM task_steps WHERE step_id=NEW.step_id) IS NOT 'CAPABILITY'
  OR (SELECT capability_registry_generation FROM tasks WHERE task_id=NEW.task_id) IS NOT NEW.generation_id
  OR (SELECT activated_at_ms FROM capability_registry_generations WHERE generation_id=NEW.generation_id) IS NULL
  OR NOT EXISTS (SELECT 1 FROM capability_generation_members AS m
                  WHERE m.generation_id=NEW.generation_id
                    AND m.descriptor_digest=NEW.descriptor_digest
                    AND m.capability_id=NEW.capability_id
                    AND m.capability_version=NEW.capability_version
                    AND m.provider_id=NEW.provider_id
                    AND m.implementation_id IS NEW.implementation_id)
BEGIN SELECT RAISE(ABORT, 'capability binding facts do not match task, step and generation'); END;
CREATE TRIGGER step_capability_binding_no_update
BEFORE UPDATE ON step_capability_bindings
BEGIN SELECT RAISE(ABORT, 'step capability bindings are immutable'); END;
CREATE TRIGGER step_capability_binding_no_delete
BEFORE DELETE ON step_capability_bindings
BEGIN SELECT RAISE(ABORT, 'step capability bindings are immutable'); END;

CREATE TRIGGER bound_task_step_identity_immutable
BEFORE UPDATE OF task_id,kind ON task_steps
WHEN EXISTS (SELECT 1 FROM step_capability_bindings WHERE step_id=OLD.step_id)
 AND (NEW.task_id IS NOT OLD.task_id OR NEW.kind IS NOT OLD.kind)
BEGIN SELECT RAISE(ABORT, 'bound capability step identity is immutable'); END;

CREATE TRIGGER capability_registry_state_requires_activated_generation
BEFORE UPDATE OF active_generation_id ON capability_registry_state
WHEN NEW.active_generation_id IS NOT NULL
  AND (SELECT activated_at_ms FROM capability_registry_generations WHERE generation_id=NEW.active_generation_id) IS NULL
BEGIN SELECT RAISE(ABORT, 'active registry generation must be activated'); END;

CREATE TRIGGER capability_registry_state_advances_only
BEFORE UPDATE OF active_generation_id ON capability_registry_state
WHEN (OLD.active_generation_id IS NOT NULL AND
      (NEW.active_generation_id IS NULL OR NEW.active_generation_id <= OLD.active_generation_id))
BEGIN SELECT RAISE(ABORT, 'active registry generation only advances'); END;

ALTER TABLE tasks
  ADD COLUMN capability_registry_generation INTEGER
  REFERENCES capability_registry_generations(generation_id) ON DELETE RESTRICT;

CREATE TRIGGER tasks_capability_registry_generation_immutable
BEFORE UPDATE OF capability_registry_generation ON tasks
WHEN OLD.capability_registry_generation IS NOT NULL
 AND NEW.capability_registry_generation IS NOT OLD.capability_registry_generation
BEGIN SELECT RAISE(ABORT, 'task registry generation pin is immutable'); END;

CREATE TRIGGER tasks_capability_registry_generation_activated
BEFORE UPDATE OF capability_registry_generation ON tasks
WHEN NEW.capability_registry_generation IS NOT NULL
 AND (SELECT activated_at_ms FROM capability_registry_generations
       WHERE generation_id=NEW.capability_registry_generation) IS NULL
BEGIN SELECT RAISE(ABORT, 'task registry generation must be activated'); END;
