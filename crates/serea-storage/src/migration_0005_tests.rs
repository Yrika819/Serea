//! P6B RED-first durable policy and approval contract tests.
//!
//! Every obligation in the P6B closure list is asserted against real SQLite
//! authority, not against a Rust mirror. Nothing here proves a physical
//! power-loss guarantee: the crash cases are transaction-boundary faults plus
//! a reopen, and the note is repeated where it applies.

use crate::{
    ApprovalAutomationContext, ApprovalLifecycleWake, ApprovalRequestActionDraft,
    ApprovalRequestStorageDraft, ApprovalResponseDecisionStorage, ApprovalResponseStorageDraft,
    ApprovalResponseStorageOutcome, ApprovalRevocationOutcome, ApprovalRevocationStorageDraft,
    DurableTransition, EventDraft, EventParticipant, GrantConsumptionOutcome, JournalKind,
    JournalRecord, JournalRecords, Migrations, Store, StoreError, TaskAuditParticipant,
    TransitionContext, Tx, approval_action_set_digest,
};
use rusqlite::{Connection, TransactionBehavior, params};
use serde_json::json;
use serea_protocol::{
    Actor, ActorId, ActorKind, ApprovalId, Authorization, CapabilityId, Clock, DataClass, Digest,
    EnvelopeVersion, EpochMillis, EventId, EventKind, GrantId, LeaseOwner, ProtocolError,
    RequestedBy, RiskClass, SemVer, Seq, SereaEvent, SideEffectClass, StepId, TaskId, TaskKind,
    TaskOriginKind, TaskState, Timestamp, Trace, WireSurface, digest_of,
};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Barrier};

static NEXT: AtomicU64 = AtomicU64::new(0);

struct FixedClock;

impl Clock for FixedClock {
    fn now_ms(&self) -> Result<EpochMillis, ProtocolError> {
        EpochMillis::new(1_796_000_000_000)
    }
}

struct TempDb(PathBuf);

impl TempDb {
    fn new() -> Self {
        let dir = std::env::temp_dir().join(format!(
            "serea-p6b-migration-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&dir).unwrap();
        Self(dir.join("store.sqlite"))
    }
}

impl Drop for TempDb {
    fn drop(&mut self) {
        std::fs::remove_dir_all(self.0.parent().unwrap()).unwrap();
    }
}

/// Deterministic representative digests and identities.
const MANIFEST: &str = "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const SCHEMA_CATALOG: &str =
    "sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
const DESCRIPTOR: &str = "sha256:ccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc1";
const RULES: &str = "sha256:dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd";
const ARGS_A: &str = "sha256:eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee1";
const ARGS_B: &str = "sha256:eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee2";
const SCOPE: &str = r#"{"calendar":"primary"}"#;
const SCOPE_DIGEST: &str =
    "sha256:fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff1";
const BAD: &str = "sha256:0000000000000000000000000000000000000000000000000000000000000000";

const TASK: &str = "tsk_01JQ8Z9K3M7QWXR4V2T6YH0BNA";
const STEP_A: &str = "stp_01JQ8Z9M5T9WXK2H4BNPQ7RDSF";
const STEP_B: &str = "stp_01JQ8Z9M5T9WXK2H4BNPQ7RDST";
const STEP_C: &str = "stp_01JQ8Z9M5T9WXK2H4BNPQ7RDSA";
const MODEL_STEP: &str = "stp_01JQ8Z9M5T9WXK2H4BNPQ7RDSB";
const OTHER_TASK: &str = "tsk_01JQ8Z9K3M7QWXR4V2T6YH0BNB";
const OTHER_STEP: &str = "stp_01JQ8Z9M5T9WXK2H4BNPQ7RDTU";
const APPROVAL: &str = "apr_01JQ8ZA1D4NFG8K2M6RTV9XCWB";
const APPROVAL_2: &str = "apr_01JQ8ZA1D4NFG8K2M6RTV9XCWC";
const GRANT_2: &str = "grt_01JQ8ZA7B3KMW9Q4TVY7XN2RDQ";
const GRANT: &str = "grt_01JQ8ZA7B3KMW9Q4TVY7XN2RDP";

fn seed_v4(path: &Path) {
    let conn = Connection::open(path).unwrap();
    conn.pragma_update(None, "foreign_keys", "ON").unwrap();
    for migration in Migrations::embedded().iter().take(4) {
        conn.execute_batch(migration.sql).unwrap();
        conn.execute(
            "INSERT INTO schema_migrations(version,name,checksum,applied_at_ms) VALUES (?1,?2,?3,0)",
            params![
                migration.version,
                migration.name,
                Migrations::checksum(migration.sql).as_str()
            ],
        )
        .unwrap();
    }
}

fn seed_v5(path: &Path) {
    let conn = Connection::open(path).unwrap();
    conn.pragma_update(None, "foreign_keys", "ON").unwrap();
    for migration in Migrations::embedded().iter().take(5) {
        conn.execute_batch(migration.sql).unwrap();
        conn.execute(
            "INSERT INTO schema_migrations(version,name,checksum,applied_at_ms) VALUES (?1,?2,?3,0)",
            params![
                migration.version,
                migration.name,
                Migrations::checksum(migration.sql).as_str()
            ],
        )
        .unwrap();
    }
}

/// A P5-shaped registry generation, descriptor revision, membership, default,
/// task, step and step binding, all of which an approval action must match.
fn seed_registry_and_task(conn: &Connection) {
    conn.execute(
        "INSERT INTO capability_registry_generations(generation_id,manifest_digest,schema_catalog_digest)
         VALUES (1,?1,?2)",
        params![MANIFEST, SCHEMA_CATALOG],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO capability_descriptor_revisions
           (descriptor_digest,capability_id,capability_version,provider_id,implementation_id,
            title,description,input_schema_uri,input_schema_digest,output_schema_uri,
            output_schema_digest,side_effect_class,risk_class,required_authorization,
            replay_safety,data_class,root_requirement,idempotency_support,max_duration_ms,
            cost_class,experimental)
         VALUES (?1,'calendar.event.create','1.0.0','calendar',NULL,'t','d','serea://i',?2,
                 'serea://o',?3,'EXTERNAL_WRITE','EXTERNAL_WRITE','SCOPED_GRANT','IDEMPOTENT',
                 'PERSONAL','NOT_REQUIRED','NATIVE',5000,'FREE',0)",
        params![DESCRIPTOR, BAD, BAD],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO capability_generation_members
           (generation_id,descriptor_digest,capability_id,capability_version,provider_id,
            implementation_id,candidate_priority)
         VALUES (1,?1,'calendar.event.create','1.0.0','calendar',NULL,0)",
        params![DESCRIPTOR],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO capability_generation_defaults(generation_id,capability_id,capability_version)
         VALUES (1,'calendar.event.create','1.0.0')",
        [],
    )
    .unwrap();
    conn.execute(
        "UPDATE capability_registry_generations SET activated_at_ms=1000 WHERE generation_id=1",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO tasks(task_id,kind,title,state,origin_kind,data_class_rank,policy_class_rank,
                           created_at_ms,updated_at_ms,max_model_calls,max_tool_calls,
                           max_attempts_per_step,capability_registry_generation)
         VALUES (?1,'USER_REQUEST','t','EXECUTING','USER_MESSAGE',1,3,0,1,12,24,3,1)",
        params![TASK],
    )
    .unwrap();
    for (index, (step, digest)) in [(STEP_A, ARGS_A), (STEP_B, ARGS_B), (STEP_C, ARGS_A)]
        .into_iter()
        .enumerate()
    {
        // A valid 68-character IDK-1 body: `idk_` plus 64 lowercase hex.
        let idk = format!("idk_{:064x}", index + 1);
        // task_steps carries UNIQUE (task_id, sequence).
        let sequence = index as i64;
        conn.execute(
            "INSERT INTO task_steps(step_id,task_id,sequence,kind,status,attempt,plan_revision,
                                    provider_id,capability_id,capability_version,input_digest,
                                    idempotency_key,lease_generation)
             VALUES (?1,?2,?3,'CAPABILITY','PLANNED',0,0,'calendar','calendar.event.create','1.0.0',?4,?5,0)",
            params![step, TASK, sequence, digest, idk],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO step_capability_bindings(task_id,step_id,generation_id,descriptor_digest,
                                                  capability_id,capability_version,provider_id,
                                                  implementation_id)
             VALUES (?1,?2,1,?3,'calendar.event.create','1.0.0','calendar',NULL)",
            params![TASK, step, DESCRIPTOR],
        )
        .unwrap();
    }
}

fn insert_revision(conn: &Connection, revision: i64, _activated: bool) {
    conn.execute(
        "INSERT INTO policy_revisions(revision_id,rules_digest,rule_count,created_at_ms,activated_at_ms,
                                      actor_class,actor_id,reason_code)
         VALUES (?1,?2,1,500,NULL,'HOST','local-admin','SEED')",
        params![revision, RULES],
    )
    .unwrap();
}

fn insert_rule(conn: &Connection, revision: i64, rule_id: &str, decision: &str) {
    conn.execute(
        "INSERT INTO policy_rules(revision_id,rule_id,priority,capability_id,risk_class,
                                  side_effect_class,authorization,automation_context,requested_by,
                                  enabled,decision,reason_code)
         VALUES (?1,?2,10,'calendar.event.create','EXTERNAL_WRITE','EXTERNAL_WRITE','SCOPED_GRANT',
                 'INTERACTIVE','USER',1,?3,'SEED_RULE')",
        params![revision, rule_id, decision],
    )
    .unwrap();
}

fn activate_seed_revision(conn: &Connection, revision: i64) {
    conn.execute(
        "UPDATE policy_revisions SET activated_at_ms=600 WHERE revision_id=?1",
        [revision],
    )
    .unwrap();
}

/// One approval request enumerating `steps`, with the action-set digest the
/// caller must compute by hand for the same input.
fn insert_request(conn: &Connection, steps: &[&str], digests: &[&str]) {
    conn.execute(
        "INSERT INTO approval_requests(approval_id,task_id,plan_revision,capability_id,
                                       capability_version,generation_id,descriptor_digest,
                                       risk_class,side_effect_class,authorization,data_class,
                                       requested_by,automation_context,action_set_digest,
                                       action_count,max_uses,summary_kind,summary,raised_at_ms,
                                       expires_at_ms,status)
         VALUES (?1,?2,0,'calendar.event.create','1.0.0',1,?3,'EXTERNAL_WRITE','EXTERNAL_WRITE',
                 'SCOPED_GRANT','PERSONAL','USER','INTERACTIVE',?4,?5,?5,'host.builder',
                 'Create three calendar events.',100,100000,'PENDING')",
        params![APPROVAL, TASK, DESCRIPTOR, BAD, steps.len() as i64],
    )
    .unwrap();
    for (position, (step, digest)) in steps.iter().zip(digests).enumerate() {
        conn.execute(
            "INSERT INTO approval_request_actions(approval_id,position,step_id,arguments_digest,
                                                 scope,scope_digest)
             VALUES (?1,?2,?3,?4,?5,?6)",
            params![APPROVAL, position as i64, step, digest, SCOPE, SCOPE_DIGEST],
        )
        .unwrap();
    }
}

fn approve_and_grant(conn: &Connection, granted_steps: &[&str], digests: &[&str]) {
    approve_and_grant_with_seal(conn, granted_steps, digests, true);
}

fn approve_and_grant_unsealed(conn: &Connection, granted_steps: &[&str], digests: &[&str]) {
    approve_and_grant_with_seal(conn, granted_steps, digests, false);
}

fn approve_and_grant_with_seal(
    conn: &Connection,
    granted_steps: &[&str],
    digests: &[&str],
    seal: bool,
) {
    conn.execute(
        "UPDATE approval_requests SET status='APPROVED' WHERE approval_id=?1",
        params![APPROVAL],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO approval_grants(grant_id,approval_id,task_id,plan_revision,capability_id,
                                     capability_version,generation_id,descriptor_digest,
                                     action_set_digest,action_count,max_uses,uses_remaining,
                                     granted_at_ms,expires_at_ms,granted_by,auth_strength,status)
         VALUES (?1,?2,?3,0,'calendar.event.create','1.0.0',1,?4,?5,?6,?6,?6,200,100000,'USER',
                 'ELEVATED_CONFIRMED','ACTIVE')",
        params![
            GRANT,
            APPROVAL,
            TASK,
            DESCRIPTOR,
            BAD,
            granted_steps.len() as i64
        ],
    )
    .unwrap();
    for (position, (step, digest)) in granted_steps.iter().zip(digests).enumerate() {
        conn.execute(
            "INSERT INTO approval_grant_members(grant_id,position,approval_id,step_id,
                                                arguments_digest,scope_digest)
             VALUES (?1,?2,?3,?4,?5,?6)",
            params![GRANT, position as i64, APPROVAL, step, digest, SCOPE_DIGEST],
        )
        .unwrap();
    }
    if seal {
        conn.execute(
            "INSERT INTO approval_grant_seals(grant_id,sealed_at_ms) VALUES (?1,200)",
            params![GRANT],
        )
        .unwrap();
    }
}

fn scalar(conn: &Connection, sql: &str) -> rusqlite::Result<i64> {
    conn.query_row(sql, [], |row| row.get(0))
}

/// Reads a possibly-NULL singleton scalar as 0, which is how the empty
/// activation pointer is represented.
fn pointer(conn: &Connection, sql: &str) -> i64 {
    conn.query_row(sql, [], |row| row.get(0)).unwrap_or(0)
}

fn text(conn: &Connection, sql: &str, grant_id: &str) -> rusqlite::Result<String> {
    conn.query_row(sql, params![grant_id], |row| row.get(0))
}

fn text_pair(conn: &Connection, sql: &str, first: &str, second: &str) -> rusqlite::Result<String> {
    conn.query_row(sql, params![first, second], |row| row.get(0))
}

// ---------------------------------------------------------------------------
// 1 and 2 — upgrade and fresh migration
// ---------------------------------------------------------------------------

#[test]
fn migration_catalog_appends_p6b_without_rewriting_any_prior_migration() {
    let catalog = Migrations::embedded();
    assert_eq!(
        Migrations::LATEST,
        if cfg!(feature = "p7-experimental") {
            9
        } else {
            8
        }
    );
    assert_eq!(catalog.len(), Migrations::LATEST as usize);
    assert_eq!(
        (catalog[4].version, catalog[4].name),
        (5, "0005_policy_approval")
    );
    // Every prior migration byte and checksum is preserved. This is the P5
    // frozen-boundary assertion, not an accident of ordering.
    assert_eq!(
        Migrations::checksum(catalog[0].sql).as_str(),
        "sha256:d9068dccbc26ececb71be79c475080633166ba0163c62b2d98b9733512baefea"
    );
    assert_eq!(
        Migrations::checksum(catalog[1].sql).as_str(),
        "sha256:4924e69150bbff9c39e2e6b7e2bdd61045202e504900fe0f510d513fbf815e67"
    );
    assert_eq!(
        Migrations::checksum(catalog[2].sql).as_str(),
        "sha256:530a6d6cb5ec9c757311d48e10a62ef456d9d01c09512f321cffe42fe3307f80"
    );
    assert_eq!(
        Migrations::checksum(catalog[3].sql).as_str(),
        "sha256:06b22fa682564290b71a26825a777f7a295220bd02f4c6c614d234b73001685f"
    );
    assert!(
        Migrations::checksum(catalog[4].sql)
            .as_str()
            .starts_with("sha256:")
    );
    assert_eq!(
        Migrations::checksum(catalog[4].sql).as_str(),
        "sha256:cf6686807e3ceb10bd21c6653fbdd61c487e6b8938a2d78104d160920831e71a"
    );
    assert_eq!(
        (catalog[5].version, catalog[5].name),
        (6, "0006_policy_authority_sealing")
    );
    assert_eq!(
        (catalog[6].version, catalog[6].name),
        (7, "0007_policy_revision_sealing")
    );
    assert_eq!(
        (catalog[7].version, catalog[7].name),
        (8, "0008_approval_lifecycle_routing")
    );
    #[cfg(feature = "p7-experimental")]
    assert_eq!(
        (catalog[8].version, catalog[8].name),
        (9, "test-fixture-p7-experimental-memory-v9")
    );
    assert_eq!(
        Migrations::checksum(catalog[6].sql).as_str(),
        "sha256:48f2f1805d59f4eec215963ac5109007294a0dfe015b9a4b4e7e69c14f9197eb"
    );
}

#[test]
fn fresh_store_applies_the_whole_schema_at_v9() {
    let store = Store::open_in_memory(&FixedClock).unwrap();
    assert_eq!(store.schema_version().unwrap(), Migrations::LATEST);
    let conn = store.conn.lock().unwrap();
    for table in [
        "policy_revisions",
        "policy_rules",
        "policy_state",
        "approval_requests",
        "approval_request_actions",
        "approval_grants",
        "approval_grant_members",
        "approval_grant_uses",
        "approval_grant_seals",
    ] {
        let strict: i64 = conn
            .query_row(
                "SELECT strict FROM pragma_table_list WHERE name=?1",
                params![table],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(strict, 1, "{table} must be STRICT");
    }
    #[cfg(feature = "p7-experimental")]
    for table in ["p7_owners", "p7_memory_items", "p7_resource_accounting"] {
        let strict: i64 = conn
            .query_row(
                "SELECT strict FROM pragma_table_list WHERE name=?1",
                params![table],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(strict, 1, "{table} must be STRICT");
    }
    // The singleton pointer exists and starts empty.
    assert_eq!(
        pointer(
            &conn,
            "SELECT active_revision_id FROM policy_state WHERE singleton=1"
        ),
        0
    );
    let integrity: String = conn
        .query_row("PRAGMA integrity_check", [], |row| row.get(0))
        .unwrap();
    assert_eq!(integrity, "ok");
    assert_eq!(
        scalar(&conn, "SELECT count(*) FROM pragma_foreign_key_check").unwrap(),
        0
    );
}

#[test]
fn migration_0008_preserves_existing_approval_wakes_and_adds_grant_outcomes() {
    let path = TempDb::new();
    {
        let mut conn = Connection::open(&path.0).unwrap();
        crate::migrate::apply(
            &mut conn,
            &Migrations::embedded()[..7],
            EpochMillis::new(100).unwrap(),
            true,
        )
        .unwrap();
        seed_registry_and_task(&conn);
        conn.execute("INSERT INTO approval_lifecycle_wakes(source_event_id,source_seq,approval_id,task_id,step_id,outcome_kind,created_at_ms) VALUES (?1,1,?2,?3,?4,'GRANTED',10)",params!["evt_01JQ8ZA7B3KMW9Q4TVY7XN2RDP",APPROVAL,TASK,STEP_A]).unwrap();
    }
    let store = Store::open(&path.0, &FixedClock).unwrap();
    let conn = store.conn.lock().unwrap();
    assert_eq!(
        scalar(
            &conn,
            "SELECT count(*) FROM approval_lifecycle_wakes WHERE outcome_kind='GRANTED'"
        )
        .unwrap(),
        1
    );
    conn.execute("INSERT INTO approval_lifecycle_wakes(source_event_id,source_seq,approval_id,task_id,step_id,outcome_kind,created_at_ms) VALUES (?1,3,?2,?3,?4,'EXPIRED_UNUSED',12)",params!["evt_01JQ8ZA7B3KMW9Q4TVY7XN2RDS",APPROVAL,TASK,STEP_A]).unwrap();
    assert_eq!(
        scalar(
            &conn,
            "SELECT count(*) FROM approval_lifecycle_wakes WHERE outcome_kind='EXPIRED_UNUSED'"
        )
        .unwrap(),
        1
    );
    conn.execute("INSERT INTO approval_lifecycle_wakes(source_event_id,source_seq,approval_id,task_id,step_id,outcome_kind,created_at_ms) VALUES (?1,2,?2,?3,?4,'REVOKED',11)",params!["evt_01JQ8ZA7B3KMW9Q4TVY7XN2RDQ",APPROVAL,TASK,STEP_A]).unwrap();
    assert_eq!(
        scalar(
            &conn,
            "SELECT count(*) FROM approval_lifecycle_wakes WHERE outcome_kind='REVOKED'"
        )
        .unwrap(),
        1
    );
}

#[test]
fn existing_schema_v4_upgrades_through_v5_to_v8_and_keeps_its_rows() {
    let temp = TempDb::new();
    seed_v4(&temp.0);
    {
        let conn = Connection::open(&temp.0).unwrap();
        // P5-shaped pre-P6 rows that must survive the upgrade untouched.
        conn.execute(
            "INSERT INTO capability_registry_generations(generation_id,manifest_digest,schema_catalog_digest)
             VALUES (1,?1,?2)",
            params![MANIFEST, SCHEMA_CATALOG],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO tasks(task_id,kind,title,state,origin_kind,data_class_rank,policy_class_rank,
                               created_at_ms,updated_at_ms,max_model_calls,max_tool_calls,
                               max_attempts_per_step)
             VALUES (?1,'MAINTENANCE','legacy','EXECUTING','SYSTEM',0,0,0,0,12,0,0)",
            params![TASK],
        )
        .unwrap();
    }

    let store = Store::open(&temp.0, &FixedClock).unwrap();
    assert_eq!(store.schema_version().unwrap(), Migrations::LATEST);
    let conn = store.conn.lock().unwrap();
    assert_eq!(
        scalar(
            &conn,
            "SELECT count(*) FROM tasks WHERE task_id='tsk_01JQ8Z9K3M7QWXR4V2T6YH0BNA'"
        )
        .unwrap(),
        1
    );
    assert_eq!(
        scalar(&conn, "SELECT count(*) FROM approval_requests").unwrap(),
        0
    );
    assert_eq!(
        scalar(
            &conn,
            "SELECT count(*) FROM schema_migrations WHERE version=5"
        )
        .unwrap(),
        1
    );
    assert_eq!(
        scalar(
            &conn,
            "SELECT count(*) FROM schema_migrations WHERE version=7"
        )
        .unwrap(),
        1
    );
}

#[test]
fn existing_schema_v5_upgrades_to_v8_without_rewriting_migration_0005() {
    let temp = TempDb::new();
    seed_v5(&temp.0);
    let before = Migrations::checksum(Migrations::embedded()[4].sql);
    let store = Store::open(&temp.0, &FixedClock).unwrap();
    assert_eq!(store.schema_version().unwrap(), Migrations::LATEST);
    store.verify_integrity().unwrap();
    let conn = store.conn.lock().unwrap();
    let checksum: String = conn
        .query_row(
            "SELECT checksum FROM schema_migrations WHERE version=5",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(checksum, before.as_str());
    assert_eq!(
        scalar(
            &conn,
            "SELECT count(*) FROM sqlite_schema WHERE type='table' AND name='approval_grant_seals'"
        )
        .unwrap(),
        1
    );
}

// ---------------------------------------------------------------------------
// 3 and 4 — corruption and unsupported-schema refusals
// ---------------------------------------------------------------------------

#[test]
fn altered_p6b_migration_bytes_are_refused_as_a_checksum_mismatch() {
    let temp = TempDb::new();
    {
        let store = Store::open(&temp.0, &FixedClock).unwrap();
        assert_eq!(store.schema_version().unwrap(), Migrations::LATEST);
    }
    // Rewriting an applied migration's bytes is exactly what the checksum
    // guard is for. No reset, no force: the store refuses and stays intact.
    let mut before = std::fs::read(&temp.0).unwrap();
    assert!(!before.is_empty());
    {
        let conn = Connection::open(&temp.0).unwrap();
        conn.execute(
            "UPDATE schema_migrations SET checksum=?1 WHERE version=5",
            params![BAD],
        )
        .unwrap();
    }
    let after = std::fs::read(&temp.0).unwrap();
    assert_ne!(before, after);
    before = after;
    assert!(matches!(
        Store::open(&temp.0, &FixedClock),
        Err(StoreError::MigrationChecksumMismatch)
    ));
    assert_eq!(std::fs::read(&temp.0).unwrap(), before);
}

#[test]
fn a_database_without_the_serea_migration_authority_is_refused() {
    let temp = TempDb::new();
    std::fs::write(&temp.0, b"not a sqlite database at all, but not empty").unwrap();
    assert!(matches!(
        Store::open(&temp.0, &FixedClock).err(),
        Some(StoreError::NotSereaStore)
    ));
}

// ---------------------------------------------------------------------------
// 5, 6 and 7 — the singleton pointer's three guarantees
// ---------------------------------------------------------------------------

#[test]
fn the_pointer_only_advances_and_deletion_is_refused() {
    let store = Store::open_in_memory(&FixedClock).unwrap();
    let conn = store.conn.lock().unwrap();
    for revision in 1..=3 {
        insert_revision(&conn, revision, true);
        insert_rule(&conn, revision, "pol_0001", "ALLOW");
        activate_seed_revision(&conn, revision);
        conn.execute(
            "UPDATE policy_state SET active_revision_id=?1 WHERE singleton=1",
            params![revision],
        )
        .unwrap();
    }
    assert_eq!(
        scalar(&conn, "SELECT active_revision_id FROM policy_state").unwrap(),
        3
    );
    // Downgrade, including to NULL, is refused by a trigger rather than by a
    // Rust-side check a direct SQL writer could skip.
    for attempt in [1, 2, -1] {
        assert!(
            conn.execute(
                "UPDATE policy_state SET active_revision_id=?1 WHERE singleton=1",
                params![attempt]
            )
            .is_err(),
            "pointer must not move to {attempt}"
        );
    }
    assert!(
        conn.execute("DELETE FROM policy_state", []).is_err(),
        "the singleton row must not be deletable"
    );
    assert_eq!(
        scalar(&conn, "SELECT active_revision_id FROM policy_state").unwrap(),
        3
    );
}

#[test]
fn the_pointer_cannot_name_an_unactivated_revision() {
    let store = Store::open_in_memory(&FixedClock).unwrap();
    let conn = store.conn.lock().unwrap();
    insert_revision(&conn, 1, false);
    assert!(
        conn.execute(
            "UPDATE policy_state SET active_revision_id=1 WHERE singleton=1",
            []
        )
        .is_err(),
        "an uncommitted revision must never become active"
    );
    assert_eq!(
        scalar(&conn, "SELECT active_revision_id FROM policy_state").unwrap_or(0),
        0
    );
}

// ---------------------------------------------------------------------------
// 8 and 9 — activation is atomic with its event, and revisions are immutable
// ---------------------------------------------------------------------------

#[test]
fn revision_activation_and_its_policy_changed_event_commit_together_or_not_at_all() {
    let store = Store::open_in_memory(&FixedClock).unwrap();
    {
        let conn = store.conn.lock().unwrap();
        insert_revision(&conn, 1, false);
        insert_rule(&conn, 1, "pol_0001", "ALLOW");
    }

    // Committing case: activation plus the event, one transaction.
    let event = policy_changed_event();
    store
        .transact(|tx: &mut Tx<'_>| {
            activate(tx, 1)?;
            tx.append_event(event.clone(), None)?;
            Ok(())
        })
        .unwrap();
    assert_eq!(
        store
            .conn
            .lock()
            .unwrap()
            .query_row("SELECT active_revision_id FROM policy_state", [], |r| r
                .get::<_, i64>(0))
            .unwrap(),
        1
    );
    assert_eq!(count_kind(&store, "POLICY_CHANGED"), 1);

    // Rollback case: revision 2 was prepared before the activation transaction.
    // Its unactivated row may survive; only activation and the event must roll
    // back together.
    {
        let conn = store.conn.lock().unwrap();
        insert_revision(&conn, 2, false);
        insert_rule(&conn, 2, "pol_0002", "DENY");
    }
    let event = policy_changed_event();
    let failed = store.transact(|tx: &mut Tx<'_>| {
        activate(tx, 2)?;
        let revoked = tx.append_event(event, None)?;
        // Fail after the event row is durably written inside this
        // transaction: the whole transaction must roll back.
        Err::<(), _>(StoreError::Sqlite).map(|()| revoked)
    });
    assert!(failed.is_err());
    let conn = store.conn.lock().unwrap();
    assert_eq!(
        pointer(&conn, "SELECT active_revision_id FROM policy_state"),
        1
    );
    assert_eq!(count_kind_store(&conn, "POLICY_CHANGED"), 1);
    assert_eq!(
        scalar(
            &conn,
            "SELECT count(*) FROM policy_revisions WHERE revision_id=2"
        )
        .unwrap(),
        1
    );
    assert_eq!(
        scalar(
            &conn,
            "SELECT count(*) FROM policy_revisions WHERE revision_id=2 AND activated_at_ms IS NULL"
        )
        .unwrap(),
        1,
        "the prepared revision survives but remains unactivated"
    );
    assert_eq!(
        pointer(&conn, "SELECT active_revision_id FROM policy_state"),
        1
    );
    assert_eq!(count_kind_store(&conn, "POLICY_CHANGED"), 1);
}

fn activate(tx: &mut Tx<'_>, revision: i64) -> Result<(), StoreError> {
    tx.inner
        .execute(
            "UPDATE policy_revisions SET activated_at_ms=600 WHERE revision_id=?1",
            params![revision],
        )
        .map_err(StoreError::from)?;
    tx.inner
        .execute(
            "UPDATE policy_state SET active_revision_id=?1 WHERE singleton=1",
            params![revision],
        )
        .map_err(StoreError::from)?;
    Ok(())
}

fn count_kind(store: &Store, kind: &str) -> i64 {
    count_kind_store(&store.conn.lock().unwrap(), kind)
}

fn count_kind_store(conn: &Connection, kind: &str) -> i64 {
    conn.query_row(
        "SELECT count(*) FROM event_content WHERE kind=?1",
        params![kind],
        |row| row.get(0),
    )
    .unwrap()
}

fn policy_changed_event() -> serea_protocol::SereaEvent {
    serde_json::from_str(
        r#"{
          "envelope_version":"1",
          "surface":"serea.event/1",
          "message_id":"evt_01JQ8ZB7H2XKM9P4QW7NRT5YCD",
          "seq":"0",
          "kind":"POLICY_CHANGED",
          "occurred_at":"2026-10-01T09:14:23.902Z",
          "correlation_id":null,
          "causation_id":null,
          "actor":{"kind":"HOST","id":"serea-core","version":"0.1.0"},
          "data_class":"PUBLIC",
          "trace":null,
          "payload":{"actor_class":"HOST"}
        }"#,
    )
    .unwrap()
}

