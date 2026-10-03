//! The frozen Serea wire types and enums.
//!
//! Normative source: the eleven documents under `docs/protocols/`. Every item
//! below cites the section it implements; nothing here restates a protocol
//! definition as a competing contract.
//!
//! Three structural rules hold across this module.
//!
//! * **Model output and caller data never create authority.** Host-resolved
//!   fields (Capability Protocol §4.2: `capability_version`, `risk_class`,
//!   `side_effect_class`, `required_authorization`, `provider_id`,
//!   `arguments_digest`, `data_class`, `deadline_ms`) are either absent from a
//!   type or are non-optional fields a caller must supply explicitly. No
//!   request, result, task, step, model, event, or descriptor type here derives
//!   `Default`, and no convenience constructor fills in an authority-bearing
//!   field. (`Trace` does derive `Default`, which is safe: it carries no
//!   authority and every member is independently optional.)
//! * **Closed enum sets fail closed.** Every enum is derived with the frozen
//!   set and rejects an unrecognised variant on parse (Protocol Index §4.2).
//! * **Shared surfaces stay forward-compatible.** `Envelope` and `SereaEvent`
//!   retain unknown fields in an opaque extension set and round-trip them
//!   unchanged (Protocol Index §4.2 rule 3, §5). The action surfaces are
//!   closed instead, because an unknown field there must be rejected before any
//!   policy or provider decision.

use std::collections::BTreeMap;
use std::fmt;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::errors::{ContractRule, ProtocolError, ValueField, ValueRejection};
use crate::ids::{
    CapabilityId, Digest, EventId, IdempotencyKey, ImplementationId, ProviderId, ReceiptId,
    RequestId, StepId, TaskId,
};

/// The ceiling for machine-readable codes, kept tight so a code stays a code.
const MAX_CODE_LENGTH: usize = 64;
/// The ceiling for a capability's schema reference.
const MAX_SCHEMA_REFERENCE_LENGTH: usize = 512;

// ---------------------------------------------------------------------------
// Validated scalar values
// ---------------------------------------------------------------------------

/// Rejects control characters so an identifier, code, or label can never carry
/// a newline or a NUL into a log line or a comparison.
fn has_control_characters(value: &str) -> bool {
    value.chars().any(char::is_control)
}

fn reject_empty_or_long(field: ValueField, value: &str, limit: usize) -> Result<(), ProtocolError> {
    reject_empty(field, value)?;
    if value.len() > limit {
        return Err(malformed(field, ValueRejection::TooLong));
    }
    Ok(())
}

fn reject_empty(field: ValueField, value: &str) -> Result<(), ProtocolError> {
    if value.is_empty() || value.trim().is_empty() {
        return Err(malformed(field, ValueRejection::Empty));
    }
    Ok(())
}

fn malformed(field: ValueField, reason: ValueRejection) -> ProtocolError {
    ProtocolError::MalformedValue { field, reason }
}

/// Declares a validated protocol value: a validating constructor, a validating
/// `FromStr`, `Display`, and a `Deserialize` that routes through the same
/// validator so a value that cannot be constructed also cannot be parsed.
macro_rules! declare_value {
    (
        $(#[$meta:meta])* $name:ident, $field:ident, $validate:expr
    ) => {
        $(#[$meta])*
        #[derive(Debug, Clone, PartialEq, Eq, Hash)]
        pub struct $name(String);

        impl $name {
            /// The frozen field this value occupies, for machine-readable
            /// errors.
            pub const FIELD: ValueField = ValueField::$field;

            /// Validates `value` against this field's frozen shape.
            pub fn new(value: impl Into<String>) -> Result<Self, ProtocolError> {
                #[allow(clippy::redundant_closure_call)]
                let validator: fn(String) -> Result<String, ProtocolError> = $validate;
                validator(value.into()).map(Self)
            }

            /// The exact wire form.
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&self.0)
            }
        }

        impl std::str::FromStr for $name {
            type Err = ProtocolError;
            fn from_str(value: &str) -> Result<Self, Self::Err> {
                Self::new(value)
            }
        }

        impl Serialize for $name {
            fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
                serializer.serialize_str(&self.0)
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                let raw = String::deserialize(deserializer)?;
                Self::new(raw).map_err(serde::de::Error::custom)
            }
        }
    };
}

/// Validates an uppercase `SHOUTING_SNAKE` machine-readable code.
///
/// Event Protocol §4 requires these to be stable machine-readable codes and
/// never free text; the frozen documents give them no closed set, so P1
/// validates the code shape rather than inventing one.
fn validate_code(field: ValueField, value: String) -> Result<String, ProtocolError> {
    reject_empty_or_long(field, &value, MAX_CODE_LENGTH)?;
    // `^[A-Z][A-Z0-9_]*$`, the exact pattern the checked-in schemas require of a
    // machine-readable code. A leading underscore or digit is not a code, and
    // accepting one here would make the Rust type admit what every schema
    // refuses.
    let mut characters = value.chars();
    let starts_correctly = characters.next().is_some_and(|c| c.is_ascii_uppercase());
    let rest_is_correct =
        characters.all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_');
    if starts_correctly && rest_is_correct && !has_control_characters(&value) {
        Ok(value)
    } else {
        Err(malformed(field, ValueRejection::Malformed))
    }
}

/// The three text categories frozen by ADR-0023, without a length ceiling.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextCategory {
    /// O: opaque references; exact prefixed/fixed-shape identifier impersonations refused.
    Opaque,
    /// L: single-line labels; control characters and line separators refused.
    Label,
    /// P: prose; interior TAB, LF and Unicode line separators permitted.
    Prose,
}

fn is_text_whitespace(character: char) -> bool {
    matches!(character, '\u{0009}'..='\u{000d}' | '\u{0020}' | '\u{0085}' | '\u{00a0}'
        | '\u{1680}' | '\u{2000}'..='\u{200a}' | '\u{2028}' | '\u{2029}'
        | '\u{202f}' | '\u{205f}' | '\u{3000}')
}

impl TextCategory {
    /// Checks the pinned whitespace, control and identifier rules, without normalization.
    pub fn accepts(self, value: &str) -> bool {
        if value.is_empty()
            || value.starts_with(is_text_whitespace)
            || value.ends_with(is_text_whitespace)
        {
            return false;
        }
        let forbidden = value.chars().any(|c| match self {
            Self::Opaque | Self::Label => matches!(c, '\u{0000}'..='\u{001f}' | '\u{007f}'..='\u{009f}' | '\u{2028}' | '\u{2029}'),
            Self::Prose => matches!(c, '\u{0000}'..='\u{0008}' | '\u{000b}'..='\u{001f}' | '\u{007f}'..='\u{009f}'),
        });
        !forbidden
            && (self != Self::Opaque || !crate::ids::is_opaque_identifier_impersonation(value))
    }

    /// A complete JSON Schema string fragment with the generated ECMA-262 pattern.
    pub fn schema_fragment(self) -> Value {
        serde_json::json!({"type": "string", "pattern": text_pattern(self)})
    }
}

/// Generates the complete strict-EOF ECMA-262 pattern defined by ADR-0023.
/// Identifier exclusions are generated from `ids.rs`, never a copied prefix/verb list.
pub fn text_pattern(category: TextCategory) -> String {
    let whitespace =
        r"[\u0009-\u000d\u0020\u0085\u00a0\u1680\u2000-\u200a\u2028\u2029\u202f\u205f\u3000]";
    let end = r"(?![\s\S])";
    let mut pattern = format!(r"^(?!{whitespace})(?![\s\S]*{whitespace}{end})");
    if category == TextCategory::Opaque {
        pattern.push_str(&crate::ids::opaque_identifier_exclusion_pattern());
    }
    pattern.push_str(match category {
        TextCategory::Opaque | TextCategory::Label => r"[^\u0000-\u001f\u007f-\u009f\u2028\u2029]+",
        TextCategory::Prose => r"[^\u0000-\u0008\u000b-\u001f\u007f-\u009f]+",
    });
    pattern.push_str(end);
    pattern
}

fn validate_text(
    field: ValueField,
    category: TextCategory,
    value: String,
) -> Result<String, ProtocolError> {
    if value.is_empty() || value.chars().all(is_text_whitespace) {
        return Err(malformed(field, ValueRejection::Empty));
    }
    if !category.accepts(&value) {
        return Err(malformed(field, ValueRejection::Malformed));
    }
    Ok(value)
}

declare_value!(
    /// A `reason_code` on a task or step: a stable code, never prose.
    ReasonCode,
    ReasonCode,
    |value| validate_code(ValueField::ReasonCode, value)
);
declare_value!(
    /// A `blocked_reason`. `BLOCKED` is not terminal (Task Protocol §4.1), so
    /// this is a code the host resolves against durable state, not a message.
    BlockedReason,
    BlockedReason,
    |value| validate_code(ValueField::BlockedReason, value)
);
declare_value!(
    /// A task `failure_reason`, for example `BOUND_EXCEEDED_MODEL_CALLS`
    /// (Bounds Protocol §4.4).
    FailureReason,
    FailureReason,
    |value| validate_code(ValueField::FailureReason, value)
);
declare_value!(
    /// An `ActionError.code`, for example `GMAIL_HISTORY_EXPIRED`.
    ErrorCode,
    ErrorCode,
    |value| validate_code(ValueField::ErrorCode, value)
);
declare_value!(
    /// An `ActionError.host_action`, for example `FULL_RESYNC`.
    HostAction,
    HostAction,
    |value| validate_code(ValueField::HostAction, value)
);
declare_value!(
    /// A `ModelError` kind. Model Protocol §3.2 names `ModelError` and says it
    /// carries a kind, but freezes no variant set at P0, so P1 validates the
    /// code shape.
    ModelErrorCode,
    ModelErrorCode,
    |value| validate_code(ValueField::ModelErrorCode, value)
);
declare_value!(
    /// An `AssistantTask.origin.kind`. Task Protocol §2 shows `USER_MESSAGE`
    /// and freezes no closed set, so P1 validates the code shape.
    TaskOriginKind,
    TaskOriginKind,
    |value| validate_code(ValueField::TaskOriginKind, value)
);
declare_value!(
    /// A `TaskStep.status`. Task Protocol §3 shows `SUCCEEDED` and Bounds
    /// Protocol §4.4 names `RECONCILED_ABSENT`, without freezing a closed set.
    StepStatus,
    StepStatus,
    |value| validate_code(ValueField::StepStatus, value)
);
declare_value!(
    /// A conversation message role (`user`, `system`, …). Model Protocol §3
    /// shows `user` and freezes no closed set.
    MessageRole,
    TokenRole,
    |value: String| {
        reject_empty_or_long(ValueField::TokenRole, &value, MAX_CODE_LENGTH)?;
        let ok = value
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_');
        if ok {
            Ok(value)
        } else {
            Err(malformed(ValueField::TokenRole, ValueRejection::Malformed))
        }
    }
);
declare_value!(
    /// The `actor.id` of an event, such as `serea-core`. Event Protocol §2.1
    /// names `id` but gives it no grammar.
    ActorId,
    ActorId,
    |value| validate_text(ValueField::ActorId, TextCategory::Opaque, value)
);
declare_value!(
    /// A step `lease_owner`. Task Protocol §3.1 requires the field but names no
    /// value space.
    LeaseOwner,
    LeaseOwner,
    |value| validate_text(ValueField::LeaseOwner, TextCategory::Opaque, value)
);
declare_value!(
    /// A `SideEffectReceipt.provider_reference`: the external system's own
    /// handle for the affected object. It is opaque here; Serea stores what it
    /// is given and never parses it (GoalLatch Adapter §2 item 6, §6.4).
    ProviderReference,
    ProviderReference,
    |value| validate_text(ValueField::ProviderReference, TextCategory::Opaque, value)
);
declare_value!(
    /// A diagnostic `ActionError.message`. Diagnostic only: no control flow may
    /// depend on it (Event Protocol §4).
    ErrorMessage,
    ErrorMessage,
    |value| validate_text(ValueField::ErrorMessage, TextCategory::Prose, value)
);
declare_value!(
    /// An `AssistantTask.title`.
    TaskTitle,
    TaskTitle,
    |value| validate_text(ValueField::TaskTitle, TextCategory::Label, value)
);
declare_value!(
    /// A `CapabilityDescriptor.title`.
    DescriptorTitle,
    DescriptorTitle,
    |value| validate_text(ValueField::DescriptorTitle, TextCategory::Label, value)
);
declare_value!(
    /// A `CapabilityDescriptor.description`.
    DescriptorDescription,
    DescriptorDescription,
    |value| validate_text(ValueField::DescriptorDescription, TextCategory::Prose, value)
);
declare_value!(
    /// An `ApprovalRequest.plain_summary`: host-written from validated
    /// arguments, so a user can consent from the summary alone
    /// (Approval Protocol §2.1).
    PlainSummary,
    PlainSummary,
    |value| validate_text(ValueField::PlainSummary, TextCategory::Label, value)
);
declare_value!(
    /// A `SideEffectReceipt.effect_summary`.
    EffectSummary,
    EffectSummary,
    |value| validate_text(ValueField::EffectSummary, TextCategory::Label, value)
);

