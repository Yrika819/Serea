//! A scripted, in-memory capability provider.
//!
//! GoalLatch Adapter §6 states the principle this double follows: "The fake
//! returns real results. A test double that returns structurally-simplified
//! results tests nothing, and the bugs it hides are the ones that matter"
//! (`G5`). So `MockCapabilityProvider` serves the exact `ActionResult` a test
//! scripted, receipts included, rather than a convenience shape.

use std::collections::VecDeque;
use std::sync::Mutex;

use async_trait::async_trait;

use serea_protocol::provider::{CapabilityProvider, ProviderContext};
use serea_protocol::{
    ActionError, ActionRequest, ActionResult, Authorization, CapabilityDescriptor,
    CapabilityDescriptorDraft, CostClass, CredentialHandle, DataClass, DescriptorDescription,
    DescriptorTitle, IdempotencySupport, JsonSchemaRef, ProviderHealth, ProviderId, ReplaySafety,
    RiskClass, RootRequirement, SemVer, SideEffectClass,
};

/// One scripted outcome for a capability invocation.
///
/// The result variant is boxed: an `ActionResult` carrying evidence and a
/// receipt is an order of magnitude larger than an `ActionError`, and an
/// unboxed enum would waste that space on every scripted failure.
#[derive(Debug, Clone, PartialEq)]
pub enum CapabilityScript {
    /// Serve this exact result, receipt and evidence included.
    Succeed(Box<ActionResult>),
    /// Fail with this exact typed error.
    Fail(ActionError),
}

/// A `CapabilityProvider` that replays a scripted queue and never leaves the
/// process.
///
/// An exhausted queue repeats the last entry rather than panicking, so an
/// under-scripted test still gets a deterministic answer.
#[derive(Debug)]
pub struct MockCapabilityProvider {
    provider_id: ProviderId,
    descriptors: Vec<CapabilityDescriptor>,
    script: Mutex<VecDeque<CapabilityScript>>,
    calls: Mutex<u64>,
    /// Every credential handle the provider was handed. Recorded so a test can
    /// assert that only handles arrived and no secret bytes were ever in scope;
    /// the type has no field that could hold bytes.
    seen_handles: Mutex<Vec<CredentialHandle>>,
}

impl MockCapabilityProvider {
    /// A provider advertising `descriptors`, with no scripted outcome yet.
    pub fn new(provider_id: ProviderId, descriptors: Vec<CapabilityDescriptor>) -> Self {
        Self {
            provider_id,
            descriptors,
            script: Mutex::new(VecDeque::new()),
            calls: Mutex::new(0),
            seen_handles: Mutex::new(Vec::new()),
        }
    }

    /// Appends one scripted outcome.
    pub fn push(&self, entry: CapabilityScript) -> &Self {
        self.script
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .push_back(entry);
        self
    }

    /// Appends a scripted result.
    pub fn push_result(&self, result: ActionResult) -> &Self {
        self.push(CapabilityScript::Succeed(Box::new(result)))
    }

    /// Appends a scripted typed failure.
    pub fn push_error(&self, error: ActionError) -> &Self {
        self.push(CapabilityScript::Fail(error))
    }

    /// How many calls this provider has served.
    pub fn calls(&self) -> u64 {
        *self
            .calls
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// The distinct credential handles the provider was handed.
    pub fn seen_handles(&self) -> Vec<CredentialHandle> {
        self.seen_handles
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
    }

    fn next_script(&self) -> Option<CapabilityScript> {
        let mut script = self
            .script
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if script.is_empty() {
            return None;
        }
        if script.len() == 1 {
            return script.front().cloned();
        }
        script.pop_front()
    }
}

#[async_trait]
impl CapabilityProvider for MockCapabilityProvider {
    fn provider_id(&self) -> ProviderId {
        self.provider_id.clone()
    }

    fn capabilities(&self) -> Vec<CapabilityDescriptor> {
        self.descriptors.clone()
    }