#[test]
fn an_activated_policy_revision_and_its_rules_are_immutable() {
    let store = Store::open_in_memory(&FixedClock).unwrap();
    let conn = store.conn.lock().unwrap();
    insert_revision(&conn, 1, false);
    insert_rule(&conn, 1, "pol_0001", "ALLOW");
    conn.execute(
        "INSERT INTO policy_rules(revision_id,rule_id,priority,enabled,decision,reason_code)
         VALUES (1,'pol_0002',10,1,'ALLOW','TIE')",
        [],
    )
    .unwrap();
    conn.execute(
        "UPDATE policy_revisions SET rule_count=2 WHERE revision_id=1",
        [],
    )
    .unwrap();
    assert!(
        conn.execute(
            "INSERT INTO policy_rules(revision_id,rule_id,priority,enabled,decision,reason_code)
         VALUES (1,'pol_0001',10,1,'DENY','DUP')",
            [],
        )
        .is_err(),
        "duplicate rule_id within one prepared revision must be refused"
    );
    activate_seed_revision(&conn, 1);
    conn.execute(
        "UPDATE policy_state SET active_revision_id=1 WHERE singleton=1",
        [],
    )
    .unwrap();

    // Facts, activation stamp and rules all refuse to change.
    assert!(
        conn.execute(
            "UPDATE policy_revisions SET rules_digest=?1 WHERE revision_id=1",
            params![BAD]
        )
        .is_err()
    );
    assert!(
        conn.execute(
            "UPDATE policy_revisions SET rule_count=2 WHERE revision_id=1",
            []
        )
        .is_err()
    );
    assert!(
        conn.execute(
            "UPDATE policy_revisions SET activated_at_ms=700 WHERE revision_id=1",
            []
        )
        .is_err()
    );
    assert!(
        conn.execute("DELETE FROM policy_revisions WHERE revision_id=1", [])
            .is_err()
    );
    assert!(
        conn.execute(
            "UPDATE policy_rules SET decision='DENY' WHERE revision_id=1 AND rule_id='pol_0001'",
            []
        )
        .is_err()
    );
    assert!(
        conn.execute(
            "DELETE FROM policy_rules WHERE revision_id=1 AND rule_id='pol_0001'",
            []
        )
        .is_err()
    );
    // A late rule cannot alter an activated digest, even when its identifier is unique.
    assert!(
        conn.execute(
            "INSERT INTO policy_rules(revision_id,rule_id,priority,enabled,decision,reason_code)
             VALUES (1,'pol_0003',10,1,'ALLOW','LATE')",
            []
        )
        .is_err(),
        "activated policy rule set must reject new members"
    );
}

#[test]
fn an_unactivated_revision_can_still_be_prepared_and_then_activated() {
    let store = Store::open_in_memory(&FixedClock).unwrap();
    let conn = store.conn.lock().unwrap();
    insert_revision(&conn, 1, false);
    insert_rule(&conn, 1, "pol_0001", "ALLOW");
    insert_rule(&conn, 1, "pol_0002", "DENY");
    // An unactivated revision is editable, which is what makes a multi-statement
    // import possible at all.
    conn.execute(
        "UPDATE policy_revisions SET rule_count=2 WHERE revision_id=1",
        [],
    )
    .unwrap();
    // A rule row is immutable from birth: `policy_rule_no_update` is not scoped
    // to activated revisions, so a prepared revision must be built by deleting
    // and re-inserting, never by editing.
    assert!(
        conn.execute(
            "UPDATE policy_rules SET priority=20 WHERE revision_id=1 AND rule_id='pol_0002'",
            []
        )
        .is_err(),
        "rule rows must not be editable after insertion"
    );
    conn.execute(
        "UPDATE policy_revisions SET activated_at_ms=600 WHERE revision_id=1",
        [],
    )
    .unwrap();
    // A second activation of the same revision is refused.
    assert!(
        conn.execute(
            "UPDATE policy_revisions SET activated_at_ms=700 WHERE revision_id=1",
            []
        )
        .is_err()
    );
}

#[test]
fn a_revision_may_not_exceed_the_rule_bound() {
    let store = Store::open_in_memory(&FixedClock).unwrap();
    let conn = store.conn.lock().unwrap();
    insert_revision(&conn, 1, false);
    assert!(
        conn.execute(
            "UPDATE policy_revisions SET rule_count=513 WHERE revision_id=1",
            []
        )
        .is_err(),
        "max_active_policy_rules must be refused at write time, not at evaluation"
    );
    conn.execute(
        "UPDATE policy_revisions SET rule_count=512 WHERE revision_id=1",
        [],
    )
    .unwrap();
}

// ---------------------------------------------------------------------------
// 10 — invalid Step/Task association
// ---------------------------------------------------------------------------

#[test]
fn an_approval_action_must_belong_to_the_request_task_and_pinned_step() {
    let store = Store::open_in_memory(&FixedClock).unwrap();
    let conn = store.conn.lock().unwrap();
    seed_registry_and_task(&conn);

    // A step of another task.
    conn.execute(
        "INSERT INTO tasks(task_id,kind,title,state,origin_kind,data_class_rank,policy_class_rank,
                           created_at_ms,updated_at_ms,max_model_calls,max_tool_calls,
                           max_attempts_per_step,capability_registry_generation)
         VALUES (?1,'USER_REQUEST','other','EXECUTING','USER_MESSAGE',1,3,0,1,12,24,3,1)",
        params![OTHER_TASK],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO task_steps(step_id,task_id,sequence,kind,status,attempt,plan_revision,
                                provider_id,capability_id,capability_version,input_digest,
                                idempotency_key,lease_generation)
         VALUES (?1,?2,0,'CAPABILITY','PLANNED',0,0,'calendar','calendar.event.create','1.0.0',?3,?4,0)",
        params![OTHER_STEP, OTHER_TASK, ARGS_A, "idk_000000000000000000000000000000000000000000000000000000000000000c"],
    )
    .unwrap();

    // A step from another task must never enter this task's action set.
    conn.execute(
        "INSERT INTO approval_requests(approval_id,task_id,plan_revision,capability_id,
                                       capability_version,generation_id,descriptor_digest,
                                       risk_class,side_effect_class,authorization,data_class,
                                       requested_by,automation_context,action_set_digest,
                                       action_count,max_uses,summary_kind,summary,raised_at_ms,
                                       expires_at_ms,status)
         VALUES (?1,?2,0,'calendar.event.create','1.0.0',1,?3,'EXTERNAL_WRITE','EXTERNAL_WRITE',
                 'SCOPED_GRANT','PERSONAL','USER','INTERACTIVE',?4,1,1,'host.builder','s',100,
                 100000,'PENDING')",
        params![APPROVAL, TASK, DESCRIPTOR, BAD],
    )
    .unwrap();
    assert!(
        conn.execute(
            "INSERT INTO approval_request_actions(approval_id,position,step_id,arguments_digest,
                                                 scope,scope_digest)
             VALUES (?1,0,?2,?3,?4,?5)",
            params![APPROVAL, OTHER_STEP, ARGS_A, SCOPE, SCOPE_DIGEST],
        )
        .is_err(),
        "an action step from another task must be refused"
    );

    // A non-capability step is not an approvable action either.
    conn.execute(
        "INSERT INTO task_steps(step_id,task_id,sequence,kind,status,attempt,plan_revision,
                                input_digest,lease_generation)
         VALUES (?1,?2,9,'MODEL_TURN','PLANNED',0,0,?3,0)",
        params![MODEL_STEP, TASK, ARGS_A],
    )
    .unwrap();
    assert!(
        conn.execute(
            "INSERT INTO approval_request_actions(approval_id,position,step_id,arguments_digest,
                                                 scope,scope_digest)
             VALUES (?1,0,?2,?3,?4,?5)",
            params![APPROVAL, MODEL_STEP, ARGS_A, SCOPE, SCOPE_DIGEST],
        )
        .is_err(),
        "a non-capability step must never become an approval action"
    );
}

#[test]
fn an_approval_action_must_carry_the_durable_step_arguments_digest() {
    let store = Store::open_in_memory(&FixedClock).unwrap();
    let conn = store.conn.lock().unwrap();
    seed_registry_and_task(&conn);
    insert_request(&conn, &[STEP_A], &[ARGS_A]);
    assert_eq!(
        scalar(&conn, "SELECT count(*) FROM approval_request_actions").unwrap(),
        1
    );

    // A digest that is not the durable canonical step digest is refused, so a
    // prompt can never be built from arguments the step does not hold.
    conn.execute(
        "INSERT INTO approval_requests(approval_id,task_id,plan_revision,capability_id,
                                       capability_version,generation_id,descriptor_digest,
                                       risk_class,side_effect_class,authorization,data_class,
                                       requested_by,automation_context,action_set_digest,
                                       action_count,max_uses,summary_kind,summary,raised_at_ms,
                                       expires_at_ms,status)
         VALUES (?4,?1,0,'calendar.event.create','1.0.0',1,?2,
                 'EXTERNAL_WRITE','EXTERNAL_WRITE','SCOPED_GRANT','PERSONAL','USER','INTERACTIVE',
                 ?3,1,1,'host.builder','s',100,100000,'PENDING')",
        params![TASK, DESCRIPTOR, BAD, APPROVAL_2],
    )
    .unwrap();
    assert!(
        conn.execute(
            "INSERT INTO approval_request_actions(approval_id,position,step_id,arguments_digest,
                                                 scope,scope_digest)
             VALUES (?5,0,?1,?2,?3,?4)",
            params![STEP_A, ARGS_B, SCOPE, SCOPE_DIGEST, APPROVAL_2],
        )
        .is_err(),
        "the action digest must equal the durable step input_digest"
    );
}

#[test]
fn an_action_step_plan_capability_generation_and_descriptor_must_all_match() {
    let store = Store::open_in_memory(&FixedClock).unwrap();
    let conn = store.conn.lock().unwrap();
    seed_registry_and_task(&conn);
    insert_request(&conn, &[STEP_A], &[ARGS_A]);

    // A step re-pinned to a newer plan revision cannot join the set.
    conn.execute(
        "UPDATE task_steps SET plan_revision=1 WHERE step_id=?1",
        params![STEP_B],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO approval_requests(approval_id,task_id,plan_revision,capability_id,
                                       capability_version,generation_id,descriptor_digest,
                                       risk_class,side_effect_class,authorization,data_class,
                                       requested_by,automation_context,action_set_digest,
                                       action_count,max_uses,summary_kind,summary,raised_at_ms,
                                       expires_at_ms,status)
         VALUES (?4,?1,1,'calendar.event.create','1.0.0',1,?2,
                 'EXTERNAL_WRITE','EXTERNAL_WRITE','SCOPED_GRANT','PERSONAL','USER','INTERACTIVE',
                 ?3,1,1,'host.builder','s',100,100000,'PENDING')",
        params![TASK, DESCRIPTOR, BAD, APPROVAL_2],
    )
    .unwrap();
    // That persisted because plan_revision 1 matches; the refusal is on the
    // action insert against the step whose pinned revision is still 0.
    assert!(
        conn.execute(
            "INSERT INTO approval_request_actions(approval_id,position,step_id,arguments_digest,
                                                 scope,scope_digest)
             VALUES (?5,0,?1,?2,?3,?4)",
            params![STEP_A, ARGS_A, SCOPE, SCOPE_DIGEST, APPROVAL_2],
        )
        .is_err(),
        "a step pinned to a different plan revision cannot be enumerated"
    );
}

#[test]
fn the_action_set_is_bounded_before_the_request_is_ever_raised() {
    let store = Store::open_in_memory(&FixedClock).unwrap();
    let conn = store.conn.lock().unwrap();
    seed_registry_and_task(&conn);

    // `action_count` is the request's own declared set size. The within-count
    // trigger is the mechanical backstop for a direct SQL writer that ignored it.
    conn.execute(
        "INSERT INTO approval_requests(approval_id,task_id,plan_revision,capability_id,
                                       capability_version,generation_id,descriptor_digest,
                                       risk_class,side_effect_class,authorization,data_class,
                                       requested_by,automation_context,action_set_digest,
                                       action_count,max_uses,summary_kind,summary,raised_at_ms,
                                       expires_at_ms,status)
         VALUES (?1,?2,0,'calendar.event.create','1.0.0',1,?3,'EXTERNAL_WRITE','EXTERNAL_WRITE',
                 'SCOPED_GRANT','PERSONAL','USER','INTERACTIVE',?4,3,3,'host.builder','s',100,
                 100000,'PENDING')",
        params![APPROVAL, TASK, DESCRIPTOR, BAD],
    )
    .unwrap();
    for (position, (step, digest)) in [(STEP_A, ARGS_A), (STEP_B, ARGS_B), (STEP_C, ARGS_A)]
        .into_iter()
        .enumerate()
    {
        conn.execute(
            "INSERT INTO approval_request_actions(approval_id,position,step_id,arguments_digest,
                                                 scope,scope_digest)
             VALUES (?1,?2,?3,?4,?5,?6)",
            params![APPROVAL, position as i64, step, digest, SCOPE, SCOPE_DIGEST],
        )
        .unwrap();
    }
    assert!(
        conn.execute(
            "INSERT INTO approval_request_actions(approval_id,position,step_id,arguments_digest,
                                                 scope,scope_digest)
             VALUES (?1,3,?2,?3,?4,?5)",
            params![APPROVAL, STEP_A, ARGS_A, SCOPE, SCOPE_DIGEST],
        )
        .is_err(),
        "the action set must not grow past its declared count"
    );
    assert_eq!(
        scalar(&conn, "SELECT count(*) FROM approval_request_actions").unwrap(),
        3
    );

    // `approval_grant_max_uses = 8` is a CHECK, not a convention.
    conn.execute(
        "UPDATE approval_requests SET action_count=9,max_uses=9 WHERE approval_id=?1",
        params![APPROVAL],
    )
    .unwrap_err();
}

#[test]
fn an_incomplete_request_action_set_cannot_be_approved() {
    let store = Store::open_in_memory(&FixedClock).unwrap();
    let conn = store.conn.lock().unwrap();
    seed_registry_and_task(&conn);
    conn.execute(
        "INSERT INTO approval_requests(approval_id,task_id,plan_revision,capability_id,
                                       capability_version,generation_id,descriptor_digest,
                                       risk_class,side_effect_class,authorization,data_class,
                                       requested_by,automation_context,action_set_digest,
                                       action_count,max_uses,summary_kind,summary,raised_at_ms,
                                       expires_at_ms,status)
         VALUES (?1,?2,0,'calendar.event.create','1.0.0',1,?3,'EXTERNAL_WRITE','EXTERNAL_WRITE',
                 'SCOPED_GRANT','PERSONAL','USER','INTERACTIVE',?4,2,2,'host.builder','s',100,
                 100000,'PENDING')",
        params![APPROVAL, TASK, DESCRIPTOR, BAD],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO approval_request_actions(approval_id,position,step_id,arguments_digest,
                                             scope,scope_digest)
         VALUES (?1,0,?2,?3,?4,?5)",
        params![APPROVAL, STEP_A, ARGS_A, SCOPE, SCOPE_DIGEST],
    )
    .unwrap();

    assert!(
        conn.execute(
            "UPDATE approval_requests SET status='APPROVED' WHERE approval_id=?1",
            params![APPROVAL],
        )
        .is_err(),
        "a declared two-action request with only one durable action cannot be approved"
    );
    assert_eq!(
        text(
            &conn,
            "SELECT status FROM approval_requests WHERE approval_id=?1",
            APPROVAL
        )
        .unwrap(),
        "PENDING"
    );
    assert!(
        conn.execute(
            "INSERT INTO approval_requests(approval_id,task_id,plan_revision,capability_id,
                                           capability_version,generation_id,descriptor_digest,
                                           risk_class,side_effect_class,authorization,data_class,
                                           requested_by,automation_context,action_set_digest,
                                           action_count,max_uses,summary_kind,summary,raised_at_ms,
                                           expires_at_ms,status)
             VALUES (?1,?2,0,'calendar.event.create','1.0.0',1,?3,'EXTERNAL_WRITE',
                     'EXTERNAL_WRITE','SCOPED_GRANT','PERSONAL','USER','INTERACTIVE',?4,2,2,
                     'host.builder','s',100,100000,'APPROVED')",
            params![APPROVAL_2, TASK, DESCRIPTOR, BAD],
        )
        .is_err()
    );
}

#[test]
fn a_grant_seal_refuses_a_member_count_below_its_declaration() {
    let store = Store::open_in_memory(&FixedClock).unwrap();
    let conn = store.conn.lock().unwrap();
    seed_registry_and_task(&conn);
    insert_request(&conn, &[STEP_A, STEP_B], &[ARGS_A, ARGS_B]);
    conn.execute(
        "UPDATE approval_requests SET status='APPROVED' WHERE approval_id=?1",
        params![APPROVAL],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO approval_grants(grant_id,approval_id,task_id,plan_revision,capability_id,
                                     capability_version,generation_id,descriptor_digest,
                                     action_set_digest,action_count,max_uses,uses_remaining,
                                     granted_at_ms,expires_at_ms,granted_by,auth_strength,status)
         VALUES (?1,?2,?3,0,'calendar.event.create','1.0.0',1,?4,?5,2,2,2,200,100000,
                 'USER','ELEVATED_CONFIRMED','ACTIVE')",
        params![GRANT_2, APPROVAL, TASK, DESCRIPTOR, BAD],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO approval_grant_members(grant_id,position,approval_id,step_id,
                                            arguments_digest,scope_digest)
         VALUES (?1,0,?2,?3,?4,?5)",
        params![GRANT_2, APPROVAL, STEP_A, ARGS_A, SCOPE_DIGEST],
    )
    .unwrap();
    assert!(
        conn.execute(
            "INSERT INTO approval_grant_seals(grant_id,sealed_at_ms) VALUES (?1,200)",
            params![GRANT_2],
        )
        .is_err()
    );
    assert!(
        conn.execute(
            "INSERT INTO approval_grant_uses(grant_id,step_id,task_id,consumed_at_ms)
             VALUES (?1,?2,?3,300)",
            params![GRANT_2, STEP_A, TASK],
        )
        .is_err()
    );
    assert_eq!(
        scalar(
            &conn,
            "SELECT uses_remaining FROM approval_grants WHERE grant_id='grt_01JQ8ZA7B3KMW9Q4TVY7XN2RDQ'"
        )
        .unwrap(),
        2
    );
}

#[test]
fn a_revoked_grant_cannot_be_reactivated_or_consumed() {
    let store = Store::open_in_memory(&FixedClock).unwrap();
    let conn = store.conn.lock().unwrap();
    seed_registry_and_task(&conn);
    insert_request(&conn, &[STEP_A], &[ARGS_A]);
    approve_and_grant(&conn, &[STEP_A], &[ARGS_A]);
    conn.execute(
        "UPDATE approval_grants SET status='REVOKED' WHERE grant_id=?1",
        params![GRANT],
    )
    .unwrap();
    assert!(
        conn.execute(
            "UPDATE approval_grants SET status='ACTIVE' WHERE grant_id=?1",
            params![GRANT],
        )
        .is_err()
    );
    assert!(
        conn.execute(
            "INSERT INTO approval_grant_uses(grant_id,step_id,task_id,consumed_at_ms)
             VALUES (?1,?2,?3,300)",
            params![GRANT, STEP_A, TASK],
        )
        .is_err()
    );
    assert_eq!(
        text(
            &conn,
            "SELECT status FROM approval_grants WHERE grant_id=?1",
            GRANT
        )
        .unwrap(),
        "REVOKED"
    );
}

#[test]
fn an_incomplete_grant_member_set_cannot_consume_a_use_or_grow_after_sealing() {
    let store = Store::open_in_memory(&FixedClock).unwrap();
    let conn = store.conn.lock().unwrap();
    seed_registry_and_task(&conn);
    insert_request(&conn, &[STEP_A, STEP_B], &[ARGS_A, ARGS_B]);
    approve_and_grant_unsealed(&conn, &[STEP_A], &[ARGS_A]);

    // The grant declares exactly one member in this partial approval. Until
    // that complete subset is explicitly sealed, it is not consumption authority.
    assert!(
        conn.execute(
            "INSERT INTO approval_grant_uses(grant_id,step_id,task_id,consumed_at_ms)
             VALUES (?1,?2,?3,300)",
            params![GRANT, STEP_A, TASK],
        )
        .is_err(),
        "unsealed action membership cannot be spent"
    );
    conn.execute(
        "INSERT INTO approval_grant_seals(grant_id,sealed_at_ms) VALUES (?1,200)",
        params![GRANT],
    )
    .unwrap();
    assert!(
        conn.execute(
            "INSERT INTO approval_grant_members(grant_id,position,approval_id,step_id,
                                                arguments_digest,scope_digest)
             VALUES (?1,1,?2,?3,?4,?5)",
            params![GRANT, APPROVAL, STEP_B, ARGS_B, SCOPE_DIGEST],
        )
        .is_err()
    );
    conn.execute(
        "INSERT INTO approval_grant_uses(grant_id,step_id,task_id,consumed_at_ms)
         VALUES (?1,?2,?3,300)",
        params![GRANT, STEP_A, TASK],
    )
    .unwrap();
    assert!(
        conn.execute(
            "DELETE FROM approval_grant_uses WHERE grant_id=?1",
            params![GRANT]
        )
        .is_err()
    );
    assert!(
        conn.execute(
            "UPDATE approval_grant_uses SET consumed_at_ms=301 WHERE grant_id=?1",
            params![GRANT],
        )
        .is_err()
    );
    assert_eq!(
        scalar(&conn, "SELECT count(*) FROM approval_grant_uses").unwrap(),
        1,
        "the global consumed-Step record cannot be deleted or changed"
    );
}

#[test]
fn authority_history_and_bound_step_facts_cannot_be_deleted_or_changed_in_place() {
    let store = Store::open_in_memory(&FixedClock).unwrap();
    let conn = store.conn.lock().unwrap();
    seed_registry_and_task(&conn);
    insert_request(&conn, &[STEP_A], &[ARGS_A]);
    approve_and_grant(&conn, &[STEP_A], &[ARGS_A]);

    assert!(
        conn.execute(
            "UPDATE task_steps SET input_digest=?1 WHERE step_id=?2",
            params![ARGS_B, STEP_A],
        )
        .is_err()
    );
    assert!(
        conn.execute("DELETE FROM task_steps WHERE step_id=?1", params![STEP_A])
            .is_err()
    );
    assert!(
        conn.execute(
            "DELETE FROM approval_grants WHERE grant_id=?1",
            params![GRANT]
        )
        .is_err()
    );
    assert!(
        conn.execute(
            "DELETE FROM approval_requests WHERE approval_id=?1",
            params![APPROVAL],
        )
        .is_err()
    );

    conn.execute(
        "INSERT INTO approval_grant_uses(grant_id,step_id,task_id,consumed_at_ms)
         VALUES (?1,?2,?3,300)",
        params![GRANT, STEP_A, TASK],
    )
    .unwrap();
    assert_eq!(
        scalar(&conn, "SELECT count(*) FROM approval_grant_uses").unwrap(),
        1
    );
}

// ---------------------------------------------------------------------------
// 11 — invalid identifiers, digests and states
// ---------------------------------------------------------------------------

