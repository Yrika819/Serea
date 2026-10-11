//! `serea-policy` — deterministic policy evaluation, approval request and grant
//! lifecycle, and the P6 authorization evidence P8 revalidates.
//!
//! P6 owns everything between the frozen P5 `PreparedActionV1` and the P8
//! dispatch boundary (ADR-0037 through ADR-0040). It is a pure function of
//! durable state and trusted host input: it holds no model in the loop, no
//! network call in the loop, and no wall clock that the rules can read.
//!
//! The crate deliberately does **not** invoke a provider. P8 remains the first
//! phase permitted to call `CapabilityProvider::invoke`. It mints no `RequestId`,
//! creates no dispatch intent, runs no duplicate suppression and keeps no
//! repeated-action counter. Its output is `AuthorizationEvidenceV1` — evidence
//! about state at a named policy revision — never an execution permission and
//! never an `approved: bool`.
//!
//! Approval authority is lent, bounded, expiring and revocable. A grant binds an
//! exact task, an exact capability and version, an exact registry generation and
//! descriptor revision, an exact plan revision, an exact expiry, and — under
//! owner decision R2 — an explicitly enumerated set of 1 to 8 individually
//! approved actions, each with its own `StepId` and its own exact
//! `arguments_digest`. Membership, never scope or digest equality, is what
//! authorizes a consuming Step.
//!
//! No credential material is stored anywhere in this crate. `serea-storage`
//! refuses `SECRET` and `CREDENTIAL` mechanically, and the persisted approval row
//! is bounded to the ratified `PUBLIC`/`PERSONAL` ceiling.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

mod approval;

use std::collections::BTreeSet;
use std::fmt;

use serde_json::{Value, json};
use serea_event_bus::{EventBus, PolicyChangedEventV1};
use serea_protocol::canonical::canonicalize;
use serea_protocol::{
    ActorId, Authorization, CapabilityId, DataClass, Digest, EpochMillis, ImplementationId,
    ProviderId, ReasonCode, RequestedBy, RiskClass, RootRequirement, SemVer, SideEffectClass,
    StepId, TaskId, TaskKind, TaskState,
};
use serea_storage::{
    PolicyActorClass, PolicyAutomationContext, PolicyDecisionKind, PolicyRevisionDraft,
    PolicyRevisionSnapshot, PolicyRuleDraft, PolicyRuleMatchDraft, Store, StoreError,
};
use sha2::{Digest as _, Sha256};

pub use approval::{
    ApprovalActionSetError, ApprovalActionSetV1, ApprovalActionV1, ApprovalAuthenticationError,
    ApprovalAuthenticationStrengthV1, ApprovalAuthenticationVerifier, ApprovalError,
    ApprovalPrincipalV1, ApprovalRequestDraftV1, ApprovalRequestV1, ApprovalResponseCandidateV1,
    ApprovalResponseDecisionV1, ApprovalResponseError, ApprovalResponseFactsV1,
    ApprovalRevocationAuthenticationVerifier, ApprovalRevocationCandidateV1,
    ApprovalRevocationFactsV1, ApprovalScopeV1, AuthenticatedApprovalResponseV1,
    AuthenticatedApprovalRevocationV1, SafeApprovalSummaryV1,
};

/// Maximum number of active policy rules, ratified in ADR-0040.
pub const MAX_POLICY_RULES: usize = 512;

/// Host-derived automation posture. This enum is intentionally not serialized.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AutomationContext {
    /// User-directed or delegated host work.
    Interactive,
    /// Work created by a durable schedule.
    Scheduled,
    /// Read-only proactive watcher work.
    Proactive,
    /// Host maintenance work with narrower authority.
    System,
}

/// Host-observed device registry posture carried as evidence, not queried by rules.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DeviceTrustState {
    /// The device has a durable pairing record.
    pub paired: bool,
    /// The paired device is currently trusted for elevated confirmation.
    pub trusted: bool,
    /// The device is reachable at the snapshot instant.
    pub online: bool,
}

impl AutomationContext {
    /// Derives context only from durable Task kind and trusted request provenance.
    pub fn derive(kind: TaskKind, requested_by: RequestedBy) -> Result<Self, PolicyError> {
        use RequestedBy as R;
        use TaskKind as T;
        match (kind, requested_by) {
            (T::Proactive, _) | (_, R::ProactiveWatcher) => Ok(Self::Proactive),
            (T::Maintenance, _) => Ok(Self::System),
            (T::Scheduled, _) => Ok(Self::Scheduled),
            (T::UserRequest | T::DelegatedHostGoal, R::User | R::Model | R::Scheduler) => {
                Ok(Self::Interactive)
            }
            (T::UserRequest | T::DelegatedHostGoal, R::System) => {
                Err(PolicyError::InvalidContextCombination)
            }
        }
    }

    fn wire_name(self) -> &'static str {
        match self {
            Self::Interactive => "INTERACTIVE",
            Self::Scheduled => "SCHEDULED",
            Self::Proactive => "PROACTIVE",
            Self::System => "SYSTEM",
        }
    }
}