    async fn invoke(
        &self,
        _request: &ActionRequest,
        ctx: &ProviderContext,
    ) -> Result<ActionResult, ActionError> {
        *self
            .calls
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) += 1;
        if let Some(handle) = ctx.credential_handle() {
            self.seen_handles
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .push(handle.clone());
        }
        match self.next_script() {
            Some(CapabilityScript::Succeed(result)) => Ok(*result),
            Some(CapabilityScript::Fail(error)) => Err(error),
            None => Err(ActionError {
                kind: serea_protocol::ActionErrorKind::Internal,
                code: serea_protocol::ErrorCode::new("SCRIPT_EXHAUSTED").unwrap_or_else(|error| {
                    unreachable!("a frozen code shape is valid: {error:?}")
                }),
                message: serea_protocol::ErrorMessage::new(
                    "no scripted outcome was registered for this call",
                )
                .unwrap_or_else(|error| unreachable!("a frozen message shape is valid: {error:?}")),
                retryable: false,
                host_action: serea_protocol::HostAction::new("NONE").unwrap_or_else(|error| {
                    unreachable!("a frozen code shape is valid: {error:?}")
                }),
                details: serde_json::Map::new(),
            }),
        }
    }

    async fn health(&self) -> ProviderHealth {
        ProviderHealth::Ready
    }
}

/// Builds a descriptor from a fully stated draft.
///
/// Every authority-bearing field is written at the call site, which is the point:
/// `risk_class`, `required_authorization`, `replay_safety` and `data_class` must
/// never be filled in by a helper a reviewer cannot see (Capability Protocol §1,
/// `C3`). Use [`synthetic_read_descriptor`] only when the capability genuinely
/// is a low-authority read.
pub fn synthetic_descriptor(draft: CapabilityDescriptorDraft) -> CapabilityDescriptor {
    CapabilityDescriptor::new(draft).unwrap_or_else(|error| {
        unreachable!("a scripted descriptor with a matching namespace is registrable: {error:?}")
    })
}

/// A synthetic `read` descriptor: `OBSERVE`, `NONE` side effects, idempotent,
/// no authorization, no root, `PERSONAL` data.
///
/// This is the only convenience the testkit offers, and it is deliberately the
/// *lowest* authority a capability can have, so a test that wants anything else
/// has to state it.
pub fn synthetic_read_descriptor(
    provider: &str,
    capability: &str,
    version: &str,
) -> CapabilityDescriptor {
    synthetic_descriptor(CapabilityDescriptorDraft {
        id: capability_id(capability),
        version: semver(version),
        title: descriptor_title("synthetic read capability"),
        description: descriptor_description("synthetic read descriptor for offline tests"),
        provider_id: provider_id(provider),
        implementation_id: None,
        input_schema: schema_reference(capability, version, "input"),
        output_schema: schema_reference(capability, version, "output"),
        side_effect_class: SideEffectClass::None,
        risk_class: RiskClass::Observe,
        required_authorization: Authorization::None,
        replay_safety: ReplaySafety::Idempotent,
        data_class: DataClass::Personal,
        root_requirement: RootRequirement::NotRequired,
        idempotency_support: IdempotencySupport::Native,
        max_duration_ms: 15_000,
        cost_class: CostClass::Free,
        experimental: false,
    })
}

fn capability_id(value: &str) -> serea_protocol::CapabilityId {
    serea_protocol::CapabilityId::new(value.to_owned())
        .unwrap_or_else(|error| unreachable!("a scripted capability id must be valid: {error:?}"))
}

fn semver(value: &str) -> SemVer {
    SemVer::new(value)
        .unwrap_or_else(|error| unreachable!("a scripted version must be valid: {error:?}"))
}

fn provider_id(value: &str) -> ProviderId {
    ProviderId::new(value.to_owned())
        .unwrap_or_else(|error| unreachable!("a scripted namespace must be valid: {error:?}"))
}

fn descriptor_title(value: &str) -> DescriptorTitle {
    DescriptorTitle::new(value)
        .unwrap_or_else(|error| unreachable!("a frozen title shape is valid: {error:?}"))
}

fn descriptor_description(value: &str) -> DescriptorDescription {
    DescriptorDescription::new(value)
        .unwrap_or_else(|error| unreachable!("a frozen description shape is valid: {error:?}"))
}

fn schema_reference(capability: &str, version: &str, direction: &str) -> JsonSchemaRef {
    JsonSchemaRef::new(format!(
        "https://serea.local/schemas/{capability}.{direction}.{version}.json"
    ))
    .unwrap_or_else(|error| unreachable!("a generated reference is valid: {error:?}"))
}
