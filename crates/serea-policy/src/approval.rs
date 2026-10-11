//! Approval authority models and deterministic R2 action-set normalization.

use std::collections::BTreeSet;
use std::fmt;

use serde_json::{Value, json};
use serea_protocol::{
    ApprovalId, Digest, EpochMillis, RiskClass, StepId, canonical::canonicalize, digest_of,
};

/// Ratified maximum request lifetime in milliseconds.
pub const MAX_APPROVAL_REQUEST_LIFETIME_MS: i64 = 1_800_000;

/// Host-authored preview with the accepted persisted privacy ceiling.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SafeApprovalSummaryV1 {
    kind: String,
    text: String,
    data_class: serea_protocol::DataClass,
}

/// Typed refusal for a summary that exceeds the accepted persistence contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApprovalError {
    /// Summary builder category is outside the stored grammar.
    InvalidSummaryKind,
    /// Persisted summary or request data exceeds PERSONAL.
    DataClassExceeded,
    /// Request expiry is invalid or exceeds thirty minutes.
    InvalidExpiry,
    /// Task kind and requested-by provenance are inconsistent.
    InvalidProvenance,
    /// Durable plan, registry, or action-set identity is invalid.
    InvalidIdentity,
    /// Credential handoff is a distinct channel, not an ordinary approval.
    CredentialHandoffSeparated,
}

impl fmt::Display for ApprovalError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::InvalidSummaryKind => "approval summary category is malformed",
            Self::DataClassExceeded => "approval summary exceeds the PERSONAL ceiling",
            Self::InvalidExpiry => "approval request expiry exceeds its accepted lifetime",
            Self::InvalidProvenance => "approval request provenance is inconsistent",
            Self::InvalidIdentity => "approval request identity is invalid",
            Self::CredentialHandoffSeparated => {
                "credential handoff cannot be represented as an ordinary approval"
            }
        })
    }
}

impl std::error::Error for ApprovalError {}

impl SafeApprovalSummaryV1 {
    /// Constructs a typed value from a host capability-specific summary builder.
    ///
    /// The constructor accepts no raw arguments. Callers supply only a display
    /// projection already derived from validated, classified action facts.
    pub fn from_host_builder(
        kind: impl Into<String>,
        text: impl Into<String>,
        data_class: serea_protocol::DataClass,
    ) -> Result<Self, ApprovalError> {
        let kind = kind.into();
        if kind.is_empty()
            || kind.len() > 64
            || !kind
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || b"._".contains(&byte))
        {
            return Err(ApprovalError::InvalidSummaryKind);
        }
        if !matches!(
            data_class,
            serea_protocol::DataClass::Public | serea_protocol::DataClass::Personal
        ) {
            return Err(ApprovalError::DataClassExceeded);
        }
        Ok(Self {
            kind,
            text: text.into(),
            data_class,
        })
    }

    /// Stable host summary builder category.
    pub fn kind(&self) -> &str {
        &self.kind
    }

    /// Host-authored summary text. The accepted design introduces no global byte bound.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Persisted summary classification.
    pub const fn data_class(&self) -> serea_protocol::DataClass {
        self.data_class
    }
}

/// Host-trusted input for constructing a durable ApprovalRequest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApprovalRequestDraftV1 {
    /// Host-minted request identity.
    pub approval_id: serea_protocol::ApprovalId,
    /// Durable Task identity.
    pub task_id: serea_protocol::TaskId,
    /// Durable Task kind.
    pub task_kind: serea_protocol::TaskKind,
    /// Exact durable plan revision.
    pub plan_revision: i64,
    /// Pinned capability identity.
    pub capability_id: serea_protocol::CapabilityId,
    /// Pinned capability version.
    pub capability_version: serea_protocol::SemVer,
    /// Pinned capability registry generation.
    pub registry_generation: i64,
    /// Pinned descriptor digest.
    pub descriptor_digest: serea_protocol::Digest,
    /// Host-reviewed capability risk.
    pub risk_class: serea_protocol::RiskClass,
    /// Host-reviewed effect classification.
    pub side_effect_class: serea_protocol::SideEffectClass,
    /// Host-resolved capability authorization requirement.
    pub authorization: serea_protocol::Authorization,
    /// Classification of the action projection.
    pub data_class: serea_protocol::DataClass,
    /// Host-owned task provenance.
    pub requested_by: serea_protocol::RequestedBy,
    /// Exact requested R2 action set.
    pub actions: ApprovalActionSetV1,
    /// Host-authored safe summary.
    pub summary: SafeApprovalSummaryV1,
    /// Trusted request time.
    pub raised_at: serea_protocol::EpochMillis,
    /// Trusted request expiry.
    pub expires_at: serea_protocol::EpochMillis,
}

