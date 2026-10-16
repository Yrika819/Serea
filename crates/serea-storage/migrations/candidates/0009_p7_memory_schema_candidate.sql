-- Candidate 0009 for migration Option A. NOT ACCEPTED and intentionally excluded
-- from the ordinary migration catalog. This disposable schema candidate is
-- distinct from tests/fixtures/p7_experimental_memory_v9.sql and from production.
-- It applies accepted opaque OwnerId / EventId direction with a test SQL length
-- check only. OwnerId authentication/issuer, numeric bounds, retention, exact
-- provenance and Forget semantics remain proposed. Never apply to user DBs.
CREATE TABLE owners (
  owner_id TEXT PRIMARY KEY CHECK (length(owner_id) > 0),
  epoch INTEGER NOT NULL UNIQUE CHECK (epoch > 0),
  state TEXT NOT NULL CHECK (state IN ('ACTIVE','QUARANTINED','RETIRED')),
  created_at_ms INTEGER NOT NULL CHECK (created_at_ms BETWEEN -62167219200000 AND 253402300799999)
) STRICT;

CREATE TRIGGER owner_identity_immutable BEFORE UPDATE OF owner_id,epoch,created_at_ms ON owners
BEGIN SELECT RAISE(ABORT, 'P7 owner identity is immutable'); END;
CREATE TRIGGER owner_identity_no_delete BEFORE DELETE ON owners
BEGIN SELECT RAISE(ABORT, 'P7 owner identity is permanent'); END;

CREATE TABLE task_owner_bindings (
  task_id TEXT PRIMARY KEY CHECK (length(task_id) = 30 AND substr(task_id,1,4)='tsk_'),
  owner_id TEXT NOT NULL REFERENCES owners(owner_id) ON DELETE RESTRICT,
  admitted_at_ms INTEGER NOT NULL CHECK (admitted_at_ms BETWEEN -62167219200000 AND 253402300799999),
  UNIQUE(task_id, owner_id)
) STRICT;
CREATE TABLE legacy_task_quarantine (
  task_id TEXT PRIMARY KEY CHECK (length(task_id) = 30 AND substr(task_id,1,4)='tsk_'),
  reason_code TEXT NOT NULL CHECK (reason_code IN ('OWNER_UNPROVEN')),
  quarantined_at_ms INTEGER NOT NULL CHECK (quarantined_at_ms BETWEEN -62167219200000 AND 253402300799999)
) STRICT;
INSERT INTO legacy_task_quarantine(task_id,reason_code,quarantined_at_ms)
SELECT task_id,'OWNER_UNPROVEN',created_at_ms FROM tasks;
CREATE TRIGGER binding_requires_task BEFORE INSERT ON task_owner_bindings
WHEN NOT EXISTS (SELECT 1 FROM tasks WHERE task_id=NEW.task_id)
BEGIN SELECT RAISE(ABORT, 'P7 owner binding requires Task'); END;
CREATE TRIGGER unbound_task_quarantine AFTER INSERT ON tasks
WHEN NOT EXISTS (SELECT 1 FROM task_owner_bindings WHERE task_id=NEW.task_id)
BEGIN INSERT INTO legacy_task_quarantine(task_id,reason_code,quarantined_at_ms) VALUES(NEW.task_id,'OWNER_UNPROVEN',NEW.created_at_ms); END;
CREATE TRIGGER bind_task_clears_quarantine AFTER INSERT ON task_owner_bindings
BEGIN DELETE FROM legacy_task_quarantine WHERE task_id=NEW.task_id; END;
CREATE TRIGGER quarantine_immutable_update BEFORE UPDATE ON legacy_task_quarantine
BEGIN SELECT RAISE(ABORT, 'P7 legacy quarantine is immutable'); END;
CREATE TRIGGER quarantine_delete_requires_binding BEFORE DELETE ON legacy_task_quarantine
WHEN NOT EXISTS (SELECT 1 FROM task_owner_bindings WHERE task_id=OLD.task_id)
BEGIN SELECT RAISE(ABORT, 'P7 legacy quarantine requires authenticated binding'); END;
CREATE TRIGGER task_owner_binding_immutable_update BEFORE UPDATE ON task_owner_bindings
BEGIN SELECT RAISE(ABORT, 'P7 task owner binding is immutable'); END;
CREATE TRIGGER task_owner_binding_immutable_delete BEFORE DELETE ON task_owner_bindings
BEGIN SELECT RAISE(ABORT, 'P7 task owner binding is immutable'); END;