#[test]
fn invalid_identifiers_digests_and_states_are_refused_by_the_schema() {
    let store = Store::open_in_memory(&FixedClock).unwrap();
    let conn = store.conn.lock().unwrap();
    seed_registry_and_task(&conn);

    let bad_ids = [
        "apr_short",
        "grt_01JQ8ZA7B3KMW9Q4TVY7XN2RDP",
        "xxx_01JQ8ZA7B3KMW9Q4TVY7XN2RDP",
        "apr_01JQ8ZA1D4NFG8K2M6RTV9XCW",
    ];
    for id in bad_ids {
        assert!(
            conn.execute(
                "INSERT INTO approval_requests(approval_id,task_id,plan_revision,capability_id,
                                               capability_version,generation_id,descriptor_digest,
                                               risk_class,side_effect_class,authorization,data_class,
                                               requested_by,automation_context,action_set_digest,
                                               action_count,max_uses,summary_kind,summary,
                                               raised_at_ms,expires_at_ms,status)
                 VALUES (?1,?2,0,'calendar.event.create','1.0.0',1,?3,'EXTERNAL_WRITE',
                         'EXTERNAL_WRITE','SCOPED_GRANT','PERSONAL','USER','INTERACTIVE',
                         ?4,1,1,'host.builder','s',100,100000,'PENDING')",
                params![id, TASK, DESCRIPTOR, BAD],
            )
            .is_err(),
            "identifier {id} must be refused"
        );
    }

    let insert_candidate = |id: &str, class: &str, digest: &str| {
        conn.execute(
            "INSERT INTO approval_requests(approval_id,task_id,plan_revision,capability_id,
                                           capability_version,generation_id,descriptor_digest,
                                           risk_class,side_effect_class,authorization,data_class,
                                           requested_by,automation_context,action_set_digest,
                                           action_count,max_uses,summary_kind,summary,raised_at_ms,
                                           expires_at_ms,status)
             VALUES (?1,?2,0,'calendar.event.create','1.0.0',1,?3,'EXTERNAL_WRITE',
                     'EXTERNAL_WRITE','SCOPED_GRANT',?4,'USER','INTERACTIVE',?5,1,1,
                     'host.builder','s',100,100000,'PENDING')",
            params![id, TASK, DESCRIPTOR, class, digest],
        )
    };
    // A persisted request row may never be PRIVATE, SECRET or CREDENTIAL.
    for (index, class) in ["PRIVATE", "SECRET", "CREDENTIAL"].into_iter().enumerate() {
        let id = format!("apr_0000000000000000000000000{}", index + 1);
        assert!(
            insert_candidate(&id, class, BAD).is_err(),
            "data_class {class} must be refused at insertion"
        );
    }
    // Digest shape: malformed encoding and invalid length are distinct cases.
    let invalid_length = &BAD[..BAD.len() - 1];
    for (index, digest) in ["nope", "sha256:zz", invalid_length, &format!("{BAD}ff")]
        .into_iter()
        .enumerate()
    {
        let id = format!("apr_0000000000000000000000000{}", index + 5);
        assert!(
            insert_candidate(&id, "PERSONAL", digest).is_err(),
            "digest {digest} must be refused at insertion"
        );
    }

    insert_request(&conn, &[STEP_A], &[ARGS_A]);
    // Every state outside the closed request set is refused. A request
    // REVOKED value does not exist: revocation is grant-only.
    for status in ["REVOKED", "PENDING ", "pending", "CANCELLED"] {
        conn.execute_batch("SAVEPOINT invalid_request_status")
            .unwrap();
        assert!(
            conn.execute(
                "UPDATE approval_requests SET status=?1 WHERE approval_id=?2",
                params![status, APPROVAL],
            )
            .is_err(),
            "request status {status} must be refused"
        );
        conn.execute_batch("ROLLBACK TO invalid_request_status; RELEASE invalid_request_status")
            .unwrap();
    }
}

#[test]
fn a_grant_may_only_name_a_closed_actor_authentication_and_auth_strength() {
    let store = Store::open_in_memory(&FixedClock).unwrap();
    let conn = store.conn.lock().unwrap();
    seed_registry_and_task(&conn);
    insert_request(&conn, &[STEP_A], &[ARGS_A]);
    approve_and_grant(&conn, &[STEP_A], &[ARGS_A]);

    for granted_by in ["MODEL", "PROVIDER", "SCHEDULER", "user"] {
        assert!(
            conn.execute(
                "UPDATE approval_grants SET granted_by=?1 WHERE grant_id=?2",
                params![granted_by, GRANT],
            )
            .is_err(),
            "granted_by {granted_by} must be refused; a model can never mint a grant"
        );
    }
    for strength in ["NONE", "SESSION ", "elevated_confirmed"] {
        assert!(
            conn.execute(
                "UPDATE approval_grants SET auth_strength=?1 WHERE grant_id=?2",
                params![strength, GRANT],
            )
            .is_err(),
            "auth_strength {strength} must be refused"
        );
    }
    for status in ["ACTIVE", "EXPIRED", "REVOKED"] {
        conn.execute_batch("SAVEPOINT invalid_grant_status")
            .unwrap();
        conn.execute(
            "UPDATE approval_grants SET status=?1 WHERE grant_id=?2",
            params![status, GRANT],
        )
        .unwrap();
        conn.execute_batch("ROLLBACK TO invalid_grant_status; RELEASE invalid_grant_status")
            .unwrap();
    }
    // EXHAUSTED means every granted step has consumed. Writing it by hand while
    // a use is still available would forge a state only the spend trigger makes.
    assert!(
        conn.execute(
            "UPDATE approval_grants SET status='EXHAUSTED' WHERE grant_id=?1",
            params![GRANT],
        )
        .is_err(),
        "EXHAUSTED with a use still available must be refused"
    );
    // ...and once the use really is spent, the trigger sets it.
    conn.execute(
        "INSERT INTO approval_grant_uses(grant_id,step_id,task_id,consumed_at_ms)
         VALUES (?1,?2,?3,300)",
        params![GRANT, STEP_A, TASK],
    )
    .unwrap();
    assert_eq!(
        text(
            &conn,
            "SELECT status FROM approval_grants WHERE grant_id=?1",
            GRANT
        )
        .unwrap(),
        "EXHAUSTED",
        "the final use must exhaust the grant"
    );
}

// ---------------------------------------------------------------------------
// 12 — task deletion cascade and 13 — policy history retention
// ---------------------------------------------------------------------------

#[test]
fn deleting_the_task_cascades_every_approval_row_and_spares_policy_history() {
    let store = Store::open_in_memory(&FixedClock).unwrap();
    let conn = store.conn.lock().unwrap();
    seed_registry_and_task(&conn);
    insert_revision(&conn, 1, true);
    insert_rule(&conn, 1, "pol_0001", "REQUIRE_APPROVAL");
    activate_seed_revision(&conn, 1);
    conn.execute(
        "UPDATE policy_state SET active_revision_id=1 WHERE singleton=1",
        [],
    )
    .unwrap();
    insert_request(&conn, &[STEP_A], &[ARGS_A]);
    approve_and_grant(&conn, &[STEP_A], &[ARGS_A]);
    conn.execute(
        "INSERT INTO approval_grant_uses(grant_id,step_id,task_id,consumed_at_ms)
         VALUES (?1,?2,?3,300)",
        params![GRANT, STEP_A, TASK],
    )
    .unwrap();

    conn.execute("DELETE FROM tasks WHERE task_id=?1", params![TASK])
        .unwrap();

    for table in [
        "approval_requests",
        "approval_request_actions",
        "approval_grants",
        "approval_grant_members",
        "approval_grant_uses",
    ] {
        assert_eq!(
            scalar(&conn, &format!("SELECT count(*) FROM {table}")).unwrap(),
            0,
            "{table} must cascade with the task"
        );
    }
    // Policy history is a host audit artefact and never cascades with a task.
    assert_eq!(
        scalar(&conn, "SELECT count(*) FROM policy_revisions").unwrap(),
        1
    );
    assert_eq!(
        scalar(&conn, "SELECT count(*) FROM policy_rules").unwrap(),
        1
    );
    assert_eq!(
        pointer(&conn, "SELECT active_revision_id FROM policy_state"),
        1
    );
    // The Store mutex is not reentrant: drop the raw connection guard before
    // calling a Store-level method, or the test deadlocks.
    drop(conn);
    store.verify_integrity().unwrap();
}

#[test]
fn deleting_the_task_never_touches_the_active_policy_pointer() {
    let store = Store::open_in_memory(&FixedClock).unwrap();
    let conn = store.conn.lock().unwrap();
    seed_registry_and_task(&conn);
    for revision in 1..=3 {
        insert_revision(&conn, revision, true);
        insert_rule(&conn, revision, "pol_0001", "ALLOW");
        activate_seed_revision(&conn, revision);
        conn.execute(
            "UPDATE policy_state SET active_revision_id=?1 WHERE singleton=1",
            params![revision],
        )
        .unwrap();
    }
    insert_request(&conn, &[STEP_A], &[ARGS_A]);
    conn.execute("DELETE FROM tasks WHERE task_id=?1", params![TASK])
        .unwrap();
    assert_eq!(
        scalar(&conn, "SELECT active_revision_id FROM policy_state").unwrap(),
        3
    );
    // An activated revision survives a task deletion, which is what makes the
    // one-year POLICY_CHANGED audit class real.
    assert_eq!(
        scalar(&conn, "SELECT count(*) FROM policy_revisions").unwrap(),
        3
    );
}

// ---------------------------------------------------------------------------
// 14 — duplicate grant use, and the R2 membership rule
// ---------------------------------------------------------------------------

#[test]
fn the_same_step_cannot_be_consumed_twice_by_the_same_grant() {
    let store = Store::open_in_memory(&FixedClock).unwrap();
    let conn = store.conn.lock().unwrap();
    seed_registry_and_task(&conn);
    insert_request(&conn, &[STEP_A], &[ARGS_A]);
    approve_and_grant(&conn, &[STEP_A], &[ARGS_A]);

    conn.execute(
        "INSERT INTO approval_grant_uses(grant_id,step_id,task_id,consumed_at_ms)
         VALUES (?1,?2,?3,300)",
        params![GRANT, STEP_A, TASK],
    )
    .unwrap();
    let remaining: i64 = conn
        .query_row(
            "SELECT uses_remaining FROM approval_grants WHERE grant_id=?1",
            params![GRANT],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(remaining, 0);
    assert!(
        conn.execute(
            "INSERT INTO approval_grant_uses(grant_id,step_id,task_id,consumed_at_ms)
             VALUES (?1,?2,?3,301)",
            params![GRANT, STEP_A, TASK],
        )
        .is_err(),
        "a repeated consumption for the same step must be refused"
    );
    let remaining: i64 = conn
        .query_row(
            "SELECT uses_remaining FROM approval_grants WHERE grant_id=?1",
            params![GRANT],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(remaining, 0);
}

#[test]
fn a_second_grant_cannot_authorize_an_already_consumed_step() {
    let store = Store::open_in_memory(&FixedClock).unwrap();
    let conn = store.conn.lock().unwrap();
    seed_registry_and_task(&conn);
    insert_request(&conn, &[STEP_A], &[ARGS_A]);
    approve_and_grant(&conn, &[STEP_A], &[ARGS_A]);
    conn.execute(
        "INSERT INTO approval_grant_uses(grant_id,step_id,task_id,consumed_at_ms)
         VALUES (?1,?2,?3,300)",
        params![GRANT, STEP_A, TASK],
    )
    .unwrap();

    // A fresh approval unit and grant for the same step, which is the legitimate
    // re-approval flow after an unused grant was revoked or expired.
    conn.execute(
        "INSERT INTO approval_requests(approval_id,task_id,plan_revision,capability_id,
                                       capability_version,generation_id,descriptor_digest,
                                       risk_class,side_effect_class,authorization,data_class,
                                       requested_by,automation_context,action_set_digest,
                                       action_count,max_uses,summary_kind,summary,raised_at_ms,
                                       expires_at_ms,status)
         VALUES (?4,?1,0,'calendar.event.create','1.0.0',1,?2,
                 'EXTERNAL_WRITE','EXTERNAL_WRITE','SCOPED_GRANT','PERSONAL','USER','INTERACTIVE',
                 ?3,1,1,'host.builder','s',100,100000,'PENDING')",
        params![TASK, DESCRIPTOR, BAD, APPROVAL_2],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO approval_request_actions(approval_id,position,step_id,arguments_digest,
                                              scope,scope_digest)
         VALUES (?5,0,?1,?2,?3,?4)",
        params![STEP_A, ARGS_A, SCOPE, SCOPE_DIGEST, APPROVAL_2],
    )
    .unwrap();
    let second = GRANT_2;
    conn.execute(
        "UPDATE approval_requests SET status='APPROVED' WHERE approval_id=?1",
        params![APPROVAL_2],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO approval_grants(grant_id,approval_id,task_id,plan_revision,capability_id,
                                     capability_version,generation_id,descriptor_digest,
                                     action_set_digest,action_count,max_uses,uses_remaining,
                                     granted_at_ms,expires_at_ms,granted_by,auth_strength,status)
         VALUES (?1,?5,?2,0,'calendar.event.create','1.0.0',1,?3,
                 ?4,1,1,1,200,100000,'USER','ELEVATED_CONFIRMED','ACTIVE')",
        params![second, TASK, DESCRIPTOR, BAD, APPROVAL_2],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO approval_grant_members(grant_id,position,approval_id,step_id,
                                            arguments_digest,scope_digest)
         VALUES (?1,0,?5,?2,?3,?4)",
        params![second, STEP_A, ARGS_A, SCOPE_DIGEST, APPROVAL_2],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO approval_grant_seals(grant_id,sealed_at_ms) VALUES (?1,200)",
        params![second],
    )
    .unwrap();
    // The member row is fine; the *use* is not. UNIQUE(step_id) is what makes a
    // previously consumed Step unauthorized by any second grant.
    let duplicate = conn.execute(
        "INSERT INTO approval_grant_uses(grant_id,step_id,task_id,consumed_at_ms)
             VALUES (?1,?2,?3,301)",
        params![second, STEP_A, TASK],
    );
    assert!(
        matches!(
            duplicate,
            Err(rusqlite::Error::SqliteFailure(ref error, _)) if error.extended_code == 2067
        ),
        "UNIQUE(step_id) must reject a second grant's attempt to consume the Step"
    );
    // And the second grant was never spent.
    assert_eq!(
        scalar(
            &conn,
            &format!("SELECT uses_remaining FROM approval_grants WHERE grant_id='{second}'")
        )
        .unwrap(),
        1
    );
}

#[test]
fn only_an_enumerated_granted_step_can_consume_a_use() {
    let store = Store::open_in_memory(&FixedClock).unwrap();
    let conn = store.conn.lock().unwrap();
    seed_registry_and_task(&conn);
    insert_request(&conn, &[STEP_A], &[ARGS_A]);
    approve_and_grant(&conn, &[STEP_A], &[ARGS_A]);

    // STEP_C is a valid Task Step with the same canonical arguments digest and
    // structural scope as STEP_A, but it was not enumerated by the human.
    conn.execute(
        "INSERT INTO approval_grant_uses(grant_id,step_id,task_id,consumed_at_ms)
         VALUES (?1,?2,?3,300)",
        params![GRANT, STEP_C, TASK],
    )
    .unwrap_err();
    // Membership, rather than equal scope or digest, decides authority.
    conn.execute(
        "INSERT INTO approval_grant_uses(grant_id,step_id,task_id,consumed_at_ms)
         VALUES (?1,?2,?3,300)",
        params![GRANT, STEP_A, TASK],
    )
    .unwrap();
    let remaining: i64 = conn
        .query_row(
            "SELECT uses_remaining FROM approval_grants WHERE grant_id=?1",
            params![GRANT],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(remaining, 0);
}

#[test]
fn a_consuming_use_row_cannot_cross_the_grant_task_boundary() {
    let store = Store::open_in_memory(&FixedClock).unwrap();
    let conn = store.conn.lock().unwrap();
    seed_registry_and_task(&conn);
    conn.execute(
        "INSERT INTO tasks(task_id,kind,title,state,origin_kind,data_class_rank,policy_class_rank,
                           created_at_ms,updated_at_ms,max_model_calls,max_tool_calls,
                           max_attempts_per_step,capability_registry_generation)
         VALUES (?1,'USER_REQUEST','other','EXECUTING','USER_MESSAGE',1,3,0,1,12,24,3,1)",
        params![OTHER_TASK],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO task_steps(step_id,task_id,sequence,kind,status,attempt,plan_revision,
                                provider_id,capability_id,capability_version,input_digest,
                                idempotency_key,lease_generation)
         VALUES (?1,?2,0,'CAPABILITY','PLANNED',0,0,'calendar','calendar.event.create','1.0.0',?3,?4,0)",
        params![OTHER_STEP, OTHER_TASK, ARGS_A, "idk_000000000000000000000000000000000000000000000000000000000000000c"],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO step_capability_bindings(task_id,step_id,generation_id,descriptor_digest,
                                              capability_id,capability_version,provider_id,
                                              implementation_id)
         VALUES (?1,?2,1,?3,'calendar.event.create','1.0.0','calendar',NULL)",
        params![OTHER_TASK, OTHER_STEP, DESCRIPTOR],
    )
    .unwrap();
    insert_request(&conn, &[STEP_A], &[ARGS_A]);
    approve_and_grant(&conn, &[STEP_A], &[ARGS_A]);
    // A cross-task replay of an enumerated step_id is refused.
    conn.execute(
        "INSERT INTO approval_grant_uses(grant_id,step_id,task_id,consumed_at_ms)
         VALUES (?1,?2,?3,300)",
        params![GRANT, OTHER_STEP, OTHER_TASK],
    )
    .unwrap_err();
    // And so is a row whose own task_id disagrees with the grant.
    conn.execute(
        "INSERT INTO approval_grant_uses(grant_id,step_id,task_id,consumed_at_ms)
         VALUES (?1,?2,?3,300)",
        params![GRANT, STEP_A, OTHER_TASK],
    )
    .unwrap_err();
}

#[test]
fn a_granted_step_must_be_one_the_human_actually_saw() {
    let store = Store::open_in_memory(&FixedClock).unwrap();
    let conn = store.conn.lock().unwrap();
    seed_registry_and_task(&conn);
    insert_request(&conn, &[STEP_A, STEP_B], &[ARGS_A, ARGS_B]);
    approve_and_grant(&conn, &[STEP_A], &[ARGS_A]);

    // The granted subset is {STEP_A}. STEP_B was raised but never granted, so it
    // is not a member and cannot be added afterwards.
    conn.execute(
        "INSERT INTO approval_grant_members(grant_id,position,approval_id,step_id,
                                            arguments_digest,scope_digest)
         VALUES (?1,1,?2,?3,?4,?5)",
        params![GRANT, APPROVAL, STEP_B, ARGS_B, SCOPE_DIGEST],
    )
    .unwrap_err();
    // An action the human never saw at all is refused too.
    conn.execute(
        "INSERT INTO approval_grant_members(grant_id,position,approval_id,step_id,
                                            arguments_digest,scope_digest)
         VALUES (?1,1,?2,?3,?4,?5)",
        params![GRANT, APPROVAL, STEP_C, ARGS_A, SCOPE_DIGEST],
    )
    .unwrap_err();
    // Members are immutable after the grant exists.
    conn.execute(
        "DELETE FROM approval_grant_members WHERE grant_id=?1 AND step_id=?2",
        params![GRANT, STEP_A],
    )
    .unwrap_err();
    conn.execute(
        "UPDATE approval_grant_members SET step_id=?1 WHERE grant_id=?2 AND position=0",
        params![STEP_C, GRANT],
    )
    .unwrap_err();
}

// ---------------------------------------------------------------------------
// 15 and 16 — restart/reopen and independent-connection contention
// ---------------------------------------------------------------------------

#[test]
fn a_closed_store_reopens_with_the_same_authoritative_policy_and_approval_state() {
    let temp = TempDb::new();
    let closed: (i64, i64, i64, i64) = {
        let store = Store::open(&temp.0, &FixedClock).unwrap();
        {
            let conn = store.conn.lock().unwrap();
            seed_registry_and_task(&conn);
            insert_revision(&conn, 1, true);
            insert_rule(&conn, 1, "pol_0001", "REQUIRE_APPROVAL");
            activate_seed_revision(&conn, 1);
            conn.execute(
                "UPDATE policy_state SET active_revision_id=1 WHERE singleton=1",
                [],
            )
            .unwrap();
            insert_request(&conn, &[STEP_A], &[ARGS_A]);
            approve_and_grant(&conn, &[STEP_A], &[ARGS_A]);
            conn.execute(
                "INSERT INTO approval_grant_uses(grant_id,step_id,task_id,consumed_at_ms)
                 VALUES (?1,?2,?3,300)",
                params![GRANT, STEP_A, TASK],
            )
            .unwrap();
        }
        assert_eq!(
            store.checkpoint_for_close().unwrap(),
            crate::CheckpointOutcome::Complete
        );
        let conn = store.conn.lock().unwrap();
        (
            pointer(&conn, "SELECT active_revision_id FROM policy_state"),
            scalar(&conn, "SELECT count(*) FROM approval_requests").unwrap(),
            scalar(&conn, "SELECT count(*) FROM approval_grant_members").unwrap(),
            scalar(&conn, "SELECT count(*) FROM approval_grant_uses").unwrap(),
        )
    };
    // No -shm or -wal sidecar survives as a portable persisted asset.
    assert!(!temp.0.with_extension("sqlite-shm").exists());
    assert!(!temp.0.with_extension("sqlite-wal").exists());

    let reopened = Store::open(&temp.0, &FixedClock).unwrap();
    assert_eq!(reopened.schema_version().unwrap(), Migrations::LATEST);
    reopened.verify_integrity().unwrap();
    let conn = reopened.conn.lock().unwrap();
    assert_eq!(
        pointer(&conn, "SELECT active_revision_id FROM policy_state"),
        closed.0
    );
    assert_eq!(
        scalar(&conn, "SELECT count(*) FROM approval_requests").unwrap(),
        closed.1
    );
    assert_eq!(
        scalar(&conn, "SELECT count(*) FROM approval_grant_members").unwrap(),
        closed.2
    );
    assert_eq!(
        scalar(&conn, "SELECT count(*) FROM approval_grant_uses").unwrap(),
        closed.3
    );
    // The spend trigger's effect is durable, not recomputed.
    let remaining: i64 = conn
        .query_row(
            "SELECT uses_remaining FROM approval_grants WHERE grant_id=?1",
            params![GRANT],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(remaining, 0);
}

#[test]
fn independent_connections_cannot_both_consume_one_step() {
    let temp = TempDb::new();
    Store::open(&temp.0, &FixedClock).unwrap();
    {
        let conn = Connection::open(&temp.0).unwrap();
        conn.pragma_update(None, "foreign_keys", "ON").unwrap();
        seed_registry_and_task(&conn);
        insert_request(&conn, &[STEP_A], &[ARGS_A]);
        approve_and_grant(&conn, &[STEP_A], &[ARGS_A]);
    }

    // Two genuinely independent connections, each holding a writer reservation.
    let mut first = Connection::open(&temp.0).unwrap();
    let mut second = Connection::open(&temp.0).unwrap();
    for conn in [&first, &second] {
        conn.pragma_update(None, "foreign_keys", "ON").unwrap();
    }
    second.busy_timeout(std::time::Duration::ZERO).unwrap();
    let tx_first = first
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .unwrap();
    tx_first
        .execute(
            "INSERT INTO approval_grant_uses(grant_id,step_id,task_id,consumed_at_ms)
             VALUES (?1,?2,?3,300)",
            params![GRANT, STEP_A, TASK],
        )
        .unwrap();

    // The first independent connection holds SQLite's writer reservation after
    // inserting the use. A simultaneous second writer may be refused with
    // SQLITE_BUSY; classify only that exact outcome as contention.
    let busy = second.transaction_with_behavior(TransactionBehavior::Immediate);
    assert!(matches!(
        busy,
        Err(rusqlite::Error::SqliteFailure(ref error, _))
            if error.code == rusqlite::ErrorCode::DatabaseBusy
    ));
    drop(busy);
    tx_first.commit().unwrap();

    // Retry after the winner's commit. The competing connection now gets a
    // serialized view and must still be unable to consume or decrement again.
    let tx_second = second
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .unwrap();
    assert!(
        tx_second
            .execute(
                "INSERT INTO approval_grant_uses(grant_id,step_id,task_id,consumed_at_ms)
             VALUES (?1,?2,?3,301)",
                params![GRANT, STEP_A, TASK],
            )
            .is_err()
    );
    tx_second.commit().unwrap();
    drop(first);
    drop(second);

    // The winner is durable after reopening, with one use row and exactly one
    // decrement. The rejected competitor cannot spend it after restart either.
    let reopened = Connection::open(&temp.0).unwrap();
    reopened.pragma_update(None, "foreign_keys", "ON").unwrap();
    assert_eq!(
        scalar(&reopened, "SELECT count(*) FROM approval_grant_uses").unwrap(),
        1,
        "one and only one consumption committed"
    );
    assert_eq!(
        scalar(
            &reopened,
            "SELECT uses_remaining FROM approval_grants WHERE grant_id='grt_01JQ8ZA7B3KMW9Q4TVY7XN2RDP'"
        )
        .unwrap(),
        0
    );
    assert_eq!(
        text(
            &reopened,
            "SELECT status FROM approval_grants WHERE grant_id=?1",
            GRANT
        )
        .unwrap(),
        "EXHAUSTED"
    );
    assert!(
        reopened
            .execute(
                "INSERT INTO approval_grant_uses(grant_id,step_id,task_id,consumed_at_ms)
             VALUES (?1,?2,?3,302)",
                params![GRANT, STEP_A, TASK],
            )
            .is_err()
    );
    assert_eq!(
        scalar(&reopened, "SELECT count(*) FROM approval_grant_uses").unwrap(),
        1
    );
    assert_eq!(
        scalar(
            &reopened,
            "SELECT uses_remaining FROM approval_grants WHERE grant_id='grt_01JQ8ZA7B3KMW9Q4TVY7XN2RDP'"
        )
        .unwrap(),
        0
    );
}

#[test]
fn a_failed_transaction_leaves_no_partial_authoritative_state() {
    let store = Store::open_in_memory(&FixedClock).unwrap();
    {
        let conn = store.conn.lock().unwrap();
        seed_registry_and_task(&conn);
        insert_revision(&conn, 1, true);
        insert_rule(&conn, 1, "pol_0001", "REQUIRE_APPROVAL");
    }
    let event = policy_changed_event();
    let failed = store.transact(|tx: &mut Tx<'_>| {
        activate(tx, 2)?;
        tx.append_event(event, None)?;
        Err::<(), _>(StoreError::Sqlite)
    });
    // Revision 2 was never inserted, so the failure is a pure rollback.
    assert!(failed.is_err());
    let conn = store.conn.lock().unwrap();
    assert_eq!(
        scalar(&conn, "SELECT active_revision_id FROM policy_state").unwrap_or(0),
        0,
        "a rolled-back activation must leave the pointer untouched"
    );
    assert_eq!(count_kind_store(&conn, "POLICY_CHANGED"), 0);
    assert_eq!(
        scalar(&conn, "SELECT count(*) FROM policy_revisions").unwrap(),
        1
    );
}

