//! Compile-time probes from a Cargo package outside the Serea workspace.
//!
//! This is deliberately not a runtime dispatcher. It checks which APIs an
//! arbitrary dependent crate can reach and which crate dependencies Cargo
//! actually resolves. `serea-host` is a binary-only package, so its live
//! Provider composition cannot be imported by downstream crates.

use serea_protocol::{
    ActionError, ActionRequest, ActionResult, CapabilityDescriptor, CapabilityProvider,
    EpochMillis, GrantId, ProviderContext, ProviderId, SereaEvent, StepId, TaskId,
};
use serea_storage::{GrantConsumptionOutcome, Store, StoreError, Tx};

/// Arbitrary consumers can compose a valid public P6 grant-use operation in
/// the public transaction callback without calling TaskEngine or Policy.
/// This consumes an enumerated Grant member only; it does not create an Intent
/// or prove permission to invoke a Provider.
pub fn external_grant_use_composer(
    store: &Store,
    grant: &GrantId,
    task: &TaskId,
    step: &StepId,
    now: EpochMillis,
) -> Result<GrantConsumptionOutcome, StoreError> {
    store.transact(|tx| tx.consume_grant_use(grant, task, step, now))
}

/// A downstream caller can append an Event Protocol record through Tx. The
/// Event row is therefore not proof of an atomic Dispatch Intent by itself.
pub fn external_event_composer(
    tx: &mut Tx<'_>,
    event: SereaEvent,
    retention_at: Option<EpochMillis>,
) -> Result<SereaEvent, StoreError> {
    tx.append_event(event, retention_at)
}

/// These imports establish that the probe links the real Policy, TaskEngine,
/// Core, Host, Capability and Protocol crates alongside Storage.
pub fn compile_real_crate_surface(
    _evaluation: Option<serea_policy::PolicyEvaluation>,
    _engine: Option<serea_task_engine::TaskEngine>,
    _prepared: Option<serea_capability::PreparedActionV1>,
    _core: Option<serea_core::Core>,
    _tx: Option<Tx<'_>>,
) {
}

/// Any downstream crate can implement the provider port and inherit its
/// default Ready health. This is a caller-owned provider implementation, not
/// evidence that the physical Host registry contains that instance.
pub struct ArbitraryExternalProvider;

#[async_trait::async_trait]
impl CapabilityProvider for ArbitraryExternalProvider {
    fn provider_id(&self) -> ProviderId {
        ProviderId::new("external.synthetic").unwrap()
    }

    fn capabilities(&self) -> Vec<CapabilityDescriptor> {
        Vec::new()
    }

    async fn invoke(
        &self,
        _request: &ActionRequest,
        _ctx: &ProviderContext,
    ) -> Result<ActionResult, ActionError> {
        unimplemented!("compile-only downstream provider")
    }
}

/// Public fields let a caller detach the outer decision from the decision
/// sealed inside `AuthorizationEvidenceV1`. No dispatch API currently accepts
/// this value, and it must never become authorization input by itself.
pub fn caller_relabels_policy_evaluation(
    evidence: serea_policy::AuthorizationEvidenceV1,
) -> serea_policy::PolicyEvaluation {
    serea_policy::PolicyEvaluation {
        decision: serea_policy::PolicyDecision::Allow,
        evidence,
    }
}
