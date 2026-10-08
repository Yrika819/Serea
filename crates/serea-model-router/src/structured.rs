//! Bounded, offline host validation for JSON Schema model responses.

use jsonschema::{Draft, Validator};
use serde::de::{DeserializeSeed, Error as _, MapAccess, SeqAccess, Visitor};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Number, Value};
use std::collections::BTreeMap;

use crate::{
    BoundedCounter, MAX_MODEL_JSON_DEPTH, MAX_MODEL_RESPONSE_BYTES, MAX_MODEL_SCHEMA_BYTES,
    MAX_MODEL_VALIDATION_ERROR_BYTES, MAX_MODEL_VALIDATION_ERRORS,
};

const DUPLICATE_KEY_MARKER: &str = "SEREA_DUPLICATE_JSON_OBJECT_KEY";
const DEPTH_LIMIT_MARKER: &str = "SEREA_JSON_DEPTH_LIMIT";
const MAX_PATH_BYTES: usize = 512;
const MAX_KEYWORD_BYTES: usize = 128;
const MAX_DESCRIPTION_BYTES: usize = 256;

#[derive(Debug, Clone, PartialEq, Eq)]
/// Failure while parsing or validating a structured provider response.
pub enum StructuredValidationError {
    /// The host schema is invalid or could not be compiled offline.
    InvalidSchema,
    /// The provider response exceeded the configured byte limit.
    ResponseTooLarge,
    /// The provider response was not a single valid JSON value.
    MalformedJson,
    /// The provider response contained a duplicate object key.
    DuplicateKey,
    /// The schema or response exceeded the configured nesting limit.
    TooDeep,
    /// The parsed response did not satisfy the host schema.
    InvalidOutput(Vec<ValidationDiagnostic>),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
/// Bounded, content-free schema diagnostic suitable for a repair prompt.
pub struct ValidationDiagnostic {
    /// Bounded JSON Pointer location in the instance.
    pub instance_path: String,
    /// Bounded schema keyword associated with the failure.
    pub schema_keyword: String,
    /// Safe static description that does not contain instance data.
    pub description: String,
}

/// A value-free, bounded diagnostic suitable for the repair request.
pub fn validate_structured_response(
    schema: &Value,
    raw: &str,
) -> Result<Value, StructuredValidationError> {
    let validator = validate_json_schema(schema)?;
    if raw.len() > MAX_MODEL_RESPONSE_BYTES {
        return Err(StructuredValidationError::ResponseTooLarge);
    }
    let instance = parse_unique_json(raw)?;
    let diagnostics = diagnostics(&validator, &instance);
    if diagnostics.is_empty() {
        Ok(instance)
    } else {
        Err(StructuredValidationError::InvalidOutput(diagnostics))
    }
}

/// Compiles one host schema using Draft 2020-12 with no HTTP or file resolver.
pub(crate) fn validate_json_schema(schema: &Value) -> Result<Validator, StructuredValidationError> {
    let mut bytes = BoundedCounter::new(MAX_MODEL_SCHEMA_BYTES);
    if serde_json::to_writer(&mut bytes, schema).is_err() {
        return Err(StructuredValidationError::InvalidSchema);
    }
    if value_exceeds_depth(schema, MAX_MODEL_JSON_DEPTH) {
        return Err(StructuredValidationError::TooDeep);
    }
    jsonschema::options()
        .with_draft(Draft::Draft202012)
        .build(schema)
        .map_err(|_| StructuredValidationError::InvalidSchema)
}

fn value_exceeds_depth(root: &Value, limit: usize) -> bool {
    let mut pending = vec![(root, 0usize)];
    while let Some((value, depth)) = pending.pop() {
        if depth > limit {
            return true;
        }
        match value {
            Value::Array(items) => {
                pending.extend(items.iter().map(|item| (item, depth + 1)));
            }
            Value::Object(properties) => {
                pending.extend(properties.values().map(|value| (value, depth + 1)));
            }
            Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {}
        }
    }
    false
}

fn diagnostics(validator: &Validator, instance: &Value) -> Vec<ValidationDiagnostic> {
    let mut result = Vec::new();
    let mut encoded_bytes = 0usize;
    for error in validator
        .iter_errors(instance)
        .take(MAX_MODEL_VALIDATION_ERRORS)
    {
        let diagnostic = ValidationDiagnostic {
            instance_path: bounded(error.instance_path().as_str(), MAX_PATH_BYTES),
            schema_keyword: bounded(error.kind().keyword(), MAX_KEYWORD_BYTES),
            description: bounded(description(error.kind().keyword()), MAX_DESCRIPTION_BYTES),
        };
        let encoded_len = serde_json::to_vec(&diagnostic)
            .map(|bytes| bytes.len())
            .unwrap_or(usize::MAX);
        let separator = usize::from(!result.is_empty());
        let Some(next_bytes) = encoded_bytes
            .checked_add(encoded_len)
            .and_then(|bytes| bytes.checked_add(separator))
        else {
            break;
        };
        if next_bytes > MAX_MODEL_VALIDATION_ERROR_BYTES {
            break;
        }
        encoded_bytes = next_bytes;
        result.push(diagnostic);
    }
    result
}

fn description(keyword: &str) -> &'static str {
    match keyword {
        "type" => "value has the wrong JSON type",
        "required" => "required property is missing",
        "additionalProperties" | "unevaluatedProperties" => "property is not allowed",
        "minimum" | "maximum" | "exclusiveMinimum" | "exclusiveMaximum" => {
            "number is outside the allowed range"
        }
        "minLength" | "maxLength" | "pattern" => "string constraint is not satisfied",
        "minItems" | "maxItems" | "uniqueItems" => "array constraint is not satisfied",
        "enum" | "const" => "value is not one of the allowed choices",
        "anyOf" | "oneOf" | "allOf" => "value does not match the required schema",
        _ => "schema constraint is not satisfied",
    }
}