CREATE TABLE resource_accounting (
  owner_id TEXT PRIMARY KEY REFERENCES owners(owner_id) ON DELETE RESTRICT,
  active_items INTEGER NOT NULL DEFAULT 0 CHECK (active_items BETWEEN 0 AND 1000),
  retained_items INTEGER NOT NULL DEFAULT 0 CHECK (retained_items BETWEEN 0 AND 1000),
  active_body_bytes INTEGER NOT NULL DEFAULT 0 CHECK (active_body_bytes BETWEEN 0 AND 4194304),
  logical_fts_bytes INTEGER NOT NULL DEFAULT 0 CHECK (logical_fts_bytes BETWEEN 0 AND 8388608),
  extraction_starts_today INTEGER NOT NULL DEFAULT 0 CHECK (extraction_starts_today BETWEEN 0 AND 20),
  concurrent_extractions INTEGER NOT NULL DEFAULT 0 CHECK (concurrent_extractions BETWEEN 0 AND 1),
  utc_day TEXT NOT NULL DEFAULT ''
) STRICT;

CREATE TABLE memory_items (
  memory_id TEXT NOT NULL CHECK (length(memory_id)=30 AND substr(memory_id,1,4)='evt_' AND substr(memory_id,5,1)<='7'),
  owner_id TEXT NOT NULL REFERENCES owners(owner_id) ON DELETE RESTRICT,
  class_rank INTEGER NOT NULL CHECK (class_rank IN (0,1)),
  kind TEXT NOT NULL CHECK (kind IN ('WORKING','EPISODIC','SEMANTIC','PREFERENCE')),
  body TEXT NOT NULL CHECK (length(CAST(body AS BLOB)) BETWEEN 1 AND 4096),
  created_at_ms INTEGER NOT NULL CHECK (created_at_ms BETWEEN -62167219200000 AND 253402300799999),
  expires_at_ms INTEGER NOT NULL CHECK (expires_at_ms BETWEEN -62167219200000 AND 253402300799999),
  renewals INTEGER NOT NULL DEFAULT 0 CHECK (renewals BETWEEN 0 AND 2),
  active INTEGER NOT NULL DEFAULT 1 CHECK (active IN (0,1)),
  PRIMARY KEY(owner_id, memory_id),
  UNIQUE(memory_id, owner_id),
  CHECK (expires_at_ms > created_at_ms)
) STRICT;
CREATE INDEX memory_owner_expiry ON memory_items(owner_id, active, expires_at_ms, memory_id);
CREATE INDEX memory_owner_class ON memory_items(owner_id, class_rank, active, memory_id);

