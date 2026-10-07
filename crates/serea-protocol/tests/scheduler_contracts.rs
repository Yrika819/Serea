use serea_protocol::{
    DataClass, DeviceConnectedPayloadV1, EventKind, EventPredicateV1, MAX_SCHEDULE_TEMPLATE_BYTES,
    ScheduledTaskTemplateV1, TaskTitle,
};

#[test]
fn device_connected_payload_is_closed_typed_and_canonical() {
    let parsed = DeviceConnectedPayloadV1::parse_json(
        r#"{ "device_id" : "dev_01JQ8ZC5N8TVG3K6MRQ2XW9JHF" }"#,
    )
    .unwrap();
    assert_eq!(
        parsed.device_id().as_str(),
        "dev_01JQ8ZC5N8TVG3K6MRQ2XW9JHF"
    );
    assert_eq!(
        parsed.canonical_json(),
        r#"{"device_id":"dev_01JQ8ZC5N8TVG3K6MRQ2XW9JHF"}"#
    );
    assert_eq!(
        DeviceConnectedPayloadV1::parse_json(parsed.canonical_json()).unwrap(),
        parsed
    );
}

#[test]
fn device_connected_payload_rejects_missing_invalid_unknown_and_duplicate_fields() {
    for input in [
        r#"{}"#,
        r#"{"device_id":"not-a-device"}"#,
        r#"{"device_id":"dev_01JQ8ZC5N8TVG3K6MRQ2XW9JHF","task_id":"tsk_01JQ8Z9K3M7QWXR4V2T6YH0BNA"}"#,
        r#"{"device_id":"dev_01JQ8ZC5N8TVG3K6MRQ2XW9JHF","device_id":"dev_01JQ8ZC5N8TVG3K6MRQ2XW9JHF"}"#,
    ] {
        assert!(
            DeviceConnectedPayloadV1::parse_json(input).is_err(),
            "accepted {input}"
        );
    }
}

