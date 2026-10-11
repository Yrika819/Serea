use std::str::FromStr;

use serea_protocol::{
    Authorization, CapabilityId, DataClass, Digest, RequestedBy, RiskClass, RootRequirement,
    SemVer, SideEffectClass, StepId, TaskId, TaskKind, TaskState,
};

use crate::{
    AutomationContext, DenyReason, DeviceTrustState, PolicyDecision, PolicyInputDraft,
    PolicyInputV1, PolicyRevision, PolicyRule, RuleDecision, RuleMatch, evaluate,
    evaluate_with_evidence,
};

fn input(kind: TaskKind, requested_by: RequestedBy, risk: RiskClass) -> PolicyInputDraft {
    PolicyInputV1::new(
        PolicyInputDraft {
            task_id: TaskId::from_str("tsk_01JQ8ZB8H2XKM9P4QW7NRT5YCD").unwrap(),
            step_id: StepId::from_str("stp_01JQ8ZB8H2XKM9P4QW7NRT5YCE").unwrap(),
            task_kind: kind,
            task_state: TaskState::Executing,
            task_cancelled: false,
            task_deadline_at: None,
            task_data_class: DataClass::Personal,
            task_policy_class: RiskClass::Credential,
            capability_id: CapabilityId::from_str("mail.messages.list").unwrap(),
            capability_version: SemVer::new("1.0.0").unwrap(),
            provider_id: serea_protocol::ProviderId::from_str("mail").unwrap(),
            implementation_id: None,
            risk_class: risk,
            side_effect_class: if risk == RiskClass::Observe {
                SideEffectClass::None
            } else {
                SideEffectClass::ExternalWrite
            },
            authorization: Authorization::None,
            root_requirement: RootRequirement::NotRequired,
            data_class: DataClass::Personal,
            requested_by,
            device_state: None,
            capability_enabled: true,
            destructive_enabled: true,
            plan_revision: 1,
            registry_generation: 1,
            generation_digest: Digest::from_str(&format!("sha256:{}", "c".repeat(64))).unwrap(),
            descriptor_digest: Digest::from_str(&format!("sha256:{}", "a".repeat(64))).unwrap(),
            arguments_digest: Digest::from_str(&format!("sha256:{}", "b".repeat(64))).unwrap(),
        },
        &revision(vec![]),
    )
    .unwrap()
    .into()
}

fn revision(rules: Vec<PolicyRule>) -> PolicyRevision {
    PolicyRevision::new(1, rules).unwrap()
}

fn eval(draft: PolicyInputDraft, revision: &PolicyRevision) -> PolicyDecision {
    let input = PolicyInputV1::new(draft, revision).unwrap();
    evaluate(&input, revision)
}

fn rule(id: &str, priority: i32, decision: RuleDecision, matcher: RuleMatch) -> PolicyRule {
    PolicyRule::new(id, priority, true, matcher, decision).unwrap()
}

#[test]
fn observe_and_local_state_default_allow() {
    let rev = revision(vec![]);
    for risk in [RiskClass::Observe, RiskClass::LocalState] {
        assert_eq!(
            eval(input(TaskKind::UserRequest, RequestedBy::User, risk), &rev),
            PolicyDecision::Allow
        );
    }
}

#[test]
fn code_owned_defaults_and_mandatory_human_paths_are_not_rule_defaults() {
    let rev = revision(vec![]);
    for risk in [
        RiskClass::ExternalWrite,
        RiskClass::Communication,
        RiskClass::ElevatedDevice,
    ] {
        assert!(matches!(
            eval(input(TaskKind::UserRequest, RequestedBy::User, risk), &rev),
            PolicyDecision::RequireApproval(_)
        ));
    }

    let mut reversible_local = input(
        TaskKind::UserRequest,
        RequestedBy::User,
        RiskClass::ReversibleWrite,
    );
    reversible_local.side_effect_class = SideEffectClass::LocalState;
    assert_eq!(eval(reversible_local, &rev), PolicyDecision::Allow);
    assert!(matches!(
        eval(
            input(
                TaskKind::UserRequest,
                RequestedBy::User,
                RiskClass::ReversibleWrite
            ),
            &rev
        ),
        PolicyDecision::RequireApproval(_)
    ));

    let allow = revision(vec![rule(
        "allow_everything",
        i32::MAX,
        RuleDecision::Allow,
        RuleMatch::default(),
    )]);
    let destructive = eval(
        input(
            TaskKind::UserRequest,
            RequestedBy::User,
            RiskClass::Destructive,
        ),
        &allow,
    );
    assert_eq!(
        destructive,
        PolicyDecision::RequireApproval(crate::ApprovalReason::DestructiveRequiresApproval)
    );
    assert_eq!(
        eval(
            input(
                TaskKind::UserRequest,
                RequestedBy::User,
                RiskClass::Credential
            ),
            &allow
        ),
        PolicyDecision::RequireHandoff
    );
}

