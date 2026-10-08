use std::future::Future;
use std::pin::pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::task::{Context, Poll, Waker};

use async_trait::async_trait;
use serea_model_router::{
    ModelDeploymentClass, ModelEgressPolicySnapshotV1, ModelRosterEntryV1, ModelRosterV1,
    ModelRouterV1, ModelRoutingRequirementsV1, PreparedModelCallDraftV1, PreparedModelCallV1,
    RouterError, StructuredRequirementV1, bind_provider_response, preference_chain,
    vision_candidate_chain,
};
use serea_protocol::provider::{ModelCallContext, ModelProvider};
use serea_protocol::{
    CostClass, DataClass, JsonSchemaMode, ModelCapabilities, ModelDescriptor, ModelError,
    ModelErrorCode, ModelId, ModelMessage, ModelPurpose, ModelResponse, ProviderHealth, ProviderId,
    ResponseFormat,
};

struct FakeProvider {
    id: ProviderId,
    models: Vec<ModelDescriptor>,
    health: ProviderHealth,
    health_reads: Arc<AtomicUsize>,
    generate_calls: Arc<AtomicUsize>,
}

#[async_trait]
impl ModelProvider for FakeProvider {
    fn provider_id(&self) -> ProviderId {
        self.id.clone()
    }
    fn models(&self) -> Vec<ModelDescriptor> {
        self.models.clone()
    }
    async fn health(&self) -> ProviderHealth {
        self.health_reads.fetch_add(1, Ordering::SeqCst);
        self.health
    }
    async fn generate(
        &self,
        _request: &serea_protocol::ModelRequest,
        _ctx: &ModelCallContext,
    ) -> Result<ModelResponse, ModelError> {
        self.generate_calls.fetch_add(1, Ordering::SeqCst);
        Err(ModelError {
            kind: ModelErrorCode::new("UNEXPECTED_TEST_DISPATCH")
                .unwrap_or_else(|_| unreachable!()),
            message: serea_protocol::ErrorMessage::new("test provider must not dispatch")
                .unwrap_or_else(|_| unreachable!()),
            retryable: false,
        })
    }
}

#[test]
fn exact_chains_are_ordered_and_codex_is_impossible() {
    for purpose in [
        ModelPurpose::Chat,
        ModelPurpose::Planning,
        ModelPurpose::Extraction,
        ModelPurpose::Analysis,
        ModelPurpose::Proactive,
    ] {
        assert_eq!(
            preference_chain(purpose),
            &["nemotron-3-nano-30b", "gpt-oss-20b"]
        );
    }
    assert_eq!(
        preference_chain(ModelPurpose::StructuredRepair),
        &["gpt-oss-20b"]
    );
    assert_eq!(vision_candidate_chain(), &["gemma-4-31b"]);
    assert!(matches!(
        ModelRosterEntryV1::new(
            model_id("codex"),
            provider_id("provider"),
            ModelDeploymentClass::Local,
            true,
            capabilities(JsonSchemaMode::Strict, 1000, 1000),
            CostClass::Free
        ),
        Err(RouterError::UnknownOrForbiddenModel)
    ));
}

#[test]
fn provider_response_identity_is_bound_and_untrusted_fields_are_cleared() {
    let request_id = serea_protocol::RequestId::new("req_00000000000000000000000001")
        .unwrap_or_else(|_| unreachable!());
    let expected_model = model_id("nemotron-3-nano-30b");
    let expected_provider = provider_id("provider");
    let response = ModelResponse {
        request_id: request_id.clone(),
        model_id: expected_model.clone(),
        provider_id: expected_provider.clone(),
        content: "hello".into(),
        structured: Some(serde_json::json!({"untrusted": true})),
        finish_reason: serea_protocol::FinishReason::Stop,
        usage: serea_protocol::ModelUsage {
            input_tokens: serea_protocol::TokenCount::new(2),
            output_tokens: serea_protocol::TokenCount::new(1),
            cost_class: CostClass::Paid,
        },
        latency_ms: 3,
        repair_attempts: 2,
    };
    let accepted = bind_provider_response(
        &request_id,
        &expected_model,
        &expected_provider,
        response.clone(),
    )
    .expect("matching provider identity should bind");
    assert_eq!(accepted.structured, None);
    assert_eq!(accepted.repair_attempts, 0);

    assert!(
        bind_provider_response(
            &request_id,
            &expected_model,
            &expected_provider,
            ModelResponse {
                request_id: serea_protocol::RequestId::new("req_00000000000000000000000002")
                    .unwrap_or_else(|_| unreachable!()),
                ..response.clone()
            },
        )
        .is_err()
    );
    assert!(
        bind_provider_response(
            &request_id,
            &expected_model,
            &expected_provider,
            ModelResponse {
                model_id: model_id("gpt-oss-20b"),
                ..response.clone()
            },
        )
        .is_err()
    );
    assert!(
        bind_provider_response(
            &request_id,
            &expected_model,
            &expected_provider,
            ModelResponse {
                provider_id: provider_id("spoofed"),
                ..response
            },
        )
        .is_err()
    );
}