CREATE TABLE memory_provenance (
  memory_id TEXT NOT NULL,
  owner_id TEXT NOT NULL,
  source_id TEXT NOT NULL CHECK (length(CAST(source_id AS BLOB)) BETWEEN 1 AND 1024),
  source_task_id TEXT,
  source_class_rank INTEGER NOT NULL CHECK (source_class_rank IN (0,1)),
  metadata_bytes INTEGER NOT NULL CHECK (metadata_bytes BETWEEN 0 AND 4096),
  recorded_at_ms INTEGER NOT NULL CHECK (recorded_at_ms BETWEEN -62167219200000 AND 253402300799999),
  PRIMARY KEY(owner_id,memory_id),
  FOREIGN KEY(memory_id, owner_id) REFERENCES memory_items(memory_id, owner_id) ON DELETE CASCADE,
  CHECK (metadata_bytes + length(CAST(source_id AS BLOB)) + coalesce(length(CAST(source_task_id AS BLOB)),0) <= 4096)
) STRICT;
CREATE TRIGGER provenance_class_check BEFORE INSERT ON memory_provenance
WHEN NEW.source_class_rank > (SELECT class_rank FROM memory_items WHERE memory_id=NEW.memory_id AND owner_id=NEW.owner_id)
BEGIN SELECT RAISE(ABORT, 'P7 Memory below provenance classification'); END;
CREATE TRIGGER provenance_immutable_update BEFORE UPDATE ON memory_provenance
BEGIN SELECT RAISE(ABORT, 'P7 provenance is immutable'); END;
CREATE TRIGGER provenance_immutable_delete BEFORE DELETE ON memory_provenance
WHEN NOT EXISTS (SELECT 1 FROM memory_tombstones WHERE memory_id=OLD.memory_id AND owner_id=OLD.owner_id)
BEGIN SELECT RAISE(ABORT, 'P7 provenance deletion requires tombstone'); END;

CREATE TABLE supersession_edges (
  owner_id TEXT NOT NULL REFERENCES owners(owner_id) ON DELETE RESTRICT,
  prior_memory_id TEXT NOT NULL,
  successor_memory_id TEXT NOT NULL,
  created_at_ms INTEGER NOT NULL CHECK (created_at_ms BETWEEN -62167219200000 AND 253402300799999),
  PRIMARY KEY(owner_id, prior_memory_id),
  UNIQUE(owner_id, successor_memory_id),
  CHECK(prior_memory_id <> successor_memory_id),
  FOREIGN KEY(prior_memory_id, owner_id) REFERENCES memory_items(memory_id, owner_id) ON DELETE CASCADE,
  FOREIGN KEY(successor_memory_id, owner_id) REFERENCES memory_items(memory_id, owner_id) ON DELETE CASCADE
) STRICT;
CREATE TRIGGER supersession_no_cycle BEFORE INSERT ON supersession_edges
BEGIN
  SELECT CASE WHEN EXISTS (
    WITH RECURSIVE walk(id) AS (
      SELECT NEW.successor_memory_id
      UNION
      SELECT e.successor_memory_id FROM supersession_edges e JOIN walk w
        ON e.owner_id=NEW.owner_id AND e.prior_memory_id=w.id
    ) SELECT 1 FROM walk WHERE id=NEW.prior_memory_id
  ) THEN RAISE(ABORT, 'P7 supersession cycle') END;
END;
CREATE TRIGGER supersession_no_cycle_update BEFORE UPDATE ON supersession_edges
BEGIN SELECT RAISE(ABORT, 'P7 supersession edges are immutable'); END;
CREATE TRIGGER supersession_class_check BEFORE INSERT ON supersession_edges
WHEN (SELECT class_rank FROM memory_items WHERE memory_id=NEW.prior_memory_id AND owner_id=NEW.owner_id) >
     (SELECT class_rank FROM memory_items WHERE memory_id=NEW.successor_memory_id AND owner_id=NEW.owner_id)
BEGIN SELECT RAISE(ABORT, 'P7 supersession cannot lower classification'); END;

-- Synthetic derivation lineage is distinct from supersession and provenance.
-- Forget closure follows only this explicit relation (plus the selected roots).
CREATE TABLE memory_lineage (
  owner_id TEXT NOT NULL,
  parent_memory_id TEXT NOT NULL,
  derived_memory_id TEXT NOT NULL,
  created_at_ms INTEGER NOT NULL CHECK (created_at_ms BETWEEN -62167219200000 AND 253402300799999),
  PRIMARY KEY(owner_id,parent_memory_id,derived_memory_id),
  CHECK(parent_memory_id <> derived_memory_id),
  FOREIGN KEY(parent_memory_id,owner_id) REFERENCES memory_items(memory_id,owner_id),
  FOREIGN KEY(derived_memory_id,owner_id) REFERENCES memory_items(memory_id,owner_id)
) STRICT;
CREATE INDEX memory_lineage_child ON memory_lineage(owner_id,derived_memory_id);
CREATE TRIGGER memory_lineage_immutable_update BEFORE UPDATE ON memory_lineage
BEGIN SELECT RAISE(ABORT, 'P7 Memory lineage is immutable'); END;
CREATE TRIGGER memory_lineage_immutable_delete BEFORE DELETE ON memory_lineage
WHEN NOT EXISTS (SELECT 1 FROM memory_tombstones WHERE owner_id=OLD.owner_id AND memory_id=OLD.parent_memory_id)
  OR NOT EXISTS (SELECT 1 FROM memory_tombstones WHERE owner_id=OLD.owner_id AND memory_id=OLD.derived_memory_id)
