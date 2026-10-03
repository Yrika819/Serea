//! Schema contract tests for the five checked-in JSON Schema 2020-12
//! documents.
//!
//! Frozen source: `docs/protocols/00-protocol-index.md` §4.2 and §5;
//! Capability Protocol §3.1 (closed-world constraints) and §4.2 (host-resolved
//! fields); Data Classification §4 (an allowlist, never a denylist, on
//! security-sensitive input) and §4.2 (the output direction).
//!
//! Every test names the protocol section that pins the schema's content, so a
//! future edit to a schema document has to be justified against a frozen
//! contract rather than against whichever expectation happened to be written.

use serde_json::{Value, json};
use serea_protocol::schema::{self, SchemaError, SchemaName};
use serea_protocol::types::{
    AttemptBudget, BlockedReason, ErrorCode, ErrorMessage, HostAction, ReasonCode, StepStatus,
    TaskOrigin, Trace,
};

const TASK_ID: &str = "tsk_01JQ8Z9K3M7QWXR4V2T6YH0BNA";
const STEP_ID: &str = "stp_01JQ8Z9M3R2CVN8H5FWK7PQDSF";
const REQUEST_ID: &str = "req_01JQ8ZA4H6NFG8K2M6RTV9XCWB";
const EVENT_ID: &str = "evt_01JQ8ZB7H2XKM9P4QW7NRT5YCD";
const DIGEST: &str = "sha256:3b1f0c9e2a7d4e6b8f0a2c4d6e8b0d2f4a6c8e0b2d4f6a8c0e2b4d6f8a0c2e4b";
const IDEMPOTENCY_KEY: &str =
    "idk_9f2c1a7e4b6d0f8a3c5e9b1d7f2a4c6e8b0d3f5a7c9e1b4d6f8a0c2e4b6d8f9a";

/// Protocol Index §5: every `input_schema` and `output_schema` is JSON Schema
/// 2020-12, and every checked-in document must declare that dialect.
#[test]
fn every_checked_in_document_is_json_schema_2020_12() {
    assert_eq!(SchemaName::ALL.len(), 5);
    for name in SchemaName::ALL {
        let document = name
            .json()
            .unwrap_or_else(|e| panic!("{name} must parse: {e}"));
        assert_eq!(
            document["$schema"], "https://json-schema.org/draft/2020-12/schema",
            "{name} must declare the frozen dialect"
        );
        assert!(
            document["$id"]
                .as_str()
                .is_some_and(|id| id.starts_with("https://serea.local/schemas/")),
            "{name} must declare a $id in the frozen schema namespace"
        );
        assert!(!document["title"].as_str().unwrap_or_default().is_empty());
        // The validator must be able to compile it. A document that cannot be
        // compiled would silently disable every check below.
        schema::validator(name).unwrap_or_else(|e| panic!("{name} must compile: {e}"));
    }
}

/// The `additionalProperties` decision is the security-relevant one, and it is
/// per-protocol rather than global (Protocol Index §4.2 rule 3).
#[test]
fn only_the_action_surfaces_are_closed() {
    let closed = [SchemaName::ActionRequest, SchemaName::ActionResult];
    let forward_compatible = [
        SchemaName::Envelope,
        SchemaName::AssistantTask,
        SchemaName::Event,
    ];
    for name in closed {
        let document = name.json().expect("parses");
        assert_eq!(
            document["additionalProperties"],
            Value::Bool(false),
            "{name} is an action surface and must reject undeclared properties"
        );
    }
    for name in forward_compatible {
        let document = name.json().expect("parses");
        assert_ne!(
            document["additionalProperties"],
            Value::Bool(false),
            "{name} is a shared forward-compatible surface and must not close the object"
        );
    }
}

// ---------------------------------------------------------------------------
// PROTO-CAP §4 — ActionRequest
// ---------------------------------------------------------------------------

/// The Capability Protocol §4 example, verbatim.
fn frozen_action_request() -> Value {
    json!({
        "request_id": REQUEST_ID,
        "task_id": TASK_ID,
        "step_id": STEP_ID,
        "capability_id": "calendar.events.list",
        "capability_version": "1.2.0",
        "arguments": { "range": "tomorrow" },
        "arguments_digest": DIGEST,
        "idempotency_key": IDEMPOTENCY_KEY,
        "data_class": "PERSONAL",
        "requested_by": "MODEL",
        "deadline_ms": 15000
    })
}

#[test]
fn pro_cap_4_the_frozen_action_request_satisfies_the_schema() {
    assert!(
        schema::is_valid(SchemaName::ActionRequest, &frozen_action_request()).expect("compiles"),
        "the Capability Protocol Section 4 example must validate"
    );
}

#[test]
fn pro_cap_4_2_a_model_cannot_supply_a_host_resolved_field() {
    // Capability Protocol §4.2: these are set by the host after validation and
    // are absent from model-authored input. The closed schema is where that
    // becomes a rejection rather than a comment.
    for (field, value) in [
        ("risk_class", json!("EXTERNAL_WRITE")),
        ("side_effect_class", json!("NONE")),
        ("required_authorization", json!("NONE")),
        ("provider_id", json!("calendar")),
        ("authorization", json!("SCOPED_GRANT")),
        ("grant_id", json!("grt_01JQ8ZA7B3KMW9Q4TVY7XN2RDP")),
        ("approval_id", json!("apr_01JQ8ZA1D4NFG8K2M6RTV9XCWB")),
        ("codex_allowed", json!(true)),
        ("codex_allowed", json!(false)),
    ] {
        let mut polluted = frozen_action_request();
        polluted[field] = value.clone();
        let outcome = schema::validate(SchemaName::ActionRequest, &polluted);
        assert!(
            matches!(outcome, Err(SchemaError::InstanceInvalid { .. })),
            "{field}={value} must be refused before any authority decision"
        );
    }
}

#[test]
fn pro_cap_3_1_an_undeclared_property_is_refused_on_the_action_request() {
    let mut polluted = frozen_action_request();
    polluted["future_field"] = json!(true);
    assert!(schema::validate(SchemaName::ActionRequest, &polluted).is_err());
}

#[test]
fn pro_cap_4_identifier_forms_are_pinned_by_the_schema() {
    let cases: [(&str, &str); 12] = [
        ("request_id", STEP_ID),
        ("task_id", EVENT_ID),
        ("step_id", TASK_ID),
        ("request_id", "req_01JQ8Z9K3M7QWXR4V2T6YH0B"),
        ("request_id", "req_01jq8z9k3m7qwxr4v2t6yh0bna"),
        ("task_id", "tsk_81JQ8Z9K3M7QWXR4V2T6YH0BNA"),
        ("idempotency_key", "idk_9f2c1a7e"),
        ("idempotency_key", DIGEST),
        ("arguments_digest", IDEMPOTENCY_KEY),
        ("capability_id", "calendar.events"),
        ("capability_id", "calendar.events.execute"),
        ("capability_id", "calendar-events.list"),
    ];
    for (field, bad) in cases {
        let mut wrong = frozen_action_request();
        wrong[field] = json!(bad);
        assert!(
            schema::validate(SchemaName::ActionRequest, &wrong).is_err(),
            "{field}={bad} must be refused"
        );
    }
}

#[test]
fn pro_cap_4_1_an_unregistered_requested_by_variant_is_refused() {
    for bad in ["model", "Agent", "DELEGATED", "PROACTIVE"] {
        let mut wrong = frozen_action_request();
        wrong["requested_by"] = json!(bad);
        assert!(
            schema::validate(SchemaName::ActionRequest, &wrong).is_err(),
            "requested_by={bad} must fail closed"
        );
    }
}

#[test]
fn pro_data_2_an_unregistered_data_class_is_refused() {
    for bad in ["PUBLICISH", "Confidential", "", "personal"] {
        let mut wrong = frozen_action_request();
        wrong["data_class"] = json!(bad);
        assert!(schema::validate(SchemaName::ActionRequest, &wrong).is_err());
    }
}

#[test]
fn pro_cap_4_every_required_field_is_required() {
    for field in [
        "request_id",
        "task_id",
        "step_id",
        "capability_id",
        "capability_version",
        "arguments",
        "arguments_digest",
        "idempotency_key",
        "data_class",
        "requested_by",
        "deadline_ms",
    ] {
        let mut incomplete = frozen_action_request();
        incomplete.as_object_mut().expect("object").remove(field);
        assert!(
            schema::validate(SchemaName::ActionRequest, &incomplete).is_err(),
            "{field} is required by Capability Protocol Section 4"
        );
    }
}