#[test]
fn event_predicate_is_closed_duplicate_aware_and_exactly_canonical() {
    let parsed =
        EventPredicateV1::parse_json(r#"{ "event_kind" : "TASK_CREATED", "version" : "1" }"#)
            .unwrap();
    assert_eq!(parsed.event_kind(), EventKind::TaskCreated);
    assert!(parsed.matches_event_kind(EventKind::TaskCreated));
    assert!(!parsed.matches_event_kind(EventKind::TaskFailed));
    assert_eq!(
        parsed.canonical_json(),
        r#"{"event_kind":"TASK_CREATED","version":"1"}"#
    );
    assert_eq!(
        EventPredicateV1::parse_json(parsed.canonical_json()).unwrap(),
        parsed
    );
}

#[test]
fn event_predicate_rejects_unknown_version_kind_fields_and_duplicate_keys() {
    for input in [
        r#"{"version":"2","event_kind":"TASK_CREATED"}"#,
        r#"{"version":"1","event_kind":"UNREGISTERED"}"#,
        r#"{"version":"1","event_kind":"TASK_CREATED","payload":{}}"#,
        r#"{"version":"1","version":"1","event_kind":"TASK_CREATED"}"#,
        r#"{"version":"1","event_kind":"TASK_CREATED","extra":1.0}"#,
    ] {
        assert!(
            EventPredicateV1::parse_json(input).is_err(),
            "accepted {input}"
        );
    }
}

#[test]
fn every_registered_event_kind_has_pinned_host_event_eligibility() {
    let excluded = [
        "APPROVAL_GRANTED",
        "APPROVAL_DENIED",
        "APPROVAL_EXPIRED",
        "DEVICE_CONNECTED",
        "SCHEDULE_CREATED",
        "SCHEDULE_UPDATED",
        "SCHEDULE_PAUSED",
        "SCHEDULE_RESUMED",
        "SCHEDULE_CANCELLED",
        "SCHEDULE_OCCURRENCE_MISSED",
        "SCHEDULE_TASK_CREATED",
        "SCHEDULE_CATCH_UP_DEFERRED",
    ];
    for token in EventKind::WIRE_NAMES {
        let event_kind: EventKind = serde_json::from_str(&format!("\"{token}\"")).unwrap();
        assert_eq!(
            EventPredicateV1::parse_json(&format!(r#"{{"version":"1","event_kind":"{token}"}}"#))
                .is_ok(),
            !excluded.contains(token),
            "unexpected HOST_EVENT eligibility for {token}"
        );
        assert_eq!(
            serea_protocol::event_kind_is_host_event_eligible(event_kind),
            !excluded.contains(token),
            "runtime eligibility mismatch for {token}"
        );
    }
}

#[test]
fn template_uses_task_title_and_prose_validation() {
    let template = ScheduledTaskTemplateV1::parse_json(
        r#"{"version":"1","title":"Morning summary","intent":"Summarize approved updates."}"#,
    )
    .unwrap();
    assert_eq!(
        template.title(),
        &TaskTitle::new("Morning summary").unwrap()
    );
    assert_eq!(template.intent(), "Summarize approved updates.");
    assert_eq!(
        ScheduledTaskTemplateV1::parse_json(template.canonical_json()).unwrap(),
        template
    );
}

#[test]
fn template_is_closed_and_rejects_all_authority_fields() {
    for field in [
        "policy_class",
        "approval_policy",
        "approval_grant",
        "approval_result",
        "capability_id",
        "provider_id",
        "capability_version",
        "action_request",
        "task_step",
        "plan",
        "idempotency_key",
        "receipt",
        "credential_handle",
        "credential",
        "goal",
        "goal_latch_goal",
        "model_id",
        "model_route",
        "bound_override",
        "admin_authorization",
        "arguments",
    ] {
        let input =
            format!(r#"{{"version":"1","title":"Title","intent":"Do work","{field}":{{}}}}"#);
        assert!(
            ScheduledTaskTemplateV1::parse_json(&input).is_err(),
            "accepted {field}"
        );
    }
    for input in [
        r#"{"version":"2","title":"Title","intent":"Do work"}"#,
        r#"{"version":"1","title":"Title","intent":"Do work","unknown":true}"#,
        r#"{"version":"1","version":"1","title":"Title","intent":"Do work"}"#,
        r#"{"version":"1","title":"","intent":"Do work"}"#,
        r#"{"version":"1","title":"   ","intent":"Do work"}"#,
        r#"{"version":"1","title":"Title","intent":" \n\t "}"#,
    ] {
        assert!(
            ScheduledTaskTemplateV1::parse_json(input).is_err(),
            "accepted {input}"
        );
    }
}

#[test]
fn template_inherits_highest_host_class_and_enforces_byte_bound() {
    let template = ScheduledTaskTemplateV1::parse_json(
        r#"{ "intent" : "Do work", "title" : "Title", "version" : "1" }"#,
    )
    .unwrap();
    assert_eq!(
        template.inherited_data_class(DataClass::Personal, DataClass::Public),
        DataClass::Personal
    );
    assert_eq!(
        template.inherited_data_class(DataClass::Public, DataClass::Private),
        DataClass::Private
    );

    let empty = r#"{"intent":"","title":"T","version":"1"}"#;
    let exact = format!(
        r#"{{"intent":"{}","title":"T","version":"1"}}"#,
        "x".repeat(MAX_SCHEDULE_TEMPLATE_BYTES - empty.len())
    );
    assert_eq!(exact.len(), MAX_SCHEDULE_TEMPLATE_BYTES);
    assert!(ScheduledTaskTemplateV1::parse_json(&exact).is_ok());
    let over = exact.replace("\"title\":\"T\"", "\"title\":\"TT\"");
    assert_eq!(over.len(), MAX_SCHEDULE_TEMPLATE_BYTES + 1);
    assert!(ScheduledTaskTemplateV1::parse_json(&over).is_err());
}
