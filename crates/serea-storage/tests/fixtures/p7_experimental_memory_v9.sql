-- P7 synthetic prototype only. Numeric checks implement unratified provisional
-- bounds for isolated testing; they are not production policy approval.
CREATE TABLE p7_owners (
  owner_id TEXT PRIMARY KEY CHECK (
    length(owner_id) = 36 AND substr(owner_id,1,4) = 'own_' AND
    substr(owner_id,5) NOT GLOB '*[^0-9a-f]*'
  ),
  epoch INTEGER NOT NULL UNIQUE CHECK (epoch > 0),
  state TEXT NOT NULL CHECK (state IN ('ACTIVE','QUARANTINED','RETIRED')),
  created_at_ms INTEGER NOT NULL CHECK (created_at_ms BETWEEN -62167219200000 AND 253402300799999)
) STRICT;

CREATE TRIGGER p7_owner_identity_immutable BEFORE UPDATE OF owner_id,epoch,created_at_ms ON p7_owners
BEGIN SELECT RAISE(ABORT, 'P7 owner identity is immutable'); END;
CREATE TRIGGER p7_owner_identity_no_delete BEFORE DELETE ON p7_owners
BEGIN SELECT RAISE(ABORT, 'P7 owner identity is permanent'); END;

CREATE TABLE p7_task_owner_bindings (
  task_id TEXT PRIMARY KEY CHECK (length(task_id) = 30 AND substr(task_id,1,4)='tsk_'),
  owner_id TEXT NOT NULL REFERENCES p7_owners(owner_id) ON DELETE RESTRICT,
  admitted_at_ms INTEGER NOT NULL CHECK (admitted_at_ms BETWEEN -62167219200000 AND 253402300799999),
  UNIQUE(task_id, owner_id)
) STRICT;
CREATE TABLE p7_legacy_task_quarantine (
  task_id TEXT PRIMARY KEY CHECK (length(task_id) = 30 AND substr(task_id,1,4)='tsk_'),
  reason_code TEXT NOT NULL CHECK (reason_code IN ('OWNER_UNPROVEN')),
  quarantined_at_ms INTEGER NOT NULL CHECK (quarantined_at_ms BETWEEN -62167219200000 AND 253402300799999)
) STRICT;
INSERT INTO p7_legacy_task_quarantine(task_id,reason_code,quarantined_at_ms)
SELECT task_id,'OWNER_UNPROVEN',created_at_ms FROM tasks;
CREATE TRIGGER p7_binding_requires_task BEFORE INSERT ON p7_task_owner_bindings
WHEN NOT EXISTS (SELECT 1 FROM tasks WHERE task_id=NEW.task_id)
BEGIN SELECT RAISE(ABORT, 'P7 owner binding requires Task'); END;
CREATE TRIGGER p7_unbound_task_quarantine AFTER INSERT ON tasks
WHEN NOT EXISTS (SELECT 1 FROM p7_task_owner_bindings WHERE task_id=NEW.task_id)
BEGIN INSERT INTO p7_legacy_task_quarantine(task_id,reason_code,quarantined_at_ms) VALUES(NEW.task_id,'OWNER_UNPROVEN',NEW.created_at_ms); END;
CREATE TRIGGER p7_bind_task_clears_quarantine AFTER INSERT ON p7_task_owner_bindings
BEGIN DELETE FROM p7_legacy_task_quarantine WHERE task_id=NEW.task_id; END;
CREATE TRIGGER p7_quarantine_immutable_update BEFORE UPDATE ON p7_legacy_task_quarantine
BEGIN SELECT RAISE(ABORT, 'P7 legacy quarantine is immutable'); END;
CREATE TRIGGER p7_quarantine_delete_requires_binding BEFORE DELETE ON p7_legacy_task_quarantine
WHEN NOT EXISTS (SELECT 1 FROM p7_task_owner_bindings WHERE task_id=OLD.task_id)
BEGIN SELECT RAISE(ABORT, 'P7 legacy quarantine requires authenticated binding'); END;
CREATE TRIGGER p7_task_owner_binding_immutable_update BEFORE UPDATE ON p7_task_owner_bindings
BEGIN SELECT RAISE(ABORT, 'P7 task owner binding is immutable'); END;
CREATE TRIGGER p7_task_owner_binding_immutable_delete BEFORE DELETE ON p7_task_owner_bindings
BEGIN SELECT RAISE(ABORT, 'P7 task owner binding is immutable'); END;