#[test]
fn proactive_observe_is_allowed_but_external_write_is_hard_denied() {
    let rev = revision(vec![]);
    assert_eq!(
        AutomationContext::derive(TaskKind::Proactive, RequestedBy::Model).unwrap(),
        AutomationContext::Proactive
    );
    assert_eq!(
        eval(
            input(TaskKind::Proactive, RequestedBy::User, RiskClass::Observe),
            &rev
        ),
        PolicyDecision::Allow
    );
    assert_eq!(
        eval(
            input(
                TaskKind::Proactive,
                RequestedBy::User,
                RiskClass::ExternalWrite
            ),
            &rev
        ),
        PolicyDecision::Deny(DenyReason::AutomatedActionForbidden)
    );
}

#[test]
fn root_allow_rule_still_requires_approval() {
    let matcher = RuleMatch {
        risk_class: Some(RiskClass::ElevatedDevice),
        ..RuleMatch::default()
    };
    let rev = revision(vec![rule("root", 100, RuleDecision::Allow, matcher)]);
    let mut draft = input(
        TaskKind::UserRequest,
        RequestedBy::User,
        RiskClass::ElevatedDevice,
    );
    draft.root_requirement = RootRequirement::RequiresRoot;
    assert!(matches!(
        eval(draft, &rev),
        PolicyDecision::RequireApproval(_)
    ));
}

#[test]
fn system_context_never_auto_allows_destructive_or_effecting_work() {
    let rev = revision(vec![rule(
        "allow",
        100,
        RuleDecision::Allow,
        RuleMatch::default(),
    )]);
    assert_eq!(
        AutomationContext::derive(TaskKind::Maintenance, RequestedBy::System).unwrap(),
        AutomationContext::System
    );
    assert_ne!(
        eval(
            input(
                TaskKind::Maintenance,
                RequestedBy::System,
                RiskClass::Destructive
            ),
            &rev
        ),
        PolicyDecision::Allow
    );
    assert_ne!(
        eval(
            input(
                TaskKind::Maintenance,
                RequestedBy::System,
                RiskClass::ExternalWrite
            ),
            &rev
        ),
        PolicyDecision::Allow
    );
}

#[test]
fn invalid_user_request_system_provenance_is_refused() {
    assert!(AutomationContext::derive(TaskKind::UserRequest, RequestedBy::System).is_err());
}

#[test]
fn task_ceiling_is_hard_and_deny_overrides_allow_at_any_priority() {
    let allow_match = RuleMatch {
        capability_id: Some(CapabilityId::from_str("mail.messages.list").unwrap()),
        ..RuleMatch::default()
    };
    let deny_match = RuleMatch {
        risk_class: Some(RiskClass::Observe),
        ..RuleMatch::default()
    };
    let rev = revision(vec![
        rule("z_allow", i32::MAX, RuleDecision::Allow, allow_match),
        rule(
            "a_deny",
            i32::MIN,
            RuleDecision::Deny(DenyReason::PolicyRuleDenied),
            deny_match,
        ),
    ]);
    assert_eq!(
        eval(
            input(TaskKind::UserRequest, RequestedBy::User, RiskClass::Observe),
            &rev
        ),
        PolicyDecision::Deny(DenyReason::PolicyRuleDenied)
    );
    let mut draft = input(
        TaskKind::UserRequest,
        RequestedBy::User,
        RiskClass::Communication,
    );
    draft.task_policy_class = RiskClass::Observe;
    assert_eq!(
        eval(draft, &rev),
        PolicyDecision::Deny(DenyReason::TaskPolicyCeilingExceeded)
    );
}