#[test]
fn pro_cap_3_1_incorrect_field_types_are_refused_rather_than_coerced() {
    for (field, bad) in [
        ("deadline_ms", json!("15000")),
        ("deadline_ms", json!(1.5)),
        ("arguments", json!([])),
        ("arguments", json!("tomorrow")),
        ("capability_version", json!(1)),
    ] {
        let mut wrong = frozen_action_request();
        wrong[field] = bad.clone();
        assert!(
            schema::validate(SchemaName::ActionRequest, &wrong).is_err(),
            "{field}={bad} must be refused"
        );
    }
}

#[test]
fn pro_bounds_2_3_a_zero_deadline_is_permitted_but_a_negative_one_is_not() {
    // Bounds Protocol Section 2.3: a bound of 0 means the dimension is
    // disabled, so 0 is a meaningful value and must not be refused here.
    let mut zero = frozen_action_request();
    zero["deadline_ms"] = json!(0);
    assert!(schema::is_valid(SchemaName::ActionRequest, &zero).expect("compiles"));

    let mut negative = frozen_action_request();
    negative["deadline_ms"] = json!(-1);
    assert!(schema::validate(SchemaName::ActionRequest, &negative).is_err());

    let mut overflow = frozen_action_request();
    overflow["deadline_ms"] = json!(4_294_967_296u64);
    assert!(schema::validate(SchemaName::ActionRequest, &overflow).is_err());
}

// ---------------------------------------------------------------------------
// PROTO-CAP §5 — ActionResult
// ---------------------------------------------------------------------------

#[test]
fn pro_cap_5_the_frozen_action_result_satisfies_the_schema() {
    let value = json!({
        "request_id": REQUEST_ID,
        "status": "SUCCEEDED",
        "output": { "events": [] },
        "output_digest": DIGEST,
        "evidence": [],
        "receipt": null,
        "error": null,
        "duration_ms": 412
    });
    assert!(schema::is_valid(SchemaName::ActionResult, &value).expect("compiles"));
}

#[test]
fn pro_cap_5_unavailable_and_duplicate_suppressed_are_first_class_statuses() {
    for status in [
        "SUCCEEDED",
        "FAILED",
        "REJECTED",
        "CANCELLED",
        "UNAVAILABLE",
        "DUPLICATE_SUPPRESSED",
    ] {
        let value = json!({
            "request_id": REQUEST_ID,
            "status": status,
            "duration_ms": 1
        });
        assert!(
            schema::is_valid(SchemaName::ActionResult, &value).expect("compiles"),
            "{status} is a frozen status"
        );
    }
    for status in ["TIMEOUT", "PARTIAL", "SKIPPED", "succeeded"] {
        let value = json!({
            "request_id": REQUEST_ID,
            "status": status,
            "duration_ms": 1
        });
        assert!(schema::validate(SchemaName::ActionResult, &value).is_err());
    }
}

#[test]
fn pro_cap_6_1_every_frozen_action_error_kind_is_accepted() {
    for kind in serea_protocol::ActionErrorKind::WIRE_NAMES {
        let value = json!({
            "request_id": REQUEST_ID,
            "status": "FAILED",
            "duration_ms": 1,
            "error": {
                "kind": kind,
                "code": "SYNTHETIC_CODE",
                "message": "synthetic",
                "retryable": false,
                "host_action": "NONE"
            }
        });
        assert!(
            schema::is_valid(SchemaName::ActionResult, &value).expect("compiles"),
            "{kind} must be a valid frozen error kind"
        );
    }
    let unknown = json!({
        "request_id": REQUEST_ID,
        "status": "FAILED",
        "duration_ms": 1,
        "error": {
            "kind": "BOUND_EXCEEDED_MODEL_CALLS",
            "code": "SYNTHETIC_CODE",
            "message": "synthetic",
            "retryable": false,
            "host_action": "NONE"
        }
    });
    assert!(
        schema::validate(SchemaName::ActionResult, &unknown).is_err(),
        "bound exhaustion is a task-level reason, not a new ActionErrorKind"
    );
}

#[test]
fn pro_cap_3_1_an_error_code_must_be_a_code_not_prose() {
    let value = json!({
        "request_id": REQUEST_ID,
        "status": "FAILED",
        "duration_ms": 1,
        "error": {
            "kind": "PROVIDER_ERROR",
            "code": "the token expired while refreshing",
            "message": "synthetic",
            "retryable": false,
            "host_action": "NONE"
        }
    });
    assert!(schema::validate(SchemaName::ActionResult, &value).is_err());
}

#[test]
fn pro_cap_5_1_a_receipt_requires_its_proof_fields() {
    let receipt = |extra: Value| {
        let mut base = json!({
            "receipt_id": "rcp_01JQ8ZF4T7KMV2X9NPQ5RD8WCS",
            "capability_id": "calendar.events.create",
            "idempotency_key": IDEMPOTENCY_KEY,
            "effect_summary": "Created event on the primary calendar",
            "observed_at": "2026-10-01T09:14:23.880Z",
            "replay_safe": false
        });
        for (key, value) in extra.as_object().expect("object") {
            if value.is_null() {
                base.as_object_mut().expect("object").remove(key);
            } else {
                base[key] = value.clone();
            }
        }
        json!({
            "request_id": REQUEST_ID,
            "status": "SUCCEEDED",
            "duration_ms": 412,
            "receipt": base
        })
    };
    assert!(schema::is_valid(SchemaName::ActionResult, &receipt(json!({}))).expect("compiles"));
    assert!(
        schema::is_valid(
            SchemaName::ActionResult,
            &receipt(json!({ "provider_reference": "provider-ref-0001" }))
        )
        .expect("compiles"),
        "a provider_reference is the value reconciliation needs"
    );
    for missing in [
        "receipt_id",
        "idempotency_key",
        "effect_summary",
        "observed_at",
    ] {
        assert!(
            schema::validate(SchemaName::ActionResult, &receipt(json!({ missing: null }))).is_err(),
            "{missing} is required on a receipt"
        );
    }
}

#[test]
fn pro_cap_5_1_an_evidence_record_requires_every_declared_field() {
    let evidence = |drop: Option<&str>| {
        let mut record = json!({
            "evidence_id": EVENT_ID,
            "kind": "PROVIDER_RECEIPT",
            "capability_id": "calendar.events.list",
            "task_id": TASK_ID,
            "step_id": STEP_ID,
            "attempt": 1,
            "produced_at": "2026-10-01T09:14:23.902Z",
            "actor": { "kind": "PROVIDER", "id": "synthetic-provider", "version": "1.0.0" },
            "data_class": "PERSONAL",
            "payload_digest": DIGEST
        });
        if let Some(field) = drop {
            record.as_object_mut().expect("object").remove(field);
        }
        json!({
            "request_id": REQUEST_ID,
            "status": "SUCCEEDED",
            "duration_ms": 412,
            "evidence": [record]
        })
    };
    assert!(schema::is_valid(SchemaName::ActionResult, &evidence(None)).expect("compiles"));
    for field in [
        "evidence_id",
        "kind",
        "capability_id",
        "task_id",
        "step_id",
        "attempt",
        "produced_at",
        "actor",
        "data_class",
        "payload_digest",
    ] {
        assert!(
            schema::validate(SchemaName::ActionResult, &evidence(Some(field))).is_err(),
            "{field} is required on every evidence record"
        );
    }
}

#[test]
fn pro_goallatch_6_3_goal_result_evidence_is_a_frozen_kind() {
    // GoalLatch Adapter §7: GOAL_RESULT evidence must be host-observed. It is a
    // member of the frozen capability evidence vocabulary, not of EventKind.
    let mut record = json!({
        "evidence_id": EVENT_ID,
        "kind": "GOAL_RESULT",
        "capability_id": "host.goal.result",
        "task_id": TASK_ID,
        "step_id": STEP_ID,
        "attempt": 1,
        "produced_at": "2026-10-01T00:00:01.450Z",
        "actor": { "kind": "PROVIDER", "id": "goallatch", "version": "1.0.0" },
        "data_class": "PERSONAL",
        "payload_digest": DIGEST
    });
    let value = json!({
        "request_id": REQUEST_ID,
        "status": "SUCCEEDED",
        "duration_ms": 50,
        "evidence": [record.clone()]
    });
    assert!(schema::is_valid(SchemaName::ActionResult, &value).expect("compiles"));
    record["kind"] = json!("GOAL_COMPLETED");
    let value = json!({
        "request_id": REQUEST_ID,
        "status": "SUCCEEDED",
        "duration_ms": 50,
        "evidence": [record]
    });
    assert!(schema::validate(SchemaName::ActionResult, &value).is_err());
}