CREATE TABLE p7_resource_accounting (
  owner_id TEXT PRIMARY KEY REFERENCES p7_owners(owner_id) ON DELETE RESTRICT,
  active_items INTEGER NOT NULL DEFAULT 0 CHECK (active_items BETWEEN 0 AND 1000),
  retained_items INTEGER NOT NULL DEFAULT 0 CHECK (retained_items BETWEEN 0 AND 1000),
  active_body_bytes INTEGER NOT NULL DEFAULT 0 CHECK (active_body_bytes BETWEEN 0 AND 4194304),
  logical_fts_bytes INTEGER NOT NULL DEFAULT 0 CHECK (logical_fts_bytes BETWEEN 0 AND 8388608),
  extraction_starts_today INTEGER NOT NULL DEFAULT 0 CHECK (extraction_starts_today BETWEEN 0 AND 20),
  concurrent_extractions INTEGER NOT NULL DEFAULT 0 CHECK (concurrent_extractions BETWEEN 0 AND 1),
  utc_day TEXT NOT NULL DEFAULT ''
) STRICT;

CREATE TABLE p7_memory_items (
  memory_id TEXT PRIMARY KEY CHECK (length(memory_id) BETWEEN 1 AND 128),
  owner_id TEXT NOT NULL REFERENCES p7_owners(owner_id) ON DELETE RESTRICT,
  class_rank INTEGER NOT NULL CHECK (class_rank IN (0,1)),
  kind TEXT NOT NULL CHECK (kind IN ('WORKING','EPISODIC','SEMANTIC','PREFERENCE')),
  body TEXT NOT NULL CHECK (length(CAST(body AS BLOB)) BETWEEN 1 AND 4096),
  created_at_ms INTEGER NOT NULL CHECK (created_at_ms BETWEEN -62167219200000 AND 253402300799999),
  expires_at_ms INTEGER NOT NULL CHECK (expires_at_ms BETWEEN -62167219200000 AND 253402300799999),
  renewals INTEGER NOT NULL DEFAULT 0 CHECK (renewals BETWEEN 0 AND 2),
  active INTEGER NOT NULL DEFAULT 1 CHECK (active IN (0,1)),
  UNIQUE(memory_id, owner_id),
  CHECK (expires_at_ms > created_at_ms)
) STRICT;
CREATE INDEX p7_memory_owner_expiry ON p7_memory_items(owner_id, active, expires_at_ms, memory_id);
CREATE INDEX p7_memory_owner_class ON p7_memory_items(owner_id, class_rank, active, memory_id);

CREATE TABLE p7_memory_provenance (
  memory_id TEXT PRIMARY KEY,
  owner_id TEXT NOT NULL,
  source_id TEXT NOT NULL CHECK (length(CAST(source_id AS BLOB)) BETWEEN 1 AND 1024),
  source_task_id TEXT,
  source_class_rank INTEGER NOT NULL CHECK (source_class_rank IN (0,1)),
  metadata_bytes INTEGER NOT NULL CHECK (metadata_bytes BETWEEN 0 AND 4096),
  recorded_at_ms INTEGER NOT NULL CHECK (recorded_at_ms BETWEEN -62167219200000 AND 253402300799999),
  FOREIGN KEY(memory_id, owner_id) REFERENCES p7_memory_items(memory_id, owner_id) ON DELETE CASCADE,
  CHECK (metadata_bytes + length(CAST(source_id AS BLOB)) + coalesce(length(CAST(source_task_id AS BLOB)),0) <= 4096)
) STRICT;
CREATE TRIGGER p7_provenance_class_check BEFORE INSERT ON p7_memory_provenance
WHEN NEW.source_class_rank > (SELECT class_rank FROM p7_memory_items WHERE memory_id=NEW.memory_id AND owner_id=NEW.owner_id)
BEGIN SELECT RAISE(ABORT, 'P7 Memory below provenance classification'); END;
CREATE TRIGGER p7_provenance_immutable_update BEFORE UPDATE ON p7_memory_provenance
BEGIN SELECT RAISE(ABORT, 'P7 provenance is immutable'); END;
CREATE TRIGGER p7_provenance_immutable_delete BEFORE DELETE ON p7_memory_provenance
WHEN NOT EXISTS (SELECT 1 FROM p7_memory_tombstones WHERE memory_id=OLD.memory_id AND owner_id=OLD.owner_id)
BEGIN SELECT RAISE(ABORT, 'P7 provenance deletion requires tombstone'); END;