#[test]
fn priority_then_rule_id_is_stable_and_rule_permutation_is_irrelevant() {
    let match_all = RuleMatch {
        side_effect_class: Some(SideEffectClass::None),
        ..RuleMatch::default()
    };
    let low_id = rule("alpha", 8, RuleDecision::RequireApproval, match_all.clone());
    let high_id = rule("omega", 8, RuleDecision::Allow, match_all);
    let first = revision(vec![high_id.clone(), low_id.clone()]);
    let second = revision(vec![low_id, high_id]);
    let action = input(
        TaskKind::UserRequest,
        RequestedBy::Model,
        RiskClass::Observe,
    );
    assert_eq!(eval(action.clone(), &first), eval(action.clone(), &second));
    assert_eq!(
        eval(action, &first),
        PolicyDecision::RequireApproval(crate::ApprovalReason::PolicyRequiresApproval)
    );
}

#[test]
fn priority_descending_precedes_the_rule_id_tiebreak() {
    let matcher = RuleMatch {
        risk_class: Some(RiskClass::Observe),
        ..RuleMatch::default()
    };
    let rev = revision(vec![
        rule(
            "z_high_priority",
            11,
            RuleDecision::RequireApproval,
            matcher.clone(),
        ),
        rule("a_low_priority", 10, RuleDecision::Allow, matcher),
    ]);
    assert!(matches!(
        eval(
            input(TaskKind::UserRequest, RequestedBy::User, RiskClass::Observe),
            &rev
        ),
        PolicyDecision::RequireApproval(_)
    ));
}

#[test]
fn explicit_rules_precede_defaults_and_credential_handoff_is_distinct() {
    let matcher = RuleMatch {
        risk_class: Some(RiskClass::ExternalWrite),
        ..RuleMatch::default()
    };
    let rev = revision(vec![rule(
        "allow_external",
        1,
        RuleDecision::Allow,
        matcher,
    )]);
    assert_eq!(
        eval(
            input(
                TaskKind::UserRequest,
                RequestedBy::User,
                RiskClass::ExternalWrite
            ),
            &rev
        ),
        PolicyDecision::Allow
    );
    let mut draft = input(
        TaskKind::UserRequest,
        RequestedBy::User,
        RiskClass::Credential,
    );
    draft.authorization = Authorization::CredentialHandoff;
    let rev = revision(vec![]);
    assert_eq!(eval(draft, &rev), PolicyDecision::RequireHandoff);
}

#[test]
fn disabled_capability_and_unenabled_destructive_action_fail_closed() {
    let rev = revision(vec![rule(
        "allow",
        1,
        RuleDecision::Allow,
        RuleMatch::default(),
    )]);
    let mut draft = input(TaskKind::UserRequest, RequestedBy::User, RiskClass::Observe);
    draft.capability_enabled = false;
    assert_eq!(
        eval(draft, &rev),
        PolicyDecision::Deny(DenyReason::CapabilityDisabled)
    );
    let mut draft = input(
        TaskKind::UserRequest,
        RequestedBy::User,
        RiskClass::Destructive,
    );
    draft.destructive_enabled = false;
    assert_eq!(
        eval(draft, &rev),
        PolicyDecision::Deny(DenyReason::CapabilityDisabled)
    );
}

#[test]
fn terminal_or_cancelled_tasks_cannot_receive_allow_and_credential_data_needs_credential_risk() {
    let rev = revision(vec![]);
    let mut draft = input(TaskKind::UserRequest, RequestedBy::User, RiskClass::Observe);
    draft.task_state = TaskState::Cancelled;
    assert_eq!(
        eval(draft, &rev),
        PolicyDecision::Deny(DenyReason::TaskNotLive)
    );

    let mut draft = input(TaskKind::UserRequest, RequestedBy::User, RiskClass::Observe);
    draft.task_cancelled = true;
    assert_eq!(
        eval(draft, &rev),
        PolicyDecision::Deny(DenyReason::TaskNotLive)
    );

    let mut draft = input(TaskKind::UserRequest, RequestedBy::User, RiskClass::Observe);
    draft.data_class = DataClass::Credential;
    assert!(PolicyInputV1::new(draft, &rev).is_err());
}