/// Immutable, validated approval request facts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApprovalRequestV1 {
    draft: ApprovalRequestDraftV1,
    automation_context: crate::AutomationContext,
}

impl ApprovalRequestV1 {
    /// Validates trusted provenance, persistence class and the 30-minute horizon.
    pub fn new(draft: ApprovalRequestDraftV1) -> Result<Self, ApprovalError> {
        if draft.plan_revision <= 0 || draft.registry_generation <= 0 {
            return Err(ApprovalError::InvalidIdentity);
        }
        if !matches!(
            draft.data_class,
            serea_protocol::DataClass::Public | serea_protocol::DataClass::Personal
        ) {
            return Err(ApprovalError::DataClassExceeded);
        }
        if draft.summary.data_class.rank() > draft.data_class.rank() {
            return Err(ApprovalError::DataClassExceeded);
        }
        if draft.risk_class == serea_protocol::RiskClass::Credential
            || draft.authorization == serea_protocol::Authorization::CredentialHandoff
        {
            return Err(ApprovalError::CredentialHandoffSeparated);
        }
        let automation_context =
            crate::AutomationContext::derive(draft.task_kind, draft.requested_by)
                .map_err(|_| ApprovalError::InvalidProvenance)?;
        let duration = draft
            .expires_at
            .get()
            .checked_sub(draft.raised_at.get())
            .ok_or(ApprovalError::InvalidExpiry)?;
        if duration <= 0 || duration > 1_800_000 {
            return Err(ApprovalError::InvalidExpiry);
        }
        Ok(Self {
            draft,
            automation_context,
        })
    }

    /// Validated input facts, including the exact action-set commitment.
    pub fn draft(&self) -> &ApprovalRequestDraftV1 {
        &self.draft
    }

    /// Context derived from durable Task kind and host provenance.
    pub const fn automation_context(&self) -> crate::AutomationContext {
        self.automation_context
    }

    /// Converts validated host facts to Storage's independent digest-checking draft.
    pub fn storage_draft(&self) -> serea_storage::ApprovalRequestStorageDraft {
        use serea_storage::{
            ApprovalAutomationContext as StorageContext, ApprovalRequestActionDraft,
        };
        let context = match self.automation_context {
            crate::AutomationContext::Interactive => StorageContext::Interactive,
            crate::AutomationContext::Scheduled => StorageContext::Scheduled,
            crate::AutomationContext::Proactive => StorageContext::Proactive,
            crate::AutomationContext::System => StorageContext::System,
        };
        serea_storage::ApprovalRequestStorageDraft {
            approval_id: self.draft.approval_id.clone(),
            task_id: self.draft.task_id.clone(),
            task_kind: self.draft.task_kind,
            plan_revision: self.draft.plan_revision,
            capability_id: self.draft.capability_id.clone(),
            capability_version: self.draft.capability_version.clone(),
            generation_id: self.draft.registry_generation,
            descriptor_digest: self.draft.descriptor_digest.clone(),
            risk_class: self.draft.risk_class,
            side_effect_class: self.draft.side_effect_class,
            authorization: self.draft.authorization,
            data_class: self.draft.data_class,
            requested_by: self.draft.requested_by,
            automation_context: context,
            action_set_digest: self.draft.actions.digest().clone(),
            actions: self
                .draft
                .actions
                .actions()
                .iter()
                .map(|action| ApprovalRequestActionDraft {
                    step_id: action.step_id.clone(),
                    arguments_digest: action.arguments_digest.clone(),
                    scope_json: action.scope.canonical_json.clone(),
                    scope_digest: action.scope.digest.clone(),
                })
                .collect(),
            summary_kind: self.draft.summary.kind.clone(),
            summary: self.draft.summary.text.clone(),
            raised_at: self.draft.raised_at,
            expires_at: self.draft.expires_at,
        }
    }
}

