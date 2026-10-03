#![cfg(feature = "arbitrary-precision")]

use jsonschema_value::{numeric::bignum, types::number_is_integer};
use serde_json::Number;

fn number(text: &str) -> Number {
    serde_json::from_str(text).expect("valid JSON number")
}

#[test]
fn conversion_helpers_decline_extreme_shifts_without_overflow() {
    for text in [
        "1e-9223372036854775808",
        "1.0e-9223372036854775807",
        "1.00e-9223372036854775807",
        "-1.00e-9223372036854775807",
        "0.00e-9223372036854775807",
        "1e-170141183460469231731687303715884105728",
        "1e999999999999999999999999999999999999999999",
        "1e-999999999999999999999999999999999999999999",
    ] {
        let num = number(text);
        assert!(bignum::try_parse_bigint(&num).is_none(), "BigInt/{text}");
        assert!(
            bignum::try_parse_bigfraction(&num).is_none(),
            "BigFraction/{text}"
        );
    }
}

#[test]
fn conversion_helpers_preserve_supported_decimal_values_and_zero() {
    for (text, expected) in [
        ("10e-1", "1"),
        ("0.1e1", "1"),
        ("-10e-1", "-1"),
        ("42949672950e-1", "4294967295"),
        ("0.0e-9223372036854775807", "0"),
    ] {
        assert_eq!(
            bignum::try_parse_bigint(&number(text)).unwrap().to_string(),
            expected,
            "{text}"
        );
    }
    for (text, expected) in [("1.5e0", "3/2"), ("-1.5e0", "-3/2"), ("1e-2", "1/100")] {
        assert_eq!(
            bignum::try_parse_bigfraction(&number(text))
                .unwrap()
                .to_string(),
            expected,
            "{text}"
        );
    }
}

#[test]
fn fraction_cap_accounts_for_fractional_digits_not_only_the_exponent() {
    // Raw exponent -1 is small, but the actual denominator needs 1,000,001 places.
    let text = format!("0.{}1e-1", "0".repeat(999_999));
    assert!(bignum::try_parse_bigfraction(&number(&text)).is_none());
}

#[test]
fn integer_classification_is_exact_without_materializing_extreme_powers() {
    for (text, expected) in [
        ("100e-2", true),
        ("100e-3", false),
        ("1.00000000000000000001", false),
        ("-0.0", true),
        ("1.00e-9223372036854775807", false),
        ("1e-170141183460469231731687303715884105728", false),
        ("1e-999999999999999999999999999999999999999999", false),
        ("-1e+999999999999999999999999999999999999999999", true),
        ("0.00e-9223372036854775807", true),
        ("-0e-999999999999999999999999999999999999999999", true),
        ("0e+999999999999999999999999999999999999999999", true),
        ("1E+0000000000000000000000000000000000000000", true),
    ] {
        assert_eq!(number_is_integer(&number(text)), expected, "{text}");
    }
}