// ---------------------------------------------------------------------------
// Data Classification §4 — credential exclusion at the schema gate
// ---------------------------------------------------------------------------

#[test]
fn pro_data_4_a_credential_shaped_field_cannot_enter_the_request() {
    // Data Classification §4.1: an allowlist is the control. Every undeclared
    // name is refused, credential-shaped or not.
    for name in [
        "password",
        "api_key",
        "access_token",
        "refresh_token",
        "Authorization",
        "private_key",
        "client_secret",
        "oauth",
        "seed_phrase",
        "x_cred",
    ] {
        let mut polluted = frozen_action_request();
        polluted[name] = json!("synthetic");
        assert!(
            schema::validate(SchemaName::ActionRequest, &polluted).is_err(),
            "{name} must not be expressible"
        );
        let mut in_arguments = frozen_action_request();
        in_arguments["arguments"][name] = json!("synthetic");
        assert!(
            schema::validate(SchemaName::ActionRequest, &in_arguments).is_ok(),
            "arguments are validated against the capability's own closed schema in P5, \
             not by this envelope-level document"
        );
    }
}

#[test]
fn pro_data_3_1_no_protocol_type_carries_credential_bytes() {
    // DC7 is structural: the crate has no type that can hold a secret, so there
    // is nothing to serialise, log, or compare by accident.
    let handle = serea_protocol::CredentialHandle::new(
        serea_protocol::Digest::new(DIGEST).expect("valid digest"),
    );
    let rendered = serde_json::to_string(&handle).expect("serialises");
    assert_eq!(rendered, format!("\"{DIGEST}\""));
    assert!(!rendered.to_lowercase().contains("bearer"));
}

// ---------------------------------------------------------------------------
// Protocol Index §6 — the forward-compatible envelope
// ---------------------------------------------------------------------------

#[test]
fn pro_index_6_the_frozen_envelope_satisfies_the_schema() {
    let value = json!({
        "envelope_version": "1",
        "surface": "serea.action/2",
        "message_id": EVENT_ID,
        "correlation_id": TASK_ID,
        "causation_id": "evt_01JQ8Z9M4SBDT6K8H2WNRQVPXF",
        "issued_at": "2026-10-01T09:14:22.418Z",
        "data_class": "PERSONAL",
        "trace": { "task_id": TASK_ID, "step_id": STEP_ID },
        "payload": {}
    });
    assert!(schema::is_valid(SchemaName::Envelope, &value).expect("compiles"));
}

#[test]
fn pro_index_4_2_an_unknown_field_on_a_wire_surface_is_accepted_not_rejected() {
    let mut extended = json!({
        "envelope_version": "1",
        "surface": "serea.action/2",
        "message_id": EVENT_ID,
        "issued_at": "2026-10-01T09:14:22.418Z",
        "data_class": "PERSONAL",
        "payload": {},
        "future_minor_member": { "anything": [1, 2, 3] }
    });
    assert!(
        schema::is_valid(SchemaName::Envelope, &extended).expect("compiles"),
        "Protocol Index 4.2 rule 3: unknown fields on a forward-compatible surface are \
         ignored for semantics and preserved for round-trip"
    );
    // ... but the frozen *enums* still fail closed on the same surface.
    extended["data_class"] = json!("UNCLASSIFIED");
    assert!(schema::validate(SchemaName::Envelope, &extended).is_err());
}

#[test]
fn pro_index_6_every_envelope_surface_name_is_accepted() {
    for surface in [
        "serea.action/2",
        "serea.task/2",
        "serea.model/1",
        "serea.policy/1",
        "serea.approval/1",
        "serea.event/1",
        "serea.device/1",
        "serea.goallatch/1",
        "serea.data/1",
        "serea.bounds/1",
    ] {
        let value = json!({
            "envelope_version": "1",
            "surface": surface,
            "message_id": EVENT_ID,
            "issued_at": "2026-10-01T09:14:22.418Z",
            "data_class": "PUBLIC",
            "payload": {}
        });
        assert!(
            schema::is_valid(SchemaName::Envelope, &value).expect("compiles"),
            "{surface} is a frozen surface"
        );
    }
}

#[test]
fn pro_index_6_a_syntactically_malformed_timestamp_is_refused() {
    for bad in [
        "2026-10-01T09:14:22",
        "2026-10-01 09:14:22Z",
        "2026-10-01T09:14:22+09:00",
        "2026-13-01T00:00:00Z",
        "2026-10-01T24:00:00Z",
        "2026-10-01T09:60:00Z",
        "2026-10-01T09:14:22.4Z",
        "2026-10-01T09:14:22.418418Z",
        "",
    ] {
        let value = json!({
            "envelope_version": "1",
            "surface": "serea.action/2",
            "message_id": EVENT_ID,
            "issued_at": bad,
            "data_class": "PUBLIC",
            "payload": {}
        });
        assert!(
            schema::validate(SchemaName::Envelope, &value).is_err(),
            "{bad} must be refused"
        );
    }
}

#[test]
fn pro_index_6_the_rust_value_is_the_authority_for_calendar_validity() {
    // JSON Schema 2020-12 has no date arithmetic: ECMA-262 regular expressions
    // cannot express "day <= days in month". The schema therefore pins the
    // syntactic form, and the `Timestamp` value refuses a calendar-impossible
    // one. Every value Serea puts on the wire is deserialised into that type, so
    // the weaker pattern is never the only gate.
    let calendar_impossible = "2026-02-30T00:00:00Z";
    let value = json!({
        "envelope_version": "1",
        "surface": "serea.action/2",
        "message_id": EVENT_ID,
        "issued_at": calendar_impossible,
        "data_class": "PUBLIC",
        "payload": {}
    });
    assert!(
        schema::is_valid(SchemaName::Envelope, &value).expect("compiles"),
        "documented limit: the schema pattern is syntactic only"
    );
    assert!(
        serea_protocol::Timestamp::new(calendar_impossible).is_err(),
        "the authority for calendar validity refuses it"
    );
    assert!(
        serde_json::from_value::<serea_protocol::Envelope<Value>>(value).is_err(),
        "and a payload carrying it cannot be deserialised"
    );
}

#[test]
fn pro_index_4_2_a_payload_that_is_not_an_object_is_refused() {
    for payload in [json!("text"), json!([]), json!(1), json!(null)] {
        let value = json!({
            "envelope_version": "1",
            "surface": "serea.action/2",
            "message_id": EVENT_ID,
            "issued_at": "2026-10-01T09:14:22.418Z",
            "data_class": "PUBLIC",
            "payload": payload
        });
        assert!(schema::validate(SchemaName::Envelope, &value).is_err());
    }
}

// ---------------------------------------------------------------------------
// PROTO-TASK §2 and PROTO-EVENT §2 — the forward-compatible records
// ---------------------------------------------------------------------------

#[test]
fn pro_task_2_the_frozen_task_satisfies_the_schema() {
    let value = json!({
        "task_id": TASK_ID,
        "kind": "USER_REQUEST",
        "title": "Summarize today's mail",
        "state": "EXECUTING",
        "origin": {
            "kind": "USER_MESSAGE",
            "device_id": "dev_01JQ8ZC5N8TVG3K6MRQ2XW9JHF",
            "message_id": EVENT_ID
        },
        "data_class": "PERSONAL",
        "policy_class": "OBSERVE",
        "created_at": "2026-10-01T09:14:20.001Z",
        "updated_at": "2026-10-01T09:14:23.902Z",
        "attempt_budget": {
            "max_model_calls": 12,
            "max_tool_calls": 24,
            "max_attempts_per_step": 3
        },
        "steps": []
    });
    assert!(schema::is_valid(SchemaName::AssistantTask, &value).expect("compiles"));
}

#[test]
fn pro_task_2_1_the_policy_ceiling_accepts_only_the_frozen_risk_classes() {
    for class in [
        "OBSERVE",
        "LOCAL_STATE",
        "REVERSIBLE_WRITE",
        "EXTERNAL_WRITE",
        "COMMUNICATION",
        "ELEVATED_DEVICE",
        "DESTRUCTIVE",
        "CREDENTIAL",
    ] {
        let mut value = frozen_task();
        value["policy_class"] = json!(class);
        assert!(schema::is_valid(SchemaName::AssistantTask, &value).expect("compiles"));
    }
    for bad in ["observe", "SUPER_WRITE", "", "ROOT"] {
        let mut value = frozen_task();
        value["policy_class"] = json!(bad);
        assert!(schema::validate(SchemaName::AssistantTask, &value).is_err());
    }
}