#[test]
fn post_routing_request_uses_selected_model_and_host_prepared_semantics() {
    let roster = ModelRosterV1::new(vec![entry(
        "nemotron-3-nano-30b",
        "provider",
        ModelDeploymentClass::Cloud,
        true,
        capabilities(JsonSchemaMode::Strict, 1000, 1000),
        CostClass::Paid,
    )])
    .unwrap();
    let provider = provider(
        "provider",
        vec![descriptor(
            "nemotron-3-nano-30b",
            "provider",
            capabilities(JsonSchemaMode::Strict, 1000, 1000),
        )],
        ProviderHealth::Ready,
    );
    let router = ModelRouterV1::new(roster, vec![provider]).unwrap();
    let call = prepared(
        ModelPurpose::Chat,
        ResponseFormat::Text,
        DataClass::Public,
        requirements(StructuredRequirementV1::Any),
    );
    let request_id = serea_protocol::RequestId::new("req_00000000000000000000000003")
        .unwrap_or_else(|_| unreachable!());
    let session = block_on(router.route(&call)).unwrap();
    let request = router
        .build_request(
            &call,
            &session,
            &request_id,
            &model_id("nemotron-3-nano-30b"),
        )
        .expect("configured selected model should produce a request");
    assert_eq!(request.request_id, request_id);
    assert_eq!(request.model_id, model_id("nemotron-3-nano-30b"));
    assert_eq!(request.purpose, ModelPurpose::Chat);
    assert_eq!(request.messages, call.messages());
    assert_eq!(request.system.as_deref(), call.system());
    assert_eq!(request.response_format, ResponseFormat::Text);
    assert_eq!(request.tools, call.tools());
    assert_eq!(request.max_output_tokens, call.max_output_tokens());
    assert_eq!(request.temperature, call.temperature());
    assert_eq!(request.deadline_ms, call.deadline_ms());
    assert_eq!(request.data_class, call.data_class());
    assert_eq!(
        router.build_request(&call, &session, &request_id, &model_id("gpt-oss-20b"),),
        Err(RouterError::ModelSelectionInvalid)
    );
}

#[test]
fn duplicate_model_id_and_provider_registration_are_rejected() {
    let item = entry(
        "nemotron-3-nano-30b",
        "provider",
        ModelDeploymentClass::Cloud,
        true,
        capabilities(JsonSchemaMode::Strict, 1000, 1000),
        CostClass::Low,
    );
    assert!(matches!(
        ModelRosterV1::new(vec![item.clone(), item]),
        Err(RouterError::DuplicateModelId)
    ));
    let roster = ModelRosterV1::new(vec![entry(
        "nemotron-3-nano-30b",
        "provider",
        ModelDeploymentClass::Cloud,
        true,
        capabilities(JsonSchemaMode::Strict, 1000, 1000),
        CostClass::Low,
    )])
    .unwrap();
    let provider = provider(
        "provider",
        vec![descriptor(
            "nemotron-3-nano-30b",
            "provider",
            capabilities(JsonSchemaMode::Strict, 1000, 1000),
        )],
        ProviderHealth::Ready,
    );
    assert!(matches!(
        ModelRouterV1::new(roster, vec![provider.clone(), provider]),
        Err(RouterError::DuplicateProviderId)
    ));
}