/// An RFC 3339 timestamp in UTC, exactly as every frozen example writes it:
/// `YYYY-MM-DDTHH:MM:SS`, optionally with three fractional digits, then `Z`.
///
/// P1 accepts only the UTC `Z` form because every value in the frozen documents
/// uses it and no rule requires an offset. The value is stored as its exact
/// wire form so serialization of an existing value is byte-identical. Equality
/// and hashing remain spelling-based: `…SSZ` differs from `…SS.000Z` even though
/// both denote the same instant. Epoch conversion preserves that instant and
/// reconstruction emits canonical `.mmmZ`, not the original spelling.
///
/// This type is deliberately not orderable: wire byte ordering is not time
/// ordering across the two spellings. Compare [`crate::EpochMillis`] instead.
///
/// ```compile_fail
/// use serea_protocol::Timestamp;
/// fn requires_ord<T: Ord>() {}
/// requires_ord::<Timestamp>();
/// ```
///
/// ```compile_fail
/// use serea_protocol::Timestamp;
/// fn requires_partial_ord<T: PartialOrd>() {}
/// requires_partial_ord::<Timestamp>();
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Timestamp(String);

impl Timestamp {
    /// Parses and validates the frozen wire form.
    pub fn new(value: impl Into<String>) -> Result<Self, ProtocolError> {
        let value = value.into();
        validate_timestamp(&value)?;
        Ok(Self(value))
    }

    /// The exact wire form.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Converts to signed Unix epoch milliseconds without changing this value's
    /// exact stored wire spelling. Total for every validated Timestamp,
    /// including all legal dates before 1970 and year 0000.
    pub fn to_epoch_millis(&self) -> crate::EpochMillis {
        crate::clock::to_epoch_millis(self)
    }

    /// Reconstructs the instant with the canonical `YYYY-MM-DDTHH:MM:SS.mmmZ`
    /// spelling, always including three fractional digits. The original
    /// seconds-versus-milliseconds spelling is not retained in an epoch value.
    /// Total because EpochMillis is bounded to the Timestamp wire domain.
    pub fn from_epoch_millis(epoch: crate::EpochMillis) -> Self {
        Self::new(crate::clock::canonical_wire(epoch)).unwrap_or_else(|error| {
            unreachable!("validated EpochMillis produces a legal wire timestamp: {error:?}")
        })
    }
}

impl fmt::Display for Timestamp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl TryFrom<String> for Timestamp {
    type Error = ProtocolError;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl From<Timestamp> for String {
    fn from(value: Timestamp) -> Self {
        value.0
    }
}

impl std::str::FromStr for Timestamp {
    type Err = ProtocolError;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::new(value)
    }
}

/// Validates `YYYY-MM-DDTHH:MM:SS[.mmm]Z` including real calendar days, so a
/// `2026-02-30T00:00:00.000Z` is refused rather than stored.
fn validate_timestamp(value: &str) -> Result<(), ProtocolError> {
    let reject = || malformed(ValueField::Timestamp, ValueRejection::Malformed);
    let bytes = value.as_bytes();
    let zone_index = match bytes.len() {
        20 => 19,
        24 => 23,
        _ => return Err(reject()),
    };
    let digits_at = |range: std::ops::Range<usize>| -> Option<u32> {
        range.clone().try_fold(0u32, |acc, index| {
            let digit = bytes.get(index)?;
            if digit.is_ascii_digit() {
                Some(acc * 10 + u32::from(digit - b'0'))
            } else {
                None
            }
        })
    };
    let separator = |index: usize, expected: u8| match bytes.get(index) {
        Some(actual) if *actual == expected => Ok(()),
        _ => Err(reject()),
    };
    separator(4, b'-')?;
    separator(7, b'-')?;
    separator(10, b'T')?;
    separator(13, b':')?;
    separator(16, b':')?;
    separator(zone_index, b'Z')?;
    if bytes.len() == 24 {
        separator(19, b'.')?;
    }
    let (Some(year), Some(month), Some(day)) = (digits_at(0..4), digits_at(5..7), digits_at(8..10))
    else {
        return Err(reject());
    };
    let (Some(hour), Some(minute), Some(second)) =
        (digits_at(11..13), digits_at(14..16), digits_at(17..19))
    else {
        return Err(reject());
    };
    if bytes.len() == 24 && digits_at(20..23).is_none() {
        return Err(reject());
    }

    if !(1..=12).contains(&month) || day < 1 || day > days_in_month(year, month) {
        return Err(reject());
    }
    if hour > 23 || minute > 59 || second > 59 {
        return Err(reject());
    }
    Ok(())
}

/// The number of days in `month`, honouring the Gregorian leap rule.
fn days_in_month(year: u32, month: u32) -> u32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if is_leap_year(year) => 29,
        2 => 28,
        _ => 0,
    }
}

/// The proleptic Gregorian leap rule.
fn is_leap_year(year: u32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}

/// A `serea.<surface>/<major>` wire protocol version
/// (Protocol Index §4, §5).
///
/// The surface is a validated opaque name rather than a closed enum: the
/// registry grows with an architecture-minor change, and Protocol Index §4.2
/// rule 1 requires a consumer to reject a major it does not implement, which
/// is a consumer capability P1 does not have.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct WireSurface(String);

impl WireSurface {
    /// The `serea.action/2` surface (Capability Protocol).
    pub const ACTION: &'static str = "serea.action/2";
    /// The `serea.task/2` surface (Task Protocol).
    pub const TASK: &'static str = "serea.task/2";
    /// The `serea.model/1` surface (Model Protocol).
    pub const MODEL: &'static str = "serea.model/1";
    /// The `serea.event/1` surface (Event Protocol).
    pub const EVENT: &'static str = "serea.event/1";
    /// The `serea.policy/1` surface (Policy Protocol).
    pub const POLICY: &'static str = "serea.policy/1";
    /// The `serea.approval/1` surface (Approval Protocol).
    pub const APPROVAL: &'static str = "serea.approval/1";
    /// The `serea.device/1` surface (Device Protocol).
    pub const DEVICE: &'static str = "serea.device/1";
    /// The `serea.goallatch/1` adapter-internal surface (GoalLatch Adapter §10).
    pub const GOALLATCH: &'static str = "serea.goallatch/1";
    /// The `serea.data/1` surface (Data Classification Protocol).
    pub const DATA: &'static str = "serea.data/1";
    /// The `serea.bounds/1` surface (Bounds Protocol).
    pub const BOUNDS: &'static str = "serea.bounds/1";
    /// The `serea.scheduler/1` wire surface, not a scheduling runtime capability.
    pub const SCHEDULER: &'static str = "serea.scheduler/1";

    /// Every supported surface/version pair registered at `serea-arch/1.0.0`.
    pub const ALL: [&'static str; 11] = [
        Self::ACTION,
        Self::TASK,
        Self::MODEL,
        Self::POLICY,
        Self::APPROVAL,
        Self::EVENT,
        Self::DEVICE,
        Self::GOALLATCH,
        Self::DATA,
        Self::BOUNDS,
        Self::SCHEDULER,
    ];

    /// Whether this exact surface/version pair is in this build's registry.
    /// Unknown names and unsupported majors fail closed, including old task/action majors.
    pub fn is_supported(&self) -> bool {
        Self::ALL.contains(&self.as_str())
    }

    /// Parses and validates a surface name.
    pub fn new(value: impl Into<String>) -> Result<Self, ProtocolError> {
        let value = value.into();
        let malformed = || malformed(ValueField::WireSurface, ValueRejection::Malformed);
        let Some((prefix, major)) = value.rsplit_once('/') else {
            return Err(malformed());
        };
        let Some(surface) = prefix.strip_prefix("serea.") else {
            return Err(malformed());
        };
        let surface_ok = !surface.is_empty()
            && surface
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '.');
        let major_ok = matches!(major.as_bytes().first(), Some(b'1'..=b'9'))
            && major.bytes().all(|b| b.is_ascii_digit());
        if surface_ok && major_ok && value.len() <= MAX_CODE_LENGTH {
            Ok(Self(value))
        } else {
            Err(malformed())
        }
    }

    /// The exact wire form.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// The major from the `serea.<surface>/<major>` form, or `None` when it is
    /// not a number this build can represent. `None` is treated as unsupported
    /// everywhere it matters, so an overflowed major fails closed.
    pub fn major(&self) -> Option<u32> {
        match self.0.rsplit_once('/') {
            Some((_, major)) => major.parse().ok(),
            None => None,
        }
    }

    /// The surface name without its major.
    pub fn name(&self) -> &str {
        match self.0.split_once('/') {
            Some((name, _)) => name,
            None => &self.0,
        }
    }
}

impl fmt::Display for WireSurface {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl TryFrom<String> for WireSurface {
    type Error = ProtocolError;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl From<WireSurface> for String {
    fn from(value: WireSurface) -> Self {
        value.0
    }
}

/// The major envelope version carried as a decimal string (Protocol Index §6).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct EnvelopeVersion(u32);

impl EnvelopeVersion {
    /// The only major P1 implements. A payload declaring a different major is a
    /// consumer-support question, not something P1 can answer, so every ingress
    /// must call [`EnvelopeVersion::is_supported`] on what it parsed.
    pub const SUPPORTED_MAJOR: u32 = 1;

    /// Parses and validates a decimal major with no leading zero.
    pub fn new(value: impl Into<String>) -> Result<Self, ProtocolError> {
        let value = value.into();
        let reject = || {
            malformed(
                ValueField::EnvelopeVersion,
                if value.is_empty() {
                    ValueRejection::Empty
                } else {
                    ValueRejection::Malformed
                },
            )
        };
        if !matches!(value.as_bytes().first(), Some(b'1'..=b'9')) {
            return Err(reject());
        }
        if !value.bytes().all(|b| b.is_ascii_digit()) {
            return Err(reject());
        }
        value.parse::<u32>().map(Self).map_err(|_| reject())
    }

    /// The major as a number.
    pub fn major(self) -> u32 {
        self.0
    }

