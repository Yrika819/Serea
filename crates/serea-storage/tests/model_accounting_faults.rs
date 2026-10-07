#![cfg(feature = "p2h-fault-injection")]

use serea_protocol::{
    Clock, CostClass, DataClass, EpochMillis, FinishReason, ModelId, ModelPurpose, ProviderId,
    RequestId, TokenCount,
};
use serea_storage::fault::{Action, Window};
use serea_storage::{
    ModelAttemptRelationKind, ModelAttemptState, ModelCallAttemptDraft, ModelCallCompletion,
    ModelDeploymentClass, ModelPriceSnapshot, ModelResponseStorage, Store, StoreError, UsdMicros,
    UtcAccountingDay,
};

struct Fixed;
impl Clock for Fixed {
    fn now_ms(&self) -> Result<EpochMillis, serea_protocol::ProtocolError> {
        EpochMillis::new(0)
    }
}

fn request(n: u8) -> RequestId {
    RequestId::new(format!("req_000000000000000000000000{n:02}")).unwrap()
}

fn draft(id: RequestId) -> ModelCallAttemptDraft {
    ModelCallAttemptDraft {
        request_id: id,
        task_id: None,
        purpose: ModelPurpose::Chat,
        model_id: ModelId::new("nemotron-3-nano-30b").unwrap(),
        provider_id: ProviderId::new("ollama").unwrap(),
        deployment_class: ModelDeploymentClass::Local,
        data_class: DataClass::Public,
        relation_kind: ModelAttemptRelationKind::None,
        parent_request_id: None,
        fallback_from_model_id: None,
        price: ModelPriceSnapshot::new(CostClass::Free, "free-1", 0, 0),
        max_context_tokens: 100,
        effective_max_output_tokens: 20,
        dispatch_intent_at: EpochMillis::new(0).unwrap(),
    }
}

fn completion() -> ModelCallCompletion {
    ModelCallCompletion {
        input_tokens: TokenCount::new(2),
        output_tokens: TokenCount::new(3),
        latency_ms: 1,
        repair_attempts: 0,
        finish_reason: FinishReason::Stop,
        recorded_at: EpochMillis::new(1).unwrap(),
        accepted_response: ModelResponseStorage {
            canonical_json: br#""accepted""#.to_vec(),
            data_class: DataClass::Public,
        },
    }
}

#[test]
fn attempt_insert_fault_rolls_back_call_count_and_reservation() {
    let store = Store::open_in_memory(&Fixed).unwrap();
    Window::AfterModelAttemptInsert
        .arm(Action::Fail(StoreError::Sqlite))
        .unwrap();
    assert_eq!(
        store.reserve_model_call(draft(request(1)), UsdMicros::new(100).unwrap()),
        Err(StoreError::Sqlite)
    );
    assert!(store.get_model_call_attempt(&request(1)).unwrap().is_none());
    assert_eq!(
        store
            .utc_day_spend_occupancy(UtcAccountingDay::from_epoch_millis(
                EpochMillis::new(0).unwrap()
            ))
            .unwrap(),
        UsdMicros::new(0).unwrap()
    );
}

#[test]
fn response_usage_and_terminal_faults_rollback_completion_as_one_unit() {
    for window in [
        Window::AfterModelResponseBlob,
        Window::AfterModelUsageInsert,
        Window::AfterModelAttemptTerminal,
    ] {
        let store = Store::open_in_memory(&Fixed).unwrap();
        store
            .reserve_model_call(draft(request(2)), UsdMicros::new(100).unwrap())
            .unwrap();
        window.arm(Action::Fail(StoreError::Sqlite)).unwrap();
        assert_eq!(
            store.complete_model_call(&request(2), completion()),
            Err(StoreError::Sqlite)
        );
        assert_eq!(
            store
                .get_model_call_attempt(&request(2))
                .unwrap()
                .unwrap()
                .state,
            ModelAttemptState::DispatchIntent
        );
        assert!(
            store
                .model_usage_for_request(&request(2))
                .unwrap()
                .is_none()
        );
        assert!(
            store
                .get_model_call_response(&request(2))
                .unwrap()
                .is_none()
        );
    }
}
