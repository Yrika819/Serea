use serde_json::json;
use serea_protocol::schema::{self, SchemaName};

#[test]
fn planned_model_turn_schema_requires_no_fabricated_execution_values() {
    let schema_doc = SchemaName::AssistantTask.json().expect("schema");
    let mut step_schema = schema_doc["$defs"]["step"].clone();
    step_schema["$defs"] = schema_doc["$defs"].clone();
    let validator = jsonschema::validator_for(&step_schema).expect("compiles");
    let step = json!({
        "step_id": "stp_01JQ8Z9M3R2CVN8H5FWK7PQDSF",
        "task_id": "tsk_01JQ8Z9K3M7QWXR4V2T6YH0BNA",
        "sequence": 0, "kind": "MODEL_TURN", "status": "PLANNED", "attempt": 0,
        "input_digest": "sha256:44136fa355b3678a1146ad16f7e8649e94fb4fc21fe77e8310c060f61caaff8a"
    });
    assert!(
        validator.is_valid(&step),
        "PLANNED needs no timestamps, result, or external-action key"
    );
    assert!(schema::validator(SchemaName::AssistantTask).is_ok());
}