CREATE TABLE p7_supersession_edges (
  owner_id TEXT NOT NULL REFERENCES p7_owners(owner_id) ON DELETE RESTRICT,
  prior_memory_id TEXT NOT NULL,
  successor_memory_id TEXT NOT NULL,
  created_at_ms INTEGER NOT NULL CHECK (created_at_ms BETWEEN -62167219200000 AND 253402300799999),
  PRIMARY KEY(owner_id, prior_memory_id),
  UNIQUE(owner_id, successor_memory_id),
  CHECK(prior_memory_id <> successor_memory_id),
  FOREIGN KEY(prior_memory_id, owner_id) REFERENCES p7_memory_items(memory_id, owner_id) ON DELETE CASCADE,
  FOREIGN KEY(successor_memory_id, owner_id) REFERENCES p7_memory_items(memory_id, owner_id) ON DELETE CASCADE
) STRICT;
CREATE TRIGGER p7_supersession_no_cycle BEFORE INSERT ON p7_supersession_edges
BEGIN
  SELECT CASE WHEN EXISTS (
    WITH RECURSIVE walk(id) AS (
      SELECT NEW.successor_memory_id
      UNION
      SELECT e.successor_memory_id FROM p7_supersession_edges e JOIN walk w
        ON e.owner_id=NEW.owner_id AND e.prior_memory_id=w.id
    ) SELECT 1 FROM walk WHERE id=NEW.prior_memory_id
  ) THEN RAISE(ABORT, 'P7 supersession cycle') END;
END;
CREATE TRIGGER p7_supersession_no_cycle_update BEFORE UPDATE ON p7_supersession_edges
BEGIN SELECT RAISE(ABORT, 'P7 supersession edges are immutable'); END;
CREATE TRIGGER p7_supersession_class_check BEFORE INSERT ON p7_supersession_edges
WHEN (SELECT class_rank FROM p7_memory_items WHERE memory_id=NEW.prior_memory_id AND owner_id=NEW.owner_id) >
     (SELECT class_rank FROM p7_memory_items WHERE memory_id=NEW.successor_memory_id AND owner_id=NEW.owner_id)
BEGIN SELECT RAISE(ABORT, 'P7 supersession cannot lower classification'); END;

-- Synthetic derivation lineage is distinct from supersession and provenance.
-- Forget closure follows only this explicit relation (plus the selected roots).
CREATE TABLE p7_memory_lineage (
  owner_id TEXT NOT NULL,
  parent_memory_id TEXT NOT NULL,
  derived_memory_id TEXT NOT NULL,
  created_at_ms INTEGER NOT NULL CHECK (created_at_ms BETWEEN -62167219200000 AND 253402300799999),
  PRIMARY KEY(owner_id,parent_memory_id,derived_memory_id),
  CHECK(parent_memory_id <> derived_memory_id),
  FOREIGN KEY(parent_memory_id,owner_id) REFERENCES p7_memory_items(memory_id,owner_id),
  FOREIGN KEY(derived_memory_id,owner_id) REFERENCES p7_memory_items(memory_id,owner_id)
) STRICT;
CREATE INDEX p7_memory_lineage_child ON p7_memory_lineage(owner_id,derived_memory_id);
CREATE TRIGGER p7_memory_lineage_immutable_update BEFORE UPDATE ON p7_memory_lineage
BEGIN SELECT RAISE(ABORT, 'P7 Memory lineage is immutable'); END;
CREATE TRIGGER p7_memory_lineage_immutable_delete BEFORE DELETE ON p7_memory_lineage
WHEN NOT EXISTS (SELECT 1 FROM p7_memory_tombstones WHERE owner_id=OLD.owner_id AND memory_id=OLD.parent_memory_id)
  OR NOT EXISTS (SELECT 1 FROM p7_memory_tombstones WHERE owner_id=OLD.owner_id AND memory_id=OLD.derived_memory_id)
