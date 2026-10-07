use serea_event_bus::{EventBus, ModelEventMetadataV1, ModelEventRelationV1};
use serea_protocol::{
    CostClass, DataClass, EpochMillis, EventKind, FinishReason, ModelErrorCode, ModelId,
    ModelPurpose, ProviderId, RequestId, TimestampMs, UlidSource, UlidValue,
};

struct FixedIds;

impl UlidSource for FixedIds {
    fn next_ulid(&mut self) -> UlidValue {
        UlidValue::new(TimestampMs::new(1).unwrap(), [1; 10])
    }
}

#[test]
fn model_completed_event_contains_only_trusted_accounting_metadata() {
    let bus = EventBus::new(FixedIds);
    let metadata = ModelEventMetadataV1 {
        request_id: RequestId::new("req_00000000000000000000000002").unwrap(),
        model_id: ModelId::new("gpt-oss-20b").unwrap(),
        provider_id: ProviderId::new("provider").unwrap(),
        task_id: None,
        purpose: ModelPurpose::Chat,
        relation: ModelEventRelationV1::Normal,
        data_class: DataClass::Personal,
        occurred_at: EpochMillis::new(1_767_225_600_001).unwrap(),
    };
    let draft = bus
        .draft_model_completed(serea_event_bus::ModelCompletedEventV1 {
            metadata,
            finish_reason: FinishReason::Stop,
            input_tokens: 12,
            output_tokens: 8,
            cost_class: CostClass::Paid,
            cost_usd_micros: 19,
            price_revision: "price-1".into(),
            repair_attempts: 0,
        })
        .unwrap();

    assert_eq!(draft.event.kind, EventKind::ModelCompleted);
    assert_eq!(draft.event.payload.len(), 12);
    assert_eq!(draft.event.payload["input_tokens"], 12);
    assert_eq!(draft.event.payload["output_tokens"], 8);
    assert_eq!(draft.event.payload["cost_usd_micros"], 19);
    assert_eq!(draft.event.payload["price_revision"], "price-1");
    assert_eq!(draft.event.payload["repair_attempts"], 0);
}

#[test]
fn model_failed_event_has_stable_code_and_no_diagnostic_text() {
    let bus = EventBus::new(FixedIds);
    let draft = bus
        .draft_model_failed(serea_event_bus::ModelFailedEventV1 {
            metadata: ModelEventMetadataV1 {
                request_id: RequestId::new("req_00000000000000000000000003").unwrap(),
                model_id: ModelId::new("nemotron-3-nano-30b").unwrap(),
                provider_id: ProviderId::new("provider").unwrap(),
                task_id: None,
                purpose: ModelPurpose::Chat,
                relation: ModelEventRelationV1::Normal,
                data_class: DataClass::Public,
                occurred_at: EpochMillis::new(1_767_225_600_002).unwrap(),
            },
            error_kind: ModelErrorCode::new("UPSTREAM_UNAVAILABLE").unwrap(),
            retryable: true,
        })
        .unwrap();

    assert_eq!(draft.event.kind, EventKind::ModelFailed);
    assert_eq!(draft.event.payload.len(), 7);
    assert_eq!(draft.event.payload["error_kind"], "UPSTREAM_UNAVAILABLE");
    assert_eq!(draft.event.payload["retryable"], true);
}

#[test]
fn model_called_event_has_only_bounded_host_metadata() {
    let bus = EventBus::new(FixedIds);
    let draft = bus
        .draft_model_called(ModelEventMetadataV1 {
            request_id: RequestId::new("req_00000000000000000000000001").unwrap(),
            model_id: ModelId::new("nemotron-3-nano-30b").unwrap(),
            provider_id: ProviderId::new("provider").unwrap(),
            task_id: None,
            purpose: ModelPurpose::Chat,
            relation: ModelEventRelationV1::Normal,
            data_class: DataClass::Public,
            occurred_at: EpochMillis::new(1_767_225_600_000).unwrap(),
        })
        .unwrap();

    assert_eq!(draft.event.kind, EventKind::ModelCalled);
    assert_eq!(draft.event.payload.len(), 5);
    assert_eq!(
        draft.event.payload["request_id"],
        "req_00000000000000000000000001"
    );
    assert_eq!(draft.event.payload["model_id"], "nemotron-3-nano-30b");
    assert_eq!(draft.event.payload["provider_id"], "provider");
    assert_eq!(draft.event.payload["purpose"], "CHAT");
    assert_eq!(draft.event.payload["relation_kind"], "NORMAL");
    assert!(draft.retention_at.is_some());
}