BEGIN SELECT RAISE(ABORT, 'P7 Memory lineage is immutable'); END;
CREATE TRIGGER memory_lineage_no_cycle BEFORE INSERT ON memory_lineage
BEGIN
  SELECT CASE WHEN EXISTS (
    WITH RECURSIVE descendants(id) AS (
      SELECT derived_memory_id FROM memory_lineage
       WHERE owner_id=NEW.owner_id AND parent_memory_id=NEW.derived_memory_id
      UNION
      SELECT e.derived_memory_id FROM memory_lineage e JOIN descendants d
        ON e.owner_id=NEW.owner_id AND e.parent_memory_id=d.id
    ) SELECT 1 FROM descendants WHERE id=NEW.parent_memory_id
  ) THEN RAISE(ABORT, 'P7 Memory lineage cycle') END;
END;

CREATE TABLE memory_blob_refs (
  memory_id TEXT NOT NULL,
  owner_id TEXT NOT NULL,
  digest TEXT NOT NULL,
  data_class_rank INTEGER NOT NULL CHECK (data_class_rank IN (0,1)),
  PRIMARY KEY(owner_id, memory_id, digest, data_class_rank),
  FOREIGN KEY(memory_id, owner_id) REFERENCES memory_items(memory_id, owner_id) ON DELETE CASCADE,
  FOREIGN KEY(digest, data_class_rank) REFERENCES blobs(digest, data_class_rank) ON DELETE RESTRICT
) STRICT;
CREATE TRIGGER memory_blob_class_check BEFORE INSERT ON memory_blob_refs
WHEN NEW.data_class_rank > (SELECT class_rank FROM memory_items WHERE memory_id=NEW.memory_id AND owner_id=NEW.owner_id)
BEGIN SELECT RAISE(ABORT, 'P7 Memory below Blob classification'); END;
CREATE TRIGGER memory_blob_class_check_update BEFORE UPDATE ON memory_blob_refs
WHEN NEW.data_class_rank > (SELECT class_rank FROM memory_items WHERE memory_id=NEW.memory_id AND owner_id=NEW.owner_id)
BEGIN SELECT RAISE(ABORT, 'P7 Memory below Blob classification'); END;

CREATE VIEW memory_search_content AS
  SELECT rowid, memory_id, owner_id, class_rank, body FROM memory_items WHERE active=1;
CREATE VIRTUAL TABLE memory_fts USING fts5(
  memory_id UNINDEXED, owner_id UNINDEXED, class_rank UNINDEXED, body,
  content='memory_search_content', content_rowid='rowid', tokenize='unicode61'
);
CREATE TRIGGER memory_fts_insert AFTER INSERT ON memory_items WHEN NEW.active=1
BEGIN INSERT INTO memory_fts(rowid,memory_id,owner_id,class_rank,body) VALUES(NEW.rowid,NEW.memory_id,NEW.owner_id,NEW.class_rank,NEW.body); END;
CREATE TRIGGER memory_fts_delete AFTER DELETE ON memory_items WHEN OLD.active=1
BEGIN INSERT INTO memory_fts(memory_fts,rowid,memory_id,owner_id,class_rank,body) VALUES('delete',OLD.rowid,OLD.memory_id,OLD.owner_id,OLD.class_rank,OLD.body); END;
CREATE TRIGGER memory_fts_update AFTER UPDATE OF body,active,class_rank,owner_id ON memory_items
BEGIN
  INSERT INTO memory_fts(memory_fts,rowid,memory_id,owner_id,class_rank,body)
    SELECT 'delete',OLD.rowid,OLD.memory_id,OLD.owner_id,OLD.class_rank,OLD.body WHERE OLD.active=1;
  INSERT INTO memory_fts(rowid,memory_id,owner_id,class_rank,body)
    SELECT NEW.rowid,NEW.memory_id,NEW.owner_id,NEW.class_rank,NEW.body WHERE NEW.active=1;