    /// Whether this is the major P1 implements.
    ///
    /// Protocol Index §4.2 rule 1: "A consumer must reject a payload whose major
    /// wire-protocol version it does not implement. Silent downgrade is
    /// forbidden." The check lives here so it cannot be forgotten; the ingress
    /// that must call it is the device link in `serea-core` (P12).
    pub fn is_supported(self) -> bool {
        self.0 == Self::SUPPORTED_MAJOR
    }
}

impl fmt::Display for EnvelopeVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl TryFrom<String> for EnvelopeVersion {
    type Error = ProtocolError;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl From<EnvelopeVersion> for String {
    fn from(value: EnvelopeVersion) -> Self {
        value.0.to_string()
    }
}

/// A SemVer version: the capability-version axis (Protocol Index §4).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct SemVer(String);

impl SemVer {
    /// Parses and validates `major.minor.patch` with optional SemVer
    /// pre-release and build metadata.
    pub fn new(value: impl Into<String>) -> Result<Self, ProtocolError> {
        let value = value.into();
        let reject = || {
            malformed(
                ValueField::CapabilityVersion,
                if value.is_empty() {
                    ValueRejection::Empty
                } else {
                    ValueRejection::Malformed
                },
            )
        };
        if value.is_empty() || value.len() > MAX_CODE_LENGTH {
            return Err(reject());
        }
        let (before_build, build) = match value.split_once('+') {
            Some((head, build)) => (head, Some(build)),
            None => (value.as_str(), None),
        };
        let (core, pre_release) = match before_build.split_once('-') {
            Some((core, rest)) => (core, Some(rest)),
            None => (before_build, None),
        };
        let numbers: Vec<&str> = core.split('.').collect();
        if numbers.len() != 3 || !numbers.iter().all(|n| is_numeric_identifier(n)) {
            return Err(reject());
        }
        for metadata in [pre_release, build] {
            if metadata.is_some_and(|metadata| !is_dot_separated_identifier(metadata)) {
                return Err(reject());
            }
        }
        Ok(Self(value))
    }

    /// The exact wire form.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for SemVer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl TryFrom<String> for SemVer {
    type Error = ProtocolError;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl From<SemVer> for String {
    fn from(value: SemVer) -> Self {
        value.0
    }
}

/// True for a decimal quantity with no sign, no exponent, and no leading zero
/// unless the value is exactly `0` — the form the checked-in schemas require.
fn is_decimal_quantity(value: &str) -> bool {
    !value.is_empty()
        && value.bytes().all(|b| b.is_ascii_digit())
        && (value.len() == 1 || !value.starts_with('0'))
}

/// A SemVer numeric identifier: digits only, no leading zero unless the value
/// is exactly `0`.
fn is_numeric_identifier(value: &str) -> bool {
    !value.is_empty()
        && value.bytes().all(|b| b.is_ascii_digit())
        && (value.len() == 1 || !value.starts_with('0'))
}

/// A dot-separated SemVer alphanumeric identifier.
fn is_dot_separated_identifier(value: &str) -> bool {
    value.split('.').all(|part| {
        !part.is_empty() && part.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
    })
}

/// A gapless event sequence number, serialized as a decimal **string** because
/// it will exceed JavaScript's exact integer range (Protocol Index §5;
/// Event Protocol §2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Seq(u64);

impl Seq {
    /// Wraps a host-assigned sequence number. `seq` is assigned at commit time
    /// inside the state change's transaction (Event Protocol §2), which is P3
    /// work; P1 carries the value only.
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    /// The sequence number.
    pub const fn get(self) -> u64 {
        self.0
    }
}

impl fmt::Display for Seq {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl TryFrom<String> for Seq {
    type Error = ProtocolError;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        if !is_decimal_quantity(&value) {
            return Err(malformed(
                ValueField::SequenceNumber,
                ValueRejection::Malformed,
            ));
        }
        value
            .parse::<u64>()
            .map(Self)
            .map_err(|_| malformed(ValueField::SequenceNumber, ValueRejection::OutOfRange))
    }
}

impl From<Seq> for String {
    fn from(value: Seq) -> Self {
        value.0.to_string()
    }
}

/// A counted model-usage quantity, serialized as a decimal **string**
/// (Protocol Index §5 names `model_usage` counts alongside `seq`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct TokenCount(u64);

impl TokenCount {
    /// Wraps a counted quantity. Counts are counted, never estimated
    /// (Bounds Protocol §7.2).
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    /// The counted quantity.
    pub const fn get(self) -> u64 {
        self.0
    }
}

impl fmt::Display for TokenCount {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl TryFrom<String> for TokenCount {
    type Error = ProtocolError;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        if !is_decimal_quantity(&value) {
            return Err(malformed(ValueField::TokenCount, ValueRejection::Malformed));
        }
        value
            .parse::<u64>()
            .map(Self)
            .map_err(|_| malformed(ValueField::TokenCount, ValueRejection::OutOfRange))
    }
}

impl From<TokenCount> for String {
    fn from(value: TokenCount) -> Self {
        value.0.to_string()
    }
}

/// An opaque reference to a secret in the OS credential store — never the
/// secret (Data Classification §3.1, `DC6`).
///
/// The handle reuses the frozen `Digest` wire form rather than introducing a
/// new identifier shape, so it reveals no keychain service, account, path, or
/// slot index. It is safe to log: the egress matrix permits a handle in logs
/// and denies bytes (Data Classification §5).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "Digest", into = "Digest")]
pub struct CredentialHandle(Digest);

impl CredentialHandle {
    /// Reuses a digest as a handle wire form. The caller supplies the digest;
    /// P1 does not derive one, because derivation needs a hashing primitive and
    /// secret custody, both of which belong to `serea-credential-store`.
    pub fn new(digest: Digest) -> Self {
        Self(digest)
    }

    /// The exact wire form.
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }

    /// The underlying digest form, for a credential store to resolve.
    pub fn digest(&self) -> &Digest {
        &self.0
    }
}

impl From<Digest> for CredentialHandle {
    fn from(value: Digest) -> Self {
        Self(value)
    }
}

impl From<CredentialHandle> for Digest {
    fn from(value: CredentialHandle) -> Self {
        value.0
    }
}

// ---------------------------------------------------------------------------
// Frozen enum sets
// ---------------------------------------------------------------------------

/// Declares a frozen, closed enum set whose wire form is the uppercase name.
macro_rules! declare_enum {
    (
        $(#[$meta:meta])* $name:ident { $($(#[$vmeta:meta])* $variant:ident => $wire:literal),* $(,)? }
    ) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
        pub enum $name {
            $(
                $(#[$vmeta])*
                #[serde(rename = $wire)]
                $variant,
            )*
        }

        impl $name {
            /// Every wire name in the frozen set, in declaration order.
            pub const WIRE_NAMES: &'static [&'static str] = &[$($wire),*];
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(self.wire_name())
            }
        }

        impl $name {
            /// The exact wire form.
            pub fn wire_name(self) -> &'static str {
                match self { $(Self::$variant => $wire),* }
            }
        }
    };
}

declare_enum!(
    /// The frozen `DataClass` set (Data Classification §2). Exactly five
    /// values; adding one is an architecture-major change.
    DataClass {
        /// Published by its owner; no disclosure harm.
        Public => "PUBLIC",
        /// Identifies the user to the user.
        Personal => "PERSONAL",
        /// Identifies or is sensitive about someone other than the user.
        Private => "PRIVATE",
        /// Disclosing would harm the user or a third party materially, and it is
        /// not an authentication credential.
        Secret => "SECRET",
        /// Authentication and authorization material.
        Credential => "CREDENTIAL",
    }
);

impl DataClass {
    /// The frozen harm ordering: `PUBLIC < PERSONAL < PRIVATE < SECRET <
    /// CREDENTIAL`.
    pub const fn rank(self) -> u8 {
        match self {
            DataClass::Public => 0,
            DataClass::Personal => 1,
            DataClass::Private => 2,
            DataClass::Secret => 3,
            DataClass::Credential => 4,
        }
    }

    /// The higher of two classes. Composition is `max`, which is always defined
    /// because the set is totally ordered (Data Classification §2.1, §2.2).
    pub fn compose(a: Self, b: Self) -> Self {
        if a.rank() >= b.rank() { a } else { b }
    }

    /// Composes a sequence, returning [`DataClass::Credential`] for an empty
    /// sequence because an unclassified value is treated as `CREDENTIAL`
    /// (Data Classification §2.1, absence rule).
    pub fn compose_all<I: IntoIterator<Item = Self>>(classes: I) -> Self {
        let mut classes = classes.into_iter();
        match classes.next() {
            None => DataClass::Credential,
            Some(first) => classes.fold(first, Self::compose),
        }
    }
}

declare_enum!(
    /// The frozen `RiskClass` set (Policy Protocol §2). A capability has
    /// exactly one class, and a model may not select it (`C3`, `INV-SEC-03`).
    RiskClass {
        /// Reads state. No external effect.
        Observe => "OBSERVE",
        /// Mutates Serea's own durable state.
        LocalState => "LOCAL_STATE",
        /// External effect with a defined inverse.
        ReversibleWrite => "REVERSIBLE_WRITE",
        /// External effect without a reliable inverse.
        ExternalWrite => "EXTERNAL_WRITE",
        /// Content leaves the system to a third party.
        Communication => "COMMUNICATION",
        /// Requires privileges above the app's normal grant.
        ElevatedDevice => "ELEVATED_DEVICE",
        /// Irreversible removal of data or capability.
        Destructive => "DESTRUCTIVE",
        /// Touches secret material.
        Credential => "CREDENTIAL",
    }
);

impl RiskClass {
    /// The frozen class ordering from Policy Protocol §2.
    pub const fn rank(self) -> u8 {
        match self {
            RiskClass::Observe => 0,
            RiskClass::LocalState => 1,
            RiskClass::ReversibleWrite => 2,
            RiskClass::ExternalWrite => 3,
            RiskClass::Communication => 4,
            RiskClass::ElevatedDevice => 5,
            RiskClass::Destructive => 6,
            RiskClass::Credential => 7,
        }
    }