/// Stable reasons for a policy denial. These are codes, never user-authored prose.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DenyReason {
    /// The Task's immutable risk ceiling was exceeded.
    TaskPolicyCeilingExceeded,
    /// A proactive action is outside its read-only risk classes.
    AutomatedActionForbidden,
    /// The capability has been disabled in the trusted registry overlay.
    CapabilityDisabled,
    /// A matching durable policy rule denied the action.
    PolicyRuleDenied,
    /// A SYSTEM context attempted an effecting action.
    SystemEffectingActionForbidden,
    /// A SYSTEM context attempted a destructive action.
    SystemDestructiveActionForbidden,
    /// The evaluation input and immutable revision do not identify the same snapshot.
    StalePolicyRevision,
    /// The Task is cancelled or terminal when policy was evaluated.
    TaskNotLive,
}

impl DenyReason {
    /// The stable machine-readable reason code.
    pub const fn code(self) -> &'static str {
        match self {
            Self::TaskPolicyCeilingExceeded => "TASK_POLICY_CEILING_EXCEEDED",
            Self::AutomatedActionForbidden => "AUTOMATED_ACTION_FORBIDDEN",
            Self::CapabilityDisabled => "CAPABILITY_DISABLED",
            Self::PolicyRuleDenied => "POLICY_RULE_DENIED",
            Self::SystemEffectingActionForbidden => "SYSTEM_EFFECTING_ACTION_FORBIDDEN",
            Self::SystemDestructiveActionForbidden => "SYSTEM_DESTRUCTIVE_ACTION_FORBIDDEN",
            Self::StalePolicyRevision => "STALE_POLICY_REVISION",
            Self::TaskNotLive => "TASK_NOT_LIVE",
        }
    }
}

/// Why an action is held for ordinary authenticated approval.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ApprovalReason {
    /// The matched policy/default requires a human decision.
    PolicyRequiresApproval,
    /// The prepared capability can use root privileges.
    RootRequiresApproval,
    /// Destructive capability use is always approval-gated.
    DestructiveRequiresApproval,
    /// Descriptor authorization requires a human-bound grant.
    CapabilityRequiresApproval,
}

/// A decision about policy state, never a dispatch or execution permission.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PolicyDecision {
    /// No P6 policy or approval hold applies.
    Allow,
    /// An authenticated human approval is required.
    RequireApproval(ApprovalReason),
    /// A separate human credential channel is required.
    RequireHandoff,
    /// The action is refused under a stable machine code.
    Deny(DenyReason),
}

/// A point-in-time record of policy facts and its outcome.
///
/// This type carries no `approved` flag, `RequestId`, dispatch intent, or method that can
/// authorize execution. A consumer must re-evaluate current Task, registry, policy and
/// approval state before any later action.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthorizationEvidenceV1 {
    facts: PolicyInputDraft,
    policy_revision_id: i64,
    policy_rules_digest: Digest,
    decision: PolicyDecision,
}

impl AuthorizationEvidenceV1 {
    /// Task identity observed by this evaluation.
    pub fn task_id(&self) -> &TaskId {
        &self.facts.task_id
    }
    /// Step identity observed by this evaluation.
    pub fn step_id(&self) -> &StepId {
        &self.facts.step_id
    }
    /// Canonical argument digest observed by this evaluation.
    pub fn arguments_digest(&self) -> &Digest {
        &self.facts.arguments_digest
    }
    /// Full trusted, raw-argument-free input facts observed by this evaluation.
    pub fn input_facts(&self) -> &PolicyInputDraft {
        &self.facts
    }
    /// Policy revision identity observed by this evaluation.
    pub const fn policy_revision_id(&self) -> i64 {
        self.policy_revision_id
    }
    /// Policy snapshot digest observed by this evaluation.
    pub fn policy_rules_digest(&self) -> &Digest {
        &self.policy_rules_digest
    }
    /// Policy outcome recorded at this point in time; it is not an execution permit.
    pub const fn decision(&self) -> PolicyDecision {
        self.decision
    }
}

/// A policy result paired with its non-authorizing point-in-time evidence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PolicyEvaluation {
    /// Deterministic policy decision.
    pub decision: PolicyDecision,
    /// Facts and snapshot identity used to produce the decision.
    pub evidence: AuthorizationEvidenceV1,
}

/// Policy result selected by a matching rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RuleDecision {
    /// Selects the normal allow outcome, subject to hard gates.
    Allow,
    /// Refuses the action regardless of allow priority.
    Deny(DenyReason),
    /// Requires an ordinary human approval.
    RequireApproval,
    /// Requires the separate credential handoff channel.
    RequireHandoff,
}