#[test]
fn fallback_events_carry_only_bounded_host_identity_metadata() {
    let bus = EventBus::new(FixedIds);
    let metadata = ModelEventMetadataV1 {
        request_id: RequestId::new("req_00000000000000000000000005").unwrap(),
        model_id: ModelId::new("nemotron-3-nano-30b").unwrap(),
        provider_id: ProviderId::new("provider").unwrap(),
        task_id: None,
        purpose: ModelPurpose::Chat,
        relation: ModelEventRelationV1::Normal,
        data_class: DataClass::Public,
        occurred_at: EpochMillis::new(1_767_225_600_005).unwrap(),
    };
    let fallback = bus
        .draft_model_fallback(serea_event_bus::ModelFallbackEventV1 {
            metadata: metadata.clone(),
            fallback_request_id: RequestId::new("req_00000000000000000000000006").unwrap(),
            fallback_model_id: ModelId::new("gpt-oss-20b").unwrap(),
        })
        .unwrap();
    assert_eq!(fallback.event.kind, EventKind::ModelFallback);
    assert_eq!(fallback.event.payload.len(), 7);
    assert_eq!(
        fallback.event.payload["fallback_request_id"],
        "req_00000000000000000000000006"
    );
    assert_eq!(fallback.event.payload["fallback_model_id"], "gpt-oss-20b");

    let exhausted = bus
        .draft_model_fallback_exhausted(serea_event_bus::ModelFallbackExhaustedEventV1 {
            metadata,
            fallback_model_id: ModelId::new("gpt-oss-20b").unwrap(),
        })
        .unwrap();
    assert_eq!(exhausted.event.kind, EventKind::ModelFallbackExhausted);
    assert_eq!(exhausted.event.payload.len(), 6);
    assert_eq!(exhausted.event.payload["fallback_model_id"], "gpt-oss-20b");
}

#[test]
fn model_output_invalid_event_contains_only_bounded_failure_metadata() {
    let bus = EventBus::new(FixedIds);
    let draft = bus
        .draft_model_output_invalid(serea_event_bus::ModelOutputInvalidEventV1 {
            metadata: ModelEventMetadataV1 {
                request_id: RequestId::new("req_00000000000000000000000004").unwrap(),
                model_id: ModelId::new("gpt-oss-20b").unwrap(),
                provider_id: ProviderId::new("provider").unwrap(),
                task_id: None,
                purpose: ModelPurpose::Analysis,
                relation: ModelEventRelationV1::Normal,
                data_class: DataClass::Public,
                occurred_at: EpochMillis::new(1_767_225_600_003).unwrap(),
            },
            diagnostic_count: 32,
        })
        .unwrap();

    assert_eq!(draft.event.kind, EventKind::ModelOutputInvalid);
    assert_eq!(draft.event.payload.len(), 6);
    assert_eq!(
        draft.event.payload["request_id"],
        "req_00000000000000000000000004"
    );
    assert_eq!(draft.event.payload["diagnostic_count"], 32);
    assert!(!draft.event.payload.contains_key("raw_response"));
    assert!(!draft.event.payload.contains_key("prompt"));
}

#[test]
fn model_output_invalid_event_refuses_unbounded_diagnostic_count() {
    let bus = EventBus::new(FixedIds);
    let result = bus.draft_model_output_invalid(serea_event_bus::ModelOutputInvalidEventV1 {
        metadata: ModelEventMetadataV1 {
            request_id: RequestId::new("req_00000000000000000000000005").unwrap(),
            model_id: ModelId::new("gpt-oss-20b").unwrap(),
            provider_id: ProviderId::new("provider").unwrap(),
            task_id: None,
            purpose: ModelPurpose::Analysis,
            relation: ModelEventRelationV1::Normal,
            data_class: DataClass::Public,
            occurred_at: EpochMillis::new(1_767_225_600_004).unwrap(),
        },
        diagnostic_count: 33,
    });
    assert!(matches!(
        result,
        Err(serea_storage::StoreError::InvalidModelCall)
    ));
}