#[test]
fn missing_primary_discovery_selects_next_chain_member_without_reordering() {
    let roster = ModelRosterV1::new(vec![
        entry(
            "nemotron-3-nano-30b",
            "p1",
            ModelDeploymentClass::Cloud,
            true,
            capabilities(JsonSchemaMode::BestEffort, 1000, 1000),
            CostClass::Low,
        ),
        entry(
            "gpt-oss-20b",
            "p2",
            ModelDeploymentClass::Cloud,
            true,
            capabilities(JsonSchemaMode::BestEffort, 1000, 1000),
            CostClass::Paid,
        ),
    ])
    .unwrap();
    let p1 = provider("p1", vec![], ProviderHealth::Ready);
    let p2 = provider(
        "p2",
        vec![descriptor(
            "gpt-oss-20b",
            "p2",
            capabilities(JsonSchemaMode::BestEffort, 1000, 1000),
        )],
        ProviderHealth::Ready,
    );
    let router = ModelRouterV1::new(roster, vec![p2, p1]).unwrap();
    let session = block_on(router.route(&prepared(
        ModelPurpose::Analysis,
        ResponseFormat::JsonSchema {
            schema: serde_json::json!({"type":"object"}),
        },
        DataClass::Public,
        requirements(StructuredRequirementV1::Any),
    )))
    .unwrap();
    assert_eq!(
        session.decision().map(|id| id.to_string()),
        Some("gpt-oss-20b".into())
    );
}

#[test]
fn provider_identity_mismatch_fails_closed() {
    let roster = ModelRosterV1::new(vec![entry(
        "nemotron-3-nano-30b",
        "p1",
        ModelDeploymentClass::Cloud,
        true,
        capabilities(JsonSchemaMode::Strict, 1000, 1000),
        CostClass::Low,
    )])
    .unwrap();
    let provider = provider(
        "p1",
        vec![descriptor(
            "nemotron-3-nano-30b",
            "p2",
            capabilities(JsonSchemaMode::Strict, 1000, 1000),
        )],
        ProviderHealth::Ready,
    );
    assert!(matches!(
        ModelRouterV1::new(roster, vec![provider]),
        Err(RouterError::ProviderIdentityMismatch)
    ));
}

#[test]
fn tool_filter_and_price_never_reorder_the_frozen_chain() {
    let mut primary_host = capabilities(JsonSchemaMode::Strict, 1000, 1000);
    primary_host.tools = false;
    let roster = ModelRosterV1::new(vec![
        entry(
            "nemotron-3-nano-30b",
            "provider1",
            ModelDeploymentClass::Cloud,
            true,
            primary_host,
            CostClass::Paid,
        ),
        entry(
            "gpt-oss-20b",
            "provider2",
            ModelDeploymentClass::Local,
            true,
            capabilities(JsonSchemaMode::Strict, 1000, 1000),
            CostClass::Free,
        ),
    ])
    .unwrap();
    let p1 = provider(
        "provider1",
        vec![descriptor(
            "nemotron-3-nano-30b",
            "provider1",
            capabilities(JsonSchemaMode::Strict, 1000, 1000),
        )],
        ProviderHealth::Ready,
    );
    let p2 = provider(
        "provider2",
        vec![descriptor(
            "gpt-oss-20b",
            "provider2",
            capabilities(JsonSchemaMode::Strict, 1000, 1000),
        )],
        ProviderHealth::Ready,
    );
    let router = ModelRouterV1::new(roster, vec![p2, p1]).unwrap();
    let mut req = requirements(StructuredRequirementV1::Any);
    req.tools_required = true;
    let routed = block_on(router.route(&prepared(
        ModelPurpose::Chat,
        ResponseFormat::Text,
        DataClass::Public,
        req,
    )))
    .unwrap();
    assert_eq!(
        routed.decision().map(|id| id.to_string()),
        Some("gpt-oss-20b".into())
    );

    let roster = ModelRosterV1::new(vec![
        entry(
            "nemotron-3-nano-30b",
            "provider1",
            ModelDeploymentClass::Cloud,
            true,
            capabilities(JsonSchemaMode::Strict, 1000, 1000),
            CostClass::Paid,
        ),
        entry(
            "gpt-oss-20b",
            "provider2",
            ModelDeploymentClass::Local,
            true,
            capabilities(JsonSchemaMode::Strict, 1000, 1000),
            CostClass::Free,
        ),
    ])
    .unwrap();
    let p1 = provider(
        "provider1",
        vec![descriptor(
            "nemotron-3-nano-30b",
            "provider1",
            capabilities(JsonSchemaMode::Strict, 1000, 1000),
        )],
        ProviderHealth::Ready,
    );
    let p2 = provider(
        "provider2",
        vec![descriptor(
            "gpt-oss-20b",
            "provider2",
            capabilities(JsonSchemaMode::Strict, 1000, 1000),
        )],
        ProviderHealth::Ready,
    );
    let router = ModelRouterV1::new(roster, vec![p2, p1]).unwrap();
    let routed = block_on(router.route(&prepared(
        ModelPurpose::Chat,
        ResponseFormat::Text,
        DataClass::Public,
        requirements(StructuredRequirementV1::Any),
    )))
    .unwrap();
    assert_eq!(
        routed.decision().map(|id| id.to_string()),
        Some("nemotron-3-nano-30b".into())
    );
}

