use std::str::FromStr;

use serde_json::json;
use serea_protocol::{
    ActorId, ApprovalId, Authorization, CapabilityId, DataClass, DeviceId, Digest, EpochMillis,
    GrantId, RequestedBy, RiskClass, SemVer, SessionId, SideEffectClass, StepId, TaskId, TaskKind,
};

use super::{
    ApprovalActionSetError, ApprovalActionSetV1, ApprovalActionV1, ApprovalAuthenticationError,
    ApprovalAuthenticationStrengthV1, ApprovalAuthenticationVerifier, ApprovalError,
    ApprovalPrincipalV1, ApprovalRequestDraftV1, ApprovalRequestV1, ApprovalResponseCandidateV1,
    ApprovalResponseDecisionV1, ApprovalResponseError, ApprovalRevocationAuthenticationVerifier,
    ApprovalRevocationCandidateV1, ApprovalScopeV1, AuthenticatedApprovalResponseV1,
    AuthenticatedApprovalRevocationV1, SafeApprovalSummaryV1,
};

struct AcceptAuth;

impl ApprovalAuthenticationVerifier for AcceptAuth {
    fn verify(&self, _: &ApprovalResponseCandidateV1) -> Result<(), ApprovalAuthenticationError> {
        Ok(())
    }
}

struct StaleAuth;

impl ApprovalAuthenticationVerifier for StaleAuth {
    fn verify(&self, _: &ApprovalResponseCandidateV1) -> Result<(), ApprovalAuthenticationError> {
        Err(ApprovalAuthenticationError::StaleActorSession)
    }
}

struct AcceptRevocation;
impl ApprovalRevocationAuthenticationVerifier for AcceptRevocation {
    fn verify(&self, _: &ApprovalRevocationCandidateV1) -> Result<(), ApprovalAuthenticationError> {
        Ok(())
    }
}

fn step(value: &str) -> StepId {
    StepId::from_str(value).unwrap()
}

fn digest(ch: char) -> Digest {
    Digest::new(format!("sha256:{}", ch.to_string().repeat(64))).unwrap()
}

fn action(id: &str, arg: char, scope: serde_json::Value) -> ApprovalActionV1 {
    ApprovalActionV1::new(step(id), digest(arg), scope, DataClass::Personal).unwrap()
}

#[test]
fn action_set_is_order_independent_and_uses_step_id_byte_order() {
    let first = action(
        "stp_01JQ8ZB8H2XKM9P4QW7NRT5YCD",
        'a',
        json!({"calendar":"primary"}),
    );
    let second = action(
        "stp_01JQ8ZB8H2XKM9P4QW7NRT5YCE",
        'b',
        json!({"calendar":"secondary"}),
    );
    let left = ApprovalActionSetV1::new(vec![second.clone(), first.clone()]).unwrap();
    let right = ApprovalActionSetV1::new(vec![first.clone(), second.clone()]).unwrap();
    assert_eq!(left, right);
    assert_eq!(left.actions()[0].step_id(), first.step_id());
    assert_eq!(left.digest(), right.digest());
}

#[test]
fn action_set_rejects_duplicate_and_unexpected_step_ids() {
    let a = action(
        "stp_01JQ8ZB8H2XKM9P4QW7NRT5YCD",
        'a',
        json!({"calendar":"primary"}),
    );
    assert_eq!(
        ApprovalActionSetV1::new(vec![a.clone(), a.clone()]),
        Err(ApprovalActionSetError::DuplicateStep)
    );
    let set = ApprovalActionSetV1::new(vec![a]).unwrap();
    assert_eq!(
        set.approved_subset(&[step("stp_01JQ8ZB8H2XKM9P4QW7NRT5YCE")]),
        Err(ApprovalActionSetError::UnexpectedStep)
    );
}

#[test]
fn action_set_refuses_empty_oversized_and_wildcard_scope() {
    assert_eq!(
        ApprovalActionSetV1::new(vec![]),
        Err(ApprovalActionSetError::ActionCountOutOfRange)
    );
    let actions = (0..9)
        .map(|index| {
            let body = format!("01JQ8ZB8H2XKM9P4QW7NRT5Y{index:02X}");
            action(&format!("stp_{body}"), 'a', json!({"target":"one"}))
        })
        .collect();
    assert_eq!(
        ApprovalActionSetV1::new(actions),
        Err(ApprovalActionSetError::ActionCountOutOfRange)
    );
    assert_eq!(
        ApprovalActionV1::new(
            step("stp_01JQ8ZB8H2XKM9P4QW7NRT5YCD"),
            digest('a'),
            json!({"target":"*"}),
            DataClass::Personal
        ),
        Err(ApprovalActionSetError::WildcardScope)
    );
}

