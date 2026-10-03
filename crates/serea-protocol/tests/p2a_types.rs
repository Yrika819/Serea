//! ADR-0018/0023 corrected wire invariants; no storage or engine behavior.
use serde_json::{Value, json};
use serea_protocol::errors::{ContractRule, ProtocolError, ValueField, ValueRejection};
use serea_protocol::ids::*;
use serea_protocol::types::*;

const DIGEST: &str = "sha256:44136fa355b3678a1146ad16f7e8649e94fb4fc21fe77e8310c060f61caaff8a";
const KEY: &str = "idk_9f2c1a7e4b6d0f8a3c5e9b1d7f2a4c6e8b0d3f5a7c9e1b4d6f8a0c2e4b6d8f9a";
const TIME: &str = "2026-10-01T09:14:22.100Z";
const KINDS: [&str; 8] = [
    "CAPABILITY",
    "DELEGATE",
    "VERIFY",
    "MODEL_TURN",
    "WAIT_APPROVAL",
    "WAIT_USER",
    "WAIT_SCHEDULE",
    "NOTIFY",
];
const STATUSES: [&str; 7] = [
    "PLANNED",
    "LEASED",
    "EXECUTING",
    "WAITING",
    "SUCCEEDED",
    "FAILED",
    "RECONCILED_ABSENT",
];
const OPTIONAL: [&str; 12] = [
    "idempotency_key",
    "provider_id",
    "capability_id",
    "capability_version",
    "result_digest",
    "started_at",
    "completed_at",
    "lease_owner",
    "lease_expires_at",
    "lease_generation",
    "side_effect_receipt",
    "error",
];

fn capability_shaped(kind: &str) -> bool {
    matches!(kind, "CAPABILITY" | "DELEGATE" | "VERIFY")
}

fn wait_kind(kind: &str) -> bool {
    matches!(kind, "WAIT_APPROVAL" | "WAIT_USER" | "WAIT_SCHEDULE")
}

fn supplied(field: &str) -> Value {
    match field {
        "idempotency_key" => json!(KEY),
        "provider_id" => json!("calendar"),
        "capability_id" => json!("calendar.events.list"),
        "capability_version" => json!("1.2.0"),
        "result_digest" => json!(DIGEST),
        "started_at" | "completed_at" | "lease_expires_at" => json!(TIME),
        "lease_owner" => json!("worker-1"),
        "lease_generation" => json!(1),
        "side_effect_receipt" => json!({
            "receipt_id": "rcp_01JQ8Z9M3R2CVN8H5FWK7PQDSF", "capability_id": "calendar.events.list",
            "idempotency_key": KEY, "effect_summary": "Recorded external effect",
            "observed_at": TIME, "replay_safe": true
        }),
        "error" => {
            json!({"kind": "INTERNAL", "code": "SYNTHETIC_ERROR", "message": "Synthetic failure", "retryable": false, "host_action": "STOP"})
        }
        _ => panic!("unknown test field"),
    }
}

// Independent expected matrix: None means optional; Some(true/false) means R/N.
fn expected_presence(kind: &str, status: &str, field: &str) -> Option<bool> {
    match field {
        "idempotency_key" | "provider_id" | "capability_id" | "capability_version" => {
            Some(capability_shaped(kind))
        }
        "result_digest" => match status {
            "SUCCEEDED" => Some(true),
            "FAILED" | "RECONCILED_ABSENT" => None,
            _ => Some(false),
        },
        "started_at" => Some(!matches!(status, "PLANNED" | "LEASED")),
        "completed_at" => Some(matches!(
            status,
            "SUCCEEDED" | "FAILED" | "RECONCILED_ABSENT"
        )),
        "lease_owner" | "lease_expires_at" => Some(matches!(status, "LEASED" | "EXECUTING")),
        "lease_generation" => Some(status != "PLANNED"),
        "side_effect_receipt" if status == "SUCCEEDED" && capability_shaped(kind) => None,
        "side_effect_receipt" => Some(false),
        "error" => Some(status == "FAILED"),
        _ => panic!("unknown test field"),
    }
}