BEGIN SELECT RAISE(ABORT, 'P7 Memory lineage is immutable'); END;
CREATE TRIGGER p7_memory_lineage_no_cycle BEFORE INSERT ON p7_memory_lineage
BEGIN
  SELECT CASE WHEN EXISTS (
    WITH RECURSIVE descendants(id) AS (
      SELECT derived_memory_id FROM p7_memory_lineage
       WHERE owner_id=NEW.owner_id AND parent_memory_id=NEW.derived_memory_id
      UNION
      SELECT e.derived_memory_id FROM p7_memory_lineage e JOIN descendants d
        ON e.owner_id=NEW.owner_id AND e.parent_memory_id=d.id
    ) SELECT 1 FROM descendants WHERE id=NEW.parent_memory_id
  ) THEN RAISE(ABORT, 'P7 Memory lineage cycle') END;
END;

CREATE TABLE p7_memory_blob_refs (
  memory_id TEXT NOT NULL,
  owner_id TEXT NOT NULL,
  digest TEXT NOT NULL,
  data_class_rank INTEGER NOT NULL CHECK (data_class_rank IN (0,1)),
  PRIMARY KEY(memory_id, digest, data_class_rank),
  FOREIGN KEY(memory_id, owner_id) REFERENCES p7_memory_items(memory_id, owner_id) ON DELETE CASCADE,
  FOREIGN KEY(digest, data_class_rank) REFERENCES blobs(digest, data_class_rank) ON DELETE RESTRICT
) STRICT;
CREATE TRIGGER p7_memory_blob_class_check BEFORE INSERT ON p7_memory_blob_refs
WHEN NEW.data_class_rank > (SELECT class_rank FROM p7_memory_items WHERE memory_id=NEW.memory_id AND owner_id=NEW.owner_id)
BEGIN SELECT RAISE(ABORT, 'P7 Memory below Blob classification'); END;
CREATE TRIGGER p7_memory_blob_class_check_update BEFORE UPDATE ON p7_memory_blob_refs
WHEN NEW.data_class_rank > (SELECT class_rank FROM p7_memory_items WHERE memory_id=NEW.memory_id AND owner_id=NEW.owner_id)
BEGIN SELECT RAISE(ABORT, 'P7 Memory below Blob classification'); END;

CREATE VIEW p7_memory_search_content AS
  SELECT rowid, memory_id, owner_id, class_rank, body FROM p7_memory_items WHERE active=1;
CREATE VIRTUAL TABLE p7_memory_fts USING fts5(
  memory_id UNINDEXED, owner_id UNINDEXED, class_rank UNINDEXED, body,
  content='p7_memory_search_content', content_rowid='rowid', tokenize='unicode61'
);
CREATE TRIGGER p7_memory_fts_insert AFTER INSERT ON p7_memory_items WHEN NEW.active=1
BEGIN INSERT INTO p7_memory_fts(rowid,memory_id,owner_id,class_rank,body) VALUES(NEW.rowid,NEW.memory_id,NEW.owner_id,NEW.class_rank,NEW.body); END;
CREATE TRIGGER p7_memory_fts_delete AFTER DELETE ON p7_memory_items WHEN OLD.active=1
BEGIN INSERT INTO p7_memory_fts(p7_memory_fts,rowid,memory_id,owner_id,class_rank,body) VALUES('delete',OLD.rowid,OLD.memory_id,OLD.owner_id,OLD.class_rank,OLD.body); END;
CREATE TRIGGER p7_memory_fts_update AFTER UPDATE OF body,active,class_rank,owner_id ON p7_memory_items
BEGIN
  INSERT INTO p7_memory_fts(p7_memory_fts,rowid,memory_id,owner_id,class_rank,body)
    SELECT 'delete',OLD.rowid,OLD.memory_id,OLD.owner_id,OLD.class_rank,OLD.body WHERE OLD.active=1;
  INSERT INTO p7_memory_fts(rowid,memory_id,owner_id,class_rank,body)
    SELECT NEW.rowid,NEW.memory_id,NEW.owner_id,NEW.class_rank,NEW.body WHERE NEW.active=1;
