use rusqlite::params;
use serde_json::{Map, Value, json};
use serea_protocol::{
    ActorId, Authorization, CapabilityId, Digest, EpochMillis, EventId, ReasonCode, RiskClass,
    SideEffectClass,
};

use crate::{
    EventDraft, Migrations, PolicyActorClass, PolicyAutomationContext, PolicyDecisionKind,
    PolicyRevisionDraft, PolicyRuleDraft, PolicyRuleMatchDraft, Store, StoreError, Tx,
};

struct FixedClock;
impl serea_protocol::Clock for FixedClock {
    fn now_ms(&self) -> Result<EpochMillis, serea_protocol::ProtocolError> {
        EpochMillis::new(1_000)
    }
}

fn digest(fill: char) -> Digest {
    Digest::new(format!("sha256:{}", fill.to_string().repeat(64))).unwrap()
}

fn revision_draft(rules: Vec<PolicyRuleDraft>) -> PolicyRevisionDraft {
    let rules_digest = crate::policy::policy_rules_digest(&rules).unwrap();
    PolicyRevisionDraft {
        rules_digest,
        created_at: EpochMillis::new(1_000).unwrap(),
        actor_class: PolicyActorClass::Host,
        actor_id: ActorId::new("serea-core").unwrap(),
        reason_code: ReasonCode::new("TEST_POLICY_CHANGE").unwrap(),
        rules,
    }
}

#[test]
fn storage_refuses_policy_digest_that_does_not_bind_the_complete_rule_set() {
    let store = Store::open_in_memory(&FixedClock).unwrap();
    let mut draft = revision_draft(vec![enabled_match_rule()]);
    draft.rules_digest = digest('a');
    let result = store.transact(|tx| tx.prepare_policy_revision(draft));
    assert!(matches!(result, Err(StoreError::InvalidPolicyRevision)));
    let conn = store.conn.lock().unwrap();
    let revisions: i64 = conn
        .query_row("SELECT count(*) FROM policy_revisions", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(revisions, 0);
}

fn enabled_match_rule() -> PolicyRuleDraft {
    PolicyRuleDraft {
        rule_id: "disabled_capability".into(),
        priority: 1,
        enabled: true,
        matcher: PolicyRuleMatchDraft {
            capability_id: Some(CapabilityId::new("mail.messages.list").unwrap()),
            risk_class: Some(RiskClass::Observe),
            side_effect_class: Some(SideEffectClass::None),
            authorization: Some(Authorization::None),
            automation_context: Some(PolicyAutomationContext::Interactive),
            requested_by: None,
            enabled: Some(false),
        },
        decision: PolicyDecisionKind::Deny,
        reason_code: ReasonCode::new("CAPABILITY_DISABLED").unwrap(),
    }
}

fn policy_event(
    revision_id: i64,
    previous_revision_id: Option<i64>,
    rules_digest: &Digest,
    reason_code: &ReasonCode,
    payload_padding: usize,
) -> EventDraft {
    let mut payload = Map::new();
    payload.insert(
        "change_kind".into(),
        Value::String("REVISION_ACTIVATED".into()),
    );
    payload.insert("revision_id".into(), Value::from(revision_id));
    payload.insert(
        "previous_revision_id".into(),
        previous_revision_id.map(Value::from).unwrap_or(Value::Null),
    );
    payload.insert(
        "rules_digest".into(),
        Value::String(rules_digest.to_string()),
    );
    payload.insert("reason_code".into(), Value::String(reason_code.to_string()));
    if payload_padding > 0 {
        payload.insert("padding".into(), Value::String("x".repeat(payload_padding)));
    }
    EventDraft {
        event: serde_json::from_value(json!({
            "envelope_version":"1",
            "surface":"serea.event/1",
            "message_id":"evt_01JQ8ZB7H2XKM9P4QW7NRT5YCD",
            "seq":"0",
            "kind":"POLICY_CHANGED",
            "occurred_at":"1970-01-01T00:00:01.000Z",
            "correlation_id":null,
            "causation_id":null,
            "actor":{"kind":"HOST","id":"serea-core","version":"0.1.0"},
            "data_class":"PUBLIC",
            "trace":null,
            "payload":payload
        }))
        .unwrap(),
        retention_at: None,
    }
}

#[test]
fn current_policy_snapshot_round_trips_exact_closed_dimensions() {
    let store = Store::open_in_memory(&FixedClock).unwrap();
    let revision_id = store
        .transact(|tx: &mut Tx<'_>| {
            tx.prepare_policy_revision(revision_draft(vec![enabled_match_rule()]))
        })
        .unwrap();
    let snapshot = store.get_policy_revision(revision_id).unwrap().unwrap();
    assert_eq!(snapshot.rules.len(), 1);
    assert_eq!(snapshot.rules[0].matcher.enabled, Some(false));
    assert_eq!(
        snapshot.rules[0]
            .matcher
            .capability_id
            .as_ref()
            .unwrap()
            .as_str(),
        "mail.messages.list"
    );
    assert!(store.current_policy_revision().unwrap().is_none());
}

