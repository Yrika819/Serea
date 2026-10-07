//! Closed persisted Scheduler values. Runtime wake orchestration belongs to
//! `serea-scheduler`; these values own only canonical parsing and validation.

use serde::Deserialize;
use serde_json::{Map, Value};

use crate::{
    ApprovalId, DataClass, DeviceId, EventKind, StepId, TaskId, TaskTitle, TextCategory, Trace,
    canonicalize,
};

/// Payload-free validation error for an approval lifecycle event payload.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApprovalLifecyclePayloadError {
    /// The kind, payload, correlation, or required task/step trace is invalid.
    Invalid,
}

impl std::fmt::Display for ApprovalLifecyclePayloadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("invalid ApprovalLifecyclePayloadV1")
    }
}

impl std::error::Error for ApprovalLifecyclePayloadError {}

/// Closed routing identity for an approval lifecycle event. It conveys no
/// grant, policy, or approval authority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApprovalLifecyclePayloadV1 {
    approval_id: ApprovalId,
    task_id: TaskId,
    step_id: StepId,
    canonical_json: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawApprovalLifecyclePayload {
    approval_id: ApprovalId,
    task_id: TaskId,
    step_id: StepId,
}

impl ApprovalLifecyclePayloadV1 {
    /// Parses a closed payload for one approval lifecycle event and validates
    /// its correlation and required task/step trace. Duplicate JSON keys are
    /// rejected by SCJ-1 before typed decoding.
    pub fn parse_event(
        kind: EventKind,
        input: &str,
        correlation_id: Option<&TaskId>,
        trace: Option<&Trace>,
    ) -> Result<Self, ApprovalLifecyclePayloadError> {
        if !matches!(
            kind,
            EventKind::ApprovalGranted | EventKind::ApprovalDenied | EventKind::ApprovalExpired
        ) {
            return Err(ApprovalLifecyclePayloadError::Invalid);
        }
        let canonical_input =
            canonicalize(input).map_err(|_| ApprovalLifecyclePayloadError::Invalid)?;
        let value: Value = serde_json::from_slice(&canonical_input)
            .map_err(|_| ApprovalLifecyclePayloadError::Invalid)?;
        let object = value
            .as_object()
            .ok_or(ApprovalLifecyclePayloadError::Invalid)?;
        let actual: Vec<&str> = object.keys().map(String::as_str).collect();
        if actual != ["approval_id", "step_id", "task_id"] {
            return Err(ApprovalLifecyclePayloadError::Invalid);
        }
        let raw: RawApprovalLifecyclePayload =
            serde_json::from_value(value).map_err(|_| ApprovalLifecyclePayloadError::Invalid)?;
        let event_task = correlation_id.ok_or(ApprovalLifecyclePayloadError::Invalid)?;
        let event_trace = trace.ok_or(ApprovalLifecyclePayloadError::Invalid)?;
        if &raw.task_id != event_task
            || event_trace.task_id.as_ref() != Some(&raw.task_id)
            || event_trace.step_id.as_ref() != Some(&raw.step_id)
        {
            return Err(ApprovalLifecyclePayloadError::Invalid);
        }
        let source = serde_json::to_string(&serde_json::json!({
            "approval_id": raw.approval_id,
            "task_id": raw.task_id,
            "step_id": raw.step_id,
        }))
        .map_err(|_| ApprovalLifecyclePayloadError::Invalid)?;
        let canonical_json = String::from_utf8(
            canonicalize(&source).map_err(|_| ApprovalLifecyclePayloadError::Invalid)?,
        )
        .map_err(|_| ApprovalLifecyclePayloadError::Invalid)?;
        Ok(Self {
            approval_id: raw.approval_id,
            task_id: raw.task_id,
            step_id: raw.step_id,
            canonical_json,
        })
    }

    /// Approval request identity used for durable routing.
    pub fn approval_id(&self) -> &ApprovalId {
        &self.approval_id
    }

    /// Existing task identity used for durable routing, not authority.
    pub fn task_id(&self) -> &TaskId {
        &self.task_id
    }

    /// Existing step identity used for durable routing, not authority.
    pub fn step_id(&self) -> &StepId {
        &self.step_id
    }

    /// Returns the canonical SCJ-1 JSON routing payload.
    pub fn canonical_json(&self) -> &str {
        &self.canonical_json
    }
}

/// Payload-free validation error for the kind-specific DEVICE_CONNECTED payload.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceConnectedPayloadError {
    /// The payload is malformed, has unknown members, or contains an invalid DeviceId.
    Invalid,
}

impl std::fmt::Display for DeviceConnectedPayloadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("invalid DeviceConnectedPayloadV1")
    }
}

impl std::error::Error for DeviceConnectedPayloadError {}

/// The closed DEVICE_CONNECTED payload. It identifies a device session only;
/// it does not identify or authorize a task to resume.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceConnectedPayloadV1 {
    device_id: DeviceId,
    canonical_json: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawDeviceConnectedPayload {
    device_id: DeviceId,
}

impl DeviceConnectedPayloadV1 {
    /// Parses a duplicate-aware, exact one-member payload and canonicalizes it.
    pub fn parse_json(input: &str) -> Result<Self, DeviceConnectedPayloadError> {
        let canonical_input =
            canonicalize(input).map_err(|_| DeviceConnectedPayloadError::Invalid)?;
        let value: Value = serde_json::from_slice(&canonical_input)
            .map_err(|_| DeviceConnectedPayloadError::Invalid)?;
        let object = value
            .as_object()
            .ok_or(DeviceConnectedPayloadError::Invalid)?;
        if object.len() != 1 || !object.contains_key("device_id") {
            return Err(DeviceConnectedPayloadError::Invalid);
        }
        let raw: RawDeviceConnectedPayload =
            serde_json::from_value(value).map_err(|_| DeviceConnectedPayloadError::Invalid)?;
        let mut object = Map::new();
        object.insert(
            "device_id".into(),
            Value::String(raw.device_id.as_str().into()),
        );
        let source = serde_json::to_string(&Value::Object(object))
            .map_err(|_| DeviceConnectedPayloadError::Invalid)?;
        let canonical_json = String::from_utf8(
            canonicalize(&source).map_err(|_| DeviceConnectedPayloadError::Invalid)?,
        )
        .map_err(|_| DeviceConnectedPayloadError::Invalid)?;
        Ok(Self {
            device_id: raw.device_id,
            canonical_json,
        })
    }

    /// Identifies only the device whose session was established.
    pub fn device_id(&self) -> &DeviceId {
        &self.device_id
    }

    /// Returns canonical SCJ-1 JSON containing exactly `device_id`.
    pub fn canonical_json(&self) -> &str {
        &self.canonical_json
    }
}

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
