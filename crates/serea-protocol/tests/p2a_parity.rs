//! ADR-0018 wire-only matrix: independent expected outcomes, not agreement alone.
use serde_json::{Value, json};
use serea_protocol::{
    AssistantTask, TaskStep,
    schema::{self, SchemaName},
};

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
const TUPLE: [&str; 4] = [
    "provider_id",
    "capability_id",
    "capability_version",
    "idempotency_key",
];
const OPTIONAL: [&str; 12] = [
    "provider_id",
    "capability_id",
    "capability_version",
    "idempotency_key",
    "result_digest",
    "started_at",
    "completed_at",
    "lease_owner",
    "lease_expires_at",
    "lease_generation",
    "side_effect_receipt",
    "error",
];
const REQUIRED: [&str; 7] = [
    "step_id",
    "task_id",
    "sequence",
    "kind",
    "status",
    "attempt",
    "input_digest",
];
const TIME: &str = "2026-10-01T09:14:22.100Z";
const GENERATION_CASES: &[(&str, bool)] = &[
    ("1", true),
    ("1.0", true),
    ("1e0", true),
    ("10e-1", true),
    ("0.1e1", true),
    ("1.00000000000000000000", true),
    ("42949672950e-1", true),
    ("4294967295", true),
    ("4294967295.0", true),
    ("0.00000000000000000000000000001e29", true),
    ("429496729500000000000000000000e-20", true),
    ("1E+0000000000000000000000000000000000000000", true),
    ("0", false),
    ("0.0", false),
    ("-0", false),
    ("-0.0", false),
    ("-1", false),
    ("-1.0", false),
    ("1.5", false),
    ("1.0000000000000001", false),
    ("1.00000000000000000001", false),
    ("1.00000000000000000001e0", false),
    ("0.99999999999999999999", false),
    ("4294967295.0000001", false),
    ("4294967294.9999999", false),
    ("42949672950000000001e-10", false),
    ("100000000000000000001e-20", false),
    ("1e-1000", false),
    ("4294967296", false),
    ("4294967296.0", false),
    ("18446744073709551615", false),
    ("1e100", false),
    ("1e1000000000", false),
    ("1e-1000000000", false),
    ("1e-9223372036854775808", false),
    ("1.0e-9223372036854775807", false),
    ("1.00e-9223372036854775807", false),
    ("1e-170141183460469231731687303715884105728", false),
    ("1e999999999999999999999999999999999999999999", false),
    ("1e-999999999999999999999999999999999999999999", false),
    ("\"1\"", false),
    ("true", false),
];

