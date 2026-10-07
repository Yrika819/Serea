use serea_protocol::{DataClass, TaskTitle};
use serea_scheduler::{MAX_SCHEDULE_TEMPLATE_BYTES, ScheduledTaskTemplateV1};

fn parse(intent: &str) -> Result<ScheduledTaskTemplateV1, serea_scheduler::TemplateError> {
    let title = serde_json::to_string("Morning summary").unwrap();
    let intent = serde_json::to_string(intent).unwrap();
    ScheduledTaskTemplateV1::parse_json(&format!(
        r#"{{"version":"1","title":{title},"intent":{intent}}}"#
    ))
}

#[test]
fn valid_template_uses_task_title_and_prose_validation() {
    let template = parse("Summarize the latest approved updates.").unwrap();
    assert_eq!(
        template.title(),
        &TaskTitle::new("Morning summary").unwrap()
    );
    assert_eq!(template.intent(), "Summarize the latest approved updates.");
}

#[test]
fn exact_shape_and_versions_are_enforced() {
    for input in [
        r#"{"version":"2","title":"Title","intent":"Do work"}"#,
        r#"{"version":"1","title":"Title","intent":"Do work","extra":true}"#,
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
fn authority_bearing_and_argument_fields_are_impossible() {
    for field in [
        "policy_class",
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
}

#[test]
fn canonical_round_trip_and_classification_inheritance_are_stable() {
    let template = ScheduledTaskTemplateV1::parse_json(
        r#"{ "intent" : "Do work", "title" : "Title", "version" : "1" }"#,
    )
    .unwrap();
    assert_eq!(
        ScheduledTaskTemplateV1::parse_json(template.canonical_json()).unwrap(),
        template
    );
    assert_eq!(
        template.inherited_data_class(DataClass::Personal, DataClass::Public),
        DataClass::Personal
    );
    assert_eq!(
        template.inherited_data_class(DataClass::Public, DataClass::Private),
        DataClass::Private
    );
}

#[test]
fn canonical_template_byte_bound_is_inclusive_and_deterministic() {
    let empty = r#"{"intent":"","title":"T","version":"1"}"#;
    let intent_bytes = MAX_SCHEDULE_TEMPLATE_BYTES - empty.len();
    let exact = format!(
        r#"{{"intent":"{}","title":"T","version":"1"}}"#,
        "x".repeat(intent_bytes)
    );
    assert_eq!(exact.len(), MAX_SCHEDULE_TEMPLATE_BYTES);
    assert!(ScheduledTaskTemplateV1::parse_json(&exact).is_ok());
    let over = exact.replace("\"title\":\"T\"", "\"title\":\"TT\"");
    assert_eq!(over.len(), MAX_SCHEDULE_TEMPLATE_BYTES + 1);
    assert!(ScheduledTaskTemplateV1::parse_json(&over).is_err());
}