#[test]
fn personal_cloud_is_eligible_only_through_the_trusted_prepared_boundary() {
    let roster = ModelRosterV1::new(vec![entry(
        "nemotron-3-nano-30b",
        "provider",
        ModelDeploymentClass::Cloud,
        true,
        capabilities(JsonSchemaMode::Strict, 1000, 1000),
        CostClass::Low,
    )])
    .unwrap();
    let provider = provider(
        "provider",
        vec![descriptor(
            "nemotron-3-nano-30b",
            "provider",
            capabilities(JsonSchemaMode::Strict, 1000, 1000),
        )],
        ProviderHealth::Ready,
    );
    let router = ModelRouterV1::new(roster, vec![provider]).unwrap();
    let mut draft = call(
        ModelPurpose::Chat,
        ResponseFormat::Text,
        DataClass::Personal,
        requirements(StructuredRequirementV1::Any),
    );
    draft.egress = ModelEgressPolicySnapshotV1::from_host(true);
    let allowed = PreparedModelCallV1::from_host(draft).unwrap();
    assert_eq!(
        block_on(router.route(&allowed))
            .unwrap()
            .decision()
            .map(|id| id.to_string()),
        Some("nemotron-3-nano-30b".into())
    );

    let denied = prepared(
        ModelPurpose::Chat,
        ResponseFormat::Text,
        DataClass::Personal,
        requirements(StructuredRequirementV1::Any),
    );
    assert_eq!(block_on(router.route(&denied)).unwrap().decision(), None);
}

#[test]
fn strict_filters_best_effort_but_any_accepts_it() {
    let roster = ModelRosterV1::new(vec![entry(
        "nemotron-3-nano-30b",
        "provider",
        ModelDeploymentClass::Cloud,
        true,
        capabilities(JsonSchemaMode::Strict, 1000, 1000),
        CostClass::Low,
    )])
    .unwrap();
    let provider = provider(
        "provider",
        vec![descriptor(
            "nemotron-3-nano-30b",
            "provider",
            capabilities(JsonSchemaMode::BestEffort, 1000, 1000),
        )],
        ProviderHealth::Ready,
    );
    let router = ModelRouterV1::new(roster, vec![provider]).unwrap();
    let any = block_on(router.route(&prepared(
        ModelPurpose::Analysis,
        ResponseFormat::JsonSchema {
            schema: serde_json::json!({}),
        },
        DataClass::Public,
        requirements(StructuredRequirementV1::Any),
    )))
    .unwrap();
    assert_eq!(
        any.decision().map(|id| id.to_string()),
        Some("nemotron-3-nano-30b".into())
    );
    let strict = block_on(router.route(&prepared(
        ModelPurpose::Planning,
        ResponseFormat::JsonSchema {
            schema: serde_json::json!({}),
        },
        DataClass::Public,
        requirements(StructuredRequirementV1::Strict),
    )))
    .unwrap();
    assert_eq!(strict.decision(), None);
}