fn frozen_task() -> Value {
    json!({
        "task_id": TASK_ID,
        "kind": "USER_REQUEST",
        "title": "Summarize today's mail",
        "state": "EXECUTING",
        "origin": { "kind": "USER_MESSAGE" },
        "data_class": "PERSONAL",
        "policy_class": "OBSERVE",
        "created_at": "2026-10-01T09:14:20.001Z",
        "updated_at": "2026-10-01T09:14:23.902Z",
        "attempt_budget": {
            "max_model_calls": 12,
            "max_tool_calls": 24,
            "max_attempts_per_step": 3
        },
        "steps": []
    })
}

#[test]
fn pro_task_4_1_every_frozen_task_state_is_accepted() {
    for state in [
        "RECEIVED",
        "PLANNING",
        "READY",
        "EXECUTING",
        "WAITING_APPROVAL",
        "WAITING_USER",
        "VERIFYING",
        "COMPLETED",
        "FAILED",
        "BLOCKED",
        "CANCELLED",
    ] {
        let mut value = frozen_task();
        value["state"] = json!(state);
        assert!(schema::is_valid(SchemaName::AssistantTask, &value).expect("compiles"));
    }
    for bad in ["PENDING", "RUNNING", "executing", "DONE"] {
        let mut value = frozen_task();
        value["state"] = json!(bad);
        assert!(schema::validate(SchemaName::AssistantTask, &value).is_err());
    }
}

#[test]
fn pro_bounds_2_3_a_negative_attempt_budget_is_refused() {
    let mut value = frozen_task();
    value["attempt_budget"]["max_model_calls"] = json!(-1);
    assert!(schema::validate(SchemaName::AssistantTask, &value).is_err());
    let mut value = frozen_task();
    value["attempt_budget"]["max_attempts_per_step"] = json!(0);
    assert!(
        schema::is_valid(SchemaName::AssistantTask, &value).expect("compiles"),
        "a bound of zero disables the dimension; only a negative bound is a configuration error"
    );
}

#[test]
fn pro_task_3_a_step_round_trips_and_stays_forward_compatible() {
    let step = json!({
        "step_id": STEP_ID,
        "task_id": TASK_ID,
        "sequence": 3,
        "kind": "CAPABILITY",
        "status": "SUCCEEDED",
        "attempt": 1,
        "lease_generation": 1,
        "idempotency_key": IDEMPOTENCY_KEY,
        "provider_id": "calendar",
        "capability_id": "calendar.events.list",
        "capability_version": "1.2.0",
        "input_digest": DIGEST,
        "result_digest": DIGEST,
        "side_effect_receipt": null,
        "started_at": "2026-10-01T09:14:22.100Z",
        "completed_at": "2026-10-01T09:14:22.512Z",
        "lease_owner": null,
        "lease_expires_at": null,
        "error": null
    });
    let mut value = frozen_task();
    value["steps"] = json!([step.clone()]);
    assert!(schema::is_valid(SchemaName::AssistantTask, &value).expect("compiles"));

    // `plan_revision` is named in Task Protocol Section 4.3 rule 5. An
    // architecture-minor field must validate, not break a reader.
    let mut minor_step = step.clone();
    minor_step["plan_revision"] = json!(3);
    value["steps"] = json!([minor_step]);
    assert!(schema::is_valid(SchemaName::AssistantTask, &value).expect("compiles"));

    // The invocation tuple remains conditionally required for CAPABILITY.
    let mut incomplete_step = step;
    incomplete_step
        .as_object_mut()
        .expect("object")
        .remove("idempotency_key");
    value["steps"] = json!([incomplete_step]);
    assert!(schema::validate(SchemaName::AssistantTask, &value).is_err());
}

#[test]
fn pro_task_2_the_task_surface_stays_forward_compatible() {
    let mut extended = frozen_task();
    extended["future_minor_field"] = json!("synthetic");
    assert!(schema::is_valid(SchemaName::AssistantTask, &extended).expect("compiles"));
}

#[test]
fn pro_event_2_the_frozen_event_satisfies_the_schema() {
    let value = json!({
        "envelope_version": "1",
        "surface": "serea.event/1",
        "message_id": EVENT_ID,
        "seq": "10427",
        "kind": "CAPABILITY_COMPLETED",
        "occurred_at": "2026-10-01T09:14:23.902Z",
        "correlation_id": TASK_ID,
        "causation_id": "evt_01JQ8ZB5G1XKP7N9M3QRT2V8WC",
        "actor": { "kind": "HOST", "id": "serea-core", "version": "0.1.0" },
        "data_class": "PERSONAL",
        "trace": { "task_id": TASK_ID, "step_id": STEP_ID, "attempt": 1 },
        "payload": {
            "capability_id": "calendar.events.list",
            "status": "SUCCEEDED",
            "duration_ms": 412,
            "output_digest": DIGEST
        }
    });
    assert!(schema::is_valid(SchemaName::Event, &value).expect("compiles"));
}

#[test]
fn pro_event_2_seq_must_be_a_decimal_string_not_a_number() {
    let mut value = frozen_event();
    value["seq"] = json!(10_427);
    assert!(
        schema::validate(SchemaName::Event, &value).is_err(),
        "Protocol Index Section 5: seq is a decimal string"
    );
    for bad in ["", "-1", "010427", "1e3", "10427 "] {
        let mut wrong = frozen_event();
        wrong["seq"] = json!(bad);
        assert!(
            schema::validate(SchemaName::Event, &wrong).is_err(),
            "seq={bad:?}"
        );
    }
}

fn frozen_event() -> Value {
    json!({
        "envelope_version": "1",
        "surface": "serea.event/1",
        "message_id": EVENT_ID,
        "seq": "10427",
        "kind": "CAPABILITY_COMPLETED",
        "occurred_at": "2026-10-01T09:14:23.902Z",
        "actor": { "kind": "HOST", "id": "serea-core", "version": "0.1.0" },
        "data_class": "PERSONAL",
        "payload": {}
    })
}

#[test]
fn pro_event_3_every_frozen_event_kind_is_accepted_and_an_unregistered_one_is_not() {
    for kind in serea_protocol::EventKind::WIRE_NAMES {
        let mut value = frozen_event();
        value["kind"] = json!(kind);
        assert!(
            schema::is_valid(SchemaName::Event, &value).expect("compiles"),
            "{kind} is a frozen event kind"
        );
    }
    for bad in [
        "CAPABILITY_ALMOST_COMPLETED",
        "DELETION_CASCADE_PARTIAL",
        "capability_completed",
        "TASK_START",
    ] {
        let mut value = frozen_event();
        value["kind"] = json!(bad);
        assert!(
            schema::validate(SchemaName::Event, &value).is_err(),
            "{bad} is not in the frozen Event Protocol Section 3 table"
        );
    }
}

#[test]
fn pro_data_8_2_the_deletion_cascade_event_is_registered_in_the_schema_too() {
    // Data Classification Section 8.2 step 4 requires the cascade to be
    // recorded with its counts. ADR-0017 registers the kind in Event Protocol
    // Section 3.7, so the checked-in schema and the Rust enum must agree.
    let mut value = frozen_event();
    value["kind"] = json!("DELETION_CASCADE_COMPLETED");
    value["payload"] = json!({
        "task_id": TASK_ID,
        "memory_items_deleted": 3,
        "provenance_rows_deleted": 3,
        "blobs_deleted": 5,
        "tombstones_written": 3
    });
    assert!(schema::is_valid(SchemaName::Event, &value).expect("compiles"));
    // Fail-closed still holds for a near-miss on the registered name.
    for bad in ["DELETION_CASCADE", "DELETION_CASCADE_COMPLETE"] {
        let mut near = frozen_event();
        near["kind"] = json!(bad);
        assert!(
            schema::validate(SchemaName::Event, &near).is_err(),
            "{bad} is not the registered kind"
        );
    }
}