fn shaped(kind: &str) -> bool {
    matches!(kind, "CAPABILITY" | "DELEGATE" | "VERIFY")
}
fn wait(kind: &str) -> bool {
    matches!(kind, "WAIT_APPROVAL" | "WAIT_USER" | "WAIT_SCHEDULE")
}
fn terminal(status: &str) -> bool {
    matches!(status, "SUCCEEDED" | "FAILED" | "RECONCILED_ABSENT")
}
fn validator() -> jsonschema::Validator {
    let doc = SchemaName::AssistantTask.json().expect("schema");
    let mut step = doc["$defs"]["step"].clone();
    step["$defs"] = doc["$defs"].clone();
    jsonschema::validator_for(&step).expect("step schema compiles")
}
fn supplied(field: &str) -> Value {
    match field {
        "provider_id" => json!("calendar"),
        "capability_id" => json!("calendar.events.list"),
        "capability_version" => json!("1.2.0"),
        "idempotency_key" => json!(format!("idk_{}", "a".repeat(64))),
        "result_digest" => json!(format!("sha256:{}", "b".repeat(64))),
        "started_at" | "completed_at" | "lease_expires_at" => json!(TIME),
        "lease_owner" => json!("worker-1"),
        "lease_generation" => json!(1),
        "side_effect_receipt" => json!({
            "receipt_id": "rcp_01JQ8ZF4T7KMV2X9NPQ5RD8WCS", "capability_id": "calendar.events.list",
            "idempotency_key": format!("idk_{}", "a".repeat(64)), "effect_summary": "Observed result",
            "provider_reference": "external/123", "observed_at": TIME, "replay_safe": true
        }),
        "error" => {
            json!({"kind": "PROVIDER_ERROR", "code": "FAILED", "message": "Failed attempt", "retryable": false, "host_action": "NONE"})
        }
        _ => panic!("unknown field {field}"),
    }
}
// R = required non-null; N = missing/null only; O = optional validated.
fn cell(kind: &str, status: &str, field: &str) -> char {
    if TUPLE.contains(&field) {
        return if shaped(kind) { 'R' } else { 'N' };
    }
    match field {
        "result_digest" => match status {
            "SUCCEEDED" => 'R',
            "FAILED" | "RECONCILED_ABSENT" => 'O',
            _ => 'N',
        },
        "started_at" => {
            if matches!(status, "PLANNED" | "LEASED") {
                'N'
            } else {
                'R'
            }
        }
        "completed_at" => {
            if terminal(status) {
                'R'
            } else {
                'N'
            }
        }
        "lease_owner" | "lease_expires_at" => {
            if matches!(status, "LEASED" | "EXECUTING") {
                'R'
            } else {
                'N'
            }
        }
        "lease_generation" => {
            if status == "PLANNED" {
                'N'
            } else {
                'R'
            }
        }
        "side_effect_receipt" => {
            if status == "SUCCEEDED" && shaped(kind) {
                'O'
            } else {
                'N'
            }
        }
        "error" => {
            if status == "FAILED" {
                'R'
            } else {
                'N'
            }
        }
        _ => panic!("unknown field"),
    }
}
fn step(kind: &str, status: &str) -> Value {
    let mut value = json!({
        "step_id": "stp_01JQ8Z9M3R2CVN8H5FWK7PQDSF", "task_id": "tsk_01JQ8Z9K3M7QWXR4V2T6YH0BNA",
        "sequence": 0, "kind": kind, "status": status, "attempt": if status == "PLANNED" {0} else {1},
        "input_digest": format!("sha256:{}", "a".repeat(64))
    });
    for field in OPTIONAL {
        if cell(kind, status, field) == 'R' {
            value[field] = supplied(field);
        }
    }
    value
}
fn check(validator: &jsonschema::Validator, value: &Value, expected: bool, context: &str) {
    assert_eq!(
        validator.is_valid(value),
        expected,
        "schema: {context}: {value}"
    );
    assert_eq!(
        serde_json::from_value::<TaskStep>(value.clone()).is_ok(),
        expected,
        "Rust: {context}: {value}"
    );
}
#[test]
fn every_kind_status_has_51_positive_and_five_waiting_negative_cells() {
    let validator = validator();
    let mut counts = [0, 0];
    for kind in KINDS {
        for status in STATUSES {
            let expected = status != "WAITING" || wait(kind);
            counts[usize::from(!expected)] += 1;
            check(
                &validator,
                &step(kind, status),
                expected,
                &format!("{kind}/{status}"),
            );
        }
    }
    assert_eq!(counts, [51, 5]);
}
#[test]
fn every_presence_cell_checks_missing_null_and_supplied_in_both_directions() {
    let validator = validator();
    for kind in KINDS {
        for status in STATUSES {
            if status == "WAITING" && !wait(kind) {
                continue;
            }
            let base = step(kind, status);
            for field in OPTIONAL {
                for form in 0..3 {
                    let mut value = base.clone();
                    match form {
                        0 => {
                            value.as_object_mut().unwrap().remove(field);
                        }
                        1 => value[field] = Value::Null,
                        _ => value[field] = supplied(field),
                    }
                    let expected = match cell(kind, status, field) {
                        'R' => form == 2,
                        'N' => form != 2,
                        _ => true,
                    };
                    check(
                        &validator,
                        &value,
                        expected,
                        &format!("{kind}/{status}/{field}/{form}"),
                    );
                }
            }
            for field in REQUIRED {
                for null in [false, true] {
                    let mut value = base.clone();
                    if null {
                        value[field] = Value::Null;
                    } else {
                        value.as_object_mut().unwrap().remove(field);
                    }
                    check(
                        &validator,
                        &value,
                        false,
                        &format!("required {kind}/{status}/{field}/{null}"),
                    );
                }
            }
            for attempt in [0, 1, u32::MAX] {
                let mut value = base.clone();
                value["attempt"] = json!(attempt);
                check(
                    &validator,
                    &value,
                    (status == "PLANNED") == (attempt == 0),
                    "attempt boundary",
                );
            }
        }
    }
}
#[test]
fn kind_tuple_rejects_every_partial_subset_even_for_unknown_status() {
    let validator = validator();
    for kind in KINDS {
        for status in ["PLANNED", "FUTURE_STATE"] {
            for mask in 0..16 {
                for null in [false, true] {
                    let mut value = step(kind, "PLANNED");
                    value["status"] = json!(status);
                    for (index, field) in TUPLE.iter().enumerate() {
                        value.as_object_mut().unwrap().remove(*field);
                        if mask & (1 << index) != 0 {
                            value[*field] = supplied(field);
                        } else if null {
                            value[*field] = Value::Null;
                        }
                    }
                    check(
                        &validator,
                        &value,
                        if shaped(kind) { mask == 15 } else { mask == 0 },
                        &format!("tuple {kind}/{status}/{mask}/{null}"),
                    );
                }
            }
        }
    }
}
#[test]
fn unknown_status_preserves_real_extensions_without_applying_known_clauses() {
    let validator = validator();
    for kind in KINDS {
        let mut value = step(kind, "PLANNED");
        value["status"] = json!("FUTURE_STATE");
        value["attempt"] = json!(17);
        value["future_extension"] =
            json!({"array": [null, {"opaque": "preserved"}], "number": 123});
        for field in OPTIONAL {
            if !TUPLE.contains(&field) && (field != "side_effect_receipt" || shaped(kind)) {
                value[field] = supplied(field);
            }
        }
        check(&validator, &value, true, "unknown status supplied values");
        let parsed: TaskStep = serde_json::from_value(value.clone()).unwrap();
        let output = serde_json::to_value(parsed).unwrap();
        assert_eq!(output["future_extension"], value["future_extension"]);
        assert_eq!(output["status"], "FUTURE_STATE");
        for status in ["future_state", "1FUTURE", "FUTURE STATE", "FUTURE\n", ""] {
            let mut bad = value.clone();
            bad["status"] = json!(status);
            check(&validator, &bad, false, "malformed open code");
        }
    }
}
#[test]
fn receipts_follow_kind_security_even_when_status_is_unknown() {
    let validator = validator();
    // Owner-authorized security rule: host-internal kinds cannot acquire
    // external-action semantics, even via a future well-formed status.
    for (kinds, external) in [
        (
            &[
                "MODEL_TURN",
                "WAIT_APPROVAL",
                "WAIT_USER",
                "WAIT_SCHEDULE",
                "NOTIFY",
            ][..],
            false,
        ),
        (&["CAPABILITY", "DELEGATE", "VERIFY"][..], true),
    ] {
        for kind in kinds {
            for status in STATUSES.into_iter().chain(["FUTURE_STATE"]) {
                let known = status != "FUTURE_STATE";
                let mut base = step(kind, if known { status } else { "PLANNED" });
                base["status"] = json!(status);
                let valid_status = status != "WAITING" || wait(kind);
                for form in 0..3 {
                    let mut value = base.clone();
                    match form {
                        0 => {
                            value.as_object_mut().unwrap().remove("side_effect_receipt");
                        }
                        1 => value["side_effect_receipt"] = Value::Null,
                        _ => value["side_effect_receipt"] = supplied("side_effect_receipt"),
                    }
                    let receipt_allowed = external && (status == "SUCCEEDED" || !known);
                    check(
                        &validator,
                        &value,
                        valid_status && (form != 2 || receipt_allowed),
                        &format!("receipt security {kind}/{status}/{form}"),
                    );
                }
            }
        }
    }
}
#[test]
fn generation_and_integer_boundaries_never_wrap_or_admit_wire_zero() {
    let validator = validator();
    for status in [
        "PLANNED",
        "LEASED",
        "EXECUTING",
        "WAITING",
        "SUCCEEDED",
        "FAILED",
        "RECONCILED_ABSENT",
        "FUTURE_STATE",
    ] {
        let mut base = step(
            "WAIT_USER",
            if status == "FUTURE_STATE" {
                "PLANNED"
            } else {
                status
            },
        );
        base["status"] = json!(status);
        for number in [
            json!(-1),
            json!(0),
            json!(1),
            json!(u32::MAX),
            json!(4294967296_u64),
            json!(u64::MAX),
            json!(1.5),
            json!("1"),
            json!(true),
        ] {
            let mut value = base.clone();
            value["lease_generation"] = number.clone();
            let positive = number
                .as_u64()
                .is_some_and(|n| (1..=u64::from(u32::MAX)).contains(&n));
            check(
                &validator,
                &value,
                status != "PLANNED" && positive,
                "generation boundary",
            );
        }
    }
    for field in ["sequence", "attempt"] {
        for number in [
            json!(-1),
            json!(4294967296_u64),
            json!(u64::MAX),
            json!(0.5),
            json!("1"),
            json!(false),
        ] {
            let mut value = step("MODEL_TURN", "SUCCEEDED");
            value[field] = number;
            check(&validator, &value, false, "u32 boundary");
        }
    }
}
#[test]
fn generation_raw_number_spellings_match_schema_integer_and_presence_domains() {
    let validator = validator();
    for status in STATUSES.into_iter().chain(["FUTURE_STATE"]) {
        let mut base = step(
            "WAIT_USER",
            if status == "FUTURE_STATE" {
                "PLANNED"
            } else {
                status
            },
        );
        base["status"] = json!(status);
        for &(number, positive) in GENERATION_CASES {
            let mut value = base.clone();
            value["lease_generation"] = json!("RAW_GENERATION");
            let raw = serde_json::to_string(&value)
                .unwrap()
                .replace("\"RAW_GENERATION\"", number);
            let value: Value = serde_json::from_str(&raw).unwrap();
            let expected = positive && status != "PLANNED";
            check(&validator, &value, expected, &format!("{status}/{number}"));
            assert_eq!(
                serde_json::from_str::<TaskStep>(&raw).is_ok(),
                expected,
                "raw {status}/{number}"
            );
        }
        // Preserve the literal object shape: arbitrary_precision's Value parser
        // itself recognizes this private marker, unlike the typed field decoder.
        let mut object = base.clone();
        object["lease_generation"] = json!({"$serde_json::private::Number": "1.0"});
        check(
            &validator,
            &object,
            false,
            "generation is not a marker object",
        );
        assert!(
            serde_json::from_str::<TaskStep>(&serde_json::to_string(&object).unwrap()).is_err()
        );
        for null in [false, true] {
            let mut value = base.clone();
            if null {
                value["lease_generation"] = Value::Null;
            } else {
                value.as_object_mut().unwrap().remove("lease_generation");
            }
            check(
                &validator,
                &value,
                matches!(status, "PLANNED" | "FUTURE_STATE"),
                "generation missing/null",
            );
        }
    }
}

