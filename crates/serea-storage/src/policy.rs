//! Typed SQLite access to the P6 policy revision authority.

use std::collections::BTreeSet;
use std::str::FromStr;

use rusqlite::{OptionalExtension, params};
use serde_json::{Value, json};
use serea_protocol::{
    ActorId, ActorKind, Authorization, CapabilityId, Digest, EpochMillis, ReasonCode, RequestedBy,
    RiskClass, SereaEvent, SideEffectClass,
};
use sha2::{Digest as _, Sha256};

use crate::{EventDraft, Store, StoreError, Tx};

/// Host actor category that created a policy revision.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PolicyActorClass {
    /// Serea host configuration or a host administrator.
    Host,
    /// Authenticated local user configuration.
    User,
}

impl PolicyActorClass {
    fn wire_name(self) -> &'static str {
        match self {
            Self::Host => "HOST",
            Self::User => "USER",
        }
    }
}

/// Closed policy automation contexts stored as exact enum names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PolicyAutomationContext {
    /// User-directed or delegated host work.
    Interactive,
    /// Work created by a durable schedule.
    Scheduled,
    /// Read-only proactive watcher work.
    Proactive,
    /// Host maintenance work.
    System,
}

impl PolicyAutomationContext {
    fn wire_name(self) -> &'static str {
        match self {
            Self::Interactive => "INTERACTIVE",
            Self::Scheduled => "SCHEDULED",
            Self::Proactive => "PROACTIVE",
            Self::System => "SYSTEM",
        }
    }
}

/// Closed durable rule decisions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PolicyDecisionKind {
    /// Selects allow subject to hard policy gates.
    Allow,
    /// Explicit refusal.
    Deny,
    /// Ordinary authenticated approval is required.
    RequireApproval,
    /// Credential handoff is required.
    RequireHandoff,
}

impl PolicyDecisionKind {
    fn wire_name(self) -> &'static str {
        match self {
            Self::Allow => "ALLOW",
            Self::Deny => "DENY",
            Self::RequireApproval => "REQUIRE_APPROVAL",
            Self::RequireHandoff => "REQUIRE_HANDOFF",
        }
    }
}

/// Exact optional match dimensions for one durable policy rule.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PolicyRuleMatchDraft {
    /// Exact capability match.
    pub capability_id: Option<CapabilityId>,
    /// Exact risk match.
    pub risk_class: Option<RiskClass>,
    /// Exact side-effect match.
    pub side_effect_class: Option<SideEffectClass>,
    /// Exact authorization match.
    pub authorization: Option<Authorization>,
    /// Exact host-derived automation context.
    pub automation_context: Option<PolicyAutomationContext>,
    /// Exact trusted provenance match.
    pub requested_by: Option<RequestedBy>,
    /// Exact capability overlay enabled value.
    pub enabled: Option<bool>,
}

/// One row in an immutable policy revision.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PolicyRuleDraft {
    /// Stable identifier, unique within one revision.
    pub rule_id: String,
    /// Signed 32-bit evaluation priority.
    pub priority: i32,
    /// Whether this rule participates in evaluation.
    pub enabled: bool,
    /// Closed exact-value match dimensions.
    pub matcher: PolicyRuleMatchDraft,
    /// Typed result.
    pub decision: PolicyDecisionKind,
    /// Stable machine-readable explanation code.
    pub reason_code: ReasonCode,
}

/// Complete trusted request to prepare a new policy revision.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PolicyRevisionDraft {
    /// Digest of the complete normalized policy rule array.
    pub rules_digest: Digest,
    /// Trusted creation instant.
    pub created_at: EpochMillis,
    /// Authenticated host actor category.
    pub actor_class: PolicyActorClass,
    /// Host-owned actor identity.
    pub actor_id: ActorId,
    /// Stable configuration-change reason.
    pub reason_code: ReasonCode,
    /// Full rule set, bounded to 512 and inserted atomically.
    pub rules: Vec<PolicyRuleDraft>,
}