/// Closed exact-value rule dimensions. An absent field matches every value.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RuleMatch {
    /// Exact capability identifier, if constrained.
    pub capability_id: Option<CapabilityId>,
    /// Exact risk class, if constrained.
    pub risk_class: Option<RiskClass>,
    /// Exact side-effect class, if constrained.
    pub side_effect_class: Option<SideEffectClass>,
    /// Exact authorization requirement, if constrained.
    pub authorization: Option<Authorization>,
    /// Exact host-derived context, if constrained.
    pub automation_context: Option<AutomationContext>,
    /// Exact host-owned provenance, if constrained.
    pub requested_by: Option<RequestedBy>,
    /// Exact registry enabled state, if constrained.
    pub enabled: Option<bool>,
}

/// One immutable policy rule.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PolicyRule {
    rule_id: String,
    priority: i32,
    enabled: bool,
    matcher: RuleMatch,
    decision: RuleDecision,
    reason_code: ReasonCode,
}

impl PolicyRule {
    /// Validates a rule identifier against the durable schema's closed grammar.
    pub fn new(
        rule_id: impl Into<String>,
        priority: i32,
        enabled: bool,
        matcher: RuleMatch,
        decision: RuleDecision,
    ) -> Result<Self, PolicyError> {
        let rule_id = rule_id.into();
        if rule_id.is_empty()
            || rule_id.len() > 64
            || !rule_id.as_bytes()[0].is_ascii_alphabetic()
            || !rule_id
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'_')
        {
            return Err(PolicyError::InvalidRuleId);
        }
        let reason_code = match decision {
            RuleDecision::Deny(reason) => ReasonCode::new(reason.code()),
            _ => ReasonCode::new("POLICY_RULE"),
        }
        .map_err(|_| PolicyError::Canonicalization)?;
        Ok(Self {
            rule_id,
            priority,
            enabled,
            matcher,
            decision,
            reason_code,
        })
    }

    /// Stable identifier within one policy revision.
    pub fn rule_id(&self) -> &str {
        &self.rule_id
    }
    /// Priority used before the rule identifier tiebreak.
    pub const fn priority(&self) -> i32 {
        self.priority
    }
    /// Whether this rule participates in evaluation.
    pub const fn enabled(&self) -> bool {
        self.enabled
    }
    /// Closed match values.
    pub fn matcher(&self) -> &RuleMatch {
        &self.matcher
    }
    /// The rule's typed result.
    pub const fn decision(&self) -> RuleDecision {
        self.decision
    }
    /// Stable machine-readable rule reason code.
    pub fn reason_code(&self) -> &ReasonCode {
        &self.reason_code
    }
}

/// One immutable revision and its verified canonical digest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PolicyRevision {
    revision_id: i64,
    rules_digest: Digest,
    rules: Vec<PolicyRule>,
}

/// Complete but not yet active revision, returned only by the trusted import writer.
///
/// It cannot be used to construct a `PolicyInputV1` or evaluate an action. The runtime
/// `PolicyRevision` accepted by the evaluator is available only after the SQLite activation
/// transaction commits or from the durable active pointer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedPolicyRevision {
    revision: PolicyRevision,
}

impl PreparedPolicyRevision {
    /// Monotonic identity reserved for this not-yet-active revision.
    pub const fn revision_id(&self) -> i64 {
        self.revision.revision_id
    }

    /// Digest of the complete prepared immutable rule set.
    pub fn rules_digest(&self) -> &Digest {
        &self.revision.rules_digest
    }
}

impl PolicyRevision {
    /// Creates a revision snapshot and computes its rules digest.
    #[cfg(test)]
    fn new(revision_id: i64, rules: Vec<PolicyRule>) -> Result<Self, PolicyError> {
        if revision_id <= 0 {
            return Err(PolicyError::InvalidRevision);
        }
        validate_rules(&rules)?;
        let rules_digest = digest_rules(&rules)?;
        Ok(Self {
            revision_id,
            rules_digest,
            rules: normalized_rules(rules),
        })
    }

    /// Loads a persisted snapshot only when its digest matches the full rule set.
    pub(crate) fn from_snapshot(
        revision_id: i64,
        expected_digest: Digest,
        rules: Vec<PolicyRule>,
    ) -> Result<Self, PolicyError> {
        if revision_id <= 0 {
            return Err(PolicyError::InvalidRevision);
        }
        validate_rules(&rules)?;
        let actual = digest_rules(&rules)?;
        if actual != expected_digest {
            return Err(PolicyError::RevisionDigestMismatch);
        }
        Ok(Self {
            revision_id,
            rules_digest: actual,
            rules: normalized_rules(rules),
        })
    }

    /// Strictly increasing durable revision identity.
    pub const fn revision_id(&self) -> i64 {
        self.revision_id
    }
    /// SHA-256 over the canonical normalized rule array.
    pub fn rules_digest(&self) -> &Digest {
        &self.rules_digest
    }
    /// Immutable normalized rules in canonical rule-id order.
    pub fn rules(&self) -> &[PolicyRule] {
        &self.rules
    }