fn wire(kind: &str, status: &str) -> Value {
    let mut value = json!({
        "step_id": "stp_01JQ8Z9M3R2CVN8H5FWK7PQDSF", "task_id": "tsk_01JQ8Z9K3M7QWXR4V2T6YH0BNA",
        "sequence": 0, "kind": kind, "status": status,
        "attempt": if status == "PLANNED" { 0 } else { 1 }, "input_digest": DIGEST
    });
    for field in OPTIONAL {
        if expected_presence(kind, status, field) == Some(true) {
            value[field] = supplied(field);
        }
    }
    value
}

fn both_paths(value: Value, expected: bool) {
    let draft: TaskStepDraft =
        serde_json::from_value(value.clone()).expect("typed supplied values");
    assert_eq!(
        TaskStep::new(draft.clone()).is_ok(),
        expected,
        "constructor: {value}"
    );
    assert_eq!(
        TaskStep::try_from(draft.clone()).is_ok(),
        expected,
        "TryFrom: {value}"
    );
    assert_eq!(
        StepPresence::try_from(draft).is_ok(),
        expected,
        "presence: {value}"
    );
    assert_eq!(
        serde_json::from_value::<TaskStep>(value).is_ok(),
        expected,
        "deserializer"
    );
}

#[test]
fn every_kind_status_cell_and_each_presence_member_uses_the_checked_paths() {
    for kind in KINDS {
        for status in STATUSES {
            let legal = status != "WAITING" || wait_kind(kind);
            let base = wire(kind, status);
            both_paths(base.clone(), legal);
            if !legal {
                continue;
            }
            for field in OPTIONAL {
                for present in [false, true] {
                    let mut value = base.clone();
                    if present {
                        value[field] = supplied(field);
                    } else {
                        value.as_object_mut().expect("object").remove(field);
                    }
                    let expected = expected_presence(kind, status, field)
                        .is_none_or(|required| required == present);
                    both_paths(value, expected);
                    if !present {
                        let mut null = base.clone();
                        null[field] = Value::Null;
                        both_paths(null, expected);
                    }
                }
            }
            for attempt in [0, 1, u32::MAX] {
                let mut value = base.clone();
                value["attempt"] = json!(attempt);
                both_paths(value, (attempt == 0) == (status == "PLANNED"));
            }
        }
    }
}

#[test]
fn exactly_seven_unconditional_members_and_none_is_omitted() {
    let value = wire("MODEL_TURN", "PLANNED");
    for field in [
        "step_id",
        "task_id",
        "sequence",
        "kind",
        "status",
        "attempt",
        "input_digest",
    ] {
        let mut missing = value.clone();
        missing.as_object_mut().expect("object").remove(field);
        assert!(
            serde_json::from_value::<TaskStep>(missing).is_err(),
            "missing {field}"
        );
        let mut null = value.clone();
        null[field] = Value::Null;
        assert!(
            serde_json::from_value::<TaskStep>(null).is_err(),
            "null {field}"
        );
    }
    let mut explicit_null = value.clone();
    for field in OPTIONAL {
        explicit_null[field] = Value::Null;
    }
    let step: TaskStep =
        serde_json::from_value(explicit_null).expect("all absent cells accept null");
    assert_eq!(serde_json::to_value(&step).expect("serialize"), value);
    assert_eq!(step.input_digest.as_str(), DIGEST);
    assert_eq!(step.attempt, 0);
    let draft = TaskStepDraft::from(step.clone());
    let checked = StepPresence::new(draft).expect("checked");
    assert_eq!(TaskStep::from(checked), step);
}

#[test]
fn generation_is_positive_when_supplied_and_never_wraps() {
    for status in STATUSES.into_iter().chain(["FUTURE_STATUS"]) {
        let kind = if status == "WAITING" {
            "WAIT_USER"
        } else {
            "CAPABILITY"
        };
        for generation in [1, u32::MAX] {
            let mut value = wire(kind, status);
            value["lease_generation"] = json!(generation);
            both_paths(value, status != "PLANNED");
        }
        let mut draft: TaskStepDraft =
            serde_json::from_value(wire(kind, status)).expect("valid draft scalar types");
        draft.lease_generation = Some(0);
        assert_eq!(
            TaskStep::new(draft),
            Err(ProtocolError::MalformedValue {
                field: ValueField::LeaseGeneration,
                reason: ValueRejection::OutOfRange
            })
        );
        let mut zero = wire(kind, status);
        zero["lease_generation"] = json!(0);
        assert!(serde_json::from_value::<TaskStepDraft>(zero.clone()).is_err());
        assert!(serde_json::from_value::<TaskStep>(zero).is_err());
        for invalid in [json!(4294967296_u64), json!(-1), json!(1.5), json!("1")] {
            let mut value = wire(kind, status);
            value["lease_generation"] = invalid;
            assert!(serde_json::from_value::<TaskStep>(value).is_err());
        }
    }
}