/// Authenticated user's response choice; this is an internal typed value, not wire JSON.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApprovalResponseDecisionV1 {
    /// Approve exactly the named subset of the request's enumerated actions.
    Grant,
    /// Refuse every action in the request.
    Deny,
}

/// Authenticated principal class for a human response.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApprovalPrincipalV1 {
    /// Paired user device.
    User,
    /// Host administrator surface.
    Admin,
}

/// Authentication strength established by the trusted host authentication adapter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApprovalAuthenticationStrengthV1 {
    /// Existing authenticated session.
    Session,
    /// Session plus a verified local elevated confirmation.
    ElevatedConfirmed,
}

/// Failure returned when the host cannot authenticate the actor/session.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApprovalAuthenticationError {
    /// Device, principal, or session is stale, unpaired, or revoked.
    StaleActorSession,
    /// Required elevated confirmation was not verified by the host.
    MissingElevatedConfirmation,
    /// Authentication did not bind the supplied confirmation timestamp.
    InvalidConfirmationTime,
}

impl fmt::Display for ApprovalAuthenticationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::StaleActorSession => "approval actor or session is stale",
            Self::MissingElevatedConfirmation => "elevated approval confirmation is missing",
            Self::InvalidConfirmationTime => "approval confirmation time is invalid",
        })
    }
}

impl std::error::Error for ApprovalAuthenticationError {}

/// Typed approval response received from an authenticated, host-verified UI port.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApprovalResponseCandidateV1 {
    approval_id: ApprovalId,
    decision: ApprovalResponseDecisionV1,
    principal: ApprovalPrincipalV1,
    device_id: Option<serea_protocol::DeviceId>,
    session_id: Option<serea_protocol::SessionId>,
    authentication_strength: ApprovalAuthenticationStrengthV1,
    confirmed_at: EpochMillis,
    elevated_confirmation: Option<Digest>,
    approved_step_ids: Vec<StepId>,
    requested_max_uses: Option<u32>,
    requested_expires_at: Option<EpochMillis>,
}

impl ApprovalResponseCandidateV1 {
    /// Forms an internal candidate from the adapter's already parsed fields.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        approval_id: ApprovalId,
        decision: ApprovalResponseDecisionV1,
        principal: ApprovalPrincipalV1,
        device_id: Option<serea_protocol::DeviceId>,
        session_id: Option<serea_protocol::SessionId>,
        authentication_strength: ApprovalAuthenticationStrengthV1,
        confirmed_at: EpochMillis,
        elevated_confirmation: Option<Digest>,
        approved_step_ids: Vec<StepId>,
        requested_max_uses: Option<u32>,
        requested_expires_at: Option<EpochMillis>,
    ) -> Self {
        Self {
            approval_id,
            decision,
            principal,
            device_id,
            session_id,
            authentication_strength,
            confirmed_at,
            elevated_confirmation,
            approved_step_ids,
            requested_max_uses,
            requested_expires_at,
        }
    }
}

/// Host verification port. Production implementations must verify the current
/// paired-device/session or local admin authentication, not trust client claims.
pub trait ApprovalAuthenticationVerifier {
    /// Verifies the candidate's current principal/session and any claimed local confirmation.
    fn verify(
        &self,
        candidate: &ApprovalResponseCandidateV1,
    ) -> Result<(), ApprovalAuthenticationError>;
}

/// Approval response that crossed the trusted host authentication port.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthenticatedApprovalResponseV1 {
    candidate: ApprovalResponseCandidateV1,
}

impl AuthenticatedApprovalResponseV1 {
    /// Calls the trusted verifier and creates an authenticated response only on success.
    pub fn authenticate(
        candidate: ApprovalResponseCandidateV1,
        verifier: &dyn ApprovalAuthenticationVerifier,
    ) -> Result<Self, ApprovalAuthenticationError> {
        verifier.verify(&candidate)?;
        Ok(Self { candidate })
    }