END;

CREATE TABLE memory_tombstones (
  owner_id TEXT NOT NULL REFERENCES owners(owner_id) ON DELETE RESTRICT,
  memory_id TEXT NOT NULL CHECK (length(memory_id)=30 AND substr(memory_id,1,4)='evt_' AND substr(memory_id,5,1)<='7'),
  forget_op_id TEXT NOT NULL CHECK (length(forget_op_id) BETWEEN 1 AND 128),
  ledger_generation INTEGER NOT NULL CHECK (ledger_generation > 0),
  forgotten_at_ms INTEGER NOT NULL CHECK (forgotten_at_ms BETWEEN -62167219200000 AND 253402300799999),
  PRIMARY KEY(owner_id, memory_id),
  UNIQUE(owner_id, forget_op_id, memory_id)
) STRICT;
CREATE TRIGGER tombstone_immutable_update BEFORE UPDATE ON memory_tombstones
BEGIN SELECT RAISE(ABORT, 'P7 tombstone is immutable'); END;
CREATE TRIGGER tombstone_immutable_delete BEFORE DELETE ON memory_tombstones
BEGIN SELECT RAISE(ABORT, 'P7 tombstone is immutable'); END;
CREATE TRIGGER tombstone_content_free BEFORE INSERT ON memory_tombstones
WHEN length(NEW.memory_id)>128 OR length(NEW.forget_op_id)>128
BEGIN SELECT RAISE(ABORT, 'P7 tombstone bound'); END;

CREATE TABLE forget_operations (
  owner_id TEXT NOT NULL REFERENCES owners(owner_id) ON DELETE RESTRICT,
  forget_op_id TEXT NOT NULL,
  generation INTEGER NOT NULL CHECK (generation > 0),
  state TEXT NOT NULL CHECK (state IN ('AUTHORIZED','INTENT','PRIMARY_APPLIED','FINALIZED','ABORTED')),
  target_count INTEGER NOT NULL CHECK (target_count BETWEEN 0 AND 1000),
  created_at_ms INTEGER NOT NULL CHECK (created_at_ms BETWEEN -62167219200000 AND 253402300799999),
  PRIMARY KEY(owner_id, forget_op_id),
  UNIQUE(owner_id, generation)
) STRICT;
CREATE TABLE ledger_state (
  owner_id TEXT PRIMARY KEY REFERENCES owners(owner_id) ON DELETE RESTRICT,
  generation INTEGER NOT NULL DEFAULT 0 CHECK (generation >= 0),
  checkpoint_digest TEXT,
  read_state TEXT NOT NULL CHECK (read_state IN ('READY','QUARANTINED','RECOVERY_REQUIRED')),
  pending_operation_id TEXT,
  pending_generation INTEGER CHECK (pending_generation IS NULL OR pending_generation >= 0),
  CHECK ((pending_operation_id IS NULL) = (pending_generation IS NULL))
) STRICT;