#[test]
fn generation_raw_integral_numbers_decode_through_draft_and_checked_step() {
    for (number, expected) in [
        ("1", 1),
        ("1.0", 1),
        ("1e0", 1),
        ("10e-1", 1),
        ("0.1e1", 1),
        ("1.00000000000000000000", 1),
        ("4294967295.0", u32::MAX),
        ("42949672950e-1", u32::MAX),
        ("0.00000000000000000000000000001e29", 1),
        ("429496729500000000000000000000e-20", u32::MAX),
        ("1E+0000000000000000000000000000000000000000", 1),
    ] {
        let mut value = wire("MODEL_TURN", "LEASED");
        value["lease_generation"] = json!("RAW_GENERATION");
        let raw = serde_json::to_string(&value)
            .unwrap()
            .replace("\"RAW_GENERATION\"", number);
        let draft: TaskStepDraft = serde_json::from_str(&raw).expect("integral generation draft");
        assert_eq!(draft.lease_generation, Some(expected), "{number}");
        let value: Value = serde_json::from_str(&raw).expect("precision-preserved value");
        let value_draft: TaskStepDraft = serde_json::from_value(value.clone()).unwrap();
        assert_eq!(
            value_draft.lease_generation,
            Some(expected),
            "value/{number}"
        );
        let checked = TaskStep::new(draft).expect("checked draft");
        let decoded: TaskStep = serde_json::from_str(&raw).expect("integral generation step");
        assert_eq!(decoded, checked);
        assert_eq!(serde_json::from_value::<TaskStep>(value).unwrap(), checked);
        assert_eq!(
            serde_json::to_value(decoded).unwrap()["lease_generation"],
            json!(expected)
        );
    }
}

#[test]
fn generation_raw_fractions_are_rejected_before_binary64_rounding() {
    for number in [
        "1.0000000000000001",
        "1.00000000000000000001",
        "0.99999999999999999999",
        "4294967295.0000001",
        "4294967294.9999999",
        "100000000000000000001e-20",
        "1e-1000",
        "1e1000000000",
        "1e-1000000000",
        "1e-9223372036854775808",
        "1.0e-9223372036854775807",
        "1e-170141183460469231731687303715884105728",
        "1e999999999999999999999999999999999999999999",
        "1e-999999999999999999999999999999999999999999",
    ] {
        let mut value = wire("MODEL_TURN", "LEASED");
        value["lease_generation"] = json!("RAW_GENERATION");
        let raw = serde_json::to_string(&value)
            .unwrap()
            .replace("\"RAW_GENERATION\"", number);
        let value: Value = serde_json::from_str(&raw).unwrap();
        assert!(
            serde_json::from_str::<TaskStepDraft>(&raw).is_err(),
            "raw draft/{number}"
        );
        assert!(
            serde_json::from_str::<TaskStep>(&raw).is_err(),
            "raw step/{number}"
        );
        assert!(
            serde_json::from_value::<TaskStepDraft>(value.clone()).is_err(),
            "value draft/{number}"
        );
        assert!(
            serde_json::from_value::<TaskStep>(value).is_err(),
            "value step/{number}"
        );
    }
}

#[test]
fn generation_draft_errors_do_not_echo_rejected_raw_values() {
    const MARKER: &str = "sk-proj-generation-private-marker";
    for invalid in [
        json!(MARKER),
        json!({"$serde_json::private::Number": MARKER}),
        json!({"$serde_json::private::RawValue": MARKER}),
        json!([MARKER]),
    ] {
        let mut value = wire("MODEL_TURN", "LEASED");
        value["lease_generation"] = invalid;
        let raw = serde_json::to_string(&value).unwrap();
        for error in [
            serde_json::from_str::<TaskStepDraft>(&raw).unwrap_err(),
            serde_json::from_value::<TaskStepDraft>(value.clone()).unwrap_err(),
            serde_json::from_str::<TaskStep>(&raw).unwrap_err(),
            serde_json::from_value::<TaskStep>(value).unwrap_err(),
        ] {
            assert!(!error.to_string().contains(MARKER), "{error}");
            assert!(!format!("{error:?}").contains(MARKER), "{error:?}");
        }
    }
}