    /// Revalidates response facts against the exact durable request before persistence.
    pub fn validate_for(
        &self,
        request: &ApprovalRequestV1,
        now: EpochMillis,
    ) -> Result<ApprovalResponseFactsV1, ApprovalResponseError> {
        let candidate = &self.candidate;
        if candidate.approval_id != request.draft.approval_id
            || candidate.confirmed_at < request.draft.raised_at
            || candidate.confirmed_at > now
        {
            return Err(ApprovalResponseError::InvalidIdentityOrTime);
        }
        if candidate.principal == ApprovalPrincipalV1::User
            && (candidate.device_id.is_none() || candidate.session_id.is_none())
        {
            return Err(ApprovalResponseError::MissingAuthenticatedSession);
        }
        if candidate.authentication_strength == ApprovalAuthenticationStrengthV1::ElevatedConfirmed
            && candidate.elevated_confirmation.is_none()
        {
            return Err(ApprovalResponseError::MissingElevatedConfirmation);
        }
        if request.draft.risk_class == RiskClass::ElevatedDevice
            && (candidate.authentication_strength
                != ApprovalAuthenticationStrengthV1::ElevatedConfirmed
                || candidate.elevated_confirmation.is_none())
        {
            return Err(ApprovalResponseError::MissingElevatedConfirmation);
        }
        if now >= request.draft.expires_at || candidate.confirmed_at >= request.draft.expires_at {
            return Err(ApprovalResponseError::RequestExpired);
        }
        let approved_actions = match candidate.decision {
            ApprovalResponseDecisionV1::Grant => {
                if candidate.approved_step_ids.is_empty() {
                    return Err(ApprovalResponseError::InvalidDecisionPayload);
                }
                let subset = request
                    .draft
                    .actions
                    .approved_subset(&candidate.approved_step_ids)
                    .map_err(|_| ApprovalResponseError::InvalidActionSubset)?;
                if candidate
                    .requested_max_uses
                    .is_some_and(|uses| usize::try_from(uses).ok() != Some(subset.actions().len()))
                {
                    return Err(ApprovalResponseError::InvalidUseBound);
                }
                subset
            }
            ApprovalResponseDecisionV1::Deny => {
                if !candidate.approved_step_ids.is_empty()
                    || candidate.requested_max_uses.is_some()
                    || candidate.requested_expires_at.is_some()
                {
                    return Err(ApprovalResponseError::InvalidDecisionPayload);
                }
                request
                    .draft
                    .actions
                    .approved_subset(
                        &request
                            .draft
                            .actions
                            .actions()
                            .iter()
                            .map(|action| action.step_id.clone())
                            .collect::<Vec<_>>(),
                    )
                    .map_err(|_| ApprovalResponseError::InvalidActionSubset)?
            }
        };
        let expires_at = match candidate.requested_expires_at {
            Some(value)
                if value <= request.draft.expires_at
                    && value > now
                    && value
                        .get()
                        .checked_sub(candidate.confirmed_at.get())
                        .is_some_and(|delta| delta <= MAX_APPROVAL_REQUEST_LIFETIME_MS) =>
            {
                value
            }
            Some(_) => return Err(ApprovalResponseError::InvalidExpiryBound),
            None => request.draft.expires_at,
        };
        Ok(ApprovalResponseFactsV1 {
            approval_id: candidate.approval_id.clone(),
            decision: candidate.decision,
            principal: candidate.principal,
            device_id: candidate.device_id.clone(),
            session_id: candidate.session_id.clone(),
            authentication_strength: candidate.authentication_strength,
            confirmed_at: candidate.confirmed_at,
            elevated_confirmation: candidate.elevated_confirmation.clone(),
            approved_actions,
            request_action_set_digest: request.draft.actions.digest().clone(),
            checked_at: now,
            requested_max_uses: candidate.requested_max_uses,
            expires_at,
        })
    }
}