END;

CREATE TABLE p7_memory_tombstones (
  owner_id TEXT NOT NULL REFERENCES p7_owners(owner_id) ON DELETE RESTRICT,
  memory_id TEXT NOT NULL CHECK (length(memory_id) BETWEEN 1 AND 128),
  forget_op_id TEXT NOT NULL CHECK (length(forget_op_id) BETWEEN 1 AND 128),
  ledger_generation INTEGER NOT NULL CHECK (ledger_generation > 0),
  forgotten_at_ms INTEGER NOT NULL CHECK (forgotten_at_ms BETWEEN -62167219200000 AND 253402300799999),
  PRIMARY KEY(owner_id, memory_id),
  UNIQUE(owner_id, forget_op_id, memory_id)
) STRICT;
CREATE TRIGGER p7_tombstone_immutable_update BEFORE UPDATE ON p7_memory_tombstones
BEGIN SELECT RAISE(ABORT, 'P7 tombstone is immutable'); END;
CREATE TRIGGER p7_tombstone_immutable_delete BEFORE DELETE ON p7_memory_tombstones
BEGIN SELECT RAISE(ABORT, 'P7 tombstone is immutable'); END;
CREATE TRIGGER p7_tombstone_content_free BEFORE INSERT ON p7_memory_tombstones
WHEN length(NEW.memory_id)>128 OR length(NEW.forget_op_id)>128
BEGIN SELECT RAISE(ABORT, 'P7 tombstone bound'); END;

CREATE TABLE p7_forget_operations (
  owner_id TEXT NOT NULL REFERENCES p7_owners(owner_id) ON DELETE RESTRICT,
  forget_op_id TEXT NOT NULL,
  generation INTEGER NOT NULL CHECK (generation > 0),
  state TEXT NOT NULL CHECK (state IN ('AUTHORIZED','INTENT','PRIMARY_APPLIED','FINALIZED','ABORTED')),
  target_count INTEGER NOT NULL CHECK (target_count BETWEEN 0 AND 1000),
  created_at_ms INTEGER NOT NULL CHECK (created_at_ms BETWEEN -62167219200000 AND 253402300799999),
  PRIMARY KEY(owner_id, forget_op_id),
  UNIQUE(owner_id, generation)
) STRICT;
CREATE TABLE p7_ledger_state (
  owner_id TEXT PRIMARY KEY REFERENCES p7_owners(owner_id) ON DELETE RESTRICT,
  generation INTEGER NOT NULL DEFAULT 0 CHECK (generation >= 0),
  checkpoint_digest TEXT,
  read_state TEXT NOT NULL CHECK (read_state IN ('READY','QUARANTINED','RECOVERY_REQUIRED')),
  pending_operation_id TEXT,
  pending_generation INTEGER CHECK (pending_generation IS NULL OR pending_generation >= 0),
  CHECK ((pending_operation_id IS NULL) = (pending_generation IS NULL))
) STRICT;

