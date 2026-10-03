use serea_protocol::types::TaskStepDraft;
use serea_protocol::*;

#[test]
fn planned_model_turn_has_no_fabricated_execution_values() {
    let step = TaskStep::new(TaskStepDraft {
        step_id: StepId::new("stp_01JQ8Z9M3R2CVN8H5FWK7PQDSF").expect("synthetic"),
        task_id: TaskId::new("tsk_01JQ8Z9K3M7QWXR4V2T6YH0BNA").expect("synthetic"),
        sequence: 0,
        kind: StepKind::ModelTurn,
        status: StepStatus::new("PLANNED").expect("known"),
        attempt: 0,
        idempotency_key: None,
        provider_id: None,
        capability_id: None,
        capability_version: None,
        input_digest: Digest::new(
            "sha256:44136fa355b3678a1146ad16f7e8649e94fb4fc21fe77e8310c060f61caaff8a",
        )
        .expect("real digest of instruction {}"),
        result_digest: None,
        side_effect_receipt: None,
        started_at: None,
        completed_at: None,
        lease_owner: None,
        lease_expires_at: None,
        lease_generation: None,
        error: None,
        extensions: Default::default(),
    })
    .expect("valid planned model turn");
    let value = serde_json::to_value(step).expect("serializes");
    assert!(value.get("started_at").is_none());
    assert!(value.get("completed_at").is_none());
    assert!(value.get("result_digest").is_none());
}
