//! The provider *ports*: the lowest-layer trait declarations Serea's
//! dependency inversion rests on.
//!
//! Frozen source: `docs/protocols/01-capability-protocol.md` §9
//! (`CapabilityProvider`), `docs/protocols/03-model-protocol.md` §2
//! (`ModelProvider`), and `docs/protocols/08-goallatch-adapter-protocol.md` §3
//! (`HostGoalProvider`).
//!
//! Crate Map §1 rule 3 places ports in the lowest layer and implementations in
//! the highest, which is what makes the graph acyclic and providers swappable.
//! This module therefore declares three traits and implements none of them.
//! There is **no** GoalLatch provider and no GoalLatch fake here: the fake is
//! P15 work (`docs/architecture/README.md` §6), and P1 must not create a real
//! connection or imply these capabilities are available at runtime
//! (GoalLatch Adapter §4, `G13`).
//!
//! `async-trait` keeps the ports `dyn`-composable, because `serea-core`
//! registers providers as trait objects (Crate Map §6.1 rule 1). It is a
//! procedural macro only and introduces no async runtime.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use async_trait::async_trait;

use crate::ids::{ImplementationId, ProviderId};
use crate::types::{
    ActionError, ActionRequest, ActionResult, CapabilityDescriptor, CredentialHandle, ModelError,
    ModelRequest, ModelResponse, ProviderHealth,
};

/// Cooperative cancellation for one in-flight call.
///
/// Capability Protocol §9 requires `ProviderContext` to carry a cancellation
/// token. P1 has no runtime and no task engine, so this is the data shape only:
/// a flag a host sets and a provider observes at its own next checkpoint. It
/// does not schedule anything, wake anything, or abort anything — actually
/// aborting a call is Task Protocol §5's job and belongs to
/// `serea-task-engine` in P2. A provider written against this type will read
/// "cancellation" and expect a future; there is none yet.
#[derive(Debug, Clone, Default)]
pub struct CancellationToken(Arc<AtomicBool>);

impl CancellationToken {
    /// A token that is not cancelled.
    pub fn new() -> Self {
        Self::default()
    }

    /// Requests cancellation. Cooperative: a provider observes it at its own
    /// next checkpoint, never mid-effect.
    pub fn cancel(&self) {
        self.0.store(true, Ordering::SeqCst);
    }

    /// Whether cancellation has been requested.
    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }
}

/// What a provider is given for one call (Capability Protocol §9).
///
/// A provider receives no ambient authority: it cannot read policy, approve
/// itself, escalate, or reach another provider's credentials (`C8`, `TB-8`).
/// The secret itself is never present — only an opaque [`CredentialHandle`] to
/// resolve inside the credential-store process boundary
/// (Data Classification §3.1, `DC6`).
#[derive(Debug, Clone)]
pub struct ProviderContext {
    descriptor: CapabilityDescriptor,
    deadline_ms: u32,
    cancellation: CancellationToken,
    credential_handle: Option<CredentialHandle>,
}

impl ProviderContext {
    /// Assembles the context the host hands a provider for one call.
    ///
    /// A single constructor rather than public fields, so the value a provider
    /// receives always comes from one place. Note what this does **not** do: it
    /// cannot check that `descriptor` matches `request.capability_id`, because
    /// the request is a separate argument. Capability Protocol §1 places the
    /// capability-to-descriptor binding in the registry, upstream of the
    /// provider, so a provider must treat `descriptor()` as "the descriptor the
    /// host resolved for this call" and must not treat it as independent proof
    /// about the request it was handed.
    pub fn new(
        descriptor: CapabilityDescriptor,
        deadline_ms: u32,
        cancellation: CancellationToken,
        credential_handle: Option<CredentialHandle>,
    ) -> Self {
        Self {
            descriptor,
            deadline_ms,
            cancellation,
            credential_handle,
        }
    }

    /// The resolved descriptor the call is pinned to. Immutable for the
    /// lifetime of a task that may reference it (Capability Protocol §3.2).
    pub fn descriptor(&self) -> &CapabilityDescriptor {
        &self.descriptor
    }

    /// The host-resolved deadline in milliseconds. Never model-supplied.
    pub fn deadline_ms(&self) -> u32 {
        self.deadline_ms
    }

    /// The cancellation signal. Cooperative: a provider observes it at its own
    /// next checkpoint, never mid-effect.
    pub fn cancellation(&self) -> &CancellationToken {
        &self.cancellation
    }

