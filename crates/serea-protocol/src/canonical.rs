//! SCJ-1 canonical JSON and named, domain-separated IDK-1 framing (ADR-0019).
//!
//! Only original text can preserve duplicate names. These functions cannot recover
//! duplicates discarded by a caller's earlier parse; retain text at the input boundary.
//! SCJ-1 deliberately excludes fractional/exponent numbers, including model documents
//! with fractional temperature. This does not change model wire validation.
use std::fmt;

use serde::de::{self, DeserializeSeed, MapAccess, SeqAccess, Visitor};
use serde_json::{Map, Value};
use sha2::{Digest as _, Sha256};

use crate::{
    CapabilityId, Digest, IdempotencyKey, MAX_INSTANCE_DEPTH, ProtocolError, SemVer, StepId, TaskId,
};

/// Payload-free canonical JSON rejection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CanonicalJsonError {
    /// Repeated decoded object name.
    DuplicateKey,
    /// Invalid JSON syntax.
    InvalidJson,
    /// Number outside the SCJ-1 integer domain.
    NonInteger,
    /// Instance deeper than the protocol limit.
    DepthExceeded,
    /// A checked output failed protocol validation.
    Protocol(ProtocolError),
}
impl fmt::Display for CanonicalJsonError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateKey => f.write_str("canonical JSON contains a duplicate object name"),
            Self::InvalidJson => f.write_str("canonical JSON input is invalid JSON"),
            Self::NonInteger => {
                f.write_str("canonical JSON number is outside the SCJ-1 integer domain")
            }
            Self::DepthExceeded => f.write_str("canonical JSON exceeds the instance depth limit"),
            Self::Protocol(error) => write!(f, "canonical JSON output validation failed: {error}"),
        }
    }
}
impl std::error::Error for CanonicalJsonError {}

/// Returns SCJ-1 UTF-8 bytes for original JSON text (ADR-0019 §SCJ-1).
///
/// Accepts any JSON root, but refuses decoded duplicate names, non-integer
/// numbers (including `-0`), integers outside `i64::MIN..=u64::MAX`, and instances
/// deeper than [`MAX_INSTANCE_DEPTH`] (root depth is 1). Depth is bounded during
/// parsing, not after constructing an arbitrarily deep value.
///
/// Callers must retain original text: serializing an already-lossy `Value` cannot
/// restore duplicate names. Numeric lexical preflight runs first: when multiple
/// rules fail, its NonInteger/InvalidJson diagnostic can precede DuplicateKey or
/// another syntax error. All such inputs are refused without a digest.
/// Errors never retain rejected text or parser messages.
pub fn canonicalize(text: &str) -> Result<Vec<u8>, CanonicalJsonError> {
    validate_numeric_spellings(text)?;
    let mut parser = serde_json::Deserializer::from_str(text);
    let mut rejection = None;
    let value = InstanceSeed {
        depth: 1,
        rejection: &mut rejection,
    }
    .deserialize(&mut parser)
    .map_err(|_| rejection.unwrap_or(CanonicalJsonError::InvalidJson))?;
    parser.end().map_err(|_| CanonicalJsonError::InvalidJson)?;
    let mut output = Vec::new();
    write_value(&value, &mut output);
    Ok(output)
}

// Validate original spellings before serde_json can normalize -0 or dispatch
// arbitrary_precision numbers as synthetic maps. This pass is allocation-free
// and nonrecursive; JSON structure, string validity and duplicates remain serde's job.
fn validate_numeric_spellings(text: &str) -> Result<(), CanonicalJsonError> {
    fn separator(byte: u8) -> bool {
        matches!(
            byte,
            b' ' | b'\t' | b'\r' | b'\n' | b'[' | b']' | b'{' | b'}' | b',' | b':' | b'"'
        )
    }

    let bytes = text.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'"' {
            index += 1;
            while index < bytes.len() {
                let byte = bytes[index];
                index += 1;
                if byte == b'\\' && index < bytes.len() {
                    index += 1;
                } else if byte == b'"' {
                    break;
                }
            }
        } else if separator(bytes[index]) {
            index += 1;
        } else {
            let start = index;
            while index < bytes.len() && !separator(bytes[index]) {
                index += 1;
            }
            let token = &text[start..index];
            if matches!(bytes[start], b'-' | b'0'..=b'9') {
                validate_number_token(token)?;
            } else if !matches!(token, "true" | "false" | "null") {
                // Stop at invalid unquoted text, rather than finding numbers
                // inside comments or malformed literals. If preflight reaches
                // this point, the visitor determines the remaining diagnostic.
                return Ok(());
            }
        }
    }
    Ok(())
}