/// Immutable policy revision read back from SQLite.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PolicyRevisionSnapshot {
    /// Monotonic database revision identity.
    pub revision_id: i64,
    /// Canonical digest of all rules.
    pub rules_digest: Digest,
    /// Trusted creation time.
    pub created_at: EpochMillis,
    /// Activation time, or absent while prepared only.
    pub activated_at: Option<EpochMillis>,
    /// Revision actor category.
    pub actor_class: PolicyActorClass,
    /// Revision actor identity.
    pub actor_id: ActorId,
    /// Revision reason.
    pub reason_code: ReasonCode,
    /// Exact persisted rules.
    pub rules: Vec<PolicyRuleDraft>,
}

impl Store {
    /// Reads the active policy snapshot and its rules in one SQLite transaction.
    pub fn current_policy_revision(&self) -> Result<Option<PolicyRevisionSnapshot>, StoreError> {
        self.transact(|tx| tx.current_policy_revision())
    }

    /// Reads a policy revision by its immutable identity.
    pub fn get_policy_revision(
        &self,
        revision_id: i64,
    ) -> Result<Option<PolicyRevisionSnapshot>, StoreError> {
        self.transact(|tx| tx.get_policy_revision(revision_id))
    }
}

impl Tx<'_> {
    /// Prepares the complete immutable rule rows for a future activation.
    pub fn prepare_policy_revision(
        &mut self,
        draft: PolicyRevisionDraft,
    ) -> Result<i64, StoreError> {
        self.ensure_active()?;
        if draft.rules.len() > 512 {
            return Err(StoreError::InvalidPolicyRevision);
        }
        if policy_rules_digest(&draft.rules)? != draft.rules_digest {
            return Err(StoreError::InvalidPolicyRevision);
        }
        let mut ids = BTreeSet::new();
        for rule in &draft.rules {
            if !valid_rule_id(&rule.rule_id) || !ids.insert(rule.rule_id.as_str()) {
                return Err(StoreError::InvalidPolicyRevision);
            }
        }
        let revision_id = self.operation_savepoint(|tx| {
            let (actor_class, actor_id) = (draft.actor_class.wire_name(), draft.actor_id.as_str());
            tx.inner.execute(
                "INSERT INTO policy_revisions(rules_digest,rule_count,created_at_ms,actor_class,actor_id,reason_code) VALUES (?1,?2,?3,?4,?5,?6)",
                params![draft.rules_digest.as_str(), i64::try_from(draft.rules.len()).map_err(|_| StoreError::InvalidPolicyRevision)?, draft.created_at.get(), actor_class, actor_id, draft.reason_code.as_str()],
            ).map_err(policy_sql_error)?;
            let id = tx.inner.last_insert_rowid();
            for rule in &draft.rules {
                insert_rule(&tx.inner, id, rule)?;
            }
            Ok(id)
        })?;
        Ok(revision_id)
    }

    /// Reads a complete immutable policy snapshot by identity.
    pub fn get_policy_revision(
        &mut self,
        revision_id: i64,
    ) -> Result<Option<PolicyRevisionSnapshot>, StoreError> {
        self.ensure_active()?;
        if revision_id <= 0 {
            return Ok(None);
        }
        self.inner.query_row(
            "SELECT revision_id,rules_digest,rule_count,created_at_ms,activated_at_ms,actor_class,actor_id,reason_code FROM policy_revisions WHERE revision_id=?1",
            [revision_id],
            |row| Ok((row.get::<_,i64>(0)?,row.get::<_,String>(1)?,row.get::<_,i64>(2)?,row.get::<_,i64>(3)?,row.get::<_,Option<i64>>(4)?,row.get::<_,String>(5)?,row.get::<_,String>(6)?,row.get::<_,String>(7)?)),
        ).optional()?.map(|(id,digest,count,created,activated,actor_class,actor_id,reason)| {
            let count = usize::try_from(count).map_err(|_| StoreError::CorruptRow)?;
            let rules = load_rules(&self.inner, id)?;
            if rules.len() != count { return Err(StoreError::CorruptRow); }
            let rules_digest = Digest::from_str(&digest).map_err(|_| StoreError::CorruptRow)?;
            if policy_rules_digest(&rules).map_err(|_| StoreError::CorruptRow)? != rules_digest {
                return Err(StoreError::CorruptRow);
            }
            Ok(PolicyRevisionSnapshot {
                revision_id: id,
                rules_digest,
                created_at: EpochMillis::new(created).map_err(|_| StoreError::CorruptRow)?,
                activated_at: activated.map(EpochMillis::new).transpose().map_err(|_| StoreError::CorruptRow)?,
                actor_class: decode_actor_class(&actor_class)?,
                actor_id: ActorId::new(actor_id).map_err(|_| StoreError::CorruptRow)?,
                reason_code: ReasonCode::new(reason).map_err(|_| StoreError::CorruptRow)?,
                rules,
            })
        }).transpose()
    }

    /// Reads the single active policy revision and all its rules atomically.
    pub fn current_policy_revision(
        &mut self,
    ) -> Result<Option<PolicyRevisionSnapshot>, StoreError> {
        self.ensure_active()?;
        let id = self.inner.query_row(
            "SELECT active_revision_id FROM policy_state WHERE singleton=1",
            [],
            |row| row.get::<_, Option<i64>>(0),
        )?;
        match id {
            Some(id) => self.get_policy_revision(id),
            None => Ok(None),
        }
    }

    /// Activates a prepared revision and appends its matching event in one savepoint.
    ///
    /// The caller must construct the event through Event Bus. This storage boundary verifies
    /// that it names the same revision/digest and actor; event append failure rolls activation
    /// and the pointer update back even if an outer transaction catches the returned error.
    pub fn activate_policy_revision(
        &mut self,
        revision_id: i64,
        activated_at: EpochMillis,
        event: EventDraft,
    ) -> Result<PolicyRevisionSnapshot, StoreError> {
        self.ensure_active()?;
        self.operation_savepoint(|tx| {
            let snapshot = tx.get_policy_revision(revision_id)?.ok_or(StoreError::InvalidPolicyRevision)?;
            if snapshot.activated_at.is_some() { return Err(StoreError::InvalidPolicyRevision); }
            if snapshot.rules.len() > 512 { return Err(StoreError::InvalidPolicyRevision); }
            if activated_at < snapshot.created_at { return Err(StoreError::InvalidTimestamp); }
            validate_policy_event(&event.event, &snapshot)?;
            if event.event.occurred_at.to_epoch_millis() != activated_at {
                return Err(StoreError::InvalidPolicyRevision);
            }
            let previous = tx.inner.query_row(
                "SELECT active_revision_id FROM policy_state WHERE singleton=1",
                [],
                |row| row.get::<_, Option<i64>>(0),
            )?;
            if event.event.payload.get("previous_revision_id").and_then(|value| value.as_i64()) != previous {
                return Err(StoreError::InvalidPolicyRevision);
            }
            tx.inner.execute(
                "UPDATE policy_revisions SET activated_at_ms=?1 WHERE revision_id=?2 AND activated_at_ms IS NULL",
                params![activated_at.get(), revision_id],
            ).map_err(policy_sql_error)?;
            if tx.inner.changes() != 1 { return Err(StoreError::InvalidPolicyRevision); }
            tx.inner.execute(
                "UPDATE policy_state SET active_revision_id=?1 WHERE singleton=1",
                [revision_id],
            ).map_err(policy_sql_error)?;
            tx.append_event(event.event, event.retention_at)?;
            tx.get_policy_revision(revision_id)?.ok_or(StoreError::InvalidPolicyRevision)
        })
    }
}