-- All admission accounting is checked and updated in the same SQLite write
-- transaction as item/provenance/FTS insertion. The limits are provisional test bounds.
CREATE TRIGGER memory_admission BEFORE INSERT ON memory_items
BEGIN
  SELECT CASE WHEN EXISTS(SELECT 1 FROM memory_tombstones WHERE owner_id=NEW.owner_id AND memory_id=NEW.memory_id)
    THEN RAISE(ABORT, 'P7 forgotten Memory ID is permanently retired') END;
  SELECT CASE WHEN NOT EXISTS(SELECT 1 FROM resource_accounting WHERE owner_id=NEW.owner_id)
    THEN RAISE(ABORT, 'P7 quota row missing') END;
  SELECT CASE WHEN (SELECT retained_items FROM resource_accounting WHERE owner_id=NEW.owner_id) >= 1000
    THEN RAISE(ABORT, 'P7 retained item cap') END;
  SELECT CASE WHEN (SELECT active_items FROM resource_accounting WHERE owner_id=NEW.owner_id) >= 1000
    THEN RAISE(ABORT, 'P7 item quota') END;
  SELECT CASE WHEN (SELECT active_body_bytes FROM resource_accounting WHERE owner_id=NEW.owner_id) > 4194304-length(CAST(NEW.body AS BLOB))
    THEN RAISE(ABORT, 'P7 body quota') END;
  SELECT CASE WHEN (SELECT logical_fts_bytes FROM resource_accounting WHERE owner_id=NEW.owner_id) > 8388608-length(CAST(NEW.body AS BLOB))
    THEN RAISE(ABORT, 'P7 FTS quota') END;
END;
CREATE TRIGGER memory_account_insert AFTER INSERT ON memory_items
BEGIN
  UPDATE resource_accounting SET active_items=active_items+1,retained_items=retained_items+1,
    active_body_bytes=active_body_bytes+length(CAST(NEW.body AS BLOB)),
    logical_fts_bytes=logical_fts_bytes+length(CAST(NEW.body AS BLOB)) WHERE owner_id=NEW.owner_id;
END;
CREATE TRIGGER memory_account_delete AFTER DELETE ON memory_items
BEGIN
  UPDATE resource_accounting SET active_items=active_items-OLD.active,retained_items=retained_items-1,
    active_body_bytes=active_body_bytes-OLD.active*length(CAST(OLD.body AS BLOB)),
    logical_fts_bytes=logical_fts_bytes-OLD.active*length(CAST(OLD.body AS BLOB)) WHERE owner_id=OLD.owner_id;
END;
CREATE TRIGGER memory_owner_body_immutable BEFORE UPDATE OF owner_id,class_rank,kind,body,created_at_ms,expires_at_ms,renewals ON memory_items
BEGIN SELECT RAISE(ABORT, 'P7 memory identity, classification, content, and retention are immutable'); END;
CREATE TRIGGER memory_delete_requires_tombstone BEFORE DELETE ON memory_items
WHEN NOT EXISTS (SELECT 1 FROM memory_tombstones WHERE owner_id=OLD.owner_id AND memory_id=OLD.memory_id)
BEGIN SELECT RAISE(ABORT, 'P7 Memory deletion requires tombstone'); END;
CREATE TRIGGER memory_reactivation_denied BEFORE UPDATE OF active ON memory_items
WHEN OLD.active=0 AND NEW.active=1
BEGIN SELECT RAISE(ABORT, 'P7 superseded Memory cannot reactivate'); END;
CREATE TRIGGER memory_deactivate_requires_edge BEFORE UPDATE OF active ON memory_items
WHEN NEW.active=0 AND NOT EXISTS (
  SELECT 1 FROM supersession_edges e WHERE e.owner_id=OLD.owner_id AND e.prior_memory_id=OLD.memory_id
)
BEGIN SELECT RAISE(ABORT, 'P7 deactivation requires supersession edge'); END;
CREATE TRIGGER memory_account_update AFTER UPDATE OF active ON memory_items
WHEN OLD.active<>NEW.active
BEGIN
  UPDATE resource_accounting SET active_items=active_items+(NEW.active-OLD.active),
    active_body_bytes=active_body_bytes+(NEW.active-OLD.active)*length(CAST(NEW.body AS BLOB)),
    logical_fts_bytes=logical_fts_bytes+(NEW.active-OLD.active)*length(CAST(NEW.body AS BLOB)) WHERE owner_id=NEW.owner_id;
END;