    /// Reconstructs an immutable policy snapshot from the typed SQLite row set and verifies
    /// its full canonical digest before it may be evaluated.
    fn from_storage(snapshot: &PolicyRevisionSnapshot) -> Result<Self, PolicyError> {
        if snapshot.activated_at.is_none() {
            return Err(PolicyError::InvalidRevision);
        }
        Self::from_unactivated_storage(snapshot)
    }

    fn from_unactivated_storage(snapshot: &PolicyRevisionSnapshot) -> Result<Self, PolicyError> {
        let rules = snapshot
            .rules
            .iter()
            .map(rule_from_storage)
            .collect::<Result<Vec<_>, _>>()?;
        Self::from_snapshot(snapshot.revision_id, snapshot.rules_digest.clone(), rules)
    }
}

/// Persists a complete unactivated revision through Storage's typed authority port.
pub fn prepare_revision(
    store: &Store,
    rules: Vec<PolicyRule>,
    created_at: EpochMillis,
    actor_class: PolicyActorClass,
    actor_id: ActorId,
    reason_code: ReasonCode,
) -> Result<PreparedPolicyRevision, PolicyError> {
    validate_rules(&rules)?;
    let rules_digest = digest_rules(&rules)?;
    let durable_rules = rules.iter().map(rule_to_storage).collect();
    let draft = PolicyRevisionDraft {
        rules_digest,
        created_at,
        actor_class,
        actor_id,
        reason_code,
        rules: durable_rules,
    };
    let revision_id = store
        .transact(|tx| tx.prepare_policy_revision(draft))
        .map_err(PolicyError::Storage)?;
    let snapshot = store
        .get_policy_revision(revision_id)
        .map_err(PolicyError::Storage)?
        .ok_or(PolicyError::InvalidRevision)?;
    Ok(PreparedPolicyRevision {
        revision: PolicyRevision::from_unactivated_storage(&snapshot)?,
    })
}

/// Loads and verifies the SQLite active revision; an absent pointer remains absent.
pub fn load_active_revision(store: &Store) -> Result<Option<PolicyRevision>, PolicyError> {
    store
        .current_policy_revision()
        .map_err(PolicyError::Storage)?
        .as_ref()
        .map(PolicyRevision::from_storage)
        .transpose()
}

/// Activates a previously prepared revision and its `POLICY_CHANGED` event atomically.
///
/// The Event Bus owns event identity and retention. Storage checks that the event actor,
/// revision, digest, reason and previous pointer match the durable snapshot, and it rolls back
/// the activation savepoint if event append fails. This operation creates no execution token.
pub fn activate_revision(
    store: &Store,
    events: &EventBus,
    prepared: &PreparedPolicyRevision,
    occurred_at: EpochMillis,
    actor_version: SemVer,
) -> Result<PolicyRevision, PolicyError> {
    let snapshot = store
        .transact(|tx| {
            let durable = tx
                .get_policy_revision(prepared.revision.revision_id)?
                .ok_or(StoreError::InvalidPolicyRevision)?;
            let checked = PolicyRevision::from_unactivated_storage(&durable)
                .map_err(|_| StoreError::InvalidPolicyRevision)?;
            if checked != prepared.revision || durable.activated_at.is_some() {
                return Err(StoreError::InvalidPolicyRevision);
            }
            let previous = tx
                .current_policy_revision()?
                .map(|current| current.revision_id);
            let actor_kind = match durable.actor_class {
                PolicyActorClass::Host => serea_protocol::ActorKind::Host,
                PolicyActorClass::User => serea_protocol::ActorKind::User,
            };
            let event = events.draft_policy_changed(PolicyChangedEventV1 {
                revision_id: durable.revision_id,
                previous_revision_id: previous,
                rules_digest: durable.rules_digest.clone(),
                actor_kind,
                actor_id: durable.actor_id.clone(),
                actor_version: actor_version.clone(),
                reason_code: durable.reason_code.clone(),
                occurred_at,
            })?;
            tx.activate_policy_revision(durable.revision_id, occurred_at, event)
        })
        .map_err(PolicyError::Storage)?;
    PolicyRevision::from_storage(&snapshot)
}

fn rule_to_storage(rule: &PolicyRule) -> PolicyRuleDraft {
    PolicyRuleDraft {
        rule_id: rule.rule_id.clone(),
        priority: rule.priority,
        enabled: rule.enabled,
        matcher: PolicyRuleMatchDraft {
            capability_id: rule.matcher.capability_id.clone(),
            risk_class: rule.matcher.risk_class,
            side_effect_class: rule.matcher.side_effect_class,
            authorization: rule.matcher.authorization,
            automation_context: rule.matcher.automation_context.map(context_to_storage),
            requested_by: rule.matcher.requested_by,
            enabled: rule.matcher.enabled,
        },
        decision: match rule.decision {
            RuleDecision::Allow => PolicyDecisionKind::Allow,
            RuleDecision::Deny(_) => PolicyDecisionKind::Deny,
            RuleDecision::RequireApproval => PolicyDecisionKind::RequireApproval,
            RuleDecision::RequireHandoff => PolicyDecisionKind::RequireHandoff,
        },
        reason_code: rule.reason_code.clone(),
    }
}