fn bounded(value: &str, limit: usize) -> String {
    if value.len() <= limit {
        return value.to_owned();
    }
    let mut end = limit;
    while !value.is_char_boundary(end) {
        end -= 1;
    }
    value[..end].to_owned()
}

fn parse_unique_json(raw: &str) -> Result<Value, StructuredValidationError> {
    let mut deserializer = serde_json::Deserializer::from_str(raw);
    let parsed = UniqueJsonSeed { depth: 0 }
        .deserialize(&mut deserializer)
        .and_then(|value| deserializer.end().map(|()| value));
    parsed.map_err(|error| {
        let message = error.to_string();
        if message.contains(DUPLICATE_KEY_MARKER) {
            StructuredValidationError::DuplicateKey
        } else if message.contains(DEPTH_LIMIT_MARKER) {
            StructuredValidationError::TooDeep
        } else {
            StructuredValidationError::MalformedJson
        }
    })
}

struct UniqueJsonSeed {
    depth: usize,
}

impl<'de> DeserializeSeed<'de> for UniqueJsonSeed {
    type Value = Value;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        if self.depth > MAX_MODEL_JSON_DEPTH {
            return Err(D::Error::custom(DEPTH_LIMIT_MARKER));
        }
        deserializer.deserialize_any(UniqueJsonVisitor { depth: self.depth })
    }
}

struct UniqueJsonVisitor {
    depth: usize,
}

impl<'de> Visitor<'de> for UniqueJsonVisitor {
    type Value = Value;

    fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("a JSON value with unique object keys")
    }

    fn visit_unit<E>(self) -> Result<Self::Value, E> {
        Ok(Value::Null)
    }

    fn visit_none<E>(self) -> Result<Self::Value, E> {
        Ok(Value::Null)
    }

    fn visit_bool<E>(self, value: bool) -> Result<Self::Value, E> {
        Ok(Value::Bool(value))
    }

    fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E> {
        Ok(Value::Number(Number::from(value)))
    }

    fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E> {
        Ok(Value::Number(Number::from(value)))
    }

    fn visit_f64<E>(self, value: f64) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Number::from_f64(value)
            .map(Value::Number)
            .ok_or_else(|| E::custom("number is not finite"))
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E> {
        Ok(Value::String(value.to_owned()))
    }

    fn visit_string<E>(self, value: String) -> Result<Self::Value, E> {
        Ok(Value::String(value))
    }

    fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        let mut values = Vec::new();
        while let Some(value) = sequence.next_element_seed(UniqueJsonSeed {
            depth: self.depth + 1,
        })? {
            values.push(value);
        }
        Ok(Value::Array(values))
    }

    fn visit_map<A>(self, mut object: A) -> Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        let mut values = Map::new();
        let mut seen = BTreeMap::new();
        while let Some(key) = object.next_key_seed(UniqueJsonMapKeySeed)? {
            let key = match key {
                UniqueJsonMapKey::Number => {
                    let raw_number = object.next_value::<String>()?;
                    let number =
                        serde_json::from_str::<Number>(&raw_number).map_err(A::Error::custom)?;
                    return Ok(Value::Number(number));
                }
                UniqueJsonMapKey::String(key) => key,
            };
            if seen.insert(key.clone(), ()).is_some() {
                return Err(A::Error::custom(DUPLICATE_KEY_MARKER));
            }
            let value = object.next_value_seed(UniqueJsonSeed {
                depth: self.depth + 1,
            })?;
            values.insert(key, value);
        }
        Ok(Value::Object(values))
    }
}

enum UniqueJsonMapKey {
    Number,
    String(String),
}

struct UniqueJsonMapKeySeed;

impl<'de> DeserializeSeed<'de> for UniqueJsonMapKeySeed {
    type Value = UniqueJsonMapKey;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_any(UniqueJsonMapKeyVisitor)
    }
}

struct UniqueJsonMapKeyVisitor;

impl<'de> Visitor<'de> for UniqueJsonMapKeyVisitor {
    type Value = UniqueJsonMapKey;

    fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("a JSON object key or arbitrary-precision number marker")
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E> {
        Ok(UniqueJsonMapKey::String(value.to_owned()))
    }

    fn visit_borrowed_str<E>(self, value: &'de str) -> Result<Self::Value, E> {
        Ok(UniqueJsonMapKey::String(value.to_owned()))
    }

    fn visit_string<E>(self, value: String) -> Result<Self::Value, E> {
        Ok(UniqueJsonMapKey::String(value))
    }

    fn visit_newtype_struct<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let marker = String::deserialize(deserializer)?;
        if marker == "$serde_json::private::Number" {
            Ok(UniqueJsonMapKey::Number)
        } else {
            Err(D::Error::custom("invalid internal JSON map key"))
        }
    }
}