#[test]
fn generation_typed_decode_refuses_objects_and_non_numbers_without_changing_p1_fields() {
    for invalid in [
        json!(1.5),
        json!(0.0),
        json!(-1.0),
        json!(4294967296.0),
        json!(u64::MAX),
        json!(1e100),
        json!("1"),
        json!(true),
        json!([]),
        json!({"$serde_json::private::Number": "1.0"}),
        json!({"$serde_json::private::RawValue": "1.0"}),
    ] {
        let mut value = wire("MODEL_TURN", "LEASED");
        value["lease_generation"] = invalid;
        let raw = serde_json::to_string(&value).unwrap();
        assert!(
            serde_json::from_str::<TaskStepDraft>(&raw).is_err(),
            "{raw}"
        );
        assert!(
            serde_json::from_value::<TaskStepDraft>(value.clone()).is_err(),
            "{value}"
        );
        assert!(serde_json::from_str::<TaskStep>(&raw).is_err(), "{raw}");
        assert!(serde_json::from_value::<TaskStep>(value).is_err());
    }
    for field in ["sequence", "attempt"] {
        let mut value = wire("MODEL_TURN", "LEASED");
        value[field] = json!(1.0);
        assert!(serde_json::from_value::<TaskStep>(value.clone()).is_err());
        assert!(serde_json::from_str::<TaskStep>(&serde_json::to_string(&value).unwrap()).is_err());
    }
}

#[test]
fn task_step_decode_errors_never_echo_rejected_payloads_or_raw_keys() {
    const MARKER: &str = "sk-proj-private-marker";
    let mut cases = Vec::new();
    for field in [
        "lease_generation",
        "sequence",
        "attempt",
        "kind",
        "status",
        "step_id",
        "started_at",
    ] {
        let mut value = wire("MODEL_TURN", "PLANNED");
        value[field] = json!(MARKER);
        cases.push(value);
    }
    let mut bad_type = wire("MODEL_TURN", "PLANNED");
    bad_type["input_digest"] = json!({MARKER: [MARKER]});
    cases.push(bad_type);
    for field in ["kind", "retryable", "host_action", "code"] {
        let mut value = wire("MODEL_TURN", "FAILED");
        value["error"][field] = json!(MARKER);
        cases.push(value);
    }
    let mut nested_key = wire("MODEL_TURN", "FAILED");
    nested_key["error"][MARKER] = json!(MARKER);
    cases.push(nested_key);
    let mut receipt = wire("CAPABILITY", "SUCCEEDED");
    receipt["side_effect_receipt"] = supplied("side_effect_receipt");
    receipt["side_effect_receipt"]["replay_safe"] = json!(MARKER);
    cases.push(receipt);
    cases.push(json!(MARKER));
    let mut failures = Vec::new();
    for value in cases {
        let raw = serde_json::to_string(&value).unwrap();
        for (path, error) in [
            ("raw", serde_json::from_str::<TaskStep>(&raw).unwrap_err()),
            (
                "value",
                serde_json::from_value::<TaskStep>(value.clone()).unwrap_err(),
            ),
        ] {
            let display = error.to_string();
            let debug = format!("{error:?}");
            if display.contains(MARKER) || debug.contains(MARKER) {
                failures.push(format!("{path}: {display}; {debug}"));
            }
            if display.len() >= 256 || debug.len() >= 512 {
                failures.push(format!("{path}: unbounded diagnostics: {display}; {debug}"));
            }
        }
    }
    // Duplicate known members and malformed JSON fail before presence validation too.
    for raw in [
        format!("{{\"{MARKER}\":{{\"lease_generation\":\"{MARKER}\"}}"),
        format!("{{\"sequence\":0,\"sequence\":\"{MARKER}\"}}"),
    ] {
        let error = serde_json::from_str::<TaskStep>(&raw).unwrap_err();
        assert!(!error.to_string().contains(MARKER));
        assert!(!format!("{error:?}").contains(MARKER));
    }
    assert!(failures.is_empty(), "payload-bearing errors: {failures:#?}");
}