/// Typed rejection when a trusted response does not fit the request's authority bounds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApprovalResponseError {
    /// Request identity or authenticated confirmation time does not match.
    InvalidIdentityOrTime,
    /// User response lacks its verified device and session.
    MissingAuthenticatedSession,
    /// Elevated action lacks local device confirmation and digest handle.
    MissingElevatedConfirmation,
    /// Request is expired at or before the checked instant.
    RequestExpired,
    /// Response action ids do not select a valid enumerated subset.
    InvalidActionSubset,
    /// Response use ceiling does not match the enumerated subset.
    InvalidUseBound,
    /// Response expiry exceeds or contradicts the request horizon.
    InvalidExpiryBound,
    /// Denial has approval-only fields or Grant selected no member.
    InvalidDecisionPayload,
}

impl fmt::Display for ApprovalResponseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::InvalidIdentityOrTime => "approval response identity or time is invalid",
            Self::MissingAuthenticatedSession => "approval response has no authenticated session",
            Self::MissingElevatedConfirmation => "elevated approval confirmation is required",
            Self::RequestExpired => "approval request is expired",
            Self::InvalidActionSubset => "approval response selects an invalid action subset",
            Self::InvalidUseBound => "approval max uses does not match the approved action set",
            Self::InvalidExpiryBound => "approval grant expiry exceeds its request bound",
            Self::InvalidDecisionPayload => {
                "approval response contains an invalid decision payload"
            }
        })
    }
}

impl std::error::Error for ApprovalResponseError {}

/// Validated, immutable facts a Storage transaction may persist.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApprovalResponseFactsV1 {
    /// Request identity.
    approval_id: ApprovalId,
    /// Authenticated decision.
    decision: ApprovalResponseDecisionV1,
    /// Authenticated principal class.
    principal: ApprovalPrincipalV1,
    /// Authenticated device, where a paired-device session was used.
    device_id: Option<serea_protocol::DeviceId>,
    /// Authenticated session, where a paired-device session was used.
    session_id: Option<serea_protocol::SessionId>,
    /// Verified authentication strength.
    authentication_strength: ApprovalAuthenticationStrengthV1,
    /// Trusted confirmation instant.
    confirmed_at: EpochMillis,
    /// Digest handle for the elevated confirmation; never biometric material.
    elevated_confirmation: Option<Digest>,
    /// Exact approved subset, or the requested set for DENY routing.
    approved_actions: ApprovalActionSetV1,
    /// Complete action-set commitment read from the immutable Policy request.
    request_action_set_digest: Digest,
    checked_at: EpochMillis,
    /// Requested max uses proposal.
    requested_max_uses: Option<u32>,
    /// Grant expiry after request-horizon validation.
    expires_at: EpochMillis,
}

/// Candidate for the separate grant-revocation authentication port.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApprovalRevocationCandidateV1 {
    approval_id: ApprovalId,
    grant_id: serea_protocol::GrantId,
    principal: ApprovalPrincipalV1,
    actor_id: serea_protocol::ActorId,
    device_id: Option<serea_protocol::DeviceId>,
    session_id: Option<serea_protocol::SessionId>,
    confirmed_at: EpochMillis,
}

impl ApprovalRevocationCandidateV1 {
    /// Forms typed candidate facts from a trusted host-side authentication adapter.
    pub fn new(
        approval_id: ApprovalId,
        grant_id: serea_protocol::GrantId,
        principal: ApprovalPrincipalV1,
        actor_id: serea_protocol::ActorId,
        device_id: Option<serea_protocol::DeviceId>,
        session_id: Option<serea_protocol::SessionId>,
        confirmed_at: EpochMillis,
    ) -> Self {
        Self {
            approval_id,
            grant_id,
            principal,
            actor_id,
            device_id,
            session_id,
            confirmed_at,
        }
    }
}

/// Separate trusted port for authenticating a grant revocation.
pub trait ApprovalRevocationAuthenticationVerifier {
    /// Revalidates the current local-admin or paired-user session.
    fn verify(
        &self,
        candidate: &ApprovalRevocationCandidateV1,
    ) -> Result<(), ApprovalAuthenticationError>;
}

/// Authenticated revocation response; it cannot be expressed as GRANT or DENY.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthenticatedApprovalRevocationV1 {
    candidate: ApprovalRevocationCandidateV1,
}