    /// Whether this class exceeds `ceiling`.
    ///
    /// A task's `policy_class` is an immutable ceiling no step may exceed
    /// (Task Protocol §2.1, `T6`).
    pub const fn exceeds(self, ceiling: Self) -> bool {
        self.rank() > ceiling.rank()
    }
}

declare_enum!(
    /// What changes in the world, independent of risk (Capability Protocol
    /// §3.1).
    SideEffectClass {
        /// Pure read of durable or cached state.
        None => "NONE",
        /// Mutates Serea's own durable state only.
        LocalState => "LOCAL_STATE",
        /// Changes observable device UI or device-side settings.
        DeviceState => "DEVICE_STATE",
        /// Creates or changes state visible outside this host.
        ExternalWrite => "EXTERNAL_WRITE",
        /// Transmits content to a party outside this host.
        Communication => "COMMUNICATION",
        /// Requires privileges above the app's normal grant.
        ElevatedDevice => "ELEVATED_DEVICE",
    }
);

declare_enum!(
    /// Governs automatic retry (Capability Protocol §3.1, §8.1).
    ReplaySafety {
        /// Automatic retry on a retryable failure and on `AMBIGUOUS` is allowed.
        Idempotent => "IDEMPOTENT",
        /// Only after the provider confirms no effect.
        Conditional => "CONDITIONAL",
        /// Never retried; reconcile or block.
        NonReplayable => "NON_REPLAYABLE",
    }
);

declare_enum!(
    /// `required_authorization` on a descriptor (Capability Protocol §3.1).
    Authorization {
        /// No approval required.
        None => "NONE",
        /// A human on a paired device.
        DeviceUser => "DEVICE_USER",
        /// A bounded, expiring, task-bound grant.
        ScopedGrant => "SCOPED_GRANT",
        /// A human channel that can handle credentials; Serea never handles raw
        /// credentials in band (Policy Protocol §3.2).
        CredentialHandoff => "CREDENTIAL_HANDOFF",
    }
);

declare_enum!(
    /// `root_requirement` on a descriptor (Capability Protocol §3.1).
    RootRequirement {
        /// No root involved.
        NotRequired => "NOT_REQUIRED",
        /// Registered in both variants behind one `CapabilityId`.
        OptionalRoot => "OPTIONAL_ROOT",
        /// Root is required.
        RequiresRoot => "REQUIRES_ROOT",
    }
);

declare_enum!(
    /// `idempotency_support` on a descriptor (Capability Protocol §3.1).
    IdempotencySupport {
        /// The provider dedupes by key.
        Native => "NATIVE",
        /// The host records the key and suppresses duplicates.
        Emulated => "EMULATED",
        /// No duplicate suppression for this capability.
        None => "NONE",
    }
);

declare_enum!(
    /// `cost_class` on a descriptor (Capability Protocol §3.1). Advisory for
    /// policy; never used to select a model.
    CostClass {
        /// No cost.
        Free => "FREE",
        /// Low cost.
        Low => "LOW",
        /// Paid.
        Paid => "PAID",
    }
);

declare_enum!(
    /// `requested_by` on an `ActionRequest`: provenance for audit that **never**
    /// grants authority (Capability Protocol §4.1).
    RequestedBy {
        /// Synthesized from validated model output. Authority: zero.
        Model => "MODEL",
        /// Directly instructed by the device user in this session.
        User => "USER",
        /// Triggered by a durable schedule wake.
        Scheduler => "SCHEDULER",
        /// Triggered by the read-only proactive watcher.
        ProactiveWatcher => "PROACTIVE_WATCHER",
        /// Host-internal maintenance.
        System => "SYSTEM",
    }
);

declare_enum!(
    /// `ActionResult.status` (Capability Protocol §5).
    ActionStatus {
        /// The provider performed the action.
        Succeeded => "SUCCEEDED",
        /// The provider attempted the action and it failed.
        Failed => "FAILED",
        /// Refused before invocation.
        Rejected => "REJECTED",
        /// Aborted at a deadline or by cancellation.
        Cancelled => "CANCELLED",
        /// The capability exists but its backing condition is absent. A
        /// success-adjacent outcome, not an error path.
        Unavailable => "UNAVAILABLE",
        /// An equivalent action already completed; the prior receipt is
        /// returned and no new effect occurred.
        DuplicateSuppressed => "DUPLICATE_SUPPRESSED",
    }
);

declare_enum!(
    /// The frozen `ActionErrorKind` set (Capability Protocol §6.1). Frozen at
    /// P0; Bounds Protocol §4.4 explicitly records that bound exhaustion does
    /// **not** add a member here.
    ActionErrorKind {
        /// Arguments failed schema or host-side validation.
        Validation => "VALIDATION",
        /// Not in the registry, or the version is unsupported.
        UnknownCapability => "UNKNOWN_CAPABILITY",
        /// The policy engine returned `DENY`.
        PolicyDenied => "POLICY_DENIED",
        /// Approval is needed and was not granted.
        ApprovalRequired => "APPROVAL_REQUIRED",
        /// A human rejected the request. Terminal for the step.
        ApprovalDenied => "APPROVAL_DENIED",
        /// A backing condition is absent.
        CapabilityUnavailable => "CAPABILITY_UNAVAILABLE",
        /// An external provider failed.
        ProviderError => "PROVIDER_ERROR",
        /// The deadline was exceeded.
        ProviderTimeout => "PROVIDER_TIMEOUT",
        /// The provider throttled the host.
        RateLimited => "RATE_LIMITED",
        /// A credential needs re-authorization.
        AuthExpired => "AUTH_EXPIRED",
        /// An equivalent action already completed.
        DuplicateSuppressed => "DUPLICATE_SUPPRESSED",
        /// The effect may or may not have occurred. Never blindly retried.
        Ambiguous => "AMBIGUOUS",
        /// A host fault.
        Internal => "INTERNAL",
    }
);

declare_enum!(
    /// The capability-owned evidence vocabulary (Capability Protocol §7).
    /// Distinct from `EventKind`; the Event Protocol owns event kinds.
    EvidenceKind {
        /// The host recorded an action attempt, including a denied one.
        ActionAttempted => "ACTION_ATTEMPTED",
        /// A provider supplied a receipt proving an effect.
        ProviderReceipt => "PROVIDER_RECEIPT",
        /// A validated observation with no external effect.
        CapabilityObservation => "CAPABILITY_OBSERVATION",
        /// A `HostGoalProvider` returned a validated delegated-goal result.
        GoalResult => "GOAL_RESULT",
        /// Policy refused the action request.
        PolicyDenial => "POLICY_DENIAL",
        /// A read-back resolved an ambiguous effect.
        Reconciliation => "RECONCILIATION",
    }
);

declare_enum!(
    /// `AssistantTask.kind` (Task Protocol §2).
    TaskKind {
        /// Direct user request.
        UserRequest => "USER_REQUEST",
        /// Created by a durable schedule wake.
        Scheduled => "SCHEDULED",
        /// Created by the read-only proactive watcher.
        Proactive => "PROACTIVE",
        /// Delegates work to a host goal through `host.goal.*`.
        DelegatedHostGoal => "DELEGATED_HOST_GOAL",
        /// Host-internal maintenance.
        Maintenance => "MAINTENANCE",
    }
);

declare_enum!(
    /// The frozen task state machine (Task Protocol §4.1).
    TaskState {
        /// Accepted and durably recorded; no work begun.
        Received => "RECEIVED",
        /// Deriving a step plan.
        Planning => "PLANNING",
        /// Plan complete and ready to execute.
        Ready => "READY",
        /// One or more steps in flight.
        Executing => "EXECUTING",
        /// Blocked on a human or a scoped grant.
        WaitingApproval => "WAITING_APPROVAL",
        /// Blocked on user input that is not an approval.
        WaitingUser => "WAITING_USER",
        /// Confirming the outcome of completed steps.
        Verifying => "VERIFYING",
        /// Terminal success.
        Completed => "COMPLETED",
        /// Terminal failure.
        Failed => "FAILED",
        /// Cannot proceed without an external change. **Not** terminal.
        Blocked => "BLOCKED",
        /// Terminal, user- or system-initiated.
        Cancelled => "CANCELLED",
    }
);

impl TaskState {
    /// `COMPLETED`, `FAILED` and `CANCELLED` are terminal; `BLOCKED` is not
    /// (Task Protocol §4.1, `T8`).
    pub const fn is_terminal(self) -> bool {
        matches!(
            self,
            TaskState::Completed | TaskState::Failed | TaskState::Cancelled
        )
    }
}

declare_enum!(
    /// `TaskStep.kind` (Task Protocol §3).
    StepKind {
        /// Invoke a capability.
        Capability => "CAPABILITY",
        /// A model turn.
        ModelTurn => "MODEL_TURN",
        /// Wait for an approval decision.
        WaitApproval => "WAIT_APPROVAL",
        /// Wait for non-approval user input.
        WaitUser => "WAIT_USER",
        /// Wait for a durable schedule.
        WaitSchedule => "WAIT_SCHEDULE",
        /// Verify completed steps.
        Verify => "VERIFY",
        /// Emit a notification.
        Notify => "NOTIFY",
        /// Delegate to a host goal.
        Delegate => "DELEGATE",
    }
);

declare_enum!(
    /// `Actor.kind` (Event Protocol §2.1). A `MODEL` actor records involvement,
    /// never authority (`E10`, `INV-SEC-02`).
    ActorKind {
        /// Serea Core itself.
        Host => "HOST",
        /// A human on a named device.
        User => "USER",
        /// A model call, quoted by `model_id`.
        Model => "MODEL",
        /// An external provider.
        Provider => "PROVIDER",
        /// A durable schedule firing.
        Scheduler => "SCHEDULER",
        /// Maintenance.
        System => "SYSTEM",
    }
);

declare_enum!(
    /// `ModelRequest.purpose` (Model Protocol §3). Host-assigned from the
    /// task phase and step role, never chosen by the model.
    ModelPurpose {
        /// Free-text assistant conversation.
        Chat => "CHAT",
        /// Deriving or revising a plan.
        Planning => "PLANNING",
        /// Explicit memory extraction.
        Extraction => "EXTRACTION",
        /// Analysis of already-obtained data.
        Analysis => "ANALYSIS",
        /// Proactive watcher work.
        Proactive => "PROACTIVE",
        /// A bounded structured-output repair call.
        StructuredRepair => "STRUCTURED_REPAIR",
    }
);

declare_enum!(
    /// `response_format.type` (Model Protocol §3.1). There is no third option
    /// and no "parse the prose and hope" path.
    ResponseFormatType {
        /// Free text for `CHAT` only; never parsed for authority.
        Text => "TEXT",
        /// Output must validate against `schema` or the call fails.
        JsonSchema => "JSON_SCHEMA",
    }
);

declare_enum!(
    /// `ModelResponse.finish_reason` (Model Protocol §4).
    FinishReason {
        /// The model stopped on its own terms; still only a proposal.
        Stop => "STOP",
        /// Output was truncated by `max_output_tokens_per_call`.
        Length => "LENGTH",
        /// A provider content filter fired.
        ContentFilter => "CONTENT_FILTER",
        /// A provider-side error.
        Error => "ERROR",
        /// Structured output failed validation.
        StructureInvalid => "STRUCTURE_INVALID",
    }
);

declare_enum!(
    /// `json_schema_mode` (Model Protocol §5). Only `STRICT` models may serve
    /// `PLANNING` and `EXTRACTION` (`M9`).
    JsonSchemaMode {
        /// The provider guarantees schema conformance.
        Strict => "STRICT",
        /// The provider is instructed but may deviate.
        BestEffort => "BEST_EFFORT",
        /// No structured output.
        Unsupported => "UNSUPPORTED",
    }
);

/// `ModelRequest.response_format` (Model Protocol §3.1). There is no third
/// option and no "parse the prose and hope" path.
///
/// Serialisation is hand-written because serde's internally tagged enum
/// representation silently ignores `deny_unknown_fields`, which would leave an
/// undeclared member accepted on a security-sensitive surface. This type is
/// closed exactly: `TEXT` carries no other member and `JSON_SCHEMA` carries
/// exactly `schema`.
#[derive(Debug, Clone, PartialEq)]
pub enum ResponseFormat {
    /// Free text for `CHAT` only. Its output is rendered to the user and is
    /// never parsed for authority.
    Text,
    /// Output must validate against `schema` or the call fails.
    JsonSchema {
        /// The host-defined JSON Schema 2020-12 the output must satisfy.
        schema: Value,
    },
}

impl ResponseFormat {
    /// The frozen `response_format.type` discriminator.
    pub fn type_name(&self) -> &'static str {
        match self {
            ResponseFormat::Text => "TEXT",
            ResponseFormat::JsonSchema { .. } => "JSON_SCHEMA",
        }
    }
}

impl Serialize for ResponseFormat {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeMap;
        let mut map = serializer.serialize_map(Some(2))?;
        map.serialize_entry("type", self.type_name())?;
        if let ResponseFormat::JsonSchema { schema } = self {
            map.serialize_entry("schema", schema)?;
        }
        map.end()
    }
}

impl<'de> Deserialize<'de> for ResponseFormat {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        use serde::de::Error as _;
        let raw = Value::deserialize(deserializer)?;
        let object = raw
            .as_object()
            .ok_or_else(|| D::Error::custom("response_format must be an object"))?;
        let kind = object
            .get("type")
            .and_then(Value::as_str)
            .ok_or_else(|| D::Error::custom("response_format.type must be a string"))?;
        match (kind, object.len()) {
            ("TEXT", 1) => Ok(ResponseFormat::Text),
            ("JSON_SCHEMA", 2) => match object.get("schema") {
                Some(schema) => Ok(ResponseFormat::JsonSchema {
                    schema: schema.clone(),
                }),
                None => Err(D::Error::custom("JSON_SCHEMA requires a schema member")),
            },
            ("TEXT", _) => Err(D::Error::custom(
                "response_format of type TEXT accepts no other member",
            )),
            ("JSON_SCHEMA", _) => Err(D::Error::custom(
                "response_format of type JSON_SCHEMA accepts only a schema member",
            )),
            _ => Err(D::Error::custom(
                "response_format.type must be TEXT or JSON_SCHEMA",
            )),
        }
    }
}

