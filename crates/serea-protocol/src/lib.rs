//! `serea-protocol` — the frozen Serea wire contracts.
//!
//! This crate is layer `L0` (`docs/architecture/03-crate-map.md` §2): it
//! depends on no other workspace crate, and every other crate depends on it.
//! It holds the identifier grammar, the frozen wire types and enums, the
//! checked-in JSON Schema contracts, and the provider *port* declarations.
//!
//! It holds no orchestration, no persistence, no scheduler, and no network
//! client.
//!
//! On credentials, the accurate claim is the narrow one: **no type here is
//! *designed* to carry secret material.** There is no `Secret<T>`, no byte
//! buffer, no field whose job is holding a token, and `CredentialHandle` is a
//! digest-shaped reference that Data Classification §5 explicitly permits in a
//! log. But several fields — `ActionError.message`, `ActionError.details`,
//! `PlainSummary`, `ProviderReference`, `ActorId`, `LeaseOwner`,
//! `EffectSummary`, `ModelMessage.content`, `ModelResponse.content` — are
//! validated strings with `Debug`, `Display` and `Serialize`, and a caller *can*
//! put a live token in one. `ActionError.message` is therefore the carrier a
//! P2+ provider author must be reviewed for (`AB-13`, `DC7`); see
//! `docs/plans/P1-closure.md`.
//!
//! Two items on this crate's frozen public surface (Crate Map §3.1) are
//! deliberately **not** defined here, and no other crate may define them in
//! their place:
//!
//! * `Secret<T>` — Data Classification §3.2 declares it in this crate. Its byte
//!   custody is `serea-credential-store`'s, but the type itself is ours, and
//!   `From<Secret<Vec<u8>>> for CredentialHandle` is part of that frozen pair.
//!   P1 defines only the other half, [`types::CredentialHandle`]; defining
//!   `Secret<T>` needs `zeroize` and `subtle` and a decision about
//!   `ZeroizeOnDrop` bounds that the P1 plan does not make.
//! * `GoalHandle`, `GoalObservedState`, `GoalEvidenceRef`, `GoalArtifactRef`,
//!   `GoalSummary` — GoalLatch Adapter §3.1 freezes the vocabulary a fake and a
//!   real adapter share, and §3 states those types travel *inside* `arguments`
//!   and `output` as schema-validated JSON. P1 declares the
//!   `HostGoalProvider` port and no adapter, so there is nothing for them to
//!   travel in yet.
//! * `PolicyDecision` and `DenyReason` — owned by `serea-policy` (Crate Map
//!   §3.1) and P2. The types P1 does define for the same axis are
//!   [`types::RiskClass`] and [`types::Authorization`], which Policy Protocol §2
//!   and Capability Protocol §3.1 actually specify.
//! * `ApprovalRequest` and `ApprovalGrant` — owned by `serea-capability`
//!   (Crate Map §3.1) and P6. P1 defines `ApprovalId` and `GrantId` (Protocol
//!   Index §2) and nothing about a grant's semantics.
//! * `DeviceLinkPort` — Crate Map §2.1 places the `serea.device/2` wire types
//!   and the port here, but the link itself is `serea-core`'s and P12.
//! * `Ids` — P1 provides [`ids::UlidSource`] and [`ids::IdMinter`] for minting
//!   (Protocol Index §2 rule 1), under the frozen protocol names rather than
//!   the Crate Map's shorthand.
//!
//! P2B adds the declared [`Clock`] injection port (Crate Map §3; Model Protocol
//! §10), implemented by `serea_testkit::TestClock`, and the signed, wire-bounded
//! [`EpochMillis`] instant. ULID-specific [`TimestampMs`] remains separate.
//!
//! The protocol documents under `docs/protocols/` are normative. Nothing here
//! restates them as a second contract: every public item cites the section it
//! implements.

#![forbid(unsafe_code)]
#![deny(missing_docs)]
// No `unwrap`, `expect`, or `panic!` in the contract layer: validation of
// untrusted protocol input must produce a typed error, never a control transfer.
// `unreachable!` and `assert!` are deliberately *not* covered by this list and
// are used only where the invariant is provable at the call site (for example
// `"goallatch"` matching the frozen `ProviderId` grammar); each site carries the
// reasoning that makes it unreachable.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

pub mod canonical;
pub mod clock;
pub mod errors;
pub mod ids;
pub mod provider;
pub mod scheduler_contracts;
pub mod schema;
pub mod types;

pub use canonical::{CanonicalJsonError, canonicalize, derive_idempotency_key, digest_of};
pub use clock::{Clock, EpochMillis};
pub use errors::{
    ContractRule, IdentifierDomain, IdentifierRejection, ProtocolError, SerializationRejection,
    ValueField, ValueRejection,
};
pub use ids::{
    ApprovalId, CAPABILITY_VERBS, CapabilityId, DeviceId, Digest, EventId, GrantId, IdMinter,
    IdempotencyKey, ImplementationId, ModelId, ProposalId, ProviderId, ReceiptId, RequestId,
    ScheduleId, SessionId, StepId, TaskId, TimestampMs, UlidSource, UlidValue,
};
pub use provider::{
    CancellationToken, CapabilityProvider, HostGoalProvider, ModelCallContext, ModelProvider,
    ProviderContext,
};
pub use scheduler_contracts::{
    ApprovalLifecyclePayloadError, ApprovalLifecyclePayloadV1, DeviceConnectedPayloadError,
    DeviceConnectedPayloadV1, EventPredicateError, EventPredicateV1, MAX_SCHEDULE_TEMPLATE_BYTES,
    ScheduledTaskTemplateV1, TemplateError, event_kind_is_host_event_eligible,
};
pub use schema::{MAX_INSTANCE_DEPTH, SchemaError, SchemaName, SchemaViolation};
pub use types::{
    ActionError, ActionErrorKind, ActionRequest, ActionResult, ActionStatus, Actor, ActorId,
    ActorKind, AssistantTask, AttemptBudget, Authorization, BlockedReason, CapabilityDescriptor,
    CapabilityDescriptorDraft, CostClass, CredentialHandle, DataClass, DescriptorDescription,
    DescriptorTitle, EffectSummary, Envelope, EnvelopeVersion, ErrorCode, ErrorMessage, EventKind,
    Evidence, EvidenceKind, Extensions, FailureReason, FinishReason, HostAction,
    IdempotencySupport, JsonSchemaMode, JsonSchemaRef, LeaseOwner, MessageRole, ModelCapabilities,
    ModelDescriptor, ModelError, ModelErrorCode, ModelMessage, ModelPurpose, ModelRequest,
    ModelResponse, ModelUsage, PlainSummary, ProviderHealth, ProviderReference, ReasonCode,
    ReplaySafety, RequestedBy, ResponseFormat, RiskClass, RootRequirement, SemVer, Seq, SereaEvent,
    SideEffectClass, SideEffectReceipt, StepKind, StepPresence, StepStatus, TaskKind, TaskOrigin,
    TaskOriginKind, TaskState, TaskStep, TaskStepDraft, TaskTitle, TextCategory, Timestamp,
    TokenCount, Trace, WireSurface, text_pattern,
};