fn insert_rule(
    conn: &rusqlite::Connection,
    revision_id: i64,
    rule: &PolicyRuleDraft,
) -> Result<(), StoreError> {
    let m = &rule.matcher;
    conn.execute(
        "INSERT INTO policy_rules(revision_id,rule_id,priority,capability_id,risk_class,side_effect_class,authorization,automation_context,requested_by,enabled,decision,reason_code,match_enabled) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13)",
        params![revision_id,rule.rule_id,rule.priority,m.capability_id.as_ref().map(ToString::to_string),m.risk_class.map(|v| v.wire_name()),m.side_effect_class.map(|v| v.wire_name()),m.authorization.map(|v| v.wire_name()),m.automation_context.map(PolicyAutomationContext::wire_name),m.requested_by.map(|v| v.wire_name()),rule.enabled,rule.decision.wire_name(),rule.reason_code.as_str(),m.enabled],
    ).map_err(policy_sql_error)?;
    Ok(())
}

fn load_rules(
    conn: &rusqlite::Connection,
    revision_id: i64,
) -> Result<Vec<PolicyRuleDraft>, StoreError> {
    let mut stmt = conn.prepare(
        "SELECT rule_id,priority,capability_id,risk_class,side_effect_class,authorization,automation_context,requested_by,enabled,decision,reason_code,match_enabled FROM policy_rules WHERE revision_id=?1 ORDER BY rule_id ASC"
    )?;
    let rows = stmt.query_map([revision_id], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, i64>(1)?,
            row.get::<_, Option<String>>(2)?,
            row.get::<_, Option<String>>(3)?,
            row.get::<_, Option<String>>(4)?,
            row.get::<_, Option<String>>(5)?,
            row.get::<_, Option<String>>(6)?,
            row.get::<_, Option<String>>(7)?,
            row.get::<_, bool>(8)?,
            row.get::<_, String>(9)?,
            row.get::<_, String>(10)?,
            row.get::<_, Option<bool>>(11)?,
        ))
    })?;
    let mut rules = Vec::new();
    for row in rows {
        let (
            rule_id,
            priority,
            capability,
            risk,
            effect,
            authorization,
            context,
            requested_by,
            enabled,
            decision,
            reason,
            match_enabled,
        ) = row?;
        let priority = i32::try_from(priority).map_err(|_| StoreError::CorruptRow)?;
        rules.push(PolicyRuleDraft {
            rule_id,
            priority,
            enabled,
            matcher: PolicyRuleMatchDraft {
                capability_id: capability
                    .map(|v| CapabilityId::from_str(&v).map_err(|_| StoreError::CorruptRow))
                    .transpose()?,
                risk_class: risk.map(|v| decode_enum(&v)).transpose()?,
                side_effect_class: effect.map(|v| decode_enum(&v)).transpose()?,
                authorization: authorization.map(|v| decode_enum(&v)).transpose()?,
                automation_context: context.map(|v| decode_context(&v)).transpose()?,
                requested_by: requested_by.map(|v| decode_enum(&v)).transpose()?,
                enabled: match_enabled,
            },
            decision: decode_decision(&decision)?,
            reason_code: ReasonCode::new(reason).map_err(|_| StoreError::CorruptRow)?,
        });
    }
    Ok(rules)
}

