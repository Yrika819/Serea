//! Closed persisted Scheduler values. Runtime wake orchestration belongs to
//! `serea-scheduler`; these values own only canonical parsing and validation.

use serde::Deserialize;
use serde_json::{Map, Value};

use crate::{DataClass, EventKind, TaskTitle, TextCategory, canonicalize};

/// A payload-free validation error for EventPredicateV1.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventPredicateError {
    /// The object is malformed, unknown, or not eligible for HOST_EVENT.
    Invalid,
}

impl std::fmt::Display for EventPredicateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("invalid EventPredicateV1")
    }
}

impl std::error::Error for EventPredicateError {}

/// The exact closed HOST_EVENT predicate persisted by Scheduler V1.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventPredicateV1 {
    event_kind: EventKind,
    canonical_json: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawPredicate {
    version: String,
    event_kind: EventKind,
}

impl EventPredicateV1 {
    /// Parses duplicate-aware SCJ-1 input and normalizes it to one stable form.
    pub fn parse_json(input: &str) -> Result<Self, EventPredicateError> {
        let canonical_input = canonicalize(input).map_err(|_| EventPredicateError::Invalid)?;
        let value: Value =
            serde_json::from_slice(&canonical_input).map_err(|_| EventPredicateError::Invalid)?;
        let object = value.as_object().ok_or(EventPredicateError::Invalid)?;
        let actual: Vec<&str> = object.keys().map(String::as_str).collect();
        if actual != ["event_kind", "version"] {
            return Err(EventPredicateError::Invalid);
        }
        let raw: RawPredicate =
            serde_json::from_value(value).map_err(|_| EventPredicateError::Invalid)?;
        if raw.version != "1" || !event_kind_is_host_event_eligible(raw.event_kind) {
            return Err(EventPredicateError::Invalid);
        }
        let canonical_json = serialize_predicate(raw.event_kind)?;
        Ok(Self {
            event_kind: raw.event_kind,
            canonical_json,
        })
    }

    /// Returns the registered exact-match kind.
    pub fn event_kind(&self) -> EventKind {
        self.event_kind
    }

    /// Returns the deterministic canonical SCJ-1 object.
    pub fn canonical_json(&self) -> &str {
        &self.canonical_json
    }

    /// Performs the sole positive V1 match rule; Scheduler separately rejects
    /// events whose durable causal root is a Scheduler occurrence.
    pub fn matches_event_kind(&self, kind: EventKind) -> bool {
        event_kind_is_host_event_eligible(kind) && self.event_kind == kind
    }
}

/// Whether a registered event kind is eligible for the generic HOST_EVENT
/// trigger path. Dedicated and Scheduler-owned events are excluded.
pub fn event_kind_is_host_event_eligible(kind: EventKind) -> bool {
    !matches!(
        kind,
        EventKind::DeviceConnected
            | EventKind::ApprovalGranted
            | EventKind::ApprovalDenied
            | EventKind::ApprovalExpired
            | EventKind::ScheduleCreated
            | EventKind::ScheduleUpdated
            | EventKind::SchedulePaused
            | EventKind::ScheduleResumed
            | EventKind::ScheduleCancelled
            | EventKind::ScheduleOccurrenceMissed
            | EventKind::ScheduleTaskCreated
            | EventKind::ScheduleCatchUpDeferred
    )
}

fn serialize_predicate(kind: EventKind) -> Result<String, EventPredicateError> {
    let mut object = Map::new();
    object.insert("event_kind".into(), Value::String(kind.wire_name().into()));
    object.insert("version".into(), Value::String("1".into()));
    let source =
        serde_json::to_string(&Value::Object(object)).map_err(|_| EventPredicateError::Invalid)?;
    let canonical = canonicalize(&source).map_err(|_| EventPredicateError::Invalid)?;
    String::from_utf8(canonical).map_err(|_| EventPredicateError::Invalid)
}

/// Maximum UTF-8 bytes accepted for raw or canonical ScheduledTaskTemplateV1.
pub const MAX_SCHEDULE_TEMPLATE_BYTES: usize = 32_768;

/// A validated, non-authoritative task title and planning-intent template.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScheduledTaskTemplateV1 {
    title: TaskTitle,
    intent: String,
    canonical_json: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawTemplate {
    version: String,
    title: String,
    intent: String,
}

/// A payload-free validation error for ScheduledTaskTemplateV1.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TemplateError {
    /// The object or text fields are malformed or not supported.
    Invalid,
    /// Raw input or canonical form exceeds `max_schedule_template_bytes`.
    TooLarge,
}

impl std::fmt::Display for TemplateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Invalid => "invalid ScheduledTaskTemplateV1",
            Self::TooLarge => "ScheduledTaskTemplateV1 exceeds max_schedule_template_bytes",
        })
    }
}

impl std::error::Error for TemplateError {}

impl ScheduledTaskTemplateV1 {
    /// Parses duplicate-aware SCJ-1 input and validates title and intent using
    /// the existing protocol text rules.
    pub fn parse_json(input: &str) -> Result<Self, TemplateError> {
        if input.len() > MAX_SCHEDULE_TEMPLATE_BYTES {
            return Err(TemplateError::TooLarge);
        }
        let canonical_input = canonicalize(input).map_err(|_| TemplateError::Invalid)?;
        if canonical_input.len() > MAX_SCHEDULE_TEMPLATE_BYTES {
            return Err(TemplateError::TooLarge);
        }
        let value: Value =
            serde_json::from_slice(&canonical_input).map_err(|_| TemplateError::Invalid)?;
        let object = value.as_object().ok_or(TemplateError::Invalid)?;
        let actual: Vec<&str> = object.keys().map(String::as_str).collect();
        if actual != ["intent", "title", "version"] {
            return Err(TemplateError::Invalid);
        }
        let raw: RawTemplate = serde_json::from_value(value).map_err(|_| TemplateError::Invalid)?;
        if raw.version != "1" || !TextCategory::Prose.accepts(&raw.intent) {
            return Err(TemplateError::Invalid);
        }
        let title = TaskTitle::new(raw.title).map_err(|_| TemplateError::Invalid)?;
        let canonical_json = serialize_template(&title, &raw.intent)?;
        if canonical_json.len() > MAX_SCHEDULE_TEMPLATE_BYTES {
            return Err(TemplateError::TooLarge);
        }
        Ok(Self {
            title,
            intent: raw.intent,
            canonical_json,
        })
    }

    /// Returns the already validated human-readable task title.
    pub fn title(&self) -> &TaskTitle {
        &self.title
    }

    /// Returns the planning-context prose; it is never authority or executable.
    pub fn intent(&self) -> &str {
        &self.intent
    }

    /// Returns the deterministic canonical SCJ-1 object.
    pub fn canonical_json(&self) -> &str {
        &self.canonical_json
    }

    /// Composes host-classified title and intent without permitting the value
    /// to lower either classification.
    pub fn inherited_data_class(&self, title: DataClass, intent: DataClass) -> DataClass {
        DataClass::compose(title, intent)
    }
}

fn serialize_template(title: &TaskTitle, intent: &str) -> Result<String, TemplateError> {
    let mut object = Map::new();
    object.insert("intent".into(), Value::String(intent.to_owned()));
    object.insert("title".into(), Value::String(title.as_str().to_owned()));
    object.insert("version".into(), Value::String("1".into()));
    let source =
        serde_json::to_string(&Value::Object(object)).map_err(|_| TemplateError::Invalid)?;
    let canonical = canonicalize(&source).map_err(|_| TemplateError::Invalid)?;
    String::from_utf8(canonical).map_err(|_| TemplateError::Invalid)
}
