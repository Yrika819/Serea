//! Duplicate-name-rejecting JSON parsing.
//!
//! `serde_json` keeps the last of two identical object names, so a document
//! that a human reads as `{"maxLength":1,"maxLength":1048576}` would compile
//! and digest as one value. Both the trusted schema catalog and the
//! model-authored tool-call proposal go through this parser, so no Serea code
//! path can act on a document whose member names repeat.

use std::fmt;

use serde::de::Deserialize;
use serde_json::{Map, Value};

/// Why a strict parse failed. The rejected text is never retained.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StrictJsonError {
    /// Invalid JSON syntax.
    Syntax,
    /// A repeated object name at any depth.
    DuplicateMemberName,
}

impl fmt::Display for StrictJsonError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Syntax => f.write_str("document is not valid JSON"),
            Self::DuplicateMemberName => f.write_str("document repeats an object member name"),
        }
    }
}
impl std::error::Error for StrictJsonError {}

/// Parses `text` into a [`Value`], refusing any repeated object name.
pub fn parse_strict(text: &str) -> Result<Value, StrictJsonError> {
    let mut deserializer = serde_json::Deserializer::from_str(text);
    let parsed = StrictValue::deserialize(&mut deserializer).map_err(|error| classify(&error))?;
    deserializer.end().map_err(|error| classify(&error))?;
    Ok(parsed.0)
}

fn classify(error: &serde_json::Error) -> StrictJsonError {
    if error.to_string().contains(DUPLICATE_MARKER) {
        StrictJsonError::DuplicateMemberName
    } else {
        StrictJsonError::Syntax
    }
}

/// Marker text carried by the deserializer's duplicate-name error so the
/// caller can distinguish it without retaining the rejected document.
const DUPLICATE_MARKER: &str = "repeated object name";

/// Wrapper whose only purpose is a duplicate-rejecting `Deserialize`.
struct StrictValue(Value);

impl<'de> Deserialize<'de> for StrictValue {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::de::Deserializer<'de>,
    {
        deserializer.deserialize_any(StrictVisitor)
    }
}

struct StrictVisitor;

impl<'de> serde::de::Visitor<'de> for StrictVisitor {
    type Value = StrictValue;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a JSON value without a repeated object name")
    }

    fn visit_bool<E: serde::de::Error>(self, value: bool) -> Result<StrictValue, E> {
        Ok(StrictValue(Value::Bool(value)))
    }

    fn visit_i64<E: serde::de::Error>(self, value: i64) -> Result<StrictValue, E> {
        Ok(StrictValue(Value::from(value)))
    }

    fn visit_u64<E: serde::de::Error>(self, value: u64) -> Result<StrictValue, E> {
        Ok(StrictValue(Value::from(value)))
    }

    fn visit_f64<E: serde::de::Error>(self, value: f64) -> Result<StrictValue, E> {
        serde_json::Number::from_f64(value)
            .map(|number| StrictValue(Value::Number(number)))
            .ok_or_else(|| serde::de::Error::custom("non-finite number"))
    }

    fn visit_str<E: serde::de::Error>(self, value: &str) -> Result<StrictValue, E> {
        Ok(StrictValue(Value::String(value.to_owned())))
    }

    fn visit_string<E: serde::de::Error>(self, value: String) -> Result<StrictValue, E> {
        Ok(StrictValue(Value::String(value)))
    }

    fn visit_unit<E: serde::de::Error>(self) -> Result<StrictValue, E> {
        Ok(StrictValue(Value::Null))
    }

    fn visit_some<D>(self, deserializer: D) -> Result<StrictValue, D::Error>
    where
        D: serde::de::Deserializer<'de>,
    {
        deserializer.deserialize_any(self)
    }

    fn visit_none<E: serde::de::Error>(self) -> Result<StrictValue, E> {
        Ok(StrictValue(Value::Null))
    }

    fn visit_newtype_struct<D>(self, deserializer: D) -> Result<StrictValue, D::Error>
    where
        D: serde::de::Deserializer<'de>,
    {
        deserializer.deserialize_any(self)
    }

    fn visit_seq<A>(self, mut visitor: A) -> Result<StrictValue, A::Error>
    where
        A: serde::de::SeqAccess<'de>,
    {
        let mut items = Vec::new();
        while let Some(item) = visitor.next_element::<StrictValue>()? {
            items.push(item.0);
        }
        Ok(StrictValue(Value::Array(items)))
    }

    fn visit_map<A>(self, mut visitor: A) -> Result<StrictValue, A::Error>
    where
        A: serde::de::MapAccess<'de>,
    {
        let mut members = Map::new();
        while let Some((key, value)) = visitor.next_entry::<String, StrictValue>()? {
            if members.contains_key(&key) {
                return Err(serde::de::Error::custom(DUPLICATE_MARKER));
            }
            members.insert(key, value.0);
        }
        Ok(StrictValue(Value::Object(members)))
    }

    fn visit_bytes<E: serde::de::Error>(self, _value: &[u8]) -> Result<StrictValue, E> {
        Err(serde::de::Error::custom("bytes are not JSON"))
    }

    fn visit_byte_buf<E: serde::de::Error>(self, _value: Vec<u8>) -> Result<StrictValue, E> {
        Err(serde::de::Error::custom("bytes are not JSON"))
    }
}