fn decode_enum<T: for<'de> serde::Deserialize<'de>>(value: &str) -> Result<T, StoreError> {
    serde_json::from_str(&format!("\"{value}\"")).map_err(|_| StoreError::CorruptRow)
}

fn decode_context(value: &str) -> Result<PolicyAutomationContext, StoreError> {
    match value {
        "INTERACTIVE" => Ok(PolicyAutomationContext::Interactive),
        "SCHEDULED" => Ok(PolicyAutomationContext::Scheduled),
        "PROACTIVE" => Ok(PolicyAutomationContext::Proactive),
        "SYSTEM" => Ok(PolicyAutomationContext::System),
        _ => Err(StoreError::CorruptRow),
    }
}

fn decode_decision(value: &str) -> Result<PolicyDecisionKind, StoreError> {
    match value {
        "ALLOW" => Ok(PolicyDecisionKind::Allow),
        "DENY" => Ok(PolicyDecisionKind::Deny),
        "REQUIRE_APPROVAL" => Ok(PolicyDecisionKind::RequireApproval),
        "REQUIRE_HANDOFF" => Ok(PolicyDecisionKind::RequireHandoff),
        _ => Err(StoreError::CorruptRow),
    }
}

fn decode_actor_class(value: &str) -> Result<PolicyActorClass, StoreError> {
    match value {
        "HOST" => Ok(PolicyActorClass::Host),
        "USER" => Ok(PolicyActorClass::User),
        _ => Err(StoreError::CorruptRow),
    }
}