declare_enum!(
    /// `ProviderHealth` (Capability Protocol §9). A provider that cannot honour
    /// its declared descriptor marks itself `Degraded` and stops advertising
    /// rather than relaxing its output (`C7`).
    ProviderHealth {
        /// The default; the provider may serve.
        Ready => "READY",
        /// The provider cannot honour its descriptor and has withdrawn the
        /// capability.
        Degraded => "DEGRADED",
    }
);

declare_enum!(
    /// The frozen `SereaEvent.kind` set (Event Protocol §3.1–§3.10).
    EventKind {
        /// Task durably accepted.
        TaskCreated => "TASK_CREATED",
        /// Task left `RECEIVED` and began work, including on recovery.
        TaskStarted => "TASK_STARTED",
        /// Any legal state transition.
        TaskStateChanged => "TASK_STATE_CHANGED",
        /// Terminal success.
        TaskCompleted => "TASK_COMPLETED",
        /// Terminal failure with a `reason_code`.
        TaskFailed => "TASK_FAILED",
        /// Cancelled with a `cancelled_by`.
        TaskCancelled => "TASK_CANCELLED",
        /// Entered `BLOCKED` with a `blocked_reason`.
        TaskBlocked => "TASK_BLOCKED",
        /// Left `BLOCKED` or a waiting state.
        TaskResumed => "TASK_RESUMED",
        /// Before model dispatch.
        ModelCalled => "MODEL_CALLED",
        /// After a model response.
        ModelCompleted => "MODEL_COMPLETED",
        /// Provider error with a `ModelError` kind.
        ModelFailed => "MODEL_FAILED",
        /// Structured output failed validation.
        ModelOutputInvalid => "MODEL_OUTPUT_INVALID",
        /// A repair call succeeded.
        ModelRepaired => "MODEL_REPAIRED",
        /// Routing advanced to a different model.
        ModelFallback => "MODEL_FALLBACK",
        /// An `ActionRequest` was constructed.
        CapabilityRequested => "CAPABILITY_REQUESTED",
        /// A provider returned; status is in the payload.
        CapabilityCompleted => "CAPABILITY_COMPLETED",
        /// Policy returned a denial.
        CapabilityDenied => "CAPABILITY_DENIED",
        /// A backing condition is absent.
        CapabilityUnavailable => "CAPABILITY_UNAVAILABLE",
        /// An equivalent action was already done and was not repeated.
        CapabilityDuplicateSuppressed => "CAPABILITY_DUPLICATE_SUPPRESSED",
        /// A `SideEffectReceipt` was persisted.
        CapabilityReceiptRecorded => "CAPABILITY_RECEIPT_RECORDED",
        /// An `AMBIGUOUS` result was resolved by read-back.
        CapabilityReconciled => "CAPABILITY_RECONCILED",
        /// Model-authored data carried invalid host-resolved fields.
        ModelSchemaViolation => "MODEL_SCHEMA_VIOLATION",
        /// A permitted `SYSTEM` resync bypassed duplicate suppression.
        ToolDuplicateWindowBypassed => "TOOL_DUPLICATE_WINDOW_BYPASSED",
        /// An approval request was raised.
        ApprovalRequired => "APPROVAL_REQUIRED",
        /// A grant was created.
        ApprovalGranted => "APPROVAL_GRANTED",
        /// The user refused.
        ApprovalDenied => "APPROVAL_DENIED",
        /// A request expired unused.
        ApprovalExpired => "APPROVAL_EXPIRED",
        /// A use was consumed by a step.
        ApprovalConsumed => "APPROVAL_CONSUMED",
        /// A grant hit expiry with uses remaining.
        ApprovalExpiredUnused => "APPROVAL_EXPIRED_UNUSED",
        /// Rules or the disabled overlay changed.
        PolicyChanged => "POLICY_CHANGED",
        /// A host bound was hit; the payload carries `bound_name`.
        BoundExceeded => "BOUND_EXCEEDED",
        /// An administrator raised a bound, with a before/after diff.
        BoundsChanged => "BOUNDS_CHANGED",
        /// A request tried something the policy forbids.
        PolicyViolationAttempt => "POLICY_VIOLATION_ATTEMPT",
        /// The model-call budget was exhausted.
        ModelBudgetExhausted => "MODEL_BUDGET_EXHAUSTED",
        /// The configured fallback bound was exhausted.
        ModelFallbackExhausted => "MODEL_FALLBACK_EXHAUSTED",
        /// A device session was established.
        DeviceConnected => "DEVICE_CONNECTED",
        /// A session ended with a reason.
        DeviceDisconnected => "DEVICE_DISCONNECTED",
        /// A new device was bound.
        DevicePaired => "DEVICE_PAIRED",
        /// A device was removed.
        DeviceUnpaired => "DEVICE_UNPAIRED",
        /// A device reported its capability set.
        DeviceCapabilitiesReported => "DEVICE_CAPABILITIES_REPORTED",
        /// The host revoked the device credential and sessions.
        DeviceRevoked => "DEVICE_REVOKED",
        /// A memory item was created with provenance.
        MemoryItemWritten => "MEMORY_ITEM_WRITTEN",
        /// A memory item was superseded.
        MemoryItemUpdated => "MEMORY_ITEM_UPDATED",
        /// A memory item was removed with a reason.
        MemoryItemDeleted => "MEMORY_ITEM_DELETED",
        /// The right-to-delete cascade transaction committed; the payload
        /// carries the counts and evidence that make a partial failure visible
        /// (Data Classification §8.2 step 4, ADR-0017). It is the completion
        /// record of one cascade transaction, not a per-item deletion event, and
        /// it proves nothing about any effect outside that transaction.
        DeletionCascadeCompleted => "DELETION_CASCADE_COMPLETED",
        /// The proactive watcher produced a suggestion.
        ProposalCreated => "PROPOSAL_CREATED",
        /// The user dismissed a proposal.
        ProposalDismissed => "PROPOSAL_DISMISSED",
        /// A cursor predates retained history.
        EventHistoryExpired => "EVENT_HISTORY_EXPIRED",
        /// A missing sequence was detected inside retained committed history.
        EventSequenceCorruption => "EVENT_SEQUENCE_CORRUPTION",
        /// A schedule was durably created.
        ScheduleCreated => "SCHEDULE_CREATED",
        /// A schedule definition or state was durably changed.
        ScheduleUpdated => "SCHEDULE_UPDATED",
        /// A schedule was paused.
        SchedulePaused => "SCHEDULE_PAUSED",
        /// A paused schedule was resumed.
        ScheduleResumed => "SCHEDULE_RESUMED",
        /// A schedule was cancelled.
        ScheduleCancelled => "SCHEDULE_CANCELLED",
        /// A due occurrence was skipped under its frozen policy.
        ScheduleOccurrenceMissed => "SCHEDULE_OCCURRENCE_MISSED",
        /// A due occurrence was deduplicated and created its scheduled task.
        ScheduleTaskCreated => "SCHEDULE_TASK_CREATED",
        /// A per-wake catch-up ceiling was reached; occurrences stay queued.
        ScheduleCatchUpDeferred => "SCHEDULE_CATCH_UP_DEFERRED",
        /// An incremental sync cycle began.
        ProviderSyncStarted => "PROVIDER_SYNC_STARTED",
        /// A sync cycle finished with counts.
        ProviderSyncCompleted => "PROVIDER_SYNC_COMPLETED",
        /// A cycle fell back to full resynchronization.
        ProviderSyncDegraded => "PROVIDER_SYNC_DEGRADED",
    }
);

// ---------------------------------------------------------------------------
// Shared envelope and events
// ---------------------------------------------------------------------------

/// Unknown fields on a forward-compatible surface, retained verbatim so a
/// round trip is exact and an audit can prove nothing was silently dropped
/// (Protocol Index §4.2 rule 3, §5).
pub type Extensions = BTreeMap<String, Value>;

/// Correlation context carried by envelopes and events (Protocol Index §6;
/// Event Protocol §2).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Trace {
    /// The task this message belongs to.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task_id: Option<TaskId>,
    /// The step this message belongs to.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub step_id: Option<StepId>,
    /// The attempt number within the step.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attempt: Option<u32>,
    /// Fields this version does not know, preserved unchanged.
    ///
    /// Correlation context is not an authority decision, so this structure is
    /// forward-compatible like the surfaces that carry it. The checked-in
    /// schemas agree; see the P1 closure record for the earlier divergence
    /// between the two.
    #[serde(flatten)]
    pub extensions: Extensions,
}

/// Every cross-boundary message, so transport, auth, and audit concerns are
/// uniform (Protocol Index §6).
///
/// This surface is forward-compatible: unknown fields are retained in
/// `extensions` and round-tripped unchanged, and are not interpreted.
///
/// `T` is unconstrained so the same wrapper can carry a typed body. The wire
/// form every message actually takes is a JSON **object**, which
/// `envelope.schema.json` enforces; a `T` that is not an object serialises into
/// something that surface correctly refuses.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Envelope<T> {
    /// The envelope major, as a decimal string.
    pub envelope_version: EnvelopeVersion,
    /// The wire surface this message travels on.
    pub surface: WireSurface,
    /// This message's own identity. Delivery is at-least-once and devices
    /// deduplicate by it.
    pub message_id: EventId,
    /// The causal chain of the *task*.
    #[serde(default)]
    pub correlation_id: Option<TaskId>,
    /// The specific message that caused this one; absent only for
    /// user-originated messages.
    #[serde(default)]
    pub causation_id: Option<EventId>,
    /// When the producer issued the message.
    pub issued_at: Timestamp,
    /// The producer's declared class for `payload`. A consumer that computes a
    /// higher class must treat the payload as the higher class.
    pub data_class: DataClass,
    /// Correlation context.
    #[serde(default)]
    pub trace: Option<Trace>,
    /// The message body.
    pub payload: T,
    /// Fields this version does not know, preserved unchanged.
    #[serde(flatten)]
    pub extensions: Extensions,
}

impl<T> Envelope<T> {
    /// Rejects a payload whose major this build does not implement.
    ///
    /// Protocol Index §4.2 rule 1. This is deliberately an explicit call rather
    /// than something deserialisation does implicitly: an envelope can be
    /// deserialised for inspection, and only a consumer that is about to *act*
    /// on it owes the check.
    pub fn require_supported_major(&self) -> Result<(), ProtocolError> {
        if self.envelope_version.is_supported() {
            Ok(())
        } else {
            Err(ProtocolError::ContractViolation {
                rule: ContractRule::UnsupportedEnvelopeMajor,
            })
        }
    }

    /// Rejects a payload outside the per-surface supported-version registry.
    /// The envelope major is checked independently by `require_supported_major`.
    pub fn require_supported_surface(&self) -> Result<(), ProtocolError> {
        if self.surface.is_supported() {
            Ok(())
        } else {
            Err(ProtocolError::ContractViolation {
                rule: ContractRule::UnsupportedWireSurfaceMajor,
            })
        }
    }

    /// Checks both version axes and the consumer's expected surface/version pair.
    /// Pass a registered `WireSurface` constant; a supported but misrouted surface is refused.
    pub fn require_expected_surface(&self, expected: &str) -> Result<(), ProtocolError> {
        self.require_supported_major()?;
        self.require_supported_surface()?;
        if self.surface.as_str() != expected {
            return Err(ProtocolError::ContractViolation {
                rule: ContractRule::UnexpectedWireSurface,
            });
        }
        Ok(())
    }
}