impl AuthenticatedApprovalRevocationV1 {
    /// Calls the distinct revocation authenticator.
    pub fn authenticate(
        candidate: ApprovalRevocationCandidateV1,
        verifier: &dyn ApprovalRevocationAuthenticationVerifier,
    ) -> Result<Self, ApprovalAuthenticationError> {
        verifier.verify(&candidate)?;
        Ok(Self { candidate })
    }

    /// Revalidates request/grant identity and time before yielding non-forgeable facts.
    pub fn validate_for(
        &self,
        approval_id: &ApprovalId,
        grant_id: &serea_protocol::GrantId,
        now: EpochMillis,
    ) -> Result<ApprovalRevocationFactsV1, ApprovalResponseError> {
        let c = &self.candidate;
        if &c.approval_id != approval_id || &c.grant_id != grant_id || c.confirmed_at > now {
            return Err(ApprovalResponseError::InvalidIdentityOrTime);
        }
        if c.principal == ApprovalPrincipalV1::User
            && (c.device_id.is_none() || c.session_id.is_none())
        {
            return Err(ApprovalResponseError::MissingAuthenticatedSession);
        }
        if c.device_id.is_some() != c.session_id.is_some() {
            return Err(ApprovalResponseError::MissingAuthenticatedSession);
        }
        Ok(ApprovalRevocationFactsV1 {
            approval_id: c.approval_id.clone(),
            grant_id: c.grant_id.clone(),
            principal: c.principal,
            actor_id: c.actor_id.clone(),
            device_id: c.device_id.clone(),
            session_id: c.session_id.clone(),
            revoked_at: c.confirmed_at,
            checked_at: now,
        })
    }
}

/// Private-field evidence accepted by the Storage adapter for one revocation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApprovalRevocationFactsV1 {
    approval_id: ApprovalId,
    grant_id: serea_protocol::GrantId,
    principal: ApprovalPrincipalV1,
    actor_id: serea_protocol::ActorId,
    device_id: Option<serea_protocol::DeviceId>,
    session_id: Option<serea_protocol::SessionId>,
    revoked_at: EpochMillis,
    checked_at: EpochMillis,
}

impl ApprovalRevocationFactsV1 {
    /// Converts only verified revocation facts to the durable Storage boundary.
    pub fn storage_draft(&self) -> serea_storage::ApprovalRevocationStorageDraft {
        serea_storage::ApprovalRevocationStorageDraft {
            approval_id: self.approval_id.clone(),
            grant_id: self.grant_id.clone(),
            actor_id: self.actor_id.clone(),
            actor_class: match self.principal {
                ApprovalPrincipalV1::User => "USER",
                ApprovalPrincipalV1::Admin => "ADMIN",
            },
            device_id: self.device_id.as_ref().map(ToString::to_string),
            session_id: self.session_id.as_ref().map(ToString::to_string),
            revoked_at: self.revoked_at,
            checked_at: self.checked_at,
        }
    }
}

impl ApprovalResponseFactsV1 {
    /// Authenticated decision after request-bound validation.
    pub const fn decision(&self) -> ApprovalResponseDecisionV1 {
        self.decision
    }

    /// Exact validated action set; for denial this is the request set used for routing only.
    pub fn approved_actions(&self) -> &ApprovalActionSetV1 {
        &self.approved_actions
    }

    /// Digest of the immutable full action set against which this response was checked.
    pub fn request_action_set_digest(&self) -> &Digest {
        &self.request_action_set_digest
    }