fn validate_number_token(token: &str) -> Result<(), CanonicalJsonError> {
    let bytes = token.as_bytes();
    let negative = bytes[0] == b'-';
    let mut index = usize::from(negative);
    match bytes.get(index) {
        Some(b'0') => index += 1,
        Some(b'1'..=b'9') => {
            while bytes.get(index).is_some_and(u8::is_ascii_digit) {
                index += 1;
            }
        }
        _ => return Err(CanonicalJsonError::InvalidJson),
    }
    let integer_end = index;
    if bytes.get(index) == Some(&b'.') {
        index += 1;
        let start = index;
        while bytes.get(index).is_some_and(u8::is_ascii_digit) {
            index += 1;
        }
        if index == start {
            return Err(CanonicalJsonError::InvalidJson);
        }
    }
    if matches!(bytes.get(index), Some(b'e' | b'E')) {
        index += 1;
        if matches!(bytes.get(index), Some(b'+' | b'-')) {
            index += 1;
        }
        let start = index;
        while bytes.get(index).is_some_and(u8::is_ascii_digit) {
            index += 1;
        }
        if index == start {
            return Err(CanonicalJsonError::InvalidJson);
        }
    }
    if index != bytes.len() {
        return Err(CanonicalJsonError::InvalidJson);
    }
    // Both integer endpoints fit in 20 bytes (including a possible minus).
    // Only bounded integer tokens reach native conversion, never float parsing.
    if integer_end != bytes.len() || token == "-0" || token.len() > 20 {
        return Err(CanonicalJsonError::NonInteger);
    }
    let in_domain = if negative {
        token.parse::<i64>().is_ok()
    } else {
        token.parse::<u64>().is_ok()
    };
    if in_domain {
        Ok(())
    } else {
        Err(CanonicalJsonError::NonInteger)
    }
}

struct InstanceSeed<'a> {
    depth: usize,
    rejection: &'a mut Option<CanonicalJsonError>,
}

impl InstanceSeed<'_> {
    fn reject<E: de::Error>(&mut self, error: CanonicalJsonError) -> E {
        *self.rejection = Some(error);
        // The serde error is discarded at the public boundary. No rejected name,
        // number or string is ever passed to its formatting machinery here.
        E::custom("SCJ-1 input refused")
    }
}

impl<'de> DeserializeSeed<'de> for InstanceSeed<'_> {
    type Value = Value;

    fn deserialize<D: de::Deserializer<'de>>(mut self, deserializer: D) -> Result<Value, D::Error> {
        if self.depth > MAX_INSTANCE_DEPTH {
            return Err(self.reject(CanonicalJsonError::DepthExceeded));
        }
        deserializer.deserialize_any(self)
    }
}

