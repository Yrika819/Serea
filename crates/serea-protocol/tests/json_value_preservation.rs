//! Frozen N3 regression: literal marker objects must remain opaque JSON data,
//! while exact synthetic Number/RawValue transport survives derive flatten.

mod tests {
    use serde::{Deserialize, Serialize};
    use serde_json::{Map, Number, Value, json};
    use serea_protocol::{ActionRequest, Extensions, Trace};

    const NUMBER: &str = "$serde_json::private::Number";
    const RAW: &str = "$serde_json::private::RawValue";

    fn marker(k: &str, v: Value) -> Value {
        let mut m = Map::new();
        m.insert(k.into(), v);
        Value::Object(m)
    }

    fn corpus() -> Vec<Value> {
        let mut values = Vec::new();
        for k in [NUMBER, RAW] {
            for v in [
                json!("1"),
                json!("not-a-number"),
                json!("{\"changed\":true}"),
                json!("\"quoted\""),
                json!("☃"),
                json!(null),
                json!(true),
                json!([1]),
                json!({"inner": 2}),
            ] {
                let single = marker(k, v);
                values.push(single.clone());
                let mut extra = single.clone();
                extra
                    .as_object_mut()
                    .unwrap()
                    .insert("extra".into(), json!(true));
                values.push(extra);
                let mut first = single.clone();
                first
                    .as_object_mut()
                    .unwrap()
                    .insert("!first".into(), json!(true));
                values.push(first);
                values.push(json!({"nested": [single.clone(), {"deeper": single}]}));
            }
        }
        values
    }

    #[test]
    fn literal_marker_objects_raw_owned_and_borrowed_value() {
        for expected in corpus() {
            let raw = serde_json::to_string(&expected).unwrap();
            let parsed: Value = serde_json::from_str(&raw).unwrap();
            assert_eq!(parsed, expected, "{raw}");
            assert_eq!(raw.parse::<Value>().unwrap(), expected);
            assert_eq!(
                serde_json::from_slice::<Value>(raw.as_bytes()).unwrap(),
                expected
            );
            assert_eq!(
                serde_json::from_reader::<_, Value>(raw.as_bytes()).unwrap(),
                expected
            );
            assert_eq!(
                serde_json::from_value::<Value>(expected.clone()).unwrap(),
                expected
            );
            assert_eq!(Value::deserialize(&expected).unwrap(), expected);
            assert_eq!(serde_json::to_string(&parsed).unwrap(), raw);
        }
    }

    #[test]
    fn escaped_literal_keys_and_key_order() {
        for raw in [
            r#"{"\u0024serde_json::private::Number":"1"}"#,
            r#"{"$serde_json::private::Raw\u0056alue":"1"}"#,
            r#"{"$serde_json::private::Number":"1","extra":true}"#,
            r#"{"extra":true,"$serde_json::private::Number":"1"}"#,
            r#"{"$serde_json::private::RawValue":"{\"escaped\\\"quote\":\"☃\"}"}"#,
        ] {
            let value: Value = serde_json::from_str(raw).unwrap();
            assert!(value.is_object(), "{raw}");
            assert_eq!(
                serde_json::from_value::<Value>(value.clone()).unwrap(),
                value
            );
        }
    }

    #[test]
    fn actual_trace_flatten_raw_and_value() {
        for literal in corpus() {
            let expected = json!({"future": literal});
            let raw = serde_json::to_string(&expected).unwrap();
            let from_raw: Trace = serde_json::from_str(&raw).unwrap();
            let from_value: Trace = serde_json::from_value(expected.clone()).unwrap();
            assert_eq!(serde_json::to_value(from_raw).unwrap(), expected);
            assert_eq!(serde_json::to_value(from_value).unwrap(), expected);
        }
    }

    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    struct Inner {
        #[serde(flatten)]
        extensions: Extensions,
    }
    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    struct Outer {
        known: String,
        #[serde(flatten)]
        inner: Inner,
    }