-- All admission accounting is checked and updated in the same SQLite write
-- transaction as item/provenance/FTS insertion. The limits are provisional test bounds.
CREATE TRIGGER p7_memory_admission BEFORE INSERT ON p7_memory_items
BEGIN
  SELECT CASE WHEN NOT EXISTS(SELECT 1 FROM p7_resource_accounting WHERE owner_id=NEW.owner_id)
    THEN RAISE(ABORT, 'P7 quota row missing') END;
  SELECT CASE WHEN (SELECT retained_items FROM p7_resource_accounting WHERE owner_id=NEW.owner_id) >= 1000
    THEN RAISE(ABORT, 'P7 retained item cap') END;
  SELECT CASE WHEN (SELECT active_items FROM p7_resource_accounting WHERE owner_id=NEW.owner_id) >= 1000
    THEN RAISE(ABORT, 'P7 item quota') END;
  SELECT CASE WHEN (SELECT active_body_bytes FROM p7_resource_accounting WHERE owner_id=NEW.owner_id) > 4194304-length(CAST(NEW.body AS BLOB))
    THEN RAISE(ABORT, 'P7 body quota') END;
  SELECT CASE WHEN (SELECT logical_fts_bytes FROM p7_resource_accounting WHERE owner_id=NEW.owner_id) > 8388608-length(CAST(NEW.body AS BLOB))
    THEN RAISE(ABORT, 'P7 FTS quota') END;
END;
CREATE TRIGGER p7_memory_account_insert AFTER INSERT ON p7_memory_items
BEGIN
  UPDATE p7_resource_accounting SET active_items=active_items+1,retained_items=retained_items+1,
    active_body_bytes=active_body_bytes+length(CAST(NEW.body AS BLOB)),
    logical_fts_bytes=logical_fts_bytes+length(CAST(NEW.body AS BLOB)) WHERE owner_id=NEW.owner_id;
END;
CREATE TRIGGER p7_memory_account_delete AFTER DELETE ON p7_memory_items
BEGIN
  UPDATE p7_resource_accounting SET active_items=active_items-OLD.active,retained_items=retained_items-1,
    active_body_bytes=active_body_bytes-OLD.active*length(CAST(OLD.body AS BLOB)),
    logical_fts_bytes=logical_fts_bytes-OLD.active*length(CAST(OLD.body AS BLOB)) WHERE owner_id=OLD.owner_id;
END;
CREATE TRIGGER p7_memory_owner_body_immutable BEFORE UPDATE OF owner_id,class_rank,kind,body,created_at_ms,expires_at_ms,renewals ON p7_memory_items
BEGIN SELECT RAISE(ABORT, 'P7 memory identity, classification, content, and retention are immutable'); END;
CREATE TRIGGER p7_memory_delete_requires_tombstone BEFORE DELETE ON p7_memory_items
WHEN NOT EXISTS (SELECT 1 FROM p7_memory_tombstones WHERE owner_id=OLD.owner_id AND memory_id=OLD.memory_id)
BEGIN SELECT RAISE(ABORT, 'P7 Memory deletion requires tombstone'); END;
CREATE TRIGGER p7_memory_reactivation_denied BEFORE UPDATE OF active ON p7_memory_items
WHEN OLD.active=0 AND NEW.active=1
BEGIN SELECT RAISE(ABORT, 'P7 superseded Memory cannot reactivate'); END;
CREATE TRIGGER p7_memory_deactivate_requires_edge BEFORE UPDATE OF active ON p7_memory_items
WHEN NEW.active=0 AND NOT EXISTS (
  SELECT 1 FROM p7_supersession_edges e WHERE e.owner_id=OLD.owner_id AND e.prior_memory_id=OLD.memory_id
)
BEGIN SELECT RAISE(ABORT, 'P7 deactivation requires supersession edge'); END;
CREATE TRIGGER p7_memory_account_update AFTER UPDATE OF active ON p7_memory_items
WHEN OLD.active<>NEW.active
BEGIN
  UPDATE p7_resource_accounting SET active_items=active_items+(NEW.active-OLD.active),
    active_body_bytes=active_body_bytes+(NEW.active-OLD.active)*length(CAST(NEW.body AS BLOB)),
    logical_fts_bytes=logical_fts_bytes+(NEW.active-OLD.active)*length(CAST(NEW.body AS BLOB)) WHERE owner_id=NEW.owner_id;
END;