#[test]
fn unknown_status_round_trips_without_a_known_lifecycle_matrix_but_retains_kind_invariants() {
    for kind in KINDS {
        let mut value = wire(kind, "FUTURE_STATUS");
        value["attempt"] = json!(0);
        for field in [
            "result_digest",
            "started_at",
            "completed_at",
            "lease_owner",
            "lease_expires_at",
            "lease_generation",
            "error",
        ] {
            value.as_object_mut().expect("object").remove(field);
        }
        value["future_member"] = json!({"opaque": true});
        let step: TaskStep = serde_json::from_value(value.clone()).expect("open status");
        assert_eq!(serde_json::to_value(step).expect("serialize"), value);
        for field in [
            "idempotency_key",
            "provider_id",
            "capability_id",
            "capability_version",
        ] {
            let mut wrong = value.clone();
            if capability_shaped(kind) {
                wrong.as_object_mut().expect("object").remove(field);
            } else {
                wrong[field] = supplied(field);
            }
            both_paths(wrong, false);
        }
        let mut receipt = value.clone();
        receipt["side_effect_receipt"] = supplied("side_effect_receipt");
        both_paths(receipt, capability_shaped(kind));
        value["started_at"] = json!("not a timestamp");
        assert!(serde_json::from_value::<TaskStep>(value).is_err());
    }
    for status in ["future", "FUTURE STATUS", "", "_FUTURE"] {
        let mut value = wire("MODEL_TURN", "PLANNED");
        value["status"] = json!(status);
        assert!(serde_json::from_value::<TaskStep>(value).is_err());
    }
}

#[test]
fn extensions_cannot_duplicate_any_known_member_even_an_omitted_one() {
    let base: TaskStepDraft = serde_json::from_value(wire("MODEL_TURN", "PLANNED")).expect("draft");
    for field in [
        "step_id",
        "task_id",
        "sequence",
        "kind",
        "status",
        "attempt",
        "input_digest",
    ]
    .into_iter()
    .chain(OPTIONAL)
    {
        let mut draft = base.clone();
        draft.extensions.insert(field.to_owned(), Value::Null);
        assert_eq!(
            TaskStep::new(draft),
            Err(ProtocolError::ContractViolation {
                rule: ContractRule::StepReservedExtensionKey
            })
        );
    }
    let mut draft = base;
    draft
        .extensions
        .insert("plan_revision".to_owned(), json!(3));
    let checked = TaskStep::new(draft.clone()).expect("unknown extension retained");
    draft.attempt = 17;
    draft
        .extensions
        .insert("started_at".to_owned(), json!(TIME));
    assert_eq!(
        checked.attempt, 0,
        "the draft does not alias checked fields"
    );
    let value = serde_json::to_value(checked).expect("serialize");
    assert_eq!(value["plan_revision"], json!(3));
    assert!(value.get("started_at").is_none());
}

fn envelope(surface: &str, major: &str) -> Envelope<Value> {
    serde_json::from_value(json!({"envelope_version": major, "surface": surface,
        "message_id": "evt_01JQ8ZB7H2XKM9P4QW7NRT5YCD", "issued_at": TIME,
        "data_class": "PERSONAL", "payload": {}}))
    .expect("envelope shape")
}

#[test]
fn registry_dispatch_checks_both_axes_and_the_expected_surface() {
    assert_eq!(WireSurface::ACTION, "serea.action/2");
    assert_eq!(WireSurface::TASK, "serea.task/2");
    assert_eq!(WireSurface::SCHEDULER, "serea.scheduler/1");
    let published = [
        "serea.action/2",
        "serea.task/2",
        "serea.model/1",
        "serea.policy/1",
        "serea.approval/1",
        "serea.event/1",
        "serea.device/1",
        "serea.goallatch/1",
        "serea.data/1",
        "serea.bounds/1",
        "serea.scheduler/1",
    ];
    let mut registered = WireSurface::ALL.to_vec();
    registered.sort_unstable();
    let mut expected = published.to_vec();
    expected.sort_unstable();
    assert_eq!(registered, expected, "published eleven-surface registry");
    for surface in published {
        let message = envelope(surface, "1");
        assert!(message.surface.is_supported());
        assert!(message.require_supported_surface().is_ok());
        assert!(message.require_expected_surface(surface).is_ok());
        let other = if surface == WireSurface::TASK {
            WireSurface::ACTION
        } else {
            WireSurface::TASK
        };
        assert_eq!(
            message.require_expected_surface(other),
            Err(ProtocolError::ContractViolation {
                rule: ContractRule::UnexpectedWireSurface
            })
        );
        assert_eq!(
            envelope(surface, "2").require_expected_surface(surface),
            Err(ProtocolError::ContractViolation {
                rule: ContractRule::UnsupportedEnvelopeMajor
            })
        );
    }
    for unsupported in [
        "serea.action/1",
        "serea.task/1",
        "serea.action/3",
        "serea.task/3",
        "serea.event/2",
        "serea.model/2",
        "serea.scheduler/2",
        "serea.unknown/1",
        "serea.unknown/2",
        "serea.action/99999999999999",
    ] {
        let message = envelope(unsupported, "1");
        assert!(!message.surface.is_supported());
        assert_eq!(
            message.require_expected_surface(unsupported),
            Err(ProtocolError::ContractViolation {
                rule: ContractRule::UnsupportedWireSurfaceMajor
            })
        );
    }
}