fn rule_from_storage(rule: &PolicyRuleDraft) -> Result<PolicyRule, PolicyError> {
    let decision = match rule.decision {
        PolicyDecisionKind::Allow => RuleDecision::Allow,
        PolicyDecisionKind::Deny => {
            RuleDecision::Deny(deny_reason_from_code(rule.reason_code.as_str())?)
        }
        PolicyDecisionKind::RequireApproval => RuleDecision::RequireApproval,
        PolicyDecisionKind::RequireHandoff => RuleDecision::RequireHandoff,
    };
    let matcher = RuleMatch {
        capability_id: rule.matcher.capability_id.clone(),
        risk_class: rule.matcher.risk_class,
        side_effect_class: rule.matcher.side_effect_class,
        authorization: rule.matcher.authorization,
        automation_context: rule.matcher.automation_context.map(context_from_storage),
        requested_by: rule.matcher.requested_by,
        enabled: rule.matcher.enabled,
    };
    let output = PolicyRule {
        rule_id: rule.rule_id.clone(),
        priority: rule.priority,
        enabled: rule.enabled,
        matcher,
        decision,
        reason_code: rule.reason_code.clone(),
    };
    if PolicyRule::new(
        output.rule_id.clone(),
        output.priority,
        output.enabled,
        output.matcher.clone(),
        output.decision,
    )?
    .reason_code
        != output.reason_code
    {
        return Err(PolicyError::InvalidRuleReason);
    }
    Ok(output)
}

fn deny_reason_from_code(code: &str) -> Result<DenyReason, PolicyError> {
    match code {
        "TASK_POLICY_CEILING_EXCEEDED" => Ok(DenyReason::TaskPolicyCeilingExceeded),
        "AUTOMATED_ACTION_FORBIDDEN" => Ok(DenyReason::AutomatedActionForbidden),
        "CAPABILITY_DISABLED" => Ok(DenyReason::CapabilityDisabled),
        "POLICY_RULE_DENIED" => Ok(DenyReason::PolicyRuleDenied),
        "SYSTEM_EFFECTING_ACTION_FORBIDDEN" => Ok(DenyReason::SystemEffectingActionForbidden),
        "SYSTEM_DESTRUCTIVE_ACTION_FORBIDDEN" => Ok(DenyReason::SystemDestructiveActionForbidden),
        "STALE_POLICY_REVISION" => Ok(DenyReason::StalePolicyRevision),
        "TASK_NOT_LIVE" => Ok(DenyReason::TaskNotLive),
        _ => Err(PolicyError::InvalidRuleReason),
    }
}

fn context_to_storage(context: AutomationContext) -> PolicyAutomationContext {
    match context {
        AutomationContext::Interactive => PolicyAutomationContext::Interactive,
        AutomationContext::Scheduled => PolicyAutomationContext::Scheduled,
        AutomationContext::Proactive => PolicyAutomationContext::Proactive,
        AutomationContext::System => PolicyAutomationContext::System,
    }
}

fn context_from_storage(context: PolicyAutomationContext) -> AutomationContext {
    match context {
        PolicyAutomationContext::Interactive => AutomationContext::Interactive,
        PolicyAutomationContext::Scheduled => AutomationContext::Scheduled,
        PolicyAutomationContext::Proactive => AutomationContext::Proactive,
        PolicyAutomationContext::System => AutomationContext::System,
    }
}

fn validate_rules(rules: &[PolicyRule]) -> Result<(), PolicyError> {
    if rules.len() > MAX_POLICY_RULES {
        return Err(PolicyError::RuleBoundExceeded);
    }
    let mut ids = BTreeSet::new();
    if rules.iter().any(|rule| !ids.insert(rule.rule_id.as_str())) {
        return Err(PolicyError::DuplicateRuleId);
    }
    if rules.iter().any(|rule| {
        rule.decision == RuleDecision::RequireHandoff
            && rule.matcher.risk_class != Some(RiskClass::Credential)
            && rule.matcher.authorization != Some(Authorization::CredentialHandoff)
    }) {
        return Err(PolicyError::InvalidHandoffRule);
    }
    if rules.iter().any(|rule| {
        let expected = match rule.decision {
            RuleDecision::Deny(reason) => reason.code(),
            _ => "POLICY_RULE",
        };
        rule.reason_code.as_str() != expected
    }) {
        return Err(PolicyError::InvalidRuleReason);
    }
    Ok(())
}

fn normalized_rules(mut rules: Vec<PolicyRule>) -> Vec<PolicyRule> {
    rules.sort_by(|a, b| a.rule_id.cmp(&b.rule_id));
    rules
}