#[test]
fn tools_context_output_data_class_health_and_single_snapshot_are_enforced() {
    let health_reads = Arc::new(AtomicUsize::new(0));
    let generate_calls = Arc::new(AtomicUsize::new(0));
    let roster = ModelRosterV1::new(vec![
        entry(
            "nemotron-3-nano-30b",
            "provider1",
            ModelDeploymentClass::Cloud,
            true,
            capabilities(JsonSchemaMode::Strict, 200, 64),
            CostClass::Low,
        ),
        entry(
            "gpt-oss-20b",
            "provider2",
            ModelDeploymentClass::Local,
            true,
            capabilities(JsonSchemaMode::Strict, 200, 64),
            CostClass::Paid,
        ),
    ])
    .unwrap();
    let p1 = provider_with_counts(
        "provider1",
        vec![descriptor(
            "nemotron-3-nano-30b",
            "provider1",
            capabilities(JsonSchemaMode::Strict, 200, 64),
        )],
        ProviderHealth::Degraded,
        health_reads.clone(),
        generate_calls.clone(),
    );
    let p2 = provider_with_counts(
        "provider2",
        vec![descriptor(
            "gpt-oss-20b",
            "provider2",
            capabilities(JsonSchemaMode::Strict, 200, 64),
        )],
        ProviderHealth::Ready,
        health_reads.clone(),
        generate_calls.clone(),
    );
    let router = ModelRouterV1::new(roster, vec![p2, p1]).unwrap();
    let session = block_on(router.route(&prepared(
        ModelPurpose::Planning,
        ResponseFormat::JsonSchema {
            schema: serde_json::json!({}),
        },
        DataClass::Personal,
        requirements(StructuredRequirementV1::Strict),
    )))
    .unwrap();
    assert_eq!(
        session.decision().map(|id| id.to_string()),
        Some("gpt-oss-20b".into())
    );
    assert_eq!(health_reads.load(Ordering::SeqCst), 2);
    assert_eq!(generate_calls.load(Ordering::SeqCst), 0);
    assert_eq!(
        session.health().health(&provider_id("provider1")),
        ProviderHealth::Degraded
    );

    let mut needs_tools = requirements(StructuredRequirementV1::Strict);
    needs_tools.tools_required = true;
    assert_eq!(
        block_on(router.route(&prepared(
            ModelPurpose::Planning,
            ResponseFormat::JsonSchema {
                schema: serde_json::json!({})
            },
            DataClass::Public,
            needs_tools
        )))
        .unwrap()
        .decision()
        .map(|id| id.to_string()),
        Some("gpt-oss-20b".into())
    );
    let mut too_large = requirements(StructuredRequirementV1::Strict);
    too_large.min_context_tokens = 201;
    assert_eq!(
        block_on(router.route(&prepared(
            ModelPurpose::Planning,
            ResponseFormat::JsonSchema {
                schema: serde_json::json!({})
            },
            DataClass::Public,
            too_large
        )))
        .unwrap()
        .decision(),
        None
    );
    let mut output_too_large = requirements(StructuredRequirementV1::Strict);
    output_too_large.min_output_tokens = 65;
    assert_eq!(
        block_on(router.route(&prepared(
            ModelPurpose::Planning,
            ResponseFormat::JsonSchema {
                schema: serde_json::json!({})
            },
            DataClass::Public,
            output_too_large
        )))
        .unwrap()
        .decision(),
        None
    );
    for class in [DataClass::Private, DataClass::Secret, DataClass::Credential] {
        let result = PreparedModelCallV1::from_host(call(
            class_purpose(class),
            ResponseFormat::JsonSchema {
                schema: serde_json::json!({}),
            },
            class,
            requirements(StructuredRequirementV1::Strict),
        ));
        assert!(matches!(result, Err(RouterError::DataClassRefused)));
    }
}