    #[test]
    fn two_flatten_layers_preserve_literals_and_precise_numbers() {
        for literal in corpus() {
            let expected = json!({"known":"k", "future":literal});
            let raw = serde_json::to_string(&expected).unwrap();
            let parsed: Outer = serde_json::from_str(&raw).unwrap();
            assert_eq!(serde_json::to_value(parsed).unwrap(), expected);
            let parsed: Outer = serde_json::from_value(expected.clone()).unwrap();
            assert_eq!(serde_json::to_value(parsed).unwrap(), expected);
        }
        for n in precise_numbers() {
            let raw = format!(r#"{{"known":"k","future":{n}}}"#);
            let expected: Value = serde_json::from_str(&raw).unwrap();
            let parsed: Outer = serde_json::from_str(&raw).unwrap();
            assert_eq!(serde_json::to_value(parsed).unwrap(), expected);
            let parsed: Outer = serde_json::from_value(expected.clone()).unwrap();
            assert_eq!(serde_json::to_value(parsed).unwrap(), expected);
        }
    }

    fn precise_numbers() -> &'static [&'static str] {
        &[
            "1.000000000000000000000000000001",
            "4294967295.000000000000000000000001",
            "18446744073709551616",
            "100000000000000000000",
            "-100000000000000000000",
            "0.1",
            "1.0",
            "340282366920938463463374607431768211456",
            "-340282366920938463463374607431768211457",
            "1e9999999999999999999999999999999999999999",
            "1e-9999999999999999999999999999999999999999",
        ]
    }

    #[test]
    fn genuine_numbers_stay_numbers_through_raw_value_and_flatten() {
        for raw in precise_numbers() {
            let value: Value = serde_json::from_str(raw).unwrap();
            assert!(value.is_number());
            assert_eq!(raw.parse::<Value>().unwrap(), value);
            assert_eq!(
                serde_json::from_slice::<Value>(raw.as_bytes()).unwrap(),
                value
            );
            assert_eq!(
                serde_json::from_reader::<_, Value>(raw.as_bytes()).unwrap(),
                value
            );
            let normalized = value.to_string();
            assert_eq!(
                serde_json::from_value::<Value>(value.clone())
                    .unwrap()
                    .to_string(),
                normalized
            );
            assert_eq!(Value::deserialize(&value).unwrap().to_string(), normalized);
            assert_eq!(
                serde_json::from_str::<Number>(raw).unwrap().to_string(),
                normalized
            );
            assert_eq!(
                serde_json::from_value::<Number>(value.clone())
                    .unwrap()
                    .to_string(),
                normalized
            );
            let expected = json!({"future": value});
            let text = serde_json::to_string(&expected).unwrap();
            let trace: Trace = serde_json::from_str(&text).unwrap();
            assert_eq!(serde_json::to_value(trace).unwrap(), expected);
            let trace: Trace = serde_json::from_value(expected.clone()).unwrap();
            assert_eq!(serde_json::to_value(trace).unwrap(), expected);
        }
    }

    #[derive(Debug, Serialize, Deserialize)]
    struct NumericFlat {
        #[serde(flatten)]
        inner: NumericInner,
    }
    #[derive(Debug, Serialize, Deserialize)]
    struct NumericInner {
        number: Number,
    }