impl<'de> Visitor<'de> for InstanceSeed<'_> {
    type Value = Value;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("an SCJ-1 value")
    }

    fn visit_unit<E: de::Error>(self) -> Result<Value, E> {
        Ok(Value::Null)
    }

    fn visit_bool<E: de::Error>(self, value: bool) -> Result<Value, E> {
        Ok(Value::Bool(value))
    }

    fn visit_i64<E: de::Error>(self, value: i64) -> Result<Value, E> {
        Ok(Value::Number(value.into()))
    }

    fn visit_u64<E: de::Error>(self, value: u64) -> Result<Value, E> {
        Ok(Value::Number(value.into()))
    }

    fn visit_f64<E: de::Error>(mut self, _: f64) -> Result<Value, E> {
        // Defense in depth; lexical admission must not depend on this callback.
        Err(self.reject(CanonicalJsonError::NonInteger))
    }

    fn visit_str<E: de::Error>(self, value: &str) -> Result<Value, E> {
        Ok(Value::String(value.to_owned()))
    }

    fn visit_string<E: de::Error>(self, value: String) -> Result<Value, E> {
        Ok(Value::String(value))
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut sequence: A) -> Result<Value, A::Error> {
        let mut values = Vec::new();
        while let Some(value) = sequence.next_element_seed(InstanceSeed {
            depth: self.depth + 1,
            rejection: self.rejection,
        })? {
            values.push(value);
        }
        Ok(Value::Array(values))
    }

    fn visit_map<A: MapAccess<'de>>(mut self, mut object: A) -> Result<Value, A::Error> {
        let mut values = Map::new();
        while let Some(key) = object.next_key::<String>()? {
            // Check the decoded name BEFORE parsing its value. Escaped names
            // compare identically, and a malicious duplicate value is not read.
            if values.contains_key(&key) {
                return Err(self.reject(CanonicalJsonError::DuplicateKey));
            }
            let value = object.next_value_seed(InstanceSeed {
                depth: self.depth + 1,
                rejection: self.rejection,
            })?;
            values.insert(key, value);
        }
        Ok(Value::Object(values))
    }
}

fn write_string(value: &str, output: &mut Vec<u8>) {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    output.push(b'"');
    for character in value.chars() {
        match character {
            '"' => output.extend_from_slice(br#"\""#),
            '\\' => output.extend_from_slice(br"\\"),
            '\u{8}' => output.extend_from_slice(br"\b"),
            '\u{c}' => output.extend_from_slice(br"\f"),
            '\n' => output.extend_from_slice(br"\n"),
            '\r' => output.extend_from_slice(br"\r"),
            '\t' => output.extend_from_slice(br"\t"),
            '\u{0}'..='\u{1f}' | '\u{7f}' => {
                let code = character as usize;
                output.extend_from_slice(br"\u00");
                output.push(HEX[code >> 4]);
                output.push(HEX[code & 15]);
            }
            _ => output.extend_from_slice(character.encode_utf8(&mut [0; 4]).as_bytes()),
        }
    }
    output.push(b'"');
}

// Recursion is safe here: only the depth-bounded visitor above constructs values.
fn write_value(value: &Value, output: &mut Vec<u8>) {
    match value {
        Value::Null => output.extend_from_slice(b"null"),
        Value::Bool(true) => output.extend_from_slice(b"true"),
        Value::Bool(false) => output.extend_from_slice(b"false"),
        Value::Number(number) => output.extend_from_slice(number.to_string().as_bytes()),
        Value::String(string) => write_string(string, output),
        Value::Array(values) => {
            output.push(b'[');
            for (index, item) in values.iter().enumerate() {
                if index != 0 {
                    output.push(b',');
                }
                write_value(item, output);
            }
            output.push(b']');
        }
        Value::Object(values) => {
            // Explicit ordering remains correct if serde_json's preserve_order
            // feature is enabled by a future workspace consumer.
            let mut members: Vec<_> = values.iter().collect();
            members.sort_unstable_by(|(a, _), (b, _)| a.as_bytes().cmp(b.as_bytes()));
            output.push(b'{');
            for (index, (key, item)) in members.into_iter().enumerate() {
                if index != 0 {
                    output.push(b',');
                }
                write_string(key, output);
                output.push(b':');
                write_value(item, output);
            }
            output.push(b'}');
        }
    }
}

fn hash_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut result = String::with_capacity(64);
    for byte in Sha256::digest(bytes) {
        result.push(char::from(HEX[usize::from(byte >> 4)]));
        result.push(char::from(HEX[usize::from(byte & 15)]));
    }
    result
}