#[test]
fn policy_activation_and_matching_event_commit_together() {
    let store = Store::open_in_memory(&FixedClock).unwrap();
    let revision_id = store
        .transact(|tx: &mut Tx<'_>| tx.prepare_policy_revision(revision_draft(Vec::new())))
        .unwrap();
    let snapshot = store.get_policy_revision(revision_id).unwrap().unwrap();
    let event = policy_event(
        revision_id,
        None,
        &snapshot.rules_digest,
        &snapshot.reason_code,
        0,
    );
    store
        .transact(|tx| {
            tx.activate_policy_revision(revision_id, EpochMillis::new(1_000).unwrap(), event)?;
            Ok(())
        })
        .unwrap();
    let active = store.current_policy_revision().unwrap().unwrap();
    assert_eq!(active.revision_id, revision_id);
    assert_eq!(active.activated_at, Some(EpochMillis::new(1_000).unwrap()));
    let conn = store.conn.lock().unwrap();
    let events: i64 = conn
        .query_row(
            "SELECT count(*) FROM event_content WHERE kind='POLICY_CHANGED'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(events, 1);
}

#[test]
fn caught_event_append_failure_rolls_back_activation_and_pointer() {
    let store = Store::open_in_memory(&FixedClock).unwrap();
    let revision_id = store
        .transact(|tx: &mut Tx<'_>| tx.prepare_policy_revision(revision_draft(Vec::new())))
        .unwrap();
    let snapshot = store.get_policy_revision(revision_id).unwrap().unwrap();
    let event = policy_event(
        revision_id,
        None,
        &snapshot.rules_digest,
        &snapshot.reason_code,
        40_000,
    );
    store
        .transact(|tx| {
            assert_eq!(
                tx.activate_policy_revision(revision_id, EpochMillis::new(1_000).unwrap(), event),
                Err(StoreError::EventPayloadTooLarge)
            );
            Ok(())
        })
        .unwrap();
    assert!(store.current_policy_revision().unwrap().is_none());
    assert_eq!(
        store
            .get_policy_revision(revision_id)
            .unwrap()
            .unwrap()
            .activated_at,
        None
    );
    let conn = store.conn.lock().unwrap();
    let events: i64 = conn
        .query_row(
            "SELECT count(*) FROM event_content WHERE kind='POLICY_CHANGED'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(events, 0);
}

#[test]
fn direct_sql_cannot_activate_partial_rules_or_append_after_activation() {
    let store = Store::open_in_memory(&FixedClock).unwrap();
    let conn = store.conn.lock().unwrap();
    conn.execute(
        "INSERT INTO policy_revisions(rules_digest,rule_count,created_at_ms,actor_class,actor_id,reason_code) VALUES (?1,1,1000,'HOST','serea-core','TEST_POLICY_CHANGE')",
        params![digest('a').as_str()],
    ).unwrap();
    let incomplete_id = conn.last_insert_rowid();
    assert!(
        conn.execute(
            "UPDATE policy_revisions SET activated_at_ms=1000 WHERE revision_id=?1",
            [incomplete_id],
        )
        .is_err()
    );

    let store = Store::open_in_memory(&FixedClock).unwrap();
    let id = store
        .transact(|tx| tx.prepare_policy_revision(revision_draft(Vec::new())))
        .unwrap();
    let snapshot = store.get_policy_revision(id).unwrap().unwrap();
    let event = policy_event(id, None, &snapshot.rules_digest, &snapshot.reason_code, 0);
    store
        .transact(|tx| {
            tx.activate_policy_revision(id, EpochMillis::new(1_000).unwrap(), event)?;
            Ok(())
        })
        .unwrap();
    let conn = store.conn.lock().unwrap();
    assert!(conn.execute(
        "INSERT INTO policy_rules(revision_id,rule_id,priority,enabled,decision,reason_code) VALUES (?1,'late_rule',0,1,'ALLOW','POLICY_RULE')",
        [id],
    ).is_err());
}

#[test]
fn migration_seven_is_forward_only_and_policy_schema_version_is_current() {
    assert_eq!(Migrations::LATEST, 7);
    assert_eq!(Migrations::embedded().len(), 7);
    assert_eq!(Migrations::embedded()[6].version, 7);
    assert_eq!(
        Migrations::embedded()[6].name,
        "0007_policy_revision_sealing"
    );
    let store = Store::open_in_memory(&FixedClock).unwrap();
    assert_eq!(store.schema_version().unwrap(), 7);
    assert_eq!(
        EventId::new("evt_01JQ8ZB7H2XKM9P4QW7NRT5YCD")
            .unwrap()
            .as_str(),
        "evt_01JQ8ZB7H2XKM9P4QW7NRT5YCD"
    );
}

#[test]
fn existing_v6_store_upgrades_to_v7_without_rewriting_frozen_migrations() {
    let path = std::env::temp_dir().join(format!(
        "serea-policy-v6-upgrade-{}.sqlite",
        std::process::id()
    ));
    let _ = std::fs::remove_file(&path);
    let mut conn = rusqlite::Connection::open(&path).unwrap();
    crate::migrate::apply(
        &mut conn,
        &Migrations::embedded()[..6],
        EpochMillis::new(1_000).unwrap(),
        true,
    )
    .unwrap();
    drop(conn);

    let store = Store::open(&path, &FixedClock).unwrap();
    assert_eq!(store.schema_version().unwrap(), 7);
    let conn = store.conn.lock().unwrap();
    for version in [5_i64, 6] {
        let recorded: String = conn
            .query_row(
                "SELECT checksum FROM schema_migrations WHERE version=?1",
                [version],
                |row| row.get(0),
            )
            .unwrap();
        let embedded = Migrations::embedded()[usize::try_from(version - 1).unwrap()].sql;
        assert_eq!(recorded, Migrations::checksum(embedded).as_str());
    }
    let has_match_dimension: i64 = conn
        .query_row(
            "SELECT count(*) FROM pragma_table_info('policy_rules') WHERE name='match_enabled'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(has_match_dimension, 1);
    drop(conn);
    drop(store);
    let _ = std::fs::remove_file(&path);
    let _ = std::fs::remove_file(path.with_extension("sqlite-wal"));
    let _ = std::fs::remove_file(path.with_extension("sqlite-shm"));
}