/// Who caused an event (Event Protocol §2.1).
///
/// Forward-compatible like the event that carries it: an attribution member
/// records involvement and can never confer authority (`E10`), so there is
/// nothing here to close. A `MODEL` actor records that a model was involved,
/// never that it decided.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Actor {
    /// The actor class.
    pub kind: ActorKind,
    /// The actor's own identifier.
    pub id: ActorId,
    /// The actor's version.
    pub version: SemVer,
    /// Fields this version does not know, preserved unchanged.
    #[serde(flatten)]
    pub extensions: Extensions,
}

/// One durable, ordered, append-only record of what Serea did or observed
/// (Event Protocol §2).
///
/// This surface is forward-compatible: unknown fields are retained in
/// `extensions` and round-tripped unchanged.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SereaEvent {
    /// The envelope major, as a decimal string.
    pub envelope_version: EnvelopeVersion,
    /// The event surface.
    pub surface: WireSurface,
    /// This event's own identity.
    pub message_id: EventId,
    /// The gapless per-host sequence number, assigned at commit.
    pub seq: Seq,
    /// The frozen event kind.
    pub kind: EventKind,
    /// When the described thing happened.
    pub occurred_at: Timestamp,
    /// The causal chain of the task.
    #[serde(default)]
    pub correlation_id: Option<TaskId>,
    /// The specific message that caused this one.
    #[serde(default)]
    pub causation_id: Option<EventId>,
    /// Who caused it.
    pub actor: Actor,
    /// The declared class for `payload`.
    pub data_class: DataClass,
    /// Correlation context.
    #[serde(default)]
    pub trace: Option<Trace>,
    /// The event-kind-specific body. Its shape is defined per kind in Event
    /// Protocol §3 and validated by the event-bus layer in P3.
    pub payload: Map<String, Value>,
    /// Fields this version does not know, preserved unchanged.
    #[serde(flatten)]
    pub extensions: Extensions,
}

// ---------------------------------------------------------------------------
// Capability protocol
// ---------------------------------------------------------------------------

/// An opaque reference to a capability's JSON Schema 2020-12 document
/// (Protocol Index §5).
///
/// P1 carries the reference and never resolves it: the `jsonschema` dependency
/// is built with `default-features = false`, so no code path in Serea can
/// resolve a `$ref` over the network or off the filesystem.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct JsonSchemaRef(String);

impl JsonSchemaRef {
    /// Wraps and validates a schema reference.
    ///
    /// The accepted character set is a URL/reference subset. The frozen
    /// documents require every schema string to carry a `maxLength`
    /// (Capability Protocol §3.1), so a length bound is required here; the
    /// specific value is a host choice, not a frozen number. Unlike free text,
    /// this field has a frozen grounding for *having* a bound, so it is not the
    /// unratified competing bound that `MAX_VALUE_LENGTH` was. A schema
    /// reference is host-authored and never caller-supplied.
    pub fn new(value: impl Into<String>) -> Result<Self, ProtocolError> {
        let value = value.into();
        let reject = || malformed(ValueField::SchemaReference, ValueRejection::Malformed);
        let ok = !value.is_empty()
            && value.len() <= MAX_SCHEMA_REFERENCE_LENGTH
            && !has_control_characters(&value)
            && value
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_' | '/' | ':'));
        if ok { Ok(Self(value)) } else { Err(reject()) }
    }

    /// The exact wire form.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for JsonSchemaRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for JsonSchemaRef {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for JsonSchemaRef {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(deserializer)?;
        Self::new(raw).map_err(serde::de::Error::custom)
    }
}

/// The complete, host-owned description of a capability (Capability Protocol
/// §3).
///
/// Fields are private and *every* construction path — [`CapabilityDescriptor::new`],
/// `TryFrom<CapabilityDescriptorDraft>`, and therefore deserialisation — goes
/// through the same two registration-time checks that the frozen contract
/// requires:
///
/// * `provider_id` must equal the first segment of `id`
///   (Capability Protocol §3.1; Crate Map §6.1 rule 4);
/// * a `CREDENTIAL`-classified capability must carry `risk_class: CREDENTIAL`
///   (Data Classification §2.3).
///
/// Two deliberate deviations from the frozen prose, both in the safe direction
/// and both recorded in `docs/plans/P1-closure.md`:
///
/// * Capability Protocol §3.1 says a mismatch is a "registration-time panic".
///   P1 returns a typed [`ProtocolError::ContractViolation`] instead, matching
///   this crate's no-panic posture and the P1 plan's requirement for typed
///   protocol errors. The guarantee — never silently accepted — is unchanged;
///   `serea-capability` owns the startup behaviour in P5.
/// * Data Classification §2.3 names the rejected shape as "`CREDENTIAL`-class at
///   `OBSERVE` risk". P1 enforces the general form of the same rule, because
///   a `CREDENTIAL` capability at a lower risk class would be the same
///   contradiction with a different number. Relaxing it is a contract question
///   under Protocol Index §7, not an implementation detail.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "CapabilityDescriptorDraft")]
#[serde(deny_unknown_fields)]
pub struct CapabilityDescriptor {
    id: CapabilityId,
    version: SemVer,
    title: DescriptorTitle,
    description: DescriptorDescription,
    provider_id: ProviderId,
    #[serde(default)]
    implementation_id: Option<ImplementationId>,
    input_schema: JsonSchemaRef,
    output_schema: JsonSchemaRef,
    side_effect_class: SideEffectClass,
    risk_class: RiskClass,
    required_authorization: Authorization,
    replay_safety: ReplaySafety,
    data_class: DataClass,
    root_requirement: RootRequirement,
    idempotency_support: IdempotencySupport,
    max_duration_ms: u32,
    cost_class: CostClass,
    experimental: bool,
}

/// Every field of a [`CapabilityDescriptor`], listed explicitly so no
/// authority-bearing field can be filled in implicitly.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CapabilityDescriptorDraft {
    /// The capability identifier.
    pub id: CapabilityId,
    /// The SemVer of this descriptor's input/output contract.
    pub version: SemVer,
    /// A short human title.
    pub title: DescriptorTitle,
    /// A human description of what the capability does.
    pub description: DescriptorDescription,
    /// The registering provider's namespace.
    pub provider_id: ProviderId,
    /// Which implementation is registered behind `id`, when there is more than
    /// one. `Capability Protocol` §3 omits it; `GoalLatch Adapter` §4 includes
    /// it, so it is optional.
    pub implementation_id: Option<ImplementationId>,
    /// Reference to the input JSON Schema 2020-12 document.
    pub input_schema: JsonSchemaRef,
    /// Reference to the output JSON Schema 2020-12 document.
    pub output_schema: JsonSchemaRef,
    /// What changes in the world.
    pub side_effect_class: SideEffectClass,
    /// How dangerous the operation is. Host-owned; a model may not select it.
    pub risk_class: RiskClass,
    /// The approval requirement.
    pub required_authorization: Authorization,
    /// Whether automatic retry is permitted.
    pub replay_safety: ReplaySafety,
    /// The highest data class this capability transits.
    pub data_class: DataClass,
    /// Whether root is needed.
    pub root_requirement: RootRequirement,
    /// Whether the provider or the host dedupes by key.
    pub idempotency_support: IdempotencySupport,
    /// The hard host-side deadline in milliseconds.
    pub max_duration_ms: u32,
    /// Advisory cost class; never used to select a model.
    pub cost_class: CostClass,
    /// Whether the capability is experimental.
    pub experimental: bool,
}

impl TryFrom<CapabilityDescriptorDraft> for CapabilityDescriptor {
    type Error = ProtocolError;
    fn try_from(draft: CapabilityDescriptorDraft) -> Result<Self, Self::Error> {
        Self::new(draft)
    }
}

impl From<CapabilityDescriptor> for CapabilityDescriptorDraft {
    fn from(descriptor: CapabilityDescriptor) -> Self {
        let CapabilityDescriptor {
            id,
            version,
            title,
            description,
            provider_id,
            implementation_id,
            input_schema,
            output_schema,
            side_effect_class,
            risk_class,
            required_authorization,
            replay_safety,
            data_class,
            root_requirement,
            idempotency_support,
            max_duration_ms,
            cost_class,
            experimental,
        } = descriptor;
        CapabilityDescriptorDraft {
            id,
            version,
            title,
            description,
            provider_id,
            implementation_id,
            input_schema,
            output_schema,
            side_effect_class,
            risk_class,
            required_authorization,
            replay_safety,
            data_class,
            root_requirement,
            idempotency_support,
            max_duration_ms,
            cost_class,
            experimental,
        }
    }
}

impl CapabilityDescriptor {
    /// Validates the two registration-time rules and returns the descriptor.
    pub fn new(draft: CapabilityDescriptorDraft) -> Result<Self, ProtocolError> {
        if draft.id.as_str().split('.').next() != Some(draft.provider_id.as_str()) {
            return Err(ProtocolError::ContractViolation {
                rule: ContractRule::CapabilityProviderNamespaceMismatch,
            });
        }
        if draft.data_class == DataClass::Credential && draft.risk_class != RiskClass::Credential {
            return Err(ProtocolError::ContractViolation {
                rule: ContractRule::CapabilityCredentialClassContradiction,
            });
        }
        Ok(Self {
            id: draft.id,
            version: draft.version,
            title: draft.title,
            description: draft.description,
            provider_id: draft.provider_id,
            implementation_id: draft.implementation_id,
            input_schema: draft.input_schema,
            output_schema: draft.output_schema,
            side_effect_class: draft.side_effect_class,
            risk_class: draft.risk_class,
            required_authorization: draft.required_authorization,
            replay_safety: draft.replay_safety,
            data_class: draft.data_class,
            root_requirement: draft.root_requirement,
            idempotency_support: draft.idempotency_support,
            max_duration_ms: draft.max_duration_ms,
            cost_class: draft.cost_class,
            experimental: draft.experimental,
        })
    }

    /// The capability identifier.
    pub fn id(&self) -> &CapabilityId {
        &self.id
    }

    /// The descriptor's contract version.
    pub fn version(&self) -> &SemVer {
        &self.version
    }

    /// The human title.
    pub fn title(&self) -> &DescriptorTitle {
        &self.title
    }

    /// The human description.
    pub fn description(&self) -> &DescriptorDescription {
        &self.description
    }

    /// The registering provider's namespace.
    pub fn provider_id(&self) -> &ProviderId {
        &self.provider_id
    }

    /// Which implementation is registered, when there is more than one.
    pub fn implementation_id(&self) -> Option<&ImplementationId> {
        self.implementation_id.as_ref()
    }

    /// Reference to the input schema document.
    pub fn input_schema(&self) -> &JsonSchemaRef {
        &self.input_schema
    }

    /// Reference to the output schema document.
    pub fn output_schema(&self) -> &JsonSchemaRef {
        &self.output_schema
    }

    /// What changes in the world.
    pub fn side_effect_class(&self) -> SideEffectClass {
        self.side_effect_class
    }

    /// How dangerous the operation is.
    pub fn risk_class(&self) -> RiskClass {
        self.risk_class
    }

    /// The approval requirement.
    pub fn required_authorization(&self) -> Authorization {
        self.required_authorization
    }

    /// Whether automatic retry is permitted.
    pub fn replay_safety(&self) -> ReplaySafety {
        self.replay_safety
    }

    /// The highest data class this capability transits.
    pub fn data_class(&self) -> DataClass {
        self.data_class
    }

    /// Whether root is needed.
    pub fn root_requirement(&self) -> RootRequirement {
        self.root_requirement
    }

    /// Whether the provider or the host dedupes by key.
    pub fn idempotency_support(&self) -> IdempotencySupport {
        self.idempotency_support
    }

    /// The hard host-side deadline in milliseconds.
    pub fn max_duration_ms(&self) -> u32 {
        self.max_duration_ms
    }

    /// Advisory cost class.
    pub fn cost_class(&self) -> CostClass {
        self.cost_class
    }

    /// Whether the capability is experimental.
    pub fn experimental(&self) -> bool {
        self.experimental
    }
}