#[test]
fn protected_scope_classes_are_refused_and_scope_debug_redacts_values() {
    for class in [DataClass::Private, DataClass::Secret, DataClass::Credential] {
        assert_eq!(
            ApprovalActionV1::new(
                step("stp_01JQ8ZB8H2XKM9P4QW7NRT5YCD"),
                digest('a'),
                json!({"target":"credential-sentinel-7c91"}),
                class,
            ),
            Err(ApprovalActionSetError::ScopeClassExceeded)
        );
    }

    let scope = ApprovalScopeV1::new(
        json!({"target":"personal-sentinel-2af3"}),
        DataClass::Personal,
    )
    .unwrap();
    let debug = format!("{scope:?}");
    assert!(!debug.contains("personal-sentinel-2af3"));
    assert!(debug.contains("canonical_json_bytes"));
}

#[test]
fn action_set_commits_arguments_and_scope_exactly() {
    let base = action(
        "stp_01JQ8ZB8H2XKM9P4QW7NRT5YCD",
        'a',
        json!({"calendar":"primary"}),
    );
    let changed_args = action(
        "stp_01JQ8ZB8H2XKM9P4QW7NRT5YCD",
        'b',
        json!({"calendar":"primary"}),
    );
    let changed_scope = action(
        "stp_01JQ8ZB8H2XKM9P4QW7NRT5YCD",
        'a',
        json!({"calendar":"other"}),
    );
    assert_ne!(
        ApprovalActionSetV1::new(vec![base.clone()])
            .unwrap()
            .digest(),
        ApprovalActionSetV1::new(vec![changed_args])
            .unwrap()
            .digest()
    );
    assert_ne!(
        ApprovalActionSetV1::new(vec![base]).unwrap().digest(),
        ApprovalActionSetV1::new(vec![changed_scope])
            .unwrap()
            .digest()
    );
}

fn request_draft(expires_at: i64, requested_by: RequestedBy) -> ApprovalRequestDraftV1 {
    ApprovalRequestDraftV1 {
        approval_id: ApprovalId::from_str("apr_01JQ8ZB8H2XKM9P4QW7NRT5YCD").unwrap(),
        task_id: TaskId::from_str("tsk_01JQ8ZB8H2XKM9P4QW7NRT5YCD").unwrap(),
        task_kind: TaskKind::UserRequest,
        plan_revision: 1,
        capability_id: CapabilityId::from_str("calendar.events.create").unwrap(),
        capability_version: SemVer::new("1.0.0").unwrap(),
        registry_generation: 1,
        descriptor_digest: digest('a'),
        risk_class: RiskClass::ExternalWrite,
        side_effect_class: SideEffectClass::ExternalWrite,
        authorization: Authorization::ScopedGrant,
        data_class: DataClass::Personal,
        requested_by,
        actions: ApprovalActionSetV1::new(vec![action(
            "stp_01JQ8ZB8H2XKM9P4QW7NRT5YCD",
            'b',
            json!({"calendar":"primary"}),
        )])
        .unwrap(),
        summary: SafeApprovalSummaryV1::from_host_builder(
            "calendar.event.create",
            "Create one event",
            DataClass::Personal,
        )
        .unwrap(),
        raised_at: EpochMillis::new(100).unwrap(),
        expires_at: EpochMillis::new(expires_at).unwrap(),
    }
}

#[test]
fn request_accepts_exact_bound_and_rejects_larger_lifetime() {
    assert!(
        ApprovalRequestV1::new(request_draft(1_800_100, serea_protocol::RequestedBy::User)).is_ok()
    );
    assert_eq!(
        ApprovalRequestV1::new(request_draft(1_800_101, serea_protocol::RequestedBy::User)),
        Err(ApprovalError::InvalidExpiry)
    );
}

#[test]
fn request_rejects_forged_provenance_and_private_persistence() {
    assert_eq!(
        ApprovalRequestV1::new(request_draft(1_000, serea_protocol::RequestedBy::System)),
        Err(ApprovalError::InvalidProvenance)
    );
    assert_eq!(
        SafeApprovalSummaryV1::from_host_builder(
            "calendar.event.create",
            "do not persist",
            DataClass::Private
        ),
        Err(ApprovalError::DataClassExceeded)
    );
}