    /// The opaque credential reference, if the call has one. Never the secret.
    pub fn credential_handle(&self) -> Option<&CredentialHandle> {
        self.credential_handle.as_ref()
    }
}

/// What a model provider is given for one call (Model Protocol §3).
///
/// `Model Protocol` §2 names `ModelCallContext` and freezes no field set, so
/// P1 carries the one field the trait's determinism requirement implies: the
/// host-resolved deadline. Everything else a provider might need — prompt
/// contents, schema, tool definitions — travels inside the [`ModelRequest`],
/// which keeps the context free of ambient state (Model Protocol §10).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ModelCallContext {
    /// The host-resolved deadline in milliseconds.
    pub deadline_ms: u32,
}

/// A source of capability actions (Capability Protocol §9).
#[async_trait]
pub trait CapabilityProvider: Send + Sync {
    /// The provider's own namespace. A provider may not register a capability
    /// outside it (Protocol Index §3).
    fn provider_id(&self) -> ProviderId;

    /// The capabilities this provider advertises. A provider that cannot honour
    /// a descriptor withdraws it rather than advertising it loosely (`C7`).
    fn capabilities(&self) -> Vec<CapabilityDescriptor>;

    /// Performs one action. The only place a side effect occurs
    /// (Capability Protocol §1).
    async fn invoke(
        &self,
        request: &ActionRequest,
        ctx: &ProviderContext,
    ) -> Result<ActionResult, ActionError>;

    /// The provider's health. `Ready` unless the provider has withdrawn
    /// itself.
    async fn health(&self) -> ProviderHealth {
        ProviderHealth::Ready
    }
}

/// A source of model calls (Model Protocol §2).
#[async_trait]
pub trait ModelProvider: Send + Sync {
    /// The provider's own identity.
    fn provider_id(&self) -> ProviderId;

    /// The roster entries this provider serves. Codex is known but disabled and
    /// never appears here (Model Protocol §5.1, §8, `M5`).
    fn models(&self) -> Vec<crate::types::ModelDescriptor>;

    /// Performs one model call.
    async fn generate(
        &self,
        request: &ModelRequest,
        ctx: &ModelCallContext,
    ) -> Result<ModelResponse, ModelError>;

    /// The provider's health. `Ready` unless the provider has withdrawn
    /// itself.
    async fn health(&self) -> ProviderHealth {
        ProviderHealth::Ready
    }
}

/// The only sanctioned seam to a host goal runner (GoalLatch Adapter §3).
///
/// This is **not** a [`CapabilityProvider`] and registers no descriptors. A thin
/// registry shim does that, and the shim is the only caller of this trait
/// (GoalLatch Adapter §3.2). P1 declares the port and nothing else: there is
/// no implementation, no `FakeGoalLatchProvider`, and no `local_mcp` path
/// anywhere in this workspace (`G1`, `G3`, `G7`).
#[async_trait]
pub trait HostGoalProvider: Send + Sync {
    /// The adapter's own identity. Not a registered capability namespace: no
    /// `CapabilityDescriptor` may begin with `goallatch.`
    /// (GoalLatch Adapter §3.2, `G13`).
    fn provider_id(&self) -> ProviderId {
        ProviderId::new("goallatch").unwrap_or_else(|error| {
            unreachable!("the frozen adapter identity matches the frozen grammar: {error:?}")
        })
    }

    /// Which implementation this is, for example `fake-goallatch`.
    fn implementation_id(&self) -> ImplementationId;

    /// `host.goal.start` → `start`.
    async fn start(
        &self,
        request: &ActionRequest,
        ctx: &ProviderContext,
    ) -> Result<ActionResult, ActionError>;

    /// `host.goal.status` → `status`.
    async fn status(
        &self,
        request: &ActionRequest,
        ctx: &ProviderContext,
    ) -> Result<ActionResult, ActionError>;

    /// `host.goal.run` → `run`.
    async fn run(
        &self,
        request: &ActionRequest,
        ctx: &ProviderContext,
    ) -> Result<ActionResult, ActionError>;

    /// `host.goal.cancel` → `cancel`.
    async fn cancel(
        &self,
        request: &ActionRequest,
        ctx: &ProviderContext,
    ) -> Result<ActionResult, ActionError>;

    /// `host.goal.result` → `result`.
    async fn result(
        &self,
        request: &ActionRequest,
        ctx: &ProviderContext,
    ) -> Result<ActionResult, ActionError>;

    /// The adapter's health. `Ready` unless the adapter has withdrawn itself.
    async fn health(&self) -> ProviderHealth {
        ProviderHealth::Ready
    }
}