#[test]
fn revision_digest_is_content_addressed_and_checked() {
    let rules = vec![rule("one", 1, RuleDecision::Allow, RuleMatch::default())];
    let first = PolicyRevision::new(7, rules.clone()).unwrap();
    let second = PolicyRevision::new(7, rules).unwrap();
    assert_eq!(first.rules_digest(), second.rules_digest());
    let permuted = PolicyRevision::new(
        7,
        vec![
            rule("z_rule", 1, RuleDecision::Allow, RuleMatch::default()),
            rule("a_rule", 1, RuleDecision::Allow, RuleMatch::default()),
        ],
    )
    .unwrap();
    let reordered = PolicyRevision::new(
        7,
        vec![
            rule("a_rule", 1, RuleDecision::Allow, RuleMatch::default()),
            rule("z_rule", 1, RuleDecision::Allow, RuleMatch::default()),
        ],
    )
    .unwrap();
    assert_eq!(permuted.rules_digest(), reordered.rules_digest());
    let restart = PolicyRevision::from_snapshot(
        7,
        permuted.rules_digest().clone(),
        permuted.rules().to_vec(),
    )
    .unwrap();
    let action = input(TaskKind::UserRequest, RequestedBy::User, RiskClass::Observe);
    assert_eq!(eval(action.clone(), &permuted), eval(action, &restart));
    assert!(PolicyRevision::new(0, vec![]).is_err());
    assert!(PolicyRevision::from_snapshot(7, first.rules_digest().clone(), vec![]).is_err());
}

#[test]
fn repeated_evaluation_over_one_snapshot_is_identical() {
    let rev = revision(vec![
        rule(
            "approval_for_external",
            30,
            RuleDecision::RequireApproval,
            RuleMatch {
                risk_class: Some(RiskClass::ExternalWrite),
                ..RuleMatch::default()
            },
        ),
        rule(
            "deny_untrusted_source",
            -10,
            RuleDecision::Deny(DenyReason::PolicyRuleDenied),
            RuleMatch {
                requested_by: Some(RequestedBy::ProactiveWatcher),
                ..RuleMatch::default()
            },
        ),
    ]);
    let validated = PolicyInputV1::new(
        input(
            TaskKind::UserRequest,
            RequestedBy::User,
            RiskClass::ExternalWrite,
        ),
        &rev,
    )
    .unwrap();
    let first = evaluate(&validated, &rev);
    for _ in 0..1_000 {
        assert_eq!(evaluate(&validated, &rev), first);
    }
}

#[test]
fn stale_revision_is_refused_and_evidence_is_not_execution_authority() {
    let rev = revision(vec![]);
    let different = revision(vec![rule(
        "new",
        1,
        RuleDecision::Deny(DenyReason::PolicyRuleDenied),
        RuleMatch::default(),
    )]);
    let input = PolicyInputV1::new(
        input(TaskKind::UserRequest, RequestedBy::User, RiskClass::Observe),
        &rev,
    )
    .unwrap();
    assert_eq!(
        evaluate(&input, &different),
        PolicyDecision::Deny(DenyReason::StalePolicyRevision)
    );

    let result = evaluate_with_evidence(&input, &rev);
    assert_eq!(result.decision, PolicyDecision::Allow);
    assert_eq!(result.evidence.task_id(), &input.facts().task_id);
    assert_eq!(result.evidence.decision(), PolicyDecision::Allow);
    assert_eq!(result.evidence.policy_revision_id(), 1);
}

#[test]
fn every_trusted_context_derivation_is_finite_and_model_user_are_equal() {
    use AutomationContext::{
        Interactive, Proactive as ProactiveContext, Scheduled as ScheduledContext,
        System as SystemContext,
    };
    use RequestedBy::{Model, ProactiveWatcher, Scheduler, System, User};
    use TaskKind::{DelegatedHostGoal, Maintenance, Proactive, Scheduled, UserRequest};
    for kind in [UserRequest, DelegatedHostGoal] {
        assert_eq!(AutomationContext::derive(kind, User).unwrap(), Interactive);
        assert_eq!(AutomationContext::derive(kind, Model).unwrap(), Interactive);
        assert_eq!(
            AutomationContext::derive(kind, Scheduler).unwrap(),
            Interactive
        );
        assert!(AutomationContext::derive(kind, System).is_err());
    }
    for requested_by in [User, Model, Scheduler, System] {
        assert_eq!(
            AutomationContext::derive(Scheduled, requested_by).unwrap(),
            ScheduledContext
        );
        assert_eq!(
            AutomationContext::derive(Maintenance, requested_by).unwrap(),
            SystemContext
        );
    }
    assert_eq!(
        AutomationContext::derive(Proactive, System).unwrap(),
        ProactiveContext
    );
    assert_eq!(
        AutomationContext::derive(Scheduled, ProactiveWatcher).unwrap(),
        ProactiveContext
    );
}

