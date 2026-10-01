//! Frozen protocol type tests: enum membership, serialization names,
//! round-trips, unknown-variant rejection, and the closed-versus
//! forward-compatible surface split.
//!
//! Frozen source: `docs/protocols/00-protocol-index.md` §2, §4.2, §5, §6;
//! Capability Protocol §3.1, §4, §5, §6, §7; Task Protocol §2, §3, §4.1;
//! Event Protocol §2, §3; Model Protocol §3, §4, §5; Data Classification §2,
//! §3.1.
//!
//! All fixtures are synthetic and copied from the frozen example blocks. No
//! fixture contains a credential, and no fixture names Codex as an enabled
//! route.

use serde_json::{Value, json};

use serea_protocol::EventKind;
use serea_protocol::errors::{ContractRule, ProtocolError};
use serea_protocol::ids::ModelId;
use serea_protocol::ids::{
    CapabilityId, Digest, EventId, IdempotencyKey, ProviderId, RequestId, StepId, TaskId,
};
use serea_protocol::types::{
    ActionError, ActionErrorKind, ActionRequest, ActionResult, ActionStatus, Actor, ActorId,
    ActorKind, AssistantTask, Authorization, BlockedReason, CapabilityDescriptor,
    CapabilityDescriptorDraft, CostClass, CredentialHandle, DataClass, DescriptorDescription,
    DescriptorTitle, Envelope, EnvelopeVersion, ErrorCode, EvidenceKind, FailureReason,
    FinishReason, HostAction, IdempotencySupport, JsonSchemaMode, JsonSchemaRef, MessageRole,
    ModelCapabilities, ModelDescriptor, ModelError, ModelErrorCode, ModelMessage, ModelPurpose,
    ModelRequest, ModelResponse, ModelUsage, ProviderHealth, ProviderReference, ReasonCode,
    ReplaySafety, RequestedBy, ResponseFormat, RiskClass, RootRequirement, SemVer, Seq, SereaEvent,
    SideEffectClass, StepKind, StepStatus, TaskKind, TaskOriginKind, TaskState, TaskStep,
    Timestamp, TokenCount, Trace, WireSurface,
};

const TASK_ID: &str = "tsk_01JQ8Z9K3M7QWXR4V2T6YH0BNA";
const STEP_ID: &str = "stp_01JQ8Z9M3R2CVN8H5FWK7PQDSF";
const REQUEST_ID: &str = "req_01JQ8ZA4H6NFG8K2M6RTV9XCWB";
const EVENT_ID: &str = "evt_01JQ8ZB7H2XKM9P4QW7NRT5YCD";
const DEVICE_ID: &str = "dev_01JQ8ZC5N8TVG3K6MRQ2XW9JHF";
const RECEIPT_ID: &str = "rcp_01JQ8ZF4T7KMV2X9NPQ5RD8WCS";
const DIGEST: &str = "sha256:3b1f0c9e2a7d4e6b8f0a2c4d6e8b0d2f4a6c8e0b2d4f6a8c0e2b4d6f8a0c2e4b";
const IDEMPOTENCY_KEY: &str =
    "idk_9f2c1a7e4b6d0f8a3c5e9b1d7f2a4c6e8b0d3f5a7c9e1b4d6f8a0c2e4b6d8f9a";

fn task_id() -> TaskId {
    TaskId::new(TASK_ID).expect("valid")
}
fn step_id() -> StepId {
    StepId::new(STEP_ID).expect("valid")
}
fn request_id() -> RequestId {
    RequestId::new(REQUEST_ID).expect("valid")
}
fn event_id() -> EventId {
    EventId::new(EVENT_ID).expect("valid")
}
fn digest() -> Digest {
    Digest::new(DIGEST).expect("valid")
}
fn idempotency_key() -> IdempotencyKey {
    IdempotencyKey::new(IDEMPOTENCY_KEY).expect("valid")
}
fn timestamp(value: &str) -> Timestamp {
    Timestamp::new(value).expect("valid frozen timestamp")
}
fn actor() -> Actor {
    Actor {
        kind: ActorKind::Host,
        id: ActorId::new("serea-core").expect("valid"),
        version: SemVer::new("0.1.0").expect("valid"),
        extensions: Default::default(),
    }
}

// ---------------------------------------------------------------------------
// Timestamps and other validated scalars
// ---------------------------------------------------------------------------

mod timestamps {
    use super::*;
    use serea_protocol::errors::{ValueField, ValueRejection};

    #[test]
    fn accepts_the_frozen_wire_form() {
        for value in [
            "2026-10-01T09:14:22.418Z",
            "2026-10-01T09:14:22Z",
            "2024-02-29T00:00:00.000Z",
            "2000-02-29T23:59:59.999Z",
        ] {
            assert_eq!(timestamp(value).as_str(), value);
        }
    }

    #[test]
    fn rejects_malformed_and_impossible_values() {
        let cases = [
            "",
            "2026-10-01",
            "2026-10-01T09:14:22",
            "2026-10-01T09:14:22.418",
            "2026-10-01 09:14:22Z",
            "2026-10-01T09:14:22+09:00",
            "2026-10-01T09:14:22.418+00:00",
            "2026-13-01T00:00:00Z",
            "2026-00-01T00:00:00Z",
            "2026-10-32T00:00:00Z",
            "2026-10-00T00:00:00Z",
            "2026-02-30T00:00:00Z",
            "2026-04-31T00:00:00Z",
            "2026-02-29T00:00:00Z",
            "1900-02-29T00:00:00Z",
            "2026-10-01T24:00:00Z",
            "2026-10-01T09:60:00Z",
            "2026-10-01T09:14:60Z",
            "2026-10-01T09:14:22.4Z",
            "2026-10-01T09:14:22.418418Z",
            "2026-10-01T09:14:22Z\n",
            // Regression: the validator once indexed the fractional-seconds
            // position unconditionally, so a 20-byte value whose final byte
            // was not `Z` read past the end and panicked. Reachable from every
            // wire deserialisation, so it had to fail closed instead.
            "2026-10-01T09:14:22X",
            "2026-10-01T09:14:22x",
            "2026-10-01T09:14:22 ",
            "2026-10-01T09:14:2",
            "202-10-01T09:14:22Z",
            "2026-1-01T09:14:22Z",
        ];
        for value in cases {
            assert_eq!(
                Timestamp::new(value),
                Err(ProtocolError::MalformedValue {
                    field: ValueField::Timestamp,
                    reason: ValueRejection::Malformed,
                }),
                "{value:?} must be rejected"
            );
        }
    }
}

mod wire_surface {
    use super::*;
    use serea_protocol::errors::{ValueField, ValueRejection};

    #[test]
    fn accepts_the_frozen_surface_names() {
        for value in [
            "serea.action/1",
            "serea.task/1",
            "serea.model/1",
            "serea.policy/1",
            "serea.approval/1",
            "serea.event/1",
            "serea.device/1",
            "serea.goallatch/1",
            "serea.data/1",
            "serea.bounds/1",
        ] {
            assert_eq!(
                WireSurface::new(value).expect("frozen surface").as_str(),
                value
            );
        }
        assert_eq!(WireSurface::ACTION, "serea.action/1");
        assert_eq!(WireSurface::EVENT, "serea.event/1");
    }

    #[test]
    fn rejects_a_surface_that_is_not_a_frozen_shape() {
        for value in [
            "",
            "serea.action",
            "serea.action/",
            "serea.action/0",
            "serea.action/01",
            "serea./1",
            "action/1",
            "serea.ACTION/1",
            "serea.action/1 ",
            "https://serea.local/action/1",
        ] {
            assert_eq!(
                WireSurface::new(value),
                Err(ProtocolError::MalformedValue {
                    field: ValueField::WireSurface,
                    reason: ValueRejection::Malformed,
                }),
                "{value:?} must be rejected"
            );
        }
    }

    #[test]
    fn envelope_version_is_a_decimal_major_with_no_leading_zero() {
        assert_eq!(EnvelopeVersion::new("1").expect("valid").major(), 1);
        assert_eq!(EnvelopeVersion::SUPPORTED_MAJOR, 1);
        for value in ["", "0", "01", "1.0", "one", "1 "] {
            assert!(
                EnvelopeVersion::new(value).is_err(),
                "{value:?} must be rejected"
            );
        }
    }
}

mod semver {
    use super::*;