#[test]
fn illegal_format_non_finite_temperature_and_vision_refuse_before_health() {
    let reads = Arc::new(AtomicUsize::new(0));
    let roster = ModelRosterV1::new(vec![entry(
        "nemotron-3-nano-30b",
        "provider",
        ModelDeploymentClass::Cloud,
        true,
        capabilities(JsonSchemaMode::Strict, 1000, 1000),
        CostClass::Low,
    )])
    .unwrap();
    let p = provider_with_counts(
        "provider",
        vec![descriptor(
            "nemotron-3-nano-30b",
            "provider",
            capabilities(JsonSchemaMode::Strict, 1000, 1000),
        )],
        ProviderHealth::Ready,
        reads.clone(),
        Arc::new(AtomicUsize::new(0)),
    );
    let router = ModelRouterV1::new(roster, vec![p]).unwrap();
    let illegal = call(
        ModelPurpose::Planning,
        ResponseFormat::Text,
        DataClass::Public,
        requirements(StructuredRequirementV1::Strict),
    );
    assert!(matches!(
        PreparedModelCallV1::from_host(illegal),
        Err(RouterError::IllegalPurposeFormat)
    ));
    for temperature in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        let mut non_finite = call(
            ModelPurpose::Chat,
            ResponseFormat::Text,
            DataClass::Public,
            requirements(StructuredRequirementV1::Any),
        );
        non_finite.temperature = temperature;
        assert!(matches!(
            PreparedModelCallV1::from_host(non_finite),
            Err(RouterError::NonFiniteTemperature)
        ));
    }
    let mut vision = requirements(StructuredRequirementV1::Any);
    vision.vision_required = true;
    let prepared = PreparedModelCallV1::from_host(call(
        ModelPurpose::Chat,
        ResponseFormat::Text,
        DataClass::Public,
        vision,
    ))
    .unwrap();
    assert!(matches!(
        block_on(router.route(&prepared)),
        Err(RouterError::VisionInputUnsupported)
    ));
    assert_eq!(reads.load(Ordering::SeqCst), 0);
}

#[test]
fn oversized_prompt_and_schema_fail_before_routing() {
    let mut prompt = call(
        ModelPurpose::Chat,
        ResponseFormat::Text,
        DataClass::Public,
        requirements(StructuredRequirementV1::Any),
    );
    prompt.messages[0].content = "x".repeat(serea_model_router::MAX_MODEL_PROMPT_BYTES + 1);
    assert!(matches!(
        PreparedModelCallV1::from_host(prompt),
        Err(RouterError::PromptTooLarge)
    ));

    let schema =
        serde_json::Value::String("x".repeat(serea_model_router::MAX_MODEL_SCHEMA_BYTES + 1));
    let schema_call = call(
        ModelPurpose::Analysis,
        ResponseFormat::JsonSchema { schema },
        DataClass::Public,
        requirements(StructuredRequirementV1::Any),
    );
    assert!(matches!(
        PreparedModelCallV1::from_host(schema_call),
        Err(RouterError::SchemaTooLarge)
    ));
}

#[test]
fn invalid_host_schema_refuses_before_any_provider_operation() {
    let health_reads = Arc::new(AtomicUsize::new(0));
    let generate_calls = Arc::new(AtomicUsize::new(0));
    let roster = ModelRosterV1::new(vec![entry(
        "nemotron-3-nano-30b",
        "provider",
        ModelDeploymentClass::Cloud,
        true,
        capabilities(JsonSchemaMode::Strict, 1000, 1000),
        CostClass::Paid,
    )])
    .unwrap();
    let provider = provider_with_counts(
        "provider",
        vec![descriptor(
            "nemotron-3-nano-30b",
            "provider",
            capabilities(JsonSchemaMode::Strict, 1000, 1000),
        )],
        ProviderHealth::Ready,
        health_reads.clone(),
        generate_calls.clone(),
    );
    let _router = ModelRouterV1::new(roster, vec![provider]).unwrap();
    let invalid = call(
        ModelPurpose::Analysis,
        ResponseFormat::JsonSchema {
            schema: serde_json::json!({"type":"not-a-json-schema-type"}),
        },
        DataClass::Public,
        requirements(StructuredRequirementV1::Any),
    );
    assert!(matches!(
        PreparedModelCallV1::from_host(invalid),
        Err(RouterError::InvalidJsonSchema)
    ));
    assert_eq!(health_reads.load(Ordering::SeqCst), 0);
    assert_eq!(generate_calls.load(Ordering::SeqCst), 0);
}