fn digest_rules(rules: &[PolicyRule]) -> Result<Digest, PolicyError> {
    let normalized = normalized_rules(rules.to_vec());
    let value = Value::Array(normalized.iter().map(rule_value).collect());
    let text = serde_json::to_string(&value).map_err(|_| PolicyError::Canonicalization)?;
    let canonical = canonicalize(&text).map_err(|_| PolicyError::Canonicalization)?;
    let mut hex = String::with_capacity(64);
    for byte in Sha256::digest(canonical) {
        use fmt::Write as _;
        write!(&mut hex, "{byte:02x}").map_err(|_| PolicyError::Canonicalization)?;
    }
    Digest::new(format!("sha256:{hex}")).map_err(|_| PolicyError::Canonicalization)
}

fn rule_value(rule: &PolicyRule) -> Value {
    let m = &rule.matcher;
    json!({
        "rule_id": rule.rule_id,
        "priority": rule.priority,
        "enabled": rule.enabled,
        "match": {
            "capability_id": m.capability_id.as_ref().map(ToString::to_string),
            "risk_class": m.risk_class.map(|x| x.wire_name()),
            "side_effect_class": m.side_effect_class.map(|x| x.wire_name()),
            "authorization": m.authorization.map(|x| x.wire_name()),
            "automation_context": m.automation_context.map(AutomationContext::wire_name),
            "requested_by": m.requested_by.map(|x| x.wire_name()),
            "enabled": m.enabled
        },
        "decision": decision_name(rule.decision),
        "reason_code": rule.reason_code.as_str()
    })
}

fn decision_name(decision: RuleDecision) -> &'static str {
    match decision {
        RuleDecision::Allow => "ALLOW",
        RuleDecision::Deny(_) => "DENY",
        RuleDecision::RequireApproval => "REQUIRE_APPROVAL",
        RuleDecision::RequireHandoff => "REQUIRE_HANDOFF",
    }
}

/// Host-owned facts needed for one deterministic evaluation.
///
/// This input has no serde implementation and has no field for raw arguments, model
/// authority claims, current time, or a caller-supplied automation context.
/// Authority-looking model JSON therefore cannot construct this input:
///
/// ```compile_fail
/// use serea_policy::PolicyInputV1;
/// let _: PolicyInputV1 = serde_json::from_str(r#"{"risk_class":"CREDENTIAL"}"#).unwrap();
/// ```
///
/// A prepared-but-unactivated snapshot is a different type and cannot be passed to evaluation:
///
/// ```compile_fail
/// use serea_policy::{PolicyInputDraft, PolicyInputV1, PreparedPolicyRevision};
/// let draft: PolicyInputDraft = todo!();
/// let prepared: PreparedPolicyRevision = todo!();
/// let _ = PolicyInputV1::new(draft, &prepared);
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PolicyInputDraft {
    /// Durable Task identity.
    pub task_id: TaskId,
    /// Durable Step identity.
    pub step_id: StepId,
    /// Durable Task kind.
    pub task_kind: TaskKind,
    /// Current durable Task state.
    pub task_state: TaskState,
    /// Whether cancellation is durably recorded for the Task.
    pub task_cancelled: bool,
    /// Durable Task deadline, if any. Evaluation does not read the clock.
    pub task_deadline_at: Option<EpochMillis>,
    /// Durable Task data classification ceiling.
    pub task_data_class: DataClass,
    /// Immutable Task risk ceiling.
    pub task_policy_class: RiskClass,
    /// Host-resolved capability identity.
    pub capability_id: CapabilityId,
    /// Pinned descriptor version.
    pub capability_version: SemVer,
    /// Provider name carried as pinned transparency identity only.
    pub provider_id: ProviderId,
    /// Optional implementation name carried as pinned transparency identity only.
    pub implementation_id: Option<ImplementationId>,
    /// Host-reviewed capability risk.
    pub risk_class: RiskClass,
    /// Host-reviewed effect classification.
    pub side_effect_class: SideEffectClass,
    /// Host-resolved authorization requirement.
    pub authorization: Authorization,
    /// Host-resolved root posture.
    pub root_requirement: RootRequirement,
    /// Classified action data; never a rule match dimension.
    pub data_class: DataClass,
    /// Host-owned action provenance.
    pub requested_by: RequestedBy,
    /// Device trust facts snapshotted by the host, when known.
    pub device_state: Option<DeviceTrustState>,
    /// Current registry overlay state for this capability.
    pub capability_enabled: bool,
    /// Whether the user explicitly enabled the destructive capability.
    pub destructive_enabled: bool,
    /// Exact durable Task plan revision.
    pub plan_revision: i64,
    /// Exact pinned registry generation.
    pub registry_generation: i64,
    /// Digest of the pinned registry generation manifest.
    pub generation_digest: Digest,
    /// Pinned capability descriptor digest.
    pub descriptor_digest: Digest,
    /// Exact canonical argument digest; no raw arguments are retained here.
    pub arguments_digest: Digest,
}

/// Validated immutable policy evaluation input.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PolicyInputV1 {
    draft: PolicyInputDraft,
    automation_context: AutomationContext,
    policy_revision_id: i64,
    policy_rules_digest: Digest,
}