#[test]
fn a_scope_cannot_exceed_the_aggregate_request_classification() {
    let mut draft = request_draft(1_800_100, RequestedBy::User);
    draft.data_class = DataClass::Public;
    draft.summary = SafeApprovalSummaryV1::from_host_builder(
        "calendar.event.create",
        "Create one event",
        DataClass::Public,
    )
    .unwrap();
    assert_eq!(
        ApprovalRequestV1::new(draft),
        Err(ApprovalError::DataClassExceeded)
    );
}

#[test]
fn safe_summary_has_no_unratified_global_byte_limit() {
    let text = "host-reviewed ".repeat(4_000);
    assert_eq!(
        SafeApprovalSummaryV1::from_host_builder(
            "calendar.event.create",
            text.clone(),
            DataClass::Personal
        )
        .unwrap()
        .text(),
        text
    );
}

#[test]
fn approval_debug_views_redact_summary_and_structural_scope_values() {
    let mut draft = request_draft(1_800_100, RequestedBy::User);
    draft.summary = SafeApprovalSummaryV1::from_host_builder(
        "calendar.event.create",
        "private-summary-sentinel-81fd",
        DataClass::Personal,
    )
    .unwrap();
    draft.actions = ApprovalActionSetV1::new(vec![
        ApprovalActionV1::new(
            step("stp_01JQ8ZB8H2XKM9P4QW7NRT5YCD"),
            digest('b'),
            json!({"target":"private-scope-sentinel-9e24"}),
            DataClass::Personal,
        )
        .unwrap(),
    ])
    .unwrap();

    let request = ApprovalRequestV1::new(draft).unwrap();
    let policy_debug = format!("{request:?}");
    let storage_debug = format!("{:?}", request.storage_draft());
    for debug in [policy_debug, storage_debug] {
        assert!(!debug.contains("private-summary-sentinel-81fd"));
        assert!(!debug.contains("private-scope-sentinel-9e24"));
    }
}

#[test]
fn grant_revocation_uses_separate_authenticated_port_and_exact_grant_identity() {
    let approval = ApprovalId::from_str("apr_01JQ8ZB8H2XKM9P4QW7NRT5YCD").unwrap();
    let grant = GrantId::from_str("grt_01JQ8ZA7B3KMW9Q4TVY7XN2RDP").unwrap();
    let candidate = ApprovalRevocationCandidateV1::new(
        approval.clone(),
        grant.clone(),
        ApprovalPrincipalV1::User,
        ActorId::from_str("paired-user").unwrap(),
        Some(DeviceId::from_str("dev_01JQ8ZB8H2XKM9P4QW7NRT5YCD").unwrap()),
        Some(SessionId::from_str("ses_01JQ8ZB8H2XKM9P4QW7NRT5YCD").unwrap()),
        EpochMillis::new(200).unwrap(),
    );
    let auth =
        AuthenticatedApprovalRevocationV1::authenticate(candidate, &AcceptRevocation).unwrap();
    let facts = auth
        .validate_for(&approval, &grant, EpochMillis::new(300).unwrap())
        .unwrap();
    assert_eq!(facts.storage_draft().grant_id, grant);
    let wrong = GrantId::from_str("grt_01JQ8ZA7B3KMW9Q4TVY7XN2RDQ").unwrap();
    assert_eq!(
        auth.validate_for(&approval, &wrong, EpochMillis::new(300).unwrap()),
        Err(ApprovalResponseError::InvalidIdentityOrTime)
    );
}

#[test]
fn authenticated_grant_is_bound_to_exact_subset_and_use_count() {
    let request = ApprovalRequestV1::new(request_draft(1_800_100, RequestedBy::User)).unwrap();
    let candidate = ApprovalResponseCandidateV1::new(
        ApprovalId::from_str("apr_01JQ8ZB8H2XKM9P4QW7NRT5YCD").unwrap(),
        ApprovalResponseDecisionV1::Grant,
        ApprovalPrincipalV1::User,
        Some(DeviceId::from_str("dev_01JQ8ZB8H2XKM9P4QW7NRT5YCD").unwrap()),
        Some(SessionId::from_str("ses_01JQ8ZB8H2XKM9P4QW7NRT5YCD").unwrap()),
        ApprovalAuthenticationStrengthV1::Session,
        EpochMillis::new(200).unwrap(),
        None,
        vec![step("stp_01JQ8ZB8H2XKM9P4QW7NRT5YCD")],
        Some(1),
        Some(EpochMillis::new(1_000).unwrap()),
    );
    let response = AuthenticatedApprovalResponseV1::authenticate(candidate, &AcceptAuth).unwrap();
    let facts = response
        .validate_for(&request, EpochMillis::new(300).unwrap())
        .unwrap();
    assert_eq!(facts.approved_actions().actions().len(), 1);
    assert_eq!(
        facts.approved_actions().actions()[0].step_id().as_str(),
        "stp_01JQ8ZB8H2XKM9P4QW7NRT5YCD"
    );
}