fn class_purpose(_class: DataClass) -> ModelPurpose {
    ModelPurpose::Planning
}

fn call(
    purpose: ModelPurpose,
    response_format: ResponseFormat,
    data_class: DataClass,
    requirements: ModelRoutingRequirementsV1,
) -> PreparedModelCallDraftV1 {
    PreparedModelCallDraftV1 {
        task_id: None,
        purpose,
        messages: vec![ModelMessage {
            role: serea_protocol::MessageRole::new("user").unwrap_or_else(|_| unreachable!()),
            content: "prepared".into(),
        }],
        system: Some("system".into()),
        response_format,
        tools: vec![],
        max_output_tokens: 128,
        temperature: 0.2,
        deadline_ms: 10_000,
        data_class,
        requirements,
        egress: ModelEgressPolicySnapshotV1::from_host(false),
        host_max_output_tokens: 2048,
    }
}

fn prepared(
    purpose: ModelPurpose,
    response_format: ResponseFormat,
    data_class: DataClass,
    requirements: ModelRoutingRequirementsV1,
) -> PreparedModelCallV1 {
    PreparedModelCallV1::from_host(call(purpose, response_format, data_class, requirements))
        .unwrap_or_else(|error| panic!("prepared call invalid: {error:?}"))
}

fn requirements(structured_requirement: StructuredRequirementV1) -> ModelRoutingRequirementsV1 {
    ModelRoutingRequirementsV1 {
        vision_required: false,
        tools_required: false,
        min_context_tokens: 0,
        min_output_tokens: 0,
        structured_requirement,
    }
}

fn provider(
    id: &str,
    models: Vec<ModelDescriptor>,
    health: ProviderHealth,
) -> Arc<dyn ModelProvider> {
    provider_with_counts(
        id,
        models,
        health,
        Arc::new(AtomicUsize::new(0)),
        Arc::new(AtomicUsize::new(0)),
    )
}

fn provider_with_counts(
    id: &str,
    models: Vec<ModelDescriptor>,
    health: ProviderHealth,
    health_reads: Arc<AtomicUsize>,
    generate_calls: Arc<AtomicUsize>,
) -> Arc<dyn ModelProvider> {
    Arc::new(FakeProvider {
        id: provider_id(id),
        models,
        health,
        health_reads,
        generate_calls,
    })
}

fn provider_id(id: &str) -> ProviderId {
    ProviderId::new(id).unwrap_or_else(|_| unreachable!())
}
fn model_id(id: &str) -> ModelId {
    ModelId::new(id).unwrap_or_else(|_| unreachable!())
}

fn entry(
    id: &str,
    provider: &str,
    deployment: ModelDeploymentClass,
    enabled: bool,
    allowed: ModelCapabilities,
    cost: CostClass,
) -> ModelRosterEntryV1 {
    ModelRosterEntryV1::new(
        model_id(id),
        provider_id(provider),
        deployment,
        enabled,
        allowed,
        cost,
    )
    .unwrap_or_else(|_| unreachable!())
}

fn descriptor(id: &str, provider: &str, capabilities: ModelCapabilities) -> ModelDescriptor {
    ModelDescriptor {
        model_id: model_id(id),
        provider_id: provider_id(provider),
        capabilities,
    }
}

fn capabilities(mode: JsonSchemaMode, context: u32, output: u32) -> ModelCapabilities {
    ModelCapabilities {
        vision: false,
        tools: true,
        structured_output: true,
        json_schema_mode: mode,
        thinking: false,
        long_context: false,
        fast: false,
        code_specialist: false,
        max_context_tokens: context,
        max_output_tokens: output,
        supports_streaming: false,
        supports_seeds: false,
    }
}

fn block_on<F: Future>(future: F) -> F::Output {
    let mut context = Context::from_waker(Waker::noop());
    let mut future = pin!(future);
    match Future::poll(future.as_mut(), &mut context) {
        Poll::Ready(value) => value,
        Poll::Pending => unreachable!("fake provider health future is immediately ready"),
    }
}