fn field_accepts(category: TextCategory, value: &str) -> Vec<bool> {
    match category {
        TextCategory::Opaque => vec![
            ActorId::new(value).is_ok(),
            LeaseOwner::new(value).is_ok(),
            ProviderReference::new(value).is_ok(),
        ],
        TextCategory::Label => vec![
            TaskTitle::new(value).is_ok(),
            DescriptorTitle::new(value).is_ok(),
            EffectSummary::new(value).is_ok(),
            PlainSummary::new(value).is_ok(),
        ],
        TextCategory::Prose => vec![
            ErrorMessage::new(value).is_ok(),
            DescriptorDescription::new(value).is_ok(),
        ],
    }
}

#[test]
fn every_text_field_and_generated_schema_fragment_follow_the_pinned_corpus() {
    use TextCategory::{Label as L, Opaque as O, Prose as P};
    let mut corpus: Vec<(String, [bool; 3])> = vec![
        ("".into(), [false; 3]),
        ("ordinary".into(), [true; 3]),
        ("worker-1".into(), [true; 3]),
        ("session-42.worker".into(), [true; 3]),
        ("x".into(), [true; 3]),
        ("w".into(), [true; 3]),
        ("provider:handle/1234".into(), [true; 3]),
        ("非ASCII参照".into(), [true; 3]),
        ("a".repeat(5000), [true; 3]),
        ("goallatch.goal.run".into(), [true; 3]),
        ("p.r.list".into(), [true; 3]),
        ("calendar.events.future".into(), [true; 3]),
        ("calendar.events.list".into(), [false, true, true]),
        ("goallatch1.goal.run".into(), [false, true, true]),
        ("goallatch_foo.goal.run".into(), [false, true, true]),
        (KEY.into(), [false, true, true]),
        (DIGEST.into(), [false, true, true]),
        (format!("idk:{}", "a".repeat(64)), [true; 3]),
        (format!("sha256_{}", "a".repeat(64)), [true; 3]),
        (format!("idk_{}", "A".repeat(64)), [true; 3]),
        (format!("sha256:{}", "a".repeat(63)), [true; 3]),
        ("x\u{feff}y".into(), [true; 3]),
        ("\u{feff}x".into(), [true; 3]),
        ("\u{200b}x".into(), [true; 3]),
    ];
    for prefix in [
        "tsk_", "stp_", "apr_", "grt_", "req_", "evt_", "dev_", "sch_", "prop_", "rcp_", "ses_",
    ] {
        corpus.push((
            format!("{prefix}01JQ8Z9K3M7QWXR4V2T6YH0BNA"),
            [false, true, true],
        ));
        corpus.push((format!("{prefix}81JQ8Z9K3M7QWXR4V2T6YH0BNA"), [true; 3]));
        corpus.push((format!("{prefix}01JQ8Z9K3M7QWXR4V2T6YH0BNI"), [true; 3]));
        corpus.push((
            format!("before:{prefix}01JQ8Z9K3M7QWXR4V2T6YH0BNA"),
            [true; 3],
        ));
        corpus.push((format!("{prefix}01JQ8Z9K3M7QWXR4V2T6YH0BNAX"), [true; 3]));
    }
    for code in (0..=0x1f).chain(0x7f..=0x9f).chain([0x2028, 0x2029]) {
        let c = char::from_u32(code).expect("scalar");
        corpus.push((
            format!("x{c}y"),
            [false, false, matches!(code, 9 | 10 | 0x2028 | 0x2029)],
        ));
    }
    for code in [
        9, 10, 11, 12, 13, 0x20, 0x85, 0xa0, 0x1680, 0x2000, 0x2001, 0x2002, 0x2003, 0x2004,
        0x2005, 0x2006, 0x2007, 0x2008, 0x2009, 0x200a, 0x2028, 0x2029, 0x202f, 0x205f, 0x3000,
    ] {
        let c = char::from_u32(code).expect("scalar");
        corpus.push((format!("{c}x"), [false; 3]));
        corpus.push((format!("x{c}"), [false; 3]));
        corpus.push((c.to_string(), [false; 3]));
    }
    for (index, category) in [O, L, P].into_iter().enumerate() {
        let fragment = category.schema_fragment();
        assert_eq!(fragment["pattern"], text_pattern(category));
        let schema = jsonschema::options()
            .with_draft(jsonschema::Draft::Draft202012)
            .build(&fragment)
            .expect("generated fragment compiles");
        for (value, expected) in &corpus {
            assert_eq!(
                category.accepts(value),
                expected[index],
                "category {category:?}: {value:?}"
            );
            for result in field_accepts(category, value) {
                assert_eq!(
                    result, expected[index],
                    "field category {category:?}: {value:?}"
                );
            }
            assert_eq!(
                schema.is_valid(&json!(value)),
                expected[index],
                "schema category {category:?}: {value:?}"
            );
        }
    }
}