#[test]
fn handoff_rules_cannot_route_noncredential_actions_to_a_credential_channel() {
    assert!(
        PolicyRevision::new(
            1,
            vec![rule(
                "bad_handoff",
                1,
                RuleDecision::RequireHandoff,
                RuleMatch::default()
            )]
        )
        .is_err()
    );
}

#[test]
fn every_closed_rule_dimension_is_an_exact_match() {
    let matcher = RuleMatch {
        capability_id: Some(CapabilityId::from_str("mail.messages.list").unwrap()),
        risk_class: Some(RiskClass::Observe),
        side_effect_class: Some(SideEffectClass::None),
        authorization: Some(Authorization::None),
        automation_context: Some(AutomationContext::Interactive),
        requested_by: Some(RequestedBy::User),
        enabled: Some(true),
    };
    let policy_rule = rule("exact_match", 1, RuleDecision::RequireApproval, matcher);
    let revision = revision(vec![policy_rule.clone()]);
    let base = input(TaskKind::UserRequest, RequestedBy::User, RiskClass::Observe);
    let accepted = PolicyInputV1::new(base.clone(), &revision).unwrap();
    assert!(crate::matches_rule(&policy_rule, &accepted));

    let mut changed = base.clone();
    changed.capability_id = CapabilityId::from_str("calendar.events.list").unwrap();
    assert!(!crate::matches_rule(
        &policy_rule,
        &PolicyInputV1::new(changed, &revision).unwrap()
    ));

    let mut changed = base.clone();
    changed.risk_class = RiskClass::LocalState;
    assert!(!crate::matches_rule(
        &policy_rule,
        &PolicyInputV1::new(changed, &revision).unwrap()
    ));

    let mut changed = base.clone();
    changed.side_effect_class = SideEffectClass::LocalState;
    assert!(!crate::matches_rule(
        &policy_rule,
        &PolicyInputV1::new(changed, &revision).unwrap()
    ));

    let mut changed = base.clone();
    changed.authorization = Authorization::DeviceUser;
    assert!(!crate::matches_rule(
        &policy_rule,
        &PolicyInputV1::new(changed, &revision).unwrap()
    ));

    let mut changed = base.clone();
    changed.task_kind = TaskKind::Scheduled;
    changed.requested_by = RequestedBy::Scheduler;
    assert!(!crate::matches_rule(
        &policy_rule,
        &PolicyInputV1::new(changed, &revision).unwrap()
    ));

    let mut changed = base.clone();
    changed.requested_by = RequestedBy::Model;
    assert!(!crate::matches_rule(
        &policy_rule,
        &PolicyInputV1::new(changed, &revision).unwrap()
    ));

    let mut changed = base;
    changed.capability_enabled = false;
    assert!(!crate::matches_rule(
        &policy_rule,
        &PolicyInputV1::new(changed, &revision).unwrap()
    ));
}

#[test]
fn task_data_ceiling_and_device_pairing_facts_are_validated() {
    let revision = revision(vec![]);
    let mut private_action = input(TaskKind::UserRequest, RequestedBy::User, RiskClass::Observe);
    private_action.data_class = DataClass::Private;
    assert!(matches!(
        PolicyInputV1::new(private_action, &revision),
        Err(crate::PolicyError::TaskDataClassExceeded)
    ));

    for state in [
        DeviceTrustState {
            paired: false,
            trusted: true,
            online: false,
        },
        DeviceTrustState {
            paired: false,
            trusted: false,
            online: true,
        },
    ] {
        let mut draft = input(TaskKind::UserRequest, RequestedBy::User, RiskClass::Observe);
        draft.device_state = Some(state);
        assert!(matches!(
            PolicyInputV1::new(draft, &revision),
            Err(crate::PolicyError::InvalidDeviceState)
        ));
    }

    let mut draft = input(TaskKind::UserRequest, RequestedBy::User, RiskClass::Observe);
    draft.device_state = Some(DeviceTrustState {
        paired: true,
        trusted: true,
        online: true,
    });
    let validated = PolicyInputV1::new(draft, &revision).unwrap();
    assert!(validated.facts().device_state.unwrap().trusted);
}