impl PolicyInputV1 {
    /// Validates durable identity facts and derives the trusted automation context.
    pub fn new(draft: PolicyInputDraft, revision: &PolicyRevision) -> Result<Self, PolicyError> {
        if draft.plan_revision <= 0 || draft.registry_generation <= 0 || revision.revision_id <= 0 {
            return Err(PolicyError::InvalidActionIdentity);
        }
        if draft.data_class == DataClass::Credential && draft.risk_class != RiskClass::Credential {
            return Err(PolicyError::ClassificationMismatch);
        }
        if draft.data_class.rank() > draft.task_data_class.rank() {
            return Err(PolicyError::TaskDataClassExceeded);
        }
        if draft
            .device_state
            .is_some_and(|state| !state.paired && (state.trusted || state.online))
        {
            return Err(PolicyError::InvalidDeviceState);
        }
        let automation_context = AutomationContext::derive(draft.task_kind, draft.requested_by)?;
        Ok(Self {
            draft,
            automation_context,
            policy_revision_id: revision.revision_id,
            policy_rules_digest: revision.rules_digest.clone(),
        })
    }

    /// The derived context; the caller cannot supply it.
    pub const fn automation_context(&self) -> AutomationContext {
        self.automation_context
    }
    /// Immutable input facts, excluding any raw arguments.
    pub fn facts(&self) -> &PolicyInputDraft {
        &self.draft
    }
    /// Exact immutable policy revision used by this evaluation input.
    pub const fn policy_revision_id(&self) -> i64 {
        self.policy_revision_id
    }
    /// Exact policy rules digest named by this evaluation input.
    pub fn policy_rules_digest(&self) -> &Digest {
        &self.policy_rules_digest
    }
}

impl From<PolicyInputV1> for PolicyInputDraft {
    fn from(input: PolicyInputV1) -> Self {
        input.draft
    }
}

/// Typed, payload-free error when trusted policy input is invalid.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PolicyError {
    /// The Task kind and provenance combination is prohibited.
    InvalidContextCombination,
    /// The exact action identity contains a nonpositive revision or generation.
    InvalidActionIdentity,
    /// A rule identifier is malformed.
    InvalidRuleId,
    /// A revision contains duplicate rule identifiers.
    DuplicateRuleId,
    /// The revision exceeds the ratified 512-rule bound.
    RuleBoundExceeded,
    /// A persisted revision digest does not match its rule snapshot.
    RevisionDigestMismatch,
    /// A revision identifier is not positive.
    InvalidRevision,
    /// Canonical encoding or digest construction failed.
    Canonicalization,
    /// A handoff rule is not constrained to a credential action.
    InvalidHandoffRule,
    /// A rule's durable reason code does not match its closed decision mapping.
    InvalidRuleReason,
    /// The storage operation failed; diagnostics remain payload-free.
    Storage(StoreError),
    /// Credential-classified action data does not carry the credential risk class.
    ClassificationMismatch,
    /// Device trust facts cannot be trusted without a durable pairing record.
    InvalidDeviceState,
    /// Prepared action data exceeds the durable Task's declared data-class ceiling.
    TaskDataClassExceeded,
}

impl fmt::Display for PolicyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::InvalidContextCombination => {
                "task kind and requested-by provenance are inconsistent"
            }
            Self::InvalidActionIdentity => {
                "action identity contains an invalid revision or generation"
            }
            Self::InvalidRuleId => "policy rule identifier is malformed",
            Self::DuplicateRuleId => "policy revision contains a duplicate rule identifier",
            Self::RuleBoundExceeded => "policy revision exceeds the active rule bound",
            Self::RevisionDigestMismatch => {
                "policy revision digest does not match its rule snapshot"
            }
            Self::InvalidRevision => "policy revision identifier must be positive",
            Self::Canonicalization => "policy rule snapshot could not be canonically digested",
            Self::InvalidHandoffRule => {
                "credential handoff rule must match credential authorization or risk"
            }
            Self::InvalidRuleReason => "policy rule reason code does not match its decision",
            Self::Storage(_) => "policy storage operation failed",
            Self::ClassificationMismatch => "credential-classified action must use credential risk",
            Self::InvalidDeviceState => "device trust facts are inconsistent with pairing state",
            Self::TaskDataClassExceeded => "action data exceeds the task data-class ceiling",
        })
    }
}
impl std::error::Error for PolicyError {}