fn typed_request_draft() -> ApprovalRequestStorageDraft {
    let step_id = StepId::new(STEP_A).unwrap();
    let scope_digest = digest_of(SCOPE).unwrap();
    let action = ApprovalRequestActionDraft {
        step_id: step_id.clone(),
        arguments_digest: Digest::new(ARGS_A).unwrap(),
        scope_json: SCOPE.to_owned(),
        scope_digest,
    };
    let actions = vec![action];
    ApprovalRequestStorageDraft {
        approval_id: ApprovalId::new(APPROVAL).unwrap(),
        task_id: TaskId::new(TASK).unwrap(),
        task_kind: TaskKind::UserRequest,
        plan_revision: 1,
        capability_id: CapabilityId::new("calendar.event.create").unwrap(),
        capability_version: SemVer::new("1.0.0").unwrap(),
        generation_id: 1,
        descriptor_digest: Digest::new(DESCRIPTOR).unwrap(),
        risk_class: RiskClass::ExternalWrite,
        side_effect_class: SideEffectClass::ExternalWrite,
        authorization: Authorization::ScopedGrant,
        data_class: DataClass::Personal,
        requested_by: RequestedBy::User,
        automation_context: ApprovalAutomationContext::Interactive,
        action_set_digest: approval_action_set_digest(&actions).unwrap(),
        actions,
        summary_kind: "calendar.event.create".to_owned(),
        summary: "Create one event on the primary calendar".to_owned(),
        raised_at: EpochMillis::new(100).unwrap(),
        expires_at: EpochMillis::new(1_800_100).unwrap(),
    }
}

#[test]
fn approval_storage_draft_debug_redacts_summary_and_scope_values() {
    let mut draft = typed_request_draft();
    draft.summary = "storage-summary-sentinel-2b07".to_owned();
    draft.actions[0].scope_json = r#"{"target":"storage-scope-sentinel-5d91"}"#.to_owned();

    let debug = format!("{draft:?}");
    assert!(!debug.contains("storage-summary-sentinel-2b07"));
    assert!(!debug.contains("storage-scope-sentinel-5d91"));
    assert!(debug.contains("summary_bytes"));
    assert!(debug.contains("action_count"));
}

fn approval_required_event(draft: &ApprovalRequestStorageDraft) -> EventDraft {
    let step = draft.actions.first().unwrap().step_id.clone();
    EventDraft {
        event: SereaEvent {
            envelope_version: EnvelopeVersion::new("1").unwrap(),
            surface: WireSurface::new(WireSurface::EVENT).unwrap(),
            message_id: EventId::new("evt_01JQ8ZA7B3KMW9Q4TVY7XN2RDP").unwrap(),
            seq: Seq::new(0),
            kind: EventKind::ApprovalRequired,
            occurred_at: Timestamp::from_epoch_millis(draft.raised_at),
            correlation_id: Some(draft.task_id.clone()),
            causation_id: None,
            actor: Actor {
                kind: ActorKind::Host,
                id: ActorId::new("serea-core").unwrap(),
                version: SemVer::new("1.0.0").unwrap(),
                extensions: Default::default(),
            },
            data_class: DataClass::Personal,
            trace: Some(Trace {
                task_id: Some(draft.task_id.clone()),
                step_id: Some(step.clone()),
                attempt: None,
                extensions: Default::default(),
            }),
            payload: json!({
                "approval_id": draft.approval_id.as_str(),
                "task_id": draft.task_id.as_str(),
                "step_id": step.as_str()
            })
            .as_object()
            .unwrap()
            .clone(),
            extensions: Default::default(),
        },
        retention_at: Some(EpochMillis::new(2_592_000_100).unwrap()),
    }
}

fn approval_expired_event(draft: &ApprovalRequestStorageDraft, now: EpochMillis) -> EventDraft {
    let mut event = approval_required_event(draft);
    event.event.kind = EventKind::ApprovalExpired;
    event.event.message_id = EventId::new("evt_01JQ8ZA7B3KMW9Q4TVY7XN2RDQ").unwrap();
    event.event.occurred_at = Timestamp::from_epoch_millis(now);
    event
}

fn approval_store(path: &Path) -> Store {
    approval_store_with_tool_budget(path, 24)
}

fn approval_store_with_tool_budget(path: &Path, max_tool_calls: i64) -> Store {
    seed_v4(path);
    {
        let conn = Connection::open(path).unwrap();
        conn.pragma_update(None, "foreign_keys", "ON").unwrap();
        seed_registry_and_task(&conn);
        conn.execute(
            "UPDATE tasks SET plan_revision=1,max_tool_calls=?2 WHERE task_id=?1",
            rusqlite::params![TASK, max_tool_calls],
        )
        .unwrap();
        conn.execute(
            "UPDATE task_steps SET plan_revision=1 WHERE task_id=?1",
            [TASK],
        )
        .unwrap();
    }
    Store::open(path, &FixedClock).unwrap()
}

fn activate_test_policy(store: &Store) {
    let conn = store.conn.lock().unwrap();
    insert_revision(&conn, 1, false);
    insert_rule(&conn, 1, "approval_wait_test", "ALLOW");
    activate_seed_revision(&conn, 1);
    conn.execute(
        "UPDATE policy_state SET active_revision_id=1 WHERE singleton=1",
        [],
    )
    .unwrap();
}

fn install_wait_step(store: &Store) {
    let input = format!(
        "{{\"actions\":[{{\"arguments_digest\":\"{ARGS_A}\",\"step_id\":\"{STEP_A}\"}}],\"gate\":\"R2_ACTION_GROUP\"}}"
    );
    let input_digest = store
        .transact(|tx| {
            tx.put_blob(input.as_bytes(), DataClass::Personal)
                .map(|blob| blob.digest().to_string())
        })
        .unwrap();
    let conn = store.conn.lock().unwrap();
    conn.execute(
        "UPDATE task_steps SET sequence=sequence+10 WHERE task_id=?1",
        [TASK],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO task_steps(step_id,task_id,sequence,kind,status,attempt,plan_revision,input_digest)
         VALUES (?1,?2,0,'WAIT_APPROVAL','PLANNED',0,1,?3)",
        params![MODEL_STEP, TASK, input_digest],
    )
    .unwrap();
}

fn approval_response_event(
    draft: &ApprovalResponseStorageDraft,
    kind: EventKind,
    step_id: &str,
) -> EventDraft {
    EventDraft {
        event: SereaEvent {
            envelope_version: EnvelopeVersion::new("1").unwrap(),
            surface: WireSurface::new(WireSurface::EVENT).unwrap(),
            message_id: EventId::new("evt_01JQ8ZA7B3KMW9Q4TVY7XN2RDQ").unwrap(),
            seq: Seq::new(0),
            kind,
            occurred_at: Timestamp::from_epoch_millis(draft.decided_at),
            correlation_id: Some(TaskId::new(TASK).unwrap()),
            causation_id: None,
            actor: Actor {
                kind: ActorKind::User,
                id: draft.actor_id.clone(),
                version: SemVer::new("1.0.0").unwrap(),
                extensions: Default::default(),
            },
            data_class: DataClass::Personal,
            trace: Some(Trace {
                task_id: Some(TaskId::new(TASK).unwrap()),
                step_id: Some(StepId::new(step_id).unwrap()),
                attempt: None,
                extensions: Default::default(),
            }),
            payload: json!({
                "approval_id": draft.approval_id.as_str(),
                "task_id": TASK,
                "step_id": step_id
            })
            .as_object()
            .unwrap()
            .clone(),
            extensions: Default::default(),
        },
        retention_at: Some(EpochMillis::new(2_592_000_100).unwrap()),
    }
}

fn authenticated_grant_storage_draft() -> ApprovalResponseStorageDraft {
    ApprovalResponseStorageDraft {
        approval_id: ApprovalId::new(APPROVAL).unwrap(),
        grant_id: Some(GrantId::new("grt_01JQ8ZA7B3KMW9Q4TVY7XN2RDP").unwrap()),
        actor_id: ActorId::new("user_01JQ8ZA7B3KMW9Q4TVY7XN2RDP").unwrap(),
        request_action_set_digest: typed_request_draft().action_set_digest,
        checked_at: EpochMillis::new(200).unwrap(),
        decision: ApprovalResponseDecisionStorage::Grant,
        approved_step_ids: vec![StepId::new(STEP_A).unwrap()],
        decided_at: EpochMillis::new(200).unwrap(),
        expires_at: Some(EpochMillis::new(1_800_000).unwrap()),
        granted_by: "USER",
        actor_device_id: Some("dev_01JQ8ZA7B3KMW9Q4TVY7XN2RDP".into()),
        actor_session_id: Some("ses_01JQ8ZA7B3KMW9Q4TVY7XN2RDP".into()),
        auth_strength: "SESSION",
        elevated_ref: None,
    }
}

fn two_action_request_draft() -> ApprovalRequestStorageDraft {
    let mut draft = typed_request_draft();
    let action = ApprovalRequestActionDraft {
        step_id: StepId::new(STEP_B).unwrap(),
        arguments_digest: Digest::new(ARGS_B).unwrap(),
        scope_json: SCOPE.to_owned(),
        scope_digest: digest_of(SCOPE).unwrap(),
    };
    draft.actions.push(action);
    draft.actions.sort_by(|a, b| {
        a.step_id
            .as_str()
            .as_bytes()
            .cmp(b.step_id.as_str().as_bytes())
    });
    draft.action_set_digest = approval_action_set_digest(&draft.actions).unwrap();
    draft
}

fn two_action_grant_storage_draft() -> ApprovalResponseStorageDraft {
    let mut draft = authenticated_grant_storage_draft();
    draft.request_action_set_digest = two_action_request_draft().action_set_digest;
    draft.approved_step_ids = vec![StepId::new(STEP_A).unwrap(), StepId::new(STEP_B).unwrap()];
    draft
}

fn revocation_storage_draft() -> ApprovalRevocationStorageDraft {
    ApprovalRevocationStorageDraft {
        approval_id: ApprovalId::new(APPROVAL).unwrap(),
        grant_id: GrantId::new(GRANT).unwrap(),
        actor_id: ActorId::new("user_01JQ8ZA7B3KMW9Q4TVY7XN2RDP").unwrap(),
        actor_class: "USER",
        device_id: Some("dev_01JQ8ZA7B3KMW9Q4TVY7XN2RDP".into()),
        session_id: Some("ses_01JQ8ZA7B3KMW9Q4TVY7XN2RDP".into()),
        revoked_at: EpochMillis::new(400).unwrap(),
        checked_at: EpochMillis::new(400).unwrap(),
    }
}

fn approval_revoked_event(draft: &ApprovalRevocationStorageDraft) -> EventDraft {
    let mut payload = serde_json::Map::new();
    payload.insert(
        "actor_class".into(),
        serde_json::Value::String(draft.actor_class.into()),
    );
    payload.insert(
        "approval_id".into(),
        serde_json::Value::String(draft.approval_id.to_string()),
    );
    payload.insert(
        "device_id".into(),
        serde_json::Value::String(draft.device_id.clone().unwrap()),
    );
    payload.insert(
        "grant_id".into(),
        serde_json::Value::String(draft.grant_id.to_string()),
    );
    payload.insert(
        "revoked_at".into(),
        serde_json::Value::from(draft.revoked_at.get()),
    );
    payload.insert(
        "session_id".into(),
        serde_json::Value::String(draft.session_id.clone().unwrap()),
    );
    payload.insert("step_id".into(), serde_json::Value::String(STEP_A.into()));
    payload.insert("task_id".into(), serde_json::Value::String(TASK.into()));
    EventDraft {
        event: SereaEvent {
            envelope_version: EnvelopeVersion::new("1").unwrap(),
            surface: WireSurface::new(WireSurface::EVENT).unwrap(),
            message_id: EventId::new("evt_01JQ8ZA7B3KMW9Q4TVY7XN2RDS").unwrap(),
            seq: Seq::new(0),
            kind: EventKind::ApprovalRevoked,
            occurred_at: Timestamp::from_epoch_millis(draft.revoked_at),
            correlation_id: Some(TaskId::new(TASK).unwrap()),
            causation_id: None,
            actor: Actor {
                kind: ActorKind::User,
                id: draft.actor_id.clone(),
                version: SemVer::new("1.0.0").unwrap(),
                extensions: Default::default(),
            },
            data_class: DataClass::Personal,
            trace: Some(Trace {
                task_id: Some(TaskId::new(TASK).unwrap()),
                step_id: Some(StepId::new(STEP_A).unwrap()),
                attempt: None,
                extensions: Default::default(),
            }),
            payload,
            extensions: Default::default(),
        },
        retention_at: Some(EpochMillis::new(2_592_000_400).unwrap()),
    }
}

fn approval_expired_unused_event(now: EpochMillis) -> EventDraft {
    EventDraft {
        event: SereaEvent {
            envelope_version: EnvelopeVersion::new("1").unwrap(),
            surface: WireSurface::new(WireSurface::EVENT).unwrap(),
            message_id: EventId::new("evt_01JQ8ZA7B3KMW9Q4TVY7XN2RDR").unwrap(),
            seq: Seq::new(0),
            kind: EventKind::ApprovalExpiredUnused,
            occurred_at: Timestamp::from_epoch_millis(now),
            correlation_id: Some(TaskId::new(TASK).unwrap()),
            causation_id: None,
            actor: Actor {
                kind: ActorKind::Host,
                id: ActorId::new("host_01JQ8ZA7B3KMW9Q4TVY7XN2RDS").unwrap(),
                version: SemVer::new("1.0.0").unwrap(),
                extensions: Default::default(),
            },
            data_class: DataClass::Personal,
            trace: Some(Trace {
                task_id: Some(TaskId::new(TASK).unwrap()),
                step_id: Some(StepId::new(STEP_A).unwrap()),
                attempt: None,
                extensions: Default::default(),
            }),
            payload: json!({
                "approval_id": APPROVAL,
                "grant_id": GRANT,
                "task_id": TASK,
                "step_id": STEP_A,
            })
            .as_object()
            .unwrap()
            .clone(),
            extensions: Default::default(),
        },
        retention_at: Some(EpochMillis::new(now.get() + 2_592_000_000).unwrap()),
    }
}

#[test]
fn typed_request_write_recomputes_r2_commitment_and_commits_routing_event() {
    let path = TempDb::new();
    let store = approval_store(&path.0);
    activate_test_policy(&store);
    let draft = typed_request_draft();
    let original_digest = draft.action_set_digest.clone();
    store
        .create_approval_request(draft.clone(), approval_required_event(&draft))
        .unwrap();
    let conn = store.conn.lock().unwrap();
    assert_eq!(
        scalar(&conn, "SELECT count(*) FROM approval_requests").unwrap(),
        1
    );
    assert_eq!(
        scalar(&conn, "SELECT count(*) FROM approval_request_actions").unwrap(),
        1
    );
    assert_eq!(
        text(
            &conn,
            "SELECT action_set_digest FROM approval_requests WHERE approval_id=?1",
            APPROVAL
        )
        .unwrap(),
        original_digest.as_str()
    );
    assert_eq!(count_kind_store(&conn, "APPROVAL_REQUIRED"), 1);
    drop(conn);
    store.verify_integrity().unwrap();
}

#[test]
fn task_wait_and_complete_approval_request_commit_together() {
    struct Audit;
    impl TaskAuditParticipant for Audit {
        fn records(&self, facts: &DurableTransition) -> Result<JournalRecords, StoreError> {
            Ok(vec![JournalRecord {
                kind: JournalKind::TaskStateChanged,
                state_from: facts.task_from().map(|state| state.wire_name().to_owned()),
                state_to: Some(facts.task_to().wire_name().to_owned()),
                reason: None,
                payload_json: b"{}".to_vec(),
            }])
        }
    }
    struct Events;
    impl EventParticipant for Events {
        fn events(&self, _: &DurableTransition) -> Result<Vec<EventDraft>, StoreError> {
            Ok(Vec::new())
        }
    }

    let path = TempDb::new();
    let store = approval_store(&path.0);
    activate_test_policy(&store);
    install_wait_step(&store);
    let actor = ActorId::new("local-user").unwrap();
    let version = SemVer::new("1.0.0").unwrap();
    let context = TransitionContext {
        actor_kind: ActorKind::User,
        actor_id: &actor,
        actor_version: &version,
        causation_id: None,
    };
    let draft = typed_request_draft();
    store
        .transact_with_participants(&Audit, &Events, |tx| {
            tx.raise_approval_request_and_wait(
                draft.clone(),
                approval_required_event(&draft),
                1,
                &Digest::new(RULES).unwrap(),
                draft.raised_at,
                &context,
            )
        })
        .unwrap();
    let conn = store.conn.lock().unwrap();
    assert_eq!(
        text(&conn, "SELECT state FROM tasks WHERE task_id=?1", TASK).unwrap(),
        TaskState::WaitingApproval.wire_name()
    );
    assert_eq!(
        scalar(
            &conn,
            "SELECT count(*) FROM approval_requests WHERE status='PENDING'"
        )
        .unwrap(),
        1
    );
    assert_eq!(
        scalar(&conn, "SELECT count(*) FROM approval_request_actions").unwrap(),
        1
    );
    assert_eq!(
        scalar(
            &conn,
            "SELECT count(*) FROM event_content WHERE kind='APPROVAL_REQUIRED'"
        )
        .unwrap(),
        1
    );
}

#[test]
fn approval_wait_rolls_back_when_the_task_audit_participant_fails() {
    struct RejectAudit;
    impl TaskAuditParticipant for RejectAudit {
        fn records(&self, _: &DurableTransition) -> Result<JournalRecords, StoreError> {
            Err(StoreError::AuditRejected)
        }
    }
    struct Events;
    impl EventParticipant for Events {
        fn events(&self, _: &DurableTransition) -> Result<Vec<EventDraft>, StoreError> {
            Ok(Vec::new())
        }
    }

    let path = TempDb::new();
    let store = approval_store(&path.0);
    activate_test_policy(&store);
    install_wait_step(&store);
    let actor = ActorId::new("local-user").unwrap();
    let version = SemVer::new("1.0.0").unwrap();
    let context = TransitionContext {
        actor_kind: ActorKind::User,
        actor_id: &actor,
        actor_version: &version,
        causation_id: None,
    };
    let draft = typed_request_draft();
    assert_eq!(
        store.transact_with_participants(&RejectAudit, &Events, |tx| {
            tx.raise_approval_request_and_wait(
                draft.clone(),
                approval_required_event(&draft),
                1,
                &Digest::new(RULES).unwrap(),
                draft.raised_at,
                &context,
            )
        }),
        Err(StoreError::AuditRejected)
    );
    let conn = store.conn.lock().unwrap();
    assert_eq!(
        text(&conn, "SELECT state FROM tasks WHERE task_id=?1", TASK).unwrap(),
        "EXECUTING"
    );
    assert_eq!(
        scalar(&conn, "SELECT count(*) FROM approval_requests").unwrap(),
        0
    );
    assert_eq!(
        scalar(&conn, "SELECT count(*) FROM approval_request_actions").unwrap(),
        0
    );
    assert_eq!(
        scalar(
            &conn,
            "SELECT count(*) FROM event_content WHERE kind='APPROVAL_REQUIRED'"
        )
        .unwrap(),
        0
    );
}

#[test]
fn approval_wait_refuses_stale_policy_revision_identity() {
    struct Audit;
    impl TaskAuditParticipant for Audit {
        fn records(&self, _: &DurableTransition) -> Result<JournalRecords, StoreError> {
            Ok(Vec::new())
        }
    }
    struct Events;
    impl EventParticipant for Events {
        fn events(&self, _: &DurableTransition) -> Result<Vec<EventDraft>, StoreError> {
            Ok(Vec::new())
        }
    }

    let path = TempDb::new();
    let store = approval_store(&path.0);
    activate_test_policy(&store);
    let actor = ActorId::new("local-user").unwrap();
    let version = SemVer::new("1.0.0").unwrap();
    let context = TransitionContext {
        actor_kind: ActorKind::User,
        actor_id: &actor,
        actor_version: &version,
        causation_id: None,
    };
    let draft = typed_request_draft();
    assert_eq!(
        store.transact_with_participants(&Audit, &Events, |tx| {
            tx.raise_approval_request_and_wait(
                draft.clone(),
                approval_required_event(&draft),
                2,
                &Digest::new(RULES).unwrap(),
                draft.raised_at,
                &context,
            )
        }),
        Err(StoreError::InvalidPolicyRevision)
    );
    let conn = store.conn.lock().unwrap();
    assert_eq!(
        text(&conn, "SELECT state FROM tasks WHERE task_id=?1", TASK).unwrap(),
        "EXECUTING"
    );
    assert_eq!(
        scalar(&conn, "SELECT count(*) FROM approval_requests").unwrap(),
        0
    );
    assert_eq!(count_kind_store(&conn, "APPROVAL_REQUIRED"), 0);
}

struct ApprovalTaskAudit;
impl TaskAuditParticipant for ApprovalTaskAudit {
    fn records(&self, facts: &DurableTransition) -> Result<JournalRecords, StoreError> {
        let mut rows = Vec::new();
        let reason = facts.reason().cloned().or_else(|| match facts.operation() {
            crate::AuditOperation::ApprovalWaitStarted => {
                serea_protocol::ReasonCode::new("AWAIT_APPROVAL").ok()
            }
            crate::AuditOperation::ApprovalWakeApplied if facts.task_to() == TaskState::Ready => {
                serea_protocol::ReasonCode::new("RESUME_READY").ok()
            }
            _ => None,
        });
        match facts.operation() {
            crate::AuditOperation::ApprovalWaitStarted => rows.push(JournalRecord {
                kind: JournalKind::StepAttemptStarted,
                state_from: facts.step_from().map(|status| status.as_str().to_owned()),
                state_to: facts.step_to().map(|status| status.as_str().to_owned()),
                reason: reason.clone(),
                payload_json: b"{}".to_vec(),
            }),
            crate::AuditOperation::ApprovalWakeApplied => rows.push(JournalRecord {
                kind: if facts.task_to() == TaskState::Ready {
                    JournalKind::StepCommitted
                } else {
                    JournalKind::StepFailed
                },
                state_from: facts.step_from().map(|status| status.as_str().to_owned()),
                state_to: facts.step_to().map(|status| status.as_str().to_owned()),
                reason: reason.clone(),
                payload_json: b"{}".to_vec(),
            }),
            _ => return Err(StoreError::AuditRejected),
        }
        if let Some(from) = facts.task_from().filter(|from| *from != facts.task_to()) {
            rows.push(JournalRecord {
                kind: JournalKind::TaskStateChanged,
                state_from: Some(from.wire_name().to_owned()),
                state_to: Some(facts.task_to().wire_name().to_owned()),
                reason: reason.clone(),
                payload_json: b"{}".to_vec(),
            });
            if facts.task_to().is_terminal() {
                rows.push(JournalRecord {
                    kind: JournalKind::TaskTerminal,
                    state_from: Some(from.wire_name().to_owned()),
                    state_to: Some(facts.task_to().wire_name().to_owned()),
                    reason: reason.clone(),
                    payload_json: b"{}".to_vec(),
                });
            }
        }
        Ok(rows)
    }
}

struct ApprovalTaskEvents;
impl EventParticipant for ApprovalTaskEvents {
    fn events(&self, _: &DurableTransition) -> Result<Vec<EventDraft>, StoreError> {
        Ok(Vec::new())
    }
}

fn waiting_approval_store() -> (TempDb, Store, ApprovalRequestStorageDraft) {
    let path = TempDb::new();
    let store = approval_store(&path.0);
    activate_test_policy(&store);
    install_wait_step(&store);
    let draft = typed_request_draft();
    let actor = ActorId::new("local-user").unwrap();
    let version = SemVer::new("1.0.0").unwrap();
    let context = TransitionContext {
        actor_kind: ActorKind::User,
        actor_id: &actor,
        actor_version: &version,
        causation_id: None,
    };
    store
        .transact_with_participants(&ApprovalTaskAudit, &ApprovalTaskEvents, |tx| {
            tx.raise_approval_request_and_wait(
                draft.clone(),
                approval_required_event(&draft),
                1,
                &Digest::new(RULES).unwrap(),
                draft.raised_at,
                &context,
            )
        })
        .unwrap();
    (path, store, draft)
}

fn persist_approval_wake(store: &Store, outcome: &str) -> ApprovalLifecycleWake {
    let conn = store.conn.lock().unwrap();
    let source_seq: i64 = conn
        .query_row(
            "SELECT last_allocated_seq FROM event_store_state WHERE singleton=1",
            [],
            |row| row.get(0),
        )
        .unwrap();
    let source_event_id = EventId::new("evt_01JQ8ZA7B3KMW9Q4TVY7XN2RDQ").unwrap();
    conn.execute(
        "INSERT INTO approval_lifecycle_wakes(source_event_id,source_seq,approval_id,task_id,step_id,outcome_kind,created_at_ms)
         VALUES (?1,?2,?3,?4,?5,?6,250)",
        params![source_event_id.as_str(), source_seq, APPROVAL, TASK, STEP_A, outcome],
    )
    .unwrap();
    ApprovalLifecycleWake {
        source_event_id,
        source_seq: source_seq as u64,
        approval_id: ApprovalId::new(APPROVAL).unwrap(),
        task_id: TaskId::new(TASK).unwrap(),
        step_id: StepId::new(STEP_A).unwrap(),
        outcome: match outcome {
            "GRANTED" => crate::ApprovalLifecycleOutcome::Granted,
            "DENIED" => crate::ApprovalLifecycleOutcome::Denied,
            _ => unreachable!(),
        },
        created_at: EpochMillis::new(250).unwrap(),
    }
}