#[test]
fn authenticated_grant_expiry_must_be_after_the_current_revalidation_time() {
    let request = ApprovalRequestV1::new(request_draft(1_800_100, RequestedBy::User)).unwrap();
    let candidate = ApprovalResponseCandidateV1::new(
        ApprovalId::from_str("apr_01JQ8ZB8H2XKM9P4QW7NRT5YCD").unwrap(),
        ApprovalResponseDecisionV1::Grant,
        ApprovalPrincipalV1::User,
        Some(DeviceId::from_str("dev_01JQ8ZB8H2XKM9P4QW7NRT5YCD").unwrap()),
        Some(SessionId::from_str("ses_01JQ8ZB8H2XKM9P4QW7NRT5YCD").unwrap()),
        ApprovalAuthenticationStrengthV1::Session,
        EpochMillis::new(200).unwrap(),
        None,
        vec![step("stp_01JQ8ZB8H2XKM9P4QW7NRT5YCD")],
        Some(1),
        Some(EpochMillis::new(300).unwrap()),
    );
    let response = AuthenticatedApprovalResponseV1::authenticate(candidate, &AcceptAuth).unwrap();
    assert_eq!(
        response.validate_for(&request, EpochMillis::new(300).unwrap()),
        Err(ApprovalResponseError::InvalidExpiryBound)
    );
}

#[test]
fn authenticated_port_refuses_stale_session_before_lifecycle_mutation() {
    let candidate = ApprovalResponseCandidateV1::new(
        ApprovalId::from_str("apr_01JQ8ZB8H2XKM9P4QW7NRT5YCD").unwrap(),
        ApprovalResponseDecisionV1::Deny,
        ApprovalPrincipalV1::User,
        Some(DeviceId::from_str("dev_01JQ8ZB8H2XKM9P4QW7NRT5YCD").unwrap()),
        Some(SessionId::from_str("ses_01JQ8ZB8H2XKM9P4QW7NRT5YCD").unwrap()),
        ApprovalAuthenticationStrengthV1::Session,
        EpochMillis::new(200).unwrap(),
        None,
        Vec::new(),
        None,
        None,
    );
    assert_eq!(
        AuthenticatedApprovalResponseV1::authenticate(candidate, &StaleAuth),
        Err(ApprovalAuthenticationError::StaleActorSession)
    );
}

#[test]
fn elevated_action_refuses_a_plain_session_response() {
    let mut draft = request_draft(1_800_100, RequestedBy::User);
    draft.risk_class = RiskClass::ElevatedDevice;
    draft.side_effect_class = SideEffectClass::ElevatedDevice;
    let request = ApprovalRequestV1::new(draft).unwrap();
    let candidate = ApprovalResponseCandidateV1::new(
        ApprovalId::from_str("apr_01JQ8ZB8H2XKM9P4QW7NRT5YCD").unwrap(),
        ApprovalResponseDecisionV1::Grant,
        ApprovalPrincipalV1::User,
        Some(DeviceId::from_str("dev_01JQ8ZB8H2XKM9P4QW7NRT5YCD").unwrap()),
        Some(SessionId::from_str("ses_01JQ8ZB8H2XKM9P4QW7NRT5YCD").unwrap()),
        ApprovalAuthenticationStrengthV1::Session,
        EpochMillis::new(200).unwrap(),
        None,
        vec![step("stp_01JQ8ZB8H2XKM9P4QW7NRT5YCD")],
        None,
        None,
    );
    let response = AuthenticatedApprovalResponseV1::authenticate(candidate, &AcceptAuth).unwrap();
    assert_eq!(
        response.validate_for(&request, EpochMillis::new(300).unwrap()),
        Err(ApprovalResponseError::MissingElevatedConfirmation)
    );
}

#[test]
fn ordinary_approval_cannot_carry_credential_handoff() {
    let mut draft = request_draft(1_800_100, RequestedBy::User);
    draft.risk_class = RiskClass::Credential;
    draft.authorization = Authorization::CredentialHandoff;
    assert_eq!(
        ApprovalRequestV1::new(draft),
        Err(ApprovalError::CredentialHandoffSeparated)
    );
}