    #[test]
    fn accepts_semver_with_optional_pre_release_and_build() {
        for value in [
            "1.2.0",
            "0.0.1",
            "1.0.0-alpha.1",
            "1.0.0+build.5",
            "1.0.0-rc.1+b.2",
        ] {
            assert_eq!(SemVer::new(value).expect("valid").as_str(), value);
        }
    }

    #[test]
    fn rejects_anything_that_is_not_semver() {
        for value in [
            "", "1", "1.2", "1.2.3.4", "v1.2.3", "01.2.3", "1.02.3", "1.2.03", "1.2.3-", "1.2.3+",
            "1.2.-3", "1.2.x",
        ] {
            assert!(SemVer::new(value).is_err(), "{value:?} must be rejected");
        }
    }
}

mod machine_readable_codes {
    use super::*;
    use serea_protocol::errors::{ValueField, ValueRejection};

    #[test]
    fn accepts_the_codes_the_frozen_documents_name() {
        for value in [
            "PROVIDER_ERROR",
            "BOUND_EXCEEDED",
            "POLICY_DENIED",
            "MODEL_BUDGET_EXHAUSTED",
            "BOUND_EXCEEDED_MODEL_CALLS",
            "GMAIL_HISTORY_EXPIRED",
            "GOAL_EXECUTION_FAILED",
        ] {
            assert!(ReasonCode::new(value).is_ok());
            assert!(FailureReason::new(value).is_ok());
            assert!(ErrorCode::new(value).is_ok());
        }
        for value in ["AMBIGUOUS_EFFECT", "UNRECOGNISED_STATE", "APPROVAL_BACKLOG"] {
            assert!(BlockedReason::new(value).is_ok());
        }
        assert!(HostAction::new("FULL_RESYNC").is_ok());
        assert!(HostAction::new("NONE").is_ok());
        assert!(TaskOriginKind::new("USER_MESSAGE").is_ok());
        assert!(StepStatus::new("SUCCEEDED").is_ok());
        assert!(StepStatus::new("RECONCILED_ABSENT").is_ok());
        assert!(ModelErrorCode::new("PROVIDER_TIMEOUT").is_ok());
    }

    #[test]
    fn refuses_prose_where_a_code_is_required() {
        // Event Protocol Section 4: control flow must never depend on parsing
        // prose, so a code field may not hold a sentence.
        let cases = [
            "",
            "   ",
            "provider error",
            "Provider error",
            "PROVIDER ERROR",
            "PROVIDER-ERROR",
            "PROVIDER_ERROR: the token expired",
            "PROVIDER_ERROR\n",
            "PROVIDER_ERROR\t",
            &"A".repeat(65),
        ];
        for value in cases {
            let expected = if value.trim().is_empty() {
                ValueRejection::Empty
            } else if value.len() > 64 {
                ValueRejection::TooLong
            } else {
                ValueRejection::Malformed
            };
            assert_eq!(
                ReasonCode::new(value),
                Err(ProtocolError::MalformedValue {
                    field: ValueField::ReasonCode,
                    reason: expected,
                }),
                "{value:?} must be rejected"
            );
        }
    }

    #[test]
    fn labels_refuse_control_characters() {
        for value in ["bad\nvalue", "bad\tvalue", "bad\0value", "bad\u{7f}value"] {
            assert!(ActorId::new(value).is_err(), "{value:?} must be rejected");
        }
        assert!(ProviderReference::new("provider-ref-0001").is_ok());
    }

    #[test]
    fn message_roles_accept_the_frozen_example() {
        assert_eq!(MessageRole::new("user").expect("valid").as_str(), "user");
        assert_eq!(
            MessageRole::new("system").expect("valid").as_str(),
            "system"
        );
        for value in ["User", "user ", "", "u ser"] {
            assert!(
                MessageRole::new(value).is_err(),
                "{value:?} must be rejected"
            );
        }
    }
}

mod numeric_newtypes {
    use super::*;

    #[test]
    fn sequence_numbers_and_token_counts_serialise_as_decimal_strings() {
        // Protocol Index Section 5: 64-bit quantities that could exceed
        // JavaScript's exact integer range are decimal strings.
        assert_eq!(
            serde_json::to_string(&Seq::new(10_427)).expect("serialises"),
            "\"10427\""
        );
        assert_eq!(
            serde_json::to_string(&TokenCount::new(1_840)).expect("serialises"),
            "\"1840\""
        );
        assert_eq!(
            serde_json::from_str::<Seq>("\"10427\"").expect("parses"),
            Seq::new(10_427)
        );
        assert_eq!(Seq::new(10_427).get(), 10_427);
    }

