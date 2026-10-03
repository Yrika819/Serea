//! Deterministic generated properties and hostile-text parser regressions.
use serea_protocol::{
    CanonicalJsonError, CapabilityId, MAX_INSTANCE_DEPTH, SemVer, StepId, TaskId, canonicalize,
    derive_idempotency_key, digest_of,
};

fn derive(arguments: &str) -> Result<serea_protocol::IdempotencyKey, CanonicalJsonError> {
    derive_idempotency_key(
        &TaskId::new("tsk_01JQ8Z9K3M7QWXR4V2T6YH0BNA").unwrap(),
        &StepId::new("stp_01JQ8Z9M3R2CVN8H5FWK7PQDSF").unwrap(),
        &CapabilityId::new("pp.rr.list").unwrap(),
        &SemVer::new("1.2.3-alpha.1+build.9").unwrap(),
        arguments,
    )
}

#[test]
fn idempotence_and_semantic_preservation_generated_corpus() {
    for n in -128..=128 {
        for root in [
            format!("{n}"),
            format!(r#" {{ "z": [null, true, false, {n}], "a": {{"é":"/\\\n", "n":{n}}} }} "#),
            format!(r#"["😀",{{"value":{n}}},[]]"#),
        ] {
            let bytes = canonicalize(&root).unwrap();
            let text = std::str::from_utf8(&bytes).unwrap();
            assert_eq!(canonicalize(text).unwrap(), bytes);
            assert_eq!(
                serde_json::from_str::<serde_json::Value>(&root).unwrap(),
                serde_json::from_str::<serde_json::Value>(text).unwrap()
            );
            assert_eq!(digest_of(&root).unwrap(), digest_of(text).unwrap());
            assert_eq!(derive(&root).unwrap(), derive(text).unwrap());
        }
    }
}

#[test]
fn every_permutation_of_nested_members_is_invariant() {
    let members = [
        r#""é":{"z":2,"a":1}"#,
        r#""a":[3,1,2]"#,
        r#""😀":null"#,
        r#""\ue000":false"#,
    ];
    let expected = canonicalize(&format!("{{{}}}", members.join(","))).unwrap();
    for a in 0..4 {
        for b in 0..4 {
            for c in 0..4 {
                for d in 0..4 {
                    let indices = [a, b, c, d];
                    if (0..4).all(|i| indices[..i].iter().all(|j| *j != indices[i])) {
                        let input = format!("{{ {} }}", indices.map(|i| members[i]).join(" , "));
                        assert_eq!(canonicalize(&input).unwrap(), expected);
                    }
                }
            }
        }
    }
}

#[test]
fn utf8_not_utf16_member_order_and_no_unicode_normalization() {
    assert_eq!(
        canonicalize(r#"{"😀":0,"\ue000":1,"é":2,"e\u0301":3}"#).unwrap(),
        "{\"é\":3,\"é\":2,\"\u{e000}\":1,\"😀\":0}".as_bytes()
    );
    assert_ne!(
        canonicalize(r#""é""#).unwrap(),
        canonicalize(r#""e\u0301""#).unwrap()
    );
}

#[test]
fn array_order_remains_semantic() {
    assert_ne!(
        canonicalize("[1,2,3]").unwrap(),
        canonicalize("[3,2,1]").unwrap()
    );
}

#[test]
fn every_scj_root_is_supported_by_public_derivation() {
    for text in ["{}", "[]", "null", "true", "false", "0", "-1", r#""text""#] {
        assert!(canonicalize(text).is_ok());
        assert!(derive(text).is_ok());
    }
}

#[test]
fn complete_closed_escape_table() {
    let mut value = String::new();
    let mut expected = String::from("\"");
    for n in 0..=31u8 {
        value.push(char::from(n));
        expected.push_str(&match n {
            8 => "\\b".to_owned(),
            9 => "\\t".to_owned(),
            10 => "\\n".to_owned(),
            12 => "\\f".to_owned(),
            13 => "\\r".to_owned(),
            _ => format!("\\u{n:04x}"),
        });
    }
    value.push_str("\u{7f}\"\\/é\u{85}\u{2028}\u{2029}😀");
    expected.push_str("\\u007f\\\"\\\\/é\u{85}\u{2028}\u{2029}😀\"");
    let input = serde_json::to_string(&value).unwrap();
    assert_eq!(canonicalize(&input).unwrap(), expected.as_bytes());
    assert_eq!(
        canonicalize(&format!("{{{input}:{input}}}")).unwrap(),
        format!("{{{expected}:{expected}}}").as_bytes()
    );
}

#[test]
fn equivalent_string_escape_spellings_canonicalize_identically() {
    for (escaped, raw) in [
        (r#""\u00e9""#, "\"é\""),
        (r#""\ud83d\ude00""#, "\"😀\""),
        (r#""\/""#, "\"/\""),
        (r#""\u000a""#, r#""\n""#),
        (r#""\u007F""#, r#""\u007f""#),
    ] {
        assert_eq!(canonicalize(escaped).unwrap(), canonicalize(raw).unwrap());
    }
}

macro_rules! refusal {
    ($name:ident, $input:expr, $error:expr) => {
        #[test]
        fn $name() {
            assert_eq!(canonicalize($input), Err($error));
            assert_eq!(digest_of($input), Err($error));
            assert_eq!(derive($input), Err($error));
        }
    };
}
refusal!(
    duplicate_at_root,
    r#"{"key":1,"key":2}"#,
    CanonicalJsonError::DuplicateKey
);
refusal!(
    duplicate_nested_object,
    r#"{"a":{"key":1,"key":2}}"#,
    CanonicalJsonError::DuplicateKey
);
refusal!(
    duplicate_in_array,
    r#"[{"key":1,"key":2}]"#,
    CanonicalJsonError::DuplicateKey
);
refusal!(
    duplicate_escaped_key,
    r#"{"a":1,"\u0061":2}"#,
    CanonicalJsonError::DuplicateKey
);
refusal!(
    duplicate_surrogate_pair_key,
    r#"{"😀":1,"\ud83d\ude00":2}"#,
    CanonicalJsonError::DuplicateKey
);
refusal!(
    duplicate_checked_before_second_value,
    r#"{"key":1,"key": malformed_private_value}"#,
    CanonicalJsonError::DuplicateKey
);
refusal!(
    numeric_preflight_precedes_duplicate_diagnostic,
    r#"{"key":1,"key":1.0}"#,
    CanonicalJsonError::NonInteger
);
refusal!(
    malformed_numeric_preflight_precedes_duplicate_diagnostic,
    r#"{"key":1,"key":1e}"#,
    CanonicalJsonError::InvalidJson
);
refusal!(
    numeric_preflight_can_precede_earlier_string_syntax_error,
    r#"["\q",1.0]"#,
    CanonicalJsonError::NonInteger
);
refusal!(fraction, "1.25", CanonicalJsonError::NonInteger);
refusal!(integral_float, "1.0", CanonicalJsonError::NonInteger);
refusal!(exponent, "1e2", CanonicalJsonError::NonInteger);
refusal!(zero_exponent, "0E+0", CanonicalJsonError::NonInteger);
refusal!(negative_zero, "-0", CanonicalJsonError::NonInteger);
refusal!(negative_zero_float, "-0.0", CanonicalJsonError::NonInteger);
refusal!(
    above_unsigned_domain,
    "18446744073709551616",
    CanonicalJsonError::NonInteger
);
refusal!(
    distinct_above_unsigned_domain,
    "18446744073709551617",
    CanonicalJsonError::NonInteger
);
refusal!(
    below_signed_domain,
    "-9223372036854775809",
    CanonicalJsonError::NonInteger
);
refusal!(
    nested_fraction,
    r#"{"a":[1,2.5]}"#,
    CanonicalJsonError::NonInteger
);
refusal!(leading_zero, "01", CanonicalJsonError::InvalidJson);
refusal!(leading_plus, "+1", CanonicalJsonError::InvalidJson);
refusal!(trailing_document, "{} {}", CanonicalJsonError::InvalidJson);
refusal!(
    lone_surrogate,
    r#""\ud800""#,
    CanonicalJsonError::InvalidJson
);
refusal!(utf8_bom, "\u{feff}{}", CanonicalJsonError::InvalidJson);

#[test]
fn integer_domain_boundaries_are_exact() {
    for text in [
        "-9223372036854775808",
        "-9223372036854775807",
        "-1",
        "0",
        "9223372036854775807",
        "9223372036854775808",
        "18446744073709551614",
        "18446744073709551615",
    ] {
        assert_eq!(canonicalize(text).unwrap(), text.as_bytes());
    }
    for n in -2048..=2048 {
        let text = n.to_string();
        assert_eq!(canonicalize(&text).unwrap(), text.as_bytes());
    }
}

#[test]
fn malformed_numeric_spellings_are_never_digested() {
    for text in [
        "-01", "00", "1.", ".1", "1e", "NaN", "Infinity", "1e9999", "-1e9999", "[0,01]", "[+1]",
        "[1e2]",
    ] {
        assert!(canonicalize(text).is_err());
        assert!(digest_of(text).is_err());
        assert!(derive(text).is_err());
    }
}

#[test]
fn literal_number_magic_objects_remain_objects_not_numeric_spellings() {
    for spelling in [
        "1.25",
        "1.0",
        "1e+2",
        "0e+0",
        "-0",
        "-0.0",
        "18446744073709551616",
        "-9223372036854775809",
    ] {
        let object = format!(r#"{{"$serde_json::private::Number":"{spelling}"}}"#);
        assert_eq!(canonicalize(&object).unwrap(), object.as_bytes());
        assert!(digest_of(&object).is_ok());
        assert!(derive(&object).is_ok());
        assert_eq!(canonicalize(spelling), Err(CanonicalJsonError::NonInteger));
        assert_eq!(digest_of(spelling), Err(CanonicalJsonError::NonInteger));
        assert_eq!(derive(spelling), Err(CanonicalJsonError::NonInteger));
    }
    assert_eq!(
        canonicalize(
            r#"{"$serde_json::private::Number":"1.25","$serde_json::private::Number":"2.5"}"#
        ),
        Err(CanonicalJsonError::DuplicateKey)
    );
}

#[test]
fn forbidden_numeric_spellings_are_refused_in_every_value_position() {
    for spelling in [
        "-0",
        "-0.0",
        "0.0",
        "1.25",
        "1e2",
        "1E-2",
        "0E+0",
        "1e9999",
        "-1e9999",
        "18446744073709551616",
        "-9223372036854775809",
    ] {
        for text in [
            spelling.to_owned(),
            format!(" \t{spelling}\r\n"),
            format!("[0,{spelling},1]"),
            format!(r#"{{"n":{spelling}}}"#),
            format!(r#"{{"a":[{{"n":{spelling}}}]}}"#),
        ] {
            assert_eq!(canonicalize(&text), Err(CanonicalJsonError::NonInteger));
            assert_eq!(digest_of(&text), Err(CanonicalJsonError::NonInteger));
            assert_eq!(derive(&text), Err(CanonicalJsonError::NonInteger));
        }
    }
}

#[test]
fn numeric_looking_strings_keys_and_escaped_quotes_are_not_numbers() {
    for text in [
        r#""-0 1.25 1e2 18446744073709551616""#,
        r#"{"-0":"1.25","1e2":"18446744073709551616"}"#,
        r#"["é😀1.25","quote\"1e2","backslash\\","-0"]"#,
        r#"["\\\"-0","\u0031.25","\ud83d\ude00 1e9999"]"#,
        r#"{"$serde_json::private::Number":{"n":18446744073709551615}}"#,
    ] {
        let bytes = canonicalize(text).unwrap();
        let canonical = std::str::from_utf8(&bytes).unwrap();
        assert_eq!(canonicalize(canonical).unwrap(), bytes);
        assert_eq!(digest_of(text).unwrap(), digest_of(canonical).unwrap());
        assert_eq!(derive(text).unwrap(), derive(canonical).unwrap());
    }
    for text in [
        r#"["é😀\\",-0]"#,
        r#"["escaped\"quote",1.25]"#,
        r#"{"key\\":1e2}"#,
    ] {
        assert_eq!(canonicalize(text), Err(CanonicalJsonError::NonInteger));
    }
}

#[test]
fn malformed_tokens_and_comments_are_not_scanned_as_numeric_values() {
    for text in [
        "-",
        "-01",
        "00",
        "1.",
        ".1",
        "1e",
        "1e+",
        "1E-",
        "1.2.3",
        "1e2e3",
        "1.25private",
        "private1.25",
        "+1",
        "/* 1.25 -0 */",
        "// 1e2\n0",
        "[0,/* 1.25 */1]",
        "[0,// -0\n1]",
        "[0,private1e2]",
    ] {
        assert_eq!(canonicalize(text), Err(CanonicalJsonError::InvalidJson));
        assert_eq!(digest_of(text), Err(CanonicalJsonError::InvalidJson));
        assert_eq!(derive(text), Err(CanonicalJsonError::InvalidJson));
    }
}

#[test]
fn long_numeric_tokens_are_bounded_and_payload_free() {
    for text in [
        "9".repeat(100_000),
        format!("-{}", "9".repeat(100_000)),
        format!("1e{}", "9".repeat(100_000)),
    ] {
        assert!(matches!(
            canonicalize(&text),
            Err(CanonicalJsonError::NonInteger)
        ));
        for error in [
            canonicalize(&text).unwrap_err(),
            digest_of(&text).unwrap_err(),
            derive(&text).unwrap_err(),
        ] {
            assert_eq!(error, CanonicalJsonError::NonInteger);
            assert!(!error.to_string().contains(&text));
            assert!(!format!("{error:?}").contains(&text));
            assert!(std::error::Error::source(&error).is_none());
        }
    }
}

#[test]
fn same_key_in_different_objects_is_not_duplicate() {
    assert!(canonicalize(r#"[{"a":1},{"a":2}]"#).is_ok());
}

fn nested(levels: usize, object: bool, leaf: &str) -> String {
    let (open, close) = if object {
        (r#"{"a":"#, "}")
    } else {
        ("[", "]")
    };
    format!("{}{leaf}{}", open.repeat(levels), close.repeat(levels))
}

#[test]
fn root_depth_one_and_exact_limit_for_arrays_and_objects() {
    assert_eq!(MAX_INSTANCE_DEPTH, 64);
    for object in [false, true] {
        for leaf in ["0", "[]", "{}"] {
            assert!(canonicalize(&nested(63, object, leaf)).is_ok());
            assert_eq!(
                canonicalize(&nested(64, object, leaf)),
                Err(CanonicalJsonError::DepthExceeded)
            );
        }
    }
}

#[test]
fn depth_is_bounded_during_parse_before_malformed_deep_leaf() {
    for object in [false, true] {
        let input = nested(10_000, object, "private_malformed_leaf");
        assert_eq!(canonicalize(&input), Err(CanonicalJsonError::DepthExceeded));
    }
}

#[test]
fn mixed_container_depth_and_nested_duplicates_at_limit() {
    let mut input = r#"{"x":1,"\u0078":2}"#.to_owned();
    for n in 0..62 {
        input = if n % 2 == 0 {
            format!("[{input}]")
        } else {
            format!(r#"{{"a":{input}}}"#)
        };
    }
    assert_eq!(canonicalize(&input), Err(CanonicalJsonError::DuplicateKey));
    let valid = input.replace(",\"\\u0078\":2", "");
    assert!(canonicalize(&valid).is_ok());
}

#[test]
fn malformed_documents_are_sanitized_and_errors_have_no_payload_or_key() {
    let marker = "private_credential_marker_9a7f";
    let inputs = [
        format!(r#"{{"{marker}":1,"{marker}":2}}"#),
        format!(r#"{{"{marker}": "\q{marker}"}}"#),
        format!(r#"{{"{marker}": [1, ]}}"#),
        format!(r#"{{"{marker}": 1.25}}"#),
        nested(65, true, &format!("\"{marker}\"")),
    ];
    for input in inputs {
        let error = canonicalize(&input).unwrap_err();
        for rendered in [error.to_string(), format!("{error:?}")] {
            assert!(!rendered.contains(marker));
            assert!(!rendered.contains(&input));
        }
        assert!(std::error::Error::source(&error).is_none());
    }
}

#[test]
fn errors_are_typed_send_sync_and_payload_free() {
    fn check<T: std::error::Error + Send + Sync + Copy + Eq>() {}
    check::<CanonicalJsonError>();
    assert!(std::mem::size_of::<CanonicalJsonError>() <= 64);
}