#[test]
fn pro_event_6_2_a_model_actor_is_valid_but_grants_nothing() {
    let mut value = frozen_event();
    value["actor"] = json!({ "kind": "MODEL", "id": "nemotron-3-nano-30b", "version": "1.0.0" });
    assert!(schema::is_valid(SchemaName::Event, &value).expect("compiles"));
    for injected in ["authority", "approved", "grant_id", "risk_class"] {
        let mut polluted = frozen_event();
        polluted[injected] = json!("anything");
        // The event surface is forward-compatible, so an unknown member is
        // accepted and preserved rather than interpreted. What matters is that
        // nothing in this crate reads it as authority, which the typed
        // `SereaEvent` guarantees by having no such field.
        assert!(
            schema::is_valid(SchemaName::Event, &polluted).expect("compiles"),
            "{injected} is retained opaquely, never interpreted"
        );
    }
}

#[test]
fn pro_event_1_an_event_payload_must_be_an_object() {
    for payload in [json!([]), json!("text"), json!(1)] {
        let mut value = frozen_event();
        value["payload"] = payload;
        assert!(schema::validate(SchemaName::Event, &value).is_err());
    }
}

// ---------------------------------------------------------------------------
// Schema documents stay inside the process
// ---------------------------------------------------------------------------

/// Every `$ref` value in a document, so a test can assert they are all local.
fn collect_refs(node: &Value, document: SchemaName, out: &mut Vec<String>) {
    match node {
        Value::Object(map) => {
            for (key, child) in map {
                if key == "$ref" {
                    if let Some(reference) = child.as_str() {
                        assert!(
                            reference.starts_with("#/$defs/"),
                            "{document} $ref must be local to the document: {reference}"
                        );
                        out.push(reference.to_owned());
                    }
                }
                collect_refs(child, document, out);
            }
        }
        Value::Array(items) => {
            for item in items {
                collect_refs(item, document, out);
            }
        }
        _ => {}
    }
}

#[test]
fn the_jsonschema_dependency_cannot_resolve_a_reference_off_process() {
    // Protocol Index §5 and Capability Protocol §3.1 require JSON Schema
    // 2020-12; neither requires *fetching* one. The guarantee is a build
    // setting, so the test asserts the setting rather than pretending a string
    // search proves a process-level property: the `jsonschema` dependency is
    // declared with `default-features = false`, which compiles out
    // `resolve-http`, `resolve-file`, `resolve-async` and both TLS backends.
    let workspace = include_str!("../../../Cargo.toml");
    let declaration = workspace
        .lines()
        .find(|line| line.trim_start().starts_with("jsonschema ="))
        .expect("the workspace must declare jsonschema");
    assert_eq!(
        declaration.trim(),
        "jsonschema = { version = \"=0.58.3\", default-features = false }",
        "the validator must be pinned with every resolver feature off"
    );
    let protocol_manifest = include_str!("../Cargo.toml");
    let manifest_lines: Vec<&str> = workspace
        .lines()
        .chain(protocol_manifest.lines())
        .map(str::trim)
        .filter(|line| !line.starts_with('#'))
        .collect();
    for feature in [
        "resolve-http",
        "resolve-file",
        "resolve-async",
        "reqwest",
        "tls-ring",
    ] {
        assert!(
            manifest_lines.iter().all(|line| !line.contains(feature)),
            "{feature} must not be enabled by any manifest line"
        );
    }
    // Every checked-in document resolves only within itself: no `$ref` points
    // outside the document it appears in, so validation cannot reach another
    // document, a file, or a URL even in principle.
    for document in SchemaName::ALL {
        let parsed = document.json().expect("parses");
        let mut refs = Vec::new();
        collect_refs(&parsed, document, &mut refs);
        assert!(
            !refs.is_empty() || document == SchemaName::AssistantTask,
            "{document} is expected to use local $refs"
        );
    }
}

// ---------------------------------------------------------------------------
// Regression tests for the Pass B security findings
// ---------------------------------------------------------------------------

#[test]
fn pro_goallatch_3_2_no_schema_admits_the_adapter_namespace() {
    // Regression: the Rust validator refuses `goallatch.` as a capability
    // namespace (`G13`) but the three schema documents carried the identical
    // three-segment regex without the prohibition, so the schema and the type
    // disagreed about the same contract.
    // Each document that can carry a `CapabilityId` places it differently, and
    // each placement must carry the prohibition.
    let mut request = frozen_action_request();
    request["capability_id"] = json!("goallatch.goal.run");
    assert!(
        schema::validate(SchemaName::ActionRequest, &request).is_err(),
        "ActionRequest must refuse the adapter namespace"
    );
    let mut request = frozen_action_request();
    request["capability_id"] = json!("host.goal.run");
    assert!(schema::is_valid(SchemaName::ActionRequest, &request).expect("compiles"));

    let mut result = frozen_result();
    result["receipt"] = json!({
        "receipt_id": "rcp_01JQ8ZF4T7KMV2X9NPQ5RD8WCS",
        "capability_id": "goallatch.goal.start",
        "idempotency_key": IDEMPOTENCY_KEY,
        "effect_summary": "synthetic",
        "observed_at": "2026-10-01T09:14:23.880Z",
        "replay_safe": false
    });
    assert!(
        schema::validate(SchemaName::ActionResult, &result).is_err(),
        "ActionResult must refuse the adapter namespace"
    );

    let mut task = frozen_task();
    task["steps"] = json!([{
        "step_id": STEP_ID,
        "task_id": TASK_ID,
        "sequence": 0,
        "kind": "CAPABILITY",
        "status": "SUCCEEDED",
        "attempt": 1,
        "lease_generation": 1,
        "idempotency_key": IDEMPOTENCY_KEY,
        "provider_id": "host",
        "capability_version": "1.0.0",
        "capability_id": "goallatch.goal.run",
        "input_digest": DIGEST,
        "result_digest": DIGEST,
        "started_at": "2026-10-01T09:14:22.100Z",
        "completed_at": "2026-10-01T09:14:22.512Z"
    }]);
    assert!(
        schema::validate(SchemaName::AssistantTask, &task).is_err(),
        "AssistantTask must refuse the adapter namespace"
    );
}

#[test]
fn a_rejected_schema_value_is_never_echoed_into_the_error() {
    // Regression: the validator's own rendering quotes the offending instance,
    // so a credential-shaped rejection reached the error string and from there
    // any log or crash report (`DC7`). The reason now comes from the kind.
    let secret_shaped = "req_sk-proj-4a7f9c2b1d0e3f5a7c9e1b4d6f8a0c2eSECRETVALUE";
    let mut polluted = frozen_action_request();
    polluted["request_id"] = json!(secret_shaped);
    let error = schema::validate(SchemaName::ActionRequest, &polluted)
        .expect_err("a malformed request_id must fail closed");
    let rendered = error.to_string();
    assert!(
        rendered.contains("request_id"),
        "the location must be named: {rendered}"
    );
    assert!(
        !rendered.contains("sk-proj") && !rendered.contains("SECRETVALUE"),
        "the rejected value must not be echoed: {rendered}"
    );
    // The violation still names the JSON Pointer, so a caller can find the field.
    match error {
        SchemaError::InstanceInvalid { violations, .. } => {
            assert!(violations.iter().any(|v| v.instance_path == "/request_id"));
        }
        other => panic!("expected InstanceInvalid, got {other:?}"),
    }
}

#[test]
fn an_instance_deeper_than_the_bound_is_refused_before_the_validator_runs() {
    // Regression: `serde_json`'s 128-level recursion guard only applies when
    // parsing text, so a `Value` read from durable state could exhaust the stack
    // and abort the process rather than unwind.
    let mut nested = json!({"leaf": true});
    for _ in 0..serea_protocol::MAX_INSTANCE_DEPTH + 40 {
        nested = json!({ "nested": nested });
    }
    let error = schema::validate(SchemaName::AssistantTask, &nested)
        .expect_err("an over-deep instance must be refused");
    match error {
        SchemaError::TooDeep { depth, limit, .. } => {
            assert!(depth > limit);
            assert_eq!(limit, serea_protocol::MAX_INSTANCE_DEPTH);
        }
        other => panic!("expected TooDeep, got {other:?}"),
    }
    // A legitimately nested instance is unaffected.
    let mut ordinary = json!({"leaf": true});
    for _ in 0..8 {
        ordinary = json!({ "nested": ordinary });
    }
    let mut task = frozen_task();
    task["future_nested"] = ordinary.clone();
    assert!(schema::is_valid(SchemaName::AssistantTask, &task).expect("compiles"));
    assert!(
        schema::is_valid(SchemaName::AssistantTask, &nested).is_err(),
        "an over-deep value is refused through is_valid too, not just validate"
    );
}

