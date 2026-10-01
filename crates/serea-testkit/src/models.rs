//! A scripted, in-memory model provider.
//!
//! Model Protocol §10: `MockModelProvider` scripts exact responses, including
//! malformed JSON, schema-violating objects, injected `finish_reason` values,
//! and transport errors. It is the primary tool for every failure-path test in
//! the system. Model Protocol §10 also states "No test may reach a real model
//! provider" (`M10`); nothing in this crate opens a socket.

use std::collections::VecDeque;
use std::sync::Mutex;

use async_trait::async_trait;

use serea_protocol::provider::{ModelCallContext, ModelProvider};
use serea_protocol::{ModelDescriptor, ModelError, ProviderHealth, ProviderId};

/// One scripted outcome for a model call.
#[derive(Debug, Clone, PartialEq)]
pub enum ModelScript {
    /// Serve this exact response.
    Respond(serea_protocol::ModelResponse),
    /// Fail with this exact typed error.
    Fail(ModelError),
}

impl ModelScript {
    /// Convenience for the common success case.
    pub fn ok(response: serea_protocol::ModelResponse) -> Self {
        ModelScript::Respond(response)
    }

    /// Convenience for the common typed-failure case.
    pub fn err(error: ModelError) -> Self {
        ModelScript::Fail(error)
    }
}

/// A `ModelProvider` that replays a scripted queue and never leaves the process.
///
/// Behaviour worth knowing:
///
/// * The queue is consumed in order. An exhausted queue repeats its **last**
///   entry rather than panicking, so a test that under-scripts still gets a
///   deterministic answer instead of a flaky one.
/// * The default roster contains no `codex`. `codex` is known but disabled
///   (Model Protocol §5.1) and is never a routing candidate (`M5`).
#[derive(Debug)]
pub struct MockModelProvider {
    provider_id: ProviderId,
    models: Vec<ModelDescriptor>,
    script: Mutex<VecDeque<ModelScript>>,
    calls: Mutex<u64>,
}

impl MockModelProvider {
    /// A provider serving `models`, with no scripted outcome yet.
    pub fn new(provider_id: ProviderId, models: Vec<ModelDescriptor>) -> Self {
        Self {
            provider_id,
            models,
            script: Mutex::new(VecDeque::new()),
            calls: Mutex::new(0),
        }
    }

    /// Appends one scripted outcome.
    pub fn push(&self, entry: ModelScript) -> &Self {
        self.script
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .push_back(entry);
        self
    }

    /// Appends a scripted response.
    pub fn push_response(&self, response: serea_protocol::ModelResponse) -> &Self {
        self.push(ModelScript::Respond(response))
    }

    /// Appends a scripted typed failure.
    pub fn push_error(&self, error: ModelError) -> &Self {
        self.push(ModelScript::Fail(error))
    }

    /// How many calls this provider has served.
    pub fn calls(&self) -> u64 {
        *self
            .calls
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// Whether the served roster names `codex`. Always `false` for a default
    /// roster; a caller can only make it `true` by naming it explicitly, which
    /// is why the testkit asserts it stays `false`.
    pub fn advertises_codex(&self) -> bool {
        self.models
            .iter()
            .any(|model| model.model_id.as_str() == crate::CODEX_MODEL_ID)
    }

    fn next_script(&self) -> Option<ModelScript> {
        let mut script = self
            .script
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if script.is_empty() {
            return None;
        }
        if script.len() == 1 {
            // Repeat the last entry rather than panicking, so an under-scripted
            // test stays deterministic.
            return script.front().cloned();
        }
        script.pop_front()
    }
}

#[async_trait]
impl ModelProvider for MockModelProvider {
    fn provider_id(&self) -> ProviderId {
        self.provider_id.clone()
    }

    fn models(&self) -> Vec<ModelDescriptor> {
        self.models.clone()
    }

    async fn generate(
        &self,
        _request: &serea_protocol::ModelRequest,
        _ctx: &ModelCallContext,
    ) -> Result<serea_protocol::ModelResponse, ModelError> {
        *self
            .calls
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) += 1;
        match self.next_script() {
            Some(ModelScript::Respond(response)) => Ok(response),
            Some(ModelScript::Fail(error)) => Err(error),
            None => Err(ModelError {
                kind: serea_protocol::ModelErrorCode::new("SCRIPT_EXHAUSTED").unwrap_or_else(
                    |error| unreachable!("a frozen code shape is valid: {error:?}"),
                ),
                message: serea_protocol::ErrorMessage::new(
                    "no scripted outcome was registered for this call",
                )
                .unwrap_or_else(|error| unreachable!("a frozen message shape is valid: {error:?}")),
                retryable: false,
            }),
        }
    }

    async fn health(&self) -> ProviderHealth {
        ProviderHealth::Ready
    }
}

/// Two of the four frozen roster entries from Model Protocol §5.1, chosen
/// because they are the two the P1 protocol tests exercise a `STRUCTURED`
/// path through.
///
/// The other two are deliberately absent, for different reasons:
///
/// * `gemma-4-31b` is the vision model and no P1 capability input is an image;
///   a roster that claims `vision: false` for it would contradict §5.1.
/// * `codex` is *known but disabled* (§5.1) and is never a routing candidate
///   (`M5`). A testkit roster is a list a test can iterate, so it must not carry
///   it; `MockModelProvider::advertises_codex` and the exclusion tests depend on
///   that.
///
/// §5.1 marks only `nemotron-3-nano-30b` as fast, so only that descriptor sets
/// `fast: true`. `fast` is a Model Protocol §6 routing filter input, so a flag
/// the frozen roster does not state would be an invented routing property.
pub fn synthetic_model_roster() -> Vec<ModelDescriptor> {
    vec![
        descriptor("nemotron-3-nano-30b", true, true),
        descriptor("gpt-oss-20b", true, false),
    ]
}

fn descriptor(model_id: &str, strict_structured_output: bool, fast: bool) -> ModelDescriptor {
    ModelDescriptor {
        model_id: serea_protocol::ModelId::new(model_id).unwrap_or_else(|error| {
            unreachable!("the frozen roster names only valid model identifiers: {error:?}")
        }),
        provider_id: ProviderId::new("ollama").unwrap_or_else(|error| {
            unreachable!("the frozen roster names only valid namespaces: {error:?}")
        }),
        capabilities: serea_protocol::ModelCapabilities {
            vision: false,
            tools: true,
            structured_output: strict_structured_output,
            json_schema_mode: if strict_structured_output {
                serea_protocol::JsonSchemaMode::Strict
            } else {
                serea_protocol::JsonSchemaMode::Unsupported
            },
            thinking: false,
            long_context: false,
            fast,
            code_specialist: false,
            max_context_tokens: 131_072,
            max_output_tokens: 8_192,
            supports_streaming: true,
            supports_seeds: false,
        },
    }
}