#[test]
fn generator_sources_the_frozen_prefix_and_verb_tables() {
    assert_eq!(
        ULID_PREFIXES,
        [
            TaskId::PREFIX,
            StepId::PREFIX,
            ApprovalId::PREFIX,
            GrantId::PREFIX,
            RequestId::PREFIX,
            EventId::PREFIX,
            DeviceId::PREFIX,
            ScheduleId::PREFIX,
            ProposalId::PREFIX,
            ReceiptId::PREFIX,
            SessionId::PREFIX
        ]
    );
    assert_eq!(
        ULID_PREFIXES,
        [
            "tsk_", "stp_", "apr_", "grt_", "req_", "evt_", "dev_", "sch_", "prop_", "rcp_", "ses_"
        ]
    );
    assert_eq!(
        CAPABILITY_VERBS,
        [
            "list", "read", "search", "open", "control", "write", "create", "send", "delete",
            "start", "status", "run", "cancel", "result"
        ]
    );
    let pattern = text_pattern(TextCategory::Opaque);
    assert!(pattern.contains(&ULID_PREFIXES.join("|")));
    assert!(pattern.contains(&CAPABILITY_VERBS.join("|")));
    assert!(pattern.contains("[0-7][0-9A-HJKMNP-TV-Z]{25}"));
    assert!(pattern.contains(IdempotencyKey::PREFIX));
    assert!(pattern.contains(Digest::PREFIX));
}

#[test]
fn every_unicode_scalar_has_the_independently_expected_text_verdict() {
    // Independent numerical predicates, not the production Unicode predicate.
    for code in 0..=0x10ffff {
        let Some(c) = char::from_u32(code) else {
            continue;
        };
        let whitespace = matches!(code, 9..=13 | 0x20 | 0x85 | 0xa0 | 0x1680 | 0x2000..=0x200a | 0x2028 | 0x2029 | 0x202f | 0x205f | 0x3000);
        let control = code <= 31 || (127..=159).contains(&code);
        let line_separator = matches!(code, 0x2028 | 0x2029);
        let values = [format!("{c}x"), format!("x{c}y"), format!("x{c}")];
        for category in [
            TextCategory::Opaque,
            TextCategory::Label,
            TextCategory::Prose,
        ] {
            let interior = match category {
                TextCategory::Opaque | TextCategory::Label => !control && !line_separator,
                TextCategory::Prose => !control || matches!(code, 9 | 10),
            };
            for (position, value) in values.iter().enumerate() {
                let expected = interior && (position == 1 || !whitespace);
                assert_eq!(
                    category.accepts(value),
                    expected,
                    "U+{code:04X}, {category:?}, position {position}"
                );
            }
        }
    }
}