/// A structured request to cause one action (Capability Protocol §4).
///
/// This surface is **closed**: an unrecognised field is rejected on parse,
/// before any policy or provider decision (Protocol Index §4.2 rule 3).
///
/// The type has no `risk_class`, `side_effect_class`, `required_authorization`
/// or `provider_id` field at all, so a caller — including a model adapter —
/// cannot supply them. Those are host-resolved from the descriptor
/// (Capability Protocol §4.2), which is why they are absent here rather than
/// merely ignored.
///
/// Capability Protocol §4.2 says a model that emits a host-resolved field has
/// "the extras dropped and a `MODEL_SCHEMA_VIOLATION` event recorded". That
/// obligation belongs to the model-output validation stage in `serea-core` /
/// `serea-model-router` (P4–P5), not to this type: P1 does not implement that
/// stage, and `EventKind::ModelSchemaViolation` is present for it. This type
/// refuses an undeclared field rather than dropping it, which is the stricter
/// direction Protocol Index §4.2 rule 3 requires for a security-sensitive
/// closed schema.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ActionRequest {
    /// Correlates this request with its result.
    pub request_id: RequestId,
    /// The task this step belongs to.
    pub task_id: TaskId,
    /// The step this request is one attempt of.
    pub step_id: StepId,
    /// The capability being requested.
    pub capability_id: CapabilityId,
    /// The pinned descriptor version. Host-resolved.
    pub capability_version: SemVer,
    /// The arguments, validated against the descriptor's input schema.
    pub arguments: Map<String, Value>,
    /// `sha256` over the canonical JSON of `arguments`. Host-resolved.
    pub arguments_digest: Digest,
    /// Derived from the request, so every attempt of this step reuses it.
    pub idempotency_key: IdempotencyKey,
    /// The highest class in `arguments`. Host-resolved.
    pub data_class: DataClass,
    /// Provenance for audit. Never grants authority.
    pub requested_by: RequestedBy,
    /// The host-resolved deadline in milliseconds. Never model-supplied.
    pub deadline_ms: u32,
}

/// The result of one capability invocation (Capability Protocol §5).
///
/// This surface is **closed** for the same reason as [`ActionRequest`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ActionResult {
    /// The `RequestId` of the originating request.
    pub request_id: RequestId,
    /// The outcome class.
    pub status: ActionStatus,
    /// The provider's output, validated against the output schema.
    #[serde(default)]
    pub output: Option<Map<String, Value>>,
    /// `sha256` over the canonical JSON of `output`.
    #[serde(default)]
    pub output_digest: Option<Digest>,
    /// Evidence produced for this call, including for calls with no effect.
    #[serde(default)]
    pub evidence: Vec<Evidence>,
    /// The only accepted proof of an externally visible effect. Non-null
    /// exactly when `status == SUCCEEDED` and `side_effect_class != NONE`
    /// (Capability Protocol §5.1).
    #[serde(default)]
    pub receipt: Option<SideEffectReceipt>,
    /// The failure, when there was one.
    #[serde(default)]
    pub error: Option<ActionError>,
    /// Measured call duration in milliseconds.
    pub duration_ms: u32,
}

/// The append-only record of what was attempted and observed (Capability
/// Protocol §7).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Evidence {
    /// The record's identity. An `EventId`; the frozen registry has no
    /// separate evidence prefix.
    pub evidence_id: EventId,
    /// The capability-owned evidence kind.
    pub kind: EvidenceKind,
    /// The capability the record is about.
    pub capability_id: CapabilityId,
    /// The task.
    pub task_id: TaskId,
    /// The step.
    pub step_id: StepId,
    /// The attempt number within the step.
    pub attempt: u32,
    /// When the record was produced, by the execution environment rather than
    /// by a model turn (GoalLatch Adapter §7).
    pub produced_at: Timestamp,
    /// Who produced it.
    pub actor: Actor,
    /// The class of the payload the record refers to.
    pub data_class: DataClass,
    /// `sha256` over the canonical JSON of the payload.
    pub payload_digest: Digest,
    /// A content-addressed reference to the payload, when it is stored
    /// separately.
    #[serde(default)]
    pub payload_reference: Option<Digest>,
}

/// The common envelope of a receipt (Capability Protocol §5.1).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SideEffectReceipt {
    /// The receipt's identity.
    pub receipt_id: ReceiptId,
    /// The capability that produced the effect.
    pub capability_id: CapabilityId,
    /// The key the effect was made under.
    pub idempotency_key: IdempotencyKey,
    /// The external system's own handle for the affected object — the value
    /// reconciliation needs. Absent is valid only for
    /// `side_effect_class: LOCAL_STATE`.
    #[serde(default)]
    pub provider_reference: Option<ProviderReference>,
    /// A host-readable statement of what changed.
    pub effect_summary: EffectSummary,
    /// When the effect was observed.
    pub observed_at: Timestamp,
    /// Whether replaying the request is safe, taken from the descriptor.
    pub replay_safe: bool,
}

/// A structured failure (Capability Protocol §6).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ActionError {
    /// The frozen error kind.
    pub kind: ActionErrorKind,
    /// A stable machine-readable code, never prose.
    pub code: ErrorCode,
    /// A diagnostic message. No control flow may depend on it.
    pub message: ErrorMessage,
    /// Whether the host may retry. It never authorises a retry on `AMBIGUOUS`.
    pub retryable: bool,
    /// The host remediation the provider suggests.
    pub host_action: HostAction,
    /// Structured detail. P0 specifies no bound on this object and the closure
    /// records payload bounds as an open gap; P1 carries it without inventing
    /// one.
    #[serde(default)]
    pub details: Map<String, Value>,
}

// ---------------------------------------------------------------------------
// Task protocol
// ------------------------------------------------------------------------;

/// Where an `AssistantTask` came from (Task Protocol §2).
///
/// Forward-compatible: the P1 plan keeps shared task objects open, so an
/// architecture-minor addition to the origin shape does not break a reader.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskOrigin {
    /// The origin code, for example `USER_MESSAGE`.
    pub kind: TaskOriginKind,
    /// The device, when the origin is a device.
    #[serde(default)]
    pub device_id: Option<crate::ids::DeviceId>,
    /// The message that triggered the task.
    #[serde(default)]
    pub message_id: Option<EventId>,
    /// Fields this version does not know, preserved unchanged, so a nested
    /// origin round-trips as losslessly as the task that carries it.
    #[serde(flatten)]
    pub extensions: Extensions,
}

/// The per-task attempt budget, materialised on the task at creation
/// (Task Protocol §2; Bounds Protocol §2.1). Forward-compatible for the same
/// reason as [`TaskOrigin`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AttemptBudget {
    /// `max_model_calls_per_task`.
    pub max_model_calls: u32,
    /// `max_tool_calls_per_task`.
    pub max_tool_calls: u32,
    /// `max_attempts_per_step`.
    pub max_attempts_per_step: u32,
    /// Fields this version does not know, preserved unchanged.
    #[serde(flatten)]
    pub extensions: Extensions,
}

/// Serea's unit of durable work (Task Protocol §2).
///
/// This is **not** GoalLatch's `Goal`; a goal is reachable only as an opaque
/// handle behind the `host.goal.*` family (Task Protocol §2;
/// GoalLatch Adapter §1).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AssistantTask {
    /// The task's identity.
    pub task_id: TaskId,
    /// What kind of work this is.
    pub kind: TaskKind,
    /// A short human title.
    pub title: TaskTitle,
    /// The current state-machine state.
    pub state: TaskState,
    /// Where the work came from.
    pub origin: TaskOrigin,
    /// The highest data class this task's data reaches.
    pub data_class: DataClass,
    /// The **maximum** risk class any step of this task may perform. Host
    /// assigned at creation and immutable for this task (Task Protocol §2.1,
    /// `T6`).
    pub policy_class: RiskClass,
    /// When the task was created.
    pub created_at: Timestamp,
    /// When the task last changed.
    pub updated_at: Timestamp,
    /// The task's deadline, when it has one.
    #[serde(default)]
    pub deadline_at: Option<Timestamp>,
    /// The attempt budget.
    pub attempt_budget: AttemptBudget,
    /// The steps, in `sequence` order.
    pub steps: Vec<TaskStep>,
    /// Why the task is blocked. `BLOCKED` is not terminal.
    #[serde(default)]
    pub blocked_reason: Option<BlockedReason>,
    /// The terminal outcome summary, when there is one.
    #[serde(default)]
    pub result_summary: Option<PlainSummary>,
    /// When the task was cancelled, stamped on cancellation (Task Protocol §7).
    #[serde(default)]
    pub cancelled_at: Option<Timestamp>,
    /// Who cancelled it, stamped on cancellation (Task Protocol §7).
    #[serde(default)]
    pub cancelled_by: Option<TaskOriginKind>,
    /// The terminal failure reason (Bounds Protocol §4.4).
    #[serde(default)]
    pub failure_reason: Option<FailureReason>,
    /// Fields this version does not know, preserved unchanged.
    #[serde(flatten)]
    pub extensions: Extensions,
}

/// One step of a task, carrying enough information to be re-executed or
/// verified after a hard restart (Task Protocol §3).
///
/// Forward-compatible, unlike the action surfaces. Task Protocol §3 gives an
/// exhaustive field list, but it does not require a closed schema, and Protocol
/// Index §4.1 makes a new optional field an architecture-*minor* change. Closing
/// it here would make every such minor change a breaking one.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(into = "TaskStepDraft")]
pub struct TaskStep {
    presence: StepPresence,
}

/// Unchecked task-step fields, validated by `TaskStep::new` or `TryFrom` (ADR-0018).
/// Mutating a draft never mutates an already validated step.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TaskStepDraft {
    /// The step's identity.
    pub step_id: StepId,
    /// The task.
    pub task_id: TaskId,
    /// The total order position.
    pub sequence: u32,
    /// What kind of step this is.
    pub kind: StepKind,
    /// The step's status code.
    pub status: StepStatus,
    /// The attempt number.
    pub attempt: u32,
    /// The key derived from the request, so a post-crash re-issue is
    /// recognised as the same action.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub idempotency_key: Option<IdempotencyKey>,
    /// The provider namespace, when the step invokes one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_id: Option<ProviderId>,
    /// The capability, when the step invokes one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub capability_id: Option<CapabilityId>,
    /// The pinned capability version.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub capability_version: Option<SemVer>,
    /// `sha256` over the canonical JSON of the step input, for duplicate
    /// detection without retaining full arguments.
    pub input_digest: Digest,
    /// `sha256` over the canonical JSON of the result, to detect corruption or
    /// partial writes on recovery.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result_digest: Option<Digest>,
    /// The proof of external effect, permitted only on succeeded capability-shaped steps.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub side_effect_receipt: Option<SideEffectReceipt>,
    /// When the attempt started.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub started_at: Option<Timestamp>,
    /// When the attempt completed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<Timestamp>,
    /// The worker holding the lease.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lease_owner: Option<LeaseOwner>,
    /// When the lease expires.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lease_expires_at: Option<Timestamp>,
    /// Positive fencing generation, retained after a lease ends; wire zero is refused.
    #[serde(
        default,
        deserialize_with = "deserialize_lease_generation",
        skip_serializing_if = "Option::is_none"
    )]
    pub lease_generation: Option<u32>,
    /// The last failure, preserved so a terminal task explains itself.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<ActionError>,
    /// Fields this version does not know, preserved unchanged so a step
    /// round-trips as exactly as the task that contains it does.
    #[serde(flatten)]
    pub extensions: Extensions,
}

