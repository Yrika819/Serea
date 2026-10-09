use serea_capability::{ProposalRejection, ToolCallProposalV1, parse_tool_call_proposal};

#[test]
fn valid_proposal_is_parsed() {
    let proposal = parse_tool_call_proposal(
        r#"{"version":"1","capability_id":"calendar.events.read","arguments":{"calendar":"work"}}"#,
    )
    .expect("valid proposal");
    assert_eq!(proposal.capability_id().as_str(), "calendar.events.read");
    assert_eq!(proposal.arguments().len(), 1);
}

#[test]
fn proposal_debug_redacts_argument_values() {
    const SENTINEL: &str = "DO_NOT_LOG_THIS_VALUE_7a31";
    let proposal = parse_tool_call_proposal(&format!(
        r#"{{"version":"1","capability_id":"calendar.events.read","arguments":{{"x":"{SENTINEL}"}}}}"#
    ))
    .unwrap();
    assert!(!format!("{proposal:?}").contains(SENTINEL));
}

#[test]
fn rejection_debug_and_display_never_echo_raw_proposal_content() {
    const SENTINEL: &str = "DO_NOT_LOG_THIS_VALUE_7a31";
    for raw in [
        format!(
            r#"{{"version":"1","capability_id":"calendar.events.read","arguments":{{"calendar":"{SENTINEL}","calendar":"second"}}}}"#
        ),
        format!(
            r#"{{"version":"1","capability_id":"calendar.events.read","arguments":{{"calendar":"{SENTINEL}"}},"risk_class":"SECRET"}}"#
        ),
    ] {
        let rejection = parse_tool_call_proposal(&raw).unwrap_err();
        assert!(!format!("{rejection:?}").contains(SENTINEL));
        assert!(!rejection.to_string().contains(SENTINEL));
    }
}

#[test]
fn duplicate_root_key_rejected() {
    let err = parse_tool_call_proposal(
        r#"{"version":"1","capability_id":"calendar.events.read","capability_id":"gmail.messages.send","arguments":{}}"#,
    )
    .unwrap_err();
    assert!(matches!(err, ProposalRejection::DuplicateMemberName));
}

#[test]
fn duplicate_nested_arguments_key_rejected() {
    let err = parse_tool_call_proposal(
        r#"{"version":"1","capability_id":"calendar.events.read","arguments":{"calendar":"work","calendar":"home"}}"#,
    )
    .unwrap_err();
    assert!(matches!(err, ProposalRejection::DuplicateMemberName));
}

#[test]
fn wrong_version_rejected() {
    let err = parse_tool_call_proposal(
        r#"{"version":"2","capability_id":"calendar.events.read","arguments":{}}"#,
    )
    .unwrap_err();
    assert!(matches!(err, ProposalRejection::UnsupportedVersion));
}

#[test]
fn missing_field_rejected() {
    for (text, missing) in [
        (r#"{"version":"1","arguments":{}}"#, "capability_id"),
        (
            r#"{"capability_id":"calendar.events.read","arguments":{}}"#,
            "version",
        ),
        (
            r#"{"version":"1","capability_id":"calendar.events.read"}"#,
            "arguments",
        ),
    ] {
        let err = parse_tool_call_proposal(text).unwrap_err();
        assert!(matches!(err, ProposalRejection::MissingField { .. }));
        assert_eq!(err.offending_field_names(), [missing]);
    }
}

#[test]
fn non_object_arguments_rejected() {
    for text in [
        r#"{"version":"1","capability_id":"calendar.events.read","arguments":"work"}"#,
        r#"{"version":"1","capability_id":"calendar.events.read","arguments":[]}"#,
        r#"{"version":"1","capability_id":"calendar.events.read","arguments":7}"#,
        r#"{"version":"1","capability_id":"calendar.events.read","arguments":null}"#,
    ] {
        let err = parse_tool_call_proposal(text).unwrap_err();
        assert!(
            matches!(err, ProposalRejection::ArgumentsNotObject),
            "{text} produced {err:?}"
        );
    }
}

#[test]
fn root_not_object_rejected() {
    let err = parse_tool_call_proposal(r#"[{"version":"1"}]"#).unwrap_err();
    assert!(matches!(err, ProposalRejection::RootNotObject));
}

#[test]
fn malformed_json_rejected() {
    let err = parse_tool_call_proposal(r#"{"version":"1","#).unwrap_err();
    assert!(matches!(err, ProposalRejection::MalformedJson));
}

#[test]
fn every_host_authority_field_injection_rejects_whole_proposal() {
    // Each of these is a host-resolved fact that must never appear in model
    // output. The whole proposal is refused; nothing is stripped and reused.
    let injections = [
        "request_id",
        "task_id",
        "step_id",
        "capability_version",
        "version_id",
        "provider_id",
        "implementation_id",
        "risk_class",
        "side_effect_class",
        "required_authorization",
        "replay_safety",
        "arguments_digest",
        "idempotency_key",
        "data_class",
        "requested_by",
        "deadline_ms",
        "approval",
        "policy",
        "credential_handle",
        "descriptor_digest",
        "descriptor",
        "generation_id",
        "risk",
        "authorization",
    ];
    for field in injections {
        let text = format!(
            r#"{{"version":"1","capability_id":"calendar.events.read","arguments":{{}},"{field}":"injected"}}"#
        );
        let err = parse_tool_call_proposal(&text).unwrap_err();
        assert!(
            matches!(err, ProposalRejection::UndeclaredMember { .. }),
            "{field} produced {err:?}"
        );
    }
}

#[test]
fn unknown_member_rejected() {
    let err = parse_tool_call_proposal(
        r#"{"version":"1","capability_id":"calendar.events.read","arguments":{},"extra":"x"}"#,
    )
    .unwrap_err();
    assert!(matches!(err, ProposalRejection::UndeclaredMember { .. }));
}

#[test]
fn malformed_capability_id_rejected() {
    let err = parse_tool_call_proposal(r#"{"version":"1","capability_id":"nope","arguments":{}}"#)
        .unwrap_err();
    assert!(matches!(err, ProposalRejection::MalformedCapabilityId));
}

#[test]
fn rejection_reason_carries_only_field_names_and_counts() {
    let err = parse_tool_call_proposal(
        r#"{"version":"1","capability_id":"calendar.events.read","arguments":{},"risk_class":"LOCAL_STATE","data_class":"SECRET"}"#,
    )
    .unwrap_err();
    // names and a count only; never the injected values
    assert_eq!(
        err.offending_field_names(),
        vec!["data_class", "risk_class"]
    );
    assert_eq!(err.offending_field_count(), 2);
    let rendered = err.to_string();
    assert!(!rendered.contains("LOCAL_STATE"));
    assert!(!rendered.contains("SECRET"));
}

#[test]
fn proposal_exposes_no_authority_surface() {
    let proposal: &ToolCallProposalV1 = &parse_tool_call_proposal(
        r#"{"version":"1","capability_id":"calendar.events.read","arguments":{"calendar":"work"}}"#,
    )
    .unwrap();
    // the type offers only what the frozen shape declares
    let capability_id = proposal.capability_id().as_str();
    let arguments = proposal.arguments().len();
    assert_eq!(capability_id, "calendar.events.read");
    assert_eq!(arguments, 1);
}