fn frozen_result() -> Value {
    json!({
        "request_id": REQUEST_ID,
        "status": "SUCCEEDED",
        "duration_ms": 412
    })
}

// ---------------------------------------------------------------------------
// The checked-in schema enum sets are pinned against the Rust types
// ---------------------------------------------------------------------------

/// Reads `$defs.<name>.enum` out of a checked-in document.
fn schema_def_enum(document: SchemaName, definition: &str) -> Vec<String> {
    let parsed = document.json().expect("document parses");
    let values = parsed["$defs"][definition]["enum"]
        .as_array()
        .unwrap_or_else(|| panic!("{document} must declare $defs.{definition}.enum"));
    values
        .iter()
        .map(|value| {
            value
                .as_str()
                .unwrap_or_else(|| panic!("{document} {definition} must hold strings"))
                .to_owned()
        })
        .collect()
}

/// Reads the closed set behind `properties.<name>`, following a local `$ref` so
/// a property is asserted through the single declaration that owns its set.
fn schema_prop_enum(document: SchemaName, property: &str) -> Vec<String> {
    let parsed = document.json().expect("document parses");
    let mut node = parsed["properties"][property].clone();
    if let Some(reference) = node["$ref"].as_str() {
        let definition = reference
            .strip_prefix("#/$defs/")
            .unwrap_or_else(|| panic!("{document} {property} must use a local $ref"));
        node = parsed["$defs"][definition].clone();
    }
    let values = node["enum"]
        .as_array()
        .unwrap_or_else(|| panic!("{document} properties.{property} must resolve to a closed set"));
    values
        .iter()
        .map(|value| {
            value
                .as_str()
                .unwrap_or_else(|| panic!("{document} {property} must hold strings"))
                .to_owned()
        })
        .collect()
}