    /// Converts validated authentication facts into the durable Storage DTO.
    /// Grant identity and event actor identity are host-minted by the caller.
    pub fn storage_draft(
        &self,
        grant_id: Option<serea_protocol::GrantId>,
        actor_id: serea_protocol::ActorId,
    ) -> serea_storage::ApprovalResponseStorageDraft {
        use serea_storage::{ApprovalResponseDecisionStorage as D, ApprovalResponseStorageDraft};
        ApprovalResponseStorageDraft {
            approval_id: self.approval_id.clone(),
            grant_id,
            actor_id,
            decision: match self.decision {
                ApprovalResponseDecisionV1::Grant => D::Grant,
                ApprovalResponseDecisionV1::Deny => D::Deny,
            },
            approved_step_ids: if self.decision == ApprovalResponseDecisionV1::Grant {
                self.approved_actions
                    .actions()
                    .iter()
                    .map(|action| action.step_id.clone())
                    .collect()
            } else {
                Vec::new()
            },
            request_action_set_digest: self.request_action_set_digest.clone(),
            checked_at: self.checked_at,
            decided_at: self.confirmed_at,
            expires_at: (self.decision == ApprovalResponseDecisionV1::Grant)
                .then_some(self.expires_at),
            granted_by: match self.principal {
                ApprovalPrincipalV1::User => "USER",
                ApprovalPrincipalV1::Admin => "ADMIN",
            },
            actor_device_id: self.device_id.as_ref().map(ToString::to_string),
            actor_session_id: self.session_id.as_ref().map(ToString::to_string),
            auth_strength: match self.authentication_strength {
                ApprovalAuthenticationStrengthV1::Session => "SESSION",
                ApprovalAuthenticationStrengthV1::ElevatedConfirmed => "ELEVATED_CONFIRMED",
            },
            elevated_ref: self.elevated_confirmation.as_ref().map(ToString::to_string),
        }
    }
}

/// Payload-free refusal when an R2 action or set violates the closed contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApprovalActionSetError {
    /// The action count is outside the ratified 1–8 range.
    ActionCountOutOfRange,
    /// A StepId appears more than once.
    DuplicateStep,
    /// A requested subset includes a StepId absent from the request.
    UnexpectedStep,
    /// A scope is not a canonical structural object within the stored bound.
    InvalidScope,
    /// Wildcard scope values are unsupported.
    WildcardScope,
    /// SCJ-1 canonicalization or digesting failed.
    Canonicalization,
}

impl fmt::Display for ApprovalActionSetError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::ActionCountOutOfRange => "approval action count is outside 1..=8",
            Self::DuplicateStep => "approval action set repeats a StepId",
            Self::UnexpectedStep => "approval subset contains an unrequested StepId",
            Self::InvalidScope => "approval action scope is not a bounded JSON object",
            Self::WildcardScope => "wildcard approval scopes are unsupported",
            Self::Canonicalization => "approval action set could not be canonically digested",
        })
    }
}

impl std::error::Error for ApprovalActionSetError {}

/// Canonical structural scope shown for exactly one Step.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApprovalScopeV1 {
    canonical_json: String,
    digest: Digest,
}

impl ApprovalScopeV1 {
    /// Validates and canonicalizes a capability-defined object scope.
    pub fn new(value: Value) -> Result<Self, ApprovalActionSetError> {
        if !value.is_object() {
            return Err(ApprovalActionSetError::InvalidScope);
        }
        if has_wildcard(&value) {
            return Err(ApprovalActionSetError::WildcardScope);
        }
        let source =
            serde_json::to_string(&value).map_err(|_| ApprovalActionSetError::Canonicalization)?;
        let canonical =
            canonicalize(&source).map_err(|_| ApprovalActionSetError::Canonicalization)?;
        if !(2..=4096).contains(&canonical.len()) {
            return Err(ApprovalActionSetError::InvalidScope);
        }
        let canonical_json =
            String::from_utf8(canonical).map_err(|_| ApprovalActionSetError::Canonicalization)?;
        let digest =
            digest_of(&canonical_json).map_err(|_| ApprovalActionSetError::Canonicalization)?;
        Ok(Self {
            canonical_json,
            digest,
        })
    }

    /// Canonical `scope` JSON persisted for this action.
    pub fn canonical_json(&self) -> &str {
        &self.canonical_json
    }

    /// SHA-256 of the SCJ-1 canonical structural scope.
    pub fn digest(&self) -> &Digest {
        &self.digest
    }

    /// Decodes the canonical JSON value for digest construction.
    fn value(&self) -> Result<Value, ApprovalActionSetError> {
        serde_json::from_str(&self.canonical_json)
            .map_err(|_| ApprovalActionSetError::Canonicalization)
    }
}

/// One exact, host-validated approval action under owner decision R2.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApprovalActionV1 {
    step_id: StepId,
    arguments_digest: Digest,
    scope: ApprovalScopeV1,
}