fn task_with_generation(number: &str) -> String {
    let mut value = json!({
        "task_id": "tsk_01JQ8Z9K3M7QWXR4V2T6YH0BNA",
        "kind": "USER_REQUEST", "title": "Generation parity", "state": "EXECUTING",
        "origin": {"kind": "USER_MESSAGE"}, "data_class": "PERSONAL",
        "policy_class": "OBSERVE", "created_at": TIME, "updated_at": TIME,
        "attempt_budget": {"max_model_calls": 12, "max_tool_calls": 24, "max_attempts_per_step": 3},
        "steps": [step("MODEL_TURN", "LEASED")]
    });
    value["steps"][0]["lease_generation"] = json!("RAW_GENERATION");
    serde_json::to_string(&value)
        .unwrap()
        .replace("\"RAW_GENERATION\"", number)
}

#[test]
fn generation_exact_domain_reaches_direct_schema_validator_and_enclosing_task() {
    let validator = schema::validator(SchemaName::AssistantTask).unwrap();
    for &(number, expected) in GENERATION_CASES {
        let raw = task_with_generation(number);
        let value: Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(
            validator.is_valid(&value),
            expected,
            "direct schema/{number}"
        );
        assert_eq!(
            schema::validate(SchemaName::AssistantTask, &value).is_ok(),
            expected,
            "schema boundary/{number}"
        );
        assert_eq!(
            serde_json::from_str::<AssistantTask>(&raw).is_ok(),
            expected,
            "raw task/{number}"
        );
        assert_eq!(
            serde_json::from_value::<AssistantTask>(value).is_ok(),
            expected,
            "value task/{number}"
        );
    }
}