    #[test]
    fn sequence_numbers_reject_a_json_number_and_junk() {
        for raw in [
            "10427",
            "\"\"",
            "\"-1\"",
            "\"1e3\"",
            "\"10427 \"",
            "\"010427\"",
        ] {
            assert!(
                serde_json::from_str::<Seq>(raw).is_err(),
                "{raw} must be rejected: seq is a decimal string, never a number"
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Frozen enums
// ---------------------------------------------------------------------------

mod frozen_enum_sets {
    use super::*;
    use serea_protocol::types::{ActorKind, TaskKind};

    #[test]
    fn pro_cap_3_1_the_descriptor_enum_sets_are_exactly_the_frozen_values() {
        // Capability Protocol §3.1 field semantics.
        assert_eq!(
            SideEffectClass::WIRE_NAMES,
            [
                "NONE",
                "LOCAL_STATE",
                "DEVICE_STATE",
                "EXTERNAL_WRITE",
                "COMMUNICATION",
                "ELEVATED_DEVICE",
            ]
        );
        assert_eq!(
            Authorization::WIRE_NAMES,
            ["NONE", "DEVICE_USER", "SCOPED_GRANT", "CREDENTIAL_HANDOFF"]
        );
        assert_eq!(
            RootRequirement::WIRE_NAMES,
            ["NOT_REQUIRED", "OPTIONAL_ROOT", "REQUIRES_ROOT"]
        );
        assert_eq!(
            IdempotencySupport::WIRE_NAMES,
            ["NATIVE", "EMULATED", "NONE"]
        );
        assert_eq!(CostClass::WIRE_NAMES, ["FREE", "LOW", "PAID"]);
        assert_eq!(
            ReplaySafety::WIRE_NAMES,
            ["IDEMPOTENT", "CONDITIONAL", "NON_REPLAYABLE"]
        );
    }

    #[test]
    fn pro_cap_4_1_requested_by_is_exactly_the_five_frozen_values() {
        // Capability Protocol §4.1: provenance for audit that never grants
        // authority.
        assert_eq!(
            RequestedBy::WIRE_NAMES,
            ["MODEL", "USER", "SCHEDULER", "PROACTIVE_WATCHER", "SYSTEM"]
        );
    }

    #[test]
    fn pro_cap_5_action_status_is_exactly_the_six_frozen_values() {
        assert_eq!(
            ActionStatus::WIRE_NAMES,
            [
                "SUCCEEDED",
                "FAILED",
                "REJECTED",
                "CANCELLED",
                "UNAVAILABLE",
                "DUPLICATE_SUPPRESSED",
            ]
        );
        assert!(!ActionStatus::WIRE_NAMES.contains(&"TIMEOUT"));
        assert!(!ActionStatus::WIRE_NAMES.contains(&"SKIPPED"));
    }

    #[test]
    fn pro_cap_6_1_action_error_kind_is_exactly_the_thirteen_frozen_values() {
        assert_eq!(
            ActionErrorKind::WIRE_NAMES,
            [
                "VALIDATION",
                "UNKNOWN_CAPABILITY",
                "POLICY_DENIED",
                "APPROVAL_REQUIRED",
                "APPROVAL_DENIED",
                "CAPABILITY_UNAVAILABLE",
                "PROVIDER_ERROR",
                "PROVIDER_TIMEOUT",
                "RATE_LIMITED",
                "AUTH_EXPIRED",
                "DUPLICATE_SUPPRESSED",
                "AMBIGUOUS",
                "INTERNAL",
            ]
        );
    }

    #[test]
    fn pro_cap_7_evidence_kind_is_exactly_the_six_frozen_values() {
        // Capability Protocol §7: a vocabulary the Event Protocol does not own.
        assert_eq!(
            EvidenceKind::WIRE_NAMES,
            [
                "ACTION_ATTEMPTED",
                "PROVIDER_RECEIPT",
                "CAPABILITY_OBSERVATION",
                "GOAL_RESULT",
                "POLICY_DENIAL",
                "RECONCILIATION",
            ]
        );
    }

    #[test]
    fn pro_data_2_data_class_is_exactly_the_five_frozen_values() {
        assert_eq!(
            DataClass::WIRE_NAMES,
            ["PUBLIC", "PERSONAL", "PRIVATE", "SECRET", "CREDENTIAL"]
        );
    }

    #[test]
    fn pro_policy_2_risk_class_is_exactly_the_eight_frozen_values_in_harm_order() {
        let ordered = [
            RiskClass::Observe,
            RiskClass::LocalState,
            RiskClass::ReversibleWrite,
            RiskClass::ExternalWrite,
            RiskClass::Communication,
            RiskClass::ElevatedDevice,
            RiskClass::Destructive,
            RiskClass::Credential,
        ];
        let names: Vec<&str> = ordered.iter().map(|class| class.wire_name()).collect();
        assert_eq!(
            names.as_slice(),
            [
                "OBSERVE",
                "LOCAL_STATE",
                "REVERSIBLE_WRITE",
                "EXTERNAL_WRITE",
                "COMMUNICATION",
                "ELEVATED_DEVICE",
                "DESTRUCTIVE",
                "CREDENTIAL",
            ]
        );
        assert_eq!(
            ordered
                .iter()
                .map(|class| class.rank())
                .collect::<Vec<u8>>(),
            vec![0, 1, 2, 3, 4, 5, 6, 7]
        );
    }

    #[test]
    fn pro_task_2_task_kind_is_exactly_the_five_frozen_values() {
        assert_eq!(
            TaskKind::WIRE_NAMES,
            [
                "USER_REQUEST",
                "SCHEDULED",
                "PROACTIVE",
                "DELEGATED_HOST_GOAL",
                "MAINTENANCE",
            ]
        );
    }

    #[test]
    fn pro_task_3_step_kind_is_exactly_the_eight_frozen_values() {
        assert_eq!(
            StepKind::WIRE_NAMES,
            [
                "CAPABILITY",
                "MODEL_TURN",
                "WAIT_APPROVAL",
                "WAIT_USER",
                "WAIT_SCHEDULE",
                "VERIFY",
                "NOTIFY",
                "DELEGATE",
            ]
        );
    }

    #[test]
    fn pro_task_4_1_task_state_is_exactly_the_eleven_frozen_values() {
        assert_eq!(
            TaskState::WIRE_NAMES,
            [
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
            ]
        );
    }

    #[test]
    fn pro_event_2_1_actor_kind_is_exactly_the_six_frozen_values() {
        assert_eq!(
            ActorKind::WIRE_NAMES,
            ["HOST", "USER", "MODEL", "PROVIDER", "SCHEDULER", "SYSTEM"]
        );
    }

    #[test]
    fn pro_model_3_purpose_is_exactly_the_six_frozen_values() {
        assert_eq!(
            ModelPurpose::WIRE_NAMES,
            [
                "CHAT",
                "PLANNING",
                "EXTRACTION",
                "ANALYSIS",
                "PROACTIVE",
                "STRUCTURED_REPAIR",
            ]
        );
    }

    #[test]
    fn pro_model_4_finish_reason_is_exactly_the_five_frozen_values() {
        assert_eq!(
            FinishReason::WIRE_NAMES,
            [
                "STOP",
                "LENGTH",
                "CONTENT_FILTER",
                "ERROR",
                "STRUCTURE_INVALID"
            ]
        );
    }

    #[test]
    fn pro_model_5_json_schema_mode_is_exactly_the_three_frozen_values() {
        assert_eq!(
            JsonSchemaMode::WIRE_NAMES,
            ["STRICT", "BEST_EFFORT", "UNSUPPORTED"]
        );
    }

    #[test]
    fn pro_cap_9_provider_health_is_exactly_the_two_frozen_values() {
        assert_eq!(ProviderHealth::WIRE_NAMES, ["READY", "DEGRADED"]);
    }

    #[test]
    fn pro_event_3_event_kind_is_exactly_the_fifty_nine_frozen_values() {
        let frozen = [
            // §3.1 Task lifecycle
            "TASK_CREATED",
            "TASK_STARTED",
            "TASK_STATE_CHANGED",
            "TASK_COMPLETED",
            "TASK_FAILED",
            "TASK_CANCELLED",
            "TASK_BLOCKED",
            "TASK_RESUMED",
            // §3.2 Model activity
            "MODEL_CALLED",
            "MODEL_COMPLETED",
            "MODEL_FAILED",
            "MODEL_OUTPUT_INVALID",
            "MODEL_REPAIRED",
            "MODEL_FALLBACK",
            // §3.3 Capability activity
            "CAPABILITY_REQUESTED",
            "CAPABILITY_COMPLETED",
            "CAPABILITY_DENIED",
            "CAPABILITY_UNAVAILABLE",
            "CAPABILITY_DUPLICATE_SUPPRESSED",
            "CAPABILITY_RECEIPT_RECORDED",
            "CAPABILITY_RECONCILED",
            "MODEL_SCHEMA_VIOLATION",
            "TOOL_DUPLICATE_WINDOW_BYPASSED",
            // §3.4 Approval activity
            "APPROVAL_REQUIRED",
            "APPROVAL_GRANTED",
            "APPROVAL_DENIED",
            "APPROVAL_EXPIRED",
            "APPROVAL_CONSUMED",
            "APPROVAL_EXPIRED_UNUSED",
            // §3.5 Policy and bounds
            "POLICY_CHANGED",
            "BOUND_EXCEEDED",
            "BOUNDS_CHANGED",
            "POLICY_VIOLATION_ATTEMPT",
            "MODEL_BUDGET_EXHAUSTED",
            "MODEL_FALLBACK_EXHAUSTED",
            // §3.6 Device activity
            "DEVICE_CONNECTED",
            "DEVICE_DISCONNECTED",
            "DEVICE_PAIRED",
            "DEVICE_UNPAIRED",
            "DEVICE_CAPABILITIES_REPORTED",
            "DEVICE_REVOKED",
            // §3.7 Memory and proactive
            "MEMORY_ITEM_WRITTEN",
            "MEMORY_ITEM_UPDATED",
            "MEMORY_ITEM_DELETED",
            "PROPOSAL_CREATED",
            "PROPOSAL_DISMISSED",
            // §3.8 Event history and sequence integrity
            "EVENT_HISTORY_EXPIRED",
            "EVENT_SEQUENCE_CORRUPTION",
            // §3.9 Scheduler activity
            "SCHEDULE_CREATED",
            "SCHEDULE_UPDATED",
            "SCHEDULE_PAUSED",
            "SCHEDULE_RESUMED",
            "SCHEDULE_CANCELLED",
            "SCHEDULE_OCCURRENCE_MISSED",
            "SCHEDULE_TASK_CREATED",
            "SCHEDULE_CATCH_UP_DEFERRED",
            // §3.10 Provider sync
            "PROVIDER_SYNC_STARTED",
            "PROVIDER_SYNC_COMPLETED",
            "PROVIDER_SYNC_DEGRADED",
        ];
        assert_eq!(frozen.len(), 59);
        assert_eq!(EventKind::WIRE_NAMES, frozen);
    }

    #[test]
    fn a_bound_exhaustion_is_not_a_new_action_error_kind() {
        // Bounds Protocol §4.4, B10: exhaustion is a task-level failure reason,
        // and the frozen ActionErrorKind set gains no member for it.
        for code in [
            "BOUND_EXCEEDED_MODEL_CALLS",
            "BOUND_EXCEEDED_TOOL_CALLS",
            "BOUND_EXCEEDED_REPEATED_ACTION",
            "BOUND_EXCEEDED_WALL_CLOCK",
            "BOUND_EXCEEDED_TOKEN_BUDGET",
        ] {
            assert!(
                !ActionErrorKind::WIRE_NAMES.contains(&code),
                "{code} must not become an ActionErrorKind"
            );
            assert!(
                ErrorCode::new(code).is_ok(),
                "{code} belongs in ActionError.code"
            );
        }
    }

    #[test]
    fn the_evidence_vocabulary_is_distinct_from_the_event_vocabulary() {
        // Capability Protocol §7.
        for evidence in EvidenceKind::WIRE_NAMES {
            assert!(
                !EventKind::WIRE_NAMES.contains(evidence),
                "{evidence} belongs to the evidence vocabulary only"
            );
        }
    }

    #[test]
    fn data_class_composition_takes_the_maximum_and_defaults_to_credential() {
        // Data Classification §2.1 and §2.2.
        assert_eq!(
            DataClass::compose_all([DataClass::Personal, DataClass::Personal, DataClass::Private]),
            DataClass::Private
        );
        assert_eq!(
            DataClass::compose_all([DataClass::Public, DataClass::Secret]),
            DataClass::Secret
        );
        assert_eq!(
            DataClass::compose_all([DataClass::Public, DataClass::Public]),
            DataClass::Public
        );
        assert_eq!(DataClass::compose_all([]), DataClass::Credential);
    }

    #[test]
    fn the_task_policy_ceiling_is_a_ceiling() {
        // Task Protocol §2.1, T6.
        assert!(!RiskClass::Observe.exceeds(RiskClass::Observe));
        assert!(!RiskClass::LocalState.exceeds(RiskClass::ExternalWrite));
        assert!(RiskClass::Communication.exceeds(RiskClass::Observe));
        assert!(RiskClass::Credential.exceeds(RiskClass::Destructive));
    }

    #[test]
    fn blocked_is_not_terminal_and_the_terminal_states_have_no_exits() {
        // Task Protocol §4.1, T8.
        assert!(!TaskState::Blocked.is_terminal());
        for state in [
            TaskState::Received,
            TaskState::Planning,
            TaskState::Ready,
            TaskState::Executing,
            TaskState::WaitingApproval,
            TaskState::WaitingUser,
            TaskState::Verifying,
            TaskState::Blocked,
        ] {
            assert!(!state.is_terminal(), "{state:?} is not terminal");
        }
        for state in [
            TaskState::Completed,
            TaskState::Failed,
            TaskState::Cancelled,
        ] {
            assert!(state.is_terminal(), "{state:?} is terminal");
        }
    }
}

mod unknown_enum_variants_fail_closed {
    use super::*;

    #[test]
    fn a_deserialiser_refuses_a_variant_outside_every_frozen_set() {
        let cases: [(&str, &str); 8] = [
            ("OBSERVE", r#""NOT_A_CLASS""#),
            ("LOCAL_STATE", r#""SUPER_WRITE""#),
            ("AMBIGUOUS", r#""AMBIGUOUS_EFFECT""#),
            ("GOAL_RESULT", r#""PROVIDER_EVIDENCE""#),
            ("PLANNING", r#""PLAN""#),
            ("RECEIVED", r#""PENDING""#),
            ("CAPABILITY", r#""TOOL""#),
            ("PUBLIC", r#""PUBLIC\"""#),
        ];
        for (known, unknown) in cases {
            let document = format!("\"{unknown}\"");
            let parsed = match known {
                "OBSERVE" => serde_json::from_str::<SideEffectClass>(&document).is_ok(),
                "LOCAL_STATE" => serde_json::from_str::<ReplaySafety>(&document).is_ok(),
                "AMBIGUOUS" => serde_json::from_str::<ActionErrorKind>(&document).is_ok(),
                "GOAL_RESULT" => serde_json::from_str::<EvidenceKind>(&document).is_ok(),
                "PLANNING" => serde_json::from_str::<ModelPurpose>(&document).is_ok(),
                "RECEIVED" => serde_json::from_str::<TaskState>(&document).is_ok(),
                "CAPABILITY" => serde_json::from_str::<StepKind>(&document).is_ok(),
                _ => serde_json::from_str::<DataClass>(&document).is_ok(),
            };
            assert!(!parsed, "{unknown} must not parse as a frozen variant");
        }
    }

    #[test]
    fn action_status_and_action_result_status_use_the_frozen_set() {
        assert_eq!(
            ActionStatus::WIRE_NAMES,
            [
                "SUCCEEDED",
                "FAILED",
                "REJECTED",
                "CANCELLED",
                "UNAVAILABLE",
                "DUPLICATE_SUPPRESSED"
            ]
        );
        assert!(!ActionStatus::WIRE_NAMES.contains(&"TIMEOUT"));
        assert!(!ActionStatus::WIRE_NAMES.contains(&"SKIPPED"));
    }
}

// ---------------------------------------------------------------------------
// Round trips
// ---------------------------------------------------------------------------

mod action_request_round_trip {
    use super::*;

    /// The Capability Protocol Section 4 example, verbatim.
    fn frozen_example() -> Value {
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

    fn parsed() -> ActionRequest {
        serde_json::from_value(frozen_example()).expect("the frozen example must parse")
    }

    #[test]
    fn the_frozen_example_round_trips_exactly() {
        let value = frozen_example();
        let request: ActionRequest = serde_json::from_value(value.clone()).expect("parses");
        assert_eq!(serde_json::to_value(&request).expect("serialises"), value);
        assert_eq!(
            request.capability_id,
            CapabilityId::new("calendar.events.list").expect("valid")
        );
        assert_eq!(request.requested_by, RequestedBy::Model);
        assert_eq!(request.data_class, DataClass::Personal);
        assert_eq!(request.arguments_digest.as_str(), DIGEST);
    }

    #[test]
    fn the_surface_is_closed_so_a_host_resolved_field_cannot_be_smuggled_in() {
        // Capability Protocol Section 4.2 lists the host-resolved fields. The
        // type has no such field at all, and the schema and the deserialiser
        // both refuse one that is supplied.
        let mut with_risk = frozen_example();
        with_risk["risk_class"] = json!("EXTERNAL_WRITE");
        assert!(serde_json::from_value::<ActionRequest>(with_risk.clone()).is_err());

        let mut with_provider = frozen_example();
        with_provider["provider_id"] = json!("calendar");
        assert!(serde_json::from_value::<ActionRequest>(with_provider).is_err());

        for field in [
            "side_effect_class",
            "required_authorization",
            "authorization",
            "grant_id",
            "approval_id",
            "codex_allowed",
            "arguments_preview",
        ] {
            let mut polluted = frozen_example();
            polluted[field] = json!("anything");
            assert!(
                serde_json::from_value::<ActionRequest>(polluted).is_err(),
                "{field} must not be accepted"
            );
        }
    }

    #[test]
    fn the_parsed_request_is_the_one_the_later_tests_assume() {
        assert_eq!(parsed().capability_id.as_str(), "calendar.events.list");
    }

    #[test]
    fn a_missing_required_field_is_refused() {
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
            let mut incomplete = frozen_example();
            incomplete.as_object_mut().expect("object").remove(field);
            assert!(
                serde_json::from_value::<ActionRequest>(incomplete).is_err(),
                "{field} is required"
            );
        }
    }

    #[test]
    fn wrong_field_types_are_refused_rather_than_coerced() {
        let mut wrong = frozen_example();
        wrong["deadline_ms"] = json!("15000");
        assert!(serde_json::from_value::<ActionRequest>(wrong).is_err());

        let mut wrong = frozen_example();
        wrong["arguments"] = json!(["range"]);
        assert!(serde_json::from_value::<ActionRequest>(wrong).is_err());

        let mut wrong = frozen_example();
        wrong["capability_version"] = json!(1);
        assert!(serde_json::from_value::<ActionRequest>(wrong).is_err());

        let mut wrong = frozen_example();
        wrong["task_id"] = json!(null);
        assert!(serde_json::from_value::<ActionRequest>(wrong).is_err());
    }

    #[test]
    fn a_wrong_identifier_prefix_is_refused_on_the_wire() {
        for (field, bad) in [
            ("request_id", STEP_ID),
            ("task_id", EVENT_ID),
            ("step_id", TASK_ID),
        ] {
            let mut wrong = frozen_example();
            wrong[field] = json!(bad);
            assert!(
                serde_json::from_value::<ActionRequest>(wrong).is_err(),
                "{field} must not accept the {bad} domain"
            );
        }
        let mut wrong = frozen_example();
        wrong["idempotency_key"] = json!(DIGEST);
        assert!(serde_json::from_value::<ActionRequest>(wrong).is_err());
        let mut wrong = frozen_example();
        wrong["arguments_digest"] = json!(IDEMPOTENCY_KEY);
        assert!(serde_json::from_value::<ActionRequest>(wrong).is_err());
    }

    #[test]
    fn the_error_message_names_the_domain_without_echoing_the_value() {
        let secret_shaped = "tsk_sk-proj-4a7f9c2b1d-and-more-0123456789AB";
        let wrong =
            serde_json::from_value::<TaskId>(json!(secret_shaped)).expect_err("must be rejected");
        let rendered = wrong.to_string();
        assert!(rendered.contains("TaskId"), "{rendered}");
        assert!(
            !rendered.contains("sk-proj"),
            "the rejected value must not be echoed into an error: {rendered}"
        );
    }
}

mod action_result_round_trip {
    use super::*;

    fn succeeded() -> Value {
        json!({
            "request_id": REQUEST_ID,
            "status": "SUCCEEDED",
            "output": { "events": [] },
            "output_digest": DIGEST,
            "evidence": [ {
                "evidence_id": EVENT_ID,
                "kind": "CAPABILITY_OBSERVATION",
                "capability_id": "calendar.events.list",
                "task_id": TASK_ID,
                "step_id": STEP_ID,
                "attempt": 1,
                "produced_at": "2026-10-01T09:14:23.902Z",
                "actor": { "kind": "PROVIDER", "id": "synthetic-provider", "version": "1.0.0" },
                "data_class": "PERSONAL",
                "payload_digest": DIGEST,
                "payload_reference": null
            } ],
            "receipt": null,
            "error": null,
            "duration_ms": 412
        })
    }

    #[test]
    fn the_frozen_shape_round_trips_exactly() {
        // `request_id()` and `event_id()` are the fixture helpers the frozen
        // example reuses; asserting them keeps the constants honest.
        assert_eq!(request_id().as_str(), REQUEST_ID);
        assert_eq!(event_id().as_str(), EVENT_ID);
        assert_eq!(actor().kind, ActorKind::Host);
        let value = succeeded();
        let result: ActionResult = serde_json::from_value(value.clone()).expect("parses");
        assert_eq!(serde_json::to_value(&result).expect("serialises"), value);
        assert_eq!(result.status, ActionStatus::Succeeded);
        assert_eq!(result.evidence.len(), 1);
        assert_eq!(result.evidence[0].kind, EvidenceKind::CapabilityObservation);
        assert!(result.receipt.is_none());
    }

    #[test]
    fn a_receipt_carries_the_proof_of_an_external_effect() {
        let with_receipt = json!({
            "request_id": REQUEST_ID,
            "status": "SUCCEEDED",
            "output": { "event_ref": "evt_01JQ8ZK5H4NQW9T2XR7BV3M8DF" },
            "output_digest": DIGEST,
            "evidence": [],
            "receipt": {
                "receipt_id": RECEIPT_ID,
                "capability_id": "calendar.events.create",
                "idempotency_key": IDEMPOTENCY_KEY,
                "provider_reference": "provider-ref-0001",
                "effect_summary": "Created event on the primary calendar",
                "observed_at": "2026-10-01T09:14:23.880Z",
                "replay_safe": false
            },
            "error": null,
            "duration_ms": 412
        });
        let result: ActionResult = serde_json::from_value(with_receipt).expect("parses");
        let receipt = result.receipt.as_ref().expect("receipt present");
        assert_eq!(receipt.capability_id.as_str(), "calendar.events.create");
        assert!(!receipt.replay_safe);
        assert_eq!(
            receipt.provider_reference.as_ref().map(|r| r.as_str()),
            Some("provider-ref-0001")
        );
    }

    #[test]
    fn an_ambiguous_error_is_carried_without_authorising_a_retry() {
        // Capability Protocol Section 6.1: AMBIGUOUS is the most consequential
        // kind; its `retryable` flag never authorises a blind re-issue.
        let value = json!({
            "request_id": REQUEST_ID,
            "status": "FAILED",
            "receipt": null,
            "error": {
                "kind": "AMBIGUOUS",
                "code": "PROVIDER_TIMEOUT",
                "message": "Connection dropped after the write was issued",
                "retryable": false,
                "host_action": "RECONCILE",
                "details": {}
            },
            "duration_ms": 30000
        });
        let result: ActionResult = serde_json::from_value(value).expect("parses");
        let error = result.error.as_ref().expect("error present");
        assert_eq!(error.kind, ActionErrorKind::Ambiguous);
        assert!(!error.retryable);
    }

    #[test]
    fn the_closed_surface_refuses_an_undeclared_field() {
        let mut polluted = succeeded();
        polluted["side_effect_class"] = json!("NONE");
        assert!(serde_json::from_value::<ActionResult>(polluted).is_err());

        let mut polluted = succeeded();
        polluted["risk_class"] = json!("OBSERVE");
        assert!(serde_json::from_value::<ActionResult>(polluted).is_err());
    }

    #[test]
    fn every_frozen_error_kind_round_trips() {
        for kind in ActionErrorKind::WIRE_NAMES {
            let value = json!({
                "kind": kind,
                "code": "SYNTHETIC_CODE",
                "message": "synthetic failure",
                "retryable": false,
                "host_action": "NONE",
                "details": {}
            });
            let parsed: ActionError = serde_json::from_value(value.clone()).expect("parses");
            assert_eq!(parsed.kind.wire_name(), *kind);
            assert_eq!(serde_json::to_value(&parsed).expect("serialises"), value);
        }
    }
}

mod assistant_task_round_trip {
    use super::*;

    /// The Task Protocol Section 2 example, verbatim.
    fn frozen_example() -> Value {
        json!({
            "task_id": TASK_ID,
            "kind": "USER_REQUEST",
            "title": "Summarize today's mail",
            "state": "EXECUTING",
            "origin": {
                "kind": "USER_MESSAGE",
                "device_id": DEVICE_ID,
                "message_id": EVENT_ID
            },
            "data_class": "PERSONAL",
            "policy_class": "OBSERVE",
            "created_at": "2026-10-01T09:14:20.001Z",
            "updated_at": "2026-10-01T09:14:23.902Z",
            "deadline_at": null,
            "attempt_budget": {
                "max_model_calls": 12,
                "max_tool_calls": 24,
                "max_attempts_per_step": 3
            },
            "steps": [],
            "blocked_reason": null,
            "result_summary": null,
            "cancelled_at": null,
            "cancelled_by": null,
            "failure_reason": null
        })
    }

    #[test]
    fn the_frozen_example_round_trips_exactly() {
        let value = frozen_example();
        let task: AssistantTask = serde_json::from_value(value.clone()).expect("parses");
        assert_eq!(serde_json::to_value(&task).expect("serialises"), value);
        assert_eq!(task.policy_class, RiskClass::Observe);
        assert_eq!(task.kind, TaskKind::UserRequest);
        assert_eq!(task.state, TaskState::Executing);
    }

    #[test]
    fn pro_task_2_1_the_policy_ceiling_is_a_declared_field_and_not_a_wire_control() {
        // T6 / INV-SEC-03: `policy_class` is a field the host *assigned*. Reading
        // one off the wire — from durable state or from a device — yields an
        // untrusted value, and this test says so plainly rather than implying
        // deserialisation enforces the ceiling. It does not: enforcement is the
        // policy engine's job, in P6. What P1 guarantees is the weaker and
        // actually provable thing — the field is a plain declared member of the
        // task, not an authority a consumer can invoke.
        let mut widened = frozen_example();
        widened["policy_class"] = json!("DESTRUCTIVE");
        let task: AssistantTask = serde_json::from_value(widened).expect("parses");
        assert_eq!(
            task.policy_class,
            RiskClass::Destructive,
            "the field itself parses; the host enforces the ceiling in policy, which is P6"
        );
    }

    #[test]
    fn an_unknown_field_on_the_shared_task_surface_is_preserved_not_dropped() {
        let mut extended = frozen_example();
        extended["future_minor_field"] = json!({ "nested": [1, 2, 3] });
        let task: AssistantTask = serde_json::from_value(extended).expect("parses");
        assert_eq!(
            task.extensions.get("future_minor_field"),
            Some(&json!({ "nested": [1, 2, 3] })),
            "an architecture-minor field must round-trip, not vanish"
        );
        let reserialised = serde_json::to_value(&task).expect("serialises");
        assert_eq!(
            reserialised["future_minor_field"],
            json!({ "nested": [1, 2, 3] })
        );
    }

    /// `plan_revision` is named in Task Protocol §4.3 rule 5. It is a real
    /// future field, so an architecture-minor addition must round-trip rather
    /// than break a reader.
    fn step() -> TaskStep {
        TaskStep {
            step_id: step_id(),
            task_id: task_id(),
            sequence: 3,
            kind: StepKind::Capability,
            status: StepStatus::new("SUCCEEDED").expect("valid"),
            attempt: 1,
            idempotency_key: idempotency_key(),
            provider_id: Some(ProviderId::new("calendar").expect("valid")),
            capability_id: Some(CapabilityId::new("calendar.events.list").expect("valid")),
            capability_version: Some(SemVer::new("1.2.0").expect("valid")),
            input_digest: digest(),
            result_digest: digest(),
            side_effect_receipt: None,
            started_at: timestamp("2026-10-01T09:14:22.100Z"),
            completed_at: timestamp("2026-10-01T09:14:22.512Z"),
            lease_owner: None,
            lease_expires_at: None,
            error: None,
            extensions: Default::default(),
        }
    }

    #[test]
    fn a_task_step_round_trips_exactly() {
        let step = step();
        let value = serde_json::to_value(&step).expect("serialises");
        assert_eq!(
            serde_json::from_value::<TaskStep>(value.clone()).expect("parses"),
            step
        );
    }

    #[test]
    fn an_unknown_field_on_a_step_is_preserved_not_dropped() {
        // Task Protocol §3's field list is exhaustive but does not require a
        // closed schema, and Protocol Index §4.1 makes a new optional field an
        // architecture-minor change. A step must round-trip as losslessly as
        // the task that holds it.
        let mut value = serde_json::to_value(step()).expect("serialises");
        value["plan_revision"] = json!(3);
        let parsed: TaskStep = serde_json::from_value(value).expect("parses");
        assert_eq!(parsed.extensions.get("plan_revision"), Some(&json!(3)));
        assert_eq!(
            serde_json::to_value(&parsed).expect("serialises")["plan_revision"],
            json!(3)
        );
    }
}

mod event_round_trip {
    use super::*;

    /// The Event Protocol Section 2 example, verbatim.
    fn frozen_example() -> Value {
        json!({
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
        })
    }

    #[test]
    fn the_frozen_example_round_trips_exactly() {
        let value = frozen_example();
        let event: SereaEvent = serde_json::from_value(value.clone()).expect("parses");
        assert_eq!(serde_json::to_value(&event).expect("serialises"), value);
        assert_eq!(event.seq, Seq::new(10_427));
        assert_eq!(event.kind, EventKind::CapabilityCompleted);
        assert_eq!(event.actor.kind, ActorKind::Host);
        assert_eq!(event.payload["status"], json!("SUCCEEDED"));
    }

    #[test]
    fn an_unknown_field_on_the_shared_event_surface_is_preserved() {
        let mut extended = frozen_example();
        extended["producer_note"] = json!("synthetic");
        let event: SereaEvent = serde_json::from_value(extended).expect("parses");
        assert_eq!(
            event.extensions.get("producer_note"),
            Some(&json!("synthetic"))
        );
        assert_eq!(
            serde_json::to_value(&event).expect("serialises")["producer_note"],
            json!("synthetic")
        );
    }

    #[test]
    fn every_frozen_event_kind_parses_and_round_trips() {
        for kind in EventKind::WIRE_NAMES {
            let mut value = frozen_example();
            value["kind"] = json!(kind);
            let event: SereaEvent = serde_json::from_value(value.clone())
                .unwrap_or_else(|error| panic!("{kind} must parse: {error}"));
            assert_eq!(event.kind.wire_name(), *kind);
            assert_eq!(serde_json::to_value(&event).expect("serialises"), value);
        }
    }

    #[test]
    fn an_unregistered_event_kind_fails_closed() {
        // Protocol Index Section 4.2 rule 3: unknown enum variants fail closed
        // everywhere. Skipping an unknown kind is a recipient rendering
        // behaviour (Event Protocol Section 6 rule 2), not a host parse.
        let mut value = frozen_example();
        value["kind"] = json!("CAPABILITY_ALMOST_COMPLETED");
        assert!(serde_json::from_value::<SereaEvent>(value).is_err());

        let mut value = frozen_example();
        value["kind"] = json!("DELETION_CASCADE_COMPLETED");
        assert!(
            serde_json::from_value::<SereaEvent>(value).is_err(),
            "a name used in Data Classification Section 8.2 but absent from the frozen \
             Event Protocol Section 3 table must not be admitted"
        );
    }

    #[test]
    fn a_model_actor_records_involvement_not_authority() {
        let mut value = frozen_example();
        value["actor"] =
            json!({ "kind": "MODEL", "id": "nemotron-3-nano-30b", "version": "1.0.0" });
        let event: SereaEvent = serde_json::from_value(value).expect("parses");
        assert_eq!(event.actor.kind, ActorKind::Model);
        // There is no authority-bearing field on an event at all.
        let rendered = serde_json::to_value(&event).expect("serialises");
        assert!(rendered.get("authority").is_none());
        assert!(rendered.get("approved").is_none());
    }
}

mod envelope_round_trip {
    use super::*;

    /// The Protocol Index Section 6 example, verbatim.
    fn frozen_example() -> Value {
        json!({
            "envelope_version": "1",
            "surface": "serea.action/1",
            "message_id": EVENT_ID,
            "correlation_id": TASK_ID,
            "causation_id": "evt_01JQ8Z9M4SBDT6K8H2WNRQVPXF",
            "issued_at": "2026-10-01T09:14:22.418Z",
            "data_class": "PERSONAL",
            "trace": { "task_id": TASK_ID, "step_id": STEP_ID },
            "payload": {}
        })
    }

    #[test]
    fn the_frozen_example_round_trips_exactly() {
        let value = frozen_example();
        let envelope: Envelope<Value> = serde_json::from_value(value.clone()).expect("parses");
        assert_eq!(serde_json::to_value(&envelope).expect("serialises"), value);
        assert_eq!(envelope.surface.as_str(), "serea.action/1");
        assert_eq!(envelope.data_class, DataClass::Personal);
    }

    #[test]
    fn causation_is_absent_only_for_user_originated_messages() {
        let mut without = frozen_example();
        without
            .as_object_mut()
            .expect("object")
            .remove("causation_id");
        let envelope: Envelope<Value> = serde_json::from_value(without).expect("parses");
        assert!(envelope.causation_id.is_none());
        let reserialised = serde_json::to_value(&envelope).expect("serialises");
        assert_eq!(reserialised["causation_id"], Value::Null);
    }

    #[test]
    fn unknown_fields_on_a_wire_surface_are_preserved_and_reinterpreted_nothing() {
        let mut extended = frozen_example();
        extended["future_extension"] = json!([1, 2, 3]);
        let envelope: Envelope<Value> = serde_json::from_value(extended).expect("parses");
        assert_eq!(
            envelope.extensions.get("future_extension"),
            Some(&json!([1, 2, 3]))
        );
        assert_eq!(
            serde_json::to_value(&envelope).expect("serialises")["future_extension"],
            json!([1, 2, 3])
        );
    }

    #[test]
    fn a_missing_required_envelope_field_is_refused() {
        for field in [
            "envelope_version",
            "surface",
            "message_id",
            "issued_at",
            "data_class",
            "payload",
        ] {
            let mut incomplete = frozen_example();
            incomplete.as_object_mut().expect("object").remove(field);
            assert!(
                serde_json::from_value::<Envelope<Value>>(incomplete).is_err(),
                "{field} is required"
            );
        }
    }

    #[test]
    fn a_declared_higher_class_than_the_consumer_computes_is_still_carried() {
        // Data Classification Section 9, DC9 is a consumer obligation; the
        // envelope carries the producer's declaration verbatim either way.
        for declared in ["PUBLIC", "PERSONAL", "PRIVATE", "SECRET", "CREDENTIAL"] {
            let mut value = frozen_example();
            value["data_class"] = json!(declared);
            let envelope: Envelope<Value> = serde_json::from_value(value).expect("parses");
            assert_eq!(envelope.data_class.wire_name(), declared);
        }
    }

    fn trace_value() -> Value {
        json!({ "task_id": TASK_ID, "step_id": STEP_ID, "attempt": 1 })
    }

    #[test]
    fn trace_context_is_optional_typed_and_forward_compatible() {
        let trace = Trace {
            task_id: Some(task_id()),
            step_id: Some(step_id()),
            attempt: Some(1),
            extensions: Default::default(),
        };
        let value = serde_json::to_value(&trace).expect("serialises");
        assert_eq!(value, trace_value());
        assert_eq!(
            trace_value(),
            json!({ "task_id": TASK_ID, "step_id": STEP_ID, "attempt": 1 })
        );
        assert_eq!(
            serde_json::from_value::<Trace>(value).expect("parses"),
            trace
        );
        assert_eq!(
            serde_json::to_value(Trace::default()).expect("serialises"),
            json!({})
        );
    }
}

mod model_round_trip {
    use super::*;

    fn request() -> Value {
        json!({
            "request_id": REQUEST_ID,
            "model_id": "nemotron-3-nano-30b",
            "task_id": TASK_ID,
            "purpose": "PLANNING",
            "messages": [ { "role": "user", "content": "synthetic prompt" } ],
            "system": "synthetic system prompt",
            "response_format": { "type": "TEXT" },
            "tools": [],
            "max_output_tokens": 2048,
            "temperature": 0.2,
            "deadline_ms": 30000,
            "data_class": "PERSONAL"
        })
    }

    #[test]
    fn a_model_request_round_trips_exactly() {
        let value = request();
        let parsed: ModelRequest = serde_json::from_value(value.clone()).expect("parses");
        assert_eq!(serde_json::to_value(&parsed).expect("serialises"), value);
        assert_eq!(parsed.purpose, ModelPurpose::Planning);
        assert_eq!(parsed.messages[0].role.as_str(), "user");
        assert!(matches!(parsed.response_format, ResponseFormat::Text));
    }

    #[test]
    fn a_json_schema_response_format_carries_the_host_schema() {
        let mut value = request();
        value["response_format"] = json!({
            "type": "JSON_SCHEMA",
            "schema": {
                "type": "object",
                "additionalProperties": false,
                "required": ["actions", "complete"],
                "properties": {
                    "actions": { "type": "array", "maxItems": 8 },
                    "complete": { "type": "boolean" }
                }
            }
        });
        let parsed: ModelRequest = serde_json::from_value(value).expect("parses");
        match &parsed.response_format {
            ResponseFormat::JsonSchema { schema } => {
                assert_eq!(schema["required"], json!(["actions", "complete"]))
            }
            ResponseFormat::Text => panic!("expected a JSON_SCHEMA response format"),
        }
        // Model Protocol Section 3.1: "There is no third option."
        assert!(serde_json::from_str::<ResponseFormat>(r#"{"type":"JSON"}"#).is_err());
        assert!(serde_json::from_str::<ResponseFormat>(r#"{"type":"PROSE"}"#).is_err());
        assert!(serde_json::from_str::<ResponseFormat>(r#"{"type":"TEXT","schema":{}}"#).is_err());
    }

    #[test]
    fn a_response_round_trips_and_keeps_structured_output_as_a_proposal() {
        let value = json!({
            "request_id": REQUEST_ID,
            "model_id": "nemotron-3-nano-30b",
            "provider_id": "ollama",
            "content": "synthetic",
            "structured": { "kind": "ACTION_PLAN", "actions": [] },
            "finish_reason": "STOP",
            "usage": { "input_tokens": "1840", "output_tokens": "260", "cost_class": "FREE" },
            "latency_ms": 2410,
            "repair_attempts": 0
        });
        let parsed: ModelResponse = serde_json::from_value(value.clone()).expect("parses");
        assert_eq!(serde_json::to_value(&parsed).expect("serialises"), value);
        assert_eq!(parsed.usage.input_tokens, TokenCount::new(1_840));
        assert_eq!(parsed.finish_reason, FinishReason::Stop);
    }

    #[test]
    fn malformed_structured_output_is_carried_as_data_for_the_host_to_refuse() {
        // Model Protocol Section 4.1 and Section 7.3: invalid structured output
        // is a routine, expected failure that fails the step; the protocol layer
        // never guesses at it.
        let value = json!({
            "request_id": REQUEST_ID,
            "model_id": "nemotron-3-nano-30b",
            "provider_id": "ollama",
            "content": "synthetic",
            "structured": { "kind": "ACTION_PLAN" },
            "finish_reason": "STRUCTURE_INVALID",
            "usage": { "input_tokens": "1840", "output_tokens": "0", "cost_class": "FREE" },
            "latency_ms": 2410,
            "repair_attempts": 2
        });
        let parsed: ModelResponse = serde_json::from_value(value).expect("parses");
        assert_eq!(parsed.finish_reason, FinishReason::StructureInvalid);
        assert_eq!(parsed.repair_attempts, 2);
    }

    #[test]
    fn a_model_error_is_not_an_action_error() {
        let value = json!({
            "kind": "PROVIDER_UNAVAILABLE",
            "message": "synthetic provider outage",
            "retryable": true
        });
        let parsed: ModelError = serde_json::from_value(value.clone()).expect("parses");
        assert_eq!(serde_json::to_value(&parsed).expect("serialises"), value);
        assert!(parsed.retryable);
        for kind in ActionErrorKind::WIRE_NAMES {
            assert_ne!(*kind, "PROVIDER_UNAVAILABLE");
        }
    }

    #[test]
    fn model_capabilities_are_data_not_branches() {
        let capabilities = ModelCapabilities {
            vision: false,
            tools: true,
            structured_output: true,
            json_schema_mode: JsonSchemaMode::Strict,
            thinking: false,
            long_context: false,
            fast: true,
            code_specialist: false,
            max_context_tokens: 131_072,
            max_output_tokens: 8_192,
            supports_streaming: true,
            supports_seeds: false,
        };
        let descriptor = ModelDescriptor {
            model_id: ModelId::new("nemotron-3-nano-30b").expect("valid"),
            provider_id: ProviderId::new("ollama").expect("valid"),
            capabilities,
        };
        let value = serde_json::to_value(&descriptor).expect("serialises");
        assert_eq!(value["capabilities"]["json_schema_mode"], json!("STRICT"));
        assert_eq!(
            serde_json::from_value::<ModelDescriptor>(value).expect("parses"),
            descriptor
        );
    }

    #[test]
    fn usage_counts_reject_a_json_number() {
        let value = json!({
            "input_tokens": 1840,
            "output_tokens": 260,
            "cost_class": "FREE"
        });
        assert!(serde_json::from_value::<ModelUsage>(value).is_err());
    }

    #[test]
    fn a_model_message_is_a_role_and_content_only() {
        let message = ModelMessage {
            role: MessageRole::new("assistant").expect("valid"),
            content: "synthetic".to_owned(),
        };
        let mut value = serde_json::to_value(&message)
            .expect("serialises")
            .as_object()
            .cloned()
            .expect("object");
        value.insert("authority".to_owned(), json!("ADMIN"));
        assert!(serde_json::from_value::<ModelMessage>(Value::Object(value)).is_err());
    }
}

mod envelope_version_support {
    use super::*;

    fn envelope(version: &str, surface: &str) -> Envelope<Value> {
        Envelope {
            envelope_version: EnvelopeVersion::new(version).expect("valid major"),
            surface: WireSurface::new(surface).expect("valid surface"),
            message_id: event_id(),
            correlation_id: None,
            causation_id: None,
            issued_at: timestamp("2026-10-01T09:14:22.418Z"),
            data_class: DataClass::Personal,
            trace: None,
            payload: Value::Object(serde_json::Map::new()),
            extensions: Default::default(),
        }
    }

    #[test]
    fn pro_index_4_2_an_unimplemented_major_is_refused_at_both_axes() {
        // Protocol Index §4.2 rule 1: "A consumer must reject a payload whose
        // major wire-protocol version it does not implement. Silent downgrade is
        // forbidden." The two axes are independent: the envelope major and the
        // surface major.
        assert_eq!(EnvelopeVersion::SUPPORTED_MAJOR, 1);
        assert!(EnvelopeVersion::new("1").expect("valid").is_supported());

        // A leading zero is not a major at all, on either axis.
        assert_eq!(
            EnvelopeVersion::new("0"),
            Err(ProtocolError::MalformedValue {
                field: serea_protocol::ValueField::EnvelopeVersion,
                reason: serea_protocol::ValueRejection::Malformed,
            })
        );
        assert!(WireSurface::new("serea.action/0").is_err());

        for major in ["2", "42", "4294967295"] {
            let version = EnvelopeVersion::new(major).expect("a decimal major parses");
            assert!(
                !version.is_supported(),
                "major {major} is not implemented here"
            );
            assert_eq!(
                envelope(major, "serea.action/1").require_supported_major(),
                Err(ProtocolError::ContractViolation {
                    rule: ContractRule::UnsupportedEnvelopeMajor,
                })
            );
        }
        for major in ["2", "7", "4294967295"] {
            assert_eq!(
                envelope("1", &format!("serea.action/{major}")).require_supported_surface(),
                Err(ProtocolError::ContractViolation {
                    rule: ContractRule::UnsupportedWireSurfaceMajor,
                }),
                "surface major {major} is not implemented here"
            );
        }

        let supported = envelope("1", "serea.action/1");
        assert_eq!(supported.require_supported_major(), Ok(()));
        assert_eq!(supported.require_supported_surface(), Ok(()));
        assert_eq!(supported.surface.major(), Some(1));
        assert_eq!(supported.surface.name(), "serea.action");
    }

    #[test]
    fn a_surface_major_too_large_to_be_a_number_is_treated_as_unsupported() {
        // Fail closed rather than silently reporting a major of zero.
        let surface = WireSurface::new("serea.action/99999999999999").expect("digits are in range");
        assert_eq!(surface.major(), None);
        let mut envelope = envelope("1", "serea.action/1");
        envelope.surface = surface;
        assert_eq!(
            envelope.require_supported_surface(),
            Err(ProtocolError::ContractViolation {
                rule: ContractRule::UnsupportedWireSurfaceMajor,
            })
        );
    }
}

// ---------------------------------------------------------------------------
// Capability descriptors and credential handles
// ---------------------------------------------------------------------------

mod capability_descriptor {
    use super::*;

    fn draft(
        provider: &str,
        data_class: DataClass,
        risk_class: RiskClass,
    ) -> CapabilityDescriptorDraft {
        CapabilityDescriptorDraft {
            id: CapabilityId::new(format!("{provider}.events.list")).expect("valid capability id"),
            version: SemVer::new("1.2.0").expect("valid"),
            title: DescriptorTitle::new("List calendar events").expect("valid"),
            description: DescriptorDescription::new(
                "Lists events in a time range from a selected calendar.",
            )
            .expect("valid"),
            provider_id: ProviderId::new(provider).expect("valid"),
            implementation_id: None,
            input_schema: JsonSchemaRef::new(
                "https://serea.local/schemas/calendar.events.list.input.1.2.0.json",
            )
            .expect("valid"),
            output_schema: JsonSchemaRef::new(
                "https://serea.local/schemas/calendar.events.list.output.1.2.0.json",
            )
            .expect("valid"),
            side_effect_class: SideEffectClass::None,
            risk_class,
            required_authorization: Authorization::None,
            replay_safety: ReplaySafety::Idempotent,
            data_class,
            root_requirement: RootRequirement::NotRequired,
            idempotency_support: IdempotencySupport::Native,
            max_duration_ms: 15_000,
            cost_class: CostClass::Free,
            experimental: false,
        }
    }

    #[test]
    fn a_consistent_descriptor_is_constructible() {
        let descriptor =
            CapabilityDescriptor::new(draft("calendar", DataClass::Personal, RiskClass::Observe))
                .expect("consistent");
        assert_eq!(descriptor.id().as_str(), "calendar.events.list");
        assert_eq!(descriptor.provider_id().as_str(), "calendar");
        assert_eq!(descriptor.risk_class(), RiskClass::Observe);
        assert!(!descriptor.experimental());
        assert!(descriptor.implementation_id().is_none());
    }

    #[test]
    fn a_namespace_mismatch_is_refused_at_registration() {
        let mut inconsistent = draft("calendar", DataClass::Personal, RiskClass::Observe);
        inconsistent.provider_id = ProviderId::new("gmail").expect("valid");
        assert_eq!(
            CapabilityDescriptor::new(inconsistent),
            Err(ProtocolError::ContractViolation {
                rule: ContractRule::CapabilityProviderNamespaceMismatch,
            })
        );
    }

    #[test]
    fn a_credential_class_capability_must_carry_the_credential_risk_class() {
        assert!(
            CapabilityDescriptor::new(draft(
                "synthetic",
                DataClass::Credential,
                RiskClass::Credential
            ))
            .is_ok()
        );
        assert_eq!(
            CapabilityDescriptor::new(draft(
                "synthetic",
                DataClass::Credential,
                RiskClass::Observe
            )),
            Err(ProtocolError::ContractViolation {
                rule: ContractRule::CapabilityCredentialClassContradiction,
            })
        );
    }

    #[test]
    fn deserialisation_cannot_bypass_either_registration_invariant() {
        // Regression: a derived `Deserialize` wrote the private fields
        // directly, so a descriptor that `new` refuses would parse. Deserialising
        // through the same gate is the only way the documented guarantee holds on
        // the wire path.
        let good =
            CapabilityDescriptor::new(draft("calendar", DataClass::Personal, RiskClass::Observe))
                .expect("consistent");
        let value = serde_json::to_value(&good).expect("serialises");

        let mut mismatched = value.clone();
        mismatched["provider_id"] = json!("gmail");
        let error = serde_json::from_value::<CapabilityDescriptor>(mismatched)
            .expect_err("a namespace mismatch must not deserialize");
        assert!(
            error.to_string().contains("provider segment"),
            "the typed reason must survive the serde boundary: {error}"
        );

        let mut contradicted = value;
        contradicted["data_class"] = json!("CREDENTIAL");
        let error = serde_json::from_value::<CapabilityDescriptor>(contradicted)
            .expect_err("a credential-class contradiction must not deserialize");
        assert!(
            error.to_string().contains("risk_class CREDENTIAL"),
            "the typed reason must survive the serde boundary: {error}"
        );
    }

    #[test]
    fn a_descriptor_survives_the_draft_round_trip() {
        let good =
            CapabilityDescriptor::new(draft("calendar", DataClass::Personal, RiskClass::Observe))
                .expect("consistent");
        let value = serde_json::to_value(&good).expect("serialises");
        let parsed: CapabilityDescriptorDraft = serde_json::from_value(value).expect("parses");
        assert_eq!(
            CapabilityDescriptor::new(parsed.clone()).expect("consistent"),
            good
        );
        assert_eq!(
            CapabilityDescriptorDraft::from(good.clone()),
            parsed,
            "the draft is a lossless view of the descriptor"
        );
    }

    #[test]
    fn an_unexpected_descriptor_field_is_refused() {
        let descriptor =
            CapabilityDescriptor::new(draft("calendar", DataClass::Personal, RiskClass::Observe))
                .expect("consistent");
        let mut value = serde_json::to_value(&descriptor).expect("serialises");
        value["risk_class_was_here"] = json!(true);
        assert!(serde_json::from_value::<CapabilityDescriptor>(value).is_err());
    }

    #[test]
    fn the_frozen_calendar_descriptor_round_trips() {
        let descriptor =
            CapabilityDescriptor::new(draft("calendar", DataClass::Personal, RiskClass::Observe))
                .expect("consistent");
        let value = serde_json::to_value(&descriptor).expect("serialises");
        assert_eq!(value["version"], json!("1.2.0"));
        assert_eq!(value["side_effect_class"], json!("NONE"));
        assert_eq!(value["required_authorization"], json!("NONE"));
        assert_eq!(value["replay_safety"], json!("IDEMPOTENT"));
        assert_eq!(value["root_requirement"], json!("NOT_REQUIRED"));
        assert_eq!(value["idempotency_support"], json!("NATIVE"));
        assert_eq!(value["cost_class"], json!("FREE"));
        assert_eq!(value["experimental"], json!(false));
        assert_eq!(
            serde_json::from_value::<CapabilityDescriptor>(value).expect("parses"),
            descriptor
        );
    }
}

mod credential_handles {
    use super::*;

    #[test]
    fn a_handle_reuses_the_digest_wire_form_and_reveals_no_address() {
        let handle = CredentialHandle::new(digest());
        assert_eq!(handle.as_str(), DIGEST);
        assert_eq!(handle.digest().as_str(), DIGEST);
        assert_eq!(handle.digest().algorithm(), "sha256");
        let rendered = format!("{handle:?}");
        assert!(rendered.contains("sha256:"));
        assert!(
            !rendered.contains("keychain"),
            "a handle is not an address (Data Classification Section 3.1)"
        );
    }

    #[test]
    fn a_handle_cannot_be_built_from_anything_but_a_valid_digest() {
        for raw in ["", "sha256:", "not-a-digest", &DIGEST.to_uppercase()] {
            assert!(Digest::new(raw).is_err(), "{raw:?} must not yield a handle");
        }
    }

    #[test]
    fn a_handle_is_a_reference_and_not_a_secret() {
        // The egress matrix permits a handle in a log and denies bytes
        // (Data Classification Section 5). P1 has no type that can carry
        // credential bytes at all.
        let handle = CredentialHandle::new(digest());
        let value = serde_json::to_value(&handle).expect("serialises");
        assert_eq!(value, json!(DIGEST));
    }
}