fn from_rust(values: &[&'static str]) -> Vec<String> {
    values.iter().map(|value| (*value).to_owned()).collect()
}

/// Every frozen set that appears in a checked-in document, pinned in **both**
/// directions against the Rust enum.
///
/// Two-directional equality is the point: iterating the Rust constant to check
/// the schema only catches removals, so an extra value added to a schema
/// `enum` would ship green. Each row therefore compares two independent lists.
///
/// | document | location | owning protocol |
/// | --- | --- | --- |
/// | action-request | `properties.data_class` | Data Classification §2 |
/// | action-request | `properties.requested_by` | Capability Protocol §4.1 |
/// | action-result | `properties.status` | Capability Protocol §5 |
/// | action-result | `$defs.errorKind` | Capability Protocol §6.1 |
/// | action-result | `$defs.dataClass` | Data Classification §2 |
/// | action-result | `$defs.evidenceKind` | Capability Protocol §7 |
/// | action-result | `$defs.actorKind` | Event Protocol §2.1 |
/// | envelope | `$defs.dataClass` | Data Classification §2 |
/// | assistant-task | `$defs.dataClass` | Data Classification §2 |
/// | assistant-task | `$defs.riskClass` | Policy Protocol §2 |
/// | assistant-task | `properties.kind` | Task Protocol §2 |
/// | assistant-task | `$defs.taskState` | Task Protocol §4.1 |
/// | assistant-task | `$defs.stepKind` | Task Protocol §3 |
/// | event | `$defs.eventKind` | Event Protocol §3 |
/// | event | `$defs.dataClass` | Data Classification §2 |
/// | event | `$defs.actorKind` | Event Protocol §2.1 |
#[test]
fn every_schema_enum_set_is_pinned_to_its_rust_type_in_both_directions() {
    use serea_protocol::types::{
        ActionErrorKind, ActionStatus, ActorKind, DataClass, EvidenceKind, RequestedBy, RiskClass,
        StepKind, TaskKind, TaskState,
    };

    /// One pinned set: the document, where the enum lives, the schema's list,
    /// the Rust list, and the protocol section that freezes it.
    type PinnedSet = (SchemaName, String, Vec<String>, Vec<String>, &'static str);
    let rows: [PinnedSet; 16] = [
        (
            SchemaName::ActionRequest,
            "properties.data_class".to_owned(),
            schema_prop_enum(SchemaName::ActionRequest, "data_class"),
            from_rust(DataClass::WIRE_NAMES),
            "Data Classification §2",
        ),
        (
            SchemaName::ActionRequest,
            "properties.requested_by".to_owned(),
            schema_prop_enum(SchemaName::ActionRequest, "requested_by"),
            from_rust(RequestedBy::WIRE_NAMES),
            "Capability Protocol §4.1",
        ),
        (
            SchemaName::ActionResult,
            "properties.status".to_owned(),
            schema_prop_enum(SchemaName::ActionResult, "status"),
            from_rust(ActionStatus::WIRE_NAMES),
            "Capability Protocol §5",
        ),
        (
            SchemaName::ActionResult,
            "$defs.errorKind".to_owned(),
            schema_def_enum(SchemaName::ActionResult, "errorKind"),
            from_rust(ActionErrorKind::WIRE_NAMES),
            "Capability Protocol §6.1",
        ),
        (
            SchemaName::ActionResult,
            "$defs.dataClass".to_owned(),
            schema_def_enum(SchemaName::ActionResult, "dataClass"),
            from_rust(DataClass::WIRE_NAMES),
            "Data Classification §2",
        ),
        (
            SchemaName::ActionResult,
            "$defs.evidenceKind".to_owned(),
            schema_def_enum(SchemaName::ActionResult, "evidenceKind"),
            from_rust(EvidenceKind::WIRE_NAMES),
            "Capability Protocol §7",
        ),
        (
            SchemaName::ActionResult,
            "$defs.actorKind".to_owned(),
            schema_def_enum(SchemaName::ActionResult, "actorKind"),
            from_rust(ActorKind::WIRE_NAMES),
            "Event Protocol §2.1",
        ),
        (
            SchemaName::Envelope,
            "$defs.dataClass".to_owned(),
            schema_def_enum(SchemaName::Envelope, "dataClass"),
            from_rust(DataClass::WIRE_NAMES),
            "Data Classification §2",
        ),
        (
            SchemaName::AssistantTask,
            "$defs.dataClass".to_owned(),
            schema_def_enum(SchemaName::AssistantTask, "dataClass"),
            from_rust(DataClass::WIRE_NAMES),
            "Data Classification §2",
        ),
        (
            SchemaName::AssistantTask,
            "$defs.riskClass".to_owned(),
            schema_def_enum(SchemaName::AssistantTask, "riskClass"),
            from_rust(RiskClass::WIRE_NAMES),
            "Policy Protocol §2",
        ),
        (
            SchemaName::AssistantTask,
            "properties.kind".to_owned(),
            schema_prop_enum(SchemaName::AssistantTask, "kind"),
            from_rust(TaskKind::WIRE_NAMES),
            "Task Protocol §2",
        ),
        (
            SchemaName::AssistantTask,
            "$defs.taskState".to_owned(),
            schema_def_enum(SchemaName::AssistantTask, "taskState"),
            from_rust(TaskState::WIRE_NAMES),
            "Task Protocol §4.1",
        ),
        (
            SchemaName::AssistantTask,
            "$defs.stepKind".to_owned(),
            schema_def_enum(SchemaName::AssistantTask, "stepKind"),
            from_rust(StepKind::WIRE_NAMES),
            "Task Protocol §3",
        ),
        (
            SchemaName::Event,
            "$defs.eventKind".to_owned(),
            schema_def_enum(SchemaName::Event, "eventKind"),
            from_rust(serea_protocol::EventKind::WIRE_NAMES),
            "Event Protocol §3",
        ),
        (
            SchemaName::Event,
            "$defs.dataClass".to_owned(),
            schema_def_enum(SchemaName::Event, "dataClass"),
            from_rust(DataClass::WIRE_NAMES),
            "Data Classification §2",
        ),
        (
            SchemaName::Event,
            "$defs.actorKind".to_owned(),
            schema_def_enum(SchemaName::Event, "actorKind"),
            from_rust(ActorKind::WIRE_NAMES),
            "Event Protocol §2.1",
        ),
    ];

    assert_eq!(rows.len(), 16, "no comparable frozen set is left unpinned");
    for (document, location, in_schema, in_rust, protocol) in rows {
        assert_eq!(
            in_schema, in_rust,
            "{document} {location} must hold exactly the frozen set from {protocol}"
        );
    }
}

/// The reverse direction, stated separately so the intent is unambiguous: a
/// value that is in the schema but not in the Rust type must be refused by both
/// gates. The rows above prove the lists are equal; this proves the schema gate
/// actually enforces its own list.
#[test]
fn every_schema_enum_gate_refuses_a_value_outside_its_own_list() {
    let cases: [(SchemaName, &str, Value); 6] = [
        (SchemaName::Envelope, "data_class", json!("PUBLICISH")),
        (
            SchemaName::ActionRequest,
            "data_class",
            json!("CREDENTIALS"),
        ),
        (
            SchemaName::ActionRequest,
            "requested_by",
            json!("DELEGATED"),
        ),
        (SchemaName::ActionResult, "status", json!("SKIPPED")),
        (SchemaName::AssistantTask, "state", json!("PENDING")),
        (
            SchemaName::Event,
            "kind",
            json!("CAPABILITY_MOSTLY_COMPLETED"),
        ),
    ];
    let base = |document: SchemaName| -> Value {
        let task = frozen_task();
        let result = frozen_result();
        match document {
            SchemaName::Envelope => json!({
                "envelope_version": "1",
                "surface": "serea.action/2",
                "message_id": EVENT_ID,
                "issued_at": "2026-10-01T09:14:22.418Z",
                "data_class": "PERSONAL",
                "payload": {}
            }),
            SchemaName::ActionRequest => frozen_action_request(),
            SchemaName::ActionResult => result,
            SchemaName::AssistantTask => task,
            _ => frozen_event(),
        }
    };
    for (document, field, bad) in cases {
        let mut value = base(document);
        value[field] = bad.clone();
        assert!(
            schema::validate(document, &value).is_err(),
            "{document} {field}={bad} must fail closed"
        );
    }
}

/// The nested, forward-compatible sub-objects must be forward-compatible on
/// *both* sides: the Rust type keeps an unknown member and the schema accepts
/// one. Closing either half alone is a divergence, and this test fails if
/// either half is closed.
#[test]
fn every_forward_compatible_nested_object_is_open_on_both_sides() {
    let open_in_schema = [
        (SchemaName::Envelope, "/properties/trace"),
        (SchemaName::Event, "/properties/trace"),
        (SchemaName::Event, "/properties/actor"),
        (SchemaName::AssistantTask, "/properties/origin"),
        (SchemaName::AssistantTask, "/properties/attempt_budget"),
        (SchemaName::AssistantTask, "/$defs/step"),
    ];
    for (document, pointer) in open_in_schema {
        let parsed = document.json().expect("parses");
        let mut node = &parsed;
        for segment in pointer.trim_start_matches('/').split('/') {
            node = &node[segment];
        }
        assert_ne!(
            node["additionalProperties"],
            Value::Bool(false),
            "{document}{pointer} must stay forward-compatible in the schema"
        );
    }

    // ... and the same members are accepted on the wire.
    let envelope = json!({
        "envelope_version": "1",
        "surface": "serea.action/2",
        "message_id": EVENT_ID,
        "issued_at": "2026-10-01T09:14:22.418Z",
        "data_class": "PERSONAL",
        "payload": {},
        "trace": { "future_correlation": "synthetic" }
    });
    assert!(schema::is_valid(SchemaName::Envelope, &envelope).expect("compiles"));

    let mut event = frozen_event();
    event["actor"]["future_attribution"] = json!("synthetic");
    assert!(schema::is_valid(SchemaName::Event, &event).expect("compiles"));

    let mut task = frozen_task();
    task["origin"]["future_minor_member"] = json!("synthetic");
    task["attempt_budget"]["future_minor_member"] = json!("synthetic");
    assert!(schema::is_valid(SchemaName::AssistantTask, &task).expect("compiles"));

    // The Rust halves keep the member as well.
    let parsed: Trace =
        serde_json::from_value(json!({ "future_correlation": "synthetic" })).expect("parses");
    assert_eq!(
        parsed.extensions.get("future_correlation"),
        Some(&json!("synthetic")),
        "Trace must retain an unknown member"
    );
    let mut origin = json!({ "kind": "USER_MESSAGE", "future_minor_member": "synthetic" });
    origin["kind"] = json!("USER_MESSAGE");
    let parsed: TaskOrigin = serde_json::from_value(origin).expect("parses");
    assert_eq!(
        parsed.extensions.get("future_minor_member"),
        Some(&json!("synthetic")),
        "TaskOrigin must retain an unknown member"
    );
    let parsed: AttemptBudget = serde_json::from_value(json!({
        "max_model_calls": 12,
        "max_tool_calls": 24,
        "max_attempts_per_step": 3,
        "future_minor_member": "synthetic"
    }))
    .expect("parses");
    assert_eq!(
        parsed.extensions.get("future_minor_member"),
        Some(&json!("synthetic")),
        "AttemptBudget must retain an unknown member"
    );

    // A negative control, so the assertions above cannot pass because the
    // assertions are inert: closing a nested object fails this test.
    let closed = json!({
        "envelope_version": "1",
        "surface": "serea.action/2",
        "message_id": EVENT_ID,
        "issued_at": "2026-10-01T09:14:22.418Z",
        "data_class": "PERSONAL",
        "payload": {},
        "trace": { "future_correlation": 1, "task_id": 2 }
    });
    assert!(
        !schema::is_valid(SchemaName::Envelope, &closed).expect("compiles"),
        "the test harness is live: an ill-typed trace member is still refused"
    );
}

#[test]
fn every_machine_readable_code_shares_one_grammar_with_the_schemas() {
    // Regression: the Rust validator accepted any run of `[A-Z0-9_]` while five
    // schema sites required `^[A-Z][A-Z0-9_]*$`, so `1FOO`, `_NONE` and `___`
    // were valid Rust and invalid on the wire.
    let bad = ["1FOO", "_NONE", "___", "FOO-BAR", "foo", "FOO BAR", "FOO\n"];
    for value in bad {
        assert!(ReasonCode::new(value).is_err(), "{value:?} must be refused");
        assert!(ErrorCode::new(value).is_err(), "{value:?} must be refused");
        assert!(
            BlockedReason::new(value).is_err(),
            "{value:?} must be refused"
        );
        assert!(HostAction::new(value).is_err(), "{value:?} must be refused");
        assert!(StepStatus::new(value).is_err(), "{value:?} must be refused");
    }
    for value in ["NONE", "FULL_RESYNC", "RECONCILED_ABSENT", "A1", "X0_Y"] {
        assert!(ReasonCode::new(value).is_ok(), "{value:?} is a legal code");
    }

    // ... and the schema agrees, through the very gate that failed closed.
    for bad in ["1FOO", "_NONE", "___"] {
        let mut value = frozen_result();
        value["error"] = json!({
            "kind": "PROVIDER_ERROR",
            "code": bad,
            "message": "synthetic",
            "retryable": false,
            "host_action": "NONE"
        });
        assert!(
            schema::validate(SchemaName::ActionResult, &value).is_err(),
            "schema must refuse code {bad:?} too"
        );
    }
}

#[test]
fn no_schema_imposes_a_free_text_ceiling() {
    // Bounds Protocol Section 2 declares its table authoritative and `B3` calls
    // a bound enforced anywhere else a bug. P0 leaves payload-byte, attachment
    // size, and object-count bounds unresolved, so no checked-in schema may
    // carry a free-text `maxLength`, and the Rust boundary may not either. The
    // value below is one character over the retired 4096 ceiling, so the fixture
    // stays small while still failing on the pre-fix schemas.
    let long = "s".repeat(4_097);

    let mut result = frozen_result();
    result["error"] = json!({
        "kind": "PROVIDER_ERROR",
        "code": "GMAIL_HISTORY_EXPIRED",
        "message": long.clone(),
        "retryable": false,
        "host_action": "FULL_RESYNC"
    });
    assert!(
        schema::is_valid(SchemaName::ActionResult, &result).expect("compiles"),
        "a diagnostic message is not length-bounded by the retired ceiling"
    );

    let mut event = frozen_event();
    event["actor"] = json!({ "kind": "HOST", "id": long.clone(), "version": "0.1.0" });
    assert!(
        schema::is_valid(SchemaName::Event, &event).expect("compiles"),
        "an actor id is not length-bounded by the retired ceiling"
    );

    let mut task = frozen_task();
    task["title"] = json!(long.clone());
    assert!(
        schema::is_valid(SchemaName::AssistantTask, &task).expect("compiles"),
        "a task title is not length-bounded by the retired ceiling"
    );

    let mut step_task = frozen_task();
    step_task["steps"] = json!([{
        "step_id": STEP_ID,
        "task_id": TASK_ID,
        "sequence": 3,
        "kind": "CAPABILITY",
        "status": "SUCCEEDED",
        "attempt": 1,
        "idempotency_key": IDEMPOTENCY_KEY,
        "provider_id": "calendar",
        "capability_id": "calendar.events.list",
        "capability_version": "1.2.0",
        "input_digest": DIGEST,
        "result_digest": DIGEST,
        "lease_generation": 1,
        "side_effect_receipt": {
            "receipt_id": "rcp_01JQ8ZF4T7KMV2X9NPQ5RD8WCS",
            "capability_id": "calendar.events.create",
            "idempotency_key": IDEMPOTENCY_KEY,
            "provider_reference": long.clone(),
            "effect_summary": long.clone(),
            "observed_at": "2026-10-01T09:14:23.902Z",
            "replay_safe": true
        },
        "started_at": "2026-10-01T09:14:22.100Z",
        "completed_at": "2026-10-01T09:14:22.512Z",
        "lease_owner": null,
        "lease_expires_at": null,
        "error": null
    }]);
    step_task["result_summary"] = json!(long.clone());
    assert!(
        schema::is_valid(SchemaName::AssistantTask, &step_task).expect("compiles"),
        "task summary and succeeded-step receipt text are not length-bounded"
    );

    let mut executing = step_task.clone();
    executing["steps"][0]["status"] = json!("EXECUTING");
    executing["steps"][0]["result_digest"] = Value::Null;
    executing["steps"][0]["completed_at"] = Value::Null;
    executing["steps"][0]["side_effect_receipt"] = Value::Null;
    executing["steps"][0]["lease_owner"] = json!(long.clone());
    executing["steps"][0]["lease_expires_at"] = json!("2026-10-01T09:15:22.100Z");
    assert!(schema::is_valid(SchemaName::AssistantTask, &executing).expect("compiles"));

    let mut failed = step_task.clone();
    failed["steps"][0]["status"] = json!("FAILED");
    failed["steps"][0]["side_effect_receipt"] = Value::Null;
    failed["steps"][0]["error"] = json!({
        "kind": "PROVIDER_ERROR", "code": "GMAIL_HISTORY_EXPIRED", "message": long.clone(),
        "retryable": false, "host_action": "FULL_RESYNC"
    });
    assert!(schema::is_valid(SchemaName::AssistantTask, &failed).expect("compiles"));

    // What survives: a machine-readable code keeps its own short ceiling, which
    // Capability Protocol Section 3.1 requires every schema string to carry.
    let mut coded = frozen_result();
    coded["error"] = json!({
        "kind": "PROVIDER_ERROR",
        "code": "A".repeat(65),
        "message": "synthetic",
        "retryable": false,
        "host_action": "NONE"
    });
    assert!(
        schema::validate(SchemaName::ActionResult, &coded).is_err(),
        "a code is still a code, not free text"
    );

    // ADR-0023 prose permits interior LF/TAB, but not empty, boundary
    // whitespace, CR, DEL or C1. The former whitespace divergence is closed.
    for bad in [
        "",
        "   ",
        " bad",
        "bad ",
        "bad\rvalue",
        "bad\u{7f}value",
        "bad\u{85}value",
    ] {
        let mut empty = frozen_result();
        empty["error"] = json!({
            "kind": "PROVIDER_ERROR",
            "code": "GMAIL_HISTORY_EXPIRED",
            "message": bad,
            "retryable": false,
            "host_action": "NONE"
        });
        assert!(
            schema::validate(SchemaName::ActionResult, &empty).is_err(),
            "{bad:?} must still be refused"
        );
    }
}

#[test]
fn model_text_carries_no_rust_layer_ceiling() {
    // Model input and output text is bounded by P0 itself, with
    // `max_output_tokens_per_call` (Model Protocol Section 9; Bounds Protocol
    // Section 2). The Rust layer must not add a second, narrower character
    // limit, and after the free-text ceiling is removed there is no asymmetry
    // left to document.
    let long = "s".repeat(4_097);
    let message = serea_protocol::ModelMessage {
        role: serea_protocol::MessageRole::new("user").expect("valid"),
        content: long.clone(),
    };
    assert!(
        message.content.len() > 4_096,
        "and over the retired ceiling"
    );
    let request = serea_protocol::ModelRequest {
        request_id: serea_protocol::RequestId::new("req_01JQ8ZA4H6NFG8K2M6RTV9XCWB")
            .expect("valid"),
        model_id: serea_protocol::ModelId::new("nemotron-3-nano-30b").expect("valid"),
        task_id: None,
        purpose: serea_protocol::ModelPurpose::Chat,
        messages: vec![message],
        system: Some(long),
        response_format: serea_protocol::ResponseFormat::Text,
        tools: Vec::new(),
        max_output_tokens: 2_048,
        temperature: 0.2,
        deadline_ms: 30_000,
        data_class: serea_protocol::DataClass::Personal,
    };
    assert!(
        serde_json::to_value(&request).is_ok(),
        "a long prompt is representable"
    );
    assert!(
        ErrorMessage::new("s".repeat(4_097)).is_ok(),
        "and a long diagnostic message is now accepted the same way"
    );
    // The code ceiling survives. The value must be a *well-formed* code that is
    // merely too long: a lowercase value would be refused by the code grammar
    // regardless of length, which would make this assertion unable to detect a
    // removed ceiling.
    assert!(
        ReasonCode::new("A".repeat(4_097)).is_err(),
        "while a well-formed code that is merely too long still is not"
    );
}

#[test]
fn a_step_receipt_and_error_are_validated_not_accepted_as_any_object() {
    // Regression: `side_effect_receipt` and `error` on a step were bare
    // `{"type": ["object", "null"]}`, so a step could carry an arbitrary object
    // including a credential-shaped one. Both are now closed and shape-checked.
    let base = |receipt: Value, error: Value| {
        let mut task = frozen_task();
        task["steps"] = json!([{
            "step_id": STEP_ID,
            "task_id": TASK_ID,
            "sequence": 0,
            "kind": "CAPABILITY",
            "status": if error.is_null() { "SUCCEEDED" } else { "FAILED" },
            "attempt": 1,
            "lease_generation": 1,
            "provider_id": "calendar",
            "capability_id": "calendar.events.list",
            "capability_version": "1.2.0",
            "idempotency_key": IDEMPOTENCY_KEY,
            "input_digest": DIGEST,
            "result_digest": DIGEST,
            "side_effect_receipt": receipt,
            "error": error,
            "started_at": "2026-10-01T09:14:22.100Z",
            "completed_at": "2026-10-01T09:14:22.512Z"
        }]);
        task
    };

    let good_receipt = json!({
        "receipt_id": "rcp_01JQ8ZF4T7KMV2X9NPQ5RD8WCS",
        "capability_id": "calendar.events.create",
        "idempotency_key": IDEMPOTENCY_KEY,
        "effect_summary": "Created one synthetic event",
        "observed_at": "2026-10-01T09:14:23.880Z",
        "replay_safe": false
    });
    assert!(
        schema::is_valid(
            SchemaName::AssistantTask,
            &base(good_receipt.clone(), Value::Null)
        )
        .expect("compiles"),
        "a well-formed receipt validates"
    );

    for hostile in [
        json!({ "password": "synthetic" }),
        json!({ "receipt_id": "not-a-receipt" }),
        json!({ "receipt_id": "rcp_01JQ8ZF4T7KMV2X9NPQ5RD8WCS", "extra": 1 }),
        json!([]),
        json!("a string"),
    ] {
        assert!(
            schema::validate(
                SchemaName::AssistantTask,
                &base(hostile.clone(), Value::Null)
            )
            .is_err(),
            "step receipt {hostile} must be refused"
        );
    }

    let hostile_error = json!({ "kind": "PROVIDER_ERROR", "refresh_token": "synthetic" });
    assert!(
        schema::validate(
            SchemaName::AssistantTask,
            &base(Value::Null, hostile_error.clone())
        )
        .is_err(),
        "step error {hostile_error} must be refused"
    );
    assert!(
        schema::is_valid(
            SchemaName::AssistantTask,
            &base(
                Value::Null,
                json!({
                    "kind": "PROVIDER_ERROR",
                    "code": "SYNTHETIC_CODE",
                    "message": "synthetic",
                    "retryable": false,
                    "host_action": "NONE"
                })
            )
        )
        .expect("compiles"),
        "a well-formed step error validates"
    );
}