#[test]
fn direct_integer_schema_classifies_extreme_decimal_tokens_exactly() {
    let validator = jsonschema::validator_for(&json!({"type": "integer"})).unwrap();
    for (number, expected) in [
        ("1.0", true),
        ("10e-1", true),
        ("0.1e1", true),
        ("100e-2", true),
        ("100e-3", false),
        ("-1.5", false),
        ("1.00000000000000000001", false),
        ("1e-1000001", false),
        ("1e-9223372036854775808", false),
        ("1.0e-9223372036854775807", false),
        ("1.00e-9223372036854775807", false),
        ("-1.00e-9223372036854775807", false),
        ("1e-170141183460469231731687303715884105728", false),
        ("1e-999999999999999999999999999999999999999999", false),
        ("-1e-999999999999999999999999999999999999999999", false),
        ("1e+999999999999999999999999999999999999999999", true),
        ("-1e+999999999999999999999999999999999999999999", true),
        ("0.00e-9223372036854775807", true),
        ("-0.0e-9223372036854775808", true),
        ("0e-170141183460469231731687303715884105728", true),
        ("0e-999999999999999999999999999999999999999999", true),
        ("-0e+999999999999999999999999999999999999999999", true),
        ("1E+0000000000000000000000000000000000000000", true),
    ] {
        let value: Value = serde_json::from_str(number).unwrap();
        assert_eq!(validator.is_valid(&value), expected, "is_valid/{number}");
        assert_eq!(
            validator.validate(&value).is_ok(),
            expected,
            "validate/{number}"
        );
        assert_eq!(
            validator.iter_errors(&value).next().is_none(),
            expected,
            "iter_errors/{number}"
        );
    }
}