impl ApprovalActionV1 {
    /// Binds the Step to its exact durable arguments digest and structural scope.
    pub fn new(
        step_id: StepId,
        arguments_digest: Digest,
        scope: Value,
    ) -> Result<Self, ApprovalActionSetError> {
        Ok(Self {
            step_id,
            arguments_digest,
            scope: ApprovalScopeV1::new(scope)?,
        })
    }

    /// Exact Step membership identity.
    pub fn step_id(&self) -> &StepId {
        &self.step_id
    }

    /// Exact canonical arguments digest shown to the approver.
    pub fn arguments_digest(&self) -> &Digest {
        &self.arguments_digest
    }

    /// Immutable scope shown for this action.
    pub fn scope(&self) -> &ApprovalScopeV1 {
        &self.scope
    }
}

/// Normalized one-to-eight action authority set and its canonical R2 digest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApprovalActionSetV1 {
    actions: Vec<ApprovalActionV1>,
    digest: Digest,
}

impl ApprovalActionSetV1 {
    /// Sorts byte-wise by StepId, rejects duplicates and commits all member facts.
    pub fn new(mut actions: Vec<ApprovalActionV1>) -> Result<Self, ApprovalActionSetError> {
        if !(1..=8).contains(&actions.len()) {
            return Err(ApprovalActionSetError::ActionCountOutOfRange);
        }
        actions.sort_by(|left, right| {
            left.step_id
                .as_str()
                .as_bytes()
                .cmp(right.step_id.as_str().as_bytes())
        });
        let mut seen = BTreeSet::new();
        if actions
            .iter()
            .any(|action| !seen.insert(action.step_id.as_str()))
        {
            return Err(ApprovalActionSetError::DuplicateStep);
        }
        let digest = action_set_digest(&actions)?;
        Ok(Self { actions, digest })
    }

    /// Actions in the exact normalized order committed by `digest()`.
    pub fn actions(&self) -> &[ApprovalActionV1] {
        &self.actions
    }

    /// SHA-256 of SCJ-1 canonical JSON for the ordered action array.
    pub fn digest(&self) -> &Digest {
        &self.digest
    }

    /// Selects an approved subset only by explicit StepId membership.
    pub fn approved_subset(&self, step_ids: &[StepId]) -> Result<Self, ApprovalActionSetError> {
        if !(1..=8).contains(&step_ids.len()) {
            return Err(ApprovalActionSetError::ActionCountOutOfRange);
        }
        let mut unique = BTreeSet::new();
        let mut actions = Vec::with_capacity(step_ids.len());
        for step_id in step_ids {
            if !unique.insert(step_id.as_str()) {
                return Err(ApprovalActionSetError::DuplicateStep);
            }
            let action = self
                .actions
                .iter()
                .find(|action| action.step_id == *step_id)
                .ok_or(ApprovalActionSetError::UnexpectedStep)?;
            actions.push(action.clone());
        }
        Self::new(actions)
    }
}

fn action_set_digest(actions: &[ApprovalActionV1]) -> Result<Digest, ApprovalActionSetError> {
    let values = actions
        .iter()
        .map(|action| {
            Ok(json!({
                "step_id": action.step_id.as_str(),
                "arguments_digest": action.arguments_digest.as_str(),
                "scope": action.scope.value()?,
            }))
        })
        .collect::<Result<Vec<_>, ApprovalActionSetError>>()?;
    let source = serde_json::to_string(&Value::Array(values))
        .map_err(|_| ApprovalActionSetError::Canonicalization)?;
    let canonical = canonicalize(&source).map_err(|_| ApprovalActionSetError::Canonicalization)?;
    let canonical =
        String::from_utf8(canonical).map_err(|_| ApprovalActionSetError::Canonicalization)?;
    digest_of(&canonical).map_err(|_| ApprovalActionSetError::Canonicalization)
}

fn has_wildcard(value: &Value) -> bool {
    match value {
        Value::String(text) => text.contains('*'),
        Value::Array(values) => values.iter().any(has_wildcard),
        Value::Object(values) => values
            .iter()
            .any(|(key, value)| key.contains('*') || has_wildcard(value)),
        Value::Null | Value::Bool(_) | Value::Number(_) => false,
    }
}

#[cfg(test)]
mod tests;