/// Computes `sha256:` plus lowercase SHA-256 of SCJ-1 bytes (ADR-0019).
///
/// Takes original JSON text and propagates the same refusals as [`canonicalize`].
pub fn digest_of(text: &str) -> Result<Digest, CanonicalJsonError> {
    Digest::new(format!("sha256:{}", hash_hex(&canonicalize(text)?)))
        .map_err(CanonicalJsonError::Protocol)
}

fn idempotency_preimage(fields: [&[u8]; 5]) -> Vec<u8> {
    const NAMES: [&[u8]; 5] = [
        b"task_id",
        b"step_id",
        b"capability_id",
        b"capability_version",
        b"arguments_canonical",
    ];
    let mut preimage = Vec::new();
    preimage.extend_from_slice(b"serea.idempotency.v1\0");
    preimage.push(5);
    for (name, value) in NAMES.into_iter().zip(fields) {
        for bytes in [name, value] {
            // All supported targets have usize <= u64; the length is in BYTES,
            // for both field names and values, never Unicode scalar count.
            preimage.extend_from_slice(&(bytes.len() as u64).to_be_bytes());
            preimage.extend_from_slice(bytes);
        }
    }
    preimage
}

/// Derives `idk_` plus lowercase SHA-256 of the named IDK-1 preimage (ADR-0019).
///
/// Identifier/version inputs must already have passed their typed validation.
/// Arguments are original text and may have any SCJ-1 root; this generic primitive
/// does not relax [`crate::ActionRequest`]'s object-only arguments requirement.
/// Non-capability steps carry no key; their presence rules belong to
/// [`crate::TaskStep`], not to a `kind` argument on this capability-tuple API.
pub fn derive_idempotency_key(
    task: &TaskId,
    step: &StepId,
    capability: &CapabilityId,
    version: &SemVer,
    arguments: &str,
) -> Result<IdempotencyKey, CanonicalJsonError> {
    let arguments = canonicalize(arguments)?;
    let preimage = idempotency_preimage([
        task.as_str().as_bytes(),
        step.as_str().as_bytes(),
        capability.as_str().as_bytes(),
        version.as_str().as_bytes(),
        &arguments,
    ]);
    IdempotencyKey::new(format!("idk_{}", hash_hex(&preimage)))
        .map_err(CanonicalJsonError::Protocol)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::CAPABILITY_VERBS;
    use std::collections::HashSet;

    const TASK: &[u8] = b"tsk_01JQ8Z9K3M7QWXR4V2T6YH0BNA";
    const STEP: &[u8] = b"stp_01JQ8Z9M3R2CVN8H5FWK7PQDSF";

    #[test]
    fn vector_one_preimage_is_exactly_282_bytes_with_named_lp_fields() {
        let fields: [&[u8]; 5] = [
            TASK,
            STEP,
            b"calendar.events.list",
            b"1.2.0",
            br#"{"limit":25,"range":"tomorrow"}"#,
        ];
        let encoded = idempotency_preimage(fields);
        assert_eq!(encoded.len(), 282);
        assert_eq!(&encoded[..21], b"serea.idempotency.v1\0");
        assert_eq!(encoded[21], 5);
        let mut cursor = 22;
        for (name, value) in [
            b"task_id".as_slice(),
            b"step_id",
            b"capability_id",
            b"capability_version",
            b"arguments_canonical",
        ]
        .into_iter()
        .zip(fields)
        {
            for expected in [name, value] {
                assert_eq!(
                    &encoded[cursor..cursor + 8],
                    &(expected.len() as u64).to_be_bytes()
                );
                cursor += 8;
                assert_eq!(&encoded[cursor..cursor + expected.len()], expected);
                cursor += expected.len();
            }
        }
        assert_eq!(cursor, encoded.len());
    }

    #[test]
    fn historical_invalid_id_a_raw_framing_only() {
        assert!(CapabilityId::new("p.r.list").is_err());
        let preimage = idempotency_preimage([TASK, STEP, b"p.r.list", b"0.0.0", b"-12"]);
        assert_eq!(
            hash_hex(&preimage),
            "b9e8299d5b628af7d40e253035ce7af0f653a4523a20448ea061bea316f0adaf"
        );
    }

    #[test]
    fn historical_invalid_id_b_raw_framing_only() {
        assert!(CapabilityId::new("p.r.list").is_err());
        let preimage = idempotency_preimage([TASK, STEP, b"p.r.list", b"0.0.0-1", b"2"]);
        assert_eq!(
            hash_hex(&preimage),
            "141fa78316b05b374a0a11a2fe7093880daa19598dddc9b22675ffadedf232ef"
        );
    }

    #[test]
    fn raw_concat_collision_is_removed_by_named_framing_not_a_hash_injectivity_claim() {
        let a: [&[u8]; 5] = [TASK, STEP, b"pp.rr.list", b"0.0.0", b"-12"];
        let b: [&[u8]; 5] = [TASK, STEP, b"pp.rr.list", b"0.0.0-1", b"2"];
        assert_eq!(a.concat(), b.concat());
        assert_eq!(idempotency_preimage(a).len(), 244);
        assert_eq!(idempotency_preimage(b).len(), 244);
        assert_ne!(idempotency_preimage(a), idempotency_preimage(b));
        for (version, args) in [
            (b"0.0.0".as_slice(), br#"{"n":-12}"#.as_slice()),
            (b"0.0.0-1", br#"{"n":2}"#),
        ] {
            assert_eq!(
                idempotency_preimage([TASK, STEP, b"pp.rr.list", version, args]).len(),
                250
            );
        }
    }

    #[test]
    fn each_of_five_values_changes_the_preimage() {
        let base: [&[u8]; 5] = [TASK, STEP, b"calendar.events.list", b"1.2.0", b"{}"];
        let changes: [&[u8]; 5] = [
            b"tsk_01JQ8Z9K3M7QWXR4V2T6YH0BNB",
            b"stp_01JQ8Z9M3R2CVN8H5FWK7PQDSG",
            b"calendar.event.create",
            b"1.2.1",
            br#"{"n":1}"#,
        ];
        for index in 0..5 {
            let mut changed = base;
            changed[index] = changes[index];
            assert_ne!(idempotency_preimage(base), idempotency_preimage(changed));
        }
    }

    #[test]
    fn named_encoding_is_injective_over_113400_valid_tuples_not_sha256() {
        let suffixes = [
            "",
            "-0",
            "-1",
            "-2",
            "-alpha",
            "-alpha.1",
            "-beta",
            "-rc.1",
            "+build",
            "-0+build",
            "-1+build",
            "-2+build",
            "-alpha+build",
            "-beta+build",
            "-rc.1+build",
        ];
        let arguments = [
            "-12",
            "2",
            "0",
            "-1",
            "1",
            "null",
            "true",
            "false",
            "{}",
            "[]",
            r#""""#,
            r#""é""#,
            "[1]",
            "[2]",
            "[1,2]",
            "[2,1]",
            r#"{"n":1}"#,
            r#"{"n":2}"#,
            r#"{"n":-12}"#,
            "18446744073709551615",
        ];
        let mut seen = HashSet::new();
        for verb in CAPABILITY_VERBS {
            let capability = CapabilityId::new(format!("pp.rr.{verb}")).unwrap();
            for major in 0..3 {
                for minor in 0..3 {
                    for patch in 0..3 {
                        for suffix in suffixes {
                            let version =
                                SemVer::new(format!("{major}.{minor}.{patch}{suffix}")).unwrap();
                            for argument in arguments {
                                let canonical = canonicalize(argument).unwrap();
                                assert!(
                                    seen.insert(idempotency_preimage([
                                        TASK,
                                        STEP,
                                        capability.as_str().as_bytes(),
                                        version.as_str().as_bytes(),
                                        &canonical
                                    ])),
                                    "distinct canonical tuples must have distinct encodings"
                                );
                            }
                        }
                    }
                }
            }
        }
        assert_eq!(seen.len(), 113_400);
    }
}