#[test]
fn generation_decimal_shifts_match_an_independent_integer_arithmetic_oracle() {
    let validator = validator();
    for coefficient in [
        0_u128,
        1,
        9,
        10,
        11,
        100,
        101,
        4294967294,
        4294967295,
        4294967296,
        42949672950,
        u128::from(u64::MAX),
    ] {
        for exponent in -20_i32..=10 {
            let integer = if exponent >= 0 {
                Some(coefficient * 10_u128.pow(exponent.unsigned_abs()))
            } else {
                let denominator = 10_u128.pow(exponent.unsigned_abs());
                (coefficient % denominator == 0).then_some(coefficient / denominator)
            };
            let expected = integer.filter(|n| (1..=u128::from(u32::MAX)).contains(n));
            for number in [
                format!("{coefficient}e{exponent}"),
                format!("{coefficient}.00E{exponent:+}"),
            ] {
                let raw = task_with_generation(&number);
                let value: Value = serde_json::from_str(&raw).unwrap();
                let step_value = &value["steps"][0];
                check(&validator, step_value, expected.is_some(), &number);
                let decoded = serde_json::from_str::<AssistantTask>(&raw);
                assert_eq!(decoded.is_ok(), expected.is_some(), "raw task/{number}");
                if let Some(expected) = expected {
                    assert_eq!(
                        decoded.unwrap().steps[0].lease_generation,
                        Some(u32::try_from(expected).unwrap()),
                        "{number}"
                    );
                }
            }
        }
    }
}

#[test]
fn every_supplied_optional_scalar_is_validated_even_on_unknown_status() {
    let validator = validator();
    for field in OPTIONAL {
        let kind = if TUPLE.contains(&field) || field == "side_effect_receipt" {
            "CAPABILITY"
        } else {
            "MODEL_TURN"
        };
        let mut base = step(kind, "PLANNED");
        base["status"] = json!("FUTURE_STATE");
        for invalid in [
            json!(false),
            json!([]),
            json!({"credential": "synthetic"}),
            json!(""),
        ] {
            let mut value = base.clone();
            value[field] = invalid;
            check(&validator, &value, false, &format!("invalid {field}"));
        }
    }
}
#[test]
fn none_is_accepted_as_missing_or_null_and_serialized_by_omission() {
    let validator = validator();
    for kind in KINDS {
        for status in STATUSES {
            if status == "WAITING" && !wait(kind) {
                continue;
            }
            let mut value = step(kind, status);
            for field in OPTIONAL {
                if cell(kind, status, field) != 'R' {
                    value[field] = Value::Null;
                }
            }
            check(&validator, &value, true, "explicit nulls");
            let parsed: TaskStep = serde_json::from_value(value).unwrap();
            let output = serde_json::to_value(parsed).unwrap();
            for field in OPTIONAL {
                if cell(kind, status, field) != 'R' {
                    assert!(
                        output.get(field).is_none(),
                        "None must omit {kind}/{status}/{field}"
                    );
                }
            }
            assert!(validator.is_valid(&output));
        }
    }
}
#[test]
fn schema_has_exactly_seven_unconditional_step_fields_and_mixed_major_titles() {
    let doc = SchemaName::AssistantTask.json().unwrap();
    let mut actual: Vec<_> = doc["$defs"]["step"]["required"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    actual.sort_unstable();
    let mut expected = REQUIRED;
    expected.sort_unstable();
    assert_eq!(actual, expected);
    for (name, title) in [
        (SchemaName::AssistantTask, "serea.task/2 AssistantTask"),
        (SchemaName::ActionRequest, "serea.action/2 ActionRequest"),
        (SchemaName::ActionResult, "serea.action/2 ActionResult"),
        (SchemaName::Event, "serea.event/1 SereaEvent"),
    ] {
        assert_eq!(name.json().unwrap()["title"], title);
    }
}