fn validate_policy_event(
    event: &SereaEvent,
    revision: &PolicyRevisionSnapshot,
) -> Result<(), StoreError> {
    let expected_actor = match revision.actor_class {
        PolicyActorClass::Host => ActorKind::Host,
        PolicyActorClass::User => ActorKind::User,
    };
    if event.kind.wire_name() != "POLICY_CHANGED"
        || event.data_class != serea_protocol::DataClass::Public
        || event.actor.kind != expected_actor
        || event.actor.id.as_str() != revision.actor_id.as_str()
        || event.payload.get("revision_id").and_then(|v| v.as_i64()) != Some(revision.revision_id)
        || event.payload.get("rules_digest").and_then(|v| v.as_str())
            != Some(revision.rules_digest.as_str())
        || event.payload.get("reason_code").and_then(|v| v.as_str())
            != Some(revision.reason_code.as_str())
    {
        return Err(StoreError::InvalidPolicyRevision);
    }
    Ok(())
}

fn valid_rule_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 64
        && id.as_bytes()[0].is_ascii_alphabetic()
        && id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
}

pub(crate) fn policy_rules_digest(rules: &[PolicyRuleDraft]) -> Result<Digest, StoreError> {
    let mut sorted = rules.iter().collect::<Vec<_>>();
    sorted.sort_by(|left, right| left.rule_id.cmp(&right.rule_id));
    let value = Value::Array(
        sorted
            .into_iter()
            .map(|rule| {
                let matcher = &rule.matcher;
                json!({
                    "rule_id": rule.rule_id,
                    "priority": rule.priority,
                    "enabled": rule.enabled,
                    "match": {
                        "capability_id": matcher.capability_id.as_ref().map(ToString::to_string),
                        "risk_class": matcher.risk_class.map(|value| value.wire_name()),
                        "side_effect_class": matcher.side_effect_class.map(|value| value.wire_name()),
                        "authorization": matcher.authorization.map(|value| value.wire_name()),
                        "automation_context": matcher.automation_context.map(PolicyAutomationContext::wire_name),
                        "requested_by": matcher.requested_by.map(|value| value.wire_name()),
                        "enabled": matcher.enabled,
                    },
                    "decision": rule.decision.wire_name(),
                    "reason_code": rule.reason_code.as_str(),
                })
            })
            .collect(),
    );
    let source = serde_json::to_string(&value).map_err(|_| StoreError::CanonicalJson)?;
    let canonical = serea_protocol::canonicalize(&source).map_err(|_| StoreError::CanonicalJson)?;
    let mut hex = String::with_capacity(64);
    for byte in Sha256::digest(canonical) {
        use std::fmt::Write as _;
        write!(&mut hex, "{byte:02x}").map_err(|_| StoreError::CanonicalJson)?;
    }
    Digest::new(format!("sha256:{hex}")).map_err(|_| StoreError::InvalidPolicyRevision)
}

fn policy_sql_error(error: rusqlite::Error) -> StoreError {
    match error {
        rusqlite::Error::SqliteFailure(ref failure, _)
            if failure.code == rusqlite::ErrorCode::ConstraintViolation =>
        {
            StoreError::ConstraintViolation
        }
        _ => StoreError::Sqlite,
    }
}