#[test]
fn granted_approval_wake_resumes_the_wait_step_and_acknowledges_atomically() {
    let (_path, store, request) = waiting_approval_store();
    let response = authenticated_grant_storage_draft();
    store
        .respond_to_approval(
            response.clone(),
            approval_response_event(&response, EventKind::ApprovalGranted, STEP_A),
        )
        .unwrap();
    let wake = persist_approval_wake(&store, "GRANTED");
    let actor = ActorId::new("serea-core").unwrap();
    let version = SemVer::new("1.0.0").unwrap();
    let context = TransitionContext {
        actor_kind: ActorKind::Host,
        actor_id: &actor,
        actor_version: &version,
        causation_id: None,
    };
    let outcome = store
        .transact_with_participants(&ApprovalTaskAudit, &ApprovalTaskEvents, |tx| {
            tx.apply_approval_lifecycle_wake(&wake, EpochMillis::new(300).unwrap(), &context)
        })
        .unwrap();
    assert_eq!(
        outcome,
        crate::ApprovalWakeApplyOutcome::Applied {
            task_id: TaskId::new(TASK).unwrap(),
            state: TaskState::Ready,
        }
    );
    // SUCCEEDED steps must retain the exact host-written result document, not
    // just its digest. The reference is the durable lookup path used on reload.
    let result_ref_count: i64 = store
        .conn
        .lock()
        .unwrap()
        .query_row(
            "SELECT count(*) FROM step_blob_refs r JOIN task_steps s USING(step_id)
          WHERE s.task_id=?1 AND s.step_id=?2 AND r.role='RESULT' AND r.digest=s.result_digest",
            params![TASK, MODEL_STEP],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(result_ref_count, 1);
    let conn = store.conn.lock().unwrap();
    assert_eq!(
        text(&conn, "SELECT state FROM tasks WHERE task_id=?1", TASK).unwrap(),
        "READY"
    );
    assert_eq!(
        text_pair(
            &conn,
            "SELECT status FROM task_steps WHERE task_id=?1 AND step_id=?2",
            TASK,
            MODEL_STEP
        )
        .unwrap(),
        "SUCCEEDED"
    );
    assert_eq!(
        scalar(&conn, "SELECT count(*) FROM approval_lifecycle_wakes").unwrap(),
        0
    );
    assert_eq!(
        scalar(&conn, "SELECT count(*) FROM approval_grant_uses").unwrap(),
        0
    );
    assert_eq!(request.approval_id.as_str(), APPROVAL);
}

#[test]
fn denied_approval_wake_fails_the_wait_step_without_revoking_any_grant() {
    let (_path, store, _) = waiting_approval_store();
    let mut response = authenticated_grant_storage_draft();
    response.decision = ApprovalResponseDecisionStorage::Deny;
    response.grant_id = None;
    response.approved_step_ids.clear();
    response.expires_at = None;
    let event = approval_response_event(&response, EventKind::ApprovalDenied, STEP_A);
    store.respond_to_approval(response, event).unwrap();
    let wake = persist_approval_wake(&store, "DENIED");
    let actor = ActorId::new("serea-core").unwrap();
    let version = SemVer::new("1.0.0").unwrap();
    let context = TransitionContext {
        actor_kind: ActorKind::Host,
        actor_id: &actor,
        actor_version: &version,
        causation_id: None,
    };
    let outcome = store
        .transact_with_participants(&ApprovalTaskAudit, &ApprovalTaskEvents, |tx| {
            tx.apply_approval_lifecycle_wake(&wake, EpochMillis::new(300).unwrap(), &context)
        })
        .unwrap();
    assert_eq!(
        outcome,
        crate::ApprovalWakeApplyOutcome::Applied {
            task_id: TaskId::new(TASK).unwrap(),
            state: TaskState::Failed,
        }
    );
    let conn = store.conn.lock().unwrap();
    assert_eq!(
        text(&conn, "SELECT state FROM tasks WHERE task_id=?1", TASK).unwrap(),
        "FAILED"
    );
    assert_eq!(
        text_pair(
            &conn,
            "SELECT status FROM task_steps WHERE task_id=?1 AND step_id=?2",
            TASK,
            MODEL_STEP
        )
        .unwrap(),
        "FAILED"
    );
    assert_eq!(
        text_pair(
            &conn,
            "SELECT error_code FROM task_steps WHERE task_id=?1 AND step_id=?2",
            TASK,
            MODEL_STEP
        )
        .unwrap(),
        "APPROVAL_DENIED"
    );
    assert_eq!(
        scalar(&conn, "SELECT count(*) FROM approval_lifecycle_wakes").unwrap(),
        0
    );
    assert_eq!(
        scalar(&conn, "SELECT count(*) FROM approval_grants").unwrap(),
        0
    );
}

#[test]
fn task_event_failure_rolls_back_wake_and_keeps_it_for_restart_retry() {
    struct RejectEvents;
    impl EventParticipant for RejectEvents {
        fn events(&self, _: &DurableTransition) -> Result<Vec<EventDraft>, StoreError> {
            Err(StoreError::CanonicalJson)
        }
    }

    let (path, store, _) = waiting_approval_store();
    let response = authenticated_grant_storage_draft();
    store
        .respond_to_approval(
            response.clone(),
            approval_response_event(&response, EventKind::ApprovalGranted, STEP_A),
        )
        .unwrap();
    let wake = persist_approval_wake(&store, "GRANTED");
    let actor = ActorId::new("serea-core").unwrap();
    let version = SemVer::new("1.0.0").unwrap();
    let context = TransitionContext {
        actor_kind: ActorKind::Host,
        actor_id: &actor,
        actor_version: &version,
        causation_id: None,
    };
    assert_eq!(
        store.transact_with_participants(&ApprovalTaskAudit, &RejectEvents, |tx| {
            tx.apply_approval_lifecycle_wake(&wake, EpochMillis::new(300).unwrap(), &context)
                .map(|_| ())
        }),
        Err(StoreError::CanonicalJson)
    );
    drop(store);
    let reopened = Store::open(&path.0, &FixedClock).unwrap();
    let conn = reopened.conn.lock().unwrap();
    assert_eq!(
        text(&conn, "SELECT state FROM tasks WHERE task_id=?1", TASK).unwrap(),
        "WAITING_APPROVAL"
    );
    assert_eq!(
        text_pair(
            &conn,
            "SELECT status FROM task_steps WHERE task_id=?1 AND step_id=?2",
            TASK,
            MODEL_STEP
        )
        .unwrap(),
        "WAITING"
    );
    assert_eq!(
        scalar(&conn, "SELECT count(*) FROM approval_lifecycle_wakes").unwrap(),
        1
    );
    assert_eq!(
        text(
            &conn,
            "SELECT status FROM approval_grants WHERE grant_id=?1",
            GRANT
        )
        .unwrap(),
        "ACTIVE"
    );
    assert_eq!(
        scalar(
            &conn,
            "SELECT count(*) FROM task_journal WHERE journal_kind='STEP_COMMITTED'"
        )
        .unwrap(),
        0
    );
}

#[test]
fn a_cancelled_task_is_never_revived_by_a_later_grant_wake() {
    let (_path, store, _) = waiting_approval_store();
    let response = authenticated_grant_storage_draft();
    store
        .respond_to_approval(
            response.clone(),
            approval_response_event(&response, EventKind::ApprovalGranted, STEP_A),
        )
        .unwrap();
    let wake = persist_approval_wake(&store, "GRANTED");
    store.conn.lock().unwrap().execute(
        "UPDATE tasks SET state='CANCELLED',cancelled_at_ms=275,cancelled_by='USER',updated_at_ms=275
          WHERE task_id=?1 AND state='WAITING_APPROVAL'",
        [TASK],
    ).unwrap();
    let actor = ActorId::new("serea-core").unwrap();
    let version = SemVer::new("1.0.0").unwrap();
    let context = TransitionContext {
        actor_kind: ActorKind::Host,
        actor_id: &actor,
        actor_version: &version,
        causation_id: None,
    };
    assert_eq!(
        store
            .transact_with_participants(&ApprovalTaskAudit, &ApprovalTaskEvents, |tx| {
                tx.apply_approval_lifecycle_wake(&wake, EpochMillis::new(300).unwrap(), &context)
            })
            .unwrap(),
        crate::ApprovalWakeApplyOutcome::AcknowledgedStale {
            task_id: TaskId::new(TASK).unwrap()
        }
    );
    let conn = store.conn.lock().unwrap();
    assert_eq!(
        text(&conn, "SELECT state FROM tasks WHERE task_id=?1", TASK).unwrap(),
        "CANCELLED"
    );
    assert_eq!(
        text_pair(
            &conn,
            "SELECT status FROM task_steps WHERE task_id=?1 AND step_id=?2",
            TASK,
            MODEL_STEP
        )
        .unwrap(),
        "WAITING"
    );
    assert_eq!(
        scalar(&conn, "SELECT count(*) FROM approval_lifecycle_wakes").unwrap(),
        0
    );
}

#[test]
fn typed_request_rejects_digest_scope_order_and_event_mismatch_without_partial_rows() {
    let path = TempDb::new();
    let store = approval_store(&path.0);
    let mut draft = typed_request_draft();
    draft.action_set_digest = Digest::new(BAD).unwrap();
    assert!(matches!(
        store.create_approval_request(draft.clone(), approval_required_event(&draft)),
        Err(StoreError::InvalidApprovalRequest)
    ));
    draft = typed_request_draft();
    draft.actions[0].scope_json = r#"{"calendar":"*"}"#.to_owned();
    draft.actions[0].scope_digest = digest_of(&draft.actions[0].scope_json).unwrap();
    draft.action_set_digest = approval_action_set_digest(&draft.actions).unwrap();
    assert!(matches!(
        store.create_approval_request(draft.clone(), approval_required_event(&draft)),
        Err(StoreError::InvalidApprovalRequest)
    ));
    draft = typed_request_draft();
    let mut invalid_event = approval_required_event(&draft);
    invalid_event.event.payload = json!({
        "approval_id": draft.approval_id.as_str(),
        "task_id": draft.task_id.as_str(),
        "step_id": STEP_B
    })
    .as_object()
    .unwrap()
    .clone();
    assert!(matches!(
        store.create_approval_request(draft, invalid_event),
        Err(StoreError::InvalidApprovalEvent)
    ));
    let conn = store.conn.lock().unwrap();
    assert_eq!(
        scalar(&conn, "SELECT count(*) FROM approval_requests").unwrap(),
        0
    );
    assert_eq!(
        scalar(&conn, "SELECT count(*) FROM approval_request_actions").unwrap(),
        0
    );
    assert_eq!(count_kind_store(&conn, "APPROVAL_REQUIRED"), 0);
}

#[test]
fn failed_event_append_rolls_back_request_and_complete_action_set() {
    let path = TempDb::new();
    let store = approval_store(&path.0);
    {
        let conn = store.conn.lock().unwrap();
        conn.execute(
            "UPDATE event_store_state SET retained_count=1000000 WHERE singleton=1",
            [],
        )
        .unwrap();
    }
    let draft = typed_request_draft();
    assert!(matches!(
        store.create_approval_request(draft.clone(), approval_required_event(&draft)),
        Err(StoreError::EventStoreCapacity)
    ));
    let conn = store.conn.lock().unwrap();
    assert_eq!(
        scalar(&conn, "SELECT count(*) FROM approval_requests").unwrap(),
        0
    );
    assert_eq!(
        scalar(&conn, "SELECT count(*) FROM approval_request_actions").unwrap(),
        0
    );
    assert_eq!(count_kind_store(&conn, "APPROVAL_REQUIRED"), 0);
}

#[test]
fn authenticated_response_commits_approved_subset_grant_members_seal_and_event_together() {
    let path = TempDb::new();
    let store = approval_store(&path.0);
    let request = typed_request_draft();
    store
        .create_approval_request(request.clone(), approval_required_event(&request))
        .unwrap();
    let response = authenticated_grant_storage_draft();
    let result = store
        .respond_to_approval(
            response.clone(),
            approval_response_event(&response, EventKind::ApprovalGranted, STEP_A),
        )
        .unwrap();
    assert_eq!(
        result,
        ApprovalResponseStorageOutcome::Granted(response.grant_id.clone().unwrap())
    );
    let conn = store.conn.lock().unwrap();
    assert_eq!(
        text(
            &conn,
            "SELECT status FROM approval_requests WHERE approval_id=?1",
            APPROVAL
        )
        .unwrap(),
        "APPROVED"
    );
    assert_eq!(
        text(
            &conn,
            "SELECT status FROM approval_grants WHERE approval_id=?1",
            APPROVAL
        )
        .unwrap(),
        "ACTIVE"
    );
    assert_eq!(
        scalar(&conn, "SELECT count(*) FROM approval_grant_members").unwrap(),
        1
    );
    assert_eq!(
        scalar(&conn, "SELECT count(*) FROM approval_grant_seals").unwrap(),
        1
    );
    assert_eq!(count_kind_store(&conn, "APPROVAL_GRANTED"), 1);
    drop(conn);
    store.verify_integrity().unwrap();
}

#[test]
fn authenticated_response_refuses_unrequested_step_and_duplicate_deny_is_idempotent() {
    let path = TempDb::new();
    let store = approval_store(&path.0);
    let request = typed_request_draft();
    store
        .create_approval_request(request.clone(), approval_required_event(&request))
        .unwrap();
    let mut response = authenticated_grant_storage_draft();
    response.approved_step_ids = vec![StepId::new(STEP_B).unwrap()];
    let invalid = approval_response_event(&response, EventKind::ApprovalGranted, STEP_B);
    assert!(matches!(
        store.respond_to_approval(response, invalid),
        Err(StoreError::InvalidApprovalResponse)
    ));
    assert_eq!(
        scalar(
            &store.conn.lock().unwrap(),
            "SELECT count(*) FROM approval_grants"
        )
        .unwrap(),
        0
    );
    let mut deny = authenticated_grant_storage_draft();
    deny.grant_id = None;
    deny.decision = ApprovalResponseDecisionStorage::Deny;
    deny.approved_step_ids.clear();
    deny.expires_at = None;
    assert_eq!(
        store
            .respond_to_approval(
                deny.clone(),
                approval_response_event(&deny, EventKind::ApprovalDenied, STEP_A),
            )
            .unwrap(),
        ApprovalResponseStorageOutcome::Denied
    );
    assert_eq!(
        store
            .respond_to_approval(
                deny.clone(),
                approval_response_event(&deny, EventKind::ApprovalDenied, STEP_A),
            )
            .unwrap(),
        ApprovalResponseStorageOutcome::AlreadyDenied
    );
    let conn = store.conn.lock().unwrap();
    assert_eq!(
        text(
            &conn,
            "SELECT status FROM approval_requests WHERE approval_id=?1",
            APPROVAL
        )
        .unwrap(),
        "DENIED"
    );
    assert_eq!(count_kind_store(&conn, "APPROVAL_DENIED"), 1);
}

#[test]
fn grant_consumption_is_member_bound_single_use_and_durable_after_reopen() {
    let path = TempDb::new();
    let store = approval_store(&path.0);
    let request = typed_request_draft();
    store
        .create_approval_request(request.clone(), approval_required_event(&request))
        .unwrap();
    let response = authenticated_grant_storage_draft();
    let grant_id = response.grant_id.clone().unwrap();
    store
        .respond_to_approval(
            response.clone(),
            approval_response_event(&response, EventKind::ApprovalGranted, STEP_A),
        )
        .unwrap();
    assert!(matches!(
        store.consume_grant_use(
            &grant_id,
            &TaskId::new(TASK).unwrap(),
            &StepId::new(STEP_B).unwrap(),
            EpochMillis::new(300).unwrap()
        ),
        Err(StoreError::InvalidApprovalResponse)
    ));
    assert_eq!(
        store
            .consume_grant_use(
                &grant_id,
                &TaskId::new(TASK).unwrap(),
                &StepId::new(STEP_A).unwrap(),
                EpochMillis::new(301).unwrap()
            )
            .unwrap(),
        GrantConsumptionOutcome::Consumed { uses_remaining: 0 }
    );
    assert_eq!(
        store
            .consume_grant_use(
                &grant_id,
                &TaskId::new(TASK).unwrap(),
                &StepId::new(STEP_A).unwrap(),
                EpochMillis::new(302).unwrap()
            )
            .unwrap(),
        GrantConsumptionOutcome::AlreadyConsumed
    );
    drop(store);
    let reopened = Store::open(&path.0, &FixedClock).unwrap();
    let conn = reopened.conn.lock().unwrap();
    assert_eq!(
        scalar(&conn, "SELECT count(*) FROM approval_grant_uses").unwrap(),
        1
    );
    assert_eq!(
        text(
            &conn,
            "SELECT status FROM approval_grants WHERE grant_id=?1",
            "grt_01JQ8ZA7B3KMW9Q4TVY7XN2RDP"
        )
        .unwrap(),
        "EXHAUSTED"
    );
    drop(conn);
    reopened.verify_integrity().unwrap();
}

#[test]
fn read_only_grant_fact_projection_observes_member_and_use_without_consuming() {
    let path = TempDb::new();
    let store = approval_store(&path.0);
    let request = typed_request_draft();
    store
        .create_approval_request(request.clone(), approval_required_event(&request))
        .unwrap();
    let response = authenticated_grant_storage_draft();
    let grant_id = response.grant_id.clone().unwrap();
    store
        .respond_to_approval(
            response.clone(),
            approval_response_event(&response, EventKind::ApprovalGranted, STEP_A),
        )
        .unwrap();

    let before = {
        let conn = store.conn.lock().unwrap();
        (
            scalar(&conn, "SELECT count(*) FROM approval_grant_uses").unwrap(),
            scalar(&conn, "SELECT count(*) FROM event_content").unwrap(),
        )
    };
    let observed = store
        .transact(|tx| {
            let member = tx.observe_grant_facts(
                &grant_id,
                &TaskId::new(TASK).unwrap(),
                &StepId::new(STEP_A).unwrap(),
            )?;
            let nonmember = tx.observe_grant_facts(
                &grant_id,
                &TaskId::new(TASK).unwrap(),
                &StepId::new(STEP_B).unwrap(),
            )?;
            Ok((member, nonmember))
        })
        .unwrap();
    let member = observed.0.unwrap();
    assert_eq!(member.grant_id(), &grant_id);
    assert_eq!(member.task_id(), &TaskId::new(TASK).unwrap());
    assert_eq!(member.status(), crate::GrantFactStatus::Active);
    assert_eq!(member.uses_remaining(), 1);
    assert_eq!(member.expires_at(), EpochMillis::new(1_800_000).unwrap());
    assert_eq!(
        member.member_arguments_digest(),
        Some(&Digest::new(ARGS_A).unwrap())
    );
    assert!(!member.already_used());
    let nonmember = observed.1.unwrap();
    assert_eq!(nonmember.member_arguments_digest(), None);
    assert_eq!(nonmember.member_scope_digest(), None);
    assert!(!nonmember.already_used());
    {
        let conn = store.conn.lock().unwrap();
        assert_eq!(
            scalar(&conn, "SELECT count(*) FROM approval_grant_uses").unwrap(),
            before.0
        );
        assert_eq!(
            scalar(&conn, "SELECT count(*) FROM event_content").unwrap(),
            before.1
        );
    }

    store
        .consume_grant_use(
            &grant_id,
            &TaskId::new(TASK).unwrap(),
            &StepId::new(STEP_A).unwrap(),
            EpochMillis::new(301).unwrap(),
        )
        .unwrap();
    let used = store
        .transact(|tx| {
            tx.observe_grant_facts(
                &grant_id,
                &TaskId::new(TASK).unwrap(),
                &StepId::new(STEP_A).unwrap(),
            )
        })
        .unwrap()
        .unwrap();
    assert_eq!(used.status(), crate::GrantFactStatus::Exhausted);
    assert_eq!(used.uses_remaining(), 0);
    assert!(used.already_used());
    drop(store);
    let reopened = Store::open(&path.0, &FixedClock).unwrap();
    let reopened_facts = reopened
        .transact(|tx| {
            tx.observe_grant_facts(
                &grant_id,
                &TaskId::new(TASK).unwrap(),
                &StepId::new(STEP_A).unwrap(),
            )
        })
        .unwrap()
        .unwrap();
    assert!(reopened_facts.already_used());
    reopened.verify_integrity().unwrap();
}

#[test]
fn grant_fact_projection_does_not_reject_a_corrupted_sealed_member_set() {
    // Adversarial corruption fixture only: remove the SQLite write fences to
    // model a local file/schema attacker. The normal Grant writer is used to
    // create the real sealed row before corruption.
    let path = TempDb::new();
    let store = approval_store(&path.0);
    let request = typed_request_draft();
    store
        .create_approval_request(request.clone(), approval_required_event(&request))
        .unwrap();
    let response = authenticated_grant_storage_draft();
    let grant_id = response.grant_id.clone().unwrap();
    store
        .respond_to_approval(
            response.clone(),
            approval_response_event(&response, EventKind::ApprovalGranted, STEP_A),
        )
        .unwrap();

    {
        let conn = store.conn.lock().unwrap();
        conn.execute_batch(
            "DROP TRIGGER approval_grant_member_no_delete;
             DROP TRIGGER approval_grant_seal_no_delete;",
        )
        .unwrap();
        conn.execute(
            "DELETE FROM approval_grant_members WHERE grant_id=?1 AND step_id=?2",
            params![grant_id.as_str(), STEP_A],
        )
        .unwrap();
    }

    let facts = store
        .transact(|tx| {
            tx.observe_grant_facts(
                &grant_id,
                &TaskId::new(TASK).unwrap(),
                &StepId::new(STEP_A).unwrap(),
            )
        })
        .unwrap()
        .unwrap();
    // The existing projection reports inconsistent facts instead of failing
    // closed. No use, event, or intent is created by this read-only probe.
    assert!(facts.sealed());
    assert_eq!(facts.member_count(), 0);
    assert_eq!(facts.action_count(), 1);
    assert_eq!(facts.member_arguments_digest(), None);
    assert_eq!(
        scalar(
            &store.conn.lock().unwrap(),
            "SELECT count(*) FROM approval_grant_uses"
        )
        .unwrap(),
        0
    );
    drop(store);

    // SQLite's structural check does not attest the application-level Grant
    // complete-set invariant after trigger removal and row corruption.
    let reopened = Store::open(&path.0, &FixedClock).unwrap();
    reopened.verify_integrity().unwrap();
}

/// Experimental, test-only full-set validator. It runs on the same real
/// `BEGIN IMMEDIATE` transaction as Storage operations and deliberately exposes
/// no runtime authorization result or public constructor.
fn probe_grant_full_set(
    tx: &Tx<'_>,
    grant_id: &GrantId,
    approval_id: &ApprovalId,
    task_id: &TaskId,
    selected_step: &StepId,
) -> Result<Vec<(i64, StepId, Digest, Digest)>, StoreError> {
    crate::candidate_c_test_only::probe_grant_full_set(
        tx,
        grant_id,
        approval_id,
        task_id,
        selected_step,
    )
}

fn full_grant_probe_effect_counts(store: &Store) -> (i64, i64, i64) {
    let conn = store.conn.lock().unwrap();
    (
        scalar(&conn, "SELECT count(*) FROM approval_grant_uses").unwrap(),
        scalar(&conn, "SELECT count(*) FROM event_content").unwrap(),
        scalar(&conn, "SELECT count(*) FROM approval_requests").unwrap(),
    )
}

fn full_sqlite_table_rows(
    store: &Store,
) -> std::collections::BTreeMap<String, Vec<Vec<rusqlite::types::Value>>> {
    let conn = store.conn.lock().unwrap();
    let tables: Vec<String> = {
        let mut statement = conn
            .prepare("SELECT name FROM sqlite_schema WHERE type='table' ORDER BY name")
            .unwrap();
        statement
            .query_map([], |row| row.get(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap()
    };
    tables
        .into_iter()
        .filter(|table| !table.starts_with("sqlite_"))
        .map(|table| {
            let escaped = table.replace('"', "\"\"");
            let columns: Vec<String> = {
                let mut statement = conn
                    .prepare(&format!("PRAGMA table_info(\"{escaped}\")"))
                    .unwrap();
                statement
                    .query_map([], |row| row.get(1))
                    .unwrap()
                    .collect::<Result<_, _>>()
                    .unwrap()
            };
            let order = columns
                .iter()
                .map(|column| format!("\"{}\"", column.replace('"', "\"\"")))
                .collect::<Vec<_>>()
                .join(", ");
            let sql = if order.is_empty() {
                format!("SELECT * FROM \"{escaped}\"")
            } else {
                format!("SELECT * FROM \"{escaped}\" ORDER BY {order}")
            };
            let rows = {
                let mut statement = conn.prepare(&sql).unwrap();
                let column_count = statement.column_count();
                statement
                    .query_map([], |row| {
                        (0..column_count)
                            .map(|column| row.get(column))
                            .collect::<rusqlite::Result<Vec<rusqlite::types::Value>>>()
                    })
                    .unwrap()
                    .collect::<Result<Vec<_>, _>>()
                    .unwrap()
            };
            (table, rows)
        })
        .collect()
}

#[test]
fn test_only_full_grant_set_probe_checks_subset_and_rejects_corruption() {
    fn setup(two: bool) -> (TempDb, Store, GrantId) {
        let path = TempDb::new();
        let store = approval_store(&path.0);
        let request = if two {
            two_action_request_draft()
        } else {
            typed_request_draft()
        };
        store
            .create_approval_request(request.clone(), approval_required_event(&request))
            .unwrap();
        let response = if two {
            two_action_grant_storage_draft()
        } else {
            authenticated_grant_storage_draft()
        };
        let grant = response.grant_id.clone().unwrap();
        store
            .respond_to_approval(
                response.clone(),
                approval_response_event(&response, EventKind::ApprovalGranted, STEP_A),
            )
            .unwrap();
        (path, store, grant)
    }
    let (_single_path, single, single_grant) = setup(false);
    let one = single
        .transact(|tx| {
            probe_grant_full_set(
                tx,
                &single_grant,
                &ApprovalId::new(APPROVAL).unwrap(),
                &TaskId::new(TASK).unwrap(),
                &StepId::new(STEP_A).unwrap(),
            )
        })
        .unwrap();
    assert_eq!(one.len(), 1);
    let (_subset_path, subset_store, subset_grant) = {
        let path = TempDb::new();
        let store = approval_store(&path.0);
        let request = two_action_request_draft();
        store
            .create_approval_request(request.clone(), approval_required_event(&request))
            .unwrap();
        let mut response = authenticated_grant_storage_draft();
        response.request_action_set_digest = request.action_set_digest.clone();
        let grant = response.grant_id.clone().unwrap();
        store
            .respond_to_approval(
                response.clone(),
                approval_response_event(&response, EventKind::ApprovalGranted, STEP_A),
            )
            .unwrap();
        (path, store, grant)
    };
    assert_eq!(
        subset_store
            .transact(|tx| probe_grant_full_set(
                tx,
                &subset_grant,
                &ApprovalId::new(APPROVAL).unwrap(),
                &TaskId::new(TASK).unwrap(),
                &StepId::new(STEP_A).unwrap(),
            ))
            .unwrap()
            .len(),
        1,
        "Grant subset is one member from a two-action Request"
    );
    let (_multi_path, multi, multi_grant) = setup(true);
    let both = multi
        .transact(|tx| {
            probe_grant_full_set(
                tx,
                &multi_grant,
                &ApprovalId::new(APPROVAL).unwrap(),
                &TaskId::new(TASK).unwrap(),
                &StepId::new(STEP_B).unwrap(),
            )
        })
        .unwrap();
    assert_eq!(both.len(), 2);
    assert_eq!(
        both.iter().map(|m| m.1.as_str()).collect::<Vec<_>>(),
        vec![STEP_A, STEP_B]
    );
    let before = full_grant_probe_effect_counts(&multi);
    let before_rows = full_sqlite_table_rows(&multi);
    multi
        .transact(|tx| {
            probe_grant_full_set(
                tx,
                &multi_grant,
                &ApprovalId::new(APPROVAL).unwrap(),
                &TaskId::new(TASK).unwrap(),
                &StepId::new(STEP_A).unwrap(),
            )
        })
        .unwrap();
    let after = full_grant_probe_effect_counts(&multi);
    assert_eq!(
        before, after,
        "probe is read-only: no Grant use, Event or Request rows"
    );
    assert_eq!(
        before_rows,
        full_sqlite_table_rows(&multi),
        "probe is read-only across every ordinary SQLite table row"
    );
    assert!(
        multi
            .transact(|tx| probe_grant_full_set(
                tx,
                &multi_grant,
                &ApprovalId::new(APPROVAL).unwrap(),
                &TaskId::new(TASK).unwrap(),
                &StepId::new(STEP_C).unwrap()
            ))
            .is_err()
    );
    // A surrounding transaction error rolls back normally; the probe itself
    // performs only SELECTs and changes no Grant/event facts.
    assert!(
        multi
            .transact(|tx| {
                probe_grant_full_set(
                    tx,
                    &multi_grant,
                    &ApprovalId::new(APPROVAL).unwrap(),
                    &TaskId::new(TASK).unwrap(),
                    &StepId::new(STEP_A).unwrap(),
                )?;
                tx.inner.execute(
                    "UPDATE approval_grants SET status='REVOKED' WHERE grant_id=?1",
                    [multi_grant.as_str()],
                )?;
                Err::<(), _>(StoreError::ConstraintViolation)
            })
            .is_err()
    );
    assert_eq!(
        text(
            &multi.conn.lock().unwrap(),
            "SELECT status FROM approval_grants WHERE grant_id=?1",
            multi_grant.as_str()
        )
        .unwrap(),
        "ACTIVE"
    );
    let (before_uses, before_events, _) = full_grant_probe_effect_counts(&multi);
    assert_eq!((before_uses, before_events), (0, 2));
    drop(multi);
    let reopened = Store::open(&_multi_path.0, &FixedClock).unwrap();
    assert_eq!(
        reopened
            .transact(|tx| probe_grant_full_set(
                tx,
                &multi_grant,
                &ApprovalId::new(APPROVAL).unwrap(),
                &TaskId::new(TASK).unwrap(),
                &StepId::new(STEP_A).unwrap()
            ))
            .unwrap()
            .len(),
        2
    );
    reopened.verify_integrity().unwrap();

    // Direct SQL is confined to adversarial fixtures: drop immutable-row
    // triggers to represent a local database/schema attacker.
    for (damage, sql) in [
        (
            "missing",
            "DROP TRIGGER approval_grant_member_no_delete; DELETE FROM approval_grant_members WHERE position=0",
        ),
        (
            "position",
            "DROP TRIGGER approval_grant_member_no_update; UPDATE approval_grant_members SET position=7 WHERE position=0",
        ),
        (
            "reordered",
            "DROP TRIGGER approval_grant_member_no_update; UPDATE approval_grant_members SET position=7 WHERE position=0; UPDATE approval_grant_members SET position=0 WHERE position=1; UPDATE approval_grant_members SET position=1 WHERE position=7",
        ),
        (
            "digest",
            "DROP TRIGGER approval_grant_member_no_update; UPDATE approval_grant_members SET arguments_digest='sha256:0000000000000000000000000000000000000000000000000000000000000000' WHERE position=0",
        ),
        (
            "scope-digest",
            "DROP TRIGGER approval_grant_member_no_update; UPDATE approval_grant_members SET scope_digest='sha256:0000000000000000000000000000000000000000000000000000000000000000' WHERE position=0",
        ),
        (
            "request-action",
            "DROP TRIGGER approval_request_action_no_update; UPDATE approval_request_actions SET arguments_digest='sha256:0000000000000000000000000000000000000000000000000000000000000000' WHERE position=0",
        ),
        (
            "seal",
            "DROP TRIGGER approval_grant_seal_no_delete; DELETE FROM approval_grant_seals",
        ),
    ] {
        let (path, store, grant) = setup(true);
        {
            let conn = store.conn.lock().unwrap();
            conn.execute_batch(sql).unwrap();
        }
        assert!(
            store
                .transact(|tx| probe_grant_full_set(
                    tx,
                    &grant,
                    &ApprovalId::new(APPROVAL).unwrap(),
                    &TaskId::new(TASK).unwrap(),
                    &StepId::new(STEP_A).unwrap()
                ))
                .is_err(),
            "{damage}"
        );
        assert_eq!(
            scalar(
                &store.conn.lock().unwrap(),
                "SELECT count(*) FROM approval_grant_uses"
            )
            .unwrap(),
            0
        );
        drop(store);
        let reopened = Store::open(&path.0, &FixedClock).unwrap();
        assert!(
            reopened
                .transact(|tx| probe_grant_full_set(
                    tx,
                    &grant,
                    &ApprovalId::new(APPROVAL).unwrap(),
                    &TaskId::new(TASK).unwrap(),
                    &StepId::new(STEP_A).unwrap()
                ))
                .is_err(),
            "{damage} after reopen"
        );
        reopened.verify_integrity().unwrap();
    }
    let (_path, constrained, grant) = setup(true);
    let conn = constrained.conn.lock().unwrap();
    assert!(
        conn.execute(
            "UPDATE approval_grants SET action_count=1 WHERE grant_id=?1",
            [grant.as_str()]
        )
        .is_err(),
        "SQLite CHECK max_uses=action_count rejects count-only corruption"
    );
    drop(conn);
    assert_eq!(
        constrained
            .transact(|tx| probe_grant_full_set(
                tx,
                &grant,
                &ApprovalId::new(APPROVAL).unwrap(),
                &TaskId::new(TASK).unwrap(),
                &StepId::new(STEP_A).unwrap()
            ))
            .unwrap()
            .len(),
        2
    );
    for sql in [
        "DROP TRIGGER approval_grant_member_no_delete; DROP TRIGGER approval_grant_member_no_insert_after_seal; DELETE FROM approval_grant_members WHERE position=1; INSERT INTO approval_grant_members(grant_id,position,approval_id,step_id,arguments_digest,scope_digest) SELECT grant_id,1,approval_id,step_id,arguments_digest,scope_digest FROM approval_grant_members WHERE position=0",
        "DROP TRIGGER approval_grant_member_no_delete; DROP TRIGGER approval_grant_member_no_insert_after_seal; DELETE FROM approval_grant_members WHERE position=1; INSERT INTO approval_grant_members(grant_id,position,approval_id,step_id,arguments_digest,scope_digest) SELECT grant_id,0,approval_id,'stp_01JQ8Z9M5T9WXK2H4BNPQ7RDST','sha256:eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee2',scope_digest FROM approval_grant_members WHERE position=0",
    ] {
        let (_path, store, _grant) = setup(true);
        let conn = store.conn.lock().unwrap();
        assert!(
            conn.execute_batch(sql).is_err(),
            "duplicate Step or position remains SQLite-constrained"
        );
    }
}

#[test]
fn full_set_probe_is_serialized_with_an_independent_sqlite_writer() {
    let (path, first, grant) = {
        let path = TempDb::new();
        let store = approval_store(&path.0);
        let request = two_action_request_draft();
        store
            .create_approval_request(request.clone(), approval_required_event(&request))
            .unwrap();
        let response = two_action_grant_storage_draft();
        let grant = response.grant_id.clone().unwrap();
        store
            .respond_to_approval(
                response.clone(),
                approval_response_event(&response, EventKind::ApprovalGranted, STEP_A),
            )
            .unwrap();
        (path, store, grant)
    };
    let second = Store::open(&path.0, &FixedClock).unwrap();
    // Prove lock ownership directly before relying on the competing typed
    // writer's behavior. This connection cannot wait for the Store default
    // busy timeout, so success here means SQLite itself reports the held lock.
    let mut lock_witness = Connection::open(&path.0).unwrap();
    lock_witness
        .busy_timeout(std::time::Duration::ZERO)
        .unwrap();
    let (start_tx, start_rx) = std::sync::mpsc::channel();
    let (attempting_tx, attempting_rx) = std::sync::mpsc::channel();
    let (done_tx, done_rx) = std::sync::mpsc::channel();
    let worker_grant = grant.clone();
    let worker = std::thread::spawn(move || {
        start_rx.recv().unwrap();
        attempting_tx.send(()).unwrap();
        let result = second.transact(|tx| {
            probe_grant_full_set(
                tx,
                &worker_grant,
                &ApprovalId::new(APPROVAL).unwrap(),
                &TaskId::new(TASK).unwrap(),
                &StepId::new(STEP_A).unwrap(),
            )
            .map(|rows| rows.len())
        });
        done_tx.send(result).unwrap();
    });
    first
        .transact(|tx| {
            assert_eq!(
                probe_grant_full_set(
                    tx,
                    &grant,
                    &ApprovalId::new(APPROVAL).unwrap(),
                    &TaskId::new(TASK).unwrap(),
                    &StepId::new(STEP_A).unwrap()
                )?
                .len(),
                2
            );
            let lock_attempt = lock_witness
                .transaction_with_behavior(TransactionBehavior::Immediate);
            assert!(
                matches!(
                    lock_attempt,
                    Err(rusqlite::Error::SqliteFailure(ref error, _))
                        if matches!(error.code, rusqlite::ErrorCode::DatabaseBusy | rusqlite::ErrorCode::DatabaseLocked)
                ),
                "independent zero-timeout connection directly witnesses the held writer lock"
            );
            start_tx.send(()).unwrap();
            attempting_rx.recv().unwrap();
            assert!(
                done_rx
                    .recv_timeout(std::time::Duration::from_millis(100))
                    .is_err(),
                "second connection must wait behind BEGIN IMMEDIATE"
            );
            Ok(())
        })
        .unwrap();
    assert_eq!(done_rx.recv().unwrap().unwrap(), 2);
    worker.join().unwrap();
    first.verify_integrity().unwrap();
}

#[test]
fn transaction_closure_clock_sample_occurs_only_after_writer_reservation() {
    use std::sync::atomic::AtomicUsize;
    let path = TempDb::new();
    let first = approval_store(&path.0);
    let second = Store::open(&path.0, &FixedClock).unwrap();
    let samples = Arc::new(AtomicUsize::new(0));
    let worker_samples = Arc::clone(&samples);
    let (start_tx, start_rx) = std::sync::mpsc::channel();
    let (attempting_tx, attempting_rx) = std::sync::mpsc::channel();
    let (done_tx, done_rx) = std::sync::mpsc::channel();
    let worker = std::thread::spawn(move || {
        start_rx.recv().unwrap();
        attempting_tx.send(()).unwrap();
        let sampled = second.transact(|_tx| {
            worker_samples.fetch_add(1, Ordering::SeqCst);
            Ok(())
        });
        done_tx.send(sampled).unwrap();
    });
    first
        .transact(|_tx| {
            start_tx.send(()).unwrap();
            attempting_rx.recv().unwrap();
            assert_eq!(
                samples.load(Ordering::SeqCst),
                0,
                "closure must not run while another connection owns the writer reservation"
            );
            assert!(
                done_rx
                    .recv_timeout(std::time::Duration::from_millis(100))
                    .is_err()
            );
            Ok(())
        })
        .unwrap();
    done_rx.recv().unwrap().unwrap();
    worker.join().unwrap();
    assert_eq!(
        samples.load(Ordering::SeqCst),
        1,
        "one caller-owned sample occurred after BEGIN IMMEDIATE"
    );
}

#[test]
fn request_expiration_uses_inclusive_boundary_and_is_event_atomic() {
    let path = TempDb::new();
    let store = approval_store(&path.0);
    let request = typed_request_draft();
    store
        .create_approval_request(request.clone(), approval_required_event(&request))
        .unwrap();
    assert!(
        !store
            .expire_approval_request(
                &request.approval_id,
                EpochMillis::new(1_800_099).unwrap(),
                approval_expired_event(&request, EpochMillis::new(1_800_099).unwrap())
            )
            .unwrap()
    );
    assert!(
        store
            .expire_approval_request(
                &request.approval_id,
                request.expires_at,
                approval_expired_event(&request, request.expires_at)
            )
            .unwrap()
    );
    assert!(
        !store
            .expire_approval_request(
                &request.approval_id,
                request.expires_at,
                approval_expired_event(&request, request.expires_at)
            )
            .unwrap()
    );
    let conn = store.conn.lock().unwrap();
    assert_eq!(
        text(
            &conn,
            "SELECT status FROM approval_requests WHERE approval_id=?1",
            APPROVAL
        )
        .unwrap(),
        "EXPIRED"
    );
    assert_eq!(count_kind_store(&conn, "APPROVAL_EXPIRED"), 1);
}

#[test]
fn grant_response_at_the_request_expiry_boundary_commits_only_expiration() {
    let path = TempDb::new();
    let store = approval_store(&path.0);
    let request = typed_request_draft();
    store
        .create_approval_request(request.clone(), approval_required_event(&request))
        .unwrap();

    let mut response = authenticated_grant_storage_draft();
    response.checked_at = request.expires_at;
    response.decided_at = request.expires_at;
    response.expires_at = Some(request.expires_at);
    assert_eq!(
        store
            .respond_to_approval(
                response.clone(),
                approval_response_event(&response, EventKind::ApprovalExpired, STEP_A),
            )
            .unwrap(),
        ApprovalResponseStorageOutcome::Expired
    );

    let conn = store.conn.lock().unwrap();
    assert_eq!(
        text(
            &conn,
            "SELECT status FROM approval_requests WHERE approval_id=?1",
            APPROVAL
        )
        .unwrap(),
        "EXPIRED"
    );
    assert_eq!(
        scalar(&conn, "SELECT count(*) FROM approval_grants").unwrap(),
        0
    );
    assert_eq!(count_kind_store(&conn, "APPROVAL_GRANTED"), 0);
    assert_eq!(count_kind_store(&conn, "APPROVAL_EXPIRED"), 1);
}

#[test]
fn grant_response_after_task_cancellation_creates_no_grant() {
    let path = TempDb::new();
    let store = approval_store(&path.0);
    let request = typed_request_draft();
    store
        .create_approval_request(request.clone(), approval_required_event(&request))
        .unwrap();

    // The cancellation fixture satisfies the frozen task-row constraints and
    // represents an already committed host cancellation before the response.
    store
        .conn
        .lock()
        .unwrap()
        .execute(
            "UPDATE tasks SET state='CANCELLED',cancelled_at_ms=1800100,
                cancelled_by='HOST',updated_at_ms=1800100 WHERE task_id=?1",
            [TASK],
        )
        .unwrap();

    let response = authenticated_grant_storage_draft();
    assert!(matches!(
        store.respond_to_approval(
            response.clone(),
            approval_response_event(&response, EventKind::ApprovalGranted, STEP_A),
        ),
        Err(StoreError::TaskNotApprovable)
    ));

    let conn = store.conn.lock().unwrap();
    assert_eq!(
        text(
            &conn,
            "SELECT status FROM approval_requests WHERE approval_id=?1",
            APPROVAL
        )
        .unwrap(),
        "PENDING"
    );
    assert_eq!(
        scalar(&conn, "SELECT count(*) FROM approval_grants").unwrap(),
        0
    );
    assert_eq!(count_kind_store(&conn, "APPROVAL_GRANTED"), 0);
}

fn run_approval_commit_crash_child(path: &Path, stage: &str) -> ExitStatus {
    Command::new(std::env::current_exe().unwrap())
        .arg("--exact")
        .arg("migration_0005_tests::approval_commit_crash_child_entry")
        .arg("--nocapture")
        .env("SEREA_P6D_CRASH_DB", path)
        .env("SEREA_P6D_CRASH_STAGE", stage)
        .status()
        .unwrap()
}

#[test]
fn approval_commit_crash_child_entry() {
    let Ok(path) = std::env::var("SEREA_P6D_CRASH_DB") else {
        return;
    };
    let stage = std::env::var("SEREA_P6D_CRASH_STAGE").unwrap();
    let store = Store::open(Path::new(&path), &FixedClock).unwrap();
    let response = authenticated_grant_storage_draft();
    store
        .transact(|tx| {
            tx.respond_to_approval(
                response.clone(),
                approval_response_event(&response, EventKind::ApprovalGranted, STEP_A),
            )?;
            if stage == "before_commit" {
                std::process::exit(41);
            }
            Ok(())
        })
        .unwrap();
    assert_eq!(stage, "after_commit");
    std::process::exit(42);
}

#[test]
fn approval_process_death_before_and_after_commit_recovers_one_atomic_outcome() {
    let path = TempDb::new();
    let setup = approval_store(&path.0);
    let request = typed_request_draft();
    setup
        .create_approval_request(request.clone(), approval_required_event(&request))
        .unwrap();
    drop(setup);

    let before = run_approval_commit_crash_child(&path.0, "before_commit");
    assert_eq!(before.code(), Some(41));
    let reopened = Store::open(&path.0, &FixedClock).unwrap();
    {
        let conn = reopened.conn.lock().unwrap();
        assert_eq!(
            text(
                &conn,
                "SELECT status FROM approval_requests WHERE approval_id=?1",
                APPROVAL
            )
            .unwrap(),
            "PENDING"
        );
        assert_eq!(
            scalar(&conn, "SELECT count(*) FROM approval_grants").unwrap(),
            0
        );
        assert_eq!(count_kind_store(&conn, "APPROVAL_GRANTED"), 0);
    }
    drop(reopened);

    let after = run_approval_commit_crash_child(&path.0, "after_commit");
    assert_eq!(after.code(), Some(42));
    let reopened = Store::open(&path.0, &FixedClock).unwrap();
    {
        let conn = reopened.conn.lock().unwrap();
        assert_eq!(
            text(
                &conn,
                "SELECT status FROM approval_requests WHERE approval_id=?1",
                APPROVAL
            )
            .unwrap(),
            "APPROVED"
        );
        assert_eq!(
            scalar(&conn, "SELECT count(*) FROM approval_grants").unwrap(),
            1
        );
        assert_eq!(
            scalar(&conn, "SELECT count(*) FROM approval_grant_members").unwrap(),
            1
        );
        assert_eq!(
            scalar(&conn, "SELECT count(*) FROM approval_grant_seals").unwrap(),
            1
        );
        assert_eq!(count_kind_store(&conn, "APPROVAL_GRANTED"), 1);
    }
    reopened.verify_integrity().unwrap();
}

#[test]
fn failed_grant_event_rolls_back_request_grant_members_and_seal() {
    let path = TempDb::new();
    let store = approval_store(&path.0);
    let request = typed_request_draft();
    store
        .create_approval_request(request.clone(), approval_required_event(&request))
        .unwrap();
    {
        let conn = store.conn.lock().unwrap();
        conn.execute(
            "UPDATE event_store_state SET retained_count=1000000 WHERE singleton=1",
            [],
        )
        .unwrap();
    }
    let response = authenticated_grant_storage_draft();
    assert!(matches!(
        store.respond_to_approval(
            response.clone(),
            approval_response_event(&response, EventKind::ApprovalGranted, STEP_A)
        ),
        Err(StoreError::EventStoreCapacity)
    ));
    let conn = store.conn.lock().unwrap();
    assert_eq!(
        text(
            &conn,
            "SELECT status FROM approval_requests WHERE approval_id=?1",
            APPROVAL
        )
        .unwrap(),
        "PENDING"
    );
    assert_eq!(
        scalar(&conn, "SELECT count(*) FROM approval_grants").unwrap(),
        0
    );
    assert_eq!(
        scalar(&conn, "SELECT count(*) FROM approval_grant_members").unwrap(),
        0
    );
    assert_eq!(
        scalar(&conn, "SELECT count(*) FROM approval_grant_seals").unwrap(),
        0
    );
}

#[test]
fn authenticated_revocation_is_grant_only_atomic_and_preserves_prior_consumption() {
    let path = TempDb::new();
    let store = approval_store(&path.0);
    let request = two_action_request_draft();
    store
        .create_approval_request(request.clone(), approval_required_event(&request))
        .unwrap();
    let response = two_action_grant_storage_draft();
    store
        .respond_to_approval(
            response.clone(),
            approval_response_event(&response, EventKind::ApprovalGranted, STEP_A),
        )
        .unwrap();
    let task = TaskId::new(TASK).unwrap();
    let grant = GrantId::new(GRANT).unwrap();
    assert_eq!(
        store
            .consume_grant_use(
                &grant,
                &task,
                &StepId::new(STEP_A).unwrap(),
                EpochMillis::new(300).unwrap()
            )
            .unwrap(),
        GrantConsumptionOutcome::Consumed { uses_remaining: 1 }
    );
    let revoke = revocation_storage_draft();
    assert_eq!(
        store
            .revoke_approval_grant(revoke.clone(), approval_revoked_event(&revoke))
            .unwrap(),
        ApprovalRevocationOutcome::Revoked
    );
    assert_eq!(
        store
            .revoke_approval_grant(revoke, approval_revoked_event(&revocation_storage_draft()))
            .unwrap(),
        ApprovalRevocationOutcome::AlreadyRevoked
    );
    let revoked = store
        .transact(|tx| tx.observe_grant_facts(&grant, &task, &StepId::new(STEP_B).unwrap()))
        .unwrap()
        .unwrap();
    assert_eq!(revoked.status(), crate::GrantFactStatus::Revoked);
    assert_eq!(revoked.uses_remaining(), 1);
    assert!(revoked.sealed());
    assert_eq!(revoked.member_count(), 2);
    assert_eq!(
        revoked.member_arguments_digest(),
        Some(&Digest::new(ARGS_B).unwrap())
    );
    assert!(!revoked.already_used());
    assert_eq!(
        store
            .consume_grant_use(
                &grant,
                &task,
                &StepId::new(STEP_B).unwrap(),
                EpochMillis::new(500).unwrap()
            )
            .unwrap(),
        GrantConsumptionOutcome::Revoked
    );
    let conn = store.conn.lock().unwrap();
    assert_eq!(
        text(
            &conn,
            "SELECT status FROM approval_requests WHERE approval_id=?1",
            APPROVAL
        )
        .unwrap(),
        "APPROVED"
    );
    assert_eq!(
        text(
            &conn,
            "SELECT status FROM approval_grants WHERE grant_id=?1",
            GRANT
        )
        .unwrap(),
        "REVOKED"
    );
    assert_eq!(
        scalar(
            &conn,
            "SELECT count(*) FROM approval_grant_uses WHERE grant_id='grt_01JQ8ZA7B3KMW9Q4TVY7XN2RDP'",
        )
        .unwrap(),
        1
    );
    assert_eq!(count_kind_store(&conn, "APPROVAL_REVOKED"), 1);
    drop(conn);
    store.verify_integrity().unwrap();
}

#[test]
fn unused_grant_expiration_is_inclusive_audited_atomic_and_durable() {
    let path = TempDb::new();
    let store = approval_store(&path.0);
    let request = two_action_request_draft();
    store
        .create_approval_request(request.clone(), approval_required_event(&request))
        .unwrap();
    let response = two_action_grant_storage_draft();
    store
        .respond_to_approval(
            response.clone(),
            approval_response_event(&response, EventKind::ApprovalGranted, STEP_A),
        )
        .unwrap();
    let grant = GrantId::new(GRANT).unwrap();
    let task = TaskId::new(TASK).unwrap();
    assert_eq!(
        store
            .consume_grant_use(
                &grant,
                &task,
                &StepId::new(STEP_A).unwrap(),
                EpochMillis::new(300).unwrap(),
            )
            .unwrap(),
        GrantConsumptionOutcome::Consumed { uses_remaining: 1 }
    );

    assert!(
        !store
            .expire_approval_grant(
                &grant,
                EpochMillis::new(1_799_999).unwrap(),
                approval_expired_unused_event(EpochMillis::new(1_799_999).unwrap()),
            )
            .unwrap()
    );
    assert!(
        store
            .expire_approval_grant(
                &grant,
                EpochMillis::new(1_800_000).unwrap(),
                approval_expired_unused_event(EpochMillis::new(1_800_000).unwrap()),
            )
            .unwrap()
    );
    assert!(
        !store
            .expire_approval_grant(
                &grant,
                EpochMillis::new(1_800_001).unwrap(),
                approval_expired_unused_event(EpochMillis::new(1_800_001).unwrap()),
            )
            .unwrap()
    );
    let conn = store.conn.lock().unwrap();
    assert_eq!(
        text(
            &conn,
            "SELECT status FROM approval_grants WHERE grant_id=?1",
            GRANT
        )
        .unwrap(),
        "EXPIRED"
    );
    assert_eq!(
        scalar(&conn, "SELECT uses_remaining FROM approval_grants").unwrap(),
        1
    );
    assert_eq!(count_kind_store(&conn, "APPROVAL_EXPIRED_UNUSED"), 1);
    drop(conn);
    drop(store);
    let reopened = Store::open(&path.0, &FixedClock).unwrap();
    reopened.verify_integrity().unwrap();
    let conn = reopened.conn.lock().unwrap();
    assert_eq!(
        text(
            &conn,
            "SELECT status FROM approval_grants WHERE grant_id=?1",
            GRANT
        )
        .unwrap(),
        "EXPIRED"
    );
    assert_eq!(count_kind_store(&conn, "APPROVAL_EXPIRED_UNUSED"), 1);
}

#[test]
fn failed_unused_grant_expiration_event_rolls_back_state() {
    let path = TempDb::new();
    let store = approval_store(&path.0);
    let request = two_action_request_draft();
    store
        .create_approval_request(request.clone(), approval_required_event(&request))
        .unwrap();
    let response = two_action_grant_storage_draft();
    store
        .respond_to_approval(
            response.clone(),
            approval_response_event(&response, EventKind::ApprovalGranted, STEP_A),
        )
        .unwrap();
    {
        let conn = store.conn.lock().unwrap();
        conn.execute(
            "UPDATE event_store_state SET retained_count=1000000 WHERE singleton=1",
            [],
        )
        .unwrap();
    }
    let now = EpochMillis::new(1_800_000).unwrap();
    assert!(matches!(
        store.expire_approval_grant(
            &GrantId::new(GRANT).unwrap(),
            now,
            approval_expired_unused_event(now)
        ),
        Err(StoreError::EventStoreCapacity)
    ));
    let conn = store.conn.lock().unwrap();
    assert_eq!(
        text(
            &conn,
            "SELECT status FROM approval_grants WHERE grant_id=?1",
            GRANT
        )
        .unwrap(),
        "ACTIVE"
    );
    assert_eq!(count_kind_store(&conn, "APPROVAL_EXPIRED_UNUSED"), 0);
}

#[test]
fn revocation_event_failure_rolls_back_grant_state_and_reopens_active() {
    let path = TempDb::new();
    let store = approval_store(&path.0);
    let request = typed_request_draft();
    store
        .create_approval_request(request.clone(), approval_required_event(&request))
        .unwrap();
    let response = authenticated_grant_storage_draft();
    store
        .respond_to_approval(
            response.clone(),
            approval_response_event(&response, EventKind::ApprovalGranted, STEP_A),
        )
        .unwrap();
    {
        let conn = store.conn.lock().unwrap();
        conn.execute(
            "UPDATE event_store_state SET retained_count=1000000 WHERE singleton=1",
            [],
        )
        .unwrap();
    }
    let revoke = revocation_storage_draft();
    assert!(matches!(
        store.revoke_approval_grant(revoke.clone(), approval_revoked_event(&revoke)),
        Err(StoreError::EventStoreCapacity)
    ));
    {
        let conn = store.conn.lock().unwrap();
        assert_eq!(
            text(
                &conn,
                "SELECT status FROM approval_grants WHERE grant_id=?1",
                GRANT
            )
            .unwrap(),
            "ACTIVE"
        );
        assert_eq!(count_kind_store(&conn, "APPROVAL_REVOKED"), 0);
    }
    drop(store);
    let reopened = Store::open(&path.0, &FixedClock).unwrap();
    let conn = reopened.conn.lock().unwrap();
    assert_eq!(
        text(
            &conn,
            "SELECT status FROM approval_grants WHERE grant_id=?1",
            GRANT
        )
        .unwrap(),
        "ACTIVE"
    );
    assert_eq!(count_kind_store(&conn, "APPROVAL_REVOKED"), 0);
}

#[test]
fn approved_response_replay_after_revocation_is_never_idempotent_success() {
    let path = TempDb::new();
    let store = approval_store(&path.0);
    let request = typed_request_draft();
    store
        .create_approval_request(request.clone(), approval_required_event(&request))
        .unwrap();
    let grant_response = authenticated_grant_storage_draft();
    store
        .respond_to_approval(
            grant_response.clone(),
            approval_response_event(&grant_response, EventKind::ApprovalGranted, STEP_A),
        )
        .unwrap();
    let revoke = revocation_storage_draft();
    store
        .revoke_approval_grant(revoke.clone(), approval_revoked_event(&revoke))
        .unwrap();

    assert!(matches!(
        store.respond_to_approval(
            grant_response.clone(),
            approval_response_event(&grant_response, EventKind::ApprovalGranted, STEP_A),
        ),
        Err(StoreError::ApprovalGrantRevoked)
    ));
    let mut deny = grant_response;
    deny.decision = ApprovalResponseDecisionStorage::Deny;
    deny.grant_id = None;
    deny.approved_step_ids.clear();
    deny.expires_at = None;
    assert!(matches!(
        store.respond_to_approval(
            deny.clone(),
            approval_response_event(&deny, EventKind::ApprovalDenied, STEP_A),
        ),
        Err(StoreError::ApprovalGrantRevoked)
    ));
    let conn = store.conn.lock().unwrap();
    assert_eq!(count_kind_store(&conn, "APPROVAL_GRANTED"), 1);
    assert_eq!(count_kind_store(&conn, "APPROVAL_REVOKED"), 1);
    assert_eq!(count_kind_store(&conn, "APPROVAL_DENIED"), 0);
}

#[test]
fn independent_connections_cannot_both_consume_one_step_through_typed_api() {
    let path = TempDb::new();
    let setup = approval_store(&path.0);
    let request = two_action_request_draft();
    setup
        .create_approval_request(request.clone(), approval_required_event(&request))
        .unwrap();
    let grant_response = two_action_grant_storage_draft();
    setup
        .respond_to_approval(
            grant_response.clone(),
            approval_response_event(&grant_response, EventKind::ApprovalGranted, STEP_A),
        )
        .unwrap();
    drop(setup);

    let left = Store::open(&path.0, &FixedClock).unwrap();
    let right = Store::open(&path.0, &FixedClock).unwrap();
    let barrier = Arc::new(Barrier::new(3));
    let spawn = |store: Store, barrier: Arc<Barrier>| {
        std::thread::spawn(move || {
            barrier.wait();
            store.consume_grant_use(
                &GrantId::new(GRANT).unwrap(),
                &TaskId::new(TASK).unwrap(),
                &StepId::new(STEP_A).unwrap(),
                EpochMillis::new(300).unwrap(),
            )
        })
    };
    let a = spawn(left, barrier.clone());
    let b = spawn(right, barrier.clone());
    barrier.wait();
    let results = [a.join().unwrap(), b.join().unwrap()];
    let consumed = results
        .iter()
        .filter(|result| matches!(result, Ok(GrantConsumptionOutcome::Consumed { .. })))
        .count();
    assert_eq!(
        consumed, 1,
        "one and only one call must commit: {results:?}"
    );
    assert!(
        results.iter().all(|result| matches!(
            result,
            Ok(GrantConsumptionOutcome::Consumed { .. })
                | Ok(GrantConsumptionOutcome::AlreadyConsumed)
                | Err(StoreError::Busy)
        )),
        "unexpected typed consumption result: {results:?}"
    );

    let reopened = Store::open(&path.0, &FixedClock).unwrap();
    let conn = reopened.conn.lock().unwrap();
    assert_eq!(
        conn.query_row(
            "SELECT count(*) FROM approval_grant_uses WHERE step_id=?1",
            [STEP_A],
            |row| row.get::<_, i64>(0),
        )
        .unwrap(),
        1
    );
    assert_eq!(
        conn.query_row(
            "SELECT uses_remaining FROM approval_grants WHERE grant_id=?1",
            [GRANT],
            |row| row.get::<_, i64>(0),
        )
        .unwrap(),
        1
    );
}

/// Test-only composition probe for P8's missing runtime boundary. It uses the
/// real Store transaction, real Lease acquisition, real P6 grant-use writer,
/// real active-policy/registry/overlay rows, and candidate 0009/0010 SQL. It
/// does not authorize dispatch: budget semantics and durable Provider
/// availability are absent, and TaskEngine has no dispatch entry point.
fn p8_real_storage_fixture() -> (TempDb, Store) {
    p8_real_storage_fixture_with_tool_budget(24)
}

fn p8_real_storage_fixture_with_tool_budget(max_tool_calls: i64) -> (TempDb, Store) {
    let path = TempDb::new();
    let store = approval_store_with_tool_budget(&path.0, max_tool_calls);
    {
        let conn = store.conn.lock().unwrap();
        conn.execute_batch(include_str!(
            "../migrations/candidates/0009_p7_memory_schema_candidate.sql"
        ))
        .unwrap();
        conn.execute_batch(include_str!(
            "../migrations/candidates/0010_p8_dispatch_candidate.sql"
        ))
        .unwrap();
        conn.execute(
            "INSERT INTO owners(owner_id,epoch,state,created_at_ms) VALUES ('own_synthetic_p8',1,'ACTIVE',0)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO task_owner_bindings(task_id,owner_id,admitted_at_ms) VALUES (?1,'own_synthetic_p8',0)",
            [TASK],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO capability_overlays(capability_id,availability_state,experimental_opt_in,revision) VALUES ('calendar.event.create','ENABLED',0,1)",
            [],
        )
        .unwrap();
    }
    activate_test_policy(&store);
    let request = typed_request_draft();
    store
        .create_approval_request(request.clone(), approval_required_event(&request))
        .unwrap();
    let response = authenticated_grant_storage_draft();
    store
        .respond_to_approval(
            response.clone(),
            approval_response_event(&response, EventKind::ApprovalGranted, STEP_A),
        )
        .unwrap();
    // Exercise the actual Storage lease authority and persist its generation.
    let lease = store
        .transact(|tx| {
            tx.acquire_lease(
                TaskId::new(TASK).unwrap(),
                StepId::new(STEP_A).unwrap(),
                LeaseOwner::new("p8-test-worker-a").unwrap(),
                None,
                EpochMillis::new(301).unwrap(),
                EpochMillis::new(10_000).unwrap(),
            )
        })
        .unwrap();
    assert_eq!(lease.generation(), 1);
    drop(lease);
    (path, store)
}

fn p8_real_storage_fixture_with_two_eligible_steps(max_tool_calls: i64) -> (TempDb, Store) {
    let path = TempDb::new();
    let store = approval_store_with_tool_budget(&path.0, max_tool_calls);
    {
        let conn = store.conn.lock().unwrap();
        conn.execute_batch(include_str!(
            "../migrations/candidates/0009_p7_memory_schema_candidate.sql"
        ))
        .unwrap();
        conn.execute_batch(include_str!(
            "../migrations/candidates/0010_p8_dispatch_candidate.sql"
        ))
        .unwrap();
        conn.execute(
            "INSERT INTO owners(owner_id,epoch,state,created_at_ms) VALUES ('own_synthetic_p8',1,'ACTIVE',0)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO task_owner_bindings(task_id,owner_id,admitted_at_ms) VALUES (?1,'own_synthetic_p8',0)",
            [TASK],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO capability_overlays(capability_id,availability_state,experimental_opt_in,revision) VALUES ('calendar.event.create','ENABLED',0,1)",
            [],
        )
        .unwrap();
    }
    activate_test_policy(&store);
    let request = two_action_request_draft();
    store
        .create_approval_request(request.clone(), approval_required_event(&request))
        .unwrap();
    let response = two_action_grant_storage_draft();
    store
        .respond_to_approval(
            response.clone(),
            approval_response_event(&response, EventKind::ApprovalGranted, STEP_A),
        )
        .unwrap();
    for step_id in [STEP_A, STEP_B] {
        let lease = store
            .transact(|tx| {
                tx.acquire_lease(
                    TaskId::new(TASK).unwrap(),
                    StepId::new(step_id).unwrap(),
                    LeaseOwner::new("p8-test-worker-a").unwrap(),
                    None,
                    EpochMillis::new(301).unwrap(),
                    EpochMillis::new(10_000).unwrap(),
                )
            })
            .unwrap();
        assert_eq!(lease.generation(), 1);
        drop(lease);
    }
    (path, store)
}

fn p8_candidate_request_event(
    request_id: &str,
    event_id: &str,
    step_id: &str,
    now: EpochMillis,
) -> SereaEvent {
    SereaEvent {
        envelope_version: EnvelopeVersion::new("1").unwrap(),
        surface: WireSurface::new(WireSurface::EVENT).unwrap(),
        message_id: EventId::new(event_id).unwrap(),
        seq: Seq::new(0),
        kind: EventKind::CapabilityRequested,
        occurred_at: Timestamp::from_epoch_millis(now),
        correlation_id: Some(TaskId::new(TASK).unwrap()),
        causation_id: None,
        actor: Actor {
            kind: ActorKind::Host,
            id: ActorId::new("serea-core").unwrap(),
            version: SemVer::new("1.0.0").unwrap(),
            extensions: Default::default(),
        },
        data_class: DataClass::Personal,
        trace: Some(Trace {
            task_id: Some(TaskId::new(TASK).unwrap()),
            step_id: Some(StepId::new(step_id).unwrap()),
            attempt: Some(1),
            extensions: Default::default(),
        }),
        payload: json!({"request_id": request_id, "step_id": step_id})
            .as_object()
            .unwrap()
            .clone(),
        extensions: Default::default(),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum P8TestProviderReadiness {
    Unknown,
    Unavailable,
    SyntheticReadyForCompositionOnly,
}

/// This persists a candidate Intent in the same transaction as P6 consumption
/// and a protocol Event. All values are synthetic test inputs; no result is
/// exposed as a callable production dispatch API.
fn p8_real_storage_commit_probe(
    store: &Store,
    fail_after_grant: bool,
    fail_after_intent: bool,
) -> Result<(), StoreError> {
    p8_real_storage_commit_probe_with_readiness(
        store,
        P8TestProviderReadiness::SyntheticReadyForCompositionOnly,
        fail_after_grant,
        fail_after_intent,
    )
}

fn p8_real_storage_commit_probe_with_readiness(
    store: &Store,
    readiness: P8TestProviderReadiness,
    fail_after_grant: bool,
    fail_after_intent: bool,
) -> Result<(), StoreError> {
    p8_real_storage_commit_probe_for_step(
        store,
        STEP_A,
        "req_00000000000000000000000070",
        "evt_00000000000000000000000070",
        readiness,
        fail_after_grant,
        fail_after_intent,
    )
}

fn p8_real_storage_commit_probe_for_step(
    store: &Store,
    step_id: &str,
    request_id: &str,
    event_id: &str,
    readiness: P8TestProviderReadiness,
    fail_after_grant: bool,
    fail_after_intent: bool,
) -> Result<(), StoreError> {
    // The synthetic ready variant exists only to exercise database composition
    // in this cfg(test) helper. Unknown or unavailable readiness fails before
    // the candidate transaction; no Provider trait or runtime is involved.
    if readiness != P8TestProviderReadiness::SyntheticReadyForCompositionOnly {
        return Err(StoreError::ConstraintViolation);
    }
    let now = EpochMillis::new(301).unwrap();
    store.transact(|tx| {
        let task: (String, Option<i64>, Option<i64>, i64, i64, i64) = tx.inner.query_row(
            "SELECT state,cancelled_at_ms,deadline_at_ms,plan_revision,state_revision,max_tool_calls FROM tasks WHERE task_id=?1",
            [TASK],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?, row.get(5)?)),
        )?;
        if task.0 != "EXECUTING"
            || task.1.is_some()
            || task.2.is_some_and(|deadline| deadline <= now.get())
            || task.3 != 1
            || task.4 <= 0
        {
            return Err(StoreError::InvalidApprovalResponse);
        }
        // ADR-0036 accepts one max_tool_calls unit per durably committed
        // provider dispatch intent. Count inside this write transaction so
        // competing workers serialize. Candidate intents are append-only
        // because retention/Forget semantics are still unresolved.
        let committed_dispatches: i64 = tx.inner.query_row(
            "SELECT count(*) FROM dispatch_intents WHERE task_id=?1",
            [TASK],
            |row| row.get(0),
        )?;
        if committed_dispatches >= task.5 {
            return Err(StoreError::ConstraintViolation);
        }
        let step: (String, i64, String, String, i64) = tx.inner.query_row(
            "SELECT status,plan_revision,input_digest,idempotency_key,lease_generation FROM task_steps WHERE task_id=?1 AND step_id=?2",
            rusqlite::params![TASK, step_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?)),
        )?;
        if step.0 != "LEASED" || step.1 != 1 || step.4 <= 0 {
            return Err(StoreError::InvalidApprovalResponse);
        }
        let lease: (String, i64, i64, Option<i64>) = tx.inner.query_row(
            "SELECT owner,generation,expires_at_ms,released_at_ms FROM leases WHERE step_id=?1",
            [step_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )?;
        if lease.0 != "p8-test-worker-a"
            || lease.1 != step.4
            || lease.2 <= now.get()
            || lease.3.is_some()
        {
            return Err(StoreError::LeaseFenced);
        }
        let policy: (i64, String) = tx.inner.query_row(
            "SELECT p.active_revision_id,r.rules_digest FROM policy_state p JOIN policy_revisions r ON r.revision_id=p.active_revision_id WHERE p.singleton=1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?;
        if policy != (1, RULES.to_owned()) {
            return Err(StoreError::InvalidPolicyRevision);
        }
        let binding: i64 = tx.inner.query_row(
            "SELECT count(*) FROM step_capability_bindings b JOIN capability_generation_members m ON m.generation_id=b.generation_id AND m.descriptor_digest=b.descriptor_digest WHERE b.task_id=?1 AND b.step_id=?2 AND b.generation_id=1 AND b.descriptor_digest=?3 AND m.capability_id='calendar.event.create' AND m.capability_version='1.0.0' AND m.provider_id='calendar'",
            rusqlite::params![TASK, step_id, DESCRIPTOR],
            |row| row.get(0),
        )?;
        let owner_binding: i64 = tx.inner.query_row(
            "SELECT count(*) FROM task_owner_bindings WHERE task_id=?1 AND owner_id='own_synthetic_p8'",
            [TASK],
            |row| row.get(0),
        )?;
        let overlay: (String, i64) = tx.inner.query_row(
            "SELECT availability_state,revision FROM capability_overlays WHERE capability_id='calendar.event.create'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?;
        if binding != 1 || owner_binding != 1 || overlay != ("ENABLED".to_owned(), 1) {
            return Err(StoreError::InvalidApprovalResponse);
        }

        match tx.consume_grant_use(
            &GrantId::new(GRANT).unwrap(),
            &TaskId::new(TASK).unwrap(),
            &StepId::new(step_id).unwrap(),
            now,
        )? {
            GrantConsumptionOutcome::Consumed { .. } => {}
            GrantConsumptionOutcome::AlreadyConsumed
            | GrantConsumptionOutcome::Revoked
            | GrantConsumptionOutcome::Expired
            | GrantConsumptionOutcome::Exhausted => {
                return Err(StoreError::InvalidApprovalResponse);
            }
        }
        if fail_after_grant {
            return Err(StoreError::ConstraintViolation);
        }
        tx.inner.execute(
            "INSERT INTO dispatch_intents(request_id,idempotency_key,task_id,step_id,owner_id,grant_id,plan_revision,task_state_revision,lease_generation,policy_revision_id,policy_rules_digest,generation_id,descriptor_digest,capability_id,capability_version,provider_id,overlay_revision,arguments_digest,request_digest,event_id,committed_at_ms) VALUES (?1,?2,?3,?4,'own_synthetic_p8',?5,1,?6,?7,?8,?9,1,?10,'calendar.event.create','1.0.0','calendar',1,?11,?11,?12,?13)",
            rusqlite::params![
                request_id,
                step.3,
                TASK,
                step_id,
                GRANT,
                task.4,
                step.4,
                policy.0,
                policy.1,
                DESCRIPTOR,
                step.2,
                event_id,
                now.get(),
            ],
        )?;
        if fail_after_intent {
            return Err(StoreError::ConstraintViolation);
        }
        tx.append_event(
            p8_candidate_request_event(request_id, event_id, step_id, now),
            Some(EpochMillis::new(2_592_301).unwrap()),
        )?;
        Ok(())
    })
}

fn assert_p8_candidate_probe_rolled_back(path: &Path) {
    let reopened = Store::open(path, &FixedClock).unwrap();
    let conn = reopened.conn.lock().unwrap();
    assert_eq!(
        scalar(&conn, "SELECT count(*) FROM approval_grant_uses").unwrap(),
        0
    );
    assert_eq!(
        scalar(&conn, "SELECT uses_remaining FROM approval_grants").unwrap(),
        1
    );
    assert_eq!(
        scalar(&conn, "SELECT count(*) FROM dispatch_intents").unwrap(),
        0
    );
    assert_eq!(
        scalar(
            &conn,
            "SELECT count(*) FROM event_content WHERE message_id='evt_00000000000000000000000070'"
        )
        .unwrap(),
        0
    );
}

#[test]
fn real_storage_candidate_provider_readiness_unknown_or_unavailable_fails_closed() {
    for readiness in [
        P8TestProviderReadiness::Unknown,
        P8TestProviderReadiness::Unavailable,
    ] {
        let (path, store) = p8_real_storage_fixture();
        assert_eq!(
            p8_real_storage_commit_probe_with_readiness(&store, readiness, false, false),
            Err(StoreError::ConstraintViolation)
        );
        drop(store);
        assert_p8_candidate_probe_rolled_back(&path.0);
    }
}

#[test]
fn real_storage_p6_grant_intent_and_event_join_one_sqlite_transaction() {
    // RED boundary: the implementation fails closed if any live P6 fact has
    // changed, while an approved synthetic fixture commits through actual Tx.
    for (fail_after_grant, fail_after_intent) in [(true, false), (false, true)] {
        let (path, store) = p8_real_storage_fixture();
        assert!(p8_real_storage_commit_probe(&store, fail_after_grant, fail_after_intent).is_err());
        drop(store);
        assert_p8_candidate_probe_rolled_back(&path.0);
    }

    let (path, store) = p8_real_storage_fixture();
    p8_real_storage_commit_probe(&store, false, false).unwrap();
    assert_eq!(
        scalar(
            &store.conn.lock().unwrap(),
            "SELECT count(*) FROM dispatch_intents WHERE task_id='tsk_01JQ8Z9K3M7QWXR4V2T6YH0BNA'",
        )
        .unwrap(),
        1,
        "the committed intent is the candidate's durable tool-call unit"
    );
    // The already-used Grant is a past operation record, never authority to
    // create a second intent for the same Step.
    assert!(p8_real_storage_commit_probe(&store, false, false).is_err());
    // Existing TaskEngine deletion delegates to Tx::delete_task. The candidate
    // FK makes an unresolved Intent deletion fail closed on the real Store DB.
    assert!(
        store
            .transact(|tx| tx.delete_task(&TaskId::new(TASK).unwrap()))
            .is_err()
    );
    assert!(
        store
            .conn
            .lock()
            .unwrap()
            .execute(
                "DELETE FROM dispatch_intents WHERE request_id='req_00000000000000000000000070'",
                [],
            )
            .is_err()
    );
    drop(store);
    let reopened = Store::open(&path.0, &FixedClock).unwrap();
    reopened.verify_integrity().unwrap();
    let conn = reopened.conn.lock().unwrap();
    assert_eq!(
        scalar(&conn, "SELECT count(*) FROM approval_grant_uses").unwrap(),
        1
    );
    assert_eq!(
        scalar(&conn, "SELECT uses_remaining FROM approval_grants").unwrap(),
        0
    );
    assert_eq!(
        scalar(&conn, "SELECT count(*) FROM dispatch_intents").unwrap(),
        1
    );
    assert_eq!(
        scalar(
            &conn,
            "SELECT count(*) FROM tasks WHERE task_id='tsk_01JQ8Z9K3M7QWXR4V2T6YH0BNA'"
        )
        .unwrap(),
        1
    );
    assert_eq!(
        scalar(
            &conn,
            "SELECT count(*) FROM event_content WHERE message_id='evt_00000000000000000000000070'"
        )
        .unwrap(),
        1
    );
}

#[test]
fn real_storage_candidate_budget_exhaustion_rolls_back_grant_and_all_dispatch_rows() {
    let (path, store) = p8_real_storage_fixture_with_tool_budget(0);

    assert_eq!(
        scalar(
            &store.conn.lock().unwrap(),
            "SELECT max_tool_calls FROM tasks WHERE task_id='tsk_01JQ8Z9K3M7QWXR4V2T6YH0BNA'",
        )
        .unwrap(),
        0
    );
    assert_eq!(
        p8_real_storage_commit_probe(&store, false, false),
        Err(StoreError::ConstraintViolation)
    );
    drop(store);
    assert_p8_candidate_probe_rolled_back(&path.0);
}

#[test]
fn real_storage_unresolved_intent_survives_second_store_deletes_and_event_expiry() {
    let (path, store) = p8_real_storage_fixture();
    p8_real_storage_commit_probe(&store, false, false).unwrap();
    drop(store);

    // Store::open must apply connection-local foreign-key enforcement on each
    // independent connection. This remains a candidate-schema test, not a
    // production retention or Forget contract.
    let reopened = Store::open(&path.0, &FixedClock).unwrap();
    let conn = reopened.conn.lock().unwrap();
    let foreign_keys: i64 = conn
        .query_row("PRAGMA foreign_keys", [], |row| row.get(0))
        .unwrap();
    assert_eq!(foreign_keys, 1);

    assert!(
        conn.execute("DELETE FROM tasks WHERE task_id=?1", [TASK])
            .is_err(),
        "Task deletion must preserve unresolved Intent evidence"
    );
    assert!(
        conn.execute("DELETE FROM task_steps WHERE step_id=?1", [STEP_A])
            .is_err(),
        "Step deletion must preserve unresolved Intent evidence"
    );
    assert!(
        conn.execute(
            "DELETE FROM approval_grant_uses WHERE task_id=?1 AND step_id=?2",
            rusqlite::params![TASK, STEP_A],
        )
        .is_err(),
        "consumed Grant-use deletion must preserve unresolved Intent evidence"
    );
    assert!(
        conn.execute(
            "DELETE FROM dispatch_intents WHERE request_id=?1",
            ["req_00000000000000000000000070"],
        )
        .is_err(),
        "direct Intent deletion must be refused"
    );

    let intents_before_expiry: i64 = conn
        .query_row(
            "SELECT count(*) FROM dispatch_intents WHERE request_id='req_00000000000000000000000070' AND idempotency_key=(SELECT idempotency_key FROM task_steps WHERE step_id=?1)",
            [STEP_A],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(intents_before_expiry, 1);
    drop(conn);

    // Event retention is independent from the candidate Intent record. The
    // Event is intentionally pruned after its configured deadline, while the
    // exact RequestId/IDK budget evidence survives another Store reopen.
    let expired = reopened
        .expire_eligible_events(EpochMillis::new(3_000_000).unwrap(), 32)
        .unwrap();
    assert!(expired.content_records_deleted > 0);
    drop(reopened);

    let final_store = Store::open(&path.0, &FixedClock).unwrap();
    let conn = final_store.conn.lock().unwrap();
    assert_eq!(
        scalar(
            &conn,
            "SELECT count(*) FROM event_content WHERE message_id='evt_00000000000000000000000070'",
        )
        .unwrap(),
        0,
        "the expired event content should be pruned"
    );
    assert_eq!(
        conn.query_row(
            "SELECT count(*) FROM dispatch_intents WHERE request_id='req_00000000000000000000000070' AND idempotency_key=(SELECT idempotency_key FROM task_steps WHERE step_id=?1)",
            [STEP_A],
            |row| row.get::<_, i64>(0),
        )
        .unwrap(),
        1,
        "event pruning must not erase or rewrite unresolved Intent evidence"
    );
}

#[test]
fn real_storage_task_budget_limit_one_serializes_two_eligible_steps() {
    let (path, setup) = p8_real_storage_fixture_with_two_eligible_steps(1);
    drop(setup);

    let workers = [
        (
            STEP_A,
            "req_00000000000000000000000070",
            "evt_00000000000000000000000070",
        ),
        (
            STEP_B,
            "req_00000000000000000000000071",
            "evt_00000000000000000000000071",
        ),
    ];
    let barrier = Arc::new(Barrier::new(3));
    let joins = workers.map(|(step_id, request_id, event_id)| {
        let store = Store::open(&path.0, &FixedClock).unwrap();
        let barrier = barrier.clone();
        std::thread::spawn(move || {
            barrier.wait();
            p8_real_storage_commit_probe_for_step(
                &store,
                step_id,
                request_id,
                event_id,
                P8TestProviderReadiness::SyntheticReadyForCompositionOnly,
                false,
                false,
            )
        })
    });
    barrier.wait();
    let outcomes = joins.map(|join| join.join().unwrap());
    assert_eq!(outcomes.iter().filter(|result| result.is_ok()).count(), 1);
    assert_eq!(
        outcomes
            .iter()
            .filter(|result| **result == Err(StoreError::ConstraintViolation))
            .count(),
        1,
        "the second eligible Step must be refused at max_tool_calls=1: {outcomes:?}"
    );

    let reopened = Store::open(&path.0, &FixedClock).unwrap();
    let conn = reopened.conn.lock().unwrap();
    assert_eq!(
        scalar(
            &conn,
            "SELECT max_tool_calls FROM tasks WHERE task_id='tsk_01JQ8Z9K3M7QWXR4V2T6YH0BNA'"
        )
        .unwrap(),
        1
    );
    assert_eq!(
        scalar(&conn, "SELECT count(*) FROM dispatch_intents").unwrap(),
        1
    );
    assert_eq!(
        scalar(&conn, "SELECT count(*) FROM approval_grant_uses").unwrap(),
        1
    );
    assert_eq!(
        conn.query_row(
            "SELECT uses_remaining FROM approval_grants WHERE grant_id=?1",
            [GRANT],
            |row| row.get::<_, i64>(0),
        )
        .unwrap(),
        1
    );
    assert_eq!(
        scalar(&conn, "SELECT count(*) FROM event_content WHERE message_id IN ('evt_00000000000000000000000070','evt_00000000000000000000000071')").unwrap(),
        1,
        "the refused second Step must not leave a candidate Event"
    );
    assert_eq!(
        scalar(&conn, "SELECT count(*) FROM dispatch_intents WHERE step_id IN ('stp_01JQ8Z9M5T9WXK2H4BNPQ7RDSF','stp_01JQ8Z9M5T9WXK2H4BNPQ7RDST')").unwrap(),
        1
    );
}

#[test]
fn real_storage_task_budget_limit_two_commits_exactly_two_eligible_steps() {
    let (path, setup) = p8_real_storage_fixture_with_two_eligible_steps(2);
    drop(setup);

    let workers = [
        (
            STEP_A,
            "req_00000000000000000000000070",
            "evt_00000000000000000000000070",
        ),
        (
            STEP_B,
            "req_00000000000000000000000071",
            "evt_00000000000000000000000071",
        ),
    ];
    let barrier = Arc::new(Barrier::new(3));
    let joins = workers.map(|(step_id, request_id, event_id)| {
        let store = Store::open(&path.0, &FixedClock).unwrap();
        let barrier = barrier.clone();
        std::thread::spawn(move || {
            barrier.wait();
            p8_real_storage_commit_probe_for_step(
                &store,
                step_id,
                request_id,
                event_id,
                P8TestProviderReadiness::SyntheticReadyForCompositionOnly,
                false,
                false,
            )
        })
    });
    barrier.wait();
    let outcomes = joins.map(|join| join.join().unwrap());
    assert_eq!(
        outcomes.iter().filter(|result| result.is_ok()).count(),
        2,
        "both distinct approved Steps should fit exactly within budget 2: {outcomes:?}"
    );

    let reopened = Store::open(&path.0, &FixedClock).unwrap();
    reopened.verify_integrity().unwrap();
    let conn = reopened.conn.lock().unwrap();
    assert_eq!(
        scalar(
            &conn,
            "SELECT max_tool_calls FROM tasks WHERE task_id='tsk_01JQ8Z9K3M7QWXR4V2T6YH0BNA'"
        )
        .unwrap(),
        2
    );
    assert_eq!(
        scalar(&conn, "SELECT count(*) FROM dispatch_intents").unwrap(),
        2
    );
    assert_eq!(
        scalar(&conn, "SELECT count(*) FROM approval_grant_uses").unwrap(),
        2
    );
    assert_eq!(
        conn.query_row(
            "SELECT uses_remaining FROM approval_grants WHERE grant_id=?1",
            [GRANT],
            |row| row.get::<_, i64>(0),
        )
        .unwrap(),
        0
    );
    assert_eq!(
        scalar(
            &conn,
            "SELECT count(*) FROM event_content WHERE message_id IN ('evt_00000000000000000000000070','evt_00000000000000000000000071')"
        )
        .unwrap(),
        2
    );
    assert_eq!(
        scalar(
            &conn,
            "SELECT count(DISTINCT step_id) FROM dispatch_intents"
        )
        .unwrap(),
        2
    );
}

#[test]
fn real_storage_boundary_fails_closed_on_event_failure_revocation_policy_and_lease_change() {
    // Event persistence failure after the P6 use and candidate Intent has
    // started must roll the entire SQLite transaction back.
    let (path, store) = p8_real_storage_fixture();
    store
        .conn
        .lock()
        .unwrap()
        .execute_batch(
            "CREATE TRIGGER p8_test_fail_event BEFORE INSERT ON event_content
             BEGIN SELECT RAISE(ABORT,'injected P8 event failure'); END;",
        )
        .unwrap();
    assert!(p8_real_storage_commit_probe(&store, false, false).is_err());
    drop(store);
    assert_p8_candidate_probe_rolled_back(&path.0);

    // Cancellation through the real Tx lifecycle writer is re-read at the
    // transaction boundary before the P6 Grant-use is touched.
    struct P8CancelAudit;
    impl TaskAuditParticipant for P8CancelAudit {
        fn records(&self, facts: &DurableTransition) -> Result<JournalRecords, StoreError> {
            Ok(vec![JournalRecord {
                kind: JournalKind::TaskCancelRequested,
                state_from: facts.task_from().map(|state| state.wire_name().to_owned()),
                state_to: Some(facts.task_to().wire_name().to_owned()),
                reason: None,
                payload_json: b"{}".to_vec(),
            }])
        }
    }
    let (path, store) = p8_real_storage_fixture();
    let actor = ActorId::new("local-user").unwrap();
    let actor_version = SemVer::new("1.0.0").unwrap();
    let context = TransitionContext {
        actor_kind: ActorKind::User,
        actor_id: &actor,
        actor_version: &actor_version,
        causation_id: None,
    };
    store
        .transact_with_participants(&P8CancelAudit, &ApprovalTaskEvents, |tx| {
            tx.cancel_task(
                &TaskId::new(TASK).unwrap(),
                TaskOriginKind::new("USER_MESSAGE").unwrap(),
                EpochMillis::new(350).unwrap(),
                &context,
            )
        })
        .unwrap();
    assert!(p8_real_storage_commit_probe(&store, false, false).is_err());
    drop(store);
    assert_p8_candidate_probe_rolled_back(&path.0);

    // The actual P6 revocation operation is authoritative in the next
    // transaction; the attempted candidate write observes it and refuses.
    let (path, store) = p8_real_storage_fixture();
    let revoke = revocation_storage_draft();
    store
        .revoke_approval_grant(revoke.clone(), approval_revoked_event(&revoke))
        .unwrap();
    assert!(p8_real_storage_commit_probe(&store, false, false).is_err());
    drop(store);
    let reopened = Store::open(&path.0, &FixedClock).unwrap();
    let conn = reopened.conn.lock().unwrap();
    assert_eq!(
        scalar(&conn, "SELECT count(*) FROM approval_grant_uses").unwrap(),
        0
    );
    assert_eq!(
        scalar(&conn, "SELECT count(*) FROM dispatch_intents").unwrap(),
        0
    );
    assert_eq!(
        text(
            &conn,
            "SELECT status FROM approval_grants WHERE grant_id=?1",
            GRANT
        )
        .unwrap(),
        "REVOKED"
    );
    drop(conn);

    // An active Policy pointer change cannot be hidden by the earlier policy
    // snapshot. This test setup mutates rows directly; runtime authority is not
    // being inferred from the test fixture.
    let (path, store) = p8_real_storage_fixture();
    {
        let conn = store.conn.lock().unwrap();
        insert_revision(&conn, 2, false);
        insert_rule(&conn, 2, "p8_policy_change", "ALLOW");
        activate_seed_revision(&conn, 2);
        conn.execute(
            "UPDATE policy_state SET active_revision_id=2 WHERE singleton=1",
            [],
        )
        .unwrap();
    }
    assert!(p8_real_storage_commit_probe(&store, false, false).is_err());
    drop(store);
    assert_p8_candidate_probe_rolled_back(&path.0);

    // Deadline equality is expired: it refuses before the existing Grant-use
    // accounting is touched.
    let (path, store) = p8_real_storage_fixture();
    store
        .conn
        .lock()
        .unwrap()
        .execute(
            "UPDATE tasks SET deadline_at_ms=301 WHERE task_id=?1",
            [TASK],
        )
        .unwrap();
    assert!(p8_real_storage_commit_probe(&store, false, false).is_err());
    drop(store);
    assert_p8_candidate_probe_rolled_back(&path.0);

    // A newer lease generation fences the stale worker before it consumes the
    // real Grant-use row.
    let (path, store) = p8_real_storage_fixture();
    store
        .transact(|tx| {
            tx.acquire_lease(
                TaskId::new(TASK).unwrap(),
                StepId::new(STEP_A).unwrap(),
                LeaseOwner::new("p8-test-worker-b").unwrap(),
                Some(1),
                EpochMillis::new(10_001).unwrap(),
                EpochMillis::new(20_000).unwrap(),
            )
        })
        .unwrap();
    assert_eq!(
        p8_real_storage_commit_probe(&store, false, false),
        Err(StoreError::LeaseFenced)
    );
    drop(store);
    assert_p8_candidate_probe_rolled_back(&path.0);
}

#[test]
fn independent_connections_admit_at_most_one_real_storage_candidate_intent() {
    let (path, setup) = p8_real_storage_fixture();
    drop(setup);
    let left = Store::open(&path.0, &FixedClock).unwrap();
    let right = Store::open(&path.0, &FixedClock).unwrap();
    let barrier = Arc::new(Barrier::new(3));
    let spawn = |store: Store, barrier: Arc<Barrier>| {
        std::thread::spawn(move || {
            barrier.wait();
            p8_real_storage_commit_probe(&store, false, false)
        })
    };
    let left_worker = spawn(left, barrier.clone());
    let right_worker = spawn(right, barrier.clone());
    barrier.wait();
    let outcomes = [left_worker.join().unwrap(), right_worker.join().unwrap()];
    assert_eq!(outcomes.iter().filter(|result| result.is_ok()).count(), 1);
    let reopened = Store::open(&path.0, &FixedClock).unwrap();
    reopened.verify_integrity().unwrap();
    let conn = reopened.conn.lock().unwrap();
    assert_eq!(
        scalar(&conn, "SELECT count(*) FROM approval_grant_uses").unwrap(),
        1
    );
    assert_eq!(
        scalar(&conn, "SELECT count(*) FROM dispatch_intents").unwrap(),
        1
    );
    assert_eq!(
        scalar(
            &conn,
            "SELECT count(*) FROM event_content WHERE message_id='evt_00000000000000000000000070'"
        )
        .unwrap(),
        1
    );
}

#[test]
fn independent_connections_can_consume_distinct_r2_members_concurrently() {
    let path = TempDb::new();
    let setup = approval_store(&path.0);
    let request = two_action_request_draft();
    setup
        .create_approval_request(request.clone(), approval_required_event(&request))
        .unwrap();
    let response = two_action_grant_storage_draft();
    setup
        .respond_to_approval(
            response.clone(),
            approval_response_event(&response, EventKind::ApprovalGranted, STEP_A),
        )
        .unwrap();
    drop(setup);

    let left = Store::open(&path.0, &FixedClock).unwrap();
    let right = Store::open(&path.0, &FixedClock).unwrap();
    let barrier = Arc::new(Barrier::new(3));
    let spawn = |store: Store, step_id: &'static str, barrier: Arc<Barrier>| {
        std::thread::spawn(move || {
            barrier.wait();
            store.consume_grant_use(
                &GrantId::new(GRANT).unwrap(),
                &TaskId::new(TASK).unwrap(),
                &StepId::new(step_id).unwrap(),
                EpochMillis::new(300).unwrap(),
            )
        })
    };
    let action_a = spawn(left, STEP_A, barrier.clone());
    let action_b = spawn(right, STEP_B, barrier.clone());
    barrier.wait();
    let outcomes = [action_a.join().unwrap(), action_b.join().unwrap()];
    assert!(
        outcomes
            .iter()
            .all(|outcome| matches!(outcome, Ok(GrantConsumptionOutcome::Consumed { .. }))),
        "both individually enumerated R2 members must consume exactly once: {outcomes:?}"
    );

    let reopened = Store::open(&path.0, &FixedClock).unwrap();
    let conn = reopened.conn.lock().unwrap();
    assert_eq!(
        scalar(&conn, "SELECT count(*) FROM approval_grant_uses").unwrap(),
        2
    );
    assert_eq!(
        scalar(&conn, "SELECT uses_remaining FROM approval_grants").unwrap(),
        0
    );
    assert_eq!(
        text(
            &conn,
            "SELECT status FROM approval_grants WHERE grant_id=?1",
            GRANT
        )
        .unwrap(),
        "EXHAUSTED"
    );
    assert_eq!(
        scalar(
            &conn,
            "SELECT count(DISTINCT step_id) FROM approval_grant_uses",
        )
        .unwrap(),
        2
    );
}

#[test]
fn independent_connections_serialize_r2_member_consumption_against_revocation() {
    let path = TempDb::new();
    let setup = approval_store(&path.0);
    let request = two_action_request_draft();
    setup
        .create_approval_request(request.clone(), approval_required_event(&request))
        .unwrap();
    let response = two_action_grant_storage_draft();
    setup
        .respond_to_approval(
            response.clone(),
            approval_response_event(&response, EventKind::ApprovalGranted, STEP_A),
        )
        .unwrap();
    drop(setup);

    let consumer = Store::open(&path.0, &FixedClock).unwrap();
    let revoker = Store::open(&path.0, &FixedClock).unwrap();
    let barrier = Arc::new(Barrier::new(3));
    let consume_barrier = barrier.clone();
    let consume = std::thread::spawn(move || {
        consume_barrier.wait();
        let first = consumer.consume_grant_use(
            &GrantId::new(GRANT).unwrap(),
            &TaskId::new(TASK).unwrap(),
            &StepId::new(STEP_A).unwrap(),
            EpochMillis::new(300).unwrap(),
        );
        if matches!(first, Err(StoreError::Busy)) {
            consumer.consume_grant_use(
                &GrantId::new(GRANT).unwrap(),
                &TaskId::new(TASK).unwrap(),
                &StepId::new(STEP_A).unwrap(),
                EpochMillis::new(300).unwrap(),
            )
        } else {
            first
        }
    });
    let revoke_barrier = barrier.clone();
    let revoke = std::thread::spawn(move || {
        let draft = revocation_storage_draft();
        revoke_barrier.wait();
        let first = revoker.revoke_approval_grant(draft.clone(), approval_revoked_event(&draft));
        if matches!(first, Err(StoreError::Busy)) {
            revoker.revoke_approval_grant(draft.clone(), approval_revoked_event(&draft))
        } else {
            first
        }
    });
    barrier.wait();
    let consume_result = consume.join().unwrap();
    let revoke_result = revoke.join().unwrap();
    assert!(matches!(
        consume_result,
        Ok(GrantConsumptionOutcome::Consumed { .. }) | Ok(GrantConsumptionOutcome::Revoked)
    ));
    assert!(matches!(
        revoke_result,
        Ok(ApprovalRevocationOutcome::Revoked)
    ));

    let reopened = Store::open(&path.0, &FixedClock).unwrap();
    let conn = reopened.conn.lock().unwrap();
    assert_eq!(
        text(
            &conn,
            "SELECT status FROM approval_grants WHERE grant_id=?1",
            GRANT
        )
        .unwrap(),
        "REVOKED"
    );
    assert_eq!(count_kind_store(&conn, "APPROVAL_REVOKED"), 1);
    let consumed = scalar(&conn, "SELECT count(*) FROM approval_grant_uses").unwrap();
    assert!(consumed == 0 || consumed == 1);
    drop(conn);
    assert_eq!(
        reopened
            .consume_grant_use(
                &GrantId::new(GRANT).unwrap(),
                &TaskId::new(TASK).unwrap(),
                &StepId::new(STEP_B).unwrap(),
                EpochMillis::new(500).unwrap(),
            )
            .unwrap(),
        GrantConsumptionOutcome::Revoked
    );
}

#[test]
fn independent_connections_serialize_task_cancellation_against_authenticated_response() {
    struct CancelAudit;
    impl TaskAuditParticipant for CancelAudit {
        fn records(&self, facts: &DurableTransition) -> Result<JournalRecords, StoreError> {
            Ok(vec![JournalRecord {
                kind: JournalKind::TaskCancelRequested,
                state_from: facts.task_from().map(|state| state.wire_name().to_owned()),
                state_to: Some(facts.task_to().wire_name().to_owned()),
                reason: None,
                payload_json: b"{}".to_vec(),
            }])
        }
    }

    let (_path, setup, _) = waiting_approval_store();
    drop(setup);
    let response_store = Store::open(&_path.0, &FixedClock).unwrap();
    let cancel_store = Store::open(&_path.0, &FixedClock).unwrap();
    let barrier = Arc::new(Barrier::new(3));
    let response_barrier = barrier.clone();
    let response = std::thread::spawn(move || {
        let draft = authenticated_grant_storage_draft();
        response_barrier.wait();
        let first = response_store.respond_to_approval(
            draft.clone(),
            approval_response_event(&draft, EventKind::ApprovalGranted, STEP_A),
        );
        if matches!(first, Err(StoreError::Busy)) {
            response_store.respond_to_approval(
                draft.clone(),
                approval_response_event(&draft, EventKind::ApprovalGranted, STEP_A),
            )
        } else {
            first
        }
    });
    let cancel_barrier = barrier.clone();
    let cancel = std::thread::spawn(move || {
        let actor = ActorId::new("local-user").unwrap();
        let version = SemVer::new("1.0.0").unwrap();
        let context = TransitionContext {
            actor_kind: ActorKind::User,
            actor_id: &actor,
            actor_version: &version,
            causation_id: None,
        };
        cancel_barrier.wait();
        let first =
            cancel_store.transact_with_participants(&CancelAudit, &ApprovalTaskEvents, |tx| {
                tx.cancel_task(
                    &TaskId::new(TASK).unwrap(),
                    TaskOriginKind::new("USER_MESSAGE").unwrap(),
                    EpochMillis::new(275).unwrap(),
                    &context,
                )
            });
        if matches!(first, Err(StoreError::Busy)) {
            cancel_store.transact_with_participants(&CancelAudit, &ApprovalTaskEvents, |tx| {
                tx.cancel_task(
                    &TaskId::new(TASK).unwrap(),
                    TaskOriginKind::new("USER_MESSAGE").unwrap(),
                    EpochMillis::new(275).unwrap(),
                    &context,
                )
            })
        } else {
            first
        }
    });
    barrier.wait();
    let response_result = response.join().unwrap();
    let cancel_result = cancel.join().unwrap();
    assert!(matches!(
        response_result,
        Ok(ApprovalResponseStorageOutcome::Granted(_)) | Err(StoreError::TaskNotApprovable)
    ));
    assert!(matches!(
        cancel_result,
        Ok(crate::CancellationOutcome {
            changed: true,
            already_terminal: false,
            ..
        })
    ));

    let reopened = Store::open(&_path.0, &FixedClock).unwrap();
    let conn = reopened.conn.lock().unwrap();
    assert_eq!(
        text(&conn, "SELECT state FROM tasks WHERE task_id=?1", TASK).unwrap(),
        "CANCELLED"
    );
    let status = text(
        &conn,
        "SELECT status FROM approval_requests WHERE approval_id=?1",
        APPROVAL,
    )
    .unwrap();
    assert!(status == "PENDING" || status == "APPROVED");
    let grant_count = scalar(&conn, "SELECT count(*) FROM approval_grants").unwrap();
    assert_eq!(grant_count, i64::from(status == "APPROVED"));
    assert_eq!(count_kind_store(&conn, "APPROVAL_GRANTED"), grant_count);
    assert_eq!(
        scalar(
            &conn,
            "SELECT count(*) FROM task_journal WHERE journal_kind='TASK_CANCEL_REQUESTED'"
        )
        .unwrap(),
        1
    );
    drop(conn);
    if status == "APPROVED" {
        assert_eq!(
            reopened
                .consume_grant_use(
                    &GrantId::new(GRANT).unwrap(),
                    &TaskId::new(TASK).unwrap(),
                    &StepId::new(STEP_A).unwrap(),
                    EpochMillis::new(300).unwrap(),
                )
                .unwrap_err(),
            StoreError::InvalidApprovalResponse
        );
    }
}

#[test]
fn independent_connections_serialize_approval_against_inclusive_request_expiry() {
    let path = TempDb::new();
    let setup = approval_store(&path.0);
    let request = typed_request_draft();
    setup
        .create_approval_request(request.clone(), approval_required_event(&request))
        .unwrap();
    drop(setup);

    let responder = Store::open(&path.0, &FixedClock).unwrap();
    let expirer = Store::open(&path.0, &FixedClock).unwrap();
    let barrier = Arc::new(Barrier::new(3));
    let response_barrier = barrier.clone();
    let response = std::thread::spawn(move || {
        let draft = authenticated_grant_storage_draft();
        response_barrier.wait();
        let first = responder.respond_to_approval(
            draft.clone(),
            approval_response_event(&draft, EventKind::ApprovalGranted, STEP_A),
        );
        if matches!(first, Err(StoreError::Busy)) {
            responder.respond_to_approval(
                draft.clone(),
                approval_response_event(&draft, EventKind::ApprovalGranted, STEP_A),
            )
        } else {
            first
        }
    });
    let expire_barrier = barrier.clone();
    let expire = std::thread::spawn(move || {
        expire_barrier.wait();
        let first = expirer.expire_approval_request(
            &request.approval_id,
            request.expires_at,
            approval_expired_event(&request, request.expires_at),
        );
        if matches!(first, Err(StoreError::Busy)) {
            expirer.expire_approval_request(
                &request.approval_id,
                request.expires_at,
                approval_expired_event(&request, request.expires_at),
            )
        } else {
            first
        }
    });
    barrier.wait();
    let response_result = response.join().unwrap();
    let expire_result = expire.join().unwrap();
    assert!(matches!(
        response_result,
        Ok(ApprovalResponseStorageOutcome::Granted(_)) | Err(StoreError::ApprovalRequestExpired)
    ));
    assert!(matches!(expire_result, Ok(true) | Ok(false)));

    let reopened = Store::open(&path.0, &FixedClock).unwrap();
    let conn = reopened.conn.lock().unwrap();
    let status = text(
        &conn,
        "SELECT status FROM approval_requests WHERE approval_id=?1",
        APPROVAL,
    )
    .unwrap();
    assert!(status == "APPROVED" || status == "EXPIRED");
    let approved = status == "APPROVED";
    assert_eq!(
        scalar(&conn, "SELECT count(*) FROM approval_grants").unwrap(),
        i64::from(approved)
    );
    assert_eq!(
        count_kind_store(&conn, "APPROVAL_GRANTED"),
        i64::from(approved)
    );
    assert_eq!(
        count_kind_store(&conn, "APPROVAL_EXPIRED"),
        i64::from(!approved)
    );
}

#[test]
fn independent_connections_serialize_grant_and_deny_responses_without_dual_outcome() {
    let path = TempDb::new();
    let setup = approval_store(&path.0);
    let request = typed_request_draft();
    setup
        .create_approval_request(request.clone(), approval_required_event(&request))
        .unwrap();
    drop(setup);
    let left = Store::open(&path.0, &FixedClock).unwrap();
    let right = Store::open(&path.0, &FixedClock).unwrap();
    let barrier = Arc::new(Barrier::new(2));
    let grant_barrier = barrier.clone();
    let grant = std::thread::spawn(move || {
        let response = authenticated_grant_storage_draft();
        grant_barrier.wait();
        left.respond_to_approval(
            response.clone(),
            approval_response_event(&response, EventKind::ApprovalGranted, STEP_A),
        )
    });
    let deny = std::thread::spawn(move || {
        let mut response = authenticated_grant_storage_draft();
        response.grant_id = None;
        response.decision = ApprovalResponseDecisionStorage::Deny;
        response.approved_step_ids.clear();
        response.expires_at = None;
        barrier.wait();
        right.respond_to_approval(
            response.clone(),
            approval_response_event(&response, EventKind::ApprovalDenied, STEP_A),
        )
    });
    let outcomes = [grant.join().unwrap(), deny.join().unwrap()];
    assert_eq!(
        outcomes
            .iter()
            .filter(|result| matches!(
                result,
                Ok(ApprovalResponseStorageOutcome::Granted(_)
                    | ApprovalResponseStorageOutcome::Denied)
            ))
            .count(),
        1
    );
    assert_eq!(
        outcomes
            .iter()
            .filter(|result| matches!(result, Err(StoreError::ApprovalResponseConflict)))
            .count(),
        1
    );
    let reopened = Store::open(&path.0, &FixedClock).unwrap();
    let conn = reopened.conn.lock().unwrap();
    assert_eq!(
        scalar(&conn, "SELECT count(*) FROM approval_grants").unwrap()
            + scalar(
                &conn,
                "SELECT count(*) FROM approval_requests WHERE status='DENIED'"
            )
            .unwrap(),
        1
    );
    assert_eq!(
        count_kind_store(&conn, "APPROVAL_GRANTED") + count_kind_store(&conn, "APPROVAL_DENIED"),
        1
    );
    drop(conn);
    reopened.verify_integrity().unwrap();
}

#[test]
fn independent_connections_replay_of_same_grant_is_idempotent_without_double_event() {
    let path = TempDb::new();
    let setup = approval_store(&path.0);
    let request = typed_request_draft();
    setup
        .create_approval_request(request.clone(), approval_required_event(&request))
        .unwrap();
    drop(setup);
    let left = Store::open(&path.0, &FixedClock).unwrap();
    let right = Store::open(&path.0, &FixedClock).unwrap();
    let barrier = Arc::new(Barrier::new(2));
    let other = barrier.clone();
    let grant_a = std::thread::spawn(move || {
        let response = authenticated_grant_storage_draft();
        barrier.wait();
        left.respond_to_approval(
            response.clone(),
            approval_response_event(&response, EventKind::ApprovalGranted, STEP_A),
        )
    });
    let grant_b = std::thread::spawn(move || {
        let response = authenticated_grant_storage_draft();
        other.wait();
        right.respond_to_approval(
            response.clone(),
            approval_response_event(&response, EventKind::ApprovalGranted, STEP_A),
        )
    });
    let outcomes = [grant_a.join().unwrap(), grant_b.join().unwrap()];
    assert_eq!(
        outcomes
            .iter()
            .filter(|result| matches!(result, Ok(ApprovalResponseStorageOutcome::Granted(_))))
            .count(),
        1
    );
    assert_eq!(
        outcomes
            .iter()
            .filter(|result| matches!(
                result,
                Ok(ApprovalResponseStorageOutcome::AlreadyGranted(_))
            ))
            .count(),
        1
    );
    let reopened = Store::open(&path.0, &FixedClock).unwrap();
    let conn = reopened.conn.lock().unwrap();
    assert_eq!(
        scalar(&conn, "SELECT count(*) FROM approval_grants").unwrap(),
        1
    );
    assert_eq!(
        scalar(&conn, "SELECT count(*) FROM approval_grant_members").unwrap(),
        1
    );
    assert_eq!(count_kind_store(&conn, "APPROVAL_GRANTED"), 1);
    drop(conn);
    reopened.verify_integrity().unwrap();
}

#[cfg(not(feature = "p7-experimental"))]
#[test]
fn default_build_does_not_apply_or_expose_p7_experimental_schema() {
    let store = Store::open_in_memory(&FixedClock).unwrap();
    assert_eq!(store.schema_version().unwrap(), 8);
    let conn = store.conn.lock().unwrap();
    let p7: i64 = conn
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_schema WHERE name='p7_memory_items')",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(p7, 0);
}
