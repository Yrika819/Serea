use super::structured::{
    StructuredValidationError, validate_json_schema, validate_structured_response,
};
use serde_json::{Value, json};

#[test]
fn valid_output_is_parsed_and_schema_checked() {
    let schema = json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "type": "object",
        "properties": {"count": {"type": "integer"}},
        "required": ["count"],
        "additionalProperties": false
    });
    let value =
        validate_structured_response(&schema, r#"{"count":3}"#).unwrap_or_else(|_| unreachable!());
    assert_eq!(value, json!({"count": 3}));
    assert!(matches!(
        validate_structured_response(&schema, r#"{"count":"three"}"#),
        Err(StructuredValidationError::InvalidOutput(_))
    ));
    let numeric_schema = json!({"type":"number"});
    let numeric = validate_structured_response(&numeric_schema, "3.125e2");
    assert!(numeric.is_ok(), "numeric parse result: {numeric:?}");
    assert_eq!(
        numeric
            .unwrap_or_else(|_| unreachable!())
            .as_f64()
            .unwrap_or_else(|| unreachable!()),
        312.5
    );
    assert_eq!(
        validate_structured_response(&numeric_schema, "1.234567890123456789")
            .unwrap_or_else(|_| unreachable!())
            .to_string(),
        "1.234567890123456789"
    );
    let marker_object = json!({"type":"object"});
    assert_eq!(
        validate_structured_response(
            &marker_object,
            r#"{"$serde_json::private::Number":"ordinary property"}"#,
        )
        .unwrap_or_else(|_| unreachable!()),
        json!({"$serde_json::private::Number":"ordinary property"})
    );
}

#[test]
fn duplicate_keys_are_rejected_at_every_object_depth() {
    let schema = json!({"type": "object"});
    for raw in [
        r#"{"same":1,"same":2}"#,
        r#"{"outer":{"same":1,"same":2}}"#,
        r#"{"same":1,"\u0073ame":2}"#,
    ] {
        assert!(matches!(
            validate_structured_response(&schema, raw),
            Err(StructuredValidationError::DuplicateKey)
        ));
    }
}

#[test]
fn host_schemas_fail_closed_before_compilation() {
    assert!(matches!(
        validate_json_schema(&json!({"type": "nonesuch"})),
        Err(StructuredValidationError::InvalidSchema)
    ));
    assert!(matches!(
        validate_json_schema(&json!({"$ref": "https://example.invalid/schema.json"})),
        Err(StructuredValidationError::InvalidSchema)
    ));
    let local_reference = json!({
        "$defs": {"count": {"type": "integer"}},
        "$ref": "#/$defs/count"
    });
    assert_eq!(
        validate_structured_response(&local_reference, "7").unwrap_or_else(|_| unreachable!()),
        json!(7)
    );
    assert!(matches!(
        validate_structured_response(&json!({}), "{not json}"),
        Err(StructuredValidationError::MalformedJson)
    ));
}

#[test]
fn response_size_and_json_depth_are_bounded_before_validation() {
    let schema = json!({});
    let oversized = format!("\"{}\"", "x".repeat(super::MAX_MODEL_RESPONSE_BYTES));
    assert!(matches!(
        validate_structured_response(&schema, &oversized),
        Err(StructuredValidationError::ResponseTooLarge)
    ));

    let nested = format!(
        "{}0{}",
        "[".repeat(super::MAX_MODEL_JSON_DEPTH + 1),
        "]".repeat(super::MAX_MODEL_JSON_DEPTH + 1)
    );
    assert!(matches!(
        validate_structured_response(&schema, &nested),
        Err(StructuredValidationError::TooDeep)
    ));
}

#[test]
fn schema_depth_is_checked_and_validation_diagnostics_do_not_echo_values() {
    let mut deep_schema = json!({});
    for _ in 0..=super::MAX_MODEL_JSON_DEPTH {
        deep_schema = Value::Array(vec![deep_schema]);
    }
    assert!(matches!(
        validate_json_schema(&deep_schema),
        Err(StructuredValidationError::TooDeep)
    ));

    let schema = json!({"type":"object", "additionalProperties":false});
    let raw = r#"{"private_fragment_marker":"must not be echoed"}"#;
    let Err(StructuredValidationError::InvalidOutput(diagnostics)) =
        validate_structured_response(&schema, raw)
    else {
        unreachable!()
    };
    assert!(diagnostics.len() <= super::MAX_MODEL_VALIDATION_ERRORS);
    let encoded = serde_json::to_string(&diagnostics).unwrap_or_else(|_| unreachable!());
    assert!(encoded.len() <= super::MAX_MODEL_VALIDATION_ERROR_BYTES);
    assert!(!encoded.contains("must not be echoed"));

    let required = (0..40).map(|i| format!("p{i}")).collect::<Vec<_>>();
    let many_errors = json!({"type":"object", "required":required});
    let Err(StructuredValidationError::InvalidOutput(diagnostics)) =
        validate_structured_response(&many_errors, "{}")
    else {
        unreachable!()
    };
    assert_eq!(diagnostics.len(), super::MAX_MODEL_VALIDATION_ERRORS);
    let encoded = serde_json::to_vec(&diagnostics).unwrap_or_else(|_| unreachable!());
    assert!(encoded.len() <= super::MAX_MODEL_VALIDATION_ERROR_BYTES);
}