/// Evaluates one action against one already-verified immutable revision.
pub fn evaluate(input: &PolicyInputV1, revision: &PolicyRevision) -> PolicyDecision {
    if input.policy_revision_id != revision.revision_id
        || input.policy_rules_digest != revision.rules_digest
    {
        return PolicyDecision::Deny(DenyReason::StalePolicyRevision);
    }
    let facts = &input.draft;
    if facts.task_cancelled || facts.task_state.is_terminal() {
        return PolicyDecision::Deny(DenyReason::TaskNotLive);
    }
    if facts.risk_class.exceeds(facts.task_policy_class) {
        return PolicyDecision::Deny(DenyReason::TaskPolicyCeilingExceeded);
    }
    if input.automation_context == AutomationContext::Proactive
        && !matches!(facts.risk_class, RiskClass::Observe | RiskClass::LocalState)
    {
        return PolicyDecision::Deny(DenyReason::AutomatedActionForbidden);
    }
    if input.automation_context == AutomationContext::System {
        if facts.risk_class == RiskClass::Destructive {
            return PolicyDecision::Deny(DenyReason::SystemDestructiveActionForbidden);
        }
        if facts.side_effect_class != SideEffectClass::None
            && facts.authorization != Authorization::CredentialHandoff
        {
            return PolicyDecision::Deny(DenyReason::SystemEffectingActionForbidden);
        }
    }
    if !facts.capability_enabled {
        return PolicyDecision::Deny(DenyReason::CapabilityDisabled);
    }
    if facts.risk_class == RiskClass::Destructive && !facts.destructive_enabled {
        return PolicyDecision::Deny(DenyReason::CapabilityDisabled);
    }

    let mut matching = revision
        .rules
        .iter()
        .filter(|rule| rule.enabled && matches_rule(rule, input))
        .collect::<Vec<_>>();
    matching.sort_by(|a, b| {
        b.priority
            .cmp(&a.priority)
            .then_with(|| a.rule_id.cmp(&b.rule_id))
    });

    if let Some(denied) = matching
        .iter()
        .find(|rule| matches!(rule.decision, RuleDecision::Deny(_)))
    {
        if let RuleDecision::Deny(reason) = denied.decision {
            return PolicyDecision::Deny(reason);
        }
    }

    if facts.authorization == Authorization::CredentialHandoff
        || facts.risk_class == RiskClass::Credential
    {
        return PolicyDecision::RequireHandoff;
    }
    if facts.root_requirement != RootRequirement::NotRequired {
        return PolicyDecision::RequireApproval(ApprovalReason::RootRequiresApproval);
    }
    if facts.risk_class == RiskClass::Destructive {
        return PolicyDecision::RequireApproval(ApprovalReason::DestructiveRequiresApproval);
    }
    if facts.authorization != Authorization::None {
        return PolicyDecision::RequireApproval(ApprovalReason::CapabilityRequiresApproval);
    }

    if let Some(rule) = matching.first() {
        return match rule.decision {
            RuleDecision::Allow => PolicyDecision::Allow,
            RuleDecision::Deny(reason) => PolicyDecision::Deny(reason),
            RuleDecision::RequireApproval => {
                PolicyDecision::RequireApproval(ApprovalReason::PolicyRequiresApproval)
            }
            RuleDecision::RequireHandoff => PolicyDecision::RequireHandoff,
        };
    }
    default_decision(facts.risk_class, facts.side_effect_class)
}

/// Evaluates an action and returns its decision with non-authorizing evidence.
pub fn evaluate_with_evidence(
    input: &PolicyInputV1,
    revision: &PolicyRevision,
) -> PolicyEvaluation {
    let decision = evaluate(input, revision);
    let facts = &input.draft;
    PolicyEvaluation {
        decision,
        evidence: AuthorizationEvidenceV1 {
            facts: facts.clone(),
            policy_revision_id: input.policy_revision_id,
            policy_rules_digest: input.policy_rules_digest.clone(),
            decision,
        },
    }
}

fn matches_rule(rule: &PolicyRule, input: &PolicyInputV1) -> bool {
    let facts = &input.draft;
    let m = &rule.matcher;
    m.capability_id
        .as_ref()
        .is_none_or(|v| v == &facts.capability_id)
        && m.risk_class.is_none_or(|v| v == facts.risk_class)
        && m.side_effect_class
            .is_none_or(|v| v == facts.side_effect_class)
        && m.authorization.is_none_or(|v| v == facts.authorization)
        && m.automation_context
            .is_none_or(|v| v == input.automation_context)
        && m.requested_by.is_none_or(|v| v == facts.requested_by)
        && m.enabled.is_none_or(|v| v == facts.capability_enabled)
}

fn default_decision(risk: RiskClass, effect: SideEffectClass) -> PolicyDecision {
    match risk {
        RiskClass::Observe | RiskClass::LocalState => PolicyDecision::Allow,
        RiskClass::ReversibleWrite
            if matches!(effect, SideEffectClass::None | SideEffectClass::LocalState) =>
        {
            PolicyDecision::Allow
        }
        RiskClass::Credential => PolicyDecision::RequireHandoff,
        RiskClass::ReversibleWrite
        | RiskClass::ExternalWrite
        | RiskClass::Communication
        | RiskClass::ElevatedDevice
        | RiskClass::Destructive => {
            PolicyDecision::RequireApproval(ApprovalReason::PolicyRequiresApproval)
        }
    }
}

#[cfg(test)]
mod storage_tests;
#[cfg(test)]
mod tests;