fn deserialize_lease_generation<'de, D>(deserializer: D) -> Result<Option<u32>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let invalid = || {
        serde::de::Error::custom(malformed(
            ValueField::LeaseGeneration,
            ValueRejection::OutOfRange,
        ))
    };
    // RawValue preserves the numeric token and distinguishes literal objects from
    // serde_json's private number-marker maps. Value inputs retain their numeric
    // text through the dependency's arbitrary_precision feature.
    let Some(raw) = Option::<Box<serde_json::value::RawValue>>::deserialize(deserializer)
        .map_err(|_| invalid())?
    else {
        return Ok(None);
    };
    exact_lease_generation(raw.get())
        .map(Some)
        .ok_or_else(invalid)
}

// The input is a validated JSON value from RawValue, not an arbitrary string.
fn exact_lease_generation(raw: &str) -> Option<u32> {
    let raw = raw.trim();
    if !raw.as_bytes().first()?.is_ascii_digit() {
        return None;
    }
    let (mantissa, exponent) = match raw.split_once(['e', 'E']) {
        Some((mantissa, exponent)) => (mantissa, exponent.parse::<i128>().ok()?),
        None => (raw, 0),
    };
    let (whole, fraction) = mantissa.split_once('.').unwrap_or((mantissa, ""));
    let digits = whole.bytes().chain(fraction.bytes());
    let length = whole.len().checked_add(fraction.len())?;
    let leading = digits.clone().take_while(|byte| *byte == b'0').count();
    if leading == length {
        return None;
    }
    let trailing = digits
        .clone()
        .rev()
        .take_while(|byte| *byte == b'0')
        .count();

    // Removing trailing zeros leaves an integer exactly when the remaining
    // decimal shift is nonnegative. Never expand an exponent or round a fraction.
    let shift = exponent
        .checked_sub(i128::try_from(fraction.len()).ok()?)?
        .checked_add(i128::try_from(trailing).ok()?)?;
    let shift = u32::try_from(shift).ok()?;
    let coefficient = digits
        .skip(leading)
        .take(length - leading - trailing)
        .try_fold(0_u32, |value, byte| {
            value.checked_mul(10)?.checked_add(u32::from(byte - b'0'))
        })?;
    coefficient.checked_mul(10_u32.checked_pow(shift)?)
}

impl<'de> Deserialize<'de> for TaskStep {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        // Serde's draft errors can include rejected values and nested raw keys.
        // Sanitize before the checked presence boundary; never format that error.
        let draft = TaskStepDraft::deserialize(deserializer)
            .map_err(|_| serde::de::Error::custom("invalid task step draft"))?;
        StepPresence::new(draft)
            .map(Self::from)
            .map_err(serde::de::Error::custom)
    }
}

/// Checked step-kind and known-status field presence (ADR-0018 §2–§4).
/// Unknown well-formed statuses retain kind invariants and supplied-value checks.
#[derive(Debug, Clone, PartialEq)]
pub struct StepPresence {
    draft: TaskStepDraft,
}

#[derive(Clone, Copy)]
enum PresenceCell {
    Absent,
    Required,
    Optional,
}

impl PresenceCell {
    fn accepts(self, present: bool) -> bool {
        match self {
            Self::Absent => !present,
            Self::Required => present,
            Self::Optional => true,
        }
    }
}

const STEP_MEMBERS: [&str; 19] = [
    "step_id",
    "task_id",
    "sequence",
    "kind",
    "status",
    "attempt",
    "input_digest",
    "idempotency_key",
    "provider_id",
    "capability_id",
    "capability_version",
    "result_digest",
    "side_effect_receipt",
    "started_at",
    "completed_at",
    "lease_owner",
    "lease_expires_at",
    "lease_generation",
    "error",
];

impl StepPresence {
    /// Validates all shape-level invariants without runtime, descriptor or storage state.
    pub fn new(draft: TaskStepDraft) -> Result<Self, ProtocolError> {
        let violation = |rule| ProtocolError::ContractViolation { rule };
        if draft
            .extensions
            .keys()
            .any(|key| STEP_MEMBERS.contains(&key.as_str()))
        {
            return Err(violation(ContractRule::StepReservedExtensionKey));
        }
        if draft.lease_generation == Some(0) {
            return Err(malformed(
                ValueField::LeaseGeneration,
                ValueRejection::OutOfRange,
            ));
        }
        let capability_shaped = matches!(
            draft.kind,
            StepKind::Capability | StepKind::Delegate | StepKind::Verify
        );
        let tuple = [
            draft.provider_id.is_some(),
            draft.capability_id.is_some(),
            draft.capability_version.is_some(),
            draft.idempotency_key.is_some(),
        ];
        if tuple.iter().any(|present| *present != capability_shaped)
            || (!capability_shaped && draft.side_effect_receipt.is_some())
        {
            return Err(violation(ContractRule::StepKindFieldPresence));
        }
        if draft.status.as_str() == "WAITING"
            && !matches!(
                draft.kind,
                StepKind::WaitApproval | StepKind::WaitUser | StepKind::WaitSchedule
            )
        {
            return Err(violation(ContractRule::StepWaitingKind));
        }
        use PresenceCell::{Absent as N, Optional as O, Required as R};
        // result, started, completed, lease owner/expiry, generation, receipt, error.
        let (planned, cells) = match draft.status.as_str() {
            "PLANNED" => (true, [N, N, N, N, N, N, N]),
            "LEASED" => (false, [N, N, N, R, R, N, N]),
            "EXECUTING" => (false, [N, R, N, R, R, N, N]),
            "WAITING" => (false, [N, R, N, N, R, N, N]),
            "SUCCEEDED" => (false, [R, R, R, N, R, O, N]),
            "FAILED" => (false, [O, R, R, N, R, N, R]),
            "RECONCILED_ABSENT" => (false, [O, R, R, N, R, N, N]),
            _ => return Ok(Self { draft }),
        };
        let supplied = [
            draft.result_digest.is_some(),
            draft.started_at.is_some(),
            draft.completed_at.is_some(),
            draft.lease_owner.is_some(),
            draft.lease_generation.is_some(),
            draft.side_effect_receipt.is_some(),
            draft.error.is_some(),
        ];
        if (draft.attempt == 0) != planned
            || cells
                .iter()
                .zip(supplied)
                .any(|(cell, present)| !cell.accepts(present))
            || !cells[3].accepts(draft.lease_expires_at.is_some())
        {
            return Err(violation(ContractRule::StepStatusFieldPresence));
        }
        Ok(Self { draft })
    }
}

impl TryFrom<TaskStepDraft> for StepPresence {
    type Error = ProtocolError;
    fn try_from(draft: TaskStepDraft) -> Result<Self, Self::Error> {
        Self::new(draft)
    }
}

impl TaskStep {
    /// Constructs a step through the same presence checks used by deserialization.
    pub fn new(draft: TaskStepDraft) -> Result<Self, ProtocolError> {
        StepPresence::new(draft).map(Self::from)
    }
}

impl TryFrom<TaskStepDraft> for TaskStep {
    type Error = ProtocolError;
    fn try_from(draft: TaskStepDraft) -> Result<Self, Self::Error> {
        Self::new(draft)
    }
}

impl From<StepPresence> for TaskStep {
    fn from(presence: StepPresence) -> Self {
        Self { presence }
    }
}

impl From<TaskStep> for TaskStepDraft {
    fn from(step: TaskStep) -> Self {
        step.presence.draft
    }
}

impl std::ops::Deref for TaskStep {
    type Target = TaskStepDraft;
    fn deref(&self) -> &Self::Target {
        &self.presence.draft
    }
}

// ---------------------------------------------------------------------------
// Model protocol
// ------------------------------------------------------------------------}

/// One conversation message (Model Protocol §3).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModelMessage {
    /// Who produced the message.
    pub role: MessageRole,
    /// The message content.
    pub content: String,
}

/// One model call request (Model Protocol §3).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModelRequest {
    /// Correlates the call with its response.
    pub request_id: RequestId,
    /// The model the router selected. Host-chosen; no step consults a model
    /// about which model to use.
    pub model_id: crate::ids::ModelId,
    /// The task, when the call charges to one.
    #[serde(default)]
    pub task_id: Option<TaskId>,
    /// The host-assigned purpose. Drives routing and accounting.
    pub purpose: ModelPurpose,
    /// The conversation.
    #[serde(default)]
    pub messages: Vec<ModelMessage>,
    /// The system prompt. Never a security control (Data Classification §1).
    #[serde(default)]
    pub system: Option<String>,
    /// What the output must look like.
    pub response_format: ResponseFormat,
    /// Host-defined tool schemas offered to the model.
    #[serde(default)]
    pub tools: Vec<Value>,
    /// `max_output_tokens_per_call`, host-set.
    pub max_output_tokens: u32,
    /// The sampling temperature. Injected variability (Model Protocol §10).
    pub temperature: f64,
    /// The host-resolved deadline in milliseconds.
    pub deadline_ms: u32,
    /// The highest class of anything in the prompt.
    pub data_class: DataClass,
}

/// Counted usage for one call (Model Protocol §9; Bounds Protocol §7.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModelUsage {
    /// Input tokens, counted rather than estimated.
    pub input_tokens: TokenCount,
    /// Output tokens, counted rather than estimated.
    pub output_tokens: TokenCount,
    /// The call's cost class.
    pub cost_class: CostClass,
}

/// One model call response (Model Protocol §4).
///
/// `structured` is a **proposal**. It has not passed capability validation at
/// this point; that is a separate, later, host-owned stage
/// (Model Protocol §4.1).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModelResponse {
    /// The `RequestId` of the originating call.
    pub request_id: RequestId,
    /// The model that served the call.
    pub model_id: crate::ids::ModelId,
    /// The provider that served the call.
    pub provider_id: ProviderId,
    /// The rendered content.
    pub content: String,
    /// Structured output, present only when `response_format` was
    /// `JSON_SCHEMA` and validation succeeded.
    #[serde(default)]
    pub structured: Option<Value>,
    /// Why the provider stopped.
    pub finish_reason: FinishReason,
    /// Counted usage.
    pub usage: ModelUsage,
    /// Measured latency in milliseconds.
    pub latency_ms: u32,
    /// Repair calls spent on this step, bounded by `max_repair_attempts`.
    pub repair_attempts: u32,
}

/// A model failure (Model Protocol §3.2, §7.3).
///
/// Distinct from `ActionError`: a model failure is not an action failure, and
/// the frozen `ActionErrorKind` set gains no member for it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModelError {
    /// A stable code for the failure kind.
    pub kind: ModelErrorCode,
    /// A diagnostic message. No control flow may depend on it.
    pub message: ErrorMessage,
    /// Whether the host may try the call again.
    pub retryable: bool,
}

/// What a model can do, as data rather than scattered provider branches
/// (Model Protocol §5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModelCapabilities {
    /// The model accepts images.
    pub vision: bool,
    /// The model accepts tool definitions.
    pub tools: bool,
    /// The model produces structured output.
    pub structured_output: bool,
    /// How strictly the provider honours `response_format.schema`.
    pub json_schema_mode: JsonSchemaMode,
    /// The model emits reasoning traces.
    pub thinking: bool,
    /// The model accepts long context.
    pub long_context: bool,
    /// The model is fast.
    pub fast: bool,
    /// The model is a code specialist. `codex` is known but disabled.
    pub code_specialist: bool,
    /// The context window.
    pub max_context_tokens: u32,
    /// The maximum output length.
    pub max_output_tokens: u32,
    /// The provider supports streaming.
    pub supports_streaming: bool,
    /// The provider supports seeds.
    pub supports_seeds: bool,
}

/// One roster entry.
///
/// `Model Protocol` §2 names `ModelDescriptor` as the element of `models()` and
/// §5/§5.1 define `ModelCapabilities` and the per-model provider, but no
/// document spells out a descriptor shape. P1 therefore carries exactly those
/// three facts and nothing else.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModelDescriptor {
    /// The model identifier.
    pub model_id: crate::ids::ModelId,
    /// The provider that serves it.
    pub provider_id: ProviderId,
    /// What the model can do.
    pub capabilities: ModelCapabilities,
}