    #[test]
    fn buffered_numberkey_consumers_accept_newtype_provenance() {
        for raw in precise_numbers() {
            let text = format!(r#"{{"number":{raw}}}"#);
            let expected: Value = serde_json::from_str(&text).unwrap();
            let flat: NumericFlat = serde_json::from_str(&text).unwrap();
            assert_eq!(serde_json::to_value(flat).unwrap(), expected);
            let flat: NumericFlat = serde_json::from_value(expected.clone()).unwrap();
            assert_eq!(serde_json::to_value(flat).unwrap(), expected);
        }
    }

    #[test]
    fn explicit_numeric_target_decoding_is_unchanged() {
        let value: Value = serde_json::from_str("18446744073709551616").unwrap();
        assert_eq!(
            serde_json::from_value::<u128>(value).unwrap(),
            18446744073709551616u128
        );
        let value: Value = serde_json::from_str("-18446744073709551616").unwrap();
        assert_eq!(
            serde_json::from_value::<i128>(value).unwrap(),
            -18446744073709551616i128
        );
        let value: Value = serde_json::from_str("0.1").unwrap();
        assert_eq!(serde_json::from_value::<f64>(value).unwrap(), 0.1);
    }

    #[test]
    fn genuine_rawvalue_transport_and_marker_literals() {
        for raw in [
            "1.000000000000000000000000000001",
            "1e999999999999999999999999999",
            r#"{"$serde_json::private::Number":"1"}"#,
            r#"{"$serde_json::private::RawValue":"1"}"#,
        ] {
            let boxed: Box<serde_json::value::RawValue> = serde_json::from_str(raw).unwrap();
            assert_eq!(boxed.get(), raw);
            let expected: Value = serde_json::from_str(raw).unwrap();
            assert_eq!(serde_json::to_value(&boxed).unwrap(), expected);
            assert_eq!(Value::deserialize(&*boxed).unwrap(), expected);
            let via_value: Box<serde_json::value::RawValue> =
                serde_json::from_value(expected.clone()).unwrap();
            assert_eq!(
                serde_json::from_str::<Value>(via_value.get()).unwrap(),
                expected
            );
        }
    }

    fn request(arguments: Value) -> Value {
        json!({
            "request_id":"req_01JQ8ZA4H6NFG8K2M6RTV9XCWB",
            "task_id":"tsk_01JQ8Z9K3M7QWXR4V2T6YH0BNA",
            "step_id":"stp_01JQ8Z9M3R2CVN8H5FWK7PQDSF",
            "capability_id":"calendar.events.list", "capability_version":"1.2.0",
            "arguments":arguments,
            "arguments_digest":"sha256:3b1f0c9e2a7d4e6b8f0a2c4d6e8b0d2f4a6c8e0b2d4f6a8c0e2b4d6f8a0c2e4b",
            "idempotency_key":"idk_9f2c1a7e4b6d0f8a3c5e9b1d7f2a4c6e8b0d3f5a7c9e1b4d6f8a0c2e4b6d8f9a",
            "data_class":"PERSONAL", "requested_by":"MODEL", "deadline_ms":15000
        })
    }

    #[test]
    fn actual_action_arguments_and_schema_validity_preserved() {
        let schema = json!({"type":"object","properties":{"opaque":{"type":"object"}},"required":["opaque"]});
        let validator = jsonschema::validator_for(&schema).unwrap();
        for literal in corpus() {
            let args = json!({"opaque":literal});
            assert!(validator.is_valid(&args));
            // Synthetic host fields are fixture data; this test performs no execution.
            let expected = request(args);
            let raw = serde_json::to_string(&expected).unwrap();
            let parsed: ActionRequest = serde_json::from_str(&raw).unwrap();
            assert!(validator.is_valid(&Value::Object(parsed.arguments.clone())));
            assert_eq!(serde_json::to_value(parsed).unwrap(), expected);
            let parsed: ActionRequest = serde_json::from_value(expected.clone()).unwrap();
            assert!(validator.is_valid(&Value::Object(parsed.arguments.clone())));
            assert_eq!(serde_json::to_value(parsed).unwrap(), expected);
        }
    }

    #[test]
    fn exact_numeric_schema_predicate_still_works() {
        let integer = jsonschema::validator_for(&json!({"type":"integer"})).unwrap();
        let bounded = jsonschema::validator_for(
            &json!({"type":"integer","minimum":1,"maximum":4294967295u64}),
        )
        .unwrap();
        for (raw, integral, in_range) in [
            ("1.0", true, true),
            ("42949672950e-1", true, true),
            ("1.000000000000000000000000000001", false, false),
            ("4294967295.000000000000000000000001", false, false),
            ("1e999999999999999999999999999999", true, false),
            ("1e-999999999999999999999999999999", false, false),
            ("0e-999999999999999999999999999999", true, false),
        ] {
            let v: Value = serde_json::from_str(raw).unwrap();
            assert_eq!(integer.is_valid(&v), integral, "integer {raw}");
            assert_eq!(bounded.is_valid(&v), in_range, "bounded {raw}");
        }
    }
}
