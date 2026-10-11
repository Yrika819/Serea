//! P6B RED-first durable policy and approval contract tests.
//!
//! Every obligation in the P6B closure list is asserted against real SQLite
//! authority, not against a Rust mirror. Nothing here proves a physical
//! power-loss guarantee: the crash cases are transaction-boundary faults plus
//! a reopen, and the note is repeated where it applies.

use crate::{Migrations, Store, StoreError, Tx};
use rusqlite::{Connection, TransactionBehavior, params};
use serea_protocol::{Clock, EpochMillis, ProtocolError};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

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

fn insert_revision(conn: &Connection, revision: i64, activated: bool) {
    conn.execute(
        "INSERT INTO policy_revisions(revision_id,rules_digest,rule_count,created_at_ms,activated_at_ms,
                                      actor_class,actor_id,reason_code)
         VALUES (?1,?2,1,500,?3,'HOST','local-admin','SEED')",
        params![revision, RULES, activated.then_some(600)],
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

// ---------------------------------------------------------------------------
// 1 and 2 — upgrade and fresh migration
// ---------------------------------------------------------------------------

#[test]
fn migration_catalog_appends_p6b_without_rewriting_any_prior_migration() {
    let catalog = Migrations::embedded();
    assert_eq!(Migrations::LATEST, 6);
    assert_eq!(catalog.len(), 6);
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
}

#[test]
fn fresh_store_applies_the_whole_p6_schema_at_v6() {
    let store = Store::open_in_memory(&FixedClock).unwrap();
    assert_eq!(store.schema_version().unwrap(), 6);
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
fn existing_schema_v4_upgrades_through_v5_to_v6_and_keeps_its_rows() {
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
    assert_eq!(store.schema_version().unwrap(), 6);
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
            "SELECT count(*) FROM schema_migrations WHERE version=6"
        )
        .unwrap(),
        1
    );
}

#[test]
fn existing_schema_v5_upgrades_to_v6_without_rewriting_migration_0005() {
    let temp = TempDb::new();
    seed_v5(&temp.0);
    let before = Migrations::checksum(Migrations::embedded()[4].sql);
    let store = Store::open(&temp.0, &FixedClock).unwrap();
    assert_eq!(store.schema_version().unwrap(), 6);
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
        assert_eq!(store.schema_version().unwrap(), 6);
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
    insert_revision(&conn, 1, true);
    insert_rule(&conn, 1, "pol_0001", "ALLOW");
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
    // Rule identity is unique within a revision. A priority tie between two
    // *different* rules is legal and is exactly what rule_id ASC resolves, so
    // only the duplicate rule_id is refused.
    assert!(
        conn.execute(
            "INSERT INTO policy_rules(revision_id,rule_id,priority,enabled,decision,reason_code)
             VALUES (1,'pol_0001',10,1,'DENY','DUP')",
            []
        )
        .is_err(),
        "duplicate rule_id within one revision must be refused"
    );
    conn.execute(
        "INSERT INTO policy_rules(revision_id,rule_id,priority,enabled,decision,reason_code)
         VALUES (1,'pol_0002',10,1,'ALLOW','TIE')",
        [],
    )
    .unwrap();
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
    assert_eq!(reopened.schema_version().unwrap(), 6);
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
